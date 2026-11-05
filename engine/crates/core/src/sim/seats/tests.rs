//! Enter, drive, exit on flat ground with the TAT1 skiff's hover numbers.

use super::{driver_input, EXIT_RING, MOVE_FLOOR};
use crate::sim::tests::flat_ground;
use crate::sim::{buttons, PadState, Sim};
use crate::unit::{Controller, UnitHandle, UnitSpec};
use crate::vehicle::hover::tests::skiff;
use crate::vehicle::{SeatKind, SeatSetup, VehicleHandle, VehicleKind, VehicleSpec};
use crate::weapon::HealthType;
use glam::Vec3;

const R: f32 = 5.0;

/// A skiff-like hover (pilot + 3 gunners) `at`, facing −Z (yaw π), on flat
/// ground; the player at the origin facing −Z.
fn world(at: Vec3, home_team: Option<u8>, neutral: bool) -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    s.hover_params = vec![Some(skiff())];
    s.seat_setup = vec![Some(SeatSetup {
        radius: R,
        body_active: vec![],
        boardable: vec![true; 4],
        exit_drop: 10.0,
        ai_size: 1,
        guns: Vec::new(),
    })];
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team,
        neutral_when_empty: neutral,
        spawn_pad: None,
        pos: at,
        yaw: std::f32::consts::PI,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![
            SeatKind::Pilot,
            SeatKind::Gunner(0),
            SeatKind::Gunner(1),
            SeatKind::Gunner(2),
        ],
    });
    (s, p, v)
}

fn press(s: &mut Sim) {
    s.step(&[PadState {
        buttons: buttons::ACTION,
        ..PadState::default()
    }]);
    s.step(&[PadState::default()]);
}

#[test]
fn action_enters_the_vehicle_ahead_in_its_first_boardable_seat() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    assert_eq!(s.enter_target(p), Some((v, 0)));
    press(&mut s);
    assert_eq!(s.seat_of(p), Some((v, SeatKind::Pilot)));
    assert_eq!(s.vehicle(v).unwrap().team, Some(0), "the entrant's team");
    // A held button is one edge: still seated a tick later.
    s.step(&[PadState::default()]);
    assert!(s.seat_of(p).is_some());
}

#[test]
fn nothing_behind_or_out_of_reach_is_entered() {
    let (s, p, _) = world(Vec3::new(0.0, 2.0, 6.0), None, true);
    assert_eq!(s.enter_target(p), None, "behind the eye: t < 0");
    let (s, p, _) = world(Vec3::new(0.0, 2.0, -12.0), None, true);
    assert_eq!(s.enter_target(p), None, "the spheres do not meet");
}

#[test]
fn an_enemy_vehicle_refuses_and_a_neutral_one_takes_anyone() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), Some(1), false);
    assert_eq!(s.vehicle(v).unwrap().team, Some(1));
    assert_eq!(s.enter_target(p), None);
    s.vehicle_mut(v).unwrap().team = None;
    assert_eq!(s.enter_target(p), Some((v, 0)));
}

#[test]
fn a_player_takes_the_ai_drivers_seat_and_the_ai_is_placed_outside() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let ai = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(3.0, 0.0, 0.0)))
        .unwrap();
    assert!(s.seat_unit(ai, v, 0));
    assert_eq!(
        s.enter_seat(ai, v),
        Some(1),
        "an AI never takes a seat over"
    );
    press(&mut s);
    assert_eq!(s.seat_of(p), Some((v, SeatKind::Pilot)));
    let a = s.unit(ai).unwrap();
    assert_eq!(a.mount, None);
    assert!(
        a.soldier.pos.y.abs() < 1e-3,
        "on the ground: {}",
        a.soldier.pos
    );
    assert!(a.soldier.pos.distance(Vec3::new(0.0, 0.0, -6.0)) > R);
}

