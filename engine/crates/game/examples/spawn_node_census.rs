//! Card SPNC: every spawn node of every post on the baked maps against the
//! first leg a bot spawned there plans.
//!
//! BF2 Dagobah's cave post has a node on the ground OVER the cave; before
//! card DAGS the first leg after a spawn started at the bare XZ locate, which
//! put the bot on the cave's arc 8 m under its feet. This asks the same
//! question at every node, so the rule is checked everywhere and not found
//! one playtest at a time.
//!
//! Each map's sim is built the way the round sweep builds it (mode layer,
//! common props, the mission's route blocks, barriers and death regions, the
//! posts with their resolved floors and spawn seats; no vehicles). The
//! player's unit is planted on each node's spawn floor (`Sim::spawn_ground`)
//! and asked, for every other post with a hub, the first leg
//! (`core::ai::first_leg`) from today's start hub (`start_hub`, AFTER) and
//! from the bare XZ locate the first leg took before card DAGS (BEFORE).
//!
//! A hub is REACHED when the floor walked from the feet straight at its XZ
//! (`core::ai::floor_walked`, the post point's walk: a step at a time, no
//! climb over a step, no drop past `LEDGE_DROP`) arrives within
//! `HUB_LEVEL_TOL` of its resolved floor (`ok`), else when a soldier's floor
//! flood from the feet (`Sim::floor_flood`, card LLEG's pen flood: round
//! walls, no drop past `PEN_DROP`) gets inside the hub's disc at that floor
//! (`fl`). The straight walk alone called 20 nodes failing whose bots walk
//! off as well as any other node's (main fdebf98, 16 rounds: a trunk or a
//! log on the line). A leg FAILS when neither its start hub nor its next hub
//! is reached (`NO`): the bot can walk onto neither end of it. A node fails
//! when its leg to any goal fails. A hub with no resolved floor is `?` and is
//! not counted as a failure.
//!
//! Also printed per node: its floor, the level test there (`off` = given up
//! as lost from the first tick, `Sim::level_at`) and the first wall a
//! soldier's stepped capsule meets on the straight line to the start hub.
//! Walls do not make a node fail: the steering walks round them.
//!
//! ```text
//! cargo run --release -p game --example spawn_node_census [map ...]
//! ```
//! One line per map, then each failing node (`SPNC_ALL=1`: every node) with
//! post, position and every goal's leg, then every failing node that no line
//! of `spawn_node_census_accepted.txt` beside this file covers (`NEW`), then
//! the total with the accepted and new counts. Exit 0.
//! `SPNC_RULE=before` prints the BEFORE column's failures instead.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::ai::{first_leg, floor_walked, HUB_LEVEL_TOL};
use core::sim::Sim;
use glam::{Vec2, Vec3};
use std::fmt::Write as _;
use std::path::Path;

fn mission_for(key: &str) -> Option<game::mission::MissionDef> {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let path = dir.join(format!("{key}_con.ron"));
    path.exists()
        .then(|| game::mission::MissionDef::load(&path).ok())
        .flatten()
}

/// The map's sim with its posts in, as the sweep builds it before the
/// opening wave; `None` with the reason when it will not load.
fn build_sim(path: &Path) -> Result<game::walk::WalkSim, String> {
    let (scene, _) =
        game::assets::load_battle_scene(path, "con").map_err(|e| format!("LOAD FAILED {e:#}"))?;
    let key = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
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
    let mut teams = factions.clone().map(|faction| game::walk::TeamTemplate {
        faction,
        ..game::walk::TeamTemplate::default_trooper()
    });
    if let Ok((rs, _)) = game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    ) {
        for (t, r) in teams.iter_mut().zip(rs.iter()) {
            let f = r.fill_class();
            t.consts = f.consts;
            t.name = f.class.clone();
        }
    }
    let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    if let Some(m) = &mission {
        m.apply_arc_blocks(nav.as_mut());
        m.apply_barriers(nav.as_mut());
    }
    let mut sim = Sim::empty(1);
    scene.install_water(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    let mut w = game::walk::WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, teams, nav);
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
    }
    w.add_posts(&layout);
    Ok(w)
}

