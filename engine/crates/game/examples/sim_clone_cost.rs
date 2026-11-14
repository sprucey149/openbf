//! Issue #168 (LAN prediction, `analysis/plan/LAN_MULTIPLAYER.md` §6): what
//! a predicted copy of a real round costs.
//!
//! Builds a real baked conquest round (BF2 Geonosis by default) with a human
//! in slot 0 and `--bots N` per side, steps it 300 ticks, then:
//! - times 1,000 `Sim::clone`s (mean and p99, µs) and counts the bytes one
//!   clone allocates (the dynamic state it copies), with a per-field split;
//! - clones at that tick and steps the copy and the original with the same
//!   pads for 300 ticks, comparing `Sim::hash` every tick;
//! - over those ticks, times `Sim::step` against `Sim::step_predicted` from
//!   the same state (two fresh clones per tick).
//!
//! `cargo run --release -p game --example sim_clone_cost -- [map] [--bots N]`
//! from `engine/`, with the baked assets in `assets_baked/`. Headless.

#[path = "support/conquest_battle.rs"]
mod conquest_battle;

use anyhow::Result;
use conquest_battle::{battle, pad};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Counts the bytes allocated, so one clone's copied state can be read off.
struct Counting;
static ALLOCATED: AtomicU64 = AtomicU64::new(0);
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(
            new_size.saturating_sub(layout.size()) as u64,
            Ordering::Relaxed,
        );
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Bytes and allocations `f` makes.
fn allocated<T>(f: impl FnOnce() -> T) -> (T, u64, u64) {
    let (b0, a0) = (
        ALLOCATED.load(Ordering::Relaxed),
        ALLOCS.load(Ordering::Relaxed),
    );
    let out = f();
    (
        out,
        ALLOCATED.load(Ordering::Relaxed) - b0,
        ALLOCS.load(Ordering::Relaxed) - a0,
    )
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

fn stats(name: &str, mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "{name}: n {} mean {mean:.1} µs  p50 {:.1}  p99 {:.1}  max {:.1}",
        v.len(),
        pct(&v, 0.5),
        pct(&v, 0.99),
        v[v.len() - 1]
    );
    mean
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let key = args
        .iter()
        .find(|a| !a.starts_with("--") && a.parse::<u32>().is_err())
        .cloned()
        .unwrap_or_else(|| "bf2_geo1".into());
    let bots = args
        .iter()
        .position(|a| a == "--bots")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(32u32);
    let root = Path::new("assets_baked");
    let t0 = Instant::now();
    let mut walk = battle(root, &key, bots)?;
    println!(
        "{key}: built in {:.1} s, {} units, {} vehicles, {} bodies",
        t0.elapsed().as_secs_f32(),
        walk.sim.units.len(),
        walk.sim.vehicles.len(),
        walk.sim.collision.bodies().len()
    );
    let warm = args
        .iter()
        .position(|a| a == "--warm")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(300u64);
    for t in 0..warm {
        walk.sim.step(&[pad(t)]);
    }
    let sim = &walk.sim;
    let alive = sim.units.iter().filter(|u| u.alive).count();
    let ai = sim
        .units
        .iter()
        .filter(|u| u.alive && u.controller == core::unit::Controller::Ai)
        .count();
    println!("tick {}: {alive} alive ({ai} bots)", sim.tick);

    // `--wall-census`: what each soldier's wall pass costs at this tick (a
    // rising ceiling sweep and the push-out, as the movement step asks
    // them), dearest first: a few soldiers inside dense meshes can be most
    // of a tick.
    if args.iter().any(|a| a == "--wall-census") {
        let mut rows = Vec::new();
        for (i, u) in sim.units.iter().enumerate() {
            if !u.alive || u.mount.is_some() {
                continue;
            }
            let cap = u.capsule();
            let up = core::collision::Capsule {
                base: cap.base + glam::Vec3::Y * 0.1,
                ..cap
            };
            let t = Instant::now();
            for _ in 0..20 {
                std::hint::black_box(sim.collision.ceiling_sweep(&cap, &up));
            }
            let ceiling = t.elapsed().as_secs_f64() * 1e6 / 20.0;
            let t = Instant::now();
            for _ in 0..20 {
                std::hint::black_box(sim.collision.push_out_soldier_from(&cap, &cap));
            }
            let push = t.elapsed().as_secs_f64() * 1e6 / 20.0;
            rows.push((
                ceiling + push,
                ceiling,
                push,
                i,
                u.soldier.pos,
                u.soldier.stance,
                u.controller,
            ));
        }
        rows.sort_by(|a, b| b.0.total_cmp(&a.0));
        let total: f64 = rows.iter().map(|r| r.0).sum();
        println!("wall census: {} soldiers, {total:.0} µs in all", rows.len());
        for r in rows.iter().take(8) {
            println!(
                "  unit {:>3} {:?} {:?} at ({:.1}, {:.1}, {:.1}): ceiling {:.0} µs, push-out {:.0} µs",
                r.3, r.6, r.5, r.4.x, r.4.y, r.4.z, r.1, r.2
            );
        }
        return Ok(());
    }

    // `--profile-predicted N` / `--profile-step N`: just N predicted (or
    // full) steps from this state, each on a fresh clone, for a profiler.
    for (flag, predicted) in [("--profile-predicted", true), ("--profile-step", false)] {
        if let Some(n) = args
            .iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .and_then(|n| n.parse::<u32>().ok())
        {
            let t = Instant::now();
            for k in 0..n {
                let mut c = sim.clone();
                for j in 0..4 {
                    let p = [pad(warm + u64::from(k) + j)];
                    if predicted {
                        c.step_predicted(&p);
                    } else {
                        c.step(&p);
                    }
                }
            }
            println!(
                "{flag}: {:.1} µs per step",
                t.elapsed().as_secs_f64() * 1e6 / f64::from(n * 4)
            );
            return Ok(());
        }
    }

    // 1. Clone cost.
    let (copy, bytes, allocs) = allocated(|| sim.clone());
    drop(copy);
    println!("one clone allocates {bytes} bytes in {allocs} allocations");
    macro_rules! field {
        ($($f:ident),*) => {{
            let mut rows: Vec<(&str, u64, f64)> = vec![$({
                let (_, b, _) = allocated(|| sim.$f.clone());
                let t = Instant::now();
                for _ in 0..100 {
                    std::hint::black_box(sim.$f.clone());
                }
                (stringify!($f), b, t.elapsed().as_secs_f64() * 1e6 / 100.0)
            }),*];
            rows.sort_by(|a, b| b.2.total_cmp(&a.2));
            for (name, b, us) in rows.iter().take(14) {
                println!("  field {name:<22} {b:>9} B {us:>8.2} µs");
            }
        }};
    }
    field!(
        units,
        vehicles,
        collision,
        heightfield,
        nav,
        positions,
        flank,
        classes,
        weapon_classes,
        nav_floors,
        nav_unwalkable,
        nav_trapped,
        spawn_seats,
        unit_grid,
        cps,
        bolts,
        commander,
        arc_fails,
        events,
        hull_parts,
        station_wreck_collision,
        station_live_collision,
        boundary,
        damage_regions,
        death_regions,
        item_classes,
        station_classes,
        water_mask,
        flyer_paths,
        missile_target_points,
        vehicle_pads,
        waves,
        flyer_params,
        walker_params,
        hover_params,
        seat_setup,
        blast_setup,
        landing_regions,
        doors,
        stations,
        building_turrets
    );
    // What a step that moves a map body pays once per copy when the body
    // list is shared (copy-on-write): the list's deep copy.
    {
        let t = Instant::now();
        for _ in 0..100 {
            std::hint::black_box(sim.collision.bodies().to_vec());
        }
        println!(
            "  body list deep copy ({} bodies): {:.2} µs",
            sim.collision.bodies().len(),
            t.elapsed().as_secs_f64() * 1e6 / 100.0
        );
    }
    // And what one turning station costs a fresh copy: its bodies' moves
    // (the copy-on-write above, and a broad-phase rebuild if a body left
    // its mover envelope), and whether the grid is still shared after.
    if let Some(parts) = sim.station_bodies.iter().find(|p| !p.is_empty()) {
        let mut v = Vec::new();
        let mut grid_kept = 0;
        for _ in 0..100 {
            let mut c = sim.collision.clone();
            let t = Instant::now();
            for &(i, _) in parts {
                let to = c.bodies()[i].to_world;
                c.move_body(i, to);
            }
            v.push(t.elapsed().as_secs_f64() * 1e6);
            grid_kept += usize::from(c.grid_shared_with(&sim.collision));
            std::hint::black_box(c);
        }
        stats(
            &format!("one station's {} body moves on a fresh copy", parts.len()),
            v,
        );
        println!("  broad phase still shared after them: {grid_kept}/100");
    }
    let mut clone_us = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let t = Instant::now();
        let c = sim.clone();
        clone_us.push(t.elapsed().as_secs_f64() * 1e6);
        drop(c);
    }
    stats("clone", clone_us);

    // 2. A clone steps as the original does, and step vs step_predicted.
    let mut a = sim.clone();
    let mut b = sim.clone();
    let mut step_us = Vec::new();
    let mut pred_us = Vec::new();
    let mut same = 0;
    // Ticks whose step moved a map body (a turning station, a door): the
    // stepping copy then holds its own bodies (copy-on-write).
    let mut unshared = 0;
    for k in 0..300u64 {
        let p = [pad(warm + k)];
        {
            let mut c = a.clone();
            let t = Instant::now();
            c.step(&p);
            step_us.push(t.elapsed().as_secs_f64() * 1e6);
            unshared += usize::from(!c.collision.shares_static_with(&a.collision));
            let mut d = a.clone();
            let t = Instant::now();
            d.step_predicted(&p);
            pred_us.push(t.elapsed().as_secs_f64() * 1e6);
        }
        a.step(&p);
        b.step(&p);
        anyhow::ensure!(
            a.hash() == b.hash(),
            "clone diverged at tick {} after {k} steps",
            a.tick
        );
        same += 1;
    }
    println!(
        "clone stepped identically: {same}/300 ticks, hash {:016x}",
        a.hash()
    );
    // Stepping never wrote the shared map data: the stepped copies still
    // share it with the sim they were cloned from.
    let nav_shared = match (&a.nav, &sim.nav) {
        (Some(x), Some(y)) => x.shares_tables_with(y),
        (None, None) => true,
        _ => false,
    };
    println!("ticks whose step moved a map body: {unshared}/300");
    println!(
        "after 300 steps the copy shares: collision {} nav {} positions {}",
        a.collision.shares_static_with(&sim.collision),
        nav_shared,
        a.positions.shares_with(&sim.positions)
    );
    // A LAN frame's prediction: a clone and 2 predicted ticks; the cap: 8.
    for depth in [2u64, 8] {
        let mut v = Vec::new();
        let (mut first, mut later) = (Vec::new(), Vec::new());
        for k in 0..100u64 {
            let t = Instant::now();
            let mut c = a.clone();
            for j in 0..depth {
                let s0 = Instant::now();
                c.step_predicted(&[pad(warm + 300 + k + j)]);
                let us = s0.elapsed().as_secs_f64() * 1e6;
                if j == 0 {
                    first.push(us);
                } else {
                    later.push(us);
                }
            }
            v.push(t.elapsed().as_secs_f64() * 1e6);
        }
        stats(&format!("clone + {depth} predicted steps"), v);
        stats(
            &format!("  of which the first predicted step (depth {depth})"),
            first,
        );
        stats(
            &format!("  of which a later predicted step (depth {depth})"),
            later,
        );
    }
    let s = stats("step", step_us);
    let p = stats("step_predicted", pred_us);
    println!("step / step_predicted = {:.2}x", s / p);
    Ok(())
}
