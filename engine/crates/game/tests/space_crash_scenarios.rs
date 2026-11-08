//! Sub-second deterministic micro-scenarios for Space Battles Issue #124:
//! Flyer crashes: falling wrecks, impact feedback and death-view acceptance.
//!
//! Validates:
//! - Floor, ceiling, capital hull, and craft-to-craft collisions with exact impulse and damage formulas.
//! - Takeoff collision obstacle destruction and Falling -> Crashed instant transition on hull contact.
//! - Camera crash detach rate (0.25) and death view hold (4.0s).
//! - Blackbox `FlyerRow` phase progression (Flying -> Falling -> Crashed).

use core::collision::{Body, CollisionWorld, HullPart, Layers, Shape};
use core::sim::{Event, PadState, Sim};
use core::unit::UnitSpec;
use core::vehicle::flyer::{FlightPhase, FlyerParams};
use core::vehicle::{SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use core::weapon::{DamageDesc, HealthType};
use game::blackbox::FlyerRow;
use game::vehicle_class::SeatSpec;
use game::walk::{WalkSim, DEATH_VIEW_HOLD, FLYER_CRASH_DETACH_RATE};
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

const TEST_HEALTH: f32 = 1500.0;
const NOSE_DAMAGE: f32 = 0.5;

fn test_flyer_params() -> FlyerParams {
    FlyerParams {
        bf2: true,
        has_chunks: true,
        landed_height: 1.0,
        takeoff_height: 2.5,
        takeoff_time: 1.0,
        acceleration: 30.0,
        min_speed: 20.0,
        mid_speed: 80.0,
        max_speed: 105.0,
        boost_speed: 170.0,
        collision_threshold: [5.0, 2.5, 0.0],
        collision_scale: [1.5, 0.0, 0.0],
        ..FlyerParams::default()
    }
}

fn box_at(center: Vec3, half: Vec3) -> Body {
    let mut b = Body::new(Mat4::from_translation(center), Shape::Box { half });
    b.layers = Layers::FLYER | Layers::SOLDIER;
    b
}

fn floor_body() -> Body {
    box_at(Vec3::new(0.0, -1.0, 0.0), Vec3::new(500.0, 1.0, 500.0))
}

fn ceiling_body() -> Body {
    box_at(Vec3::new(0.0, 100.0, 0.0), Vec3::new(500.0, 1.0, 500.0))
}

fn capital_hull_body() -> Body {
    box_at(Vec3::new(0.0, 0.0, 100.0), Vec3::new(50.0, 50.0, 2.0))
}

fn setup_sim_flyer(world: Vec<Body>) -> (Sim, VehicleHandle) {
    let mut sim = Sim::empty(42);
    sim.collision = CollisionWorld::new(world, Vec::new());
    sim.flyer_params = vec![Some(test_flyer_params())];
    sim.hull_parts = vec![Some(Arc::from(vec![HullPart::new(
        Mat4::IDENTITY,
        Shape::Sphere { radius: 4.0 },
        Layers::VEHICLE,
    )
    .with_flyer_subsets(true, true)]))];

    let pilot = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: TEST_HEALTH,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    assert!(sim.seat_unit(pilot, vh, 0));
    let v = sim.vehicle_mut(vh).unwrap();
    v.flyer.phase = FlightPhase::Flying;
    v.flyer.lift = 1.0;
    (sim, vh)
}

fn spawn_ground_vehicle(sim: &mut Sim, team: u8, pos: Vec3) -> VehicleHandle {
    let spec = VehicleSpec {
        kind: VehicleKind::Hover,
        class: 1,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 800.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    };
    sim.spawn_vehicle(&spec)
}

#[test]
fn floor_collision_calculates_exact_restitution_damage_and_impact_event() {
    let (mut sim, vh) = setup_sim_flyer(vec![floor_body()]);

    let vel = Vec3::new(0.0, -10.0, 80.0);
    let rot = Quat::from_rotation_arc(Vec3::Z, vel.normalize());
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.rot = rot;
        v.pos = Vec3::Y * 3.5;
        v.vel = vel;
    }

    sim.step(&[]);

    // Verify FlyerImpact event was emitted
    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::FlyerImpact { vehicle, .. } if *vehicle == vh));
    assert!(
        impact.is_some(),
        "Event::FlyerImpact must be emitted on floor collision"
    );
    let (s, d) = match impact {
        Some(Event::FlyerImpact {
            speed,
            damage,
            lethal,
            ..
        }) => {
            assert!(!lethal, "scrape is non-lethal");
            assert!(
                (*speed - 10.0).abs() < 0.2,
                "impact speed is closing speed ({speed})"
            );
            (*speed, *damage)
        }
        _ => unreachable!(),
    };

    let v = sim
        .vehicle(vh)
        .expect("flyer survives non-lethal floor scrape");
    assert!(v.alive);
    // Vertical velocity rebounded upward by restitution factor -1.4:
    // pre-impact along was -s, post-bounce vel.y = -s + 1.4 * s = 0.4 * s
    let expected_vy = 0.4 * s;
    assert!(
        (v.vel.y - expected_vy).abs() < 1e-2,
        "vel.y was {}, expected {}",
        v.vel.y,
        expected_vy
    );
    assert!(v.vel.z < 80.0 && v.vel.z > 70.0, "vel.z was {}", v.vel.z);

    // Damage verification:
    let nose = (rot * Vec3::Z).dot(Vec3::NEG_Y);
    let up_y = (rot * Vec3::Y).y;
    let threshold = 5.0 + 2.5 * up_y;
    let speed_excess = (s - threshold).max(0.0);
    let expected_damage = speed_excess * speed_excess * 1.5 + nose * NOSE_DAMAGE * TEST_HEALTH;
    let expected_hp = TEST_HEALTH - expected_damage;
    assert!(
        (v.health.cur - expected_hp).abs() < 1.0,
        "health was {}, expected {}",
        v.health.cur,
        expected_hp
    );
    assert!(
        (d - expected_damage).abs() < 1.0,
        "impact damage matches formula"
    );
}

