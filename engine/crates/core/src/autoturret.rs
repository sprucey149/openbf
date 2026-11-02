//! The BF2 sniper's portable auto-turret: class data with the `portableturret`
//! defaults, fall / land / bounce, lifetime, the hover-stick aim turn and replay
//! state. Follows `analysis/re-portable-turret.md` (card ATUR).

use crate::ai::hover_drive::shaped;
use crate::fmath;
use crate::remote::{look_frame, name, tilted};
use crate::unit::UnitHandle;
use crate::vehicle::gun::wrap;
use crate::vehicle::VehicleHandle;
use crate::weapon::{
    pair, pf, ExplosionClass, HealthType, PropSource, WeaponClass, WeaponState, DEG,
};
use glam::{Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// `boneroot` height above the origin (`re-portable-turret.md` §7, skeleton).
pub const CENTRE: f32 = 1.056;
/// The authored `p_-so_sphere` on boneroot, mask 17 (§10: soldier + ordnance).
pub const RADIUS: f32 = 0.25;
/// `aimer_x` offset from the centre along the yaw (§7, skeleton).
pub const AIMER: f32 = 0.234;
/// `hp_fire` offset from `aimer_x` along the aim (§7, skeleton).
pub const FIRE: f32 = 0.074;
/// A contact lands the turret when its normal's y is above this (§4, B2:143871).
pub const LAND_NY: f32 = 0.7;
/// `Gravity` 1 x 9.8 m/s^2 (§0 "flight").
pub const GRAVITY: f32 = -9.8;
/// Tracking range, about 100 m (§0 "its aim"; `re-turret-ai.md` §2-3).
pub const VIEW: f32 = 100.0;
/// The fight is kept to MaxRange + this (§0 "its aim").
pub const FIGHT_MARGIN: f32 = 10.0;
/// Fires only with the aim inside this cone (§0 "its aim"; `re-turret-ai.md` §2-3).
pub const AIM_CONE: f32 = 22.5 * DEG;
/// Idle look: base yaw +/- this (§0 "its aim"; `re-turret-ai.md` §2.3).
pub const LOOK_SPREAD: f32 = 30.0 * DEG;
/// The idle look point is redrawn every this many thinks (§0 "its aim").
pub const LOOK_EVERY: u32 = 18;
/// OURS: the think interval in seconds (§9). PROVISIONAL.
pub const THINK: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretEnd {
    Destroyed,
    Expired,
    Replaced,
    Fell,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurretBody {
    pub lifetime: f32,
    pub gravity: f32,
    pub rebound: f32,
    pub friction: f32,
    pub max_health: f32,
    pub yaw_limits: [f32; 2],
    pub pitch_limits: [f32; 2],
    pub turn_speed: f32,
    pub pitch_speed: f32,
}

impl TurretBody {
    pub fn from_props(src: &dyn PropSource) -> Self {
        let f = |key, default| pf(src, key).unwrap_or(default);
        let limits = |key, default| {
            pair(src, key)
                .map(|p| p.map(|v| v * DEG))
                .unwrap_or(default)
        };
        Self {
            lifetime: f("Lifetime", f32::MAX),
            gravity: f("Gravity", 1.0) * GRAVITY,
            rebound: f("Rebound", 0.3),
            friction: f("Friction", 0.5),
            max_health: f("MaxHealth", 100.0),
            yaw_limits: limits("YawLimits", [-PI, PI]),
            pitch_limits: limits("PitchLimits", [-FRAC_PI_2, FRAC_PI_2]),
            turn_speed: f("MaxTurnSpeed", 2.0),
            pitch_speed: f("MaxPitchSpeed", 0.5),
        }
    }

    pub fn gains(&self) -> (f32, f32) {
        (
            4.5 - self.turn_speed.clamp(1.0, 3.0),
            5.0 - self.pitch_speed.clamp(1.0, 3.0),
        )
    }

    pub fn wraps(&self) -> bool {
        self.yaw_limits[1] - self.yaw_limits[0] >= TAU - 1e-3
    }
}

/// Visual health-band effect from the portable turret's authored class.
/// Its PS2 model has no `hp_smoke_1`, so both effects use the entity origin.
#[derive(Clone, Debug, PartialEq)]
pub struct DamageEffect {
    pub start: f32,
    pub stop: f32,
    pub effect: String,
}

impl DamageEffect {
    pub fn active(&self, fraction: f32) -> bool {
        fraction <= self.start && fraction > self.stop
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurretClass {
    pub body: TurretBody,
    pub health_type: HealthType,
    pub explosion: Option<ExplosionClass>,
    pub geometry: Option<String>,
    pub gun: Box<WeaponClass>,
    pub gun_index: usize,
    /// Presentation only, excluded from simulation hashing.
    pub damage_effects: Vec<DamageEffect>,
}

impl TurretClass {
    pub fn new(src: &dyn PropSource, gun: WeaponClass) -> Self {
        Self {
            body: TurretBody::from_props(src),
            health_type: src
                .prop("HealthType")
                .and_then(HealthType::parse)
                .unwrap_or(HealthType::Building),
            explosion: None,
            geometry: name(src, "GeometryName"),
            gun: Box::new(gun),
            gun_index: usize::MAX,
            damage_effects: Vec::new(),
        }
    }

    pub fn end_explosion(&self, end: TurretEnd) -> Option<&ExplosionClass> {
        self.explosion.as_ref().filter(|_| end != TurretEnd::Fell)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    Unit(UnitHandle),
    Vehicle(VehicleHandle),
}

#[derive(Clone, Debug)]
pub struct Turret {
    pub id: u64,
    pub owner: UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
    pub pos: Vec3,
    pub vel: Vec3,
    pub up: Vec3,
    pub landed: bool,
    pub base_yaw: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub look: f32,
    pub health: f32,
    pub age: f32,
    pub think: f32,
    pub thinks: u32,
    pub target: Option<Aim>,
    pub fighting: bool,
    pub gun: WeaponState,
}

impl Turret {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        owner: UnitHandle,
        team: u8,
        weapon_class: usize,
        c: &TurretClass,
        centre: Vec3,
        vel: Vec3,
        yaw: f32,
    ) -> Self {
        Self {
            id,
            owner,
            team,
            weapon_class,
            pos: centre - Vec3::Y * CENTRE,
            vel,
            up: Vec3::Y,
            landed: false,
            base_yaw: yaw,
            yaw,
            pitch: 0.0,
            look: yaw,
            health: c.body.max_health,
            age: 0.0,
            think: 0.0,
            thinks: 0,
            target: None,
            fighting: false,
            gun: WeaponState::with_clips(&c.gun, false, 0),
        }
    }

    pub fn centre(&self) -> Vec3 {
        self.pos + self.up * CENTRE
    }

    pub fn aim_frame(&self) -> (Vec3, Vec3, Vec3) {
        look_frame(self.yaw, self.pitch)
    }

    pub fn muzzle(&self) -> Vec3 {
        self.centre() + look_frame(self.yaw, 0.0).0 * AIMER + self.aim_frame().0 * FIRE
    }

    pub fn rotation(&self) -> Quat {
        tilted(self.up, self.yaw + PI)
    }

    pub fn alive(&self) -> bool {
        self.health > 0.0
    }

    pub fn grow_old(&mut self, c: &TurretBody, dt: f32) -> Option<TurretEnd> {
        if !self.alive() {
            return Some(TurretEnd::Destroyed);
        }
        if c.lifetime > 0.0 {
            self.age += dt;
        }
        (self.age > c.lifetime).then_some(TurretEnd::Expired)
    }

    pub fn land(&mut self, point: Vec3, n: Vec3) {
        self.up = n;
        self.pos = point;
        self.vel = Vec3::ZERO;
        self.landed = true;
        self.base_yaw = self.yaw;
        self.look = self.yaw;
    }

    pub fn bounce(&mut self, c: &TurretBody, point: Vec3, n: Vec3, push: f32) {
        self.vel = crate::mine::rebound(self.vel, n, c.friction, c.rebound);
        self.pos = point + n * push;
    }

    pub fn angles_to(&self, p: Vec3) -> (f32, f32) {
        let to = p - self.centre();
        (
            fmath::atan2(-to.x, -to.z),
            fmath::atan2(to.y, to.with_y(0.0).length()),
        )
    }

    pub fn on_target(&self, p: Vec3) -> bool {
        (p - self.muzzle())
            .normalize_or_zero()
            .dot(self.aim_frame().0)
            >= fmath::sin_cos(AIM_CONE).1
    }

    pub fn turn(&mut self, c: &TurretBody, dt: f32, want: (f32, f32)) {
        let (yaw_gain, pitch_gain) = c.gains();
        let (s, co) = fmath::sin_cos(wrap(want.0 - self.yaw));
        let t = if co < 0.0 { s.signum() } else { s };
        self.yaw = wrap(self.yaw + shaped(t, yaw_gain) * c.turn_speed * dt);
        if !c.wraps() {
            let off = wrap(self.yaw - self.base_yaw).clamp(c.yaw_limits[0], c.yaw_limits[1]);
            self.yaw = wrap(self.base_yaw + off);
        }
        let p = fmath::sin_cos(want.1 - self.pitch).0;
        self.pitch = (self.pitch + shaped(p, pitch_gain) * c.pitch_speed * dt)
            .clamp(c.pitch_limits[0], c.pitch_limits[1]);
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.id.to_le_bytes());
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[self.team, u8::from(self.landed), u8::from(self.fighting)]);
        eat(&(self.weapon_class as u64).to_le_bytes());
        for v in [self.pos, self.vel, self.up] {
            for x in v.to_array() {
                eat(&x.to_bits().to_le_bytes());
            }
        }
        for v in [
            self.base_yaw,
            self.yaw,
            self.pitch,
            self.look,
            self.health,
            self.age,
            self.think,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&self.thinks.to_le_bytes());
        match self.target {
            None => eat(&[0]),
            Some(Aim::Unit(h)) => {
                eat(&[1]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            Some(Aim::Vehicle(h)) => {
                eat(&[2]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
        }
        self.gun.hash_into(eat);
    }
}

#[cfg(test)]
pub(crate) mod tests;
