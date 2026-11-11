//! Seat guns on flat ground: a skiff-like hover whose pilot seat has no gun
//! and whose three gunner seats carry the BF1 rifle at the skiff's fire
//! points (the baked `hp_firel` / `hp_fire` / `hp_firer`, rounded) with
//! TURRET1..3's limits. STCHG checks charge-tenure reset on a real seat switch.

use crate::sim::tests::flat_ground;
use crate::sim::{buttons, Event, PadState, Sim};
use crate::unit::{UnitHandle, UnitSpec};
use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon};
use crate::vehicle::hover::tests::skiff;
use crate::vehicle::{SeatKind, SeatSetup, VehicleHandle, VehicleKind, VehicleSpec};
use crate::weapon::{HealthType, OrdnanceClass, Secondary, WeaponClass};
use glam::Vec3;

const DEG: f32 = std::f32::consts::PI / 180.0;

#[test]
fn flyer_salvo_uses_each_authored_muzzle() {
    let (mut sim, pilot, h) = world(true);
    let points = vec![
        Vec3::new(0.0, 2.0, 3.0),
        Vec3::new(2.0, -1.0, 3.0),
        Vec3::new(-2.0, -1.0, 3.0),
    ];
    sim.seat_setup[0].as_mut().unwrap().guns[0]
        .as_mut()
        .unwrap()
        .weapons[0]
        .fire_points = points.clone();
    sim.weapon_classes[0].shots_per_salvo = 3;
    sim.weapon_classes[0].salvo_count = 1;
    let vehicle = sim.vehicle_mut(h).unwrap();
    vehicle.kind = VehicleKind::Flyer;
    vehicle.flyer.phase = crate::vehicle::flyer::FlightPhase::Flying;
    assert!(sim.seat_unit(pilot, h, 0));
    sim.seat_gun(pilot.index as usize, &pad(buttons::FIRE));
    let origins: Vec<_> = sim
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Shot { origin, .. } => Some(*origin),
            _ => None,
        })
        .collect();
    let v = sim.vehicle(h).unwrap();
    let expected: Vec<_> = points.iter().map(|p| v.pos + v.rot * *p).collect();
    assert_eq!(origins, expected);
    let hash = sim.hash();
    sim.vehicle_mut(h).unwrap().guns[0]
        .as_mut()
        .unwrap()
        .weapons[0]
        .shots_fired += 1;
    assert_ne!(sim.hash(), hash);
}

#[test]
fn flyer_pilot_looks_along_untilted_body_without_accumulating_stick_or_mouse_pitch() {
    let (mut sim, pilot, h) = world(true);
    let v = sim.vehicle_mut(h).unwrap();
    v.kind = VehicleKind::Flyer;
    v.flyer.tilt_pitch = 0.2;
    v.flyer.tilt_roll = 0.3;
    v.rot = crate::vehicle::hover::quat_z(0.3) * crate::vehicle::hover::quat_x(0.2);
    assert!(sim.seat_unit(pilot, h, 0));
    let pad = PadState {
        ry: i16::MAX,
        my: PadState::quantize_look(0.5),
        ..PadState::default()
    };
    for _ in 0..10 {
        sim.seat_gun(pilot.index as usize, &pad);
    }
    let v = sim.vehicle(h).unwrap();
    let look = v.guns[0].as_ref().unwrap().look;
    assert_eq!(look, (0.0, 0.2));
    assert!((v.rot * crate::vehicle::gun::dir(look.0, look.1)).abs_diff_eq(Vec3::Z, 1e-6));
    assert!(sim.look_preview(pilot, &pad).is_none());
}

#[test]
fn flyer_pilot_and_gunner_fire_only_in_flight_outside_bf2_landing_regions() {
    use crate::vehicle::flyer::{FlightPhase, FlyerParams};
    for bf2 in [false, true] {
        for in_region in [false, true] {
            for phase in [
                FlightPhase::Landed,
                FlightPhase::Takeoff,
                FlightPhase::Flying,
                FlightPhase::Landing,
            ] {
                for seat in [0, 1] {
                    let (mut sim, pilot, h) = world(true);
                    sim.hover_params.clear();
                    sim.flyer_params = vec![Some(FlyerParams {
                        bf2,
                        ..FlyerParams::default()
                    })];
                    let v = sim.vehicle_mut(h).unwrap();
                    v.kind = VehicleKind::Flyer;
                    v.flyer.phase = phase;
                    if in_region {
                        sim.landing_regions
                            .push(crate::region::Region::sphere(Vec3::ZERO, 50.0));
                    }
                    assert!(sim.seat_unit(pilot, h, seat));
                    sim.seat_gun(pilot.index as usize, &pad(buttons::FIRE));
                    let fired = sim.events.iter().any(|e| matches!(e, Event::Shot { .. }));
                    assert_eq!(
                        fired,
                        phase == FlightPhase::Flying && (!bf2 || !in_region),
                        "bf2={bf2}, region={in_region}, phase={phase:?}, seat={seat}"
                    );
                }
            }
        }
    }
}

fn gun(fire: [f32; 3], pivot: [f32; 3], yaw: [f32; 2], pilot: bool) -> GunSetup {
    GunSetup {
        weapons: vec![GunWeapon {
            class: 0,
            infinite: true,
            reserve_clips: 0,
            fire_points: Vec::new(),
        }],
        fire_point: Vec3::from(fire),
        pivot: Vec3::from(pivot),
        eye: Vec3::new(0.0, 1.0, 2.0),
        aimer: Aimer {
            yaw: [yaw[0] * DEG, yaw[1] * DEG],
            pitch: [-45.0 * DEG, 45.0 * DEG],
            pitch_rate: 0.8,
            ..Aimer::default()
        },
        pilot,
        turn: None,
        framed: false,
        aimer_bind: glam::Quat::IDENTITY,
        gunner: None,
    }
}

