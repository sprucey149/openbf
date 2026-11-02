//! The remote droid (BF1 and BF2): class data, launcher, destruct (salvo or
//! burst), flight step, drain and replay state. Follows
//! `analysis/re-remote-droid.md` (card REMOTE); the bots' droid rules are card DROIDAI.

use crate::unit::UnitHandle;
use crate::weapon::{
    pair, pf, pf_hash, ExplosionClass, HealthType, PropSource, WeaponClass, WeaponState, DEG,
};
use glam::{Mat3, Quat, Vec3};

pub const RADIUS: f32 = 0.35;
/// [ours] Every droid lasts 90 s from launch, both games, all maps (the PS2:
/// BF1 40 s of health drain, BF2 about 58.8 s of energy drain).
pub const LIFETIME: f32 = 90.0;
pub const GRAVITY: f32 = 9.8;
pub const PROBE: f32 = 10.0;
// [ours] The bots' droid rules (card DROIDAI, `analysis/re-remote-droid.md`
// §7). A bot launches from cover into a fight it is not in, flies a while,
// then goes in itself.
/// A flying bot hands back after this long with no enemy near its droid (s).
pub const BOT_RECALL: f32 = 10.0;
/// The droid looks for an enemy to shoot within this of itself (m).
pub const BOT_RANGE: f32 = 40.0;
/// Any enemy this close to the body and the bot fights with its own gun (m).
pub const BOT_NEAR: f32 = 20.0;
/// The fight a droid is sent into: an enemy the bot (or its team, through
/// what friends tell it) knows of within this of the body. Also how far an
/// enemy's sight of the body counts as exposure (m).
pub const BOT_FIGHT: f32 = 60.0;
/// No launch for this long after the body was hit (s).
pub const BOT_HIT_QUIET: f32 = 3.0;
/// The chance a bot that may launch does, drawn once per [`BOT_ROLL`] window.
pub const BOT_CHANCE: f32 = 0.25;
/// The window of one launch draw (s).
pub const BOT_ROLL: f32 = 2.0;
/// No new launch for this long after a flight ends (s).
pub const BOT_REST: f32 = 30.0;
/// The longest a bot flies before it goes in itself (s).
pub const BOT_FLIGHT: f32 = 30.0;
/// An enemy this close to the body with sight of it ends the flight (m).
pub const BOT_GUARD: f32 = 15.0;
/// Enemies within this of the droid are "among" it, for the destruct (m).
pub const BOT_BLAST: f32 = 8.0;
/// The destruct goes off among this many enemies...
pub const BOT_CROWD: usize = 2;
/// ...or among one with the droid's health under this share.
pub const BOT_LOW: f32 = 0.3;

pub fn look_frame(yaw: f32, pitch: f32) -> (Vec3, Vec3, Vec3) {
    let (sy, cy) = crate::fmath::sin_cos(yaw);
    let (sp, cp) = crate::fmath::sin_cos(pitch);
    let fwd = Vec3::new(-sy * cp, sp, -cy * cp);
    let right = Vec3::new(cy, 0.0, -sy);
    let up = right.cross(fwd).normalize();
    (fwd, right, up)
}

