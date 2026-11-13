//! Headless flyer motion, from the PS2 updates FUN_001d9180 (BF1) and
//! FUN_00202d08 (BF2). See analysis/re-flyer.md for source identities,
//! equations and the outstanding collision/live-comparison work.
//!
//! This kernel owns flight state and motion. The caller supplies the down
//! ray, height band and landing-region membership, then handles contacts.
//! There is no terrain-following hover spring and no artificial pitch cap.

use super::hover::{quat_x, quat_z, HoverInput};
use super::Vehicle;
use crate::fmath;
use glam::{Quat, Vec3};

mod maneuver;
pub use maneuver::{FlightReversal, ManeuverKind, PilotManeuvers};

#[derive(Clone, Debug, PartialEq)]
pub struct FlyerParams {
    pub bf2: bool,
    pub never_crash_when_unpiloted: bool,
    pub trick_roll_speed: f32,
    pub trick_flip_speed: f32,
    pub pilot_repair_scale: f32,
    pub aim_box: f32,
    pub trick_drain: f32,
    pub trick_flip_drain: f32,
    pub trick_side_strafe: f32,
    pub has_chunks: bool,
    pub view_fov: f32,
    pub acceleration: f32,
    pub min_speed: f32,
    pub mid_speed: f32,
    pub max_speed: f32,
    pub strafe_speed: f32,
    pub pitch_rate: f32,
    pub turn_rate: f32,
    pub pitch_filter: f32,
    /// BF2 PitchFilterDecel, a rate slew bound rather than exponential drag.
    pub pitch_damp: f32,
    pub turn_filter: f32,
    pub turn_damp: f32,
    pub roll_rate: f32,
    pub roll_filter: f32,
    pub thrust_pitch_angle: f32,
    pub strafe_roll_angle: f32,
    pub bank_angle: f32,
    pub bank_filter: f32,
    pub level_filter: f32,
    pub level_damp: f32,
    pub takeoff_height: f32,
    pub takeoff_time: f32,
    pub takeoff_speed: f32,
    pub landing_time: f32,
    pub landing_speed: f32,
    pub landed_height: f32,
    pub boost_speed: f32,
    pub command_flyer: bool,
    pub boost_acceleration: f32,
    pub energy_max: f32,
    pub energy_auto_restore: f32,
    pub energy_boost_drain: f32,
    pub velocity_filter: f32,
    pub collision_threshold: [f32; 3],
    pub collision_scale: [f32; 3],
    /// BF2 AI: the path follower's class numbers (FUN_00300168).
    pub follower: FollowerParams,
}

/// BF2 FUN_00300168's defaults; authored through the setter FUN_00300230.
/// See analysis/re-flyer-ai.md for hashes and unit conversions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowerParams {
    /// PathFollowerClass, BF2 class +0x54 (zero bypasses discovery filtering).
    pub class_id: u32,
    pub speed: f32,
    pub turn_speed: f32,
    /// Stored inverse of PathFollowerAcceleration (s).
    pub inv_acceleration: f32,
    pub roll_acceleration: f32,
    pub roll_look_ahead: f32,
    pub roll_tolerance: f32,
    pub roll_speed: f32,
    /// Stored inverse of PathFollowerRollAccelerationLag (s).
    pub roll_lag_inv: f32,
    pub roll_turn_factor: f32,
    pub roll_speed_factor: f32,
    pub roll_dampening: f32,
    pub min_branch_dist: f32,
    pub flyer_vs_flyer: bool,
    pub formations: u8,
}

impl Default for FollowerParams {
    fn default() -> Self {
        Self {
            class_id: 0,
            speed: 1.0,
            turn_speed: 0.5,
            inv_acceleration: 0.5,
            roll_acceleration: core::f32::consts::FRAC_PI_2,
            roll_look_ahead: 1.0,
            roll_tolerance: core::f32::consts::FRAC_PI_8,
            roll_speed: core::f32::consts::FRAC_PI_2,
            roll_lag_inv: 1000.0,
            roll_turn_factor: 5.0,
            roll_speed_factor: 1.0,
            roll_dampening: 0.5,
            min_branch_dist: 300.0,
            flyer_vs_flyer: true,
            formations: 0,
        }
    }
}

impl Default for FlyerParams {
    /// BF1 FUN_001dead8, class +1778..17e4. ODF rates/angles are radians.
    fn default() -> Self {
        Self {
            bf2: false,
            never_crash_when_unpiloted: false,
            trick_roll_speed: 1.0,
            trick_flip_speed: 1.0,
            pilot_repair_scale: 0.005,
            aim_box: 0.0,
            trick_drain: 15.0,
            trick_flip_drain: 50.0,
            trick_side_strafe: 0.0,
            has_chunks: false,
            view_fov: 48.0,
            acceleration: 5.0,
            min_speed: 10.0,
            mid_speed: 20.0,
            max_speed: 30.0,
            strafe_speed: 0.0,
            pitch_rate: 1.0,
            turn_rate: 2.0,
            pitch_filter: 10.0,
            pitch_damp: 10.0,
            turn_filter: 10.0,
            turn_damp: 10.0,
            roll_rate: 0.0,
            roll_filter: 0.0,
            thrust_pitch_angle: 0.0,
            strafe_roll_angle: 0.0,
            bank_angle: 0.5,
            bank_filter: 2.0,
            level_filter: 3.0,
            level_damp: 3.0,
            takeoff_height: 5.0,
            takeoff_time: 5.0,
            takeoff_speed: 3.0,
            landing_time: 5.0,
            landing_speed: 3.0,
            landed_height: 0.0,
            boost_speed: 0.0,
            command_flyer: false,
            boost_acceleration: 0.0,
            energy_max: 100.0,
            energy_auto_restore: 0.0,
            energy_boost_drain: 0.0,
            velocity_filter: 0.0,
            collision_threshold: [7.5, 2.5, 0.0],
            collision_scale: [1.0, 0.0, 0.0],
            follower: FollowerParams::default(),
        }
    }
}

impl FlyerParams {
    /// BF2 FUN_0020f098, class +82c..89c and +e04..e34.
    pub fn bf2() -> Self {
        Self {
            bf2: true,
            roll_rate: 3.15,
            roll_filter: 4.0,
            level_damp: 5.0,
            energy_boost_drain: 1.0,
            velocity_filter: 8.0,
            ..Self::default()
        }
    }

