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
    pub meshes: HashMap<String, HudMesh>,
    /// Width / height of the sheets a `bitmap:<sheet>` weapon icon may name.
    pub sheet_aspect: HashMap<String, f32>,
    /// The control prompts' icons (`crate::input_icons`).
    pub icons: crate::input_icons::Icons,
}

impl HudArt {
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
}

impl Ctx<'_> {
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
            let first = self.out.indices.len() as u32;
            let base = self.out.verts.len() as u32;
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for (x, y, u, v) in corners {
                let p = m.transform_point3(Vec3::new(x, y, 0.0)) + Vec3::new(*off, *off, 0.0);
                let w = s.map(p);
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
            Kind::Text => self.text(i, e, &m, s, alpha),
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
        let (ox, oy) = e.rect.origin();
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
                    ox + e.rect.w * 0.5 * (1.0 + sn),
                    oy + e.rect.h * 0.5 * (1.0 - cs),
                    0.0,
                )) * Mat4::from_rotation_z(th)
            } else {
                Mat4::from_translation(Vec3::new(
                    ox + (e.rect.w.trunc() / n as f32) * k as f32,
                    oy,
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

    fn text(&mut self, i: usize, e: &Elem, m: &Mat4, s: &Screen, alpha: f32) {
        let Some(font) = self.art.fonts.get(&e.font) else {
            return;
        };
        let height = f32::from(font.line_height) * e.text_scale[1];
        let top = match e.v_align {
            VAlign::Top => 0.0,
            VAlign::Center => -height * 0.5,
            VAlign::Bottom => -height,
        };
        self.run(
            i,
            m,
            s,
            font,
            &Run {
                text: &e.text,
                at: (0.0, top),
                h_align: e.h_align,
                scale: e.text_scale,
                color: color(self.hud.drawn(i), alpha),
                additive: e.additive,
                shadow: false,
            },
        );
    }

    pub(crate) fn run(&mut self, i: usize, m: &Mat4, s: &Screen, font: &HudFont, r: &Run<'_>) {
        let glyphs: Vec<&HudGlyph> = r
            .text
            .chars()
            .filter_map(|c| font.glyphs.iter().find(|g| u32::from(g.code) == c as u32))
            .collect();
        let [kx, ky] = r.scale;
        let spacing = if r.shadow { SHADOW_SPACING } else { 0.0 };
        let width = run_width(font, r.text, kx, r.shadow);
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
                    self.quad(
                        i,
                        m,
                        s,
                        &Quad {
                            rect: [
                                pen + f32::from(g.x0) * kx,
                                top + off + f32::from(g.y0) * ky,
                                pen + f32::from(g.x1) * kx,
                                top + off + f32::from(g.y1) * ky,
                            ],
                            uv: [g.uv[0], g.uv[2], g.uv[1], g.uv[3]],
                            color: *col,
                            texture: page,
                            additive: r.additive,
                            shadow: false,
                            boxed: *boxed,
                        },
                    );
                }
                pen += (f32::from(g.advance) + spacing) * kx;
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
            self.run(
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
pub fn draw(hud: &Hud, art: &HudArt, size: [f32; 2]) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
    };
    if let Some((menus, movie)) = &hud.shell {
        ctx.shell_page(menus, *movie, size);
        if let Some(ia) = &hud.instant {
            ctx.instant_page(ia, menus.pulse.alpha(), size);
        }
        return ctx.out;
    }
    if let Some(result) = hud.banner {
        return crate::hud::draw_round_end(result, size);
    }
    if let Some(st) = &hud.stats {
        ctx.stats_page(st, size);
        return ctx.out;
    }
    let scale = frame_scale(size);
    let box_w = 4.0 / 3.0 * size[1];
    // A spawn screen (card SSELB) dims the backdrop under everything; the
    // page says whether the tree still draws under it.
    let tree = match &hud.spawn {
        Some(page) => {
            ctx.spawn_veil(size);
            page.keeps_tree()
        }
        None => true,
    };
    // The sniper scope (`crate::hud_scope`): under the tree, so the target
    // readout and the reticle stay over it.
    if let Some(v) = hud.scope.as_ref().filter(|_| tree) {
        ctx.scope(v, size);
    }
    let mut order: Vec<usize> = if tree { hud.roots.clone() } else { Vec::new() };
    order.reverse();
    order.sort_by_key(|c| std::cmp::Reverse(hud.elems[*c].z_order));
    for r in order {
        let e = &hud.elems[r];
        let (screen, parent) = match e.window_pos {
            // A projected world point (the reticle): the element's origin is
            // that window pixel.
            Some(p) => (
                Screen {
                    origin: [p[0] * size[0], p[1] * size[1]],
                    scale,
                },
                Mat4::from_translation(Vec3::new(-e.pos.x, -e.pos.y, 0.0)),
            ),
            None => (
                Screen {
                    origin: [
                        OVERFILL[0].0 * box_w + anchor_shift(e.pos.x / FRAME_W, size),
                        OVERFILL[1].0 * size[1],
                    ],
                    scale,
                },
                Mat4::IDENTITY,
            ),
        };
        ctx.element(r, &parent, &screen);
    }
    // The capture icon: a root group of the PS2's own (card CPBAR).
    if let Some(icon) = hud.capture.as_ref().filter(|_| tree) {
        ctx.capture(icon, size);
    }
    // The hit-direction chevrons: one of the PS2's code-drawn pieces (card
    // HATK).
    if tree {
        ctx.ambush(&hud.ambush, size);
    }
    // BF1's squad order cross: over everything, like the PS2's own group.
    if let Some(sq) = hud.squad.as_ref().filter(|_| tree) {
        ctx.squad(sq, size);
    }
    if let Some(page) = &hud.spawn {
        ctx.spawn_page(page, size);
    }
    ctx.out
}

#[cfg(test)]
mod tests {
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
