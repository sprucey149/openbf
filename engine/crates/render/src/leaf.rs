//! The leaf patches: the PS2's `leafpatch` sprite clusters
//! (`analysis/re-leafpatch.md` §5–§7, [O]; card LEAF).
//!
//! A class holds ONE template of sprites (`game::viewer::leaf` builds it as
//! B1 `FUN_001b38f8` does); every object of the class draws that template
//! under its own matrix — up from its hardpoint or instance, yaw dropped, the
//! up axis × the class's vertical scale — with its own tint. Each view the
//! PS2 builds the class's sprite buffer in TEMPLATE space from the world
//! camera's axes, back to front (B1 `FUN_002ee3c0`, B2 `FUN_003c8260`), one
//! quad per sprite (`FUN_002ee030`), and draws it through each visible
//! object's matrix (`FUN_002ee970`). It rides the grass pass (`grass.rs`):
//! the same vertex and instance buffers, pipeline and far-to-near order.
//!
//! - Sprite i is a square of side `size` centred on `p + 0.1·back`: right =
//!   `normalize(cam right + back·(±0.3·(i mod 5)))` (odd i negative), up =
//!   the camera's up; its top edge sheared along right by `sway·size`,
//!   `sway = sin(phase₀ + t·rate)·amplitude` (the class wind, §7; every
//!   object of a class sways in step).
//! - Vertex k takes UV corner k — top-left, top-right, bottom-left,
//!   bottom-right — from the part's row of the ELF's table (§6.4).
//! - An object draws while `−bound ≤ d ≤ D` (d its depth along the look
//!   axis, D its MaxDistance), alpha `min((D − d)/(0.4·D), 1)` — or, nearer
//!   than 6 m and not a vine, `max((d/6)², 0.1)` — times the NearSceneRange
//!   fade `1 − (d − b)/((c + bound) − b)` past b (§6.1).
//!
//! Colour: texture × the sprite's grey × the object's tint (GS MODULATE),
//! alpha = texel × the class `Alpha` × the fade, BLENDED — the PS2 blends
//! every texel (ALPHA 0x44) and writes depth only where the alpha passes
//! 0.5 (TEST 0x5140b, AFAIL FB_ONLY); most of BF2 YAV1's fading canopy is the
//! part below the test (§6.5), so an alpha test would lose it. The port
//! draws the grass pass's way: depth-tested, no depth write, far to near.
//! BF1 leaves are unlit; BF2's are lit on ±Y normals through the static
//! `Lit` set — **PROVISIONAL [H]** (§6.6: the PS2 lights them; the exact sum
//! and which light set is left in its buffer are unread).

use crate::grass::{GrassFrame, GrassInstance, GrassRun, GrassVertex};
use crate::{Camera, Renderer, TextureHandle};
use glam::{Mat4, Vec3};

/// One sprite of a class template, in the object's frame (y already ÷ the
/// class's vertical scale — the object's up axis carries it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeafSprite {
    pub pos: Vec3,
    pub size: f32,
    /// The sun-shaded grey (the vertex colour, 1.0 = 0x80).
    pub grey: f32,
    /// The sway phase at t = 0.
    pub phase: f32,
    /// The atlas part, `i mod 4`.
    pub part: u8,
}

/// One `leafpatch` class ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct LeafClass {
    pub texture: Option<TextureHandle>,
    pub sprites: Vec<LeafSprite>,
    /// `NumParts == 1`: the whole sheet turned by 90° per part; else the
    /// sheet's quarters.
    pub whole_sheet: bool,
    /// The class `Alpha` (the vertex alpha).
    pub alpha: f32,
    /// The sway: phase rate (per second) and amplitude.
    pub wind: [f32; 2],
    /// The template's radius about its box centre (+0x5c).
    pub bound: f32,
    /// A vine: no near-camera fade.
    pub vine: bool,
    /// BF2: lit on the quads' ±Y normals (PROVISIONAL, see the module doc).
    pub lit: bool,
}

