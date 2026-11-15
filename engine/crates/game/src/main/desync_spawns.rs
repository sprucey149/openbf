//! `game --desync-spawns FILE` (issue #173 item 6, a player spawned inside a
//! wall on BF1 Bespin Platforms): re-simulates a LAN desync report's round
//! from its setup and bundles, as `--desync-report` does, and logs every
//! human spawn with a wall probe on the ticks after it.
//!
//!   --desync-spawns FILE [--assets DIR] [--missions DIR] [--probe N]
//!
//! For each spawn of a human slot (a slot's unit alive this tick and dead,
//! or another unit, the tick before): the tick, slot, team, class, the
//! spawn point the sim drew (`UnitSpec::pos`; the post and node index it is,
//! or `team list` / `walker` / `?`), the slot's chosen post, the floor and
//! the body's position. Then, at the node's spawn floor and at the body's
//! pose on each of the first `N` ticks (default 60, 2 s): the soldier wall
//! pass's push out of a standing pose there and how many of eight
//! directions a wall shuts at once (`spawn_node_census`'s probe), the XZ
//! step moved since the tick before and the slot's stick. `INSIDE` marks a
//! pose past 5 cm of push or 3 shut sides; `PUSHED` a tick before the slot's
//! first stick input that moved the body over 5 cm in XZ (a body spawned at
//! rest moves only when pushed), `SHOVED` the same with another live unit
//! within 1.5 m (the soldiers' shove, not a wall). A spawn prints one summary line under it;
//! `--verbose` prints every probed tick, and a flagged tick always prints.
//!
//! The re-simulation's rows are compared with both sides' rows in the
//! report: the line `trust` says which side this build agrees with and the
//! first tick the two sides differ. A spawn at or after that tick is marked
//! `after-divergence` (the side that differs saw its own world from there).
//! The probes read the sim and change nothing in it.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use anyhow::{bail, Context, Result};
use core::unit::SpawnPost;
use game::netplay::desync::{self, Row};
use game::netplay::DesyncReport;
use glam::Vec3;
use std::path::PathBuf;

/// The census's limits for a pose inside a wall (m, sides of eight).
const INSIDE_PUSH: f32 = 0.05;
const INSIDE_SHUT: usize = 3;
/// A step this long with the stick at rest is a push (m).
const PUSHED_STEP: f32 = 0.05;
/// A push with another live unit this close is a shove between soldiers,
/// not a wall (m; two soldier radii and a margin).
const SHOVE_REACH: f32 = 1.5;

/// The wall pass's push out of a standing soldier at `feet`, and the shut
/// sides of eight (a 0.5 m step meeting a wall within 5 cm), as
/// `spawn_node_census::embedded`.
fn embedded(sim: &core::sim::Sim, feet: Vec3) -> (f32, usize) {
    let cap = core::collision::Capsule {
        base: feet,
        height: core::unit::capsule_height(core::soldier::Stance::Stand),
        radius: core::unit::SOLDIER_RADIUS,
    };
    let push = sim.collision.push_out_soldier(&cap).length();
    let stepped = cap.stepped();
    let shut = (0..8)
        .filter(|k| {
            let a = *k as f32 * std::f32::consts::FRAC_PI_4;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            sim.collision
                .wall_sweep(&stepped, d, 0.5)
                .is_some_and(|(t, _)| t < 0.05)
        })
        .count();
    (push, shut)
}

fn inside(p: (f32, usize)) -> bool {
    p.0 > INSIDE_PUSH || p.1 >= INSIDE_SHUT
}

/// Which authored point `node` is: a post's node, the team's flat list
/// (the opening wave), or neither.
fn source_of(sim: &core::sim::Sim, node: Vec3, yaw: f32) -> String {
    for c in &sim.cps {
        if let Some(i) = c
            .def
            .spawn_nodes
            .iter()
            .position(|&(p, y)| p == node && y == yaw)
        {
            return format!("{} node {i}", c.def.name);
        }
    }
    for c in &sim.cps {
        if let Some(i) = c
            .def
            .spawn_nodes
            .iter()
            .position(|&(p, _)| p.x == node.x && p.z == node.z)
        {
            return format!("{} node {i} (yaw differs)", c.def.name);
        }
    }
    "? (no post node)".to_owned()
}

fn post_name(sim: &core::sim::Sim, p: Option<SpawnPost>) -> String {
    match p {
        Some(SpawnPost::Post(i)) => sim
            .cps
            .get(usize::from(i))
            .map_or_else(|| format!("post {i}"), |c| c.def.name.clone()),
        Some(SpawnPost::Walker(h)) => format!("walker {}", h.index),
        None => "-".to_owned(),
    }
}

