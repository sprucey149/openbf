//! Card SNPR's scenes: the spot a sniper picks, one sniper a spot, the hold
//! left after a quiet lane, and the pistol close in.

use super::*;
use crate::ai::THINK_PERIOD;
use crate::collision::{Body, Shape};
use crate::cp::CpDef;
use crate::heightfield::Heightfield;
use crate::nav::{Arc, Hint, Hub, NavGraph, RouteTable};
use crate::region::Region;
use crate::sim::{Event, PadState};
use crate::unit::{Controller, SlotSpec, UnitSpec};
use crate::weapon::WeaponClass;
use glam::Mat4;

/// A scoped rifle (the AI sniper table's class): MinRange `min`.
fn sniper_rifle(min: f32) -> WeaponClass {
    WeaponClass {
        name: "sniper_rifle".into(),
        trigger_single: true,
        zoom_max: 8.0,
        yaw_spread: 0.0,
        ai_range: [min, 64.0, 128.0],
        ai_target_kind: 0x07,
        ..WeaponClass::bf2_rifle()
    }
}

/// A pistol: 0 / 16 / 24 m, a person's weapon.
fn pistol() -> WeaponClass {
    WeaponClass {
        name: "pistol".into(),
        ai_range: [0.0, 16.0, 24.0],
        ai_target_kind: 0x07,
        ..WeaponClass::bf2_rifle()
    }
}

fn post(name: &str, x: f32) -> CpDef {
    CpDef {
        name: name.into(),
        pos: Vec3::new(x, 0.0, 0.0),
        capture: Region::sphere(Vec3::new(x, 0.0, 0.0), 6.0),
        control: None,
        value_bleed: 100,
        value_atk: [100, 100],
        value_def: [100, 100],
        spawn_nodes: vec![],
        capture_time: 10.0,
        neutralize_time: 12.0,
    }
}

fn snipe_hint(name: &str, x: f32) -> Hint {
    Hint {
        name: name.into(),
        kind: HINT_SNIPE,
        pos: Vec3::new(x, 0.0, 0.0),
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        radius: None,
        mode: None,
        stance_mask: crate::commander::MASK_STAND,
        secondary_stance_mask: crate::commander::MASK_STAND,
        peek_sides: 0,
        command_post: None,
    }
}

/// Flat ground and a line of hubs at x = 0, 20, 40, 60 and 100; team 0's
/// post at x = 30, team 1's at x = 100 - so the enemy ground is that post
/// and hub 4 on it. Two Snipe hints: NEAR at x = 75 (25 m short of it) and
/// FAR at x = 55 (45 m off, inside the 49 m sight). `[weapon classes]`:
/// the rifle, the pistol.
fn line_scene() -> (Sim, [usize; 2]) {
    let mut sim = Sim::empty(13);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let xs = [0.0, 20.0, 40.0, 60.0, 100.0];
    let mut hubs: Vec<Hub> = xs
        .iter()
        .map(|&x| Hub {
            pos: Vec3::new(x, 0.0, 0.0),
            radius: 2.0,
            arcs: Vec::new(),
        })
        .collect();
    let mut arcs = Vec::new();
    for i in 0..xs.len() - 1 {
        hubs[i].arcs.push(i as u16);
        hubs[i + 1].arcs.push(i as u16);
        arcs.push(Arc {
            a: i as u16,
            b: i as u16 + 1,
            type_mask: 1,
            flags: 0,
        });
    }
    let mut g = NavGraph::new(
        hubs,
        arcs,
        RouteTable::empty(xs.len()),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    g.hints = vec![snipe_hint("near", 75.0), snipe_hint("far", 55.0)];
    sim.nav = Some(g);
    sim.add_cp(post("cp0", 30.0), Some(0));
    sim.add_cp(post("cp1", 100.0), Some(1));
    let classes = [
        sim.add_weapon_class(sniper_rifle(8.0)),
        sim.add_weapon_class(pistol()),
    ];
    (sim, classes)
}

/// A sniper bot of team 0 at `pos`: the rifle in hand, the pistol in slot 1.
fn sniper(sim: &mut Sim, [rifle, pistol]: [usize; 2], pos: Vec3) -> UnitHandle {
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        weapon_class: rifle,
        slots: std::sync::Arc::new([SlotSpec {
            slot: 1,
            channel: 0,
            weapon_class: pistol,
            clips: 6,
        }]),
        ..UnitSpec::trooper(0, pos)
    })
    .expect("sniper")
}

