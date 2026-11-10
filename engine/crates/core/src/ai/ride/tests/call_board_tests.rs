//! GBOARD: natural issue/think/entry witnesses, controls and delivery invariants.
use super::*;
use crate::sim::{Call, Event};

fn scene(
    kind: VehicleKind,
    distance: f32,
    case: u8,
) -> (Sim, UnitHandle, UnitHandle, VehicleHandle) {
    let (mut s, pilot, vh) = pickup_world(kind);
    s.ground_rules = if case == 2 {
        GroundRules::Bf2
    } else {
        GroundRules::Bf1
    };
    if case != 3 {
        arm_gunner_seat(&mut s);
    }
    if case != 5 {
        s.cps[0].def.pos = Vec3::new(0.0, 0.0, -90.0);
        s.cps[0].def.capture = Region::sphere(s.cps[0].def.pos, 8.0);
    }
    // WGCAL: phase 9 boards at 9 and starts/calls at 10. Phase 10
    // boards at 10 but its start roll at 11 fails (formerly entry 0 failed).
    s.unit_mut(pilot).unwrap().ai.next_think = match case {
        1 => 10,
        5 => 0,
        _ => 9,
    };
    let receiver = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(12.0, 0.0, -distance))
        })
        .unwrap();
    (s, pilot, receiver, vh)
}

fn natural(kind: VehicleKind, distance: f32, case: u8) -> bool {
    let (mut s, pilot, receiver, vh) = scene(kind, distance, case);
    let (mut calls, mut yes, mut expected_yes) = (0, 0, 0);
    let (mut issue, mut claim, mut entry, mut departure) = (None, None, None, None);
    let mut pilot_entry = None;
    let mut departure_calls = 0;
    let mut delivery_think = None;
    let mut since = None;
    let mut full = false;
    for _ in 0..150 {
        let tick = s.tick;
        let thinking = tick >= s.unit(receiver).unwrap().ai.next_think;
        let was_departed = s
            .unit(pilot)
            .unwrap()
            .ai
            .ride
            .and_then(|r| r.pickup)
            .is_some_and(|w| w.departed);
        s.step(&[]);
        let departing = !was_departed
            && s.unit(pilot)
                .unwrap()
                .ai
                .ride
                .and_then(|r| r.pickup)
                .is_some_and(|w| w.departed);
        for e in &s.events {
            if matches!(e, Event::Call{unit, call:Call::NeedGunner} if *unit==pilot) {
                if departing {
                    departure_calls += 1;
                    if s.vehicle(vh)
                        .unwrap()
                        .seats
                        .iter()
                        .any(|seat| seat.occupant.is_none())
                    {
                        assert_eq!(
                            tick,
                            pilot_entry.unwrap() + 121,
                            "vacant-seat deadline departure"
                        );
                    }
                    assert!(
                        s.unit(pilot)
                            .unwrap()
                            .ai
                            .ride
                            .unwrap()
                            .pickup
                            .unwrap()
                            .complete
                    );
                } else {
                    calls += 1;
                }
                let receiver_now = s.unit(receiver).unwrap();
                expected_yes += usize::from(
                    receiver_now.mount.is_none()
                        && receiver_now
                            .soldier
                            .pos
                            .distance_squared(s.unit(pilot).unwrap().soldier.pos)
                            <= 30. * 30.,
                );
                issue = Some(tick);
                assert!(
                    departing
                        || s.unit(receiver)
                            .unwrap()
                            .ai
                            .ride
                            .is_none_or(|r| r.board.is_none()),
                    "no independent claim before call delivery"
                );
            }
            if matches!(e, Event::Call{unit, call:Call::Yes} if *unit==receiver) {
                yes += 1;
            }
        }
        if s.unit(pilot).unwrap().mount.is_some() {
            pilot_entry.get_or_insert(tick);
        }
        let u = s.unit(receiver).unwrap();
        if case == 0 && issue.is_some_and(|t| tick > t) && delivery_think.is_none() && thinking {
            delivery_think = Some(tick);
        }
        if let Some(r) = u.ai.ride.filter(|r| r.board == Some(vh)) {
            if claim.is_none() && case == 0 {
                assert_eq!(Some(tick), delivery_think, "claim at next scheduled think");
                assert!(issue.is_some_and(|t| tick > t));
            }
            claim.get_or_insert(tick);
            assert_eq!(
                *since.get_or_insert(r.since),
                r.since,
                "original claim clock"
            );
        }
        if u.mount.is_some_and(|m| m.vehicle == vh && m.seat == 1) {
            entry.get_or_insert(tick);
            full = true;
        }
        if full && locomotion(&s, pilot) > 0.0 {
            departure.get_or_insert(tick);
        }
    }
    assert_eq!(
        calls,
        usize::from(case == 0 || case == 3 || case == 4),
        "call setup {kind:?}/{case}"
    );
    assert!(departure_calls <= 1, "no duplicate departure attempt");
    assert_eq!(yes, expected_yes, "issue-time membership {kind:?}/{case}");
    assert_eq!(
        s.unit(pilot).unwrap().mount.unwrap().vehicle,
        vh,
        "natural pilot"
    );
    eprintln!("GBOARD {kind:?}/{distance}/{case}: vehicle={vh:?} pilot={pilot_entry:?} issue={issue:?} think={delivery_think:?} claim={claim:?} entry={entry:?} departure={departure:?}");
    if case == 0 || case == 3 || case == 5 {
        entry.is_some() && departure.is_some()
    } else {
        entry.is_none() && claim.is_none()
    }
}

