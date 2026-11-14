//! The hover integrator (card HOVR): BF2's drive, spring points and contact
//! response, transcribed from `analysis/re-hover.md` [O] — the ONE integrator
//! of every `hover`, `treaded` and `commandhover` class on both discs (PLAN
//! locks BF2 ground handling; BF1-disc classes lose their stick-based turn
//! blend and the strafe→turn fold, §8, `FEEL_NOTES.md`).
//!
//! Per 1/30 tick, in the PS2's task order (re-hover §2; the contact pass's
//! place settled by `re-vehicle-rest.md` §5, card HSTEP): the controller
//! fills [`HoverInput`] → [`drive`] (§6: gravity, level spring,
//! velocity-matching thrust and carve, the yaw slew, the orientation and
//! position step, the visual tilt) → [`contacts`] (§7: the world's contacts
//! fill each point's compression at this pose, push the body out and bounce
//! it) → [`springs`] (§3 T3-T5, §4: each point's compression pushes the
//! body, then g decays). Only the spring branch (§6.5) is built: every shipped hover has 3-6
//! spring points (§0.4); a class with none runs it too, PROVISIONAL.
//!
//! Frames (re-hover conventions). The PS2 keeps E as row-vector rows right,
//! up, fwd; here `Vehicle::rot` maps the model's local axes (+x right, +y
//! up, +z forward) into the world, so `Mat3::from_quat(rot)`'s COLUMNS are
//! those rows, and a PS2 product A·B is the glam product B·A. E includes the
//! visual tilt T = Rx(pitch)·Rz(roll); the drive untilts it for its frames
//! (U) and builds the new orientation from the TILTED E, re-applying the new
//! tilt on top — so the tilt feeds back into the body, as on both discs
//! (§0.1). ω is in the body frame: x pitch, y yaw, z roll.
//!
//! Determinism: sines only through `fmath` (glam's rotation builders call the
//! host libm), sqrt through glam. The spring jitter's LCG is per vehicle
//! (PROVISIONAL: the PS2's is one engine global, DAT_0073d868, §4).
//!
//! BF2's boost and energy (§5.1 I5, §6.3, §6.7; card HOVX): throttle past
//! 0.9 with the boost button latches the boost, which drains the energy pool
//! (FUN_001e2010) and, once the pool runs dry, drops until the button is
//! pressed again; the pool refills at EnergyAutoRestore a second while
//! neither boosting nor jumping (FUN_001e1fc0). The speeder bikes' jump
//! (§6.9) pushes along E's up row for a time that grows with the speed.
//!
//! PROVISIONAL (re-hover §11.7 and ours): the jitter's RNG; the trample (§6.12, not built); the other body's
//! damage factor (1); missing model mass metadata; the depth correction between contacts of
//! one query ([`contacts`]); bodies are the CollisionWorld's SOLDIER subset
//! (the port has no vehicle subset yet, card VSUB); a hull box or cylinder
//! against a building is parry's exact contact (FUN_0016bf10's box branch is
//! not transcribed, §13.7); the energy refill's ×1.5 driver bonus
//! (DAT_005feda0, when FUN_00360018(driver, 6) passes) is not modelled. Not
//! built yet: a hull box or cylinder against the TERRAIN (§13.5's corner and
//! rim rules; no shipped hover has one in its terrain subset), the water
//! float (§4 S3, card HWAT), the AI's boost (I5's AI branch, card VAI), the
//! tread scroll (§6.18), the no-spring branch (§6.4). Hover/treaded pairs
//! run through `sim::hover_pairs` between contacts and springs (§13.15);
//! contacts with other vehicle physics classes remain unimplemented.
//!
//! Which bodies meet the world (re-hover §13, card HHUL): the spring spheres
//! meet the terrain and the buildings; [`HoverParams::hull`] holds the
//! class's OTHER bodies in those two subsets — on the discs one box,
//! `p_cube1` of the two BF2 fighter tanks, buildings only. The coll mesh and
//! every other primitive meet neither. A sphere meets the terrain by
//! [`terrain_sphere`] (§13.5).

use super::Vehicle;
use crate::fmath;
use crate::fmath::FloatExt;
use crate::lcg::GameLcg;
use glam::{Mat3, Quat, Vec3};

/// The engine's gravity (−9.8 in §6.5a).
pub const GRAVITY: f32 = 9.8;
/// At most six spring points (`if (5 < count) return`, re-vehicles §3.1).
pub const MAX_SPRINGS: usize = 6;
/// Hull contact: friction and restitution (§7 K7: μ 0.25, j = −1.125·s).
pub const HULL_FRICTION: f32 = 0.25;
pub const HULL_RESTITUTION: f32 = 0.125;
/// The energy pool E+0x1dc0 reads the class's K+0xf58 triple, which the ctor
/// fills with FUN_001e1c88(100, 20) (B2:136728) and no setter writes: at
/// most 100, at least 0 (K+0xf5c, ctor 0 B2:136629), and a pool that ran
/// dry works again once it refills to 20. The entity starts full (the ctor
/// copies K+0xf58, B2:130727-130732).
pub const ENERGY_MAX: f32 = 100.0;
pub const ENERGY_MIN: f32 = 0.0;
pub const ENERGY_RECOVER: f32 = 20.0;
/// §5.1 I5: the boost latches only with the throttle past this.
pub const BOOST_THROTTLE: f32 = 0.9;
/// §6.9: a jump starts only with the ground factor past this.
pub const JUMP_MIN_G: f32 = 0.9;

