//! [ours] (the maintainer, 2026-10-30): a human hero always turns during an
//! attack, whatever the combo's InputLock and TurnFactor author. A bot turns
//! through an InputLock too (the PS2's lock is a seven-bit mask with no turn,
//! `combo::output`) and keeps the state's TurnFactor.

use super::*;

fn locked_swing() -> Vec<F> {
    vec![
        field(K::Animation, vec![text("swing")], vec![]),
        field(K::Duration, vec![V::Number(1.0)], vec![]),
        field(K::InputLock, vec![text("All"), text("!Thrust")], vec![]),
        field(K::TurnFactor, vec![V::Number(0.25)], vec![]),
    ]
}

fn swinging(attack: Vec<F>) -> (Sim, UnitHandle, UnitHandle) {
    let (mut sim, player, other) = scene();
    sim.weapon_classes[0] = saber(program(attack));
    sim.unit_mut(player).unwrap().weapon.melee = None;
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    sim.step(&[PadState::default()]);
    (sim, player, other)
}

fn yaw(sim: &Sim, h: UnitHandle) -> f32 {
    sim.unit(h).unwrap().soldier.yaw
}

/// What the same stick turns a hero in a state that authors no lock.
fn free_turn(pad: PadState) -> f32 {
    let (mut sim, player, _) = swinging(vec![
        field(K::Animation, vec![text("swing")], vec![]),
        field(K::Duration, vec![V::Number(1.0)], vec![]),
    ]);
    let start = yaw(&sim, player);
    for _ in 0..5 {
        sim.step(&[pad]);
    }
    yaw(&sim, player) - start
}

#[test]
fn a_human_hero_turns_at_full_stick_speed_through_a_locked_factored_swing() {
    let pad = PadState {
        rx: 20000,
        ..Default::default()
    };
    let (mut sim, player, _) = swinging(locked_swing());
    let start = yaw(&sim, player);
    for _ in 0..5 {
        sim.step(&[pad]);
    }
    let turned = yaw(&sim, player) - start;
    assert_ne!(turned, 0.0, "the lock must not hold a human's stick");
    assert_eq!(turned, free_turn(pad), "TurnFactor must not slow a human");
}

#[test]
fn a_human_hero_turns_by_the_mouse_through_a_locked_swing() {
    let (mut sim, player, _) = swinging(locked_swing());
    let start = yaw(&sim, player);
    sim.step(&[PadState {
        mx: 6000,
        buttons: buttons::MOUSE_AIM,
        ..Default::default()
    }]);
    assert_ne!(yaw(&sim, player), start);
}

#[test]
fn a_bot_hero_turns_through_an_inputlock_all_at_the_turn_factor() {
    let (mut sim, player, _) = swinging(locked_swing());
    sim.unit_mut(player).unwrap().controller = crate::unit::Controller::Ai;
    sim.unit_mut(player).unwrap().is_player = false;
    let mut pads = [PadState {
        rx: 20000,
        mx: 6000,
        ..Default::default()
    }];
    sim.saber_inputs(&mut pads);
    assert_eq!((pads[0].rx, pads[0].mx), (20000, 6000));
    assert!(!sim.saber_look_locked(player.index as usize));
    let state = sim.unit(player).unwrap().weapon.melee.as_ref().unwrap();
    assert_ne!(
        state.input_lock & crate::combo::JUMP,
        0,
        "All still locks Jump"
    );
    let mut constants = sim.units[player.index as usize].consts;
    let full = constants.max_turn_speed;
    sim.saber_constants(player.index as usize, &mut constants);
    assert_eq!(constants.max_turn_speed, full * 0.25);
}

#[test]
fn the_locked_swing_still_locks_a_humans_buttons_and_keeps_his_walk() {
    let (mut sim, player, _) = swinging(locked_swing());
    let mut pads = [PadState {
        buttons: buttons::JUMP,
        ly: 20000,
        lx: 20000,
        ..Default::default()
    }];
    sim.saber_inputs(&mut pads);
    assert_eq!(pads[0].buttons & buttons::JUMP, 0, "Jump stays locked");
    assert_eq!(pads[0].ly, 20000, "!Thrust: walking stays");
    assert_eq!(pads[0].lx, 20000, "!Thrust frees the strafe with it");
    assert!(sim.unit(player).is_some());
}

/// The blade is posed in the body's frame, so turning sweeps it: a victim
/// beside the hero, out of the blade's reach at the swing's start, is cut
/// once the body has turned to him.
#[test]
fn turning_during_a_swing_sweeps_the_blade_onto_a_victim_beside_him() {
    let (mut sim, player, victim) = swinging(vec![
        field(K::Animation, vec![text("swing")], vec![]),
        field(K::Duration, vec![V::Number(1.0)], vec![]),
        field(K::InputLock, vec![text("All")], vec![]),
        field(
            K::Attack,
            vec![],
            vec![
                field(
                    K::DamageTime,
                    vec![V::Number(0.0), V::Number(30.0), text("Frames")],
                    vec![],
                ),
                field(K::Damage, vec![V::Number(100.0)], vec![]),
            ],
        ),
    ]);
    let (s, c) = crate::fmath::sin_cos(yaw(&sim, player));
    let pos = sim.unit(player).unwrap().soldier.pos;
    // The body faces (-s, 0, -c); stand him a metre to its side.
    let side = Vec3::new(c, 0.0, -s);
    sim.unit_mut(victim).unwrap().soldier.pos = pos + side;
    sim.unit_mut(victim).unwrap().soldier.vel = Vec3::ZERO;
    let before = sim.unit(victim).unwrap().health.cur;
    sim.unit_mut(player).unwrap().soldier.energy = 100.0;
    sim.step(&[PadState::default()]);
    sim.unit_mut(victim).unwrap().soldier.pos = pos + side;
    assert_eq!(sim.unit(victim).unwrap().health.cur, before, "out of reach");
    for _ in 0..20 {
        sim.step(&[PadState {
            mx: 6000,
            buttons: buttons::MOUSE_AIM,
            ..Default::default()
        }]);
        sim.unit_mut(victim).unwrap().soldier.pos = pos + side;
    }
    assert!(sim.unit(victim).unwrap().health.cur < before);
}

/// An attack's lunge is a local velocity, so it follows the turned body.
#[test]
fn a_turned_body_carries_the_attacks_lunge() {
    let (mut sim, player, _) = swinging(vec![
        field(K::Animation, vec![text("swing")], vec![]),
        field(K::Duration, vec![V::Number(1.0)], vec![]),
        field(K::InputLock, vec![text("All")], vec![]),
        field(
            K::AnimatedMove,
            vec![],
            vec![field(K::VelocityZ, vec![V::Number(6.0)], vec![])],
        ),
    ]);
    let start = yaw(&sim, player);
    for _ in 0..10 {
        sim.step(&[PadState {
            mx: 8000,
            buttons: buttons::MOUSE_AIM,
            ..Default::default()
        }]);
    }
    assert!((yaw(&sim, player) - start).abs() > 0.5, "he turned");
    for _ in 0..3 {
        sim.step(&[PadState::default()]);
    }
    let u = sim.unit(player).unwrap();
    let (s, c) = crate::fmath::sin_cos(u.soldier.yaw);
    let along = u.soldier.vel.dot(Vec3::new(-s, 0.0, -c));
    let across = u.soldier.vel.dot(Vec3::new(c, 0.0, -s));
    assert!(along > 1.0, "along {along}");
    assert!(across.abs() < along * 0.5, "across {across} along {along}");
}
