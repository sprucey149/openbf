//! An officer ability's data, read from its emitter ordnance class
//! (`analysis/re-officer-abilities.md`).

use super::{pf, PropSource};
use crate::fmath::FloatExt;

/// `[ours]` Poison damages at two thirds of the disc's rate (the MagnaGuard's
/// 30/s for 15 s becomes 20/s): at the full rate it killed too fast in play.
pub const POISON_RATE: f32 = 2.0 / 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Ability {
    pub radius: f32,
    pub friends: bool,
    pub enemies: bool,
    pub self_on_hit: bool,
    pub offense: (f32, f32),
    pub defense: (f32, f32),
    pub regeneration: (f32, f32),
    pub poison: (f32, f32),
    pub activation_effect: Option<String>,
}

impl Ability {
    /// `None` unless the class is a radius ability (Radius > 0, no cone) with
    /// at least one timed effect.
    pub fn from_props(src: &dyn PropSource) -> Option<Self> {
        let pair = |timer, value, default| {
            (
                pf(src, timer).unwrap_or(0.0).fmax(0.0),
                pf(src, value).unwrap_or(default),
            )
        };
        let ability = Self {
            radius: pf(src, "Radius").unwrap_or(0.0).fmax(0.0),
            friends: pf(src, "AffectFriends").unwrap_or(1.0) != 0.0,
            enemies: pf(src, "AffectEnemies").unwrap_or(1.0) != 0.0,
            self_on_hit: src
                // The self-on-hit flag; its key name is not known.
                .prop_hash(0x39b3_648c)
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0)
                != 0.0,
            offense: pair("BuffOffenseTimer", "BuffOffenseMult", 1.0),
            defense: pair("BuffDefenseTimer", "BuffDefenseMult", 1.0),
            regeneration: pair("BuffHealthTimer", "BuffHealthRate", 0.0),
            poison: {
                let (timer, rate) = pair("DebuffDamageTimer", "DebuffDamageRate", 0.0);
                (timer, rate * POISON_RATE)
            },
            // The activation particle effect; its key name is not known.
            activation_effect: src.prop_hash(0x76f3_beea).map(str::to_owned),
        };
        (ability.radius > 0.0
            && pf(src, "ConeLength").unwrap_or(10.0) == 0.0
            && [
                ability.offense.0,
                ability.defense.0,
                ability.regeneration.0,
                ability.poison.0,
            ]
            .into_iter()
            .any(|t| t > 0.0))
        .then_some(ability)
    }
}
