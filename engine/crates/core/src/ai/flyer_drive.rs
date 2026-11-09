//! A bot at a flyer's pilot seat. The games differ (analysis/re-flyer-ai.md):
//!
//! - BF1 writes sticks. UnitFlyAgent (B1 FUN_00246d80, FUN_00246718) probes
//!   100 m up, escapes 200 m forward and 1 m up while that is blocked, then
//!   flies to a point 20 m over a command post, drawn at random, and draws
//!   again on arrival. FUN_001dcbf0 shapes the speed and lifts the goal
//!   (FUN_001dc700), FUN_00235c08 sidesteps an obstacle, and the shared steering
//!   turns that into throttle, strafe, turn and pitch.
//! - BF2 flies a Catmull-Rom curve along authored `EntityPath`s and writes the
//!   pose itself (B2 FUN_002fccd0, `flyer::spline_step`). This module plans
//!   the curves; FUN_002fd930 gives the pursuit curve's shape.
//!
//! Both take off through the flight state machine and both clear their guns
//! outside the flying state (`Sim::seat_gun`).
//!
//! OURS, PROVISIONAL, none of it read from the PS2 (`re-flyer-ai.md` has the numbers
//! each changed on real maps):
//! - the draws hash tick and unit, not the PS2's `rand()`, because a think
//!   reads the sim immutably;
//! - BF1's attack run ([`attack_goal`]): the original's attack order is not
//!   transcribed, so a bot turns on what it sees and breaks off 0.6 s short;
//! - BF2's pursuit trigger: any seen enemy in a flying flyer. The PS2's target
//!   source (+0xdc) is not transcribed, and FUN_002faf90 suggests a human's
//!   flyer is what it wants;
//! - BF1's arrival radius, MaxSpeed x 0.5 or passing the goal: RE_LOG's 4 m
//!   for "others" would orbit forever at flight speed;
//! - giving up a takeoff under a roof after [`TAKEOFF_GIVEUP`];
//! - a wedged craft ([`clearest`]): the hover's 2 m in 3 s window, then the
//!   longest of eight rays, since a hull's response is not read, held for
//!   [`WEDGED_HOLD`] against the upward probe;
//! - a pilot leaving a craft wedged [`WEDGED_GIVEUP`] times over;
//! - the pitch stick's error ([`stick_to`]), its slew ([`PITCH_SLEW`]) and the altitude
//!   law's slope limit ([`CLIMB_SLOPE`]): without them the bots porpoised;
//! - the connection chords' fallback when all four candidates are blocked;
//! - the obstacle top in the altitude sweep, hit point + the class radius.
//!
//! NOT BUILT: landing hints (agent states 5, 6), squadrons, transports, BF2
//! boost, avoidance, docking and landing, branching at a node, the path's
//! node properties, and the
//! roll wobble's address-derived phase (the vehicle's generation stands in).

mod admission;
pub(crate) use admission::commit as commit_plan;
pub use admission::{has_transport_paths, TRANSPORT_CLASS};

use super::gunner::Look;
use super::hover_drive::{self, Body, Class, Order};
use super::state::{AiState, FlyerOut, NavTick, RideState, SteerPick};
use crate::collision::{HullFilter, Layers};
use crate::sim::{PadState, Sim};
use crate::unit::UnitHandle;
use crate::vehicle::flyer::{FlightPhase, FlyAgent, FlyerAi, FlyerParams, PathCursor};
use crate::vehicle::{Mount, Vehicle, VehicleHandle, VehicleKind};
use glam::Vec3;

