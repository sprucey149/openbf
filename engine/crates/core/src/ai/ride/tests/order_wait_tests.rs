use super::*;
use crate::sim::buttons;

#[test]
fn wpoke_actual_edges_eight_repairs_forty_controls() {
    let mut failures = 0;
    let mut total = 0;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for order in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
                for case in ["near", "extended", "out", "enemy", "none", "claim"] {
                    let (mut s, pilot, v) = pickup_world(kind);
                    s.ground_rules = rules;
                    arm_gunner_seat(&mut s);
                    for _ in 0..300 {
                        s.step(&[]);
                        if s.unit(pilot).unwrap().mount.is_some() {
                            break;
                        }
                    }
                    assert_eq!(s.unit(pilot).unwrap().mount.unwrap().vehicle, v);
                    let origin = s.vehicle(v).unwrap().pos;
                    let distance = match case {
                        "extended" => 90.0,
                        "out" => 136.0,
                        _ => 8.0,
                    };
                    let player = s
                        .spawn(&UnitSpec::player(
                            u8::from(case == "enemy"),
                            origin + Vec3::new(distance, 0.0, 0.0),
                        ))
                        .unwrap();
                    s.unit_mut(player).unwrap().soldier.pos.y = 0.0;
                    let claimant = if case == "claim" {
                        let pos = origin + Vec3::new(0.0, 0.0, 70.0);
                        let h = s
                            .spawn(&UnitSpec {
                                controller: Controller::Ai,
                                ..UnitSpec::trooper(0, Vec3::new(pos.x, 0.0, pos.z))
                            })
                            .unwrap();
                        Some(h)
                    } else {
                        None
                    };
                    for _ in 0..30 {
                        s.step(&[PadState::default()]);
                    }
                    assert_eq!(locomotion(&s, pilot), 0.0);
                    let old = s.unit(pilot).unwrap().ai.ride.unwrap().pickup.unwrap();
                    assert!(!old.complete);
                    if let Some(h) = claimant {
                        let r = s.unit(h).unwrap().ai.ride.unwrap();
                        assert_eq!(r.board, Some(v));
                        assert!(r.maintenance.is_some());
                    }
                    let sent = s.tick;
                    let mut departure = None;
                    let mut receivers = None;
                    let mut answers = 0;
                    let mut acknowledged = None;
                    let mut departure_pos = None;
                    for tick in 0..30 {
                        s.step(&[PadState {
                            buttons: if tick == 0 && case != "none" {
                                order
                            } else {
                                0
                            },
                            ..PadState::default()
                        }]);
                        for event in &s.events {
                            match event {
                                crate::sim::Event::Command { units, .. } => {
                                    receivers = Some(*units)
                                }
                                crate::sim::Event::OrderTaken { unit, .. } if *unit == pilot => {
                                    answers += 1;
                                    acknowledged = Some(s.tick);
                                }
                                _ => (),
                            }
                        }
                        if locomotion(&s, pilot) > 0.0 && departure.is_none() {
                            departure = Some(s.tick);
                            departure_pos = Some(s.vehicle(v).unwrap().pos);
                        }
                        assert!(s.unit(pilot).unwrap().mount.is_some());
                    }
                    // Both games since issue #7.
                    let should_depart = matches!(case, "near" | "extended");
                    let pass = if should_depart {
                        departure.is_some_and(|t| t <= sent + crate::ai::THINK_PERIOD + 2)
                            && answers == 1
                            && wait(&s, pilot).complete
                            && nav::xz_dist(origin, s.vehicle(v).unwrap().pos) > 1.0
                    } else {
                        departure.is_none()
                    };
                    eprintln!("POKE rules={rules:?} kind={kind:?} order={order} case={case} sent={sent} receivers={receivers:?} answers={answers} acknowledged={acknowledged:?} origin={origin:?} departure={departure:?} departure_pos={departure_pos:?} drift={:.6} complete={} pass={pass}", nav::xz_dist(origin, s.vehicle(v).unwrap().pos), s.unit(pilot).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
                    total += 1;
                    failures += usize::from(!pass);
                }
            }
        }
    }
    assert_eq!(failures, 0, "{failures}/{total} release pickup scenes fail");
}

