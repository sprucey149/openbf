//! [ours] The proxy's HAND: how a human holding a pad aims and fires, in
//! place of the bot gunner's error model, bursts and AI ranges. Proxy units
//! only (`Profile::hand`); a bot never comes here.
//!
//! - Aim: the lead point (a little above the body's centre, where a player
//!   holds the reticle), off by an angle that starts wide when the target is
//!   first seen and settles toward a small floor while he stays on it - the
//!   stick's first swing and the assist's pull in one curve. The direction
//!   is redrawn each think, square to the line of sight.
//! - Fire: the trigger stays down while the reticle is on the target (the
//!   tolerance is the body's width at the range, never wider than a bot's),
//!   out to the round's own reach, until the gun is nearly hot; a
//!   single-shot gun is tapped as fast as it allows.

use crate::ai::draw::unit_dir;
use crate::ai::THINK_PERIOD;
use crate::collision::lead_point;
use crate::fmath;
use crate::fmath::FloatExt;
use crate::unit::{Unit, UnitHandle};
use glam::Vec3;

/// Salt of the hand's error direction (unique among `ai::draw`'s).
pub const SALT_HAND: u32 = 0x4A_0D11;

/// The width (m) a fire decision counts as "on him" at range.
pub const ON_BODY: f32 = 0.6;

/// The heat a held trigger lets go at (a player learns his gun's meter).
pub const HEAT_LET_GO: f32 = 0.9;

/// The hand's numbers. PROVISIONAL until the calibration fits them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hand {
    /// The steady aim error once settled (rad).
    pub floor: f32,
    /// The aim error at first sight (rad).
    pub acquire: f32,
    /// How fast the error settles from `acquire` to `floor` (s, the time
    /// constant).
    pub settle: f32,
    /// How far above the body's centre the reticle is held (m).
    pub up: f32,
}

impl Hand {
    pub const DEFAULT: Hand = Hand {
        floor: 0.012,
        acquire: 0.06,
        settle: 0.5,
        up: 0.25,
    };

    /// The aim error (rad) `secs` after first sight.
    pub fn error_at(&self, secs: f32) -> f32 {
        let k = fmath::exp(-secs.fmax(0.0) / self.settle.fmax(1e-3));
        self.floor + (self.acquire - self.floor).fmax(0.0) * k
    }

    /// Feed into the sim hash.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [self.floor, self.acquire, self.settle, self.up] {
            eat(&v.hash_bits().to_le_bytes());
        }
    }
}

/// The aim point at seen enemy `e` from `eye` for a round of `speed`,
/// `first_seen` the tick the target was first known.
pub fn aim(
    hand: &Hand,
    me: UnitHandle,
    tick: u64,
    first_seen: u64,
    eye: Vec3,
    e: &Unit,
    speed: f32,
) -> Vec3 {
    let c = e.capsule().center() + Vec3::Y * hand.up;
    let lead = lead_point(c - eye, e.soldier.vel, speed).map_or(c, |rel| eye + rel);
    let to = lead - eye;
    let dist = to.length();
    if dist < 1e-3 {
        return lead;
    }
    let los = to / dist;
    let d = unit_dir(me.index, tick / THINK_PERIOD, me.gen, SALT_HAND);
    let side = (d - los * d.dot(los)).normalize_or_zero();
    let secs = tick.saturating_sub(first_seen) as f32 / crate::TICK_HZ as f32;
    lead + side * hand.error_at(secs) * dist
}

/// The angle (rad) the aim may be off and still fire, at `range` m: the
/// body's width, capped at the bot's tolerance.
pub fn tolerance(range: f32) -> f32 {
    (ON_BODY / range.fmax(1.0)).fmin(crate::ai::AIM_TOLERANCE)
}

/// Is a held trigger still down at this `heat`?
pub fn holds(heat: f32) -> bool {
    heat < HEAT_LET_GO
}

/// A single-shot gun: a fresh press every other tick while on target
/// (`gate` is the last tick's press).
pub fn tap(gate: &mut bool, on_target: bool) -> bool {
    *gate = on_target && !*gate;
    *gate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_settles_from_acquire_to_floor() {
        let h = Hand::DEFAULT;
        assert!((h.error_at(0.0) - h.acquire).abs() < 1e-6);
        assert!(h.error_at(0.5) < h.acquire && h.error_at(0.5) > h.floor);
        assert!((h.error_at(10.0) - h.floor).abs() < 1e-4);
    }

    #[test]
    fn the_tolerance_is_the_body_width_capped() {
        assert_eq!(tolerance(5.0), crate::ai::AIM_TOLERANCE);
        assert!((tolerance(60.0) - 0.01).abs() < 1e-6);
    }

    #[test]
    fn a_tap_alternates_and_stops_off_target() {
        let mut g = false;
        let presses: Vec<bool> = (0..4).map(|_| tap(&mut g, true)).collect();
        assert_eq!(presses, [true, false, true, false]);
        assert!(!tap(&mut g, false));
        assert!(tap(&mut g, true));
    }
}
