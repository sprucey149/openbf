//! Owned-data issue #166 checks. Missions are generated into an owned folder;
//! the shared mission cache is never rewritten by these tests.

use core::{
    sim::{buttons, PadState, Sim},
    unit::Controller,
};
use game::{
    mission::MissionDef,
    roster::FactionRoster,
    walk::{ClassVisual, TeamTemplate, WalkSim},
};
use glam::Vec3;
use std::path::PathBuf;

#[test]
fn bf1_heroes_same_named_assets_keep_their_source_banks() {
    let mut team = TeamTemplate::default_trooper();
    team.classes = vec![
        ClassVisual {
            name: "all_inf_soldier".into(),
            geometry: Some("shared_body".into()),
            weapon_geometry: Some("shared_weapon".into()),
            ..Default::default()
        },
        ClassVisual {
            name: "all_hero_luke_jedi".into(),
            geometry: Some("shared_body".into()),
            weapon_geometry: Some("shared_weapon".into()),
            ..Default::default()
        },
    ];
    let mut empty = team.clone();
    empty.classes.clear();
    let plan = game::soldier_vis::DrawPlan::for_game(&[team, empty], "bf1");
    assert_eq!(plan.body_games, ["bf1", "bf2"]);
    assert_eq!(plan.weapon_games, ["bf1", "bf2"]);
    assert_ne!(plan.per_class[0][0].body, plan.per_class[0][1].body);
    assert_ne!(plan.per_class[0][0].weapon, plan.per_class[0][1].weapon);
}

fn missions() -> Vec<MissionDef> {
    let dir = PathBuf::from(
        std::env::var_os("OPENBF_BF1_HERO_MISSIONS").expect("generate owned missions first"),
    );
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    paths.sort();
    paths.iter().map(|p| MissionDef::load(p).unwrap()).collect()
}

fn battle(m: &MissionDef) -> (Sim, [TeamTemplate; 2], [u8; 2]) {
    let root = game::assets::baked_root();
    let f = m.factions();
    let (mut rosters, _) = FactionRoster::pair(
        &root.join("classes"),
        &m.source_game,
        [&f[0], &f[1]],
        Some(m),
    )
    .unwrap();
    let heroes = std::array::from_fn(|team| {
        let before = rosters[team].classes.clone();
        let hero = game::heroes::append(&mut rosters[team], &root.join("classes"), Some(m), team)
            .expect("hero class");
        assert_eq!(usize::from(hero), before.len());
        for (a, b) in before.iter().zip(&rosters[team].classes) {
            assert_eq!(
                (&a.class, a.min, a.max, a.max_health, &a.geometry),
                (&b.class, b.min, b.max, b.max_health, &b.geometry)
            );
        }
        assert!(
            game::heroes::append(&mut rosters[team], &root.join("classes"), Some(m), team)
                .is_none(),
            "duplicate append"
        );
        let mut hunt = m.clone();
        hunt.mode = "hunt".into();
        assert!(
            game::heroes::append(&mut rosters[team], &root.join("classes"), Some(&hunt), team)
                .is_none(),
            "Hunt never appends a hero"
        );
        assert_eq!(
            rosters[team].classes[usize::from(hero)].class,
            m.heroes
                .iter()
                .find(|(t, _)| usize::from(*t) == team + 1)
                .unwrap()
                .1
                .to_ascii_lowercase()
        );
        hero
    });
    let mut sim = Sim::empty(166);
    let mut names = Vec::new();
    let teams = std::array::from_fn(|team| {
        sim.classes[team] = rosters[team]
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
        TeamTemplate::from_roster(
            &rosters[team],
            &sim.classes[team][0],
            rosters[team]
                .classes
                .iter()
                .map(ClassVisual::from_roster_with_held)
                .collect(),
        )
    });
    game::heroes::install(&mut sim, heroes.map(Some));
    (sim, teams, heroes)
}

