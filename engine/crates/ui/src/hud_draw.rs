//! The HUD laid out (card BHUD2, `analysis/re-hud.md` §2.4, §4): the elements
//! of [`crate::hud_tree::Hud`] turned into window-pixel triangles, back to
//! front, for the renderer's HUD pass (`render::hud`).
//!
//! One space carries everything: the PS2's 512 × 448 frame. Element matrices
//! nest in it, a bitmap rect and a glyph texel are its pixels, and a Model3D
//! mesh unit is [`MESH_UNIT`] of them (the draw scales the translation by
//! 0.01 × 1.24 under one camera — the same thing seen from the mesh). The
//! frame then lands on a 4:3 box of the window's height by the PS2's own
//! overfill ([`OVERFILL`]), which is where the 1.128 stretch of meshes and
//! glyphs comes from: 512 × 448 pixels shown 4:3 are not square. The port
//! keeps it (FEEL_NOTES).
//!
//! `[ours]`: Hor+ — every top-level element keeps its 4:3 offset from the
//! left edge, the centre or the right edge of a wider window, picked by its
//! own x (`anchor_shift`); meshes are drawn orthographically (the PS2's
//! perspective camera sits 6.8 mesh units away).

use crate::hud_tree::{trs, Elem, HAlign, Hud, Kind, VAlign, FRAME_H, FRAME_W, SHADOW_SPACING};
use glam::{Mat4, Vec3};
use manifest::hud::{HudFont, HudGlyph};
use std::collections::HashMap;

#[path = "campaign_map.rs"]
pub mod campaign;
#[path = "hud_text.rs"]
mod text;

#[cfg(test)]
#[path = "hud_frame_identity.rs"]
pub(crate) mod frame_identity;

#[cfg(test)]
#[path = "hud_frame_tests.rs"]
mod frame_tests;

/// Frame pixels per Model3D mesh unit: 1 / (0.01 × 1.24) (`re-hud.md` §2.4
/// [V] `FUN_0059fdb0`, DAT_0060c7b8).
pub const MESH_UNIT: f32 = 1.0 / (0.01 * 1.24);

/// The 1-player HUD's frame → display mapping (`re-hud.md` §2.4 [D][O]):
/// display fraction = offset + gain × frame fraction, per axis.
pub const OVERFILL: [(f32, f32); 2] = [(-0.0059, 1.0139), (-0.0242, 1.0484)];

/// How many Model3D draws share the depth range; each gets its own slice so
/// a later mesh always covers an earlier one.
const DEPTH_SLICES: f32 = 64.0;

/// Mesh units → frame pixels: the mesh's Y is up, the frame's is down
/// (`FUN_0059fdb0` negates the Y row).
pub(crate) fn mesh_frame() -> Mat4 {
    Mat4::from_scale(Vec3::new(MESH_UNIT, -MESH_UNIT, MESH_UNIT))
}

/// Ends the texture name of a mesh part whose sheet is drawn without its
/// alpha plane (a world model's opaque material: a weapon's sheet keeps its
/// gloss there).
pub const OPAQUE_SHEET: &str = "#opaque";

pub const SCROLL_SHEET: &str = "#scroll";

/// One mesh part: a texture and its triangles.
#[derive(Debug, Clone, Default)]
pub struct HudMeshPart {
    /// The sheet's name, with [`OPAQUE_SHEET`] after it where its alpha is
    /// not coverage.
    pub texture: String,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Vertex colours, display values 0..1 with alpha (the disc's COL0 at
    /// 0x80 = 1); empty = white.
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub scroll: [f32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct HudMesh {
    pub parts: Vec<HudMeshPart>,
}

/// What the layout needs beside the tree: glyph metrics and mesh geometry.
#[derive(Debug, Clone, Default)]
pub struct HudArt {
    pub fonts: HashMap<String, HudFont>,
    pub distance_fonts: HashMap<String, manifest::hud::HudDistanceFont>,
    pub meshes: HashMap<String, HudMesh>,
    /// Width / height of the sheets a `bitmap:<sheet>` weapon icon may name.
    pub sheet_aspect: HashMap<String, f32>,
    /// The control prompts' icons (`crate::input_icons`).
    pub icons: crate::input_icons::Icons,
    /// The screen of the Instant Action backdrop's centre device, in shell
    /// units, measured on this copy's art (`crate::shell_ia::device_screen`);
    /// `None` keeps the PS2 art's ([`crate::shell_ia::DEVICE_SCREEN`]).
    pub instant_device: Option<[f32; 4]>,
    /// True on BF2-only builds where `common_bf1/ig_victory_defeat` is absent,
    /// so the round-end banner falls back to text (`DrawCtx::round_end_text`).
    pub no_bf1_victory_sheet: bool,
}

impl HudArt {
    /// The shell and its pointer queries use this same role selection.
    pub fn menu_font(&self, role: &str) -> Option<&HudFont> {
        manifest::hud::MENU_FONTS
            .iter()
            .find(|(_, legacy)| *legacy == role)
            .and_then(|(menu, _)| self.fonts.get(*menu))
            .or_else(|| self.fonts.get(role))
    }

    pub fn has_mesh(&self, name: &str) -> bool {
        self.meshes.contains_key(name)
    }
}

/// Window pixels (x right, y down), depth 0 near … 1 far; `color` is display
/// (sRGB encoded) RGB with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// Window bounds of emitted vertices; an empty iterator keeps the extrema.
pub(crate) fn vertex_bounds<'a>(vertices: impl IntoIterator<Item = &'a HudVertex>) -> [f32; 4] {
    vertices
        .into_iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, v| {
            [
                b[0].min(v.pos[0]),
                b[1].min(v.pos[1]),
                b[2].max(v.pos[0]),
                b[3].max(v.pos[1]),
            ]
        })
}

/// A run of triangles sharing one texture and state, in draw order.
#[derive(Debug, Clone, PartialEq)]
pub struct HudBatch {
    /// A sheet, a font page or a mesh texture by name.
    pub texture: String,
    pub additive: bool,
    /// A mesh: depth-tested against itself.
    pub depth: bool,
    pub first: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct HudFrame {
    pub verts: Vec<HudVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<HudBatch>,
    /// Per drawn element: its index in [`Hud::elems`] and its window box
    /// `[x0, y0, x1, y1]` (shadows left out).
    pub boxes: Vec<(usize, [f32; 4])>,
    /// The squad cross's parts as drawn (`crate::hud_squad`): the part, its
    /// window box and its alpha.
    pub squad: Vec<(crate::hud_squad::Part, [f32; 4], f32)>,
    /// The capture icon's parts as drawn (`crate::hud_capture`): the part,
    /// its window box and its colour.
    pub capture: Vec<(crate::hud_capture::Part, [f32; 4], [u8; 3])>,
    /// The hit-direction chevrons as drawn (`crate::hud_ambush`): the edge,
    /// the sheet's window box and its alpha.
    pub ambush: Vec<(crate::hud_ambush::Edge, [f32; 4], f32)>,
    meshes: u32,
}

impl HudFrame {
    pub(crate) fn batch(&mut self, texture: &str, additive: bool, depth: bool, first: u32) {
        let count = self.indices.len() as u32 - first;
        if count == 0 {
            return;
        }
        if let Some(b) = self.batches.last_mut() {
            if !depth
                && !b.depth
                && b.additive == additive
                && b.texture == texture
                && b.first + b.count == first
            {
                b.count += count;
                return;
            }
        }
        self.batches.push(HudBatch {
            texture: texture.to_owned(),
            additive,
            depth,
            first,
            count,
        });
    }
}

/// Frame pixels → window pixels for one top-level element.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Screen {
    pub(crate) origin: [f32; 2],
    pub(crate) scale: [f32; 2],
}

impl Screen {
    pub(crate) fn map(&self, p: Vec3) -> [f32; 2] {
        [
            self.origin[0] + self.scale[0] * p.x,
            self.origin[1] + self.scale[1] * p.y,
        ]
    }
}

/// Window pixels per frame pixel: the 4:3 box of the window height.
pub fn frame_scale(size: [f32; 2]) -> [f32; 2] {
    let h = size[1];
    [
        OVERFILL[0].1 / FRAME_W * (4.0 / 3.0) * h,
        OVERFILL[1].1 / FRAME_H * h,
    ]
}

/// PROVISIONAL `[ours]` Hor+ thirds: how far right of the 4:3 box's own place an element whose
/// origin sits at frame fraction `fx` is drawn — nothing for the left third,
/// the whole extra width for the right third, half of it between.
pub fn anchor_shift(fx: f32, size: [f32; 2]) -> f32 {
    let extra = size[0] - 4.0 / 3.0 * size[1];
    if fx < 1.0 / 3.0 {
        0.0
    } else if fx > 2.0 / 3.0 {
        extra
    } else {
        extra * 0.5
    }
}

/// `a + b`, with `a` itself when `b` is 0 (so a single window's numbers keep
/// their exact bits, a `-0.0` included).
fn plus(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        a
    } else {
        a + b
    }
}

/// Where a HUD root hangs up and down in a screen taller than its 4:3 box
/// (`[ours]`, splitscreen amendment B, `SPLITSCREEN.md` 18.1 item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAnchor {
    Top,
    #[default]
    Centre,
    Bottom,
}

impl VAnchor {
    /// The share of the spare height above the box.
    fn share(self) -> f32 {
        match self {
            VAnchor::Top => 0.0,
            VAnchor::Centre => 0.5,
            VAnchor::Bottom => 1.0,
        }
    }
}

/// PROVISIONAL `[ours]` roots whose box centre would pick the wrong edge, by a
/// part of their name (lower case): the weapon panel, the jet pack bar and the
/// vehicle seats hang at the bottom, the countdowns and the flag readout at
/// the top, the popups and the spawn and hacking messages in the middle.
/// measure: the maintainer's look at the split captures (B6).
const V_ANCHOR_TABLE: [(&str, VAnchor); 10] = [
    ("weaponinformation", VAnchor::Bottom),
    ("jetpackbar", VAnchor::Bottom),
    ("vehicleseatinggroup", VAnchor::Bottom),
    ("objectivetimergroup", VAnchor::Top),
    ("victorytimergroup", VAnchor::Top),
    ("defeattimergroup", VAnchor::Top),
    ("flags", VAnchor::Top),
    ("hintpopup", VAnchor::Centre),
    ("spawnmessage", VAnchor::Centre),
    ("spawnvehicle", VAnchor::Centre),
];

/// The table's entries that keep a root centred, beside [`V_ANCHOR_TABLE`]
/// (the reticle is placed by its projected point; when it has none it stays
/// in the middle too).
const V_ANCHOR_CENTRE: [&str; 3] = ["borderbox", "hacking", "reticule"];

