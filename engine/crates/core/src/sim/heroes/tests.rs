use super::*;
use crate::unit::{Controller, UnitClass, UnitSpec};
use glam::Vec3;

fn class(health: f32) -> UnitClass {
    let s = UnitSpec::trooper(0, Vec3::ZERO);
    UnitClass {
        consts: s.consts,
        max_health: health,
        health_type: s.health_type,
        weapon_class: 0,
        total_clips: 0,
        weapon_slot: 0,
        slots: s.slots,
        jet: false,
        capture_posts: true,
        drops: Default::default(),
        chunk: None,
        droideka: None,
        min: 1,
        max: None,
        unlock_points: 0,
    }
}

fn scene() -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(42);
    sim.classes = [
        vec![class(300.0), class(1200.0)],
        vec![class(300.0), class(1500.0)],
    ];
    for (team, x) in [(0, 0.0), (1, 100.0)] {
        sim.add_cp(
            crate::sim::tests::sphere_cp("post", Vec3::new(x, 0.0, 0.0), 8.0, true),
            Some(team),
        );
    }
    let player = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    sim.heroes = Some(Heroes::new([Some(1), Some(1)]));
    (sim, player)
}

fn earn(sim: &mut Sim, member: UnitHandle, points: i32) {
    let scores = &mut sim.heroes.as_mut().unwrap().points;
    scores.resize(sim.units.len(), 0);
    scores[member.index as usize] = points;
}

#[test]
fn hero_unlocks_at_ten_points_and_reserves_only_the_winner() {
    let (mut sim, p) = scene();
    earn(&mut sim, p, 9);
    sim.step(&[PadState::default()]);
    assert_eq!(sim.hero_offer(0), None);
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    assert_eq!(sim.hero_offer(0).unwrap().0, 1);
    assert_eq!(
        sim.class_refusal(0, 0, 1),
        Some(super::super::ClassRefusal::HeroUnavailable)
    );
    assert_eq!(sim.ai_class_weights(0)[1], 0.0);
    assert!(sim.ai_class_weights(0)[0] > 0.0);
}

#[test]
fn hero_accept_replaces_without_a_death_ticket_drop_then_health_drains() {
    let (mut sim, p) = scene();
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    let tickets = sim.battle.teams[0].tickets();
    sim.step(&[PadState {
        buttons: buttons::HERO_ACCEPT | buttons::SQUAD_AT_EASE,
        ..Default::default()
    }]);
    let h = sim.players[0].unwrap();
    assert!(sim.is_hero(h));
    let hero = sim.unit(h).unwrap();
    assert_eq!(
        (hero.health.cur, hero.health.max, hero.deaths),
        (1200.0, 1200.0, 0)
    );
    assert_eq!(sim.battle.teams[0].tickets(), tickets);
    assert_eq!(sim.battle.teams[0].alive_players, 1);
    assert_eq!(sim.class_refusal(0, 0, 1), None);
    assert!(!sim
        .events
        .iter()
        .any(|e| matches!(e, Event::Kill { .. } | Event::EnvDeath { .. })));
    assert!(!sim.units[p.index as usize].drop_due);
    for _ in 0..300 {
        sim.step(&[PadState::default()]);
    }
    assert!((sim.unit(h).unwrap().health.cur - 1150.0).abs() < 0.02);
}

#[test]
fn hero_human_candidates_outrank_bots_and_decline_goes_to_next_human() {
    let (mut sim, first) = scene();
    let second = sim
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::X * 3.0))
        .unwrap();
    let bot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::X * 5.0)
        })
        .unwrap();
    earn(&mut sim, first, 11);
    earn(&mut sim, second, 10);
    earn(&mut sim, bot, 1000);
    sim.step(&[PadState::default(); 2]);
    assert!(sim.hero_offer(0).is_some());
    sim.step(&[
        PadState {
            buttons: buttons::HERO_DECLINE,
            ..Default::default()
        },
        PadState::default(),
    ]);
    assert!(sim.hero_offer(0).is_none());
    assert!(sim.hero_offer(1).is_some());
}

#[test]
fn hero_offer_expires_and_death_waits_for_the_respawn_rule() {
    let (mut sim, p) = scene();
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    let expires = sim.heroes.as_ref().unwrap().teams[0].offer.unwrap().expires;
    sim.tick = expires + 1;
    sim.step(&[PadState::default()]);
    assert!(sim.hero_offer(0).is_none());
    assert!(sim.heroes.as_ref().unwrap().teams[0].ready_at > sim.tick);
    sim.tick = sim.heroes.as_ref().unwrap().teams[0].ready_at + 1;
    sim.step(&[PadState::default()]);
    sim.step(&[PadState {
        buttons: buttons::HERO_ACCEPT,
        ..Default::default()
    }]);
    let hero = sim.players[0].unwrap();
    assert!(sim.is_hero(hero));
    sim.kill_environment(hero, super::super::DeathCause::Suicide);
    sim.step(&[PadState::default()]);
    let state = &sim.heroes.as_ref().unwrap().teams[0];
    assert!(state.owner.is_none());
    assert_eq!(state.ready_at, sim.tick - 1 + 90 * u64::from(TICK_HZ));
    assert_eq!(sim.spawn_choice[0], 0);
}

