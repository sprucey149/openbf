//! Issue #28: change class at a friendly command post without dying
//! (`analysis/re-class-change.md`). The PS2 (BF2 only) opens its class
//! screen over a live soldier who presses the use button inside a post his
//! team can spawn at while facing it (`FUN_001f8548`), and an accept swaps the
//! soldier for the new class in place (`FUN_0034dac0`). The maintainer's rules
//! (2026-10-27) replace the PS2's carried ammo fraction: the health carries,
//! the ammo is per class and per life, and a death forgets it.

use super::{ClassAmmo, SpawnStage, CLASS_CHANGE_FACING};
use crate::sim::input::{buttons, PadState};
use crate::sim::state::{Event, Sim};
use crate::unit::{Controller, Unit, UnitHandle};

/// The rounds in each of `u`'s weapons, by class weapon slot.
fn rounds_of(u: &Unit) -> Vec<(u8, u32, u32)> {
    std::iter::once((u.weapon_slot, &u.weapon))
        .chain(u.held.iter().map(|h| (h.slot, &h.state)))
        .map(|(slot, w)| (slot, w.clip, w.spare))
        .collect()
}

impl Sim {
    pub fn begin_test_classes(&mut self, team: u8) {
        let t = usize::from(team.min(1));
        let roster = (self.classes[t].len(), self.hero_class(team));
        self.test_rosters[t].get_or_insert(roster);
    }

    pub fn test_change_class(&mut self, slot: usize, class: u8, hero: bool) -> bool {
        let Some(h) = self.players.get(slot).copied().flatten() else {
            return false;
        };
        let Some(u) = self.unit(h) else { return false };
        let team = usize::from(u.team);
        if !u.alive
            || u.mount.is_some()
            || self.outcome.is_some()
            || usize::from(class) >= self.classes[team].len()
        {
            return false;
        }
        self.retire_saber_flights(h.index as usize);
        self.class_change_life_ended(h);
        self.change_class(slot, h, class, true);
        let next = self.players[slot].unwrap();
        let u = &mut self.units[next.index as usize];
        u.health.cur = u.health.max;
        self.spawn_choice[slot] = 0;
        self.spawn_stage[slot] = SpawnStage::Accepted;
        if hero && self.heroes.is_none() {
            self.heroes = Some(crate::sim::heroes::Heroes::new([None; 2]));
        }
        if let Some(heroes) = self.heroes.as_mut() {
            if hero || heroes.teams[team].owner == Some(next.index) {
                heroes.classes[team] = if hero {
                    Some(class)
                } else {
                    self.test_rosters[team].and_then(|(_, hero)| hero)
                };
                heroes.teams[team] = crate::sim::heroes::HeroTeam::default();
                if hero {
                    let state = &mut heroes.teams[team];
                    state.unlocked = true;
                    state.owner = Some(next.index);
                    state.owner_gen = h.gen;
                    state.spawned = true;
                }
            }
        }
        true
    }

    /// The friendly post unit `i` may change class at now, if any: a live
    /// human on foot, counted inside the post's capture region (the PS2's
    /// players-inside mask, cp+0x74), the post spawnable by his team
    /// (`FUN_00356880`), and facing it (`CLASS_CHANGE_FACING`). Enemy and
    /// neutral posts never qualify; neither does a bot.
    ///
    /// Hunt, XL and campaign take [`Self::hunt_class_change_near`]: their
    /// posts count no occupants ([`Self::counts_in_cp`]), or a script
    /// disabled the capture region. Hero Assault, the other deathmatch,
    /// keeps no class change.
    pub fn class_change_post(&self, i: usize) -> Option<usize> {
        let u = self.units.get(i)?;
        if !u.alive || !matches!(u.controller, Controller::Player { .. }) || u.mount.is_some() {
            return None;
        }
        let hunt = self.is_hunt() || self.is_xl();
        let (fwd, _, _) = u.aim_frame();
        (0..self.cps.len()).find(|&ci| {
            let cp = &self.cps[ci];
            let to = cp.def.pos - u.soldier.pos;
            let d = to.length();
            let near = if hunt || self.battle.campaign_rules {
                self.hunt_class_change_near(i, ci, d)
            } else {
                self.counts_in_cp(i, ci) && cp.spawnable_by(u.team)
            };
            near && d > 1e-4 && fwd.dot(to / d) > CLASS_CHANGE_FACING
        })
    }

