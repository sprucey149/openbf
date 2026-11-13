//! Issue #114: owned-source, headless presentation regressions.
//! UI_SHARP_ASSETS and UI_SHARP_OUT are mandatory; never writes shared bakes.
use std::path::PathBuf;
use ui::shell::{Group, ListCursor, Page};

#[path = "ui_sharp/cutter.rs"]
mod cutter;

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

/// The production loader's treated font drives both the draw and pointer path.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn menu_font_pointer_uses_loaded_metrics() {
    let (assets, _) = paths();
    let mut ia = game::shell_ia::instant_action(
        &assets,
        &assets.join("../missions"),
        core::skill::Difficulty::Normal,
    );
    ia.state = ui::shell::State::Map;
    let font: manifest::hud::HudFont = ron::from_str(
        &std::fs::read_to_string(assets.join("common_bf2/fonts/menu_gamefont_large.ron")).unwrap(),
    )
    .unwrap();
    let mut menus = ui::shell_ps2::ShellMenus::new(&|_, english| english.to_owned());
    menus.page = ui::shell_ps2::Page::Main;
    menus.on_page = 5.0;
    for size in [[640, 480], [1920, 1080], [3580, 2020]] {
        let r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        let view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
        let screen = size.map(|v| v as f32);
        let mut targets = std::collections::BTreeSet::new();
        let mut shell_hits = 0;
        // Sample the actual screen, comparing the production loader to the
        // explicitly baked role. Includes row interiors, edges and empty space.
        for state in [ui::shell::State::Map, ui::shell::State::Command] {
            ia.state = state;
            for y in (0..size[1]).step_by(13) {
                for x in (0..size[0]).step_by(13) {
                    let point = [x as f32, y as f32];
                    let expected = ui::shell_ia::pointer_target(&ia, point, screen, &font);
                    let actual = view.instant_target(&ia, point, screen);
                    assert_eq!(actual, expected, "pointer at {point:?} / {size:?}");
                    let expected_shell = ui::shell_ps2::row_at(&font, &menus, point, screen);
                    assert_eq!(
                        view.shell_row_at(&menus, point, screen),
                        expected_shell,
                        "shell pointer at {point:?} / {size:?}"
                    );
                    shell_hits += usize::from(expected_shell.is_some());
                    if let Some(target) = actual {
                        targets.insert(format!("{target:?}"));
                    }
                }
            }
        }
        assert!(
            targets.iter().any(|t| t.starts_with("Map(")),
            "no map row hit"
        );
        assert!(
            targets.iter().any(|t| t.starts_with("Command(")),
            "no command hit"
        );
        assert!(shell_hits > 0, "no shell row hit");
    }
}

/// Scoped shell-art registration capture, including the reported window size.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn capture_instant_action_registration() {
    let (assets, out) = paths();
    let ia = game::shell_ia::instant_action(
        &assets,
        &assets.join("../missions"),
        core::skill::Difficulty::Normal,
    );
    for size in [[1920, 1080], [3580, 2020]] {
        let rgba = instant_frame(&assets, &ia, size);
        std::fs::write(
            out.join(format!("ia_{}x{}.png", size[0], size[1])),
            render::encode_png(&rgba, size[0], size[1]).unwrap(),
        )
        .unwrap();
    }
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

