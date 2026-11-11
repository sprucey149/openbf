//! Card DFOCUS (D25, OURS): focus fire on HUMAN players at Hard and Elite.
//! The holders of a human's hit tokens (`super::tokn`) are his focusers: at
//! a tier that focuses ([`focuses`]) the deal also offers his tokens to the
//! enemy bots that SEE him ([`may_focus`]) and not only to those already
//! shooting at him, and a holder makes him its target ([`pick`]). So up to
//! the tier's token count of bots converge on him (Hard 5, Elite 7), and the
//! deal, once a tick in the sim's order, decides who - not every bullet.
//!
//! The rules (the user's, 2026-10-28; the values are PROVISIONAL):
//! - Only a bot whose own eyes see him at its last think: no wallhack, no
//!   map-wide hunt. A friend's word (`perception::tell`) or a shot heard
//!   ([`crate::perception::HEAR_RADIUS`], 30 m) makes a record and turns the
//!   bot, but it takes his token only once it sees him.
//! - Within the bot's reach ([`reach`]): [`FOOT_RANGE`] on foot,
//!   [`SCOPED_RANGE`] with a scoped rifle; against a human in a vehicle or
//!   turret [`VEHICLE_RANGE`] for a bot with a weapon that hurts that hull,
//!   never for one without. A bot with only heavy weapons does not focus a
//!   soldier. Bots in seats are never focusers (the tokens govern foot bots).
//! - After losing sight a focuser keeps him [`MEMORY`] - aiming at his last
//!   seen point, and walking to it when it is within [`PUSH`] ([`push`]) -
//!   then lets go.
//! - A bot capturing a post, or defending one under threat, keeps its job
//!   ([`on_post`]).
//! - No focus on a human in his first [`GRACE`] after he spawns.
//! - Several humans: each has his own tokens; a bot focuses the best of
//!   those whose token it holds (seen first, then the nearest).
//! - Focusers fight from spread angles: the Tactician's spaced try counts a
//!   position within [`SPREAD_COS`] of a fellow focuser's bearing from the
//!   human as crowded (`tactic::choose`).
//! - A friend in the line of fire: card FFLN's rule stands for focusers as
//!   for every bot (the shot waits, the holder steps aside).

use crate::commander::{Objective, DEFEND_QUIET_FALL};
use crate::perception::{contact_of, Contact};
use crate::sim::Sim;
use crate::skill::Difficulty;
use crate::unit::{Unit, UnitHandle};
use glam::Vec3;

const S: u64 = crate::TICK_HZ as u64;

/// A focuser's reach on foot against a human on foot (m). PROVISIONAL.
pub const FOOT_RANGE: f32 = 45.0;
/// ...with a scoped rifle (m). PROVISIONAL.
pub const SCOPED_RANGE: f32 = 120.0;
/// ...against a human in a vehicle or turret, with a weapon that hurts it
/// (m). PROVISIONAL.
pub const VEHICLE_RANGE: f32 = 90.0;
/// A focuser keeps a human it lost sight of this long (ticks). The contact
/// memory is as long (`perception::MEMORY_TICKS`). PROVISIONAL.
pub const MEMORY: u64 = 3 * S;
/// ...and walks to his last seen point when it is this near (m, XZ).
/// PROVISIONAL.
pub const PUSH: f32 = 25.0;
/// No focus on a human this long after he spawns (ticks). PROVISIONAL.
pub const GRACE: u64 = 3 * S;
/// Two focusers' bearings from the human closer than this cosine (30
/// degrees) are one approach. PROVISIONAL.
pub const SPREAD_COS: f32 = 0.866;

/// Does a team at tier `d` focus on humans?
pub fn focuses(d: Difficulty) -> bool {
    matches!(d, Difficulty::Hard | Difficulty::Elite)
}

/// Does unit `u` focus on the humans whose tokens it holds: its team's
/// tier, or a proxy's frozen rule (`ai::proxy::frozen_focuses`)?
pub(crate) fn focuses_as(sim: &Sim, u: &crate::unit::Unit) -> bool {
    let d = sim.difficulty_of(u.team);
    match u.proxy_human {
        Some(_) => super::proxy::frozen_focuses(d),
        None => focuses(d),
    }
}

