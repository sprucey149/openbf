//! BF2's omni lights on the terrain (card FXBL, `analysis/re-bolt.md` §5.3,
//! [O]): the gather that hands [`render::TerrainLights`] its lights and, per
//! PS2 terrain patch, the ≤ 3 lights the patch draws.
//!
//! - **f** = max(0, 1 − dy²/r²), dy = the light's height over the
//!   heightfield at its (x, z) (B2 `FUN_0055ebc0`, [`Heightfield::height_at`]);
//!   the colour handed on is the light's bytes/255 × f.
//! - **Which patches** (B2 `FUN_00599040`, 730351–730416): a patch takes a
//!   light when the squared distance from the light to the patch's box is ≤
//!   r²; it ranks them by (0.3R + 0.45G + 0.25B)·min(1, r²/4d²)/max(1,
//!   e²/3600), e the light's distance to the eye, keeps the top 4 and the
//!   terrain draws the first 3. A light whose disc reaches past the patches
//!   within r is cut at their edge, as on the PS2.
//! - **Patches**: the disc's 8-cell `PCHS` patches on the heightfield's grid;
//!   a patch's box spans its 9×9 vertices' heights.
//!
//! Render-only: it reads the heightfield and the lights, never the sim.

use core::fmath::FloatExt;
use core::heightfield::Heightfield;
use glam::Vec3;
use render::{
    LitPatch, TerrainLight, TerrainLights, LIGHTS_PER_PATCH, MAX_LIT_PATCHES, MAX_TERRAIN_LIGHTS,
};

/// Cells along one edge of a PS2 terrain patch.
pub const PATCH_CELLS: u32 = 8;
/// The gather keeps the top 4 lights per patch (§5.3).
const GATHER: usize = 4;

/// A patch's (x, z) index and its lights as (index, rank).
type PatchLights = ((u32, u32), Vec<(usize, f32)>);

/// One omni light: where, its colour bytes, its range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OmniLight {
    pub pos: Vec3,
    pub rgb: [u8; 3],
    pub radius: f32,
}

/// f = max(0, 1 − dy²/r²) for a light `dy` metres over the ground.
pub fn height_factor(dy: f32, r: f32) -> f32 {
    if r <= 0.0 {
        return 0.0;
    }
    (1.0 - dy * dy / (r * r)).fmax(0.0)
}

/// The light's rank on a patch `d2` (squared) metres away, `e2` (squared)
/// from the eye.
fn rank(l: &OmniLight, d2: f32, e2: f32) -> f32 {
    let [r, g, b] = l.rgb.map(|c| f32::from(c) / 255.0);
    let r2 = l.radius * l.radius;
    let mut s = 0.3 * r + 0.45 * g + 0.25 * b;
    if r2 < 4.0 * d2 {
        s *= r2 / (4.0 * d2);
    }
    s / (e2 / 3600.0).fmax(1.0)
}

/// The patch (px, pz)'s box: min and max corners.
fn patch_box(hf: &Heightfield, px: u32, pz: u32) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for iz in pz * PATCH_CELLS..=((pz + 1) * PATCH_CELLS).min(hf.size - 1) {
        for ix in px * PATCH_CELLS..=((px + 1) * PATCH_CELLS).min(hf.size - 1) {
            let h = hf.heights[(iz * hf.size + ix) as usize];
            lo = lo.fmin(h);
            hi = hi.fmax(h);
        }
    }
    let side = PATCH_CELLS as f32 * hf.unit;
    let x0 = px as f32 * side - hf.offset_x;
    let z0 = pz as f32 * side - hf.offset_z;
    (Vec3::new(x0, lo, z0), Vec3::new(x0 + side, hi, z0 + side))
}

