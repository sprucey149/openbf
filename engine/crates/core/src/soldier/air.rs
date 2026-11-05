//! Knockback, take-off, the jet pack and steering in the air.

use super::*;

impl Soldier {
    /// The tumble, the get-up and a corpse (card KNOCK, §2-§4): no stick; on
    /// the floor the forward and side speeds each brake toward 0 by the
    /// state's rate ([`KNOCK_BRAKE_SHARE`] x Acceleration x the brake; BF1
    /// only on a floor of normal y over [`KNOCK_BRAKE_MIN_NY`], BF2 a tenth
    /// of it under its climb limit), in the air gravity (BF1's get-up
    /// [`BF1_THROWN_GRAVITY`]); then the state's end.
    pub(super) fn knock_slide(
        &mut self,
        dt: f32,
        (t1, t2): (Vec3, Vec3),
        n: Vec3,
        c: &SoldierConstants,
        rules: GroundRules,
    ) {
        let bf1 = rules == GroundRules::Bf1;
        let t = self.time_in_stance;
        let decel = match self.stance {
            Stance::Tumble => c.acceleration * KNOCK_BRAKE_SHARE * TUMBLE_BRAKE,
            Stance::GetUp => c.acceleration * KNOCK_BRAKE_SHARE * GETUP_BRAKE,
            _ => {
                let k = t.min(1.0) * t.min(1.0);
                if bf1 {
                    c.acceleration * KNOCK_BRAKE_SHARE * k
                } else {
                    BF2_CORPSE_BRAKE * k
                }
            }
        };
        if self.grounded {
            let floor = match rules {
                GroundRules::Bf1 if n.y > KNOCK_BRAKE_MIN_NY => 1.0,
                GroundRules::Bf1 => 0.0,
                GroundRules::Bf2 => rules.slope_control(n.y),
            };
            let a = decel * floor * dt;
            let (f, s) = (self.vel.dot(t1), self.vel.dot(t2));
            let (nf, ns) = (f - f.clamp(-a, a), s - s.clamp(-a, a));
            self.vel += t1 * (nf - f) + t2 * (ns - s);
        } else {
            let g = if self.stance == Stance::GetUp {
                rules.thrown_gravity()
            } else {
                GRAVITY
            };
            self.vel.y -= g * dt;
        }
        self.pos += self.vel * dt;
        let sp2 = self.vel.length_squared();
        match self.stance {
            Stance::Tumble => {
                let max = if bf1 { BF1_TUMBLE_MAX } else { BF2_TUMBLE_MAX };
                if t > TUMBLE_MIN && (sp2 < TUMBLE_END_SPEED * TUMBLE_END_SPEED || t >= max) {
                    self.set_stance(Stance::GetUp);
                }
            }
            Stance::GetUp
                if self.grounded && (sp2 < GETUP_END_SPEED * GETUP_END_SPEED || t > GETUP_MAX) =>
            {
                self.set_stance(Stance::Stand);
                self.lock_timer = GETUP_LOCK;
                self.lock_hard = true;
                self.fire_lock = true;
            }
            _ => {}
        }
    }

