//! A hover vehicle's class numbers, typed: the drive / spring props of the
//! PS2 hover class (analysis/re-vehicles.md §3.1) with its constructor
//! defaults, the seat ring its sections build (§5), and the damage-effect
//! slots and `ExplosionName` of its hull (§6). Read from a
//! [`ClassDb`] — a side class (`imp_hover_speederbike`) or a map-local one
//! (`tat_hover_skiff`, `ClassDb::load_map`).
//!
//! A vehicle ODF is ONE ordered prop list split by `FLYERSECTION` lines:
//! props before any section, or after `FLYERSECTION = BODY`, are the
//! vehicle's own (hover drive + the pilot seat); `TURRET1..8` open a
//! MountedTurret gunner seat whose props (its own `TurnRate`, `PitchLimits`,
//! weapons) never reach the hover (B1 FUN_001f0648 B1:102492-102537).
//! `CHUNKSECTION` only picks the destruction chunk later `Chunk*` keys fill
//! (B2 FUN_00214600 B2:126998-127046, the base setter every vehicle setter
//! falls through to); it closes no section, so the body's rows after it
//! (the snail tank's `DamageStartPercent` rows, B2 FUN_0026a340
//! B2:186058, read with no section state) are still the body's.
//!
//! A building turret (`armedbuilding` / `armedbuildingdynamic`, §8, card
//! VTURB) splits its list the same way with `BuildingSection` (hash
//! 0xd89b1a5c: B1 FUN_001c4db8 B1:70347, B2 FUN_001e8948 B2:97129), into the
//! same TURRET1..8 table and MountedTurret setter; it has no seat of its own,
//! so its ring is its gunner sections alone ([`seats`]).
//!
//! BASE chains: the PS2 derives a class from its parent by a copy that keeps
//! the scalar props but RESETS the spring points to none and the
//! velocity/omega springs to 15/5/3/1/3/1 (B1 FUN_001f00e0 B1:101527-101534,
//! B2 FUN_00221e08 B2:136938-136944), so here scalars inherit child-over-
//! parent and the springs come from the child-most def alone. Every shipped
//! vehicle derives straight from its engine base, so the entc-parent case is
//! [H] and untested on data.

mod aim;
mod flyer;
pub mod tread;
mod walker;
pub use aim::{AimerSpec, BarrelSlot};
pub(crate) use flyer::flyer_exhaust;
pub use flyer::{FlyerClass, FlyerExhaust};
pub use walker::{WalkerClass, WALKER_BASES};

use crate::classdb::{fnv1a_lowercase, ClassDb, ClassDef};
use core::vehicle::hover::{HoverParams, HullBody, HullShape, JumpParams, Spring};
use core::weapon::{floats, HealthType};

/// Engine bases that construct the PS2 hover class (`treaded` IS a hover on
/// both discs, re-vehicles §2; BF1's commandhover reuses the integrator §3).
pub const HOVER_BASES: [&str; 3] = ["hover", "treaded", "commandhover"];

/// Engine bases of a building turret (§8): BF1's `armedbuilding`, BF2's and
/// its `armedbuildingdynamic` subclass (`portableturret`, `defensegridturret`).
pub const TURRET_BASES: [&str; 2] = ["armedbuilding", "armedbuildingdynamic"];

/// Is `class` a building turret ([`TURRET_BASES`])?
pub fn is_turret(db: &ClassDb, class: &str) -> bool {
    db.engine_base(class).is_some_and(is_turret_base)
}

pub(crate) fn is_turret_base(base: &str) -> bool {
    TURRET_BASES.iter().any(|t| t.eq_ignore_ascii_case(base))
}

/// At most this many spring points (`if (5 < count) return`, B1:102074).
pub const MAX_SPRING_POINTS: usize = 6;

/// Vehicle sound properties authored in the ODF chain (card VSND / card UCRY).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VehicleSoundClass {
    pub collision_sound: Option<String>,
    pub hurt_sound: Option<String>,
    pub death_sound: Option<String>,
    pub cockpit_sound: Option<String>,
}

impl VehicleSoundClass {
    pub fn from_db(db: &ClassDb, class: &str) -> Self {
        Self {
            collision_sound: crate::roster::sound_key(db, class, "VehicleCollisionSound"),
            hurt_sound: crate::roster::sound_key(db, class, "HurtSound"),
            death_sound: crate::roster::sound_key(db, class, "DeathSound"),
            cockpit_sound: crate::roster::sound_key(db, class, "Cockpit3rdPersonSound")
                .or_else(|| crate::roster::sound_key(db, class, "Cockpit1stPersonSound")),
        }
    }
}
/// MountedTurret sections per vehicle (TURRET1..8, class+0x2678+4i).
pub const MAX_TURRET_SECTIONS: usize = 8;
/// `PassengerSlots` is clamped to 0..=4 (B1:102135-102150).
pub const MAX_PASSENGERS: usize = 4;
/// Weapons one seat carries (the MountedTurret setter's four, §5). The pilot
/// seat's cap is the same number, [H].
pub use core::vehicle::gun::MAX_SEAT_WEAPONS;

/// One `AddSpringBody = "x y z r"` point and its per-point modifiers, as the
/// PS2's entry init FUN_001f2030 (B1:102605) leaves them: spring length
/// 0.5·r (`DAT_00517828` = 0.5, its only writer B1:101892), the velocity and
/// both damp factors 1, and the two omega SPRING factors a lever-arm sign —
/// −sign(z) for X (pitch) and sign(x) for Z (roll), 0 inside ±0.1. BF2's
/// entry init FUN_00224368 (B2:138140-138183) is the same (re-hover §10 [O]).
pub use core::vehicle::hover::Spring as SpringPoint;

fn spring_point(pos: [f32; 3], radius: f32) -> Spring {
    let sign = |v: f32| {
        if v > 0.1 {
            1.0
        } else if v < -0.1 {
            -1.0
        } else {
            0.0
        }
    };
    Spring {
        pos: pos.into(),
        radius,
        length: radius * 0.5,
        velocity_spring: 1.0,
        velocity_damp: 1.0,
        omega_x_spring: -sign(pos[2]),
        omega_x_damp: 1.0,
        omega_z_spring: sign(pos[0]),
        omega_z_damp: 1.0,
    }
}

/// Every §3.1 hover prop, defaulted to the PS2 constructor's value (B1
/// FUN_001efdd8 B1:101257-101290 = B2 FUN_00221a20 B2:136636-136668) or the
/// derive copy's (the six body springs, `SpringPoint` list). Units are the
/// ODF's except `pitch_limits`, which the setter turns from degrees to
/// radians (B1:101859-101860). BF1's PC* rates are not carried (PC-only);
/// BF2's boost, energy and jump props (re-hover §1, §6.3, §6.9) are — BF1
/// authors none, so its classes neither boost nor jump.
#[derive(Clone, Debug, PartialEq)]
pub struct HoverClass {
    motion: HoverParams,
    pub lift_spring: f32,
    pub lift_damp: f32,
    pub level_damp: f32,
    /// Radians, [min, max].
    pub pitch_limits: [f32; 2],
    /// The bodies in the class model's TERRAIN and BUILDING subsets, by
    /// lowercase name in the order named (re-hover §13.2 [O]): `GeometryName`
    /// empties both; a `TerrainCollision` / `BuildingCollision` line adds its
    /// body (`none` / `clear` empties that layer); every `AddSpringBody`
    /// names its own sphere, `springbody<n>`, on both lines — so a class with
    /// a spring holds exactly the bodies named, never the layer default.
    pub terrain_bodies: Vec<String>,
    pub building_bodies: Vec<String>,
    /// The soldier, vehicle and ordnance lines and the `AddSpringBody`
    /// spheres in PROP order, for [`Self::spring_subsets`].
    spring_steps: Vec<SpringStep>,
    /// BF2's tread scroll ([`tread`]); empty on every BF1 class.
    pub treads: tread::Treads,
}

/// One PROP of [`HoverClass::spring_subsets`]'s replay.
#[derive(Clone, Debug, PartialEq)]
enum SpringStep {
    /// `GeometryName`: the generic parser resets the layer accumulators
    /// (B2:127747-127766, B1:92633-92651).
    Geometry,
    /// `SoldierCollision` (0), `VehicleCollision` (1) or
    /// `OrdnanceCollision` (2) naming a body, lowercased.
    Line(usize, String),
    /// An `AddSpringBody` sphere joins the model.
    Spring,
}

