//! Scene tests for missiles (card UORD, `analysis/re-ordnance-kinds.md` §4-§5):
//! homing onto a locked target's point (and the drop of a target that dies in
//! flight), an unguided round's straight flight and expiry burst, the dwell
//! lock (`acquire_magnet`, a bot's combat target, a non-launcher), its
//! consumption at the press and a vehicle's aim-sphere point.

use super::*;
use crate::collision::HullFilter;
use crate::projectile::Shooter;
use crate::sim::tests::flat_ground;
use crate::sim::{buttons, Event, PadState};
use crate::soldier::Stance;
use crate::unit::{UnitHandle, UnitSpec};
use crate::vehicle::{BlastSetup, SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::{blast_falloff, ExplosionClass, HealthType, OrdKind, WeaponClass};
use crate::TICK_DT;

/// A launcher's missile class at the REP torpedo's numbers, on a rifle's
/// chassis: 40 m/s, TurnRate 2, no waver, a 1 s lock, a blast on expiry.
fn torpedo() -> WeaponClass {
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
    // Two shots at once, so the trigger's own cycle does not hide the flight.
    w.rounds_per_clip = 8;
    w.lock_time = 1.0;
    w
}

/// Flat ground (a trooper must stand on something) and no map bodies.
fn sim() -> Sim {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    s
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

fn step(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.step(&[PadState::default()]);
    }
}

/// A player (`me`) at the origin holding the launcher, facing −Z with a level
/// aim, and an idle trooper of team 1 `range` m down −Z.
fn scene(range: f32) -> (Sim, UnitHandle, UnitHandle, usize) {
    let mut s = sim();
    let wc = s.add_weapon_class(torpedo());
    let me = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    s.units[me.index as usize].soldier.pitch = 0.0;
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -range)))
        .unwrap();
    (s, me, enemy, wc)
}

/// A missile of class `wc` in flight from `origin` along `dir`, as
/// `Sim::soldier_shots` would have left it (no predictive ray, the class's
/// life, the lock the caller passes). Returns its bolt index.
fn launch(
    s: &mut Sim,
    me: UnitHandle,
    wc: usize,
    origin: Vec3,
    dir: Vec3,
    target: Option<MissileTarget>,
) -> usize {
    let ord = s.weapon_classes[wc].ordnance.clone();
    let mc = *ord.missile.as_ref().expect("a missile class");
    let who = Shooter {
        owner: me,
        team: s.unit(me).unwrap().team,
        weapon_class: wc,
        is_player: true,
    };
    let mut b = Bolt::fire(
        origin,
        dir,
        ord.velocity,
        ord.life_span,
        who,
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        HullFilter::All,
    )
    .shaped(&ord);
    b.as_missile(&ord, &mc, target, &mut s.lcg);
    s.bolts.push(b);
    s.bolts.len() - 1
}

/// Press FIRE until the launcher's round is in the air (one shot this press).
fn fire(s: &mut Sim) {
    for _ in 0..4 {
        s.step(&[pad(buttons::FIRE)]);
        if !s.bolts.is_empty() {
            return;
        }
    }
    panic!("the launcher did not fire");
}

/// `n` counted dwell ticks after the acquisition's settle tick.
fn dwell(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.tick_locks(TICK_DT);
    }
}

/// §4.3: a missile turns its launch direction onto its locked target's aim
/// point (within a few degrees a second in) and strikes it.
#[test]
fn a_guided_missile_curves_onto_its_live_target_and_strikes_it() {
    // A target 20 m off the launch line at 50 m: 21 deg of turn to make.
    let (mut s, me, enemy, wc) = scene(50.0);
    s.units[enemy.index as usize].soldier.pos = Vec3::new(20.0, 0.0, -50.0);
    let hp = s.unit(enemy).unwrap().health.cur;
    let origin = s.unit(me).unwrap().muzzle();
    launch(
        &mut s,
        me,
        wc,
        origin,
        Vec3::NEG_Z,
        Some(MissileTarget::Unit(enemy)),
    );
    let start = s.bolts[0].missile.unwrap().dir;
    // The missile turns off its launch line ...
    step(&mut s, 10);
    let turned = s.bolts[0].missile.unwrap().dir.angle_between(start);
    assert!(turned > 0.1, "it turned {turned} rad");
    // ... and a second in, its direction is the target's point's.
    step(&mut s, 20);
    let b = &s.bolts[0];
    let aim = s.unit(enemy).unwrap().aim_point();
    let off = b
        .missile
        .unwrap()
        .dir
        .angle_between((aim - b.pos).normalize());
    assert!(off < 0.09, "aimed {:.2} deg off: {}", off.to_degrees(), off);
    // It strikes the target.
    for _ in 0..120 {
        if s.bolts.is_empty() {
            break;
        }
        step(&mut s, 1);
    }
    assert!(s.bolts.is_empty(), "it struck or expired");
    let left = s.unit(enemy).unwrap().health.cur;
    assert!(left < hp, "the missile hurt its target: {hp} -> {left}");
}