/// One `AddSpringBody` point with its per-point factors
/// (`game::vehicle_class::SpringPoint`; the class entry at +0xe94 + 32i).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Sphere centre in the vehicle's frame, and its radius.
    pub pos: Vec3,
    pub radius: f32,
    /// BodySpringLength.
    pub length: f32,
    pub velocity_spring: f32,
    pub velocity_damp: f32,
    pub omega_x_spring: f32,
    pub omega_x_damp: f32,
    pub omega_z_spring: f32,
    pub omega_z_damp: f32,
}

/// A hull body's shape, in its own space (the collision primitives' kinds,
/// as `crate::collision::Shape` holds them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HullShape {
    Sphere {
        radius: f32,
    },
    Box {
        half: Vec3,
    },
    /// Y-aligned, centred on the origin.
    Cylinder {
        radius: f32,
        half_height: f32,
    },
}

/// A body of the class's model that meets the world and is no spring (§13.2:
/// a primitive a `TerrainCollision` / `BuildingCollision` line names; §7
/// K7-K9), placed in the vehicle's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullBody {
    pub shape: HullShape,
    pub pos: Vec3,
    /// Shape space → the vehicle's frame.
    pub rot: Mat3,
    /// In the class's terrain subset / its building subset.
    pub terrain: bool,
    pub building: bool,
}

/// A hover class's numbers, as `game::vehicle_class::HoverClass` reads them
/// (constructor defaults, then the ODF). SETUP data: never hashed.
#[derive(Clone, Debug, PartialEq)]
pub struct HoverParams {
    /// Model vis-box volume + 0.0001 (BF2 FUN_00212b38, B2:125779-125782).
    /// The physics constructor's 1.0 is retained when no model is installed.
    pub mass: f32,
    pub set_altitude: f32,
    pub gravity_scale: f32,
    pub acceleration: f32,
    pub deceleration: f32,
    pub traction: f32,
    pub forward_speed: f32,
    pub reverse_speed: f32,
    pub strafe_speed: f32,
    pub spin_rate: f32,
    pub turn_rate: f32,
    pub pitch_rate: f32,
    pub turn_filter: f32,
    pub thrust_pitch_angle: f32,
    pub strafe_roll_angle: f32,
    pub bank_angle: f32,
    pub bank_filter: f32,
    pub level_spring: f32,
    pub velocity_spring: f32,
    pub velocity_damp: f32,
    pub omega_x_spring: f32,
    pub omega_x_damp: f32,
    pub omega_z_spring: f32,
    pub omega_z_damp: f32,
    /// BoostSpeed (K+0xf6c; 0 = no boost) and the yaw target's scale at it
    /// (K+0xf78, ctor 0.5) — §6.10's turn scale reads them boosting or not.
    pub boost_speed: f32,
    pub boost_turn_multiplier: f32,
    /// BoostAcceleration (K+0xf70, ctor 0): §6.7's step while boosting.
    pub boost_acceleration: f32,
    /// EnergyAutoRestore (K+0xf64, ctor 0): the refill a second.
    pub energy_auto_restore: f32,
    /// EnergyBoostDrain (K+0xf68, ctor 0): the drain a second while boosting.
    pub energy_boost_drain: f32,
    /// The speeder bikes' jump (§6.9); off while `jump.force` ≤ 0.
    pub jump: JumpParams,
    pub collision_threshold: [f32; 3],
    pub collision_scale: [f32; 3],
    /// The class is BF1's: its AI driver takes BF1's rules
    /// (`ai::hover_drive`, re-hover-ai §9). Set from the map's disc.
    pub bf1_ai: bool,
    /// The ×(0.9 + 0.1·u) spring jitter (K+0xe90, default on).
    pub random_spring: bool,
    /// `FloatsOnWater` (BF2 K+0xe76, ctor 1): the spring points also press
    /// on the water surface (§4 S3). BF1 has no flag and always floats.
    pub floats_on_water: bool,
    /// At most [`MAX_SPRINGS`].
    pub springs: Vec<Spring>,
    /// The non-spring bodies that meet the world (§13.2).
    pub hull: Vec<HullBody>,
}

/// §6.9's five class fields (K+0xf88..+0xf98), constructor defaults
/// (B2:136729-136739). Only the two BF2 speeder bikes author them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpParams {
    /// `JumpTimeMin` (+0xf88): the push time at the start speed, and the
    /// time after which releasing the button stops it.
    pub time_min: f32,
    /// `JumpTimeMax` (+0xf8c): the push time at ForwardSpeed.
    pub time_max: f32,
    /// `JumpForce` (+0xf90): the push along E's up row, m/s².
    pub force: f32,
    /// Hash 0x534dfe50 (+0xf94, ctor 0.3; the name is not known): the jump
    /// starts only above this fraction of ForwardSpeed.
    pub speed_factor: f32,
    /// Hash 0x391e36a1 (+0xf98): the energy drain a second while jumping.
    pub energy_drain: f32,
}

