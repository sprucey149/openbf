//! The collision LEAK census (card LEAK): on every baked map, drive a real
//! soldier at every wall the soldier collides with — walking, sprinting,
//! jumping and sprint-jumping, from three headings, from both sides — and
//! flag every run that ends somewhere a soldier must not be.
//!
//! ```text
//! cargo run --release -p game --example leak_census [map ...]
//! ```
//!
//! ## What a run is
//! The WALL SAMPLES are the SOLDIER-layer faces the wall pass pushes against:
//! every mesh triangle steeper than the walkable limit (`|n.y| <
//! WALKABLE_MIN_NY`) and at least `MIN_AREA`, and the sides of every box and
//! cylinder primitive. One sample per `CELL` m cell, floor band and facing
//! octant (the first body and triangle in index order wins), so a 3,000-face
//! hut costs a few dozen samples, not thousands.
//!
//! For each sample and each SIDE of it, the soldier (the map's team-0 fill
//! class, its own ODF constants) stands on the floor under a point
//! `RUN_UP` m out from the face, facing it. A start that is not an ordinary
//! place to stand is skipped: no floor within `FLOOR_DROP` m under the
//! face's middle, the face not spanning the soldier's wall band there, the
//! capsule already in contact (a wall push, or ANY contact deeper than 1 cm
//! on the stepped capsule, [`in_contact`]), the axis inside a primitive
//! ([`in_primitive`]), the start itself enclosed (below), or the start inside
//! a death region. Then, per MODE × HEADING (0, ±35 deg off the
//! face normal), `DRIVE` ticks of full stick through `Sim::step` — the real
//! stance machine, ground clamp, wall pass, doors and death regions — then
//! the stick released until the soldier has stood `SETTLE` ticks (at most
//! `SETTLE_MAX`, so a fall off a deck lands inside it):
//! - `walk`     — forward;
//! - `sprint`   — forward + SPRINT;
//! - `jump`     — forward, JUMP pressed (one tick) whenever grounded within
//!   `JUMP_AT` m of the face;
//! - `sjump`    — sprint, the same jump rule at `SJUMP_AT` m;
//! - `spam`     — sprint, JUMP pressed every other tick (jumps AT the wall).
//!
//! ## What a leak is
//! Every run ends in exactly one class, the first that holds. Four are
//! CROSSINGS — between two ticks a point of the soldier's axis above the
//! step (`AXIS_HEIGHTS`, clipped to the stance's capsule) crossed a
//! SOLDIER-layer face (`CollisionWorld::ray`). The axis is inside a 0.5 m
//! capsule, so no legal move crosses a face; the class names the mechanism:
//! - `died`    — killed (a death region or a lethal landing): not a leak;
//! - `push`    — a wall face (`|n.y| < WALKABLE_MIN_NY`) on a tick the wall
//!   pass pushed at least `PUSH_THROUGH` m: the pass put the soldier through;
//! - `tunnel`  — a wall face with a smaller push: the pass never answered;
//! - `ceiling` — a downward face (a head through a roof or a deck's underside);
//! - `floor`   — an upward face;
//!
//! and four are END STATES, tested after the settle:
//! - `closed`  — inside a closed volume by ray parity (an odd number of face
//!   crossings along each of `RAYS` horizontal rays and the ray up) when the
//!   start was not. UNVALIDATED beyond the KAS2 control: open meshes (Endor's
//!   trees) break parity — card LEAK2;
//! - `buried`  — more than `TERRAIN_BURY` under an uncut terrain it started
//!   above (fell under the map);
//! - `offmap`  — outside the heightfield's square or below `NO_GROUND`;
//! - `float`   — still not grounded after `SETTLE_MAX`;
//! - `ok`.
//!
//! `offgraph` is a FLAG on a site, not a class: the start was inside a nav hub
//! or arc corridor and the end is in neither. The nav graph is XZ only, so it
//! cannot tell a deck from the hollow under it; it is printed, not counted.
//!
//! ## The output and the gate
//! STDOUT: per map, the run counts by class and `sites N  accepted A  new N`,
//! then the footer
//!
//! ```text
//! leak: maps M  samples S  runs R  sites T  accepted A  new N
//! ```
//!
//! Leaking runs are grouped into SITES by class and end cell (`SITE_CELL` m,
//! 2 m floor band): one hole found by twelve runs is one site, listed with its
//! first run (in sample order) as the reproduction in the file `LEAK_SITES`
//! names (not on stdout: the first census has ~67,000). A site is ACCEPTED
//! when a line of `leak_census_accepted.txt` beside this file covers it — a
//! site (`map class x z`, the cell centre in whole metres), a map's class or
//! every map's (`*`) — each line with its reason and its card; every other
//! site is NEW. `scripts/gates.py`'s `leak` group runs the census twice and
//! fails when the pair differs or `new` is not 0, and reads MOVED when the
//! stdout differs from `<baseline_dir>/leak_main_1.txt`: a change that moves
//! a count declares `leak`. Deterministic: each map's runs are one
//! single-threaded sim in sample order; maps run on worker threads and print
//! in discovery order (like `round_to_outcome`). ~3 min on 32 threads.
//!
//! The positive control (card JWAL's census, fixed by WPSH): `bf1_kas2 push`
//! sites round the hut on the platform at (5, 16.65, -7) — a jump from the
//! deck at the bamboo wall ended inside the ring. The replay line is in
//! `leak_census_accepted.txt`.
//!
//! Debug (all to STDERR): `LEAK_SAMPLE_LIMIT=<n>` caps the samples per map;
//! `LEAK_VERBOSE=1` lists every leaking run, `=2` every run;
//! `LEAK_NEAR=<x>,<z>,<r>` keeps the samples within r m of (x, z) and prints
//! each; `LEAK_TRACE=<x>,<z>,<mode>` prints every tick of the runs from that
//! start (as a site line prints it); `LEAK_SLICE=<y>` prints the SOLDIER mesh
//! triangles the plane y cuts (with `LEAK_NEAR`'s box); `LEAK_THREADS=<n>`.
//!
//! All of this is OURS (engineering, no RE): the thresholds are the
//! census's, and a verdict about what the PS2 does with the same wall goes
//! through a card (JWAL, WTUN, VCOL, LEAK2).

