//! The collision LEAK census v2 (card LEAK, issue #182): on every baked map,
//! drive a real soldier at every wall, across every cutter edge, over a
//! probe grid of the walkable floors, out of every vehicle and onto every
//! spawn node, and flag every run that ends somewhere a soldier must not be.
//!
//! ```text
//! cargo run --release -p game --example leak_census [map ...]
//! ```
//!
//! ## The passes
//! - `wall` — v1's drive. The WALL SAMPLES are the SOLDIER-layer faces the
//!   wall pass pushes against: every mesh triangle steeper than the walkable
//!   limit (`|n.y| < WALKABLE_MIN_NY`) and at least `MIN_AREA`, and the sides
//!   of every box and cylinder primitive, one per `CELL` m cell, floor band
//!   and facing octant. For each sample and each SIDE of it, the soldier
//!   stands on the floor `RUN_UP` m out from the face, facing it (a start
//!   that is no ordinary place to stand is skipped: see [`wall_start`]); then,
//!   per MODE × HEADING (0, ±35 deg off the face normal), `DRIVE` ticks of
//!   pad through `Sim::step` — the real stance machine, ground clamp, wall
//!   pass, doors and death regions — then the stick released until he has
//!   stood `SETTLE` ticks (at most `SETTLE_MAX`). The modes:
//!   - `walk`, `sprint`; `jump` / `sjump` (JUMP when grounded within
//!     `JUMP_AT` / `SJUMP_AT` m of the face); `spam` (sprint, JUMP every
//!     other tick) — v1's five;
//!   - `roll` / `sroll` — walk / sprint, ROLL_STANCE within `ROLL_AT` /
//!     `SROLL_AT` m (the roll exits to stand, `soldier/step.rs`);
//!   - `crouch` — crouch where he stands, crouch-walk to the face, stand up
//!     there with JUMP; `prone` — the same crawling;
//!   - `jet` — a jet class only: sprint-jump at `SJUMP_AT`, then JUMP again
//!     in the air (the jet), into the face;
//!   - `knock` — walk; within `KNOCK_AT` m a blast's knock of `KNOCK_SCALE`
//!     m/s toward the face and up (`Soldier::knock`, the game's own rule).
//!
//!   Every mode runs with the map's team-0 FILL class. Every other speed
//!   group of both sides' rosters and their heroes (max speed, roll speed,
//!   jump, jet — the capsule is one radius for every class,
//!   `core::unit::SOLDIER_RADIUS`, and its heights are the stance's) runs
//!   `sprint`, `sjump` and `sroll`, and `jet` when it has one. The droideka
//!   is not driven (its capsule is its own, its rolling is a bot rule).
//! - `grid` — the WALKABLE PROBE GRID. Every `GRID` m over the heightfield's
//!   square (on a map with none — Coruscant, the Death Star, the BF2
//!   interiors — the XZ box of its SOLDIER bodies), within `GRID_HUB_R` m
//!   of a nav hub, the floors stacked by down
//!   rays from above every SOLDIER body (and the uncut terrain), each with
//!   at least `GRID_HEADROOM` m to the floor over it. A standing soldier is
//!   planted on each and stepped `GRID_REST` idle ticks (`rest`). Where a
//!   SOLDIER face hangs within `GRID_COVER` m over the feet, every stance
//!   change is tried from there too: `crouch` (crouch, then stand),
//!   `prone` (prone, then stand) and `roll` in `GRID_ROLLS` headings.
//!   A plant the oracle already calls embedded (the floor of a sealed
//!   box, a deck's underside) is UNREACHABLE and not run; a plant off the
//!   level ([`core::sim::Sim::level_at`]) is not run either.
//! - `cutter` — every terrain-cutter EDGE (a cut point on the terrain,
//!   `GRID` m apart, with an uncut neighbour), one per `CUT_CELL` m cell:
//!   from `CUT_RUN` m before it, walk across it in `CUT_HEADINGS` headings
//!   (`CUT_DRIVE` ticks), then settle. The burial class.
//! - `exit` — every vehicle the map's pads field, turned to each of
//!   `EXIT_YAWS` headings in place: the player is seated in its first seat
//!   and gets out through the game's own `Sim::exit_vehicle` (its
//!   `exit_point`), judged where he lands and after `PLACE_SETTLE` ticks.
//! - `spawn` — every post's spawn node: the player stands on
//!   `Sim::spawn_ground` there, judged at once and after `PLACE_SETTLE` ticks.
//!
//! ## What a leak is
//! Every run ends in exactly one class, the first that holds. Four are
//! CROSSINGS — between two ticks a point of the soldier's axis above the
//! step (`AXIS_HEIGHTS`, clipped to the stance's capsule) crossed a
//! SOLDIER-layer face (`CollisionWorld::ray`): `push` (a wall face on a tick
//! the wall pass pushed at least `PUSH_THROUGH` m), `tunnel` (a wall face,
//! a smaller push), `ceiling` (a downward face), `floor` (an upward one).
//! `died` (a death region or a lethal landing) is not a leak. The END STATES
//! are the EMBED ORACLE's (`core::sim::Embed`, `Sim::embed_at`, shared with
//! `round_to_outcome SWEEP_EMBED`): `inprim`, `inclosed`, `inshell`,
//! `underterrain`, `overlap`, `offlevel` — each only when the run's start
//! was not already in it — then `offmap` (outside the heightfield's square
//! or below `NO_GROUND`) and `float` (not grounded after `SETTLE_MAX`).
//!
//! Census v3 (issue #182, the leak closure): a crossing is not the end of
//! the run. It is a leak only if the soldier's axis is still behind the face
//! [`CLIP_WINDOW`] ticks later; one he came back from, or whose edge he only
//! cut, is a `clip`. An open shell or off the level reached with no crossing
//! on the way is legal ground (`room`, `offnav`: a room, a canopy, a roof);
//! an overlap no deeper than [`GRAZE`] is a `graze`. `clip`, `room`,
//! `offnav` and `graze` print beside `ok` and `died` and make no site. The
//! drive sim has the round's kill floor.
//! The drive sim has no posts, so `offlevel` is judged on a second sim of
//! the same map built as the round sweep builds it (posts, pads, mission
//! AI), which also runs the `exit` and `spawn` passes.
//!
//! ## The output and the gate
//! STDOUT: per map, the run counts by class, by pass and by mode, then
//! `sites N  accepted A  new N`, then the footer
//!
//! ```text
//! leak: maps M  samples S  runs R  sites T  accepted A  new N
//! ```
//!
//! Leaking runs are grouped into SITES by class and end cell (`SITE_CELL` m,
//! 2 m floor band), listed with their first run (in pass, sample and mode
//! order) as the reproduction, the passes and modes that found them, and
//! the body (index and map instance) the crossing hit or the oracle named,
//! in the file `LEAK_SITES` names. A site is ACCEPTED when a line of
//! the ratchet covers it (`LEAK_ACCEPTED=<file>`, the gate's private copy;
//! else `leak_census_accepted.txt` beside this file) (`map class x z`,
//! the cell centre in whole metres; `map class *` or a `*` map or class
//! still parse, but the v2 baseline lists every site, a RATCHET: a new site
//! fails the gate, a fix deletes cells; a line left with no cells covers
//! nothing). The ratchet's cell has no floor band: a new site on another
//! storey of an accepted cell reads as accepted, and only the pinned
//! per-map counts catch it. `LEAK_WRITE_ACCEPTED=<file>` writes every
//! site of the run as such lines (the baseline generator).
//!
//! Deterministic: each map is cut into fixed TASKS (`WALL_CHUNKS` per wall
//! speed group, `GRID_CHUNKS` grid chunks, the cutter pass, the placement passes), each
//! run single-threaded on its own clone of the map's sims in a fixed order;
//! tasks run on worker threads and merge back in task order, so the report
//! does not depend on the thread count.
//!
//! Debug (all to STDERR): `LEAK_SAMPLE_LIMIT=<n>` caps the wall samples per
//! map; `LEAK_PASSES=wall,grid,cutter,exit,spawn` runs only those passes
//! (the gate runs all); `LEAK_VERBOSE=1` lists every leaking run, `=2`
//! every run; `LEAK_NO_RESCUE=1` turns the rescue off (issue #182 B4: an
//! embedded run is counted where it ends, not put back or killed);
//! `LEAK_NEAR=<x>,<z>,<r>` keeps the wall samples, grid points
//! and cutter edges within r m of (x, z); `LEAK_TRACE=<x>,<z>,<mode>` prints
//! every tick of the runs from that start; `LEAK_SLICE=<y>` prints the
//! SOLDIER mesh triangles the plane y cuts (`LEAK_SLICE_TRIS=1`: each
//! triangle too, world and model space); `LEAK_PROBE=<x>,<y>,<z>` prints
//! the ground, terrain, cut, oracle class and the faces over and under a
//! point (several: `;` between them); `LEAK_MODE=<mode>` builds that
//! mode's world layer and mission instead of Conquest's; `LEAK_HMAP=<x0>,<z0>,<x1>,<z1>,<step>`
//! prints the terrain and stacked floors over a grid; `LEAK_THREADS=<n>`.
//!
//! All of this is OURS (engineering, no RE): the thresholds are the
//! census's, and a verdict about what the PS2 does with the same wall goes
//! through a card (JWAL, WTUN, VCOL, LEAK2).
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::collision::{HullFilter, Layers, Shape, NO_GROUND, WALKABLE_MIN_NY};
use core::sim::{buttons, Embed, PadState, Sim};
use core::soldier::{Soldier, Stance};
use glam::{Quat, Vec2, Vec3};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// `writeln!` into a report `String`, which cannot fail.
macro_rules! outln {
    ($out:expr, $($arg:tt)*) => {{ let _ = writeln!($out, $($arg)*); }};
}

