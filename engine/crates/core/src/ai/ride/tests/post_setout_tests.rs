//! WGNXT natural post-stop and next-route witnesses.
use super::*;
use crate::sim::{Call, Event};
// These witnesses measure ordinary post stops/departures, not last-crew
// endgames. Keep distant, inert teammates beyond COMMAND_ENDGAME; the walker
// driver's dedicated tests cover the small-team exit.
fn ordinary_population(s: &mut Sim) {
    for i in 0..crate::ai::walker_drive::COMMAND_ENDGAME {
        s.spawn(&UnitSpec {
            controller: Controller::Player { slot: 7 },
            ..UnitSpec::trooper(0, Vec3::new(1500.0 + i as f32 * 3.0, 0.0, 1500.0))
        })
        .unwrap();
    }
}

fn count(s: &Sim, h: UnitHandle, call: Call) -> usize {
    s.events
        .iter()
        .filter(|e| matches!(e, Event::Call {unit,call:c} if *unit==h && *c==call))
        .count()
}

#[test]
fn wgnxt_natural_post_departures() {
    let mut faults = 0;
    let mut scenes = 0;
    let mut successes = 0;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for delay in [12., 16., 20., 24., 28., 32.] {
            for capture in [false, true] {
                for second in [false, true] {
                    let (mut s, first, v) = pickup_world(VehicleKind::Walker);
                    let p = if second {
                        s.unit_mut(first).unwrap().controller = Controller::Player { slot: 1 };
                        s.unit_mut(first).unwrap().soldier.pos = Vec3::new(900., 0., 900.);
                        s.spawn(&UnitSpec {
                            controller: Controller::Ai,
                            ..UnitSpec::trooper(0, Vec3::ZERO)
                        })
                        .unwrap()
                    } else {
                        first
                    };
                    s.ground_rules = rules;
                    s.walker_params[0].as_mut().unwrap().spawn_points = vec![(Vec3::ZERO, 0.)];
                    let pos = Vec3::new(12., 0., -60.);
                    s.cps[0].def.pos = pos;
                    s.cps[0].def.capture = Region::sphere(pos, 8.);
                    s.cps[0].def.capture_time = delay;
                    s.cps[0].owner = None;
                    let mut def = s.cps[0].def.clone();
                    def.name = "second".into();
                    def.pos = Vec3::new(12., 0., -300.);
                    def.capture = Region::sphere(def.pos, 8.);
                    s.add_cp(def, Some(1));
                    let human = s
                        .spawn(&UnitSpec::player(
                            0,
                            if capture { pos } else { pos + Vec3::X * 30. },
                        ))
                        .unwrap();
                    s.unit_mut(human).unwrap().soldier.yaw = std::f32::consts::PI;
                    ordinary_population(&mut s);
                    let mut first_route = None;
                    let mut at_post_ticks = 0;
                    let mut restart = None;
                    let mut calls = Vec::new();
                    let mut captured = None;
                    let mut initial_wait = None;
                    let mut initial_departure = None;
                    let mut expected = Vec::new();
                    let mut trace = 0u64;
                    for _ in 0..2000 {
                        let tick = s.tick;
                        s.step(&[PadState::default()]);
                        let vehicle = s.vehicle(v).unwrap();
                        for bits in vehicle
                            .pos
                            .to_array()
                            .map(f32::to_bits)
                            .into_iter()
                            .chain(vehicle.rot.to_array().map(f32::to_bits))
                            .chain([s.rng.state(), s.lcg.state()])
                        {
                            trace = trace.wrapping_mul(1099511628211) ^ u64::from(bits);
                        }
                        let u = s.unit(p).unwrap();
                        if let Some(w) = u.ai.ride.and_then(|r| r.pickup) {
                            if w.started == Some(tick) {
                                initial_wait = Some(tick);
                            }
                        }
                        if s.cps[0].owner == Some(0) {
                            captured.get_or_insert(tick);
                        }
                        let driving = u.mount.is_some()
                            && u.ai.ride.and_then(|r| r.pickup).is_some_and(|w| w.complete)
                            && u.ai.steer == crate::ai::SteerPick::Fwd;
                        if driving && first_route.is_none() {
                            first_route = Some(tick);
                            initial_departure = Some(tick);
                        }
                        if first_route.is_some()
                            && u.ai.target_cp == Some(0)
                            && !driving
                            && crate::nav::xz_dist(s.vehicle(v).unwrap().pos, pos) <= 10.
                        {
                            at_post_ticks += 1;
                        }
                        if driving && u.ai.target_cp == Some(1) && restart.is_none() {
                            assert!(
                                capture && at_post_ticks > 0,
                                "real post wait before restart"
                            );
                            restart = Some(tick);
                        }
                        if rules == GroundRules::Bf1
                            && (initial_wait == Some(tick)
                                || initial_departure == Some(tick)
                                || restart == Some(tick))
                            && crate::ai::unit_unit(p.index, tick, 0, crate::ai::SALT_NEED_GUNNER)
                                < 0.3
                        {
                            expected.push(tick);
                        }
                        for _ in 0..count(&s, p, Call::NeedGunner) {
                            calls.push(tick);
                        }
                    }
                    assert!(
                        first_route.is_some()
                            && at_post_ticks > 0
                            && s.unit(p).unwrap().mount.is_some()
                    );
                    assert_eq!(s.unit(p).unwrap().mount.unwrap().vehicle, v);
                    assert_eq!(restart.is_some(), capture);
                    assert_eq!(captured.is_some(), capture);
                    eprintln!(
                        "TRACE {rules:?} delay{delay} capture{capture} second{second} {trace}"
                    );
                    let good = calls == expected;
                    successes += usize::from(
                        rules == GroundRules::Bf1
                            && restart.is_some_and(|t| {
                                crate::ai::unit_unit(p.index, t, 0, crate::ai::SALT_NEED_GUNNER)
                                    < 0.3
                            }),
                    );
                    scenes += 1;
                    faults += usize::from(!good);
                    eprintln!("POST {rules:?} delay{delay} capture{capture} p{} wait{initial_wait:?} first{first_route:?} stop{at_post_ticks} captured{captured:?} restart{restart:?} actual{calls:?} expected{expected:?} good{good}",p.index);
                }
            }
        }
    }
    eprintln!("POST TOTAL scenes{scenes} faults{faults} successful_later_draws{successes}");
    assert_eq!(faults, 0, "later seated post set-out call");
}