#[test]
fn gboard_four_natural_calls_board_at_next_think() {
    let mut failures = 0;
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for distance in [20.0, 25.0] {
            failures += usize::from(!natural(kind, distance, 0));
        }
    }
    assert_eq!(
        failures, 0,
        "four supported call answerers must board and depart"
    );
}

#[test]
fn gboard_ten_call_and_autonomous_controls() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for case in 1..=5 {
            assert!(
                natural(kind, if case == 4 { 35.0 } else { 20.0 }, case),
                "{kind:?}/{case}"
            );
        }
    }
}

fn issued(kind: VehicleKind) -> (Sim, UnitHandle, UnitHandle, VehicleHandle) {
    let (mut s, p, a, vh) = scene(kind, 20.0, 0);
    while s.tick <= 10 {
        s.step(&[]);
    }
    assert_eq!(s.unit(a).unwrap().ai.pending_board, Some((a, vh)));
    assert!(s
        .events
        .iter()
        .any(|e| matches!(e, Event::Call{unit,call:Call::Yes} if *unit==a)));
    (s, p, a, vh)
}

#[test]
fn gboard_no_action_before_think_and_no_extra_calls_or_random_draws() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, _, a, vh) = issued(kind);
        let (mut control, _, ca, _) = issued(kind);
        control.unit_mut(ca).unwrap().ai.pending_board = None;
        let due = s.unit(a).unwrap().ai.next_think;
        assert!(due > s.tick, "nonzero delivery delay");
        while s.tick < due {
            s.step(&[]);
            control.step(&[]);
            let u = s.unit(a).unwrap();
            let c = control.unit(ca).unwrap();
            assert_eq!(u.ai.pending_board, Some((a, vh)));
            assert!(u.ai.ride.is_none_or(|r| r.board.is_none() && !r.use_req));
            assert_eq!(
                u.soldier.pos, c.soldier.pos,
                "pending goal cannot move before think"
            );
            assert_eq!(s.events, control.events, "no extra acknowledgement");
            assert_eq!(s.rng.state(), control.rng.state());
        }
        s.step(&[]);
        control.step(&[]);
        let u = s.unit(a).unwrap();
        assert!(u.ai.pending_board.is_none());
        assert_eq!(u.ai.ride.unwrap().board, Some(vh));
        assert_eq!(u.ai.ride.unwrap().since, due);
        assert_eq!(s.events, control.events);
        assert_eq!(s.rng.state(), control.rng.state());
    }
}

#[test]
fn gboard_explicit_target_ignores_search_distance_cooldown_and_nearer_decoy() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, _, a, vh) = issued(kind);
        let at = s.unit(a).unwrap().soldier.pos;
        let decoy = s.spawn_vehicle(&VehicleSpec {
            kind,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: at + Vec3::X * 3.0,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        });
        let ai = &mut s.unit_mut(a).unwrap().ai;
        ai.ride = Some(RideState {
            block_until: 10000,
            ..Default::default()
        });
        let due = ai.next_think;
        while s.tick <= due {
            s.step(&[]);
        }
        assert_ne!(vh, decoy);
        assert_eq!(s.unit(a).unwrap().ai.ride.unwrap().board, Some(vh));
        assert_eq!(s.unit(a).unwrap().ai.ride.unwrap().block_until, 10000);
    }
}

#[test]
fn gboard_repeat_delivery_preserves_both_same_and_other_live_claims() {
    for other in [false, true] {
        let (mut s, _, a, vh) = issued(VehicleKind::Hover);
        let mut st = s.unit(a).unwrap().ai;
        st.ride = Some(RideState {
            board: Some(vh),
            since: 1,
            ..Default::default()
        });
        if other {
            let live = s.spawn_vehicle(&VehicleSpec {
                kind: VehicleKind::Hover,
                class: 0,
                home_team: None,
                neutral_when_empty: true,
                spawn_pad: None,
                pos: Vec3::new(24.0, 2.0, 0.0),
                yaw: 0.0,
                max_health: 1000.0,
                health_type: HealthType::Vehicle,
                seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
            });
            assert!(
                claim_seat(&s, a, live).is_some(),
                "other target is live and boardable"
            );
            st.ride.as_mut().unwrap().board = Some(live);
        }
        let old = st.ride;
        let rng = s.rng.state();
        for _ in 0..3 {
            st.pending_board = Some((a, vh));
            deliver_call(&s, a, s.unit(a).unwrap(), &mut st);
            assert_eq!(
                st.ride, old,
                "delivery cannot replace claim or restart clock"
            );
            assert!(st.pending_board.is_none());
        }
        assert_eq!(s.rng.state(), rng);
        s.unit_mut(a).unwrap().ai = st;
    }
}

