//! The playtest black box (card BLKB) — DEV SCAFFOLDING: delete this module
//! and its `blackbox/` helpers
//! it (and its three call sites: `main/windowed.rs`, `examples/blackbox_read.rs`,
//! `examples/round_to_outcome.rs`) when the game is playable for real.
//!
//! In dev mode (on by default; `OPENBF_DEV=0` turns it off, `OPENBF_BLACKBOX=0`
//! only this) the windowed game keeps
//! the last 60 s of game time in memory and writes it as one JSON file,
//! `engine/artifacts/playtest/last.json` (the five before it as
//! `prev1..prev5.json`), when the player presses F9, quits, the process
//! panics, or the stuck alarm fires. `examples/blackbox_read.rs` prints a
//! digest of a file. A headless round writes the same file
//! (`round_to_outcome`'s `BLACKBOX_AT` / `BLACKBOX_ON`).
//!
//! What the ring holds ([`Ring`]), all read from the sim by reference — the
//! recorder never writes to it and is in no hash:
//! - every [`UNIT_PERIOD`] ticks (0.5 s), a [`crate::unit_probe::Probe`] per
//!   alive unit — the sweep's `UNIT_SAMPLES` row plus the steering's stop
//!   test (what shuts the way along the direction the steering was asked that
//!   tick, and the body; nothing for a bot that did not walk — card BLKF);
//! - the human ([`HumanRow`]): every tick for the last 5 s, every 0.5 s
//!   before that — position, camera, pad, weapon, what the reticle is on;
//! - every [`WORLD_PERIOD`] ticks (2 s), the posts, tickets, vehicles, doors;
//! - the events with their ticks (kills, environmental deaths — a recycle
//!   and a lost death among them —, spawns, post changes, orders, the
//!   commander's lines, a no-progress run reaching 3 s);
//! - each unit's trail: its last 10 s of positions at 4 Hz, with the tick of
//!   its last point (a dead unit's trail ends at its death);
//! - the sounds (card SNDB), in their own ring: every one-shot the mixer was
//!   handed in the last 60 s ([`SoundRow`]: tick, property, take, gain,
//!   pitch, metres from the listener, voices sounding). [`sound_digest`]
//!   lists them with a line per tick whose starts sum over full scale.
//!
//! The header names two commits: `build_commit` (stamped into the binary by
//! `game/build.rs`) is the code that ran; `commit` is the checkout's HEAD when
//! the battle started, which a stale binary does not belong to.
//!
//! The rings are fixed in size and reuse their rows, so a warm recorder
//! allocates nothing per tick (a commander's line is one short string).
//! A snapshot copies the ring; the hotkey's is written on its own thread.
//!
//! No screenshot: the windowed renderer cannot read its frame back
//! (`Renderer::capture_rgba` is the headless target's), so `last.png` is not
//! written. The stuck alarm fires only when the player was PUSHING the stick
//! for most of 20 s without getting 2 m away — standing still on purpose is
//! not a bug, and an alarm rotates the files.

use crate::sounds::Handed;
use crate::unit_probe::Probe;
use crate::walk::WalkSim;
use core::sim::{AimTarget, DeathCause, Event, PadState, Sim};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

mod rotating;
pub mod session;
pub use rotating::PREVIOUS;

/// The file's schema version. 2 (card BLKF): the stop is asked along the
/// steering's own direction (`walk*`), the hold / boarding fields, the build's
/// commit, each trail's last tick. 3 (card SNDB): the sound rows. 4 (card
/// OBST2): a unit's behaviour and its going-round commitment. 5 (card
/// CPBAR): a post's units inside per team and its two times. 6 (card HITCH):
/// the frame record (`hitches`). 7: the human's mouse look (`mx`, `my`)
/// and the mouse-aim bit. 8: the human's shots and every hit he deals or
/// takes (`shot`, `hit`, the event's `v`). 9 (card WHY): each unit's last
/// reasons (`why`). 10 (card DPRX): the match-long `session` tally of the
/// human (a headless round's: the proxy's). 11: a hit's age is -1 for a
/// blast (`core::sim::BLAST_AGE`); before it 0 was a blast OR a bolt that
/// struck in its first update (a sniper's inside 66 m). 12 (playtest
/// 2026-10-28, building turrets): every hit on a vehicle's or turret's pool
/// the human deals or takes in his ride (`vhit`). 13 (issue #58): a unit's
/// remote droid target (`target_droid`). An older file still reads.
pub const SCHEMA: u32 = 13;
/// Ticks between two unit frames (0.5 s).
pub const UNIT_PERIOD: u64 = core::TICK_HZ as u64 / 2;
/// Ticks between two world frames (2 s).
pub const WORLD_PERIOD: u64 = core::TICK_HZ as u64 * 2;
/// Unit frames kept: 60 s.
pub const FRAMES: usize = 120;
/// World frames kept: 60 s.
pub const WORLD_FRAMES: usize = 30;
/// The human's per-tick rows kept: 5 s.
pub const HUMAN_TICKS: usize = 5 * core::TICK_HZ as usize;
/// Trail points per unit: 10 s at 4 Hz.
pub const TRAIL_LEN: usize = 40;
/// Events kept (several minutes of a 64-unit battle).
pub const EVENTS_MAX: usize = 2048;
/// Sound starts kept: 60 s ([`SOUND_TICKS`]) and no more rows than this (a
/// start every tick from each of 9 sources).
pub const SOUNDS_MAX: usize = 16384;
pub const SOUND_TICKS: u64 = 60 * core::TICK_HZ as u64;
/// Starts in one tick that sum over this are an overload line in the digest
/// (1.0 = full scale: the takes are summed, each at its gain).
pub const SOUND_OVERLOAD: f32 = 1.0;
/// A no-progress run this long is an event (3 s).
pub const STUCK_EVENT_TICKS: u16 = 3 * core::TICK_HZ as u16;
/// The stuck alarm: this long (20 s) within [`ALARM_RADIUS`] of one spot...
pub const ALARM_TICKS: u32 = 20 * core::TICK_HZ;
pub const ALARM_RADIUS: f32 = 2.0;
/// ...with the move stick past half for this share of it...
pub const ALARM_PUSH_SHARE: f32 = 0.6;
/// ...and no more than once in this long (2 min).
pub const ALARM_EVERY: u64 = 120 * core::TICK_HZ as u64;
/// Frame times kept for the header's frame-rate line.
const FRAME_TIMES: usize = 240;
/// Is the black box on? Dev mode (on by default, `OPENBF_DEV=0` is off), and not `OPENBF_BLACKBOX=0`.
pub fn switch(dev: Option<&str>, blackbox: Option<&str>) -> bool {
    crate::dev::flag(dev) && blackbox != Some("0")
}

/// [`switch`], from the environment.
pub fn on() -> bool {
    switch(
        std::env::var("OPENBF_DEV").ok().as_deref(),
        std::env::var("OPENBF_BLACKBOX").ok().as_deref(),
    )
}

/// Where the snapshots go: `OPENBF_BLACKBOX_DIR`, else
/// `engine/artifacts/playtest` — found from the repository root or from
/// `engine/`, where the game is run. A packed build run from anywhere else
/// writes beside the profile ([`crate::profile::data_dir`]: `openbf_data/playtest`
/// beside the game file).
pub fn default_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("OPENBF_BLACKBOX_DIR") {
        return PathBuf::from(d);
    }
    if Path::new("engine/artifacts").is_dir() {
        PathBuf::from("engine/artifacts/playtest")
    } else if Path::new("artifacts").is_dir() || !pak::packed() {
        PathBuf::from("artifacts/playtest")
    } else {
        crate::profile::data_dir().join("playtest")
    }
}

/// A fixed ring that reuses its slots: [`Wheel::next`] hands out the oldest
/// one to overwrite, so a row's own buffers keep their capacity.
#[derive(Clone)]
struct Wheel<T> {
    items: Vec<T>,
    /// The next slot to overwrite once the wheel is full.
    head: usize,
    cap: usize,
}

impl<T: Default> Wheel<T> {
    fn new(cap: usize) -> Wheel<T> {
        Wheel {
            items: Vec::with_capacity(cap),
            head: 0,
            cap,
        }
    }

    fn next(&mut self) -> &mut T {
        if self.items.len() < self.cap {
            self.items.push(T::default());
            return self.items.last_mut().expect("just pushed");
        }
        let i = self.head;
        self.head = (self.head + 1) % self.cap;
        &mut self.items[i]
    }

    /// Oldest first.
    fn iter(&self) -> impl Iterator<Item = &T> {
        let (new, old) = self.items.split_at(self.head.min(self.items.len()));
        old.iter().chain(new.iter())
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}

/// One unit frame: every alive unit at `tick`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frame {
    pub tick: u64,
    pub units: Vec<Probe>,
}

/// A pick or magnet as [`HumanRow::reticle`] writes it: a unit's slot, or
/// -2 - a vehicle's.
fn target_code(t: AimTarget) -> i32 {
    match t {
        AimTarget::Unit(h) => h.index as i32,
        AimTarget::Vehicle(h) => -2 - h.index as i32,
        // A marker beyond any vehicle's: a turret's id is no slot.
        AimTarget::Turret(_) | AimTarget::Droid(_) => i32::MIN,
    }
}

/// The human at one tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HumanRow {
    pub tick: u64,
    pub alive: bool,
    pub pos: [f32; 3],
    /// The soldier's aim.
    pub yaw: f32,
    pub pitch: f32,
    /// The camera: eye, yaw, pitch.
    pub cam: [f32; 3],
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    /// The pad: sticks (i16) and `core::sim::buttons` bits.
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
    pub buttons: u32,
    #[serde(default)]
    pub mx: i32,
    #[serde(default)]
    pub my: i32,
    pub hp: f32,
    pub stance: u8,
    pub grounded: bool,
    pub speed: f32,
    /// Index into `names.weapons`, the clip and the spare rounds.
    pub weapon: u32,
    pub clip: u32,
    pub spare: u32,
    /// The unit slot the reticle is on (`Sim::reticle_pick`) and its colour
    /// code, the aim assist's magnet target; -1 none, -2 - N vehicle slot N
    /// (card HTGT: vehicles and building turrets are picked too).
    pub reticle: i32,
    pub reticle_k: i8,
    pub magnet: i32,
    /// The vehicle slot and seat ridden, -1 on foot.
    pub veh: i32,
    pub seat: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PostRow {
    /// -1 neutral.
    pub owner: i8,
    /// The capture charge and whose it is; the neutralise charge.
    pub cap: f32,
    pub cap_team: i8,
    pub neutral: f32,
    /// The units that count inside its capture region, team 0 / team 1
    /// (`Sim::cp_inside`; schema 5, 0 before).
    #[serde(default)]
    pub inside: [u16; 2],
    /// CaptureTime and NeutralizeTime (schema 5, 0 before).
    #[serde(default)]
    pub times: [f32; 2],
}

