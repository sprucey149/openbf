//! Placed explosives: mines, time bombs and detpacks (issue #43,
//! `analysis/re-grenades.md` sections 5 and 9).

use crate::fmath::FloatExt;
use crate::sticky::{Anchor, Sticky};
use crate::unit::UnitHandle;
use crate::weapon::{ExplosionClass, HealthType, PropSource};
use glam::{Quat, Vec3};

/// A placed item's sphere for shots and blasts. Ours: the PS2's is not known.
pub const RADIUS: f32 = 0.15;

pub fn fall(pos: &mut Vec3, vel: &mut Vec3, gravity: f32, dt: f32) {
    vel.y += gravity * dt * 0.5;
    *pos += *vel * dt;
    vel.y += gravity * dt * 0.5;
}

pub fn rebound(vel: Vec3, n: Vec3, friction: f32, rebound: f32) -> Vec3 {
    let vn = vel.dot(n);
    let vt = vel - n * vn;
    let f = (-vn * friction * (rebound + 1.0) / vt.length()).fmin(1.0);
    vt * (1.0 - f) - n * (vn * rebound)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detonation {
    Trigger,
    Expire,
    Destroyed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MineClass {
    pub bf2: bool,
    pub remote: bool,
    pub gravity: f32,
    pub rebound: f32,
    pub friction: f32,
    pub life_span: f32,
    pub health: f32,
    pub health_type: Option<HealthType>,
    pub trigger_radius: f32,
    pub friend_radius: f32,
    pub contact_trigger: bool,
    pub explosions: [Option<ExplosionClass>; 3],
    pub geometry: Option<String>,
    pub tick_sound: Option<String>,
    pub contact_sound: Option<String>,
}

impl MineClass {
    pub fn from_props(src: &dyn PropSource, bf2: bool) -> Self {
        let num = |key, default| {
            src.prop(key)
                .and_then(|s| s.split_whitespace().next()?.parse::<f32>().ok())
                .unwrap_or(default)
        };
        let hash = |key| {
            src.prop_hash(key)
                .and_then(|v| v.trim().parse::<f32>().ok())
        };
        Self {
            bf2,
            remote: false,
            gravity: num("Gravity", 1.0) * -9.8,
            rebound: num("Rebound", 0.3),
            friction: num("Friction", 0.5),
            life_span: num("LifeSpan", f32::MAX),
            health: num("MaxHealth", 100.0),
            health_type: src
                .prop("HealthType")
                .map_or(Some(HealthType::Person), HealthType::parse),
            trigger_radius: num("TriggerRadius", 0.0),
            // A friendly soldier this close stops the mine (key name unknown).
            friend_radius: hash(0xce754894).unwrap_or(0.0),
            // BF2 only: the mine also fires on touch (key name unknown).
            contact_trigger: bf2 && hash(0xbfb22e35).unwrap_or(0.0) != 0.0,
            explosions: [None, None, None],
            geometry: src.prop("GeometryName").map(|s| s.trim().to_owned()),
            tick_sound: src.prop("TickSound").map(|s| s.trim().to_owned()),
            contact_sound: src.prop("CollisionOtherSound").map(|s| s.trim().to_owned()),
        }
    }

    pub fn explosion(&self, cause: Detonation) -> Option<&ExplosionClass> {
        self.explosions[cause as usize].as_ref()
    }

    /// The numbers a placed item's tick reads, copied so the per-tick loop
    /// does not clone the class's names.
    pub fn motion(&self) -> MineMotion {
        MineMotion {
            bf2: self.bf2,
            gravity: self.gravity,
            rebound: self.rebound,
            friction: self.friction,
            life_span: self.life_span,
            trigger_radius: self.trigger_radius,
            friend_radius: self.friend_radius,
            contact_trigger: self.contact_trigger,
        }
    }
}

/// See [`MineClass::motion`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MineMotion {
    pub bf2: bool,
    pub gravity: f32,
    pub rebound: f32,
    pub friction: f32,
    pub life_span: f32,
    pub trigger_radius: f32,
    pub friend_radius: f32,
    pub contact_trigger: bool,
}

#[derive(Clone, Debug)]
pub struct Mine {
    pub id: u64,
    pub flight: Sticky,
    pub age: f32,
    pub health: f32,
    pub triggered: bool,
    pub rotation: Quat,
    pub anchor_rotation: Quat,
}

impl Mine {
    pub fn new(
        id: u64,
        pos: Vec3,
        vel: Vec3,
        class: &MineClass,
        owner: UnitHandle,
        team: u8,
        weapon: usize,
    ) -> Self {
        let mut flight = Sticky::new(
            pos,
            vel,
            &crate::sticky::StickyClass::new(class.bf2),
            class.life_span,
            owner,
            team,
            weapon,
        );
        flight.gravity = class.gravity;
        Self {
            id,
            flight,
            age: 0.0,
            health: class.health,
            triggered: false,
            rotation: Quat::IDENTITY,
            anchor_rotation: Quat::IDENTITY,
        }
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.id.to_le_bytes());
        self.flight.hash_into(eat);
        for v in [
            self.age,
            self.health,
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
            self.rotation.w,
            self.anchor_rotation.x,
            self.anchor_rotation.y,
            self.anchor_rotation.z,
            self.anchor_rotation.w,
        ] {
            eat(&v.hash_bits().to_le_bytes());
        }
        eat(&[u8::from(self.triggered)]);
    }

    pub fn anchor(&mut self, normal: Vec3, anchor: Anchor, hull: Option<Quat>) {
        self.flight.vel = Vec3::ZERO;
        self.flight.moving = false;
        self.flight.armed = true;
        self.flight.anchor = Some(anchor);
        let tilt = if normal.y < -0.999999 {
            Quat::from_xyzw(1.0, 0.0, 0.0, 0.0)
        } else {
            Quat::from_xyzw(normal.z, 0.0, -normal.x, 1.0 + normal.y).normalize()
        };
        self.rotation = tilt * self.rotation;
        if let Some(hull) = hull {
            self.anchor_rotation = hull.inverse() * self.rotation;
        }
    }
}
