//! BF2 officer area abilities: Rage, Rally, Regeneration, Neuro poison
//! (`analysis/re-officer-abilities.md`; issue #47).

use super::{soldier_blast_sphere, Event, Sim};
use crate::weapon::Ability;
use glam::Vec3;

impl Sim {
    /// The soldiers an ability fired by unit `i` reaches: alive, on foot,
    /// within its radius of the caster, on a side it affects and in sight.
    /// The PS2 gathers at most 128 bodies before those tests.
    pub(crate) fn ability_targets<'a>(
        &'a self,
        i: usize,
        ability: &'a Ability,
    ) -> impl Iterator<Item = usize> + 'a {
        self.ability_candidates(i, ability)
            .filter(move |&t| self.ability_sees(i, t))
    }

    /// [`Self::ability_targets`] without the sight test, for a caller that
    /// filters on cheaper conditions before paying for the rays.
    pub(crate) fn ability_candidates<'a>(
        &'a self,
        i: usize,
        ability: &'a Ability,
    ) -> impl Iterator<Item = usize> + 'a {
        let owner = &self.units[i];
        let from = owner.muzzle();
        self.units
            .iter()
            .enumerate()
            .filter(move |(target, u)| {
                if *target == i || !u.alive || u.mount.is_some() {
                    return false;
                }
                let (height, radius) = soldier_blast_sphere(u.soldier.stance);
                let centre = u.soldier.pos + Vec3::Y * height;
                let reach = ability.radius + radius;
                from.distance_squared(centre) <= reach * reach
            })
            .take(128)
            .filter(move |(_, u)| {
                (ability.friends && self.allied(owner.team, u.team))
                    || (ability.enemies && self.hostile(owner.team, u.team))
            })
            .map(|(target, _)| target)
    }

    /// The caster `i` sees soldier `t` (the PS2's visibility ray).
    pub(crate) fn ability_sees(&self, i: usize, t: usize) -> bool {
        crate::perception::los(self, self.units[i].soldier.pos, self.units[t].soldier.pos)
    }

    /// Apply the ability to every target; the caster gets a friendly one
    /// only when someone else did (the PS2's self-on-hit bit).
    pub(super) fn fire_ability(&mut self, i: usize, class: usize) {
        let ability = self.weapon_classes[class].ability.as_ref().unwrap();
        let owner = self.handle(i);
        let from = self.units[i].muzzle();
        self.events.push(Event::Ability {
            unit: owner,
            weapon_class: class as u32,
            pos: from,
            charge: 1.0,
        });
        let targets: Vec<_> = self.ability_targets(i, ability).collect();
        let affected = !targets.is_empty();
        for target in targets {
            let u = &self.units[target];
            let hero = self.ground_rules == crate::soldier::GroundRules::Bf2
                && self.hero_class(u.team) == Some(u.spec.class);
            if !hero {
                self.units[target].buffs.apply_ability(ability, owner);
            }
        }
        let hero = self.ground_rules == crate::soldier::GroundRules::Bf2
            && self.hero_class(self.units[i].team) == Some(self.units[i].spec.class);
        if affected && ability.self_on_hit && ability.friends && !hero {
            self.units[i].buffs.apply_ability(ability, owner);
        }
    }
}

#[cfg(test)]
mod tests;
