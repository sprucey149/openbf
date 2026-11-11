//! Card OBST (`analysis/bot-design.md` 4.7, D3): the AVOIDANCE of a bot on
//! foot — [`pick`], the heading the mover walks on its pick tick. OURS and
//! PROVISIONAL throughout: the PS2 has no sweep at all (barriers, a contact
//! push and stuck timers, `analysis/re-soldier-ai.md` 2).
//!
//! Five rules sit on the steering's probes ([`super::steer`]):
//!
//! 1. **One side per obstacle.** A bot that meets a body picks the side to
//!    go round on ONCE ([`AiState::slide`]) and keeps it over open ticks and
//!    over the progress window, which does not fail a bot going round while
//!    it moves on ([`ROUND_TRAVEL`] a window, for [`ROUND_MAX`] ticks of a
//!    walk — a penned bot's walk fails that much later). The side changes at
//!    a dead end, once. Card OBSR: the window's allowance is for a BODY — a
//!    rim barrier ([`Round::rim`]) is an edge of the level with no far side
//!    to come round to, so it gets no allowance. A drop keeps its allowance
//!    (card OBSR).
//! 2. **An inside corner is turned, not a dead end.** Where another wall
//!    shuts the way along the first, the bot goes along THAT one on the same
//!    hand ([`CORNER_TURNS`]): in a room it follows the walls to the door.
//! 3. **The leave rule.** The obstacle is left for the straight way only
//!    when that way, as far as it is seen to be free ([`LEAVE_LOOK`]), ends
//!    nearer the aim point than the bot has been on its walk at that point
//!    ([`LEAVE_STEP`]). Until then it goes on along the obstacle and round
//!    its end when the wall beside it stops ([`SIDE_PROBE`]) — on its own
//!    level ([`CLIMB_MAX`]). (Kamon and Rivlin's DistBug, 1997 — the idea,
//!    not code.)
//! 4. **A small body is seen before it is touched.** With nothing being
//!    gone round, a sweep looks [`LOOK_DIST`] ahead before the probe; a body
//!    no wider than [`SMALL_BODY`] met there is
//!    passed by the tangent of its disc grown by [`LOOK_CLEAR`], on the side
//!    the bot already leans to ([`SteerPick::Round`]).
//! 5. **A standing vehicle is passed by its footprint** (card OBSV; the bot
//!    design's 4.7 item 4). A vehicle under [`FOOT_STILL`] — parked, or
//!    manned and standing — is a RECTANGLE on the ground, in its own frame,
//!    about the parts of its hull between a soldier's step and his head
//!    ([`footprint`]). With nothing being gone round, a way that crosses it
//!    within [`FOOT_LOOK`] is turned at its corner on ONE side: the shorter
//!    way round, chosen once ([`Round::foot`]) and kept until the footprint
//!    is behind ([`foot_ahead`]). Never the vehicle the bot boards. And a
//!    walk point a vehicle stands ON — a waypoint, a detour corner — is
//!    reached at the vehicle's side ([`at_foot_of`], `route::advance_route`).
//!
//! Limits: moving vehicles and soldiers in velocity space, barriers walked
//! by their corners every step, and probing only checked ground are not
//! built (bot design 4.7, card OBST2). Rule 5 applies only to standing
//! vehicles; moving vehicle avoidance is a separate design step. Map-body
//! walk points are not relocated by that rule: the turret corner and hollow
//! tree cases remain tracked by OBSV2 / ETREE.
//!
//! Evidence and rejected variants live in the existing reports under
//! `engine/artifacts/loop/reports/`: `20261007_0048_card_OBST.md`,
//! `20261009_1840_card_OBST2.md`, `20261015_1515_card_OBSV.md`.
//! The final OBSV landing report supersedes its pre-VMESH Endor blocker:
//! the paired 72-seed Endor comparison was 0 / 0 recycles after VMESH.
//! That does not resolve the separate OBSV2 / ETREE issues. OBST2's cost
//! work keeps the first steep-floor ray (`steer::steep_ahead`) and limits
//! look-ahead to small bodies; `collision::shape` casts cylinders directly.
//!
//! The tests of going round live here.
//!
//! [`AiState::slide`]: super::AiState::slide

use super::state::{NavTick, Round, SteerPick};
use super::steer::{floor_ahead, rotate_xz, stop_ahead, wall_ahead, StopKind};
use super::{FAN_STEP, PROBE_DIST};
use crate::nav;
use crate::sim::Sim;
use glam::{Vec2, Vec3};

/// A commitment is for ONE aim point: one more than this from it (m) is
/// another walk.
pub const ROUND_AIM_MOVED: f32 = 2.0;
/// A bot going round is making progress while it moves on this far in a
/// progress window (m; the PS2's stuck test is 2 m from an anchor in 3 s,
/// re-soldier-ai 2)...
pub const ROUND_TRAVEL: f32 = 2.0;
/// ...for at most this many ticks of going round since the walk last got
/// nearer its waypoint (10 s: 55 m of wall), over every obstacle on the way.
/// After that the window measures the distance to the waypoint again.
pub const ROUND_MAX: u16 = 300;
/// At an inside corner the next wall's tangent is tried, this many walls on.
pub const CORNER_TURNS: usize = 3;
/// The leave rule looks this far along the straight way (m)...
pub const LEAVE_LOOK: f32 = 6.0;
/// ...and leaves when its free end is this much nearer the aim point than
/// the bot has been (m).
pub const LEAVE_STEP: f32 = 0.5;
/// The wall beside a bot going along it is looked for this far out (m).
pub const SIDE_PROBE: f32 = 1.5;
/// The way along an obstacle climbs at most this far over where the
/// obstacle was met and over the aim point (m).
pub const CLIMB_MAX: f32 = 1.0;
/// How far ahead a small body is looked for (m): half a second of running.
pub const LOOK_DIST: f32 = 3.0;
/// The widest body passed by its disc: half its larger XZ extent (m).
pub const SMALL_BODY: f32 = 1.5;
/// The room left between the capsule and a body passed early (m).
pub const LOOK_CLEAR: f32 = 0.3;

/// Rule 5: a vehicle's footprint is looked for this far along the way (m).
pub const FOOT_LOOK: f32 = 6.0;
/// The room left between the capsule and a footprint's side (m)...
pub const FOOT_CLEAR: f32 = 0.3;
/// ...and how much further out its corners are walked at (m).
pub const FOOT_PLACE: f32 = 0.3;
/// A vehicle slower than this STANDS (m/s): a parked hover bobs at up to 0.9.
pub const FOOT_STILL: f32 = 1.5;
/// No hull's middle is further than this from its footprint's edge (m): the
/// coarse reject.
const FOOT_REACH: f32 = 16.0;
/// A bot inside a footprint's room is on its way INTO it when its way comes
/// this much nearer (m).
const FOOT_NEARER: f32 = 0.05;
/// Two ways round within this of each other (m) are a tie.
const FOOT_TIE: f32 = 0.25;

