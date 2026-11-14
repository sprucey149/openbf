use super::*;
use crate::hero_weapon::{Emitter, MeleeState, SaberClass};
use crate::sim::{tests::flat_ground, PadState};
use crate::unit::UnitSpec;
use crate::weapon::{HealthType, OrdnanceClass, WeaponClass};

fn emitter() -> Emitter {
    Emitter {
        hit_all: true,
        first_radius: 0.0,
        jump_deduction: 0.0,
        damage_threshold: 0.0,
        max_jump_distance: 10.0,
        lightning: false,
        burn_seconds: 0.0,
        burn_damage_per_second: 0.0,
    }
}

fn scene(config: Emitter) -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(72);
    sim.heightfield = Some(flat_ground());
    let weapon = WeaponClass {
        force: Some(ForcePower {
            range: 20.0,
            cone_cos: 0.5,
            max_targets: 10,
            push: 0.0,
            affect_friends: false,
            affect_enemies: true,
            energy: 10.0,
            kind: ForceKind::Emitter(config),
            effect: None,
            animation: None,
        }),
        ordnance: OrdnanceClass {
            max_damage: 90.0,
            min_damage: Some(10.0),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    };
    sim.weapon_classes = vec![weapon].into();
    let player = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    sim.units[player.index as usize].soldier.energy = 100.0;
    (sim, player)
}

fn victim(sim: &mut Sim, z: f32, x: f32) -> UnitHandle {
    // Match the standing sphere centre to the caster's actual muzzle height.
    let y = sim.units[0].muzzle().y - 1.0;
    sim.spawn(&UnitSpec::trooper(1, Vec3::new(x, y, -z)))
        .unwrap()
}

#[test]
fn hit_all_divides_by_candidates_then_limits_hits_and_pays_once() {
    let (mut sim, player) = scene(emitter());
    let a = victim(&mut sim, 3.0, 0.0);
    let b = victim(&mut sim, 5.0, 0.0);
    let c = victim(&mut sim, 7.0, 0.0);
    sim.weapon_classes[0].force.as_mut().unwrap().max_targets = 2;
    sim.fire_force(player.index as usize, 0, 1.0);
    assert_eq!(sim.unit(a).unwrap().health.cur, 270.0);
    assert_eq!(sim.unit(b).unwrap().health.cur, 270.0);
    assert_eq!(sim.unit(c).unwrap().health.cur, 300.0);
    assert_eq!(sim.unit(player).unwrap().soldier.energy, 90.0);
    let contacts: Vec<_> = sim
        .events
        .iter()
        .filter_map(|e| match e {
            Event::ForceHit {
                target, blocked, ..
            } => Some((*target, *blocked)),
            _ => None,
        })
        .collect();
    assert_eq!(
        contacts,
        [
            (crate::projectile::Target::Unit(a), false),
            (crate::projectile::Target::Unit(b), false)
        ]
    );
}

#[test]
fn chain_damage_deduction_threshold_and_jump_distance_are_distinct() {
    let config = Emitter {
        hit_all: false,
        jump_deduction: 40.0,
        damage_threshold: 30.0,
        ..emitter()
    };
    let (mut sim, player) = scene(config);
    sim.weapon_classes[0].ordnance.max_damage = 100.0;
    let a = victim(&mut sim, 3.0, 0.0);
    let b = victim(&mut sim, 5.0, 0.0);
    let c = victim(&mut sim, 7.0, 0.0);
    sim.fire_force(0, 0, 1.0);
    assert_eq!(sim.unit(a).unwrap().health.cur, 200.0);
    assert_eq!(sim.unit(b).unwrap().health.cur, 240.0);
    assert_eq!(sim.unit(c).unwrap().health.cur, 300.0);
    assert_eq!(sim.unit(player).unwrap().soldier.energy, 90.0);

    let (mut sim, _) = scene(Emitter {
        hit_all: false,
        max_jump_distance: 1.0,
        ..emitter()
    });
    let a = victim(&mut sim, 3.0, 0.0);
    let b = victim(&mut sim, 5.0, 0.0);
    sim.fire_force(0, 0, 1.0);
    assert_eq!(sim.unit(a).unwrap().health.cur, 210.0);
    assert_eq!(sim.unit(b).unwrap().health.cur, 300.0);
}

#[test]
fn first_radius_is_a_chain_filter_and_hit_all_ignores_it() {
    for hit_all in [false, true] {
        let (mut sim, _) = scene(Emitter {
            hit_all,
            first_radius: 0.25,
            ..emitter()
        });
        let target = victim(&mut sim, 3.0, 1.0);
        sim.fire_force(0, 0, 1.0);
        assert_eq!(
            sim.unit(target).unwrap().health.cur,
            if hit_all { 210.0 } else { 300.0 }
        );
    }
}

