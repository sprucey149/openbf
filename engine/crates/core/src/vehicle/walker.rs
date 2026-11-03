//! The walker integrator (card WALK): BF2's walker, transcribed from
//! `analysis/re-walker.md` [O] — the ONE integrator of every `walker` and
//! `commandwalker` class on both discs (PLAN locks BF2 ground handling; a
//! BF1-disc class runs BF2's clock and turn tree, card WALKB1).
//!
//! Per 1/30 tick, in the PS2's task order (re-walker §2): the controller
//! fills `Vehicle::input` → [`step`]: the input (§3 I7-I16: the aim rates,
//! the forced throttle on a slope) around the locomotion (§4: the stick
//! classes, speed / boost / energy, the aim window and the camera-relative
//! turn, the clip state machine and its clock, the root motion), then the
//! body task (§8: the terrain wall probes, the legs §6), then the building
//! contacts (§9 C7).
//!
//! **The walker moves by its clips.** The body goes where the walk clip's
//! root goes: each tick the clip advances by dt' = dt·max(0.5,
//! speed·|stick| / (the walk clip's speed · SkeletonRootScale)) (§4.8
//! branch B), and the root's z displacement moves the body along the LEGS'
//! forward (§4.9, FUN_00260918). An AT-ST at full stick plays its walk at
//! ×0.735 and covers 5.0 m/s. The root curve is sampled interpolated — the
//! drawn walker's case (§5); an undrawn PS2 walker steps whole keyframes at
//! the same average speed.
//!
//! Frames. `Vehicle::rot` is E (rows right, up, fwd = the model's +x, +y,
//! +z in the world) and stays upright: nothing here tilts it (§10). The
//! body turns by 2θ for a half-angle command θ (FUN_0025ed70's (c²−s², 2cs)):
//! θ > 0 moves fwd toward −right. The legs' offset h is a unit complex; the
//! legs (and the drawn model, bone 0) trail the body by 2h — [`legs_rot`].
//! The aim yaw is body-relative, + toward +x (the model's left, as a seat
//! gun's yaw, `gun.rs`), and keeps its world direction as the body turns.
//!
//! Not built (each unused by the shipped data or a later card): the jump
//! (§7: no walker on either disc authors JumpHeight, so §4.8 branch A is
//! dead too); the custom stances and anims (K+0x675 = 0 on every class
//! seen); death (a destroyed vehicle leaves no wreck, re-vehicles §6); the
//! water damage (§3 I3); the stomp (C4) and vehicle contacts (C6); the AI's
//! boost mode (its steering is `ai::walker_drive`, card WAI). The leftup/rightup leg blend (§6 P+0x120) is kept for the
//! drawn pose (card WSKN, `game::viewer::walker_pose`). Sim also hashes it
//! when BF2 hero queries use animated rider frames.
//!
//! PROVISIONAL (re-walker §14.5 and ours): where the contact pass sits in
//! the frame (last, as the hover's); C7's horizontal push-out distance; the
//! terrain rays — the heightfield sheet (`collision::ray_heightfield`) for
//! FUN_0055fb38 and the heightfield plus the SOLDIER-layer bodies for the leg
//! rays' mask 0x90; the foot and building spheres at the bind pose (only
//! the drawn model is posed, WSKN); the two-pair probe offset from pair 0.

use super::hover::{quat_y, Contact};
use super::Vehicle;
use crate::fmath;
use glam::{Mat3, Vec3};

/// The anim table's fixed entries (FUN_002663e0; re-walker §5).
pub const ANIM_COUNT: usize = 11;
pub const ANIM_IDLE: u8 = 0;
pub const ANIM_TURNL: u8 = 1;
pub const ANIM_TURNR: u8 = 2;
/// death01, reached by the death action in the walker transition table.
pub const ANIM_DEATH: u8 = 3;
/// The empty entry after death01 (FUN_0025ea48 only records it).
pub const ANIM_NONE: u8 = 4;
/// walk_leftfoot_rightfoot: its speed is the clock's W (§4.8).
pub const ANIM_WALK: usize = 7;

/// The locomotion's actions (FUN_0025ec40).
const ACT_STOP: usize = 0;
const ACT_STOP_BACK: usize = 1;
const ACT_FWD: usize = 2;
const ACT_BACK: usize = 3;
const ACT_TURNL: usize = 4;
const ACT_TURNR: usize = 5;

/// `DAT_005fd100[row = current anim][action]`, 11 × 7 words read from the
/// ELF (re_check word b2 0x5fd100 77; re-walker §5). Action 6 (die) is 3.
pub const TRANSITIONS: [[u8; 7]; ANIM_COUNT] = [
    [0, 0, 5, 9, 1, 2, 3],
    [0, 0, 5, 9, 0, 0, 3],
    [0, 0, 5, 10, 0, 0, 3],
    [4, 4, 4, 4, 4, 4, 4],
    [4, 4, 4, 4, 4, 4, 4],
    [9, 0, 7, 10, 9, 9, 3],
    [10, 0, 8, 9, 10, 10, 3],
    [10, 5, 8, 8, 10, 10, 3],
    [9, 6, 7, 7, 9, 9, 3],
    [0, 5, 5, 8, 1, 2, 3],
    [0, 6, 5, 7, 1, 2, 3],
];