    /// A blast's push (card KNOCK, §1 / §4): `dir` = normalize(d) + (0,
    /// [`KNOCK_LIFT`], 0), `scale` = Push x the falloff; `draw` is the
    /// landing's [`KNOCK_TUMBLE_DRAW`] roll. The caller has tested the
    /// skipped states ([`Stance::knocked`], a seat).
    /// - **BF1** (FUN_002008b0): v += dir x scale; a v.y over
    ///   [`KNOCK_THROW_VY`] throws him, else a ground speed over
    ///   [`KNOCK_TUMBLE_SPEED`] tumbles him.
    /// - **BF2** (FUN_002388b8): half the velocity against `dir` goes first;
    ///   thrown in the hover by a push over [`BF2_KNOCK_HOVER_SCALE`], in the
    ///   air by one of [`BF2_KNOCK_AIR_SCALE`] or more, on the floor when
    ///   v . its normal is over [`BF2_KNOCK_THROW_VN`], else tumbling over
    ///   [`BF2_KNOCK_TUMBLE_SQ`]. A push that does neither flinches him
    ///   ([`Soldier::flinch`]): the stagger a hero shows under a saber hit.
    ///
    /// The state setter refuses a corpse ([`Stance::Dead`]): it keeps the
    /// velocity and its state (B1 FUN_00209758).
    pub fn knock(&mut self, dir: Vec3, scale: f32, rules: GroundRules, draw: f32) {
        self.knock_draw = draw;
        let next = match rules {
            GroundRules::Bf1 => {
                self.vel += dir * scale;
                if self.vel.y > KNOCK_THROW_VY {
                    Some(Stance::Thrown)
                } else if self.vel.x * self.vel.x + self.vel.z * self.vel.z
                    > KNOCK_TUMBLE_SPEED * KNOCK_TUMBLE_SPEED
                {
                    Some(Stance::Tumble)
                } else {
                    None
                }
            }
            GroundRules::Bf2 => {
                let along = self.vel.dot(dir);
                if along < 0.0 {
                    self.vel -= dir * (0.5 * along);
                }
                self.vel += dir * scale;
                if self.stance == Stance::Thrown || self.stance == Stance::Dead {
                    None
                } else if self.stance == Stance::Hover {
                    if scale > BF2_KNOCK_HOVER_SCALE {
                        Some(Stance::Thrown)
                    } else {
                        if scale > BF2_FLINCH_FLOOR_SCALE {
                            self.flinch((scale - BF2_FLINCH_FLOOR_SCALE) / BF2_FLINCH_HOVER_RATE);
                        }
                        None
                    }
                } else if !self.grounded
                    || matches!(self.stance, Stance::Jump | Stance::Jet | Stance::Fall)
                {
                    if scale >= BF2_KNOCK_AIR_SCALE {
                        Some(Stance::Thrown)
                    } else {
                        if scale > BF2_FLINCH_AIR_SCALE {
                            self.flinch(BF2_FLINCH_AIR_SECS);
                        }
                        None
                    }
                } else if self.vel.dot(self.ground_n) > BF2_KNOCK_THROW_VN {
                    Some(Stance::Thrown)
                } else if self.vel.length_squared() > BF2_KNOCK_TUMBLE_SQ {
                    Some(Stance::Tumble)
                } else {
                    if scale > BF2_FLINCH_FLOOR_SCALE {
                        if self.stance == Stance::Sprint {
                            self.set_stance(Stance::Stand);
                        }
                        self.flinch(BF2_FLINCH_FLOOR_SECS);
                    }
                    None
                }
            }
        };
        if let Some(s) = next.filter(|&s| s != self.stance && self.stance != Stance::Dead) {
            self.set_stance(s);
        }
    }

    /// BF2's flinch: every input, the trigger included, is locked for
    /// `secs` (a longer lock already running is kept).
    fn flinch(&mut self, secs: f32) {
        self.lock_timer = self.lock_timer.max(secs);
        self.lock_hard = true;
        self.fire_lock = true;
    }

    pub fn force_knock(&mut self, dir: Vec3, scale: f32, draw: f32) {
        self.vel = Vec3::ZERO;
        self.knock(dir, scale, GroundRules::Bf2, draw);
        if self.stance.knocked() {
            self.knock_rules = GroundRules::Bf2;
        }
    }

    /// A hold takes him out of whatever he was doing, a throw's state included.
    pub fn catch_upright(&mut self) {
        if self.stance != Stance::Stand {
            self.set_stance(Stance::Stand);
        }
    }

    /// A death's kick: [`DEATH_KICK`] m/s along `dir` (§3).
    pub fn kick(&mut self, dir: Vec3) {
        self.vel += dir.normalize_or_zero() * DEATH_KICK;
    }

    /// Thrown, tumbling, getting up, dead, or in the get-up's lock: the
    /// trigger is held off (§2).
    pub fn no_fire(&self) -> bool {
        self.stance.knocked()
            || self.stance == Stance::Dead
            || (self.fire_lock && self.lock_timer > 0.0)
    }

