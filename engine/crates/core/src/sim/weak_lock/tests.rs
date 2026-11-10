//! The weak-part lock (`analysis/re-ordnance-kinds.md` §5.1): the lock
//! point is the weak part in sight nearest the reticle, a locked rocket
//! steers to it and its hit there is the critical hit; with no weak part in
//! sight, for a bot and in a space round the lock stays on the centre.

use super::*;
use crate::collision::{Body, HullPart};
use crate::missile::{Missile, MissileTarget};
use crate::projectile::Target;
use crate::sim::foot_lock::tests::{ahead, launcher, pad, scene_rules, steps};
use crate::sim::input::buttons;
use crate::sim::{Event, PadState};
use crate::soldier::GroundRules;
use crate::unit::{Controller, UnitHandle, UnitSpec};
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::{HealthType, ShotSpec, WeaponClass};
use glam::Mat4;

/// The launcher's direct hit, with no blast to blur the numbers.
const DAMAGE: f32 = 120.0;

fn rocket() -> WeaponClass {
    let mut w = launcher();
    w.ordnance.expire = None;
    w
}

/// A human with the launcher at the origin facing -Z, and a hostile tank
/// 40 m ahead with a 4 x 2 x 6 m hull plus `spots`.
fn scene(rules: GroundRules, yaw: f32, spots: &[HullPart]) -> (Sim, UnitHandle, VehicleHandle) {
    let (mut s, me) = scene_rules(rocket(), rules);
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(1),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: ahead(40.0, 0.0),
        yaw,
        max_health: 5000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let mut parts = vec![HullPart::new(
        Mat4::from_translation(Vec3::Y),
        Shape::Box {
            half: Vec3::new(2.0, 1.0, 3.0),
        },
        Layers::ALL,
    )];
    parts.extend_from_slice(spots);
    s.hull_parts = vec![Some(parts.into())];
    s.sync_hull(v.index as usize);
    (s, me, v)
}

/// A small plate standing just proud of the hull's +Z face (the one toward
/// the shooter at yaw 0), `x` m to the side, with factor `mult`.
fn spot(x: f32, mult: f32) -> HullPart {
    HullPart::new(
        Mat4::from_translation(SPOT + Vec3::X * x),
        Shape::Box {
            half: Vec3::new(0.35, 0.25, 0.1),
        },
        Layers::ALL,
    )
    .with_hit_mult(mult)
}

const SPOT: Vec3 = Vec3::new(0.0, 1.5, 3.05);

/// Where the model point `p` of `v` is in the world.
fn at(s: &Sim, v: VehicleHandle, p: Vec3) -> Vec3 {
    let v = s.vehicle(v).unwrap();
    v.pos + v.rot * p
}

fn near(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-3
}

/// Turn the player to face `p` (yaw only).
fn face(s: &mut Sim, me: UnitHandle, p: Vec3) {
    let u = s.unit_mut(me).unwrap();
    let d = p - u.soldier.pos;
    u.soldier.yaw = (-d.x).atan2(-d.z);
}

/// The lock box's point.
fn boxed(s: &Sim) -> Vec3 {
    s.foot_lock_target(0).expect("a lock").1
}

/// Dwell to a full lock and fire: the missile as it left.
fn shoot(s: &mut Sim) -> Missile {
    steps(s, 40, 0);
    let before = s.bolts.len();
    for _ in 0..4 {
        s.step(&[pad(buttons::FIRE)]);
        if s.bolts.len() > before {
            return s.bolts.last().unwrap().missile.expect("a missile");
        }
    }
    panic!("the launcher did not fire");
}

/// Follow the rounds in flight: the vehicle's health drop and whether a
/// critical hit on it was raised.
fn land(s: &mut Sim, me: UnitHandle, v: VehicleHandle) -> (f32, bool) {
    let before = s.vehicle(v).unwrap().health.cur;
    let mut critical = false;
    for _ in 0..120 {
        s.step(&[PadState::default()]);
        critical |= s.events.iter().any(|e| {
            matches!(e, Event::CriticalHit { shooter, victim, .. }
                if *shooter == me && *victim == Target::Vehicle(v))
        });
    }
    (before - s.vehicle(v).unwrap().health.cur, critical)
}

