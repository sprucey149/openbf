//! A vehicle spawn pad (`com_item_vehicle_spawn`, analysis/re-vehicles.md §7
//! [O], card VPAD): every stock vehicle enters the battle through one — no
//! map places a drivable vehicle directly (§1).
//!
//! What the PS2 does (B1 ctor FUN_002af4f0, setter FUN_002af778, tick
//! FUN_002b0780, spawn FUN_002b0238, decay FUN_002afa90):
//! - The pad keeps ONE class per team slot, resolved at load from its
//!   `Class<Side>` / `Class<Side>Atk` / `Class<Side>Def` props against each
//!   team's side (Atk = team 1, Def = team 2, §7). The game layer does that
//!   resolution (`game::vehicle_pads`); here a pad holds one [`PadClass`] per
//!   port team.
//! - Its team follows its `ControlZone` CP's owner every tick; a change resets
//!   the countdown to `SpawnTime` (B1:240799-240800 = step.rs order below).
//! - With no live vehicle and a class for its team it counts down: armed at
//!   construction with the countdown at 0 (the first spawn is immediate,
//!   ctor +0x100 = 1, +0x104 = 0), re-armed with `SpawnTime` after each
//!   spawn (B1:240445-240447, 240668).
//! - One vehicle per pad in BF1; BF2 keeps a list capped by `SpawnCount`
//!   (default 1, B2 FUN_00373138: `count < SpawnCount && class` → spawn).
//! - The idle decay of a live vehicle: see [`VehiclePad`]'s counters and
//!   `Sim::tick_vehicle_pads`.
//!
//! Left out, each [H] or unused by stock data: the Historical-row override
//! (the dropped BF1 campaign), the `vehiclepad` carrier (no stock instance),
//! BF1's sabotage bonus (a new vehicle at 15 % health when the enemy holds
//! bonus bit 0x20 — the port has no bonus system yet), and the convex-region
//! list DAT_00518314 the decay tests first (FUN_002956d8, unnamed): the port
//! takes the branch the PS2 takes when a vehicle is inside that list, or the
//! list is empty.

use crate::fmath::FloatExt;
use crate::vehicle::{VehicleHandle, VehicleSpec};
use glam::Vec3;

/// Ctor defaults (B1 FUN_002af4f0 B1:239754-239759): `SpawnTime` +0x78.
pub const SPAWN_TIME: f32 = 10.0;
/// `DecayTime` +0x84: a decaying vehicle loses MaxHealth / DecayTime a second.
pub const DECAY_TIME: f32 = 10.0;
/// Idle seconds before decay when the pad's class for its current team
/// differs from the class it spawned (+0x7c, B1:240222).
pub const IDLE_CLASS_CHANGED: f32 = 10.0;
/// Idle seconds before decay when the vehicle is outside its CP's
/// ControlRegion, or the CP has none (+0x80, B1:240146-240208).
pub const IDLE_OUTSIDE: f32 = 20.0;
/// A player within r_player + r_vehicle + this many metres keeps a vehicle
/// from idling (the literal 10.0, B1:240254).
pub const PLAYER_MARGIN: f32 = 10.0;
/// A failed create retries after 1 s (B1:240521-240524). The port's create
/// never fails; kept for the record.
pub const RETRY: f32 = 1.0;

/// The vehicle a pad makes for one team.
#[derive(Clone, Debug)]
pub struct PadClass {
    /// The class as built by the game layer; `home_team`, `spawn_pad`, `pos`
    /// and `yaw` are the pad's, filled at spawn.
    pub spec: VehicleSpec,
    /// The vehicle's bounding radius for the player-proximity test.
    pub radius: f32,
    /// A flyer class's `LandedHeight` (card VREST,
    /// `analysis/re-vehicle-rest.md` §2): its constructor stands the origin
    /// this far over the pad's spawn point. `None` for every other kind.
    pub landed_height: Option<f32>,
    pub vis_box: Option<(Vec3, Vec3)>,
}

