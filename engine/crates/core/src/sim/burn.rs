use super::Sim;
use crate::fmath::FloatExt;
use crate::hero_weapon::Burn;
use crate::unit::UnitHandle;

impl Sim {
    pub fn burn_effects(&self, u: &crate::unit::Unit) -> Option<(usize, &[(String, String)])> {
        let burn = u.burn?;
        let em = self
            .weapon_classes
            .get(burn.weapon_class)?
            .emitter
            .as_ref()?;
        Some((burn.weapon_class, em.burn.effects.as_slice()))
    }

    /// Set an emitter's burn on `victim`. The one burn state and its damage
    /// tick are the hero path's (`hero_weapon::Burn`, `Sim::tick_burns`).
    pub(super) fn ignite(&mut self, victim: UnitHandle, by: UnitHandle, class: usize) {
        let Some(em) = self.weapon_classes[class].emitter.as_ref() else {
            return;
        };
        let (time, damage_per_second) = (em.burn.time, em.burn.rate);
        if !em.bf2 || time <= 0.0 {
            return;
        }
        let Some(team) = self.unit(by).map(|u| u.team) else {
            return;
        };
        let Some(u) = self.unit_mut(victim) else {
            return;
        };
        if u.droideka.is_some() {
            return;
        }
        let left = u.burn.map_or(0.0, |b| b.left).fmax(time);
        u.burn = Some(Burn {
            left,
            damage_per_second,
            owner: by,
            team,
            weapon_class: class,
        });
    }
}

#[cfg(test)]
mod tests;
