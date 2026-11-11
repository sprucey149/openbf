//! HWAIT: real ground-driver dispatch, human eligibility and tenure controls.
use super::*;
use crate::ai::THINK_PERIOD;
use crate::unit::{Controller, UnitSpec};

#[derive(Clone, Copy, Debug)]
enum HumanCase {
    Near,
    FarFacing,
    FarAway,
    Enemy,
    Dead,
    Mounted,
}

fn scenario(rules: GroundRules, kind: VehicleKind, full: bool, case: HumanCase) -> bool {
    let (mut s, b, v) = pickup_world(kind);
    s.ground_rules = rules;
    for _ in 0..300 {
        s.step(&[]);
        if s.unit(b).unwrap().mount.is_some() {
            break;
        }
    }
    assert_eq!(s.unit(b).unwrap().mount.unwrap().vehicle, v);
    let seated = s.tick;
    let pos = s.vehicle(v).unwrap().pos;
    let offset = match case {
        HumanCase::FarFacing => Vec3::new(0.0, 0.0, 100.0),
        HumanCase::FarAway => Vec3::new(0.0, 0.0, 30.0),
        _ => Vec3::new(8.0, 0.0, 0.0),
    };
    let h = s
        .spawn(&UnitSpec::player(
            u8::from(matches!(case, HumanCase::Enemy)),
            pos + offset,
        ))
        .unwrap();
    let u = s.unit_mut(h).unwrap();
    u.soldier.pos.y = 0.0;
    u.soldier.yaw = if matches!(case, HumanCase::FarAway) {
        std::f32::consts::PI
    } else {
        0.0
    };
    u.soldier.pitch = 0.0;
    if matches!(case, HumanCase::Dead) {
        u.alive = false;
        u.health.cur = 0.0;
    }
    if matches!(case, HumanCase::Mounted) {
        assert!(!full);
        assert!(s.enter_vehicle(h, v, 1));
    } else if full {
        let rider = s
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::ZERO))
            .unwrap();
        assert!(s.enter_vehicle(rider, v, 1));
    }
    assert_eq!(s.players[0], Some(h));
    let human = &s.units[h.index as usize];
    assert_eq!(human.gen, h.gen);
    assert_eq!(human.controller, Controller::Player { slot: 0 });
    assert_eq!(human.alive, !matches!(case, HumanCase::Dead));
    assert_eq!(human.team, u8::from(matches!(case, HumanCase::Enemy)));
    assert_eq!(human.mount.is_some(), matches!(case, HumanCase::Mounted));
    assert_eq!(human.soldier.pitch, 0.0);
    assert_eq!(
        human.soldier.yaw,
        if matches!(case, HumanCase::FarAway) {
            std::f32::consts::PI
        } else {
            0.0
        }
    );
    assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, Some(b));
    let mut first_drive = None;
    let mut max_stick: f32 = 0.0;
    for _ in 0..180 {
        s.step(&[]);
        assert!(
            s.unit(b).unwrap().mount.is_some(),
            "pilot exited {kind:?} {case:?}"
        );
        let stick = locomotion(&s, b);
        max_stick = max_stick.max(stick);
        if stick > 0.0 && first_drive.is_none() {
            first_drive = Some(s.tick);
        }
    }
    let delta = nav::xz_dist(pos, s.vehicle(v).unwrap().pos);
    let expected_wait =
        rules == GroundRules::Bf2 && matches!(case, HumanCase::Near | HumanCase::FarFacing);
    let wait = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
    assert_eq!(wait.seated + 1, seated, "successful entry clock");
    assert_eq!(wait.started, Some(seated), "first seated think clock");
    assert_eq!(wait.complete, !expected_wait, "tenure completion");
    let pass = if expected_wait {
        max_stick == 0.0 && delta < 0.1
    } else {
        max_stick > 0.0 && delta > 1.0
    };
    eprintln!("HWAIT rules={rules:?} kind={kind:?} full={full} case={case:?} seated={seated} wait={:?} tick={} first_drive={first_drive:?} max_stick={max_stick:.6} delta={delta:.6} expected_wait={expected_wait} pass={pass}",s.unit(b).unwrap().ai.ride.unwrap().pickup,s.tick);
    pass
}