/// Rule 5: the footprint of vehicle slot `i` for a soldier whose feet are at
/// `y` — the rectangle, in the vehicle's own ground frame, about the parts
/// of its hull a soldier meets between his step and his head
/// ([`crate::collision::Body::ground_extent`]). `None`: a dead slot, or no
/// such part (an AT-AT's body is over him).
pub(super) fn footprint(sim: &Sim, i: usize, y: f32) -> Option<nav::Barrier> {
    let v = sim.vehicles.get(i).filter(|v| v.alive)?;
    let h = crate::vehicle::VehicleHandle {
        index: i as u32,
        gen: v.gen,
    };
    let f = v.rot * Vec3::Z;
    let fwd = Vec2::new(f.x, f.z).try_normalize().unwrap_or(Vec2::Y);
    let axes = [Vec2::new(fwd.y, -fwd.x), fwd];
    let origin = Vec2::new(v.pos.x, v.pos.z);
    let (lo_y, hi_y) = (
        y + crate::collision::STEP_HEIGHT,
        y + crate::unit::capsule_height(crate::soldier::Stance::Stand),
    );
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for b in sim.collision.hull(h) {
        if !b.layers.contains(crate::collision::Layers::SOLDIER)
            || b.aabb.max.y < lo_y
            || b.aabb.min.y > hi_y
        {
            continue;
        }
        let (l, m) = b.ground_extent(origin, axes);
        lo = lo.min(l);
        hi = hi.max(m);
    }
    if lo.x > hi.x {
        return None;
    }
    let mid = 0.5 * (lo + hi);
    Some(nav::Barrier {
        centre: origin + axes[0] * mid.x + axes[1] * mid.y,
        axes,
        half: 0.5 * (hi - lo),
        mask: u32::MAX,
        enabled: true,
    })
}

/// The footprints of the vehicles that STAND ([`FOOT_STILL`]) within `far` of
/// `a` (XZ; and [`FOOT_REACH`] of `y`), each with its slot.
fn standing<'a>(
    sim: &'a Sim,
    a: Vec2,
    y: f32,
    far: f32,
) -> impl Iterator<Item = (usize, nav::Barrier)> + 'a {
    let far = far + FOOT_REACH;
    sim.vehicles
        .iter()
        .enumerate()
        .filter(move |(_, v)| {
            v.alive
                && v.vel.length_squared() < FOOT_STILL * FOOT_STILL
                && (Vec2::new(v.pos.x, v.pos.z) - a).length_squared() <= far * far
                && (v.pos.y - y).abs() <= FOOT_REACH
        })
        .filter_map(move |(i, _)| footprint(sim, i, y).map(|f| (i, f)))
}

/// How far outside rectangle `f` the point `p` is, by the rectangle's own
/// measure ([`nav::Barrier::contains_r`]: the larger of the two distances
/// past a side); 0 or less inside it.
fn foot_dist(f: &nav::Barrier, p: Vec2) -> f32 {
    let d = p - f.centre;
    (d.dot(f.axes[0]).abs() - f.half.x).max(d.dot(f.axes[1]).abs() - f.half.y)
}

/// Rule 5's other half: is the point `p` under a standing vehicle — in its
/// footprint with the room a soldier keeps — and is a soldier at `pos`
/// within `reach` of that? Then he is as near `p` as he gets: a walk point a
/// vehicle is parked on is REACHED there (`route::advance_route`), where the
/// bot used to press on the hull until its progress window failed (a third
/// of the contact time left at standing vehicles once they are gone round).
pub(super) fn at_foot_of(sim: &Sim, p: Vec2, pos: Vec3, reach: f32) -> bool {
    let seen = crate::unit::SOLDIER_RADIUS + FOOT_CLEAR;
    let a = Vec2::new(pos.x, pos.z);
    standing(sim, p, pos.y, 0.0)
        .any(|(_, f)| f.contains_r(p, seen) && foot_dist(&f, a) <= seen + reach)
}

/// Rule 5 (`analysis/bot-design.md` 4.7 item 4): a standing vehicle's
/// footprint on the way is passed by its corner on ONE side — the shorter way
/// round, chosen once ([`Round::foot`]) and kept until the footprint is
/// behind. The heading and [`SteerPick::Round`], or `None`: nothing on the
/// way, the bot is in the footprint, the corner's heading is shut, or the
/// aim point is in the footprint itself (reached at its side,
/// [`at_foot_of`]). Never the vehicle the bot boards.
#[allow(clippy::too_many_arguments)]
fn foot_ahead(
    sim: &Sim,
    u: &crate::unit::Unit,
    want: Vec3,
    aim: Vec3,
    dist: f32,
    side: i8,
    rd: &mut Round,
    open: &impl Fn(Vec3) -> bool,
) -> Option<(Vec3, SteerPick)> {
    let pos = u.soldier.pos;
    let a = Vec2::new(pos.x, pos.z);
    let reach = dist.min(FOOT_LOOK);
    let b = a + Vec2::new(want.x, want.z) * reach;
    let seen = crate::unit::SOLDIER_RADIUS + FOOT_CLEAR;
    let boards = u.ai.ride.and_then(|r| r.board).map(|h| h.index as usize);
    // The nearest footprint the way crosses: (where, slot, the rectangle,
    // the room it was asked with, how far outside it the bot is).
    let mut best: Option<(f32, usize, nav::Barrier, f32, f32)> = None;
    for (i, f) in standing(sim, a, pos.y, reach) {
        if Some(i) == boards {
            continue;
        }
        // A bot already nearer than the room (it cut the corner, it was
        // shoved) is asked whether its way takes it NEARER still: along the
        // side and away from it the footprint is behind.
        let near = foot_dist(&f, a);
        let r = if near < seen {
            (near - FOOT_NEARER).max(0.0)
        } else {
            seen
        };
        let Some(t) = f.cross_t_r(a, b, r) else {
            continue;
        };
        if best.as_ref().is_none_or(|x| t < x.0) {
            best = Some((t, i, f, r, near));
        }
    }
    let (_, i, f, seen_r, near) = best?;
    // A point in the footprint is no point to go round to, and a bot in it
    // (it got out of a seat there) is the wall pass's.
    let to = Vec2::new(aim.x, aim.z);
    if f.contains_r(to, seen) || near <= 0.0 {
        return None;
    }
    // The two corners the footprint shows a bot looking at its middle: by
    // the left one he goes round it one way, by the right one the other —
    // from in front of it and from beside it alike (seen against the line
    // to the aim point, a corner already passed comes back as the widest).
    let corners = f.silhouette(a, f.centre, seen_r, seen + FOOT_PLACE);
    let id = i as u16 + 1;
    if rd.foot != id || rd.foot_side == 0 {
        // Which way: the shorter by its corner; a tie goes to `side`.
        let len = |c: Option<Vec2>| c.map_or(f32::MAX, |c| (c - a).length() + (to - c).length());
        let (l0, l1) = (len(corners[0]), len(corners[1]));
        rd.foot = id;
        rd.foot_side = if (l0 - l1).abs() < FOOT_TIE {
            side
        } else if l0 < l1 {
            -1
        } else {
            1
        };
    }
    corners[usize::from(rd.foot_side > 0)]
        .and_then(|c| (c - a).try_normalize())
        .map(|h| Vec3::new(h.x, 0.0, h.y))
        .filter(|h| open(*h))
        .map(|h| (h, SteerPick::Round))
}