/// The retained direction is the last live aim minus the position BEFORE that
/// guide pass. It is neither the launch line, the current flight direction nor
/// a world point to chase after handle loss (BF2 FUN_0031f2e8:312665-312768,
/// 312843-312872; BF1 FUN_0027d6b8:202955-202987).
#[test]
fn target_loss_retains_the_last_desired_heading() {
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        for generation_mismatch in [false, true] {
            let (mut s, me, enemy, wc) = scene(100.0);
            s.ground_rules = rules;
            s.units[enemy.index as usize].controller = Controller::Player { slot: 1 };
            s.units[enemy.index as usize].soldier.pos.x = 40.0;
            let origin = s.unit(me).unwrap().muzzle();
            launch(
                &mut s,
                me,
                wc,
                origin,
                Vec3::NEG_Z,
                Some(MissileTarget::Unit(enemy)),
            );
            step(&mut s, 14);
            let previous_pos = s.bolts[0].pos;
            step(&mut s, 1);
            let last_point = s.unit(enemy).unwrap().aim_point();
            let desired = (last_point - previous_pos).normalize();
            let mut expected = s.bolts[0].missile.unwrap();
            let mut frozen_point = expected;
            let mut frozen_point_delta = 0.0_f32;
            assert!(expected.desired.abs_diff_eq(desired, 1e-6));
            assert!(desired.angle_between(expected.launch) > 0.3);
            assert!(
                desired.angle_between(expected.dir) > 0.05,
                "still turning at loss"
            );
            if generation_mismatch {
                s.units[enemy.index as usize].gen += 1;
            } else {
                s.units[enemy.index as usize].alive = false;
            }
            assert!(s.unit(enemy).is_none());
            let c = s.weapon_classes[wc].ordnance.missile.unwrap();
            for tick in 0..12 {
                let pos = s.bolts[0].pos;
                let mut rng = s.lcg.clone();
                let want = expected.step(&c, Some(pos + desired * 100.0), pos, TICK_DT, &mut rng);
                let stale = frozen_point.step(&c, Some(last_point), pos, TICK_DT, &mut rng);
                step(&mut s, 1);
                let b = &s.bolts[0];
                frozen_point_delta = frozen_point_delta.max((stale - b.vel).length());
                assert_eq!(b.missile.unwrap().target, None, "invalid handle cleared");
                assert!(b.vel.abs_diff_eq(want, 1e-5),
                    "{rules:?} generation_mismatch={generation_mismatch} loss tick {tick}: got {:?}, retained-heading {:?}", b.vel, want);
            }
            println!("{rules:?} generation_mismatch={generation_mismatch} frozen-point delta {frozen_point_delta} m/s");
            assert!(
                frozen_point_delta > 1e-3,
                "frozen-point control must differ by more than 100 times the velocity tolerance"
            );
        }
    }
}

/// Losing the handle before any live guide pass keeps the initialized heading.
#[test]
fn target_loss_before_first_guide_keeps_the_launch_heading() {
    let (mut s, me, enemy, wc) = scene(100.0);
    let origin = s.unit(me).unwrap().muzzle();
    launch(
        &mut s,
        me,
        wc,
        origin,
        Vec3::NEG_Z,
        Some(MissileTarget::Unit(enemy)),
    );
    s.units[enemy.index as usize].gen += 1;
    step(&mut s, 1);
    let m = s.bolts[0].missile.unwrap();
    assert_eq!(m.target, None);
    assert_eq!(m.desired, m.launch);
    assert_eq!(m.dir, m.launch);
}

