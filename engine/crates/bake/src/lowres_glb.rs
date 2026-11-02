//! `tern -> PCHS -> LOWR` -> `lowres_<name>.glb`: the far-LOD terrain sheet,
//! the ONLY terrain either PS2 draws past the near scene
//! (analysis/re-lowr.md, lead-verified; `mungefs::terrain::LowResSheet` is the
//! parse).
//!
//! Sixteen of the 32 shipped terrains carry a sheet. It is a coarse
//! triangle-strip LOD of the terrain's OWN footprint — not an outside fill —
//! so the PS2 bake is a transcription: the patches' triangles, one flat texture,
//! no lightmap and no normals worth the name. Donor sheets retain their own
//! texture but use the complete corrected terrain surface, avoiding LOD holes
//! and a distant slope that differs from the ground the simulation reads.
//!
//! ## What is the PS2's here, and what is ours
//!
//! * **Geometry** [V]: the patches as they sit on the disc, dequantised over
//!   the sheet box by `mungefs::terrain`. Nothing is welded, dropped or
//!   re-ordered.
//! * **Texture** [V]: `DomeInfo.LowResTerrain.Texture`, which the manifest
//!   already records as `ManifestSky.low_res_terrain`.
//! * **UV = the sheet's own 0..1 footprint times `TextureScale`**
//!   ([D], upgraded from the brief's [H] by the art itself): the VU packs
//!   `x_quantised * k` with `k = extent/65536 * S * 2048 / gridUnit /
//!   gridSize` (re-lowr.md §1), which is `S` times a 0..1 sweep across the
//!   sheet up to the GS's own UV fixed-point unit — and the unit is what the
//!   VU1 program would settle. The ART settles it instead: the two maps that
//!   author a big `TextureScale` (BES1/BES2 20, END1/YAV1 19) ship a TILING
//!   detail sheet, and every map that authors none (so S = 1) ships an AERIAL
//!   VIEW OF ITS OWN TERRAIN — `tat1` is Tatooine's dunes and its settlement,
//!   `hoth` is Hoth's outpost, both unmistakable. One repeat per sheet is the
//!   only reading that puts that photograph over the ground it is a
//!   photograph of. (`engine/artifacts/lead_tmp/lowr-sheet/tex_*.png`.)
//! * **Orientation** [D]: `u` grows with world +X and `v` with world +Z, no
//!   flip — the same convention `terrain_glb` and `water_glb` use. Measured on
//!   TAT1 by correlating the sheet texture's luminance with a hillshade of the
//!   heightfield under it: the as-stored orientation is the only one of the
//!   four that correlates coherently (r = −0.25 with the sun at azimuth 90
//!   through +0.23 at 270 — the antipodal swing a real match makes), while
//!   the v-flip, the u-flip and the transpose sit inside ±0.1 at every
//!   azimuth (`lead_tmp/lowr-sheet/uv_orient.py`).
//! * **The material says OPAQUE; the draw alpha-tests at the sheet's own
//!   AREF.** The PS2 alpha-tests the sheet at AREF 0x10 (re-lowr.md §2.2,
//!   TEST 0x5010b), and two of the 14 drawn sheets carry real alpha on the
//!   DISC: BF1 KAS1 `kashyyyk1` (62,571 of 65,536 mip-0 texels below 16) and
//!   BF1 KAS2 `kas2` (27,482); the other twelve are 255 everywhere
//!   (`lead_tmp/cln5-lowr/sheet_alpha_disc.txt`, queue CLN5 — the earlier
//!   "all 255" census read the BAKED files, which the bake had forced
//!   opaque). So `bake::map` keeps the sheet texture's alpha and the
//!   renderer's far-sheet pass (`render::DrawLayer::FarSheet`) tests it at
//!   the sheet's AREF, not the world MASK cutoff (193/255, 12x higher,
//!   which would punch holes the PS2 does not). The GLB material stays
//!   OPAQUE: the game routes the sheet by what it is, not by `alphaMode`.
//! * **No lightmap and a constant normal** [V for the first]: the sheet is
//!   drawn with base colour 0x80808080 — GS unity, unlit — and the loader
//!   generates UVs only, no colour and no normal (re-lowr.md §1/§2.2). The
//!   +Y normal below exists so the GLB validates and so the renderer's world
//!   vertex layout fits; nothing may light by it.

use anyhow::Result;
use mungefs::terrain::LowResSheet;
use serde_json::json;

use crate::gltf_bin::{flat3, min_max3, Bin};

/// The normal every sheet vertex gets — see the module header: the draw is
/// unlit and the disc stores none.
const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// `LowResTerrain.TextureScale` for a map whose sky authors none. Both games
/// reset the field to 1.0 at every `tern` load (re-lowr.md §2.4 B1:545886,
/// §3 B2:703199), so an unauthored sheet is one repeat.
pub const DEFAULT_TEXTURE_SCALE: f32 = 1.0;

/// Build the sheet GLB. `half` is `gridUnit * gridSize / 2` — the sheet box's
/// west/north edge, which is what the UVs are measured from. Returns `None`
/// when the sheet has no triangles, which no shipped one lacks.
pub fn lowres_to_glb(
    name: &str,
    sheet: &LowResSheet,
    half: f32,
    texture: Option<&str>,
    texture_scale: f32,
) -> Result<Option<Vec<u8>>> {
    lowres_to_glb_with_surface(name, sheet, half, texture, texture_scale, None)
}