/// The [`SpringStep::Line`] index of each subset key.
const SPRING_STEP_KEYS: [&str; 3] = ["SoldierCollision", "VehicleCollision", "OrdnanceCollision"];

/// `SetSpringBody` (re-hover §13.2): a spring on an existing primitive.
const SET_SPRING_BODY: u32 = 0x577b_739e;
/// The name [`HoverClass`] lists an `AddSpringBody` sphere under.
const SPRING_BODY: &str = "springbody";

/// One `TerrainCollision` / `BuildingCollision` line (B2 FUN_00216198 /
/// FUN_00215fb8): the layer's accumulator gains the body, or empties.
fn name_body(list: &mut Vec<String>, v: &str) {
    let v = v.trim().to_ascii_lowercase();
    if v == "none" || v == "clear" {
        list.clear();
    } else if !list.contains(&v) {
        list.push(v);
    }
}

impl Default for HoverClass {
    fn default() -> Self {
        Self {
            motion: HoverParams {
                set_altitude: 1.0,
                gravity_scale: 1.0,
                acceleration: 10.0,
                deceleration: 5.0,
                traction: 10.0,
                forward_speed: 20.0,
                reverse_speed: 10.0,
                strafe_speed: 15.0,
                spin_rate: 3.0,
                turn_rate: 2.0,
                pitch_rate: 0.5,
                turn_filter: 10.0,
                thrust_pitch_angle: 0.0,
                strafe_roll_angle: 0.0,
                bank_angle: 0.0,
                bank_filter: 2.0,
                level_spring: 10.0,
                collision_threshold: [7.5, 2.5, 0.0],
                collision_scale: [1.0, 0.0, 0.0],
                velocity_spring: 15.0,
                velocity_damp: 5.0,
                omega_x_spring: 3.0,
                omega_x_damp: 1.0,
                omega_z_spring: 3.0,
                omega_z_damp: 1.0,
                boost_speed: 0.0,
                boost_turn_multiplier: 0.5,
                boost_acceleration: 0.0,
                energy_auto_restore: 0.0,
                energy_boost_drain: 0.0,
                jump: JumpParams::default(),
                random_spring: true,
                floats_on_water: true,
                springs: Vec::new(),
                bf1_ai: false,
                hull: Vec::new(),
            },
            lift_spring: 5.0,
            lift_damp: 2.0,
            pitch_limits: [-0.5, 0.5],
            level_damp: 3.0,
            terrain_bodies: Vec::new(),
            building_bodies: Vec::new(),
            spring_steps: Vec::new(),
            treads: tread::Treads::default(),
        }
    }
}

/// Which part of the ODF a prop belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// The vehicle itself: hover drive and the pilot seat.
    Body,
    /// MountedTurret section index 0..=7 (TURRET / TURRET1 → 0, TURRETn → n−1).
    Turret(u8),
}

const SLOW_OFFSET: u32 = 0x5dbc_abb0;
const FAST_OFFSET: u32 = 0x8d0c_fc36;
const FAST_FOV: u32 = 0xaaa6_d95c;

fn h(name: &str) -> u32 {
    fnv1a_lowercase(name)
}

/// The FLYERSECTION value → scope: TURRET / TURRET1 → 0, TURRET2..8 → 1..7,
/// anything else → the body (B1:102492-102537).
fn section_scope(value: &str) -> Scope {
    let v = value.trim().to_ascii_uppercase();
    match v.strip_prefix("TURRET") {
        Some("") | Some("1") => Scope::Turret(0),
        Some(n) => match n.parse::<u8>() {
            Ok(k @ 2..=8) => Scope::Turret(k - 1),
            _ => Scope::Body,
        },
        None => Scope::Body,
    }
}

/// One def's props with the scope each falls in, in file order. A building
/// turret's `BuildingSection` and a walker's `WalkerSection` (B2
/// FUN_00263f68 B2:182865-182869, card WALK) open the same scopes as
/// `FLYERSECTION`; inside a `WalkerSection` only the props the turret setter
/// chain claims ([`walker::turret_claims`]) are the section's, the rest the
/// body's.
fn scoped(def: &ClassDef) -> Vec<(Scope, u32, &str)> {
    let (flyer, building) = (h("FLYERSECTION"), h("BuildingSection"));
    let walker = h("WalkerSection");
    let mut scope = Scope::Body;
    let mut by_walker = false;
    let mut out = Vec::with_capacity(def.props.len());
    for (k, v) in &def.props {
        if *k == flyer || *k == building || *k == walker {
            scope = section_scope(v);
            by_walker = *k == walker;
        } else if by_walker && matches!(scope, Scope::Turret(_)) && !walker::turret_claims(*k) {
            // The walker setter gives the turret setter first refusal
            // (B2:182865-182869); what it does not claim is the walker's.
            out.push((Scope::Body, *k, v.as_str()));
        } else {
            out.push((scope, *k, v.as_str()));
        }
    }
    out
}

/// Fill `out` from as many numbers as the line gives, as the PS2's
/// three-`%f` sscanf does (B1:101776, B1:101816): the skiff's
/// `CollisionScale = "30.0"` sets only the first of three.
fn fill(out: &mut [f32], v: &str) {
    for (slot, x) in out.iter_mut().zip(v.split_whitespace()) {
        match x.parse() {
            Ok(x) => *slot = x,
            Err(_) => break,
        }
    }
}

/// The first number of a value; ODF lines can carry a trailing `//comment`
/// (the skiff's `BankFilter = "3//low"`), which the PS2's sscanf stops at.
pub(crate) fn float(v: &str) -> Option<f32> {
    let end = v
        .trim_start()
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(v.trim_start().len());
    v.trim_start()[..end].parse().ok()
}

fn tension_pair(v: &str) -> Option<[f32; 2]> {
    floats::<2>(v).or_else(|| float(v).map(|x| [x, x]))
}

impl HoverClass {
    /// `class`'s hover numbers, or `None` if the db lacks it or its BASE
    /// chain does not end at a [`HOVER_BASES`] engine class.
    pub fn from_db(db: &ClassDb, class: &str) -> Option<Self> {
        if !HOVER_BASES.contains(&db.engine_base(class)?) {
            return None;
        }
        let mut c = Self::default();
        // Root first, so the child's value is the last write.
        for def in db.chain(class).into_iter().rev() {
            // The derive copy (FUN_001f00e0): springs reset per class.
            let d = Self::default();
            c.motion.springs.clear();
            c.terrain_bodies.clear();
            c.building_bodies.clear();
            c.spring_steps.clear();
            c.treads.derive();
            c.motion.velocity_spring = d.motion.velocity_spring;
            c.motion.velocity_damp = d.motion.velocity_damp;
            c.motion.omega_x_spring = d.motion.omega_x_spring;
            c.motion.omega_x_damp = d.motion.omega_x_damp;
            c.motion.omega_z_spring = d.motion.omega_z_spring;
            c.motion.omega_z_damp = d.motion.omega_z_damp;
            for (scope, k, v) in scoped(def) {
                if scope == Scope::Body {
                    c.set(k, v);
                }
            }
        }
        Some(c)
    }