pub fn tilted(up: Vec3, yaw: f32) -> Quat {
    let tilt = if up.y < -0.999999 {
        Quat::from_xyzw(1.0, 0.0, 0.0, 0.0)
    } else {
        Quat::from_xyzw(up.z, 0.0, -up.x, 1.0 + up.y).normalize()
    };
    tilt * crate::vehicle::hover::quat_y(yaw)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DroidEnd {
    /// Shot down: its explosion goes off.
    Destroyed,
    /// [ours] Its 90 s ran out: it vanishes quietly and control returns.
    Expired,
    /// The self-destruct: its explosion plus the salvo or burst.
    Destruct,
    /// [ours] Its owner died: it vanishes quietly.
    OwnerDied,
}

impl DroidEnd {
    /// Whether this ending sets off the droid's explosion.
    pub fn explodes(self) -> bool {
        matches!(self, DroidEnd::Destroyed | DroidEnd::Destruct)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DroidClass {
    pub set_altitude: f32,
    pub gravity_scale: f32,
    pub lift_spring: f32,
    pub lift_damp: f32,
    pub acceleration: f32,
    pub max_speed: f32,
    pub max_strafe_speed: f32,
    pub max_turn_speed: f32,
    /// `MaxPitchSpeed` (B2 class +0x5d0, default 0.5): an AI pilot's pitch
    /// rate (B2 `FUN_001ee668` B2:101233-101240).
    pub max_pitch_speed: f32,
    pub pitch_limits: [f32; 2],
    pub level_spring: f32,
    pub level_damp: f32,
    pub max_health: f32,
    /// The disc's drains, read but not applied: the clock is [`LIFETIME`].
    pub add_health: f32,
    pub energy_restore: f32,
    pub health_type: HealthType,
    pub eye_offset: Vec3,
    pub track_offset: Vec3,
    pub tilt: f32,
    pub explosion: Option<ExplosionClass>,
    pub geometry: Option<String>,
}

fn lead(s: &str) -> Option<f32> {
    let s = s.trim_start();
    let end = s
        .char_indices()
        .find(|&(k, c)| !(c.is_ascii_digit() || c == '.' || (k == 0 && c == '-')))
        .map_or(s.len(), |(k, _)| k);
    s[..end].parse().ok()
}

fn vec3(src: &dyn PropSource, key: &str) -> Option<Vec3> {
    let v: Vec<f32> = src
        .prop(key)?
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

pub(crate) fn name(src: &dyn PropSource, key: &str) -> Option<String> {
    src.prop(key).map(|s| s.trim().to_owned())
}

impl DroidClass {
    pub fn from_props(src: &dyn PropSource, bf2: bool) -> Self {
        let f = |key, default| pf(src, key).unwrap_or(default);
        let acceleration = if bf2 {
            pf(src, "Acceleration").or_else(|| pf(src, "Acceleraton"))
        } else {
            pf(src, "Acceleraton")
        };
        Self {
            set_altitude: f("SetAltitude", 0.0),
            gravity_scale: src.prop("GravityScale").and_then(lead).unwrap_or(1.0),
            lift_spring: f("LiftSpring", 5.0),
            lift_damp: f("LiftDamp", 2.0),
            acceleration: acceleration.unwrap_or(70.0),
            max_speed: f("MaxSpeed", 9.0),
            max_strafe_speed: f("MaxStrafeSpeed", 7.0),
            max_turn_speed: f("MaxTurnSpeed", 2.0),
            max_pitch_speed: f("MaxPitchSpeed", 0.5),
            pitch_limits: pair(src, "PitchLimits")
                .map(|p| p.map(|v| v * DEG))
                .unwrap_or([-1.0, 1.0]),
            level_spring: f("LevelSpring", 10.0),
            level_damp: f("LevelDamp", 3.0),
            max_health: f("MaxHealth", 100.0),
            add_health: f("AddHealth", 0.0),
            energy_restore: if bf2 {
                f("EnergyAutoRestore", 0.0)
            } else {
                0.0
            },
            health_type: src
                .prop("HealthType")
                .and_then(HealthType::parse)
                .unwrap_or(HealthType::Droid),
            eye_offset: vec3(src, "EyePointOffset").unwrap_or(Vec3::ZERO),
            track_offset: vec3(src, "TrackOffset").unwrap_or(Vec3::new(0.0, 1.0, 8.0)),
            tilt: f("TiltValue", 0.0) * DEG,
            explosion: None,
            geometry: name(src, "GeometryName"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Salvo {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub count: u32,
    pub delay: f32,
    pub initial: f32,
    pub spread: f32,
    pub shell: Box<WeaponClass>,
}

impl Salvo {
    pub fn from_props(src: &dyn PropSource, shell: WeaponClass) -> Self {
        let h = |hash, default| pf_hash(src, hash).unwrap_or(default);
        Self {
            yaw: h(0x3186fbfe, 0.0) * DEG,
            pitch: pf_hash(src, 0x8339fb35).map_or(std::f32::consts::FRAC_PI_2, |v| v * DEG),
            distance: h(0xecdb9d1d, 100.0),
            count: pf(src, "SalvoCount").unwrap_or(1.0).max(1.0) as u32,
            delay: pf(src, "SalvoDelay").unwrap_or(0.0).max(0.0),
            initial: pf(src, "InitialSalvoDelay").unwrap_or(0.0).max(0.0),
            spread: h(0xebd81a5e, 0.0),
            shell: Box::new(shell),
        }
    }

    pub fn frame(&self) -> Mat3 {
        let (sp, cp) = crate::fmath::sin_cos(-self.pitch);
        let (sy, cy) = crate::fmath::sin_cos(self.yaw);
        let x = Mat3::from_cols(Vec3::X, Vec3::new(0.0, cp, sp), Vec3::new(0.0, -sp, cp));
        let y = Mat3::from_cols(Vec3::new(cy, 0.0, -sy), Vec3::Y, Vec3::new(sy, 0.0, cy));
        x * y
    }

    pub fn shot(&self, target: Vec3, r: [f32; 4]) -> (Vec3, Vec3) {
        let m = self.frame();
        let local = Vec3::new(
            self.spread * (r[0] - r[1]),
            self.spread * (r[2] - r[3]),
            self.distance,
        );
        (target + m * local, -(m * Vec3::Z))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Destruct {
    pub charge_time: f32,
    pub charge_sound: Option<String>,
    pub salvo: Option<Salvo>,
    pub burst: Option<ExplosionClass>,
    pub burst_sound: Option<String>,
}

impl Destruct {
    pub fn from_props(src: &dyn PropSource) -> Self {
        Self {
            charge_time: pf(src, "MaxPressedTime").unwrap_or(0.0).max(0.0),
            charge_sound: name(src, "ChargeSound"),
            salvo: None,
            burst: None,
            burst_sound: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemoteWeapon {
    pub droid: DroidClass,
    pub take_control: bool,
    pub velocity: f32,
    pub strength: [f32; 2],
    pub blaster: Box<WeaponClass>,
    pub destruct: Option<Destruct>,
    pub blaster_index: usize,
    pub shell_index: usize,
}

impl RemoteWeapon {
    pub fn new(src: &dyn PropSource, droid: DroidClass, blaster: WeaponClass) -> Self {
        Self {
            droid,
            take_control: pf_hash(src, 0xc43b77dd).is_some_and(|v| v != 0.0),
            velocity: pf(src, "Velocity").unwrap_or(10.0),
            strength: [
                pf(src, "MinStrength").unwrap_or(0.3),
                pf(src, "MaxStrength").unwrap_or(1.0),
            ],
            blaster: Box::new(blaster),
            destruct: None,
            blaster_index: usize::MAX,
            shell_index: usize::MAX,
        }
    }

    pub fn launch_speed(&self, charge: f32) -> f32 {
        (self.strength[0] + (self.strength[1] - self.strength[0]) * charge) * self.velocity
    }

    pub fn end_burst(&self, end: DroidEnd) -> Option<&Destruct> {
        self.destruct.as_ref().filter(|_| end == DroidEnd::Destruct)
    }

    pub fn end_explosions(&self, end: DroidEnd) -> impl Iterator<Item = &ExplosionClass> {
        let burst = self.end_burst(end).and_then(|x| x.burst.as_ref());
        let own = self.droid.explosion.as_ref().filter(|_| end.explodes());
        own.into_iter().chain(burst)
    }
}

#[derive(Clone, Debug)]
pub struct Droid {
    pub id: u64,
    pub owner: UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub up: Vec3,
    pub up_vel: Vec3,
    pub health: f32,
    pub age: f32,
    pub controlled: bool,
    pub idle: f32,
    pub prev_buttons: u32,
    pub blaster: WeaponState,
    pub charge: Option<f32>,
    /// The boundary countdown's seconds left; `None` inside the map.
    pub leaving: Option<f32>,
}

impl Droid {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        owner: UnitHandle,
        team: u8,
        weapon_class: usize,
        remote: &RemoteWeapon,
        pos: Vec3,
        vel: Vec3,
        yaw: f32,
        human: bool,
    ) -> Self {
        Self {
            id,
            owner,
            team,
            weapon_class,
            pos,
            vel,
            yaw,
            pitch: 0.0,
            up: Vec3::Y,
            up_vel: Vec3::ZERO,
            health: remote.droid.max_health,
            age: 0.0,
            controlled: remote.take_control,
            idle: 0.0,
            prev_buttons: 0,
            blaster: WeaponState::with_clips(&remote.blaster, human, 0),
            charge: None,
            leaving: None,
        }
    }

    pub fn aim_frame(&self) -> (Vec3, Vec3, Vec3) {
        look_frame(self.yaw, self.pitch)
    }

    pub fn mouse_turned(&self, c: &DroidClass, look: (f32, f32), scale: f32) -> (f32, f32) {
        (
            crate::vehicle::gun::wrap(self.yaw - look.0 * scale),
            (self.pitch + look.1 * scale).clamp(c.pitch_limits[0], c.pitch_limits[1]),
        )
    }

    pub fn muzzle(&self) -> Vec3 {
        self.pos + self.aim_frame().0 * RADIUS
    }

    pub fn rotation(&self) -> Quat {
        tilted(self.up, self.yaw)
    }

    pub fn alive(&self) -> bool {
        self.health > 0.0
    }

    pub fn fly(
        &mut self,
        c: &DroidClass,
        dt: f32,
        forward: f32,
        strafe: f32,
        ground: Option<(f32, Vec3)>,
    ) {
        let a = c.set_altitude;
        let (k, n) = match ground {
            Some((h, n)) if a > 0.0 && h <= PROBE * a => {
                let k = (1.0 - (h - a) / (9.0 * a)).clamp(0.0, 1.0);
                (k, n)
            }
            _ => (0.0, Vec3::Y),
        };
        self.vel.y -= ((c.gravity_scale - 1.0) * k + 1.0) * GRAVITY * dt;
        if let Some((h, _)) = ground.filter(|_| k > 0.0) {
            let f = k * GRAVITY + c.lift_spring * (a - h) - c.lift_damp * self.vel.dot(n);
            if f > 0.0 {
                self.vel += n * (f * dt);
            }
        }
        let (sy, cy) = crate::fmath::sin_cos(self.yaw);
        let level = |d: Vec3| (d - n * d.dot(n)).normalize_or(d);
        let fwd = level(Vec3::new(-sy, 0.0, -cy));
        let right = level(Vec3::new(cy, 0.0, -sy));
        let cap = c.acceleration * k * n.y.max(0.0) * dt;
        let along = |v: Vec3, d: Vec3, want: f32| d * (want - v.dot(d)).clamp(-cap, cap);
        self.vel += along(self.vel, fwd, forward * c.max_speed);
        self.vel += along(self.vel, right, strafe * c.max_strafe_speed);
        let target = if k > 0.0 { n } else { Vec3::Y };
        let spring = (target - self.up) * c.level_spring - self.up_vel * c.level_damp;
        self.up_vel += spring * dt;
        self.up = (self.up + self.up_vel * dt).normalize_or(Vec3::Y);
        self.pos += self.vel * dt;
    }

    /// One update of the boundary countdown (`analysis/re-boundary.md` T1, T2;
    /// that their third entity is the droid is [H]): `time` is set and spent in
    /// the first outside update, and the droid dies in the one that takes it
    /// below 0. Coming back inside clears it. True on the update it dies.
    pub fn tick_leaving(&mut self, outside: bool, time: f32, dt: f32) -> bool {
        self.leaving = outside.then(|| self.leaving.unwrap_or(time) - dt);
        self.leaving.is_some_and(|t| t < 0.0)
    }

    /// Seconds of its [`LIFETIME`] left.
    pub fn time_left(&self) -> f32 {
        (LIFETIME - self.age).max(0.0)
    }

    /// [ours] Age one tick: `Expired` on the tick it reaches [`LIFETIME`].
    /// Health does not drain.
    pub fn age_by(&mut self, dt: f32) -> Option<DroidEnd> {
        self.age += dt;
        (self.age >= LIFETIME - dt * 0.5).then_some(DroidEnd::Expired)
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.id.to_le_bytes());
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[self.team, u8::from(self.controlled)]);
        eat(&(self.weapon_class as u64).to_le_bytes());
        for v in [self.pos, self.vel, self.up, self.up_vel] {
            for x in v.to_array() {
                eat(&x.to_bits().to_le_bytes());
            }
        }
        for v in [
            self.yaw,
            self.pitch,
            self.health,
            self.age,
            self.idle,
            self.charge.unwrap_or(-1.0),
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        // Only while outside, so a flight inside the map hashes as it always has.
        if let Some(t) = self.leaving {
            eat(&t.to_bits().to_le_bytes());
        }
        eat(&self.prev_buttons.to_le_bytes());
        self.blaster.hash_into(eat);
    }
}

#[derive(Clone, Debug)]
pub struct StrikeRun {
    pub owner: UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
    pub target: Vec3,
    pub left: u32,
    pub timer: f32,
}

impl StrikeRun {
    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[self.team]);
        eat(&(self.weapon_class as u64).to_le_bytes());
        for x in self.target.to_array() {
            eat(&x.to_bits().to_le_bytes());
        }
        eat(&self.left.to_le_bytes());
        eat(&self.timer.to_bits().to_le_bytes());
    }
}

#[cfg(test)]
pub(crate) mod tests;
