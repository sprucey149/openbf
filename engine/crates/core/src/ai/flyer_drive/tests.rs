use super::*;
use crate::cp::CpDef;
use crate::region::Region;
use crate::unit::{Controller, UnitSpec};
use crate::vehicle::flyer::{curve_at, FlyerPath};
use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
use crate::weapon::HealthType;

fn xwing(bf2: bool) -> FlyerParams {
    let base = if bf2 {
        FlyerParams::bf2()
    } else {
        FlyerParams::default()
    };
    FlyerParams {
        acceleration: 30.0,
        min_speed: 20.0,
        mid_speed: 50.0,
        max_speed: 82.0,
        pitch_rate: 1.0,
        turn_rate: 1.25,
        pitch_filter: 2.9,
        turn_filter: 1.75,
        takeoff_height: 2.5,
        takeoff_time: 1.0,
        takeoff_speed: 3.0,
        landing_time: 2.0,
        landing_speed: 5.0,
        landed_height: 1.0,
        ..base
    }
}

fn post(s: &mut Sim, name: &str, at: Vec3, owner: Option<u8>) {
    s.add_cp(
        CpDef {
            name: name.into(),
            pos: at,
            capture: Region::sphere(at, 8.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        owner,
    );
}

/// Flat ground, a far enemy post, a bot at the origin and a landed fighter.
fn world(bf2: bool, flyer_bots: bool) -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(3);
    if bf2 {
        s.ground_rules = crate::soldier::GroundRules::Bf2;
    }
    s.heightfield = Some(crate::heightfield::Heightfield {
        size: 2,
        unit: 8000.0,
        offset_x: 4000.0,
        offset_z: 4000.0,
        heights: vec![0.0; 4],
    });
    s.flyer_bots = flyer_bots;
    s.flyer_params = vec![Some(xwing(bf2))];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 0x20,
        guns: Vec::new(),
    })];
    post(&mut s, "far", Vec3::new(0.0, 0.0, -600.0), Some(1));
    post(&mut s, "other", Vec3::new(600.0, 0.0, 0.0), Some(1));
    let b = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(12.0, 1.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    (s, b, v)
}

#[test]
fn a_bf1_bot_boards_waits_takes_off_and_flies_over_the_posts() {
    let (mut s, b, v) = world(false, true);
    let (mut boarded, mut flying) = (None, None);
    let mut closest = f32::MAX;
    for _ in 0..(60 * crate::TICK_HZ) {
        s.step(&[]);
        if boarded.is_none() && s.unit(b).unwrap().mount.is_some() {
            boarded = Some(s.tick);
        }
        let c = s.vehicle(v).unwrap();
        if flying.is_none() && c.flyer.phase == FlightPhase::Flying {
            flying = Some(s.tick);
        }
        if flying.is_some() {
            let ground = 0.0;
            assert!(c.pos.y > ground, "tick {} crashed at {:?}", s.tick, c.pos);
            for p in &s.cps {
                closest = closest.min((c.pos - p.def.pos).length());
            }
        }
    }
    let boarded = boarded.expect("never boarded");
    let flying = flying.expect("never flew");
    // A one-seat fighter is full, so it has no pickup wait; TakeoffTime 1 s.
    assert!(flying >= boarded + u64::from(crate::TICK_HZ));
    assert!(flying < boarded + 8 * u64::from(crate::TICK_HZ));
    assert!(closest < 90.0, "passed no post closer than {closest}");
    assert_eq!(s.unit(b).unwrap().mount.map(|m| m.vehicle), Some(v));
}

#[test]
fn a_free_seat_holds_the_takeoff_for_the_pickup_wait() {
    let (mut s, b, v) = world(false, true);
    s.seat_setup[0].as_mut().unwrap().boardable = vec![true, true];
    s.vehicle_mut(v).unwrap().seats.push(crate::vehicle::Seat {
        kind: SeatKind::Gunner(0),
        occupant: None,
    });
    let mut boarded = None;
    for _ in 0..(20 * crate::TICK_HZ) {
        s.step(&[]);
        if s.unit(b).unwrap().mount.is_some() {
            let at = *boarded.get_or_insert(s.tick);
            if s.vehicle(v).unwrap().flyer.phase != FlightPhase::Landed {
                assert!(
                    s.tick > at + 4 * u64::from(crate::TICK_HZ),
                    "left at {}",
                    s.tick - at
                );
                return;
            }
        }
    }
    panic!("never left");
}

