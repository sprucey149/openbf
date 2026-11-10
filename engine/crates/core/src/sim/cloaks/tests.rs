use super::*;
use crate::cloak::CloakClass;
use crate::sim::{buttons, AimTarget, DeathCause, PadState};
use crate::unit::{Controller, UnitHandle, UnitSpec};
use crate::weapon::{WeaponClass, WeaponState};
use glam::Vec3;

fn scene() -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(crate::sim::tests::flat_ground());
    let mut spec = UnitSpec::player(0, Vec3::ZERO);
    let class = WeaponClass {
        cloak: Some(CloakClass {
            energy_cost: 40.0,
            energy_drain: 17.0,
            delay: 0.3,
            linger: 1.0,
            moving: 1.0,
            still: 1.0,
            fast_speed: 9.5,
            slow_speed: 6.5,
            fade_in: 0.6,
            fade_out: 0.9,
            flicker: 0.6,
            flicker_interval: [5.0, 10.0],
        }),
        ..WeaponClass::default()
    };
    spec.weapon_class = sim.add_weapon_class(class);
    spec.consts.energy_restore = 0.0;
    spec.consts.energy_restore_idle = 0.0;
    let owner = sim.spawn(&spec).unwrap();
    (sim, owner)
}

fn press(sim: &mut Sim, buttons: u32) {
    sim.step(&[PadState {
        buttons,
        ..Default::default()
    }]);
}

fn activate(sim: &mut Sim, owner: UnitHandle) {
    press(sim, buttons::FIRE);
    for _ in 0..12 {
        press(sim, 0);
    }
    assert!(sim.unit(owner).unwrap().cloak.active);
}

#[test]
fn cloak_cost_delay_edges_drain_and_recharge() {
    let (mut sim, owner) = scene();
    sim.unit_mut(owner).unwrap().soldier.energy = 39.0;
    press(&mut sim, buttons::FIRE);
    assert!(sim.unit(owner).unwrap().cloak.warmup.is_none());
    press(&mut sim, 0);
    sim.unit_mut(owner).unwrap().soldier.energy = 100.0;
    press(&mut sim, buttons::FIRE);
    assert_eq!(sim.unit(owner).unwrap().soldier.energy, 60.0);
    assert!(!sim.unit(owner).unwrap().cloak.active);
    for _ in 0..15 {
        press(&mut sim, buttons::FIRE);
    }
    assert!(sim.unit(owner).unwrap().cloak.active);
    assert!(sim.unit(owner).unwrap().soldier.energy < 60.0);
    assert!(sim.bolts.is_empty());
    for _ in 0..150 {
        press(&mut sim, 0);
    }
    let u = sim.unit(owner).unwrap();
    assert!(!u.cloak.active && u.cloak.visible());
    assert_eq!(u.soldier.energy, 0.0);
    sim.unit_mut(owner).unwrap().consts.energy_restore_idle = 15.0;
    for _ in 0..120 {
        press(&mut sim, 0);
    }
    assert!(sim.unit(owner).unwrap().soldier.energy > 50.0);
    press(&mut sim, buttons::FIRE);
    assert!(sim.unit(owner).unwrap().cloak.warmup.is_some());
}

#[test]
fn damage_reveals_without_cancelling_and_switch_death_clean_up() {
    let (mut sim, owner) = scene();
    activate(&mut sim, owner);
    for _ in 0..45 {
        press(&mut sim, 0);
    }
    assert!(!sim.unit(owner).unwrap().cloak.visible());
    sim.finish_unit_damage(owner, None, 1.0, 0.0, Vec3::ZERO);
    let u = sim.unit(owner).unwrap();
    assert!(u.cloak.active && u.cloak.visible());
    press(&mut sim, 0);
    assert!(!sim.unit(owner).unwrap().cloak.visible());
    let state = WeaponState::new(&sim.weapon_classes[0], true, 0);
    sim.unit_mut(owner)
        .unwrap()
        .held
        .push(crate::unit::HeldWeapon {
            slot: 1,
            channel: 0,
            class: 0,
            state,
        });
    press(&mut sim, buttons::SWITCH_WEAPON);
    assert!(!sim.unit(owner).unwrap().cloak.active);
    for _ in 0..70 {
        press(&mut sim, 0);
    }
    assert!(sim.unit(owner).unwrap().cloak.visible());
    sim.kill_environment(owner, DeathCause::Suicide);
    assert_eq!(sim.units[owner.index as usize].cloak, CloakState::default());
}

