//! A vehicle: the sim's second entity kind (PLAN §DESIGN SPECS "P7 DESIGN —
//! entity shape", amended by `analysis/re-vehicles.md` §5-§8 — card VENT).
//!
//! A vehicle is not a `Unit`: it is not a team member (no tickets, no spawn
//! waves, no respawn — its pad owns that, §7), it has no stance, and it can be
//! EMPTY. Vehicles live in `Sim::vehicles` slots addressed by
//! generation-checked [`VehicleHandle`]s, the same slot-reuse scheme as units.
//!
//! Occupancy lives on BOTH sides: [`Seat::occupant`] and `Unit::mount`
//! ([`Mount`]). `Sim::seat_unit` / `Sim::unseat_unit` are the only writers,
//! so the two never disagree.
//!
//! What the PS2 fixes here (each [O] in re-vehicles.md):
//! - ONE hull pool per vehicle (§6) — no damage regions, so
//!   `Target::Vehicle` carries no region. Its `HealthType` is the class's
//!   (default vehicle; BF1's tauntaun is "animal", §7.1). A hull contact
//!   deals [`impact_damage`] (§3.2 item 10) raw ([`collision_desc`]); the
//!   pool's damage and death are `Sim::damage_vehicle` / `destroy_vehicle`.
//! - The seat ring (§5, B1 FUN_001d76a0): the vehicle's own seat (the BODY
//!   props, the pilot), then one gunner per TURRETn section in index order
//!   (≤ 8), then the passengers (≤ 4) — at most 13 seats.
//! - A building turret (§8) is a vehicle with no pilot seat and 1..=8 gunner
//!   seats, and never moves.
//! - The live team (§7.1, card VTEAM) is the last entrant's, whichever seat
//!   they took; when the last rider leaves an ordinary class goes back to
//!   team 0 (`None`, neutral) and a command vehicle keeps its team. A pad
//!   gives an EMPTY ordinary vehicle no team, but records its own team apart
//!   as `home_team` for the AI. B1 keeps that record per CLASS (class+0x88),
//!   B2 per vehicle (bits 40-43); the port keeps it per vehicle, which covers
//!   both.
//!
//! The spawn pads (`crate::vehicle_pad`, card VPAD) make them; [`hover`]
//! moves every hover whose class the sim was given (`Sim::hover_params`,
//! card HOVR), and [`walker`] every walker (`Sim::walker_params`, card
//! WALK). An empty `Sim::vehicles` costs the sim hash nothing
//! (`Sim::hash`), so a pad-free sim hashes as it always did.

pub mod flyer;
pub mod gun;
pub mod hover;
pub mod tow;
pub mod walker;

use crate::fmath::FloatExt;
use crate::unit::UnitHandle;
use crate::weapon::{Health, HealthType};
use glam::{Quat, Vec3};
use gun::{GunSetup, GunState};
use hover::{HoverInput, HoverState};
use walker::WalkerState;

/// Generation-checked vehicle reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VehicleHandle {
    pub index: u32,
    pub gen: u32,
}

/// Which integrator moves the vehicle (§2-§4). `treaded` and `commandhover`
/// are Hover (one integrator for 21 classes, §3); `commandwalker` is Walker;
/// `carrier` is Flyer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehicleKind {
    Hover = 0,
    Walker = 1,
    Flyer = 2,
    /// A building turret (`armedbuilding`, §8): seats only, never moves. OURS
    /// as an entity kind — the PS2 keeps it a building (kind 8) carrying
    /// MountedTurret sections; the port shares the vehicle seat machinery.
    Turret = 3,
}

/// Where a seat sits in the ring (§5). Mirrors the class reader's
/// `game::vehicle_class::SeatKind`, which re-exports this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatKind {
    /// The vehicle's own seat (the BODY props).
    Pilot,
    /// A MountedTurret section, index 0..=7 (TURRET1 = 0).
    Gunner(u8),
    /// A PassengerSlot, 0..=3: a camera and a seat, no weapon.
    Passenger(u8),
}

