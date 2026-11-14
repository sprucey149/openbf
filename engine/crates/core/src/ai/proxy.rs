//! Card DPRX (OURS, a measuring tool): the HUMAN PROXY - a bot the sim treats
//! as a human player for every human-only rule, so the sweep and the arena
//! can measure difficulty the way a human meets it, without one playing.
//!
//! A proxy is an ordinary AI unit (`Controller::Ai`, the bot brain drives
//! it) with [`crate::unit::Unit::proxy_human`] set. Everywhere a rule asks
//! "is this a human?" about the unit as a TARGET or as a team member it asks
//! [`crate::unit::Unit::is_human`], which counts the proxy:
//! - target choice: the tier's pressure weight on a human target
//!   (`perception::choose_target_with`);
//! - card TOKN's hit tokens (`ai::tokn`): only a token holder's shots at him
//!   may hit, the rest aim near misses;
//! - the aim error against him: BF2's whole error against a human, 0.3 of it
//!   against a bot (`gunner_foot::error_vector`), and his error block updated
//!   every tick (`Sim::aim_blocks`);
//! - a bot's friendly fire not hurting him (`resolve_bolt`, `blast_unit`);
//! - the bleed handicap's "team with a human" (`Sim::apply_team_handicap`);
//! - card DTEAM's per-team human count: [`humans_on`].
//!
//! What stays a BOT's: the cheats (they test `is_player` only, so a proxy
//! never gets them), spawning (waves, the alive-AI cap, the class draw),
//! seats, and every rule about the unit as a SHOOTER (a bot's headshot gate,
//! a bot's spread multiplier, the AI aim error). It gets no damage buff: its
//! edge over a bot is only its fixed [`Profile`].
//!
//! [ours] The stronger stand-in (2026-10-29, the maintainer: "a little bit
//! worse than how I played"): a proxy may also play by a player's rules
//! where a bot's would hold it back - each a [`Profile`] field:
//! - `hand`: aims and fires as a pad does ([`hand`]): the lead point with a
//!   settling error, followed straight (the assist's rate match), the
//!   trigger held while on him, out to the round's reach, a player's
//!   weapon spread ([`hand_weapons`]);
//! - `headshots`: its head hits count, as a player's do on every tier;
//! - `class`: respawns as one class, as a player picks his;
//! - `hunts`: goes to the nearest post its team does not hold ([`front`]);
//! - `mend`: hurt, breaks off to a health droid ([`mend`]);
//! - `picks`: shoots who shoots at it first ([`pick`]);
//! - `skill` over 20 ([`skill_over`]): the curves carried on (measured; not
//!   in a strength).
//!
//! [ours] PROVISIONAL: `hunts`, `mend` and `picks` also serve the tier
//! columns for ordinary bots. The other player rules remain proxy-only.
//!
//! The profile is a fixed "human" row, the same on every tier (a human does
//! not get better when the host picks Elite; the tier columns the profile
//! does not replace still vary by tier, below): its aim skill, its reaction,
//! what it does when hit, how readily it runs from a grenade, how far it
//! sees and whether it strafes (circles) instead of standing. Two strengths,
//! [`Strength::Average`] (PROVISIONAL) and [`Strength::Good`] (fitted to the
//! maintainer's own matches on 2026-10-29, a little under him; its row says
//! which numbers the fit supports - `scripts/human_profile.py`,
//! `analysis/difficulty-proposal.md` §5).
//!
//! The proxy reads [`FROZEN_TIERS`] and thinks through the separate legacy
//! brain. The asset-free pin `an_all_proxy_skirmish_keeps_its_hash` and
//! ground proxy round goldens check the dispatch and the shared helpers.
//!
//! Tier reads a proxy still takes from live code, not the frozen rows:
//! - a manned building turret's view (`ai::man::hittable`, `Sim::ai_tier`
//!   of the gunner's team);
//! - the level curve under its error cap at a droid or a turret
//!   (`Skill::from_level` in [`frozen_err_cap`]);
//! - when a proxy faces another human (two proxies, or a proxy against a
//!   player): the focus switch and token budget against that human
//!   (`dfocus::on_against`, `tokn::budget`), which follow the proxy's
//!   team's live row;
//! - the round's bleed handicap: a proxy counts as a human, so at a tier
//!   whose live `bleed_handicap` is not 1 (Elite, x1.25) a lone proxy's
//!   team bleeds faster (`Sim::apply_team_handicap`, reading
//!   `Sim::difficulty.tier()`). `FROZEN_TIERS` copies the column, but no
//!   rule reads the copy.

