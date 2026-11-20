//! Typography and scrolling captures (`ui::typography`, `ui::scroll`):
//! every menu that draws text, at the window size `UI_SHARP_SIZE`
//! (default 1920x1080), headless. UI_SHARP_ASSETS and UI_SHARP_OUT are
//! mandatory; never writes the bake.
#![allow(clippy::disallowed_methods)]
use std::path::PathBuf;
use ui::menu::{MenuModel, PausePage, Popup, SettingsPage};

fn paths() -> (PathBuf, PathBuf) {
    let assets = PathBuf::from(std::env::var_os("UI_SHARP_ASSETS").expect("UI_SHARP_ASSETS"));
    let out = PathBuf::from(std::env::var_os("UI_SHARP_OUT").expect("UI_SHARP_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    (assets, out)
}

fn save(out: &std::path::Path, name: &str, r: &render::Renderer) {
    let (rgba, w, h) = r.capture_rgba().unwrap();
    std::fs::write(
        out.join(format!("{name}.png")),
        render::encode_png(&rgba, w, h).unwrap(),
    )
    .unwrap();
}

fn model() -> MenuModel {
    MenuModel {
        page: Some(PausePage::Main),
        respawn_label: Some("Respawn".into()),
        restart_available: true,
        team_size_pct: 100,
        reinforcements_pct: 100,
        fov_scale: 1.0,
        base_fov_deg: 60.0,
        music_volume: 0.6,
        effects_volume: 0.8,
        dialogue_volume: 0.8,
        difficulty: 2,
        mouse_sensitivity: 1.0,
        pad_look_sensitivity: 1.0,
        rumble_strength: 1.0,
        input_icons_label: "Auto",
        player_name: "Player".into(),
        cheats: core::sim::Cheats::default().entries().to_vec(),
        ..Default::default()
    }
}

#[test]
#[ignore = "GPU"]
fn typography_captures() {
    let (assets, out) = paths();
    let size: [u32; 2] = std::env::var("UI_SHARP_SIZE")
        .ok()
        .and_then(|s| {
            s.split_once('x')
                .map(|(w, h)| [w.parse().unwrap(), h.parse().unwrap()])
        })
        .unwrap_or([1920, 1080]);
    let texts = game::event_feed::Texts::load(&assets, "bf2");
    let text = |key: &str, english: &str| {
        texts
            .bf2_text(key)
            .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
    };
    let base = ui::shell_ps2::ShellMenus::new(&text);
    let movie = game::shell_movie::ShellMovie::load(&assets).unwrap();
    let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
    let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
    let paint = |r: &mut render::Renderer, view: &mut game::hud_feed::HudView| {
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.15, 0.2, 0.25, 1.0],
            Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
        )
        .unwrap();
    };
    // Shell stacks.
    for (name, page) in [
        ("shell_main", ui::shell_ps2::Page::Main),
        ("shell_single", ui::shell_ps2::Page::Singleplayer),
        ("shell_options", ui::shell_ps2::Page::Options),
    ] {
        let mut m = base.clone();
        m.page = page;
        m.step(ui::menu::MenuInput::default(), false, 0.0);
        m.on_page = 5.0;
        m.clock = 0.0;
        let (rgba, w, h) = movie.frame_now(m.movie(), 0, 150).unwrap();
        view.set_movie_frame(rgba, w, h);
        view.update_shell(&m, true, size);
        paint(&mut r, &mut view);
        save(&out, name, &r);
    }
    // Instant Action.
    let ia = game::shell_ia::instant_action(
        &assets,
        &assets.join("../missions"),
        core::skill::Difficulty::Normal,
    );
    let mut menus = base.clone();
    menus.page = ui::shell_ps2::Page::Singleplayer;
    menus.covered = true;
    for (name, page) in [
        ("ia_missions", ui::shell::Page::Missions),
        ("ia_planets", ui::shell::Page::Missions),
        ("ia_session", ui::shell::Page::Overview(0)),
        (
            "ia_global",
            ui::shell::Page::Group(ui::shell::Group::Global, ui::shell::ListCursor::new(1)),
        ),
    ] {
        let mut ia = ia.clone();
        ia.page = page;
        if name == "ia_planets" {
            // The planet list focused, keys moved well down it: the view
            // keeps a row below the cursor and eases there.
            ia.state = ui::shell::State::Map;
            let down = ui::shell::IaInput {
                menu: ui::menu::MenuInput {
                    down: true,
                    ..Default::default()
                },
                ..Default::default()
            };
            for _ in 0..12 {
                ia.step(down, 0.0);
            }
            for _ in 0..60 {
                ia.step(Default::default(), 1.0 / 60.0);
            }
        }
        view.update_instant(&menus, &ia, false, size);
        paint(&mut r, &mut view);
        save(&out, name, &r);
    }
    // Profiles.
    let mut sm = base.clone();
    sm.page = ui::shell_ps2::Page::Main;
    sm.covered = true;
    let picker = ui::shell_ps2::profiles::Picker::new(
        vec![(1, "the maintainer".into()), (2, "Kai".into())],
        Some(1),
    );
    view.update_shell_profiles(&sm, false, &picker, size);
    paint(&mut r, &mut view);
    save(&out, "profiles", &r);

    // Settings pages, battle and shell.
    let mut animation = ui::pause_draw::Animation::default();
    animation.advance(2.0);
    let hud = ui::hud::HudModel::default();
    let m0 = model();
    let pages: Vec<(&str, MenuModel)> = vec![
        ("pause_main", m0.clone()),
        (
            "pause_options",
            MenuModel {
                page: Some(PausePage::Options),
                ..m0.clone()
            },
        ),
        (
            "pause_game",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::Game)),
                ..m0.clone()
            },
        ),
        (
            "pause_audio",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::Audio)),
                ..m0.clone()
            },
        ),
        (
            "pause_graphics",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::Graphics)),
                ..m0.clone()
            },
        ),
        (
            "pause_graphics_c5",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::Graphics)),
                cursor: 6,
                ..m0.clone()
            },
        ),
        (
            "pause_cheats",
            MenuModel {
                page: Some(PausePage::Cheats),
                cursor: 2,
                ..m0.clone()
            },
        ),
        (
            "pause_cheats_c8",
            MenuModel {
                page: Some(PausePage::Cheats),
                cursor: 8,
                ..m0.clone()
            },
        ),
        (
            "pause_controls",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::Controls)),
                ..m0.clone()
            },
        ),
        (
            "pause_kbm",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::KeyboardMouse)),
                ..m0.clone()
            },
        ),
        (
            "pause_gamepad",
            MenuModel {
                page: Some(PausePage::Settings(SettingsPage::Gamepad)),
                ..m0.clone()
            },
        ),
        (
            "pause_quit",
            MenuModel {
                popup: Some(Popup::Quit),
                ..m0.clone()
            },
        ),
        (
            "pause_bindings",
            MenuModel {
                page: None,
                bindings: Some(ui::menu::BindingRows {
                    title: "Infantry Controls",
                    labels: [
                        ("Move Forward", "W"),
                        ("Move Back", "S"),
                        ("Strafe Left", "A"),
                        ("Strafe Right", "D"),
                        ("Jump", "Space"),
                        ("Crouch", "Left Ctrl"),
                        ("Sprint", "Left Shift"),
                        ("Fire", "Left Mouse Button"),
                        ("Secondary Fire", "Right Mouse Button"),
                        ("Reload", "R"),
                        ("Next Weapon", "Mouse Wheel Up"),
                        ("Previous Weapon", "Mouse Wheel Down"),
                        ("Zoom", "Z"),
                        ("Use / Enter Vehicle", "E"),
                    ]
                    .iter()
                    .map(|(a, b)| (a.to_string(), b.to_string()))
                    .collect(),
                    capture: false,
                    message: None,
                }),
                cursor: 7,
                ..m0.clone()
            },
        ),
    ];
    for (name, state) in &pages {
        let mut animation = ui::pause_draw::Animation::default();
        animation.advance(2.0);
        view.update_pause(&hud, state, &animation, size);
        paint(&mut r, &mut view);
        save(&out, name, &r);
    }
    // Scrolling: Cheats with the cursor walked to its last row by keys (the
    // view keeps the row in sight, never recentres), then the wheel half a
    // row up, captured mid-ease (rows cut at the list's window).
    let fsize = [size[0] as f32, size[1] as f32];
    let mut animation = ui::pause_draw::Animation::default();
    animation.advance(2.0);
    let mut m = MenuModel {
        page: Some(PausePage::Cheats),
        ..m0.clone()
    };
    let last = ui::menu::rows(&m).len() - 1;
    for c in 0..=last {
        m.cursor = c;
        view.update_pause(&hud, &m, &animation, size);
        for _ in 0..3 {
            animation.advance(1.0 / 60.0);
        }
    }
    for _ in 0..60 {
        animation.advance(1.0 / 60.0);
    }
    view.update_pause(&hud, &m, &animation, size);
    paint(&mut r, &mut view);
    save(&out, "scroll_cheats_end", &r);
    ui::pause_draw::scroll(&m, &animation, 1.5, fsize);
    animation.advance(1.0 / 60.0);
    animation.advance(1.0 / 60.0);
    view.update_pause(&hud, &m, &animation, size);
    paint(&mut r, &mut view);
    save(&out, "scroll_cheats_wheel_mid", &r);
    // A click on a shown row selects it and leaves the list where it is.
    for _ in 0..60 {
        animation.advance(1.0 / 60.0);
    }
    let before = animation.scroll_offset();
    let row = before.ceil() as usize + 1;
    let _ = ui::pause_draw::click(&mut m, &animation, ui::pause_draw::Target::Row(row));
    view.update_pause(&hud, &m, &animation, size);
    for _ in 0..60 {
        animation.advance(1.0 / 60.0);
    }
    view.update_pause(&hud, &m, &animation, size);
    assert_eq!(m.cursor, row);
    assert!(
        (animation.scroll_offset() - before).abs() < 1e-3,
        "{before} -> {}",
        animation.scroll_offset()
    );
    paint(&mut r, &mut view);
    save(&out, "scroll_cheats_clicked", &r);
    // The in-battle binding list, driven as the battle loop drives it
    // (`game::pause::binding_list_frame` every frame): keys walk the
    // selection to the last row, the view follows; then the wheel.
    let mut animation = ui::pause_draw::Animation::default();
    animation.advance(2.0);
    let mut labels: Vec<(String, String)> = vec![
        ("Back".into(), String::new()),
        ("Restore Defaults".into(), String::new()),
    ];
    labels.extend((1..=16).map(|i| (format!("Action {i}"), format!("Key {i}"))));
    let mut b = MenuModel {
        page: None,
        bindings: Some(ui::menu::BindingRows {
            title: "Infantry Controls",
            labels,
            capture: false,
            message: None,
        }),
        ..m0.clone()
    };
    let last = ui::menu::rows(&b).len() - 1;
    for c in 0..=last {
        b.cursor = c;
        for _ in 0..4 {
            game::pause::binding_list_frame(
                Some(&b),
                &mut animation,
                0.0,
                None,
                Some(fsize),
                1.0 / 60.0,
            );
        }
    }
    for _ in 0..60 {
        game::pause::binding_list_frame(
            Some(&b),
            &mut animation,
            0.0,
            None,
            Some(fsize),
            1.0 / 60.0,
        );
    }
    assert!(ui::pause_draw::row_in_view(&b, &animation, last, fsize));
    view.update_pause(&hud, &b, &animation, size);
    paint(&mut r, &mut view);
    save(&out, "bindings_battle_end", &r);
    game::pause::binding_list_frame(Some(&b), &mut animation, 2.5, None, Some(fsize), 1.0 / 60.0);
    for _ in 0..60 {
        game::pause::binding_list_frame(
            Some(&b),
            &mut animation,
            0.0,
            None,
            Some(fsize),
            1.0 / 60.0,
        );
    }
    view.update_pause(&hud, &b, &animation, size);
    paint(&mut r, &mut view);
    save(&out, "bindings_battle_wheel", &r);
    let mut sm = base.clone();
    sm.page = ui::shell_ps2::Page::Options;
    sm.on_page = 5.0;
    let (rgba, w, h) = movie.frame_now(sm.movie(), 0, 150).unwrap();
    view.set_movie_frame(rgba, w, h);
    for (name, page) in [
        (
            "shell_set_graphics",
            PausePage::Settings(SettingsPage::Graphics),
        ),
        ("shell_set_game", PausePage::Settings(SettingsPage::Game)),
        ("shell_set_cheats", PausePage::Cheats),
    ] {
        let m = MenuModel {
            page: Some(page),
            in_shell: Some(String::new()),
            ..m0.clone()
        };
        view.update_shell_options(&sm, true, &m, &animation, size);
        paint(&mut r, &mut view);
        save(&out, name, &r);
    }

    // egui surfaces, over the shell movie frame.
    let egui_shot = |name: &str, f: &dyn Fn(&ui::egui::Context)| {
        let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).unwrap();
        let mut m = base.clone();
        m.page = ui::shell_ps2::Page::Main;
        m.covered = true;
        let (rgba, w, h) = movie.frame_now(m.movie(), 0, 150).unwrap();
        view.set_movie_frame(rgba, w, h);
        view.update_shell(&m, true, size);
        let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
        ui::typography::face::install(overlay.ctx(), &game::hud_feed::menu_face(&assets));
        let p = overlay.begin_headless(size, 1.0, |ctx| f(ctx));
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.15, 0.2, 0.25, 1.0],
            Some(&mut |dev, q, enc, target, size| {
                view.paint(dev, q, enc, target, size);
                overlay.paint(&p, dev, q, enc, target, size)
            }),
        )
        .unwrap();
        save(&out, name, &r);
    };
    egui_shot("egui_lan_host", &|ctx| {
        let rows = vec![
            ui::lobby::PageRow {
                label: "Map".into(),
                value: Some("Hoth: Echo Base".into()),
                enabled: true,
                text: false,
            },
            ui::lobby::PageRow {
                label: "Mode".into(),
                value: Some("Conquest".into()),
                enabled: true,
                text: false,
            },
            ui::lobby::PageRow {
                label: "Player name".into(),
                value: Some("Player".into()),
                enabled: true,
                text: true,
            },
            ui::lobby::PageRow {
                label: "Start".into(),
                value: None,
                enabled: true,
                text: false,
            },
            ui::lobby::PageRow {
                label: "Back".into(),
                value: None,
                enabled: true,
                text: false,
            },
        ];
        let lines = vec![(
            "Waiting for players on 192.168.1.4".to_string(),
            ui::egui::Color32::LIGHT_GRAY,
        )];
        ui::lobby::draw_page(ctx, "LAN: Host Game", &rows, 1, None, false, &lines);
    });
    egui_shot("egui_roster", &|ctx| {
        let rows = vec![
            ui::lobby::RosterRow {
                slot: 0,
                name: "Host".into(),
                ping_ms: None,
                addon: false,
                team: "Republic".into(),
                ready: true,
            },
            ui::lobby::RosterRow {
                slot: 1,
                name: "Kai".into(),
                ping_ms: Some(42),
                addon: false,
                team: "CIS".into(),
                ready: false,
            },
        ];
        let mut nav = ui::lobby::RosterNav::default();
        let prompts = ui::lobby::RosterPrompts {
            open: "Tab".into(),
            accept: "Enter".into(),
            back: "Backspace".into(),
        };
        ui::lobby::draw_roster(
            ctx,
            "Players",
            &rows,
            true,
            &["Kai joined".to_string()],
            &mut nav,
            &prompts,
        );
        ui::lobby::draw_status(ctx, "Waiting for the host to start");
    });
    egui_shot("egui_name_box", &|ctx| {
        let mut name = "Player 3".to_string();
        ui::lobby::draw_name_box(ctx, &mut name, Some("Names are 1 to 16 characters"));
    });
    egui_shot("egui_flat_menu", &|ctx| {
        let m = MenuModel {
            page: Some(PausePage::Cheats),
            cursor: 2,
            ..model()
        };
        ui::menu::draw_menu(ctx, &m);
        ui::menu::draw_fps_counter(ctx, ui::menu::FpsCounter::On, 1.0 / 60.0);
    });
    // The LAN net meter's lines in its role (`READOUT`, as
    // `main::windowed::lan::draw_meter` draws them), with the FPS counter.
    egui_shot("egui_readouts", &|ctx| {
        ui::menu::draw_fps_counter_note(ctx, ui::menu::FpsCounter::On, 1.0 / 60.0, Some("res 85%"));
        ui::egui::Area::new(ui::egui::Id::new("net_meter"))
            .anchor(ui::egui::Align2::RIGHT_TOP, ui::egui::vec2(-8.0, 8.0))
            .show(ctx, |ui| {
                ui::egui::Frame::new()
                    .fill(ui::egui::Color32::from_black_alpha(180))
                    .inner_margin(ui::egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        for line in [
                            "host 192.168.1.4  ping 42 ms  jitter 3 ms",
                            "input delay 3 ticks (50 ms)  queue 2 starved 0  slack 1.2 ms",
                            "peers 3  bundles 30/s  resends 0",
                            "quality: auto 85%",
                        ] {
                            ui.add(
                                ui::egui::Label::new(ui::typography::egui_text(
                                    ctx,
                                    &ui::typography::READOUT,
                                    ui::typography::tint::HELP,
                                    line,
                                ))
                                .wrap_mode(ui::egui::TextWrapMode::Extend),
                            );
                        }
                    });
            });
    });
    egui_shot("egui_hud_lines", &|ctx| {
        let m = ui::hud::HudModel {
            time_left: Some(312.0),
            outcome: Some(0),
            team_names: ["rep".into(), "cis".into()],
            ..Default::default()
        };
        ui::hud::draw_hud(ctx, &m);
        ui::hud::draw_pad_warning(ctx, "Controller: the camera stick is not mapped");
    });
}