#[test]
fn hwait_actual_step() {
    let mut failed = 0;
    let mut total = 0;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for full in [false, true] {
                for case in [
                    HumanCase::Near,
                    HumanCase::FarFacing,
                    HumanCase::FarAway,
                    HumanCase::Enemy,
                    HumanCase::Dead,
                ] {
                    total += 1;
                    if !scenario(rules, kind, full, case) {
                        failed += 1;
                    }
                }
            }
            total += 1;
            if !scenario(rules, kind, false, HumanCase::Mounted) {
                failed += 1;
            }
        }
    }
    assert_eq!(
        failed, 0,
        "{failed}/{total} actual-step human pickup scenarios violate existing rule"
    );
}

#[test]
fn hwait_actual_step_deadline_and_boarding() {
    let mut failed = 0;
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for action in ["expire-empty", "expire-full", "board"] {
            let (mut s, b, v) = pickup_world(kind);
            s.ground_rules = GroundRules::Bf2;
            for _ in 0..300 {
                s.step(&[]);
                if s.unit(b).unwrap().mount.is_some() {
                    break;
                }
            }
            assert_eq!(s.unit(b).unwrap().mount.unwrap().vehicle, v);
            let pos = s.vehicle(v).unwrap().pos;
            let h = s
                .spawn(&UnitSpec::player(0, pos + Vec3::new(8.0, 0.0, 0.0)))
                .unwrap();
            s.unit_mut(h).unwrap().soldier.pos.y = 0.0;
            if action == "expire-full" {
                let rider = s
                    .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::ZERO))
                    .unwrap();
                assert!(s.enter_vehicle(rider, v, 1));
            }
            let mut max_held: f32 = 0.0;
            let mut first_drive = None;
            for _ in 0..300 {
                s.step(&[]);
                let stick = locomotion(&s, b);
                max_held = max_held.max(stick);
                if stick > 0.0 && first_drive.is_none() {
                    first_drive = Some(s.tick);
                }
            }
            let start = s
                .unit(b)
                .unwrap()
                .ai
                .ride
                .unwrap()
                .pickup
                .unwrap()
                .started
                .unwrap();
            let delta_held = nav::xz_dist(pos, s.vehicle(v).unwrap().pos);
            let before = s.tick;
            if action == "board" {
                assert!(s.enter_vehicle(h, v, 1));
            }
            let end = if action == "board" {
                before + THINK_PERIOD + 3
            } else {
                start + 450 + THINK_PERIOD + 3
            };
            while s.tick < end {
                s.step(&[]);
                assert!(
                    s.unit(b).unwrap().mount.is_some(),
                    "pilot exited {kind:?} {action}"
                );
                if locomotion(&s, b) > 0.0 && first_drive.is_none() {
                    first_drive = Some(s.tick);
                }
            }
            let interval = if action == "board" {
                before..=end
            } else {
                (start + 450)..=end
            };
            let pass = max_held == 0.0
                && delta_held < 0.1
                && first_drive.is_some_and(|t| interval.contains(&t));
            eprintln!("HWAIT_LONG kind={kind:?} action={action} start={start} held_to={before} max_held={max_held:.6} delta_held={delta_held:.6} first_drive={first_drive:?} pass={pass}");
            if !pass {
                failed += 1;
            }
        }
    }
    assert_eq!(failed, 0, "{failed}/6 long-wait/boarding scenarios failed");
}

// All geometry is independently chosen, rather than reading runtime thresholds.
fn human_scene(
    kind: VehicleKind,
    distance: f32,
    yaw: f32,
) -> (Sim, UnitHandle, VehicleHandle, UnitHandle) {
    let (mut s, b, v) = pickup_world(kind);
    s.ground_rules = GroundRules::Bf2;
    while s.unit(b).unwrap().mount.is_none() {
        assert!(s.tick < 300, "natural boarding");
        s.step(&[]);
    }
    let pos = s.vehicle(v).unwrap().pos;
    let h = s
        .spawn(&UnitSpec::player(
            0,
            Vec3::new(pos.x, 0.0, pos.z + distance),
        ))
        .unwrap();
    s.unit_mut(h).unwrap().soldier.yaw = yaw;
    s.unit_mut(h).unwrap().soldier.pitch = 0.0;
    assert_eq!(s.players[0], Some(h));
    assert_eq!(
        s.unit(h).unwrap().controller,
        Controller::Player { slot: 0 }
    );
    assert!(s.unit(h).unwrap().alive);
    assert_eq!(s.unit(h).unwrap().team, s.unit(b).unwrap().team);
    assert!(s.unit(h).unwrap().mount.is_none());
    (s, b, v, h)
}

