//! Ordnance and explosion class data, damage descriptors and health application.
//! Sources and shared weapon rules are documented in the parent module.

use super::{pf, PropSource};

// ── Ordnance ──────────────────────────────────────────────────────────────

/// BF2-only time-based damage falloff (OrdnanceClass +0x34/+0x38/+0x3c;
/// key names uncracked, hashes below). Runs on bolt AGE, not distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Falloff {
    pub start_time: f32,
    pub duration: f32,
    /// < 0 disables (treated as full damage).
    pub end_damage: f32,
}

/// Uncracked BF2 ordnance keys (functionally identified, report §7.3).
pub const HASH_FALLOFF_START: u32 = 0xA745_61B2;
pub const HASH_FALLOFF_DURATION: u32 = 0xEE08_D1BA;
pub const HASH_FALLOFF_END_DAMAGE: u32 = 0x955F_71E9;
pub const HASH_TEAM_BONUS_SCALE: u32 = 0x9748_1656;

#[derive(Clone, Debug, PartialEq)]
pub struct OrdnanceClass {
    /// m/s along the spawn direction (OrdnanceLaser ctor default 150).
    pub velocity: f32,
    /// Seconds; max range = velocity × life_span (ctor default 2.0).
    pub life_span: f32,
    pub max_damage: f32,
    /// Per victim HealthType: [Person, Animal, Droid, Vehicle, Building,
    /// Building2, Building3] (class +0x50..+0x68 / BF2 +0x74..+0x8c).
    pub scales: [f32; 7],
    pub shield_scale: f32,
    pub falloff: Option<Falloff>,
    /// BF2 +0x40: extra fraction when the attacker's team holds the bonus.
    pub team_bonus_scale: f32,
    /// Sound property names (SOUND/<planet>.LVL snd_ scripts).
    pub ordnance_sound: Option<String>,
    pub collision_sound: Option<String>,
    pub geometry: Option<String>,
    /// The laser keys of a `bolt` / `laser` class (its own draw, card FXB);
    /// `None` means no laser ribbon; geometry-backed items use their own draw path.
    /// Render-only: the sim never reads it.
    pub laser: Option<crate::laser::LaserLook>,
    /// What its hits spawn (card FXP3, [`crate::impact`]); `None` only for a
    /// class built without the class db (a fixture) — an ordnance whose chain
    /// the bullet setter does not read carries just its trail. Render-only.
    pub impact: Option<crate::impact::ImpactLook>,
    /// A `sticky` class's own rules (card GREN, [`crate::sticky`]): gravity,
    /// the stick mask, the explosion. The firing path decides whether the
    /// sticky is thrown or launched; filled by the class loader.
    pub sticky: Option<crate::sticky::StickyClass>,
    /// The impact explosion (card RKEX, re-blaster-damage 7.3: `Explosion` /
    /// `ExplosionImpact` / `ExplosionName`, B1 class +0x38 / B2 +0x4c): what
    /// a struck bolt detonates beside its hit (B1 FUN_00272d70). `None` for
    /// an ordnance that names none - every rifle bolt. Filled by the class
    /// loader.
    pub explosion: Option<ExplosionClass>,
    /// The engine base, for the AI's choice ([`OrdKind`]).
    pub kind: OrdKind,
    /// `Gravity`: a shell falls at this x 9.8 m/s² ([`SHELL_G`];
    /// `analysis/re-ordnance-kinds.md` §6). The class loader reads it for a
    /// `shell` only: the key is dead on a bolt (the rifles author 1.0).
    pub gravity: f32,
    /// A shell's `ExplosionExpire`: its burst when its LifeSpan runs out in
    /// the air (§6). Filled by the class loader.
    pub expire: Option<ExplosionClass>,
    /// The ordnance's charge points after the root (a `NextCharge` in the
    /// ordnance class; issue #29, `analysis/re-weapon-charge.md` §4): the
    /// charge picks the segment it falls in and blends Velocity, LifeSpan,
    /// damage and its scales toward the segment's end ([`OrdnanceClass::charged`]).
    pub knots: Vec<OrdKnot>,
    /// An emitter's (the arc caster's) `Damage` beside its `MaxDamage`: its
    /// damage is lerp(Damage, MaxDamage, charge) (B1 `FUN_00278208`
    /// 199074-199075). None: `max_damage` whatever the charge.
    pub min_damage: Option<f32>,
    /// A `missile` class's steering keys (card UORD,
    /// [`crate::missile::MissileClass`]); `None` for every other base. Filled
    /// by the class loader.
    pub missile: Option<crate::missile::MissileClass>,
    pub fat_ray: Option<super::FatRayClass>,
    pub beam: Option<super::BeamClass>,
    pub tow: Option<crate::vehicle::tow::TowClass>,
}

