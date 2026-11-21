//! [ours] The proxy's HAND: how a human holding a pad aims and fires, in
//! place of the bot gunner's error model, bursts and AI ranges. Proxy units
//! only (`Profile::hand`); a bot never comes here.
//!
//! - Aim: the lead point (a little above the body's centre, where a player
//!   holds the reticle), off by an angle that starts wide when the target is
//!   first seen and settles toward a small floor while he stays on it - the
//!   stick's first swing and the assist's pull in one curve. The direction
//!   is redrawn each think, square to the line of sight.
//! - [ours] What makes a player miss more (2026-11-20, the maintainer: his
//!   accuracy must fall with his own maneuvering, long range and
//!   hard-to-hit targets): the error grows with how fast the target crosses
//!   his view - its motion and his own, across the line of sight, over the
//!   range ([`Hand::track`], the tracking lag: a crossing target, a strafe
//!   of his own, a jump, a vehicle's speed), it is wider in the air or a
//!   roll ([`Hand::air`]) and in a vehicle's seat ([`Hand::seat`], which
//!   aims with the same hand). Range needs no term of its own: a fixed
//!   angle is a wider miss in metres at range.
//! - Fire: the trigger stays down while the reticle is on the target (the
//!   tolerance is the body's width at the range, never wider than a bot's),
//!   out to the round's own reach, until the gun is nearly hot; a
//!   single-shot gun is tapped as fast as it allows.

use crate::ai::draw::unit_dir;
use crate::ai::THINK_PERIOD;
use crate::collision::lead_point;
use crate::fmath;
use crate::fmath::FloatExt;
use crate::sim::Sim;
use crate::soldier::Stance;
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
    /// [ours] The tracking lag (s): the error gains this many seconds of
    /// the target's angular speed across the line of sight (both units'
    /// motion, over the range). OURS; measure: the target's crossing speed
    /// is not in the match log yet (a shot row schema with it), so the
    /// value is fitted through the stand-in calibration, not read.
    pub track: f32,
    /// [ours] The error's multiplier while the shooter is airborne or
    /// rolling (a jump, a fall, the jets). OURS; measure: the logs hold too
    /// few shots in the air (under 20 of 5,424) to read it.
    pub air: f32,
    /// [ours] The error's multiplier in a vehicle's gun seat. OURS: a
    /// player's 275 seat shots hit 0.14 to the stand-in's 0.24 (2026-11-20);
    /// measure: more seat shots in players' logs, split by seat weapon.
    pub seat: f32,
}

/// [ours] The shooter's own state for [`aim`]: its velocity and whether it
/// is in the air (or a roll) or in a seat.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Shooter {
    pub vel: Vec3,
    pub airborne: bool,
    pub seated: bool,
}

impl Shooter {
    /// `u` as a shooter: its own velocity on foot, its vehicle's in a seat.
    pub fn of(sim: &Sim, u: &Unit) -> Shooter {
        let seat = u.mount.and_then(|m| sim.vehicle(m.vehicle));
        Shooter {
            vel: moving(sim, u),
            airborne: seat.is_none()
                && (u.soldier.stance.airborne() || u.soldier.stance == Stance::Roll),
            seated: seat.is_some(),
        }
    }
}

/// A unit's velocity: its vehicle's when it rides one.
pub fn moving(sim: &Sim, u: &Unit) -> Vec3 {
    u.mount
        .and_then(|m| sim.vehicle(m.vehicle))
        .map_or(u.soldier.vel, |v| v.vel)
}

impl Hand {
    pub const DEFAULT: Hand = Hand {
        floor: 0.012,
        acquire: 0.06,
        settle: 0.5,
        up: 0.25,
        track: 0.0,
        air: 1.0,
        seat: 1.0,
    };

    /// The aim error (rad) `secs` after first sight.
    pub fn error_at(&self, secs: f32) -> f32 {
        let k = fmath::exp(-secs.fmax(0.0) / self.settle.fmax(1e-3));
        self.floor + (self.acquire - self.floor).fmax(0.0) * k
    }

