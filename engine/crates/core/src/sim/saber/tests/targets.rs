//! Physical contacts use the same blade queries for either controller.

use super::*;
use crate::collision::{Body, HullPart, Layers, Shape};
use crate::sim::stations::{Station, StationClass};
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::{Health, HealthType};
use glam::Mat4;

fn weapon() -> WeaponClass {
    saber(program(vec![
        field(K::Animation, vec![text("swing")], vec![]),
        field(
            K::Attack,
            vec![],
            vec![
                field(
                    K::DamageTime,
                    vec![V::Number(0.0), V::Number(30.0), text("Frames")],
                    vec![],
                ),
                field(K::Damage, vec![V::Number(100.0)], vec![]),
            ],
        ),
    ]))
}

#[test]
fn physical_melee_hits_unperceived_bystanders_for_ai_and_player() {
    for ai in [false, true] {
        let (mut sim, owner, victim) = scene();
        sim.weapon_classes[0] = weapon();
        let u = sim.unit_mut(owner).unwrap();
        u.weapon.melee = None;
        if ai {
            u.controller = crate::unit::Controller::Ai;
        }
        assert!(u.ai.contacts.iter().all(Option::is_none));
        sim.unit_mut(victim).unwrap().soldier.pos = Vec3::NEG_Z;
        let before = sim.unit(victim).unwrap().health.cur;
        sim.saber_inputs(&mut [PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        sim.saber_hits();
        assert_eq!(
            sim.unit(victim).unwrap().health.cur,
            before - 100.0,
            "ai={ai}"
        );
    }
}

#[test]
fn a_swing_damages_each_vehicle_and_station_pool_once_with_native_unit_scales() {
    for hittable in [true, false] {
        let (mut sim, owner, _) = scene();
        sim.weapon_classes[0] = weapon();
        sim.weapon_classes[0].ordnance.scales.fill(0.0);
        sim.unit_mut(owner).unwrap().weapon.melee = None;
        let layers = if hittable {
            Layers::ORDNANCE
        } else {
            Layers::SOLDIER
        };
        let vehicle = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(1),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(0.15, 0.9, -0.8),
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
        let mut station = Station::new(class, Vec3::new(-0.15, 0.9, -0.8), 0.0);
        station.instance = Some(17);
        station.health = Health::new(500.0, HealthType::Building);
        sim.stations.push(station);
        let mut body = Body::with_layers(
            Mat4::from_translation(station.pos),
            Shape::Sphere { radius: 0.1 },
            layers,
        );
        body.instance = Some(17);
        sim.collision.push_body(body);
        sim.collision.bind_station(17, 0);
        sim.saber_inputs(&mut [PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        sim.saber_hits();
        sim.saber_hits();
        let expected = if hittable { 400.0 } else { 500.0 };
        assert_eq!(sim.vehicle(vehicle).unwrap().health.cur, expected);
        assert_eq!(sim.stations[0].health.cur, expected);
    }
}

/// A blade cuts a remote droid (the maintainer's turret report, 2026-10-30):
/// the PS2's gather holds neither, so [ours]; the wielder's own is spared.
#[test]
fn the_blade_cuts_an_enemys_remote_droid_but_not_his_own() {
    for own in [false, true] {
        let (mut sim, owner, other) = scene();
        sim.weapon_classes[0] = weapon();
        sim.unit_mut(owner).unwrap().weapon.melee = None;
        sim.unit_mut(other).unwrap().soldier.pos = Vec3::X * 50.0;
        let builder = if own { owner } else { other };
        let team = sim.unit(builder).unwrap().team;
        let blaster = crate::weapon::WeaponState::with_clips(&sim.weapon_classes[1], false, 0);
        sim.droids.push(crate::remote::Droid {
            id: 7,
            owner: builder,
            team,
            weapon_class: 1,
            pos: Vec3::new(0.0, 0.9, -0.8),
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            up: Vec3::Y,
            up_vel: Vec3::ZERO,
            health: 300.0,
            age: 0.0,
            controlled: false,
            idle: 0.0,
            prev_buttons: 0,
            blaster,
            charge: None,
            leaving: None,
            adrift: 0.0,
        });
        sim.saber_inputs(&mut [PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        sim.saber_hits();
        let contacts = sim
            .events
            .iter()
            .filter(|e| matches!(e, crate::sim::Event::SaberContact { .. }))
            .count();
        if own {
            assert_eq!((sim.droids[0].health, contacts), (300.0, 0));
        } else {
            assert_eq!((sim.droids[0].health, contacts), (200.0, 1));
        }
    }
}
