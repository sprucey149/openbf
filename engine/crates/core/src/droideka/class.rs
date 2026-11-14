use crate::fmath::FloatExt;
use glam::Vec3;

use super::{
    Form, ShieldClass, Sphere, CLIP_FRAMES, CLIP_HZ, GRAVITY, IDLE_TO_LEFT, IDLE_TO_ROLL,
    LEFT_TO_IDLE, LEFT_TO_RIGHT, LEG_UNITS, RIGHT_TO_IDLE, RIGHT_TO_LEFT, ROLL_TO_IDLE, STATES,
    TURN_CLIP_ANGLE, TURN_LEFT, TURN_RATE_SHARE, TURN_RIGHT,
};
use crate::weapon::{floats, pf, pf_hash, PropSource, DEG};

const HASH_BALL_SPEED_SCALE: u32 = 0xdc52_4dd9;
const HASH_BALL_ZERO_STICK_TURN: u32 = 0x12a7_e73d;
const HASH_BALL_BOOST_SCALE: u32 = 0xbebb_6ed6;
const HASH_BALL_JUMP_FORWARD: u32 = 0x388d_3934;
const HASH_WALK_WATER_HEIGHT: u32 = 0xf01d_5583;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipInfo {
    pub frames: u16,
    pub root_travel: f32,
}

impl ClipInfo {
    pub fn duration(&self) -> f32 {
        if self.frames < 2 {
            0.0
        } else {
            f32::from(self.frames - 1) / CLIP_HZ
        }
    }