/// K+0xe18's bits (the setter's six flag hashes, §11; ctor bits 1-3).
pub mod flags {
    /// 0x55bc9af9: read by the AI (B2:181581).
    pub const AI: u32 = 1 << 0;
    /// 0xe57fbb1d: the aim window pulls the body (§4.5 A).
    pub const AIM_WINDOW: u32 = 1 << 1;
    /// 0xba804df3: skip the camera-relative turn (→ §4.5 C).
    pub const NO_CAMERA_TURN: u32 = 1 << 2;
    /// 0x8ebd30be: the camera-relative turn (§4.5 B, §4.1's rewrite).
    pub const CAMERA_TURN: u32 = 1 << 3;
    /// 0x3f164e6b: a human backing up steers like a car (§4.3).
    pub const REVERSE_STEER: u32 = 1 << 4;
    /// 0x7fdeedf1: damage buildings (C7; not built).
    pub const DAMAGE_BUILDINGS: u32 = 1 << 6;
    /// The ctor's (FUN_00263160).
    pub const CTOR: u32 = AIM_WINDOW | NO_CAMERA_TURN | CAMERA_TURN;
}

/// The stick-class and clock constants (B2 .data, re_check word b2 0x5fd238).
/// DAT_005fd238: a custom stance counts after this long; FUN_0025eef8's
/// threshold from a standstill with the stick pushed.
pub const STEP_THRESHOLD_MOVING: f32 = 0.1;
/// DAT_005fd240: the walk clock's floor.
pub const CLOCK_FLOOR: f32 = 0.5;
/// §4.2: the boost needs p2² + p3² past this.
pub const BOOST_STICK2: f32 = 0.81;
/// §4.5 B: the camera-relative command's gain.
pub const CAMERA_GAIN: f32 = 0.95;
/// §4.4: the look stick narrows the far limit by this much.
pub const WINDOW_NARROW: f32 = 0.8;
/// §6: the legs' gravity, 3 g (B2:180461-180464).
pub const LEG_GRAVITY: f32 = 29.4;
/// §6: a one-pair walker's origin may sit this far under the foot ground.
pub const LIFT_SLACK: f32 = 0.1;
/// §6: the body pitch and roll clamp, a half angle (0x3e974e6d).
pub const PITCH_CLAMP: f32 = 0.295_520_22;
/// §6 R's clamp's cosine (0.9553365 = cos 0.3).
pub const ROLL_CLAMP_COS: f32 = 0.955_336_5;
/// §6: DAT_005fdf5c, the leg ray's height spread → support blend.
pub const SUPPORT_BLEND: f32 = 0.4;
/// §6 FUN_00260ba0: the body pitch step per metre of pair height difference.
pub const PITCH_STEP: f32 = 0.01;
/// §6: the roll relaxation's rate.
pub const ROLL_RATE: f32 = 0.05;
/// §4.9 FUN_0025ecd8: the legs' offset decays by this a tick while walking.
pub const LEG_DECAY: f32 = 0.1;
/// §8: the wall probes' origin height and reach, × LegRayHitLength.
pub const PROBE_UP: f32 = 0.85;
pub const PROBE_FWD: f32 = 0.8;
pub const PROBE_SIDE: f32 = 0.7;
/// §8: two or more pairs reach this much farther.
pub const PROBE_LONG: f32 = 1.3;
/// §9 C7: a foot on a building face this steep or flatter is supported.
pub const FOOT_MIN_NY: f32 = 0.2;
/// §9: n.y past this sets the supported bit (cos 45°).
pub const SUPPORT_NY: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// The energy pool's floor and its recover threshold (FUN_001e1c88's
/// defaults; K+0x798 / K+0x79c, no walker setter writes them).
pub const ENERGY_MIN: f32 = 0.0;
pub const ENERGY_RECOVER: f32 = 20.0;
/// §6 (FUN_002a2028): the leftup/rightup blend's target gain on the pair's
/// height difference (DAT_005fdf5c) and its relaxation per tick.
pub const LAYER_GAIN: f32 = 0.6;
pub const LAYER_RATE: f32 = 0.4;
/// The leg pairs whose layer blend is kept (every shipped walker has one or
/// two).
pub const MAX_PAIRS: usize = 4;

/// One anim-table entry's clip (FUN_002a0c78): duration (N − 1)/30 and
/// track 0's root z at each frame, before SkeletonRootScale.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub duration: f32,
    pub looping: bool,
    pub root_z: Vec<f32>,
}

impl Clip {
    /// The root z at time `t`, interpolated between keyframes (the drawn
    /// walker's sampling, §5): frame (N − 1)·t/duration.
    pub fn z_at(&self, t: f32) -> f32 {
        let n = self.root_z.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 || self.duration <= 0.0 {
            return self.root_z[0];
        }
        let x = ((n - 1) as f32 * (t / self.duration)).clamp(0.0, (n - 1) as f32);
        let i = (x as usize).min(n - 2);
        let f = x - i as f32;
        self.root_z[i] + (self.root_z[i + 1] - self.root_z[i]) * f
    }