/// §4.3: no target, no steer - the round keeps its launch direction (this
/// class waver 0) and bursts with its ExplosionExpire where LifeSpan runs
/// out, hurting a soldier standing at the burst.
#[test]
fn an_unguided_missile_flies_its_launch_line_and_bursts_at_its_life() {
    let (mut s, me, enemy, wc) = scene(50.0);
    // The scene's trooper steps off the line: only the burst may hurt.
    s.units[enemy.index as usize].soldier.pos = Vec3::new(-40.0, 0.0, 0.0);
    // 3 m off the burst point (160 m down −Z): out of the sweep's reach, in
    // the blast's.
    let victim = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, -160.0)))
        .unwrap();
    let hp = s.unit(victim).unwrap().health.cur;
    let origin = s.unit(me).unwrap().muzzle();
    launch(&mut s, me, wc, origin, Vec3::NEG_Z, None);
    let ord = s.weapon_classes[wc].ordnance.clone();
    let burst = origin + Vec3::NEG_Z * (ord.velocity * ord.life_span);
    for _ in 0..400 {
        if s.bolts.is_empty() {
            break;
        }
        let dir = s.bolts[0].missile.unwrap().dir;
        assert!(
            dir.angle_between(Vec3::NEG_Z) < 1e-5,
            "no steer, the launch line: {dir}"
        );
        assert_eq!(s.bolts[0].missile.unwrap().target, None);
        s.step(&[PadState::default()]);
    }
    assert!(s.bolts.is_empty(), "it bursts at LifeSpan");
    let expired = s
        .events
        .iter()
        .find_map(|e| match e {
            Event::BoltExpired { pos, struck } => Some((*pos, *struck)),
            _ => None,
        })
        .expect("the burst says so");
    assert!(!expired.1, "in the air, not struck");
    assert!(
        (expired.0 - burst).length() < 0.01,
        "burst at {} want {burst}",
        expired.0
    );
    // The blast is ExplosionExpire's: its damage at the victim's sphere.
    let (lift, w) = crate::sim::soldier_blast_sphere(Stance::Stand);
    let d = (Vec3::new(3.0, lift, -160.0) - burst).length();
    let e = ord.expire.as_ref().unwrap();
    let want = e.damage * blast_falloff(d, w, e.damage_inner, e.damage_outer);
    let lost = hp - s.unit(victim).unwrap().health.cur;
    assert!((lost - want).abs() < 1e-2, "{lost} vs {want}");
}

