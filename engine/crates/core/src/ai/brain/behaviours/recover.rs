//! Returning to the playable level and giving up a repeatedly failed walk.

use crate::ai::state::{AiState, LostState, NavTick, SteerPick};
use crate::ai::why::Why;
use crate::ai::{
    ride, GIVE_UP_AFTER, HULL_PUSH_MIN, LOST_JUMP_AGAIN, LOST_JUMP_CLEAR, LOST_RECYCLE, PIN_FAILS,
    PROBE_DIST, RECYCLE_AFTER, RECYCLE_ROAM, RETURN_ARRIVE, RETURN_ARRIVE_DY, RETURN_PLANS,
    SHUT_PROBE_UP, SHUT_RECYCLE, STUCK_PROGRESS, STUCK_WINDOW,
};
use crate::nav;
use crate::sim::Sim;
use crate::unit::Unit;
use glam::Vec3;

pub(in crate::ai) fn think(sim: &Sim, u: &Unit, st: &mut AiState) {
    finish_approach_walk(sim, u, st);
    if let Some(walk) = approach_walk(sim, u, st) {
        st.stall = None;
        st.lost = Some(walk);
        return;
    }

    // Card BNDS: is this bot still on the playable level? Asked on the
    // ground only (a faller keeps what it had); two thinks off running
    // start the return walk, one think on ends it.
    // Card VMESH: one shut inside a WATERTIGHT map body (a closed prop,
    // never a room, cave or tunnel: those have open edges and bots walk
    // in them) is lost too, at once and whatever it is doing; it is
    // given up after `SHUT_RECYCLE`.
    let shut = u.mount.is_none()
        && sim
            .shut_in_solid(u.soldier.pos + Vec3::Y * SHUT_PROBE_UP)
            .is_some();
    if shut {
        st.lost = Some(match st.lost.filter(|l| l.shut) {
            Some(l) => l,
            None => LostState {
                since: sim.tick,
                confirmed: true,
                shut: true,
                ..LostState::default()
            },
        });
    } else if st.lost.is_some_and(|l| l.hunt_rejoin)
        && u.soldier.grounded
        && st
            .route()
            .is_some_and(|r| r.source != crate::ai::state::RouteSource::Straight)
        && sim.level_at(u.soldier.pos) == crate::sim::Level::On
    {
        // The Navigator found a leg from the walkable floor again.
        st.lost = None;
        st.stall = None;
        st.mv.round.end();
        st.slide = 0;
    } else if st.lost.is_some_and(|l| l.pinned) {
        // Card RLOOP: a pinned bot's walk is not the level test's; it
        // ends with the walk (below).
    } else if u.mount.is_none() && u.soldier.grounded {
        st.lost = match (sim.level_at(u.soldier.pos), st.lost.filter(|l| !l.shut)) {
            (crate::sim::Level::Off(_), None) => Some(LostState {
                since: sim.tick,
                ..LostState::default()
            }),
            (crate::sim::Level::Off(_), Some(l)) => Some(LostState {
                confirmed: true,
                ..l
            }),
            _ => None,
        };
    }
    // Card RETW: a confirmed spell looks for the FLOOR walk back once
    // (`Sim::return_path`), and again from where it stands when the walk
    // is spent with the bot still off — its kept points walked, or a
    // progress window with no progress (`mover::run`) —
    // [`RETURN_PLANS`] times in all; then the hubs.
    if let Some(mut l) = st.lost.filter(|l| l.confirmed && !l.shut && !l.pinned) {
        let spent = l.path.len > 0 && l.path_at >= l.path.len;
        if l.plans == 0 || (spent && l.plans < RETURN_PLANS) {
            l.plans += 1;
            l.path = sim.return_path(u.soldier.pos).unwrap_or_default();
            l.path_at = 0;
            st.lost = Some(l);
        }
    }
}