fn post_world(second: bool) -> (Sim, UnitHandle, VehicleHandle, UnitHandle) {
    let (mut s, first, v) = pickup_world(VehicleKind::Walker);
    let p = if second {
        s.unit_mut(first).unwrap().controller = Controller::Player { slot: 1 };
        s.unit_mut(first).unwrap().soldier.pos = Vec3::new(900., 0., 900.);
        s.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap()
    } else {
        first
    };
    s.ground_rules = GroundRules::Bf1;
    s.walker_params[0].as_mut().unwrap().spawn_points = vec![(Vec3::ZERO, 0.)];
    let pos = Vec3::new(12., 0., -60.);
    s.cps[0].def.pos = pos;
    s.cps[0].def.capture = Region::sphere(pos, 8.);
    s.cps[0].def.capture_time = 20.;
    s.cps[0].owner = None;
    let mut def = s.cps[0].def.clone();
    def.name = "second".into();
    def.pos.z = -300.;
    def.capture = Region::sphere(def.pos, 8.);
    s.add_cp(def, Some(1));
    let human = s.spawn(&UnitSpec::player(0, pos + Vec3::X * 40.)).unwrap();
    s.unit_mut(human).unwrap().soldier.yaw = std::f32::consts::PI;
    ordinary_population(&mut s);
    (s, p, v, human)
}
fn stopped_scene(second: bool) -> (Sim, UnitHandle, VehicleHandle, UnitHandle) {
    let (mut s, p, v, h) = post_world(second);
    for _ in 0..400 {
        s.step(&[]);
    }
    let u = s.unit(p).unwrap();
    assert_eq!(u.mount.unwrap().vehicle, v);
    assert_eq!(u.ai.target_cp, Some(0));
    assert_eq!(u.ai.steer, crate::ai::SteerPick::Idle);
    assert!(crate::nav::xz_dist(s.vehicle(v).unwrap().pos, s.cps[0].def.pos) <= 10.);
    (s, p, v, h)
}
fn drive_state(s: &Sim, p: UnitHandle, st: &mut AiState) {
    let m = s.unit(p).unwrap().mount.unwrap();
    crate::ai::walker_drive::drive(
        s,
        p,
        m,
        s.walker_params[0].as_ref().unwrap(),
        st,
        Default::default(),
    );
}