#[test]
fn a_saber_deflect_stops_emitter_damage_and_burn() {
    let (mut sim, _) = scene(Emitter {
        burn_seconds: 5.0,
        burn_damage_per_second: 50.0,
        ..emitter()
    });
    let target = victim(&mut sim, 3.0, 0.0);
    sim.weapon_classes.push(WeaponClass {
        saber: Some(Box::new(SaberClass {
            combo: manifest::combo::ComboDef {
                name_hash: 1,
                fields: Vec::new(),
            },
            blades: Vec::new(),
            throw_edges: Vec::new(),
            deflect_edges: Vec::new(),
            weapon_bind: glam::Mat4::IDENTITY,
            throw_bindings: Vec::new(),
            clips: Vec::new(),
            state_seconds: Vec::new(),
            damage: 0.0,
            hit_sound: None,
            deflect_sound: None,
            hit_effect: None,
            deflect_effect: None,
        })),
        ..WeaponClass::default()
    });
    let u = sim.unit_mut(target).unwrap();
    u.weapon_class = 1;
    u.soldier.yaw = std::f32::consts::PI;
    u.soldier.energy = 100.0;
    u.weapon.melee = Some(MeleeState {
        deflect: Some(crate::combo::Deflect {
            min_angle: -60.0,
            max_angle: 60.0,
            energy_cost: 2.5,
        }),
        ..Default::default()
    });
    sim.fire_force(0, 0, 1.0);
    let u = sim.unit(target).unwrap();
    assert_eq!(u.health.cur, 300.0);
    assert_eq!(u.soldier.energy, 97.5);
    assert!(u.burn.is_none());
    assert!(sim.events.iter().any(|e| matches!(e, Event::ForceHit {
        target: crate::projectile::Target::Unit(h), blocked: true, ..
    } if *h == target)));
}

#[test]
fn burn_uses_unit_scales_expires_and_keeps_the_latest_owner() {
    let (mut sim, owner) = scene(Emitter {
        burn_seconds: 0.1,
        burn_damage_per_second: 50.0,
        ..emitter()
    });
    let target = victim(&mut sim, 3.0, 0.0);
    sim.weapon_classes[0].ordnance.scales[HealthType::Person as usize] = 0.0;
    sim.fire_force(0, 0, 1.0);
    assert_eq!(sim.unit(target).unwrap().health.cur, 300.0);
    sim.tick_burns(0.05);
    assert_eq!(sim.unit(target).unwrap().health.cur, 297.5);
    sim.tick_burns(0.06);
    assert!(sim.unit(target).unwrap().burn.is_none());
    assert_eq!(sim.unit(target).unwrap().health.cur, 297.5);
    sim.units[target.index as usize].burn = Some(Burn {
        left: 5.0,
        damage_per_second: 1.0,
        owner: target,
        team: 1,
        weapon_class: 0,
    });
    sim.fire_force(0, 0, 1.0);
    let burn = sim.unit(target).unwrap().burn.unwrap();
    assert_eq!(burn.left, 5.0);
    assert_eq!(burn.damage_per_second, 50.0);
    assert_eq!(burn.owner, owner);
    sim.unit_mut(target).unwrap().alive = false;
    sim.tick_burns(TICK_DT);
    assert!(sim.units[target.index as usize].burn.is_none());
}

#[test]
fn burn_hash_covers_every_future_field_and_absence_is_inert() {
    let (mut sim, owner) = scene(emitter());
    let h = sim.hash();
    let burn = Burn {
        left: 5.0,
        damage_per_second: 50.0,
        owner,
        team: 0,
        weapon_class: 0,
    };
    sim.units[0].burn = Some(burn);
    let hash = sim.hash();
    assert_ne!(h, hash);
    for which in 0..6 {
        let mut altered = burn;
        match which {
            0 => altered.left += 1.0,
            1 => altered.damage_per_second += 1.0,
            2 => altered.owner.index += 1,
            3 => altered.owner.gen += 1,
            4 => altered.team = 1,
            _ => altered.weapon_class = 1,
        }
        sim.units[0].burn = Some(altered);
        assert_ne!(sim.hash(), hash, "burn field {which}");
    }
    sim.units[0].burn = None;
    sim.tick_burns(TICK_DT);
    assert_eq!(sim.hash(), h);
}