pub(in crate::ai) fn goal(
    sim: &Sim,
    st: &mut AiState,
    pos: Vec3,
    nt: &mut NavTick,
) -> Option<Vec3> {
    // Card BNDS: a bot off the playable level walks BACK to it before
    // anything else — at the nearest hub it can stand level with
    // (`Sim::return_point`), the next one after a window with no progress.
    // Off it for `LOST_RECYCLE` with no way back, it is given up.
    // Card RETW: ...along the floor walk found for it when there is one
    // (each of its points reached in turn), and only then at a hub.
    if let Some(mut l) = st.lost.filter(|l| l.confirmed) {
        while l.path_at < l.path.len && {
            let p = l.path.pts[usize::from(l.path_at)];
            nav::xz_dist(pos, p) <= RETURN_ARRIVE && (pos.y - p.y).abs() <= RETURN_ARRIVE_DY
        } {
            // (A fresh progress window for the next point.)
            l.path_at += 1;
            l.window = 0;
            l.ref_dist = 0.0;
        }
        st.lost = Some(l);
    }
    // Card RLOOP: a pinned bot at the end of its walk is at its waypoint;
    // its route goes on from there.
    if let Some(l) = st.lost.filter(|l| l.pinned && l.path_at >= l.path.len) {
        if l.replan_after_escape {
            crate::ai::route::clear(st);
            st.mv.round.end();
            st.slide = 0;
        }
        st.lost = None;
    }
    let lost = st.lost.filter(|l| l.confirmed);
    let ret = lost.and_then(|l| {
        if l.path_at < l.path.len {
            Some(l.path.pts[usize::from(l.path_at)])
        } else {
            sim.return_point(pos, l.tries)
        }
    });
    if let Some(l) = lost {
        let after = if l.shut { SHUT_RECYCLE } else { LOST_RECYCLE };
        let over = sim.tick.saturating_sub(l.since) >= after;
        if l.pinned {
            // (On the level: given up as the last resort's, not as lost.)
            nt.stalled |= u32::from(over);
        } else {
            nt.lost = 1;
            nt.recycle |= u32::from(over);
        }
    }
    ret
}

/// Give a stranded ground bot one physical way back before the last resort.
pub(in crate::ai) fn rejoin(sim: &Sim, me: crate::unit::UnitHandle, st: &mut AiState) -> bool {
    let Some(u) = sim.unit(me) else { return false };
    let Some(mut stall) = st.stall else {
        return false;
    };
    let pos = u.soldier.pos;
    let escape = floor_escape_at(sim, pos);
    let replans = escape.is_some_and(|e| {
        matches!(
            e.headings,
            nav::EscapeHeadings::Compass(_) | nav::EscapeHeadings::CompassTo { .. }
        )
    });
    let retry = replans
        && st
            .lost
            .is_some_and(|l| l.pinned && l.escape_heading.is_none());
    if !u.soldier.grounded
        || u.soldier.stance != crate::soldier::Stance::Stand
        || u.soldier.lock_timer > 0.0
        || u.droideka.is_some()
        || u.mount.is_some()
        || (!retry
            && (st.lost.is_some()
                || stall
                    .walked
                    .is_some_and(|p| pos.distance_squared(p) < crate::ai::PIN_WALKED.powi(2))))
    {
        return false;
    }
    let hunt = st.route().is_none()
        && sim
            .commander
            .get(usize::from(u.team))
            .and_then(|p| p.of(me.index))
            == Some(crate::commander::Objective::Hunt);
    let hunt_path = hunt
        .then(|| crate::ai::route::hunt_rejoin(sim, me))
        .flatten();
    // OURS: the long stall watch can expire while the leg is still moving
    // in circles and has never been offered the pinned floor walk.
    // measure: the Cloud City lower approach at the watch bound (#108).
    let watched_path = || {
        let goal = st
            .route()
            .filter(|_| stall.ticks >= RECYCLE_AFTER)?
            .waypoint;
        if pos.distance_squared(goal) > RECYCLE_ROAM * RECYCLE_ROAM
            || (goal.y - pos.y).abs() > crate::collision::STEP_HEIGHT
            || !sim.floor_line(pos, goal)
        {
            return None;
        }
        sim.floor_walk_to(pos, goal)
    };
    let walk = || {
        let (path, heading) = walk_out(sim, u, escape?, None, false, false, None)?;
        Some((path, Some(heading)))
    };
    let next = if retry {
        walk()
    } else {
        hunt_path
            .or_else(watched_path)
            .map(|path| (path, None))
            .or_else(walk)
            .or_else(|| descent(sim, u).map(|path| (path, None)))
    };
    let Some((path, escape_heading)) = next else {
        return false;
    };
    st.lost = Some(LostState {
        since: sim.tick,
        confirmed: true,
        path,
        plans: RETURN_PLANS,
        pinned: true,
        hunt_rejoin: !retry && hunt_path.is_some(),
        escape_heading,
        replan_after_escape: replans && escape_heading.is_some(),
        ..LostState::default()
    });
    stall.ticks = 0;
    stall.fails = 0;
    stall.walked = Some(pos);
    st.stall = Some(stall);
    true
}