/// The skiff at the origin facing +Z (yaw 0), the player 6 m ahead of its
/// nose facing it (−Z). `pilot_gun` arms the pilot seat too.
fn world(pilot_gun: bool) -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    s.hover_params = vec![Some(skiff())];
    s.add_weapon_class(WeaponClass::bf1_rifle());
    s.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true; 4],
        exit_drop: 10.0,
        ai_size: 1,
        guns: vec![
            pilot_gun.then(|| gun([0.0, 0.9, 4.2], [0.0, 0.8, 3.3], [0.0, 0.0], true)),
            Some(gun([2.3, 0.9, 0.0], [1.4, 0.8, 0.0], [65.0, 115.0], false)),
            Some(gun([0.0, 0.9, 4.2], [0.0, 0.8, 3.3], [-25.0, 25.0], false)),
            Some(gun(
                [-2.5, 0.9, 0.0],
                [-1.5, 0.8, 0.0],
                [-115.0, -65.0],
                false,
            )),
        ],
    })];
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 2.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![
            SeatKind::Pilot,
            SeatKind::Gunner(0),
            SeatKind::Gunner(1),
            SeatKind::Gunner(2),
        ],
    });
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, 6.0)))
        .unwrap();
    (s, p, v)
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..PadState::default()
    }
}

/// Press and release `b`.
fn tap(s: &mut Sim, b: u32) {
    s.step(&[pad(b)]);
    s.step(&[pad(0)]);
}

fn seat(s: &Sim, p: UnitHandle) -> SeatKind {
    s.seat_of(p).expect("seated").1
}

#[test]
fn switch_walks_the_ring_to_the_next_free_seat() {
    let (mut s, p, _) = world(false);
    tap(&mut s, buttons::ACTION);
    assert_eq!(seat(&s, p), SeatKind::Pilot);
    for want in [
        SeatKind::Gunner(0),
        SeatKind::Gunner(1),
        SeatKind::Gunner(2),
        SeatKind::Pilot,
    ] {
        s.units[p.index as usize].ai.charge_until = 999;
        tap(&mut s, buttons::SWITCH_WEAPON);
        assert_eq!(seat(&s, p), want);
        assert_eq!(
            s.unit(p).unwrap().ai.charge_until,
            0,
            "seat switch charge tenure"
        );
    }
    // A held button is one edge.
    s.step(&[pad(buttons::SWITCH_WEAPON)]);
    s.step(&[pad(buttons::SWITCH_WEAPON)]);
    assert_eq!(seat(&s, p), SeatKind::Gunner(0));
}

#[test]
fn nobody_human_aboard_leaves_the_guns_unmade() {
    let (mut s, _, v) = world(true);
    let h0 = s.hash();
    s.step(&[pad(0)]);
    assert!(s.vehicle(v).unwrap().guns.is_empty());
    let (mut t, _, _) = world(false);
    t.step(&[pad(0)]);
    assert_eq!(s.hash(), t.hash(), "setup alone does not reach the hash");
    assert_ne!(h0, s.hash());
}

#[test]
fn the_nose_gunner_fires_from_its_fire_point_along_its_aimer() {
    let (mut s, p, v) = world(false);
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_WEAPON);
    assert_eq!(seat(&s, p), SeatKind::Gunner(1));
    s.events.clear();
    s.step(&[pad(buttons::FIRE)]);
    let shot = s
        .events
        .iter()
        .find_map(|e| match e {
            Event::Shot {
                unit, origin, dir, ..
            } if *unit == p => Some((*origin, *dir)),
            _ => None,
        })
        .expect("a shot");
    let veh = s.vehicle(v).unwrap();
    let fp = veh.pos + veh.rot * Vec3::new(0.0, 0.9, 4.2);
    assert!(shot.0.distance(fp) < 0.2, "origin {:?} vs {fp:?}", shot.0);
    assert!(shot.1.z > 0.9, "straight ahead: {:?}", shot.1);
    assert!(!s.bolts.is_empty());
}

#[test]
fn the_aimer_follows_the_look_at_its_rate_and_stops_at_its_limit() {
    let (mut s, _, v) = world(false);
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_WEAPON);
    let right = PadState {
        rx: PadState::quantize(1.0),
        ..PadState::default()
    };
    s.step(&[right]);
    let g = s.vehicle(v).unwrap().guns[2].clone().unwrap();
    assert!(g.look.0 < 0.0, "stick right lowers yaw: {:?}", g.look);
    assert!(g.aim.0 <= 0.0 && g.aim.0 >= -2.0 * crate::TICK_DT - 1e-6);
    for _ in 0..60 {
        s.step(&[right]);
    }
    let g = s.vehicle(v).unwrap().guns[2].clone().unwrap();
    assert!((g.look.0 + 25.0 * DEG).abs() < 1e-5, "look clamped");
    assert!(
        (g.aim.0 + 25.0 * DEG).abs() < 1e-3,
        "aim clamped: {}",
        g.aim.0
    );
}

fn mouse(yaw: f32, pitch: f32) -> PadState {
    PadState {
        mx: PadState::quantize_look(f64::from(yaw)),
        my: PadState::quantize_look(f64::from(pitch)),
        buttons: buttons::MOUSE_AIM,
        ..PadState::default()
    }
}

