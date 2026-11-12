//! The walker against re-walker §14.6's AT-ST numbers (dt = 1/30), on a flat
//! ground at y = 0 with no buildings.

use super::*;
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::HealthType;

const DT: f32 = 1.0 / 30.0;

/// The flat ground: y = 0 everywhere, a terrain that no probe reaches.
struct Flat;

impl WalkerWorld for Flat {
    fn has_terrain(&self) -> bool {
        true
    }
    fn ground_below(&self, o: Vec3, len: f32) -> Option<f32> {
        (0.0..=len).contains(&o.y).then_some(o.y)
    }
    fn terrain_segment(&self, from: Vec3, to: Vec3) -> Option<f32> {
        (to.y < 0.0 && from.y >= 0.0).then(|| from.y / (from.y - to.y))
    }
    fn building_contacts(&self, _: Vec3, _: f32, _: &mut Vec<Contact>) {}
}

/// A clip of `frames` frames whose root runs linearly to `z`.
fn clip(frames: usize, z: f32) -> Option<Clip> {
    Some(Clip {
        duration: (frames - 1) as f32 / 30.0,
        looping: false,
        root_z: (0..frames)
            .map(|i| z * i as f32 / (frames - 1) as f32)
            .collect(),
    })
}

/// BF2's imp_walk_atst (re-walker §11's census, through the setter's
/// conversions) with its bank's walk (21 frames, 4.5376 m: 6.8064 m/s) and
/// the transition clips at their §5 speeds.
pub(crate) fn atst() -> WalkerParams {
    let tan = |d: f32| d.to_radians().tan();
    let mut clips = vec![None; ANIM_COUNT];
    clips[0] = clip(21, 0.0);
    clips[1] = clip(21, 0.0);
    clips[2] = clip(21, 0.0);
    clips[5] = clip(21, 3.059 * 20.0 / 30.0);
    clips[6] = clip(21, 3.059 * 20.0 / 30.0);
    clips[7] = clip(21, 4.5376);
    clips[8] = clip(21, 4.5376);
    clips[9] = clip(21, 3.077 * 20.0 / 30.0);
    clips[10] = clip(21, 3.077 * 20.0 / 30.0);
    WalkerParams {
        max_speed: 5.0,
        boost_speed: 10.0,
        stopped_turn_speed: 0.5,
        forward_turn_speed: 0.5,
        boost_turn_speed: 0.3,
        max_turn_speed: 1.5,
        turn_threshold: (0.6f32 * 0.5).sin(),
        yaw_limits: [-90f32.to_radians(), 90f32.to_radians()],
        pitch_limits: [-35f32.to_radians(), 40f32.to_radians()],
        max_yaw_speed: 0.7,
        max_pitch_speed: 0.4,
        e08: tan(87.0),
        e0c: tan(88.0),
        e10: tan(89.0),
        e14: 0.0,
        flags: flags::AI | flags::AIM_WINDOW | flags::CAMERA_TURN | flags::REVERSE_STEER,
        energy_max: 80.0,
        energy_auto_restore: 9.0,
        energy_boost_drain: 20.0,
        root_scale: 1.0,
        leg_ray: 5.0,
        roll: false,
        width: 1.0,
        leg_pairs: vec![LegPair {
            feet: [Vec3::new(1.5, 0.0, 0.3), Vec3::new(-1.5, 0.0, 0.3)],
        }],
        clips,
        building: Vec::new(),
        spawn_points: Vec::new(),
        spawn_sphere_centre: None,
    }
}

fn walker(p: &WalkerParams) -> Vehicle {
    let mut v = VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 8000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
    .build(1);
    v.walker = WalkerState::new(p);
    v
}

fn drive(v: &mut Vehicle, throttle: f32, strafe: f32, turn: f32, boost: bool) {
    v.input.throttle = throttle;
    v.input.strafe = strafe;
    v.input.turn = turn;
    v.input.human = true;
    v.input.boost = boost;
}

fn yaw(v: &Vehicle) -> f32 {
    let f = v.rot * Vec3::Z;
    fmath::atan2(f.x, f.z)
}