/// OURS: join a marked corridor from its walkable entry. A replan can start
/// a bot under the walkway and steer it farther into the obstruction.
/// measure: the BF2 Yavin Bazaar H83/H84 approaches (#108).
pub(in crate::ai) fn join_arc_floor(sim: &Sim, u: &Unit, st: &mut AiState) {
    if st.lost.is_some() || !u.soldier.grounded || u.mount.is_some() || u.droideka.is_some() {
        return;
    }
    let Some(r) = st
        .route()
        .filter(|r| r.stuck_ticks == 0 && r.stuck_run == 0)
    else {
        return;
    };
    let Some(g) = sim.nav.as_ref() else { return };
    let Some(arc) = r
        .via_arc
        .filter(|i| g.floor_join_arcs.contains(i))
        .and_then(|i| g.arcs.get(usize::from(i)))
    else {
        return;
    };
    let entry = arc.other(r.next_hub);
    let pos = u.soldier.pos;
    let (Some(from_y), Some(to_y)) = (sim.nav_floor(entry), sim.nav_floor(r.next_hub)) else {
        return;
    };
    if from_y.min(to_y) - pos.y <= crate::sim::BOUND_DROP + crate::ai::HUB_LEVEL_TOL
        || (from_y - to_y).abs() > crate::ai::HUB_LEVEL_TOL
    {
        return;
    }
    let Some(hub) = g.hub(entry) else { return };
    let from = Vec3::new(hub.pos.x, from_y, hub.pos.z);
    let Some(path) = sim.floor_walk_to(pos, from) else {
        return;
    };
    st.lost = Some(LostState {
        since: sim.tick,
        confirmed: true,
        path,
        plans: RETURN_PLANS,
        pinned: true,
        ..LostState::default()
    });
}

fn finish_approach_walk(sim: &Sim, u: &Unit, st: &mut AiState) {
    let Some(walk) = st.lost.as_mut().filter(|l| l.escape_arrival_plane) else {
        return;
    };
    let Some(heading) = walk.escape_heading else {
        return;
    };
    let pos = u.soldier.pos;
    if u.soldier.grounded
        && (pos - walk.path.pts[0]).dot(heading) >= 0.0
        && sim.level_at(pos) == crate::sim::Level::On
        && sim.soldier_clear(pos)
        && sim.shut_in_solid(pos + Vec3::Y * SHUT_PROBE_UP).is_none()
    {
        walk.path_at = walk.path.len;
    }
}