    /// The non-spring bodies that meet the world (card HHUL, re-hover §13):
    /// each name in [`Self::terrain_bodies`] / [`Self::building_bodies`] that
    /// is no spring, resolved against the class geometry's baked primitives
    /// `prims` (the model's body list starts with them, in bake order; the
    /// first of a name wins) at its baked model-space transform: the bake
    /// composes `XFRM` with the `PRNT` bone's bind matrix (card RKHUL,
    /// `analysis/re-walker.md` §17). A name no primitive has is dropped:
    /// the PS2 writes nothing for it, and `CollisionMesh` never collides as
    /// the mover (§13.1). On the discs: `p_cube1`, buildings only, on the two
    /// BF2 fighter tanks. PROVISIONAL: a `SetSpringBody` primitive (no
    /// shipped class) is a hull body here, not a spring; a layer emptied
    /// after a spring (none either) still has its springs.
    pub fn hull(&self, prims: &[manifest::ManifestPrim]) -> Vec<HullBody> {
        let mut out = Vec::new();
        let (terrain, building) = self.hull_names();
        for p in prims {
            let name = p.name.to_ascii_lowercase();
            let (t, b) = (
                terrain.contains(&name.as_str()),
                building.contains(&name.as_str()),
            );
            // The first primitive of a name is the one a line finds.
            let first = prims
                .iter()
                .find(|q| q.name.eq_ignore_ascii_case(&p.name))
                .is_some_and(|q| std::ptr::eq(q, p));
            if !(t || b) || !first {
                continue;
            }
            let shape = match p.kind.as_str() {
                "sphere" => HullShape::Sphere { radius: p.dims[0] },
                // `dims[1]` is the FULL height (mungefs::coll).
                "cylinder" => HullShape::Cylinder {
                    radius: p.dims[0],
                    half_height: p.dims[1] * 0.5,
                },
                "box" => HullShape::Box {
                    half: p.dims.into(),
                },
                _ => continue,
            };
            let m = crate::viewer::instance_matrix(p.rotation, p.translation);
            out.push(HullBody {
                shape,
                pos: m.w_axis.truncate(),
                rot: glam::Mat3::from_mat4(m),
                terrain: t,
                building: b,
            });
        }
        out
    }

    /// The names in the terrain and the building subset that are no
    /// `AddSpringBody` sphere.
    pub fn hull_names(&self) -> (Vec<&str>, Vec<&str>) {
        fn hull(l: &[String]) -> Vec<&str> {
            l.iter()
                .map(String::as_str)
                .filter(|n| !n.starts_with(SPRING_BODY))
                .collect()
        }
        (hull(&self.terrain_bodies), hull(&self.building_bodies))
    }

    /// Does a line name a body that is no spring ([`Self::hull`] may then
    /// hold one)?
    pub fn names_a_hull_body(&self) -> bool {
        let (t, b) = self.hull_names();
        !(t.is_empty() && b.is_empty())
    }

    /// The sim's copy (`core::vehicle::hover`, card HOVR), with no hull
    /// bodies: [`Self::hull`] needs the geometry's primitives
    /// (`vehicle_pads::install` fills them in).
    pub fn params(&self) -> HoverParams {
        self.motion.clone()
    }

    /// The authored spring points in property order.
    pub fn springs(&self) -> &[SpringPoint] {
        &self.motion.springs
    }

    /// Whether each [`Self::springs`] sphere ends in the class model's
    /// soldier, vehicle and ordnance subsets (`analysis/re-hover.md` §13.8).
    /// `bodies` are the lowercased names of the bodies the geometry gave the
    /// model before any spring: `collisionmesh` when it has a mesh, and each
    /// primitive.
    ///
    /// The sphere joins with MASK 0, which ORs it into all five subsets
    /// (B2 `FUN_00167048`, B2:5917-5966). A later line of a layer then
    /// replaces that subset with its accumulator, so the sphere stays only if
    /// a line named it (B2 `FUN_00216378` / `FUN_002160a8` / `FUN_00216288`);
    /// `none` / `clear` empties it, and a name no body has writes nothing.
    /// `VehicleCollisionOnly` is read only by `FUN_0022f6e8` (B2:145579),
    /// which the hover setter never calls.
    pub fn spring_subsets(&self, bodies: &[String]) -> Vec<[bool; 3]> {
        let mut names: Vec<String> = bodies.to_vec();
        let mut acc: [Vec<String>; 3] = Default::default();
        let mut out: Vec<[bool; 3]> = Vec::new();
        let spring = |j: usize| format!("{SPRING_BODY}{}", j + 1);
        for step in &self.spring_steps {
            match step {
                SpringStep::Geometry => acc.iter_mut().for_each(Vec::clear),
                SpringStep::Spring => {
                    names.push(spring(out.len()));
                    out.push([true; 3]);
                }
                SpringStep::Line(k, v) => {
                    if v == "none" || v == "clear" {
                        acc[*k].clear();
                    } else if names.contains(v) {
                        if !acc[*k].contains(v) {
                            acc[*k].push(v.clone());
                        }
                    } else {
                        continue;
                    }
                    for (j, s) in out.iter_mut().enumerate() {
                        s[*k] = acc[*k].contains(&spring(j));
                    }
                }
            }
        }
        out
    }

    fn set(&mut self, k: u32, v: &str) {
        if self.treads.set(k, v) {
            return;
        }
        if k == h("NoRandomSpring") {
            self.motion.random_spring = false;
            return;
        }
        if k == h("FloatsOnWater") {
            self.motion.floats_on_water = walker::strtol_nonzero(v);
            return;
        }
        let scalars: [(&str, &mut f32); 34] = [
            ("SetAltitude", &mut self.motion.set_altitude),
            ("GravityScale", &mut self.motion.gravity_scale),
            ("LiftSpring", &mut self.lift_spring),
            ("LiftDamp", &mut self.lift_damp),
            ("Acceleration", &mut self.motion.acceleration),
            ("Deceleration", &mut self.motion.deceleration),
            ("Traction", &mut self.motion.traction),
            ("ForwardSpeed", &mut self.motion.forward_speed),
            ("ReverseSpeed", &mut self.motion.reverse_speed),
            ("StrafeSpeed", &mut self.motion.strafe_speed),
            ("SpinRate", &mut self.motion.spin_rate),
            ("TurnRate", &mut self.motion.turn_rate),
            ("PitchRate", &mut self.motion.pitch_rate),
            ("TurnFilter", &mut self.motion.turn_filter),
            ("ThrustPitchAngle", &mut self.motion.thrust_pitch_angle),
            ("StrafeRollAngle", &mut self.motion.strafe_roll_angle),
            ("BankAngle", &mut self.motion.bank_angle),
            ("BankFilter", &mut self.motion.bank_filter),
            ("LevelSpring", &mut self.motion.level_spring),
            ("LevelDamp", &mut self.level_damp),
            ("VelocitySpring", &mut self.motion.velocity_spring),
            ("VelocityDamp", &mut self.motion.velocity_damp),
            ("OmegaXSpring", &mut self.motion.omega_x_spring),
            ("OmegaXDamp", &mut self.motion.omega_x_damp),
            ("OmegaZSpring", &mut self.motion.omega_z_spring),
            ("OmegaZDamp", &mut self.motion.omega_z_damp),
            ("BoostSpeed", &mut self.motion.boost_speed),
            (
                "BoostTurnMultiplier",
                &mut self.motion.boost_turn_multiplier,
            ),
            ("BoostAcceleration", &mut self.motion.boost_acceleration),
            ("EnergyAutoRestore", &mut self.motion.energy_auto_restore),
            ("EnergyBoostDrain", &mut self.motion.energy_boost_drain),
            ("JumpTimeMin", &mut self.motion.jump.time_min),
            ("JumpTimeMax", &mut self.motion.jump.time_max),
            ("JumpForce", &mut self.motion.jump.force),
        ];
        // The two jump props whose names are not known, by hash.
        let unnamed: [(u32, &mut f32); 2] = [
            (0x534d_fe50, &mut self.motion.jump.speed_factor),
            (0x391e_36a1, &mut self.motion.jump.energy_drain),
        ];
        let named = scalars.into_iter().map(|(name, slot)| (h(name), slot));
        for (key, slot) in named.chain(unnamed) {
            if k == key {
                if let Some(x) = float(v) {
                    *slot = x;
                }
                return;
            }
        }
        if k == h("PitchLimits") {
            if let Some([lo, hi]) = floats::<2>(v) {
                self.pitch_limits = [lo.to_radians(), hi.to_radians()];
            }
        } else if k == h("CollisionThreshold") {
            fill(&mut self.motion.collision_threshold, v);
        } else if k == h("CollisionScale") {
            fill(&mut self.motion.collision_scale, v);
        } else if k == h("GeometryName") {
            // The generic parser resets the layer accumulators (§13.2 step 1).
            self.terrain_bodies.clear();
            self.building_bodies.clear();
            self.spring_steps.push(SpringStep::Geometry);
        } else if let Some(layer) = SPRING_STEP_KEYS.iter().position(|n| k == h(n)) {
            self.spring_steps
                .push(SpringStep::Line(layer, v.trim().to_ascii_lowercase()));
        } else if k == h("TerrainCollision") {
            name_body(&mut self.terrain_bodies, v);
        } else if k == h("BuildingCollision") {
            name_body(&mut self.building_bodies, v);
        } else if k == SET_SPRING_BODY {
            name_body(&mut self.building_bodies, v);
            name_body(&mut self.terrain_bodies, v);
        } else if k == h("AddSpringBody") {
            if self.motion.springs.len() < MAX_SPRING_POINTS {
                if let Some([x, y, z, r]) = floats::<4>(v) {
                    self.motion.springs.push(spring_point([x, y, z], r));
                    // The sphere names itself on both lines (B2:137561-137564).
                    let name = format!("{SPRING_BODY}{}", self.motion.springs.len());
                    name_body(&mut self.building_bodies, &name);
                    name_body(&mut self.terrain_bodies, &name);
                    self.spring_steps.push(SpringStep::Spring);
                }
            }
        } else if let Some(p) = self.motion.springs.last_mut() {
            // The per-point modifiers write the LAST point (B1:102272).
            let slot = match k {
                _ if k == h("BodySpringLength") => &mut p.length,
                _ if k == h("BodyVelocitySpringFactor") => &mut p.velocity_spring,
                _ if k == h("BodyVelocityDampFactor") => &mut p.velocity_damp,
                _ if k == h("BodyOmegaXSpringFactor") => &mut p.omega_x_spring,
                _ if k == h("BodyOmegaXDampFactor") => &mut p.omega_x_damp,
                _ if k == h("BodyOmegaZSpringFactor") => &mut p.omega_z_spring,
                _ if k == h("BodyOmegaZDampFactor") => &mut p.omega_z_damp,
                _ => return,
            };
            if let Some(x) = float(v) {
                *slot = x;
            }
        }
    }
}

