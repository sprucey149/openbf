//! WGNXT witnesses reached-post stops and subsequent accepted post departures.
//! WGOUT marks the first accepted route goal after pickup as departure.
//! CWAIT pickup holds run in `ride` before route progress.
//! A bot at a walker's pilot seat (card WAI): the sticks the PS2's walker AI
//! writes, transcribed from `analysis/re-walker-ai.md` [O] (B2 FUN_00262248 →
//! the shared steering FUN_002a54d8 → FUN_002a45a0), and the port's driver
//! around them ([`drive`], `ride::drive`'s walker half).
//!
//! **The law** ([`sticks`], §2-§4, §7). The body is steered with the LEFT
//! stick's X (E+0x2e0, the locomotion's p3), not the turn stick:
//! - plain branch (A9 → B9): p3 = the deadbeat turn toward the goal at
//!   MaxTurnSpeed, clamped to ±1; the throttle is 0 until the goal is within
//!   5° ([`COS_WALK`]), then cos(error) — a walker turns in place, then walks;
//! - strafe branch (A8 → B8), a class with K+0xe18 bit 0
//!   (`walker::flags::AI`: BF2's AT-STs and tauntaun) and the goal inside 30°
//!   ([`COS_STRAFE`]): the steering's strafe output, NEGATED, is p3, which
//!   cancels re-walker §4.5 C's negative factor;
//! - stopped (no goal, or inside [`ARRIVE`] with no route left): throttle 0
//!   and p3 = the turn toward the aim point — straight ahead (0) with no
//!   target.
//!
//! The sticks are raw: a walking BF2 spider holds (0.997, 1.0), so they go to
//! the walker unshaped and unquantized (`RideState::sticks` →
//! `Sim::ride`), never through `sim::seats::driver_input` or the pad's i8.
//! On a class that authors K+0xe08 / +0xe0c and has no bit 0 (BF2's spider,
//! AT-TE, AT-AT) the same law turns the body AWAY from the goal while it
//! walks; it crabs along ~5° off its goal with the throttle toggling. That
//! is the PS2's own behaviour (live: median 5.31°), kept.
//!
//! The route goal projects 20.2 m ahead along the current segment (§6).
//! `Route::from` keeps and hashes its start, including passed detour corners.
//! OURS, PROVISIONAL; measure: PS2 node arrival and post/drop-hub exit traces.
//! A node is reached at `ARRIVE_MIN_RADIUS` (the PS2's
//! 1.0 m, or its plane passed); the dismount at the post or the drop hub
//! (`route::post_hub`) — [`ARRIVE`] plus the hull's radius short of either —
//! when stuck or routeless, and the held throttle for a
//! gunner walking up, are `ride`'s — but an ARMED walker's pilot that
//! arrives with an enemy in sight stays, stopped, and fights from the seat
//! until it sees none (the PS2's AT-ST stood with an aim order on 1040 of
//! its 2093 sampled ticks, §10), and a command walker's pilot (the AT-AT,
//! the AT-TE) stays at its post (card WAI2, §13), subject to VBAIL
//! low-health exit in `ride::drive`; a stopped walker
//! turns to its seen target's centre (the PS2's lead and aim error, §5, are
//! not transcribed); with no goal the throttle is 0 (the PS2 leaves the old
//! value, and A8 flips the stick's sign each tick — seen only on the tick an
//! AI attaches). Not built: the boost (speed mode 3's senders are [H], §8),
//! the 30 ticks
//! a rider stands after boarding (§10).

use super::gunner::Look;
use super::route::{advance_route, mark_abandoned, progress, set_waypoint, sub_point, Refresh};
use super::state::{AiState, NavTick, RideState, SteerPick};
use super::{ARRIVE_MIN_RADIUS, DRIVE_STUCK_BAIL, SIDESTEP_OFFSET};
use crate::nav;
use crate::perception;
use crate::sim::{PadState, Sim};
use crate::unit::UnitHandle;
use crate::vehicle::walker::{flags, WalkerParams};
use crate::vehicle::Mount;
use glam::Vec3;

