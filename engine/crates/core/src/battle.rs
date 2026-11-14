//! Battle state v0: per-team reinforcements with the openbf NO-OVERSPAWN
//! invariant (PLAN §Modes & rules, locked 2026-08-30):
//!
//!   live units on a team may never exceed remaining reinforcements — when a
//!   player (re)spawns into the last slots, excess AI despawn. Players are
//!   never despawned; AI absorb the shortfall.
//!
//! Model — RE-CONFIRMED for both games (analysis/re-tickets-spawn.md,
//! lead-verified): DEATH consumes the reinforcement (BF1 FUN_00297440:
//! tickets at team+0x10 decrement, floor 0; BF2 via Objective.lua
//! AddReinforcements(team,-1)); spawning consumes nothing; AI (re)spawn is
//! engine-gated while tickets <= aliveCount in BOTH games (BF1 FUN_00295c40
//! team+0x10 vs +0x2c; BF2 FUN_0034cea8 team+0x24 vs +0x38). The openbf
//! addition on top: a PLAYER spawning into the last slots despawns AI to
//! make room (PLAN worked example) — the originals simply refuse.
//!
//! BF2 shipping bleed (ObjectiveConquest.lua, PS2 retail runs the MP rules
//! path in SP): enemy strict majority of CP bleed value → −1 ticket per 3 s
//! (rate 1/3), clamped so tickets never drop below the alive count; 20 s
//! zero-CP domination timer. BF1's own engine bleed is LIVE on the PS2
//! (re-tickets-spawn.md §2.4 as corrected 2026-09-30, re-announcer.md §5:
//! 0.1875 / 0.5625 / 0.75 tickets/s by post value, live RAM on NAB1) and is
//! NOT ported: the sim plays BF2's rule on every map.

use crate::fmath::FloatExt;

/// BF2 conquest bleed constants (ObjectiveConquest.lua via re-tickets-spawn.md).
pub const BF2_BLEED_TICKETS_PER_SEC: f32 = 1.0 / 3.0;
/// Seconds a team may hold zero CPs before losing outright (defeatTimerSeconds):
/// the "Victory In" / "Defeat In" countdown (issue #33, `analysis/re-round-end.md` §1.1).
pub const BF2_DOMINATION_DEFEAT_SECS: f32 = 20.0;
/// After the countdown's 0 the objective completes and `dittyTimer` (1.0 s)
/// calls `MissionVictory` (`objective.luac` main.f9; re-round-end.md §1.1):
/// the round ends 21 s after one team took every post.
pub const BF2_DOMINATION_DITTY_SECS: f32 = 1.0;

/// One team's reinforcement pool + live census.
#[derive(Debug, Clone, Copy)]
pub struct TeamState {
    /// Remaining reinforcements ("tickets"). Fractional to carry bleed.
    pub reinforcements: f32,
    pub alive_players: u32,
    pub alive_ai: u32,
    /// Currently applied ticket drain per second (0 = no bleed), set by
    /// [`crate::cp::apply_bleed`] for the command-post majority.
    pub bleed_per_sec: f32,
}

impl TeamState {
    pub fn new(reinforcements: u32) -> Self {
        Self {
            reinforcements: reinforcements as f32,
            alive_players: 0,
            alive_ai: 0,
            bleed_per_sec: 0.0,
        }
    }

    pub fn alive(&self) -> u32 {
        self.alive_players + self.alive_ai
    }

    /// Whole reinforcements still available.
    pub fn tickets(&self) -> u32 {
        self.reinforcements.fmax(0.0) as u32
    }

    /// The enforceable form of the invariant: AI never push live count
    /// beyond remaining reinforcements. Players already on the field are
    /// never despawned, so when bleed/deaths drop tickets below the live
    /// PLAYER count the pool simply can't cover respawns — the ceiling is
    /// max(tickets, alive_players).
    pub fn invariant_holds(&self) -> bool {
        self.alive() <= self.tickets().max(self.alive_players)
    }
}

/// Outcome of a spawn request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnOutcome {
    /// Spawned; `despawned_ai` AI were removed to keep the invariant.
    Spawned { despawned_ai: u32 },
    /// No reinforcements left (or AI would displace a player slot).
    Refused,
}