/// `PilotType` (Controllable +0x21c): who drives from the seat. The value is
/// hashed like a key (B1 FUN_0023da58 B1:156006-156035); an unknown value
/// leaves the default. BF2 adds `vehicleself`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilotType {
    None = 0,
    SelfPilot = 1,
    /// A soldier boards the seat — the only kind the enter path accepts.
    Vehicle = 2,
    Remote = 3,
    VehicleSelf = 4,
}

impl PilotType {
    /// BF2's Controllable ctor writes 2 (B2:284987). BF1's default is [D]:
    /// the skiff's four seats name no PilotType and are boardable, which
    /// needs 2 (§5 seat choice).
    pub const DEFAULT: PilotType = PilotType::Vehicle;

    fn parse(v: &str) -> Option<Self> {
        let k = h(v.trim());
        [
            ("none", Self::None),
            ("self", Self::SelfPilot),
            ("vehicle", Self::Vehicle),
            ("remote", Self::Remote),
            ("vehicleself", Self::VehicleSelf),
        ]
        .into_iter()
        .find(|(n, _)| h(n) == k)
        .map(|(_, t)| t)
    }
}

/// Where a seat sits in the ring — the sim's own enum, so a class's ring
/// feeds `core::vehicle::VehicleSpec::seats` as it is.
pub use core::vehicle::SeatKind;

/// One weapon a seat fires, `WeaponName` with the `WeaponAmmo` after it
/// (ammo < 1 = infinite, §5).
#[derive(Clone, Debug, PartialEq)]
pub struct SeatWeapon {
    pub class: String,
    pub ammo: i32,
}

/// One seat of the ring (§5): the pilot, the gunner sections in index order,
/// then the passengers — at most 1 + 8 + 4.
#[derive(Clone, Debug, PartialEq)]
pub struct SeatSpec {
    pub kind: SeatKind,
    pub pilot_type: PilotType,
    /// `PilotPosition`: the hardpoint the rider is posed at.
    pub pilot_position: Option<String>,
    /// `AnimatedPilotPosition` (card VRIDE, `analysis/re-vehicle-riders.md`
    /// 2.1 [O]): a node the rider takes the matrix of as it is animated now,
    /// with no hardpoint offset (the tauntaun's, the kaadu's, BF2's one-man
    /// AT-ST's).
    pub animated_pilot_position: Option<String>,
    /// `EyePointOffset` (x y z), the seat camera's eye.
    pub eye_point_offset: Option<[f32; 3]>,
    /// The seat's third-person camera (card VSEAT; how the PS2 combines them
    /// is untraced, `crate::walk::vehicle_camera` is PROVISIONAL):
    /// `TrackCenter` and `TrackOffset` (x y z, the body frame, +z forward),
    /// `TiltValue` (degrees) and `ThirdPersonFOV` (degrees).
    pub track_center: Option<[f32; 3]>,
    pub track_offset: Option<[f32; 3]>,
    pub slow_offset: Option<[f32; 3]>,
    pub fast_offset: Option<[f32; 3]>,
    pub fast_fov: Option<f32>,
    pub tilt_value: Option<f32>,
    pub aim_tension: Option<f32>,
    pub move_tension: [Option<[f32; 2]>; 3],
    pub third_person_fov: Option<f32>,
    /// `BoostFOV` (degrees; a hover's BODY, B2 K+0xf74): the FOV the hover's
    /// camera widens to as its forward speed runs from `ForwardSpeed` to
    /// `BoostSpeed` (B2 `FUN_00221388`).
    pub boost_fov: Option<f32>,
    /// `FirstPersonFOV` (degrees): a building turret's zoomed view takes it
    /// (card VTURZ: B1 `FUN_00225ba0`, B2 `FUN_002703d0` read the class's
    /// +0x1bc when the zoom flag is set; the Controllable's default is 48).
    pub first_person_fov: Option<f32>,
    pub weapons: Vec<SeatWeapon>,
    /// Card VTUR (§5, §9): `FirePointName` (the node a bolt leaves),
    /// `TurretNodeName` (the node the aimer turns), `YawLimits` /
    /// `PitchLimits` (degrees, "lo hi") and `YawRate` / `PitchRate` (rad/s).
    pub fire_point: Option<String>,
    pub turret_node: Option<String>,
    pub yaw_limits: Option<[f32; 2]>,
    pub pitch_limits: Option<[f32; 2]>,
    pub yaw_rate: Option<f32>,
    pub pitch_rate: Option<f32>,
    /// Card VTURD (`analysis/re-turret-draw.md` 2.1): what the drawn turret
    /// turns by. `PitchTurnFactor` scales the pitch the `TurretNodeName`
    /// node takes (the class default is 1.0; every soldier-seat turret but
    /// one authors 0.0); `AimerNodeName` is the node that looks along the
    /// aimer's direction; each `BarrelNodeName` is a node that slides back
    /// at a shot by its `BarrelRecoil` (`BarrelLength` writes a quarter of
    /// itself into the same field).
    pub pitch_turn_factor: Option<f32>,
    pub aimer_node: Option<String>,
    pub barrels: Vec<(String, f32)>,
    /// Card VTURG (`analysis/re-turret-draw.md` 3.1, 3.3): the seated
    /// soldier's pose key - `PilotAnimation` (a clip played) or
    /// `Pilot9Pose` (nine frames blended by the gun's angles), the clip
    /// being `human_<value>` - and `IsPilotExposed` (default 1).
    pub pilot_pose: Option<PilotPose>,
    pub pilot_exposed: bool,
    /// Card VTURL (`analysis/re-turret-draw.md` 2.2, 2.3): the MountedTurret's
    /// turn law - the full-stick rates (`MaxTurnSpeed` or its alias
    /// `TurnRate`, `MaxPitchSpeed` or `PitchRate`: the last authored wins,
    /// B2:191503-191506, 191751-191754), `TurnFilter` / `PitchFilter`, and
    /// the FIRST aimer's `AimerYawLimits` / `AimerPitchLimits` (degrees) and
    /// `AimerYawRate` / `AimerPitchRate` (rad/s; B1 `FUN_0026e260`).
    pub turn_speed: Option<f32>,
    pub pitch_speed: Option<f32>,
    pub turn_filter: Option<f32>,
    pub pitch_filter: Option<f32>,
    pub aimer_yaw_limits: Option<[f32; 2]>,
    pub aimer_pitch_limits: Option<[f32; 2]>,
    pub aimer_yaw_rate: Option<f32>,
    pub aimer_pitch_rate: Option<f32>,
    /// A second `AimerNodeName` was read: later aimer keys are its.
    aimer_second: bool,
    pub aimers: Vec<AimerSpec>,
}