#[test]
fn a_gunners_mouse_moves_the_look_by_its_angle_inside_the_limits() {
    let (mut s, p, v) = world(false);
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_WEAPON);
    let look = |s: &Sim| s.vehicle(v).unwrap().guns[2].clone().unwrap().look;
    let before = look(&s);
    let pad = mouse(0.3, 0.1);
    let preview = s.look_preview(p, &pad);
    s.step(&[pad]);
    let after = look(&s);
    let (dy, dp) = pad.look();
    assert!(
        (before.0 - dy - after.0).abs() < 1e-6,
        "{before:?} {after:?}"
    );
    assert!((before.1 + dp - after.1).abs() < 1e-6);
    assert_eq!(
        preview,
        Some(crate::sim::LookPreview::Seat {
            vehicle: v.index as usize,
            seat: 2,
            look: after,
            walker: false,
        })
    );
    s.step(&[mouse(-3.0, 3.0)]);
    assert_eq!(look(&s), (25.0 * DEG, 45.0 * DEG));
}

#[test]
fn a_turrets_mouse_look_wraps_and_holds_its_pitch_limits() {
    let (mut s, v) = dish_world();
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, 3.0)))
        .unwrap();
    assert!(s.enter_vehicle(p, v, 0));
    let look = |s: &Sim| s.vehicle(v).unwrap().guns[0].clone().unwrap().look;
    for _ in 0..5 {
        s.step(&[mouse(1.0, 0.0)]);
    }
    let l = look(&s);
    assert!(l.0.abs() <= std::f32::consts::PI, "wrapped {l:?}");
    assert!((crate::vehicle::gun::wrap(l.0 + 5.0)).abs() < 1e-4, "{l:?}");
    s.step(&[mouse(0.0, -2.0)]);
    assert!((look(&s).1 + 35.0 * DEG).abs() < 1e-5);
    s.step(&[mouse(0.0, 2.0)]);
    assert!((look(&s).1 - 10.0 * DEG).abs() < 1e-5);
}

#[test]
fn a_side_gun_rests_across_the_hull() {
    let (mut s, _, v) = world(false);
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    s.step(&[pad(buttons::FIRE)]);
    let g = s.vehicle(v).unwrap().guns[1].clone().unwrap();
    assert!((g.aim.0 - 90.0 * DEG).abs() < 1e-2, "{:?}", g.aim);
    let d = s.bolts.last().expect("a bolt").vel.normalize();
    assert!(d.x > 0.9, "the left gun fires out along +x: {d:?}");
}

#[test]
fn the_pilot_aims_straight_ahead() {
    let (mut s, p, v) = world(true);
    tap(&mut s, buttons::ACTION);
    assert_eq!(seat(&s, p), SeatKind::Pilot);
    let stick = PadState {
        rx: PadState::quantize(1.0),
        ry: PadState::quantize(1.0),
        ..PadState::default()
    };
    for _ in 0..10 {
        s.step(&[stick]);
    }
    let g = s.vehicle(v).unwrap().guns[0].clone().unwrap();
    assert_eq!(g.aim.0, 0.0);
    assert_eq!(g.look.0, 0.0);
    assert!(g.look.1 > 0.0, "the look stick pitches the aim");
}

#[test]
fn a_gunners_bolt_hits_the_soldier_it_aims_at() {
    let (mut s, p, v) = world(false);
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 30.0)))
        .unwrap();
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_WEAPON);
    let hp = s.unit(enemy).unwrap().health.cur;
    // Aim down onto the soldier's chest, then fire.
    let down = PadState {
        ry: PadState::quantize(-1.0),
        ..PadState::default()
    };
    let veh = s.vehicle(v).unwrap();
    let eye = veh.pos + Vec3::new(0.0, 0.8, 3.3) + Vec3::new(0.0, 1.0, 2.0);
    let chest = Vec3::new(0.0, 1.0, 30.0);
    let want = ((chest - eye).y / (chest - eye).length()).asin();
    while s
        .vehicle(v)
        .unwrap()
        .guns
        .get(2)
        .and_then(|g| g.as_ref())
        .is_none_or(|g| g.look.1 > want)
    {
        s.step(&[down]);
    }
    for _ in 0..30 {
        s.step(&[pad(buttons::FIRE)]);
    }
    assert!(s.seat_of(p).is_some());
    assert!(
        s.unit(enemy).is_none_or(|u| u.health.cur < hp),
        "the enemy took the gunner's bolts"
    );
}

#[test]
fn a_seat_gun_rides_its_tracer_on_every_period_th_round() {
    let (mut s, p, v) = world(false);
    let mut chaingun = WeaponClass::bf1_rifle();
    chaingun.name = "test_chaingun".into();
    chaingun.secondary = Some(Secondary {
        name: "test_chaingun_tracer".into(),
        ordnance: OrdnanceClass {
            velocity: 200.0,
            life_span: 2.0,
            max_damage: 0.0,
            ..OrdnanceClass::default()
        },
        period: 2,
        index: usize::MAX,
    });
    let armed = s.add_weapon_class(chaingun);
    let tracer = s.weapon_classes[armed].secondary.as_ref().unwrap().index;
    s.weapon_classes[0] = s.weapon_classes[armed].clone();
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    assert_eq!(seat(&s, p), SeatKind::Gunner(0));
    let mut rounds = 0u32;
    for _ in 0..30 {
        s.step(&[pad(buttons::FIRE)]);
        rounds += s
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Shot {
                        weapon_class: 0,
                        ..
                    }
                )
            })
            .count() as u32;
    }
    assert!(rounds >= 4, "the gunner fired {rounds} rounds");
    let gun = s.vehicle(v).unwrap().guns[1].as_ref().unwrap();
    assert_eq!(gun.weapons[0].secondary_rounds, rounds);
    let tracers = s.bolts.iter().filter(|b| b.weapon_class == tracer).count() as u32;
    assert_eq!(tracers, rounds.div_ceil(2));
}

