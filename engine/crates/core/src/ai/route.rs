//! Where a unit is going and the leg it walks to get there (Phases A, A.2
//! and B): the objective rules ([`retarget`] — the commander's objective, or
//! Phase A's own rule for a unit the commander has not reached yet), the leg
//! planner [`plan_leg`] — the authored route table, the port's BFS, the
//! straight line — with its corner walk ([`fill_detour`]), the same-level
//! waypoint test ([`hub_has_floor`]), the hub a unit routes from
//! ([`start_hub`] — on the unit's own storey, [`level_hub`], card STLVL) and
//! the point a leg walks at right now ([`sub_point`]), and the point of a
//! post's capture region a last leg walks to ([`post_point`],
//! [`aim_last_leg`], card PARR — or the enemy contesting that region), and
//! the point of the next hub a leg walks at ([`hub_point`], card HUBP).
//! Card SPL11: and the route's advance ([`advance_route`]: the corners behind,
//! then the hub, `hub_reached`), the navigator's half of what `think` held.
//! The routing tests are in `route/tests.rs`.

use super::avoid;
use super::draw::{route_rand, stance_hold, unit_unit, SALT_HUB_POINT, SALT_POST_POINT};
use super::state::{AiState, NavTick, Route, RouteSource};
use super::{
    ARC_LINE_TOL, ARRIVE_MIN_RADIUS, CORNER_ARRIVE, DETOUR_MAX, DETOUR_RADIUS, DETOUR_WIDE_SLACK,
    HUB_LEVEL_TOL, LEG_LEAD, LEVEL_HUB_NEAR, LEVEL_HUB_REACH, LEVEL_HUB_TRIES, SIDESTEP_TICKS,
    STUCK_PROGRESS, STUCK_WINDOW,
};
use crate::nav::{self, NavGraph};
use crate::sim::Sim;
use crate::unit::UnitHandle;
use glam::{Vec2, Vec3};

/// The leg the navigator follows. No leg means there is no graph or no
/// destination on it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct NavState {
    route: Option<Route>,
}

impl NavState {
    pub fn route(&self) -> Option<Route> {
        self.route
    }

    #[cfg(test)]
    pub(super) fn for_fixture(route: Option<Route>) -> Self {
        Self { route }
    }

    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        match &self.route {
            Some(r) => {
                eat(&[1]);
                r.hash_into(eat);
            }
            None => eat(&[0]),
        }
    }
}

impl AiState {
    /// Install a synthetic navigation leg for test fixtures.
    #[cfg(any(test, feature = "route-fixtures"))]
    #[doc(hidden)]
    pub fn install_route_for_fixture(&mut self, route: Option<Route>) {
        self.nav.route = route;
    }
}

pub(crate) fn clear(st: &mut AiState) {
    st.nav.route = None;
}

pub(crate) fn plan_to(
    sim: &Sim,
    st: &mut AiState,
    me: UnitHandle,
    cur: u16,
    dest: u16,
    nt: &mut NavTick,
) {
    st.nav.route = plan_leg(sim, st, me, cur, dest, None, nt);
}

pub(crate) fn keep_or_plan(
    sim: &Sim,
    st: &mut AiState,
    me: UnitHandle,
    cur: u16,
    dest: u16,
    nt: &mut NavTick,
) {
    if st.nav.route.is_none_or(|r| r.dest_hub != dest) {
        plan_to(sim, st, me, cur, dest, nt);
    }
}

pub(crate) enum Refresh {
    None,
    RefDist(Vec3),
}

pub(crate) fn set_waypoint(st: &mut AiState, point: Vec3, refresh: Refresh) {
    if let Some(r) = st.nav.route.as_mut() {
        r.waypoint = point;
        if let Refresh::RefDist(pos) = refresh {
            r.ref_dist = nav::xz_dist(pos, sub_point(r, pos));
        }
    }
}

pub(crate) fn set_last_waypoint(st: &mut AiState, point: Vec3, refresh: Refresh) {
    if st.nav.route.is_some_and(|r| r.is_last()) {
        set_waypoint(st, point, refresh);
    }
}

pub(crate) fn grant_flank(st: &mut AiState, r: Route) {
    st.nav.route = Some(r);
}

pub(crate) fn dodge(st: &mut AiState, side: i8) -> bool {
    if let Some(r) = st.nav.route.as_mut() {
        r.sidestep = side * (SIDESTEP_TICKS + 1);
        true
    } else {
        false
    }
}

pub(crate) fn sidestep_tick(st: &mut AiState) {
    if let Some(r) = st.nav.route.as_mut().filter(|r| r.sidestep.abs() > 1) {
        r.sidestep -= r.sidestep.signum();
    }
}

pub(crate) fn mark_abandoned(st: &mut AiState, floor: u16) {
    if let Some(r) = st.nav.route.as_mut() {
        r.stuck_run = r.stuck_run.max(floor);
    }
}

