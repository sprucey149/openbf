//! HUD identity for the single view (splitscreen amendment B, packet B5,
//! `SPLITSCREEN.md` 18.2): every vertex, box and hit-test point the
//! one-player HUD makes, hashed at the gate's window sizes and at 1280 x 1024
//! (narrower than 4:3), pinned to what the code made before the split-cell
//! layout (`hud_draw::Frame`) went in. Uses the window-size entry points
//! only, so the same file ran on the code before the change to make the pins.

use crate::hud_ambush::{visible_box, Edge};
use crate::hud_capture::{picture, CaptureIcon, Mark, Side};
use crate::hud_draw::{draw, HudArt, HudFrame};
use crate::hud_map::{hit_spawn_post, spawn_window_point, window_point, MapModel, MapView};
use crate::hud_squad::{centre, label_anchor, SquadCross};
use crate::hud_tree::{EventValue, Hud};
use crate::spawn_draw::{help_at, help_icon_point, side_row_at, SpawnPage};
use crate::stats::{Page, StatsScreens};
use glam::Vec3;
use manifest::hud::{HudArg, HudFont, HudGlyph, HudLayout, HudNode};

/// The sizes pinned: the gate's three, then a window narrower than 4:3.
pub(crate) const SIZES: [[f32; 2]; 4] = [
    [640.0, 480.0],
    [1280.0, 720.0],
    [3840.0, 1080.0],
    [1280.0, 1024.0],
];

/// FNV-1a over 32-bit words: stable across toolchains.
#[derive(Clone, Copy)]
pub(crate) struct Fnv(u64);

