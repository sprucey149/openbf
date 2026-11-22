//! Adaptive difficulty (OURS, the maintainer's fourth playtest, 2026-11-22;
//! analysis/plan/FEATURE_SET.md "Adaptive difficulty").
//!
//! At every tier the bots facing the humans drift up to
//! [`MAX_DRIFT_PM`] (10 %) stronger or weaker than the tier's base skill,
//! from how the humans' side is doing against them. The rule:
//!
//! - **When.** Only when the humans (players, human stand-ins and
//!   [`Sim::extra_humans`], [`Sim::humans_per_team`]) are all on one of
//!   teams 0 and 1. A bots-only round, or humans on both sides, keeps the
//!   factor at exactly 1.0 and the state out of the hash, so bots-only
//!   parity sweeps and every bots-only pin keep their numbers. The team with
//!   no human is the BOT TEAM; only its bots drift (the humans' own
//!   teammates keep their DTEAM row).
//! - **Standing.** Every [`UPDATE_SECS`] the humans' side's lead `L` in
//!   per-mille, from the round's score in the order the session log reads
//!   it: Space Assault points, CTF flags, Hero Assault / Hunt / XL points
//!   (each side's points as a share of its own target), else tickets (each
//!   side's remaining tickets as a share of what it had when the round's
//!   first step ran). `L` is the humans' share minus the bots', -1000..=1000;
//!   0 is even. A ticket round where a side cannot lose tickets (campaign
//!   rules, unlimited reinforcements) reads as even.
//! - **Target.** Inside the dead band (`|L|` up to [`DEAD_BAND_PM`], a
//!   45-55 % split) the target is 0. Past it the target rises linearly to
//!   the full drift at `|L|` = [`FULL_AT_PM`] (a 25-75 % split): humans
//!   ahead make the bots stronger, humans behind make them weaker.
//! - **Smoothing.** Each update moves the factor a [`SMOOTH_DIV`]th of the
//!   way to its target (integer per-mille, rounded so it always arrives), so
//!   a step change is a third there after one update (10 s), about 70 % after
//!   three (30 s) and nearly all of it after a minute.
//! - **Effect.** A drifted bot's aim error radius (every on-foot aim site
//!   that also carries the Hunt ease, [`Sim::aim_widen`]) and its reaction
//!   time to a new contact ([`Sim::ai_tier`]) are scaled by
//!   `(1000 - factor) / 1000`: +100 is 10 % tighter aim and quicker reaction,
//!   -100 10 % looser and slower. Nothing else (skill level, row, tokens,
//!   damage) moves.
//!
//! Everything is integer per-mille read off sim state at fixed ticks, so
//! every LAN peer derives the same factor; nothing reads a clock or a draw.
//! Every number here is OURS and PROVISIONAL.
//! measure: the session-stats logs' factor timeline against the humans' win
//! rates per tier (the maintainer's next LAN sessions) before any retune.

use super::Sim;
use crate::TICK_HZ;

/// The largest drift, per-mille of the tier's base strength (10 %).
pub const MAX_DRIFT_PM: i32 = 100;
/// Seconds between two updates.
pub const UPDATE_SECS: u64 = 10;
/// The humans' lead (per-mille share difference) the dead band spans on
/// each side of even: 100 = a 45 / 55 split.
pub const DEAD_BAND_PM: i32 = 100;
/// The lead at which the drift is full: 500 = a 25 / 75 split.
pub const FULL_AT_PM: i32 = 500;
/// Each update closes 1 / this of the gap to the target.
pub const SMOOTH_DIV: i32 = 3;