/// FUN_002470e0: the departure probe's reach (0x42c80000).
pub const UP_PROBE: f32 = 100.0;
/// FUN_00246718 state 2: the escape path's end.
pub const ESCAPE_FORWARD: f32 = 200.0;
pub const ESCAPE_UP: f32 = 1.0;
/// FUN_00246718 state 4: the travel point over a command post.
pub const POST_CLEARANCE: f32 = 20.0;
/// FUN_001dc700: the height held over the highest sample.
pub const TERRAIN_CLEARANCE: f32 = 25.0;
/// FUN_001dcbf0 / FUN_001dd0d0: speed floors, m/s.
pub const SLOW_FLOOR: f32 = 20.0;
pub const SPEED_FLOOR: f32 = 5.0;
/// FUN_00235c08: the sidestep's minimum reach and FUN_001dcbf0's cadence.
pub const AVOID_REACH: f32 = 30.0;
const PROBE_TICKS: u64 = 3;
const AVOID_EXPIRY: u64 = crate::TICK_HZ as u64;
/// FUN_002fd930: the pursuit point's offset around the target.
pub const PURSUIT_OFFSET: f32 = 30.0;
/// The 3.5 s after a pursuit ends (re-flyer-ai.md, FUN_002fccd0).
pub const PURSUIT_ESCAPE: f32 = 3.5;
/// FUN_002fe868's connection: P0 behind the craft (0x41a00000).
pub const CONNECT_BEHIND: f32 = 20.0;
/// FUN_002ff478: the choices a blocked connection may try.
pub const CONNECT_TRIES: usize = 4;
/// OURS: a takeoff that has not flown after this long gives up.
pub const TAKEOFF_GIVEUP: u64 = 15 * crate::TICK_HZ as u64;
/// OURS: an attack run's break-off, and how long it flies on before another.
pub const ATTACK_BREAK: u64 = 4 * crate::TICK_HZ as u64;
/// OURS: a craft that has not moved 2 m in 3 s looks for a way out, as the
/// hover's path follower counts it stuck (`hover_drive::stuck`).
pub const WEDGED_MOVE: f32 = hover_drive::STUCK_MOVE;
pub const WEDGED_TICKS: u64 = (hover_drive::STUCK_STILL * crate::TICK_HZ as f32) as u64;
/// OURS: the ray's reach in that search.
pub const WEDGED_RAY: f32 = 200.0;
/// OURS: how long a way out holds against the upward probe.
pub const WEDGED_HOLD: u64 = 6 * crate::TICK_HZ as u64;
/// OURS: how many ways out a craft tries before its pilot leaves it.
pub const WEDGED_GIVEUP: u8 = 3;
/// OURS: an attack run dips no lower than this over the ground ahead.
pub const ATTACK_FLOOR: f32 = 10.0;
/// OURS: the steepest slope the altitude law asks for in cruise (about 20 degrees),
/// within [`GROUND_DANGER`] of the ground ahead (about 45), and that margin in metres.
pub const CLIMB_SLOPE: f32 = 0.36;
pub const CLIMB_SLOPE_LOW: f32 = 1.0;
pub const GROUND_DANGER: f32 = 8.0;
/// OURS: how fast a bot pilot's pitch stick moves, in full deflections a second.
pub const PITCH_SLEW: f32 = 2.5;
const FREE_BURST_SALT: u32 = 0xF12E;
const FREE_BURST_FIRST: f32 = 10.0;
const FREE_BURST_WINDOW: f32 = 0.6;
const FREE_BURST_DONE: f32 = 0.0001;
const FREE_BURST_SPREAD: f32 = 1.5;
const FREE_BURST_MEAN: f32 = 6.0;
const FREE_BURST_THINK: u64 = 120;
const FREE_BURST_THINK_STEP: u64 = 37;
/// OURS: how long an attack run holds a target it no longer sees.
pub const ATTACK_HOLD: u64 = 2 * crate::TICK_HZ as u64;
/// OURS: an attack run's steepest climb or dive, rise over run (about 20 degrees).
pub const ATTACK_SLOPE: f32 = 0.36;

/// Can a bot fly this vehicle? BF2's bots follow authored paths, so a BF2 map
/// without any has nothing for them to fly.
pub fn can_fly(sim: &Sim, v: &Vehicle) -> bool {
    sim.flyer_bots
        && v.kind == VehicleKind::Flyer
        && sim
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .is_some_and(|p| {
                !p.bf2
                    || sim.ctf.is_some()
                    || sim.flyer_paths.iter().any(|path| {
                        p.follower.class_id == 0
                            || path.classes.is_empty()
                            || path
                                .classes
                                .iter()
                                .any(|(id, _)| *id == p.follower.class_id)
                            || (p.follower.class_id == admission::TRANSPORT_CLASS
                                && !admission::has_transport_paths(sim))
                    })
            })
}

