//! The hover's equations against re-hover.md's hand numbers (§6, §4, §7)
//! and the TAT1 skiff by hand (§9), on a flat plane and a wall.

use super::*;
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::HealthType;

const DT: f32 = 1.0 / 30.0;

/// `tat_hover_skiff` as re-hover §9 reads it (TAT1 classes.ron over the BF2
/// defaults): three AddSpringBody points, BodySpringLength 1 each, the entry
/// init's lever-arm signs, BodyOmegaXSpringFactor 2 on the third. No hull
/// body: its terrain / building subsets hold its springs alone (§13.2).
pub(crate) fn skiff() -> HoverParams {
    let spring = |pos: [f32; 3], radius: f32, oxsf: f32, ozsf: f32| Spring {
        pos: Vec3::from(pos),
        radius,
        length: 1.0,
        velocity_spring: 1.0,
        velocity_damp: 1.0,
        omega_x_spring: oxsf,
        omega_x_damp: 1.0,
        omega_z_spring: ozsf,
        omega_z_damp: 1.0,
    };
    HoverParams {
        mass: 1.0,
        set_altitude: 1.0,
        gravity_scale: 4.0,
        acceleration: 5.0,
        deceleration: 8.0,
        traction: 30.0,
        forward_speed: 16.0,
        reverse_speed: 12.0,
        strafe_speed: 4.0,
        spin_rate: 1.0,
        turn_rate: 1.0,
        pitch_rate: 1.5,
        turn_filter: 2.0,
        thrust_pitch_angle: 0.01,
        strafe_roll_angle: 0.01,
        bank_angle: -0.01,
        bank_filter: 3.0,
        level_spring: 7.0,
        velocity_spring: 5.0,
        velocity_damp: 2.5,
        omega_x_spring: 4.0,
        omega_x_damp: 2.2,
        omega_z_spring: 6.0,
        omega_z_damp: 2.2,
        boost_speed: 0.0,
        boost_turn_multiplier: 0.5,
        boost_acceleration: 0.0,
        energy_auto_restore: 0.0,
        energy_boost_drain: 0.0,
        jump: JumpParams::default(),
        collision_threshold: [7.5, 2.5, 0.0],
        collision_scale: [30.0, 0.0, 0.0],
        bf1_ai: false,
        random_spring: true,
        floats_on_water: true,
        springs: vec![
            spring([-1.8, -0.7, 1.7], 2.0, -1.0, -1.0),
            spring([1.8, -0.7, 1.7], 2.0, -1.0, 1.0),
            spring([0.0, 0.3, -2.0], 3.0, 2.0, 0.0),
        ],
        hull: Vec::new(),
    }
}

fn vehicle(pos: Vec3, yaw: f32) -> Vehicle {
    let mut v = VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos,
        yaw,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
    .build(1);
    v.hover.jitter = 12345;
    v
}

/// Flat ground at y = 0, and an optional wall x = `wall` facing −x.
struct World {
    wall: Option<f32>,
}

impl ContactSource for World {
    fn sphere_contacts(&self, c: Vec3, r: f32, out: &mut Vec<Contact>) {
        if r - c.y > 0.0 {
            out.push(Contact {
                depth: r - c.y,
                normal: Vec3::Y,
            });
        }
        if let Some(x) = self.wall {
            if r - (x - c.x) > 0.0 {
                out.push(Contact {
                    depth: r - (x - c.x),
                    normal: Vec3::NEG_X,
                });
            }
        }
    }

    /// The body's reach along −y (the ground, a terrain body) and +x (the
    /// wall, a building body).
    fn hull_contacts(&self, b: &HullBody, pos: Vec3, rot: Mat3, out: &mut Vec<Contact>) {
        let reach = |axis: Vec3| match b.shape {
            HullShape::Sphere { radius } => radius,
            HullShape::Box { half } => (rot.transpose() * axis).abs().dot(half),
            HullShape::Cylinder { .. } => unimplemented!(),
        };
        let down = reach(Vec3::Y) - pos.y;
        if b.terrain && down > 0.0 {
            out.push(Contact {
                depth: down,
                normal: Vec3::Y,
            });
        }
        if let Some(x) = self.wall.filter(|_| b.building) {
            let depth = reach(Vec3::X) - (x - pos.x);
            if depth > 0.0 {
                out.push(Contact {
                    depth,
                    normal: Vec3::NEG_X,
                });
            }
        }
    }
}

const FLAT: World = World { wall: None };

fn run(v: &mut Vehicle, p: &HoverParams, w: &World, ticks: u32) -> Vec<Impact> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        all.extend(step(v, p, w, DT));
    }
    all
}

