//! The real BF2 1-player layout in a tall split cell (`SPLITSCREEN.md` 18.2,
//! B5's check): each root hangs at the edge the anchor rule gives it.
//! Ignored by default: needs the baked `common_bf2/hud` (`engine/assets_baked`,
//! or the bake `HUD_ASSETS` names).
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim
use ui::hud_draw::{draw_in, Frame, HudArt, VAnchor};
use ui::hud_tree::{Hud, Kind};

fn read<T: serde::de::DeserializeOwned>(p: &std::path::Path) -> T {
    ron::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
#[ignore = "requires the baked common_bf2/hud"]
fn real_layout_hangs_at_the_cells_edges() {
    // `HUD_ASSETS` names another bake (a worktree has none of its own).
    let root = std::env::var_os("HUD_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root);
    let mut hud = Hud::new(
        &read(&root.join("common_bf2/hud/1playerhud.ron")),
        &read(&root.join("common_bf2/hud/hudtransforms.ron")),
    );
    ui::hud::place_countdown_labels(&mut hud);
    for r in hud.roots.clone() {
        println!("{:40} {:?}", hud.elems[r].name, hud.v_anchor(r));
    }
    let find = |n: &str| {
        hud.roots
            .iter()
            .copied()
            .find(|r| hud.elems[*r].name.eq_ignore_ascii_case(n))
            .unwrap_or_else(|| panic!("{n}"))
    };
    let health = find("player1healthbar");
    let map = hud
        .roots
        .iter()
        .copied()
        .find(|r| hud.elems[*r].kind == Kind::Map)
        .unwrap();
    assert_eq!(hud.v_anchor(health), VAnchor::Bottom);
    assert_eq!(hud.v_anchor(map), VAnchor::Top);
    assert_eq!(hud.v_anchor(find("player1reticule")), VAnchor::Centre);
    assert_eq!(
        hud.v_anchor(find("player1weaponinformation")),
        VAnchor::Bottom
    );
    assert_eq!(hud.v_anchor(find("player1objectives")), VAnchor::Top);
    let cell = [900.0, 1600.0];
    let f = Frame::cell(cell, 1.0);
    let h = ui::hud_draw::root_y_span(&hud, health, &f).unwrap();
    let m = ui::hud_draw::root_y_span(&hud, map, &f).unwrap();
    println!("health {h:?} minimap {m:?}");
    assert!(h[1] >= cell[1] * 0.92 && h[1] <= cell[1], "{h:?}");
    assert!(m[0] >= -cell[1] * 0.02 && m[0] <= cell[1] * 0.08, "{m:?}");
    // The whole tree lays out in the cell.
    let out = draw_in(&hud, &HudArt::default(), &f);
    for (_, b) in &out.boxes {
        assert!(b[1] >= -cell[1] * 0.05 && b[3] <= cell[1] * 1.05, "{b:?}");
    }
}

/// Issue #194 B8 (`SPLITSCREEN.md` 18.1 item 6): a split cell's round timer
/// ([`ui::hud::SPLIT_TIMER_TOP`] of the cell's HUD box) sits in the
/// reinforcement counts' row, left of the counts, at every cell shape and HUD
/// size.
#[test]
#[ignore = "requires the baked common_bf2/hud"]
fn the_split_timer_sits_in_the_counts_row() {
    let root = std::env::var_os("HUD_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root);
    let mut hud = Hud::new(
        &read(&root.join("common_bf2/hud/1playerhud.ron")),
        &read(&root.join("common_bf2/hud/hudtransforms.ron")),
    );
    ui::hud::place_countdown_labels(&mut hud);
    let find = |n: &str| {
        hud.roots
            .iter()
            .copied()
            .find(|r| hud.elems[*r].name.eq_ignore_ascii_case(n))
            .unwrap_or_else(|| panic!("{n}"))
    };
    let teams = find("player1teams");
    for cell in [
        [960, 1080],
        [1920, 540],
        [960, 540],
        [640, 1080],
        [640, 360],
    ] {
        for k in [1.0, 0.85, 0.7] {
            let c = [0, 0, cell[0], cell[1]];
            let f = game::split_ui::cell_frame(c, k);
            let t = game::name_tags::cell_timer_box(c, k);
            let [left, top, _, bottom] = ui::hud_draw::root_span(&hud, teams, &f).unwrap();
            let mid = 0.5 * (t[1] + t[3]);
            assert!(
                (mid - 0.5 * (top + bottom)).abs() < 0.01 * f.box_h,
                "timer {t:?} not centred on the counts row {top}..{bottom} ({cell:?}, {k})"
            );
            // Left of the counts, with room for the left count's fourth digit.
            let centre = 0.5 * cell[0] as f32;
            assert!(
                centre - left < ui::hud::SPLIT_COUNTS_HALF * f.box_h - 0.05 * f.box_h,
                "the counts reach {left} ({cell:?}, {k})"
            );
            assert!(
                t[2] <= centre - ui::hud::SPLIT_COUNTS_HALF * f.box_h + 1e-3,
                "{t:?} ({cell:?}, {k})"
            );
        }
    }
}

/// Integration finding (issue #194): a split cell's round timer box
/// (`game::name_tags::cell_timer_box`) never covers the event feed, the
/// counts, the target bar, the minimap, the CTF flags, the objective and
/// victory countdowns, nor the reticle where the game draws it (the PS2 aim
/// point, `game::walk::ps2_reticle_y`, on the cell's 4:3 band, for BF1 and BF2
/// alike; the layout's own `player1reticule` root is the band's centre), and
/// stays in its cell, at every layout's cell shape and HUD size.
#[test]
#[ignore = "requires the baked common_bf2/hud"]
fn the_split_timer_clears_the_hud() {
    let root = std::env::var_os("HUD_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root);
    let mut hud = Hud::new(
        &read(&root.join("common_bf2/hud/1playerhud.ron")),
        &read(&root.join("common_bf2/hud/hudtransforms.ron")),
    );
    ui::hud::place_countdown_labels(&mut hud);
    let find = |n: &str| {
        hud.roots
            .iter()
            .copied()
            .find(|r| hud.elems[*r].name.eq_ignore_ascii_case(n))
            .unwrap_or_else(|| panic!("{n}"))
    };
    let reticle = find("player1reticule");
    let clear_of = [
        "player1systemMessages",
        "player1teams",
        "player1targetnamegroup",
        "player1targethealthbar",
        "player1reticule",
        "player1flags",
        "objectivetimerGROUP",
        "VictoryTimerGroup",
    ]
    .map(find);
    let map = hud
        .roots
        .iter()
        .copied()
        .find(|r| hud.elems[*r].kind == Kind::Map)
        .unwrap();
    // Every cell shape the layouts make (two side by side or stacked, three, four), at 1080p,
    // 720p, a 4K 16:10 window and a 4:3 one.
    let mut cells = Vec::new();
    for (w, h) in [(1920u32, 1080u32), (1280, 720), (3840, 2400), (1600, 1200)] {
        cells.extend([[w / 2, h], [w, h / 2], [w / 2, h / 2], [w / 3, h]]);
    }
    for cell in cells {
        for k in [1.0, 0.85, 0.7] {
            let c = [0, 0, cell[0], cell[1]];
            let f = game::split_ui::cell_frame(c, k);
            let t = game::name_tags::cell_timer_box(c, k);
            assert!(
                t[0] >= 0.0 && t[2] <= cell[0] as f32 && t[1] >= 0.0,
                "{t:?} out of {cell:?}"
            );
            let overlaps = |b: [f32; 4]| t[0] < b[2] && b[0] < t[2] && t[1] < b[3] && b[1] < t[3];
            for r in clear_of {
                let b = ui::hud_draw::root_span(&hud, r, &f).unwrap();
                assert!(
                    !overlaps(b),
                    "timer {t:?} covers {} {b:?} ({cell:?}, {k})",
                    hud.elems[r].name
                );
            }
            // The drawn reticle: the root's size, centred on the aim point in the band.
            let [x0, y0, x1, y1] = ui::hud_draw::root_span(&hud, reticle, &f).unwrap();
            let [px, py, pw, ph] = f.proj;
            for game_name in ["bf1", "bf2"] {
                let at = [
                    px + 0.5 * pw,
                    py + game::walk::ps2_reticle_y(game_name) * ph,
                ];
                let half = [0.5 * (x1 - x0), 0.5 * (y1 - y0)];
                let drawn = [
                    at[0] - half[0],
                    at[1] - half[1],
                    at[0] + half[0],
                    at[1] + half[1],
                ];
                assert!(
                    !overlaps(drawn),
                    "timer {t:?} covers the {game_name} reticle {drawn:?} ({cell:?}, {k})"
                );
                assert!(t[3] < drawn[1] - 0.02 * f.box_h, "{t:?} vs {drawn:?}");
            }
            // The minimap is drawn round its root's point, 0.11 of the box out.
            let m = ui::hud_draw::root_span(&hud, map, &f).unwrap();
            let r = 0.11 * f.box_h;
            let b = [m[0] - r, m[1] - r, m[0] + r, m[1] + r];
            assert!(
                !overlaps(b),
                "timer {t:?} covers the minimap {b:?} ({cell:?}, {k})"
            );
        }
    }
}

fn hud_root() -> std::path::PathBuf {
    std::env::var_os("HUD_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root)
}

fn tagged(name: &str) -> ui::hud::HudModel {
    ui::hud::HudModel {
        hp: 200.0,
        hp_max: 300.0,
        name_tags: vec![ui::hud::NameTagDraw {
            at: [640.0, 300.0],
            name: name.into(),
            friend: true,
            alpha: 1.0,
        }],
        ..Default::default()
    }
}

/// Issue #194 B22, the LAN path: a single view's model with one tag draws its glyphs through
/// `HudView::update`, in the real event feed's font; and the name keyboard's characters all
/// have glyphs there (the ones the font lacks are printed: a feed line drops the same ones).
#[test]
#[ignore = "requires the baked common_bf2/hud"]
fn a_lan_frame_draws_its_name_tag_in_the_feed_font() {
    let mut hv = game::hud_feed::HudView::load(&hud_root(), wgpu::TextureFormat::Rgba8Unorm, None)
        .expect("the BF2 HUD bake");
    hv.update(&tagged("Leia"), 0.0, [1280, 720]);
    let boxes = hv.name_tag_boxes();
    assert_eq!(boxes.len(), 4, "a glyph per letter: {boxes:?}");
    for b in &boxes {
        assert!(b[3] <= 300.0 + 1e-3 && b[1] >= 250.0, "{b:?}");
    }
    let keys = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
        "0123456789!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~",
    ];
    let mut dropped = String::new();
    for page in keys {
        for c in page.chars() {
            hv.update(&tagged(&c.to_string()), 0.0, [1280, 720]);
            if hv.name_tag_boxes().is_empty() {
                dropped.push(c);
            }
        }
    }
    println!("name keyboard characters the feed font lacks: {dropped:?}");
    assert!(
        dropped.chars().all(|c| !c.is_ascii_alphanumeric()),
        "letters and digits all draw: {dropped:?}"
    );
}

/// Issue #194 B22: under the Game Menu (a split battle's, or LAN's native one) the HUD lays out
/// the menu, never the tags.
#[test]
#[ignore = "requires the baked common_bf2/hud"]
fn a_paused_frame_draws_no_name_tags() {
    let mut hv = game::hud_feed::HudView::load(&hud_root(), wgpu::TextureFormat::Rgba8Unorm, None)
        .expect("the BF2 HUD bake");
    let menu = ui::menu::MenuModel {
        page: Some(ui::menu::PausePage::Main),
        paused_by: Some("Paused by Leia".into()),
        ..Default::default()
    };
    hv.update_pause(
        &tagged("Han"),
        &menu,
        &ui::pause_draw::Animation::default(),
        [1280, 720],
    );
    assert!(hv.name_tag_boxes().is_empty());
    hv.update(&tagged("Han"), 0.0, [1280, 720]);
    assert!(!hv.name_tag_boxes().is_empty(), "unpaused, it draws");
}
