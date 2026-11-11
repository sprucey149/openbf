//! Card VDROP's checks: the stick law against 347 ticks of PS2 RAM, then by
//! hand the two bikes' pairs, the probe, the unstick manoeuvre, the
//! look-ahead point and the stuck test. (A bot boarding a hover, driving it
//! to its post and getting out is `ride`'s test.)

mod ps2_rows;

use super::*;

fn v3(r: &[f32], i: usize) -> Vec3 {
    Vec3::new(r[i], r[i + 1], r[i + 2])
}

/// Acceptance 2: every PS2 row's four sticks and its unstick state, to 2e-3.
#[test]
fn the_sticks_are_the_ps2s_on_347_ticks() {
    let mut worst = 0.0f32;
    let mut seen = [0u32; ps2_rows::BRANCHES.len()];
    for (i, r) in ps2_rows::ROWS.iter().enumerate() {
        let k = Class {
            forward_speed: r[0],
            strafe_speed: r[1],
            turn_rate: r[2],
            pitch_rate: r[3],
            moving_turn_only: r[4] != 0.0,
            bf1: false,
        };
        let m = Body {
            pos: v3(r, 5),
            left: v3(r, 8),
            fwd: v3(r, 14),
            vel: v3(r, 17),
            aim_origin: v3(r, 20),
            aim_dir: v3(r, 23),
        };
        // Every row's turn is replayable: toward the goal, or an aim order
        // of type 2.
        let o = Order {
            goal: (r[26] != 0.0).then(|| v3(r, 27)),
            path: r[30] != 0.0,
            bit4: r[31] != 0.0,
            stand: r[32] != 0.0,
            probe: r[43] as i8,
            stuck: r[38] != 0.0,
            since_end: r[39],
            since_state2: r[40],
            held: [r[41], r[42]],
        };
        let mut state = r[34] as u8;
        let mut fwd0 = v3(r, 35);
        let (s, _, _) = sticks(&k, &m, &o, &mut state, &mut fwd0);
        let e = (0..4).map(|j| (s[j] - r[44 + j]).abs()).fold(0.0, f32::max);
        let branch = ps2_rows::BRANCHES[r[49] as usize];
        assert!(
            e < 2e-3 && state == r[48] as u8,
            "row {i} ({branch}): {s:?} state {state} vs the PS2's {:?}",
            &r[44..49]
        );
        worst = worst.max(e);
        seen[r[49] as usize] += 1;
    }
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
    let probed = ps2_rows::ROWS.iter().filter(|r| r[43] != 0.0).count();
    eprintln!(
        "VDROP rows: {} of {} within 2e-3 on four sticks and the unstick state (worst {worst:.2e}); {} branches, {probed} rows with a probe hit",
        ps2_rows::ROWS.len(),
        ps2_rows::ROWS.len(),
        seen.len()
    );
}

fn bike(bf1: bool) -> Class {
    Class {
        forward_speed: if bf1 { 40.0 } else { 28.0 },
        strafe_speed: if bf1 { 0.0 } else { 3.0 },
        turn_rate: if bf1 { 2.0 } else { 5.0 },
        pitch_rate: 0.5,
        moving_turn_only: false,
        bf1,
    }
}

fn level(pos: Vec3, fwd: Vec3, vel: Vec3) -> Body {
    Body {
        pos,
        left: Vec3::Y.cross(fwd),
        fwd,
        vel,
        aim_origin: pos + Vec3::Y,
        aim_dir: fwd,
    }
}

fn order(goal: Vec3) -> Order {
    Order {
        goal: Some(goal),
        path: true,
        bit4: false,
        stand: false,
        probe: 0,
        stuck: false,
        since_end: f32::MAX,
        since_state2: f32::MAX,
        held: [0.0; 2],
    }
}

/// The sticks of a hover in unstick state 0.
fn fresh(k: &Class, m: &Body, o: &Order) -> [f32; 4] {
    let (mut state, mut fwd0) = (0, Vec3::ZERO);
    sticks(k, m, o, &mut state, &mut fwd0).0
}