/// A skiff settled for 10 s on the plane.
fn settled(p: &HoverParams) -> Vehicle {
    let mut v = vehicle(Vec3::new(0.0, 3.0, 0.0), 0.0);
    run(&mut v, p, &FLAT, 300);
    v
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// Deep water over a far lakebed: the surface at y = 0 over every cell, the
/// ground 40 m down.
struct Lake;

impl ContactSource for Lake {
    fn sphere_contacts(&self, c: Vec3, r: f32, out: &mut Vec<Contact>) {
        FLAT.sphere_contacts(c + Vec3::Y * 40.0, r, out);
    }

    fn hull_contacts(&self, b: &HullBody, pos: Vec3, rot: Mat3, out: &mut Vec<Contact>) {
        FLAT.hull_contacts(b, pos + Vec3::Y * 40.0, rot, out);
    }

    fn water_under(&self, _at: Vec3) -> Option<f32> {
        Some(0.0)
    }
}

/// §4 S3: a spring sphere sunk d into the water compresses min(d / length,
/// 1), as one sunk d into flat ground does, so a hover rests on a lake at the
/// height it rests on ground at the surface's level. With `FloatsOnWater` 0
/// it sinks to the bed.
#[test]
fn a_hover_rests_on_water_as_on_ground_at_its_level() {
    let p = skiff();
    let mut ground = vehicle(Vec3::new(0.0, 3.0, 0.0), 0.0);
    run(&mut ground, &p, &FLAT, 1800);
    let mut v = vehicle(Vec3::new(0.0, 3.0, 0.0), 0.0);
    for _ in 0..1800 {
        step(&mut v, &p, &Lake, DT);
    }
    assert!(
        close(v.pos.y, ground.pos.y, 1e-3),
        "water {} ground {}",
        v.pos.y,
        ground.pos.y
    );
    let sinks = HoverParams {
        floats_on_water: false,
        ..skiff()
    };
    let mut v = vehicle(Vec3::new(0.0, 3.0, 0.0), 0.0);
    for _ in 0..1800 {
        step(&mut v, &sinks, &Lake, DT);
    }
    assert!(v.pos.y < -30.0, "{}", v.pos.y);
}

/// §6.5a: gravity weighted by g — 9.8·(GS + (1 − GS)·g). The skiff (GS 4)
/// at g = 1 − dt falls at 10.78 m/s², at g = 1 at 9.8, airborne at 39.2.
#[test]
fn gravity_is_weighted_by_the_ground_factor() {
    let p = skiff();
    for (g, a) in [(29.0 / 30.0, 10.78), (1.0, 9.8), (0.0, 39.2)] {
        let mut v = vehicle(Vec3::ZERO, 0.0);
        v.hover.g = g;
        drive(&mut v, &p, DT);
        assert!(close(v.vel.y, -a * DT, 1e-5), "g {g}: {}", v.vel.y / DT);
    }
}

/// §6.7: the forward step is (Deceleration + (Acceleration − Deceleration)·m)
/// ·g·up.y·dt — the skiff at full throttle 5·(29/30)/30 = 0.1611 m/s a tick;
/// idle, 8·(29/30)/30 toward zero.
#[test]
fn accel_step_matches_the_hand_number() {
    let p = skiff();
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.hover.g = 29.0 / 30.0;
    v.input.throttle = 1.0;
    drive(&mut v, &p, DT);
    assert!(
        close(v.vel.z, 5.0 * (29.0 / 30.0) / 30.0, 1e-6),
        "{}",
        v.vel.z
    );

    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.hover.g = 29.0 / 30.0;
    v.vel = Vec3::new(0.0, 0.0, 10.0);
    drive(&mut v, &p, DT);
    assert!(close(v.vel.z, 10.0 - 8.0 * (29.0 / 30.0) / 30.0, 1e-5));
    // Airborne (g = 0): no thrust at all.
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.input.throttle = 1.0;
    drive(&mut v, &p, DT);
    assert_eq!(v.vel.z, 0.0);
}

/// §6.7: with StrafeSpeed < 1e-4 the side slip is killed at Traction·s a
/// tick (30·(29/30)/30 = 0.967 m/s); with a strafe the side step is aS.
#[test]
fn traction_step_kills_side_slip() {
    let mut p = skiff();
    p.strafe_speed = 0.0;
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.hover.g = 29.0 / 30.0;
    v.vel = Vec3::new(2.0, 0.0, 0.0);
    drive(&mut v, &p, DT);
    assert!(
        close(v.vel.x, 2.0 - 30.0 * (29.0 / 30.0) / 30.0, 1e-5),
        "{}",
        v.vel.x
    );
    // Traction clamps: a small slip goes to zero in one tick.
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.hover.g = 29.0 / 30.0;
    v.vel = Vec3::new(0.5, 0.0, 0.0);
    drive(&mut v, &p, DT);
    assert!(v.vel.x.abs() < 1e-6);
}

/// §6.10: base = SpinRate + (TurnRate − SpinRate)·clamp(vF/FS); a human
/// blends from PitchRate by the full-stick timer. Defaults 3 / 2 / 0.5, FS 20.
#[test]
fn turn_rate_blends_by_speed_and_timer() {
    let p = HoverParams {
        spin_rate: 3.0,
        turn_rate: 2.0,
        pitch_rate: 0.5,
        forward_speed: 20.0,
        ..skiff()
    };
    let ai = HoverInput {
        turn: 1.0,
        ..HoverInput::default()
    };
    assert_eq!(
        yaw_target(&p, &ai, 0.0, 0.0, 0.0),
        -3.0,
        "standing: SpinRate"
    );
    assert_eq!(yaw_target(&p, &ai, 0.0, 10.0, 10.0), -2.5);
    assert_eq!(yaw_target(&p, &ai, 0.0, 40.0, 40.0), -2.0, "clamped at FS");
    assert_eq!(
        yaw_target(&p, &ai, 0.0, -5.0, 5.0),
        -3.0,
        "reversing: SpinRate"
    );
    let pad = HoverInput { human: true, ..ai };
    assert_eq!(
        yaw_target(&p, &pad, 0.0, 10.0, 10.0),
        -0.5,
        "PitchRate at first"
    );
    assert_eq!(yaw_target(&p, &pad, 0.5, 10.0, 10.0), -1.5);
    assert_eq!(yaw_target(&p, &pad, 1.0, 10.0, 10.0), -2.5);
}

/// §6.10's boost turn scale (B2:134382-134398): above ForwardSpeed a class
/// with a BoostSpeed scales the target by 1 − fb·(1 − BoostTurnMultiplier),
/// fb = clamp((|v| − FS)/(BS − FS)) — boosting or not. FS 20, BS 40, 0.5.
#[test]
fn boost_speed_scales_the_turn_above_forward_speed() {
    let p = HoverParams {
        spin_rate: 3.0,
        turn_rate: 2.0,
        forward_speed: 20.0,
        boost_speed: 40.0,
        boost_turn_multiplier: 0.5,
        ..skiff()
    };
    let ai = HoverInput {
        turn: 1.0,
        ..HoverInput::default()
    };
    assert_eq!(yaw_target(&p, &ai, 0.0, 10.0, 15.0), -2.5, "below FS: none");
    assert_eq!(yaw_target(&p, &ai, 0.0, 30.0, 30.0), -1.5, "fb 0.5: x0.75");
    assert_eq!(yaw_target(&p, &ai, 0.0, 50.0, 50.0), -1.0, "fb 1: x0.5");
    // |v|, not the forward speed: a side slide at 30 m/s scales too.
    assert_eq!(yaw_target(&p, &ai, 0.0, 0.0, 30.0), -2.25);
    let no_boost = HoverParams {
        boost_speed: 0.0,
        ..p
    };
    assert_eq!(
        yaw_target(&no_boost, &ai, 0.0, 30.0, 30.0),
        -2.0,
        "BoostSpeed 0"
    );
}

/// §6.19 (B2:134798-134800): a SetAltitude ≤ 0 class ends the drive with
/// g = 0, so airborne (no spring contact) it has none after the tick; a
/// SetAltitude > 0 class keeps the spring tick's decay.
#[test]
fn set_altitude_zero_drops_g_after_the_drive() {
    let tread = HoverParams {
        set_altitude: 0.0,
        ..skiff()
    };
    let mut v = vehicle(Vec3::new(0.0, 50.0, 0.0), 0.0);
    v.hover.g = 29.0 / 30.0;
    drive(&mut v, &tread, DT);
    assert_eq!(v.hover.g, 0.0);
    springs(&mut v, &tread, |_| None, DT);
    assert_eq!(v.hover.g, 0.0, "airborne: g = 0 after the tick");
    let mut v = vehicle(Vec3::new(0.0, 50.0, 0.0), 0.0);
    v.hover.g = 29.0 / 30.0;
    drive(&mut v, &skiff(), DT);
    springs(&mut v, &skiff(), |_| None, DT);
    assert!(close(v.hover.g, 28.0 / 30.0, 1e-6), "{}", v.hover.g);
}

/// FUN_00160da8 re-queries after each push-out: the skiff bottomed level at
/// y 0 (its spring spheres' centres at −0.7, −0.7, 0.3; radii 2, 2, 3;
/// length 1) is pushed out by the first sphere's 2.7 − 1 = 1.7 m, which
/// leaves the other two at depth 1.0 — the spring's, no push. Found all at
/// the start pose they would add 1.7 + 2.3 more.
#[test]
fn bottomed_springs_push_out_one_after_another() {
    let p = skiff();
    let mut v = vehicle(Vec3::ZERO, 0.0);
    let impacts = contacts(&mut v, &p, &World { wall: None });
    assert!(close(v.pos.y, 1.7, 1e-5), "{}", v.pos.y);
    assert_eq!(impacts.len(), 1);
    assert_eq!(v.hover.compress[..3], [1.0, 1.0, 1.0]);
}

/// §5.2: +dt while the raw stick is at full deflection, capped at 1; else 0.
#[test]
fn stick_timer_ramps_at_full_deflection() {
    let mut st = HoverState::default();
    for _ in 0..40 {
        ramp_stick_timer(&mut st, true, DT);
    }
    assert_eq!(st.stick_timer, 1.0);
    ramp_stick_timer(&mut st, false, DT);
    assert_eq!(st.stick_timer, 0.0);
}

/// §6.10: a slew of TurnFilter·dt a tick that stops at the target — the
/// skiff (TurnFilter 2) reaches 1 rad/s after 15 ticks = 0.5 s.
#[test]
fn yaw_slews_to_its_target() {
    let mut w = 0.0;
    let mut n = 0;
    while w < 1.0 {
        w = slew(w, 1.0, 2.0, DT);
        n += 1;
    }
    assert_eq!((n, w), (15, 1.0));
    assert_eq!(slew(0.9, 1.0, 2.0, DT), 0.9 + 2.0 * DT);
    assert_eq!(slew(1.0, -1.0, 2.0, DT), 1.0 - 2.0 * DT);
    assert_eq!(slew(-0.99, -1.0, 2.0, DT), -1.0, "stops at the target");
}

/// §4 S4: v += up·(c·VSF·VS − (1−c)·VDF·VD·min(v·up, 0))·dt, and g = 1,
/// then the tick's g decay leaves 1 − dt; the compression is cleared.
#[test]
fn spring_force_matches_the_hand_number() {
    let mut p = skiff();
    p.random_spring = false;
    p.springs.truncate(1);
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.hover.compress[0] = 0.5;
    springs(&mut v, &p, |_| None, DT);
    assert!(close(v.vel.y, 0.5 * 5.0 * DT, 1e-7));
    // OXSF −1 · OmegaXSpring 4 · c; OZSF −1 · OmegaZSpring 6 · c.
    assert!(close(v.hover.omega.x, -0.5 * 4.0 * DT, 1e-7));
    assert!(close(v.hover.omega.z, -0.5 * 6.0 * DT, 1e-7));
    assert_eq!(v.hover.g, 1.0 - DT);
    assert_eq!(v.hover.compress[0], 0.0);
    // Falling at 2 m/s the damper adds (1 − c)·VD·2 = 2.5.
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.vel.y = -2.0;
    v.hover.compress[0] = 0.5;
    springs(&mut v, &p, |_| None, DT);
    assert!(close(v.vel.y, -2.0 + 5.0 * DT, 1e-6), "{}", v.vel.y);
    // Untouched: nothing but the decay.
    let mut v = vehicle(Vec3::ZERO, 0.0);
    v.hover.g = 0.02;
    springs(&mut v, &p, |_| None, DT);
    assert_eq!((v.vel, v.hover.g), (Vec3::ZERO, 0.0));
}

/// §7 K5 and the hook: c = min(d·(n·up), len)/len; within the length the
/// contact is the spring's; deeper, push out by d − len, kill the closing
/// speed and deal an impact. A wall (n·up = 0) never compresses.
#[test]
fn spring_contacts_compress_then_bottom_out() {
    let mut p = skiff();
    p.springs.truncate(1);
    // Sphere bottom 0.4 m under the plane: c = 0.4.
    let mut v = vehicle(Vec3::new(0.0, 2.7 - 0.4, 0.0), 0.0);
    v.vel.y = -3.0;
    assert!(contacts(&mut v, &p, &FLAT).is_empty());
    assert!(close(v.hover.compress[0], 0.4, 1e-5));
    assert_eq!((v.pos.y, v.vel.y), (2.3, -3.0), "no push inside the length");
    // 1.5 m under: c = 1, pushed up 0.5, v.y zeroed, one impact.
    let mut v = vehicle(Vec3::new(0.0, 2.7 - 1.5, 0.0), 0.0);
    v.vel.y = -9.0;
    let im = contacts(&mut v, &p, &FLAT);
    assert_eq!(v.hover.compress[0], 1.0);
    assert!(close(v.pos.y, 1.2 + 0.5, 1e-5));
    assert_eq!(v.vel.y, 0.0);
    assert_eq!(
        im,
        vec![Impact {
            normal_speed: -9.0,
            n_up: 1.0,
            n_fwd: 0.0
        }]
    );
}

/// §7 K7-K9: a hull sphere against a static wall comes back at 0.125 of its
/// closing speed and is pushed out by the full depth; no impact.
#[test]
fn hull_contacts_bounce_at_one_eighth() {
    let mut p = skiff();
    p.springs.clear();
    p.hull = vec![HullBody {
        shape: HullShape::Sphere { radius: 1.0 },
        pos: Vec3::new(0.0, 5.0, 0.0),
        rot: Mat3::IDENTITY,
        terrain: true,
        building: true,
    }];
    let wall = World { wall: Some(10.0) };
    let mut v = vehicle(Vec3::new(9.5, 0.0, 0.0), 0.0);
    v.vel = Vec3::new(8.0, 0.0, 0.0);
    assert!(contacts(&mut v, &p, &wall).is_empty());
    assert!(close(v.vel.x, -1.0, 1e-6), "{}", v.vel.x);
    assert!(close(v.pos.x, 9.0, 1e-6));
    // Friction: the tangential speed loses μ·j = 0.25·9 = 2.25 of 4.
    let mut v = vehicle(Vec3::new(9.5, 0.0, 0.0), 0.0);
    v.vel = Vec3::new(8.0, 0.0, 4.0);
    contacts(&mut v, &p, &wall);
    assert!(close(v.vel.z, 4.0 - 2.25, 1e-6), "{}", v.vel.z);
}

/// §13.2: a hull body meets only the layers its class names it in — the
/// fighter tanks' `p_cube1` the buildings, not the terrain — and a box is
/// met at its own faces, turned with the vehicle.
#[test]
fn a_hull_box_meets_only_its_layers() {
    let mut p = skiff();
    p.springs.clear();
    p.hull = vec![p_cube1()];
    let wall = World { wall: Some(10.0) };
    // Nose at the wall: the box's front face, 1.559 + 0.956 ahead of the
    // origin, is 0.515 in.
    let mut v = vehicle(Vec3::new(8.0, 0.2, 0.0), -std::f32::consts::FRAC_PI_2);
    assert!((v.rot * Vec3::Z).x > 0.999, "facing +x");
    v.vel = Vec3::new(8.0, 0.0, 0.0);
    assert!(contacts(&mut v, &p, &wall).is_empty());
    assert!(close(v.pos.x, 10.0 - 2.5149, 1e-3), "{}", v.pos.x);
    assert!(close(v.vel.x, -1.0, 1e-5), "{}", v.vel.x);
    // Its floor, 0.02 over the origin, is under the ground: no contact.
    assert!(close(v.pos.y, 0.2, 1e-6), "the terrain is not its layer");
    // Side on, the half-width 1.511 is clear of a wall 2 m off.
    let mut v = vehicle(Vec3::new(8.0, 0.2, 0.0), 0.0);
    contacts(&mut v, &p, &wall);
    assert!(close(v.pos.x, 8.0, 1e-6));
}

/// §13.5, the PS2's sphere against the terrain: d = h − c.y + r inside
/// [0, 2r], depth d·n.y along the triangle's normal.
#[test]
fn the_terrain_sphere_rule_by_hand() {
    // Flat: r 2, centre 1.5 over the ground → d 0.5, depth 0.5.
    let c = terrain_sphere(1.5, 2.0, 0.0, Vec3::Y).unwrap();
    assert!(close(c.depth, 0.5, 1e-6));
    // The window is the terrain within r of the centre's height.
    assert!(terrain_sphere(2.01, 2.0, 0.0, Vec3::Y).is_none(), "clear");
    assert!(terrain_sphere(-1.99, 2.0, 0.0, Vec3::Y).is_some());
    assert!(terrain_sphere(-2.01, 2.0, 0.0, Vec3::Y).is_none(), "buried");
    // A 30° slope, the centre 1 m over the surface point: d = 1, depth
    // d·cos 30° = 0.8660. The plane rule this replaces gave r − 1·cos 30°
    // = 1.1340: r(1 − n.y) = 0.2679 deeper.
    let n = Vec3::new(0.5, 0.75f32.sqrt(), 0.0);
    let c = terrain_sphere(1.0, 2.0, 0.0, n).unwrap();
    assert!(close(c.depth, 0.866_025, 1e-5), "{}", c.depth);
    assert_eq!(c.normal, n);
    let plane = 2.0 - 1.0 * n.y;
    assert!(close(plane - c.depth, 0.267_949, 1e-5));
    // 20°: 0.1206 shallower.
    let ny = 0.939_692_6;
    let c = terrain_sphere(1.0, 2.0, 0.0, Vec3::new(0.342_020_1, ny, 0.0)).unwrap();
    assert!(close((2.0 - ny) - c.depth, 0.120_615, 1e-5));
}

/// The BF2 fighter tanks' `p_cube1` (§13.2's census): a box, half-extents
/// (1.511, 0.491, 0.956) at (0, 0.511, 1.559), in the building subset only.
fn p_cube1() -> HullBody {
    HullBody {
        shape: HullShape::Box {
            half: Vec3::new(1.510_763_2, 0.491_286_04, 0.955_526_35),
        },
        pos: Vec3::new(0.0, 0.510_627_2, 1.559_324_6),
        rot: Mat3::IDENTITY,
        terrain: false,
        building: true,
    }
}

/// BF2's `imp_hover_fightertank` as its ODF reads over the constructor
/// defaults: four r 2.3 springs of length 1, the rear pair's
/// BodyOmegaXSpringFactor 1.2.
fn fighter_tank(hull: Vec<HullBody>) -> HoverParams {
    let spring = |x: f32, z: f32, oxsf: f32| Spring {
        pos: Vec3::new(x, 1.25, z),
        radius: 2.3,
        length: 1.0,
        velocity_spring: 1.0,
        velocity_damp: 1.0,
        omega_x_spring: oxsf,
        omega_x_damp: 1.0,
        omega_z_spring: x.signum(),
        omega_z_damp: 1.0,
    };
    HoverParams {
        set_altitude: 0.5,
        gravity_scale: 4.0,
        acceleration: 7.0,
        deceleration: 5.5,
        traction: 20.0,
        forward_speed: 9.0,
        reverse_speed: 7.0,
        strafe_speed: 6.0,
        spin_rate: 3.0,
        turn_rate: 2.0,
        pitch_rate: 0.5,
        turn_filter: 10.0,
        thrust_pitch_angle: 0.0,
        strafe_roll_angle: 0.0,
        bank_angle: 0.0,
        level_spring: 2.0,
        omega_x_spring: 3.0,
        omega_x_damp: 3.0,
        omega_z_spring: 5.0,
        omega_z_damp: 1.5,
        collision_threshold: [5.0, 2.5, 0.0],
        collision_scale: [1.5, 0.0, 0.0],
        random_spring: true,
        floats_on_water: true,
        springs: vec![
            spring(-1.8, 1.7, -1.0),
            spring(1.8, 1.7, -1.0),
            spring(-1.4, -2.3, 1.2),
            spring(1.4, -2.3, 1.2),
        ],
        hull,
        ..skiff()
    }
}

/// Where a fighter tank ends after 10 s of full throttle from the origin
/// along its heading `yaw`, on flat ground, at a thin post (r 0.1) standing
/// 12 m ahead between its front springs: how far it came and its speed.
fn tank_at_a_post(hull: Vec<HullBody>, yaw: f32) -> (f32, f32) {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::heightfield::Heightfield;
    use crate::sim::Sim;
    let mut s = Sim::empty(1);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let fwd = vehicle(Vec3::ZERO, yaw).rot * Vec3::Z;
    let post = Body::new(
        glam::Mat4::from_translation(fwd * 12.0 + Vec3::new(0.0, 3.0, 0.0)),
        Shape::Cylinder {
            radius: 0.1,
            half_height: 3.0,
        },
    );
    s.collision = CollisionWorld::new(vec![post], Vec::new());
    s.hover_params = vec![Some(fighter_tank(hull))];
    let h = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 1.5, 0.0),
        yaw,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    for _ in 0..300 {
        s.vehicle_mut(h).unwrap().input.throttle = 1.0;
        s.tick_hovers(DT);
    }
    let v = s.vehicle(h).unwrap();
    (v.pos.dot(fwd), v.vel.length())
}

