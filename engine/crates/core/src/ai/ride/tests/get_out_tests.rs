//! GETOUT: public seating and Left edges; delivery, lifecycle and own search.
use super::*;
use crate::sim::{buttons, DeathCause, Event};
use crate::squad::{Command, CommandKind, Order};

use super::order_scene::*;

/// Planner witness: source-only fixtures driven by public seating and actual Left edges.
#[test]
fn planner_everyone_out_actual_edges() {
    let mut faults = 0;
    let mut controls = 0;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for armed in [false, true] {
                for first in [false, true] {
                    for mode in ["pilot_left", "pilot_idle", "foot_left", "pilot_right"] {
                        let (mut s, bot, pilot, v) = scene(rules, kind, armed, first, 0);
                        if mode == "pilot_left" || mode == "pilot_idle" {
                            assert!(s.enter_vehicle(bot, v, 1));
                            s.unit_mut(bot).unwrap().ai.next_think = 9;
                        } else if mode == "foot_left" {
                            s.exit_vehicle(pilot);
                        }
                        let mut commands = Vec::new();
                        let mut answers = Vec::new();
                        let mut left = None;
                        let mut boarded = None;
                        for t in 0..16u64 {
                            let b = if t < 3 {
                                match mode {
                                    "pilot_left" | "foot_left" => buttons::SQUAD_AT_EASE,
                                    "pilot_right" => buttons::SQUAD_FOLLOW,
                                    _ => 0,
                                }
                            } else {
                                0
                            };
                            step(&mut s, 0, b);
                            for e in &s.events {
                                if let Event::Command { slot, kind, units } = *e {
                                    commands.push((t, slot, kind, units));
                                }
                            }
                            for did in takes(&s, bot) {
                                answers.push((t, did));
                            }
                            if s.unit(bot).unwrap().mount.is_none() {
                                left.get_or_insert(t);
                            }
                            if s.unit(bot)
                                .unwrap()
                                .mount
                                .is_some_and(|m| m.vehicle == v && m.seat == 1)
                            {
                                boarded.get_or_insert(t);
                            }
                            if mode != "foot_left" {
                                assert!(s
                                    .unit(pilot)
                                    .unwrap()
                                    .mount
                                    .is_some_and(|m| m.vehicle == v && m.seat == 0));
                            }
                        }
                        if mode == "pilot_left" {
                            let passed = commands == [(0, 0, 5, 1)]
                                && answers == [(9, true)]
                                && left == Some(9);
                            eprintln!("GETOUT {rules:?}/{kind:?}/armed={armed}/pilot_first={first}: commands={commands:?} answers={answers:?} left={left:?} cooldown={:?}", s.unit(bot).unwrap().ai.ride.map(|r|r.block_until));
                            faults += usize::from(!passed);
                        } else if mode == "pilot_idle" {
                            assert!(commands.is_empty() && answers.is_empty());
                            assert_eq!(left, None);
                            controls += 1;
                        } else if mode == "foot_left" {
                            assert_eq!(commands, [(0, 0, Order::AtEase as u8, 1)]);
                            assert_eq!(answers, [(9, true)]);
                            assert_eq!(s.unit(bot).unwrap().command, None);
                            controls += 1;
                        } else {
                            assert_eq!(commands, [(0, 0, Order::PileIn as u8, 1)]);
                            assert_eq!(answers, [(9, true)]);
                            assert_eq!(boarded, Some(9));
                            controls += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!("GETOUT TOTAL faults={faults}/16 controls={controls}/48");
    assert_eq!(controls, 48);
    assert_eq!(
        faults, 0,
        "pilot Left must issue EVERYONE OUT and evict at the receiver think"
    );
}

fn seated(
    rules: GroundRules,
    first: bool,
    slot: u8,
) -> (Sim, UnitHandle, UnitHandle, VehicleHandle) {
    let (mut s, bot, pilot, v) = scene(rules, VehicleKind::Hover, true, first, slot);
    assert!(s.enter_vehicle(bot, v, 1));
    (s, bot, pilot, v)
}

fn vehicle(s: &mut Sim) -> VehicleHandle {
    s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(12.0, 2.0, 0.0),
        yaw: std::f32::consts::PI,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    })
}

#[test]
fn get_out_exact_deadline_survives_steps_and_explicit_pile_in_at_fifteen_seconds() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for explicit in [false, true] {
            let (mut s, bot, pilot, v) = seated(rules, false, 0);
            for t in 0..=9 {
                step(&mut s, 0, if t < 3 { buttons::SQUAD_AT_EASE } else { 0 });
            }
            let deadline = 9 + 600;
            assert!(s.unit(bot).unwrap().mount.is_none());
            assert_eq!(
                s.unit(bot).unwrap().ai.ride.unwrap().own_search_until,
                deadline
            );
            no_claim(&s, bot);
            while s.tick < 459 {
                step(&mut s, 0, 0);
            }
            assert_eq!(
                s.unit(bot).unwrap().ai.ride.unwrap().own_search_until,
                deadline
            );
            if explicit {
                // The normal pad, claim arbitration and physical entry bypass own-search.
                s.unit_mut(bot).unwrap().soldier.pos = s.vehicle(v).unwrap().pos + Vec3::X * 4.0;
                s.unit_mut(bot).unwrap().ai.next_think = s.tick;
                step(&mut s, 0, buttons::SQUAD_FOLLOW);
                assert_eq!(takes(&s, bot), [true]);
                assert_eq!(s.unit(bot).unwrap().mount.unwrap().vehicle, v);
                assert!(s.tick < deadline);
            } else {
                // Make an autonomous claim legally available, with a positive
                // no-cooldown control at the *same* tick/position/vehicle.
                s.exit_vehicle(pilot);
                s.unit_mut(bot).unwrap().soldier.pos = s.vehicle(v).unwrap().pos + Vec3::X * 4.0;
                for now in [deadline - 1, deadline] {
                    s.tick = now;
                    let u = s.unit(bot).unwrap();
                    let mut st = u.ai;
                    st.target_cp = Some(0);
                    st.ride.as_mut().unwrap().block_until = 0;
                    let mut control = st;
                    control.ride.as_mut().unwrap().own_search_until = 0;
                    claim(&s, bot, u, &mut control);
                    assert_eq!(control.ride.unwrap().board, Some(v), "eligible control");
                    claim(&s, bot, u, &mut st);
                    assert_eq!(st.ride.unwrap().board, (now == deadline).then_some(v));
                }
            }
        }
    }
}