#[derive(Debug, Clone)]
pub struct Battle {
    pub teams: [TeamState; 2],
    /// A mission's EXTRA per-death drains: `death_drain[dying][paying]`
    /// tickets `paying` loses when a unit of `dying` dies, on top of the one
    /// [`Battle::on_death`] always takes. All zero except on BF2 Death Star
    /// conquest, whose `ScriptPostLoad` registers `OnCharacterDeathTeam` →
    /// `AddReinforcements(1, -1)` for Lua team 1 (the port's team 0), so a
    /// team-0 death costs team 0 two tickets (analysis/re-time-limit.md §6
    /// [V][O]; queue DSD). PROVISIONAL [O]: that the filter selects the DYING
    /// character's team is [D], and the floor at 0 is ours (§6 [H]). Not in
    /// `Sim::hash`: it is fixed setup, and what it spends lands in
    /// `reinforcements`, which is.
    pub death_drain: [[u32; 2]; 2],
    pub unlimited_reinforcements: bool,
}

impl Battle {
    pub fn new(reinforcements: [u32; 2]) -> Self {
        Self {
            teams: [
                TeamState::new(reinforcements[0]),
                TeamState::new(reinforcements[1]),
            ],
            death_drain: [[0; 2]; 2],
            unlimited_reinforcements: false,
        }
    }

    /// A unit died: the team pays one reinforcement, plus any mission drain
    /// ([`Battle::death_drain`]). A drain can leave the paying team with
    /// fewer tickets than live units; the next [`Battle::tick_bleed`] sheds
    /// the AI even when command-post bleed is clamped off.
    pub fn on_death(&mut self, team: usize, was_player: bool) {
        let t = &mut self.teams[team];
        if was_player {
            t.alive_players = t.alive_players.saturating_sub(1);
        } else {
            t.alive_ai = t.alive_ai.saturating_sub(1);
        }
        if self.unlimited_reinforcements {
            return;
        }
        // Floor at 0: BF1's engine does `if (tickets>0) tickets--`
        // (`FUN_00297440`) and BF2's `Objective.lua` `OnCharacterDeath` guards
        // `GetReinforcementCount(t) > 0` before `AddReinforcements(t,-1)`
        // (analysis/re-tickets-spawn.md §0 and §4, both [V]). `tickets()`
        // clamps on READ, but `Sim::hash` eats the raw field, so an unfloored
        // pool hashes a state neither disc can hold.
        t.reinforcements = (t.reinforcements - 1.0).fmax(0.0);
        debug_assert!(t.invariant_holds() || self.death_drain != [[0; 2]; 2]);
        for (paying, &n) in self.death_drain[team].iter().enumerate() {
            if n > 0 {
                let p = &mut self.teams[paying];
                p.reinforcements = (p.reinforcements - n as f32).fmax(0.0);
            }
        }
    }

    /// A unit wants to spawn. Players always fit while any reinforcement
    /// remains (AI despawn to make room); AI only spawn into genuinely free
    /// reinforcement slots.
    pub fn try_spawn(&mut self, team: usize, is_player: bool) -> SpawnOutcome {
        let t = &mut self.teams[team];
        let tickets = t.tickets();
        if is_player {
            if tickets == 0 || t.alive_players >= tickets {
                return SpawnOutcome::Refused;
            }
            t.alive_players += 1;
            // Enforce the invariant by shedding AI.
            let mut despawned = 0;
            while t.alive() > t.tickets().max(t.alive_players) && t.alive_ai > 0 {
                t.alive_ai -= 1;
                despawned += 1;
            }
            debug_assert!(t.invariant_holds());
            SpawnOutcome::Spawned {
                despawned_ai: despawned,
            }
        } else {
            if t.alive() >= tickets {
                return SpawnOutcome::Refused;
            }
            t.alive_ai += 1;
            debug_assert!(t.invariant_holds());
            SpawnOutcome::Spawned { despawned_ai: 0 }
        }
    }