    pub fn natural_speed(&self) -> f32 {
        let d = self.duration();
        if d > 0.0 {
            self.root_travel / d
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DroidekaClass {
    pub max_speed: f32,
    pub max_turn_speed: f32,
    pub max_pitch_speed: f32,
    pub pitch_limits: (f32, f32),
    pub aimer_pitch_limits: (f32, f32),
    pub aimer_yaw_limits: (f32, f32),
    pub turn_threshold: f32,
    pub ball_radius: f32,
    pub ball_move_speed: f32,
    pub ball_turn_speed: f32,
    pub ball_zero_stick_turn: f32,
    pub ball_boost_turn: f32,
    pub ball_acceleration: f32,
    pub ball_friction: f32,
    pub ball_max_lean: f32,
    pub ball_speed_scale: f32,
    pub ball_boost_scale: f32,
    pub roll_time: f32,
    pub unroll_time: f32,
    pub jump_vy: f32,
    pub jump_forward: f32,
    pub energy_bar: f32,
    pub energy_recover_threshold: f32,
    pub energy_cost_jump: f32,
    /// `EnergyCostJumpSprint`, as the soldier reads it; no droideka authors
    /// it, so it is `EnergyCostJump`'s value.
    pub energy_cost_jump_sprint: f32,
    pub energy_drain_boost: f32,
    pub walk_water_height: f32,
    pub ball_water_height: f32,
    pub water_interval: f32,
    pub water_amount: f32,
    pub clips: [ClipInfo; STATES],
    pub rate: [f32; STATES],
    pub speed: [f32; STATES],
    pub duration: [f32; STATES],
    pub shield: ShieldClass,
    pub walk_body: Sphere,
    pub walk_soldier_radius: f32,
    pub walk_height: f32,
    pub walk_ordnance: Vec<Sphere>,
    pub guns: Vec<Vec3>,
    pub eye: Vec3,
    pub explosion: Option<crate::weapon::ExplosionClass>,
    pub cameras: Vec<(u32, crate::unit::CameraSection)>,
}

pub const CAMERA_STAND: u32 = 1_967_731_491;
pub const CAMERA_STAND_ZOOM: u32 = 2_497_170_924;
pub const CAMERA_CROUCH: u32 = 1_313_842_571;

pub const BF1_WALK_NATURAL_SPEED: f32 = 1.13;

pub fn default_clips() -> [ClipInfo; STATES] {
    let mut clips = [ClipInfo {
        frames: 0,
        root_travel: 0.0,
    }; STATES];
    for (s, c) in clips.iter_mut().enumerate() {
        c.frames = CLIP_FRAMES[s];
        if matches!(s as u8, LEFT_TO_RIGHT | RIGHT_TO_LEFT) {
            c.root_travel = BF1_WALK_NATURAL_SPEED * c.duration();
        }
    }
    clips
}

impl Default for DroidekaClass {
    fn default() -> Self {
        let mut c = Self {
            max_speed: 2.5,
            max_turn_speed: 1.25,
            max_pitch_speed: crate::soldier::SoldierConstants::default().max_pitch_speed,
            pitch_limits: (-25.0 * DEG, 25.0 * DEG),
            aimer_pitch_limits: (-25.0 * DEG, 25.0 * DEG),
            aimer_yaw_limits: (-40.0 * DEG, 40.0 * DEG),
            turn_threshold: 30.0 * DEG,
            ball_radius: 0.65,
            ball_move_speed: 6.0,
            ball_turn_speed: 2.0,
            ball_zero_stick_turn: 2.5,
            ball_boost_turn: 1.0,
            ball_acceleration: 100.0,
            ball_friction: 50.0,
            ball_max_lean: 10.0 * DEG,
            ball_speed_scale: 1.0,
            ball_boost_scale: 2.0,
            roll_time: 0.5,
            unroll_time: 1.66,
            jump_vy: (2.0 * GRAVITY * 1.27f32).sqrt(),
            jump_forward: 1.25,
            energy_bar: 100.0,
            energy_recover_threshold: 20.0,
            energy_cost_jump: 30.0,
            energy_cost_jump_sprint: 30.0,
            energy_drain_boost: 30.0,
            walk_water_height: 1.5,
            ball_water_height: 1.5,
            water_interval: 1.0,
            water_amount: 10.0,
            clips: default_clips(),
            rate: [1.0; STATES],
            speed: [0.0; STATES],
            duration: [0.0; STATES],
            shield: ShieldClass::default(),
            walk_body: Sphere {
                center: Vec3::new(0.0, 0.992, 0.421),
                radius: 1.0,
            },
            walk_soldier_radius: 1.0,
            walk_height: 2.0,
            walk_ordnance: Vec::new(),
            guns: Vec::new(),
            eye: Vec3::new(0.0, 1.8, 0.0),
            explosion: None,
            cameras: Vec::new(),
        };
        c.derive_rates();
        c
    }
}

impl DroidekaClass {
    pub fn from_props(src: &dyn PropSource, shield: Option<&dyn PropSource>) -> Self {
        let d = Self::default();
        let f = |key: &str, v: f32| pf(src, key).unwrap_or(v);
        let fh = |hash: u32, v: f32| pf_hash(src, hash).unwrap_or(v);
        let pitch_limits = src
            .prop("PitchLimits")
            .and_then(floats::<2>)
            .map_or(d.pitch_limits, |[lo, hi]| (lo * DEG, hi * DEG));
        let mut c = Self {
            max_speed: f("MaxSpeed", d.max_speed),
            max_turn_speed: f("MaxTurnSpeed", d.max_turn_speed),
            pitch_limits,
            aimer_pitch_limits: src
                .prop("AimerPitchLimits")
                .and_then(floats::<2>)
                .map_or(d.aimer_pitch_limits, |[lo, hi]| (lo * DEG, hi * DEG)),
            aimer_yaw_limits: src
                .prop("AimerYawLimits")
                .and_then(floats::<2>)
                .map_or(d.aimer_yaw_limits, |[lo, hi]| (lo * DEG, hi * DEG)),
            turn_threshold: pf(src, "TurnThreshold").map_or(d.turn_threshold, |t| t * DEG),
            ball_radius: f("BallRadius", d.ball_radius),
            ball_move_speed: f("BallMoveSpeed", d.ball_move_speed),
            ball_turn_speed: f("BallTurnSpeed", d.ball_turn_speed),
            ball_zero_stick_turn: fh(HASH_BALL_ZERO_STICK_TURN, d.ball_zero_stick_turn),
            ball_acceleration: f("BallAcceleration", d.ball_acceleration),
            ball_friction: f("BallRollingFriction", d.ball_friction),
            ball_max_lean: f("BallMaxLean", d.ball_max_lean / DEG) * DEG,
            ball_speed_scale: fh(HASH_BALL_SPEED_SCALE, d.ball_speed_scale),
            ball_boost_scale: fh(HASH_BALL_BOOST_SCALE, d.ball_boost_scale),
            roll_time: f("RollTime", d.roll_time),
            unroll_time: f("UnrollTime", d.unroll_time),
            jump_vy: pf(src, "BallJumpHeight").map_or(d.jump_vy, |h| (2.0 * GRAVITY * h).sqrt()),
            jump_forward: fh(HASH_BALL_JUMP_FORWARD, d.jump_forward),
            energy_bar: crate::soldier::SoldierConstants::from_props(src).energy_bar,
            energy_recover_threshold: crate::soldier::SoldierConstants::from_props(src)
                .energy_recover_threshold,
            energy_cost_jump: f("EnergyCostJump", d.energy_cost_jump),
            energy_cost_jump_sprint: pf(src, "EnergyCostJumpSprint")
                .unwrap_or_else(|| f("EnergyCostJump", d.energy_cost_jump)),
            energy_drain_boost: f("EnergyDrainSprint", d.energy_drain_boost),
            walk_water_height: fh(HASH_WALK_WATER_HEIGHT, d.walk_water_height),
            ball_water_height: f("BallWaterDamageHeight", d.ball_water_height),
            water_interval: f("WaterDamageInterval", d.water_interval),
            water_amount: f("WaterDamageAmount", d.water_amount),
            shield: shield.map_or(d.shield.clone(), ShieldClass::from_props),
            ..d
        };
        c.derive_rates();
        c
    }

    pub fn with_clips(mut self, clips: [ClipInfo; STATES]) -> Self {
        self.clips = clips;
        self.derive_rates();
        self
    }

    pub fn derive_rates(&mut self) {
        for s in 0..STATES {
            self.rate[s] = 1.0;
            self.duration[s] = self.clips[s].duration();
            self.speed[s] = self.clips[s].natural_speed();
        }
        for s in [TURN_LEFT, TURN_RIGHT] {
            let d = self.duration[usize::from(s)];
            let sign = if s == TURN_LEFT { 1.0 } else { -1.0 };
            self.speed[usize::from(s)] = if d > 0.0 {
                sign * TURN_CLIP_ANGLE / (d * LEG_UNITS)
            } else {
                0.0
            };
        }
        let walk = self.speed[usize::from(LEFT_TO_RIGHT)];
        if walk > 0.0 {
            for s in [
                IDLE_TO_LEFT,
                LEFT_TO_RIGHT,
                RIGHT_TO_LEFT,
                LEFT_TO_IDLE,
                RIGHT_TO_IDLE,
            ] {
                self.rate[usize::from(s)] = self.max_speed / walk;
            }
        }
        let turn = self.speed[usize::from(TURN_LEFT)];
        if turn > 0.0 {
            let r = self.max_turn_speed * TURN_RATE_SHARE / turn;
            self.rate[usize::from(TURN_LEFT)] = r;
            self.rate[usize::from(TURN_RIGHT)] = r;
        }
        if self.roll_time > 0.0 {
            self.rate[usize::from(IDLE_TO_ROLL)] =
                self.duration[usize::from(IDLE_TO_ROLL)] / self.roll_time;
        }
        if self.unroll_time > 0.0 {
            self.rate[usize::from(ROLL_TO_IDLE)] =
                self.duration[usize::from(ROLL_TO_IDLE)] / self.unroll_time;
        }
    }

    pub fn state_time(&self, s: u8) -> f32 {
        let s = usize::from(s);
        if self.rate[s] > 0.0 {
            self.duration[s] / self.rate[s]
        } else {
            0.0
        }
    }

    pub fn camera_section(&self, form: Form, zoomed: bool) -> Option<crate::unit::CameraSection> {
        let want = match (form, zoomed) {
            (Form::Ball, _) => CAMERA_CROUCH,
            (Form::Walk, true) => CAMERA_STAND_ZOOM,
            (Form::Walk, false) => CAMERA_STAND,
        };
        self.cameras
            .iter()
            .find(|(name, _)| *name == want)
            .map(|(_, s)| *s)
    }

    pub fn ball_omega_cap(&self) -> f32 {
        self.ball_move_speed * self.ball_speed_scale / self.ball_radius
    }

    /// Points of the shared bar a sprinting ball spends a second. The share of
    /// the bar is the class's own, `EnergyDrainSprint` / `EnergyBar`; spending it
    /// from the shield's bar is OURS (the maintainer, 2026-11-01). BF2: 900/s, a
    /// full bar in 3.3 s. PROVISIONAL, measure: the maintainer's timed full-bar
    /// ball sprint in live play.
    pub fn ball_sprint_drain(&self) -> f32 {
        self.share_of_bar(self.energy_drain_boost)
    }

    /// Points of the shared bar a ball jump spends: the soldier's jump rule
    /// (`crate::soldier::Soldier::pay_jump`) at the class's own share of the
    /// bar, `EnergyCostJump` / `EnergyBar`, the dearer `EnergyCostJumpSprint`
    /// when sprinting (the ball's sprint stands for the soldier's "over
    /// MaxSpeed"). BF2: 900 points, 30 % of a full bar. Spending it from the
    /// shield's bar is OURS (the maintainer, 2026-11-02). PROVISIONAL, measure:
    /// The maintainer's count of ball jumps on a full bar in live play.
    pub fn ball_jump_cost(&self, sprinting: bool) -> f32 {
        let cost = if sprinting {
            self.energy_cost_jump.fmax(self.energy_cost_jump_sprint)
        } else {
            self.energy_cost_jump
        };
        self.share_of_bar(cost)
    }

    /// The bar a spent ball needs before it sprints or jumps again: the soldier's recover
    /// threshold as a share (BF2: 600 points, 2 s). OURS and PROVISIONAL,
    /// measure: the maintainer's timed wait from an empty bar to the next sprint
    /// in live play.
    pub fn ball_sprint_recover(&self) -> f32 {
        self.share_of_bar(self.energy_recover_threshold)
    }

    pub(super) fn ball_energy_floor(&self) -> f32 {
        -self.share_of_bar(
            self.energy_cost_jump
                .fmax(self.energy_cost_jump_sprint)
                .fmax(0.0),
        )
    }

    fn share_of_bar(&self, energy: f32) -> f32 {
        if self.energy_bar > 0.0 {
            self.shield.max * energy / self.energy_bar
        } else {
            0.0
        }
    }

    pub fn ball_sphere(&self) -> Sphere {
        Sphere {
            center: Vec3::new(0.0, self.ball_radius + 0.1, -0.2),
            radius: self.ball_radius,
        }
    }

    pub fn gun(&self, barrel: u8) -> Vec3 {
        if self.guns.is_empty() {
            self.shield.sphere.center
        } else {
            self.guns[usize::from(barrel) % self.guns.len()]
        }
    }

    pub fn ordnance_spheres(&self, form: Form) -> Vec<Sphere> {
        match form {
            Form::Ball => vec![self.ball_sphere()],
            Form::Walk if self.walk_ordnance.is_empty() => vec![self.walk_body],
            Form::Walk => self.walk_ordnance.clone(),
        }
    }
}