use core::collision::{Layers, Shape, NO_GROUND, TERRAIN_BURY, WALKABLE_MIN_NY};
use core::nav::{Locus, Mover};
use core::sim::{buttons, PadState, Sim};
use glam::{Vec2, Vec3};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

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
/// The settle: stick released until the soldier has been grounded for
/// `SETTLE` ticks in a row, at most `SETTLE_MAX` ticks (a fall off a deck
/// lands inside it).
const SETTLE: u32 = 10;
/// 420 since card AIRC: BF2's fall has the PS2's drag, so a soldier off a
/// deck falls at 38.7 m/s at most and the 410 m from Bespin's platforms to
/// `NO_GROUND` take 11 s (at 240, 3,700 such falls ended as `float`).
const SETTLE_MAX: u32 = 420;
/// A wall-pass push at least this long (m) on the crossing tick makes it a
/// `push` leak: the pass itself put the soldier through.
const PUSH_THROUGH: f32 = 0.25;
/// Jump when grounded within this far of the face (m): walk / sprint.
const JUMP_AT: f32 = 1.5;
const SJUMP_AT: f32 = 2.5;
/// Heading offsets from straight at the face (deg).
const HEADINGS: [f32; 3] = [0.0, 35.0, -35.0];
/// Points on the soldier's axis, above the feet (m), tested for crossings.
/// All above `STEP_HEIGHT` (a riser the ground clamp climbs is not a wall).
const AXIS_HEIGHTS: [f32; 3] = [0.8, 1.25, 1.7];
/// The parity test: `RAYS` horizontal rays and one up from `CHEST` m over
/// the feet, each followed `PARITY_R` m.
const RAYS: usize = 8;
const CHEST: f32 = 1.2;
const PARITY_R: f32 = 120.0;
/// Site grouping cell (m).
const SITE_CELL: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Mode {
    Walk,
    Sprint,
    Jump,
    SJump,
    Spam,
}
const MODES: [Mode; 5] = [
    Mode::Walk,
    Mode::Sprint,
    Mode::Jump,
    Mode::SJump,
    Mode::Spam,
];

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Walk => "walk",
            Mode::Sprint => "sprint",
            Mode::Jump => "jump",
            Mode::SJump => "sjump",
            Mode::Spam => "spam",
        }
    }
    fn sprint(self) -> bool {
        matches!(self, Mode::Sprint | Mode::SJump | Mode::Spam)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Class {
    Ok,
    Died,
    Push,
    Tunnel,
    Ceiling,
    Floor,
    Closed,
    Buried,
    OffMap,
    Float,
}
/// The classes that are leaks, in print order.
const LEAKS: [Class; 8] = [
    Class::Push,
    Class::Tunnel,
    Class::Ceiling,
    Class::Floor,
    Class::Closed,
    Class::Buried,
    Class::OffMap,
    Class::Float,
];

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Ok => "ok",
            Class::Died => "died",
            Class::Push => "push",
            Class::Tunnel => "tunnel",
            Class::Ceiling => "ceiling",
            Class::Floor => "floor",
            Class::Closed => "closed",
            Class::Buried => "buried",
            Class::OffMap => "offmap",
            Class::Float => "float",
        }
    }
}