fn approach_walk(sim: &Sim, u: &Unit, st: &AiState) -> Option<LostState> {
    let approaches = &sim.nav.as_ref()?.floor_approaches;
    if approaches.is_empty()
        || u.mount.is_some()
        || !u.soldier.grounded
        || u.soldier.stance != crate::soldier::Stance::Stand
        || u.soldier.lock_timer > 0.0
    {
        return None;
    }
    let hub = st.route().map(|r| r.next_hub);
    let approach = approaches.iter().find(|a| {
        if !contains(a.escape, u.soldier.pos) {
            return false;
        }
        let approaching_hub = st.lost.is_none() && hub == Some(a.escape.hub);
        if u.droideka.is_some() != a.motion.droideka() {
            return false;
        }
        match a.motion {
            nav::ApproachMotion::Any
            | nav::ApproachMotion::Droideka
            | nav::ApproachMotion::DroidekaApproaching(_) => approaching_hub,
            nav::ApproachMotion::Stalled { .. } => {
                approaching_hub && st.route().is_some_and(|r| r.stuck_run >= 3 * STUCK_WINDOW)
            }
            nav::ApproachMotion::PinnedHop { .. } => {
                hub == Some(a.escape.hub)
                    && st.lost.is_some_and(|l| {
                        l.pinned
                            && l.escape_heading.is_none()
                            && sim.tick.saturating_sub(l.since) >= u64::from(3 * STUCK_WINDOW / 2)
                    })
            }
            nav::ApproachMotion::YieldToHover(_) => {
                approaching_hub && approaching_hover(sim, u.soldier.pos)
            }
            nav::ApproachMotion::PinnedWalk(_) | nav::ApproachMotion::DroidekaPinned { .. } => {
                let wait = match a.motion {
                    nav::ApproachMotion::DroidekaPinned { min_windows, .. } => {
                        u64::from(min_windows) * u64::from(STUCK_WINDOW)
                    }
                    _ => 0,
                };
                hub == Some(a.escape.hub)
                    && st.lost.is_some_and(|l| {
                        l.pinned
                            && l.escape_heading.is_none()
                            && !l.replan_after_escape
                            && sim.tick.saturating_sub(l.since) >= wait
                    })
            }
            nav::ApproachMotion::DroidekaStalled(_)
            | nav::ApproachMotion::DroidekaStalledDescent(_) => {
                (st.lost.is_none() && st.route().is_some_and(|r| r.stuck_run >= STUCK_WINDOW))
                    || (st
                        .lost
                        .is_some_and(|l| l.pinned && l.escape_heading.is_none())
                        && st.stall.is_some_and(|s| {
                            s.ticks >= u32::from(PIN_FAILS) * u32::from(STUCK_WINDOW)
                        }))
            }
            nav::ApproachMotion::Sliding => approaching_hub && st.slide != 0,
            nav::ApproachMotion::Toward(dir) => approaching_hub && u.soldier.vel.dot(dir) > 0.0,
            nav::ApproachMotion::AlongArc { arc, velocity, .. } => {
                st.lost.is_none()
                    && st.route().is_some_and(|r| r.via_arc == Some(arc))
                    && u.soldier.vel.dot(velocity) > 0.0
            }
            nav::ApproachMotion::ReturningOnArc(arc) => {
                st.route()
                    .is_some_and(|r| r.via_arc == Some(arc) && r.next_hub == a.escape.hub)
                    && st.lost.is_some_and(|l| l.confirmed && !l.pinned && !l.shut)
            }
            nav::ApproachMotion::Returning | nav::ApproachMotion::DroidekaReturning(_) => {
                st.lost.is_some_and(|l| l.confirmed && !l.pinned && !l.shut)
            }
        }
    })?;
    let exit = match approach.motion {
        nav::ApproachMotion::Stalled { exit, .. }
        | nav::ApproachMotion::PinnedHop { exit, .. }
        | nav::ApproachMotion::YieldToHover(exit)
        | nav::ApproachMotion::DroidekaApproaching(exit)
        | nav::ApproachMotion::PinnedWalk(exit)
        | nav::ApproachMotion::DroidekaStalled(exit)
        | nav::ApproachMotion::DroidekaStalledDescent(exit)
        | nav::ApproachMotion::DroidekaReturning(exit)
        | nav::ApproachMotion::DroidekaPinned { exit, .. }
        | nav::ApproachMotion::AlongArc { exit, .. } => Some(exit),
        _ => None,
    };
    let descent = matches!(
        approach.motion,
        nav::ApproachMotion::Stalled { descend: true, .. }
            | nav::ApproachMotion::AlongArc { descend: true, .. }
            | nav::ApproachMotion::DroidekaStalledDescent(_)
    ) || matches!(approach.motion, nav::ApproachMotion::DroidekaReturning(to) if to.y < u.soldier.pos.y);
    let jump_at = match approach.motion {
        nav::ApproachMotion::PinnedHop { delay, .. } => Some(delay),
        _ => None,
    };
    let hold_yaw = matches!(
        approach.escape.headings,
        nav::EscapeHeadings::CompassTo { .. }
    );
    let (path, heading) = walk_out(
        sim,
        u,
        approach.escape,
        exit,
        descent,
        hold_yaw
            || matches!(
                approach.motion,
                nav::ApproachMotion::Stalled { .. }
                    | nav::ApproachMotion::PinnedWalk(_)
                    | nav::ApproachMotion::PinnedHop { .. }
                    | nav::ApproachMotion::YieldToHover(_)
            ),
        jump_at,
    )?;
    Some(LostState {
        since: st.lost.map_or(sim.tick, |l| l.since),
        confirmed: true,
        path,
        plans: RETURN_PLANS,
        pinned: true,
        escape_heading: Some(heading),
        escape_jump_tick: jump_at.map(|delay| sim.tick + u64::from(delay)),
        replan_after_escape: true,
        escape_arrival_plane: approach.finish_on_heading_plane,
        escape_hold_yaw: hold_yaw,
        ..LostState::default()
    })
}