/// The rule for root `root` of `hud`: the table, else where the centre of its
/// subtree's frame-space box falls (above 0.4 of the frame Top, below 0.6
/// Bottom, else Centre).
pub(crate) fn root_v_anchor(hud: &Hud, root: usize) -> VAnchor {
    let name = hud.elems[root].name.to_ascii_lowercase();
    if let Some((_, a)) = V_ANCHOR_TABLE.iter().find(|(k, _)| name.contains(k)) {
        return *a;
    }
    if V_ANCHOR_CENTRE.iter().any(|k| name.contains(k)) {
        return VAnchor::Centre;
    }
    let mut b = [f32::MAX, f32::MIN];
    subtree_y(hud, root, &Mat4::IDENTITY, &mut b);
    if b[0] > b[1] {
        return VAnchor::Centre;
    }
    let c = (b[0] + b[1]) * 0.5 / FRAME_H;
    if c < 0.4 {
        VAnchor::Top
    } else if c > 0.6 {
        VAnchor::Bottom
    } else {
        VAnchor::Centre
    }
}

/// Where root `root`'s rects (element origins for a mesh) reach up and down
/// in the screen `f`, in pixels: top, bottom. `None` when it has nothing.
pub fn root_y_span(hud: &Hud, root: usize, f: &Frame) -> Option<[f32; 2]> {
    let mut b = [f32::MAX, f32::MIN];
    subtree_y(hud, root, &Mat4::IDENTITY, &mut b);
    let e = &hud.elems[root];
    let s = f.tree(e.pos.x / FRAME_W, hud.v_anchor(root));
    (b[0] <= b[1]).then(|| [b[0], b[1]].map(|y| s.origin[1] + s.scale[1] * y))
}

/// Where root `root`'s rects (element origins for a mesh) reach in the screen `f`, in pixels:
/// left, top, right, bottom ([`root_y_span`] with its x). `None` when it has nothing. Issue #194:
/// the split round timer's tests keep it clear of the feed and the minimap with it.
pub fn root_span(hud: &Hud, root: usize, f: &Frame) -> Option<[f32; 4]> {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    subtree_xy(hud, root, &Mat4::IDENTITY, &mut b);
    let e = &hud.elems[root];
    let s = f.tree(e.pos.x / FRAME_W, hud.v_anchor(root));
    (b[0] <= b[2] && b[1] <= b[3]).then(|| {
        [
            s.origin[0] + s.scale[0] * b[0],
            s.origin[1] + s.scale[1] * b[1],
            s.origin[0] + s.scale[0] * b[2],
            s.origin[1] + s.scale[1] * b[3],
        ]
    })
}

/// Grow `b` (min y, max y) by element `i`'s frame-space extent under `parent`.
fn subtree_y(hud: &Hud, i: usize, parent: &Mat4, b: &mut [f32; 2]) {
    let mut xy = [f32::MAX, b[0], f32::MIN, b[1]];
    subtree_xy(hud, i, parent, &mut xy);
    *b = [xy[1], xy[3]];
}

/// Grow `b` (min x, min y, max x, max y) by element `i`'s frame-space extent under `parent`.
fn subtree_xy(hud: &Hud, i: usize, parent: &Mat4, b: &mut [f32; 4]) {
    let e = &hud.elems[i];
    if e.is_segment {
        return;
    }
    let m = *parent * e.local();
    let (ox, oy) = e.rect.origin();
    let mut grow = |x: f32, y: f32| {
        let p = m.transform_point3(Vec3::new(x, y, 0.0));
        b[0] = b[0].min(p.x);
        b[1] = b[1].min(p.y);
        b[2] = b[2].max(p.x);
        b[3] = b[3].max(p.y);
    };
    match e.kind {
        Kind::Group => {
            let m = m * Mat4::from_translation(Vec3::new(ox, oy, 0.0));
            for &c in &e.children {
                subtree_xy(hud, c, &m, b);
            }
        }
        _ => {
            grow(ox, oy);
            grow(ox + e.rect.w, oy + e.rect.h);
        }
    }
}

/// The screen a HUD is laid out for (`[ours]`, `SPLITSCREEN.md` 18.1 item
/// 5): a 4:3 box `box_h` high, its items pushed to the screen's edges, and
/// the rectangle a projected point's 0..1 position maps over.
///
/// [`Frame::window`] is the single view's: the box is the window's height
/// and its numbers are [`frame_scale`] and [`anchor_shift`] exactly, a
/// window narrower than 4:3 included. [`Frame::cell`] is a split cell's:
/// the box fits inside the cell (times the HUD size setting), x keeps the
/// thirds rule on the spare width, y hangs each root at the top, the
/// middle or the bottom of the spare height ([`VAnchor`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The screen in pixels (the window, or the cell, whose top-left is 0, 0).
    pub size: [f32; 2],
    /// The 4:3 box's height.
    pub box_h: f32,
    /// x, y, w, h: where a projected point's 0..1 lands (a split cell's
    /// unwidened 4:3 band, `game::split_layout::ui_rect` relative to the cell).
    pub proj: [f32; 4],
    /// y, h: the centred panel's band ([`Frame::panel`]), the projection's,
    /// so the class page's boxes stay on the 3D line-up and the game's
    /// `class_at` band at every HUD size (the setting does not shrink it).
    pub panel: [f32; 2],
}

impl Frame {
    /// The single view's window (today's layout).
    pub fn window(size: [f32; 2]) -> Self {
        Frame {
            size,
            box_h: size[1],
            proj: [0.0, 0.0, size[0], size[1]],
            panel: [0.0, size[1]],
        }
    }

    /// A split cell `size` pixels with the HUD size setting `hud_k` (1.0
    /// Full): the box is `min(h, 0.75 w) * hud_k` high; the projection is
    /// the cell, or for a cell narrower than 4:3 the 4:3 band its width,
    /// centred up and down in whole pixels by `game::split_layout::ui_rect`'s
    /// own integer rule (`w * 3 / 4` and `(h - band) / 2`, both floored), so
    /// the reticle, the markers and the panel sit on the band the 3D
    /// line-up and `fit_fov_y` use. The panel is the projection's height
    /// whatever `hud_k` is.
    pub fn cell(size: [f32; 2], hud_k: f32) -> Self {
        let [w, h] = size;
        let proj = if w * 3.0 >= h * 4.0 {
            [0.0, 0.0, w, h]
        } else {
            let (wi, hi) = (w.max(0.0) as u64, h.max(0.0) as u64);
            let band = (wi * 3 / 4).max(1);
            [0.0, (hi.saturating_sub(band) / 2) as f32, w, band as f32]
        };
        Frame {
            size,
            box_h: h.min(0.75 * w) * hud_k,
            proj,
            panel: [proj[1], proj[3]],
        }
    }

    /// The same frame projecting over `proj` (x, y, w, h in the screen).
    pub fn with_proj(self, proj: [f32; 4]) -> Self {
        Frame { proj, ..self }
    }

    pub fn box_w(&self) -> f32 {
        4.0 / 3.0 * self.box_h
    }

    /// Screen pixels per frame pixel.
    pub fn scale(&self) -> [f32; 2] {
        let h = self.box_h;
        [
            OVERFILL[0].1 / FRAME_W * (4.0 / 3.0) * h,
            OVERFILL[1].1 / FRAME_H * h,
        ]
    }

    /// The width and height the screen has beyond the box.
    pub fn extra(&self) -> [f32; 2] {
        [
            self.size[0] - 4.0 / 3.0 * self.box_h,
            self.size[1] - self.box_h,
        ]
    }

    /// [`anchor_shift`] on this frame's spare width.
    pub fn shift_x(&self, fx: f32) -> f32 {
        let extra = self.extra()[0];
        if fx < 1.0 / 3.0 {
            0.0
        } else if fx > 2.0 / 3.0 {
            extra
        } else {
            extra * 0.5
        }
    }

    /// How far down an item anchored `v` moves.
    pub fn shift_y(&self, v: VAnchor) -> f32 {
        let extra = self.extra()[1];
        if extra == 0.0 {
            0.0
        } else {
            extra * v.share()
        }
    }

    /// The screen pixel of a projected point `p` (0..1 of the projection).
    pub fn project(&self, p: [f32; 2]) -> [f32; 2] {
        let [x, y, w, h] = self.proj;
        [plus(p[0] * w, x), plus(p[1] * h, y)]
    }

    /// The screen pixel of an off-screen marker clamped to the projection's edge box
    /// (`ndc`, -1..1 of the projection, scaled toward its target until its larger coordinate
    /// is the clamp edge): the same direction from the centre and the same edge share, but of
    /// the whole screen (the cell), not the projection. A split cell narrower than 4:3 projects
    /// through its 4:3 band, whose edge box would stop the markers well short of the cell's top
    /// and bottom edges, where the rest of its HUD sits. Where the projection is the screen
    /// (the single view, a wide cell) it is [`Frame::project`].
    pub fn project_edge(&self, ndc: [f32; 2]) -> [f32; 2] {
        let [x, y, w, h] = self.proj;
        let [sw, sh] = self.size;
        if x == 0.0 && y == 0.0 && w == sw && h == sh {
            return self.project([ndc[0] * 0.5 + 0.5, ndc[1] * 0.5 + 0.5]);
        }
        let edge = ndc[0].abs().max(ndc[1].abs());
        // In screen-sized shares: the direction kept, then clamped to the screen's own box.
        let d = [ndc[0] * w / sw.max(1.0), ndc[1] * h / sh.max(1.0)];
        let bound = d[0].abs().max(d[1].abs());
        let k = if bound > 0.0 { edge / bound } else { 0.0 };
        [(d[0] * k * 0.5 + 0.5) * sw, (d[1] * k * 0.5 + 0.5) * sh]
    }

    /// A HUD root's frame → screen mapping for an element at frame fraction
    /// `fx` across, anchored `v` up and down.
    pub(crate) fn tree(&self, fx: f32, v: VAnchor) -> Screen {
        Screen {
            origin: [
                OVERFILL[0].0 * self.box_w() + self.shift_x(fx),
                plus(OVERFILL[1].0 * self.box_h, self.shift_y(v)),
            ],
            scale: self.scale(),
        }
    }

    /// [`Frame::tree`] in the minimap's and the spawn legend's own
    /// arithmetic (`crate::hud_map`), which they keep bit for bit.
    pub(crate) fn tree_map(&self, fx: f32, v: VAnchor) -> Screen {
        Screen {
            origin: [
                OVERFILL[0].0 * 4.0 / 3.0 * self.box_h + self.shift_x(fx),
                plus(OVERFILL[1].0 * self.box_h, self.shift_y(v)),
            ],
            scale: self.scale(),
        }
    }

    /// The panel screen (spawn pages, stats pages): the screen on the
    /// `panel` band, centred in this one (the projection's band), its footers
    /// at the real bottom. It does not shrink with the HUD
    /// size setting, as the class page's 3D line-up and `class_at` do not.
    pub fn panel(&self) -> Panel {
        let [dy, h] = self.panel;
        Panel {
            size: [self.size[0], h],
            dy,
            bottom: self.size[1],
        }
    }
}

/// A centred panel's screen ([`Frame::panel`]): laid out for `size`, moved
/// down `dy`, its help lines at `bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub size: [f32; 2],
    pub dy: f32,
    pub bottom: f32,
}

