//! Command posts: ownership, the two-meter capture machine, BF2 conquest
//! bleed inputs, domination timer, spawn points.
//!
//! Capture machine transcribed from analysis/re-cp-capture.md §4 (BF2 tick
//! `FUN_00357440`, BF1 `FUN_0029f0d8`; lead-verified on the BF2 body: the
//! `×0.5` presence-weight loops, capture-meter drain-then-flip on +0xa4/+0x7c,
//! strict `T < meter` capture, `N <= meter` neutralize, owner-inside recovery
//! decay). openbf runs the BF2 semantics (PLAN: Conquest = BF2 rules):
//! - presence weight `w(n) = Σ_{i<n} 0.5^i = 2 − 2^(1−n)` (1 → 1×, 2 → 1.5×,
//!   3 → 1.75×);
//! - neutral post: the sole team present charges the capture meter at w(n)
//!   (a different team first drains the previous charge, then the meter flips
//!   to it); two teams present = contested, meter frozen; empty = holds;
//!   captured when `CaptureTime < meter` (strict);
//! - owned post: enemies present raise the neutralize meter at w(n_enemies)
//!   (defenders do NOT slow it; weights continue across enemy teams in team
//!   order); neutralized when `NeutralizeTime <= meter` → neutral, capture
//!   meter 0 tagged to the sole attacker; BF2 recovery: the neutralize meter
//!   decays at w(n_owner) only while owner units stand inside and holds
//!   while empty ("saved" when it reaches 0);
//! - DIVERGENCE (documented, report §4 note + §10): BF2 leaves the
//!   neutralize meter at N after a capture, so an immediate counter-attack
//!   neutralizes faster than N. openbf zeroes it on capture (BF1's `== N`
//!   guard reset makes this the evident intent).
//!
//! Who counts: alive soldiers with `CapturePosts` (vehicles never; BF1 heroes
//! 0), tested as POINTS at the entity origin. Spawning: owner == team and the
//! post has spawn nodes; meters ignored (report §6).
//!
//! Bleed (analysis/re-tickets-spawn.md, verified): BF2 ObjectiveConquest —
//! enemy STRICT majority of total CP bleed value → 1/3 ticket/s while
//! `alive < tickets`; zero-CP domination 20 s. Class timings (COMMON.LVL/
//! INGAME.LVL entc): controlzone CaptureTime 10 / NeutralizeTime 12, major
//! 16 / 36 (class defaults 10 / 6).

use crate::battle::{Battle, BF2_BLEED_TICKETS_PER_SEC, BF2_DOMINATION_DEFEAT_SECS};
use crate::region::Region;
use glam::Vec3;

/// Static CP definition (from game::layout).
#[derive(Debug, Clone)]
pub struct CpDef {
    pub name: String,
    pub pos: Vec3,
    pub capture: Region,
    pub control: Option<Region>,
    /// ValueBleed (per-faction values are identical in retail data).
    pub value_bleed: i32,
    /// `Value_ATK_<faction>` RESOLVED PER TEAM INDEX (`[team0, team1]`): what
    /// this post is worth to a team that does not own it. `core` has no notion
    /// of a faction — the faction rows live in `game::layout` and
    /// `game::walk::WalkSim::setup_conquest` picks the row for each team's
    /// `TeamTemplate.faction` — so by the time a post reaches the sim the two
    /// numbers the commander needs are already chosen. PROVISIONAL only in how
    /// they are USED (PLAN "PHASE B DESIGN"); the values themselves are the
    /// disc's.
    pub value_atk: [i32; 2],
    /// `Value_DEF_<faction>` per team index: what this post is worth to the
    /// team that owns it.
    pub value_def: [i32; 2],
    /// (position, yaw) spawn nodes from the CP's SpawnPath.
    pub spawn_nodes: Vec<(Vec3, f32)>,
    /// Seconds for a neutral CP to become owned / an owned CP to go neutral.
    pub capture_time: f32,
    pub neutralize_time: f32,
}