#[test]
fn wgnxt_stop_consumption_and_invalid_transitions() {
    for mode in [
        "depart",
        "same",
        "owner",
        "lost",
        "order",
        "unroutable",
        "noncommand",
        "bf2",
    ] {
        let (mut s, p, v, _) = stopped_scene(false);
        let mut st = s.unit(p).unwrap().ai;
        let before = st.ride.unwrap().pickup.unwrap();
        assert_eq!(before.post_stop, Some(0), "witness actual reached stop");
        assert!(before.departed && before.complete);
        match mode {
            "same" => (),
            "owner" => s.cps[0].owner = Some(0),
            "lost" => st.target_cp = None,
            "order" => {
                st.cmd_clear();
                st.target_cp = Some(1);
            }
            "unroutable" => {
                s.nav = Some(crate::ai::tests::l_graph());
                st.route = None;
                st.target_cp = Some(1);
            }
            "noncommand" => {
                s.walker_params[0].as_mut().unwrap().spawn_points.clear();
                st.target_cp = Some(1);
            }
            "bf2" => {
                s.ground_rules = GroundRules::Bf2;
                st.target_cp = Some(1);
            }
            _ => st.target_cp = Some(1),
        }
        drive_state(&s, p, &mut st);
        let after = st.ride.unwrap().pickup.unwrap();
        assert_eq!(
            after.post_departure,
            (mode == "depart").then_some(s.tick),
            "accepted next goal only: {mode}"
        );
        assert_eq!(
            (
                after.seated,
                after.started,
                after.released,
                after.complete,
                after.departed
            ),
            (
                before.seated,
                before.started,
                before.released,
                before.complete,
                before.departed
            ),
            "initial history/clocks preserved"
        );
        if mode == "depart" {
            assert!(after.post_stop.is_none(), "consume before rolling");
            for tick in 401..450 {
                s.tick = tick;
                // Continuous route target changes must not manufacture another source.
                st.target_cp = Some(1);
                drive_state(&s, p, &mut st);
                // Going back to the reached post can legitimately stop again.
                if st.steer == crate::ai::SteerPick::Fwd {
                    assert_eq!(st.ride.unwrap().pickup.unwrap().post_departure, Some(400));
                }
            }
        }
        assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, Some(p));
    }
    // A changed goal while still travelling has no witnessed stop to consume.
    let (mut s, p, _, _) = post_world(false);
    for _ in 0..200 {
        s.step(&[]);
    }
    let mut st = s.unit(p).unwrap().ai;
    assert_eq!(st.steer, crate::ai::SteerPick::Fwd);
    assert!(st.ride.unwrap().pickup.unwrap().post_stop.is_none());
    st.target_cp = Some(1);
    drive_state(&s, p, &mut st);
    assert!(
        st.ride.unwrap().pickup.unwrap().post_departure.is_none(),
        "early retarget is not post departure"
    );
}