/// A seat's pose key: the clip `human_<name>`, and whether it is a nine-pose.
#[derive(Clone, Debug, PartialEq)]
pub struct PilotPose {
    pub name: String,
    pub nine: bool,
}

impl PilotPose {
    /// The clip the soldier is posed with.
    pub fn clip(&self) -> String {
        format!("human_{}", self.name.to_ascii_lowercase())
    }
}

/// What the PS2 does with a seated soldier (`analysis/re-turret-draw.md`
/// 3.1 [O]): BF1's seat slot draws him when the seat has a pose key and takes
/// him out of the scene otherwise (B1:121244-121254); BF2 also hides him on
/// `IsPilotExposed = 0` (B2:167517-167526). A seat no soldier boards
/// (`PilotType` self / remote / none: the sim's `SeatSetup::boardable`) has
/// nobody to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GunnerDraw {
    Drawn,
    Hidden,
    NoSoldier,
}

/// Where a vehicle seat's drawn rider hangs in the vehicle's model frame
/// (card VRIDE, `analysis/re-vehicle-riders.md` 2.1, 3 [O]): the PS2's seat
/// function puts him at `L x E x O`, which on a vehicle drawn rigid is the
/// `PilotPosition` hardpoint's whole model-space BIND matrix (a turret-node
/// gunner's at the turret's rest, as the drawn gun is, until card VPOSE);
/// an `AnimatedPilotPosition` rider at that node's matrix NOW (`E x O`).
#[derive(Clone, Debug, PartialEq)]
pub enum RiderHang {
    /// The hardpoint's bind matrix in the model frame.
    Bind(glam::Mat4),
    /// The animated node, by name, and its bind matrix for a vehicle not
    /// drawn posed.
    Node(String, glam::Mat4),
}

impl RiderHang {
    /// The hang of `seat` on a model whose `skel` nodes are `nodes`. A seat
    /// naming no node the model has sits at the model's origin: the PS2's
    /// seat keeps its default L there, [H] (re-vehicle-riders 7: BF1's
    /// hovernaut, whose `hp_pilot` is not in its skeleton).
    pub fn of(seat: &SeatSpec, nodes: &[manifest::ManifestHardpoint]) -> RiderHang {
        let bind = |name: Option<&str>| {
            let name = name?.trim();
            let n = nodes.iter().find(|n| n.name.eq_ignore_ascii_case(name))?;
            let ax = n
                .axes
                .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
            Some(glam::Mat4::from_cols(
                glam::Vec3::from(ax[0]).extend(0.0),
                glam::Vec3::from(ax[1]).extend(0.0),
                glam::Vec3::from(ax[2]).extend(0.0),
                glam::Vec3::from(n.pos).extend(1.0),
            ))
        };
        match &seat.animated_pilot_position {
            Some(node) => RiderHang::Node(
                node.trim().to_string(),
                bind(Some(node)).unwrap_or(glam::Mat4::IDENTITY),
            ),
            None => RiderHang::Bind(
                bind(seat.pilot_position.as_deref()).unwrap_or(glam::Mat4::IDENTITY),
            ),
        }
    }
}

impl SeatSpec {
    pub fn new(kind: SeatKind) -> Self {
        Self {
            kind,
            pilot_type: PilotType::DEFAULT,
            pilot_position: None,
            animated_pilot_position: None,
            eye_point_offset: None,
            track_center: None,
            track_offset: None,
            slow_offset: None,
            fast_offset: None,
            fast_fov: None,
            tilt_value: None,
            aim_tension: None,
            move_tension: [None; 3],
            third_person_fov: None,
            boost_fov: None,
            first_person_fov: None,
            weapons: Vec::new(),
            fire_point: None,
            turret_node: None,
            yaw_limits: None,
            pitch_limits: None,
            yaw_rate: None,
            pitch_rate: None,
            pitch_turn_factor: None,
            aimer_node: None,
            barrels: Vec::new(),
            pilot_pose: None,
            pilot_exposed: true,
            turn_speed: None,
            pitch_speed: None,
            turn_filter: None,
            pitch_filter: None,
            aimer_yaw_limits: None,
            aimer_pitch_limits: None,
            aimer_yaw_rate: None,
            aimer_pitch_rate: None,
            aimer_second: false,
            aimers: Vec::new(),
        }
    }

    /// Is a soldier in this seat drawn ([`GunnerDraw`])? `bf2`: the class
    /// is a BF2 disc's (BF1 reads `IsPilotExposed` for the body only).
    pub fn gunner_draw(&self, bf2: bool) -> GunnerDraw {
        if !matches!(self.pilot_type, PilotType::Vehicle | PilotType::VehicleSelf) {
            GunnerDraw::NoSoldier
        } else if self.pilot_pose.is_none() || (bf2 && !self.pilot_exposed) {
            GunnerDraw::Hidden
        } else {
            GunnerDraw::Drawn
        }
    }

    fn set(&mut self, k: u32, v: &str) {
        let weapon = self.weapons.len().saturating_sub(1);
        if k == h("NextAimer") {
            self.aimers.push(AimerSpec::new("", weapon));
        } else if k == h("AimerNodeName") {
            match self.aimers.last_mut() {
                Some(a) if a.unnamed() && a.weapon == weapon => a.name(v),
                _ => self.aimers.push(AimerSpec::new(v, weapon)),
            }
        } else if let Some(a) = self.aimers.last_mut() {
            a.set(k, v);
        }
        if k == h("PilotType") {
            if let Some(t) = PilotType::parse(v) {
                self.pilot_type = t;
            }
        } else if k == h("PilotPosition") {
            self.pilot_position = Some(v.trim().to_string());
        } else if k == h("AnimatedPilotPosition") {
            self.animated_pilot_position = Some(v.trim().to_string());
        } else if k == h("PilotAnimation") || k == h("Pilot9Pose") {
            // One slot for both keys: the last one authored wins.
            self.pilot_pose = Some(PilotPose {
                name: v.trim().to_string(),
                nine: k == h("Pilot9Pose"),
            });
        } else if k == h("IsPilotExposed") {
            self.pilot_exposed = float(v).is_none_or(|x| x != 0.0);
        } else if k == h("EyePointOffset") {
            self.eye_point_offset = floats::<3>(v);
        } else if k == h("TrackCenter") {
            self.track_center = floats::<3>(v);
        } else if k == h("TrackOffset") {
            self.track_offset = floats::<3>(v);
        } else if k == SLOW_OFFSET {
            self.slow_offset = floats::<3>(v);
        } else if k == FAST_OFFSET {
            self.fast_offset = floats::<3>(v);
        } else if k == FAST_FOV {
            self.fast_fov = float(v);
        } else if k == h("TiltValue") {
            self.tilt_value = v.trim().parse().ok();
        } else if k == h("AimTension") {
            self.aim_tension = float(v);
        } else if k == h("MoveTension") {
            self.move_tension = [float(v).map(|x| [x, x]); 3];
        } else if k == h("MoveTensionX") {
            self.move_tension[0] = tension_pair(v);
        } else if k == h("MoveTensionY") {
            self.move_tension[1] = tension_pair(v);
        } else if k == h("MoveTensionZ") {
            self.move_tension[2] = tension_pair(v);
        } else if k == h("ThirdPersonFOV") {
            self.third_person_fov = v.trim().parse().ok();
        } else if k == h("BoostFOV") {
            self.boost_fov = float(v);
        } else if k == h("FirstPersonFOV") {
            self.first_person_fov = v.trim().parse().ok();
        } else if k == h("WeaponName") {
            if self.weapons.len() < MAX_SEAT_WEAPONS {
                self.weapons.push(SeatWeapon {
                    class: v.trim().to_string(),
                    ammo: 0,
                });
                self.aimers.push(AimerSpec::new("", self.weapons.len() - 1));
            }
        } else if k == h("FirePointName") {
            self.fire_point = Some(v.trim().to_string());
        } else if k == h("TurretNodeName") {
            self.turret_node = Some(v.trim().to_string());
        } else if k == h("YawLimits") {
            self.yaw_limits = floats::<2>(v);
        } else if k == h("PitchLimits") {
            self.pitch_limits = floats::<2>(v);
        } else if k == h("YawRate") {
            self.yaw_rate = float(v);
        } else if k == h("PitchRate") {
            self.pitch_rate = float(v);
            self.pitch_speed = float(v);
        } else if k == h("MaxPitchSpeed") {
            self.pitch_speed = float(v);
        } else if k == h("MaxTurnSpeed") || k == h("TurnRate") {
            self.turn_speed = float(v);
        } else if k == h("TurnFilter") {
            self.turn_filter = float(v);
        } else if k == h("PitchFilter") {
            self.pitch_filter = float(v);
        } else if k == h("AimerYawLimits") && !self.aimer_second {
            self.aimer_yaw_limits = floats::<2>(v);
        } else if k == h("AimerPitchLimits") && !self.aimer_second {
            self.aimer_pitch_limits = floats::<2>(v);
        } else if k == h("AimerYawRate") && !self.aimer_second {
            self.aimer_yaw_rate = float(v);
        } else if k == h("AimerPitchRate") && !self.aimer_second {
            self.aimer_pitch_rate = float(v);
        } else if k == h("PitchTurnFactor") {
            self.pitch_turn_factor = float(v);
        } else if k == h("AimerNodeName") {
            // One aimer chain per weapon section; the first is the drawn one.
            if self.aimer_node.is_none() {
                self.aimer_node = Some(v.trim().to_string());
            } else {
                self.aimer_second = true;
            }
        } else if k == h("BarrelNodeName") {
            self.barrels.push((v.trim().to_string(), 0.0));
        } else if k == h("BarrelRecoil") || k == h("BarrelLength") {
            if let (Some(b), Some(x)) = (self.barrels.last_mut(), float(v)) {
                b.1 = if k == h("BarrelLength") { 0.25 * x } else { x };
            }
        } else if k == h("WeaponAmmo") {
            if let (Some(w), Ok(a)) = (self.weapons.last_mut(), v.trim().parse()) {
                w.ammo = a;
            }
        }
    }
}