impl CpDef {
    /// What this post is worth to `team` right now, under the BF2 rule
    /// (`analysis/re-cp-capture.md` §176-177, PLAN "Phase B addendum"): the
    /// DEFEND value when the team already owns it, the ATTACK value when the
    /// other team owns it, and the average of the two when it is neutral.
    pub fn value_for(&self, team: u8, owner: Option<u8>) -> i32 {
        let t = usize::from(team).min(1);
        match owner {
            Some(o) if o == team => self.value_def[t],
            Some(_) => self.value_atk[t],
            None => (self.value_atk[t] + self.value_def[t]) / 2,
        }
    }

    /// Shipped class timings: (CaptureTime, NeutralizeTime).
    pub fn class_times(major: bool) -> (f32, f32) {
        if major {
            (16.0, 36.0)
        } else {
            (10.0, 12.0)
        }
    }
}

/// Presence weight `w(n) = Σ_{i<n} 0.5^i` (FUN_00357440 `×0.5` loops).
pub fn presence_weight(n: u32) -> f32 {
    let mut w = 0.0;
    let mut k = 1.0;
    for _ in 0..n {
        w += k;
        k *= 0.5;
    }
    w
}

#[derive(Debug, Clone)]
pub struct CommandPost {
    pub def: CpDef,
    /// Owning team (0-based) or None = neutral.
    pub owner: Option<u8>,
    /// Capture meter (seconds) and the team it is charged for.
    pub capture: f32,
    pub capture_team: Option<u8>,
    /// Neutralize meter (seconds).
    pub neutral: f32,
    /// The planning hub this post routes to — `NavGraph::hub_for(capture
    /// centre)`, resolved ONCE by [`crate::sim::Sim::add_cp`] because capture
    /// centres do not move. An INPUT-derived cache: `None` on a map with no
    /// graph, and never part of the hash.
    pub nav_hub: Option<u16>,
    /// BF2's `AISpawnWeight` (re-command-walker-post §6 [O]: cp+0x11c, the
    /// constructor's 100.0, an instance key clamped ≥ 0): this post's weight
    /// in a bot's draw of where to spawn (`Sim::keep_or_draw_spawn_post`).
    /// BF1 has no such key — every post keeps the default, a uniform draw.
    /// Never changes after setup, so it is not part of the hash.
    pub ai_spawn_weight: f32,
    /// Card MAIS: `AICanCaptureCP(post, team, false)` - bit `t` set: team
    /// `t`'s bots do not count in this post's capture (BF2 `cp+0x80`,
    /// `FUN_003573f8`; re-cp-capture.md 4). Set from the mission, never
    /// changed after setup, so it is not part of the hash.
    pub ai_capture_ban: u8,
}

/// The `AISpawnWeight` a post is constructed with (BF2; a command walker's
/// post too).
pub const AI_SPAWN_WEIGHT_DEFAULT: f32 = 100.0;

/// What changed during a CP update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpChange {
    None,
    Neutralized {
        by: u8,
    },
    Captured {
        by: u8,
    },
    /// Owner units brought the neutralize meter back to 0.
    Saved {
        team: u8,
    },
}

impl CommandPost {
    pub fn new(def: CpDef, owner: Option<u8>) -> Self {
        Self {
            def,
            owner,
            capture: 0.0,
            capture_team: None,
            neutral: 0.0,
            nav_hub: None,
            ai_spawn_weight: AI_SPAWN_WEIGHT_DEFAULT,
            ai_capture_ban: 0,
        }
    }

    /// Card MAIS: may team `team`'s bots not capture this post?
    pub fn ai_no_capture(&self, team: u8) -> bool {
        team < 8 && self.ai_capture_ban >> team & 1 != 0
    }