    /// Hunt: is unit `i` near enough to post `ci` (at `d` m) to change
    /// class there? The PS2's two scans (B2 `FUN_001f8548` 109161-109290),
    /// without the capture count Hunt's posts never keep
    /// ([`Self::counts_in_cp`]): (1) inside the post's AUTHORED capture
    /// region with the post spawnable by his team (the BF1 adaptations'
    /// posts keep their regions), or (2) within the post class's
    /// `SwitchClassRadius` of a post his team owns (BF2's Hunt posts, which
    /// author no region; a BF1 post's is `[ours]` 5 m). The caller tests the facing, which both scans
    /// share. A post the mission killed is not in the sim at all.
    fn hunt_class_change_near(&self, i: usize, ci: usize, d: f32) -> bool {
        let u = &self.units[i];
        let cp = &self.cps[ci];
        let inside = cp.capture_authored
            && cp.def.capture.contains(u.soldier.pos)
            && cp.spawnable_by(u.team);
        let close = !cp.killed && d < cp.switch_class_radius && cp.owner == Some(u.team);
        inside || close
    }

    /// Step 0a, after the spawn screens: a use-button edge (`ACTION`, the
    /// pad's Triangle) from a live human at a friendly post opens the class
    /// screen over him (stage [`SpawnStage::Alive`]), on the class he plays.
    /// A vehicle he could enter takes the press first (the use input's own
    /// pass); the PS2's order of the two is not read (PROVISIONAL).
    pub(in crate::sim) fn class_change_open(&mut self, pads: &[PadState]) {
        for s in 0..self.players.len() {
            let Some(h) = self.players[s] else { continue };
            let i = h.index as usize;
            let Some(u) = self.units.get(i).filter(|u| u.gen == h.gen) else {
                continue;
            };
            let held = pads.get(s).map_or(0, |p| p.buttons);
            let edge =
                held & buttons::ACTION != 0 && u.soldier.prev_buttons() & buttons::ACTION == 0;
            if !edge
                || self.spawn_stage.get(s) != Some(&SpawnStage::Accepted)
                || self.classes[usize::from(u.team.min(1))].is_empty()
                || self.enter_target(h).is_some()
                || self.class_change_post(i).is_none()
            {
                continue;
            }
            // `[ours]` (maintainer, 2026-11-19): a hero keeps his class
            // until he dies; the use press only says so.
            if self.hero_locked(s) {
                self.events.push(Event::HeroClassLocked { unit: h });
                continue;
            }
            let class = u.spec.class;
            self.spawn_stage[s] = SpawnStage::Alive;
            self.spawn_choice[s] = class;
            // The press that opened it does not back out next tick, and a
            // push held into the screen steps only once let go.
            self.prev_spawn_bits[s] = held
                & (buttons::SPAWN_CLASS_NEXT
                    | buttons::SPAWN_CLASS_PREV
                    | buttons::SPAWN_POST_UP
                    | buttons::SPAWN_POST_DOWN
                    | buttons::SPAWN_POST_LEFT
                    | buttons::SPAWN_POST_RIGHT
                    | buttons::SPAWN_ACCEPT
                    | buttons::SPAWN_BACK);
            self.class_carousel[s].armed = held
                & (buttons::SPAWN_CLASS_NEXT
                    | buttons::SPAWN_CLASS_PREV
                    | buttons::SPAWN_POST_LEFT
                    | buttons::SPAWN_POST_RIGHT)
                != 0;
        }
    }

    /// The pads the rest of the tick reads: a slot with the class screen up
    /// gives its soldier none (the PS2's pad drives the screen).
    pub(in crate::sim) fn class_screen_pads(&self, pads: &[PadState]) -> Option<Vec<PadState>> {
        if !self.spawn_stage.contains(&SpawnStage::Alive) {
            return None;
        }
        let mut out = pads.to_vec();
        for (s, p) in out.iter_mut().enumerate() {
            if self.spawn_stage.get(s) == Some(&SpawnStage::Alive) {
                *p = PadState::default();
            }
        }
        Some(out)
    }

    /// A script popup closed over the live class screen. The PS2 hides that
    /// screen while the popup is up (`FUN_0038c7a0` → `FUN_00397840` when
    /// the display's state is 4) and reopens it on the close
    /// (`FUN_0038cb28` → `FUN_00397f00`). OURS: the press that closed the
    /// popup is not an accept on the reopened screen; whether
    /// `FUN_00397f00` arms the X is not read. trace: `FUN_00397f00`.
    pub fn hold_class_screen_accept(&mut self) {
        for (s, stage) in self.spawn_stage.iter().enumerate() {
            if *stage == SpawnStage::Alive {
                if let Some(bits) = self.prev_spawn_bits.get_mut(s) {
                    *bits |= buttons::SPAWN_ACCEPT;
                }
            }
        }
    }

