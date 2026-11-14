use crate::fmath::FloatExt;
use glam::Vec3;

use super::{
    Body, Droideka, DroidekaClass, Input, Report, BALL_BOOST_STICK, BALL_JUMP_WINDOW,
    BALL_LEAN_RATE, BALL_TURN_DEADZONE, BALL_TURN_MIN_FWD, ROLLING,
};
use crate::fmath;

impl Droideka {
    /// The ball's sprint spends the one bar the shield uses. The disc's boost
    /// spent the hidden jump pool; the bar is the maintainer's port rule. The
    /// lockout's release is everyone's; only a player's empty bar sets it here
    /// (a bot never sprints or jumps).
    pub(super) fn tick_sprint(&mut self, dt: f32, input: &Input, c: &DroidekaClass) {
        if self.boosting && !input.free_sprint {
            self.shield.points =
                (self.shield.points - c.ball_sprint_drain() * dt).fmax(c.ball_energy_floor());
        }
        if input.human && self.shield.points <= 0.0 {
            self.sprint_spent = true;
            self.boosting = false;
        } else if self.sprint_spent && self.shield.points >= c.ball_sprint_recover() {
            self.sprint_spent = false;
        }
    }

    /// The soldier's jump payment (`crate::soldier::Soldier::pay_energy`) on
    /// the shared bar, with [`Droideka::sprint_spent`] as its depleted latch:
    /// a cost of 0 always passes, a spent bar refuses, and a bar that cannot
    /// cover the cost incurs bounded debt and is marked spent. OURS (the maintainer,
    /// 2026-11-02): the disc spends a hidden pool of its own. PROVISIONAL,
    /// measure: the maintainer's ball jumps from a full and a low bar in live
    /// play.
    fn pay_bar(&mut self, cost: f32, floor: f32) -> bool {
        if cost <= 0.0 {
            return true;
        }
        if self.sprint_spent {
            return false;
        }
        self.shield.points -= cost;
        if self.shield.points <= 0.0 {
            self.shield.points = self.shield.points.fmax(floor);
            self.sprint_spent = true;
        }
        true
    }

    fn boost_on(&mut self) -> bool {
        if self.state != ROLLING || self.sprint_spent || self.shield.points <= 0.0 {
            return false;
        }
        self.boosting = true;
        true
    }

    fn jump(
        &mut self,
        c: &DroidekaClass,
        sprinting: bool,
        input: &Input,
        body: &mut Body,
        report: &mut Report,
    ) {
        if self.state != ROLLING || self.air >= BALL_JUMP_WINDOW {
            return;
        }
        let cost = if input.free_sprint {
            0.0
        } else {
            c.ball_jump_cost(sprinting)
        };
        if !self.pay_bar(cost, c.ball_energy_floor()) {
            report.energy_refused = true;
            return;
        }
        self.air = BALL_JUMP_WINDOW;
        body.vel.y += c.jump_vy;
        let (sy, cy) = fmath::sin_cos(body.yaw);
        let facing = Vec3::new(-sy, 0.0, -cy);
        let along = body.vel.dot(facing);
        if along > 0.0 {
            body.vel += facing * along * (c.jump_forward - 1.0);
        }
        report.jumped = true;
    }

    pub(super) fn ball(
        &mut self,
        dt: f32,
        input: &Input,
        c: &DroidekaClass,
        body: &mut Body,
        report: &mut Report,
    ) {
        let mut fwd = input.fwd;
        let turn = input.turn;
        if input.human && turn * turn > BALL_TURN_DEADZONE * BALL_TURN_DEADZONE {
            if (0.0..BALL_TURN_MIN_FWD).contains(&fwd) {
                fwd = BALL_TURN_MIN_FWD;
            } else if fwd < 0.0 && -fwd < BALL_TURN_MIN_FWD {
                fwd = -BALL_TURN_MIN_FWD;
            }
        }
        let rate = if !input.human {
            c.max_turn_speed
        } else if self.boosting {
            c.ball_boost_turn
        } else {
            c.ball_zero_stick_turn + (c.ball_turn_speed - c.ball_zero_stick_turn) * input.fwd.abs()
        };
        let d = turn * rate * dt;
        body.yaw -= d;
        let sign = if fwd > 0.0 { 1.0 } else { -1.0 };
        let step = dt * BALL_LEAN_RATE;
        let diff = turn * sign * c.ball_max_lean - self.lean;
        let lean = if step < diff {
            self.lean + step
        } else if step < -diff {
            self.lean - step
        } else {
            self.lean + diff
        };
        self.lean = lean / (1.0 + lean * lean).sqrt();
        let accel;
        if input.jump {
            let sprinting = self.boosting;
            self.boosting = false;
            self.jump(c, sprinting, input, body, report);
            accel = 0.0;
        } else if fwd > BALL_BOOST_STICK && input.boost && self.boost_on() {
            accel = fwd * c.ball_acceleration * c.ball_boost_scale;
        } else {
            self.boosting = false;
            accel = fwd * c.ball_acceleration;
        }
        self.omega += dt * accel;
        if body.grounded {
            let fr = c.ball_friction * dt;
            self.omega = if fr < self.omega {
                self.omega - fr
            } else if self.omega < -fr {
                self.omega + fr
            } else {
                0.0
            };
        }
        let mut cap = c.ball_omega_cap();
        if self.boosting {
            cap *= c.ball_boost_scale;
        }
        self.omega = self.omega.clamp(-cap, cap);
        let h = dt * self.omega * 0.5;
        let (a, b) = (self.spin[0], self.spin[1]);
        let nb = b + a * h;
        let na = a - b * h;
        let n = (na * na + nb * nb).sqrt();
        self.spin = [na / n, nb / n];
    }
}
