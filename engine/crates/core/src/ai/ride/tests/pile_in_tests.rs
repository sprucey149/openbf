//! PILEIN: public player entry and real Right edges, with physical boarding.
use super::*;
use crate::sim::{buttons, DeathCause, Event};
use crate::squad::{CommandKind, Order};

use super::order_scene::*;

/// The original 32-case witness, now also refereeing identity and exact clocks.
#[test]
fn pile_in_actual_edges_and_24_field_idle_follower_controls() {
    let mut passed = 0;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for armed in [false, true] {
                for mode in ["pilot_right", "foot_right", "pilot_idle", "pilot_follow"] {
                    let (mut s, bot, pilot, v) = scene(rules, kind, armed, false, 0);
                    if mode == "foot_right" {
                        s.exit_vehicle(pilot);
                    }
                    if mode == "pilot_follow" {
                        s.unit_mut(bot).unwrap().command.as_mut().unwrap().kind =
                            CommandKind::Follow(pilot);
                    }
                    let mut commands = Vec::new();
                    let mut answers = Vec::new();
                    let mut boarded = None;
                    for tick in 0..60 {
                        step(
                            &mut s,
                            0,
                            if matches!(mode, "pilot_right" | "foot_right") && tick < 3 {
                                buttons::SQUAD_FOLLOW
                            } else {
                                0
                            },
                        );
                        for e in &s.events {
                            if let Event::Command { slot, kind, units } = *e {
                                commands.push((tick, slot, kind, units));
                            }
                        }
                        for did in takes(&s, bot) {
                            answers.push((tick, did));
                        }
                        if s.unit(bot)
                            .unwrap()
                            .mount
                            .is_some_and(|m| m.vehicle == v && m.seat == 1)
                        {
                            boarded.get_or_insert(tick);
                        }
                    }
                    match mode {
                        "pilot_right" => {
                            assert_eq!(commands, [(0, 0, Order::PileIn as u8, 1)]);
                            assert_eq!(answers, [(9, true)]);
                            assert_eq!(boarded, Some(9));
                            assert!(matches!(
                                s.unit(bot).unwrap().command.unwrap().kind,
                                CommandKind::Hold(_)
                            ));
                        }
                        "foot_right" => {
                            assert_eq!(commands, [(0, 0, Order::Follow as u8, 1)]);
                            assert_eq!(answers, [(9, true)]);
                            assert_eq!(
                                s.unit(bot).unwrap().command.unwrap().kind,
                                CommandKind::Follow(pilot)
                            );
                            assert_eq!(boarded, None);
                        }
                        "pilot_follow" => {
                            assert!(commands.is_empty() && answers.is_empty());
                            assert_eq!(boarded, Some(0));
                        }
                        _ => {
                            assert!(commands.is_empty() && answers.is_empty());
                            assert_eq!(boarded, None);
                        }
                    }
                    passed += 1;
                }
            }
        }
    }
    assert_eq!(passed, 32);
    eprintln!("PILEIN actual edges 8/8; field/idle/follower controls 24/24");
}

