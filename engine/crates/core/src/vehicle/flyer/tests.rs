use super::*;
use crate::unit::UnitHandle;
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::HealthType;

const DT: f32 = 1.0 / 30.0;

fn craft(phase: FlightPhase) -> Vehicle {
    let mut v = VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 10.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
    .build(1);
    v.seats[0].occupant = Some(UnitHandle { index: 0, gen: 1 });
    v.flyer.phase = phase;
    v.flyer.initialized = phase != FlightPhase::Landed;
    v.flyer.lift = if phase == FlightPhase::Flying {
        1.0
    } else {
        0.0
    };
    v
}

fn ground(height: f32) -> FlyerEnvironment {
    FlyerEnvironment {
        ground: Some(LandingSurface {
            height,
            normal: Vec3::Y,
            can_land: true,
        }),
        ..FlyerEnvironment::default()
    }
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn parked_flyer_keeps_pose_and_hash_extension_empty() {
    let mut v = craft(FlightPhase::Landed);
    v.seats[0].occupant = None;
    let pos = v.pos;
    let rot = v.rot;
    for _ in 0..300 {
        step(&mut v, &FlyerParams::default(), ground(0.0), DT);
    }
    assert_eq!(v.pos, pos);
    assert_eq!(v.rot, rot);
    assert_eq!(v.flyer, FlyerState::default());
    let mut bytes = Vec::new();
    v.flyer.hash_into(&mut |b| bytes.extend_from_slice(b));
    assert!(bytes.is_empty());
}

#[test]
fn throttle_extrema_and_neutral_have_authored_speeds() {
    // BF1 X-wing's authored 20 / 50 / 82 m/s, acceleration 30 m/s².
    let p = FlyerParams {
        min_speed: 20.0,
        mid_speed: 50.0,
        max_speed: 82.0,
        acceleration: 30.0,
        ..FlyerParams::default()
    };
    for (throttle, speed) in [(-1.0, 20.0), (0.0, 50.0), (1.0, 82.0)] {
        let mut v = craft(FlightPhase::Flying);
        v.input.throttle = throttle;
        for _ in 0..120 {
            step(&mut v, &p, FlyerEnvironment::default(), DT);
        }
        near(v.vel.z, speed);
        near(v.vel.y, 0.0);
    }
    let mut v = craft(FlightPhase::Flying);
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    near(v.vel.z, 1.0);
    near(v.pos.z, 1.0 / 30.0);
}

#[test]
fn takeoff_fraction_waits_for_landed_height_plus_clearance_in_both_games() {
    for bf2 in [false, true] {
        let p = FlyerParams {
            bf2,
            landed_height: 1.0,
            takeoff_height: 2.0,
            takeoff_time: 1.0,
            ..FlyerParams::default()
        };
        let mut v = craft(FlightPhase::Takeoff);
        v.pos.y = 3.0;
        step(&mut v, &p, ground(0.0), DT);
        assert_eq!(v.flyer.lift, 0.0);
        v.pos.y = 3.001;
        step(&mut v, &p, ground(0.0), DT);
        near(v.flyer.lift, DT);
    }
}

#[test]
fn held_takeoff_button_is_not_a_second_toggle() {
    let p = FlyerParams {
        takeoff_time: 0.5,
        ..FlyerParams::default()
    };
    let mut v = craft(FlightPhase::Landed);
    v.input.jump = true;
    for _ in 0..60 {
        step(&mut v, &p, FlyerEnvironment::default(), DT);
    }
    assert_eq!(v.flyer.phase, FlightPhase::Flying);
    v.input.jump = false;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    v.input.jump = true;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Landing);
}

#[test]
fn landing_requires_the_ray_and_slope_and_does_not_snap() {
    let p = FlyerParams {
        landed_height: 1.0,
        ..FlyerParams::default()
    };
    let mut v = craft(FlightPhase::Landing);
    v.pos.y = 0.99;
    step(&mut v, &p, ground(0.0), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Landed);
    assert_eq!(v.pos.y, 0.99);
    let mut v = craft(FlightPhase::Landing);
    v.pos.y = 0.99;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Landing);
    let mut v = craft(FlightPhase::Landing);
    v.pos.y = 0.99;
    let mut env = ground(0.0);
    env.ground.as_mut().unwrap().normal = Vec3::new(0.4, 0.916_515, 0.0);
    step(&mut v, &p, env, DT);
    assert_eq!(v.flyer.phase, FlightPhase::Landing);
}

#[test]
fn bf2_landing_cancel_threshold_is_point_zero_five_not_bf1_point_two_five() {
    for (bf2, expected) in [(false, FlightPhase::Takeoff), (true, FlightPhase::Landing)] {
        let p = FlyerParams {
            bf2,
            ..FlyerParams::default()
        };
        let mut v = craft(FlightPhase::Landing);
        v.flyer.lift = 0.1;
        v.input.jump = true;
        step(&mut v, &p, FlyerEnvironment::default(), DT);
        assert_eq!(v.flyer.phase, expected);
    }
}