const NEAR: Vec3 = Vec3::new(75.0, 0.0, 0.0);
const FAR: Vec3 = Vec3::new(55.0, 0.0, 0.0);

fn spot_of(sim: &Sim, h: UnitHandle) -> Option<SnipeState> {
    sim.unit(h).expect("alive").ai.snipe.cur
}

/// Acceptance: a sniper 10 m from the NEAR hint (its lane 25 m) and 30 m
/// from the FAR one (45 m) picks the far one - the score weighs each point
/// of enemy ground by its distance, so two points 45 m off outscore two 25
/// m off - and walks there and holds it.
#[test]
fn a_sniper_picks_the_long_lane_hint_over_a_nearer_short_lane_one() {
    let (mut sim, w) = line_scene();
    let h = sniper(&mut sim, w, Vec3::new(85.0, 0.0, 0.0));
    let u = sim.unit(h).expect("alive").clone();
    assert!(is_sniper(&sim, &u));
    let (near, near_len) = lane(&sim, &u, NEAR);
    let (far, far_len) = lane(&sim, &u, FAR);
    assert!((near_len - 25.0).abs() < 0.01 && (far_len - 45.0).abs() < 0.01);
    assert!(far > near, "{far} over {near}");
    let got = pick(&sim, h, &u, None).expect("a spot");
    assert_eq!(got.spot, FAR, "the long lane");
    // Without it the near one is the pick.
    assert_eq!(pick(&sim, h, &u, Some(FAR)).expect("a spot").spot, NEAR);
    // A plain trooper is no sniper.
    let mut plain = u.clone();
    plain.weapon_class = w[1];
    plain.held.iter_mut().for_each(|x| x.class = w[1]);
    assert!(!is_sniper(&sim, &plain));
    // In the battle: it walks to the far hint and stands on it.
    let mut held = false;
    for _ in 0..30 * TICK_HZ {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        if u.ai.snipe.cur.is_some_and(|c| c.arrived) {
            held = true;
            assert_eq!(u.ai.snipe.cur.expect("spot").spot, FAR);
            assert!(
                nav::xz_dist(u.soldier.pos, FAR) <= ARRIVE,
                "{}",
                u.soldier.pos
            );
            assert!(u.ai.hold.is_some_and(|x| sim.tick < x.until));
            for _ in 0..TICK_HZ {
                sim.step(&[PadState::default()]);
            }
            let b = sim.unit(h).expect("alive").ai.behaviour;
            assert_eq!(b, crate::ai::Behaviour::Overwatch, "it says so");
            assert!(b.stands(), "a reason to stand");
            break;
        }
    }
    assert!(held, "it reached its spot");
}

/// 2026-10-29 (long-range bot sniping): a lane reaches as far as the scope
/// sees at the tier (Normal 96 m, Hard 128 m), not the plain 49 m sight. A
/// third hint at x = 20 looks 80 m down the line onto the enemy post and
/// outscores the FAR hint's 45 m lane; at Easy (no scope sight), or with the
/// suitability distance at 30 m (a lane to 60 m), it has none and FAR is the
/// pick again.
#[test]
fn a_sniper_picks_a_lane_out_to_its_scopes_reach() {
    const LONG: Vec3 = Vec3::new(20.0, 0.0, 0.0);
    let (mut sim, w) = line_scene();
    sim.nav
        .as_mut()
        .expect("graph")
        .hints
        .push(snipe_hint("long", LONG.x));
    let h = sniper(&mut sim, w, Vec3::new(35.0, 0.0, 0.0));
    let u = sim.unit(h).expect("alive").clone();
    let (long, long_len) = lane(&sim, &u, LONG);
    assert!((long_len - 80.0).abs() < 0.01, "lane {long_len}");
    assert!(long > lane(&sim, &u, FAR).0);
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").spot, LONG);
    sim.difficulty = crate::skill::Difficulty::Hard;
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").spot, LONG, "Hard");
    sim.difficulty = crate::skill::Difficulty::Easy;
    assert_eq!(lane(&sim, &u, LONG), (0.0, 0.0), "Easy: 80 m > 49 m");
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").spot, FAR, "Easy");
    sim.difficulty = crate::skill::Difficulty::Hard;
    sim.mission_ai.set_snipe_suitability(30.0);
    assert_eq!(lane(&sim, &u, LONG), (0.0, 0.0), "80 m > 60 m");
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").spot, FAR);
}