    /// BF2 FUN_00202c50/88/c0: these caps apply in a landing region,
    /// including a space hangar, not to every BF2 or atmospheric flyer.
    pub fn speeds(&self, in_landing_region: bool) -> [f32; 3] {
        if self.bf2 && in_landing_region {
            [
                (self.min_speed * 0.2).min(10.0),
                self.mid_speed.min(20.0),
                self.max_speed.min(60.0),
            ]
        } else {
            [self.min_speed, self.mid_speed, self.max_speed]
        }
    }

    pub fn exhaust_scale(&self, [low, high, start]: [f32; 3], speed: f32) -> f32 {
        let top = if self.max_speed != 0.0 {
            self.max_speed
        } else {
            1.0
        };
        let floor = if self.min_speed != 0.0 {
            self.min_speed + start * (top - self.min_speed)
        } else {
            start * top
        };
        let t = ((speed - floor) / (top.max(self.boost_speed) - floor)).max(0.0);
        low + t * (high - low)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum FlightPhase {
    #[default]
    Landed,
    Takeoff,
    Flying,
    Landing,
    Falling,
    Crashed,
}

pub const THREAT_SLOTS: usize = 6;
pub const THREAT_UNSEEN: u8 = 1;
pub const THREAT_FORCED: u8 = 2;
pub const THREAT_SEEN: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatObject {
    Vehicle(super::VehicleHandle),
    Station(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotThreat {
    pub object: ThreatObject,
    pub status: u8,
    pub tested_at: u64,
    pub seen_at: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotThreats {
    pub slots: [Option<PilotThreat>; THREAT_SLOTS],
    pub visible: Option<u8>,
    pub next_think: u64,
    pub thinks: u32,
}

impl PilotThreats {
    pub fn visible_object(&self) -> Option<ThreatObject> {
        self.slots[usize::from(self.visible?)].map(|t| t.object)
    }

    fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if self.thinks == 0 {
            return;
        }
        eat(&self.thinks.to_le_bytes());
        eat(&self.next_think.to_le_bytes());
        eat(&[self.visible.map_or(255, |v| v)]);
        for t in self.slots.iter().flatten() {
            match t.object {
                ThreatObject::Vehicle(h) => {
                    eat(&h.index.to_le_bytes());
                    eat(&h.gen.to_le_bytes());
                }
                ThreatObject::Station(s) => eat(&s.to_le_bytes()),
            }
            eat(&[t.status]);
            eat(&t.tested_at.to_le_bytes());
            eat(&t.seen_at.to_le_bytes());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyerState {
    pub threats: PilotThreats,
    pub phase: FlightPhase,
    pub lift: f32,
    pub strafe_speed: f32,
    pub omega: Vec3,
    pub tilt_pitch: f32,
    pub tilt_roll: f32,
    pub roll_input: f32,
    pub energy: f32,
    pub drained: bool,
    pub boosting: bool,
    pub jump_was_down: bool,
    pub level_time: f32,
    /// Defer authored energy initialization until the first takeoff, so
    /// an untouched parked flyer keeps its historical hash stream.
    pub initialized: bool,
    /// Seconds left of the countdown outside the map's boundary (`Sim::tick_flyers`,
    /// `re-boundary.md`), `None` inside. The PS2 keeps it on the crew's team member.
    pub leaving: Option<f32>,
    pub crash_timer: f32,
    pub crash_roll: f32,
    pub crash_roll_target: f32,
    pub in_roll: bool,
    pub side_roll: Vec3,
    pub strafe_vertical: f32,
    pub roll_delta: f32,
    pub pilot_maneuvers: PilotManeuvers,
    pub reversal: FlightReversal,
    pub damager: Option<crate::unit::UnitHandle>,
    /// The AI pilot's flight state (`ai::flyer_drive`). Default unless a bot
    /// is flying it, so it adds no bytes to any other flyer's hash.
    pub ai: FlyerAi,
}

/// Authored BF2 route admission (analysis/re-flyer-ai.md, issue 103).
#[derive(Clone, Debug, PartialEq)]
pub struct FlyerPath {
    pub nodes: Vec<Vec3>,
    pub name: u32,
    /// (PathFollowerClass hash, maximum concurrent users), in authored order.
    pub classes: Vec<(u32, u32)>,
    pub max_entities: u32,
    /// -1 last endpoint only, 0 either, +1 first endpoint only.
    pub direction: i8,
    pub single_direction: bool,
    pub frequency: f32,
    /// Runtime EnableFlyerPath bit; defaults on, not an authored property.
    pub enabled: bool,
    /// EnableObject resolved to a placed instance, if present on this map.
    pub enable_instance: Option<u32>,
}

impl Default for FlyerPath {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            name: 0,
            classes: Vec::new(),
            max_entities: u32::MAX,
            direction: 0,
            single_direction: false,
            frequency: 1.0,
            enabled: true,
            enable_instance: None,
        }
    }
}

/// The BF1 UnitFlyAgent states the port keeps (FUN_00246d80).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum FlyAgent {
    /// Before the first upward probe.
    #[default]
    Start,
    /// State 2: the 100 m upward ray is blocked; 200 m forward, 1 m up.
    Escape,
    /// State 4: to a point over a command post.
    Travel,
}

/// A BF2 follower's place on an authored path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathCursor {
    pub path: u16,
    /// The node the curve's P2 stands on.
    pub node: u16,
    pub reverse: bool,
}

/// What an AI pilot keeps between ticks. BF1 fields drive the sticks; BF2
/// fields drive [`spline_step`], which writes the pose as the PS2 does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlyerAi {
    pub trick: Option<Vec3>,
    pub channel: u8,
    pub armed_target: Option<super::VehicleHandle>,
    pub armed_station: Option<u16>,
    pub threat_thinks: u32,
    pub heavy_claim: Option<(super::VehicleHandle, u64)>,
    pub agent: FlyAgent,
    /// BF1 travel goal; BF2 temporary loiter center (flyer_drive::loiter).
    pub goal: Option<Vec3>,
    /// FUN_001dcbf0's cached obstacle goal and where it was made.
    pub avoid: Option<[Vec3; 2]>,
    /// The next tick FUN_00235c08 may probe (0.1 s cadence); the cached goal
    /// expires 1 s after this.
    pub probe_at: u64,
    /// OURS: an attack run is broken off until this tick.
    pub break_until: u64,
    /// OURS: the point an attack run flies at, and the tick its target was last seen.
    pub run: Option<Vec3>,
    pub run_seen: u64,
    /// OURS: the tick a takeoff began, to give up under a roof.
    pub takeoff_at: u64,
    /// OURS: where the craft last moved from, and the tick it did.
    pub anchor: Vec3,
    pub anchor_at: u64,
    /// OURS: a wedged craft keeps its way out until this tick, even under an
    /// open sky, where the upward probe would drop it at once.
    pub wedged_until: u64,
    /// OURS: ways out tried since it last travelled.
    pub wedges: u8,
    /// The Catmull-Rom P0..P3 of the current BF2 segment.
    pub curve: Option<[Vec3; 4]>,
    pub t: f32,
    /// Follower +0x60; 0 until the first update, which starts it at 1.
    pub factor: f32,
    pub cursor: Option<PathCursor>,
    /// A pursuit curve (flag +0xfd: speed x 0.8).
    pub pursuit: bool,
    /// Follower +0xf8: the 3.5 s after a pursuit (speed x 1.5).
    pub escape: f32,
    pub heading: Option<f32>,
    /// FUN_002feed0: +0x6c filtered goal, +0x70 angle, +0x74 acceleration,
    /// +0x78 rate.
    pub roll: [f32; 4],
    pub relocate_in: f32,
    pub burst_in: f32,
    pub fire_held: bool,
    /// A hangar's approach target and the earliest departure after unloading.
    pub transport_goal: Option<Vec3>,
    pub transport_returning: bool,
    pub transport_departing: bool,
    pub transport_depart_at: Option<u64>,
}

impl FlyerAi {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if *self == Self::default() {
            return;
        }
        eat(b"FAI");
        if self.transport_returning || self.transport_departing {
            eat(&[
                b'T',
                b'R',
                u8::from(self.transport_returning),
                u8::from(self.transport_departing),
            ]);
        }
        if let Some(at) = self.transport_depart_at {
            eat(b"TD");
            eat(&at.to_le_bytes());
        }
        eat(&[self.agent as u8, self.pursuit as u8, self.wedges]);
        if self.relocate_in != 0.0 {
            eat(&self.relocate_in.to_bits().to_le_bytes());
        }
        if self.burst_in != 0.0 {
            eat(&self.burst_in.to_bits().to_le_bytes());
        }
        if self.fire_held {
            eat(&[1]);
        }
        if self.channel != 0 {
            eat(&[self.channel]);
        }
        if let Some(t) = self.armed_target {
            eat(&t.index.to_le_bytes());
            eat(&t.gen.to_le_bytes());
        }
        if let Some(s) = self.armed_station {
            eat(&s.to_le_bytes());
        }
        if self.threat_thinks != 0 {
            eat(&self.threat_thinks.to_le_bytes());
        }
        if let Some((t, until)) = self.heavy_claim {
            eat(&t.index.to_le_bytes());
            eat(&t.gen.to_le_bytes());
            eat(&until.to_le_bytes());
        }
        for x in self.anchor.to_array() {
            eat(&x.to_bits().to_le_bytes());
        }
        for tick in [
            self.probe_at,
            self.break_until,
            self.run_seen,
            self.takeoff_at,
            self.anchor_at,
            self.wedged_until,
        ] {
            eat(&tick.to_le_bytes());
        }
        let vec = |v: Option<Vec3>, eat: &mut dyn FnMut(&[u8])| match v {
            Some(v) => {
                eat(&[1]);
                for x in v.to_array() {
                    eat(&x.to_bits().to_le_bytes());
                }
            }
            None => eat(&[0]),
        };
        if self.transport_goal.is_some() {
            eat(b"TG");
            vec(self.transport_goal, eat);
        }
        vec(self.trick, eat);
        vec(self.goal, eat);
        vec(self.run, eat);
        for k in 0..2 {
            vec(self.avoid.map(|a| a[k]), eat);
        }
        for k in 0..4 {
            vec(self.curve.map(|c| c[k]), eat);
        }
        vec(self.heading.map(Vec3::splat), eat);
        if let Some(c) = self.cursor {
            eat(&c.path.to_le_bytes());
            eat(&c.node.to_le_bytes());
            eat(&[c.reverse as u8]);
        }
        for x in [self.t, self.factor, self.escape] {
            eat(&x.to_bits().to_le_bytes());
        }
        for x in self.roll {
            eat(&x.to_bits().to_le_bytes());
        }
    }
}

impl Default for FlyerState {
    fn default() -> Self {
        Self {
            phase: FlightPhase::Landed,
            lift: 0.0,
            strafe_speed: 0.0,
            omega: Vec3::ZERO,
            tilt_pitch: 0.0,
            tilt_roll: 0.0,
            roll_input: 0.0,
            energy: 100.0,
            drained: false,
            boosting: false,
            jump_was_down: false,
            level_time: 0.0,
            initialized: false,
            leaving: None,
            crash_timer: 0.0,
            crash_roll: 0.0,
            crash_roll_target: 0.0,
            threats: PilotThreats::default(),
            in_roll: false,
            side_roll: Vec3::ZERO,
            strafe_vertical: 0.0,
            roll_delta: 0.0,
            pilot_maneuvers: PilotManeuvers::default(),
            reversal: FlightReversal::default(),
            damager: None,
            ai: FlyerAi::default(),
        }
    }
}

impl FlyerState {
    pub fn active_maneuver(&self) -> Option<ManeuverKind> {
        if self.phase != FlightPhase::Flying {
            None
        } else if self.reversal.active() {
            Some(ManeuverKind::Reversal)
        } else if self.side_roll != Vec3::ZERO {
            Some(ManeuverKind::SideRoll)
        } else {
            None
        }
    }

