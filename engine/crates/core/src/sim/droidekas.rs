use glam::Vec3;

use super::input::{buttons, PadState};
use super::state::{DeathCause, Event, Sim};
use crate::collision::Capsule;
use crate::droideka::{local_to_world, Body, Droideka, DroidekaClass, Form, Input, Order, Sphere};
use crate::unit::{Controller, Unit};

pub const FLOOR_SNAP: f32 = 0.05;
pub const UNROLL_HEADROOM: f32 = 1.1;

pub fn capsule_of(c: &DroidekaClass, form: Form, pos: Vec3) -> Capsule {
    match form {
        Form::Ball => Capsule {
            base: pos,
            height: 2.0 * c.ball_radius,
            radius: c.ball_radius,
        },
        Form::Walk => Capsule {
            base: pos,
            height: c.walk_height,
            radius: c.walk_body.radius,
        },
    }
}

impl Unit {
    /// AI/perception origin. The droideka's class eye is in its yaw-only
    /// body frame (BF1 FUN_001d2fb8, B1:80053-80066); soldiers keep the muzzle.
    pub fn ai_eye(&self) -> Vec3 {
        self.droideka_class()
            .map_or_else(|| self.muzzle(), |c| self.droideka_point(c.eye))
    }

    pub fn droideka_class(&self) -> Option<&DroidekaClass> {
        self.spec.droideka.as_deref()
    }

    pub fn droideka_form(&self) -> Option<Form> {
        self.droideka.as_ref().map(|d| d.form())
    }

    pub fn droideka_point(&self, local: Vec3) -> Vec3 {
        local_to_world(self.soldier.pos, self.soldier.yaw, local)
    }

    pub fn droideka_sphere(&self, local: Sphere) -> Sphere {
        Sphere {
            center: self.droideka_point(local.center),
            radius: local.radius,
        }
    }

    pub fn droideka_shield(&self) -> Option<Sphere> {
        let d = self.droideka.as_ref()?;
        let c = self.droideka_class()?;
        d.shield.on.then(|| self.droideka_sphere(c.shield.sphere))
    }

    pub fn droideka_spheres(&self) -> Vec<Sphere> {
        let (Some(d), Some(c)) = (&self.droideka, self.droideka_class()) else {
            return Vec::new();
        };
        c.ordnance_spheres(d.form())
            .into_iter()
            .map(|s| self.droideka_sphere(s))
            .collect()
    }

    pub fn droideka_muzzle(&self) -> Option<Vec3> {
        self.droideka_gun(0)
    }

    pub fn droideka_gun(&self, ahead: u8) -> Option<Vec3> {
        let d = self.droideka.as_ref()?;
        let c = self.droideka_class()?;
        Some(self.droideka_point(c.gun(d.barrel.wrapping_add(ahead))))
    }

    pub(crate) fn lend_droideka_shield<R>(
        &mut self,
        f: impl FnOnce(&mut crate::weapon::Health) -> R,
    ) -> R {
        let Some(d) = self.droideka.as_mut().filter(|d| d.shield.on) else {
            return f(&mut self.health);
        };
        self.health.shield = d.shield.points;
        let out = f(&mut self.health);
        d.shield.points = std::mem::take(&mut self.health.shield);
        out
    }

    pub fn separation_radius(&self) -> f32 {
        match (self.droideka_class(), self.droideka_form()) {
            (Some(c), Some(Form::Ball)) => c.ball_radius,
            (Some(c), Some(Form::Walk)) => c.walk_soldier_radius,
            _ => self.capsule().radius,
        }
    }

    pub fn unpushed(&self) -> bool {
        self.droideka_form() == Some(Form::Ball)
    }

    /// What the stamina bar reads, (value, full). The maintainer: "The
    /// droideka stamina bar is how much shield is left." It is one pool: the
    /// shield standing, the sprint as a ball (`Droideka::tick_sprint`).
    pub fn stamina_bar(&self) -> (f32, f32) {
        match (&self.droideka, self.droideka_class()) {
            (Some(d), Some(c)) => (d.shield.points, c.shield.max),
            _ => (self.soldier.energy, self.consts.energy_bar),
        }
    }
}