/// Wall-sample cell (m): one sample per cell, floor band and octant.
const CELL: f32 = 1.5;
/// A wall triangle smaller than this (m²) is not sampled.
const MIN_AREA: f32 = 0.05;
/// How far out from the face the soldier starts (m) — room to reach sprint.
const RUN_UP: f32 = 5.0;
/// The floor under a face's middle must be within this far below it (m).
const FLOOR_DROP: f32 = 2.5;
/// Stick ticks, then released ticks.
const DRIVE: u32 = 90;
/// The settle: grounded `SETTLE` ticks in a row, at most `SETTLE_MAX`.
const SETTLE: u32 = 10;
/// 420 since card AIRC: BF2's fall to `NO_GROUND` from Bespin's platforms.
const SETTLE_MAX: u32 = 420;
/// A wall-pass push at least this long (m) on the crossing tick: `push`.
const PUSH_THROUGH: f32 = 0.25;
/// Jump within this far of the face (m): walk / sprint.
const JUMP_AT: f32 = 1.5;
const SJUMP_AT: f32 = 2.5;
/// Roll within this far of the face (m): walk / sprint. The roll covers
/// ~2-3 m, so it ends at or under the face.
const ROLL_AT: f32 = 2.0;
const SROLL_AT: f32 = 3.0;
/// Crouch / prone: stand up this close to the face (m), or with this many
/// drive ticks left.
const STAND_AT: f32 = 0.7;
const STAND_LEFT: u32 = 15;
/// The knock: within `KNOCK_AT` m, `KNOCK_SCALE` m/s toward the face and
/// `KNOCK_UP` of it upward (a grenade's push; OURS).
const KNOCK_AT: f32 = 2.0;
const KNOCK_SCALE: f32 = 12.0;
const KNOCK_UP: f32 = 0.4;
/// Heading offsets from straight at the face (deg).
const HEADINGS: [f32; 3] = [0.0, 35.0, -35.0];
/// Points on the soldier's axis, above the feet (m), tested for crossings.
const AXIS_HEIGHTS: [f32; 3] = [0.8, 1.25, 1.7];
/// Site grouping cell (m).
const SITE_CELL: f32 = 4.0;
/// A CLIP (OURS, issue #182): a crossing whose axis point is back on the
/// side it came from (past the crossed face's plane by `CLIP_BACK` m)
/// within this many ticks is not a leak by itself. The wall pass or the
/// floor's push put him back (a lip's slope he walked a capsule into, a
/// rock's corner); a thin wall he went through, a deck he rose through or
/// a floor he sank under never takes him back. Nor is one whose axis is by
/// then no longer behind the face ([`behind_face`]: he cut its edge). The
/// run goes on, and is judged by where it ends.
const CLIP_WINDOW: u32 = 10;
/// A GRAZE (OURS, issue #182): an end in the oracle's `overlap` no deeper
/// than this (m, half the capsule's radius) leaves his axis at least
/// 0.25 m out of every face: his skin against a lip, a root or a wall the
/// wall pass holds him at. Deeper, half his body is in it: a leak.
const GRAZE: f32 = 0.25;
const CLIP_BACK: f32 = 0.01;
/// How far past the crossed face's plane a CLIP's look back still counts
/// the face (m): a bent face's other part, a seam.
const BEHIND_REACH: f32 = 0.5;
/// The probe grid: spacing (m), the reach from a nav hub (m), the headroom a
/// floor needs to the one over it (m), the idle ticks, the cover height
/// that earns the stance changes (m), the roll headings, the chunks.
const GRID: f32 = 1.5;
const GRID_HUB_R: f32 = 25.0;
const GRID_HEADROOM: f32 = 1.0;
const GRID_REST: u32 = 30;
const GRID_COVER: f32 = 2.4;
const GRID_ROLLS: usize = 4;
const GRID_LAYERS: usize = 8;
const GRID_CHUNKS: usize = 8;
/// Each wall speed group's samples are dealt to this many tasks (sample
/// index mod it), so the largest map's fill-class drive is not one thread.
const WALL_CHUNKS: usize = 4;
/// The most speed groups a map drives at the walls (the task list is made
/// before any map loads); a map with more says so on its first line.
const MAX_GROUPS: usize = 12;
/// Cutter edges: one per `CUT_CELL` m cell, approached from `CUT_RUN` m in
/// `CUT_HEADINGS` headings for `CUT_DRIVE` ticks.
const CUT_CELL: f32 = 3.0;
const CUT_RUN: f32 = 3.0;
const CUT_HEADINGS: usize = 8;
const CUT_DRIVE: u32 = 45;
/// The exit pass's headings, and the placement passes' settle (ticks).
const EXIT_YAWS: usize = 8;
const PLACE_SETTLE: u32 = 30;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Mode {
    Walk,
    Sprint,
    Jump,
    SJump,
    Spam,
    Roll,
    SRoll,
    Crouch,
    Prone,
    Jet,
    Knock,
    Rest,
    Cross,
    Exit,
    Spawn,
}
const WALL_MODES: [Mode; 11] = [
    Mode::Walk,
    Mode::Sprint,
    Mode::Jump,
    Mode::SJump,
    Mode::Spam,
    Mode::Roll,
    Mode::SRoll,
    Mode::Crouch,
    Mode::Prone,
    Mode::Jet,
    Mode::Knock,
];
/// The modes an extra speed group runs at the walls.
const GROUP_MODES: [Mode; 4] = [Mode::Sprint, Mode::SJump, Mode::SRoll, Mode::Jet];
const ALL_MODES: [Mode; 15] = [
    Mode::Walk,
    Mode::Sprint,
    Mode::Jump,
    Mode::SJump,
    Mode::Spam,
    Mode::Roll,
    Mode::SRoll,
    Mode::Crouch,
    Mode::Prone,
    Mode::Jet,
    Mode::Knock,
    Mode::Rest,
    Mode::Cross,
    Mode::Exit,
    Mode::Spawn,
];

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Walk => "walk",
            Mode::Sprint => "sprint",
            Mode::Jump => "jump",
            Mode::SJump => "sjump",
            Mode::Spam => "spam",
            Mode::Roll => "roll",
            Mode::SRoll => "sroll",
            Mode::Crouch => "crouch",
            Mode::Prone => "prone",
            Mode::Jet => "jet",
            Mode::Knock => "knock",
            Mode::Rest => "rest",
            Mode::Cross => "cross",
            Mode::Exit => "exit",
            Mode::Spawn => "spawn",
        }
    }
    fn sprint(self) -> bool {
        matches!(
            self,
            Mode::Sprint | Mode::SJump | Mode::Spam | Mode::SRoll | Mode::Jet
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Pass {
    Wall,
    Grid,
    Cutter,
    Exit,
    Spawn,
}
const PASSES: [Pass; 5] = [
    Pass::Wall,
    Pass::Grid,
    Pass::Cutter,
    Pass::Exit,
    Pass::Spawn,
];

impl Pass {
    fn name(self) -> &'static str {
        match self {
            Pass::Wall => "wall",
            Pass::Grid => "grid",
            Pass::Cutter => "cutter",
            Pass::Exit => "exit",
            Pass::Spawn => "spawn",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Class {
    Ok,
    Died,
    /// A crossing the soldier came back from within [`CLIP_WINDOW`] ticks
    /// and a run that then ended well: a clip, not a leak.
    Clip,
    /// The oracle's open shell at the end of a run whose axis crossed no
    /// face: he walked in through the shell's own opening (a room, a
    /// hangar, a tree's canopy, a ledge under an overhang). Not a leak.
    Room,
    /// Off the bots' level at the end of a run whose axis crossed no face:
    /// he walked, slid or fell somewhere legal the nav graph does not
    /// cover (a roof, a slope's foot). Not a leak (the bots' own recovery
    /// is `recover.rs`'s).
    OffNav,
    /// The oracle's overlap no deeper than [`GRAZE`]: his capsule's skin
    /// in a face his axis is well clear of. Not a leak.
    Graze,
    Push,
    Tunnel,
    Ceiling,
    Floor,
    InPrim,
    InClosed,
    InShell,
    Under,
    Overlap,
    OffLevel,
    OffMap,
    Float,
}
const NCLASS: usize = 18;
/// The classes that are leaks, in print order.
const LEAKS: [Class; 12] = [
    Class::Push,
    Class::Tunnel,
    Class::Ceiling,
    Class::Floor,
    Class::InPrim,
    Class::InClosed,
    Class::InShell,
    Class::Under,
    Class::Overlap,
    Class::OffLevel,
    Class::OffMap,
    Class::Float,
];

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Ok => "ok",
            Class::Died => "died",
            Class::Clip => "clip",
            Class::Room => "room",
            Class::OffNav => "offnav",
            Class::Graze => "graze",
            Class::Push => "push",
            Class::Tunnel => "tunnel",
            Class::Ceiling => "ceiling",
            Class::Floor => "floor",
            Class::InPrim => "inprim",
            Class::InClosed => "inclosed",
            Class::InShell => "inshell",
            Class::Under => "underterrain",
            Class::Overlap => "overlap",
            Class::OffLevel => "offlevel",
            Class::OffMap => "offmap",
            Class::Float => "float",
        }
    }
    /// A placement's class (an exit or a spawn puts him there; no axis
    /// crossed a face): an open shell is a room, off the level is off the
    /// nav, as for a run with no crossing.
    fn placed(self) -> Class {
        // (An exit's or a spawn's overlap still counts at any depth: it is
        // where the game puts him, B6.)
        match self {
            Class::InShell => Class::Room,
            Class::OffLevel => Class::OffNav,
            c => c,
        }
    }
    fn of_embed(e: Embed) -> Option<Class> {
        Some(match e {
            Embed::None => return None,
            Embed::InPrim(_) => Class::InPrim,
            Embed::InClosedMesh(_) => Class::InClosed,
            Embed::InOpenShell(_) => Class::InShell,
            Embed::UnderTerrain(_) => Class::Under,
            Embed::Overlap(..) => Class::Overlap,
            Embed::OffLevel => Class::OffLevel,
        })
    }
}

/// One wall sample: a point on a face and the face's horizontal normal.
#[derive(Clone, Copy)]
struct Sample {
    point: Vec3,
    normal: Vec2,
    y_lo: f32,
    y_hi: f32,
}

/// A soldier speed group: one class's constants standing for every class
/// that moves the same.
#[derive(Clone)]
struct Group {
    name: String,
    consts: core::soldier::SoldierConstants,
    jet: bool,
}

/// The first leaking run of a site: enough to replay it. `ord` orders runs
/// as one thread would meet them: (task, item, side, mode, heading).
#[derive(Clone)]
struct Site {
    ord: (usize, usize, u8, u8, u8),
    class: Class,
    cell: (i32, i32, i32),
    runs: u32,
    passes: u8,
    modes: u16,
    pass: Pass,
    group: String,
    start: Vec3,
    yaw: f32,
    mode: Mode,
    tick: u32,
    end: Vec3,
    body: Option<usize>,
}

/// One run's verdict.
struct Verdict {
    class: Class,
    tick: u32,
    end: Vec3,
    body: Option<usize>,
    /// `LEAK_VERBOSE`'s note on a crossing: how deep the tick's start was
    /// in a body, the stances, the move.
    why: String,
}

/// What a task reports.
#[derive(Default)]
struct Part {
    verbose: String,
    counts: [u32; NCLASS],
    pass_runs: [usize; 5],
    pass_leaks: [usize; 5],
    mode_leaks: [u32; 15],
    starts: usize,
    skipped: usize,
    /// Of `skipped`: starts the oracle reads as inside a primitive, a closed
    /// mesh or a body.
    skipped_embedded: usize,
    samples: usize,
    grid_points: usize,
    grid_unreachable: usize,
    grid_covered: usize,
    cutter_edges: usize,
    exit_vehicles: usize,
    exit_refused: usize,
    spawn_nodes: usize,
    spawn_noground: usize,
    sites: Vec<Site>,
}

/// A map's two sims and what the passes share, built once and cloned per task.
struct Proto {
    out: String,
    /// v1's drive sim: one player soldier, no posts, no vehicles.
    drive: Sim,
    pi: usize,
    template: core::unit::Unit,
    /// The round sweep's sim: posts, pads, mission AI (offlevel, exit, spawn).
    judge: Option<(Sim, usize)>,
    groups: Vec<Group>,
    samples: Vec<Sample>,
    /// Map instance labels, by `Body::instance`.
    instances: Vec<String>,
    /// Map body index -> instance.
    body_instance: Vec<Option<u32>>,
}

/// The SOLDIER-layer wall faces of the world, one per cell/band/octant.
fn wall_samples(sim: &Sim) -> Vec<Sample> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let mut add = |out: &mut Vec<Sample>, s: Sample| {
        let oct = ((core::fmath::atan2(s.normal.y, s.normal.x) + std::f32::consts::PI)
            / (std::f32::consts::FRAC_PI_4)) as i32
            & 7;
        let key = (
            (s.point.x / CELL).floor() as i32,
            (s.point.z / CELL).floor() as i32,
            (s.point.y / 2.0).floor() as i32,
            oct,
        );
        if seen.insert(key) {
            out.push(s);
        }
    };
    for b in sim.collision.bodies() {
        if !b.layers.contains(Layers::SOLDIER) {
            continue;
        }
        let m = b.to_world;
        match &b.shape {
            Shape::Mesh(mesh) => {
                let vs = mesh.vertices();
                for t in mesh.indices() {
                    let p = |i: u32| {
                        let v = vs[i as usize];
                        m.transform_point3(Vec3::new(v.x, v.y, v.z))
                    };
                    let (a, bb, c) = (p(t[0]), p(t[1]), p(t[2]));
                    let n = (bb - a).cross(c - a);
                    let area2 = n.length();
                    if area2 * 0.5 < MIN_AREA {
                        continue;
                    }
                    let n = n / area2;
                    if n.y.abs() >= WALKABLE_MIN_NY {
                        continue;
                    }
                    let h = Vec2::new(n.x, n.z).normalize();
                    add(
                        &mut out,
                        Sample {
                            point: (a + bb + c) / 3.0,
                            normal: h,
                            y_lo: a.y.min(bb.y).min(c.y),
                            y_hi: a.y.max(bb.y).max(c.y),
                        },
                    );
                }
            }
            Shape::Box { half } => {
                for (ax, sgn) in [(0usize, 1.0f32), (0, -1.0), (2, 1.0), (2, -1.0)] {
                    let mut local = Vec3::ZERO;
                    local[ax] = half[ax] * sgn;
                    let mut ln = Vec3::ZERO;
                    ln[ax] = sgn;
                    let wn = m.transform_vector3(ln);
                    if wn.y.abs() >= WALKABLE_MIN_NY {
                        continue;
                    }
                    let c = m.transform_point3(local);
                    add(
                        &mut out,
                        Sample {
                            point: c,
                            normal: Vec2::new(wn.x, wn.z).normalize(),
                            y_lo: c.y - half.y,
                            y_hi: c.y + half.y,
                        },
                    );
                }
            }
            Shape::Cylinder {
                radius,
                half_height,
            } => {
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    let (s, c) = core::fmath::sin_cos(a);
                    let wn = m.transform_vector3(Vec3::new(c, 0.0, s));
                    if wn.y.abs() >= WALKABLE_MIN_NY {
                        continue;
                    }
                    let p = m.transform_point3(Vec3::new(c * radius, 0.0, s * radius));
                    add(
                        &mut out,
                        Sample {
                            point: p,
                            normal: Vec2::new(wn.x, wn.z).normalize(),
                            y_lo: p.y - half_height,
                            y_hi: p.y + half_height,
                        },
                    );
                }
            }
            Shape::Sphere { .. } => {}
        }
    }
    out
}