impl PostRow {
    /// The post's state as the HUD's capture icon reads it (card CPBAR).
    pub fn state(&self) -> crate::capture_feed::PostState {
        let team = |t: i8| u8::try_from(t).ok();
        crate::capture_feed::PostState {
            owner: team(self.owner),
            capture: self.cap,
            capture_team: team(self.cap_team),
            neutral: self.neutral,
            capture_time: self.times[0],
            neutralize_time: self.times[1],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct VehicleRow {
    pub id: u32,
    /// Index into `names.vehicles`.
    pub class: u32,
    /// 0 hover, 1 walker, 2 flyer, 3 turret.
    pub kind: u8,
    /// -1 neutral.
    pub team: i8,
    pub pos: [f32; 3],
    pub hp: f32,
    /// The unit slot in each of the first six seats, -1 empty.
    pub riders: [i32; 6],
}

/// The world at `tick`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldFrame {
    pub tick: u64,
    pub tickets: [u32; 2],
    pub bleed: [f32; 2],
    pub alive: [u32; 2],
    #[serde(default)]
    pub points: [u32; 2],
    pub posts: Vec<PostRow>,
    pub vehicles: Vec<VehicleRow>,
    /// Each door's open parameter, 0 shut .. 1 open.
    pub doors: Vec<f32>,
}

/// What happened ([`EventRow::kind`]); `a` / `b` / `c` per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ev {
    /// a killer, b victim, c the killer's weapon in hand (`names.weapons`; not
    /// the seat gun or grenade that killed); at the victim.
    #[default]
    Kill,
    /// The world killed unit a: each [`DeathCause`].
    DeathRegion,
    Fall,
    RiderDeath,
    Suicide,
    HeroExhausted,
    Blast,
    KillFloor,
    UnderMap,
    /// A bot off the level, given up.
    Lost,
    /// A bot that made no net progress, given up (card RCYC).
    Recycle,
    Water,
    /// Unit a spawned.
    Spawn,
    /// Unit a shed by the no-overspawn rule.
    Despawn,
    /// Post a: captured by / neutralised by / saved by team b.
    Captured,
    Neutralized,
    Saved,
    /// Team a ran out; time up with winner a.
    Defeated,
    TimeUp,
    /// Vehicle a (class c) destroyed by unit b (-1 nobody).
    VehicleLost,
    /// Player slot a gave order b to c bots.
    Order,
    /// The commander said `text`.
    Voice,
    /// Unit a's no-progress run reached 3 s; b the body in its way (-1 none).
    Stuck,
    /// The human (unit a) fired weapon class c (`names.weapons`); one row per
    /// round leaving the weapon (schema 8).
    Shot,
    /// Unit a hit unit b, one of them the human; c a's weapon in hand; `v`
    /// = [damage, b's health after it, the round's age in s: -1 for a
    /// blast] (schema 8; a blast was 0 before schema 11).
    Hit,
    /// Bot a, stalled, moved on where no human saw it (card UNSN): at its
    /// new spot; `v` = where it stood.
    Relocate,
    /// Unit a (-1: a collision) damaged vehicle or building turret b, the
    /// human the shooter or a rider of b; c a's weapon in hand; at the
    /// vehicle; `v` = [damage, b's health at the tick's end: 0 when it was
    /// destroyed] (schema 11).
    VehicleHit,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    pub tick: u64,
    pub kind: Ev,
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub pos: [f32; 3],
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// The row's numbers (a hit's damage, health after and age).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub v: Vec<f32>,
}

/// A one-shot the mixer was handed (card SNDB). `name` / `take` index
/// `names.sounds` / `names.takes` (a take is a file under the baked root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SoundRow {
    pub tick: u64,
    pub name: u32,
    pub take: u32,
    /// Linear, the level by distance included: what the mixer was told.
    pub gain: f32,
    pub pitch: f32,
    /// Metres from the listener; 0 for a 2-D line (a commander's).
    pub dist: f32,
    /// One-shots still sounding by their length (this one too) plus the
    /// engine layers looping.
    pub voices: u16,
    /// The take's length at that pitch, seconds.
    pub secs: f32,
    /// What each ear got [left, right]: the gain x the bus, split by the
    /// PS2's per-ear law for a 3-D sound (card MIXH; [0, 0] in an older
    /// file).
    #[serde(default)]
    pub ears: [f32; 2],
}

/// A [`SoundRow`] in the ring: the property by hash, the take by the ring's
/// own table.
#[derive(Clone, Copy)]
struct SoundKept {
    hash: u32,
    row: SoundRow,
}

/// One unit's trail in the ring.
#[derive(Clone, Copy)]
struct Trail {
    gen: u32,
    pts: [[f32; 3]; TRAIL_LEN],
    head: u8,
    n: u8,
    /// The tick of the newest point.
    last: u64,
}

impl Default for Trail {
    fn default() -> Trail {
        Trail {
            gen: 0,
            pts: [[0.0; 3]; TRAIL_LEN],
            head: 0,
            n: 0,
            last: 0,
        }
    }
}

/// One unit's trail in the file: oldest first, 4 Hz, ending at `tick` — the
/// header's `trail_tick` for a unit alive then, its last sample before it
/// died otherwise. A schema 1 file has no `tick` / `gen` (0).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrailOut {
    pub id: u32,
    #[serde(default)]
    pub gen: u32,
    #[serde(default)]
    pub tick: u64,
    pub pts: Vec<[f32; 3]>,
}

/// What does not change in a battle: the caller fills it at the start.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub map: String,
    pub mode: String,
    pub source_game: String,
    pub seed: u32,
    pub difficulty: String,
    /// The mission file the battle ran (`bf1_kas2_con_cw.ron`; card ERAM):
    /// which script set the teams, the counts and the commander's lines.
    /// Empty without a mission and in a file written before the card.
    #[serde(default)]
    pub mission: String,
    /// The sim team (0 or 1) the player's army sits on.
    #[serde(default)]
    pub player_team: u8,
    /// The checkout's HEAD when the battle started, and whether tracked
    /// files were modified then. NOT the binary: see `build_commit`.
    pub commit: String,
    pub dirty: bool,
    /// The commit the binary was built from and whether a tracked source
    /// file differed from it at build time (`game/build.rs`): the code this
    /// snapshot belongs to. Empty in a schema 1 file.
    #[serde(default)]
    pub build_commit: String,
    #[serde(default)]
    pub build_dirty: bool,
    /// The executable's modification time (seconds since the epoch).
    pub build_time: u64,
    /// Every `OPENBF_*` variable set.
    pub env: Vec<(String, String)>,
}

impl Meta {
    /// The battle's own fields, plus the commit, the build time and the
    /// environment read from the process.
    pub fn for_run(map: &str, mode: &str, source_game: &str, seed: u32, difficulty: &str) -> Meta {
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        };
        // The game's own switches; the loop launcher's variables are not.
        const LAUNCHER: [&str; 5] = [
            "OPENBF_JOB",
            "OPENBF_LANE",
            "OPENBF_LOOP",
            "OPENBF_EFFORT",
            "OPENBF_FABLE",
        ];
        let mut env: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| k.starts_with("OPENBF_") && !LAUNCHER.iter().any(|l| k.starts_with(l)))
            .collect();
        env.sort();
        Meta {
            map: map.to_owned(),
            mode: mode.to_owned(),
            source_game: source_game.to_owned(),
            seed,
            difficulty: difficulty.to_owned(),
            mission: String::new(),
            player_team: 0,
            commit: git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into()),
            dirty: git(&["status", "--porcelain", "-uno"]).is_some_and(|s| !s.is_empty()),
            build_commit: env!("OPENBF_BUILD_COMMIT").to_owned(),
            build_dirty: env!("OPENBF_BUILD_DIRTY") == "1",
            build_time: std::env::current_exe()
                .and_then(std::fs::metadata)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs()),
            env,
        }
    }
}

/// The names the rows' indices point at.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Names {
    pub factions: [String; 2],
    /// Per team, by `Probe::class`.
    pub classes: [Vec<String>; 2],
    pub posts: Vec<String>,
    /// By vehicle class.
    pub vehicles: Vec<String>,
    pub weapons: Vec<String>,
    /// The map bodies the rows name (`Probe::body`): `<class> <instance>`.
    #[serde(default)]
    pub bodies: BTreeMap<u32, String>,
    /// The sound rows' properties (`SoundRow::name`) and take files.
    #[serde(default)]
    pub sounds: Vec<String>,
    #[serde(default)]
    pub takes: Vec<String>,
}

/// The file's header.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Header {
    /// Why it was written: `hotkey`, `exit`, `stuck`, `panic: ...`, `manual`,
    /// or a headless trigger.
    pub reason: String,
    #[serde(flatten)]
    pub meta: Meta,
    pub window: [u32; 2],
    pub tick: u64,
    /// Game time, seconds.
    pub time: f32,
    /// The last frames' rate, and their mean / 99th-percentile / longest
    /// time in milliseconds (0 in a headless run).
    pub fps: f32,
    pub frame_ms: [f32; 3],
    /// The human's unit slot, -1 none (a headless round).
    pub human: i32,
    /// The tick the trails were last sampled at (each trail's own last point:
    /// `trails[].tick`).
    pub trail_tick: u64,
    /// The recorder's own memory, MB.
    pub ring_mb: f32,
}

/// The unit a bug report's "this guy" is: what the human was looking at.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Focus {
    /// The unit slot, -1 none.
    pub unit: i32,
    /// How it was picked: `reticle` (the sim's pick), `magnet` (the aim
    /// assist's target), `view` (nearest to the view axis within 8 degrees),
    /// `trigger` (a headless trigger's unit), `none`.
    pub how: String,
    /// The camera's eye and view direction.
    pub eye: [f32; 3],
    pub dir: [f32; 3],
    /// The world point under the reticle and metres to it; `dist` -1 = sky.
    pub point: [f32; 3],
    pub dist: f32,
}

/// The file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u32,
    pub header: Header,
    /// What the short keys mean.
    pub legend: BTreeMap<String, String>,
    pub names: Names,
    pub focus: Focus,
    /// Oldest first: every 0.5 s, then every tick for the last 5 s.
    pub human: Vec<HumanRow>,
    /// Oldest first, 0.5 s apart.
    pub frames: Vec<Frame>,
    /// Oldest first, 2 s apart.
    pub world: Vec<WorldFrame>,
    pub events: Vec<EventRow>,
    pub trails: Vec<TrailOut>,
    /// Oldest first: the last 60 s of sound starts (none in a file written
    /// before card SNDB, by a headless round or with no sound device).
    #[serde(default)]
    pub sounds: Vec<SoundRow>,
    /// The battle's frame record (card HITCH, `game::frame_profile`): the
    /// frame times' summary and the slow frames with their breakdown. None
    /// before schema 6, in a headless round or with the record off.
    #[serde(default)]
    pub hitches: Option<crate::frame_profile::Hitches>,
    /// Card WHY (schema 9): each unit slot's last reasons — why it entered
    /// a behaviour or why its walk failed — as at its last unit frame (a
    /// dead unit's as it died, until its slot respawns).
    #[serde(default)]
    pub why: Vec<WhyUnit>,
    /// Card DPRX (schema 10): the whole battle's tally of the human (in a
    /// headless round, the human proxy's), [`session::Session`]. None before
    /// schema 10 and with nobody to follow.
    #[serde(default)]
    pub session: Option<session::Session>,
}

/// One unit's reasons ([`Snapshot::why`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WhyUnit {
    pub id: u32,
    pub gen: u32,
    /// Oldest first.
    pub rows: Vec<WhyOut>,
}

/// One reason (`core::ai::why::WhyRow`): the tick, the behaviour entered
/// (or, `fail`, the one the bot was in when its walk failed), the code, and
/// the nav arc a failed leg walked (-1 none).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WhyOut {
    pub tick: u64,
    pub doing: crate::unit_probe::Doing,
    pub why: core::ai::why::Why,
    pub arc: i32,
    pub fail: bool,
}

