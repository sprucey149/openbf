//! Card HYST: the early think's tests.

use super::*;
use crate::ai::think;
use crate::heightfield::Heightfield;
use crate::projectile::{Bolt, Hit};
use crate::unit::{Controller, UnitSpec};
use glam::Vec3;

/// An AI soldier at the origin facing -Z on flat ground (it has thought once,
/// at tick 0), and an enemy at `enemy` (none when `None`).
fn scene(enemy: Option<Vec3>) -> (Sim, UnitHandle, Option<UnitHandle>, AiState) {
    let mut s = Sim::empty(5);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let a = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .expect("bot");
    let e = enemy.map(|p| s.spawn(&UnitSpec::trooper(1, p)).expect("enemy"));
    let mut st = AiState::default();
    think(&s, a, &mut st);
    assert_eq!(st.last_think, 0);
    (s, a, e, st)
}

/// Ticks `from..=to`, thinking each: the first tick that thought.
fn first_think(s: &mut Sim, a: UnitHandle, st: &mut AiState, from: u64, to: u64) -> Option<u64> {
    for t in from..=to {
        s.tick = t;
        let before = st.last_think;
        think(s, a, st);
        if st.last_think != before {
            return Some(t);
        }
    }
    None
}

fn bolt(pos: Vec3, vel: Vec3, team: u8, owner: UnitHandle) -> Bolt {
    Bolt {
        visual_shot: None,
        pos,
        vel,
        remaining: 1.0,
        age: 0.1,
        owner,
        team,
        weapon_class: 0,
        hit: Hit::None,
        end: pos + vel,
        shooter_is_player: false,
        hulls: crate::collision::HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    }
}

/// With nothing happening the bot thinks on its schedule; a hit since its
/// last think brings the next one to the next tick - but not inside GAP.
#[test]
fn a_hit_wakes_the_bot_at_once_but_not_inside_the_gap() {
    let (mut s, a, _, mut st) = scene(Some(Vec3::new(0.0, 0.0, 200.0)));
    let next = st.next_think;
    assert!(
        next > GAP + 2,
        "a phase late enough to see the wake: {next}"
    );
    assert_eq!(first_think(&mut s, a, &mut st, 1, next), Some(next));

    let (mut s, a, e, mut st) = scene(Some(Vec3::new(0.0, 0.0, 200.0)));
    let shooter = e.expect("enemy");
    // Hit at tick 1: no think before the gap is over, one at GAP.
    s.unit_mut(a).expect("bot").last_hit = Some((shooter, 1));
    assert_eq!(first_think(&mut s, a, &mut st, 1, next), Some(GAP));
    // What woke it is reported once, then cleared.
    assert_eq!(st.wake, 0);
    // A hit before that think does not wake it again.
    let next = st.next_think;
    assert_eq!(first_think(&mut s, a, &mut st, GAP + 1, next), Some(next));
}

/// An enemy bolt passing a metre from the bot wakes it; one 5 m off, or a
/// friend's, does not.
#[test]
fn a_near_miss_wakes_the_bot() {
    let (mut s, a, e, _) = scene(Some(Vec3::new(0.0, 0.0, -200.0)));
    let e = e.expect("enemy");
    let u = s.unit(a).expect("bot").clone();
    let past = |x: f32| bolt(Vec3::new(x, 1.0, 3.0), Vec3::new(0.0, 0.0, 300.0), 1, e);
    s.bolts.push(past(1.0));
    assert_eq!(
        events(&s, a, &u, &mut u.ai.clone(), &mut 0) & NEAR,
        NEAR,
        "a metre off"
    );
    s.bolts[0] = past(5.0);
    assert_eq!(
        events(&s, a, &u, &mut u.ai.clone(), &mut 0) & NEAR,
        0,
        "5 m off"
    );
    let mut own = past(0.5);
    own.team = 0;
    s.bolts[0] = own;
    assert_eq!(
        events(&s, a, &u, &mut u.ai.clone(), &mut 0) & NEAR,
        0,
        "a friend's"
    );
}

/// An enemy it holds no record of, in view within SIGHT and in the line of
/// its eye, wakes it; beyond SIGHT, behind it, or already known, not.
#[test]
fn a_new_enemy_in_sight_wakes_the_bot() {
    for (at, wakes) in [
        (Vec3::new(0.0, 0.0, -15.0), true),
        (Vec3::new(0.0, 0.0, -30.0), false),
        (Vec3::new(0.0, 0.0, 15.0), false),
    ] {
        let (mut s, a, _, _) = scene(None);
        s.spawn(&UnitSpec::trooper(1, at)).expect("enemy");
        let u = s.unit(a).expect("bot").clone();
        let st = AiState::default();
        assert_eq!(
            events(&s, a, &u, &mut st.clone(), &mut 0) & SEEN != 0,
            wakes,
            "enemy at {at}"
        );
    }
    let (mut s, a, _, _) = scene(None);
    let e = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -15.0)))
        .expect("enemy");
    let u = s.unit(a).expect("bot").clone();
    let mut st = AiState::default();
    crate::perception::scan(&s, a, &mut st, s.ai_tier(0));
    assert!(
        crate::perception::contact_of(&st, e).is_some(),
        "seen by the scan"
    );
    assert_eq!(
        events(&s, a, &u, &mut st.clone(), &mut 0) & SEEN,
        0,
        "known already"
    );
}

#[test]
fn segments_measure_their_least_distance() {
    let d = seg_seg_dist(
        Vec3::new(-5.0, 1.0, 0.0),
        Vec3::new(5.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(0.0, 2.0, 2.0),
    );
    assert!((d - 2.0).abs() < 1e-5, "{d}");
    let d = seg_seg_dist(
        Vec3::ZERO,
        Vec3::X,
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(4.0, 0.0, 0.0),
    );
    assert!((d - 2.0).abs() < 1e-5, "{d}");
}