    /// One tick given the per-team headcount of counting units inside the
    /// capture region (`inside[t]` = alive CapturePosts soldiers of team t).
    pub fn update(&mut self, dt: f32, inside: &[u32]) -> CpChange {
        let t_cap = self.def.capture_time;
        let t_neu = self.def.neutralize_time;
        match self.owner {
            None => {
                // The teams with someone inside, counted in team order. No
                // list is built: only "exactly one" and "which one" are asked.
                let mut present = 0u32;
                let mut first = 0u8;
                for (t, &n) in inside.iter().enumerate() {
                    if n > 0 {
                        if present == 0 {
                            first = t as u8;
                        }
                        present += 1;
                    }
                }
                if present != 1 {
                    // Empty: holds. Contested: frozen (dispute sound).
                    return CpChange::None;
                }
                let t = first;
                let speed = presence_weight(inside[t as usize]);
                if self.capture_team == Some(t) {
                    self.capture += speed * dt;
                } else {
                    // Drain the previous team's charge first, then flip.
                    self.capture -= speed * dt;
                    if self.capture <= 0.0 {
                        self.capture = -self.capture;
                        self.capture_team = Some(t);
                    }
                }
                if t_cap < self.capture {
                    self.capture = t_cap;
                    self.owner = Some(t);
                    // openbf: clear the leftover neutralize meter (see module doc).
                    self.neutral = 0.0;
                    return CpChange::Captured { by: t };
                }
                CpChange::None
            }
            Some(o) => {
                // The enemy teams inside, walked in team order — the same walk
                // accumulates the weights, so nothing is collected. Weights
                // continue across enemy teams in team order.
                let mut enemies = 0u32;
                let mut first_enemy = 0u8;
                let mut speed = 0.0;
                let mut k = 1.0;
                for (t, &n) in inside.iter().enumerate() {
                    if n == 0 || t as u8 == o {
                        continue;
                    }
                    if enemies == 0 {
                        first_enemy = t as u8;
                    }
                    enemies += 1;
                    for _ in 0..n {
                        speed += k;
                        k *= 0.5;
                    }
                }
                if enemies > 0 {
                    self.neutral += speed * dt;
                    if t_neu <= self.neutral {
                        self.neutral = t_neu;
                        self.capture = 0.0;
                        self.capture_team = if enemies == 1 {
                            Some(first_enemy)
                        } else {
                            None
                        };
                        self.owner = None;
                        return CpChange::Neutralized { by: first_enemy };
                    }
                } else if self.neutral > 0.0 && inside.get(o as usize).is_some_and(|&n| n > 0) {
                    // BF2 recovery: decays only with owner units inside.
                    self.neutral -= presence_weight(inside[o as usize]) * dt;
                    if self.neutral <= 0.0 {
                        self.neutral = 0.0;
                        return CpChange::Saved { team: o };
                    }
                }
                CpChange::None
            }
        }
    }

    /// Spawnable for `team`: owned outright with spawn nodes (meters are
    /// ignored — CanTeamSpawnAt, report §6).
    pub fn spawnable_by(&self, team: u8) -> bool {
        self.owner == Some(team) && !self.def.spawn_nodes.is_empty()
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[self.owner.unwrap_or(255), self.capture_team.unwrap_or(255)]);
        eat(&self.capture.to_bits().to_le_bytes());
        eat(&self.neutral.to_bits().to_le_bytes());
    }
}

/// BF2 ObjectiveConquest bleed recompute for `team` over `cps`: 1/3 per
/// second when the enemy holds a strict majority of the total bleed value.
pub fn bf2_bleed_rate(cps: &[CommandPost], team: u8) -> f32 {
    let total: i32 = cps.iter().map(|c| c.def.value_bleed).sum();
    let enemy: i32 = cps
        .iter()
        .filter(|c| c.owner.is_some_and(|o| o != team))
        .map(|c| c.def.value_bleed)
        .sum();
    if (total as f32) / 2.0 < enemy as f32 {
        BF2_BLEED_TICKETS_PER_SEC
    } else {
        0.0
    }
}

/// Apply the BF2 bleed rule to the battle: rate only while `alive < tickets`
/// (the Lua timer's clamp), 0 otherwise. Returns the rates set.
pub fn apply_bleed(battle: &mut Battle, cps: &[CommandPost]) -> [f32; 2] {
    let mut rates = [0.0; 2];
    for (t, r) in rates.iter_mut().enumerate() {
        let team = &battle.teams[t];
        let rate = bf2_bleed_rate(cps, t as u8);
        *r = if team.alive() < team.tickets() && team.tickets() > 0 {
            rate
        } else {
            0.0
        };
        battle.teams[t].bleed_per_sec = *r;
    }
    rates
}

