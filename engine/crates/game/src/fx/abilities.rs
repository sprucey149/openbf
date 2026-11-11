//! BF2 officer abilities on screen: activation effects and the timed auras that
//! follow their recipients. Follows `analysis/re-officer-abilities.md`
//! (issue #47).

use super::{emitter::Frame, Fx, Gain};

use core::sim::{Event, Sim};
use core::unit::UnitHandle;
use glam::Vec3;

/// The auras of the offense, defense, regeneration and poison effects, in
/// `Buffs::timers` order (`com_sfx_buffed_*`).
pub(super) const BUFF_EFFECTS: [u32; 4] = [0xc17a_b11a, 0x28b9_f3ac, 0xbb3e_84e9, 0xcaa1_4018];

impl Fx {
    pub(super) fn follow_buffs(&mut self, sim: &Sim) {
        for effect in &mut self.live {
            let Some((h, kind)) = effect.buff else {
                continue;
            };
            let owner = sim
                .unit(h)
                .filter(|u| u.buffs.timers()[kind] > 0.0 && u.cloak.visible());
            if let Some(u) = owner {
                effect.follow_unit(u);
            } else {
                effect.stop();
                effect.buff = None;
            }
        }
        for (h, u) in sim.alive_units() {
            for (kind, (timer, hash)) in u.buffs.timers().into_iter().zip(BUFF_EFFECTS).enumerate()
            {
                if timer <= 0.0
                    || !u.cloak.visible()
                    || self.live.iter().any(|e| e.buff == Some((h, kind)))
                {
                    continue;
                }
                let frame = Frame {
                    vel: u.soldier.vel,
                    ..Frame::at(u.soldier.pos)
                };
                if self.spawn_hash(hash, frame) {
                    self.live.last_mut().unwrap().buff = Some((h, kind));
                }
            }
        }
        for event in &sim.events {
            let Event::Ability {
                unit, weapon_class, ..
            } = event
            else {
                continue;
            };
            let Some(u) = sim.unit(*unit) else { continue };
            let Some(name) = sim
                .weapon_classes
                .get(*weapon_class as usize)
                .and_then(|w| w.ability.as_ref())
                .and_then(|a| a.activation_effect.as_deref())
            else {
                continue;
            };
            if self.spawn(name, Frame::at(u.soldier.pos)) {
                self.live.last_mut().unwrap().unit = Some((*unit, Gain::Activation));
                // The cast effect's own sound (`buff_rage_fire` and the like),
                // beside the weapon's `FireSound` (`crate::event_sounds`).
                self.queue_last_effect_sound(u.soldier.pos);
            }
        }
    }

    /// Each live aura's looping sound (its effect's `SoundName`, e.g.
    /// `buff_rage_effect`, `" defer"` dropped) at the unit it follows:
    /// (unit, the aura's kind, the sound, where). It lasts as long as the
    /// aura, which [`Fx::follow_buffs`] stops at the effect's end, the
    /// unit's death or its cloak, as the PS2 does (B2 `FUN_002368a0`).
    pub fn buff_loops(&self, sim: &Sim) -> Vec<(UnitHandle, usize, String, Vec3)> {
        self.live
            .iter()
            .filter(|e| !e.is_dead())
            .filter_map(|e| {
                let (h, kind) = e.buff?;
                let u = sim.unit(h)?;
                let name = e.def.emitters.iter().find_map(|em| {
                    em.sound_name
                        .as_deref()
                        .and_then(|s| s.split_whitespace().next())
                })?;
                Some((h, kind, name.to_owned(), u.soldier.pos))
            })
            .collect()
    }
}
