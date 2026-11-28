//! Headless captures of whole splitscreen frames (issue #194): each view's world through
//! `Renderer::render_view` into its cell, and each view's UI through `game::split_ui::SplitUi`,
//! the code the window's split frame paints with (`main/windowed/split.rs`, `render_views`),
//! over HOT1's mission camera shots with fixed models, each spawn page's soldiers standing in its
//! own view's UI rectangle (`game::spawn_preview::Preview` per view, laid out by
//! `game::split_layout::ui_view`, as the window does). Evidence for review, not a golden: it
//! writes PNGs to `engine/artifacts/split_captures/` (git-ignored) and checks only that every
//! view's UI painted its own whole cell (`ui::hud_draw::Frame::cell`: the HUD at the cell's
//! edges, the pages centred on its 4:3 band).
//!
//! Ignored by default - needs a GPU adapter AND baked assets_baked/bf2_hot1 + common:
//!   cargo test -p game --test split_capture -- --ignored --test-threads=1 --nocapture

mod common;

use game::split_layout::{arranged, Cell, TwoPlayer};
use game::split_ui::{SplitPause, SplitUi};
use ui::hud::HudModel;
use ui::input_icons::{IconSet, Icons};
use ui::spawn_draw::SpawnPage;

/// The seats' profile names, through the table the window names its feeds from
/// (`game::player_names::PlayerNames`, keyed by sim slot; the captures' slot `i` is player `i`).
fn names() -> game::player_names::PlayerNames {
    let mut n = game::player_names::PlayerNames::default();
    n.set([(0, "Luke"), (1, "Han"), (2, "Leia"), (3, "Chewie")]);
    n
}

/// A soldier in battle: scores, minimap, a target under the reticle, ammo, health and stamina,
/// two message lines (Han's kill, by his profile name).
fn battle(map: Option<ui::hud_map::MapModel>) -> HudModel {
    use ui::hud::message_color::{ENEMY, OWN};
    HudModel {
        team_names: ["imp".into(), "all".into()],
        tickets: [153, 165],
        hp: 220.0,
        hp_max: 300.0,
        energy: 64.0,
        energy_max: 100.0,
        weapon: Some(ui::hud::HudWeapon {
            key: "imp_weap_inf_rifle".into(),
            class: String::new(),
            icon: None,
            name: String::new(),
            total: Some(281),
            clip_fraction: 31.0 / 50.0,
            bf2: true,
        }),
        reticle: Some(-1),
        target: Some(ui::hud::HudTarget {
            id: 1,
            name: "Rebel Soldier".into(),
            health_fraction: 0.6,
            relation: -1,
            team0: false,
            shield: None,
        }),
        reticle_y: game::walk::ps2_reticle_y("bf2"),
        messages: [
            ("Rebels captured the Shield Generator".to_string(), ENEMY),
            (
                format!("{} killed Rebel Soldier", names().name_or_default(1)),
                OWN,
            ),
        ]
        .into_iter()
        .map(|(text, color)| ui::hud::HudMessage {
            text: text.into(),
            color,
        })
        .collect(),
        stats: Some(ui::hud::HudStats::default()),
        map,
        ..HudModel::default()
    }
}

/// Dead behind a spawn page (`None`: the death camera, with the line that says who killed them).
fn dead(page: Option<SpawnPage>, map: Option<ui::hud_map::MapModel>, who: usize) -> HudModel {
    HudModel {
        team_names: ["imp".into(), "all".into()],
        tickets: [153, 165],
        hp: 0.0,
        hp_max: 300.0,
        dead: true,
        messages: vec![ui::hud::HudMessage {
            text: format!(
                "Rebel Soldier killed {}",
                names().name_or_default(who as u8 - 1)
            ),
            color: ui::hud::message_color::OWN,
        }],
        stats: Some(ui::hud::HudStats::default()),
        spawn: page,
        map,
        ..HudModel::default()
    }
}

fn side() -> SpawnPage {
    SpawnPage::Side {
        title: "Select a team".into(),
        rows: vec!["Empire".into(), "Rebels".into(), "Auto Assign".into()],
        selected: 0,
        select: "Select".into(),
    }
}

fn map_page() -> SpawnPage {
    SpawnPage::Map {
        objective: "Capture all the Command Posts".into(),
        legend: [
            "Command Post".into(),
            "Change Command Post".into(),
            "{btna} Select Command Post".into(),
        ],
    }
}

fn class_page() -> SpawnPage {
    SpawnPage::Class {
        title: "Snowtrooper".into(),
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
        classes: (0..6).collect(),
        refused: 0,
    }
}