/// Donor distant terrain follows the same corrected surface as near terrain.
/// The PS2 path passes None and retains its original coarse sheet byte for byte.
pub fn lowres_to_glb_with_surface(
    name: &str,
    sheet: &LowResSheet,
    half: f32,
    texture: Option<&str>,
    texture_scale: f32,
    surface: Option<&crate::terrain_glb::AlignedSurface>,
) -> Result<Option<Vec<u8>>> {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(sheet.vertex_count());
    let mut indices: Vec<u32> = Vec::with_capacity(sheet.triangle_count() * 3);
    if let Some(surface) = surface {
        (positions, indices) = surface.positions_indices();
    } else {
        for p in &sheet.patches {
            // Each patch keeps its own vertices: they are the disc's, and two
            // patches that meet share an edge in space but not in the file.
            let base = positions.len() as u32;
            positions.extend_from_slice(&p.positions);
            for t in &p.triangles {
                indices.extend_from_slice(&[base + t[0], base + t[1], base + t[2]]);
            }
        }
    }
    if indices.is_empty() {
        return Ok(None);
    }

    // `extent` rather than `2 * half` because they are the same number on all
    // 16 shipped sheets but the box the vertices came out of is the extent's.
    let span = if sheet.extent.is_finite() && sheet.extent > 0.0 {
        sheet.extent
    } else {
        2.0 * half
    };
    let scale = if texture_scale.is_finite() && texture_scale > 0.0 {
        texture_scale
    } else {
        DEFAULT_TEXTURE_SCALE
    };

    let mut bin = Bin::default();
    let (min, max) = min_max3(&positions);
    let pos = bin.f32s(&flat3(&positions), "VEC3", json!({"min": min, "max": max}));
    let up: Vec<f32> = positions.iter().flat_map(|_| UP).collect();
    let nrm = bin.f32s(&up, "VEC3", json!({}));
    let uvs: Vec<f32> = positions
        .iter()
        .flat_map(|p| [(p[0] + half) / span * scale, (p[2] + half) / span * scale])
        .collect();
    let uv = bin.f32s(&uvs, "VEC2", json!({}));
    let idx = bin.u32s(&indices, json!({}));

    crate::gltf_bin::rigid_glb(
        bin,
        json!({"name": name, "primitives": [{
            "attributes": {"POSITION":pos,"NORMAL":nrm,"TEXCOORD_0":uv},
            "indices": idx,
            "material": 0,
            "mode": 4
        }]}),
        name,
        Some(json!([{"name": texture.unwrap_or_default()}])),
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::terrain::LowResPatch;

    /// A 100 m sheet of one two-triangle patch spanning its whole box.
    fn sheet(scale_extent: f32) -> LowResSheet {
        LowResSheet {
            grid: 1,
            resolution: 5,
            extent: scale_extent,
            y_min: -1.0,
            y_max: 1.0,
            patches: vec![LowResPatch {
                positions: vec![
                    [-50.0, 0.0, -50.0],
                    [50.0, 0.0, -50.0],
                    [-50.0, 0.0, 50.0],
                    [50.0, 0.0, 50.0],
                ],
                triangles: vec![[0, 1, 2], [1, 3, 2]],
            }],
        }
    }

    fn read_back(bytes: &[u8]) -> (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>) {
        let (doc, buffers, _) = gltf::import_slice(bytes).expect("gltf reads it");
        let mesh = doc.meshes().next().expect("one mesh");
        let prim = mesh.primitives().next().expect("one primitive");
        let r = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
        (
            r.read_positions().expect("positions").collect(),
            r.read_tex_coords(0).expect("uvs").into_f32().collect(),
            r.read_indices().expect("indices").into_u32().collect(),
        )
    }

    /// The sheet's UVs sweep 0..`TextureScale` across the footprint, `u` with
    /// +X and `v` with +Z (module header): one repeat for a map that authors
    /// no scale, `S` for one that does.
    #[test]
    fn uvs_sweep_texture_scale_across_the_sheet() {
        for (scale, edge) in [(1.0f32, 1.0f32), (19.0, 19.0)] {
            let glb = lowres_to_glb("s", &sheet(100.0), 50.0, Some("sand"), scale)
                .unwrap()
                .expect("triangles");
            let (pos, uv, idx) = read_back(&glb);
            assert_eq!(idx, vec![0, 1, 2, 1, 3, 2]);
            assert_eq!(pos.len(), 4);
            // (-50,-50) -> (0,0) and (+50,+50) -> (edge, edge).
            assert_eq!(uv[0], [0.0, 0.0]);
            assert_eq!(uv[1], [edge, 0.0]);
            assert_eq!(uv[2], [0.0, edge]);
            assert_eq!(uv[3], [edge, edge]);
        }
    }

    /// A sheet whose patches hold no triangles bakes no file at all, rather
    /// than an empty mesh the renderer would have to special-case.
    #[test]
    fn an_empty_sheet_bakes_nothing() {
        let mut s = sheet(100.0);
        s.patches[0].triangles.clear();
        assert!(lowres_to_glb("s", &s, 50.0, None, 1.0).unwrap().is_none());
    }

    /// A map that authors a nonsense scale falls back to one repeat instead of
    /// writing NaN UVs into the GLB.
    #[test]
    fn a_broken_texture_scale_falls_back_to_one_repeat() {
        let glb = lowres_to_glb("s", &sheet(100.0), 50.0, None, f32::NAN)
            .unwrap()
            .expect("triangles");
        let (_, uv, _) = read_back(&glb);
        assert_eq!(uv[3], [1.0, 1.0]);
    }
}
