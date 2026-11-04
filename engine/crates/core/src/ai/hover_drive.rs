//! WGOUT marks the first accepted route goal after pickup as departure.
//! CWAIT pickup holds run in `ride` before route progress.
//! A bot at a hover's pilot seat (card VDROP): the sticks the PS2's hover AI
//! writes, transcribed from `analysis/re-hover-ai.md` [O] (B2 FUN_00220528 →
//! the shared steering FUN_002a54d8 → FUN_002a45a0; BF1 FUN_001eedc0), and
//! the port's driver around them ([`drive`], `ride::drive`'s hover half).
//!
//! **The law** ([`sticks`], §2-§4):
//! - throttle and strafe are the steering's pair toward the goal: with a
//!   StrafeSpeed, inside atan(SS / FS) of the goal the throttle is 1 and the
//!   strafe tan(error) · FS / SS; outside it the strafe is ±1 and the
//!   throttle falls to (SS / FS) / tan(error) — a BF2 speeder bike 20° off
//!   its goal holds 0.29. Without one, throttle = cos(error), strafe 0;
//! - the turn is a shaped proportional stick: u = clamp(s · (4.5 −
//!   clamp(TurnRate, 1, 3)), ±1), turn = u·|u|, s the sine of the heading
//!   error in the body's tilted frame (±1 when the goal is behind);
//! - inside R = ForwardSpeed / 2 of the goal with no path left a strafing
//!   hover fades the pair by |d| / R and turns to its aim point; a
//!   strafe-less one stops (BF2 only, §9);
//! - the obstacle probe (H4): a hit on a ray 1.5 × speed ahead moves the
//!   goal a quarter of that length sideways, off the hit's normal, and
//!   halves the throttle;
//! - the unstick manoeuvre (H8): stuck on its path ([`Follower::stuck`], §7)
//!   and [`Class::cooldown`] since the last one, it backs up at full
//!   throttle with half turn until its nose has swung 90°, then drives
//!   forward 3 s the same way;
//! - BF2: the turn × 0.6 while the throttle is under 0.1 (H9).
//!
//! The sticks are raw (`HoverInput { human: false }`): `RideState::hover` →
//! `Sim::ride`, past the pad's i8 and `sim::seats::driver_input`.
//!
//! The goal ([`look_ahead`], §5) is the point |velocity| + 0.2 m ahead of
//! the body's foot on the path segment, so a hover runs down the graph's own
//! line; the segment's start is the node it last reached (the PS2's), or
//! where it stood when its route began or changed (OURS). A detour corner is
//! a node like any other (card HJAM): the segment to it is begun once, and it
//! is reached at R (`route::corner_arrive`) — at the soldier's 1 m a BF1 bike
//! turned round each corner it had passed until its progress window failed.
//!
//! OURS, PROVISIONAL: the probe's ray set (the world's ORDNANCE bodies and
//! the other vehicles' hulls; the PS2's mask 0x43b has soldiers too, §2);
//! the `MovingTurnOnly` floor (H6) is transcribed and never taken — its
//! mover bit's sender is [H], §1; with a seen target the PS2 turns a stopped
//! hover to its aim order's point, the port to the type-2 point (ahead); the
//! pitch stick is `gunner::aim`'s; a teammate walking up to a gunner seat
//! zeroes throttle and strafe; the dismount at the post, when the progress
//! window has failed [`super::DRIVE_STUCK_BAIL`] (paused while the
//! manoeuvre runs) or routeless is `ride`'s. VBAIL checks the 20% health
//! bail-out (H2) in `ride::drive` before steering. Not built: the boost
//! (speed mode 3's senders are [H], §6).

use super::gunner::Look;
use super::mover::stuck_window;
use super::route::{advance_route, sub_point};
use super::state::{AiState, HoverOut, NavTick, RideState, SteerPick};
use super::{DRIVE_STUCK_BAIL, HOVER_ARRIVE_FACTOR};
use crate::collision::{HullFilter, Layers};
use crate::nav;
use crate::sim::{PadState, Sim};
use crate::unit::UnitHandle;
use crate::vehicle::hover::{HoverAi, HoverParams};
use crate::vehicle::Mount;
use glam::Vec3;

