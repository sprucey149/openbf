//! The ORDNANCE floor census (card GRFL): on every baked map, for every floor
//! a soldier can stand on over the nav graph, is a grenade held up by it?
//! The maintainer's playtest, 2026-10-07: grenades thrown on two Kamino decks fell
//! through the floor he stood on.
//!
//! ```text
//! ordnance_census [map ...]
//!   ORDN_CELL=<m>      grid step (default 2)
//!   ORDN_AT=x,y,z      one spot instead of a grid: every surface on the
//!                      vertical line, the bodies there, and the four runs
//!   ORDN_SITES=<file>  every flagged floor point, one line each
//!   ORDN_STATIC=1      no sim runs
//!   ORDN_TRACE=1       every grenade's tick on stderr (with ORDN_AT)
//! ```
//!
//! FLOORS: a grid of `CELL` m over the hubs' box, the cells inside a nav hub
//! or arc corridor (the graph is XZ only), and under each every level
//! `Sim::ground_under` answers going down from the top (at most `LEVELS`),
//! kept when a soldier has `HEADROOM` over it and it lies within `HUB_BAND` m
//! of the nearest hub's authored height.
//!
//! STATIC: a floor has a GAP when no ordnance surface (an ORDNANCE-layer
//! face, or the terrain by the grenade's own segment test) is met by a ray
//! from `PROBE_UP` over it down to `GAP_TOL` under it. Counted, not gated.
//!
//! DYNAMIC: four real grenades per floor through `Sim::step` (pushed into
//! `Sim::stickies`, the thermal's constants) — a DROP from 1.5 m and three
//! THROWS from 1.5 m at 6 m/s level, headings 0 / 120 / 240 deg. A run is
//! THROUGH when between two ticks the grenade's path crossed an up-facing
//! SOLDIER-layer face (`n.y > 0.7`, going down) and it then went `GAP_TOL`
//! under the crossing, or was still moving at the end: it passed a floor a
//! soldier stands on. One that comes to rest within `GAP_TOL` under it is
//! SUNK, not through: it lies on an ordnance face the map authors a little
//! under the soldier floor (10 to 31 cm on kas1, dag1, fel1, tat2, uta1),
//! the same tolerance the STATIC gap test gives.
//!
//! THE GATE (`scripts/gates.py`, with the `leak` group): a through run is
//! keyed by the body it CROSSED — `S-` a soldier-only body (the disc gives it
//! no ordnance layer: authored), `SG` a body with both layers whose ordnance
//! faces do not reach under that soldier face where the grenade went down (a
//! GAP, as the STATIC test finds them: authored), `SO` one the grenade should
//! have met (a sweep fault), `--` none found at the crossing — and is ACCEPTED when a
//! line of `ordnance_census_accepted.txt` beside this file covers that key;
//! every other is NEW. The footer
//!
//! ```text
//! ordnance: maps M  floors F  gaps G  runs R  through T  sunk K  accepted A  new N
//! ```
//!
//! must equal `[ordnance] footer` in `engine/artifacts/loop/gates.toml`, and
//! `new` must be 0. 16 threads at most (a map each).
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)
use core::collision::{ray_terrain_ps2, HullFilter, Layers};

use core::nav::{Locus, Mover};
use core::sim::{PadState, Sim};
use core::sticky::{Sticky, StickyClass};
use core::weapon::{OrdnanceClass, WeaponClass};
use glam::Vec3;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[path = "support/ordnance_grid.rs"]
mod ordnance_grid;

const LEVELS: usize = 8;
const HEADROOM: f32 = 1.8;
const HUB_BAND: f32 = 6.0;
const PROBE_UP: f32 = 1.0;
const GAP_TOL: f32 = 0.4;
/// How far over a crossing the `SG` test starts (a soldier face a little
/// under an ordnance one).
const GAP_UP: f32 = 0.05;
const TICKS: u32 = 360;
const THROW_UP: f32 = 1.5;
const THROW_SPEED: f32 = 6.0;
const NO_GROUND: f32 = -250.0;

fn mission_for(key: &str) -> Option<game::mission::MissionDef> {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    game::mission::MissionDef::load(&dir.join(format!("{key}_con.ron"))).ok()
}

