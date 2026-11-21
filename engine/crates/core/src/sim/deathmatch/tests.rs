use super::*;
use crate::cp::CpDef;
use crate::sim::tests::{flat_ground, sphere_cp};
use crate::sim::DeathCause;
use crate::unit::{Controller, UnitClass, UnitSpec};
use crate::TICK_HZ;
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
    assert_eq!(sim.team_points(), Some(([1, 0], [100; 2])));
    assert_eq!(sim.outcome, None);
}

#[test]
fn xl_counts_up_to_350_and_ends_on_the_winning_kill() {
    let (mut sim, first, ally, enemy) = arena(XL_SCORE_LIMIT);
    sim.deathmatch = Some(Deathmatch::xl([12, 12]));
    assert!(sim.is_xl());
    assert!(!sim.hero_assault());
    assert!(!sim.is_hunt());
    assert_eq!(sim.team_points(), Some(([0, 0], [350; 2])));
    sim.unit_killed_with_cause(ally, Some(first), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 0], [350; 2])));
    sim.unit_killed_with_cause(first, Some(enemy), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 1], [350; 2])));
    assert_eq!(sim.outcome, None);

    let replacement = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    sim.deathmatch.as_mut().unwrap().points[1] = 349;
    sim.unit_killed_with_cause(replacement, Some(enemy), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 350], [350; 2])));
    assert_eq!(sim.outcome, Some(1));
}

/// An XL round with a time limit ends on the clock like the other
/// deathmatches: the side further along wins.
#[test]
fn xl_ends_on_its_time_limit() {
    let (mut sim, _, _, _) = arena(XL_SCORE_LIMIT);
    sim.deathmatch = Some(Deathmatch::xl([12, 12]));
    sim.deathmatch.as_mut().unwrap().points = [3, 7];
    sim.time_limit = Some(2);
    sim.tick = u64::from(TICK_HZ);
    sim.update_cps(1.0 / TICK_HZ as f32);
    assert_eq!(sim.outcome, None, "one second in");
    sim.tick = 2 * u64::from(TICK_HZ);
    sim.update_cps(1.0 / TICK_HZ as f32);
    assert_eq!(sim.outcome, Some(1));
}

#[test]
fn hunt_scores_one_per_kill_for_either_side_and_ends_without_hero_rules() {
    let (mut sim, first, ally, enemy) = arena(3);
    sim.deathmatch = Some(Deathmatch::hunt(3, [12, 12]));
    assert!(sim.is_hunt() && !sim.hero_assault());
    assert!(!sim.hero_unit(sim.unit(first).unwrap()));
    sim.unit_killed_with_cause(first, Some(enemy), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 1], [3; 2])));
    sim.unit_killed_with_cause(enemy, Some(ally), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([1, 1], [3; 2])));
    assert_eq!(sim.outcome, None);
}

#[test]
fn team_kills_suicides_and_the_environment_score_nothing() {
    let (mut sim, first, ally, enemy) = arena(100);
    sim.unit_killed_with_cause(ally, Some(first), DeathCause::Suicide);
    sim.unit_killed_with_cause(first, Some(first), DeathCause::Suicide);
    sim.kill_environment(enemy, DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 0], [100; 2])));
}

#[test]
fn a_native_death_scores_for_no_side() {
    let (mut sim, first, _, _) = arena(100);
    let mut native = UnitSpec::trooper(2, Vec3::ZERO);
    native.controller = Controller::Ai;
    sim.add_native_team(2, vec![native]);
    let native = sim
        .spawn_native_character(2, (Vec3::new(0.0, 0.0, 5.0), 0.0))
        .unwrap();
    sim.unit_killed_with_cause(native, Some(first), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([0, 0], [100; 2])));
}

