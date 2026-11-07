//! Headless Space Assault sweep across bf2_spa1..9 maps.
//!
//! Evaluates round completion, score progression, and combat metrics
//! (stations destroyed, flyers shot, flyers crashed) in release mode.
//!
//! Usage:
//! ```text
//! cargo run --release -p game --example space_sweep -- [--seed N] [--maps spa1,spa3...] [--max-ticks N]
//! ```

use core::sim::{Event, PadState};
use game::walk::WalkSim;
use std::path::{Path, PathBuf};

const DEFAULT_MAPS: &[&str] = &[
    "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
];
const SCORE_LIMIT: u32 = 180;
const DEFAULT_MAX_TICKS: u64 = 7200; // 4 minutes at 30 Hz

fn assets_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OPENBF_ASSETS_BAKED") {
        return PathBuf::from(dir);
    }
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
    if fallback.exists() {
        fallback
    } else {
        game::assets::baked_root()
    }
}

fn missions_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OPENBF_MISSIONS") {
        return PathBuf::from(dir);
    }
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions");
    if fallback.exists() {
        fallback
    } else {
        PathBuf::from("missions")
    }
}

struct RunResult {
    map: String,
    seed: u32,
    outcome: String,
    duration_s: f32,
    points: String,
    stations_destroyed: u32,
    flyers_shot: u32,
    flyers_crashed: u32,
}