impl Panel {
    /// The shell's screen under anchor `a` (`crate::spawn_draw::screen_at`), moved down.
    pub(crate) fn screen(&self, a: crate::spawn_draw::Anchor) -> Screen {
        let mut s = crate::spawn_draw::screen_at(self.size, a);
        s.origin[1] = plus(s.origin[1], self.dy);
        s
    }

    /// The HUD tree's own mapping in the panel's screen for an element at `fx`.
    pub(crate) fn tree(&self, fx: f32) -> Screen {
        let mut s = Frame::window(self.size).tree_map(fx, VAnchor::Top);
        s.origin[1] = plus(s.origin[1], self.dy);
        s
    }
}

/// `[ours]` A name tag's line is never drawn shorter than this many screen pixels, though the
/// event feed beside it may be (issue #194 B22: four quarter cells of a 1280x720 window at the
/// smallest HUD Size drew 5-pixel tags). measure: split capture b22_tags_four_hud_small,
/// 1280x720, four views, smallest HUD Size; the 11 px floor awaits the maintainer's playtest.
pub const NAME_TAG_MIN_PX: f32 = 11.0;

/// The element index a name tag's glyph boxes carry in [`HudFrame::boxes`] (no tree element
/// draws them; issue #194 B22).
pub const NAME_TAG_BOX: usize = usize::MAX - 1;

pub(crate) fn color(rgb: [f32; 3], alpha: f32) -> [f32; 4] {
    [
        (rgb[0] / 255.0).clamp(0.0, 1.0),
        (rgb[1] / 255.0).clamp(0.0, 1.0),
        (rgb[2] / 255.0).clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    ]
}

pub(crate) struct Ctx<'a> {
    pub(crate) hud: &'a Hud,
    pub(crate) art: &'a HudArt,
    pub(crate) out: HudFrame,
    /// Window pixels outside this box are cut away (a scrolling list's
    /// window); applies to axis-aligned quads.
    pub(crate) clip: Option<[f32; 4]>,
}

/// A textured quad in element-local frame pixels.
pub(crate) struct Quad<'a> {
    /// x0, y0, x1, y1.
    pub(crate) rect: [f32; 4],
    /// u0, v0, u1, v1.
    pub(crate) uv: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) texture: &'a str,
    pub(crate) additive: bool,
    /// The BitmapStyle Shadow copy first (`re-hud.md` §2.2 [V]): (+1, +1)
    /// frame pixels, black, half the alpha.
    pub(crate) shadow: bool,
    /// Counted in the element's window box (a text's shadow run is not).
    pub(crate) boxed: bool,
}

/// A run of glyphs on one baseline, in element-local frame pixels.
pub(crate) struct Run<'a> {
    pub(crate) text: &'a str,
    /// Where the alignment hangs the run: its left edge, centre or right
    /// edge by `h_align`, and the line's top.
    pub(crate) at: (f32, f32),
    pub(crate) h_align: HAlign,
    pub(crate) scale: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) additive: bool,
    /// TextStyle Shadow: the whole run first at (+1, +1), black, half the
    /// alpha — [H] the bitmap style's numbers (`re-hud.md` §2.2) — and every
    /// glyph [`SHADOW_SPACING`] further on.
    pub(crate) shadow: bool,
    /// Shell units added after every glyph (a text role's tracking).
    pub(crate) tracking: f32,
}

/// The countdown's colour, the same on PS2 (B2 `FUN_00384078`) and PC
/// (0x599c52, BGRA bytes f2 a7 43): (67, 167, 242), whatever the flag's team.
const FLAG_TIMER_RGB: [f32; 3] = [67.0, 167.0, 242.0];
const HINT_SCALE_FAR: f32 = 0.25;
const HINT_SCALE_NEAR: f32 = 0.7;
const LOCK_RAMP_NEAR: f32 = 200.0;
const LOCK_RAMP_FAR: f32 = 1700.0;
const LOCK_SCALE_NEAR: f32 = 1.5;
const LOCK_SCALE_FAR: f32 = 0.1;
const LOCK_CORNER_MIN: f32 = 0.9;
const LOCK_CORNER_MAX: f32 = 1.4;
const LOCK_CORNER_FIXED: f32 = 1.2;

type Corner = (f32, f32, f32, f32);

/// A window-space quad (corners x0y0, x1y0, x1y1, x0y1 with their uvs) cut
/// to `clip`: `None` when it is not axis-aligned (left whole), `Some(None)`
/// when nothing of it is inside.
fn clip_quad(q: [Corner; 4], [cx0, cy0, cx1, cy1]: [f32; 4]) -> Option<Option<[Corner; 4]>> {
    let eq = |a: f32, b: f32| (a - b).abs() < 1.0e-3;
    if !(eq(q[0].1, q[1].1) && eq(q[2].1, q[3].1) && eq(q[0].0, q[3].0) && eq(q[1].0, q[2].0)) {
        return None;
    }
    let (x0, x1, y0, y1) = (q[0].0, q[1].0, q[0].1, q[3].1);
    let (u0, u1, v0, v1) = (q[0].2, q[1].2, q[0].3, q[3].3);
    let cut = |a: f32, b: f32, lo: f32, hi: f32| {
        let (mn, mx) = (a.min(b).max(lo), a.max(b).min(hi));
        (mn < mx).then(|| (a.clamp(mn, mx), b.clamp(mn, mx)))
    };
    let lerp = |a: f32, b: f32, ta: f32, tb: f32, t: f32| {
        if (b - a).abs() < 1.0e-6 {
            ta
        } else {
            ta + (t - a) / (b - a) * (tb - ta)
        }
    };
    let Some((nx0, nx1)) = cut(x0, x1, cx0, cx1) else {
        return Some(None);
    };
    let Some((ny0, ny1)) = cut(y0, y1, cy0, cy1) else {
        return Some(None);
    };
    let (nu0, nu1) = (lerp(x0, x1, u0, u1, nx0), lerp(x0, x1, u0, u1, nx1));
    let (nv0, nv1) = (lerp(y0, y1, v0, v1, ny0), lerp(y0, y1, v0, v1, ny1));
    Some(Some([
        (nx0, ny0, nu0, nv0),
        (nx1, ny0, nu1, nv0),
        (nx1, ny1, nu1, nv1),
        (nx0, ny1, nu0, nv1),
    ]))
}

impl<'a> Ctx<'a> {
    pub(crate) fn quad(&mut self, elem: usize, m: &Mat4, s: &Screen, q: &Quad<'_>) {
        let [x0, y0, x1, y1] = q.rect;
        let [u0, v0, u1, v1] = q.uv;
        let corners = [
            (x0, y0, u0, v0),
            (x1, y0, u1, v0),
            (x1, y1, u1, v1),
            (x0, y1, u0, v1),
        ];
        let passes: &[(f32, [f32; 4])] = if q.shadow {
            &[(1.0, [0.0, 0.0, 0.0, q.color[3] * 0.5]), (0.0, q.color)]
        } else {
            &[(0.0, q.color)]
        };
        for (off, col) in passes {
            let mut placed = corners.map(|(x, y, u, v)| {
                let p = m.transform_point3(Vec3::new(x, y, 0.0)) + Vec3::new(*off, *off, 0.0);
                let w = s.map(p);
                (w[0], w[1], u, v)
            });
            if let Some(clip) = self.clip {
                match clip_quad(placed, clip) {
                    Some(Some(cut)) => placed = cut,
                    Some(None) => continue,
                    None => {}
                }
            }
            let first = self.out.indices.len() as u32;
            let base = self.out.verts.len() as u32;
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for (wx, wy, u, v) in placed {
                let w = [wx, wy];
                b = [
                    b[0].min(w[0]),
                    b[1].min(w[1]),
                    b[2].max(w[0]),
                    b[3].max(w[1]),
                ];
                self.out.verts.push(HudVertex {
                    pos: [w[0], w[1], 0.0],
                    uv: [u, v],
                    color: *col,
                });
            }
            self.out
                .indices
                .extend([0, 1, 2, 0, 2, 3].map(|k| base + k));
            self.out.batch(q.texture, q.additive, false, first);
            if *off == 0.0 && q.boxed {
                self.out.boxes.push((elem, b));
            }
        }
    }

    fn hint_markers(&mut self, f: &Frame) {
        let scale = f.scale();
        let hud = self.hud;
        for marker in &hud.hint_markers {
            let styles = if marker.carrier {
                &hud.carrier_styles
            } else if marker.objective {
                &hud.objective_styles
            } else if marker.flag {
                &hud.flag_styles
            } else {
                &hud.hint_styles
            };
            let slot = usize::from(!marker.on_screen) * 2 + usize::from(!marker.visible);
            let Some(style) = styles[slot] else {
                continue;
            };
            let e = &hud.elems[style.elem];
            let Some(texture) = e.texture.as_deref().map(|t| self.art.icons.layout_sheet(t)) else {
                continue;
            };
            let tint = marker.tint.unwrap_or_else(|| {
                style
                    .friendly
                    .filter(|_| marker.friendly)
                    .unwrap_or(e.color)
            });
            let t = ((marker.distance - LOCK_RAMP_NEAR) / (LOCK_RAMP_FAR - LOCK_RAMP_NEAR))
                .clamp(0.0, 1.0);
            let mut s = if marker.carrier && marker.flyer_view && marker.on_screen {
                let scale = t * 0.5 + (1.0 - t) * 0.8;
                scale * if marker.own_carrier { 1.6 } else { 1.0 }
            } else if marker.flag || marker.carrier || marker.objective || !marker.on_screen {
                t * style.max_scale + (1.0 - t) * style.min_scale
            } else {
                t * HINT_SCALE_FAR + (1.0 - t) * HINT_SCALE_NEAR
            };
            if marker.on_screen {
                if let Some(pulse) = marker.pulse {
                    s = s * (1.0 - pulse) + 0.9;
                }
            }
            let at = if marker.on_screen {
                f.project([marker.ndc[0] * 0.5 + 0.5, marker.ndc[1] * 0.5 + 0.5])
            } else {
                f.project_edge(marker.ndc)
            };
            let screen = Screen { origin: at, scale };
            let (x0, y0) = e.rect.origin();
            let quad = Quad {
                rect: [x0, y0, x0 + e.rect.w, y0 + e.rect.h],
                uv: e.tex_coords,
                color: color(tint, e.alpha),
                texture,
                additive: e.additive,
                shadow: e.shadow,
                boxed: false,
            };
            if !marker.on_screen {
                // An off-screen objective points at its target: the icon turns
                // to the clamped edge direction (`re-hud-map.md` §5).
                let turn = marker.ndc[0].atan2(-marker.ndc[1]);
                let m = Mat4::from_rotation_z(turn) * Mat4::from_scale(Vec3::splat(s));
                self.quad(style.elem, &m, &screen, &quad);
                continue;
            }
            self.quad(
                style.elem,
                &Mat4::from_scale(Vec3::splat(s)),
                &screen,
                &quad,
            );
        }
    }