impl SeatKind {
    fn hash_byte(self) -> u8 {
        match self {
            SeatKind::Pilot => 0,
            SeatKind::Gunner(i) => 0x10 | i,
            SeatKind::Passenger(i) => 0x20 | i,
        }
    }
}

/// Most gunner sections a class carries (TURRET1..8, §5).
pub const MAX_GUNNERS: usize = 8;
/// `PassengerSlots` is clamped to 0..=4 (§5).
pub const MAX_PASSENGERS: usize = 4;
/// 1 + 8 + 4 (§5 [D]).
pub const MAX_SEATS: usize = 1 + MAX_GUNNERS + MAX_PASSENGERS;

/// Is `kinds` a seat ring the PS2 could build: at most one pilot and only
/// first, gunners in strictly rising section order, then passengers
/// 0, 1, 2 … — with the per-kind caps?
pub fn is_ring(kinds: &[SeatKind]) -> bool {
    let (mut gunners, mut passengers) = (0usize, 0usize);
    let mut last_gunner: Option<u8> = None;
    for (i, k) in kinds.iter().enumerate() {
        match *k {
            SeatKind::Pilot if i == 0 => {}
            SeatKind::Pilot => return false,
            SeatKind::Gunner(s) => {
                if passengers > 0
                    || usize::from(s) >= MAX_GUNNERS
                    || last_gunner.is_some_and(|l| s <= l)
                {
                    return false;
                }
                last_gunner = Some(s);
                gunners += 1;
            }
            SeatKind::Passenger(p) => {
                if usize::from(p) != passengers || passengers == MAX_PASSENGERS {
                    return false;
                }
                passengers += 1;
            }
        }
    }
    gunners <= MAX_GUNNERS && kinds.len() <= MAX_SEATS
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub kind: SeatKind,
    pub occupant: Option<UnitHandle>,
}

/// A unit's side of the occupancy: which vehicle, which seat of its ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mount {
    pub vehicle: VehicleHandle,
    pub seat: u8,
}

impl Mount {
    /// Fed into `Unit::hash_into` only when the unit is mounted, led by a tag
    /// so a mounted unit's stream is self-delimiting.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"M");
        eat(&self.vehicle.index.to_le_bytes());
        eat(&self.vehicle.gen.to_le_bytes());
        eat(&[self.seat]);
    }
}