fn contains(escape: nav::FloorEscape, pos: Vec3) -> bool {
    escape.bounds.min.cmple(pos).all() && pos.cmple(escape.bounds.max).all()
}

fn approaching_hover(sim: &Sim, pos: Vec3) -> bool {
    use glam::Vec3Swizzles;

    // OURS: use one second of travel in the marked crossing.
    // measure: BF1 Theed closing speed and side step (#108).
    sim.vehicles.iter().enumerate().any(|(i, vehicle)| {
        let advance = vehicle.vel.xz();
        let offset = pos - vehicle.pos;
        vehicle.alive
            && vehicle.kind == crate::vehicle::VehicleKind::Hover
            && offset.y.abs() < 2.0
            && offset.xz().length_squared() <= (advance.length() + 4.0).powi(2)
            && advance.length_squared() > crate::ai::avoid::FOOT_STILL.powi(2)
            && advance.dot(offset.xz()) > 0.0
            && crate::ai::avoid::footprint(sim, i, pos.y).is_some_and(|hull| {
                hull.crosses_r(pos.xz(), pos.xz() - advance, crate::unit::SOLDIER_RADIUS)
            })
    })
}

fn floor_escape_at(sim: &Sim, pos: Vec3) -> Option<nav::FloorEscape> {
    let graph = sim.nav.as_ref()?;
    graph
        .floor_escapes
        .iter()
        .copied()
        .chain(
            graph
                .floor_approaches
                .iter()
                .filter(|a| {
                    !a.motion.droideka()
                        && !matches!(
                            a.motion,
                            nav::ApproachMotion::Stalled { .. }
                                | nav::ApproachMotion::PinnedHop { .. }
                                | nav::ApproachMotion::YieldToHover(_)
                                | nav::ApproachMotion::PinnedWalk(_)
                        )
                })
                .map(|a| a.escape),
        )
        .find(|area| contains(*area, pos))
}

/// Validate the actual standing capsule before selecting a route out of a pocket.
fn walk_out(
    sim: &Sim,
    u: &Unit,
    escape: nav::FloorEscape,
    exit: Option<Vec3>,
    descent: bool,
    require_exit: bool,
    jump_at: Option<u16>,
) -> Option<(crate::sim::ReturnPath, Vec3)> {
    let g = sim.nav.as_ref()?;
    let pos = u.soldier.pos;
    let exit = match escape.headings {
        nav::EscapeHeadings::CompassTo { exit, .. } => Some(exit),
        _ => exit,
    };
    let to = match exit {
        Some(exit) => exit,
        None => g.hub(escape.hub)?.pos.with_y(sim.nav_floor(escape.hub)?),
    };
    let attempt = |heading| {
        let end = if u.droideka.is_some() {
            sim.preview_droideka_walk(u, heading, to, descent)
                .filter(|&end| sim.floor_line(end, to))
        } else {
            sim.preview_recovery_walk(u, heading, descent, require_exit.then_some(to), jump_at)
        }?;
        (end.distance_squared(to) < pos.distance_squared(to)).then_some((end, heading))
    };
    let (end, heading) = match escape.headings {
        nav::EscapeHeadings::AroundSupport => {
            let forward = (to - pos).with_y(0.0).try_normalize()?;
            let side = Vec3::new(-forward.z, 0.0, forward.x);
            [forward - side, forward + side]
                .into_iter()
                .filter_map(|dir| attempt(dir.normalize()))
                .find(|(end, _)| sim.floor_line(*end, to))
        }
        nav::EscapeHeadings::Compass(angles) | nav::EscapeHeadings::CompassTo { angles, .. } => {
            angles
                .iter()
                .filter_map(|a| {
                    let (z, x) = crate::fmath::sin_cos(a.to_radians());
                    let dir = Vec3::new(x, 0.0, z);
                    attempt(dir).filter(|(end, _)| {
                        let offset = *end - pos;
                        let travelled = if descent { offset.with_y(0.0) } else { offset };
                        travelled.normalize_or_zero().dot(dir) >= 0.8
                    })
                })
                .min_by(|(a, _), (b, _)| a.distance_squared(to).total_cmp(&b.distance_squared(to)))
        }
    }?;
    let mut path = crate::sim::ReturnPath::default();
    path.pts[0] = end;
    path.len = 1;
    Some((path, heading))
}

