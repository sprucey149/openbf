//! Laser bolts: the ordnance class's own draw (card FXB,
//! `analysis/re-bolt.md` §3, [O]).
//!
//! A bolt is TWO camera-facing tapered ribbons the PS2 builds on the CPU
//! (B1 `FUN_0047e108` / B2 `FUN_00569328` modes 4 and 5), core first:
//! - the core: the class's `LaserTexture`, NORMAL blend (GS ALPHA 0x44),
//!   half-width w at the head → w/2 over min(len, LaserLength) behind it,
//!   plus a cap of w ahead of the head; white, alpha = the fade;
//! - the glow: `lightglow`, ADDITIVE (0x48), 2w → 1.6w over min(len,
//!   GlowLength), cap 2w; the `LaserGlowColor`.
//!
//! One ribbon from the head H (half-width s) to its tail P (half-width p),
//! with E the eye (§3.2): n = normalize((P−E)×(H−E)) (−camera right when
//! that is zero), u = normalize((H−E)×n), pointing from H toward the tail.
//! The cap is the strip H−us+ns (U,V 0,0), H−us−ns (0,1), H+ns (0.25,0),
//! H−ns (0.25,1); the body a fan round H (0.25,0.5) over H−ns (0.25,1),
//! P−np (1,1), P+np (1,0), H+ns (0.25,0). Nose-on — the eye's squared
//! distance d² to the line (H, P−H) below s² — the body's alpha becomes
//! a·d²/s² and a mirrored cap behind the head (H+us±ns → H±ns) takes the
//! rest, except its (0,1) vertex, which keeps the full alpha (the PS2
//! rebuilds that one from the full colour, both games).
//!
//! Every vertex byte goes to the GS scale, (b+1)>>1 with 128 = 1.0, and the
//! texture modulates it (no overbright). Depth tested, never written, never
//! fogged, unlit. The port draws the frame's bolts after its blend pass,
//! each bolt's core then its glow, sorted far to near AMONG the particle
//! effects as the PS2 does (§3.4; `particles.rs` builds the shared list).

use crate::grass::{GrassInstance, GrassVertex};
use crate::{Camera, Renderer, TextureHandle};
use glam::Vec3;

/// One bolt ready to draw — the game layer applies the class's rules
/// (re-bolt §3.1, §4) and hands the renderer the geometry and colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoltDraw {
    /// The head H.
    pub head: Vec3,
    /// Unit direction of flight (the tails lie behind the head along −dir).
    pub dir: Vec3,
    /// min(len, LaserLength), min(len, GlowLength).
    pub core_len: f32,
    pub glow_len: f32,
    /// w = LaserWidth·(0.9·fade + 0.1).
    pub width: f32,
    pub core_texture: Option<TextureHandle>,
    pub glow_texture: Option<TextureHandle>,
    /// Vertex bytes as the PS2 computes them (before the GS halving).
    pub core_color: [u8; 4],
    pub glow_color: [u8; 4],
}

/// One vertex of a ribbon's triangle list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RibbonVertex {
    pub pos: Vec3,
    pub uv: [f32; 2],
    /// The vertex alpha byte (before the GS halving).
    pub alpha: u8,
}

/// The glow's sizes against the core's: 2w at the head, × 0.8 at the tail.
pub const GLOW_WIDTH: f32 = 2.0;
pub const GLOW_TAIL: f32 = 0.8;
/// The core's tail against its head.
pub const CORE_TAIL: f32 = 0.5;
/// U at the head (the cap runs 0 → 0.25, the body 0.25 → 1).
pub const HEAD_U: f32 = 0.25;

/// UV as the PS2 packs it: (int)(u·2047), in 1/2048.
fn q(u: f32) -> f32 {
    (u * 2047.0).floor() / 2048.0
}

/// A vertex byte on the GS scale, 1.0 = 0x80.
pub fn gs_unit(b: u8) -> f32 {
    ((u32::from(b) + 1) >> 1) as f32 / 128.0
}

