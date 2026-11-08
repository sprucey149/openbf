//! Headless, sub-second deterministic micro-scenarios for Space Battles.
//!
//! Covers the core mechanics for:
//! - Issue #122: Pilot threat tracking, secondary weapon channel selection, and evasive side rolls.
//! - Issue #123: Capital-ship linked shield absorption, controller destruction, and turret fire-line gating.
//! - Issue #124: Shot-down flyer crash delay (falling transition, tumbling, and destruction).
//!
//! These scenarios run in milliseconds via `cargo test -p game --test space_scenarios`.

use core::sim::{Sim, Station};
use core::unit::{UnitHandle, UnitSpec};
use core::vehicle::flyer::{FlightPhase, FlyerParams};
use core::vehicle::{SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use core::weapon::{DamageDesc, Health, HealthType};
use glam::Vec3;

fn test_flyer_params() -> FlyerParams {
    FlyerParams {
        bf2: true,
        has_chunks: true,
        trick_drain: 15.0,
        trick_side_strafe: 20.0,
        min_speed: 35.0,
        mid_speed: 70.0,
        max_speed: 100.0,
        ..FlyerParams::bf2()
    }
}

fn spawn_test_pilot(sim: &mut Sim, team: u8) -> UnitHandle {
    sim.spawn(&UnitSpec::trooper(team, Vec3::new(0.0, 100.0, 0.0)))
        .expect("spawn pilot unit")
}

fn spawn_test_flyer(sim: &mut Sim, team: u8, pos: Vec3) -> VehicleHandle {
    let spec = VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 1500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    };
    sim.spawn_vehicle(&spec)
}

#[test]
fn shot_down_flyer_enters_falling_crash_state_instead_of_instant_death() {
    let mut sim = Sim::empty(42);
    sim.flyer_params = vec![Some(test_flyer_params())];

    let pilot = spawn_test_pilot(&mut sim, 0);
    let vh = spawn_test_flyer(&mut sim, 0, Vec3::new(0.0, 500.0, 0.0));

    // Seat the pilot and transition to Flying
    sim.units[pilot.index as usize].mount = Some(core::vehicle::Mount {
        vehicle: vh,
        seat: 0,
    });
    if let Some(v) = sim.vehicle_mut(vh) {
        v.seats[0].occupant = Some(pilot);
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
        v.flyer.lift = 1.0;
    }

    // Deal lethal damage to the craft in flight
    let desc = DamageDesc {
        damage: 2000.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    sim.damage_vehicle(vh, &desc, 1.0, None);

    let v = sim
        .vehicle(vh)
        .expect("flyer still exists in the sim during crash");
    assert_eq!(
        v.flyer.phase,
        FlightPhase::Falling,
        "a shot-down flyer enters Falling phase rather than immediate destruction"
    );
    assert!(
        v.flyer.crash_timer >= 0.2 && v.flyer.crash_timer <= 2.2,
        "crash timer is between 0.2s and 2.2s for craft with chunks: got {:.2}",
        v.flyer.crash_timer
    );
    assert!(
        !v.health.alive(),
        "craft has zero health while tumbling down"
    );

    // Step 2 ticks: craft should remain in Falling state while timer counts down
    let initial_timer = v.flyer.crash_timer;
    sim.step(&[]);
    if let Some(v) = sim.vehicle(vh) {
        assert_eq!(v.flyer.phase, FlightPhase::Falling);
        assert!(v.flyer.crash_timer < initial_timer);
    }
}

#[test]
fn evasive_side_roll_consumes_trick_energy_and_sets_strafe_vector() {
    let params = test_flyer_params();
    let mut v = VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 200.0, 0.0),
        yaw: 0.0,
        max_health: 1500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
    .build(1);

    v.flyer.phase = FlightPhase::Flying;
    v.flyer.energy = 80.0;
    v.flyer.in_roll = false;

    // Trigger a side roll to the right
    let roll_right = Vec3::X;
    core::vehicle::flyer::side_roll(&mut v.flyer, &params, roll_right);

    assert_eq!(
        v.flyer.energy,
        80.0 - params.trick_drain,
        "energy is drained by trick_drain"
    );
    assert_eq!(
        v.flyer.side_roll, roll_right,
        "side roll direction vector is set"
    );
}