    /// Triangle on the live class screen: back to play on the class he has.
    pub(in crate::sim) fn class_change_close(&mut self, s: usize, held: u32) {
        self.spawn_stage[s] = SpawnStage::Accepted;
        if let Some(h) = self.players[s] {
            let u = &mut self.units[h.index as usize];
            self.spawn_choice[s] = u.spec.class;
            // The Triangle that closed it is not a use press.
            u.soldier.set_prev_buttons(held);
        }
    }

    /// X on the live class screen: a refused class stays (as the class
    /// screen, `FUN_00364290`); the class he plays just closes it; another
    /// swaps him in place ([`Sim::change_class`]). A hero's owner never
    /// swaps (`[ours]` 2026-11-19, [`Sim::hero_locked`]): the screen closes.
    pub(in crate::sim) fn class_change_accept(&mut self, s: usize, team: u8, held: u32) {
        let Some(h) = self.players[s] else { return };
        let n = self.classes[usize::from(team.min(1))].len();
        let c = usize::from(self.spawn_choice[s]).min(n.saturating_sub(1));
        if self.hero_locked(s) {
            self.class_change_close(s, held);
            return;
        }
        if self.class_refusal(s, team, c).is_some() {
            return;
        }
        if usize::from(self.units[h.index as usize].spec.class) != c {
            self.change_class(s, h, c as u8, false);
        }
        self.class_change_close(s, held);
    }

    /// Swap slot `s`'s live unit `h` for class `class` where it stands (BF2
    /// `FUN_0034dac0`: the old soldier removed, the new one spawned at its
    /// matrix; no death, no ticket). The new unit takes the old one's share
    /// of its health (the PS2's health / max), and its ammo is the class's
    /// remembered this life, else full; the class it leaves is remembered.
    /// A new generation, as the PS2's new object (`Event::Respawn`, the
    /// member spawn `FUN_0034e1f0`).
    pub(in crate::sim) fn change_class(
        &mut self,
        s: usize,
        h: UnitHandle,
        class: u8,
        refill: bool,
    ) {
        let i = h.index as usize;
        let old = &self.units[i];
        let team = old.team;
        let share = if old.health.max > 1e-4 {
            (old.health.cur / old.health.max).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let leaving = ClassAmmo {
            class: old.spec.class,
            rounds: rounds_of(old),
        };
        let memory = &mut self.class_ammo[s];
        memory.retain(|m| m.class != leaving.class);
        memory.push(leaving);
        let back = if refill {
            None
        } else {
            memory.iter().find(|m| m.class == class).cloned()
        };

        let old = &self.units[i];
        let base = self.bot_skill(team);
        let spec = self.classes[usize::from(team.min(1))][usize::from(class)].spec(
            team,
            old.soldier.pos,
            old.soldier.yaw,
            old.controller,
            class,
            base,
        );
        let mut unit = spec.build(old.gen + 1, &self.weapon_classes);
        unit.skill = crate::skill_for(spec.controller, base);
        unit.spec.skill = unit.skill;
        unit.soldier.pos = old.soldier.pos;
        unit.soldier.pitch = old.soldier.pitch;
        unit.kills = old.kills;
        unit.deaths = old.deaths;
        unit.health.cur = share * unit.health.max;
        if let Some(back) = back {
            for (slot, clip, spare) in back.rounds {
                let w = if unit.weapon_slot == slot {
                    Some(&mut unit.weapon)
                } else {
                    unit.held
                        .iter_mut()
                        .find(|w| w.slot == slot)
                        .map(|w| &mut w.state)
                };
                if let Some(w) = w {
                    w.clip = clip;
                    w.spare = spare;
                }
            }
        }
        self.units[i] = unit;
        let h = self.handle(i);
        self.players[s] = Some(h);
        self.events.push(Event::Respawn { unit: h });
    }

    /// A life ends for unit `h` (a death, or the slot leaving the field):
    /// its slot forgets every class's ammo and its live class screen closes.
    pub(in crate::sim) fn class_change_life_ended(&mut self, h: UnitHandle) {
        let Some(s) = self
            .units
            .get(h.index as usize)
            .and_then(|u| u.spec.player_slot)
            .map(usize::from)
        else {
            return;
        };
        if let Some(m) = self.class_ammo.get_mut(s) {
            m.clear();
        }
        if self.spawn_stage.get(s) == Some(&SpawnStage::Alive) {
            self.spawn_stage[s] = SpawnStage::Accepted;
            self.spawn_choice[s] = self.units[h.index as usize].spec.class;
        }
    }
}
