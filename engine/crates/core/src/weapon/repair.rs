//! The repair tool's authored repair amounts, one per health type plus building
//! rebuild and build. Follows `analysis/re-build-zones.md` (card TBLD).

use super::{pf, FireState, PropSource, WeaponClass, WeaponState};
use crate::fmath::FloatExt;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Repair {
    pub health: [f32; 8],
}

impl Repair {
    pub fn from_props(src: &dyn PropSource) -> Self {
        let mut out = Self::default();
        for (i, key) in [
            "PersonHealth",
            "AnimalHealth",
            "DroidHealth",
            "VehicleHealth",
            "BuildingHealth",
            "BuildingRebuild",
            "BuildingBuild",
            "MineHealth",
        ]
        .iter()
        .enumerate()
        {
            out.health[i] = pf(src, key).unwrap_or(0.0);
        }
        out
    }
}

impl WeaponState {
    pub(crate) fn repair_tick(
        &mut self,
        c: &WeaponClass,
        held: bool,
        blocked: bool,
        dt: f32,
    ) -> bool {
        self.fire_cmd = if held {
            self.fire_cmd.max(0).saturating_add(1)
        } else if self.fire_cmd > 0 {
            -1
        } else {
            0
        };
        self.refire_accum = (self.refire_accum - dt).fmax(0.0);
        if !held || blocked || !self.active {
            self.state = FireState::Idle;
            return false;
        }
        self.state = FireState::Firing;
        if self.refire_accum > super::FIRE_EPS {
            return false;
        }
        self.refire_accum += c.shot_delay[0];
        self.shots_fired += 1;
        true
    }
}
