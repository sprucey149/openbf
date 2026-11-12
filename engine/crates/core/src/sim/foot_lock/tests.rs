//! The on-foot launcher lock (`analysis/re-ordnance-kinds.md` §5), BF2's
//! rule on every ground map: the lock comes on its own (OURS) or by the
//! button, the hold keeps it, the dwell runs only inside 25.8° and in sight;
//! space rounds keep the magnet dwell.

use super::*;
use crate::collision::{Body, Shape};
use crate::sim::tests::flat_ground;
use crate::sim::PadState;
use crate::soldier::GroundRules;
use crate::unit::UnitSpec;
use crate::vehicle::{BlastSetup, SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use crate::weapon::{ExplosionClass, HealthType, OrdKind, WeaponClass};
use glam::Mat4;

const VEHICLE: u32 = 1 << HealthType::Vehicle as u32;
const PERSON_DROID_ANIMAL: u32 = (1 << HealthType::Person as u32)
    | (1 << HealthType::Droid as u32)
    | (1 << HealthType::Animal as u32);

/// A vehicle launcher: 40 m/s missile, a 1 s lock, the BF2 lock defaults
/// (100 / 110 m).
pub(in crate::sim) fn launcher() -> WeaponClass {
    let mut w = WeaponClass::bf1_rifle();
    let o = &mut w.ordnance;
    o.kind = OrdKind::Missile;
    o.velocity = 40.0;
    o.life_span = 4.0;
    o.max_damage = 120.0;
    o.scales = [1.0; 7];
    o.expire = Some(ExplosionClass {
        damage: 120.0,
        damage_inner: 2.0,
        damage_outer: 6.0,
        ..Default::default()
    });
    o.missile = Some(crate::missile::MissileClass {
        turn_rate: 2.0,
        ..crate::missile::MissileClass::default()
    });
    w.rounds_per_clip = 8;
    w.lock_time = 1.0;
    w.target_kind = VEHICLE;
    w
}

pub(in crate::sim) fn tank(s: &mut Sim, team: Option<u8>, pos: Vec3) -> VehicleHandle {
    s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: team,
        neutral_when_empty: team.is_none(),
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    })
}

/// The Bulldog as baked from BF2 (`com_weap_inf_bulldog`): people, droids
/// and animals, a 0.4 s lock, six rounds a clip, a 3 s reload.
fn bulldog() -> WeaponClass {
    let mut w = launcher();
    w.rounds_per_clip = 6;
    w.reload_time = 3.0;
    w.lock_time = 0.4;
    w.target_kind = PERSON_DROID_ANIMAL;
    w
}

/// BF1's torpedo launcher (`*_weap_inf_torpedo_launcher`): vehicles only,
/// the disc's 1.5 s lock (the shared numbers bake 1.0; a time unlike the
/// rocket's shows each launcher counts its own), two rounds a clip (ALL,
/// REP), a 4 s reload.
pub(in crate::sim) fn torpedo() -> WeaponClass {
    let mut w = launcher();
    w.rounds_per_clip = 2;
    w.reload_time = 4.0;
    w.lock_time = 1.5;
    w.target_kind = VEHICLE;
    w
}

/// A player at the origin facing −Z with the launcher, on a BF2 ground map.
pub(in crate::sim) fn scene() -> (Sim, UnitHandle) {
    scene_with(launcher())
}

fn scene_with(class: WeaponClass) -> (Sim, UnitHandle) {
    let (s, me) = scene_rules(class, GroundRules::Bf2);
    assert_eq!(s.ground_rules, GroundRules::Bf2);
    (s, me)
}

pub(in crate::sim) fn scene_rules(class: WeaponClass, rules: GroundRules) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(3);
    s.ground_rules = rules;
    s.heightfield = Some(flat_ground());
    s.blast_setup = vec![Some(BlastSetup {
        explosion: None,
        sphere_centre: Vec3::Y * 1.5,
        sphere_radius: 2.0,
    })];
    let wc = s.add_weapon_class(class);
    let me = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    s.units[me.index as usize].soldier.pitch = 0.0;
    (s, me)
}

