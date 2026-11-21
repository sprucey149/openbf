//! Issue #164: a human gunner's aim assist in a building turret.

use super::*;
use crate::sim::tests::flat_ground;
use crate::sim::{buttons, Event};
use crate::unit::UnitSpec;
use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon, Slew};
use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
use crate::weapon::HealthType;

use crate::weapon::DEG;

/// A building gun turret at the origin facing +Z: the turret turns the
/// full circle at 2 rad/s and pitches -30..30 degrees at 1 rad/s, both
/// filters 10 (`slew` the game's law), the camera 2 m behind and 1 m over
/// the pivot, the barrel 1.5 m ahead of it; its gun the BF1 rifle's 80 m/s
/// round for 1.5 s (120 m, BF1's building gun's), zooming at 2.5. The
/// human gunner (team 0) is aboard.
fn turret_world(slew: Slew) -> (Sim, UnitHandle, VehicleHandle) {
    turret_world_with(test_gun(slew, 10.0))
}

/// [`turret_world`]'s gun with both turn filters `filter`.
fn test_gun(slew: Slew, filter: f32) -> GunSetup {
    GunSetup {
        weapons: vec![GunWeapon {
            class: 0,
            infinite: true,
            reserve_clips: 0,
            fire_points: Vec::new(),
            fire_limits: Vec::new(),
            fire_pivots: Vec::new(),
        }],
        fire_point: Vec3::new(0.0, 2.0, 1.5),
        pivot: Vec3::new(0.0, 2.0, 0.0),
        eye: Vec3::new(0.0, 1.0, -2.0),
        aimer: Aimer {
            yaw: [0.0, 0.0],
            pitch: [-30.0 * DEG, 30.0 * DEG],
            ..Aimer::default()
        },
        pilot: false,
        turn: Some(Turn::from_class(
            Some([-180.0, 180.0]),
            Some([-30.0, 30.0]),
            (2.0, 1.0),
            (filter, filter),
            slew,
        )),
        framed: true,
        aimer_bind: Quat::IDENTITY,
        weapon_aimer: None,
        follows_owner: false,
        gunner: None,
    }
}

/// BF1's `com_weap_gunturret` turn law from its class (the bake's
/// `bf1_common`): `YawLimits -180 180`, `PitchLimits -45 45`,
/// `MaxTurnSpeed 0.85`, `MaxPitchSpeed 0.75`, no `TurnFilter` (the default
/// 10), BF1's exponential slew, `AimerYawLimits -5 5`, `AimerPitchLimits
/// -45 45`, `EyePointOffset 0 1.5 1`; its `com_weap_bldg_gunturret` bolt is
/// 80 m/s for 1.5 s, the BF1 rifle's. The pivot (`main_cylinder`) and fire
/// point (`hp_fire`) are model nodes, not in the class: stand-ins 1 m up and
/// 1.5 m ahead of it here.
fn bf1_gunturret() -> GunSetup {
    let mut g = test_gun(Slew::Exp, 10.0);
    g.pivot = Vec3::new(0.0, 1.0, 0.0);
    g.fire_point = Vec3::new(0.0, 1.0, 1.5);
    g.eye = Vec3::new(0.0, 1.5, 1.0);
    g.aimer.yaw = [-5.0 * DEG, 5.0 * DEG];
    g.aimer.pitch = [-45.0 * DEG, 45.0 * DEG];
    g.turn = Some(Turn::from_class(
        Some([-180.0, 180.0]),
        Some([-45.0, 45.0]),
        (0.85, 0.75),
        (10.0, 10.0),
        Slew::Exp,
    ));
    g
}

/// BF2's `tur_bldg_recoilless_lg` turn law from its class (the bake's
/// `bf2_tur`): `YawLimits -180 180`, `PitchLimits -30 20`, `MaxTurnSpeed`
/// and `MaxPitchSpeed 3.8`, no `TurnFilter` (10), BF2's linear slew,
/// `AimerYawLimits 0 0`, `AimerPitchLimits -30 20`, `EyePointOffset 1.0
/// 4.2 -8.3`, with card TURR's model nodes (`aimer_y` at the origin,
/// `hp_fire1` 3.39 m over it and 1.28 m to the side). The gun is the BF1
/// rifle in place of its recoilless rifle, as the bot test's.
fn bf2_recoilless() -> GunSetup {
    let mut g = test_gun(Slew::Linear, 10.0);
    g.pivot = Vec3::ZERO;
    g.fire_point = Vec3::new(1.281, 3.388, 3.372);
    g.eye = Vec3::new(1.0, 4.2, -8.3);
    g.aimer.pitch = [-30.0 * DEG, 20.0 * DEG];
    g.turn = Some(Turn::from_class(
        Some([-180.0, 180.0]),
        Some([-30.0, 20.0]),
        (3.8, 3.8),
        (10.0, 10.0),
        Slew::Linear,
    ));
    g
}