#[test]
fn gboard_delivery_rejects_invalid_target_or_receiver_once() {
    for case in [
        "target_gen",
        "target_index",
        "destroyed",
        "full",
        "unarmed",
        "no_post",
        "ineligible",
        "turret",
        "flyer",
        "enemy",
        "owner_gen",
        "owner_index",
        "player",
        "mounted",
        "dead",
    ] {
        let (mut s, p, a, vh) = issued(VehicleKind::Hover);
        let mut st = s.unit(a).unwrap().ai;
        st.target_cp = Some(0);
        match case {
            "target_gen" => st.pending_board.as_mut().unwrap().1.gen += 1,
            "target_index" => st.pending_board.as_mut().unwrap().1.index += 1,
            "destroyed" => s.remove_vehicle(vh),
            "full" => {
                let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
                assert!(s.seat_unit(h, vh, 1));
            }
            "unarmed" => s.seat_setup[0].as_mut().unwrap().guns.clear(),
            "no_post" => st.target_cp = None,
            "ineligible" => s.seat_setup[0].as_mut().unwrap().boardable.fill(false),
            "turret" => s.vehicle_mut(vh).unwrap().kind = VehicleKind::Turret,
            "flyer" => s.vehicle_mut(vh).unwrap().kind = VehicleKind::Flyer,
            "enemy" => s.vehicle_mut(vh).unwrap().team = Some(1),
            "owner_gen" => st.pending_board.as_mut().unwrap().0.gen += 1,
            "owner_index" => st.pending_board.as_mut().unwrap().0.index = p.index,
            "player" => s.unit_mut(a).unwrap().controller = Controller::Player { slot: 0 },
            "mounted" => {
                assert!(s.seat_unit(a, vh, 1));
            }
            "dead" => {
                s.units[a.index as usize].alive = false;
            }
            _ => unreachable!(),
        }
        let u = &s.units[a.index as usize];
        deliver_call(&s, a, u, &mut st);
        assert!(st.pending_board.is_none(), "{case}: consumed exactly once");
        assert_eq!(
            st.ride.and_then(|r| r.board),
            if case == "unarmed" { Some(vh) } else { None },
            "{case}: supported policy"
        );
    }
}

#[test]
fn gboard_admission_keeps_issue_tick_yes_list_and_3d_reach() {
    for policy in ["armed", "unarmed", "full"] {
        for case in [
            "near3d",
            "edge3d",
            "outside3d",
            "far3d",
            "enemy",
            "dead",
            "player",
            "mounted",
            "hold",
            "follow",
        ] {
            let (mut s, p, a, vh) = scene(VehicleKind::Hover, 20.0, 0);
            let human = s
                .spawn(&UnitSpec::player(0, Vec3::new(12.0, 0.0, -20.0)))
                .unwrap();
            match case {
                "near3d" => s.unit_mut(a).unwrap().soldier.pos.y = 10.0,
                "edge3d" => s.unit_mut(a).unwrap().soldier.pos = Vec3::new(0.0, 30.0, 0.0),
                "outside3d" => s.unit_mut(a).unwrap().soldier.pos = Vec3::new(0.0, 30.01, 0.0),
                "far3d" => s.unit_mut(a).unwrap().soldier.pos.y = 40.0,
                "enemy" => s.unit_mut(a).unwrap().team = 1,
                "dead" => s.kill_environment(a, crate::sim::DeathCause::VehicleDestroyed),
                "player" => s.unit_mut(a).unwrap().controller = Controller::Player { slot: 1 },
                "mounted" => {
                    assert!(s.seat_unit(a, vh, 1));
                }
                "hold" => {
                    s.issue_command(0, crate::squad::CommandKind::Hold(Vec3::ZERO));
                }
                "follow" => {
                    s.issue_command(0, crate::squad::CommandKind::Follow(human));
                }
                _ => {}
            }
            if policy == "unarmed" {
                s.seat_setup[0].as_mut().unwrap().guns.clear();
            }
            if policy == "full" && case != "mounted" {
                let occupant = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
                assert!(s.enter_vehicle(occupant, vh, 1));
            }
            // Isolate the existing call receiver predicate from falling/commander motion.
            s.tick = 10;
            let rng = s.rng.state();
            s.need_gunner_call(p, vh);
            let yes = s
                .events
                .iter()
                .any(|e| matches!(e,Event::Call{unit,call:Call::Yes} if *unit==a));
            let expected = matches!(case, "near3d" | "edge3d" | "hold" | "follow");
            assert_eq!(yes, expected, "{case}");
            assert_eq!(
                s.units[a.index as usize].ai.pending_board.is_some(),
                expected,
                "{case}"
            );
            assert_eq!(s.rng.state(), rng, "call must not add entropy draws");
        }
    }
}

#[test]
fn gboard_intervening_order_and_receiver_or_target_lifecycle_discard_pending() {
    for case in ["order", "entry", "death", "reuse", "target"] {
        let (mut s, _, a, vh) = issued(VehicleKind::Hover);
        match case {
            "order" => {
                s.spawn(&UnitSpec::player(0, s.unit(a).unwrap().soldier.pos))
                    .unwrap();
                assert!(s.issue_command(0, crate::squad::CommandKind::Hold(Vec3::ZERO)) > 0);
                assert!(s.unit(a).unwrap().command.is_some());
            }
            "entry" => assert!(s.seat_unit(a, vh, 1)),
            "death" | "reuse" => {
                s.kill_environment(a, crate::sim::DeathCause::VehicleDestroyed);
                assert!(s.units[a.index as usize].ai.pending_board.is_none());
                if case == "reuse" {
                    let fresh = s
                        .spawn(&UnitSpec {
                            controller: Controller::Ai,
                            ..UnitSpec::trooper(0, Vec3::ZERO)
                        })
                        .unwrap();
                    assert_eq!(fresh.index, a.index);
                    assert_ne!(fresh.gen, a.gen);
                    assert!(s.unit(fresh).unwrap().ai.pending_board.is_none());
                }
            }
            "target" => s.remove_vehicle(vh),
            _ => unreachable!(),
        }
        assert!(
            s.units[a.index as usize].ai.pending_board.is_none(),
            "{case}"
        );
    }
}

