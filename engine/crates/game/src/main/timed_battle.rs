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
    let (layers, kills) = match &v.mission {
        Some(path) => {
            let m = game::mission::MissionDef::load(path)?;
            let kills = m.scene_kills().to_vec();
            (m.layers, kills)
        }
        None => (Vec::new(), Vec::new()),
    };
    let (mut scene, load) = game::assets::load_battle_scene_with_layers(&v.map, &v.mode, &layers)?;
    scene.kill_objects(&kills);
    if let Some((path, Err(error))) = load.common {
        anyhow::bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let setup = BattleSetup::from_args(v, v.seed)?;
    println!(
        "mission: {}",
        setup.mission_file.as_deref().unwrap_or("none")
    );
    let mut walk = super::battle::make_walk(&scene, &setup)?;
    // Diagnostics only (`HERO_CENSUS`): `TIMED_AI_HEROES=<unlock s>` lets bots
    // take a conquest hero (Random, a 30 s respawn), `TIMED_BOTS_ONLY=1` gives
    // the idle player's slot to a bot.
    if let Some(secs) = std::env::var("TIMED_AI_HEROES")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    {
        if let Some(h) = walk.sim.heroes.as_mut() {
            h.rules.ai_candidates = true;
            h.rules.unlock = core::sim::heroes::HeroUnlock::Timer(secs);
            h.rules.assign = core::sim::heroes::HeroAssign::Random;
            h.rules.respawn_ticks = Some(30 * u64::from(TICK_HZ));
        }
    }
    if std::env::var("TIMED_BOTS_ONLY").is_ok_and(|v| v == "1") {
        walk.sim.leave_human(0);
    }
    let mut census = super::hero_census::wanted().then(super::hero_census::HeroCensus::default);
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
    // Dev-only session stats (`session-stats` feature): the round as a
    // played one writes it, read beside it (the feed only reads the sim).
    let mut stats = game::session_stats::ENABLED.then(|| {
        let assets = v
            .map
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let feed =
            game::event_feed::EventFeed::for_walk(&assets, &scene.manifest.source_game, &walk);
        let info = game::session_stats::RoundInfo {
            map: map.clone(),
            mode: v.mode.clone(),
            mission: setup.mission_file.clone().unwrap_or_default(),
            source_game: scene.manifest.source_game.clone(),
            seed: v.seed,
            xl: setup.xl,
            team_size_pct: setup.team_size_pct,
            role: "headless".into(),
            local_slot: walk.local_slot,
        };
        let dir = game::session_stats::dir();
        (
            game::session_stats::SessionStats::start(info, &walk, dir),
            feed,
        )
    });
    let idle = [PadState::default()];
    let hz = u64::from(TICK_HZ);
    let mut most = [0usize; 2];
    let mut seen = [BTreeMap::new(), BTreeMap::new()];
    for tick in 1..=u64::from(seconds) * hz {
        walk.step_tick(&idle);
        black_box.tick(&walk, None);
        if let Some(c) = census.as_mut() {
            c.tick(&walk);
        }
        if let Some((s, feed)) = stats.as_mut() {
            feed.tick(&walk.sim, walk.player);
            drop(feed.take());
            drop(feed.take_ambush());
            s.tick(&walk);
        }
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
    if let Some(c) = census {
        print!("{}", c.report(walk.sim.tick));
    }
    if let Some((mut s, feed)) = stats {
        let reason = if walk.sim.outcome.is_some() {
            "outcome"
        } else {
            "quit"
        };
        if let Some(p) = s.write(&walk, feed.round(), Some(&feed), reason) {
            println!("session stats: wrote {}", p.display());
        }
    }
    let snapshot = black_box.snapshot("timed_battle", Some(&walk), None);
    let mode = &v.mode;
    let seed = v.seed;
    let path = dir.join(format!("{map}_{mode}_{seed}.json"));
    game::blackbox::write_to(&path, &snapshot)?;
    println!("black box: {}", path.display());
    Ok(())
}
