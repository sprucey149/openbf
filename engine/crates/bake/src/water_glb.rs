//! `tern -> WATR` → a water-surface GLB (see `mungefs::water` for the format).
//!
//! Twelve of the 34 ground maps ship water and none drew it. The surface is
//! flat — every shipped `CPAT` vertex confirms the plane — so this emits one
//! horizontal mesh at `LAYR.height`:
//!
//! - a shared `(grid+1)²` vertex lattice over the whole sheet, with two
//!   triangles emitted per WET `WMAP` cell (shared vertices, so no seams and
//!   no duplicated verts along a shoreline);
//! - plus the `CPAT` patches' own triangles, which are the shoreline-fitted
//!   surface for the handful of tiles that have one.
//!
//! **VERIFIED, and it is the PS2's: a `CPAT` patch replaces the flat quad of
//! its OWN cell — the one whose `WMAP` nibble sets bit 3 — and of no other**
//! (`analysis/re-water.md` §4, [V]; the cull re-read 2026-09-17 in BF1
//! `FUN_004770a8` 539306-539324 and BF2 `FUN_00561b58` 696884-696892): the
//! per-frame cull hangs the patch packet on the tile record's `+0x24` only
//! when `z * grid + x` is equal, and the flat-water draw takes the patch iff
//! that pointer is set (§4's CORRECTION carries the two refinements a 2026-09-17
//! verifier pass found: the ELSE branch has a middle case, and the OCEAN path
//! never reads `+0x24` at all — moot here, since no shipped ocean map ships a
//! `CPAT`). Drawing both would put two surfaces at one height, a z-fight over
//! Theed's canals.
//!
//! This bake used to drop every wet cell whose CENTRE fell inside any patch's
//! AABB, and an `LPTC INFO` box is its cell padded by half a cell on every
//! side — its edges run through the eight neighbours' centres, so the rule
//! took the whole 3x3 block: **17 / 32 / 2 cells where the disc marks
//! 4 / 8 / 1** (BF1 NAB2, BF2 NAB2, BF2 DEA1, which drew no lattice cell at
//! all), holes in the water beside every patch. Fixed 2026-09-17, pinned by
//! `bake/tests/suite/water_patch_cells.rs`. A patch's own triangles cover 13-96 %
//! of the cell they replace (measured there) — the "3-24 %" in
//! `mungefs::water`'s header is that area over the twice-as-wide box.
//!
//! ## The texture and the UV scale are the PS2's since 2026-09-09
//!
//! Both come from the LVL's `fx__ Effect("Water")` ([`mungefs::fx`],
//! `re-water.md` §3, [V]) rather than from `WATR -> LAYR`, whose copies both
//! ELFs read and drop:
//!
//! 1. **`MainTexture` paints the surface.** All twelve water maps author one.
//!    On both KAM1s it is `kam1_water_2` where `LAYR` names `kam1_water` —
//!    the FOAM texture, which is what the port used to paint the sea with.
//! 2. **Metres per texture repeat = `cell_size / (Tile · INFO5)`**, where
//!    `INFO5` is `WATR -> INFO`'s sixth word (4 on both Kashyyyk Docks, 1 on
//!    the other ten). This moves nine of the twelve maps; BF1 YAV1, BF2 YAV1
//!    and BF2 FEL1 land on the same number the old `LAYR` rule did.
//! 3. **Scroll = `Velocity / INFO5`**, recorded in the manifest for the
//!    renderer to apply. Its SIGN is PROVISIONAL — see
//!    [`mungefs::fx::WaterConfig::scroll_per_second`].
//!
//! ## What is still ours, and still PROVISIONAL
//!
//! 1. **The blend is the runtime's, not this GLB's.** The tint and alpha come
//!    from the Min → Max diffuse table and the sheet's own alpha (W2, WA;
//!    `game::viewer::look::water_look`). Flat sheets without CPAT now carry
//!    diffuse LUT RGB on the authored PatchDivisions * INFO5 lattice.
//!    `LAYR.color` is NOT applied and never will be — both ELFs drop it, so
//!    the byte order that fit no map's art is moot.
//! 2. The diffuse template's random realization remains provisional;
//!    `re-water-colour.md` §1.5 establishes the 11 LUT entries and edge wrap.
//!    CPAT sheets keep their original triangles and mean tint. Far water
//!    still uses the near lattice rather than the original reduced template.
//! 3. **Texture fallbacks**, for data neither disc ships: a map with no
//!    `MainTexture` falls back to `LAYR`'s name, and one that names neither
//!    falls back to its single texture ending in `_water`. Both print when
//!    they fire, and on the shipped twelve neither does.

