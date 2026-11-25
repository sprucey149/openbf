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
        // The scope, BF2's on every map: its drawing is not this pass's to change.
        let scoped = HudModel {
            scope: Some(ui::hud_scope::ScopeView),
            ..base()
        };
        frame(size, |v| v.update(&scoped, 0.0, size), Panel::None)
            .save(out.join(format!("hud_scope_{label}.png")))
            .unwrap();
        let dead = HudModel {
            dead: true,
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

/// Issue #194 (`SPLITSCREEN.md` 18.2 B2, B22): the Players screen, always the 2x2 grid: one seat
/// picking with three open quarters (their join glyphs), two seats (P1 Ready on a pad, P2
/// choosing on the keyboard), four (P1 Ready, P2 on its name keyboard, P3 Ready with its pad
/// gone and a held Select able to empty it, P4 open), four pickers, and two seats Ready on one
/// profile (Continue refused), over the shell movie, at 4:3, 16:9, 21:9 and 1024x768.
#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake"]
fn capture_players() {
    use ui::input_icons::IconSet;
    use ui::shell_ps2::players::PlayerCell;
    use ui::shell_ps2::profiles::{Picker, Target};
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
    let movie = game::shell_movie::ShellMovie::load(&assets).expect("the shell bake");
    let mut m = ShellMenus::new(&text);
    m.open_page(Page::Players);
    m.on_page = 5.0;
    m.clock = 0.0;
    let names: Vec<(u64, String)> = ["Ann", "Bob", "Cy", "Dee", "Eve", "Finn"]
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u64, n.to_string()))
        .collect();
    let mut pick = Picker::new(names, None);
    pick.seat = true;
    pick.taken = vec![0];
    pick.set_focus(Target::Profile(2));
    // Issue #194 B22: one seat picking in the top-left quarter, three open quarters showing the
    // waiting DualShock 4s' Cross and Options glyphs; and four pickers at once.
    let mut one = m.clone();
    one.players.cells[0] = PlayerCell::Picking {
        picker: pick.clone(),
        device: "Xbox controller".into(),
        icons: IconSet::XboxOne,
        lost: false,
    };
    one.players.join_icons = Some(IconSet::Ds4);
    let mut pickers = one.clone();
    for (i, icons) in [IconSet::Ds4, IconSet::Ds2, IconSet::KeyboardMouse]
        .into_iter()
        .enumerate()
    {
        pickers.players.cells[i + 1] = PlayerCell::Picking {
            picker: pick.clone(),
            device: "Pad".into(),
            icons,
            lost: false,
        };
    }
    let ready = |name: &str, icons, device: &str, lost| PlayerCell::Seated {
        name: name.into(),
        device: device.into(),
        icons,
        career: "25 kills   310 points   4 rounds".into(),
        lost,
        rechoose: true,
    };
    m.players.cells[0] = ready("Ann", IconSet::XboxOne, "Xbox controller", false);
    m.players.cells[1] = PlayerCell::Picking {
        picker: pick.clone(),
        device: "Keyboard and mouse".into(),
        icons: IconSet::KeyboardMouse,
        lost: false,
    };
    let two = m.clone();
    let mut naming = pick;
    naming.edit_name("Player 1".into());
    m.players.cells[1] = PlayerCell::Picking {
        picker: naming,
        device: "DualShock 4".into(),
        icons: IconSet::Ds4,
        lost: false,
    };
    m.players.cells[2] = ready("Cy", IconSet::Ds2, "DualShock 2", true);
    m.players.keyboard_free = true;
    // P3's pad is gone: a held Select on a seated pad (P1's Xbox pad) empties the seat.
    m.players.removable[2] = true;
    m.players.remove_icons = Some(IconSet::XboxOne);
    let four = m.clone();
    // Findings players-continue-live-but-refused-duplicate-profile: P2 and P3 Ready on one
    // profile; the party refuses Continue, so no card shows it live, and both cards say why.
    let mut twice = m;
    twice.players.cells[1] = ready("Cy", IconSet::Ds4, "DualShock 4", false);
    twice.players.cells[2] = ready("Cy", IconSet::Ds2, "DualShock 2", false);
    twice.players.removable = [false; 4];
    twice.players.twice = [false, true, true, false];
    twice.players.blocked = true;
    let (rgba, w, h) = movie.frame_now(two.movie(), 0, 150).expect("movie frame");
    let sizes = SIZES.into_iter().chain([("1024x768", [1024, 768])]);
    for (label, size) in sizes {
        for (name, menus) in [
            ("1", &one),
            ("2", &two),
            ("4", &four),
            ("4pickers", &pickers),
            ("twice", &twice),
        ] {
            frame(
                size,
                |v| {
                    v.set_movie_frame(rgba.clone(), w, h);
                    v.update_shell(menus, true, size)
                },
                Panel::None,
            )
            .save(out.join(format!("players_{name}_{label}.png")))
            .unwrap();
        }
    }
}