#[test]
fn pile_in_capacity_recipients_and_delayed_thinks_in_both_index_orders() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for first in [false, true] {
            for n in 0usize..=3 {
                for k in 0usize..=2 {
                    let (mut s, bot, _, v) = scene(rules, VehicleKind::Hover, false, first, 0);
                    // Configure real seats before the press; no injected boarding state.
                    let veh = s.vehicle_mut(v).unwrap();
                    veh.seats.resize(
                        k + 1,
                        crate::vehicle::Seat {
                            kind: SeatKind::Gunner(0),
                            occupant: None,
                        },
                    );
                    for (i, seat) in veh.seats.iter_mut().enumerate().skip(1) {
                        seat.kind = SeatKind::Gunner((i - 1) as u8);
                    }
                    s.seat_setup[0].as_mut().unwrap().boardable = vec![true; k + 1];
                    let mut bots = Vec::new();
                    if n == 0 {
                        s.kill_environment(bot, DeathCause::Fall);
                    } else {
                        bots.push(bot);
                    }
                    for j in 1..n {
                        let at = s.unit(bot).unwrap().soldier.pos + Vec3::Z * (j as f32 * 1.5);
                        let h = s
                            .spawn(&UnitSpec {
                                controller: Controller::Ai,
                                ..UnitSpec::trooper(0, at)
                            })
                            .unwrap();
                        hold(&mut s, h, 0);
                        // Some scenes contend in the same snapshot, others arrive on separate thinks.
                        s.unit_mut(h).unwrap().ai.next_think = if first { 9 } else { 9 + j as u64 };
                        bots.push(h);
                    }
                    let mut answers = vec![0; n];
                    let mut commands = 0;
                    for tick in 0..60 {
                        step(&mut s, 0, if tick < 3 { buttons::SQUAD_FOLLOW } else { 0 });
                        commands += s.events.iter().filter(|e| matches!(e, Event::Command {kind:4, units, ..} if usize::from(*units)==n)).count();
                        for (j, h) in bots.iter().enumerate() {
                            answers[j] += takes(&s, *h).len();
                        }
                    }
                    assert_eq!(commands, 1);
                    assert_eq!(answers, vec![1; n]);
                    let occupied = s
                        .vehicle(v)
                        .unwrap()
                        .seats
                        .iter()
                        .skip(1)
                        .filter(|s| s.occupant.is_some())
                        .count();
                    assert_eq!(occupied, n.min(k), "{rules:?} first{first} n{n} k{k}");
                    for h in bots {
                        no_claim(&s, h);
                    }
                }
            }
        }
    }
}

#[test]
fn pile_in_held_release_repress_and_player_slots() {
    for slot in [0, 1, 3] {
        let (mut s, bot, _, _) = scene(GroundRules::Bf2, VehicleKind::Walker, false, false, slot);
        let mut presses = Vec::new();
        let mut answers = Vec::new();
        for tick in 0..35 {
            let right = tick < 3 || (5..8).contains(&tick) || (20..24).contains(&tick);
            step(&mut s, slot, if right { buttons::SQUAD_FOLLOW } else { 0 });
            for e in &s.events {
                if let Event::Command {
                    slot: issuer, kind, ..
                } = *e
                {
                    assert_eq!((issuer, kind), (slot, 4));
                    presses.push(tick);
                }
            }
            answers.extend(takes(&s, bot).into_iter().map(|_| tick));
        }
        assert_eq!(presses, [0, 5, 20]);
        assert_eq!(
            answers,
            [9],
            "repress overwrites pending once; mounted receivers excluded"
        );
    }
}

#[test]
fn pile_in_rejects_ineligible_recipients_and_entry_without_stale_claims() {
    for case in [
        "unboardable",
        "enemy_vehicle",
        "human_seat",
        "enemy",
        "human",
        "dead",
        "mounted",
        "above",
        "far",
    ] {
        let (mut s, bot, _, v) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
        let reached = matches!(case, "unboardable" | "enemy_vehicle" | "human_seat");
        match case {
            "unboardable" => s.seat_setup[0].as_mut().unwrap().boardable[1] = false,
            "enemy_vehicle" => s.vehicle_mut(v).unwrap().team = Some(1),
            "human_seat" => {
                let h = s
                    .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::ZERO))
                    .unwrap();
                assert!(s.enter_vehicle(h, v, 1));
            }
            "enemy" => s.unit_mut(bot).unwrap().team = 1,
            "human" => {
                s.unit_mut(bot).unwrap().controller = Controller::Player { slot: 1 };
                s.unit_mut(bot).unwrap().is_player = true;
            }
            "dead" => s.kill_environment(bot, DeathCause::Fall),
            "mounted" => {
                assert!(s.enter_vehicle(bot, v, 1));
            }
            "above" => s.unit_mut(bot).unwrap().soldier.pos.y = 31.0,
            "far" => s.unit_mut(bot).unwrap().soldier.pos.x += 40.0,
            _ => unreachable!(),
        }
        step(&mut s, 0, buttons::SQUAD_FOLLOW);
        assert!(
            s.events.iter().any(
                |e| matches!(e, Event::Command {kind:4, units, ..} if *units==u8::from(reached))
            ),
            "{case}"
        );
        let mut answers = Vec::new();
        for _ in 1..30 {
            step(&mut s, 0, 0);
            answers.extend(takes(&s, bot));
        }
        assert_eq!(
            answers,
            if reached { vec![false] } else { vec![] },
            "{case}"
        );
        if case != "mounted" {
            assert!(s.units[bot.index as usize].mount.is_none(), "{case}");
        }
        no_claim(&s, bot);
    }
}