/// The steering's "nothing" (0x38d1b717).
const EPS: f32 = 1.0e-4;
/// §4: the aim point's reach along the travel or the body's forward.
const AIM_REACH: f32 = 30.0;
/// H4: the probe's length per m/s (0x3fc00000), the length it runs above
/// (0x40a00000), the goal's sideways shift as a share of it (0x3e800000)
/// and the throttle's factor on a hit.
pub const PROBE_REACH: f32 = 1.5;
pub const PROBE_MIN: f32 = 5.0;
pub const PROBE_SHIFT: f32 = 0.25;
pub const PROBE_THROTTLE: f32 = 0.5;
/// H4: the ray starts this far above the body's origin.
pub const PROBE_LIFT: f32 = 1.0;
/// H8: the forward half's length (s), the manoeuvre's turn stick, and the
/// time from one manoeuvre's end to the next (BF2 0x41700000, BF1
/// 0x40800000).
pub const UNSTICK_FORWARD: f32 = 3.0;
pub const UNSTICK_TURN: f32 = 0.5;
pub const UNSTICK_COOLDOWN_BF2: f32 = 15.0;
pub const UNSTICK_COOLDOWN_BF1: f32 = 4.0;
/// H9: under this throttle (0x3dcccccd) the turn is × 0.6 (0x3f19999a).
pub const SLOW_THROTTLE: f32 = 0.1;
pub const SLOW_TURN: f32 = 0.6;
/// H6: the `MovingTurnOnly` throttle floor.
pub const TURN_ONLY_FLOOR: f32 = 0.5;
/// §5: the goal leads the body's foot on the segment by |v| plus this (m).
pub const LOOKAHEAD: f32 = 0.2;
/// §7 (FUN_002f2448): 2 m from the anchor resets the still clock, which
/// trips at 3 s; 5 s with no new closest distance to the node trips too;
/// within 0.04 m² of the node neither runs.
pub const STUCK_MOVE: f32 = 2.0;
pub const STUCK_STILL: f32 = 3.0;
pub const STUCK_FAR: f32 = 5.0;
pub const STUCK_AT_NODE: f32 = 0.04;

/// What the law reads of the class (K+0x670, +0x678, +0x680, +0x688,
/// +0xe75) and the disc's rules (§9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Class {
    pub forward_speed: f32,
    pub strafe_speed: f32,
    pub turn_rate: f32,
    pub pitch_rate: f32,
    pub moving_turn_only: bool,
    pub bf1: bool,
}

impl Class {
    pub fn of(p: &HoverParams) -> Self {
        Self {
            forward_speed: p.forward_speed,
            strafe_speed: p.strafe_speed,
            turn_rate: p.turn_rate,
            pitch_rate: p.pitch_rate,
            moving_turn_only: false,
            bf1: p.bf1_ai,
        }
    }

    /// R: the stop / fade radius and the arrival at every node (§5).
    pub fn arrive(&self) -> f32 {
        self.forward_speed * HOVER_ARRIVE_FACTOR
    }

    pub fn cooldown(&self) -> f32 {
        if self.bf1 {
            UNSTICK_COOLDOWN_BF1
        } else {
            UNSTICK_COOLDOWN_BF2
        }
    }
}

/// The body of the tick before: the matrix rows (tilted; `left` = up × fwd,
/// the PS2's X), the velocity and the aim frame (ctl+0xd0, +0xe0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pos: Vec3,
    pub left: Vec3,
    pub fwd: Vec3,
    pub vel: Vec3,
    pub aim_origin: Vec3,
    pub aim_dir: Vec3,
}

/// The mover's order of this tick and what else the AI reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order {
    /// mv+0x20: the move point, `None` = not valid.
    pub goal: Option<Vec3>,
    /// A path follower is active (m != 0).
    pub path: bool,
    /// mv+0xa8 bit 4 (the `MovingTurnOnly` gate) and bit 5 ("stand").
    pub bit4: bool,
    pub stand: bool,
    /// H4's ray: 0 = no hit, ±1 = a hit whose normal points to ±side.
    pub probe: i8,
    /// The path follower's stuck word is set (§7).
    pub stuck: bool,
    /// Mission time since the last manoeuvre ended (E+0x5a4) and since its
    /// state 2 began (E+0x5a0), s.
    pub since_end: f32,
    pub since_state2: f32,
    /// Last tick's throttle and strafe: an invalid goal keeps them.
    pub held: [f32; 2],
}

/// FUN_002a4400: a unit vector, (1, 0, 0) for a zero input.
fn unit(v: Vec3) -> Vec3 {
    if v == Vec3::ZERO {
        Vec3::X
    } else {
        v / v.length()
    }
}