#[test]
fn gboard_two_receivers_share_two_source_claim_slots() {
    let (mut s, _, a, vh) = scene(VehicleKind::Hover, 25.0, 0);
    let b = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(12.0, 0.0, -25.0))
        })
        .unwrap();
    while s.tick <= 10 {
        s.step(&[]);
    }
    for h in [a, b] {
        assert_eq!(s.unit(h).unwrap().ai.pending_board, Some((h, vh)));
        s.unit_mut(h).unwrap().ai.next_think = 20;
    }
    while s.tick <= 20 {
        s.step(&[]);
    }
    for h in [a, b] {
        assert_eq!(s.unit(h).unwrap().ai.ride.unwrap().board, Some(vh));
    }
    s.step(&[]);
    assert_eq!(s.unit(a).unwrap().ai.ride.unwrap().board, Some(vh));
    assert_eq!(
        s.unit(b).unwrap().ai.ride.unwrap().board,
        Some(vh),
        "AI pilot does not consume either of two source claim slots"
    );
    for _ in 0..130 {
        s.step(&[]);
    }
    assert_eq!(
        [a, b]
            .iter()
            .filter(|h| s.unit(**h).unwrap().mount.is_some_and(|m| m.vehicle == vh))
            .count(),
        1,
        "both claims share one physical secondary seat"
    );
}

#[test]
fn gboard_pending_identity_is_hashed_field_by_field_and_absence_costs_nothing() {
    let (s, _, a, vh) = issued(VehicleKind::Hover);
    let mut st = s.unit(a).unwrap().ai;
    let bytes = |st: &AiState| {
        let mut b = vec![];
        st.hash_into(&mut |x| b.extend_from_slice(x));
        b
    };
    st.pending_board = None;
    let bare = bytes(&st);
    st.pending_board = Some((a, vh));
    let base = bytes(&st);
    assert_eq!(base.len(), bare.len() + 22);
    for field in 0..4 {
        let mut variant = st;
        let (owner, target) = variant.pending_board.as_mut().unwrap();
        match field {
            0 => owner.index += 1,
            1 => owner.gen += 1,
            2 => target.index += 1,
            _ => target.gen += 1,
        }
        assert_ne!(base, bytes(&variant), "pending identity field {field}");
    }
    st.pending_board = None;
    assert_eq!(bare, bytes(&st));
}

#[test]
fn gboard_actual_respawn_and_vehicle_slot_reuse_do_not_retarget_a_call() {
    let (mut s, _, a, vh) = issued(VehicleKind::Hover);
    // Actual timed respawn rebuilds the receiver, independently of slot reuse by spawn.
    s.cps[0].owner = Some(0);
    s.cps[0].def.spawn_nodes = vec![(Vec3::new(0.0, 0.0, -90.0), 0.0)];
    s.kill_environment(a, crate::sim::DeathCause::VehicleDestroyed);
    s.units[a.index as usize].respawn_at = Some(s.tick);
    s.units[a.index as usize].spawn_slot = None;
    s.step(&[]);
    let fresh = s.handle(a.index as usize);
    assert_ne!(a, fresh);
    assert!(s.unit(fresh).unwrap().ai.pending_board.is_none());
    let mut st = s.unit(fresh).unwrap().ai;
    st.pending_board = Some((a, vh));
    deliver_call(&s, fresh, s.unit(fresh).unwrap(), &mut st);
    assert!(st.ride.is_none_or(|r| r.board.is_none()));

    s.remove_vehicle(vh);
    let replacement = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
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
    assert_eq!(replacement.index, vh.index);
    assert_ne!(replacement.gen, vh.gen);
    st.target_cp = Some(0);
    st.pending_board = Some((fresh, vh));
    deliver_call(&s, fresh, s.unit(fresh).unwrap(), &mut st);
    assert!(st.pending_board.is_none());
    assert!(
        st.ride.is_none_or(|r| r.board.is_none()),
        "stale target cannot alias replacement"
    );
}

#[test]
fn gboard_older_claim_keeps_clock_with_second_called_claim() {
    for ordered in [false, true] {
        let (mut s, _, a, vh) = scene(VehicleKind::Hover, 25.0, 0);
        let b = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(12.0, 0.0, -25.0))
            })
            .unwrap();
        if ordered {
            s.spawn(&UnitSpec::player(0, Vec3::new(12., 0., -45.)))
                .unwrap();
            assert_eq!(
                s.issue_command(0, crate::squad::CommandKind::Hold(Vec3::ZERO)),
                2
            );
        }
        while s.tick <= 10 {
            s.step(&[]);
        }
        s.unit_mut(a).unwrap().ai.next_think = 20;
        s.unit_mut(b).unwrap().ai.next_think = 11;
        while s.tick <= 20 {
            s.step(&[]);
        }
        assert!(a.index < b.index);
        assert_eq!(s.unit(b).unwrap().ai.ride.unwrap().since, 11);
        assert_eq!(s.unit(b).unwrap().ai.ride.unwrap().board, Some(vh));
        assert_eq!(s.unit(a).unwrap().ai.ride.unwrap().board, Some(vh));
        assert_eq!(s.unit(a).unwrap().ai.ride.unwrap().since, 20);
        for _ in 0..140 {
            s.step(&[]);
        }
        assert_eq!(s.unit(b).unwrap().mount.unwrap().vehicle, vh);
        assert!(s.unit(a).unwrap().mount.is_none());
    }
}