/// The debug slice (`LEAK_SLICE`): each SOLDIER mesh triangle the plane
/// `y` cuts, as a segment.
/// `LEAK_PROBE=<x>,<y>,<z>`: what the drive sim says of a point: the ground
/// under it, the terrain there and whether it is cut, the oracle's class,
/// and the first SOLDIER hit of a ray down and one up.
fn probe(proto: &Proto, q: Vec3) {
    let sim = &proto.drive;
    let terrain = sim.heightfield.as_ref().map(|h| h.height_at(q.x, q.z));
    let hit = |d: Vec3| {
        sim.collision
            .ray_with(q, d, 60.0, Layers::SOLDIER, HullFilter::Static)
            .map(|h| {
                format!(
                    "t {:.2} n ({:.2},{:.2},{:.2}) {}",
                    h.t,
                    h.normal.x,
                    h.normal.y,
                    h.normal.z,
                    body_label(proto, h.instance.map(|i| i as usize | INSTANCE_TAG))
                )
            })
    };
    eprintln!(
        "probe ({:.2},{:.2},{:.2}): ground {:.2}  terrain {terrain:?}  cut {}  embed {:?}  down {:?}  up {:?}",
        q.x,
        q.y,
        q.z,
        sim.ground_under(q, false),
        sim.collision.is_cut(q),
        sim.embed_at(q, Stance::Stand),
        hit(-Vec3::Y),
        hit(Vec3::Y),
    );
}

