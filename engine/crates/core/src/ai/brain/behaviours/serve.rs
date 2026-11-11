//! Release an unreachable turret repair claim before recovering the walk.

use super::recover;
use crate::ai::{fix, AiState, NavTick};
use crate::sim::Sim;

pub(super) fn walk_failed(sim: &Sim, st: &mut AiState, nt: &mut NavTick) {
    fix::walk_failed(sim, st);
    recover::walk_failed(sim, st, nt);
}