impl Default for JumpParams {
    fn default() -> Self {
        Self {
            time_min: 0.0,
            time_max: 0.0,
            force: 0.0,
            speed_factor: 0.3,
            energy_drain: 0.0,
        }
    }
}

/// The controllable's inputs (E+0x2dc throttle, +0x2e0 strafe, +0x2e4 turn,
/// +0x2e8 pitch), written by whoever drives it each tick; zero when empty.
/// The signs are the PS2's: ω.yaw's target is −rate·turn (§6.10).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HoverInput {
    pub throttle: f32,
    pub strafe: f32,
    pub turn: f32,
    pub pitch: f32,
    /// A human at the pad (E.driver ≥ 0): the turn rate blends from
    /// PitchRate by the full-stick timer (§5.1 I7, §6.10). Offline every
    /// human takes the pad branch (§11.5). False = AI or nobody.
    pub human: bool,
    /// The boost button (E+0x2bc bit 0) and the jump button (E+0x2b4 bit 0),
    /// held this tick.
    pub boost: bool,
    pub jump: bool,
}

impl HoverInput {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for x in [self.throttle, self.strafe, self.turn, self.pitch] {
            eat(&x.hash_bits().to_le_bytes());
        }
        eat(&[self.human as u8, self.boost as u8, self.jump as u8]);
    }
}

/// What a bot's driving keeps on the hover (card VDROP, re-hover-ai §1,
/// §7; `ai::hover_drive` moves it, `Sim::ride` stores it): the unstick
/// manoeuvre (E+0x584 state, +0x590 the forward it began with, +0x5a0 /
/// +0x5a4 the stamps, here ticks) and the path follower's segment and stuck
/// test (m+0x80 anchor, +0x90, +0x94 closest distance², +0x98).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverAi {
    pub state: u8,
    pub fwd0: Vec3,
    pub state2_at: u64,
    pub ended_at: Option<u64>,
    pub leg: Option<[Vec3; 2]>,
    pub anchor: Vec3,
    pub still: f32,
    pub best: f32,
    pub far: f32,
}

impl Default for HoverAi {
    fn default() -> Self {
        Self {
            state: 0,
            fwd0: Vec3::ZERO,
            state2_at: 0,
            ended_at: None,
            leg: None,
            anchor: Vec3::ZERO,
            still: 0.0,
            best: f32::MAX,
            far: 0.0,
        }
    }
}

impl HoverAi {
    /// A new driver's path follower: the segment and the stuck test start
    /// over; the manoeuvre's state and stamps are the hover's and stay.
    pub fn unseat(&mut self) {
        *self = Self {
            state: self.state,
            fwd0: self.fwd0,
            state2_at: self.state2_at,
            ended_at: self.ended_at,
            ..Self::default()
        };
    }

    fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[8, self.state]);
        eat(&self.state2_at.to_le_bytes());
        eat(&self.ended_at.unwrap_or(u64::MAX).to_le_bytes());
        let [a, b] = self.leg.unwrap_or([Vec3::NAN; 2]);
        for v in [self.fwd0, a, b, self.anchor] {
            for x in v.to_array() {
                eat(&x.hash_bits().to_le_bytes());
            }
        }
        for x in [self.still, self.best, self.far] {
            eat(&x.hash_bits().to_le_bytes());
        }
    }
}

/// The integrator's own entity fields.
#[derive(Clone, Debug, PartialEq)]
pub struct HoverState {
    /// ω, body frame (E+0x4d0 pitch / +0x4d4 yaw / +0x4d8 roll).
    pub omega: Vec3,
    /// The ground factor g (E+0x4e0): 1 when a spring touched, then −dt a
    /// tick. PROVISIONAL: starts at 0.
    pub g: f32,
    /// The visual tilt (E+0x544 pitch / +0x548 roll), radians.
    pub tilt_pitch: f32,
    pub tilt_roll: f32,
    /// Each spring's compression c from this tick's contacts (E+0x1cf0+32i),
    /// used and cleared by the spring tick that follows them in [`step`]
    /// (so 0 between ticks, as in PS2 RAM).
    pub compress: [f32; MAX_SPRINGS],
    /// The full-stick timer (E+0x360, §5.2): +dt while turn² + pitch² ≥ 0.9,
    /// else 0, capped at 1. Ramped only for a human.
    pub stick_timer: f32,
    /// The jitter LCG's state (PROVISIONAL: per vehicle).
    pub jitter: u32,
    /// The energy pool (E+0x1dc0: value +0x1dc4, flag +0x1dc8 bit 0 =
    /// `drained`, set when a drain takes it below 0 and cleared when it
    /// refills to [`ENERGY_RECOVER`]).
    pub energy: f32,
    pub drained: bool,
    /// The boost latch (E+0x1ddc), kept across ticks (§5.1 I5).
    pub boosting: bool,
    /// The jump (E+0x1dd8 bit 41) and its timer (E+0x1dd4).
    pub jumping: bool,
    pub jump_timer: f32,
    /// A bot driver's state; hashed only once a bot has driven.
    pub ai: HoverAi,
}