#[test]
fn cloak_scales_sight_without_tracking_a_hidden_enemys_position() {
    let (mut sim, owner) = scene();
    let enemy = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
        .unwrap();
    let tier = crate::skill::Difficulty::Normal.tier();
    let mut ai = crate::ai::AiState::default();
    crate::perception::scan(&sim, owner, &mut ai, tier);
    let before = crate::perception::contact_of(&ai, enemy).unwrap().last_pos;
    sim.unit_mut(enemy).unwrap().cloak.strength = 1.0;
    sim.unit_mut(enemy).unwrap().soldier.pos.x += 5.0;
    sim.tick += 5;
    crate::perception::scan(&sim, owner, &mut ai, tier);
    let c = crate::perception::contact_of(&ai, enemy).unwrap();
    assert!(!c.seen_now);
    assert_eq!(c.last_pos, before);
    assert!(!crate::perception::fireable(&c, sim.tick, 0));
    sim.tick += crate::perception::MEMORY_TICKS + 1;
    crate::perception::scan(&sim, owner, &mut ai, tier);
    assert!(crate::perception::contact_of(&ai, enemy).is_none());
}

#[test]
fn bots_cloak_without_threat_and_cloak_state_replays() {
    let (mut a, owner) = scene();
    a.unit_mut(owner).unwrap().controller = Controller::Ai;
    assert_eq!(crate::ai::ability_input(&a, owner), Some(buttons::FIRE));
    a.unit_mut(owner).unwrap().controller = Controller::Player { slot: 0 };
    let (mut b, _) = scene();
    for tick in 0..300 {
        let buttons = if tick == 0 || tick == 50 {
            buttons::FIRE
        } else {
            0
        };
        press(&mut a, buttons);
        press(&mut b, buttons);
        assert_eq!(a.hash(), b.hash(), "tick {tick}");
    }
    let before = a.hash();
    a.unit_mut(owner).unwrap().cloak.flicker_left += 1.0;
    assert_ne!(a.hash(), before);
}

#[test]
fn native_ps2_flicker_hides_partial_concealment_and_reveals_only_at_zero() {
    let (mut sim, owner) = scene();
    sim.cheats.infinite_stamina = true;
    let class = sim.unit(owner).unwrap().weapon_class;
    sim.weapon_classes[class].cloak.as_mut().unwrap().moving = 0.3;
    let u = sim.unit_mut(owner).unwrap();
    u.cloak = CloakState {
        active: true,
        strength: 1.0,
        flicker_left: crate::TICK_DT * 0.5,
        health: u.health.cur,
        ..Default::default()
    };
    let mut expected_rng = sim.lcg.clone();
    let interval = expected_rng.uniform(5.0, 10.0);
    sim.tick_cloaks(crate::TICK_DT);
    let cloak = sim.unit(owner).unwrap().cloak;
    assert_eq!(cloak.strength.to_bits(), 0x3ecc_cccc);
    assert_eq!((cloak.concealment() * 255.0) as u8, 101);
    assert!(!cloak.visible());
    assert_eq!(cloak.flicker_left, interval);
    assert_eq!(sim.lcg.state(), expected_rng.state());
    for _ in 0..31 {
        sim.tick_cloaks(crate::TICK_DT);
        assert!(!sim.unit(owner).unwrap().cloak.visible());
    }
    assert_eq!(sim.unit(owner).unwrap().cloak.strength, 1.0);
    let u = sim.unit_mut(owner).unwrap();
    u.soldier.vel = Vec3::Z * 9.5;
    u.cloak.strength = 0.3;
    u.cloak.flicker_left = crate::TICK_DT * 0.5;
    sim.tick_cloaks(crate::TICK_DT);
    assert!(sim.unit(owner).unwrap().cloak.visible());
    sim.tick_cloaks(crate::TICK_DT);
    assert!(!sim.unit(owner).unwrap().cloak.visible());
}

#[test]
fn a_portable_turret_drops_and_cannot_reacquire_a_concealed_soldier() {
    let (mut sim, owner) = scene();
    let enemy = sim
        .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, -10.0)))
        .unwrap();
    let mut gun = WeaponClass::bf1_rifle();
    gun.ai_range = [15.0, 30.0, 40.0];
    let mut class = crate::autoturret::TurretClass::new(
        &crate::remote::tests::Props(crate::autoturret::tests::BF2_TURRET),
        gun.clone(),
    );
    class.gun_index = sim.add_weapon_class(gun);
    let mut weapon = WeaponClass::default();
    weapon.turret = Some(Box::new(crate::weapon::Dispenser {
        item: class.clone(),
        velocity: 0.0,
        strength: [0.0; 2],
        max_items: 1,
    }));
    let index = sim.add_weapon_class(weapon);
    let mut turret = crate::autoturret::Turret::new(
        1,
        owner,
        0,
        index,
        &class,
        Vec3::Y * crate::autoturret::CENTRE,
        Vec3::ZERO,
        0.0,
    );
    turret.landed = true;
    sim.turrets.push(turret);
    press(&mut sim, 0);
    assert_eq!(
        sim.turrets[0].target,
        Some(crate::autoturret::Aim::Unit(enemy))
    );
    sim.unit_mut(enemy).unwrap().cloak.strength = 1.0;
    press(&mut sim, 0);
    assert!(sim.turrets[0].target.is_none());
    for _ in 0..40 {
        press(&mut sim, 0);
        assert!(sim.turrets[0].target.is_none());
    }
}