impl Default for OrdnanceClass {
    fn default() -> Self {
        Self {
            velocity: 150.0,
            life_span: 2.0,
            max_damage: 0.0,
            // Ctor defaults for the scales are untraced; every shipped
            // ordnance sets them explicitly.
            scales: [1.0; 7],
            shield_scale: 1.0,
            falloff: None,
            team_bonus_scale: 0.0,
            ordnance_sound: None,
            collision_sound: None,
            geometry: None,
            laser: None,
            impact: None,
            sticky: None,
            explosion: None,
            kind: OrdKind::Other,
            gravity: 0.0,
            expire: None,
            knots: Vec::new(),
            min_damage: None,
            missile: None,
            fat_ray: None,
            beam: None,
            tow: None,
        }
    }
}

/// An ordnance class's engine base as the AI's weapon choice sorts it (card
/// HVYW, `analysis/re-soldier-ai.md` 4.3 / 22: a weapon too weak for a
/// vehicle may still be used on it by its ordnance class). Only the AI reads
/// it; filled by the class loader from the chain's engine label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrdKind {
    /// A bolt, a laser, a bullet, anything the choice has no rule for.
    #[default]
    Other,
    /// `missile`: the rocket and torpedo launchers, the wrist rocket.
    Missile,
    Shell,
    Beam,
    /// `grenade`, `sticky` and what the PS2 derives from sticky (`haywire`,
    /// `beacon`): used on a vehicle only when it moves slower than 12 m/s.
    Lobbed,
}

impl OrdKind {
    /// From the ordnance chain's engine label (`ClassDb::engine_base`).
    pub fn from_base(base: &str) -> Self {
        match base.to_ascii_lowercase().as_str() {
            "missile" => Self::Missile,
            "shell" => Self::Shell,
            "beam" => Self::Beam,
            "sticky" | "grenade" | "haywire" | "beacon" => Self::Lobbed,
            _ => Self::Other,
        }
    }
}

/// What TakeDamage receives (BF1 desc[4..12] / BF2 class+0x70..+0x94 copy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageDesc {
    pub damage: f32,
    pub scales: [f32; 7],
    pub shield_scale: f32,
}

impl OrdnanceClass {
    pub fn max_range(&self) -> f32 {
        self.velocity * self.life_span
    }

    /// Is this ordnance a TRACER - a round that exists only to be drawn? Every
    /// BF2 class a weapon names as its `SecondaryOrdnanceName` is one
    /// (`com_weap_inf_shotgun_tracer`, `com_weap_inf_chaingun_tracer`, ...:
    /// `MaxDamage 0`, no scales; `analysis/re-ordnance-kinds.md` §7). The
    /// exception on the discs is the probe droid's sonic blaster, whose
    /// "secondary" is a real 100-damage round, so the spawn is gated on this
    /// test rather than on the key's presence.
    pub fn is_tracer(&self) -> bool {
        self.max_damage <= 0.0
    }

    /// Damage descriptor at impact. `age` = bolt flight time (BF2 falloff);
    /// `team_bonus` = attacker team holds the +bonus (BF2 hash 0xbb03442e).
    /// Charge/NextCharge lerp omitted (rifles have none).
    /// The round's values at `charge` (B1 `FUN_002728a8` 195082-195133,
    /// `FUN_002762a0`, `FUN_00272f88`; re-weapon-charge §4): the last point
    /// at or under the charge (the root at 0), blended by `s` = the charge's
    /// place in its segment toward the next point (0 on the last); an
    /// emitter's damage by the raw charge instead.
    pub fn charged(&self, charge: f32) -> OrdBlend {
        let root = OrdBlend {
            velocity: self.velocity,
            life_span: self.life_span,
            damage: self.max_damage,
            scales: self.scales,
            shield_scale: self.shield_scale,
        };
        let point = |k: &OrdKnot| OrdBlend {
            velocity: k.velocity,
            life_span: k.life_span,
            damage: k.damage,
            scales: k.scales,
            shield_scale: k.shield_scale,
        };
        let mut pts = vec![(0.0, root)];
        pts.extend(self.knots.iter().map(|k| (k.at, point(k))));
        let i = pts.iter().rposition(|(at, _)| *at <= charge).unwrap_or(0);
        let mut out = pts[i].1;
        if let Some((t1, next)) = pts.get(i + 1) {
            let t0 = pts[i].0;
            let s = if *t1 > t0 {
                (charge - t0) / (t1 - t0)
            } else {
                0.0
            };
            let l = |a: f32, b: f32| a + (b - a) * s;
            out.velocity = l(out.velocity, next.velocity);
            out.life_span = l(out.life_span, next.life_span);
            out.damage = l(out.damage, next.damage);
            for (a, b) in out.scales.iter_mut().zip(next.scales) {
                *a = l(*a, b);
            }
            out.shield_scale = l(out.shield_scale, next.shield_scale);
        }
        if let Some(min) = self.min_damage {
            out.damage = min + (self.max_damage - min) * charge.clamp(0.0, 1.0);
        }
        out
    }