#[test]
fn wgord_ordered_call_matrix() {
    use crate::squad::CommandKind;
    let mut faults = 0;
    let mut controls = 0;
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for bf2 in [false, true] {
            for order in ["pool", "hold", "follow"] {
                let (mut s, p, a, vh) = scene(kind, 20.0, if bf2 { 2 } else { 0 });
                let human = s
                    .spawn(&UnitSpec::player(0, Vec3::new(12.0, 0.0, -40.0)))
                    .unwrap();
                match order {
                    "hold" => assert_eq!(s.issue_command(0, CommandKind::Hold(Vec3::ZERO)), 1),
                    "follow" => assert_eq!(s.issue_command(0, CommandKind::Follow(human)), 1),
                    _ => (),
                }
                assert!(s.unit(p).unwrap().command.is_none());
                let original = s.unit(a).unwrap().command;
                let mut pilot_entry = None;
                let mut call = None;
                let mut yes = None;
                let mut pending = None;
                let mut claim = None;
                let mut entry = None;
                let mut due = None;
                for _ in 0..150 {
                    let tick = s.tick;
                    s.step(&[]);
                    if s.unit(p).unwrap().mount.is_some() {
                        pilot_entry.get_or_insert(tick);
                    }
                    for e in &s.events {
                        if matches!(e,Event::Call{unit,call:Call::NeedGunner} if *unit==p)
                            && call.is_none()
                        {
                            call = Some(tick);
                            due = Some(s.unit(a).unwrap().ai.next_think);
                        }
                        if matches!(e,Event::Call{unit,call:Call::Yes} if *unit==a) {
                            yes.get_or_insert(tick);
                        }
                    }
                    let u = s.unit(a).unwrap();
                    if u.ai.pending_board == Some((a, vh)) {
                        pending.get_or_insert(tick);
                    }
                    if u.ai.ride.is_some_and(|r| r.board == Some(vh)) {
                        claim.get_or_insert(tick);
                    }
                    if u.mount.is_some_and(|m| m.vehicle == vh && m.seat == 1) {
                        entry.get_or_insert(tick);
                    }
                }
                assert_eq!(pilot_entry, Some(9), "natural pilot entry");
                assert_eq!(s.unit(a).unwrap().command, original, "retained order");
                if !bf2 {
                    assert_eq!(call, Some(10));
                    assert_eq!(yes, Some(10));
                    assert_eq!(pending, Some(10));
                    assert_eq!(due, Some(19));
                    assert_eq!(claim, Some(19));
                    if order == "pool" {
                        // The gunner faces the vehicle it walks to (playtest C3).
                        assert_eq!(entry, Some(76));
                    }
                }
                let supported = !bf2;
                assert_eq!(call.is_some(), supported, "natural source");
                assert_eq!(yes.is_some(), supported, "issue membership");
                let passes = if supported {
                    pending == call && claim == due && entry.is_some()
                } else {
                    pending.is_none() && claim.is_none() && entry.is_none()
                };
                if !passes {
                    faults += 1;
                } else {
                    controls += 1;
                }
                eprintln!("ORDER_PROBE {kind:?}/{bf2}/{order}: p={p:?} a={a:?} vehicle={vh:?} call={call:?} yes={yes:?} pending={pending:?} due={due:?} claim={claim:?} entry={entry:?} original={original:?} final={:?} pass={passes}",s.unit(a).unwrap().command);
            }
        }
    }
    eprintln!("ORDER_PROBE TOTAL faults={faults} controls={controls}");
    assert_eq!(
        faults, 0,
        "supported ordered recipients receive a boarding goal"
    );
}

fn ordered_scene(
    kind: VehicleKind,
    follow: bool,
) -> (Sim, UnitHandle, UnitHandle, VehicleHandle, UnitHandle) {
    let (mut s, p, a, vh) = scene(kind, 20., 0);
    let human = s
        .spawn(&UnitSpec::player(0, Vec3::new(12., 0., -40.)))
        .unwrap();
    let order = if follow {
        crate::squad::CommandKind::Follow(human)
    } else {
        crate::squad::CommandKind::Hold(Vec3::ZERO)
    };
    assert_eq!(s.issue_command(0, order), 1);
    while s.tick <= 10 {
        s.step(&[]);
    }
    assert_eq!(s.unit(a).unwrap().ai.pending_board, Some((a, vh)));
    (s, p, a, vh, human)
}

#[test]
fn wgord_retained_orders_resume_after_entry_and_ordinary_dismount() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for follow in [false, true] {
            let (mut s, _, a, vh, human) = ordered_scene(kind, follow);
            let original = s.unit(a).unwrap().command;
            // Meaningful accumulated release progress must not restart on delivery or entry.
            s.unit_mut(a).unwrap().ai.cmd.as_mut().unwrap().far_ticks = 40;
            let mut entry = None;
            while s.tick < 150 && entry.is_none() {
                let tick = s.tick;
                s.step(&[]);
                let u = s.unit(a).unwrap();
                assert_eq!(u.command, original, "order retained every tick");
                assert!(
                    u.ai.cmd.unwrap().far_ticks >= 40,
                    "release progress retained"
                );
                assert!(u.ai.target_cp.is_none(), "ordered bot has no invented post");
                if u.mount.is_some() {
                    entry = Some(tick);
                }
            }
            assert!(entry.is_some(), "physical entry");
            assert_eq!(s.unit(a).unwrap().mount.unwrap().vehicle, vh);
            assert_eq!(s.unit(a).unwrap().mount.unwrap().seat, 1);
            assert!(!s.unit(a).unwrap().ai.ride.unwrap().ordered_call);
            s.exit_vehicle(a);
            let start = s.unit(a).unwrap().soldier.pos;
            if follow {
                s.unit_mut(human).unwrap().soldier.pos = Vec3::new(12., 0., -25.);
            }
            let goal = if follow {
                s.unit(human).unwrap().soldier.pos
            } else {
                match original.unwrap().kind {
                    crate::squad::CommandKind::Hold(p) => p,
                    _ => unreachable!(),
                }
            };
            for _ in 0..24 {
                s.step(&[]);
            }
            let u = s.unit(a).unwrap();
            assert_eq!(u.command, original);
            assert!(u.mount.is_none());
            assert!(
                u.soldier.pos.distance(goal) < start.distance(goal),
                "retained walking resumes after dismount"
            );
            eprintln!("WGORD_RESUME {kind:?} follow{follow} receiver{a:?} vehicle{vh:?} issuer{human:?} entry{entry:?} resume{}", s.tick);
        }
    }
}