impl Default for HoverState {
    fn default() -> Self {
        Self {
            omega: Vec3::ZERO,
            g: 0.0,
            tilt_pitch: 0.0,
            tilt_roll: 0.0,
            compress: [0.0; MAX_SPRINGS],
            stick_timer: 0.0,
            jitter: 0,
            energy: ENERGY_MAX,
            drained: false,
            boosting: false,
            jumping: false,
            jump_timer: 0.0,
            ai: HoverAi::default(),
        }
    }
}

impl HoverState {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for x in [
            self.omega.x,
            self.omega.y,
            self.omega.z,
            self.g,
            self.tilt_pitch,
            self.tilt_roll,
            self.stick_timer,
            self.energy,
            self.jump_timer,
        ]
        .into_iter()
        .chain(self.compress)
        {
            eat(&x.hash_bits().to_le_bytes());
        }
        eat(&self.jitter.to_le_bytes());
        eat(&[self.drained as u8, self.boosting as u8, self.jumping as u8]);
        if self.ai != HoverAi::default() {
            self.ai.hash_into(eat);
        }
    }

    /// FUN_001e1fc0: the pool refills by `x`; at [`ENERGY_RECOVER`] or more
    /// it works again, capped at [`ENERGY_MAX`].
    pub fn refill(&mut self, x: f32) {
        self.energy += x;
        if ENERGY_RECOVER <= self.energy {
            self.drained = false;
            if ENERGY_MAX < self.energy {
                self.energy = ENERGY_MAX;
            }
        }
    }

    /// FUN_001e2010: take `x` from the pool. False only when it ran dry
    /// before (the tick that takes it below 0 still succeeds, floored at
    /// [`ENERGY_MIN`]); a non-positive `x` always succeeds.
    pub fn drain(&mut self, x: f32) -> bool {
        if x <= 0.0 {
            return true;
        }
        if self.drained {
            return false;
        }
        self.energy -= x;
        if self.energy < 0.0 {
            self.drained = true;
            if self.energy < ENERGY_MIN {
                self.energy = ENERGY_MIN;
            }
        }
        true
    }
}

/// Rotation about local +x by `a` (the PS2's Rx rows (1,0,0),(0,c,s),(0,−s,c)
/// = glam's `from_rotation_x`), built through `fmath`.
pub fn quat_x(a: f32) -> Quat {
    let (s, c) = fmath::sin_cos(a * 0.5);
    Quat::from_xyzw(s, 0.0, 0.0, c)
}

/// Rotation about +y by `a` (glam's `from_rotation_y`), through `fmath`.
pub fn quat_y(a: f32) -> Quat {
    let (s, c) = fmath::sin_cos(a * 0.5);
    Quat::from_xyzw(0.0, s, 0.0, c)
}

/// Rotation about local +z by `a` (Rz rows (c,s,0),(−s,c,0),(0,0,1)).
pub fn quat_z(a: f32) -> Quat {
    let (s, c) = fmath::sin_cos(a * 0.5);
    Quat::from_xyzw(0.0, 0.0, s, c)
}

/// E's untilted frame U = M·E, M = Rz(−roll)·Rx(−pitch) (§6.2), as a
/// rotation: columns right, up, fwd.
fn untilted(v: &Vehicle) -> Mat3 {
    Mat3::from_quat(untilted_rot(v))
}

/// [`untilted`] as a rotation: the body frame the hover's camera hangs on
/// (B2 `FUN_00220d68` B2:135389-135404, B1 B1:100694-100696).
pub fn untilted_rot(v: &Vehicle) -> Quat {
    let h = &v.hover;
    v.rot * quat_x(-h.tilt_pitch) * quat_z(-h.tilt_roll)
}

/// A hover pilot's BODY camera section as the aim ray reads it
/// (`analysis/re-hover-camera.md` §2.1): `TrackCenter` on the body,
/// `TrackOffset` in the view (x right, y up, z back), the view the aim
/// tilted down by `TiltValue` (radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChaseSection {
    pub center: Vec3,
    pub offset: Vec3,
    pub tilt: f32,
    /// A walker gunner seat's `MountPos` (model space): with no turret node
    /// its aim frame, and so its camera, stands there (B2 `FUN_0026cea0`
    /// B2:188710); `None` = the model's origin.
    pub mount: Option<Vec3>,
}

impl ChaseSection {
    /// The chase camera's ideal place, before any pull or lag (B2
    /// `FUN_001ce948`: E·`TrackCenter` + the tilted view's rows ·
    /// `TrackOffset`, B2:132376-132439), on the frame the drawn camera hangs on.
    pub fn camera(&self, v: &Vehicle, look_pitch: f32) -> Vec3 {
        self.camera_on(v.pos, untilted_rot(v), (0.0, look_pitch))
    }

    /// [`ChaseSection::camera`] on the body frame `frame` at `pos`, the view
    /// turned to the aim (`aim` = model-frame yaw, pitch up): `frame`·
    /// `TrackCenter` + the tilted view's rows · `TrackOffset`. The walker
    /// builds its view from its aim direction E+0x350 and the body's up row
    /// before the same tilt and the same FUN_001ce948 (B2 FUN_00261208).
    pub fn camera_on(&self, pos: Vec3, frame: Quat, aim: (f32, f32)) -> Vec3 {
        let view = frame
            * quat_y(aim.0)
            * quat_x(-aim.1)
            * quat_y(std::f32::consts::PI)
            * quat_x(-self.tilt);
        pos + frame * self.center + view * self.offset
    }