/// The cells [`Sim::floor_flood`] may count before a hub is called out of
/// reach: a disc of about 100 m round the node.
const FLOOD_CAP: usize = 50_000;
/// A flood walk longer than this times the straight distance, plus
/// [`DETOUR_SLACK`], is a detour a bot steering at the hub does not find.
/// The flood walks a grid in four directions (up to 1.41 times a straight
/// line). PROVISIONAL (ours, card SPNC).
const DETOUR_RATIO: f32 = 2.0;
/// See [`DETOUR_RATIO`] (m).
const DETOUR_SLACK: f32 = 10.0;

/// How hub `h` is reached from `feet`, `None` when it has no resolved
/// floor.
#[derive(Clone, Copy)]
struct Reach {
    /// The floor the straight walk arrived at.
    walked: Option<f32>,
    /// It arrived within `HUB_LEVEL_TOL` of the hub's floor.
    straight: bool,
    /// The flood's shortest walk into the hub's disc at its floor (m), when
    /// the straight walk did not arrive; `None` when it never got there.
    flood: Option<f32>,
    /// The XZ distance to the hub's centre (m).
    dist: f32,
}

impl Reach {
    /// Reached: the straight walk, or a flood walk no longer than
    /// [`DETOUR_RATIO`] times the distance plus [`DETOUR_SLACK`].
    fn ok(&self) -> bool {
        self.straight
            || self
                .flood
                .is_some_and(|m| m <= DETOUR_RATIO * self.dist + DETOUR_SLACK)
    }

    fn mark(&self) -> String {
        match (self.straight, self.flood) {
            (true, _) => "ok".to_owned(),
            (false, Some(m)) => format!(
                "{}{m:.0}/{:.0}",
                if self.ok() { "fl" } else { "FAR" },
                self.dist
            ),
            (false, None) => "NO".to_owned(),
        }
    }
}

fn reaches(sim: &Sim, feet: Vec3, h: u16) -> Option<Reach> {
    let g = sim.nav.as_ref()?;
    let f = sim.nav_floor(h)?;
    let hub = g.hub(h)?;
    let c = Vec2::new(hub.pos.x, hub.pos.z);
    let walked = floor_walked(sim, feet, c);
    let straight = walked.is_some_and(|y| (y - f).abs() <= HUB_LEVEL_TOL);
    let r = hub.radius.max(1.0);
    let goal =
        |p: Vec3| (Vec2::new(p.x, p.z) - c).length() <= r && (p.y - f).abs() <= HUB_LEVEL_TOL;
    let flood = (!straight)
        .then(|| sim.floor_flood(feet, FLOOD_CAP, &goal).1)
        .flatten()
        .map(|(_, m)| m);
    Some(Reach {
        walked,
        straight,
        flood,
        dist: (c - Vec2::new(feet.x, feet.z)).length(),
    })
}

fn fmt_y(y: Option<f32>) -> String {
    y.map_or("-".to_owned(), |y| format!("{y:.1}"))
}

/// The first wall on the straight line from `feet` to `to` (m), for a
/// soldier's stepped capsule.
fn first_wall(sim: &Sim, feet: Vec3, to: Vec2) -> Option<f32> {
    let d = to - Vec2::new(feet.x, feet.z);
    let len = d.length();
    if len < 0.01 {
        return None;
    }
    let cap = core::collision::Capsule {
        base: feet,
        height: core::unit::capsule_height(core::soldier::Stance::Stand),
        radius: core::unit::SOLDIER_RADIUS,
    }
    .stepped();
    sim.collision
        .wall_sweep(&cap, Vec3::new(d.x, 0.0, d.y) / len, len)
        .map(|(t, _)| t)
}

/// One post as the census reads it.
struct Post {
    name: String,
    hub: Option<u16>,
    /// The starting owner (0-based team).
    owner: Option<u8>,
    pos: Vec3,
    nodes: Vec<Vec3>,
}

struct MapOut {
    key: String,
    /// The nodes failing under today's rule: post name, node index.
    failing: Vec<(String, usize)>,
    line: String,
    detail: String,
    nodes: usize,
    fail: [usize; 2],
}

