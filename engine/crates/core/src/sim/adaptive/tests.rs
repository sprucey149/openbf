use super::*;
use crate::sim::deathmatch::Deathmatch;
use crate::sim::tests::flat_ground;
use crate::skill::Difficulty;
use crate::unit::{Controller, UnitHandle, UnitSpec};
use glam::Vec3;

const EVERY: u64 = UPDATE_SECS * TICK_HZ as u64;

/// A ticket round with one AI soldier on each side, far apart.
fn tickets() -> (Sim, UnitHandle, UnitHandle) {
    let mut sim = Sim::empty(11);
    sim.heightfield = Some(flat_ground());
    sim.battle = crate::battle::Battle::new([200, 200]);
    let mut bot = UnitSpec::trooper(0, Vec3::new(-300.0, 0.0, 0.0));
    bot.controller = Controller::Ai;
    let ally = sim.spawn(&bot).unwrap();
    bot.team = 1;
    bot.pos.x = 300.0;
    let enemy = sim.spawn(&bot).unwrap();
    (sim, ally, enemy)
}

/// Run the rule's update `n` times, holding team 1's tickets at `t1`.
fn updates(sim: &mut Sim, n: usize, t1: f32) -> Vec<i32> {
    let mut out = Vec::new();
    for _ in 0..n {
        sim.battle.teams[1].reinforcements = t1;
        sim.tick = (sim.tick / EVERY + 1) * EVERY;
        sim.tick_adaptive();
        out.push(sim.adaptive.factor_pm());
    }
    out
}

#[test]
fn the_target_has_a_dead_band_and_saturates_at_ten_percent() {
    assert_eq!(target_for(0), 0);
    assert_eq!(target_for(DEAD_BAND_PM), 0, "45 / 55 is still even");
    assert_eq!(target_for(-DEAD_BAND_PM), 0);
    assert_eq!(target_for(300), 50, "halfway up the ramp");
    assert_eq!(target_for(-300), -50);
    assert_eq!(target_for(FULL_AT_PM), MAX_DRIFT_PM);
    assert_eq!(target_for(1000), MAX_DRIFT_PM, "never past 10 %");
    assert_eq!(target_for(-1000), -MAX_DRIFT_PM);
}

#[test]
fn smoothing_closes_a_third_of_the_gap_per_update() {
    assert_eq!(smooth(0, 100), 34);
    assert_eq!(smooth(34, 100), 56);
    assert_eq!(smooth(100, 0), 66);
    assert_eq!(smooth(99, 100), 100, "it always arrives");
    assert_eq!(smooth(-100, -100), -100);
    assert_eq!(smooth(0, 1000), 100, "clamped");
}

#[test]
fn a_bots_only_round_never_drifts_or_hashes() {
    let (mut sim, ally, _) = tickets();
    sim.tick_adaptive();
    let seen = updates(&mut sim, 6, 20.0);
    assert!(seen.iter().all(|&f| f == 0), "{seen:?}");
    assert_eq!(sim.adaptive_factor(), None);
    let u = sim.unit(ally).unwrap();
    assert_eq!(sim.aim_widen(u), 1.0);
    // The rule's state (the starting tickets it recorded) never enters
    // the hash while the factor is 0: a sim without it hashes the same.
    let mut fresh = sim.clone();
    fresh.adaptive = Adaptive::default();
    assert_ne!(fresh.adaptive, sim.adaptive);
    assert_eq!(sim.hash(), fresh.hash());
}

#[test]
fn humans_on_both_sides_keep_the_factor_off() {
    let (mut sim, _, _) = tickets();
    sim.extra_humans = [1, 1];
    sim.tick_adaptive();
    let seen = updates(&mut sim, 6, 20.0);
    assert!(seen.iter().all(|&f| f == 0), "{seen:?}");
    assert_eq!(sim.adaptive_factor(), None);
}