/// Hoth's Empire as the spawn screens draw it (the class page's line-up), as `golden_spawn`'s.
fn hot1_teams() -> [game::walk::TeamTemplate; 2] {
    let mut empire = game::walk::TeamTemplate::default_trooper();
    let base = empire.classes[0].clone();
    empire.classes = [
        ("imp_inf_snowtrooper", "imp_weap_inf_rifle"),
        ("imp_inf_snowtrooper", "imp_weap_inf_launcher"),
        ("imp_inf_atatpilot", "imp_weap_inf_rifle"),
        ("imp_inf_snowtrooper", "imp_weap_inf_sniperrifle"),
        ("imp_inf_atatcommander", "imp_weap_inf_pistol"),
        ("imp_inf_darktrooper", "imp_weap_inf_rifle"),
    ]
    .into_iter()
    .map(|(body, gun)| game::walk::ClassVisual {
        geometry: Some(body.into()),
        weapon_geometry: Some(gun.into()),
        anim: game::animator::ClassAnim {
            family: "human".into(),
            root_scale: 1.0,
            banks: Vec::new(),
            fallback_banks: Vec::new(),
            custom: Default::default(),
        },
        ..base.clone()
    })
    .collect();
    [empire, game::walk::TeamTemplate::default_trooper()]
}

/// Each view's own round-end stats pages (issue #194 B9): the same battle seen by `me` (the
/// viewer's row and their own personal page), on `page`.
fn pages(me: &str, page: ui::stats::Page) -> HudModel {
    use ui::stats::*;
    let humans = ["Luke", "Han", "Leia", "Chewie"];
    let row = |name: &str, points: i32| StatRow {
        name: name.into(),
        points,
        kills: points,
        deaths: 2,
        posts: 1,
        me: name == me,
        human: humans.contains(&name),
        personal: vec![
            ("Points".into(), points.to_string()),
            ("Kills".into(), points.to_string()),
            ("Deaths".into(), "2".into()),
        ],
        medals: if name == me {
            vec![("War Hero".into(), 1)]
        } else {
            Vec::new()
        },
    };
    let mut s = StatsScreens::new(
        StatsTexts {
            team_title: "Team Stats".into(),
            awards_title: "Awards".into(),
            personal_title: "Personal Stats".into(),
            medals_title: "Medals".into(),
            career_title: "Career Stats".into(),
            career_medals: "Career Medals".into(),
            done: "Done".into(),
            select: "Select".into(),
            back: "Back".into(),
            ..Default::default()
        },
        [
            TeamList {
                icon: "imp_icon".into(),
                rows: vec![row("Snowtrooper 3", 4), row("Snowtrooper 7", 1)],
            },
            TeamList {
                icon: "all_icon".into(),
                rows: vec![
                    row("Luke", 9),
                    row("Han", 6),
                    row("Leia", 5),
                    row("Chewie", 3),
                    row("Rebel 12", 2),
                ],
            },
        ],
        [Vec::new(), Vec::new()],
        Career::default(),
    );
    s.page = page;
    HudModel {
        round_end: Some(ui::hud::RoundEndModel {
            result: 0,
            stats: Some(s),
            fade: 1.0,
        }),
        ..battle(None)
    }
}

fn pad(set: IconSet) -> Icons {
    game::input_icons::with_keyboard(
        game::input_icons::icons(set, &input::Mapping::default()),
        &game::kbm::Bindings::default(),
    )
}

/// One frame to capture: the views' models and icons, and the Game Menu if open.
struct Scene {
    name: &'static str,
    layout: TwoPlayer,
    /// The Splitscreen options' order, `cells[seat]` (issue #194 B7); seat `i` is player `i`.
    order: [u8; 4],
    /// The HUD size option's factor.
    hud_k: f32,
    models: Vec<Option<HudModel>>,
    icons: Vec<Icons>,
    pause: Option<(usize, ui::menu::MenuModel)>,
    /// A LAN round's Game Menu: in its owner's cell only (issue #194 B20).
    menu_in_view: bool,
    /// The views whose cells are away (the controller is gone): no world draws, and the
    /// lost-controller notice in the cell's HUD pass (`SplitUi::lost`), as the window's. With
    /// `Some(here)`, a returning pad asks whose it is: the pick is on this player (`true`) or on
    /// another (`false`), and the notice's acting line is the pick line (`split_ui::pick_line`).
    away: Vec<(usize, Option<bool>)>,
}

/// Every rectangle each view painted, by view.
type Painted = Vec<Option<Cell>>;

struct World<'a> {
    scene: &'a game::assets::Scene,
    shots: &'a [game::spawn_backdrop::CameraShot],
}

/// The egui overlay a capture paints over every view, after the last (as the window's), given the
/// views' cells (pixels, at 1 point per pixel) and cameras.
type OverlayUi<'a> = &'a dyn Fn(&ui::egui::Context, &[Cell], &[render::Camera]);

/// Each view's camera: a mission shot per view, widened to its cell (`fit_fov_y`).
fn cameras(world: &World<'_>, views: &[Cell]) -> Vec<render::Camera> {
    views
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let mut cam = world.shots[(i * 2 + 1) % world.shots.len()].camera();
            cam.fov_y =
                game::split_layout::fit_fov_y(cam.fov_y, cell[2] as f32 / cell[3].max(1) as f32);
            cam
        })
        .collect()
}

