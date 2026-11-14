//! The BF2 Bothan cloak: class properties, the concealment curve, byte
//! visibility and deterministic state. Follows `analysis/re-cloaking.md` (card CLOAK).

use crate::fmath::FloatExt;
use crate::soldier::Soldier;
use crate::weapon::{pf, PropSource};

#[derive(Clone, Debug, PartialEq)]
pub struct CloakClass {
    pub energy_cost: f32,
    pub energy_drain: f32,
    pub delay: f32,
    pub linger: f32,
    pub moving: f32,
    pub still: f32,
    pub fast_speed: f32,
    pub slow_speed: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub flicker: f32,
    pub flicker_interval: [f32; 2],
}

impl CloakClass {
    pub fn from_props(src: &dyn PropSource) -> Self {
        let raw = |key, default| {
            src.prop_hash(key)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .unwrap_or(default)
        };
        Self {
            energy_cost: raw(0xb493_2ba7, 0.0),
            energy_drain: pf(src, "EnergyDrain").unwrap_or(0.0),
            delay: pf(src, "InitialSalvoDelay").unwrap_or(0.6),
            linger: raw(0xa591_fd27, 0.0),
            moving: raw(0xfb2e_6bba, 0.4),
            still: raw(0xe543_63d0, 0.8),
            fast_speed: raw(0xe071_6444, 8.0),
            slow_speed: raw(0xee85_0206, 2.0),
            fade_in: raw(0xb624_3814, 0.4),
            fade_out: raw(0x5d77_dec4, 0.8),
            flicker: raw(0xe91a_b0b8, 0.0),
            flicker_interval: [raw(0xc229_8dea, 10.0), raw(0xcc3d_2560, 10.0)],
        }
    }

    pub fn affordable(&self, soldier: &Soldier) -> bool {
        soldier.energy >= self.energy_cost && !soldier.energy_depleted
    }

    pub fn concealment(&self, speed_squared: f32) -> f32 {
        let (lo, hi) = (
            self.slow_speed * self.slow_speed,
            self.fast_speed * self.fast_speed,
        );
        if speed_squared > hi {
            self.moving
        } else if speed_squared > lo && hi > lo {
            let weight = (hi - speed_squared) / (hi - lo);
            let blend = weight * weight;
            self.moving * (1.0 - blend) + self.still * blend
        } else {
            self.still
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CloakState {
    pub active: bool,
    pub strength: f32,
    pub warmup: Option<f32>,
    pub linger: f32,
    pub flicker_left: f32,
    pub trigger: bool,
    pub health: f32,
}

impl CloakState {
    pub fn concealment(&self) -> f32 {
        f32::from((self.strength.clamp(0.0, 1.0) * 255.0) as u8) / 255.0
    }

    pub fn visibility(&self) -> f32 {
        1.0 - self.concealment()
    }

    pub fn visible(&self) -> bool {
        self.concealment() == 0.0
    }

    pub(crate) fn release(&mut self, linger: f32) {
        self.active = false;
        self.linger = linger;
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if *self == Self::default() {
            return;
        }
        eat(&[0xc1, self.active as u8, self.trigger as u8]);
        for value in [
            self.strength,
            self.warmup.unwrap_or(-1.0),
            self.linger,
            self.flicker_left,
            self.health,
        ] {
            eat(&value.hash_bits().to_le_bytes());
        }
    }
}