#[test]
fn pile_in_delivery_has_no_follow_cap_or_line_of_sight_gate() {
    let (mut s, bot, _, v) = scene(GroundRules::Bf1, VehicleKind::Hover, false, false, 0);
    // Delivery is radial even when physical entry is impossible; capacity test
    // above exercises >2 recipients, and this one separates delivery from reach.
    s.unit_mut(bot).unwrap().soldier.pos += Vec3::X * 20.0;
    s.collision.push_body(crate::collision::Body::new(
        glam::Mat4::from_translation(Vec3::new(24.0, 2.0, 0.0)),
        crate::collision::Shape::Box {
            half: Vec3::new(1.0, 5.0, 10.0),
        },
    ));
    step(&mut s, 0, buttons::SQUAD_FOLLOW);
    assert_eq!(s.unit(bot).unwrap().order.unwrap().vehicle, Some(v));
    for _ in 1..=9 {
        step(&mut s, 0, 0);
    }
    assert_eq!(takes(&s, bot), [true]);
    assert!(s.unit(bot).unwrap().mount.is_none());
    assert_eq!(s.unit(bot).unwrap().ai.ride.unwrap().board, Some(v));
}

#[test]
fn pile_in_keeps_the_issued_vehicle_when_the_pilot_switches() {
    let (mut s, bot, pilot, v) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
    step(&mut s, 0, buttons::SQUAD_FOLLOW);
    let spec = VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: s.vehicle(v).unwrap().pos,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    };
    let other = s.spawn_vehicle(&spec);
    s.exit_vehicle(pilot);
    assert!(s.enter_vehicle(pilot, other, 0));
    for _ in 1..60 {
        step(&mut s, 0, 0);
    }
    assert_eq!(s.unit(bot).unwrap().mount.unwrap().vehicle, v);
    assert!(s.vehicle(other).unwrap().seats[1].occupant.is_none());
}

#[test]
fn pile_in_prior_orders_survive_and_newer_orders_cancel_boarding() {
    for follows in [false, true] {
        for newer in [false, true] {
            let (mut s, bot, pilot, v) =
                scene(GroundRules::Bf1, VehicleKind::Hover, false, false, 0);
            if follows {
                s.unit_mut(bot).unwrap().command.as_mut().unwrap().kind =
                    CommandKind::Follow(pilot);
            }
            let prior = s.unit(bot).unwrap().command;
            step(&mut s, 0, buttons::SQUAD_FOLLOW);
            assert!(
                s.unit(bot).unwrap().mount.is_none(),
                "Follow cannot board before PILE IN delivery"
            );
            if newer {
                s.exit_vehicle(pilot);
                step(&mut s, 0, buttons::SQUAD_HOLD);
            }
            for _ in s.tick..30 {
                step(&mut s, 0, 0);
            }
            if newer {
                assert!(matches!(
                    s.unit(bot).unwrap().command.unwrap().kind,
                    CommandKind::Hold(_)
                ));
                assert!(s.unit(bot).unwrap().mount.is_none());
            } else {
                assert_eq!(s.unit(bot).unwrap().command, prior);
                assert_eq!(s.unit(bot).unwrap().mount.unwrap().vehicle, v);
            }
            no_claim(&s, bot);
        }
    }
    // A redundant field Follow is still newer than an active board goal.
    let (mut s, bot, pilot, _) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
    s.unit_mut(bot).unwrap().soldier.pos += Vec3::X * 15.0;
    s.unit_mut(bot).unwrap().command.as_mut().unwrap().kind = CommandKind::Follow(pilot);
    let prior = s.unit(bot).unwrap().command;
    step(&mut s, 0, buttons::SQUAD_FOLLOW);
    for _ in 1..=9 {
        step(&mut s, 0, 0);
    }
    assert!(s.unit(bot).unwrap().ai.ride.unwrap().player_board.is_some());
    s.exit_vehicle(pilot);
    step(&mut s, 0, buttons::SQUAD_FOLLOW);
    for _ in s.tick..30 {
        step(&mut s, 0, 0);
    }
    assert_eq!(s.unit(bot).unwrap().command, prior);
    assert!(s.unit(bot).unwrap().mount.is_none());
    no_claim(&s, bot);
}