#[test]
fn a_killer_who_has_since_respawned_still_scores() {
    let (mut sim, first, _, enemy) = arena(100);
    let earlier_life = UnitHandle {
        index: first.index,
        gen: first.gen.wrapping_sub(1),
    };
    sim.unit_killed_with_cause(enemy, Some(earlier_life), DeathCause::Suicide);
    assert_eq!(sim.team_points(), Some(([1, 0], [100; 2])));
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
    assert_eq!(sim.team_points(), Some(([0, 1], [1; 2])));
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

#[test]
fn each_hunt_side_wins_at_its_own_target() {
    let (mut sim, first, ally, enemy) = arena(100);
    sim.deathmatch = Some(Deathmatch::hunt_with_targets([3, 2], [12, 12]));
    for _ in 0..2 {
        sim.unit_killed_with_cause(enemy, Some(first), DeathCause::Suicide);
    }
    assert_eq!(sim.outcome, None, "team 0 needs 3");
    sim.unit_killed_with_cause(first, Some(enemy), DeathCause::Suicide);
    assert_eq!(sim.outcome, None);
    sim.unit_killed_with_cause(ally, Some(enemy), DeathCause::Suicide);
    assert_eq!(sim.outcome, Some(1), "team 1 wins at its 2");
    assert_eq!(sim.team_points(), Some(([2, 2], [3, 2])));
}

// The maintainer (2026-11-20): at the timer the side further along toward its own
// target wins, not the side with more raw points (which would favour the larger target).
#[test]
fn at_the_timer_the_larger_share_of_its_own_target_leads() {
    use std::cmp::Ordering;
    let mut round = Deathmatch::hunt_with_targets([60, 40], [12, 12]);
    round.points = [30, 21];
    assert_eq!(round.timer_leader(), Ordering::Less, "21/40 beats 30/60");
    round.points = [30, 20];
    assert_eq!(round.timer_leader(), Ordering::Equal);
    round.points = [31, 20];
    assert_eq!(round.timer_leader(), Ordering::Greater);
    let mut even = Deathmatch::hunt(50, [12, 12]);
    even.points = [10, 9];
    assert_eq!(even.timer_leader(), Ordering::Greater);
}

#[test]
fn split_targets_reach_the_hash_and_even_ones_keep_the_old_layout() {
    let (mut sim, _, _, _) = arena(100);
    sim.deathmatch = Some(Deathmatch::hunt(50, [12, 12]));
    let even = sim.hash();
    sim.deathmatch = Some(Deathmatch::hunt_with_targets([50, 50], [12, 12]));
    assert_eq!(even, sim.hash());
    sim.deathmatch = Some(Deathmatch::hunt_with_targets([50, 40], [12, 12]));
    assert_ne!(even, sim.hash());
    let split = sim.hash();
    sim.deathmatch = Some(Deathmatch::hunt_with_targets([40, 50], [12, 12]));
    assert_ne!(split, sim.hash());
}

// The maintainer (2026-11-20): when a Hunt's clock runs out, the side further along
// toward its own target wins, through the round's real time-limit step.
#[test]
fn a_hunt_clock_gives_the_round_to_the_side_further_along_its_own_target() {
    use crate::sim::OUTCOME_DRAW;
    for (points, winner) in [
        ([30, 21], 1),
        ([31, 20], 0),
        ([30, 20], OUTCOME_DRAW),
        ([5, 4], 1),
    ] {
        let mut sim = Sim::empty(7);
        sim.heightfield = Some(flat_ground());
        sim.battle = crate::battle::Battle::new([999_999; 2]);
        sim.battle.unlimited_reinforcements = true;
        let mut round = Deathmatch::hunt_with_targets([60, 40], [12, 12]);
        round.points = points;
        sim.deathmatch = Some(round);
        sim.time_limit = Some(2);
        for _ in 0..59 {
            sim.step(&[]);
            assert_eq!(sim.outcome, None, "{points:?}: before the clock");
        }
        sim.step(&[]);
        assert_eq!(sim.outcome, Some(winner), "{points:?} of [60, 40]");
        assert!(sim
            .events
            .iter()
            .any(|e| matches!(e, Event::TimeUp { winner: w } if *w == winner)));
    }
}

/// A Hunt with team 0 eased to `pct`, and one AI soldier on each side.
fn eased(pct: u32) -> (Sim, UnitHandle, UnitHandle) {
    let (mut sim, first, _, enemy) = arena(50);
    sim.deathmatch = Some(Deathmatch::hunt(50, [12, 12]).with_skill_pct([pct, 100]));
    (sim, first, enemy)
}

#[test]
fn a_hunt_ease_never_goes_below_the_30_percent_floor() {
    assert_eq!(HUNT_SKILL_FLOOR, 30);
    let round = Deathmatch::hunt(50, [12, 12]).with_skill_pct([1, 250]);
    assert_eq!(round.skill_pct(), [30, 100]);
    let round = Deathmatch::hunt(50, [12, 12]).with_skill_pct([29, 30]);
    assert_eq!(round.skill_pct(), [30, 30]);
}

#[test]
fn an_eased_hunt_bot_mainly_misses_farther_and_keeps_its_level() {
    use crate::skill::Difficulty;
    let (mut sim, first, enemy) = eased(50);
    sim.difficulty = Difficulty::Hard;
    // The level is the tier's: the ease is in the aim.
    assert_eq!([sim.bot_skill(0), sim.bot_skill(1)], [20, 20]);
    let widen = |sim: &Sim, h| sim.hunt_aim_widen(sim.unit(h).unwrap());
    assert_eq!(widen(&sim, first), 4.0, "(100 / 50) squared");
    assert_eq!(widen(&sim, enemy), 1.0, "the side at 100 aims as before");
    sim.deathmatch = Some(Deathmatch::hunt(50, [12, 12]).with_skill_pct([30, 100]));
    assert!(
        (widen(&sim, first) - 100.0 / 9.0).abs() < 1e-4,
        "about 11x at the floor"
    );
    // A human on the eased side aims for himself.
    sim.units[first.index as usize].is_player = true;
    assert_eq!(widen(&sim, first), 1.0);
}

#[test]
fn an_eased_hunt_bot_still_fires_within_a_second_of_a_clear_sight() {
    use crate::skill::Difficulty;
    for tier in [
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::Elite,
    ] {
        let (mut sim, ..) = eased(100);
        sim.difficulty = tier;
        let plain = sim.ai_tier(0).reaction_ticks;
        for pct in [30, 50, 70] {
            sim.deathmatch = Some(Deathmatch::hunt(50, [12, 12]).with_skill_pct([pct, 100]));
            let ticks = sim.ai_tier(0).reaction_ticks;
            assert!(ticks >= plain, "{tier:?} {pct}: a little slower");
            assert!(
                ticks <= HUNT_REACTION_MAX,
                "{tier:?} {pct}: {ticks} ticks is over 1 s"
            );
            assert_eq!(
                sim.ai_tier(1).reaction_ticks,
                plain,
                "the other side is not eased"
            );
        }
    }
}

#[test]
fn an_eased_hunt_bot_fires_a_weapon_that_needs_no_aim_less_often() {
    use crate::weapon::{test_props::List, EmitterOrdnance, FireInput, WeaponState};
    let (sim, first, enemy) = eased(50);
    let rifle = sim.weapon_classes[0].clone();
    let mut cone = rifle.clone();
    cone.emitter = Some(EmitterOrdnance::from_props(
        &List(&[(2_470_609_998, "10"), (2_750_430_025, "50")]),
        true,
    ));
    // Shots in 3 s of a held trigger at each tick's weapon step.
    let shots = |h: UnitHandle, wc: &crate::weapon::WeaponClass| {
        let u = sim.unit(h).unwrap();
        let dt = sim.hunt_weapon_dt(u, wc, crate::TICK_DT);
        let mut w = WeaponState::new(wc, false, 6);
        let mut lcg = crate::lcg::GameLcg::new(1);
        let (mut out, mut n) = (Vec::new(), 0);
        let input = FireInput {
            held: true,
            blocked: false,
            infinite_ammo: true,
            no_reload: true,
            ai_dispersion: None,
        };
        for _ in 0..90 {
            out.clear();
            w.update(dt, wc, input, &mut lcg, &mut out);
            n += out.len();
        }
        n
    };
    let (eased_cone, plain_cone) = (shots(first, &cone), shots(enemy, &cone));
    assert!(plain_cone >= 4, "the cone fires: {plain_cone}");
    assert!(
        eased_cone * 3 <= plain_cone * 2 && eased_cone * 3 >= plain_cone,
        "about half as often at 50 %: {eased_cone} of {plain_cone}"
    );
    // A weapon that is aimed keeps its rate: its ease is in the aim.
    assert_eq!(shots(first, &rifle), shots(enemy, &rifle));
}

#[test]
fn the_hunt_ease_touches_no_other_mode() {
    use crate::skill::Difficulty;
    let (mut sim, first, _, _) = arena(50);
    sim.difficulty = Difficulty::Hard;
    let mut cone = sim.weapon_classes[0].clone();
    cone.emitter = Some(crate::weapon::EmitterOrdnance::from_props(
        &crate::weapon::test_props::List(&[(2_470_609_998, "10")]),
        true,
    ));
    let reaction = sim.ai_tier(0).reaction_ticks;
    // Hero Assault ignores a skill row; Conquest (no round rules) has none.
    for round in [
        Some(Deathmatch::new(50, [12, 12]).with_skill_pct([30, 30])),
        None,
    ] {
        sim.deathmatch = round;
        assert_eq!([sim.hunt_skill_pct(0), sim.hunt_skill_pct(1)], [100, 100]);
        let u = sim.unit(first).unwrap();
        assert_eq!(sim.hunt_aim_widen(u), 1.0);
        assert_eq!(sim.hunt_weapon_dt(u, &cone, crate::TICK_DT), crate::TICK_DT);
        assert_eq!(sim.ai_tier(0).reaction_ticks, reaction);
        assert_eq!(sim.bot_skill(0), 20);
    }
    // Natives (team 2) keep theirs in a Hunt too.
    sim.deathmatch = Some(Deathmatch::hunt(50, [12, 12]).with_skill_pct([30, 30]));
    assert_eq!(sim.hunt_skill_pct(2), 100);
}

#[test]
fn an_unscaled_hunt_hashes_as_before_and_a_scaled_one_differs() {
    let hash = |round: &Deathmatch| {
        let mut bytes = Vec::new();
        round.hash_into(&mut |b: &[u8]| bytes.extend_from_slice(b));
        bytes
    };
    let plain = Deathmatch::hunt(50, [12, 12]);
    assert_eq!(
        hash(&plain),
        hash(&plain.clone().with_skill_pct([100, 100]))
    );
    assert_ne!(hash(&plain), hash(&plain.clone().with_skill_pct([80, 100])));
}

#[test]
fn a_hunt_spawn_on_a_crowded_node_steps_out_onto_a_free_spot() {
    let crowd = |hunt: bool| {
        let mut sim = Sim::empty(9);
        sim.heightfield = Some(flat_ground());
        sim.battle = crate::battle::Battle::new([999_999; 2]);
        sim.battle.unlimited_reinforcements = true;
        sim.deathmatch = Some(if hunt {
            Deathmatch::hunt(50, [12, 12])
        } else {
            Deathmatch::new(50, [12, 12])
        });
        let mut bot = UnitSpec::trooper(0, Vec3::new(3.0, 0.0, 4.0));
        bot.controller = Controller::Ai;
        let spots: Vec<Vec3> = (0..9)
            .map(|_| {
                let h = sim.spawn(&bot).unwrap();
                sim.unit(h).unwrap().soldier.pos
            })
            .collect();
        spots
    };
    let hunt = crowd(true);
    for (i, a) in hunt.iter().enumerate() {
        for b in &hunt[i + 1..] {
            let d = Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
            assert!(d >= 1.2 - 1e-4, "{a} and {b} stand {d} m apart");
        }
        let from = Vec3::new(a.x - 3.0, 0.0, a.z - 4.0).length();
        assert!(from <= 4.5 + 1e-4, "{a} is {from} m from its node");
    }
    // Hero Assault (like every other mode) keeps the node.
    assert!(crowd(false).iter().all(|p| (p.x, p.z) == (3.0, 4.0)));
}

#[test]
fn a_hunt_spawn_beside_an_enemy_takes_the_next_clear_node_of_its_post() {
    let land = |hunt: bool, enemy_x: f32| {
        let mut sim = Sim::empty(9);
        sim.heightfield = Some(flat_ground());
        sim.battle = crate::battle::Battle::new([999_999; 2]);
        sim.battle.unlimited_reinforcements = true;
        sim.deathmatch = Some(if hunt {
            Deathmatch::hunt(50, [12, 12])
        } else {
            Deathmatch::new(50, [12, 12])
        });
        // One shared path, as BF2 Endor's Hunt post lays it out.
        let mut def: CpDef = sphere_cp("hunt", Vec3::ZERO, 5.0, false);
        def.spawn_nodes = [0.0, 10.0, 40.0, 80.0]
            .map(|x| (Vec3::new(x, 0.0, 0.0), 0.0))
            .to_vec();
        sim.add_cp(def, Some(1));
        let mut enemy = UnitSpec::trooper(0, Vec3::new(enemy_x, 0.0, 0.0));
        enemy.controller = Controller::Ai;
        sim.spawn(&enemy).unwrap();
        let mut bot = UnitSpec::trooper(1, Vec3::ZERO);
        bot.controller = Controller::Ai;
        let h = sim.spawn(&bot).unwrap();
        sim.unit(h).unwrap().soldier.pos.x
    };
    // An enemy on the node: the next node 20 m or more from it (40 m).
    assert_eq!(land(true, 0.0), 40.0);
    // No enemy within 20 m: the node stands.
    assert_eq!(land(true, -30.0), 0.0);
    // Every other mode keeps the node.
    assert_eq!(land(false, 0.0), 0.0);
}