#[test]
fn ceiling_collision_transitions_takeoff_to_landing_and_bounces_flying() {
    let (mut sim, vh) = setup_sim_flyer(vec![ceiling_body()]);

    // 1. Takeoff phase hitting ceiling transitions to Landing
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.flyer.phase = FlightPhase::Takeoff;
        v.flyer.lift = 0.2;
        v.flyer.initialized = true;
        v.pos = Vec3::Y * 96.0;
        v.vel = Vec3::Y * 5.0;
    }

    sim.step(&[]);

    let v = sim.vehicle(vh).unwrap();
    assert_eq!(
        v.flyer.phase,
        FlightPhase::Landing,
        "ceiling contact during Takeoff must transition craft to Landing"
    );

    // 2. Flying phase hitting ceiling reflects velocity downward
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.pos = Vec3::Y * 96.0;
        v.vel = Vec3::new(0.0, 10.0, 50.0);
    }

    sim.step(&[]);

    let v = sim.vehicle(vh).unwrap();
    assert!(
        v.vel.y < 0.0,
        "ceiling contact during Flying must deflect vertical speed downward, got {}",
        v.vel.y
    );
}

#[test]
fn capital_hull_head_on_impact_destroys_craft_with_nose_damage() {
    let (mut sim, vh) = setup_sim_flyer(vec![capital_hull_body()]);

    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.pos = Vec3::new(0.0, 0.0, 96.0);
        v.vel = Vec3::Z * 85.0; // High speed head-on into wall at Z=100
        v.rot = Quat::IDENTITY; // Nose pointing straight +Z into normal -Z
    }

    sim.step(&[]);

    // Lethal impact should immediately destroy the craft
    let v = sim.vehicle(vh);
    assert!(
        v.is_none_or(|v| !v.alive),
        "high speed head-on hull collision must destroy the flyer"
    );

    // Check that Event::FlyerImpact was emitted with lethal = true
    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::FlyerImpact { vehicle, .. } if *vehicle == vh));
    assert!(impact.is_some(), "Event::FlyerImpact must be emitted");
    if let Some(Event::FlyerImpact { lethal, damage, .. }) = impact {
        assert!(lethal, "head-on capital hull collision at 85m/s is lethal");
        assert!(*damage >= TEST_HEALTH, "damage exceeded max health");
    }
}

