//! BF2 award guided rockets: class data, quaternion flight, energy and boost,
//! end reasons and replay state. Follows `analysis/re-guided-rockets.md` (card
//! GUIDED).

use crate::unit::UnitHandle;
use crate::vehicle::flyer::{self, Flight, FlightPhase, FlyerEnvironment, FlyerParams, FlyerState};
use crate::vehicle::hover::{quat_x, quat_y, quat_z, HoverInput};
use crate::weapon::{pf, pf_hash, ExplosionClass, HealthType, PropSource};
use glam::{Quat, Vec3};

pub const COLLISION_RADIUS_PROVISIONAL: f32 = 0.2;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuidedView {
    pub eye_point_offset: Option<[f32; 3]>,
    pub track_center: Option<[f32; 3]>,
    pub track_offset: Option<[f32; 3]>,
    pub slow_offset: Option<[f32; 3]>,
    pub fast_offset: Option<[f32; 3]>,
    pub fast_fov: Option<f32>,
    pub third_person_fov: Option<f32>,
    pub move_tension: [Option<[f32; 2]>; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuidedClass {
    pub flight: FlyerParams,
    pub view: GuidedView,
    pub health: f32,
    pub health_type: HealthType,
    pub explosion: Option<ExplosionClass>,
    pub noncritical: Option<ExplosionClass>,
    pub critical_speed: f32,
    pub contact_damage: [f32; 2],
    pub hold_time: f32,
    pub hold_speed: f32,
    pub hold_distance: f32,
    pub track_center: Vec3,
    pub track_offset: Vec3,
    pub engine_sound: Option<String>,
    pub death_sound: Option<String>,
    pub trail: Option<String>,
    pub trail_scale: [f32; 3],
}

fn vector(src: &dyn PropSource, hash: u32, default: Vec3) -> Vec3 {
    let Some(value) = src.prop_hash(hash) else {
        return default;
    };
    let mut values = value
        .split_whitespace()
        .filter_map(|v| v.parse::<f32>().ok());
    match (values.next(), values.next(), values.next()) {
        (Some(x), Some(y), Some(z)) => Vec3::new(x, y, z),
        _ => default,
    }
}

impl GuidedClass {
    pub fn from_props(src: &dyn PropSource) -> Self {
        Self {
            flight: FlyerParams::bf2(),
            view: GuidedView::default(),
            health: pf(src, "MaxHealth").unwrap_or(1.0),
            health_type: src
                .prop("HealthType")
                .and_then(HealthType::parse)
                .unwrap_or(HealthType::Droid),
            explosion: None,
            noncritical: None,
            critical_speed: pf_hash(src, 0xbb30_82af).unwrap_or(0.8),
            contact_damage: [
                pf_hash(src, 0x376b_238a).unwrap_or(500.0),
                pf_hash(src, 0x487b_2329).unwrap_or(2000.0),
            ],
            hold_time: pf_hash(src, 0x9bdf_a490).unwrap_or(1.0),
            hold_speed: pf_hash(src, 0x962e_0745).unwrap_or(60.0),
            hold_distance: pf_hash(src, 0x325b_f479).unwrap_or(20.0),
            track_center: vector(src, 0xe85d_5895, Vec3::ZERO),
            track_offset: vector(src, 0xfd3d_9507, Vec3::ZERO),
            engine_sound: src.prop("EngineSound").map(str::to_owned),
            death_sound: src.prop("DeathSound").map(str::to_owned),
            trail: src.prop_hash(0x0894_d756).map(str::to_owned),
            trail_scale: [
                pf_hash(src, 0xf174_a3f8).unwrap_or(1.0),
                pf_hash(src, 0xc3fd_784a).unwrap_or(1.0),
                pf_hash(src, 0xe970_41b8).unwrap_or(0.0),
            ],
        }
    }

    pub fn trail_size(&self, speed: f32) -> f32 {
        self.flight.exhaust_scale(self.trail_scale, speed)
    }

    pub fn top_speed(&self) -> f32 {
        self.flight.boost_speed.max(self.flight.max_speed)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuidedWeapon {
    pub class: GuidedClass,
    pub spawn_offset: Vec3,
    pub requires_award: bool,
}

impl GuidedWeapon {
    pub fn new(src: &dyn PropSource, class: GuidedClass) -> Self {
        Self {
            class,
            spawn_offset: vector(src, 0x389c_8e46, Vec3::ZERO),
            requires_award: pf(src, "MedalsTypeToUnlock").is_some_and(|v| v != -1.0),
        }
    }

    pub fn explosion(&self, end: GuidedEnd) -> Option<&ExplosionClass> {
        if matches!(end, GuidedEnd::Destroyed | GuidedEnd::CriticalExpiry) {
            self.class.explosion.as_ref()
        } else {
            self.class.noncritical.as_ref()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidedEnd {
    Destroyed,
    Expired,
    CriticalExpiry,
    Returned,
    OwnerGone,
}

/// `[ours]` A bot flying a guided rocket whose target is lost, cloaked or
/// out of sight for this long hands control back (PR #61 review: it flew
/// blind up to the rocket's 40 s of energy). The PS2's bot rule is untraced.
pub const BOT_BLIND: f32 = 3.0;

#[derive(Clone, Debug)]
pub struct GuidedRocket {
    pub id: u64,
    pub owner: UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
    pub seat: Option<(crate::vehicle::VehicleHandle, u8)>,
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: Quat,
    pub flyer: FlyerState,
    pub health: f32,
    pub age: f32,
    pub prev_buttons: u32,
    pub pending_end: Option<GuidedEnd>,
    pub spent: Option<(GuidedEnd, f32)>,
    /// `[ours]` A bot's seconds of flight with no target in sight; at
    /// [`BOT_BLIND`] it hands control back.
    pub blind: f32,
}

impl GuidedRocket {
    pub fn body(aim: Vec3) -> Quat {
        let f = aim.normalize_or_zero();
        let yaw = crate::fmath::atan2(-f.x, -f.z);
        let pitch = crate::fmath::asin(f.y.clamp(-1.0, 1.0));
        (quat_y(yaw) * quat_x(pitch) * quat_y(std::f32::consts::PI)).normalize()
    }

    pub fn flying(c: &GuidedClass) -> FlyerState {
        FlyerState {
            phase: FlightPhase::Flying,
            lift: 1.0,
            energy: c.flight.energy_max,
            level_time: 1.0,
            initialized: true,
            ..FlyerState::default()
        }
    }

    pub fn frame(&self) -> Quat {
        self.rot * quat_x(-self.flyer.tilt_pitch) * quat_z(-self.flyer.tilt_roll)
    }

    pub fn forward(&self) -> Vec3 {
        self.frame() * Vec3::Z
    }

    pub fn speed(&self) -> f32 {
        self.vel.dot(self.rot * Vec3::Z)
    }

    pub fn energy(&self) -> f32 {
        self.flyer.energy
    }

    pub fn live(&self) -> bool {
        self.spent.is_none()
    }

    pub fn yaw_pitch(&self) -> (f32, f32) {
        let f = self.forward();
        (
            crate::fmath::atan2(-f.x, -f.z),
            crate::fmath::asin(f.y.clamp(-1.0, 1.0)),
        )
    }

    pub fn drawn_rotation(&self) -> Quat {
        self.rot
    }

    pub fn fly(&mut self, c: &GuidedClass, input: HoverInput, env: FlyerEnvironment, dt: f32) {
        flyer::fly(
            Flight {
                pos: &mut self.pos,
                vel: &mut self.vel,
                rot: &mut self.rot,
                state: &mut self.flyer,
                input,
                occupied: true,
            },
            &c.flight,
            env,
            dt,
        );
        self.age += dt;
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.id.to_le_bytes());
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[
            self.team,
            self.pending_end.map_or(255, |e| e as u8),
            self.spent.map_or(255, |(e, _)| e as u8),
        ]);
        eat(&(self.weapon_class as u64).to_le_bytes());
        for value in self
            .pos
            .to_array()
            .into_iter()
            .chain(self.vel.to_array())
            .chain(self.rot.to_array())
            .chain([
                self.health,
                self.age,
                self.blind,
                self.spent.map_or(0.0, |(_, t)| t),
            ])
        {
            eat(&value.to_bits().to_le_bytes());
        }
        self.flyer.hash_into(eat);
        eat(&self.prev_buttons.to_le_bytes());
    }
}