    /// [ours] The error (rad) `secs` after first sight for `shooter`, the
    /// target crossing the line of sight at `cross` rad/s.
    pub fn error_for(&self, secs: f32, shooter: Shooter, cross: f32) -> f32 {
        let mut m = 1.0;
        if shooter.airborne {
            m *= self.air;
        }
        if shooter.seated {
            m *= self.seat;
        }
        self.error_at(secs) * m + self.track * cross.abs()
    }

    /// Feed into the sim hash. The three tracking numbers only when they
    /// are not the neutral ones, so a hand without them hashes as before.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [self.floor, self.acquire, self.settle, self.up] {
            eat(&v.hash_bits().to_le_bytes());
        }
        if self.track != 0.0 || self.air != 1.0 || self.seat != 1.0 {
            eat(&[0x54]);
            for v in [self.track, self.air, self.seat] {
                eat(&v.hash_bits().to_le_bytes());
            }
        }
    }
}

/// The aim point at seen enemy `e` from `eye` for a round of `speed`,
/// `first_seen` the tick the target was first known, `shooter` the
/// shooter's own state and `target_vel` the target's velocity (its
/// vehicle's when it rides one, [`moving`]).
#[allow(clippy::too_many_arguments)]
pub fn aim(
    hand: &Hand,
    me: UnitHandle,
    tick: u64,
    first_seen: u64,
    eye: Vec3,
    e: &Unit,
    speed: f32,
    shooter: Shooter,
    target_vel: Vec3,
) -> Vec3 {
    let c = e.capsule().center() + Vec3::Y * hand.up;
    // The lead uses the SOLDIER's velocity, as `seen_aim` does, so a rider
    // (whose soldier velocity the seat code zeroes) gets no lead while his
    // vehicle's motion still adds to the tracking miss below. Leading with
    // `target_vel` would change every proxy round against vehicles and move
    // the pins; left for the next profile refit (replace: lead with
    // `target_vel` and re-pin in that refit's commit).
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
    lead + side
        * hand.error_for(
            secs,
            shooter,
            cross_rate(los, dist, target_vel - shooter.vel),
        )
        * dist
}

/// The angular speed (rad/s) at which a target `dist` m along `los` moves
/// across the line of sight at relative velocity `rel`.
pub fn cross_rate(los: Vec3, dist: f32, rel: Vec3) -> f32 {
    (rel - los * rel.dot(los)).length() / dist.fmax(1.0)
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
    fn the_error_grows_with_crossing_speed_the_air_and_the_seat() {
        let h = Hand {
            track: 0.1,
            air: 2.0,
            seat: 1.5,
            ..Hand::DEFAULT
        };
        let still = Shooter::default();
        let base = h.error_for(10.0, still, 0.0);
        assert!((base - h.error_at(10.0)).abs() < 1e-6);
        assert!((h.error_for(10.0, still, 0.5) - (base + 0.05)).abs() < 1e-6);
        let air = Shooter {
            airborne: true,
            ..still
        };
        assert!((h.error_for(10.0, air, 0.0) - 2.0 * base).abs() < 1e-6);
        let seat = Shooter {
            seated: true,
            ..still
        };
        assert!((h.error_for(10.0, seat, 0.0) - 1.5 * base).abs() < 1e-6);
        // The neutral hand adds nothing, so DEFAULT aims as before.
        let d = Hand::DEFAULT;
        assert_eq!(d.error_for(1.0, air, 3.0), d.error_at(1.0));
    }

    #[test]
    fn a_crossing_target_turns_faster_up_close() {
        let los = Vec3::Z;
        let rel = Vec3::X * 5.0;
        assert!((cross_rate(los, 10.0, rel) - 0.5).abs() < 1e-6);
        assert!((cross_rate(los, 50.0, rel) - 0.1).abs() < 1e-6);
        // Motion along the line of sight does not cross it.
        assert!(cross_rate(los, 10.0, Vec3::Z * 9.0) < 1e-6);
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
