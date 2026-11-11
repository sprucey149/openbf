//! State updates owned by the ground brain's behaviours.

use super::situation::{Cands, Frame, Post};
use crate::ai::state::{AiState, NavTick, Walk};
use crate::sim::Sim;

pub(in crate::ai) mod board;
pub(in crate::ai) mod fight;
pub(in crate::ai) mod melee;
pub(in crate::ai) mod order;
mod overwatch;
pub(in crate::ai) mod pickup;
pub(in crate::ai) mod recover;
mod serve;
pub(in crate::ai) mod travel;

pub(in crate::ai) struct WalkContext<'a, 'world> {
    pub walk: Walk,
    pub cands: &'a Cands<'world>,
    pub at_sniper_spot: bool,
    pub post: &'a Post<'world>,
}

/// Dispatch a walk outcome to its first applicable owner. A fight can
/// finish walking while the request still has a ring point.
pub(in crate::ai) fn on_walk(
    sim: &Sim,
    frame: &Frame<'_>,
    st: &mut AiState,
    context: &WalkContext<'_, '_>,
    nt: &mut NavTick,
) {
    let WalkContext {
        walk,
        cands,
        at_sniper_spot,
        post,
    } = *context;
    let tk = &frame.tick;
    if cands.ring.is_none() || !fight::on_walk(sim, tk, st, frame.pos, walk, nt) {
        match walk {
            Walk::Arrived if at_sniper_spot && post.holding.is_none() => {
                crate::ai::snipe::arrive(sim, tk.me, st);
            }
            Walk::Failed if cands.fixing.is_some() => serve::walk_failed(sim, st, nt),
            Walk::Failed if cands.spot.is_some() && post.holding.is_none() => {
                overwatch::walk_failed(sim, st, nt);
            }
            Walk::Failed if cands.item.is_some() => pickup::walk_failed(st),
            Walk::Failed => recover::walk_failed(sim, st, nt),
            _ => {}
        }
    }
    // A bot that has arrived is not recovering any more.
    if walk == Walk::Arrived {
        st.mv.recover = 0;
    }
}
