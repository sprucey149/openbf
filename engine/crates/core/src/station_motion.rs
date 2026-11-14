//! A service droid's idle turn and serving state (B1 `FUN_002a9750`, `analysis/re-powerup-station.md` §14), shared
//! by the sim (collision bodies and blast sphere follow the yaw) and the viewer (the drawn pose). Outside `sim` so the
//! golden scenes, which place droids but build no Sim, do not link `core::sim` (`engine/goldens/deps.toml`).

use crate::fmath::FloatExt;

/// Per-station LCG from B1 FUN_002a9750 (§6).
fn lcg(x: u32) -> u32 {
    x.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f)
}

/// PS2 state (+0x2e4), timer (+0x6f4), spin sign (+0x6f8), wobble frequency
/// (+0x6fc), plus accumulated yaw and a private deterministic generator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationMotion {
    pub state: u8,
    pub timer: f32,
    pub sign: f32,
    pub wobble_freq: f32,
    pub yaw: f32,
    rng: u32,
}

impl StationMotion {
    /// Constructor state: turn (0), zero timer/yaw, positive spin sign.
    pub fn new(seed: u32) -> Self {
        StationMotion {
            state: 0,
            timer: 0.0,
            sign: 1.0,
            wobble_freq: 0.0,
            yaw: 0.0,
            rng: seed,
        }
    }

    /// Two LCG draws form one uniform float in [0, 1), as FUN_002a9750.
    fn uniform(&mut self) -> f32 {
        let a = lcg(self.rng);
        self.rng = lcg(a);
        let bits = ((a >> 16) | (self.rng & 0xffff_0000)) >> 9 | 0x3f80_0000;
        f32::from_bits(bits) - 1.0
    }

    /// Advance the authored four-state idle/serving machine by one sim step.
    /// Turn duration is (0.75 + 0.5u) pi / speed; wobble does six rocks per turn.
    pub fn step(&mut self, anim: &crate::sim::StationAnim, serving: bool, dt: f32) {
        let busy = matches!(self.state, 2 | 3);
        if serving && !busy {
            self.state = 3;
        } else if !serving && busy {
            self.state = 1;
        }
        match self.state {
            1 => {
                self.timer -= dt;
                if self.timer >= 0.0 {
                    return;
                }
                self.state = 0;
                self.sign = -self.sign;
                if anim.rotate_speed <= 0.0001 {
                    self.timer = 0.0;
                    return;
                }
                let t = ((self.uniform() * 0.5 + 0.75) * std::f32::consts::PI) / anim.rotate_speed;
                self.timer = t;
                if anim.wobble_node.is_some() {
                    self.wobble_freq = (12.0 * std::f32::consts::PI) / t;
                }
            }
            3 => self.timer -= dt,
            _ => {
                self.timer -= dt;
                if self.timer < 0.0 {
                    self.state = if self.state == 0 { 1 } else { 3 };
                    self.timer = 0.0;
                }
                self.yaw += self.sign * anim.rotate_speed * dt;
                if self.state == 1 {
                    self.timer = anim.wait_time;
                }
            }
        }
    }

    /// Include every dynamic field, including the generator, in replay state.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[self.state]);
        eat(&self.rng.to_le_bytes());
        for v in [self.timer, self.sign, self.wobble_freq, self.yaw] {
            eat(&v.hash_bits().to_le_bytes());
        }
    }
}