/// One ribbon (mode 4 latching H and s, then mode 5 to P and p) as a
/// triangle list, for the eye `eye` (see the module doc).
pub fn ribbon(
    h: Vec3,
    s: f32,
    p_pt: Vec3,
    p: f32,
    eye: Vec3,
    cam_right: Vec3,
    a: u8,
) -> Vec<RibbonVertex> {
    let n = (p_pt - eye).cross(h - eye);
    let n = if n.length_squared() > 1e-12 {
        n.normalize()
    } else {
        -cam_right.normalize_or_zero()
    };
    let u = (h - eye).cross(n).normalize_or_zero();
    let axis = p_pt - h;
    let rel = eye - h;
    let d2 = if axis.length_squared() > 0.0 {
        (rel - axis * (rel.dot(axis) / axis.length_squared())).length_squared()
    } else {
        rel.length_squared()
    };
    let (a_body, a_mirror) = if d2 < s * s {
        let ab = (f32::from(a) * d2 / (s * s)) as u8;
        (ab, a - ab)
    } else {
        (a, 0)
    };
    let v = |pos: Vec3, uu: f32, vv: f32, alpha: u8| RibbonVertex {
        pos,
        uv: [q(uu), q(vv)],
        alpha,
    };
    let mut out = Vec::with_capacity(21);
    let strip = |out: &mut Vec<RibbonVertex>, k: [RibbonVertex; 4]| {
        out.extend([k[0], k[1], k[2], k[2], k[1], k[3]]);
    };
    // The cap ahead of the head.
    strip(
        &mut out,
        [
            v(h - u * s + n * s, 0.0, 0.0, a),
            v(h - u * s - n * s, 0.0, 1.0, a),
            v(h + n * s, HEAD_U, 0.0, a),
            v(h - n * s, HEAD_U, 1.0, a),
        ],
    );
    if a_body > 0 {
        let c = v(h, HEAD_U, 0.5, a_body);
        let f = [
            v(h - n * s, HEAD_U, 1.0, a_body),
            v(p_pt - n * p, 1.0, 1.0, a_body),
            v(p_pt + n * p, 1.0, 0.0, a_body),
            v(h + n * s, HEAD_U, 0.0, a_body),
        ];
        for k in 0..3 {
            out.extend([c, f[k], f[k + 1]]);
        }
    }
    if a_mirror > 0 {
        strip(
            &mut out,
            [
                v(h + u * s + n * s, 0.0, 0.0, a_mirror),
                v(h + u * s - n * s, 0.0, 1.0, a),
                v(h + n * s, HEAD_U, 0.0, a_mirror),
                v(h - n * s, HEAD_U, 1.0, a_mirror),
            ],
        );
    }
    out
}

/// One draw of the frame's bolts: a vertex range, its sheet, and whether it
/// is a glow (ADDITIVE) or a core (NORMAL).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoltRun {
    pub texture: Option<TextureHandle>,
    pub vertices: std::ops::Range<u32>,
    pub glow: bool,
    pub solid: bool,
    pub cutout: bool,
    pub premultiplied: bool,
}

pub(crate) struct DisplayPipelines {
    pub core: wgpu::RenderPipeline,
    pub atmosphere: wgpu::RenderPipeline,
    pub glow: wgpu::RenderPipeline,
    pub solid: wgpu::RenderPipeline,
    pub cutout: wgpu::RenderPipeline,
    pub format: wgpu::TextureFormat,
}

#[derive(Debug, Default)]
pub(crate) struct BoltFrame {
    pub vertices: Vec<GrassVertex>,
    pub runs: Vec<BoltRun>,
    pub blur_runs: Vec<(TextureHandle, std::ops::Range<u32>)>,
}

impl BoltFrame {
    pub fn append(&mut self, other: BoltFrame) {
        let base = self.vertices.len() as u32;
        let shift = |range: std::ops::Range<u32>| range.start + base..range.end + base;
        self.vertices.extend(other.vertices);
        self.runs.extend(other.runs.into_iter().map(|run| BoltRun {
            vertices: shift(run.vertices),
            ..run
        }));
        self.blur_runs.extend(
            other
                .blur_runs
                .into_iter()
                .map(|(texture, range)| (texture, shift(range))),
        );
    }
}

/// The one instance every bolt draw uses: the vertices are in world space.
pub(crate) fn identity_instance() -> GrassInstance {
    GrassInstance {
        model: glam::Mat4::IDENTITY.to_cols_array_2d(),
        color: [1.0; 4],
        params: [0.0, 1.0, 0.0, 0.0],
    }
}

/// One bolt's two ribbons for `camera`, core then glow: (vertices, sheet,
/// glow). `particles::build` sorts them among the particles, far to near.
pub(crate) fn bolt_vertices(
    b: &BoltDraw,
    camera: &Camera,
) -> [(Vec<GrassVertex>, Option<TextureHandle>, bool); 2] {
    let eye = camera.position;
    let right = camera.right();
    let w = b.width;
    [false, true].map(|glow| {
        let (s, p, len, tex, col) = if glow {
            (
                w * GLOW_WIDTH,
                w * GLOW_WIDTH * GLOW_TAIL,
                b.glow_len,
                b.glow_texture,
                b.glow_color,
            )
        } else {
            (w, w * CORE_TAIL, b.core_len, b.core_texture, b.core_color)
        };
        let rgb = [gs_unit(col[0]), gs_unit(col[1]), gs_unit(col[2])];
        let verts = ribbon(
            b.head,
            s,
            b.head - b.dir * len.max(0.0),
            p,
            eye,
            right,
            col[3],
        )
        .into_iter()
        .map(|rv| GrassVertex {
            position: rv.pos.to_array(),
            uv: rv.uv,
            color: [rgb[0], rgb[1], rgb[2], gs_unit(rv.alpha)],
            up: 0.0,
        })
        .collect();
        (verts, tex, glow)
    })
}