#[derive(Clone, Debug)]
pub struct Vehicle {
    pub gen: u32,
    pub alive: bool,
    pub kind: VehicleKind,
    /// Index into the game layer's vehicle class table (SETUP data, like
    /// `Sim::weapon_classes`; the table itself is not hashed, this is).
    pub class: u32,
    /// The live team (§7.1): the last entrant's, `None` = the PS2's team 0
    /// (neutral, anyone may enter). Written only by the pad spawn and
    /// `Sim::seat_unit` / `Sim::unseat_unit`.
    pub team: Option<u8>,
    /// The spawn pad's team, kept apart from `team` (§7.1): what the AI
    /// reads as the vehicle's side while it stands empty and neutral.
    pub home_team: Option<u8>,
    /// Does the team go back to neutral when the last rider leaves? False
    /// for the command classes (B1 class+0x80, B2 class+0x330 bit 34).
    pub neutral_when_empty: bool,
    /// Index of the pad that spawned it (the pad owns respawn, §7); `None` for
    /// a building turret or a scripted spawn.
    pub spawn_pad: Option<u32>,
    pub pos: Vec3,
    pub vel: Vec3,
    /// The PS2's entity matrix E as a rotation: the model's local axes (+x
    /// right, +y up, +z forward) into the world, the hover's visual tilt
    /// included ([`hover`]). Draw the model at `pos` with this.
    pub rot: Quat,
    /// What the driver's controller wrote this tick (zero when empty).
    pub input: HoverInput,
    /// The hover integrator's fields (ω, g, the tilt, the springs'
    /// compressions); untouched for a vehicle nothing integrates. A walker
    /// keeps its full-stick timer here too (E+0x360 is the controllable's).
    pub hover: HoverState,
    /// The walker integrator's fields (card WALK, [`walker`]); hashed only
    /// for a [`VehicleKind::Walker`].
    pub walker: WalkerState,
    /// Flight-only dynamic state; default parked state contributes no bytes
    /// to the hash, preserving existing ground/parked scenarios.
    pub flyer: flyer::FlyerState,
    /// The ring, in the PS2's order ([`is_ring`]).
    pub seats: Vec<Seat>,
    /// The ONE hull pool (§6).
    pub health: Health,
    /// Per ring seat, the gun of a seat that has one (card VTUR,
    /// [`gun`]); empty for a class with none, which then hashes as before.
    pub guns: Vec<Option<GunState>>,
    /// Card VMESH: where its last bot driver gave it up as stuck; no bot
    /// claims it again until it is [`STUCK_MOVED`] from there (ours). Hashed
    /// only when set.
    pub stuck_at: Option<Vec3>,
    /// Qualifying walker/hover contact hold deadline (fixed simulation ticks).
    /// Zero means no active hold; only contact priority can start it.
    pub yield_until: u64,
    /// Card TURR: a building turret's last hit by a unit, and its tick (a
    /// gunner hit from outside every arc gets out, `ai::man`). Written for
    /// [`VehicleKind::Turret`] only; hashed only when set.
    pub last_hit: Option<(crate::unit::UnitHandle, u64)>,
    /// A building turret's map instance (`collision::Body::instance`): its
    /// building, which its camera ray passes (card TURR). SETUP data, set by
    /// the game layer at the spawn; not hashed.
    pub instance: Option<u32>,
    /// The acquisition's bounds (card HTGT, `analysis/re-target-hud.md` §2):
    /// a sphere's centre in the vehicle's frame and its radius. Set by the
    /// game layer for a building turret from its building's bodies (the PS2
    /// picks the building by its bounds); `None` = the class's blast sphere.
    /// SETUP data, not hashed.
    pub aim_sphere: Option<(Vec3, f32)>,
    /// Issue #35: the slice meter, s (`analysis/re-build-zones.md` §11).
    /// Hashed only when non-zero.
    pub slice: f32,
    /// Issue #35 [ours]: after a slice only this unit may board, until the
    /// tick ([`SLICE_RESERVE`]). Hashed only when set.
    pub slice_hold: Option<(crate::unit::UnitHandle, u64)>,
}

/// Issue #35 [ours, the maintainer 2026-10-29]: after a slice only the
/// slicer may board for this long, s. The PS2 locks everyone out for the
/// class's `TimeTilReboard` (5 s) instead.
pub const SLICE_RESERVE: f32 = 3.0;

/// Card VMESH (ours): how far (m, in XZ) a vehicle given up as stuck must
/// have moved before a bot claims it again. A walker wedged on BF1 Endor's
/// cp2 boulder was boarded by pair after pair, each put out after its
/// stuck window (main 662e152: 31 and 40 such rides in one round).
pub const STUCK_MOVED: f32 = 2.0;

impl Vehicle {
    pub fn pilot_seat(&self) -> Option<&Seat> {
        self.seats.first().filter(|s| s.kind == SeatKind::Pilot)
    }

    pub fn is_empty(&self) -> bool {
        self.seats.iter().all(|s| s.occupant.is_none())
    }

    /// Is someone in the seat that flies or drives it?
    pub fn has_pilot(&self) -> bool {
        self.seats
            .iter()
            .any(|s| s.kind == SeatKind::Pilot && s.occupant.is_some())
    }