/// The side-step countdown, then the progress window (design decision 4),
/// for a unit that `moving` — its stick is deflected — this tick. Card
/// ARCM: `back_and_forth` (the detector, `arcm::Osc`) closes the window now
/// as a failed one.
pub(super) fn progress(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    pos: Vec3,
    moving: bool,
    back_and_forth: bool,
    nt: &mut NavTick,
) {
    if sim.unit(me).is_none() {
        return;
    }
    // Both only matter while the unit is actually trying to walk somewhere.
    if let Some(mut r) = st.nav.route {
        if r.sidestep.abs() > 1 {
            r.sidestep -= r.sidestep.signum();
        }
        if moving {
            r.stuck_ticks += 1;
            if r.stuck_ticks >= STUCK_WINDOW || back_and_forth {
                r.stuck_ticks = 0;
                // Progress is measured to the CURRENT sub-waypoint, not to the
                // hub: a corner walk legitimately moves AWAY from the hub for
                // a while, and measuring the hub made every detour look stuck
                // and re-plan itself every 60 ticks. The PS2's own timer is
                // "distance to the node stopped improving" (`FUN_0026b450`).
                let d = nav::xz_dist(pos, sub_point(&r, pos));
                // Card OBST: a bot going round a body on its committed side
                // gets no nearer its waypoint for a while, and is not stuck
                // while it moves on: the window neither fails nor resets.
                // Card OBSR: ...round a BODY. A rim barrier is not gone
                // round, and a bot that follows one gets no allowance.
                let rd = st.mv.round;
                let rounding = rd.live()
                    && !rd.rim
                    && rd.ticks < avoid::ROUND_MAX
                    && nav::xz_dist(pos, rd.anchor) >= avoid::ROUND_TRAVEL;
                st.mv.round.anchor = pos;
                if r.ref_dist - d >= STUCK_PROGRESS && !back_and_forth {
                    r.stuck_run = 0;
                    r.ref_dist = d;
                } else if !rounding || back_and_forth {
                    // Card WHY: what the books say, before the cure below
                    // changes them.
                    let driving = sim.unit(me).is_some_and(|u| u.mount.is_some());
                    nt.failed(if back_and_forth {
                        super::why::Why::Oscillation
                    } else {
                        super::why::walk_failed(st, &r, driving)
                    });
                    nt.fail_arc = r.via_arc;
                    // Card ARCM: the leg's arc failed twice running (a
                    // failure since the walk last got anywhere), or back
                    // and forth: closed to the bot, so the re-plan below
                    // draws round it while another way exists.
                    if let Some(a) = r
                        .via_arc
                        .filter(|_| !driving && (back_and_forth || st.mv.recover > 0))
                    {
                        st.arcm.close(a, sim.tick);
                        nt.closed_arc = Some(a);
                    }
                    // The commitment ends with the failed window: the
                    // side-step's side starts the next.
                    st.mv.round.end();
                    r.stuck_run = r.stuck_run.saturating_add(STUCK_WINDOW);
                    // Re-locate and re-route once, then side-step to the side
                    // we did not use last time.
                    let side: i8 = if r.sidestep > 0 { -1 } else { 1 };
                    // ...and hand that side to the avoidance as well (Phase
                    // A.4; card OBST: the commitment has just ended above, so
                    // this side starts the next one — the PS2's "side-step to
                    // the side we did not use last time").
                    st.slide = side;
                    st.nav.route = Some(r);
                    // Plan again from the hub the unit stands in
                    // (`route::replan`).
                    if replan(sim, me, st, pos, nt) {
                        r = st.nav.route.unwrap_or(r);
                    }
                    r.sidestep = side * (SIDESTEP_TICKS + 1);
                }
            }
        }
        nt.stuck_run = r.stuck_run;
        st.nav.route = Some(r);
    }
}

/// How this unit routes on the planning graph: as a soldier on foot, as its
/// vehicle's `AISizeType` in a seat (card VAI; RE_LOG "The six type bits" —
/// the PS2's unit mask is the controlled entity's class+0x78).
pub(crate) fn mover(sim: &Sim, u: &crate::unit::Unit) -> nav::Mover {
    match u.mount.and_then(|m| sim.vehicle(m.vehicle)) {
        Some(v) => nav::Mover::new(
            sim.seat_setup
                .get(v.class as usize)
                .and_then(|s| s.as_ref())
                .map_or(nav::SOLDIER, |s| s.nav_type()),
            false,
        ),
        None => nav::Mover::new(nav::SOLDIER, u.spec.jet),
    }
}

/// Resolve an approach point on the same floor as the route chosen for it.
pub(crate) fn unit_goal_hub(sim: &Sim, g: &NavGraph, m: nav::Mover, at: Vec3) -> Option<u16> {
    g.unit_hub_on(at, m, &|h| hub_on_level(sim, g, h, at.y))
        .map(|h| level_hub(sim, g, h, at))
}

/// The world point this route is ultimately walking to: the target post, or
/// the destination hub when hunting.
fn goal_pos(sim: &Sim, g: &NavGraph, st: &AiState, dest: u16, from_y: f32) -> Vec3 {
    // (Card WAI2: a route that ends at another hub than the post's own — a
    // walker's drop hub, a claimer's vehicle — ends at that hub.)
    match st.target_cp.and_then(|i| sim.cps.get(i as usize)) {
        Some(cp) if cp.nav_hub == Some(dest) => cp.def.pos,
        _ => hub_waypoint(sim, g, dest, from_y),
    }
}

/// How many points of a post's capture region [`post_point`] draws before it
/// falls back. PROVISIONAL (ours): the PS2 draws one and paths to it.
pub const POST_POINT_TRIES: u32 = 8;
/// A last leg takes its point once the bot is within this of the capture
/// region's footprint (m, XZ). PROVISIONAL (ours): the floor is walked from
/// the bot's feet, so the draw waits until the bot is near.
pub const POST_POINT_RANGE: f32 = 10.0;
/// The drawn point's share of the region's footprint, so that a bot a
/// [`super::CORNER_ARRIVE`] short of it is inside the region. PROVISIONAL.
pub const POST_POINT_INSET: f32 = 0.9;
/// The step of [`walk_floor`] and of the fallback walk toward the bot (m).
const POST_POINT_STEP: f32 = 0.5;
/// A capsule the wall pass moves by more than this is in a body (m).
const POST_POINT_CLEAR: f32 = 0.02;
/// OURS, PROVISIONAL: allow a plane pass within three corner radii of the
/// incoming segment, retaining the spider's crab/turn corridor.
/// measure: matched PS2 walker corner passes after lateral displacement.
const WALKER_CORNER_LATERAL_RADII: f32 = 3.0;

/// The floor a soldier walking the straight line from `from` (feet on a
/// floor) arrives at over `to`: the foot's own ground query a step at a time,
/// so a rise over a step is not climbed and the walk goes under it. `None`
/// over open air or a drop past [`super::LEDGE_DROP`].
pub(super) fn walk_floor(sim: &Sim, from: Vec3, to: Vec2) -> Option<f32> {
    let start = Vec2::new(from.x, from.z);
    let len = (to - start).length();
    let steps = (len / POST_POINT_STEP).ceil().max(1.0) as u32;
    let mut y = from.y;
    for i in 1..=steps {
        let p = start + (to - start) * (i as f32 / steps as f32);
        let f = sim.ground_under(Vec3::new(p.x, y, p.y), false);
        if f <= crate::collision::NO_GROUND || f < y - super::LEDGE_DROP {
            return None;
        }
        y = f;
    }
    Some(y)
}