#[test]
fn emitter_uses_vehicle_health_type_scale_and_includes_it_in_divisor() {
    let (mut sim, player) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    let vehicle = sim.spawn_vehicle(&crate::vehicle::VehicleSpec {
        kind: crate::vehicle::VehicleKind::Hover,
        class: 0,
        home_team: Some(1),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, sim.units[0].muzzle().y, -6.0),
        yaw: 0.0,
        max_health: 500.0,
        health_type: HealthType::Vehicle,
        seats: vec![crate::vehicle::SeatKind::Pilot],
    });
    sim.weapon_classes[0].ordnance.scales[HealthType::Vehicle as usize] = 0.1;
    sim.fire_force(player.index as usize, 0, 1.0);
    assert_eq!(sim.unit(target).unwrap().health.cur, 255.0);
    assert_eq!(sim.vehicle(vehicle).unwrap().health.cur, 495.5);
}

#[test]
fn zero_authored_damage_does_not_apply_the_ordinary_burn() {
    let (mut sim, _) = scene(Emitter {
        burn_seconds: 5.0,
        burn_damage_per_second: 50.0,
        ..emitter()
    });
    let target = victim(&mut sim, 3.0, 0.0);
    sim.weapon_classes[0].ordnance.max_damage = 0.0;
    sim.weapon_classes[0].ordnance.min_damage = Some(0.0);
    sim.fire_force(0, 0, 1.0);
    assert_eq!(sim.unit(target).unwrap().health.cur, 300.0);
    assert!(sim.unit(target).unwrap().burn.is_none());
}

#[test]
fn an_ai_emitter_hits_unperceived_bystanders_but_still_respects_walls() {
    for ai in [false, true] {
        let (mut sim, owner) = scene(emitter());
        if ai {
            sim.unit_mut(owner).unwrap().controller = crate::unit::Controller::Ai;
        }
        sim.friendly_fire_scale = 1.0;
        sim.weapon_classes[0].force.as_mut().unwrap().affect_friends = true;
        let ally = victim(&mut sim, 3.0, 0.0);
        sim.unit_mut(ally).unwrap().team = 0;
        let enemy = victim(&mut sim, 7.0, 0.0);
        sim.collision.push_body(crate::collision::Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 1.0, -5.0)),
            crate::collision::Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        sim.fire_force(owner.index as usize, 0, 1.0);
        assert_eq!(
            sim.unit(ally).unwrap().health.cur,
            255.0,
            "AI {ai}: physical ally"
        );
        assert_eq!(
            sim.unit(enemy).unwrap().health.cur,
            300.0,
            "AI {ai}: wall blocks"
        );
    }
}

#[path = "choke_review/tests.rs"]
mod choke_review;

#[path = "choke_tests.rs"]
mod choke_tests;

#[test]
fn a_force_push_throws_a_soldier_and_only_hurts_a_droideka() {
    let (mut sim, player) = scene(emitter());
    sim.weapon_classes[0].force.as_mut().unwrap().push = 30.0;
    let soldier = victim(&mut sim, 3.0, 0.0);
    let y = sim.units[0].muzzle().y - 1.0;
    let deka = sim
        .spawn(&UnitSpec {
            droideka: Some(std::sync::Arc::new(Default::default())),
            ..UnitSpec::trooper(1, Vec3::new(0.0, y, -5.0))
        })
        .unwrap();
    sim.fire_force(player.index as usize, 0, 1.0);
    let knocked = |sim: &Sim, h| sim.unit(h).unwrap().soldier.stance.knocked();
    assert!(knocked(&sim, soldier), "the control is thrown");
    assert!(
        !knocked(&sim, deka),
        "its mover never steps a soldier stance"
    );
    assert!(
        sim.unit(deka).unwrap().health.cur < 300.0,
        "it is still hit"
    );
}

#[test]
fn force_push_uses_native_lift_falloff_and_bf2_reactions_on_both_maps() {
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        let (mut sim, _) = scene(emitter());
        sim.ground_rules = rules;
        sim.weapon_classes[0].force.as_mut().unwrap().push = 15.0;
        sim.weapon_classes[0].force.as_mut().unwrap().max_targets = 4;
        let targets: Vec<_> = [3.0, 8.0, 14.0, 19.0, 19.5]
            .into_iter()
            .map(|z| victim(&mut sim, z, 0.0))
            .collect();
        for h in &targets {
            sim.unit_mut(*h).unwrap().soldier.grounded = true;
        }
        sim.fire_force(0, 0, 1.0);
        let from = sim.units[0].muzzle();
        for (k, h) in targets.iter().take(4).enumerate() {
            let s = &sim.unit(*h).unwrap().soldier;
            assert_eq!(s.stance, crate::soldier::Stance::Thrown);
            assert!(s.vel.y > 3.0);
            let delta = s.pos + Vec3::Y - from;
            let length = delta.length();
            let strength = if k < 2 {
                15.0
            } else {
                15.0 * (40.0 - (length - 1.462_635 * 0.5)) / 30.0
            };
            let expected = strength * -delta.z / length;
            assert!(
                (-s.vel.z - expected).abs() < 0.001,
                "{rules:?} {k}: {:?}",
                s.vel
            );
        }
        assert_eq!(sim.unit(targets[4]).unwrap().soldier.vel, Vec3::ZERO);
    }
}