/// FUN_002a63e0 for every kind but a soldier, droid or hover: the arrival
/// radius at the goal (0x2a63e0-0x2a6484).
pub const ARRIVE: f32 = 4.0;
/// BF2 FUN_002f1d80:281250; BF1 FUN_0026aba8:189731.
const ROUTE_LOOKAHEAD: f32 = 20.0;
/// The segment helper adds its shared 0.2 m padding to the walker's 20 m
/// lead. Traffic scans/bypass directions use the same effective reach.
pub(crate) const ROUTE_REACH: f32 = ROUTE_LOOKAHEAD + super::hover_drive::LOOKAHEAD;
/// A8's test: the strafe branch is taken inside 30° (0x3f5db3d7).
pub const COS_STRAFE: f32 = 0.866_025_4;
/// B9: a walker outside [`ARRIVE`] walks only inside 5° — FUN_00539ff8(
/// 0.0872665), the engine's table cosine.
pub const COS_WALK: f32 = 0.996_193_5;
/// A8's strafe speed, × MaxSpeed (0x3ecccccd).
pub const STRAFE_FACTOR: f32 = 0.4;
/// A command walker leaves its route end even with a retained post order
/// when its team has this many units or fewer, or either team has HOLD_ENDGAME
/// tickets. OURS, PROVISIONAL; measure: PS2 last-crew/post-order endgames.
/// The sampled PS2 pilots stay seated (re-walker-ai §13).
pub const COMMAND_ENDGAME: usize = 6;
/// B1 / B6's "nothing" (0x38d1b717).
const EPS: f32 = 1.0e-4;
/// §5: the aim point's reach along the travel or the body's forward.
const AIM_REACH: f32 = 30.0;

/// Shared command-pilot exit/boarding context; keep the claim policy from
/// reopening a seat the same last crew just left.
pub(crate) fn command_endgame(sim: &Sim, team: u8) -> bool {
    sim.battle
        .teams
        .iter()
        .any(|t| t.tickets() <= super::HOLD_ENDGAME)
        || sim.alive_units().filter(|(_, u)| u.team == team).count() <= COMMAND_ENDGAME
}

/// What the steering reads of the class (K+0x748, K+0x75c, K+0xe18 bit 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Class {
    pub max_speed: f32,
    pub max_turn_speed: f32,
    pub strafe: bool,
}

impl Class {
    pub fn of(p: &WalkerParams) -> Self {
        Self {
            max_speed: p.max_speed,
            max_turn_speed: p.max_turn_speed,
            strafe: p.flags & flags::AI != 0,
        }
    }
}

/// The body matrix's rows the steering reads (M+0x10, +0x20, +0x30).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pos: Vec3,
    pub up: Vec3,
    pub fwd: Vec3,
}

/// FUN_002a45a0's yaw for a walker (§4): the stick that would close the
/// angle to `target` in one tick at `rate`, clamped to ±1 (the ±2 of
/// mv+0xa8 bit 11 is not built). + = the target is toward −X (X = up × fwd).
fn turn_to(m: &Body, target: Vec3, rate_dt: f32) -> f32 {
    let d = target - m.pos;
    let n = (d - m.up * m.up.dot(d)).try_normalize().unwrap_or(Vec3::X);
    let mut v = -m.up.dot(m.fwd.cross(n));
    if rate_dt > 0.0 {
        v /= rate_dt;
    }
    v.clamp(-1.0, 1.0)
}