    /// Merged take-off (JUNI, re-stance-roll.md §4.3b): BF2
    /// FUN_00246ed0 adds vertical speed, scales the positive forward
    /// component by JumpForwardSpeedFactor and the backward/side components
    /// by JumpStrafeSpeedFactor. BF1 FUN_00208b78 originally stored vertical
    /// speed and added facing * jump velocity * stick * 0.25. FEEL_NOTES
    /// records the deliberate BF1 deviations. The ground mover runs first;
    /// the first air update skips gravity. Class constants remain unchanged.
    pub(super) fn take_off(&mut self, c: &SoldierConstants) {
        self.jump_animation = None;
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let fwd_axis = Vec2::new(-sy, -cy);
        self.vel.y += c.jump_vy;
        let planar = Vec2::new(self.vel.x, self.vel.z);
        let f = planar.dot(fwd_axis);
        let s = planar.dot(Vec2::new(cy, -sy));
        let k = if f > 0.0 {
            c.jump_forward_factor
        } else {
            c.jump_strafe_factor
        };
        let (f2, s2) = (f * k, s * c.jump_strafe_factor);
        self.vel.x = fwd_axis.x * f2 + cy * s2;
        self.vel.z = fwd_axis.y * f2 - sy * s2;
        self.set_stance(Stance::Jump);
    }

    /// The fly-height ceiling (both games after the air movers; re-jetpack.md
    /// §5.3): within [`FLY_CEILING_BAND`] of `ceiling` the vertical speed is
    /// pulled toward that far under it, after the update's move, so it bends
    /// the next one.
    pub fn fly_ceiling(&mut self, dt: f32, ceiling: f32) {
        let gap = ceiling - self.pos.y;
        if gap < FLY_CEILING_BAND {
            self.vel.y += (gap - FLY_CEILING_BAND) * dt * FLY_CEILING_PULL;
        }
    }

    /// A jet's start (BF2 `FUN_002476a0`, re-jetpack.md §3.1; card JETP,
    /// BF2's rules on every map): refused under `JetFuelMinBorder` (equal
    /// passes) or on a depleted energy bar (`JetEnergyDrain` is paid as the
    /// jump's cost is, [`Soldier::pay_energy`]); then `JetFuelInitialCost`
    /// (fuel floored at 0) and the kick, `v += normalize(R x strafe x 0.1 +
    /// F x max(fwd, -0.1) + (0, 1.1, 0)) x JetJump`, R and F the body's level
    /// rows. A sabre class that does not hover double-jumps (§3.3); else the
    /// hover or the jump-jet. `stick` = (fwd, strafe), right positive.
    pub fn jet_on(&mut self, c: &SoldierConstants, stick: (f32, f32)) -> bool {
        self.jet_on_with_direction(c, stick, None)
    }

    pub(super) fn jet_on_with_direction(
        &mut self,
        c: &SoldierConstants,
        stick: (f32, f32),
        direction: Option<Vec3>,
    ) -> bool {
        if self.fuel < c.jet_fuel_min_border || !self.pay_stamina(c.jet_energy_drain, c) {
            return false;
        }
        self.fuel = (self.fuel - c.jet_fuel_initial).max(0.0);
        let (fwd_in, strafe_in) = stick;
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let f = Vec3::new(-sy, 0.0, -cy);
        let r = Vec3::new(cy, 0.0, -sy);
        let dir = (r * (strafe_in * JET_KICK_STRAFE)
            + f * fwd_in.max(JET_KICK_BACK)
            + Vec3::Y * JET_KICK_LIFT)
            .normalize();
        // The PS2 uses the controller vector without normalising it. Zero
        // means use the ordinary kick (FUN_002476a0, 0x247814–0x247848).
        let dir = direction
            .filter(|d| d.length_squared() > 0.0)
            .unwrap_or(dir);
        self.vel += dir * c.jet_jump;
        if c.saber && !c.jet_hover {
            let direction = if fwd_in * fwd_in + strafe_in * strafe_in < 0.0625 {
                0
            } else if fwd_in > strafe_in.abs() {
                1
            } else if -fwd_in > strafe_in.abs() {
                2
            } else if strafe_in < 0.0 {
                3
            } else {
                4
            };
            let restart = self.jump_animation != Some(direction)
                || direction < 2
                || (self.stance == Stance::Jump && self.time_in_stance >= 0.5);
            // The Jedi's double jump: the kick's horizontal part is thrown
            // away for the stick's run speeds, and the rise is the jump's
            // again, or 1.5 times it (B2:162553-162600).
            let run = if fwd_in > 0.0 {
                c.max_speed
            } else {
                c.max_strafe_speed
            };
            let j = c.jump_vy;
            let vy = self.vel.y;
            self.vel = f * (fwd_in * run) + r * (strafe_in * c.max_strafe_speed);
            self.vel.y = if vy < 0.5 * j { vy + j } else { 1.5 * j };
            let age = self.time_in_stance;
            self.set_stance(Stance::Jump);
            self.jump_animation = Some(direction);
            if restart {
                self.jump_animation_serial = self.jump_animation_serial.wrapping_add(1);
            } else {
                self.time_in_stance = age;
            }
        } else {
            self.set_stance(if c.jet_hover {
                Stance::Hover
            } else {
                Stance::Jet
            });
        }
        true
    }