/// Card MAIS: the mission's `AISnipeSuitabilityDist` bounds a lane at twice
/// itself. The indoor maps' 30 m (a lane to 60 m) leaves the Normal sight
/// (49 m) and the pick as they were; 20 m (to 40 m) drops the FAR hint's
/// 45 m lane, and the NEAR one is the pick.
#[test]
fn the_snipe_suitability_distance_bounds_the_lane() {
    let (mut sim, w) = line_scene();
    let h = sniper(&mut sim, w, Vec3::new(85.0, 0.0, 0.0));
    let u = sim.unit(h).expect("alive").clone();
    sim.mission_ai.set_snipe_suitability(30.0);
    assert!((lane(&sim, &u, FAR).1 - 45.0).abs() < 0.01);
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").spot, FAR);
    sim.mission_ai.set_snipe_suitability(20.0);
    assert_eq!(lane(&sim, &u, FAR), (0.0, 0.0), "45 m > 40 m");
    assert!((lane(&sim, &u, NEAR).1 - 25.0).abs() < 0.01);
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").spot, NEAR);
}

/// Acceptance: two snipers that spawn together and think on one tick take
/// two spots - the far hint and the near one - not one.
#[test]
fn two_snipers_take_two_spots() {
    let (mut sim, w) = line_scene();
    let a = sniper(&mut sim, w, Vec3::new(85.0, 0.0, 0.0));
    let b = sniper(&mut sim, w, Vec3::new(85.0, 0.0, 2.0));
    for _ in 0..3 * TICK_HZ {
        sim.step(&[PadState::default()]);
    }
    let (sa, sb) = (spot_of(&sim, a), spot_of(&sim, b));
    assert_eq!(sa.map(|c| c.spot), Some(FAR), "the first the far hint");
    assert_eq!(sb.map(|c| c.spot), Some(NEAR), "the second the near one");
}

/// Acceptance: a sniper on its spot with no enemy in its lane stays 20 s
/// and leaves then - the spot is the one left, and its next is another;
/// with an enemy in sight it is still there after 40 s.
#[test]
fn a_sniper_leaves_its_spot_after_twenty_quiet_seconds_and_not_before() {
    for enemy in [false, true] {
        let _stand = crate::ai::tests::all_stand();
        let (mut sim, w) = line_scene();
        // It stands on the far hint when it picks it: it holds at once.
        let h = sniper(&mut sim, w, FAR);
        let e = enemy.then(|| {
            let e = sim
                .spawn(&UnitSpec::trooper(1, Vec3::new(95.0, 0.0, 0.0)))
                .expect("enemy");
            sim.units[e.index as usize].health.max = 1e9;
            sim.units[e.index as usize].health.cur = 1e9;
            e
        });
        let mut since = None;
        for t in 0..45 * TICK_HZ as u64 {
            sim.step(&[PadState::default()]);
            if let Some(e) = e {
                sim.unit_mut(e).expect("enemy").health.cur = 1e9;
            }
            let c = spot_of(&sim, h);
            if since.is_none() && c.is_some_and(|c| c.arrived) {
                since = Some(t);
            }
            let Some(t0) = since else { continue };
            let held = c.is_some_and(|c| c.arrived && c.spot == FAR);
            let secs = (t - t0) as f32 / TICK_HZ as f32;
            if enemy || secs < 19.5 {
                assert!(held, "enemy {enemy}: still on it after {secs} s");
            } else if secs > 21.0 {
                let u = sim.unit(h).expect("alive");
                assert!(!held, "left by {secs} s");
                assert_eq!(u.ai.snipe.left, Some(FAR));
                assert_ne!(c.map(|c| c.spot), Some(FAR));
                break;
            }
        }
        assert!(
            since.is_some_and(|t| t < TICK_HZ as u64),
            "held from the start"
        );
    }
}

