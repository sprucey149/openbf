//! Fixed-step deterministic sim — the capture/replay spine (PLAN.md
//! §Determinism) and, from P4, the combat world: units (soldier + health +
//! weapon), bolts, damage, deaths feeding `Battle`, BF1 aim assist, dev
//! cheats. No wall-clock, no OS randomness: `SimRng` (SCE rand) + `GameLcg`
//! (inline gameplay LCG) are the only entropy, both seeded from the log.
//!
//! Tick order (one `step`): confirmed Respawn command →
//! 0a the spawn screen's class choice → 0 respawns
//! (member timer, spawn wave, alive-AI cap) → 0b squad commands (Phase D) →
//! 1 the commander (Phase B) → 1b controllers → per-unit pads → 1c the use
//! input (enter / exit, VSEAT) → 2 player aim assist → doors → 3 soldier movement (a human's zoom press first, card ZOOM; a
//! rider's pad drives its vehicle instead; + 3a wall pass; 3a' the floor
//! contact under the new feet; then the hovers and their riders; 3b unit
//! separation; 3c environmental death: the kill floor, the death regions) → 4
//! weapon cycles (a soldier's channels, switches and held weapons first, card
//! GREN) → bolt spawns (predictive raycast) and throws → 5 bolt flight →
//! impacts → damage/death → 5b the thrown grenades (fuse, flight, sweep,
//! blast) → 5c the pickups (flight, life, the touch; card PWUP) → 5c' the health / ammo
//! droids' passes (card PWST) → 6 command posts → bleed → domination → outcome → 7 the
//! vehicle pads (follow, decay, spawn); tick++.
//! Events for the tick are left in `events`.
//!
//! ## Where the code lives
//!
//! `state` owns the world, queries and hash; `input` owns pads, logs and aim
//! assist; `spawn` owns class selection, spawns and waves; `step` orders each
//! tick and separates units. `combat` resolves bolt impacts and deaths;
//! `round` updates posts and outcome; `guns` handles mounted weapons;
//! `vehicles` owns vehicle slots and pads; `doors` advances door bodies and events; `seats` handles entry, exit and
//! drivers; `blast` applies area damage; `bounds` defines the playable level;
//! `ordnance` handles weapon channels, throws and grenade flight; `pickups`
//! owns dropped items; `trace` writes the optional observational state stream; `stations` runs health/ammo droids. Public types are
//! re-exported here; shared test fixtures stay in this facade.

mod abilities;
mod beam;
mod blast;
mod bounds;
mod buffs;
mod building_turrets;
mod burn;
mod cloaks;
mod combat;
mod damage_regions;
pub mod deathmatch;
mod doors;
mod droid_death;
mod droidekas;
mod emitter;
mod fatray;
mod flyer_contact;
mod flyers;
mod force;
mod guided;
mod guns;
mod hero_bodies;
pub mod heroes;
mod input;
mod lock_on;
mod look;
mod mines;
mod missiles;
mod native_teams;
mod ordnance;
mod pickups;
mod population;
mod remotes;
mod repair;
#[cfg(test)]
mod rider_health_tests;
mod round;
mod saber;
mod saber_throw;
mod seats;
mod self_turrets;
mod space_assault;
mod space_balance;
pub use space_balance::SpaceBalance;
mod spawn;
mod state;
mod stations;
mod step;
mod tow;
pub mod trace;
mod traffic;
mod turrets;
mod vehicles;
mod water;