/// Card VTAI: a bot in the nose gunner's seat (a human pilots) turns its
/// look toward the enemy it sees and fires, every shot at a perceived
/// target; a wall between them means no shot at all.
#[test]
fn a_bot_gunner_aims_and_fires_at_what_it_sees() {
    let run = |wall: bool| {
        let (mut s, _, v) = world(false);
        if wall {
            s.collision.push_body(crate::collision::Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 5.0, 15.0)),
                crate::collision::Shape::Box {
                    half: Vec3::new(20.0, 5.0, 0.5),
                },
            ));
        }
        tap(&mut s, buttons::ACTION);
        let b = s
            .spawn(&UnitSpec {
                controller: crate::unit::Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(3.0, 0.0, 0.0))
            })
            .unwrap();
        assert!(s.enter_vehicle(b, v, 2));
        let enemy = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(5.0, 0.0, 30.0)))
            .unwrap();
        let hp = s.unit(enemy).unwrap().health.cur;
        // The peak look yaw: once the enemy is down the look goes back to rest.
        let mut look = 0.0f32;
        for _ in 0..300 {
            s.step(&[pad(0)]);
            if let Some(Some(g)) = s.vehicle(v).unwrap().guns.get(2) {
                look = look.max(g.look.0);
            }
        }
        assert_eq!(s.nav_stats.unperceived_shots, 0);
        assert!(s.unit(b).unwrap().mount.is_some(), "the gunner stays");
        let hurt = s.unit(enemy).is_none_or(|u| u.health.cur < hp);
        (s.nav_stats.seat_shots, look, hurt)
    };
    let (shots, look, hurt) = run(false);
    // Card CLSE: a bot that rides with a human aims with no error (BF2,
    // `analysis/re-soldier-ai.md` 17.2): the enemy is down in 5 shots.
    assert!(shots >= 4, "{shots} seat shots");
    assert!(look > 5.0 * DEG, "the look turned toward +x: {look}");
    assert!(hurt, "the bolts reached the enemy");
    let (shots, _, hurt) = run(true);
    assert_eq!(shots, 0, "no shot through the wall");
    assert!(!hurt);
}

/// Card VTURL: a BF2 Hoth dish turret (`YawLimits -180 180`, `PitchLimits
/// -10 35`, speeds 1, `AimerYawLimits 0 0`, `AimerPitchLimits -15 40`) at
/// the origin facing +Z, its gunner seat armed with the rifle.
fn dish_world() -> (Sim, VehicleHandle) {
    use crate::vehicle::gun::{Slew, Turn};
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    s.hover_params = vec![None];
    s.add_weapon_class(WeaponClass::bf1_rifle());
    let mut g = gun([0.0, 2.0, 2.0], [0.0, 2.0, 0.0], [0.0, 0.0], false);
    g.aimer.pitch = [-15.0 * DEG, 40.0 * DEG];
    g.turn = Some(Turn::from_class(
        Some([-180.0, 180.0]),
        Some([-10.0, 35.0]),
        (1.0, 1.0),
        (10.0, 10.0),
        Slew::Linear,
    ));
    g.framed = true;
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
    (s, v)
}

/// The traced fault (card VTURL): the look held at -180 and the aimer
/// walking back through 0. Now a held stick turns the dish through the
/// seam and on, at 1 rad/s, and the aimer (no yaw of its own) points where
/// the turret does on every tick.
#[test]
fn a_full_circle_turret_turns_past_180_and_its_gun_follows() {
    let (mut s, v) = dish_world();
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, 3.0)))
        .unwrap();
    assert!(s.enter_vehicle(p, v, 0));
    let mut prev: Option<(f32, f32)> = None;
    let mut turned = 0.0;
    let mut worst = 0.0f32;
    for _ in 0..240 {
        s.step(&[PadState {
            rx: PadState::quantize(1.0),
            ..PadState::default()
        }]);
        let g = s.vehicle(v).unwrap().guns[0].clone().unwrap();
        if let Some(l) = prev {
            turned += crate::vehicle::gun::wrap(g.look.0 - l.0);
        }
        prev = Some(g.look);
        worst = worst.max(crate::vehicle::gun::wrap(g.aim.0 - g.look.0).abs());
    }
    assert!(turned < -7.0, "turned {turned} rad in 8 s");
    assert!(
        worst < 1e-3,
        "the aimer left the turret's heading by {worst}"
    );
    // Stick down: the dish's nose goes DOWN to 35 degrees (the PS2's turret
    // pitch is + = nose down), up only to 10.
    for (ry, want) in [(-1.0, -35.0), (1.0, 10.0)] {
        for _ in 0..90 {
            s.step(&[PadState {
                ry: PadState::quantize(ry),
                ..PadState::default()
            }]);
        }
        let g = s.vehicle(v).unwrap().guns[0].clone().unwrap();
        assert!((g.look.1 - want * DEG).abs() < 1e-4, "{ry}: {:?}", g.look);
    }
}

/// Card TURR: BF2 Mygeeto's recoilless turret (`tur_bldg_recoilless_lg`):
/// the pivot `aimer_y` at the origin, `hp_fire1` 3.39 m over it and 1.28 m
/// to the side, `EyePointOffset 1.0 4.2 -8.3`, `AimerYawLimits 0 0`,
/// `AimerPitchLimits -30 20`, `PitchLimits -30 20`, turned at 1.5 rad/s;
/// at the origin facing +Z. `building`: the turret's own building as a map
/// body round the camera's eye (instance 7, the turret's), as the bake
/// builds it.
fn recoilless_gun() -> GunSetup {
    use crate::vehicle::gun::{Slew, Turn};
    let mut g = gun([1.281, 3.388, 3.372], [0.0, 0.0, 0.0], [0.0, 0.0], false);
    g.eye = Vec3::new(1.0, 4.2, -8.3);
    g.aimer.pitch = [-30.0 * DEG, 20.0 * DEG];
    g.aimer.pitch_rate = 1.0;
    g.turn = Some(Turn::from_class(
        Some([-180.0, 180.0]),
        Some([-30.0, 20.0]),
        (1.5, 1.0),
        (10.0, 10.0),
        Slew::Linear,
    ));
    g.framed = true;
    g
}

