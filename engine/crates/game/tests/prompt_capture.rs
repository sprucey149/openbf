//! Each button-prompt set in the screens that draw it, for a person to look at:
//! the shell's help lines, the spawn screens, a stats page's Done, and the
//! pause and shell settings over the Button prompts row, at 4:3, 16:9 and 21:9.
//!
//! Ignored by default: it needs a GPU adapter and the BF2 common bake. It writes
//! PNGs and compares nothing.
//!   PROMPT_CAPTURE_OUT=<dir> cargo test -p game --test prompt_capture -- --ignored
//! `PROMPT_CAPTURE_ASSETS=<dir>`: read the bake from there, to see a bake with
//! the Xbox disc's icons in it.

mod common;

use game::input_icons::{set_for, Device, IconChoice};
use ui::input_icons::{IconSet, Icons};

fn stats() -> ui::stats::StatsScreens {
    use ui::stats::*;
    let row = |name: &str, points: i32, me: bool| StatRow {
        name: name.into(),
        points,
        kills: points,
        deaths: 1,
        posts: 0,
        me,
        personal: vec![("Kills".into(), points.to_string())],
    };
    let mut s = StatsScreens::new(
        StatsTexts {
            team_title: "Team Stats".into(),
            personal_title: "Personal Stats".into(),
            done: "Done".into(),
            back: "Back".into(),
            ..Default::default()
        },
        [
            TeamList {
                icon: "cis_icon".into(),
                rows: vec![row("UNIT 001", 4, false), row("UNIT 002", 2, false)],
            },
            TeamList {
                icon: "rep_icon".into(),
                rows: vec![row("PLAYER 1", 3, true), row("RHO 557", 1, false)],
            },
        ],
        [Vec::new(), Vec::new()],
        Career::default(),
    );
    // A bot's page: the one with Done on it.
    s.page = Page::Personal;
    s.list = 0;
    s.row = 0;
    s
}

fn spawn_pages() -> [(&'static str, ui::spawn_draw::SpawnPage); 2] {
    use ui::spawn_draw::SpawnPage;
    [
        (
            "spawn_map",
            SpawnPage::Map {
                objective: "Capture all the Command Posts".into(),
                legend: [
                    "Command Post".into(),
                    "Change Command Post".into(),
                    "{btna} Select Command Post".into(),
                ],
            },
        ),
        (
            "spawn_class",
            SpawnPage::Class {
                title: "Snowtrooper".into(),
                lines: vec!["Unit Count: 7".into()],
                back: "Back".into(),
                accept: "Select Character".into(),
                team: 0,
                class: 0,
                classes: (0..6).collect(),
                refused: 0,
            },
        ),
    ]
}

fn menu(icons: Icons, label: &'static str, shell: bool) -> ui::menu::MenuModel {
    let mut m = ui::menu::MenuModel {
        class_picker_available: false,
        class_choices: None,
        cheats: Vec::new(),
        assist_intensity: 1.0,
        team_size_pct: 100,
        reinforcements_pct: 100,
        population_note: Default::default(),
        team_size_off: false,
        fov_scale: 1.0,
        fov_deg: 37.7,
        music_volume: 1.0,
        mouse_sensitivity: 1.0,
        pad_look_sensitivity: 1.0,
        invert_y: false,
        low_power_resolution: false,
        input_icons_label: label,
        icons,
        first_person: false,
        cursor: 0,
        page: None,
        popup: None,
        restart_available: false,
        texts: Default::default(),
        respawn_label: Some("Respawn".into()),
        outcome_line: None,
        in_shell: shell.then(|| "Game Options".to_owned()),
    };
    m.cursor = ui::menu::rows(&m)
        .iter()
        .position(|r| *r == ui::menu::Row::InputIcons)
        .expect("the menu has the row");
    m
}

/// One frame of the HUD pass, or of `egui` when given, over a mid grey.
fn frame(
    assets: &std::path::Path,
    size: [u32; 2],
    icons: Icons,
    lay_out: impl FnOnce(&mut game::hud_feed::HudView),
    panel: Option<&ui::menu::MenuModel>,
) -> image::RgbaImage {
    let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
    let mut view = game::hud_feed::HudView::for_renderer(assets, &r).expect("the HUD bake");
    view.set_icons(icons);
    lay_out(&mut view);
    let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
    let paint = overlay.begin_headless(size, 1.0, |ctx| {
        if let Some(m) = panel {
            ui::menu::draw_menu(ctx, m);
        }
    });
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.2, 0.2, 0.2, 1.0],
        Some(&mut |dev, q, enc, target, size| {
            view.paint(dev, q, enc, target, size);
            overlay.paint(&paint, dev, q, enc, target, size)
        }),
    )
    .expect("render");
    let (rgba, w, h) = r.capture_rgba().expect("capture");
    image::RgbaImage::from_raw(w, h, rgba).expect("frame")
}

