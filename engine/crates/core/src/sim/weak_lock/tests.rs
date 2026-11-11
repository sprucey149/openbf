//! The weak-part lock (`analysis/re-ordnance-kinds.md` §5.1): the lock
//! point is the weak part in sight nearest the reticle and a locked rocket
//! steers to it. Any human's round that strikes a weak part, locked or not,
//! rocket or rifle bolt, deals its vehicle class's factor on a ground map
//! ([`GROUND_WEAK_SPOT_FACTOR`], x4 on a big walker's row) and is the
//! critical hit; a hit beside it is plain. With no weak part in
//! sight, for a bot and in a space round the lock stays on the centre; a
//! bot's strike is plain and a space round keeps the data's factor.

use super::*;
use crate::collision::{Body, HullPart};
use crate::missile::{Missile, MissileTarget};
use crate::projectile::{Bolt, Shooter, Target};
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

/// The weak spot's factor on a ground map, for a class with no row.
const X: f32 = GROUND_WEAK_SPOT_FACTOR;

/// A rifle whose bolt takes `RIFLE` off a vehicle, with no blast.
const RIFLE: f32 = 30.0;

fn rifle() -> WeaponClass {
    let mut w = WeaponClass::bf1_rifle();
    w.ordnance.max_damage = RIFLE;
    w.ordnance.scales = [1.0; 7];
    w.ordnance.expire = None;
    w.ordnance.explosion = None;
    w
}

/// Lay one round of class `wc` from `who`'s fire point straight at `to`.
fn lay(s: &mut Sim, who: UnitHandle, wc: usize, to: Vec3) {
    let u = s.unit(who).unwrap();
    let from = u.fire_point();
    let shooter = Shooter {
        owner: who,
        team: u.team,
        weapon_class: wc,
        is_player: u.is_player,
    };
    let o = &s.weapon_classes[wc].ordnance;
    let b = Bolt::fire(
        from,
        (to - from).normalize(),
        o.velocity,
        o.life_span,
        shooter,
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        crate::collision::HullFilter::All,
    );
    s.bolts.push(b);
}

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

/// One round with no spread.
const STRAIGHT: ShotSpec = ShotSpec {
    tilt: (0.0, 0.0),
    charge: 1.0,
    stage: 0,
    pitch_spread: 0.0,
    yaw_spread: 0.0,
};

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
    let (drop, critical, _) = land_at(s, me, v);
    (drop, critical)
}

/// As [`land`], and where the last round struck `v`.
fn land_at(s: &mut Sim, me: UnitHandle, v: VehicleHandle) -> (f32, bool, Option<Vec3>) {
    let before = s.vehicle(v).unwrap().health.cur;
    let (mut critical, mut at) = (false, None);
    for _ in 0..120 {
        s.step(&[PadState::default()]);
        for e in &s.events {
            match e {
                Event::CriticalHit {
                    shooter, victim, ..
                } if *shooter == me && *victim == Target::Vehicle(v) => critical = true,
                Event::Impact {
                    pos,
                    vehicle: Some(h),
                    ..
                } if *h == v => at = Some(*pos),
                _ => {}
            }
        }
    }
    (before - s.vehicle(v).unwrap().health.cur, critical, at)
}

/// Is `p` on the plate centred at `c` (half 0.35 x 0.25 x 0.1, a little
/// slack for the round's last step)?
fn on_plate(p: Option<Vec3>, c: Vec3) -> bool {
    p.is_some_and(|p| {
        let d = (p - c).abs();
        d.x <= 0.4 && d.y <= 0.3 && d.z <= 0.2
    })
}