fn ready(kind: VehicleKind) -> (Sim, UnitHandle, VehicleHandle, UnitHandle) {
    let (mut s, b, v) = pickup_world(kind);
    s.ground_rules = GroundRules::Bf1;
    arm_gunner_seat(&mut s);
    for _ in 0..300 {
        s.step(&[]);
        if s.unit(b).unwrap().mount.is_some() {
            break;
        }
    }
    assert_eq!(s.unit(b).unwrap().mount.unwrap().vehicle, v);
    let pos = s.vehicle(v).unwrap().pos;
    let p = s.spawn(&UnitSpec::player(0, pos + Vec3::X * 8.0)).unwrap();
    (s, b, v, p)
}
fn press(bit: u32) -> PadState {
    PadState {
        buttons: bit,
        ..Default::default()
    }
}
fn wait(s: &Sim, b: UnitHandle) -> crate::ai::state::PickupWait {
    s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap()
}
fn answers(s: &Sim, b: UnitHandle) -> usize {
    s.events
        .iter()
        .filter(|e| matches!(e,crate::sim::Event::OrderTaken {unit,..} if *unit==b))
        .count()
}

#[test]
fn wpoke_mounted_reach_and_receiver_exclusions() {
    for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
        let order = if bit == buttons::SQUAD_MOVE_OUT {
            crate::squad::Order::MoveOut
        } else {
            crate::squad::Order::AtEase
        };
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for (offset, expected) in [
                (Vec3::X * 135.0, 1),
                (Vec3::X * 135.01, 0),
                (Vec3::new(80.0, 108.0, 0.0), 1),
                (Vec3::new(80.0, 109.0, 0.0), 0),
            ] {
                let (mut s, b, v, p) = ready(kind);
                s.unit_mut(p).unwrap().soldier.pos = s.unit(b).unwrap().soldier.pos + offset;
                assert_eq!(s.give_order(0, order), expected, "{kind:?}/{offset}");
                assert_eq!(s.unit(b).unwrap().order.is_some(), expected == 1);
                assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, Some(b));
            }
            for case in [
                "enemy",
                "dead",
                "non-ai",
                "wrong-wait",
                "wrong-seat",
                "wrong-occupant",
                "stale-vehicle",
                "complete",
                "passenger",
                "gunner",
                "turret",
                "human",
                "mounted-issuer",
                "stale-player",
                "hold",
                "follow",
            ] {
                let (mut s, b, v, p) = ready(kind);
                match case {
                    "enemy" => s.unit_mut(b).unwrap().team = 1,
                    "dead" => s.kill_environment(b, crate::sim::DeathCause::Fall),
                    "non-ai" => s.unit_mut(b).unwrap().controller = Controller::Player { slot: 1 },
                    "wrong-seat" => s.unit_mut(b).unwrap().mount.as_mut().unwrap().seat = 1,
                    "wrong-occupant" => s.vehicle_mut(v).unwrap().seats[0].occupant = Some(p),
                    "wrong-wait" => {
                        s.unit_mut(b)
                            .unwrap()
                            .ai
                            .ride
                            .as_mut()
                            .unwrap()
                            .pickup
                            .as_mut()
                            .unwrap()
                            .vehicle
                            .index += 1
                    }
                    "stale-vehicle" => s.vehicle_mut(v).unwrap().gen += 1,
                    "complete" => {
                        s.unit_mut(b)
                            .unwrap()
                            .ai
                            .ride
                            .as_mut()
                            .unwrap()
                            .pickup
                            .as_mut()
                            .unwrap()
                            .complete = true
                    }
                    "passenger" | "gunner" => {
                        s.unseat_unit(b);
                        assert!(s.seat_unit(b, v, 1));
                        if case == "passenger" {
                            s.vehicle_mut(v).unwrap().seats[1].kind = SeatKind::Passenger(0);
                        }
                    }
                    "turret" => s.vehicle_mut(v).unwrap().kind = VehicleKind::Turret,
                    "human" => {
                        assert!(s.enter_vehicle(p, v, 0));
                        // The ejected AI remains an ordinary foot recipient; move it
                        // to the other team so this control isolates the human pilot.
                        s.unit_mut(b).unwrap().team = 1;
                    }
                    "mounted-issuer" => {
                        assert!(s.enter_vehicle(p, v, 1));
                    }
                    "stale-player" => s.players[0].as_mut().unwrap().gen += 1,
                    _ => (),
                }
                let o = match case {
                    "hold" => crate::squad::Order::Hold,
                    "follow" => crate::squad::Order::Follow,
                    _ => order,
                };
                assert_eq!(s.give_order(0, o), 0, "{kind:?}/{case}");
                assert!(s.units[b.index as usize].order.is_none(), "{case}");
            }
        }
    }
}

