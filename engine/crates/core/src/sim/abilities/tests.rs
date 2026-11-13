use super::*;
use crate::collision::{Body, Shape};
use crate::sim::{buttons, Buffs, PadState};
use crate::unit::{Controller, SlotSpec, UnitHandle, UnitSpec};
use crate::weapon::{ExplosionClass, WeaponClass};
use glam::Mat4;

fn boost() -> Ability {
    Ability {
        radius: 10.0,
        friends: true,
        enemies: false,
        self_on_hit: true,
        offense: (15.0, 1.5),
        defense: (20.0, 0.5),
        regeneration: (30.0, 20.0),
        poison: (0.0, 0.0),
        activation_effect: None,
    }
}

fn setup(ability: Ability) -> (Sim, UnitHandle, usize) {
    let mut sim = Sim::empty(47);
    sim.heightfield = Some(crate::sim::tests::flat_ground());
    let class = sim.add_weapon_class(WeaponClass {
        rounds_per_clip: 1,
        shot_delay: [1.0; 2],
        initial_salvo_delay: 0.4,
        reload_time: 10.0,
        trigger_single: true,
        ability: Some(ability),
        ..Default::default()
    });
    let mut spec = UnitSpec::player(0, Vec3::ZERO);
    spec.slots = vec![SlotSpec {
        slot: 2,
        channel: 1,
        weapon_class: class,
        clips: 1,
    }]
    .into();
    let owner = sim.spawn(&spec).unwrap();
    (sim, owner, class)
}

#[test]
fn boost_reaches_friends_in_every_direction_and_the_owner_after_a_hit() {
    let (mut sim, owner, class) = setup(boost());
    let ally = sim.spawn(&UnitSpec::trooper(0, Vec3::Z * 5.0)).unwrap();
    let enemy = sim.spawn(&UnitSpec::trooper(1, Vec3::X * 5.0)).unwrap();
    let far = sim.spawn(&UnitSpec::trooper(0, Vec3::X * 20.0)).unwrap();
    sim.fire_ability(owner.index as usize, class);
    assert_eq!(sim.unit(ally).unwrap().buffs.offense, (15.0, 1.5));
    assert_eq!(sim.unit(owner).unwrap().buffs.defense, (20.0, 0.5));
    assert!(!sim.unit(enemy).unwrap().buffs.active());
    assert!(!sim.unit(far).unwrap().buffs.active());
    assert!(sim.bolts.is_empty() && sim.stickies.is_empty());
}

#[test]
fn an_isolated_caster_does_not_get_its_own_boost() {
    let (mut sim, owner, class) = setup(boost());
    sim.fire_ability(owner.index as usize, class);
    assert!(!sim.unit(owner).unwrap().buffs.active());
}

#[test]
fn hero_refuses_officer_buffs_and_poison_without_changing_regular_recipients() {
    use crate::soldier::GroundRules;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let (mut sim, owner, class) = setup(Ability {
            enemies: true,
            poison: (10.0, 20.0),
            ..boost()
        });
        sim.ground_rules = rules;
        sim.heroes = Some(crate::sim::heroes::Heroes::new([Some(1), Some(1)]));
        let ally = sim
            .spawn(&UnitSpec {
                class: 1,
                ..UnitSpec::trooper(0, Vec3::Z * 5.0)
            })
            .unwrap();
        let enemy = sim
            .spawn(&UnitSpec {
                class: 1,
                ..UnitSpec::trooper(1, Vec3::NEG_Z * 5.0)
            })
            .unwrap();
        let regular = sim.spawn(&UnitSpec::trooper(0, Vec3::X * 5.0)).unwrap();
        sim.fire_ability(owner.index as usize, class);
        assert!(!sim.unit(ally).unwrap().buffs.active());
        assert!(!sim.unit(enemy).unwrap().buffs.active());
        assert!(sim.unit(regular).unwrap().buffs.active());
        assert!(sim.unit(owner).unwrap().buffs.active());
    }
}

