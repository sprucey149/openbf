//! Every screen at 4:3, 16:9 and 21:9, for a person to look at (the HUD, menu and
//! audio cleanse). The button prompt sets are `tests/prompt_capture.rs`'s job; this
//! takes the rest: the shell's front pages, Instant Action's pages, the HUD in play
//! with long event lines, the droideka's shared bar in both forms, the stats pages,
//! the controls list and the loading caption.
//!
//! Ignored by default: it needs a GPU adapter and the BF2 common bake. It writes
//! PNGs and compares nothing.
//!   UI_CAPTURE_OUT=<dir> cargo test -p game --test ui_capture -- --ignored

mod common;

use ui::hud::{message_color, HudMessage, HudModel, HudWeapon};
use ui::input_icons::{IconSet, Icons};

const SIZES: [(&str, [u32; 2]); 3] = [
    ("4x3", [640, 480]),
    ("16x9", [1280, 720]),
    ("21x9", [1680, 720]),
];

fn out_dir() -> std::path::PathBuf {
    let out = std::path::PathBuf::from(std::env::var_os("UI_CAPTURE_OUT").expect("UI_CAPTURE_OUT"));
    std::fs::create_dir_all(&out).expect("out dir");
    out
}

/// What the egui pass draws.
enum Panel {
    None,
    Hud(Box<HudModel>),
    Controls(Vec<(String, String)>),
    Loading,
}

