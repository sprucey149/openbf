use glam::Vec3;

use crate::fmath;
use crate::vehicle::gun::wrap;

mod ball;
mod class;
mod shield;
mod walk;

pub use class::{
    default_clips, ClipInfo, DroidekaClass, BF1_WALK_NATURAL_SPEED, CAMERA_CROUCH, CAMERA_STAND,
    CAMERA_STAND_ZOOM,
};
pub use shield::{Shield, ShieldClass};

pub const IDLE: u8 = 0;
pub const TURN_LEFT: u8 = 1;
pub const TURN_RIGHT: u8 = 2;
pub const DYING: u8 = 3;
pub const DEAD: u8 = 4;
pub const IDLE_TO_LEFT: u8 = 5;
pub const IDLE_TO_RIGHT: u8 = 6;
pub const LEFT_TO_RIGHT: u8 = 7;
pub const RIGHT_TO_LEFT: u8 = 8;
pub const LEFT_TO_IDLE: u8 = 9;
pub const RIGHT_TO_IDLE: u8 = 10;
pub const IDLE_TO_ROLL: u8 = 11;
pub const ROLLING: u8 = 12;
pub const ROLL_TO_IDLE: u8 = 13;
pub const STATES: usize = 14;

pub const EV_STOP: u8 = 0;
pub const EV_STOP_BACK: u8 = 1;
pub const EV_FORWARD: u8 = 2;
pub const EV_BACK: u8 = 3;
pub const EV_TURN_LEFT: u8 = 4;
pub const EV_TURN_RIGHT: u8 = 5;
pub const EV_DIE: u8 = 6;
pub const EV_ROLL: u8 = 7;

pub const DIR_NONE: u8 = 0;
pub const DIR_FORWARD: u8 = 1;
pub const DIR_BACK: u8 = 2;

pub const TRANSITIONS: [[u8; 8]; STATES] = [
    [0, 0, 5, 9, 1, 2, 3, 11],
    [0, 0, 5, 9, 0, 0, 3, 11],
    [0, 0, 5, 10, 0, 0, 3, 11],
    [4, 4, 4, 4, 4, 4, 4, 11],
    [4, 4, 4, 4, 4, 4, 4, 11],
    [9, 0, 7, 10, 9, 9, 3, 11],
    [10, 0, 8, 9, 10, 10, 3, 11],
    [10, 5, 8, 8, 10, 10, 3, 11],
    [9, 7, 7, 7, 9, 9, 3, 11],
    [0, 5, 5, 8, 1, 2, 3, 11],
    [0, 7, 5, 7, 1, 2, 3, 11],
    [12, 12, 12, 12, 12, 12, 4, 12],
    [13, 13, 12, 12, 12, 12, 4, 12],
    [0, 0, 0, 0, 0, 0, 4, 0],
];

pub const CLIP_FRAMES: [u16; STATES] = [46, 9, 9, 81, 0, 17, 0, 17, 17, 24, 24, 31, 11, 101];