/// §5: a human's launcher counts its magnet target's dwell; LockTime is
/// reached on the 30th counted tick, a new target restarts the clock, and a
/// class that is no launcher never locks.
#[test]
fn a_launchers_dwell_lock_reaches_lock_time_and_a_new_target_restarts_it() {
    let (mut s, me, enemy, wc) = scene(40.0);
    let class = s.weapon_classes[wc].clone();
    let other = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(1.0, 0.0, -40.0)))
        .unwrap();
    let i = me.index as usize;
    // The magnet is the acquisition's: the enemy under the reticle.
    s.acquire_magnet(0, me, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(enemy)));
    // The first tick takes the candidate and starts the clock at zero.
    s.tick_locks(TICK_DT);
    assert_eq!(
        s.units[i].weapon.lock_target,
        Some(MissileTarget::Unit(enemy))
    );
    assert_eq!(s.units[i].weapon.lock_held, 0.0);
    assert!(!s.units[i].weapon.locked(&class));
    // 29 counted ticks are short of the 1 s LockTime ...
    for n in 1..30 {
        dwell(&mut s, 1);
        assert!(
            !s.units[i].weapon.locked(&class),
            "tick {n}: held {}",
            s.units[i].weapon.lock_held
        );
    }
    // ... and the 30th reaches it.
    dwell(&mut s, 1);
    assert!(
        s.units[i].weapon.locked(&class),
        "held {}",
        s.units[i].weapon.lock_held
    );
    assert!((s.units[i].weapon.lock_held - 1.0).abs() < 1e-5);
    // A new target restarts the clock: the first steps out of the cone and
    // the magnet takes the second.
    s.units[enemy.index as usize].soldier.pos = Vec3::new(40.0, 0.0, 0.0);
    s.acquire_magnet(0, me, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(other)));
    s.tick_locks(TICK_DT);
    assert_eq!(
        s.units[i].weapon.lock_target,
        Some(MissileTarget::Unit(other))
    );
    assert_eq!(s.units[i].weapon.lock_held, 0.0);
    assert!(!s.units[i].weapon.locked(&class));
    for _ in 0..29 {
        dwell(&mut s, 1);
        assert!(!s.units[i].weapon.locked(&class));
    }
    dwell(&mut s, 1);
    assert!(s.units[i].weapon.locked(&class));

    // A non-launcher never locks: a rifle's state (no missile) and a
    // missile class with LockTime 0 both drop a hand-set candidate.
    let mut s = sim();
    let rifle = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let mut no_lock = torpedo();
    no_lock.lock_time = 0.0;
    let wc = s.add_weapon_class(no_lock);
    let inert = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            ..UnitSpec::trooper(0, Vec3::new(2.0, 0.0, 0.0))
        })
        .unwrap();
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
        .unwrap();
    for h in [rifle, inert] {
        let u = &mut s.units[h.index as usize];
        u.weapon.lock_target = Some(MissileTarget::Unit(enemy));
        u.weapon.lock_held = 5.0;
    }
    s.tick_locks(TICK_DT);
    for h in [rifle, inert] {
        let u = &s.units[h.index as usize];
        let class = s.weapon_classes[u.weapon_class].clone();
        assert_eq!(u.weapon.lock_target, None, "{h:?}");
        assert_eq!(u.weapon.lock_held, 0.0, "{h:?}");
        assert!(!u.weapon.locked(&class), "{h:?}");
    }
}

/// §5: the press consumes the lock - a locked shot leaves guided and empties
/// the weapon's state; a shot before LockTime leaves unguided.
#[test]
fn the_press_consumes_the_lock_and_carries_it_onto_the_missile() {
    // Locked: the missile carries the target.
    let (mut s, me, enemy, wc) = scene(40.0);
    let class = s.weapon_classes[wc].clone();
    let i = me.index as usize;
    s.acquire_magnet(0, me, false);
    s.tick_locks(TICK_DT);
    dwell(&mut s, 30);
    assert!(s.units[i].weapon.locked(&class));
    fire(&mut s);
    assert_eq!(s.bolts.len(), 1);
    let m = s.bolts[0].missile.expect("a missile");
    assert_eq!(m.target, Some(MissileTarget::Unit(enemy)));
    assert_eq!(s.units[i].weapon.lock_target, None);
    assert_eq!(s.units[i].weapon.lock_held, 0.0);
    assert!(!s.units[i].weapon.locked(&class));

    // Before the lock: the same press leaves unguided and the lock is
    // consumed all the same (B1 class+0x258 && +0x254).
    let (mut s, me, _enemy, wc) = scene(40.0);
    let class = s.weapon_classes[wc].clone();
    let i = me.index as usize;
    s.acquire_magnet(0, me, false);
    s.tick_locks(TICK_DT);
    dwell(&mut s, 14);
    assert!(!s.units[i].weapon.locked(&class));
    fire(&mut s);
    assert_eq!(s.bolts.len(), 1);
    assert_eq!(s.bolts[0].missile.expect("a missile").target, None);
    assert_eq!(s.units[i].weapon.lock_target, None);
    assert_eq!(s.units[i].weapon.lock_held, 0.0);
}