/// The commitment ends: no side ([`Round::end`]).
pub(super) fn clear(slide: &mut i8, rd: &mut Round) {
    *slide = 0;
    rd.end();
}

/// The world XZ direction to move in on this pick, and why.
///
/// `want` is the unit direction toward the point walked at, `aim` the point
/// the walk is measured against (the leg's own point, not its look-ahead),
/// `slide` the side this bot is going round on (0 = none; written back),
/// `side` the side taken when nothing else decides, `low` the lowest floor a
/// step may land on. What says "blocked" is the steering's own tests
/// ([`super::steer`]: the sweep, a rim barrier, a hull, a steep floor) and a
/// missing floor; a drop is gone round like a wall facing back.
#[allow(clippy::too_many_arguments)]
pub(super) fn pick(
    sim: &Sim,
    u: &crate::unit::Unit,
    want: Vec3,
    low: f32,
    aim: Vec3,
    slide: &mut i8,
    side: i8,
    rd: &mut Round,
    nt: &mut NavTick,
) -> (Vec3, SteerPick) {
    // Every heading tried below sweeps from this one pose.
    sim.collision
        .remembering_start_contacts(|| pick_heading(sim, u, want, low, aim, slide, side, rd, nt))
}

#[allow(clippy::too_many_arguments)]
fn pick_heading(
    sim: &Sim,
    u: &crate::unit::Unit,
    want: Vec3,
    low: f32,
    aim: Vec3,
    slide: &mut i8,
    side: i8,
    rd: &mut Round,
    nt: &mut NavTick,
) -> (Vec3, SteerPick) {
    let pos = u.soldier.pos;
    let dist = nav::xz_dist(pos, aim);
    // A commitment, and the nearest the walk has been, are for one aim
    // point.
    if nav::xz_dist(aim, rd.aim) > ROUND_AIM_MOVED {
        if rd.live() {
            clear(slide, rd);
        }
        rd.best = 0.0;
        rd.aim = aim;
        rd.foot = 0;
    }
    if rd.live() {
        // The nearest the bot has been since it met the obstacle.
        rd.meet = rd.meet.min(dist.max(f32::MIN_POSITIVE));
    }
    // The probe is the STEPPED capsule (`Capsule::stepped`): a kerb is not a
    // wall, and the wall pass asks the same shape.
    let cap = u.capsule().stepped();
    let open = |d: Vec3| wall_ahead(sim, u, &cap, d).is_none() && floor_ahead(sim, u, d, low);
    // Along the wall on side `s` — and where another wall shuts that (an
    // inside corner), along THAT one on the same hand: a bot in a room
    // follows its walls round to the door...
    // ...and it stays on its level: a way along an obstacle that climbs
    // more than [`CLIMB_MAX`] over both where the obstacle was met (`y0`)
    // and the aim point is no way (BF2 Kashyyyk: round the end of a
    // platform's sloped roof, up it, and no way down but a drop).
    let aim_y = aim.y;
    let level = |d: Vec3, y0: f32| {
        let at = pos + d * PROBE_DIST + Vec3::Y * PROBE_DIST;
        sim.ground_point_under(at, false) <= y0.max(aim_y) + CLIMB_MAX
    };
    let follow = |mut dir: Vec3, s: i8, y0: f32| {
        for _ in 0..CORNER_TURNS {
            let Some(n2) = wall_ahead(sim, u, &cap, dir) else {
                return (floor_ahead(sim, u, dir, low) && level(dir, y0)).then_some(dir);
            };
            dir = Vec3::new(-n2.z, 0.0, n2.x) * f32::from(s);
        }
        None
    };
    // Rule 5, before everything: a standing vehicle's footprint.
    if !rd.live() {
        match foot_ahead(sim, u, want, aim, dist, side, rd, &open) {
            Some(h) => {
                *slide = 0;
                return h;
            }
            // (Behind, or not this rule's: the next one met is chosen anew.)
            None => rd.foot = 0,
        }
    }
    // Rule 4, before the probe: a small body further on is walked by.
    if !rd.live() {
        if let Some(h) = look_ahead(sim, u, &cap, want, side, &open) {
            *slide = 0;
            return h;
        }
    }
    let stop = stop_ahead(sim, u, &cap, want);
    if stop.is_some() {
        nt.sweep_hits = 1;
    }
    // Card MOUTH2: the way is shut by a door that is opening and by nothing
    // else — on at the point, and whatever was being gone round is dropped
    // (`steer.rs`'s header).
    if stop.is_some_and(|s| s.kind == StopKind::Door) {
        clear(slide, rd);
        return (want, SteerPick::Fwd);
    }
    let n = match stop.map(|s| s.normal) {
        Some(n) => n,
        None if floor_ahead(sim, u, want, low) => {
            if rd.live() {
                // THE LEAVE RULE: the obstacle is left for the straight way
                // when that way, as far as it is seen to be free, ends
                // nearer the aim point than the bot has been since it met
                // the obstacle. Until then it goes on along the obstacle,
                // and round its end when the wall beside it stops.
                let reach = dist.min(LEAVE_LOOK);
                let free = sim
                    .collision
                    .wall_sweep(&cap, want, reach)
                    .map_or(reach, |(t, _)| t);
                let h = rd.dir;
                if dist - free > rd.meet - LEAVE_STEP && h != Vec3::ZERO {
                    let w = Vec3::new(-h.z, 0.0, h.x) * f32::from(*slide);
                    if sim.collision.wall_sweep(&cap, w, SIDE_PROBE).is_none() {
                        let turn = (h + w).normalize_or_zero();
                        if turn != Vec3::ZERO && open(turn) && level(turn, rd.y0) {
                            rd.dir = turn;
                            return (turn, SteerPick::Slide);
                        }
                    }
                    if let Some(dir) = follow(h, *slide, rd.y0) {
                        rd.dir = dir;
                        return (dir, SteerPick::Slide);
                    }
                }
                clear(slide, rd);
                return (want, SteerPick::Fwd);
            }
            // A side that was only asked for (the stuck window's) is spent
            // on an open way.
            *slide = 0;
            return (want, SteerPick::Fwd);
        }
        // A drop ahead: go round it as round a wall facing back at us.
        None => -want,
    };
    // Which way round: the wall tangent `want - (want.n)n` leans toward the
    // aim point on its own, so its sign about the wall is the answer — the
    // FIRST time. After that the side is the commitment's.
    let along = Vec3::new(-n.z, 0.0, n.x);
    if !rd.live() {
        if *slide == 0 {
            let t = want.dot(along);
            *slide = if t.abs() < 0.1 {
                side
            } else if t < 0.0 {
                -1
            } else {
                1
            };
        }
        let near = if rd.best > 0.0 {
            dist.min(rd.best)
        } else {
            dist
        };
        *rd = Round {
            meet: near.max(f32::MIN_POSITIVE),
            y0: pos.y,
            best: rd.best,
            aim: rd.aim,
            anchor: rd.anchor,
            ticks: rd.ticks,
            ..Round::default()
        };
    }
    // Card OBSR: a rim barrier is an edge of the level, not a body with a
    // far side — what shuts the way now says which the bot is at.
    rd.rim = stop.is_some_and(|s| s.kind == StopKind::Barrier);
    let t = along * f32::from(*slide);
    if let Some(dir) = follow(t, *slide, rd.y0) {
        rd.dir = dir;
        return (dir, SteerPick::Slide);
    }
    // The committed side is a DEAD END: the other one, once — written back,
    // so the bot goes all the way round on it.
    let other = -t;
    if open(other) && level(other, rd.y0) {
        if !rd.flipped {
            rd.flipped = true;
            *slide = -*slide;
        }
        rd.dir = other;
        return (other, SteerPick::Slide);
    }
    rd.dir = Vec3::ZERO;
    // Both tangents are shut: the fan about `want`, the committed side first
    // at each step.
    for k in 1i8..=4 {
        for s in [*slide, -*slide] {
            let c = rotate_xz(want, f32::from(s) * f32::from(k) * FAN_STEP);
            if open(c) {
                return (c, SteerPick::Fan(s * k));
            }
        }
    }
    if floor_ahead(sim, u, -want, low) {
        return (-want, SteerPick::Back);
    }
    (Vec3::ZERO, SteerPick::Ledge)
}