    /// The entry's speed: the root's z at the last frame over the duration.
    pub fn speed(&self) -> f32 {
        match self.root_z.last() {
            Some(&z) if self.duration > 0.0 => z / self.duration,
            _ => 0.0,
        }
    }
}

/// One `WalkerLegPair` (FUN_002a1468): its two foot prims' model-space
/// positions (left, right), and P[0], half their summed z (FUN_002a1750).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegPair {
    pub feet: [Vec3; 2],
}

impl LegPair {
    pub fn offset(&self) -> f32 {
        (self.feet[0].z + self.feet[1].z) * 0.5
    }
}

/// A walker primitive that meets buildings (`BuildingCollision`, §9 C7), in
/// the model's frame; `foot` = one of the leg pairs' prims or on their bone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildingSphere {
    pub pos: Vec3,
    pub radius: f32,
    pub foot: bool,
}

/// A walker class's numbers (`game::vehicle_class::WalkerClass`: the ctor
/// defaults FUN_00263160, then the ODF through the setter's conversions,
/// §11). Angles in radians; the turn speeds are HALF-angle rates.
#[derive(Clone, Debug, PartialEq)]
pub struct WalkerParams {
    pub max_speed: f32,
    pub boost_speed: f32,
    pub stopped_turn_speed: f32,
    pub forward_turn_speed: f32,
    pub boost_turn_speed: f32,
    /// K+0x75c MaxTurnSpeed: the rate the AI's body-turn stick is scaled by
    /// (card WAI, re-walker-ai §2; `ai::walker_drive`). No locomotion reader.
    pub max_turn_speed: f32,
    /// K+0x784: sin(TurnThreshold/2) as the setter stores it (ctor 0.3 raw).
    pub turn_threshold: f32,
    pub yaw_limits: [f32; 2],
    pub pitch_limits: [f32; 2],
    pub max_yaw_speed: f32,
    pub max_pitch_speed: f32,
    /// K+0xe08 / +0xe0c / +0xe10 / +0xe14 (tangents; §11).
    pub e08: f32,
    pub e0c: f32,
    pub e10: f32,
    pub e14: f32,
    /// K+0xe18 ([`flags`]).
    pub flags: u32,
    /// EnergyBar (K+0x794) and the rates a second.
    pub energy_max: f32,
    pub energy_auto_restore: f32,
    pub energy_boost_drain: f32,
    pub root_scale: f32,
    pub leg_ray: f32,
    /// K+0x78c, the roll flag, and WalkerWidth.
    pub roll: bool,
    pub width: f32,
    pub leg_pairs: Vec<LegPair>,
    /// The anim table's 11 fixed entries; `None` where the bank lacks one.
    pub clips: Vec<Option<Clip>>,
    pub building: Vec<BuildingSphere>,
    /// A `commandwalker`'s post (card CWSP, `analysis/re-command-walker-post.md`
    /// §4 [O]): its class's `SpawnPointLocation`s as (position in the
    /// walker's frame, the turn about up in radians), in PROP order. Empty
    /// for a plain walker — it owns no post ([`Sim::spawn_points`]).
    ///
    /// [`Sim::spawn_points`]: crate::sim::Sim::spawn_points
    pub spawn_points: Vec<(Vec3, f32)>,
}

impl Default for WalkerParams {
    /// FUN_00263160's defaults (re-walker §11).
    fn default() -> Self {
        Self {
            max_speed: 20.0,
            boost_speed: 0.0,
            stopped_turn_speed: 0.1,
            forward_turn_speed: 0.1,
            boost_turn_speed: 0.01,
            max_turn_speed: 2.0,
            turn_threshold: 0.3,
            yaw_limits: [0.0, 0.0],
            pitch_limits: [-0.5, 0.5],
            max_yaw_speed: 2.0,
            max_pitch_speed: 0.5,
            e08: 0.0,
            e0c: 0.0,
            e10: 5.0,
            e14: 0.0,
            flags: flags::CTOR,
            energy_max: 100.0,
            energy_auto_restore: 0.0,
            energy_boost_drain: 1.0,
            root_scale: 1.0,
            leg_ray: 2.5,
            roll: false,
            width: 1.0,
            leg_pairs: Vec::new(),
            clips: Vec::new(),
            building: Vec::new(),
            spawn_points: Vec::new(),
        }
    }
}

impl WalkerParams {
    fn has(&self, bit: u32) -> bool {
        self.flags & bit != 0
    }

    fn clip(&self, anim: u8) -> Option<&Clip> {
        self.clips.get(usize::from(anim)).and_then(Option::as_ref)
    }

    fn duration(&self, anim: u8) -> f32 {
        self.clip(anim).map_or(0.0, |c| c.duration)
    }
}