/// A Snipe hint 2.5 m over the floor with nothing to climb (BF1 Kashyyyk 2's
/// perch, land SNPR 2026-10-16: the sniper stood under it 120 s and was
/// recycled): the sniper picks it, walks under it and gives it up, the spot
/// the one left - never on it from below.
#[test]
fn a_sniper_under_a_spot_it_cannot_climb_to_gives_it_up() {
    let (mut sim, w) = line_scene();
    let perch = FAR + Vec3::Y * 2.5;
    sim.nav.as_mut().expect("graph").hints[1].pos = perch;
    let h = sniper(&mut sim, w, Vec3::new(85.0, 0.0, 0.0));
    let mut picked = false;
    for _ in 0..40 * TICK_HZ {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        let on = u.ai.snipe.cur.filter(|c| c.spot == perch);
        assert!(!on.is_some_and(|c| c.arrived), "on the perch from below");
        picked |= on.is_some();
        if u.ai.snipe.left == Some(perch) {
            assert!(picked, "it picked the perch");
            assert!(
                nav::xz_dist(u.soldier.pos, perch) <= ARRIVE,
                "given up under it: {}",
                u.soldier.pos
            );
            assert_ne!(u.ai.snipe.cur.map(|c| c.spot), Some(perch));
            return;
        }
    }
    panic!("picked {picked}: still on its way to the perch after 40 s");
}

/// Playtest C4: a sniper on its spot with nothing in sight watches down its
/// lane — the lane's farthest point, the enemy post at x = 100 — not the
/// nearest enemy wherever it is. An enemy that steps behind a wall is not
/// stared at through it: the hold keeps the lane look and the gun stays
/// level.
#[test]
fn a_sniper_on_its_spot_looks_down_its_lane() {
    let _stand = crate::ai::tests::all_stand();
    let (mut sim, w) = line_scene();
    let h = sniper(&mut sim, w, FAR);
    let u = sim.unit(h).expect("alive").clone();
    let lane_far = Vec3::new(100.0, 1.0, 0.0);
    assert_eq!(pick(&sim, h, &u, None).expect("a spot").look, lane_far);
    for _ in 0..TICK_HZ as u64 {
        sim.step(&[PadState::default()]);
    }
    let u = sim.unit(h).expect("alive");
    assert!(spot_of(&sim, h).is_some_and(|c| c.arrived && c.spot == FAR));
    assert_eq!(
        u.ai.hold.expect("holding").look,
        lane_far,
        "no enemy: the lane"
    );
    // An enemy 10 m off behind a wall: unseen, its way blocked, the lane
    // still the look and the gun level.
    sim.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(60.0, 5.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.5, 5.0, 5.0),
        },
    ));
    let e = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(65.0, 0.0, 0.0)))
        .expect("enemy");
    for _ in 0..2 * THINK_PERIOD {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 1e9;
        let u = sim.unit(h).expect("alive");
        assert!(
            u.soldier.pitch.abs() < 0.2,
            "pitched at a wall: {}",
            u.soldier.pitch
        );
    }
    let u = sim.unit(h).expect("alive");
    assert!(
        u.ai.contacts.iter().all(Option::is_none),
        "the wall hides it"
    );
    assert_eq!(u.ai.hold.expect("holding").look, lane_far, "the lane still");
    // The wall gone (body 0, moved under the map): the holder looks at the
    // enemy itself.
    sim.collision
        .move_body(0, Mat4::from_translation(Vec3::new(60.0, -50.0, 0.0)));
    for _ in 0..2 * THINK_PERIOD {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 1e9;
    }
    let u = sim.unit(h).expect("alive");
    let p = sim.unit(e).expect("enemy").soldier.pos + Vec3::Y * 1.4;
    assert_eq!(u.ai.hold.expect("holding").look, p, "in sight: the enemy");
}

