//! Native angular slab and no late acquisition controls.
use super::*;

fn choke() -> (Sim, UnitHandle) {
    let (mut sim, owner) = scene(emitter());
    let power = sim.weapon_classes[0].force.as_mut().unwrap();
    power.kind = ForceKind::Choke {
        angle: 10.0 * crate::weapon::DEG,
        damage_per_second: 150.0,
        height: 4.0,
        falloff: 1.0,
        drain_rate: 15.0,
    };
    power.cone_cos = crate::fmath::sin_cos(10.0 * crate::weapon::DEG).1;
    power.max_targets = 1;
    (sim, owner)
}

#[test]
fn choke_damages_an_exposed_rider_without_forcing_the_seated_actor_into_choked() {
    for exposed in [false, true] {
        let (mut sim, owner) = choke();
        let target = victim(&mut sim, 4.0, 0.0);
        let vehicle = crate::sim::hero_bodies::seat_fixture(&mut sim, target, exposed);
        crate::sim::hero_bodies::pose_fixture(&mut sim);
        sim.unit_mut(target).unwrap().soldier.pos = Vec3::splat(100.0);
        let before = sim.unit(target).unwrap().health.cur;
        sim.force_input(owner.index as usize, None, true, false);
        let target = sim.unit(target).unwrap();
        assert_eq!(target.health.cur, before - if exposed { 5.0 } else { 0.0 });
        assert!(target.choked.is_none());
        assert_eq!(target.mount.unwrap().vehicle, vehicle);
    }
}

#[test]
fn choke_catching_a_force_thrown_victim_leaves_the_throw_state_behind() {
    let (mut sim, owner) = choke();
    let target = victim(&mut sim, 4.0, 0.0);
    let soldier = &mut sim.unit_mut(target).unwrap().soldier;
    soldier.force_knock(Vec3::Y, 30.0, 0.0);
    soldier.time_in_stance = 1.0;
    assert_eq!(soldier.stance, crate::soldier::Stance::Thrown);
    sim.force_input(owner.index as usize, None, true, false);
    let soldier = &sim.unit(target).unwrap().soldier;
    assert_eq!(soldier.stance, crate::soldier::Stance::Stand);
    assert_eq!(soldier.time_in_stance, 0.0);
}

#[test]
fn held_choke_does_not_acquire_a_late_target_and_release_allows_a_new_cast() {
    let (mut sim, owner) = choke();
    sim.force_input(owner.index as usize, None, true, false);
    let u = sim.unit(owner).unwrap();
    assert!(u.weapon.force_channel.as_ref().unwrap().acquired);
    assert_eq!(u.weapon.state, crate::weapon::FireState::Idle);
    let target = victim(&mut sim, 4.0, 0.0);
    let before = sim.unit(target).unwrap().health.cur;
    for _ in 0..30 {
        sim.force_input(owner.index as usize, None, true, false);
    }
    assert_eq!(sim.unit(target).unwrap().health.cur, before);
    sim.force_input(owner.index as usize, None, false, false);
    sim.force_input(owner.index as usize, None, true, false);
    assert_eq!(sim.unit(target).unwrap().health.cur, before - 5.0);
    assert_eq!(sim.unit(owner).unwrap().soldier.energy, 80.0);
}

#[test]
fn ray_area_accepts_an_off_axis_body_and_rejects_outside_angle_height_and_cover() {
    use crate::collision::{Body, Shape};
    use glam::Mat4;
    for (x, y, wall, accepted) in [
        (2.0, 0.0, false, true),
        (4.0, 0.0, false, false),
        (0.0, 5.0, false, false),
        (0.0, 0.0, true, false),
    ] {
        let (mut sim, owner) = choke();
        let target = victim(&mut sim, 8.0, x);
        sim.unit_mut(target).unwrap().soldier.pos.y += y;
        if wall {
            sim.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 2.0, -4.0)),
                Shape::Box {
                    half: Vec3::new(4.0, 4.0, 0.2),
                },
            ));
        }
        let before = sim.unit(target).unwrap().health.cur;
        sim.force_input(owner.index as usize, None, true, false);
        assert_eq!(
            sim.unit(target).unwrap().health.cur,
            before - if accepted { 5.0 } else { 0.0 },
            "x={x} y={y} wall={wall}"
        );
        assert_eq!(
            sim.unit(owner)
                .unwrap()
                .weapon
                .force_channel
                .as_ref()
                .unwrap()
                .target,
            accepted.then_some(target)
        );
    }
}

#[test]
fn a_lost_hold_target_cannot_rejoin_the_same_cast() {
    let (mut sim, owner) = choke();
    let target = victim(&mut sim, 4.0, 0.0);
    sim.force_input(owner.index as usize, None, true, false);
    let near = sim.unit(target).unwrap().soldier.pos;
    sim.unit_mut(target).unwrap().soldier.pos.z = 30.0;
    for _ in 0..12 {
        sim.force_input(owner.index as usize, None, true, false);
    }
    assert!(sim
        .unit(owner)
        .unwrap()
        .weapon
        .force_channel
        .as_ref()
        .unwrap()
        .target
        .is_none());
    let before = sim.unit(target).unwrap().health.cur;
    sim.unit_mut(target).unwrap().soldier.pos = near;
    for _ in 0..30 {
        sim.force_input(owner.index as usize, None, true, false);
    }
    assert_eq!(sim.unit(target).unwrap().health.cur, before);
}

#[test]
fn initial_ray_gather_excludes_a_body_behind_the_caster() {
    let (mut sim, owner) = choke();
    let target = victim(&mut sim, -8.0, 0.0);
    let before = sim.unit(target).unwrap().health.cur;
    sim.force_input(owner.index as usize, None, true, false);
    assert_eq!(sim.unit(target).unwrap().health.cur, before);
    assert!(sim
        .unit(owner)
        .unwrap()
        .weapon
        .force_channel
        .as_ref()
        .unwrap()
        .target
        .is_none());
}

#[test]
fn choke_can_affect_a_friend_without_damage_and_does_not_force_an_air_status() {
    for airborne in [false, true] {
        let (mut sim, owner) = choke();
        sim.friendly_fire_scale = 0.0;
        sim.weapon_classes[0].force.as_mut().unwrap().affect_friends = true;
        let target = victim(&mut sim, 4.0, 0.0);
        let u = sim.unit_mut(target).unwrap();
        u.team = 0;
        if airborne {
            u.soldier.stance = crate::soldier::Stance::Jump;
        }
        let health = u.health.cur;
        sim.force_input(owner.index as usize, None, true, false);
        let u = sim.unit(target).unwrap();
        assert_eq!(u.health.cur, health);
        assert_eq!(u.choked.is_some(), !airborne);
        assert_eq!(
            sim.unit(owner)
                .unwrap()
                .weapon
                .force_channel
                .as_ref()
                .unwrap()
                .target,
            Some(target)
        );
    }
}

#[test]
fn choke_hurts_a_droideka_without_a_soldiers_hold() {
    let (mut sim, owner) = choke();
    let y = sim.units[0].muzzle().y - 1.0;
    let deka = sim
        .spawn(&crate::unit::UnitSpec {
            droideka: Some(std::sync::Arc::new(Default::default())),
            ..crate::unit::UnitSpec::trooper(1, Vec3::new(0.0, y, -4.0))
        })
        .unwrap();
    let before = sim.unit(deka).unwrap().health.cur;
    sim.force_input(owner.index as usize, None, true, false);
    let u = sim.unit(deka).unwrap();
    assert!(u.health.cur < before);
    assert!(u.choked.is_none());
}