#[test]
fn bf1_rate_filter_and_bf2_rate_slew_are_different_laws() {
    let mut bf1 = craft(FlightPhase::Flying);
    let mut bf2 = bf1.clone();
    bf1.input.pitch = 1.0;
    bf2.input.pitch = 1.0;
    let p = FlyerParams {
        pitch_rate: 1.0,
        pitch_filter: 4.0,
        bank_angle: 0.0,
        ..FlyerParams::default()
    };
    let p2 = FlyerParams {
        bf2: true,
        ..p.clone()
    };
    for _ in 0..2 {
        step(&mut bf1, &p, FlyerEnvironment::default(), DT);
        step(&mut bf2, &p2, FlyerEnvironment::default(), DT);
    }
    near(bf1.flyer.omega.x, -0.248_888_9);
    near(bf2.flyer.omega.x, -0.266_666_68);
    near(angular_slew(-1.0, 0.0, 4.0, 10.0, DT), -2.0 / 3.0);
}

#[test]
fn bf2_landing_region_caps_are_not_applied_to_open_space() {
    let p = FlyerParams {
        min_speed: 35.0,
        mid_speed: 70.0,
        max_speed: 95.0,
        ..FlyerParams::bf2()
    };
    assert_eq!(p.speeds(false), [35.0, 70.0, 95.0]);
    assert_eq!(p.speeds(true), [7.0, 20.0, 60.0]);
    let p1 = FlyerParams { bf2: false, ..p };
    assert_eq!(p1.speeds(true), [35.0, 70.0, 95.0]);
}

#[test]
fn bf2_registered_landing_regions_gate_the_landing_button() {
    for (bf2, required, inside, expected) in [
        (true, true, false, FlightPhase::Flying),
        (true, true, true, FlightPhase::Landing),
        (true, false, false, FlightPhase::Landing),
        (false, true, false, FlightPhase::Landing),
    ] {
        let p = FlyerParams {
            bf2,
            ..FlyerParams::default()
        };
        let mut v = craft(FlightPhase::Flying);
        v.input.jump = true;
        let env = FlyerEnvironment {
            require_landing_region: required,
            in_landing_region: inside,
            ..FlyerEnvironment::default()
        };
        step(&mut v, &p, env, DT);
        assert_eq!(v.flyer.phase, expected);
    }
}

#[test]
fn bf2_boost_uses_authored_acceleration_and_energy_then_stops_on_empty() {
    let p = FlyerParams {
        min_speed: 35.0,
        mid_speed: 70.0,
        max_speed: 95.0,
        boost_speed: 150.0,
        boost_acceleration: 80.0,
        energy_boost_drain: 40.0,
        ..FlyerParams::bf2()
    };
    let mut v = craft(FlightPhase::Flying);
    v.vel.z = 95.0;
    v.input.throttle = 1.0;
    v.input.boost = true;
    v.flyer.energy = 1.0;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    near(v.vel.z, 95.0 + 80.0 / 30.0);
    assert!(v.flyer.boosting && v.flyer.drained);
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    assert!(!v.flyer.boosting);
    near(v.flyer.energy, 0.0);
}

#[test]
fn no_floor_is_invented_in_space_and_unoccupied_flyer_falls() {
    let p = FlyerParams::default();
    let mut v = craft(FlightPhase::Flying);
    v.pos.y = -100.0;
    v.vel.z = 20.0;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    near(v.pos.y, -100.0);
    v.seats[0].occupant = None;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Falling);
    assert!(v.vel.y < 0.0);
}

#[test]
fn pitch_has_no_ground_vehicle_clamp() {
    let p = FlyerParams {
        pitch_rate: 1.0,
        pitch_filter: 100.0,
        bank_angle: 0.0,
        ..FlyerParams::default()
    };
    let mut v = craft(FlightPhase::Flying);
    v.input.pitch = 1.0;
    for _ in 0..100 {
        step(&mut v, &p, FlyerEnvironment::default(), DT);
    }
    assert!(
        (v.rot * Vec3::Z).z < -0.9,
        "flyer should complete the vertical part of a loop"
    );
    near(v.rot.length(), 1.0);
}