/// The rule's state on the sim. SIM STATE: hashed only while the factor is
/// off 0 ([`Adaptive::hash_into`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adaptive {
    /// The rule is on (the default). An INPUT: a sweep may turn it off
    /// (`round_to_outcome`'s `SWEEP_ADAPTIVE=0`) to read a stand-in round
    /// the way it played before the rule.
    pub enabled: bool,
    /// Each team's tickets when the round's first step ran.
    start_tickets: Option<[u32; 2]>,
    /// The team whose bots drift, while the rule applies.
    bot_team: Option<u8>,
    /// The drift now, per-mille, -[`MAX_DRIFT_PM`]..=[`MAX_DRIFT_PM`]
    /// (positive = stronger bots).
    factor_pm: i32,
    /// The last update's target, for the logs.
    target_pm: i32,
}

impl Default for Adaptive {
    fn default() -> Self {
        Self {
            enabled: true,
            start_tickets: None,
            bot_team: None,
            factor_pm: 0,
            target_pm: 0,
        }
    }
}

impl Adaptive {
    /// The drift now, per-mille (positive = stronger bots).
    pub fn factor_pm(&self) -> i32 {
        self.factor_pm
    }

    /// The last update's target, per-mille.
    pub fn target_pm(&self) -> i32 {
        self.target_pm
    }

    /// The team whose bots drift, while the rule applies.
    pub fn bot_team(&self) -> Option<u8> {
        self.bot_team
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if self.factor_pm == 0 {
            return;
        }
        eat(b"ADAPTIVE");
        eat(&self.factor_pm.to_le_bytes());
        eat(&[self.bot_team.unwrap_or(u8::MAX)]);
    }
}

/// The target drift for the humans' lead `lead_pm` (-1000..=1000).
pub fn target_for(lead_pm: i32) -> i32 {
    let mag = lead_pm.abs();
    if mag <= DEAD_BAND_PM {
        return 0;
    }
    let span = FULL_AT_PM - DEAD_BAND_PM;
    let t = ((mag - DEAD_BAND_PM).min(span) * MAX_DRIFT_PM) / span;
    t * lead_pm.signum()
}

/// One smoothing step from `factor` toward `target`: a [`SMOOTH_DIV`]th
/// of the gap, rounded away from `factor` so it always arrives.
pub fn smooth(factor: i32, target: i32) -> i32 {
    let gap = target - factor;
    let step = gap / SMOOTH_DIV
        + if gap % SMOOTH_DIV != 0 {
            gap.signum()
        } else {
            0
        };
    (factor + step).clamp(-MAX_DRIFT_PM, MAX_DRIFT_PM)
}

/// `num / den` in per-mille, clamped to 0..=1000 (`den` 0 reads 1000).
fn share_pm(num: i64, den: i64) -> i32 {
    if den <= 0 {
        return 1000;
    }
    ((num.max(0) * 1000) / den).clamp(0, 1000) as i32
}

impl Sim {
    /// The bots' adaptive-difficulty factor: `Some(1.0 + drift)` (0.9..=1.1;
    /// 1.0 = the tier's base strength) while the rule applies to this round,
    /// `None` when it does not (bots only, humans on both sides, or turned
    /// off). Read-only; the session-stats log's
    /// `session_stats::AdaptiveProbe` has this exact shape.
    pub fn adaptive_factor(&self) -> Option<f32> {
        self.adaptive
            .bot_team
            .map(|_| 1.0 + self.adaptive.factor_pm as f32 / 1000.0)
    }

    /// The team the rule would drift now: the one of teams 0 and 1 with no
    /// human while the other has one.
    fn adaptive_bot_team(&self) -> Option<u8> {
        if !self.adaptive.enabled {
            return None;
        }
        match self.humans_per_team() {
            [0, 0] => None,
            [0, _] => Some(0),
            [_, 0] => Some(1),
            _ => None,
        }
    }