pub use beam::Beam;
pub use blast::{soldier_blast_sphere, BLAST_GATHER_MAX};
pub use bounds::{
    Level, ReturnPath, BOUND_DROP, KILL_FLOOR_DROP, LEVEL_HUBS, LEVEL_RISE, UNDER_MAP_SPEED,
};
pub use buffs::Buffs;
pub use building_turrets::{BuildingTurret, PosedBody, PosedParts, TurretFall};
pub use damage_regions::DamageRegion;
pub use droid_death::PUSHED_DEAD_HEALTH;
pub use droidekas::{capsule_of as droideka_capsule, FLOOR_SNAP as DROIDEKA_FLOOR_SNAP};
pub use flyers::FlightLimits;
pub use guns::SeatLook;
pub use hero_bodies::HeroRiderFrames;
pub use input::{buttons, AimTarget, InputLog, PadState, ScreenPick, LOOK_SCALE, LOOK_UNIT};
pub use lock_on::{LockOn, LockTarget};
pub use look::{LookPreview, MOUSE_STEER};
pub use native_teams::AllySpawn;
pub use pickups::{
    DropTable, ItemClass, Pickup, BODY_AHEAD, BODY_RADIUS, DROP_HEIGHT, DROP_SIDE_SPEED,
    ITEM_GRAVITY, ITEM_POOL_DEFAULT, LIFE_END, MAX_DROPS, TAKE_AGE,
};
pub use round::scale_reinforcements;
pub use seats::{ENTER_REACH, EXIT_RING};
pub use space_assault::{
    CapitalMeter, CriticalSystem, FrigateTurrets, LinkedTurretSet, ShieldGroup, SpaceAssault,
    DEFAULT_SCORE_LIMIT, FIGHTER_KILL_POINTS, SPENT_MAX_HEALTH, UNMANNED_FLYER_POINTS,
};
pub use spawn::{ClassAmmo, ClassRefusal, SideChoice, SpawnStage, SpawnWaves, CLASS_CHANGE_FACING};
pub use state::{
    omni_radius_for, Call, Cheats, DeathCause, Event, Hazard, MissionAi, NavStats, Sim, TickCost,
    UnitCost, BLAST_AGE, HAZARD_RADIUS, LETHAL_LANDING_SPEED, OUTCOME_DRAW,
    SNIPE_SUITABILITY_DEFAULT,
};
pub use stations::{
    Station, StationAnim, StationClass, StationDamageEffect, StationMotion,
    DELAY_DEFAULT as STATION_DELAY_DEFAULT, GATHER_MAX as STATION_GATHER_MAX,
};
pub use tow::{TowCable, TowFall};

#[cfg(test)]
mod tests {
    //! The fixtures the parts' test modules share; each part's own tests
    //! live beside its code, in that part's `mod tests`.
    use crate::cp::CpDef;
    use crate::heightfield::Heightfield;
    use crate::sim::{buttons, PadState, Sim};
    use crate::unit::{Controller, UnitHandle, UnitSpec};
    use glam::Vec3;

    /// Flat ground at y = 0 over ±2 km. A `Sim` with NO heightfield now floors
    /// at [`crate::collision::NO_GROUND`] (−300) rather than 0, so the tests
    /// that were written against the old 0-fallback say what they meant.
    pub(super) fn flat_ground() -> Heightfield {
        Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }
    }

    pub(super) fn hits(s: &Sim) -> Vec<(u32, f32)> {
        s.events
            .iter()
            .filter_map(|e| match e {
                crate::sim::Event::Hit { victim, damage, .. } => Some((victim.index, *damage)),
                _ => None,
            })
            .collect()
    }

    /// Player at the origin facing -Z, target trooper 20 m ahead.
    pub(super) fn duel() -> (Sim, UnitHandle, UnitHandle) {
        let mut s = Sim::empty(7);
        s.heightfield = Some(flat_ground());
        let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        let t = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
            .unwrap();
        (s, p, t)
    }

    pub(super) fn fire() -> PadState {
        PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }
    }

    pub(super) fn sphere_cp(name: &str, pos: Vec3, r: f32, major: bool) -> CpDef {
        let (capture_time, neutralize_time) = CpDef::class_times(major);
        CpDef {
            name: name.into(),
            pos,
            capture: crate::region::Region::sphere(pos, r),
            control: None,
            value_bleed: 10,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![
                (pos + Vec3::new(2.0, 0.0, 2.0), 0.0),
                (pos - Vec3::X * 2.0, 0.0),
            ],
            capture_time,
            neutralize_time,
        }
    }

    pub(super) fn ai(team: u8, pos: Vec3, yaw: f32) -> UnitSpec {
        UnitSpec {
            controller: Controller::Ai,
            yaw,
            ..UnitSpec::trooper(team, pos)
        }
    }
}