/// A point `range` m out at `deg` degrees right of −Z.
pub(in crate::sim) fn ahead(range: f32, deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(a.sin() * range, 0.0, -a.cos() * range)
}

pub(in crate::sim) fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

pub(in crate::sim) fn steps(s: &mut Sim, n: usize, buttons: u32) {
    for _ in 0..n {
        s.step(&[pad(buttons)]);
    }
}

/// One press and release of the lock button.
fn tap(s: &mut Sim) {
    steps(s, 1, buttons::RELOAD);
    steps(s, 1, 0);
}

fn fire(s: &mut Sim) -> Option<MissileTarget> {
    let before = s.bolts.len();
    for _ in 0..4 {
        s.step(&[pad(buttons::FIRE)]);
        if s.bolts.len() > before {
            return s.bolts.last().unwrap().missile.expect("a missile").target;
        }
    }
    panic!("the launcher did not fire");
}

pub(in crate::sim) fn held(s: &Sim, me: UnitHandle) -> f32 {
    s.unit(me).unwrap().weapon.lock_held
}

/// The playtest's rule (B7): dwelling alone locks, fills LockTime and the
/// shot homes; a press on the same lone target keeps it. BF1 maps too.
#[test]
fn launcher_locks_by_dwelling_alone() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        launcher_locks_by_dwelling_alone_on(rules);
    }
}

#[allow(clippy::assertions_on_constants)] // the flag must be on for this rule's test
fn launcher_locks_by_dwelling_alone_on(rules: GroundRules) {
    const { assert!(FOOT_AUTO_LOCK) };
    let (mut s, me) = scene_rules(launcher(), rules);
    let v = tank(&mut s, Some(1), ahead(60.0, 3.0));
    steps(&mut s, 1, 0);
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(v)))
    );
    tap(&mut s);
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(v)))
    );
    assert_eq!(
        s.foot_lock_target(0).map(|t| t.0),
        Some(LockTarget::Aim(AimTarget::Vehicle(v)))
    );
    steps(&mut s, 30, 0);
    let class = s.weapon_classes[s.unit(me).unwrap().weapon_class].clone();
    assert!(
        s.unit(me).unwrap().weapon.locked(&class),
        "{rules:?} {}",
        held(&s, me)
    );
    assert_eq!(s.foot_lock_charge(0), Some(1.0));
    assert_eq!(fire(&mut s), Some(MissileTarget::Vehicle(v)));
    // The shot spends the dwell, not the lock.
    assert_eq!(s.unit(me).unwrap().weapon.lock_held, 0.0);
    assert!(s.foot_lock[0].target.is_some());
}

/// The pick: the 175 m reach, dot >= 0.8, sight, hostile only.
#[test]
fn bf2_pick_needs_dot_0_8_target_lock_max_and_sight() {
    let refused = |place: &dyn Fn(&mut Sim)| {
        let (mut s, _) = scene();
        place(&mut s);
        tap(&mut s);
        s.foot_lock[0].target
    };
    assert_eq!(
        refused(&|s| {
            tank(s, Some(1), ahead(180.0, 0.0));
        }),
        None,
        "180 m is past the 175 m reach"
    );
    assert_eq!(
        refused(&|s| {
            tank(s, Some(1), ahead(50.0, 40.0));
        }),
        None,
        "40 deg is outside dot 0.8"
    );
    assert_eq!(
        refused(&|s| {
            tank(s, None, ahead(50.0, 0.0));
        }),
        None,
        "an empty neutral vehicle"
    );
    assert_eq!(
        refused(&|s| {
            tank(s, Some(0), ahead(50.0, 0.0));
        }),
        None,
        "a friend"
    );
    assert_eq!(
        refused(&|s| {
            tank(s, Some(1), ahead(50.0, 0.0));
            s.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 2.0, -25.0)),
                Shape::Box {
                    half: Vec3::new(6.0, 6.0, 0.5),
                },
            ));
        }),
        None,
        "behind a wall"
    );
    // A soldier is no vehicle: the launcher's kind refuses him.
    assert_eq!(
        refused(&|s| {
            s.spawn(&UnitSpec::trooper(1, ahead(30.0, 0.0))).unwrap();
        }),
        None
    );
    // The controls: 95 m at 30 deg and 170 m ahead lock.
    assert!(refused(&|s| {
        tank(s, Some(1), ahead(95.0, 30.0));
    })
    .is_some());
    assert!(refused(&|s| {
        tank(s, Some(1), ahead(170.0, 0.0));
    })
    .is_some());
}