/// FUN_002a54d8 for an EntityWalker (§3): (throttle, strafe, turn).
/// `strafe_speed` = 0 is the plain call (A9), 0.4·MaxSpeed the strafe one.
fn steer(
    dt: f32,
    k: &Class,
    strafe_speed: f32,
    m: &Body,
    goal: Option<Vec3>,
    route_left: bool,
    target: Option<Vec3>,
) -> (f32, f32, f32) {
    let has_strafe = strafe_speed > EPS;
    let (mut thr, mut st) = (0.0, 0.0);
    let mut no_goal = true;
    if let Some(g) = goal {
        let d = Vec3::new(g.x - m.pos.x, 0.0, g.z - m.pos.z);
        let inside = d.length_squared() < ARRIVE * ARRIVE;
        // B6: a walker inside R with no path follower stops.
        if !((d.x.abs() < EPS && d.z.abs() < EPS) || (inside && !route_left)) {
            no_goal = false;
            let n = d.normalize();
            let f = Vec3::new(m.fwd.x, 0.0, m.fwd.z);
            let f = if f.length_squared() >= EPS {
                f.normalize()
            } else {
                Vec3::X
            };
            let dot = f.dot(n);
            if has_strafe {
                // B8
                let a = dot / k.max_speed;
                let b = -f.cross(n).y / strafe_speed;
                if b.abs() < a.abs() {
                    st = b / a.abs();
                    thr = if a <= 0.0 { -1.0 } else { 1.0 };
                } else {
                    thr = a / b.abs();
                    st = if b <= 0.0 { -1.0 } else { 1.0 };
                }
            } else {
                // B9
                thr = dot.clamp(0.0, 1.0);
                if !inside && dot < COS_WALK {
                    thr = 0.0;
                }
            }
        }
    }
    // B10: toward the aim point with a strafe speed or no goal, else the goal.
    let rate_dt = k.max_turn_speed * dt;
    let turn = match goal {
        Some(g) if !has_strafe && !no_goal => turn_to(m, g, rate_dt),
        _ => {
            // §5: the target; else along the travel outside R, else ahead.
            let p = target.unwrap_or_else(|| {
                let d = goal.map_or(Vec3::ZERO, |g| g - m.pos);
                if ARRIVE * ARRIVE < d.x * d.x + d.z * d.z {
                    m.pos + d.normalize() * AIM_REACH
                } else {
                    m.pos + m.fwd * AIM_REACH
                }
            });
            turn_to(m, p, rate_dt)
        }
    };
    (thr, st, turn)
}

/// FUN_00262248's A8 / A9: the sticks (p2 throttle E+0x2dc, p3 strafe
/// E+0x2e0) a bot writes this tick from the body of the tick before.
/// `goal`: the move order's point at the body's height, `None` = no valid
/// order; `route_left`: a path follower is active; `target`: the aim
/// order's point, `None` = aim type 2.
pub fn sticks(
    dt: f32,
    k: &Class,
    m: &Body,
    goal: Option<Vec3>,
    route_left: bool,
    target: Option<Vec3>,
) -> (f32, f32) {
    let ahead = goal.is_some_and(|g| (g - m.pos).normalize_or_zero().dot(m.fwd) > COS_STRAFE);
    if k.strafe && ahead {
        let (thr, st, _) = steer(
            dt,
            k,
            STRAFE_FACTOR * k.max_speed,
            m,
            goal,
            route_left,
            target,
        );
        (thr.max(0.0), -st)
    } else {
        let (thr, _, turn) = steer(dt, k, 0.0, m, goal, route_left, target);
        (thr, turn)
    }
}