/// §13 (card HHUL): the post passes between the front spring spheres — it
/// is never deeper in one than BodySpringLength (1.8 m off their centres, r
/// 2.3 + 0.1: 0.6 at most), so K5 gives no response — and stops the tank at
/// `p_cube1`'s front face, 2.515 m ahead of the origin. With no hull body
/// the tank drives through it.
#[test]
fn a_fighter_tank_stops_at_a_post_on_its_hull_box() {
    let (far, speed) = tank_at_a_post(Vec::new(), 0.0);
    assert!(
        far > 40.0 && speed > 8.0,
        "no hull: through it ({far}, {speed})"
    );
    // Along +z, and on a heading that turns the box against the world's axes.
    for yaw in [0.0, 0.7] {
        let (far, speed) = tank_at_a_post(vec![p_cube1()], yaw);
        eprintln!("yaw {yaw}: the origin {far} m out at {speed} m/s");
        // The post's near side, 11.9, on the face: the origin at 9.385.
        assert!(close(far, 11.9 - 2.514_851, 0.1), "yaw {yaw}: {far}");
        assert!(speed < 1.5, "yaw {yaw}: held at the post, {speed} m/s");
    }
}

/// §9: the skiff at rest rides 1.95 m over flat ground; 1.98 without the
/// jitter. With the contacts before the spring tick (card HSTEP,
/// `re-vehicle-rest.md` §5) the jitter bobs it ±1.2 cm: the numeric model
/// in that order (`lead_tmp/hstep/skiff_sim_new.py`, last 5 of 15 s) reads
/// mean 1.948, 1.937-1.961, 1.984 without the jitter. The old order
/// (springs, then contacts: each push one pose late) bobbed 1.887-2.022.
#[test]
fn skiff_settles_at_its_spring_height() {
    let p = skiff();
    let mut v = settled(&p);
    let (mut sum, mut lo, mut hi) = (0.0, f32::MAX, f32::MIN);
    for _ in 0..150 {
        step(&mut v, &p, &FLAT, DT);
        sum += v.pos.y;
        lo = lo.min(v.pos.y);
        hi = hi.max(v.pos.y);
    }
    let mean = sum / 150.0;
    assert!(close(mean, 1.948, 0.01), "mean {mean}");
    assert!(lo > 1.925 && hi < 1.975, "bob {lo}..{hi}");
    assert!(v.hover.g > 0.9, "touching");
    let up = v.rot * Vec3::Y;
    assert!(up.y > 0.999, "level: {up}");

    let mut p = skiff();
    p.random_spring = false;
    let mut v = settled(&p);
    let mut sum = 0.0;
    for _ in 0..150 {
        step(&mut v, &p, &FLAT, DT);
        sum += v.pos.y;
    }
    assert!(close(sum / 150.0, 1.984, 0.01), "{}", sum / 150.0);
}