#[test]
fn wgord_failure_keeps_command_and_resumes_walk() {
    for follow in [false, true] {
        for active in [false, true] {
            for fault in ["full", "nonboardable", "target", "giveup", "older", "stale"] {
                if !active && matches!(fault, "giveup" | "older" | "stale") {
                    continue;
                }
                let (mut s, _, a, vh, human) = ordered_scene(VehicleKind::Hover, follow);
                if active {
                    while s.tick <= 19 {
                        s.step(&[]);
                    }
                    assert!(s.unit(a).unwrap().ai.ride.unwrap().ordered_call);
                }
                let original = s.unit(a).unwrap().command;
                let progress = s.unit(a).unwrap().ai.cmd.unwrap().far_ticks;
                match fault {
                    "full" => {
                        let h = s.spawn(&UnitSpec::player(1, Vec3::ZERO)).unwrap();
                        assert!(s.enter_vehicle(h, vh, 1));
                    }
                    "nonboardable" => s.seat_setup[0].as_mut().unwrap().boardable.fill(false),
                    "target" => s.remove_vehicle(vh),
                    "giveup" => {
                        let mut st = s.unit(a).unwrap().ai;
                        give_up(&s, &mut st, &mut NavTick::default());
                        s.unit_mut(a).unwrap().ai = st;
                    }
                    "older" => {
                        for _ in 0..2 {
                            let b = s
                                .spawn(&UnitSpec {
                                    controller: Controller::Ai,
                                    ..UnitSpec::trooper(0, Vec3::new(12., 0., -22.))
                                })
                                .unwrap();
                            let tick = s.tick;
                            s.unit_mut(b).unwrap().ai.ride = Some(RideState {
                                board: Some(vh),
                                since: 0,
                                maintenance: Some(crate::ai::state::BoardMaintenance {
                                    owner: b,
                                    tick,
                                }),
                                ..RideState::default()
                            });
                            s.unit_mut(b).unwrap().ai.next_think = tick + 100;
                        }
                    }
                    "stale" => {
                        let r = s.unit_mut(a).unwrap().ai.ride.as_mut().unwrap();
                        r.maintenance.as_mut().unwrap().owner.gen += 1;
                    }
                    _ => unreachable!(),
                }
                for _ in 0..12 {
                    s.step(&[]);
                }
                let u = s.unit(a).unwrap();
                assert_eq!(u.command, original, "{fault}/{active}: command survives");
                assert!(u.ai.cmd.unwrap().far_ticks >= progress);
                assert!(
                    u.ai.ride
                        .is_none_or(|r| r.board.is_none() && !r.ordered_call),
                    "{fault}/{active}: ended"
                );
                assert!(u.mount.is_none());
                // Move the receiver within walking distance of its retained point,
                // avoiding an already reached Hold point in delivery failures.
                let goal = if follow {
                    s.unit(human).unwrap().soldier.pos
                } else {
                    match original.unwrap().kind {
                        crate::squad::CommandKind::Hold(p) => p,
                        _ => unreachable!(),
                    }
                };
                s.unit_mut(a).unwrap().soldier.pos = goal + Vec3::new(8., 0., 0.);
                let before = s.unit(a).unwrap().soldier.pos.distance(goal);
                for _ in 0..15 {
                    s.step(&[]);
                }
                assert!(
                    s.unit(a).unwrap().soldier.pos.distance(goal) < before,
                    "{fault}/{active}: retained walking"
                );
            }
        }
    }
}

#[test]
fn wgord_new_orders_release_and_death_cancel_without_stale_revival() {
    for follow in [false, true] {
        for phase in [11, 20, 90] {
            for replacement in [
                "hold",
                "follow",
                "release",
                "issuer_death",
                "receiver_death",
            ] {
                if follow && replacement == "follow" {
                    continue;
                } // Existing Follow veto is unchanged.
                let (mut s, _, a, _, human) = ordered_scene(VehicleKind::Hover, follow);
                while s.tick < phase {
                    s.step(&[]);
                }
                // At phase90 the bot is seated: ordinary on-foot orders do not
                // reach it. clear_all and issuer death still reach every state.
                if phase == 90 && matches!(replacement, "hold" | "follow") {
                    continue;
                }
                let pos = s.unit(a).unwrap().soldier.pos;
                s.unit_mut(human).unwrap().soldier.pos = pos + Vec3::X;
                match replacement {
                    "hold" => {
                        assert!(
                            s.issue_command(0, crate::squad::CommandKind::Hold(Vec3::ZERO)) > 0
                        );
                    }
                    "follow" => {
                        assert!(s.issue_command(0, crate::squad::CommandKind::Follow(human)) > 0);
                    }
                    "release" => {
                        assert!(s.clear_all(0) > 0);
                    }
                    "issuer_death" => {
                        s.kill_environment(human, crate::sim::DeathCause::VehicleDestroyed)
                    }
                    "receiver_death" => {
                        s.kill_environment(a, crate::sim::DeathCause::VehicleDestroyed)
                    }
                    _ => unreachable!(),
                }
                let expected = s.units[a.index as usize].command;
                assert!(s.units[a.index as usize].ai.pending_board.is_none());
                if replacement != "receiver_death" {
                    assert!(s
                        .unit(a)
                        .unwrap()
                        .ai
                        .ride
                        .is_none_or(|r| !r.ordered_call && r.board.is_none()));
                    for _ in 0..18 {
                        s.step(&[]);
                    }
                    assert_eq!(
                        s.unit(a).unwrap().command,
                        expected,
                        "{replacement}/{phase}: no stale restoration"
                    );
                }
            }
        }
    }
}