/// One placed leaf object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeafObject {
    pub class: u32,
    /// Template → world: columns right, up × the class's vertical scale,
    /// forward, position (re-leafpatch §5).
    pub matrix: Mat4,
    /// The draw colour, 1.0 = unity (the object's tint, or a FOLG record's).
    pub color: [f32; 4],
    /// D, metres.
    pub distance: f32,
}

/// The per-sprite twist toward the back axis, × (i mod 5).
const TWIST: f32 = 0.3;
/// The quad's push toward the camera along the back axis.
const PUSH: f32 = 0.1;
/// The fade over the last 40 % of D.
const FADE: f32 = 0.4;
/// +0xec: nearer than this an object fades toward 0.1 (no setter).
const NEAR: f32 = 6.0;

/// The UV tables (u16 (u, v) in 1/2048, corners TL, TR, BL, BR, by part):
/// B1 0x518ac0 (`NumParts == 1`) and 0x518a80 (else), byte-identical in BF2
/// at 0x5ffca8 / 0x5ffc68 (re-leafpatch §6.4).
const WHOLE: [[[u16; 2]; 4]; 4] = [
    [[0, 0], [2047, 0], [0, 2047], [2047, 2047]],
    [[2047, 0], [2047, 2047], [0, 0], [0, 2047]],
    [[2047, 2047], [0, 2047], [2047, 0], [0, 0]],
    [[0, 2047], [0, 0], [2047, 2047], [2047, 0]],
];
const QUARTERS: [[[u16; 2]; 4]; 4] = [
    [[0, 0], [1023, 0], [0, 1023], [1023, 1023]],
    [[1023, 0], [2047, 0], [1023, 1023], [2047, 1023]],
    [[0, 1023], [1023, 1023], [0, 2047], [1023, 2047]],
    [[1023, 1023], [2047, 1023], [1023, 2047], [2047, 2047]],
];

/// Part `part`'s four UV corners.
pub fn uv(whole_sheet: bool, part: u8) -> [[f32; 2]; 4] {
    let t = if whole_sheet { &WHOLE } else { &QUARTERS };
    t[(part % 4) as usize].map(|[u, v]| [u as f32 / 2048.0, v as f32 / 2048.0])
}

/// The camera's axes as the leaf build reads them.
#[derive(Debug, Clone, Copy)]
pub struct LeafAxes {
    pub right: Vec3,
    pub up: Vec3,
    /// The look direction (−back).
    pub look: Vec3,
}

impl LeafAxes {
    pub fn of(c: &Camera) -> LeafAxes {
        let right = c.right().normalize_or_zero();
        let look = c.forward().normalize_or_zero();
        LeafAxes {
            right,
            up: right.cross(look).normalize_or_zero(),
            look,
        }
    }
}

/// A sprite's sway shear at `t` seconds. `t` is [`Renderer::time`], wrapped
/// at [`crate::TIME_WRAP`] by the caller: the sway jumps phase once at each
/// wrap (as the grass's does).
pub fn sway(s: &LeafSprite, wind: [f32; 2], t: f32) -> f32 {
    (s.phase + t * wind[0]).sin() * wind[1]
}

/// Sprite `i`'s four corners in its object's frame, in UV-corner order.
pub fn sprite_quad(i: usize, s: &LeafSprite, wind: [f32; 2], v: &LeafAxes, t: f32) -> [Vec3; 4] {
    let back = -v.look;
    let sign = if i % 2 == 1 { -1.0 } else { 1.0 };
    let right = (v.right + back * (sign * TWIST * (i % 5) as f32)).normalize_or_zero();
    let c = s.pos + back * PUSH;
    let w = right * s.size;
    let u = v.up * s.size;
    let tl = c + u * 0.5 - w * 0.5 + w * sway(s, wind, t);
    let bl = c - u * 0.5 - w * 0.5;
    [tl, tl + w, bl, bl + w]
}

/// An object's alpha factor at depth `d` along the look axis, `None` when it
/// is not drawn (see the module doc). `near` is NearSceneRange (b, c).
pub fn object_alpha(
    d: f32,
    distance: f32,
    bound: f32,
    vine: bool,
    near: Option<(f32, f32)>,
) -> Option<f32> {
    if !(-bound <= d && d <= distance) {
        return None;
    }
    let mut a = ((distance - d) / (FADE * distance)).min(1.0);
    if d < NEAR && !vine {
        a = ((d / NEAR) * (d / NEAR)).max(0.1);
    }
    if let Some((b, c)) = near {
        if d > b {
            a *= 1.0 - (d - b) / ((c + bound) - b);
            if a <= 0.0 {
                return None;
            }
        }
    }
    Some(a)
}

