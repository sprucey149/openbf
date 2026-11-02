//! Deterministic simulation core: fixed-step loop, seeded RNG, replay,
//! gameplay. No wall-clock reads inside sim logic (PLAN.md determinism spine).

pub mod ai;
pub mod aim_assist;
pub mod announcer;
pub mod autoturret;
pub mod battle;
pub mod boundary;
pub mod cloak;
pub mod collision;
pub mod combo;
pub mod commander;
pub mod cp;
pub mod door;
pub mod droideka;
pub mod emitter;
pub mod flash;
pub mod fmath;
pub mod guided;
pub mod heightfield;
pub mod hero_weapon;
pub mod impact;
pub mod laser;
pub mod lcg;
pub mod mine;
pub mod missile;
pub mod music;
pub mod nav;
pub mod perception;
pub mod projectile;
pub mod region;
pub mod remote;
pub mod rng;
pub mod route_patch;
pub mod sim;
pub mod skill;
pub mod soldier;
pub mod squad;
pub mod station_motion;
pub mod sticky;
pub mod unit;
pub mod vehicle;
pub mod vehicle_pad;
pub mod water;
pub mod weapon;

/// Sim tick rate, from audit A5 (analysis/a5-tickrate.md): the PS2 games run
/// variable measured-dt but deliver one Update per 2 NTSC vblanks in the
/// steady state (~33.37 ms), and the whole animation corpus is authored at
/// 30 fps. We fix the step at exactly 1/30 s for determinism (0.1% off real
/// 59.94/2 hardware — imperceptible).
pub const TICK_HZ: u32 = 30;

/// Fixed timestep in seconds (1/30).
pub const TICK_DT: f32 = 1.0 / TICK_HZ as f32;

/// The integer AI skill a unit spawns with: the tier's base skill for a BOT,
/// 0 for a player, a dummy or a pacer (PLAN "PHASE C DESIGN" decision 4 —
/// symmetric competence, every team's bots at the same level). One place, so
/// `Sim::spawn`, `Sim::respawn_slot` and `UnitClass::spec` cannot drift.
pub fn skill_for(controller: unit::Controller, base_skill: u8) -> u8 {
    match controller {
        unit::Controller::Ai => base_skill,
        _ => 0,
    }
}

/// Accumulated-frame-time clamp, mirroring the games' own 0.1 s dt clamp
/// (BF1 FUN_00173190, BF2 FUN_00174510): under extreme stalls the PS2 game
/// goes slow-motion instead of spiraling; we cap the accumulator at 3 ticks.
pub const MAX_FRAME_ACCUM_SECS: f32 = 0.1;