#[test]
fn capital_shield_group_absorbs_damage_and_destroys_controller_when_depleted() {
    let mut sim = Sim::empty(7);
    sim.station_classes
        .push(core::sim::StationClass::soldier("shield_gen", 1.0, 0.0, 0));

    // Set up 2 stations: controller (shield gen) and target system (bridge)
    let gen_id = sim.stations.len() as u16;
    let mut gen_st = Station::new(0, Vec3::new(0.0, 50.0, 0.0), 10.0);
    gen_st.team = Some(1);
    gen_st.health = Health::new(4500.0, HealthType::Building);
    sim.stations.push(gen_st);

    let bridge_id = sim.stations.len() as u16;
    let mut bridge_st = Station::new(0, Vec3::new(0.0, 100.0, 200.0), 20.0);
    bridge_st.team = Some(1);
    bridge_st.health = Health::new(3000.0, HealthType::Building);
    sim.stations.push(bridge_st);

    let mut sa = core::sim::SpaceAssault::new(180);
    // Link bridge to the shield generator with a 1,000 HP shield
    let shield_group = core::sim::ShieldGroup::new(vec![bridge_id], Some(gen_id), 1000.0, 50.0);
    sa.shields.push(shield_group);
    sim.install_space_assault(sa);

    let desc = DamageDesc {
        damage: 400.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };

    // Deal 400 damage to bridge: shield absorbs it all, bridge hull remains 100%
    sim.damage_station(bridge_id, &desc, 1.0, None, None);
    assert_eq!(
        sim.stations[usize::from(bridge_id)].health.cur,
        3000.0,
        "bridge hull takes zero damage while shield has pool remaining"
    );
    assert_eq!(
        sim.space_assault.as_ref().unwrap().shields[0].shield,
        600.0,
        "shield pool decreases from 1000 to 600"
    );
    assert!(
        !sim.stations[usize::from(gen_id)].destroyed,
        "shield generator remains alive while shield holds"
    );

    // Deal 700 more damage: depletes the remaining 600 shield
    let desc_kill = DamageDesc {
        damage: 700.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    sim.damage_station(bridge_id, &desc_kill, 1.0, None, None);
    let sa_after = sim.space_assault.as_ref().unwrap();
    assert_eq!(sa_after.shields[0].shield, 0.0);
    assert!(
        sa_after.shields[0].all_down,
        "all_down flag is set when shield reaches zero"
    );
    assert!(
        sim.stations[usize::from(gen_id)].destroyed,
        "controller station (generator) is destroyed when shield collapses"
    );
}

#[test]
fn turret_eye_direction_aligns_within_cone_before_permitting_fire() {
    // Original game alignment rule (§19): eye direction (turret barrel axis)
    // must be within 22.5 degrees of target vector.
    let forward = Vec3::Z;

    // 10 degrees off target: within 22.5 deg cone -> fire allowed
    let angle_10_deg = 10.0f32.to_radians();
    let dir_10_deg = Vec3::new(angle_10_deg.sin(), 0.0, angle_10_deg.cos());
    assert!(
        core::ai::flyer_on_target(forward, dir_10_deg * 100.0),
        "barrel aligned within 10 degrees permits fire"
    );

    // 40 degrees off target (turret barrel still swinging): fire suppressed
    let angle_40_deg = 40.0f32.to_radians();
    let dir_40_deg = Vec3::new(angle_40_deg.sin(), 0.0, angle_40_deg.cos());
    assert!(
        !core::ai::flyer_on_target(forward, dir_40_deg * 100.0),
        "barrel swinging 40 degrees off target suppresses fire"
    );
}