    /// The jet stances' update (BF2 `FUN_00240bf8` cases 6 / 7, re-jetpack.md
    /// §4.1-§4.2): the fuel's seconds (empty ends it), a tap ends it, else
    /// the stance's mover and the ceiling. Ending falls (stance 8) with no
    /// move this update; a sabre class tries its jet again at once (Vader and
    /// the Emperor, whose border is a full tank, are refused). `aim` is the
    /// aim before this update's turn (yaw, pitch).
    pub(super) fn step_jet(
        &mut self,
        dt: f32,
        stick: (f32, f32),
        tap: bool,
        aim: (f32, f32),
        c: &SoldierConstants,
        ceiling: f32,
    ) {
        self.fuel -= c.jet_fuel_cost * dt;
        let empty = self.fuel < 0.0;
        if empty {
            self.fuel = 0.0;
        }
        if empty || tap {
            self.set_stance(Stance::Fall);
            if c.saber {
                self.jet_on(c, stick);
            }
            return;
        }
        let (fwd_in, strafe_in) = stick;
        let (sy, cy) = fmath::sin_cos(aim.0);
        let f = Vec3::new(-sy, 0.0, -cy);
        let r = Vec3::new(cy, 0.0, -sy);
        if self.stance == Stance::Jet {
            // §4.1: JetPush up, then BF2's air mover with the stick as an
            // acceleration of JetAcceleration (FUN_00248560): the drag,
            // the stick, gravity off the floor.
            self.vel.y += c.jet_push * dt;
            self.vel *= fmath::exp(-AIR_DRAG * self.vel.length() * dt);
            self.vel += (f * fwd_in + r * strafe_in) * (c.jet_acceleration * dt);
            if !self.grounded {
                self.vel.y -= GRAVITY * dt;
            }
        } else {
            // §4.2 (FUN_00248898): on the floor the into-floor part goes; all
            // speed fades by exp(-2 dt); along the flight axis D (the aim with
            // its height lifted by 0.08 and doubled) and the level right axis
            // the speed moves toward the jet row's targets by at most
            // JetAcceleration x dt. No gravity, no JetPush.
            if self.grounded {
                let vn = self.vel.dot(self.ground_n);
                if vn < 0.0 {
                    self.vel -= self.ground_n * vn;
                }
            }
            self.vel *= fmath::exp(-HOVER_DAMP * dt);
            let (sp, cp) = fmath::sin_cos(aim.1);
            let d = Vec3::new(f.x * cp, 2.0 * (sp + HOVER_LIFT), f.z * cp).normalize();
            let run = if fwd_in > 0.0 {
                c.max_speed
            } else {
                c.max_strafe_speed
            };
            let fwd_t = fwd_in * run * c.jet_control[0];
            let side_t = strafe_in * c.max_strafe_speed * c.jet_control[1];
            let a = c.jet_acceleration * dt;
            self.vel += d * (fwd_t - self.vel.dot(d)).clamp(-a, a);
            self.vel += r * (side_t - self.vel.dot(r)).clamp(-a, a);
        }
        self.pos += self.vel * dt;
        self.fly_ceiling(dt, ceiling);
    }

