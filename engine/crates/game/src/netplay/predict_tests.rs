//! The predictor's rules on the asset-free fixture battle: the copy never
//! reaches the confirmed sim, it predicts what the confirmed steps do,
//! corrections ease out, and own shots feed back once.

use super::*;
use crate::walk::WalkSim;
use core::sim::{buttons, PadState};

fn walk() -> WalkSim {
    let mut w = WalkSim::new(3, Vec3::ZERO, None);
    w.blend.on = true;
    for _ in 0..3 {
        w.step_tick(&[PadState::default()]);
    }
    w
}

fn run(turn: f32, b: u32) -> PadState {
    PadState {
        ly: PadState::quantize(1.0),
        rx: PadState::quantize(turn),
        buttons: b,
        ..PadState::default()
    }
}

fn me(w: &WalkSim) -> UnitPose {
    UnitPose::of(&w.sim.units[w.player.index as usize])
}

/// Swapping the prediction in and out (inside the render blend, as a frame
/// draws) leaves the confirmed sim's hash unchanged, draws the predicted
/// soldier ahead of the confirmed one, and the prediction is what the
/// confirmed steps over the same inputs do (no bots to disagree).
#[test]
fn prediction_swaps_in_and_out_and_matches_the_confirmed_steps() {
    let mut w = walk();
    let pending = [run(0.2, 0), run(0.4, 0), run(0.0, 0), run(-0.3, 0)];
    let mut p = Predictor::new();
    p.update(&w.sim, 0, &pending, 1.0 / 60.0);
    assert_eq!(p.depth(), 4);
    assert_eq!(p.stats.mode, PredictMode::Full);
    assert_eq!(p.poses().len(), 5);
    let hash = w.sim.hash();
    let confirmed = me(&w);
    for frac in [0.0, 0.5, 1.0] {
        w.set_frame_fraction(0.5);
        w.blend_in();
        assert!(p.swap_in(&mut w, frac, &[], 0.0));
        let drawn = me(&w);
        assert!(
            drawn.pos.distance(confirmed.pos) > 0.1,
            "drawn ahead: {drawn:?}"
        );
        p.swap_out(&mut w);
        w.blend_out();
        assert_eq!(w.sim.hash(), hash, "the prediction reached the sim");
    }
    // At fraction 1 the newest predicted tick is drawn.
    w.blend_in();
    p.swap_in(&mut w, 1.0, &[], 0.0);
    assert_eq!(me(&w).pos, p.poses()[4].unit.pos);
    p.swap_out(&mut w);
    w.blend_out();
    for (k, pad) in pending.iter().enumerate() {
        w.step_tick(&[*pad]);
        let want = &p.poses()[k + 1];
        assert_eq!(want.tick, w.sim.tick);
        let got = me(&w);
        assert_eq!(
            (got.pos, got.yaw, got.pitch),
            (want.unit.pos, want.unit.yaw, want.unit.pitch)
        );
    }
}

/// A prediction that the confirmed sim contradicts (the host repeated a
/// standing pad where the player ran) is drawn where the old one was, then
/// eased onto the new one over 0.25 s.
#[test]
fn a_correction_eases_out_over_a_quarter_second() {
    let mut w = walk();
    let pending = [run(0.0, 0); 4];
    let mut p = Predictor::new();
    p.update(&w.sim, 0, &pending, 1.0 / 60.0);
    let old_newest = p.poses()[4].unit.pos;
    // The confirmed tick stepped a standing pad instead of the first input.
    w.step_tick(&[PadState::default()]);
    let b = net::TickBundle {
        tick: w.sim.tick as u32 - 1,
        pads: vec![(0, PadState::default())],
        seqs: vec![1],
        ..Default::default()
    };
    p.note_bundle(&b, &w.sim, 0);
    p.update(&w.sim, 0, &pending[1..], 0.0);
    assert_eq!(p.stats.corrections, 1);
    let new_newest = p.poses().last().unwrap().unit.pos;
    let err = old_newest - new_newest;
    assert!(err.length() > 0.05, "{err}");
    assert!((p.ease().now().0 - err).length() < 1e-5);
    // Drawn where the old prediction was: no snap.
    w.blend_in();
    p.swap_in(&mut w, 1.0, &[], 0.0);
    assert!(me(&w).pos.distance(old_newest) < 1e-4);
    p.swap_out(&mut w);
    w.blend_out();
    // Half way after 0.125 s, gone after 0.25 s (no new confirmed tick).
    p.update(&w.sim, 0, &pending[1..], 0.125);
    assert!((p.ease().now().0 - err * 0.5).length() < 1e-4);
    p.update(&w.sim, 0, &pending[1..], 0.125);
    assert_eq!(p.ease().now().0, Vec3::ZERO);
    assert_eq!(p.stats.corrections, 1);
}