/// Domination: the team owning EVERY post (none neutral) for
/// `BF2_DOMINATION_DEFEAT_SECS` wins, the round ending
/// `BF2_DOMINATION_DITTY_SECS` after the countdown's 0. `state` = (dominating
/// team, seconds). Returns the loser then.
pub fn tick_domination(
    cps: &[CommandPost],
    state: &mut Option<(u8, f32)>,
    dt: f32,
    teams: u8,
) -> Option<u8> {
    let dominant = (0..teams).find(|&t| !cps.is_empty() && cps.iter().all(|c| c.owner == Some(t)));
    match dominant {
        None => {
            *state = None;
            None
        }
        Some(t) => {
            let secs = match *state {
                Some((prev, s)) if prev == t => s + dt,
                _ => dt,
            };
            *state = Some((t, secs));
            (secs >= BF2_DOMINATION_DEFEAT_SECS + crate::battle::BF2_DOMINATION_DITTY_SECS)
                .then_some(if t == 0 { 1 } else { 0 })
        }
    }
}

#[cfg(test)]
mod tests {
    //! Reference vectors from re-cp-capture.md §8 (T = 10, N = 12; major
    //! 16/36). Times measured in 1/30 s ticks; ±1 tick tolerance.
    use super::*;

    fn def(name: &str, x: f32, major: bool) -> CpDef {
        let (capture_time, neutralize_time) = CpDef::class_times(major);
        CpDef {
            name: name.into(),
            pos: Vec3::new(x, 0.0, 0.0),
            capture: Region::sphere(Vec3::new(x, 0.0, 0.0), 8.0),
            control: None,
            value_bleed: 10,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![(Vec3::new(x, 0.0, 3.0), 0.0)],
            capture_time,
            neutralize_time,
        }
    }

    const DT: f32 = 1.0 / 30.0;

    /// Run until `until` returns true or `max` seconds pass; returns seconds.
    fn run_until(
        cp: &mut CommandPost,
        inside: &[u32],
        max: f32,
        mut until: impl FnMut(CpChange) -> bool,
    ) -> f32 {
        let mut t = 0.0;
        while t < max {
            let ev = cp.update(DT, inside);
            t += DT;
            if until(ev) {
                return t;
            }
        }
        panic!("condition not reached within {max} s");
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() <= DT + 1e-4
    }

    /// The BF2 rule: DEF when I own it, ATK when the other side does, the
    /// average when it is neutral — per TEAM INDEX, and the two teams read
    /// different numbers off the same post.
    #[test]
    fn value_for_is_def_atk_or_the_average() {
        let mut d = def("cp1", 0.0, false);
        d.value_atk = [30, 20];
        d.value_def = [50, 10];
        // Team 0.
        assert_eq!(d.value_for(0, Some(0)), 50, "owned by me -> DEF");
        assert_eq!(d.value_for(0, Some(1)), 30, "owned by them -> ATK");
        assert_eq!(d.value_for(0, None), 40, "neutral -> average");
        // Team 1 reads its own row.
        assert_eq!(d.value_for(1, Some(1)), 10);
        assert_eq!(d.value_for(1, Some(0)), 20);
        assert_eq!(d.value_for(1, None), 15);
        // The average truncates (integer values, like the disc's).
        d.value_atk = [11, 0];
        d.value_def = [0, 0];
        assert_eq!(d.value_for(0, None), 5);
    }

    #[test]
    fn presence_weight_series() {
        assert_eq!(presence_weight(0), 0.0);
        assert_eq!(presence_weight(1), 1.0);
        assert_eq!(presence_weight(2), 1.5);
        assert_eq!(presence_weight(3), 1.75);
        assert!((presence_weight(20) - 2.0).abs() < 1e-5);
    }

    /// V1: neutral post, 1 / 3 / 4 units → 10.0 / 5.714 / 5.333 s.
    #[test]
    fn v1_capture_times_scale_with_presence_weight() {
        for (n, want) in [(1u32, 10.0f32), (3, 10.0 / 1.75), (4, 10.0 / 1.875)] {
            let mut cp = CommandPost::new(def("cp", 0.0, false), None);
            let t = run_until(&mut cp, &[n, 0], 20.0, |e| {
                e == CpChange::Captured { by: 0 }
            });
            assert!(near(t, want), "{n} units: {t} vs {want}");
            assert_eq!(cp.owner, Some(0));
            assert_eq!(cp.neutral, 0.0, "openbf zeroes the leftover meter");
        }
    }