    /// A pad's untouched flyer adds no bytes to the old ground-map hash.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if *self == Self::default() {
            return;
        }
        eat(b"FLY");
        eat(&[
            self.phase as u8,
            self.drained as u8,
            self.boosting as u8,
            self.jump_was_down as u8,
            self.initialized as u8,
        ]);
        for value in [
            self.lift,
            self.strafe_speed,
            self.omega.x,
            self.omega.y,
            self.omega.z,
            self.tilt_pitch,
            self.tilt_roll,
            self.roll_input,
            self.energy,
            self.level_time,
        ] {
            eat(&value.to_bits().to_le_bytes());
        }
        if let Some(t) = self.leaving {
            eat(&t.to_bits().to_le_bytes());
        }
        if self.phase == FlightPhase::Falling || self.phase == FlightPhase::Crashed {
            for value in [self.crash_timer, self.crash_roll, self.crash_roll_target] {
                eat(&value.to_bits().to_le_bytes());
            }
        }
        if self.phase == FlightPhase::Flying
            && (self.in_roll || self.crash_roll != 0.0 || self.strafe_vertical != 0.0)
        {
            eat(&[self.in_roll as u8]);
            for value in [
                self.crash_roll,
                self.crash_roll_target,
                self.side_roll.x,
                self.side_roll.y,
                self.side_roll.z,
                self.strafe_vertical,
                self.roll_delta,
            ] {
                eat(&value.to_bits().to_le_bytes());
            }
        }
        if let Some(d) = self.damager {
            eat(&d.index.to_le_bytes());
            eat(&d.gen.to_le_bytes());
        }
        self.ai.hash_into(eat);
        self.threats.hash_into(eat);
        self.pilot_maneuvers.hash_into(eat);
        self.reversal.hash_into(eat);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LandingSurface {
    pub height: f32,
    pub normal: Vec3,
    /// False for the PS2's excluded moving/flagged landing surfaces.
    pub can_land: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct FlyerEnvironment {
    /// A ray down 1024 m, ignoring the flyer's own body. No hit is a
    /// 1024 m clearance; it does not invent a floor at world y=0.
    pub ground: Option<LandingSurface>,
    pub min_height: f32,
    pub max_height: f32,
    pub in_landing_region: bool,
    pub almost_inside_landing_region: bool,
    /// BF2 maps which register landing regions permit the toggle only in
    /// one. Maps without any registered region retain unrestricted landing.
    pub require_landing_region: bool,
    pub player_aboard: bool,
    pub crash_random: [f32; 4],
}

impl FlyerEnvironment {
    pub(crate) fn permits_bf2_landing(self) -> bool {
        !self.require_landing_region || self.almost_inside_landing_region
    }
}

impl Default for FlyerEnvironment {
    fn default() -> Self {
        Self {
            ground: None,
            min_height: -200.0,
            max_height: 200.0,
            in_landing_region: false,
            almost_inside_landing_region: false,
            player_aboard: false,
            crash_random: [0.0; 4],
            require_landing_region: false,
        }
    }
}

/// The flight frame under the visual tilt: what the pilot's aim follows
/// (B1 FUN_001dbbd8 stores its forward at entity +0x3b0, the aim FUN_00232b78
/// reads; analysis/re-flyer.md).
pub fn untilted(v: &Vehicle) -> Quat {
    v.rot * quat_x(-v.flyer.tilt_pitch) * quat_z(-v.flyer.tilt_roll)
}

fn approach(current: f32, target: f32, delta: f32) -> f32 {
    current + (target - current).clamp(-delta.max(0.0), delta.max(0.0))
}

fn filtered(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    current + (target - current) * (rate * dt).clamp(0.0, 1.0)
}

fn angular_slew(current: f32, target: f32, filter: f32, decel: f32, dt: f32) -> f32 {
    let accelerating = current == 0.0 || (target * current >= 0.0 && target.abs() > current.abs());
    approach(
        current,
        target,
        if accelerating { filter } else { decel } * dt,
    )
}

/// OURS: the least clearance `FlightPhase::Landing` touches down under. The
/// contact sweep leaves a craft's origin 1 cm off the surface it stops at.
pub const TOUCHDOWN_MIN: f32 = 0.02;
/// OURS: how far over a pad's floor to stand an origin whose class says less
/// than [`REST_FLOOR`].
pub const REST_LIFT: f32 = 0.02;
pub const REST_FLOOR: f32 = 0.005;

/// How far over the pad's floor a flyer's origin stands. `LandedHeight` is the
/// PS2's number, and a model whose vis box starts over its origin makes it
/// negative: BF1's TIE bomber is -0.0115. The PS2's clearance ray starts at the
/// body's sphere centre (`re-flyer.md`, B1:85703), but the port's rays start at
/// the origin, so an origin under the deck sees no deck below it and takes the
/// deck over it for a ceiling: the craft never left the deck's underside.
pub fn rest_height(landed_height: f32) -> f32 {
    if landed_height < REST_FLOOR {
        REST_LIFT
    } else {
        landed_height
    }
}

pub const ABANDONED_CRASH_TIME: f32 = 0.5;
const SIDE_ROLL_ANGLE: f32 = std::f32::consts::TAU * 1.1;
const SLOW_STRAFE_STEP: f32 = 1.0 / 30.0;
const CRASH_VELOCITY_SPIN: f32 = 0.015625;

fn restore_energy(s: &mut FlyerState, p: &FlyerParams, dt: f32) {
    if s.initialized {
        s.energy = (s.energy + p.energy_auto_restore * dt).clamp(0.0, p.energy_max);
        if s.energy >= 20.0_f32.min(p.energy_max) {
            s.drained = false;
        }
    }
}

fn crash_spin(s: &mut FlyerState, p: &FlyerParams, sign: f32, dt: f32) {
    use std::f32::consts::{PI, TAU};
    if s.crash_roll_target == 0.0 {
        s.crash_roll_target = if sign < 0.5 { 2.0 * TAU } else { -2.0 * TAU };
    }
    if s.crash_roll_target > 0.0 {
        s.crash_roll += p.trick_roll_speed * dt;
        if s.crash_roll > PI {
            s.crash_roll -= TAU;
            s.tilt_roll -= TAU;
        }
    } else {
        s.crash_roll -= p.trick_roll_speed * dt;
        if s.crash_roll < -PI {
            s.crash_roll += TAU;
            s.tilt_roll += TAU;
        }
    }
}

pub fn side_roll(s: &mut FlyerState, p: &FlyerParams, side: Vec3) {
    if p.trick_drain == 0.0 || s.in_roll || s.reversal.active() || s.drained {
        s.side_roll = Vec3::ZERO;
        return;
    }
    s.energy -= p.trick_drain;
    if s.energy < 0.0 {
        s.energy = 0.0;
        s.drained = true;
    }
    s.side_roll = side;
    s.crash_roll_target += if side.x < 0.0 {
        -SIDE_ROLL_ANGLE
    } else {
        SIDE_ROLL_ANGLE
    };
    s.in_roll = true;
    s.ai.curve = None;
    s.ai.cursor = None;
}

fn roll_stop(s: &mut FlyerState) {
    use std::f32::consts::{PI, TAU};
    s.crash_roll = 0.0;
    s.crash_roll_target = 0.0;
    while s.tilt_roll > PI {
        s.tilt_roll -= TAU;
    }
    while s.tilt_roll < -PI {
        s.tilt_roll += TAU;
    }
    s.in_roll = false;
}

fn roll_update(s: &mut FlyerState, p: &FlyerParams, speed: f32, dt: f32) {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};
    if s.side_roll != Vec3::ZERO && s.crash_roll == s.crash_roll_target && s.roll_delta.abs() < 0.8
    {
        s.side_roll = Vec3::ZERO;
    }
    if s.crash_roll != s.crash_roll_target {
        s.in_roll = true;
        if s.crash_roll_target >= s.crash_roll {
            s.crash_roll += p.trick_roll_speed * dt;
            if s.crash_roll > PI {
                s.crash_roll -= TAU;
                s.crash_roll_target -= TAU;
                s.tilt_roll -= TAU;
            }
            if s.crash_roll < s.crash_roll_target {
                return;
            }
        } else {
            s.crash_roll -= p.trick_roll_speed * dt;
            if s.crash_roll < -PI {
                s.crash_roll += TAU;
                s.crash_roll_target += TAU;
                s.tilt_roll += TAU;
            }
            if s.crash_roll > s.crash_roll_target {
                return;
            }
        }
        if s.crash_roll_target.abs() < 0.001 {
            roll_stop(s);
        } else {
            s.crash_roll = s.crash_roll_target;
        }
        return;
    }
    if s.roll_delta.abs() < 0.25 {
        if maneuver::continue_upside_down_roll(s, p, speed) {
            return;
        }
        if s.crash_roll_target.abs() <= FRAC_PI_2 {
            s.crash_roll_target = 0.0;
            if s.crash_roll == 0.0 && s.in_roll {
                roll_stop(s);
            }
        } else {
            s.in_roll = false;
        }
    }
}

fn side_strafe(s: &mut FlyerState, p: &FlyerParams) {
    let x_step = if s.side_roll.x == 0.0 {
        SLOW_STRAFE_STEP
    } else {
        1.0
    };
    let y_step = if s.side_roll.y == 0.0 {
        SLOW_STRAFE_STEP
    } else {
        1.0
    };
    let toward = |now: f32, want: f32, down: f32, up: f32| want.min(now + up).max(now - down);
    let a = p.acceleration;
    s.strafe_speed = toward(
        s.strafe_speed,
        s.side_roll.x * p.trick_side_strafe,
        a * x_step,
        a * x_step,
    );
    s.strafe_vertical = toward(
        s.strafe_vertical,
        s.side_roll.y * p.trick_side_strafe,
        a * y_step,
        a * x_step,
    );
}

/// State machine and free-flight integration. `v.input` uses the same
/// pad convention as hover: pitch and turn write negative local x/y rates.
/// The contact response runs separately; a sweep is not original hull parity.
pub fn step(v: &mut Vehicle, p: &FlyerParams, env: FlyerEnvironment, dt: f32) {
    if !v.alive || dt <= 0.0 {
        return;
    }
    if v.flyer.phase == FlightPhase::Crashed {
        return;
    }
    let occupied = !v.is_empty();
    let has_pilot = v.has_pilot();
    if p.bf2
        && !p.never_crash_when_unpiloted
        && matches!(v.flyer.phase, FlightPhase::Takeoff | FlightPhase::Flying)
        && (!occupied || (!has_pilot && !env.player_aboard))
    {
        v.health.cur = 0.0;
        v.flyer.phase = FlightPhase::Falling;
        v.flyer.crash_timer = ABANDONED_CRASH_TIME;
    }
    fly(
        Flight {
            pos: &mut v.pos,
            vel: &mut v.vel,
            rot: &mut v.rot,
            state: &mut v.flyer,
            input: v.input,
            occupied,
        },
        p,
        env,
        dt,
    );
}

/// A flyer body the kernel moves: a vehicle's, or a guided rocket's, which the
/// PS2 builds as a flyer entity too (`analysis/re-guided-rockets.md`).
pub struct Flight<'a> {
    pub pos: &'a mut Vec3,
    pub vel: &'a mut Vec3,
    pub rot: &'a mut Quat,
    pub state: &'a mut FlyerState,
    pub input: HoverInput,
    pub occupied: bool,
}