    /// Where a driven hover's aim ray starts in the chase view (B2
    /// `FUN_0021b470`, B2:132440-132461): the camera's ideal place carried
    /// along the aim direction `dir` to the depth of the eye `eye`
    /// (E+0x340), cam + dir·((eye − cam)·dir). The ray, 512 m along `dir`
    /// from there (B2:132474-132477), so passes through the camera and the
    /// shots meet what the reticle covers.
    pub fn ray_origin(&self, v: &Vehicle, look_pitch: f32, eye: Vec3, dir: Vec3) -> Vec3 {
        let cam = self.camera(v, look_pitch);
        cam + dir * (eye - cam).dot(dir)
    }

    /// A driven walker's [`ChaseSection::ray_origin`] (B2 FUN_00261208: the
    /// camera FUN_001ce948 on E+0x120 with the aim's view, carried along
    /// the aim E+0x350 to the eye E+0x340's depth into E+0x3c0, then the
    /// 512 m ray FUN_00161c60 from there): the walker's own frame, not the
    /// hover's untilted one, and the view turned by the aim's yaw as well.
    pub fn walker_ray_origin(&self, v: &Vehicle, aim: (f32, f32), eye: Vec3, dir: Vec3) -> Vec3 {
        let cam = self.camera_on(v.pos, v.rot, aim);
        cam + dir * (eye - cam).dot(dir)
    }

    /// A player gunner's [`ChaseSection::ray_origin`]: the turret's chase
    /// camera, `TrackCenter` on its aim frame and `TrackOffset` in the view
    /// (the aim tilted down by `TiltValue`; x right, y up, z back), carried
    /// along the aim `dir` to the eye's depth, then the 512 m ray from there
    /// (B2 FUN_0026cea0: the view from the aim T+0xf0 and the aim frame's up
    /// row T+0x1d0, B2:189181-189217, the camera FUN_001ce948 on the aim
    /// frame T+0x1c0, B2:188973, 189240, into T+0x160, B2:189241-189258, the
    /// ray B2:189275; B1 FUN_00223850 B1:137707-137842, FUN_001b5850). So
    /// the camera lies on the ray and the reticle covers what the guns hit.
    /// The aim frame is the gun's look at `at` in the vehicle's frame, the
    /// frame the port's drawn gunner camera hangs on.
    pub fn gunner_ray_origin(
        &self,
        v: &Vehicle,
        at: Vec3,
        look: (f32, f32),
        eye: Vec3,
        dir: Vec3,
    ) -> Vec3 {
        let [x, y, z] = super::gun::frame(look);
        let (s, c) = fmath::sin_cos(self.tilt);
        let forward = z * c - y * s;
        let up = y * c + z * s;
        let local = at + x * self.center.x + y * self.center.y + z * self.center.z
            - x * self.offset.x
            + up * self.offset.y
            - forward * self.offset.z;
        let cam = v.pos + v.rot * local;
        cam + dir * (eye - cam).dot(dir)
    }
}

/// §5.2, the player controller's timer (pad only): +dt while the RAW look
/// stick is at full deflection (turn² + pitch² ≥ 0.9, tested before the
/// stick shaping — `Sim`'s seat pass passes `full`), else 0, capped at 1.
pub fn ramp_stick_timer(st: &mut HoverState, full: bool, dt: f32) {
    st.stick_timer = if full {
        (st.stick_timer + dt).fmin(1.0)
    } else {
        0.0
    };
}

/// §6.10: the yaw rate's target for this speed and stick, before the slew.
/// `v_fwd` is E's forward row · v, `speed` is |v|: above ForwardSpeed a
/// class with a BoostSpeed turns slower, toward ×BoostTurnMultiplier at
/// BoostSpeed (B2:134382-134398) — whether or not it is boosting.
pub fn yaw_target(p: &HoverParams, inp: &HoverInput, timer: f32, v_fwd: f32, speed: f32) -> f32 {
    let f = (v_fwd / p.forward_speed).clamp(0.0, 1.0);
    let base = p.spin_rate + (p.turn_rate - p.spin_rate) * f;
    let rate = if inp.human {
        p.pitch_rate + (base - p.pitch_rate) * timer
    } else {
        base
    };
    let mut target = -rate * inp.turn;
    if p.boost_speed > 0.0 && speed > p.forward_speed {
        let fb = ((speed - p.forward_speed) / (p.boost_speed - p.forward_speed)).clamp(0.0, 1.0);
        target *= 1.0 - fb * (1.0 - p.boost_turn_multiplier);
    }
    target
}

/// §6.10: ω.yaw steps TurnFilter·dt toward its target and stops there (BF2's
/// slew; BF1 filters exponentially, §8). Building turrets also use this
/// linear rate step.
pub fn slew(w: f32, target: f32, turn_filter: f32, dt: f32) -> f32 {
    let step = turn_filter * dt;
    if w < target {
        (w + step).fmin(target)
    } else {
        (w - step).fmax(target)
    }
}

