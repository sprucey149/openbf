//! Returning to the playable level and giving up a repeatedly failed walk.

use crate::ai::state::{AiState, LostState, NavTick, SteerPick};
use crate::ai::why::Why;
#[cfg(doc)]
use crate::ai::PIN_FAILS;
use crate::ai::{
    ride, GIVE_UP_AFTER, HULL_PUSH_MIN, LOST_JUMP_AGAIN, LOST_JUMP_CLEAR, LOST_RECYCLE, PROBE_DIST,
    RETURN_ARRIVE, RETURN_ARRIVE_DY, RETURN_PLANS, SHUT_PROBE_UP, SHUT_RECYCLE, STUCK_PROGRESS,
    STUCK_WINDOW,
};
use crate::nav;
use crate::sim::Sim;
use crate::unit::Unit;
use glam::Vec3;

pub(in crate::ai) fn think(sim: &Sim, u: &Unit, st: &mut AiState) {
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
    if st.lost.is_some_and(|l| l.pinned && l.path_at >= l.path.len) {
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