/// `leak_census`'s sim: mode layer, common props, death regions, nav graph.
fn build_sim(key: &str, path: &Path) -> Option<(game::walk::WalkSim, Vec<String>, bool)> {
    let (scene, _) = game::assets::load_battle_scene(path, "con").ok()?;
    let mission = mission_for(key);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut sim = Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    let team = game::walk::TeamTemplate::default_trooper();
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
    }
    let bf2 = scene.manifest.source_game == "bf2";
    Some((w, scene.collision_body_labels(), bf2))
}

/// The thermal detonator's flight constants (`ordnance.rs`'s test fixture);
/// the fuse is never armed long enough to matter (the run ends first).
fn thermal(bf2: bool) -> WeaponClass {
    WeaponClass {
        name: "thermal".into(),
        ordnance: OrdnanceClass {
            velocity: 25.0,
            life_span: 1.0e6,
            sticky: Some(StickyClass {
                gravity: -12.25,
                stick_mask: if bf2 { 0x7fff_ffd8 } else { 0x7fff_ff80 },
                ..StickyClass::new(bf2)
            }),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    }
}

fn on_graph(sim: &Sim, p: Vec3) -> bool {
    matches!(
        sim.nav
            .as_ref()
            .and_then(|n| n.locate_unit_on(p, Mover::SOLDIER, &|_| true)),
        Some(Locus::Hub(_)) | Some(Locus::Arc(..))
    )
}

fn nearest_hub_dy(sim: &Sim, p: Vec3) -> f32 {
    let Some(n) = sim.nav.as_ref() else {
        return 0.0;
    };
    let mut best = (f32::MAX, f32::MAX);
    for h in &n.hubs {
        let d = (h.pos.x - p.x).hypot(h.pos.z - p.z);
        let dy = (h.pos.y - p.y).abs();
        // The nearest hub in XZ whose disc (plus a corridor's worth) holds the point; else the nearest.
        let k = (d - h.radius).max(0.0) + dy * 0.01;
        if k < best.0 {
            best = (k, dy);
        }
    }
    best.1
}

/// The ordnance surface a grenade over `f` (a floor point) meets going down:
/// its height, and `t` for terrain / `m` for a model face.
fn ordnance_floor(sim: &Sim, f: Vec3) -> Option<(f32, char)> {
    ordnance_under(sim, f + Vec3::Y * PROBE_UP, PROBE_UP + GAP_TOL)
}

/// The highest ordnance surface within `max` under `o`.
fn ordnance_under(sim: &Sim, o: Vec3, max: f32) -> Option<(f32, char)> {
    let mut best: Option<(f32, char)> = None;
    if let Some(hf) = sim.heightfield.as_ref() {
        if let Some(k) = ray_terrain_ps2(hf, &sim.collision.cutters, o, o - Vec3::Y * max) {
            best = Some((o.y - max * k, 't'));
        }
    }
    if let Some(h) = sim
        .collision
        .ray_with(o, -Vec3::Y, max, Layers::ORDNANCE, HullFilter::All)
    {
        let y = o.y - h.t;
        if best.is_none_or(|b| y > b.0) {
            best = Some((y, 'm'));
        }
    }
    best
}

/// Every soldier floor under (x, z): going down from the top, and from just
/// over every hub whose disc is near (a map under an uncut terrain answers
/// the terrain from the top).
fn levels(sim: &Sim, x: f32, z: f32) -> Vec<f32> {
    let mut out: Vec<f32> = Vec::new();
    let mut starts = vec![600.0f32];
    if let Some(n) = sim.nav.as_ref() {
        for h in &n.hubs {
            if (h.pos.x - x).hypot(h.pos.z - z) < h.radius + 8.0 {
                starts.push(h.pos.y + 1.0);
            }
        }
    }
    for s in starts {
        let mut y = s;
        for _ in 0..LEVELS {
            let g = sim.ground_under(Vec3::new(x, y, z), false);
            if g < NO_GROUND || g > y + 1.0 {
                break;
            }
            if !out.iter().any(|o| (o - g).abs() < 0.3) {
                out.push(g);
            }
            y = g - 1.0;
        }
    }
    out.sort_by(|a, b| b.total_cmp(a));
    out
}

fn headroom(sim: &Sim, f: Vec3) -> bool {
    sim.collision
        .ray(f + Vec3::Y * 0.4, Vec3::Y, HEADROOM - 0.4, Layers::SOLDIER)
        .is_none()
}

fn bodies_at(sim: &Sim, labels: &[String], p: Vec3, out: &mut String) {
    for (i, b) in sim.collision.bodies().iter().enumerate() {
        let (lo, hi) = (b.aabb.min, b.aabb.max);
        if p.x < lo.x || p.x > hi.x || p.z < lo.z || p.z > hi.z {
            continue;
        }
        if p.y < lo.y - 1.0 || p.y > hi.y + 1.0 {
            continue;
        }
        let s = b.layers.contains(Layers::SOLDIER);
        let o = b.layers.contains(Layers::ORDNANCE);
        let kind = match &b.shape {
            core::collision::Shape::Mesh(_) => "mesh",
            _ => "prim",
        };
        let _ = writeln!(
            out,
            "    body {i:5} {kind} {}{}  y {:.2}..{:.2}  {}",
            if s { 'S' } else { '-' },
            if o { 'O' } else { '-' },
            lo.y,
            hi.y,
            labels.get(i).map_or("?", String::as_str)
        );
    }
}

/// The label of a SOLDIER body whose box holds the point and has no ORDNANCE
/// bit (`S-`), else of any soldier body there (`SO`), else `-- none`.
fn suspect(sim: &Sim, labels: &[String], p: Vec3) -> String {
    let mut any = None;
    for (i, b) in sim.collision.bodies().iter().enumerate() {
        let (lo, hi) = (b.aabb.min, b.aabb.max);
        if p.x < lo.x || p.x > hi.x || p.z < lo.z || p.z > hi.z {
            continue;
        }
        if p.y < lo.y - 0.3 || p.y > hi.y + 0.3 || !b.layers.contains(Layers::SOLDIER) {
            continue;
        }
        let l = labels.get(i).map_or("?", String::as_str);
        if !b.layers.contains(Layers::ORDNANCE) {
            return format!("S- {l}");
        }
        any.get_or_insert_with(|| format!("SO {l}"));
    }
    any.unwrap_or_else(|| "-- none".into())
}

struct Run {
    floor: usize,
    kind: u8,
    through: Option<Vec3>,
    /// The soldier face it crossed going down, not yet `GAP_TOL` under.
    crossed: Option<Vec3>,
    /// Where it went `GAP_TOL` under the crossing, no ordnance face lies
    /// under the crossing's height (`SG`).
    gap: bool,
    end: Vec3,
}

/// Step every run's grenade at once (grenades meet only the map and living
/// units; the one unit is the owner, which the sweep skips).
fn fly(sim: &mut Sim, wc: usize, floors: &[Vec3], bf2: bool) -> Vec<Run> {
    let owner = sim.alive_units().next().map(|(h, _)| h);
    let Some(owner) = owner else {
        return Vec::new();
    };
    let class = StickyClass {
        gravity: -12.25,
        ..StickyClass::new(bf2)
    };
    let mut runs = Vec::new();
    sim.stickies.clear();
    for (fi, f) in floors.iter().enumerate() {
        for kind in 0..4u8 {
            let vel = if kind == 0 {
                Vec3::ZERO
            } else {
                let a = (f32::from(kind - 1) * 120.0).to_radians();
                Vec3::new(a.sin(), 0.0, a.cos()) * THROW_SPEED
            };
            sim.stickies.push(Sticky::new(
                *f + Vec3::Y * THROW_UP,
                vel,
                &class,
                1.0e6,
                owner,
                0,
                wc,
            ));
            runs.push(Run {
                floor: fi,
                kind,
                through: None,
                crossed: None,
                gap: false,
                end: Vec3::ZERO,
            });
        }
    }
    let pad = PadState::default();
    let trace = std::env::var_os("ORDN_TRACE").is_some();
    for _ in 0..TICKS {
        let prev: Vec<Vec3> = sim.stickies.iter().map(|g| g.pos).collect();
        sim.step(std::slice::from_ref(&pad));
        assert_eq!(sim.stickies.len(), runs.len(), "a grenade left the list");
        let mut live = false;
        for (i, r) in runs.iter_mut().enumerate() {
            let (a, moving) = (prev[i], sim.stickies[i].moving);
            let b = sim.stickies[i].pos;
            if trace && r.through.is_none() && (moving || a != b) {
                let g = &sim.stickies[i];
                eprintln!(
                    "run {} tick {} a ({:.3},{:.3},{:.3}) b ({:.3},{:.3},{:.3}) v ({:.2},{:.2},{:.2}) bounces {} moving {}",
                    r.kind, sim.tick, a.x, a.y, a.z, b.x, b.y, b.z, g.vel.x, g.vel.y, g.vel.z, g.bounces, g.moving
                );
            }
            if r.through.is_some() || !moving && a == b {
                continue;
            }
            live = true;
            if b.y < floors[r.floor].y - 40.0 {
                // Off an edge into the void: park it before the world's floor takes it out of the list.
                sim.stickies[i].moving = false;
                continue;
            }
            let d = b - a;
            let len = d.length();
            if len > 0.0 && d.y < 0.0 && r.crossed.is_none() {
                if let Some((t, n)) = sim.collision.ray(a, d / len, len, Layers::SOLDIER) {
                    if n.y > 0.7 && t < len - 0.02 {
                        r.crossed = Some(a + d / len * t);
                    }
                }
            }
            if let Some(c) = r.crossed.filter(|c| b.y < c.y - GAP_TOL) {
                r.through = Some(c);
                r.gap = ordnance_under(sim, Vec3::new(a.x, c.y + GAP_UP, a.z), GAP_UP + GAP_TOL)
                    .is_none();
                // Park it: it would fall for 9 s and leave the list.
                sim.stickies[i].moving = false;
            }
        }
        if !live {
            break;
        }
    }
    for (i, r) in runs.iter_mut().enumerate() {
        r.end = sim.stickies[i].pos;
        if r.through.is_none() && sim.stickies[i].moving {
            r.through = r.crossed;
        }
    }
    sim.stickies.clear();
    runs
}

struct Report {
    text: String,
    sites: String,
    floors: usize,
    gaps: usize,
    runs: usize,
    through: usize,
    sunk: usize,
    accepted: usize,
    new: usize,
}

/// One line of `ordnance_census_accepted.txt`: `<map|*> <S-|SG|SO|--> <label>`,
/// the label exact or a prefix ending in `*`; `#` starts the reason.
struct Accepted {
    map: String,
    key: String,
}

impl Accepted {
    fn covers(&self, map: &str, key: &str) -> bool {
        (self.map == "*" || self.map == map)
            && match self.key.strip_suffix('*') {
                Some(prefix) => key.starts_with(prefix),
                None => key == self.key,
            }
    }
}

fn accepted_list() -> Vec<Accepted> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/ordnance_census_accepted.txt"
    );
    let text = std::fs::read_to_string(path).expect("ordnance_census_accepted.txt");
    text.lines()
        .filter_map(|l| {
            let l = l.split('#').next()?.trim();
            let (map, key) = l.split_once(char::is_whitespace)?;
            Some(Accepted {
                map: map.into(),
                key: key.split_whitespace().collect::<Vec<_>>().join(" "),
            })
        })
        .collect()
}