/// The weapon in a sniper's hands at each distance of an enemy dummy put
/// there: `steps` of (distance m, seconds), the class of the last shot of
/// each, and the rifle's and the pistol's classes.
fn shots_at(steps: &[(f32, u32)], rifle_min: f32) -> (Vec<Option<u32>>, [u32; 2]) {
    shots_at_tier(steps, rifle_min, crate::skill::Difficulty::Normal)
}

/// [`shots_at`] at tier `d`.
fn shots_at_tier(
    steps: &[(f32, u32)],
    rifle_min: f32,
    d: crate::skill::Difficulty,
) -> (Vec<Option<u32>>, [u32; 2]) {
    let _stand = crate::ai::tests::all_stand();
    let mut sim = Sim::empty(17);
    sim.difficulty = d;
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let w = [
        sim.add_weapon_class(sniper_rifle(rifle_min)),
        sim.add_weapon_class(pistol()),
    ];
    let h = sniper(&mut sim, w, Vec3::ZERO);
    let e = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -steps[0].0)))
        .expect("enemy");
    let mut out = Vec::new();
    for &(d, secs) in steps {
        let mut last = None;
        for _ in 0..secs * TICK_HZ {
            {
                let u = sim.unit_mut(e).expect("enemy");
                u.health.cur = 1e9;
                u.health.max = 1e9;
                u.soldier.pos = Vec3::new(0.0, 0.0, -d);
            }
            sim.step(&[PadState::default()]);
            for ev in &sim.events {
                if let Event::Shot {
                    unit, weapon_class, ..
                } = *ev
                {
                    if unit == h {
                        last = Some(weapon_class);
                    }
                }
            }
        }
        out.push(last);
    }
    (out, w.map(|c| c as u32))
}

/// Acceptance: the rifle at 45 m, the pistol at 15 m (inside a BF1 rifle's
/// MinRange of 20) and at 22 m, the rifle again past the pistol's 24 m reach
/// and at 45 m (the pistol kept once drawn: `arsenal`'s test).
#[test]
fn a_sniper_takes_its_pistol_close_in_and_the_rifle_past_its_reach() {
    let steps = [(45.0, 6), (15.0, 6), (22.0, 6), (30.0, 6), (45.0, 6)];
    let (got, [r, p]) = shots_at(&steps, 20.0);
    assert_eq!(got, vec![Some(r), Some(p), Some(p), Some(r), Some(r)]);
}

/// 2026-10-29 (long-range bot sniping, by tier): a sniper standing still
/// fires its rifle as far as its tier's scope sees - Hard at 80 m and 110 m
/// (inside its 128 m `MaxRange`), not at 140 m; Normal (96 m) at 80 m, not
/// at 110 m; Easy (the plain 49 m) not at 80 m.
#[test]
fn a_still_sniper_fires_as_far_as_its_tiers_scope_sees() {
    use crate::skill::Difficulty;
    let (got, [r, _]) = shots_at_tier(&[(80.0, 8), (110.0, 8), (140.0, 8)], 20.0, Difficulty::Hard);
    assert_eq!(got, vec![Some(r), Some(r), None], "Hard");
    let (got, [r, _]) = shots_at_tier(&[(80.0, 8), (110.0, 8)], 20.0, Difficulty::Normal);
    assert_eq!(got, vec![Some(r), None], "Normal");
    let (got, _) = shots_at_tier(&[(80.0, 8)], 20.0, Difficulty::Easy);
    assert_eq!(got, vec![None], "Easy");
}
