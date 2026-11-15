//! Issue #173, the maintainer's rule (2026-11-14) [ours]: a soldier who dies
//! from a fall after an attacker choked, pushed, pulled, shoved or blasted him
//! is that attacker's kill.
//!
//! - **What sets it.** A Force Choke's hold (refreshed while it holds, and
//!   restamped on the release), and a Force Push or Pull, a saber shove, an
//!   emitter's shove or a blast that actually throws him (he is knocked, or
//!   he was already in the air) record a [`ForcedMove`] on the victim
//!   ([`Unit::forced_by`]). A nudge that leaves him standing records
//!   nothing. The victim himself never does: a soldier thrown by his own
//!   grenade dies the environment's death as before.
//! - **Who counts.** The credit follows the damage path's friendly-fire
//!   rule ([`Sim::forced_move_counts`]): an enemy always; a friend only where
//!   his own direct damage would have hurt the victim (friendly fire on,
//!   and never a bot's throw of a human of its own team, whom the bot's
//!   blasts and Force powers already spare). A friend's credited fall is a
//!   team kill.
//! - **What it credits.** A death by [`DeathCause::Fall`], the kill floor, a
//!   mission death region or a landing under the map while it is set goes
//!   through the kill credit ([`Event::Kill`], the attacker's kill count, team
//!   kills, a hero's kill heal), followed by an [`Event::ForcedDeath`] that
//!   keeps the death's cause for the black box. Water and damage regions wear
//!   a soldier down over seconds, so they stay the world's.
//! - **When it ends.** Once he has been back in control for
//!   [`FORCED_GRACE`] (on his feet, or flying his own jet pack, and not
//!   thrown, tumbling, getting up or held), or [`FORCED_MAX`] after it was
//!   set; also when he boards a vehicle, dies, or the attacker's life it
//!   names is gone (a respawn or a leaver: a new generation in the slot).
//!
//! The originals credit neither case: their landing damage goes through the
//! ordinary damage call with no attacker (B2 `FUN_0023cfd0`), and a death
//! region or the kill floor kills with no killer (`analysis/re-kill-floor.md`
//! 1e). This is the maintainer's rule, not a port of one.
//!
//! [`Unit::forced_by`]: crate::unit::Unit::forced_by
//! [`Event::Kill`]: super::Event::Kill
//! [`Event::ForcedDeath`]: super::Event::ForcedDeath

use super::{DeathCause, Sim};
use crate::soldier::Stance;
use crate::unit::UnitHandle;
use crate::TICK_HZ;

/// How long a soldier must be back in control before a forced move stops
/// counting, from the first tick he is steady again. Half a second covers a
/// throw that lands him at an edge he then slides over. [ours] PROVISIONAL,
/// measure: the longest grounded slide before a fall in pushed-off-a-ledge
/// captures.
pub const FORCED_GRACE: f32 = 0.5;

/// The longest a forced move counts, from the move (a choke's from its
/// release). [ours] PROVISIONAL: a 3 s choke hold is restamped at the
/// release, and a fall from 40 m under the port's gravity (18 m/s², no drag)
/// takes about 2.1 s, so ten seconds leaves room for a throw, a tumble and a
/// slide before the drop. measure: the longest time from a forced move to
/// its fall death on the maps' deepest drops (a fall-credit census).
pub const FORCED_MAX: f32 = 10.0;

const GRACE_TICKS: u64 = (FORCED_GRACE * TICK_HZ as f32) as u64;
const MAX_TICKS: u64 = (FORCED_MAX * TICK_HZ as f32) as u64;

/// A forced move on a unit ([`crate::unit::Unit::forced_by`]): who made it,
/// when (a choke's release restamps it), and since when the victim has been
/// steadily back in control (`None` while he is not).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForcedMove {
    pub by: UnitHandle,
    pub at: u64,
    pub steady_since: Option<u64>,
}

impl ForcedMove {
    pub fn new(by: UnitHandle, at: u64) -> Self {
        Self {
            by,
            at,
            steady_since: None,
        }
    }
}