use crate::fmath::FloatExt;
use crate::sim::Sim;
use crate::skill::{Difficulty, Skill, Tier};
use crate::unit::Unit;
use glam::Vec3;

/// The two proxy strengths of `analysis/difficulty-proposal.md` §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    /// "P-avg": a decent player.
    Average,
    /// "P-good": a competent player; the difficulty targets are his.
    Good,
}

impl Strength {
    pub fn label(self) -> &'static str {
        match self {
            Strength::Average => "average",
            Strength::Good => "good",
        }
    }

    /// `average|avg|good`, case-insensitive.
    pub fn parse(s: &str) -> Option<Strength> {
        match s.trim().to_ascii_lowercase().as_str() {
            "average" | "avg" => Some(Strength::Average),
            "good" => Some(Strength::Good),
            _ => None,
        }
    }

    /// This strength's row (ours). Average: PROVISIONAL, the proposal's
    /// defaults with the damage multipliers dropped (a proxy must be fair).
    /// Good: fitted to the maintainer's matches (its comment).
    pub fn profile(self) -> Profile {
        match self {
            Strength::Average => Profile {
                skill: 17,
                reaction_ticks: 9,
                react: [0.30, 0.15, 0.20],
                grenade: 0.60,
                view_mult: 1.25,
                circles: false,
                hand: None,
                headshots: false,
                hunts: false,
                class: None,
                mend: 0.0,
                picks: false,
            },
            // [ours] Fitted 2026-10-29 to the maintainer's two BF1 Yavin 4
            // Arena matches at Normal, set a little under him on purpose
            // (`human_profile.py calibrate`, 16 seeds): kills/min 5.4 of his
            // 8.3, K/D 8.7 of 23.8, accuracy 0.37 of 0.37, life 73 s of 110.
            // Fit-supported: `hand.floor` (the accuracy), `class` 0 (the
            // rifleman he played), `headshots` (his rule; 13 % of his rifle
            // hits were heads). The rest stays PROVISIONAL.
            Strength::Good => Profile {
                skill: 20,
                reaction_ticks: 6,
                react: [0.35, 0.25, 0.30],
                grenade: 0.95,
                view_mult: 2.0,
                circles: true,
                hand: Some(hand::Hand {
                    floor: 0.006,
                    ..hand::Hand::DEFAULT
                }),
                headshots: true,
                hunts: true,
                class: Some(0),
                mend: 0.35,
                picks: true,
            },
        }
    }
}

