//! The aim request and the walk plan at the point before the gunner runs.

use super::selector::StepKind;
use super::situation::{Cands, Frame};
use super::{fight_reach, Tick};
use crate::ai::gunner_foot::AimRequest;
use crate::ai::{AiState, HOLD_ARRIVE};
use crate::nav;
use crate::perception;
use crate::sim::Sim;
use glam::Vec3;

pub(in crate::ai) struct Intent<'a> {
    pub aim: AimRequest<'a>,
    pub step: StepKind,
    pub goal: Option<Vec3>,
    pub arrive: Option<(Vec3, f32)>,
}

#[derive(Clone, Copy, Default)]
pub(in crate::ai) struct Acts {
    pub roll: i8,
    pub throw_stand: bool,
    pub melee_jump: bool,
}

/// Values retained in the mover state and in black-box logs.
#[repr(u8)]
pub(in crate::ai) enum MoveKind {
    Return = 1,
    Flee = 2,
    Hold = 3,
    Board = 4,
    Leg = 5,
    Pickup = 6,
    Mend = 7,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ai) struct Goal {
    pub approach: Option<Vec3>,
    pub at_ring: Option<Vec3>,
    pub at_spot: Option<Vec3>,
    pub at_fix: Option<Vec3>,
}

impl Goal {
    pub(in crate::ai) fn immediate(self, cands: &Cands<'_>) -> Option<Vec3> {
        cands
            .ret
            .or(cands.flee)
            .or(cands.item)
            .or(cands.holding.map(|h| h.goal))
            .or(self.approach)
    }
}

pub(in crate::ai) fn compose_goal(cands: &Cands<'_>, pos: Vec3) -> Goal {
    let mut goal = cands.base_goal;
    if cands.last_leg {
        goal = cands.boarding.or(goal);
    }
    // Record each arrival against the goal at that point in the chain.
    if cands.last_leg {
        goal = cands.ring.or(goal);
    }
    let at_ring = cands.ring.filter(|p| goal == Some(*p));
    if cands.last_leg {
        goal = cands.spot.or(goal);
    }
    let at_spot = cands.spot.filter(|p| goal == Some(*p));
    if cands.last_leg {
        goal = cands.fixing.or(goal);
    }
    let at_fix = cands.fixing.filter(|p| goal == Some(*p));
    if let Some(station) = cands.mend {
        goal = (!station.reaches(pos)).then_some(station.pos);
    }
    Goal {
        approach: goal,
        at_ring,
        at_spot,
        at_fix,
    }
}

