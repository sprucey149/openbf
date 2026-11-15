use super::*;
use crate::heightfield::Heightfield;
use crate::sim::PadState;
use crate::soldier::GroundRules;
use crate::unit::UnitSpec;

#[test]
fn threshold_scale_and_contact_direction_follow_the_class() {
    let mut c = SoldierConstants::default();
    assert_eq!(damage(&c, -Vec3::Y * 13.0, Vec3::Y, 0.0), 0.0);
    assert_eq!(damage(&c, -Vec3::Y * 16.0, Vec3::Y, 0.0), 9.0);
    assert_eq!(damage(&c, Vec3::Y * 16.0, Vec3::Y, 0.0), 0.0);
    assert_eq!(damage(&c, Vec3::new(40.0, -16.0, 0.0), Vec3::Y, 0.0), 9.0);
    c.collision_threshold = [4.0, 2.0, 1.0];
    c.collision_damage_scale = [2.0, 1.0, 0.5];
    let n = Vec3::new(0.0, 0.8, 0.6);
    let hit = damage(&c, -n * 10.0, n, 0.0);
    assert!((hit - 62.5).abs() < 1e-4, "{hit}");
    let reverse = damage(&c, -n * 10.0, n, std::f32::consts::PI);
    assert!((reverse - 44.764).abs() < 1e-4, "{reverse}");
    c.collision_damage_scale = [0.0; 3];
    assert_eq!(damage(&c, -Vec3::Y * 50.0, Vec3::Y, 0.0), 0.0);
}

fn drop(
    height: f32,
    rules: GroundRules,
    human: bool,
    immune: bool,
    cheat: bool,
) -> (Sim, crate::unit::UnitHandle, usize) {
    let mut sim = Sim::empty(43);
    sim.ground_rules = rules;
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 100.0,
        offset_x: 50.0,
        offset_z: 50.0,
        heights: vec![0.0; 4],
    });
    sim.cheats.invincible = cheat;
    let mut spec = if human {
        UnitSpec::player(0, Vec3::ZERO)
    } else {
        UnitSpec::trooper(0, Vec3::ZERO)
    };
    spec.max_health = 100.0;
    spec.consts.collision_damage_immune = immune;
    let h = sim.spawn(&spec).unwrap();
    let u = sim.unit_mut(h).unwrap();
    u.soldier.pos.y = height;
    u.soldier.grounded = false;
    let mut hits = 0;
    for _ in 0..120 {
        sim.step(&[PadState::default()]);
        hits += sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::FallDamage { victim, .. } if *victim == h))
            .count();
    }
    (sim, h, hits)
}

#[test]
fn ordinary_landings_damage_once_and_small_drops_do_not() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for human in [false, true] {
            let (sim, h, hits) = drop(8.0, rules, human, false, false);
            let u = sim.unit(h).unwrap();
            assert!(u.soldier.grounded);
            assert!(
                u.health.cur > 70.0 && u.health.cur < 95.0,
                "{}",
                u.health.cur
            );
            assert_eq!(hits, 1);
            assert_eq!(sim.battle.teams[0].tickets(), 250);
            let (sim, h, hits) = drop(2.0, rules, human, false, false);
            assert_eq!(sim.unit(h).unwrap().health.cur, 100.0);
            assert_eq!(hits, 0);
        }
    }
}

#[test]
fn fatal_falls_immunity_and_player_cheat_keep_their_bookkeeping() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for human in [false, true] {
            for (immune, cheat) in [(false, false), (true, false), (false, true)] {
                let (sim, h, hits) = drop(30.0, rules, human, immune, cheat);
                let spared = immune || human && cheat;
                assert_eq!(sim.unit(h).is_some(), spared);
                assert_eq!(hits, usize::from(!spared));
                assert_eq!(
                    sim.battle.teams[0].tickets(),
                    if spared { 250 } else { 249 }
                );
                assert_eq!(sim.units[h.index as usize].deaths, u32::from(!spared));
            }
        }
    }
}

#[test]
fn authored_parameters_are_hashed_for_active_and_respawn_state() {
    let mut sim = Sim::empty(43);
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let base = sim.hash();
    sim.unit_mut(h).unwrap().consts.collision_threshold[0] = 11.0;
    let active = sim.hash();
    assert_ne!(base, active);
    let u = sim.unit_mut(h).unwrap();
    u.consts.collision_threshold[0] = 10.0;
    u.spec.consts.collision_threshold[0] = 11.0;
    assert_ne!(base, sim.hash());
    assert_ne!(active, sim.hash());
}
