use super::*;
use crate::cp::CpDef;
use crate::region::Region;
use crate::sim::CtfState;
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
            ..FlyerPath::default()
        },
        FlyerPath {
            nodes: vec![
                Vec3::new(-400.0, 40.0, 0.0),
                Vec3::new(-400.0, 40.0, -400.0),
            ],
            ..FlyerPath::default()
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
fn a_class_no_route_names_gets_no_pilot() {
    let (mut s, b, v) = bf2_world();
    s.flyer_params[0].as_mut().unwrap().follower.class_id = 3;
    for path in &mut s.flyer_paths {
        path.classes = vec![(7, 1)];
    }
    assert_eq!(super::super::ride::claim_seat(&s, b, v), None);
    s.flyer_paths[1].classes.push((3, 1));
    assert_eq!(super::super::ride::claim_seat(&s, b, v), Some(0));
}

fn walled(layers: crate::collision::Layers) -> (Sim, UnitHandle, VehicleHandle) {
    use crate::collision::{Body, CollisionWorld, Shape};
    let (mut s, b, v) = bf2_world();
    let at = Vec3::new(0.0, 40.0, 0.0);
    let c = s.vehicle_mut(v).unwrap();
    c.pos = at;
    c.vel = Vec3::ZERO;
    c.flyer.phase = FlightPhase::Flying;
    let wall = |offset: Vec3, half: Vec3| {
        Body::with_layers(
            glam::Mat4::from_translation(at + offset),
            Shape::Box { half },
            layers,
        )
    };
    let (far, side) = (60.0, 200.0);
    s.collision = CollisionWorld::new(
        vec![
            wall(Vec3::X * far, Vec3::new(5.0, side, side)),
            wall(-Vec3::X * far, Vec3::new(5.0, side, side)),
            wall(Vec3::Z * far, Vec3::new(side, side, 5.0)),
            wall(-Vec3::Z * far, Vec3::new(side, side, 5.0)),
            wall(Vec3::Y * far, Vec3::new(side, 5.0, side)),
            wall(-Vec3::Y * far, Vec3::new(side, 5.0, side)),
        ],
        Vec::new(),
    );
    (s, b, v)
}

#[test]
fn a_wall_refuses_a_route_and_a_vehicle_clear_field_does_not() {
    use crate::collision::Layers;
    let (s, _, v) = walled(Layers::ALL | Layers::FLYER);
    let ai = planned(&s, v);
    assert!(ai.curve.is_none() && ai.cursor.is_none(), "{ai:?}");
    let (s, _, v) = walled(Layers::ALL);
    assert!(planned(&s, v).cursor.is_some(), "the field lets it through");
}

#[test]
fn a_blocked_ctf_hop_keeps_the_authored_route() {
    use crate::collision::{Body, CollisionWorld, Layers, Shape};
    let (mut s, b, v, goal) = hop_world();
    // One wall between the pad and the flag, clear of the authored route.
    s.collision = CollisionWorld::new(
        vec![Body::with_layers(
            glam::Mat4::from_translation(Vec3::new(60.0, 40.0, 0.0)),
            Shape::Box {
                half: Vec3::new(5.0, 200.0, 200.0),
            },
            Layers::ALL | Layers::VEHICLE | Layers::FLYER,
        )],
        Vec::new(),
    );
    s.install_ctf(one_flag(false, goal));
    let mut ai = FlyerAi::default();
    plan(&s, b, v, &mut ai);
    assert_ne!(ai.goal, Some(goal), "the blocked hop was taken");
    assert!(ai.cursor.is_some(), "the authored route plans the way out");

    // Without the wall the same goal is a direct hop.
    s.collision = CollisionWorld::new(Vec::new(), Vec::new());
    let mut ai = FlyerAi::default();
    plan(&s, b, v, &mut ai);
    assert_eq!(ai.goal, Some(goal));
    assert_eq!(ai.curve, Some(ctf_curve(s.vehicle(v).unwrap(), goal)));
}

/// A craft flying at (0, 40, 0) and a flag 400 m east of it.
fn hop_world() -> (Sim, UnitHandle, VehicleHandle, Vec3) {
    let (mut s, b, v) = bf2_world();
    let c = s.vehicle_mut(v).unwrap();
    c.pos = Vec3::new(0.0, 40.0, 0.0);
    c.vel = Vec3::ZERO;
    c.flyer.phase = FlightPhase::Flying;
    (s, b, v, Vec3::new(400.0, 40.0, 0.0))
}

fn one_flag(space: bool, flag: Vec3) -> CtfState {
    if space {
        CtfState::new_space_one_flag(5, flag, Vec3::ZERO, Vec3::ZERO)
    } else {
        CtfState::new_ground_one_flag(5, flag, Vec3::ZERO, Vec3::ZERO)
    }
}

fn plan(s: &Sim, b: UnitHandle, v: VehicleHandle, ai: &mut FlyerAi) {
    let c = s.vehicle(v).unwrap();
    let p = s.flyer_params[c.class as usize].clone().unwrap();
    bf2_plan(s, b, c, v, &p, ai, None);
}

#[test]
fn an_authored_transport_keeps_its_route_when_its_hull_would_graze_a_wall() {
    use crate::collision::{Body, CollisionWorld, HullPart, Layers, Shape};

    let (mut sim, _, handle) = bf2_world();
    sim.space_assault = Some(crate::sim::SpaceAssault::new(180));
    sim.flyer_paths[0].classes.push((TRANSPORT_CLASS, 1));
    sim.flyer_params[0].as_mut().unwrap().follower.class_id = TRANSPORT_CLASS;
    sim.hull_parts = vec![Some(std::sync::Arc::from(vec![HullPart::new(
        glam::Mat4::IDENTITY,
        Shape::Sphere { radius: 5.0 },
        Layers::VEHICLE,
    )]))];
    sim.collision = CollisionWorld::new(
        vec![Body::with_layers(
            glam::Mat4::from_translation(Vec3::new(4.5, 30.0, 50.0)),
            Shape::Box {
                half: Vec3::new(1.5, 5.0, 20.0),
            },
            Layers::FLYER,
        )],
        Vec::new(),
    );
    let start = Vec3::new(0.0, 30.0, 0.0);
    let leg = [
        start - Vec3::Z * 50.0,
        start,
        start + Vec3::Z * 100.0,
        start + Vec3::Z * 150.0,
    ];
    assert!(curve_clear(&sim, &leg, handle));
    let craft = sim.vehicle(handle).unwrap();
    assert!(!transport_curve_clear(&sim, craft, &leg));
    sim.flyer_params[0].as_mut().unwrap().follower.class_id = 0;
    assert!(transport_curve_clear(
        &sim,
        sim.vehicle(handle).unwrap(),
        &leg
    ));
}

#[test]
fn a_space_flag_draws_no_pilot() {
    let (mut s, b, v, goal) = hop_world();
    s.install_ctf(one_flag(true, goal));
    let mut ai = FlyerAi::default();
    plan(&s, b, v, &mut ai);
    assert_ne!(ai.goal, Some(goal), "the flag was chased");
    assert!(ai.cursor.is_some(), "the authored route is flown");
}

#[test]
fn a_ctf_hop_the_hull_does_not_fit_is_refused() {
    use crate::collision::{Body, CollisionWorld, HullPart, Layers, Shape};
    let (mut s, b, v, goal) = hop_world();
    s.install_ctf(one_flag(false, goal));
    s.collision = CollisionWorld::new(
        vec![Body::with_layers(
            glam::Mat4::from_translation(Vec3::new(200.0, 45.0, 0.0)),
            Shape::Box {
                half: Vec3::new(150.0, 0.5, 100.0),
            },
            Layers::ALL | Layers::VEHICLE | Layers::FLYER,
        )],
        Vec::new(),
    );
    let mut ai = FlyerAi::default();
    plan(&s, b, v, &mut ai);
    assert_eq!(ai.goal, Some(goal), "a point craft passes under the slab");
    s.hull_parts = vec![Some(std::sync::Arc::from(vec![HullPart::new(
        glam::Mat4::IDENTITY,
        Shape::Sphere { radius: 5.0 },
        Layers::VEHICLE,
    )
    .with_flyer_subsets(true, true)]))];
    let mut ai = FlyerAi::default();
    plan(&s, b, v, &mut ai);
    assert_ne!(ai.goal, Some(goal), "a 5 m hull grazes the slab");
}

#[test]
fn a_route_segment_begun_is_flown_before_a_ctf_hop() {
    let (mut s, b, v, goal) = hop_world();
    s.install_ctf(one_flag(true, goal));
    let mut ai = FlyerAi::default();
    plan(&s, b, v, &mut ai);
    let route = (ai.curve, ai.cursor);
    assert!(route.1.is_some());
    s.install_ctf(one_flag(false, goal));
    ai.t = 0.5;
    plan(&s, b, v, &mut ai);
    assert_eq!((ai.curve, ai.cursor), route, "the segment was cut");
    ai.t = 1.0;
    plan(&s, b, v, &mut ai);
    assert_eq!(ai.goal, Some(goal), "the hop follows the segment's end");
}

#[test]
fn a_pilot_with_no_clear_way_stays_inside_the_walls() {
    use crate::collision::{HullPart, Layers, Shape};
    let (mut s, b, v) = walled(Layers::ALL | Layers::VEHICLE | Layers::FLYER);
    s.hull_parts = vec![Some(std::sync::Arc::from(vec![HullPart::new(
        glam::Mat4::IDENTITY,
        Shape::Sphere { radius: 2.0 },
        Layers::VEHICLE,
    )
    .with_flyer_subsets(true, true)]))];
    assert!(s.seat_unit(b, v, 0));
    for _ in 0..(20 * crate::TICK_HZ) {
        s.step(&[]);
        let Some(c) = s.vehicle(v).filter(|c| c.alive) else {
            return;
        };
        let from = (c.pos - Vec3::new(0.0, 40.0, 0.0)).abs().max_element();
        assert!(
            from < 55.0,
            "through the wall at tick {}: {:?}",
            s.tick,
            c.pos
        );
    }
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

fn admission_cursor(reverse: bool) -> PathCursor {
    PathCursor {
        path: 0,
        node: if reverse { 3 } else { 0 },
        reverse,
    }
}

fn planned(s: &Sim, h: VehicleHandle) -> FlyerAi {
    let v = s.vehicle(h).unwrap();
    let p = s.flyer_params[v.class as usize].as_ref().unwrap();
    let mut ai = FlyerAi::default();
    connect(s, v, h, p, &mut ai, None);
    ai
}

#[test]
fn flight_admission_refuses_wrong_class_disabled_and_full_routes() {
    let (mut s, _, v) = bf2_world();
    s.flyer_paths.truncate(1);
    s.flyer_params[0].as_mut().unwrap().follower.class_id = 7;
    assert!(planned(&s, v).cursor.is_some(), "ordinary path control");
    s.flyer_paths[0].classes = vec![(8, 1)];
    assert!(planned(&s, v).cursor.is_none(), "wrong class");
    s.flyer_paths[0].classes = vec![(7, 1)];
    assert!(planned(&s, v).cursor.is_some(), "matching class control");
    s.flyer_paths[0].max_entities = 0;
    assert!(planned(&s, v).cursor.is_none(), "total capacity zero");
    s.flyer_paths[0].max_entities = u32::MAX;
    s.flyer_paths[0].classes[0].1 = 0;
    assert!(planned(&s, v).cursor.is_none(), "class capacity zero");
    s.flyer_paths[0].classes.clear();
    let before = s.hash();
    s.enable_flyer_path(0, false);
    assert_ne!(before, s.hash(), "runtime toggle is sim state");
    assert!(planned(&s, v).cursor.is_none(), "disabled");
    s.enable_flyer_path(0, true);
    assert_eq!(before, s.hash());
    assert!(planned(&s, v).cursor.is_some());
}

#[test]
fn flight_admission_checks_both_forced_directions_and_frequency_ranking() {
    let (mut s, _, v) = bf2_world();
    s.flyer_paths.truncate(1);
    s.flyer_paths[0].direction = 1;
    assert!(!planned(&s, v).cursor.unwrap().reverse);
    s.flyer_paths[0].direction = -1;
    assert!(planned(&s, v).cursor.unwrap().reverse);
    s.flyer_paths[0].direction = 0;
    let mut other = s.flyer_paths[0].clone();
    other.nodes.iter_mut().for_each(|p| p.x += 1.0);
    s.flyer_paths.push(other);
    s.flyer_paths[0].frequency = 0.001;
    assert_eq!(planned(&s, v).cursor.unwrap().path, 1);
    s.flyer_paths[0].frequency = 1.0;
    s.flyer_paths[1].frequency = 0.001;
    assert_eq!(planned(&s, v).cursor.unwrap().path, 0);
    s.flyer_paths.truncate(1);
    s.flyer_paths[0].frequency = 0.0;
    assert!(
        planned(&s, v).cursor.is_some(),
        "PS2 permits a geometric fallback at zero Frequency"
    );
}

fn two_pilots() -> (Sim, [UnitHandle; 2], [VehicleHandle; 2]) {
    let (mut s, a, va) = bf2_world();
    s.flyer_paths.truncate(1);
    let b = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let vb = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(30.0, 40.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    for (u, v) in [(a, va), (b, vb)] {
        assert!(s.seat_unit(u, v, 0));
        s.vehicle_mut(v).unwrap().flyer.phase = FlightPhase::Flying;
    }
    (s, [a, b], [va, vb])
}

#[test]
fn flight_admission_reserves_atomically_and_releases_on_lifecycle_changes() {
    let (mut s, [a, b], [va, vb]) = two_pilots();
    s.flyer_paths[0].max_entities = 1;
    // Both immutable planners saw the one remaining slot in this tick.
    let first = planned(&s, va);
    let second = planned(&s, vb);
    assert!(first.cursor.is_some() && second.cursor.is_some());
    s.vehicle_mut(va).unwrap().flyer.ai = admission::commit(&s, va, first);
    let loser = admission::commit(&s, vb, second);
    assert!(loser.cursor.is_none());
    assert!(
        loser.curve.is_some(),
        "a same-tick join loser needs fallback control"
    );
    assert!(planned(&s, vb).cursor.is_none(), "total capacity occupied");
    s.vehicle_mut(va).unwrap().flyer.phase = FlightPhase::Landing;
    assert!(
        planned(&s, vb).cursor.is_none(),
        "a flight-state change alone does not detach the follower"
    );
    s.vehicle_mut(va).unwrap().flyer.phase = FlightPhase::Flying;
    s.flyer_paths[0].max_entities = 2;
    s.flyer_paths[0].classes = vec![(7, 1), (8, 1)];
    s.flyer_params[0].as_mut().unwrap().follower.class_id = 7;
    assert!(planned(&s, vb).cursor.is_none(), "class capacity occupied");
    let mut p = s.flyer_params[0].clone().unwrap();
    p.follower.class_id = 8;
    s.flyer_params.push(Some(p));
    s.vehicle_mut(vb).unwrap().class = 1;
    assert!(
        planned(&s, vb).cursor.is_some(),
        "other class still has capacity"
    );
    s.flyer_paths[0].classes.clear();
    s.flyer_paths[0].max_entities = 1;
    // Pursuit owns a curve, but no authored route.
    s.vehicle_mut(va).unwrap().flyer.ai.cursor = None;
    s.vehicle_mut(va).unwrap().flyer.ai.pursuit = true;
    assert!(planned(&s, vb).cursor.is_some());
    s.vehicle_mut(va).unwrap().flyer.ai = first;
    s.unseat_unit(a);
    assert!(s.vehicle(va).unwrap().flyer.ai.cursor.is_none());
    assert!(planned(&s, vb).cursor.is_some());
    assert!(s.seat_unit(a, va, 0));
    assert!(
        s.vehicle(va).unwrap().flyer.ai.cursor.is_none(),
        "same-tick reentry has no old reservation"
    );
    s.vehicle_mut(va).unwrap().flyer.ai = first;
    s.unit_mut(a).unwrap().alive = false;
    assert!(
        planned(&s, vb).cursor.is_some(),
        "dead pilot releases before cleanup"
    );
    s.units[a.index as usize].alive = true;
    s.unit_mut(a).unwrap().controller = Controller::Player { slot: 0 };
    assert!(planned(&s, vb).cursor.is_some(), "human takeover releases");
    s.unit_mut(a).unwrap().controller = Controller::Ai;
    s.vehicle_mut(va).unwrap().alive = false;
    assert!(planned(&s, vb).cursor.is_some(), "dead vehicle releases");
    assert!(s.unit(b).unwrap().alive);
}

#[test]
fn flight_single_direction_unlocks_only_after_the_last_user_leaves() {
    let (mut s, [a, b], [va, vb]) = two_pilots();
    s.flyer_paths[0].single_direction = true;
    for v in [va, vb] {
        s.vehicle_mut(v).unwrap().flyer.ai.cursor = Some(admission_cursor(true));
    }
    assert!(admission::allows(&s, vb, 0, admission_cursor(true), true));
    assert!(!admission::allows(&s, vb, 0, admission_cursor(false), true));
    s.unseat_unit(a);
    // The remaining pilot can reverse only after releasing its own tenure;
    // a third joining pilot still observes the second user's direction.
    let vh = VehicleHandle {
        index: u32::MAX,
        gen: 0,
    };
    assert!(!admission::allows(&s, vh, 0, admission_cursor(false), true));
    s.unseat_unit(b);
    assert!(admission::allows(&s, vh, 0, admission_cursor(false), true));
}

#[test]
fn flight_discovery_and_join_keep_the_ps2_unspecified_class_distinction() {
    let (mut s, _, v) = bf2_world();
    s.flyer_paths.truncate(1);
    s.flyer_paths[0].classes = vec![(7, 2)];
    assert!(admission::allows(&s, v, 0, admission_cursor(false), false));
    let candidate = planned(&s, v);
    assert!(
        candidate.cursor.is_none(),
        "selector must require a joinable choice"
    );
    assert!(
        admission::commit(&s, v, candidate).cursor.is_none(),
        "joining needs a matching class record"
    );
    s.flyer_paths[0].classes = vec![(0, 2)];
    assert!(
        planned(&s, v).cursor.is_some(),
        "explicit zero-class record can join"
    );
}

#[test]
fn flight_enable_object_follows_live_station_and_missing_object_defaults_on() {
    let (mut s, _, v) = bf2_world();
    s.flyer_paths.truncate(1);
    s.flyer_paths[0].enable_instance = Some(12);
    assert!(
        planned(&s, v).cursor.is_some(),
        "unresolved object stays enabled"
    );
    let mut station = crate::sim::Station::new(0, Vec3::ZERO, 1.0);
    station.instance = Some(12);
    s.stations.push(station);
    assert!(planned(&s, v).cursor.is_some());
    s.stations[0].destroyed = true;
    assert!(planned(&s, v).cursor.is_none());
    s.stations[0].destroyed = false;
    assert!(
        planned(&s, v).cursor.is_some(),
        "repair reenables admission"
    );
}

#[test]
fn flight_capacity_holds_through_the_real_step_and_replays() {
    fn run() -> Vec<u64> {
        let (mut s, pilots, vehicles) = two_pilots();
        s.flyer_paths[0].max_entities = 1;
        for u in pilots {
            s.units[u.index as usize].ai.ride = None;
        }
        for v in vehicles {
            s.vehicle_mut(v).unwrap().pos.y = 40.0;
        }
        let mut hashes = Vec::new();
        let mut admitted = false;
        for _ in 0..180 {
            s.step(&[]);
            let users = vehicles
                .iter()
                .filter_map(|&h| s.vehicle(h))
                .filter(|v| v.flyer.ai.cursor.is_some())
                .count();
            assert!(users <= 1, "tick {} overbooked {users}", s.tick);
            admitted |= users == 1;
            hashes.push(s.hash());
        }
        assert!(admitted, "normal route must actually be used");
        hashes
    }
    assert_eq!(run(), run());
}

#[test]
fn flight_all_routes_refused_loiters_without_stalling_or_reserving() {
    for refusal in 0..3 {
        let (mut s, b, v) = bf2_world();
        for path in &mut s.flyer_paths {
            match refusal {
                0 => path.enabled = false,
                1 => path.classes = vec![(7, 1)],
                _ => path.max_entities = 0,
            }
        }
        assert!(s.seat_unit(b, v, 0));
        let start = Vec3::new(0.0, 40.0, 0.0);
        let c = s.vehicle_mut(v).unwrap();
        c.pos = start;
        c.vel = Vec3::Z * 50.0;
        c.flyer.phase = FlightPhase::Flying;
        let mut travelled = 0.0;
        let mut previous = start;
        let mut window_travel = 0.0;
        for _ in 0..3600 {
            s.step(&[]);
            let c = s.vehicle(v).expect("fallback destroyed the craft");
            assert_eq!(s.unit(b).unwrap().mount.map(|m| m.vehicle), Some(v));
            assert!(c.flyer.ai.cursor.is_none(), "refused route was reserved");
            assert!(
                c.flyer.ai.curve.is_some(),
                "uncontrolled flight at {}",
                s.tick
            );
            assert!((c.pos - start).length() < 400.0, "flew away: {:?}", c.pos);
            window_travel += (c.pos - previous).length();
            if s.tick.is_multiple_of(120) {
                assert!(window_travel > 50.0, "stalled at {}", s.tick);
                window_travel = 0.0;
            }
            travelled += (c.pos - previous).length();
            previous = c.pos;
        }
        assert!(travelled > 1000.0, "fallback made no sustained progress");
        s.flyer_paths[1].enabled = true;
        s.flyer_paths[1].classes.clear();
        s.flyer_paths[1].max_entities = u32::MAX;
        for _ in 0..600 {
            s.step(&[]);
            if s.vehicle(v).unwrap().flyer.ai.cursor.is_some() {
                break;
            }
        }
        assert!(
            s.vehicle(v).unwrap().flyer.ai.cursor.is_some(),
            "never retried admission"
        );
    }
}

#[test]
fn flight_zero_class_selects_a_joinable_route_through_step() {
    let (mut s, b, v) = bf2_world();
    s.flyer_paths[0].classes = vec![(7, 1)];
    // The geometrically preferred first path passes discovery but cannot join.
    assert!(admission::allows(&s, v, 0, admission_cursor(false), false));
    assert!(!admission::allows(&s, v, 0, admission_cursor(false), true));
    assert!(s.seat_unit(b, v, 0));
    let c = s.vehicle_mut(v).unwrap();
    c.pos = Vec3::new(0.0, 40.0, 0.0);
    c.flyer.phase = FlightPhase::Flying;
    s.step(&[]);
    assert_eq!(s.vehicle(v).unwrap().flyer.ai.cursor.unwrap().path, 1);
    for _ in 0..600 {
        s.step(&[]);
    }
    assert!(s.vehicle(v).unwrap().flyer.ai.curve.is_some());
}

#[test]
fn flight_zero_class_discovery_skips_capacity_but_join_does_not() {
    let (mut s, _, v) = bf2_world();
    s.flyer_paths[0].max_entities = 0;
    let c = admission_cursor(false);
    assert!(admission::allows(&s, v, 0, c, false));
    assert!(!admission::allows(&s, v, 0, c, true));
    assert!(!admission::allows(&s, v, 7, c, false));
}

#[test]
fn flight_pursuit_discards_the_loiter_center_before_recovery() {
    let (mut s, me, vh) = bf2_world();
    for path in &mut s.flyer_paths {
        path.enabled = false;
    }
    let v = s.vehicle(vh).unwrap();
    let mut p = s.flyer_params[0].clone().unwrap();
    p.follower.flyer_vs_flyer = true;
    let mut ai = planned(&s, vh);
    assert!(ai.goal.is_some());
    let mut target = v.clone();
    target.pos += Vec3::Z * 200.0;
    bf2_plan(&s, me, v, vh, &p, &mut ai, Some(&target));
    assert!(ai.pursuit);
    assert!(ai.goal.is_none());
    ai.t = 1.0;
    bf2_plan(&s, me, v, vh, &p, &mut ai, None);
    assert!(!ai.pursuit);
    assert!(ai.goal.is_some());
    assert_eq!(ai.curve.unwrap()[1], v.pos);
    assert!(ai.cursor.is_none());
}

fn human_in_flyer(s: &mut Sim, at: Vec3, team: u8) -> (UnitHandle, VehicleHandle) {
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: at,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let h = s.spawn(&UnitSpec::player(team, at)).unwrap();
    assert!(s.seat_unit(h, v, 0));
    s.vehicle_mut(v).unwrap().flyer.phase = FlightPhase::Flying;
    s.players = vec![Some(h)];
    (h, v)
}

fn seeker_of(me: UnitHandle, wanted: bool) -> UnitHandle {
    (0..64)
        .map(|gen| UnitHandle { gen, ..me })
        .find(|h| crate::sim::flyer_seek::seeks_human(*h) == wanted)
        .unwrap()
}

#[test]
fn about_half_of_the_bots_seek_the_human() {
    let seekers = (0..4000)
        .filter(|&index| crate::sim::flyer_seek::seeks_human(UnitHandle { index, gen: 0 }))
        .count();
    assert!((1700..2100).contains(&seekers), "{seekers} of 4000");
}

#[test]
fn a_seeking_bot_ending_its_route_flies_150_m_at_a_far_human() {
    let (mut s, me, vh) = bf2_world();
    human_in_flyer(&mut s, Vec3::new(12.0, 1.0, 1000.0), 1);
    let v = s.vehicle(vh).unwrap();
    let p = s.flyer_params[0].clone().unwrap();
    let mut ai = FlyerAi::default();
    assert!(s.flyer_seek_curve(seeker_of(me, true), v, vh, &p, &mut ai));
    let curve = ai.curve.unwrap();
    assert_eq!(curve[1], v.pos);
    assert!((curve[2] - (v.pos + Vec3::Z * 150.0)).length() < 1e-2);
    assert!((curve[3] - (v.pos + Vec3::Z * 350.0)).length() < 1e-2);
    assert!(ai.cursor.is_none());
    let mut other = FlyerAi::default();
    assert!(!s.flyer_seek_curve(seeker_of(me, false), v, vh, &p, &mut other));
    assert!(other.curve.is_none());
}

#[test]
fn a_seeking_bot_within_400_m_of_the_human_keeps_to_the_routes() {
    let (mut s, me, vh) = bf2_world();
    human_in_flyer(&mut s, Vec3::new(12.0, 1.0, 390.0), 1);
    let v = s.vehicle(vh).unwrap();
    let p = s.flyer_params[0].clone().unwrap();
    let mut ai = FlyerAi::default();
    assert!(!s.flyer_seek_curve(seeker_of(me, true), v, vh, &p, &mut ai));
}

fn relocation_world(
    formations: u8,
    after_battle_start: bool,
) -> (Sim, UnitHandle, VehicleHandle, Vec3) {
    let (mut s, _, vh) = bf2_world();
    s.flyer_params[0].as_mut().unwrap().follower.formations = formations;
    let (_, human) = human_in_flyer(&mut s, Vec3::new(0.0, 200.0, 0.0), 1);
    let human_pos = s.vehicle(human).unwrap().pos;
    let bot = (0..8)
        .find_map(|n| {
            let u = s
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(0, Vec3::new(n as f32, 0.0, 0.0))
                })
                .unwrap();
            crate::sim::flyer_seek::seeks_human(u).then_some(u)
        })
        .expect("a seeker");
    assert!(s.seat_unit(bot, vh, 0));
    let v = s.vehicle_mut(vh).unwrap();
    v.flyer.phase = FlightPhase::Flying;
    v.pos = human_pos - Vec3::Z * 1500.0;
    s.tick = if after_battle_start {
        31 * u64::from(crate::TICK_HZ)
    } else {
        10 * u64::from(crate::TICK_HZ)
    };
    (s, bot, vh, human_pos)
}

#[test]
fn an_enemy_behind_the_human_is_relocated_ahead_of_the_human() {
    let (mut s, _, vh, human_pos) = relocation_world(1, true);
    s.tick_flyer_seek(crate::TICK_DT);
    let v = s.vehicle(vh).unwrap();
    let ahead = (v.pos - human_pos).dot(Vec3::Z);
    let side = (v.pos - human_pos - Vec3::Z * ahead).length();
    assert!(
        (ahead - 85.0).abs() < 1e-2 || (ahead - 700.0).abs() < 1e-2,
        "ahead {ahead}"
    );
    assert!((side - 85.0).abs() < 1e-2, "side {side}");
    let heading = v.rot * Vec3::Z;
    if ahead < 100.0 {
        assert!(heading.dot(Vec3::Z) > 0.99);
    } else {
        assert!(heading.dot(Vec3::Z) < -0.99);
    }
    assert!(v.flyer.ai.relocate_in >= 5.5 && v.flyer.ai.relocate_in <= 8.5);
}

#[test]
fn relocation_waits_for_30_seconds_and_needs_a_formation() {
    let (mut early, _, vh, human) = relocation_world(1, false);
    early.tick_flyer_seek(crate::TICK_DT);
    assert!((early.vehicle(vh).unwrap().pos - human).length() > 1000.0);
    let (mut none, _, vh, human) = relocation_world(0, true);
    none.tick_flyer_seek(crate::TICK_DT);
    assert!((none.vehicle(vh).unwrap().pos - human).length() > 1000.0);
}

#[test]
fn a_craft_on_the_humans_marker_list_is_not_relocated() {
    let (mut s, _, vh, human) = relocation_world(1, true);
    s.vehicle_mut(vh).unwrap().pos = human + Vec3::Z * 1500.0;
    let before = s.vehicle(vh).unwrap().pos;
    s.flyer_params[0].as_mut().unwrap().follower.formations = 1;
    let listed = s.target_hint_for_test(vh);
    assert!(listed);
    s.tick_flyer_seek(crate::TICK_DT);
    assert_eq!(s.vehicle(vh).unwrap().pos, before);
}

#[test]
fn a_free_burst_rises_late_in_its_timer_and_holds_to_the_next_think() {
    let (mut s, b, _) = bf2_world();
    let mut p = xwing(true);
    p.follower.flyer_vs_flyer = true;
    let mut ai = FlyerAi::default();
    let mut rises = Vec::new();
    let mut was = false;
    for t in 0..(120.0 / crate::TICK_DT) as u64 {
        s.tick = t;
        let on = free_burst(&s, b, &p, &mut ai);
        if on && !was {
            rises.push(t);
        }
        if was && !on {
            let grid = t + u64::from(b.index) * FREE_BURST_THINK_STEP;
            assert_eq!(grid % FREE_BURST_THINK, 0, "released off the grid at {t}");
        }
        was = on;
    }
    let first = rises[0] as f32 * crate::TICK_DT;
    assert!((first - 9.4).abs() < 0.1, "first burst at {first}");
    assert!(rises.len() >= 16, "{rises:?}");
    for pair in rises.windows(2) {
        let gap = (pair[1] - pair[0]) as f32 * crate::TICK_DT;
        assert!((5.2..=6.8).contains(&gap), "bursts {gap} s apart");
    }
}

#[test]
fn a_class_without_flyer_vs_flyer_takes_no_free_burst() {
    let (mut s, b, _) = bf2_world();
    let mut p = xwing(true);
    p.follower.flyer_vs_flyer = false;
    let mut ai = FlyerAi::default();
    for t in 0..(30.0 / crate::TICK_DT) as u64 {
        s.tick = t;
        assert!(!free_burst(&s, b, &p, &mut ai));
    }
    assert_eq!(ai.burst_in, 0.0);
}

#[test]
fn a_transport_pilot_requests_landing_on_a_fresh_press_inside_the_region() {
    let (mut s, b, v) = world(true, true);
    s.flyer_params[0].as_mut().unwrap().follower.class_id = admission::TRANSPORT_CLASS;
    let center = s.vehicle(v).unwrap().pos;
    s.space_assault = Some(crate::sim::SpaceAssault::new(180));
    s.flyer_paths.push(FlyerPath {
        classes: vec![(TRANSPORT_CLASS, 1)],
        ..FlyerPath::default()
    });
    s.landing_regions.push(Region::sphere(center, 20.0));
    s.blast_setup.push(Some(crate::vehicle::BlastSetup {
        explosion: None,
        sphere_centre: Vec3::ZERO,
        sphere_radius: 5.0,
    }));
    assert!(s.enter_vehicle(b, v, 0));
    let mount = s.unit(b).unwrap().mount.unwrap();
    for (offset, held, requests) in [
        (18.0, false, false),
        (10.0, true, false),
        (10.0, false, true),
    ] {
        let craft = s.vehicle_mut(v).unwrap();
        craft.pos = center + Vec3::X * offset;
        craft.flyer.phase = FlightPhase::Flying;
        craft.flyer.jump_was_down = held;
        let mut state = AiState::default();
        drive(&s, b, mount, &mut state, NavTick::default());
        assert_eq!(state.ride.unwrap().flyer.unwrap().takeoff, requests);
    }
}

fn transport_world() -> (Sim, UnitHandle, UnitHandle, UnitHandle, VehicleHandle) {
    let (mut s, pilot, spare) = world(true, true);
    s.space_assault = Some(crate::sim::SpaceAssault::new(180));
    s.flyer_params[0].as_mut().unwrap().follower.class_id = TRANSPORT_CLASS;
    s.seat_setup[0].as_mut().unwrap().boardable = vec![true; 3];
    let hangar = Vec3::new(0.0, 5.0, 200.0);
    post(&mut s, "enemy hangar", hangar, Some(1));
    post(&mut s, "home hangar", Vec3::ZERO, Some(0));
    s.landing_regions = vec![
        Region::sphere(Vec3::ZERO, 35.0),
        Region::sphere(hangar, 35.0),
    ];
    s.flyer_paths.push(FlyerPath {
        nodes: vec![Vec3::new(0.0, 6.0, 60.0), hangar],
        classes: vec![(TRANSPORT_CLASS, 1)],
        ..FlyerPath::default()
    });
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 5.0, 105.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![
            SeatKind::Pilot,
            SeatKind::Passenger(0),
            SeatKind::Passenger(1),
        ],
    });
    s.vehicle_mut(spare).unwrap().pos = Vec3::X * 900.0;
    assert!(s.seat_unit(pilot, v, 0));
    let cargo = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    assert_eq!(crate::ai::ride::claim_seat(&s, cargo, v), Some(1));
    assert!(s.seat_unit(cargo, v, 1));
    let human = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    assert!(s.seat_unit(human, v, 2));
    {
        let c = s.vehicle_mut(v).unwrap();
        c.flyer.initialized = true;
        c.flyer.phase = FlightPhase::Flying;
        c.flyer.lift = 1.0;
    }
    (s, pilot, cargo, human, v)
}

