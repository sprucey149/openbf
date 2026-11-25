//! The split cell's HUD screen (`hud_draw::Frame`, splitscreen amendment B,
//! packet B5): items at each cell's edges, panels centred, hit tests that
//! round-trip in a tall cell.

use super::frame_identity::{art, battle_hud, frame_digest, reticle_hud, spawn_map, spawn_pages};
use super::{draw, draw_in, vertex_bounds, Frame, VAnchor};
use crate::hud_ambush::{visible_box, visible_box_in, Edge};
use crate::hud_map::{hit_spawn_post_in, spawn_window_point_in};
use crate::hud_tree::Hud;
use crate::spawn_draw::{
    help_at_in, help_icon_point_in, side_row_at_in, window_point_in, Help, SpawnPage, ROW_H,
    ROW_PITCH, SIDE_STACK_TOP, SIDE_STACK_X,
};
use glam::Vec3;
use manifest::hud::HudLayout;

const TALL: [f32; 2] = [900.0, 1600.0];

fn empty() -> HudLayout {
    HudLayout {
        name: "none".into(),
        nodes: Vec::new(),
    }
}

fn huds() -> Vec<Hud> {
    let mut out = vec![battle_hud(), reticle_hud()];
    for page in spawn_pages() {
        let mut p = Hud::new(&empty(), &empty());
        p.spawn = Some(page);
        p.map = Some(spawn_map());
        out.push(p);
    }
    let mut stats = Hud::new(&empty(), &empty());
    stats.stats = Some(Default::default());
    out.push(stats);
    let mut banner = Hud::new(&empty(), &empty());
    banner.banner = Some(0);
    banner.banner_fade = 0.5;
    out.push(banner);
    let mut scope = battle_hud();
    scope.scope = Some(crate::hud_scope::ScopeView);
    out.push(scope);
    out
}

/// A split cell 4:3 or wider at the Full HUD size lays out exactly as a
/// single window of its size (the vertex identity at the gate's sizes).
#[test]
fn a_wide_cell_at_full_size_is_the_window_layout() {
    let art = art();
    for size in [[640.0, 480.0], [1280.0, 720.0], [3840.0, 1080.0]] {
        let f = Frame::cell(size, 1.0);
        assert_eq!(f, Frame::window(size));
        for (k, hud) in huds().iter().enumerate() {
            assert_eq!(
                frame_digest(&draw_in(hud, &art, &f)),
                frame_digest(&draw(hud, &art, size)),
                "hud {k} at {size:?}"
            );
        }
        for e in Edge::ALL {
            assert_eq!(visible_box_in(e, &f), visible_box(e, size));
        }
    }
}