pub(in crate::ai) fn compose_aim<'a>(
    sim: &Sim,
    st: &AiState,
    frame: &Frame<'a>,
    cands: &Cands<'a>,
    goal: Option<Vec3>,
) -> Option<AimRequest<'a>> {
    let Tick {
        u, eye, wc, rules, ..
    } = frame.tick;
    let pos = frame.pos;
    let ground = rules.round.ground();
    // Playtest C3: a bot walking somewhere faces its walk unless it can
    // fight what it looks at. The PS2 gives no aim order outside an attack
    // (`analysis/re-soldier-ai.md` 1.5), ends one past the weapon's range
    // + 10 m (4.2), never attacks a vehicle its weapons cannot hurt (4.3)
    // and shares no contacts (6). So a rider no weapon of the bot's hurts,
    // a target beyond its reach, a friend's report and a remembered rider
    // are not faced on the walk (they stay its target). A bot that has
    // arrived, holds or takes a position, and a space round's, aim as before.
    let walking_on = goal.is_some()
        && !st.mv.arrived
        && cands.holding.is_none()
        && cands.ring.is_none()
        && wc.saber.is_none()
        && rules.faces_walk();
    let reach = fight_reach(sim, u);
    let within = |p: Vec3| (p - eye).length_squared() <= reach * reach;
    let faced = frame.enemy.zip(frame.seen).filter(|(e, _)| {
        !walking_on || (within(e.capsule().center()) && (e.mount.is_none() || !st.arm.spare))
    });
    let faced_memory = frame.memory.filter(|p| {
        !walking_on
            || goal == Some(*p)
            || frame.held.is_some_and(|c| {
                !c.told
                    && c.handle.live_unit(sim).is_none_or(|o| o.mount.is_none())
                    && within(c.last_pos)
            })
    });
    let walks_past = (frame.enemy.is_some() && frame.seen.is_some() && faced.is_none())
        || (frame.memory.is_some() && faced_memory.is_none());

    // Where to look: the engaged enemy's lead point, else where we are
    // walking, else the target post (the last leg, and the no-graph
    // placeholder).
    Some(if let Some((d, ready)) = frame.droid {
        AimRequest::Droid(d, ready)
    } else if let Some((t, ready)) = frame.turret {
        AimRequest::Turret(t, ready)
    } else {
        match (faced, faced_memory, cands.corner, cands.route_leg) {
            // A seen enemy: the gunner's own aim point (its lead point under the
            // D10 error), and the only target it fires on.
            (Some((e, c)), _, _, _) => AimRequest::Enemy(e, c),
            // A remembered contact is aimed at where it was last known to be —
            // a point on the target, no lead (the PS2 aims at a point, and there
            // is no velocity to lead with once the eyes are off it). A heard
            // shot, a friend's report or a lost contact behind geometry is a
            // direction to face, not a point to pitch at: the gun stays level
            // (`perception::can_aim`, playtest C4).
            (None, Some(p), _, _) if st.memory_blocked => {
                AimRequest::Travel(perception::level(eye, p + Vec3::Y * 1.4))
            }
            (None, Some(p), _, _) => AimRequest::Memory(p + Vec3::Y * 1.4),
            // Card HOLD: a holder standing on its point with nobody in sight
            // watches the way the nearest enemy is.
            (None, None, _, _)
                if cands.holding.is_some_and(|h| {
                    h.look != Vec3::ZERO && nav::xz_dist(pos, h.goal) <= HOLD_ARRIVE
                }) =>
            {
                AimRequest::Travel(cands.holding.map_or(pos, |h| h.look))
            }
            (None, None, _, _) if cands.item.is_some() => {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            (None, None, Some(_), _) => AimRequest::Travel(goal.unwrap_or(pos)),
            // Running from a grenade, or walking back to the level: where it goes.
            (None, None, None, _) if cands.flee.is_some() || cands.ret.is_some() => {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            // A boarder on its last leg looks at the vehicle it walks to, not
            // at its post (playtest C3: it walked there sideways); ground
            // rounds only. PILEIN can send an uncommanded bot to a vehicle
            // without a post or graph; its approach must not take the idle
            // fallback either.
            (None, None, None, None)
                if cands.boarding.is_some() && goal == cands.boarding && ground =>
            {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            (None, None, None, None)
                if cands.boarding.is_some()
                    && st.ride.is_some_and(|r| r.player_board.is_some()) =>
            {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            // Aim at the turret or droid while approaching or servicing it.
            (None, None, None, None) if cands.fixing.is_some() || cands.mend.is_some() => {
                let at = cands.mend.map_or_else(|| goal.unwrap_or(pos), |s| s.pos);
                AimRequest::Travel(at + Vec3::Y * 1.4)
            }
            (None, None, None, Some(r)) => AimRequest::Travel(r.waypoint),
            // (Playtest C3: a bot walking past what it does not face, with
            // no leg and no post, looks where it walks rather than stopping.)
            (None, None, None, None) if walks_past => {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            (None, None, None, None) => AimRequest::Travel(match cands.post {
                Some(cp) => cp.def.pos + Vec3::Y * 1.4,
                // Hunting on the last leg: the destination hub itself.
                None => match st.route() {
                    Some(r) => r.waypoint,
                    // D6: a commanded unit LOOKS where it walks — the idle arm
                    // below is for a unit with no route, no post AND no order.
                    None => st.cmd?.goal + Vec3::Y * 1.4,
                },
            }),
        }
    })
}