/// One uniform draw of `region`'s volume from three numbers in [0, 1), its
/// footprint drawn in by [`POST_POINT_INSET`]. Only the XZ is used.
fn region_draw(region: &crate::region::Region, a: f32, b: f32, c: f32) -> Vec2 {
    use crate::region::Shape;
    let l = match region.shape {
        Shape::Box => Vec3::new(
            (2.0 * a - 1.0) * region.size.x * POST_POINT_INSET,
            (2.0 * c - 1.0) * region.size.y,
            (2.0 * b - 1.0) * region.size.z * POST_POINT_INSET,
        ),
        Shape::Sphere | Shape::Cylinder => {
            let r = region.radius() * a.sqrt() * POST_POINT_INSET;
            let (s, co) = crate::fmath::sin_cos(std::f32::consts::TAU * b);
            let y = match region.shape {
                Shape::Cylinder => (2.0 * c - 1.0) * region.size.y,
                _ => 0.0,
            };
            Vec3::new(r * co, y, r * s)
        }
    };
    let p = region.center + region.basis * l;
    Vec2::new(p.x, p.z)
}

/// Card PARR: the point a bot on foot walks to at a post — the PS2's capture
/// behaviour walks to the post's position + a random point of its capture
/// region and is "at the post" inside the region
/// (`analysis/re-conquest-goal.md` section 4); it never aims at the centre,
/// which on the discs is a solid plinth (BF1) or on top of a prop (BF2
/// Kamino's cp0, 3 m over the floor).
///
/// The point is drawn from the unit's own stream at this tick, and it is one
/// a soldier can stand on: the floor walked from the bot's feet reaches it
/// ([`walk_floor`]), it is inside the region there, and the wall pass leaves
/// a soldier's capsule where it is. Where the post has a floor of its own
/// under the point (card STEND's: one at the centre's height,
/// [`super::mover::SPOT_FLOOR_TOL`]) the point is ON it — a bot under a
/// post's platform, or on a roof over it, is given no point there.
///
/// Of [`POST_POINT_TRIES`] draws the best wins: on the post's own floor
/// before beside it (BF2 Kamino's cp0: the floor round the depot), with no
/// wall on the straight line from the bot before with one. Else the nearest
/// standable point from the centre toward the bot; else the centre, as
/// before. OURS, PROVISIONAL: the standing tests and the order — the PS2
/// draws once and its mover finds the way.
pub(super) fn post_point(
    sim: &Sim,
    me: UnitHandle,
    u: &crate::unit::Unit,
    cp: &crate::cp::CommandPost,
) -> Vec3 {
    let from = u.soldier.pos;
    let region = &cp.def.capture;
    let tol = super::mover::SPOT_FLOOR_TOL;
    // The point over `xz` and whether it is on the post's own floor.
    let stands = |xz: Vec2| -> Option<(Vec3, bool)> {
        let p = Vec3::new(xz.x, walk_floor(sim, from, xz)?, xz.y);
        let own = sim.ground_under(Vec3::new(xz.x, cp.def.pos.y, xz.y), false);
        let has_own = (own - cp.def.pos.y).abs() <= tol;
        if has_own && (own - p.y).abs() > crate::collision::STEP_HEIGHT {
            return None;
        }
        let cap = crate::collision::Capsule {
            base: p,
            height: crate::unit::capsule_height(crate::soldier::Stance::Stand),
            radius: crate::unit::SOLDIER_RADIUS,
        };
        (region.contains(p) && sim.collision.push_out_soldier(&cap).length() <= POST_POINT_CLEAR)
            .then_some((p, has_own))
    };
    let clear = |p: Vec3| {
        let d = Vec3::new(p.x - from.x, 0.0, p.z - from.z);
        let len = d.length();
        len < 1e-3
            || (sim
                .collision
                .wall_sweep(&u.capsule().stepped(), d / len, len)
                .is_none()
                && sim.nav.as_ref().is_none_or(|g| {
                    g.barrier_stop(Vec2::new(from.x, from.z), Vec2::new(p.x, p.z), nav::SOLDIER)
                        >= 1.0
                }))
    };
    let mut best: Option<(u8, Vec3)> = None;
    for k in 0..POST_POINT_TRIES {
        let draw = |j: u32| unit_unit(me.index, sim.tick, me.gen, SALT_POST_POINT + 3 * k + j);
        let Some((p, own)) = stands(region_draw(region, draw(0), draw(1), draw(2))) else {
            continue;
        };
        let rank = 2 * u8::from(own) + u8::from(clear(p));
        if rank == 3 {
            return p;
        }
        if best.is_none_or(|(b, _)| rank > b) {
            best = Some((rank, p));
        }
    }
    if let Some((_, p)) = best {
        return p;
    }
    let c = Vec2::new(cp.def.pos.x, cp.def.pos.z);
    let to = Vec2::new(from.x, from.z) - c;
    let reach = to.length().min(region.radius());
    let n = (reach / POST_POINT_STEP) as u32;
    (1..=n)
        .find_map(|i| stands(c + to.normalize_or_zero() * (i as f32 * POST_POINT_STEP)))
        .map_or(cp.def.pos, |(p, _)| p)
}