/// §9: 0 → 16 m/s in ≈ 3.3 s by hand, never past ForwardSpeed; released, it
/// brakes at Deceleration·g = 7.73 m/s². The hand number ignores the tilt:
/// full throttle pitches the body ~0.01 rad nose-down (ThrustPitchAngle,
/// fed back into the body), so the springs push a little forward too — the
/// numeric model (`lead_tmp/hvre/skiff_sim.py`, first tick with vF ≥ 15.95)
/// takes 3.1 s, the port 3.13 s. (re-hover §9's "3.4 s" is that model's
/// 0.5 s print grid.)
#[test]
fn skiff_reaches_forward_speed_and_coasts_down() {
    let p = skiff();
    let mut v = settled(&p);
    v.input.throttle = 1.0;
    let mut t = 0;
    while (v.rot * Vec3::Z).dot(v.vel) < 15.95 {
        step(&mut v, &p, &FLAT, DT);
        t += 1;
        assert!(t < 200, "never reached 16 m/s");
    }
    let secs = t as f32 * DT;
    assert!((3.0..=3.3).contains(&secs), "{secs} s");
    run(&mut v, &p, &FLAT, 60);
    let s0 = v.vel.length();
    assert!(close(s0, 16.0, 0.05), "top speed {s0}");
    v.input.throttle = 0.0;
    run(&mut v, &p, &FLAT, 30);
    let decel = s0 - v.vel.length();
    assert!(close(decel, 7.73, 0.3), "coast-down {decel} m/s²");
}

