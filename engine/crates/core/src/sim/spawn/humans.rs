//! Issue #168 (`analysis/plan/LAN_MULTIPLAYER.md` §4.4-§4.6): a human
//! joining or leaving a running round, the sim side of the `AddHuman` and
//! `RemoveHuman` session commands. Every peer applies a command before the
//! same tick's step, so both are plain deterministic edits of the sim: no
//! wall clock, no randomness beyond the sim's own LCG (the bot's class draw).
//!
//! - [`Sim::join_human`] registers a player slot and puts its human behind the
//!   side, post and class screens, as the game's round start does
//!   (`Sim::open_spawn_screens`). The held unit is placed off the field
//!   without the battle's admission, so a join never spends, sheds or is
//!   refused by reinforcements; the class accept spawns it through the
//!   ordinary gate. With a combatant target ([`Sim::team_combatants`]) the
//!   human's reserved slot sheds one surplus bot of its team at once
//!   ([`Sim::set_combatants`]), the population rule a round start keeps.
//! - [`Sim::leave_human`] frees the slot. A live human is stood out of any
//!   seat ([`Sim::exit_vehicle`]), un-spawned without a reinforcement
//!   ([`Sim::vacate_player_slot`]) and replaced by a bot of the same team
//!   at the same spot. A dead human (or one behind the spawn screens) leaves
//!   no bot: the freed combatant slot is refilled by the ordinary respawns.
//!
//! The slot's per-slot inputs (cheats, aim-assist profile, medal career) and
//! screen state go with it, so a later joiner of the same slot starts clean.

use super::{Carousel, SideChoice, SpawnStage};
use crate::sim::state::Sim;
use crate::unit::{Controller, UnitHandle, UnitSpec};
use glam::Vec3;

/// The most player slots the sim takes (LAN §8: the protocol and the slot
/// tables support 32 humans).
pub const MAX_PLAYER_SLOTS: usize = 32;

/// Why [`Sim::join_human`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinError {
    /// The slot is [`MAX_PLAYER_SLOTS`] or more.
    BadSlot,
    /// The slot already holds a human.
    SlotTaken,
    /// A team other than 0 or 1.
    BadTeam,
}

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            JoinError::BadSlot => "player slot out of range",
            JoinError::SlotTaken => "player slot already holds a human",
            JoinError::BadTeam => "team must be 0 or 1",
        })
    }
}

impl std::error::Error for JoinError {}

impl Sim {
    /// Seat a human in player slot `slot` mid-round, behind the spawn
    /// screens like a fresh round start: on the side select when `team` is
    /// `None` (Instant Action), else on `team`'s spawn map. Returns the held
    /// (dead) unit the slot points at; the class accept brings it to life.
    ///
    /// The held unit is the team's first class (the trooper of
    /// [`UnitSpec::player_in_slot`] without a class table), at the team's
    /// first spawn point. With `team` `None` it waits on the team with fewer
    /// humans (team 0 on a tie) until the side select moves it. It takes a
    /// free unit index (a dead unit no slot holds and no respawn awaits), so
    /// no pending bot respawn is lost.
    pub fn join_human(&mut self, slot: usize, team: Option<u8>) -> Result<UnitHandle, JoinError> {
        if slot >= MAX_PLAYER_SLOTS {
            return Err(JoinError::BadSlot);
        }
        if self.players.get(slot).copied().flatten().is_some() {
            return Err(JoinError::SlotTaken);
        }
        if team.is_some_and(|t| t > 1) {
            return Err(JoinError::BadTeam);
        }
        let seat = team.unwrap_or_else(|| {
            let h = self.humans_per_team();
            u8::from(h[1] < h[0])
        });
        let (pos, yaw) = self
            .spawn_points(seat)
            .first()
            .copied()
            .unwrap_or((Vec3::ZERO, 0.0));
        let controller = Controller::Player { slot: slot as u8 };
        let spec = match self.classes[usize::from(seat)].first() {
            Some(class) => class.spec(seat, pos, yaw, controller, 0, 0),
            None => UnitSpec {
                yaw,
                ..UnitSpec::player_in_slot(slot as u8, seat, pos)
            },
        };
        let held = |i: usize| self.players.iter().flatten().any(|p| p.index as usize == i);
        let free = (0..self.units.len())
            .find(|&i| !self.units[i].alive && !held(i) && self.units[i].respawn_at.is_none());
        let gen = free.map_or(1, |i| self.units[i].gen + 1);
        let mut unit = spec.build(gen, &self.weapon_classes);
        if self.weapon_classes[unit.weapon_class].saber.is_some()
            || unit
                .held
                .iter()
                .any(|w| self.weapon_classes[w.class].saber.is_some())
        {
            self.melee_lcg_active = true;
        }
        unit.alive = false;
        unit.respawn_at = None;
        unit.spawn_slot = None;
        let index = match free {
            Some(i) => {
                self.units[i] = unit;
                i
            }
            None => {
                self.units.push(unit);
                self.units.len() - 1
            }
        };
        let h = UnitHandle {
            index: index as u32,
            gen,
        };
        self.reserve_slot(slot);
        self.players[slot] = Some(h);
        self.reset_slot_state(slot);
        self.spawn_screens = true;
        self.side_choice[slot] = if seat == 1 {
            SideChoice::Team2
        } else {
            SideChoice::Team1
        };
        match team {
            Some(t) => self.open_spawn_map(slot, t),
            None => self.spawn_stage[slot] = SpawnStage::Side,
        }
        if let Some(counts) = self.team_combatants {
            self.set_combatants(counts);
        }
        Ok(h)
    }