#[test]
fn craft_to_craft_collision_halves_impulse_and_scales_damage() {
    let (mut sim, v1) = setup_sim_flyer(Vec::new());

    let p2 = sim.spawn(&UnitSpec::player(1, Vec3::ZERO)).unwrap();
    let v2 = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(1),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 0.0, 5.0),
        yaw: 0.0,
        max_health: TEST_HEALTH,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    assert!(sim.seat_unit(p2, v2, 0));

    {
        let craft1 = sim.vehicle_mut(v1).unwrap();
        craft1.flyer.phase = FlightPhase::Flying;
        craft1.pos = Vec3::ZERO;
        craft1.vel = Vec3::Z * 40.0;

        let craft2 = sim.vehicle_mut(v2).unwrap();
        craft2.flyer.phase = FlightPhase::Flying;
        craft2.pos = Vec3::new(0.0, 0.0, 5.0);
        craft2.vel = Vec3::NEG_Z * 40.0;
    }

    sim.step(&[]);

    let craft1 = sim.vehicle(v1).unwrap();
    let craft2 = sim.vehicle(v2).unwrap();
    assert!(
        craft1.health.cur < TEST_HEALTH,
        "craft 1 took collision damage"
    );
    assert!(
        craft2.health.cur < TEST_HEALTH,
        "craft 2 took collision damage"
    );
    assert!(
        craft1.alive && craft2.alive,
        "crafts survive moderate glancing at 40m/s due to 0.2 damage scale"
    );
}

#[test]
fn takeoff_collision_destroys_non_flying_obstacle() {
    let (mut sim, flyer_h) = setup_sim_flyer(Vec::new());
    sim.hull_parts.push(Some(Arc::from(vec![HullPart::new(
        Mat4::IDENTITY,
        Shape::Sphere { radius: 4.0 },
        Layers::FLYER | Layers::VEHICLE,
    )
    .with_flyer_subsets(true, true)])));
    let ground_h = spawn_ground_vehicle(&mut sim, 1, Vec3::new(0.0, 3.0, 0.0));

    {
        let v = sim.vehicle_mut(flyer_h).unwrap();
        v.flyer.phase = FlightPhase::Takeoff;
        v.flyer.lift = 0.2;
        v.flyer.initialized = true;
        v.pos = Vec3::ZERO;
        v.vel = Vec3::Y * 10.0;
    }

    sim.step(&[]);

    let ground_v = sim.vehicle(ground_h);
    assert!(
        ground_v.is_none_or(|v| !v.alive),
        "SWBFRNT2.ELF FUN_0020bff8 line 121671: non-flying obstacle destroyed on takeoff collision"
    );
}

#[test]
fn falling_craft_instantly_transitions_to_crashed_on_hull_contact() {
    let (mut sim, vh) = setup_sim_flyer(vec![floor_body()]);

    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.flyer.phase = FlightPhase::Falling;
        v.flyer.crash_timer = 1.5;
        v.health.cur = 0.0;
        v.pos = Vec3::Y * 3.5;
        v.vel = Vec3::NEG_Y * 15.0; // Falling into the floor
    }

    sim.step(&[]);

    // Craft must transition immediately to Crashed / destroyed on hull contact
    let v = sim.vehicle(vh);
    assert!(
        v.is_none_or(|v| !v.alive),
        "falling craft must be destroyed immediately on hull contact"
    );
    if let Some(v) = v {
        assert_eq!(
            v.flyer.phase,
            FlightPhase::Crashed,
            "phase must be FlightPhase::Crashed"
        );
    }

    // Impact event should be lethal
    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::FlyerImpact { vehicle, .. } if *vehicle == vh));
    assert!(impact.is_some());
    if let Some(Event::FlyerImpact { lethal, .. }) = impact {
        assert!(lethal, "contact while Falling is lethal");
    }
}

