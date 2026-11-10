//! BF2's award weapons (`analysis/re-grenades.md` §2.4,
//! `analysis/award-weapons.md`). Every trooper class of both games carries
//! its award slots (`MedalsTypeToUnlock`; a BF1 class BF2's, from
//! `game::roster::awards`); a slot joins the channel-0 switch cycle only when
//! the unit holds that medal, and then the weapon it replaces
//! (`MedalsTypeToLock`) leaves the cycle (B2 `FUN_00240bf8` 161138-161154).
//! Medals are not earned yet (issue #130): the session cheat "awards" grants
//! the player all of them, and with them the damage award
//! (`super::buffs::WAR_HERO`).

use super::state::Sim;
use crate::unit::Unit;

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
    /// their kits as they are): "awards" every medal but the damage award,
    /// "damage" that one. Issue #130 adds earned medals here.
    pub fn award_bits(&self, u: &Unit) -> u16 {
        if !u.is_player || self.space_round() {
            return 0;
        }
        let war_hero = 1u16 << crate::sim::buffs::WAR_HERO_MEDAL;
        let mut bits = 0;
        if self.cheats.all_awards {
            bits |= !war_hero;
        }
        if self.cheats.damage_award {
            bits |= war_hero;
        }
        bits
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