/// The walker's own entity fields (re-walker §1).
#[derive(Clone, Debug, PartialEq)]
pub struct WalkerState {
    /// E+0x50c / +0x510: the aim, body-relative.
    pub aim_yaw: f32,
    pub aim_pitch: f32,
    /// E+0x514: the body pitch, a half angle (two or more pairs only).
    pub pitch: f32,
    /// E+0x2210 / +0x2214: the legs' offset as (cos h, sin h).
    pub h: (f32, f32),
    /// E+0x2220 / +0x2224: the body roll as (cos, sin).
    pub roll: (f32, f32),
    /// E+0x21e8: the anim-table entry playing.
    pub anim: u8,
    /// E+0x2200: 0 none, 1 forward, 2 backward.
    pub dir: u8,
    /// The clip player: time into the entry, and playing reversed.
    pub clip_time: f32,
    pub reversed: bool,
    /// E+0x2188: the turn-step clip's leg-offset target.
    pub turn_step: f32,
    /// E+0x2190 bit 0x40, set by the legs and contacts, cleared by the
    /// locomotion's tail.
    pub supported: bool,
    /// E+0x21e4: time since last supported.
    pub since_supported: f32,
    /// The energy pool (E+0x2204: [1] value, [2] bit 0 exhausted).
    pub energy: f32,
    pub exhausted: bool,
    /// E+0x2270 bit 32: the boost latch.
    pub boosting: bool,
    /// Each leg pair's P+0x120 (§6): its `_leftup` (< 0) / `_rightup` (> 0)
    /// layer weight. Sim adds these bytes when hero rider collision frames
    /// are installed; ordinary walker hashes retain their existing layout.
    pub layer_blend: [f32; MAX_PAIRS],
}

impl Default for WalkerState {
    fn default() -> Self {
        Self {
            aim_yaw: 0.0,
            aim_pitch: 0.0,
            pitch: 0.0,
            h: (1.0, 0.0),
            roll: (1.0, 0.0),
            anim: ANIM_IDLE,
            dir: 0,
            clip_time: 0.0,
            reversed: false,
            turn_step: 0.0,
            supported: false,
            since_supported: 0.0,
            energy: 100.0,
            exhausted: false,
            boosting: false,
            layer_blend: [0.0; MAX_PAIRS],
        }
    }
}

impl WalkerState {
    /// A fresh walker of class `p`: full energy (the ctor copies EnergyBar).
    pub fn new(p: &WalkerParams) -> Self {
        Self {
            energy: p.energy_max,
            ..Self::default()
        }
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for x in [
            self.aim_yaw,
            self.aim_pitch,
            self.pitch,
            self.h.0,
            self.h.1,
            self.roll.0,
            self.roll.1,
            self.clip_time,
            self.turn_step,
            self.since_supported,
            self.energy,
        ] {
            eat(&x.to_bits().to_le_bytes());
        }
        eat(&[
            self.anim,
            self.dir,
            self.reversed as u8,
            self.supported as u8,
            self.exhausted as u8,
            self.boosting as u8,
        ]);
    }

    /// The legs' angle 2h (full angle).
    pub fn leg_angle(&self) -> f32 {
        2.0 * fmath::atan2(self.h.1, self.h.0)
    }

    /// FUN_001e2010: false only while exhausted; the drain that takes the
    /// pool below 0 still succeeds, exhausts it and floors it.
    fn drain(&mut self, x: f32) -> bool {
        if x <= 0.0 {
            return true;
        }
        if self.exhausted {
            return false;
        }
        self.energy -= x;
        if self.energy < 0.0 {
            self.exhausted = true;
            self.energy = self.energy.max(ENERGY_MIN);
        }
        true
    }

    /// FUN_001e1fc0: refill; at the recover threshold the pool works again,
    /// capped at the class's EnergyBar.
    fn refill(&mut self, x: f32, max: f32) {
        self.energy += x;
        if ENERGY_RECOVER <= self.energy {
            self.exhausted = false;
            self.energy = self.energy.min(max);
        }
    }

    /// h ← h·(cos φ + i sin φ), renormalised.
    fn turn_legs(&mut self, phi: f32) {
        let (s, c) = fmath::sin_cos(phi);
        let (hc, hs) = self.h;
        self.h = unit(hc * c - hs * s, hc * s + hs * c);
    }
}

fn unit(c: f32, s: f32) -> (f32, f32) {
    let l = (c * c + s * s).sqrt();
    if l > 0.0 {
        (c / l, s / l)
    } else {
        (1.0, 0.0)
    }
}

/// The world a walker's rays and spheres meet.
pub trait WalkerWorld {
    /// Is a terrain loaded (DAT_0060bed4, §8)?
    fn has_terrain(&self) -> bool;
    /// The leg ray (FUN_00161c60, mask 0x90): the distance straight down
    /// from `origin` to the first terrain or building surface within `len`.
    fn ground_below(&self, origin: Vec3, len: f32) -> Option<f32>;
    /// The terrain ray FUN_0055fb38: the fraction of `from` → `to` at which
    /// the segment meets the terrain, or `None`.
    fn terrain_segment(&self, from: Vec3, to: Vec3) -> Option<f32>;
    /// Every building contact of a sphere (class 0x10, §9 C7).
    fn building_contacts(&self, centre: Vec3, radius: f32, out: &mut Vec<Contact>);
}