/// `LEAK_HMAP=<x0>,<z0>,<x1>,<z1>,<step>`: per grid point, the terrain
/// height and the floors stacked under `top` (`column_floors`), one row per z.
fn hmap(proto: &Proto, x0: f32, z0: f32, x1: f32, z1: f32, step: f32) {
    let sim = &proto.drive;
    let mut z = z0;
    while z <= z1 + 1e-3 {
        let mut row = format!("hmap z {z:.1}:");
        let mut x = x0;
        while x <= x1 + 1e-3 {
            let t = sim.heightfield.as_ref().map(|h| h.height_at(x, z));
            let fl = column_floors(sim, x, z, 400.0);
            let _ = write!(
                row,
                "  x {x:.1} t {} f [{}]",
                t.map_or("-".to_owned(), |t| format!("{t:.1}")),
                fl.iter()
                    .map(|f| format!("{f:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            x += step;
        }
        eprintln!("{row}");
        z += step;
    }
}

fn slice(proto: &Proto, y: f32, near: Option<(f32, f32, f32)>, out: &mut String) {
    let sim = &proto.drive;
    let tris = std::env::var("LEAK_SLICE_TRIS").is_ok_and(|v| v == "1");
    for (bi, b) in sim.collision.bodies().iter().enumerate() {
        if !b.layers.contains(Layers::SOLDIER) {
            continue;
        }
        let Shape::Mesh(mesh) = &b.shape else {
            continue;
        };
        let vs = mesh.vertices();
        for t in mesh.indices() {
            let p: Vec<Vec3> = t
                .iter()
                .map(|&i| {
                    let v = vs[i as usize];
                    b.to_world.transform_point3(Vec3::new(v.x, v.y, v.z))
                })
                .collect();
            let mut cut = Vec::new();
            for k in 0..3 {
                let (a, c) = (p[k], p[(k + 1) % 3]);
                if (a.y - y) * (c.y - y) < 0.0 {
                    cut.push(a + (c - a) * ((y - a.y) / (c.y - a.y)));
                }
            }
            if cut.len() == 2 {
                if near.is_some_and(|(x, z, r)| Vec2::new(cut[0].x - x, cut[0].z - z).length() > r)
                {
                    continue;
                }
                outln!(
                    out,
                    "slice {} {} {} {} {}",
                    body_label(proto, Some(bi)),
                    cut[0].x,
                    cut[0].z,
                    cut[1].x,
                    cut[1].z
                );
                // `LEAK_SLICE_TRIS=1`: the cut triangle, world and model
                // space (what `game::collision_fixes` names).
                if tris {
                    let local: Vec<String> = t
                        .iter()
                        .map(|&i| {
                            let v = vs[i as usize];
                            format!("[{}, {}, {}]", v.x, v.y, v.z)
                        })
                        .collect();
                    outln!(
                        out,
                        "  tri world {:?} local {}",
                        p.iter().map(|v| v.to_array()).collect::<Vec<_>>(),
                        local.join(" ")
                    );
                }
            }
        }
    }
}

/// Does the STEPPED capsule touch any SOLDIER body deeper than 1 cm,
/// whatever the contact's normal? (card WPSH, v1's start filter.)
fn in_contact(sim: &Sim, feet: Vec3) -> bool {
    let r = core::unit::SOLDIER_RADIUS;
    let top = core::unit::capsule_height(Stance::Stand);
    let (lo, hi) = (core::collision::STEP_HEIGHT + r, top - r);
    let mut out = Vec::new();
    [lo, (lo + hi) * 0.5, hi].iter().any(|h| {
        out.clear();
        sim.collision
            .sphere_contacts(feet + Vec3::Y * *h, r, Layers::SOLDIER, &mut out);
        out.iter().any(|(depth, _)| *depth > 0.01)
    })
}

/// The first SOLDIER-layer face the soldier's axis crossed between `a` and
/// `b` (feet): its normal, its map instance, the hit point and the axis
/// height that met it.
fn crossing(sim: &Sim, a: Vec3, b: Vec3, height: f32) -> Option<(Vec3, Option<u32>, Vec3, f32)> {
    let d = b - a;
    let len = d.length();
    if len < 1e-5 {
        return None;
    }
    let dir = d / len;
    for h in AXIS_HEIGHTS {
        if h > height - 0.15 {
            continue;
        }
        let o = a + Vec3::Y * h;
        if let Some(hit) = sim
            .collision
            .ray_with(o, dir, len, Layers::SOLDIER, HullFilter::All)
        {
            return Some((hit.normal, hit.instance, o + dir * hit.t, h));
        }
    }
    None
}

/// Is the axis point `a`, past a crossed face's plane, still BEHIND that
/// face: does a ray from it back toward the side he came from (`n`) meet
/// the same map instance by the face's plane (through `p`), give or take
/// [`BEHIND_REACH`] m? A thin wall he went
/// through, a deck he fell through or rose through does; the rim of a deck
/// he cut falling past it, or a rock's corner, does not.
fn behind_face(sim: &Sim, a: Vec3, p: Vec3, n: Vec3, body: Option<usize>) -> bool {
    let reach = (p - a).dot(n).max(0.0) + BEHIND_REACH;
    sim.collision
        .ray_with(a, n, reach, Layers::SOLDIER, HullFilter::All)
        .is_some_and(|hit| hit.instance.map(|i| i as usize | INSTANCE_TAG) == body)
}

/// The heightfield's square, (min x, min z, max x, max z).
fn map_square(sim: &Sim) -> Option<(f32, f32, f32, f32)> {
    let h = sim.heightfield.as_ref()?;
    let ext = (h.size - 1) as f32 * h.unit;
    Some((
        -h.offset_x,
        -h.offset_z,
        -h.offset_x + ext,
        -h.offset_z + ext,
    ))
}

/// The XZ box of the SOLDIER-layer bodies, (min x, min z, max x, max z).
fn body_square(sim: &Sim) -> Option<(f32, f32, f32, f32)> {
    sim.collision
        .bodies()
        .iter()
        .filter(|b| b.layers.contains(Layers::SOLDIER))
        .fold(None, |acc: Option<(f32, f32, f32, f32)>, b| {
            let (lo, hi) = (b.aabb.min, b.aabb.max);
            Some(match acc {
                None => (lo.x, lo.z, hi.x, hi.z),
                Some((a, c, d, e)) => (a.min(lo.x), c.min(lo.z), d.max(hi.x), e.max(hi.z)),
            })
        })
}

/// `yaw` facing the XZ direction `d` (yaw 0 = −Z, `Soldier::step`'s axes).
fn yaw_of(d: Vec2) -> f32 {
    core::fmath::atan2(-d.x, -d.y)
}

fn dir_of(a: f32) -> Vec2 {
    let (s, c) = core::fmath::sin_cos(a);
    Vec2::new(c, s)
}

/// `LEAK_MODE=<mode>` (default `con`): the mode whose world layer and
/// mission the census builds (`hunt`, `1flag`, `eli`, `ass`: the layers
/// other modes merge add their own instances).
fn census_mode() -> String {
    std::env::var("LEAK_MODE")
        .ok()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "con".to_owned())
}

fn mission_for(key: &str) -> Option<game::mission::MissionDef> {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let path = dir.join(format!("{key}_{}.ron", census_mode()));
    game::mission::MissionDef::load(&path).ok()
}

fn near_filter() -> Option<(f32, f32, f32)> {
    let f: Vec<f32> = std::env::var("LEAK_NEAR")
        .ok()?
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
    (f.len() == 3).then(|| (f[0], f[1], f[2]))
}

fn near_ok(near: Option<(f32, f32, f32)>, p: Vec3) -> bool {
    near.is_none_or(|(x, z, r)| Vec2::new(p.x - x, p.z - z).length() <= r)
}

fn passes_on() -> [bool; 5] {
    match std::env::var("LEAK_PASSES") {
        Ok(v) => PASSES.map(|p| v.split(',').any(|x| x == p.name())),
        Err(_) => [true; 5],
    }
}

/// Load one map: the drive sim (v1's), the judge sim (the round sweep's),
/// the speed groups and the wall samples.
fn build_proto(key: &str, path: &Path, limit: Option<usize>) -> Result<Proto, String> {
    let mut out = String::new();
    let (scene, _) = game::assets::load_battle_scene(path, &census_mode())
        .map_err(|e| format!("{key:10} LOAD FAILED {e:#}"))?;
    let mission = mission_for(key);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));
    let factions = mission
        .as_ref()
        .map(game::mission::MissionDef::factions)
        .unwrap_or_else(|| {
            game::roster::default_factions(&scene.manifest.source_game).map(str::to_owned)
        });
    let classes_dir = path
        .parent()
        .map(|p| p.join("classes"))
        .unwrap_or_else(|| std::path::PathBuf::from("classes"));
    let rosters = game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    );
    // The speed groups: the fill class first, then every other class of
    // both sides and their heroes that moves differently.
    let mut groups: Vec<Group> = Vec::new();
    let mut team = game::walk::TeamTemplate::default_trooper();
    match &rosters {
        Ok((rs, _)) => {
            let f = rs[0].fill_class();
            team.consts = f.consts;
            team.name = f.class.clone();
            let mut rs = rs.clone();
            for (t, r) in rs.iter_mut().enumerate() {
                game::heroes::append(r, &classes_dir, mission.as_ref(), t);
            }
            for c in std::iter::once(f).chain(rs.iter().flat_map(|r| r.classes.iter())) {
                if c.droideka.is_some() {
                    continue;
                }
                let k = &c.consts;
                let jet = k.has_jet();
                let same = |g: &Group| {
                    g.consts.max_speed == k.max_speed
                        && g.consts.roll_speed_factor == k.roll_speed_factor
                        && g.consts.jump_vy == k.jump_vy
                        && g.jet == jet
                };
                if !groups.iter().any(same) {
                    groups.push(Group {
                        name: c.class.clone(),
                        consts: c.consts,
                        jet,
                    });
                }
            }
        }
        Err(e) => {
            outln!(out, "{key:10} roster unavailable: {e:#} (built-in trooper)");
            groups.push(Group {
                name: team.name.clone(),
                consts: team.consts,
                jet: team.consts.has_jet(),
            });
        }
    }
    if groups.len() > MAX_GROUPS {
        outln!(
            out,
            "{key:10} {} speed groups: the walls drive the first {MAX_GROUPS}",
            groups.len()
        );
        groups.truncate(MAX_GROUPS);
    }
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut sim = Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    let mut w = game::walk::WalkSim::with_teams(
        sim,
        scene.heightfield(),
        Vec3::ZERO,
        [team.clone(), team],
        nav,
    );
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
        w.sim.rescue_off = no_rescue();
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.landing_regions = a.landing_regions.clone();
    }
    let pi = w.player.index as usize;
    let drive = w.sim;
    let template = drive.units[pi].clone();
    let instances: Vec<String> = scene
        .manifest
        .instances
        .iter()
        .map(|i| format!("{} {}", i.class, i.name))
        .collect();
    let body_instance = drive
        .collision
        .bodies()
        .iter()
        .map(|b| b.instance)
        .collect();
    let mut samples = wall_samples(&drive);
    let near = near_filter();
    samples.retain(|s| near_ok(near, s.point));
    if let Some(n) = limit {
        samples.truncate(n);
    }
    // The judge: play_census's battle, with the player and no bot.
    let judge = match &rosters {
        Ok((rs, _)) => build_judge(
            key,
            &scene,
            mission.as_ref(),
            &layout,
            applied.as_ref(),
            rs,
            &classes_dir,
            &mut out,
        ),
        Err(_) => None,
    };
    // The round's kill floor (15 m under the lowest post's floor, card
    // KFLR), which only a sim with posts derives: a run that falls out of a
    // space hangar or off a deck's edge dies there as in a round, instead
    // of falling to the floor of the world and reading `offmap`.
    let mut drive = drive;
    drive.kill_floor = judge.as_ref().and_then(|(j, _)| j.kill_floor);
    Ok(Proto {
        out,
        drive,
        pi,
        template,
        judge,
        groups,
        samples,
        instances,
        body_instance,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_judge(
    key: &str,
    scene: &game::assets::Scene,
    mission: Option<&game::mission::MissionDef>,
    layout: &game::layout::MapLayout,
    applied: Option<&game::layout::MissionApplied>,
    rosters: &[game::roster::FactionRoster; 2],
    classes_dir: &Path,
    out: &mut String,
) -> Option<(Sim, usize)> {
    let mut sim = Sim::empty(42);
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.item_pool = game::roster::item_pool(&scene.manifest.source_game, &scene.manifest.map);
    sim.battle = core::battle::Battle::new([100_000; 2]);
    sim.team_combatants = Some([1; 2]);
    let mut tables: [Vec<core::unit::UnitClass>; 2] = [Vec::new(), Vec::new()];
    let mut names: Vec<(String, usize)> = Vec::new();
    let mut t = 0usize;
    let teams = rosters.each_ref().map(|r| {
        for c in &r.classes {
            tables[t].push(c.register(&mut sim, &mut names));
        }
        t += 1;
        game::walk::TeamTemplate::from_roster(
            r,
            &tables[t - 1][0],
            r.classes
                .iter()
                .map(game::walk::ClassVisual::from_roster)
                .collect(),
        )
    });
    let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    if let Some(m) = mission {
        m.apply_arc_blocks(nav.as_mut());
        m.apply_barriers(nav.as_mut());
    }
    let mut w = game::walk::WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, teams, nav);
    w.sim.classes = tables;
    scene.install_collision(&mut w.sim);
    if let Some(a) = applied {
        w.sim.death_regions = a.death_regions.clone();
        w.sim.rescue_off = no_rescue();
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.landing_regions = a.landing_regions.clone();
        w.sim.ground_flyer_map = mission.is_some_and(|m| m.ground_flyer_map);
    }
    w.add_posts(layout);
    if let Some(m) = mission {
        m.apply_ai(&mut w.sim);
    }
    let factions = [w.teams[0].faction.clone(), w.teams[1].faction.clone()];
    if let Err(e) = game::vehicle_pads::install(
        &mut w.sim,
        &scene.manifest,
        &scene.dir,
        classes_dir,
        &factions,
    ) {
        outln!(out, "{key:10} pads unavailable: {e:#}");
    }
    w.opening_wave([0, 0], true);
    w.sim.cheats.invincible = true;
    let pi = w.player.index as usize;
    Some((w.sim, pi))
}

/// The pad state machine of one run: what to press this tick.
struct Driver {
    mode: Mode,
    /// The face's point and outward normal (wall pass); `None`: a free run
    /// along the start yaw.
    face: Option<(Vec3, Vec2)>,
    drive: u32,
    rolled: bool,
    knocked: bool,
    jumped: bool,
    low: bool,
    stood: bool,
}

impl Driver {
    fn new(mode: Mode, face: Option<(Vec3, Vec2)>, drive: u32) -> Driver {
        Driver {
            mode,
            face,
            drive,
            rolled: false,
            knocked: false,
            jumped: false,
            low: false,
            stood: false,
        }
    }

    /// The pad for tick `t`, and a knock to apply before it (dir, scale).
    fn pad(&mut self, t: u32, so: &Soldier) -> (PadState, Option<(Vec3, f32)>) {
        let dt = 1.0 / core::TICK_HZ as f32;
        let mut pad = PadState::default();
        if t >= self.drive {
            return (pad, None);
        }
        // Distance to the face along its normal; a free run triggers by time.
        let dist = self
            .face
            .map(|(p, n)| Vec2::new(so.pos.x - p.x, so.pos.z - p.z).dot(n));
        let near = |at: f32, tick: u32| dist.map_or(t >= tick, |d| d < at);
        let edge = |b: u32| so.prev_buttons() & b == 0;
        let mut knock = None;
        pad.ly = i16::MAX;
        if self.mode.sprint() {
            pad.buttons |= buttons::SPRINT;
        }
        match self.mode {
            Mode::Walk | Mode::Sprint | Mode::Cross | Mode::Exit | Mode::Spawn => {}
            Mode::Rest => pad.ly = 0,
            Mode::Jump | Mode::SJump => {
                let at = if self.mode == Mode::Jump {
                    JUMP_AT
                } else {
                    SJUMP_AT
                };
                if so.grounded && near(at, 20) && edge(buttons::JUMP) {
                    pad.buttons |= buttons::JUMP;
                }
            }
            Mode::Spam => {
                if t.is_multiple_of(2) && edge(buttons::JUMP) {
                    pad.buttons |= buttons::JUMP;
                }
            }
            Mode::Roll | Mode::SRoll => {
                let at = if self.mode == Mode::Roll {
                    ROLL_AT
                } else {
                    SROLL_AT
                };
                if !self.rolled && so.grounded && near(at, 3) && edge(buttons::ROLL_STANCE) {
                    pad.buttons |= buttons::ROLL_STANCE;
                    self.rolled = so.stance != Stance::Roll;
                }
                if so.stance == Stance::Roll {
                    self.rolled = true;
                }
            }
            Mode::Crouch | Mode::Prone => {
                let want = if self.mode == Mode::Crouch {
                    Stance::Crouch
                } else {
                    Stance::Prone
                };
                if !self.low {
                    // Down first, standing still: the stationary taps.
                    pad.ly = 0;
                    pad.buttons &= !buttons::SPRINT;
                    if so.stance == want {
                        self.low = true;
                    } else if so
                        .stance_tap_after(dt)
                        .is_some_and(|s| s == Stance::Crouch || s == want)
                    {
                        pad.buttons |= buttons::ROLL_STANCE;
                    }
                } else if !self.stood {
                    let stop = dist.map_or(t + STAND_LEFT >= self.drive, |d| d < STAND_AT)
                        || t + STAND_LEFT >= self.drive;
                    if stop {
                        pad.ly = 0;
                        if so.jump_stands_after(dt) {
                            pad.buttons |= buttons::JUMP;
                        }
                        if matches!(so.stance, Stance::Stand) {
                            self.stood = true;
                        }
                    }
                } else {
                    pad.ly = 0;
                }
            }
            Mode::Jet => {
                if !self.jumped {
                    if so.grounded && near(SJUMP_AT, 15) && edge(buttons::JUMP) {
                        pad.buttons |= buttons::JUMP;
                        self.jumped = true;
                    }
                } else if !so.grounded
                    && so.stance == Stance::Jump
                    && edge(buttons::JUMP)
                    && !self.rolled
                {
                    pad.buttons |= buttons::JUMP;
                    self.rolled = true; // the jet's one press
                }
            }
            Mode::Knock => {
                if !self.knocked && so.grounded && near(KNOCK_AT, 20) {
                    let toward = self.face.map_or_else(
                        || {
                            let (s, c) = core::fmath::sin_cos(so.yaw);
                            Vec3::new(-s, 0.0, -c)
                        },
                        |(_, n)| Vec3::new(-n.x, 0.0, -n.y),
                    );
                    knock = Some(((toward + Vec3::Y * KNOCK_UP).normalize(), KNOCK_SCALE));
                    self.knocked = true;
                }
            }
        }
        (pad, knock)
    }
}

/// The census's oracle at `end` in `stance`: the drive sim's (its nav graph
/// says what is lived in), with `offlevel` from the judge's posts.
fn oracle(drive: &Sim, judge: Option<&Sim>, end: Vec3, stance: Stance) -> Embed {
    match drive.embed_at(end, stance) {
        Embed::None => match judge {
            Some(j) if j.level_at(end).is_off() => Embed::OffLevel,
            _ => Embed::None,
        },
        e => e,
    }
}

/// Drive one run on `sim` from the unit already placed: `driver`'s pads,
/// then the settle. `start_embed` is the start's own class (a run that ends
/// in it is no leak).
#[allow(clippy::too_many_arguments)]
fn run_one(
    sim: &mut Sim,
    judge: Option<&Sim>,
    pi: usize,
    driver: &mut Driver,
    square: Option<(f32, f32, f32, f32)>,
    start_embed: Option<Class>,
    tracing: bool,
    trace_out: &mut String,
) -> Verdict {
    let mut class = Class::Ok;
    let mut at_tick = 0u32;
    let mut body = None;
    let mut still = 0u32;
    let mut t = 0u32;
    let rules = sim.ground_rules;
    // A crossing not yet judged a leak or a clip: its class, tick, end,
    // body, the face's point and its normal toward the start's side, and
    // the axis height.
    let mut pending: Option<(Class, u32, Vec3, Option<usize>, Vec3, Vec3, f32)> = None;
    let mut why = String::new();
    let verbose = std::env::var("LEAK_VERBOSE").is_ok_and(|v| !v.is_empty() && v != "0");
    let mut clipped = false;
    while t < driver.drive + SETTLE_MAX && still < SETTLE {
        let before = sim.units[pi].soldier.pos;
        let stance0 = sim.units[pi].soldier.stance;
        let depth0 = if verbose {
            let c = sim.units[pi].capsule();
            sim.collision
                .capsule_overlap(&c.stepped())
                .map_or(0.0, |(d, _)| d)
        } else {
            0.0
        };
        let (pad, knock) = driver.pad(t, &sim.units[pi].soldier);
        if let Some((dir, scale)) = knock {
            sim.units[pi].soldier.knock(dir, scale, rules, 0.0);
        }
        sim.step(&[pad]);
        let u = &sim.units[pi];
        if !u.alive {
            class = Class::Died;
            at_tick = t;
            break;
        }
        // The tick he came back on crosses the face again, outward: that
        // crossing is the clip's own, not a new one.
        let mut came_back = false;
        if let Some((_, ct, _, cbody, p, n, h)) = pending {
            if (u.soldier.pos + Vec3::Y * h - p).dot(n) > CLIP_BACK {
                clipped = true;
                came_back = true;
                pending = None;
            } else if t >= ct + CLIP_WINDOW {
                if behind_face(sim, u.soldier.pos + Vec3::Y * h, p, n, cbody) {
                    break;
                }
                // Past the face's plane but out from behind the face: he cut
                // its edge or corner in one tick (a fall past a deck's rim).
                clipped = true;
                pending = None;
            }
        }
        let h = core::unit::capsule_height(u.soldier.stance);
        // A rescue's put-back (issue #182 B4) is a teleport, not a move
        // through the world: no crossing on its tick.
        let rescued = sim.events.iter().any(
            |e| matches!(e, core::sim::Event::Rescued { unit, .. } if unit.index as usize == pi),
        );
        let cross = if rescued {
            None
        } else {
            crossing(sim, before, u.soldier.pos, h)
        };
        if tracing {
            outln!(
                trace_out,
                "trace t {t} pos ({:.3},{:.3},{:.3}) vel ({:.2},{:.2},{:.2}) {:?} grounded {} push ({:.3},{:.3}) cross {:?} embed {:?}",
                u.soldier.pos.x, u.soldier.pos.y, u.soldier.pos.z,
                u.soldier.vel.x, u.soldier.vel.y, u.soldier.vel.z,
                u.soldier.stance, u.soldier.grounded, u.wall_push.x, u.wall_push.z, cross,
                sim.embed_at(u.soldier.pos, u.soldier.stance)
            );
        }
        if let (Some((n, inst, p, h)), None, false) = (cross, pending, came_back) {
            let c = if n.y.abs() < WALKABLE_MIN_NY {
                if u.wall_push.length() >= PUSH_THROUGH {
                    Class::Push
                } else {
                    Class::Tunnel
                }
            } else if n.y < 0.0 {
                Class::Ceiling
            } else {
                Class::Floor
            };
            if verbose {
                let d = u.soldier.pos - before;
                why = format!(
                    "pre_depth {:.2} stance {:?}->{:?} dy {:.2} dxz {:.2} h {h} n ({:.2},{:.2},{:.2}) grounded {}",
                    depth0,
                    stance0,
                    u.soldier.stance,
                    d.y,
                    d.with_y(0.0).length(),
                    n.x,
                    n.y,
                    n.z,
                    u.soldier.grounded
                );
            }
            // The face's normal turned toward where the axis came from.
            let from_side = before + Vec3::Y * h - p;
            let n = if n.dot(from_side) < 0.0 { -n } else { n };
            pending = Some((
                c,
                t,
                u.soldier.pos,
                inst.map(|i| i as usize | INSTANCE_TAG),
                p,
                n,
                h,
            ));
        }
        if t >= driver.drive {
            still = if u.soldier.grounded { still + 1 } else { 0 };
        }
        t += 1;
    }
    // A crossing he did not come back from (confirmed in the loop, or
    // still open when the run died or settled, and still behind its face)
    // is the run's leak, at the crossing's tick and end.
    if let Some((_, _, _, cbody, p, n, h)) = pending {
        let u = &sim.units[pi];
        if !behind_face(sim, u.soldier.pos + Vec3::Y * h, p, n, cbody) {
            pending = None;
            clipped = true;
        }
    }
    if let Some((c, ct, cend, cbody, ..)) = pending {
        return Verdict {
            class: c,
            tick: ct,
            end: cend,
            body: cbody,
            why,
        };
    }
    let u = &sim.units[pi];
    let end = u.soldier.pos;
    if class == Class::Ok {
        at_tick = t;
        let off = square
            .is_some_and(|(x0, z0, x1, z1)| end.x < x0 || end.z < z0 || end.x > x1 || end.z > z1)
            || end.y < NO_GROUND + 1.0;
        let e = oracle(sim, judge, end, u.soldier.stance);
        if let Some(c) = Class::of_embed(e).filter(|c| Some(*c) != start_embed) {
            // A shell or off the level with no crossing on the way: legal
            // ground the oracle cannot tell from a leak (the A4 sweep's
            // false positives). After a clip it still counts.
            class = match (c, e) {
                (Class::InShell, _) if !clipped => Class::Room,
                (Class::OffLevel, _) if !clipped => Class::OffNav,
                (Class::Overlap, Embed::Overlap(d, _)) if d <= GRAZE => Class::Graze,
                (c, _) => c,
            };
            body = e.body();
            if let (Embed::Overlap(d, _), true) = (e, verbose) {
                why = format!(
                    "depth {d:.3} stance {:?} grounded {}",
                    u.soldier.stance, u.soldier.grounded
                );
            }
        } else if off {
            class = Class::OffMap;
        } else if !u.soldier.grounded {
            class = Class::Float;
        } else if clipped {
            class = Class::Clip;
        }
    }
    Verdict {
        class,
        tick: at_tick,
        end,
        body,
        why,
    }
}

/// `LEAK_NO_RESCUE=1`: the drive runs with no rescue (issue #182 B4,
/// `core::sim::Sim::rescue_off`), so the census reads the raw geometry: a
/// run that ends inside a solid is counted where it ends, not put back.
fn no_rescue() -> bool {
    std::env::var("LEAK_NO_RESCUE").is_ok_and(|v| v == "1")
}

/// A body reference that is a map INSTANCE (a crossing's ray hit), not a
/// body index (the oracle's).
const INSTANCE_TAG: usize = 1 << 40;

/// Place the template soldier, in speed group `g`'s constants, at `start`.
fn place(sim: &mut Sim, pi: usize, template: &core::unit::Unit, g: &Group, start: Vec3, yaw: f32) {
    let mut u = template.clone();
    u.consts = g.consts;
    u.soldier = Soldier::new(start);
    u.soldier.yaw = yaw;
    u.soldier.fuel = Soldier::spawn_fuel(&g.consts);
    u.soldier.energy = g.consts.energy_bar;
    sim.units[pi] = u;
}

struct Recorder<'a> {
    part: &'a mut Part,
    key: &'a str,
    verbose_on: bool,
    verbose_all: bool,
}