    /// Air steering: Jump/Fall select BF2 everywhere (JUNI); Thrown selects
    /// its original game. Original rules (card AIRC; re-stance-roll.md
    /// §4.3a, both games measured on open floor with PCSX2). `axes` are the
    /// level forward and right axes; `took_off` = the first update after the
    /// take-off (the PS2's floor bit is still set).
    ///
    /// - **BF2** (FUN_00240bf8 cases 4 / 8 -> FUN_00248560): the velocity is
    ///   scaled by exp(-[`AIR_DRAG`] * |v| * dt), then the stick is ADDED as an
    ///   acceleration, stick x ControlSpeed[jump] x Acceleration (2.1 m/s^2),
    ///   with no target and no cap. A standing jump with the stick forward
    ///   lands 0.94 m ahead at 1.85 m/s; a running jump keeps its speed but
    ///   for the drag, and the stick held back takes 2.6 m/s off it.
    /// - **BF1** (FUN_00205cc8 cases 4-6 -> FUN_002098b8, brake 0): each axis
    ///   is raised toward stick x [`BF1_AIR_SPEED`] x MaxSpeed (forward AND
    ///   back) / MaxStrafeSpeed at Acceleraton x [`BF1_ACCEL`] x
    ///   [`BF1_AIR_ACCEL`], and never lowered: a standing jump moves at
    ///   1.3 m/s from its first update (1.21 m), a running jump keeps its
    ///   speed with the stick released or held, and the stick held back
    ///   brings it to -1.3 m/s in 14 updates. The take-off update still has
    ///   the grounded bit, so its acceleration is the ground's (x 1).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn air_steer(
        &mut self,
        dt: f32,
        fwd_in: f32,
        strafe_in: f32,
        (t1, t2): (Vec3, Vec3),
        c: &SoldierConstants,
        rules: GroundRules,
        took_off: bool,
    ) {
        let ctl = self.stance.control();
        match rules {
            GroundRules::Bf2 => {
                self.vel *= fmath::exp(-AIR_DRAG * self.vel.length() * dt);
                if let Some(motion) = c.motion_target.filter(|m| m.weight > 0.0) {
                    // Native cases 4/8/9 replace the stick acceleration;
                    // this output is neither a velocity cap nor a lerp.
                    let weight = motion.weight.min(1.0);
                    self.vel +=
                        t1 * (motion.forward * weight * dt) + t2 * (motion.strafe * weight * dt);
                } else {
                    self.vel += t1 * (fwd_in * ctl[0] * c.acceleration * dt)
                        + t2 * (strafe_in * ctl[1] * c.acceleration * dt);
                }
            }
            GroundRules::Bf1 => {
                let air = if took_off { 1.0 } else { BF1_AIR_ACCEL };
                let a = c.acceleration * BF1_ACCEL * air * dt;
                let raise = |cur: f32, tgt: f32, stick: f32| -> f32 {
                    if stick > 0.0 && cur < tgt {
                        (cur + a).min(tgt)
                    } else if stick < 0.0 && tgt < cur {
                        (cur - a).max(tgt)
                    } else {
                        cur
                    }
                };
                let (cur_fwd, cur_strafe) = (self.vel.dot(t1), self.vel.dot(t2));
                let new_fwd = raise(cur_fwd, fwd_in * BF1_AIR_SPEED * c.max_speed, fwd_in);
                let new_strafe = raise(
                    cur_strafe,
                    strafe_in * BF1_AIR_SPEED * c.max_strafe_speed,
                    strafe_in,
                );
                self.vel += t1 * (new_fwd - cur_fwd) + t2 * (new_strafe - cur_strafe);
            }
        }
    }

    /// A floor contact. BF1's stand timer restarts at 5 s when it ends an
    /// airborne spell (B1:117407-117409), retaining the long fall time.
    /// JUNI jump eligibility no longer depends on this timer.
    pub(super) fn touch_down(&mut self, rules: GroundRules) {
        if rules == GroundRules::Bf1 && self.air_time > 0.0 {
            self.stand_age = self.stand_age.max(5.0);
        }
        self.grounded = true;
        self.air_time = 0.0;
    }
}
