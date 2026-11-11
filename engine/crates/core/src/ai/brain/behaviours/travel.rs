//! Advance route legs and choose their immediate walking point.

use super::super::situation::Frame;
use super::order;
use crate::ai::route::{advance_route, leg_point, sub_point};
use crate::ai::{AiState, NavTick};
use crate::cp::CommandPost;
use crate::sim::Sim;
use crate::unit::UnitHandle;
use glam::{Vec2, Vec3};

pub(in crate::ai) fn advance(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    pos: Vec3,
    nt: &mut NavTick,
) {
    let leg_hub = st.route().filter(|r| !r.is_last()).map(|r| r.next_hub);
    advance_route(sim, me, st, pos, 0.0, nt);
    // Card HOLD: a hub just reached settles a hold that has run out.
    if leg_hub.is_some_and(|h| st.route().map(|r| r.next_hub) != Some(h)) {
        hub_reached(sim, st);
    }
}

pub(in crate::ai) fn goal(
    st: &AiState,
    frame: &Frame<'_>,
    post: Option<&CommandPost>,
    push: Option<Vec3>,
) -> Option<Vec3> {
    let pos = frame.pos;
    let u = frame.tick.u;
    match (st.route(), st.cmd) {
        _ if push.is_some() => push,
        // On an arc leg, along the arc's line (`leg_point`) — but not while
        // the steering is going round something (`slide`): a wall on the line
        // is passed by leaving the line, and pulling back to it flipped the
        // slide's side every few ticks.
        (Some(r), _) => Some(if st.slide == 0 {
            let v = u.soldier.vel;
            leg_point(&r, pos, Vec2::new(v.x, v.z).length())
        } else {
            sub_point(&r, pos)
        }),
        // A COMMANDED unit with no route walks at the point its order gives
        // it — the leader, its standing point by him, or the spot it holds —
        // and stands once there. It has no post (the command branch clears
        // `target_cp`), so this arm is the whole of its walking.
        (None, Some(c)) => order::goal(pos, c),
        // Brief D7: with no route leg and no post to walk at, a unit that has
        // LOST sight of its target walks at the last place it knew of, until
        // the memory expires. Nothing else about walking changes — an engaged
        // unit still walks its route (A.1 decision 4).
        (None, None) => post.map(|cp| cp.def.pos).or(frame.memory),
    }
}

/// Card HOLD: hub `hub` has just been reached: a hold that has run out is
/// dropped, or pays one of the hubs it owes. Card TPOS: no bot takes a hold
/// at a hub any more because an enemy is about (card HOLD's 10-24 s dwell at
/// a hint or a drawn hub point) - a fight's position is the Tactician's.
/// Those holders were 11-24 % of Yavin 2's, Utapau's and Polis Massa's fight
/// time with no position, standing on a target they did not see; without
/// them B12 rose on 7 of 8 maps and kills a minute x1.06 (8 seeds).
fn hub_reached(sim: &Sim, st: &mut AiState) {
    if let Some(mut h) = st.hold {
        if sim.tick < h.until {
            return;
        }
        h.skip = h.skip.saturating_sub(1);
        st.hold = (h.skip > 0).then_some(h);
    }
}