#[test]
fn humans_ahead_on_tickets_make_only_the_enemy_bots_stronger() {
    let (mut sim, ally, enemy) = tickets();
    sim.extra_humans = [1, 0];
    // The starting tickets, 200 / 200; then even: the dead band.
    sim.tick_adaptive();
    assert_eq!(updates(&mut sim, 2, 200.0), [0, 0]);
    assert_eq!(sim.adaptive_factor(), Some(1.0));
    // 200 / 190: the humans' lead is 50 per-mille, inside the band.
    assert_eq!(updates(&mut sim, 2, 190.0), [0, 0]);
    // 200 / 60: the lead is 700, past the full drift.
    let seen = updates(&mut sim, 12, 60.0);
    assert_eq!(&seen[..3], &[34, 56, 71]);
    assert!(seen.windows(2).all(|w| w[1] >= w[0]), "{seen:?}");
    assert_eq!(*seen.last().unwrap(), MAX_DRIFT_PM);
    assert_eq!(sim.adaptive.bot_team(), Some(1));
    let f = sim.adaptive_factor().unwrap();
    assert!((f - 1.1).abs() < 1e-6, "{f}");
    // The enemy bot aims tighter and reacts sooner; the humans' own bot
    // keeps its aim and reaction.
    let pm = sim.adaptive.factor_pm();
    let widen = |sim: &Sim, h| sim.aim_widen(sim.unit(h).unwrap());
    assert_eq!(widen(&sim, ally), 1.0);
    assert!((widen(&sim, enemy) - (1000 - pm) as f32 / 1000.0).abs() < 1e-6);
    let base = Difficulty::Elite.tier().reaction_ticks;
    sim.difficulty = Difficulty::Elite;
    assert_eq!(sim.ai_tier(0).reaction_ticks, base);
    assert_eq!(
        sim.ai_tier(1).reaction_ticks,
        4,
        "Elite's 5 ticks, 10 % quicker"
    );
    // A stand-in on the bot team is never drifted.
    sim.units[enemy.index as usize].proxy_human = Some(crate::ai::proxy::Strength::Good.profile());
    assert_eq!(widen(&sim, enemy), 1.0);
}

#[test]
fn humans_behind_make_the_bots_weaker_and_the_factor_comes_back() {
    let (mut sim, _, enemy) = tickets();
    sim.extra_humans = [0, 2]; // humans on team 1: team 0 is the bot team
    sim.tick_adaptive();
    // Team 1 (the humans) at 40 of 200 while the bots keep 200.
    let seen = updates(&mut sim, 12, 40.0);
    assert_eq!(*seen.last().unwrap(), -MAX_DRIFT_PM, "{seen:?}");
    assert_eq!(sim.adaptive.bot_team(), Some(0));
    sim.difficulty = Difficulty::Elite;
    assert_eq!(
        sim.ai_tier(0).reaction_ticks,
        6,
        "Elite's 5 ticks, 10 % slower"
    );
    assert_eq!(sim.ai_tier(1).reaction_ticks, 5);
    let u = sim.unit(enemy).unwrap();
    assert_eq!(sim.aim_widen(u), 1.0, "a human-side bot");
    // Back to even: the drift decays toward 0.
    let back = updates(&mut sim, 20, 200.0);
    assert!(back.windows(2).all(|w| w[1] >= w[0]), "{back:?}");
    assert_eq!(*back.last().unwrap(), 0, "{back:?}");
    // The humans leaving the team: off at the next update.
    sim.extra_humans = [0, 0];
    updates(&mut sim, 1, 40.0);
    assert_eq!(sim.adaptive_factor(), None);
    assert_eq!(sim.adaptive.factor_pm(), 0);
}

#[test]
fn point_modes_read_points_against_each_sides_target() {
    let (mut sim, _, _) = tickets();
    sim.extra_humans = [1, 0];
    let mut round = Deathmatch::new(100, [12, 12]);
    round.points = [40, 10];
    sim.deathmatch = Some(round);
    sim.tick_adaptive();
    // Lead 300 per-mille: target 50.
    updates(&mut sim, 1, 200.0);
    assert_eq!(sim.adaptive.target_pm(), 50);
    // Even points: the band.
    sim.deathmatch.as_mut().unwrap().points = [12, 10];
    updates(&mut sim, 1, 200.0);
    assert_eq!(sim.adaptive.target_pm(), 0);
}

#[test]
fn unlimited_tickets_read_as_even() {
    let (mut sim, _, _) = tickets();
    sim.extra_humans = [1, 0];
    sim.battle.unlimited_teams = [true, false];
    sim.tick_adaptive();
    updates(&mut sim, 6, 10.0);
    assert_eq!(sim.adaptive.factor_pm(), 0);
    assert_eq!(sim.adaptive_factor(), Some(1.0));
}

#[test]
fn the_factor_only_updates_on_its_ten_second_ticks_and_is_hashed_when_on() {
    let (mut sim, _, _) = tickets();
    sim.extra_humans = [1, 0];
    sim.tick_adaptive();
    sim.battle.teams[1].reinforcements = 60.0;
    sim.tick = EVERY - 1;
    sim.tick_adaptive();
    assert_eq!(sim.adaptive.factor_pm(), 0, "not yet");
    sim.tick = EVERY;
    sim.tick_adaptive();
    assert_eq!(sim.adaptive.factor_pm(), 34);
    // A drifted factor is sim state: it is hashed.
    let mut fresh = sim.clone();
    fresh.adaptive = Adaptive::default();
    assert_ne!(sim.hash(), fresh.hash());
    // Turned off, the next update drops it.
    let mut off = sim.clone();
    off.adaptive.enabled = false;
    off.tick = 2 * EVERY;
    off.tick_adaptive();
    assert_eq!(off.adaptive.factor_pm(), 0);
    assert_eq!(off.adaptive_factor(), None, "turned off");
}