fn recoilless_building() -> crate::collision::Body {
    use crate::collision::{Body, Shape};
    let mut b = Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 3.0, -5.0)),
        Shape::Box {
            half: Vec3::new(3.0, 3.0, 6.0),
        },
    );
    b.instance = Some(7);
    b
}

fn recoilless_world(building: bool) -> (Sim, VehicleHandle) {
    let (mut s, v) = dish_world();
    s.seat_setup[0].as_mut().unwrap().guns[0] = Some(recoilless_gun());
    if building {
        s.collision =
            crate::collision::CollisionWorld::new(vec![recoilless_building()], Vec::new());
        s.vehicle_mut(v).unwrap().instance = Some(7);
    }
    (s, v)
}

/// How far `p` is from the barrel's line (from the muzzle along the barrel).
fn off_the_barrel(s: &Sim, u: UnitHandle, p: Vec3) -> f32 {
    let l = s.seat_look(u).unwrap();
    let w = p - l.muzzle;
    (w - l.barrel * w.dot(l.barrel)).length()
}

/// Card TURR: the camera ray of a building turret passes its own building
/// (the PS2's excludes the turret's owner entity, B1 `FUN_00223850`
/// B1:138064, BF2 `FUN_0026cea0` B2:189959): a human looking level from an
/// eye inside his turret's building lays the barrel on the ray's far end,
/// 512 m out. The same building as no one's map body (instance 8) stops the
/// ray at the eye and the barrel runs to its limit. And the aimer lays the
/// BARREL's line on that point, not the pivot's (PS2 RAM, Mygeeto: 0.004 m
/// off the barrels' line, 3.5 m off the pivot's): it passes 0.281 m beside
/// it, the fire point's offset from the eye's line (1.281 against 1.0 m),
/// which an aimer with no yaw of its own keeps; none above or below.
#[test]
fn a_turrets_camera_ray_passes_its_own_building_and_its_barrel_meets_the_point() {
    for own in [true, false] {
        let (mut s, v) = recoilless_world(true);
        if !own {
            s.vehicle_mut(v).unwrap().instance = Some(8);
        }
        let p = s
            .spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, 4.0)))
            .unwrap();
        assert!(s.enter_vehicle(p, v, 0));
        for _ in 0..90 {
            s.step(&[PadState::default()]);
        }
        let g = s.vehicle(v).unwrap().guns[0].clone().unwrap();
        assert_eq!(g.look, (0.0, 0.0), "the look rests level");
        let far = Vec3::new(1.0, 4.2, -8.3) + Vec3::Z * crate::vehicle::gun::AIM_RAY;
        let off = off_the_barrel(&s, p, far);
        if own {
            assert!(
                (off - 0.281).abs() < 0.01,
                "the barrel's line passes {off} m off the point"
            );
            assert!(g.aim.1.abs() < 1.0 * DEG, "aim {:?}", g.aim);
        } else {
            assert!(off > 10.0, "{off}");
            assert!(
                (g.aim.1 - 20.0 * DEG).abs() < 1e-3 || (g.aim.1 + 30.0 * DEG).abs() < 1e-3,
                "the aimer ran to a limit: {:?}",
                g.aim
            );
        }
    }
}

/// Card TURR: a building turret's barrel hangs off its aimer at its bind
/// offset (re-turret-draw 2.3, `lead_tmp/vturd/replay_aimer.py` A3): the
/// muzzle at rest IS the fire point, 1.28 m to the side - not turned onto
/// the pivot's axis by its heading, as a vehicle's side gun is.
#[test]
fn a_building_turrets_muzzle_rests_at_its_fire_point() {
    let g = recoilless_gun();
    assert!(g.muzzle((0.0, 0.0)).distance(g.fire_point) < 1e-5);
    let m = g.muzzle((90.0 * DEG, 0.0));
    assert!(m.distance(Vec3::new(3.372, 3.388, -1.281)) < 1e-3, "{m}");
}

/// STCHG: the shared AI policy never substitutes its sampled deadline for a
/// human seat trigger. A human's held charge fires only on their release.
#[test]
fn human_seat_charge_keeps_the_human_release_edge() {
    let (mut s, p, v) = world(true);
    s.weapon_classes[0].max_pressed_time = 1.5;
    s.weapon_classes[0].trigger_single = true;
    tap(&mut s, buttons::ACTION);
    for _ in 0..10 {
        s.step(&[pad(buttons::FIRE)]);
        let w = &s.vehicle(v).unwrap().guns[0].as_ref().unwrap().weapons[0];
        assert!(w.fire_cmd > 0);
        assert_eq!(w.shots_fired, 0);
        assert_eq!(s.unit(p).unwrap().ai.charge_until, 0);
    }
    s.step(&[pad(0)]);
    let w = &s.vehicle(v).unwrap().guns[0].as_ref().unwrap().weapons[0];
    assert_eq!(w.fire_cmd, -1);
    assert_eq!(w.shots_fired, 1);
    assert!(s
        .events
        .iter()
        .any(|e| matches!(e, Event::Shot { unit, weapon_class: 0, .. } if *unit == p)));
}