fn shot(unit: u32, wc: u32) -> Event {
    Event::Shot {
        unit: UnitHandle {
            index: unit,
            gen: 0,
        },
        weapon_class: wc,
        origin: Vec3::ZERO,
        dir: Vec3::Z,
    }
}

/// Shots are numbered per unit and weapon: a predicted shot feeds back
/// once however often it is re-predicted, and mutes the confirmed shot of
/// the same number; other units are not touched.
#[test]
fn own_shot_feedback_plays_once() {
    let mut l = ShotLedger::default();
    let mut out = Vec::new();
    let predict = |l: &mut ShotLedger, out: &mut Vec<PredictedShot>| {
        let mut counts = l.counts();
        l.predicted(&mut counts, &[shot(2, 5), shot(9, 5)], 2, 11, out);
        l.predicted(&mut counts, &[], 2, 12, out);
        l.predicted(&mut counts, &[shot(2, 5)], 2, 13, out);
    };
    predict(&mut l, &mut out);
    assert_eq!(out.len(), 2, "two own shots, the other unit's ignored");
    out.clear();
    predict(&mut l, &mut out);
    assert!(out.is_empty(), "re-predicted shots do not play again");
    assert_eq!(l.outstanding(), 2);
    assert!(l.confirm(&[shot(2, 5)], 2), "confirmed #1 muted");
    assert!(!l.confirm(&[shot(9, 5)], 2), "another unit's shot");
    assert!(l.confirm(&[shot(2, 5)], 2), "confirmed #2 muted");
    assert!(!l.confirm(&[shot(2, 5)], 2), "#3 was never predicted");
    assert_eq!(l.outstanding(), 0);
    // A predicted shot that never comes is forgotten once passed.
    let mut counts = l.counts();
    l.predicted(&mut counts, &[shot(2, 7)], 2, 20, &mut out);
    assert_eq!(l.outstanding(), 1);
    assert!(!l.confirm(&[], 2));
    assert_eq!(l.outstanding(), 1);
    assert!(l.confirm(&[shot(2, 7)], 2));
    assert_eq!(l.outstanding(), 0);
}

/// The same through the predictor on the fixture: firing inputs feed back
/// at once, and the confirmed shots that follow are muted.
#[test]
fn predicted_fire_mutes_the_confirmed_shot() {
    let mut w = walk();
    let pending = [
        run(0.0, buttons::FIRE),
        run(0.0, buttons::FIRE),
        run(0.0, 0),
    ];
    let mut p = Predictor::new();
    let fed = p.update(&w.sim, 0, &pending, 1.0 / 60.0).len();
    assert!(fed > 0, "the fixture soldier fires");
    let mut again = p.clone_for_test();
    assert!(again.update(&w.sim, 0, &pending, 0.0).is_empty());
    let mut muted = 0;
    for pad in pending {
        w.step_tick(&[pad]);
        muted += usize::from(p.confirm_shots(&w.sim, w.player));
    }
    assert!(muted > 0, "the confirmed shot was muted");
    assert_eq!(p.ledger().outstanding(), 0);
}

/// The drawn aim folds the inputs not predicted onto the predicted aim: a
/// mouse look drawn whole, so the swapped-in yaw is where stepping it lands.
#[test]
fn the_swapped_aim_folds_the_rest_of_the_inputs() {
    let mut w = walk();
    let look = PadState {
        mx: PadState::quantize_look(0.3),
        my: PadState::quantize_look(-0.1),
        buttons: buttons::MOUSE_AIM,
        ..PadState::default()
    };
    let pending = [run(0.0, 0), look];
    let mut p = Predictor::new();
    p.max_depth = 1;
    p.update(&w.sim, 0, &pending, 1.0 / 60.0);
    assert_eq!(p.stats.mode, PredictMode::Reduced);
    let hash = w.sim.hash();
    w.blend_in();
    assert!(p.swap_in(&mut w, 1.0, &pending[1..], 0.0));
    let drawn = me(&w);
    p.swap_out(&mut w);
    w.blend_out();
    assert_eq!(w.sim.hash(), hash);
    for pad in pending {
        w.step_tick(&[pad]);
    }
    let after = me(&w);
    assert_eq!((drawn.yaw, drawn.pitch), (after.yaw, after.pitch));
}