/// When a living human was first seen alive while focus was on (his spawn:
/// a respawn is a new handle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Born {
    pub human: UnitHandle,
    pub tick: u64,
}

impl Born {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.human.index.to_le_bytes());
        eat(&self.human.gen.to_le_bytes());
        eat(&self.tick.to_le_bytes());
    }
}

/// Does the enemy of a human of team `team` focus on him?
fn on_against(sim: &Sim, team: u8) -> bool {
    focuses(sim.difficulty_of(1 - team.min(1)))
}

/// Once a tick, before the deal: the living humans, and when each was first
/// seen alive (the focus grace and [`just_spawned`] read it).
pub fn upkeep(sim: &mut Sim) {
    let tick = sim.tick;
    let mut born = std::mem::take(&mut sim.focus_born);
    born.retain(|b| sim.unit(b.human).is_some());
    for (h, u) in sim.alive_units() {
        if u.is_human() && !born.iter().any(|b| b.human == h) {
            born.push(Born { human: h, tick });
        }
    }
    sim.focus_born = born;
}

/// Is `human` inside his spawn grace?
fn in_grace(sim: &Sim, human: UnitHandle) -> bool {
    alive_under(sim, human, GRACE)
}

/// `[ours]` No bot takes a human as its target this long after he spawns
/// (ticks; the maintainer 2026-10-29).
pub const UNTARGETED: u64 = S;

/// Is `human` a human the upkeep saw spawn under [`UNTARGETED`] ago? (The
/// upkeep runs every tick before the thinks, so a spawn is recorded on his
/// first tick alive.)
pub fn just_spawned(sim: &Sim, human: UnitHandle) -> bool {
    sim.unit(human).is_some_and(Unit::is_human)
        && sim
            .focus_born
            .iter()
            .any(|b| b.human == human && sim.tick < b.tick + UNTARGETED)
}

/// Has `human` been alive under `ticks` (no record yet: he spawned this tick)?
fn alive_under(sim: &Sim, human: UnitHandle, ticks: u64) -> bool {
    sim.focus_born
        .iter()
        .find(|b| b.human == human)
        .is_none_or(|b| sim.tick < b.tick + ticks)
}

/// How far bot `b` may focus human `h` from (m), or `None`: none of its
/// weapons is for him.
pub fn reach(sim: &Sim, b: &Unit, h: &Unit) -> Option<f32> {
    match super::arsenal::against_ride(sim, b, h) {
        Some((_, any)) => any.then_some(VEHICLE_RANGE),
        None if !super::arsenal::arms_for_soldier(sim, b) => None,
        None if super::snipe::is_sniper(sim, b) => Some(SCOPED_RANGE),
        None => Some(FOOT_RANGE),
    }
}

/// Is bot `me` on a post's job it keeps: capturing (in the capture region
/// of its objective's post, not its team's), or defending a post its team
/// saw threatened in the last [`DEFEND_QUIET_FALL`] (its objective's, or the
/// post of its Cover role)?
pub fn on_post(sim: &Sim, me: UnitHandle, u: &Unit) -> bool {
    let team = usize::from(u.team).min(1);
    let plan = &sim.commander[team];
    let threatened = |cp: usize| {
        plan.threat
            .get(cp)
            .is_some_and(|&t| sim.tick < t.saturating_add(DEFEND_QUIET_FALL))
    };
    match plan.of(me.index) {
        Some(Objective::Attack(cp) | Objective::Defend(cp))
            if sim
                .cps
                .get(cp as usize)
                .is_some_and(|p| p.owner != Some(u.team))
                && sim.counts_in_cp(me.index as usize, cp as usize) =>
        {
            return true;
        }
        Some(Objective::Defend(cp)) if threatened(cp as usize) => return true,
        _ => {}
    }
    u.ai.role.post.is_some_and(|p| threatened(usize::from(p)))
}

/// `me`'s own record of `h` (not a friend's word), seen or remembered
/// within [`MEMORY`].
fn own(st: &crate::ai::AiState, h: UnitHandle, tick: u64) -> Option<Contact> {
    contact_of(st, h).filter(|c| !c.told && tick.saturating_sub(c.last_seen) <= MEMORY)
}