/// The dwell candidate is re-checked every tick: aim 30 deg off (outside
/// 25.8) and the clock and the candidate reset, the lock itself stays.
#[test]
fn bf2_dwell_resets_outside_25_8_degrees_or_out_of_sight() {
    let (mut s, me) = scene();
    let v = tank(&mut s, Some(1), ahead(60.0, 0.0));
    tap(&mut s);
    steps(&mut s, 20, 0);
    assert!(held(&s, me) > 0.5);
    let yaw = s.units[me.index as usize].soldier.yaw;
    s.units[me.index as usize].soldier.yaw = yaw + 30f32.to_radians();
    steps(&mut s, 1, 0);
    assert_eq!(held(&s, me), 0.0);
    assert_eq!(s.unit(me).unwrap().weapon.lock_target, None);
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(v)))
    );
    assert_eq!(fire(&mut s), None);
    // Back on it, then a wall: the dwell stops again.
    s.units[me.index as usize].soldier.yaw = yaw;
    steps(&mut s, 10, 0);
    assert!(held(&s, me) > 0.0);
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 2.0, -25.0)),
        Shape::Box {
            half: Vec3::new(6.0, 6.0, 0.5),
        },
    ));
    steps(&mut s, 1, 0);
    assert_eq!(held(&s, me), 0.0);
}

/// The hold: 2 s in a row outside dot 0.1 drops the lock; a tick back
/// inside restarts the count. BF1 maps too.
#[test]
fn locked_target_drops_after_two_seconds_away() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        locked_target_drops_after_two_seconds_away_on(rules);
    }
}

fn locked_target_drops_after_two_seconds_away_on(rules: GroundRules) {
    let (mut s, me) = scene_rules(launcher(), rules);
    tank(&mut s, Some(1), ahead(60.0, 0.0));
    tap(&mut s);
    assert!(s.foot_lock[0].target.is_some());
    let i = me.index as usize;
    let yaw = s.units[i].soldier.yaw;
    // Turn to face away (dot -1).
    s.units[i].soldier.yaw = yaw + std::f32::consts::PI;
    steps(&mut s, 50, 0);
    assert!(s.foot_lock[0].target.is_some());
    s.units[i].soldier.yaw = yaw;
    steps(&mut s, 1, 0);
    assert_eq!(s.foot_lock[0].lose, 0.0);
    // Sixty f32 ticks of 1/30 s sum to just under 2.0, so the 61st drops it.
    s.units[i].soldier.yaw = yaw + std::f32::consts::PI;
    steps(&mut s, 60, 0);
    assert!(s.foot_lock[0].target.is_some(), "{}", s.foot_lock[0].lose);
    steps(&mut s, 1, 0);
    assert_eq!(s.foot_lock[0].target, None);
}

/// The Bulldog (six rounds): a press on a partly fired clip reloads even
/// with a target in front, and a press on a full clip cycles the lock
/// without reloading. Holding the button clears a pressed lock; letting go
/// hands the target back to the auto lock. BF1 maps too.
#[test]
fn the_bulldog_reloads_a_partly_fired_clip_and_locks_on_a_full_one() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        bulldog_on(rules);
    }
}

