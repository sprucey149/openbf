//! Fixed priorities for the ground brain's destination and walk owner.

use super::intent::{Goal, MoveKind};
use super::situation::Cands;
use crate::ai::{route, AiState, NavTick, CORNER_ARRIVE, HOLD_ARRIVE, RING_ARRIVE};
use crate::sim::Sim;
use crate::squad::Command;
use crate::unit::UnitHandle;
use glam::Vec3;

/// Each row takes priority over the rows below it. Board claims can still
/// retarget afterwards, in the same order as they were delivered.
pub(in crate::ai) fn destination(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    nt: &mut NavTick,
    order: Option<Command>,
    direct: [Option<Vec3>; 4],
    hunt: Option<Vec3>,
) {
    if let Some(command) = order {
        crate::squad::steer(sim, me, st, &command, nt);
    } else if let Some(point) = direct.into_iter().flatten().next() {
        route::retarget_board(sim, me, st, point, nt);
    } else {
        route::retarget(sim, me, st, hunt, nt);
    }
}

/// Owner of the walk request. The score orders eligible owners.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u16)]
pub enum StepKind {
    #[default]
    None = 0,
    Leg = 100,
    Push = 200,
    Spot = 300,
    Ring = 390,
    RingTake = 400,
    Fix = 450,
    Mend = 500,
    Board = 600,
    Hold = 700,
    Melee = 750,
    Pickup = 800,
    Flee = 900,
    Return = 1000,
}

impl StepKind {
    pub(in crate::ai) fn move_kind(self) -> MoveKind {
        match self {
            Self::Return => MoveKind::Return,
            Self::Flee => MoveKind::Flee,
            Self::Hold => MoveKind::Hold,
            Self::Board => MoveKind::Board,
            Self::Pickup => MoveKind::Pickup,
            Self::Mend => MoveKind::Mend,
            Self::None
            | Self::Leg
            | Self::Push
            | Self::Spot
            | Self::Ring
            | Self::RingTake
            | Self::Fix
            | Self::Melee => MoveKind::Leg,
        }
    }

    pub(in crate::ai) fn after_aim(self, melee: Option<Vec3>) -> Self {
        if melee.is_some() {
            Self::Melee
        } else {
            self
        }
    }

    pub(in crate::ai) fn arrive(
        self,
        cands: &Cands<'_>,
        goal: Goal,
        last_waypoint: Option<Vec3>,
        no_route: bool,
        commanded: bool,
    ) -> Option<(Vec3, f32)> {
        match self {
            Self::Pickup => cands.item.map(|p| (p, 0.2)),
            Self::Hold => cands.holding.map(|h| (h.goal, HOLD_ARRIVE)),
            Self::Fix => goal.at_fix.map(|p| (p, HOLD_ARRIVE)),
            Self::RingTake => goal.at_ring.map(|p| (p, HOLD_ARRIVE)),
            Self::Ring => goal.at_ring.map(|p| (p, RING_ARRIVE)),
            Self::Spot => goal.at_spot.map(|p| (p, RING_ARRIVE)),
            Self::Push => cands.push.map(|p| (p, CORNER_ARRIVE)),
            Self::Leg if !commanded => {
                let point = if no_route {
                    goal.approach
                } else {
                    last_waypoint
                };
                point.map(|p| (p, CORNER_ARRIVE))
            }
            _ => None,
        }
    }
}

pub(in crate::ai) fn select(cands: &Cands<'_>, goal: Goal, ring_take: bool) -> StepKind {
    // Descending score. Melee is decided after the gunner has updated the weapon pick.
    match () {
        _ if cands.ret.is_some() => StepKind::Return,
        _ if cands.flee.is_some() => StepKind::Flee,
        _ if cands.item.is_some() => StepKind::Pickup,
        _ if cands.holding.is_some() => StepKind::Hold,
        _ if cands.boarding.is_some() && goal.approach == cands.boarding => StepKind::Board,
        _ if cands.mend.is_some() => StepKind::Mend,
        _ if cands.fixing.is_some() => StepKind::Fix,
        _ if cands.ring.is_some() && ring_take => StepKind::RingTake,
        _ if cands.ring.is_some() => StepKind::Ring,
        _ if cands.spot.is_some() => StepKind::Spot,
        _ if cands.push.is_some() && goal.approach == cands.push => StepKind::Push,
        _ => StepKind::Leg,
    }
}

#[cfg(test)]
mod tests;
