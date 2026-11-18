use super::super::*;
use crate::collision::{Body, HullFilter, HullPart, Layers, Shape};
use crate::projectile::{Bolt, Hit, Shooter, Target};
use crate::sim::PadState;
use crate::unit::UnitSpec;
use crate::weapon::{DamageDesc, ExplosionClass};
use glam::Mat4;

fn fixture() -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(5);
    let owner = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 5.0)))
        .unwrap();
    sim.weapon_classes[0].ordnance.max_damage = 40.0;
    sim.weapon_classes[0].ordnance.scales[HealthType::Building as usize] = 0.5;
    sim.add_station_class(StationClass::soldier("medical", 0.1, 15.0, 0));
    let mut s = Station::new(0, Vec3::ZERO, 5.0);
    s.instance = Some(7);
    s.sphere_centre = Vec3::Y;
    s.sphere_radius = 1.2;
    sim.stations.push(s);
    let mut body = Body::new(
        Mat4::from_translation(Vec3::Y),
        Shape::Cylinder {
            radius: 0.4,
            half_height: 1.0,
        },
    );
    body.instance = Some(7);
    sim.collision.push_body(body);
    sim.collision.bind_station(7, 0);
    sim.collision.rebuild_broadphase();
    sim.station_wreck_collision.push(vec![HullPart::new(
        Mat4::from_translation(Vec3::Y * 0.2),
        Shape::Box {
            half: Vec3::new(0.4, 0.2, 0.4),
        },
        Layers::ALL,
    )]);
    (sim, owner)
}

fn shot(sim: &Sim, owner: UnitHandle) -> Bolt {
    Bolt::spawn(
        Vec3::new(0.0, 1.0, 4.0),
        -Vec3::Z,
        150.0,
        2.0,
        Shooter {
            owner,
            team: 0,
            weapon_class: 0,
            is_player: true,
        },
        None,
        None,
        &sim.collision,
        HullFilter::All,
        std::iter::empty(),
    )
}

fn lethal() -> DamageDesc {
    crate::weapon::OrdnanceClass {
        max_damage: 500.0,
        ..Default::default()
    }
    .damage_desc(0.0, false)
}

#[test]
fn the_first_wreck_already_blocks_a_chained_stations_blast() {
    let (mut sim, owner) = fixture();
    // The intact body is ABOVE the second blast's ray. Only the low wreck
    // blocks it; otherwise moving the wreck swap after detonation passes.
    sim.collision
        .move_body(0, Mat4::from_translation(Vec3::Y * 2.0));
    sim.collision.rebuild_broadphase();
    sim.station_classes[0].explosion = Some(ExplosionClass {
        damage: 200.0,
        damage_outer: 1.3,
        ..Default::default()
    });
    let mut second = StationClass::soldier("second", 1.0, 0.0, 0);
    second.explosion = Some(ExplosionClass {
        damage: 30.0,
        damage_outer: 5.0,
        ..Default::default()
    });
    sim.add_station_class(second);
    let mut b = Station::new(1, Vec3::new(0.0, 0.2, 1.0), 0.0);
    b.health.cur = 1.0;
    sim.stations.push(b);
    sim.stations
        .push(Station::new(1, Vec3::new(0.0, 0.2, -1.5), 0.0));
    assert!(
        crate::perception::los_seat(
            &sim,
            sim.stations[1].pos,
            sim.stations[2].pos,
            HullFilter::Static,
            [None, None],
        ),
        "intact body must leave the chained-blast ray clear"
    );
    sim.damage_station(0, &lethal(), 1.0, Some(owner), Some(0));
    assert!(sim.stations[1].destroyed);
    assert_eq!(sim.stations[2].health.cur, 80.0);
}

#[test]
fn bullets_damage_the_station_pool_with_building_scale_and_leave_a_solid_wreck() {
    let (mut sim, owner) = fixture();
    sim.friendly_fire_scale = 0.0;
    let base = sim.hash();
    for hp in [60.0, 40.0, 20.0, 0.0] {
        let b = shot(&sim, owner);
        assert_eq!(b.hit, Hit::Target(Target::Station(0)));
        sim.resolve_bolt(&b);
        assert_eq!(sim.stations[0].health.cur, hp);
        assert_ne!(base, sim.hash());
    }
    assert!(sim.stations[0].destroyed);
    assert_eq!(
        sim.events
            .iter()
            .filter(|e| matches!(e, Event::StationDestroyed { .. }))
            .count(),
        1
    );
    assert_eq!(shot(&sim, owner).hit, Hit::None);
    let h = sim
        .collision
        .ray(Vec3::new(0.0, 0.2, 4.0), -Vec3::Z, 8.0, Layers::ORDNANCE);
    assert!(h.is_some());
    assert_eq!(sim.collision.bodies()[0].instance, Some(7));
    let stale = Bolt {
        hit: Hit::Target(Target::Station(0)),
        ..shot(&sim, owner)
    };
    let events = sim.events.len();
    sim.resolve_bolt(&stale);
    assert_eq!(sim.events.len(), events);
}

#[test]
fn a_wall_intercepts_bullets_and_blocks_splash_but_the_station_does_not_block_its_own_splash() {
    let (mut sim, owner) = fixture();
    let e = ExplosionClass {
        damage: 200.0,
        damage_outer: 10.0,
        ..Default::default()
    };
    sim.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 1.0, 2.0)),
        Shape::Box {
            half: Vec3::new(2.0, 2.0, 0.1),
        },
    ));
    assert_eq!(shot(&sim, owner).hit, Hit::World);
    sim.explode(&e, Vec3::new(0.0, 1.0, 4.0), Some(owner), Some(0));
    assert_eq!(sim.stations[0].health.cur, 80.0);
    let (mut sim, owner) = fixture();
    sim.explode(&e, Vec3::new(0.0, 1.0, 4.0), Some(owner), Some(0));
    assert!(sim.stations[0].destroyed);
}