/// One tick of [`step`]'s motion after its abandoned-craft check.
pub fn fly(f: Flight, p: &FlyerParams, env: FlyerEnvironment, dt: f32) {
    let Flight {
        pos,
        vel,
        rot,
        state: s,
        mut input,
        occupied,
    } = f;
    let toggle = input.jump && !s.jump_was_down;
    s.jump_was_down = input.jump;
    let mut frame = *rot * quat_x(-s.tilt_pitch) * quat_z(-s.tilt_roll);
    let clearance = env.ground.map_or(1024.0, |g| pos.y - g.height);
    let normal = env.ground.map_or(Vec3::Y, |g| g.normal);
    let current_speed = vel.dot(*rot * Vec3::Z);
    // BF2's energy helper permits the tick which drains past zero, then
    // recovers at 20 (or the smaller authored maximum).
    // The original blocks recovery during stunts, not ordinary boost.
    if p.bf2 && !s.in_roll && !s.reversal.active() && current_speed <= p.max_speed + 0.0001 {
        restore_energy(s, p, dt);
    }

    if !p.bf2 {
        if matches!(s.phase, FlightPhase::Takeoff | FlightPhase::Flying) && !occupied {
            s.phase = FlightPhase::Falling;
        } else if s.phase == FlightPhase::Falling && occupied {
            s.phase = FlightPhase::Takeoff;
        }
    }
    match s.phase {
        FlightPhase::Landed => {
            s.lift = 0.0;
            if toggle && occupied {
                s.phase = FlightPhase::Takeoff;
                if !s.initialized {
                    s.energy = p.energy_max;
                    if p.bf2 {
                        s.level_time = 1.0;
                    }
                    s.initialized = true;
                }
            }
        }
        FlightPhase::Takeoff => {
            if toggle && s.lift > 0.75 {
                s.phase = FlightPhase::Landing;
            } else if clearance > p.landed_height + p.takeoff_height {
                s.lift = (s.lift + dt / p.takeoff_time.max(dt)).min(1.0);
                if s.lift >= 1.0 {
                    s.phase = FlightPhase::Flying;
                    if p.bf2 {
                        s.roll_input = 0.0;
                    }
                }
            }
        }
        FlightPhase::Flying => {
            s.lift = 1.0;
            if toggle && (!p.bf2 || env.permits_bf2_landing()) {
                s.phase = FlightPhase::Landing;
            }
        }
        FlightPhase::Landing => {
            let cancel_at = if p.bf2 { 0.05 } else { 0.25 };
            if toggle && s.lift < cancel_at {
                s.phase = FlightPhase::Takeoff;
            } else {
                s.lift = (s.lift - dt / p.landing_time.max(dt)).max(0.0);
                // OURS: the sweep stops an origin 1 cm off the surface, so a class
                // whose number is smaller than that must still be able to touch down.
                if clearance < p.landed_height.max(TOUCHDOWN_MIN)
                    && (frame * Vec3::Y).dot(normal) > 0.9375
                {
                    if env.ground.is_some_and(|g| g.can_land) {
                        s.phase = FlightPhase::Landed;
                    } else {
                        s.phase = FlightPhase::Takeoff;
                    }
                }
            }
        }
        FlightPhase::Falling => s.lift = (s.lift - dt * 0.2).max(0.0),
        FlightPhase::Crashed => {}
    }
    if !input.human || !p.bf2 {
        s.pilot_maneuvers = PilotManeuvers::default();
    }
    if s.phase != FlightPhase::Flying {
        s.reversal = FlightReversal::default();
    }
    if let Some(request) = s.pilot_maneuvers.take_request(
        input,
        input.human
            && p.bf2
            && s.phase == FlightPhase::Flying
            && !env.in_landing_region
            && p.trick_drain > 0.0
            && p.trick_roll_speed > 0.0
            && !s.drained,
        s.in_roll || s.reversal.active(),
        p.trick_flip_drain > 0.0 && p.trick_flip_speed > 0.0,
        dt,
    ) {
        maneuver::start(s, p, request, current_speed);
        input.pitch = 0.0;
        input.turn = 0.0;
        input.strafe = 0.0;
    }
    if s.phase == FlightPhase::Landed {
        *vel = Vec3::ZERO;
        s.omega = Vec3::ZERO;
        if s.tilt_pitch != 0.0 || s.tilt_roll != 0.0 {
            *rot = frame;
        }
        s.tilt_pitch = 0.0;
        s.tilt_roll = 0.0;
        s.boosting = false;
        return;
    }

    let [min, mid, max] = p.speeds(env.in_landing_region);
    let throttle = input.throttle.clamp(-1.0, 1.0);
    let target_speed = if throttle < 0.0 {
        mid - throttle * (min - mid)
    } else {
        mid + throttle * (max - mid)
    };
    let boost_requested = p.bf2
        && p.boost_speed > 0.0
        && s.phase == FlightPhase::Flying
        && (input.boost || s.boosting)
        && throttle >= 0.2
        && current_speed >= p.min_speed * 0.3;
    s.boosting = boost_requested && (!s.drained || p.energy_boost_drain <= 0.0);
    if s.boosting {
        s.energy -= p.energy_boost_drain * dt;
        if s.energy < 0.0 {
            s.energy = 0.0;
            s.drained = true;
        }
    }
    let acceleration = if s.boosting || (p.bf2 && current_speed > max) {
        p.boost_acceleration
    } else {
        p.acceleration
    };
    let target_speed = if s.boosting {
        p.boost_speed
    } else {
        target_speed
    };
    let speed = if target_speed < current_speed && p.bf2 && env.in_landing_region {
        approach(current_speed, target_speed, acceleration * dt * 3.0)
    } else {
        approach(current_speed, target_speed, acceleration * dt)
    };
    if !p.bf2 {
        s.strafe_speed = approach(
            s.strafe_speed,
            input.strafe.clamp(-1.0, 1.0) * p.strafe_speed,
            p.acceleration * dt,
        );
    }

    if p.bf2 {
        match s.phase {
            FlightPhase::Flying => {
                roll_update(s, p, current_speed, dt);
                maneuver::advance_reversal(s, p, &mut frame, dt);
                side_strafe(s, p);
            }
            FlightPhase::Falling | FlightPhase::Crashed => {}
            _ => {
                roll_stop(s);
                s.side_roll = Vec3::ZERO;
            }
        }
    }
    if p.bf2 && s.phase == FlightPhase::Flying {
        let roll = if env.in_landing_region || s.reversal.active() || input.strafe.abs() < 0.3 {
            0.0
        } else {
            (input.strafe - 0.3 * input.strafe.signum()) / 0.7
        };
        s.roll_input = filtered(s.roll_input, roll, p.roll_filter, dt);
        frame *= quat_z(s.roll_input * p.roll_rate * dt);
        if roll != 0.0 {
            s.level_time = 0.0;
        } else if s.level_time != 0.0 && s.level_time < 0.8 && !s.in_roll {
            s.level_time = if input.pitch.abs() > 0.1 || input.turn.abs() > 0.1 {
                0.01
            } else {
                (s.level_time + dt).min(1.0)
            };
        }
        if env.in_landing_region {
            s.level_time = 0.8;
        }
    }
    let forward = frame * Vec3::Z;
    let right = frame * Vec3::X;
    let up = frame * Vec3::Y;
    let mut pitch = input.pitch.clamp(-1.0, 1.0);
    // Both updates bend the pitch back before the authored height band.
    if s.phase == FlightPhase::Flying && vel.y != 0.0 && input.human {
        let (height, away) = if vel.y < 0.0 {
            (env.min_height, Vec3::Y)
        } else {
            (env.max_height, Vec3::NEG_Y)
        };
        let proximity = ((height - pos.y) * 0.5 / vel.y).clamp(0.0, 1.0);
        let toward = away.dot(forward);
        if proximity < 1.0 && toward * vel.dot(forward) <= 0.0 {
            let correction = if away.dot(up) > 0.0 { -toward } else { toward };
            let correction =
                (correction / (p.pitch_rate * dt).max(f32::MIN_POSITIVE)).clamp(-1.0, 1.0);
            pitch = correction + (pitch - correction) * proximity * proximity;
        }
    }
    let mut pitch_rate = p.pitch_rate;
    let mut turn_rate = p.turn_rate;
    if p.bf2 {
        // BF2 blends the smaller rate into each full-axis rate over .7..1.
        let shared = pitch_rate.min(turn_rate);
        pitch_rate = shared + (pitch_rate - shared) * ((pitch.abs() - 0.7) / 0.3).clamp(0.0, 1.0);
        turn_rate =
            shared + (turn_rate - shared) * ((input.turn.abs() - 0.7) / 0.3).clamp(0.0, 1.0);
        if p.boost_speed > max && current_speed > max {
            let boost = ((current_speed - max) / (p.boost_speed - max)).clamp(0.0, 1.0);
            pitch_rate *= 1.0 - 0.3 * boost;
            turn_rate *= 1.0 - 0.3 * boost;
        }
    }
    let pitch_target = -pitch * pitch_rate;
    let turn_target = -input.turn.clamp(-1.0, 1.0) * turn_rate;
    let cruise_velocity = forward * speed - right * s.strafe_speed;
    match s.phase {
        FlightPhase::Flying => {
            if p.bf2 {
                s.omega.x = angular_slew(s.omega.x, pitch_target, p.pitch_filter, p.pitch_damp, dt);
                s.omega.y = angular_slew(s.omega.y, turn_target, p.turn_filter, p.turn_damp, dt);
                s.omega.z = if s.level_time >= 0.8 && !s.in_roll && !s.reversal.active() {
                    s.roll_input = 0.0;
                    -p.level_filter * right.y
                } else {
                    0.0
                };
                let blend = (p.velocity_filter * dt).clamp(0.0, 1.0);
                let target =
                    *rot * Vec3::Z * speed - right * s.strafe_speed - up * s.strafe_vertical;
                *vel = *vel * (1.0 - blend) + target * blend;
                let forward_speed = vel.dot(*rot * Vec3::Z);
                if forward_speed > 0.0 && forward_speed != speed {
                    *vel *= speed / forward_speed;
                }
            } else {
                s.omega.x = filtered(s.omega.x, pitch_target, p.pitch_filter, dt);
                s.omega.y = filtered(s.omega.y, turn_target, p.turn_filter, dt);
                s.omega.z = -p.level_filter * right.y;
                *vel = cruise_velocity;
            }
        }
        FlightPhase::Takeoff | FlightPhase::Landing => {
            let height_factor = (clearance / p.takeoff_height.max(f32::MIN_POSITIVE)).min(1.0);
            let level = if p.bf2 { p.level_damp } else { p.level_filter };
            let pitch_level = s.omega.x * (1.0 - level * (1.0 - s.lift) * dt).max(0.0)
                + level * forward.dot(normal) * 0.5 * (1.0 - s.lift);
            s.omega.x = filtered(pitch_level, pitch_target * s.lift, p.pitch_filter, dt);
            s.omega.y = filtered(s.omega.y, turn_target * height_factor, p.turn_filter, dt);
            s.omega.z = -level * right.dot(normal);
            let vertical = if s.phase == FlightPhase::Takeoff {
                p.takeoff_speed
            } else {
                -p.landing_speed
            };
            let vertical = Vec3::Y * vertical * (height_factor * 0.875 + 0.125);
            *vel = vertical + (cruise_velocity - vertical) * s.lift;
        }
        FlightPhase::Falling if p.bf2 => {
            crash_spin(s, p, env.crash_random[3], dt);
            let free = 1.0 - s.lift;
            if vel.x * vel.x + vel.z * vel.z > 1.0 {
                s.omega.x -= p.level_filter * up.dot(vel.normalize()) * free;
                let blend = (p.pitch_filter * s.lift * dt).min(1.0);
                s.omega.x += (pitch_target * 0.5 - s.omega.x) * blend;
            }
            let blend = (p.turn_filter * s.lift * dt).min(1.0);
            s.omega.y += (turn_target * 0.5 - s.omega.y) * blend;
            let blend = (p.level_filter * (s.lift * 0.75 + 0.25) * dt).min(1.0);
            *vel += (cruise_velocity - *vel) * blend;
            vel.y -= 9.8 * free * dt;
            s.omega += forward.cross(*vel) * CRASH_VELOCITY_SPIN * free * dt;
            let [a, b, c, _] = env.crash_random;
            s.omega.x += (a - 0.5) * 0.25 + forward.y * 0.0625 + 0.0625;
            s.omega.y += (b - 0.5) * 0.25;
            s.omega.z += (c - 0.5) * 0.5;
            s.crash_timer -= dt;
            if s.crash_timer < 0.0 {
                s.phase = FlightPhase::Crashed;
            }
        }
        FlightPhase::Falling => {
            // The original's uncontrolled fall also adds global-RNG tumble
            // and class-dependent death timers. Those are not transcribed;
            // this deterministic gravity/velocity component is a scoped slice.
            let response = p.level_filter * (s.lift * 0.75 + 0.25);
            *vel += (cruise_velocity - *vel) * (response * dt).clamp(0.0, 1.0);
            vel.y -= 9.8 * (1.0 - s.lift) * dt;
            s.omega *= (1.0 - p.level_filter * dt * 0.25).max(0.0);
        }
        FlightPhase::Landed | FlightPhase::Crashed => unreachable!(),
    }
    let local_velocity = frame.conjugate() * *vel;
    // BF1 uses MaxSpeed; BF2 uses max(MaxSpeed, BoostSpeed) even outside
    // boost. Takeoff alone also divides by TurnRate (B1:85954;
    // B2:117113,118231,118641,118758).
    let bank_speed = if p.bf2 {
        p.max_speed.max(p.boost_speed)
    } else {
        p.max_speed
    };
    let bank_divisor = if s.phase == FlightPhase::Takeoff {
        bank_speed * p.turn_rate
    } else {
        bank_speed
    };
    let bank_acceleration = s.omega.cross(local_velocity) / bank_divisor.max(f32::MIN_POSITIVE);
    let mut pitch_tilt =
        throttle * p.thrust_pitch_angle * s.lift + bank_acceleration.z * p.bank_angle;
    if s.reversal.active() {
        pitch_tilt += s.reversal.pitch();
    }
    let rolling = s.phase == FlightPhase::Flying && (s.in_roll || s.crash_roll != 0.0);
    let roll_tilt =
        if p.bf2 && (rolling || matches!(s.phase, FlightPhase::Falling | FlightPhase::Crashed)) {
            s.crash_roll - bank_acceleration.x * p.bank_angle
        } else {
            input.strafe * p.strafe_roll_angle * s.lift - bank_acceleration.x * p.bank_angle
        };
    s.tilt_pitch += (pitch_tilt - s.tilt_pitch) * p.bank_filter * dt;
    s.tilt_roll += (roll_tilt - s.tilt_roll) * p.bank_filter * dt;
    s.roll_delta = roll_tilt - s.tilt_roll;
    maneuver::finish_reversal(s, pitch_tilt - s.tilt_pitch);
    let angular_speed = s.omega.length();
    if angular_speed > 0.0001 {
        let (sin, cos) = fmath::sin_cos(angular_speed * dt * 0.5);
        let axis = s.omega * (sin / angular_speed);
        frame *= Quat::from_xyzw(axis.x, axis.y, axis.z, cos);
    }
    *pos += *vel * dt;
    if p.bf2 {
        // Not `clamp`: a mission may set one end past the other's default,
        // and PS2's two tests do not panic. Its upper test goes first.
        pos.y = pos.y.max(env.min_height - 0.2).min(env.max_height + 0.2);
    }
    *rot = (frame * quat_z(s.tilt_roll) * quat_x(s.tilt_pitch)).normalize();
}

