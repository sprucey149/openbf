//! Inputs read before reactions and after the gunner.

use super::Tick;
use crate::ai::gunner_foot::GunReport;
use crate::ai::state::{AiState, HoldState};
use crate::autoturret::Turret;
use crate::perception::{self, Contact};
use crate::remote::Droid;
use crate::sim::{Sim, Station};
use crate::unit::Unit;
use glam::Vec3;

#[derive(Default)]
pub(in crate::ai) struct Sense {
    pub held: u8,
    pub heard: u8,
    pub hunt: Option<(f32, Vec3)>,
    pub seen_any: bool,
    pub fixing_dest: Option<Vec3>,
}

pub(in crate::ai) struct Frame<'a> {
    pub tick: Tick<'a>,
    pub pos: Vec3,
    pub mend: Option<&'a Station>,
    pub held: Option<Contact>,
    pub seen: Option<Contact>,
    pub enemy: Option<&'a Unit>,
    pub memory: Option<Vec3>,
    pub droid: Option<(&'a Droid, bool)>,
    pub turret: Option<(&'a Turret, bool)>,
}

impl<'a> Frame<'a> {
    pub(in crate::ai) fn read(
        sim: &'a Sim,
        st: &AiState,
        tick: Tick<'a>,
        mend: Option<&'a Station>,
    ) -> Self {
        let held = st.target.and_then(|h| perception::contact_of(st, h));
        let seen = held.filter(|c| c.seen_now);
        let enemy = seen.and_then(|c| c.handle.live_unit(sim));
        let memory = held.filter(|c| !c.seen_now).map(|c| c.last_pos);
        let fireable = |c: &Contact| perception::fireable(c, sim.tick, tick.tier.reaction_ticks);
        let droid = seen.and_then(|c| Some((sim.live_droid(c.handle.droid()?)?, fireable(&c))));
        let turret = seen.and_then(|c| Some((sim.live_turret(c.handle.turret()?)?, fireable(&c))));
        Self {
            tick,
            pos: tick.u.soldier.pos,
            mend,
            held,
            seen,
            enemy,
            memory,
            droid,
            turret,
        }
    }

    pub(in crate::ai) fn seen_at(&self) -> Option<Vec3> {
        self.enemy
            .map(|u| u.capsule().center())
            .or(self.droid.map(|(d, _)| d.pos))
            .or(self.turret.map(|(t, _)| t.centre()))
    }
}

pub(in crate::ai) struct Post<'a> {
    pub gun: GunReport,
    pub holding: Option<HoldState>,
    pub hold: bool,
    pub fight: Option<(&'a Unit, f32)>,
    pub defending: bool,
    pub free: bool,
}

/// The candidate points and their route context, read after the reactions.
#[derive(Default)]
pub(in crate::ai) struct Cands<'a> {
    pub last_leg: bool,
    pub route_leg: Option<crate::ai::Route>,
    pub post: Option<&'a crate::cp::CommandPost>,
    pub corner: Option<glam::Vec2>,
    pub push: Option<Vec3>,
    pub base_goal: Option<Vec3>,
    pub boarding: Option<Vec3>,
    pub item: Option<Vec3>,
    pub ret: Option<Vec3>,
    pub flee: Option<Vec3>,
    pub ring: Option<Vec3>,
    pub spot: Option<Vec3>,
    pub fixing: Option<Vec3>,
    pub mend: Option<&'a Station>,
    pub holding: Option<HoldState>,
}
