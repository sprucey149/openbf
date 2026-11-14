//! BF2's award weapons (`analysis/re-grenades.md` §2.4,
//! `analysis/award-weapons.md`). Every trooper class of both games carries
//! its award slots (`MedalsTypeToUnlock`; a BF1 class BF2's, from
//! `game::roster::awards`); a slot joins the channel-0 switch cycle only when
//! the unit holds that medal, and then the weapon it replaces
//! (`MedalsTypeToLock`) leaves the cycle (B2 `FUN_00240bf8` 161138-161154).
//! The local player's earned medals follow BF2's thresholds and career tiers.

use super::state::Sim;
use crate::unit::{Unit, UnitHandle};
use crate::weapon::HealthType;
use crate::TICK_HZ;

/// BF2 medal IDs in simulation order (B2 medal table at 0x62c560).
pub const MEDAL_THRESHOLDS: [u32; 9] = [6, 12, 4, 1, 6, 8, 12, 24, 36];
pub type MedalCounts = [u32; MEDAL_THRESHOLDS.len()];

/// An award still carries through a death at exactly 30 seconds from its
/// activation (BF2 FUN_00360028, B2:357312–357320).
const AWARD_DEATH_WINDOW: u64 = 30 * TICK_HZ as u64;

/// Green, Veteran, Elite and Legendary (B2 FUN_0035fc28).
pub fn medal_level(count: u32) -> usize {
    match count {
        0..=3 => 0,
        4..=31 => 1,
        32..=63 => 2,
        _ => 3,
    }
}

pub fn medal_threshold(medal: usize, count: u32) -> u32 {
    let base = MEDAL_THRESHOLDS[medal];
    if medal_level(count) >= 2 {
        (base * 3 / 4).max(1)
    } else {
        base
    }
}

#[derive(Clone)]
pub(super) struct MedalCareer {
    life: UnitHandle,
    counts: MedalCounts,
    progress: MedalCounts,
    earned: u16,
    activated_at: [Option<u64>; MEDAL_THRESHOLDS.len()],
    carry_on_respawn: u16,
    preserve_on_respawn: bool,
    bonuses: bool,
}

impl MedalCareer {
    fn add(&mut self, life: UnitHandle, medal: usize, amount: u32, tick: u64) {
        if self.life != life {
            self.life = life;
            self.progress.fill(0);
            self.earned = 0;
            self.activated_at.fill(None);
        }
        let bit = 1 << medal;
        if self.earned & bit != 0 {
            return;
        }
        let threshold = medal_threshold(medal, self.counts[medal]);
        self.progress[medal] = (self.progress[medal] + amount.min(threshold)).min(threshold);
        if self.progress[medal] == threshold {
            self.counts[medal] = self.counts[medal].saturating_add(1);
            self.earned |= bit;
            if self.bonuses && (4..64).contains(&self.counts[medal]) {
                self.activated_at[medal] = Some(tick);
            }
        }
    }