impl Predictor {
    /// A predictor holding the same ledger (re-predicting from scratch).
    fn clone_for_test(&self) -> Predictor {
        let mut p = Predictor::new();
        p.main.ledger = self.main.ledger.clone();
        p
    }
}

/// The local saber unit's melee state as a frame might hold it: idle (no
/// pose, a clock left from the last swing), a swing's first ticks, or a
/// block's reaction over a swing's clock.
fn melee(kind: &str, later: f32) -> core::hero_weapon::MeleeState {
    let mut m = core::hero_weapon::MeleeState::default();
    match kind {
        "idle" => m.runtime.time = 0.4 + later,
        "swing" => {
            m.animation = Some("stand_attack1a".into());
            m.runtime.time = 0.2 + later;
        }
        _ => {
            m.animation = Some("stand_block".into());
            m.runtime.time = 0.3 + later;
            m.block_animation = Some("stand_block_front1".into());
            m.block_animation_time = 0.1 + later;
        }
    }
    m
}

/// Issue #168, first VPN playtest: the peer playing a saber hero parted
/// from the others. Drawing (render blend, then the predicted local pose
/// swapped in and out) must leave the confirmed unit exactly as it was
/// whatever the predicted pose's melee is: across idle -> swing start,
/// swing -> block and block -> idle, both ways, its `Sim::hash` and every
/// field are unchanged.
#[test]
fn drawing_a_predicted_saber_swing_or_block_leaves_the_confirmed_sim_unchanged() {
    let pending = [run(0.2, 0), run(0.4, 0), run(0.0, 0)];
    let mut swapped = 0;
    for (confirmed, predicted) in [
        ("idle", "swing"),
        ("swing", "idle"),
        ("swing", "block"),
        ("block", "swing"),
        ("block", "idle"),
        ("idle", "block"),
    ] {
        let mut w = walk();
        let i = w.player.index as usize;
        w.sim.units[i].weapon.melee = Some(melee(confirmed, 0.0));
        let mut view = crate::netplay::LocalView::new();
        view.predictor.update(&w.sim, 0, &pending, 1.0 / 60.0);
        assert_eq!(view.predictor.depth(), pending.len());
        // The copy's swing or block as its poses would hold it.
        let mut drawn = w.sim.units[i].clone();
        for p in &mut view.predictor.main.poses {
            drawn.weapon.melee = Some(melee(predicted, 0.033 * p.seq as f32));
            p.unit.melee = UnitPose::of(&drawn).melee;
        }
        let hash = w.sim.hash();
        let unit = format!("{:?}", w.sim.units[i]);
        for frac in [0.0, 0.5, 1.0] {
            w.set_frame_fraction(frac);
            view.draw_in(&mut w);
            swapped += usize::from(view.predictor.swapped());
            view.draw_out(&mut w);
            assert_eq!(
                w.sim.hash(),
                hash,
                "{confirmed} -> {predicted}: drawing reached the sim"
            );
            assert_eq!(
                format!("{:?}", w.sim.units[i]),
                unit,
                "{confirmed} -> {predicted}: the unit changed"
            );
        }
    }
    assert_eq!(swapped, 18, "every frame drew the predicted pose");
}