/// §9: at speed the AI turns at 1.0 rad/s (ω reaches it 0.5 s after full
/// stick), the carve keeps the velocity on the heading: a 16 m circle.
#[test]
fn skiff_turns_at_one_radian_a_second() {
    let p = skiff();
    let mut v = settled(&p);
    v.input.throttle = 1.0;
    run(&mut v, &p, &FLAT, 150);
    v.input.turn = -1.0;
    run(&mut v, &p, &FLAT, 15);
    assert!(close(v.hover.omega.y, 1.0, 1e-5), "{}", v.hover.omega.y);
    run(&mut v, &p, &FLAT, 60);
    let heading = |v: &Vehicle| {
        let f = v.rot * Vec3::Z;
        crate::fmath::atan2(f.x, f.z)
    };
    let (h0, p0) = (heading(&v), v.pos);
    run(&mut v, &p, &FLAT, 15);
    let rate = (heading(&v) - h0) / (15.0 * DT);
    assert!(close(rate, 1.0, 0.03), "heading rate {rate}");
    let speed = v.vel.length();
    assert!(close(speed / rate, 16.0, 0.8), "radius {}", speed / rate);
    // The velocity follows the heading (the carve), within a few degrees.
    let f = v.rot * Vec3::Z;
    assert!(f.dot(v.vel.normalize()) > 0.99);
    assert!(p0.distance(v.pos) > 7.0);
}

