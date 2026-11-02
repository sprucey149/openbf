//! PS2 water-update timelines, through the complete fixed-step simulation.
use core::heightfield::Heightfield;
use core::sim::{buttons, DeathCause, Event, PadState, Sim};
use core::unit::{SlotSpec, UnitHandle, UnitSpec};
use core::water::WetMask;
use glam::Vec3;

fn scene(health: f32, depth: f32) -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 100.0,
        offset_x: 50.0,
        offset_z: 50.0,
        heights: vec![0.0; 4],
    });
    sim.water_level = Some(depth);
    sim.water_mask = WetMask::new(2, 50.0, [50.0, 50.0], &[1, 0, 0, 0]);
    let mut spec = UnitSpec::player(0, Vec3::new(-10.0, 0.0, -10.0));
    spec.max_health = health;
    spec.slots = vec![SlotSpec {
        slot: 1,
        channel: 1,
        weapon_class: 0,
        clips: 6,
    }]
    .into();
    let h = sim.spawn(&spec).unwrap();
    (sim, h)
}

#[test]
fn ps2_drowning_timelines_and_environment_credit() {
    for (health, depth, hits) in [(300.0, 7.28, 10), (400.0, 3.46, 14)] {
        let (mut sim, h) = scene(health, depth);
        sim.ground_rules = if health == 300.0 {
            core::soldier::GroundRules::Bf1
        } else {
            core::soldier::GroundRules::Bf2
        };
        let tickets = sim.battle.teams[0].tickets();
        let mut damages = Vec::new();
        let mut prev = health;
        for elapsed in 1..=90 + 30 * (hits - 1) {
            sim.step(&[PadState::default()]);
            let u = &sim.units[h.index as usize];
            if u.health.cur != prev {
                assert_eq!(prev - u.health.cur, 30.0);
                damages.push(elapsed);
                prev = u.health.cur;
            }
        }
        assert_eq!(damages, (0..hits).map(|n| 90 + 30 * n).collect::<Vec<_>>());
        assert!(!sim.units[h.index as usize].alive);
        assert_eq!(sim.units[h.index as usize].deaths, 1);
        assert_eq!(sim.battle.teams[0].tickets(), tickets - 1);
        assert!(sim
            .events
            .iter()
            .any(|e| matches!(e, Event::EnvDeath {unit, cause: DeathCause::Water} if *unit == h)));
        assert!(!sim.events.iter().any(|e| matches!(e, Event::Kill { .. })));
    }
}

#[test]
fn shallow_dry_cells_and_surfacing_reset_the_timer() {
    for depth in [1.5, 7.28] {
        let (mut sim, h) = scene(300.0, depth);
        if depth > 2.0 {
            sim.unit_mut(h).unwrap().soldier.pos.x = 10.0;
        }
        for _ in 0..600 {
            sim.step(&[PadState::default()]);
        }
        assert_eq!(sim.unit(h).unwrap().health.cur, 300.0);
    }
    for leave_cell in [false, true] {
        let (mut sim, h) = scene(300.0, 7.28);
        for _ in 0..89 {
            sim.step(&[PadState::default()]);
        }
        if leave_cell {
            sim.unit_mut(h).unwrap().soldier.pos.x = 10.0;
        } else {
            sim.water_level = Some(1.5);
        }
        sim.step(&[PadState::default()]);
        sim.unit_mut(h).unwrap().soldier.pos.x = -10.0;
        sim.water_level = Some(7.28);
        for _ in 0..89 {
            sim.step(&[PadState::default()]);
        }
        assert_eq!(sim.unit(h).unwrap().health.cur, 300.0);
        sim.step(&[PadState::default()]);
        assert_eq!(sim.unit(h).unwrap().health.cur, 270.0);
    }
}

#[test]
fn both_triggers_stop_under_the_fire_point_and_resume_above_it() {
    for trigger in [buttons::FIRE, buttons::SECONDARY_FIRE] {
        let (mut sim, h) = scene(300.0, 1.6);
        let pad = PadState {
            buttons: trigger,
            ..Default::default()
        };
        assert!((sim.unit(h).unwrap().fire_point().y - 1.583).abs() < 0.001);
        for _ in 0..30 {
            sim.step(&[pad]);
        }
        let u = sim.unit(h).unwrap();
        assert_eq!(u.weapon.shots_fired + u.held[0].state.shots_fired, 0);
        sim.water_level = Some(1.5);
        for _ in 0..30 {
            sim.step(&[pad]);
        }
        let u = sim.unit(h).unwrap();
        assert!(u.weapon.shots_fired + u.held[0].state.shots_fired > 0);
    }
}