/// One wall sample: a point on a face and the face's horizontal normal.
#[derive(Clone, Copy)]
struct Sample {
    body: usize,
    point: Vec3,
    normal: Vec2,
    /// The face's vertical extent.
    y_lo: f32,
    y_hi: f32,
}

/// The first leaking run of a site: enough to replay it. `ord` is where the run stands in the one-thread order
/// (sample, side, mode, heading): chunks of a map are merged by it, so the report is the one-thread report.
struct Site {
    ord: (usize, u8, u8, u8),
    class: Class,
    cell: (i32, i32, i32),
    runs: u32,
    start: Vec3,
    yaw: f32,
    mode: Mode,
    tick: u32,
    end: Vec3,
    body: usize,
    offgraph: bool,
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
    for (bi, b) in sim.collision.bodies().iter().enumerate() {
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
                            body: bi,
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
                            body: bi,
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
                            body: bi,
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
fn slice(sim: &Sim, y: f32, out: &mut String) {
    let near: Vec<f32> = std::env::var("LEAK_NEAR")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
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
                if near.len() == 3
                    && Vec2::new(cut[0].x - near[0], cut[0].z - near[1]).length() > near[2]
                {
                    continue;
                }
                outln!(
                    out,
                    "slice {bi} {} {} {} {}",
                    cut[0].x,
                    cut[0].z,
                    cut[1].x,
                    cut[1].z
                );
            }
        }
    }
}

/// Crossings of SOLDIER collision along a ray, to `PARITY_R` m.
fn crossings(sim: &Sim, mut o: Vec3, dir: Vec3) -> u32 {
    let mut left = PARITY_R;
    let mut n = 0;
    while n < 64 {
        let Some((t, _)) = sim.collision.ray(o, dir, left, Layers::SOLDIER) else {
            break;
        };
        n += 1;
        let step = t + 1e-3;
        o += dir * step;
        left -= step;
        if left <= 0.0 {
            break;
        }
    }
    n
}

/// Inside a closed volume by ray parity: from the chest, an ODD number of
/// SOLDIER-face crossings along every one of `RAYS` horizontal rays and the
/// ray up. A room with a door or a window has a ray that leaves through it
/// (0 crossings); a wall modelled with two faces is crossed twice.
/// Is a point of the soldier's axis (`AXIS_HEIGHTS`, the crossing test's)
/// inside a SOLDIER-layer PRIMITIVE? Such a start is inside a solid, not
/// beside a wall, and the pass that pushes him out of it is right to. The
/// bare pass does not see it:
/// - from deep in a box that sits on the floor, the shortest way out is DOWN,
///   a ceiling-like contact it skips, until a few steps later a side is
///   nearer and the pass pushes the soldier out through it (card WPSH:
///   TAT2's 7.8 m box at (24.9, -82.0), most of that map's `push` runs);
/// - a primitive whose top is between the step and the chest (BF2 YAV1's
///   1 m round platform at (116.1, -439.5), r 2.8) is a ledge, which the
///   stepped capsule's bare pass skips; the soldier's pass (the ledge rule)
///   pushes him out radially on the first tick.
fn in_primitive(sim: &Sim, feet: Vec3) -> bool {
    AXIS_HEIGHTS
        .iter()
        .any(|h| inside_primitive(sim, feet + Vec3::Y * *h))
}