impl Fnv {
    pub(crate) fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    pub(crate) fn word(&mut self, w: u32) {
        for b in w.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    pub(crate) fn f(&mut self, v: f32) {
        self.word(v.to_bits());
    }
    pub(crate) fn fs(&mut self, v: &[f32]) {
        for x in v {
            self.f(*x);
        }
    }
    pub(crate) fn s(&mut self, t: &str) {
        for b in t.bytes() {
            self.word(u32::from(b));
        }
        self.word(0xffff_ffff);
    }
}

/// Everything a frame carries, hashed.
pub(crate) fn frame_digest(f: &HudFrame) -> u64 {
    let mut h = Fnv::new();
    for v in &f.verts {
        h.fs(&v.pos);
        h.fs(&v.uv);
        h.fs(&v.color);
    }
    for i in &f.indices {
        h.word(*i);
    }
    for b in &f.batches {
        h.s(&b.texture);
        h.word(u32::from(b.additive) | u32::from(b.depth) << 1);
        h.word(b.first);
        h.word(b.count);
    }
    for (i, b) in &f.boxes {
        h.word(*i as u32);
        h.fs(b);
    }
    for (p, b, a) in &f.squad {
        h.s(&format!("{p:?}"));
        h.fs(b);
        h.f(*a);
    }
    for (p, b, c) in &f.capture {
        h.s(&format!("{p:?}"));
        h.fs(b);
        h.word(u32::from(c[0]) | u32::from(c[1]) << 8 | u32::from(c[2]) << 16);
    }
    for (e, b, a) in &f.ambush {
        h.s(&format!("{e:?}"));
        h.fs(b);
        h.f(*a);
    }
    h.0
}

fn node(name: &str, args: &[HudArg], children: Vec<HudNode>) -> HudNode {
    HudNode {
        name: name.into(),
        args: args.to_vec(),
        children,
    }
}

fn s(v: &str) -> HudArg {
    HudArg::Str(v.into())
}

fn n(v: f32) -> HudArg {
    HudArg::Num(v)
}

/// A shown group at viewport (`x`, `y`) holding a half-full bar over a
/// shadowed backing, named `name`.
fn bar(name: &str, x: f32, y: f32) -> HudNode {
    let rect = |w: f32| {
        node(
            "bitmaprect",
            &[n(w), n(0.05), s("Left"), s("Top"), s("Viewport")],
            vec![],
        )
    };
    node(
        "Group",
        &[s(name)],
        vec![
            node("Position", &[n(x), n(y), n(0.0), s("Viewport")], vec![]),
            node("eventenable", &[s("go")], vec![]),
            node(
                "barbitmap",
                &[s(&format!("{name}_fill"))],
                vec![
                    node("Bitmap", &[s("sheet")], vec![]),
                    rect(0.12),
                    node("zorder", &[n(2.0)], vec![]),
                    node("eventvalue", &[s("go")], vec![]),
                    node("eventenable", &[s("go")], vec![]),
                ],
            ),
            node(
                "barbitmap",
                &[s(&format!("{name}_back"))],
                vec![
                    node("Bitmap", &[s("sheet")], vec![]),
                    node("bitmapstyle", &[s("Shadow")], vec![]),
                    rect(0.12),
                    node("zorder", &[n(3.0)], vec![]),
                    node("eventenable", &[s("go")], vec![]),
                ],
            ),
        ],
    )
}

fn empty() -> HudLayout {
    HudLayout {
        name: "none".into(),
        nodes: Vec::new(),
    }
}

/// A font whose every printable glyph is 10 wide, 12 tall.
fn font(name: &str, line_height: u8) -> HudFont {
    HudFont {
        name: name.into(),
        line_height,
        pages: vec![format!("{name}_tex0")],
        glyphs: (32u8..127)
            .map(|code| HudGlyph {
                code: u16::from(code),
                page: 0,
                advance: 10,
                x0: 0,
                x1: if code == 32 { 0 } else { 10 },
                y0: 2,
                y1: 14,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect(),
    }
}

pub(crate) fn art() -> HudArt {
    let mut art = HudArt::default();
    for (name, lh) in [
        ("gamefont_large", 22),
        ("gamefont_small", 18),
        ("gamefont_tiny", 14),
        ("menu_gamefont_large", 22),
        ("menu_gamefont_small", 18),
        ("menu_gamefont_tiny", 14),
    ] {
        art.fonts.insert(name.into(), font(name, lh));
    }
    let part = crate::hud_draw::HudMeshPart {
        texture: "reticle".into(),
        positions: vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.5],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    };
    for name in ["hud_main_reticule_hit", "hud_main_reticule_criticalhit"] {
        art.meshes.insert(
            name.into(),
            crate::hud_draw::HudMesh {
                parts: vec![part.clone()],
            },
        );
    }
    art
}

/// Bars at every edge and the middle, and every code-drawn piece over them.
pub(crate) fn battle_hud() -> Hud {
    let layout = HudLayout {
        name: "t".into(),
        nodes: vec![
            bar("tl", 0.05, 0.04),
            bar("tr", 0.82, 0.04),
            bar("ml", 0.05, 0.47),
            bar("mid", 0.45, 0.5),
            bar("bl", 0.05, 0.9),
            bar("br", 0.82, 0.9),
        ],
    };
    let mut hud = Hud::new(&layout, &empty());
    hud.send("go", EventValue::Number(0.5), &|_| false);
    hud.advance(1.0);
    hud.squad = Some(SquadCross {
        labels: [
            "Follow".into(),
            "Hold".into(),
            "Attack".into(),
            "Defend".into(),
        ],
        arm: 2,
        count: Some(3),
        timer: 1.0,
    });
    hud.capture = Some(CaptureIcon {
        split: 0.4,
        top: Side::Own,
        bottom: Side::Enemy,
        mark: Some(Mark::Up),
    });
    hud.ambush.timers = [1.0, 0.75, 0.5, 0.25];
    hud.damage_flash = 0.3;
    hud
}

/// The reticle group at a projected point.
pub(crate) fn reticle_hud() -> Hud {
    let mut hud = crate::hud_tree::fixture::reticle_hud();
    let r = hud.find("player1reticule").unwrap();
    hud.elems[r].fader.show();
    hud.elems[r].window_pos = Some([0.5, 0.4375]);
    let hit = "player1.weapon1.target.hit";
    hud.send(hit, EventValue::None, &|_| true);
    hud.advance(0.05);
    hud
}

pub(crate) fn spawn_pages() -> [SpawnPage; 3] {
    [
        SpawnPage::Side {
            title: "Select a team".into(),
            rows: vec!["Empire".into(), "Rebels".into(), "Auto Assign".into()],
            selected: 1,
            select: "Select".into(),
        },
        SpawnPage::Class {
            title: "Rebels".into(),
            lines: vec!["Rifle".into(), "Pistol".into(), "Thermal detonator".into()],
            back: "Back".into(),
            accept: "Select Character".into(),
            team: 1,
            class: 0,
            classes: vec![0, 1, 2],
            refused: 0,
        },
        SpawnPage::Map {
            objective: "Destroy the shield generator before the reinforcements run out".into(),
            legend: ["{btna} Spawn".into(), "Select post".into(), "Back".into()],
        },
    ]
}

pub(crate) fn spawn_map() -> MapModel {
    MapModel {
        view: MapView::Spawn,
        radius: 200.0,
        extent: 200.0,
        ..Default::default()
    }
}

/// Every frame the single view draws in these fixtures, and every hit test,
/// hashed at `size`.
pub(crate) fn digest(size: [f32; 2]) -> u64 {
    let art = art();
    let mut h = Fnv::new();
    let mut frame = |hud: &Hud, art: &HudArt| h.word(frame_digest(&draw(hud, art, size)) as u32);
    let mut target = crate::hud_tree::fixture::target_hud();
    let ev = "player1.weapon1.target.healthFraction";
    target.send(ev, EventValue::Number(0.7), &|_| false);
    target.advance(1.0);
    frame(&target, &art);
    frame(&reticle_hud(), &art);
    let mut hud = battle_hud();
    frame(&hud, &art);
    hud.scope = Some(crate::hud_scope::ScopeView);
    frame(&hud, &art);
    hud.scope = None;
    for page in spawn_pages() {
        let mut p = Hud::new(&empty(), &empty());
        p.spawn = Some(page);
        if matches!(p.spawn, Some(SpawnPage::Map { .. })) {
            p.map = Some(spawn_map());
        }
        frame(&p, &art);
    }
    for page in [Page::Team, Page::Personal, Page::Career] {
        let mut p = Hud::new(&empty(), &empty());
        p.stats = Some(Box::new(StatsScreens {
            page,
            ..Default::default()
        }));
        frame(&p, &art);
    }
    for (result, text) in [(0u8, false), (1, true)] {
        let mut p = Hud::new(&empty(), &empty());
        p.banner = Some(result);
        p.banner_fade = 0.4;
        let mut a = art.clone();
        a.no_bf1_victory_sheet = text;
        frame(&p, &a);
    }
    // The hit tests and the points the game reads.
    let (w, ht) = (size[0], size[1]);
    for i in 0..=16 {
        for j in 0..=16 {
            let p = [w * i as f32 / 16.0, ht * j as f32 / 16.0];
            h.word(side_row_at(p, size, 3).map_or(99, |r| r as u32));
            for page in &spawn_pages() {
                h.word(help_at(None, page, p, size).map_or(9, |x| x as u32));
            }
        }
    }
    for right in [false, true] {
        h.fs(&help_icon_point(size, right));
    }
    h.fs(&crate::spawn_draw::window_point([100.0, 200.0], size));
    h.fs(&centre(size));
    for arm in 0..4 {
        h.fs(&label_anchor(arm, size));
    }
    h.fs(&picture(size));
    for e in Edge::ALL {
        h.fs(&visible_box(e, size));
    }
    for (r, uv) in crate::hud_scope::quads(size) {
        h.fs(&r);
        h.fs(&uv);
    }
    let map = spawn_map();
    let k = Vec3::splat(0.9);
    for world in [[0.0, 0.0], [100.0, 50.0], [-150.0, 120.0]] {
        h.fs(&spawn_window_point(&map, k, world, true, size).unwrap());
    }
    let posts = [(1u32, Vec3::ZERO), (2u32, Vec3::new(100.0, 0.0, 50.0))];
    let p2 = spawn_window_point(&map, k, [100.0, 50.0], true, size).unwrap();
    h.word(hit_spawn_post(&map, k, &posts, p2, size, 28.0).unwrap_or(0));
    let mut hud = Hud::new(&empty(), &empty());
    hud.map = Some(spawn_map());
    if let Some(p) = window_point(&hud, [10.0, 10.0], true, size) {
        h.fs(&p);
    }
    // A layout with a Map root: the minimap's (Small) and the Spawn and
    // Large views' window points.
    let mut hud = map_hud();
    for view in [MapView::Small, MapView::Spawn, MapView::Large] {
        hud.map = Some(MapModel {
            view,
            ..small_map()
        });
        for world in [[0.0, 0.0], [30.0, -20.0], [-150.0, 120.0]] {
            h.fs(&window_point(&hud, world, true, size).unwrap());
        }
    }
    h.0
}

/// A layout whose one root is the minimap (`Map`), high on the right.
pub(crate) fn map_hud() -> Hud {
    let layout = HudLayout {
        name: "m".into(),
        nodes: vec![node(
            "Map",
            &[s("player1map")],
            vec![node(
                "Position",
                &[n(0.8), n(0.03), n(0.0), s("Viewport")],
                vec![],
            )],
        )],
    };
    Hud::new(&layout, &empty())
}

/// The minimap's model: eye off the centre, turned.
pub(crate) fn small_map() -> MapModel {
    MapModel {
        view: MapView::Small,
        radius: 60.0,
        extent: 200.0,
        eye: [12.0, -7.0],
        heading: 0.4,
        ..Default::default()
    }
}

/// The digests the code made before `Frame` (private `main` at `26530897a`),
/// per [`SIZES`] entry; taken again when the tree took the scene's one draw
/// list, which reordered these frames' triangles and changed none of them.
const PINS: [u64; 4] = [
    0xa63f_a0f5_96ba_d608,
    0x47e3_acb2_0f4f_a5a7,
    0x6c0b_56ed_bd37_23eb,
    0x0d95_b43f_9407_1123,
];

/// The single view draws and hit-tests exactly as before, a window narrower
/// than 4:3 included.
#[test]
fn the_single_view_is_unchanged() {
    for (size, pin) in SIZES.into_iter().zip(PINS) {
        assert_eq!(digest(size), pin, "{size:?}: {:#018x}", digest(size));
    }
}

/// The fixtures draw something of every kind, so the pins hold real geometry.
#[test]
fn the_fixtures_draw() {
    let art = art();
    let f = draw(&battle_hud(), &art, [640.0, 480.0]);
    assert!(f.boxes.len() >= 6, "{}", f.boxes.len());
    assert!(!f.squad.is_empty() && !f.capture.is_empty() && f.ambush.len() == 4);
    let mut m = map_hud();
    m.map = Some(small_map());
    assert!(window_point(&m, [5.0, 5.0], true, [640.0, 480.0]).is_some());
    let r = draw(&reticle_hud(), &art, [640.0, 480.0]);
    assert!(!r.verts.is_empty(), "the reticle flash draws");
    for page in spawn_pages() {
        let mut p = Hud::new(&empty(), &empty());
        p.spawn = Some(page);
        p.map = Some(spawn_map());
        assert!(draw(&p, &art, [640.0, 480.0]).verts.len() > 8);
    }
}

/// A tall split cell (`SPLITSCREEN.md` 18.2 B5's check): a health bar and a
/// minimap at the cell's edges. The rest of the split-cell checks are in
/// `hud_frame_tests.rs`.
mod split_cell {
    use super::*;
    use crate::hud_draw::{draw_in, Frame, VAnchor};

    const CELL: [f32; 2] = [900.0, 1600.0];

    fn bounds_of(f: &HudFrame, hud: &Hud, name: &str) -> [f32; 4] {
        let ids: Vec<usize> = (0..hud.elems.len())
            .filter(|i| {
                let mut k = Some(*i);
                while let Some(j) = k {
                    if hud.elems[j].name == name {
                        return true;
                    }
                    k = hud.elems[j].parent;
                }
                false
            })
            .collect();
        f.boxes.iter().filter(|(i, _)| ids.contains(i)).fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |b, (_, x)| {
                [
                    b[0].min(x[0]),
                    b[1].min(x[1]),
                    b[2].max(x[2]),
                    b[3].max(x[3]),
                ]
            },
        )
    }

    /// A health bar low on the frame and a minimap high on it, as the
    /// 1-player layout has them.
    fn hud() -> Hud {
        let layout = HudLayout {
            name: "t".into(),
            nodes: vec![
                bar("player1health", 0.05, 0.9),
                bar("player1minimap", 0.8, 0.03),
            ],
        };
        let mut hud = Hud::new(&layout, &empty());
        hud.send("go", EventValue::Number(0.5), &|_| false);
        hud.advance(1.0);
        hud
    }

    #[test]
    fn the_rule_picks_each_roots_edge() {
        let h = hud();
        assert_eq!(
            h.v_anchor(h.find("player1health").unwrap()),
            VAnchor::Bottom
        );
        assert_eq!(h.v_anchor(h.find("player1minimap").unwrap()), VAnchor::Top);
        assert_eq!(
            h.v_anchor(h.find("player1health_fill").unwrap()),
            VAnchor::Bottom
        );
        let b = battle_hud();
        assert_eq!(b.v_anchor(b.find("mid").unwrap()), VAnchor::Centre);
        assert_eq!(b.v_anchor(b.find("ml").unwrap()), VAnchor::Centre);
    }

    #[test]
    fn health_hangs_at_the_bottom_and_the_minimap_at_the_top() {
        let h = hud();
        let f = Frame::cell(CELL, 1.0);
        let out = draw_in(&h, &art(), &f);
        let health = bounds_of(&out, &h, "player1health");
        let map = bounds_of(&out, &h, "player1minimap");
        assert!(
            health[3] <= CELL[1] && health[3] >= CELL[1] * 0.92,
            "{health:?}"
        );
        assert!(map[1] >= 0.0 && map[1] <= CELL[1] * 0.08, "{map:?}");
        // The minimap keeps its right-third place at the cell's right edge.
        assert!(map[2] <= CELL[0] && map[2] >= CELL[0] * 0.9, "{map:?}");
        // A smaller HUD size keeps the edges.
        let small = draw_in(&h, &art(), &Frame::cell(CELL, 0.7));
        let hs = bounds_of(&small, &h, "player1health");
        assert!(hs[3] >= CELL[1] * 0.92 && hs[3] - hs[1] < health[3] - health[1]);
    }
}
