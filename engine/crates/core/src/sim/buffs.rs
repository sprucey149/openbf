//! Timed soldier effects from abilities and pickups: damage dealt and taken
//! multipliers, regeneration and poison. Each is (seconds left, strength);
//! a longer incoming timer replaces both (`analysis/re-officer-abilities.md`).

use super::Sim;
use crate::unit::UnitHandle;
use crate::weapon::{Ability, DamageDesc};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Buffs {
    pub offense: (f32, f32),
    pub defense: (f32, f32),
    pub regeneration: (f32, f32),
    pub poison: (f32, f32),
    pub poisoned_by: Option<UnitHandle>,
}

impl Default for Buffs {
    fn default() -> Self {
        Buffs {
            offense: (0.0, 1.0),
            defense: (0.0, 1.0),
            regeneration: (0.0, 0.0),
            poison: (0.0, 0.0),
            poisoned_by: None,
        }
    }
}

impl Buffs {
    /// Count the timers down. Timers over 300 s never run down: the PS2
    /// keeps award effects (340 s) permanent that way.
    pub(super) fn tick(&mut self, dt: f32) {
        for b in [
            &mut self.offense,
            &mut self.defense,
            &mut self.regeneration,
            &mut self.poison,
        ] {
            if b.0 > 0.0 && b.0 <= 300.0 {
                b.0 = (b.0 - dt).max(0.0);
            }
        }
    }

    pub fn timers(&self) -> [f32; 4] {
        [
            self.offense.0,
            self.defense.0,
            self.regeneration.0,
            self.poison.0,
        ]
    }

    pub fn active(&self) -> bool {
        self.timers().into_iter().any(|timer| timer > 0.0)
    }

    pub(super) fn apply_multipliers(&mut self, offense: (f32, f32), defense: (f32, f32)) {
        refresh(&mut self.offense, offense);
        refresh(&mut self.defense, defense);
    }

    pub(super) fn apply_ability(&mut self, ability: &Ability, owner: UnitHandle) {
        self.apply_multipliers(ability.offense, ability.defense);
        refresh(&mut self.regeneration, ability.regeneration);
        if refresh(&mut self.poison, ability.poison) {
            self.poisoned_by = Some(owner);
        }
    }

    pub fn dealt(&self) -> f32 {
        if self.offense.0 > 0.0 {
            self.offense.1
        } else {
            1.0
        }
    }

    pub fn taken(&self) -> f32 {
        if self.defense.0 > 0.0 {
            self.defense.1
        } else {
            1.0
        }
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [
            self.offense.0,
            self.offense.1,
            self.defense.0,
            self.defense.1,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        if self.regeneration.0 > 0.0 || self.poison.0 > 0.0 {
            eat(b"ability");
            for v in [
                self.regeneration.0,
                self.regeneration.1,
                self.poison.0,
                self.poison.1,
            ] {
                eat(&v.to_bits().to_le_bytes());
            }
            if self.poison.0 > 0.0 {
                if let Some(h) = self.poisoned_by {
                    eat(&h.index.to_le_bytes());
                    eat(&h.gen.to_le_bytes());
                }
            }
        }
    }
}

/// Replace an effect only with a longer one; true when it was replaced.
fn refresh(current: &mut (f32, f32), incoming: (f32, f32)) -> bool {
    if incoming.0 > current.0 {
        *current = incoming;
        true
    } else {
        false
    }
}

impl Sim {
    /// One tick of every soldier's effects: dead soldiers lose theirs,
    /// regeneration heals up to full health, poison damages with its
    /// caster's credit.
    pub(super) fn tick_buffs(&mut self, dt: f32) {
        for i in 0..self.units.len() {
            if !self.units[i].alive {
                self.units[i].buffs = Buffs::default();
                continue;
            }
            self.units[i].buffs.tick(dt);
            let sealed = self.units[i].mount.is_some() && !self.hero_body_active(i);
            let u = &mut self.units[i];
            // Inside a hull his health is what he boarded with, either way.
            if !sealed && u.buffs.regeneration.0 > 0.0 {
                u.health.cur = (u.health.cur + u.buffs.regeneration.1 * dt).min(u.health.max);
            }
            if u.buffs.poison.0 <= 0.0 {
                u.buffs.poisoned_by = None;
                continue;
            }
            if sealed {
                continue;
            }
            let attacker = u.buffs.poisoned_by;
            let damage = u.buffs.poison.1 * dt;
            let victim = self.handle(i);
            let scale = self.buff_scale(attacker, victim);
            let invincible = self.units[i].is_player && self.cheats.invincible;
            let now = self.tick;
            let u = &mut self.units[i];
            if !invincible {
                if damage > 0.0 {
                    u.cloak.strength = 0.0;
                }
                u.health.take_damage(
                    &DamageDesc {
                        damage,
                        scales: [1.0; 7],
                        shield_scale: 1.0,
                    },
                    scale,
                    false,
                    1.0,
                );
            }
            // Poison keeps its attacker for perception and the kill credit,
            // but is no shot: an `Event::Hit` every tick would spawn a hit
            // puff, a hurt-cry check and a black-box row 30 times a second.
            if let Some(a) = attacker {
                u.last_hit = Some((a, now));
            }
            if !u.health.alive() {
                self.unit_killed(victim, attacker);
                self.units[i].buffs = Buffs::default();
            }
        }
    }
}