/// Aim the player's eye at `p` (yaw and pitch; the eye moves with the aim,
/// so a few passes).
fn aim_at(s: &mut Sim, me: UnitHandle, p: Vec3) {
    let i = me.index as usize;
    for _ in 0..8 {
        let (eye, _) = s.units[i].aim_ray(s.ground_rules, s.views());
        let d = p - eye;
        let u = s.unit_mut(me).unwrap();
        u.soldier.yaw = (-d.x).atan2(-d.z);
        u.soldier.pitch = d.y.atan2(d.x.hypot(d.z));
    }
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

/// A rocket fired on a full lock whose box is on the part strikes it for
/// the weak-spot factor and lights the claws, on both games, whether the
/// part is off to the side or on the line to the centre. Turned to the
/// centre the same rocket strikes plain hull beside the side plate.
#[test]
fn a_locked_rocket_on_the_part_takes_the_weak_spot_factor() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        for x in [1.5, 0.0] {
            let (mut s, me, v) = scene(rules, 0.0, &[spot(x, 4.0)]);
            let m = shoot(&mut s);
            assert_eq!(m.target, Some(MissileTarget::Vehicle(v)), "{rules:?}");
            assert_eq!(m.part, Some(1), "{rules:?}");
            let part = at(&s, v, SPOT + Vec3::X * x);
            let (drop, critical, hit) = land_at(&mut s, me, v);
            assert!(on_plate(hit, part), "{rules:?} {x}: {hit:?} {part}");
            assert!(critical, "{rules:?} {x}: the claws");
            assert_eq!(drop, X * DAMAGE, "{rules:?} {x}");
        }
        // The same shot steering to the centre: the plain face beside the
        // side plate.
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

/// On a ground map a weak part deals its class's factor whatever its data
/// (x2 by default, x4 on a big walker's row); a space round keeps the
/// data's; plain and x0 bodies keep their own.
#[test]
fn the_struck_factor_is_the_classs_on_the_ground() {
    assert_eq!(X, 2.0);
    assert_eq!(HEAVY_WALKER_WEAK_SPOT_FACTOR, 4.0);
    for class in [X, HEAVY_WALKER_WEAK_SPOT_FACTOR] {
        for data in [4.0, 6.0, 1.5] {
            assert_eq!(struck_factor(data, Some(class)), class, "{data}");
            assert_eq!(struck_factor(data, None), data, "{data}");
        }
        for data in [1.0, 0.0] {
            assert_eq!(struck_factor(data, Some(class)), data);
            assert_eq!(struck_factor(data, None), data);
        }
    }
    // A class with no row takes the default.
    assert_eq!(class_factor(&[], 3), X);
    assert_eq!(class_factor(&[4.0], 0), 4.0);
    assert_eq!(class_factor(&[4.0], 1), X);
}

/// A big walker's class (the AT-TE, the AT-AT, the spider walker): a
/// locked rocket, an unlocked one and a rifle bolt on its part deal x4 and
/// are the critical hit; beside the part they are plain; a bot's are plain;
/// on both games.
#[test]
fn a_heavy_walkers_part_takes_x4() {
    use crate::skill::Difficulty;
    const W: f32 = HEAVY_WALKER_WEAK_SPOT_FACTOR;
    let walker = |rules| {
        let (mut s, me, v) = scene(rules, 0.0, &[spot(1.5, 4.0)]);
        s.weak_spot_factor = vec![W];
        assert_eq!(s.ground_weak_spot_factor(0), W);
        (s, me, v)
    };
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        // Locked: the box on the part.
        let (mut s, me, v) = walker(rules);
        assert_eq!(shoot(&mut s).part, Some(1), "{rules:?}");
        let (drop, critical, hit) = land_at(&mut s, me, v);
        assert!(on_plate(hit, at(&s, v, SPOT + Vec3::X * 1.5)), "{hit:?}");
        assert!(critical, "{rules:?}");
        assert_eq!(drop, W * DAMAGE, "{rules:?}: locked");
        // Unlocked, laid straight at the part, and beside it.
        let (mut s, me, v) = walker(rules);
        let wc = s.unit(me).unwrap().weapon_class;
        let rifle = s.add_weapon_class(rifle());
        let part = at(&s, v, SPOT + Vec3::X * 1.5);
        let face = at(&s, v, Vec3::new(0.0, 1.5, 3.0));
        for (class, damage) in [(wc, DAMAGE), (rifle, RIFLE)] {
            lay(&mut s, me, class, part);
            let (drop, critical, hit) = land_at(&mut s, me, v);
            assert!(on_plate(hit, part), "{rules:?}: {hit:?}");
            assert!(critical, "{rules:?}");
            assert_eq!(drop, W * damage, "{rules:?}: on the part");
            lay(&mut s, me, class, face);
            let (drop, critical, hit) = land_at(&mut s, me, v);
            assert!(hit.is_some_and(|h| !on_plate(Some(h), part)), "{hit:?}");
            assert!(!critical, "{rules:?}");
            assert_eq!(drop, damage, "{rules:?}: beside the part");
        }
    }
    // A bot's strike on the walker's part stays plain.
    let (mut s, _, v) = walker(GroundRules::Bf2);
    s.difficulty = Difficulty::Elite;
    let rw = s.add_weapon_class(rifle());
    let bot = s
        .spawn(&UnitSpec {
            weapon_class: rw,
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(1.0, 0.0, 0.0))
        })
        .unwrap();
    let part = at(&s, v, SPOT + Vec3::X * 1.5);
    lay(&mut s, bot, rw, part);
    let before = s.vehicle(v).unwrap().health.cur;
    steps(&mut s, 120, 0);
    assert_eq!(
        before - s.vehicle(v).unwrap().health.cur,
        RIFLE,
        "a bot: plain"
    );
    // A space round keeps the data's x4 whatever the row (here x2).
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 6.0)]);
    s.weak_spot_factor = vec![X];
    s.space_assault = Some(crate::sim::SpaceAssault::new(10));
    let rw = s.add_weapon_class(rifle());
    let part = at(&s, v, SPOT + Vec3::X * 1.5);
    lay(&mut s, me, rw, part);
    let (drop, _, _) = land_at(&mut s, me, v);
    assert_eq!(drop, 6.0 * RIFLE, "space: the data's");
}

