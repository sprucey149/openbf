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

/// Two teams with a post each on flat ground (team 0's at the origin, team 1's 300 m west), a
/// first player on team 0 and one further local player in slot 1 on team 1 (issue #194).
#[cfg(test)]
pub(crate) fn two_locals_walk() -> WalkSim {
    let mut w = posts_walk();
    w.add_local_player(1, Some(1)).expect("slot 1 is free");
    w
}

/// Every local player of `w` spawns through their own spawn screens, from their own pad.
#[cfg(test)]
pub(crate) fn spawn_locals(w: &mut WalkSim) {
    let n = w.local_count();
    let tap = |w: &mut WalkSim, slot: usize| {
        let mut p = vec![PadState::default(); n];
        p[slot].buttons = core::sim::buttons::SPAWN_ACCEPT;
        w.step_tick(&p);
        w.step_tick(&vec![PadState::default(); n]);
    };
    for i in 0..n {
        let slot = usize::from(w.local_slot_of(i));
        w.sim.open_spawn_screens(slot, false);
        tap(w, slot);
        tap(w, slot);
    }
    for i in 0..n {
        assert!(w.sim.unit(w.local_unit(i)).is_some_and(|u| u.alive));
    }
}

/// [`two_locals_walk`]'s ground and posts with the first player alone.
#[cfg(test)]
pub(crate) fn posts_walk() -> WalkSim {
    let flat = core::heightfield::Heightfield {
        size: 33,
        unit: 8.0,
        offset_x: 128.0,
        offset_z: 128.0,
        heights: vec![0.0; 33 * 33],
    };
    let mut w = WalkSim::new(7, Vec3::ZERO, Some(flat));
    for (x, team) in [(0.0f32, 0u8), (-300.0, 1)] {
        let pos = Vec3::new(x, 0.0, 0.0);
        w.sim.add_cp(
            core::cp::CpDef {
                name: format!("cp{x}"),
                pos,
                capture: core::region::Region::sphere(pos, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(pos + Vec3::new(0.0, 0.0, 1.0), 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(team),
        );
    }
    w
}