#[test]
fn the_drivers_pad_drives_the_hover_and_the_rider_follows() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    for _ in 0..60 {
        s.step(&[PadState::default()]); // settle on the springs
    }
    press(&mut s);
    let start = s.vehicle(v).unwrap().pos;
    let fwd0 = s.vehicle(v).unwrap().rot * Vec3::Z;
    let full = PadState {
        ly: i16::MAX,
        ..PadState::default()
    };
    for _ in 0..90 {
        s.step(&[full]);
    }
    let veh = s.vehicle(v).unwrap();
    let ran = (veh.pos - start).dot(fwd0);
    assert!(ran > 20.0, "3 s of full throttle: {ran} m");
    assert!(veh.input.human && veh.input.throttle == 1.0);
    assert_eq!(s.unit(p).unwrap().soldier.pos, veh.pos, "the rider rides");
    // The right stick turns toward the screen's right: f × up of a camera
    // looking along the heading.
    let right0 = (veh.rot * Vec3::Z).cross(Vec3::Y);
    let turn = PadState {
        ly: i16::MAX,
        rx: i16::MAX,
        ..PadState::default()
    };
    for _ in 0..20 {
        s.step(&[turn]);
    }
    let veh = s.vehicle(v).unwrap();
    assert!((veh.rot * Vec3::Z).dot(right0) > 0.1);
    assert!(
        veh.hover.stick_timer > 0.5,
        "full right stick ramps the timer"
    );
}