/// Exercise the longest real catalog labels, all option groups and extreme
/// population/reinforcement values using the ordinary compact menu paths.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake with shared menu fonts"]
fn capture_menu_label_extremes() {
    let (assets, out) = paths();
    let mut ia = game::shell_ia::instant_action(
        &assets,
        &assets.join("../missions"),
        core::skill::Difficulty::Normal,
    );
    let font: manifest::hud::HudFont = ron::from_str(
        &std::fs::read_to_string(assets.join("common_bf2/fonts/menu_gamefont_tiny.ron")).unwrap(),
    )
    .unwrap();
    let width = |text: &str| {
        text.chars()
            .map(|c| {
                f32::from(
                    font.glyphs
                        .iter()
                        .find(|g| u32::from(g.code) == c as u32)
                        .unwrap_or_else(|| panic!("missing menu glyph {c:?}"))
                        .advance,
                )
            })
            .sum::<f32>()
            * ui::spawn_draw::MAP_STRETCH
    };
    ia.maps
        .sort_by(|a, b| width(&b.label.to_uppercase()).total_cmp(&width(&a.label.to_uppercase())));
    for row in ia.map_rows() {
        println!("menu label width {:.3}: {}", width(&row.text), row.text);
    }
    ia.options.num_bots = 500;
    ia.options.con_numbots = 500;
    ia.options.con_mult = 500;
    ia.options.con_timer = 60;
    ia.options.difficulty = 4;
    let small: manifest::hud::HudFont = ron::from_str(
        &std::fs::read_to_string(assets.join("common_bf2/fonts/menu_gamefont_small.ron")).unwrap(),
    )
    .unwrap();
    for population in [10, 100, 500] {
        let mut options = ia.options;
        options.num_bots = population;
        options.con_numbots = population;
        for group in [
            Group::Global,
            Group::Hero,
            Group::Conquest,
            Group::Ctf,
            Group::Hunt,
            Group::Assault,
        ] {
            for option in options.rows(group) {
                let pieces = ia.texts.option(option, &options);
                let w: f32 = pieces
                    .iter()
                    .map(|p| match p {
                        ui::shell::Piece::Text(text) => {
                            text.chars()
                                .map(|c| {
                                    f32::from(
                                        small
                                            .glyphs
                                            .iter()
                                            .find(|g| u32::from(g.code) == c as u32)
                                            .unwrap()
                                            .advance,
                                    )
                                })
                                .sum::<f32>()
                                * ui::spawn_draw::SMALL_STRETCH
                        }
                        _ => 14.0,
                    })
                    .sum();
                println!("option width {w:.3}: {population}% {group:?} {option:?}");
                assert!(w <= 363.6, "{group:?} {option:?}: {w} exceeds compact row");
            }
        }
    }
    for size in [[1920, 1080], [2560, 1440], [3840, 2160]] {
        for (label, page) in [
            ("longest_maps", Page::Missions),
            ("overview", Page::Overview(0)),
            (
                "extreme_global",
                Page::Group(Group::Global, ListCursor { at: 0, top: 0 }),
            ),
            (
                "extreme_conquest",
                Page::Group(Group::Conquest, ListCursor { at: 1, top: 0 }),
            ),
        ] {
            ia.page = page;
            let rgba = instant_frame(&assets, &ia, size);
            std::fs::write(
                out.join(format!("{label}_{}x{}.png", size[0], size[1])),
                render::encode_png(&rgba, size[0], size[1]).unwrap(),
            )
            .unwrap();
        }
    }
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

#[path = "support/hud_layout.rs"]
mod shared_hud_layout;

/// The same production model as the CPU source parity check, using native art.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn shared_conquest_hud_geometry() {
    let (assets, out) = paths();
    for game in ["bf1", "bf2"] {
        for size in shared_hud_layout::SIZES {
            let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
            for state in 0..3 {
                let model = shared_hud_layout::model(&assets, game, state);
                let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
                shared_hud_layout::settle(&mut view, &model, state, size);
                r.render_with_overlay(
                    &render::Camera::default(),
                    &[],
                    [0.1, 0.1, 0.1, 1.0],
                    Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
                )
                .unwrap();
                assert_all_sheets_bound(&view);
                save(&out, &format!("shared_{game}_{state}"), &r);
                let bounds = shared_hud_layout::ELEMENTS
                    .iter()
                    .map(|name| (*name, view.bounds(name)))
                    .collect::<std::collections::BTreeMap<_, _>>();
                std::fs::write(
                    out.join(format!(
                        "shared_{game}_{state}_{}x{}.json",
                        size[0], size[1]
                    )),
                    serde_json::to_vec_pretty(&bounds).unwrap(),
                )
                .unwrap();
            }
        }
    }
}