#[test]
fn hero_offer_accepts_at_the_exact_thirty_second_boundary() {
    for offset in [-1_i64, 0, 1] {
        let (mut sim, p) = scene();
        earn(&mut sim, p, 10);
        sim.step(&[PadState::default()]);
        let expires = sim.heroes.as_ref().unwrap().teams[0].offer.unwrap().expires;
        sim.tick = expires.checked_add_signed(offset).unwrap();
        sim.step(&[PadState {
            buttons: buttons::HERO_ACCEPT,
            ..Default::default()
        }]);
        assert_eq!(
            sim.is_hero(sim.players[0].unwrap()),
            offset <= 0,
            "offset {offset}"
        );
    }
}

#[test]
fn hero_ai_requires_a_mode_that_explicitly_admits_ai_candidates() {
    let (mut sim, _) = scene();
    let bot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(1, Vec3::X * 100.0)
        })
        .unwrap();
    earn(&mut sim, bot, 20);
    sim.step(&[PadState::default()]);
    assert!(sim.heroes.as_ref().unwrap().teams[1].owner.is_none());
    let heroes = sim.heroes.as_mut().unwrap();
    heroes.rules.ai_candidates = true;
    heroes.teams[1].ready_at = 0;
    sim.step(&[PadState::default()]);
    let bot = sim.handle(bot.index as usize);
    assert!(sim.is_hero(bot));
    assert_eq!(sim.unit(bot).unwrap().skill, sim.bot_skill(1));
    assert_eq!(sim.battle.teams[1].alive_ai, 1);
}

#[test]
fn hero_live_state_changes_the_hash_and_absence_keeps_legacy_state() {
    let (mut sim, p) = scene();
    let baseline = sim.hash();
    sim.heroes.as_mut().unwrap().teams[0].ready_at = 9;
    assert_ne!(sim.hash(), baseline);
    sim.heroes.as_mut().unwrap().teams[0].ready_at = 0;
    assert_eq!(sim.hash(), baseline);
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    let offer = sim.hash();
    sim.heroes.as_mut().unwrap().teams[0]
        .offer
        .as_mut()
        .unwrap()
        .expires += 1;
    assert_ne!(sim.hash(), offer);
}

/// Playtest A4: the button that answers the offer also carries a squad
/// order and a spawn-map arm. Held past the answer it issues nothing; the
/// next press after its release orders as usual.
#[test]
fn hero_prompt_press_held_past_the_offer_issues_no_order() {
    let commands = |sim: &Sim| {
        sim.events
            .iter()
            .filter(|e| matches!(e, Event::Command { .. }))
            .count()
    };
    let hold = |bits| PadState {
        buttons: bits,
        ..Default::default()
    };
    // The pad's Right: decline, FOLLOW and the spawn map's right.
    let (mut sim, p) = scene();
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    assert!(sim.hero_offer(0).is_some());
    let right = buttons::HERO_DECLINE | buttons::SQUAD_FOLLOW | buttons::SPAWN_POST_RIGHT;
    let mut issued = 0;
    for _ in 0..10 {
        sim.step(&[hold(right)]);
        issued += commands(&sim);
    }
    assert!(sim.hero_offer(0).is_none(), "the press declined");
    assert_eq!(issued, 0, "the decline press is not also an order");
    sim.step(&[PadState::default()]);
    assert!(sim.held_eaten.iter().all(|h| h.is_clear()));
    sim.step(&[hold(buttons::SQUAD_FOLLOW)]);
    assert_eq!(commands(&sim), 1, "a fresh press still orders");

    // F1: accept and FOLLOW. The hero comes in at once headless.
    let (mut sim, p) = scene();
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    let f1 = buttons::HERO_ACCEPT | buttons::SQUAD_FOLLOW;
    let mut issued = 0;
    for _ in 0..10 {
        sim.step(&[hold(f1)]);
        issued += commands(&sim);
    }
    assert!(sim.is_hero(sim.players[0].unwrap()));
    assert_eq!(issued, 0, "the accept press is not also an order");
    assert_eq!(sim.spawn_post_choice[0], None);
}

