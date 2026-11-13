use super::*;

fn fixture() -> PathBuf {
    std::env::var_os("OPENBF_ASSAULT_ROOT")
        .map(PathBuf::from)
        .expect("OPENBF_ASSAULT_ROOT must name an owned engine fixture")
}

fn app(root: &std::path::Path) -> App {
    let mut app = App::new(
        root.join("assets_baked"),
        root.join("missions"),
        setup(None),
        None,
        None,
        None,
        Default::default(),
    );
    app.mode = "eli".to_owned();
    app.session.ass_numbots = 2;
    app.session.ass_score = 50;
    app.session.con_numbots = 200;
    app.session.con_mult = 500;
    app.session.con_timer = 1;
    app.renderer = Some(Renderer::new_headless(640, 480).expect("headless GPU"));
    app
}

fn start(a: &mut App, name: &str) -> anyhow::Result<()> {
    let (scene, load) = game::assets::load_battle_scene(&a.assets_dir.join(name), &a.mode)?;
    a.start_battle(
        name.into(),
        loading::Prepared {
            scene,
            load,
            cpu_ms: 0.0,
        },
    )
}

#[test]
#[ignore = "owned Hunt assets and headless GPU; scripts/gpu_tests.sh --battle-launch"]
fn shell_hunt_launches_every_matchup_with_playable_models_and_session_rules() {
    use core::sim::{buttons::SPAWN_ACCEPT, PadState, ScreenPick, SpawnStage};

    let Some(root) = std::env::var_os("OPENBF_HUNT_ROOT").map(PathBuf::from) else {
        eprintln!("SKIP: set OPENBF_HUNT_ROOT to a Hunt bake");
        return;
    };
    let mut a = app(&root);
    a.mode = "hunt".into();
    a.session.hun_timer = 10;
    a.session.hun_score = 70;
    let mut maps: Vec<_> = std::fs::read_dir(root.join("missions"))
        .unwrap()
        .filter_map(|entry| {
            let file = entry.ok()?.file_name().into_string().ok()?;
            file.strip_suffix("_hunt.ron").map(str::to_owned)
        })
        .collect();
    maps.sort();
    assert_eq!(maps.len(), 14);
    for name in maps {
        let mission = game::mission::MissionDef::load(
            &root.join("missions").join(format!("{name}_hunt.ron")),
        )
        .unwrap();
        a.current = Some(ui::shell::Launch {
            key: name.clone(),
            game: if mission.source_game == "bf1" {
                ui::shell::Game::Bf1
            } else {
                ui::shell::Game::Bf2
            },
            mode: ui::shell::Mode::Hunt,
            armies: mission.factions(),
            label: name.clone(),
        });
        start(&mut a, &name).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        let walk = &mut a.walk.as_mut().unwrap().0;
        let sim = &walk.sim;
        assert!(sim.is_hunt(), "{name}");
        assert_eq!(sim.team_points(), Some(([0, 0], 70)), "{name}");
        assert_eq!(sim.time_limit, Some(600), "{name}");
        assert_eq!(sim.spawn_stage[0], core::sim::SpawnStage::Side, "{name}");
        let vis = a.soldier_vis.as_ref().unwrap();
        for team in 0..2 {
            assert!(!sim.spawn_points(team).is_empty(), "{name} team {team}");
            for (class, visual) in walk.teams[usize::from(team)].classes.iter().enumerate() {
                assert!(
                    vis.asset_for(team, class as u8).is_some(),
                    "{name} team {team} class {} has no body model",
                    visual.name
                );
            }
        }

        let team = match name.as_str() {
            "bf2_hot1" | "tat2" => 0,
            _ => 1,
        };
        let press = |walk: &mut WalkSim, pick| {
            walk.step_tick(&[PadState {
                buttons: SPAWN_ACCEPT,
                pick,
                ..Default::default()
            }]);
            walk.step_tick(&[PadState::default()]);
        };
        press(walk, Some(ScreenPick::Side(team)));
        assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Map, "{name}");
        let post = walk
            .sim
            .cps
            .iter()
            .position(|cp| cp.owner == Some(team))
            .expect("playable side has a spawn post");
        press(walk, Some(ScreenPick::Post(post as u16)));
        assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Class, "{name}");
        press(walk, None);
        for _ in 0..core::TICK_HZ * 20 {
            if walk.sim.unit(walk.player).is_some_and(|unit| unit.alive) {
                break;
            }
            walk.step_tick(&[PadState::default()]);
        }
        let unit = walk.sim.unit(walk.player).expect("player slot");
        assert!(unit.alive, "{name}: native player spawned");
        assert_eq!(unit.team, team, "{name}");
    }
}