/// Card PARR: give a bot's LAST LEG at a post its point ([`post_point`]) — a
/// leg still aimed at the post's centre, of a grounded bot on foot within
/// [`POST_POINT_RANGE`] of the region. A defender's Cover / Snipe spot is not
/// the centre and is kept; a re-plan (the stuck window) aims at the centre
/// again and so draws afresh.
///
/// THE CONTESTER. A bot with no target, at a post an enemy soldier stands in
/// the region of, walks at that enemy (the nearest) instead: spread over a
/// wide region with walls in it, the two sides stand out of each other's
/// sight and the post stays contested for good (TAT1 cp1, seed 3: sixteen
/// attackers on their points and one defender in a hut 10 m off, 16 v 1 at
/// 60,000 ticks — the ring round BF1 NAB1's cp3 again). OURS, PROVISIONAL:
/// like the hunt it is not bounded by what the bot sees.
pub(super) fn aim_last_leg(sim: &Sim, me: UnitHandle, st: &mut AiState, nt: &mut NavTick) {
    let (Some(u), Some(mut r)) = (sim.unit(me), st.nav.route) else {
        return;
    };
    let Some(cp) = st.target_cp.and_then(|i| sim.cps.get(i as usize)) else {
        return;
    };
    let pos = u.soldier.pos;
    let region = &cp.def.capture;
    if !r.is_last()
        || u.mount.is_some()
        || !u.soldier.grounded
        || nav::xz_dist(pos, region.center) > region.radius() + POST_POINT_RANGE
    {
        return;
    }
    let contester = (st.target.is_none())
        .then(|| {
            sim.alive_units()
                .filter(|(_, o)| {
                    sim.hostile(u.team, o.team)
                        && o.mount.is_none()
                        && region.contains(o.soldier.pos)
                })
                .map(|(_, o)| o.soldier.pos)
                .min_by(|a, b| {
                    (*a - pos)
                        .length_squared()
                        .total_cmp(&(*b - pos).length_squared())
                })
        })
        .flatten();
    let p = match contester {
        Some(e) => e,
        None if r.waypoint == cp.def.pos => post_point(sim, me, u, cp),
        None => return,
    };
    if p != r.waypoint {
        r.waypoint = p;
        // The corner walk to the new point — but not on a leg the stuck
        // window has re-planned: that one dropped its corners on purpose.
        if let (Some(g), 0) = (sim.nav.as_ref(), r.stuck_run) {
            fill_detour(g, mover(sim, u).ty, pos, &mut r, nt);
        }
        r.ref_dist = nav::xz_dist(pos, sub_point(&r, pos));
        st.nav.route = Some(r);
    }
}

/// Fill this leg's CORNER WALK: the intermediate waypoints that get round the
/// barriers the straight leg `from → r.waypoint` crosses (PLAN "A.1 addendum",
/// [`crate::nav::NavGraph::detour`]).
///
/// An empty result — the straight leg is clear — and a FAILED search are both
/// "walk straight at the waypoint": a leg the corner search cannot resolve is
/// exactly the case the wall probe is there for, and on the discs it is
/// always a BF2 door — an engine `door` entity that opens itself, not the
/// mission Lua (`analysis/re-bf2-doors.md`) — (RE_LOG: 100 % of BF1's crossing
/// legs resolve, 1,150 of 1,262 overall). A walk of more than [`DETOUR_MAX`]
/// corners is cut to that many; [`advance_route`] asks again at the last.
fn fill_detour(g: &NavGraph, ty: usize, from: Vec3, r: &mut Route, nt: &mut NavTick) {
    r.detour = [Vec2::ZERO; DETOUR_MAX];
    r.detour_len = 0;
    r.detour_at = 0;
    if g.barriers.is_empty() {
        return;
    }
    let a = Vec2::new(from.x, from.z);
    let b = Vec2::new(r.waypoint.x, r.waypoint.z);
    let path = if ty == nav::SOLDIER {
        soldier_way(g, a, b)
    } else {
        g.detour(a, b, DETOUR_RADIUS, ty)
    };
    let Some(path) = path else {
        return;
    };
    if path.is_empty() {
        return;
    }
    for (i, &p) in path.iter().take(DETOUR_MAX).enumerate() {
        r.detour[i] = p;
        r.detour_len = (i + 1) as u8;
    }
    nt.detoured += 1;
}

/// A soldier's corner walk from `a` to `b`: the wide margin's way
/// ([`DETOUR_RADIUS`]), unless it finds none, ignores a barrier the PS2's
/// margin (the capsule radius) keeps - one an end point stands within 1.0 m
/// of, a stair's rail - or is longer than the PS2's by more than
/// [`DETOUR_WIDE_SLACK`]: then the PS2's (issue #34).
fn soldier_way(g: &NavGraph, a: Vec2, b: Vec2) -> Option<Vec<Vec2>> {
    let ty = nav::SOLDIER;
    let narrow = crate::unit::SOLDIER_RADIUS;
    let wide = g.detour(a, b, DETOUR_RADIUS, ty);
    let drops =
        g.leg_ignores(a, b, DETOUR_RADIUS, ty).len() > g.leg_ignores(a, b, narrow, ty).len();
    if !drops && wide.as_ref().is_some_and(|w| w.is_empty()) {
        return wide;
    }
    match (wide, g.detour(a, b, narrow, ty)) {
        (_, Some(p)) if drops => Some(p),
        (Some(w), Some(p)) if way_len(a, &w, b) > way_len(a, &p, b) + DETOUR_WIDE_SLACK => Some(p),
        (None, p) => p,
        (w, _) => w,
    }
}

/// The length of the walk from `a` over `corners` to `b` (XZ).
fn way_len(a: Vec2, corners: &[Vec2], b: Vec2) -> f32 {
    let mut at = a;
    let mut len = 0.0;
    for &c in corners.iter().chain(std::iter::once(&b)) {
        len += (c - at).length();
        at = c;
    }
    len
}

/// How near a detour corner counts as reached. A soldier and a walker:
/// [`CORNER_ARRIVE`]. A hover's driver: its own node radius (`min_arrive`,
/// ForwardSpeed x 0.5 — `analysis/re-hover-ai.md` 5: the PS2's hover arrives
/// at every node of its path at that distance, and a corner is a node of the
/// follower's, re-soldier-ai 2) [D], PROVISIONAL. Card HJAM: at the soldier's 1 m a BF1
/// speeder bike (40 m/s, no strafe, R 20 m) passed the corner some metres
/// wide and then turned round it at full stick for as long as its progress
/// window let it — 43 % of BF1 Endor's and 50 % of BF1 Yavin's bike-driving
/// time was within 8 m of a corner, and half of all the drivers' failed
/// windows (1,153 of 2,303 in 240 rounds) were at one.
fn corner_arrive(sim: &Sim, me: UnitHandle, min_arrive: f32) -> f32 {
    let hover = sim
        .unit(me)
        .and_then(|u| u.mount)
        .and_then(|m| sim.vehicle(m.vehicle))
        .is_some_and(|v| v.kind == crate::vehicle::VehicleKind::Hover);
    if hover {
        CORNER_ARRIVE.max(min_arrive)
    } else {
        CORNER_ARRIVE
    }
}

/// The point a leg is walking at right now: its current detour corner (which is
/// XZ, so it takes the unit's own y) or, with the corners behind it, the leg's
/// own waypoint. This is the PS2's "current path node", and it is what BOTH
/// the steering and the stuck window measure against.
pub(super) fn sub_point(r: &Route, pos: Vec3) -> Vec3 {
    match r.corner() {
        Some(c) => Vec3::new(c.x, pos.y, c.y),
        None => r.waypoint,
    }
}