/// A deterministic draw in 0..1; the PS2 uses `rand()` (see the module note).
pub(crate) fn draw(tick: u64, me: UnitHandle, salt: u32) -> f32 {
    let mut x = (tick as u32) ^ me.index.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// FUN_00235dc8: a speed as a share of MaxSpeed.
fn normalized(speed: f32, max: f32) -> f32 {
    (speed / max.max(f32::MIN_POSITIVE)).clamp(0.0, 1.0)
}

/// FUN_00235e00: the throttle for a share of MaxSpeed.
pub fn throttle_for(share: f32, p: &FlyerParams) -> f32 {
    let neutral = if p.min_speed > 0.0 {
        -1.0
    } else if p.mid_speed > 0.0 {
        p.min_speed / (p.mid_speed - p.min_speed)
    } else if p.max_speed > 0.0 {
        -p.mid_speed / (p.max_speed - p.mid_speed)
    } else {
        1.0
    };
    if share < 0.0 {
        share * (neutral + 1.0) - 1.0
    } else {
        neutral + share * (1.0 - neutral)
    }
}

/// FUN_001dcbf0's speed share, for a mover asking for 1.0.
pub fn speed_share(to_goal: Vec3, forward: Vec3, p: &FlyerParams) -> f32 {
    let distance = to_goal.length();
    let direction = to_goal / distance.max(f32::MIN_POSITIVE);
    let aligned = direction.dot(forward);
    let mut share = (aligned.max(0.9) - 0.9) * 9.999998;
    let max = p.max_speed;
    let reach = max * 3.0;
    let near = if distance >= max {
        distance.min(reach)
    } else {
        max
    };
    let blend = (near - max) / (reach - max).max(f32::MIN_POSITIVE) * 0.7 + 0.3;
    // UNCONFIRMED: the order's forward vector is taken as the goal direction.
    share *= blend + (1.0 - blend) * (2.0 * aligned.max(0.5) - 1.0);
    let floor = normalized(SLOW_FLOOR, max);
    if share < 1.0 {
        share = share.max(floor);
    }
    share.max(normalized(SPEED_FLOOR, max))
}

/// BF1's flyer form of FUN_00232b78: yaw from the goal in the full body frame, each
/// axis a signed square of a clamped gain.
pub fn stick_to(p: &FlyerParams, m: &Body, goal: Vec3) -> (f32, f32) {
    let up = m.fwd.cross(m.left);
    let flat = Vec3::new(goal.x - m.pos.x, 0.0, goal.z - m.pos.z);
    let local = Vec3::new(flat.dot(m.left), flat.dot(up), flat.dot(m.fwd));
    let local = local.try_normalize().unwrap_or(Vec3::Z);
    let mut s = -local.x;
    if local.z < 0.0 {
        s = if s < 0.0 { -1.0 } else { 1.0 };
    }
    let turn = hover_drive::shaped(s, 4.5 - p.turn_rate.clamp(1.0, 3.0));
    // OURS: the pitch error is the goal's elevation against the nose's, whatever the
    // goal's bearing. FUN_00232b78 takes the goal's height in the nose's own frame: the
    // same dead ahead, but while a goal abeam or behind is being turned to it reads
    // near zero, or the wrong way on a raised nose, which then keeps climbing or diving
    // through the whole turn. That was `bf1_kas1`'s porpoising.
    let to = goal - m.aim_origin;
    let wanted = crate::fmath::atan2(to.y, Vec3::new(to.x, 0.0, to.z).length());
    let nose = crate::fmath::atan2(
        m.aim_dir.y,
        Vec3::new(m.aim_dir.x, 0.0, m.aim_dir.z).length(),
    );
    let error = crate::fmath::sin_cos(wanted - nose).0;
    let pitch = hover_drive::shaped(error, 5.0 - p.pitch_rate.clamp(1.0, 3.0));
    (turn, pitch)
}

fn ground(sim: &Sim, x: f32, z: f32, floor: f32) -> f32 {
    sim.heightfield
        .as_ref()
        .map_or(floor, |hf| hf.height_at(x, z).max(floor))
}

/// FUN_001dc700: lift `goal` to clear the ground ahead. Five samples of the
/// terrain and a sweep along the nose; the goal rises in proportion to its
/// distance, and relaxes to its own height as the craft arrives. Returns the
/// lifted goal and the speed share an obstacle ahead asks for.
fn lift_goal(
    sim: &Sim,
    v: &Vehicle,
    handle: VehicleHandle,
    p: &FlyerParams,
    goal: Vec3,
) -> (Vec3, Option<f32>) {
    let pos = v.pos;
    let [floor, ceiling] = sim.flight_limits.ai;
    let reach = (v.vel.length() * 1.5).max(1.0);
    let forward = v.rot * Vec3::Z;
    let sample = |angle: f32, length: f32| {
        let (sin, cos) = crate::fmath::sin_cos(angle);
        let dir = Vec3::new(
            forward.x * cos - forward.z * sin,
            forward.y,
            forward.x * sin + forward.z * cos,
        );
        let at = pos + dir * length;
        ground(sim, at.x, at.z, floor)
    };
    let quarter = core::f32::consts::FRAC_PI_4;
    let half = core::f32::consts::FRAC_PI_2;
    let mut high = sample(0.0, reach)
        .max(sample(quarter, reach * 0.7))
        .max(sample(-quarter, reach * 0.7))
        .max(sample(half, reach * 0.5))
        .max(sample(-half, reach * 0.5));
    let radius = sim
        .seat_setup
        .get(v.class as usize)
        .and_then(Option::as_ref)
        .map_or(0.0, |s| s.radius);
    let mut slow = None;
    let origin = pos - Vec3::Y * (radius * 1.5);
    if let Some(hit) = sim.collision.ray_with(
        origin,
        forward,
        reach,
        Layers::FLYER,
        HullFilter::except(Some(handle)),
    ) {
        let top = (origin.y + forward.y * hit.t) + radius;
        let rise = top - pos.y;
        let fraction = (hit.t / reach).max(f32::MIN_POSITIVE);
        high = high.max(if rise > 0.0 {
            rise / fraction + pos.y
        } else {
            top
        });
        slow = Some(normalized(SLOW_FLOOR, p.max_speed));
    }
    high = high.max(ground(sim, pos.x, pos.z, floor));
    let flat = Vec3::new(goal.x - pos.x, 0.0, goal.z - pos.z).length();
    let raised = pos.y + flat * (high + TERRAIN_CLEARANCE - pos.y) / reach;
    let arrival = {
        let edge = (2.0 * v.vel.length() / p.turn_rate.max(f32::MIN_POSITIVE)).max(2.0);
        let distance = (goal - pos).length();
        let d = if distance >= 1.0 {
            distance.min(edge)
        } else {
            1.0
        };
        (d - 1.0) / (edge - 1.0).max(f32::MIN_POSITIVE)
    };
    let lifted = goal.y + (raised - goal.y) * arrival;
    let mut out = goal;
    if lifted > out.y {
        out.y = lifted;
    }
    out.y = out.y.min(ceiling);
    // OURS: the law above asks for its whole rise over the look-ahead L, so a slow
    // craft or one a few metres low aims at the ceiling, overshoots by tens of metres,
    // and dives back. Height is taken at a steady slope unless the ground is close.
    let danger = pos.y < high + GROUND_DANGER;
    let band = flat.max(reach) * if danger { CLIMB_SLOPE_LOW } else { CLIMB_SLOPE };
    out.y = out.y.clamp(pos.y - band, pos.y + band);
    (out, slow)
}

/// A new point over a command post (FUN_00246718 state 4: ordinary pilots
/// take the first draw, whoever holds the post). `None` with no posts.
fn post_point(sim: &Sim, me: UnitHandle, salt: u32) -> Option<Vec3> {
    if sim.cps.is_empty() {
        return None;
    }
    let n = sim.cps.len();
    let k = ((draw(sim.tick, me, salt) * n as f32) as usize).min(n - 1);
    Some(sim.cps[k].def.pos + Vec3::Y * POST_CLEARANCE)
}

fn ctf_goal(sim: &Sim, me: UnitHandle) -> Option<Vec3> {
    let ctf = sim.ctf.as_ref()?;
    let my_team = sim.unit(me).map_or(0, |u| u.team);
    for flag in &ctf.flags {
        match flag.status {
            crate::sim::FlagStatus::Carried { carrier } => {
                if carrier == me {
                    return ctf.capture_regions.get(my_team as usize).map(|r| r.pos);
                }
            }
            crate::sim::FlagStatus::AtHome | crate::sim::FlagStatus::Dropped { .. } => {
                return Some(flag.current_pos);
            }
        }
    }
    None
}

fn ctf_curve(v: &Vehicle, goal: Vec3) -> [Vec3; 4] {
    let forward = v.rot * Vec3::Z;
    let to = goal - v.pos;
    let dist = to.length();
    let step = dist.min(200.0);
    let dir = if dist > 1e-3 { to / dist } else { forward };
    let mid = v.pos + dir * (step * 0.5);
    let end = v.pos + dir * step;
    [v.pos - forward * CONNECT_BEHIND, v.pos, mid, end]
}

/// OURS: the goal of an attack run, or `None` to fly on. A run dips below the
/// travel height, keeps [`ATTACK_FLOOR`] over the ground ahead, and ends at
/// the reach the altitude law looks ahead, or the class's turning room. It holds
/// its last sighting for [`ATTACK_HOLD`], and climbs or dives no steeper than
/// [`ATTACK_SLOPE`]: a sight that came and went within a second, or a target right
/// under the nose, otherwise threw the stick from stop to stop (`bf1_kas1`).
fn attack_goal(sim: &Sim, v: &Vehicle, ai: &mut FlyerAi, seen: Option<Vec3>) -> Option<Vec3> {
    if sim.tick < ai.break_until {
        ai.run = None;
        return None;
    }
    if let Some(point) = seen {
        ai.run = Some(point);
        ai.run_seen = sim.tick;
    } else if sim.tick >= ai.run_seen + ATTACK_HOLD {
        ai.run = None;
    }
    let point = ai.run?;
    let reach = (v.vel.length() * 0.6).max(25.0);
    let flat = Vec3::new(point.x - v.pos.x, 0.0, point.z - v.pos.z).length();
    // Past the turning room, however far below: a target under the nose is a dive.
    if flat < reach {
        ai.break_until = sim.tick + ATTACK_BREAK;
        ai.run = None;
        return None;
    }
    let ahead = v.pos + v.rot * Vec3::Z * reach;
    let floor = ground(sim, ahead.x, ahead.z, sim.flight_limits.ai[0]) + ATTACK_FLOOR;
    let band = flat * ATTACK_SLOPE;
    let y = point.y.max(floor).clamp(v.pos.y - band, v.pos.y + band);
    Some(Vec3::new(point.x, y, point.z))
}

/// FUN_00235c08: an obstacle within a second's flight ahead: a goal to the
/// side of the nose, kept until passed or a second old (FUN_001dcb50, FUN_001dcbf0).
fn avoid_goal(
    sim: &Sim,
    v: &Vehicle,
    handle: VehicleHandle,
    p: &FlyerParams,
    ai: &mut FlyerAi,
) -> Option<Vec3> {
    if sim.tick > ai.probe_at + AVOID_EXPIRY {
        ai.avoid = None;
    }
    if let Some([goal, from]) = ai.avoid {
        let leg = Vec3::new(goal.x - from.x, 0.0, goal.z - from.z);
        let past = Vec3::new(v.pos.x - goal.x, 0.0, v.pos.z - goal.z);
        if leg.dot(past) > 0.0 {
            ai.avoid = None;
        }
    }
    let forward = v.rot * Vec3::Z;
    let flat = Vec3::new(forward.x, 0.0, forward.z)
        .try_normalize()
        .unwrap_or(Vec3::Z);
    if ai.avoid.is_none() && sim.tick >= ai.probe_at {
        ai.probe_at = sim.tick + PROBE_TICKS;
        let blocked = sim
            .collision
            .ray_with(
                v.pos,
                flat,
                p.max_speed,
                Layers::FLYER,
                HullFilter::except(Some(handle)),
            )
            .is_some();
        if blocked {
            let side = flat.cross(Vec3::Y) * AVOID_REACH.max(p.max_speed * 2.0);
            ai.avoid = Some([v.pos + side, v.pos]);
        }
    }
    ai.avoid.map(|a| a[0])
}

/// OURS: the horizontal direction with the longest clear ray, with the nose's
/// own last so a craft nosed into a wall turns off it. Ties go to the first.
fn clearest(sim: &Sim, v: &Vehicle, handle: VehicleHandle) -> Vec3 {
    let forward = v.rot * Vec3::Z;
    let flat = Vec3::new(forward.x, 0.0, forward.z)
        .try_normalize()
        .unwrap_or(Vec3::Z);
    let mut best = (-1.0f32, flat);
    for k in 1..=8 {
        let (sin, cos) = crate::fmath::sin_cos(k as f32 * core::f32::consts::FRAC_PI_4);
        let dir = Vec3::new(
            flat.x * cos - flat.z * sin,
            0.0,
            flat.x * sin + flat.z * cos,
        );
        let reach = sim
            .collision
            .ray_with(
                v.pos,
                dir,
                WEDGED_RAY,
                Layers::FLYER,
                HullFilter::except(Some(handle)),
            )
            .map_or(WEDGED_RAY, |h| h.t);
        let reach = sim.terrain_ray(v.pos, dir, reach, 2.0).unwrap_or(reach);
        if reach > best.0 {
            best = (reach, dir);
        }
    }
    best.1
}

fn up_blocked(sim: &Sim, v: &Vehicle, handle: VehicleHandle) -> bool {
    sim.collision
        .ray_with(
            v.pos,
            Vec3::Y,
            UP_PROBE,
            Layers::FLYER,
            HullFilter::except(Some(handle)),
        )
        .is_some()
}

/// BF1's agent and steering: the four sticks.
fn bf1_sticks(
    sim: &Sim,
    me: UnitHandle,
    v: &Vehicle,
    handle: VehicleHandle,
    p: &FlyerParams,
    ai: &mut FlyerAi,
    aim: Option<Vec3>,
) -> [f32; 4] {
    let pos = v.pos;
    let forward = v.rot * Vec3::Z;
    if ai.agent != FlyAgent::Travel && sim.tick >= ai.wedged_until {
        if up_blocked(sim, v, handle) {
            if ai.agent == FlyAgent::Start {
                let flat = Vec3::new(forward.x, 0.0, forward.z)
                    .try_normalize()
                    .unwrap_or(Vec3::Z);
                ai.goal = Some(pos + flat * ESCAPE_FORWARD + Vec3::Y * ESCAPE_UP);
                ai.agent = FlyAgent::Escape;
            }
        } else {
            ai.goal = None;
            ai.agent = FlyAgent::Travel;
        }
    }
    if pos.distance(ai.anchor) > WEDGED_MOVE {
        ai.anchor = pos;
        ai.anchor_at = sim.tick;
    } else if sim.tick >= ai.anchor_at + WEDGED_TICKS && v.flyer.phase == FlightPhase::Flying {
        ai.anchor_at = sim.tick;
        ai.wedged_until = sim.tick + WEDGED_HOLD;
        ai.wedges = ai.wedges.saturating_add(1);
        let out = pos + clearest(sim, v, handle) * ESCAPE_FORWARD + Vec3::Y * ESCAPE_UP;
        ai.goal = Some(out);
        ai.agent = FlyAgent::Escape;
    }
    let arrive = p.max_speed * 0.5;
    if ai.agent == FlyAgent::Escape && ai.goal.is_some_and(|g| (g - pos).length() <= arrive) {
        // OURS: still under cover at the path's end: another 200 m on.
        let flat = Vec3::new(forward.x, 0.0, forward.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        ai.goal = Some(pos + flat * ESCAPE_FORWARD + Vec3::Y * ESCAPE_UP);
    }
    if let Some(goal) = ai.goal.filter(|_| ai.agent == FlyAgent::Travel) {
        ai.wedges = 0;
        let to = goal - pos;
        let flat = Vec3::new(to.x, 0.0, to.z);
        if to.length() <= arrive || (flat.length() <= arrive * 2.0 && flat.dot(forward) < 0.0) {
            ai.goal = None;
        }
    }
    if ai.goal.is_none() {
        ai.goal = ctf_goal(sim, me)
            .or_else(|| post_point(sim, me, 1))
            .or(Some(pos + forward * ESCAPE_FORWARD));
    }
    let travel = ai.goal.unwrap_or(pos + forward * ESCAPE_FORWARD);
    // UNCONFIRMED: an explicit escape path takes no altitude lift or sidestep:
    // over a hangar's floor, the terrain above it would hold the nose on the
    // ceiling. FUN_001dcbf0's gate on the order's type is not fully read.
    let (goal, slow) = if ai.agent == FlyAgent::Escape {
        (travel, None)
    } else {
        let run = attack_goal(sim, v, ai, aim);
        let avoid = avoid_goal(sim, v, handle, p, ai);
        match (avoid, run) {
            (Some(side), _) => lift_goal(sim, v, handle, p, side),
            (None, Some(point)) => (point, None),
            (None, None) => lift_goal(sim, v, handle, p, travel),
        }
    };
    // Yaw reads the transform, tilt and all; pitch reads the stored aim, which
    // has the tilt taken out (FUN_00232b78, FUN_001dbbd8). Pitching on the tilted
    // nose feeds the bank's own pitch back into the stick.
    let body = Body {
        pos,
        left: v.rot * Vec3::X,
        fwd: forward,
        vel: v.vel,
        aim_origin: pos,
        aim_dir: crate::vehicle::flyer::untilted(v) * Vec3::Z,
    };
    let (turn, pitch) = stick_to(p, &body, goal);
    // OURS: a hand on the stick. Stop to stop takes most of a second.
    let slew = PITCH_SLEW * crate::TICK_DT;
    let pitch = v.input.pitch + (pitch - v.input.pitch).clamp(-slew, slew);
    let k = Class {
        forward_speed: p.max_speed,
        strafe_speed: p.strafe_speed,
        turn_rate: p.turn_rate,
        pitch_rate: p.pitch_rate,
        moving_turn_only: false,
        bf1: true,
    };
    // A travel point is passed through, not stopped at: path = true.
    let order = Order {
        goal: Some(goal),
        path: true,
        bit4: false,
        stand: false,
        probe: 0,
        stuck: false,
        since_end: f32::MAX,
        since_state2: 0.0,
        held: [v.input.throttle, v.input.strafe],
    };
    let [ahead, strafe, _, _] = hover_drive::steer(&k, &body, &order, Some(goal));
    let share = speed_share(goal - pos, forward, p);
    let share = slow.map_or(share, |s| share.min(s));
    let throttle = throttle_for(share, p);
    [ahead * throttle, strafe * throttle, turn, pitch]
}

pub(crate) fn clear_chord(sim: &Sim, a: Vec3, b: Vec3, handle: VehicleHandle) -> bool {
    let delta = b - a;
    let length = delta.length();
    if length < 1e-3 {
        return true;
    }
    let dir = delta / length;
    sim.collision
        .ray_with(
            a,
            dir,
            length,
            Layers::FLYER,
            HullFilter::except(Some(handle)),
        )
        .is_none()
        && sim.terrain_ray(a, dir, length, 2.0).is_none()
}

/// The four sampled points of `curve` at 0, 1/3, 2/3 and 1 joined by clear
/// chords (`FUN_002fff48` checks three chords at the parameter's thirds).
fn curve_clear(sim: &Sim, curve: &[Vec3; 4], handle: VehicleHandle) -> bool {
    let points =
        [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0].map(|t| crate::vehicle::flyer::curve_at(curve, t).0);
    points
        .windows(2)
        .all(|w| clear_chord(sim, w[0], w[1], handle))
}

/// FUN_002ff478 and FUN_002f9498: the best path end to join, as a connection
/// curve. Frequency weights the score; MaxAngle is still unimplemented. FUN_002fff48 checks three chords at the parameter's thirds.
fn connect(
    sim: &Sim,
    v: &Vehicle,
    handle: VehicleHandle,
    p: &FlyerParams,
    ai: &mut FlyerAi,
    leave: Option<u16>,
) {
    let pos = v.pos;
    let forward = v.rot * Vec3::Z;
    let mut ranked: Vec<(f32, PathCursor, [Vec3; 4])> = Vec::new();
    for (i, path) in sim.flyer_paths.iter().enumerate() {
        if leave == Some(i as u16) || path.nodes.len() < 2 {
            continue;
        }
        let last = path.nodes.len() - 1;
        for (node, next, reverse) in [(0, 1, false), (last, last - 1, true)] {
            let cursor = PathCursor {
                path: i as u16,
                node: node as u16,
                reverse,
            };
            // Class-zero discovery is broader than joining on PS2. Rank
            // only candidates that can actually reserve a slot.
            if !admission::allows(sim, handle, p.follower.class_id, cursor, true) {
                continue;
            }
            let end = path.nodes[node];
            let after = path.nodes[next];
            let to = end - pos;
            let direction = to.try_normalize().unwrap_or(forward);
            let along = (after - end).try_normalize().unwrap_or(direction);
            let nose = (direction.dot(forward) + 1.0) * 0.5 + 0.0001;
            let leg = (direction.dot(along) + 1.0) * 0.5 + 0.0001;
            let mut score = nose * leg;
            if to.length() < p.follower.min_branch_dist {
                score *= 0.4;
            }
            let curve = [pos - forward * CONNECT_BEHIND, pos, end, after];
            ranked.push(((score * path.frequency).clamp(0.0, 1.0), cursor, curve));
        }
    }
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let chosen = ranked
        .iter()
        .take(CONNECT_TRIES)
        .find(|c| curve_clear(sim, &c.2, handle));
    if let Some((_, cursor, curve)) = chosen {
        ai.curve = Some(*curve);
        ai.cursor = Some(*cursor);
        ai.goal = None;
    } else {
        loiter(v, p, ai);
        if !ai
            .curve
            .as_ref()
            .is_some_and(|c| curve_clear(sim, c, handle))
        {
            let esc_dir = clearest(sim, v, handle);
            let curve = [
                pos - forward * CONNECT_BEHIND,
                pos,
                pos + esc_dir * 40.0,
                pos + esc_dir * 80.0,
            ];
            if clear_chord(sim, pos, pos + esc_dir * WEDGED_RAY, handle)
                && curve_clear(sim, &curve, handle)
            {
                ai.curve = Some(curve);
                ai.cursor = None;
                ai.goal = None;
            } else {
                ai.curve = None;
                ai.goal = None;
            }
        }
    }
    ai.t = 0.0;
}

/// OURS: hold near the failed connection without occupying a refused route.
/// measure: PS2 all-routes-refused trajectory after FUN_002fa478 resets the
/// follower; replace this circle and its two seconds of mid-speed radius.
fn loiter(v: &Vehicle, p: &FlyerParams, ai: &mut FlyerAi) {
    ai.curve = Some(if let (Some(center), Some(old)) = (ai.goal, ai.curve) {
        [old[1], old[2], old[3], center * 2.0 - old[2]]
    } else {
        let forward = (v.rot * Vec3::Z * Vec3::new(1.0, 0.0, 1.0))
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let right = Vec3::Y.cross(forward);
        let radius = p.mid_speed.max(SLOW_FLOOR) * 2.0;
        let center = v.pos + right * radius;
        ai.goal = Some(center);
        [
            v.pos - forward * radius,
            v.pos,
            center + forward * radius,
            center + right * radius,
        ]
    });
    ai.cursor = None;
    ai.t = 0.0;
}

/// FUN_002fd930: a curve from here to a point near the target, led by half
/// the distance along its nose, offset 30 m in its right/up plane.
fn pursuit_curve(sim: &Sim, me: UnitHandle, v: &Vehicle, target: &Vehicle) -> [Vec3; 4] {
    let forward = v.rot * Vec3::Z;
    let a = (draw(sim.tick, me, 2) - 0.5) * 2.0;
    let b = (draw(sim.tick, me, 3) - 0.5) * 2.0;
    let off = (target.rot * Vec3::X * a + target.rot * Vec3::Y * b)
        .try_normalize()
        .unwrap_or(Vec3::Y);
    let distance = (v.pos - target.pos).length();
    let point = target.pos + off * PURSUIT_OFFSET + target.rot * Vec3::Z * (distance * 0.5);
    [v.pos - forward * CONNECT_BEHIND, v.pos, point, point]
}

/// BF2's planner: the curve the kernel flies next.
fn bf2_plan(
    sim: &Sim,
    me: UnitHandle,
    v: &Vehicle,
    handle: VehicleHandle,
    p: &FlyerParams,
    ai: &mut FlyerAi,
    target: Option<&Vehicle>,
) {
    if let Some(t) = target.filter(|_| p.follower.flyer_vs_flyer && !ai.pursuit && ai.escape <= 0.0)
    {
        ai.goal = None;
        ai.curve = Some(pursuit_curve(sim, me, v, t));
        ai.cursor = None;
        ai.t = 0.0;
        ai.pursuit = true;
        return;
    }
    if let Some(goal) = ctf_goal(sim, me) {
        let to = goal - v.pos;
        if to.length_squared() > 15.0 * 15.0
            && (ai.curve.is_none() || ai.t >= 1.0 || ai.goal != Some(goal))
        {
            // A hangar wall between the pad and the flag does not make the
            // direct hop flyable: only take it when every sampled chord is
            // clear, else the authored route plans the way out (spa1).
            let curve = ctf_curve(v, goal);
            if curve_clear(sim, &curve, handle) {
                ai.goal = Some(goal);
                ai.curve = Some(curve);
                ai.cursor = None;
                ai.t = 0.0;
                return;
            }
        }
    }
    if ai.curve.is_some() && ai.t < 1.0 {
        return;
    }
    if ai.pursuit {
        ai.pursuit = false;
        ai.escape = PURSUIT_ESCAPE;
    }
    let mut leave = None;
    if let (Some(c), Some(curve)) = (ai.cursor, ai.curve) {
        let nodes = &sim.flyer_paths[c.path as usize].nodes;
        let step = |n: u16| -> Option<u16> {
            if c.reverse {
                n.checked_sub(1)
            } else {
                Some(n + 1).filter(|&n| usize::from(n) < nodes.len())
            }
        };
        if let Some(next) = step(c.node) {
            let after = step(next).map_or(nodes[usize::from(next)], |n| nodes[usize::from(n)]);
            ai.curve = Some([curve[1], curve[2], nodes[usize::from(next)], after]);
            ai.cursor = Some(PathCursor { node: next, ..c });
            ai.t = 0.0;
            return;
        }
        leave = Some(c.path);
    }
    if sim.flyer_seek_curve(me, v, handle, p, ai) {
        return;
    }
    connect(sim, v, handle, p, ai, leave);
}

/// The enemy flyer this bot's perception sees now, if any.
fn free_burst(sim: &Sim, me: UnitHandle, p: &FlyerParams, ai: &mut FlyerAi) -> bool {
    if !p.follower.flyer_vs_flyer {
        return false;
    }
    if (sim.tick + u64::from(me.index) * FREE_BURST_THINK_STEP).is_multiple_of(FREE_BURST_THINK) {
        ai.fire_held = false;
    }
    let held = if ai.burst_in > 0.0 {
        ai.burst_in
    } else {
        FREE_BURST_FIRST
    };
    let left = held - crate::TICK_DT;
    ai.burst_in = if left <= FREE_BURST_DONE {
        (draw(sim.tick, me, FREE_BURST_SALT) - 0.5) * FREE_BURST_SPREAD + FREE_BURST_MEAN
    } else {
        left
    };
    ai.fire_held |= left < FREE_BURST_WINDOW;
    ai.fire_held
}

fn seen_flyer_handle(sim: &Sim, st: &AiState) -> Option<crate::vehicle::VehicleHandle> {
    seen_flyer(sim, st)?;
    let contact = st
        .target
        .and_then(|h| crate::perception::contact_of(st, h))?;
    Some(contact.handle.live_unit(sim)?.mount?.vehicle)
}

fn seen_flyer<'a>(sim: &'a Sim, st: &AiState) -> Option<&'a Vehicle> {
    let contact = st
        .target
        .and_then(|h| crate::perception::contact_of(st, h))
        .filter(|c| c.seen_now)?;
    let enemy = contact.handle.live_unit(sim)?;
    sim.vehicle(enemy.mount?.vehicle)
        .filter(|v| v.kind == VehicleKind::Flyer && v.flyer.phase == FlightPhase::Flying)
}