/// The drawn model's rotation: the body turned by the legs' 2h — FUN_0025f050
/// yaws bone 0 by 2h, so the whole rigid model turns with the legs (§10).
pub fn legs_rot(v: &Vehicle) -> glam::Quat {
    (v.rot * quat_y(v.walker.leg_angle())).normalize()
}

/// One tick of one walker (§2's order): input and locomotion, the aim, the
/// fall, the body task (probes, legs), then the building contacts.
pub fn step(v: &mut Vehicle, p: &WalkerParams, world: &impl WalkerWorld, dt: f32) {
    input(v, p, dt);
    if world.has_terrain() {
        probes(v, p, world);
    }
    legs(v, p, world, dt);
    contacts(v, p, world);
}

/// FUN_0025f4a0's I7-I16 around the locomotion (§3). Offline, no mouse.
pub fn input(v: &mut Vehicle, p: &WalkerParams, dt: f32) {
    let inp = v.input;
    // I7: the aim rates.
    let pitch_rate = p.max_pitch_speed;
    let mut yaw_rate = if inp.human {
        p.max_pitch_speed + (p.max_yaw_speed - p.max_pitch_speed) * v.hover.stick_timer
    } else {
        p.max_yaw_speed
    };
    // I8
    v.walker.aim_pitch += inp.pitch * pitch_rate * dt;
    // I10: the body pitch forces the throttle downhill.
    let a = v.walker.pitch;
    let fwd = if a < -0.25 {
        -1.0
    } else if a < -0.18 {
        inp.throttle.min(0.0)
    } else if a > 0.25 {
        1.0
    } else if a > 0.18 {
        inp.throttle.max(0.0)
    } else {
        inp.throttle
    };
    // I11
    let turn_speed = locomotion(v, p, dt, fwd, inp.strafe, inp.turn);
    // I13: the yaw rate eases toward the turn speed near the limits.
    let w = &mut v.walker;
    let hh = w.leg_angle();
    let (max_, min_) = (p.yaw_limits[1] + hh, p.yaw_limits[0] + hh);
    let f_max = if inp.turn >= 0.0 || max_ <= 0.0 {
        1.0
    } else {
        ((max_ - w.aim_yaw).max(0.0) / max_).clamp(0.0, 1.0)
    };
    let f_min = if inp.turn <= 0.0 || min_ >= 0.0 {
        1.0
    } else {
        ((w.aim_yaw - min_).max(0.0) / -min_).clamp(0.0, 1.0)
    };
    let f = f_min.min(f_max);
    if turn_speed < yaw_rate {
        yaw_rate = f * yaw_rate + (1.0 - f) * turn_speed;
    }
    // I14, I15
    w.aim_yaw -= inp.turn * yaw_rate * dt;
    w.aim_pitch = w.aim_pitch.clamp(p.pitch_limits[0], p.pitch_limits[1]);
    // I16: the fall the legs asked for last tick.
    if v.vel.y < 0.0 {
        v.pos.y += v.vel.y * dt;
    }
}