/// The held mask is hashed only while it holds something.
#[test]
fn hero_held_prompt_press_is_hashed_only_while_held() {
    let (mut sim, _) = scene();
    sim.sync_slot_tables();
    let baseline = sim.hash();
    sim.held_eaten[0].eat(buttons::SQUAD_FOLLOW);
    assert_ne!(sim.hash(), baseline);
    sim.held_eaten[0] = Default::default();
    assert_eq!(sim.hash(), baseline);
}

#[test]
fn hero_kill_heal_uses_the_pre_kill_streak_and_caps_at_disc_health() {
    let (mut sim, p) = scene();
    earn(&mut sim, p, 10);
    sim.step(&[PadState::default()]);
    sim.step(&[PadState {
        buttons: buttons::HERO_ACCEPT,
        ..Default::default()
    }]);
    let hero = sim.players[0].unwrap();
    for (before, expected) in [
        (0, 150.0),
        (14, 150.0),
        (15, 100.0),
        (29, 100.0),
        (30, 50.0),
    ] {
        sim.heroes.as_mut().unwrap().teams[0].kill_streak = before;
        sim.unit_mut(hero).unwrap().health.cur = 100.0;
        let victim = sim.spawn(&UnitSpec::trooper(1, Vec3::X * 20.0)).unwrap();
        sim.unit_mut(victim).unwrap().health.cur = 0.0;
        sim.unit_killed(victim, Some(hero));
        assert_eq!(sim.unit(hero).unwrap().health.cur, 100.0 + expected);
        assert_eq!(
            sim.heroes.as_ref().unwrap().teams[0].kill_streak,
            before + 1
        );
    }
    sim.unit_mut(hero).unwrap().health.cur = 1190.0;
    sim.heal_hero_kill(hero);
    assert_eq!(sim.unit(hero).unwrap().health.cur, 1200.0);
    let count = sim.heroes.as_ref().unwrap().teams[0].kill_streak;
    let friend = sim.spawn(&UnitSpec::trooper(0, Vec3::X * 20.0)).unwrap();
    sim.unit_mut(hero).unwrap().health.cur = 100.0;
    sim.unit_mut(friend).unwrap().health.cur = 0.0;
    sim.unit_killed(friend, Some(hero));
    assert_eq!(sim.unit(hero).unwrap().health.cur, 100.0);
    assert_eq!(sim.heroes.as_ref().unwrap().teams[0].kill_streak, count);
}

#[test]
fn hero_healing_reads_the_validated_rule_data() {
    let (mut sim, player) = scene();
    let rules = HeroRules {
        health: HeroHealthRules {
            streak_bounds: [2, 4],
            heal_gains: [71.0, 39.0, 13.0],
            time_drain_per_second: 0.0,
        },
        ..HeroRules::default()
    };
    sim.heroes = Some(Heroes::with_rules([Some(1), Some(1)], rules).unwrap());
    earn(&mut sim, player, 10);
    sim.step(&[PadState::default()]);
    sim.step(&[PadState {
        buttons: buttons::HERO_ACCEPT,
        ..Default::default()
    }]);
    let hero = sim.players[0].unwrap();
    for (before, expected) in [(1, 71.0), (2, 39.0), (3, 39.0), (4, 13.0)] {
        sim.heroes.as_mut().unwrap().teams[0].kill_streak = before;
        sim.unit_mut(hero).unwrap().health.cur = 100.0;
        sim.heal_hero_kill(hero);
        assert_eq!(sim.unit(hero).unwrap().health.cur, 100.0 + expected);
    }
    let mut invalid = rules;
    invalid.health.time_drain_per_second = -1.0;
    assert_eq!(
        Heroes::with_rules([Some(1), None], invalid).unwrap_err(),
        "hero time drain must be finite and nonnegative"
    );
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        invalid = rules;
        invalid.health.heal_gains[0] = value;
        assert!(invalid.validate().is_err());
    }
    invalid = rules;
    invalid.health.streak_bounds = [4, 4];
    assert!(invalid.validate().is_err());
}

#[test]
fn hero_scores_and_health_rules_are_full_future_state_in_the_hash() {
    fn bytes(heroes: &Heroes) -> Vec<u8> {
        let mut out = Vec::new();
        heroes.hash_into(&mut |b| out.extend_from_slice(b));
        out
    }
    let mut base = Heroes::new([Some(1), None]);
    base.points = vec![11, 22];
    for index in 0..base.points.len() {
        let mut changed = base.clone();
        changed.points[index] += 7;
        assert_ne!(
            bytes(&base),
            bytes(&changed),
            "nonzero score{index} must be hashed"
        );
    }
    let mut changed = base.clone();
    changed.points.push(33);
    assert_ne!(bytes(&base), bytes(&changed));
    for index in 0..3 {
        let mut changed = base.clone();
        changed.rules.health.heal_gains[index] += 1.0;
        assert_ne!(bytes(&base), bytes(&changed), "heal gain{index}");
    }
    for index in 0..2 {
        let mut changed = base.clone();
        changed.rules.health.streak_bounds[index] += 1;
        assert_ne!(bytes(&base), bytes(&changed), "streak bound{index}");
    }
    let mut changed = base.clone();
    changed.rules.health.time_drain_per_second = 1.0;
    assert_ne!(bytes(&base), bytes(&changed));
}