/// Playtest 2026-10-28 (BF2 Kamino, `analysis/re-turret-collision.md` 3.1,
/// 3.4, 4.1): a building turret's building bodies are its own once bound
/// (`Sim::bind_building`): a round that meets one strikes the turret, whose
/// pool takes the ordnance's building scale; a blast beside it measures it
/// past its own bodies; at 0 it is destroyed with its gunner; and its own
/// gunner's round passes it (card TCOL). Unbound (the port before),
/// the round stops on a plain wall and the pool keeps its health.
#[test]
fn a_bound_building_takes_rounds_and_blasts_and_its_gunner_dies_with_it() {
    use crate::collision::HullFilter;
    use crate::projectile::{Bolt, Hit, Shooter, Target};
    for bound in [true, false] {
        let (mut s, v) = recoilless_world(true);
        if bound {
            s.bind_building(v, 7);
        }
        s.vehicle_mut(v).unwrap().aim_sphere = Some((Vec3::new(0.0, 3.0, -5.0), 6.0));
        s.weapon_classes[0].ordnance.max_damage = 100.0;
        s.weapon_classes[0].ordnance.falloff = None;
        s.weapon_classes[0].ordnance.scales = [1.0, 1.0, 1.0, 0.5, 0.25, 0.25, 0.25];
        let gunner = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 4.0)))
            .unwrap();
        assert!(s.seat_unit(gunner, v, 0));
        let shooter = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 40.0)))
            .unwrap();
        let fire = |s: &Sim, owner, hulls| {
            Bolt::fire(
                Vec3::new(0.0, 1.0, 30.0),
                -Vec3::Z,
                300.0,
                1.0,
                Shooter {
                    owner,
                    team: 1,
                    weapon_class: 0,
                    is_player: false,
                },
                s.heightfield.as_ref(),
                None,
                &s.collision,
                hulls,
            )
        };
        let b = fire(&s, shooter, HullFilter::All);
        let want = if bound {
            Hit::Target(Target::Vehicle(v))
        } else {
            Hit::World
        };
        assert_eq!(b.hit, want, "bound {bound}");
        // The gunner's own round: it passes his own building (card TCOL,
        // re-turret-collision 6.2: the PS2's ray skips the owner entity),
        // and stops on it as a plain wall while it is no turret's.
        assert_eq!(
            fire(&s, gunner, HullFilter::except(Some(v))).hit,
            if bound { Hit::None } else { Hit::World }
        );
        s.bolts.push(b);
        for _ in 0..3 {
            s.step(&[]);
        }
        let hp = s.vehicle(v).unwrap().health.cur;
        assert_eq!(hp, if bound { 1000.0 - 25.0 } else { 1000.0 });
        if !bound {
            continue;
        }
        // A blast just off the building's side wall, the building's centre
        // 3.5 m in past its own bodies: inside DamageRadiusInner + w / 2.
        let e = crate::weapon::ExplosionClass {
            damage: 400.0,
            damage_inner: 2.0,
            damage_outer: 4.0,
            scales: [1.0, 1.0, 1.0, 0.0, 0.25, 0.25, 0.25],
            ..Default::default()
        };
        s.explode(&e, Vec3::new(3.5, 3.0, -5.0), Some(shooter), Some(1));
        assert_eq!(s.vehicle(v).unwrap().health.cur, hp - 100.0);
        // At 0 the turret is destroyed and its seated gunner killed by the
        // shooter (`destroy_vehicle`); its bodies are a wall again.
        s.vehicle_mut(v).unwrap().health.cur = 1.0;
        s.explode(&e, Vec3::new(3.5, 3.0, -5.0), Some(shooter), Some(1));
        assert!(s.vehicle(v).is_none());
        assert!(s.unit(gunner).is_none_or(|g| !g.alive));
        assert!(s.events.iter().any(|e| matches!(e,
            Event::Kill { shooter: k, victim } if *k == shooter && *victim == gunner)));
        assert_eq!(fire(&s, shooter, HullFilter::All).hit, Hit::World);
    }
}

