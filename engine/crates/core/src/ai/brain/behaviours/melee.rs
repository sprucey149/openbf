//! Apply a saber fight to the ground brain's walk after the gunner.

use super::super::situation::Frame;
use crate::ai::brain::Tick;
use crate::ai::melee::MeleeBrain;
use crate::ai::mover::MoveRequest;
use crate::ai::{route, tactic, AiState};
use crate::sim::Sim;
use glam::Vec3;

pub(in crate::ai) fn goal(
    sim: &Sim,
    frame: &Frame<'_>,
    st: &mut AiState,
    free: bool,
) -> Option<Vec3> {
    let Tick { me, u, wc, .. } = frame.tick;
    let melee_goal = if free
        && wc.saber.is_some()
        && !st.arm.spare
        && st.arm.pick.is_none_or(|slot| slot == u.weapon_slot)
    {
        frame.enemy.filter(|e| e.mount.is_none()).map(|target| {
            let state = st
                .melee
                .get_or_insert_with(|| MeleeBrain::begin(me, sim.tick));
            crate::ai::melee::goal(sim, me, u, target, state)
        })
    } else {
        None
    };
    if melee_goal.is_some() {
        tactic::release(st, sim.tick);
        st.hold = None;
        st.fight.mode = 0;
        route::clear(st);
        st.want_stance = 0;
    } else if wc.saber.is_none() {
        st.melee = None;
    }
    melee_goal
}

pub(in crate::ai) fn steer(mv: &mut MoveRequest<'_>, melee_goal: Option<Vec3>) {
    if let Some(goal) = melee_goal {
        mv.goal = Some(goal);
        mv.arrive = None;
        mv.post = None;
        mv.in_range = false;
        mv.hold = None;
        mv.acts.throw_stand = false;
    }
}
