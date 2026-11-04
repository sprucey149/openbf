use super::*;
use crate::collision::{HullPart, Layers, Shape};
use crate::unit::UnitSpec;
use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
use crate::weapon::HealthType;
use glam::Mat4;

fn yields(a: &Vehicle, b: &Vehicle, ra: f32, rb: f32) -> bool {
    super::yields(a, b, ra, rb, b.pos - a.pos)
}

fn spec(kind: VehicleKind, pos: Vec3) -> VehicleSpec {
    VehicleSpec {
        kind,
        pos,
        class: 0,
        yaw: 0.0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
}

#[test]
fn traffic_priority_uses_size_then_strict_normalized_motion_in_both_directions() {
    let mut a = spec(VehicleKind::Walker, Vec3::ZERO).build(0);
    let mut b = spec(VehicleKind::Hover, Vec3::Z * 4.0).build(0);
    for other in [VehicleKind::Walker, VehicleKind::Hover] {
        b.kind = other;
        assert!(!yields(&a, &b, 1.0, 1.0), "stationary equal-size");
        assert!(yields(&a, &b, 1.0, 3.01), "size is tested before speed");
        assert!(!yields(&a, &b, 3.01, 1.0));
        assert!(!yields(&a, &b, 1.0, 3.0), "strict radius threshold");
        a.vel = Vec3::Z;
        b.vel = -Vec3::Z;
        assert!(!yields(&a, &b, 1.0, 1.0));
        assert!(
            !yields(&b, &a, 1.0, 1.0),
            "opposing tie is not mutual yield"
        );
        b.vel = Vec3::Z * 2.0;
        assert!(
            yields(&a, &b, 1.0, 1.0),
            "rear vehicle yields, regardless of speed"
        );
        assert!(!yields(&b, &a, 1.0, 1.0));
        b.vel = Vec3::X;
        assert!(yields(&a, &b, 1.0, 1.0), "crossing priority");
        assert!(!yields(&b, &a, 1.0, 1.0));
        b.vel = Vec3::ZERO;
        assert!(!yields(&a, &b, 1.0, 1.0));
        a.vel = Vec3::ZERO;
    }
    b.kind = VehicleKind::Flyer;
    assert!(
        !yields(&a, &b, 1.0, 100.0),
        "not a ground traffic participant"
    );
}

fn contact_world() -> (Sim, VehicleHandle, VehicleHandle) {
    let mut s = Sim::empty(1);
    s.seat_setup = vec![Some(SeatSetup {
        radius: 1.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 8,
        guns: vec![],
    })];
    s.hull_parts = vec![Some(
        vec![HullPart::new(
            Mat4::IDENTITY,
            Shape::Sphere { radius: 1.0 },
            Layers::SOLDIER,
        )]
        .into(),
    )];
    let a = s.spawn_vehicle(&spec(VehicleKind::Walker, Vec3::ZERO));
    let b = s.spawn_vehicle(&spec(VehicleKind::Hover, Vec3::Z * 4.0));
    for vehicle in [a, b] {
        let pilot = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, s.vehicle(vehicle).unwrap().pos)
            })
            .unwrap();
        assert!(s.seat_unit(pilot, vehicle, 0));
    }
    (s, a, b)
}

#[test]
fn traffic_hold_requires_real_contact_refreshes_and_expires_on_fixed_ticks() {
    let (mut s, a, b) = contact_world();
    s.tick = 10;
    s.vehicle_mut(a).unwrap().vel = Vec3::Z;
    s.vehicle_mut(b).unwrap().vel = Vec3::X;
    // No touching: same priority does not start an anticipatory timer.
    s.vehicle_mut(a).unwrap().pos = Vec3::Z;
    s.ground_traffic(a.index as usize, Vec3::ZERO);
    assert_eq!(s.vehicle(a).unwrap().yield_until, 0);
    // A whole-step sweep cannot tunnel through either sphere.
    s.sync_hulls();
    s.vehicle_mut(a).unwrap().pos = Vec3::Z * 10.0;
    s.ground_traffic(a.index as usize, Vec3::Z);
    assert_eq!(s.vehicle(a).unwrap().yield_until, 10 + HOLD_TICKS);
    assert!(s.vehicle(a).unwrap().pos.z < 2.001);
    assert_eq!(s.vehicle(b).unwrap().yield_until, 0);
    // A later qualifying impact refreshes the deadline, using the same writer.
    s.tick = 50;
    let from = s.vehicle(a).unwrap().pos;
    s.vehicle_mut(a).unwrap().vel = Vec3::Z;
    s.vehicle_mut(a).unwrap().pos = Vec3::Z * 3.0;
    s.ground_traffic(a.index as usize, from);
    assert_eq!(s.vehicle(a).unwrap().yield_until, 50 + HOLD_TICKS);
    let held = s.hash();
    s.vehicle_mut(a).unwrap().yield_until += 1;
    assert_ne!(held, s.hash(), "deadline must be hashed");
    s.vehicle_mut(a).unwrap().yield_until -= 1;
    // Production tick clears stale deadlines even with no AI controller.
    s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
    s.tick = 50 + HOLD_TICKS;
    s.tick_walkers(crate::TICK_DT);
    assert_eq!(s.vehicle(a).unwrap().yield_until, 0);
}