/// The stuck alarm's watch on the human.
#[derive(Clone, Copy, Default)]
struct Alarm {
    anchor: [f32; 3],
    still: u32,
    pushing: u32,
    last: Option<u64>,
}

/// Everything a snapshot is written from. It holds no reference to the sim,
/// so the panic hook can write it.
#[derive(Clone)]
pub struct Ring {
    meta: Meta,
    names: Names,
    /// Every map body's label, by index; a snapshot keeps the ones named.
    body_labels: Vec<String>,
    frames: Wheel<Frame>,
    human_fast: Wheel<HumanRow>,
    human_slow: Wheel<HumanRow>,
    world: Wheel<WorldFrame>,
    events: VecDeque<EventRow>,
    sounds: VecDeque<SoundKept>,
    /// The property names by hash and the baked root the takes are under.
    sound_names: HashMap<u32, String>,
    sound_root: PathBuf,
    /// The take files the rows index, and each one's index.
    takes: Vec<String>,
    take_ix: HashMap<PathBuf, u32>,
    /// The ticks at which the one-shots sounding now end.
    sounding: Vec<u64>,
    trails: Vec<Trail>,
    /// Each unit slot's no-progress run at the last frame.
    prev_stuck: Vec<u16>,
    /// Card WHY: each unit slot's generation and reason log at its last
    /// frame (copied: no allocation per frame).
    whys: Vec<Option<(u32, core::ai::why::WhyLog)>>,
    frame_ms: Wheel<f32>,
    window: [u32; 2],
    tick: u64,
    trail_tick: u64,
    human: i32,
    focus: Focus,
    alarm: Alarm,
    /// The windowed game's frame record (card HITCH), read at a snapshot.
    hitches: Option<Arc<Mutex<crate::frame_profile::Record>>>,
    /// Card DPRX: the match-long tally of the human or the proxy.
    session: session::Tally,
}

fn r2(v: f32) -> f32 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        // JSON has no NaN; the sample's "no graph" reads as far off.
        -999.0
    }
}

fn r2v(v: [f32; 3]) -> [f32; 3] {
    [r2(v[0]), r2(v[1]), r2(v[2])]
}

/// A probe as the file keeps it: centimetres, and no NaN.
fn tidy(mut p: Probe) -> Probe {
    p.pos = r2v(p.pos);
    p.wp = r2v(p.wp);
    p.walk_at = r2v(p.walk_at);
    p.hold_goal = r2v(p.hold_goal);
    p.hold_spot = r2v(p.hold_spot);
    p.board_at = r2v(p.board_at);
    for f in p.walk_dir.iter_mut() {
        *f = (*f * 1000.0).round() / 1000.0;
    }
    for f in [
        &mut p.yaw,
        &mut p.speed,
        &mut p.hp,
        &mut p.floor_dy,
        &mut p.level,
        &mut p.overhead,
        &mut p.wp_d,
        &mut p.stop_d,
        &mut p.target_d,
        &mut p.hold_left,
        &mut p.hold_commit,
    ] {
        *f = r2(*f);
    }
    p.stop_n = [r2(p.stop_n[0]), r2(p.stop_n[1])];
    p
}

impl Ring {
    fn new(meta: Meta, names: Names, body_labels: Vec<String>) -> Ring {
        Ring {
            meta,
            names,
            body_labels,
            frames: Wheel::new(FRAMES),
            human_fast: Wheel::new(HUMAN_TICKS),
            human_slow: Wheel::new(FRAMES),
            world: Wheel::new(WORLD_FRAMES),
            events: VecDeque::with_capacity(EVENTS_MAX),
            sounds: VecDeque::new(),
            sound_names: HashMap::new(),
            sound_root: PathBuf::new(),
            takes: Vec::new(),
            take_ix: HashMap::new(),
            sounding: Vec::new(),
            trails: Vec::new(),
            prev_stuck: Vec::new(),
            whys: Vec::new(),
            frame_ms: Wheel::new(FRAME_TIMES),
            window: [0; 2],
            tick: 0,
            trail_tick: 0,
            human: -1,
            focus: Focus {
                unit: -1,
                how: "none".into(),
                ..Focus::default()
            },
            alarm: Alarm::default(),
            hitches: None,
            session: session::Tally::default(),
        }
    }

    fn event(&mut self, row: EventRow) {
        Self::push_event(&mut self.events, row);
    }

    fn push_event(events: &mut VecDeque<EventRow>, row: EventRow) {
        if events.len() == EVENTS_MAX {
            events.pop_front();
        }
        events.push_back(row);
    }

    fn expire_sounds(&mut self, tick: u64) {
        while self
            .sounds
            .front()
            .is_some_and(|s| s.row.tick + SOUND_TICKS <= tick)
        {
            self.sounds.pop_front();
        }
    }

    /// The one-shots the mixer was handed at `tick`, `loops` engine layers
    /// looping; `handed` is left empty.
    fn sounds_of(&mut self, tick: u64, handed: &mut Vec<Handed>, loops: usize) {
        self.expire_sounds(tick);
        self.sounding.retain(|end| *end > tick);
        for h in handed.drain(..) {
            let take = match self.take_ix.get(&h.path) {
                Some(i) => *i,
                None => {
                    let i = self.takes.len() as u32;
                    let rel = h.path.strip_prefix(&self.sound_root).unwrap_or(&h.path);
                    self.takes.push(rel.to_string_lossy().into_owned());
                    self.take_ix.insert(h.path, i);
                    i
                }
            };
            self.sounding
                .push(tick + (h.secs * core::TICK_HZ as f32).ceil().max(1.0) as u64);
            if self.sounds.len() == SOUNDS_MAX {
                self.sounds.pop_front();
            }
            self.sounds.push_back(SoundKept {
                hash: h.hash,
                row: SoundRow {
                    tick,
                    name: 0,
                    take,
                    gain: (h.gain * 1e4).round() / 1e4,
                    pitch: (h.pitch * 1e4).round() / 1e4,
                    dist: r2(h.dist),
                    voices: (self.sounding.len() + loops).min(usize::from(u16::MAX)) as u16,
                    secs: r2(h.secs),
                    ears: [h.ears.0, h.ears.1].map(|e| (e * 1e4).round() / 1e4),
                },
            });
        }
    }

    fn events_of(&mut self, sim: &Sim) {
        let at = |h: core::unit::UnitHandle| {
            sim.units
                .get(h.index as usize)
                .map_or([0.0; 3], |u| r2v(u.soldier.pos.to_array()))
        };
        let tick = sim.tick;
        let human = |h: core::unit::UnitHandle| {
            sim.units.get(h.index as usize).is_some_and(|u| u.is_player)
        };
        let weapon = |h: core::unit::UnitHandle| {
            sim.units
                .get(h.index as usize)
                .map_or(-1, |u| u.weapon_class as i32)
        };
        let rides_human = |vh: core::vehicle::VehicleHandle| {
            sim.vehicles.get(vh.index as usize).is_some_and(|x| {
                x.gen == vh.gen && x.seats.iter().filter_map(|s| s.occupant).any(human)
            })
        };
        for ev in &sim.events {
            let mut v = Vec::new();
            let (kind, a, b, c, pos) = match *ev {
                Event::Shot {
                    unit,
                    weapon_class,
                    origin,
                    ..
                } if human(unit) => (
                    Ev::Shot,
                    unit.index as i32,
                    -1,
                    weapon_class as i32,
                    r2v(origin.to_array()),
                ),
                Event::Hit {
                    shooter,
                    victim,
                    damage,
                    age,
                    health_left,
                    ..
                } if human(shooter) || human(victim) => {
                    v = vec![r2(damage), r2(health_left), (age * 1e3).round() / 1e3];
                    (
                        Ev::Hit,
                        shooter.index as i32,
                        victim.index as i32,
                        weapon(shooter),
                        at(victim),
                    )
                }
                Event::VehicleHit {
                    shooter,
                    vehicle,
                    damage,
                    ..
                } if shooter.is_some_and(human) || rides_human(vehicle) => {
                    let v_now = sim.vehicle(vehicle);
                    v = vec![r2(damage), r2(v_now.map_or(0.0, |x| x.health.cur.max(0.0)))];
                    (
                        Ev::VehicleHit,
                        shooter.map_or(-1, |s| s.index as i32),
                        vehicle.index as i32,
                        shooter.map_or(-1, weapon),
                        r2v(sim
                            .vehicles
                            .get(vehicle.index as usize)
                            .map_or([0.0; 3], |x| x.pos.to_array())),
                    )
                }
                Event::Kill { shooter, victim } => (
                    Ev::Kill,
                    shooter.index as i32,
                    victim.index as i32,
                    sim.units
                        .get(shooter.index as usize)
                        .map_or(-1, |u| u.weapon_class as i32),
                    at(victim),
                ),
                Event::EnvDeath { unit, cause } => (
                    match cause {
                        DeathCause::Suicide => Ev::Suicide,
                        DeathCause::HeroExhausted => Ev::HeroExhausted,
                        DeathCause::DeathRegion => Ev::DeathRegion,
                        DeathCause::Fall => Ev::Fall,
                        DeathCause::VehicleDestroyed => Ev::RiderDeath,
                        DeathCause::Blast => Ev::Blast,
                        DeathCause::KillFloor => Ev::KillFloor,
                        DeathCause::UnderMap => Ev::UnderMap,
                        DeathCause::Lost => Ev::Lost,
                        DeathCause::Stalled => Ev::Recycle,
                        DeathCause::Water | DeathCause::DamageRegion => Ev::Water,
                    },
                    unit.index as i32,
                    -1,
                    -1,
                    at(unit),
                ),
                Event::Relocated { unit, from, to } => {
                    v = from.to_array().iter().map(|&x| r2(x)).collect();
                    (Ev::Relocate, unit.index as i32, -1, -1, r2v(to.to_array()))
                }
                Event::Respawn { unit } => (Ev::Spawn, unit.index as i32, -1, -1, at(unit)),
                Event::Despawn { unit } => (Ev::Despawn, unit.index as i32, -1, -1, at(unit)),
                Event::Captured { cp, team } => {
                    (Ev::Captured, cp as i32, i32::from(team), -1, [0.0; 3])
                }
                Event::Neutralized { cp, by } => {
                    (Ev::Neutralized, cp as i32, i32::from(by), -1, [0.0; 3])
                }
                Event::Saved { cp, team } => (Ev::Saved, cp as i32, i32::from(team), -1, [0.0; 3]),
                Event::TeamDefeated { team } => (Ev::Defeated, i32::from(team), -1, -1, [0.0; 3]),
                Event::TimeUp { winner } => (Ev::TimeUp, i32::from(winner), -1, -1, [0.0; 3]),
                Event::VehicleDestroyed {
                    vehicle,
                    class,
                    pos,
                    killer,
                    ..
                } => (
                    Ev::VehicleLost,
                    vehicle.index as i32,
                    killer.map_or(-1, |k| k.index as i32),
                    class as i32,
                    r2v(pos.to_array()),
                ),
                Event::Command { slot, kind, units } => (
                    Ev::Order,
                    i32::from(slot),
                    i32::from(kind),
                    i32::from(units),
                    [0.0; 3],
                ),
                _ => continue,
            };
            self.event(EventRow {
                tick,
                kind,
                a,
                b,
                c,
                pos,
                text: String::new(),
                v,
            });
        }
    }