/// Rule 4: a small body [`LOOK_DIST`] ahead along `want` is passed by the
/// tangent of its disc, when that heading is open. Asked only with no
/// commitment live.
fn look_ahead(
    sim: &Sim,
    u: &crate::unit::Unit,
    cap: &crate::collision::Capsule,
    want: Vec3,
    side: i8,
    open: &impl Fn(Vec3) -> bool,
) -> Option<(Vec3, SteerPick)> {
    // Only the small bodies are asked ([`wall_sweep_small`]): the sweep
    // against every floor and wall inside the look cost 7 % of the whole
    // simulation, to be thrown away when one of those came first.
    //
    // [`wall_sweep_small`]: crate::collision::CollisionWorld::wall_sweep_small
    let (_, n, body) = sim
        .collision
        .wall_sweep_small(cap, want, LOOK_DIST, SMALL_BODY)?;
    // A face that leans is a ramp's or a dune's.
    if n.y.abs() > 0.3 {
        return None;
    }
    let (lo, hi) = (body.aabb.min, body.aabb.max);
    let r = 0.5 * (hi.x - lo.x).max(hi.z - lo.z);
    let pos = u.soldier.pos;
    let c = Vec3::new(0.5 * (lo.x + hi.x), pos.y, 0.5 * (lo.z + hi.z));
    let to = Vec3::new(c.x - pos.x, 0.0, c.z - pos.z);
    let len = to.length();
    if len < 1e-3 {
        return None;
    }
    // The disc the body's box lies in, grown by the capsule and the room.
    let grown = r * std::f32::consts::SQRT_2 + crate::unit::SOLDIER_RADIUS + LOOK_CLEAR;
    let d = to / len;
    let perp = Vec3::new(-d.z, 0.0, d.x);
    // The side `want` already passes the centre on; dead on, the tie's.
    let lean = want.dot(perp);
    let s = if lean.abs() < 1e-3 {
        f32::from(side)
    } else {
        lean.signum()
    };
    // (Inside the grown disc the heading is along its rim.)
    let sin = (grown / len).min(1.0);
    let cos = (1.0 - sin * sin).max(0.0).sqrt();
    let h = d * cos + perp * (s * sin);
    // `want` already clears the disc: the sweep met a corner the disc holds.
    if want.dot(d) <= cos {
        return None;
    }
    open(h).then_some((h, SteerPick::Round))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::l_sim;
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::sim::PadState;
    use crate::unit::UnitHandle;

    fn block(c: Vec3, half: Vec3) -> Body {
        Body::new(glam::Mat4::from_translation(c), Shape::Box { half })
    }

    /// What one walk of `l_sim`'s bot down its first leg (the origin to hub
    /// 1 at z -40, the bodies across it) looked like.
    #[derive(Debug, Default)]
    struct Walked {
        /// The tick the bot was past z -25, if it got there.
        past: Option<u32>,
        /// Ticks on a blocked pick (a slide, the fan, back, a stand).
        blocked: u32,
        /// Changes of the side gone round on.
        flips: u32,
        /// The furthest it went back from the line's start (m, +z).
        back: f32,
    }

    fn walk(sim: &mut Sim, h: UnitHandle, ticks: u32) -> Walked {
        let mut w = Walked::default();
        let mut side = 0i8;
        for t in 0..ticks {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).expect("alive");
            let blocked = !matches!(
                u.ai.steer,
                SteerPick::Fwd | SteerPick::Round | SteerPick::Idle
            );
            w.blocked += u32::from(blocked);
            if u.ai.slide != 0 && u.ai.slide != side {
                w.flips += u32::from(side != 0);
                side = u.ai.slide;
            }
            w.back = w.back.max(u.soldier.pos.z);
            if u.soldier.pos.z < -25.0 {
                w.past = Some(t);
                break;
            }
        }
        w
    }

    /// The bot walks its leg with nothing on it in this many ticks (25 m).
    fn free_walk() -> u32 {
        let (mut sim, h) = l_sim();
        walk(&mut sim, h, 400).past.expect("an open leg is walked")
    }

    /// Card OBST's acceptance: a bot sent past a round block, a crate and a
    /// placed turret's base on its line walks BY each — no blocked pick at
    /// all, it is seen before it is touched — and loses under a second on
    /// the way. A parked vehicle's width of box (2.4 m, over the early
    /// turn's size) is met and gone round in under a second of sliding.
    #[test]
    fn a_bot_walks_by_a_block_a_crate_and_a_turret_and_round_a_hull() {
        let free = free_walk();
        let at = |x: f32| Vec3::new(x, 1.0, -12.0);
        let scenes: [(&str, Shape, f32, bool); 4] = [
            (
                "a round block",
                Shape::Cylinder {
                    radius: 1.0,
                    half_height: 1.0,
                },
                0.2,
                true,
            ),
            (
                "a crate",
                Shape::Box {
                    half: Vec3::new(0.6, 1.0, 0.6),
                },
                -0.2,
                true,
            ),
            (
                "a turret",
                Shape::Box {
                    half: Vec3::new(0.9, 1.0, 0.9),
                },
                0.0,
                true,
            ),
            (
                "a parked hull",
                Shape::Box {
                    half: Vec3::new(1.2, 1.0, 2.4),
                },
                0.3,
                false,
            ),
        ];
        for (name, shape, x, small) in scenes {
            let (mut sim, h) = l_sim();
            sim.collision = CollisionWorld::new(
                vec![Body::new(glam::Mat4::from_translation(at(x)), shape)],
                Vec::new(),
            );
            let w = walk(&mut sim, h, 400);
            let t = w
                .past
                .unwrap_or_else(|| panic!("{name}: never past it ({w:?})"));
            assert!(t <= free + 30, "{name}: {t} ticks against {free} free");
            assert_eq!(w.flips, 0, "{name}: one side ({w:?})");
            if small {
                assert_eq!(w.blocked, 0, "{name}: touched ({w:?})");
            } else {
                assert!(w.blocked < 30, "{name}: slid for over a second ({w:?})");
            }
        }
    }

    /// Card OBST's acceptance: a 20 m wall across the line, its near end 6 m
    /// to one side and its far end 14 m to the other. The bot goes round it
    /// on ONE side — the progress window, which the way round outlasts, does
    /// not hand it the other — and reaches the far side.
    #[test]
    fn a_wall_across_the_line_is_rounded_on_one_side() {
        for centre_x in [-4.0f32, 4.0] {
            let (mut sim, h) = l_sim();
            // Head-on: the leg's point is the hub's centre (card HUBP).
            if let Some(g) = sim.nav.as_mut() {
                g.hubs[1].radius = 0.0;
            }
            sim.collision = CollisionWorld::new(
                vec![block(
                    Vec3::new(centre_x, 2.0, -10.0),
                    Vec3::new(10.0, 2.0, 0.25),
                )],
                Vec::new(),
            );
            let w = walk(&mut sim, h, 900);
            assert!(w.past.is_some(), "{centre_x}: not round the wall ({w:?})");
            assert_eq!(w.flips, 0, "{centre_x}: changed side ({w:?})");
        }
    }

    /// AUDIT OBST: the 20 m wall above is passed by the steering before card
    /// OBST tick for tick; twice as wide it is not — the way round outlasts
    /// the progress window many times over, and each failed window handed
    /// that steering the other side (1,277 ticks and 19 changes of side on
    /// 9cd9b52).
    #[test]
    fn a_long_wall_is_rounded_on_one_side_in_good_time() {
        for centre_x in [-8.0f32, 8.0] {
            let (mut sim, h) = l_sim();
            if let Some(g) = sim.nav.as_mut() {
                g.hubs[1].radius = 0.0;
            }
            sim.collision = CollisionWorld::new(
                vec![block(
                    Vec3::new(centre_x, 2.0, -10.0),
                    Vec3::new(20.0, 2.0, 0.25),
                )],
                Vec::new(),
            );
            let w = walk(&mut sim, h, 1500);
            let t = w
                .past
                .unwrap_or_else(|| panic!("{centre_x}: not round the wall ({w:?})"));
            assert!(t <= 500, "{centre_x}: {t} ticks ({w:?})");
            assert_eq!(w.flips, 0, "{centre_x}: changed side ({w:?})");
        }
    }

    /// AUDIT OBST: the "parked hull" of the acceptance above is ONE
    /// placement. Here the same 2.4 x 4.8 m box stands at every yaw (5 degree
    /// steps) and seven offsets across the line: how many of the 252
    /// placements change the side gone round on, how many cost over a second
    /// of blocked picks, and none is never passed. (The steering before card
    /// OBST: 104 with a change of side.) Card OBSV: this box is a MAP body,
    /// which rules 1-3 go round (67 and 42 on main 17ae28e); as a vehicle's
    /// hull it is rule 5's, the next test but one.
    #[test]
    fn a_box_at_any_yaw_and_offset_is_rounded() {
        let (mut flipped, mut over1, mut never) = (0u32, 0u32, 0u32);
        for k in 0..36 {
            let yaw = (k as f32 * 5.0).to_radians();
            for x in [-1.5f32, -1.0, -0.5, 0.0, 0.3, 0.5, 1.0] {
                let (mut sim, h) = l_sim();
                sim.collision = CollisionWorld::new(
                    vec![Body::new(
                        glam::Mat4::from_translation(Vec3::new(x, 1.0, -12.0))
                            * glam::Mat4::from_rotation_y(yaw),
                        Shape::Box {
                            half: Vec3::new(1.2, 1.0, 2.4),
                        },
                    )],
                    Vec::new(),
                );
                let w = walk(&mut sim, h, 1200);
                flipped += u32::from(w.flips > 0);
                over1 += u32::from(w.blocked >= 30);
                never += u32::from(w.past.is_none());
            }
        }
        assert_eq!(never, 0, "a placement never passed");
        assert!(flipped <= 70, "{flipped} of 252 placements changed side");
        assert!(over1 <= 50, "{over1} of 252 placements cost over a second");
    }

    /// `l_sim` with a vehicle on the bot's first leg: a hull of one box,
    /// `half` its half extents, its middle at `at` on the ground, turned
    /// `yaw` about +Y. (A turret's kind: it has no drive to move it.)
    fn hull_sim(at: Vec3, half: Vec3, yaw: f32) -> (Sim, UnitHandle) {
        use crate::collision::{HullPart, Layers};
        use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
        use crate::weapon::HealthType;
        let (mut sim, h) = l_sim();
        sim.hull_parts = vec![Some(std::sync::Arc::from(vec![HullPart::new(
            glam::Mat4::from_translation(Vec3::Y * half.y),
            Shape::Box { half },
            Layers::ALL,
        )]))];
        sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Turret,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: at,
            yaw,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Gunner(0)],
        });
        (sim, h)
    }

    /// The parked hull of the scenes below: 2.4 x 4.8 m, 2 m high.
    const HULL: Vec3 = Vec3::new(1.2, 1.0, 2.4);

    /// Card OBSV's acceptance (rule 5): the 2.4 x 4.8 m box of the test
    /// above as what it stands for — a parked vehicle's HULL — at every yaw
    /// (5 degree steps) and seven offsets across the line. The bot passes
    /// every one of the 252 placements by a corner without ever being on a
    /// blocked pick (it does not touch the hull), on one side, and loses
    /// under half a second on the way. (MEASURED on main 17ae28e, the hull
    /// met and gone round by rules 1-3: 251 placements touched, 41 for over a
    /// second, 67 with a change of side, the slowest 143 ticks for 84 free.)
    #[test]
    fn a_parked_hull_at_any_yaw_and_offset_is_passed_by_its_corner() {
        let free = free_walk();
        for k in 0..36 {
            let yaw = (k as f32 * 5.0).to_radians();
            for x in [-1.5f32, -1.0, -0.5, 0.0, 0.3, 0.5, 1.0] {
                let (mut sim, h) = hull_sim(Vec3::new(x, 0.0, -12.0), HULL, yaw);
                let w = walk(&mut sim, h, 400);
                let at = format!("yaw {} x {x}: {w:?}", k * 5);
                let t = w.past.unwrap_or_else(|| panic!("never past it, {at}"));
                assert_eq!(w.blocked, 0, "touched, {at}");
                assert_eq!(w.flips, 0, "changed side, {at}");
                assert!(t <= free + 15, "{t} ticks for {free} free, {at}");
            }
        }
    }

    /// One pick for `l_sim`'s bot standing at `pos` with its aim point
    /// straight ahead (-Z) at `aim_z`: the pick and the commitment after it.
    fn pick_at(
        sim: &mut Sim,
        h: UnitHandle,
        pos: Vec3,
        aim_z: f32,
        rd: &mut Round,
    ) -> (Vec3, SteerPick) {
        sim.unit_mut(h).expect("alive").soldier.pos = pos;
        let u = sim.unit(h).expect("alive");
        let aim = Vec3::new(pos.x, 0.0, aim_z);
        let (mut slide, mut nt) = (0, NavTick::default());
        pick(sim, u, Vec3::NEG_Z, -1.0, aim, &mut slide, 1, rd, &mut nt)
    }

    /// Rule 5's side: the shorter way round is chosen ONCE per footprint and
    /// kept — a bot that has begun on the left of a hull goes on by its left
    /// corner from where the right one has become the nearer — and a hull
    /// dead ahead (a tie) is passed on the side handed in. With the
    /// footprint behind, the commitment is over.
    #[test]
    fn the_side_round_a_footprint_is_chosen_once_and_kept() {
        let (mut sim, h) = hull_sim(Vec3::new(0.0, 0.0, -8.0), HULL, 0.0);
        // 0.6 m left of the hull's middle line: the left corner (-X) is the
        // shorter way.
        let mut rd = Round::default();
        let (dir, p) = pick_at(&mut sim, h, Vec3::new(-0.6, 0.0, -2.0), -30.0, &mut rd);
        assert_eq!(p, SteerPick::Round);
        assert!(dir.x < -0.1 && dir.z < 0.0, "by the left corner: {dir}");
        assert_eq!((rd.foot, rd.live()), (1, false), "committed, no contact");
        let side = rd.foot_side;
        // Shoved 1.2 m to the right, where a first look would go right...
        let mut fresh = Round::default();
        let (right, _) = pick_at(&mut sim, h, Vec3::new(0.6, 0.0, -2.5), -30.0, &mut fresh);
        assert!(right.x > 0.1, "a first look from there: {right}");
        assert_eq!(fresh.foot_side, -side);
        // ...the commitment still goes left.
        let (dir, p) = pick_at(&mut sim, h, Vec3::new(0.6, 0.0, -2.5), -30.0, &mut rd);
        assert_eq!((p, rd.foot_side), (SteerPick::Round, side));
        assert!(dir.x < -0.1, "kept: {dir}");
        // Dead ahead: the side handed in (+1 here), the other with -1.
        let tie = |side: i8| {
            let (sim, h) = hull_sim(Vec3::new(0.0, 0.0, -8.0), HULL, 0.0);
            let u = sim.unit(h).expect("alive");
            let (mut slide, mut nt, mut rd) = (0, NavTick::default(), Round::default());
            let aim = Vec3::new(0.0, 0.0, -30.0);
            let d = pick(
                &sim,
                u,
                Vec3::NEG_Z,
                -1.0,
                aim,
                &mut slide,
                side,
                &mut rd,
                &mut nt,
            )
            .0;
            (d.x.signum(), rd.foot_side)
        };
        assert_eq!(tie(1), (-tie(-1).0, 1));
        assert_eq!(tie(-1).1, -1);
        // Past its far end the footprint is behind: straight on, no side.
        let (dir, p) = pick_at(&mut sim, h, Vec3::new(-2.3, 0.0, -11.5), -30.0, &mut rd);
        assert_eq!((p, rd.foot), (SteerPick::Fwd, 0), "{dir}");
    }

    /// Rule 5 for a bot already nearer the hull than the room it keeps (it
    /// cut the corner, it was shoved): the footprint is on its way only when
    /// the way takes it NEARER — along the hull's side it walks straight on,
    /// and turned in toward the hull it is sent to the corner. And an aim
    /// point in the footprint is not gone round to (it is reached at the
    /// vehicle's side).
    #[test]
    fn beside_a_hull_only_a_way_nearer_it_is_turned() {
        // The hull's left side is at x -1.2, the room's edge at x -2.0.
        let look = |x: f32, want: Vec3, aim: Vec3| {
            let (mut sim, h) = hull_sim(Vec3::new(0.0, 0.0, -8.0), HULL, 0.0);
            sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(x, 0.0, -7.0);
            let u = sim.unit(h).expect("alive");
            let (mut slide, mut nt, mut rd) = (0, NavTick::default(), Round::default());
            pick(&sim, u, want, -1.0, aim, &mut slide, 1, &mut rd, &mut nt).1
        };
        let ahead = Vec3::new(-1.8, 0.0, -30.0);
        // 0.2 m inside the room, walking along the side: on.
        assert_eq!(look(-1.8, Vec3::NEG_Z, ahead), SteerPick::Fwd, "along it");
        // ...and away from it.
        let away = Vec3::new(-0.6, 0.0, -0.8).normalize();
        assert_eq!(
            look(-1.8, away, Vec3::new(-15.0, 0.0, -25.0)),
            SteerPick::Fwd
        );
        // Turned in toward the hull: round by the corner.
        let inward = Vec3::new(0.6, 0.0, -0.8);
        assert_eq!(
            look(-1.8, inward, Vec3::new(10.0, 0.0, -23.0)),
            SteerPick::Round,
            "into it"
        );
        // From outside the room the same way in is turned too...
        assert_eq!(
            look(-2.6, inward, Vec3::new(10.0, 0.0, -23.0)),
            SteerPick::Round
        );
        // ...but not one that ends IN the footprint.
        assert_eq!(
            look(-2.6, inward, Vec3::new(-0.5, 0.0, -9.8)),
            SteerPick::Fwd,
            "an aim point under the vehicle"
        );
    }

    /// Rule 5's other half on a corner walk: a detour corner a vehicle is
    /// parked on is passed from the vehicle's side — the route goes on to
    /// its next corner — and from further off it is still ahead.
    #[test]
    fn a_detour_corner_under_a_parked_vehicle_is_passed_at_its_side() {
        let (mut sim, h) = hull_sim(Vec3::new(0.0, 0.0, -8.0), HULL, 0.0);
        let at = |sim: &mut Sim, pos: Vec3| {
            let mut st = sim.unit(h).expect("alive").ai;
            let mut r = st.route().expect("a route");
            r.detour[0] = glam::Vec2::new(0.3, -8.5);
            r.detour[1] = glam::Vec2::new(5.0, -20.0);
            (r.detour_len, r.detour_at) = (2, 0);
            st.install_route_for_fixture(Some(r));
            sim.unit_mut(h).expect("alive").soldier.pos = pos;
            let mut nt = NavTick::default();
            crate::ai::route::advance_route(sim, h, &mut st, pos, 0.0, &mut nt);
            st.route().expect("a route").detour_at
        };
        // (A tick, so the bot has its route.)
        sim.step(&[PadState::default()]);
        assert_eq!(at(&mut sim, Vec3::new(-2.6, 0.0, -8.0)), 1, "at its side");
        assert_eq!(at(&mut sim, Vec3::new(-4.5, 0.0, -8.0)), 0, "2.5 m off");
    }

    /// Rule 5 is for a vehicle that STANDS, that the bot does not board, and
    /// for the parts of its hull a soldier meets: one that moves, the one it
    /// walks to a seat of, and a hull over its head are walked straight at.
    #[test]
    fn a_moving_a_boarded_and_an_overhead_hull_are_no_footprint() {
        use crate::ai::RideState;
        let at = Vec3::new(0.0, 0.0, -6.0);
        let look = |sim: &mut Sim, h: UnitHandle| {
            let mut rd = Round::default();
            let p = pick_at(sim, h, Vec3::ZERO, -30.0, &mut rd).1;
            (p, rd.foot)
        };
        let (mut sim, h) = hull_sim(at, HULL, 0.3);
        assert_eq!(look(&mut sim, h), (SteerPick::Round, 1), "it stands");
        // It moves.
        sim.vehicles[0].vel = Vec3::new(0.0, 0.0, FOOT_STILL + 0.1);
        assert_eq!(look(&mut sim, h), (SteerPick::Fwd, 0), "it moves");
        sim.vehicles[0].vel = Vec3::new(0.0, 0.0, FOOT_STILL - 0.1);
        assert_eq!(look(&mut sim, h).0, SteerPick::Round, "it bobs");
        // The bot boards it.
        let vh = crate::vehicle::VehicleHandle {
            index: 0,
            gen: sim.vehicles[0].gen,
        };
        sim.unit_mut(h).expect("alive").ai.ride = Some(RideState {
            board: Some(vh),
            ..RideState::default()
        });
        assert_eq!(look(&mut sim, h), (SteerPick::Fwd, 0), "it is boarded");
        sim.unit_mut(h).expect("alive").ai.ride = None;
        // A hull 3 m up (a walker's body): no footprint, and one that
        // reaches down to the knee is one.
        let (mut sim, h) = hull_sim(at + Vec3::Y * 3.0, HULL, 0.3);
        assert!(footprint(&sim, 0, 0.0).is_none(), "over his head");
        assert_eq!(look(&mut sim, h), (SteerPick::Fwd, 0));
        let (sim, _) = hull_sim(at + Vec3::Y * 0.6, HULL, 0.3);
        let f = footprint(&sim, 0, 0.0).expect("at his knee");
        // The rectangle is the hull's own, turned with it.
        assert!(
            (f.half - glam::Vec2::new(1.2, 2.4)).length() < 1e-3,
            "{}",
            f.half
        );
        assert!((f.centre - glam::Vec2::new(0.0, -6.0)).length() < 1e-3);
        let fwd = sim.vehicles[0].rot * Vec3::Z;
        assert!((f.axes[1] - glam::Vec2::new(fwd.x, fwd.z)).length() < 1e-4);
        assert!(f.axes[0].dot(f.axes[1]).abs() < 1e-6);
    }

    /// Rule 5's other half: a leg's waypoint a vehicle is parked on is
    /// reached at the vehicle's side — the bot goes on down its next leg
    /// without a blocked pick. (Without it: it presses on the hull until its
    /// progress window fails.) A vehicle parked 6 m beside the waypoint
    /// changes nothing.
    #[test]
    fn a_waypoint_under_a_parked_vehicle_is_reached_at_its_side() {
        let run = |at: Vec3| {
            let (mut sim, h) = hull_sim(at, HULL, 0.4);
            // Head-on: the leg's point is the hub's centre.
            if let Some(g) = sim.nav.as_mut() {
                g.hubs[1].radius = 0.0;
            }
            let (mut blocked, mut hop) = (0u32, None);
            for t in 0..400 {
                sim.step(&[PadState::default()]);
                let u = sim.unit(h).expect("alive");
                blocked += u32::from(!matches!(
                    u.ai.steer,
                    SteerPick::Fwd | SteerPick::Round | SteerPick::Idle
                ));
                if hop.is_none() && u.ai.route().is_some_and(|r| r.next_hub == 2) {
                    hop = Some((t, nav::xz_dist(u.soldier.pos, Vec3::new(0.0, 0.0, -40.0))));
                }
            }
            (blocked, hop)
        };
        // The open leg: the hop at the node radius.
        let (blocked, hop) = run(Vec3::new(6.0, 0.0, -30.0));
        let (free, d) = hop.expect("the open leg is walked");
        assert!(
            blocked == 0 && d <= crate::ai::ARRIVE_MIN_RADIUS + 0.3,
            "{blocked} {d}"
        );
        // The vehicle on hub 1's centre: the hop comes sooner, from beside
        // the hull, and nothing was touched.
        let (blocked, hop) = run(Vec3::new(0.0, 0.0, -40.0));
        let (t, d) = hop.expect("the waypoint under the hull is reached");
        assert_eq!(blocked, 0, "touched the hull");
        assert!(
            t <= free && d > 2.4,
            "tick {t} for {free}, {d} m from the point"
        );
        // A point is under a vehicle only inside its footprint's room.
        let (sim, _) = hull_sim(Vec3::new(0.0, 0.0, -40.0), HULL, 0.0);
        let by = Vec3::new(2.5, 0.0, -40.0);
        let under = |x: f32| at_foot_of(&sim, glam::Vec2::new(x, -40.0), by, 1.0);
        assert!(under(0.0) && under(1.9) && !under(2.1));
        // ...and its side is `reach` from that room, no further.
        let far = Vec3::new(3.1, 0.0, -40.0);
        assert!(!at_foot_of(&sim, glam::Vec2::new(0.0, -40.0), far, 1.0));
    }

    /// Sixteen friends in a corridor `width` wide between two 40 m walls,
    /// two abreast: eight walk 22 m north to a point each, eight 22 m south
    /// through them. The tick each stood within a metre of its point, and
    /// the ticks any of them spent on a blocked pick.
    fn corridor(width: f32, ticks: u32) -> ([Option<u32>; 16], u32) {
        use crate::ai::HoldState;
        use crate::unit::{Controller, UnitSpec};
        let (mut sim, h0) = l_sim();
        // (The fixture's own bot stands out of the way.)
        sim.unit_mut(h0).expect("alive").soldier.pos = Vec3::new(100.0, 0.0, 100.0);
        let x = 0.5 * width + 0.25;
        sim.collision = CollisionWorld::new(
            vec![
                block(Vec3::new(-x, 2.0, -20.0), Vec3::new(0.25, 2.0, 20.0)),
                block(Vec3::new(x, 2.0, -20.0), Vec3::new(0.25, 2.0, 20.0)),
            ],
            Vec::new(),
        );
        let mut bots = Vec::new();
        for k in 0..16u8 {
            let (row, north) = (f32::from((k % 8) / 2), k < 8);
            let x = if k % 2 == 0 { -0.6 } else { 0.6 };
            let (z, to) = if north {
                (-6.0 - 1.5 * row, -22.0)
            } else {
                (-34.0 + 1.5 * row, 22.0)
            };
            let h = sim
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(0, Vec3::new(x, 0.0, z))
                })
                .expect("spawned");
            let goal = Vec3::new(x, 0.0, z + to);
            sim.unit_mut(h).expect("alive").ai.hold = Some(HoldState {
                spot: goal,
                goal,
                until: u64::MAX,
                shuffle_at: u64::MAX,
                commit_until: u64::MAX,
                last_contact: u64::MAX,
                ..HoldState::default()
            });
            bots.push((h, goal));
        }
        let (mut there, mut blocked) = ([None; 16], 0u32);
        for t in 0..ticks {
            sim.step(&[PadState::default()]);
            for (k, (h, goal)) in bots.iter().enumerate() {
                let u = sim.unit(*h).expect("alive");
                if there[k].is_none() && nav::xz_dist(u.soldier.pos, *goal) < 1.0 {
                    there[k] = Some(t);
                }
                blocked += u32::from(!matches!(
                    u.ai.steer,
                    SteerPick::Fwd | SteerPick::Round | SteerPick::Idle
                ));
            }
        }
        (there, blocked)
    }

    /// Card OBST2 (the bot design's 4.7, its scene for soldiers): eight
    /// bots each way through a 3 m corridor pass one another without a
    /// stall — every one is at its point within a second of the time the
    /// same walk takes with the walls 30 m apart, and none is ever on a
    /// blocked pick. (MEASURED on main 7dfb448: the last one there at tick
    /// 106 against 104 in the open — the separation pass shoulders them by.
    /// At 2 m none arrives: shoved onto the walls, they give the walk to a
    /// held point up, `HOLD_BLOCKED`.)
    #[test]
    fn eight_bots_each_way_pass_in_a_three_metre_corridor() {
        let last = |there: [Option<u32>; 16]| {
            there
                .iter()
                .map(|t| t.unwrap_or(u32::MAX))
                .max()
                .unwrap_or(u32::MAX)
        };
        let (open, _) = corridor(30.0, 400);
        let free = last(open);
        assert!(free < 150, "the open walk: {open:?}");
        let (there, blocked) = corridor(3.0, 400);
        assert!(
            last(there) <= free + 30,
            "a stall: {there:?} against {free} in the open"
        );
        assert_eq!(blocked, 0, "blocked picks");
    }

    /// Card OBSR: a commitment met at a rim barrier is marked so
    /// ([`Round::rim`]) — the progress window gives it no allowance
    /// (`route::progress`) — and one met at a wall is not.
    #[test]
    fn a_commitment_at_a_rim_barrier_is_marked() {
        let rim_of = |sim: &Sim, h: UnitHandle, want: Vec3| {
            let u = sim.unit(h).expect("alive");
            let aim = u.soldier.pos + want * 30.0;
            let (mut slide, mut nt, mut rd) = (0, NavTick::default(), Round::default());
            let low = u.soldier.pos.y - 0.5;
            let pick = pick(sim, u, want, low, aim, &mut slide, 1, &mut rd, &mut nt).1;
            (pick, rd.live(), rd.rim)
        };
        // The deck's rim rail, the hull falling away beyond it.
        let (mut sim, h) = crate::ai::tests::rim_sim(true);
        sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-8.6, 10.0, 0.0);
        assert_eq!(
            rim_of(&sim, h, Vec3::NEG_X),
            (SteerPick::Slide, true, true),
            "at the rim"
        );
        // A wall across the line.
        let (mut sim, h) = l_sim();
        sim.collision = CollisionWorld::new(
            vec![block(Vec3::new(0.0, 2.0, -1.5), Vec3::new(30.0, 2.0, 0.5))],
            Vec::new(),
        );
        assert_eq!(
            rim_of(&sim, h, Vec3::NEG_Z),
            (SteerPick::Slide, true, false),
            "at a wall"
        );
    }

    /// Rule 3's level: a bot that met an obstacle 1.5 m lower, with its aim
    /// point down there too, is not taken further along the obstacle up
    /// here; with the aim point on this level it is.
    #[test]
    fn a_way_along_an_obstacle_does_not_climb_off_its_level() {
        let (mut sim, h) = l_sim();
        sim.collision = CollisionWorld::new(
            vec![block(Vec3::new(0.0, 2.0, -1.5), Vec3::new(30.0, 2.0, 0.5))],
            Vec::new(),
        );
        let u = sim.unit(h).expect("alive");
        let pick_at = |aim_y: f32| {
            let aim = Vec3::new(0.0, aim_y, -30.0);
            let mut rd = Round {
                meet: 30.0,
                aim,
                y0: -1.5,
                ..Round::default()
            };
            let (mut slide, mut nt) = (1, NavTick::default());
            pick(
                &sim,
                u,
                Vec3::NEG_Z,
                -5.0,
                aim,
                &mut slide,
                1,
                &mut rd,
                &mut nt,
            )
            .1
        };
        assert_eq!(pick_at(0.0), SteerPick::Slide, "the aim is on this level");
        assert_ne!(pick_at(-1.5), SteerPick::Slide, "1.5 m over its level");
    }

    /// Rules 2 and 3: a bot in a three-walled yard — a wall across its line
    /// and one down each side, open behind it — follows the walls round an
    /// inside corner, out round a side wall's end, and on to its hub. Leaving
    /// for the straight way where that opens (the old rule) walks it back
    /// into the yard; taking the inside corner for a dead end paces it along
    /// the front wall.
    #[test]
    fn a_bot_in_a_yard_follows_the_walls_out() {
        let (mut sim, h) = l_sim();
        if let Some(g) = sim.nav.as_mut() {
            g.hubs[1].radius = 0.0;
        }
        sim.collision = CollisionWorld::new(
            vec![
                block(Vec3::new(0.0, 2.0, -6.0), Vec3::new(6.25, 2.0, 0.25)),
                block(Vec3::new(-6.0, 2.0, -1.0), Vec3::new(0.25, 2.0, 5.0)),
                block(Vec3::new(6.0, 2.0, -1.0), Vec3::new(0.25, 2.0, 5.0)),
            ],
            Vec::new(),
        );
        let w = walk(&mut sim, h, 900);
        assert!(w.past.is_some(), "not out of the yard ({w:?})");
        assert!(w.flips <= 1, "the side changed more than once ({w:?})");
        assert!(w.back > 3.5, "it went out by the open end ({w:?})");
    }
}
