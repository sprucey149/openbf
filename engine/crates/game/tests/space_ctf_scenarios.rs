//! Headless, sub-second deterministic micro-scenarios for Issue #126 (Instant Action CTF & Space One-Flag).
//!
//! Covers:
//! - Space One-Flag flyer pickup (`FLYER_PICKUP_RADIUS`), carrier loss drop, `FLAG_REPICKUP_DELAY`
//!   cooldown, re-pickup, hangar capture scoring, and round victory (`Event::TeamDefeated`).
//! - Two-Flag home-stand friendly exclusion, enemy steal, death-position drop, friendly Touch Return,
//!   and bit-exact `Sim::hash` invariance when `ctf` is `None`.

use core::sim::{
    CtfState, Event, FlagStatus, PadState, Sim, FLAG_REPICKUP_DELAY, FLYER_PICKUP_RADIUS,
    SPACE_CARRIED_OFFSET,
};
use core::unit::{UnitHandle, UnitSpec};
use core::vehicle::flyer::{FlightPhase, FlyerParams};
use core::vehicle::{Mount, SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use core::weapon::HealthType;
use glam::Vec3;

fn spawn_seated_flyer(sim: &mut Sim, team: u8, pos: Vec3) -> (UnitHandle, VehicleHandle) {
    let pilot = sim
        .spawn(&UnitSpec::trooper(team, pos))
        .expect("spawn pilot");
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 1200.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    sim.units[pilot.index as usize].mount = Some(Mount {
        vehicle: vh,
        seat: 0,
    });
    if let Some(v) = sim.vehicle_mut(vh) {
        v.seats[0].occupant = Some(pilot);
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
        v.flyer.lift = 1.0;
    }
    (pilot, vh)
}

#[test]
fn space_one_flag_flyer_pickup_drop_repickup_and_hangar_capture_win() {
    let mut sim = Sim::empty(126);
    sim.flyer_params = vec![Some(FlyerParams::bf2())];

    let home = Vec3::new(0.0, 200.0, 0.0);
    let team0_goal = Vec3::new(400.0, 200.0, 0.0);
    let team1_goal = Vec3::new(-400.0, 200.0, 0.0);
    sim.install_ctf(CtfState::new_space_one_flag(1, home, team0_goal, team1_goal));

    // Spawn a Team 0 starfighter inside the 20m space-flag pickup sphere.
    let (pilot_a, flyer_a) = spawn_seated_flyer(
        &mut sim,
        0,
        home + Vec3::new(FLYER_PICKUP_RADIUS - 2.0, 0.0, 0.0),
    );
    sim.step(&[PadState::default()]);

    let ctf = sim.ctf.as_ref().expect("ctf active");
    assert_eq!(ctf.flags[0].status, FlagStatus::Carried { carrier: pilot_a });
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::FlagPickedUp { flag: 0, unit, team: 0 } if *unit == pilot_a)));

    // Move carrier mid-space, then destroy the starfighter so the flag drops in space.
    let drop_spot = Vec3::new(180.0, 200.0, 0.0);
    sim.vehicle_mut(flyer_a).unwrap().pos = drop_spot;
    sim.vehicle_mut(flyer_a).unwrap().health.cur = 0.0;
    sim.step(&[PadState::default()]);

    assert!(matches!(
        sim.ctf.as_ref().unwrap().flags[0].status,
        FlagStatus::Dropped { .. }
    ));
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::FlagDropped { flag: 0, team: 0, .. })));

    // Second starfighter arrives immediately: cannot re-pickup before FLAG_REPICKUP_DELAY (0.5s).
    let (pilot_b, flyer_b) = spawn_seated_flyer(&mut sim, 0, drop_spot);
    sim.step(&[PadState::default()]);
    assert!(
        matches!(
            sim.ctf.as_ref().unwrap().flags[0].status,
            FlagStatus::Dropped { .. }
        ),
        "flag must remain dropped while pickup_cooldown > 0"
    );

    // Advance past FLAG_REPICKUP_DELAY while holding flyer_b at drop_spot.
    let steps = ((FLAG_REPICKUP_DELAY / (1.0 / 30.0)).ceil() as usize) + 2;
    for _ in 0..steps {
        sim.vehicle_mut(flyer_b).unwrap().pos = drop_spot;
        sim.step(&[PadState::default()]);
    }
    assert_eq!(
        sim.ctf.as_ref().unwrap().flags[0].status,
        FlagStatus::Carried { carrier: pilot_b }
    );

    // Deliver the flag into Team 0's capture zone (enemy capital-ship hangar) to win (score_limit = 1).
    sim.vehicle_mut(flyer_b).unwrap().pos = team0_goal - SPACE_CARRIED_OFFSET;
    sim.step(&[PadState::default()]);

    let ctf = sim.ctf.as_ref().unwrap();
    assert_eq!(ctf.team_points, [1, 0]);
    assert_eq!(ctf.flags[0].status, FlagStatus::AtHome);
    assert_eq!(sim.outcome, Some(0));
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::FlagCaptured { flag: 0, team: 0 })));
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::TeamDefeated { team: 1 })));
}

#[test]
fn two_flag_home_guard_touch_return_and_disabled_hash_invariance() {
    let base_a = Sim::empty(999);
    let base_b = Sim::empty(999);
    assert_eq!(base_a.hash(), base_b.hash());

    let mut sim = Sim::empty(999);
    let home0 = Vec3::new(-60.0, 0.0, 0.0);
    let home1 = Vec3::new(60.0, 0.0, 0.0);
    sim.install_ctf(CtfState::new_two_flag(3, home0, home1, home1, home0));
    assert_ne!(
        sim.hash(),
        base_a.hash(),
        "installing CTF state participates in Sim::hash"
    );

    // Friendly unit standing on its own home flag in 2-flag must NOT pick it up.
    let defender = sim.spawn(&UnitSpec::trooper(0, home0)).unwrap();
    sim.unit_mut(defender).unwrap().soldier.pos = home0;
    sim.step(&[PadState::default()]);
    assert_eq!(
        sim.ctf.as_ref().unwrap().flags[0].status,
        FlagStatus::AtHome,
        "friendly unit must not steal own home flag in TwoFlag mode"
    );

    // Enemy unit steals Team 0's flag, moves to origin, and dies.
    sim.unit_mut(defender).unwrap().soldier.pos = Vec3::new(-200.0, 0.0, 0.0);
    let thief = sim.spawn(&UnitSpec::trooper(1, home0)).unwrap();
    sim.unit_mut(thief).unwrap().soldier.pos = home0;
    sim.step(&[PadState::default()]);
    assert_eq!(
        sim.ctf.as_ref().unwrap().flags[0].status,
        FlagStatus::Carried { carrier: thief }
    );

    let death_pos = Vec3::new(12.0, 0.0, -4.0);
    sim.unit_mut(thief).unwrap().soldier.pos = death_pos;
    sim.unit_mut(thief).unwrap().alive = false;
    sim.step(&[PadState::default()]);
    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].current_pos, death_pos);

    // Wait out the 0.5s repickup delay, then move friendly defender onto the dropped flag for Touch Return.
    for _ in 0..20 {
        sim.step(&[PadState::default()]);
    }
    sim.unit_mut(defender).unwrap().soldier.pos = death_pos;
    sim.step(&[PadState::default()]);

    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::AtHome);
    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].current_pos, home0);
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::FlagReturned { flag: 0, team: 0 })));
}