/// Does the STEPPED capsule (spheres of its radius along its segment, the
/// same 0.6 … 1.9 m the bare pass asks) touch any SOLDIER body deeper than
/// 1 cm, whatever the contact's normal? The bare pass's test below answers
/// only for a WALL-like contact, so a start whose deepest contact is a ceiling, a ledge above the
/// step or the inside of a small open mesh (BF1 BES1's 1.5 m object at
/// (88.5, 180.5): the axis inside its skins, pushed out through the far one
/// on tick 3) counted as a clean start (card WPSH).
fn in_contact(sim: &Sim, feet: Vec3) -> bool {
    let r = core::unit::SOLDIER_RADIUS;
    let top = core::unit::capsule_height(core::soldier::Stance::Stand);
    let (lo, hi) = (core::collision::STEP_HEIGHT + r, top - r);
    let mut out = Vec::new();
    [lo, (lo + hi) * 0.5, hi].iter().any(|h| {
        out.clear();
        sim.collision
            .sphere_contacts(feet + Vec3::Y * *h, r, Layers::SOLDIER, &mut out);
        out.iter().any(|(depth, _)| *depth > 0.01)
    })
}

fn inside_primitive(sim: &Sim, o: Vec3) -> bool {
    sim.collision.bodies().iter().any(|b| {
        let (lo, hi) = (b.aabb.min, b.aabb.max);
        if !b.layers.contains(Layers::SOLDIER) || o.cmplt(lo).any() || o.cmpgt(hi).any() {
            return false;
        }
        let l = b.to_local.transform_point3(o);
        match &b.shape {
            Shape::Mesh(_) => false,
            Shape::Sphere { radius } => l.length() < *radius,
            Shape::Cylinder {
                radius,
                half_height,
            } => l.x * l.x + l.z * l.z < radius * radius && l.y.abs() < *half_height,
            Shape::Box { half } => l.x.abs() < half.x && l.y.abs() < half.y && l.z.abs() < half.z,
        }
    })
}

fn enclosed(sim: &Sim, feet: Vec3) -> bool {
    let o = feet + Vec3::Y * CHEST;
    if crossings(sim, o, Vec3::Y).is_multiple_of(2) {
        return false;
    }
    (0..RAYS).all(|k| {
        let (s, c) = core::fmath::sin_cos(k as f32 * std::f32::consts::TAU / RAYS as f32);
        !crossings(sim, o, Vec3::new(c, 0.0, s)).is_multiple_of(2)
    })
}

/// The normal of the first SOLDIER-layer face the soldier's axis crossed
/// between `a` and `b` (feet).
fn crossing(sim: &Sim, a: Vec3, b: Vec3, height: f32) -> Option<Vec3> {
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
        if let Some((_t, n)) = sim.collision.ray(o, dir, len, Layers::SOLDIER) {
            return Some(n);
        }
    }
    None
}

/// Is `p` inside a nav hub or arc corridor (not merely nearest a hub)?
fn on_graph(sim: &Sim, p: Vec3) -> bool {
    matches!(
        sim.nav
            .as_ref()
            .and_then(|n| n.locate_unit_on(p, Mover::SOLDIER, &|_| true)),
        Some(Locus::Hub(_)) | Some(Locus::Arc(..))
    )
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

/// `yaw` facing the XZ direction `d` (yaw 0 = −Z, `Soldier::step`'s axes).
fn yaw_of(d: Vec2) -> f32 {
    core::fmath::atan2(-d.x, -d.y)
}

/// One chunk of a map: samples whose index is `chunk` mod `chunks`.
/// See the `LEAK_CHUNKS` caveat in `main`: state dependence makes k>1
/// reports differ from the k=1 gate. [`merge_map`] combines the chunks.
struct Part {
    /// A failed load, or the build messages of the map (chunk 0's).
    out: String,
    failed: bool,
    verbose: String,
    counts: [u32; 10],
    starts: usize,
    skipped: usize,
    runs: usize,
    samples: usize,
    sites: Vec<Site>,
}

struct MapReport {
    text: String,
    site_text: String,
    verbose: String,
    samples: usize,
    runs: usize,
    sites: usize,
    accepted: usize,
    new: usize,
}

fn mission_for(key: &str) -> Option<game::mission::MissionDef> {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let path = dir.join(format!("{key}_con.ron"));
    game::mission::MissionDef::load(&path).ok()
}

/// Build the map's sim the way the round sweep does (mode layer, common
/// props, the mission's death regions, the nav graph), with one player.
fn build_sim(key: &str, path: &Path, out: &mut String) -> Option<game::walk::WalkSim> {
    let (scene, _) = match game::assets::load_battle_scene(path, "con") {
        Ok(s) => s,
        Err(e) => {
            outln!(out, "{key:10} LOAD FAILED {e:#}");
            return None;
        }
    };
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
    let mut team = game::walk::TeamTemplate::default_trooper();
    match game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    ) {
        Ok((rs, _)) => {
            let f = rs[0].fill_class();
            team.consts = f.consts;
            team.name = f.class.clone();
        }
        Err(e) => outln!(out, "{key:10} roster unavailable: {e:#} (built-in trooper)"),
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
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.landing_regions = a.landing_regions.clone();
    }
    Some(w)
}