#[test]
fn wpoke_early_delayed_think_held_and_repeated_edges() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            let (mut s, b, _v, _p) = ready(kind);
            assert!(s.tick < 60);
            let old = wait(&s, b);
            let scheduled = s.tick + 7;
            s.unit_mut(b).unwrap().ai.next_think = scheduled;
            let mut count = 0;
            while s.tick <= scheduled + 2 {
                let before = s.tick;
                s.step(&[press(bit)]);
                count += answers(&s, b);
                let w = wait(&s, b);
                if before < scheduled {
                    assert_eq!(w.released, None);
                    assert!(!w.complete);
                    assert_eq!(locomotion(&s, b), 0.0);
                } else {
                    assert_eq!(w.released, Some(scheduled));
                    assert!(w.complete);
                    assert!(locomotion(&s, b) > 0.0);
                }
                assert_eq!(w.seated, old.seated);
            }
            assert_eq!(count, 1, "held input answers once at scheduled think");
            let completed = wait(&s, b);
            s.step(&[PadState::default()]);
            s.step(&[press(bit)]);
            assert_eq!(
                answers(&s, b),
                0,
                "completed tenure is no longer a receiver"
            );
            assert_eq!(wait(&s, b), completed);
            assert_eq!(s.nav_stats.commands, 2);
        }
    }
}