pub const CLIP_HZ: f32 = 30.0;
pub const GRAVITY: f32 = 9.8;
pub const THREAD_UPDATES: u32 = 2;
pub const BALL_TURN_DEADZONE: f32 = 0.1;
pub const BALL_TURN_MIN_FWD: f32 = 0.2;
pub const BALL_LEAN_RATE: f32 = 0.5;
pub const BALL_BOOST_STICK: f32 = 0.9;
pub const BALL_JUMP_WINDOW: f32 = 0.1;
pub const WALK_MOVE_DEADZONE_SQ: f32 = 0.01;
pub const WALK_BACK_HYSTERESIS: f32 = 0.3;
pub const WALK_LEG_STOP: f32 = 1.4;
pub const TURN_CLIP_ANGLE: f32 = std::f32::consts::FRAC_PI_6;
pub const TURN_RATE_SHARE: f32 = 0.51;
pub const LEG_UNITS: f32 = 6.0;
pub const ENERGY_RECOVER: f32 = crate::soldier::ENERGY_RECOVER_THRESHOLD;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Form {
    Walk,
    Ball,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cue {
    Fold,
    Unfold,
    ShieldOn,
    ShieldOff,
    Jump,
    /// A grounded ball jump was refused: the shared bar is spent (it ran
    /// empty and is not yet back at the recover share).
    EnergyRefused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Order {
    Deploy,
    #[default]
    Roll,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub fwd: f32,
    pub strafe: f32,
    pub turn: f32,
    pub fold: bool,
    pub shield: bool,
    pub jump: bool,
    /// The sprint button held: a ball's sprint, nothing on its feet.
    pub boost: bool,
    pub human: bool,
    /// The stamina cheat: a ball's sprint and jump cost the bar nothing.
    pub free_sprint: bool,
    pub cramped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub grounded: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Report {
    pub entered: Option<u8>,
    pub left: Option<u8>,
    pub weapons_off: bool,
    pub weapons_on: bool,
    pub shield: Option<bool>,
    pub jumped: bool,
    pub energy_refused: bool,
    pub water_damage: f32,
    pub walk_step: f32,
}

impl Report {
    pub fn cues(&self) -> impl Iterator<Item = Cue> + '_ {
        let fold = (self.entered == Some(IDLE_TO_ROLL)).then_some(Cue::Fold);
        let unfold = (self.entered == Some(ROLL_TO_IDLE)).then_some(Cue::Unfold);
        let shield = self
            .shield
            .map(|on| if on { Cue::ShieldOn } else { Cue::ShieldOff });
        let jump = self.jumped.then_some(Cue::Jump);
        fold.into_iter()
            .chain(unfold)
            .chain(shield)
            .chain(jump)
            .chain(self.energy_refused.then_some(Cue::EnergyRefused))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Droideka {
    pub state: u8,
    pub dir: u8,
    pub clip_t: f32,
    pub legs: f32,
    pub omega: f32,
    pub spin: [f32; 2],
    pub lean: f32,
    pub boosting: bool,
    /// The bar ran empty: no sprint and no ball jump until it is back over
    /// the recover share (the soldier's depleted latch). A player's only: a
    /// bot never sprints or jumps and never sets it.
    pub sprint_spent: bool,
    pub air: f32,
    pub water: f32,
    pub shield: Shield,
    pub order: Order,
    pub prev_buttons: u32,
    pub alive: bool,
    pub barrel: u8,
}

impl Droideka {
    pub fn new(c: &DroidekaClass, order: Order) -> Self {
        Self {
            state: IDLE,
            dir: DIR_NONE,
            clip_t: 0.0,
            legs: 0.0,
            omega: 0.0,
            spin: [1.0, 0.0],
            lean: 0.0,
            boosting: false,
            sprint_spent: false,
            air: 0.0,
            water: c.water_interval,
            shield: Shield::full(&c.shield),
            order,
            prev_buttons: 0,
            alive: true,
            barrel: 0,
        }
    }

    pub fn form(&self) -> Form {
        if self.state == ROLLING {
            Form::Ball
        } else {
            Form::Walk
        }
    }

    pub fn in_transform(&self) -> bool {
        matches!(self.state, IDLE_TO_ROLL | ROLLING | ROLL_TO_IDLE)
    }

    pub fn no_fire(&self) -> bool {
        self.in_transform() || !self.alive
    }

    pub fn engine_on(&self) -> bool {
        matches!(self.state, IDLE_TO_ROLL | ROLLING)
    }

    pub fn walk_state(&self) -> bool {
        self.state <= RIGHT_TO_IDLE && !matches!(self.state, DYING | DEAD)
    }

    pub fn raise(&mut self, ev: u8, c: &DroidekaClass, body: &mut Body, report: &mut Report) {
        let ev = match ev {
            EV_STOP if self.dir == DIR_BACK => EV_STOP_BACK,
            e => e,
        };
        match ev {
            EV_FORWARD => self.dir = DIR_FORWARD,
            EV_BACK => self.dir = DIR_BACK,
            _ => {}
        }
        if self.state == ROLLING {
            body.vel = Vec3::ZERO;
            self.legs = 0.0;
            self.lean = 0.0;
            // Leaving the ball ends its sprint: standing form never sprints.
            self.boosting = false;
        }
        let next = TRANSITIONS[usize::from(self.state)][usize::from(ev)];
        self.enter(next, c, report);
        if self.state == ROLLING {
            self.omega = 0.0;
            self.spin = [1.0, 0.0];
            self.legs = 0.0;
        }
    }

    fn enter(&mut self, next: u8, c: &DroidekaClass, report: &mut Report) {
        let prev = self.state;
        if next != DEAD {
            if next <= DEAD || next >= IDLE_TO_ROLL {
                self.dir = DIR_NONE;
            }
            if c.clips[usize::from(next)].frames > 0 {
                self.clip_t = if self.dir == DIR_BACK {
                    c.duration[usize::from(next)]
                } else {
                    0.0
                };
            }
            if prev != next {
                if next == IDLE_TO_ROLL {
                    report.weapons_off = true;
                } else if next == IDLE && prev == ROLL_TO_IDLE {
                    report.weapons_on = true;
                }
            }
        }
        self.state = next;
        if prev != next {
            report.left = Some(prev);
            report.entered = Some(next);
        }
    }

    pub fn turn_body(&mut self, body: &mut Body, delta: f32) {
        body.yaw -= delta;
        if self.state != ROLLING {
            self.legs = wrap(self.legs + delta);
        }
    }

    pub fn step(&mut self, dt: f32, input: &Input, c: &DroidekaClass, body: &mut Body) -> Report {
        let mut report = Report::default();
        if !self.alive {
            if !matches!(self.state, DEAD) {
                self.raise(EV_DIE, c, body, &mut report);
            }
            self.shield.on = false;
            return report;
        }
        if input.human && input.fold {
            if self.state == ROLLING {
                self.order = Order::Deploy;
            } else if self.walk_state() {
                self.order = Order::Roll;
            }
        }
        if self.state == ROLLING {
            if self.order == Order::Deploy && body.grounded && !input.cramped {
                self.raise(EV_STOP, c, body, &mut report);
            }
        } else if self.walk_state() && self.order == Order::Roll {
            self.raise(EV_ROLL, c, body, &mut report);
        }
        // Before the shield's recharge: a bar a hit emptied counts as empty.
        if input.human && self.shield.points <= 0.0 {
            self.sprint_spent = true;
        }
        if let Some(on) = self.shield.tick(
            dt,
            input.shield,
            self.in_transform(),
            !self.boosting,
            &c.shield,
        ) {
            report.shield = Some(on);
        }
        if self.state == ROLLING {
            self.ball(dt, input, c, body, &mut report);
        } else {
            self.walk(dt, input, c, body, &mut report);
        }
        self.tick_sprint(dt, input, c);
        let (sy, cy) = fmath::sin_cos(body.yaw);
        if self.state == ROLLING {
            let v = self.omega * c.ball_radius / c.ball_speed_scale;
            body.vel.x = -sy * v;
            body.vel.z = -cy * v;
            for _ in 0..THREAD_UPDATES {
                body.pos += body.vel * dt;
                body.vel.y -= GRAVITY * dt;
            }
        } else {
            for _ in 0..THREAD_UPDATES {
                body.pos.y += body.vel.y * dt;
                body.vel.y -= GRAVITY * dt;
            }
        }
        report
    }

    pub fn land(&mut self, body: &mut Body, grounded: bool, dt: f32) {
        body.grounded = grounded;
        if grounded {
            if body.vel.y < 0.0 {
                body.vel.y = 0.0;
            }
            self.air = 0.0;
        } else {
            self.air += dt;
        }
    }

    pub fn water_tick(&mut self, dt: f32, depth: Option<f32>, c: &DroidekaClass) -> f32 {
        let limit = if self.state == ROLLING {
            c.ball_water_height
        } else {
            c.walk_water_height
        };
        if depth.is_some_and(|d| d >= limit) {
            if self.water > 0.0 {
                self.water -= dt;
                0.0
            } else {
                c.water_amount
            }
        } else {
            self.water = c.water_interval;
            0.0
        }
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[
            0xde,
            self.state,
            self.dir,
            // One byte, so a unit that never sprints hashes as it always did.
            self.boosting as u8 | (self.sprint_spent as u8) << 1,
            self.shield.on as u8,
            self.order as u8,
            self.alive as u8,
            self.barrel,
        ]);
        for v in [
            self.clip_t,
            self.legs,
            self.omega,
            self.spin[0],
            self.spin[1],
            self.lean,
            self.air,
            self.water,
            self.shield.points,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&self.prev_buttons.to_le_bytes());
    }
}

pub fn local_to_world(pos: Vec3, yaw: f32, local: Vec3) -> Vec3 {
    let (sy, cy) = fmath::sin_cos(yaw);
    let right = Vec3::new(cy, 0.0, -sy);
    let fwd = Vec3::new(-sy, 0.0, -cy);
    pos + right * local.x + Vec3::Y * local.y + fwd * local.z
}

pub fn ray_sphere(from: Vec3, dir: Vec3, max_t: f32, s: Sphere) -> Option<f32> {
    if from.distance_squared(s.center) <= s.radius * s.radius {
        return None;
    }
    crate::unit::ray_sphere(from, dir, s.center, s.radius).filter(|&t| t <= max_t)
}

#[cfg(test)]
mod tests;