    /// Spend ticket bleed and repair the no-overspawn invariant; returns AI
    /// shed per team this tick. The sim's command-post bleed clamps at the
    /// live count, but mission death drains can leave excess AI even with
    /// zero bleed (`analysis/re-bleed-reachability.md`). Direct callers may
    /// also supply an unclamped bleed rate.
    pub fn tick_bleed(&mut self, dt: f32) -> [u32; 2] {
        let mut shed = [0u32; 2];
        for (i, t) in self.teams.iter_mut().enumerate() {
            if t.bleed_per_sec > 0.0 && t.reinforcements > 0.0 {
                t.reinforcements = (t.reinforcements - t.bleed_per_sec * dt).fmax(0.0);
            }
            while t.alive() > t.tickets().max(t.alive_players) && t.alive_ai > 0 {
                t.alive_ai -= 1;
                shed[i] += 1;
            }
        }
        shed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PLAN §Modes & rules worked example: 3 tickets, 2 players + 1 AI alive;
    /// a player dies and respawns → the AI teammate despawns.
    #[test]
    fn plan_worked_example() {
        let mut b = Battle::new([3, 100]);
        b.teams[0].alive_players = 2;
        b.teams[0].alive_ai = 1;
        assert!(b.teams[0].invariant_holds());

        b.on_death(0, true); // player dies: tickets 3 -> 2, alive 2
        assert_eq!(b.teams[0].tickets(), 2);
        let out = b.try_spawn(0, true); // player respawns into the last slots
        assert_eq!(out, SpawnOutcome::Spawned { despawned_ai: 1 });
        assert_eq!(b.teams[0].alive_players, 2);
        assert_eq!(b.teams[0].alive_ai, 0);
        assert!(b.teams[0].invariant_holds());
    }

    #[test]
    fn ai_never_overspawn_and_players_displace_ai() {
        let mut b = Battle::new([2, 100]);
        assert_eq!(
            b.try_spawn(0, false),
            SpawnOutcome::Spawned { despawned_ai: 0 }
        );
        assert_eq!(
            b.try_spawn(0, false),
            SpawnOutcome::Spawned { despawned_ai: 0 }
        );
        // Third AI refused: 2 alive, 2 tickets.
        assert_eq!(b.try_spawn(0, false), SpawnOutcome::Refused);
        // A player still gets in — one AI is shed to make room.
        assert_eq!(
            b.try_spawn(0, true),
            SpawnOutcome::Spawned { despawned_ai: 1 }
        );
        assert!(b.teams[0].invariant_holds());
        // With zero tickets nobody spawns.
        b.teams[0].reinforcements = 0.0;
        assert_eq!(b.try_spawn(0, true), SpawnOutcome::Refused);
        assert_eq!(b.try_spawn(0, false), SpawnOutcome::Refused);
    }

    /// P4 acceptance criterion: the invariant survives arbitrary event
    /// sequences (pseudo-random spawns/deaths/bleed).
    #[test]
    fn invariant_survives_random_event_storm() {
        let mut b = Battle::new([50, 50]);
        b.teams[0].bleed_per_sec = 2.0;
        b.teams[1].bleed_per_sec = 1.0;
        let mut rng = crate::rng::SimRng::new(0xB00B1E5);
        for _ in 0..5000 {
            let r = rng.next_rand();
            let team = (r & 1) as usize;
            let is_player = r & 2 != 0;
            match (r >> 2) % 3 {
                0 => {
                    b.try_spawn(team, is_player);
                }
                1 => {
                    let t = &b.teams[team];
                    if (is_player && t.alive_players > 0) || (!is_player && t.alive_ai > 0) {
                        b.on_death(team, is_player);
                    }
                }
                _ => {
                    b.tick_bleed(1.0 / 30.0);
                }
            }
            for t in &b.teams {
                assert!(
                    t.invariant_holds(),
                    "live {} > tickets {}",
                    t.alive(),
                    t.tickets()
                );
            }
        }
    }

    #[test]
    fn bleed_drains_and_sheds_ai() {
        let mut b = Battle::new([3, 3]);
        b.teams[0].bleed_per_sec = 1.0;
        b.teams[0].alive_ai = 3;
        // After ~1s of bleed the pool has lost a ticket -> AI get shed.
        let mut shed_total = 0;
        for _ in 0..31 {
            shed_total += b.tick_bleed(1.0 / 30.0)[0];
        }
        assert!(b.teams[0].tickets() <= 2);
        assert!(shed_total >= 1);
        assert!(b.teams[0].invariant_holds());
        // Drain to zero and shed the remaining AI. Players are exempt from
        // shedding; Sim's ticket-zero outcome does not require alive == 0.
        for _ in 0..200 {
            b.tick_bleed(1.0 / 30.0);
        }
        assert_eq!(b.teams[0].tickets(), 0);
        assert_eq!(b.teams[0].alive(), 0, "all AI shed as tickets vanished");
    }

    /// `re-tickets-spawn.md` §0 and §4 [V]: DEATH consumes the reinforcement
    /// with a FLOOR AT 0 in both engines. `cleanse-core-nav`'s repro: a pool
    /// bled to 0 with one player still alive hashed as −1.0 when that player
    /// died, a state neither disc can hold (`Sim::step` keeps processing
    /// deaths after the outcome).
    #[test]
    fn death_floors_the_pool_at_zero() {
        let mut b = Battle::new([2, 100]);
        b.teams[0].alive_players = 1;
        b.teams[0].bleed_per_sec = 1.0;
        for _ in 0..70 {
            b.tick_bleed(1.0 / 30.0);
        }
        assert_eq!(b.teams[0].reinforcements, 0.0, "bleed already floors");
        assert!(b.teams[0].invariant_holds(), "a live player is exempt");

        b.on_death(0, true); // the last player dies with an empty pool
        assert_eq!(b.teams[0].reinforcements, 0.0, "was -1.0 before the guard");
        assert_eq!(b.teams[0].tickets(), 0);

        // Whole tickets still pay one per death, exactly as the discs do.
        let mut c = Battle::new([2, 100]);
        c.teams[0].alive_ai = 2;
        c.on_death(0, false);
        assert_eq!(c.teams[0].reinforcements, 1.0);
        c.on_death(0, false);
        assert_eq!(c.teams[0].reinforcements, 0.0);
    }

    /// DSD: Death Star conquest's extra drain — a team-0 death costs team 0
    /// two tickets, a team-1 death one, and neither pool drops below 0; the
    /// next bleed tick sheds the AI the drain left without a ticket.
    #[test]
    fn a_mission_death_drain_costs_the_extra_ticket_floored_at_zero() {
        let mut b = Battle::new([10, 10]);
        b.death_drain[0][0] = 1;
        b.teams[0].alive_ai = 3;
        b.teams[1].alive_ai = 3;
        b.on_death(0, false);
        assert_eq!(b.teams[0].tickets(), 8, "a team-0 death costs two");
        assert_eq!(b.teams[1].tickets(), 10);
        b.on_death(1, false);
        assert_eq!(b.teams[0].tickets(), 8);
        assert_eq!(b.teams[1].tickets(), 9, "a team-1 death costs one");

        // Floor: one ticket left, a team-0 death spends both halves of it.
        let mut c = Battle::new([1, 10]);
        c.death_drain[0][0] = 1;
        c.teams[0].alive_ai = 3;
        c.on_death(0, false);
        assert_eq!(c.teams[0].reinforcements, 0.0, "never below 0");
        // 2 AI alive on 0 tickets: the bleed tick sheds them.
        assert_eq!(c.tick_bleed(1.0 / 30.0), [2, 0]);
        assert!(c.teams[0].invariant_holds());

        // With the drain empty a death is the plain one ticket.
        let mut d = Battle::new([10, 10]);
        d.teams[0].alive_ai = 1;
        d.on_death(0, false);
        assert_eq!(d.teams[0].tickets(), 9);
    }

    /// A mid-bleed fraction is floored too: 0.5 − 1 is 0 in the hash, not −0.5.
    #[test]
    fn a_fractional_pool_floors_at_zero_on_death() {
        let mut b = Battle::new([1, 100]);
        b.teams[0].alive_players = 1;
        b.teams[0].reinforcements = 0.5;
        b.on_death(0, true);
        assert_eq!(b.teams[0].reinforcements, 0.0);
    }
}