#[test]
fn pile_in_issuer_receiver_and_vehicle_death_or_generation_reuse_cancel() {
    for case in [
        "issuer",
        "receiver",
        "vehicle",
        "vehicle_reuse",
        "receiver_reuse",
        "issuer_reuse",
    ] {
        let (mut s, bot, pilot, v) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
        s.cps.clear(); // Isolate explicit cancellation from autonomous piloting after death.
        step(&mut s, 0, buttons::SQUAD_FOLLOW);
        match case {
            "issuer" | "issuer_reuse" => {
                s.kill_environment(pilot, DeathCause::Fall);
                if case == "issuer_reuse" {
                    s.players[0] = None; // Release the dead slot, then use normal spawn generation.
                    let h = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
                    assert_eq!(h.index, pilot.index);
                    assert_ne!(h.gen, pilot.gen);
                }
            }
            "receiver" | "receiver_reuse" => {
                s.kill_environment(bot, DeathCause::Fall);
                if case == "receiver_reuse" {
                    let h = s
                        .spawn(&UnitSpec {
                            controller: Controller::Ai,
                            ..UnitSpec::trooper(0, Vec3::new(16.0, 0.0, 0.0))
                        })
                        .unwrap();
                    assert_eq!(h.index, bot.index);
                    assert_ne!(h.gen, bot.gen);
                    hold(&mut s, h, 0);
                }
            }
            "vehicle" | "vehicle_reuse" => {
                s.remove_vehicle(v);
                if case == "vehicle_reuse" {
                    let h = s.spawn_vehicle(&VehicleSpec {
                        kind: VehicleKind::Hover,
                        class: 0,
                        home_team: None,
                        neutral_when_empty: true,
                        spawn_pad: None,
                        pos: Vec3::new(12.0, 2.0, 0.0),
                        yaw: 0.0,
                        max_health: 1000.0,
                        health_type: HealthType::Vehicle,
                        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
                    });
                    assert_eq!(h.index, v.index);
                    assert_ne!(h.gen, v.gen);
                }
            }
            _ => unreachable!(),
        }
        for _ in 1..30 {
            step(&mut s, 0, 0);
        }
        assert!(s.units[bot.index as usize].mount.is_none(), "{case}");
        no_claim(&s, bot);
    }
}

