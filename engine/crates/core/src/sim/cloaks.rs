//! The cloak in the sim: activation, shared energy, drain, fade, flicker and
//! cleanup when it ends. Follows `analysis/re-cloaking.md` (card CLOAK).

use super::{Event, Sim};
use crate::cloak::CloakState;
use crate::fmath::FloatExt;

impl Sim {
    pub(super) fn cloak_input(
        &mut self,
        i: usize,
        slot: Option<usize>,
        held: bool,
        blocked: bool,
    ) -> bool {
        let u = &mut self.units[i];
        let class = slot.map_or(u.weapon_class, |k| u.held[k].class);
        let Some(c) = self.weapon_classes[class].cloak.as_ref() else {
            return false;
        };
        let selected = slot.is_none() && u.active_channel == 0;
        if selected && held && !u.cloak.trigger && !blocked && u.cloak.warmup.is_none() {
            if u.cloak.active {
                u.cloak.release(c.linger);
            } else if c.affordable(&u.soldier) {
                u.soldier.energy -= c.energy_cost;
                u.cloak.warmup = Some(c.delay);
            }
        }
        u.cloak.trigger = held && selected;
        true
    }

    pub(super) fn tick_cloaks(&mut self, dt: f32) {
        for i in 0..self.units.len() {
            let unit = self.handle(i);
            let u = &self.units[i];
            let Some((class, c)) = std::iter::once(u.weapon_class)
                .chain(u.held.iter().map(|h| h.class))
                .find_map(|k| self.weapon_classes[k].cloak.as_ref().map(|c| (k, c)))
            else {
                continue;
            };
            let stamina_cheat = self.unit_cheats(i).infinite_stamina;
            let u = &mut self.units[i];
            if !u.alive || u.mount.is_some() {
                u.cloak = CloakState::default();
                continue;
            }
            if u.cloak.health != u.health.cur {
                u.cloak.strength = 0.0;
                u.cloak.health = u.health.cur;
            }
            let selected = u.weapon_class == class && u.active_channel == 0;
            if !selected {
                u.cloak.warmup = None;
                u.cloak.trigger = false;
                if u.cloak.active {
                    u.cloak.release(c.linger);
                }
            }
            if let Some(left) = u.cloak.warmup.as_mut() {
                *left -= dt;
                if *left <= 0.0 {
                    u.cloak.warmup = None;
                    u.cloak.active = true;
                    self.events.push(Event::Cloak {
                        unit,
                        weapon_class: class as u32,
                        pos: u.soldier.pos,
                    });
                }
            }
            if u.cloak.active {
                if u.soldier.energy_depleted || u.soldier.energy <= 0.0 {
                    u.cloak.release(0.0);
                } else if !(stamina_cheat && u.is_player) {
                    u.soldier.energy = (u.soldier.energy - c.energy_drain * dt).fmax(0.0);
                    if u.soldier.energy == 0.0 {
                        u.soldier.energy_depleted = true;
                        u.cloak.release(0.0);
                    }
                }
            }
            u.cloak.linger = (u.cloak.linger - dt).fmax(0.0);
            let concealed = u.cloak.active || u.cloak.linger > 0.0;
            let target = if concealed {
                c.concealment(u.soldier.vel.length_squared())
            } else {
                0.0
            };
            let rate = if u.cloak.strength < target {
                c.fade_in
            } else {
                c.fade_out
            };
            u.cloak.strength += (target - u.cloak.strength).clamp(-rate * dt, rate * dt);
            if u.cloak.active && c.flicker > 0.0 {
                u.cloak.flicker_left -= dt;
                if u.cloak.flicker_left <= 0.0 {
                    u.cloak.strength = (u.cloak.strength - c.flicker).fmax(0.0);
                    let [lo, hi] = c.flicker_interval;
                    u.cloak.flicker_left = if hi > lo {
                        lo + self.lcg.unit() * (hi - lo)
                    } else {
                        lo
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
