//! Item seeking and the end of a failed pickup detour.

use crate::ai::{items, AiState};
use crate::sim::Sim;
use crate::unit::Unit;
use glam::Vec3;

pub(in crate::ai) fn goal(
    sim: &Sim,
    u: &Unit,
    st: &mut AiState,
    flee: Option<Vec3>,
) -> Option<Vec3> {
    // Update the seeker before deciding whether fleeing masks the pickup.
    items::goal(sim, u, st).filter(|_| flee.is_none())
}

pub(super) fn walk_failed(st: &mut AiState) {
    st.items.detour = None;
}