/// FUN_0025d5e0 (§4) with p2 = fwd, p3 = strafe, p4 = turn. Returns the
/// turn speed it leaves (I13 reads it).
pub fn locomotion(
    v: &mut Vehicle,
    p: &WalkerParams,
    dt: f32,
    mut p2: f32,
    mut p3: f32,
    p4: f32,
) -> f32 {
    let human = v.input.human;
    // 4.1
    let mut in_place = p.e10 * p2.abs() <= p3.abs() || p2 * p2 + p3 * p3 <= 0.01;
    let mut walking = !in_place;
    if human
        && !walking
        && !p.has(flags::NO_CAMERA_TURN)
        && p.has(flags::CAMERA_TURN)
        && p3.abs() > 0.1
    {
        p2 = 1.0 / (p.e10 * p.e10 + 1.0).sqrt();
        if v.walker.dir == 2 {
            p2 = -p2;
        }
        in_place = false;
        walking = true;
    }
    // 4.2
    let w = &mut v.walker;
    let mut turn_speed = p.stopped_turn_speed;
    let mut speed = p.max_speed;
    if w.supported {
        w.since_supported = 0.0;
    } else {
        w.since_supported += dt;
    }
    let want_boost = p.max_speed < p.boost_speed
        && w.dir == 1
        && p2 > 0.0
        && p2 * p2 + p3 * p3 > BOOST_STICK2
        && human
        && (v.input.boost || w.boosting);
    if want_boost && w.drain(p.energy_boost_drain * dt) {
        speed = p.boost_speed;
        turn_speed = p.boost_turn_speed;
        w.boosting = true;
    } else {
        w.boosting = false;
        w.refill(p.energy_auto_restore * dt, p.energy_max);
        if walking {
            turn_speed = p.forward_turn_speed;
        }
    }
    // 4.3
    if human && p.has(flags::REVERSE_STEER) && !in_place && p2 < 0.0 {
        p3 = -p3;
    }
    // 4.4
    let hh = w.leg_angle();
    let (lo, hi) = (p.yaw_limits[0], p.yaw_limits[1]);
    let (min_, max_) = if p4 <= 0.0 {
        (lo + hh, hi * (1.0 + WINDOW_NARROW * p4) + hh)
    } else {
        (lo * (1.0 - WINDOW_NARROW * p4) + hh, hi + hh)
    };
    let step = turn_speed * dt;
    // 4.5 A-C
    let aim = w.aim_yaw;
    let c_branch = |walking: bool| {
        let mut cmd = p3 * step;
        if walking && p3.abs() < p2.abs() * p.e0c {
            cmd = cmd.clamp(-step, step) * (p3.abs() / p2.abs() - p.e08) / (p.e0c - p.e08);
        }
        if p.e14 * p3.abs() > 0.0 {
            cmd -= p.e14 * (1.0 - p3.abs()) * aim;
        }
        cmd
    };
    let toward_side = || CAMERA_GAIN * (fmath::atan2(p3 * p.e08, p3.abs()) - aim);
    let cmd = if human && p.has(flags::AIM_WINDOW) && aim < min_ {
        2.0 * (min_ - aim)
    } else if human && p.has(flags::AIM_WINDOW) && aim > max_ {
        2.0 * (max_ - aim)
    } else if human && p.has(flags::CAMERA_TURN) {
        if walking {
            let inside =
                (aim <= 0.0 && -p3 < p2.abs() * p.e08) || (aim >= 0.0 && p3 < p2.abs() * p.e08);
            if inside {
                CAMERA_GAIN * (fmath::atan2(p3, p2.abs()) - aim)
            } else if p.has(flags::NO_CAMERA_TURN) {
                c_branch(walking)
            } else {
                toward_side()
            }
        } else if p.has(flags::NO_CAMERA_TURN) {
            c_branch(walking)
        } else if p3.abs() > 0.1 {
            toward_side()
        } else {
            0.0
        }
    } else {
        c_branch(walking)
    };
    // D: a far command stops the walk to turn first.
    let d_gate = match w.dir {
        0 => true,
        1 => p2 < 0.5,
        2 => p2 > -0.5,
        _ => false,
    };
    if !p.has(flags::NO_CAMERA_TURN) && p4 == 0.0 && d_gate && (cmd <= 0.5 * lo || cmd >= 0.5 * hi)
    {
        walking = false;
    }
    // E-G
    let theta = if cmd == 0.0 {
        0.0
    } else {
        cmd.clamp(-step, step)
    };
    if human && p.has(flags::CAMERA_TURN) {
        w.aim_yaw += 2.0 * theta;
    }
    if theta != 0.0 {
        v.rot = (v.rot * quat_y(-2.0 * theta)).normalize();
        v.walker.turn_legs(theta);
    }

    // 4.7: a new clip only from idle.
    if v.walker.anim == ANIM_IDLE {
        if !walking {
            turn_step(v, p, p2, p3);
        } else {
            start_action(v, p, if p2 < 0.0 { ACT_BACK } else { ACT_FWD });
        }
    }

    // 4.8 the clip dt.
    let mut dt_c = dt;
    if dt > 0.0 {
        let w = &mut v.walker;
        if walking && w.dir != 0 {
            if (w.dir == 1 && p2 < 0.0) || (w.dir == 2 && p2 > 0.0) {
                w.reversed = !w.reversed;
                w.dir = 3 - w.dir;
            }
            let wspeed = p
                .clips
                .get(ANIM_WALK)
                .and_then(Option::as_ref)
                .map_or(0.0, Clip::speed)
                * p.root_scale;
            let ss = speed * (p2 * p2 + p3 * p3).sqrt();
            dt_c = if CLOCK_FLOOR * wspeed < ss {
                dt * ss / wspeed
            } else {
                CLOCK_FLOOR * dt
            };
        }
        advance(v, p, dt, dt_c, walking);
    }
    // 4.10
    v.walker.supported = false;
    turn_speed
}

/// FUN_0025eef8: from idle, a turn step when the legs trail far enough.
fn turn_step(v: &mut Vehicle, p: &WalkerParams, p2: f32, p3: f32) {
    let w = &v.walker;
    let thr = if w.dir == 0 && (p2 != 0.0 || p3 != 0.0) {
        STEP_THRESHOLD_MOVING
    } else {
        p.turn_threshold
    };
    let (hc, hs) = w.h;
    if hs > thr {
        v.walker.turn_step = -fmath::atan2(hs, hc);
        start_action(v, p, ACT_TURNR);
    } else if hs < -thr {
        v.walker.turn_step = -fmath::atan2(hs, hc);
        start_action(v, p, ACT_TURNL);
    }
}

/// FUN_0025ec40: the action's direction, then the table's next entry.
fn start_action(v: &mut Vehicle, p: &WalkerParams, mut action: usize) {
    let w = &mut v.walker;
    if action == ACT_FWD {
        w.dir = 1;
    } else if action == ACT_BACK {
        w.dir = 2;
    }
    if action == ACT_STOP && w.dir == 2 {
        action = ACT_STOP_BACK;
    }
    let row = if usize::from(w.anim) < ANIM_COUNT {
        usize::from(w.anim)
    } else {
        0
    };
    start_anim(v, p, TRANSITIONS[row][action]);
}

