//! An explosion's camera shake (`analysis/re-vehicle-death.md` §1, the
//! explosion class's `Shake` / `ShakeRadiusInner` / `ShakeRadius` /
//! `ShakeLength`).
//!
//! The push: the explode (B2 `FUN_0030c8f8` B2:288298-288337, B1
//! `FUN_002701a0` B1:193890-193934), when the class's `ShakeRadius` is
//! above 0, measures `d` from the blast to the local player's controlled
//! object and, past 0.0001 m, takes `f` = the blast falloff on that object's
//! sphere (`FUN_00268fe0` B2:185158, the damage's own law, `d − w/2` over
//! `ShakeRadiusInner..ShakeRadius`). A positive `f` pushes `Shake·f·k` with
//! a fall of `Shake·f·k / (ShakeLength·f·k)` per second (`FUN_001ba7e0`
//! B2:63054-63068, at most four at once). The loop runs twice and in single
//! player both passes find the same camera, so every blast pushes two.
//!
//! The camera (B2 `FUN_001baea8` B2:63534-63582, B1 the same): each update
//! sums the live amplitudes into `S`, then lowers each by its fall times the
//! step and drops the spent ones. While `S > 0` the camera turns about each
//! of its own three axes by `r³ · S · 0.1` radians, `r` uniform in (−1, 1)
//! from the game's LCG, picked afresh every update (B2:64092-64124,
//! B1:52019-52047).
//!
//! Measured (BF2 Kashyyyk, the maintainer's save state 9, a private PCSX2,
//! 2026-11-13): the player's snail tank 28.52 m from a dying one
//! (`cis_tread_snailtank_exp`: Shake 1, ShakeLength 1, ShakeRadius 35;
//! w 5.358) held two entries of 0.2616 falling 1.0 per second, `S` 0.5232
//! on the frame they landed; the law gives `f` 0.2617 with `k` = 1.

use crate::fx::emitter::Lcg;
use core::impact::Shake;
use glam::Vec3;
use render::Camera;

/// The camera's four shake slots (`FUN_001ba7e0`: `count < 4`).
pub const MAX_SHAKES: usize = 4;

/// Radians per unit of summed shake at `r` = ±1 (B2:64114-64124).
const TURN: f32 = 0.1;

/// Passes of the explode's player loop (B2:288298 `uVar5 < 2`, B1:193893
/// `iVar5` 1..0).
const PASSES: usize = 2;

pub struct CameraShake {
    amps: Vec<f32>,
    falls: Vec<f32>,
    turn: Vec3,
    rng: Lcg,
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            amps: Vec::new(),
            falls: Vec::new(),
            turn: Vec3::ZERO,
            // OURS: the PS2 draws from its one global LCG; this one is the
            // shake's alone. trace: the seed only orders the picks.
            rng: Lcg(0x5a4e_c0de),
        }
    }
}

impl CameraShake {
    /// A blast of `shake` `d` metres from the controlled object, whose
    /// blast sphere is `w` across. `k` is the controlled class's factor
    /// (`*(class + 4)`, B2:288328); OURS: always 1. measure: the RAM above
    /// gives 1 for a snail tank driver; the field's name was not found.
    pub fn blast(&mut self, shake: &Shake, d: f32, w: f32) {
        if shake.outer <= 0.0 || d <= 0.0001 {
            return;
        }
        let f = core::weapon::blast_falloff(d, w, shake.inner, shake.outer);
        if f <= 0.0 {
            return;
        }
        let k = 1.0;
        let amp = shake.amount * f * k;
        let length = shake.length * f * k;
        for _ in 0..PASSES {
            if self.amps.len() < MAX_SHAKES {
                self.amps.push(amp);
                self.falls.push(amp / length);
            }
        }
    }

    /// One camera update of `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let sum: f32 = self.amps.iter().sum();
        let mut i = 0;
        while i < self.amps.len() {
            self.amps[i] -= dt * self.falls[i];
            if self.amps[i] <= 0.0 {
                self.amps.remove(i);
                self.falls.remove(i);
            } else {
                i += 1;
            }
        }
        self.turn = if sum > 0.0 {
            let mut r = || {
                let x = self.rng.float() * 2.0 - 1.0;
                x * x * x * sum * TURN
            };
            Vec3::new(r(), r(), r())
        } else {
            Vec3::ZERO
        };
    }

    /// `camera` turned by this update's shake: x about its right axis
    /// (pitch), y about its up axis (yaw), z about its view axis (roll).
    /// OURS: the PS2 multiplies three axis turns into the camera matrix;
    /// added to the yaw / pitch / roll they agree to first order. trace:
    /// at most 0.1·S rad per axis.
    pub fn apply(&self, camera: Camera) -> Camera {
        Camera {
            pitch: camera.pitch + self.turn.x,
            yaw: camera.yaw + self.turn.y,
            roll: camera.roll + self.turn.z,
            ..camera
        }
    }

    pub fn is_shaking(&self) -> bool {
        self.turn != Vec3::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAIL_EXP: Shake = Shake {
        amount: 1.0,
        inner: 0.0,
        outer: 35.0,
        length: 1.0,
    };

    #[test]
    fn the_snail_tank_blast_matches_the_ps2_ram() {
        let mut s = CameraShake::default();
        s.blast(&SNAIL_EXP, 28.52, 5.358);
        assert_eq!(s.amps.len(), 2);
        assert!((s.amps[0] - 0.2617).abs() < 2e-4, "{}", s.amps[0]);
        assert!((s.falls[0] - 1.0).abs() < 1e-6);
        s.tick(1.0 / 30.0);
        assert!(s.is_shaking());
        assert!((s.amps[0] - (0.2617 - 1.0 / 30.0)).abs() < 2e-4);
        assert!(s.turn.abs().max_element() <= 2.0 * 0.2617 * TURN + 1e-6);
    }

    #[test]
    fn out_of_reach_and_spent_shakes_leave_the_camera_still() {
        let mut s = CameraShake::default();
        s.blast(&SNAIL_EXP, 40.0, 5.358);
        s.blast(
            &Shake {
                outer: 0.0,
                ..SNAIL_EXP
            },
            1.0,
            1.0,
        );
        s.tick(1.0 / 30.0);
        assert!(!s.is_shaking());
        s.blast(&SNAIL_EXP, 3.0, 1.0);
        for _ in 0..31 {
            s.tick(1.0 / 30.0);
        }
        assert!(s.amps.is_empty());
        s.tick(1.0 / 30.0);
        assert!(!s.is_shaking());
    }

    #[test]
    fn at_most_four_shakes_at_once() {
        let mut s = CameraShake::default();
        for _ in 0..3 {
            s.blast(&SNAIL_EXP, 3.0, 1.0);
        }
        assert_eq!(s.amps.len(), MAX_SHAKES);
    }
}