#[test]
fn blackbox_phase_progression_camera_detach_and_death_view_hold() {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    walk.sim.flyer_params = vec![Some(test_flyer_params())];
    walk.sim.collision = CollisionWorld::new(vec![floor_body()], Vec::new());
    walk.sim.hull_parts = vec![Some(Arc::from(vec![HullPart::new(
        Mat4::IDENTITY,
        Shape::Sphere { radius: 4.0 },
        Layers::VEHICLE,
    )
    .with_flyer_subsets(true, true)]))];

    let vh = walk.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 50.0, 0.0),
        yaw: 0.0,
        max_health: TEST_HEALTH,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });

    let mut seat = SeatSpec::new(SeatKind::Pilot);
    seat.track_center = Some([0.0, 4.0, -8.0]);
    seat.track_offset = Some([0.0, 0.0, 0.0]);
    seat.move_tension = [Some([20.0, 20.0]), Some([20.0, 20.0]), Some([20.0, 20.0])];
    walk.vehicle_seats = vec![vec![seat]];

    let player = walk.player;
    assert!(walk.sim.seat_unit(player, vh, 0));

    // 1. Initial flying phase
    {
        let v = walk.sim.vehicle_mut(vh).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
        v.flyer.lift = 1.0;
        v.vel = Vec3::Z * 60.0;
    }
    walk.step_tick(&[PadState::default()]);

    let v = walk.sim.vehicle(vh).unwrap();
    let row1 = FlyerRow {
        phase: v.flyer.phase as u8,
        speed: v.vel.length(),
        channel: v.flyer.ai.channel,
        in_roll: v.flyer.in_roll,
        threat_kind: 0,
        threat_id: 0,
    };
    assert_eq!(row1.phase, 2, "phase 2 is Flying");

    // 2. Fatal weapon damage puts flyer into Falling (shot down)
    walk.sim.damage_vehicle(
        vh,
        &DamageDesc {
            damage: 2000.0,
            scales: [1.0; 7],
            shield_scale: 1.0,
        },
        1.0,
        None,
    );

    let v = walk.sim.vehicle(vh).unwrap();
    assert_eq!(
        v.flyer.phase,
        FlightPhase::Falling,
        "shot-down flyer enters Falling phase"
    );

    // Step a tick in Falling phase
    walk.step_tick(&[PadState::default()]);

    let v = walk.sim.vehicle(vh).unwrap();
    let row2 = FlyerRow {
        phase: v.flyer.phase as u8,
        speed: v.vel.length(),
        channel: v.flyer.ai.channel,
        in_roll: v.flyer.in_roll,
        threat_kind: 0,
        threat_id: 0,
    };
    assert_eq!(row2.phase, 4, "phase 4 is Falling");

    // Camera detach verification: flyer is falling, move_tension is scaled by FLYER_CRASH_DETACH_RATE
    assert_eq!(FLYER_CRASH_DETACH_RATE, 0.25);

    // 3. Craft hits ground and crashes
    {
        let v = walk.sim.vehicle_mut(vh).unwrap();
        v.pos = Vec3::new(0.0, 3.5, 0.0);
        v.vel = Vec3::NEG_Y * 20.0;
    }
    walk.step_tick(&[PadState::default()]);

    // Flyer is now destroyed / crashed
    assert_eq!(DEATH_VIEW_HOLD, 4.0);
    assert!(
        walk.seat_view_timer > 0.0,
        "seat view timer is active during death hold"
    );
    assert!(walk.seat_view_timer <= DEATH_VIEW_HOLD);

    // Death view camera is held
    let cam = walk.camera();
    assert!(
        cam.position.length() > 0.0,
        "camera held in place or tracking crash point"
    );

    // Advance 60 ticks (2.0s): death view should still be held
    for _ in 0..60 {
        walk.step_tick(&[PadState::default()]);
    }
    assert!(walk.seat_view_timer > 0.0 && walk.seat_view_timer < 2.5);

    // Advance 70 more ticks (~2.33s, total > 4.0s): death view timer expires
    for _ in 0..70 {
        walk.step_tick(&[PadState::default()]);
    }
    assert_eq!(
        walk.seat_view_timer, 0.0,
        "death view timer expires after 4.0s"
    );
}

#[test]
fn flyer_impact_sparks_and_controller_rumble() {
    let mut sim = Sim::empty(42);
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });

    let impact_pos = Vec3::new(10.0, 50.0, 10.0);
    let impact_normal = Vec3::Y;
    sim.events.push(Event::FlyerImpact {
        vehicle: vh,
        pos: impact_pos,
        normal: impact_normal,
        speed: 85.0,
        damage: 50.0,
        lethal: false,
    });

    let mut fx = game::fx::Fx::default();
    assert_eq!(fx.len(), 0);
    fx.ingest_sim(&sim);
    assert_eq!(fx.len(), 1, "FlyerImpact event produces spark / hit effect");

    // Also test lethal impact event produces effect
    sim.events.clear();
    sim.events.push(Event::FlyerImpact {
        vehicle: vh,
        pos: impact_pos,
        normal: impact_normal,
        speed: 150.0,
        damage: 2000.0,
        lethal: true,
    });
    fx.ingest_sim(&sim);
    assert_eq!(
        fx.len(),
        2,
        "lethal FlyerImpact event produces spark / hit effect"
    );

    // Verify gilrs rumble helper runs safely with gamepad
    if let Ok(mut g) = gilrs::Gilrs::new() {
        let ids: Vec<_> = g.gamepads().map(|(id, _)| id).collect();
        for id in ids {
            input::rumble(&mut g, id, 0.6, 0.4, 300);
        }
    }
}
