use super::{BattleSetup, ViewerArgs};
use anyhow::{Context, Result};
use core::sim::PadState;
use core::TICK_HZ;
use std::collections::BTreeMap;

const REPORT_SECS: u64 = 10;

fn fielded(walk: &game::walk::WalkSim, team: u8) -> BTreeMap<&str, u32> {
    let mut classes = BTreeMap::new();
    for unit in walk.sim.units.iter().filter(|u| u.alive && u.team == team) {
        let class = walk.team_template(team).class(unit.spec.class);
        *classes.entry(class.name.as_str()).or_insert(0) += 1;
    }
    classes
}

pub(super) fn run(v: &ViewerArgs, seconds: u32) -> Result<()> {
    // Timed diagnostics require an authored mode; ordinary views may use the
    // existing design-layer fallback. Check before the loader consumes a layer.
    if v.mode != "con" {
        let manifest: game::assets::Manifest =
            ron::from_str(&pak::read_to_string(v.map.join("manifest.ron"))?)?;
        anyhow::ensure!(
            game::assets::authored_mode_layer_index(&manifest, &v.mode).is_some(),
            "map has no authored world layer for mode {}",
            v.mode
        );
    }
    let (scene, load) = game::assets::load_battle_scene(&v.map, &v.mode)?;
    if let Some((path, Err(error))) = load.common {
        anyhow::bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let setup = BattleSetup::from_args(v, v.seed)?;
    println!(
        "mission: {}",
        setup.mission_file.as_deref().unwrap_or("none")
    );
    let mut walk = super::battle::make_walk(&scene, &setup)?;
    let map = v
        .map
        .file_name()
        .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
    let mut meta = game::blackbox::Meta::for_run(
        &map,
        &v.mode,
        &scene.manifest.source_game,
        v.seed,
        &format!("{:?}", setup.difficulty),
    );
    meta.mission = setup.mission_file.clone().unwrap_or_default();
    meta.player_team = setup.player_team;
    let dir = game::blackbox::default_dir().join("headless");
    let mut black_box = game::blackbox::BlackBox::start(
        true,
        meta,
        &walk,
        scene.collision_body_labels(),
        dir.clone(),
    )
    .context("black box")?;
    let idle = [PadState::default()];
    let hz = u64::from(TICK_HZ);
    let mut most = [0usize; 2];
    let mut seen = [BTreeMap::new(), BTreeMap::new()];
    for tick in 1..=u64::from(seconds) * hz {
        walk.step_tick(&idle);
        black_box.tick(&walk, None);
        for team in 0..2u8 {
            let classes = fielded(&walk, team);
            most[usize::from(team)] = most[usize::from(team)].max(classes.len());
            for class in classes.into_keys() {
                if let Some(count) = seen[usize::from(team)].get_mut(class) {
                    *count += 1;
                } else {
                    seen[usize::from(team)].insert(class.to_owned(), 1);
                }
            }
        }
        if tick.is_multiple_of(REPORT_SECS * hz) || walk.sim.outcome.is_some() {
            println!(
                "t={:>4}s points={:?} alive={:?}",
                tick / hz,
                walk.sim.team_points().map(|p| p.0),
                [
                    walk.sim.battle.teams[0].alive(),
                    walk.sim.battle.teams[1].alive()
                ],
            );
            for team in 0..2u8 {
                println!("  team {team}: {:?}", fielded(&walk, team));
            }
        }
        if walk.sim.outcome.is_some() {
            break;
        }
    }
    println!(
        "most distinct classes alive at once: {most:?}; outcome {:?}",
        walk.sim.outcome
    );
    for (team, classes) in seen.iter().enumerate() {
        println!(
            "team {team} fielded: {:?}",
            classes.keys().collect::<Vec<_>>()
        );
    }
    let snapshot = black_box.snapshot("timed_battle", Some(&walk), None);
    let mode = &v.mode;
    let seed = v.seed;
    let path = dir.join(format!("{map}_{mode}_{seed}.json"));
    game::blackbox::write_to(&path, &snapshot)?;
    println!("black box: {}", path.display());
    Ok(())
}
