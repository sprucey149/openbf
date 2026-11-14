use crate::fmath::FloatExt;
use glam::Vec3;

use super::Sphere;
use crate::weapon::{floats, pf, PropSource};

#[derive(Clone, Debug, PartialEq)]
pub struct ShieldClass {
    pub max: f32,
    pub rate_on: f32,
    pub rate_off: f32,
    pub sphere: Sphere,
    pub drains_full: bool,
    pub sound: Option<String>,
    pub off_sound: Option<String>,
}

impl Default for ShieldClass {
    fn default() -> Self {
        Self {
            max: 3000.0,
            rate_on: -100.0,
            rate_off: 300.0,
            sphere: Sphere {
                center: Vec3::new(0.0, 1.0, 0.125),
                radius: 1.25,
            },
            drains_full: false,
            sound: None,
            off_sound: None,
        }
    }
}

impl ShieldClass {
    pub fn from_props(src: &dyn PropSource) -> Self {
        let d = Self::default();
        let f = |key: &str, v: f32| pf(src, key).unwrap_or(v);
        let name = |key: &str| {
            let v = src.prop(key)?.trim().trim_matches('"');
            (!v.is_empty()).then(|| v.to_owned())
        };
        Self {
            max: f("MaxShield", d.max),
            rate_on: f("AddShield", d.rate_on),
            rate_off: f("AddShieldOff", d.rate_off),
            sphere: Sphere {
                center: src
                    .prop("ShieldOffset")
                    .and_then(floats::<3>)
                    .map_or(d.sphere.center, Vec3::from_array),
                radius: f("ShieldRadius", d.sphere.radius),
            },
            drains_full: d.drains_full,
            sound: name("ShieldSound"),
            off_sound: name("ShieldOffSound"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shield {
    pub points: f32,
    pub on: bool,
}

impl Shield {
    pub fn full(c: &ShieldClass) -> Self {
        Self {
            points: c.max,
            on: false,
        }
    }

    pub fn tick(
        &mut self,
        dt: f32,
        press: bool,
        forced_off: bool,
        recharge: bool,
        c: &ShieldClass,
    ) -> Option<bool> {
        let was = self.on;
        if self.points <= 0.0 || forced_off {
            self.on = false;
        } else if press {
            self.on = !self.on;
        }
        let rate = match (self.on, recharge) {
            (true, _) => c.rate_on,
            (false, true) => c.rate_off,
            (false, false) => 0.0,
        };
        if c.drains_full || self.points < c.max {
            // A ball payment may leave debt on this same bar. Shield drain
            // stops at zero; recharge repays existing debt before filling it.
            let floor = self.points.fmin(0.0);
            self.points = (self.points + dt * rate).clamp(floor, c.max);
        }
        (was != self.on).then_some(self.on)
    }

    pub fn absorb(&mut self, damage: f32, shield_scale: f32) -> f32 {
        if !self.on || self.points <= 0.0 {
            return damage;
        }
        let taken = damage * shield_scale;
        if taken <= self.points {
            self.points -= taken;
            0.0
        } else {
            let left = damage - self.points / shield_scale;
            self.points = 0.0;
            left
        }
    }
}