    /// Feed into the sim hash, every field bit-exact.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.gen.to_le_bytes());
        eat(&[
            self.alive as u8,
            self.kind as u8,
            self.team.unwrap_or(u8::MAX),
            self.home_team.unwrap_or(u8::MAX),
            self.neutral_when_empty as u8,
            self.health.health_type as u8,
        ]);
        eat(&self.class.to_le_bytes());
        eat(&self.spawn_pad.unwrap_or(u32::MAX).to_le_bytes());
        for v in [
            self.pos.x, self.pos.y, self.pos.z, self.vel.x, self.vel.y, self.vel.z, self.rot.x,
            self.rot.y, self.rot.z, self.rot.w,
        ] {
            eat(&v.hash_bits().to_le_bytes());
        }
        self.input.hash_into(eat);
        self.hover.hash_into(eat);
        if self.kind == VehicleKind::Walker {
            self.walker.hash_into(eat);
        }
        if self.kind == VehicleKind::Flyer {
            self.flyer.hash_into(eat);
        }
        eat(&[self.seats.len() as u8]);
        for s in &self.seats {
            eat(&[s.kind.hash_byte()]);
            let (i, g) = s
                .occupant
                .map_or((u32::MAX, u32::MAX), |h| (h.index, h.gen));
            eat(&i.to_le_bytes());
            eat(&g.to_le_bytes());
        }
        eat(&self.health.cur.hash_bits().to_le_bytes());
        eat(&self.health.max.hash_bits().to_le_bytes());
        if !self.guns.is_empty() {
            eat(b"G");
            for g in &self.guns {
                match g {
                    Some(g) => g.hash_into(eat),
                    None => eat(&[0xff]),
                }
            }
        }
        if let Some(p) = self.stuck_at {
            eat(&[0x5a]);
            for v in [p.x, p.y, p.z] {
                eat(&v.hash_bits().to_le_bytes());
            }
        }
        if self.yield_until != 0 {
            eat(&[0x5e]);
            eat(&self.yield_until.to_le_bytes());
        }
        if let Some((u, t)) = self.last_hit {
            eat(&[0x5b]);
            eat(&u.index.to_le_bytes());
            eat(&u.gen.to_le_bytes());
            eat(&t.to_le_bytes());
        }
        if self.slice != 0.0 {
            eat(&[0x5c]);
            eat(&self.slice.hash_bits().to_le_bytes());
        }
        if let Some((u, t)) = self.slice_hold {
            eat(&[0x5d]);
            eat(&u.index.to_le_bytes());
            eat(&u.gen.to_le_bytes());
            eat(&t.to_le_bytes());
        }
    }

    /// Issue #35: is boarding shut to `u` by another unit's slice hold?
    pub fn held_against(&self, u: crate::unit::UnitHandle, tick: u64) -> bool {
        self.slice_hold
            .is_some_and(|(who, until)| who != u && tick < until)
    }
}

/// What the use input and the exit read of a vehicle CLASS (card VSEAT,
/// re-vehicles §5): SETUP data per class like `Sim::hover_params`, NOT
/// hashed. A class without one cannot be entered.
#[derive(Clone, Debug, PartialEq)]
pub struct SeatSetup {
    /// The collision sphere's radius (FUN_001645a0), about the vehicle's
    /// origin. PROVISIONAL: the game layer bounds the spring spheres.
    pub radius: f32,
    /// Per ring seat: may a soldier take it — PilotType `vehicle` (B1
    /// FUN_0020b1b0) or BF2's `vehicleself`.
    pub boardable: Vec<bool>,
    /// Whether each seated soldier has a collision body, so can be hit himself:
    /// a pilot pose and IsPilotExposed (B1 FUN_0020c880, B2 FUN_0024de60). A
    /// hit on the hull never reaches him, nor one on him the hull.
    pub body_active: Vec<bool>,
    /// The exit's ground ray: 10 m (B1 FUN_0020b918) / 15 m (B2 FUN_0024ba68).
    pub exit_drop: f32,
    /// The class's `AISizeType` mask (RE_LOG "The six type bits": SOLDIER 1,
    /// HOVER 2, SMALL 4, MEDIUM 8, HUGE 0x10, FLYER 0x20, default 1) — the
    /// planning-graph type its AI driver routes as (card VAI).
    pub ai_size: u32,
    /// Per ring seat, its guns (card VTUR); shorter than the ring, or
    /// `None`, where a seat has none.
    pub guns: Vec<Option<GunSetup>>,
}