#[test]
fn walls_block_the_area_query_and_do_not_enable_self_boost() {
    let (mut sim, owner, class) = setup(boost());
    let ally = sim.spawn(&UnitSpec::trooper(0, Vec3::Z * -5.0)).unwrap();
    sim.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 1.0, -2.5)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    sim.fire_ability(owner.index as usize, class);
    assert!(!sim.unit(ally).unwrap().buffs.active());
    assert!(!sim.unit(owner).unwrap().buffs.active());
}

#[test]
fn the_radius_path_does_not_use_the_cone_max_targets_limit() {
    let (mut sim, owner, class) = setup(boost());
    for n in 0..16 {
        sim.spawn(&UnitSpec::trooper(0, Vec3::new(n as f32 * 0.2, 0.0, 4.0)))
            .unwrap();
    }
    sim.fire_ability(owner.index as usize, class);
    assert_eq!(
        sim.alive_units().filter(|(_, u)| u.buffs.active()).count(),
        17
    );
}

#[test]
fn effects_keep_the_longer_remaining_timer_and_replace_its_strength_together() {
    let owner = UnitHandle { index: 0, gen: 0 };
    let mut buffs = Buffs::default();
    buffs.apply_ability(&boost(), owner);
    let mut replacement = boost();
    replacement.offense = (10.0, 2.0);
    buffs.apply_ability(&replacement, owner);
    assert_eq!(buffs.offense, (15.0, 1.5));
    buffs.tick(6.0);
    buffs.apply_ability(&replacement, owner);
    assert_eq!(buffs.offense, (10.0, 2.0));
    buffs.tick(31.0);
    assert!(!buffs.active());
}

#[test]
fn regeneration_is_capped_and_stops_when_the_original_timer_expires() {
    let (mut sim, owner, _) = setup(boost());
    let u = sim.unit_mut(owner).unwrap();
    u.buffs.regeneration = (3.0, 20.0);
    u.health.cur = 100.0;
    sim.tick_buffs(1.0);
    assert_eq!(sim.unit(owner).unwrap().health.cur, 120.0);
    sim.tick_buffs(1.0);
    sim.tick_buffs(1.0);
    assert_eq!(sim.unit(owner).unwrap().health.cur, 140.0);
    let u = sim.unit_mut(owner).unwrap();
    u.buffs.regeneration = (3.0, 20.0);
    u.health.cur = u.health.max - 1.0;
    sim.tick_buffs(1.0);
    assert_eq!(
        sim.unit(owner).unwrap().health.cur,
        sim.unit(owner).unwrap().health.max
    );
}

#[test]
fn poison_is_enemy_only_persists_after_the_caster_dies_and_credits_the_kill() {
    let mut ability = boost();
    ability.friends = false;
    ability.enemies = true;
    ability.self_on_hit = false;
    ability.offense.0 = 0.0;
    ability.defense.0 = 0.0;
    ability.regeneration.0 = 0.0;
    ability.poison = (15.0, 30.0);
    let (mut sim, owner, class) = setup(ability);
    let enemy = sim.spawn(&UnitSpec::trooper(1, Vec3::Z * -4.0)).unwrap();
    let ally = sim.spawn(&UnitSpec::trooper(0, Vec3::Z * 4.0)).unwrap();
    sim.fire_ability(owner.index as usize, class);
    assert_eq!(sim.unit(enemy).unwrap().buffs.poison, (15.0, 30.0));
    assert!(!sim.unit(owner).unwrap().buffs.active());
    assert!(!sim.unit(ally).unwrap().buffs.active());
    sim.units[owner.index as usize].alive = false;
    for _ in 0..10 {
        sim.tick_buffs(1.0);
    }
    assert!(sim.unit(enemy).is_none());
    assert_eq!(sim.units[owner.index as usize].kills, 1);
    assert!(sim.events.iter().any(
        |e| matches!(e, Event::Kill { shooter, victim } if *shooter == owner && *victim == enemy)
    ));
    assert!(!sim.units[enemy.index as usize].buffs.active());
}