#[test]
fn drowning_respects_invincibility_and_replay_hashes() {
    let (mut a, h) = scene(300.0, 7.28);
    let (mut b, _) = scene(300.0, 7.28);
    for _ in 0..120 {
        a.step(&[PadState::default()]);
        b.step(&[PadState::default()]);
        assert_eq!(a.hash(), b.hash());
    }
    assert_eq!(a.unit(h).unwrap().health.cur, 240.0);
    let (mut sim, h) = scene(300.0, 7.28);
    sim.cheats.invincible = true;
    for _ in 0..600 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.unit(h).unwrap().health.cur, 300.0);
}

#[test]
fn going_under_cancels_an_already_latched_salvo() {
    for trigger in [buttons::FIRE, buttons::SECONDARY_FIRE] {
        let (mut sim, h) = scene(300.0, 1.5);
        sim.weapon_classes[0].salvo_count = 4;
        sim.weapon_classes[0].salvo_delay = [0.15, 0.15];
        let pad = PadState {
            buttons: trigger,
            ..Default::default()
        };
        sim.step(&[pad]);
        let fired = |s: &Sim| {
            let u = s.unit(h).unwrap();
            u.weapon.shots_fired + u.held[0].state.shots_fired
        };
        assert_eq!(fired(&sim), 1);
        sim.water_level = Some(1.6);
        for _ in 0..30 {
            sim.step(&[pad]);
        }
        assert_eq!(
            fired(&sim),
            1,
            "the remaining rounds must not fire underwater"
        );
    }
}

#[test]
fn bob_reloads_breath_at_the_measured_shallow_boundary() {
    let (mut sim, h) = scene(300.0, 1.85);
    sim.water_oscillates = true;
    for _ in 0..600 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(
        sim.unit(h).unwrap().health.cur,
        300.0,
        "the 6 cm bob dips below the 1.80 m eye before breath runs out"
    );
    sim.water_oscillates = false;
    for _ in 0..90 {
        sim.step(&[PadState::default()]);
    }
    assert!(sim.unit(h).unwrap().health.cur < 300.0);
}

#[test]
fn breath_history_is_hashed_and_new_lives_start_with_full_breath() {
    let (mut sim, h) = scene(300.0, 7.28);
    sim.step(&[PadState::default()]);
    let after = sim.hash();
    assert_eq!(sim.unit(h).unwrap().soldier.drown_in, 89);
    sim.unit_mut(h).unwrap().soldier.drown_in = 90;
    assert_ne!(
        sim.hash(),
        after,
        "breath changes future damage despite equal health"
    );
    let u = sim.unit(h).unwrap();
    let respawn = u.spec.build(h.gen + 1, &sim.weapon_classes);
    assert_eq!(respawn.soldier.drown_in, 90);
}

#[test]
fn firing_gate_tracks_the_actual_pitched_muzzle() {
    let (mut sim, h) = scene(300.0, 1.6);
    sim.unit_mut(h).unwrap().soldier.pitch = 0.8;
    assert!(sim.unit(h).unwrap().fire_point().y > 1.6);
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    assert_eq!(sim.unit(h).unwrap().weapon.shots_fired, 1);
}

#[test]
fn crouch_uses_the_stance_eye_height_and_reset_does_not_erase_motion() {
    let (mut sim, h) = scene(300.0, 1.4);
    sim.unit_mut(h).unwrap().soldier.stance = core::soldier::Stance::Crouch;
    for _ in 0..90 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.unit(h).unwrap().health.cur, 270.0);
    sim.unit_mut(h).unwrap().soldier.stance = core::soldier::Stance::Stand;
    for _ in 0..90 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.unit(h).unwrap().health.cur, 270.0);
    let (mut dry, d) = scene(300.0, 0.0);
    let (mut wet, w) = scene(300.0, 1.4);
    for _ in 0..30 {
        let pad = PadState {
            ly: i16::MAX,
            ..Default::default()
        };
        dry.step(&[pad]);
        wet.step(&[pad]);
    }
    assert_eq!(
        dry.unit(d).unwrap().soldier.pos,
        wet.unit(w).unwrap().soldier.pos
    );
    assert_eq!(
        dry.unit(d).unwrap().soldier.vel,
        wet.unit(w).unwrap().soldier.vel
    );
}

#[test]
fn water_thresholds_are_inclusive_at_the_eye_but_strict_at_the_muzzle() {
    let (mut sim, h) = scene(300.0, 1.8);
    for _ in 0..90 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.unit(h).unwrap().health.cur, 270.0);
    let (mut sim, h) = scene(300.0, 0.0);
    sim.water_level = Some(sim.unit(h).unwrap().fire_point().y);
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    assert_eq!(sim.unit(h).unwrap().weapon.shots_fired, 1);
}