/// u = clamp(t · k, ±1) by sign, then u·|u| (0x2a47c0-0x2a4830).
pub(crate) fn shaped(t: f32, k: f32) -> f32 {
    let u = if t < 0.0 {
        (t * k).max(-1.0)
    } else if t > 0.0 {
        (t * k).min(1.0)
    } else {
        0.0
    };
    u * u.abs()
}

/// FUN_002a45a0 for an EntityHover (§4): (turn, pitch) toward `target`.
fn stick_to(k: &Class, m: &Body, target: Vec3) -> (f32, f32) {
    let d = Vec3::new(target.x - m.pos.x, 0.0, target.z - m.pos.z);
    let l = unit(Vec3::new(d.dot(m.left), 0.0, d.dot(m.fwd)));
    let mut t = -l.x;
    if l.z < 0.0 {
        t = if t < 0.0 { -1.0 } else { 1.0 };
    }
    let turn = shaped(t, 4.5 - k.turn_rate.clamp(1.0, 3.0));
    let aim = unit(m.aim_dir);
    let right = unit(Vec3::Y.cross(m.aim_dir));
    let up = aim.cross(right);
    let da = target - m.aim_origin;
    let la = unit(Vec3::new(da.dot(right), da.dot(up), da.dot(aim)));
    let pitch = shaped(la.y, 5.0 - k.pitch_rate.clamp(1.0, 3.0));
    (turn, pitch)
}

/// H4's goal for a probe hit on `side` (±1) at speed |`vel`|, or `None`
/// when the probe does not run (L ≤ 5 m).
pub fn probe_ray(pos: Vec3, vel: Vec3, goal: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let len = PROBE_REACH * vel.length();
    (len > PROBE_MIN).then(|| {
        let dir = unit(Vec3::new(goal.x - pos.x, EPS, goal.z - pos.z));
        (pos + Vec3::Y * PROBE_LIFT, dir, len)
    })
}

/// FUN_002a54d8 for an EntityHover (§3): (throttle, strafe, turn, pitch).
pub(super) fn steer(k: &Class, m: &Body, o: &Order, goal: Option<Vec3>) -> [f32; 4] {
    let has_strafe = k.strafe_speed > EPS;
    let r = k.arrive();
    let [mut thr, mut st] = o.held;
    let mut no_goal = goal.is_none();
    let mut slowing = true;
    if let Some(g) = goal {
        let d = Vec3::new(g.x - m.pos.x, 0.0, g.z - m.pos.z);
        let d2 = d.length_squared();
        slowing = d2 < r * r && !o.path;
        // BF1 has no stop for a slowing strafe-less hover (§9).
        if (d.x.abs() < EPS && d.z.abs() < EPS) || (slowing && !has_strafe && !k.bf1) {
            (thr, st, no_goal) = (0.0, 0.0, true);
        } else {
            let n = unit(d);
            let f = Vec3::new(m.fwd.x, 0.0, m.fwd.z);
            let f = if f.length_squared() >= EPS {
                unit(f)
            } else {
                Vec3::X
            };
            let dot = f.dot(n);
            let scale = if slowing {
                (d2.sqrt() / r).clamp(0.0, 1.0)
            } else {
                1.0
            };
            if has_strafe {
                let a = dot / k.forward_speed;
                let b = -f.cross(n).y / k.strafe_speed;
                if b.abs() < a.abs() {
                    st = b / a.abs() * scale;
                    thr = if a <= 0.0 { -scale } else { scale };
                } else {
                    thr = a / b.abs() * scale;
                    st = if b <= 0.0 { -scale } else { scale };
                }
            } else {
                thr = dot.clamp(0.0, 1.0) * scale;
                st = 0.0;
            }
        }
    }
    // The tail: toward the aim point (type 2: along the travel outside R,
    // else ahead) when stopped, fading with a strafe speed, or standing.
    let (turn, pitch) = match goal {
        Some(g) if !(no_goal || o.stand || (slowing && has_strafe)) => stick_to(k, m, g),
        _ => {
            let d = goal.map_or(Vec3::ZERO, |g| g - m.pos);
            let p = if goal.is_some() && r * r < d.x * d.x + d.z * d.z {
                m.pos + unit(d) * AIM_REACH
            } else {
                m.pos + m.fwd * AIM_REACH
            };
            stick_to(k, m, p)
        }
    };
    [thr, st, turn, pitch]
}