#[test]
fn native_teams_take_the_effect_their_relation_to_the_caster_names() {
    let mut poison = boost();
    poison.friends = false;
    poison.enemies = true;
    poison.self_on_hit = false;
    poison.regeneration.0 = 0.0;
    poison.offense.0 = 0.0;
    poison.defense.0 = 0.0;
    poison.poison = (15.0, 20.0);
    for (ability, friend_hit, foe_hit) in [(boost(), true, false), (poison, false, true)] {
        let (mut sim, owner, class) = setup(ability);
        sim.set_team_relation(0, 2, true);
        sim.set_team_relation(2, 0, true);
        sim.set_team_relation(0, 3, false);
        sim.set_team_relation(3, 0, false);
        let friend = sim.spawn(&UnitSpec::trooper(1, Vec3::Z * 4.0)).unwrap();
        let foe = sim.spawn(&UnitSpec::trooper(1, Vec3::Z * -4.0)).unwrap();
        sim.units[friend.index as usize].team = 2;
        sim.units[foe.index as usize].team = 3;
        sim.fire_ability(owner.index as usize, class);
        assert_eq!(sim.unit(friend).unwrap().buffs.active(), friend_hit);
        assert_eq!(sim.unit(foe).unwrap().buffs.active(), foe_hit);
    }
}

#[test]
fn poison_stops_at_fifteen_seconds_and_never_restarts_after_expiration() {
    let (mut sim, owner, _) = setup(boost());
    let enemy = sim.spawn(&UnitSpec::trooper(1, Vec3::Z * -4.0)).unwrap();
    let u = sim.unit_mut(enemy).unwrap();
    u.health.cur = 1000.0;
    u.health.max = 1000.0;
    u.buffs.poison = (15.0, 30.0);
    u.buffs.poisoned_by = Some(owner);
    for _ in 0..29 {
        sim.tick_buffs(0.5);
    }
    assert_eq!(sim.unit(enemy).unwrap().health.cur, 565.0);
    assert_eq!(sim.unit(enemy).unwrap().buffs.poison.0, 0.5);
    sim.tick_buffs(0.5);
    assert_eq!(sim.unit(enemy).unwrap().buffs.poison.0, 0.0);
    assert_eq!(sim.unit(enemy).unwrap().buffs.poisoned_by, None);
    sim.tick_buffs(15.0);
    assert_eq!(sim.unit(enemy).unwrap().health.cur, 565.0);
}

#[test]
fn damage_and_defense_multiply_real_blast_damage_and_expire_independently() {
    let (mut sim, owner, _) = setup(boost());
    let enemy = sim.spawn(&UnitSpec::trooper(1, Vec3::Z * -5.0)).unwrap();
    sim.unit_mut(owner).unwrap().buffs.offense = (2.0, 1.5);
    sim.unit_mut(enemy).unwrap().buffs.defense = (3.0, 0.5);
    let blast = ExplosionClass {
        damage: 40.0,
        damage_inner: 2.0,
        damage_outer: 3.0,
        ..Default::default()
    };
    sim.explode(&blast, Vec3::new(0.0, 1.0, -5.0), Some(owner), Some(0));
    assert_eq!(sim.unit(enemy).unwrap().health.cur, 270.0);
    sim.tick_buffs(2.0);
    sim.explode(&blast, Vec3::new(0.0, 1.0, -5.0), Some(owner), Some(0));
    assert_eq!(sim.unit(enemy).unwrap().health.cur, 250.0);
    sim.tick_buffs(1.0);
    sim.explode(&blast, Vec3::new(0.0, 1.0, -5.0), Some(owner), Some(0));
    assert_eq!(sim.unit(enemy).unwrap().health.cur, 210.0);
}