/// A proxy's fixed "human" row. Every field replaces the tier's own for this
/// unit only. PROVISIONAL.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Profile {
    /// The aim skill (1..=20, the originals' scale): spread, dispersion, the
    /// aim tracker, the error cap, burst and pause ([`Skill::from_level`]),
    /// WITHOUT a tier's override (Elite's 0.9 m cap is a bot's extra).
    pub skill: u8,
    /// Ticks from first seeing an enemy to the first shot at it.
    pub reaction_ticks: u64,
    /// When hit: the chances to strafe, roll, move to a new position (the
    /// rest it stays) - `Tier::react`.
    pub react: [f32; 3],
    /// The chance to run clear of a grenade that lands near.
    pub grenade: f32,
    /// The view distance multiplier (x the map's AI view multiplier).
    pub view_mult: f32,
    /// Fights on the move (the circling style: walks a ring round its
    /// target) instead of the bot's own drawn style; a scoped weapon still
    /// stands.
    pub circles: bool,
    /// [ours] Aims and fires with a human's hand ([`hand`]) instead of the
    /// bot gunner's error, bursts and AI ranges. `None`: the bot gunner.
    pub hand: Option<hand::Hand>,
    /// [ours] Its head hits count as a human's do (the PS2 drops a bot's
    /// below our Hard; a player's always count).
    pub headshots: bool,
    /// [ours] Goes where the fight is ([`front`]): the nearest post its
    /// team does not hold, never a rear post to defend - a player hunting
    /// kills, not the commander's pick.
    pub hunts: bool,
    /// [ours] The class it respawns as (an index into its team's table), as
    /// a player picks his on the spawn screen; `None`: the bots' weighted
    /// draw.
    pub class: Option<u8>,
    /// [ours] Under this share of its health it breaks off to the nearest
    /// health droid ([`mend`]); 0: never.
    pub mend: f32,
    /// [ours] Picks its target as a player does ([`pick`]): the one
    /// shooting at it first, then a hurt one, then the nearest.
    pub picks: bool,
}

impl Profile {
    /// `tier` with this profile's columns in place of the tier's.
    pub fn over(&self, mut tier: Tier, map_view: f32) -> Tier {
        tier.picks = self.picks;
        tier.mend = self.mend;
        tier.hunts = self.hunts;
        tier.reaction_ticks = self.reaction_ticks;
        tier.react = self.react;
        tier.grenade = self.grenade;
        tier.view_mult = self.view_mult * map_view;
        tier
    }

    /// The skill vector: the level's curves, no tier override. [ours] Over
    /// 20 (a proxy only; no tier goes there) the curves run on past their
    /// end ([`skill_over`]).
    pub fn skill(&self) -> Skill {
        skill_over(self.skill)
    }

    /// Feed into the sim hash (only a proxy's unit eats it).
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[0xD7, self.skill, u8::from(self.circles)]);
        if let Some(h) = &self.hand {
            eat(&[0x4A]);
            h.hash_into(eat);
        }
        if self.headshots {
            eat(&[0x4E]);
        }
        if self.hunts {
            eat(&[0x48]);
        }
        if let Some(c) = self.class {
            eat(&[0x43, c]);
        }
        if self.mend > 0.0 {
            eat(&self.mend.hash_bits().to_le_bytes());
        }
        if self.picks {
            eat(&[0x50]);
        }
        eat(&self.reaction_ticks.to_le_bytes());
        for v in self
            .react
            .iter()
            .chain([self.grenade, self.view_mult].iter())
        {
            eat(&v.hash_bits().to_le_bytes());
        }
    }
}