    fn human_row(walk: &WalkSim, pad: PadState) -> HumanRow {
        let sim = &walk.sim;
        let u = &sim.units[walk.player.index as usize];
        let cam = walk.camera();
        let v = u.soldier.vel;
        let pick = sim.reticle_pick.first().copied().flatten();
        HumanRow {
            tick: sim.tick,
            alive: u.alive,
            pos: r2v(u.soldier.pos.to_array()),
            yaw: r2(u.soldier.yaw),
            pitch: r2(u.soldier.pitch),
            cam: r2v(cam.position.to_array()),
            cam_yaw: r2(cam.yaw),
            cam_pitch: r2(cam.pitch),
            lx: pad.lx,
            ly: pad.ly,
            rx: pad.rx,
            ry: pad.ry,
            buttons: pad.buttons,
            mx: pad.mx,
            my: pad.my,
            hp: r2(u.health.cur),
            stance: crate::unit_probe::stance_code(u.soldier.stance),
            grounded: u.soldier.grounded,
            speed: r2(glam::Vec2::new(v.x, v.z).length()),
            weapon: u.weapon_class as u32,
            clip: u.weapon.clip,
            spare: u.weapon.spare,
            reticle: pick.map_or(-1, |(t, _)| target_code(t)),
            reticle_k: pick.map_or(0, |(_, k)| k),
            magnet: sim.magnet_at(0).map_or(-1, target_code),
            veh: u.mount.map_or(-1, |m| m.vehicle.index as i32),
            seat: u.mount.map_or(0, |m| m.seat),
        }
    }

    /// One stepped tick. Returns whether the stuck alarm fires.
    fn tick(&mut self, walk: &WalkSim, pad: Option<PadState>) -> bool {
        let sim = &walk.sim;
        let t = sim.tick;
        self.tick = t;
        self.expire_sounds(t);
        self.events_of(sim);
        let human = sim
            .units
            .get(walk.player.index as usize)
            .filter(|u| u.is_player && u.gen == walk.player.gen);
        // Card DPRX: the tally follows the human's slot (alive or not), else
        // the first proxy's. A vacated slot (the sweep's bot in slot 0) holds
        // no human.
        let seated = sim
            .players
            .iter()
            .flatten()
            .any(|h| h.index == walk.player.index);
        let subject = if seated
            && sim
                .units
                .get(walk.player.index as usize)
                .is_some_and(|u| u.is_player)
        {
            Some((walk.player.index as usize, "human"))
        } else {
            sim.units
                .iter()
                .position(|u| u.proxy_human.is_some())
                .map(|i| (i, "proxy"))
        };
        self.session.tick(sim, subject);
        self.human = human.map_or(-1, |_| walk.player.index as i32);
        let mut alarm = false;
        if let Some(u) = human {
            let pad = pad.unwrap_or_default();
            let row = Ring::human_row(walk, pad);
            *self.human_fast.next() = row;
            if t.is_multiple_of(UNIT_PERIOD) {
                *self.human_slow.next() = row;
                self.focus = focus_of(walk);
            }
            alarm = self.watch(u, pad, t);
        }
        if t.is_multiple_of(UNIT_PERIOD) {
            self.unit_frame(sim, t);
        }
        // 4 Hz on a 30 Hz tick: the ticks where the quarter-second turns.
        let hz = u64::from(core::TICK_HZ);
        if t * 4 / hz != t.saturating_sub(1) * 4 / hz {
            self.trail_points(sim, t);
        }
        if t.is_multiple_of(WORLD_PERIOD) {
            self.world_frame(sim, t);
        }
        alarm
    }

    fn unit_frame(&mut self, sim: &Sim, t: u64) {
        if self.prev_stuck.len() < sim.units.len() {
            self.prev_stuck.resize(sim.units.len(), 0);
        }
        let f = self.frames.next();
        f.tick = t;
        f.units.clear();
        if self.whys.len() < sim.units.len() {
            self.whys.resize(sim.units.len(), None);
        }
        for (h, u) in sim.alive_units() {
            if !u.is_player {
                self.whys[h.index as usize] = Some((h.gen, u.ai.why));
            }
            let p = tidy(Probe::of(sim, h, u, t, true));
            let was = std::mem::replace(&mut self.prev_stuck[h.index as usize], p.stuck);
            if was < STUCK_EVENT_TICKS && p.stuck >= STUCK_EVENT_TICKS {
                Self::push_event(
                    &mut self.events,
                    EventRow {
                        tick: t,
                        kind: Ev::Stuck,
                        a: p.id as i32,
                        b: p.body,
                        c: -1,
                        pos: p.pos,
                        text: String::new(),
                        v: Vec::new(),
                    },
                );
            }
            f.units.push(p);
        }
    }

    fn trail_points(&mut self, sim: &Sim, t: u64) {
        if self.trails.len() < sim.units.len() {
            self.trails.resize(sim.units.len(), Trail::default());
        }
        self.trail_tick = t;
        for (h, u) in sim.alive_units() {
            let tr = &mut self.trails[h.index as usize];
            if tr.gen != h.gen {
                // A new life in the slot: its trail starts over.
                *tr = Trail {
                    gen: h.gen,
                    ..Trail::default()
                };
            }
            tr.pts[usize::from(tr.head)] = r2v(u.soldier.pos.to_array());
            tr.head = (tr.head + 1) % TRAIL_LEN as u8;
            tr.n = (tr.n + 1).min(TRAIL_LEN as u8);
            tr.last = t;
        }
    }

    fn world_frame(&mut self, sim: &Sim, t: u64) {
        let w = self.world.next();
        w.tick = t;
        w.points = sim.team_points().map_or([0; 2], |(points, _)| points);
        for k in 0..2 {
            let team = &sim.battle.teams[k];
            w.tickets[k] = team.tickets();
            w.bleed[k] = r2(team.bleed_per_sec);
            w.alive[k] = team.alive();
        }
        w.posts.clear();
        w.posts.extend(sim.cps.iter().enumerate().map(|(i, c)| {
            let inside = sim.cp_inside(i);
            let n = |t: usize| {
                inside
                    .get(t)
                    .map_or(0, |&n| n.min(u32::from(u16::MAX)) as u16)
            };
            PostRow {
                owner: c.owner.map_or(-1, |o| o as i8),
                cap: r2(c.capture),
                cap_team: c.capture_team.map_or(-1, |o| o as i8),
                neutral: r2(c.neutral),
                inside: [n(0), n(1)],
                times: [c.def.capture_time, c.def.neutralize_time],
            }
        }));
        w.vehicles.clear();
        w.vehicles.extend(sim.alive_vehicles().map(|(h, v)| {
            let mut riders = [-1; 6];
            for (r, s) in riders.iter_mut().zip(&v.seats) {
                *r = s.occupant.map_or(-1, |o| o.index as i32);
            }
            VehicleRow {
                id: h.index,
                class: v.class,
                kind: v.kind as u8,
                team: v.team.map_or(-1, |t| t as i8),
                pos: r2v(v.pos.to_array()),
                hp: r2(v.health.cur),
                riders,
            }
        }));
        w.doors.clear();
        w.doors.extend(sim.doors.iter().map(|d| r2(d.open)));
    }

    /// The stuck alarm (module header): the human pushing the stick for most
    /// of [`ALARM_TICKS`] without leaving [`ALARM_RADIUS`].
    fn watch(&mut self, u: &core::unit::Unit, pad: PadState, t: u64) -> bool {
        let a = &mut self.alarm;
        let p = u.soldier.pos;
        let far = glam::Vec3::from(a.anchor).distance(p) > ALARM_RADIUS;
        if !u.alive || u.mount.is_some() || far {
            *a = Alarm {
                anchor: p.to_array(),
                last: a.last,
                ..Alarm::default()
            };
            return false;
        }
        a.still += 1;
        let stick = glam::Vec2::new(PadState::axis(pad.lx), PadState::axis(pad.ly));
        if stick.length() > 0.5 {
            a.pushing += 1;
        }
        if a.still < ALARM_TICKS {
            return false;
        }
        let fire = a.pushing as f32 >= ALARM_PUSH_SHARE * a.still as f32
            && a.last.is_none_or(|l| t >= l + ALARM_EVERY);
        // Either way the window starts over.
        a.still = 0;
        a.pushing = 0;
        if fire {
            a.last = Some(t);
        }
        fire
    }

    /// The recorder's own memory: every ring at its full size.
    fn bytes(&self) -> usize {
        use std::mem::size_of;
        let units: usize = self
            .frames
            .items
            .iter()
            .map(|f| f.units.capacity() * size_of::<Probe>())
            .sum();
        let world: usize = self
            .world
            .items
            .iter()
            .map(|w| {
                w.posts.capacity() * size_of::<PostRow>()
                    + w.vehicles.capacity() * size_of::<VehicleRow>()
                    + w.doors.capacity() * 4
            })
            .sum();
        units
            + world
            + (HUMAN_TICKS + FRAMES) * size_of::<HumanRow>()
            + self.events.capacity() * size_of::<EventRow>()
            + self.sounds.capacity() * size_of::<SoundKept>()
            + self.takes.iter().map(|t| 2 * t.len() + 64).sum::<usize>()
            + self.trails.capacity() * size_of::<Trail>()
            + self.body_labels.iter().map(|l| l.len() + 24).sum::<usize>()
    }