    /// The human in player slot `slot` leaves (LAN §4.6). A live human is
    /// stood out of any seat at its exit point (the vehicle stays where it
    /// is, empty; a bot may board it the ordinary way), taken off the field
    /// without spending a reinforcement, and a bot of the same team spawns
    /// where it stood, facing its way: that bot's handle is returned. `None`
    /// when the slot held no human, the human was dead or behind the spawn
    /// screens (the population refills through the ordinary respawns), or
    /// the battle refused the bot.
    ///
    /// The bot is the team's next AI class ([`Sim::spawn_ai`]'s draw); a sim
    /// without a class table for the team spawns the leaver's own spec as
    /// an AI instead. Either way it takes a free unit index without
    /// cancelling a pending respawn.
    pub fn leave_human(&mut self, slot: usize) -> Option<UnitHandle> {
        let h = self.players.get(slot).copied().flatten()?;
        let i = h.index as usize;
        let bot = if self.unit(h).is_some() {
            let old = self.units[i].spec.clone();
            if self.units[i].mount.is_some() {
                self.exit_vehicle(h);
            }
            let (pos, yaw) = self.vacate_player_slot(slot)?;
            self.takeover_bot(old, pos, yaw)
        } else {
            // Dead, waiting to respawn or behind the screens: nothing on the
            // field to replace. The held unit must not come back.
            let u = &mut self.units[i];
            if u.gen == h.gen {
                u.respawn_at = None;
                u.spawn_slot = None;
            }
            self.players[slot] = None;
            None
        };
        self.reset_slot_state(slot);
        self.set_slot_cheats(slot, None);
        self.set_slot_assist(slot, None);
        self.remove_slot_medal_career(slot);
        bot
    }

    /// The bot that takes a leaving human's place at `pos`/`yaw`.
    fn takeover_bot(&mut self, old: UnitSpec, pos: Vec3, yaw: f32) -> Option<UnitHandle> {
        let team = old.team;
        if self
            .classes
            .get(usize::from(team))
            .is_some_and(|t| !t.is_empty())
        {
            return self.spawn_ai_roster_slot(team, pos, yaw);
        }
        let spec = UnitSpec {
            player_slot: None,
            controller: Controller::Ai,
            pos,
            yaw,
            ..old
        };
        self.spawn_roster_slot(&spec)
    }

    /// Put player slot `slot`'s screen, aim and lock state back to a fresh
    /// slot's (what [`Sim::sync_slot_tables`] grows a new slot with).
    fn reset_slot_state(&mut self, slot: usize) {
        self.sync_slot_tables();
        self.assist[slot] = Default::default();
        self.assist_lead[slot] = None;
        self.magnet[slot] = None;
        self.magnet_hidden[slot] = 0;
        self.reticle_pick[slot] = None;
        self.spawn_choice[slot] = 0;
        self.prev_spawn_bits[slot] = 0;
        self.held_eaten[slot] = Default::default();
        self.spawn_post_choice[slot] = None;
        self.spawn_stage[slot] = SpawnStage::Accepted;
        self.side_choice[slot] = SideChoice::Team1;
        self.class_carousel[slot] = Carousel::default();
        self.class_ammo[slot].clear();
        self.target_hints[slot] = Default::default();
        if let Some(l) = self.lock_on.get_mut(slot) {
            *l = Default::default();
        }
        if let Some(l) = self.foot_lock.get_mut(slot) {
            *l = Default::default();
        }
        if let Some(p) = self.spawn_points.get_mut(slot) {
            *p = 0;
        }
        if let Some(c) = self.spawn_costs.get_mut(slot) {
            *c = Default::default();
        }
    }
}