fn census(key: &str, path: &Path, show_all: bool, rule: usize) -> MapOut {
    let mut w = match build_sim(path) {
        Ok(w) => w,
        Err(e) => {
            return MapOut {
                key: key.to_owned(),
                failing: Vec::new(),
                line: format!("{key:10} {e}"),
                detail: String::new(),
                nodes: 0,
                fail: [0; 2],
            }
        }
    };
    let me = w.player;
    let mut detail = String::new();
    let mut nodes = 0;
    let mut fail = [0usize; 2];
    let mut failing = Vec::new();
    let mut lost = 0;
    let posts: Vec<Post> = w
        .sim
        .cps
        .iter()
        .map(|c| Post {
            name: c.def.name.clone(),
            hub: c.nav_hub,
            owner: c.owner,
            pos: c.def.pos,
            nodes: c.def.spawn_nodes.iter().map(|n| n.0).collect(),
        })
        .collect();
    for (pi, post) in posts.iter().enumerate() {
        let (name, owner) = (&post.name, post.owner);
        for (ni, &node) in post.nodes.iter().enumerate() {
            nodes += 1;
            let feet = Vec3::new(node.x, w.sim.spawn_ground(node), node.z);
            if let Some(u) = w.sim.unit_mut(me) {
                u.soldier.pos = feet;
                u.mount = None;
            }
            let off = w.sim.level_at(feet);
            if off.is_off() {
                lost += 1;
            }
            // Phase A's own goal for the post's team: the nearest post it
            // does not own (by the starting owners).
            let phase_a = owner.and_then(|t| {
                posts
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.owner != Some(t) && p.hub.is_some())
                    .min_by(|a, b| {
                        (a.1.pos - feet)
                            .length_squared()
                            .total_cmp(&(b.1.pos - feet).length_squared())
                    })
                    .map(|(i, _)| i)
            });
            let mut seen: Vec<(u16, Option<Reach>)> = Vec::new();
            let mut reach = |h: u16| {
                if let Some(&(_, r)) = seen.iter().find(|e| e.0 == h) {
                    return r;
                }
                let r = reaches(&w.sim, feet, h);
                seen.push((h, r));
                r
            };
            let mut node_fail = [false; 2];
            let mut rows = String::new();
            let mut starts = [None; 2];
            for (gi, goal) in posts.iter().enumerate() {
                let (gname, Some(dest)) = (&goal.name, goal.hub) else {
                    continue;
                };
                if gi == pi {
                    continue;
                }
                let legs = first_leg(&w.sim, me, dest);
                let mut cols = Vec::new();
                for (k, leg) in legs.iter().enumerate() {
                    let Some((cur, r)) = leg else {
                        cols.push("off the graph".to_owned());
                        continue;
                    };
                    starts[k] = Some(*cur);
                    let rs = reach(*cur);
                    let rn = reach(r.next_hub);
                    let ok = |x: &Option<Reach>| x.map(|x| x.ok());
                    let bad = ok(&rs) == Some(false) && ok(&rn) != Some(true)
                        || ok(&rn) == Some(false) && ok(&rs) != Some(true);
                    if bad {
                        node_fail[k] = true;
                    }
                    cols.push(format!(
                        "{cur:>3} {}({}/{}) -> {:>3} {}({}/{}){}",
                        rs.map_or("?".to_owned(), |x| x.mark()),
                        fmt_y(rs.and_then(|x| x.walked)),
                        fmt_y(w.sim.nav_floor(*cur)),
                        r.next_hub,
                        rn.map_or("?".to_owned(), |x| x.mark()),
                        fmt_y(rn.and_then(|x| x.walked)),
                        fmt_y(w.sim.nav_floor(r.next_hub)),
                        if bad { " FAIL" } else { "" }
                    ));
                }
                let _ = writeln!(
                    rows,
                    "      {}{gname:<14} before {:<44} after {}",
                    if Some(gi) == phase_a { "*" } else { " " },
                    cols.first().map_or("", String::as_str),
                    cols.get(1).map_or("", String::as_str)
                );
            }
            for k in 0..2 {
                fail[k] += usize::from(node_fail[k]);
            }
            if node_fail[1] {
                failing.push((name.clone(), ni));
            }
            if show_all || node_fail[rule] || off.is_off() {
                let wall = starts[1]
                    .and_then(|h| w.sim.nav.as_ref().and_then(|g| g.hub(h)))
                    .and_then(|h| first_wall(&w.sim, feet, Vec2::new(h.pos.x, h.pos.z)));
                let _ = writeln!(
                    detail,
                    "  {key} {name} node {ni} ({:.1}, {:.1}, {:.1}) floor {:.1} level {:?} wall {} starts before {} after {}{}{}",
                    node.x,
                    node.y,
                    node.z,
                    feet.y,
                    off,
                    first_wall_txt(wall),
                    starts[0].map_or("-".to_owned(), |h| h.to_string()),
                    starts[1].map_or("-".to_owned(), |h| h.to_string()),
                    if node_fail[0] { " FAIL-BEFORE" } else { "" },
                    if node_fail[1] { " FAIL-AFTER" } else { "" },
                );
                detail.push_str(&rows);
            }
        }
    }
    MapOut {
        key: key.to_owned(),
        failing,
        line: format!(
            "{key:10} posts {:>2} nodes {nodes:>3} fail before {:>2} after {:>2} level-off {lost}",
            posts.len(),
            fail[0],
            fail[1]
        ),
        detail,
        nodes,
        fail,
    }
}