impl SeatSetup {
    /// The route table an AI driver of this class uses: the lowest type bit
    /// of [`SeatSetup::ai_size`] that has a table (FLYER has none — flyers do
    /// not route), else SOLDIER. PROVISIONAL: a mask with two bits set routes
    /// as the smaller type; no hover class on the discs authors two.
    pub fn nav_type(&self) -> usize {
        (0..crate::nav::TYPE_COUNT)
            .find(|&t| self.ai_size & (1 << t) != 0)
            .unwrap_or(crate::nav::SOLDIER)
    }
}

/// What a vehicle CLASS's death and a blast's gather read (card VDTH,
/// `analysis/re-vehicle-death.md`): SETUP data per class like
/// `Sim::hover_params`, NOT hashed. A class past the end, or `None`, neither
/// explodes nor has a sphere (a blast finds it at its origin, radius 0).
#[derive(Clone, Debug, PartialEq)]
pub struct BlastSetup {
    /// The class's `ExplosionName` class (§6 of re-vehicles: detonated when
    /// the hull pool runs out); `None` = no area damage.
    pub explosion: Option<crate::weapon::ExplosionClass>,
    /// The collision sphere a blast measures this class by (§3): its centre
    /// in the vehicle's frame and its radius `w`.
    pub sphere_centre: Vec3,
    pub sphere_radius: f32,
}

/// How to build a vehicle (class data resolved by the game layer from
/// `game::vehicle_class`; `core` never reads a class file).
#[derive(Clone, Debug)]
pub struct VehicleSpec {
    pub kind: VehicleKind,
    pub class: u32,
    /// The spawning pad's team (`None` for a neutral pad or no pad). An
    /// ordinary class starts empty and neutral whatever it is; a command
    /// class takes it as its live team (§7.1).
    pub home_team: Option<u8>,
    /// `game::vehicle_class::neutral_when_empty`.
    pub neutral_when_empty: bool,
    pub spawn_pad: Option<u32>,
    pub pos: Vec3,
    /// The turn about +Y (the pad instance's, `game::vehicle_pads`): the
    /// vehicle starts at `rot` = [`hover::quat_y`]`(−yaw)`, whose right row
    /// is (cos yaw, 0, sin yaw).
    pub yaw: f32,
    /// ODF `MaxHealth`.
    pub max_health: f32,
    /// ODF `HealthType` over the class chain, default `Vehicle`
    /// (`game::vehicle_class::health_type`).
    pub health_type: HealthType,
    /// The ring's kinds, in order; must pass [`is_ring`].
    pub seats: Vec<SeatKind>,
}

impl VehicleSpec {
    pub fn build(&self, gen: u32) -> Vehicle {
        assert!(
            is_ring(&self.seats),
            "not a PS2 seat ring: {:?}",
            self.seats
        );
        assert!(
            self.kind != VehicleKind::Turret || self.seats.first() != Some(&SeatKind::Pilot),
            "a building turret has no pilot seat (§8)"
        );
        Vehicle {
            gen,
            alive: true,
            kind: self.kind,
            class: self.class,
            // B1 FUN_002b0238 / B2 FUN_003725f0: the pad sets an empty
            // ordinary vehicle no team. A spawn WITH a rider gets the rider's
            // team from `Sim::seat_unit`.
            team: if self.neutral_when_empty {
                None
            } else {
                self.home_team
            },
            home_team: self.home_team,
            neutral_when_empty: self.neutral_when_empty,
            spawn_pad: self.spawn_pad,
            pos: self.pos,
            vel: Vec3::ZERO,
            rot: hover::quat_y(-self.yaw),
            input: HoverInput::default(),
            hover: HoverState::default(),
            walker: WalkerState::default(),
            flyer: flyer::FlyerState::default(),
            seats: self
                .seats
                .iter()
                .map(|&kind| Seat {
                    kind,
                    occupant: None,
                })
                .collect(),
            health: Health::new(self.max_health, self.health_type),
            guns: Vec::new(),
            stuck_at: None,
            yield_until: 0,
            last_hit: None,
            instance: None,
            aim_sphere: None,
            slice: 0.0,
            slice_hold: None,
        }
    }
}

