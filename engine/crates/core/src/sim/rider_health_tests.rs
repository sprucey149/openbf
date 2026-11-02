//! A vehicle's health and its riders' are two pools: what reaches each.

use super::hero_bodies::{pose_fixture, seat_fixture};
use super::Sim;
use crate::collision::HullFilter;
use crate::projectile::{Bolt, Shooter};
use crate::sim::tests::flat_ground;
use crate::unit::{UnitHandle, UnitSpec};
use crate::vehicle::VehicleHandle;
use crate::weapon::{ExplosionClass, FatRayClass, OrdnanceClass, WeaponClass};
use glam::Vec3;

const AT: Vec3 = Vec3::new(6.0, 0.0, 0.0);

/// A shooter at the origin and a rider `AT` 6 m along +X in a seat.
fn scene(exposed: bool) -> (Sim, usize, UnitHandle, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(31);
    s.heightfield = Some(flat_ground());
    let class = s.add_weapon_class(WeaponClass::bf2_rifle());
    let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let rider = s.spawn(&UnitSpec::trooper(1, AT)).unwrap();
    let vehicle = seat_fixture(&mut s, rider, exposed);
    (s, class, shooter, rider, vehicle)
}

/// A bolt along +X through the rider's chest, flown until it lands.
fn shoot(s: &mut Sim, class: usize, owner: UnitHandle) {
    let bolt = Bolt::fire(
        Vec3::new(0.0, 1.2, 0.0),
        Vec3::X,
        100.0,
        0.25,
        Shooter {
            owner,
            team: 0,
            weapon_class: class,
            is_player: false,
        },
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        HullFilter::All,
    );
    s.bolts.push(bolt);
    for _ in 0..12 {
        s.fly_bolts(crate::TICK_DT);
    }
}

fn health(s: &Sim, u: UnitHandle) -> f32 {
    s.units[u.index as usize].health.cur
}

fn hull(s: &Sim, v: VehicleHandle) -> f32 {
    s.vehicle(v).unwrap().health.cur
}

fn blast() -> ExplosionClass {
    ExplosionClass {
        damage: 100.0,
        damage_inner: 20.0,
        damage_outer: 20.0,
        ..Default::default()
    }
}

#[test]
fn a_bolt_hurts_an_exposed_rider_only_and_passes_an_enclosed_one() {
    for exposed in [false, true] {
        let (mut s, class, shooter, rider, vehicle) = scene(exposed);
        let before = (health(&s, rider), hull(&s, vehicle));
        shoot(&mut s, class, shooter);
        assert_eq!(hull(&s, vehicle), before.1, "a rider's hit is no hull hit");
        if exposed {
            assert!(health(&s, rider) < before.0);
        } else {
            assert_eq!(health(&s, rider), before.0);
        }
    }
}

#[test]
fn a_shot_rider_dies_alone_and_the_seat_empties() {
    let (mut s, class, shooter, rider, vehicle) = scene(true);
    pose_fixture(&mut s);
    s.units[rider.index as usize].health.cur = 1.0;
    let before = hull(&s, vehicle);
    shoot(&mut s, class, shooter);
    let dead = &s.units[rider.index as usize];
    assert!(!dead.alive);
    assert!(dead.mount.is_none());
    assert!(dead.body, "an exposed rider leaves a corpse");
    assert_eq!(dead.soldier.pos, AT, "at his seat");
    let v = s.vehicle(vehicle).expect("the vehicle remains");
    assert!(v.alive);
    assert!(v.seats[0].occupant.is_none());
    assert_eq!(v.health.cur, before);
}

#[test]
fn a_rider_killed_inside_a_hull_leaves_no_corpse() {
    let (mut s, _, _, rider, vehicle) = scene(false);
    s.units[rider.index as usize].health.cur = 0.0;
    s.unit_killed(rider, None);
    assert!(!s.units[rider.index as usize].body);
    assert!(s.vehicle(vehicle).unwrap().seats[0].occupant.is_none());
}

/// A hull's end kills every rider, with or without a destroyer to credit: the
/// exposed ones leave a corpse at the seat (re-vehicle-death 8), the rest none.
#[test]
fn a_destroyed_vehicle_leaves_only_its_exposed_riders_corpses() {
    for exposed in [false, true] {
        for credited in [false, true] {
            let (mut s, _, shooter, rider, vehicle) = scene(exposed);
            pose_fixture(&mut s);
            let seat = Vec3::new(6.0, 4.0, 2.0);
            s.vehicle_mut(vehicle).unwrap().pos = seat;
            s.destroy_vehicle(vehicle, credited.then_some(shooter));
            let dead = &s.units[rider.index as usize];
            assert!(!dead.alive && dead.mount.is_none());
            assert_eq!(dead.body, exposed, "credited={credited}");
            assert_eq!(dead.drop_due, exposed, "credited={credited}");
            if exposed {
                assert_eq!(dead.soldier.pos, seat, "credited={credited}");
            }
        }
    }
}