/// The uniform Catmull-Rom segment P1..P2 (FUN_00539970 / FUN_00539b60):
/// position and tangent at `t`.
pub fn curve_at(p: &[Vec3; 4], t: f32) -> (Vec3, Vec3) {
    let a = p[0] * -0.5 + p[1] * 1.5 - p[2] * 1.5 + p[3] * 0.5;
    let b = p[0] - p[1] * 2.5 + p[2] * 2.0 - p[3] * 0.5;
    let c = (p[2] - p[0]) * 0.5;
    (
        ((a * t + b) * t + c) * t + p[1],
        (a * 3.0 * t + b * 2.0) * t + c,
    )
}

const WOBBLE_BF2: f32 = 2.0;
const WOBBLE_GROUND_MAP: f32 = 1.0;

/// FUN_002feed0's roll. `rate` is the heading's rate, negative to the right in
/// the port's frame, so the goal's sign banks into the turn. `phase` stands
/// in for the follower's address bits, which have no portable value.
fn roll_step(
    s: &mut [f32; 4],
    f: &FollowerParams,
    input: (f32, f32),
    phase: f32,
    wobble: f32,
    dt: f32,
) {
    let (rate, factor) = input;
    let half_pi = core::f32::consts::FRAC_PI_2;
    let goal = (-rate * factor * f.roll_speed_factor).clamp(-half_pi, half_pi)
        + wobble
            * fmath::sin_cos(phase * 1.17).0
            * fmath::sin_cos(phase * 2.33).1
            * fmath::sin_cos(phase * 0.43).0;
    s[0] += (goal - s[0]) * (f.roll_turn_factor * dt).clamp(0.0, 1.0);
    let error = s[0] - s[1];
    let tolerance = f.roll_tolerance * (s[3].abs() / f.roll_speed.max(f32::MIN_POSITIVE) + 1.0);
    let target = if error.abs() < tolerance {
        -f.roll_acceleration * if s[3] < 0.0 { -1.0 } else { 1.0 }
    } else {
        f.roll_acceleration * if error < 0.0 { -1.0 } else { 1.0 }
    };
    s[2] += (target - s[2]) * (f.roll_lag_inv * dt).clamp(0.0, 1.0);
    s[3] = (s[3] + s[2] * dt).clamp(-f.roll_speed, f.roll_speed);
    s[1] += s[3] * dt;
    s[1] += (goal - s[1]) * (f.roll_dampening * dt).clamp(0.0, 1.0);
}

