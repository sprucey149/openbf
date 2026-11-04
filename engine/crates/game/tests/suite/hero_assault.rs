use std::path::{Path, PathBuf};

fn engine_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn mission() -> Option<game::mission::MissionDef> {
    let path = engine_dir().join("missions/tat2_eli.ron");
    let baked = game::assets::baked_root();
    if !path.is_file()
        || !baked.join("tat2/manifest.ron").is_file()
        || !baked.join("classes/bf2_rep.ron").is_file()
    {
        eprintln!("SKIP: the Mos Eisley bake and generated missions are required");
        return None;
    }
    Some(game::mission::MissionDef::load(&path).unwrap())
}

#[test]
fn mos_eisley_lists_the_disc_rosters_on_their_own_layer() {
    let Some(m) = mission() else {
        return;
    };
    assert!(m.is_hero_assault());
    assert_eq!(m.factions(), ["all".to_owned(), "imp".to_owned()]);
    assert_eq!([m.teams[0].units, m.teams[1].units], [12, 12]);
    assert_eq!(m.tickets(), [999_999, 999_999]);
    let classes = game::assets::baked_root().join("classes");
    let factions = m.factions();
    let (rosters, _) =
        game::roster::FactionRoster::pair(&classes, "bf2", [&factions[0], &factions[1]], Some(&m))
            .unwrap();
    let names = |team: usize| -> Vec<&str> {
        rosters[team]
            .classes
            .iter()
            .map(|c| c.class.as_str())
            .collect()
    };
    assert_eq!(names(0).len(), 9);
    assert_eq!(names(1).len(), 8);
    assert!(names(0).contains(&"rep_hero_yoda") && names(0).contains(&"all_hero_luke_jedi"));
    assert!(names(1).contains(&"imp_hero_darthvader") && names(1).contains(&"cis_hero_darthmaul"));
    for roster in &rosters {
        assert!(roster
            .classes
            .iter()
            .all(|c| c.min == 1 && c.max == Some(2)));
    }
}

#[test]
fn every_authored_post_stands_including_the_one_with_no_regions() {
    if mission().is_none() {
        return;
    }
    let baked = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&baked.join("tat2"), "eli").unwrap();
    let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let mut names: Vec<&str> = layout.cps.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["eli_cp1", "eli_cp2", "eli_cp3", "eli_cp6", "eli_cp7", "eli_cp8"]
    );
    let (conquest, _) = game::assets::load_battle_scene(&baked.join("tat2"), "con").unwrap();
    assert!(!conquest.manifest.spawn_only_posts);
}

#[test]
fn a_timed_battle_fields_concurrent_heroes_and_scores_kills() {
    if mission().is_none() {
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hero_assault");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_game"))
        .current_dir(engine_dir())
        .env("OPENBF_BLACKBOX_DIR", &dir)
        .args([
            "--map",
            "assets_baked/tat2",
            "--mode",
            "eli",
            "--headless",
            "--seconds",
            "60",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let path = dir.join("headless/tat2_eli_42.json");
    let snapshot: game::blackbox::Snapshot =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let last = snapshot.world.last().unwrap();
    assert!(
        last.points[0] > 0 && last.points[1] > 0,
        "{:?}",
        last.points
    );
    assert_eq!(last.bleed, [0.0, 0.0]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("most distinct classes alive at once: [9, 8]"),
        "{stdout}"
    );
}

#[test]
fn every_hero_falls_on_the_hero_death_clip_with_no_hero_manager() {
    let Some(m) = mission() else {
        return;
    };
    let root = game::assets::baked_root();
    let factions = m.factions();
    let (rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        "bf2",
        [&factions[0], &factions[1]],
        Some(&m),
    )
    .unwrap();
    let mut sim = core::sim::Sim::empty(53);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    let mut names = Vec::new();
    let teams = std::array::from_fn(|t| {
        let registered: Vec<_> = rosters[t]
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
        sim.classes[t] = registered.clone();
        let visuals = rosters[t]
            .classes
            .iter()
            .map(game::walk::ClassVisual::from_roster)
            .collect();
        game::walk::TeamTemplate::from_roster(&rosters[t], &registered[0], visuals)
    });
    let mut walk = game::walk::WalkSim::with_teams(sim, None, glam::Vec3::ZERO, teams, None);
    assert_eq!(walk.sim.hero_class(0), None);
    let mut units = Vec::new();
    for team in 0..2u8 {
        for class in 0..walk.sim.classes[usize::from(team)].len() {
            let at = glam::Vec3::new(units.len() as f32 * 4.0 + 8.0, 0.0, 0.0);
            let skill = walk.sim.bot_skill(team);
            let spec = walk.sim.classes[usize::from(team)][class].spec(
                team,
                at,
                0.0,
                core::unit::Controller::Ai,
                class as u8,
                skill,
            );
            units.push(walk.sim.spawn(&spec).unwrap());
        }
    }
    assert_eq!(units.len(), 17);
    let mut vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, "bf2", &walk.teams);
    vis.update(&walk, core::TICK_DT);
    for &unit in &units {
        walk.sim
            .kill_environment(unit, core::sim::DeathCause::Suicide);
    }
    vis.update(&walk, core::TICK_DT);
    for unit in units {
        let index = unit.index as usize;
        let u = &walk.sim.units[index];
        let asset = vis.asset_for(u.team, u.spec.class).unwrap();
        let clip = vis.units[index].animator.lower_clip().unwrap();
        assert!(
            asset.asset.clips[clip].name.contains("stand_deadhero"),
            "{}: {}",
            walk.team_template(u.team).class(u.spec.class).name,
            asset.asset.clips[clip].name
        );
    }
}