    pub fn damage_desc(&self, age: f32, team_bonus: bool) -> DamageDesc {
        self.damage_desc_charged(age, team_bonus, 1.0)
    }

    /// [`OrdnanceClass::damage_desc`] for a round fired at `charge`
    /// ([`OrdnanceClass::charged`]); the age falloff and team bonus are the
    /// root's (re-weapon-charge §4).
    pub fn damage_desc_charged(&self, age: f32, team_bonus: bool, charge: f32) -> DamageDesc {
        let blend = self.charged(charge);
        let mut damage = blend.damage;
        if let Some(f) = self.falloff {
            let t = age - f.start_time;
            if t > 0.0 {
                let frac = if f.duration > 0.0 {
                    (t / f.duration).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let end = if f.end_damage < 0.0 {
                    damage
                } else {
                    f.end_damage
                };
                damage += (end - damage) * frac;
            }
        }
        if team_bonus {
            damage += damage * self.team_bonus_scale;
        }
        DamageDesc {
            damage,
            scales: blend.scales,
            shield_scale: blend.shield_scale,
        }
    }

    /// Parse an `ordc` PROP list (BF1 FUN_002738a8/FUN_002774b0, BF2
    /// FUN_00310620/FUN_00314b98). Gravity is deliberately NOT read: only the
    /// `shell` class consumes it; bolts fly straight (report §5.1).
    pub fn from_props(src: &dyn PropSource) -> Self {
        let mut o = Self::default();
        if let Some(v) = pf(src, "Velocity") {
            o.velocity = v;
        }
        if let Some(v) = pf(src, "LifeSpan") {
            o.life_span = v;
        }
        if let Some(v) = pf(src, "MaxDamage").or_else(|| pf(src, "Damage")) {
            o.max_damage = v;
        }
        read_scales(src, &mut o.scales, &mut o.shield_scale);
        let fh = |h: u32| src.prop_hash(h).and_then(|s| s.trim().parse::<f32>().ok());
        if let Some(end) = fh(HASH_FALLOFF_END_DAMAGE) {
            o.falloff = Some(Falloff {
                start_time: fh(HASH_FALLOFF_START).unwrap_or(0.0),
                duration: fh(HASH_FALLOFF_DURATION).unwrap_or(0.0),
                end_damage: end,
            });
        }
        if let Some(v) = fh(HASH_TEAM_BONUS_SCALE) {
            o.team_bonus_scale = v;
        }
        o.ordnance_sound = src.prop("OrdnanceSound").map(|s| s.trim().to_owned());
        o.collision_sound = src.prop("CollisionSound").map(|s| s.trim().to_owned());
        o.geometry = src
            .prop("GeometryName")
            .map(|s| s.trim().to_ascii_lowercase());
        o
    }
}

/// The damage-scale props shared by ordnance and explosion classes (the
/// victim HealthType table + ShieldScale; re-vehicle-death §1: `HealthScale`
/// 0xc37970cf sets Person + Animal + Droid, `ArmorScale` 0xa0bb60ce Vehicle +
/// the three building scales). Legacy combos first, explicit keys override.
fn read_scales(src: &dyn PropSource, scales: &mut [f32; 7], shield: &mut f32) {
    if let Some(v) = pf(src, "HealthScale") {
        scales[0] = v;
        scales[1] = v;
        scales[2] = v;
    }
    if let Some(v) = src.prop_hash(0xA0BB_60CE).and_then(|s| s.parse().ok()) {
        for s in &mut scales[3..7] {
            *s = v;
        }
    }
    for (i, key) in [
        "PersonScale",
        "AnimalScale",
        "DroidScale",
        "VehicleScale",
        "BuildingScale",
    ]
    .iter()
    .enumerate()
    {
        if let Some(v) = pf(src, key) {
            scales[i] = v;
            if i == 4 {
                scales[5] = v;
                scales[6] = v;
            }
        }
    }
    if let Some(v) = pf(src, "ShieldScale") {
        *shield = v;
    }
}

// ── Explosion class ───────────────────────────────────────────────────────

/// How far an ordnance's impact explosion sits off the hit, metres: B1
/// FUN_00272d70 detonates at hit + 0.2·v (DAT_005182a8 = 0.2). PROVISIONAL
/// [H]: v is the handler's fourth argument, unidentified (a normal is
/// likely); the port backs off along the bolt. The sim's blast
/// (`Sim::resolve_bolt`) and the drawn explosion (`game::fx`) both use it.
pub const EXPLOSION_OFFSET: f32 = 0.2;

/// An `expc` class's damage and push numbers (`analysis/re-vehicle-death.md`
/// §1 [O]: B1 ctor FUN_0026ed88 / setter FUN_0026f528, B2 FUN_0030b2f8 /
/// FUN_0030bb88). What `Sim::explode` applies (§2-§4); the presentation props
/// (Effect, chunks, light, shake) are the game layer's.
#[derive(Clone, Debug, PartialEq)]
pub struct ExplosionClass {
    /// `Damage` / `MaxDamage` (ctor 0).
    pub damage: f32,
    /// Per victim HealthType, as [`OrdnanceClass::scales`] (ctor 1.0 each).
    pub scales: [f32; 7],
    pub shield_scale: f32,
    /// `DamageRadiusInner` (ctor 0) and `DamageRadius` = `DamageRadiusOuter`
    /// (ctor 0 — no area damage).
    pub damage_inner: f32,
    pub damage_outer: f32,
    /// `Push` and `PushRadius` = `PushRadiusOuter` (ctor 0), applied by
    /// `Sim::explode` (card KNOCK, `analysis/re-knockback.md`). The disc's
    /// `PushRadiusInner` is not read: the push fades over the whole radius
    /// (`sim::blast::knock_falloff`).
    pub push: f32,
    pub push_outer: f32,
    /// The push moves only bodies, never a living soldier (a named
    /// deviation, `game::roster::PUSH_SPARES_THE_LIVING`); the damage is
    /// untouched.
    pub spares_living: bool,
    /// `HurtOwner` (flags bit 1 at class+0xa0, ctor 1,
    /// `analysis/re-vehicle-death.md` §1): when clear, the blast leaves out
    /// its owner - B2 FUN_0030c8f8 skips the gathered object that is the
    /// damage info's owner, for the damage and the push alike
    /// (B2:300612-300633). Mace's ground crack and Sidious's blast author 0;
    /// a combo's PlayExplosion passes its soldier as the owner (FUN_00340350,
    /// B2:335530).
    pub hurt_owner: bool,
    /// Its `Effect` / `WaterEffect` (card FXP3, [`crate::impact`]).
    /// Render-only: the sim never reads it; filled by the game's class
    /// loader.
    pub look: crate::impact::ExplosionLook,
}

impl Default for ExplosionClass {
    fn default() -> Self {
        Self {
            damage: 0.0,
            scales: [1.0; 7],
            shield_scale: 1.0,
            damage_inner: 0.0,
            damage_outer: 0.0,
            push: 0.0,
            push_outer: 0.0,
            spares_living: false,
            hurt_owner: true,
            look: crate::impact::ExplosionLook::default(),
        }
    }
}

impl ExplosionClass {
    /// Parse an `expc` PROP list (§1's table; each radius's two names set the
    /// same field).
    pub fn from_props(src: &dyn PropSource) -> Self {
        let mut e = Self::default();
        if let Some(v) = pf(src, "MaxDamage").or_else(|| pf(src, "Damage")) {
            e.damage = v;
        }
        read_scales(src, &mut e.scales, &mut e.shield_scale);
        let any = |keys: [&str; 2]| keys.iter().find_map(|k| pf(src, k));
        if let Some(v) = pf(src, "DamageRadiusInner") {
            e.damage_inner = v;
        }
        if let Some(v) = any(["DamageRadiusOuter", "DamageRadius"]) {
            e.damage_outer = v;
        }
        if let Some(v) = pf(src, "Push") {
            e.push = v;
        }
        if let Some(v) = any(["PushRadiusOuter", "PushRadius"]) {
            e.push_outer = v;
        }
        if let Some(v) = pf(src, "HurtOwner") {
            e.hurt_owner = v != 0.0;
        }
        e
    }