impl Recorder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        ord: (usize, usize, u8, u8, u8),
        pass: Pass,
        group: &str,
        mode: Mode,
        start: Vec3,
        yaw: f32,
        v: &Verdict,
    ) {
        let p = &mut *self.part;
        p.counts[v.class as usize] += 1;
        p.pass_runs[pass as usize] += 1;
        let leak = LEAKS.contains(&v.class);
        if self.verbose_all || (self.verbose_on && leak) {
            outln!(
                p.verbose,
                "{} {} {} {} {} start ({:.2},{:.2},{:.2}) yaw {:.4} tick {} end ({:.2},{:.2},{:.2}) body {:?} {}",
                self.key, v.class.name(), pass.name(), mode.name(), group,
                start.x, start.y, start.z, yaw, v.tick, v.end.x, v.end.y, v.end.z, v.body, v.why
            );
        }
        if !leak {
            return;
        }
        p.pass_leaks[pass as usize] += 1;
        p.mode_leaks[mode as usize] += 1;
        let cell = (
            (v.end.x / SITE_CELL).floor() as i32,
            (v.end.z / SITE_CELL).floor() as i32,
            (v.end.y / 2.0).floor() as i32,
        );
        let site = Site {
            ord,
            class: v.class,
            cell,
            runs: 1,
            passes: 1 << pass as u8,
            modes: 1 << mode as u16,
            pass,
            group: group.to_owned(),
            start,
            yaw,
            mode,
            tick: v.tick,
            end: v.end,
            body: v.body,
        };
        merge_site(&mut p.sites, site);
    }
}

fn merge_site(sites: &mut Vec<Site>, x: Site) {
    match sites
        .iter_mut()
        .find(|y| y.class == x.class && y.cell == x.cell)
    {
        Some(y) => {
            y.runs += x.runs;
            y.passes |= x.passes;
            y.modes |= x.modes;
            if x.ord < y.ord {
                let (runs, passes, modes) = (y.runs, y.passes, y.modes);
                *y = x;
                y.runs = runs;
                y.passes = passes;
                y.modes = modes;
            }
        }
        None => sites.push(x),
    }
}

/// v1's start filter for a wall sample's side: the start, or why not.
fn wall_start(sim: &Sim, s: &Sample, n: Vec2) -> Option<Vec3> {
    let near = s.point + Vec3::new(n.x, 0.0, n.y) * 0.8;
    let floor = sim.ground_under(near, false);
    if floor < s.point.y - FLOOR_DROP || floor > s.point.y + 0.1 {
        return None;
    }
    if s.y_hi < floor + 0.8 || s.y_lo > floor + 1.8 {
        return None;
    }
    let from = Vec3::new(
        s.point.x + n.x * RUN_UP,
        floor + 0.3,
        s.point.z + n.y * RUN_UP,
    );
    let gy = sim.ground_under(from, false);
    let start = Vec3::new(from.x, gy, from.z);
    if (gy - floor).abs() > 1.0
        || sim.death_regions.iter().any(|r| r.contains(start))
        || in_contact(sim, start)
    {
        return None;
    }
    let probe = core::collision::Capsule {
        base: start,
        height: core::unit::capsule_height(Stance::Stand),
        radius: core::unit::SOLDIER_RADIUS,
    };
    (sim.collision.push_out(&probe.stepped()) == Vec3::ZERO).then_some(start)
}

fn trace_target() -> Option<(String, String, String)> {
    std::env::var("LEAK_TRACE").ok().and_then(|v| {
        let f: Vec<&str> = v.split(',').collect();
        (f.len() == 3).then(|| (f[0].to_owned(), f[1].to_owned(), f[2].to_owned()))
    })
}

fn is_traced(t: &Option<(String, String, String)>, start: Vec3, mode: Mode) -> bool {
    t.as_ref().is_some_and(|(x, z, m)| {
        *x == format!("{:.2}", start.x) && *z == format!("{:.2}", start.z) && m == mode.name()
    })
}

/// The tasks of one map, in report order.
#[derive(Clone, Copy, Debug)]
enum Task {
    /// The wall pass for speed group `g` (0 = the fill class, every mode),
    /// chunk `c` of `WALL_CHUNKS`.
    Wall(usize, usize),
    /// Grid chunk `c` of `GRID_CHUNKS`.
    Grid(usize),
    Cutter,
    /// The exit and spawn passes, on the judge.
    Place,
}

fn tasks_for(groups: usize, on: [bool; 5]) -> Vec<Task> {
    let mut t = Vec::new();
    if on[0] {
        t.extend((0..groups).flat_map(|g| (0..WALL_CHUNKS).map(move |c| Task::Wall(g, c))));
    }
    if on[1] {
        t.extend((0..GRID_CHUNKS).map(Task::Grid));
    }
    if on[2] {
        t.push(Task::Cutter);
    }
    if on[3] || on[4] {
        t.push(Task::Place);
    }
    t
}