/// Issue #194 B18: a machine with two local players predicts both slots in
/// the one copy, each from its own pending inputs (neither repeats a stale
/// pad), and each local's poses are what the confirmed steps over the same
/// two input rows do. Both are drawn swapped in at once and swapped out
/// again with the confirmed sim unchanged.
#[test]
fn prediction_of_two_local_slots_matches_the_confirmed_steps() {
    let mut w = crate::walk::two_locals_walk();
    crate::walk::spawn_locals(&mut w);
    let mine = [run(0.2, 0), run(0.4, 0), run(0.0, 0), run(-0.3, 0)];
    let theirs = [run(-0.5, 0), run(-0.5, 0), run(0.6, 0), run(0.1, 0)];
    let mut p = Predictor::new();
    p.set_extra_slots(&[1]);
    p.update_locals(&w.sim, 0, &mine, &[&theirs], 1.0 / 60.0);
    assert_eq!(p.depth(), 4);
    assert_eq!(p.stats.mode, PredictMode::Full);
    assert_eq!((p.poses_of(0).len(), p.poses_of(1).len()), (5, 5));
    assert_eq!(p.poses_of(0), p.poses());
    assert_ne!(p.poses_of(0)[4].index, p.poses_of(1)[4].index);
    let hash = w.sim.hash();
    let unit = |w: &WalkSim, k: usize| UnitPose::of(&w.sim.units[w.local_unit(k).index as usize]);
    let confirmed = [unit(&w, 0), unit(&w, 1)];
    assert!(p.swap_in_local(0, &mut w, 1.0, &[], 0.0));
    assert!(p.swap_in_local(1, &mut w, 1.0, &[], 0.0));
    assert!(p.swapped());
    for (k, before) in confirmed.iter().enumerate() {
        let drawn = unit(&w, k);
        assert!(
            drawn.pos.distance(before.pos) > 0.1,
            "local {k} drawn ahead"
        );
        assert_eq!(drawn.pos, p.poses_of(k)[4].unit.pos);
    }
    p.swap_out(&mut w);
    assert!(!p.swapped());
    assert_eq!(w.sim.hash(), hash, "the prediction reached the sim");
    for (n, (a, b)) in mine.iter().zip(&theirs).enumerate() {
        w.step_tick(&[*a, *b]);
        for k in 0..2 {
            let want = &p.poses_of(k)[n + 1];
            assert_eq!(want.tick, w.sim.tick);
            let got = unit(&w, k);
            assert_eq!(
                (got.pos, got.yaw, got.pitch),
                (want.unit.pos, want.unit.yaw, want.unit.pitch),
                "local {k} after input {n}"
            );
        }
    }
    // One local predicted alone stays the old single-slot prediction.
    let mut alone = Predictor::new();
    let mut same = Predictor::new();
    alone.update(&w.sim, 0, &mine, 1.0 / 60.0);
    same.update_locals(&w.sim, 0, &mine, &[], 1.0 / 60.0);
    assert_eq!(alone.poses(), same.poses());
}

/// Issue #194 B18 review: two locals whose pending queues differ (one seated
/// late). The copy goes as deep as the longer queue, but the shorter one's
/// poses stop at its own inputs (`depth_of`, where its look fold starts);
/// when its queue grows into the ticks the copy stepped with a repeated pad,
/// the prediction starts over and equals one made fresh from both queues.
#[test]
fn prediction_of_uneven_local_queues_stops_each_at_its_own_inputs() {
    let mut w = crate::walk::two_locals_walk();
    crate::walk::spawn_locals(&mut w);
    let mine = [run(0.2, 0), run(0.4, 0), run(0.0, 0), run(-0.3, 0)];
    let theirs = [run(-0.5, 0), run(0.6, 0), run(0.1, 0)];
    let mut p = Predictor::new();
    p.set_extra_slots(&[1]);
    assert_eq!(
        (p.index_of_slot(0), p.index_of_slot(1), p.index_of_slot(7)),
        (Some(0), Some(1), None)
    );
    p.update_locals(&w.sim, 0, &mine, &[&theirs[..2]], 1.0 / 60.0);
    assert_eq!((p.depth(), p.depth_of(0), p.depth_of(1)), (4, 4, 2));
    assert_eq!((p.poses_of(0).len(), p.poses_of(1).len()), (5, 3));
    let base = p.poses_of(1)[0].seq;
    assert_eq!(p.poses_of(1).last().unwrap().seq, base + 2);
    // Local 1's third input arrives; the confirmed sim has not moved.
    p.update_locals(&w.sim, 0, &mine, &[&theirs], 1.0 / 60.0);
    assert_eq!((p.depth(), p.depth_of(1)), (4, 3));
    let mut fresh = Predictor::new();
    fresh.set_extra_slots(&[1]);
    fresh.update_locals(&w.sim, 0, &mine, &[&theirs], 1.0 / 60.0);
    for k in 0..2 {
        assert_eq!(p.poses_of(k), fresh.poses_of(k), "local {k}");
    }
}
