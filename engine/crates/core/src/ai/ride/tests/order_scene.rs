//! The order scene `get_out_tests` and `pile_in_tests` share.
use super::*;
use crate::sim::{DeathCause, Event};
use crate::squad::{Command, CommandKind};

pub(super) fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..PadState::default()
    }
}

pub(super) fn hold(s: &mut Sim, bot: UnitHandle, issuer: u8) {
    let u = s.unit_mut(bot).unwrap();
    u.command = Some(Command {
        kind: CommandKind::Hold(u.soldier.pos),
        issuer,
        issued_tick: 0,
    });
    u.ai.next_think = 9;
}

pub(super) fn scene(
    rules: GroundRules,
    kind: VehicleKind,
    armed: bool,
    pilot_first: bool,
    slot: u8,
) -> (Sim, UnitHandle, UnitHandle, VehicleHandle) {
    let (mut s, mut bot, v) = pickup_world(kind);
    s.ground_rules = rules;
    if armed {
        arm_gunner_seat(&mut s);
    }
    let pos = s.vehicle(v).unwrap().pos;
    // Reuse a dead slot through public spawn, so both handle index orders run.
    if pilot_first {
        s.kill_environment(bot, DeathCause::Fall);
    }
    let pilot = s.spawn(&UnitSpec::player_in_slot(slot, 0, pos)).unwrap();
    if pilot_first {
        bot = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, pos + Vec3::X * 4.0)
            })
            .unwrap();
    }
    s.unit_mut(bot).unwrap().soldier.pos = Vec3::new(pos.x + 4.0, 0.0, pos.z);
    assert!(s.enter_vehicle(pilot, v, 0));
    hold(&mut s, bot, slot);
    (s, bot, pilot, v)
}

pub(super) fn step(s: &mut Sim, slot: u8, buttons: u32) {
    let mut pads = vec![PadState::default(); usize::from(slot) + 1];
    pads[usize::from(slot)] = pad(buttons);
    s.step(&pads);
}

pub(super) fn takes(s: &Sim, bot: UnitHandle) -> Vec<bool> {
    s.events
        .iter()
        .filter_map(|e| match e {
            Event::OrderTaken { unit, did, .. } if *unit == bot => Some(*did),
            _ => None,
        })
        .collect()
}

pub(super) fn no_claim(s: &Sim, bot: UnitHandle) {
    assert!(s.units[bot.index as usize]
        .ai
        .ride
        .is_none_or(|r| r.board.is_none()
            && r.called_board.is_none()
            && r.player_board.is_none()
            && !r.use_req));
}