#[test]
fn the_lock_point_is_the_weak_part_facing_the_shooter() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let (mut s, _, v) = scene(rules, 0.0, &[spot(0.0, 4.0)]);
        steps(&mut s, 2, 0);
        let part = at(&s, v, SPOT);
        assert!(near(boxed(&s), part), "{rules:?}: {} {part}", boxed(&s));
        assert!(s.foot_lock_weak(0) && s.foot_lock_visible(0), "{rules:?}");
        // Turned about, the part is behind the hull: the centre.
        let (mut s, _, v) = scene(rules, std::f32::consts::PI, &[spot(0.0, 4.0)]);
        steps(&mut s, 2, 0);
        let centre = at(&s, v, Vec3::Y * 1.5);
        assert!(near(boxed(&s), centre), "{rules:?}: {} {centre}", boxed(&s));
        assert!(!s.foot_lock_weak(0) && s.foot_lock_visible(0), "{rules:?}");
    }
}

#[test]
fn the_part_nearest_the_reticle_wins() {
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(-1.5, 4.0), spot(1.5, 4.0)]);
    steps(&mut s, 2, 0);
    for x in [-1.5f32, 1.5] {
        let part = at(&s, v, SPOT + Vec3::X * x);
        face(&mut s, me, part);
        assert!(near(boxed(&s), part), "{x}: {} {part}", boxed(&s));
        let i = me.index as usize;
        let (eye, dir) = s.units[i].aim_ray(s.ground_rules, s.views());
        let k = if x < 0.0 { 1 } else { 2 };
        assert_eq!(s.weak_part(v, eye, dir).map(|p| p.0), Some(k));
        assert!(near(s.weak_part_point(v, k).unwrap(), part));
    }
    assert_eq!(s.weak_part_point(v, 0), None, "the plain hull is no part");
    assert_eq!(s.weak_part_point(v, 3), None);
}

#[test]
fn no_weak_part_keeps_the_hull_centre() {
    // No part over 1: a plain plate, and one that takes no damage.
    for spots in [vec![], vec![spot(0.0, 1.0)], vec![spot(0.0, 0.0)]] {
        let (mut s, _, v) = scene(GroundRules::Bf2, 0.0, &spots);
        steps(&mut s, 2, 0);
        assert!(near(boxed(&s), at(&s, v, Vec3::Y * 1.5)), "{}", boxed(&s));
        assert!(!s.foot_lock_weak(0));
        assert_eq!(shoot(&mut s).part, None);
    }
    // No hull at all.
    let (mut s, _, v) = scene(GroundRules::Bf2, 0.0, &[]);
    s.hull_parts.clear();
    s.collision.remove_hull(v);
    steps(&mut s, 2, 0);
    assert!(near(boxed(&s), at(&s, v, Vec3::Y * 1.5)));
    // A weak plate out of the ordnance subset cannot be struck: no part.
    let mut soft = spot(0.0, 4.0);
    soft.layers = Layers::SOLDIER;
    let (mut s, _, v) = scene(GroundRules::Bf2, 0.0, &[soft]);
    steps(&mut s, 2, 0);
    assert!(near(boxed(&s), at(&s, v, Vec3::Y * 1.5)));
}

#[test]
fn something_before_the_part_keeps_the_centre() {
    // A post in the line to the part; the hull's centre stays in sight.
    let (mut s, _, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    steps(&mut s, 2, 0);
    let part = at(&s, v, SPOT + Vec3::X * 1.5);
    assert!(near(boxed(&s), part));
    let mid = part * 0.9;
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(mid.x + 0.3, 2.0, mid.z)),
        Shape::Box {
            half: Vec3::new(0.5, 4.0, 0.2),
        },
    ));
    steps(&mut s, 2, 0);
    assert!(s.foot_lock[0].target.is_some(), "the lock holds");
    assert!(near(boxed(&s), at(&s, v, Vec3::Y * 1.5)), "{}", boxed(&s));
    // A plain plate of the same hull standing before the part hides it too.
    let cover = HullPart::new(
        Mat4::from_translation(SPOT + Vec3::Z * 0.4),
        Shape::Box {
            half: Vec3::new(1.0, 1.0, 0.1),
        },
        Layers::ALL,
    );
    let (mut s, _, v) = scene(GroundRules::Bf2, 0.0, &[spot(0.0, 4.0), cover]);
    steps(&mut s, 2, 0);
    assert!(near(boxed(&s), at(&s, v, Vec3::Y * 1.5)), "{}", boxed(&s));
}