/// One spawn being probed.
struct Watch {
    slot: usize,
    handle: core::unit::UnitHandle,
    tick: u32,
    left: u32,
    probed: u32,
    last: Vec3,
    start: Vec3,
    /// The first tick the slot's stick left rest.
    stick_at: Option<u32>,
    max_push: f32,
    max_shut: usize,
    flags: Vec<&'static str>,
    lines: Vec<String>,
}

pub(super) fn run(args: &[String]) -> Result<()> {
    let mut file = None;
    let mut assets = PathBuf::from("assets_baked");
    let mut missions = PathBuf::from("missions");
    let mut probe: u32 = 60;
    let mut verbose = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--assets" => assets = it.next().context("--assets DIR")?.into(),
            "--missions" => missions = it.next().context("--missions DIR")?.into(),
            "--probe" => probe = it.next().context("--probe N")?.parse()?,
            "--verbose" => verbose = true,
            f if !f.starts_with("--") && file.is_none() => file = Some(PathBuf::from(f)),
            other => bail!("unknown --desync-spawns argument: {other}"),
        }
    }
    let file = file.context("--desync-spawns FILE [--assets DIR] [--missions DIR] [--probe N]")?;
    let r = DesyncReport::read(&file).with_context(|| file.display().to_string())?;
    anyhow::ensure!(
        !r.bundles_broken,
        "the report's bundle log is not contiguous from tick 0"
    );
    let hr = desync::parse_rows(&r.host.rows)?;
    let gr = desync::parse_rows(&r.guest.rows)?;
    let parted = desync::first_difference(&hr, &gr).map(|d| d.0);
    let setup = r.round_setup()?;
    println!(
        "desync-spawns: {} ({}), report by the {} (local slot {}), guest slot {}, check at tick {}",
        r.session, r.setup_summary, r.side, r.local_slot, r.slot, r.tick
    );
    let humans: Vec<String> = setup
        .humans
        .iter()
        .map(|h| format!("slot {} {:?} team {:?}", h.slot, h.name, h.team))
        .collect();
    println!("humans at the start: {}", humans.join(", "));
    let (scene, load) = game::assets::load_battle_scene(&assets.join(&setup.map), &setup.mode)
        .with_context(|| format!("loading {} from {}", setup.map, assets.display()))?;
    if let Some((path, Err(error))) = load.common {
        bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let mut walk =
        super::round_setup::build_round(&scene, &setup, &missions, &assets, r.local_slot)?;
    let mut feed =
        game::event_feed::EventFeed::for_walk(&assets, &scene.manifest.source_game, &walk);
    let digest = format!("{:016x}", walk.sim.static_digest());
    println!(
        "static digest {digest} ({})",
        if digest == r.static_digest {
            "the report's".to_owned()
        } else {
            format!("WARNING: the report's is {}", r.static_digest)
        }
    );
    let bundles = r.all_bundles()?;
    let mut prev: Vec<Option<(core::unit::UnitHandle, bool)>> = Vec::new();
    let mut watches: Vec<Watch> = Vec::new();
    let mut done: Vec<Watch> = Vec::new();
    let mut agree = [(0u32, None::<u32>); 2];
    for (i, b) in bundles.iter().enumerate() {
        anyhow::ensure!(b.tick == i as u32, "bundle {i} says tick {}", b.tick);
        game::netplay::step(&mut walk, b, Some(&mut feed), |_, _| {});
        let sim = &walk.sim;
        let mine: Row = desync::row_of(sim);
        for (k, rows) in [&hr, &gr].into_iter().enumerate() {
            if let Some((_, theirs)) = rows.iter().find(|x| x.0 == b.tick) {
                agree[k].0 += 1;
                if agree[k].1.is_none() && *theirs != mine {
                    agree[k].1 = Some(b.tick);
                }
            }
        }
        // Probe the spawns being watched.
        for w in &mut watches {
            let Some(u) = sim.unit(w.handle).filter(|u| u.alive) else {
                w.left = 0;
                w.lines.push("      (died)".to_owned());
                continue;
            };
            let at = u.soldier.pos;
            let p = embedded(sim, at);
            let step = glam::Vec2::new(at.x - w.last.x, at.z - w.last.z).length();
            let pad = b.pad(w.slot as u8).copied().unwrap_or_default();
            let stick = (pad.lx, pad.ly);
            // The nearest other live unit (a soldier the body is shoved by).
            let near = sim
                .alive_units()
                .filter(|(o, _)| *o != w.handle)
                .map(|(_, o)| (o.soldier.pos - at).length())
                .fold(f32::INFINITY, f32::min);
            let rest = stick.0.unsigned_abs() < 2000 && stick.1.unsigned_abs() < 2000;
            if !rest && w.stick_at.is_none() {
                w.stick_at = Some(b.tick);
            }
            let mut mark = String::new();
            if inside(p) {
                mark.push_str(" INSIDE");
                w.flags.push("inside");
            }
            if w.stick_at.is_none() && step > PUSHED_STEP {
                if near < SHOVE_REACH {
                    mark.push_str(" SHOVED");
                    w.flags.push("shoved-by-a-unit");
                } else {
                    mark.push_str(" PUSHED");
                    w.flags.push("pushed");
                }
            }
            if verbose || !mark.is_empty() {
                w.lines.push(format!(
                    "      t{} ({:.2}, {:.2}, {:.2}) push {:.2} shut {}/8 step {step:.2} stick ({}, {}) buttons {:#x} nearest unit {near:.2} m{mark}",
                    b.tick, at.x, at.y, at.z, p.0, p.1, stick.0, stick.1, pad.buttons
                ));
            }
            w.max_push = w.max_push.max(p.0);
            w.max_shut = w.max_shut.max(p.1);
            w.probed += 1;
            w.last = at;
            w.left -= 1;
        }
        let (ended, going): (Vec<Watch>, Vec<Watch>) =
            watches.into_iter().partition(|w| w.left == 0);
        watches = going;
        done.extend(ended);
        // New spawns this tick.
        prev.resize(sim.players.len(), None);
        for (s, h) in sim.players.iter().enumerate() {
            let now = h.and_then(|h| sim.unit(h).map(|u| (h, u.alive)));
            let was = prev[s];
            prev[s] = now;
            let Some((h, true)) = now else {
                continue;
            };
            if was.is_some_and(|(ph, alive)| ph == h && alive) {
                continue;
            }
            let Some(u) = sim.unit(h) else {
                continue;
            };
            let node = u.spec.pos;
            let floor = sim.spawn_ground(node);
            let at_node = embedded(sim, Vec3::new(node.x, floor, node.z));
            let p = embedded(sim, u.soldier.pos);
            let team = usize::from(u.team.min(1));
            let class = walk.teams[team]
                .classes
                .get(usize::from(u.spec.class))
                .map_or_else(|| format!("class {}", u.spec.class), |c| c.name.clone());
            let mut flags = Vec::new();
            if inside(at_node) {
                flags.push("node-inside");
            }
            if inside(p) {
                flags.push("inside");
            }
            let trust = match parted {
                Some(t) if b.tick >= t => " after-divergence",
                _ => "",
            };
            let head = format!(
                "t{} slot {s} team {} {class}: node ({:.2}, {:.2}, {:.2}) yaw {:.2} = {}; chosen post {}; floor {floor:.2}; node probe push {:.2} shut {}/8; body ({:.2}, {:.2}, {:.2}) push {:.2} shut {}/8{trust}",
                b.tick,
                u.team,
                node.x,
                node.y,
                node.z,
                u.spec.yaw,
                source_of(sim, node, u.spec.yaw),
                post_name(sim, sim.spawn_post_choice.get(s).copied().flatten()),
                at_node.0,
                at_node.1,
                u.soldier.pos.x,
                u.soldier.pos.y,
                u.soldier.pos.z,
                p.0,
                p.1,
            );
            watches.push(Watch {
                slot: s,
                handle: h,
                tick: b.tick,
                left: probe,
                probed: 0,
                last: u.soldier.pos,
                start: u.soldier.pos,
                stick_at: None,
                max_push: p.0,
                max_shut: p.1,
                flags,
                lines: vec![head],
            });
        }
    }
    done.extend(watches);
    let say = |k: usize| match agree[k] {
        (0, _) => "no rows".to_owned(),
        (n, None) => format!("agrees ({n} rows)"),
        (_, Some(t)) => format!("differs from tick {t}"),
    };
    println!(
        "trust: {} bundles stepped; this build vs the host's rows {}, the guest's {}; the two sides first differ at {}",
        bundles.len(),
        say(0),
        say(1),
        parted.map_or("no tick in the rows".to_owned(), |t| format!("tick {t}"))
    );
    done.sort_by_key(|w| (w.tick, w.slot));
    let mut flagged = 0;
    for w in &done {
        let mut f = w.flags.clone();
        f.dedup();
        if !f.is_empty() {
            flagged += 1;
        }
        println!(
            "SPAWN {}{}",
            w.lines[0],
            if f.is_empty() {
                " -- clear".to_owned()
            } else {
                format!(" -- FLAG {}", f.join(","))
            }
        );
        for l in &w.lines[1..] {
            println!("{l}");
        }
        println!(
            "      {} ticks probed: max push {:.2}, max shut {}/8, first stick {}, {:.1} m from the spawn at the end",
            w.probed,
            w.max_push,
            w.max_shut,
            w.stick_at
                .map_or("none".to_owned(), |t| format!("t{t} (+{})", t - w.tick)),
            (w.last - w.start).length()
        );
    }
    println!(
        "desync-spawns: {} human spawns, {flagged} flagged",
        done.len()
    );
    Ok(())
}