    fn flag_timers(&mut self, f: &Frame) {
        let hud = self.hud;
        if hud.flag_timers.is_empty() {
            return;
        }
        let Some(font) = self.face(crate::typography::Face::Tiny) else {
            return;
        };
        let scale = f.scale();
        let height = f32::from(font.line_height) * scale[1];
        let screen = Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        };
        for t in &hud.flag_timers {
            let text = t.seconds.to_string();
            let [x, y] = f.project([t.ndc[0] * 0.5 + 0.5, t.ndc[1] * 0.5 + 0.5]);
            self.run(
                usize::MAX,
                &Mat4::IDENTITY,
                &screen,
                font,
                &Run {
                    text: &text,
                    at: (x, y - height),
                    h_align: HAlign::Center,
                    scale,
                    color: color(FLAG_TIMER_RGB, t.alpha),
                    additive: false,
                    shadow: false,
                    tracking: 0.0,
                },
            );
        }
    }

    fn lock_marker(&mut self, f: &Frame) {
        let scale = f.scale();
        let hud = self.hud;
        let Some(marker) = hud.lock_marker else {
            return;
        };
        let slot = usize::from(!marker.on_screen) * 2 + usize::from(!marker.visible);
        let Some(style) = hud.lock_styles[slot] else {
            return;
        };
        let e = &hud.elems[style.elem];
        let Some(texture) = e.texture.as_deref().map(|t| self.art.icons.layout_sheet(t)) else {
            return;
        };
        let at = if marker.on_screen {
            f.project([marker.ndc[0] * 0.5 + 0.5, marker.ndc[1] * 0.5 + 0.5])
        } else {
            f.project_edge(marker.ndc)
        };
        let screen = Screen { origin: at, scale };
        let (x0, y0) = e.rect.origin();
        let quad = Quad {
            rect: [x0, y0, x0 + e.rect.w, y0 + e.rect.h],
            uv: e.tex_coords,
            color: color(e.color, e.alpha),
            texture,
            additive: e.additive,
            shadow: e.shadow,
            boxed: false,
        };
        if !marker.on_screen {
            let turn = marker.ndc[0].atan2(-marker.ndc[1]);
            let t = ((marker.distance - LOCK_RAMP_NEAR) / (LOCK_RAMP_FAR - LOCK_RAMP_NEAR))
                .clamp(0.0, 1.0);
            let s = t * style.min_scale + (1.0 - t) * style.max_scale;
            let m = Mat4::from_rotation_z(turn) * Mat4::from_scale(Vec3::splat(s));
            self.quad(style.elem, &m, &screen, &quad);
            return;
        }
        let k = (marker.pulse.fract() - 0.5).abs() * 2.0;
        let flash = if marker.visible { 255.0 } else { 64.0 };
        let mix = e.color.map(|c| c * k + flash * (1.0 - k));
        let quad = Quad {
            color: color(mix, e.alpha),
            ..quad
        };
        let (spread, size_scale) = if marker.flyer {
            let t = ((marker.distance - LOCK_RAMP_NEAR) / (LOCK_RAMP_FAR - LOCK_RAMP_NEAR))
                .clamp(0.0, 1.0);
            let spread = t * LOCK_SCALE_FAR + (1.0 - t) * LOCK_SCALE_NEAR;
            let corner = if spread < LOCK_CORNER_MIN {
                LOCK_CORNER_MIN
            } else {
                spread.min(LOCK_CORNER_MAX)
            };
            (spread, corner)
        } else {
            (style.min_scale, LOCK_CORNER_FIXED)
        };
        let (w, h) = (e.rect.w * spread, e.rect.h * spread);
        let corners = [(-w, -h), (w, -h), (w, h), (-w, h)];
        for (k, (cx, cy)) in corners.into_iter().enumerate() {
            let m = Mat4::from_translation(Vec3::new(cx, cy, 0.0))
                * Mat4::from_rotation_z(k as f32 * std::f32::consts::FRAC_PI_2)
                * Mat4::from_scale(Vec3::splat(size_scale));
            self.quad(style.elem, &m, &screen, &quad);
        }
    }

    fn element(&mut self, i: usize, parent: &Mat4, s: &Screen) {
        let hud = self.hud;
        let e = &hud.elems[i];
        if e.fader.state == 0 || e.is_segment {
            return;
        }
        let alpha = hud.alpha(i);
        if alpha <= 0.0 {
            return;
        }
        let m = *parent * e.local();
        match e.kind {
            Kind::Group => {
                // A group's Rect hangs its children on the origin like a
                // bitmap's own ([D] `re-event-messages.md` §5: the PS2 frame
                // has `player1stats` — Rect(1, 1, Left, Center) — half a
                // viewport above where its Position alone would put it).
                let (ox, oy) = e.rect.origin();
                let m = m * Mat4::from_translation(Vec3::new(ox, oy, 0.0));
                // 255 first (the back), 0 last; equal z: the later record
                // first (`re-hud.md` §2.1 [V] `FUN_005a3368`).
                let mut order: Vec<usize> = e.children.clone();
                order.reverse();
                order.sort_by_key(|c| std::cmp::Reverse(hud.elems[*c].z_order));
                for c in order {
                    self.element(c, &m, s);
                }
            }
            Kind::Bitmap | Kind::BarBitmap => self.bitmap(i, e, &m, s, alpha),
            Kind::BarSegmented => self.segmented(i, e, &m, s, alpha),
            Kind::Text => self.elem_text(i, e, &m, s, alpha),
            Kind::MultiLineText => self.multiline(i, e, &m, s, alpha),
            Kind::Model3D => self.model(i, e, parent, s, alpha),
            Kind::Map => self.map(i, e, parent, s, alpha),
            Kind::Other => {}
        }
    }

    fn bitmap(&mut self, i: usize, e: &Elem, m: &Mat4, s: &Screen, alpha: f32) {
        let Some(texture) = e.texture.as_deref().map(|t| self.art.icons.layout_sheet(t)) else {
            return;
        };
        let (x0, y0) = e.rect.origin();
        let [u0, v0, u1, v1] = e.tex_coords;
        // The value shortens the rect from its left edge, and the u range
        // with it unless ScaleTexture is 0 (`re-hud.md` §2.2 [V]).
        let cut = |v: f32| {
            let u = if e.scale_texture {
                u0 + (u1 - u0) * v
            } else {
                u1
            };
            (x0 + e.rect.w * v, u)
        };
        let value = if e.kind == Kind::BarBitmap {
            e.value
        } else {
            1.0
        };
        let (x1, u_end) = cut(value);
        let rgb = self.hud.drawn(i);
        if x1 > x0 && e.rect.h > 0.0 {
            self.quad(
                i,
                m,
                s,
                &Quad {
                    rect: [x0, y0, x1, y0 + e.rect.h],
                    uv: [u0, v0, u_end, v1],
                    color: color(rgb, alpha),
                    texture,
                    additive: e.additive,
                    shadow: e.shadow,
                    boxed: true,
                },
            );
        }
        // The flash copy (`re-target-hud.md` §5 [V][O]): the bar's own quad
        // over the span between the two values — its u range too — fading
        // out; a fall's top edge sinks to FlashyScale above its bottom:
        // top = bottom − (h·(1 − f) + FlashyScale·f).
        if let Some(fl) = e.flash.filter(|_| e.kind == Kind::BarBitmap) {
            let f = fl.progress();
            let (a, b) = (fl.from.min(fl.to), fl.from.max(fl.to));
            let ((xa, ua), (xb, ub)) = (cut(a), cut(b));
            let bottom = y0 + e.rect.h;
            let top = if fl.fall() {
                bottom - (e.rect.h * (1.0 - f) + e.flashy_scale * f)
            } else {
                y0
            };
            let v_top = v1 - (v1 - v0) * (bottom - top) / e.rect.h.max(1.0e-6);
            if xb > xa && bottom > top {
                self.quad(
                    i,
                    m,
                    s,
                    &Quad {
                        rect: [xa, top, xb, bottom],
                        uv: [ua, v_top, ub, v1],
                        color: color(rgb, alpha * (1.0 - f)),
                        texture,
                        additive: e.additive,
                        shadow: false,
                        boxed: false,
                    },
                );
            }
        }
    }

    /// `FUN_003a04b8` (B2:397669–397801, read for this card, [D]): segment k
    /// of N sits at angle start + k·|end − start|/N while that is below
    /// start + |end − start|·value; at x = w/2·(1 + sin θ), y = h/2·(1 − cos θ)
    /// of the Rect — 0° is the top, clockwise — turned by θ. A bar that is
    /// not Circular steps (int)w / N along x instead.
    fn segmented(&mut self, i: usize, e: &Elem, m: &Mat4, s: &Screen, alpha: f32) {
        let Some(seg) = e.segment.map(|k| &self.hud.elems[k]) else {
            return;
        };
        let Some(texture) = seg.texture.as_deref() else {
            return;
        };
        let n = e.segments;
        if n == 0 {
            return;
        }
        // A turned bar turns about its rect's corner: `refire_rightbit`, the one
        // the layout turns, is the left bit mirrored by a Position of the rect's
        // width, and on the PS2 the two meet at 6 o'clock (`re-heat-hud.md` 1, 6).
        let (ox, oy) = e.rect.origin();
        let turn = trs(Vec3::ZERO, e.rot, Vec3::ONE);
        let m = &(*m * turn.inverse() * Mat4::from_translation(Vec3::new(ox, oy, 0.0)) * turn);
        let (sx, sy) = seg.rect.origin();
        let own = self.hud.drawn(i);
        let col = color(
            [0, 1, 2].map(|c| own[c] * seg.drawn[c] / 255.0),
            alpha * seg.alpha,
        );
        let start = e.angle_start.to_radians();
        let span = (e.angle_end - e.angle_start).to_radians().abs();
        for k in 0..n {
            let f = k as f32 / n as f32;
            if f >= e.value - 1.0e-4 {
                break;
            }
            let at = if e.circular {
                let th = start + span * f;
                let (sn, cs) = th.sin_cos();
                Mat4::from_translation(Vec3::new(
                    e.rect.w * 0.5 * (1.0 + sn),
                    e.rect.h * 0.5 * (1.0 - cs),
                    0.0,
                )) * Mat4::from_rotation_z(th)
            } else {
                Mat4::from_translation(Vec3::new(
                    (e.rect.w.trunc() / n as f32) * k as f32,
                    0.0,
                    0.0,
                ))
            };
            let sm = *m * at * seg.local();
            self.quad(
                i,
                &sm,
                s,
                &Quad {
                    rect: [sx, sy, sx + seg.rect.w, sy + seg.rect.h],
                    uv: seg.tex_coords,
                    color: col,
                    texture,
                    additive: e.additive,
                    shadow: seg.shadow,
                    boxed: true,
                },
            );
        }
    }

    /// The round-end banner as text, for a BF2-only build with no BF1
    /// `ig_victory_defeat` sheet: VICTORY or DEFEAT in BF2's large face,
    /// centred, in the sheet's own tint ([`crate::hud::round_end_color`]).
    /// `[ours]` the 1.6x size and 2px tracking stand in for the sheet's
    /// 256x64 banner. replace: BF2's own round-end presentation
    /// (`analysis/re-round-end.md`).
    fn round_end_text(&mut self, result: u8, f: &Frame) {
        const SIZE: f32 = 1.6;
        let Some(font) = self.face(crate::typography::Face::Large) else {
            return;
        };
        let size = f.size;
        let scale = f.scale();
        let height = f32::from(font.line_height) * SIZE * scale[1];
        let screen = Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        };
        self.run(
            usize::MAX,
            &Mat4::IDENTITY,
            &screen,
            font,
            &Run {
                text: if result == 1 { "DEFEAT" } else { "VICTORY" },
                at: (size[0] * 0.5, (size[1] - height) * 0.5),
                h_align: HAlign::Center,
                scale: [scale[0] * SIZE, scale[1] * SIZE],
                color: crate::hud::round_end_color(result),
                additive: false,
                shadow: true,
                tracking: 2.0 * scale[0],
            },
        );
    }

    /// The other players' names over their heads (LAN §5b, issue #194 B22): in the event feed's
    /// own font and text scale (its message box's, `gamefont_tiny` without one) at this frame's
    /// scale (a split cell's includes the HUD Size setting), so a tag reads as the kill-feed line
    /// beside it. Placed in the screen's own pixels (never through `f.project`: a tall cell's
    /// 4:3 band is not the projection the tags were placed with), centred, bottom on the anchor,
    /// shadowed, in the reticle's friend or enemy colour faded by distance.
    fn name_tags(&mut self, f: &Frame) {
        let hud = self.hud;
        if hud.name_tags.is_empty() {
            return;
        }
        let (font, text_scale) = match hud.message_box(crate::hud::MESSAGE_EVENT) {
            Some(i) => {
                // The box's own text scale under its and its groups' Scale records, as its
                // lines draw (`Ctx::element` carries each parent's matrix down).
                let mut k = hud.elems[i].text_scale;
                let mut at = Some(i);
                while let Some(j) = at {
                    let e = &hud.elems[j];
                    k = [k[0] * e.scale.x.abs(), k[1] * e.scale.y.abs()];
                    at = e.parent;
                }
                (hud.elems[i].font.as_str(), k)
            }
            None => (crate::hud_tree::DEFAULT_FONT, [1.0, 1.0]),
        };
        let Some(font) = self.art.fonts.get(font) else {
            return;
        };
        let k = f.scale();
        let mut scale = [text_scale[0] * k[0], text_scale[1] * k[1]];
        let mut height = f32::from(font.line_height) * scale[1];
        // Never below a readable size over the world (a quarter cell at the smallest HUD Size).
        if height > 0.0 && height < NAME_TAG_MIN_PX {
            let up = NAME_TAG_MIN_PX / height;
            scale = scale.map(|v| v * up);
            height = NAME_TAG_MIN_PX;
        }
        let screen = Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        };
        let [enemy, _, friend] = crate::hud::RETICLE_COLORS;
        for t in &hud.name_tags {
            let rgb = if t.friend { friend } else { enemy };
            self.run(
                NAME_TAG_BOX,
                &Mat4::IDENTITY,
                &screen,
                font,
                &Run {
                    text: &t.name,
                    at: (t.at[0], t.at[1] - height),
                    h_align: HAlign::Center,
                    scale,
                    color: color(rgb.map(f32::from), t.alpha),
                    additive: false,
                    shadow: true,
                    tracking: 0.0,
                },
            );
        }
    }

    /// The menu face's metric set for `face` (`crate::typography`).
    pub(crate) fn face(&self, face: crate::typography::Face) -> Option<&'a HudFont> {
        self.art.menu_font(face.role())
    }

    pub(crate) fn run(&mut self, i: usize, m: &Mat4, s: &Screen, font: &HudFont, r: &Run<'_>) {
        let glyphs: Vec<&HudGlyph> = r
            .text
            .chars()
            .filter_map(|c| font.glyphs.iter().find(|g| u32::from(g.code) == c as u32))
            .collect();
        let [kx, ky] = r.scale;
        let spacing = if r.shadow { SHADOW_SPACING } else { 0.0 };
        let width = run_width(font, r.text, kx, r.shadow)
            + r.tracking * r.text.chars().filter(|c| has_glyph(font, *c)).count() as f32;
        let left = r.at.0
            + match r.h_align {
                HAlign::Left => 0.0,
                HAlign::Center => -width * 0.5,
                HAlign::Right => -width,
            };
        let top = r.at.1;
        let passes: &[(f32, [f32; 4], bool)] = if r.shadow {
            &[
                (1.0, [0.0, 0.0, 0.0, r.color[3] * 0.5], false),
                (0.0, r.color, true),
            ]
        } else {
            &[(0.0, r.color, true)]
        };
        for (off, col, boxed) in passes {
            let mut pen = left + off;
            for g in &glyphs {
                let page = font.pages.get(usize::from(g.page));
                if let (Some(page), true) = (page, g.x1 > g.x0 && g.y1 > g.y0) {
                    let bitmap_rect = [
                        f32::from(g.x0),
                        f32::from(g.y0),
                        f32::from(g.x1),
                        f32::from(g.y1),
                    ];
                    let distance = self.art.distance_fonts.get(&font.name).and_then(|f| {
                        f.glyphs
                            .iter()
                            .find(|d| d.code == g.code)
                            .and_then(|d| f.pages.get(usize::from(d.page)).map(|p| (p, d)))
                    });
                    let (page, rect, uv) = match distance {
                        Some((p, d)) => (p, d.rect, d.uv),
                        None => (page, bitmap_rect, [g.uv[0], g.uv[2], g.uv[1], g.uv[3]]),
                    };
                    self.quad(
                        i,
                        m,
                        s,
                        &Quad {
                            rect: [
                                pen + rect[0] * kx,
                                top + off + rect[1] * ky,
                                pen + rect[2] * kx,
                                top + off + rect[3] * ky,
                            ],
                            uv,
                            color: *col,
                            texture: page,
                            additive: r.additive,
                            shadow: false,
                            boxed: false,
                        },
                    );
                    if *boxed {
                        // Distance padding is artwork, never a hit/layout box.
                        let [x0, y0, x1, y1] = bitmap_rect;
                        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                        for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                            let p = s.map(m.transform_point3(Vec3::new(
                                pen + x * kx,
                                top + off + y * ky,
                                0.0,
                            )));
                            bounds = [
                                bounds[0].min(p[0]),
                                bounds[1].min(p[1]),
                                bounds[2].max(p[0]),
                                bounds[3].max(p[1]),
                            ];
                        }
                        self.out.boxes.push((i, bounds));
                    }
                }
                pen += (f32::from(g.advance) + spacing) * kx + r.tracking;
            }
        }
    }

    /// A MultiLineText (`re-event-messages.md` §2): each shown line in its
    /// own colour and fade, on a line pitch that fits a whole number of the
    /// font's lines into the box ([D]: the PS2's box of 0.25 × 448 holds 7
    /// lines of `gamefont_tiny`, element +0x198 [M]), the text hung in its
    /// line by the Format's alignment.
    fn multiline(&mut self, i: usize, e: &Elem, m: &Mat4, s: &Screen, alpha: f32) {
        let (Some(ml), Some(font)) = (e.lines.as_deref(), self.art.fonts.get(&e.font)) else {
            return;
        };
        let height = f32::from(font.line_height) * e.text_scale[1];
        if height <= 0.0 {
            return;
        }
        let fit = (e.rect.h / height).floor().max(1.0);
        let pitch = if e.rect.h > 0.0 {
            e.rect.h / fit
        } else {
            height
        };
        let (ox, oy) = e.rect.origin();
        let x = ox
            + match e.h_align {
                HAlign::Left => 0.0,
                HAlign::Center => e.rect.w * 0.5,
                HAlign::Right => e.rect.w,
            };
        let hang = match e.v_align {
            VAlign::Top => 0.0,
            VAlign::Center => (pitch - height) * 0.5,
            VAlign::Bottom => pitch - height,
        };
        let tint = self.hud.tint(i);
        for line in &ml.shown {
            let a = alpha * line.fader.alpha();
            if a <= 0.0 {
                continue;
            }
            self.token_run(
                i,
                m,
                s,
                font,
                &Run {
                    text: &line.text,
                    at: (x, oy + line.y * pitch + hang),
                    h_align: e.h_align,
                    scale: e.text_scale,
                    color: color([0, 1, 2].map(|c| line.color[c] * tint[c]), a),
                    additive: e.additive,
                    shadow: ml.shadow,
                    tracking: 0.0,
                },
            );
        }
    }

    fn model(&mut self, i: usize, e: &Elem, parent: &Mat4, s: &Screen, alpha: f32) {
        let Some(name) = e.mesh.as_deref() else {
            return;
        };
        if let Some(sheet) = name.strip_prefix("bitmap:") {
            self.icon(i, e, parent, s, alpha, sheet);
            return;
        }
        // A MeshInfo for the shown mesh replaces the element's own transform.
        let local = match e.mesh_info.iter().find(|k| k.mesh == name) {
            Some(k) => trs(k.pos, k.rot, k.scale),
            None => e.local(),
        };
        let m = *parent * local * mesh_frame();
        self.mesh(i, name, &m, s, color(self.hud.drawn(i), alpha), e.additive);
    }

    /// The mesh `name` under `m` (mesh units → frame pixels: the element's
    /// matrix × [`mesh_frame`]), tinted `col`.
    pub(crate) fn mesh(
        &mut self,
        i: usize,
        name: &str,
        m: &Mat4,
        s: &Screen,
        col: [f32; 4],
        additive: bool,
    ) {
        let Some(mesh) = self.art.meshes.get(name) else {
            return;
        };
        let slice = (self.out.meshes as f32 % DEPTH_SLICES) / DEPTH_SLICES;
        self.out.meshes += 1;
        // Depth inside the slice: larger frame z is nearer.
        let (mut z0, mut z1) = (f32::MAX, f32::MIN);
        let moved: Vec<Vec<Vec3>> = mesh
            .parts
            .iter()
            .map(|p| {
                p.positions
                    .iter()
                    .map(|v| {
                        let w = m.transform_point3(Vec3::from(*v));
                        z0 = z0.min(w.z);
                        z1 = z1.max(w.z);
                        w
                    })
                    .collect()
            })
            .collect();
        let range = (z1 - z0).max(1.0e-6);
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for (part, pts) in mesh.parts.iter().zip(&moved) {
            let first = self.out.indices.len() as u32;
            let base = self.out.verts.len() as u32;
            let shift = part.scroll.map(|v| (v * self.hud.clock).fract());
            for (k, (p, uv)) in pts.iter().zip(&part.uvs).enumerate() {
                let vc = part.colors.get(k).copied().unwrap_or([1.0; 4]);
                let w = s.map(*p);
                b = [
                    b[0].min(w[0]),
                    b[1].min(w[1]),
                    b[2].max(w[0]),
                    b[3].max(w[1]),
                ];
                let near = (p.z - z0) / range;
                let depth = 1.0 - slice - near / DEPTH_SLICES;
                self.out.verts.push(HudVertex {
                    pos: [w[0], w[1], depth.clamp(0.0, 1.0)],
                    uv: [uv[0] + shift[0], uv[1] + shift[1]],
                    color: [0, 1, 2, 3].map(|c| col[c] * vc[c]),
                });
            }
            self.out
                .indices
                .extend(part.indices.iter().map(|k| base + k));
            self.out.batch(&part.texture, additive, true, first);
        }
        if b[0] <= b[2] {
            self.out.boxes.push((i, b));
        }
    }

    /// `[ours]` (PLAN §Presentation): a BF1 weapon with no BF2 mesh shows its
    /// BF1 `IconTexture` sheet where the weapon mesh would sit — centred on
    /// the spot the row's MeshInfos put a rifle (row 1) or the thermal
    /// detonator (row 2, the `weapon2` element), as tall as the backdrop
    /// pill.
    fn icon(&mut self, i: usize, e: &Elem, parent: &Mat4, s: &Screen, alpha: f32, sheet: &str) {
        const HEIGHT: f32 = 22.0;
        let centre: (f32, f32) = if e.name.to_ascii_lowercase().contains("weapon2") {
            (-0.278 * FRAME_W, 0.250 * FRAME_H)
        } else {
            (-0.215 * FRAME_W, 0.356 * FRAME_H)
        };
        let aspect = self.art.sheet_aspect.get(sheet).copied().unwrap_or(2.0);
        let (hw, hh) = (HEIGHT * aspect * 0.5, HEIGHT * 0.5);
        let m = *parent * Mat4::from_translation(Vec3::new(centre.0, centre.1, 0.0));
        self.quad(
            i,
            &m,
            s,
            &Quad {
                rect: [-hw, -hh, hw, hh],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: color(self.hud.tint(i).map(|c| c * 255.0), alpha),
                texture: sheet,
                additive: e.additive,
                shadow: false,
                boxed: true,
            },
        );
    }
}