/// A hull contact's damage (re-vehicles §3.2 item 10, B1 FUN_001ee3e0
/// B1:99944-99954): `normal_speed` is the velocity along the contact normal
/// (negative = closing), `n_up` / `n_fwd` the normal dotted with the hull's
/// up and forward rows (+0x210 / +0x220). With the class's
/// `CollisionThreshold` t = T0 + T1·n_up + T2·n_fwd, a contact closing faster
/// than t deals (s + t)² · (S0 + S1·n_up + S2·n_fwd) with `CollisionScale`;
/// anything else, or a non-positive result, deals nothing (B1:99960). The
/// other body's factor (B1:99955-99959) is the caller's to multiply in.
pub fn impact_damage(
    normal_speed: f32,
    n_up: f32,
    n_fwd: f32,
    threshold: [f32; 3],
    scale: [f32; 3],
) -> f32 {
    if normal_speed >= 0.0 {
        return 0.0;
    }
    let t = threshold[0] + threshold[1] * n_up + threshold[2] * n_fwd;
    if normal_speed >= -t {
        return 0.0;
    }
    let s = normal_speed + t;
    (s * s * (scale[0] + scale[1] * n_up + scale[2] * n_fwd)).fmax(0.0)
}

/// The descriptor a collision deals its damage through: all seven HealthType
/// scales 1.0 (a loop over a stack table, B1:100006-100014), so a hull takes
/// the raw amount whatever its HealthType. `shield_scale` 1.0 [H] (no vehicle
/// ships a shield, §6).
pub fn collision_desc(damage: f32) -> crate::weapon::DamageDesc {
    crate::weapon::DamageDesc {
        damage,
        scales: [1.0; 7],
        shield_scale: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The skiff's line `CollisionScale = "30.0"` leaves S1 = S2 = 0; a
    /// threshold of 5 m/s straight on.
    #[test]
    fn impact_damage_is_squared_past_the_threshold() {
        let (t, sc) = ([5.0, 0.0, 0.0], [30.0, 0.0, 0.0]);
        assert_eq!(impact_damage(2.0, 1.0, 0.0, t, sc), 0.0, "separating");
        assert_eq!(
            impact_damage(-5.0, 1.0, 0.0, t, sc),
            0.0,
            "at the threshold"
        );
        assert_eq!(impact_damage(-7.0, 1.0, 0.0, t, sc), 120.0, "2² × 30");
        // The directional terms: a nose-on hit (n_fwd = −1) with T2 = 2 and
        // S2 = −10 has t = 3 and scale 40.
        let d = impact_damage(-5.0, 0.0, -1.0, [5.0, 0.0, 2.0], [30.0, 0.0, -10.0]);
        assert_eq!(d, 160.0);
        assert_eq!(impact_damage(-9.0, 0.0, 0.0, t, [-1.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn the_ring_is_pilot_then_gunners_then_passengers() {
        use SeatKind::*;
        assert!(is_ring(&[]));
        assert!(is_ring(&[
            Pilot,
            Gunner(0),
            Gunner(1),
            Passenger(0),
            Passenger(1)
        ]));
        assert!(is_ring(&[Gunner(0), Gunner(3)]), "a building turret");
        assert!(!is_ring(&[Gunner(0), Pilot]), "the pilot is first");
        assert!(!is_ring(&[Pilot, Pilot]));
        assert!(!is_ring(&[Pilot, Passenger(0), Gunner(0)]));
        assert!(!is_ring(&[Pilot, Gunner(1), Gunner(0)]), "section order");
        assert!(!is_ring(&[Pilot, Gunner(8)]));
        assert!(!is_ring(&[Pilot, Passenger(1)]));
        let mut full = vec![Pilot];
        full.extend((0..8).map(Gunner));
        full.extend((0..4).map(Passenger));
        assert_eq!(full.len(), MAX_SEATS);
        assert!(is_ring(&full));
        full.push(Passenger(4));
        assert!(!is_ring(&full));
    }
}