#[test]
fn bank_uses_boost_ceiling_in_bf2_and_takeoff_turn_rate_in_both_games() {
    // A known local yaw rate and forward velocity give an analytic lateral
    // acceleration. No steering/tilt filtering obscures the PS2 divisor.
    for bf2 in [false, true] {
        for (phase, lift, local_speed, phase_divisor) in [
            (FlightPhase::Flying, 1.0, 60.0, 1.0),
            (FlightPhase::Takeoff, 0.5, 30.0, 2.0),
            (FlightPhase::Landing, 0.5 + DT, 30.0, 1.0),
        ] {
            let p = FlyerParams {
                bf2,
                min_speed: 60.0,
                mid_speed: 60.0,
                max_speed: 60.0,
                boost_speed: 120.0,
                turn_rate: 2.0,
                turn_filter: 0.0,
                turn_damp: 0.0,
                level_filter: 0.0,
                level_damp: 0.0,
                landing_time: 1.0,
                bank_angle: 1.0,
                bank_filter: 30.0,
                ..FlyerParams::default()
            };
            let mut v = craft(phase);
            v.vel = Vec3::Z * 60.0;
            v.flyer.lift = lift;
            v.flyer.omega.y = 1.0;
            let env = ground(v.pos.y - 1.0);
            step(&mut v, &p, env, DT);
            let ceiling = if bf2 { 120.0 } else { 60.0 };
            near(v.flyer.tilt_roll, -local_speed / (ceiling * phase_divisor));
        }
    }
}

#[test]
fn pitching_tilts_by_the_turn_rate_across_the_velocitys_sideways_part() {
    for bf2 in [false, true] {
        let p = FlyerParams {
            bf2,
            min_speed: 60.0,
            mid_speed: 60.0,
            max_speed: 60.0,
            boost_speed: 120.0,
            pitch_filter: 0.0,
            pitch_damp: 0.0,
            level_filter: 0.0,
            level_damp: 0.0,
            velocity_filter: 0.0,
            bank_angle: 1.0,
            bank_filter: 30.0,
            ..FlyerParams::default()
        };
        let mut v = craft(FlightPhase::Flying);
        v.vel = Vec3::new(0.0, 10.0, 60.0);
        v.flyer.lift = 1.0;
        v.flyer.omega.x = 1.0;
        let env = ground(v.pos.y - 1.0);
        step(&mut v, &p, env, DT);
        let want = if bf2 { 10.0 / 120.0 } else { 0.0 };
        near(v.flyer.tilt_pitch, want);
    }
}

#[test]
fn landing_removes_visual_tilt_and_bf2_initializes_leveling_on_takeoff() {
    let p = FlyerParams::bf2();
    let mut v = craft(FlightPhase::Landing);
    v.flyer.tilt_pitch = 0.1;
    v.flyer.tilt_roll = -0.2;
    v.rot = quat_z(-0.2) * quat_x(0.1);
    let floor = v.pos.y + 0.1;
    step(&mut v, &p, ground(floor), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Landed);
    near(v.rot.angle_between(Quat::IDENTITY), 0.0);
    let mut v = craft(FlightPhase::Landed);
    v.input.jump = true;
    step(&mut v, &p, ground(0.0), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Takeoff);
    near(v.flyer.level_time, 1.0);
}

#[test]
fn bf2_takeoff_and_automatic_leveling_clear_filtered_roll() {
    let p = FlyerParams::bf2();
    let mut v = craft(FlightPhase::Takeoff);
    v.flyer.lift = 0.999;
    v.flyer.roll_input = 0.5;
    step(&mut v, &p, ground(0.0), DT);
    assert_eq!(v.flyer.phase, FlightPhase::Flying);
    near(v.flyer.roll_input, 0.0);
    v.flyer.level_time = 0.8;
    v.flyer.roll_input = 0.5;
    step(&mut v, &p, ground(0.0), DT);
    near(v.flyer.roll_input, 0.0);
}

#[test]
fn bf2_energy_recovers_before_latched_boost_drain_below_normal_ceiling() {
    let p = FlyerParams {
        energy_auto_restore: 7.5,
        energy_boost_drain: 20.0,
        boost_speed: 150.0,
        boost_acceleration: 80.0,
        ..FlyerParams::bf2()
    };
    let mut v = craft(FlightPhase::Flying);
    v.vel.z = p.max_speed;
    v.input.throttle = 1.0;
    v.flyer.boosting = true;
    v.flyer.energy = 50.0;
    step(&mut v, &p, FlyerEnvironment::default(), DT);
    near(v.flyer.energy, 50.0 + (7.5 - 20.0) * DT);
}

#[test]
fn bf2_depleted_energy_recharges_while_landed_and_persists_on_relaunch() {
    let p = FlyerParams {
        energy_max: 80.0,
        energy_auto_restore: 7.5,
        ..FlyerParams::bf2()
    };
    let mut v = craft(FlightPhase::Landing);
    v.flyer.energy = 0.0;
    v.flyer.drained = true;
    let env = ground(v.pos.y + 0.1);
    step(&mut v, &p, env, DT);
    assert_eq!(v.flyer.phase, FlightPhase::Landed);
    for _ in 0..90 {
        step(&mut v, &p, env, DT);
    }
    near(v.flyer.energy, 91.0 * 7.5 * DT);
    assert!(!v.flyer.drained);
    let recharged = v.flyer.energy;
    v.input.jump = true;
    step(&mut v, &p, env, DT);
    assert_eq!(v.flyer.phase, FlightPhase::Takeoff);
    near(v.flyer.energy, recharged + 7.5 * DT);
    assert!(v.flyer.energy < p.energy_max);
}