    /// V2: owned by A(1), 1 unit of B(0): neutral at 12 s, owned by B at 22 s;
    /// 4 B units: 6.4 + 5.333 s.
    #[test]
    fn v2_neutralize_then_capture() {
        let mut cp = CommandPost::new(def("cp", 0.0, false), Some(1));
        let t1 = run_until(&mut cp, &[1, 0], 20.0, |e| {
            e == CpChange::Neutralized { by: 0 }
        });
        assert!(near(t1, 12.0), "{t1}");
        assert_eq!(
            (cp.owner, cp.capture_team, cp.capture),
            (None, Some(0), 0.0)
        );
        let t2 = run_until(&mut cp, &[1, 0], 20.0, |e| {
            e == CpChange::Captured { by: 0 }
        });
        assert!(near(t2, 10.0), "{t2}");
        let mut cp4 = CommandPost::new(def("cp", 0.0, false), Some(1));
        let a = run_until(&mut cp4, &[4, 0], 20.0, |e| {
            e == CpChange::Neutralized { by: 0 }
        });
        let b = run_until(&mut cp4, &[4, 0], 20.0, |e| {
            e == CpChange::Captured { by: 0 }
        });
        assert!(near(a, 6.4) && near(b, 16.0 / 3.0), "{a} {b}");
    }

    /// V3: defenders never slow neutralization (1 B + 2 A → 12 s).
    #[test]
    fn v3_defenders_do_not_slow_neutralization() {
        let mut cp = CommandPost::new(def("cp", 0.0, false), Some(1));
        let t = run_until(&mut cp, &[1, 2], 20.0, |e| {
            e == CpChange::Neutralized { by: 0 }
        });
        assert!(near(t, 12.0), "{t}");
    }

    /// V4: A charges to 6 s and leaves; B drains 6 s, flips, charges 10 →
    /// B owns 16 s after entering.
    #[test]
    fn v4_drain_then_flip() {
        let mut cp = CommandPost::new(def("cp", 0.0, false), None);
        for _ in 0..180 {
            cp.update(DT, &[1, 0]);
        }
        assert!((cp.capture - 6.0).abs() < 0.05 && cp.capture_team == Some(0));
        let t = run_until(&mut cp, &[0, 1], 30.0, |e| {
            e == CpChange::Captured { by: 1 }
        });
        assert!(near(t, 16.0), "{t}");
    }

    /// V5: no decay while empty; A returns with 2 units → (10−6)/1.5 = 2.667 s.
    #[test]
    fn v5_holds_when_empty() {
        let mut cp = CommandPost::new(def("cp", 0.0, false), None);
        for _ in 0..180 {
            cp.update(DT, &[1, 0]);
        }
        for _ in 0..300 {
            assert_eq!(cp.update(DT, &[0, 0]), CpChange::None);
        }
        assert!((cp.capture - 6.0).abs() < 0.05);
        let t = run_until(&mut cp, &[2, 0], 10.0, |e| {
            e == CpChange::Captured { by: 0 }
        });
        assert!(near(t, 4.0 / 1.5), "{t}");
    }

    /// V6: contested freezes the capture meter; resumes when one side leaves.
    #[test]
    fn v6_contested_freezes() {
        let mut cp = CommandPost::new(def("cp", 0.0, false), None);
        for _ in 0..90 {
            cp.update(DT, &[1, 0]);
        }
        let m = cp.capture;
        for _ in 0..300 {
            cp.update(DT, &[1, 1]);
        }
        assert_eq!(cp.capture, m);
        let t = run_until(&mut cp, &[1, 0], 20.0, |e| {
            e == CpChange::Captured { by: 0 }
        });
        assert!(near(t, 10.0 - 3.0), "{t}");
    }