/// Exercise the baked pause frame and its hit regions at desktop and handheld sizes.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn capture_battle_pause_theme() {
    use ui::menu::{MenuAction, MenuInput, MenuModel, PausePage, Popup, Row, SettingsPage};
    use ui::pause_draw::{self, Animation, Target};

    let (assets, out) = paths();
    for size in [[640, 480], [1280, 720], [2560, 1080], [540, 960]] {
        let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
        assert!(view.can_draw_pause());
        let hud = ui::hud::HudModel::default();
        let mut animation = Animation::default();
        let mut menu = MenuModel {
            page: Some(PausePage::Main),
            respawn_label: Some("Respawn".into()),
            restart_available: true,
            team_size_pct: 100,
            reinforcements_pct: 100,
            fov_scale: 1.0,
            fov_deg: 60.0,
            music_volume: 0.6,
            mouse_sensitivity: 1.0,
            pad_look_sensitivity: 1.0,
            rumble_strength: 1.0,
            input_icons_label: "Auto",
            ..Default::default()
        };

        let hit = |menu: &MenuModel, anim: &Animation, wanted| {
            let frame = size.map(|n| n as f32);
            (0..size[1]).step_by(5).any(|y| {
                (0..size[0]).step_by(5).any(|x| {
                    pause_draw::target(menu, anim, [x as f32, y as f32], frame) == Some(wanted)
                })
            })
        };
        assert!(!hit(&menu, &animation, Target::Row(4)));
        animation.advance(1.0);
        for (name, state) in [
            ("pause_game_menu", menu.clone()),
            (
                "pause_categories",
                MenuModel {
                    page: Some(PausePage::Options),
                    ..menu.clone()
                },
            ),
            (
                "pause_controls",
                MenuModel {
                    page: Some(PausePage::Settings(SettingsPage::Controls)),
                    ..menu.clone()
                },
            ),
            (
                "pause_options",
                MenuModel {
                    page: Some(PausePage::Settings(SettingsPage::Game)),
                    career_awards_disabled: true,
                    cursor: 4,
                    ..menu.clone()
                },
            ),
            (
                "pause_quit",
                MenuModel {
                    popup: Some(Popup::Quit),
                    ..menu.clone()
                },
            ),
            (
                "pause_graphics",
                MenuModel {
                    page: Some(PausePage::Settings(SettingsPage::Graphics)),
                    cursor: 2,
                    ..menu.clone()
                },
            ),
            (
                "pause_gamepad",
                MenuModel {
                    page: Some(PausePage::Settings(SettingsPage::Gamepad)),
                    rumble_strength: 0.67,
                    rumble_on_fire: true,
                    cursor: 4,
                    ..menu.clone()
                },
            ),
            (
                "pause_binding_scroll",
                MenuModel {
                    page: None,
                    bindings: Some(ui::menu::BindingRows {
                        title: "Infantry Controls",
                        labels: (1..=18)
                            .map(|i| (format!("Action {i}"), format!("Button {i}")))
                            .collect(),
                        capture: false,
                        message: None,
                    }),
                    cursor: 9,
                    ..menu.clone()
                },
            ),
        ] {
            view.update_pause(&hud, &state, &animation, size);
            r.render_with_overlay(
                &render::Camera::default(),
                &[],
                [0.15, 0.2, 0.25, 1.0],
                Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
            )
            .unwrap();
            assert_all_sheets_bound(&view);
            assert!(view
                .drawn_sheets()
                .any(|sheet| sheet == "bf2_buttons_title_center"));
            assert!(view
                .drawn_sheets()
                .any(|sheet| sheet == "bf2_buttons_botright"));
            assert!(view.drawn_sheets().any(|sheet| sheet.contains("menu_face")));
            save(&out, name, &r);
        }
        assert!(hit(&menu, &animation, Target::Row(4)));
        let (_, actions) = pause_draw::click(&mut menu, Target::Row(1));
        assert_eq!(actions, vec![MenuAction::OpenPage(PausePage::Options)]);
        menu.page = Some(PausePage::Options);
        let (_, actions) = pause_draw::click(&mut menu, Target::Row(1));
        assert_eq!(
            actions,
            [MenuAction::OpenPage(PausePage::Settings(
                SettingsPage::Game
            ))]
        );
        menu.page = Some(PausePage::Settings(SettingsPage::Game));
        let row = ui::menu::rows(&menu)
            .iter()
            .position(|r| *r == Row::CareerAwards)
            .unwrap();
        let disable = Target::Step { row, right: false };
        assert!(hit(&menu, &animation, disable));
        let (_, actions) = pause_draw::click(&mut menu, disable);
        assert_eq!(actions, vec![MenuAction::SetCareerAwards(false)]);
        menu.popup = Some(Popup::Quit);
        menu.cursor = 0;
        let (_, actions) = ui::menu::navigate(
            &menu,
            MenuInput {
                confirm: true,
                ..Default::default()
            },
        );
        assert_eq!(actions, vec![MenuAction::CancelPopup]);
    }
}

#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn capture_personal_stats_medals() {
    let (assets, out) = paths();
    let size = [1280, 720];
    let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
    let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
    let mut stats = ui::stats::StatsScreens {
        page: ui::stats::Page::Personal,
        texts: ui::stats::StatsTexts {
            personal_title: "Personal Stats".into(),
            medals_title: "Medals".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    stats.lists[0].rows.push(ui::stats::StatRow {
        name: "Player 1".into(),
        me: true,
        personal: vec![("Kills".into(), "66".into()), ("Deaths".into(), "9".into())],
        ..Default::default()
    });
    let mut paint = |stats: &ui::stats::StatsScreens| {
        view.update(
            &ui::hud::HudModel {
                round_end: Some(ui::hud::RoundEndModel {
                    stats: Some(stats.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            0.0,
            size,
        );
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.0, 0.0, 0.0, 1.0],
            Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
        )
        .unwrap();
        assert_all_sheets_bound(&view);
        assert!(view.drawn_sheets().any(|sheet| sheet == "listbox_cursor"));
        r.capture_rgba().unwrap().0
    };
    let blank = paint(&stats);
    stats.lists[0].rows[0].medals = vec![("Endurance".into(), 2), ("Guardian".into(), 1)];
    let earned = paint(&stats);
    let changed = |x0, x1| {
        (430..550)
            .flat_map(|y| (x0..x1).map(move |x| 4 * (y * size[0] as usize + x)))
            .filter(|&i| earned[i..i + 4] != blank[i..i + 4])
            .count()
    };
    assert!(changed(200, 640) > 100);
    assert!(changed(640, 1000) > 100);
    save(&out, "personal_stats_medals", &r);
}