/// §8 by hand: a BF2 bike 20° off its goal holds strafe ±1 and a throttle
/// of (3 / 28) / tan 20° = 0.294; a BF1 bike has cos 20° and no strafe.
#[test]
fn a_bf2_bike_slows_to_turn_and_a_bf1_bike_does_not() {
    let fwd = Vec3::Z;
    let a = 20f32.to_radians();
    let right = -Vec3::Y.cross(fwd);
    let goal = (fwd * a.cos() + right * a.sin()) * 100.0;
    let m = level(Vec3::ZERO, fwd, Vec3::ZERO);
    let s = fresh(&bike(false), &m, &order(goal));
    assert!((s[0] - 0.2943).abs() < 1e-3 && s[1] == 1.0, "{s:?}");
    // k = 1.5: u = sin 20° × 1.5 = 0.513, turn = u².
    assert!((s[2] - 0.2632).abs() < 1e-3, "{s:?}");
    let s = fresh(&bike(true), &m, &order(goal));
    assert!((s[0] - a.cos()).abs() < 1e-4 && s[1] == 0.0, "{s:?}");
    // k = 2.5: u = 0.855.
    assert!((s[2] - 0.7311).abs() < 1e-3, "{s:?}");
}

/// H4: a hit moves the goal L / 4 to the normal's side and halves the
/// throttle; under 3.33 m/s the probe does not run.
#[test]
fn a_probe_hit_shifts_the_goal_and_halves_the_throttle() {
    let m = level(Vec3::ZERO, Vec3::Z, Vec3::Z * 28.0);
    let goal = Vec3::Z * 100.0;
    let clear = fresh(&bike(true), &m, &order(goal));
    assert_eq!((clear[0], clear[2]), (1.0, 0.0));
    let hit = |probe| {
        let o = Order {
            probe,
            ..order(goal)
        };
        fresh(&bike(true), &m, &o)
    };
    // L = 42 m: the goal moves 10.5 m, 5.99° off: cos / 2, and a turn.
    let (p, n) = (hit(1), hit(-1));
    assert!((p[0] - 0.4973).abs() < 1e-3, "{p:?}");
    assert!(p[2] != 0.0 && p[2] == -n[2], "{p:?} {n:?}");
    let slow = level(Vec3::ZERO, Vec3::Z, Vec3::Z * 3.0);
    let o = Order {
        probe: 1,
        ..order(goal)
    };
    let s = fresh(&bike(true), &slow, &o);
    assert_eq!((s[0], s[2]), (1.0, 0.0));
}

/// H8 and §9: stuck with the cooldown run, the hover backs up with half
/// turn until its nose has swung 90°, then drives forward 3 s; BF1's
/// cooldown is 4 s, BF2's 15.
#[test]
fn the_unstick_manoeuvre_runs_its_three_states() {
    let goal = Vec3::Z * 100.0;
    let m = level(Vec3::ZERO, Vec3::Z, Vec3::ZERO);
    let stuck = |since_end| Order {
        stuck: true,
        since_end,
        ..order(goal)
    };
    for (bf1, wait) in [(true, 4.0), (false, 15.0)] {
        let (mut state, mut f0) = (0, Vec3::ZERO);
        sticks(&bike(bf1), &m, &stuck(wait - 0.1), &mut state, &mut f0);
        assert_eq!(state, 0, "inside the cooldown");
        let (s, b, _) = sticks(&bike(bf1), &m, &stuck(wait + 0.1), &mut state, &mut f0);
        assert_eq!((state, s[0], s[2], b), (1, -1.0, 0.5, false));
        // The nose 91° round: state 2 in that tick, already forward.
        let a = 91f32.to_radians();
        let turned = level(Vec3::ZERO, Vec3::new(a.sin(), 0.0, a.cos()), Vec3::ZERO);
        let (s, b, _) = sticks(&bike(bf1), &turned, &order(goal), &mut state, &mut f0);
        assert_eq!((state, s[0], s[2], b), (2, 1.0, 0.5, true));
        let on = Order {
            since_state2: 2.9,
            ..order(goal)
        };
        let (_, _, e) = sticks(&bike(bf1), &turned, &on, &mut state, &mut f0);
        assert_eq!((state, e), (2, false));
        let done = Order {
            since_state2: 3.1,
            ..order(goal)
        };
        let (s, _, e) = sticks(&bike(bf1), &turned, &done, &mut state, &mut f0);
        assert_eq!((state, e, s[0], s[2]), (0, true, 1.0, 0.5));
    }
}