    /// The ring as a file, oldest first.
    fn snapshot(&self, reason: &str) -> Snapshot {
        let mut ms: Vec<f32> = self.frame_ms.iter().copied().collect();
        ms.sort_by(f32::total_cmp);
        let mean = ms.iter().fold(0.0, |a, b| a + b) / ms.len().max(1) as f32;
        let p99 = ms
            .get((ms.len() * 99 / 100).min(ms.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0);
        let frames: Vec<Frame> = self.frames.iter().cloned().collect();
        // The human every 0.5 s up to where the per-tick rows begin.
        let first_fast = self.human_fast.iter().next().map_or(u64::MAX, |r| r.tick);
        let human: Vec<HumanRow> = self
            .human_slow
            .iter()
            .filter(|r| r.tick < first_fast)
            .chain(self.human_fast.iter())
            .copied()
            .collect();
        let mut names = self.names.clone();
        let mut named = |b: i32| {
            if let Some(l) = usize::try_from(b)
                .ok()
                .and_then(|i| self.body_labels.get(i))
            {
                names.bodies.entry(b as u32).or_insert_with(|| l.clone());
            }
        };
        for p in frames.iter().flat_map(|f| &f.units) {
            named(p.body);
        }
        for e in self.events.iter().filter(|e| e.kind == Ev::Stuck) {
            named(e.b);
        }
        // The sound rows name their property and take by this file's tables.
        let mut sound_ix: HashMap<u32, u32> = HashMap::new();
        let mut take_map: HashMap<u32, u32> = HashMap::new();
        let sounds: Vec<SoundRow> = self
            .sounds
            .iter()
            .map(|k| {
                let n = names.sounds.len() as u32;
                let name = *sound_ix.entry(k.hash).or_insert(n);
                if name == n {
                    names.sounds.push(
                        self.sound_names
                            .get(&k.hash)
                            .cloned()
                            .unwrap_or_else(|| format!("{:08x}", k.hash)),
                    );
                }
                let t = names.takes.len() as u32;
                let take = *take_map.entry(k.row.take).or_insert(t);
                if take == t {
                    names.takes.push(self.takes[k.row.take as usize].clone());
                }
                SoundRow {
                    name,
                    take,
                    ..k.row
                }
            })
            .collect();
        let mut legend: BTreeMap<String, String> = crate::unit_probe::LEGEND
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        for (k, v) in [
            ("ticks", "30 per second of game time; header.time = tick / 30"),
            ("frames", "every alive unit every 0.5 s, oldest first; keys below"),
            ("human", "the player: every 0.5 s, then every tick for the last 5 s; lx/ly/rx/ry sticks -32767..32767; buttons bits 1 fire 2 jump 4 roll/stance 8 sprint 16 zoom 32 reload 64 action 128 switch 256 secondary fire 512-4096 squad follow / hold / at ease / move out 8192 / 16384 spawn class next / prev 32768 switch secondary 65536-524288 spawn post up / down / left / right 1048576 spawn accept 2097152 spawn back 4194304 pause-menu respawn 8388608 mouse aim (aim assist off); mx/my the mouse look the tick took, 1/65536 rad (mx + turns right, my + looks up), 0 = none or a controller"),
            ("world", "every 2 s: tickets, bleed/s, alive, posts (owner -1 neutral, cap charge and cap_team its team, neutral the neutralise charge, inside the counting units per team, times CaptureTime / NeutralizeTime), vehicles (kind 0 hover 1 walker 2 flyer 3 turret; riders by seat), doors (0 shut..1 open)"),
            ("events", "kill: a killer b victim c the killer's weapon in hand (a seat gun's or a thrown grenade's kill reads as it); death_region/fall/rider_death/blast/kill_floor/under_map/lost/recycle: a unit; spawn/despawn: a unit; captured/neutralized/saved: a post b team; vehicle_lost: a vehicle b killer c class; order: a slot b kind c bots; voice: text; stuck: a unit b body (a no-progress run reached 3 s); shot: the human a fired weapon c; hit: a hit b (one of them the human), c a's weapon in hand, v = damage, b's health after, the round's age (-1 = a blast; schema < 11: 0 = a blast or a first-update bolt); vehicle_hit (schema 12): a (-1 a collision) damaged vehicle or building turret b (the human a or a rider of b), c a's weapon in hand, v = damage, b's health at the tick's end (0 = destroyed)"),
            ("trails", "each unit's last 10 s of positions at 4 Hz, oldest first, ending at its own tick: header.trail_tick for a unit alive then, earlier for one that died (its slot not yet respawned); gen is that life's generation"),
            ("header", "build_commit / build_dirty: the commit the BINARY was built from (the code that ran); commit / dirty: the checkout's HEAD when the battle started; build_time: the executable's mtime, s since the epoch"),
            ("sounds", "every one-shot the mixer was handed in the last 60 s, oldest first: tick, name (names.sounds: the property), take (names.takes: the file under the baked root), gain (linear, the level by distance included), ears (what each ear got [left, right]: the gain x the bus, a 3-D sound split by the PS2's per-ear law, card MIXH), pitch (playback rate), dist (m from the listener, 0 = a 2-D line), voices (one-shots still sounding by their length + engine loops), secs (the take's length at that pitch). Not in it: the engine loops themselves and the music"),
            ("hitches", "the windowed game's frame record (card HITCH): frames, the seconds they span, p50 / p99 / worst ms (a frame = its start to the next frame's start), over = frames longer than limit_ms past the battle's first settle_s seconds, per_min of them, made_frames = frames that made GPU objects; slow = the newest frames over limit_ms, worst = the longest past settle_s; a row: t (s since the battle's first frame), tick, ms, laps (the frame's phases in order: sim = the ticks with their hook, update, draws, render = render_frame with its image wait acquire_ms, other = the time outside them), ticks run, ai_ms / hook_ms (inside sim), gpu_ms (with OPENBF_GPU_TIMING=1, a frame or two old), made (kind, label, bytes of each GPU object made)"),
            ("why", "card WHY (schema 9): per unit slot (id, gen), its last 8 reasons oldest first: tick, doing = the behaviour entered (or, fail true, the one it was in when its walk failed), why = the code (recover is entered for a failed-walk code), arc = the nav arc a failed leg walked (-1 none), fail = a failed walk, not a change. Behaviour codes: off_level grenade pickup snipe_spot guard_spot seen hit memory claim order arrived goal_changed target_lost safe recovered claim_ended seated idle set_out. Failed-walk codes (a progress window that failed, by the mover's books): wall (slid or fanned off a wall) rim (a level-edge barrier) body (going round a body) round_spent (the going-round allowance spent) ledge (no floor ahead) boxed (every way shut, backed off) off_graph (no hop: a straight walk) no_gain (the way open, no nearer: pushed, jammed) oscillation (back and forth at its waypoint, card ARCM) driving (a driver's window) return_failed (a lost bot's walk back) gave_up (the goal given up) pinned (sent off along the floor) recycled"),
            ("session", "card DPRX (schema 10): the whole battle's tally of the human (headless: the proxy): subject, unit, team, from/to ticks, kills, deaths (incl. environmental), shots (presses), hits (on enemies, one per victim per tick), lives [spawn tick, end tick] (open: the last still alive), alive_ticks, foot_ticks, dist_m (ground path on foot), moving_ticks (> 0.5 m/s), strafe_ticks/strafe_starts (sideways > 1 m/s and > forward), crouch_/prone_ ticks and starts, engage = seconds from first sight (within 60 m, 40 deg of the aim, clear line) to the first hit on that enemy, engage_dropped = sightings never hit (5 s unseen); scripts/human_profile.py reads it"),
            ("focus", "what the human looked at when it was written: unit slot, how it was picked, the eye, the view direction, the world point under the reticle"),
        ] {
            legend.insert(k.to_owned(), v.to_owned());
        }
        Snapshot {
            schema: SCHEMA,
            header: Header {
                reason: reason.to_owned(),
                meta: self.meta.clone(),
                window: self.window,
                tick: self.tick,
                time: r2(self.tick as f32 * core::TICK_DT),
                fps: if mean > 0.0 { r2(1000.0 / mean) } else { 0.0 },
                frame_ms: [r2(mean), r2(p99), r2(ms.last().copied().unwrap_or(0.0))],
                human: self.human,
                trail_tick: self.trail_tick,
                ring_mb: r2(self.bytes() as f32 / (1024.0 * 1024.0)),
            },
            legend,
            names,
            focus: self.focus.clone(),
            human,
            frames,
            world: self.world.iter().cloned().collect(),
            events: self.events.iter().cloned().collect(),
            sounds,
            trails: self
                .trails
                .iter()
                .enumerate()
                .filter(|(_, t)| t.n > 0)
                .map(|(i, t)| {
                    let n = usize::from(t.n);
                    let start = (usize::from(t.head) + TRAIL_LEN - n) % TRAIL_LEN;
                    TrailOut {
                        id: i as u32,
                        gen: t.gen,
                        tick: t.last,
                        pts: (0..n).map(|k| t.pts[(start + k) % TRAIL_LEN]).collect(),
                    }
                })
                .collect(),
            // `try_lock`: the panic hook may run while a frame holds it.
            hitches: self
                .hitches
                .as_ref()
                .and_then(|h| h.try_lock().ok().map(|r| r.hitches())),
            session: (self.session.out.unit >= 0 && !self.session.out.subject.is_empty())
                .then(|| self.session.out.clone()),
            why: self
                .whys
                .iter()
                .enumerate()
                .filter_map(|(i, w)| {
                    let (gen, log) = w.as_ref()?;
                    let rows: Vec<WhyOut> = log
                        .rows()
                        .map(|r| WhyOut {
                            tick: r.tick,
                            doing: r.doing,
                            why: r.why,
                            arc: if r.arc == u16::MAX {
                                -1
                            } else {
                                i32::from(r.arc)
                            },
                            fail: r.failed,
                        })
                        .collect();
                    (!rows.is_empty()).then_some(WhyUnit {
                        id: i as u32,
                        gen: *gen,
                        rows,
                    })
                })
                .collect(),
        }
    }
}

/// The digest's sound section (card SNDB): the starts of the last `span`
/// seconds (all of them without one), a line per tick and property, and
/// before them a line for every tick of the FILE whose starts sum over
/// [`SOUND_OVERLOAD`] - "N starts, sum G: names", with the largest sum over
/// starts of one take (those add up sample for sample: card SNDX's shotgun).
pub fn sound_digest(snap: &Snapshot, span: Option<f32>) -> String {
    use std::fmt::Write as _;
    let hz = core::TICK_HZ as f32;
    let name = |i: u32| {
        snap.names
            .sounds
            .get(i as usize)
            .map_or("?", String::as_str)
    };
    let take = |i: u32| snap.names.takes.get(i as usize).map_or("?", String::as_str);
    let ago = |tick: u64| (tick as f32 - snap.header.tick as f32) / hz;
    let mut s = String::new();
    if snap.sounds.is_empty() {
        return if snap.schema < 3 {
            "SOUNDS  not recorded (schema < 3: written before card SNDB)\n".to_owned()
        } else {
            "SOUNDS  none (a headless round, no sound device, or 60 s of silence)\n".to_owned()
        };
    }
    let peak = snap.sounds.iter().map(|r| r.voices).max().unwrap_or(0);
    let _ = writeln!(
        s,
        "SOUNDS  {} starts in the file's last {:.0} s, at most {peak} voices at once",
        snap.sounds.len(),
        -ago(snap.sounds[0].tick)
    );
    // Rows of one tick are neighbours: the ring is in tick order.
    let ticks = || snap.sounds.chunk_by(|a, b| a.tick == b.tick);
    // (property, starts, summed gain), in the order they first start.
    let by_name = |rows: &[SoundRow]| {
        let mut out: Vec<(u32, usize, f32)> = Vec::new();
        for r in rows {
            match out.iter_mut().find(|o| o.0 == r.name) {
                Some(o) => {
                    o.1 += 1;
                    o.2 += r.gain;
                }
                None => out.push((r.name, 1, r.gain)),
            }
        }
        out
    };
    let mut over = 0;
    for rows in ticks() {
        let sum: f32 = rows.iter().map(|r| r.gain).sum();
        if sum <= SOUND_OVERLOAD {
            continue;
        }
        over += 1;
        let names: Vec<String> = by_name(rows)
            .iter()
            .map(|&(n, k, g)| format!("{} x{k} ({g:.2})", name(n)))
            .collect();
        let near = rows.iter().map(|r| r.dist).fold(f32::MAX, f32::min);
        // Starts of ONE take add up sample for sample; different takes do not.
        let one = rows
            .iter()
            .map(|r| {
                let same = rows.iter().filter(|o| o.take == r.take);
                (same.clone().map(|o| o.gain).sum::<f32>(), same.count())
            })
            .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a });
        let _ = writeln!(
            s,
            "  OVERLOAD {:+.2}s tick {}: {} starts, sum {sum:.2}: {} - nearest {near:.0} m, one take at most x{} (sum {:.2})",
            ago(rows[0].tick),
            rows[0].tick,
            rows.len(),
            names.join(", "),
            one.1,
            one.0
        );
    }
    if over == 0 {
        let _ = writeln!(s, "  no tick's starts sum over {SOUND_OVERLOAD:.1}");
    }
    let from = span.map_or(0, |x| snap.header.tick.saturating_sub((x * hz) as u64));
    match span {
        Some(x) => {
            let _ = writeln!(s, "  starts, last {x:.0} s (--sounds: all):");
        }
        None => s.push_str("  starts, all:\n"),
    }
    for rows in ticks().filter(|r| r[0].tick >= from) {
        for (n, k, g) in by_name(rows) {
            let r = rows.iter().find(|r| r.name == n).expect("named from it");
            let _ = writeln!(
                s,
                "  {:+7.2}s  {}{}  gain {:.2}{}  pitch {:.2}  {:.0} m  {:.1} s  voices {}  {}",
                ago(r.tick),
                name(n),
                if k > 1 {
                    format!(" x{k}")
                } else {
                    String::new()
                },
                r.gain,
                if k > 1 {
                    format!(" (sum {g:.2})")
                } else {
                    String::new()
                },
                r.pitch,
                r.dist,
                r.secs,
                rows.iter()
                    .filter(|r| r.name == n)
                    .map(|r| r.voices)
                    .max()
                    .unwrap_or(0),
                take(r.take)
            );
        }
    }
    s
}