/// [ours] The tier rows a proxy's brain reads (bot rework stage 0, the
/// proxy freeze): a copy of `Difficulty::tier`'s rows as they stood at
/// 39c6f1eb, so tier work on the bots does not change the stand-in human.
/// The columns its [`Profile`] replaces are unused; the rest still reach
/// it: the grenade delay, `pressure`, `scope_sight` (Hard's, `scope_share`),
/// `cover` (the team board's Cover role) and `err_cap` (its error cap at a
/// droid or a turret, [`frozen_err_cap`]). OURS; replace: when the
/// maintainer recalibrates the proxy against a live playtest (rework
/// question 3), with the proxy gate re-run on both sides.
pub const FROZEN_TIERS: [Tier; 4] = [
    // Easy
    Tier {
        view_mult: 1.0,
        reaction_ticks: 9,
        pressure: 0.8,
        err_cap: None,
        bleed_handicap: 1.0,
        react: [0.30, 0.15, 0.20],
        grenade: 0.60,
        grenade_delay: (18, 39),
        cover: false,
        scope_sight: 0.0,
        picks: false,
        mend: 0.0,
        hunts: false,
    },
    // Normal
    Tier {
        view_mult: 1.0,
        reaction_ticks: 9,
        pressure: 1.0,
        err_cap: None,
        bleed_handicap: 1.0,
        react: [0.30, 0.15, 0.20],
        grenade: 0.60,
        grenade_delay: (18, 39),
        cover: false,
        scope_sight: 0.75,
        picks: false,
        mend: 0.0,
        hunts: false,
    },
    // Hard
    Tier {
        view_mult: 1.25,
        reaction_ticks: 6,
        pressure: 1.3,
        err_cap: None,
        bleed_handicap: 1.0,
        react: [0.30, 0.20, 0.30],
        grenade: 0.85,
        grenade_delay: (0, 24),
        cover: true,
        scope_sight: 1.0,
        picks: false,
        mend: 0.35,
        hunts: false,
    },
    // Elite
    Tier {
        view_mult: 1.5,
        reaction_ticks: 5,
        pressure: 1.6,
        err_cap: Some(0.9),
        bleed_handicap: 1.25,
        react: [0.30, 0.25, 0.35],
        grenade: 0.95,
        grenade_delay: (0, 24),
        cover: true,
        scope_sight: 1.0,
        picks: true,
        mend: 0.35,
        hunts: true,
    },
];

/// [`FROZEN_TIERS`]' row for tier `d`.
pub fn frozen_tier(d: Difficulty) -> Tier {
    FROZEN_TIERS[match d {
        Difficulty::Easy => 0,
        Difficulty::Normal => 1,
        Difficulty::Hard => 2,
        Difficulty::Elite => 3,
    }]
}

/// A proxy's error cap at a droid or a turret, on a team at tier `d` with
/// integer skill `level`: the level's own cap, under the frozen row's
/// override (`Difficulty::skill_at` as it stood at the freeze).
pub fn frozen_err_cap(d: Difficulty, level: u8) -> f32 {
    frozen_tier(d)
        .err_cap
        .unwrap_or_else(|| Skill::from_level(level).err_cap)
}

/// Does a proxy on a team at tier `d` focus on humans (card DFOCUS's
/// `focuses` as it stood at the freeze)?
pub fn frozen_focuses(d: Difficulty) -> bool {
    matches!(d, Difficulty::Hard | Difficulty::Elite)
}

/// The highest proxy skill (5 levels past the originals' 20).
pub const SKILL_MAX: u8 = 25;

/// [ours] [`Skill::from_level`] carried past 20 on the same curves (t over
/// 1), with the same floors at zero; the skill's discrete columns (burst,
/// pause, tap, charge, grenade, dodge) stay at 20's. A level up to 20 is the
/// table's own.
pub fn skill_over(level: u8) -> Skill {
    let mut k = Skill::from_level(level);
    if level <= 20 {
        return k;
    }
    let t = f32::from(level.min(SKILL_MAX) - 1) / 19.0;
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    k.spread_mult = lerp(3.0, 1.0).fmax(0.0);
    k.dispersion_mult = lerp(2.0, 0.0).fmax(0.0);
    k.aim_accel = 0.8 + 13.2 * t * t * t;
    k.aim_damping = lerp(0.8, 2.4);
    k.err_decay = lerp(0.1, 1.1);
    k.err_floor = lerp(1.6, 0.4).fmax(0.0);
    k.err_cap = lerp(2.2, 1.2).fmax(0.0);
    k.err_cap_flyer = lerp(4.0, 1.2).fmax(0.0);
    k
}

/// Card DTEAM's hook: how many humans team `team` has, proxies included -
/// each player slot's unit on the team, plus every proxy unit on it (alive or
/// waiting to respawn: it keeps the flag through its deaths).
pub fn humans_on(sim: &Sim, team: u8) -> u8 {
    let players = sim
        .players
        .iter()
        .flatten()
        .filter(|h| {
            sim.units
                .get(h.index as usize)
                .is_some_and(|u| u.team == team && u.proxy_human.is_none())
        })
        .count();
    let proxies = sim
        .units
        .iter()
        .filter(|u| u.team == team && u.proxy_human.is_some())
        .count();
    u8::try_from(players + proxies).unwrap_or(u8::MAX)
}