#[test]
fn get_out_seat_membership_delays_and_humans_are_bound_at_the_press() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for first in [false, true] {
            for count in 0..=3usize {
                let (mut s, bot, pilot, v) = seated(rules, first, 0);
                s.exit_vehicle(bot);
                s.vehicle_mut(v).unwrap().seats.resize(
                    5,
                    crate::vehicle::Seat {
                        kind: SeatKind::Passenger(0),
                        occupant: None,
                    },
                );
                s.seat_setup[0].as_mut().unwrap().boardable = vec![true; 5];
                let mut bots = Vec::new();
                for j in 0..count {
                    let h = if j == 0 {
                        bot
                    } else {
                        s.spawn(&UnitSpec {
                            controller: Controller::Ai,
                            ..UnitSpec::trooper(0, Vec3::new(16.0, 0.0, 0.0))
                        })
                        .unwrap()
                    };
                    assert!(s.enter_vehicle(h, v, j as u8 + 1));
                    s.unit_mut(h).unwrap().ai.next_think = 9 + j as u64;
                    bots.push(h);
                }
                let human = s
                    .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(16.0, 0.0, 0.0)))
                    .unwrap();
                assert!(s.enter_vehicle(human, v, 4));
                let other_v = vehicle(&mut s);
                let other = s
                    .spawn(&UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(0, Vec3::new(16.0, 0.0, 0.0))
                    })
                    .unwrap();
                assert!(s.enter_vehicle(other, other_v, 0));
                // Keep the other driver from choosing an ordinary exit.
                s.unit_mut(other).unwrap().ai.next_think = 100;
                for t in 0..16 {
                    step(&mut s, 0, if t < 3 { buttons::SQUAD_AT_EASE } else { 0 });
                    if t == 0 {
                        assert!(s.events.iter().any(|e| matches!(e, Event::Command {kind:5, units, ..} if usize::from(*units)==count)));
                    }
                    for (j, h) in bots.iter().enumerate() {
                        assert_eq!(s.unit(*h).unwrap().mount.is_none(), t >= 9 + j as u64);
                        assert_eq!(
                            takes(&s, *h),
                            if t == 9 + j as u64 {
                                vec![true]
                            } else {
                                vec![]
                            }
                        );
                    }
                    assert!(s.unit(pilot).unwrap().mount.is_some());
                    assert!(s.unit(human).unwrap().mount.is_some());
                    assert_eq!(s.unit(other).unwrap().mount.unwrap().vehicle, other_v);
                    if count == 0 {
                        assert!(s.unit(bot).unwrap().mount.is_none());
                        assert!(takes(&s, bot).is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn get_out_held_release_repress_slots_and_newer_order() {
    for slot in [0, 1, 3] {
        let (mut s, bot, _, _) = seated(GroundRules::Bf2, false, slot);
        let mut presses = Vec::new();
        let mut answers = Vec::new();
        for t in 0..25 {
            step(
                &mut s,
                slot,
                if t < 3 || (5..8).contains(&t) || t >= 20 {
                    buttons::SQUAD_AT_EASE
                } else {
                    0
                },
            );
            for e in &s.events {
                if let Event::Command {
                    slot: issuer,
                    kind,
                    units,
                } = *e
                {
                    assert_eq!((issuer, kind, units), (slot, 5, u8::from(t < 9)));
                    presses.push(t);
                }
            }
            answers.extend(takes(&s, bot).into_iter().map(|_| t));
        }
        assert_eq!(presses, [0, 5, 20]);
        assert_eq!(answers, [9]);
    }
    // A newer PILE IN reaches foot bots, so it cannot replace a seated
    // recipient's order. An actual on-foot Hold from another player can.
    let (mut s, bot, _, _) = seated(GroundRules::Bf1, false, 0);
    s.spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(16.0, 0.0, 0.0)))
        .unwrap();
    step(&mut s, 0, buttons::SQUAD_AT_EASE);
    s.exit_vehicle(bot);
    s.unit_mut(bot).unwrap().soldier.pos = Vec3::new(16.0, 0.0, 0.0);
    step(&mut s, 1, buttons::SQUAD_HOLD);
    for _ in 2..=9 {
        step(&mut s, 0, 0);
    }
    assert_eq!(s.unit(bot).unwrap().command.unwrap().issuer, 1);
    assert!(matches!(
        s.unit(bot).unwrap().command.unwrap().kind,
        CommandKind::Hold(_)
    ));
    assert!(s
        .unit(bot)
        .unwrap()
        .ai
        .ride
        .is_none_or(|r| r.own_search_until == 0));
}

#[test]
fn get_out_replaces_old_follow_hold_and_pending_board_business() {
    for first in [false, true] {
        for follow in [false, true] {
            let (mut s, bot, pilot, v) = seated(GroundRules::Bf1, first, 0);
            if follow {
                s.unit_mut(bot).unwrap().command.as_mut().unwrap().kind =
                    CommandKind::Follow(pilot);
            }
            step(&mut s, 0, buttons::SQUAD_AT_EASE);
            // Retained stale call and claim represent older boarding business.
            let ai = &mut s.unit_mut(bot).unwrap().ai;
            ai.pending_board = Some((bot, v));
            let r = ai.ride.as_mut().unwrap();
            r.board = Some(v);
            r.called_board = Some(bot);
            r.ordered_call = true;
            r.player_board = Some(crate::squad::BoardIssuer {
                slot: 0,
                leader: pilot,
            });
            r.maintenance = Some(crate::ai::state::BoardMaintenance {
                owner: bot,
                tick: 0,
            });
            for _ in 1..30 {
                step(&mut s, 0, 0);
            }
            assert!(s.unit(bot).unwrap().mount.is_none());
            assert_eq!(s.unit(bot).unwrap().command, None);
            assert_eq!(s.unit(bot).unwrap().ai.pending_board, None);
            no_claim(&s, bot);
            assert!(s.unit(bot).unwrap().ai.ride.unwrap().maintenance.is_none());
        }
    }
    let (mut s, bot, _, _) = seated(GroundRules::Bf2, false, 0);
    s.spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(16.0, 0.0, 0.0)))
        .unwrap();
    s.unit_mut(bot).unwrap().command.as_mut().unwrap().issuer = 1;
    for t in 0..=9 {
        step(&mut s, 0, if t == 0 { buttons::SQUAD_AT_EASE } else { 0 });
    }
    assert_eq!(
        s.unit(bot).unwrap().command.unwrap().issuer,
        1,
        "other player's ownership"
    );
}

