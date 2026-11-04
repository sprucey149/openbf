//! Issue #114: owned-source, headless presentation regressions.
//! UI_SHARP_ASSETS and UI_SHARP_OUT are mandatory; never writes shared bakes.
use std::path::PathBuf;
use ui::shell::{Group, ListCursor, Page};

const VEHICLE_SHEETS: [&str; 3] = ["droidfighter_icon", "jedi_starfighter_icon", "gunship_icon"];

fn assert_all_sheets_bound(view: &game::hud_feed::HudView) {
    let missing: std::collections::BTreeSet<_> = view.unresolved_sheets().collect();
    assert!(missing.is_empty(), "unresolved HUD sheets: {missing:?}");
}

fn paths() -> (PathBuf, PathBuf) {
    let assets = PathBuf::from(std::env::var_os("UI_SHARP_ASSETS").expect("UI_SHARP_ASSETS"));
    let out = PathBuf::from(std::env::var_os("UI_SHARP_OUT").expect("UI_SHARP_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    (assets, out)
}

fn instant_frame(
    assets: &std::path::Path,
    ia: &ui::shell::InstantAction,
    size: [u32; 2],
) -> Vec<u8> {
    let mut menus = ui::shell_ps2::ShellMenus::new(&|_, e| e.to_owned());
    menus.page = ui::shell_ps2::Page::Singleplayer;
    menus.covered = true;
    let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
    let mut view = game::hud_feed::HudView::for_renderer(assets, &r).unwrap();
    view.update_instant(&menus, ia, false, size);
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.02, 0.02, 0.02, 1.0],
        Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
    )
    .unwrap();
    assert_all_sheets_bound(&view);
    r.capture_rgba().unwrap().0
}