    /// V7 (BF2): neutralize meter holds while empty, decays at w(owner) only
    /// with owner units inside → "saved" after 6 s with 1 owner unit.
    #[test]
    fn v7_bf2_recovery_holds_then_decays() {
        let mut cp = CommandPost::new(def("cp", 0.0, false), Some(1));
        for _ in 0..180 {
            cp.update(DT, &[1, 0]);
        }
        assert!((cp.neutral - 6.0).abs() < 0.05);
        for _ in 0..300 {
            assert_eq!(cp.update(DT, &[0, 0]), CpChange::None);
        }
        assert!((cp.neutral - 6.0).abs() < 0.05, "holds while empty");
        let t = run_until(&mut cp, &[0, 1], 20.0, |e| e == CpChange::Saved { team: 1 });
        assert!(near(t, 6.0), "{t}");
        assert_eq!(cp.neutral, 0.0);
        assert!(cp.spawnable_by(1) && !cp.spawnable_by(0));
    }

    /// V8: major post (16/36), 3 attackers: neutralize 20.571 s, capture 9.143 s.
    #[test]
    fn v8_major_post_timings() {
        let mut cp = CommandPost::new(def("cp10", 0.0, true), Some(1));
        let a = run_until(&mut cp, &[3, 0], 40.0, |e| {
            e == CpChange::Neutralized { by: 0 }
        });
        let b = run_until(&mut cp, &[3, 0], 40.0, |e| {
            e == CpChange::Captured { by: 0 }
        });
        assert!(near(a, 36.0 / 1.75), "{a}");
        assert!(near(b, 16.0 / 1.75), "{b}");
    }

    /// re-tickets-spawn §1.6: 6 CPs à 10 — enemy holding 3 → no bleed
    /// (majority must be strict), 4 → 1/3 ticket/s; stops at alive == tickets.
    #[test]
    fn bf2_bleed_majority_and_clamp() {
        let mut cps: Vec<CommandPost> = (0..6)
            .map(|i| CommandPost::new(def(&format!("cp{i}"), i as f32 * 50.0, false), Some(1)))
            .collect();
        for cp in cps.iter_mut().take(3) {
            cp.owner = Some(0);
        }
        assert_eq!(bf2_bleed_rate(&cps, 0), 0.0);
        assert_eq!(bf2_bleed_rate(&cps, 1), 0.0);
        cps[3].owner = Some(0); // team 0 holds 4/6
        assert_eq!(bf2_bleed_rate(&cps, 1), BF2_BLEED_TICKETS_PER_SEC);
        assert_eq!(bf2_bleed_rate(&cps, 0), 0.0);

        let mut b = Battle::new([20, 20]);
        b.teams[1].alive_ai = 5;
        assert_eq!(apply_bleed(&mut b, &cps), [0.0, BF2_BLEED_TICKETS_PER_SEC]);
        for _ in 0..(30 * 9) {
            apply_bleed(&mut b, &cps);
            b.tick_bleed(DT);
        }
        assert_eq!(b.teams[1].tickets(), 17, "3 tickets in 9 s");
        b.teams[1].alive_ai = 17;
        assert_eq!(apply_bleed(&mut b, &cps)[1], 0.0);
        for _ in 0..300 {
            apply_bleed(&mut b, &cps);
            b.tick_bleed(DT);
        }
        assert_eq!(b.teams[1].tickets(), 17);
    }

    #[test]
    fn domination_timer_defeats_after_20s() {
        let mut cps: Vec<CommandPost> = (0..3)
            .map(|i| CommandPost::new(def(&format!("cp{i}"), i as f32 * 50.0, false), Some(0)))
            .collect();
        let mut st = None;
        let mut t = 0.0;
        let mut loser = None;
        while loser.is_none() && t < 30.0 {
            loser = tick_domination(&cps, &mut st, DT, 2);
            t += DT;
        }
        assert_eq!(loser, Some(1));
        // The 20 s countdown, then BF2's 1 s step (issue #33).
        assert!((t - 21.0).abs() < 0.05, "{t}");
        cps[1].owner = None;
        assert_eq!(tick_domination(&cps, &mut st, DT, 2), None);
        assert_eq!(st, None);
    }
}