/// What the human is looking at (module header, [`Focus`]).
fn focus_of(walk: &WalkSim) -> Focus {
    let sim = &walk.sim;
    let cam = walk.camera();
    let (eye, dir) = (cam.position, cam.forward());
    const REACH: f32 = 512.0;
    let world = sim
        .collision
        .ray(eye, dir, REACH, core::collision::Layers::ORDNANCE)
        .map(|(t, _)| t);
    let terrain = sim.terrain_ray(eye, dir, REACH, 0.5);
    let dist = match (world, terrain) {
        (Some(a), Some(b)) => a.min(b),
        (a, b) => a.or(b).unwrap_or(-1.0),
    };
    let reticle = sim.reticle_pick.first().copied().flatten();
    let (unit, how) = if let Some(h) = reticle.and_then(|(t, _)| t.unit()) {
        (h.index as i32, "reticle")
    } else if let Some(h) = sim.magnet_at(0).and_then(AimTarget::unit) {
        (h.index as i32, "magnet")
    } else {
        // The alive unit nearest the view axis, within 8 degrees and 200 m.
        // cos 8 deg, as a constant: this file is under core's banned-math referee.
        let cos_min = 0.990_268_f32;
        sim.alive_units()
            .filter(|(h, _)| *h != walk.player)
            .filter_map(|(h, u)| {
                let to = u.aim_point() - eye;
                let d = to.length();
                let c = to.dot(dir) / d.max(1e-3);
                (d < 200.0 && c >= cos_min).then_some((h, c))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or((-1, "none"), |(h, _)| (h.index as i32, "view"))
    };
    Focus {
        unit,
        how: how.to_owned(),
        eye: r2v(eye.to_array()),
        dir: [
            (dir.x * 1000.0).round() / 1000.0,
            (dir.y * 1000.0).round() / 1000.0,
            (dir.z * 1000.0).round() / 1000.0,
        ],
        point: if dist >= 0.0 {
            r2v((eye + dir * dist).to_array())
        } else {
            [0.0; 3]
        },
        dist: r2(dist),
    }
}

/// The names a battle's indices point at.
pub fn names_of(walk: &WalkSim) -> Names {
    let class_names = |t: usize| -> Vec<String> {
        walk.teams[t]
            .classes
            .iter()
            .map(|c| c.name.clone())
            .collect()
    };
    Names {
        factions: [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()],
        classes: [class_names(0), class_names(1)],
        posts: walk.sim.cps.iter().map(|c| c.def.name.clone()).collect(),
        vehicles: walk
            .vehicle_geometry
            .iter()
            .zip(walk.vehicle_labels.iter().chain(std::iter::repeat(&None)))
            .map(|(g, l)| {
                l.clone()
                    .or_else(|| g.clone())
                    .unwrap_or_else(|| "?".to_owned())
            })
            .collect(),
        weapons: walk
            .sim
            .weapon_classes
            .iter()
            .map(|w| w.name.clone())
            .collect(),
        bodies: BTreeMap::new(),
        sounds: Vec::new(),
        takes: Vec::new(),
    }
}

/// Each body's label with where the body is: a label can be shared by many
/// instances (160 of BF1 Hoth's 395 bodies), a place cannot.
fn with_places(mut labels: Vec<String>, sim: &Sim) -> Vec<String> {
    use std::fmt::Write as _;
    for (l, b) in labels.iter_mut().zip(sim.collision.bodies()) {
        let t = b.to_world.w_axis;
        let _ = write!(l, " @ ({:.1},{:.1},{:.1})", t.x, t.y, t.z);
    }
    labels
}

/// The ring the panic hook writes: the newest recorder's, and its folder.
static CURRENT: Mutex<Option<(Weak<Mutex<Ring>>, PathBuf)>> = Mutex::new(None);
static HOOK: std::sync::Once = std::sync::Once::new();

/// The recorder: a [`Ring`] behind a lock (the panic hook's way in), the
/// folder its files go to and the hotkey's writer thread.
pub struct BlackBox {
    ring: Arc<Mutex<Ring>>,
    dir: PathBuf,
    writer: Option<std::thread::JoinHandle<()>>,
    /// `OPENBF_BLACKBOX_PANIC=<tick>`: panic at that tick (the check that a
    /// panic still writes a file).
    panic_at: Option<u64>,
}

fn locked(ring: &Mutex<Ring>) -> std::sync::MutexGuard<'_, Ring> {
    ring.lock().unwrap_or_else(|e| e.into_inner())
}

impl BlackBox {
    /// A recorder for `walk`'s battle when `on`, else nothing at all: no
    /// ring, no folder, no hook.
    pub fn start(
        on: bool,
        meta: Meta,
        walk: &WalkSim,
        body_labels: Vec<String>,
        dir: PathBuf,
    ) -> Option<BlackBox> {
        on.then(|| BlackBox {
            ring: Arc::new(Mutex::new(Ring::new(
                meta,
                names_of(walk),
                with_places(body_labels, &walk.sim),
            ))),
            dir,
            writer: None,
            panic_at: std::env::var("OPENBF_BLACKBOX_PANIC")
                .ok()
                .and_then(|v| v.parse().ok()),
        })
    }

    /// Write a snapshot when the process panics (reason `panic: <message>`),
    /// after the usual panic message. The newest recorder's ring is the one
    /// written.
    pub fn install_panic_hook(&self) {
        *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Arc::downgrade(&self.ring), self.dir.clone()));
        HOOK.call_once(|| {
            let before = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                before(info);
                let Ok(cur) = CURRENT.try_lock() else { return };
                let Some((ring, dir)) = cur.as_ref() else {
                    return;
                };
                let Some(ring) = ring.upgrade() else { return };
                // The panic may have come from inside the recorder: a held
                // lock is skipped, a poisoned one read as it is.
                let snap = match ring.try_lock() {
                    Ok(r) => r.snapshot(&format!("panic: {info}")),
                    Err(std::sync::TryLockError::Poisoned(e)) => {
                        e.into_inner().snapshot(&format!("panic: {info}"))
                    }
                    Err(std::sync::TryLockError::WouldBlock) => return,
                };
                match write_rotating(dir, &snap) {
                    Ok(p) => eprintln!("blackbox: wrote {}", p.display()),
                    Err(e) => eprintln!("blackbox: {e}"),
                }
            }));
        });
    }

    /// Record the tick the sim just stepped (`pad`: the human's, when there
    /// is one). True when the stuck alarm fires: the caller writes `stuck`.
    pub fn tick(&mut self, walk: &WalkSim, pad: Option<PadState>) -> bool {
        let alarm = locked(&self.ring).tick(walk, pad);
        // The panic path's own check: `OPENBF_BLACKBOX_PANIC=<tick>` panics
        // there, outside the ring's lock, as a bug in the game would.
        if self.panic_at == Some(walk.sim.tick) {
            panic!("OPENBF_BLACKBOX_PANIC: forced at tick {}", walk.sim.tick);
        }
        alarm
    }

    pub fn extend_class_names(&mut self, walk: &WalkSim) {
        let mut ring = locked(&self.ring);
        let names = names_of(walk);
        ring.names.classes = names.classes;
        ring.names.weapons = names.weapons;
    }

    /// Add a `key=value` line to the header's environment list (a headless
    /// run's own settings, card DPRX's proxy).
    pub fn note_env(&mut self, key: &str, value: &str) {
        let mut r = locked(&self.ring);
        r.meta.env.push((key.to_owned(), value.to_owned()));
        r.meta.env.sort();
    }

    /// One rendered frame's time and the window's size, for the header.
    pub fn frame(&mut self, dt: f32, window: [u32; 2]) {
        let mut r = locked(&self.ring);
        *r.frame_ms.next() = dt * 1000.0;
        r.window = window;
    }

    /// The frame record its snapshots carry (card HITCH; None = none).
    pub fn frame_record(&mut self, rec: Option<Arc<Mutex<crate::frame_profile::Record>>>) {
        locked(&self.ring).hitches = rec;
    }

    /// The commander said `line`.
    pub fn voice(&mut self, tick: u64, line: &str) {
        locked(&self.ring).event(EventRow {
            tick,
            kind: Ev::Voice,
            a: -1,
            b: -1,
            c: -1,
            pos: [0.0; 3],
            text: line.to_owned(),
            v: Vec::new(),
        });
    }

    /// The property names the sound rows are written with, and the baked
    /// root their take files are under.
    pub fn sound_names(&mut self, names: HashMap<u32, String>, root: PathBuf) {
        let mut r = locked(&self.ring);
        r.sound_names = names;
        r.sound_root = root;
    }

    /// The one-shots the mixer was handed since the last call
    /// (`SoundLib::handed`, left empty), at `tick`; `loops` engine layers
    /// are looping.
    pub fn sounds(&mut self, tick: u64, handed: &mut Vec<Handed>, loops: usize) {
        if !handed.is_empty() {
            locked(&self.ring).sounds_of(tick, handed, loops);
        }
    }

    /// The file's content now. With `walk`, the focus is what the human
    /// looks at this instant; `focus_unit` names a unit instead (a headless
    /// trigger's).
    pub fn snapshot(
        &self,
        reason: &str,
        walk: Option<&WalkSim>,
        focus_unit: Option<u32>,
    ) -> Snapshot {
        let mut r = locked(&self.ring);
        if let Some(w) = walk.filter(|_| r.human >= 0) {
            r.focus = focus_of(w);
        }
        if let Some(u) = focus_unit {
            r.focus = Focus {
                unit: u as i32,
                how: "trigger".into(),
                ..Focus::default()
            };
        }
        r.snapshot(reason)
    }

    /// Write `last.json` now (rotating the older ones) and return its path.
    /// `wait = false` hands the encoding and the write to a thread — the
    /// hotkey's, so the frame does not wait for the disk.
    pub fn write(&mut self, reason: &str, walk: Option<&WalkSim>, wait: bool) -> PathBuf {
        let snap = self.snapshot(reason, walk, None);
        let dir = self.dir.clone();
        // Bound the hotkey to one worker. The rotation also serializes with
        // the panic hook, which does not pass through this join.
        if let Some(h) = self.writer.take() {
            let _ = h.join();
        }
        let job = move || match write_rotating(&dir, &snap) {
            Ok(p) => eprintln!("blackbox: wrote {} ({})", p.display(), snap.header.reason),
            Err(e) => eprintln!("blackbox: {e}"),
        };
        if wait {
            job();
        } else {
            self.writer = Some(std::thread::spawn(job));
        }
        self.dir.join("last.json")
    }

    /// The recorder's memory, bytes.
    pub fn ring_bytes(&self) -> usize {
        locked(&self.ring).bytes()
    }

    /// Unit frames held (at most [`FRAMES`]).
    pub fn frames(&self) -> usize {
        locked(&self.ring).frames.len()
    }
}