    fn bits(&self, gen: u32) -> u16 {
        if !self.bonuses {
            return 0;
        }
        self.counts.iter().enumerate().fold(0, |bits, (i, &count)| {
            let bit = 1 << i;
            if count >= 64 || (count >= 4 && self.life.gen == gen && self.earned & bit != 0) {
                bits | bit
            } else {
                bits
            }
        })
    }

    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"MEDAL_CAREER");
        eat(&self.life.index.to_le_bytes());
        eat(&self.life.gen.to_le_bytes());
        eat(&self.earned.to_le_bytes());
        for n in self.counts.into_iter().chain(self.progress) {
            eat(&n.to_le_bytes());
        }
        for (medal, tick) in self.activated_at.into_iter().enumerate() {
            if let Some(tick) = tick {
                eat(b"MEDAL_ACTIVATED");
                eat(&[medal as u8]);
                eat(&tick.to_le_bytes());
            }
        }
        if self.carry_on_respawn != 0 {
            eat(b"MEDAL_DEATH_CARRY");
            eat(&self.carry_on_respawn.to_le_bytes());
        }
        if self.preserve_on_respawn {
            eat(b"MEDAL_HERO_TRANSITION");
        }
        if !self.bonuses {
            eat(b"MEDAL_BONUSES_OFF");
        }
    }
}

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
    /// Initialize a player's career from the saved profile: the career of
    /// the player slot `life` holds (slot 0 for a unit without one, the
    /// single player's).
    pub fn install_medal_career(&mut self, life: UnitHandle, counts: MedalCounts) {
        let slot = self
            .units
            .get(life.index as usize)
            .and_then(|u| u.spec.player_slot)
            .map_or(0, usize::from);
        self.install_slot_medal_career(slot, life, counts);
    }

    /// Issue #168: initialize player slot `slot`'s career, whose life is
    /// `life`, from that player's profile (LAN §4.1). Nothing in a space
    /// round, as for the single player.
    pub fn install_slot_medal_career(
        &mut self,
        slot: usize,
        life: UnitHandle,
        counts: MedalCounts,
    ) {
        if self.space_round() {
            return;
        }
        if self.medal_careers.len() <= slot {
            self.medal_careers.resize_with(slot + 1, || None);
        }
        self.medal_careers[slot] = Some(MedalCareer {
            life,
            counts,
            progress: [0; MEDAL_THRESHOLDS.len()],
            earned: 0,
            activated_at: [None; MEDAL_THRESHOLDS.len()],
            carry_on_respawn: 0,
            preserve_on_respawn: false,
            bonuses: true,
        });
    }

    /// Issue #168: drop player slot `slot`'s career (a player who left).
    pub fn remove_slot_medal_career(&mut self, slot: usize) {
        if let Some(c) = self.medal_careers.get_mut(slot) {
            *c = None;
        }
    }

    fn career(&self, slot: usize) -> Option<&MedalCareer> {
        self.medal_careers.get(slot).and_then(Option::as_ref)
    }

    /// The career of unit `u`'s player slot.
    fn career_of(&self, u: &Unit) -> Option<&MedalCareer> {
        self.career(usize::from(u.spec.player_slot?))
    }

    /// Whether slot 0's earned career medals can grant weapons and stat
    /// bonuses ([`Sim::slot_career_award_bonuses`]).
    pub fn career_award_bonuses(&self) -> bool {
        self.slot_career_award_bonuses(0)
    }

    /// Whether player slot `slot`'s earned career medals can grant weapons
    /// and stat bonuses.
    pub fn slot_career_award_bonuses(&self, slot: usize) -> bool {
        self.career(slot).is_some_and(|c| c.bonuses)
    }

    /// Slot 0's [`Sim::set_slot_career_award_bonuses`].
    pub fn set_career_award_bonuses(&mut self, enabled: bool) {
        self.set_slot_career_award_bonuses(0, enabled);
    }

    /// Apply player slot `slot`'s bonus preference while continuing to
    /// record progress.
    pub fn set_slot_career_award_bonuses(&mut self, slot: usize, enabled: bool) {
        let tick = self.tick;
        let Some(career) = self
            .medal_careers
            .get_mut(slot)
            .and_then(Option::as_mut)
            .filter(|c| c.bonuses != enabled)
        else {
            return;
        };
        career.bonuses = enabled;
        let i = career.life.index as usize;
        let life_gen = career.life.gen;
        if self.units.get(i).is_some_and(|u| u.is_player && u.alive) {
            if enabled && self.units[i].gen == life_gen {
                let career = self.medal_careers[slot].as_mut().unwrap();
                for (medal, activated) in career.activated_at.iter_mut().enumerate() {
                    if career.earned & (1 << medal) != 0 && (4..64).contains(&career.counts[medal])
                    {
                        activated.get_or_insert(tick);
                    }
                }
            }
            self.award_fixup(i);
            self.sync_award_buffs(i);
        }
    }

    /// Slot 0's medal counts ([`Sim::slot_medal_counts`]).
    pub fn medal_counts(&self) -> Option<MedalCounts> {
        self.slot_medal_counts(0)
    }

    /// Player slot `slot`'s medal counts, when it has a career.
    pub fn slot_medal_counts(&self, slot: usize) -> Option<MedalCounts> {
        self.career(slot).map(|career| career.counts)
    }

    /// The careers whose current life is `life`, in slot order.
    fn careers_of_life(&mut self, life: UnitHandle) -> impl Iterator<Item = &mut MedalCareer> {
        self.medal_careers
            .iter_mut()
            .flatten()
            .filter(move |c| c.life == life)
    }

    // A living hero accept changes the body, but does not end the medal life.
    pub(super) fn carry_medals_into_hero(&mut self, life: UnitHandle) {
        for career in self.careers_of_life(life) {
            career.preserve_on_respawn = true;
        }
    }

    pub(super) fn note_medal_death(&mut self, life: UnitHandle) {
        let tick = self.tick;
        for career in self.careers_of_life(life) {
            career.carry_on_respawn = 0;
            if career.bonuses {
                for (medal, activated) in career.activated_at.iter().enumerate() {
                    if activated.is_some_and(|t| tick.saturating_sub(t) <= AWARD_DEATH_WINDOW) {
                        career.carry_on_respawn |= 1 << medal;
                    }
                }
            }
        }
    }

    pub(in crate::sim) fn respawn_medals(&mut self, old: UnitHandle, new: UnitHandle) {
        for career in self.careers_of_life(old) {
            if std::mem::take(&mut career.preserve_on_respawn) {
                career.life = new;
            } else {
                let carry = std::mem::take(&mut career.carry_on_respawn);
                if carry != 0 {
                    career.life = new;
                    career.earned &= carry;
                    for medal in 0..career.progress.len() {
                        if carry & (1 << medal) == 0 {
                            career.progress[medal] = 0;
                            career.activated_at[medal] = None;
                        }
                    }
                }
            }
        }
    }

    /// Progress toward `medal` for the career following unit slot
    /// `life.index` (the first in slot order: a slot's unit index is its
    /// own while it holds the slot), when that unit is a player of `life`'s
    /// generation.
    fn medal_progress(&mut self, life: UnitHandle, medal: usize, amount: u32) {
        if !self
            .units
            .get(life.index as usize)
            .is_some_and(|u| u.is_player && u.gen == life.gen)
        {
            return;
        }
        let tick = self.tick;
        if let Some(career) = self
            .medal_careers
            .iter_mut()
            .flatten()
            .find(|c| c.life.index == life.index)
        {
            career.add(life, medal, amount, tick);
        }
    }

    pub fn score_medal_points(&mut self, life: UnitHandle, amount: u32) {
        if amount > 0 {
            for medal in 6..=8 {
                self.medal_progress(life, medal, amount);
            }
        }
    }

    pub(super) fn medal_slice(&mut self, life: UnitHandle, health_type: HealthType) {
        if health_type == HealthType::Vehicle {
            self.medal_progress(life, usize::from(TECHNICIAN_MEDAL), 1);
        }
    }

    // B2 FUN_001e0580 calls FUN_0035fd50 with the actual firing weapon,
    // a kill bit, a critical-hit bit and the target's health type.
    pub(super) fn medal_weapon_hit(
        &mut self,
        life: UnitHandle,
        class: usize,
        health_type: HealthType,
        killed: bool,
        critical: bool,
    ) {
        if self.medal_careers.iter().all(Option::is_none) {
            return;
        }
        match self.weapon_classes.get(class).and_then(|w| w.medals_type) {
            Some(i @ (0 | 1 | 5)) if killed => self.medal_progress(life, i as usize, 1),
            Some(2) if critical && health_type == HealthType::Vehicle => {
                self.medal_progress(life, 2, 1)
            }
            Some(4)
                if critical
                    && matches!(
                        health_type,
                        HealthType::Person | HealthType::Animal | HealthType::Droid
                    ) =>
            {
                self.medal_progress(life, 4, 1)
            }
            _ => {}
        }
    }

    /// Earned medals and session cheats for the player. Bots and space
    /// rounds keep their ordinary kits.
    pub fn award_bits(&self, u: &Unit) -> u16 {
        if !u.is_player || self.space_round() {
            return 0;
        }
        let c = self.cheats_of(u);
        let mut bits = self.career_of(u).map_or(0, |c| c.bits(u.gen));
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

    /// The cheats of vehicle `v`'s pilot (its pilot seat's rider): the
    /// session's when no one holds the seat, as when cheats were one set.
    pub(super) fn pilot_cheats(&self, v: &crate::vehicle::Vehicle) -> super::state::Cheats {
        v.seats
            .first()
            .filter(|s| s.kind == crate::vehicle::SeatKind::Pilot)
            .and_then(|s| s.occupant)
            .and_then(|o| self.units.get(o.index as usize).filter(|u| u.gen == o.gen))
            .map_or(self.cheats, |u| self.cheats_of(u))
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
        let kit = std::iter::once(u.weapon_class).chain(u.held.iter().map(|w| w.class));
        self.locked_in(self.award_bits(u), kit, class)
    }

    /// [`Sim::weapon_locked`] for `u` were it to carry class `kit`'s weapons
    /// (the unit screen, which shows a class before `u` takes it).
    pub fn class_weapon_locked(
        &self,
        u: &Unit,
        kit: &crate::unit::UnitClass,
        class: usize,
    ) -> bool {
        let weapons =
            std::iter::once(kit.weapon_class).chain(kit.slots.iter().map(|s| s.weapon_class));
        self.locked_in(self.award_bits(u), weapons, class)
    }

    /// Is weapon class `class` out of the cycle of a holder of medals `bits`
    /// carrying the weapon classes `kit`?
    fn locked_in(&self, bits: u16, kit: impl IntoIterator<Item = usize>, class: usize) -> bool {
        let Some(wc) = self.weapon_classes.get(class) else {
            return false;
        };
        if wc.unlock_medal().is_some_and(|t| bits & bit(t) == 0) {
            return true;
        }
        let Some(t) = wc.medal_lock.filter(|&t| bits & bit(t) != 0) else {
            return false;
        };
        kit.into_iter().any(|c| {
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