fn descent(sim: &Sim, u: &Unit) -> Option<crate::sim::ReturnPath> {
    let pos = u.soldier.pos;
    // A nearby lower resolved hub is evidence of a playable floor below.
    // Most failed walks are on flat ground and need no descent preview.
    let graph = sim.nav.as_ref()?;
    let reach = u.consts.max_speed * crate::TICK_DT * f32::from(STUCK_WINDOW);
    if !graph.hubs.iter().enumerate().any(|(i, hub)| {
        sim.nav_floor(i as u16).is_some_and(|y| {
            y < pos.y - crate::sim::BOUND_DROP && nav::xz_dist(hub.pos, pos) <= reach + hub.radius
        })
    }) {
        return None;
    }
    let headings = [
        Vec3::Z,
        Vec3::X,
        Vec3::NEG_Z,
        Vec3::NEG_X,
        (Vec3::X + Vec3::Z).normalize(),
        (Vec3::X - Vec3::Z).normalize(),
        (Vec3::NEG_X - Vec3::Z).normalize(),
        (Vec3::NEG_X + Vec3::Z).normalize(),
    ];
    let land = headings
        .into_iter()
        .find_map(|d| sim.recovery_descent(u, d))?;
    let mut path = crate::sim::ReturnPath::default();
    path.pts[0] = land;
    path.len = 1;
    Some(path)
}

/// Card MOVR — RECOVER (`analysis/bot-design.md` 6.3): the walk has FAILED
/// again (the mover's progress window; `MoverState::recover` counts the
/// failures since the walk last made progress). The ladder:
///
/// 1. plan again from the hub the bot stands in and side-step — the window's
///    own cure (`route::progress`, `route::replan`), every failure;
/// 2. (card RETW: back along the floor to a hub it can stand level with);
/// 3. on every [`GIVE_UP_AFTER`]-th failure give the goal up — a boarder
///    drops its claim; any other reports its post and the commander draws
///    again without it (`commander::plan`, `MoverState::give_up`). Not
///    "stand where it is, in the post's region": a bot kept under its post's
///    floor would hold the post contested for good (card STEND);
/// 4. [`PIN_FAILS`] failures at one place, whatever the
///    plan: the bot walks to its waypoint along the floor, or is recycled
///    when there is no such walk (card RLOOP, `stall::send_off`) — this count
///    starts again with every plan that is nearer its waypoint, so two
///    plans taken in turn never reached rung 3;
/// 5. the 120 s recycle (card RCYC, `stall::watch`), counted by the sweep.
pub(super) fn walk_failed(sim: &Sim, st: &mut AiState, nt: &mut NavTick) {
    st.mv.recover = st.mv.recover.saturating_add(1);
    if !st.mv.recover.is_multiple_of(GIVE_UP_AFTER) {
        return;
    }
    if st.ride.is_some_and(|r| r.board.is_some()) {
        ride::give_up(sim, st, nt);
    } else if let (Some(cp), None) = (st.target_cp, st.cmd) {
        st.mv.give_up = cp + 1;
    } else {
        return;
    }
    // Card WHY: the goal given up is this failure's outcome.
    nt.failed(Why::GaveUp);
}