#[test]
fn a_locked_rocket_homes_on_the_part_and_strikes_critical() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        // The plate is off to the side: a rocket to the centre misses it.
        let (mut s, me, v) = scene(rules, 0.0, &[spot(1.5, 4.0)]);
        let m = shoot(&mut s);
        assert_eq!(m.target, Some(MissileTarget::Vehicle(v)), "{rules:?}");
        assert_eq!(m.part, Some(1), "{rules:?}");
        let (drop, critical) = land(&mut s, me, v);
        assert!(critical, "{rules:?}: the critical hit");
        assert_eq!(drop, 4.0 * DAMAGE, "{rules:?}");
        // The same shot steering to the centre: a plain hit.
        let (mut s, me, v) = scene(rules, 0.0, &[spot(1.5, 4.0)]);
        shoot(&mut s);
        s.bolts.last_mut().unwrap().missile.as_mut().unwrap().part = None;
        let (drop, critical) = land(&mut s, me, v);
        assert!(!critical, "{rules:?}: the centre is plain hull");
        assert_eq!(drop, DAMAGE, "{rules:?}");
        // With the part hidden the lock and the rocket keep the centre.
        let (mut s, me, v) = scene(rules, std::f32::consts::PI, &[spot(1.5, 4.0)]);
        assert_eq!(shoot(&mut s).part, None, "{rules:?}");
        let (drop, critical) = land(&mut s, me, v);
        assert!(!critical, "{rules:?}");
        assert_eq!(drop, DAMAGE, "{rules:?}");
    }
}

#[test]
fn the_missile_follows_the_part_as_the_vehicle_moves() {
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    // The tank slides 6 m aside while the round is in the air.
    let mut critical = false;
    let before = s.vehicle(v).unwrap().health.cur;
    for k in 0..120 {
        if k < 30 {
            s.vehicles[v.index as usize].pos.x -= 0.2;
        }
        s.step(&[PadState::default()]);
        critical |= s
            .events
            .iter()
            .any(|e| matches!(e, Event::CriticalHit { shooter, .. } if *shooter == me));
    }
    assert!(critical, "the round stayed on the part");
    assert_eq!(before - s.vehicle(v).unwrap().health.cur, 4.0 * DAMAGE);
    // The tank turns about under the round: it meets the plain face, and
    // still counts as a hit on the part it was sent at.
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    s.vehicles[v.index as usize].rot = glam::Quat::from_rotation_y(std::f32::consts::PI);
    let (drop, critical) = land(&mut s, me, v);
    assert!(critical, "the part's factor on the plain face");
    assert_eq!(drop, 4.0 * DAMAGE);
    // The same round into another vehicle standing in its way is plain.
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    let other = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(1),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: ahead(25.0, 0.0),
        yaw: 0.0,
        max_health: 5000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let before = s.vehicle(other).unwrap().health.cur;
    let (drop, critical) = land(&mut s, me, v);
    assert!(!critical && drop == 0.0);
    assert_eq!(before - s.vehicle(other).unwrap().health.cur, DAMAGE);
    // A part the hull no longer has: the round goes on to the centre.
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    let plain: Vec<HullPart> = s.hull_parts[0].as_ref().unwrap()[..1].to_vec();
    s.hull_parts = vec![Some(plain.into())];
    let (drop, critical) = land(&mut s, me, v);
    assert!(!critical);
    assert_eq!(drop, DAMAGE);
}

#[test]
fn a_part_change_does_not_restart_the_dwell() {
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(-1.5, 4.0), spot(1.5, 4.0)]);
    let (left, right) = (
        at(&s, v, SPOT - Vec3::X * 1.5),
        at(&s, v, SPOT + Vec3::X * 1.5),
    );
    face(&mut s, me, left);
    steps(&mut s, 12, 0);
    assert!(near(boxed(&s), left));
    let held = s.unit(me).unwrap().weapon.lock_held;
    assert!(held > 0.0);
    face(&mut s, me, right);
    steps(&mut s, 12, 0);
    assert!(near(boxed(&s), right));
    let now = s.unit(me).unwrap().weapon.lock_held;
    assert!(now > held + 0.15, "{held} -> {now}");
}