/// The renderer's bolts: this frame's list and the buffers they upload to.
#[derive(Default)]
pub(crate) struct BoltState {
    pub bolts: Vec<BoltDraw>,
    pub vertex_buf: Option<(wgpu::Buffer, usize)>,
    pub instance_buf: Option<(wgpu::Buffer, usize)>,
}

impl Renderer {
    /// Replace the bolts drawn from the next frame on (none until this is
    /// called). Texture handles must be this renderer's.
    pub fn set_bolts(&mut self, bolts: Vec<BoltDraw>) {
        self.bolts.bolts = bolts;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §3.2's geometry, hand computed: the eye 10 m out on +Z looking at a
    /// bolt flying along +X at the origin. n is ±Y, u = −X toward the tail
    /// (+X of the head is ahead); the cap spans s ahead of the head, the
    /// body tapers s → p, and the U runs 0 / 0.25 / 1.
    #[test]
    fn a_ribbon_is_the_ps2_strip() {
        let h = Vec3::ZERO;
        let tail = Vec3::new(-3.0, 0.0, 0.0);
        let eye = Vec3::new(0.0, 0.0, 10.0);
        let r = ribbon(h, 0.2, tail, 0.16, eye, Vec3::X, 100);
        assert_eq!(r.len(), 6 + 9, "cap + body, no mirrored cap");
        let n = (tail - eye).cross(h - eye).normalize();
        assert!(n.abs_diff_eq(Vec3::NEG_Y, 1e-6), "{n}");
        // The cap tip: ahead of the head (along +X, the flight direction).
        assert!(
            r[0].pos.abs_diff_eq(Vec3::new(0.2, -0.2, 0.0), 1e-6),
            "{:?}",
            r[0].pos
        );
        assert!(r[1].pos.abs_diff_eq(Vec3::new(0.2, 0.2, 0.0), 1e-6));
        assert_eq!((r[0].uv, r[1].uv), ([0.0, 0.0], [0.0, q(1.0)]));
        assert_eq!(r[2].uv, [q(0.25), 0.0]);
        // The body's tail corners at half-width p and U = 1.
        let tails: Vec<_> = r[6..].iter().filter(|v| v.uv[0] == q(1.0)).collect();
        assert!(tails
            .iter()
            .all(|v| (v.pos.x + 3.0).abs() < 1e-6 && (v.pos.y.abs() - 0.16).abs() < 1e-6));
        assert!(r.iter().all(|v| v.alpha == 100));
    }

    /// Nose-on (the eye on the bolt's line): the body gets a·d²/s², the
    /// mirrored cap the rest with its (0,1) vertex at the full alpha; on the
    /// line itself the body is skipped.
    #[test]
    fn a_nose_on_bolt_splits_its_alpha() {
        let h = Vec3::ZERO;
        let tail = Vec3::new(0.0, 0.0, -1.0);
        let eye = Vec3::new(0.25, 0.0, 5.0);
        let r = ribbon(h, 0.5, tail, 0.25, eye, Vec3::X, 200);
        // d² = 1/16, s² = 1/4: a_b = 50, the mirror 150.
        assert_eq!(r.len(), 6 + 9 + 6);
        assert!(r[6..15].iter().all(|v| v.alpha == 50));
        let m = &r[15..];
        assert_eq!(
            m.iter().filter(|v| v.alpha == 200).count(),
            2,
            "the (0,1) vertex, twice in the list"
        );
        assert!(m.iter().filter(|v| v.alpha != 200).all(|v| v.alpha == 150));
        let r = ribbon(h, 0.5, tail, 0.25, Vec3::new(0.0, 0.0, 5.0), Vec3::X, 200);
        assert_eq!(r.len(), 12, "cap + mirrored cap, no body");
    }

    /// A bolt at len 0 draws only its caps (a degenerate body), and the GS
    /// scale maps 255 to 1.0 and the authored glow alpha 100 to 50/128.
    #[test]
    fn caps_only_at_zero_length_and_the_gs_scale() {
        let r = ribbon(
            Vec3::ZERO,
            0.2,
            Vec3::ZERO,
            0.1,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::X,
            255,
        );
        assert_eq!(r.len(), 15);
        assert_eq!(gs_unit(255), 1.0);
        assert_eq!(gs_unit(100), 50.0 / 128.0);
        assert_eq!(gs_unit(0), 0.0);
    }
}