#[test]
fn wgnxt_repeated_real_captures_consume_success_and_failure_once() {
    let (mut s, p, v, h) = post_world(false);
    for i in 2..6 {
        let mut def = s.cps[0].def.clone();
        def.name = format!("post{i}");
        def.pos.z = -60. - i as f32 * 240.;
        def.capture = Region::sphere(def.pos, 8.);
        s.add_cp(def, Some(1));
    }
    let mut stopped = 0;
    let mut previous_post = None;
    let mut later = vec![];
    let mut expected = vec![];
    let mut actual = vec![];
    for _ in 0..14000 {
        let tick = s.tick;
        s.step(&[]);
        let u = s.unit(p).unwrap();
        let w = u.ai.ride.unwrap().pickup.unwrap();
        let cp = u.ai.target_cp.unwrap();
        if previous_post != Some(cp) {
            stopped = 0;
            previous_post = Some(cp);
        }
        let driving = u.ai.steer == crate::ai::SteerPick::Fwd;
        if !driving
            && w.departed
            && crate::nav::xz_dist(s.vehicle(v).unwrap().pos, s.cps[cp as usize].def.pos) <= 10.
        {
            stopped += 1;
        }
        if w.post_departure == Some(tick) {
            later.push(tick);
            assert!(w.departed && w.complete);
        }
        if (w.started == Some(tick) || tick == 121 || w.post_departure == Some(tick))
            && crate::ai::unit_unit(p.index, tick, 0, crate::ai::SALT_NEED_GUNNER) < 0.3
        {
            expected.push(tick);
        }
        for _ in 0..count(&s, p, Call::NeedGunner) {
            actual.push(tick);
        }
        if stopped == 30 {
            let pos = s.cps[cp as usize].def.pos;
            s.unit_mut(h).unwrap().soldier.pos = pos;
        }
        if later.len() == 5 {
            break;
        }
    }
    assert_eq!(later.len(), 5, "five genuine later post departures");
    let wins = later
        .iter()
        .filter(|&&t| crate::ai::unit_unit(p.index, t, 0, crate::ai::SALT_NEED_GUNNER) < 0.3)
        .count();
    assert!(
        wins > 0 && wins < later.len(),
        "independent successful/failed rolls: {later:?}"
    );
    assert_eq!(
        actual, expected,
        "no failed-roll retry or route-tick reroll"
    );
    eprintln!("WGNXT repeated departures={later:?} calls={actual:?}");
}

#[test]
fn wgnxt_later_call_delivers_after_writebacks_and_retains_orders() {
    use crate::squad::CommandKind;
    for receiver_first in [false, true] {
        for order in ["pool", "hold", "follow"] {
            let (mut s, p, v, h) = post_world(receiver_first);
            let a = if receiver_first {
                let a = s.handle(0);
                s.unit_mut(a).unwrap().controller = Controller::Ai;
                a
            } else {
                s.spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(0, Vec3::new(900., 0., 900.))
                })
                .unwrap()
            };
            // Capture from tick zero as in the measured matrix; receiver is
            // absent during both original sources and only arrives at set-out.
            s.unit_mut(h).unwrap().soldier.pos = s.cps[0].def.pos;
            s.unit_mut(a).unwrap().ai.next_think = 1000;
            while s.tick < if receiver_first { 609 } else { 600 } {
                s.step(&[]);
            }
            let tick = s.tick;
            assert!(crate::ai::unit_unit(p.index, tick, 0, crate::ai::SALT_NEED_GUNNER) < 0.3);
            let at = s.unit(p).unwrap().soldier.pos + Vec3::new(0., 0., -20.);
            s.unit_mut(a).unwrap().soldier.pos = at;
            // Same-tick receiver scratch write-back, then a measured next think.
            s.unit_mut(a).unwrap().ai.next_think = tick;
            match order {
                "hold" => {
                    s.issue_command(0, CommandKind::Hold(at));
                }
                "follow" => {
                    s.issue_command(0, CommandKind::Follow(h));
                }
                _ => (),
            }
            let retained = s.unit(a).unwrap().command;
            assert_eq!(retained.is_some(), order != "pool", "actual retained order");
            s.step(&[]);
            assert_eq!(count(&s, p, Call::NeedGunner), 1, "later source call");
            assert_eq!(count(&s, a, Call::Yes), 1, "one issue-time answer");
            assert_eq!(
                s.unit(a).unwrap().ai.pending_board,
                Some((a, v)),
                "receiver write-back must precede delivery"
            );
            let due = s.unit(a).unwrap().ai.next_think;
            assert!(due >= s.tick, "next scheduled think is after issue");
            while s.tick < due {
                s.step(&[]);
                assert_eq!(count(&s, p, Call::NeedGunner), 0);
            }
            s.step(&[]);
            assert_eq!(
                s.unit(a).unwrap().ai.ride.unwrap().board,
                Some(v),
                "next-think exact vehicle"
            );
            assert_eq!(
                s.unit(a).unwrap().command,
                retained,
                "retain authoritative order"
            );
            assert_eq!(s.unit(a).unwrap().ai.ride.unwrap().since, due);
            assert_eq!(
                s.unit(p).unwrap().ai.ride.unwrap().pickup.unwrap().started,
                Some(1)
            );
        }
    }
}

