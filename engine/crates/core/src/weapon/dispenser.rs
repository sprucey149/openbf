//! Weapons that place an item instead of firing a shot: health and ammo
//! pack dispensers (issue #46) and, through `MineClass`, mines and packs.

use super::{pair, pf, PropSource, Thrown, WeaponClass, WeaponState};
use crate::fmath::FloatExt;
use crate::sim::ItemClass;

#[derive(Clone, Debug, PartialEq)]
pub struct Dispenser<I = ItemClass> {
    pub item: I,
    pub velocity: f32,
    pub strength: [f32; 2],
    pub max_items: u32,
}

impl Dispenser {
    /// BF2 left-hand release point (height, forward) m: `bone_l_hand` sampled at
    /// 0.5 s of the engineer's throw (`analysis/re-grenade-throw-anim.md`).
    pub const BF2_HAND: (f32, f32) = (1.148, 0.721);
}

impl<I> Dispenser<I> {
    pub fn new(w: &mut WeaponClass, src: &dyn PropSource, bf2: bool, item: I) -> Self {
        Thrown::apply(w, src, bf2);
        w.shot_delay = pair(src, "ShotDelay")
            .unwrap_or([0.125; 2])
            .map(|v| v.fmax(0.0));
        let thrown = w.thrown.as_mut().unwrap();
        thrown.shot_elevate = 0.0;
        thrown.hand = bf2.then_some(Dispenser::BF2_HAND);
        w.flash = None;
        Self {
            item,
            velocity: pf(src, "Velocity").unwrap_or(10.0),
            strength: [
                pf(src, "MinStrength").unwrap_or(0.3),
                pf(src, "MaxStrength").unwrap_or(1.0),
            ],
            max_items: pf(src, "MaxItems").unwrap_or(0.0).fmax(0.0) as u32,
        }
    }

    /// FC-BOUNDS correction: item throws stay between the authored strengths.
    /// The shared weapon cycle can retain ShotDelay as its charge limit on a
    /// held re-trigger. PS2 Fire interpolates raw strength; we bound it here
    /// for packs and mines (analysis/re-fc-boundaries.md §1).
    pub fn speed(&self, charge: f32) -> f32 {
        let charge = charge.clamp(0.0, 1.0);
        (self.strength[0] + (self.strength[1] - self.strength[0]) * charge) * self.velocity
    }
}

impl WeaponState {
    /// Give back a use whose item could not be placed.
    pub(crate) fn refund_dispense(&mut self, c: &WeaponClass) {
        self.clip = self
            .clip
            .saturating_add(c.ammo_per_shot)
            .min(c.rounds_per_clip);
        self.reload_timer = 0.0;
        self.shots_fired = self.shots_fired.saturating_sub(1);
    }
}