#[test]
fn the_mouse_steers_a_hover_as_a_turn_stick_and_aims_a_walker() {
    let drive = |pad: PadState| {
        let (mut s, _, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
        for _ in 0..60 {
            s.step(&[PadState::default()]);
        }
        press(&mut s);
        for _ in 0..20 {
            s.step(&[pad]);
        }
        let veh = s.vehicle(v).unwrap();
        (veh.rot, veh.input.turn, veh.hover.stick_timer)
    };
    let stick = drive(PadState {
        ly: i16::MAX,
        rx: i16::MAX,
        ..PadState::default()
    });
    let mouse = drive(PadState {
        ly: i16::MAX,
        mx: PadState::quantize_look(f64::from(crate::sim::MOUSE_STEER) * 1.5),
        buttons: buttons::MOUSE_AIM,
        ..PadState::default()
    });
    assert_eq!(mouse.1, 1.0);
    assert_eq!(stick.1, 1.0);
    assert!(mouse.0.angle_between(stick.0) < 1e-4, "{stick:?} {mouse:?}");
    assert!(mouse.2 > 0.5);

    let (mut s, _, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let veh = &mut s.vehicles[v.index as usize];
    veh.kind = VehicleKind::Walker;
    let pad = PadState {
        mx: PadState::quantize_look(0.4),
        my: PadState::quantize_look(-0.2),
        ..PadState::default()
    };
    let turn = veh.input.turn;
    let full = crate::sim::look::mouse_drive(veh, &pad, false, None);
    assert!(!full);
    assert_eq!(veh.input.turn, turn);
    let (dy, dp) = pad.look();
    assert_eq!((veh.walker.aim_yaw, veh.walker.aim_pitch), (-dy, dp));
}

#[test]
fn a_walkers_mouse_aim_passes_its_turn_window_and_keeps_pitch_limited() {
    let p = crate::vehicle::walker::tests::atst();
    let (mut s, _, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let veh = &mut s.vehicles[v.index as usize];
    veh.kind = VehicleKind::Walker;
    let pad = |yaw: f64, pitch: f64| PadState {
        mx: PadState::quantize_look(yaw),
        my: PadState::quantize_look(pitch),
        ..PadState::default()
    };
    crate::sim::look::mouse_drive(veh, &pad(-2.0, 10.0), false, Some(&p));
    assert_eq!(veh.walker.aim_yaw, 2.0);
    assert!(veh.walker.aim_yaw > p.yaw_limits[1]);
    assert_eq!(veh.walker.aim_pitch, p.pitch_limits[1]);
    crate::sim::look::mouse_drive(veh, &pad(4.0, -10.0), false, Some(&p));
    assert_eq!(veh.walker.aim_yaw, -2.0);
    assert!(veh.walker.aim_yaw < p.yaw_limits[0]);
    assert_eq!(veh.walker.aim_pitch, p.pitch_limits[0]);
}

#[test]
fn exit_stands_the_soldier_behind_the_vehicle_on_the_ground() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    press(&mut s);
    s.step(&[PadState {
        ly: i16::MAX,
        ..PadState::default()
    }]);
    press(&mut s);
    let u = s.unit(p).unwrap();
    assert_eq!(u.mount, None);
    let veh = s.vehicle(v).unwrap();
    let f = veh.rot * Vec3::Z;
    let back = Vec3::new(-f.x, 0.0, -f.z).normalize();
    let off = u.soldier.pos - veh.pos;
    let flat = Vec3::new(off.x, 0.0, off.z);
    // The vehicle drifted a tick after the exit: the candidate was behind.
    assert!(flat.normalize().dot(back) > 0.95, "behind: {flat}");
    assert!(
        (flat.length() - (R + EXIT_RING)).abs() < 0.5,
        "at ρ: {}",
        flat.length()
    );
    assert!(u.soldier.pos.y.abs() < 1e-3, "on the ground");
    assert!(
        s.vehicle(v).unwrap().input == Default::default(),
        "released"
    );
    assert_eq!(s.vehicle(v).unwrap().team, None, "neutral again");
}

#[test]
fn a_bf2_pilot_stays_in_a_flying_ship_and_the_other_seats_may_leave() {
    use crate::soldier::GroundRules;
    use crate::vehicle::flyer::FlightPhase;
    let run = |rules: GroundRules, phase: FlightPhase, seat: u8| {
        let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
        s.ground_rules = rules;
        let c = s.vehicle_mut(v).unwrap();
        c.kind = VehicleKind::Flyer;
        c.flyer.phase = phase;
        assert!(s.seat_unit(p, v, seat));
        press(&mut s);
        s.unit(p).unwrap().mount.is_none()
    };
    use FlightPhase::{Flying, Landed, Landing, Takeoff};
    for phase in [Takeoff, Flying, Landing] {
        assert!(!run(GroundRules::Bf2, phase, 0), "{phase:?}");
        assert!(run(GroundRules::Bf2, phase, 1), "gunner {phase:?}");
        assert!(run(GroundRules::Bf1, phase, 0), "bf1 {phase:?}");
    }
    assert!(run(GroundRules::Bf2, Landed, 0), "parked");
}

#[test]
fn a_pilot_seat_without_a_human_drives_nothing() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    press(&mut s);
    s.step(&[PadState {
        ly: i16::MAX,
        ..PadState::default()
    }]);
    assert_eq!(s.vehicle(v).unwrap().input.throttle, 1.0);
    s.units[p.index as usize].controller = Controller::Idle;
    s.step(&[]);
    assert_eq!(s.vehicle(v).unwrap().input, Default::default());
}

#[test]
fn the_driver_sticks_are_shaped_as_the_ps2_shapes_them() {
    let q = PadState::quantize;
    let (i, full) = driver_input(&PadState {
        lx: q(0.1),
        ly: q(-0.05),
        rx: q(1.0),
        ry: q(1.0),
        ..Default::default()
    });
    assert_eq!(
        (i.strafe, i.throttle),
        (MOVE_FLOOR, -MOVE_FLOOR),
        "the 0.3 floor"
    );
    assert!(full);
    assert!(
        (i.turn * i.turn + i.pitch * i.pitch - 1.0).abs() < 1e-5,
        "into the circle"
    );
    let (_, full) = driver_input(&PadState {
        rx: q(0.9),
        ..PadState::default()
    });
    assert!(!full, "0.81 < 0.9");
}

/// Card VTURB: a building turret — one gunner seat, no pilot — standing in
/// its own collision box. ACTION takes the gunner seat, the turret never
/// moves, and the exit stands the soldier on the ring behind it: the path
/// test, which every candidate would fail from inside the box, is skipped.
#[test]
fn a_building_turret_seats_a_gunner_and_lets_it_out_of_its_own_box() {
    use crate::collision::{Body, CollisionWorld, Shape};
    let at = Vec3::new(0.0, 0.0, -4.0);
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    s.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::from_translation(at + Vec3::Y),
            Shape::Box {
                half: Vec3::splat(1.5),
            },
        )],
        Vec::new(),
    );
    s.hover_params = vec![None];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 1,
        guns: Vec::new(),
    })];
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: at,
        yaw: std::f32::consts::PI,
        max_health: 300.0,
        health_type: HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    });
    assert!(s.vehicle(v).unwrap().pilot_seat().is_none());
    assert_eq!(s.enter_target(p), Some((v, 0)));
    press(&mut s);
    assert_eq!(s.seat_of(p), Some((v, SeatKind::Gunner(0))));
    for _ in 0..30 {
        s.step(&[PadState {
            ly: i16::MAX,
            rx: i16::MAX,
            ..PadState::default()
        }]);
    }
    assert_eq!(s.vehicle(v).unwrap().pos, at, "a turret never moves");
    press(&mut s);
    let u = s.unit(p).unwrap();
    assert_eq!(u.mount, None);
    // Facing −Z (yaw π): behind is +Z, ρ = 3 + EXIT_RING from the origin.
    let want = at + Vec3::Z * (3.0 + EXIT_RING);
    assert!(
        u.soldier.pos.distance(want) < 1e-3,
        "placed at {} (want {want})",
        u.soldier.pos
    );
}