/// One tick of one hover, in the PS2's order (task keys 0 / 10 / 15,
/// `re-vehicle-rest.md` §5 [O]): the drive, the contacts (they fill each
/// spring's compression at the pose the drive left), then the spring tick
/// (it uses and clears them) — the controller (the input and its timer) ran
/// before, `Sim`'s seat pass. Returns the impacts to deal
/// (`Sim::impact_vehicle`).
pub fn step(v: &mut Vehicle, p: &HoverParams, world: &impl ContactSource, dt: f32) -> Vec<Impact> {
    drive(v, p, dt);
    let impacts = contacts(v, p, world);
    springs(v, p, |at| world.water_under(at), dt);
    impacts
}

/// The drive FUN_0021d8b8, spring branch (§6.1-6.2, 6.5, 6.7-6.8, 6.10-6.16).
/// `g` is what the last spring tick left (1 − dt when touching).
pub fn drive(v: &mut Vehicle, p: &HoverParams, dt: f32) {
    let inp = v.input;
    let thr = inp.throttle;
    // 6.1
    let str_ = if p.strafe_speed < 1e-4 {
        0.0
    } else {
        inp.strafe
    };
    // 6.2
    let u = untilted(v);
    let (right, up, fwd) = (u.x_axis, u.y_axis, u.z_axis);
    let m = thr.abs().fmax(str_.abs());
    let g = v.hover.g;
    let boost = energy(v, p, dt);

    // 6.5a gravity, weighted by the ground factor.
    v.vel.y += (p.gravity_scale + (1.0 - p.gravity_scale) * g) * -GRAVITY * dt;
    // 6.5b the level spring (no LevelDamp on this branch).
    let ls = (p.level_spring * dt).fmin(1.0);
    v.hover.omega.x += fwd.y * fwd.y.abs() * ls;
    v.hover.omega.z -= right.y * right.y.abs() * ls;

    // 6.7 velocity matching; boosting, BoostAcceleration toward BoostSpeed.
    let s = g * up.y.fmax(0.0) * dt;
    let a_s = if boost {
        p.boost_acceleration
    } else {
        p.deceleration + (p.acceleration - p.deceleration) * m
    } * s;
    let t_s = p.traction * s;
    let target = if thr < 0.0 {
        p.reverse_speed
    } else if boost {
        p.boost_speed
    } else {
        p.forward_speed
    };
    v.vel += fwd * (thr * target - v.vel.dot(fwd)).clamp(-a_s, a_s);
    if p.strafe_speed < 1e-4 {
        v.vel += right * (-str_ * p.strafe_speed - v.vel.dot(right)).clamp(-t_s, t_s);
    } else {
        v.vel += right * (-str_ * p.strafe_speed - v.vel.dot(right)).clamp(-a_s, a_s);
        // 6.8 the carve: the velocity follows the heading.
        if m > 1e-4 {
            let mut c = (up * v.hover.omega.y).cross(v.vel) * dt;
            let lim = t_s - a_s;
            let c2 = c.length_squared();
            if lim * lim < c2 {
                c *= lim / c2.sqrt();
            }
            v.vel += c * m;
        }
    }

    // 6.9 the jump, measured on E's (tilted) rows.
    let e_fwd = v.rot * Vec3::Z;
    if p.jump.force > 0.0 {
        jump(v, p, e_fwd, dt);
    }
    // 6.10 yaw: E's (tilted) forward row measures the speed.
    let target = yaw_target(
        p,
        &inp,
        v.hover.stick_timer,
        e_fwd.dot(v.vel),
        v.vel.length(),
    );
    v.hover.omega.y = slew(v.hover.omega.y, target, p.turn_filter, dt);

    // 6.11 q from the TILTED E, stepped by ω·dt in the body frame.
    let mut q = v.rot;
    let w = v.hover.omega.length();
    if w > 1e-4 {
        let (sn, cs) = fmath::sin_cos(w * dt * 0.5);
        let axis = v.hover.omega / w * sn;
        q = (q * Quat::from_xyzw(axis.x, axis.y, axis.z, cs)).normalize();
    }
    // 6.13 position, and the velocity in U's frame.
    v.pos += v.vel * dt;
    let v_local = Vec3::new(right.dot(v.vel), up.dot(v.vel), fwd.dot(v.vel));
    // 6.14 bank input.
    let k = v.hover.omega.cross(v_local) / p.forward_speed;
    // 6.15 the tilt.
    let h = &mut v.hover;
    h.tilt_pitch +=
        ((thr * p.thrust_pitch_angle + k.z * p.bank_angle) - h.tilt_pitch) * p.bank_filter * dt;
    h.tilt_roll +=
        ((str_ * p.strafe_roll_angle - k.x * p.bank_angle) - h.tilt_roll) * p.bank_filter * dt;
    // 6.16 E' = T·mat(q'), T = Rx(pitch)·Rz(roll).
    v.rot = (q * quat_z(h.tilt_roll) * quat_x(h.tilt_pitch)).normalize();
    // 6.19: a SetAltitude ≤ 0 class keeps g only from this tick's springs
    // and K8 — airborne it has no traction and full GravityScale.
    if p.set_altitude <= 0.0 {
        v.hover.g = 0.0;
    }
}