/// §9: a pad driver's rate starts at PitchRate 1.5 and blends to 1.0 over
/// 1 s of full stick; ω meets the falling target near 1.2 at 0.6 s.
#[test]
fn skiff_pad_turn_peaks_then_settles() {
    let p = skiff();
    let mut v = settled(&p);
    v.input = HoverInput {
        throttle: 1.0,
        human: true,
        ..HoverInput::default()
    };
    run(&mut v, &p, &FLAT, 150);
    v.input.turn = -1.0;
    let mut peak = (0.0f32, 0);
    for t in 1..=45 {
        // The pad controller's timer, full right stick (`Sim`'s seat pass).
        ramp_stick_timer(&mut v.hover, true, DT);
        step(&mut v, &p, &FLAT, DT);
        if v.hover.omega.y > peak.0 {
            peak = (v.hover.omega.y, t);
        }
        if t == 30 {
            assert!(
                close(v.hover.omega.y, 1.0, 1e-4),
                "1 s: {}",
                v.hover.omega.y
            );
        }
    }
    assert!(close(peak.0, 1.2, 0.02), "peak {peak:?}");
    assert!((17..=19).contains(&peak.1), "peak tick {}", peak.1);
}

/// Driven into a wall at full speed: the front springs bottom out on it
/// (n·up = 0, no compression), push the body back by d − len and kill the
/// closing speed; the skiff stays out of the wall and takes an impact.
#[test]
fn skiff_stops_at_a_wall() {
    let p = skiff();
    let wall = World { wall: Some(60.0) };
    // Facing +x: rot = quat_y(π/2) turns +z onto +x.
    let mut v = vehicle(Vec3::new(0.0, 3.0, 0.0), -core::f32::consts::FRAC_PI_2);
    assert!(close((v.rot * Vec3::Z).x, 1.0, 1e-6));
    run(&mut v, &p, &wall, 150);
    v.input.throttle = 1.0;
    let im = run(&mut v, &p, &wall, 300);
    assert!(!im.is_empty(), "the bottomed springs hit");
    assert!(v.pos.x < 60.0 - 1.7 - 2.0 + 1.0 + 0.2, "x {}", v.pos.x);
    assert!(v.vel.x.abs() < 1.0, "stopped: {}", v.vel.x);
    assert!(
        close(v.pos.y, 1.95, 0.15),
        "still on its springs: {}",
        v.pos.y
    );
}

