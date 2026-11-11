use std::sync::Arc;

use glam::Vec3;

use super::{form_order, steer, takes_fight, SIGHT_FIGHT};
use crate::ai::{AiState, Behaviour};
use crate::droideka::{DroidekaClass, Order, ROLLING};
use crate::perception::Foe;
use crate::sim::{buttons, PadState, Sim};
use crate::unit::{UnitHandle, UnitSpec};

fn deka(team: u8, at: Vec3) -> UnitSpec {
    UnitSpec {
        max_health: 550.0,
        droideka: Some(Arc::new(DroidekaClass::default())),
        ..UnitSpec::trooper(team, at)
    }
}

fn world() -> (Sim, UnitHandle, UnitHandle) {
    let mut s = Sim::empty(3);
    let me = s.spawn(&deka(1, Vec3::ZERO)).unwrap();
    let foe = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -30.0)))
        .unwrap();
    (s, me, foe)
}

fn fighting(foe: UnitHandle) -> AiState {
    AiState {
        behaviour: Behaviour::Fight,
        target: Some(Foe::Unit(foe)),
        ..AiState::default()
    }
}

#[test]
fn the_form_order_deploys_only_to_fight() {
    for (b, want) in [
        (Behaviour::Fight, Order::Deploy),
        (Behaviour::Recover, Order::Roll),
        (Behaviour::None, Order::Roll),
        (Behaviour::Travel, Order::Roll),
        (Behaviour::Capture, Order::Roll),
        (Behaviour::Guard, Order::Roll),
        (Behaviour::Follow, Order::Roll),
        (Behaviour::Evade, Order::Roll),
    ] {
        let st = AiState {
            behaviour: b,
            ..AiState::default()
        };
        assert_eq!(form_order(&st), want, "{b:?}");
    }
}

#[test]
fn steer_clears_the_soldier_buttons_and_strafe_a_droideka_has_no_use_for() {
    let (s, me, _) = world();
    let misuse = buttons::ROLL_STANCE
        | buttons::JUMP
        | buttons::SECONDARY_FIRE
        | buttons::SPRINT
        | buttons::ACTION;
    let mut pad = PadState {
        buttons: misuse | buttons::FIRE,
        lx: PadState::quantize(0.7),
        ly: PadState::quantize(1.0),
        ..Default::default()
    };
    steer(&s, me, &AiState::default(), &mut pad);
    assert_eq!(pad.buttons, buttons::FIRE);
    assert_eq!(pad.lx, 0);
    assert_eq!(pad.ly, PadState::quantize(1.0));
}

#[test]
fn steer_leaves_an_ordinary_soldier_alone() {
    let (s, _, foe) = world();
    let pad0 = PadState {
        buttons: buttons::SPRINT | buttons::JUMP,
        lx: PadState::quantize(0.5),
        ..Default::default()
    };
    let mut pad = pad0;
    steer(&s, foe, &fighting(foe), &mut pad);
    assert_eq!(pad, pad0);
}

#[test]
fn a_deployed_fighter_stands_and_raises_its_shield_over_half() {
    let (mut s, me, foe) = world();
    let mut pad = PadState {
        buttons: buttons::FIRE,
        ly: PadState::quantize(1.0),
        ..Default::default()
    };
    steer(&s, me, &fighting(foe), &mut pad);
    assert_eq!(pad.ly, 0);
    assert_eq!(pad.buttons, buttons::SECONDARY_FIRE);

    s.units[me.index as usize]
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .points = 1500.0;
    let mut pad = PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    };
    steer(&s, me, &fighting(foe), &mut pad);
    assert_eq!(pad.buttons, buttons::FIRE, "at half it is not raised");

    let d = s.units[me.index as usize].droideka.as_mut().unwrap();
    d.shield.points = 3000.0;
    d.shield.on = true;
    let mut pad = PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    };
    steer(&s, me, &fighting(foe), &mut pad);
    assert_eq!(pad.buttons, buttons::FIRE, "up already: it fires");
}

#[test]
fn a_rolling_droideka_never_presses_the_shield() {
    let (mut s, me, foe) = world();
    s.units[me.index as usize].droideka.as_mut().unwrap().state = ROLLING;
    let mut pad = PadState {
        ly: PadState::quantize(1.0),
        ..Default::default()
    };
    steer(&s, me, &fighting(foe), &mut pad);
    assert_eq!(pad.buttons, 0);
    assert_eq!(pad.ly, PadState::quantize(1.0));
}

#[test]
fn sight_starts_a_droideka_fight_only_within_ten_metres() {
    let (mut s, me, foe) = world();
    let idle = AiState::default();
    let u = s.unit(me).unwrap();
    let e = s.unit(foe).unwrap();
    assert!(takes_fight(&s, u, &idle, e, SIGHT_FIGHT));
    assert!(!takes_fight(&s, u, &idle, e, SIGHT_FIGHT + 0.5));
    assert!(
        takes_fight(&s, u, &fighting(foe), e, 40.0),
        "a fight goes on"
    );
    let soldier = s.unit(foe).unwrap();
    assert!(
        takes_fight(&s, soldier, &idle, u, 40.0),
        "soldiers unchanged"
    );
    s.tick = 100;
    s.units[me.index as usize].last_hit = Some((foe, 95));
    let u = s.unit(me).unwrap();
    let e = s.unit(foe).unwrap();
    assert!(takes_fight(&s, u, &idle, e, 40.0), "a hit starts one");
}

#[test]
fn shield_debt_does_not_make_ai_raise_the_bubble() {
    let (mut s, me, foe) = world();
    for points in [0.0, -900.0] {
        let d = s.units[me.index as usize].droideka.as_mut().unwrap();
        d.shield.points = points;
        d.shield.on = false;
        let mut pad = PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        };
        steer(&s, me, &fighting(foe), &mut pad);
        assert_eq!(pad.buttons, buttons::FIRE);
    }
}