/// Card VMESH: a rider whose vehicle's centre is inside a closed prop (a
/// bike through BF1 Endor's cp2 boulder) gets out OUTSIDE it - every one of
/// §5's points is inside and passes the path test, the ring past the
/// body's box is taken - and stands on the ground there.
#[test]
fn a_rider_whose_vehicle_is_inside_a_closed_body_gets_out_outside_it() {
    use crate::collision::{box_mesh, CollisionWorld};
    let at = Vec3::new(0.0, 2.0, -6.0);
    let (mut s, p, v) = world(at, None, true);
    press(&mut s);
    assert_eq!(s.seat_of(p), Some((v, SeatKind::Pilot)));
    // The prop closes round the seated rider's vehicle: 2 m past §5's ring.
    let half = Vec3::new(R + EXIT_RING + 2.0, 4.0, R + EXIT_RING + 2.0);
    s.collision = CollisionWorld::new(
        vec![box_mesh(Vec3::new(0.0, 3.0, -6.0), half, false)],
        Vec::new(),
    );
    s.exit_vehicle(p);
    let u = s.unit(p).unwrap();
    let feet = u.soldier.pos;
    assert_eq!(u.mount, None);
    assert_eq!(s.shut_in(feet + Vec3::Y), None, "outside: {feet}");
    let off = feet - Vec3::new(0.0, 0.0, -6.0);
    assert!(
        off.x.abs() > half.x || off.z.abs() > half.z,
        "past the box: {feet}"
    );
    assert!(feet.y.abs() < 1e-3, "on the ground: {feet}");
}

/// Card VMESH: a hover driven at a closed prop at full throttle stays
/// outside it. The discrete contact pass let a bike's centre through BF1
/// Endor's boulder at 23 m/s; here the hover has no hull bodies, so nothing
/// but the keep-out rule stands between its centre and the shell.
#[test]
fn a_hover_driven_at_a_closed_body_stays_outside() {
    use crate::collision::{box_mesh, CollisionWorld};
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    {
        // A bike's pace (2 m a tick, past its spring spheres' size).
        let hp = s.hover_params[0].as_mut().unwrap();
        hp.hull.clear();
        hp.forward_speed = 60.0;
        hp.acceleration = 60.0;
    }
    // (No impact destroys it: the test is where it goes.)
    s.vehicles[v.index as usize].health = crate::weapon::Health::new(1.0e9, HealthType::Vehicle);
    press(&mut s);
    assert_eq!(s.seat_of(p), Some((v, SeatKind::Pilot)));
    let rock = Vec3::new(0.0, 3.0, -40.0);
    s.collision = CollisionWorld::new(
        vec![box_mesh(rock, Vec3::new(5.0, 3.0, 5.0), false)],
        Vec::new(),
    );
    let (mut nearest, mut fastest) = (f32::MAX, 0.0f32);
    for _ in 0..(6 * crate::TICK_HZ) {
        s.step(&[PadState {
            ly: i16::MAX,
            ..PadState::default()
        }]);
        let veh = s.vehicle(v).unwrap();
        assert_eq!(s.shut_in(veh.pos), None, "inside at {}", veh.pos);
        nearest = nearest.min(veh.pos.z);
        fastest = fastest.max(Vec3::new(veh.vel.x, 0.0, veh.vel.z).length());
    }
    assert!(nearest < -30.0, "it reached the wall: {nearest}");
    assert!(fastest > 23.0, "at speed: {fastest} m/s");
}