/// This frame's terrain lights for the eye at `eye`: every patch within a
/// light's range, its top lights, the lights' colours × f.
pub fn gather(lights: &[OmniLight], hf: &Heightfield, eye: Vec3) -> TerrainLights {
    let mut order: Vec<usize> = (0..lights.len())
        .filter(|&i| lights[i].radius > 0.0)
        .collect();
    order.sort_by(|&a, &b| {
        lights[a]
            .pos
            .distance_squared(eye)
            .total_cmp(&lights[b].pos.distance_squared(eye))
    });
    order.truncate(MAX_TERRAIN_LIGHTS);
    let per_side = (hf.size.saturating_sub(1)) / PATCH_CELLS;
    if per_side == 0 {
        return TerrainLights::default();
    }
    let side = PATCH_CELLS as f32 * hf.unit;
    let cell = |v: f32, off: f32| ((v + off) / side).floor();
    // (patch, light, rank), then per patch its top lights.
    let mut hits: Vec<((u32, u32), usize, f32)> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let l = &lights[i];
        let r = l.radius;
        let span = |v: f32, off: f32| {
            let lo = cell(v - r, off).fmax(0.0) as u32;
            let hi = (cell(v + r, off).fmax(0.0) as u32).min(per_side - 1);
            lo..=hi
        };
        let e2 = l.pos.distance_squared(eye);
        for pz in span(l.pos.z, hf.offset_z) {
            for px in span(l.pos.x, hf.offset_x) {
                let (lo, hi) = patch_box(hf, px, pz);
                let d2 = l.pos.distance_squared(l.pos.clamp(lo, hi));
                if d2 <= r * r {
                    hits.push(((px, pz), k, rank(l, d2, e2)));
                }
            }
        }
    }
    // Per patch in first-touch order, its lights best first (stable: the
    // list order breaks ties).
    let mut patches: Vec<PatchLights> = Vec::new();
    for (p, k, s) in hits {
        match patches.iter_mut().find(|(q, _)| *q == p) {
            Some((_, v)) => v.push((k, s)),
            None => patches.push((p, vec![(k, s)])),
        }
    }
    let mut out = TerrainLights {
        lights: order
            .iter()
            .map(|&i| {
                let l = &lights[i];
                let f = height_factor(l.pos.y - hf.height_at(l.pos.x, l.pos.z), l.radius);
                TerrainLight {
                    pos: l.pos,
                    rgb: l.rgb.map(|c| f32::from(c) / 255.0 * f),
                }
            })
            .collect(),
        patches: Vec::new(),
    };
    for ((px, pz), mut v) in patches.into_iter().take(MAX_LIT_PATCHES) {
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v.truncate(GATHER.min(LIGHTS_PER_PATCH));
        let mut ids = [0u32; LIGHTS_PER_PATCH];
        for (slot, (k, _)) in ids.iter_mut().zip(&v) {
            *slot = *k as u32;
        }
        let (lo, hi) = patch_box(hf, px, pz);
        out.patches.push(LitPatch {
            min: [lo.x, lo.z],
            max: [hi.x, hi.z],
            lights: ids,
            count: v.len() as u32,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat 33×33 grid of 1 m cells centred on the origin (4×4 patches of
    /// 8 m): a rifle light (148, 25, 25), r 4, 1 m up at (1, 1, 1) lies in
    /// patch (2, 2) = [0, 8]² and reaches patches (1, 1), (1, 2), (2, 1)
    /// within 1 m; f = 1 − 1/16. Five lights on one spot: the patch keeps
    /// three, the brightest first.
    #[test]
    fn gather_patches_and_rank() {
        let hf = Heightfield {
            size: 33,
            unit: 1.0,
            offset_x: 16.0,
            offset_z: 16.0,
            heights: vec![0.0; 33 * 33],
        };
        let rifle = OmniLight {
            pos: Vec3::new(1.0, 1.0, 1.0),
            rgb: [148, 25, 25],
            radius: 4.0,
        };
        let t = gather(&[rifle], &hf, Vec3::ZERO);
        assert_eq!(t.lights.len(), 1);
        let f = 1.0 - 1.0 / 16.0;
        assert!((t.lights[0].rgb[0] - 148.0 / 255.0 * f).abs() < 1e-6);
        let mut rects: Vec<[f32; 2]> = t.patches.iter().map(|p| p.min).collect();
        rects.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            rects,
            vec![[-8.0, -8.0], [-8.0, 0.0], [0.0, -8.0], [0.0, 0.0]]
        );
        assert!(t
            .patches
            .iter()
            .all(|p| p.count == 1 && p.max[0] - p.min[0] == 8.0));

        let dim = |c: u8| OmniLight {
            pos: Vec3::new(4.0, 1.0, 4.0),
            rgb: [c, c, c],
            radius: 2.0,
        };
        let t = gather(
            &[dim(10), dim(50), dim(30), dim(40), dim(20)],
            &hf,
            Vec3::ZERO,
        );
        assert_eq!(t.patches.len(), 1);
        let p = t.patches[0];
        assert_eq!(p.count, 3);
        let c = |k: u32| t.lights[k as usize].rgb[0];
        assert!(c(p.lights[0]) > c(p.lights[1]) && c(p.lights[1]) > c(p.lights[2]));
        assert!((c(p.lights[2]) - 30.0 / 255.0 * (1.0 - 1.0 / 4.0)).abs() < 1e-6);
    }

    /// Above r nothing: f = 0 at dy = r; a light far from the eye ranks
    /// below a dimmer near one (e²/3600).
    #[test]
    fn height_factor_and_eye_rank() {
        assert_eq!(height_factor(4.0, 4.0), 0.0);
        assert!((height_factor(2.0, 4.0) - 0.75).abs() < 1e-6);
        let l = |c: u8| OmniLight {
            pos: Vec3::ZERO,
            rgb: [c; 3],
            radius: 4.0,
        };
        assert!(rank(&l(100), 0.0, 120.0 * 120.0) < rank(&l(30), 0.0, 10.0));
    }
}
