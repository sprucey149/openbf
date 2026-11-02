//! The maintainer, 2026-10-30: "Should go to its range limit and return, or go
//! until it hits collision and then it returns." [ours] over the PS2, whose
//! flight passes through soldiers, hulls and stations (only the launch's world
//! ray shortens the leg; FUN_0031cb78, FUN_0031d688).

use super::*;
use crate::collision::{Body, HullPart, Layers, Shape};
use crate::sim::stations::{Station, StationClass};
use crate::sim::PadState;
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::{Health, HealthType};
use crate::TICK_HZ;

/// One throw 40 m long (the shipped 30 m/s for 1.333 s), nothing in the way
/// unless the test puts it there. The first soldier is moved off the line.
fn long_throw() -> (Sim, UnitHandle, UnitHandle) {
    let (mut sim, owner, victim) = scene();
    let data = sim.weapon_classes[1].saber_throw.as_mut().unwrap();
    data.outbound_seconds = 1.333;
    data.return_seconds = 0.8;
    sim.unit_mut(victim).unwrap().soldier.pos = Vec3::new(80.0, 0.0, 80.0);
    (sim, owner, victim)
}

fn flight(sim: &Sim, owner: UnitHandle) -> Option<&SaberFlight> {
    sim.unit(owner).unwrap().held[0].state.saber_flight.as_ref()
}

/// How far out the blade goes and how many ticks it takes to come home.
fn fly(sim: &mut Sim, owner: UnitHandle) -> (f32, usize) {
    let start = sim.unit(owner).unwrap().soldier.pos;
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    let mut far = 0.0f32;
    for tick in 0..400 {
        sim.tick_saber_throws();
        let Some(f) = flight(sim, owner) else {
            return (far, tick);
        };
        far = far.max((f.pos - start).length());
    }
    panic!("a stuck flight");
}

fn stand_on_the_line(sim: &mut Sim, h: UnitHandle, metres: f32) {
    sim.unit_mut(h).unwrap().soldier.pos = Vec3::NEG_Z * metres;
}

#[test]
fn an_empty_line_flies_the_whole_range_and_comes_home() {
    let (mut sim, owner, _) = long_throw();
    sim.heightfield = None;
    let (far, _) = fly(&mut sim, owner);
    assert!((far - 40.0).abs() < 1.5, "{far}");
}

#[test]
fn an_enemy_is_flown_through_and_struck_once_each_way() {
    let (mut sim, owner, victim) = long_throw();
    stand_on_the_line(&mut sim, victim, 10.0);
    let before = sim.unit(victim).unwrap().health.cur;
    let (far, _) = fly(&mut sim, owner);
    assert!(far > 30.0, "flew on past the soldier, {far}");
    assert_eq!(sim.unit(victim).unwrap().health.cur, before - 80.0);
}

#[test]
fn a_team_mate_is_flown_through_under_the_friendly_fire_rule() {
    let (mut sim, owner, _) = long_throw();
    sim.friendly_fire_scale = 0.0;
    let mate = sim
        .spawn(&UnitSpec::trooper(0, Vec3::NEG_Z * 10.0))
        .unwrap();
    let before = sim.unit(mate).unwrap().health.cur;
    let (far, _) = fly(&mut sim, owner);
    assert!(far > 30.0, "{far}");
    assert_eq!(sim.unit(mate).unwrap().health.cur, before);
}

fn deka(sim: &mut Sim, metres: f32) -> UnitHandle {
    sim.spawn(&UnitSpec {
        droideka: Some(std::sync::Arc::new(Default::default())),
        ..UnitSpec::trooper(1, Vec3::NEG_Z * metres)
    })
    .unwrap()
}

#[test]
fn a_bare_droideka_is_flown_through_and_takes_the_damage_each_way() {
    let (mut sim, owner, _) = long_throw();
    let d = deka(&mut sim, 10.0);
    sim.unit_mut(d)
        .unwrap()
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .on = false;
    let before = sim.unit(d).unwrap().health.cur;
    let (far, _) = fly(&mut sim, owner);
    assert!(far > 30.0, "{far}");
    assert_eq!(sim.unit(d).unwrap().health.cur, before - 80.0);
    assert!(!sim.unit(d).unwrap().soldier.stance.knocked());
}

