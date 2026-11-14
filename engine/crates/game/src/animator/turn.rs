//! The soldier's turns: root twist, roll travel and standing foot turns, with
//! the waist's rotation bounded for fast mouse aiming. Follows
//! `analysis/re-soldier-anim-layers.md` §11.

use super::{aim, AnimInput, Animator, Body, Game, Move, Rig, Special};
use crate::skin::Clip;
use core::fmath::FloatExt;
use std::f32::consts::{FRAC_PI_8, PI, TAU};

const TURN_RIGHT: [f32; 3] = [FRAC_PI_8, FRAC_PI_8, 0.471_238_9];
const TURN_LEFT_BF1: [f32; 3] = [-0.706_858_3, -1.178_097_2, -0.471_238_9];
const TURN_LEFT_BF2: [f32; 3] = [-0.706_858_3, -0.706_858_3, -0.471_238_9];
const TURN_PAST: f32 = 0.2;
const TWIST_KEEP: f32 = 0.75;

pub(super) fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

impl Animator {
    pub(super) fn step_twist(
        &mut self,
        i: &AnimInput,
        body: Body,
        was_body: Option<Body>,
        mv: Option<Move>,
        dt: f32,
    ) {
        if let Some(t) = i.travel() {
            self.travel = t;
        }
        let rolling = body == Body::Special(Special::Roll);
        let tumble = match body {
            Body::Special(Special::Tumble(back)) => Some(if back {
                wrap(self.travel - PI)
            } else {
                self.travel
            }),
            _ => None,
        };
        if let (Some(t), false) = (
            tumble,
            matches!(was_body, Some(Body::Special(Special::Tumble(_)))),
        ) {
            self.twist = t;
        }
        let target = match mv {
            _ if rolling => Some(self.travel),
            _ if tumble.is_some() => tumble,
            _ if matches!(body, Body::Special(Special::Land(_))) => None,
            Some(m) if m.moving() => Some(self.leg),
            Some(_) => None,
            None => Some(0.0),
        };
        if let Some(target) = target {
            let rest = wrap(target - self.twist);
            self.twist += rest * (1.0 - TWIST_KEEP.powf(30.0 * dt.fmax(0.0)));
        }
        self.twist = wrap(self.twist);
        let r = &mut self.roll;
        r.ended = None;
        if rolling {
            let gap = wrap(self.travel - self.twist);
            (r.updates, r.after, r.gap) = (r.updates + 1, None, gap);
        } else if r.updates > 0 {
            (r.updates, r.after, r.ended) = (0, Some(dt), Some(r.gap));
        } else if let Some(a) = &mut r.after {
            *a += dt;
        }
    }

    pub(super) fn turn_on_spot(
        &mut self,
        rig: &Rig,
        clips: &[Clip],
        s: usize,
        turned: f32,
        dt: f32,
        mouse_aim: bool,
    ) -> Move {
        self.twist -= turned;
        let aim = self.aim.eased().0;
        let constrain = |twist: f32| {
            if mouse_aim {
                aim + (twist - aim).clamp(-aim::YAW_LIMIT, aim::YAW_LIMIT)
            } else {
                twist
            }
        };
        if mouse_aim {
            self.twist = aim + wrap(self.twist - aim);
        }
        let left = match rig.game {
            Game::Bf1 => TURN_LEFT_BF1[s],
            Game::Bf2 => TURN_LEFT_BF2[s],
        };
        let turn_right = self.twist > TURN_RIGHT[s];
        let turn_left = self.twist < left;
        self.twist = constrain(self.twist);
        let mv = match self.legs.mv {
            m @ (Move::TurnLeft | Move::TurnRight) => {
                let len = self.lower.slot.map_or(0.0, |l| {
                    let f = clips[l.clip].frames.max(2) as f32;
                    let frames = match rig.game {
                        Game::Bf1 => f,
                        Game::Bf2 => f - 1.0,
                    };
                    frames / 30.0
                });
                if len > 0.0 {
                    self.twist += self.turn / len * dt;
                }
                if len <= self.lower.secs {
                    Move::Idle
                } else {
                    m
                }
            }
            _ if turn_right => {
                self.turn = -self.twist - TURN_PAST;
                Move::TurnRight
            }
            _ if turn_left => {
                self.turn = TURN_PAST - self.twist;
                Move::TurnLeft
            }
            _ => Move::Idle,
        };
        self.twist = constrain(self.twist);
        mv
    }
}