#[test]
fn hero_respawn_waits_past_the_exact_ninety_second_boundary() {
    for offset in [-1_i64, 0, 1] {
        let (mut sim, p) = scene();
        earn(&mut sim, p, 10);
        sim.step(&[PadState::default()]);
        sim.step(&[PadState {
            buttons: buttons::HERO_DECLINE,
            ..Default::default()
        }]);
        let ready = sim.heroes.as_ref().unwrap().teams[0].ready_at;
        assert_eq!(ready, sim.tick - 1 + 90 * u64::from(TICK_HZ));
        sim.tick = ready.checked_add_signed(offset).unwrap();
        sim.step(&[PadState::default()]);
        assert_eq!(sim.hero_offer(0).is_some(), offset > 0, "offset {offset}");
    }
}

mod drain;
mod spawn;

/// The shell's hero rows reach the manager: Unlock Hero Timer (rule 5,
/// strictly after the value), Assign To Worst / Random (rules 4 / 7), and
/// the respawn Timer's Always (0 s) and Never (rule 1).
#[test]
fn hero_rules_unlock_by_timer_and_assign_worst_and_random() {
    let (mut sim, p) = scene();
    sim.heroes.as_mut().unwrap().rules.unlock = HeroUnlock::Timer(5);
    earn(&mut sim, p, 0);
    while sim.tick < 5 * u64::from(TICK_HZ) {
        sim.step(&[PadState::default()]);
        assert!(sim.hero_offer(0).is_none(), "tick {}", sim.tick);
    }
    sim.step(&[PadState::default()]);
    assert!(sim.hero_offer(0).is_none(), "strictly after 5 s");
    sim.step(&[PadState::default()]);
    assert!(sim.hero_offer(0).is_some());

    for (assign, want) in [(HeroAssign::Best, 0usize), (HeroAssign::Worst, 1)] {
        let (mut sim, first) = scene();
        let second = sim
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::X * 3.0))
            .unwrap();
        sim.heroes.as_mut().unwrap().rules.assign = assign;
        earn(&mut sim, first, 12);
        earn(&mut sim, second, 3);
        sim.step(&[PadState::default(); 2]);
        assert!(sim.hero_offer(want).is_some(), "{assign:?}");
        assert!(sim.hero_offer(1 - want).is_none(), "{assign:?}");
    }
    // Random: every eligible human ties, the LCG picks; both get picked
    // over a few seeds.
    let mut picked = [false; 2];
    for seed in 0..16 {
        let (mut sim, first) = scene();
        sim.lcg = crate::lcg::GameLcg::new(seed * 7919 + 1);
        let second = sim
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::X * 3.0))
            .unwrap();
        sim.heroes.as_mut().unwrap().rules.assign = HeroAssign::Random;
        earn(&mut sim, first, 12);
        earn(&mut sim, second, 3);
        sim.step(&[PadState::default(); 2]);
        for (k, slot) in picked.iter_mut().enumerate() {
            *slot |= sim.hero_offer(k).is_some();
        }
    }
    assert_eq!(picked, [true, true]);
}

#[test]
fn hero_respawn_always_reoffers_next_tick_and_never_does_not() {
    for respawn in [Some(0), None] {
        let (mut sim, p) = scene();
        sim.heroes.as_mut().unwrap().rules.respawn_ticks = respawn;
        earn(&mut sim, p, 10);
        sim.step(&[PadState::default()]);
        sim.step(&[PadState {
            buttons: buttons::HERO_ACCEPT,
            ..Default::default()
        }]);
        let hero = sim.players[0].unwrap();
        assert!(sim.is_hero(hero));
        sim.step(&[PadState::default()]);
        sim.kill_environment(hero, super::super::DeathCause::Suicide);
        sim.step(&[PadState::default()]);
        assert!(sim.heroes.as_ref().unwrap().teams[0].owner.is_none());
        for _ in 0..30 * 100 {
            sim.step(&[PadState::default()]);
            if sim.hero_offer(0).is_some() {
                break;
            }
        }
        // Always: offered as soon as he is alive again; Never: no more.
        assert_eq!(
            sim.hero_offer(0).is_some(),
            respawn.is_some(),
            "{respawn:?}"
        );
    }
}