#[test]
fn wpoke_natural_claim_boards_and_pre_four_second_expiry_leaves() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            for outcome in ["board", "ineligible", "stale"] {
                let (mut s, b, v, _p) = ready(kind);
                let pos = s.vehicle(v).unwrap().pos;
                let h = s
                    .spawn(&UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(0, pos + Vec3::new(70.0, -pos.y, 0.0))
                    })
                    .unwrap();
                for _ in 0..30 {
                    s.step(&[]);
                }
                let r = s.unit(h).unwrap().ai.ride.unwrap();
                assert_eq!(r.board, Some(v));
                assert!(r.maintenance.is_some());
                let since = r.since;
                if outcome == "stale" {
                    s.unit_mut(h).unwrap().ai.next_think = 300;
                }
                let sent = s.tick;
                let mut ack = 0;
                for t in 0..12 {
                    s.step(&[press(if t == 0 { bit } else { 0 })]);
                    ack += answers(&s, b);
                }
                assert_eq!(ack, 1);
                let released = wait(&s, b).released.unwrap();
                assert!(released < 60);
                assert!(!wait(&s, b).complete, "claim holds shortened window");
                assert_eq!(locomotion(&s, b), 0.0);
                // Reissue while held must retain the earliest release.
                for t in 0..12 {
                    s.step(&[press(if t == 0 { bit } else { 0 })]);
                }
                assert_eq!(wait(&s, b).released, Some(released));
                if outcome == "ineligible" {
                    s.unit_mut(h).unwrap().team = 1;
                }
                let last_stamp = s
                    .unit(h)
                    .unwrap()
                    .ai
                    .ride
                    .unwrap()
                    .maintenance
                    .unwrap()
                    .tick;
                let mut boarded = None;
                let mut departed = None;
                let mut last_maintenance = last_stamp;
                for _ in 0..400 {
                    let before = s.tick;
                    s.step(&[]);
                    if s.unit(h).unwrap().mount.is_some_and(|m| m.vehicle == v) {
                        boarded.get_or_insert(s.tick);
                    }
                    if outcome == "board" && boarded.is_none() {
                        let r = s.unit(h).unwrap().ai.ride.unwrap();
                        assert_eq!(r.since, since, "maintenance must not restart claim");
                        let stamp = r.maintenance.unwrap().tick;
                        assert!(stamp >= last_maintenance);
                        last_maintenance = stamp;
                        assert_eq!(locomotion(&s, b), 0.0);
                    }
                    if locomotion(&s, b) > 0.0 {
                        departed = Some(s.tick);
                        break;
                    }
                    if outcome == "ineligible" {
                        assert!(before < 60, "eligibility loss leaves immediately");
                    }
                }
                let departure = departed.expect("shortened window must depart");
                assert!(wait(&s, b).complete);
                if outcome == "board" {
                    assert!(boarded.is_some());
                    assert!(departure > boarded.unwrap());
                    assert!(last_maintenance > last_stamp);
                } else if outcome == "stale" {
                    assert_eq!(departure, last_stamp + 3 * u64::from(crate::TICK_HZ) + 1);
                    assert!(departure < wait(&s, b).seated + 121);
                } else {
                    assert_eq!(departure, sent + 25);
                }
                eprintln!("WPOKE CLAIM {kind:?} bit={bit} {outcome} sent={sent} released={released} since={since} stamp={last_stamp} board={boarded:?} depart={departure}");
            }
        }
    }
}

#[test]
fn wpoke_queued_identity_and_actual_tenure_resets() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for case in [
            "leader-gen",
            "leader-team",
            "leader-mounted",
            "vehicle-gen",
            "target-record",
            "tenure-record",
            "reboard",
            "switch",
            "handover",
            "remove",
        ] {
            let (mut s, b, v, p) = ready(kind);
            s.unit_mut(b).unwrap().ai.next_think = s.tick + 5;
            s.step(&[press(buttons::SQUAD_MOVE_OUT)]);
            assert!(s.unit(b).unwrap().order.unwrap().pickup.is_some());
            match case {
                "leader-gen" => s.players[0].as_mut().unwrap().gen += 1,
                "leader-team" => s.unit_mut(p).unwrap().team = 1,
                "leader-mounted" => {
                    assert!(s.enter_vehicle(p, v, 1));
                }
                "vehicle-gen" => s.vehicle_mut(v).unwrap().gen += 1,
                "target-record" => {
                    s.unit_mut(b)
                        .unwrap()
                        .order
                        .as_mut()
                        .unwrap()
                        .pickup
                        .as_mut()
                        .unwrap()
                        .0
                        .gen += 1
                }
                "tenure-record" => {
                    s.unit_mut(b)
                        .unwrap()
                        .order
                        .as_mut()
                        .unwrap()
                        .pickup
                        .as_mut()
                        .unwrap()
                        .1 += 1
                }
                "reboard" => {
                    s.unseat_unit(b);
                    assert!(s.seat_unit(b, v, 0));
                    assert!(s.unit(b).unwrap().order.is_none());
                }
                "switch" => {
                    s.unseat_unit(b);
                    assert!(s.seat_unit(b, v, 1));
                }
                "handover" => {
                    assert!(s.enter_vehicle(p, v, 0));
                }
                "remove" => s.remove_vehicle(v),
                _ => unreachable!(),
            }
            let mut ack = 0;
            for _ in 0..8 {
                s.step(&[PadState::default()]);
                ack += answers(&s, b);
            }
            assert_eq!(ack, 0, "{case}");
            assert!(
                s.unit(b)
                    .unwrap()
                    .ai
                    .ride
                    .and_then(|r| r.pickup)
                    .is_none_or(|w| w.released.is_none()),
                "{case}"
            );
        }
        // Actual consumed release, retained through ai_use_pass write-back, reset on all exits.
        let (mut s, b, v, _p) = ready(kind);
        for t in 0..12 {
            s.step(&[press(if t == 0 { buttons::SQUAD_AT_EASE } else { 0 })]);
        }
        assert!(wait(&s, b).released.is_some());
        s.exit_vehicle(b);
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
        assert!(s.enter_vehicle(b, v, 0));
        assert_eq!(wait(&s, b).released, None);
        assert!(!wait(&s, b).complete);
        s.step(&[]);
        assert_eq!(locomotion(&s, b), 0.0);
    }
}