#[test]
fn get_out_lifecycle_never_redirects_or_survives_a_seat_tenure() {
    for case in [
        "issuer_reenter",
        "issuer_switch",
        "issuer_reuse",
        "receiver_reenter",
        "receiver_switch",
        "receiver_reuse",
        "replace",
        "vehicle_reuse",
    ] {
        let (mut s, bot, pilot, v) = seated(GroundRules::Bf2, false, 0);
        step(&mut s, 0, buttons::SQUAD_AT_EASE);
        let mut watched = bot;
        match case {
            "issuer_reenter" | "issuer_switch" => {
                s.exit_vehicle(pilot);
                let target = if case == "issuer_switch" {
                    vehicle(&mut s)
                } else {
                    v
                };
                assert!(s.enter_vehicle(pilot, target, 0));
                if case == "issuer_switch" {
                    let relief = s
                        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(16.0, 0.0, 0.0)))
                        .unwrap();
                    assert!(s.enter_vehicle(relief, v, 0));
                }
            }
            "issuer_reuse" => {
                s.kill_environment(pilot, DeathCause::Fall);
                // A dead player slot is reserved by the spawn screen until released.
                s.players[0] = None;
                let new = s
                    .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(16.0, 0.0, 0.0)))
                    .unwrap();
                assert_eq!(new.index, pilot.index);
                assert_ne!(new.gen, pilot.gen);
                assert!(s.enter_vehicle(new, v, 0));
            }
            "receiver_reenter" | "receiver_switch" => {
                s.exit_vehicle(bot);
                let target = if case == "receiver_switch" {
                    vehicle(&mut s)
                } else {
                    v
                };
                assert!(s.enter_vehicle(bot, target, 1));
                if case == "receiver_switch" {
                    let relief = s
                        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(16.0, 0.0, 0.0)))
                        .unwrap();
                    assert!(s.enter_vehicle(relief, target, 0));
                }
            }
            "receiver_reuse" | "replace" => {
                if case == "receiver_reuse" {
                    s.kill_environment(bot, DeathCause::Fall);
                } else {
                    s.exit_vehicle(bot);
                }
                watched = s
                    .spawn(&UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(0, Vec3::new(16.0, 0.0, 0.0))
                    })
                    .unwrap();
                if case == "receiver_reuse" {
                    assert_eq!(watched.index, bot.index);
                    assert_ne!(watched.gen, bot.gen);
                }
                assert!(s.enter_vehicle(watched, v, 1));
            }
            "vehicle_reuse" => {
                s.remove_vehicle(v);
                let new = vehicle(&mut s);
                assert_eq!(new.index, v.index);
                assert_ne!(new.gen, v.gen);
                assert!(s.enter_vehicle(pilot, new, 0));
                assert!(s.enter_vehicle(bot, new, 1));
            }
            _ => unreachable!(),
        }
        s.unit_mut(watched).unwrap().ai.next_think = 9;
        for _ in 1..=9 {
            step(&mut s, 0, 0);
            assert!(takes(&s, watched).is_empty(), "{case}");
        }
        assert!(s.unit(watched).unwrap().mount.is_some(), "{case}");
        assert!(
            s.unit(watched)
                .unwrap()
                .ai
                .ride
                .is_none_or(|r| r.own_search_until == 0),
            "{case}"
        );
    }
}