impl Sim {
    /// Whether a forced move by `attacker` on `victim` can count: the damage
    /// path's friendly-fire rule. An enemy always; a friend only with
    /// friendly fire on, and never a bot's (a non-player's) move on a human
    /// of its own team, which blasts, Force powers and burns already spare
    /// (`Sim::blast_unit`, `Sim::force_damage`, `Sim::tick_burns`). A stale
    /// attacker handle never counts.
    pub(super) fn forced_move_counts(&self, attacker: UnitHandle, victim: UnitHandle) -> bool {
        let (Some(a), Some(v)) = (self.unit(attacker), self.unit(victim)) else {
            return false;
        };
        if !self.allied(a.team, v.team) {
            return true;
        }
        self.friendly_fire_scale > 0.0 && !(v.is_human() && !a.is_player)
    }

    /// Record that `attacker` moved `victim` against his will (a choke, a
    /// push or pull, a saber or emitter shove, a blast's throw). Nothing for
    /// the victim's own blast, a placed gun with no soldier, a dead victim,
    /// a stale handle, an attacker the friendly-fire rule rules out
    /// ([`Sim::forced_move_counts`]), or a move that left him standing (not
    /// knocked, not in the air, not held).
    pub(super) fn note_forced_move(&mut self, victim: UnitHandle, attacker: Option<UnitHandle>) {
        let Some(attacker) = attacker.filter(|a| a.index != u32::MAX && *a != victim) else {
            return;
        };
        if !self.forced_move_counts(attacker, victim) {
            return;
        }
        let tick = self.tick;
        if let Some(u) = self
            .units
            .get_mut(victim.index as usize)
            .filter(|u| u.gen == victim.gen && u.alive)
        {
            let s = &u.soldier;
            let displaced = s.stance.knocked() || !s.grounded || u.choked.is_some();
            if displaced {
                u.forced_by = Some(ForcedMove::new(attacker, tick));
            }
        }
    }

    /// Restamp a forced move that is already recorded (a choke's release:
    /// the window counts from the drop, not from the catch).
    pub(super) fn restamp_forced_move(&mut self, i: usize) {
        let tick = self.tick;
        if let Some(f) = self.units[i].forced_by.as_mut() {
            f.at = tick;
            f.steady_since = None;
        }
    }

    /// The attacker a death by `cause` of unit `h` is credited to, if any:
    /// a fall-type death inside the window, by an attacker whose life is
    /// still the one recorded (the core and the game layer read the same
    /// unit).
    pub(super) fn forced_attacker(&self, h: UnitHandle, cause: DeathCause) -> Option<UnitHandle> {
        if !matches!(
            cause,
            DeathCause::Fall
                | DeathCause::KillFloor
                | DeathCause::DeathRegion
                | DeathCause::UnderMap
        ) {
            return None;
        }
        let u = self.unit(h)?;
        let f = u.forced_by?;
        let current = self
            .units
            .get(f.by.index as usize)
            .is_some_and(|a| a.gen == f.by.gen);
        (self.tick.saturating_sub(f.at) <= MAX_TICKS && f.by != h && current).then_some(f.by)
    }

    /// Drop the forced moves that no longer count: he has been back in
    /// control for [`FORCED_GRACE`], [`FORCED_MAX`] has passed, he rides a
    /// vehicle, or the attacker's life is gone. Runs after the tick's fall,
    /// kill-floor and death-region checks.
    pub(super) fn expire_forced_moves(&mut self) {
        let now = self.tick;
        for i in 0..self.units.len() {
            let Some(mut f) = self.units[i].forced_by else {
                continue;
            };
            let attacker_gone = self
                .units
                .get(f.by.index as usize)
                .is_none_or(|a| a.gen != f.by.gen);
            let u = &self.units[i];
            let s = &u.soldier;
            let held = u.choked.is_some();
            let on_his_feet = s.grounded && !s.stance.knocked() && !s.stance.airborne();
            let flying_himself = matches!(s.stance, Stance::Jet | Stance::Hover);
            let steady = !held && (on_his_feet || flying_himself);
            let drop = !u.alive
                || u.mount.is_some()
                || attacker_gone
                || now.saturating_sub(f.at) > MAX_TICKS;
            if drop {
                self.units[i].forced_by = None;
                continue;
            }
            if steady {
                let since = *f.steady_since.get_or_insert(now);
                if now.saturating_sub(since) >= GRACE_TICKS {
                    self.units[i].forced_by = None;
                    continue;
                }
            } else {
                f.steady_since = None;
            }
            self.units[i].forced_by = Some(f);
        }
    }
}
