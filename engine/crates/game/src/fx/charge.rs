//! A charge weapon's `ChargeUpEffect` at its fire point while the trigger is
//! held: the arc caster's (BF2 `com_sfx_chargeup`, a glow and smoke) and the
//! other charge weapons' that name one (the bowcasters, BF1's `chargeup`).
//!
//! What BF2 does [O]: while the weapon is in its charge state (weapon state
//! 3, `re-heat-hud.md` 2) the effect is built once, held on
//! the weapon's fire point every drawn frame, and its emitter colour, all
//! four channels, set to the charge (time held / MaxPressedTime). So it rises
//! from nothing to full brightness as the charge fills and stays at full
//! while a full charge is held. Leaving the charge state (the release fires,
//! or the weapon is put away) stops it at once. Every soldier's weapon draws
//! it, bots' included. trace: the PS2 weapon render's charge-state branch
//! (B2, the draw beside the muzzle flash), not yet located in the PS2 code.
//!
//! Presentation only: it reads [`core::weapon::WeaponState::charging`] and
//! never writes the sim.

use super::{emitter::Frame, Fx};
use core::sim::Sim;
use core::unit::{Unit, UnitHandle};
use glam::{Mat3, Vec3};

/// Each game's charge-up effect: BF1's and BF2's. `[ours]` a weapon from the
/// other game (the Jawas' BF2 arc caster on a BF1 map) draws the map game's
/// own, as that game's effect table holds no other.
const CHARGE_UP_PAIR: [[&str; 2]; 1] = [["chargeup", "com_sfx_chargeup"]];

/// The weapon class a soldier is charging and its charge, 0..1; `None` while
/// he is not charging an on-foot weapon that names a `ChargeUpEffect`.
pub fn charging(sim: &Sim, u: &Unit) -> Option<(usize, f32)> {
    if !u.alive || u.mount.is_some() {
        return None;
    }
    let c = sim.weapon_classes.get(u.weapon_class)?;
    c.charge_up_effect.as_ref()?;
    Some((u.weapon_class, u.weapon.charging(c)?))
}

/// The sim's fire point and aim; the drawn weapon tip replaces the point
/// each drawn frame ([`Fx::set_repair_origins`]).
fn frame(u: &Unit) -> Frame {
    let (forward, right, up) = u.aim_frame();
    Frame {
        pos: u.muzzle(),
        rot: Mat3::from_cols(-right, up, forward),
        vel: u.soldier.vel,
    }
}

impl Fx {
    pub(super) fn follow_charges(&mut self, sim: &Sim) {
        // Out of the charge state: stopped at once, its particles with it.
        self.live.retain(|e| {
            let Some((h, class)) = e.charge else {
                return true;
            };
            sim.unit(h)
                .and_then(|u| charging(sim, u))
                .is_some_and(|(c, _)| c == class)
        });
        for e in &mut self.live {
            let Some((h, _)) = e.charge else { continue };
            let Some(u) = sim.unit(h) else { continue };
            if let Some((_, f)) = charging(sim, u) {
                e.glow = f;
                e.frame = frame(u);
            }
        }
        for (h, u) in sim.alive_units() {
            let Some((class, f)) = charging(sim, u) else {
                continue;
            };
            if self.live.iter().any(|e| e.charge == Some((h, class))) {
                continue;
            }
            let Some(name) = sim.weapon_classes[class].charge_up_effect.as_deref() else {
                continue;
            };
            // A weapon drawn on the other game's map (the Jawas' BF2 arc
            // caster on a BF1 map) names the other game's effect; each game
            // has its own charge-up under its own name.
            let spawned = self.spawn(name, frame(u))
                || CHARGE_UP_PAIR.iter().any(|pair| {
                    pair.iter()
                        .position(|n| n.eq_ignore_ascii_case(name))
                        .is_some_and(|k| self.spawn(pair[1 - k], frame(u)))
                });
            if spawned {
                let e = self.live.last_mut().unwrap();
                e.charge = Some((h, class));
                e.glow = f;
            }
        }
    }

    /// The live charge glows: whose, how bright and where.
    pub fn charge_glows(&self) -> Vec<(UnitHandle, f32, Vec3)> {
        self.live
            .iter()
            .filter_map(|e| e.charge.map(|(h, _)| (h, e.glow, e.frame.pos)))
            .collect()
    }
}

#[cfg(test)]
mod tests;
