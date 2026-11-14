//! Queue item 17B — the POST-OUTCOME TAIL.
//!
//! `Sim::update_cps` only LATCHES the winner (`core/src/sim/round.rs` — `if
//! self.outcome.is_none()`); nothing stops the tick, so bleed, deaths and
//! respawns keep running for as long as the round is stepped. Item 17's
//! report left the question open: can that tail still reach a NEGATIVE
//! reinforcement pool, or do `Battle::on_death`'s floor (`battle.rs:124`)
//! and `cp::apply_bleed`'s gate (`cp.rs:288-297`, a rate only while
//! `alive < tickets && tickets > 0`) already prevent it? The pool is the raw
//! `f32` field `Sim::hash` eats (`core/src/sim/state/hash.rs`), so a negative value would be
//! a hashed state neither disc can hold — that is why it matters.
//!
//! The rule these tests hold the port to is `analysis/re-tickets-spawn.md`
//! §0 and §4, both [V]: DEATH consumes the reinforcement, with a floor at 0
//! in BOTH engines (BF1 `FUN_00297440` does `if (tickets>0) tickets--`; BF2
//! `Objective.lua` `OnCharacterDeath` guards `GetReinforcementCount(t) > 0`).
//!
//! These are MEASUREMENTS, not a fix: per the queue item, a failure here is
//! reported (first tick + path) for the lead to rule on, never patched.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::battle::{Battle, SpawnOutcome, BF2_BLEED_TICKETS_PER_SEC};
use core::cp::CpDef;
use core::region::Region;
use core::rng::SimRng;
use core::sim::{scale_reinforcements, DeathCause, Event, PadState, Sim, OUTCOME_DRAW};
use core::unit::UnitSpec;
use glam::Vec3;

const DT: f32 = 1.0 / 30.0;
/// Ticks to keep running AFTER the outcome latches (the queue item's number).
const TAIL: u32 = 1000;

fn assert_pools_non_negative(b: &Battle, tick: u32, path: &str) {
    for (t, team) in b.teams.iter().enumerate() {
        assert!(
            team.reinforcements >= 0.0,
            "tick {tick}: team {t} pool went negative ({}) after {path}",
            team.reinforcements
        );
    }
}

/// The queue item's literal ask, at the `Battle` level: drive a battle past
/// its outcome and keep bleed + deaths + spawn attempts running for 1,000
/// ticks, asserting both pools stay >= 0 the whole way.
///
/// The shape is the one the live sim produces (see the sibling test): the
/// loser's pool bleeds to 0 while its four players stay alive — `tick_bleed`'s
/// shed loop (`battle.rs:174`) only sheds AI and `invariant_holds` exempts
/// live players — and the winner keeps a healthy pool, so its deaths and
/// respawns carry the tail. The loser's players then die ONTO THE FLOOR, which
/// is `cleanse-core-nav`'s B-CN1 state (item 17's report §What landed).
///
/// Harsher than the live path in one deliberate way: `cp::apply_bleed` would
/// switch the loser's bleed OFF once `tickets() == 0`, and this keeps it on.
#[test]
fn battle_pools_stay_non_negative_1000_ticks_past_the_outcome() {
    // Built through the public spawn path so the invariant is honest rather
    // than hand-set: team 0 = 4 players + 2 AI on a 6-ticket pool (the P9
    // four-pad maximum), team 1 = 12 AI on 400.
    let mut b = Battle::new([6, 400]);
    for _ in 0..4 {
        assert!(matches!(b.try_spawn(0, true), SpawnOutcome::Spawned { .. }));
    }
    for _ in 0..2 {
        assert!(matches!(
            b.try_spawn(0, false),
            SpawnOutcome::Spawned { .. }
        ));
    }
    assert_eq!(
        b.try_spawn(0, false),
        SpawnOutcome::Refused,
        "6 alive, 6 tickets"
    );
    for _ in 0..12 {
        assert!(matches!(
            b.try_spawn(1, false),
            SpawnOutcome::Spawned { .. }
        ));
    }
    // Team 0 holds no post: the enemy's strict CP-value majority bleeds it.
    b.teams[0].bleed_per_sec = BF2_BLEED_TICKETS_PER_SEC;

    let mut outcome: Option<(u32, usize)> = None;
    let mut deaths_in_tail = 0u32;
    let mut floored_deaths = 0u32;
    let mut losing_players_left = 4u32;

    for tick in 0..4000u32 {
        b.tick_bleed(DT);
        assert_pools_non_negative(&b, tick, "tick_bleed");

        // The winner fights on the whole time: a death and a spawn every
        // fifth tick, exactly as `Sim::step`'s death and respawn passes do.
        if tick % 5 == 0 && b.teams[1].alive_ai > 0 {
            b.on_death(1, false);
            assert_pools_non_negative(&b, tick, "on_death (winner)");
            if outcome.is_some() {
                deaths_in_tail += 1;
            }
        }
        if tick % 5 == 2 {
            b.try_spawn(1, false);
            assert_pools_non_negative(&b, tick, "try_spawn");
        }

        // BF2 `Objective.lua` OnTicketCountChange: count <= 0 → defeat. The
        // sim latches it and keeps stepping (`Sim::update_cps`, `sim/round.rs`).
        if outcome.is_none() {
            if let Some(t) = (0..2).find(|&t| b.teams[t].tickets() == 0) {
                outcome = Some((tick, t));
            }
        }

        if let Some((o, _)) = outcome {
            // In the tail the loser's four players die one at a time, each
            // onto a pool that is already at the floor.
            if losing_players_left > 0 && tick > o && (tick - o) % 200 == 0 {
                let before = b.teams[0].reinforcements;
                b.on_death(0, true);
                assert_pools_non_negative(&b, tick, "on_death (loser, empty pool)");
                losing_players_left -= 1;
                deaths_in_tail += 1;
                // `before - 1.0` would be negative: this death is the floor's
                // whole reason to exist.
                if before < 1.0 {
                    floored_deaths += 1;
                }
            }
            if tick - o >= TAIL {
                break;
            }
        }
    }

    let (outcome_tick, loser) = outcome.expect("the pool never reached 0 — no tail was entered");
    assert_eq!(loser, 0, "team 0 is the bleeding side");
    eprintln!(
        "17B battle: outcome at tick {outcome_tick}, {deaths_in_tail} deaths in the \
         {TAIL}-tick tail, {floored_deaths} of them onto a sub-1.0 pool"
    );
    assert_eq!(losing_players_left, 0, "all four players died in the tail");
    assert_eq!(floored_deaths, 4, "every player death landed on the floor");
    assert!(
        deaths_in_tail >= 100,
        "the tail must be a real tail: only {deaths_in_tail} deaths after tick {outcome_tick}"
    );
    // The pool the hash eats is exactly +0.0, not -0.0 and not a negative
    // epsilon (`Sim::hash` takes `to_bits`, so the sign bit is load-bearing).
    assert_eq!(b.teams[0].reinforcements.to_bits(), 0u32);
    assert!(b.teams[1].reinforcements > 0.0);
}