fn bulldog_on(rules: GroundRules) {
    let (mut s, me) = scene_rules(bulldog(), rules);
    let a = s.spawn(&UnitSpec::trooper(1, ahead(30.0, 0.0))).unwrap();
    let b = s.spawn(&UnitSpec::trooper(1, ahead(40.0, 15.0))).unwrap();
    let (la, lb) = (
        LockTarget::Aim(AimTarget::Unit(a)),
        LockTarget::Aim(AimTarget::Unit(b)),
    );
    steps(&mut s, 1, 0);
    assert_eq!(s.foot_lock[0].target, Some(la), "the centred one");
    // Full clip: the press moves to the other, and holds it though it sits
    // outside the dwell cone.
    tap(&mut s);
    assert_eq!(s.foot_lock[0].target, Some(lb));
    assert!(!s.unit(me).unwrap().weapon.reloading());
    steps(&mut s, 10, 0);
    assert_eq!(s.foot_lock[0].target, Some(lb));
    // Held 0.5 s: cleared; released: the auto lock takes the centred one.
    steps(&mut s, 16, buttons::RELOAD);
    assert_eq!(s.foot_lock[0].target, None);
    assert!(!s.unit(me).unwrap().weapon.reloading());
    steps(&mut s, 1, 0);
    assert_eq!(s.foot_lock[0].target, Some(la));

    fire(&mut s);
    steps(&mut s, 30, 0);
    let w = &s.unit(me).unwrap().weapon;
    assert!(w.clip < 6 && !w.reloading());
    steps(&mut s, 1, buttons::RELOAD);
    assert!(s.unit(me).unwrap().weapon.reloading(), "a press reloads");
    assert_eq!(s.foot_lock[0].target, Some(la), "the lock stays");
}

/// #103's crossing target: a soldier walking across the Bulldog's aim is
/// the dwell candidate only inside dot 0.9; once he is past it the clock
/// resets and the shot leaves unguided (the magnet kept him and the
/// rocket missed by 3.279 m).
#[test]
fn a_soldier_crossing_the_bulldog_aim_resets_the_dwell() {
    let (mut s, me) = scene_with(bulldog());
    let a = s.spawn(&UnitSpec::trooper(1, ahead(30.0, -10.0))).unwrap();
    let ai = a.index as usize;
    // Across the aim at 5 deg a tick, from 10 deg left to 40 deg right.
    let mut deg = -10.0;
    let mut dwelt = false;
    while deg < 40.0 {
        s.units[ai].soldier.pos = ahead(30.0, deg);
        steps(&mut s, 1, 0);
        let inside = deg.abs() < 25.0;
        if inside {
            dwelt |= held(&s, me) > 0.0;
        } else if deg > 26.0 {
            assert_eq!(held(&s, me), 0.0, "{deg} deg");
            assert_eq!(s.unit(me).unwrap().weapon.lock_target, None);
        }
        deg += 5.0;
    }
    assert!(dwelt, "he was the candidate while inside 25.8 deg");
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Unit(a))),
        "the lock itself holds out to dot 0.1"
    );
    assert_eq!(fire(&mut s), None, "unguided");
}

/// The auto lock follows the aim: a held target that leaves the dwell cone
/// gives way to the best target in the pick cone.
#[test]
fn the_auto_lock_moves_to_what_the_aim_dwells_on() {
    let (mut s, me) = scene();
    let a = tank(&mut s, Some(1), ahead(60.0, 0.0));
    let b = tank(&mut s, Some(1), ahead(60.0, 50.0));
    steps(&mut s, 1, 0);
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(a)))
    );
    // Yaw turns left, so face 50 deg right with -50.
    s.units[me.index as usize].soldier.yaw -= 50f32.to_radians();
    steps(&mut s, 1, 0);
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(b)))
    );
    steps(&mut s, 40, 0);
    assert_eq!(fire(&mut s), Some(MissileTarget::Vehicle(b)));
}

