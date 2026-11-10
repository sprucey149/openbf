//! BF2's on-foot launcher lock (`analysis/re-ordnance-kinds.md` §5): the
//! lock comes on its own (OURS) or by the button, the hold keeps it, the
//! dwell runs only inside 25.8° and in sight; BF1 maps and space rounds
//! keep the magnet dwell.

use super::*;
use crate::collision::{Body, Shape};
use crate::sim::tests::flat_ground;
use crate::sim::PadState;
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
fn launcher() -> WeaponClass {
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

fn tank(s: &mut Sim, team: Option<u8>, pos: Vec3) -> VehicleHandle {
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

/// A player at the origin facing −Z with the launcher, on a BF2 ground map.
fn scene() -> (Sim, UnitHandle) {
    scene_with(launcher())
}

fn scene_with(class: WeaponClass) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(3);
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
    assert_eq!(s.ground_rules, GroundRules::Bf2);
    (s, me)
}

/// A point `range` m out at `deg` degrees right of −Z.
fn ahead(range: f32, deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(a.sin() * range, 0.0, -a.cos() * range)
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

fn steps(s: &mut Sim, n: usize, buttons: u32) {
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

fn held(s: &Sim, me: UnitHandle) -> f32 {
    s.unit(me).unwrap().weapon.lock_held
}

/// The playtest's rule (B7): dwelling alone locks, fills LockTime and the
/// shot homes; a press on the same lone target keeps it.
#[test]
fn bf2_launcher_locks_by_dwelling_alone() {
    const { assert!(FOOT_AUTO_LOCK) };
    let (mut s, me) = scene();
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
        "{}",
        held(&s, me)
    );
    assert_eq!(s.foot_lock_charge(0), Some(1.0));
    assert_eq!(fire(&mut s), Some(MissileTarget::Vehicle(v)));
    // The shot spends the dwell, not the lock.
    assert_eq!(s.unit(me).unwrap().weapon.lock_held, 0.0);
    assert!(s.foot_lock[0].target.is_some());
}

/// The pick: TargetLockMaxDistance, dot >= 0.8, sight, hostile only.
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
            tank(s, Some(1), ahead(120.0, 0.0));
        }),
        None,
        "120 m is past the 100 m default"
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
    // The control: 95 m at 30 deg locks.
    assert!(refused(&|s| {
        tank(s, Some(1), ahead(95.0, 30.0));
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
/// inside restarts the count.
#[test]
fn locked_target_drops_after_two_seconds_away() {
    let (mut s, me) = scene();
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
/// hands the target back to the auto lock.
#[test]
fn the_bulldog_reloads_a_partly_fired_clip_and_locks_on_a_full_one() {
    let (mut s, me) = scene_with(bulldog());
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

/// BF1 maps and space rounds keep the magnet's dwell exactly: no press,
/// the candidate is the magnet, and the foot lock stays empty.
#[test]
fn bf1_rules_and_space_rounds_keep_the_magnet_dwell() {
    for space in [false, true] {
        let (mut s, me) = scene();
        if space {
            s.space_assault = Some(crate::sim::SpaceAssault::new(10));
        } else {
            s.ground_rules = GroundRules::Bf1;
        }
        let v = tank(&mut s, Some(1), ahead(60.0, 0.0));
        steps(&mut s, 2, 0);
        assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)), "space {space}");
        assert_eq!(
            s.unit(me).unwrap().weapon.lock_target,
            Some(MissileTarget::Vehicle(v))
        );
        steps(&mut s, 30, 0);
        let class = s.weapon_classes[s.unit(me).unwrap().weapon_class].clone();
        assert!(s.unit(me).unwrap().weapon.locked(&class), "space {space}");
        assert!(s.foot_lock.iter().all(|f| *f == FootLock::default()));
        // The button still reloads there.
        fire(&mut s);
        steps(&mut s, 30, 0);
        steps(&mut s, 1, buttons::RELOAD);
        assert!(s.unit(me).unwrap().weapon.reloading(), "space {space}");
        // BF1's HUD square follows the dwell target; space shows none.
        assert_eq!(s.foot_lock_target(0).is_some(), !space);
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