#[test]
fn wgord_marker_hash_and_identical_continuations() {
    let (mut s, _, a, _, _) = ordered_scene(VehicleKind::Hover, false);
    while s.tick <= 19 {
        s.step(&[]);
    }
    let mut st = s.unit(a).unwrap().ai;
    let bytes = |st: &AiState| {
        let mut out = vec![];
        st.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let explicit = bytes(&st);
    assert!(st.ride.unwrap().ordered_call);
    st.ride.as_mut().unwrap().ordered_call = false;
    assert_ne!(
        explicit,
        bytes(&st),
        "explicit marker changes future admission"
    );
    let run = || {
        let (mut s, _, a, _, _) = ordered_scene(VehicleKind::Hover, false);
        let mut out = vec![];
        while s.tick < 90 {
            s.step(&[]);
            out.push(s.hash());
        }
        assert!(s.unit(a).unwrap().mount.is_some());
        s.exit_vehicle(a);
        for _ in 0..20 {
            s.step(&[]);
            out.push(s.hash());
        }
        out
    };
    assert_eq!(
        run(),
        run(),
        "identical boarding and lifecycle continuation"
    );
}

#[test]
fn wgord_button_orders_and_delayed_departure_calls_in_both_index_orders() {
    use crate::sim::buttons;
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for receiver_first in [false, true] {
            for follow in [false, true] {
                let (mut s, p, a, vh) =
                    super::setout_call_tests::departure_scene(kind, receiver_first);
                let departure = s.tick + 121;
                let human = s
                    .spawn(&UnitSpec::player(0, s.unit(a).unwrap().soldier.pos))
                    .unwrap();
                s.unit_mut(a).unwrap().ai.next_think = s.tick;
                s.step(&[PadState {
                    buttons: if follow {
                        buttons::SQUAD_FOLLOW
                    } else {
                        buttons::SQUAD_HOLD
                    },
                    ..PadState::default()
                }]);
                let original = s.unit(a).unwrap().command;
                assert!(original.is_some(), "button installs actual order");
                while s.tick < departure {
                    s.step(&[]);
                }
                let at = s.unit(p).unwrap().soldier.pos + Vec3::new(0., 0., -20.);
                s.unit_mut(a).unwrap().soldier.pos = at;
                s.unit_mut(human).unwrap().soldier.pos = at + Vec3::X;
                s.unit_mut(a).unwrap().ai.next_think = departure + 7;
                s.step(&[]);
                assert!(s
                    .events
                    .iter()
                    .any(|e| matches!(e,Event::Call{unit,call:Call::NeedGunner}if *unit==p)));
                assert!(s
                    .events
                    .iter()
                    .any(|e| matches!(e,Event::Call{unit,call:Call::Yes}if *unit==a)));
                assert_eq!(s.unit(a).unwrap().ai.pending_board, Some((a, vh)));
                while s.tick < departure + 7 {
                    s.step(&[]);
                    assert!(s.unit(a).unwrap().ai.ride.is_none_or(|r| r.board.is_none()));
                }
                s.step(&[]);
                let u = s.unit(a).unwrap();
                assert_eq!(u.command, original);
                assert!(u.ai.pending_board.is_none());
                let r = u.ai.ride.unwrap();
                assert_eq!(r.board, Some(vh));
                assert_eq!(r.since, departure + 7);
                assert!(r.ordered_call);
                assert!(u.ai.target_cp.is_none());
            }
        }
    }
}