fn held_steps(s: &mut Sim, b: UnitHandle, v: VehicleHandle, n: u64) {
    let pos = s.vehicle(v).unwrap().pos;
    for _ in 0..n {
        s.step(&[]);
        assert_eq!(s.unit(b).unwrap().mount.unwrap().vehicle, v);
        assert_eq!(locomotion(s, b), 0.0, "human hold at {}", s.tick);
        assert!(nav::xz_dist(pos, s.vehicle(v).unwrap().pos) < 0.1);
        assert!(s.vehicle(v).unwrap().stuck_at.is_none());
        assert!(!s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
    }
}

#[test]
fn hwait_independent_near_facing_cone_and_range_controls() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for (distance, degrees, hold) in [
            (11.7, 180.0, true),
            (12.01, 180.0, false),
            (30.0, 29.0, true),
            (30.0, 31.0, false),
            (149.9, 0.0, true),
            (150.1, 0.0, false),
        ] {
            let (mut s, b, v, _) = human_scene(kind, distance, (degrees as f32).to_radians());
            if hold {
                held_steps(&mut s, b, v, 180);
            } else {
                for _ in 0..180 {
                    s.step(&[]);
                }
                assert!(
                    locomotion(&s, b) > 0.0,
                    "departure {kind:?} d={distance} yaw={degrees}"
                );
            }
        }
    }
}

#[test]
fn hwait_eligibility_loss_returns_to_initial_window_or_full_completion() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for full in [false, true] {
            for after in [30, 180] {
                let (mut s, b, v, h) = human_scene(kind, 8.0, std::f32::consts::PI);
                if full {
                    let r = s
                        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::ZERO))
                        .unwrap();
                    assert!(s.enter_vehicle(r, v, 1));
                }
                held_steps(&mut s, b, v, after);
                s.unit_mut(h).unwrap().team = 1;
                s.step(&[]);
                if !full && after == 30 {
                    assert_eq!(locomotion(&s, b), 0.0, "remaining initial window");
                    while s.tick < 140 {
                        s.step(&[]);
                    }
                }
                assert!(
                    locomotion(&s, b) > 0.0,
                    "eligibility lost {kind:?} {full} {after}"
                );
                assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
                // A later qualifying arrival and later vacancy cannot restart it.
                s.unit_mut(h).unwrap().team = 0;
                if full {
                    let r = s.vehicle(v).unwrap().seats[1].occupant.unwrap();
                    s.exit_vehicle(r);
                }
                for _ in 0..10 {
                    s.step(&[]);
                    assert!(locomotion(&s, b) > 0.0);
                }
            }
        }
    }
}

#[test]
fn hwait_current_slots_controller_and_respawn_generation() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for controller in [
            Controller::Ai,
            Controller::Idle,
            Controller::Player { slot: 1 },
        ] {
            let (mut s, b, _, h) = human_scene(kind, 8.0, 0.0);
            s.unit_mut(h).unwrap().controller = controller;
            // Keep the synthetic impostor still; do not let its AI board.
            s.unit_mut(h).unwrap().ai.next_think = u64::MAX;
            for _ in 0..180 {
                s.step(&[]);
            }
            assert!(locomotion(&s, b) > 0.0, "nonhuman slot {controller:?}");
        }
        let (mut s, b, v, h) = human_scene(kind, 8.0, 0.0);
        held_steps(&mut s, b, v, 150);
        // Ordinary removal followed by replacement reuses the unit slot with a new generation.
        assert!(s.vacate_player_slot(0).is_some());
        let pos = s.vehicle(v).unwrap().pos;
        let new = s
            .spawn(&UnitSpec::player(0, pos + Vec3::new(0.0, 0.0, 8.0)))
            .unwrap();
        assert_eq!(new.index, h.index);
        assert_ne!(new.gen, h.gen);
        assert_eq!(s.players[0], Some(new));
        held_steps(&mut s, b, v, 30);
        s.players[0] = Some(h); // stale generation must not resolve to replacement
        s.step(&[]);
        assert!(locomotion(&s, b) > 0.0, "stale slot cannot hold");
        let (mut s, b, v, h) = human_scene(kind, 8.0, 0.0);
        s.unit_mut(h).unwrap().team = 1;
        let pos = s.vehicle(v).unwrap().pos;
        let second = s
            .spawn(&UnitSpec::player_in_slot(
                1,
                0,
                pos + Vec3::new(0.0, 0.0, 100.0),
            ))
            .unwrap();
        assert_eq!(s.players[1], Some(second));
        held_steps(&mut s, b, v, 180);
    }
}