#[test]
fn transport_cargo_waits_in_friendly_hangars_and_with_a_human_pilot() {
    for (human_driving, region) in [(false, 0), (true, 1)] {
        let (mut guard, pilot, cargo, human, v) = transport_world();
        let pos = guard.landing_regions[region].center;
        let craft = guard.vehicle_mut(v).unwrap();
        craft.pos = pos;
        craft.flyer.phase = FlightPhase::Landed;
        if human_driving {
            guard.unseat_unit(pilot);
            guard.unseat_unit(human);
            assert!(guard.seat_unit(human, v, 0));
        }
        guard.step(&[]);
        assert!(
            guard.unit(cargo).unwrap().mount.is_some(),
            "cargo left with human_driving={human_driving} at region {region}"
        );
    }
}

#[test]
fn a_space_transport_lands_while_following_a_curve_and_unloads_only_ai_cargo() {
    let (mut s, pilot, cargo, human, v) = transport_world();
    let mut landing = false;
    let mut unloaded = false;
    for _ in 0..20 * crate::TICK_HZ {
        s.step(&[]);
        let c = s.vehicle(v).expect("transport survived");
        landing |= c.flyer.phase == FlightPhase::Landing;
        if s.unit(cargo).unwrap().mount.is_none() {
            assert_eq!(c.flyer.phase, FlightPhase::Landed);
            assert!(s.landing_regions[1].contains(c.pos));
            assert!(s.unit(cargo).unwrap().soldier.grounded);
            assert_eq!(c.seats[0].occupant, Some(pilot));
            assert_eq!(c.seats[2].occupant, Some(human));
            unloaded = true;
            break;
        }
    }
    assert!(
        landing && unloaded,
        "phase {:?}, position {:?}",
        s.vehicle(v).unwrap().flyer.phase,
        s.vehicle(v).unwrap().pos
    );
}