#[test]
fn force_pull_replaces_momentum_and_scales_from_beyond_the_target() {
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        let (mut sim, _) = scene(emitter());
        sim.ground_rules = rules;
        sim.weapon_classes[0].force.as_mut().unwrap().push = -35.0;
        let h = victim(&mut sim, 6.0, 0.0);
        let s = &mut sim.unit_mut(h).unwrap().soldier;
        s.grounded = true;
        s.vel = Vec3::new(9.0, 0.0, -12.0);
        sim.fire_force(0, 0, 1.0);
        let s = &sim.unit(h).unwrap().soldier;
        assert_eq!(s.stance, crate::soldier::Stance::Thrown);
        assert_eq!(s.vel.x, 0.0);
        assert!((s.vel.z - 10.706_005).abs() < 0.001, "{:?}", s.vel);
        assert!((s.vel.y - 7.198_039).abs() < 0.001, "{:?}", s.vel);
    }
}

#[test]
fn force_flight_and_recovery_keep_bf2_dynamics_on_both_maps() {
    for strength in [15.0, -35.0] {
        let mut paths = Vec::new();
        for rules in [
            crate::soldier::GroundRules::Bf1,
            crate::soldier::GroundRules::Bf2,
        ] {
            let (mut sim, _) = scene(emitter());
            sim.ground_rules = rules;
            sim.weapon_classes[0].force.as_mut().unwrap().push = strength;
            let h = victim(&mut sim, 6.0, 0.0);
            sim.unit_mut(h).unwrap().soldier.pos.y = 0.0;
            sim.fire_force(0, 0, 1.0);
            let mut path = Vec::new();
            for _ in 0..120 {
                sim.step(&[PadState::default(), PadState::default()]);
                let s = &sim.unit(h).unwrap().soldier;
                path.push((s.pos, s.vel, s.stance));
            }
            assert_eq!(path.last().unwrap().2, crate::soldier::Stance::Stand);
            let distance = (path.last().unwrap().0.z + 6.0).abs();
            assert!((3.0..25.0).contains(&distance), "{strength}: {distance}");
            paths.push(path);
        }
        for (a, b) in paths[0].iter().zip(&paths[1]) {
            assert!(a.0.distance(b.0) < 0.001, "{strength}: {a:?} / {b:?}");
            assert!(a.1.distance(b.1) < 0.001);
            assert_eq!(a.2, b.2);
        }
    }
}

#[test]
fn force_emitter_damage_is_paid_by_a_raised_droideka_shield() {
    let (mut sim, player) = scene(emitter());
    let y = sim.units[0].muzzle().y - 1.0;
    let target = sim
        .spawn(&UnitSpec {
            droideka: Some(std::sync::Arc::new(Default::default())),
            ..UnitSpec::trooper(1, Vec3::new(0.0, y, -4.0))
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
    sim.fire_force(player.index as usize, 0, 1.0);
    let u = sim.unit(target).unwrap();
    assert_eq!(u.health.cur, before);
    assert_eq!(u.droideka.as_ref().unwrap().shield.points, 2910.0);
}

/// PC BF2: an emitter (push, pull, lightning) damages through the call that
/// sets no hit side; a saber swing or throw (`hero_damage`) is direct.
#[test]
fn emitter_hits_are_not_direct_and_saber_hits_are() {
    let (mut sim, player) = scene(emitter());
    let y = sim.units[0].muzzle().y - 1.0;
    let target = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, y, -4.0)))
        .unwrap();
    let direct = |sim: &Sim| {
        sim.events.iter().rev().find_map(|e| match e {
            Event::Hit { victim, direct, .. } if *victim == target => Some(*direct),
            _ => None,
        })
    };
    sim.fire_force(player.index as usize, 0, 1.0);
    assert_eq!(direct(&sim), Some(false));
    let mut desc = sim.weapon_classes[0].ordnance.damage_desc(0.0, false);
    desc.damage = 1.0;
    sim.hero_damage(player.index as usize, target, desc, Vec3::NEG_Z);
    assert_eq!(direct(&sim), Some(true));
}