    /// The descriptor every victim's TakeDamage receives before the falloff
    /// (§2 step 3: the class's Damage, 7 scales and ShieldScale).
    pub fn desc(&self) -> DamageDesc {
        DamageDesc {
            damage: self.damage,
            scales: self.scales,
            shield_scale: self.shield_scale,
        }
    }

    /// Half-side of the gather box (§2 step 3): max(DamageOuter, PushOuter).
    pub fn gather_radius(&self) -> f32 {
        self.damage_outer.max(self.push_outer)
    }
}

/// The blast falloff (B1 FUN_00221530 B1:135586-135596, B2 FUN_00268fe0,
/// §3 [O]): `d' = d − w/2` for a victim sphere of radius `w` whose centre is
/// `d` from the blast; 1 inside `inner`, 0 at or past `outer`, linear between.
pub fn blast_falloff(d: f32, w: f32, inner: f32, outer: f32) -> f32 {
    let d = d - w * 0.5;
    if d < inner {
        1.0
    } else if d >= outer {
        0.0
    } else {
        (outer - d) / (outer - inner)
    }
}

// ── Health ────────────────────────────────────────────────────────────────

/// Victim class for the damage scale table (health component flags word,
/// low 3 bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HealthType {
    Person = 0,
    Animal = 1,
    Droid = 2,
    Vehicle = 3,
    Building = 4,
    Building2 = 5,
    Building3 = 6,
}