fn capture(
    world: &World<'_>,
    s: &Scene,
    size: [u32; 2],
    out: &std::path::Path,
    overlay_ui: Option<OverlayUi<'_>>,
) -> (Painted, Vec<u8>) {
    let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
    let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
    // Every egui text in the menu face, as the window installs it (`events.rs`).
    ui::typography::face::install(
        overlay.ctx(),
        &game::hud_feed::menu_face(&game::assets::baked_root()),
    );
    let gpu = game::viewer::upload_scene(&mut r, world.scene).expect("upload");
    let mut hv = game::hud_feed::HudView::for_renderer(&game::assets::baked_root(), &r)
        .expect("the BF2 HUD bake (bake common)");
    let views = arranged(s.models.len(), size[0], size[1], s.layout, s.order, |i| i);
    let cams = cameras(world, &views);
    let mut anim = ui::pause_draw::Animation::default();
    anim.advance(2.0);
    let mut painted: Painted = vec![None; views.len()];
    // Each view's spawn-screen soldiers, as the window's (`split.rs`, `render_views`).
    let stands = s
        .models
        .iter()
        .flatten()
        .any(|m| m.spawn.as_ref().is_some_and(game::spawn_preview::stands));
    let vis = stands.then(|| {
        game::soldier_vis::SoldierVisuals::load(
            &mut r,
            &game::assets::baked_root(),
            "bf2",
            &hot1_teams(),
        )
    });
    let mut previews: Vec<game::spawn_preview::Preview> =
        (0..views.len()).map(|_| Default::default()).collect();
    let mut stood = vec![false; views.len()];
    // A few frames: the first settles each player's fades, the next admit the message lines.
    for frame in 0..5 {
        let dt = if frame <= 1 || s.pause.is_some() {
            0.0
        } else {
            0.6
        };
        // A model's message lines are handed over once, as the game's feed does.
        let models: Vec<Option<HudModel>> = s
            .models
            .iter()
            .map(|m| {
                m.clone().map(|m| HudModel {
                    messages: if frame == 1 { m.messages } else { Vec::new() },
                    ..m
                })
            })
            .collect();
        // Each away cell's notice, as the window's frame builds it (`events.rs`).
        let lost: Vec<Option<ui::shell_ps2::players::LostCell>> = (0..views.len())
            .map(|i| {
                let (_, pick) = s.away.iter().find(|(v, _)| *v == i)?;
                let name = names().name_or_default(i as u8);
                let mut n = game::split_ui::lost_notice(name.clone());
                if let Some(here) = pick {
                    let (glyph, action) = game::split_ui::pick_line(IconSet::XboxOne, *here, &name);
                    (n.glyph, n.action) = (Some(glyph), action);
                }
                Some(n)
            })
            .collect();
        let split = SplitUi {
            lost: &lost,
            models: &models,
            icons: &s.icons,
            dt,
            hidden: false,
            pause: s.pause.as_ref().map(|(owner, menu)| SplitPause {
                owner: *owner,
                menu,
                anim: &anim,
                in_view: s.menu_in_view.then_some(*owner),
            }),
            window: size,
            views: views.len(),
            hud_k: s.hud_k,
        };
        // The scene's own overlay.
        let paint = overlay_ui.is_some().then(|| {
            overlay.begin_headless(size, 1.0, |ctx| {
                if let Some(f) = overlay_ui {
                    f(ctx, &views, &cams);
                }
            })
        });
        for (i, cell) in views.iter().copied().enumerate() {
            let slot = render::ViewSlot {
                rect: cell,
                first: i == 0,
                last: i + 1 == views.len(),
            };
            let away = s.away.iter().any(|(v, _)| *v == i);
            let cam = cams[i];
            let last = i + 1 == views.len();
            let mut hook = |dev: &wgpu::Device,
                            q: &wgpu::Queue,
                            enc: &mut wgpu::CommandEncoder,
                            view: &wgpu::TextureView,
                            _size: [u32; 2]| {
                painted[i] = split.paint(&mut hv, i, cell, dev, q, enc, view);
                if let (Some(p), true) = (paint.as_ref(), last) {
                    overlay.paint(p, dev, q, enc, view, size);
                }
            };
            let page = s.models[i].as_ref().and_then(|m| m.spawn.as_ref());
            let (mut skinned, mut draws) = (Vec::new(), gpu.draws.clone());
            if let (Some(vis), false) = (vis.as_ref(), away) {
                let (fov_y, aspect) = game::split_layout::ui_view(cam.fov_y, cell);
                previews[i].set_view_aspect(Some(aspect));
                let (body, gun) = previews[i].draws_live(
                    &mut r,
                    vis,
                    page,
                    &render::Camera { fov_y, ..cam },
                    0.0,
                    |_, _, _, _, _| None,
                );
                stood[i] |= !body.is_empty();
                skinned = body;
                draws.extend(gun);
            }
            let clear = if away {
                draws.clear();
                [0.003, 0.003, 0.005, 1.0]
            } else {
                gpu.clear
            };
            r.render_view(slot, &cam, &draws, &skinned, clear, Some(&mut hook))
                .expect("render");
        }
    }
    let (rgba, w, h) = r.capture_rgba().expect("capture");
    let path = out.join(format!("{}_{}x{}.png", s.name, size[0], size[1]));
    image::RgbaImage::from_raw(w, h, rgba.clone())
        .expect("frame")
        .save(&path)
        .expect("write");
    println!("wrote {}", path.display());
    // Every view whose page stands soldiers stood them (P2-P4 as P1).
    for (i, m) in s.models.iter().enumerate() {
        let wants = m
            .as_ref()
            .and_then(|m| m.spawn.as_ref())
            .is_some_and(game::spawn_preview::stands);
        let away = s.away.iter().any(|(v, _)| *v == i);
        assert_eq!(stood[i], wants && !away, "view {i}'s soldiers");
    }
    // Every view's UI stayed inside its own cell (the Game Menu covers the window).
    for (i, (p, cell)) in painted.iter().zip(&views).enumerate() {
        // A LAN round's menu (issue #194 B20) covers its opener's cell; every view paints its
        // own cell.
        let pause = s.pause.as_ref().filter(|_| !s.menu_in_view);
        match (p, pause) {
            (Some(rect), None) => {
                assert_eq!(*rect, *cell, "view {i} painted its whole cell");
                assert!(rect[0] >= cell[0] && rect[0] + rect[2] <= cell[0] + cell[2]);
                assert!(rect[1] >= cell[1] && rect[1] + rect[3] <= cell[1] + cell[3]);
            }
            (None, None) => panic!("view {i} painted no UI (an away view paints its notice)"),
            (p, Some(_)) => {
                let last = i + 1 == views.len();
                assert_eq!(p.is_some(), last, "the Game Menu rides the last view only");
            }
        }
    }
    (painted, rgba)
}

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/bf2_hot1"]
fn split_frames_capture() {
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("bf2_hot1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake HOT1 first (see test header) - the capture can't skip silently");
    }
    let mission = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../missions/bf2_hot1_con.ron"
    );
    let shots = game::mission::MissionDef::load(std::path::Path::new(mission))
        .expect("hot1's mission")
        .camera_shots();
    let scene = game::assets::load_scene(&baked).expect("scene");
    let mut posts = game::assets::load_scene(&baked).expect("scene");
    posts.apply_mode_layer("con");
    let spawn_map = || {
        let mut map = game::minimap::spawn_still(&posts, Some(1));
        let (f, e) = (ui::hud_map::FRIEND, ui::hud_map::ENEMY);
        for (p, c) in map.posts.iter_mut().zip([e, f, f, e, e]) {
            p.color = c;
        }
        map
    };
    let cam = shots[1].camera();
    let minimap = || Some(game::minimap::still(&scene, cam.position, cam.yaw));
    let out = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/split_captures"
    ));
    std::fs::create_dir_all(&out).expect("captures dir");
    let xbox = pad(IconSet::XboxOne);
    let ds4 = pad(IconSet::Ds4);
    let keyboard = game::input_icons::with_keyboard(
        game::input_icons::icons(IconSet::KeyboardMouse, &input::Mapping::default()),
        &game::kbm::Bindings::default(),
    );
    let pause_menu = |who: usize, icons: Icons| ui::menu::MenuModel {
        icons,
        respawn_label: Some("Respawn".into()),
        restart_available: true,
        page: Some(ui::menu::PausePage::Main),
        paused_by: Some(format!(
            "Paused by {}",
            names().name_or_default(who as u8 - 1)
        )),
        ..Default::default()
    };
    // A LAN round's menu pauses nothing: its line names whose menu it is (the window's
    // `App::paused_by_line` under `netplay`). It has no Restart Mission row: a LAN round loads with
    // no `App::current`, so `restart_available` is false there. Its cheats are the opener's own.
    let lan_menu = |who: usize, icons: Icons| ui::menu::MenuModel {
        paused_by: Some(game::split_ui::lan_menu_line(Some(
            &names().name_or_default(who as u8 - 1),
        ))),
        restart_available: false,
        own_cheats: true,
        ..pause_menu(who, icons)
    };
    const ID: [u8; 4] = [0, 1, 2, 3];
    let split_options =
        |seated: [bool; 4], top_and_bottom: bool, cells: [u8; 4]| ui::menu::MenuModel {
            icons: xbox,
            page: Some(ui::menu::PausePage::Settings(
                ui::menu::SettingsPage::Splitscreen,
            )),
            cursor: 2,
            paused_by: Some("Paused by Luke".into()),
            // B22: the screen rows by name; seat 4 has no usable name and says "P4".
            split_names: [
                Some("Luke".into()),
                Some("Han".into()),
                Some("Leia".into()),
                None,
            ],
            split: Some(ui::menu::SplitMenu {
                top_and_bottom,
                seated,
                cells,
                name_tags: true,
                hud_size: 0,
            }),
            ..Default::default()
        };
    let scenes = vec![
        // Issue #194 B7: the Splitscreen options' orders and HUD size.
        Scene {
            name: "b7_two_swapped",
            layout: TwoPlayer::SideBySide,
            order: [1, 0, 2, 3],
            hud_k: 1.0,
            models: vec![Some(dead(Some(side()), None, 1)), Some(battle(minimap()))],
            icons: vec![xbox, ds4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "b7_two_top_bottom_swapped",
            layout: TwoPlayer::TopBottom,
            order: [3, 0, 1, 2],
            hud_k: 1.0,
            models: vec![Some(battle(minimap())), Some(dead(Some(side()), None, 2))],
            icons: vec![keyboard, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "b7_three_blank_top_left",
            layout: TwoPlayer::SideBySide,
            order: [1, 2, 3, 0],
            hud_k: 1.0,
            models: vec![
                Some(battle(minimap())),
                Some(dead(None, None, 2)),
                Some(dead(Some(map_page()), Some(spawn_map()), 3)),
            ],
            icons: vec![xbox, ds4, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "b7_two_hud_smallest",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 0.7,
            models: vec![Some(battle(minimap())), Some(battle(minimap()))],
            icons: vec![xbox, ds4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "b7_two_options_page",
            layout: TwoPlayer::TopBottom,
            order: ID,
            hud_k: 1.0,
            models: vec![Some(battle(minimap())), Some(battle(minimap()))],
            icons: vec![xbox, ds4],
            pause: Some((0, split_options([true, true, false, false], true, ID))),
            menu_in_view: false,
            away: Vec::new(),
        },
        // A LAN round's Game Menu, opened by the second player (issue #194 B20): it covers only
        // their cell, and the first player's HUD stays up as they play on.
        Scene {
            name: "b20_lan_menu_in_cell",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![Some(battle(minimap())), Some(battle(minimap()))],
            icons: vec![xbox, ds4],
            pause: Some((1, lan_menu(2, ds4))),
            menu_in_view: true,
            away: Vec::new(),
        },
        Scene {
            name: "b7_three_options_page",
            layout: TwoPlayer::SideBySide,
            order: [1, 2, 3, 0],
            hud_k: 1.0,
            models: vec![
                Some(battle(minimap())),
                Some(battle(minimap())),
                Some(battle(minimap())),
            ],
            icons: vec![xbox, ds4, xbox],
            pause: Some((
                0,
                split_options([true, true, true, false], false, [1, 2, 3, 0]),
            )),
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "two_side_by_side",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![Some(dead(Some(side()), None, 1)), Some(battle(minimap()))],
            icons: vec![xbox, ds4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "two_class_side_by_side",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(dead(Some(class_page()), None, 1)),
                Some(dead(Some(class_page()), None, 2)),
            ],
            icons: vec![keyboard, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "two_class_top_bottom",
            layout: TwoPlayer::TopBottom,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(dead(Some(class_page()), None, 1)),
                Some(dead(Some(side()), None, 2)),
            ],
            icons: vec![xbox, ds4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "two_top_bottom",
            layout: TwoPlayer::TopBottom,
            order: ID,
            hud_k: 1.0,
            models: vec![Some(battle(minimap())), Some(dead(Some(side()), None, 2))],
            icons: vec![keyboard, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "three",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(battle(minimap())),
                Some(dead(None, None, 2)),
                Some(dead(Some(map_page()), Some(spawn_map()), 3)),
            ],
            icons: vec![xbox, ds4, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "four",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(dead(Some(side()), None, 1)),
                Some(dead(Some(class_page()), None, 2)),
                Some(battle(minimap())),
                Some(dead(Some(map_page()), Some(spawn_map()), 4)),
            ],
            icons: vec![xbox, ds4, keyboard, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "four_one_away",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(battle(minimap())),
                None,
                Some(dead(None, None, 3)),
                Some(dead(Some(class_page()), None, 4)),
            ],
            icons: vec![xbox, ds4, xbox, xbox],
            pause: None,
            menu_in_view: false,
            away: vec![(1, None)],
        },
        Scene {
            name: "two_paused_by_han",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![Some(battle(minimap())), Some(battle(minimap()))],
            icons: vec![xbox, ds4],
            pause: Some((1, pause_menu(2, ds4))),
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "four_paused_by_leia",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(battle(minimap())),
                Some(dead(Some(side()), None, 2)),
                Some(battle(minimap())),
                Some(battle(minimap())),
            ],
            icons: vec![xbox, ds4, xbox, ds4],
            pause: Some((2, pause_menu(3, xbox))),
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            // B22: each cell its own side's banner (P1's team won, P2's lost).
            name: "round_end_mixed_teams",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(HudModel {
                    round_end: Some(ui::hud::RoundEndModel {
                        result: 0,
                        stats: None,
                        fade: 0.0,
                    }),
                    ..battle(None)
                }),
                Some(HudModel {
                    round_end: Some(ui::hud::RoundEndModel {
                        result: 1,
                        stats: None,
                        fade: 0.0,
                    }),
                    ..battle(None)
                }),
            ],
            icons: vec![xbox, ds4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "two_stats_pages",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(pages("Luke", ui::stats::Page::Team)),
                Some(pages("Han", ui::stats::Page::Personal)),
            ],
            icons: vec![xbox, ds4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "two_stats_pages_top_bottom",
            layout: TwoPlayer::TopBottom,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(pages("Luke", ui::stats::Page::Personal)),
                Some(pages("Han", ui::stats::Page::Team)),
            ],
            icons: vec![keyboard, xbox],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
        Scene {
            name: "four_stats_pages",
            layout: TwoPlayer::SideBySide,
            order: ID,
            hud_k: 1.0,
            models: vec![
                Some(pages("Luke", ui::stats::Page::Team)),
                Some(pages("Han", ui::stats::Page::Personal)),
                Some(pages("Leia", ui::stats::Page::Personal)),
                Some(pages("Chewie", ui::stats::Page::Team)),
            ],
            icons: vec![xbox, ds4, xbox, keyboard],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        },
    ];
    let world = World {
        scene: &scene,
        shots: &shots,
    };
    for size in [[3840, 2160], [1920, 1080], [3839, 2372]] {
        for s in &scenes {
            capture(&world, s, size, &out, None);
        }
    }
}

/// Issue #194 B8, B22: per-view name tags and round timers in a split frame, through the
/// window's paths: each view's tags placed from that view's camera at its cell's aspect
/// (`game::name_tags::placed`) into its HUD model, which the HUD pass draws in the kill feed's
/// font (`SplitUi::paint`); the round timer through `ui::hud::draw_hud_in` per cell. P1 stands in
/// front of P2's camera, so only P2's view has a tag ("Luke"; nobody tags himself). Checked by a
/// diff against the same frame without the tags: every pixel the tag changed is inside P2's
/// cell, none in P1's. Captures: `b8_tags_side_by_side`, `b8_tags_top_bottom`,
/// `b8_tags_three_swapped`; and B22's readability pair, four views at 1280x720 with every view
/// tagging a player ahead, HUD Size smallest and full (`b22_tags_four_hud_small`,
/// `b22_tags_four_hud_full`).
#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/bf2_hot1"]
fn split_name_tags_capture() {
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("bf2_hot1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake HOT1 first (see test header) - the capture can't skip silently");
    }
    let mission = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../missions/bf2_hot1_con.ron"
    );
    let shots = game::mission::MissionDef::load(std::path::Path::new(mission))
        .expect("hot1's mission")
        .camera_shots();
    let scene = game::assets::load_scene(&baked).expect("scene");
    let out = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/split_captures"
    ));
    std::fs::create_dir_all(&out).expect("captures dir");
    let world = World {
        scene: &scene,
        shots: &shots,
    };
    let timed = || HudModel {
        time_left: Some(754.0),
        objective_timer: None,
        ..battle(None)
    };
    let xbox = pad(IconSet::XboxOne);
    // A player's head 12 m ahead of view `i`'s camera, a little below its eye line, named as
    // the player of the view before it.
    let tag_for = |cam: &render::Camera, who: u8| game::name_tags::NameTag {
        slot: who,
        name: names().name_or_default(who),
        at: cam.position + cam.forward() * 12.0 - glam::Vec3::Y * 0.5,
        alpha: 1.0,
        friend: who % 2 == 0,
    };
    // Each view's model with the tags the window would hand it (`events.rs`).
    let tagged_models = |views: &[Cell], tagged: &dyn Fn(usize) -> bool| -> Vec<Option<HudModel>> {
        let cams = cameras(&world, views);
        views
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let mut m = timed();
                if tagged(i) {
                    let who = ((i + views.len() - 1) % views.len()) as u8;
                    m.name_tags = game::name_tags::placed(
                        &[tag_for(&cams[i], who)],
                        game::name_tags::cell_view_proj(&cams[i], *cell),
                        [cell[2] as f32, cell[3] as f32],
                    );
                }
                Some(m)
            })
            .collect()
    };
    let scenes = [
        (
            "b8_tags_side_by_side",
            TwoPlayer::SideBySide,
            [0, 1, 2, 3],
            2,
        ),
        ("b8_tags_top_bottom", TwoPlayer::TopBottom, [0, 1, 2, 3], 2),
        (
            "b8_tags_three_swapped",
            TwoPlayer::SideBySide,
            [1, 2, 3, 0],
            3,
        ),
    ];
    let timers = |hud_k: f32| {
        move |ctx: &ui::egui::Context, cells: &[Cell], _: &[render::Camera]| {
            for (i, cell) in cells.iter().enumerate() {
                let t = game::name_tags::cell_text(ctx, *cell, hud_k);
                ui::hud::draw_hud_in(ctx, &timed(), t.rect, i + 1, t.scale, t.timer_at);
            }
        }
    };
    let size = [1920, 1080];
    for (name, layout, order, n) in scenes {
        let views = arranged(n, size[0], size[1], layout, order, |i| i);
        let scene_with = |name: &'static str, models: Vec<Option<HudModel>>| Scene {
            name,
            layout,
            order,
            hud_k: 1.0,
            models,
            icons: vec![xbox; n],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        };
        let with = scene_with(name, tagged_models(&views, &|i| i == 1));
        let without = scene_with("b8_tags_reference", tagged_models(&views, &|_| false));
        assert!(
            with.models[1]
                .as_ref()
                .is_some_and(|m| m.name_tags.len() == 1),
            "{name}: P1's tag is on P2's screen"
        );
        let draw = timers(1.0);
        let (_, tagged) = capture(&world, &with, size, &out, Some(&draw));
        let (_, plain) = capture(&world, &without, size, &out, Some(&draw));
        let inside =
            |c: Cell, x: u32, y: u32| x >= c[0] && x < c[0] + c[2] && y >= c[1] && y < c[1] + c[3];
        let (mut in_p2, mut elsewhere) = (0, 0);
        for (k, (a, b)) in tagged.chunks(4).zip(plain.chunks(4)).enumerate() {
            if a == b {
                continue;
            }
            let (x, y) = (k as u32 % size[0], k as u32 / size[0]);
            if inside(views[1], x, y) {
                in_p2 += 1;
            } else {
                elsewhere += 1;
            }
        }
        assert!(
            in_p2 > 50,
            "{name}: P1's tag drew in P2's cell ({in_p2} px)"
        );
        assert_eq!(elsewhere, 0, "{name}: nothing of the tag outside P2's cell");
    }
    // B22: four quarter cells at 1280x720, a tag in each, at the smallest and the full HUD
    // Size, beside each cell's own kill feed line.
    let small = [1280, 720];
    for (name, hud_k) in [
        ("b22_tags_four_hud_small", 0.7),
        ("b22_tags_four_hud_full", 1.0),
    ] {
        let views = arranged(
            4,
            small[0],
            small[1],
            TwoPlayer::SideBySide,
            [0, 1, 2, 3],
            |i| i,
        );
        let models = tagged_models(&views, &|_| true);
        let s = Scene {
            name,
            layout: TwoPlayer::SideBySide,
            order: [0, 1, 2, 3],
            hud_k,
            models,
            icons: vec![xbox; 4],
            pause: None,
            menu_in_view: false,
            away: Vec::new(),
        };
        let draw = timers(hud_k);
        capture(&world, &s, small, &out, Some(&draw));
    }
    // Issue #194: P2's and P4's identical pads dropped and one is back. Nothing tells whose it
    // is, so both waiting cells ask: the pick is on Han (the A glyph, "Play as Han"), Chewie's
    // shows the d-pad left/right glyph that moves it there. Capture: `pad_pick_four`.
    let views = arranged(
        4,
        size[0],
        size[1],
        TwoPlayer::SideBySide,
        [0, 1, 2, 3],
        |i| i,
    );
    let pick = |ctx: &ui::egui::Context, cells: &[Cell], _: &[render::Camera]| {
        // The playing cells' timers; a waiting cell shows its notice only.
        for i in [0usize, 2] {
            let t = game::name_tags::cell_text(ctx, cells[i], 1.0);
            ui::hud::draw_hud_in(ctx, &timed(), t.rect, i + 1, t.scale, t.timer_at);
        }
    };
    let s = Scene {
        name: "pad_pick_four",
        layout: TwoPlayer::SideBySide,
        order: [0, 1, 2, 3],
        hud_k: 1.0,
        models: (0..4).map(|i| (i % 2 == 0).then(timed)).collect(),
        icons: vec![xbox; 4],
        pause: None,
        menu_in_view: false,
        away: vec![(1, Some(true)), (3, Some(false))],
    };
    assert_eq!(views.len(), 4);
    capture(&world, &s, size, &out, Some(&pick));
}