/// Append the frame's leaves for `camera` at `t` seconds to `out` (after
/// its grass): each drawn class's sprite buffer once, then the visible
/// objects far to near, in runs.
pub(crate) fn build(
    out: &mut GrassFrame,
    classes: &[LeafClass],
    objects: &[LeafObject],
    near: Option<(f32, f32)>,
    camera: &Camera,
    t: f32,
    view: crate::grass::FoliageView<'_>,
) {
    let v = LeafAxes::of(camera);
    let mut vis: Vec<(f32, usize, f32)> = objects
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let class = classes.get(o.class as usize)?;
            let at = o.matrix.w_axis.truncate();
            let d = (at - camera.position).dot(v.look);
            let alpha = object_alpha(d, o.distance, class.bound, class.vine, near)?;
            view.may_show(at, class.bound).then_some((d, i, alpha))
        })
        .collect();
    if vis.is_empty() {
        return;
    }
    vis.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut ranges: Vec<Option<std::ops::Range<u32>>> = vec![None; classes.len()];
    for &(_, i, _) in &vis {
        let c = objects[i].class as usize;
        if ranges[c].is_some() {
            continue;
        }
        let class = &classes[c];
        let mut order: Vec<usize> = (0..class.sprites.len()).collect();
        // Far to near along the look axis, in template space (the PS2's
        // heap on −(p · back)).
        order.sort_by(|&a, &b| {
            let (da, db) = (
                class.sprites[a].pos.dot(v.look),
                class.sprites[b].pos.dot(v.look),
            );
            db.total_cmp(&da)
        });
        let first = out.vertices.len() as u32;
        for k in order {
            let s = &class.sprites[k];
            let q = sprite_quad(k, s, class.wind, &v, t);
            let uv = uv(class.whole_sheet, s.part);
            for corner in [0, 1, 2, 1, 3, 2] {
                out.vertices.push(GrassVertex {
                    position: q[corner].to_array(),
                    uv: uv[corner],
                    color: [s.grey, s.grey, s.grey, class.alpha],
                    up: if corner < 2 { 1.0 } else { -1.0 },
                });
            }
        }
        ranges[c] = Some(first..out.vertices.len() as u32);
    }
    let mut last: Option<usize> = None;
    for &(_, i, a) in &vis {
        let o = &objects[i];
        let c = o.class as usize;
        let slot = out.instances.len() as u32;
        let mut color = o.color;
        color[3] *= a;
        out.instances.push(GrassInstance {
            model: o.matrix.to_cols_array_2d(),
            color,
            params: [if classes[c].lit { 1.0 } else { 0.0 }, 1.0, 0.0, 0.0],
        });
        match (last, out.runs.last_mut()) {
            (Some(l), Some(r)) if l == c => r.instances.end = slot + 1,
            _ => out.runs.push(GrassRun {
                texture: classes[c].texture,
                vertices: ranges[c].clone().unwrap(),
                instances: slot..slot + 1,
            }),
        }
        last = Some(c);
    }
}

/// The renderer's leaves: the classes and objects of the loaded scene.
#[derive(Default)]
pub(crate) struct LeafState {
    pub classes: Vec<LeafClass>,
    pub objects: Vec<LeafObject>,
    /// NearSceneRange (b, c), when the map authors one.
    pub near: Option<(f32, f32)>,
}