/// Space rounds keep the magnet's dwell exactly: no press, the candidate is
/// the magnet, the foot lock stays empty and the box shows nothing.
#[test]
fn space_rounds_keep_the_magnet_dwell() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let (mut s, me) = scene_rules(launcher(), rules);
        s.space_assault = Some(crate::sim::SpaceAssault::new(10));
        let v = tank(&mut s, Some(1), ahead(60.0, 0.0));
        steps(&mut s, 2, 0);
        assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)), "{rules:?}");
        assert_eq!(
            s.unit(me).unwrap().weapon.lock_target,
            Some(MissileTarget::Vehicle(v))
        );
        steps(&mut s, 30, 0);
        let class = s.weapon_classes[s.unit(me).unwrap().weapon_class].clone();
        assert!(s.unit(me).unwrap().weapon.locked(&class), "{rules:?}");
        assert!(s.foot_lock.iter().all(|f| *f == FootLock::default()));
        assert_eq!(s.foot_lock_target(0), None);
        assert_eq!(s.foot_lock_charge(0), None);
        // The button still reloads there.
        fire(&mut s);
        steps(&mut s, 30, 0);
        steps(&mut s, 1, buttons::RELOAD);
        assert!(s.unit(me).unwrap().weapon.reloading(), "{rules:?}");
    }
}

/// BF1's torpedo launcher on a BF1 map takes the foot lock with its own
/// 1.5 s LockTime: the tank is held from the first tick, the box shows it,
/// the dwell fills at 1.5 s and the shot homes.
#[test]
fn bf1_torpedo_locks_by_dwelling_and_homes() {
    let (mut s, me) = scene_rules(torpedo(), GroundRules::Bf1);
    let class = s.weapon_classes[s.unit(me).unwrap().weapon_class].clone();
    let v = tank(&mut s, Some(1), ahead(60.0, 0.0));
    steps(&mut s, 1, 0);
    let lv = LockTarget::Aim(AimTarget::Vehicle(v));
    assert_eq!(s.foot_lock[0].target, Some(lv));
    assert_eq!(s.foot_lock_target(0).map(|t| t.0), Some(lv));
    assert_eq!(held(&s, me), 0.0);
    let mut ticks = 0;
    while !s.unit(me).unwrap().weapon.locked(&class) {
        steps(&mut s, 1, 0);
        ticks += 1;
        assert!(ticks < 60, "never locked: {}", held(&s, me));
    }
    // 1.5 s of 1/30 s ticks, give or take the f32 sum's last tick.
    assert!((45..=46).contains(&ticks), "{ticks} ticks");
    assert_eq!(s.foot_lock_charge(0), Some(1.0));
    assert_eq!(fire(&mut s), Some(MissileTarget::Vehicle(v)));
}

/// The named deviation: on BF1 the lock follows the foot rule, not BF1's
/// magnet. A tank 15 deg off the aim (outside the magnet's 6 deg, inside
/// the 25.8 deg dwell) fills the dwell; a tank at 180 m (past the 175 m
/// reach) is never locked.
#[test]
fn bf1_lock_follows_the_foot_rule_not_the_magnet() {
    let (mut s, me) = scene_rules(torpedo(), GroundRules::Bf1);
    let v = tank(&mut s, Some(1), ahead(60.0, 15.0));
    steps(&mut s, 20, 0);
    assert_eq!(
        s.foot_lock[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(v)))
    );
    assert_eq!(
        s.unit(me).unwrap().weapon.lock_target,
        Some(MissileTarget::Vehicle(v))
    );
    assert!(held(&s, me) > 0.5, "{}", held(&s, me));

    let (mut s, me) = scene_rules(torpedo(), GroundRules::Bf1);
    tank(&mut s, Some(1), ahead(180.0, 0.0));
    steps(&mut s, 20, 0);
    tap(&mut s);
    assert_eq!(s.foot_lock[0].target, None);
    assert_eq!(s.unit(me).unwrap().weapon.lock_target, None);
    assert_eq!(s.foot_lock_target(0), None);
}