#[test]
fn a_player_cycles_ai_held_seats_and_keeps_every_displaced_bot_aboard() {
    let (mut s, p, v) = world(true);
    assert!(s.seat_unit(p, v, 0));
    let mut bots = Vec::new();
    for seat in 1..4 {
        let h = s
            .spawn(&UnitSpec {
                controller: crate::unit::Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        assert!(s.seat_unit(h, v, seat));
        bots.push(h);
    }
    let entropy = s.rng.state();
    let bail: Vec<_> = bots
        .iter()
        .map(|h| s.unit(*h).unwrap().ai.ride.unwrap().bail_draw)
        .collect();
    let mut pads = vec![PadState::default(); s.units.len()];
    pads[p.index as usize] = pad(buttons::SWITCH_WEAPON);
    for seat in [1, 2, 3, 0] {
        s.switch_pass(&pads);
        assert_eq!(s.unit(p).unwrap().mount.unwrap().seat, seat);
        let seats = &s.vehicle(v).unwrap().seats;
        assert!(seats.iter().all(|seat| seat.occupant.is_some()));
        for (i, bot) in bots.iter().enumerate() {
            let u = s.unit(*bot).unwrap();
            let m = u.mount.unwrap();
            assert_eq!(m.vehicle, v);
            assert_eq!(seats[m.seat as usize].occupant, Some(*bot));
            assert_eq!(u.ai.ride.unwrap().bail_draw, bail[i]);
        }
        assert_eq!(s.rng.state(), entropy);
    }
}

#[test]
fn seat_switch_skips_other_players_enemies_and_nonboardable_seats() {
    let (mut s, p, v) = world(true);
    assert!(s.seat_unit(p, v, 0));
    let human = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    assert!(s.seat_unit(human, v, 1));
    let friend = s
        .spawn(&UnitSpec {
            controller: crate::unit::Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    assert!(s.seat_unit(friend, v, 2));
    let enemy = s
        .spawn(&UnitSpec {
            controller: crate::unit::Controller::Ai,
            ..UnitSpec::trooper(1, Vec3::ZERO)
        })
        .unwrap();
    assert!(s.seat_unit(enemy, v, 3));
    assert_eq!(s.next_seat(p), Some(2));
    s.seat_setup[0].as_mut().unwrap().boardable[2] = false;
    assert_eq!(s.next_seat(p), None);
    assert_eq!(s.unit(human).unwrap().mount.unwrap().seat, 1);
    assert_eq!(s.unit(enemy).unwrap().mount.unwrap().seat, 3);
}

#[test]
fn wheel_up_cycles_backward_and_passengers_do_not_cycle_to_other_passengers() {
    let (mut s, p, v) = world(true);
    assert!(s.seat_unit(p, v, 0));
    let mut pads = vec![PadState::default(); s.units.len()];
    pads[p.index as usize] = pad(buttons::SWITCH_WEAPON | buttons::SPAWN_CLASS_PREV);
    s.switch_pass(&pads);
    assert_eq!(s.unit(p).unwrap().mount.unwrap().seat, 3);
    s.vehicle_mut(v).unwrap().seats[2].kind = SeatKind::Passenger(0);
    s.vehicle_mut(v).unwrap().seats[3].kind = SeatKind::Passenger(1);
    assert_eq!(s.next_seat(p), Some(0));
}

#[test]
fn audit_boarding_does_not_reinterpret_a_cloak_press() {
    use crate::unit::Controller;
    let (mut sim, _, vehicle) = world(true);
    let cloak = sim.add_weapon_class(WeaponClass {
        cloak: Some(crate::cloak::CloakClass {
            energy_cost: 40.0,
            energy_drain: 17.0,
            delay: 0.3,
            linger: 1.0,
            moving: 1.0,
            still: 1.0,
            fast_speed: 9.5,
            slow_speed: 6.5,
            fade_in: 0.6,
            fade_out: 0.9,
            flicker: 0.6,
            flicker_interval: [5.0, 10.0],
        }),
        ..WeaponClass::default()
    });
    let actor = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            weapon_class: cloak,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 2.0, 0.0))
        })
        .unwrap();
    sim.unit_mut(actor).unwrap().soldier.energy = 100.0;
    assert!(sim.unit(actor).unwrap().mount.is_none());
    assert!(sim.unit(actor).unwrap().ai.target.is_none());
    sim.add_cp(
        crate::sim::tests::sphere_cp("far", Vec3::new(0.0, 0.0, 100.0), 8.0, true),
        Some(1),
    );
    let u = sim.unit_mut(actor).unwrap();
    u.ai.target_cp = Some(0);
    u.ai.next_think = 100;
    u.ai.ride = Some(crate::ai::RideState {
        board: Some(vehicle),
        ..Default::default()
    });
    // The whole tick computes a legitimate cloak FIRE while still on foot,
    // boards through ai_use_pass, then dispatches weapons in the new seat.
    sim.step(&[PadState::default()]);
    assert_eq!(sim.unit(actor).unwrap().mount.unwrap().vehicle, vehicle);
    assert!(
        !sim.events
            .iter()
            .any(|e| matches!(e, Event::Shot { unit, .. } if *unit == actor)),
        "the foot cloak press became a seat shot"
    );
    assert_eq!(sim.nav_stats.unperceived_shots, 0);
}

#[test]
fn audit_simultaneous_expired_rounds_keep_distinct_presentation_endpoints() {
    let (mut sim, player, vehicle) = world(true);
    assert!(sim.seat_unit(player, vehicle, 0));
    let class = &mut sim.weapon_classes[0];
    class.shots_per_salvo = 2;
    class.pitch_spread = 0.0;
    class.yaw_spread = 0.0;
    class.ordnance.life_span = crate::TICK_DT * 0.25;
    let (speed, life) = (class.ordnance.velocity, class.ordnance.life_span);
    let mut witnessed = false;
    for _ in 0..30 {
        sim.step(&[pad(buttons::FIRE)]);
        let shots: Vec<_> = sim
            .events
            .iter()
            .enumerate()
            .filter_map(|(event, e)| match e {
                Event::Shot { origin, dir, .. } => Some((event, *origin, *dir)),
                _ => None,
            })
            .collect();
        if shots.is_empty() {
            continue;
        }
        assert_eq!(
            shots.len(),
            2,
            "the real weapon emits its two simultaneous rounds"
        );
        assert_eq!(sim.shot_flights.len(), 2);
        assert!(
            sim.bolts.is_empty(),
            "both rounds expired inside their first tick"
        );
        assert_ne!(sim.shot_flights[0].shot, sim.shot_flights[1].shot);
        for (event, origin, dir) in shots {
            let flight = sim
                .shot_flights
                .iter()
                .find(|f| f.shot.event == event)
                .unwrap();
            assert_eq!(flight.shot.tick, sim.tick - 1);
            assert!(flight.end.abs_diff_eq(origin + dir * (speed * life), 1e-4));
        }
        // Presentation annotations do not change the deterministic game state.
        let hash = sim.hash();
        sim.shot_flights[0].end += Vec3::X * 100.0;
        assert_eq!(sim.hash(), hash);
        let old_tick = sim.tick - 1;
        sim.step(&[pad(0)]);
        assert!(sim.shot_flights.iter().all(|f| f.shot.tick != old_tick));
        witnessed = true;
        break;
    }
    assert!(witnessed, "the mounted weapon never emitted a salvo");
}