fn has_glyph(font: &HudFont, c: char) -> bool {
    font.glyphs.iter().any(|g| u32::from(g.code) == c as u32)
}

/// How wide [`Ctx::run`] lays `text` out: the glyphs' advances (and the
/// Shadow style's spacing) times `kx`.
pub(crate) fn run_width(font: &HudFont, text: &str, kx: f32, shadow: bool) -> f32 {
    let spacing = if shadow { SHADOW_SPACING } else { 0.0 };
    text.chars()
        .filter_map(|c| font.glyphs.iter().find(|g| u32::from(g.code) == c as u32))
        .map(|g| f32::from(g.advance) + spacing)
        .sum::<f32>()
        * kx
}

/// Lay the HUD out for a window of `size` pixels.
/// The shell movie with the same options panel used in battle.
pub fn draw_shell_options(
    hud: &Hud,
    art: &HudArt,
    menus: &crate::shell_ps2::ShellMenus,
    movie: bool,
    model: &crate::menu::MenuModel,
    animation: &crate::pause_draw::Animation,
    size: [f32; 2],
) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    ctx.shell_page(menus, movie, size);
    crate::pause_draw::draw_stack(&mut ctx, model, animation, size);
    ctx.out
}

/// A split battle cell's lost-controller notice over the whole cell (`size`, its pixels), in
/// the menu face with its button glyph (`crate::shell_ps2::players::LostCell`).
pub fn draw_lost_cell(
    hud: &Hud,
    art: &HudArt,
    notice: &crate::shell_ps2::players::LostCell,
    size: [f32; 2],
) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    ctx.lost_cell(notice, size);
    ctx.out
}

