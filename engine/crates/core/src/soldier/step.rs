//! One soldier tick on foot: `Soldier::step_looking`.

use super::*;

impl Soldier {
    pub fn step_looking(
        &mut self,
        dt: f32,
        pad: &PadState,
        c: &SoldierConstants,
        rules: GroundRules,
        look_scale: f32,
        env: AirEnv,
    ) {
        let fwd_in = PadState::axis(pad.ly);
        let strafe_in = PadState::axis(pad.lx);
        let turn_in = PadState::axis(pad.rx);
        let pitch_in = PadState::axis(pad.ry);
        let roll_stance_edge = pad.buttons & buttons::ROLL_STANCE != 0
            && self.prev_buttons & buttons::ROLL_STANCE == 0;
        let jump_edge = pad.buttons & buttons::JUMP != 0 && self.prev_buttons & buttons::JUMP == 0;

        self.time_in_stance += dt;
        self.stand_age += dt;
        self.lock_timer = (self.lock_timer - dt).max(0.0);
        if self.lock_timer <= 0.0 {
            self.lock_hard = false;
            self.fire_lock = false;
        }
        // Jet fuel (card JETP): BF2 recharges it outside the jet stances,
        // before the stance switch (B2:160108-160116), on every map (BF1
        // recharged it in them too).
        if !self.stance.jetting() {
            self.fuel = (self.fuel + c.jet_fuel_recharge * dt).min(1.0);
        }
        // BF1 hard lock (FUN_00205cc8 t<10 band): movement + turn frozen.
        let frozen = self.look_frozen();
        let (fwd_in, strafe_in, turn_in, pitch_in) = if frozen {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (fwd_in, strafe_in, turn_in, pitch_in)
        };

        // ── Aim (per-stance turn factor; prone pitch clamp — BF1) ─────────
        // The hover flies along the aim the PS2 wrote LAST update (its aim
        // update runs after the movement, re-jetpack.md §4.2 step 3).
        let aim0 = (self.yaw, self.pitch);
        (self.yaw, self.pitch) = self.turned(dt, (turn_in, pitch_in), c, env.human);
        if pad.has_look() && !frozen {
            (self.yaw, self.pitch) = self.mouse_turned(pad.look(), look_scale, c);
        }

        // ── Stance transitions ────────────────────────────────────────────
        let can_command = self.lock_timer <= 0.0;
        // The update after a take-off from the floor: the PS2's floor bit is
        // still set (no gravity, no landing — [`Soldier::take_off`]).
        let first_air =
            self.stance == Stance::Jump && self.time_in_stance <= dt && self.air_time <= dt;
        let mut jump = false;
        match self.stance {
            Stance::Roll => {
                if self.time_in_stance > c.roll_duration {
                    // openbf/BF2: roll always exits to stand + soft lock.
                    self.set_stance(Stance::Stand);
                    self.lock_timer = c.soft_lock;
                }
            }
            // A throw and its tail take no command; they end in the movement
            // below, after the update's integration, as on the PS2 (card
            // KNOCK: the landing row keeps the flight's speed).
            Stance::Thrown | Stance::Tumble | Stance::GetUp | Stance::Dead => {}
            Stance::Jump | Stance::Fall => {
                if self.grounded && !first_air {
                    self.set_stance(Stance::Stand);
                    self.lock_timer = c.soft_lock;
                }
            }
            // A jet lands on the floor with the light lock, whatever its
            // airtime (BF2 FUN_00246850 from 6 / 7, re-jetpack.md §4.4).
            Stance::Jet | Stance::Hover => {
                if self.grounded {
                    self.set_stance(Stance::Stand);
                    self.lock_timer = c.soft_lock;
                }
            }
            Stance::Stand | Stance::Crouch | Stance::Prone | Stance::Sprint => {
                if can_command && roll_stance_edge {
                    if Self::roll_predicate(fwd_in, strafe_in)
                        && self.stance != Stance::Prone
                        && c.roll_speed_factor > 0.0
                        && self.pay_stamina(c.energy_cost_roll, c)
                    {
                        self.set_stance(Stance::Roll);
                        // BF2 FUN_002471b0: scale planar velocity to exactly
                        // RollSpeedFactor*MaxSpeed when speed in (0.1, target).
                        let target = c.roll_speed_factor * c.max_speed;
                        let planar = Vec2::new(self.vel.x, self.vel.z);
                        let sp2 = planar.length_squared();
                        if 0.01 < sp2 && sp2 < target * target {
                            let s = target / sp2.sqrt();
                            self.vel.x *= s;
                            self.vel.z *= s;
                        }
                    } else if let Some(next) = (!Self::roll_predicate(fwd_in, strafe_in))
                        .then(|| Self::stance_tap_target(self.stance, self.time_in_stance))
                        .flatten()
                    {
                        // Stationary: [`Soldier::stance_tap_target`].
                        self.change_stance(next, c);
                    }
                }
                // Sprint enter/exit (hold-to-sprint; BF2 predicate).
                if self.stance == Stance::Sprint {
                    if self.energy <= 0.0 {
                        self.energy_depleted = true;
                        self.set_stance(Stance::Stand);
                    } else if !pad.pressed(buttons::SPRINT)
                        || !Self::sprint_predicate(fwd_in, strafe_in, true)
                    {
                        self.set_stance(Stance::Stand);
                    }
                } else if self.stance == Stance::Stand
                    && pad.pressed(buttons::SPRINT)
                    && Self::sprint_predicate(fwd_in, strafe_in, false)
                    && self.sprint_start_speed(c, rules)
                    && !self.energy_depleted
                    && self.energy > 0.0
                {
                    self.set_stance(Stance::Sprint);
                }
                // Jump: paid and applied after this update's ground move
                // ([`Soldier::pay_jump`], [`Soldier::take_off`]). A crouch
                // stands up instead (instant, no lock: both games), and so
                // does a prone (BF1's prone case; `[ours]` in BF2), through
                // the get-up's lock: neither launches a jump.
                if can_command && jump_edge {
                    match self.stance {
                        Stance::Stand | Stance::Sprint => jump = self.jump_open(dt, rules),
                        Stance::Crouch | Stance::Prone
                            if self.time_in_stance > CROUCH_RISE_WAIT =>
                        {
                            self.change_stance(Stance::Stand, c)
                        }
                        _ => {}
                    }
                }
            }
        }

        // ── Energy (BF2 pool; restore idle vs moving) ─────────────────────
        if let Some(rate) = c.energy_override {
            if rate < 0.0 {
                self.pay_energy(-rate * dt);
            } else {
                self.energy += rate * dt;
            }
        } else if self.stance == Stance::Sprint {
            self.pay_stamina(c.energy_drain_sprint * dt, c);
        } else if matches!(self.stance, Stance::Stand | Stance::Crouch | Stance::Prone) {
            // FUN_00240bf8 B2:160209-160228: ordinary recovery uses the
            // incoming horizontal velocity, even after the stick is released.
            // All other postures have zero rate unless a weapon overrides it.
            let idle = self.vel.x * self.vel.x + self.vel.z * self.vel.z < 1.0;
            let rate = if idle {
                c.energy_restore_idle
            } else {
                c.energy_restore
            };
            if rate < 0.0 {
                self.pay_stamina(-rate * dt, c);
            } else {
                self.energy += rate * dt;
            }
        }
        self.energy = self.energy.clamp(c.energy_floor(), c.energy_bar);
        if self.energy_depleted && self.energy >= c.energy_recover_threshold {
            self.energy_depleted = false;
        }

        // ── Movement ──────────────────────────────────────────────────────
        let mut ctl = self.stance.control();
        if self.stance == Stance::Sprint {
            ctl[..2].copy_from_slice(&c.sprint_control);
        }
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let fwd_axis = Vec2::new(-sy, -cy);
        let right_axis = Vec2::new(cy, -sy);

        // Backpedal is capped by MaxStrafeSpeed in both games (stand case:
        // `fwd > 0 ? MaxSpeed : MaxStrafeSpeed`).
        let fwd_cap = if fwd_in > 0.0 {
            if self.stance == Stance::Sprint {
                // Ramp MaxSpeed -> MaxSpeed*ControlSpeed[sprint].fwd over
                // SprintAccelerateTime (BF2 FUN_00240bf8 case 3).
                let t = (self.time_in_stance / c.sprint_accelerate_time).min(1.0);
                c.max_speed * (1.0 + (ctl[0] - 1.0) * t)
            } else {
                c.max_speed * ctl[0]
            }
        } else {
            c.max_strafe_speed * ctl[0]
        };
        let mut target_fwd = fwd_in * fwd_cap;
        let mut target_strafe = strafe_in * c.max_strafe_speed * ctl[1];
        // Native cases 0..3 blend desired locomotion before acceleration.
        // Roll and the jet states keep their own movers.
        if (rules == GroundRules::Bf2 || c.saber)
            && matches!(
                self.stance,
                Stance::Stand | Stance::Crouch | Stance::Prone | Stance::Sprint
            )
        {
            if let Some(motion) = c.motion_target.filter(|m| m.weight > 0.0) {
                if motion.weight >= 1.0 {
                    target_fwd = motion.forward;
                    target_strafe = motion.strafe;
                } else {
                    target_fwd += (motion.forward - target_fwd) * motion.weight;
                    target_strafe += (motion.strafe - target_strafe) * motion.weight;
                }
            }
        }

        // The axes lie in the plane of the last floor contact (BF2
        // FUN_002480a0: T1 = R x N, T2 = N x T1; mirrored for the port's
        // handedness: T1 = N x R, T2 = T1 x N). Airborne, N is up. A grounded
        // soldier loses the into-N part of his velocity.
        let bf1 = rules == GroundRules::Bf1;
        let n = if self.grounded {
            self.ground_n
        } else {
            Vec3::Y
        };
        let r3 = Vec3::new(right_axis.x, 0.0, right_axis.y);
        let t1 = n
            .cross(r3)
            .normalize_or(Vec3::new(fwd_axis.x, 0.0, fwd_axis.y));
        let t2 = t1.cross(n).normalize_or(r3);
        if self.grounded {
            if bf1 && !(self.stance.knocked() && self.knock_rules == GroundRules::Bf2) {
                // The in-plane part of (0, -0.4*Acceleraton*dt, 0).
                let g = -0.4 * c.acceleration * dt;
                self.vel += Vec3::new(0.0, g, 0.0) - n * (g * n.y);
            }
            let vn = self.vel.dot(n);
            if vn < 0.0 {
                self.vel -= n * vn;
            }
        }
        if self.stance == Stance::Thrown {
            // The jump's stick (BF1 cases 4-6; BF2's air integrator), with
            // no gravity while the floor bit is still set: the update after
            // the push moves him straight off it (§2 [M]).
            let grounded = self.grounded;
            self.air_steer(
                dt,
                fwd_in,
                strafe_in,
                (t1, t2),
                c,
                self.knock_rules,
                grounded,
            );
            if !grounded {
                self.vel.y -= self.knock_rules.thrown_gravity() * dt;
            }
            self.pos += self.vel * dt;
            self.fly_ceiling(dt, env.ceiling);
            if grounded && self.time_in_stance > THROWN_LAND_AFTER {
                let sp2 = self.vel.length_squared();
                let tumble = sp2 > KNOCK_LAND_FAST * KNOCK_LAND_FAST
                    || (sp2 > KNOCK_LAND_SLOW * KNOCK_LAND_SLOW
                        && self.knock_draw > KNOCK_TUMBLE_DRAW);
                self.set_stance(if tumble {
                    Stance::Tumble
                } else {
                    Stance::GetUp
                });
            }
            self.prev_buttons = pad.buttons;
            return;
        }
        if matches!(self.stance, Stance::Tumble | Stance::GetUp | Stance::Dead) {
            self.knock_slide(dt, (t1, t2), n, c, self.knock_rules);
            self.prev_buttons = pad.buttons;
            return;
        }
        if self.stance.jetting() {
            let tap =
                (env.human && jump_edge) || (!env.human && env.bot_jet.is_some_and(|j| j.stop));
            let mut jet_aim = aim0;
            let mut jet_stick = (fwd_in, strafe_in);
            if !env.human && self.stance == Stance::Hover {
                if let Some(height) = env.bot_jet.and_then(|j| j.height) {
                    // FUN_00249f80: body pitch correction, separate from the
                    // weapon aim. The PS2 caps the correction at 30 * dt.
                    jet_aim.1 = (0.35 * (height - self.pos.y)).clamp(-30.0 * dt, 30.0 * dt);
                    // The port stores weapon aim separately from the bot's
                    // flight axis. Turn that axis along requested travel so
                    // height correction also works sideways or backwards.
                    let (sy, cy) = fmath::sin_cos(aim0.0);
                    let direction =
                        Vec3::new(-sy, 0.0, -cy) * fwd_in + Vec3::new(cy, 0.0, -sy) * strafe_in;
                    if direction.length_squared() > 0.0 {
                        jet_aim.0 = fmath::atan2(-direction.x, -direction.z);
                        jet_stick = (direction.length().min(1.0), 0.0);
                    }
                }
            }
            self.step_jet(dt, jet_stick, tap, jet_aim, c, env.ceiling);
            self.prev_buttons = pad.buttons;
            return;
        }
        if matches!(self.stance, Stance::Jump | Stance::Fall) {
            // JUNI: Jump/Fall use BF2 on every map; Thrown above keeps
            // its original per-game integrator and gravity.
            self.air_steer(
                dt,
                fwd_in,
                strafe_in,
                (t1, t2),
                c,
                GroundRules::Bf2,
                first_air,
            );
            if !first_air {
                self.vel.y -= GRAVITY * dt;
            }
            self.pos += self.vel * dt;
            // Then the ceiling and a second tap: the jet (card JETP; B2
            // FUN_00240bf8 cases 4 / 8, B2:160614-160618), its kick added to
            // the velocity the NEXT move takes, as the jump's is.
            self.fly_ceiling(dt, env.ceiling);
            if env.human && jump_edge {
                self.jet_on(c, (fwd_in, strafe_in));
            } else if !env.human && self.stance == Stance::Jump && self.lock_timer <= 0.0 {
                if let Some(jet) = env.bot_jet.filter(|j| !j.stop) {
                    self.jet_on_with_direction(c, (fwd_in, strafe_in), Some(jet.kick));
                }
            }
            self.prev_buttons = pad.buttons;
            return;
        }
        let cur_fwd = self.vel.dot(t1);
        let cur_strafe = self.vel.dot(t2);
        // A roll steers with a tiny factor but keeps momentum; the standing
        // stances approach the target at Acceleration — less on a steep
        // floor, and in the air by the game's rule.
        let accel = c.acceleration
            * match self.stance {
                Stance::Roll => ctl[0].max(ctl[1]), // 0.02
                _ if self.grounded => rules.slope_control(n.y),
                _ => rules.air_control(),
            }
            * dt;
        let (mut target_fwd, mut target_strafe) = (target_fwd, target_strafe);
        let (mut s1, mut s2) = (0.0, 0.0);
        if self.grounded {
            if bf1 {
                target_fwd *= n.y.abs();
                target_strafe *= n.y.abs();
            } else {
                // The slope's gravity shifts the clamp window (DAT_005fd0ac = 1).
                s1 = GRAVITY * dt * -t1.y;
                s2 = GRAVITY * dt * -t2.y;
                let up = target_fwd * t1.y + target_strafe * t2.y;
                let t2sum = target_fwd * target_fwd + target_strafe * target_strafe;
                if up > 0.0 && t2sum > 0.0 {
                    let k = 1.0 - up * up / t2sum;
                    target_fwd *= k;
                    target_strafe *= k;
                }
            }
        }
        let step_to = |cur: f32, tgt: f32, slide: f32| -> f32 {
            cur + (tgt - cur).clamp(slide - accel, slide + accel)
        };
        let new_fwd = step_to(cur_fwd, target_fwd, s1);
        let new_strafe = step_to(cur_strafe, target_strafe, s2);
        self.vel += t1 * (new_fwd - cur_fwd) + t2 * (new_strafe - cur_strafe);

        // ── Gravity + move (contact: `resolve_ground`, after the move) ────
        if !self.grounded {
            self.vel.y -= GRAVITY * dt;
        }
        self.pos += self.vel * dt;
        if jump && self.pay_jump(c, fwd_axis) {
            self.take_off(c);
        }

        self.prev_buttons = pad.buttons;
    }
}