/// FUN_0025ea48: record the entry and start its clip, reversed while
/// walking backward; any entry but the walk family clears the direction.
fn start_anim(v: &mut Vehicle, p: &WalkerParams, anim: u8) {
    let w = &mut v.walker;
    if anim != ANIM_NONE {
        if !(5..=10).contains(&anim) {
            w.dir = 0;
        }
        w.reversed = w.dir == 2;
        w.clip_time = if w.reversed { p.duration(anim) } else { 0.0 };
    }
    w.anim = anim;
}

/// §4.9: advance the clip by `dt_c`, changing entries at a clip's end, and
/// move the body by the root's z (FUN_00260918); v.xz follows.
fn advance(v: &mut Vehicle, p: &WalkerParams, dt: f32, mut dt_c: f32, walking: bool) {
    let mut moved = 0.0;
    let dur = p.duration(v.walker.anim);
    let rem = if v.walker.reversed {
        v.walker.clip_time
    } else {
        dur - v.walker.clip_time
    };
    if rem <= dt_c {
        let d = play(v, p, rem);
        moved += d;
        move_body(v, d);
        let w = &mut v.walker;
        if (ANIM_TURNL..=ANIM_TURNR).contains(&w.anim) && dur > 0.0 {
            let phi = w.turn_step * rem / dur;
            w.turn_legs(phi);
        }
        dt_c -= rem;
        let action = if walking {
            match w.dir {
                1 => ACT_FWD,
                2 => ACT_BACK,
                _ => ACT_STOP,
            }
        } else {
            ACT_STOP
        };
        start_action(v, p, action);
    }
    let d = play(v, p, dt_c);
    move_body(v, d);
    let f = v.rot * Vec3::Z;
    if dt > 0.0 {
        let s = (moved + d) / dt;
        v.vel.x = s * f.x;
        v.vel.z = s * f.z;
    }
    let dur = p.duration(v.walker.anim);
    let w = &mut v.walker;
    if (ANIM_TURNL..=ANIM_TURNR).contains(&w.anim) {
        if dur > 0.0 {
            let phi = w.turn_step * dt_c / dur;
            w.turn_legs(phi);
        }
    } else if (3..=10).contains(&w.anim) {
        // FUN_0025ecd8: the legs come round ~10 % a tick.
        let (mut c, mut s) = w.h;
        c += s * s * LEG_DECAY;
        s -= c * s * LEG_DECAY;
        w.h = unit(c, s);
    }
}

/// Advance the clip player by `dt` (reversed: toward the start), held in
/// the clip; returns the root's z displacement × SkeletonRootScale.
fn play(v: &mut Vehicle, p: &WalkerParams, dt: f32) -> f32 {
    let w = &mut v.walker;
    let Some(c) = p.clip(w.anim) else {
        return 0.0;
    };
    let t0 = w.clip_time;
    let t1 = if w.reversed {
        (t0 - dt).max(0.0)
    } else {
        (t0 + dt).min(c.duration)
    };
    w.clip_time = t1;
    (c.z_at(t1) - c.z_at(t0)) * p.root_scale
}

/// FUN_00260918: move the body `d` along the legs' forward, pitched by the
/// body pitch.
fn move_body(v: &mut Vehicle, d: f32) {
    if d == 0.0 {
        return;
    }
    let e = Mat3::from_quat(v.rot);
    let (right, up, fwd) = (e.x_axis, e.y_axis, e.z_axis);
    let (hc, hs) = v.walker.h;
    let f = if hc < 1.0 {
        right * (2.0 * hs * hc) + fwd * (hc * hc - hs * hs)
    } else {
        fwd
    };
    let (sp, cp) = fmath::sin_cos(2.0 * v.walker.pitch);
    v.pos += (f * cp - up * sp) * d;
}

/// §8 B2: two horizontal terrain rays ahead of the body push it back from
/// terrain that rises steeply.
pub fn probes(v: &mut Vehicle, p: &WalkerParams, world: &impl WalkerWorld) {
    let f = v.rot * Vec3::Z;
    let Some(mut f) = Vec3::new(f.x, 0.0, f.z).try_normalize() else {
        return;
    };
    if v.walker.dir == 2 {
        f = -f;
    }
    let s = Vec3::new(-f.z, 0.0, f.x);
    let mut o = v.pos + Vec3::Y * (PROBE_UP * p.leg_ray);
    let mut r = PROBE_UP * p.leg_ray;
    if p.leg_pairs.len() >= 2 {
        r *= PROBE_LONG;
        o += f * p.leg_pairs[0].offset();
    }
    for d in [
        f * (PROBE_FWD * r) + s * (PROBE_SIDE * r),
        f * (PROBE_FWD * r) - s * (PROBE_SIDE * r),
    ] {
        if let Some(t) = world.terrain_segment(o, o + d) {
            if t < 1.0 {
                v.pos -= d * (1.0 - t);
            }
        }
    }
}

