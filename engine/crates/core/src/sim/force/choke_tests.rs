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
fn choke_catching_a_force_thrown_victim_holds_him_where_he_flies() {
    let (mut sim, owner) = choke();
    let target = victim(&mut sim, 4.0, 0.0);
    let soldier = &mut sim.unit_mut(target).unwrap().soldier;
    soldier.force_knock(Vec3::Y, 30.0, 0.0);
    soldier.time_in_stance = 1.0;
    assert_eq!(soldier.stance, crate::soldier::Stance::Thrown);
    let at = soldier.pos;
    sim.force_input(owner.index as usize, None, true, false);
    let u = sim.unit(target).unwrap();
    assert_eq!(u.soldier.stance, crate::soldier::Stance::Fall);
    assert_eq!(u.soldier.time_in_stance, 0.0);
    assert_eq!(u.soldier.vel, Vec3::ZERO);
    assert_eq!(u.choked.unwrap().hold, Some(at));
}

/// Issue #173, the maintainer's rule: a soldier on the floor, standing,
/// crouching, prone or sprinting, is stood up into the hold on the update
/// the choke catches him, with no get-up and no lock while it holds.
#[test]
fn choke_stands_a_floor_victim_up_into_the_hold_at_once() {
    use crate::soldier::Stance;
    for stance in [Stance::Stand, Stance::Crouch, Stance::Prone, Stance::Sprint] {
        let (mut sim, owner) = choke();
        let target = victim(&mut sim, 4.0, 0.0);
        let soldier = &mut sim.unit_mut(target).unwrap().soldier;
        soldier.stance = stance;
        soldier.grounded = true;
        soldier.time_in_stance = 2.0;
        sim.force_input(owner.index as usize, None, true, false);
        let u = sim.unit(target).unwrap();
        let held = u.choked.expect("held");
        assert_eq!(held.hold, None, "{stance:?}");
        assert_eq!(u.soldier.stance, Stance::Stand, "{stance:?}");
        assert_eq!(u.soldier.lock_timer, 0.0, "{stance:?}: no get-up lock");
        assert!(!u.soldier.lock_hard, "{stance:?}");
    }
}

/// A flying jet trooper is held where the choke catches him, his jet cut off
/// and its tank emptied; when the hold ends he drops to the floor.
#[test]
fn choke_holds_a_flying_jet_trooper_in_the_air_then_he_falls_and_lands() {
    use crate::soldier::Stance;
    let (mut sim, owner) = choke();
    // 1.2 m up at 8 m: inside the choke's 10 degree cone.
    let target = victim(&mut sim, 8.0, 0.0);
    let at = {
        let soldier = &mut sim.unit_mut(target).unwrap().soldier;
        soldier.stance = Stance::Hover;
        soldier.grounded = false;
        soldier.pos.y += 1.2;
        soldier.vel = Vec3::new(3.0, 1.0, 0.0);
        soldier.fuel = 1.0;
        soldier.pos
    };
    sim.force_input(owner.index as usize, None, true, false);
    let u = sim.unit(target).unwrap();
    assert_eq!(u.choked.unwrap().hold, Some(at));
    assert_eq!(u.soldier.stance, Stance::Fall);
    assert_eq!(u.soldier.fuel, 0.0);
    // The hold lasts 0.5 s after its last application; the sim keeps him
    // still in the air until then (no gravity, no drift).
    let pads = vec![crate::sim::PadState::default(); sim.units.len()];
    let mut released_at = None;
    for t in 0..120 {
        sim.step(&pads);
        let u = sim.unit(target).unwrap();
        match u.choked {
            Some(_) => assert_eq!(u.soldier.pos, at, "tick {t}"),
            None => {
                released_at.get_or_insert(t);
            }
        }
        if u.soldier.grounded {
            break;
        }
    }
    let released_at = released_at.expect("released");
    assert!(released_at <= 16, "released at tick {released_at}");
    let u = sim.unit(target).unwrap();
    assert!(u.alive);
    assert!(u.soldier.grounded, "landed");
    assert!(u.soldier.pos.y < at.y - 1.0, "dropped to the floor");
    assert!(!u.soldier.stance.jetting());
}

/// The lock after a hold (B2 `FUN_00246850`, the stand-up out of the choke
/// state: 0.5 s, flags 0x1c00): the sim sets it on the update the hold ends.
#[test]
fn choke_release_sets_the_stand_ups_lock() {
    let (mut sim, owner) = choke();
    let target = victim(&mut sim, 4.0, 0.0);
    sim.force_input(owner.index as usize, None, true, false);
    assert!(sim.unit(target).unwrap().choked.is_some());
    let pads = [crate::sim::PadState::default()];
    let mut ticks = 0;
    while sim.unit(target).unwrap().choked.is_some() {
        sim.step(&pads);
        ticks += 1;
        assert!(ticks < 40, "the hold ends");
    }
    let u = sim.unit(target).unwrap();
    assert!(u.soldier.lock_timer > crate::soldier::CHOKE_RELEASE_LOCK - 0.05);
    assert!(u.soldier.fire_lock && !u.soldier.lock_hard);
    assert!(u.soldier.no_fire());
}

