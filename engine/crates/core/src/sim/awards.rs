//! BF2's award weapons (`analysis/re-grenades.md` §2.4,
//! `analysis/award-weapons.md`). Every trooper class of both games carries
//! its award slots (`MedalsTypeToUnlock`; a BF1 class BF2's, from
//! `game::roster::awards`); a slot joins the channel-0 switch cycle only when
//! the unit holds that medal, and then the weapon it replaces
//! (`MedalsTypeToLock`) leaves the cycle (B2 `FUN_00240bf8` 161138-161154).
//! Medals are not earned yet (issue #130): the session cheat "awards" grants
//! the player the weapon medals, and one cheat each the four stat medals
//! (`Sim::medal_effect`).

use super::state::Sim;
use crate::unit::Unit;

/// The medals that unlock a weapon: Gunslinger 0, Frenzy 1, Demolition 2,
/// Marksman 4, Regulator 5 (`analysis/re-hitzones.md` Part B section 3).
pub const WEAPON_MEDALS: u16 = 1 << 0 | 1 << 1 | 1 << 2 | 1 << 4 | 1 << 5;

/// BF2's vehicle award (medal 3, "Technician").
pub const TECHNICIAN_MEDAL: u16 = 3;

/// BF2's energy award (medal 6, "Endurance").
pub const ENDURANCE_MEDAL: u16 = 6;

/// Endurance's factor on every energy refill of its holder and the vehicle
/// he drives (the f32 at B2 `DAT_005feda0`; B2:160231 the soldier, 101661,
/// 109523, 116988, 133701 and 178209 the vehicles). Never on a drain.
pub const ENDURANCE: f32 = 1.5;

/// A refill rate under Endurance: x[`ENDURANCE`] when it refills, as it
/// was when it drains.
pub fn endurance_rate(rate: f32) -> f32 {
    if rate > 0.0 {
        rate * ENDURANCE
    } else {
        rate
    }
}

/// A vehicle's repair a second, as a share of its maximum health, while a
/// Technician or a BF2 pilot drives it, for a class that authors no
/// `PilotSkillRepairScale` (B2 class+0x2fc).
pub const DEFAULT_REPAIR_SCALE: f32 = 0.005;

/// The bit of medal type `t`, 0 for a value outside the 16 bits.
fn bit(t: i8) -> u16 {
    u32::try_from(t)
        .ok()
        .and_then(|t| 1u16.checked_shl(t))
        .unwrap_or(0)
}

impl Sim {
    /// The medals unit `u` holds, one bit per medal type. Only the session
    /// cheats set any, for a player, outside the space modes (which keep
    /// their kits as they are): "awards" the weapon medals, and "damage",
    /// "guardian", "endurance" and "technician" one stat medal each. Bots
    /// hold none, so no difficulty depends on them. Issue #130 adds earned
    /// medals here.
    pub fn award_bits(&self, u: &Unit) -> u16 {
        if !u.is_player || self.space_round() {
            return 0;
        }
        let c = &self.cheats;
        let mut bits = 0;
        if c.all_awards {
            bits |= WEAPON_MEDALS;
        }
        for (on, medal) in [
            (c.damage_award, super::buffs::WAR_HERO_MEDAL),
            (c.defense_award, super::buffs::GUARDIAN_MEDAL),
            (c.energy_award, ENDURANCE_MEDAL),
            (c.vehicle_award, TECHNICIAN_MEDAL),
        ] {
            if on {
                bits |= 1 << medal;
            }
        }
        bits
    }

    /// Does `u` have the effect of stat medal `medal` (War Hero, Guardian,
    /// Endurance or Technician)? Never for a hero: the PS2 clears a hero's
    /// War Hero and Guardian (B2 `FUN_0024ece0`), and `[ours]` Endurance and
    /// Technician too, which the original gives a hero (B2:160231). The
    /// PS2 tests a class flag, so every Hero Assault class counts.
    pub fn medal_effect(&self, u: &Unit, medal: u16) -> bool {
        self.award_bits(u) & (1 << medal) != 0 && !self.hero_unit(u)
    }

    /// Does the driver of vehicle slot `i` (its pilot seat's rider) have the
    /// effect of stat medal `medal`? The PS2 asks the vehicle's controller.
    pub(super) fn driver_medal(&self, i: usize, medal: u16) -> bool {
        self.vehicles
            .get(i)
            .and_then(|v| v.seats.first())
            .filter(|s| s.kind == crate::vehicle::SeatKind::Pilot)
            .and_then(|s| s.occupant)
            .and_then(|o| self.unit(o))
            .is_some_and(|u| self.medal_effect(u, medal))
    }

    /// Is weapon class `class` out of `u`'s cycle? An award weapon is until
    /// `u` holds its medal; a weapon an award replaces is once `u` holds that
    /// medal. `[ours]` A lock applies only while `u` carries the award that
    /// replaces the weapon: BF2's Han pistol inherits the pistol's lock with
    /// no award pistol to take its place, so BF2's rule would leave him none.
    /// On every BF2 trooper class the result is BF2's (each lockable weapon
    /// has its award on the class).
    pub fn weapon_locked(&self, u: &Unit, class: usize) -> bool {
        let Some(wc) = self.weapon_classes.get(class) else {
            return false;
        };
        let bits = self.award_bits(u);
        if wc.unlock_medal().is_some_and(|t| bits & bit(t) == 0) {
            return true;
        }
        let Some(t) = wc.medal_lock.filter(|&t| bits & bit(t) != 0) else {
            return false;
        };
        std::iter::once(u.weapon_class)
            .chain(u.held.iter().map(|w| w.class))
            .any(|c| {
                self.weapon_classes
                    .get(c)
                    .is_some_and(|c| c.unlock_medal() == Some(t))
            })
    }

    /// The weapon in unit `i`'s hands left the cycle (the medals changed):
    /// take its stand-in, the award that replaces it or the weapon an award
    /// replaced, else the next slot. `[ours]`: the PS2 only filters the
    /// switch; this runs on an idle tick, like a switch.
    pub(super) fn award_fixup(&mut self, i: usize) {
        let u = &self.units[i];
        if !self.weapon_locked(u, u.weapon_class)
            || self.controlled_guided(self.handle(i)).is_some()
        {
            return;
        }
        let hand = &self.weapon_classes[u.weapon_class];
        let (unlock, lock) = (hand.unlock_medal(), hand.medal_lock);
        let twin = u.held.iter().position(|w| {
            let c = &self.weapon_classes[w.class];
            w.channel == 0
                && !self.weapon_locked(u, w.class)
                && ((lock.is_some() && c.unlock_medal() == lock)
                    || (unlock.is_some() && c.medal_lock == unlock))
        });
        match twin {
            Some(k) => self.take_channel0(i, k),
            None => self.switch_channel0(i),
        }
    }
}

#[cfg(test)]
mod tests;