/// [ours] A proxy with a hand carries a human's weapon state (its spread
/// rests at a player's floor, `WeaponState::human`); every other unit keeps
/// its own. Called when the flag is set and at each respawn.
pub fn hand_weapons(u: &mut Unit) {
    if u.proxy_human.is_some_and(|p| p.hand.is_some()) {
        u.weapon.human = true;
        for w in u.held.iter_mut() {
            w.state.human = true;
        }
    }
}

/// [ours] A hunting unit's objective: the nearest post (straight line)
/// its team does not hold and may take, else the nearest enemy. `None` for
/// every unit that is not a hunter (the commander's plan stands).
pub fn front(sim: &Sim, u: &Unit) -> Option<crate::commander::Objective> {
    if !sim.unit_tier(u).hunts {
        return None;
    }
    let at = u.soldier.pos;
    let post = sim
        .cps
        .iter()
        .enumerate()
        .filter(|(_, c)| c.owner != Some(u.team) && !c.ai_no_capture(u.team))
        .map(|(i, c)| (i, (c.def.pos - at).length_squared()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i as u32);
    Some(post.map_or(crate::commander::Objective::Hunt, |i| {
        crate::commander::Objective::Attack(i)
    }))
}

/// The farthest (m) a mending unit goes for a health droid.
pub const MEND_REACH: f32 = 80.0;
/// The share of its health a mending unit stays at the droid for.
pub const MEND_FULL: f32 = 0.95;

/// [ours] Where a hurt unit goes to heal: the nearest standing health droid
/// within [`MEND_REACH`] while its health is under its tier's `mend`
/// share, and on until [`MEND_FULL`] once it is at the droid. `None` for
/// every other unit and moment.
pub fn mend(sim: &Sim, u: &Unit) -> Option<Vec3> {
    mend_station(sim, u).map(|s| s.pos)
}

pub(super) fn mend_station<'a>(sim: &'a Sim, u: &Unit) -> Option<&'a crate::sim::Station> {
    let low = sim.unit_tier(u).mend;
    if low <= 0.0 || u.mount.is_some() || u.health.max <= 0.0 {
        return None;
    }
    let share = u.health.cur / u.health.max;
    if share >= MEND_FULL {
        return None;
    }
    let at = u.soldier.pos;
    let (station, d2) = sim
        .stations
        .iter()
        .filter(|s| {
            !s.destroyed
                && sim
                    .station_classes
                    .get(usize::from(s.class))
                    .is_some_and(|c| c.health[0] > 0.0)
        })
        .map(|s| (s, (s.pos - at).length_squared()))
        .filter(|&(_, d2)| d2 < MEND_REACH * MEND_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let there = d2 < (station.radius + 1.0) * (station.radius + 1.0);
    (share < low || there).then_some(station)
}

/// [`pick`]'s weights, in metres of range: an enemy aiming at the unit
/// counts as this much nearer...
pub const PICK_THREAT: f32 = 15.0;
/// ...one it has hurt (under 70 % health) this much...
pub const PICK_HURT: f32 = 10.0;
/// ...and the target it already has this much (no flicking between two).
pub const PICK_KEEP: f32 = 5.0;

