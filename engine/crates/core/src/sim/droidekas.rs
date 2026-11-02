use glam::Vec3;

use super::input::{buttons, PadState};
use super::state::{DeathCause, Event, Sim};
use crate::collision::Capsule;
use crate::droideka::{local_to_world, Body, DroidekaClass, Form, Input, Sphere};
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

    pub fn droideka_eye(&self) -> Option<Vec3> {
        let c = self.droideka_class()?;
        Some(self.soldier.pos + c.eye)
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
    /// The same edge/headroom inputs for real motion and between-tick look.
    fn droideka_input(&self, i: usize, pad: &PadState) -> Option<Input> {
        let u = self.units.get(i)?;
        let deka = u.droideka.as_ref()?;
        let class = u.droideka_class()?;
        let human = matches!(u.controller, Controller::Player { .. });
        let prev = deka.prev_buttons;
        let edge = |b: u32| pad.buttons & b != 0 && prev & b == 0;
        let walk_room = capsule_of(class, Form::Walk, u.soldier.pos);
        let cramped = deka.form() == Form::Ball
            && !self.collision.is_empty()
            && self
                .collision
                .push_out_soldier(&Capsule {
                    height: walk_room.height * UNROLL_HEADROOM,
                    ..walk_room
                })
                .length_squared()
                > 1e-6;
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
        let form = deka.form();
        if !self.collision.is_empty() {
            let push = self
                .collision
                .push_out_soldier_from(&from, &capsule_of(&class, form, body.pos));
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
            self.units[i].wall_push = push;
        }
        let rising = body.vel.y > 0.0;
        let (fy, n) = self.ground_under_n(body.pos, rising, from.base.y);
        let grounded = !rising && body.pos.y <= fy + FLOOR_SNAP;
        if grounded {
            body.pos.y = fy;
        }
        deka.land(&mut body, grounded, dt);
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
