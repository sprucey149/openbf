//! The ground bot's behaviour after the walk has been handled.

use super::behaviours::{order, WalkContext};
use super::selector::StepKind;
use super::situation::Frame;
use crate::ai::state::{AiState, Behaviour, Walk};
use crate::ai::why::Why;
use crate::sim::Sim;

pub(in crate::ai) enum Active {
    None,
    Travel(Why),
    Fight(Why),
    Evade,
    Capture,
    Guard(GuardHow),
    Board,
    Follow,
    Recover(RecoverHow),
    Overwatch,
    Pickup,
}

pub(in crate::ai) enum GuardHow {
    Role,
    Post,
}

pub(in crate::ai) enum RecoverHow {
    OffLevel,
    Ladder,
}

impl Active {
    pub(in crate::ai) fn tag(&self) -> Behaviour {
        match self {
            Self::None => Behaviour::None,
            Self::Travel(_) => Behaviour::Travel,
            Self::Fight(_) => Behaviour::Fight,
            Self::Evade => Behaviour::Evade,
            Self::Capture => Behaviour::Capture,
            Self::Guard(_) => Behaviour::Guard,
            Self::Board => Behaviour::Board,
            Self::Follow => Behaviour::Follow,
            Self::Recover(_) => Behaviour::Recover,
            Self::Overwatch => Behaviour::Overwatch,
            Self::Pickup => Behaviour::Pickup,
        }
    }

    pub(in crate::ai) fn why(&self) -> Why {
        match self {
            Self::None => Why::Idle,
            Self::Travel(why) | Self::Fight(why) => *why,
            Self::Evade => Why::Grenade,
            Self::Capture | Self::Guard(GuardHow::Post) => Why::Arrived,
            Self::Guard(GuardHow::Role) => Why::GuardSpot,
            Self::Board => Why::Claim,
            Self::Follow => Why::Order,
            Self::Recover(RecoverHow::OffLevel) => Why::OffLevel,
            Self::Recover(RecoverHow::Ladder) => Why::None,
            Self::Overwatch => Why::SnipeSpot,
            Self::Pickup => Why::Pickup,
        }
    }
}

pub(in crate::ai) fn settle(
    sim: &Sim,
    st: &AiState,
    frame: &Frame<'_>,
    context: &WalkContext<'_, '_>,
    step: StepKind,
) -> Active {
    let WalkContext {
        walk, cands, post, ..
    } = *context;
    let hit = frame
        .tick
        .u
        .last_hit
        .is_some_and(|(_, t)| sim.tick.saturating_sub(t) < u64::from(crate::TICK_HZ));
    let fought = match () {
        _ if hit => Why::Hit,
        _ if frame.enemy.is_some() => Why::Seen,
        _ => Why::Memory,
    };
    match walk {
        _ if cands.ret.is_some() => Active::Recover(RecoverHow::OffLevel),
        _ if cands.flee.is_some() => Active::Evade,
        _ if cands.item.is_some() => Active::Pickup,
        _ if crate::ai::snipe::holds(st, post.holding) => Active::Overwatch,
        _ if post
            .holding
            .is_some_and(|h| order::guard_post(sim, st) == Some(h.spot)) =>
        {
            Active::Guard(GuardHow::Role)
        }
        _ if post.holding.is_some() || cands.ring.is_some() => Active::Fight(fought),
        _ if step == StepKind::Board || cands.boarding.is_some() => Active::Board,
        _ if st.cmd.is_some() => Active::Follow,
        Walk::Arrived if cands.post.is_some() && post.defending => Active::Guard(GuardHow::Post),
        Walk::Arrived if cands.post.is_some() => Active::Capture,
        _ if post.hold || (post.fight.is_some() && post.free) => Active::Fight(fought),
        Walk::Idle => Active::None,
        _ if st.mv.recover > 0 => Active::Recover(RecoverHow::Ladder),
        _ => Active::Travel(match st.behaviour {
            Behaviour::Fight => Why::TargetLost,
            Behaviour::Evade => Why::Safe,
            Behaviour::Recover => Why::Recovered,
            Behaviour::Board => Why::ClaimEnded,
            _ => Why::SetOut,
        }),
    }
}