/// [`turret_world`] with the seat's gun `g`.
fn turret_world_with(g: GunSetup) -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    s.hover_params = vec![None];
    s.add_weapon_class(WeaponClass::bf1_rifle());
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 15.0,
        ai_size: 1,
        guns: vec![Some(g)],
    })];
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    });
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, -3.0)))
        .unwrap();
    assert!(s.enter_vehicle(p, v, 0));
    s.step(&[PadState::default()]);
    assert!(
        s.gunner_turret(p).is_some(),
        "a human gunner in a building turret"
    );
    (s, p, v)
}

/// Lay the seat's camera ray on `at` (the eye turns with the yaw).
fn look_at(s: &mut Sim, p: UnitHandle, v: VehicleHandle, at: Vec3) {
    for _ in 0..3 {
        let eye = s.seat_look(p).unwrap().eye;
        let look = gun::angles(at - eye);
        let g = s.vehicle_mut(v).unwrap().guns[0].as_mut().unwrap();
        g.look = look;
        g.rate = (0.0, 0.0);
    }
}

/// What [`cross`] measured.
struct Cross {
    shots: u32,
    /// Shots followed by a hit on the runner before the next.
    hit_shots: u32,
    /// Ticks the runner was not the magnet.
    lost: u32,
}

/// A runner `dist` m out crossing at his run speed and turning about every
/// `period` ticks, about the turret's line; the gunner's sticks still, his
/// camera on the runner at the start, tapping FIRE every 10 ticks for
/// `ticks`, bar the last second (the last rounds land). `buttons_held` adds
/// buttons to every tick's pad (the mouse); `intensity` is the assist's.
fn cross(slew: Slew, dist: f32, intensity: f32, buttons_held: u32, run: (u32, u32)) -> Cross {
    cross_with(test_gun(slew, 10.0), dist, intensity, buttons_held, run).0
}

/// [`cross`] in a turret with gun `g`, and the world after it.
fn cross_with(
    g: GunSetup,
    dist: f32,
    intensity: f32,
    buttons_held: u32,
    (period, ticks): (u32, u32),
) -> (Cross, Sim) {
    let (mut s, p, v) = turret_world_with(g);
    s.assist_intensity = intensity;
    let e = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, dist)))
        .unwrap();
    let run = s.unit(e).unwrap().consts.max_speed;
    let u = &mut s.units[e.index as usize];
    u.soldier.pos.x = -run * period as f32 * crate::TICK_DT * 0.5;
    u.controller = Controller::Pacer {
        vel: Vec3::X * run,
        period,
    };
    u.health.cur = 1.0e9;
    let at = s.unit(e).unwrap().capsule().center();
    look_at(&mut s, p, v, at);
    s.step(&[PadState {
        buttons: buttons_held,
        ..PadState::default()
    }]);
    let mut out = Cross {
        shots: 0,
        hit_shots: 0,
        lost: 0,
    };
    let mut hit = false;
    for k in 0..ticks {
        let pad = PadState {
            buttons: buttons_held
                | if k % 10 == 0 && k + 30 < ticks {
                    buttons::FIRE
                } else {
                    0
                },
            ..PadState::default()
        };
        s.step(&[pad]);
        for ev in &s.events {
            match *ev {
                Event::Shot { unit, .. } if unit == p => {
                    out.shots += 1;
                    hit = false;
                }
                Event::Hit { victim, .. } if victim == e => {
                    out.hit_shots += u32::from(!hit);
                    hit = true;
                }
                _ => {}
            }
        }
        s.units[e.index as usize].health.cur = 1.0e9;
        if s.magnet_at(0) != Some(AimTarget::Unit(e)) {
            out.lost += 1;
        }
    }
    (out, s)
}