/// THE PS2'S LEG RULE (BF2 `FUN_002f1d80` -> `FUN_002a5fa8`,
/// `analysis/re-soldier-ai.md` 2; BF1 aims straight at the point, and
/// `analysis/bot-design.md` 4.5 uses BF2's rule for both games; card MOVR2):
/// where a unit on an ARC leg walks this tick is its PROJECTION on the leg's
/// own line ([`Route::from`] to its waypoint — card HUBP: from the waypoint it
/// reached in the last hub, or from that hub's centre on a leg planned
/// anywhere else, to the unit's point of the next hub; hub centre to hub
/// centre on a stuck unit's re-plan) advanced by its `speed` (m/s read as m)
/// plus [`LEG_LEAD`], clamped to the leg's end — not straight at the far hub
/// from wherever it happens to be. Card MOUTH: with a detour corner behind
/// the unit the line starts at THAT corner ([`seg_from`]) — the PS2's line is
/// its follower's segment, the node just reached to the node walked at, and a
/// corner is a node (BF2 `FUN_002f1470` -> `FUN_002f0e78`, re-soldier-ai 2).
/// Until then the line stayed the leg's own, so a unit that had just walked
/// the corner at a walkway's mouth was aimed back at the line the corner walk
/// was planned to avoid: beside the walkway, into its rail (BF2 Kamino's four
/// walkway legs 32 stalls in 8 rounds, and BF1 Bespin's bots pulled off a
/// walkway and over the rim: 46 deaths under the floor in 8 rounds against 12).
/// A unit beside its line comes back to it:
/// at a run over the next few metres, from a stand at once. (The next leg
/// can start metres off the line a bridge lies on, and the beeline from
/// there met the platform's rail beside the bridge: 20 of 32 units stood
/// stuck on BF2 KAM1 after 200 s — the maintainer, 2026-10-01.) A detour corner, and a
/// leg with no arc (the last leg, a re-plan's approach), are walked straight
/// at. Until card MOVR2 the port aimed 5 m ahead on the line, and only when
/// over 1 m off it.
pub(super) fn leg_point(r: &Route, pos: Vec3, speed: f32) -> Vec3 {
    let sp = sub_point(r, pos);
    if r.corner().is_some() {
        return sp;
    }
    let Some((a, dir, len, along, _)) = line_from(r, seg_from(r), pos) else {
        return sp;
    };
    let t = along.max(0.0) + speed + LEG_LEAD;
    if t >= len {
        return sp;
    }
    let q = a + dir * t;
    Vec3::new(q.x, pos.y, q.y)
}

/// The stuck window's re-plan (`route::progress`; card MOVR lifted it
/// here, unchanged): plan the leg again from the hub the unit stands in. Beside the arc it would leave that hub on, and not at the hub:
/// to the hub's centre first (once — an approach leg that sticks re-plans
/// the arc); card HUBP: and on the arc's own line, centre to centre, not at
/// another drawn point. The corner walk has already failed on this leg, so
/// it is DROPPED for the straight line and the wall probe: a barrier corner
/// is a point in a 2-D rectangle world and nothing says a unit can stand
/// there (BF1 NAB2 unit 25, 6.6 m from a corner for 49,440 ticks); the PS2
/// never commits to a corner either — its walk step re-aims at one per STEP
/// (`FUN_00250fc8`). Answers whether a leg was planned.
pub(super) fn replan(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    pos: Vec3,
    nt: &mut NavTick,
) -> bool {
    let Some(r) = st.nav.route else { return false };
    let Some(g) = sim.nav.as_ref() else {
        return false;
    };
    let Some(cur) = start_hub(sim, g, me) else {
        return false;
    };
    let Some(mut next) = plan_leg(sim, st, me, cur, r.dest_hub, None, nt) else {
        return false;
    };
    centre_leg(sim, g, &mut next, pos);
    if r.via_arc.is_some() {
        let off = off_arc_line(&next, pos);
        let at = g.hub(cur).map(|h| nav::xz_dist(pos, h.pos));
        if cur != r.dest_hub
            && off.is_some_and(|d| d > ARC_LINE_TOL)
            && at.is_some_and(|d| d > ARRIVE_MIN_RADIUS)
        {
            next = approach_leg(sim, g, cur, r.dest_hub, pos);
        }
    }
    next.stuck_run = r.stuck_run;
    next.sidestep = r.sidestep;
    next.detour_len = 0;
    next.detour_at = 0;
    next.ref_dist = nav::xz_dist(pos, sub_point(&next, pos));
    st.nav.route = Some(next);
    true
}

/// An arc leg's line and where `pos` stands beside it: the leg's start
/// ([`Route::from`]), the unit direction to its waypoint, the line's length,
/// the unit's distance along it and its distance off it. `None` on a leg
/// with no arc.
fn arc_line(r: &Route, pos: Vec3) -> Option<(Vec2, Vec2, f32, f32, f32)> {
    line_from(r, r.from, pos)
}

/// Where the stretch of the leg the unit is on starts (card MOUTH): the last
/// detour corner behind it, else the leg's own start ([`Route::from`]) — the
/// PS2 follower's "node just reached". Only [`leg_point`] asks: the stuck
/// re-plan ([`off_arc_line`]) and the arrival plane (`hub_reached`) keep the
/// leg's own line (the plane from the last corner was measured twice and not
/// kept: cards OBST2 and MOUTH).
fn seg_from(r: &Route) -> Vec2 {
    let at = usize::from(r.detour_at)
        .min(usize::from(r.detour_len))
        .min(DETOUR_MAX);
    if at > 0 {
        r.detour[at - 1]
    } else {
        r.from
    }
}

/// [`arc_line`] for the line that starts at `from`.
fn line_from(r: &Route, from: Vec2, pos: Vec3) -> Option<(Vec2, Vec2, f32, f32, f32)> {
    r.via_arc?;
    let (a, b) = (from, Vec2::new(r.waypoint.x, r.waypoint.z));
    let len = (b - a).length();
    if len < 1e-3 {
        return None;
    }
    let dir = (b - a) / len;
    let rel = Vec2::new(pos.x, pos.z) - a;
    let along = rel.dot(dir);
    Some((a, dir, len, along, (rel - dir * along).length()))
}