fn run_part(key: &str, path: &Path, limit: Option<usize>, chunk: usize, chunks: usize) -> Part {
    let mut out = String::new();
    let mut verbose = String::new();
    let mut runs = 0usize;
    let Some(w) = build_sim(key, path, &mut out) else {
        return Part {
            out,
            failed: true,
            verbose,
            counts: [0; 10],
            starts: 0,
            skipped: 0,
            runs: 0,
            samples: 0,
            sites: Vec::new(),
        };
    };
    let mut sim = w.sim;
    let player = w.player;
    let pi = player.index as usize;
    let template = sim.units[pi].clone();
    let square = map_square(&sim);
    // `LEAK_SLICE=<y>`: the SOLDIER-layer mesh triangles cut by the plane
    // y, as `slice body x0 z0 x1 z1` on STDERR (with `LEAK_NEAR`'s box).
    if let Ok(y) = std::env::var("LEAK_SLICE").map(|v| v.parse::<f32>().unwrap_or(0.0)) {
        if chunk == 0 {
            slice(&sim, y, &mut verbose);
        }
    }
    let mut samples = wall_samples(&sim);
    // `LEAK_NEAR=<x>,<z>,<r>`: only the samples within r m of (x, z).
    if let Some(f) = std::env::var("LEAK_NEAR").ok().map(|v| {
        v.split(',')
            .filter_map(|x| x.parse::<f32>().ok())
            .collect::<Vec<_>>()
    }) {
        if f.len() == 3 {
            samples.retain(|s| Vec2::new(s.point.x - f[0], s.point.z - f[1]).length() <= f[2]);
        }
    }
    if let Some(n) = limit {
        samples.truncate(n);
    }
    // `LEAK_VERBOSE=1`: every leaking run on STDERR; `=2`: every run.
    let verbose_all = std::env::var("LEAK_VERBOSE").as_deref() == Ok("2");
    let verbose_on = !verbose_all && std::env::var_os("LEAK_VERBOSE").is_some();
    // `LEAK_TRACE=<x>,<z>,<mode>`: every tick of the runs from that start.
    let trace: Option<(String, String, String)> = std::env::var("LEAK_TRACE").ok().and_then(|v| {
        let f: Vec<&str> = v.split(',').collect();
        (f.len() == 3).then(|| (f[0].to_owned(), f[1].to_owned(), f[2].to_owned()))
    });

    let mut sites: Vec<Site> = Vec::new();
    let mut counts = [0u32; 10];
    let mut starts = 0usize;
    let mut skipped = 0usize;
    for (si, s) in samples.iter().enumerate() {
        if si % chunks != chunk {
            continue;
        }
        for (sd, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let n = s.normal * side;
            // The floor under the face's middle, on this side.
            let near = s.point + Vec3::new(n.x, 0.0, n.y) * 0.8;
            let floor = sim.ground_under(near, false);
            if std::env::var_os("LEAK_NEAR").is_some() {
                eprintln!(
                    "sample body {} p ({:.2},{:.2},{:.2}) n ({:.2},{:.2}) y {:.2}..{:.2} side {side} floor {floor:.2}",
                    s.body, s.point.x, s.point.y, s.point.z, n.x, n.y, s.y_lo, s.y_hi
                );
            }
            if floor < s.point.y - FLOOR_DROP || floor > s.point.y + 0.1 {
                skipped += 1;
                continue;
            }
            // The face must stand in the soldier's wall band there.
            if s.y_hi < floor + 0.8 || s.y_lo > floor + 1.8 {
                skipped += 1;
                continue;
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
                || enclosed(&sim, start)
                || in_primitive(&sim, start)
                || in_contact(&sim, start)
            {
                skipped += 1;
                continue;
            }
            let probe = core::collision::Capsule {
                base: start,
                height: core::unit::capsule_height(core::soldier::Stance::Stand),
                radius: core::unit::SOLDIER_RADIUS,
            };
            if sim.collision.push_out(&probe.stepped()) != Vec3::ZERO {
                skipped += 1;
                continue;
            }
            starts += 1;
            let start_on_graph = on_graph(&sim, start);
            let start_buried = sim
                .heightfield
                .as_ref()
                .is_some_and(|h| h.height_at(start.x, start.z) > start.y + TERRAIN_BURY);
            for (mi, mode) in MODES.into_iter().enumerate() {
                for (hi, hd) in HEADINGS.into_iter().enumerate() {
                    let (sa, ca) = core::fmath::sin_cos(hd.to_radians());
                    let toward = -n;
                    let d = Vec2::new(toward.x * ca - toward.y * sa, toward.x * sa + toward.y * ca);
                    let yaw = yaw_of(d);
                    let mut u = template.clone();
                    u.soldier.pos = start;
                    u.soldier.yaw = yaw;
                    sim.units[pi] = u;
                    runs += 1;
                    let mut class = Class::Ok;
                    let mut at_tick = 0u32;
                    let tracing = trace.as_ref().is_some_and(|(x, z, m)| {
                        *x == format!("{:.2}", start.x)
                            && *z == format!("{:.2}", start.z)
                            && m == mode.name()
                    });
                    let mut still = 0u32;
                    let mut t = 0u32;
                    while t < DRIVE + SETTLE_MAX && still < SETTLE {
                        let before = sim.units[pi].soldier.pos;
                        let mut pad = PadState::default();
                        if t < DRIVE {
                            pad.ly = i16::MAX;
                            if mode.sprint() {
                                pad.buttons |= buttons::SPRINT;
                            }
                            let dist = Vec2::new(before.x - s.point.x, before.z - s.point.z).dot(n);
                            let so = &sim.units[pi].soldier;
                            let jump = match mode {
                                Mode::Jump => so.grounded && dist < JUMP_AT,
                                Mode::SJump => so.grounded && dist < SJUMP_AT,
                                Mode::Spam => t.is_multiple_of(2),
                                _ => false,
                            };
                            if jump && so.prev_buttons() & buttons::JUMP == 0 {
                                pad.buttons |= buttons::JUMP;
                            }
                        }
                        sim.step(&[pad]);
                        let u = &sim.units[pi];
                        if !u.alive {
                            class = Class::Died;
                            at_tick = t;
                            break;
                        }
                        let h = core::unit::capsule_height(u.soldier.stance);
                        if tracing {
                            eprintln!(
                                "trace yaw {yaw:.4} t {t} pos ({:.3},{:.3},{:.3}) vel ({:.2},{:.2},{:.2}) {:?} grounded {} push ({:.3},{:.3}) cross {:?}",
                                u.soldier.pos.x, u.soldier.pos.y, u.soldier.pos.z,
                                u.soldier.vel.x, u.soldier.vel.y, u.soldier.vel.z,
                                u.soldier.stance, u.soldier.grounded, u.wall_push.x, u.wall_push.z,
                                crossing(&sim, before, u.soldier.pos, h)
                            );
                        }
                        if let Some(n) = crossing(&sim, before, u.soldier.pos, h) {
                            class = if n.y.abs() < WALKABLE_MIN_NY {
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
                            at_tick = t;
                            break;
                        }
                        if t >= DRIVE {
                            still = if u.soldier.grounded { still + 1 } else { 0 };
                        }
                        t += 1;
                    }
                    let end = sim.units[pi].soldier.pos;
                    if class == Class::Ok {
                        at_tick = t;
                        let hf_y = sim.heightfield.as_ref().map(|h| h.height_at(end.x, end.z));
                        let off = square.is_some_and(|(x0, z0, x1, z1)| {
                            end.x < x0 || end.z < z0 || end.x > x1 || end.z > z1
                        }) || end.y < NO_GROUND + 1.0;
                        if enclosed(&sim, end) {
                            class = Class::Closed;
                        } else if !start_buried
                            && !sim.collision.is_cut(end)
                            && hf_y.is_some_and(|y| y > end.y + TERRAIN_BURY)
                        {
                            class = Class::Buried;
                        } else if off {
                            class = Class::OffMap;
                        } else if !sim.units[pi].soldier.grounded {
                            class = Class::Float;
                        }
                    }
                    counts[class as usize] += 1;
                    if verbose_all {
                        outln!(
                            verbose,
                            "{key} {} {} start ({:.2},{:.2},{:.2}) yaw {:.4} tick {} end ({:.2},{:.2},{:.2}) body {}",
                            class.name(), mode.name(), start.x, start.y, start.z, yaw, at_tick, end.x, end.y, end.z, s.body
                        );
                    }
                    if !LEAKS.contains(&class) {
                        continue;
                    }
                    let offgraph = start_on_graph && !on_graph(&sim, end);
                    if verbose_on {
                        outln!(
                            verbose,
                            "{key} {} {} start ({:.2},{:.2},{:.2}) yaw {:.4} tick {} end ({:.2},{:.2},{:.2}) body {}{}",
                            class.name(),
                            mode.name(),
                            start.x,
                            start.y,
                            start.z,
                            yaw,
                            at_tick,
                            end.x,
                            end.y,
                            end.z,
                            s.body,
                            if offgraph { " offgraph" } else { "" }
                        );
                    }
                    let cell = (
                        (end.x / SITE_CELL).floor() as i32,
                        (end.z / SITE_CELL).floor() as i32,
                        (end.y / 2.0).floor() as i32,
                    );
                    match sites
                        .iter_mut()
                        .find(|x| x.class == class && x.cell == cell)
                    {
                        Some(x) => {
                            x.runs += 1;
                            x.offgraph |= offgraph;
                        }
                        None => sites.push(Site {
                            ord: (si, sd as u8, mi as u8, hi as u8),
                            class,
                            cell,
                            runs: 1,
                            start,
                            yaw,
                            mode,
                            tick: at_tick,
                            end,
                            body: s.body,
                            offgraph,
                        }),
                    }
                }
            }
        }
    }
    Part {
        out,
        failed: false,
        verbose,
        counts,
        starts,
        skipped,
        runs,
        samples: samples.len(),
        sites,
    }
}

/// The chunks of one map, in chunk order, as the one report the one-thread census printed.
fn merge_map(key: &str, accepted: &[Accepted], parts: Vec<Part>) -> MapReport {
    let mut rep = MapReport {
        text: String::new(),
        site_text: String::new(),
        verbose: String::new(),
        samples: 0,
        runs: 0,
        sites: 0,
        accepted: 0,
        new: 0,
    };
    if parts.first().is_none_or(|p| p.failed) {
        rep.text = parts.into_iter().next().map(|p| p.out).unwrap_or_default();
        return rep;
    }
    let mut out = String::new();
    let mut verbose = String::new();
    let mut counts = [0u32; 10];
    let (mut starts, mut skipped) = (0usize, 0usize);
    let mut sites: Vec<Site> = Vec::new();
    for (c, p) in parts.into_iter().enumerate() {
        if c == 0 {
            out = p.out;
            rep.samples = p.samples;
        }
        verbose.push_str(&p.verbose);
        for (a, b) in counts.iter_mut().zip(p.counts) {
            *a += b;
        }
        starts += p.starts;
        skipped += p.skipped;
        rep.runs += p.runs;
        for x in p.sites {
            match sites
                .iter_mut()
                .find(|y| y.class == x.class && y.cell == x.cell)
            {
                Some(y) => {
                    y.runs += x.runs;
                    y.offgraph |= x.offgraph;
                    if x.ord < y.ord {
                        y.ord = x.ord;
                        y.start = x.start;
                        y.yaw = x.yaw;
                        y.mode = x.mode;
                        y.tick = x.tick;
                        y.end = x.end;
                        y.body = x.body;
                    }
                }
                None => sites.push(x),
            }
        }
    }
    let samples = rep.samples;
    rep.sites = sites.len();
    outln!(
        out,
        "{key:10} samples {} starts {} skipped {} runs {} | ok {} died {} | {} | sites {}",
        samples,
        starts,
        skipped,
        rep.runs,
        counts[Class::Ok as usize],
        counts[Class::Died as usize],
        LEAKS
            .iter()
            .map(|c| format!("{} {}", c.name(), counts[*c as usize]))
            .collect::<Vec<_>>()
            .join(" "),
        sites.len()
    );
    sites.sort_by_key(|x| (x.class, x.cell));
    let mut site_text = String::new();
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
        outln!(
            site_text,
            "{key} {} {cx} {cz}  {}  runs {}  first: {} start ({:.2},{:.2},{:.2}) yaw {:.4} tick {} end ({:.2},{:.2},{:.2}) sample body {}{}  | {}",
            x.class.name(),
            if reason.is_some() { "accepted" } else { "NEW" },
            x.runs,
            x.mode.name(),
            x.start.x,
            x.start.y,
            x.start.z,
            x.yaw,
            x.tick,
            x.end.x,
            x.end.y,
            x.end.z,
            x.body,
            if x.offgraph { " offgraph" } else { "" },
            reason.map_or("", |a| a.reason.as_str()),
        );
    }
    outln!(
        out,
        "{key:10} sites {}  accepted {}  new {}",
        sites.len(),
        rep.accepted,
        rep.new
    );
    rep.text = out;
    rep.site_text = site_text;
    rep.verbose = verbose;
    rep
}

/// One line of `leak_census_accepted.txt`: `<map|*> <class|*> [<x> <z>]
/// # <reason>` — a whole map's class, every map's, or one site (its cell
/// centre, as the sites file prints it).
struct Accepted {
    map: String,
    class: String,
    at: Option<(i32, i32)>,
    reason: String,
}

impl Accepted {
    fn covers(&self, map: &str, class: &str, x: i32, z: i32) -> bool {
        (self.map == "*" || self.map == map)
            && (self.class == "*" || self.class == class)
            && self.at.is_none_or(|a| a == (x, z))
    }
}

fn accepted_list() -> Vec<Accepted> {
    let p = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/leak_census_accepted.txt"
    ));
    let text = std::fs::read_to_string(p).unwrap_or_default();
    let mut out = Vec::new();
    for line in text.lines() {
        let (spec, reason) = line.split_once('#').unwrap_or((line, ""));
        let f: Vec<&str> = spec.split_whitespace().collect();
        if f.len() < 2 {
            continue;
        }
        let at = match (
            f.get(2).and_then(|v| v.parse().ok()),
            f.get(3).and_then(|v| v.parse().ok()),
        ) {
            (Some(x), Some(z)) => Some((x, z)),
            _ => None,
        };
        out.push(Accepted {
            map: f[0].to_owned(),
            class: f[1].to_owned(),
            at,
            reason: reason.trim().to_owned(),
        });
    }
    out
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
    let accepted = accepted_list();
    // `LEAK_CHUNKS=<k>` (default 1): each map's samples are dealt to k chunks (sample index mod k) that run as separate
    // tasks, each on its own copy of the map's sim, and are merged back in order. The slowest map ran alone on one thread
    // and set the wall time (192 s against a 71 s average over 32 threads), and k = 4 brings it to about 80 s - BUT k > 1
    // is NOT byte-identical to k = 1: a run's result depends on state an earlier run left in the sim (BF2 KAM1, k = 4: one
    // run of 52,845 goes ok -> ceiling, start (-189.28, 75.30, 129.40), yaw -2.2269, spam). The doors, the sim tick, `lcg`
    // and `rng` are NOT it (reset before every run, the split still differs; found 2026-10-07, card GSPD). The gate stays
    // at 1 until a card finds that state; k > 1 is for a quick look.
    let chunks = std::env::var("LEAK_CHUNKS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let tasks = maps.len() * chunks;
    let threads = std::env::var("LEAK_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .clamp(1, tasks.max(1));
    let next = AtomicUsize::new(0);
    let parts: Mutex<Vec<Option<Part>>> = Mutex::new((0..tasks).map(|_| None).collect());
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= tasks {
                    break;
                }
                let (key, _, path) = &maps[i / chunks];
                let t = std::time::Instant::now();
                let r = run_part(key, path, limit, i % chunks, chunks);
                eprintln!(
                    "{key} part {}/{chunks} done ({:.1} s)",
                    i % chunks + 1,
                    t.elapsed().as_secs_f64()
                );
                parts.lock().unwrap()[i] = Some(r);
            });
        }
    });
    let mut parts = parts.into_inner().unwrap();
    let results: Vec<MapReport> = maps
        .iter()
        .enumerate()
        .map(|(m, (key, _, _))| {
            let ps: Vec<Part> = (0..chunks)
                .filter_map(|c| parts[m * chunks + c].take())
                .collect();
            merge_map(key, &accepted, ps)
        })
        .collect();
    let (mut samples, mut runs, mut sites, mut acc, mut new) = (0, 0, 0, 0, 0);
    let mut site_file = String::new();
    for r in &results {
        print!("{}", r.text);
        site_file.push_str(&r.site_text);
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
    println!(
        "leak: maps {}  samples {samples}  runs {runs}  sites {sites}  accepted {acc}  new {new}",
        maps.len()
    );
}