impl HealthType {
    /// ODF `HealthType` strings.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "person" | "soldier" => Self::Person,
            "animal" => Self::Animal,
            "droid" => Self::Droid,
            "vehicle" => Self::Vehicle,
            "building" => Self::Building,
            "building2" => Self::Building2,
            "building3" => Self::Building3,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub cur: f32,
    pub max: f32,
    pub shield: f32,
    pub shield_max: f32,
    pub health_type: HealthType,
}

impl Health {
    pub fn new(max: f32, health_type: HealthType) -> Self {
        Self {
            cur: max,
            max,
            shield: 0.0,
            shield_max: 0.0,
            health_type,
        }
    }

    pub fn alive(&self) -> bool {
        self.cur > 0.0
    }

    /// TakeDamage (BF2 FUN_001dfb38 / BF1 FUN_001bc190). `multiplier` is the
    /// caller's factor (1.0 for direct bolt hits [H]); `friendly_fire_scale`
    /// = DAT_00517508 / DAT_005fcbe0. Returns the health damage applied.
    pub fn take_damage(
        &mut self,
        desc: &DamageDesc,
        multiplier: f32,
        friendly: bool,
        friendly_fire_scale: f32,
    ) -> f32 {
        let mut dmg = desc.damage * multiplier;
        if friendly {
            dmg *= friendly_fire_scale;
        }
        if self.shield > 0.0 && desc.shield_scale > 0.0 && dmg > 0.0 {
            // BF2 FUN_001dfb38: shield phase first, only for positive
            // shield-scaled damage. Excess reaches health with its type scale.
            let s = dmg * desc.shield_scale;
            if s <= self.shield {
                self.shield -= s;
                return 0.0;
            }
            let carried = (s - self.shield) / s;
            self.shield = 0.0;
            dmg *= carried;
        }
        dmg *= desc.scales[self.health_type as usize];
        self.cur -= dmg;
        dmg
    }
}
/// One ordnance charge point ([`OrdnanceClass::knots`]): from charge `at`
/// the values blend toward the next point's.
#[derive(Clone, Debug, PartialEq)]
pub struct OrdKnot {
    pub at: f32,
    pub velocity: f32,
    pub life_span: f32,
    pub damage: f32,
    pub scales: [f32; 7],
    pub shield_scale: f32,
}

/// [`OrdnanceClass::charged`]'s values for one round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrdBlend {
    pub velocity: f32,
    pub life_span: f32,
    pub damage: f32,
    pub scales: [f32; 7],
    pub shield_scale: f32,
}

/// The shell's gravity unit: `v.y += Gravity x -9.8 x dt` each update (B1
/// `FUN_0027e478` 203497-203542, B2 `FUN_003202d0`; re-ordnance-kinds §6).
pub const SHELL_G: f32 = 9.8;