/// A rocket fired before the dwell is done, aimed at the part: no lock, a
/// real strike, the same factor and the critical hit as a locked one.
#[test]
fn an_unlocked_rocket_on_the_part_is_critical() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let (mut s, me, v) = scene(rules, 0.0, &[spot(1.5, 4.0)]);
        let part = at(&s, v, SPOT + Vec3::X * 1.5);
        aim_at(&mut s, me, part);
        steps(&mut s, 2, 0);
        aim_at(&mut s, me, part);
        let i = me.index as usize;
        assert!(s.units[i].weapon.lock_held < 1.0, "{rules:?}: no full lock");
        // The trigger's shot with no spread (the spread is not tested).
        let before = s.bolts.len();
        s.soldier_shots(i, None, &[STRAIGHT]);
        assert!(s.bolts.len() > before, "{rules:?}: fired");
        let m = s.bolts.last().unwrap().missile.expect("a missile");
        assert_eq!((m.target, m.part), (None, None), "{rules:?}");
        let (drop, critical, hit) = land_at(&mut s, me, v);
        assert!(on_plate(hit, part), "{rules:?}: {hit:?} {part}");
        assert!(critical, "{rules:?}: the critical hit");
        assert_eq!(drop, X * DAMAGE, "{rules:?}");
    }
}

/// A centre lock whose line runs through a weak plate strikes it: the
/// weak-spot factor, as BF2's locked rocket that met a critical part.
#[test]
fn a_centre_locked_rocket_on_the_part_is_critical() {
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(0.0, 4.0)]);
    shoot(&mut s);
    let m = s.bolts.last_mut().unwrap().missile.as_mut().unwrap();
    assert_eq!(m.target, Some(MissileTarget::Vehicle(v)));
    m.part = None;
    let (drop, critical, hit) = land_at(&mut s, me, v);
    assert!(on_plate(hit, at(&s, v, SPOT)), "{hit:?}");
    assert!(critical);
    assert_eq!(drop, X * DAMAGE);
}

/// Any ordnance: a rifle bolt on the part deals the factor and is the
/// critical hit, a bolt on the hull beside it is plain, on both games; a
/// plate authored x6 deals the same x2 on the ground.
#[test]
fn a_rifle_bolt_on_the_part_takes_the_weak_spot_factor() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        for data in [4.0, 6.0] {
            let (mut s, me, v) = scene(rules, 0.0, &[spot(1.5, data)]);
            let wc = s.add_weapon_class(rifle());
            let part = at(&s, v, SPOT + Vec3::X * 1.5);
            lay(&mut s, me, wc, part);
            let (drop, critical, hit) = land_at(&mut s, me, v);
            assert!(on_plate(hit, part), "{rules:?}: {hit:?} {part}");
            assert!(critical, "{rules:?} x{data}: the critical hit");
            assert_eq!(drop, X * RIFLE, "{rules:?} x{data}");
            // Beside the part: the plain face 1.5 m from it.
            let face = at(&s, v, Vec3::new(0.0, 1.5, 3.0));
            lay(&mut s, me, wc, face);
            let (drop, critical, hit) = land_at(&mut s, me, v);
            assert!(hit.is_some_and(|h| !on_plate(Some(h), part)), "{hit:?}");
            assert!(!critical, "{rules:?}: beside the part");
            assert_eq!(drop, RIFLE, "{rules:?}");
        }
    }
}

/// An unlocked rocket on the hull beside the part is plain.
#[test]
fn a_rocket_beside_the_part_is_plain() {
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    let face = at(&s, v, Vec3::new(0.0, 1.5, 3.0));
    aim_at(&mut s, me, face);
    let before = s.bolts.len();
    s.soldier_shots(me.index as usize, None, &[STRAIGHT]);
    assert!(s.bolts.len() > before);
    let (drop, critical, hit) = land_at(&mut s, me, v);
    assert!(!on_plate(hit, at(&s, v, SPOT + Vec3::X * 1.5)), "{hit:?}");
    assert!(!critical);
    assert_eq!(drop, DAMAGE);
}