fn run_task(key: &str, proto: &Proto, ti: usize, task: Task, on: [bool; 5]) -> Part {
    let mut part = Part::default();
    let verbose_all = std::env::var("LEAK_VERBOSE").as_deref() == Ok("2");
    let verbose_on = !verbose_all && std::env::var_os("LEAK_VERBOSE").is_some();
    let mut verbose = String::new();
    {
        let mut rec = Recorder {
            part: &mut part,
            key,
            verbose_on,
            verbose_all,
        };
        match task {
            Task::Wall(g, c) => wall_task(proto, ti, g, c, &mut rec, &mut verbose),
            Task::Grid(c) => grid_task(proto, ti, c, &mut rec, &mut verbose),
            Task::Cutter => cutter_task(proto, ti, &mut rec, &mut verbose),
            Task::Place => place_task(proto, ti, on, &mut rec),
        }
    }
    part.verbose.push_str(&verbose);
    part
}

fn wall_task(
    proto: &Proto,
    ti: usize,
    gi: usize,
    chunk: usize,
    rec: &mut Recorder,
    trace_out: &mut String,
) {
    let mut sim = proto.drive.clone();
    let judge = proto.judge.as_ref().map(|(s, _)| s);
    let pi = proto.pi;
    let g = &proto.groups[gi];
    let square = map_square(&sim);
    let trace = trace_target();
    if gi == 0 && chunk == 0 {
        rec.part.samples = proto.samples.len();
    }
    let modes: Vec<Mode> = if gi == 0 {
        WALL_MODES
            .iter()
            .copied()
            .filter(|m| *m != Mode::Jet || g.jet)
            .collect()
    } else {
        GROUP_MODES
            .iter()
            .copied()
            .filter(|m| *m != Mode::Jet || g.jet)
            .collect()
    };
    for (si, s) in proto.samples.iter().enumerate() {
        if si % WALL_CHUNKS != chunk {
            continue;
        }
        for (sd, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let n = s.normal * side;
            let Some(start) = wall_start(&sim, s, n) else {
                if gi == 0 {
                    rec.part.skipped += 1;
                }
                continue;
            };
            let start_embed = Class::of_embed(oracle(&sim, judge, start, Stance::Stand));
            if matches!(
                start_embed,
                Some(Class::InPrim | Class::InClosed | Class::Overlap)
            ) {
                // v1's rule: a start inside a primitive, enclosed or in a
                // body is no place to stand. A start the oracle reads as
                // `inshell`, `under` or `offlevel` (a hangar, a ledge, a
                // roof) is driven: a run that ends in the start's own class
                // is no leak (`run_one`).
                if gi == 0 {
                    rec.part.skipped += 1;
                    rec.part.skipped_embedded += 1;
                }
                continue;
            }
            if gi == 0 {
                rec.part.starts += 1;
            }
            for &mode in &modes {
                for (hi, hd) in HEADINGS.into_iter().enumerate() {
                    let (sa, ca) = core::fmath::sin_cos(hd.to_radians());
                    let toward = -n;
                    let d = Vec2::new(toward.x * ca - toward.y * sa, toward.x * sa + toward.y * ca);
                    let yaw = yaw_of(d);
                    place(&mut sim, pi, &proto.template, g, start, yaw);
                    let mut drv = Driver::new(mode, Some((s.point, n)), DRIVE);
                    let tracing = is_traced(&trace, start, mode);
                    let v = run_one(
                        &mut sim,
                        judge,
                        pi,
                        &mut drv,
                        square,
                        start_embed,
                        tracing,
                        trace_out,
                    );
                    rec.record(
                        (ti, si, sd as u8, mode as u8, hi as u8),
                        Pass::Wall,
                        &g.name,
                        mode,
                        start,
                        yaw,
                        &v,
                    );
                }
            }
        }
    }
}

/// The probe grid's floors at column (x, z), highest first.
fn column_floors(sim: &Sim, x: f32, z: f32, top: f32) -> Vec<f32> {
    let mut floors = Vec::new();
    let mut y = top;
    let terrain = sim
        .heightfield
        .as_ref()
        .map(|h| h.height_at(x, z))
        .filter(|t| !sim.collision.is_cut(Vec3::new(x, *t - 0.5, z)));
    while floors.len() < GRID_LAYERS {
        let bottom = terrain.unwrap_or(NO_GROUND).max(NO_GROUND);
        if y <= bottom {
            break;
        }
        let Some((fy, _)) = sim.collision.ray_down(Vec3::new(x, y, z), y - bottom) else {
            break;
        };
        floors.push(fy);
        y = fy - 0.05;
    }
    if let Some(t) = terrain {
        if floors.iter().all(|f| (f - t).abs() > 0.6) && floors.len() < GRID_LAYERS {
            floors.push(t);
            floors.sort_by(|a, b| b.total_cmp(a));
        }
    }
    floors
}

fn grid_task(proto: &Proto, ti: usize, chunk: usize, rec: &mut Recorder, trace_out: &mut String) {
    let mut sim = proto.drive.clone();
    let judge = proto.judge.as_ref().map(|(s, _)| s);
    let pi = proto.pi;
    let g = &proto.groups[0];
    // The heightfield's square, or on a map with none (Coruscant, the Death
    // Star, the BF2 interiors) the XZ box of its SOLDIER bodies.
    let square = map_square(&sim);
    let Some((x0, z0, x1, z1)) = square.or_else(|| body_square(&sim)) else {
        return;
    };
    let trace = trace_target();
    let near = near_filter();
    let hubs: Vec<Vec2> = sim
        .nav
        .as_ref()
        .map(|g| g.hubs.iter().map(|h| Vec2::new(h.pos.x, h.pos.z)).collect())
        .unwrap_or_default();
    // A coarse occupancy of the hub reach: cells of GRID_HUB_R.
    let cw = GRID_HUB_R;
    let mut reach = std::collections::BTreeSet::new();
    for h in &hubs {
        let (cx, cz) = ((h.x / cw).floor() as i32, (h.y / cw).floor() as i32);
        for dx in -1..=1 {
            for dz in -1..=1 {
                reach.insert((cx + dx, cz + dz));
            }
        }
    }
    let top = sim
        .collision
        .bodies()
        .iter()
        .filter(|b| b.layers.contains(Layers::SOLDIER))
        .map(|b| b.aabb.max.y)
        .fold(
            sim.heightfield
                .as_ref()
                .map_or(0.0, |h| h.heights.iter().copied().fold(f32::MIN, f32::max)),
            f32::max,
        )
        + 2.0;
    let nx = ((x1 - x0) / GRID).floor() as i32;
    let nz = ((z1 - z0) / GRID).floor() as i32;
    let mut col = 0usize;
    for iz in 0..=nz {
        for ix in 0..=nx {
            let x = x0 + ix as f32 * GRID + GRID * 0.5;
            let z = z0 + iz as f32 * GRID + GRID * 0.5;
            if x > x1 || z > z1 {
                continue;
            }
            let p = Vec2::new(x, z);
            if !reach.contains(&((x / cw).floor() as i32, (z / cw).floor() as i32))
                || !hubs
                    .iter()
                    .any(|h| h.distance_squared(p) <= GRID_HUB_R * GRID_HUB_R)
                || !near_ok(near, Vec3::new(x, 0.0, z))
            {
                continue;
            }
            col += 1;
            if col % GRID_CHUNKS != chunk {
                continue;
            }
            let floors = column_floors(&sim, x, z, top);
            for (li, &f) in floors.iter().enumerate() {
                // Headroom to the floor over this one.
                if li > 0 && floors[li - 1] - f < GRID_HEADROOM {
                    continue;
                }
                let start = Vec3::new(x, f, z);
                rec.part.grid_points += 1;
                if sim.death_regions.iter().any(|r| r.contains(start))
                    || Class::of_embed(oracle(&sim, judge, start, Stance::Stand)).is_some()
                {
                    rec.part.grid_unreachable += 1;
                    continue;
                }
                let covered = sim
                    .collision
                    .ray(
                        start + Vec3::Y * 0.6,
                        Vec3::Y,
                        GRID_COVER - 0.6,
                        Layers::SOLDIER,
                    )
                    .is_some();
                let mut runs: Vec<(Mode, f32)> = vec![(Mode::Rest, 0.0)];
                if covered {
                    rec.part.grid_covered += 1;
                    runs.push((Mode::Crouch, 0.0));
                    runs.push((Mode::Prone, 0.0));
                    for k in 0..GRID_ROLLS {
                        runs.push((
                            Mode::Roll,
                            k as f32 * std::f32::consts::TAU / GRID_ROLLS as f32,
                        ));
                    }
                }
                for (ri, (mode, a)) in runs.into_iter().enumerate() {
                    let yaw = yaw_of(dir_of(a));
                    place(&mut sim, pi, &proto.template, g, start, yaw);
                    let drive = match mode {
                        Mode::Rest => GRID_REST,
                        Mode::Roll => 25,
                        _ => 40,
                    };
                    let mut drv = Driver::new(mode, None, drive);
                    if mode != Mode::Roll {
                        // Crouch and prone stand up where they are.
                        drv.face = None;
                    }
                    let tracing = is_traced(&trace, start, mode);
                    let v = run_grid(&mut sim, judge, pi, &mut drv, square, tracing, trace_out);
                    rec.record(
                        (ti, col, li as u8, mode as u8, ri as u8),
                        Pass::Grid,
                        &g.name,
                        mode,
                        start,
                        yaw,
                        &v,
                    );
                }
            }
        }
    }
}

/// A grid run: crouch and prone stay put (no stick), the roll moves.
fn run_grid(
    sim: &mut Sim,
    judge: Option<&Sim>,
    pi: usize,
    drv: &mut Driver,
    square: Option<(f32, f32, f32, f32)>,
    tracing: bool,
    trace_out: &mut String,
) -> Verdict {
    if matches!(drv.mode, Mode::Crouch | Mode::Prone) {
        // Down at once, then up with STAND_LEFT ticks left: no stick.
        drv.face = Some((Vec3::splat(f32::MAX), Vec2::X));
    }
    run_one(sim, judge, pi, drv, square, None, tracing, trace_out)
}

/// The terrain-cutter edge points: a cut point on the terrain with an uncut
/// neighbour `GRID` m away, one per `CUT_CELL` cell.
fn cutter_edges(sim: &Sim) -> Vec<(Vec3, Vec2)> {
    let Some(hf) = sim.heightfield.as_ref() else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let cut_at = |x: f32, z: f32| {
        sim.collision
            .is_cut(Vec3::new(x, hf.height_at(x, z) - 0.5, z))
    };
    for c in &sim.collision.cutters {
        // The cutter's bound: its sphere, or skip a cutter with none (the
        // planes alone give no extent to walk).
        let r = c.sphere[0];
        if r <= 0.0 || c.planes.is_empty() {
            continue;
        }
        let (cx, cz) = (c.sphere[1], c.sphere[3]);
        let n = (r / GRID).ceil() as i32;
        for iz in -n..=n {
            for ix in -n..=n {
                let (x, z) = (cx + ix as f32 * GRID, cz + iz as f32 * GRID);
                if !cut_at(x, z) {
                    continue;
                }
                // The outward direction: to the first uncut neighbour.
                let out_dir = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
                    .into_iter()
                    .find(|(dx, dz)| !cut_at(x + dx * GRID, z + dz * GRID));
                let Some((dx, dz)) = out_dir else {
                    continue;
                };
                let key = ((x / CUT_CELL).floor() as i32, (z / CUT_CELL).floor() as i32);
                if seen.insert(key) {
                    out.push((Vec3::new(x, hf.height_at(x, z), z), Vec2::new(dx, dz)));
                }
            }
        }
    }
    out
}