/// FUN_00220528's H3-H10: the four sticks (throttle E+0x2dc, strafe E+0x2e0,
/// turn E+0x2e4, pitch E+0x2e8) a bot writes this tick from the body of the
/// tick before, and the unstick state (`state` E+0x584, `fwd0` E+0x590)
/// moved on. Returns the sticks and what to stamp: state 2 began, the
/// manoeuvre ended.
pub fn sticks(
    k: &Class,
    m: &Body,
    o: &Order,
    state: &mut u8,
    fwd0: &mut Vec3,
) -> ([f32; 4], bool, bool) {
    // H4: the probe moves a copy of the goal.
    let mut goal = o.goal;
    let mut factor = 1.0;
    if let (Some(g), true) = (o.goal, o.probe != 0) {
        if let Some((_, dir, len)) = probe_ray(m.pos, m.vel, g) {
            let side = Vec3::new(-dir.z, 0.0, dir.x);
            goal = Some(g + side * (f32::from(o.probe) * len * PROBE_SHIFT));
            factor = PROBE_THROTTLE;
        }
    }
    let [mut thr, st, mut turn, pitch] = steer(k, m, o, goal);
    // H6
    if k.moving_turn_only && o.bit4 && turn.abs() > EPS {
        thr = if thr >= 0.0 {
            thr.max(TURN_ONLY_FLOOR)
        } else {
            thr.min(-TURN_ONLY_FLOOR)
        };
    }
    thr *= factor;
    // H8: the three tests run in this order in one call.
    let (mut began2, mut ended) = (false, false);
    if *state == 0 && o.stuck && o.since_end > k.cooldown() {
        *state = 1;
        *fwd0 = m.fwd;
    }
    if *state == 1 {
        if fwd0.dot(m.fwd) < 0.0 {
            *state = 2;
            began2 = true;
        }
        (thr, turn) = (-1.0, UNSTICK_TURN);
    }
    if *state == 2 {
        if !began2 && o.since_state2 > UNSTICK_FORWARD {
            *state = 0;
            ended = true;
        }
        (thr, turn) = (1.0, UNSTICK_TURN);
    }
    // H9 (BF2 only, §9)
    if !k.bf1 && thr.abs() < SLOW_THROTTLE {
        turn *= SLOW_TURN;
    }
    ([thr, st, turn, pitch], began2, ended)
}

/// FUN_002a5fa8 (§5): the mover's point for a hover on the segment `a → b`
/// (at the unit's height) — |v| + 0.2 m ahead of the body's foot on it, or
/// `b` once that passes the end.
pub fn look_ahead(a: Vec3, b: Vec3, pos: Vec3, speed: f32) -> Vec3 {
    let a = Vec3::new(a.x, pos.y, a.z);
    let b = Vec3::new(b.x, pos.y, b.z);
    let len = (b - a).length();
    if len < EPS {
        return b;
    }
    let u = (b - a) / len;
    let s = u.dot(pos - a) + speed + LOOKAHEAD;
    if s >= len {
        b
    } else {
        a + u * s
    }
}

/// FUN_002f24e8 (§7): the path follower's stuck test for one tick; true =
/// its stuck word is set (not 2 m in 3 s, or no new closest distance to
/// the node for 5 s).
pub fn stuck(f: &mut HoverAi, dt: f32, pos: Vec3, node: Vec3) -> bool {
    let (dx, dz) = (pos.x - node.x, pos.z - node.z);
    let d2 = dx * dx + dz * dz;
    if d2 < STUCK_AT_NODE {
        return false;
    }
    let mut word = false;
    if (f.anchor - pos).length_squared() > STUCK_MOVE * STUCK_MOVE {
        f.anchor = pos;
        f.still = 0.0;
    } else {
        f.still += dt;
    }
    if f.still > STUCK_STILL {
        word = true;
    }
    let d3 = (node - pos).length_squared();
    if d3 < f.best {
        f.best = d3;
        f.far = 0.0;
    } else {
        f.far += dt;
        if f.far > STUCK_FAR {
            word = true;
        }
    }
    word
}

