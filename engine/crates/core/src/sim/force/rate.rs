//! The area weapon overrides the soldier's ordinary energy recovery rate.

use super::*;
use crate::soldier::SoldierConstants;

impl Sim {
    pub(in crate::sim) fn force_constants(&self, i: usize, constants: &mut SoldierConstants) {
        let u = &self.units[i];
        let (class, state) = match u.cur1.filter(|_| u.active_channel == 1) {
            Some(k) => {
                let w = &u.held[k as usize];
                (w.class, &w.state)
            }
            None => (u.weapon_class, &u.weapon),
        };
        let Some(ForcePower {
            kind: ForceKind::Choke { drain_rate, .. },
            ..
        }) = self.weapon_classes[class].force.as_ref()
        else {
            return;
        };
        // B2 0032b958: Hold mode, state 1, and an active initial target.
        if *drain_rate != 0.0
            && state.state == FireState::Firing
            && state.force_channel.as_ref().is_some_and(|channel| {
                channel.acquired && channel.target.is_some_and(|h| self.unit(h).is_some())
            })
        {
            constants.energy_override = Some(-*drain_rate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_weapon::{ForceChannel, ForcePower};
    use crate::soldier::{GroundRules, Soldier};
    use crate::unit::{HeldWeapon, UnitSpec};
    use crate::weapon::WeaponClass;

    #[test]
    fn choke_rate_overrides_recovery_only_for_the_active_valid_hold() {
        let mut sim = Sim::empty(91);
        sim.weapon_classes = vec![WeaponClass {
            force: Some(ForcePower {
                range: 20.0,
                cone_cos: 0.98,
                max_targets: 1,
                push: 0.0,
                affect_friends: false,
                affect_enemies: true,
                energy: 40.0,
                kind: ForceKind::Choke {
                    angle: 10.0 * crate::weapon::DEG,
                    damage_per_second: 150.0,
                    height: 4.0,
                    falloff: 1.0,
                    drain_rate: 15.0,
                },
                effect: None,
                animation: None,
            }),
            ..Default::default()
        }]
        .into();
        let caster = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        sim.spawn(&UnitSpec::trooper(1, Vec3::NEG_Z * 3.0)).unwrap();
        let owner = caster.index as usize;
        let target = sim.handle(1);
        sim.units[owner].weapon.force_channel = Some(ForceChannel {
            acquired: true,
            target: Some(target),
            ..Default::default()
        });
        sim.units[owner].weapon.state = FireState::Firing;
        let mut constants = sim.units[owner].consts;
        sim.force_constants(owner, &mut constants);
        assert_eq!(constants.energy_override, Some(-15.0));
        for control in 0..4 {
            let mut state = sim.units[owner].weapon.clone();
            match control {
                0 => state.force_channel.as_mut().unwrap().acquired = false,
                1 => state.force_channel.as_mut().unwrap().target = None,
                2 => state.state = FireState::Idle,
                _ => {
                    state
                        .force_channel
                        .as_mut()
                        .unwrap()
                        .target
                        .as_mut()
                        .unwrap()
                        .gen += 1
                }
            }
            let old = std::mem::replace(&mut sim.units[owner].weapon, state);
            let mut constants = sim.units[owner].consts;
            sim.force_constants(owner, &mut constants);
            assert_eq!(constants.energy_override, None, "control {control}");
            sim.units[owner].weapon = old;
        }
        // The same class and channel state in a carried slot must only
        // override recovery while that secondary is actually in hand.
        sim.weapon_classes.push(WeaponClass::default());
        sim.units[owner].held = vec![HeldWeapon {
            slot: 1,
            channel: 1,
            class: 0,
            state: sim.units[owner].weapon.clone(),
        }];
        sim.units[owner].weapon_class = 1;
        sim.units[owner].cur1 = Some(0);
        let mut constants = sim.units[owner].consts;
        sim.force_constants(owner, &mut constants);
        assert_eq!(constants.energy_override, None);
        sim.units[owner].active_channel = 1;
        sim.force_constants(owner, &mut constants);
        assert_eq!(constants.energy_override, Some(-15.0));
        if let ForceKind::Choke { drain_rate, .. } =
            &mut sim.weapon_classes[0].force.as_mut().unwrap().kind
        {
            *drain_rate = 0.0;
        }
        let mut constants = sim.units[owner].consts;
        sim.force_constants(owner, &mut constants);
        assert_eq!(constants.energy_override, None, "native rate knockout");
    }

    #[test]
    fn choke_rate_replaces_recovery_and_uses_the_resource_depletion_latch() {
        let mut soldier = Soldier::new(Vec3::ZERO);
        let constants = SoldierConstants {
            energy_restore: 25.0,
            energy_restore_idle: 25.0,
            energy_override: Some(-15.0),
            ..Default::default()
        };
        soldier.energy = 60.0;
        for _ in 0..30 {
            soldier.step(TICK_DT, &Default::default(), &constants, GroundRules::Bf2);
        }
        assert!((soldier.energy - 45.0).abs() < 0.0001);
        soldier.energy = 0.25;
        soldier.step(TICK_DT, &Default::default(), &constants, GroundRules::Bf2);
        assert_eq!(soldier.energy, 0.0);
        assert!(soldier.energy_depleted);
        let normal = SoldierConstants {
            energy_override: None,
            ..constants
        };
        soldier.step(TICK_DT, &Default::default(), &normal, GroundRules::Bf2);
        assert!(soldier.energy > 0.0, "release restores normal recovery");
        assert!(soldier.energy_depleted, "recovery threshold still applies");
    }
}