fn first_wall_txt(t: Option<f32>) -> String {
    t.map_or("none".to_owned(), |t| format!("{t:.1} m"))
}

fn main() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets_baked"));
    let only: Vec<String> = std::env::args().skip(1).collect();
    let show_all = std::env::var_os("SPNC_ALL").is_some();
    let rule = usize::from(std::env::var("SPNC_RULE").as_deref() != Ok("before"));
    let maps: Vec<(String, std::path::PathBuf)> = game::assets::discover_maps(dir)
        .into_iter()
        .filter(|(key, _, _)| only.is_empty() || only.iter().any(|o| o == key))
        .map(|(key, _, path)| (key, path))
        .collect();
    if maps.is_empty() {
        eprintln!("no baked map under {} matches {only:?}", dir.display());
        return;
    }
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, maps.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let outs: Vec<std::sync::Mutex<Option<MapOut>>> =
        maps.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((key, path)) = maps.get(i) else {
                    break;
                };
                let o = census(key, path, show_all, rule);
                if let Ok(mut slot) = outs[i].lock() {
                    *slot = Some(o);
                }
            });
        }
    });
    let outs: Vec<MapOut> = outs
        .into_iter()
        .filter_map(|m| m.into_inner().ok().flatten())
        .collect();
    let (mut nodes, mut fail) = (0, [0; 2]);
    for o in &outs {
        println!("{}", o.line);
        nodes += o.nodes;
        fail[0] += o.fail[0];
        fail[1] += o.fail[1];
    }
    println!();
    println!(
        "{} nodes (rule shown: {})",
        nodes,
        if rule == 1 { "after" } else { "before" }
    );
    for o in &outs {
        print!("{}", o.detail);
    }
    let accepted = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/spawn_node_census_accepted.txt"
    ))
    .unwrap_or_default();
    let accepted: Vec<Vec<&str>> = accepted
        .lines()
        .map(|l| {
            l.split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect()
        })
        .filter(|w: &Vec<&str>| w.len() == 3)
        .collect();
    let mut new = 0;
    for o in &outs {
        for (post, node) in &o.failing {
            let n = node.to_string();
            if !accepted
                .iter()
                .any(|w| w[0] == o.key && w[1] == post && w[2] == n)
            {
                println!("NEW failing node: {} {post} node {node}", o.key);
                new += 1;
            }
        }
    }
    println!(
        "spawn_node_census: {} maps, {nodes} nodes, failing before {} after {}, accepted {} new {new}",
        outs.len(),
        fail[0],
        fail[1],
        fail[1] - new
    );
}