/// Playtest C6: the cloak hides, it does not protect. The PS2's bolt sweep
/// and damage receiver never read the concealment byte (it only starts the
/// flicker), so a hidden Bothan takes a human's or a bot's bolt exactly as a
/// visible one does; only acquisition skips him (`analysis/re-cloaking.md`).
#[test]
fn a_cloaked_soldier_is_still_hit_by_bolts() {
    let cloak_class = || CloakClass {
        energy_cost: 40.0,
        energy_drain: 17.0,
        delay: 0.3,
        linger: 1.0,
        moving: 1.0,
        still: 1.0,
        fast_speed: 9.5,
        slow_speed: 6.5,
        fade_in: 0.6,
        fade_out: 0.9,
        flicker: 0.6,
        flicker_interval: [5.0, 10.0],
    };
    // The human's aim point, the damage the target took and his cloak just
    // after the hit.
    fn run(cloak: CloakClass, cloaked: bool, human: bool) -> (Vec3, f32, CloakState) {
        let mut s = Sim::empty(7);
        s.heightfield = Some(crate::sim::tests::flat_ground());
        let class = s.add_weapon_class(WeaponClass {
            cloak: Some(cloak),
            ..WeaponClass::default()
        });
        let shooter = if human {
            UnitSpec::player(0, Vec3::ZERO)
        } else {
            UnitSpec::trooper(0, Vec3::ZERO)
        };
        let p = s.spawn(&shooter).unwrap();
        let t = s
            .spawn(&UnitSpec {
                weapon_class: class,
                ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0))
            })
            .unwrap();
        let u = s.unit_mut(t).unwrap();
        // The visible control cannot afford to start cloaking itself.
        u.soldier.energy = if cloaked { 100.0 } else { 0.0 };
        if cloaked {
            u.cloak = CloakState {
                active: true,
                strength: 1.0,
                health: u.health.cur,
                flicker_left: 100.0,
                ..Default::default()
            };
        }
        let hidden = !s.unit(t).unwrap().cloak.visible();
        assert_eq!(hidden, cloaked);
        // No reticle lock or assist on what cannot be seen.
        assert_eq!(s.aim_candidate(AimTarget::Unit(t)).is_none(), cloaked);
        let aim = s.aim_hit(p.index as usize);
        if human {
            s.step(&[crate::sim::tests::fire()]);
        } else {
            let from = Vec3::new(0.0, 1.15, 0.0);
            let b = crate::projectile::Bolt::fire(
                from,
                Vec3::new(0.0, 1.15, -20.0) - from,
                80.0,
                1.5,
                crate::projectile::Shooter {
                    owner: p,
                    team: 0,
                    weapon_class: 0,
                    is_player: false,
                },
                s.heightfield.as_ref(),
                s.water_plane(),
                &s.collision,
                crate::collision::HullFilter::All,
            );
            s.bolts.push(b);
        }
        for _ in 0..60 {
            let hit: f32 = crate::sim::tests::hits(&s)
                .iter()
                .filter(|(v, _)| *v == t.index)
                .map(|(_, d)| d)
                .sum();
            if hit > 0.0 {
                return (aim, hit, s.unit(t).unwrap().cloak);
            }
            press(&mut s, 0);
        }
        panic!("no hit (cloaked {cloaked}, human {human})");
    }
    for human in [true, false] {
        let (aim_seen, seen, _) = run(cloak_class(), false, human);
        let (aim_hidden, hidden, after) = run(cloak_class(), true, human);
        // The reticle's ray still stops on the hidden body.
        assert_eq!(aim_hidden, aim_seen);
        if human {
            assert!(aim_hidden.z > -21.0, "{aim_hidden}");
        }
        assert!(hidden > 0.0);
        assert_eq!(hidden, seen, "human {human}");
        // The hit reveals him without ending the cloak.
        assert!(after.active);
        assert!(after.strength < 0.1, "{after:?}");
    }
}