/// Engine bases whose vehicles keep their team when the last rider leaves
/// (re-vehicles §7.1): B1 clears class+0x80 (default 1, FUN_00221b28
/// B1:135870) for commandhover / commandwalker (B1:224878, 230522); B2 clears
/// class+0x330 bit 34 (default set, FUN_00269d98 B2:186494) for commandflyer /
/// commandhover / commandwalker (B2:348465, 348966, 354410) and `mine`.
pub const KEEPS_TEAM_BASES: [&str; 4] = ["commandhover", "commandwalker", "commandflyer", "mine"];

/// Damage-effect slots per class: 10 × 0x30 bytes from class+0x90 (B1
/// FUN_00221e48, the `DamageStartPercent` case, B1:136004-136025).
pub const MAX_DAMAGE_EFFECTS: usize = 10;

/// One visual damage slot (re-vehicles §6): an effect played at a hardpoint
/// while the hull's health fraction is in its band. Fractions, not percents:
/// the setter stores the ODF value × 0.01 (B1:136023, 136048).
#[derive(Clone, Debug, PartialEq)]
pub struct DamageEffect {
    /// `DamageStartPercent` × 0.01 (slot +0x0).
    pub start: f32,
    /// `DamageStopPercent` × 0.01 (slot +0x4); a new Start resets it to 0
    /// (B1:136022).
    pub stop: f32,
    /// `DamageEffect`: the fx class (slot +0x8).
    pub effect: String,
    /// `DamageAttachPoint`: a hardpoint or bone (slot +0xc).
    pub attach: Option<String>,
    /// `DamageEffectScale` (slot +0x20), default 1.0.
    pub scale: f32,
    /// `DamageInheritVelocity`, default 0 [H].
    pub inherit_velocity: f32,
}

impl DamageEffect {
    /// Is the effect on at `health_frac` (health / max)? PROVISIONAL: the
    /// runtime reader is not found (re-vehicles §6 [H]); this is the disc's
    /// plain reading, `stop < f ≤ start` — the speeder bike sparks at 60 %
    /// and stops at 20 %; a 0 stop runs until the hull dies.
    pub fn active(&self, health_frac: f32) -> bool {
        health_frac <= self.start && health_frac > self.stop
    }
}

/// `class`'s damage-effect slots, in slot order. The setter's slot pointer
/// (DAT_00517a68): `DamageStartPercent` takes the first slot whose effect is
/// still unset — a Start with no Effect is overwritten by the next — and
/// every other damage prop writes the slot it picked; with all 10 slots
/// holding an effect, the rest are dropped. The pointer is a global the
/// PS2 never resets between classes; here it starts empty per class, and
/// the rows a later class writes through it (BF2's guided-rocket ordnance
/// authors damage rows before any Start) come from
/// [`ClassDb::replay_damage_loads`], applied after each def's own rows.
/// BODY scope (portable turrets also author these after TURRET1), over the
/// BASE chain root first.
pub fn damage_effects(db: &ClassDb, class: &str) -> Vec<DamageEffect> {
    // An empty `effect` is the PS2's null effect: the slot is still free.
    let mut slots: Vec<DamageEffect> = Vec::new();
    let mut cur: Option<usize> = None;
    // Portable turrets author these building properties after TURRET1.
    let portable = db.engine_base(class) == Some("portableturret");
    let flyer = db
        .engine_base(class)
        .is_some_and(|base| FLYER_BASES.contains(&base));
    for def in db.chain(class).into_iter().rev() {
        let mut own: Vec<Option<usize>> = Vec::new();
        for (scope, k, v) in scoped(def) {
            if scope != Scope::Body && !portable && !flyer {
                continue;
            }
            if k == h("DamageStartPercent") {
                own.push(None);
                cur = slots.iter().position(|s| s.effect.is_empty());
                if cur.is_none() && slots.len() < MAX_DAMAGE_EFFECTS {
                    slots.push(DamageEffect {
                        start: 0.0,
                        stop: 0.0,
                        effect: String::new(),
                        attach: None,
                        scale: 1.0,
                        inherit_velocity: 0.0,
                    });
                    cur = Some(slots.len() - 1);
                }
                if let Some(i) = cur {
                    slots[i].start = float(v).unwrap_or(0.0) * 0.01;
                    slots[i].stop = 0.0;
                }
                if let Some(o) = own.last_mut() {
                    *o = cur;
                }
                continue;
            }
            if let Some(s) = cur.map(|i| &mut slots[i]) {
                set_damage_row(s, k, v);
            }
        }
        for (n, k, v) in db.damage_patches(&def.name) {
            if let Some(&Some(i)) = own.get(*n) {
                set_damage_row(&mut slots[i], *k, v);
            }
        }
    }
    slots.retain(|s| !s.effect.is_empty());
    slots
}

/// One damage row other than `DamageStartPercent` into slot `s`.
fn set_damage_row(s: &mut DamageEffect, k: u32, v: &str) {
    match k {
        _ if k == h("DamageStopPercent") => s.stop = float(v).unwrap_or(0.0) * 0.01,
        _ if k == h("DamageEffect") => s.effect = v.trim().to_owned(),
        _ if k == h("DamageAttachPoint") => s.attach = Some(v.trim().to_owned()),
        _ if k == h("DamageEffectScale") => s.scale = float(v).unwrap_or(0.0),
        _ if k == h("DamageInheritVelocity") => s.inherit_velocity = float(v).unwrap_or(0.0),
        _ => {}
    }
}

pub fn prop_float(db: &ClassDb, class: &str, key: &str) -> Option<f32> {
    db.prop(class, key).and_then(float)
}

pub fn prop_flag(db: &ClassDb, class: &str, key: &str) -> bool {
    prop_float(db, class, key).is_some_and(|v| v != 0.0)
}