/// Card BNDS: should a lost bot JUMP this tick? Its way `d` is shut by a wall
/// low enough to clear (the sweep of its capsule lifted [`LOST_JUMP_CLEAR`]
/// is open) — a rim rail from outside, a kerb over the step. It jumps from
/// where it stands: the stick steers a jump in the air (card AIRC), and on
/// BF2 Kamino's 50 rim stops a standing jump with the stick held clears
/// every one a running jump does. One try per [`LOST_JUMP_AGAIN`]
/// (`LostState::jump_after`); between tries the steering slides it along.
pub(in crate::ai) fn lost_jump(
    sim: &Sim,
    u: &Unit,
    d: Vec3,
    pick: SteerPick,
    l: &mut LostState,
) -> bool {
    if !u.soldier.grounded || sim.tick < l.jump_after {
        return false;
    }
    let cap = u.capsule();
    let wall_ahead = || {
        sim.collision
            .wall_sweep(&cap.stepped(), d, PROBE_DIST)
            .is_some()
    };
    // OURS: a pinned return can report an open probe while the full capsule
    // cancels its step on a low ledge. Use that physical contact only here.
    // trace: compare a PS2 soldier's escape at the Yavin Bazaar riser.
    let hidden_ledge = pick == SteerPick::Fwd;
    if hidden_ledge {
        if !l.pinned
            || u.droideka.is_some()
            || u.soldier.stance != crate::soldier::Stance::Stand
            || u.wall_push.dot(d) >= -HULL_PUSH_MIN
            || u.soldier.vel.dot(d).abs() * crate::TICK_DT
                >= STUCK_PROGRESS / f32::from(STUCK_WINDOW)
            || wall_ahead()
        {
            return false;
        }
    } else if !wall_ahead() {
        return false; // a drop or a barrier, not a wall
    }
    let mut over = cap;
    over.base += Vec3::Y * LOST_JUMP_CLEAR;
    if (hidden_ledge && sim.collision.ceiling_sweep(&cap, &over).is_some())
        || sim
            .collision
            .wall_sweep(&over, d, PROBE_DIST + 0.5)
            .is_some()
    {
        return false; // too high to jump
    }
    l.jump_after = sim.tick + LOST_JUMP_AGAIN;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::l_sim;
    use crate::collision::Aabb;
    use crate::region::Region;

    #[test]
    fn a_floor_escape_checks_the_whole_walk_for_hazards() {
        let (mut sim, bot) = l_sim();
        let pocket = Aabb {
            min: Vec3::splat(-1.0),
            max: Vec3::splat(1.0),
        };
        let graph = sim.nav.as_mut().unwrap();
        let area = nav::FloorEscape {
            bounds: pocket,
            hub: 1,
            headings: nav::EscapeHeadings::AroundSupport,
        };
        graph.floor_escapes.push(area);
        sim.nav_floors = vec![0.0; graph.hubs.len()];
        sim.unit_mut(bot).unwrap().soldier.grounded = true;

        let escape =
            |sim: &Sim| walk_out(sim, sim.unit(bot).unwrap(), area, None, false, false, None);
        let (path, direction) = escape(&sim).unwrap();
        assert!(path.pts[0].distance(Vec3::ZERO) > crate::ai::PIN_WALKED);
        sim.death_regions.push(Region::sphere(direction * 3.0, 1.0));
        let (_, other) = escape(&sim).unwrap();
        assert!(direction.dot(other).abs() < 0.01);
        sim.death_regions.push(Region::sphere(other * 3.0, 1.0));
        assert!(escape(&sim).is_none());
    }

    #[test]
    fn a_marked_walk_must_reach_the_exit() {
        let (mut sim, bot) = l_sim();
        sim.unit_mut(bot).unwrap().soldier.grounded = true;
        sim.nav_floors = vec![0.0; sim.nav.as_ref().unwrap().hubs.len()];
        let soldier = sim.unit(bot).unwrap();
        let exit = Vec3::new(0.0, 0.0, 10.0);
        let end = sim
            .preview_recovery_walk(soldier, Vec3::Z, false, Some(exit), None)
            .unwrap();
        assert!(end.distance(exit) <= crate::ai::RETURN_ARRIVE);
        assert!(sim
            .preview_recovery_walk(soldier, Vec3::NEG_Z, false, Some(exit), None)
            .is_none());
    }

    #[test]
    fn a_failed_floor_walk_can_try_the_marked_platform_exit() {
        let (mut sim, bot) = l_sim();
        sim.unit_mut(bot).unwrap().soldier.grounded = true;
        sim.nav_floors = vec![0.0; sim.nav.as_ref().unwrap().hubs.len()];
        let mut state = AiState {
            stall: Some(crate::ai::StallState {
                walked: Some(Vec3::ZERO),
                ..Default::default()
            }),
            lost: Some(LostState {
                confirmed: true,
                pinned: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!rejoin(&sim, bot, &mut state));
        sim.nav
            .as_mut()
            .unwrap()
            .floor_escapes
            .push(nav::FloorEscape {
                bounds: Aabb {
                    min: Vec3::splat(-1.0),
                    max: Vec3::splat(1.0),
                },
                hub: 1,
                headings: nav::EscapeHeadings::Compass(&[270.0, 285.0]),
            });
        assert!(rejoin(&sim, bot, &mut state));
        let walk = state.lost.unwrap();
        assert!(walk.escape_heading.is_some());
        assert!(walk.replan_after_escape);
        assert!(
            !rejoin(&sim, bot, &mut state),
            "only one verified walk per pocket"
        );
    }
}