/// Failed public entry must preserve the occupant and both mounts.
#[test]
fn refused_takeovers_do_not_evict_the_rider() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let ai = s.spawn(&UnitSpec::trooper(0, Vec3::X * 3.0)).unwrap();
    assert!(s.seat_unit(ai, v, 0));
    let rider_pos = s.unit(ai).unwrap().soldier.pos;
    let stale = UnitHandle {
        gen: p.gen + 1,
        ..p
    };
    assert!(!s.enter_vehicle(stale, v, 0));
    let dead = s.spawn(&UnitSpec::trooper(0, Vec3::X * 9.0)).unwrap();
    s.kill_environment(dead, crate::sim::DeathCause::KillFloor);
    assert!(!s.enter_vehicle(dead, v, 0));
    assert!(s.seat_unit(p, v, 1));
    assert!(!s.enter_vehicle(p, v, 0));
    assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, Some(ai));
    assert_eq!(s.unit(ai).unwrap().soldier.pos, rider_pos);
    assert_eq!(s.unit(ai).unwrap().mount.unwrap().seat, 0);
    assert_eq!(s.unit(p).unwrap().mount.unwrap().seat, 1);
    s.unseat_unit(p);
    assert!(s.enter_vehicle(p, v, 0), "human can replace an AI");
    assert!(!s.enter_vehicle(ai, v, 0), "AI cannot replace a human");
    assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, Some(p));
    assert!(s.unit(ai).unwrap().mount.is_none());
}

/// SQVEH's follower slice, independent of baked assets. The free seats in
/// `world` have no guns: following must not use autonomous gunner eligibility.
fn sqveh_follower(s: &mut Sim, leader: UnitHandle, at: Vec3) -> UnitHandle {
    let bot = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, at)
        })
        .unwrap();
    s.issue_command(0, crate::squad::CommandKind::Follow(leader));
    assert!(s.unit(bot).unwrap().command.is_some());
    bot
}

#[test]
fn sqveh_follower_walks_to_leaders_vehicle_and_boards_within_two_seconds() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let bot = sqveh_follower(&mut s, p, Vec3::new(0.0, 0.0, 8.0));
    // A directed board must still work during an autonomous-search cooldown.
    s.unit_mut(bot).unwrap().ai.ride = Some(crate::ai::RideState {
        block_until: 20 * u64::from(crate::TICK_HZ),
        ..Default::default()
    });
    assert!(s.enter_vehicle(p, v, 0));
    let mut boarded_at = None;
    for _ in 0..2 * crate::TICK_HZ {
        s.step(&[PadState::default()]);
        if let Some(m) = s.unit(bot).unwrap().mount {
            assert_eq!((m.vehicle, m.seat), (v, 1));
            boarded_at = Some(s.tick);
            break;
        }
    }
    let tick = boarded_at.expect("the follower boards within 2 s");
    assert!(tick > 1, "starts outside the boarding radius and must walk");
    assert_eq!(s.unit(bot).unwrap().command, None);
    assert_eq!(s.unit(bot).unwrap().ai.cmd, None);
    assert_eq!(s.nav_stats.ai_boards, 1);
    println!(
        "SQVEH follower: boarded at tick {tick}, {:.3} s",
        tick as f32 * crate::TICK_DT
    );
}

