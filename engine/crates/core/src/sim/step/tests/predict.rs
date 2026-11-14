//! Issue #168 (LAN prediction, `analysis/plan/LAN_MULTIPLAYER.md` §6): a
//! cloned sim steps exactly as its original, and `Sim::step_predicted`
//! moves the local player as `Sim::step` does while the bots leave him be.

use super::*;
use crate::collision::{Body, Shape};
use glam::Mat4;

/// A human in slot 0 at the origin, six bots a side fighting each other
/// `bots_at` away, a few crates in the world (so the collision world and its
/// broad phase are real) and two posts.
fn battle(seed: u32, bots_at: f32) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(seed);
    s.heightfield = Some(flat_ground());
    let crates = (0..12)
        .map(|k| {
            let at = Vec3::new(-30.0 + 5.0 * k as f32, 1.0, bots_at - 6.0);
            Body::new(
                Mat4::from_translation(at),
                Shape::Box {
                    half: Vec3::splat(1.0),
                },
            )
        })
        .collect();
    s.collision = CollisionWorld::new(crates, Vec::new());
    s.add_cp(
        crate::sim::tests::sphere_cp("a", Vec3::new(-20.0, 0.0, bots_at), 6.0, true),
        Some(0),
    );
    s.add_cp(
        crate::sim::tests::sphere_cp("b", Vec3::new(20.0, 0.0, bots_at), 6.0, true),
        Some(1),
    );
    let me = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    for k in 0..6 {
        let x = -8.0 + 3.0 * k as f32;
        s.spawn(&ai(
            0,
            Vec3::new(x, 0.0, bots_at - 12.0),
            std::f32::consts::PI,
        ))
        .unwrap();
        s.spawn(&ai(1, Vec3::new(x, 0.0, bots_at + 12.0), 0.0))
            .unwrap();
    }
    (s, me)
}

/// The human's pads: walk and turn, fire in bursts.
fn pad(t: u64) -> PadState {
    PadState {
        ly: if (t / 45).is_multiple_of(2) {
            i16::MAX
        } else {
            -i16::MAX / 2
        },
        rx: if (t / 30).is_multiple_of(2) {
            9000
        } else {
            -9000
        },
        buttons: if t % 20 < 4 { buttons::FIRE } else { 0 },
        ..Default::default()
    }
}

#[test]
fn a_cloned_sim_steps_identically_and_keeps_sharing_its_map() {
    let (mut s, _) = battle(5, -60.0);
    for t in 0..120 {
        s.step(&[pad(t)]);
    }
    assert!(s
        .units
        .iter()
        .any(|u| u.alive && u.controller == Controller::Ai));
    let mut c = s.clone();
    assert!(c.collision.shares_static_with(&s.collision));
    assert_eq!(c.hash(), s.hash());
    let mut shots = 0;
    for t in 120..420 {
        s.step(&[pad(t)]);
        c.step(&[pad(t)]);
        assert_eq!(c.hash(), s.hash(), "tick {}", s.tick);
        shots += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { .. }))
            .count();
    }
    assert!(shots > 20, "the battle fought: {shots} shots");
    // Stepping never writes the map: the copy still shares it.
    assert!(c.collision.shares_static_with(&s.collision));
    // A write is copy-on-write: the original keeps its bodies.
    let before = s.collision.bodies().len();
    c.collision
        .push_body(Body::new(Mat4::IDENTITY, Shape::Sphere { radius: 1.0 }));
    assert!(!c.collision.shares_static_with(&s.collision));
    assert_eq!(s.collision.bodies().len(), before);
}

#[test]
fn a_predicted_step_moves_the_local_player_as_the_confirmed_step_does() {
    // The bots fight 80 m away and the aim assist is off: nothing of
    // theirs reaches the human (a magnet on a bot would turn him by where
    // that bot is, which a prediction only guesses).
    let (mut s, me) = battle(9, -80.0);
    s.assist_armed = false;
    s.assist_intensity = 0.0;
    for t in 0..90 {
        s.step(&[pad(t)]);
    }
    let reference = s.clone();
    let mut predicted = s.clone();
    let mut bots_differ = false;
    for t in 90..150 {
        s.step(&[pad(t)]);
        predicted.step_predicted(&[pad(t)]);
        let (a, b) = (s.unit(me).unwrap(), predicted.unit(me).unwrap());
        assert_eq!(
            (a.soldier.pos, a.soldier.yaw, a.soldier.pitch),
            (b.soldier.pos, b.soldier.yaw, b.soldier.pitch),
            "tick {} health {:?} {:?}",
            s.tick,
            a.health.cur,
            b.health.cur
        );
        bots_differ |= s
            .units
            .iter()
            .zip(&predicted.units)
            .any(|(x, y)| x.ai.next_think != y.ai.next_think);
    }
    // The predicted copy really skipped the thinks.
    assert!(bots_differ);
    // Nothing the prediction did reached the confirmed sim.
    let mut again = reference;
    for t in 90..150 {
        again.step(&[pad(t)]);
    }
    assert_eq!(again.hash(), s.hash());
}

#[test]
fn a_predicted_step_near_the_bots_diverges_harmlessly() {
    // The human walks into the fight: the bots' replayed pads may push or
    // shoot him differently, and the prediction just runs on.
    let (mut s, me) = battle(11, -10.0);
    for t in 0..60 {
        s.step(&[pad(t)]);
    }
    let reference = s.clone();
    let mut predicted = s.clone();
    for t in 60..240 {
        s.step(&[pad(t)]);
        predicted.step_predicted(&[pad(t)]);
    }
    assert_eq!(predicted.tick, s.tick);
    let _ = (predicted.unit(me), predicted.hash());
    let mut again = reference;
    for t in 60..240 {
        again.step(&[pad(t)]);
    }
    assert_eq!(again.hash(), s.hash(), "the confirmed sim is untouched");
}