use anyhow::Result;
use mungefs::fx::WaterConfig;
use mungefs::water::Water;
use serde_json::json;

use manifest::ManifestWaterConfig;

use crate::gltf_bin::{flat3, min_max3, Bin};

/// The normal of a flat water plane. Every shipped `CPAT` vertex lies on it.
const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// Which texture paints this water: `fx__`'s `MainTexture`, else `LAYR`'s
/// name, else the map's single `*_water`. On the twelve shipped maps the
/// first arm always wins; the other two print when they fire.
pub fn resolve_texture(
    w: &Water,
    cfg: Option<&WaterConfig>,
    textures: &[String],
) -> Option<String> {
    let find = |n: &str| textures.iter().find(|t| t.eq_ignore_ascii_case(n)).cloned();
    if let Some(main) = cfg.and_then(|c| c.main_texture.as_deref()) {
        return find(main);
    }
    if !w.layer.texture.is_empty() {
        eprintln!(
            "water: no fx__ MainTexture, falling back to LAYR's {:?}",
            w.layer.texture
        );
        return find(&w.layer.texture);
    }
    let mut ends: Vec<&String> = textures
        .iter()
        .filter(|t| t.to_ascii_lowercase().ends_with("_water"))
        .collect();
    match ends.len() {
        1 => {
            eprintln!("water: no fx__ MainTexture and no LAYR name, taking {ends:?}");
            Some(ends.remove(0).clone())
        }
        _ => None,
    }
}

/// Metres of world per texture repeat, per axis: `cell / (Tile * INFO5)` from
/// `fx__` (module header item 2, [V]), else the pre-`fx__` `LAYR` rule for a
/// map that authors no `Tile` — which neither disc ships.
fn metres_per_repeat(w: &Water, cfg: Option<&WaterConfig>) -> [f32; 2] {
    if let Some(m) = cfg.and_then(|c| c.metres_per_repeat(w.info.cell_size, w.info.unknown_5)) {
        return m;
    }
    let tile = w.layer.uv_tile[0];
    let fallback = if tile.is_finite() && tile > 0.0 {
        w.info.cell_size / tile
    } else {
        w.info.cell_size
    };
    [fallback, fallback]
}

/// What the map's `fx__ Effect("Water")` authored, in the manifest's shape.
/// Every field is carried across as it stands; `metres_per_repeat` is the one
/// derived value, and it is the number this module built `glb`'s UVs with.
pub fn manifest_config(c: &WaterConfig, w: &Water) -> ManifestWaterConfig {
    let sp = c.speckle.clone().unwrap_or_default();
    ManifestWaterConfig {
        normal_maps: c.normal_maps.as_ref().and_then(|n| {
            let fps = n.fps?;
            let refraction = c.refraction_color?;
            let reflection = c.reflection_color?;
            let fresnel = c.fresnel_min_max?;
            (!n.textures.is_empty()
                && fps.is_finite()
                && fps > 0.0
                && refraction
                    .iter()
                    .chain(&reflection)
                    .chain(&fresnel)
                    .all(|v| v.is_finite()))
            .then(|| manifest::ManifestNormalWater {
                textures: n.textures.clone(),
                fps,
                refraction,
                reflection,
                fresnel,
            })
        }),
        main_texture: c.main_texture.clone(),
        foam_texture: c.foam_texture.clone(),
        metres_per_repeat: c.metres_per_repeat(w.info.cell_size, w.info.unknown_5),
        foam_metres_per_repeat: c
            .foam_tile
            .filter(|t| t.iter().all(|v| v.is_finite() && *v > 0.0))
            .map(|t| [w.info.cell_size / t[0], w.info.cell_size / t[1]]),
        // Authored as a float on every map that has the key; anything but 0
        // is the ocean path (both KAM1s author exactly 1).
        ocean: c.ocean_enable.is_some_and(|v| v != 0.0),
        disable_low_res: c.disable_low_res,
        // Both flags are the byte the PS2 reads: its level reset stores 1 and
        // only an authored 0 clears it (`re-water-flags.md` §2, §7 `[O]`).
        specular_enable: c.specular_enable.is_none_or(|v| v != 0.0),
        oscillation_enable: c.oscillation_enable.is_none_or(|v| v != 0.0),
        min_diffuse: c.min_diffuse_color,
        max_diffuse: c.max_diffuse_color,
        border_diffuse: c.border_diffuse_color,
        specular_color: c.specular_color,
        speckle_textures: sp.textures,
        speckle_fps: sp.fps,
        speckle_tile: sp.tile,
        speckle_scroll: sp.scroll_speed,
        speckle_coord_shift: sp.coord_shift,
        speckle_specular_color: c.speckle_specular_color,
        speckle_ambient_color: c.speckle_ambient_color,
        light_azim_and_elev: c.light_azim_and_elev,
        patch_divisions: c.patch_divisions,
        lod_decimation: c.lod_decimation,
    }
}

