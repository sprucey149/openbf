//! Static station bodies and dynamic vehicle hulls share the flight query.

use super::*;
use crate::collision::{Body, HullPart, Layers, Shape};
use crate::sim::stations::{Station, StationClass};
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::{Health, HealthType};

#[test]
fn thrown_saber_hits_map_and_vehicle_pools_once_and_turns_back_with_authored_scales() {
    for (layers, target_team, factor) in [
        (Layers::ORDNANCE, 1, 1.0),
        (Layers::SOLDIER, 1, 0.0),
        (Layers::ORDNANCE, 0, 0.25),
    ] {
        let (mut sim, owner, _) = scene();
        sim.friendly_fire_scale = 0.25;
        sim.weapon_classes[1].ordnance.scales[HealthType::Vehicle as usize] = 0.2;
        sim.weapon_classes[1].ordnance.scales[HealthType::Building as usize] = 0.5;
        let vehicle = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(target_team),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(0.25, 1.15, -1.0),
            yaw: 0.0,
            max_health: 500.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        sim.collision.set_hull(
            vehicle,
            &[HullPart::new(
                Mat4::IDENTITY,
                Shape::Sphere { radius: 0.1 },
                layers,
            )],
            Mat4::from_translation(sim.vehicle(vehicle).unwrap().pos),
        );
        let class = sim.add_station_class(StationClass::soldier("fixture", 1.0, 0.0, 0));
        let mut station = Station::new(class, Vec3::new(-0.25, 1.15, -1.0), 0.0);
        station.instance = Some(17);
        station.team = Some(target_team);
        station.health = Health::new(500.0, HealthType::Building);
        let mut body = Body::with_layers(
            Mat4::from_translation(station.pos),
            Shape::Sphere { radius: 0.1 },
            layers,
        );
        body.instance = Some(17);
        sim.stations.push(station);
        sim.collision.push_body(body);
        sim.collision.bind_station(17, 0);
        sim.launch_saber_throw(owner.index as usize, Some(0), 1);
        for _ in 0..10 {
            sim.tick_saber_throws();
        }
        assert_eq!(
            sim.vehicle(vehicle).unwrap().health.cur,
            500.0 - 40.0 * 0.2 * factor
        );
        assert_eq!(sim.stations[0].health.cur, 500.0 - 40.0 * 0.5 * factor);
    }
}

#[test]
fn bound_building_turret_takes_thrown_damage_with_authored_scale_without_a_hull() {
    for layers in [Layers::SOLDIER, Layers::ORDNANCE] {
        let (mut sim, owner, victim) = scene();
        sim.unit_mut(victim).unwrap().soldier.pos = Vec3::X * 50.0;
        sim.weapon_classes[1].ordnance.scales[HealthType::Building as usize] = 0.5;
        for _ in 0..2 {
            let mut body = Body::with_layers(
                Mat4::from_translation(Vec3::new(0.0, 1.15, -1.0)),
                Shape::Sphere { radius: 0.1 },
                layers,
            );
            body.instance = Some(27);
            sim.collision.push_body(body);
        }
        let turret = sim.add_building_turret(
            VehicleSpec {
                kind: VehicleKind::Turret,
                class: 0,
                home_team: None,
                neutral_when_empty: true,
                spawn_pad: None,
                pos: Vec3::new(0.0, 0.0, -1.0),
                yaw: 0.0,
                max_health: 500.0,
                health_type: HealthType::Building,
                seats: vec![SeatKind::Gunner(0)],
            },
            glam::Quat::IDENTITY,
            None,
            27,
            (crate::sim::TurretFall::Hidden, vec![]),
        );
        assert!(sim.collision.bodies_and_hulls().all(|b| b.owner.is_none()));
        assert_eq!(sim.collision.turret(27), Some(turret));
        sim.launch_saber_throw(owner.index as usize, Some(0), 1);
        for _ in 0..10 {
            sim.tick_saber_throws();
        }
        let hittable = layers == Layers::ORDNANCE;
        assert_eq!(
            sim.vehicle(turret).unwrap().health.cur,
            if hittable { 480.0 } else { 500.0 }
        );
        assert_eq!(
            sim.events
                .iter()
                .filter(|e| matches!(e,
            crate::sim::Event::VehicleHit { shooter, vehicle, .. }
                if *shooter == Some(owner) && *vehicle == turret))
                .count(),
            usize::from(hittable)
        );
    }
}