#[test]
fn a_mounted_harpoon_latches_and_releases_on_range_or_owner_loss() {
    use crate::collision::Shape;
    use crate::vehicle::tow::{TowClass, TowFrames, TowPrimitive};
    use crate::vehicle::Vehicle;
    use glam::Mat4;
    use std::sync::Arc;

    struct Leg;
    impl TowFrames for Leg {
        fn primitives(&self, walker: &Vehicle) -> Vec<TowPrimitive> {
            if walker.kind != VehicleKind::Walker {
                return Vec::new();
            }
            vec![TowPrimitive {
                frame: Mat4::from_translation(walker.pos),
                shape: Shape::Sphere { radius: 1.0 },
                offset: Vec3::ZERO,
                leg: Some(0),
            }]
        }
    }
    let setup = || {
        let (mut sim, player, craft) = world(true);
        sim.weapon_classes[0].ordnance.tow = Some(TowClass {
            initial_length: 60.0,
            length: 60.0,
        });
        sim.weapon_classes[0].ordnance.velocity = 250.0;
        sim.tow_frames = Some(Arc::new(Leg));
        assert!(sim.seat_unit(player, craft, 0));
        let target = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Walker,
            class: 1,
            home_team: Some(1),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(0.0, 2.9, 20.0),
            yaw: 0.0,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![],
        });
        (sim, player, craft, target)
    };
    let (mut sim, player, craft, target) = setup();
    sim.seat_gun(player.index as usize, &pad(buttons::FIRE));
    assert_eq!(sim.tow_cables.len(), 1);
    assert!(sim.bolts.is_empty());
    sim.fly_tow_cables(crate::TICK_DT);
    let mid = sim.tow_cables[0].points[0];
    assert!(mid.z < 19.0, "{mid:?}");
    sim.fly_tow_cables(crate::TICK_DT);
    let anchor = sim.tow_cables[0].points[0];
    assert!(
        (anchor.distance(Vec3::new(0.0, 2.9, 20.0)) - 1.0).abs() < 0.001 && anchor.z < 19.01,
        "{anchor:?}"
    );
    sim.vehicle_mut(target).unwrap().pos.x += 3.0;
    sim.fly_tow_cables(crate::TICK_DT);
    assert!(sim.tow_cables[0].points[0].abs_diff_eq(anchor + Vec3::X * 3.0, 0.001));
    sim.vehicle_mut(craft).unwrap().pos.z -= 70.0;
    sim.fly_tow_cables(crate::TICK_DT);
    assert!(sim.tow_cables.is_empty());
    assert_eq!(sim.vehicle(target).unwrap().health.cur, 1000.0);

    let (mut sim, player, _, _) = setup();
    sim.seat_gun(player.index as usize, &pad(buttons::FIRE));
    sim.unseat_unit(player);
    sim.fly_tow_cables(crate::TICK_DT);
    assert!(sim.tow_cables.is_empty(), "the firing seat was left");

    let (mut sim, player, _, target) = setup();
    sim.seat_gun(player.index as usize, &pad(buttons::FIRE));
    for _ in 0..2 {
        sim.fly_tow_cables(crate::TICK_DT);
    }
    assert_eq!(sim.tow_cables.len(), 1);
    sim.destroy_vehicle(target, None);
    sim.fly_tow_cables(crate::TICK_DT);
    assert!(sim.tow_cables.is_empty(), "the latched vehicle is gone");
}

fn fire_seat_gun(reserve: Option<u32>, ticks: u32) -> (Vec<u64>, Vec<u64>) {
    let (mut s, p, v) = world(false);
    s.weapon_classes[0] = WeaponClass {
        rounds_per_clip: 6,
        reload_time: 5.0,
        shot_delay: [0.33, 0.33],
        trigger_single: false,
        ..WeaponClass::bf1_rifle()
    };
    let class = s.vehicle(v).unwrap().class as usize;
    for g in s.seat_setup[class]
        .as_mut()
        .unwrap()
        .guns
        .iter_mut()
        .flatten()
    {
        g.weapons[0].infinite = reserve.is_none();
        g.weapons[0].reserve_clips = reserve.unwrap_or(0);
    }
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_WEAPON);
    assert_eq!(seat(&s, p), SeatKind::Gunner(1));
    let (mut shots, mut reloads) = (Vec::new(), Vec::new());
    for _ in 0..ticks {
        s.events.clear();
        s.step(&[pad(buttons::FIRE)]);
        for e in &s.events {
            match e {
                Event::Shot { unit, .. } if *unit == p => shots.push(s.tick),
                Event::Reload { unit, .. } if *unit == p => reloads.push(s.tick),
                _ => {}
            }
        }
    }
    (shots, reloads)
}

#[test]
fn a_seat_gun_with_an_endless_reserve_still_empties_its_clip_and_reloads() {
    let (shots, reloads) = fire_seat_gun(None, 30 * 14);
    assert!(shots.len() >= 12, "it keeps firing: {shots:?}");
    assert_eq!(reloads.len(), shots.len() / 6);
    let cycle = |a: usize, b: usize| shots[b] - shots[a];
    assert!(cycle(0, 5) <= 60, "six rounds at the refire: {shots:?}");
    let gap = cycle(5, 6);
    assert!(
        (150..=165).contains(&gap),
        "the reload holds the seventh round for 5 s: {gap}"
    );
}

#[test]
fn a_seat_gun_with_counted_clips_stops_when_they_are_spent() {
    let (shots, reloads) = fire_seat_gun(Some(1), 30 * 20);
    assert_eq!(shots.len(), 12, "the loaded clip and one more: {shots:?}");
    assert_eq!(reloads.len(), 1);
}