#[test]
fn secondary_activation_has_a_windup_spends_one_use_and_never_spawns_a_bolt() {
    let (mut sim, owner, _) = setup(boost());
    sim.spawn(&UnitSpec::trooper(0, Vec3::Z * -5.0)).unwrap();
    let mut activations = Vec::new();
    for tick in 0..120 {
        sim.step(&[PadState {
            buttons: if tick % 30 == 0 {
                buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        }]);
        if sim
            .events
            .iter()
            .any(|e| matches!(e, Event::Ability { .. }))
        {
            activations.push(tick);
        }
    }
    assert_eq!(activations.len(), 1);
    assert!((11..=14).contains(&activations[0]));
    assert_eq!(sim.unit(owner).unwrap().held[0].state.clip, 0);
    assert_eq!(sim.unit(owner).unwrap().held[0].state.spare, 0);
    assert!(sim.bolts.is_empty() && sim.stickies.is_empty());
}

#[test]
fn regeneration_poison_and_its_attacker_are_part_of_the_replay_hash() {
    let (mut sim, owner, _) = setup(boost());
    let initial = sim.hash();
    sim.unit_mut(owner).unwrap().buffs.regeneration = (3.0, 20.0);
    assert_ne!(initial, sim.hash());
    sim.unit_mut(owner).unwrap().buffs.regeneration.0 = 0.0;
    assert_eq!(initial, sim.hash());
    sim.unit_mut(owner).unwrap().buffs.poison = (3.0, 30.0);
    sim.unit_mut(owner).unwrap().buffs.poisoned_by = Some(owner);
    let first = sim.hash();
    sim.unit_mut(owner).unwrap().buffs.poisoned_by = Some(UnitHandle {
        gen: owner.gen + 1,
        ..owner
    });
    assert_ne!(first, sim.hash());
}

#[test]
fn bots_use_a_heal_for_a_wounded_friend_and_wait_when_everyone_is_healthy() {
    let (mut sim, owner, _) = setup(boost());
    sim.unit_mut(owner).unwrap().controller = Controller::Ai;
    let ally = sim.spawn(&UnitSpec::trooper(0, Vec3::Z * -4.0)).unwrap();
    assert_eq!(crate::ai::ability_input(&sim, owner), None);
    sim.unit_mut(ally).unwrap().health.cur = 100.0;
    assert_eq!(
        crate::ai::ability_input(&sim, owner),
        Some(buttons::SECONDARY_FIRE)
    );
    sim.unit_mut(ally).unwrap().buffs.regeneration = (30.0, 20.0);
    assert_eq!(crate::ai::ability_input(&sim, owner), None);
}

#[test]
fn the_resupplied_use_obeys_the_authored_reload_and_refills_after_ten_seconds() {
    let (mut sim, owner, class) = setup(boost());
    let wc = sim.weapon_classes[class].clone();
    let state = &mut sim.unit_mut(owner).unwrap().held[0].state;
    state.clip = 0;
    state.spare = 1;
    assert!(state.start_reload(&wc));
    let mut rng = crate::lcg::GameLcg::new(47);
    for _ in 0..9 {
        let wc = &sim.weapon_classes[class];
        sim.units[owner.index as usize].held[0].state.update(
            1.0,
            wc,
            crate::weapon::FireInput::default(),
            &mut rng,
            &mut Vec::new(),
        );
    }
    assert_eq!(sim.unit(owner).unwrap().held[0].state.clip, 0);
    let wc = &sim.weapon_classes[class];
    sim.units[owner.index as usize].held[0].state.update(
        1.0,
        wc,
        crate::weapon::FireInput::default(),
        &mut rng,
        &mut Vec::new(),
    );
    assert_eq!(sim.unit(owner).unwrap().held[0].state.clip, 1);
    assert_eq!(sim.unit(owner).unwrap().held[0].state.spare, 0);
}