/// The shell background and the profile picker painted with the BF2 atlas.
pub fn draw_shell_profiles(
    hud: &Hud,
    art: &HudArt,
    menus: &crate::shell_ps2::ShellMenus,
    movie: bool,
    profiles: &crate::shell_ps2::profiles::Picker,
    size: [f32; 2],
) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    ctx.shell_page(menus, movie, size);
    crate::shell_ps2::profiles::draw(&mut ctx, profiles, size);
    ctx.out
}

/// The layout's centre reticle group: the cross, its hit marks and the ammo,
/// refire and heat rings around it.
pub const RETICLE_GROUP: &str = "player1reticule";

pub fn draw(hud: &Hud, art: &HudArt, size: [f32; 2]) -> HudFrame {
    draw_in(hud, art, &Frame::window(size))
}

/// [`draw`] for the screen `f`: a split cell's ([`Frame::cell`]) or the
/// single window's ([`Frame::window`], the same as [`draw`]).
pub fn draw_in(hud: &Hud, art: &HudArt, f: &Frame) -> HudFrame {
    draw_tree(hud, art, f, None)
}

/// [`draw`] with the [`RETICLE_GROUP`] laid out apart, for a view that places
/// it in the world: (everything else, the reticle at the middle of a `size`
/// window). The second frame is empty while no tree is drawn.
pub fn draw_detached(hud: &Hud, art: &HudArt, size: [f32; 2]) -> (HudFrame, HudFrame) {
    let reticle = hud.find(RETICLE_GROUP);
    let frame = draw_tree(hud, art, &Frame::window(size), reticle);
    let tree = hud.shell.is_none()
        && hud.banner.is_none()
        && hud.stats.is_none()
        && hud.spawn.as_ref().is_none_or(|page| page.keeps_tree());
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    if let Some(r) = reticle.filter(|_| tree) {
        let screen = Screen {
            origin: [0.5 * size[0], 0.5 * size[1]],
            scale: frame_scale(size),
        };
        let at = hud.elems[r].pos;
        let parent = Mat4::from_translation(Vec3::new(-at.x, -at.y, 0.0));
        ctx.element(r, &parent, &screen);
    }
    (frame, ctx.out)
}

fn draw_tree(hud: &Hud, art: &HudArt, f: &Frame, skip: Option<usize>) -> HudFrame {
    let size = f.size;
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    if let Some((menus, movie)) = &hud.shell {
        ctx.shell_page(menus, *movie, size);
        if let Some(ia) = &hud.instant {
            ctx.instant_page(ia, menus.pulse.alpha(), size);
        }
        return ctx.out;
    }
    if let Some(result) = hud.banner {
        if art.no_bf1_victory_sheet {
            ctx.round_end_text(result, f);
        } else {
            ctx.out = crate::hud::draw_round_end(result, f);
        }
        if hud.banner_fade > 0.0 {
            ctx.quad(
                usize::MAX,
                &Mat4::IDENTITY,
                &Screen {
                    origin: [0.0, 0.0],
                    scale: [1.0, 1.0],
                },
                &Quad {
                    rect: [0.0, 0.0, size[0], size[1]],
                    uv: [0.0, 0.0, 1.0, 1.0],
                    color: [0.0, 0.0, 0.0, hud.banner_fade],
                    texture: "white",
                    additive: false,
                    shadow: false,
                    boxed: false,
                },
            );
        }
        return ctx.out;
    }
    if let Some(st) = &hud.stats {
        ctx.stats_page(st, f);
        return ctx.out;
    }
    // A spawn screen (card SSELB) dims the backdrop under everything; the
    // page says whether the tree still draws under it.
    let tree = match &hud.spawn {
        Some(page) => {
            ctx.spawn_veil(size);
            page.keeps_tree()
        }
        None => true,
    };
    if tree && hud.damage_flash > 0.0 {
        ctx.quad(
            usize::MAX,
            &Mat4::IDENTITY,
            &Screen {
                origin: [0.0, 0.0],
                scale: [1.0, 1.0],
            },
            &Quad {
                rect: [0.0, 0.0, size[0], size[1]],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [1.0, 0.0, 0.0, hud.damage_flash],
                texture: "white",
                additive: false,
                shadow: false,
                boxed: false,
            },
        );
    }
    // The sniper scope (`crate::hud_scope`): under the tree, so the target
    // readout and the reticle stay over it.
    if let Some(v) = hud.scope.as_ref().filter(|_| tree) {
        ctx.scope(v, f);
    }
    // The other players' names over the world, under every HUD panel; a scope covers the world,
    // so none over it (issue #194 B22).
    if tree && hud.scope.is_none() {
        ctx.name_tags(f);
    }
    let mut order: Vec<usize> = if tree { hud.roots.clone() } else { Vec::new() };
    order.retain(|r| Some(*r) != skip);
    order.reverse();
    order.sort_by_key(|c| std::cmp::Reverse(hud.elems[*c].z_order));
    for r in order {
        let e = &hud.elems[r];
        // The countdowns' children sit at the right edge of the 4:3 frame,
        // but their groups have a small negative X origin. Keep the whole
        // group with the minimap when widening the viewport.
        let anchor_x = if e.name.eq_ignore_ascii_case("VictoryTimerGroup")
            || e.name.eq_ignore_ascii_case("DefeatTimerGroup")
        {
            1.0
        } else {
            e.pos.x / FRAME_W
        };
        let (screen, parent) = match e.window_pos {
            // A projected world point (the reticle): the element's origin is
            // that window pixel.
            Some(p) => (
                Screen {
                    origin: f.project(p),
                    scale: f.scale(),
                },
                Mat4::from_translation(Vec3::new(-e.pos.x, -e.pos.y, 0.0)),
            ),
            None => (f.tree(anchor_x, hud.v_anchor(r)), Mat4::IDENTITY),
        };
        ctx.element(r, &parent, &screen);
        if let Some(status) = hud
            .space
            .as_ref()
            .filter(|_| tree && e.name == crate::hud_space::GROUP)
        {
            ctx.space_status(status, e, r, &screen);
        }
    }
    if tree {
        ctx.hint_markers(f);
        ctx.flag_timers(f);
        ctx.lock_marker(f);
    }
    // The capture icon: a root group of the PS2's own (card CPBAR).
    if let Some(icon) = hud.capture.as_ref().filter(|_| tree) {
        ctx.capture(icon, f);
    }
    // The hit-direction chevrons: one of the PS2's code-drawn pieces (card
    // HATK).
    if tree {
        ctx.ambush(&hud.ambush, f);
    }
    // BF1's squad order cross: over everything, like the PS2's own group.
    if let Some(sq) = hud.squad.as_ref().filter(|_| tree) {
        ctx.squad(sq, f);
    }
    if let Some(page) = &hud.spawn {
        ctx.spawn_page(page, &f.panel());
    }
    ctx.out
}