#[test]
fn a_shielded_droideka_stops_it_at_the_bubble_and_the_shield_takes_the_damage() {
    let (mut sim, owner, _) = long_throw();
    let d = deka(&mut sim, 10.0);
    let u = sim.unit_mut(d).unwrap();
    u.droideka.as_mut().unwrap().shield.on = true;
    u.droideka.as_mut().unwrap().shield.points = 500.0;
    let radius = u.droideka_shield().expect("a raised shield").radius;
    let before = sim.unit(d).unwrap().health.cur;
    let (far, _) = fly(&mut sim, owner);
    assert!(far < 10.0 - radius + 2.0, "at the bubble's face: {far}");
    let u = sim.unit(d).unwrap();
    assert_eq!(u.health.cur, before, "the bubble absorbs it");
    assert_eq!(u.droideka.as_ref().unwrap().shield.points, 460.0);
}

#[test]
fn a_vehicle_hull_ends_the_leg() {
    let (mut sim, owner, _) = long_throw();
    let vehicle = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(1),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 1.15, -10.0),
        yaw: 0.0,
        max_health: 500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    sim.collision.set_hull(
        vehicle,
        &[HullPart::new(
            Mat4::IDENTITY,
            Shape::Sphere { radius: 1.0 },
            Layers::ORDNANCE,
        )],
        Mat4::from_translation(sim.vehicle(vehicle).unwrap().pos),
    );
    let (far, _) = fly(&mut sim, owner);
    assert!(far < 11.0, "{far}");
    assert!(sim.vehicle(vehicle).unwrap().health.cur < 500.0);
}

#[test]
fn a_station_ends_the_leg() {
    let (mut sim, owner, _) = long_throw();
    let class = sim.add_station_class(StationClass::soldier("fixture", 1.0, 0.0, 0));
    let mut station = Station::new(class, Vec3::new(0.0, 1.15, -10.0), 0.0);
    station.instance = Some(17);
    station.team = Some(1);
    station.health = Health::new(500.0, HealthType::Building);
    let mut body = Body::with_layers(
        Mat4::from_translation(station.pos),
        Shape::Sphere { radius: 1.0 },
        Layers::ORDNANCE,
    );
    body.instance = Some(17);
    sim.stations.push(station);
    sim.collision.push_body(body);
    sim.collision.bind_station(17, 0);
    let (far, _) = fly(&mut sim, owner);
    assert!(far < 11.0, "{far}");
}

#[test]
fn a_wall_shortens_the_leg_and_a_wall_at_his_face_still_comes_home() {
    for (metres, reach) in [(10.0, 11.0), (0.0, 2.0)] {
        let (mut sim, owner, _) = long_throw();
        sim.collision.push_body(Body::with_layers(
            Mat4::from_translation(Vec3::new(0.0, 1.15, -metres)),
            Shape::Box {
                half: Vec3::new(20.0, 20.0, 0.25),
            },
            Layers::ORDNANCE,
        ));
        let (far, _) = fly(&mut sim, owner);
        assert!(far < reach, "{metres}: {far}");
    }
}

#[test]
fn a_touch_at_the_launch_comes_home_at_once_without_a_stuck_flight() {
    let (mut sim, owner, _) = long_throw();
    let d = deka(&mut sim, 1.5);
    let shield = &mut sim.unit_mut(d).unwrap().droideka.as_mut().unwrap().shield;
    shield.on = true;
    shield.points = 500.0;
    let (far, ticks) = fly(&mut sim, owner);
    assert!(far < 2.0, "{far}");
    assert!(ticks < 15, "{ticks}");
}

/// A raised bubble stops the blade and it turns for home: the leg's clock and the
/// return scale by the outbound seconds really spent (the native cut,
/// FUN_0031d658), so the return is never longer than authored.
#[test]
fn the_cut_leg_scales_the_return_like_the_native_cut() {
    let (mut sim, owner, _) = long_throw();
    let d = deka(&mut sim, 10.0);
    let shield = &mut sim.unit_mut(d).unwrap().droideka.as_mut().unwrap().shield;
    shield.on = true;
    shield.points = 500.0;
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    let data = sim.weapon_classes[1].saber_throw.clone().unwrap();
    for _ in 0..400 {
        sim.tick_saber_throws();
        let f = flight(&sim, owner).expect("not home before it turned");
        if f.returning {
            assert_eq!(f.remaining.min(0.0), f.remaining);
            assert!(f.outbound_seconds < 0.36, "{}", f.outbound_seconds);
            assert!(f.return_duration(&data) < 0.8 * 0.36 / 1.333 + 0.01);
            return;
        }
    }
    panic!("never turned");
}