#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn option_values_remain_visible_with_and_without_focus() {
    let (assets, out) = paths();
    let base = game::shell_ia::instant_action(
        &assets,
        &assets.join("../missions"),
        core::skill::Difficulty::Normal,
    );
    let size = [2560, 1440];
    let mut failures = Vec::new();
    for (group, row) in [(Group::Global, 1), (Group::Conquest, 2)] {
        for focused in [true, false] {
            let mut ia = base.clone();
            ia.page = Page::Group(
                group,
                ListCursor {
                    at: if focused { row } else { 0 },
                    top: 0,
                },
            );
            let a = instant_frame(&assets, &ia, size);
            ia.options.difficulty = 4;
            ia.options.con_timer = 30;
            let b = instant_frame(&assets, &ia, size);
            let stem = format!("{group:?}_focused_{focused}");
            for (suffix, rgba) in [("before", &a), ("changed", &b)] {
                std::fs::write(
                    out.join(format!("{stem}_{suffix}.png")),
                    render::encode_png(rgba, size[0], size[1]).unwrap(),
                )
                .unwrap();
            }
            let changed = a
                .chunks_exact(4)
                .zip(b.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            println!("{stem}: {changed} changed pixels");
            if changed < 100 {
                failures.push(format!(
                    "{stem}: changing the option has no visible value ({changed} pixels)"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn save(out: &std::path::Path, name: &str, r: &render::Renderer) {
    let (rgba, w, h) = r.capture_rgba().unwrap();
    std::fs::write(
        out.join(format!("{name}_{w}x{h}.png")),
        render::encode_png(&rgba, w, h).unwrap(),
    )
    .unwrap();
}

/// Before/after capture matrix. The scene is rendered offscreen, with fixed
/// presentation models and a seeded control stepped around each UI update.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn capture_native_surfaces() {
    let (assets, out) = paths();
    let mut scene = game::assets::load_scene(&assets.join("bf1_bes1")).unwrap();
    scene.apply_mode_layer("con");
    let camera = game::viewer::overview_camera(&scene);
    let base = game::shell_ia::instant_action(
        &assets,
        &assets.join("../missions"),
        core::skill::Difficulty::Normal,
    );
    let mut sim = core::sim::Sim::new(42);
    let mut control = core::sim::Sim::new(42);
    for size in [[1920, 1080], [2560, 1440], [3840, 2160]] {
        let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        let gpu = game::viewer::upload_scene(&mut r, &scene).unwrap();
        for (name, group) in [
            ("ia", None),
            ("global", Some((Group::Global, 1))),
            ("conquest", Some((Group::Conquest, 2))),
        ] {
            let mut ia = base.clone();
            if let Some((g, at)) = group {
                ia.page = Page::Group(g, ListCursor { at, top: 0 });
            }
            let rgba = instant_frame(&assets, &ia, size);
            std::fs::write(
                out.join(format!("{name}_{}x{}.png", size[0], size[1])),
                render::encode_png(&rgba, size[0], size[1]).unwrap(),
            )
            .unwrap();
        }
        let mut menus = ui::shell_ps2::ShellMenus::new(&|_, e| e.to_owned());
        menus.page = ui::shell_ps2::Page::Main;
        menus.on_page = 5.0;
        menus.clock = 0.0;
        let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
        let movie = game::shell_movie::ShellMovie::load(&assets).unwrap();
        let (rgba, w, h) = movie.frame_now(menus.movie(), 0, 300).unwrap();
        view.set_movie_frame(rgba, w, h);
        view.update_shell(&menus, true, size);
        r.render_with_overlay(
            &camera,
            &[],
            [0.0, 0.0, 0.0, 1.0],
            Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
        )
        .unwrap();
        assert_all_sheets_bound(&view);
        save(&out, "main", &r);
        let weapon = |key: &str, total| ui::hud::HudWeapon {
            key: key.into(),
            class: String::new(),
            icon: None,
            name: String::new(),
            total: Some(total),
            clip_fraction: 0.7,
            bf2: true,
        };
        let mut map = game::minimap::still(&scene, camera.position, camera.yaw);
        for (sheet, x) in VEHICLE_SHEETS.into_iter().zip([-10.0, 0.0, 10.0]) {
            map.units.push(ui::hud_map::MapIcon {
                pos: [map.eye[0] + x, map.eye[1] - 10.0],
                texture: sheet.into(),
                scale: 2.0,
                color: [255; 3],
                heading: Some(0.0),
                number: None,
            });
        }
        let model = ui::hud::HudModel {
            team_names: ["rep".into(), "cis".into()],
            tickets: [147, 152],
            hp: 220.0,
            hp_max: 300.0,
            energy: 64.0,
            energy_max: 100.0,
            weapon: Some(weapon("rep_weap_inf_rifle", 281)),
            weapon2: Some(weapon("rep_weap_inf_thermaldetonator", 4)),
            reticle: Some(0),
            reticle_y: 0.5,
            gauge: ui::hud::HudGauge {
                ring: 0.45,
                ..Default::default()
            },
            map: Some(map),
            stats: Some(Default::default()),
            ambush: vec![ui::hud_ambush::Edge::Left],
            ..Default::default()
        };
        let spawn = ui::spawn_draw::SpawnPage::Class {
            title: "Clone Trooper".into(),
            lines: vec![
                "Unit Count: 7".into(),
                String::new(),
                "Equipment:".into(),
                "Blaster Rifle".into(),
                "Blaster Pistol".into(),
                "Thermal Detonator".into(),
            ],
            back: "Back".into(),
            accept: "Select Character".into(),
            team: 0,
            class: 0,
            classes: vec![0, 1, 2, 3],
            refused: 0,
        };
        let stats = ui::stats::StatsScreens {
            texts: ui::stats::StatsTexts {
                team_title: "Team Stats".into(),
                awards_title: "Awards".into(),
                done: "Done".into(),
                select: "Select".into(),
                back: "Back".into(),
                ..Default::default()
            },
            lists: [
                ui::stats::TeamList {
                    icon: "rep_icon".into(),
                    rows: vec![ui::stats::StatRow {
                        name: "Player 1".into(),
                        points: 31,
                        kills: 12,
                        deaths: 3,
                        posts: 2,
                        me: true,
                        ..Default::default()
                    }],
                },
                ui::stats::TeamList {
                    icon: "cis_icon".into(),
                    rows: vec![ui::stats::StatRow {
                        name: "Unit 538".into(),
                        points: 20,
                        kills: 8,
                        deaths: 7,
                        ..Default::default()
                    }],
                },
            ],
            ..Default::default()
        };
        for (name, m) in [
            ("hud", model.clone()),
            (
                "spawn",
                ui::hud::HudModel {
                    dead: true,
                    spawn: Some(spawn),
                    ..model.clone()
                },
            ),
            (
                "round_end",
                ui::hud::HudModel {
                    round_end: Some(ui::hud::RoundEndModel::default()),
                    ..model.clone()
                },
            ),
            (
                "stats",
                ui::hud::HudModel {
                    round_end: Some(ui::hud::RoundEndModel {
                        result: 0,
                        stats: Some(stats),
                    }),
                    ..model.clone()
                },
            ),
        ] {
            let before = sim.hash();
            let mut hud = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
            hud.update(&m, 0.0, size);
            hud.update(&m, 0.6, size);
            r.render_with_overlay(
                &camera,
                &gpu.draws,
                gpu.clear,
                Some(&mut |dev, q, enc, target, size| hud.paint(dev, q, enc, target, size)),
            )
            .unwrap();
            assert_eq!(
                before,
                sim.hash(),
                "{name}: presentation mutated simulation"
            );
            sim.step(&[]);
            control.step(&[]);
            assert_eq!(sim.hash(), control.hash());
            assert_all_sheets_bound(&hud);
            if name == "hud" {
                for sheet in VEHICLE_SHEETS {
                    assert!(hud.drawn_sheets().any(|drawn| drawn == sheet), "{sheet}");
                }
            }
            save(&out, name, &r);
            std::fs::write(
                out.join(format!("{name}_sheets.txt")),
                hud.drawn_sheets().collect::<Vec<_>>().join("\n"),
            )
            .unwrap();
        }
    }
}