/// §5.1 I5 and §6.3: this tick's boost. A human latches it with the
/// throttle past 0.9 and the button (held, or latched last tick); the pool
/// then pays EnergyBoostDrain a second, and a pool that ran dry (or a class
/// with no BoostSpeed) drops the latch. Not boosting and not jumping, the
/// pool refills at EnergyAutoRestore. The latch persists (E+0x1ddc).
fn energy(v: &mut Vehicle, p: &HoverParams, dt: f32) -> bool {
    let inp = v.input;
    let h = &mut v.hover;
    h.boosting = inp.human && inp.throttle > BOOST_THROTTLE && (inp.boost || h.boosting);
    if h.boosting && !(p.boost_speed > 0.0 && h.drain(p.energy_boost_drain * dt)) {
        h.boosting = false;
    }
    if !h.boosting && !h.jumping {
        h.refill(p.energy_auto_restore * dt);
    }
    h.boosting
}

/// §6.9 (B2:134251-134333): with the button, g past 0.9 and E's forward
/// speed past ForwardSpeed·f94 the jump starts (no push that tick). Jumping,
/// each tick pays the jump drain and pushes along E's up row by JumpForce
/// until the timer reaches a limit that grows from JumpTimeMin at the start
/// speed to JumpTimeMax at ForwardSpeed; past JumpTimeMin, letting go ends
/// it, and so does a pool that ran dry.
fn jump(v: &mut Vehicle, p: &HoverParams, e_fwd: Vec3, dt: f32) {
    let j = &p.jump;
    let button = v.input.jump;
    let v_fwd = e_fwd.dot(v.vel);
    let start = p.forward_speed * j.speed_factor;
    let h = &mut v.hover;
    if !h.jumping {
        h.jumping = button && h.g > JUMP_MIN_G && v_fwd > start;
        h.jump_timer = 0.0;
        return;
    }
    if (j.time_min <= h.jump_timer && !button) || !h.drain(j.energy_drain * dt) {
        h.jumping = false;
        return;
    }
    h.jump_timer += dt;
    let f = (v_fwd - start) / (p.forward_speed - start);
    let f = if f >= 0.0 { f.fmin(1.0) } else { 0.0 };
    if h.jump_timer < (1.0 - f) * j.time_min + f * j.time_max {
        v.vel += v.rot * Vec3::Y * (j.force * dt);
    } else {
        h.jumping = false;
    }
}

/// The tick's spring loop and g decay (§3 T3-T5, §4 S3-S5): each point's
/// sphere first presses on the water surface `water` gives under its centre
/// (B2 FUN_0021f5d0 B2:134256-134278, B1 FUN_001ee040 B1:99590-99613), then
/// its compression from this tick's contacts and the water pushes the body
/// along E's up row and torques ω, then is cleared.
pub fn springs(v: &mut Vehicle, p: &HoverParams, water: impl Fn(Vec3) -> Option<f32>, dt: f32) {
    let e = Mat3::from_quat(v.rot);
    let up = e.y_axis;
    for (i, sp) in p.springs.iter().enumerate().take(MAX_SPRINGS) {
        if p.floats_on_water {
            let centre = v.pos + e * sp.pos;
            if let Some(h) = water(centre) {
                let d = (h - (centre.y - sp.radius)) / sp.length;
                if d > 0.0 {
                    v.hover.compress[i] = v.hover.compress[i].max(d.min(1.0));
                    if d > 1.0 && v.vel.y < 0.0 {
                        v.vel.y *= 0.5;
                        v.vel.x *= 0.9;
                        v.vel.z *= 0.9;
                        v.hover.omega *= 0.9;
                    }
                }
            }
        }
        let mut c = v.hover.compress[i];
        if c > 0.0 {
            if p.random_spring {
                let mut lcg = GameLcg::new(v.hover.jitter);
                c *= 0.9 + 0.1 * lcg.unit();
                v.hover.jitter = lcg.state();
            }
            let h = &mut v.hover;
            h.g = 1.0;
            let push = c * sp.velocity_spring * p.velocity_spring
                - (1.0 - c) * sp.velocity_damp * p.velocity_damp * v.vel.dot(up).fmin(0.0);
            v.vel += up * push * dt;
            h.omega.x += (c * sp.omega_x_spring * p.omega_x_spring
                - (1.0 - c) * h.omega.x * sp.omega_x_damp * p.omega_x_damp)
                * dt;
            h.omega.z += (c * sp.omega_z_spring * p.omega_z_spring
                - (1.0 - c) * h.omega.z * sp.omega_z_damp * p.omega_z_damp)
                * dt;
        }
        v.hover.compress[i] = 0.0;
    }
    v.hover.g = (v.hover.g - dt).fmax(0.0);
}

/// A sphere pressed `depth` into the world along the unit `normal` (out of
/// the world, toward the sphere).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub depth: f32,
    pub normal: Vec3,
}

/// The world a hover's bodies touch: every contact, in a fixed order.
pub trait ContactSource {
    /// A spring sphere, against the terrain and the buildings.
    fn sphere_contacts(&self, centre: Vec3, radius: f32, out: &mut Vec<Contact>);
    /// A hull body at the world pose `pos`, `rot` (shape space → world),
    /// against the layers its flags name.
    fn hull_contacts(&self, body: &HullBody, pos: Vec3, rot: Mat3, out: &mut Vec<Contact>);
    /// The water surface (level + bob) over a wet cell at `at`, as
    /// FUN_005613c8 gives it; `None` off the water.
    fn water_under(&self, _at: Vec3) -> Option<f32> {
        None
    }
}