#[test]
fn destroyed_stations_stop_service_and_the_death_explosion_hurts_nearby_units_once() {
    let (mut sim, owner) = fixture();
    sim.units[owner.index as usize].soldier.pos = Vec3::new(0.0, 0.0, 1.5);
    sim.units[owner.index as usize].health.cur = 100.0;
    sim.tick_stations(0.1);
    assert!(sim.stations[0].serving);
    sim.station_classes[0].explosion = Some(ExplosionClass {
        damage: 20.0,
        damage_outer: 5.0,
        ..Default::default()
    });
    let before = sim.units[owner.index as usize].health.cur;
    sim.damage_station(0, &lethal(), 1.0, Some(owner), Some(0));
    let after = sim.units[owner.index as usize].health.cur;
    assert!(after < before);
    assert!(!sim.stations[0].serving);
    sim.damage_station(0, &lethal(), 1.0, Some(owner), Some(0));
    assert_eq!(sim.units[owner.index as usize].health.cur, after);
    sim.events.clear();
    for _ in 0..90 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.units[owner.index as usize].health.cur, after);
    assert!(!sim
        .events
        .iter()
        .any(|e| matches!(e, Event::StationServed { .. })));
}

#[test]
fn station_deaths_chain_without_detonating_the_same_station_twice() {
    let (mut sim, owner) = fixture();
    sim.station_classes[0].explosion = Some(ExplosionClass {
        damage: 500.0,
        damage_outer: 8.0,
        ..Default::default()
    });
    let mut other = Station::new(0, Vec3::new(2.0, 0.0, 0.0), 5.0);
    other.sphere_centre = Vec3::new(2.0, 1.0, 0.0);
    sim.stations.push(other);
    sim.damage_station(0, &lethal(), 1.0, Some(owner), Some(0));
    assert!(sim.stations.iter().all(|s| s.destroyed));
    assert_eq!(
        sim.events
            .iter()
            .filter(|e| matches!(e, Event::StationDestroyed { .. }))
            .count(),
        2
    );
}

#[test]
fn the_fixed_step_bolt_path_is_deterministic_with_destructible_stations() {
    let (mut a, owner) = fixture();
    let (mut b, _) = fixture();
    for _ in 0..4 {
        a.bolts.push(shot(&a, owner));
        b.bolts.push(shot(&b, owner));
        for _ in 0..3 {
            a.step(&[PadState::default()]);
            b.step(&[PadState::default()]);
            assert_eq!(a.hash(), b.hash());
        }
    }
    assert!(a.stations[0].destroyed);
}

#[test]
fn destroyed_ammo_droids_never_resupply_and_damage_respects_type_and_team_scales() {
    let (mut sim, owner) = fixture();
    sim.station_classes[0].health = [0.0; 5];
    sim.station_classes[0].ammo = [1, 0, 1, 0, 0];
    sim.units[owner.index as usize].soldier.pos = Vec3::X;
    sim.units[owner.index as usize].weapon.spare = 0;
    sim.tick_stations(0.1);
    assert!(sim.units[owner.index as usize].weapon.spare > 0);
    sim.stations[0].team = Some(0);
    sim.friendly_fire_scale = 0.0;
    sim.damage_station(0, &lethal(), 1.0, Some(owner), Some(0));
    assert_eq!(sim.stations[0].health.cur, 80.0);
    let mut immune = lethal();
    immune.scales[HealthType::Building as usize] = 0.0;
    sim.damage_station(0, &immune, 1.0, Some(owner), Some(1));
    assert_eq!(sim.stations[0].health.cur, 80.0);
    sim.damage_station(0, &lethal(), 1.0, Some(owner), Some(1));
    sim.units[owner.index as usize].weapon.spare = 0;
    sim.tick_stations(10.0);
    assert_eq!(sim.units[owner.index as usize].weapon.spare, 0);
    assert!(sim.stations[0].destroyed);
}

#[test]
fn ai_damage_threshold_clamps_ai_but_allows_player_to_destroy_station() {
    let (mut sim, _dummy) = fixture();
    let bot = sim
        .spawn(&UnitSpec {
            controller: crate::unit::Controller::Ai,
            ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 4.0))
        })
        .unwrap();
    let human = sim
        .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, -4.0)))
        .unwrap();
    sim.stations[0].team = Some(0);
    sim.stations[0].ai_damage_threshold = 0.5;
    // Bot hits can reduce HP from 80 down to the 40 HP floor, never below.
    let shot_desc = crate::weapon::OrdnanceClass {
        max_damage: 80.0,
        ..Default::default()
    }
    .damage_desc(0.0, false);
    sim.damage_station(0, &shot_desc, 1.0, Some(bot), Some(1));
    assert_eq!(sim.stations[0].health.cur, 40.0);
    assert!(!sim.stations[0].destroyed);
    sim.damage_station(0, &shot_desc, 1.0, Some(bot), Some(1));
    assert_eq!(sim.stations[0].health.cur, 40.0);
    // Human attacker bypasses the AI damage floor and finishes it.
    sim.damage_station(0, &shot_desc, 1.0, Some(human), Some(1));
    assert!(sim.stations[0].destroyed);
    assert_eq!(sim.stations[0].health.cur, 0.0);
}
