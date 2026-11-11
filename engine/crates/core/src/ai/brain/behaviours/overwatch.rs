//! Recovery when a sniper cannot reach its spot.

use super::recover;
use crate::ai::{snipe, AiState, NavTick};
use crate::sim::Sim;

pub(super) fn walk_failed(sim: &Sim, st: &mut AiState, nt: &mut NavTick) {
    snipe::walk_failed(sim, st);
    recover::walk_failed(sim, st, nt);
}
