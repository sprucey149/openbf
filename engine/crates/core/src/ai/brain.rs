//! Dispatch the ground brain or the frozen chain for space and proxies.

use super::fix::ServePlan;
use super::gunner_foot::{self, GunMode};
use super::mover::{self, MoverMode};
use super::{arsenal, man, ride, route, snipe, tactic, AiState};
use crate::sim::{PadState, RoundRules, Sim};
use crate::unit::{Unit, UnitHandle};
use behaviours::fight as position;
use glam::Vec3;
pub(crate) use position::fight_reach;

pub(super) mod behaviours;
pub(super) mod doing;
mod ground;
pub(super) mod intent;
mod legacy;
pub(super) mod selector;
#[cfg(feature = "brain-shadow")]
pub(crate) mod shadow;
pub(super) mod situation;

pub use legacy::think as legacy_think;

#[derive(Clone, Copy)]
pub(crate) struct UnitRules {
    pub(crate) round: RoundRules,
    pub(crate) proxy: Option<super::proxy::Profile>,
    seated: bool,
    droideka: bool,
}

impl UnitRules {
    pub(crate) fn for_unit(sim: &Sim, unit: &Unit) -> Self {
        Self {
            round: sim.round_rules(),
            proxy: unit.proxy_human,
            seated: unit.mount.is_some(),
            droideka: unit.droideka_class().is_some(),
        }
    }

    pub(crate) fn hit_reaction(self) -> bool {
        !self.seated && self.proxy.is_none() && self.round.ground()
    }

    pub(crate) fn faces_walk(self) -> bool {
        self.proxy.is_none() && !self.droideka && self.round.ground()
    }
}

pub(super) fn finish_pad(
    sim: &Sim,
    me: UnitHandle,
    st: &AiState,
    serve: Option<ServePlan>,
    pad: &mut PadState,
) {
    if let Some(plan) = serve.filter(|_| st.fix.cur.is_some()) {
        mover::finish(pad, MoverMode::Serve);
        gunner_foot::finish(pad, GunMode::Serve(plan));
    }
    if let Some(plan) = sim
        .unit(me)
        .and_then(|u| super::droideka::drive_plan(u, st))
    {
        mover::finish(pad, MoverMode::Droideka { stop: plan.stop });
        gunner_foot::finish(
            pad,
            GunMode::Droideka {
                raise_shield: plan.raise_shield,
            },
        );
    }
}

fn uses_legacy(sim: &Sim, me: UnitHandle) -> bool {
    sim.space_round() || sim.unit(me).is_some_and(|u| u.proxy_human.is_some())
}

pub fn think(sim: &Sim, me: UnitHandle, state: &mut super::AiState) -> (PadState, super::NavTick) {
    if uses_legacy(sim, me) {
        legacy::think(sim, me, state)
    } else {
        ground::think(sim, me, state)
    }
}

/// What one tick of a bot on foot reads once and every layer shares: the
/// unit, its AI eye, its weapon class, the difficulty tier and the unit's own
/// skill vector under the tier's overrides.
#[derive(Clone, Copy)]
pub(super) struct Tick<'a> {
    pub me: UnitHandle,
    pub u: &'a Unit,
    pub eye: Vec3,
    pub wc: &'a crate::weapon::WeaponClass,
    pub tier: crate::skill::Tier,
    pub skill: crate::skill::Skill,
    pub rules: UnitRules,
}