fn side_roll_wanted(
    sim: &Sim,
    v: &Vehicle,
    ai: &crate::vehicle::flyer::FlyerAi,
    foe: Option<crate::vehicle::VehicleHandle>,
) -> Option<Vec3> {
    if v.flyer.in_roll || sim.ground_flyer_map {
        return None;
    }
    let target = super::flyer_weapons::armed_point(sim, ai)
        .or_else(|| foe.and_then(|f| sim.vehicle(f)).map(|t| t.pos))?;
    let delta = target - v.pos;
    let far = delta.length_squared();
    if far >= SIDE_ROLL_REACH * SIDE_ROLL_REACH {
        return None;
    }
    let nose = v.rot * Vec3::Z;
    if far >= SIDE_ROLL_CLOSE * SIDE_ROLL_CLOSE
        && nose.dot(delta.normalize_or_zero()) >= SIDE_ROLL_CONE
    {
        return None;
    }
    let side = delta - nose * nose.dot(delta);
    Some(side.normalize_or_zero())
}

const SIDE_ROLL_REACH: f32 = 200.0;
const SIDE_ROLL_CLOSE: f32 = 40.0;
const SIDE_ROLL_CONE: f32 = 0.965925;

fn is_transport_at_destination(sim: &Sim, v: &Vehicle, p: &FlyerParams) -> bool {
    p.follower.class_id == admission::TRANSPORT_CLASS
        && sim.landing_regions.iter().any(|r| r.contains(v.pos))
        && v.spawn_pad
            .and_then(|idx| sim.vehicle_pads.get(idx as usize))
            .is_none_or(|pad| (v.pos - pad.def.pos).length_squared() > 150.0 * 150.0)
}