/// A sphere against the terrain (§13.5, B2 FUN_0016c6c0 kind 0 / B1
/// FUN_0016b7d8): `h` is the height under the centre and `n` the terrain
/// triangle's normal there. One sample under the lowest point: d = h − c.y +
/// r, a contact only for 0 ≤ d ≤ 2r — the window is VERTICAL, the terrain
/// within r of the centre's height — and the depth is d·n.y along n. The
/// caller drops it where a cutter cuts the surface point (c.x, h, c.z).
pub fn terrain_sphere(centre_y: f32, radius: f32, h: f32, n: Vec3) -> Option<Contact> {
    let d = h - centre_y + radius;
    (0.0..=2.0 * radius).contains(&d).then_some(Contact {
        depth: d * n.y,
        normal: n,
    })
}

/// An impact the hull takes (§7 K5 bottomed spring): the closing speed along
/// the normal and the normal against E's up and forward rows — the caller's
/// `Sim::impact_vehicle`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub normal_speed: f32,
    pub n_up: f32,
    pub n_fwd: f32,
}

/// Each contact `query` finds for one body at the vehicle's position, found
/// at the pose the earlier push-outs left and resolved in order: a contact's
/// depth loses what this body's earlier push-outs moved it along its normal, and
/// one left at no depth is gone. FUN_00160da8 (B2:711-800) moves its query
/// origin by each handled response's displacement (FUN_0016beb8) before
/// FUN_0016bf10 finds the next contact; the depth correction for contacts of
/// one query is ours (PROVISIONAL), exact for a plane.
fn each_contact(
    v: &mut Vehicle,
    found: &mut Vec<Contact>,
    query: impl FnOnce(Vec3, &mut Vec<Contact>),
    mut respond: impl FnMut(&mut Vehicle, Contact),
) {
    found.clear();
    let at = v.pos;
    query(at, found);
    for c in found.iter() {
        let depth = c.depth - (v.pos - at).dot(c.normal);
        if depth > 0.0 {
            respond(
                v,
                Contact {
                    depth,
                    normal: c.normal,
                },
            );
        }
    }
}

/// The contact pass (§7, FUN_0021fd98 against static world bodies, v_other
/// = 0): spring spheres through the hook and K5, then the hull bodies
/// through K7-K9, each body queried after the one before has pushed the
/// vehicle out ([`each_contact`]). Returns the impacts to deal.
pub fn contacts(v: &mut Vehicle, p: &HoverParams, world: &impl ContactSource) -> Vec<Impact> {
    let e = Mat3::from_quat(v.rot);
    let (up, fwd) = (e.y_axis, e.z_axis);
    let mut found = Vec::new();
    let mut impacts = Vec::new();
    for (i, sp) in p.springs.iter().enumerate().take(MAX_SPRINGS) {
        let query = |at: Vec3, out: &mut Vec<Contact>| {
            world.sphere_contacts(at + e * sp.pos, sp.radius, out)
        };
        each_contact(v, &mut found, query, |v, c| {
            let n = c.normal;
            let s = v.vel.dot(n);
            // The hook FUN_0021f508: only a floor-like normal compresses.
            let nu = n.dot(up);
            if nu > 0.5 && n.y > 0.5 {
                let ci = sp.length.fmin(c.depth * nu) / sp.length;
                v.hover.compress[i] = v.hover.compress[i].fmax(ci);
            }
            // K5: within the spring's length the contact is the spring's.
            if c.depth <= sp.length {
                return;
            }
            impacts.push(Impact {
                normal_speed: s,
                n_up: nu,
                n_fwd: n.dot(fwd),
            });
            if s < 0.0 && v.alive {
                v.vel -= n * s;
                v.vel -= Vec3::new(n.x, 0.0, n.z) * (0.1 * s);
            }
            v.pos += n * (c.depth - sp.length);
        });
    }
    for hb in &p.hull {
        let query = |at: Vec3, out: &mut Vec<Contact>| {
            world.hull_contacts(hb, at + e * hb.pos, e * hb.rot, out)
        };
        each_contact(v, &mut found, query, |v, c| {
            let n = c.normal;
            let s = v.vel.dot(n);
            // K7, a static other body: no damage (§0.5).
            if s < 0.0 && v.alive {
                let mu = if p.set_altitude == 0.0 {
                    let a = n.dot(up).fmax(0.0);
                    (1.0 - a * a * a * a) * HULL_FRICTION
                } else {
                    HULL_FRICTION
                };
                let j = -(1.0 + HULL_RESTITUTION) * s;
                let mut vt = v.vel - n * s;
                let l = vt.length();
                if l > 0.0 {
                    vt -= vt * (mu * j / l).fmin(1.0);
                }
                v.vel = vt + n * (j + s);
            }
            // K8
            if p.set_altitude <= 0.0 && n.y > 0.5 {
                v.hover.g = 1.0;
            }
            // K9
            v.pos += n * c.depth;
        });
    }
    impacts
}

#[cfg(test)]
pub(crate) mod tests;
