//! Player maneuver requests and the BF2 pitch reversal. The existing side
//! roll drives the same craft motion for humans and bots.

use super::{approach, side_roll, FlyerParams, FlyerState};
use crate::sim::buttons;
use crate::vehicle::HoverInput;
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};

// B2 FUN_00202d08, 117099 and 118310–118350.
const DIRECTION_WINDOW: f32 = 1.0;
const DIRECTION_THRESHOLD: f32 = 0.2;
// B2 FUN_002b0548, 234961–234999: trigger ticks at 1,966,080/s.
const MAX_TAP_HOLD: f32 = 0.3;
const DOUBLE_PRESS_WINDOW: f32 = 0.5;
// B2 FUN_00208e28, 120102–120140: 5π/4 or 13π/4 depending on speed.
const SLOW_REVERSAL_ROLL: f32 = 5.0 * PI / 4.0;
const FAST_REVERSAL_ROLL: f32 = 13.0 * PI / 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotManeuvers {
    input: u8,
    held: u8,
    tap_hold: f32,
    double_window: f32,
    direction_window: f32,
    reversing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManeuverKind {
    SideRoll,
    Reversal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Maneuver {
    SideRoll(Vec3),
    Reverse { pitch: f32, roll: f32 },
}

impl PilotManeuvers {
    /// Called only for the human at the pilot controls. The sim records these
    /// logical buttons, so the gesture can replay without a connected device.
    pub fn observe(&mut self, buttons: u32) {
        self.input = u8::from(buttons & (buttons::ROLL_STANCE | buttons::FLYER_TRICK) != 0)
            | (u8::from(buttons & buttons::FLYER_REVERSE != 0) << 1);
    }

    pub(super) fn take_request(
        &mut self,
        input: HoverInput,
        allowed: bool,
        busy: bool,
        can_reverse: bool,
        dt: f32,
    ) -> Option<Maneuver> {
        let down = std::mem::take(&mut self.input);
        let was_down = self.held;
        let edges = down & !self.held;
        self.held = down;
        if !allowed || busy {
            self.tap_hold = MAX_TAP_HOLD + dt;
            if down & 1 == 0 {
                self.tap_hold = 0.0;
            }
            self.double_window = 0.0;
            self.direction_window = 0.0;
            self.reversing = false;
            return None;
        }

        self.double_window = (self.double_window - dt).max(0.0);
        if was_down & 1 != 0 {
            self.tap_hold = (self.tap_hold + dt).min(MAX_TAP_HOLD + dt);
            if down & 1 == 0 {
                if self.tap_hold <= MAX_TAP_HOLD + f32::EPSILON {
                    self.double_window = DOUBLE_PRESS_WINDOW;
                }
                self.tap_hold = 0.0;
            }
        }
        self.direction_window = (self.direction_window - dt).max(0.0);
        if edges & 2 != 0 && can_reverse {
            self.clear_gesture();
            return Some(Maneuver::Reverse {
                pitch: -PI,
                roll: 0.0,
            });
        }
        if edges & 1 != 0 {
            self.reversing = self.double_window > 0.0 && can_reverse;
            self.double_window = 0.0;
            self.direction_window = DIRECTION_WINDOW;
        }
        if self.direction_window == 0.0 {
            self.reversing = false;
            return None;
        }

        // The native gesture uses steering axes. The manual roll axis also
        // selects a side when yaw is neutral, so A/D and either pad layout work.
        let turn = if input.turn.abs() > DIRECTION_THRESHOLD {
            input.turn
        } else {
            input.strafe
        };
        let pitch = input.pitch;
        if turn.abs().max(pitch.abs()) <= DIRECTION_THRESHOLD {
            return None;
        }
        let reversing = self.reversing;
        self.clear_gesture();
        if reversing {
            self.double_window = 0.0;
            let (pitch, roll) = if pitch.abs() > turn.abs() {
                (-PI.copysign(pitch), 0.0)
            } else {
                (-PI, turn.signum())
            };
            Some(Maneuver::Reverse { pitch, roll })
        } else {
            Some(Maneuver::SideRoll(Vec3::new(turn, -pitch, 0.0).normalize()))
        }
    }

    fn clear_gesture(&mut self) {
        self.direction_window = 0.0;
        self.reversing = false;
    }

    pub(super) fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        if self != Self::default() {
            eat(b"FM");
            eat(&[self.input, self.held, u8::from(self.reversing)]);
            eat(&self.tap_hold.to_bits().to_le_bytes());
            eat(&self.double_window.to_bits().to_le_bytes());
            eat(&self.direction_window.to_bits().to_le_bytes());
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightReversal {
    angle: f32,
    target: f32,
    settling: bool,
}

impl FlightReversal {
    pub(super) fn active(self) -> bool {
        self.target != 0.0 || self.settling
    }

    pub(super) fn pitch(self) -> f32 {
        self.angle
    }

    pub(super) fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        if self != Self::default() {
            eat(b"FR");
            eat(&self.angle.to_bits().to_le_bytes());
            eat(&self.target.to_bits().to_le_bytes());
            eat(&[u8::from(self.settling)]);
        }
    }
}

pub(super) fn start(s: &mut FlyerState, p: &FlyerParams, request: Maneuver, speed: f32) {
    match request {
        Maneuver::SideRoll(side) => side_roll(s, p, side),
        Maneuver::Reverse { pitch, roll } => {
            if s.drained {
                return;
            }
            s.energy -= p.trick_flip_drain;
            if s.energy < 0.0 {
                s.energy = 0.0;
                s.drained = true;
            }
            s.reversal.target = pitch;
            if roll != 0.0 {
                let angle = if speed >= (p.min_speed + p.mid_speed) * 0.5 {
                    FAST_REVERSAL_ROLL
                } else {
                    SLOW_REVERSAL_ROLL
                };
                s.crash_roll_target += angle * roll;
                s.in_roll = true;
            }
        }
    }
}

/// B2 FUN_00208a20. The pitch is a bank target until the half-turn finishes.
/// Rebasing the flight frame reverses forward/right and preserves the
/// displayed pose across that tick.
pub(super) fn advance_reversal(s: &mut FlyerState, p: &FlyerParams, frame: &mut Quat, dt: f32) {
    if s.reversal.target == 0.0 {
        return;
    }
    if s.reversal.angle != s.reversal.target {
        s.reversal.angle = approach(s.reversal.angle, s.reversal.target, p.trick_flip_speed * dt);
        return;
    }
    *frame *= Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
    s.tilt_pitch += PI;
    if s.tilt_pitch > PI {
        s.tilt_pitch -= TAU;
    }
    let offset = PI.copysign(s.crash_roll_target);
    s.tilt_roll = offset - s.tilt_roll;
    s.crash_roll = offset - s.crash_roll;
    s.crash_roll_target = offset - s.crash_roll_target;
    s.reversal = FlightReversal {
        settling: true,
        ..Default::default()
    };
}

/// The retail roll update adds a recovery roll when the rebase leaves the
/// target upside down (B2 FUN_00208520, 119771–119795).
pub(super) fn continue_upside_down_roll(s: &mut FlyerState, p: &FlyerParams, speed: f32) -> bool {
    if !s.reversal.settling || (s.crash_roll_target.abs() - PI).abs() >= 0.001 {
        return false;
    }
    let angle = if speed > p.min_speed * 0.9 {
        FAST_REVERSAL_ROLL
    } else {
        SLOW_REVERSAL_ROLL
    };
    s.crash_roll_target += angle.copysign(s.roll_delta);
    s.in_roll = true;
    true
}

pub(super) fn finish_reversal(s: &mut FlyerState, pitch_delta: f32) {
    if s.reversal.settling && !s.in_roll && pitch_delta.abs() < 0.01 {
        s.reversal = FlightReversal::default();
    }
}