fn elem_box(hud: &Hud, f: &crate::hud_draw::HudFrame, name: &str) -> [f32; 4] {
    let i = hud.find(name).unwrap();
    let b = f.boxes.iter().filter(|(e, _)| *e == i).fold(
        [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
        |b, (_, x)| {
            [
                b[0].min(x[0]),
                b[1].min(x[1]),
                b[2].max(x[2]),
                b[3].max(x[3]),
            ]
        },
    );
    assert!(b[0] <= b[2], "{name} drew");
    b
}

/// In a tall cell the top roots stay at the top edge, the bottom ones go to
/// the bottom edge, the middle ones to the middle; x keeps the thirds rule.
#[test]
fn a_tall_cell_pushes_roots_to_its_edges() {
    let hud = battle_hud();
    assert_eq!(hud.v_anchor(hud.find("tl").unwrap()), VAnchor::Top);
    assert_eq!(hud.v_anchor(hud.find("br_fill").unwrap()), VAnchor::Bottom);
    assert_eq!(hud.v_anchor(hud.find("mid").unwrap()), VAnchor::Centre);
    let f = Frame::cell(TALL, 1.0);
    let out = draw_in(&hud, &art(), &f);
    let h = TALL[1];
    let tl = elem_box(&hud, &out, "tl_back");
    let bl = elem_box(&hud, &out, "bl_back");
    let br = elem_box(&hud, &out, "br_back");
    let mid = elem_box(&hud, &out, "mid_back");
    assert!(tl[1] < 0.08 * h, "top: {tl:?}");
    assert!(bl[3] > 0.92 * h && bl[3] <= h, "bottom: {bl:?}");
    assert!(
        br[3] > 0.92 * h && br[2] <= TALL[0] + 1.0,
        "bottom right: {br:?}"
    );
    let c = (mid[1] + mid[3]) * 0.5;
    assert!((c / h - 0.5).abs() < 0.05, "middle: {mid:?}");
    // The box is the cell's width at Full, so x is the 4:3 frame's.
    assert!(br[2] > 0.9 * TALL[0]);
}

/// The HUD size setting shrinks the box; the edges still hold.
#[test]
fn a_smaller_hud_keeps_the_edges() {
    let hud = battle_hud();
    for k in [0.85, 0.7] {
        let f = Frame::cell(TALL, k);
        assert!((f.box_h - 900.0 * 0.75 * k).abs() < 1e-3);
        let out = draw_in(&hud, &art(), &f);
        let br = elem_box(&hud, &out, "br_back");
        let tl = elem_box(&hud, &out, "tl_back");
        assert!(
            br[3] > 0.92 * TALL[1] && br[2] > 0.9 * TALL[0],
            "{k}: {br:?}"
        );
        assert!(
            tl[1] < 0.08 * TALL[1] && tl[0] < 0.1 * TALL[0],
            "{k}: {tl:?}"
        );
    }
}

/// A projected point maps through the cell's band: the reticle at the
/// middle of the view is at the middle of the cell.
#[test]
fn the_reticle_lands_in_the_projection() {
    let f = Frame::cell(TALL, 1.0);
    let band = 900.0 * 0.75;
    // `ui_rect`'s whole pixels: (1600 - 675) / 2 floored.
    assert_eq!(f.proj, [0.0, 462.0, 900.0, band]);
    assert_eq!(f.project([0.5, 0.5]), [450.0, 799.5]);
    let hud = reticle_hud();
    let out = draw_in(&hud, &art(), &f);
    let b = vertex_bounds(&out.verts);
    let one = draw(&hud, &art(), [900.0, band]);
    let o = vertex_bounds(&one.verts);
    // The same reticle as a 900 x 675 window's, moved down into the band.
    for k in [0, 2] {
        assert!((b[k] - o[k]).abs() < 1e-3);
        assert!((b[k + 1] - (o[k + 1] + f.proj[1])).abs() < 1e-3);
    }
}

/// An off-screen marker clamped to the band's edge box (`marker_screen`'s 0.9) is drawn on the
/// cell's own edge box in a tall cell (issue #194, `SPLITSCREEN.md` 18.3): one straight above
/// or below reaches 0.9 of the way to the cell's top or bottom, not the band's; one to the side
/// keeps its direction. A wide cell and the single view are unchanged.
#[test]
fn an_off_screen_marker_clamps_to_the_cell() {
    let f = Frame::cell(TALL, 1.0);
    let [w, h] = TALL;
    let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2;
    let up = f.project_edge([0.0, -0.9]);
    assert!(near(up, [0.5 * w, 0.05 * h]), "{up:?}");
    let down = f.project_edge([0.0, 0.9]);
    assert!(near(down, [0.5 * w, 0.95 * h]), "{down:?}");
    // The band's clamp stops short: its top edge is the band's, well below the cell's.
    let band_up = f.project([0.5, 0.05]);
    assert!(band_up[1] > 0.3 * h, "{band_up:?}");
    // A marker to the right, a little up, keeps its direction from the cell's centre.
    let [x, y] = f.project_edge([0.9, -0.3]);
    let ([cx, cy], [bx, by]) = ([0.5 * w, 0.5 * h], f.project([0.95, 0.35]));
    // The band's centre is its whole-pixel one (`ui_rect`), half a pixel off the cell's.
    let [bcx, bcy] = f.project([0.5, 0.5]);
    let (dir, band_dir) = ((y - cy) / (x - cx), (by - bcy) / (bx - bcx));
    assert!((dir - band_dir).abs() < 1e-4, "{dir} vs {band_dir}");
    assert!((x - 0.95 * w).abs() < 1e-3, "on the right edge box: {x}");
    // A corner of the band's box: in a tall cell its line meets the cell's side first, so it
    // stays where the band put it (within the band's half-pixel centring).
    let [x, y] = f.project_edge([-0.9, 0.9]);
    let corner = f.project([0.05, 0.95]);
    assert!(
        (x - 0.05 * w).abs() < 1e-3 && (y - corner[1]).abs() <= 1.0,
        "{x} {y} {corner:?}"
    );
    // Wide cells and the single view project as before.
    for f in [
        Frame::cell([1920.0, 540.0], 1.0),
        Frame::window([1280.0, 720.0]),
    ] {
        for ndc in [[0.0, -0.9], [0.9, 0.2], [-0.4, 0.9]] {
            assert_eq!(
                f.project_edge(ndc),
                f.project([ndc[0] * 0.5 + 0.5, ndc[1] * 0.5 + 0.5])
            );
        }
    }
}

/// The spawn pages are a centred panel: the team rows round-trip, the help
/// lines sit at the cell's real bottom and answer the pointer there, and
/// the spawn map's posts round-trip.
#[test]
fn spawn_hit_tests_round_trip_in_a_tall_cell() {
    let f = Frame::cell(TALL, 1.0);
    let cx = (SIDE_STACK_X[0] + SIDE_STACK_X[1]) * 0.5;
    for row in 0..3 {
        let y = SIDE_STACK_TOP + ROW_PITCH * (row + 1) as f32 + ROW_H * 0.5;
        let p = window_point_in([cx, y], &f);
        assert!(p[1] > 0.2 * TALL[1] && p[1] < 0.8 * TALL[1], "{p:?}");
        assert_eq!(side_row_at_in(p, &f, 3), Some(row));
    }
    let [_, class, _]: [SpawnPage; 3] = spawn_pages();
    let (left, right) = (help_icon_point_in(&f, false), help_icon_point_in(&f, true));
    assert!(
        left[1] > 0.9 * TALL[1] && right[1] > 0.9 * TALL[1],
        "{left:?}"
    );
    assert_eq!(help_at_in(None, &class, left, &f), Some(Help::Back));
    assert_eq!(help_at_in(None, &class, right, &f), Some(Help::Accept));
    assert_eq!(help_at_in(None, &class, [450.0, 800.0], &f), None);
    let map = spawn_map();
    let k = Vec3::splat(0.9);
    let posts = [(1u32, Vec3::ZERO), (2u32, Vec3::new(100.0, 0.0, 50.0))];
    for (id, pos) in posts {
        let p = spawn_window_point_in(&map, k, [pos.x, pos.z], true, &f).unwrap();
        assert!(p[1] > 0.0 && p[1] < TALL[1]);
        assert_eq!(hit_spawn_post_in(&map, k, &posts, p, &f, 28.0), Some(id));
    }
}

/// The squad cross hangs at the bottom right, the capture icon at the top
/// right under the minimap column, the side chevrons at the middle and the
/// end ones at the edges.
#[test]
fn code_drawn_pieces_take_their_corners() {
    let f = Frame::cell(TALL, 1.0);
    let c = crate::hud_squad::centre_in(&f);
    assert!(c[0] > 0.6 * TALL[0] && c[1] > 0.8 * TALL[1], "squad {c:?}");
    let pic = crate::hud_capture::picture_in(&f);
    // Where a 900 x 675 window has it: the box hangs from the cell's top.
    assert_eq!(pic, crate::hud_capture::picture([900.0, 675.0]));
    assert!(
        pic[2] > 0.6 * TALL[0] && pic[3] < 0.3 * TALL[1],
        "capture {pic:?}"
    );
    let l = visible_box_in(Edge::Left, &f);
    assert!(
        ((l[1] + l[3]) * 0.5 / TALL[1] - 0.5).abs() < 0.05,
        "left {l:?}"
    );
    let b = visible_box_in(Edge::Bottom, &f);
    assert!(b[3] > 0.95 * TALL[1], "bottom {b:?}");
    let t = visible_box_in(Edge::Top, &f);
    assert!(t[1] < 0.05 * TALL[1], "top {t:?}");
}

/// The stats page and the banner are centred; the black and the veil cover
/// the whole cell.
#[test]
fn panels_centre_and_covers_fill_the_cell() {
    let f = Frame::cell(TALL, 1.0);
    let mut stats = Hud::new(&empty(), &empty());
    stats.stats = Some(Default::default());
    let out = draw_in(&stats, &art(), &f);
    assert_eq!(&out.verts[0].pos[..2], &[0.0, 0.0]);
    assert_eq!(&out.verts[2].pos[..2], &TALL);
    let rest = vertex_bounds(&out.verts[4..]);
    assert!(
        rest[1] > 0.2 * TALL[1],
        "panel starts in the middle: {rest:?}"
    );
    assert!(
        rest[3] > 0.9 * TALL[1],
        "its footer at the bottom: {rest:?}"
    );
    let mut banner = Hud::new(&empty(), &empty());
    banner.banner = Some(1);
    let b = vertex_bounds(&draw_in(&banner, &art(), &f).verts);
    assert!((((b[1] + b[3]) * 0.5) - 800.0).abs() < 1e-3, "{b:?}");
}

/// The panel is `ui_rect`'s band at every HUD size, so the class page's
/// boxes stay on the 3D line-up and the game's `class_at` band; only the
/// HUD box shrinks.
#[test]
fn the_panel_is_the_ui_rect_band_at_every_hud_size() {
    // `game::split_layout::ui_rect` for each cell, relative to it.
    for (cell, y, band) in [
        (TALL, 462.0, 675.0),
        ([683.0, 1600.0], 544.0, 512.0),
        ([960.0, 1080.0], 180.0, 720.0),
        ([1280.0, 720.0], 0.0, 720.0),
    ] {
        let full = Frame::cell(cell, 1.0).panel();
        for k in [0.85, 0.7] {
            let f = Frame::cell(cell, k);
            assert_eq!(f.panel(), full, "{cell:?} at {k}");
            assert_eq!([f.proj[1], f.proj[3]], [y, band], "{cell:?}");
        }
        assert_eq!((full.dy, full.size[1]), (y, band), "{cell:?}");
        let [_, class, _]: [SpawnPage; 3] = spawn_pages();
        let mut p = Hud::new(&empty(), &empty());
        p.spawn = Some(class);
        let art = art();
        assert_eq!(
            frame_digest(&draw_in(&p, &art, &Frame::cell(cell, 0.7))),
            frame_digest(&draw_in(&p, &art, &Frame::cell(cell, 1.0))),
            "the class page at {cell:?}"
        );
    }
}