fn run_map(
    key: &str,
    path: &Path,
    cell: f32,
    at: Option<Vec3>,
    dynamic: bool,
    accepted: &[Accepted],
) -> Result<Option<Report>, String> {
    let Some((mut w, labels, bf2)) = build_sim(key, path) else {
        return Ok(None);
    };
    let sim = &mut w.sim;
    let wc = sim.add_weapon_class(thermal(bf2));
    let mut text = String::new();
    let mut sites = String::new();
    let mut floors: Vec<Vec3> = Vec::new();
    if let Some(p) = at {
        let _ = writeln!(text, "{key} at ({:.2}, {:.2}, {:.2})", p.x, p.y, p.z);
        for g in levels(sim, p.x, p.z) {
            let f = Vec3::new(p.x, g, p.z);
            let o = ordnance_floor(sim, f);
            let _ = writeln!(
                text,
                "  soldier floor y {g:8.2}  headroom {}  ordnance {}",
                headroom(sim, f),
                o.map_or("NONE".into(), |(y, c)| format!("{y:.2} ({c})"))
            );
            bodies_at(sim, &labels, f, &mut text);
        }
        let g = sim.ground_under(p + Vec3::Y * 0.3, false);
        floors.push(Vec3::new(p.x, g, p.z));
    } else {
        // The hubs' box (seven BF2 maps have no heightfield).
        let Some(nav) = sim.nav.as_ref() else {
            return Ok(None);
        };
        let (mut x0, mut z0, mut x1, mut z1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for h in &nav.hubs {
            x0 = x0.min(h.pos.x - h.radius);
            z0 = z0.min(h.pos.z - h.radius);
            x1 = x1.max(h.pos.x + h.radius);
            z1 = z1.max(h.pos.z + h.radius);
        }
        if nav.hubs.is_empty() {
            return Ok(None);
        }
        let (x0, z0, n) =
            ordnance_grid::bounds(x0, z0, x1, z1, cell).map_err(|e| format!("{key}: {e}"))?;
        let mut off_band: Vec<Vec3> = Vec::new();
        for iz in 0..=n {
            for ix in 0..=n {
                let (x, z) = (x0 + ix as f32 * cell, z0 + iz as f32 * cell);
                for g in levels(sim, x, z) {
                    let f = Vec3::new(x, g, z);
                    if !on_graph(sim, f)
                        || !headroom(sim, f)
                        || sim.death_regions.iter().any(|r| r.contains(f))
                    {
                        continue;
                    }
                    if nearest_hub_dy(sim, f) <= HUB_BAND {
                        floors.push(f);
                    } else {
                        off_band.push(f);
                    }
                }
            }
        }
        // A map whose hubs' authored heights are not its floors' (BF2 Death Star, Polis Massa): every level.
        if floors.len() * 20 < off_band.len() {
            let _ = writeln!(
                text,
                "{key:10} (no hub band: {} of {} levels within it)",
                floors.len(),
                floors.len() + off_band.len()
            );
            floors.append(&mut off_band);
        }
    }
    let mut gaps = 0usize;
    let mut gap_by: std::collections::BTreeMap<String, (usize, Vec3)> = Default::default();
    let mut is_gap = vec![false; floors.len()];
    for (i, f) in floors.iter().enumerate() {
        if ordnance_floor(sim, *f).is_none() {
            gaps += 1;
            is_gap[i] = true;
            let e = gap_by.entry(suspect(sim, &labels, *f)).or_insert((0, *f));
            e.0 += 1;
        }
    }
    let runs = if dynamic {
        fly(sim, wc, &floors, bf2)
    } else {
        Vec::new()
    };
    let mut through = 0usize;
    let sunk = runs
        .iter()
        .filter(|r| r.through.is_none() && r.crossed.is_some())
        .count();
    let mut tf = vec![0u8; floors.len()];
    for r in &runs {
        if let Some(p) = r.through {
            through += 1;
            tf[r.floor] += 1;
            if at.is_some() {
                let _ = writeln!(
                    text,
                    "  run {} THROUGH at ({:.2}, {:.2}, {:.2})",
                    r.kind, p.x, p.y, p.z
                );
            }
        } else if at.is_some() {
            let _ = writeln!(
                text,
                "  run {} {}, ends ({:.2}, {:.2}, {:.2})",
                r.kind,
                if r.crossed.is_some() { "sunk" } else { "ok" },
                r.end.x,
                r.end.y,
                r.end.z
            );
        }
    }
    let through_floors = tf.iter().filter(|n| **n > 0).count();
    for (i, f) in floors.iter().enumerate() {
        if tf[i] > 0 || is_gap[i] {
            let _ = writeln!(
                sites,
                "{key} {:.2} {:.2} {:.2} gap {} through {} {}",
                f.x,
                f.y,
                f.z,
                u8::from(is_gap[i]),
                tf[i],
                suspect(sim, &labels, *f)
            );
        }
    }
    // The body each through run crossed, accepted or new.
    let mut through_by: std::collections::BTreeMap<String, (usize, Vec3)> = Default::default();
    for r in &runs {
        let Some(p) = r.through else {
            continue;
        };
        let mut s = suspect(sim, &labels, p);
        if r.gap && s.starts_with("SO ") {
            s.replace_range(..2, "SG");
        }
        through_by.entry(s).or_insert((0, p)).0 += 1;
    }
    let (mut acc, mut new) = (0usize, 0usize);
    let mut rank: Vec<_> = through_by.into_iter().collect();
    rank.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    let _ = writeln!(
        text,
        "{key:10} floors {:6}  gaps {gaps:5}  runs {:6}  through {through:5}  sunk {sunk:4}  floors through {through_floors:5}",
        floors.len(),
        runs.len()
    );
    for (rank_i, (s, (n, p))) in rank.iter().enumerate() {
        let ok = accepted.iter().any(|a| a.covers(key, s));
        if ok {
            acc += n;
        } else {
            new += n;
        }
        if rank_i < 10 || !ok {
            let _ = writeln!(
                text,
                "    through {n:5}  {}  {s}  e.g. ({:.1}, {:.1}, {:.1})",
                if ok { "accepted" } else { "NEW     " },
                p.x,
                p.y,
                p.z
            );
        }
    }
    let mut rank: Vec<_> = gap_by.into_iter().collect();
    rank.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    for (s, (n, p)) in rank.iter().take(6) {
        let _ = writeln!(
            text,
            "    gap     {n:5}  {s}  e.g. ({:.1}, {:.1}, {:.1})",
            p.x, p.y, p.z
        );
    }
    Ok(Some(Report {
        text,
        sites,
        floors: floors.len(),
        gaps,
        runs: runs.len(),
        through,
        sunk,
        accepted: acc,
        new,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let want: Vec<String> = std::env::args().skip(1).collect();
    let dir = game::assets::baked_root();
    let maps: Vec<(String, String, std::path::PathBuf)> = game::assets::discover_maps(&dir)
        .into_iter()
        .filter(|(k, _, _)| want.is_empty() || want.iter().any(|w| w == k))
        .collect();
    let cell_env = std::env::var("ORDN_CELL");
    let cell = ordnance_grid::parse_cell(match &cell_env {
        Ok(value) => Some(value.as_str()),
        Err(std::env::VarError::NotPresent) => None,
        Err(e) => return Err(format!("ORDN_CELL: {e}").into()),
    })?;
    let at = std::env::var("ORDN_AT").ok().and_then(|v| {
        let p: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        (p.len() == 3).then(|| Vec3::new(p[0], p[1], p[2]))
    });
    let dynamic = std::env::var_os("ORDN_STATIC").is_none();
    let accepted = accepted_list();
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Result<Option<Report>, String>>> =
        Mutex::new((0..maps.len()).map(|_| Ok(None)).collect());
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(16));
    std::thread::scope(|sc| {
        for _ in 0..threads.min(maps.len().max(1)) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= maps.len() {
                    break;
                }
                let (key, _, path) = &maps[i];
                let r = run_map(key, path, cell, at, dynamic, &accepted);
                out.lock().unwrap()[i] = r;
            });
        }
    });
    let out = out.into_inner().unwrap();
    let (mut f, mut g, mut r, mut t, mut k, mut acc, mut new) = (0, 0, 0, 0, 0, 0, 0);
    let mut sites = String::new();
    for (i, rep) in out.iter().enumerate() {
        let rep = rep.as_ref().map_err(|e| e.as_str())?;
        let Some(rep) = rep else {
            println!("{:10} LOAD FAILED", maps[i].0);
            continue;
        };
        print!("{}", rep.text);
        sites.push_str(&rep.sites);
        f += rep.floors;
        g += rep.gaps;
        r += rep.runs;
        t += rep.through;
        k += rep.sunk;
        acc += rep.accepted;
        new += rep.new;
    }
    if let Some(p) = std::env::var_os("ORDN_SITES") {
        std::fs::write(&p, &sites).expect("ORDN_SITES");
    }
    println!(
        "ordnance: maps {}  floors {f}  gaps {g}  runs {r}  through {t}  sunk {k}  accepted {acc}  new {new}",
        maps.len()
    );
    Ok(())
}
