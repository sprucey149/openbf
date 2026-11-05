use super::*;
use crate::cp::CpDef;
use crate::sim::tests::{flat_ground, sphere_cp};
use crate::sim::DeathCause;
use crate::unit::{Controller, UnitClass, UnitSpec};
use glam::Vec3;

fn class(min: u32, max: u32) -> UnitClass {
    UnitClass {
        consts: crate::soldier::SoldierConstants::default(),
        max_health: 300.0,
        health_type: crate::weapon::HealthType::Person,
        weapon_class: 0,
        total_clips: 6,
        weapon_slot: 0,
        slots: std::sync::Arc::new([]),
        jet: false,
        capture_posts: true,
        drops: crate::sim::DropTable::default(),
        chunk: None,
        min,
        max: Some(max),
        unlock_points: 0,
        droideka: None,
    }
}

fn arena(limit: u32) -> (Sim, UnitHandle, UnitHandle, UnitHandle) {
    let mut sim = Sim::empty(7);
    sim.heightfield = Some(flat_ground());
    sim.battle = crate::battle::Battle::new([999_999; 2]);
    sim.deathmatch = Some(Deathmatch::new(limit, [12, 12]));
    let mut bot = UnitSpec::trooper(0, Vec3::new(-5.0, 0.0, 0.0));
    bot.controller = Controller::Ai;
    let first = sim.spawn(&bot).unwrap();
    bot.pos.x = -8.0;
    let ally = sim.spawn(&bot).unwrap();
    bot.team = 1;
    bot.pos.x = 5.0;
    let enemy = sim.spawn(&bot).unwrap();
    (sim, first, ally, enemy)
}

fn roster(table: Vec<UnitClass>) -> Sim {
    let mut sim = Sim::empty(3);
    sim.heightfield = Some(flat_ground());
    sim.battle = crate::battle::Battle::new([999_999; 2]);
    sim.deathmatch = Some(Deathmatch::new(100, [12, 12]));
    sim.classes[0] = table;
    sim
}

fn field(sim: &mut Sim, class: u8, x: f32) -> UnitHandle {
    let skill = sim.bot_skill(0);
    let spec = sim.classes[0][usize::from(class)].spec(
        0,
        Vec3::new(x, 0.0, 0.0),
        0.0,
        Controller::Ai,
        class,
        skill,
    );
    sim.spawn(&spec).unwrap()
}

#[test]
fn an_enemy_kill_scores_for_the_killers_team() {
    let (mut sim, first, _, enemy) = arena(100);
    sim.unit_killed_with_cause(enemy, Some(first), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([1, 0], 100)));
    assert_eq!(sim.outcome, None);
}

#[test]
fn team_kills_suicides_and_the_environment_score_nothing() {
    let (mut sim, first, ally, enemy) = arena(100);
    sim.unit_killed_with_cause(ally, Some(first), DeathCause::Suicide);
    sim.unit_killed_with_cause(first, Some(first), DeathCause::Suicide);
    sim.kill_environment(enemy, DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 0], 100)));
}

#[test]
fn a_killer_who_has_since_respawned_still_scores() {
    let (mut sim, first, _, enemy) = arena(100);
    let earlier_life = UnitHandle {
        index: first.index,
        gen: first.gen.wrapping_sub(1),
    };
    sim.unit_killed_with_cause(enemy, Some(earlier_life), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([1, 0], 100)));
}

#[test]
fn reaching_the_limit_ends_the_round_and_stops_the_count() {
    let (mut sim, first, ally, enemy) = arena(1);
    sim.unit_killed_with_cause(first, Some(enemy), DeathCause::Suicide);
    assert_eq!(sim.outcome, Some(1));
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::TeamDefeated { team: 0 })));
    sim.unit_killed_with_cause(enemy, Some(ally), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 1], 1)));
    assert_eq!(sim.outcome, Some(1));
}

#[test]
fn posts_neither_bleed_nor_dominate_nor_count_occupants() {
    let (mut sim, first, _, _) = arena(100);
    let def: CpDef = sphere_cp("a", Vec3::new(-5.0, 0.0, 0.0), 10.0, false);
    sim.add_cp(def, Some(1));
    sim.update_cps(1.0);
    assert!(!sim.counts_in_cp(first.index as usize, 0));
    assert_eq!(sim.cps[0].owner, Some(1));
    assert_eq!(sim.battle.teams[0].bleed_per_sec, 0.0);
    assert_eq!(sim.domination, None);
    assert_eq!(sim.outcome, None);
}

#[test]
fn the_picker_fields_one_of_each_while_the_reserve_holds() {
    let mut sim = roster(vec![class(1, 2), class(1, 2), class(1, 2)]);
    let mut picked: Vec<u8> = (0..3)
        .map(|k| {
            let c = sim.assault_class(0, None).unwrap();
            field(&mut sim, c, k as f32 * 3.0);
            c
        })
        .collect();
    picked.sort_unstable();
    assert_eq!(picked, [0, 1, 2]);
    assert_eq!(sim.assault_class(0, None), None);
}

#[test]
fn after_the_first_minute_a_humanless_team_reaches_each_maximum() {
    let mut sim = roster(vec![class(1, 2)]);
    sim.tick = CLASS_RESERVE_SECS * u64::from(TICK_HZ);
    for k in 0..2 {
        let c = sim.assault_class(0, None).unwrap();
        field(&mut sim, c, k as f32 * 3.0);
    }
    assert_eq!(sim.assault_class(0, None), None);
}

#[test]
fn a_human_on_the_team_keeps_the_reserve_after_the_first_minute() {
    let mut sim = roster(vec![class(1, 2), class(1, 2)]);
    sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    sim.tick = CLASS_RESERVE_SECS * u64::from(TICK_HZ);
    assert_eq!(sim.assault_class(0, None), Some(1));
    field(&mut sim, 1, 3.0);
    assert_eq!(sim.assault_class(0, None), None);
}

#[test]
fn a_member_alone_on_its_class_can_come_back_as_it() {
    let mut sim = roster(vec![class(1, 2)]);
    let member = field(&mut sim, 0, 2.0);
    assert_eq!(sim.assault_class(0, None), None);
    assert_eq!(sim.assault_class(0, Some(member.index as usize)), Some(0));
}

#[test]
fn the_score_reaches_the_hash() {
    let (mut sim, _, _, _) = arena(100);
    let before = sim.hash();
    sim.deathmatch.as_mut().unwrap().points[0] = 1;
    assert_ne!(before, sim.hash());
}

#[test]
fn a_sim_without_the_mode_hashes_as_it_did() {
    let (mut sim, first, _, enemy) = arena(100);
    sim.deathmatch = None;
    let before = sim.hash();
    sim.deathmatch_kill(first, 1);
    assert_eq!(before, sim.hash());
    assert_eq!(sim.team_points(), None);
    let _ = enemy;
}