/// BF1's two-round torpedo: after one shot a press reloads though a target
/// sits in the cone; on a full clip a press cycles to the next tank.
#[test]
fn bf1_two_round_clip_press_reloads_then_cycles() {
    let (mut s, me) = scene_rules(torpedo(), GroundRules::Bf1);
    let a = tank(&mut s, Some(1), ahead(60.0, 0.0));
    let b = tank(&mut s, Some(1), ahead(60.0, 15.0));
    let (la, lb) = (
        LockTarget::Aim(AimTarget::Vehicle(a)),
        LockTarget::Aim(AimTarget::Vehicle(b)),
    );
    steps(&mut s, 1, 0);
    assert_eq!(s.foot_lock[0].target, Some(la), "the centred one");
    fire(&mut s);
    steps(&mut s, 30, 0);
    let w = &s.unit(me).unwrap().weapon;
    assert!(w.clip == 1 && !w.reloading(), "clip {}", w.clip);
    steps(&mut s, 1, buttons::RELOAD);
    assert!(s.unit(me).unwrap().weapon.reloading(), "a press reloads");
    assert_eq!(s.foot_lock[0].target, Some(la), "the lock stays");
    steps(&mut s, 1, 0);
    // The 4 s reload, then a press on the full clip moves the lock.
    let mut ticks = 0;
    while s.unit(me).unwrap().weapon.reloading() {
        steps(&mut s, 1, 0);
        ticks += 1;
        assert!(ticks < 200, "the reload never ended");
    }
    assert_eq!(s.unit(me).unwrap().weapon.clip, 2);
    tap(&mut s);
    assert_eq!(s.foot_lock[0].target, Some(lb));
    assert!(!s.unit(me).unwrap().weapon.reloading());
}

/// Hold `v` so its centre is `d` m from `me`'s eye, straight ahead.
fn pin(s: &mut Sim, me: UnitHandle, v: VehicleHandle, d: f32) {
    let t = LockTarget::Aim(AimTarget::Vehicle(v));
    let eye = s.units[me.index as usize]
        .aim_ray(s.ground_rules, s.views())
        .0;
    let dy = s.lock_probe(t).unwrap().pos.y - eye.y;
    let flat = (d * d - dy * dy).max(0.0).sqrt();
    let x = &mut s.vehicles[v.index as usize];
    x.pos = Vec3::new(eye.x, x.pos.y, eye.z - flat);
    x.vel = Vec3::ZERO;
}

/// How far `v`'s centre is from `me`'s eye.
fn reach(s: &Sim, me: UnitHandle, v: VehicleHandle) -> f32 {
    let eye = s.units[me.index as usize]
        .aim_ray(s.ground_rules, s.views())
        .0;
    let t = LockTarget::Aim(AimTarget::Vehicle(v));
    s.lock_probe(t).unwrap().pos.distance(eye)
}

/// Ticks of dwell to a full lock on a tank held `d` m out.
fn dwell_ticks(class: WeaponClass, rules: GroundRules, d: f32) -> usize {
    let (mut s, me) = scene_rules(class.clone(), rules);
    let v = tank(&mut s, Some(1), ahead(d, 0.0));
    pin(&mut s, me, v, d);
    steps(&mut s, 1, 0);
    let lv = LockTarget::Aim(AimTarget::Vehicle(v));
    assert_eq!(s.foot_lock[0].target, Some(lv), "{rules:?} {d} m");
    assert_eq!(held(&s, me), 0.0);
    let mut ticks = 0;
    while !s.unit(me).unwrap().weapon.locked(&class) {
        pin(&mut s, me, v, d);
        steps(&mut s, 1, 0);
        ticks += 1;
        assert!(ticks < 200, "{rules:?} {d} m: never locked");
    }
    assert!((reach(&s, me, v) - d).abs() < 0.05, "{}", reach(&s, me, v));
    ticks
}

/// The far lock: out to 100 m the dwell needs LockTime; past it the
/// need rises in a line to twice LockTime at the 175 m reach. Both games'
/// launchers, each on its own LockTime, on both games' maps.
#[test]
fn a_far_lock_needs_a_longer_dwell() {
    assert_eq!(foot_dwell_scale(50.0), 1.0);
    assert_eq!(foot_dwell_scale(100.0), 1.0);
    assert!((foot_dwell_scale(140.0) - (1.0 + 40.0 / 75.0)).abs() < 1e-6);
    assert_eq!(foot_dwell_scale(175.0), 2.0);
    assert_eq!(foot_dwell_scale(185.0), 2.0);
    for (class, name) in [(launcher(), "rocket"), (torpedo(), "torpedo")] {
        for rules in [GroundRules::Bf2, GroundRules::Bf1] {
            // 175 m less a centimetre: the reach's own edge is f32 noise.
            for (d, scale) in [
                (50.0, 1.0),
                (100.0, 1.0),
                (140.0, 1.0 + 40.0 / 75.0),
                (174.99, 2.0),
            ] {
                let want = (class.lock_time * scale * crate::TICK_HZ as f32).round() as usize;
                let got = dwell_ticks(class.clone(), rules, d);
                assert!(
                    (want..=want + 1).contains(&got),
                    "{name} {rules:?} {d} m: {got} ticks, want {want}"
                );
            }
        }
    }
}