/// Triangle count the mesh will hold. Zero when the map's water draws
/// nothing at all (no wet cell survives and no patch has geometry).
pub fn triangle_count(w: &Water) -> usize {
    let g = w.info.grid as usize;
    let cells = (0..g)
        .flat_map(|z| (0..g).map(move |x| (x, z)))
        .filter(|&(x, z)| w.is_wet(x, z) && !w.has_patch(x, z))
        .count();
    cells * 2 + w.patches.iter().map(|p| p.triangles.len()).sum::<usize>()
}

pub fn diffuse_divisions(w: &Water, cfg: Option<&WaterConfig>) -> Option<[usize; 2]> {
    let c = cfg?;
    if c.ocean_enable.is_some_and(|v| v != 0.0) || !w.patches.is_empty() {
        return None;
    }
    c.min_diffuse_color.zip(c.max_diffuse_color)?;
    let divisions = c.patch_divisions?.map(|d| d * w.info.unknown_5 as f32);
    divisions
        .iter()
        .all(|d| d.is_finite() && *d >= 1.0 && *d <= 64.0 && d.fract() == 0.0)
        .then(|| divisions.map(|d| d as usize))
}

pub fn render_triangle_count(w: &Water, cfg: Option<&WaterConfig>) -> usize {
    triangle_count(w) * diffuse_divisions(w, cfg).map_or(1, |[x, z]| x * z)
}

fn diffuse_lattice(
    w: &Water,
    c: &WaterConfig,
    divisions: [usize; 2],
) -> (Vec<[f32; 3]>, Vec<u32>, Vec<f32>) {
    let [nx, nz] = divisions;
    let min = c.min_diffuse_color.unwrap();
    let max = c.max_diffuse_color.unwrap();
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut colors = Vec::new();
    let mut rng = 0x9733_1547u32;
    let mut next = || {
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        rng
    };
    let template: Vec<_> = (0..nx * nz)
        .map(|_| {
            let low = next() >> 16;
            let high = next() & 0xffff_0000;
            next();
            next();
            ((low | high) % 11) as f32 / 5.0
        })
        .collect();
    for z in 0..w.info.grid as usize {
        for x in 0..w.info.grid as usize {
            if !w.is_wet(x, z) {
                continue;
            }
            let base = positions.len() as u32;
            for iz in 0..=nz {
                for ix in 0..=nx {
                    positions.push([
                        (x as f32 + ix as f32 / nx as f32) * w.info.cell_size
                            - w.info.half_extent[0],
                        w.layer.height,
                        (z as f32 + iz as f32 / nz as f32) * w.info.cell_size
                            - w.info.half_extent[1],
                    ]);
                    let t = template[(ix % nx) * nz + iz % nz];
                    colors.extend(
                        (0..3).map(|k| ((min[k] + (max[k] - min[k]) * t) / 255.0).clamp(0.0, 1.0)),
                    );
                    colors.push(1.0);
                }
            }
            for iz in 0..nz {
                for ix in 0..nx {
                    let a = base + (iz * (nx + 1) + ix) as u32;
                    let (b, c, d) = (a + 1, a + (nx + 1) as u32, a + (nx + 2) as u32);
                    indices.extend([a, c, b, b, c, d]);
                }
            }
        }
    }
    (positions, indices, colors)
}