#[test]
fn wpoke_pending_payload_and_release_clock_hashes() {
    let (mut s, b, _v, _p) = ready(VehicleKind::Hover);
    assert_eq!(s.give_order(0, crate::squad::Order::AtEase), 1);
    let p = s.unit(b).unwrap().order.unwrap();
    let bytes = |p: crate::squad::Pending| {
        let mut bytes = vec![];
        p.hash_into(&mut |b| bytes.extend_from_slice(b));
        bytes
    };
    for change in [0, 1, 2, 3] {
        let mut other = p;
        match change {
            0 => other.pickup.as_mut().unwrap().0.index += 1,
            1 => other.pickup.as_mut().unwrap().0.gen += 1,
            2 => other.pickup.as_mut().unwrap().1 += 1,
            _ => other.pickup = None,
        }
        assert_ne!(bytes(p), bytes(other));
    }
    let base = s.hash();
    s.unit_mut(b)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .pickup
        .as_mut()
        .unwrap()
        .released = Some(s.tick);
    let a = s.hash();
    assert_ne!(base, a);
    s.unit_mut(b)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .pickup
        .as_mut()
        .unwrap()
        .released = Some(s.tick + 1);
    assert_ne!(a, s.hash());
    // Same reachable waiting state, with/without a consumed release: different futures.
    s.unit_mut(b).unwrap().order = None;
    let (mut shortened, other, _, _) = ready(VehicleKind::Hover);
    shortened
        .unit_mut(other)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .pickup = Some(wait(&s, b));
    s.unit_mut(b)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .pickup
        .as_mut()
        .unwrap()
        .released = None;
    s.step(&[]);
    shortened.step(&[]);
    assert_eq!(locomotion(&s, b), 0.0);
    assert!(locomotion(&shortened, b) > 0.0);
}

#[test]
fn wpoke_consumed_release_clears_through_switch_handover_and_slot_reuse() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for case in ["switch", "handover", "reuse"] {
            let (mut s, b, v, p) = ready(kind);
            for t in 0..12 {
                s.step(&[press(if t == 0 { buttons::SQUAD_MOVE_OUT } else { 0 })]);
            }
            assert!(wait(&s, b).released.is_some());
            match case {
                "switch" => {
                    s.unseat_unit(b);
                    assert!(s.seat_unit(b, v, 1));
                }
                "handover" => {
                    assert!(s.enter_vehicle(p, v, 0));
                }
                "reuse" => s.remove_vehicle(v),
                _ => unreachable!(),
            }
            assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
            let target = if case == "reuse" {
                let replacement = s.spawn_vehicle(&VehicleSpec {
                    kind,
                    class: 0,
                    home_team: None,
                    neutral_when_empty: true,
                    spawn_pad: None,
                    pos: Vec3::ZERO,
                    yaw: 0.0,
                    max_health: 1000.0,
                    health_type: HealthType::Vehicle,
                    seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
                });
                assert_eq!(replacement.index, v.index);
                assert_ne!(replacement.gen, v.gen);
                replacement
            } else {
                s.unseat_unit(b);
                s.unseat_unit(p);
                v
            };
            assert!(s.enter_vehicle(b, target, 0));
            assert_eq!(wait(&s, b).released, None);
            assert!(!wait(&s, b).complete);
            s.step(&[]);
            assert_eq!(locomotion(&s, b), 0.0);
        }
    }
}