/// A frame of the HUD pass under the egui pass.
fn frame(
    size: [u32; 2],
    lay_out: impl FnOnce(&mut game::hud_feed::HudView),
    panel: Panel,
) -> image::RgbaImage {
    let assets = game::assets::baked_root();
    let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
    let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).expect("the HUD bake");
    view.set_icons(Icons::of(IconSet::Ds2));
    lay_out(&mut view);
    let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
    let paint = overlay.begin_headless(size, 1.0, |ctx| match &panel {
        Panel::None => {}
        Panel::Hud(m) => ui::hud::draw_hud(ctx, m),
        Panel::Controls(rows) => {
            ui::menu::draw_controls(
                ctx,
                rows,
                3,
                false,
                Some("Binding saved; swapped with Secondary Fire"),
            );
        }
        Panel::Loading => ui::loading::draw_loading(
            ctx,
            None,
            "Loading Hoth: terrain and a long caption of what is loading",
            3,
            9,
        ),
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

fn rifle() -> HudWeapon {
    HudWeapon {
        key: "all_weap_inf_rifle".into(),
        class: String::new(),
        icon: None,
        name: String::new(),
        total: Some(281),
        clip_fraction: 31.0 / 50.0,
        bf2: true,
    }
}

fn base() -> HudModel {
    HudModel {
        team_names: ["all".into(), "imp".into()],
        tickets: [247, 239],
        hp: 220.0,
        hp_max: 300.0,
        energy: 64.0,
        energy_max: 100.0,
        weapon: Some(rifle()),
        reticle_y: game::walk::ps2_reticle_y("bf2"),
        stats: Some(ui::hud::HudStats::default()),
        ..Default::default()
    }
}

/// The event lines at their longest: the discs' longest names, a team kill.
fn lines() -> Vec<HudMessage> {
    [
        (
            "Confederacy of Independent Systems captured the Command Center",
            message_color::ENEMY,
        ),
        (
            "Ben Pulastra killed Commander Cody Clone 12 - [TK]",
            message_color::OWN,
        ),
        ("Super Battle Droid 44 killed Player 1", message_color::OWN),
        ("Republic lost the Landing Platform", message_color::OWN),
    ]
    .into_iter()
    .map(|(text, color)| HudMessage {
        text: text.into(),
        color,
    })
    .collect()
}

#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake"]
fn capture_hud() {
    common::force_ps2_tier();
    let out = out_dir();
    for (label, size) in SIZES {
        let settle = |v: &mut game::hud_feed::HudView, m: &HudModel| {
            v.update(m, 0.0, size);
            let m2 = HudModel {
                messages: lines(),
                ..m.clone()
            };
            v.update(&m2, 0.0, size);
            let m3 = HudModel {
                messages: Vec::new(),
                ..m.clone()
            };
            for _ in 0..12 {
                v.update(&m3, 0.3, size);
            }
        };
        let play = base();
        frame(size, |v| settle(v, &play), Panel::None)
            .save(out.join(format!("hud_play_{label}.png")))
            .unwrap();
        // The droideka's two forms on the one bar: walking (no sprint, the
        // bar still shows the ball's charge), and as a ball half spent.
        let walker = HudModel {
            energy: 100.0,
            ..base()
        };
        frame(size, |v| settle(v, &walker), Panel::None)
            .save(out.join(format!("hud_droideka_walk_{label}.png")))
            .unwrap();
        let ball = HudModel {
            energy: 35.0,
            ..base()
        };
        frame(size, |v| settle(v, &ball), Panel::None)
            .save(out.join(format!("hud_droideka_ball_{label}.png")))
            .unwrap();
        // The BF1 scope, to report on: its drawing is not this pass's to change.
        let scoped = HudModel {
            scope: Some(ui::hud_scope::ScopeView {
                sheet: "common_bf1/weapon_scope".into(),
            }),
            ..base()
        };
        frame(size, |v| v.update(&scoped, 0.0, size), Panel::None)
            .save(out.join(format!("hud_scope_bf1_{label}.png")))
            .unwrap();
        let dead = HudModel {
            dead: true,
            respawn_in: Some(3.0),
            ..base()
        };
        frame(
            size,
            |v| v.update(&dead, 0.0, size),
            Panel::Hud(Box::new(dead.clone())),
        )
        .save(out.join(format!("hud_dead_{label}.png")))
        .unwrap();
    }
}

#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake"]
fn capture_shell() {
    use ui::menu::MenuInput;
    use ui::shell_ps2::{Page, ShellMenus};
    common::force_ps2_tier();
    let out = out_dir();
    let assets = game::assets::baked_root();
    game::shell_movie::load_index(&assets).expect("the shell index");
    let texts = game::event_feed::Texts::load(&assets, "bf2");
    let text = |key: &str, english: &str| {
        texts
            .bf2_text(key)
            .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
    };
    let base = ShellMenus::new(&text);
    let movie = game::shell_movie::ShellMovie::load(&assets).expect("the shell bake");
    let missions = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let ia = game::shell_ia::instant_action(&assets, &missions, core::skill::Difficulty::Normal);
    for (label, size) in SIZES {
        for (name, page) in [
            ("title", Page::Title),
            ("main", Page::Main),
            ("singleplayer", Page::Singleplayer),
            ("options", Page::Options),
        ] {
            let mut m = base.clone();
            m.page = page;
            m.step(MenuInput::default(), false, 0.0);
            m.on_page = 5.0;
            m.clock = 0.0;
            let (seg, at) = if m.movie() == ui::shell_ps2::MOVIE_MAIN {
                (0, 300)
            } else {
                (0, 150)
            };
            let (rgba, w, h) = movie.frame_now(m.movie(), seg, at).expect("movie frame");
            frame(
                size,
                |v| {
                    v.set_movie_frame(rgba, w, h);
                    v.update_shell(&m, true, size)
                },
                Panel::None,
            )
            .save(out.join(format!("shell_{name}_{label}.png")))
            .unwrap();
        }
        let mut menus = base.clone();
        menus.page = Page::Singleplayer;
        menus.covered = true;
        frame(
            size,
            |v| v.update_instant(&menus, &ia, false, size),
            Panel::None,
        )
        .save(out.join(format!("ia_missions_{label}.png")))
        .unwrap();
        let mut o = ia.clone();
        o.page = ui::shell::Page::Overview(0);
        frame(
            size,
            |v| v.update_instant(&menus, &o, false, size),
            Panel::None,
        )
        .save(out.join(format!("ia_session_{label}.png")))
        .unwrap();
        for k in 0..4 {
            let mut g = ia.clone();
            g.page = ui::shell::Page::Overview(k);
            g.step(
                ui::shell::IaInput {
                    menu: MenuInput {
                        confirm: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                0.0,
            );
            frame(
                size,
                |v| v.update_instant(&menus, &g, false, size),
                Panel::None,
            )
            .save(out.join(format!("ia_group{k}_{label}.png")))
            .unwrap();
        }
        // The controls list and the loading caption, through egui.
        let rows: Vec<(String, String)> = (0..40)
            .map(|i| {
                (
                    format!("Control row {i}"),
                    "Right Mouse Button + Left Shift".into(),
                )
            })
            .collect();
        frame(size, |_| {}, Panel::Controls(rows))
            .save(out.join(format!("controls_{label}.png")))
            .unwrap();
        frame(size, |_| {}, Panel::Loading)
            .save(out.join(format!("loading_{label}.png")))
            .unwrap();
    }
}