/// The think of a bot at a hover's pilot seat (`ride::drive` hands it
/// here): down its route by [`sticks`], out at the post, when the progress
/// window has failed or routeless.
pub(super) fn drive(
    sim: &Sim,
    me: UnitHandle,
    m: Mount,
    hp: &HoverParams,
    st: &mut AiState,
    mut nt: NavTick,
) -> (PadState, NavTick) {
    let mut pad = PadState::default();
    let Some(v) = sim.vehicle(m.vehicle) else {
        return (pad, nt);
    };
    let k = Class::of(hp);
    let pos = v.pos;
    let arrive = k.arrive();
    advance_route(sim, me, st, pos, arrive, &mut nt);

    let post = st.target_cp.and_then(|i| sim.cps.get(i as usize));
    let last_leg = st.route.is_none_or(|r| r.is_last());
    let node = match (st.route, post) {
        (Some(r), _) => Some(sub_point(&r, pos)),
        (None, Some(cp)) => Some(cp.def.pos),
        (None, None) => None,
    };
    let at_post = post.is_some_and(|cp| {
        last_leg && (cp.def.capture.contains(pos) || nav::xz_dist(pos, cp.def.pos) <= arrive)
    }) || super::ride::at_drop(st, post, pos, arrive);
    // On a graph, a driver with no route has nowhere its vehicle can go (the
    // post is off its type's graph): straight at the post it would drive
    // into a wall with no stuck window to notice (BF2 POL1, MEDIUM).
    let bail = st.route.is_some_and(|r| r.stuck_run >= DRIVE_STUCK_BAIL)
        || (sim.nav.is_some() && st.route.is_none());
    let Some(node) = node.filter(|_| !at_post && !bail) else {
        super::ride::request_exit(st);
        nt.stuck_run = st.route.map_or(0, |r| r.stuck_run);
        return (pad, nt);
    };

    super::ride::set_out(sim, m, st);

    // The segment: from the node last reached, or from here when the route
    // began or changed under the driver. The node is compared on the ground
    // (card HJAM): a detour corner has no height of its own and takes the
    // hover's (`sub_point`), which moves every tick — compared whole, the
    // segment was begun anew each tick from the corner to itself, so the goal
    // was the corner (no point ahead on a line) and the "no nearer for 5 s"
    // half of the stuck test never ran on a corner.
    let mut ai = v.hover.ai;
    if !ai.leg.is_some_and(|l| l[1].x == node.x && l[1].z == node.z) {
        let from = ai
            .leg
            .map(|l| l[1])
            .filter(|p| nav::xz_dist(pos, *p) <= 2.0 * arrive)
            .unwrap_or(pos);
        ai.leg = Some([from, node]);
        ai.best = f32::MAX;
        ai.far = 0.0;
    }
    let from = ai.leg.map_or(pos, |l| l[0]);
    let segment_goal = look_ahead(from, node, pos, v.vel.length());
    let goal = sim.ground_traffic_goal(m.vehicle, segment_goal);
    let ray = |(at, dir, len): (Vec3, Vec3, f32)| {
        let hulls = HullFilter::except(Some(m.vehicle));
        let hit = sim
            .collision
            .ray_with(at, dir, len, Layers::ORDNANCE, hulls)?;
        let side = Vec3::new(-dir.z, 0.0, dir.x);
        Some(if hit.normal.dot(side) > 0.0 { 1 } else { -1 })
    };
    let probe = if goal == segment_goal {
        probe_ray(pos, v.vel, goal).and_then(ray).unwrap_or(0)
    } else {
        0
    };
    let secs = |since: Option<u64>| {
        since.map_or(f32::MAX, |t| {
            sim.tick.saturating_sub(t) as f32 * crate::TICK_DT
        })
    };
    let order = Order {
        goal: Some(goal),
        path: st.route.is_some(),
        bit4: false,
        stand: false,
        probe,
        stuck: stuck(&mut ai, crate::TICK_DT, pos, node),
        since_end: secs(ai.ended_at),
        since_state2: secs(Some(ai.state2_at)),
        held: [v.input.throttle, v.input.strafe],
    };
    let fwd = v.rot * Vec3::Z;
    let body = Body {
        pos,
        left: v.rot * Vec3::X,
        fwd,
        vel: v.vel,
        aim_origin: pos,
        aim_dir: fwd,
    };
    let ([thr, strafe, turn, _], began2, ended) =
        sticks(&k, &body, &order, &mut ai.state, &mut ai.fwd0);
    if began2 {
        ai.state2_at = sim.tick;
    }
    if ended {
        ai.ended_at = Some(sim.tick);
    }
    let r = st.ride.unwrap_or_default();
    st.ride = Some(RideState {
        hover: Some(HoverOut {
            sticks: [thr, strafe, turn],
            ai,
        }),
        ..r
    });
    st.steer = SteerPick::Fwd;
    super::gunner::aim(sim, me, st, &mut pad, Look::Steering, None);
    if ai.state == 0 {
        stuck_window(
            sim,
            me,
            st,
            pos,
            thr != 0.0 || strafe != 0.0,
            false,
            &mut nt,
        );
    }
    (pad, nt)
}

#[cfg(test)]
mod tests;