#[cfg(test)]
mod tests {
    #[test]
    fn distance_padding_keeps_text_bounds_alignment_and_shadow_spacing() {
        use manifest::hud::{HudDistanceFont, HudDistanceGlyph, HudGlyph, HudLayout};
        let layout = HudLayout {
            name: "empty".into(),
            nodes: Vec::new(),
        };
        let hud = Hud::new(&layout, &layout);
        let font = HudFont {
            name: "fixture".into(),
            line_height: 12,
            pages: vec!["bitmap".into()],
            glyphs: vec![HudGlyph {
                code: 65,
                page: 0,
                advance: 7,
                x0: 1,
                x1: 6,
                y0: 2,
                y1: 10,
                uv: [0.0, 1.0, 0.0, 1.0],
            }],
        };
        let bitmap = HudArt::default();
        let mut scalable = bitmap.clone();
        scalable.distance_fonts.insert(
            font.name.clone(),
            HudDistanceFont {
                pages: vec!["distance".into()],
                glyphs: vec![HudDistanceGlyph {
                    code: 65,
                    page: 0,
                    rect: [-3.0, -2.0, 10.0, 14.0],
                    uv: [0.1, 0.2, 0.8, 0.9],
                }],
            },
        );
        for align in [HAlign::Left, HAlign::Center, HAlign::Right] {
            for shadow in [false, true] {
                let run = Run {
                    text: "AAA",
                    at: (50.0, 20.0),
                    h_align: align,
                    scale: [1.2, 1.1],
                    color: [0.5, 0.6, 0.7, 0.8],
                    additive: false,
                    shadow,
                    tracking: 0.0,
                };
                let draw = |art| {
                    let mut ctx = Ctx {
                        hud: &hud,
                        art,
                        out: HudFrame::default(),
                        clip: None,
                    };
                    ctx.run(
                        0,
                        &Mat4::IDENTITY,
                        &Screen {
                            origin: [0.0, 0.0],
                            scale: [4.0, 4.0],
                        },
                        &font,
                        &run,
                    );
                    ctx.out
                };
                let a = draw(&bitmap);
                let b = draw(&scalable);
                assert_eq!(a.boxes, b.boxes);
                assert_eq!(a.indices, b.indices);
                assert!(b.batches.iter().all(|b| b.texture == "distance"));
                assert_eq!(
                    a.verts.iter().map(|v| v.color).collect::<Vec<_>>(),
                    b.verts.iter().map(|v| v.color).collect::<Vec<_>>()
                );
                assert_eq!(
                    a.verts[4].pos[0] - a.verts[0].pos[0],
                    b.verts[4].pos[0] - b.verts[0].pos[0]
                );
            }
        }
    }

    use super::*;
    use crate::hud_tree::EventValue;
    use manifest::hud::{HudArg, HudLayout, HudNode};

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

    /// A group holding a half-full bar (z 2) over its Shadow-styled backing
    /// (z 3), shown by one event.
    fn bar_hud() -> Hud {
        let rect = |w: f32| {
            node(
                "bitmaprect",
                &[n(w), n(0.25), s("Left"), s("Top"), s("Viewport")],
                vec![],
            )
        };
        let layout = HudLayout {
            name: "t".into(),
            nodes: vec![node(
                "Group",
                &[s("bar")],
                vec![
                    node("propagatealpha", &[n(1.0)], vec![]),
                    node(
                        "Position",
                        &[n(0.25), n(0.5), n(0.0), s("Viewport")],
                        vec![],
                    ),
                    node("alpha", &[n(0.5)], vec![]),
                    node("eventenable", &[s("go")], vec![]),
                    node(
                        "barbitmap",
                        &[s("fill")],
                        vec![
                            node("Bitmap", &[s("sheet")], vec![]),
                            rect(0.5),
                            node("zorder", &[n(2.0)], vec![]),
                            node("color", &[n(0.0), n(150.0), n(213.0)], vec![]),
                            node("eventvalue", &[s("go")], vec![]),
                            node("eventenable", &[s("go")], vec![]),
                        ],
                    ),
                    node(
                        "barbitmap",
                        &[s("empty")],
                        vec![
                            node("Bitmap", &[s("sheet")], vec![]),
                            node("bitmapstyle", &[s("Shadow")], vec![]),
                            rect(0.5),
                            node("zorder", &[n(3.0)], vec![]),
                            node("eventenable", &[s("go")], vec![]),
                        ],
                    ),
                ],
            )],
        };
        let none = HudLayout {
            name: "none".into(),
            nodes: vec![],
        };
        let mut hud = Hud::new(&layout, &none);
        hud.send("go", EventValue::Number(0.5), &|_| false);
        hud.advance(0.0);
        hud
    }

    /// Back to front by z, the bar clipped to its value with its u range,
    /// the Shadow copy first at half alpha, the group's alpha carried down.
    #[test]
    fn a_bar_is_clipped_ordered_and_shadowed() {
        let hud = bar_hud();
        // 512 × 448 window pixels per frame = the frame itself, less the
        // PS2's overfill.
        let size = [512.0 * 448.0 / 448.0 * (448.0 / 448.0), 448.0];
        let f = draw(&hud, &HudArt::default(), size);
        // Three quads: the backing's shadow, the backing, the fill.
        assert_eq!(f.verts.len(), 12);
        assert_eq!(f.batches.len(), 1, "one sheet, one state: one batch");
        let (shadow, backing, fill) = (&f.verts[0..4], &f.verts[4..8], &f.verts[8..12]);
        assert_eq!(shadow[0].color, [0.0, 0.0, 0.0, 0.25]);
        assert_eq!(backing[0].color, [1.0, 1.0, 1.0, 0.5]);
        assert_eq!(fill[0].color, [0.0, 150.0 / 255.0, 213.0 / 255.0, 0.5]);
        let sc = frame_scale(size);
        // The shadow sits one frame pixel down and right of the backing.
        assert!((shadow[0].pos[0] - backing[0].pos[0] - sc[0]).abs() < 1.0e-3);
        assert!((shadow[0].pos[1] - backing[0].pos[1] - sc[1]).abs() < 1.0e-3);
        // The fill is half the backing's width, from the same left edge,
        // and shows the left half of its sheet.
        let width = |q: &[HudVertex]| q[1].pos[0] - q[0].pos[0];
        assert!((width(fill) * 2.0 - width(backing)).abs() < 1.0e-3);
        assert_eq!(fill[0].pos[0], backing[0].pos[0]);
        assert_eq!((fill[0].uv[0], fill[1].uv[0]), (0.0, 0.5));
        // Its box is the fill's alone.
        let i = hud.find("fill").unwrap();
        let boxes: Vec<_> = f.boxes.iter().filter(|(e, _)| *e == i).collect();
        assert_eq!(boxes.len(), 1);
        // Hidden again, nothing is drawn.
        let mut hidden = bar_hud();
        hidden.elems[0].fader.state = 0;
        assert!(draw(&hidden, &HudArt::default(), size).verts.is_empty());
    }

    #[test]
    fn a_scrolling_mesh_part_moves_its_uvs_with_the_clock() {
        let mut hud = bar_hud();
        let mut art = HudArt::default();
        let part = HudMeshPart {
            texture: format!("band{SCROLL_SHEET}"),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0.25, 0.5]; 3],
            indices: vec![0, 1, 2],
            scroll: [-0.5, -1.0],
            ..Default::default()
        };
        art.meshes.insert("m".into(), HudMesh { parts: vec![part] });
        let screen = Screen {
            origin: [0.0; 2],
            scale: [1.0; 2],
        };
        let uv = |hud: &Hud| {
            let mut ctx = Ctx {
                hud,
                art: &art,
                out: HudFrame::default(),
                clip: None,
            };
            ctx.mesh(0, "m", &Mat4::IDENTITY, &screen, [1.0; 4], false);
            ctx.out.verts[0].uv
        };
        assert_eq!(uv(&hud), [0.25, 0.5]);
        hud.advance(0.5);
        assert_eq!(uv(&hud), [0.0, 0.0]);
    }

    /// PR #61 review fix 6: the HUD clock wraps, and a disc-rate scroll
    /// (k/255 repeats a second) moves across the wrap as on any frame.
    #[test]
    fn the_hud_clock_wraps_without_a_scroll_jump() {
        let wrap = crate::hud_tree::CLOCK_WRAP;
        let mut hud = bar_hud();
        hud.clock = wrap - 0.01;
        let dt = 0.02;
        for rate in [-200.0 / 255.0, -1.0, 3.0 / 255.0] {
            let mut h = bar_hud();
            h.clock = hud.clock;
            let before = (rate * h.clock).rem_euclid(1.0);
            h.advance(dt);
            assert!((0.0..wrap).contains(&h.clock), "{}", h.clock);
            let moved = ((rate * h.clock).rem_euclid(1.0) - before).rem_euclid(1.0);
            let want = (rate * dt).rem_euclid(1.0);
            let d = (moved - want).abs();
            assert!(d.min(1.0 - d) < 1e-3, "rate {rate}: {moved} vs {want}");
        }
    }

    #[test]
    fn anchors_keep_the_4_3_offset_from_their_edge() {
        let wide = [1280.0, 720.0];
        assert_eq!(anchor_shift(0.07, wide), 0.0);
        assert_eq!(anchor_shift(0.5, wide), 160.0);
        assert_eq!(anchor_shift(0.86, wide), 320.0);
        // A 4:3 window is the PS2's own layout.
        for fx in [0.07, 0.5, 0.86] {
            assert_eq!(anchor_shift(fx, [640.0, 480.0]), 0.0);
        }
    }

    /// `re-hud.md` §2.4: a frame pixel is H/378.7 across and H/427.3 down.
    #[test]
    fn a_frame_pixel_is_the_ps2s() {
        let s = frame_scale([640.0, 480.0]);
        assert!((480.0 / s[0] - 378.7).abs() < 0.1, "{}", 480.0 / s[0]);
        assert!((480.0 / s[1] - 427.3).abs() < 0.1, "{}", 480.0 / s[1]);
    }

    /// The target bar's flash as drawn (`re-target-hud.md` §5): a fall from
    /// 1.0 to 0.6 draws the fill to 0.6 and, in front of it, a copy over
    /// 0.6–1.0 at the bar's colour fading with the fall's progress, its top
    /// sinking toward its bottom until FlashyScale (1 frame pixel of the
    /// element) is left; the backing and the fill carry the group's tint.
    #[test]
    fn a_falling_bar_draws_a_thinning_fading_copy() {
        let mut hud = crate::hud_tree::fixture::target_hud();
        let ev = "player1.weapon1.target.healthFraction";
        hud.send(
            "player1.weapon1.target.teamColor",
            EventValue::Color([200, 100, 100]),
            &|_| false,
        );
        hud.send(ev, EventValue::Number(1.0), &|_| false);
        hud.advance(1.0);
        hud.send(ev, EventValue::Number(0.6), &|_| false);
        let size = [640.0, 480.0];
        let quads = |hud: &Hud| {
            let f = draw(hud, &HudArt::default(), size);
            assert_eq!(f.verts.len(), 12, "backing, fill, flash");
            f.verts
        };
        let x = |q: &[HudVertex]| (q[0].pos[0], q[1].pos[0]);
        let h = |q: &[HudVertex]| q[2].pos[1] - q[0].pos[1];
        let v0 = quads(&hud);
        let (back, fill, flash) = (&v0[0..4], &v0[4..8], &v0[8..12]);
        // The fill ends where the copy starts; the copy spans 0.4 of the
        // bar's width (the fill is 0.6 of it).
        assert!((x(fill).1 - x(flash).0).abs() < 1e-3);
        let full = (x(fill).1 - x(fill).0) / 0.6;
        assert!(((x(flash).1 - x(flash).0) - 0.4 * full).abs() < 1e-3);
        assert!(
            (h(flash) - h(fill)).abs() < 1e-3,
            "full height at the change"
        );
        assert_eq!(flash[0].color, fill[0].color);
        // The tint: the fill red × (192, 224, 224), the backing (39, 40, 46)
        // × the same, both at the group's 0.8.
        let want = [200.0 * 192.0, 100.0 * 224.0, 100.0 * 224.0].map(|c| c / 255.0 / 255.0);
        for (got, want) in fill[0].color.iter().zip(want) {
            assert!((got - want).abs() < 1e-4, "{:?}", fill[0].color);
        }
        assert!((back[0].color[1] - 40.0 * 224.0 / 255.0 / 255.0).abs() < 1e-4);
        assert!((fill[0].color[3] - 0.8).abs() < 1e-6);
        // Half way: half the alpha, the top half way down to FlashyScale.
        hud.advance(0.5);
        let v1 = quads(&hud);
        let (fill1, flash1) = (&v1[4..8], &v1[8..12]);
        assert!((flash1[0].color[3] - 0.4).abs() < 1e-4);
        let px = h(fill1)
            / hud.elems[hud.find("player1targethealth_fill").unwrap()]
                .rect
                .h;
        assert!((h(flash1) - (0.5 * h(fill1) + 0.5 * px)).abs() < 1e-3);
        // The bottom edge stays put.
        assert!((flash1[2].pos[1] - fill1[2].pos[1]).abs() < 1e-3);
        // At the end of the fall time the copy is gone.
        hud.advance(0.51);
        assert_eq!(draw(&hud, &HudArt::default(), size).verts.len(), 8);
    }
}