/// The turret magnet's values: the round's reach (80 m/s x 1.5 s), full
/// strength to three quarters of it (to the disc's reach once the bolt is
/// lengthened), the rifle's 1° cone (1° too when the
/// class has none), soldiers and droids, and vehicles only when the round
/// hurts them. The weapon's own Target* bits are left as parsed.
#[test]
fn a_turret_magnet_reaches_as_far_as_its_round() {
    let mut c = WeaponClass::bf1_rifle();
    c.ordnance.scales[3] = 0.0;
    let m = seat_magnet(&c);
    assert_eq!((m.reach, m.full, m.angle), (120.0, 90.0, DEG));
    assert_eq!(m.kinds & 0x1f, 0b111);
    c.ordnance.scales[3] = 1.0;
    c.lock_on_angle = 0.0;
    let m = seat_magnet(&c);
    assert_eq!((m.angle, m.kinds & 0x1f), (SNIPER_LOCK_ON_ANGLE, 0b1111));
    assert_eq!(c.target_kind, WeaponClass::bf1_rifle().target_kind);
    // [ours] (2026-11-20) Its bolt lengthened to 200 m (BF1's building gun
    // turret): full strength to the disc's 120 m, nothing at 200 m.
    c.ordnance.extend_bolt_reach();
    let m = seat_magnet(&c);
    assert_eq!(m.full, 120.0);
    assert!(m.reach >= crate::weapon::BOLT_REACH && m.reach < crate::weapon::BOLT_REACH + 0.01);
}

/// A pad gunner holding his sticks still on a runner crossing at 60 and
/// 85 m hits him with the assist (BF1's and BF2's slew laws) and never
/// without it; the magnet holds throughout. On one 8 s crossing he misses
/// only while the lead grows in and the pull closes on it (the first one to
/// two seconds: the lead grows at 10 m/s to 6.8 m at 60 m, 9.5 m at 85 m). Over 20 s with a turn about every 5 s, the rounds in
/// flight at each turn and the lead's swing to his other side (at 85 m the
/// round flies 1.1 s and the lead is 9.5 m) miss too, as on foot. A
/// mouse-aimed pad keeps the magnet (as on foot) but is never pulled.
#[test]
fn a_turret_gunner_on_a_pad_leads_and_hits_a_crossing_runner() {
    for slew in [Slew::Exp, Slew::Linear] {
        for dist in [60.0, 85.0] {
            for (run, share) in [((240, 240), 0.5), ((150, 600), 0.3)] {
                let on = cross(slew, dist, 1.0, 0, run);
                let off = cross(slew, dist, 0.0, 0, run);
                let mouse = cross(slew, dist, 1.0, buttons::MOUSE_AIM, run);
                println!(
                    "{slew:?} at {dist} m, {run:?}: assist {}/{} (lost {}), none {}/{}, mouse {}/{}",
                    on.hit_shots, on.shots, on.lost, off.hit_shots, off.shots, mouse.hit_shots,
                    mouse.shots
                );
                assert_eq!(on.shots, off.shots, "{slew:?} {dist}");
                assert!(
                    on.hit_shots as f32 >= on.shots as f32 * share,
                    "{slew:?} {dist} {run:?}: {}/{}",
                    on.hit_shots,
                    on.shots
                );
                assert_eq!(off.hit_shots, 0, "{slew:?} {dist}");
                assert_eq!(mouse.hit_shots, 0, "{slew:?} {dist}: mouse");
                assert_eq!(on.lost, 0, "{slew:?} {dist}: the magnet held");
            }
        }
    }
}