#[test]
fn pile_in_hashes_pending_target_and_active_provenance_and_replays_exactly() {
    let (mut s, bot, _, _) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
    // Keep the recipient outside entry radius to inspect a live approach.
    s.unit_mut(bot).unwrap().soldier.pos += Vec3::X * 15.0;
    step(&mut s, 0, buttons::SQUAD_FOLLOW);
    let pending = s.unit(bot).unwrap().order.unwrap();
    let hash = s.hash();
    s.unit_mut(bot)
        .unwrap()
        .order
        .as_mut()
        .unwrap()
        .vehicle
        .as_mut()
        .unwrap()
        .gen += 1;
    assert_ne!(s.hash(), hash, "pending vehicle generation");
    s.unit_mut(bot).unwrap().order = Some(pending);
    for _ in 1..=9 {
        step(&mut s, 0, 0);
    }
    let active = s.unit(bot).unwrap().ai.ride.unwrap();
    assert!(active.player_board.is_some());
    let hash = s.hash();
    for mutation in 0..4 {
        let r = s.unit_mut(bot).unwrap().ai.ride.as_mut().unwrap();
        match mutation {
            0 => r.player_board = None,
            1 => r.player_board.as_mut().unwrap().slot += 1,
            2 => r.player_board.as_mut().unwrap().leader.index += 1,
            _ => r.player_board.as_mut().unwrap().leader.gen += 1,
        }
        assert_ne!(s.hash(), hash, "active provenance mutation {mutation}");
        s.unit_mut(bot).unwrap().ai.ride = Some(active);
    }
    let replay = || {
        let (mut s, _, _, _) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
        (0..60)
            .map(|t| {
                step(&mut s, 0, if t < 3 { buttons::SQUAD_FOLLOW } else { 0 });
                s.hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(replay(), replay());
}

#[test]
fn pile_in_uncommanded_passenger_needs_no_post_and_active_orders_cancel() {
    for cancel in [
        "none",
        "hold",
        "follow",
        "release",
        "issuer_death",
        "vehicle_death",
        "receiver_death",
    ] {
        let (mut s, bot, pilot, v) = scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
        s.cps.clear();
        s.vehicle_mut(v).unwrap().seats[1].kind = SeatKind::Passenger(0);
        s.unit_mut(bot).unwrap().command = None;
        s.unit_mut(bot).unwrap().soldier.pos += Vec3::X * 15.0;
        step(&mut s, 0, buttons::SQUAD_FOLLOW);
        for _ in 1..=9 {
            step(&mut s, 0, 0);
        }
        assert_eq!(takes(&s, bot), [true]);
        assert_eq!(s.unit(bot).unwrap().ai.ride.unwrap().board, Some(v));
        assert!(s.unit(bot).unwrap().mount.is_none());
        match cancel {
            "none" => {}
            "hold" | "follow" | "release" => {
                s.exit_vehicle(pilot);
                s.unit_mut(pilot).unwrap().soldier.pos = s.unit(bot).unwrap().soldier.pos;
                step(
                    &mut s,
                    0,
                    match cancel {
                        "hold" => buttons::SQUAD_HOLD,
                        "follow" => buttons::SQUAD_FOLLOW,
                        _ => buttons::SQUAD_MOVE_OUT,
                    },
                );
            }
            "issuer_death" => s.kill_environment(pilot, DeathCause::Fall),
            "receiver_death" => s.kill_environment(bot, DeathCause::Fall),
            "vehicle_death" => s.remove_vehicle(v),
            _ => unreachable!(),
        }
        for _ in s.tick..90 {
            step(&mut s, 0, 0);
        }
        if cancel == "none" {
            assert_eq!(s.unit(bot).unwrap().mount.unwrap().vehicle, v);
            assert!(s.unit(bot).unwrap().command.is_none());
        } else if cancel != "receiver_death" {
            assert!(s.unit(bot).unwrap().mount.is_none(), "{cancel}");
        }
        no_claim(&s, bot);
    }
}

#[test]
fn ground_pilot_left_and_right_preserve_other_seat_and_input_controls() {
    for kind in [
        VehicleKind::Hover,
        VehicleKind::Walker,
        VehicleKind::Flyer,
        VehicleKind::Turret,
    ] {
        for seat in [0, 1] {
            for bit in [
                buttons::SQUAD_FOLLOW,
                buttons::SQUAD_HOLD,
                buttons::SQUAD_MOVE_OUT,
                buttons::SQUAD_AT_EASE,
            ] {
                let (mut s, _, pilot, v) =
                    scene(GroundRules::Bf2, VehicleKind::Hover, false, false, 0);
                s.vehicle_mut(v).unwrap().kind = kind;
                if seat == 1 {
                    s.exit_vehicle(pilot);
                    assert!(s.enter_vehicle(pilot, v, 1));
                }
                step(&mut s, 0, bit);
                let expected =
                    if matches!(kind, VehicleKind::Hover | VehicleKind::Walker) && seat == 0 {
                        match bit {
                            buttons::SQUAD_FOLLOW => Some(Order::PileIn as u8),
                            buttons::SQUAD_AT_EASE => Some(Order::GetOut as u8),
                            _ => None,
                        }
                    } else {
                        None
                    };
                assert_eq!(
                    s.events
                        .iter()
                        .filter_map(|e| match e {
                            Event::Command { kind, .. } => Some(*kind),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    expected.into_iter().collect::<Vec<_>>()
                );
            }
        }
    }
}