impl Drop for BlackBox {
    fn drop(&mut self) {
        if let Some(h) = self.writer.take() {
            let _ = h.join();
        }
    }
}

/// Write `bytes` to `path` whole or not at all: a temporary file beside it,
/// then a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Write `snap` as `<dir>/last.json`, the file that was there becoming
/// `prev1.json`, that one `prev2.json`, up to [`PREVIOUS`].
pub fn write_rotating(dir: &Path, snap: &Snapshot) -> std::io::Result<PathBuf> {
    let bytes = serde_json::to_vec(snap).map_err(std::io::Error::other)?;
    rotating::write(dir, &bytes)
}

/// Write `snap` as `path`, no rotation (a headless run's own file).
pub fn write_to(path: &Path, snap: &Snapshot) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    write_atomic(
        path,
        &serde_json::to_vec(snap).map_err(std::io::Error::other)?,
    )
}

/// Read a snapshot file.
pub fn read(path: &Path) -> anyhow::Result<Snapshot> {
    use anyhow::Context;
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let snap: Snapshot =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    anyhow::ensure!(
        (1..=SCHEMA).contains(&snap.schema),
        "{}: schema {} (this reader knows 1..={SCHEMA})",
        path.display(),
        snap.schema
    );
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Meta {
        Meta {
            map: "testmap".into(),
            mode: "con".into(),
            source_game: "bf2".into(),
            seed: 7,
            difficulty: "Normal".into(),
            mission: "testmap_con.ron".into(),
            player_team: 0,
            commit: "abc123".into(),
            dirty: true,
            build_commit: "def456".into(),
            build_dirty: false,
            build_time: 1,
            env: vec![("OPENBF_DEV".into(), "1".into())],
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openbf_blackbox_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn walk() -> WalkSim {
        let mut w = WalkSim::new(7, glam::Vec3::ZERO, None);
        w.spawn_dummy_targets();
        w
    }

    #[test]
    fn every_stuck_crossing_in_a_frame_is_recorded_once() {
        let mut w = walk();
        let mut ids = Vec::new();
        for i in 0..12 {
            let h = w
                .sim
                .spawn(&core::unit::UnitSpec::trooper(
                    1,
                    glam::Vec3::new(i as f32, 0.0, 0.0),
                ))
                .expect("spawn");
            w.sim.units[h.index as usize].ai.route = Some(core::ai::Route {
                stuck_run: STUCK_EVENT_TICKS,
                ..core::ai::Route::default()
            });
            ids.push(h.index as i32);
        }
        let mut ring = Ring::new(meta(), Names::default(), Vec::new());
        ring.unit_frame(&w.sim, 1);
        let stuck = |ring: &Ring| {
            ring.events
                .iter()
                .filter(|e| e.kind == Ev::Stuck)
                .map(|e| e.a)
                .collect::<Vec<_>>()
        };
        assert_eq!(stuck(&ring), ids, "all twelve crossings survive");
        ring.unit_frame(&w.sim, 2);
        assert_eq!(stuck(&ring), ids, "a continuing stall is not another edge");
        let route = &mut w.sim.units[ids[0] as usize].ai.route;
        route.as_mut().unwrap().stuck_run = 0;
        ring.unit_frame(&w.sim, 3);
        w.sim.units[ids[0] as usize]
            .ai
            .route
            .as_mut()
            .unwrap()
            .stuck_run = STUCK_EVENT_TICKS;
        ring.unit_frame(&w.sim, 4);
        ids.push(ids[0]);
        assert_eq!(stuck(&ring), ids, "a new stall gets its own event");
    }

    /// Schema 8 (the maintainer 2026-10-27, the wrist rocket's damage and its double
    /// shot could not be told from a snapshot): the human's shots and every
    /// hit he deals or takes are rows with their numbers; bots' own shots and
    /// hits on each other are not.
    #[test]
    fn the_humans_shots_and_hits_are_rows() {
        use core::sim::Event;
        let mut w = walk();
        let me = w.player;
        let bot = |w: &mut WalkSim, x: f32| {
            w.sim
                .spawn(&core::unit::UnitSpec::trooper(
                    1,
                    glam::Vec3::new(x, 0.0, -5.0),
                ))
                .expect("spawn")
        };
        let (b1, b2) = (bot(&mut w, 0.0), bot(&mut w, 2.0));
        let hit = |shooter, victim, damage, age| Event::Hit {
            direct_round: true,
            shooter,
            victim,
            damage,
            age,
            health_left: 290.0,
            dir: glam::Vec3::Z,
        };
        let shot = |unit| Event::Shot {
            unit,
            weapon_class: 0,
            origin: glam::Vec3::ZERO,
            dir: glam::Vec3::Z,
        };
        w.sim.events = vec![
            shot(me),
            shot(b1),
            hit(me, b1, 110.0, 0.03),
            hit(me, b1, 250.0, 0.0),
            hit(b2, me, 20.0, 0.1),
            hit(b2, b1, 20.0, 0.1),
        ];
        let mut ring = Ring::new(meta(), Names::default(), Vec::new());
        ring.events_of(&w.sim);
        let rows: Vec<_> = ring
            .events
            .iter()
            .map(|e| (e.kind, e.a, e.b, e.v.clone()))
            .collect();
        let (m, x1, x2) = (me.index as i32, b1.index as i32, b2.index as i32);
        assert_eq!(
            rows,
            vec![
                (Ev::Shot, m, -1, vec![]),
                (Ev::Hit, m, x1, vec![110.0, 290.0, 0.03]),
                (Ev::Hit, m, x1, vec![250.0, 290.0, 0.0]),
                (Ev::Hit, x2, m, vec![20.0, 290.0, 0.1]),
            ]
        );
        // A row without numbers writes no `v`.
        let json = serde_json::to_string(&ring.events[0]).unwrap();
        assert!(!json.contains("\"v\""), "{json}");
    }

    /// Dev mode off, or `OPENBF_BLACKBOX=0` in it: no recorder — no ring, no
    /// folder, no file.
    #[test]
    fn off_is_no_ring_and_no_file() {
        assert_eq!(switch(None, None), crate::dev::DEFAULT_ON);
        assert!(!switch(Some("0"), None));
        assert!(!switch(None, Some("0")));
        assert!(switch(Some("1"), None));
        assert!(switch(Some("1"), Some("1")));
        assert!(!switch(Some("1"), Some("0")));
        let dir = scratch("off");
        let w = walk();
        let bb = BlackBox::start(switch(Some("0"), None), meta(), &w, Vec::new(), dir.clone());
        assert!(bb.is_none());
        assert!(!dir.exists(), "an off recorder makes no folder");
    }

    /// 70 s of ticks fill the 60 s rings and the oldest rows drop: 120 unit
    /// frames 0.5 s apart ending at the last sample tick, the human's last
    /// 150 ticks one apart with the 0.5 s rows before them, 30 world frames,
    /// a 40-point trail.
    #[test]
    fn the_ring_wraps_at_sixty_seconds() {
        let mut w = walk();
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), scratch("wrap")).expect("on");
        let pad = PadState {
            ly: 20000,
            ..PadState::default()
        };
        let ticks = 70 * u64::from(core::TICK_HZ);
        for _ in 0..ticks {
            w.step_tick(std::slice::from_ref(&pad));
            bb.tick(&w, Some(pad));
        }
        assert_eq!(bb.frames(), FRAMES);
        let s = bb.snapshot("manual", Some(&w), None);
        assert_eq!(s.frames.len(), FRAMES);
        assert_eq!(s.frames.last().expect("frames").tick, ticks);
        assert_eq!(
            s.frames[0].tick,
            ticks - (FRAMES as u64 - 1) * UNIT_PERIOD,
            "the first 10 s dropped"
        );
        assert!(s
            .frames
            .windows(2)
            .all(|p| p[1].tick == p[0].tick + UNIT_PERIOD));
        assert!(s.frames.iter().all(|f| !f.units.is_empty()));
        assert_eq!(s.world.len(), WORLD_FRAMES);
        assert_eq!(s.world.last().expect("world").tick, ticks);
        // The human: per tick for the last 5 s, per 0.5 s before.
        let fast = &s.human[s.human.len() - HUMAN_TICKS..];
        assert_eq!(fast[0].tick, ticks - HUMAN_TICKS as u64 + 1);
        assert!(fast.windows(2).all(|p| p[1].tick == p[0].tick + 1));
        let slow = &s.human[..s.human.len() - HUMAN_TICKS];
        assert!(!slow.is_empty() && slow.iter().all(|r| r.tick.is_multiple_of(UNIT_PERIOD)));
        assert!(slow.last().expect("slow").tick < fast[0].tick);
        assert_eq!(s.human.last().expect("human").ly, 20000, "the pad is kept");
        let me = s
            .trails
            .iter()
            .find(|t| t.id == w.player.index)
            .expect("the human's trail");
        assert_eq!(me.pts.len(), TRAIL_LEN);
        // Warm: another minute changes no capacity.
        let before = bb.ring_bytes();
        for _ in 0..ticks {
            w.step_tick(std::slice::from_ref(&pad));
            bb.tick(&w, Some(pad));
        }
        assert_eq!(bb.ring_bytes(), before, "a warm ring does not grow");
    }

    /// The header says what the report needs, and the file reads back.
    #[test]
    fn the_header_carries_the_run_and_the_file_reads_back() {
        let mut w = walk();
        let dir = scratch("header");
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), dir.clone()).expect("on");
        bb.frame(1.0 / 60.0, [1280, 720]);
        for _ in 0..45 {
            w.step_tick(&[PadState::default()]);
            bb.tick(&w, Some(PadState::default()));
        }
        bb.voice(w.sim.tick, "imp_off_cp_captured");
        let path = bb.write("hotkey", Some(&w), true);
        let s = read(&path).expect("reads back");
        let h = &s.header;
        assert_eq!(s.schema, SCHEMA);
        assert_eq!(h.reason, "hotkey");
        assert_eq!(
            (h.meta.map.as_str(), h.meta.mode.as_str(), h.meta.seed),
            ("testmap", "con", 7)
        );
        assert_eq!((h.meta.commit.as_str(), h.meta.dirty), ("abc123", true));
        assert_eq!(
            (h.meta.build_commit.as_str(), h.meta.build_dirty),
            ("def456", false)
        );
        assert_eq!(h.meta.difficulty, "Normal");
        assert_eq!(h.meta.env, vec![("OPENBF_DEV".to_owned(), "1".to_owned())]);
        assert_eq!((h.tick, h.time), (45, 1.5));
        assert_eq!(h.window, [1280, 720]);
        assert!((h.fps - 60.0).abs() < 0.5, "{}", h.fps);
        assert_eq!(h.human, w.player.index as i32);
        assert!(h.ring_mb > 0.0);
        assert!(s.legend.contains_key("stop/stop_d/stop_n/body"));
        assert!(s.legend.contains_key("doing/recover"));
        assert!(s.legend.contains_key("round/round_near/round_t/round_rim"));
        assert_eq!(s.frames.len(), 3);
        assert!(s.frames[0].units.iter().any(|u| u.human));
        assert_eq!(s.events.last().expect("the line").kind, Ev::Voice);
        assert_eq!(
            s.events.last().expect("the line").text,
            "imp_off_cp_captured"
        );
        assert!(!s.names.weapons.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Schema 5 (card CPBAR): a post's row carries the units that count
    /// inside it per team and its two times, and reads back as the capture
    /// icon's post state.
    #[test]
    fn a_post_row_carries_who_is_inside() {
        let mut w = walk();
        let me = w.player.index as usize;
        let at = w.sim.units[me].soldier.pos;
        let team = w.sim.units[me].team;
        let ci = w.sim.add_cp(
            core::cp::CpDef {
                name: "cp".into(),
                pos: at,
                capture: core::region::Region::sphere(at, 8.0),
                control: None,
                value_bleed: 10,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: Vec::new(),
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(1 - team),
        );
        let dir = scratch("posts");
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), dir.clone()).expect("on");
        for _ in 0..61 {
            w.step_tick(&[PadState::default()]);
            bb.tick(&w, Some(PadState::default()));
        }
        let inside = w.sim.cp_inside(ci);
        assert!(inside[usize::from(team)] >= 1, "{inside:?}");
        let s = read(&bb.write("hotkey", Some(&w), true)).expect("reads back");
        let row = s.world.last().expect("a world frame").posts[ci];
        assert_eq!(row.times, [10.0, 12.0]);
        assert!(row.inside[usize::from(team)] >= 1, "{row:?}");
        let st = row.state();
        assert_eq!((st.owner, st.neutralize_time), (Some(1 - team), 12.0));
        assert!(st.neutral > 0.0, "his side neutralises it: {row:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A dead unit's trail ends at its death and says so (`TrailOut::tick`);
    /// an alive unit's ends at the header's `trail_tick`.
    #[test]
    fn a_dead_units_trail_ends_at_its_death() {
        let mut w = walk();
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), scratch("trail")).expect("on");
        let step = |w: &mut WalkSim, bb: &mut BlackBox, n: u64| {
            for _ in 0..n {
                w.step_tick(&[PadState::default()]);
                bb.tick(w, Some(PadState::default()));
            }
        };
        step(&mut w, &mut bb, 60);
        let victim = w
            .sim
            .alive_units()
            .map(|(h, _)| h)
            .find(|h| *h != w.player)
            .expect("a dummy");
        w.sim.units[victim.index as usize].alive = false;
        let died = w.sim.tick;
        step(&mut w, &mut bb, 90);
        let s = bb.snapshot("manual", Some(&w), None);
        let end = s.header.trail_tick;
        assert!(end > died + 60);
        for t in &s.trails {
            if t.id == victim.index {
                assert!(t.tick <= died && t.tick + 8 > died, "{} vs {died}", t.tick);
                assert_eq!(t.gen, victim.gen);
            } else {
                assert_eq!(t.tick, end, "unit {} is alive", t.id);
            }
        }
        assert!(s.trails.iter().any(|t| t.id == victim.index));
    }

    /// A schema 1 file (the maintainer's own snapshots: no build commit, no walk / hold
    /// fields, no trail tick) still reads; a newer schema does not.
    #[test]
    fn a_schema_one_file_still_reads() {
        let mut w = walk();
        let dir = scratch("schema1");
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), dir.clone()).expect("on");
        for _ in 0..45 {
            w.step_tick(&[PadState::default()]);
            bb.tick(&w, Some(PadState::default()));
        }
        let mut v = serde_json::to_value(bb.snapshot("manual", Some(&w), None)).expect("json");
        v["schema"] = 1.into();
        let drop = |o: &mut serde_json::Value, keys: &[&str]| {
            let o = o.as_object_mut().expect("object");
            for k in keys {
                assert!(o.remove(*k).is_some(), "{k} is written");
            }
        };
        drop(&mut v["header"], &["build_commit", "build_dirty"]);
        for t in v["trails"].as_array_mut().expect("trails") {
            drop(t, &["tick", "gen"]);
        }
        for f in v["frames"].as_array_mut().expect("frames") {
            for u in f["units"].as_array_mut().expect("units") {
                drop(
                    u,
                    &[
                        "walk",
                        "walk_at",
                        "walk_dir",
                        "hold_goal",
                        "hold_spot",
                        "hold_left",
                        "hold_side",
                        "hold_commit",
                        "hold_blocked",
                        "board_at",
                    ],
                );
            }
        }
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("old.json");
        std::fs::write(&path, serde_json::to_vec(&v).expect("bytes")).expect("write");
        let s = read(&path).expect("schema 1 reads");
        assert_eq!(s.schema, 1);
        assert!(s.header.meta.build_commit.is_empty());
        assert_eq!(s.header.meta.commit, "abc123");
        assert!(s.trails.iter().all(|t| t.tick == 0));
        assert!(!s.frames.is_empty() && s.frames[0].units.iter().all(|u| u.walk == 0));
        v["schema"] = (SCHEMA + 1).into();
        std::fs::write(&path, serde_json::to_vec(&v).expect("bytes")).expect("write");
        assert!(read(&path).is_err(), "a newer schema is refused");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Card SNDB: 8 starts of one property at the ear in one tick are an
    /// overload line of the digest, by name; the rows come back from the
    /// file with their take, gain, pitch, distance and voices; a lone start
    /// is no overload; a row older than 60 s is dropped.
    #[test]
    fn eight_starts_in_a_tick_are_an_overload_line() {
        let w = walk();
        let dir = scratch("sounds");
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), dir.clone()).expect("on");
        let fire = crate::classdb::fnv1a_lowercase("cis_weap_inf_rocket_launcher_fire");
        let step = crate::classdb::fnv1a_lowercase("a_step");
        bb.sound_names(
            HashMap::from([(fire, "cis_weap_inf_rocket_launcher_fire".to_owned())]),
            PathBuf::from("/baked"),
        );
        let handed = |hash, file: &str, gain, dist| Handed {
            hash,
            path: PathBuf::from("/baked").join(file),
            gain,
            ears: (gain * 0.5, gain * 0.5),
            pitch: 1.0,
            dist,
            secs: 2.0,
        };
        // Dropped by the time of the snapshot: 61 s before its last tick.
        let mut old = vec![handed(step, "m/sounds/1/old.wav", 0.9, 3.0)];
        bb.sounds(100, &mut old, 0);
        assert!(old.is_empty(), "the list is drained");
        let mut lone = vec![handed(step, "m/sounds/1/b.wav", 0.9, 12.0)];
        bb.sounds(1900, &mut lone, 2);
        let mut eight: Vec<Handed> = (0..8)
            .map(|_| handed(fire, "m/sounds/1/a.wav", 0.7, 0.0))
            .collect();
        bb.sounds(1930, &mut eight, 2);
        let mut snap = bb.snapshot("manual", None, None);
        snap.header.tick = 1930;
        assert_eq!(snap.sounds.len(), 9, "the 61 s old row is gone");
        assert_eq!(snap.names.takes, ["m/sounds/1/b.wav", "m/sounds/1/a.wav"]);
        let last = snap.sounds.last().expect("rows");
        assert_eq!(
            (last.tick, last.gain, last.pitch, last.dist, last.secs),
            (1930, 0.7, 1.0, 0.0, 2.0)
        );
        // The step (2 s long) still sounds, the 8 and 2 engine loops.
        assert_eq!(last.voices, 11);
        assert_eq!(snap.sounds[0].voices, 3);
        // Through the file, as the reader sees it.
        write_to(&dir.join("s.json"), &snap).expect("write");
        let snap = read(&dir.join("s.json")).expect("read");
        let d = sound_digest(&snap, Some(10.0));
        let over: Vec<&str> = d.lines().filter(|l| l.contains("OVERLOAD")).collect();
        assert_eq!(over.len(), 1, "{d}");
        assert!(
            over[0].contains("tick 1930: 8 starts, sum 5.60: cis_weap_inf_rocket_launcher_fire x8"),
            "{d}"
        );
        assert!(
            over[0].contains("nearest 0 m, one take at most x8 (sum 5.60)"),
            "{d}"
        );
        // The lone start is listed (by its hash: no name known), not as an overload.
        assert!(d.contains(&format!("{step:08x}  gain 0.90")), "{d}");
        assert!(d.contains("m/sounds/1/a.wav"), "{d}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn silence_expires_sound_rows_at_the_window_boundary() {
        let mut w = walk();
        let dir = scratch("sound-silence");
        let mut bb = BlackBox::start(true, meta(), &w, Vec::new(), dir.clone()).unwrap();
        bb.sound_names(
            HashMap::from([(1, "old".into()), (2, "recent".into())]),
            PathBuf::from("/baked"),
        );
        let handed = |hash, path: &str| Handed {
            hash,
            path: PathBuf::from("/baked").join(path),
            gain: 0.7,
            ears: (0.35, 0.35),
            pitch: 1.0,
            dist: 0.0,
            secs: 1.0,
        };
        bb.sounds(
            100,
            &mut vec![handed(1, "old.wav"), handed(1, "old.wav")],
            0,
        );
        bb.sounds(101, &mut vec![handed(2, "recent.wav")], 0);
        for (tick, rows) in [
            (100 + SOUND_TICKS - 1, 3),
            (100 + SOUND_TICKS, 1),
            (101 + SOUND_TICKS, 0),
        ] {
            w.sim.tick = tick;
            bb.tick(&w, None);
            bb.sounds(tick, &mut Vec::new(), 0);
            let snap = bb.snapshot("manual", Some(&w), None);
            assert_eq!(snap.sounds.len(), rows, "tick {tick}");
            if rows == 1 {
                assert_eq!(snap.names.takes, ["recent.wav"]);
                assert_eq!(snap.sounds[0].take, 0);
                let digest = sound_digest(&snap, None);
                assert!(digest.contains("recent"), "{digest}");
                assert!(!digest.contains("OVERLOAD"), "{digest}");
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Each write is whole (no temporary left behind), `last.json` is the
    /// newest and the five before it are kept in order.
    #[test]
    fn the_write_is_atomic_and_rotates_five() {
        let dir = scratch("rotate");
        let mut snap = Snapshot {
            schema: SCHEMA,
            ..Snapshot::default()
        };
        for k in 0..8u64 {
            snap.header.tick = k;
            write_rotating(&dir, &snap).expect("write");
            let names: Vec<String> = std::fs::read_dir(&dir)
                .expect("dir")
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .collect();
            assert!(names.iter().all(|n| n.ends_with(".json")), "{names:?}");
            assert_eq!(names.len(), (k as usize + 1).min(PREVIOUS + 1));
        }
        assert_eq!(read(&dir.join("last.json")).expect("last").header.tick, 7);
        for k in 1..=PREVIOUS {
            let p = read(&dir.join(format!("prev{k}.json"))).expect("prev");
            assert_eq!(p.header.tick, 7 - k as u64);
        }
        assert!(!dir.join("prev6.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The stuck alarm: 20 s of pushing the stick without getting 2 m away
    /// fires once; standing with the stick at rest does not.
    #[test]
    fn the_stuck_alarm_wants_a_pushed_stick() {
        let w = walk();
        let u = &w.sim.units[w.player.index as usize];
        let mut ring = Ring::new(meta(), Names::default(), Vec::new());
        let push = PadState {
            ly: 32767,
            ..PadState::default()
        };
        let fired = |ring: &mut Ring, pad: PadState, from: u64| {
            (0..u64::from(ALARM_TICKS) + 1)
                .filter(|k| ring.watch(u, pad, from + k))
                .count()
        };
        assert_eq!(fired(&mut ring, PadState::default(), 0), 0, "at rest");
        assert_eq!(fired(&mut ring, push, 1000), 1, "pushing");
        assert_eq!(fired(&mut ring, push, 2000), 0, "not twice in 2 min");
        assert_eq!(fired(&mut ring, push, 1000 + ALARM_EVERY + 700), 1);
    }
}