/// §5: the goal is |v| + 0.2 m ahead of the foot on the segment, the node
/// once that passes the end.
#[test]
fn the_goal_leads_the_foot_on_the_segment() {
    let (a, b) = (Vec3::ZERO, Vec3::new(100.0, 5.0, 0.0));
    let pos = Vec3::new(30.0, 2.0, 7.0);
    assert_eq!(look_ahead(a, b, pos, 12.0), Vec3::new(42.2, 2.0, 0.0));
    assert_eq!(look_ahead(a, b, pos, 80.0), Vec3::new(100.0, 2.0, 0.0));
    assert_eq!(look_ahead(b, b, pos, 1.0), Vec3::new(100.0, 2.0, 0.0));
}

/// §7: not 2 m in 3 s, or no new closest distance to the node for 5 s.
#[test]
fn the_stuck_test_reads_still_and_no_nearer() {
    use crate::vehicle::hover::HoverAi;
    let node = Vec3::new(100.0, 0.0, 0.0);
    let dt = crate::TICK_DT;
    // Standing: the word after 3 s.
    let mut f = HoverAi::default();
    let mut at = None;
    for i in 0..200 {
        if stuck(&mut f, dt, Vec3::ONE, node) && at.is_none() {
            at = Some(i as f32 * dt);
        }
    }
    assert!(at.is_some_and(|t| (3.0..3.2).contains(&t)), "{at:?}");
    // Round the node at 10 m/s, drifting off it: the word after 5 s.
    let mut f = HoverAi::default();
    let mut at = None;
    for i in 0..200 {
        let a = i as f32 * dt * 0.2;
        let p = node + Vec3::new(a.cos(), 0.0, a.sin()) * (50.0 + i as f32 * 0.01);
        if stuck(&mut f, dt, p, node) && at.is_none() {
            at = Some(i as f32 * dt);
        }
    }
    assert!(at.is_some_and(|t| (5.0..5.2).contains(&t)), "{at:?}");
    // Driving at it: never.
    let mut f = HoverAi::default();
    for i in 0..200 {
        let p = Vec3::new(i as f32 * 0.3, 0.0, 0.0);
        assert!(!stuck(&mut f, dt, p, node));
    }
}

/// Card HJAM: a driver's segment to a detour corner is begun ONCE. A corner
/// has no height of its own and takes the hover's, which moves every tick;
/// compared whole, the segment was begun anew each tick from the corner to
/// itself (no point ahead on a line, and the stuck test's "no nearer for
/// 5 s" clock set back every tick).
#[test]
fn the_segment_to_a_corner_is_kept_while_the_hover_bobs() {
    use crate::ai::state::Route;
    use crate::ai::tests::l_sim;
    use crate::vehicle::hover::tests::skiff;
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    use glam::Vec2;
    let (mut sim, h) = l_sim();
    let hp = skiff();
    let v = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 2.0, 0.0),
        yaw: std::f32::consts::PI,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    assert!(sim.enter_vehicle(h, v, 0));
    // A leg to hub 1 with one corner 60 m off, beyond the node radius.
    let corner = Vec2::new(30.0, -52.0);
    assert!(corner.length() > Class::of(&hp).arrive() + 10.0);
    let mut route = Route {
        dest_hub: 2,
        next_hub: 1,
        via_arc: Some(0),
        waypoint: Vec3::new(0.0, 0.0, -40.0),
        ..Route::default()
    };
    route.detour[0] = corner;
    route.detour_len = 1;
    let mut st = AiState {
        nav: crate::ai::route::NavState::for_fixture(Some(route)),
        target_cp: Some(0),
        ..AiState::default()
    };
    let m = sim.unit(h).and_then(|u| u.mount).expect("seated");
    let mut legs = Vec::new();
    for i in 0..4 {
        let (_, _) = drive(&sim, h, m, &hp, &mut st, NavTick::default());
        let out = st.ride.and_then(|r| r.hover).expect("the driver's sticks");
        legs.push((out.ai.leg.expect("a segment"), out.ai.far));
        // `Sim::ride` stores the follower on the hover; the body bobs.
        let veh = sim.vehicle_mut(v).expect("the hover");
        veh.hover.ai = out.ai;
        veh.pos.y += 0.05 * (i as f32 + 1.0);
    }
    let (first, _) = legs[0];
    assert!(
        nav::xz_dist(first[0], first[1]) > 50.0,
        "from the hover to the corner: {first:?}"
    );
    for (leg, _) in &legs {
        assert_eq!(*leg, first, "the segment is begun once");
    }
    // ...and the stuck test's clock runs on: no nearer on any of the ticks.
    assert!(legs[3].1 > legs[1].1, "far: {:?}", legs);
}