/// Determinism: two runs from the same state are bit-identical.
#[test]
fn two_runs_are_bit_identical() {
    let p = bike();
    let go = || {
        let mut v = vehicle(Vec3::new(0.0, 3.0, 0.0), 0.3);
        v.input = HoverInput {
            throttle: 1.0,
            strafe: 0.5,
            turn: -0.7,
            pitch: 0.0,
            human: true,
            boost: true,
            jump: true,
        };
        run(&mut v, &p, &FLAT, 600);
        let mut bytes = Vec::new();
        v.hash_into(&mut |b| bytes.extend_from_slice(b));
        bytes
    };
    assert_eq!(go(), go());
}

/// The skiff with BF2's `imp_hover_speederbike` boost, energy and jump
/// fields (bf2_imp classes: BoostSpeed 40, BoostAcceleration 100,
/// EnergyAutoRestore 15, EnergyBoostDrain 20, jump 0.1 / 0.35 / 50 / 0.4 /
/// 100) — the skiff's ForwardSpeed 16 keeps the hand numbers simple.
fn bike() -> HoverParams {
    HoverParams {
        boost_speed: 40.0,
        boost_acceleration: 100.0,
        boost_turn_multiplier: 0.3,
        energy_auto_restore: 15.0,
        energy_boost_drain: 20.0,
        jump: JumpParams {
            time_min: 0.1,
            time_max: 0.35,
            force: 50.0,
            speed_factor: 0.4,
            energy_drain: 100.0,
        },
        ..skiff()
    }
}

/// FUN_001e1fc0 / FUN_001e2010: a drain that takes the pool below 0 still
/// succeeds (floored at 0) and marks it dry; a dry pool refuses until a
/// refill reaches 20; a refill caps at 100 only once past 20.
#[test]
fn energy_pool_runs_dry_and_recovers_at_twenty() {
    let mut h = HoverState {
        energy: 5.0,
        ..HoverState::default()
    };
    assert!(h.drain(0.0) && h.energy == 5.0);
    assert!(h.drain(6.0), "the tick that runs it dry succeeds");
    assert_eq!((h.energy, h.drained), (0.0, true));
    assert!(!h.drain(1.0), "dry: refused");
    h.refill(19.0);
    assert!(h.drained && !h.drain(1.0), "19 < 20: still dry");
    h.refill(1.0);
    assert!(!h.drained && h.drain(1.0));
    h.refill(500.0);
    assert_eq!(h.energy, ENERGY_MAX);
    assert_eq!(HoverState::default().energy, ENERGY_MAX, "starts full");
}

/// §5.1 I5, §6.3, §6.7: full throttle with the button latches the boost —
/// BoostAcceleration toward BoostSpeed, 20 energy a second; letting go of
/// the button keeps it latched; it drops when the pool runs dry after 5 s
/// (150 ticks of 20/30) and the pool refills at 15 a second. An AI (no
/// human) never boosts; below 0.9 throttle neither does a human.
#[test]
fn boost_latches_drains_and_drops_when_dry() {
    let p = bike();
    let mut v = settled(&p);
    v.input = HoverInput {
        throttle: 1.0,
        human: true,
        boost: true,
        ..HoverInput::default()
    };
    run(&mut v, &p, &FLAT, 1);
    assert!(v.hover.boosting);
    assert!(close(v.hover.energy, 100.0 - 20.0 / 30.0, 1e-4));
    v.input.boost = false;
    let mut ticks = 1;
    while v.hover.boosting {
        run(&mut v, &p, &FLAT, 1);
        ticks += 1;
        assert!(ticks < 200, "never ran dry");
        if ticks == 45 {
            let vf = (v.rot * Vec3::Z).dot(v.vel);
            assert!(close(vf, 40.0, 0.5), "1.5 s of boost: {vf} m/s");
        }
    }
    assert!((150..=152).contains(&ticks), "dry after {ticks} ticks");
    assert!(v.hover.drained);
    // Dry and unlatched: the next ticks refill 15 a second.
    let e0 = v.hover.energy;
    run(&mut v, &p, &FLAT, 30);
    assert!(!v.hover.boosting);
    assert!(close(v.hover.energy - e0, 15.0, 1e-3), "{}", v.hover.energy);

    for (throttle, human) in [(1.0, false), (0.85, true)] {
        let mut v = settled(&p);
        v.input = HoverInput {
            throttle,
            human,
            boost: true,
            ..HoverInput::default()
        };
        run(&mut v, &p, &FLAT, 10);
        assert!(!v.hover.boosting && v.hover.energy == ENERGY_MAX);
    }
}

/// A class with no BoostSpeed (the skiff; every BF1 class) never boosts.
#[test]
fn no_boost_speed_no_boost() {
    let p = skiff();
    let mut v = settled(&p);
    v.input = HoverInput {
        throttle: 1.0,
        human: true,
        boost: true,
        ..HoverInput::default()
    };
    run(&mut v, &p, &FLAT, 30);
    assert!(!v.hover.boosting && v.hover.energy == ENERGY_MAX);
}