pub(super) fn drive(
    sim: &Sim,
    me: UnitHandle,
    m: Mount,
    st: &mut AiState,
    nt: NavTick,
) -> (PadState, NavTick) {
    let mut pad = PadState::default();
    let Some(v) = sim.vehicle(m.vehicle) else {
        return (pad, nt);
    };
    let Some(p) = sim
        .flyer_params
        .get(v.class as usize)
        .and_then(Option::as_ref)
    else {
        return (pad, nt);
    };
    st.steer = SteerPick::Fwd;
    let space = p.bf2 && sim.space_assault.is_some();
    if space {
        st.target = None;
    }
    let aim = super::gunner::aim(sim, me, st, &mut pad, Look::Steering, None);
    let mut ai = v.flyer.ai;
    let mut out = [0.0; 4];
    let mut takeoff = false;
    let mut free_fire = false;
    // A pilot who came to a craft already rising (a gunner moved up after the
    // pilot died) has no landed tick: his clock starts now, not at tick 0.
    if v.flyer.phase == FlightPhase::Takeoff && ai.takeoff_at == 0 {
        ai.takeoff_at = sim.tick;
    }
    match v.flyer.phase {
        FlightPhase::Landed => {
            if is_transport_at_destination(sim, v, p) {
                super::ride::request_exit(st);
                return (pad, nt);
            }
            // FUN_002518d0: travelling while not flying requests takeoff.
            takeoff = !v.flyer.jump_was_down;
            ai.takeoff_at = sim.tick;
        }
        FlightPhase::Landing => {}
        FlightPhase::Takeoff if sim.tick >= ai.takeoff_at + TAKEOFF_GIVEUP => {
            super::ride::request_exit(st);
            return (pad, nt);
        }
        // BF2 runs its AI in the flying state only (B2:117411-117417), and
        // BF1 skips only states 0 and 3 (FUN_001dd0d0).
        _ if p.bf2 => {
            if v.flyer.phase == FlightPhase::Flying {
                let pos = v.pos;
                if pos.distance(ai.anchor) > WEDGED_MOVE {
                    ai.anchor = pos;
                    ai.anchor_at = sim.tick;
                } else if sim.tick >= ai.anchor_at + WEDGED_TICKS {
                    ai.anchor_at = sim.tick;
                    ai.wedged_until = sim.tick + WEDGED_HOLD;
                    ai.wedges = ai.wedges.saturating_add(1);
                }
                if is_transport_at_destination(sim, v, p) {
                    takeoff = true;
                }
                let pilot = super::flyer_weapons::Pilot {
                    me,
                    vehicle: v,
                    handle: m.vehicle,
                    seat: usize::from(m.seat),
                };
                let foe = if space {
                    super::flyer_weapons::follow_threats(sim, &pilot, &mut ai, &mut st.burst);
                    None
                } else {
                    let foe = seen_flyer_handle(sim, st);
                    super::flyer_weapons::think(sim, &pilot, &mut ai, foe, &mut st.burst);
                    foe
                };
                free_fire = free_burst(sim, me, p, &mut ai);
                if super::flyer_weapons::armed_shot(sim, me, &ai)
                    && !super::gunner::friend_on_nose(sim, me)
                {
                    pad.buttons |= crate::sim::buttons::FIRE;
                }
                ai.trick = side_roll_wanted(sim, v, &ai, foe);
                if !v.flyer.in_roll {
                    let chase = if space {
                        ai.armed_target.and_then(|t| sim.vehicle(t)).filter(|t| {
                            t.kind == VehicleKind::Flyer && t.flyer.phase == FlightPhase::Flying
                        })
                    } else {
                        seen_flyer(sim, st)
                    };
                    bf2_plan(sim, me, v, m.vehicle, p, &mut ai, chase);
                }
            }
        }
        _ => out = bf1_sticks(sim, me, v, m.vehicle, p, &mut ai, aim),
    }
    if free_fire {
        super::gunner::free_fire(sim, me, st, &mut pad);
    }
    // OURS: on BF1 or when landed, a craft that cannot find its way out gets
    // its pilot out; on BF2 in flight, stuck-kill destroys the wedged craft.
    if ai.wedges >= WEDGED_GIVEUP && (!p.bf2 || v.flyer.phase == FlightPhase::Landed) {
        super::ride::request_exit(st);
    }
    let r: RideState = st.ride.unwrap_or_default();
    st.ride = Some(RideState {
        flyer: Some(FlyerOut {
            sticks: out,
            takeoff,
            ai,
        }),
        ..r
    });
    (pad, nt)
}

#[cfg(test)]
mod tests;