fn cutter_task(proto: &Proto, ti: usize, rec: &mut Recorder, trace_out: &mut String) {
    let mut sim = proto.drive.clone();
    let judge = proto.judge.as_ref().map(|(s, _)| s);
    let pi = proto.pi;
    let g = &proto.groups[0];
    let square = map_square(&sim);
    let trace = trace_target();
    let near = near_filter();
    let edges: Vec<_> = cutter_edges(&sim)
        .into_iter()
        .filter(|(p, _)| near_ok(near, *p))
        .collect();
    rec.part.cutter_edges = edges.len();
    for (ei, (p, _out)) in edges.iter().enumerate() {
        for k in 0..CUT_HEADINGS {
            let d = dir_of(k as f32 * std::f32::consts::TAU / CUT_HEADINGS as f32);
            let from = Vec3::new(p.x - d.x * CUT_RUN, p.y + 3.0, p.z - d.y * CUT_RUN);
            let gy = sim.ground_under(from, false);
            if gy <= NO_GROUND + 1.0 {
                continue;
            }
            let start = Vec3::new(from.x, gy, from.z);
            let start_embed = Class::of_embed(oracle(&sim, judge, start, Stance::Stand));
            if start_embed.is_some()
                || sim.death_regions.iter().any(|r| r.contains(start))
                || in_contact(&sim, start)
            {
                continue;
            }
            let yaw = yaw_of(d);
            place(&mut sim, pi, &proto.template, g, start, yaw);
            let mut drv = Driver::new(Mode::Cross, None, CUT_DRIVE);
            let tracing = is_traced(&trace, start, Mode::Cross);
            let v = run_one(
                &mut sim, judge, pi, &mut drv, square, None, tracing, trace_out,
            );
            rec.record(
                (ti, ei, 0, Mode::Cross as u8, k as u8),
                Pass::Cutter,
                &g.name,
                Mode::Cross,
                start,
                yaw,
                &v,
            );
        }
    }
}

/// The exit and spawn passes, on a clone of the judge.
fn place_task(proto: &Proto, ti: usize, on: [bool; 5], rec: &mut Recorder) {
    let Some((judge0, pi)) = proto.judge.as_ref() else {
        return;
    };
    let pi = *pi;
    let square = map_square(judge0);
    let me = judge0.handle(pi);
    let template = judge0.units[pi].clone();
    let settle = |s: &mut Sim| {
        for _ in 0..PLACE_SETTLE {
            s.step(&[PadState::default()]);
        }
    };
    // Judge a placement now and after the settle: the first class found.
    let judge_at = |s: &mut Sim| -> Verdict {
        let u = &s.units[pi];
        let p0 = u.soldier.pos;
        let e0 = s.embed_at(p0, u.soldier.stance);
        if let Some(c) = Class::of_embed(e0) {
            return Verdict {
                class: c.placed(),
                tick: 0,
                end: p0,
                body: e0.body(),
                why: String::new(),
            };
        }
        settle(s);
        let u = &s.units[pi];
        let end = u.soldier.pos;
        if !u.alive {
            return Verdict {
                class: Class::Died,
                tick: PLACE_SETTLE,
                end,
                body: None,
                why: String::new(),
            };
        }
        let e1 = s.embed_state(me);
        let off = square
            .is_some_and(|(x0, z0, x1, z1)| end.x < x0 || end.z < z0 || end.x > x1 || end.z > z1)
            || end.y < NO_GROUND + 1.0;
        let class = Class::of_embed(e1).map(Class::placed).unwrap_or(if off {
            Class::OffMap
        } else {
            Class::Ok
        });
        Verdict {
            class,
            tick: PLACE_SETTLE,
            end,
            body: e1.body(),
            why: String::new(),
        }
    };
    if on[3] {
        let mut base = judge0.clone();
        // The pads' first vehicles come on their first ticks.
        for _ in 0..2 {
            base.step(&[PadState::default()]);
        }
        let vehicles: Vec<_> = base
            .alive_vehicles()
            .map(|(h, v)| (h, v.pos, v.rot, v.seats.len()))
            .collect();
        rec.part.exit_vehicles = vehicles.len();
        for (vi, (vh, pos, rot, seats)) in vehicles.into_iter().enumerate() {
            if seats == 0 {
                rec.part.exit_refused += 1;
                continue;
            }
            for k in 0..EXIT_YAWS {
                let mut s = base.clone();
                let turn =
                    Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / EXIT_YAWS as f32);
                if let Some(v) = s.vehicle_mut(vh) {
                    v.rot = turn * rot;
                    v.pos = pos;
                }
                s.units[pi] = template.clone();
                s.units[pi].soldier.pos = pos + Vec3::Y * 2.0;
                if !s.seat_unit(me, vh, 0) {
                    rec.part.exit_refused += 1;
                    continue;
                }
                s.exit_vehicle(me);
                let start = s.units[pi].soldier.pos;
                let yaw = s.units[pi].soldier.yaw;
                let v = judge_at(&mut s);
                rec.record(
                    (ti, vi, 0, Mode::Exit as u8, k as u8),
                    Pass::Exit,
                    "pilot",
                    Mode::Exit,
                    start,
                    yaw,
                    &v,
                );
            }
        }
    }
    if on[4] {
        let mut s = judge0.clone();
        let nodes: Vec<Vec3> = s
            .cps
            .iter()
            .flat_map(|c| c.def.spawn_nodes.iter().map(|(p, _)| *p))
            .collect();
        rec.part.spawn_nodes = nodes.len();
        for (ni, p) in nodes.into_iter().enumerate() {
            let g = s.spawn_ground(p);
            if g <= NO_GROUND {
                rec.part.spawn_noground += 1;
                continue;
            }
            let start = Vec3::new(p.x, g, p.z);
            s.units[pi] = template.clone();
            s.units[pi].soldier = Soldier::new(start);
            s.units[pi].soldier.fuel = Soldier::spawn_fuel(&template.consts);
            let v = judge_at(&mut s);
            rec.record(
                (ti, ni, 0, Mode::Spawn as u8, 0),
                Pass::Spawn,
                "spawn",
                Mode::Spawn,
                start,
                0.0,
                &v,
            );
        }
    }
}

struct MapReport {
    text: String,
    site_text: String,
    accepted_text: String,
    accepted_sites: Vec<(Class, i32, i32)>,
    verbose: String,
    samples: usize,
    runs: usize,
    sites: usize,
    accepted: usize,
    new: usize,
}

fn body_label(proto: &Proto, body: Option<usize>) -> String {
    match body {
        None => "-".to_owned(),
        Some(b) if b & INSTANCE_TAG != 0 => {
            let i = b & !INSTANCE_TAG;
            format!(
                "inst {i} {}",
                proto.instances.get(i).map_or("?", |s| s.as_str())
            )
        }
        Some(b) => match proto.body_instance.get(b).copied().flatten() {
            Some(i) => format!(
                "body {b} inst {i} {}",
                proto.instances.get(i as usize).map_or("?", |s| s.as_str())
            ),
            None => format!("body {b}"),
        },
    }
}

/// The tasks of one map, in task order, as one report.
fn merge_map(key: &str, proto: &Proto, accepted: &[Accepted], parts: Vec<Part>) -> MapReport {
    let mut rep = MapReport {
        text: proto.out.clone(),
        site_text: String::new(),
        accepted_text: String::new(),
        accepted_sites: Vec::new(),
        verbose: String::new(),
        samples: 0,
        runs: 0,
        sites: 0,
        accepted: 0,
        new: 0,
    };
    let mut all = Part::default();
    let mut sites: Vec<Site> = Vec::new();
    for p in parts {
        rep.verbose.push_str(&p.verbose);
        for (a, b) in all.counts.iter_mut().zip(p.counts) {
            *a += b;
        }
        for k in 0..5 {
            all.pass_runs[k] += p.pass_runs[k];
            all.pass_leaks[k] += p.pass_leaks[k];
        }
        for (a, b) in all.mode_leaks.iter_mut().zip(p.mode_leaks) {
            *a += b;
        }
        all.samples += p.samples;
        all.starts += p.starts;
        all.skipped += p.skipped;
        all.skipped_embedded += p.skipped_embedded;
        all.grid_points += p.grid_points;
        all.grid_unreachable += p.grid_unreachable;
        all.grid_covered += p.grid_covered;
        all.cutter_edges += p.cutter_edges;
        all.exit_vehicles += p.exit_vehicles;
        all.exit_refused += p.exit_refused;
        all.spawn_nodes += p.spawn_nodes;
        all.spawn_noground += p.spawn_noground;
        for x in p.sites {
            merge_site(&mut sites, x);
        }
    }
    rep.samples = all.samples;
    rep.runs = all.pass_runs.iter().sum();
    rep.sites = sites.len();
    let out = &mut rep.text;
    outln!(
        out,
        "{key:10} runs {} | ok {} died {} clip {} room {} offnav {} graze {} | {} | sites {}",
        rep.runs,
        all.counts[Class::Ok as usize],
        all.counts[Class::Died as usize],
        all.counts[Class::Clip as usize],
        all.counts[Class::Room as usize],
        all.counts[Class::OffNav as usize],
        all.counts[Class::Graze as usize],
        LEAKS
            .iter()
            .map(|c| format!("{} {}", c.name(), all.counts[*c as usize]))
            .collect::<Vec<_>>()
            .join(" "),
        sites.len()
    );
    outln!(
        out,
        "{key:10} wall samples {} starts {} skipped {} (embedded {}) groups {} runs {} leaks {} | grid points {} unreachable {} covered {} runs {} leaks {} | cutter edges {} runs {} leaks {} | exit vehicles {} refused {} runs {} leaks {} | spawn nodes {} noground {} runs {} leaks {}",
        all.samples, all.starts, all.skipped, all.skipped_embedded, proto.groups.len(), all.pass_runs[0], all.pass_leaks[0],
        all.grid_points, all.grid_unreachable, all.grid_covered, all.pass_runs[1], all.pass_leaks[1],
        all.cutter_edges, all.pass_runs[2], all.pass_leaks[2],
        all.exit_vehicles, all.exit_refused, all.pass_runs[3], all.pass_leaks[3],
        all.spawn_nodes, all.spawn_noground, all.pass_runs[4], all.pass_leaks[4],
    );
    outln!(
        out,
        "{key:10} leaks by mode: {}",
        ALL_MODES
            .iter()
            .map(|m| format!("{} {}", m.name(), all.mode_leaks[*m as usize]))
            .collect::<Vec<_>>()
            .join(" ")
    );
    sites.sort_by_key(|x| (x.class, x.cell));
    for x in &sites {
        let cx = ((x.cell.0 as f32 + 0.5) * SITE_CELL) as i32;
        let cz = ((x.cell.1 as f32 + 0.5) * SITE_CELL) as i32;
        let reason = accepted
            .iter()
            .find(|a| a.covers(key, x.class.name(), cx, cz));
        match reason {
            Some(_) => rep.accepted += 1,
            None => rep.new += 1,
        }
        let passes: Vec<&str> = PASSES
            .iter()
            .filter(|p| x.passes & (1 << **p as u8) != 0)
            .map(|p| p.name())
            .collect();
        let modes: Vec<&str> = ALL_MODES
            .iter()
            .filter(|m| x.modes & (1 << **m as u16) != 0)
            .map(|m| m.name())
            .collect();
        outln!(
            rep.site_text,
            "{key} {} {cx} {cz}  {}  runs {}  passes {}  modes {}  first: {} {} {} start ({:.2},{:.2},{:.2}) yaw {:.4} tick {} end ({:.2},{:.2},{:.2}) {}  | {}",
            x.class.name(),
            if reason.is_some() { "accepted" } else { "NEW" },
            x.runs,
            passes.join(","),
            modes.join(","),
            x.pass.name(),
            x.mode.name(),
            x.group,
            x.start.x, x.start.y, x.start.z,
            x.yaw,
            x.tick,
            x.end.x, x.end.y, x.end.z,
            body_label(proto, x.body),
            reason.map_or("", |a| a.reason.as_str()),
        );
        rep.accepted_sites.push((x.class, cx, cz));
    }
    outln!(
        rep.text,
        "{key:10} sites {}  accepted {}  new {}",
        sites.len(),
        rep.accepted,
        rep.new
    );
    // The ratchet lines (`LEAK_WRITE_ACCEPTED`): per class, the sites'
    // cells, `ACCEPT_PER_LINE` to a line.
    for c in LEAKS {
        let cells: Vec<String> = rep
            .accepted_sites
            .iter()
            .filter(|(k, _, _)| *k == c)
            .map(|(_, x, z)| format!("{x} {z}"))
            .collect();
        for (i, chunk) in cells.chunks(ACCEPT_PER_LINE).enumerate() {
            outln!(
                rep.accepted_text,
                "{key} {} {}  # v2 baseline {}/{}",
                c.name(),
                chunk.join("  "),
                i + 1,
                cells.len().div_ceil(ACCEPT_PER_LINE)
            );
        }
    }
    rep
}