/// The blade's contact reaches the fx layer once a leg, at the soldier, and its hit
/// sound plays at him (FUN_0031d688's tail, B2:312042-312046).
#[test]
fn a_thrown_hit_reports_its_contact_and_sound() {
    let (mut sim, owner, victim) = long_throw();
    stand_on_the_line(&mut sim, victim, 10.0);
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    let (mut contacts, mut sounds) = (Vec::new(), Vec::new());
    for _ in 0..400 {
        sim.events.clear();
        sim.tick_saber_throws();
        for e in &sim.events {
            match *e {
                Event::SaberContact { pos, deflect, .. } => contacts.push((pos, deflect)),
                Event::SaberThrowHit { pos, .. } => sounds.push(pos),
                _ => {}
            }
        }
        if flight(&sim, owner).is_none() {
            break;
        }
    }
    assert_eq!(contacts.len(), 2, "{contacts:?}");
    assert!(!contacts[0].1);
    let body = Vec3::NEG_Z * 10.0;
    assert!((contacts[0].0 - body).length() < 2.5, "{:?}", contacts[0]);
    assert_eq!(sounds, vec![body, body]);
}

/// Turning is not locked by a saber in the air, at full look speed.
#[test]
fn the_thrower_turns_at_full_speed_while_it_flies() {
    let (mut sim, owner, _) = long_throw();
    let pad = PadState {
        rx: 20000,
        ..Default::default()
    };
    let (mut free, _, _) = long_throw();
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    let a = sim.unit(owner).unwrap().soldier.yaw;
    for _ in 0..5 {
        sim.step(&[pad]);
        free.step(&[pad]);
    }
    let turned = sim.unit(owner).unwrap().soldier.yaw - a;
    assert_ne!(turned, 0.0);
    assert_eq!(turned, free.unit(owner).unwrap().soldier.yaw - a);
}

/// Past a range-end turn or a wall's the return is the PS2's fresh leg: a
/// soldier it passes then is struck, and that ends nothing.
#[test]
fn the_return_leg_still_strikes_what_it_passes_and_keeps_flying() {
    let (mut sim, owner, victim) = long_throw();
    let data = sim.weapon_classes[1].saber_throw.as_mut().unwrap();
    data.outbound_seconds = 0.2;
    let before = sim.unit(victim).unwrap().health.cur;
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    for _ in 0..TICK_HZ as usize {
        sim.tick_saber_throws();
        let Some(f) = flight(&sim, owner) else { break };
        if f.returning {
            let at = f.pos - Vec3::Y * 1.15;
            sim.unit_mut(victim).unwrap().soldier.pos = at;
            break;
        }
    }
    for _ in 0..TICK_HZ as usize {
        sim.tick_saber_throws();
        let u = sim.unit(owner).unwrap();
        if u.held[0].state.saber_flight.is_none() {
            break;
        }
        assert!(u.held[0].state.saber_flight.as_ref().unwrap().returning);
    }
    assert_eq!(sim.unit(victim).unwrap().health.cur, before - 40.0);
    assert!(flight(&sim, owner).is_none(), "caught");
}

#[test]
fn a_remote_droid_is_flown_through_and_struck_but_his_own_is_not() {
    for own in [false, true] {
        let (mut sim, owner, other) = long_throw();
        let builder = if own { owner } else { other };
        let team = sim.unit(builder).unwrap().team;
        sim.droids.push(crate::remote::Droid {
            id: 7,
            owner: builder,
            team,
            weapon_class: 1,
            pos: Vec3::new(0.0, 1.15, -10.0),
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
            blaster: crate::weapon::WeaponState::with_clips(&sim.weapon_classes[1], false, 0),
            charge: None,
            leaving: None,
            adrift: 0.0,
        });
        let health = sim.droids[0].health;
        sim.heightfield = None;
        let (far, _) = fly(&mut sim, owner);
        if own {
            assert!(far > 30.0, "{far}");
            assert_eq!(sim.droids[0].health, health);
        } else {
            assert!(far > 30.0, "{far}");
            assert!(sim.droids[0].health < health);
        }
    }
}

/// The killing blow keeps its contact and its sound at the fallen soldier.
#[test]
fn a_killing_throw_still_reports_its_contact_and_sound() {
    let (mut sim, owner, victim) = long_throw();
    stand_on_the_line(&mut sim, victim, 10.0);
    sim.unit_mut(victim).unwrap().health.cur = 10.0;
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    let mut sounds = 0;
    for _ in 0..400 {
        sim.events.clear();
        sim.tick_saber_throws();
        sounds += sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::SaberThrowHit { .. }))
            .count();
        if flight(&sim, owner).is_none() {
            break;
        }
    }
    assert!(!sim.units[victim.index as usize].alive);
    assert_eq!(sounds, 1);
}