/// How far `pos` is off an arc leg's line; `None` on a leg with no arc.
pub(super) fn off_arc_line(r: &Route, pos: Vec3) -> Option<f32> {
    arc_line(r, pos).map(|l| l.4)
}

/// A leg TO hub `cur` itself, with no arc: the re-plan of a unit that stands
/// near `cur` but beside the line of the arc it would leave on, and has
/// already been stuck for a window walking that arc from where it stood (a
/// corner between a platform's rail and a ramp's end post, BF2 KAM1). From
/// the hub's centre the arc's line is the walkway.
pub(super) fn approach_leg(sim: &Sim, g: &NavGraph, cur: u16, dest: u16, pos: Vec3) -> Route {
    let waypoint = hub_waypoint(sim, g, cur, pos.y);
    Route {
        dest_hub: dest,
        next_hub: cur,
        via_arc: None,
        waypoint,
        ref_dist: nav::xz_dist(pos, waypoint),
        from: Vec2::new(pos.x, pos.z),
        source: RouteSource::Table,
        ..Route::default()
    }
}

/// One hop from `cur` toward `dest`: the authored table first, the port's BFS
/// when the table is empty, a straight line at the goal when neither answers.
/// Every leg it produces carries its corner walk ([`fill_detour`]).
pub(crate) fn plan_leg(
    sim: &Sim,
    st: &AiState,
    me: UnitHandle,
    cur: u16,
    dest: u16,
    arrival_arc: Option<u16>,
    nt: &mut NavTick,
) -> Option<Route> {
    let g = sim.nav.as_ref()?;
    let u = sim.unit(me)?;
    let pos = u.soldier.pos;
    let m = mover(sim, u);
    let last = |source| Route {
        dest_hub: dest,
        next_hub: dest,
        via_arc: None,
        waypoint: goal_pos(sim, g, st, dest, pos.y),
        ref_dist: 0.0,
        from: Vec2::new(pos.x, pos.z),
        source,
        ..Route::default()
    };
    if cur == dest {
        let mut r = last(RouteSource::Table);
        fill_detour(g, m.ty, pos, &mut r, nt);
        return Some(r);
    }
    let r = route_rand(me.index, sim.tick, me.gen);
    // The SAME-LEVEL test every candidate hop has to pass: is there ground at
    // the unit's own level under that hub (PLAN "PHASE A.2 DESIGN" decision
    // 3)? It is a collision query, so it is only asked of hops the chooser
    // actually offers — never of the whole graph.
    // Card TGAP: a soldier on foot is sent into no hub he cannot walk back
    // out of (`Sim::hub_trapped`) unless his goal is in it or he is in one.
    let walker = m.ty == nav::SOLDIER && !m.jet;
    let trap = |h: u16| walker && h != dest && !sim.hub_trapped(cur) && sim.hub_trapped(h);
    let floor = |h: u16| hub_has_floor(sim, g, h, pos.y) && !trap(h);
    let dead = |arc: u16| walker && sim.arc_unwalkable(arc, cur);
    // Card ARCM: an arc closed to this bot or shunned by its team is drawn
    // round while another way exists — first the whole choice below with
    // those arcs off, then (nothing else found) as if none were. A soldier
    // on foot only, like the floor tests.
    let team = u.team;
    let shunning = walker && (st.arcm.any_closed(sim.tick) || sim.any_shunned(team));
    // A bot already drawn round one keeps to the fewest-arcs way without
    // them, hub by hub, to the same goal: the authored table at the next hub
    // may lead back to the shunned arc, and table and detour taken in turn
    // went round in loops (BF1 Rhen Var 1: 32 recycles in 6 seeds, against 2).
    let detouring = shunning
        && st
            .nav
            .route
            .is_some_and(|r| r.source == RouteSource::Shun && r.dest_hub == dest);
    let hop = if shunning {
        let shun = |a: u16| super::arcm::shunned(sim, st, team, a);
        let kept = detouring
            .then(|| {
                g.route_bfs_avoiding(cur, dest, m, &floor, &|a| dead(a) || shun(a))
                    .filter(|&(_, n)| floor(n) || n == dest)
            })
            .flatten();
        let mut trial = *nt;
        match kept.map(|h| (h, RouteSource::Shun)).or_else(|| {
            choose_hop(
                sim,
                g,
                st,
                cur,
                dest,
                arrival_arc,
                m,
                r,
                &floor,
                &dead,
                &shun,
                &mut trial,
            )
        }) {
            Some(mut h) => {
                *nt = trial;
                let first = choose_hop(
                    sim,
                    g,
                    st,
                    cur,
                    dest,
                    arrival_arc,
                    m,
                    r,
                    &floor,
                    &dead,
                    &|_| false,
                    &mut NavTick::default(),
                );
                if first.is_some_and(|f| f.0 .0 != h.0 .0) {
                    nt.shun_round = nt.shun_round.saturating_add(1);
                    h.1 = RouteSource::Shun;
                }
                Some(h)
            }
            None => {
                let h = choose_hop(
                    sim,
                    g,
                    st,
                    cur,
                    dest,
                    arrival_arc,
                    m,
                    r,
                    &floor,
                    &dead,
                    &|_| false,
                    nt,
                );
                if h.is_some() {
                    nt.shun_kept = nt.shun_kept.saturating_add(1);
                }
                h
            }
        }
    } else {
        choose_hop(
            sim,
            g,
            st,
            cur,
            dest,
            arrival_arc,
            m,
            r,
            &floor,
            &dead,
            &|_| false,
            nt,
        )
    };
    let Some(((arc, next), source)) = hop else {
        nt.straight += 1;
        let mut r = last(RouteSource::Straight);
        fill_detour(g, m.ty, pos, &mut r, nt);
        return Some(r);
    };
    // Card HUBP: a soldier on foot walks at its own point of the next hub. A
    // vehicle keeps the centre (OURS, PROVISIONAL: the drivers' laws were
    // measured against centre legs).
    let waypoint = if u.mount.is_none() {
        hub_point(sim, g, me, next, pos, 0.0)
    } else {
        hub_waypoint(sim, g, next, pos.y)
    };
    let mut r = Route {
        dest_hub: dest,
        next_hub: next,
        via_arc: Some(arc),
        waypoint,
        ref_dist: nav::xz_dist(pos, waypoint),
        // The arc's own end: a leg planned from wherever the unit stands (a
        // spawn, a new objective, a re-plan) is walked onto the walkway
        // first ([`leg_point`]). A leg planned on arrival starts at the
        // waypoint just reached ([`advance_route`]).
        from: g.hub(cur).map_or(Vec2::new(pos.x, pos.z), |h| h.xz()),
        source,
        ..Route::default()
    };
    fill_detour(g, m.ty, pos, &mut r, nt);
    r.ref_dist = nav::xz_dist(pos, sub_point(&r, pos));
    Some(r)
}