/// [ours] A picking unit's target out of the enemies it SEES now: the
/// nearest after [`PICK_THREAT`], [`PICK_HURT`] and [`PICK_KEEP`]. `None`
/// for every other unit, or with nobody seen.
pub fn pick(
    sim: &Sim,
    me: crate::unit::UnitHandle,
    st: &crate::ai::state::AiState,
) -> Option<crate::unit::UnitHandle> {
    let u = sim.unit(me)?;
    if !sim.unit_tier(u).picks {
        return None;
    }
    let eye = u.capsule().center();
    st.contacts
        .iter()
        .flatten()
        .filter(|c| c.seen_now && crate::perception::target_admitted(sim, me, c))
        .filter_map(|c| {
            let h = c.handle.unit()?;
            let e = sim.unit(h).filter(|e| e.team != u.team)?;
            let mut score = (e.capsule().center() - eye).length();
            if e.ai.target == Some(me.into()) {
                score -= PICK_THREAT;
            }
            if e.health.cur < 0.7 * e.health.max {
                score -= PICK_HURT;
            }
            if st.target == Some(h.into()) {
                score -= PICK_KEEP;
            }
            Some((h, score))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| h)
}

impl Sim {
    /// The tier columns unit `u`'s brain works to: its team's
    /// ([`Sim::ai_tier`]), or a proxy's own profile over them.
    pub fn unit_tier(&self, u: &Unit) -> Tier {
        match &u.proxy_human {
            // The frozen row (`FROZEN_TIERS`), its view under the mission's
            // as `Sim::ai_tier` does, then the profile over it.
            Some(p) => {
                let mut tier = frozen_tier(self.difficulty_of(u.team));
                tier.view_mult *= self.mission_ai.view_mult;
                p.over(tier, self.mission_ai.view_mult)
            }
            None => self.ai_tier(u.team),
        }
    }

    /// Unit `u`'s error cap at a droid or a turret: its integer skill's
    /// under its team's tier, or a proxy's frozen one ([`frozen_err_cap`]).
    pub fn aim_err_cap(&self, u: &Unit) -> f32 {
        let d = self.difficulty_of(u.team);
        match u.proxy_human {
            Some(_) => frozen_err_cap(d, u.skill),
            None => d.skill_at(u.skill).err_cap,
        }
    }

    /// Unit `u`'s skill vector: its integer skill under its team's tier, or
    /// a proxy's own level with no tier override.
    pub fn skill_of(&self, u: &Unit) -> Skill {
        match &u.proxy_human {
            Some(p) => p.skill(),
            None => self.difficulty_of(u.team).skill_at(u.skill),
        }
    }

    /// Make the unit `h` a proxy (`None`: an ordinary bot again). It keeps
    /// the flag through its respawns. Returns whether `h` was a live AI.
    pub fn set_proxy(&mut self, h: crate::unit::UnitHandle, profile: Option<Profile>) -> bool {
        match self.unit_mut(h) {
            Some(u) if u.controller == crate::unit::Controller::Ai && !u.is_player => {
                u.proxy_human = profile;
                hand_weapons(u);
                true
            }
            _ => false,
        }
    }
}

/// Card DPRX's difficulty targets: the proxy team's win rate with a GOOD
/// proxy, averaged over both sides of each map (the maintainer, 2026-10-29).
/// `(tier, low, high)` as fractions; `high` 1.0 = no ceiling.
pub const TARGETS: [(Difficulty, f32, f32); 4] = [
    (Difficulty::Easy, 0.90, 1.0),
    (Difficulty::Normal, 0.77, 0.90),
    (Difficulty::Hard, 0.45, 0.60),
    (Difficulty::Elite, 0.25, 0.40),
];

/// The target band of tier `d`.
pub fn target(d: Difficulty) -> (f32, f32) {
    TARGETS
        .iter()
        .find(|(t, _, _)| *t == d)
        .map_or((0.0, 1.0), |&(_, lo, hi)| (lo, hi))
}

/// PASS when `wins` of `rounds` is inside tier `d`'s band.
pub fn passes(d: Difficulty, wins: u32, rounds: u32) -> bool {
    if rounds == 0 {
        return false;
    }
    let (lo, hi) = target(d);
    let r = wins as f32 / rounds as f32;
    r >= lo && r <= hi
}

pub mod hand;

#[cfg(test)]
mod tests;