#[test]
fn a_clip_samples_between_keyframes_and_keeps_its_speed() {
    let c = clip(21, 4.5376).unwrap();
    assert!((c.speed() - 6.8064).abs() < 1e-3);
    assert!((c.z_at(c.duration * 0.5) - 4.5376 * 0.5).abs() < 1e-5);
    assert_eq!(c.z_at(-1.0), 0.0);
    assert!((c.z_at(10.0) - 4.5376).abs() < 1e-6);
}

#[test]
fn full_stick_walks_the_clip_at_0_7346_and_covers_5_m_s() {
    let p = atst();
    let mut v = walker(&p);
    drive(&mut v, 1.0, 0.0, 0.0, false);
    let mut seen = Vec::new();
    for _ in 0..90 {
        step(&mut v, &p, &Flat, DT);
        seen.push(v.walker.anim);
    }
    // idle → idle>L → the walk, alternating feet.
    assert_eq!(seen[0], 5, "the first step");
    assert!(seen.contains(&7) && seen.contains(&8), "{seen:?}");
    // In a walk clip: the clock and the speed.
    let (a, t0, z0) = (v.walker.anim, v.walker.clip_time, v.pos.z);
    step(&mut v, &p, &Flat, DT);
    if v.walker.anim == a {
        let k = (v.walker.clip_time - t0) / DT;
        assert!((k - 5.0 / 6.8064).abs() < 1e-3, "clock ×{k}");
    }
    let _ = z0;
    let z0 = v.pos.z;
    for _ in 0..300 {
        step(&mut v, &p, &Flat, DT);
    }
    let speed = (v.pos.z - z0) / (300.0 * DT);
    assert!((speed - 5.0).abs() < 0.02, "{speed} m/s");
    assert!(v.pos.x.abs() < 1e-3, "straight");
    assert!(
        v.pos.y.abs() <= LIFT_SLACK + 1e-4,
        "on its feet: {}",
        v.pos.y
    );
}

#[test]
fn a_part_stick_floors_the_clock_at_half() {
    let p = atst();
    let mut v = walker(&p);
    // |stick| below 0.5·6.8064/5 = 0.681.
    drive(&mut v, 0.6, 0.0, 0.0, false);
    for _ in 0..90 {
        step(&mut v, &p, &Flat, DT);
    }
    let z0 = v.pos.z;
    for _ in 0..300 {
        step(&mut v, &p, &Flat, DT);
    }
    let speed = (v.pos.z - z0) / (300.0 * DT);
    assert!((speed - 0.5 * 6.8064).abs() < 0.02, "{speed} m/s");
    // Just above the floor the clock follows the stick.
    let mut v = walker(&p);
    drive(&mut v, 0.8, 0.0, 0.0, false);
    for _ in 0..90 {
        step(&mut v, &p, &Flat, DT);
    }
    let z0 = v.pos.z;
    for _ in 0..300 {
        step(&mut v, &p, &Flat, DT);
    }
    let speed = (v.pos.z - z0) / (300.0 * DT);
    assert!((speed - 4.0).abs() < 0.02, "{speed} m/s");
}

#[test]
fn boost_runs_10_m_s_for_4_s_from_80_then_recovers_in_2_2_s() {
    let p = atst();
    let mut v = walker(&p);
    drive(&mut v, 1.0, 0.0, 0.0, true);
    step(&mut v, &p, &Flat, DT);
    assert!(!v.walker.boosting, "no walk direction on the first tick");
    let mut boosted = 0;
    let mut z = Vec::new();
    while v.walker.boosting || boosted == 0 {
        step(&mut v, &p, &Flat, DT);
        if v.walker.boosting {
            boosted += 1;
            z.push(v.pos.z);
        }
        assert!(boosted < 200);
    }
    // 80 / 20 a second: 120 ticks, and the one that takes it below 0.
    assert!((120..=121).contains(&boosted), "{boosted} ticks");
    // The tick that ends it already refills (floored at 0, then 9/30).
    assert!(v.walker.exhausted && (v.walker.energy - 0.3).abs() < 1e-5);
    // Walk clips at ×1.469 cover 10 m/s (the last 60 boosted ticks).
    let n = z.len();
    let speed = (z[n - 1] - z[n - 61]) / (60.0 * DT);
    assert!((speed - 10.0).abs() < 0.3, "{speed} m/s");
    // The button held: exhausted, it refills at 9/s and works again at 20.
    let mut ticks = 0;
    while v.walker.exhausted {
        step(&mut v, &p, &Flat, DT);
        ticks += 1;
        assert!(ticks < 200);
    }
    let t = (ticks + 1) as f32 * DT;
    assert!((t - 20.0 / 9.0).abs() < 0.05, "{t} s");
}