/// The pull settles without swinging past: a still enemy 3° off the
/// camera's line at 50 m, FIRE held, the sticks still. The look closes on
/// the barrel's line to him and never crosses it by more than 0.05°, for
/// both slew laws, zoomed and not, and turn filters from 1 to 400 (a filter
/// past about 200 takes the target rate within the tick: the correction's
/// cap at the error left keeps it from crossing). Within 3 s it is within
/// 0.3° of the line, bar the filter of 1, which turns too slowly for that.
#[test]
fn the_turret_pull_settles_without_overshoot() {
    for (slew, filter, zoomed) in [Slew::Exp, Slew::Linear]
        .into_iter()
        .flat_map(|w| [1.0, 10.0, 50.0, 200.0, 400.0].map(|f| (w, f)))
        .flat_map(|(w, f)| [false, true].map(|z| (w, f, z)))
    {
        let (mut s, p, v) = turret_world_with(test_gun(slew, filter));
        let e = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 50.0)))
            .unwrap();
        let at = s.unit(e).unwrap().capsule().center();
        look_at(&mut s, p, v, at);
        if zoomed {
            for b in [buttons::ZOOM, 0] {
                s.step(&[PadState {
                    buttons: b,
                    ..PadState::default()
                }]);
            }
            assert!(s.seat_zoom(p).is_some());
            look_at(&mut s, p, v, at);
        }
        s.step(&[PadState::default()]);
        assert_eq!(s.magnet_at(0), Some(AimTarget::Unit(e)));
        let start = s.vehicle(v).unwrap().guns[0].as_ref().unwrap().look.0;
        s.vehicle_mut(v).unwrap().guns[0].as_mut().unwrap().look.0 = start + 3.0 * DEG;
        let fire = PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        };
        let mut worst = 0.0f32;
        let mut last = 0.0;
        for _ in 0..90 {
            s.step(&[fire]);
            let look = s.vehicle(v).unwrap().guns[0].as_ref().unwrap().look.0;
            // Past the target's line (below `start`) is an overshoot.
            worst = worst.max(start - look);
            last = look - start;
        }
        println!(
            "{slew:?} filter {filter} zoomed {zoomed}: overshoot {worst} rad, left {last} rad"
        );
        let case = format!("{slew:?} filter {filter} zoomed {zoomed}");
        assert!(worst < 0.05 * DEG, "{case}: {worst}");
        assert!(last < 3.0 * DEG, "{case}: {last}");
        if filter >= 10.0 {
            assert!(last.abs() < 0.3 * DEG, "{case}: {last}");
        }
    }
}

/// The building turrets of both games under their own classes' turn laws
/// ([`bf1_gunturret`], [`bf2_recoilless`]): a still stick on a runner
/// crossing at 60 m for 8 s hits him with the assist (16 of 21 shots in
/// both) and never without it.
#[test]
fn a_bf1_and_a_bf2_building_turret_lead_a_crossing_runner() {
    for (name, gun, share) in [
        ("BF1 com_weap_gunturret", bf1_gunturret(), 0.5),
        ("BF2 tur_bldg_recoilless_lg", bf2_recoilless(), 0.5),
    ] {
        let (on, _) = cross_with(gun.clone(), 60.0, 1.0, 0, (240, 240));
        let (off, _) = cross_with(gun, 60.0, 0.0, 0, (240, 240));
        println!(
            "{name}: assist {}/{} (lost {}), none {}/{}",
            on.hit_shots, on.shots, on.lost, off.hit_shots, off.shots
        );
        assert!(on.hit_shots as f32 >= on.shots as f32 * share, "{name}");
        assert_eq!(off.hit_shots, 0, "{name}");
        assert_eq!(on.lost, 0, "{name}: the magnet held");
    }
}

/// The turret magnet takes enemies only (`seat_magnet`'s team bits): a
/// friendly soldier squarely on the camera's line 40 m out is the reticle's
/// pick (a friend's, so the acquisition saw him) but never the magnet, and
/// the still stick is never pulled, FIRE held. With an enemy on the line
/// at 60 m and the friend 1.5° off in front, the enemy is the magnet
/// throughout.
#[test]
fn the_turret_magnet_never_takes_a_friendly() {
    for enemy in [false, true] {
        let (mut s, p, v) = turret_world(Slew::Exp);
        let off = if enemy { 40.0 * (1.5 * DEG).tan() } else { 0.0 };
        let friend = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(off, 0.0, 40.0)))
            .unwrap();
        let foe = enemy.then(|| {
            s.spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 60.0)))
                .unwrap()
        });
        let at = s.unit(foe.unwrap_or(friend)).unwrap().capsule().center();
        look_at(&mut s, p, v, at);
        let start = s.vehicle(v).unwrap().guns[0].as_ref().unwrap().look;
        let fire = PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        };
        for k in 0..60 {
            s.step(&[fire]);
            s.units[friend.index as usize].health.cur = 1.0e9;
            assert_ne!(
                s.magnet_at(0),
                Some(AimTarget::Unit(friend)),
                "{enemy} tick {k}"
            );
            if let Some(e) = foe {
                s.units[e.index as usize].health.cur = 1.0e9;
                assert_eq!(s.magnet_at(0), Some(AimTarget::Unit(e)), "tick {k}");
            }
        }
        if !enemy {
            assert_eq!(s.magnet_at(0), None);
            assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(friend), 1)));
            let look = s.vehicle(v).unwrap().guns[0].as_ref().unwrap().look;
            assert_eq!(look, start, "no pull without an enemy");
        }
    }
}