/// One tick of BF2's ordinary AI flight (FUN_002fccd0, called from the flying
/// state): evaluate the old parameter, advance, write the pose and velocity.
/// `v.flyer.ai.curve` must be set. The caller plans the next curve when `t`
/// reaches 1; `t` holds there meanwhile. UNCONFIRMED: the base speed is taken
/// as MaxSpeed x PathFollowerSpeed (the argument's source is not transcribed);
/// boost, avoidance and docking corrections are not built.
pub fn spline_step(v: &mut Vehicle, p: &FlyerParams, ground_map: bool, time: f32, dt: f32) {
    let f = p.follower;
    let tick_phase = (v.gen & 0xf) as f32;
    let current_speed = v.vel.dot(v.rot * Vec3::Z);
    let s = &mut v.flyer;
    if current_speed <= p.max_speed + 0.0001 {
        restore_energy(s, p, dt);
    }
    s.strafe_speed = 0.0;
    s.strafe_vertical = 0.0;
    s.jump_was_down = v.input.jump;
    s.lift = 1.0;
    s.omega = Vec3::ZERO;
    s.tilt_pitch = 0.0;
    s.tilt_roll = 0.0;
    let ai = &mut s.ai;
    let Some(curve) = ai.curve else { return };
    if ai.factor == 0.0 {
        ai.factor = 1.0;
    }
    let (position, tangent) = curve_at(&curve, ai.t);
    let mut base = p.max_speed * f.speed;
    if ai.pursuit {
        base *= 0.8;
    }
    if ai.escape > 0.0 {
        base *= 1.5;
        ai.escape -= dt;
    }
    let length = tangent.length();
    let rate = if length == 0.0 {
        1.0
    } else {
        (base * ai.factor / length).min(1.05)
    };
    let (_, ahead) = curve_at(&curve, (ai.t + rate * f.roll_look_ahead).clamp(0.0, 1.0));
    ai.t = (ai.t + rate * dt).min(1.0);
    let heading = fmath::atan2(ahead.x, ahead.z);
    let mut turn = heading - ai.heading.unwrap_or(heading);
    let pi = core::f32::consts::PI;
    if turn > pi {
        turn -= 2.0 * pi;
    } else if turn < -pi {
        turn += 2.0 * pi;
    }
    ai.heading = Some(heading);
    let heading_rate = turn / dt;
    let wobble = if ground_map {
        WOBBLE_GROUND_MAP
    } else {
        WOBBLE_BF2
    };
    roll_step(
        &mut ai.roll,
        &f,
        (heading_rate, ai.factor),
        time + tick_phase,
        wobble,
        dt,
    );
    // Step 5: next tick's speed factor.
    let fraction = (heading_rate / pi).abs().clamp(0.0, 1.0);
    let wanted = 1.0 + (f.turn_speed - 1.0) * fraction;
    ai.factor += (wanted - ai.factor) * (f.inv_acceleration * dt).clamp(0.0, 1.0);
    // Step 4: a jump over 15 m is limited to 10 m.
    let delta = position - v.pos;
    let distance = delta.length();
    let moved = if distance > 15.0 {
        delta * (10.0 / distance)
    } else {
        delta
    };
    v.pos += moved;
    v.vel = moved / dt;
    let forward = if length > 0.0 {
        tangent / length
    } else {
        v.rot * Vec3::Z
    };
    let right = Vec3::Y.cross(forward).try_normalize().unwrap_or(Vec3::X);
    let up = forward.cross(right);
    let frame = Quat::from_mat3(&glam::Mat3::from_cols(right, up, forward));
    v.rot = (frame * quat_z(ai.roll[1])).normalize();
}

#[cfg(test)]
mod tests;
