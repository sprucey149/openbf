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

/// Map turrets have placed bodies and a health binding, but no dynamic hull.
fn building_turret(layers: Layers) -> (Sim, UnitHandle, crate::vehicle::VehicleHandle) {
    let (mut sim, owner, other) = scene();
    sim.weapon_classes[0] = weapon();
    sim.weapon_classes[0].ordnance.scales.fill(0.0);
    sim.unit_mut(owner).unwrap().weapon.melee = None;
    sim.unit_mut(other).unwrap().soldier.pos = Vec3::X * 50.0;
    for _ in 0..2 {
        let mut body = Body::with_layers(
            Mat4::from_translation(Vec3::new(0.0, 0.9, -0.8)),
            Shape::Sphere { radius: 0.2 },
            layers,
        );
        body.instance = Some(27);
        sim.collision.push_body(body);
    }
    sim.collision.rebuild_broadphase();
    let turret = sim.add_building_turret(
        VehicleSpec {
            kind: VehicleKind::Turret,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 0.0, -0.8),
            yaw: 0.0,
            max_health: 150.0,
            health_type: HealthType::Building,
            seats: vec![SeatKind::Gunner(0)],
        },
        glam::Quat::IDENTITY,
        None,
        27,
        (crate::sim::TurretFall::Hidden, vec![]),
    );
    assert_eq!(sim.collision.turret(27), Some(turret));
    assert_eq!(sim.vehicle(turret).unwrap().team, None);
    assert!(sim.collision.bodies_and_hulls().all(|b| b.owner.is_none()));
    (sim, owner, turret)
}

#[test]
fn bound_building_turret_takes_one_hand_hit_per_attack_then_falls_and_unbinds() {
    for layers in [Layers::SOLDIER, Layers::ORDNANCE] {
        let (mut sim, owner, turret) = building_turret(layers);
        let swing = |sim: &mut Sim| {
            sim.unit_mut(owner).unwrap().weapon.melee = None;
            sim.saber_inputs(&mut [PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            }]);
            sim.saber_hits();
        };
        swing(&mut sim);
        sim.saber_hits();
        let hittable = layers == Layers::ORDNANCE;
        assert_eq!(
            sim.vehicle(turret).unwrap().health.cur,
            if hittable { 50.0 } else { 150.0 },
            "{layers:?}: two bodies and repeated query share one attack cache"
        );
        let hits = |sim: &Sim| {
            sim.events
                .iter()
                .filter(|e| {
                    matches!(e,
            crate::sim::Event::VehicleHit { shooter, vehicle, .. }
                if *shooter == Some(owner) && *vehicle == turret)
                })
                .count()
        };
        assert_eq!(hits(&sim), usize::from(hittable));
        swing(&mut sim);
        if hittable {
            assert!(sim.vehicle(turret).is_none());
            assert!(sim.building_down(27));
            assert_eq!(sim.collision.turret(27), None);
            assert_eq!(hits(&sim), 2);
            assert_eq!(
                sim.events
                    .iter()
                    .filter(|e| matches!(e,
                crate::sim::Event::VehicleDestroyed { vehicle, killer, .. }
                    if *vehicle == turret && *killer == Some(owner)))
                    .count(),
                1
            );
            swing(&mut sim);
            assert_eq!(hits(&sim), 2, "fallen binding cannot be hit again");
        } else {
            assert!(!sim.building_down(27));
            assert_eq!(sim.collision.turret(27), Some(turret));
            assert_eq!(sim.vehicle(turret).unwrap().health.cur, 150.0);
        }
    }
}

#[test]
fn bound_building_turret_bolt_control_uses_authored_building_scale() {
    for layers in [Layers::SOLDIER, Layers::ORDNANCE] {
        let (mut sim, owner, turret) = building_turret(layers);
        sim.weapon_classes[1].ordnance.max_damage = 40.0;
        sim.weapon_classes[1].ordnance.falloff = None;
        sim.weapon_classes[1].ordnance.scales[HealthType::Building as usize] = 0.25;
        let bolt = Bolt::fire(
            Vec3::new(0.0, 0.9, 0.5),
            Vec3::NEG_Z,
            100.0,
            1.0,
            Shooter {
                owner,
                team: 0,
                weapon_class: 1,
                is_player: true,
            },
            None,
            None,
            &sim.collision,
            HullFilter::All,
        );
        let hittable = layers == Layers::ORDNANCE;
        assert_eq!(
            bolt.hit,
            if hittable {
                Hit::Target(Target::Vehicle(turret))
            } else {
                Hit::None
            }
        );
        sim.resolve_bolt(&bolt);
        assert_eq!(
            sim.vehicle(turret).unwrap().health.cur,
            if hittable { 140.0 } else { 150.0 }
        );
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