/// Bots are unchanged: a bot's rocket or bolt on the part is plain on a
/// ground map at every tier, with no critical hit.
#[test]
fn a_bots_round_on_the_part_is_plain() {
    use crate::skill::Difficulty;
    for difficulty in [Difficulty::Normal, Difficulty::Elite] {
        let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
        s.difficulty = difficulty;
        let wc = s.unit(me).unwrap().weapon_class;
        let rifle = s.add_weapon_class(rifle());
        let bot = s
            .spawn(&UnitSpec {
                weapon_class: wc,
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(1.0, 0.0, 0.0))
            })
            .unwrap();
        let part = at(&s, v, SPOT + Vec3::X * 1.5);
        for (class, damage) in [(wc, DAMAGE), (rifle, RIFLE)] {
            lay(&mut s, bot, class, part);
            let before = s.vehicle(v).unwrap().health.cur;
            let mut critical = false;
            for _ in 0..120 {
                s.step(&[PadState::default()]);
                critical |= s
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::CriticalHit { shooter, .. } if *shooter == bot));
            }
            let drop = before - s.vehicle(v).unwrap().health.cur;
            assert!(!critical, "{difficulty:?}");
            assert_eq!(drop, damage, "{difficulty:?}: a bot's hit is plain");
        }
    }
}

/// A space round keeps the data's factor: a human's bolt on a x4 or x6
/// plate deals x4 or x6.
#[test]
fn a_space_round_keeps_the_datas_factor() {
    for data in [4.0, 6.0] {
        let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, data)]);
        s.space_assault = Some(crate::sim::SpaceAssault::new(10));
        assert!(s.space_round());
        let wc = s.add_weapon_class(rifle());
        let part = at(&s, v, SPOT + Vec3::X * 1.5);
        lay(&mut s, me, wc, part);
        let (drop, critical, hit) = land_at(&mut s, me, v);
        assert!(on_plate(hit, part), "{hit:?}");
        assert!(critical);
        assert_eq!(drop, data * RIFLE, "x{data}");
    }
}

#[test]
fn the_missile_follows_the_part_as_the_vehicle_moves() {
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    // The tank slides 6 m aside while the round is in the air.
    let mut hit = None;
    let mut critical = false;
    for k in 0..120 {
        if k < 30 {
            s.vehicles[v.index as usize].pos.x -= 0.2;
        }
        s.step(&[PadState::default()]);
        for e in &s.events {
            match e {
                Event::CriticalHit { shooter, .. } if *shooter == me => critical = true,
                Event::Impact {
                    pos,
                    vehicle: Some(h),
                    ..
                } if *h == v => hit = Some((*pos, at(&s, v, SPOT + Vec3::X * 1.5))),
                _ => {}
            }
        }
    }
    let (pos, part) = hit.expect("the round met the tank");
    assert!(
        on_plate(Some(pos), part),
        "the round stayed on the part: {pos} {part}"
    );
    assert!(critical);
    let t = s.vehicle(v).unwrap();
    assert_eq!(t.health.max - t.health.cur, X * DAMAGE);
    // The tank turns about under the round: it meets the plain face, plain.
    let (mut s, me, v) = scene(GroundRules::Bf2, 0.0, &[spot(1.5, 4.0)]);
    assert_eq!(shoot(&mut s).part, Some(1));
    s.vehicles[v.index as usize].rot = glam::Quat::from_rotation_y(std::f32::consts::PI);
    let (drop, critical) = land(&mut s, me, v);
    assert!(!critical);
    assert_eq!(drop, DAMAGE);
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

/// The far lock (the 175 m reach): at 150 m the box sits on the weak
/// part, the slower dwell fills and the rocket leaves for the part.
#[test]
fn the_weak_part_locks_at_range() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let (mut s, _, v) = scene(rules, 0.0, &[spot(0.0, 4.0)]);
        s.vehicles[v.index as usize].pos = ahead(150.0, 0.0);
        s.sync_hull(v.index as usize);
        steps(&mut s, 2, 0);
        let part = at(&s, v, SPOT);
        assert!(near(boxed(&s), part), "{rules:?}: {} {part}", boxed(&s));
        assert!(s.foot_lock_weak(0) && s.foot_lock_visible(0), "{rules:?}");
        // 1 2/3 s at 150 m: not full at 40 ticks, full by 55.
        steps(&mut s, 38, 0);
        assert!(s.foot_lock_charge(0).is_some_and(|f| f < 1.0), "{rules:?}");
        steps(&mut s, 15, 0);
        assert_eq!(s.foot_lock_charge(0), Some(1.0), "{rules:?}");
        let before = s.bolts.len();
        let mut m = None;
        for _ in 0..4 {
            s.step(&[pad(buttons::FIRE)]);
            if s.bolts.len() > before {
                m = s.bolts.last().unwrap().missile;
                break;
            }
        }
        let m = m.expect("a missile");
        assert_eq!(
            (m.target, m.part),
            (Some(MissileTarget::Vehicle(v)), Some(1)),
            "{rules:?}"
        );
    }
}

/// A bot's rocket and a space round's never take a part.
#[test]
fn bots_and_space_rounds_keep_the_centre() {
    let spec = STRAIGHT;
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
    // The tag and the body's index.
    assert_eq!(with.len(), without.len() + 7 + 2);
}