/// While the release lock runs, a jump or a stance press does nothing and the
/// trigger is held off; walking and turning are free. After 0.5 s all of it
/// works again.
#[test]
fn choke_release_lock_holds_jump_stance_and_trigger_but_not_walking() {
    use crate::sim::{buttons, PadState};
    use crate::soldier::{GroundRules, Soldier, SoldierConstants, Stance};
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    s.choke_release_lock();
    assert!(s.no_fire() && !s.look_frozen());
    let mut pad = PadState {
        ly: 32767,
        rx: 16000,
        ..Default::default()
    };
    let (yaw0, z0) = (s.yaw, s.pos.z);
    for t in 0..14 {
        pad.buttons = if t % 2 == 0 {
            buttons::JUMP | buttons::ROLL_STANCE
        } else {
            0
        };
        s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        s.resolve_ground(crate::TICK_DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
        assert_eq!(
            s.stance,
            Stance::Stand,
            "tick {t}: no jump or stance change"
        );
        assert!(s.no_fire(), "tick {t}: trigger held off");
    }
    assert!((s.pos.z - z0).abs() > 0.5, "walked under the lock");
    assert!(s.yaw != yaw0, "turned under the lock");
    for _ in 0..2 {
        pad.buttons = 0;
        s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        s.resolve_ground(crate::TICK_DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
    }
    assert!(!s.no_fire(), "lock over after 0.5 s");
    pad.buttons = buttons::JUMP;
    s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
    assert_eq!(s.stance, Stance::Jump, "jumps once the lock is over");
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
fn choke_sight_runs_from_the_fire_point_so_a_curb_does_not_hide_a_standing_victim() {
    // The original tests the Ray hold's sight from the weapon fire point to the
    // target's collision-sphere centre (B2 0x32c408), not feet to feet, so a
    // low step between two soldiers on the same floor does not block it.
    use crate::collision::{Body, Shape};
    use glam::Mat4;
    for curb in [false, true] {
        let (mut sim, owner) = choke();
        let target = victim(&mut sim, 8.0, 0.0);
        sim.unit_mut(target).unwrap().soldier.pos.y = 0.0;
        if curb {
            // Sunk 5 cm so the old feet-to-feet ray at y = 0 runs through the
            // curb, not along its bottom face.
            sim.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 0.10, -4.0)),
                Shape::Box {
                    half: Vec3::new(4.0, 0.15, 0.2),
                },
            ));
        }
        let before = sim.unit(target).unwrap().health.cur;
        sim.force_input(owner.index as usize, None, true, false);
        assert_eq!(
            sim.unit(target).unwrap().health.cur,
            before - 5.0,
            "curb={curb}"
        );
        assert!(sim.unit(target).unwrap().choked.is_some(), "curb={curb}");
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
fn choke_can_affect_a_friend_without_damage_and_holds_him_in_the_air_too() {
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
        // Issue #173: a jumper is held too, where he is.
        assert!(u.choked.is_some());
        assert_eq!(u.choked.unwrap().hold.is_some(), airborne);
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

#[test]
fn choke_damage_uses_the_same_raised_shield_phase() {
    let (mut sim, owner) = choke();
    let y = sim.units[0].muzzle().y - 1.0;
    let target = sim
        .spawn(&crate::unit::UnitSpec {
            droideka: Some(std::sync::Arc::new(Default::default())),
            ..crate::unit::UnitSpec::trooper(1, Vec3::new(0.0, y, -4.0))
        })
        .unwrap();
    sim.unit_mut(target)
        .unwrap()
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .on = true;
    let before = sim.unit(target).unwrap().health.cur;
    sim.force_input(owner.index as usize, None, true, false);
    let u = sim.unit(target).unwrap();
    assert_eq!(u.health.cur, before);
    assert!(u.droideka.as_ref().unwrap().shield.points < 3000.0);
    assert!(u.choked.is_none());
}

/// PC BF2 `WeaponAreaEffect` damages through the hit entry, so a damaging
/// choke plays the hit pose and sets the death side.
#[test]
fn a_damaging_choke_is_a_direct_hit() {
    let (mut sim, owner) = choke();
    let target = victim(&mut sim, 4.0, 0.0);
    sim.force_input(owner.index as usize, None, true, false);
    assert!(sim.events.iter().any(|e| matches!(
        e,
        crate::sim::Event::Hit { victim, direct: true, damage, .. } if *victim == target && *damage > 0.0
    )));
}

/// Issue #173: the floor-or-air split goes by the stance. A floor stance a
/// few ticks off a kerb's edge (not yet grounded) is still stood up into the
/// hold, not held in the air.
#[test]
fn choke_stands_a_floor_stance_up_even_a_few_ticks_off_the_floor() {
    use crate::soldier::Stance;
    for stance in [Stance::Stand, Stance::Crouch, Stance::Prone, Stance::Sprint] {
        let (mut sim, owner) = choke();
        let target = victim(&mut sim, 4.0, 0.0);
        let soldier = &mut sim.unit_mut(target).unwrap().soldier;
        soldier.stance = stance;
        soldier.grounded = false;
        soldier.air_time = 0.1;
        soldier.time_in_stance = 2.0;
        sim.force_input(owner.index as usize, None, true, false);
        let u = sim.unit(target).unwrap();
        assert_eq!(u.choked.expect("held").hold, None, "{stance:?}");
        assert_eq!(u.soldier.stance, Stance::Stand, "{stance:?}");
    }
}

/// Issue #173: a soldier caught in the air drops when the hold lets go. The
/// hold empties his jet tank whatever air stance it caught him in, and the
/// release lock holds off the air's jump / jet tap (a Jedi's double jump,
/// a jump-jet or a hover) for its 0.5 s, even with fuel in the tank.
#[test]
fn choke_release_lock_holds_off_the_air_jump_and_jet_tap() {
    use crate::sim::{buttons, PadState};
    use crate::soldier::{GroundRules, Soldier, SoldierConstants, Stance};
    for (saber, jet_hover) in [(true, false), (false, true), (false, false)] {
        let c = SoldierConstants {
            saber,
            jet_hover,
            jet_jump: 12.0,
            ..Default::default()
        };
        let mut s = Soldier::new(Vec3::new(0.0, 20.0, 0.0));
        s.stance = Stance::Jump;
        s.grounded = false;
        s.fuel = 1.0;
        s.catch_in_air();
        assert_eq!(s.fuel, 0.0, "the hold cuts the jet off");
        s.fuel = 1.0;
        s.choke_release_lock();
        let mut pad = PadState::default();
        for t in 0..14 {
            pad.buttons = if t % 2 == 0 { buttons::JUMP } else { 0 };
            s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
            assert_eq!(
                s.stance,
                Stance::Fall,
                "saber {saber} hover {jet_hover} tick {t}"
            );
            assert!(s.vel.y < 0.0, "tick {t}: he falls");
        }
        for _ in 0..2 {
            pad.buttons = 0;
            s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        }
        assert!(!s.choke_locked());
        let vy = s.vel.y;
        pad.buttons = buttons::JUMP;
        s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        assert!(
            s.vel.y > vy + 1.0 || s.stance != Stance::Fall,
            "saber {saber} hover {jet_hover}: the tap works once the lock is over"
        );
    }
}

/// The release lock holds off the sprint too (the original's flag 0x800).
#[test]
fn choke_release_lock_holds_off_the_sprint() {
    use crate::sim::{buttons, PadState};
    use crate::soldier::{GroundRules, Soldier, SoldierConstants, Stance};
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    s.grounded = true;
    s.choke_release_lock();
    let pad = PadState {
        ly: 32767,
        buttons: buttons::SPRINT,
        ..Default::default()
    };
    for t in 0..14 {
        s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        s.resolve_ground(crate::TICK_DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
        assert_eq!(
            s.stance,
            Stance::Stand,
            "tick {t}: no sprint under the lock"
        );
    }
    for _ in 0..10 {
        s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        s.resolve_ground(crate::TICK_DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
    }
    assert_eq!(s.stance, Stance::Sprint, "sprints once the lock is over");
}

/// A landing inside the release lock keeps the lock's 0.5 s (and its trigger
/// hold) instead of the landing's shorter light lock.
#[test]
fn a_landing_inside_the_release_lock_keeps_its_time() {
    use crate::sim::PadState;
    use crate::soldier::{GroundRules, Soldier, SoldierConstants, Stance};
    let c = SoldierConstants::default();
    assert!(c.soft_lock < crate::soldier::CHOKE_RELEASE_LOCK);
    let mut s = Soldier::new(Vec3::new(0.0, 0.02, 0.0));
    s.stance = Stance::Fall;
    s.grounded = false;
    s.choke_release_lock();
    let pad = PadState::default();
    let mut ticks = 0;
    while s.lock_timer > 0.0 {
        s.step(crate::TICK_DT, &pad, &c, GroundRules::Bf2);
        s.resolve_ground(crate::TICK_DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
        ticks += 1;
        if s.lock_timer > 0.0 {
            assert!(s.no_fire(), "tick {ticks}: trigger still held off");
        }
        assert!(ticks < 40);
    }
    assert_eq!(s.stance, Stance::Stand, "landed");
    assert!(ticks >= 15, "the lock ran its 0.5 s ({ticks} ticks)");
}