#[test]
fn wpoke_consumption_changes_only_release_state_and_one_answer() {
    for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
        let (mut s, b, _v, _p) = ready(VehicleKind::Hover);
        let order = if bit == buttons::SQUAD_MOVE_OUT {
            crate::squad::Order::MoveOut
        } else {
            crate::squad::Order::AtEase
        };
        assert_eq!(s.give_order(0, order), 1);
        let pending = s.unit_mut(b).unwrap().order.take().unwrap();
        let route = s.unit(b).unwrap().ai.route();
        let rng = s.rng.state();
        let events = s.events.len();
        let old = wait(&s, b);
        assert!(!s.take_order(b.index as usize, pending));
        assert_eq!(s.rng.state(), rng);
        assert_eq!(s.unit(b).unwrap().ai.route(), route);
        assert_eq!(s.events.len(), events + 1);
        assert!(
            matches!(s.events.last(),Some(crate::sim::Event::OrderTaken {unit,did:true,..}) if *unit==b)
        );
        let new = wait(&s, b);
        assert_eq!(new.seated, old.seated);
        assert_eq!(new.started, old.started);
        assert!(
            !new.complete,
            "consumption shortens expiry; completion belongs to ride"
        );
        assert_eq!(new.released, Some(s.tick));
    }
}

#[test]
fn wpoke_queued_release_rechecks_receiver_and_issuer_at_think() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            for changed in ["controller", "kind", "team", "issuer-death"] {
                let (mut s, pilot, vehicle, player) = ready(kind);
                let scheduled = s.tick + 7;
                s.unit_mut(pilot).unwrap().ai.next_think = scheduled;
                s.step(&[press(bit)]);
                assert!(s.unit(pilot).unwrap().order.is_some());
                assert_eq!(wait(&s, pilot).released, None);
                match changed {
                    "controller" => {
                        s.unit_mut(pilot).unwrap().controller = Controller::Player { slot: 1 };
                    }
                    "kind" => s.vehicle_mut(vehicle).unwrap().kind = VehicleKind::Turret,
                    "team" => s.unit_mut(pilot).unwrap().team = 1,
                    "issuer-death" => s.kill_environment(player, crate::sim::DeathCause::Fall),
                    _ => unreachable!(),
                }
                let mut acknowledged = 0;
                while s.tick <= scheduled + 1 {
                    s.step(&[PadState::default()]);
                    acknowledged += answers(&s, pilot);
                    assert_eq!(
                        s.unit(pilot)
                            .unwrap()
                            .ai
                            .ride
                            .and_then(|r| r.pickup)
                            .and_then(|w| w.released),
                        None,
                        "{kind:?}/{bit}/{changed}: an obsolete queued receiver cannot release"
                    );
                }
                assert_eq!(acknowledged, 0, "{kind:?}/{bit}/{changed}");
                assert!(s.unit(pilot).unwrap().order.is_none());
                // A controller/kind change can discard pickup state altogether.
                // If retained, the queued order must not complete its window.
                assert!(s
                    .unit(pilot)
                    .unwrap()
                    .ai
                    .ride
                    .and_then(|r| r.pickup)
                    .is_none_or(|w| !w.complete));
            }
        }
    }
}