/// Issue #194 B20: a LAN round with two players on this machine, side by side. The round is a
/// LAN round's sim (`netplay::install_humans` from a setup with three humans: Luke and Leia on
/// this machine, Lando on another; Leia's slot adopted as this machine's second player,
/// `WalkSim::adopt_local`), each view's tags made by the window's own per-cell code
/// (`name_tags::cell_tags`) from that view's player and camera, named from the session's names.
/// Luke's view (left) sees Leia, his co-local, and Lando, the remote player; Leia's (right)
/// sees Lando ahead and Luke behind her, so only Lando's tag. Capture: `b20_lan_tags`.
#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/bf2_hot1"]
fn split_lan_name_tags_capture() {
    use game::netplay::{HumanSeat, PlayerProfile, RoundSetup};
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("bf2_hot1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake HOT1 first (see test header) - the capture can't skip silently");
    }
    let mission = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../missions/bf2_hot1_con.ron"
    );
    let shots = game::mission::MissionDef::load(std::path::Path::new(mission))
        .expect("hot1's mission")
        .camera_shots();
    let scene = game::assets::load_scene(&baked).expect("scene");
    let out = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/split_captures"
    ));
    std::fs::create_dir_all(&out).expect("captures dir");
    // Level views on the ground of HOT1's view-0 shot, facing its way.
    let base = shots[1].camera();
    let f = base.forward().with_y(0.0).normalize();
    let right = f.cross(glam::Vec3::Y).normalize();
    let eye = glam::Vec3::Y * 1.7;
    let ground = base.position - eye;
    let luke = ground - right * 2.0;
    let leia = ground + right * 1.0 + f * 9.0;
    let lando = ground + right * 0.5 + f * 16.0;
    let rot = glam::Quat::from_rotation_y(core::fmath::atan2(-f.x, -f.z));
    let shot = |at: glam::Vec3| game::spawn_backdrop::CameraShot { rot, pos: at + eye };
    // `cameras` takes view `i`'s camera from shot `2i + 1`.
    let lan_shots = [shot(luke), shot(luke), shot(leia), shot(leia)];
    let world = World {
        scene: &scene,
        shots: &lan_shots,
    };
    // The LAN round's sim: three humans, this machine's two on team 0, the remote on team 1.
    let mut setup = RoundSetup::instant_action(
        "bf2_hot1",
        "con",
        ["rep".into(), "cis".into()],
        &ui::shell::SessionOptions::default(),
        &game::profile::Profile::default(),
    );
    for (slot, name, team) in [(0u8, "Luke", 0u8), (1, "Lando", 1), (2, "Leia", 0)] {
        setup.humans.push(HumanSeat {
            slot,
            name: name.into(),
            team: Some(team),
            cheats: 0,
            profile: PlayerProfile {
                name: name.into(),
                ..PlayerProfile::default()
            },
        });
    }
    let mut walk = game::walk::WalkSim::new(1, luke, None);
    game::netplay::install_humans(&mut walk, &setup).expect("the round's humans");
    walk.adopt_local(2);
    for (slot, at) in [(0usize, luke), (2, leia), (1, lando)] {
        let h = walk.sim.players[slot].unwrap_or(walk.player);
        let u = &mut walk.sim.units[h.index as usize];
        u.alive = true;
        u.respawn_at = None;
        u.soldier.pos = at;
    }
    assert_eq!(walk.local_count(), 2);
    assert!(
        !walk.is_waiting(1) && !walk.is_suspended(1),
        "Leia is seated"
    );
    // The session's names (`App.names` in a LAN round).
    let mut session = game::player_names::PlayerNames::default();
    session.set([(0, "Luke"), (1, "Lando"), (2, "Leia")]);
    let name = |slot: u8| Some(session.name_or_default(slot));
    let size = [1920, 1080];
    let views = arranged(
        2,
        size[0],
        size[1],
        TwoPlayer::SideBySide,
        [0, 1, 2, 3],
        |i| i,
    );
    let cams = cameras(&world, &views);
    let mut tags = vec![game::name_tags::NameTags::default(); 2];
    let mut drawn: Vec<Vec<ui::hud::NameTagDraw>> = vec![Vec::new(); 2];
    for _ in 0..60 {
        for i in 0..2 {
            drawn[i] = game::name_tags::cell_tags(
                &mut tags[i],
                &walk.sim,
                walk.local_unit(i),
                &cams[i],
                views[i],
                1.0 / 60.0,
                &name,
            );
        }
    }
    let names_of = |i: usize| {
        let mut n: Vec<String> = drawn[i].iter().map(|t| t.name.clone()).collect();
        n.sort();
        n
    };
    assert_eq!(
        names_of(0),
        ["Lando", "Leia"],
        "Luke sees his co-local and the remote"
    );
    assert_eq!(
        names_of(1),
        ["Lando"],
        "Leia sees the remote; Luke is behind her"
    );
    let lando0 = drawn[0].iter().find(|t| t.name == "Lando").unwrap();
    let leia0 = drawn[0].iter().find(|t| t.name == "Leia").unwrap();
    assert!(
        !lando0.friend && leia0.friend,
        "the remote is an enemy, the co-local a friend"
    );
    let timed = || HudModel {
        time_left: Some(754.0),
        objective_timer: None,
        ..battle(None)
    };
    let models: Vec<Option<HudModel>> = drawn
        .iter()
        .map(|d| {
            Some(HudModel {
                name_tags: d.clone(),
                ..timed()
            })
        })
        .collect();
    let s = Scene {
        name: "b20_lan_tags",
        layout: TwoPlayer::SideBySide,
        order: [0, 1, 2, 3],
        hud_k: 1.0,
        models,
        icons: vec![pad(IconSet::XboxOne); 2],
        pause: None,
        menu_in_view: false,
        away: Vec::new(),
    };
    let timers = |ctx: &ui::egui::Context, cells: &[Cell], _: &[render::Camera]| {
        for (i, cell) in cells.iter().enumerate() {
            let t = game::name_tags::cell_text(ctx, *cell, 1.0);
            ui::hud::draw_hud_in(ctx, &timed(), t.rect, i + 1, t.scale, t.timer_at);
        }
    };
    capture(&world, &s, size, &out, Some(&timers));
}