#[test]
fn wgnxt_new_state_hashes_distinct_nonidle_values_and_resets_with_tenure() {
    let (mut s, p, v, _) = stopped_scene(false);
    let base = s.unit(p).unwrap().ai;
    let hash = |st: AiState| {
        let mut bytes = vec![];
        st.hash_into(&mut |b| bytes.extend_from_slice(b));
        bytes
    };
    for field in 0..2 {
        let mut states = vec![];
        for val in [None, Some(0), Some(1)] {
            let mut st = base;
            let w = st.ride.as_mut().unwrap().pickup.as_mut().unwrap();
            if field == 0 {
                w.post_stop = val;
            } else {
                w.post_departure = val.map(u64::from);
            }
            states.push(hash(st));
        }
        assert_ne!(states[0], states[1], "conditional hash field {field}");
        assert_ne!(states[1], states[2], "distinct nonidle hash field {field}");
    }
    s.exit_vehicle(p);
    assert!(s.unit(p).unwrap().ai.ride.unwrap().pickup.is_none());
    assert!(s.enter_vehicle(p, v, 0));
    let w = s.unit(p).unwrap().ai.ride.unwrap().pickup.unwrap();
    assert!(
        w.post_stop.is_none() && w.post_departure.is_none() && !w.departed,
        "fresh tenure"
    );
    let mut stale = base;
    stale.cmd_clear();
    assert!(
        stale.ride.unwrap().pickup.unwrap().post_stop.is_none(),
        "order cannot revive stop"
    );
    s.remove_vehicle(v);
    assert!(
        s.unit(p).unwrap().ai.ride.unwrap().pickup.is_none(),
        "vehicle removal invalidates source"
    );
}

