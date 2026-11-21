//! Shared test fixture: a flat-ground walk with an armed auto-turret.
//! Re-exported at crate::walk::turret_test_walk so external test modules
//! keep their existing selector paths.

use super::*;

#[cfg(test)]
pub(crate) fn turret_test_walk() -> (WalkSim, usize, usize) {
    struct NoProps;
    impl core::weapon::PropSource for NoProps {
        fn prop(&self, _: &str) -> Option<&str> {
            None
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }
    let flat = core::heightfield::Heightfield {
        size: 33,
        unit: 8.0,
        offset_x: 128.0,
        offset_z: 128.0,
        heights: vec![0.0; 33 * 33],
    };
    let mut walk = WalkSim::new(42, Vec3::ZERO, Some(flat));
    let mut gun = core::weapon::WeaponClass::bf1_rifle();
    gun.ai_range = [0.0, 15.0, 40.0];
    let item = core::autoturret::TurretClass::new(&NoProps, gun);
    let mut w = core::weapon::WeaponClass::bf1_rifle();
    w.turret = Some(Box::new(core::weapon::Dispenser::new(
        &mut w, &NoProps, true, item,
    )));
    let weapon = walk.sim.add_weapon_class(w);
    let class = &walk.sim.weapon_classes[weapon]
        .turret
        .as_ref()
        .unwrap()
        .item;
    let gun = class.gun_index;
    let mut t = core::autoturret::Turret::new(
        1,
        walk.player,
        0,
        weapon,
        class,
        Vec3::new(15.0, core::autoturret::CENTRE, 0.0),
        Vec3::ZERO,
        0.0,
    );
    t.landed = true;
    walk.sim.turrets.push(t);
    let enemy = core::unit::UnitSpec::trooper(1, Vec3::new(15.0, 0.0, -20.0));
    walk.sim.spawn(&enemy).unwrap();
    (walk, weapon, gun)
}