#[test]
fn sqveh_full_vehicle_keeps_follower_on_foot() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let bot = sqveh_follower(&mut s, p, Vec3::ZERO);
    for k in 1..4 {
        let passenger = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        assert!(s.enter_vehicle(passenger, v, k));
    }
    assert!(s.enter_vehicle(p, v, 0));
    for _ in 0..crate::ai::THINK_PERIOD {
        s.step(&[PadState::default()]);
    }
    assert!(s.unit(bot).unwrap().mount.is_none());
    assert!(matches!(s.unit(bot).unwrap().command.map(|c| c.kind),
        Some(crate::squad::CommandKind::Follow(h)) if h == p));
    assert_eq!(s.nav_stats.releases, 0);
}

#[test]
fn sqveh_two_followers_compete_for_one_seat_without_displacing_riders() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let bots = [
        sqveh_follower(&mut s, p, Vec3::ZERO),
        sqveh_follower(&mut s, p, Vec3::X),
    ];
    s.seat_setup[0].as_mut().unwrap().boardable = vec![true, true, false, false];
    assert!(s.enter_vehicle(p, v, 0));
    for _ in 0..crate::ai::THINK_PERIOD * 2 {
        s.step(&[PadState::default()]);
    }
    assert_eq!(
        bots.iter()
            .filter(|&&b| s.unit(b).unwrap().mount.is_some())
            .count(),
        1
    );
    for bot in bots {
        let u = s.unit(bot).unwrap();
        assert_eq!(u.command.is_some(), u.mount.is_none());
    }
    assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, Some(p));
    println!("SQVEH competition: 2 followers / 1 free seat -> 1 boarded, 1 following on foot");
}

#[test]
fn sqveh_holder_and_follower_of_an_unmounted_leader_do_not_board() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let bot = sqveh_follower(&mut s, p, Vec3::ZERO);
    s.ai_use_pass();
    assert!(s.unit(bot).unwrap().mount.is_none());
    s.unit_mut(bot).unwrap().command.as_mut().unwrap().kind =
        crate::squad::CommandKind::Hold(Vec3::ZERO);
    assert!(s.enter_vehicle(p, v, 0));
    s.ai_use_pass();
    assert!(s.unit(bot).unwrap().mount.is_none());
    assert_eq!(s.nav_stats.ai_boards, 0);
}

#[test]
fn sqveh_follower_never_boards_after_leader_has_left_or_died() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let bot = sqveh_follower(&mut s, p, Vec3::ZERO);
    assert!(s.enter_vehicle(p, v, 0));
    s.exit_vehicle(p);
    s.ai_use_pass();
    assert!(s.unit(bot).unwrap().mount.is_none());
    assert!(s.enter_vehicle(p, v, 0));
    s.kill_environment(p, crate::sim::DeathCause::Fall);
    s.ai_use_pass();
    assert!(s.unit(bot).unwrap().mount.is_none());
}

#[test]
fn sqveh_following_cancels_an_old_autonomous_board_request() {
    let (mut s, p, v) = world(Vec3::new(0.0, 2.0, -6.0), None, true);
    let bot = sqveh_follower(&mut s, p, Vec3::new(0.0, 0.0, 20.0));
    assert!(s.enter_vehicle(p, v, 0));
    s.unit_mut(bot).unwrap().ai.ride = Some(crate::ai::RideState {
        board: Some(v),
        use_req: true,
        block_until: 600,
        ..Default::default()
    });
    // Before its next think an old request must not be serviced or shorten
    // the active cooldown just because the leader's vehicle is out of reach.
    s.ai_use_pass();
    assert_eq!(s.unit(bot).unwrap().ai.ride.unwrap().block_until, 600);
    s.step(&[PadState::default()]);
    let u = s.unit(bot).unwrap();
    assert!(u.mount.is_none());
    let r = u.ai.ride.unwrap();
    assert!(r.board.is_none());
    assert!(!r.use_req);
    assert_eq!(r.block_until, 600);
}