#[test]
fn get_out_hashes_deadline_and_bound_handles_and_repeats_exactly() {
    let (mut s, bot, _, _) = seated(GroundRules::Bf2, false, 0);
    step(&mut s, 0, buttons::SQUAD_AT_EASE);
    let pending = s.unit(bot).unwrap().order.unwrap();
    let before = s.hash();
    for change in 0..5 {
        let p = s.unit_mut(bot).unwrap().order.as_mut().unwrap();
        match change {
            0 => p.vehicle.as_mut().unwrap().index += 1,
            1 => p.vehicle.as_mut().unwrap().gen += 1,
            2 => p.leader.index += 1,
            3 => p.leader.gen += 1,
            _ => p.issuer += 1,
        }
        assert_ne!(before, s.hash(), "pending identity {change}");
        s.unit_mut(bot).unwrap().order = Some(pending);
    }
    for _ in 1..=9 {
        step(&mut s, 0, 0);
    }
    let before = s.hash();
    s.unit_mut(bot)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .own_search_until += 1;
    assert_ne!(
        before,
        s.hash(),
        "own-search deadline drives future behavior"
    );
    let replay = || {
        let (mut s, _, _, _) = seated(GroundRules::Bf2, false, 0);
        (0..40)
            .map(|t| {
                step(&mut s, 0, if t < 3 { buttons::SQUAD_AT_EASE } else { 0 });
                s.hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(replay(), replay());
}

#[test]
fn get_out_rejects_stale_issuer_and_vehicle_generations_at_delivery() {
    for corrupt in [0, 1, 2] {
        let (mut s, bot, _, _) = seated(GroundRules::Bf2, false, 0);
        step(&mut s, 0, buttons::SQUAD_AT_EASE);
        let p = s.unit_mut(bot).unwrap().order.as_mut().unwrap();
        match corrupt {
            0 => {}
            1 => p.vehicle.as_mut().unwrap().gen += 1,
            _ => p.leader.gen += 1,
        }
        let mut answers = Vec::new();
        for _ in 1..=9 {
            step(&mut s, 0, 0);
            answers.extend(takes(&s, bot));
        }
        assert_eq!(answers, if corrupt == 0 { vec![true] } else { vec![] });
        assert_eq!(s.unit(bot).unwrap().mount.is_none(), corrupt == 0);
    }
}

#[test]
fn get_out_autonomous_turret_offer_honors_the_same_deadline() {
    let (mut s, bot, _, _) = seated(GroundRules::Bf2, false, 0);
    for t in 0..=9 {
        step(&mut s, 0, if t == 0 { buttons::SQUAD_AT_EASE } else { 0 });
    }
    let mut setup = s.seat_setup[0].clone().unwrap();
    setup.guns = vec![setup.guns[1].clone()];
    setup.boardable = vec![true];
    s.seat_setup.push(Some(setup));
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 1,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 2.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    });
    s.unit_mut(bot).unwrap().soldier.pos = Vec3::new(4.0, 0.0, 0.0);
    for j in 0..3 {
        s.spawn(&UnitSpec::trooper(
            0,
            Vec3::new(600.0 + j as f32 * 5.0, 0.0, 600.0),
        ))
        .unwrap();
    }
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 50.0)))
        .unwrap();
    s.tick = 600; // Last scheduled turret offer before take9 + 600.
    let last_pos = s.unit(enemy).unwrap().capsule().center();
    s.unit_mut(bot).unwrap().ai.contacts[0] = Some(crate::perception::Contact {
        handle: enemy.into(),
        first_seen: 600,
        last_seen: 600,
        last_pos,
        seen_now: true,
        heard: false,
        told: false,
    });
    crate::ai::man::offer_pass(&mut s);
    assert!(s.unit(bot).unwrap().ai.ride.unwrap().board.is_none());
    // Identical legal offer with only the own-search deadline removed.
    s.unit_mut(bot)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .own_search_until = 0;
    crate::ai::man::offer_pass(&mut s);
    assert_eq!(s.unit(bot).unwrap().ai.ride.unwrap().board, Some(v));
    let r = s.unit_mut(bot).unwrap().ai.ride.as_mut().unwrap();
    r.board = None;
    r.own_search_until = 609;
    s.tick = 630;
    crate::ai::man::offer_pass(&mut s);
    assert_eq!(s.unit(bot).unwrap().ai.ride.unwrap().board, Some(v));
}