/// The same question against the LIVE sim, which is what item 17's report
/// could not answer. The outcome here is DOMINATION (`cp.rs:306`, 20 s
/// holding every post), not a ticket wipe, so the round is decided while both
/// teams still have pools and units — the only shape that gives a 1,000-tick
/// tail with deaths landing on both sides.
///
/// The measurement it pins, beyond "no pool goes negative": the live pool
/// never even REACHES 0 while anyone is alive. `cp::apply_bleed` (`cp.rs:293`)
/// arms the rate only while `alive() < tickets()`, and a death drops both by
/// one, so the gap `tickets - alive` can only shrink — bleed shuts itself off
/// at `tickets == alive` and the pool stalls in `[alive, alive + 1)`. The
/// floor in `Battle::on_death` (`battle.rs:124`) is therefore a guard on a
/// state this conquest path does not produce; the sibling test above shows it
/// IS reachable through the `Battle` API, which is what `Sim::hash` eats.
#[test]
fn live_sim_pools_stay_non_negative_1000_ticks_past_the_outcome() {
    let mut s = Sim::empty(0x17B);
    s.battle = Battle::new([24, 600]);
    s.ai_alive_cap = 32;
    // Two posts, both team 1's: team 0 holds none, so it bleeds
    // (`cp::bf2_bleed_rate`'s strict majority) and team 1 dominates.
    for (i, x) in [30.0f32, -30.0].iter().enumerate() {
        s.add_cp(
            CpDef {
                name: format!("cp{i}"),
                pos: Vec3::new(*x, 0.0, -40.0),
                capture: Region::sphere(Vec3::new(*x, 0.0, -40.0), 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(Vec3::new(*x, 0.0, -44.0), 0.0)],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(1),
        );
    }
    let player = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    for i in 0..32 {
        let x = i as f32 * 3.0;
        if i < 16 {
            s.spawn(&UnitSpec::trooper(0, Vec3::new(x, 0.0, 6.0)))
                .unwrap();
        }
        s.spawn(&UnitSpec::trooper(1, Vec3::new(x, 0.0, -30.0)))
            .unwrap();
    }

    let pads = [PadState::default()];
    let mut rng = SimRng::new(0xB00B1E5);
    let mut outcome_tick: Option<u64> = None;
    let mut kills_in_tail = 0u32;
    let mut bleeding_ticks = 0u32;
    // Per team, the largest `alive() - tickets()` seen — the precondition of
    // `tick_bleed`'s AI-shed loop (`battle.rs:174`) and of the `> 0` that
    // makes `Sim::update_cps` call `shed_ai` (`sim/round.rs`).
    let mut max_overhang = [i64::MIN; 2];
    let mut min_pool = [f32::MAX; 2];
    let mut player_death: Option<(u64, f32, f32)> = None;
    // (tick, pool) the first time team 0's bleed DISARMS with a unit still
    // alive — the gate closing, which is the mechanism under test.
    let mut stall: Option<(u64, f32)> = None;

    for _ in 0..6000u64 {
        s.step(&pads);
        for (t, m) in min_pool.iter_mut().enumerate() {
            let pool = s.battle.teams[t].reinforcements;
            *m = m.min(pool);
            assert!(
                pool >= 0.0,
                "tick {}: team {t} pool went negative ({pool}) in Sim::step",
                s.tick
            );
        }
        if s.battle.teams[0].bleed_per_sec > 0.0 {
            bleeding_ticks += 1;
        }
        for (t, m) in max_overhang.iter_mut().enumerate() {
            let team = &s.battle.teams[t];
            *m = (*m).max(i64::from(team.alive()) - i64::from(team.tickets()));
        }
        if outcome_tick.is_none() && s.outcome.is_some() {
            outcome_tick = Some(s.tick);
        }

        // Environmental deaths every third tick: `kill_environment` is the
        // same `Battle::on_death` call the damage path makes, with no aiming
        // in the way. The draw falls through to the other team so the tail
        // does not go quiet when one side runs out of bodies. The player is
        // held back until it is team 0's LAST unit and the tail has started —
        // the closest the live path gets to `cleanse-core-nav`'s B-CN1 state
        // (the loser's pool at its lowest, one player still standing).
        if stall.is_none()
            && bleeding_ticks > 0
            && s.battle.teams[0].bleed_per_sec == 0.0
            && s.alive_units().any(|(_, u)| u.team == 0)
        {
            stall = Some((s.tick, s.battle.teams[0].reinforcements));
        }

        if s.tick.is_multiple_of(3) {
            let first = (rng.next_rand() & 1) as u8;
            // 500 ticks of last stand before the player is allowed to die, so
            // the stall itself is measured, not just its endpoints.
            let last_stand = outcome_tick.is_some_and(|o| s.tick >= o + u64::from(TAIL) / 2)
                && s.alive_units().filter(|(_, u)| u.team == 0).count() == 1;
            let pick = |t: u8| {
                s.alive_units()
                    .find(|(_, u)| u.team == t && (!u.is_player || last_stand))
                    .map(|(h, u)| (h, u.team))
            };
            let victim = pick(first).or_else(|| pick(1 - first));
            if let Some((h, team)) = victim {
                let before = s.battle.teams[usize::from(team)].reinforcements;
                s.kill_environment(h, DeathCause::DeathRegion);
                let after = s.battle.teams[usize::from(team)].reinforcements;
                for t in 0..2 {
                    assert!(
                        s.battle.teams[t].reinforcements >= 0.0,
                        "tick {}: team {t} pool went negative in kill_environment",
                        s.tick
                    );
                }
                if h == player {
                    player_death = Some((s.tick, before, after));
                }
                if outcome_tick.is_some() {
                    kills_in_tail += 1;
                }
            }
        }
        if outcome_tick.is_some_and(|o| s.tick - o >= u64::from(TAIL)) {
            break;
        }
    }

    let outcome_tick = outcome_tick.expect("no outcome latched — no tail was entered");
    let alive0 = s.alive_units().filter(|(_, u)| u.team == 0).count();
    eprintln!(
        "17B live: outcome {:?} at tick {outcome_tick}, {kills_in_tail} deaths in the \
         {TAIL}-tick tail, team 0 bled on {bleeding_ticks} ticks, min pools {min_pool:?}, \
         player death {player_death:?}, stall {stall:?}, max overhang {max_overhang:?}, \
         team 0 alive at the end {alive0}",
        s.outcome
    );
    assert_eq!(s.outcome, Some(1), "team 1 dominated both posts");
    assert!(
        kills_in_tail >= 100,
        "the tail must be a real tail: only {kills_in_tail} deaths after tick {outcome_tick}"
    );

    // THE ANSWER to item 17's open question, measured: no pool went negative
    // anywhere in the tail, and team 0's never even reached the floor.
    assert!(
        min_pool[0] > 0.0,
        "the live bleed gate keeps the pool off 0 while a unit is alive; got {}",
        min_pool[0]
    );
    assert!(bleeding_ticks > 0, "team 0 really was bleeding");
    // The same gate seen from the other side: the pool never fell BELOW the
    // live count either, on either team, so `tick_bleed`'s shed loop had no
    // cause to run and `Sim::update_cps` never called `shed_ai` — measured
    // here, not proved in general.
    assert!(
        max_overhang.iter().all(|&m| m <= 0),
        "the pool fell below the live count: {max_overhang:?}"
    );

    // The last-stand player dies onto the LOWEST pool the live path can
    // present, and it is an ordinary decrement — not the floor.
    let (death_tick, before, after) = player_death.expect("the last-stand player never died");
    assert!(death_tick >= outcome_tick, "the death is inside the tail");

    // The stall is the gate, measured: once `tickets() == alive()` the bleed
    // disarms and team 0's pool does not move another bit — for 300+ ticks
    // with a unit still standing — until that unit dies.
    let (stall_tick, stall_pool) = stall.expect("team 0's bleed never disarmed");
    assert!(
        death_tick - stall_tick >= 300,
        "the stall lasted only {} ticks",
        death_tick - stall_tick
    );
    assert_eq!(
        stall_pool.to_bits(),
        before.to_bits(),
        "the pool moved during the stall: {stall_pool} -> {before}"
    );
    assert!(before >= 1.0, "the pool still had a whole ticket: {before}");
    assert_eq!(after, before - 1.0, "an ordinary decrement, no clamping");
    assert_eq!(alive0, 0, "team 0 has no post, so its dead stay dead");
    assert_eq!(
        s.battle.teams[0].tickets(),
        0,
        "team 0 is out: pool {} < 1",
        s.battle.teams[0].reinforcements
    );
    assert!(
        s.battle.teams[1].reinforcements > 0.0,
        "team 1 still has a pool: {}",
        s.battle.teams[1].reinforcements
    );
}

/// Queue TLO: a sim with one NEUTRAL post and nobody on the field, so no
/// bleed, death or domination can move the pools — only the clock decides.
fn timed_round(tickets: [u32; 2], limit: Option<u32>) -> Sim {
    let mut s = Sim::empty(0x71);
    s.battle = Battle::new(tickets);
    s.time_limit = limit;
    s.add_cp(
        CpDef {
            name: "cp".into(),
            pos: Vec3::new(0.0, 0.0, -40.0),
            capture: Region::sphere(Vec3::new(0.0, 0.0, -40.0), 8.0),
            control: None,
            value_bleed: 1,
            value_atk: [1, 1],
            value_def: [1, 1],
            spawn_nodes: vec![(Vec3::new(0.0, 0.0, -44.0), 0.0)],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        None,
    );
    s
}

/// Queue TLO: the time limit (`analysis/re-time-limit.md` §1 hop 9 [V][O],
/// `GameOptionsTimeLimitUp`) ends the round on the step that brings the clock
/// to the limit — more reinforcements wins, equal counts are a draw both
/// teams win — and not one tick before.
#[test]
fn the_time_limit_gives_the_round_to_more_tickets_and_a_tie_to_both() {
    for (tickets, want) in [([150, 90], 0u8), ([90, 150], 1), ([120, 120], OUTCOME_DRAW)] {
        let mut s = timed_round(tickets, Some(2));
        for _ in 0..59 {
            s.step(&[]);
        }
        assert_eq!(
            s.outcome, None,
            "{tickets:?}: still running at tick {}",
            s.tick
        );
        s.step(&[]);
        assert_eq!(s.tick, 60, "2 s at 30 Hz");
        assert_eq!(s.outcome, Some(want), "{tickets:?}");
        assert!(s
            .events
            .iter()
            .any(|e| matches!(e, Event::TimeUp { winner } if *winner == want)));
        assert_eq!(
            [s.battle.teams[0].tickets(), s.battle.teams[1].tickets()],
            tickets,
            "nothing but the clock moved"
        );
    }
}

/// Queue TLO: Off (the disc's power-on value) never ends a round, and a
/// limit that has not run out leaves the sim hashing exactly as Off does.
#[test]
fn an_off_time_limit_never_ends_a_round() {
    let mut off = timed_round([120, 120], None);
    let mut long = timed_round([120, 120], Some(60 * 60));
    for _ in 0..(30 * 120) {
        off.step(&[]);
        long.step(&[]);
    }
    assert_eq!(off.outcome, None);
    assert_eq!(long.outcome, None);
    assert_eq!(off.hash(), long.hash(), "the limit itself is not hashed");
}

/// Queue TLO: the reinforcement % (`con_mult`, 10..500) scales each team's
/// own count in integers (`analysis/re-time-limit.md` §5 [V][O]).
#[test]
fn the_reinforcement_percentage_scales_each_count_in_integers() {
    for (count, want) in [(150, [15, 150, 750]), (220, [22, 220, 1100])] {
        let got = [10, 100, 500].map(|pct| scale_reinforcements(count, pct));
        assert_eq!(got, want, "{count}");
    }
    assert_eq!(scale_reinforcements(25, 10), 2, "integer division floors");
}