/// §6 FUN_002a2028: one pair's support S and height difference h0 − h1.
fn pair_support(
    v: &Vehicle,
    p: &WalkerParams,
    pair: &LegPair,
    world: &impl WalkerWorld,
) -> (f32, f32) {
    let l = p.leg_ray;
    let e = Mat3::from_quat(v.rot);
    let base_y = v.pos.y - fmath::sin_cos(2.0 * v.walker.pitch).0 * pair.offset();
    let mut h = [0.0; 2];
    for (i, foot) in pair.feet.iter().enumerate() {
        let origin = Vec3::new(v.pos.x, base_y, v.pos.z)
            + e.x_axis * foot.x
            + e.z_axis * foot.z
            + Vec3::Y * l;
        h[i] = match world.ground_below(origin, 2.0 * l) {
            Some(dist) => l - dist,
            None => -l,
        };
    }
    let (lo, hi) = (h[0].min(h[1]), h[0].max(h[1]));
    let s = if p.roll {
        (h[0] + h[1]) * 0.5
    } else if hi > l * 0.5 {
        lo + (hi - lo) * ((hi - l * 0.5) / (SUPPORT_BLEND * l)).min(1.0)
    } else {
        lo
    };
    (s, h[0] - h[1])
}

/// FUN_00260ba0 (§6): the legs hold the body on the ground under its feet,
/// or let it fall at 3 g (I16 moves it next tick); two or more pairs pitch
/// the body, and with the roll flag roll it (the bones' angles, §10).
pub fn legs(v: &mut Vehicle, p: &WalkerParams, world: &impl WalkerWorld, dt: f32) {
    if p.leg_pairs.is_empty() {
        return;
    }
    let pairs: Vec<(f32, f32)> = p
        .leg_pairs
        .iter()
        .map(|pair| pair_support(v, p, pair, world))
        .collect();
    // FUN_002a2028: each pair's layer blend follows its feet's height
    // difference. Drawing and exposed-rider hero queries read the result.
    for (b, &(_, d)) in v.walker.layer_blend.iter_mut().zip(&pairs) {
        *b += ((-LAYER_GAIN * d).clamp(-1.0, 1.0) - *b) * LAYER_RATE;
    }
    let max_s = pairs.iter().fold(-1.0f32, |m, &(s, _)| m.max(s));
    let avg_d = pairs.iter().map(|&(_, d)| d).sum::<f32>() / pairs.len() as f32;
    let g = -LEG_GRAVITY;
    if pairs.len() == 1 {
        if max_s >= 0.0 {
            if max_s - LIFT_SLACK > 0.0 {
                v.pos.y += max_s - LIFT_SLACK;
            }
            v.vel.y = 0.0;
            v.walker.supported = true;
        } else {
            v.vel.y += g * dt;
        }
    } else {
        if max_s > 0.0 {
            let delta = pairs[1].0 - pairs[0].0;
            if delta != 0.0 {
                v.walker.pitch =
                    (v.walker.pitch + PITCH_STEP * delta).clamp(-PITCH_CLAMP, PITCH_CLAMP);
            }
            let lift = (pairs[0].0.max(-LIFT_SLACK) + pairs[1].0.max(-LIFT_SLACK)) * 0.5;
            if lift != 0.0 {
                v.pos.y += lift;
                v.vel.y = 0.0;
                v.walker.supported = true;
            }
        } else {
            v.vel.y += g * dt;
        }
        if p.roll && avg_d.abs() > 1e-4 {
            let r = (avg_d * avg_d + p.width * p.width).sqrt();
            let (mut c, mut s) = v.walker.roll;
            let e = (avg_d / r) * c - (p.width / r) * s;
            s += c * e * ROLL_RATE;
            c -= s * e * ROLL_RATE;
            let (c, s) = unit(c, s);
            v.walker.roll = if s.abs() > PITCH_CLAMP {
                (ROLL_CLAMP_COS, PITCH_CLAMP.copysign(s))
            } else {
                (c, s)
            };
        }
    }
}

/// §9 C7: the class's building spheres against the world's buildings. A
/// foot on a face no steeper than n.y 0.2 stands on it (v.y = 0); anything
/// else is pushed out horizontally (PROVISIONAL distance: the horizontal
/// move that clears the depth along n, at most 2·depth·|n_xz|).
pub fn contacts(v: &mut Vehicle, p: &WalkerParams, world: &impl WalkerWorld) {
    let mut found = Vec::new();
    for sp in &p.building {
        found.clear();
        let at = v.pos;
        world.building_contacts(at + v.rot * sp.pos, sp.radius, &mut found);
        for c in &found {
            let depth = c.depth - (v.pos - at).dot(c.normal);
            if depth <= 0.0 {
                continue;
            }
            let n = c.normal;
            if sp.foot && n.y >= FOOT_MIN_NY {
                v.vel.y = 0.0;
                if n.y > SUPPORT_NY {
                    v.walker.supported = true;
                }
                continue;
            }
            let nh = Vec3::new(n.x, 0.0, n.z);
            let l = nh.length();
            if l < 1e-4 {
                continue;
            }
            let dist = (2.0 * depth * l).min(depth / l);
            v.pos += nh / l * dist;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