#[test]
fn the_switch_keeps_bots_off_and_adds_nothing_to_the_hash() {
    let (mut a, _, va) = world(false, false);
    let (mut b, _, _) = world(false, false);
    for _ in 0..(15 * crate::TICK_HZ) {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(a.hash(), b.hash());
    let c = a.vehicle(va).unwrap();
    assert!(c.is_empty());
    assert_eq!(c.flyer, crate::vehicle::flyer::FlyerState::default());
}

#[test]
fn flying_bots_replay_bit_exact() {
    let run = || {
        let (mut s, _, _) = world(false, true);
        for _ in 0..(30 * crate::TICK_HZ) {
            s.step(&[]);
        }
        s.hash()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_flyer_in_the_air_is_not_claimed() {
    let (mut s, b, v) = world(false, true);
    s.vehicle_mut(v).unwrap().flyer.phase = FlightPhase::Flying;
    assert_eq!(super::super::ride::claim_seat(&s, b, v), None);
    s.vehicle_mut(v).unwrap().flyer.phase = FlightPhase::Landed;
    assert_eq!(super::super::ride::claim_seat(&s, b, v), Some(0));
}

#[test]
fn a_blocked_sky_escapes_forward_until_it_clears() {
    let (s, _, _) = world(false, true);
    let v = s.vehicles[0].clone();
    let mut ai = FlyerAi::default();
    let handle = VehicleHandle {
        index: 0,
        gen: v.gen,
    };
    let p = xwing(false);
    let me = UnitHandle {
        index: 0,
        gen: s.units[0].gen,
    };
    let _ = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    // Nothing overhead: straight to the travel state, over a post.
    assert_eq!(ai.agent, FlyAgent::Travel);
    assert_eq!(ai.goal.unwrap().y, POST_CLEARANCE);
}

#[test]
fn a_wedged_craft_keeps_its_way_out_under_an_open_sky() {
    let (mut s, _, _) = world(false, true);
    s.tick = WEDGED_TICKS + 1;
    let handle = VehicleHandle {
        index: 0,
        gen: s.vehicles[0].gen,
    };
    let me = UnitHandle {
        index: 0,
        gen: s.units[0].gen,
    };
    s.vehicles[0].flyer.phase = FlightPhase::Flying;
    let v = s.vehicles[0].clone();
    let p = xwing(false);
    let mut ai = FlyerAi {
        agent: FlyAgent::Travel,
        goal: Some(Vec3::new(0.0, 20.0, -600.0)),
        anchor: v.pos,
        ..FlyerAi::default()
    };
    let _ = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    assert_eq!(ai.agent, FlyAgent::Escape);
    let out = ai.goal.expect("a way out");
    // Nothing is overhead, and the next tick must not drop it for that.
    s.tick += 1;
    let _ = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    assert_eq!((ai.agent, ai.goal), (FlyAgent::Escape, Some(out)));
    // The hold ends and the upward probe has its say again.
    s.tick = ai.wedged_until;
    ai.anchor_at = s.tick;
    let _ = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    assert_eq!(ai.agent, FlyAgent::Travel);
}

#[test]
fn a_pilot_new_to_a_rising_craft_gets_the_whole_takeoff_clock() {
    let (mut s, b, v) = world(false, true);
    s.tick = 10 * TAKEOFF_GIVEUP;
    s.vehicles[0].flyer.phase = FlightPhase::Takeoff;
    assert!(s.seat_unit(b, v, 0));
    s.step(&[]);
    assert!(
        s.unit(b).unwrap().mount.is_some(),
        "gave up on its first tick at the stick"
    );
    let at = s.vehicle(v).unwrap().flyer.ai.takeoff_at;
    assert!(at >= 10 * TAKEOFF_GIVEUP, "clock at {at}");
}

#[test]
fn a_bot_pitches_on_its_aim_not_on_the_tilted_nose() {
    let (mut s, _, _) = world(false, true);
    s.vehicles[0].flyer.phase = FlightPhase::Flying;
    s.vehicles[0].pos = Vec3::new(0.0, 100.0, 0.0);
    // Level flight toward a goal dead ahead at its own height, with the visual
    // tilt holding the drawn nose 0.3 rad up: no pitch to ask for.
    s.vehicles[0].flyer.tilt_pitch = -0.3;
    s.vehicles[0].rot = crate::vehicle::hover::quat_x(-0.3);
    let v = s.vehicles[0].clone();
    let handle = VehicleHandle {
        index: 0,
        gen: v.gen,
    };
    let me = UnitHandle {
        index: 0,
        gen: s.units[0].gen,
    };
    let p = xwing(false);
    let mut ai = FlyerAi {
        agent: FlyAgent::Travel,
        goal: Some(Vec3::new(0.0, 100.0, 3000.0)),
        anchor: v.pos,
        anchor_at: s.tick,
        ..FlyerAi::default()
    };
    let [_, _, _, pitch] = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    assert!(pitch.abs() < 0.05, "pitch {pitch}");
}

#[test]
fn bf1_throttle_law_has_the_three_neutral_branches() {
    let p = xwing(false);
    assert_eq!(throttle_for(1.0, &p), 1.0);
    assert_eq!(throttle_for(0.0, &p), -1.0);
    assert_eq!(throttle_for(0.5, &p), 0.0);
    // A gunship: Min < 0, Mid = 0.
    let g = FlyerParams {
        min_speed: -15.0,
        mid_speed: 0.0,
        max_speed: 45.0,
        ..p.clone()
    };
    assert_eq!(throttle_for(0.0, &g), 0.0);
    assert_eq!(throttle_for(1.0, &g), 1.0);
    let c = FlyerParams {
        min_speed: -10.0,
        mid_speed: 10.0,
        ..p
    };
    assert_eq!(throttle_for(0.0, &c), -0.5);
}

#[test]
fn speed_share_slows_for_a_turn_and_holds_the_floor() {
    let p = xwing(false);
    let far = Vec3::Z * (p.max_speed * 4.0);
    assert!((speed_share(far, Vec3::Z, &p) - 1.0).abs() < 1e-4);
    let floor = SLOW_FLOOR / p.max_speed;
    assert_eq!(speed_share(far, Vec3::X, &p), floor);
    assert_eq!(speed_share(-far, Vec3::Z, &p), floor);
}

#[test]
fn stick_law_is_the_signed_square_of_the_clamped_gain() {
    let p = xwing(false);
    let body = Body {
        pos: Vec3::ZERO,
        left: Vec3::X,
        fwd: Vec3::Z,
        vel: Vec3::Z,
        aim_origin: Vec3::ZERO,
        aim_dir: Vec3::Z,
    };
    // Right is -X. A goal at 30 degrees right: s = 0.5, gain 4.5 - 1.25 = 3.25,
    // clamped to one, so a full positive (rightward) stick.
    let (turn, pitch) = stick_to(&p, &body, Vec3::new(-50.0, 0.0, 86.60254));
    assert_eq!(turn, 1.0);
    assert_eq!(pitch, 0.0);
    // A small error stays inside the clamp: 0.1 x 3.25 = 0.325, squared.
    let (turn, _) = stick_to(&p, &body, Vec3::new(-10.0, 0.0, 99.49874));
    assert!((turn - 0.325 * 0.325).abs() < 1e-3, "{turn}");
    // Up is positive pitch; gain 5 - 1 = 4. A goal behind is a full stick.
    let (_, pitch) = stick_to(&p, &body, Vec3::new(0.0, 10.0, 99.49874));
    assert!((pitch - 0.16).abs() < 1e-3, "{pitch}");
    let (turn, _) = stick_to(&p, &body, Vec3::new(-1.0, 0.0, -100.0));
    assert_eq!(turn, 1.0);
}

#[test]
fn the_goal_rises_over_ground_ahead_and_relaxes_on_arrival() {
    let (s, _, _) = world(false, true);
    let mut v = s.vehicles[0].clone();
    v.pos = Vec3::new(0.0, 10.0, 0.0);
    v.vel = Vec3::Z * 50.0;
    let h = VehicleHandle {
        index: 0,
        gen: v.gen,
    };
    let p = xwing(false);
    // L = 75: the goal 750 m off is lifted toward 10 + 750 * (25 - 10) / 75.
    let (far, _) = lift_goal(&s, &v, h, &p, Vec3::new(0.0, 10.0, 750.0));
    assert!((far.y - 160.0).abs() < 0.01, "{far:?}");
    // Next to the goal the lift has relaxed to the goal's own height.
    let (near, _) = lift_goal(&s, &v, h, &p, Vec3::new(0.0, 12.0, 1.0));
    assert!((near.y - 12.0).abs() < 0.01, "{near:?}");
    let mut s = s;
    s.flight_limits.ai[1] = 90.0;
    let (capped, _) = lift_goal(&s, &v, h, &p, Vec3::new(0.0, 10.0, 750.0));
    assert_eq!(capped.y, 90.0);
}

#[test]
fn catmull_rom_passes_its_inner_points_with_their_tangents() {
    let p = [
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(20.0, 5.0, 0.0),
        Vec3::new(30.0, 5.0, 10.0),
    ];
    let (at0, tan0) = curve_at(&p, 0.0);
    let (at1, tan1) = curve_at(&p, 1.0);
    assert!((at0 - p[1]).length() < 1e-5);
    assert!((at1 - p[2]).length() < 1e-5);
    assert!((tan0 - (p[2] - p[0]) * 0.5).length() < 1e-5);
    assert!((tan1 - (p[3] - p[1]) * 0.5).length() < 1e-5);
}

fn bf2_world() -> (Sim, UnitHandle, VehicleHandle) {
    let (mut s, b, v) = world(true, true);
    s.ground_flyer_map = true;
    s.flyer_paths = vec![
        FlyerPath {
            nodes: vec![
                Vec3::new(0.0, 40.0, 400.0),
                Vec3::new(300.0, 40.0, 600.0),
                Vec3::new(600.0, 60.0, 400.0),
                Vec3::new(600.0, 60.0, 0.0),
            ],
        },
        FlyerPath {
            nodes: vec![
                Vec3::new(-400.0, 40.0, 0.0),
                Vec3::new(-400.0, 40.0, -400.0),
            ],
        },
    ];
    (s, b, v)
}

#[test]
fn a_bf2_bot_takes_off_then_flies_the_authored_paths() {
    let (mut s, b, v) = bf2_world();
    let mut near = [f32::MAX; 2];
    let mut top = 0.0f32;
    let mut followed = 0;
    let mut last = s.vehicle(v).unwrap().pos;
    for _ in 0..(90 * crate::TICK_HZ) {
        s.step(&[]);
        let c = s.vehicle(v).unwrap();
        if c.flyer.ai.curve.is_some() {
            followed += 1;
            let moved = (c.pos - last).length();
            assert!(moved <= 15.0, "tick {} jumped {moved}", s.tick);
            top = top.max(c.vel.length());
            near[0] = near[0].min((c.pos - s.flyer_paths[0].nodes[2]).length());
            near[1] = near[1].min((c.pos - s.flyer_paths[1].nodes[1]).length());
        }
        last = c.pos;
    }
    assert_eq!(s.unit(b).unwrap().mount.map(|m| m.vehicle), Some(v));
    assert!(followed > 60 * crate::TICK_HZ, "followed for {followed}");
    assert!(near[0] < 40.0 || near[1] < 40.0, "{near:?}");
    let cap = 82.0 * 1.05;
    assert!(top <= cap * 1.5, "speed {top}");
}

#[test]
fn a_human_at_the_seat_is_not_flown_by_a_leftover_curve() {
    let (mut s, _, v) = bf2_world();
    let (from, to) = (Vec3::new(0.0, 40.0, 0.0), Vec3::new(0.0, 40.0, 900.0));
    let human = s.spawn(&UnitSpec::player(0, from)).unwrap();
    let c = s.vehicle_mut(v).unwrap();
    c.pos = from;
    c.flyer.phase = FlightPhase::Flying;
    c.flyer.ai.curve = Some([from, from, to, to]);
    s.vehicle_mut(v).unwrap().seats[0].occupant = None;
    assert!(s.seat_unit(human, v, 0));
    s.step(&[crate::sim::PadState::default()]);
    let c = s.vehicle(v).unwrap();
    assert!(c.input.human);
    assert_eq!(c.flyer.ai.t, 0.0, "the follower ran under a human");
}

#[test]
fn a_bf2_map_with_no_paths_gets_no_pilots() {
    let (s, b, v) = world(true, true);
    assert_eq!(super::super::ride::claim_seat(&s, b, v), None);
}

#[test]
fn a_pursuit_leads_the_target_and_ends_with_the_escape_timer() {
    let (s, _, _) = bf2_world();
    let mut mine = s.vehicles[0].clone();
    mine.pos = Vec3::new(0.0, 50.0, 0.0);
    let mut target = mine.clone();
    target.pos = Vec3::new(0.0, 50.0, 200.0);
    let me = UnitHandle {
        index: 0,
        gen: s.units[0].gen,
    };
    let c = pursuit_curve(&s, me, &mine, &target);
    assert_eq!(c[1], mine.pos);
    assert_eq!(c[2], c[3]);
    // Half the distance along the target's nose, within 30 m of that point.
    let lead = target.pos + Vec3::Z * 100.0;
    assert!(((c[2] - lead).length() - PURSUIT_OFFSET).abs() < 1e-3);
    let p = xwing(true);
    let mut ai = FlyerAi {
        curve: Some(c),
        t: 1.0,
        pursuit: true,
        ..FlyerAi::default()
    };
    let h = VehicleHandle {
        index: 0,
        gen: mine.gen,
    };
    bf2_plan(&s, me, &mine, h, &p, &mut ai, None);
    assert!(!ai.pursuit);
    assert_eq!(ai.escape, PURSUIT_ESCAPE);
    assert!(ai.cursor.is_some());
    assert_eq!(ai.t, 0.0);
}

#[test]
fn the_connection_prefers_the_end_ahead_and_keeps_the_node_order() {
    let (s, _, _) = bf2_world();
    let mut v = s.vehicles[0].clone();
    v.pos = Vec3::new(0.0, 40.0, 0.0);
    let p = xwing(true);
    let mut ai = FlyerAi::default();
    let h = VehicleHandle {
        index: 0,
        gen: v.gen,
    };
    connect(&s, &v, h, &p, &mut ai, None);
    let c = ai.cursor.unwrap();
    // +Z is ahead: path 0 starts there, and it runs on away from the craft.
    assert_eq!((c.path, c.node, c.reverse), (0, 0, false));
    let curve = ai.curve.unwrap();
    assert_eq!(curve[1], v.pos);
    assert_eq!(curve[2], s.flyer_paths[0].nodes[0]);
    assert_eq!(curve[3], s.flyer_paths[0].nodes[1]);
    assert_eq!(curve[0], v.pos - Vec3::Z * CONNECT_BEHIND);
}

/// `bf1_kas1`'s porpoising: a raised nose turning to a goal behind it at its own
/// height. In the nose's own frame that goal reads above, and the nose kept climbing.
#[test]
fn a_raised_nose_comes_down_whatever_the_goals_bearing() {
    let p = xwing(false);
    let aim_dir = Vec3::new(0.0, 0.4, 0.9165);
    let body = Body {
        pos: Vec3::ZERO,
        left: Vec3::X,
        fwd: Vec3::Z,
        vel: Vec3::Z,
        aim_origin: Vec3::ZERO,
        aim_dir,
    };
    for goal in [
        Vec3::new(0.0, 0.0, 200.0),
        Vec3::new(200.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -200.0),
    ] {
        let (_, pitch) = stick_to(&p, &body, goal);
        assert_eq!(pitch, -1.0, "{goal}");
    }
}

/// Height comes at the steady slope, or the steep one with the ground close.
#[test]
fn the_altitude_law_climbs_no_steeper_than_its_slope() {
    let (s, _, _) = world(false, true);
    let mut v = s.vehicles[0].clone();
    v.vel = Vec3::Z * 50.0;
    let h = VehicleHandle {
        index: 0,
        gen: v.gen,
    };
    let p = xwing(false);
    let goal = Vec3::new(0.0, 500.0, 100.0);
    v.pos = Vec3::new(0.0, 30.0, 0.0);
    let (cruise, _) = lift_goal(&s, &v, h, &p, goal);
    assert!(
        (cruise.y - (30.0 + 100.0 * CLIMB_SLOPE)).abs() < 0.01,
        "{cruise}"
    );
    v.pos = Vec3::new(0.0, GROUND_DANGER - 1.0, 0.0);
    let (low, _) = lift_goal(&s, &v, h, &p, goal);
    assert!(
        (low.y - (GROUND_DANGER - 1.0 + 100.0 * CLIMB_SLOPE_LOW)).abs() < 0.01,
        "{low}"
    );
}

/// The stick moves at the hand's rate, from where it was, whatever the law asks.
#[test]
fn a_bot_pilots_pitch_stick_slews() {
    let (mut s, _, _) = world(false, true);
    s.tick = 10;
    let handle = VehicleHandle {
        index: 0,
        gen: s.vehicles[0].gen,
    };
    let me = UnitHandle {
        index: 0,
        gen: s.units[0].gen,
    };
    s.vehicles[0].flyer.phase = FlightPhase::Flying;
    s.vehicles[0].pos = Vec3::new(0.0, 100.0, 0.0);
    let mut v = s.vehicles[0].clone();
    let p = xwing(false);
    let mut ai = FlyerAi {
        agent: FlyAgent::Travel,
        goal: Some(Vec3::new(0.0, 1000.0, 400.0)),
        anchor: v.pos,
        anchor_at: s.tick,
        ..FlyerAi::default()
    };
    let step = PITCH_SLEW * crate::TICK_DT;
    let [.., first] = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    assert!((first - step).abs() < 1e-5, "{first}");
    v.input.pitch = -1.0;
    let [.., second] = bf1_sticks(&s, me, &v, handle, &p, &mut ai, None);
    assert!((second - (step - 1.0)).abs() < 1e-5, "{second}");
}

/// A sighting holds for [`ATTACK_HOLD`], the run keeps its slope, and a target under
/// the nose ends it instead of diving at it.
#[test]
fn an_attack_run_holds_its_sighting_and_never_dives_at_the_nose() {
    let (mut s, _, _) = world(false, true);
    s.tick = 100;
    let mut v = s.vehicles[0].clone();
    v.pos = Vec3::new(0.0, 100.0, 0.0);
    let mut ai = FlyerAi::default();
    let far = Vec3::new(0.0, 0.0, 200.0);
    let goal = attack_goal(&s, &v, &mut ai, Some(far)).expect("a run");
    assert!(
        (goal.y - (100.0 - 200.0 * ATTACK_SLOPE)).abs() < 0.01,
        "{goal}"
    );
    s.tick += ATTACK_HOLD - 1;
    assert!(attack_goal(&s, &v, &mut ai, None).is_some());
    s.tick += 1;
    assert_eq!(attack_goal(&s, &v, &mut ai, None), None);
    assert_eq!(ai.run, None);

    let under = Vec3::new(1.0, 0.0, 1.0);
    assert_eq!(attack_goal(&s, &v, &mut ai, Some(under)), None);
    assert_eq!(ai.break_until, s.tick + ATTACK_BREAK);
}

/// [`WEDGED_GIVEUP`] ways out and the pilot leaves; one fewer and he stays.
#[test]
fn a_pilot_leaves_a_craft_wedged_too_many_times() {
    for (wedges, leaves) in [(WEDGED_GIVEUP - 1, false), (WEDGED_GIVEUP, true)] {
        let (mut s, b, v) = world(false, true);
        assert!(s.seat_unit(b, v, 0));
        let at = Vec3::new(12.0, 50.0, 0.0);
        let c = s.vehicle_mut(v).unwrap();
        c.pos = at;
        c.flyer.phase = FlightPhase::Flying;
        c.flyer.ai = FlyerAi {
            agent: FlyAgent::Escape,
            goal: Some(at + Vec3::Z * ESCAPE_FORWARD),
            anchor: at,
            wedged_until: 1000,
            wedges,
            ..FlyerAi::default()
        };
        for _ in 0..5 {
            s.step(&[]);
        }
        assert_eq!(
            s.unit(b).unwrap().mount.is_none(),
            leaves,
            "{wedges} ways out"
        );
    }
}