#[test]
fn wgord_button_replacements_and_existing_distance_release_end_boarding() {
    use crate::sim::buttons;
    for phase in [11, 20] {
        for button in [
            buttons::SQUAD_HOLD,
            buttons::SQUAD_FOLLOW,
            buttons::SQUAD_AT_EASE,
        ] {
            let (mut s, _, a, _, human) = ordered_scene(VehicleKind::Hover, false);
            while s.tick < phase {
                s.step(&[]);
            }
            s.unit_mut(human).unwrap().soldier.pos = s.unit(a).unwrap().soldier.pos + Vec3::X;
            s.unit_mut(a).unwrap().ai.next_think = s.tick;
            s.step(&[PadState {
                buttons: button,
                ..PadState::default()
            }]);
            let u = s.unit(a).unwrap();
            assert!(u.ai.pending_board.is_none());
            assert!(u
                .ai
                .ride
                .is_none_or(|r| !r.ordered_call && r.board.is_none()));
            assert!(u.mount.is_none());
            match button {
                buttons::SQUAD_HOLD => assert!(matches!(
                    u.command.unwrap().kind,
                    crate::squad::CommandKind::Hold(_)
                )),
                buttons::SQUAD_FOLLOW => assert_eq!(
                    u.command.unwrap().kind,
                    crate::squad::CommandKind::Follow(human)
                ),
                _ => assert!(u.command.is_none()),
            }
        }
    }
    for follow in [false, true] {
        let (mut s, _, a, _, human) = ordered_scene(VehicleKind::Hover, follow);
        while s.tick <= 19 {
            s.step(&[]);
        }
        s.unit_mut(a).unwrap().ai.cmd.as_mut().unwrap().far_ticks = crate::squad::LEAVE_TICKS - 10;
        s.unit_mut(human).unwrap().soldier.pos = Vec3::new(100., 0., 100.);
        s.unit_mut(a).unwrap().ai.next_think = s.tick;
        s.step(&[]);
        let u = s.unit(a).unwrap();
        assert!(
            u.command.is_none(),
            "retained command obeys existing cumulative release"
        );
        assert!(u.ai.cmd.is_none());
        assert!(u.ai.pending_board.is_none());
        assert!(u
            .ai
            .ride
            .is_none_or(|r| !r.ordered_call && r.board.is_none()));
        assert!(u.mount.is_none(), "release preempts physical entry");
    }
}

#[test]
fn wgord_target_generation_reuse_and_receiver_death_end_explicit_claim() {
    for pending in [true, false] {
        let (mut s, _, a, vh, _) = ordered_scene(VehicleKind::Hover, false);
        if !pending {
            while s.tick <= 19 {
                s.step(&[]);
            }
        }
        let original = s.unit(a).unwrap().command;
        s.remove_vehicle(vh);
        let replacement = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.,
            max_health: 1000.,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        });
        assert_eq!(replacement.index, vh.index);
        assert_ne!(replacement.gen, vh.gen);
        for _ in 0..20 {
            s.step(&[]);
        }
        let u = s.unit(a).unwrap();
        assert_eq!(u.command, original);
        assert!(u.mount.is_none());
        assert!(u.ai.pending_board.is_none());
        assert!(u
            .ai
            .ride
            .is_none_or(|r| r.board.is_none() && !r.ordered_call));
    }
    let (mut s, _, a, _, _) = ordered_scene(VehicleKind::Hover, false);
    while s.tick <= 19 {
        s.step(&[]);
    }
    assert!(s.unit(a).unwrap().ai.ride.unwrap().ordered_call);
    s.kill_environment(a, crate::sim::DeathCause::VehicleDestroyed);
    let u = &s.units[a.index as usize];
    assert!(u.ai.pending_board.is_none());
    assert!(u.command.is_none());
    assert!(u
        .ai
        .ride
        .is_none_or(|r| r.board.is_none() && !r.ordered_call && !r.use_req));
}

#[test]
fn wgord_explicit_target_precedes_retained_followers_mounted_leader() {
    let (mut s, _, a, vh, human) = ordered_scene(VehicleKind::Hover, true);
    while s.tick <= 19 {
        s.step(&[]);
    }
    let original = s.unit(a).unwrap().command;
    let other = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(12., 0., -20.),
        yaw: 0.,
        max_health: 1000.,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    assert!(s.enter_vehicle(human, other, 0));
    let mut entered = false;
    while s.tick < 150 {
        s.step(&[]);
        let u = s.unit(a).unwrap();
        assert_eq!(u.command, original);
        if let Some(m) = u.mount {
            assert_eq!(
                m.vehicle, vh,
                "temporary explicit vehicle wins over retained Follow"
            );
            assert_eq!(m.seat, 1);
            entered = true;
            break;
        }
    }
    assert!(entered);
}

#[path = "passenger_call_tests.rs"]
mod passenger_call_tests;

#[test]
fn wgord_issuer_loss_clears_retained_order_before_slot_reuse() {
    for follow in [false, true] {
        for phase in [11, 20, 90] {
            let (mut s, _, a, _, human) = ordered_scene(VehicleKind::Hover, follow);
            while s.tick < phase {
                s.step(&[]);
            }
            assert!(s.unit(a).unwrap().command.is_some());
            s.kill_environment(human, crate::sim::DeathCause::VehicleDestroyed);
            let u = s.unit(a).unwrap();
            assert!(u.command.is_none(), "issuer death must actually release");
            assert!(u.ai.cmd.is_none());
            assert!(u.ai.pending_board.is_none());
            assert!(u.ai.ride.is_none_or(|r| {
                r.board.is_none() && !r.ordered_call && r.called_board.is_none()
            }));
            assert_eq!(s.players[0], Some(human), "death retains the player slot");
            s.players[0] = None; // Explicitly release the dead issuer before normal slot reassignment.
            let replacement = s
                .spawn(&UnitSpec::player(0, Vec3::new(12., 0., -40.)))
                .unwrap();
            assert_eq!(replacement.index, human.index);
            assert_ne!(replacement.gen, human.gen);
            assert_eq!(s.players[0], Some(replacement));
            for _ in 0..30 {
                s.step(&[]);
                assert_eq!(s.players[0], Some(replacement));
                let u = s.unit(a).unwrap();
                assert!(
                    u.command.is_none(),
                    "replacement issuer cannot revive order"
                );
                assert!(u.ai.cmd.is_none());
                assert!(u.ai.pending_board.is_none());
                assert!(u.ai.ride.is_none_or(|r| !r.ordered_call));
            }
        }
    }
}
