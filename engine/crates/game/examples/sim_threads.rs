//! Issue #168 (`analysis/plan/LAN_MULTIPLAYER.md` §8b): the bots' thinks on
//! worker threads, on real maps.
//!
//! For each map, builds the baked conquest round (a human in slot 0 and
//! `--bots N` per side), clones it once per thread count, and steps every
//! copy tick by tick in turn (interleaved, so a busy machine slows them
//! alike), comparing `Sim::hash` against the first count's every tick. Then
//! reports each count's `Sim::step` wall time and its AI share
//! (`Sim::ai_wall_ms`: the commander, the board and the thinks), p50 / p99 /
//! mean in ms.
//!
//! `cargo run --release -p game --example sim_threads -- [maps...] [--bots N]
//! [--ticks N] [--threads 1,2,4]` from `engine/`, with the baked assets in
//! `assets_baked/`. Headless. `--threads` defaults to 1 and the machine's
//! default (`core::sim::think_pool::default_think_threads`).

#[path = "support/conquest_battle.rs"]
mod conquest_battle;

use anyhow::Result;
use conquest_battle::{battle, pad};
use std::path::Path;
use std::time::Instant;

fn pct(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

/// `p50 / p99 / mean` of `v`.
fn summary(mut v: Vec<f64>) -> String {
    v.sort_by(f64::total_cmp);
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    format!(
        "{:6.3} / {:6.3} / {:6.3}",
        pct(&v, 0.5),
        pct(&v, 0.99),
        mean
    )
}

fn arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut maps = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a.starts_with("--") {
            it.next();
        } else {
            maps.push(a.clone());
        }
    }
    if maps.is_empty() {
        maps = vec!["bf2_myg1".into(), "bf2_geo1".into(), "bf1_tat2".into()];
    }
    let bots: u32 = arg(&args, "--bots").unwrap_or(32);
    let ticks: u64 = arg(&args, "--ticks").unwrap_or(1800);
    let threads: Vec<usize> = arg::<String>(&args, "--threads")
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| {
            let d = core::sim::think_pool::default_think_threads();
            if d == 1 {
                vec![1]
            } else {
                vec![1, d]
            }
        });
    anyhow::ensure!(!threads.is_empty(), "--threads: no count");
    let root = Path::new("assets_baked");
    let mut failed = false;
    for key in &maps {
        let walk = battle(root, key, bots)?;
        let mut sims: Vec<_> = threads
            .iter()
            .map(|&n| {
                let mut s = walk.sim.clone();
                s.think_threads = n;
                s
            })
            .collect();
        let mut step_ms = vec![Vec::new(); sims.len()];
        let mut ai_ms = vec![Vec::new(); sims.len()];
        // The thinks' own summed wall time (`Sim::think_hist`, every
        // thread's), the full thinks among them, and the dearest think.
        let mut think_ms = Vec::new();
        let mut full_thinks = 0u64;
        let mut worst = Vec::new();
        let mut first_diff = None;
        let mut thinks = 0usize;
        for t in 0..ticks {
            let p = [pad(t)];
            thinks += sims[0]
                .units
                .iter()
                .filter(|u| u.alive && u.controller == core::unit::Controller::Ai)
                .count();
            let (sum0, full0) = (sims[0].think_hist.all.sum_ns, sims[0].think_hist.thinks.n);
            for (k, s) in sims.iter_mut().enumerate() {
                let t0 = Instant::now();
                s.step(&p);
                step_ms[k].push(t0.elapsed().as_secs_f64() * 1000.0);
                ai_ms[k].push(f64::from(s.ai_wall_ms));
            }
            think_ms.push((sims[0].think_hist.all.sum_ns - sum0) as f64 / 1e6);
            full_thinks += sims[0].think_hist.thinks.n - full0;
            worst.push(
                sims[0]
                    .tick_cost
                    .worst_think
                    .map_or(0.0, |w| f64::from(w.ms)),
            );
            let want = sims[0].hash();
            if first_diff.is_none() {
                if let Some(s) = sims.iter().find(|s| s.hash() != want) {
                    first_diff = Some((t + 1, s.think_threads));
                }
            }
        }
        println!(
            "{key}: {bots} bots a side, {ticks} ticks, {:.1} thinks a tick, final hash {:016x}",
            thinks as f64 / ticks as f64,
            sims[0].hash()
        );
        println!(
            "  {} threads: thinks' summed ms p50 / p99 / mean {}; dearest think ms {}; {:.1} full thinks a tick",
            threads[0],
            summary(think_ms),
            summary(worst),
            full_thinks as f64 / ticks as f64
        );
        match first_diff {
            None => println!("  hashes identical every tick across threads {threads:?}"),
            Some((t, n)) => {
                failed = true;
                println!(
                    "  DIVERGED: {n} threads differ from {} at tick {t}",
                    threads[0]
                );
            }
        }
        println!("  threads   step ms p50 / p99 / mean     ai ms p50 / p99 / mean");
        for (k, n) in threads.iter().enumerate() {
            println!(
                "  {n:>7}   {}   {}",
                summary(step_ms[k].clone()),
                summary(ai_ms[k].clone())
            );
        }
    }
    anyhow::ensure!(!failed, "a thread count changed the sim");
    Ok(())
}
