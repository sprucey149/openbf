//! BF2 hero weapon data. Values come from the PS2 class chain; targeting
//! uses the emitter parser's half-angle cosine (B2 FUN_003156e0).

use crate::fmath;
use crate::fmath::FloatExt;
use crate::weapon::{PropSource, DEG};
use glam::{Mat4, Vec3};

mod checked;
mod deflect;

/// Combo visibility is one byte, shared by physical edges, drawing and sound.
pub const MAX_BLADE_EDGES: usize = u8::BITS as usize;

pub fn blade_visible(hidden: u8, edge: usize) -> bool {
    edge < MAX_BLADE_EDGES && hidden & (1u8 << edge) == 0
}

/// [ours] Input safety: at most 64 contact passes per thrown saber per tick.
/// Shipped BF2 throw classes need four, leaving 16 times that contact budget.
pub const MAX_THROW_SUBSTEPS: usize = 64;

pub fn sound_hash(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte | 0x20)).wrapping_mul(0x0100_0193)
    })
}

fn hash_target(target: &crate::projectile::Target, eat: &mut impl FnMut(&[u8])) {
    use crate::projectile::Target;
    match *target {
        Target::Unit(h) => {
            eat(&[0]);
            eat(&h.index.to_le_bytes());
            eat(&h.gen.to_le_bytes());
        }
        Target::Vehicle(h) => {
            eat(&[1]);
            eat(&h.index.to_le_bytes());
            eat(&h.gen.to_le_bytes());
        }
        Target::Station(i) => {
            eat(&[2]);
            eat(&i.to_le_bytes());
        }
        Target::Mine(i) => {
            eat(&[3]);
            eat(&i.to_le_bytes());
        }
        Target::Droid(i) => {
            eat(&[4]);
            eat(&i.to_le_bytes());
        }
        Target::Turret(i) => {
            eat(&[5]);
            eat(&i.to_le_bytes());
        }
        Target::Guided(i) => {
            eat(&[6]);
            eat(&i.to_le_bytes());
        }
        Target::Building(i) => {
            eat(&[7]);
            eat(&i.to_le_bytes());
        }
        Target::Shield(h) => {
            eat(&[8]);
            eat(&h.index.to_le_bytes());
            eat(&h.gen.to_le_bytes());
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Choked {
    pub left: f32,
    pub age: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForceChannel {
    pub acquired: bool,
    pub target_score: f32,
    pub elapsed: f32,
    pub retarget: f32,
    pub target: Option<crate::unit::UnitHandle>,
}

impl ForceChannel {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"FORCE");
        eat(&[u8::from(self.acquired)]);
        eat(&self.target_score.hash_bits().to_le_bytes());
        eat(&self.elapsed.hash_bits().to_le_bytes());
        eat(&self.retarget.hash_bits().to_le_bytes());
        match self.target {
            Some(h) => {
                eat(&[1]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            None => eat(&[0]),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForcePower {
    pub range: f32,
    pub cone_cos: f32,
    pub max_targets: usize,
    pub push: f32,
    pub affect_friends: bool,
    pub affect_enemies: bool,
    pub energy: f32,
    pub kind: ForceKind,
    pub effect: Option<String>,
    pub animation: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForceKind {
    Emitter(Emitter),
    Choke {
        angle: f32,
        damage_per_second: f32,
        height: f32,
        falloff: f32,
        drain_rate: f32,
    },
}

/// PS2 emitter-class fields, FUN_003153a8/003156e0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub hit_all: bool,
    pub first_radius: f32,
    pub jump_deduction: f32,
    pub damage_threshold: f32,
    pub max_jump_distance: f32,
    pub lightning: bool,
    pub burn_seconds: f32,
    pub burn_damage_per_second: f32,
}

/// The ordinary damage-effect timer, independent of poison/debuff immunity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burn {
    pub left: f32,
    pub damage_per_second: f32,
    pub owner: crate::unit::UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
}

impl Burn {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"BURN");
        eat(&self.left.hash_bits().to_le_bytes());
        eat(&self.damage_per_second.hash_bits().to_le_bytes());
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[self.team]);
        eat(&(self.weapon_class as u64).to_le_bytes());
    }
}

impl ForcePower {
    pub fn emitter(weapon: &dyn PropSource, ord: &dyn PropSource) -> Option<Self> {
        let range = checked::number(ord, "ConeLength", Some(10.0))?;
        if range <= 0.0 {
            return None;
        }
        Some(Self {
            range,
            cone_cos: match ord.prop("ConeAngle") {
                Some(_) => fmath::sin_cos(checked::number(ord, "ConeAngle", None)? * (DEG * 0.5)).1,
                None => f32::from_bits(0x3f5db22d),
            },
            max_targets: checked::number(ord, "MaxTargets", Some(5.0))?.fmax(0.0) as usize,
            push: checked::number(ord, "Push", Some(0.0))?,
            affect_friends: checked::number(ord, "AffectFriends", Some(1.0))? != 0.0,
            affect_enemies: checked::number(ord, "AffectEnemies", Some(1.0))? != 0.0,
            energy: checked::number(weapon, "EnergyDrain", Some(0.0))?,
            kind: ForceKind::Emitter(Emitter {
                hit_all: checked::hash(ord, 0x4c38_7ca1, Some(0.0))? != 0.0,
                first_radius: checked::number(ord, "FirstRadius", Some(1.5))?,
                jump_deduction: checked::number(ord, "JumpDeduction", Some(0.0))?,
                damage_threshold: checked::number(ord, "DamageThreshold", Some(0.0))?,
                max_jump_distance: checked::number(ord, "MaxJumpDistance", Some(10.0))?,
                lightning: ord
                    .prop_hash(0x6dd5_a894)
                    .is_some_and(|v| !v.is_empty() && !v.eq_ignore_ascii_case("null")),
                burn_seconds: checked::hash(ord, 0x6824_f471, Some(0.0))?,
                burn_damage_per_second: checked::hash(ord, 0x1076_dfd6, Some(0.0))?,
            }),
            effect: ord.prop("Effect").map(str::to_owned),
            animation: weapon.prop("SoldierAnimation").map(str::to_owned),
        })
    }

    pub fn choke(weapon: &dyn PropSource) -> Option<Self> {
        // PS2 area-effect parser FUN_0032d698, effect type Choke and Ray.
        let kind = weapon.prop_hash(0x5c1f_c8bc)?;
        if !kind.eq_ignore_ascii_case("Choke") {
            return None;
        }
        let range = checked::hash(weapon, 0x2efc_37e9, None)?;
        let height = checked::hash(weapon, 0x894a_0e77, None)?;
        if range <= 0.0 || height <= 0.0 {
            return None;
        }
        Some(Self {
            range,
            cone_cos: fmath::sin_cos(checked::hash(weapon, 0x3891_df5a, None)? * DEG).1,
            max_targets: checked::number(weapon, "MaxTargets", None)?.clamp(0.0, 4.0) as usize,
            push: 0.0,
            affect_friends: true,
            affect_enemies: true,
            energy: checked::number(weapon, "EnergyDrain", Some(0.0))?,
            kind: ForceKind::Choke {
                angle: checked::hash(weapon, 0x3891_df5a, Some(0.0))? * DEG,
                damage_per_second: checked::hash(weapon, 0xd2ea_8a45, None)?,
                height,
                falloff: checked::hash(weapon, 0x3b5e_b7fa, None)?,
                drain_rate: checked::hash(weapon, 0x8909_415b, Some(0.0))?,
            },
            effect: weapon.prop("Effect").map(str::to_owned),
            animation: weapon.prop("SoldierAnimation").map(str::to_owned),
        })
    }

    /// Range and cone only; the physical caller checks relations and sight.
    /// AI perception gates its firing choice rather than cone contacts.
    pub fn contains(&self, from: Vec3, direction: Vec3, target: Vec3) -> bool {
        let delta = target - from;
        let distance = delta.length();
        let cosine = if distance < 2.5 {
            self.cone_cos.fmin(0.1)
        } else {
            self.cone_cos
        };
        distance > 0.0 && distance < self.range && direction.dot(delta / distance) > cosine
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaberThrow {
    pub energy: f32,
    pub velocity: f32,
    pub outbound_seconds: f32,
    pub return_seconds: f32,
    pub damage: f32,
    pub width: f32,
    pub length: f32,
    pub width_from_edge: bool,
    pub length_from_edge: bool,
    pub rotation_rate: f32,
}

impl SaberThrow {
    pub fn from_props(weapon: &dyn PropSource, ord: &dyn PropSource) -> Option<Self> {
        let data = Self {
            energy: checked::number(weapon, "EnergyDrain", None)?,
            velocity: checked::number(ord, "Velocity", None)?,
            outbound_seconds: checked::number(ord, "LifeSpan", None)?,
            return_seconds: checked::hash(ord, 0x0dda_9b5b, None)?,
            damage: if ord.prop("Damage").is_some() {
                checked::number(ord, "Damage", None)?
            } else {
                checked::number(ord, "MaxDamage", None)?
            },
            width: checked::number(ord, "DamageWidth", None)?,
            length: checked::number(ord, "DamageLength", None)?,
            width_from_edge: ord.prop("DamageWidth").is_some_and(|s| {
                s.split_whitespace()
                    .any(|w| w.eq_ignore_ascii_case("FromEdge"))
            }),
            length_from_edge: ord.prop("DamageLength").is_some_and(|s| {
                s.split_whitespace()
                    .any(|w| w.eq_ignore_ascii_case("FromEdge"))
            }),
            rotation_rate: checked::number(ord, "RotationRate", None)? * std::f32::consts::TAU,
        };
        data.contact_substeps(crate::TICK_DT)?;
        (data.velocity > 0.0
            && data.outbound_seconds > 0.0
            && data.return_seconds >= 0.0
            && data.rotation_rate.is_finite()
            && (data.velocity * data.outbound_seconds).is_finite()
            && (data.return_seconds * data.outbound_seconds).is_finite())
        .then_some(data)
    }

    /// Keep the native spacing and signed final-outbound update, while refusing
    /// finite data that would demand an unbounded contact loop.
    pub fn contact_substeps(&self, seconds: f32) -> Option<usize> {
        let speed = self.velocity + self.rotation_rate;
        if !speed.is_finite() || speed < 0.0 {
            return None;
        }
        let step = (0.5 / speed).fmin(0.0333);
        let pieces = seconds / step;
        let limit = MAX_THROW_SUBSTEPS as f32;
        if !pieces.is_finite() || !(-limit..limit).contains(&pieces) {
            return None;
        }
        Some((pieces as i32 + 1).max(0) as usize)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaberFlight {
    pub pos: Vec3,
    pub velocity: Vec3,
    pub basis: Mat4,
    pub spin_axis: Vec3,
    pub angle: f32,
    pub blade_class: usize,
    pub remaining: f32,
    pub outbound_seconds: f32,
    pub return_age: f32,
    pub returning: bool,
    pub hit: Vec<crate::projectile::Target>,
}

impl SaberFlight {
    /// The PS2 scales return time by how much of the outbound flight was
    /// actually available (FUN_0031cb78; wall clipping shortens both legs).
    pub fn return_duration(&self, class: &SaberThrow) -> f32 {
        class.return_seconds * self.outbound_seconds / class.outbound_seconds
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"SABER_THROW");
        for v in self
            .pos
            .to_array()
            .into_iter()
            .chain(self.velocity.to_array())
            .chain([
                self.remaining,
                self.outbound_seconds,
                self.return_age,
                self.angle,
            ])
        {
            eat(&v.hash_bits().to_le_bytes());
        }
        for v in self
            .basis
            .to_cols_array()
            .into_iter()
            .chain(self.spin_axis.to_array())
        {
            eat(&v.hash_bits().to_le_bytes());
        }
        eat(&(self.blade_class as u64).to_le_bytes());
        eat(&[u8::from(self.returning)]);
        eat(&(self.hit.len() as u32).to_le_bytes());
        for target in &self.hit {
            hash_target(target, eat);
        }
    }

    /// The actual launch attachment rotates about the native cross axis.
    pub fn transform(&self) -> Mat4 {
        self.transform_at(self.pos, self.angle)
    }

    pub fn transform_at(&self, position: Vec3, angle: f32) -> Mat4 {
        let (s, c) = fmath::sin_cos(angle * 0.5);
        let axis = self.spin_axis * s;
        let q = if self.spin_axis.length_squared() > 0.0 {
            glam::Quat::from_xyzw(axis.x, axis.y, axis.z, c)
        } else {
            glam::Quat::IDENTITY
        };
        let mut matrix = Mat4::from_quat(q) * self.basis;
        matrix.w_axis = position.extend(1.0);
        matrix
    }

    /// [ours] (the maintainer, 2026-10-30): a contact ends the outbound leg
    /// at the blade's place in this tick. The clock and the leg's seconds
    /// shrink as the PS2's own cut does (FUN_0031d658), so the return scales
    /// by the seconds really flown and always ends.
    pub fn turn_back(&mut self, pos: Vec3, angle: f32, unspent_of_tick: f32) {
        let unspent = self.remaining.fmax(0.0) + unspent_of_tick.fmax(0.0);
        self.outbound_seconds = (self.outbound_seconds - unspent).fmax(0.0);
        self.remaining = 0.0;
        self.return_age = 0.0;
        self.returning = true;
        self.pos = pos;
        self.angle = angle;
    }

    /// PS2 0x31cf1c..0x31cfd4 uses the signed new remainder on the
    /// final outgoing update. It can step backward before returning.
    pub fn advance(&mut self, class: &SaberThrow, owner: Vec3, dt: f32) -> bool {
        let old = self.remaining;
        self.remaining -= dt;
        if old > 0.0 {
            let step = if self.remaining <= 0.0 {
                self.hit.clear();
                self.returning = true;
                self.return_age = -self.remaining;
                self.remaining
            } else {
                dt
            };
            self.pos += self.velocity * step;
            self.angle += class.rotation_rate * step;
            return true;
        }
        self.returning = true;
        self.return_age = -self.remaining;
        let duration = self.return_duration(class);
        if duration <= self.return_age {
            self.pos = owner;
            return false;
        }
        let old_age = self.return_age - dt;
        self.pos += (owner - self.pos) * (dt / (duration - old_age));
        self.angle += class.rotation_rate * 0.5 * dt;
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Blade {
    pub node: String,
    pub bone: String,
    pub length: f32,
    pub width: f32,
    pub texture: String,
    pub trail_color: Option<[u8; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BladeSegment {
    pub start: Vec3,
    pub end: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaberClip {
    pub name: String,
    /// Authored root travel, x sideways and z forward, in metres per second.
    pub root_speed: [f32; 2],
    /// Baked root displacement for load-time fallback and asset diagnostics.
    /// Runtime FromAnim movement uses the constant root_speed above.
    pub root_frames: Vec<Vec3>,
    /// Model-space posed attachment group zero at the same 30 Hz frames.
    pub weapon_frames: Vec<Mat4>,
    /// Model-space blade edges at every authored 30 Hz clip frame.
    pub frames: Vec<Vec<BladeSegment>>,
}

impl SaberClip {
    pub fn seconds(&self) -> f32 {
        self.frames.len().saturating_sub(1) as f32 / 30.0
    }

    pub fn sample(&self, seconds: f32, edge: usize) -> Option<BladeSegment> {
        let last = self.frames.len().checked_sub(1)?;
        let frame = (seconds.fmax(0.0) * 30.0).fmin(last as f32);
        let index = frame as usize;
        let a = *self.frames[index].get(edge)?;
        let b = self.frames[(index + 1).min(last)].get(edge)?;
        let blend = frame - index as f32;
        Some(BladeSegment {
            start: a.start.lerp(b.start, blend),
            end: a.end.lerp(b.end, blend),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaberThrowBinding {
    pub weapon: String,
    pub animation: String,
    pub seconds: f32,
    /// Constructor-cached model-space hp_weapons matrix (B2 3450c0).
    pub attachment: Mat4,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaberClass {
    pub combo: manifest::combo::ComboDef,
    pub blades: Vec<Blade>,
    /// Primary weapon-space edges; offhand and body attack edges stay attached.
    pub throw_edges: Vec<BladeSegment>,
    /// Local edges registered to the first owner-body attachment group.
    pub deflect_edges: Vec<BladeSegment>,
    pub weapon_bind: Mat4,
    pub throw_bindings: Vec<SaberThrowBinding>,
    pub clips: Vec<SaberClip>,
    /// Cached animation length per authored combo state.
    pub state_seconds: Vec<f32>,
    pub damage: f32,
    pub hit_sound: Option<String>,
    pub deflect_sound: Option<String>,
    /// `HitEffect` (melee class +0x24c): spawned at each blade contact that
    /// is not blocked (B2 FUN_00340c60, FUN_0031d688). Render-only.
    pub hit_effect: Option<String>,
    /// `DeflectEffect` (+0x26c): spawned at a block (B2 FUN_0033e8b8).
    pub deflect_effect: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct MeleeState {
    pub runtime: crate::combo::ComboRuntime,
    pub deflect_precision: f32,
    pub block_animation: Option<String>,
    pub block_animation_time: f32,
    pub previous_position: Vec3,
    pub previous_yaw: f32,
    pub previous_pitch: f32,
    pub aligned_to_thrust: bool,
    /// Native pre-factor owner axes, retained while Initial is set.
    pub retained_thrust: [f32; 2],
    pub current_thrust_forward: f32,
    pub animation: Option<String>,
    pub animation_loop: bool,
    pub hidden_edges: u8,
    pub deflect: Option<crate::combo::Deflect>,
    pub movement: Option<crate::combo::Movement>,
    pub movement_active: bool,
    pub movement_weight: f32,
    pub gravity: Option<crate::combo::Gravity>,
    pub gravity_done: bool,
    pub restore_rate: Option<f32>,
    pub turn_factor: f32,
    /// The combo's input lock this tick (`combo::JUMP` / `TURN` / ...): the
    /// window's look preview leaves out what the next tick will drop
    /// (`Sim::look_preview`). Rewritten every tick before any read, so it
    /// is not hashed.
    pub input_lock: u16,
    /// The thrown saber came back this tick: the next combo step enters
    /// CATCH_SABRE (the throw class's caught bit, FUN_00345c10, read by the
    /// melee update B2:333228-333249).
    pub catch_pending: bool,
    pub attacks: Vec<crate::combo::AttackWindow>,
    pub hit: Vec<MeleeHitCache>,
}

/// Native melee keeps one eight-victim cache per authored record or nonzero
/// packed AttackId. The original window end carries across state changes.
#[derive(Clone, Debug)]
pub struct MeleeHitCache {
    pub record: u64,
    pub id: u32,
    pub end: f32,
    pub victims: Vec<crate::projectile::Target>,
}

impl MeleeState {
    fn cache_index(&self, attack: &crate::combo::AttackWindow) -> Option<usize> {
        self.hit
            .iter()
            .position(|c| c.record == attack.record || (attack.id != 0 && c.id == attack.id))
    }

    pub fn ensure_hit_cache(&mut self, attack: &crate::combo::AttackWindow) -> usize {
        self.cache_index(attack).unwrap_or_else(|| {
            let index = self.hit.len();
            self.hit.push(MeleeHitCache {
                record: attack.record,
                id: attack.id,
                end: attack.cache_end,
                victims: Vec::new(),
            });
            index
        })
    }

    pub fn already_hit(
        &self,
        attack: &crate::combo::AttackWindow,
        target: crate::projectile::Target,
    ) -> bool {
        self.cache_index(attack)
            .is_some_and(|i| self.hit[i].victims.contains(&target))
    }

    pub fn record_hit(
        &mut self,
        attack: &crate::combo::AttackWindow,
        target: crate::projectile::Target,
    ) -> bool {
        let index = self.ensure_hit_cache(attack);
        let victims = &mut self.hit[index].victims;
        if victims.len() >= 8 || victims.contains(&target) {
            return false;
        }
        victims.push(target);
        true
    }

    pub fn expire_hits(&mut self) {
        let now = self.runtime.clock();
        self.hit.retain(|c| now < c.end + 0.3);
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"MELEE");
        self.runtime.hash_into(eat);
        eat(&self.deflect_precision.hash_bits().to_le_bytes());
        for value in self
            .retained_thrust
            .into_iter()
            .chain([self.current_thrust_forward])
        {
            eat(&value.hash_bits().to_le_bytes());
        }
        if let Some(animation) = &self.block_animation {
            eat(b"BLOCK_POSE");
            eat(&(animation.len() as u32).to_le_bytes());
            eat(animation.as_bytes());
            eat(&self.block_animation_time.hash_bits().to_le_bytes());
        }
        eat(&[u8::from(self.gravity_done), u8::from(self.catch_pending)]);
        eat(&[u8::from(self.movement_active)]);
        eat(&self.movement_weight.hash_bits().to_le_bytes());
        for n in self
            .previous_position
            .to_array()
            .into_iter()
            .chain([self.previous_yaw, self.previous_pitch])
        {
            eat(&n.hash_bits().to_le_bytes());
        }
        eat(&(self.hit.len() as u32).to_le_bytes());
        for cache in &self.hit {
            eat(&cache.record.to_le_bytes());
            eat(&cache.id.to_le_bytes());
            eat(&cache.end.hash_bits().to_le_bytes());
            eat(&(cache.victims.len() as u32).to_le_bytes());
            for victim in &cache.victims {
                hash_target(victim, eat);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod hostile;
    include!("hero_weapon/cache/tests.rs");

    fn cone() -> ForcePower {
        ForcePower {
            range: 20.0,
            cone_cos: 0.5,
            max_targets: 4,
            push: 15.0,
            affect_friends: true,
            affect_enemies: true,
            energy: 50.0,
            kind: ForceKind::Emitter(Emitter {
                hit_all: true,
                first_radius: 0.0,
                jump_deduction: 0.0,
                damage_threshold: 0.0,
                max_jump_distance: 0.0,
                lightning: false,
                burn_seconds: 0.0,
                burn_damage_per_second: 0.0,
            }),
            effect: None,
            animation: None,
        }
    }

    #[test]
    fn cone_is_a_full_authored_angle_with_a_range_boundary() {
        let power = cone();
        assert!(power.contains(Vec3::ZERO, -Vec3::Z, -Vec3::Z * 19.99));
        assert!(!power.contains(Vec3::ZERO, -Vec3::Z, -Vec3::Z * 20.0));
        assert!(!power.contains(Vec3::ZERO, -Vec3::Z, -Vec3::Z * 20.01));
        assert!(power.contains(Vec3::ZERO, -Vec3::Z, Vec3::new(8.0, 0.0, -5.0)));
        assert!(!power.contains(Vec3::ZERO, -Vec3::Z, Vec3::new(9.0, 0.0, -5.0)));
        assert!(!power.contains(Vec3::ZERO, -Vec3::Z, Vec3::Z));
        assert!(!power.contains(Vec3::ZERO, -Vec3::Z, Vec3::ZERO));
        assert!(power.contains(Vec3::ZERO, -Vec3::Z, Vec3::new(2.0, 0.0, -0.5)));
        assert!(!power.contains(Vec3::ZERO, -Vec3::Z, Vec3::new(3.0, 0.0, -0.75)));
    }

    fn thrown() -> SaberThrow {
        SaberThrow {
            energy: 60.0,
            velocity: 30.0,
            outbound_seconds: 1.333,
            return_seconds: 0.8,
            damage: 400.0,
            width: 0.75,
            length: 1.2,
            width_from_edge: false,
            length_from_edge: true,
            rotation_rate: 4.0 * std::f32::consts::TAU,
        }
    }

    fn flight(seconds: f32) -> SaberFlight {
        SaberFlight {
            pos: Vec3::ZERO,
            velocity: -Vec3::Z * 30.0,
            basis: Mat4::IDENTITY,
            spin_axis: Vec3::Y,
            angle: 0.0,
            blade_class: 0,
            remaining: seconds,
            outbound_seconds: seconds,
            return_age: 0.0,
            returning: false,
            hit: Vec::new(),
        }
    }

    #[test]
    fn thrown_saber_uses_the_signed_last_outgoing_update_before_returning() {
        let mut class = thrown();
        class.outbound_seconds = 0.1;
        class.return_seconds = 0.2;
        class.velocity = 10.0;
        let mut saber = flight(0.1);
        saber.velocity = -Vec3::Z * 10.0;
        assert!(saber.advance(&class, Vec3::ZERO, 0.04));
        assert!(saber.advance(&class, Vec3::ZERO, 0.04));
        assert!((saber.pos.z + 0.8).abs() < 1e-5);
        assert!(saber.advance(&class, Vec3::ZERO, 0.04));
        assert!((saber.remaining + 0.02).abs() < 1e-5);
        assert!(
            (saber.pos.z + 0.6).abs() < 1e-5,
            "negative remainder reverses the final outgoing step"
        );
        assert!(saber.advance(&class, Vec3::ZERO, 0.04));
        assert!((saber.pos.z + 0.46666667).abs() < 1e-5);
        assert!(!saber.advance(&class, Vec3::X * 3.0, 0.2));
        assert_eq!(saber.pos, Vec3::X * 3.0);
    }

    #[test]
    fn a_world_clipped_throw_shortens_the_return_and_clears_hits_at_exact_zero() {
        let class = thrown();
        let mut saber = flight(class.outbound_seconds * 0.25);
        assert!((saber.return_duration(&class) - 0.2).abs() < 1e-6);
        saber.hit.push(crate::projectile::Target::Station(2));
        let exact = saber.remaining;
        assert!(saber.advance(&class, Vec3::ZERO, exact));
        assert_eq!(saber.remaining, 0.0);
        assert!(saber.hit.is_empty());
        assert!(saber.returning);
        assert!(!saber.advance(&class, Vec3::ZERO, 0.2));
    }
}