/// May `bot` take one of `human`'s tokens as a focuser: focus on against
/// him, out of his spawn grace, the bot sees him with its own eyes, he is
/// within its [`reach`] and it is not on a post's job. (The deal asks the
/// line from its muzzle too, and that it holds no other human's token.)
pub fn may_focus(sim: &Sim, bot: UnitHandle, human: UnitHandle) -> bool {
    let (Some(b), Some(h)) = (sim.unit(bot), sim.unit(human)) else {
        return false;
    };
    on_against(sim, h.team)
        && !in_grace(sim, human)
        && contact_of(&b.ai, human).is_some_and(|c| c.seen_now && !c.heard)
        && reach(sim, b, h).is_some_and(|r| (h.capsule().center() - b.muzzle()).length() <= r)
        && !on_post(sim, bot, b)
}

/// Does focuser `bot` keep `human`'s token while it does not see him: focus
/// on, its target, its own record of him younger than [`MEMORY`], his last
/// seen point within its reach.
pub fn remembers(sim: &Sim, bot: UnitHandle, human: UnitHandle) -> bool {
    let (Some(b), Some(h)) = (sim.unit(bot), sim.unit(human)) else {
        return false;
    };
    on_against(sim, h.team)
        && b.ai.target == Some(human.into())
        && own(&b.ai, human, sim.tick).is_some_and(|c| {
            reach(sim, b, h).is_some_and(|r| (c.last_pos - b.muzzle()).length() <= r)
        })
}

/// The human bot `me` focuses this think, if any: of the humans whose
/// token it holds at a focusing tier, those it has its own record of within
/// [`MEMORY`] - seen first, then the nearest, then the lowest index. `None`
/// for a bot on a post's job or in a seat.
pub fn pick(sim: &Sim, me: UnitHandle, st: &crate::ai::AiState) -> Option<UnitHandle> {
    let u = sim.unit(me)?;
    if u.mount.is_some() || !focuses_as(sim, u) || on_post(sim, me, u) {
        return None;
    }
    let eye = u.muzzle();
    sim.tokens
        .iter()
        .filter(|t| t.bot == me)
        .filter_map(|t| own(st, t.human, sim.tick).map(|c| (t.human, c)))
        .filter(|(_, c)| crate::perception::target_admitted(sim, me, c))
        .min_by(|a, b| {
            (!a.1.seen_now)
                .cmp(&!b.1.seen_now)
                .then(
                    (a.1.last_pos - eye)
                        .length()
                        .total_cmp(&(b.1.last_pos - eye).length()),
                )
                .then(a.0.index.cmp(&b.0.index))
        })
        .map(|(h, _)| h)
}

/// Where a focuser that lost sight of its target walks: his last seen
/// point, when within [`PUSH`] of `pos`.
pub fn push(sim: &Sim, me: UnitHandle, st: &crate::ai::AiState, pos: Vec3) -> Option<Vec3> {
    let h = st.target?.unit()?;
    if pick(sim, me, st) != Some(h) {
        return None;
    }
    own(st, h, sim.tick)
        .filter(|c| !c.seen_now && crate::nav::xz_dist(c.last_pos, pos) <= PUSH)
        .map(|c| c.last_pos)
}

/// The bearings (XZ, unit) from `human` at `at` of `me`'s fellow focusers
/// on him - where each stands, or the position it holds - for the
/// Tactician's spread ([`SPREAD_COS`]). Empty unless `me` focuses him.
pub fn fellows(sim: &Sim, me: UnitHandle, human: UnitHandle, at: Vec3) -> Vec<glam::Vec2> {
    let Some(u) = sim.unit(me) else {
        return Vec::new();
    };
    if !focuses_as(sim, u) || !super::tokn::holds(sim, me, human) {
        return Vec::new();
    }
    sim.tokens
        .iter()
        .filter(|t| t.human == human && t.bot != me)
        .filter_map(|t| {
            let o = sim.unit(t.bot)?;
            let p =
                o.ai.tpos
                    .held
                    .and_then(|i| sim.positions.positions.get(i as usize))
                    .map_or(o.soldier.pos, |q| q.pos);
            let d = glam::Vec2::new(p.x - at.x, p.z - at.z);
            (d.length_squared() > 1e-4).then(|| d.normalize())
        })
        .collect()
}

#[cfg(test)]
mod tests;