#[test]
fn hwait_tenure_reentry_and_delayed_first_think() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, b, v, _) = human_scene(kind, 8.0, 0.0);
        // Full occupancy precedes the first seated think; a delayed think must
        // still establish the one shared clock rather than ending at fullness.
        let r = s
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::ZERO))
            .unwrap();
        assert!(s.enter_vehicle(r, v, 1));
        s.unit_mut(b).unwrap().ai.next_think = 20;
        held_steps(&mut s, b, v, 19);
        assert_eq!(
            s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().started,
            None
        );
        held_steps(&mut s, b, v, 1);
        assert_eq!(
            s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().started,
            Some(20)
        );
        while s.tick < 470 {
            held_steps(&mut s, b, v, 1);
        }
        s.step(&[]);
        assert!(locomotion(&s, b) > 0.0, "15s deadline from delayed think");
        let old = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
        s.exit_vehicle(b);
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
        assert!(s.enter_vehicle(b, v, 0));
        for _ in 0..30 {
            s.step(&[]);
            assert_eq!(locomotion(&s, b), 0.0);
            assert_eq!(s.unit(b).unwrap().mount.unwrap().vehicle, v);
        }
        let new = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
        assert!(new.started.unwrap() > old.started.unwrap());
        assert!(!new.complete);
    }
}

#[test]
fn hwait_extension_preserves_route_rng_events_and_bailout() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, b, v, _) = human_scene(kind, 8.0, 0.0);
        held_steps(&mut s, b, v, 180);
        let mut st = s.unit(b).unwrap().ai;
        st.install_route_for_fixture(Some(super::super::super::state::Route {
            waypoint: Vec3::new(0.0, 0.0, -300.0),
            ref_dist: 300.0,
            stuck_ticks: 59,
            stuck_run: 100,
            sidestep: 7,
            ..Default::default()
        }));
        let route = st.route();
        let rng = s.rng.state();
        let events = s.events.len();
        let follower = s.vehicle(v).unwrap().hover.ai;
        let m = s.unit(b).unwrap().mount.unwrap();
        let (_, nt) = drive(&s, b, m, &mut st, NavTick::default(), true);
        assert_eq!(st.route(), route);
        assert_eq!(nt.stuck_run, 0);
        assert!(!st.ride.unwrap().use_req);
        if let Some(h) = st.ride.unwrap().hover {
            assert_eq!(h.ai, follower);
        }
        assert_eq!(s.rng.state(), rng);
        assert_eq!(s.events.len(), events);
        s.unit_mut(b).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some(0);
        s.unit_mut(b).unwrap().ai.next_think = s.tick;
        s.vehicle_mut(v).unwrap().health.cur = 100.0;
        s.step(&[]);
        assert!(
            s.unit(b).unwrap().mount.is_none(),
            "bailout during human extension"
        );
    }
}

#[test]
fn hwait_exact_three_dimensional_predicate_boundaries() {
    let (mut s, b, v, h) = human_scene(VehicleKind::Hover, 8.0, 0.0);
    let m = s.unit(b).unwrap().mount.unwrap();
    let pos = s.vehicle(v).unwrap().pos;
    for (distance, degrees, expected) in [
        (11.999_f32, 180.0_f32, true),
        (12.001, 180.0, false),
        (30.0, 29.99, true),
        (30.0, 30.01, false),
        (150.0, 0.0, true),
        (150.001, 0.0, false),
    ] {
        let u = s.unit_mut(h).unwrap();
        u.soldier.pos = pos + Vec3::Z * distance;
        u.soldier.yaw = degrees.to_radians();
        assert_eq!(
            human_approaching(&s, m),
            expected,
            "d={distance} yaw={degrees}"
        );
    }
    // An overhead human is outside the sphere even though its XZ is near.
    s.unit_mut(h).unwrap().soldier.pos = pos + Vec3::Y * 151.0;
    assert!(!human_approaching(&s, m));
    s.unit_mut(h).unwrap().soldier.pos = pos + Vec3::Z * 8.0;
    s.players[0] = None;
    assert!(!human_approaching(&s, m), "human absent from current slots");
}