/// The think of a bot at a walker's pilot seat (`ride::drive` hands it
/// here): along its route by [`sticks`], out at the post, when stuck or
/// routeless — `ride`'s rules for a hover's driver.
pub(super) fn drive(
    sim: &Sim,
    me: UnitHandle,
    m: Mount,
    wp: &WalkerParams,
    st: &mut AiState,
    mut nt: NavTick,
) -> (PadState, NavTick) {
    let mut pad = PadState::default();
    let Some(v) = sim.vehicle(m.vehicle) else {
        return (pad, nt);
    };
    let pos = v.pos;
    // BF2 FUN_00262248:181517-181543 / BF1 FUN_0021c180:132130-132145.
    // Keep the route and both progress clocks paused while contact yield owns
    // the movement goal. Aiming continues through the ordinary gunner path.
    if sim.tick < v.yield_until {
        let seen = st
            .target
            .and_then(|h| perception::contact_of(st, h))
            .filter(|c| c.seen_now)
            .and_then(|c| perception::foe_motion(sim, c.handle))
            .map(|(p, _)| p);
        let body = Body {
            pos,
            up: v.rot * Vec3::Y,
            fwd: v.rot * Vec3::Z,
        };
        let (p2, p3) = sticks(crate::TICK_DT, &Class::of(wp), &body, Some(pos), true, seen);
        st.ride.get_or_insert_default().sticks = Some([p2, p3]);
        st.steer = SteerPick::Idle;
        nt.stuck_run = st.route().map_or(0, |r| r.stuck_run);
        super::gunner::aim(
            sim,
            me,
            st,
            &mut pad,
            Look::Walker {
                yaw_rate: wp.max_yaw_speed,
                pitch_rate: wp.max_pitch_speed,
            },
            None,
        );
        return (pad, nt);
    }
    advance_route(sim, me, st, pos, ARRIVE_MIN_RADIUS, &mut nt);

    let post = st.target_cp.and_then(|i| sim.cps.get(i as usize));
    // The hull stops its own radius short, so it is not left standing on
    // the post or on the drop hub's node for the bots on foot to grind on.
    let reach = ARRIVE
        + sim
            .seat_setup
            .get(v.class as usize)
            .and_then(|s| s.as_ref())
            .map_or(0.0, |s| s.radius);
    // Card WAI2: an unarmed mount (the tauntaun) does not stop at the hub
    // its graph ends at when the floor runs on from there to the post with
    // no drop (`route::walk_floor`: BF2 Hoth's trenches lie between); its
    // route's end becomes the post itself, walked as far as it gets (the
    // stuck bail below). The PS2's tauntaun rides end 4.5-41 m from the
    // post, 282-711 m a ride (§13); the drop hub is 62-86 m from it. OURS,
    // PROVISIONAL.
    let mut dropped = st.route().is_none_or(|r| r.corner().is_none())
        && super::ride::at_drop(st, post, pos, reach);
    if let (true, Some(_), Some(cp)) = (dropped && sim.seat_look(me).is_none(), st.route(), post) {
        let to = glam::Vec2::new(cp.def.pos.x, cp.def.pos.z);
        if super::route::walk_floor(sim, pos, to).is_some() {
            set_waypoint(st, cp.def.pos, Refresh::RefDist(pos));
            dropped = false;
        }
    }
    let goal = match (st.route(), post) {
        (Some(r), _) => Some(super::hover_drive::look_ahead(
            Vec3::new(r.from.x, pos.y, r.from.y),
            sub_point(&r, pos),
            pos,
            ROUTE_LOOKAHEAD,
        )),
        (None, Some(cp)) => Some(cp.def.pos),
        (None, None) => None,
    };
    // (From the hop to the post's hub on: that hub is often at the post.)
    let final_hop = st
        .route()
        .is_none_or(|r| r.next_hub == r.dest_hub && r.corner().is_none());
    let reached_post = post.is_some_and(|cp| {
        final_hop && (cp.def.capture.contains(pos) || nav::xz_dist(pos, cp.def.pos) <= reach)
    });
    let at_post = reached_post || dropped;
    let no_progress = st
        .ride
        .and_then(|r| r.walker_progress)
        .is_some_and(|(_, t)| t >= DRIVE_STUCK_BAIL);
    let stuck = no_progress
        || st.route().is_some_and(|r| r.stuck_run >= DRIVE_STUCK_BAIL)
        || (sim.nav.is_some() && st.route().is_none());
    let seen = st
        .target
        .and_then(|h| perception::contact_of(st, h))
        .filter(|c| c.seen_now)
        .and_then(|c| perception::foe_motion(sim, c.handle))
        .map(|(p, _)| p);
    let body = Body {
        pos,
        up: v.rot * Vec3::Y,
        fwd: v.rot * Vec3::Z,
    };
    let look = Look::Walker {
        yaw_rate: wp.max_yaw_speed,
        pitch_rate: wp.max_pitch_speed,
    };
    // Arrived in an armed walker with an enemy in sight: it stands and
    // fights from the seat, the body turning to its target (§3's tail).
    // OURS: retain the normal command-walker stop, but let the last crew
    // walk from a drop hub to its Attack post or hunted enemy. A retained
    // post order must not exempt it from the endgame escape.
    // measure: PS2 command-walker crew orders at ticket/last-unit endgames.
    let endgame = sim.unit(me).is_some_and(|u| command_endgame(sim, u.team));
    let command_walker = super::ride::command(sim, m.vehicle);
    let command = command_walker && !endgame;
    let stays = command || (!command_walker && seen.is_some() && sim.seat_look(me).is_some());
    // re-driver-wait §§4/7: a later seated post set-out is a new source.
    // Only a witnessed physical post stop arms it; an early retarget, stuck
    // route, drop hub, ownership change or turning in place cannot do so.
    if sim.ground_rules == crate::soldier::GroundRules::Bf1 {
        if let Some(w) = st.ride.as_mut().and_then(|r| r.pickup.as_mut()) {
            if w.vehicle == m.vehicle && w.complete && w.departed {
                if command && reached_post && !stuck {
                    w.post_stop = st.target_cp;
                } else {
                    let stopped = w.post_stop.take();
                    if command
                        && !at_post
                        && !stuck
                        && goal.is_some()
                        && st.target_cp.is_some()
                        && stopped.is_some_and(|old| Some(old) != st.target_cp)
                    {
                        // Consume even a failed roll. Never reopen pickup or
                        // erase first-departure history to manufacture an edge.
                        w.post_departure = Some(sim.tick);
                    }
                }
            }
        }
    }
    // (A command walker's pilot does not bail out stuck either: it stands
    // where it got to — BF2 Geonosis's AT-TE stops 8.3 m from its last hub.)
    if (at_post && !stuck && stays) || (command && (at_post || stuck)) {
        if let Some(r) = st.ride.as_mut() {
            r.walker_progress = None;
        }
        let (p2, p3) = sticks(crate::TICK_DT, &Class::of(wp), &body, None, false, seen);
        let r = st.ride.unwrap_or_default();
        st.ride = Some(RideState {
            sticks: Some([p2, p3]),
            ..r
        });
        super::gunner::aim(sim, me, st, &mut pad, look, None);
        return (pad, nt);
    }
    let Some(gp) = goal.filter(|_| !at_post && !stuck) else {
        super::ride::request_exit(st);
        if no_progress {
            // Preserve the existing abandoned-vehicle marker in ai_use_pass.
            mark_abandoned(st, DRIVE_STUCK_BAIL);
        }
        nt.stuck_run = st.route().map_or(0, |r| r.stuck_run);
        return (pad, nt);
    };

    super::ride::set_out(sim, m, st);

    let gp = sim.ground_traffic_goal(m.vehicle, gp);
    let mut to = Vec3::new(gp.x - pos.x, 0.0, gp.z - pos.z);
    // The contact side-step offsets the target as it does on foot.
    if let Some(r) = st.route().filter(|r| r.sidestep.abs() > 1) {
        let d = to.normalize_or_zero();
        to += Vec3::new(-d.z, 0.0, d.x) * (f32::from(r.sidestep.signum()) * SIDESTEP_OFFSET);
    }
    let order = Some(pos + to);
    let (p2, p3) = sticks(crate::TICK_DT, &Class::of(wp), &body, order, true, seen);
    let r = st.ride.unwrap_or_default();
    st.ride = Some(RideState {
        sticks: Some([p2, p3]),
        ..r
    });
    st.steer = SteerPick::Fwd;
    super::gunner::aim(sim, me, st, &mut pad, look, None);
    walker_progress(st, pos, p2 != 0.0);
    progress(sim, me, st, pos, p2 != 0.0, false, &mut nt);
    (pad, nt)
}

/// OURS: a separate displacement clock survives route replans and their
/// reference-distance resets. XZ ignores animation bob; less than two metres
/// of jitter cannot restart it. Turning in place pauses the throttle clock.
/// measure: PS2 walker obstacle dwell and legitimate low-speed route travel.
fn walker_progress(st: &mut AiState, pos: Vec3, moving: bool) {
    let ride = st.ride.get_or_insert_default();
    let now = glam::Vec2::new(pos.x, pos.z);
    let (anchor, ticks) = ride.walker_progress.get_or_insert((now, 0));
    if now.distance_squared(*anchor) >= 4.0 {
        *anchor = now;
        *ticks = 0;
    } else if moving {
        *ticks = ticks.saturating_add(1);
    }
}

#[cfg(test)]
mod tests;