/// Hull damage stays on the hull; the damage he took directly stays on him;
/// the next rider, bot or human, finds the hull as the last one left it.
#[test]
fn each_pool_keeps_its_own_damage_through_exit_and_reentry() {
    let (mut s, class, shooter, rider, vehicle) = scene(true);
    let full = health(&s, rider);
    let desc = s.weapon_classes[class].ordnance.damage_desc(0.0, false);
    let dealt = s.damage_vehicle(vehicle, &desc, 1.0, Some((shooter, 0)));
    assert!(dealt > 0.0);
    let scarred = hull(&s, vehicle);
    assert_eq!(health(&s, rider), full, "hull damage never reaches him");
    shoot(&mut s, class, shooter);
    let hurt = health(&s, rider);
    assert!(hurt < full);
    s.exit_vehicle(rider);
    assert_eq!(health(&s, rider), hurt, "he exits as he was");
    assert_eq!(hull(&s, vehicle), scarred, "the hull stays as it was");
    let bot = s.spawn(&UnitSpec::trooper(1, AT)).unwrap();
    let human = s.spawn(&UnitSpec::player(1, AT)).unwrap();
    for next in [bot, human] {
        let was = health(&s, next);
        assert!(s.seat_unit(next, vehicle, 0));
        assert_eq!(hull(&s, vehicle), scarred);
        s.exit_vehicle(next);
        assert_eq!(health(&s, next), was);
        assert_eq!(hull(&s, vehicle), scarred);
    }
}

#[test]
fn a_blast_takes_the_hull_and_no_rider_of_either_seat() {
    for exposed in [false, true] {
        let (mut s, _, _, rider, vehicle) = scene(exposed);
        let before = (health(&s, rider), hull(&s, vehicle));
        s.explode(&blast(), AT, None, Some(0));
        assert_eq!(health(&s, rider), before.0);
        assert!(hull(&s, vehicle) < before.1);
    }
}

/// A building turret's gunner takes splash, with a body to take it on.
#[test]
fn a_turret_gunner_takes_a_blast_only_with_a_body() {
    for exposed in [false, true] {
        let (mut s, _, _, rider, vehicle) = scene(exposed);
        s.vehicle_mut(vehicle).unwrap().kind = crate::vehicle::VehicleKind::Turret;
        let before = health(&s, rider);
        s.explode(&blast(), AT, None, Some(0));
        assert_eq!(health(&s, rider) < before, exposed);
    }
}

#[test]
fn effects_caught_on_foot_wait_outside_a_hull() {
    for exposed in [false, true] {
        let (mut s, _, shooter, rider, _) = scene(exposed);
        let u = &mut s.units[rider.index as usize];
        u.health.cur = 100.0;
        u.buffs.poison = (5.0, 10.0);
        u.buffs.regeneration = (5.0, 1.0);
        s.tick_buffs(1.0);
        assert_eq!(
            health(&s, rider) != 100.0,
            exposed,
            "poison, exposed={exposed}"
        );
        let was = health(&s, rider);
        s.units[rider.index as usize].buffs = Default::default();
        s.units[rider.index as usize].burn = Some(crate::hero_weapon::Burn {
            left: 5.0,
            damage_per_second: 10.0,
            owner: shooter,
            team: 0,
            weapon_class: 0,
        });
        s.tick_burns(1.0);
        assert_eq!(health(&s, rider) < was, exposed, "burn, exposed={exposed}");
    }
}

#[test]
fn a_fat_ray_needs_a_rider_with_a_body() {
    for exposed in [false, true] {
        let (mut s, _, shooter, rider, _) = scene(exposed);
        let class = s.add_weapon_class(WeaponClass {
            ordnance: OrdnanceClass {
                velocity: 100.0,
                life_span: 0.25,
                max_damage: 100.0,
                fat_ray: Some(FatRayClass {
                    max_damage: 100.0,
                    min_damage: 10.0,
                    damage_deduction: 5.0,
                    radius: 1.0,
                }),
                ..OrdnanceClass::default()
            },
            ..WeaponClass::default()
        });
        let before = health(&s, rider);
        shoot(&mut s, class, shooter);
        assert_eq!(health(&s, rider) < before, exposed);
    }
}

#[test]
fn each_pool_moves_the_hash() {
    let (mut s, _, _, rider, vehicle) = scene(true);
    let base = s.hash();
    s.units[rider.index as usize].health.cur -= 1.0;
    let rider_hurt = s.hash();
    assert_ne!(rider_hurt, base);
    s.vehicle_mut(vehicle).unwrap().health.cur -= 1.0;
    assert_ne!(s.hash(), rider_hurt);
}