/// [`plan_leg`]'s choice of the next hop from `cur`, the authored table
/// first, with every arc `shun` names treated as one the floor walk found
/// unwalkable (card ARCM; `|_| false` = today's choice).
#[allow(clippy::too_many_arguments)]
fn choose_hop(
    sim: &Sim,
    g: &NavGraph,
    st: &AiState,
    cur: u16,
    dest: u16,
    arrival_arc: Option<u16>,
    m: nav::Mover,
    r: u32,
    floor: &dyn Fn(u16) -> bool,
    unwalkable: &dyn Fn(u16) -> bool,
    shun: &dyn Fn(u16) -> bool,
    nt: &mut NavTick,
) -> Option<(nav::Hop, RouteSource)> {
    let walker = m.ty == nav::SOLDIER && !m.jet;
    let table = |exclude: Option<u16>| g.next_hop(cur, dest, m, arrival_arc, exclude, r);
    let bfs = |on_floor: &dyn Fn(u16) -> bool| g.route_bfs_avoiding(cur, dest, m, on_floor, shun);
    let dead = |arc: u16| unwalkable(arc) || shun(arc);
    // Card FLNK: a soldier on foot with the Flank role (`team::board`) takes
    // the next hub of its flank route while it stands on one of the route's
    // hubs, and off them the port's BFS toward the route's nearest hub
    // (`FlankTable::join`); the hop passes the tests below, else the
    // authored route.
    let flank = st
        .flank
        .route
        .filter(|_| walker && st.flank.to == dest)
        .and_then(|k| {
            let k = usize::from(k);
            sim.flank.next(k, cur).or_else(|| {
                let (mut hops, mut queue) = (Vec::new(), Vec::new());
                g.hop_distances_into(cur, m, &mut hops, &mut queue);
                let j = sim.flank.join(k, &hops)?;
                g.route_bfs(cur, j, m, &floor)
            })
        })
        .filter(|&(a, n)| {
            g.arcs.get(usize::from(a)).is_some_and(|x| x.allows(m)) && floor(n) && !dead(a)
        });
    let hop = match (flank, table(None)) {
        (Some(h), _) => {
            nt.flank += 1;
            Some((h, RouteSource::Flank))
        }
        (None, Some(h)) => Some((h, RouteSource::Table)),
        (None, None) => match bfs(&|_| true) {
            Some(h) => {
                nt.bfs += 1;
                Some((h, RouteSource::Bfs))
            }
            None => None,
        },
    };
    // A hop whose far hub has no floor on this level is REJECTED here, and the
    // alternatives are tried in the design's order: the chooser again with
    // that arc off the table, then the BFS restricted to hubs that DO have a
    // floor, then the PS2's own "any other arc of this hub"
    // (`NavGraph::any_other_arc`, `FUN_002659e8`). Only when all three fail
    // does the unit walk the straight line, and that is `level_unrouted`.
    // Card BNDS: an arc the floor walk found no soldier can walk from here
    // (open air on its line, a far end 8 m up, card TGAP: a wall) is
    // rejected the same way, and so is a hop into a hub a soldier is
    // trapped in (`trap`).
    // Card ARCM: a hop rejected only because it is shunned is not handed
    // "any other arc" (that one goes nowhere in particular): with no other
    // way the caller asks again without the shunned arcs.
    match hop {
        Some(((arc, next), _)) if !floor(next) || dead(arc) => {
            let level = !floor(next) || unwalkable(arc);
            if level {
                nt.level_rejected += 1;
            }
            let retry = table(Some(arc))
                .filter(|&(a, n)| floor(n) && !dead(a))
                .map(|h| (h, RouteSource::Table))
                .or_else(|| {
                    bfs(&floor).filter(|&(a, _)| !dead(a)).map(|h| {
                        nt.bfs += 1;
                        (h, RouteSource::Bfs)
                    })
                })
                .or_else(|| {
                    g.any_other_arc(cur, arrival_arc, Some(arc), m, r)
                        .filter(|&(a, n)| level && floor(n) && !dead(a))
                        .map(|h| (h, RouteSource::Table))
                });
            if retry.is_none() && level {
                nt.level_unrouted += 1;
            }
            retry
        }
        h => h,
    }
}