pub fn has_chunks(db: &ClassDb, class: &str) -> bool {
    let chunk_geom = h("ChunkGeometryName");
    db.chain(class).iter().any(|def| {
        def.props
            .iter()
            .any(|(k, v)| *k == chunk_geom && !v.trim().is_empty())
    }) || explosion_name(db, class).is_some_and(|exp| {
        db.chain(&exp).iter().any(|def| {
            def.props
                .iter()
                .any(|(k, v)| *k == chunk_geom && !v.trim().is_empty())
        })
    })
}

/// The class detonated when its hull pool runs out (`ExplosionDestruct` / `ExplosionName`,
/// re-vehicles §6, SWBFRONT lines 89800-89830, SWBFRNT2 lines 80169-80185), if it names one.
pub fn explosion_name(db: &ClassDb, class: &str) -> Option<String> {
    db.prop(class, "ExplosionDestruct")
        .or_else(|| db.prop(class, "ExplosionName"))
        .or_else(|| db.prop(class, "Explosion"))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The ODF collision-primitive line (`BldgCylinder aacylinder NULL x y z r h
/// …`), hash 0x8cf29543 — its name is not recovered; the classes RON keeps
/// the hash.
pub const HASH_COLLISION_PRIMITIVE: u32 = 0x8cf2_9543;

/// The sphere a blast measures `class` by (card VDTH,
/// `analysis/re-vehicle-death.md` §3 [O]): the centre is the model's SPHR
/// centre `sphr` (model space; the origin without one), the radius `w =
/// max(SPHR r, the farthest body)` — the EntityGeometry ctor B1 FUN_001e1e18
/// (B1:91424-91462) over class+0x1498, which grows by `|p − c| + e` for each
/// ODF collision primitive (FUN_001e3678 B1:93183-93266: `e = r` for a
/// sphere, `√(2r² + h²/4)` for a cylinder, `√(x² + y² + z²)` of the half
/// sizes for a box) and each hover `AddSpringBody` (`e = r`, FUN_001f0648
/// B1:102072-102125). The kind strings are matched by name here: the PS2
/// compares CRCs, and only `sphere` and `aacylinder` occur on the discs
/// ([D]: the AT-ST's aacylinder reproduces its RAM w to 3e-5).
pub fn blast_sphere(db: &ClassDb, class: &str, sphr: Option<[f32; 4]>) -> ([f32; 3], f32) {
    let [cx, cy, cz, r0] = sphr.unwrap_or([0.0; 4]);
    let dist = |p: [f32; 3]| {
        let (x, y, z) = (p[0] - cx, p[1] - cy, p[2] - cz);
        (x * x + y * y + z * z).sqrt()
    };
    let mut w = r0;
    for line in db.props_all_hash(class, HASH_COLLISION_PRIMITIVE) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let n: Vec<f32> = t.iter().skip(3).filter_map(|x| x.parse().ok()).collect();
        if t.len() < 2 || n.len() < 4 {
            continue;
        }
        let (a, b, c) = (
            n[3],
            n.get(4).copied().unwrap_or(0.0),
            n.get(5).copied().unwrap_or(0.0),
        );
        let kind = t[1].to_ascii_lowercase();
        let e = if kind.contains("cylinder") {
            (2.0 * a * a + b * b * 0.25).sqrt()
        } else if kind.contains("box") {
            (a * a + b * b + c * c).sqrt()
        } else {
            a
        };
        w = w.max(dist([n[0], n[1], n[2]]) + e);
    }
    if let Some(h) = HoverClass::from_db(db, class) {
        for s in h.springs() {
            w = w.max(dist(s.pos.to_array()) + s.radius);
        }
    }
    ([cx, cy, cz], w)
}

/// Engine bases that construct the PS2 flyer class (`carrier` is a flyer,
/// re-vehicles §2).
pub const FLYER_BASES: [&str; 3] = ["flyer", "commandflyer", "carrier"];

/// A flyer class's `LandedHeight` (card VREST, `analysis/re-vehicle-rest.md`
/// §2 [O]): how far over the pad's spawn point its constructor stands the
/// origin. The class starts at its parent's value (the root's 0) and its
/// props are applied in file order: `GeometryName` stores minus the min y of
/// that model's `modl INFO` vis box when the model is loaded (B1
/// FUN_001df3f0 B1:89911-89916, B2 FUN_0020fd58 B2:125205-125208) and
/// `LandedHeight` stores the authored number (B1:90030-90031,
/// B2:124871-124872), so the later of the two wins. `vis_min_y` answers a
/// class's baked vis-box floor by its lowercase name
/// (`ManifestVehicle::vis_min_y`). `None` when `class` is not a flyer.
pub fn landed_height(
    db: &ClassDb,
    class: &str,
    vis_min_y: impl Fn(&str) -> Option<f32>,
) -> Option<f32> {
    let base = db.engine_base(class)?;
    if !FLYER_BASES.iter().any(|b| b.eq_ignore_ascii_case(base)) {
        return None;
    }
    let mut height = 0.0;
    for def in db.chain(class).into_iter().rev() {
        for (scope, k, v) in scoped(def) {
            if scope != Scope::Body {
                continue;
            }
            if k == h("GeometryName") {
                if let Some(y) = vis_min_y(&def.name.to_ascii_lowercase()) {
                    height = -y;
                }
            } else if k == h("LandedHeight") {
                if let Some(x) = float(v) {
                    height = x;
                }
            }
        }
    }
    Some(height)
}

/// Does `class` go back to team 0 (neutral) when its last rider leaves? True
/// for every class but a [`KEEPS_TEAM_BASES`] one (and for an unknown class:
/// the PS2 default).
pub fn neutral_when_empty(db: &ClassDb, class: &str) -> bool {
    db.engine_base(class)
        .is_none_or(|b| !KEEPS_TEAM_BASES.iter().any(|k| k.eq_ignore_ascii_case(b)))
}

/// A BF1 vehicle's slice time: BF1 authors none, so each class takes its BF2
/// counterpart's `TimeRequiredToEject` and a vehicle is as hard to slice on
/// every map (`[ours]`, one game). Rows: (BF1 class, the BF2 class, seconds).
/// A class with no row keeps its base's default, which is what its BF2
/// counterpart has too (tauntaun, gunship, MAF, Jedi starfighter).
/// `tests/suite/vehicle_class.rs` holds the rows to BF2's baked classes.
pub const BF1_SLICE_TIMES: &[(&str, &str, f32)] = &[
    ("cis_hover_aat", "cis_hover_aat", 10.0),
    ("cis_hover_stap", "cis_hover_stap", 1.0),
    ("cis_tread_hailfire", "cis_tread_hailfire", 5.0),
    ("cis_walk_spider", "cis_walk_spider", 12.5),
    ("imp_hover_fightertank", "imp_hover_fightertank", 7.5),
    ("rep_hover_fightertank", "rep_hover_fightertank", 7.5),
    ("all_hover_combatspeeder", "all_hover_combatspeeder", 7.5),
    ("imp_hover_speederbike", "imp_hover_speederbike", 1.0),
    // BF2's Republic bike is the BARC speeder.
    ("rep_hover_speederbike", "rep_hover_barcspeeder", 1.0),
    ("imp_walk_atst", "imp_walk_atst", 10.0),
    ("imp_walk_atst_jungle", "imp_walk_atst_jungle", 10.0),
    ("imp_walk_atst_snow", "imp_walk_atst_snow", 10.0),
    ("all_fly_xwing", "all_fly_xwing_sc", 2.5),
    ("all_fly_ywing", "all_fly_ywing_sc", 2.5),
    ("all_fly_snowspeeder", "all_fly_snowspeeder", 2.5),
    ("imp_fly_tiefighter", "imp_fly_tiefighter_sc", 2.5),
    ("imp_fly_tiebomber", "imp_fly_tiebomber_sc", 2.5),
    ("cis_fly_droidfighter", "cis_fly_droidfighter_sc", 2.5),
    // PROVISIONAL: BF2 has no Geonosian fighter; every BF2 starfighter is
    // 2.5 s. replace: the maintainer's pick after slicing one in play.
    ("cis_fly_geofighter", "cis_fly_droidfighter_sc", 2.5),
];