/// Shared authored pitch clamp; droidekas never divide look by zoom.
fn droideka_pitch(
    class: &DroidekaClass,
    pitch: f32,
    pad: &PadState,
    mouse_scale: f32,
    dt: f32,
) -> f32 {
    let (lo, hi) = class.pitch_limits;
    let pitch = (pitch + PadState::axis(pad.ry) * class.max_pitch_speed * dt).clamp(lo, hi);
    if pad.has_look() {
        (pitch + pad.look().1 * mouse_scale).clamp(lo, hi)
    } else {
        pitch
    }
}

impl Sim {
    fn droideka_cramped(&self, d: &Droideka, class: &DroidekaClass, pos: Vec3) -> bool {
        let walk_room = capsule_of(class, Form::Walk, pos);
        d.form() == Form::Ball
            && !self.collision.is_empty()
            && self
                .collision
                .push_out_soldier(&Capsule {
                    height: walk_room.height * UNROLL_HEADROOM,
                    ..walk_room
                })
                .length_squared()
                > 1e-6
    }

    fn resolve_droideka_motion(
        &self,
        deka: &mut Droideka,
        class: &DroidekaClass,
        body: &mut Body,
        from: Capsule,
        dt: f32,
    ) -> (Option<Vec3>, Vec3) {
        let wall = (!self.collision.is_empty()).then(|| {
            let push = self
                .collision
                .push_out_soldier_from(&from, &capsule_of(class, deka.form(), body.pos));
            body.pos += push;
            let len = (push.x * push.x + push.z * push.z).sqrt();
            if len > 0.0 {
                let (nx, nz) = (push.x / len, push.z / len);
                let into = body.vel.x * nx + body.vel.z * nz;
                if into < 0.0 {
                    body.vel.x -= into * nx;
                    body.vel.z -= into * nz;
                }
            }
            push
        });
        let rising = body.vel.y > 0.0;
        let (fy, n) = self.ground_under_n(body.pos, rising, from.base.y);
        let grounded = !rising && body.pos.y <= fy + FLOOR_SNAP;
        if grounded {
            body.pos.y = fy;
        }
        deka.land(body, grounded, dt);
        (wall, n)
    }

    /// Validate the deployed droid's own body and movement on a marked exit.
    pub(crate) fn preview_droideka_walk(
        &self,
        u: &Unit,
        heading: Vec3,
        to: Vec3,
        descent: bool,
    ) -> Option<Vec3> {
        let class = u.droideka_class()?;
        let mut deka = u.droideka.as_deref()?.clone();
        deka.order = Order::Deploy;
        let start = u.soldier.pos;
        let mut body = Body {
            pos: start,
            vel: u.soldier.vel,
            yaw: u.soldier.yaw,
            grounded: u.soldier.grounded,
        };
        let mut last_floor = start.y;
        let (lx, ly) = PadState::walk_axes(body.yaw, heading);
        let mut input = Input {
            fwd: PadState::axis(ly),
            strafe: PadState::axis(lx),
            ..Input::default()
        };
        // OURS: allow time to unroll and walk the marked crossing.
        // measure: BF1 Kamino H49 to H48 on the same life (#108).
        for _ in 0..6 * crate::ai::STUCK_WINDOW {
            if body.grounded {
                last_floor = body.pos.y;
            }
            input.cramped = self.droideka_cramped(&deka, class, body.pos);
            let from = capsule_of(class, deka.form(), body.pos);
            deka.step(crate::TICK_DT, &input, class, &mut body);
            self.resolve_droideka_motion(&mut deka, class, &mut body, from, crate::TICK_DT);
            let capsule = capsule_of(class, deka.form(), body.pos);
            let depth = self
                .water_plane()
                .and_then(|w| w.under(body.pos))
                .map(|level| level - body.pos.y);
            let beyond_floor = if descent {
                body.pos.y < to.y - crate::collision::STEP_HEIGHT
            } else {
                !body.grounded && (body.pos.y - last_floor).abs() > crate::collision::STEP_HEIGHT
            };
            if beyond_floor
                || self.kill_floor.is_some_and(|floor| body.pos.y < floor)
                || self.death_regions.iter().any(|r| r.contains(body.pos))
                || self.damage_regions.iter().any(|r| {
                    r.region.touches_sphere(
                        capsule.base + Vec3::Y * (0.5 * capsule.height),
                        capsule.radius.max(0.5 * capsule.height),
                    )
                })
                || deka.water_tick(crate::TICK_DT, depth, class) > 0.0
            {
                return None;
            }
            if body.grounded
                && deka.walk_state()
                && body.pos.distance_squared(to)
                    <= crate::ai::RETURN_ARRIVE * crate::ai::RETURN_ARRIVE
                && self.level_at(body.pos) == super::Level::On
                && self.soldier_clear(body.pos)
            {
                return Some(body.pos);
            }
        }
        None
    }