#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake"]
fn capture_every_set() {
    common::force_ps2_tier();
    let out = std::path::PathBuf::from(
        std::env::var_os("PROMPT_CAPTURE_OUT").expect("PROMPT_CAPTURE_OUT=<dir>"),
    );
    std::fs::create_dir_all(&out).expect("out dir");
    let assets = std::env::var_os("PROMPT_CAPTURE_ASSETS")
        .map_or_else(game::assets::baked_root, std::path::PathBuf::from);
    game::shell_movie::load_index(&assets).expect("the shell index");
    let texts = game::event_feed::Texts::load(&assets, "bf2");
    let text = |key: &str, english: &str| {
        texts
            .bf2_text(key)
            .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
    };
    let mut shell = ui::shell_ps2::ShellMenus::new(&text);
    shell.page = ui::shell_ps2::Page::Singleplayer;
    shell.step(ui::menu::MenuInput::default(), false, 0.0);
    shell.on_page = 5.0;
    shell.clock = 0.0;

    // Auto on the GameSir G7 Pro, through the real choice.
    let family = input::family_of(
        "GameSir G7 Pro",
        Some((0x3537, 0x1022)),
        input::PadKind::Sdl,
    );
    let auto = set_for(IconChoice::Auto, Device::Pad(family));
    assert_eq!(auto, IconSet::XboxOne);
    let sets = [
        ("auto_gamesir", auto, IconChoice::Auto),
        ("ds4", IconSet::Ds4, IconChoice::Ds4),
        ("ds5", IconSet::Ds5, IconChoice::Ds5),
        ("ds2", IconSet::Ds2, IconChoice::Ds2),
        (
            "xbox_original",
            IconSet::XboxOriginal,
            IconChoice::XboxOriginal,
        ),
        ("xbox_one", IconSet::XboxOne, IconChoice::XboxOne),
        (
            "keyboard_mouse",
            IconSet::KeyboardMouse,
            IconChoice::KeyboardMouse,
        ),
    ];
    for size in [[640, 480], [1280, 720], [1920, 1080], [1680, 720]] {
        for (name, set, choice) in sets {
            let icons = Icons::of(set);
            let save = |screen: &str, img: image::RgbaImage| {
                img.save(out.join(format!("{name}_{}x{}_{screen}.png", size[0], size[1])))
                    .expect("write");
            };
            save(
                "shell",
                frame(
                    &assets,
                    size,
                    icons,
                    |v| v.update_shell(&shell, false, size),
                    None,
                ),
            );
            for (screen, page) in spawn_pages() {
                let model = ui::hud::HudModel {
                    dead: true,
                    spawn: Some(page),
                    ..Default::default()
                };
                save(
                    screen,
                    frame(&assets, size, icons, |v| v.update(&model, 0.0, size), None),
                );
            }
            let model = ui::hud::HudModel {
                round_end: Some(ui::hud::RoundEndModel {
                    result: 0,
                    stats: Some(stats()),
                }),
                ..Default::default()
            };
            save(
                "stats_done",
                frame(&assets, size, icons, |v| v.update(&model, 0.0, size), None),
            );
            for (screen, in_shell) in [("pause", false), ("settings", true)] {
                let m = menu(icons, choice.label(), in_shell);
                save(screen, frame(&assets, size, icons, |_| {}, Some(&m)));
            }
        }
    }
}