/// A pad as the level authors it, classes resolved.
#[derive(Clone, Debug)]
pub struct PadDef {
    pub name: String,
    pub pos: Vec3,
    pub yaw: f32,
    /// Index into `Sim::cps` of the `ControlZone`, if it names a live post.
    pub cp: Option<usize>,
    /// The `Team` prop as a port team (PS2 team − 1); `None` for 0, −1, or a
    /// team the port does not field (locals). Overwritten by the CP's owner.
    pub team: Option<u8>,
    pub spawn_time: f32,
    pub decay_time: f32,
    /// BF1: 1 (the setter has no `SpawnCount` branch). BF2: `SpawnCount`.
    pub spawn_count: u32,
    /// The class per port team (PS2 teams 1, 2).
    pub classes: [Option<PadClass>; 2],
}

impl PadDef {
    /// The class the pad spawns for `team`, if any.
    pub fn class_for(&self, team: Option<u8>) -> Option<&PadClass> {
        self.classes.get(usize::from(team?))?.as_ref()
    }

    /// The class identity the decay compares (`class[f8] != class[fc]`).
    pub fn class_id(&self, team: Option<u8>) -> Option<u32> {
        self.class_for(team).map(|c| c.spec.class)
    }
}

/// One live vehicle of a pad and its idle counters.
///
/// B1 keeps the counters on the pad (+0x108 idle seconds, +0x10c the 1 s
/// sub-timer) and never resets them at a spawn, so a pad's next vehicle
/// inherits a decayed one's count. The port keeps them per vehicle, fresh
/// at spawn (BF2's list shape) — PROVISIONAL for BF1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadVehicle {
    pub handle: VehicleHandle,
    /// The pad's team when it spawned (+0xf8).
    pub spawned_team: Option<u8>,
    /// Whole idle seconds (+0x108).
    pub idle: f32,
    /// The once-a-second sub-timer (+0x10c).
    pub tick: f32,
}

#[derive(Clone, Debug)]
pub struct VehiclePad {
    pub def: PadDef,
    /// The live team (+0xfc).
    pub team: Option<u8>,
    /// +0x100: the countdown is loaded and running. 1 at the ctor with the
    /// countdown at 0; cleared by each spawn, so the next spawn step loads
    /// `SpawnTime` first.
    pub armed: bool,
    /// Seconds to the next spawn (+0x104).
    pub countdown: f32,
    pub vehicles: Vec<PadVehicle>,
}

impl VehiclePad {
    pub fn new(def: PadDef) -> Self {
        Self {
            team: def.team,
            armed: true,
            countdown: 0.0,
            vehicles: Vec::new(),
            def,
        }
    }

    /// Follow the CP's owner; a change reloads the countdown (B1:240799).
    pub fn follow(&mut self, owner: Option<u8>) {
        if owner != self.team {
            self.team = owner;
            self.countdown = self.def.spawn_time;
        }
    }

    /// One spawn step (B1 FUN_002b0238 B1:240440-240452): arm with
    /// `SpawnTime` if disarmed, count down, `true` when the pad spawns now.
    /// The caller spawns and calls [`VehiclePad::spawned`].
    pub fn count_down(&mut self, dt: f32) -> bool {
        if !self.armed {
            self.armed = true;
            self.countdown = self.def.spawn_time;
        }
        self.countdown -= dt;
        self.countdown <= 0.0
    }

    pub fn spawned(&mut self, handle: VehicleHandle) {
        self.vehicles.push(PadVehicle {
            handle,
            spawned_team: self.team,
            idle: 0.0,
            tick: 0.0,
        });
        self.armed = false;
    }

    /// Does the pad want another vehicle (`count < SpawnCount && class`)?
    pub fn wants_spawn(&self) -> bool {
        (self.vehicles.len() as u32) < self.def.spawn_count.max(1)
            && self.def.class_for(self.team).is_some()
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[
            self.team.unwrap_or(u8::MAX),
            self.armed as u8,
            self.vehicles.len() as u8,
        ]);
        eat(&self.countdown.hash_bits().to_le_bytes());
        for v in &self.vehicles {
            eat(&v.handle.index.to_le_bytes());
            eat(&v.handle.gen.to_le_bytes());
            eat(&[v.spawned_team.unwrap_or(u8::MAX)]);
            eat(&v.idle.hash_bits().to_le_bytes());
            eat(&v.tick.hash_bits().to_le_bytes());
        }
    }
}