    /// Converge a bot's barrel on its CURRENT aim line at the perceived range,
    /// keeping the tracker's error and the body's world-level pitch clamp.
    /// PROVISIONAL, trace: exact animated muzzle origins and aim timing on
    /// matched ramps in both originals. See analysis/re-droideka-unit.md.
    pub(super) fn bot_droideka_shot(&self, i: usize) -> Option<(Vec3, (Vec3, Vec3, Vec3))> {
        let u = &self.units[i];
        let c = u.droideka_class()?;
        let gun = u.droideka_muzzle()?;
        let contact = crate::perception::contact_of(&u.ai, u.ai.target?)?;
        let eye = u.ai_eye();
        let (aim, aim_right, _) = u.aim_frame();
        let point = eye + aim * contact.last_pos.distance(eye);
        let dir = (point - gun).normalize_or(aim);
        // Body yaw only: neither the ground normal nor the aim pitch tilts
        // the frame in which the authored aimer angles are measured.
        let body_fwd = Vec3::new(aim_right.z, 0.0, -aim_right.x);
        let yaw = crate::fmath::atan2(dir.dot(aim_right), dir.dot(body_fwd));
        let pitch = crate::fmath::asin(dir.y.clamp(-1.0, 1.0));
        let clamped_yaw = yaw.clamp(c.aimer_yaw_limits.0, c.aimer_yaw_limits.1);
        let clamped_pitch = pitch.clamp(c.aimer_pitch_limits.0, c.aimer_pitch_limits.1);
        let fwd = if yaw == clamped_yaw && pitch == clamped_pitch {
            dir
        } else {
            let (sy, cy) = crate::fmath::sin_cos(clamped_yaw);
            let (sp, cp) = crate::fmath::sin_cos(clamped_pitch);
            (body_fwd * cy + aim_right * sy) * cp + Vec3::Y * sp
        };
        let right = fwd.cross(Vec3::Y).normalize_or(aim_right);
        Some((gun, (fwd, right, right.cross(fwd).normalize())))
    }

    /// The same edge/headroom inputs for real motion and between-tick look.
    fn droideka_input(&self, i: usize, pad: &PadState) -> Option<Input> {
        let u = self.units.get(i)?;
        let deka = u.droideka.as_ref()?;
        let class = u.droideka_class()?;
        let human = matches!(u.controller, Controller::Player { .. });
        let prev = deka.prev_buttons;
        let edge = |b: u32| pad.buttons & b != 0 && prev & b == 0;
        let cramped = self.droideka_cramped(deka, class, u.soldier.pos);
        Some(Input {
            fwd: PadState::axis(pad.ly),
            strafe: PadState::axis(pad.lx),
            turn: PadState::axis(pad.rx),
            fold: edge(buttons::ROLL_STANCE),
            shield: edge(buttons::SECONDARY_FIRE),
            jump: edge(buttons::JUMP),
            // The maintainer (2026-11-01): a player's droideka sprints in ball
            // form only. Bots do not sprint. Held like a soldier's, where the
            // disc latches: OURS, measure: the maintainer's live ball sprint.
            boost: human && pad.pressed(buttons::SPRINT),
            human,
            free_sprint: u.is_player && self.cheats.infinite_stamina,
            cramped,
        })
    }

    pub(super) fn droideka_look_preview(&self, i: usize, pad: &PadState) -> Option<(f32, f32)> {
        let u = self.units.get(i)?;
        let class = u.droideka_class()?;
        let mut deka = u.droideka.as_deref()?.clone();
        let input = self.droideka_input(i, pad)?;
        deka.alive = u.alive;
        if u.controller == Controller::Ai {
            deka.order = crate::ai::droideka::form_order(&u.ai);
        }
        let scale = u.soldier.mouse_scale();
        let pitch = droideka_pitch(class, u.soldier.pitch, pad, scale, crate::TICK_DT);
        let mut body = Body {
            pos: u.soldier.pos,
            vel: u.soldier.vel,
            yaw: u.soldier.yaw,
            grounded: u.soldier.grounded,
        };
        if pad.has_look() {
            deka.turn_body(&mut body, pad.look().0 * scale);
        }
        // Run the actual walk/ball/boost integrator on scratch state. No
        // soldier turn-factor approximation can reproduce these state laws.
        deka.step(crate::TICK_DT, &input, class, &mut body);
        Some((body.yaw, pitch))
    }