fn run_space_map(
    map_key: &str,
    seed: u32,
    max_ticks: u64,
    assets_root: &Path,
    missions_root: &Path,
) -> Result<RunResult, String> {
    let map_name = if map_key.starts_with("bf2_") {
        map_key.to_string()
    } else {
        format!("bf2_{map_key}")
    };
    let map_dir = assets_root.join(&map_name);
    if !map_dir.exists() {
        return Err(format!("map dir not found: {}", map_dir.display()));
    }

    let mut scene = game::assets::load_scene(&map_dir)
        .map_err(|e| format!("load_scene failed for {map_name}: {e}"))?;
    scene.apply_mode_layer("ass");

    let common_dir = assets_root.join("common_bf2");
    if common_dir.join("common.ron").exists() {
        if let Ok(common) = game::assets::load_common(&common_dir) {
            scene.merge_common(common);
        }
    }

    let mission_file = missions_root.join(format!("{map_name}_ass.ron"));
    let mission = if mission_file.exists() {
        Some(
            game::mission::MissionDef::load(&mission_file)
                .map_err(|e| format!("load_mission failed for {}: {e}", mission_file.display()))?,
        )
    } else {
        None
    };

    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));

    let mut sim = core::sim::Sim::empty(seed);
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);

    let factions = mission
        .as_ref()
        .map(game::mission::MissionDef::factions)
        .unwrap_or_else(|| {
            game::roster::default_factions(&scene.manifest.source_game).map(str::to_owned)
        });

    let classes_dir = assets_root.join("classes");
    let rosters = game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    );

    let mut tables: [Vec<core::unit::UnitClass>; 2] = [Vec::new(), Vec::new()];
    let teams = match &rosters {
        Ok((rs, _)) => {
            let mut names: Vec<(String, usize)> = Vec::new();
            let mut t = 0usize;
            rs.each_ref().map(|r| {
                for c in &r.classes {
                    let uc = c.register(&mut sim, &mut names);
                    tables[t].push(uc);
                }
                t += 1;
                game::walk::TeamTemplate::from_roster(
                    r,
                    &tables[t - 1][0],
                    r.classes
                        .iter()
                        .map(|c| {
                            let mut visual = game::walk::ClassVisual::from_roster(c);
                            visual.held_models = c
                                .held
                                .iter()
                                .map(|h| (h.slot, h.geometry.clone()))
                                .collect();
                            visual
                        })
                        .collect(),
                )
            })
        }
        Err(_) => {
            let t = game::walk::TeamTemplate::default_trooper();
            factions.clone().map(|faction| game::walk::TeamTemplate {
                faction,
                ..t.clone()
            })
        }
    };

    let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    if let Some(m) = &mission {
        m.apply_arc_blocks(nav.as_mut());
        m.apply_barriers(nav.as_mut());
    }

    let mut w = WalkSim::with_teams(sim, scene.heightfield(), glam::Vec3::ZERO, teams, nav);
    w.sim.classes = tables;
    scene.install_collision(&mut w.sim);

    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.landing_regions = a.landing_regions.clone();
        w.sim.ground_flyer_map = mission.as_ref().is_some_and(|m| m.ground_flyer_map);
    }
    w.add_posts(&layout);

    if let Some(m) = &mission {
        m.apply_door_locks(&mut w.sim.doors, &scene.manifest);
        m.apply_ai(&mut w.sim);
    }

    let factions_pair = [w.teams[0].faction.clone(), w.teams[1].faction.clone()];
    let space_def = mission
        .as_ref()
        .and_then(|m| m.space_assault.clone())
        .unwrap_or_default();
    let pad_setup = game::vehicle_pads::install_with_objective(
        &mut w.sim,
        &scene.manifest,
        &scene.dir,
        &classes_dir,
        &factions_pair,
        Some((&space_def, SCORE_LIMIT)),
    );
    if let Ok(p) = pad_setup {
        w.vehicle_geometry = p.geometry;
        w.vehicle_seats = p.seats;
        w.vehicle_weapon_hud = p.weapon_hud;
        w.walker_shot_styles = p.walker_shot_styles;
        w.vehicle_riders = p.riders;
        w.vehicle_engines = p.engines;
        w.vehicle_labels = p.labels;
    }

    w.space_round = mission
        .as_ref()
        .and_then(|m| m.space_assault.as_ref())
        .map(|d| std::sync::Arc::new(game::space_assault::SpaceRound::new(d)));
    w.sim.flyer_bots = true;

    // Headless bot sweep: slot 0 is an AI combatant
    w.sim.vacate_player_slot(0);
    w.opening_wave([16, 16], true);

    let mut stations_destroyed = 0u32;
    let mut flyers_shot = 0u32;
    let mut flyers_crashed = 0u32;

    let idle_pad = [PadState::default()];
    let mut tick_count = 0u64;

    while tick_count < max_ticks && w.sim.outcome.is_none() {
        w.step_tick(&idle_pad);
        tick_count += 1;

        for event in &w.sim.events {
            match event {
                Event::StationDestroyed { .. } => {
                    stations_destroyed += 1;
                }
                Event::VehicleDestroyed { class, killer, .. } => {
                    let is_flyer = w
                        .sim
                        .flyer_params
                        .get(*class as usize)
                        .is_some_and(|p| p.is_some());
                    if is_flyer && killer.is_some() {
                        flyers_shot += 1;
                    }
                }
                Event::FlyerImpact { lethal: true, .. } => {
                    flyers_crashed += 1;
                }
                _ => {}
            }
        }

        // Check if score limit reached
        if let Some(sa) = w.sim.space_assault.as_ref() {
            if sa.points[0] >= SCORE_LIMIT as i32 || sa.points[1] >= SCORE_LIMIT as i32 {
                break;
            }
        }
    }

    let duration_s = tick_count as f32 * core::TICK_DT;
    let (p0, p1) = w
        .sim
        .space_assault
        .as_ref()
        .map(|sa| (sa.points[0], sa.points[1]))
        .unwrap_or((0, 0));

    let outcome_str = match w.sim.outcome {
        Some(0) => format!("Team 0 ({})", factions_pair[0]),
        Some(1) => format!("Team 1 ({})", factions_pair[1]),
        _ if p0 >= SCORE_LIMIT as i32 => format!("Team 0 ({})", factions_pair[0]),
        _ if p1 >= SCORE_LIMIT as i32 => format!("Team 1 ({})", factions_pair[1]),
        _ => "Time Limit".to_string(),
    };

    Ok(RunResult {
        map: map_key.to_string(),
        seed,
        outcome: outcome_str,
        duration_s,
        points: format!("{p0} - {p1}"),
        stations_destroyed,
        flyers_shot,
        flyers_crashed,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 42u32;
    let mut max_ticks = DEFAULT_MAX_TICKS;
    let mut maps: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                if let Some(s) = args.get(i + 1).and_then(|v| v.parse().ok()) {
                    seed = s;
                    i += 1;
                }
            }
            "--max-ticks" => {
                if let Some(t) = args.get(i + 1).and_then(|v| v.parse().ok()) {
                    max_ticks = t;
                    i += 1;
                }
            }
            "--maps" => {
                if let Some(m) = args.get(i + 1) {
                    maps = m.split(',').map(|s| s.trim().to_string()).collect();
                    i += 1;
                }
            }
            arg if !arg.starts_with("--") => {
                maps.push(arg.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    if maps.is_empty() {
        maps = DEFAULT_MAPS.iter().map(|s| s.to_string()).collect();
    }

    let assets_root = assets_dir();
    let missions_root = missions_dir();

    println!("Running Space Assault sweep across {} maps (seed {seed}, score limit {SCORE_LIMIT})...", maps.len());
    println!("Assets: {}", assets_root.display());
    println!("Missions: {}", missions_root.display());
    println!();

    let mut results = Vec::new();
    for map in &maps {
        let t0 = std::time::Instant::now();
        eprint!("Running {}... ", map);
        match run_space_map(map, seed, max_ticks, &assets_root, &missions_root) {
            Ok(res) => {
                eprintln!("done ({:.1}s wall, {:.1}s sim)", t0.elapsed().as_secs_f32(), res.duration_s);
                results.push(res);
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
            }
        }
    }

    println!();
    println!("| map | seed | outcome | duration_s | points | stations_destroyed | flyers_shot | flyers_crashed |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for r in &results {
        println!(
            "| {} | {} | {} | {:.1} | {} | {} | {} | {} |",
            r.map,
            r.seed,
            r.outcome,
            r.duration_s,
            r.points,
            r.stations_destroyed,
            r.flyers_shot,
            r.flyers_crashed,
        );
    }
}