#[test]
#[ignore = "owned Hero Assault assets and headless GPU; scripts/gpu_tests.sh --battle-launch"]
fn shell_assault_preserves_authored_seats_options_and_spawn_flow() {
    use core::sim::{buttons::SPAWN_ACCEPT, PadState, ScreenPick, SpawnStage};
    let root = fixture();
    let mut a = app(&root);
    a.current = Some(ui::shell::Launch {
        key: "tat2".to_owned(),
        game: ui::shell::Game::Bf2,
        mode: ui::shell::Mode::Eli,
        armies: ["imp".into(), "all".into()],
        label: String::new(),
    });
    start(&mut a, "tat2").unwrap();
    assert!(matches!(a.screen, Screen::Battle));
    assert_eq!(a.setup.mission_file.as_deref(), Some("tat2_eli.ron"));
    assert_eq!(a.setup.player_team, 1);
    assert_eq!(a.setup.teams, Some(["all".into(), "imp".into()]));
    assert_eq!(
        (
            a.setup.team_size_pct,
            a.setup.ticket_pct,
            a.setup.time_limit,
            a.setup.score_limit
        ),
        (None, 100, 0, 50)
    );
    let w = &mut a.walk.as_mut().expect("battle").0;
    assert_eq!(w.sim.team_points(), Some(([0, 0], 50)));
    assert!(w.sim.heroes.is_none());
    assert_eq!(w.sim.ai_alive_cap, 2);
    assert!(w.sim.time_limit.is_none());
    assert_eq!([w.sim.classes[0].len(), w.sim.classes[1].len()], [9, 8]);
    assert_eq!(w.teams[0].class(0).name, "all_hero_hansolo_tat");
    assert_eq!(w.teams[1].class(0).name, "imp_hero_bobafett");
    assert_eq!(w.sim.cps.len(), 6);
    assert_eq!(w.sim.spawn_stage[0], SpawnStage::Side);
    let press = |w: &mut WalkSim, pick| {
        w.step_tick(&[PadState {
            buttons: SPAWN_ACCEPT,
            pick: Some(pick),
            ..Default::default()
        }]);
        w.step_tick(&[PadState::default()]);
    };
    press(w, ScreenPick::Side(1));
    assert_eq!(w.sim.spawn_stage[0], SpawnStage::Map);
    let post = w
        .sim
        .cps
        .iter()
        .position(|c| c.def.name == "eli_cp8")
        .unwrap();
    press(w, ScreenPick::Post(post as u16));
    assert_eq!(w.sim.spawn_stage[0], SpawnStage::Class);
    let choose_grievous = |w: &mut WalkSim| {
        for _ in 0..8 {
            if w.sim.spawn_class_choice(0, 1) == Some(4) {
                break;
            }
            w.step_tick(&[PadState {
                buttons: core::sim::buttons::SPAWN_CLASS_NEXT,
                ..Default::default()
            }]);
            for _ in 0..core::TICK_HZ {
                w.step_tick(&[PadState::default()]);
            }
        }
        assert_eq!(w.sim.spawn_class_choice(0, 1), Some(4));
        w.step_tick(&[PadState {
            buttons: SPAWN_ACCEPT,
            ..Default::default()
        }]);
        w.step_tick(&[PadState::default()]);
    };
    choose_grievous(w);
    for _ in 0..1200 {
        if w.sim.units[w.player.index as usize].alive {
            break;
        }
        w.step_tick(&[PadState::default()]);
    }
    let old = w.player;
    assert!(w.sim.units[old.index as usize].alive);
    assert_eq!(w.sim.units[old.index as usize].spec.class, 4);
    w.sim.kill_environment(old, core::sim::DeathCause::Suicide);
    for _ in 0..1200 {
        w.step_tick(&[PadState::default()]);
        if w.sim.spawn_stage[0] == SpawnStage::Map {
            break;
        }
    }
    assert_eq!(w.sim.spawn_stage[0], SpawnStage::Map);
    press(w, ScreenPick::Post(post as u16));
    choose_grievous(w);
    for _ in 0..1200 {
        if w.sim.units[w.player.index as usize].alive {
            break;
        }
        w.step_tick(&[PadState::default()]);
    }
    assert!(w.sim.units[w.player.index as usize].alive);
    assert_ne!(w.player.gen, old.gen);
    assert_eq!(w.sim.units[w.player.index as usize].spec.class, 4);
}

#[test]
#[ignore = "owned Hero Assault assets and headless GPU; scripts/gpu_tests.sh --battle-launch"]
fn shell_assault_rejects_bad_launches_and_keeps_partial_heroes() {
    let root = fixture();
    let fixture_root = root.parent().unwrap().parent().unwrap();
    for (kind, map) in [
        ("no-mission", "tat2"),
        ("missing-combos", "tat2"),
        ("missing-classes", "tat2"),
    ] {
        let own = fixture_root.join(kind).join("engine");
        let mut a = app(&own);
        assert!(start(&mut a, map).is_err(), "{kind}");
        assert!(matches!(a.screen, Screen::Shell), "{kind}");
        assert!(
            a.walk.is_none() && a.gpu.is_none(),
            "{kind}: no round installed"
        );
        assert!(a.renderer.is_some(), "{kind}: renderer retained for retry");
        a.assets_dir = root.join("assets_baked");
        a.missions_dir = root.join("missions");
        start(&mut a, "tat2").unwrap();
        assert!(matches!(a.screen, Screen::Battle), "{kind}: retry succeeds");
        assert_eq!(a.walk.as_ref().unwrap().0.sim.classes[0].len(), 9);
    }
    let mut a = app(&root);
    assert!(start(&mut a, "bf2_hot1").is_err());
    assert!(matches!(a.screen, Screen::Shell));
    assert!(a.walk.is_none());
    // Directory names remain valid while their owned missions are hostile.
    for kind in [
        "mode-mismatch",
        "map-mismatch",
        "game-mismatch",
        "empty-team",
        "partial-first",
    ] {
        let mut a = app(&root);
        a.missions_dir = fixture_root.join("shell").join(kind);
        let result = start(&mut a, "tat2");
        if kind == "partial-first" {
            result.unwrap();
            assert!(matches!(a.screen, Screen::Battle));
            let w = &a.walk.as_ref().unwrap().0;
            assert_eq!([w.sim.classes[0].len(), w.sim.classes[1].len()], [8, 8]);
            assert!(w
                .teams
                .iter()
                .all(|t| t.classes.iter().all(|c| c.name.contains("_hero_"))));
        } else {
            assert!(result.is_err(), "{kind}");
            assert!(matches!(a.screen, Screen::Shell), "{kind}");
            assert!(a.walk.is_none(), "{kind}");
        }
    }
}