/// §6.9 by hand, on `jump` alone: the start tick pushes nothing; at
/// ForwardSpeed (f = 1) the limit is JumpTimeMax 0.35 s, so the timer's
/// 1/30 … 10/30 push 10 ticks of 50/30 m/s along E's up and the 11th ends
/// it — every jumping tick pays 100/30 energy; half way from the start speed
/// (f = 0.5) the limit is 0.225 s, 6 pushes; past JumpTimeMin a released
/// button ends it; below the start speed or with g ≤ 0.9 it never starts.
#[test]
fn bike_jump_pushes_for_its_speed_blended_time() {
    let p = bike();
    let fwd = Vec3::Z;
    let pushes = |v_fwd: f32, release_at: Option<u32>| {
        let mut v = vehicle(Vec3::ZERO, 0.0);
        v.hover.g = 1.0;
        v.vel = fwd * v_fwd;
        v.input.jump = true;
        jump(&mut v, &p, fwd, DT);
        assert!(v.hover.jumping && v.vel.y == 0.0, "the start tick");
        let mut n = 0;
        let mut ticks = 0;
        while v.hover.jumping {
            if release_at == Some(ticks) {
                v.input.jump = false;
            }
            let y0 = v.vel.y;
            jump(&mut v, &p, fwd, DT);
            ticks += 1;
            if v.vel.y > y0 {
                assert!(close(v.vel.y - y0, 50.0 / 30.0, 1e-4));
                n += 1;
            }
            assert!(ticks < 30);
        }
        (n, ticks, v.hover.energy)
    };
    let (n, ticks, e) = pushes(16.0, None);
    assert_eq!((n, ticks), (10, 11));
    assert!(close(e, 100.0 - 11.0 * 100.0 / 30.0, 1e-3), "{e}");
    let start = 16.0 * 0.4;
    assert_eq!(pushes(start + 0.5 * (16.0 - start), None).0, 6);
    // Released with the timer at 4/30 > 0.1: that tick ends it.
    assert_eq!(pushes(16.0, Some(4)).0, 4);
    // Released before JumpTimeMin: it keeps pushing to 0.1.
    let (n, _, _) = pushes(start + 1e-3, Some(0));
    assert!((2..=3).contains(&n), "{n}");

    for (g, v_fwd) in [(1.0, start - 0.1), (0.9, 16.0)] {
        let mut v = vehicle(Vec3::ZERO, 0.0);
        v.hover.g = g;
        v.vel = fwd * v_fwd;
        v.input.jump = true;
        jump(&mut v, &p, fwd, DT);
        assert!(!v.hover.jumping);
    }
}

/// On the plane, the bike at speed jumps clear and lands again: the button
/// held through the whole jump (12 ticks), then released — held, it would
/// jump again on landing (g past 0.9 restarts it).
#[test]
fn bike_jump_leaves_the_ground_and_lands() {
    let p = bike();
    let mut v = settled(&p);
    v.input.throttle = 1.0;
    run(&mut v, &p, &FLAT, 150);
    let y0 = v.pos.y;
    v.input.jump = true;
    run(&mut v, &p, &FLAT, 12);
    assert!(!v.hover.jumping);
    v.input.jump = false;
    let mut peak = v.pos.y;
    for _ in 0..90 {
        run(&mut v, &p, &FLAT, 1);
        peak = peak.max(v.pos.y);
    }
    eprintln!(
        "jump: rest {y0:.3} m, peak {peak:.3} m, end {:.3} m",
        v.pos.y
    );
    assert!(peak - y0 > 1.0, "rose {} m", peak - y0);
    // Landed: on its springs again, still bobbing (lightly damped, §9).
    assert!(
        v.hover.g > 0.0 && close(v.pos.y, y0, 0.5),
        "at {} vs {y0}",
        v.pos.y
    );
}

/// A driven hover's aim ray starts on the chase camera's line (B2
/// B2:132440-132461). The BARC (`TrackCenter` 0 2.5 -6, no offset, eye 0 1 0)
/// casts from 2.5 m over its origin, not from its eye 1 m up; the IFT-X
/// (0 2 -5, offset 0 0.5 6, `TiltValue` 7) casts along the line through its
/// camera, at its eye's depth.
#[test]
fn a_driven_hovers_aim_ray_leaves_the_chase_camera() {
    let v = vehicle(Vec3::new(10.0, 3.0, -4.0), 0.0);
    let fwd = v.rot * Vec3::Z;
    let barc = ChaseSection {
        center: Vec3::new(0.0, 2.5, -6.0),
        offset: Vec3::ZERO,
        tilt: 5.0 * 0.017_453_294,
        mount: None,
    };
    let eye = v.pos + v.rot * Vec3::new(0.0, 1.0, 0.0);
    let o = barc.ray_origin(&v, 0.0, eye, fwd);
    assert!(o.distance(v.pos + Vec3::new(0.0, 2.5, 0.0)) < 1e-4, "{o}");
    let ift = ChaseSection {
        center: Vec3::new(0.0, 2.0, -5.0),
        offset: Vec3::new(0.0, 0.5, 6.0),
        tilt: 7.0 * 0.017_453_294,
        mount: None,
    };
    let o = ift.ray_origin(&v, 0.0, eye, fwd);
    let cam = ift.camera(&v, 0.0);
    let off = o - cam;
    assert!(off.cross(fwd).length() < 1e-4, "{off}");
    assert!((o - eye).dot(fwd).abs() < 1e-4);
    assert!(
        cam.y > o.y - 1e-4 && (cam - v.pos).dot(fwd) < -10.0,
        "{cam}"
    );
}