/// Sites per line of the written ratchet.
const ACCEPT_PER_LINE: usize = 16;

/// One line of `leak_census_accepted.txt`: `<map|*> <class|*> <x> <z> [<x>
/// <z> ...]  # <reason>` — the listed sites (each its cell centre, as the
/// sites file prints it; any number of `x z` pairs on one line), or, with a
/// literal `*` in place of the pairs, every site of that map's class. A
/// line with no pairs and no `*` covers NOTHING (a ratchet line whose last
/// cell a fix deleted must not turn into a wildcard), and an odd or
/// unparseable pair is an error: both are reported on stderr and counted
/// in the footer (`bad-accept-lines N`), which fails the byte-pinned gate.
struct Accepted {
    map: String,
    class: String,
    /// `None`: every site of the map's class (`*`).
    at: Option<std::collections::BTreeSet<(i32, i32)>>,
    reason: String,
}

impl Accepted {
    fn covers(&self, map: &str, class: &str, x: i32, z: i32) -> bool {
        (self.map == "*" || self.map == map)
            && (self.class == "*" || self.class == class)
            && self.at.as_ref().is_none_or(|at| at.contains(&(x, z)))
    }
}

/// Parse the accepted file's text: the lines, and how many were bad.
fn parse_accepted(text: &str) -> (Vec<Accepted>, usize) {
    let mut out = Vec::new();
    let mut bad = 0usize;
    for (ln, line) in text.lines().enumerate() {
        let (spec, reason) = line.split_once('#').unwrap_or((line, ""));
        let f: Vec<&str> = spec.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        let complain = |why: &str| {
            eprintln!(
                "leak_census_accepted.txt:{}: {why}, line ignored: {}",
                ln + 1,
                line.trim()
            );
        };
        if f.len() < 3 {
            complain("no `x z` pairs and no `*`");
            bad += 1;
            continue;
        }
        let at = if f[2..] == ["*"] {
            None
        } else {
            let nums: Option<Vec<i32>> = f[2..].iter().map(|v| v.parse().ok()).collect();
            match nums {
                Some(n) if n.len() % 2 == 0 => {
                    Some(n.chunks_exact(2).map(|p| (p[0], p[1])).collect())
                }
                Some(_) => {
                    complain("an odd number of coordinates");
                    bad += 1;
                    continue;
                }
                None => {
                    complain("a coordinate that is not a whole number");
                    bad += 1;
                    continue;
                }
            }
        };
        out.push(Accepted {
            map: f[0].to_owned(),
            class: f[1].to_owned(),
            at,
            reason: reason.trim().to_owned(),
        });
    }
    (out, bad)
}

/// The ratchet: `LEAK_ACCEPTED=<file>` when set (the gate passes the private
/// copy beside its baselines, `<baseline_dir>/leak_census_accepted.txt`:
/// one line per site class and map is half a megabyte, too much for the
/// public tree), else `leak_census_accepted.txt` beside this file, which in
/// the public tree holds only its header.
fn accepted_list() -> (Vec<Accepted>, usize) {
    let p = std::env::var_os("LEAK_ACCEPTED")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/leak_census_accepted.txt"
            ))
            .to_path_buf()
        });
    parse_accepted(&std::fs::read_to_string(p).unwrap_or_default())
}

/// A map's slot in the run: its proto, loaded by the first task that needs
/// it and dropped when its last task is done.
struct Slot {
    proto: OnceLock<Option<Arc<Proto>>>,
    held: Mutex<Option<Arc<Proto>>>,
    left: AtomicUsize,
}

fn main() {
    let want: Vec<String> = std::env::args().skip(1).collect();
    let dir = game::assets::baked_root();
    let maps: Vec<(String, String, std::path::PathBuf)> = game::assets::discover_maps(&dir)
        .into_iter()
        .filter(|(k, _, _)| want.is_empty() || want.iter().any(|w| w == k))
        .collect();
    let limit = std::env::var("LEAK_SAMPLE_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok());
    let (accepted, bad_accept) = accepted_list();
    let on = passes_on();
    // Each map's tasks need its group count, which needs its load: the wall
    // tasks are cut per group up to MAX_GROUPS, and a map with fewer has
    // empty ones (so the task list is known before any load).
    let per_map = tasks_for(MAX_GROUPS, on);
    let tasks: Vec<(usize, usize)> = (0..maps.len())
        .flat_map(|m| (0..per_map.len()).map(move |t| (m, t)))
        .collect();
    let threads = std::env::var("LEAK_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .clamp(1, tasks.len().max(1));
    let slots: Vec<Slot> = maps
        .iter()
        .map(|_| Slot {
            proto: OnceLock::new(),
            held: Mutex::new(None),
            left: AtomicUsize::new(per_map.len()),
        })
        .collect();
    let next = AtomicUsize::new(0);
    let parts: Mutex<Vec<Option<Part>>> = Mutex::new((0..tasks.len()).map(|_| None).collect());
    let reports: Mutex<Vec<Option<MapReport>>> =
        Mutex::new((0..maps.len()).map(|_| None).collect());
    let failed: Mutex<Vec<Option<String>>> = Mutex::new(vec![None; maps.len()]);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= tasks.len() {
                    break;
                }
                let (m, t) = tasks[i];
                let (key, _, path) = &maps[m];
                let slot = &slots[m];
                let proto = slot
                    .proto
                    .get_or_init(|| {
                        let tl = std::time::Instant::now();
                        match build_proto(key, path, limit) {
                            Ok(p) => {
                                eprintln!(
                                    "{key} loaded ({:.1} s, {} groups, {} samples)",
                                    tl.elapsed().as_secs_f64(),
                                    p.groups.len(),
                                    p.samples.len()
                                );
                                if let Ok(y) = std::env::var("LEAK_SLICE")
                                    .map(|v| v.parse::<f32>().unwrap_or(0.0))
                                {
                                    let mut s = String::new();
                                    slice(&p, y, near_filter(), &mut s);
                                    eprint!("{s}");
                                }
                                for q in std::env::var("LEAK_PROBE")
                                    .unwrap_or_default()
                                    .split(';')
                                    .filter_map(|v| {
                                        let f: Vec<f32> =
                                            v.split(',').filter_map(|x| x.parse().ok()).collect();
                                        (f.len() == 3).then(|| Vec3::new(f[0], f[1], f[2]))
                                    })
                                {
                                    probe(&p, q);
                                }
                                if let Some(f) = std::env::var("LEAK_HMAP")
                                    .ok()
                                    .map(|v| {
                                        v.split(',')
                                            .filter_map(|x| x.parse::<f32>().ok())
                                            .collect::<Vec<_>>()
                                    })
                                    .filter(|f| f.len() == 5)
                                {
                                    hmap(&p, f[0], f[1], f[2], f[3], f[4]);
                                }
                                let p = Arc::new(p);
                                *slot.held.lock().unwrap() = Some(p.clone());
                                Some(p)
                            }
                            Err(e) => {
                                failed.lock().unwrap()[m] = Some(e);
                                None
                            }
                        }
                    })
                    .clone();
                let task = per_map[t];
                let part = match (&proto, task) {
                    (Some(p), Task::Wall(g, _)) if g >= p.groups.len() => Some(Part::default()),
                    (Some(p), _) => {
                        let tt = std::time::Instant::now();
                        let r = run_task(key, p, t, task, on);
                        eprintln!("{key} {task:?} done ({:.1} s)", tt.elapsed().as_secs_f64());
                        Some(r)
                    }
                    (None, _) => None,
                };
                if let Some(part) = part {
                    parts.lock().unwrap()[i] = Some(part);
                }
                if slot.left.fetch_sub(1, Ordering::SeqCst) == 1 {
                    // The map's last task: merge it and free its sims.
                    if let Some(p) = &proto {
                        let ps: Vec<Part> = {
                            let mut all = parts.lock().unwrap();
                            (0..per_map.len())
                                .filter_map(|k| all[m * per_map.len() + k].take())
                                .collect()
                        };
                        reports.lock().unwrap()[m] = Some(merge_map(key, p, &accepted, ps));
                    }
                    *slot.held.lock().unwrap() = None;
                }
            });
        }
    });
    let reports = reports.into_inner().unwrap();
    let failed = failed.into_inner().unwrap();
    let (mut samples, mut runs, mut sites, mut acc, mut new) = (0, 0, 0, 0, 0);
    let mut site_file = String::new();
    let mut accepted_file = String::new();
    for (m, r) in reports.into_iter().enumerate() {
        if let Some(e) = &failed[m] {
            println!("{e}");
            continue;
        }
        let Some(r) = r else { continue };
        print!("{}", r.text);
        site_file.push_str(&r.site_text);
        accepted_file.push_str(&r.accepted_text);
        eprint!("{}", r.verbose);
        samples += r.samples;
        runs += r.runs;
        sites += r.sites;
        acc += r.accepted;
        new += r.new;
    }
    eprintln!("census wall time {:.1} s", t0.elapsed().as_secs_f64());
    if let Some(p) = std::env::var_os("LEAK_SITES") {
        if let Err(e) = std::fs::write(&p, &site_file) {
            eprintln!("LEAK_SITES {}: {e}", Path::new(&p).display());
        }
    }
    if let Some(p) = std::env::var_os("LEAK_WRITE_ACCEPTED") {
        if let Err(e) = std::fs::write(&p, &accepted_file) {
            eprintln!("LEAK_WRITE_ACCEPTED {}: {e}", Path::new(&p).display());
        }
    }
    let bad = if bad_accept > 0 {
        format!("  bad-accept-lines {bad_accept}")
    } else {
        String::new()
    };
    println!(
        "leak: maps {}  samples {samples}  runs {runs}  sites {sites}  accepted {acc}  new {new}{bad}",
        maps.len()
    );
}

#[cfg(test)]
mod tests {
    use super::parse_accepted;

    #[test]
    fn accepted_lines_parse_and_never_widen() {
        let (a, bad) = parse_accepted(
            "# header\n\
             m push 1 2  3 4  # kept\n\
             m tunnel  # its last cell deleted\n\
             m floor *  # whole class\n\
             m ceiling 1 2 3  # odd\n\
             m inprim 1 x  # garbage\n",
        );
        assert_eq!(bad, 3);
        assert_eq!(a.len(), 2);
        let covered = |c: &str, x, z| a.iter().any(|l| l.covers("m", c, x, z));
        assert!(covered("push", 1, 2) && covered("push", 3, 4));
        assert!(!covered("push", 1, 4));
        assert!(!covered("tunnel", 1, 2));
        assert!(covered("floor", 99, -99));
        assert!(!covered("ceiling", 1, 2) && !covered("inprim", 1, 2));
        assert!(!a.iter().any(|l| l.covers("other", "push", 1, 2)));
    }
}