/// Build the water GLB. Returns `None` when there is nothing to draw.
pub fn water_to_glb(
    w: &Water,
    cfg: Option<&WaterConfig>,
    name: &str,
    texture: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    let g = w.info.grid as usize;
    let y = w.layer.height;
    let repeat = metres_per_repeat(w, cfg);

    // A shared (grid+1)^2 lattice: vertices are cell CORNERS, so adjacent wet
    // cells share an edge and the shoreline has no doubled vertices.
    let stride = g + 1;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(stride * stride);
    for z in 0..stride {
        for x in 0..stride {
            positions.push([
                x as f32 * w.info.cell_size - w.info.half_extent[0],
                y,
                z as f32 * w.info.cell_size - w.info.half_extent[1],
            ]);
        }
    }
    let mut indices: Vec<u32> = Vec::new();
    for z in 0..g {
        for x in 0..g {
            if !w.is_wet(x, z) || w.has_patch(x, z) {
                continue;
            }
            let a = (z * stride + x) as u32;
            let (b, c, d) = (a + 1, a + stride as u32, a + stride as u32 + 1);
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    // The CPAT patches keep their own vertices — they are not on the lattice.
    for p in &w.patches {
        let base = positions.len() as u32;
        positions.extend_from_slice(&p.positions);
        for t in &p.triangles {
            indices.extend_from_slice(&[base + t[0], base + t[1], base + t[2]]);
        }
    }

    if indices.is_empty() {
        return Ok(None);
    }

    let diffuse = diffuse_divisions(w, cfg).map(|d| diffuse_lattice(w, cfg.unwrap(), d));
    let colors = if let Some((p, i, colors)) = diffuse {
        positions = p;
        indices = i;
        Some(colors)
    } else {
        None
    };
    let mut bin = Bin::default();
    let (min, max) = min_max3(&positions);
    let pos = bin.f32s(&flat3(&positions), "VEC3", json!({"min": min, "max": max}));
    let up: Vec<f32> = positions.iter().flat_map(|_| UP).collect();
    let nrm = bin.f32s(&up, "VEC3", json!({}));
    let uvs: Vec<f32> = positions
        .iter()
        .flat_map(|p| [p[0] / repeat[0], p[2] / repeat[1]])
        .collect();
    let uv = bin.f32s(&uvs, "VEC2", json!({}));
    let idx = bin.u32s(&indices, json!({}));
    let mut attributes = json!({"POSITION":pos,"NORMAL":nrm,"TEXCOORD_0":uv});
    if let Some(colors) = colors {
        attributes["COLOR_0"] = json!(bin.f32s(&colors, "VEC4", json!({})));
    }

    crate::gltf_bin::rigid_glb(
        bin,
        json!({"name": name, "primitives": [{
            "attributes": attributes,
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
    use mungefs::fx::WaterConfig;
    use mungefs::water::{WaterInfo, WaterLayer, WaterPatch};

    /// A config authoring only what a test names — every other key absent,
    /// which is what `None` means throughout `mungefs::fx`.
    fn cfg(main: Option<&str>, tile: Option<[f32; 2]>) -> WaterConfig {
        WaterConfig {
            main_texture: main.map(str::to_owned),
            tile,
            ..WaterConfig::default()
        }
    }

    fn water(grid: u32, wet: &[(usize, usize)], patches: Vec<WaterPatch>) -> Water {
        let mut cells = vec![0u8; (grid * grid) as usize];
        for &(x, z) in wet {
            cells[z * grid as usize + x] = 1;
        }
        Water {
            info: WaterInfo {
                grid,
                cell_size: 16.0,
                half_extent: [grid as f32 * 8.0, grid as f32 * 8.0],
                unknown_1: 2,
                unknown_5: 1,
            },
            layer: WaterLayer {
                texture: String::new(),
                unknown: 10,
                height: -3.0,
                uv_velocity: [0.0, 0.0],
                uv_tile: [2.0, 2.0],
                color: [255, 255, 255, 255],
            },
            cells,
            patches,
        }
    }

    /// `SpecularEnable` and `OscillationEnable` are the bytes the PS2 reads:
    /// the level reset stores 1, so unauthored is ON and only an authored 0
    /// clears one (`re-water-flags.md` §2, §7 `[O]`; BF2 FEL1 authors
    /// `OscillationEnable(0)`).
    #[test]
    fn the_water_flags_default_on_and_an_authored_zero_clears_them() {
        let w = water(4, &[(1, 1)], vec![]);
        let m = manifest_config(&cfg(None, None), &w);
        assert!(m.specular_enable && m.oscillation_enable);
        let off = WaterConfig {
            specular_enable: Some(0.0),
            oscillation_enable: Some(0.0),
            ..WaterConfig::default()
        };
        let m = manifest_config(&off, &w);
        assert!(!m.specular_enable && !m.oscillation_enable);
        let on = WaterConfig {
            specular_enable: Some(1.0),
            oscillation_enable: Some(1.0),
            ..WaterConfig::default()
        };
        let m = manifest_config(&on, &w);
        assert!(m.specular_enable && m.oscillation_enable);
    }

    #[test]
    fn wet_cells_become_two_triangles_each() {
        let w = water(4, &[(1, 1), (2, 1)], vec![]);
        assert_eq!(triangle_count(&w), 4);
        assert!(water_to_glb(&w, None, "w", None).unwrap().is_some());
    }

    #[test]
    fn diffuse_lattice_preserves_wet_footprint_and_periodic_colours() {
        let mut w = water(4, &[(1, 1), (2, 1)], vec![]);
        w.info.unknown_5 = 4;
        let c = WaterConfig {
            patch_divisions: Some([4.0; 2]),
            min_diffuse_color: Some([30.0, 40.0, 50.0, 255.0]),
            max_diffuse_color: Some([70.0, 80.0, 90.0, 255.0]),
            ..WaterConfig::default()
        };
        let divisions = diffuse_divisions(&w, Some(&c)).unwrap();
        assert_eq!(divisions, [16; 2]);
        let (p, indices, colors) = diffuse_lattice(&w, &c, divisions);
        assert_eq!(indices.len() / 3, render_triangle_count(&w, Some(&c)));
        assert_eq!(indices.len() / 3, 1024);
        assert!(p.iter().all(|p| p[1] == -3.0
            && (-16.0..=16.0).contains(&p[0])
            && (-16.0..=0.0).contains(&p[2])));
        for tile in 0..2 {
            let start = tile * 17 * 17;
            for k in 0..17 {
                let color = |i: usize| &colors[i * 4..i * 4 + 4];
                assert_eq!(color(start + k * 17), color(start + k * 17 + 16));
                assert_eq!(color(start + k), color(start + 16 * 17 + k));
            }
        }
        let lo = colors
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| (c[0] * 255.0).round() as u32)
            .min()
            .unwrap();
        let hi = colors
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| (c[0] * 255.0).round() as u32)
            .max()
            .unwrap();
        assert_eq!((lo, hi), (30, 110));
        let native_column: Vec<_> = (0..16)
            .map(|z| (colors[z * 17 * 4] * 255.0).round() as u32)
            .collect();
        assert_eq!(
            native_column,
            [94, 38, 46, 62, 102, 38, 62, 38, 110, 70, 110, 38, 54, 102, 62, 62]
        );
        let mut ocean = c.clone();
        ocean.ocean_enable = Some(1.0);
        assert_eq!(diffuse_divisions(&w, Some(&ocean)), None);
        assert_eq!(
            water_to_glb(&w, Some(&c), "water", None).unwrap(),
            water_to_glb(&w, Some(&c), "water", None).unwrap()
        );
    }

    #[test]
    fn a_map_with_no_wet_cell_emits_nothing() {
        let w = water(4, &[], vec![]);
        assert_eq!(triangle_count(&w), 0);
        assert!(water_to_glb(&w, None, "w", None).unwrap().is_none());
    }

    /// Set `WMAP` bit 3 on a cell — the disc's "an `LPTC` for this cell
    /// follows"; every shipped bit-3 nibble is 9, wet as well.
    fn with_patch_cell(mut w: Water, x: usize, z: usize) -> Water {
        let g = w.info.grid as usize;
        w.cells[z * g + x] |= 9;
        w
    }

    /// The VERIFIED rule (`re-water.md` §4): the bit-3 cell yields its quad
    /// to the patch and NO other cell does, however far the patch's box
    /// reaches. The box here covers both cells' centres — the AABB rule this
    /// replaced would have dropped the neighbour too and left a hole.
    #[test]
    fn only_the_bit_three_cell_yields_to_a_patch() {
        let p = WaterPatch {
            // At cell_size 16 and half-extent 32, cells (1,1) and (2,1)
            // centre on x = -8 and +8, z = -8; this box reaches both.
            aabb: ([-16.0, -3.0, -16.0], [16.0, -2.0, 0.0]),
            positions: vec![[-16.0, -3.0, -16.0], [16.0, -3.0, -16.0], [16.0, -3.0, 0.0]],
            triangles: vec![[0, 1, 2]],
        };
        let w = with_patch_cell(water(4, &[(1, 1), (2, 1)], vec![p]), 1, 1);
        assert!(w.has_patch(1, 1) && !w.has_patch(2, 1));
        assert_eq!(
            triangle_count(&w),
            1 + 2,
            "the patch, plus the neighbour's own quad"
        );
    }

    /// A patch whose box covers nothing but its own cell is the same rule
    /// seen from the other side: one cell yields, and the count is the
    /// patch's own triangles.
    #[test]
    fn a_lone_patch_cell_yields_only_itself() {
        let p = WaterPatch {
            aabb: ([-16.0, -3.0, -16.0], [0.0, -2.0, 0.0]),
            positions: vec![[-16.0, -3.0, -16.0], [0.0, -3.0, -16.0], [0.0, -3.0, 0.0]],
            triangles: vec![[0, 1, 2]],
        };
        let w = with_patch_cell(water(4, &[(1, 1)], vec![p]), 1, 1);
        assert_eq!(triangle_count(&w), 1);
    }

    /// The PS2's UV scale (module header item 2): `Tile` from `fx__` and the
    /// `INFO5` multiplier, NOT `LAYR`'s stale pair. BF1 KAS2's numbers —
    /// `Tile(2,2)` over 128 m cells with `INFO5` 4 — are one repeat every
    /// 16 m, where `LAYR`'s (1,1) would say 128 m.
    #[test]
    fn the_fx_tile_and_info5_set_the_uv_scale() {
        let mut w = water(4, &[(1, 1)], vec![]);
        w.info.cell_size = 128.0;
        w.info.unknown_5 = 4;
        w.layer.uv_tile = [1.0, 1.0];
        let c = cfg(None, Some([2.0, 2.0]));
        assert_eq!(metres_per_repeat(&w, Some(&c)), [16.0, 16.0]);
        // The two axes are read separately even though no disc map differs.
        let c = cfg(None, Some([2.0, 4.0]));
        assert_eq!(metres_per_repeat(&w, Some(&c)), [16.0, 8.0]);
    }

    /// The foam layer's repeat is `cell / FoamTile`, with no `× INFO5`
    /// (the PS2's foam texture matrix, `analysis/re-water.md` §3d): KAM1's
    /// 160 m cell, `Tile(4,4)`, `FoamTile(5,5)`, INFO5 1 → base 40 m, foam
    /// 32 m. INFO5 scales the base only.
    #[test]
    fn the_foam_repeats_every_cell_over_foam_tile() {
        let mut w = water(4, &[(1, 1)], vec![]);
        w.info.cell_size = 160.0;
        let mut c = cfg(None, Some([4.0, 4.0]));
        c.foam_tile = Some([5.0, 5.0]);
        let m = manifest_config(&c, &w);
        assert_eq!(m.metres_per_repeat, Some([40.0, 40.0]));
        assert_eq!(m.foam_metres_per_repeat, Some([32.0, 32.0]));
        w.info.unknown_5 = 4;
        let m = manifest_config(&c, &w);
        assert_eq!(m.metres_per_repeat, Some([10.0, 10.0]));
        assert_eq!(m.foam_metres_per_repeat, Some([32.0, 32.0]));
        // No FoamTile (every map but the KAM1s), or a zero one: none.
        assert_eq!(
            manifest_config(&cfg(None, Some([4.0, 4.0])), &w).foam_metres_per_repeat,
            None
        );
        c.foam_tile = Some([0.0, 5.0]);
        assert_eq!(manifest_config(&c, &w).foam_metres_per_repeat, None);
    }

    /// The pre-`fx__` `LAYR` rule survives only for a map that authors no
    /// `Tile` — which neither disc ships, so it is a guard, not a path.
    #[test]
    fn a_map_without_a_tile_key_keeps_the_layr_rule() {
        let mut w = water(4, &[(1, 1)], vec![]);
        assert_eq!(metres_per_repeat(&w, None), [8.0, 8.0], "uv_tile 2 of 16 m");
        let c = cfg(Some("x"), None);
        assert_eq!(metres_per_repeat(&w, Some(&c)), [8.0, 8.0]);
        // A zero Tile must not divide by zero; it falls back like an absent
        // one rather than producing an infinite UV.
        let c = cfg(None, Some([0.0, 0.0]));
        assert_eq!(metres_per_repeat(&w, Some(&c)), [8.0, 8.0]);
        w.layer.uv_tile = [0.0, 0.0];
        assert_eq!(metres_per_repeat(&w, None), [16.0, 16.0]);
    }

    /// `MainTexture` outranks `LAYR`'s name — the Kamino case, where `LAYR`
    /// names the FOAM texture and the surface is painted with the other one.
    #[test]
    fn the_fx_main_texture_outranks_the_layr_name() {
        let mut w = water(4, &[(1, 1)], vec![]);
        w.layer.texture = "kam1_water".into();
        let baked = vec!["kam1_water".to_string(), "kam1_water_2".to_string()];
        let c = cfg(Some("kam1_water_2"), None);
        assert_eq!(
            resolve_texture(&w, Some(&c), &baked).as_deref(),
            Some("kam1_water_2")
        );
        // Case-insensitively, like every other by-name lookup in the bake.
        let c = cfg(Some("KAM1_Water_2"), None);
        assert_eq!(
            resolve_texture(&w, Some(&c), &baked).as_deref(),
            Some("kam1_water_2")
        );
        // A `MainTexture` the map did not bake resolves to nothing rather
        // than silently falling through to `LAYR`'s name.
        let c = cfg(Some("missing_water"), None);
        assert_eq!(resolve_texture(&w, Some(&c), &baked), None);
    }

    /// The two fallbacks, for data neither disc ships: `LAYR`'s name, then
    /// the single `_water`, and the last one only when it is unambiguous.
    #[test]
    fn texture_fallbacks_fire_only_without_a_main_texture() {
        let mut w = water(4, &[(1, 1)], vec![]);
        let one = vec!["kas1_water".to_string(), "kas1_rock".to_string()];
        assert_eq!(
            resolve_texture(&w, None, &one).as_deref(),
            Some("kas1_water")
        );

        let two = vec![
            "nab2_water".to_string(),
            "nab2_prop_fountain_water".to_string(),
        ];
        assert_eq!(
            resolve_texture(&w, None, &two),
            None,
            "ambiguous: take neither"
        );

        // A named layer wins over the `_water` scan, case-insensitively.
        w.layer.texture = "NAB2_Water".into();
        assert_eq!(
            resolve_texture(&w, None, &two).as_deref(),
            Some("nab2_water")
        );
        // ...but only if the map actually baked it.
        w.layer.texture = "missing_water".into();
        assert_eq!(resolve_texture(&w, None, &two), None);
    }
}