/// Issue #164 pin: a controller gunner in the BF1-style test turret
/// ([`turret_world`], Exp slew) holding his sticks still on a runner
/// crossing at 60 m and tapping FIRE, for 240 ticks. The idle core pin has
/// no one in a turret; this one moves when the turret magnet, lead or pull
/// does.
#[test]
fn assist_hash_pin_turret_gunner_on_a_runner() {
    // Recorded 2026-11-14 (job claude-aim-assist-164-fixup).
    const PIN: u64 = 0x856b_77eb_cb87_cb00;
    let (c, s) = cross_with(test_gun(Slew::Exp, 10.0), 60.0, 1.0, 0, (240, 240));
    println!(
        "turret pin: {}/{} shots hit, hash {:#018x}",
        c.hit_shots,
        c.shots,
        s.hash()
    );
    assert!(c.hit_shots > 0 && c.lost == 0);
    assert_eq!(s.hash(), PIN);
}

/// Zero intensity and the reach's fringe leave the pad exactly as it was;
/// the turret's own building is never the magnet or the pick; a bot in the
/// seat has no turret assist at all.
#[test]
fn zero_intensity_the_fringe_and_bots_leave_the_turret_alone() {
    for (dist, intensity) in [(60.0, 0.0), (120.5, 1.0)] {
        let (mut s, p, v) = turret_world(Slew::Exp);
        s.assist_intensity = intensity;
        let e = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.3, 0.0, dist)))
            .unwrap();
        // The centre `dist` out from the camera's eye.
        let eye = s.seat_look(p).unwrap().eye;
        let c = s.unit(e).unwrap().capsule().center();
        let to = Vec3::new(0.3, c.y, dist) - eye;
        s.units[e.index as usize].soldier.pos.z +=
            eye.z + (dist * dist - to.x * to.x - to.y * to.y).sqrt() - c.z;
        let at = s.unit(e).unwrap().capsule().center();
        look_at(&mut s, p, v, at);
        s.step(&[PadState::default()]);
        let m = AimTarget::Unit(e);
        assert_eq!(s.magnet_at(0), Some(m), "{dist}");
        for (rx, ry) in [(i16::MIN, i16::MAX), (1234, -5678)] {
            let pad = PadState {
                rx,
                ry,
                buttons: buttons::FIRE,
                ..PadState::default()
            };
            assert_eq!(
                s.seat_assisted(0, p, m, &pad, crate::TICK_DT).unwrap().0,
                pad
            );
        }
        assert_ne!(
            s.reticle_pick[0].map(|(t, _)| t),
            Some(AimTarget::Vehicle(v))
        );
    }
    let (mut s, p, _) = turret_world(Slew::Exp);
    s.units[p.index as usize].controller = Controller::Ai;
    assert!(s.gunner_turret(p).is_none());
}

/// Mounting drops what the soldier held on foot: an enemy magnet and pick
/// do not ride into a seat that takes none. In the turret the pick is the
/// seat's own.
#[test]
fn mounting_drops_the_foot_magnet_and_the_turret_writes_its_own_pick() {
    let (mut s, p, v) = turret_world(Slew::Exp);
    s.unseat_unit(p);
    let e = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 30.0)))
        .unwrap();
    s.magnet[0] = Some(AimTarget::Unit(e));
    s.reticle_pick[0] = Some((AimTarget::Unit(e), -1));
    // A gunner seat on a hover: no magnet, no pick.
    let hover = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(10.0, 0.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Gunner(0)],
    });
    assert!(s.seat_unit(p, hover, 0));
    s.step(&[PadState::default()]);
    assert_eq!((s.magnet_at(0), s.reticle_pick[0]), (None, None));
    s.unseat_unit(p);
    assert!(s.seat_unit(p, v, 0));
    let at = s.unit(e).unwrap().capsule().center();
    look_at(&mut s, p, v, at);
    s.step(&[PadState::default()]);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(e), -1)));
    assert_eq!(s.magnet_at(0), Some(AimTarget::Unit(e)));
}