/// A full lock stays full as the tank draws away past 100 m: no new dwell,
/// the shot homes; the lock is lost only past 185 m, after the 2 s hold.
#[test]
fn a_held_lock_keeps_past_100_m_and_drops_past_185_m() {
    for class in [launcher(), torpedo()] {
        for rules in [GroundRules::Bf2, GroundRules::Bf1] {
            let (mut s, me) = scene_rules(class.clone(), rules);
            let v = tank(&mut s, Some(1), ahead(60.0, 0.0));
            pin(&mut s, me, v, 60.0);
            steps(&mut s, 60, 0);
            assert!(s.unit(me).unwrap().weapon.locked(&class), "{rules:?}");
            let full = held(&s, me);
            pin(&mut s, me, v, 150.0);
            steps(&mut s, 1, 0);
            assert!(s.unit(me).unwrap().weapon.locked(&class), "{rules:?}");
            assert!(held(&s, me) > full, "{rules:?}: the clock runs on");
            assert_eq!(s.foot_lock_charge(0), Some(1.0));
            // Out to 184 m for 3 s: still held, still full.
            for _ in 0..90 {
                pin(&mut s, me, v, 184.0);
                steps(&mut s, 1, 0);
            }
            let lv = LockTarget::Aim(AimTarget::Vehicle(v));
            assert_eq!(s.foot_lock[0].target, Some(lv), "{rules:?}");
            assert_eq!(s.foot_lock_target(0).map(|t| t.0), Some(lv));
            assert!(s.unit(me).unwrap().weapon.locked(&class), "{rules:?}");
            // Past 185 m: kept through the 2 s hold, then dropped.
            for _ in 0..60 {
                pin(&mut s, me, v, 190.0);
                steps(&mut s, 1, 0);
            }
            assert_eq!(s.foot_lock[0].target, Some(lv), "{rules:?}");
            pin(&mut s, me, v, 190.0);
            steps(&mut s, 1, 0);
            assert_eq!(s.foot_lock[0].target, None, "{rules:?}");
            assert!(!s.unit(me).unwrap().weapon.locked(&class), "{rules:?}");
            assert_eq!(fire(&mut s), None, "{rules:?}");
        }
    }
}

/// A far lock fills the ring slower (1 2/3 s at 150 m) and homes.
#[test]
fn a_far_full_lock_homes() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let (mut s, me) = scene_rules(launcher(), rules);
        let v = tank(&mut s, Some(1), ahead(150.0, 0.0));
        pin(&mut s, me, v, 150.0);
        steps(&mut s, 46, 0);
        let f = s.foot_lock_charge(0).unwrap();
        assert!((0.8..1.0).contains(&f), "{rules:?}: {f}");
        steps(&mut s, 6, 0);
        assert_eq!(s.foot_lock_charge(0), Some(1.0), "{rules:?}");
        assert_eq!(fire(&mut s), Some(MissileTarget::Vehicle(v)), "{rules:?}");
        assert!(reach(&s, me, v) > 145.0);
    }
}

#[test]
fn foot_lock_hash_is_silent_without_a_target() {
    let (mut s, _) = scene();
    tank(&mut s, Some(1), ahead(60.0, 0.0));
    let before = s.hash();
    s.foot_lock = vec![FootLock::default()];
    assert_eq!(s.hash(), before);
    s.foot_lock[0].target = Some(LockTarget::Aim(AimTarget::Turret(3)));
    assert_ne!(s.hash(), before);
}