    pub(super) fn step_droideka(&mut self, i: usize, pad: &PadState, dt: f32) {
        let Some(class) = self.units[i].spec.droideka.clone() else {
            return;
        };
        let Some(input) = self.droideka_input(i, pad) else {
            return;
        };
        let Some(mut deka) = self.units[i].droideka.take() else {
            return;
        };
        deka.prev_buttons = pad.buttons;
        deka.alive = self.units[i].alive;
        if self.units[i].controller == Controller::Ai {
            deka.order = crate::ai::droideka::form_order(&self.units[i].ai);
        }
        let u = &mut self.units[i];
        // Native droideka control has no zoom path. Its look always uses the
        // authored walk/ball rates, including while ZOOM is held.
        u.zoom = None;
        let mouse_scale = u.soldier.mouse_scale();
        u.soldier.pitch = droideka_pitch(&class, u.soldier.pitch, pad, mouse_scale, dt);
        let mut body = Body {
            pos: u.soldier.pos,
            vel: u.soldier.vel,
            yaw: u.soldier.yaw,
            grounded: u.soldier.grounded,
        };
        if pad.has_look() {
            deka.turn_body(&mut body, pad.look().0 * mouse_scale);
        }
        let from = capsule_of(&class, deka.form(), body.pos);
        let report = deka.step(dt, &input, &class, &mut body);
        if report.weapons_off {
            u.weapon.salvo_counter = 0;
        }
        let (wall, n) = self.resolve_droideka_motion(&mut deka, &class, &mut body, from, dt);
        if let Some(push) = wall {
            self.units[i].wall_push = push;
        }
        let depth = self
            .water_plane()
            .and_then(|w| w.under(body.pos))
            .map(|level| level - body.pos.y);
        let water = deka.water_tick(dt, depth, &class);
        let h = self.handle(i);
        for cue in report.cues() {
            self.events.push(Event::Droideka {
                unit: h,
                pos: body.pos,
                cue,
            });
        }
        let u = &mut self.units[i];
        u.soldier.pos = body.pos;
        u.soldier.vel = body.vel;
        u.soldier.yaw = body.yaw;
        u.soldier.grounded = body.grounded;
        u.soldier.ground_n = n;
        u.soldier.time_in_stance += dt;
        u.droideka = Some(deka);
        if water > 0.0 && u.alive {
            self.droideka_world_damage(i, water, DeathCause::Water);
        }
    }

    pub(super) fn droideka_world_damage(&mut self, i: usize, amount: f32, cause: DeathCause) {
        let h = self.handle(i);
        if self.units[i].is_player && self.cheats.invincible && cause.cheat_spares() {
            return;
        }
        let u = &mut self.units[i];
        let Some(d) = u.droideka.as_mut() else {
            return;
        };
        u.health.cur -= d.shield.absorb(amount, 1.0);
        if !u.health.alive() {
            self.kill_environment(h, cause);
        }
    }

    pub(super) fn droideka_burst(&mut self, i: usize) {
        let h = self.handle(i);
        let u = &self.units[i];
        let Some(c) = u.spec.droideka.clone() else {
            return;
        };
        let pos = u.soldier.pos + Vec3::Y * c.ball_radius;
        let (team, class, scale) = (u.team, u.spec.class, crate::unit::chunk_scale(u.health.cur));
        if let Some(e) = &c.explosion {
            self.explode(e, pos, None, Some(team));
        }
        let u = &mut self.units[i];
        if let Some(d) = u.droideka.as_mut() {
            d.alive = false;
            d.shield.on = false;
        }
        u.body = false;
        u.burst = true;
        u.soldier.stance = crate::soldier::Stance::Dead;
        u.soldier.time_in_stance = 0.0;
        self.events.push(Event::DroidBurst {
            unit: h,
            pos,
            team,
            class,
            scale,
        });
    }
}

#[cfg(test)]
mod tests;
