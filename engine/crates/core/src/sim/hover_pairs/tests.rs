use super::*;
use crate::collision::{trimesh, Shape};
use crate::sim::Sim;
use crate::vehicle::{SeatKind, VehicleSpec};
use crate::weapon::HealthType;
use glam::Quat;

fn spec(class: u32, pos: Vec3) -> VehicleSpec {
    VehicleSpec {
        kind: VehicleKind::Hover,
        class,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
}

fn pair() -> (Sim, VehicleHandle, VehicleHandle, HoverParams) {
    let mut s = Sim::empty(7);
    let parts: Arc<[HullPart]> = vec![HullPart::new(
        Mat4::IDENTITY,
        Shape::Sphere { radius: 1.0 },
        Layers::VEHICLE,
    )]
    .into();
    s.hull_parts = vec![Some(parts.clone()), Some(parts)];
    let a = s.spawn_vehicle(&spec(0, Vec3::Y * 1.5));
    let b = s.spawn_vehicle(&spec(1, Vec3::ZERO));
    (s, a, b, crate::vehicle::hover::tests::skiff())
}

#[test]
fn hover_pairs_separate_the_full_depth_without_compressing_springs() {
    let (mut s, a, _, p) = pair();
    let mut impacts = Vec::new();
    contacts(
        &mut s.vehicles,
        a.index as usize,
        &p,
        &s.hover_params,
        &s.hull_parts,
        &s.collision,
        &mut impacts,
    );
    assert_eq!(s.vehicle(a).unwrap().pos, Vec3::Y * 2.0);
    assert_eq!(s.vehicle(a).unwrap().hover.compress, [0.0; 6]);
    assert_eq!(impacts.len(), 1);
    // A stationary overlap is still separated, with no collision damage.
    assert_eq!(impacts[0].normal_speed, 0.0);
}

#[test]
fn hover_pairs_use_relative_velocity_and_speed_weighted_default_masses() {
    let (_, _, _, p) = pair();
    let mut v = spec(0, Vec3::ZERO).build(1);
    v.vel = Vec3::new(-10.0, 3.0, 0.0);
    let hit = respond(&mut v, &p, Vec3::X * 2.0, 1.0, 0.25, Vec3::X);
    assert_eq!(hit.normal_speed, -12.0);
    assert!((v.vel.x - 0.125).abs() < 1e-6);
    assert!((v.vel.y - 0.46875).abs() < 1e-6);
    assert_eq!(v.pos, Vec3::X * 0.25);
    v.vel = Vec3::X * 2.0;
    respond(&mut v, &p, Vec3::X * 2.0, 1.0, 0.25, Vec3::X);
    assert_eq!(v.vel, Vec3::X * 2.0, "co-moving hulls must not brake");
}

#[test]
fn hover_pairs_respect_both_subsets_skip_self_and_removed_hulls() {
    let (mut s, a, b, p) = pair();
    let mut impacts = Vec::new();
    for own in [false, true] {
        let class = if own { 0 } else { 1 };
        s.hull_parts[class] = Some(
            vec![HullPart::new(
                Mat4::IDENTITY,
                Shape::Sphere { radius: 10.0 },
                Layers::SOLDIER,
            )]
            .into(),
        );
        s.sync_hulls();
        contacts(
            &mut s.vehicles,
            a.index as usize,
            &p,
            &s.hover_params,
            &s.hull_parts,
            &s.collision,
            &mut impacts,
        );
        assert!(impacts.is_empty());
        assert_eq!(s.vehicle(a).unwrap().pos, Vec3::Y * 1.5);
    }
    s.remove_vehicle(b);
    contacts(
        &mut s.vehicles,
        a.index as usize,
        &p,
        &s.hover_params,
        &s.hull_parts,
        &s.collision,
        &mut impacts,
    );
    assert!(impacts.is_empty());
}

#[test]
fn hover_pairs_contact_rotated_boxes_cylinders_and_meshes() {
    let mesh = trimesh(
        &[[-5.0, 0.0, -5.0], [0.0, 0.0, 5.0], [5.0, 0.0, -5.0]],
        &[0, 1, 2],
    )
    .unwrap();
    for shape in [
        Shape::Box {
            half: Vec3::new(3.0, 1.0, 3.0),
        },
        Shape::Cylinder {
            radius: 3.0,
            half_height: 1.0,
        },
        Shape::Mesh(mesh),
    ] {
        let (mut s, a, b, p) = pair();
        s.hull_parts[1] = Some(vec![HullPart::new(Mat4::IDENTITY, shape, Layers::VEHICLE)].into());
        s.vehicle_mut(b).unwrap().rot = Quat::from_rotation_y(0.7);
        s.vehicle_mut(a).unwrap().pos = Vec3::Y * 0.5;
        s.sync_hulls();
        let mut impacts = Vec::new();
        contacts(
            &mut s.vehicles,
            a.index as usize,
            &p,
            &s.hover_params,
            &s.hull_parts,
            &s.collision,
            &mut impacts,
        );
        assert!(!impacts.is_empty());
        assert!(
            s.vehicle(a).unwrap().pos.y >= 0.999,
            "must stand above the surface"
        );
    }
}

#[test]
fn hover_pairs_run_in_the_sim_tick_for_player_ai_and_empty_seats() {
    use crate::unit::UnitSpec;
    for pilot in [None, Some(false), Some(true)] {
        let (mut s, a, b, mut p) = pair();
        p.gravity_scale = 0.0;
        p.random_spring = false;
        p.level_spring = 0.0;
        s.hover_params = vec![Some(p.clone()), None];
        if let Some(ai) = pilot {
            let pilot = s
                .spawn(&if ai {
                    UnitSpec::trooper(0, Vec3::ZERO)
                } else {
                    UnitSpec::player(0, Vec3::ZERO)
                })
                .unwrap();
            assert!(s.seat_unit(pilot, a, 0));
        }
        s.vehicle_mut(a).unwrap().vel = Vec3::NEG_Y;
        for _ in 0..60 {
            s.tick_hovers(crate::TICK_DT);
            let v = s.vehicle(a).unwrap();
            assert!(
                v.pos.y >= 1.999,
                "{pilot:?}: sank through the other hull: {:?}",
                v.pos
            );
            assert!(v.pos.is_finite() && v.vel.is_finite());
            assert_eq!(v.hover.compress, [0.0; 6]);
        }
        assert_eq!(s.vehicle(b).unwrap().pos, Vec3::ZERO);
    }
}

#[test]
fn hover_pair_impact_reaches_the_existing_vehicle_damage_path() {
    let (mut s, a, _, mut p) = pair();
    p.gravity_scale = 0.0;
    p.level_spring = 0.0;
    p.collision_threshold = [1.0, 0.0, 0.0];
    p.collision_scale = [1.0, 0.0, 0.0];
    s.hover_params = vec![Some(p), None];
    s.vehicle_mut(a).unwrap().vel = Vec3::NEG_Y * 10.0;
    s.tick_hovers(crate::TICK_DT);
    assert_eq!(s.vehicle(a).unwrap().health.cur, 919.0, "(10 - 1)^2 damage");
}

#[test]
fn hover_pairs_account_for_the_other_hulls_model_mass() {
    let (_, _, _, mut p) = pair();
    p.mass = 11.062_572;
    let mut v = spec(0, Vec3::ZERO).build(1);
    v.vel = Vec3::NEG_Y * 2.0;
    respond(&mut v, &p, Vec3::ZERO, 302.695_4, 0.1, Vec3::Y);
    assert!(
        v.vel.y > 0.2 && v.vel.y < 0.25,
        "light STAP bounces off the heavy tank"
    );
    let mut tank = spec(0, Vec3::ZERO).build(1);
    p.mass = 302.695_4;
    tank.vel = Vec3::NEG_Y * 2.0;
    respond(&mut tank, &p, Vec3::ZERO, 11.062_572, 0.1, Vec3::Y);
    assert!(
        tank.vel.y < -1.8,
        "the reciprocal contact must retain the mass difference"
    );
}