#[test]
fn the_body_yaws_at_most_1_rad_s_and_0_6_boosting() {
    let p = atst();
    // Standing, the look stick held left: the aim leaves the window and
    // the body follows at 2·StoppedTurnSpeed.
    let mut v = walker(&p);
    drive(&mut v, 0.0, 0.0, -1.0, false);
    let mut max_rate = 0.0f32;
    let mut y0 = yaw(&v);
    for _ in 0..300 {
        step(&mut v, &p, &Flat, DT);
        let y = yaw(&v);
        let mut d = y - y0;
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        if d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        max_rate = max_rate.max(d.abs() / DT);
        y0 = y;
    }
    assert!((max_rate - 1.0).abs() < 1e-3, "{max_rate} rad/s");
    // Boosting straight on with the aim off to one side: 2·BoostTurnSpeed.
    let mut v = walker(&p);
    drive(&mut v, 1.0, 0.0, 0.0, true);
    for _ in 0..40 {
        step(&mut v, &p, &Flat, DT);
    }
    assert!(v.walker.boosting);
    v.walker.aim_yaw = 0.5;
    let y0 = yaw(&v);
    step(&mut v, &p, &Flat, DT);
    let rate = (yaw(&v) - y0).abs() / DT;
    assert!((rate - 0.6).abs() < 1e-3, "{rate} rad/s");
}

#[test]
fn the_left_stick_walks_where_it_points_under_the_camera() {
    let p = atst();
    let mut v = walker(&p);
    // Stick right, the camera ahead: the body turns right (toward −x, the
    // model's right) until the aim sits at the stick's angle, then walks.
    drive(&mut v, 0.0, 1.0, 0.0, false);
    for _ in 0..240 {
        step(&mut v, &p, &Flat, DT);
    }
    let f = v.rot * Vec3::Z;
    assert!(f.x < -0.9, "facing right of the camera: {f:?}");
    assert!(v.pos.x < -1.0, "walked right: {:?}", v.pos);
    // The aim kept its world direction (+z): body-relative it is +90°.
    let aim_world = v.rot * crate::vehicle::gun::dir(v.walker.aim_yaw, 0.0);
    assert!(aim_world.z > 0.95, "{aim_world:?}");
}

#[test]
fn a_turn_step_fires_past_turn_threshold() {
    let p = atst();
    let set = |h: f32| {
        let mut v = walker(&p);
        v.walker.h = (h.cos(), h.sin());
        v
    };
    // Stick idle: the threshold is sin(0.6/2) = 0.2955, h = the half angle.
    let mut v = set(0.31);
    step(&mut v, &p, &Flat, DT);
    assert_eq!(v.walker.anim, ANIM_TURNR);
    assert!((v.walker.turn_step + 0.31).abs() < 1e-5);
    let mut v = set(-0.31);
    step(&mut v, &p, &Flat, DT);
    assert_eq!(v.walker.anim, ANIM_TURNL);
    let mut v = set(0.29);
    step(&mut v, &p, &Flat, DT);
    assert_eq!(v.walker.anim, ANIM_IDLE);
    // A nonzero stick from a standstill drops it to 0.1.
    let mut v = set(0.11);
    drive(&mut v, 0.05, 0.0, 0.0, false);
    step(&mut v, &p, &Flat, DT);
    assert_eq!(v.walker.anim, ANIM_TURNR);
    // The step brings the legs round: h → 0 over the clip.
    let mut v = set(0.4);
    for _ in 0..40 {
        step(&mut v, &p, &Flat, DT);
    }
    assert!(v.walker.h.1.abs() < 1e-4, "{:?}", v.walker.h);
    assert_eq!(v.walker.anim, ANIM_IDLE);
}