#[test]
#[ignore = "requires owned generated missions and BF1/BF2 bake"]
fn bf1_heroes_all_assignments_assets_and_audio() {
    let root = game::assets::baked_root();
    let mut count = 0;
    let mut names = std::collections::BTreeSet::new();
    for m in missions()
        .into_iter()
        .filter(|m| m.source_game == "bf1" && m.mode == "con")
    {
        let (sim, teams, heroes) = battle(&m);
        let mut walk = WalkSim::with_teams(sim, None, Vec3::ZERO, teams, None);
        walk.sound_levels = m.sound_levels();
        let teams = &walk.teams;
        let lib = game::sounds::SoundLib::load_for(&root.join(&m.map), &m).expect("sound bank");
        let mut live = game::sounds::SoundLib::load(&root.join(&m.map)).unwrap();
        live.for_walk(&root.join(&m.map), &walk);
        let f = m.factions();
        let levels = game::foley::sublevels(&root.join(&m.map), [&f[0], &f[1]], &m.sound_levels());
        let foley =
            game::foley::Table::load(&root.join(&m.map), &levels).expect("native map foley");
        for (team, hero) in heroes.into_iter().enumerate() {
            let c = teams[team].class(hero);
            assert_eq!(c.asset_game("bf1"), "bf2");
            assert_eq!(
                walk.sim.ai_class_weights(team as u8)[usize::from(hero)],
                0.0
            );
            let audio = &c.cries.hero;
            let surface = game::foley::Surface::start(&foley, c.foley_class);
            if let Some((group, row)) = surface.row {
                let row = &foley.groups[group].rows[row];
                for sound in [
                    row.walk_left,
                    row.walk_right,
                    row.run_left,
                    row.run_right,
                    row.jump,
                    row.land,
                ]
                .into_iter()
                .filter(|h| *h != 0)
                {
                    assert!(
                        lib.audio_of(sound).is_some(),
                        "{} {} foley {sound:08x}",
                        m.map,
                        c.name
                    );
                }
            } else {
                eprintln!("HERO_FOLEY_GAP {} {} {:08x}", m.map, c.name, c.foley_class);
            }
            for sound in [
                &audio.spawned,
                &audio.defeated,
                &audio.saber_ambient,
                &audio.saber_on,
                &audio.saber_off,
                &audio.saber_flight,
                &c.cries.hurt,
                &c.cries.death,
                &c.cries.low_health,
            ]
            .into_iter()
            .flatten()
            {
                assert!(lib.has(sound), "{} {} {sound}", m.map, c.name);
                let path = lib.take_path(sound, 0).expect(sound);
                assert_eq!(
                    live.take_path(sound, 0),
                    Some(path.clone()),
                    "live battle sound setup"
                );
                assert!(path.is_file(), "{}", path.display());
                assert!(audio::decode(&path).unwrap().frames() > 0);
            }
            for entry in &c.cries.vo.entries {
                assert!(lib.has(&entry.sound), "{} {}", c.name, entry.sound);
                let path = lib.take_path(&entry.sound, 0).expect(&entry.sound);
                assert!(path.is_file(), "{}", path.display());
            }
            let theme = lib
                .music(audio.music.as_deref().expect("hero theme"))
                .expect("resolved hero theme");
            assert!(!theme.takes.is_empty());
            assert!(theme
                .takes
                .iter()
                .all(|t| t.path.is_file() && t.secs() > 0.0));
            if names.insert(c.name.clone()) {
                let vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, "bf1", teams);
                let actual = vis.asset_for(team as u8, hero).expect("BF2 hero body");
                let expected = game::assets::load_visual_skinned_glb(
                    &root
                        .join("side_bf2/soldiers")
                        .join(format!("{}.glb", c.geometry.as_ref().unwrap())),
                )
                .unwrap();
                assert_eq!(actual.asset.skeleton.names, expected.skeleton.names);
                assert_eq!(
                    actual.asset.mesh.positions.len(),
                    expected.mesh.positions.len()
                );
                assert!(actual.rig.is_some());
                assert!(!actual.asset.clips.is_empty());
                assert!(
                    c.anim.custom != Default::default(),
                    "{} descriptors",
                    c.name
                );
            }
            count += 1;
        }
    }
    assert_eq!(count, 56);
    assert_eq!(names.len(), 5);
}