#[test]
fn traffic_sweeps_allow_separation_and_do_not_yield_to_a_distant_second_hull() {
    let (mut s, a, b) = contact_world();
    let from = s.vehicle(a).unwrap().pos;
    s.vehicle_mut(a).unwrap().pos = -Vec3::Z * 10.0;
    s.ground_traffic(a.index as usize, from);
    assert_eq!(s.vehicle(a).unwrap().pos, -Vec3::Z * 10.0);
    assert_eq!(s.vehicle(a).unwrap().yield_until, 0);
    assert_eq!(s.vehicle(b).unwrap().yield_until, 0);
}

#[test]
fn traffic_sweeps_test_contact_separation_nearest_priority_and_mesh_movers() {
    let (mut s, a, _) = contact_world();
    s.vehicle_mut(a).unwrap().pos = Vec3::Z * 2.0;
    s.sync_hulls();
    s.vehicle_mut(a).unwrap().pos = Vec3::Z;
    s.ground_traffic(a.index as usize, Vec3::Z * 2.0);
    assert_eq!(
        s.vehicle(a).unwrap().pos,
        Vec3::Z,
        "separate from exact contact"
    );

    let (mut s, a, _) = contact_world();
    let mut large = s.seat_setup[0].clone().unwrap();
    large.radius = 4.0;
    s.seat_setup.push(Some(large));
    s.hull_parts.push(s.hull_parts[0].clone());
    let mut far = spec(VehicleKind::Walker, Vec3::Z * 8.0);
    far.class = 1;
    let far = s.spawn_vehicle(&far);
    s.vehicle_mut(a).unwrap().vel = Vec3::Z;
    s.vehicle_mut(a).unwrap().pos = Vec3::Z * 10.0;
    s.ground_traffic(a.index as usize, Vec3::ZERO);
    assert_eq!(
        s.vehicle(a).unwrap().yield_until,
        0,
        "only the equal-size first contact decides"
    );
    assert_eq!(s.vehicle(far).unwrap().yield_until, 0);

    let (mut s, a, _) = contact_world();
    let mesh = crate::collision::box_mesh(Vec3::ZERO, Vec3::ONE, false).shape;
    s.hull_parts[0] = Some(vec![HullPart::new(Mat4::IDENTITY, mesh, Layers::SOLDIER)].into());
    s.sync_hulls();
    s.vehicle_mut(a).unwrap().pos = Vec3::Z * 10.0;
    s.ground_traffic(a.index as usize, Vec3::ZERO);
    assert!(
        s.vehicle(a).unwrap().pos.z <= 2.001,
        "mesh mover must not tunnel"
    );
}

#[test]
fn traffic_player_and_unpiloted_vehicles_keep_the_existing_contact_response() {
    for kind in [VehicleKind::Walker, VehicleKind::Hover] {
        for controller in [
            Some(Controller::Player { slot: 0 }),
            Some(Controller::Idle),
            None,
        ] {
            let (mut s, a, b) = contact_world();
            s.vehicle_mut(a).unwrap().kind = kind;
            s.vehicle_mut(b).unwrap().kind = VehicleKind::Walker;
            let pilot = s
                .vehicle(a)
                .unwrap()
                .pilot_seat()
                .unwrap()
                .occupant
                .unwrap();
            if let Some(controller) = controller {
                s.unit_mut(pilot).unwrap().controller = controller;
            } else {
                s.unseat_unit(pilot);
            }
            let from = s.vehicle(a).unwrap().pos;
            let attempted = Vec3::Z * 10.0;
            let velocity = Vec3::Z * 3.0;
            s.vehicle_mut(a).unwrap().pos = attempted;
            s.vehicle_mut(a).unwrap().vel = velocity;
            s.vehicle_mut(b).unwrap().vel = Vec3::X;
            s.ground_traffic(a.index as usize, from);
            let actual = s.vehicle(a).unwrap();
            assert_eq!(
                actual.pos, attempted,
                "{kind:?} {controller:?}: changed displacement"
            );
            assert_eq!(
                actual.vel, velocity,
                "{kind:?} {controller:?}: changed velocity"
            );
            assert_eq!(actual.yield_until, 0, "non-AI mover must not get a hold");
        }
    }
}

#[test]
fn traffic_ai_contact_does_not_start_a_hold_on_the_other_player_walker() {
    let (mut s, a, b) = contact_world();
    s.vehicle_mut(b).unwrap().kind = VehicleKind::Walker;
    let pilot = s
        .vehicle(b)
        .unwrap()
        .pilot_seat()
        .unwrap()
        .occupant
        .unwrap();
    s.unit_mut(pilot).unwrap().controller = Controller::Player { slot: 0 };
    s.vehicle_mut(a).unwrap().vel = Vec3::X;
    s.vehicle_mut(b).unwrap().vel = -Vec3::Z;
    // Crossing priority makes the other (player) walker yield.
    s.vehicle_mut(a).unwrap().pos = Vec3::Z * 10.0;
    s.ground_traffic(a.index as usize, Vec3::ZERO);
    assert_eq!(s.vehicle(b).unwrap().yield_until, 0);
    assert!(
        s.vehicle(a).unwrap().pos.z < 2.001,
        "AI clamp remains active"
    );
}