#[test]
fn one_pair_lifts_to_within_0_1_m_under_the_foot_ground() {
    let p = atst();
    let mut v = walker(&p);
    v.pos.y = -2.0;
    legs(&mut v, &p, &Flat, DT);
    assert!((v.pos.y + LIFT_SLACK).abs() < 1e-5, "{}", v.pos.y);
    assert!(v.walker.supported && v.vel.y == 0.0);
    // Within the slack it stays put.
    let mut v = walker(&p);
    v.pos.y = -0.05;
    legs(&mut v, &p, &Flat, DT);
    assert_eq!(v.pos.y, -0.05);
}

#[test]
fn it_falls_at_29_4_until_a_foot_ray_finds_ground() {
    let p = atst();
    let mut v = walker(&p);
    v.pos.y = 20.0;
    let mut vy = Vec::new();
    for _ in 0..60 {
        step(&mut v, &p, &Flat, DT);
        vy.push(v.vel.y);
        if v.walker.supported || v.vel.y == 0.0 {
            break;
        }
    }
    assert!((vy[1] - vy[0] + LEG_GRAVITY * DT).abs() < 1e-4, "{vy:?}");
    assert!((vy[0] + LEG_GRAVITY * DT).abs() < 1e-5);
    assert!(
        v.pos.y <= 0.0 && v.pos.y >= -LIFT_SLACK - 1.0,
        "{}",
        v.pos.y
    );
    for _ in 0..30 {
        step(&mut v, &p, &Flat, DT);
    }
    assert!(
        v.pos.y <= 0.0 && v.pos.y >= -LIFT_SLACK - 1e-4,
        "{}",
        v.pos.y
    );
}

#[test]
fn stopping_finishes_the_stride_then_idles() {
    let p = atst();
    let mut v = walker(&p);
    drive(&mut v, 1.0, 0.0, 0.0, false);
    for _ in 0..60 {
        step(&mut v, &p, &Flat, DT);
    }
    drive(&mut v, 0.0, 0.0, 0.0, false);
    let mut seen = Vec::new();
    for _ in 0..90 {
        step(&mut v, &p, &Flat, DT);
        if seen.last() != Some(&v.walker.anim) {
            seen.push(v.walker.anim);
        }
    }
    assert_eq!(*seen.last().unwrap(), ANIM_IDLE, "{seen:?}");
    assert!(
        seen.iter().any(|&a| a == 9 || a == 10),
        "a foot-to-idle: {seen:?}"
    );
    assert_eq!(v.walker.dir, 0);
}

#[test]
fn walking_backward_plays_the_clips_in_reverse() {
    let p = atst();
    let mut v = walker(&p);
    drive(&mut v, -1.0, 0.0, 0.0, false);
    for _ in 0..200 {
        step(&mut v, &p, &Flat, DT);
    }
    assert_eq!(v.walker.dir, 2);
    assert!(v.walker.reversed);
    assert!(v.pos.z < -5.0, "backed up: {:?}", v.pos);
}

#[test]
fn energy_drain_and_refill_follow_the_pool() {
    let mut w = WalkerState::new(&atst());
    assert_eq!(w.energy, 80.0);
    assert!(w.drain(79.0));
    assert!(w.drain(5.0), "the drain that empties it succeeds");
    assert!(w.exhausted && w.energy == 0.0);
    assert!(!w.drain(1.0));
    w.refill(19.0, 80.0);
    assert!(w.exhausted);
    w.refill(1.0, 80.0);
    assert!(!w.exhausted);
    w.refill(500.0, 80.0);
    assert_eq!(w.energy, 80.0);
}