/// On a long hull the weak part can sit outside the centre's 25.8 degree
/// cone: aiming at the part keeps the dwell clock running, and the hold.
#[test]
fn aiming_at_the_part_counts_as_aiming_at_the_vehicle() {
    let dwell = |mult: f32| {
        let (mut s, me) = scene_rules(rocket(), GroundRules::Bf2);
        let v = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(1),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: ahead(13.0, 0.0),
            yaw: 0.0,
            max_health: 5000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let far = Vec3::new(9.0, 1.5, 3.05);
        s.hull_parts = vec![Some(
            vec![
                HullPart::new(
                    Mat4::from_translation(Vec3::Y),
                    Shape::Box {
                        half: Vec3::new(12.0, 1.0, 3.0),
                    },
                    Layers::ALL,
                ),
                HullPart::new(
                    Mat4::from_translation(far),
                    Shape::Box {
                        half: Vec3::new(0.35, 0.25, 0.1),
                    },
                    Layers::ALL,
                )
                .with_hit_mult(mult),
            ]
            .into(),
        )];
        s.sync_hull(v.index as usize);
        // Lock on the centre, then swing to the far end of the hull.
        steps(&mut s, 1, 0);
        assert!(s.foot_lock[0].target.is_some());
        let part = at(&s, v, far);
        face(&mut s, me, part);
        steps(&mut s, 1, 0);
        s.unit_mut(me).unwrap().weapon.lock_held = 0.0;
        steps(&mut s, 20, 0);
        let seen = (s.unit(me).unwrap().weapon.lock_held, boxed(&s), part);
        // A wall then hides the centre and leaves the far end in sight:
        // the hold keeps the vehicle past its 2 s only by the part.
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(-0.5, 2.0, -6.0)),
            Shape::Box {
                half: Vec3::new(3.5, 6.0, 0.2),
            },
        ));
        steps(&mut s, 150, 0);
        (seen, s.foot_lock[0].target.is_some())
    };
    let ((held, point, part), kept) = dwell(4.0);
    assert!(held > 0.3, "{held}");
    assert!(near(point, part));
    assert!(kept, "the part in sight holds the lock");
    let ((held, point, part), kept) = dwell(1.0);
    assert_eq!(held, 0.0, "a plain hull: the centre is out of the cone");
    assert!(!near(point, part));
    assert!(!kept, "a hidden centre is dropped");
}

/// A bot's rocket and a space round's never take a part.
#[test]
fn bots_and_space_rounds_keep_the_centre() {
    let spec = ShotSpec {
        tilt: (0.0, 0.0),
        charge: 1.0,
        stage: 0,
        pitch_spread: 0.0,
        yaw_spread: 0.0,
    };
    let lay = |s: &mut Sim, i: usize, v: VehicleHandle| {
        let w = &mut s.units[i].weapon;
        w.lock_target = Some(MissileTarget::Vehicle(v));
        w.lock_held = 5.0;
        s.soldier_shots(i, None, &[spec]);
        s.bolts.last().unwrap().missile.unwrap()
    };
    // The human's round, laid the same way, takes the part.
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(0.0, 4.0)]);
    let m = lay(&mut s, me.index as usize, v);
    assert_eq!(
        (m.target, m.part),
        (Some(MissileTarget::Vehicle(v)), Some(1))
    );
    // A bot beside him with the same launcher and the same lock.
    let wc = s.unit(me).unwrap().weapon_class;
    let bot = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(1.0, 0.0, 0.0))
        })
        .unwrap();
    let m = lay(&mut s, bot.index as usize, v);
    assert_eq!((m.target, m.part), (Some(MissileTarget::Vehicle(v)), None));
    // A space round: the magnet's lock, the centre.
    let (mut s, _, v) = scene(GroundRules::Bf2, 0.0, &[spot(0.0, 4.0)]);
    s.space_assault = Some(crate::sim::SpaceAssault::new(10));
    assert_eq!(s.foot_lock_target(0), None);
    assert!(!s.foot_lock_weak(0));
    let m = shoot(&mut s);
    assert_eq!((m.target, m.part), (Some(MissileTarget::Vehicle(v)), None));
}

/// Only a round steering to a part adds to the hash stream.
#[test]
fn a_plain_lock_hashes_as_before() {
    let (mut s, _, _) = scene(GroundRules::Bf2, 0.0, &[spot(0.0, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    let stream = |b: &crate::projectile::Bolt| {
        let mut out = Vec::new();
        b.hash_into(&mut |x: &[u8]| out.extend_from_slice(x));
        out
    };
    let has = |bytes: &[u8]| bytes.windows(7).any(|w| w == b"MSLPART");
    let mut b = s.bolts.last().unwrap().clone();
    let with = stream(&b);
    assert!(has(&with));
    b.missile.as_mut().unwrap().part = None;
    let without = stream(&b);
    assert!(!has(&without));
    assert_eq!(with.len(), without.len() + 9);
}