impl Renderer {
    /// Replace the scene's leaves (none until this is called). Every
    /// object's `class` indexes `classes`; a texture handle must be this
    /// renderer's. `near` is the map's NearSceneRange (b, c).
    pub fn set_leaves(
        &mut self,
        classes: Vec<LeafClass>,
        objects: Vec<LeafObject>,
        near: Option<(f32, f32)>,
    ) {
        self.leaves = LeafState {
            classes,
            objects,
            near,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite() -> LeafSprite {
        LeafSprite {
            pos: Vec3::new(1.0, 2.0, 0.0),
            size: 2.0,
            grey: 0.5,
            phase: 0.0,
            part: 0,
        }
    }

    /// A level camera looking down −Z: sprite 0 is a square of side `size`
    /// facing it, centred on its point pushed 0.1 toward the camera; sprite
    /// 3 turns its right by −0.9 toward the back axis.
    #[test]
    fn a_sprite_is_a_centred_square_facing_the_camera() {
        let v = LeafAxes {
            right: Vec3::X,
            up: Vec3::Y,
            look: Vec3::NEG_Z,
        };
        let s = sprite();
        let q = sprite_quad(0, &s, [1.0, 0.0], &v, 0.0);
        let c = s.pos + Vec3::Z * 0.1;
        assert!(
            q[0].abs_diff_eq(c + Vec3::new(-1.0, 1.0, 0.0), 1e-6),
            "{q:?}"
        );
        assert!(q[1].abs_diff_eq(c + Vec3::new(1.0, 1.0, 0.0), 1e-6));
        assert!(q[2].abs_diff_eq(c + Vec3::new(-1.0, -1.0, 0.0), 1e-6));
        assert!(q[3].abs_diff_eq(c + Vec3::new(1.0, -1.0, 0.0), 1e-6));
        let q = sprite_quad(3, &s, [1.0, 0.0], &v, 0.0);
        let w = (q[3] - q[2]).normalize();
        assert!(
            w.abs_diff_eq((Vec3::X - Vec3::Z * 0.9).normalize(), 1e-6),
            "{w}"
        );
        // The sway shears the top edge only.
        let mut s = s;
        s.phase = std::f32::consts::FRAC_PI_2;
        let q = sprite_quad(0, &s, [1.0, 0.25], &v, 0.0);
        assert!(
            q[0].abs_diff_eq(c + Vec3::new(-0.5, 1.0, 0.0), 1e-6),
            "{q:?}"
        );
        assert!(q[2].abs_diff_eq(c + Vec3::new(-1.0, -1.0, 0.0), 1e-6));
    }

    /// The UV tables: part 0 of the whole sheet is the identity, part 1 the
    /// sheet turned; the quarters go TL, TR, BL, BR.
    #[test]
    fn the_uv_tables_are_the_elf_s() {
        let e = 2047.0 / 2048.0;
        assert_eq!(uv(true, 0), [[0.0, 0.0], [e, 0.0], [0.0, e], [e, e]]);
        assert_eq!(uv(true, 1), [[e, 0.0], [e, e], [0.0, 0.0], [0.0, e]]);
        let h = 1023.0 / 2048.0;
        assert_eq!(uv(false, 3), [[h, h], [e, h], [h, e], [e, e]]);
    }

    /// Drawn from −bound to D; the last 40 % fades; nearer than 6 m the
    /// square ramp (not for a vine); past NearSceneRange b the far fade.
    #[test]
    fn an_object_fades_as_the_ps2_s() {
        assert_eq!(object_alpha(-13.0, 100.0, 12.0, false, None), None);
        assert_eq!(object_alpha(50.0, 100.0, 12.0, false, None), Some(1.0));
        assert_eq!(object_alpha(80.0, 100.0, 12.0, false, None), Some(0.5));
        assert_eq!(object_alpha(100.5, 100.0, 12.0, false, None), None);
        assert_eq!(object_alpha(3.0, 100.0, 12.0, false, None), Some(0.25));
        assert_eq!(object_alpha(0.0, 100.0, 12.0, false, None), Some(0.1));
        assert_eq!(object_alpha(3.0, 100.0, 12.0, true, None), Some(1.0));
        let a = object_alpha(60.0, 1000.0, 10.0, false, Some((50.0, 100.0))).unwrap();
        assert!((a - (1.0 - 10.0 / 60.0)).abs() < 1e-6, "{a}");
        assert_eq!(
            object_alpha(111.0, 1000.0, 10.0, false, Some((50.0, 100.0))),
            None
        );
    }
}
