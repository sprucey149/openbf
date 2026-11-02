//! The soldier's PS2 water update (BF1 0x205cc8, BF2 0x240bf8;
//! `analysis/re-water.md` §9.2): no buoyancy or speed change, only masked
//! triggers and 30 damage after 3 s underwater, then every 1 s.
use super::{DeathCause, Sim};
use crate::unit::camera_section;
use crate::weapon::DamageDesc;

impl Sim {
    pub(super) fn water_blocks_fire(&self, i: usize) -> bool {
        let u = &self.units[i];
        u.mount.is_none()
            && u.droideka.is_none()
            && self
                .water_under(u.soldier.pos)
                .is_some_and(|level| level > u.fire_point().y)
    }

    pub(super) fn tick_soldier_water(&mut self) {
        for i in 0..self.units.len() {
            let u = &self.units[i];
            // Vehicles and droidekas have their own water handling. Dead or
            // seated soldiers reset the breath timer instead of retaining it.
            let eligible = u.alive && u.mount.is_none() && u.droideka.is_none();
            // Standing origin is 1.80 m in both PS2 traces. PROVISIONAL for
            // crouch/prone: the port's stance eye height until measured.
            let eye = u.soldier.pos.y + camera_section(u.soldier.stance, false).eye_y;
            let submerged = eligible && self.water_under(u.soldier.pos).is_some_and(|h| h >= eye);
            if !self.units[i].soldier.step_water(submerged) {
                continue;
            }
            let h = self.handle(i);
            if self.units[i].is_player && self.cheats.invincible {
                continue;
            }
            let u = &mut self.units[i];
            let damage = u.health.take_damage(
                &DamageDesc {
                    damage: 30.0,
                    scales: [1.0; 7],
                    shield_scale: 1.0,
                },
                1.0,
                false,
                1.0,
            );
            if damage > 0.0 {
                u.cloak.strength = 0.0;
            }
            if !u.health.alive() {
                self.unit_killed_with_cause(h, None, DeathCause::Water);
            }
        }
    }
}