/// §5: a bot's candidate is its combat target (`AiState::target`) - no
/// magnet and no cone: it locks even with the enemy behind it.
#[test]
fn a_bot_locks_its_combat_target_with_no_magnet_and_no_cone() {
    let mut s = sim();
    let wc = s.add_weapon_class(torpedo());
    let class = s.weapon_classes[wc].clone();
    // The bot faces +Z (yaw π); its target stands behind it, down −Z.
    let bot = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            controller: Controller::Ai,
            yaw: std::f32::consts::PI,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -50.0)))
        .unwrap();
    assert!(s.magnet.is_empty(), "no magnet to speak of");
    let i = bot.index as usize;
    s.units[i].ai.target = Some(enemy.into());
    s.tick_locks(TICK_DT);
    assert_eq!(
        s.units[i].weapon.lock_target,
        Some(MissileTarget::Unit(enemy))
    );
    assert_eq!(s.units[i].weapon.lock_held, 0.0);
    for _ in 0..29 {
        dwell(&mut s, 1);
        assert!(!s.units[i].weapon.locked(&class));
    }
    dwell(&mut s, 1);
    assert!(s.units[i].weapon.locked(&class));
    // No target, no lock: the clock and the candidate go.
    s.units[i].ai.target = None;
    dwell(&mut s, 1);
    assert_eq!(s.units[i].weapon.lock_target, None);
    assert_eq!(s.units[i].weapon.lock_held, 0.0);
}

fn bike(pos: Vec3) -> VehicleSpec {
    VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
}

/// §4.4: a missile locked onto a vehicle homes on its collision sphere's
/// centre (`Vehicle::aim_sphere`, here the class's blast sphere) in its
/// frame, not on its origin.
#[test]
fn a_missile_locked_onto_a_vehicle_homes_on_its_aim_sphere_centre() {
    let (mut s, me, enemy, wc) = scene(50.0);
    // Only the vehicle is in the missile's way.
    s.units[enemy.index as usize].soldier.pos = Vec3::new(-40.0, 0.0, 0.0);
    s.blast_setup = vec![Some(BlastSetup {
        explosion: None,
        sphere_centre: Vec3::Y * 10.0,
        sphere_radius: 2.0,
    })];
    let v = s.spawn_vehicle(&bike(Vec3::new(0.0, 0.0, -60.0)));
    let vpos = s.vehicle(v).unwrap().pos;
    let centre = vpos + s.vehicle(v).unwrap().rot * (Vec3::Y * 10.0);
    let aim = s.missile_aim(MissileTarget::Vehicle(v)).expect("the hull");
    assert!((aim - centre).length() < 1e-4, "{aim} vs {centre}");
    // Fired level at the vehicle's ORIGIN, it climbs to the sphere's centre.
    let origin = s.unit(me).unwrap().muzzle();
    launch(
        &mut s,
        me,
        wc,
        origin,
        (vpos - origin).normalize(),
        Some(MissileTarget::Vehicle(v)),
    );
    step(&mut s, 30);
    let b = &s.bolts[0];
    let aim = s.missile_aim(MissileTarget::Vehicle(v)).expect("the hull");
    let dir = b.vel.normalize();
    assert!(dir.y > 0.05, "it climbs to the sphere: {dir}");
    let off = dir.angle_between((aim - b.pos).normalize());
    assert!(off < 0.1, "homes on {} not {}: {off} rad", aim, vpos);
}

#[test]
fn launcher_locks_a_turret_hashes_its_id_and_drops_a_destroyed_target() {
    let (mut s, me, owner, _) = scene(50.0);
    let class = crate::autoturret::TurretClass::new(
        &crate::weapon::test_props::List(&[]),
        WeaponClass::bf1_rifle(),
    );
    s.turrets.push(crate::autoturret::Turret::new(
        19,
        owner,
        1,
        0,
        &class,
        Vec3::new(0.0, 1.0, -10.0),
        Vec3::ZERO,
        0.0,
    ));
    s.magnet[0] = Some(AimTarget::Turret(19));
    s.tick_locks(TICK_DT);
    s.tick_locks(TICK_DT);
    assert_eq!(
        s.unit(me).unwrap().weapon.lock_target,
        Some(MissileTarget::Turret(19))
    );
    assert_eq!(s.unit(me).unwrap().weapon.lock_held, TICK_DT);
    assert_eq!(
        s.missile_aim(MissileTarget::Turret(19)),
        Some(s.turrets[0].centre())
    );
    let before = s.hash();
    s.unit_mut(me).unwrap().weapon.lock_target = Some(MissileTarget::Turret(20));
    assert_ne!(s.hash(), before);
    s.turrets[0].health = 0.0;
    s.tick_locks(TICK_DT);
    assert_eq!(s.unit(me).unwrap().weapon.lock_target, None);
    assert_eq!(s.missile_aim(MissileTarget::Turret(19)), None);
}