#[test]
fn wgnxt_full_unarmed_single_and_issue_range_controls() {
    for mode in [
        "unarmed", "armed", "full", "single", "outside", "vertical", "enemy", "dead", "mounted",
    ] {
        let (mut s, p, v, h) = post_world(false);
        let a = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(900., 0., 900.))
            })
            .unwrap();
        s.unit_mut(a).unwrap().ai.next_think = 2000;
        s.unit_mut(h).unwrap().soldier.pos = s.cps[0].def.pos;
        while s.tick < 400 {
            s.step(&[]);
        }
        if mode == "armed" {
            arm_gunner_seat(&mut s);
        }
        if mode == "single" {
            s.vehicle_mut(v).unwrap().seats.truncate(1);
        }
        if mode == "full" {
            let rider = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
            assert!(s.enter_vehicle(rider, v, 1));
        }
        while s.tick < 600 {
            s.step(&[]);
        }
        let at = s.unit(p).unwrap().soldier.pos;
        s.unit_mut(a).unwrap().soldier.pos = at
            + match mode {
                "outside" => Vec3::new(0., 0., -31.),
                "vertical" => Vec3::new(0., 31., 0.),
                _ => Vec3::new(0., 0., -20.),
            };
        match mode {
            "enemy" => s.unit_mut(a).unwrap().team = 1,
            "dead" => s.kill_environment(a, crate::sim::DeathCause::Fall),
            "mounted" => {
                assert!(s.enter_vehicle(a, v, 1));
            }
            _ => (),
        }
        s.step(&[]);
        assert_eq!(
            count(&s, p, Call::NeedGunner),
            1,
            "seat-independent source: {mode}"
        );
        let eligible = matches!(mode, "unarmed" | "armed" | "full" | "single");
        assert_eq!(
            count(&s, a, Call::Yes),
            usize::from(eligible),
            "3-D issue recipients: {mode}"
        );
        if let Some(u) = s.unit(a) {
            assert_eq!(u.ai.pending_board, eligible.then_some((a, v)));
        }
        let w = s.unit(p).unwrap().ai.ride.unwrap().pickup.unwrap();
        assert_eq!(w.post_departure, Some(600));
        assert_eq!(w.started, Some(1));
    }
}

#[test]
fn wgnxt_real_lifecycle_clears_armed_and_spent_post_sources() {
    for spent in [false, true] {
        for mode in ["exit", "handover", "death", "vehicle", "bail"] {
            let (mut s, p, v, h) = stopped_scene(false);
            if spent {
                s.unit_mut(h).unwrap().soldier.pos = s.cps[0].def.pos;
                while s
                    .unit(p)
                    .unwrap()
                    .ai
                    .ride
                    .unwrap()
                    .pickup
                    .unwrap()
                    .post_departure
                    .is_none()
                {
                    s.step(&[]);
                }
            }
            let w = s.unit(p).unwrap().ai.ride.unwrap().pickup.unwrap();
            assert!(w.post_stop.is_some() || w.post_departure.is_some());
            match mode {
                "exit" => s.exit_vehicle(p),
                "handover" => {
                    assert!(s.enter_vehicle(h, v, 0));
                    s.exit_vehicle(h);
                }
                "death" => s.kill_environment(p, crate::sim::DeathCause::Fall),
                "vehicle" => s.remove_vehicle(v),
                "bail" => {
                    // Existing bailout draw is part of this tenure; force only
                    // the independently tested bailout eligibility boundary.
                    s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some(0);
                    s.vehicle_mut(v).unwrap().health.cur = 1.;
                    let due = s.unit(p).unwrap().ai.next_think;
                    while s.tick <= due {
                        s.step(&[]);
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                s.units[p.index as usize]
                    .ai
                    .ride
                    .and_then(|r| r.pickup)
                    .is_none(),
                "clear actual tenure: {mode}"
            );
            if mode == "death" {
                let fresh = s
                    .spawn(&UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(0, Vec3::ZERO)
                    })
                    .unwrap();
                assert_eq!(fresh.index, p.index);
                assert_ne!(fresh.gen, p.gen);
                assert!(s.enter_vehicle(fresh, v, 0));
                let w = s.unit(fresh).unwrap().ai.ride.unwrap().pickup.unwrap();
                assert!(
                    w.post_stop.is_none() && w.post_departure.is_none() && !w.departed,
                    "unit generation reuse"
                );
            }
            if mode == "vehicle" {
                let fresh = s.spawn_vehicle(&VehicleSpec {
                    kind: VehicleKind::Walker,
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
                assert_eq!(fresh.index, v.index);
                assert_ne!(fresh.gen, v.gen);
                assert!(s.enter_vehicle(p, fresh, 0));
                let w = s.unit(p).unwrap().ai.ride.unwrap().pickup.unwrap();
                assert!(
                    w.post_stop.is_none() && w.post_departure.is_none() && !w.departed,
                    "vehicle generation reuse"
                );
            }
        }
    }
}