/// Route advance, now that an ENGAGED unit walks its route too (design
/// decision 4): first the detour corners behind us (XZ, CORNER_ARRIVE),
/// then the hub itself at its own radius (at least `min_arrive` — a hover
/// driver's ForwardSpeed × 0.5, card VAI), at which the chooser picks the
/// leg after it.
pub(super) fn advance_route(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    pos: Vec3,
    min_arrive: f32,
    nt: &mut NavTick,
) {
    if let (Some(g), Some(mut r)) = (sim.nav.as_ref(), st.nav.route) {
        let mut moved_on = false;
        // Card OBSV: a corner or a waypoint a vehicle stands on is reached at
        // the vehicle's side ([`super::avoid::at_foot_of`]) — by a soldier on
        // foot; a driver's own vehicle stands on every point it comes to.
        // Card HJAM: a hover's driver reaches a corner at its node radius.
        let on_foot = sim.unit(me).is_some_and(|u| u.mount.is_none());
        let walker = sim
            .unit(me)
            .and_then(|u| u.mount)
            .and_then(|m| sim.vehicle(m.vehicle))
            .is_some_and(|v| v.kind == crate::vehicle::VehicleKind::Walker);
        let reach = corner_arrive(sim, me, min_arrive);
        while r.corner().is_some_and(|c| {
            (Vec2::new(pos.x, pos.z) - c).length() <= reach
                || (on_foot && super::avoid::at_foot_of(sim, c, pos, CORNER_ARRIVE))
                || (walker && {
                    let incoming = c - r.from;
                    let past = Vec2::new(pos.x, pos.z) - c;
                    let lateral = incoming.perp_dot(past);
                    let limit = WALKER_CORNER_LATERAL_RADII * reach;
                    incoming.dot(past) > 0.0
                        && lateral * lateral <= limit * limit * incoming.length_squared()
                })
        }) {
            if walker {
                // A walker follows every segment, including the approach to a
                // corner. Keep the reached node even when fill_detour refills.
                r.from = r.corner().expect("tested corner");
            }
            r.detour_at = r.detour_at.saturating_add(1);
            moved_on = true;
        }
        // Card OBST2: a corner walk that filled the leg's array may have been
        // cut short (`fill_detour`): from its last corner the rest is
        // searched for. (BF2 Mustafar's leg 30 -> 3 goes round a pit by 11
        // corners; after the sixth the walk went straight at the waypoint,
        // into the pit: 23 recycles there in 8 rounds.) Not on a leg the
        // stuck window re-planned: that one dropped its corners on purpose.
        if moved_on
            && r.corner().is_none()
            && usize::from(r.detour_len) == DETOUR_MAX
            && r.stuck_run == 0
        {
            if let Some(u) = sim.unit(me) {
                fill_detour(g, mover(sim, u).ty, pos, &mut r, nt);
            }
        }
        if moved_on {
            // A new sub-waypoint opens a new progress window: the old
            // reference distance was measured to the corner just passed.
            r.ref_dist = nav::xz_dist(pos, sub_point(&r, pos));
            r.stuck_ticks = 0;
        }
        st.nav.route = Some(r);
        let at_foot = on_foot
            && r.corner().is_none()
            && sim
                .nav_floor(r.next_hub)
                .is_none_or(|f| (f - pos.y).abs() <= HUB_LEVEL_TOL)
            && super::avoid::at_foot_of(
                sim,
                Vec2::new(r.waypoint.x, r.waypoint.z),
                pos,
                ARRIVE_MIN_RADIUS,
            );
        if !r.is_last()
            && (!walker || r.corner().is_none())
            && (at_foot || hub_reached(sim, g, &r, pos, min_arrive))
        {
            if let Some(mut next) = plan_leg(sim, st, me, r.next_hub, r.dest_hub, r.via_arc, nt) {
                // Side-step state and the stuck run survive the hop. The
                // progress window does not (card MOUTH): its reference
                // distance is the new leg's, measured this tick, so it opens
                // anew as it does past a corner. (Until then its tick count
                // was carried: a hop in the window's last ticks closed it on
                // a tick or two of the new leg, under [`STUCK_PROGRESS`] at a
                // run — a third to a half of all failed windows on BF2 Kamino
                // and Mygeeto, each a re-plan and a second's side-step for a
                // bot at full speed. At a ramp's foot that side-step put BF1
                // Kamino's bots outside the rail.)
                next.sidestep = r.sidestep;
                next.stuck_run = r.stuck_run;
                // Card HUBP: the PS2's leg — from the waypoint just reached
                // to the unit's point of the next hub.
                if next.via_arc.is_some() || walker {
                    next.from = Vec2::new(r.waypoint.x, r.waypoint.z);
                }
                st.nav.route = Some(next);
            }
        }
    }
}

/// Is the leg's hub reached? The PS2's rule (RE_LOG "Arrival and locate",
/// BF1 `FUN_0026a780` / `FUN_0026a878`, BF2 the same shape): within the
/// mover's node radius of the WAYPOINT in XZ, or past the waypoint's plane
/// along the leg — "the hub sphere never decides arrival". The port used the
/// hub's authored radius (12–15 m on BF2 KAM1's platforms, 2–4 m at its
/// walkway mouths), so a unit turned for the next hub metres short of this
/// one and off the next arc's line: at a ramp mouth that is the rail beside
/// the ramp (the maintainer, 2026-10-01: 20 of 32 units stuck there after 200 s).
/// The node radius is [`ARRIVE_MIN_RADIUS`] (PROVISIONAL: the PS2's is 1.0 m
/// for a soldier — measured by card MOVR2, it passes alone and with the
/// PS2's leg aim it doubles the "lost" deaths: bot-design 9, D18), or the
/// caller's `min_arrive` (a hover driver's). The leg
/// is the route's own line ([`Route::from`] to the waypoint, card HUBP), and
/// the plane is the waypoint's - also while a corner walk is on, which the
/// PS2's mover takes node by node (measured and NOT kept, card OBST2: the
/// plane of the path's last stretch, and none while a corner is ahead -
/// The maintainer's Kamino scene 7 -> 11 of about 400 trails over 6 reversals, no
/// gain over 24 seeds of four maps). OURS: the stuck unit's fallback to the
/// hub's radius.
///
/// Card BNDS: a hub is reached ON ITS LEVEL — a unit under a walkway's hub
/// (BF2 Kamino's hub 123 at 63.7 over a deck at 51.5) is not at it, and the
/// leg after it cannot be walked from there. Past a stuck window the radius
/// fallback below still lets it go.
fn hub_reached(sim: &Sim, g: &NavGraph, r: &crate::ai::Route, pos: Vec3, min_arrive: f32) -> bool {
    let d = nav::xz_dist(pos, r.waypoint);
    let level = sim
        .nav_floor(r.next_hub)
        .is_none_or(|f| (f - pos.y).abs() <= HUB_LEVEL_TOL);
    if d <= ARRIVE_MIN_RADIUS.max(min_arrive) && level {
        return true;
    }
    // A unit that has spent a stuck window on this leg and stands inside the
    // hub's authored radius has arrived as it used to: a hub whose centre
    // cannot be stood on (BF2 POL1's hub 152, 3.0 m away for 51,600 ticks)
    // must not hold it for good.
    if r.stuck_run > 0 && d <= g.radius(r.next_hub) {
        return true;
    }
    if r.via_arc.is_none() {
        return false;
    }
    let b = Vec2::new(r.waypoint.x, r.waypoint.z);
    level && (b - r.from).dot(Vec2::new(pos.x, pos.z) - b) > 0.0
}

mod hubs;
pub use hubs::*;
mod retarget;
pub use retarget::*;

#[cfg(test)]
mod tests;