#[test]
#[ignore = "requires owned generated missions and BF1/BF2 bake"]
fn bf1_heroes_player_offer_and_optional_ai_use_existing_manager() {
    let m = missions()
        .into_iter()
        .find(|m| m.map == "bf1_bes1" && m.era.as_deref() == Some("cw") && m.mode == "con")
        .unwrap();
    assert_eq!(
        player_and_ai_life(&m),
        player_and_ai_life(&m),
        "seeded hero path is repeatable"
    );
}

fn player_and_ai_life(m: &MissionDef) -> u64 {
    let root = game::assets::baked_root();
    let mut reversed = FactionRoster {
        faction: m.teams[1].faction.clone(),
        classes: Vec::new(),
    };
    let swapped = game::heroes::append(&mut reversed, &root.join("classes"), Some(m), 0).unwrap();
    assert_eq!(
        reversed.classes[usize::from(swapped)].class,
        "cis_hero_countdooku"
    );
    let mut foreign = FactionRoster {
        faction: "all".into(),
        classes: Vec::new(),
    };
    assert!(game::heroes::append(&mut foreign, &root.join("classes"), Some(m), 0).is_none());
    let (mut sim, teams, heroes) = battle(m);
    sim.heightfield = Some(flat_ground());
    for team in 0..2 {
        sim.add_cp(
            core::cp::CpDef {
                name: format!("home{team}"),
                pos: Vec3::X * team as f32 * 100.0,
                capture: core::region::Region::sphere(Vec3::X * team as f32 * 100.0, 8.0),
                control: None,
                value_bleed: 0,
                value_atk: [0; 2],
                value_def: [0; 2],
                spawn_nodes: vec![(Vec3::X * team as f32 * 100.0, 0.0)],
                capture_time: 1.0,
                neutralize_time: 1.0,
            },
            Some(team as u8),
        );
    }
    let player = sim
        .spawn(&sim.classes[0][0].spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
        .unwrap();
    let bot = sim
        .spawn(&sim.classes[1][0].spec(1, Vec3::X * 100.0, 0.0, Controller::Ai, 0, 0))
        .unwrap();
    sim.heroes.as_mut().unwrap().points = vec![10, 20];
    sim.step(&[PadState::default()]);
    assert_eq!(sim.hero_offer(0).unwrap().0, heroes[0]);
    assert!(sim.heroes.as_ref().unwrap().teams[1].owner.is_none());
    sim.heroes.as_mut().unwrap().rules.ai_candidates = true;
    sim.heroes.as_mut().unwrap().teams[1].ready_at = 0;
    sim.step(&[PadState {
        buttons: buttons::HERO_ACCEPT,
        ..Default::default()
    }]);
    let human = sim.players[0].unwrap();
    assert!(sim.is_hero(human));
    assert_ne!(human, player);
    let bot = sim.handle(bot.index as usize);
    assert!(sim.is_hero(bot));
    let human_name = &teams[0].class(sim.unit(human).unwrap().spec.class).name;
    assert_eq!(human_name, "rep_hero_macewindu");
    assert_eq!(
        teams[1].class(sim.unit(bot).unwrap().spec.class).name,
        "cis_hero_countdooku"
    );
    let health = sim.unit(human).unwrap().health.cur;
    sim.step(&[PadState::default()]);
    assert!(
        sim.unit(human).unwrap().health.cur < health,
        "BF2 hero time drain"
    );
    sim.hash()
}

#[test]
#[ignore = "headless GPU; requires owned generated missions and BF1/BF2 bake"]
fn bf1_heroes_headless_bodies_and_blades() {
    let root = game::assets::baked_root();
    let mut renderer = render::Renderer::new_headless(320, 240).expect("headless GPU");
    let mut names = std::collections::BTreeSet::new();
    for m in missions()
        .into_iter()
        .filter(|m| m.source_game == "bf1" && m.mode == "con")
    {
        let (sim, teams, heroes) = battle(&m);
        if heroes
            .iter()
            .enumerate()
            .all(|(t, h)| names.contains(&teams[t].class(*h).name))
        {
            continue;
        }
        let mut walk = WalkSim::with_teams(sim, Some(flat_ground()), Vec3::ZERO, teams, None);
        for (t, h) in heroes.into_iter().enumerate() {
            let spec = walk.sim.classes[t][usize::from(h)].spec(
                t as u8,
                Vec3::X * (t as f32 * 8.0 - 4.0),
                0.0,
                Controller::Ai,
                h,
                0,
            );
            let actor = walk.sim.spawn(&spec).unwrap();
            assert_eq!(
                walk.sim.unit(actor).unwrap().soldier.pos.y,
                0.0,
                "capture actors stand at camera height"
            );
            let c = walk.teams[t].class(h);
            names.insert(c.name.clone());
        }
        let mut vis =
            game::soldier_vis::SoldierVisuals::load(&mut renderer, &root, "bf1", &walk.teams);
        vis.update(&walk, 1.0 / 30.0);
        for (t, h) in heroes.into_iter().enumerate() {
            let body = vis.asset_for(t as u8, h).unwrap();
            assert!(!body.meshes.is_empty(), "hero body uploaded");
        }
        assert!(!vis.saber_hilts(&walk).is_empty(), "hero hilt uploaded");
        let blades = vis.saber_draws(&walk);
        assert!(
            blades.iter().any(|d| d.key.x < -2.0),
            "team 0 textured blade"
        );
        assert!(
            blades.iter().any(|d| d.key.x > 2.0),
            "team 1 textured blade"
        );
        let camera = render::Camera {
            position: Vec3::new(0.0, 1.5, -12.0),
            yaw: std::f32::consts::PI,
            ..Default::default()
        };
        let draws = vis.draws(&walk);
        assert!(!draws.is_empty(), "posed hero bodies reach draw submission");
        renderer.set_particles(Vec::new());
        renderer
            .render_frame(
                &camera,
                &vis.saber_hilts(&walk),
                &draws,
                [0.1, 0.1, 0.1, 1.0],
                None,
            )
            .unwrap();
        assert!(
            renderer.frame_stats().submitted > 0,
            "hero draw lies inside capture frustum"
        );
        let without_blades = renderer.capture_rgba().unwrap().0;
        renderer.set_particles(blades);
        renderer
            .render_frame(
                &camera,
                &vis.saber_hilts(&walk),
                &draws,
                [0.1, 0.1, 0.1, 1.0],
                None,
            )
            .unwrap();
        let (pixels, width, height) = renderer.capture_rgba().unwrap();
        let blade_pixels = pixels
            .chunks_exact(4)
            .zip(without_blades.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(blade_pixels > 0, "textured blades reach pixels");
        eprintln!("HERO_BLADE_PIXELS {} {:?} {blade_pixels}", m.map, m.era);
        if let Some(dir) = std::env::var_os("OPENBF_TEST_ARTIFACTS") {
            let path =
                PathBuf::from(dir).join(format!("{}_{}.png", m.map, m.era.as_deref().unwrap()));
            image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
        }
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] != pixels[0] || p[1] != pixels[1] || p[2] != pixels[2]),
            "rendered hero frame contains geometry"
        );
    }
    assert_eq!(names.len(), 5);
}

fn flat_ground() -> core::heightfield::Heightfield {
    core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    }
}