#[cfg(test)]
mod damage_flash_tests {
    use super::*;
    #[test]
    fn damage_flash_covers_the_whole_viewport_with_normal_red_alpha() {
        let layout = manifest::hud::HudLayout {
            name: String::new(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&layout, &layout);
        hud.damage_flash = 0.5;
        let frame = draw(&hud, &HudArt::default(), [1280.0, 720.0]);
        assert_eq!(frame.verts.len(), 4);
        assert_eq!(frame.indices.len(), 6);
        assert_eq!(frame.verts[0].pos, [0.0, 0.0, 0.0]);
        assert_eq!(frame.verts[2].pos, [1280.0, 720.0, 0.0]);
        assert!(frame.verts.iter().all(|v| v.color == [1.0, 0.0, 0.0, 0.5]));
        assert_eq!(frame.batches[0].texture, "white");
        assert!(!frame.batches[0].additive);
        hud.damage_flash = 0.0;
        assert!(draw(&hud, &HudArt::default(), [1280.0, 720.0])
            .verts
            .is_empty());
    }
}

/// Issue #194 B22: the name tags draw natively, in the event feed's own font.
#[cfg(test)]
mod name_tag_tests {
    use super::*;
    use crate::hud::NameTagDraw;
    use manifest::hud::{HudArg, HudGlyph, HudLayout, HudNode};

    fn node(name: &str, args: &[HudArg], children: Vec<HudNode>) -> HudNode {
        HudNode {
            name: name.into(),
            args: args.to_vec(),
            children,
        }
    }

    /// A HUD whose only element is the event feed's message box, in `feedfont` at 0.8 x 0.9.
    fn feed_hud() -> Hud {
        let s = |v: &str| HudArg::Str(v.into());
        let n = HudArg::Num;
        let layout = HudLayout {
            name: "t".into(),
            nodes: vec![node(
                "MultiLineText",
                &[s("player1messages")],
                vec![
                    node("TextFont", &[s("FeedFont")], vec![]),
                    node("TextScale", &[n(0.8), n(0.9)], vec![]),
                    node("EventText", &[s(crate::hud::MESSAGE_EVENT)], vec![]),
                ],
            )],
        };
        let none = HudLayout {
            name: "none".into(),
            nodes: vec![],
        };
        Hud::new(&layout, &none)
    }

    /// A font on page `page` with a 10-texel glyph for every character of `chars`.
    fn font(name: &str, page: &str, chars: &str) -> HudFont {
        HudFont {
            name: name.into(),
            line_height: 12,
            pages: vec![page.into()],
            glyphs: chars
                .chars()
                .map(|c| HudGlyph {
                    code: c as u32 as _,
                    page: 0,
                    advance: 8,
                    x0: 0,
                    x1: 7,
                    y0: 1,
                    y1: 11,
                    uv: [0.0, 1.0, 0.0, 1.0],
                })
                .collect(),
        }
    }

    const LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    /// The name keyboard's symbols page (`shell_ps2::profiles`).
    const SYMBOLS: &str = "0123456789 !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

    fn art() -> HudArt {
        let mut art = HudArt::default();
        let all = format!("{LETTERS}{SYMBOLS}");
        art.fonts
            .insert("feedfont".into(), font("feedfont", "feedpage", &all));
        art.fonts.insert(
            crate::hud_tree::DEFAULT_FONT.into(),
            font("tiny", "tinypage", &all),
        );
        art
    }

    fn tag(name: &str, at: [f32; 2]) -> NameTagDraw {
        NameTagDraw {
            at,
            name: name.into(),
            friend: true,
            alpha: 1.0,
        }
    }

    fn tag_boxes(f: &HudFrame) -> Vec<[f32; 4]> {
        f.boxes
            .iter()
            .filter(|(e, _)| *e == NAME_TAG_BOX)
            .map(|(_, b)| *b)
            .collect()
    }

    #[test]
    fn name_tags_draw_in_the_hud_font() {
        let art = art();
        let mut hud = feed_hud();
        // Wide quarter cell: the tag's glyphs are the feed font's, inside the cell.
        let f = Frame::cell([1280.0, 720.0], 1.0);
        hud.name_tags = vec![tag("Han", [640.0, 360.0])];
        let out = draw_in(&hud, &art, &f);
        assert!(!out.batches.is_empty());
        assert!(
            out.batches.iter().all(|b| b.texture == "feedpage"),
            "the feed's font page, not the default font's"
        );
        let boxes = tag_boxes(&out);
        assert_eq!(boxes.len(), 3, "one quad per letter");
        // The feed's text scale times the cell's frame scale.
        let k = f.scale();
        let w = boxes[0][2] - boxes[0][0];
        assert!((w - 7.0 * 0.8 * k[0]).abs() < 1e-3, "{w}");
        for b in &boxes {
            assert!(b[0] >= 0.0 && b[2] <= 1280.0 && b[1] >= 0.0 && b[3] <= 720.0);
            assert!(b[3] <= 360.0 + 1e-3, "bottom on the anchor: {b:?}");
        }
        // The HUD Size setting shrinks the tag with the feed.
        let small = draw_in(&hud, &art, &Frame::cell([1280.0, 720.0], 0.7));
        let ws = tag_boxes(&small)[0];
        assert!((ws[2] - ws[0]) < w * 0.75);
        // A tiny cell: never shorter than the floor (glyph ink 10 of a 12-texel line).
        let tiny = draw_in(&hud, &art, &Frame::cell([160.0, 90.0], 0.7));
        let t = tag_boxes(&tiny)[0];
        assert!(
            (t[3] - t[1]) >= NAME_TAG_MIN_PX * 10.0 / 12.0 - 1e-3,
            "{t:?}"
        );
        // No feed in the layout: the default font.
        let mut bare = Hud::new(
            &HudLayout {
                name: "e".into(),
                nodes: vec![],
            },
            &HudLayout {
                name: "e".into(),
                nodes: vec![],
            },
        );
        bare.name_tags = hud.name_tags.clone();
        let out = draw_in(&bare, &art, &f);
        assert!(out.batches.iter().all(|b| b.texture == "tinypage"));
    }

    /// A tall cell (3 or 4 players): the tag stays at its full-cell pixel, not moved onto the
    /// cell's 4:3 band.
    #[test]
    fn name_tags_sit_at_their_full_cell_pixel_in_a_tall_cell() {
        let art = art();
        let mut hud = feed_hud();
        let f = Frame::cell([480.0, 1000.0], 1.0);
        assert!(f.proj[1] > 0.0, "a banded cell");
        hud.name_tags = vec![tag("Leia", [240.0, 100.0])];
        let boxes = tag_boxes(&draw_in(&hud, &art, &f));
        assert_eq!(boxes.len(), 4);
        let left = boxes.iter().map(|b| b[0]).fold(f32::MAX, f32::min);
        let right = boxes.iter().map(|b| b[2]).fold(f32::MIN, f32::max);
        let bottom = boxes.iter().map(|b| b[3]).fold(f32::MIN, f32::max);
        // Centred on x 240 (the last advance's slack aside), bottom at y 100.
        let k = f.scale();
        assert!(((left + right) * 0.5 - 240.0).abs() <= 2.0 * 0.8 * k[0] + 1e-3);
        let height = (12.0 * 0.9 * k[1]).max(NAME_TAG_MIN_PX);
        assert!(
            bottom <= 100.0 + 1e-3 && bottom >= 100.0 - height,
            "{bottom}"
        );
    }

    /// Every character of the name keyboard draws a quad when the feed's font has it; one the
    /// font lacks is dropped, the same as in a feed line (`Ctx::run`).
    #[test]
    fn name_tags_cover_the_name_keyboard() {
        let art = art();
        let mut hud = feed_hud();
        let f = Frame::window([1280.0, 720.0]);
        let drawn = SYMBOLS.chars().count();
        hud.name_tags = vec![tag(SYMBOLS, [640.0, 360.0])];
        assert_eq!(tag_boxes(&draw_in(&hud, &art, &f)).len(), drawn);
        hud.name_tags = vec![tag(LETTERS, [640.0, 360.0])];
        assert_eq!(tag_boxes(&draw_in(&hud, &art, &f)).len(), LETTERS.len());
        hud.name_tags = vec![tag("Han\u{2603}", [640.0, 360.0])];
        assert_eq!(tag_boxes(&draw_in(&hud, &art, &f)).len(), 3);
    }

    /// A sniper scope covers the world: no tags over it.
    #[test]
    fn name_tags_hide_under_the_scope() {
        let art = art();
        let mut hud = feed_hud();
        hud.name_tags = vec![tag("Han", [320.0, 180.0])];
        hud.scope = Some(crate::hud_scope::ScopeView);
        let out = draw_in(&hud, &art, &Frame::window([640.0, 360.0]));
        assert!(tag_boxes(&out).is_empty());
    }
}
