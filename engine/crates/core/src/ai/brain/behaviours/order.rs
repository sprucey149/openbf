//! Walk goals from player orders and upkeep of the Cover role's hold.

use crate::ai::{AiState, CmdState, HoldState, HOLD_CLOSE, HOLD_COMMIT, THINK_PERIOD};
use crate::nav;
use crate::sim::Sim;
use glam::{Vec2, Vec3};

pub(super) fn goal(pos: Vec3, command: CmdState) -> Option<Vec3> {
    Some(command.goal).filter(|g| nav::xz_dist(pos, *g) > crate::squad::STAND_ARRIVE)
}

/// Card TEAM: the guard position of a bot with the Cover role, while no
/// fight holds it.
pub(in crate::ai) fn guard_post(sim: &Sim, st: &AiState) -> Option<Vec3> {
    st.role
        .post
        .filter(|_| st.fight.mode == 0)
        .and_then(|_| sim.positions.positions.get(st.role.at as usize))
        .map(|q| q.pos)
}

pub(in crate::ai) fn keep_guard(sim: &Sim, st: &mut AiState) {
    // Card TEAM: a bot with the COVER role (`team::board`) guards its
    // position, facing the approach, while no fight holds it.
    if let Some(g) = guard_post(sim, st) {
        let q = &sim.positions.positions[st.role.at as usize];
        let mut h = st
            .hold
            .filter(|h| sim.tick < h.until && h.spot == g)
            .unwrap_or(HoldState {
                spot: g,
                goal: g,
                commit_until: sim.tick + HOLD_COMMIT,
                ..HoldState::default()
            });
        h.until = sim.tick + THINK_PERIOD + HOLD_CLOSE;
        h.last_contact = sim.tick;
        h.look = st.role.look;
        let d = st.role.look - g;
        h.stance_mask = if q.shields_toward(Vec2::new(d.x, d.z)) {
            crate::commander::MASK_CROUCH
        } else {
            0
        };
        st.hold = Some(h);
    }
}