#[test]
fn get_out_autonomous_ground_search_observes_the_exact_twenty_second_boundary() {
    let (mut s, bot, pilot, v) = seated(GroundRules::Bf2, false, 0);
    for t in 0..=9 {
        step(&mut s, 0, if t == 0 { buttons::SQUAD_AT_EASE } else { 0 });
    }
    s.exit_vehicle(pilot);
    s.unit_mut(bot).unwrap().soldier.pos = Vec3::new(16.0, 0.0, 0.0);
    for now in [608, 609] {
        s.tick = now;
        let u = s.unit(bot).unwrap();
        let mut st = u.ai;
        st.target_cp = Some(0);
        let mut control = st;
        control.ride.get_or_insert_default().own_search_until = 0;
        claim(&s, bot, u, &mut control);
        assert_eq!(
            control.ride.unwrap().board,
            Some(v),
            "eligible no-cooldown control"
        );
        claim(&s, bot, u, &mut st);
        assert_eq!(
            st.ride.unwrap().board,
            (now == 609).then_some(v),
            "take9 + 600 at {now}"
        );
    }
}

#[test]
fn get_out_preserves_another_players_follow_release_clock() {
    let (mut s, bot, _, _) = seated(GroundRules::Bf2, false, 0);
    let owner = s
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(16.0, 0.0, 0.0)))
        .unwrap();
    let u = s.unit_mut(bot).unwrap();
    u.command = Some(Command {
        kind: CommandKind::Follow(owner),
        issuer: 1,
        issued_tick: 0,
    });
    u.ai.cmd = Some(crate::ai::CmdState {
        goal: Vec3::new(16.0, 0.0, 0.0),
        far_ticks: 137,
        roll: 4,
    });
    for t in 0..=9 {
        step(&mut s, 0, if t == 0 { buttons::SQUAD_AT_EASE } else { 0 });
    }
    let u = s.unit(bot).unwrap();
    assert!(u.mount.is_none());
    assert_eq!(u.command.unwrap().issuer, 1);
    assert_eq!(
        u.ai.cmd.unwrap().far_ticks,
        137,
        "the other player's accumulated release time stays spent"
    );
}