    /// Each side's standing in per-mille (higher = closer to winning), or
    /// `None` when the round's score cannot say.
    fn adaptive_standing(&self) -> Option<[i32; 2]> {
        if let Some(sa) = &self.space_assault {
            let lim = i64::from(sa.score_limit.max(1));
            return Some(sa.points.map(|p| share_pm(i64::from(p), lim)));
        }
        if let Some(ctf) = &self.ctf {
            let lim = i64::from(ctf.score_limit.max(1));
            return Some(ctf.team_points.map(|p| share_pm(i64::from(p), lim)));
        }
        if let Some((points, limits)) = self.team_points() {
            return Some(
                [0, 1].map(|t| share_pm(i64::from(points[t]), i64::from(limits[t].max(1)))),
            );
        }
        let b = &self.battle;
        if b.campaign_rules || b.unlimited_reinforcements || b.unlimited_teams.iter().any(|&u| u) {
            return None;
        }
        let start = self.adaptive.start_tickets?;
        Some([0, 1].map(|t| share_pm(i64::from(b.teams[t].tickets()), i64::from(start[t]))))
    }

    /// The rule's per-tick step (after the round's scoring): records the
    /// starting tickets on the first call, then every [`UPDATE_SECS`]
    /// re-reads who the bot team is and moves the factor toward its target.
    pub(crate) fn tick_adaptive(&mut self) {
        if self.adaptive.start_tickets.is_none() {
            self.adaptive.start_tickets = Some([0, 1].map(|t| self.battle.teams[t].tickets()));
        }
        let every = UPDATE_SECS * u64::from(TICK_HZ);
        if self.outcome.is_some() || self.tick == 0 || self.tick % every != 0 {
            return;
        }
        let Some(bot) = self.adaptive_bot_team() else {
            self.adaptive.bot_team = None;
            self.adaptive.factor_pm = 0;
            self.adaptive.target_pm = 0;
            return;
        };
        if self.adaptive.bot_team != Some(bot) {
            // A new bot team (the humans changed sides) starts from even.
            self.adaptive.factor_pm = 0;
        }
        self.adaptive.bot_team = Some(bot);
        let human = usize::from(1 - bot);
        let target = self
            .adaptive_standing()
            .map_or(0, |s| target_for(s[human] - s[usize::from(bot)]));
        self.adaptive.target_pm = target;
        self.adaptive.factor_pm = smooth(self.adaptive.factor_pm, target);
    }

    /// The drift on unit `u`, per-mille: the factor for an ordinary bot of
    /// the bot team, 0 for anyone else (a human, a stand-in, an ally bot).
    fn adaptive_pm_of(&self, u: &crate::unit::Unit) -> i32 {
        if self.adaptive.factor_pm == 0
            || self.adaptive.bot_team != Some(u.team)
            || u.is_player
            || u.controller != crate::unit::Controller::Ai
            || u.proxy_human.is_some()
        {
            return 0;
        }
        self.adaptive.factor_pm
    }

    /// The multiplier on unit `u`'s aim error radius: the Hunt ease
    /// ([`Sim::hunt_aim_widen`]) times the adaptive drift's
    /// `(1000 - factor) / 1000`; 1 for everyone it does not reach.
    pub fn aim_widen(&self, u: &crate::unit::Unit) -> f32 {
        let pm = self.adaptive_pm_of(u);
        let hunt = self.hunt_aim_widen(u);
        if pm == 0 {
            hunt
        } else {
            hunt * (1000 - pm) as f32 / 1000.0
        }
    }

    /// Team `team`'s reaction ticks under the drift: `ticks` scaled by
    /// `(1000 - factor) / 1000`, rounded half to even (so +10 % and -10 %
    /// move Elite's 5 ticks to 4 and 6 alike), at least 1 (0 stays 0);
    /// `ticks` itself for any team the rule does not reach.
    pub(crate) fn adaptive_reaction(&self, team: u8, ticks: u64) -> u64 {
        let pm = self.adaptive.factor_pm;
        if pm == 0 || self.adaptive.bot_team != Some(team) || ticks == 0 {
            return ticks;
        }
        let n = ticks * (1000 - pm) as u64;
        let (q, r) = (n / 1000, n % 1000);
        let scaled = if r > 500 || (r == 500 && q % 2 == 1) {
            q + 1
        } else {
            q
        };
        scaled.max(1)
    }
}

#[cfg(test)]
mod tests;
