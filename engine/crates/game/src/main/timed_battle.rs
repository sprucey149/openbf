use super::{BattleSetup, ViewerArgs};
use anyhow::{Context, Result};
use core::sim::PadState;
use core::TICK_HZ;
use std::collections::BTreeMap;
use std::path::PathBuf;

const REPORT_SECS: u64 = 10;

fn default_mission(v: &ViewerArgs) -> Option<PathBuf> {
    let root = v.map.parent()?.parent().unwrap_or(std::path::Path::new(""));
    let key = v.map.file_name()?.to_string_lossy();
    let path = root.join("missions").join(format!("{key}_{}.ron", v.mode));
    (v.mode != "con" && pak::read_to_string(&path).is_ok()).then_some(path)
}

fn fielded(walk: &game::walk::WalkSim, team: u8) -> BTreeMap<String, u32> {
    let mut classes = BTreeMap::new();
    for unit in walk.sim.units.iter().filter(|u| u.alive && u.team == team) {
        let class = walk.team_template(team).class(unit.spec.class);
        *classes.entry(class.name.clone()).or_insert(0) += 1;
    }
    classes
}

pub(super) fn run(v: &ViewerArgs, seconds: u32) -> Result<()> {
    let (scene, load) = game::assets::load_battle_scene(&v.map, &v.mode)?;
    if let Some((path, Err(error))) = load.common {
        anyhow::bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let mut setup = BattleSetup::from_args(v, v.seed)?;
    if setup.mission.is_none() {
        if let Some(path) = default_mission(v) {
            let mission = game::mission::MissionDef::load(&path)
                .with_context(|| format!("mission {}", path.display()))?;
            if setup.teams.is_none() {
                setup.teams = Some(mission.factions());
            }
            setup.mission_file = Some(path.display().to_string());
            setup.mission = Some(mission);
        }
    }
    println!(
        "mission: {}",
        setup.mission_file.as_deref().unwrap_or("none")
    );
    let mut walk = super::battle::make_walk(&scene, &setup);
    let map = v
        .map
        .file_name()
        .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
    let meta = game::blackbox::Meta::for_run(
        &map,
        &v.mode,
        &scene.manifest.source_game,
        v.seed,
        &format!("{:?}", setup.difficulty),
    );
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
                *seen[usize::from(team)].entry(class).or_insert(0u64) += 1;
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
    let path = dir.join(format!("{map}_{}_{}.json", v.mode, v.seed));
    game::blackbox::write_to(&path, &snapshot)?;
    println!("black box: {}", path.display());
    Ok(())
}