/// Issue #35: the cutter's slice time, s: `TimeRequiredToEject` over the
/// class chain, else a BF1 class's [`BF1_SLICE_TIMES`] row, else the PS2
/// class table's default for its base (hover 3, walker 12, flyer 4;
/// B2:136736, 182455, 124430). 0 for every other base (command walkers and
/// hovers, turrets): never sliced. BF2's rule on both games.
pub fn slice_time(db: &ClassDb, class: &str) -> f32 {
    let base = db.engine_base(class).map(str::to_ascii_lowercase);
    let default = match base.as_deref() {
        Some("hover") => 3.0,
        Some("walker") => 12.0,
        Some("flyer") => 4.0,
        _ => return 0.0,
    };
    let twin = (!db.is_bf2())
        .then(|| {
            BF1_SLICE_TIMES
                .iter()
                .find(|(bf1, ..)| bf1.eq_ignore_ascii_case(class))
        })
        .flatten()
        .map_or(default, |row| row.2);
    db.prop(class, "TimeRequiredToEject")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map_or(twin, |t| t.max(0.0))
}

/// `[ours]` A BF1 flyer's `PilotSkillRepairScale` is its BF2 twin's (the
/// twins of [`BF1_SLICE_TIMES`]): BF1 authors 0.03 to 0.04 on its fighters,
/// BF2 0.02 on every fighter and 0.01 on the gunship. Every ground vehicle
/// of both games authors none and takes the 0.005 default. measure: a
/// Technician in a damaged BF1 X-wing against a BF2 snowspeeder, in play.
pub const BF1_REPAIR_SCALES: &[(&str, f32)] = &[
    ("all_fly_xwing", 0.02),
    ("all_fly_ywing", 0.02),
    ("all_fly_snowspeeder", 0.02),
    ("imp_fly_tiefighter", 0.02),
    ("imp_fly_tiebomber", 0.02),
    ("cis_fly_droidfighter", 0.02),
    ("cis_fly_geofighter", 0.02),
    ("rep_fly_jedifighter", 0.02),
    ("rep_fly_gunship", 0.01),
];

/// The share of its maximum health class `class` repairs a second while a
/// Technician (BF2 medal 3) drives it: `PilotSkillRepairScale` over the
/// class chain, BF2's constructor default 0.005 without one (B2
/// `FUN_00269410` reads class+0x2fc), and a BF1 flyer its BF2 twin's
/// ([`BF1_REPAIR_SCALES`]).
pub fn repair_scale(db: &ClassDb, class: &str) -> f32 {
    let default = core::sim::DEFAULT_REPAIR_SCALE;
    if !db.is_bf2() {
        return BF1_REPAIR_SCALES
            .iter()
            .find(|(bf1, _)| bf1.eq_ignore_ascii_case(class))
            .map_or(default, |row| row.1);
    }
    prop_float(db, class, "PilotSkillRepairScale").map_or(default, |s| s.max(0.0))
}

/// `[ours]` The vehicle classes whose weak spot deals x4 on a ground map
/// (`core::sim::HEAVY_WALKER_WEAK_SPOT_FACTOR`): the AT-TE, the AT-AT and
/// the spider walker, the same names in both games, and BF2's Kashyyyk
/// spider (a class of its own, not a child of the spider). A class whose
/// BASE chain reaches one (a map's own subclass; none is baked today) is one
/// too. Every other class's weak spot deals x2. The maintainer 2026-11-10:
/// "ATTE, atat, and spider walker: make weak spot hits x4. Everything else
/// x2 and that seems good." measure: maintainer playtest of rockets to kill
/// (`analysis/re-ordnance-kinds.md` §5.1).
pub const HEAVY_WALKERS: &[&str] = &[
    "rep_walk_atte",
    "imp_walk_atat",
    "cis_walk_spider",
    "cis_walk_spiderkas",
];

/// The factor a strike on a weak part of `class` deals on a ground map
/// (`core::sim::Sim::weak_spot_factor`): x4 when `class` or its BASE chain
/// is one of [`HEAVY_WALKERS`], else x2.
pub fn weak_spot_factor(db: &ClassDb, class: &str) -> f32 {
    let heavy = |name: &str| HEAVY_WALKERS.iter().any(|w| w.eq_ignore_ascii_case(name));
    if heavy(class) || db.chain(class).iter().any(|d| heavy(&d.name)) {
        core::sim::HEAVY_WALKER_WEAK_SPOT_FACTOR
    } else {
        core::sim::GROUND_WEAK_SPOT_FACTOR
    }
}

/// The hull pool's `HealthType`: the ODF's over the BASE chain, else the
/// vehicle default (B1 FUN_00221e48 B1:136172-136195) — BF1's tauntaun is
/// "animal". A building turret defaults to `building`: its class ctor B1
/// FUN_001c4cd0 (B1:70300) runs the building ctor FUN_001c08b0, which stores
/// 4 at class+0x48 (B1:67215) where a vehicle ctor stores 3 (B1:101255).
pub fn health_type(db: &ClassDb, class: &str) -> HealthType {
    let default = if is_turret(db, class) {
        HealthType::Building
    } else {
        HealthType::Vehicle
    };
    db.prop(class, "HealthType")
        .and_then(HealthType::parse)
        .unwrap_or(default)
}

/// The class's `AISizeType` mask (B1 FUN_00221e48 B1:136104-136134, BF2
/// FUN_0026a340): the value is compared case-insensitively (FUN_0042bdd8, the
/// ctype table's upper-case bit adds 0x20) against "SOLDIER" "HOVER" "SMALL"
/// "MEDIUM" "HUGE" "FLYER" (ELF 0x533ec0, 8 B apart) → 1, 2, 4, 8, 0x10, 0x20;
/// SOLDIER, an unknown word and an absent key all give 1. The discs author
/// "hover" and "Huge" as well as the upper-case words.
pub fn ai_size(db: &ClassDb, class: &str) -> u32 {
    const WORDS: [(&str, u32); 5] = [
        ("HOVER", 2),
        ("SMALL", 4),
        ("MEDIUM", 8),
        ("HUGE", 0x10),
        ("FLYER", 0x20),
    ];
    db.prop(class, "AISizeType")
        .and_then(|v| {
            let v = v.trim();
            WORDS
                .iter()
                .find(|(w, _)| w.eq_ignore_ascii_case(v))
                .map(|&(_, m)| m)
        })
        .unwrap_or(1)
}

/// `class`'s seat ring in ring order, or empty if the db lacks it. Seats
/// merge over the BASE chain root first (a child's section re-authors its
/// parent's same-index seat) — [H], no shipped vehicle has an entc parent.
/// A building turret's ring is its gunner sections only ([`is_turret`]).
pub fn seats(db: &ClassDb, class: &str) -> Vec<SeatSpec> {
    let mut pilot = SeatSpec::new(SeatKind::Pilot);
    let mut turrets: [Option<SeatSpec>; MAX_TURRET_SECTIONS] = Default::default();
    let mut passengers = 0usize;
    let chain = db.chain(class);
    if chain.is_empty() {
        return Vec::new();
    }
    for def in chain.into_iter().rev() {
        for (scope, k, v) in scoped(def) {
            match scope {
                Scope::Body if k == h("PassengerSlots") => {
                    passengers = v.trim().parse::<i64>().unwrap_or(0).clamp(0, 4) as usize;
                }
                Scope::Body => pilot.set(k, v),
                Scope::Turret(i) => turrets[i as usize]
                    .get_or_insert_with(|| SeatSpec::new(SeatKind::Gunner(i)))
                    .set(k, v),
            }
        }
    }
    // A building turret has no seat of its own (§8: neither Controllable
    // setter's callers include it) and, [D], no PassengerSlot: its setter
    // hands everything but the sections to the building's (FUN_001c0b10).
    for s in std::iter::once(&mut pilot).chain(turrets.iter_mut().flatten()) {
        s.aimers.retain(AimerSpec::authored);
    }
    let turret = is_turret(db, class);
    let mut ring = if turret { Vec::new() } else { vec![pilot] };
    ring.extend(turrets.into_iter().flatten());
    if turret {
        return ring;
    }
    ring.extend(
        (0..passengers.min(MAX_PASSENGERS)).map(|i| SeatSpec::new(SeatKind::Passenger(i as u8))),
    );
    ring
}

#[cfg(test)]
mod tests;
