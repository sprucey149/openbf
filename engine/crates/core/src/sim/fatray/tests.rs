use super::*;
use crate::collision::HullFilter;
use crate::projectile::{Bolt, Hit, Shooter};
use crate::sim::tests::{flat_ground, hits};
use crate::unit::UnitSpec;
use crate::weapon::{FatRayClass, OrdnanceClass, WeaponClass};
use glam::Vec3;

fn sonic() -> FatRayClass {
    FatRayClass {
        max_damage: 100.0,
        min_damage: 10.0,
        damage_deduction: 5.0,
        radius: 1.0,
    }
}

fn scene() -> (Sim, usize, UnitHandle) {
    let mut s = Sim::empty(5);
    s.heightfield = Some(flat_ground());
    let class = s.add_weapon_class(WeaponClass {
        ordnance: OrdnanceClass {
            velocity: 100.0,
            life_span: 0.25,
            max_damage: 100.0,
            fat_ray: Some(sonic()),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    });
    let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    (s, class, shooter)
}

fn fire(s: &mut Sim, class: usize, owner: UnitHandle, lift: f32) {
    let b = Bolt::fire(
        Vec3::new(0.0, 1.4 + lift, 0.0),
        Vec3::X,
        100.0,
        0.25,
        Shooter {
            owner,
            team: 0,
            weapon_class: class,
            is_player: true,
        },
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        HullFilter::Static,
    );
    s.bolts.push(b);
}

fn run(s: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        s.fly_bolts(crate::TICK_DT);
    }
}

#[test]
fn a_fat_ray_strikes_every_soldier_on_its_line_and_flies_on() {
    let (mut s, class, owner) = scene();
    let a = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(1.0, 0.0, 0.0)))
        .unwrap();
    let b = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(9.0, 0.0, 0.0)))
        .unwrap();
    fire(&mut s, class, owner, 0.0);
    run(&mut s, 4);
    assert_eq!(hits(&s), vec![(a.index, 100.0), (b.index, 95.0)]);
    assert_eq!(s.bolts[0].hit, Hit::None);
}

#[test]
fn the_falloff_runs_with_the_distance_from_the_line() {
    let (mut s, class, owner) = scene();
    let a = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, 0.0)))
        .unwrap();
    s.spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 1.5)))
        .unwrap();
    fire(&mut s, class, owner, 0.5);
    run(&mut s, 4);
    let got = hits(&s);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, a.index);
    assert!((got[0].1 - 50.0).abs() < 1e-3, "{got:?}");
    assert!((s.bolts[0].fat.unwrap() - 97.5).abs() < 1e-3);
}

#[test]
fn the_damage_never_falls_under_its_floor_and_the_shooter_is_passed() {
    let (mut s, class, owner) = scene();
    s.weapon_classes[class].ordnance.fat_ray = Some(FatRayClass {
        damage_deduction: 200.0,
        ..sonic()
    });
    let far = (0..3)
        .map(|k| {
            s.spawn(&UnitSpec::trooper(
                1,
                Vec3::new(2.0 + 3.0 * k as f32, 0.0, 0.0),
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    fire(&mut s, class, owner, 0.0);
    run(&mut s, 8);
    let got = hits(&s);
    assert_eq!(
        got,
        vec![
            (far[0].index, 100.0),
            (far[1].index, 10.0),
            (far[2].index, 10.0)
        ]
    );
    assert_eq!(
        s.unit(owner).unwrap().health.cur,
        s.unit(owner).unwrap().health.max
    );
}

#[test]
fn a_fat_ray_charges_the_raised_shield_and_continues_past_it() {
    let (mut s, class, owner) = scene();
    let target = s
        .spawn(&UnitSpec {
            droideka: Some(std::sync::Arc::new(Default::default())),
            ..UnitSpec::trooper(1, Vec3::new(1.0, 0.0, 0.0))
        })
        .unwrap();
    s.unit_mut(target)
        .unwrap()
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .on = true;
    let before = s.unit(target).unwrap().health.cur;
    let shield = s
        .unit(target)
        .unwrap()
        .droideka
        .as_ref()
        .unwrap()
        .shield
        .points;
    let beyond = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(9.0, 0.0, 0.0)))
        .unwrap();
    fire(&mut s, class, owner, 0.0);
    run(&mut s, 4);
    assert_eq!(s.unit(target).unwrap().health.cur, before);
    assert!(
        s.unit(target)
            .unwrap()
            .droideka
            .as_ref()
            .unwrap()
            .shield
            .points
            < shield
    );
    assert!(s.unit(beyond).unwrap().health.cur < 300.0);
}
