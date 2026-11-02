//! Terrain heightfield → GLB meshes: world-space grid positions, central-
//! difference normals, the PS2's per-vertex COLR lightmap as the GS vertex
//! byte ([`gs_vertex_byte`], per game), and each layer's texture coordinates
//! as the PS2 projects them ([`layer_uv`]).
//!
//! ## Two GLBs: the base and the layers (card TALPH)
//!
//! `terrain_<name>.glb` ([`terrain_to_glb`]) is LTEX[0] drawn opaque over
//! the whole heightfield — the sim reads its grid block, and the renderer
//! draws it first. `terrain_layers_<name>.glb` ([`terrain_layers_to_glb`])
//! holds every painted layer above LTEX[0], one primitive per layer, each
//! vertex carrying the layer's `ALPH` "over" alpha; the renderer blends them
//! over the base in ascending order, as the PS2 does
//! (analysis/re-terrain-layers.md, [O]). Where the layers cover less than
//! 1, and on the mask-0 patches the PS2 draws no layer on at all, the base
//! LTEX[0] shows: a documented backstop (card TMSK).
//!
//! PS2 files use the original emitters below unchanged. Donor maps use
//! [`AlignedSurface`]: the public corrected heights/cutters shape a separate
//! render grid, while the first grid remains the simulation input.
//!
//! Both meshes split every quad the PS2's way ([`quad_triangles`]), so a
//! layer triangle lies exactly on the base triangle under it.
//!
//! ## Cutter-clipped patches
//!
//! The PS2 render never tests terrain cutters. Every 8x8-cell render patch a
//! cutter reaches into ships a PRE-CLIPPED replacement mesh in its `PTCH`
//! record, and the VU builder draws that instead of the procedural grid
//! (analysis/re-terrain-cutters.md §0.3, §1.2). So does this bake: a flagged
//! patch's 64 quads are dropped and the shipped triangles are appended AFTER
//! the `n x n` grid — 13 of the shipped terrains carry between 1 and 30 of
//! them. Emitting the full grid is what raised the snow inside Hoth's two
//! outpost rooms on both discs (HOLD 19(a)).
//!
//! The grid vertices themselves are untouched and stay first, which is the
//! contract `game::assets::terrain_grid_size` reads the sim's heightfield
//! through.

/// The PS2's terrain vertex colour: one `COLR` channel (5 bits, decoded by
/// `mungefs` to 8 by bit replication) as the byte the GS receives, 0x80 = 1.0.
///
/// Both games draw every terrain layer on VU1's UNLIT path, texture × this
/// colour, with no ambient, sun or `Light2` (analysis/re-terrain-lighting.md
/// §0–§3, [O]). The scale is per GAME: BF1 shifts c5 left 3 (ELF 0x482e68,
/// §3: up to 248, 1.94 × the texture), BF2 left 2 with a unity pass colour
/// × the HDR scale, which is neutral (§1: up to 124). LUMI is not read by any
/// terrain draw (§4).
fn gs_vertex_byte(c8: u8, bf2: bool) -> u8 {
    let c5 = c8 >> 3;
    if bf2 {
        c5 << 2
    } else {
        c5 << 3
    }
}

mod alignment;
pub use alignment::Surface as AlignedSurface;

use anyhow::Result;
use mungefs::terrain::Terrain;
use serde_json::json;

use crate::gltf_bin::{flat3, min_max3, Bin};

/// The GS vertex alpha a layer nibble maps to, 0x80 = 1.0: B2
/// `DAT_0060c1a0`, B1 `DAT_00522a20`, the same 16 bytes in both ELFs
/// (analysis/re-terrain-layers.md §2.2, [O]). 0xf is "absent": its alpha is
/// 0 and it drops every triangle it touches (§2.3).
const LAYER_ALPHA: [u8; 16] = [
    0, 9, 18, 27, 36, 45, 54, 64, 73, 82, 91, 100, 109, 118, 128, 0,
];

/// The absent nibble (§2.3).
const ABSENT: u8 = 0xf;

/// The shipped replacement meshes of a terrain, gathered once: which render
/// patches are flagged, their world-space vertices in one run, and their
/// triangles already based at the end of the `n x n` grid.
///
/// A patch is skipped (and keeps its procedural quads) when its strips index
/// past its own `POSI` — no shipped terrain does, but a truncated record must
/// not take the ground with it.
struct Replacement {
    flagged: Vec<bool>,
    /// Per patch, where its vertices start in `positions` (flagged only).
    first: Vec<usize>,
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

fn replacement_meshes(t: &Terrain) -> Replacement {
    let mut out = Replacement {
        flagged: vec![false; t.patch_meshes.len()],
        first: vec![0; t.patch_meshes.len()],
        positions: Vec::new(),
        triangles: Vec::new(),
    };
    let grid = t.info.grid_size * t.info.grid_size;
    for (p, pm) in t.patch_meshes.iter().enumerate() {
        let Some(pm) = pm else { continue };
        let world = t.patch_mesh_world(p);
        if world.is_empty() {
            continue;
        }
        let tris = mungefs::model::unstitch_strips(&pm.strips);
        if tris.iter().flatten().any(|&i| i as usize >= world.len()) {
            continue;
        }
        let base = grid + out.positions.len() as u32;
        out.triangles
            .extend(tris.iter().map(|v| [v[0] + base, v[1] + base, v[2] + base]));
        out.first[p] = out.positions.len();
        out.positions.extend(world);
        out.flagged[p] = true;
    }
    out
}

/// Sample a per-grid-vertex attribute at a world position, bilinearly.
///
/// The replacement vertices lie ON the heightfield (re-terrain-cutters.md
/// §1.2 — every cell-corner vertex reproduces it to under 5 mm), so the
/// shading of the ground they replace is what their normals should carry.
/// Returns the four grid corners and their weights.
fn grid_sample(t: &Terrain, wx: f32, wz: f32) -> [(usize, f32); 4] {
    let n = t.info.grid_size as usize;
    let last = (n - 1) as f32;
    let [fx, fz, _] = grid_coords(t, [wx, 0.0, wz]);
    let (fx, fz) = (fx.clamp(0.0, last), fz.clamp(0.0, last));
    let (x0, z0) = (fx.floor().min(last - 1.0), fz.floor().min(last - 1.0));
    let (ax, az) = (fx - x0, fz - z0);
    let (x0, z0) = (x0 as usize, z0 as usize);
    [
        (z0 * n + x0, (1.0 - ax) * (1.0 - az)),
        (z0 * n + x0 + 1, ax * (1.0 - az)),
        ((z0 + 1) * n + x0, (1.0 - ax) * az),
        ((z0 + 1) * n + x0 + 1, ax * az),
    ]
}

/// A world position in the PS2's texture frame: grid x, grid z (the inverse
/// of `Terrain::world_xz`, with its one-cell z offset) and the height in
/// grid units (re-terrain-layers.md §3.3).
fn grid_coords(t: &Terrain, p: [f32; 3]) -> [f32; 3] {
    let gu = t.info.grid_unit;
    let half = t.info.grid_size as f32 / 2.0;
    [p[0] / gu + half, p[2] / gu + (half - 1.0), p[1] / gu]
}

/// Layer `k`'s texture coordinates at a vertex, in repeats
/// (analysis/re-terrain-layers.md §3, [O]; B2 `FUN_0056c9d8` 703733-703758,
/// B1 546628-546641). `g` is [`grid_coords`].
///
/// The scale is `round(gridUnit / SCAL · 2048)` ST units per grid cell, 2048
/// units one repeat. AXIS picks the projection: ≥ 6 negates S, then ≥ 3
/// swaps S and T; what is left is 0 (S = x, T = z, top-down), 1 (T from the
/// height) or 2 (S from the height). A cutter-replaced patch compares the
/// raw AXIS with 1 and 2 only (§4), so ≥ 3 is top-down there with no mirror
/// or swap. S is the texture's u, T its v (§9, closed by the 2026-09-28
/// audit: texture registration on HOT1 AXIS 0 and KAS1 AXIS 6).
///
/// A SCAL missing or ≤ 0 (none shipped) falls back to one repeat across the
/// map, as the single-layer bake did.
fn layer_uv(t: &Terrain, k: usize, replacement: bool, g: [f32; 3]) -> [f32; 2] {
    let n = t.info.grid_size as f32;
    let repeats = t
        .layer_scale
        .get(k)
        .copied()
        .filter(|s| s.is_finite() && *s > 0.0)
        .map_or(1.0 / n, |s| {
            ((t.info.grid_unit / s) * 2048.0 + 0.5).floor() / 2048.0
        });
    let mut a = t.layer_axis.get(k).copied().unwrap_or(0);
    let (mut sign, mut swap) = (1.0, false);
    if !replacement {
        if a >= 6 {
            a -= 6;
            sign = -1.0;
        }
        if a >= 3 {
            a -= 3;
            swap = true;
        }
    }
    let [x, z, h] = g;
    let (s, tt) = match a {
        1 => (x, h),
        2 => (h, z),
        _ => (x, z),
    };
    let s = s * sign;
    let (s, tt) = if swap { (tt, s) } else { (s, tt) };
    [s * repeats, tt * repeats]
}

/// The two triangles of grid quad (x, z) as the PS2 splits it: diagonal
/// (x,z)–(x+1,z+1) on even z, (x,z+1)–(x+1,z) on odd z — the terrain
/// builder's strip order (re-terrain-layers.md §2.4, B2 703772-703783), the
/// height query's own split. Corners are `[a, b, c, d]` = (x,z), (x+1,z),
/// (x,z+1), (x+1,z+1).
fn quad_triangles<T: Copy>(z: u32, [a, b, c, d]: [T; 4]) -> [[T; 3]; 2] {
    if z.is_multiple_of(2) {
        [[a, c, d], [a, d, b]]
    } else {
        [[a, c, b], [b, c, d]]
    }
}

/// The ground mesh both terrain GLBs are built from: the `n x n` grid
/// vertices (row-major, first) plus the replacement meshes' vertices, with
/// their normals and GS vertex colours.
struct Ground {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    /// GS vertex byte RGB ([`gs_vertex_byte`]), `None` with no COLR / LUMI.
    colours: Option<Vec<[u8; 3]>>,
    repl: Replacement,
}

fn ground(t: &Terrain, bf2: bool) -> Ground {
    let n = t.info.grid_size;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((n * n) as usize);
    for z in 0..n {
        for x in 0..n {
            let (wx, wz) = t.world_xz(x, z);
            positions.push([wx, t.world_y(x, z), wz]);
        }
    }
    // The cutter-clipped patches' shipped meshes go AFTER the grid, so the
    // grid stays the row-major n x n block the sim reads (module header).
    let repl = replacement_meshes(t);
    positions.extend_from_slice(&repl.positions);

    // central-difference normals
    let h = |x: i64, z: i64| -> f32 {
        let cx = x.clamp(0, n as i64 - 1) as u32;
        let cz = z.clamp(0, n as i64 - 1) as u32;
        t.world_y(cx, cz)
    };
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(positions.len());
    for z in 0..n as i64 {
        for x in 0..n as i64 {
            let dx = (h(x + 1, z) - h(x - 1, z)) / (2.0 * t.info.grid_unit);
            let dz = (h(x, z + 1) - h(x, z - 1)) / (2.0 * t.info.grid_unit);
            let inv = 1.0 / (dx * dx + 1.0 + dz * dz).sqrt();
            normals.push([-dx * inv, inv, -dz * inv]);
        }
    }
    for p in &repl.positions {
        let mut v = [0.0f32; 3];
        for (i, w) in grid_sample(t, p[0], p[2]) {
            for (c, out) in v.iter_mut().enumerate() {
                *out += normals[i][c] * w;
            }
        }
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let inv = if len > 0.0 { 1.0 / len } else { 0.0 };
        normals.push([v[0] * inv, v[1] * inv, v[2] * inv]);
    }

    // Vertex colours: COLR as the GS vertex byte ([`gs_vertex_byte`]). A
    // terrain with no decodable COLR (none shipped) falls back to LUMI's top
    // five bits, grey — PROVISIONAL, the PS2 path for it is unread.
    let grid = (n * n) as usize;
    let colr: Option<std::borrow::Cow<[[u8; 3]]>> = match (&t.colr, &t.lumi) {
        (Some(c), _) if c.len() == grid => Some(c.as_slice().into()),
        (_, Some(l)) if l.len() == grid => Some(l.iter().map(|&v| [v; 3]).collect()),
        _ => None,
    };
    let colours = colr.map(|colr| {
        let g = |c: [u8; 3]| c.map(|ch| gs_vertex_byte(ch, bf2));
        let mut out: Vec<[u8; 3]> = colr.iter().map(|&c| g(c)).collect();
        // A replacement vertex takes its patch's own `COLR`, one u16 per
        // `POSI` vertex (re-terrain-layers.md §4, [O]); where that does not
        // decode (none shipped), the grid's lightmap under it, bilinearly.
        for (p, pm) in t.patch_meshes.iter().enumerate() {
            let (Some(pm), true) = (pm, repl.flagged[p]) else {
                continue;
            };
            let start = repl.first[p];
            for (i, q) in repl.positions[start..start + pm.positions.len()]
                .iter()
                .enumerate()
            {
                let own = pm.colr.as_ref().and_then(|c| c.get(i)).copied();
                out.push(match own {
                    Some(c) => g(c),
                    None => {
                        let mut c = [0.0f32; 3];
                        for (gi, w) in grid_sample(t, q[0], q[2]) {
                            for (ch, o) in c.iter_mut().enumerate() {
                                *o += f32::from(out[gi][ch]) * w;
                            }
                        }
                        c.map(|v| v.round().clamp(0.0, 255.0) as u8)
                    }
                });
            }
        }
        out
    });
    Ground {
        positions,
        normals,
        colours,
        repl,
    }
}

pub fn terrain_to_glb(t: &Terrain, bf2: bool) -> Result<Vec<u8>> {
    let n = t.info.grid_size;
    let mut bin = Bin::default();
    let Ground {
        positions,
        normals,
        colours,
        repl,
    } = ground(t, bf2);
    let (min, max) = min_max3(&positions);
    let pos_acc = bin.f32s(&flat3(&positions), "VEC3", json!({"min": min, "max": max}));
    let nrm_acc = bin.f32s(&flat3(&normals), "VEC3", json!({}));

    // UV: LTEX[0]'s own projection (`layer_uv`), so the base is exactly the
    // layer-0 pass the PS2 draws; a cutter-replaced patch's vertices take the
    // replacement path's rule.
    let grid = (n * n) as usize;
    let uvs: Vec<f32> = positions
        .iter()
        .enumerate()
        .flat_map(|(i, p)| layer_uv(t, 0, i >= grid, grid_coords(t, *p)))
        .collect();
    let uv_acc = bin.f32s(&uvs, "VEC2", json!({}));

    // Vertex colors in a normalized RGBA8 accessor. The byte is 0x80-is-1.0
    // and reaches 248 on BF1, so the renderer's `LightPath::Terrain` decodes
    // it (× 255/128); nothing else may read this COLOR_0 as a plain
    // multiplier.
    let vcount = positions.len();
    let col_acc = colours.map(|c| {
        let rgba: Vec<u8> = c.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect();
        bin.accessor(&rgba, 5121, vcount, "VEC4", json!({"normalized": true}))
    });

    // indices: two triangles per quad in the PS2's split, EXCEPT inside a
    // patch the munge replaced — there the shipped triangles stand in for
    // all 64 quads.
    let cells = t
        .patch_grid()
        .map(|(per_row, cells)| (per_row, cells as u32));
    let replaced = |x: u32, z: u32| match cells {
        Some((per_row, c)) => repl.flagged[(x / c) as usize + (z / c) as usize * per_row],
        None => false,
    };
    let mut idx: Vec<u32> = Vec::with_capacity(((n - 1) * (n - 1) * 6) as usize);
    for z in 0..n - 1 {
        for x in 0..n - 1 {
            if replaced(x, z) {
                continue;
            }
            let a = z * n + x;
            for tri in quad_triangles(z, [a, a + 1, a + n, a + n + 1]) {
                idx.extend_from_slice(&tri);
            }
        }
    }
    for tri in &repl.triangles {
        idx.extend_from_slice(tri);
    }
    let idx_acc = bin.u32s(&idx, json!({}));

    let mut attributes = json!({"POSITION":pos_acc,"NORMAL":nrm_acc,"TEXCOORD_0":uv_acc});
    if let Some(col) = col_acc {
        attributes["COLOR_0"] = json!(col);
    }

    crate::gltf_bin::rigid_glb(
        bin,
        json!({"name": t.name, "primitives": [{
            "attributes": attributes,
            "indices": idx_acc,
            "material": 0,
            "mode": 4
        }]}),
        &t.name,
        Some(json!([{"name": t.layer_textures.first().cloned().unwrap_or_default()}])),
    )
}

/// One layer's draw: vertices (compacted from the ground) and triangles.
#[derive(Default)]
struct LayerDraw {
    /// Per vertex: (ground vertex, alpha nibble).
    verts: Vec<(u32, u8)>,
    slot: std::collections::HashMap<(u32, u8), u32>,
    tris: Vec<u32>,
}

impl LayerDraw {
    /// Add one triangle of ground vertices `v` with nibbles `a` if the PS2
    /// draws it and it shows: the skip rule (a 0xf corner, or all three 0)
    /// unless `whole`, and never a triangle whose three alphas are all 0 —
    /// it would blend nothing (no depth write on this pass).
    fn triangle(&mut self, v: [u32; 3], a: [u8; 3], whole: bool) {
        let skipped = a.contains(&ABSENT) || a == [0; 3];
        if (skipped && !whole) || a.iter().all(|&n| LAYER_ALPHA[n as usize] == 0) {
            return;
        }
        for (g, n) in v.into_iter().zip(a) {
            let next = self.verts.len() as u32;
            let i = *self.slot.entry((g, n)).or_insert(next);
            if i == next {
                self.verts.push((g, n));
            }
            self.tris.push(i);
        }
    }
}

/// The terrain's texture LAYERS above LTEX[0] as `terrain_layers_<name>.glb`:
/// one primitive per layer, ascending, material = the layer's LTEX name,
/// COLOR_0 = the GS vertex byte with the layer's "over" alpha in A
/// (`LAYER_ALPHA[nibble]` / 128 → 0..255). `None` when no patch paints a
/// layer above 0 (BF1 KAM1, NAB2, BF2 NAB2) or the ALPH did not decode.
///
/// analysis/re-terrain-layers.md §0–§4 [O]: the PS2 draws every painted
/// layer of a patch in ascending LTEX order, T_k × COLR, "over" the frame
/// by its interpolated alpha (GS ALPHA 0x44). The renderer draws these
/// primitives in order over the base mesh, which is LTEX[0] opaque
/// everywhere — so a layer-0 pass would be a no-op and is not emitted, and
/// where the layers cover less than 1 (holes, and the mask-0 patches no
/// layer pass draws) the base shows: the documented backstop card TMSK
/// removes.
///
/// Per patch its own `(P+1)²` vertices (the per-patch alphas differ on
/// 0–3.3 % of shared edges, §5.4), the PS2's quad split, and the skip rule
/// (§2.3): a triangle with a 0xf corner or three 0s is not drawn — except on
/// BF2 in the patch's TOP layer, drawn whole with 0xf as alpha 0. A
/// cutter-replaced patch draws its own triangles with its own nibbles (§4).
pub fn terrain_layers_to_glb(t: &Terrain, bf2: bool) -> Result<Option<Vec<u8>>> {
    let layers = t.layer_textures.len().min(16);
    if layers < 2 || t.patch_alpha.len() != t.patch_layer_masks.len() {
        return Ok(None);
    }
    let Some((per_row, cells)) = t.patch_grid() else {
        return Ok(None);
    };
    if cells != t.patch_edge {
        return Ok(None);
    }
    let g = ground(t, bf2);
    let n = t.info.grid_size as usize;
    let edge = cells + 1;
    let mut draws: Vec<LayerDraw> = (0..layers).map(|_| LayerDraw::default()).collect();
    for (p, (&mask, alpha)) in t.patch_layer_masks.iter().zip(&t.patch_alpha).enumerate() {
        let painted: Vec<usize> = (0..16).filter(|b| mask >> b & 1 != 0).collect();
        let top = painted.len().saturating_sub(1);
        for (rank, &k) in painted.iter().enumerate() {
            if k == 0 || k >= layers {
                continue;
            }
            let whole = bf2 && rank == top;
            let d = &mut draws[k];
            if g.repl.flagged[p] {
                let pm = t.patch_meshes[p].as_ref().expect("flagged");
                let count = pm.positions.len();
                let Some(a) = pm.alpha.get(rank * count..(rank + 1) * count) else {
                    continue;
                };
                let base = (n * n + g.repl.first[p]) as u32;
                for tri in mungefs::model::unstitch_strips(&pm.strips) {
                    let v = tri.map(|i| base + i);
                    d.triangle(v, tri.map(|i| a[i as usize]), whole);
                }
                continue;
            }
            let Some(a) = alpha.get(rank * edge * edge..(rank + 1) * edge * edge) else {
                continue;
            };
            let (x0, z0) = ((p % per_row) * cells, (p / per_row) * cells);
            for r in 0..cells {
                for c in 0..cells {
                    let (x, z) = (x0 + c, z0 + r);
                    if x + 1 >= n || z + 1 >= n {
                        continue; // past the grid's last vertex, as the base
                    }
                    let gv = |dx: usize, dz: usize| ((z + dz) * n + x + dx) as u32;
                    let an = |dx: usize, dz: usize| a[(r + dz) * edge + c + dx];
                    let vs = quad_triangles(z as u32, [gv(0, 0), gv(1, 0), gv(0, 1), gv(1, 1)]);
                    let al = quad_triangles(z as u32, [an(0, 0), an(1, 0), an(0, 1), an(1, 1)]);
                    for (v, a) in vs.into_iter().zip(al) {
                        d.triangle(v, a, whole);
                    }
                }
            }
        }
    }
    if draws.iter().all(|d| d.tris.is_empty()) {
        return Ok(None);
    }

    let grid = n * n;
    let mut bin = Bin::default();
    let mut materials = Vec::new();
    let mut primitives = Vec::new();
    for (k, d) in draws.iter().enumerate() {
        if d.tris.is_empty() {
            continue;
        }
        let pos: Vec<[f32; 3]> = d
            .verts
            .iter()
            .map(|&(v, _)| g.positions[v as usize])
            .collect();
        let nrm: Vec<[f32; 3]> = d
            .verts
            .iter()
            .map(|&(v, _)| g.normals[v as usize])
            .collect();
        let uv: Vec<f32> = d
            .verts
            .iter()
            .flat_map(|&(v, _)| {
                layer_uv(
                    t,
                    k,
                    v as usize >= grid,
                    grid_coords(t, g.positions[v as usize]),
                )
            })
            .collect();
        let rgba: Vec<u8> = d
            .verts
            .iter()
            .flat_map(|&(v, nib)| {
                let c = g.colours.as_ref().map_or([128; 3], |c| c[v as usize]);
                let a = (f32::from(LAYER_ALPHA[nib as usize]) * 255.0 / 128.0).round() as u8;
                [c[0], c[1], c[2], a]
            })
            .collect();
        let (min, max) = min_max3(&pos);
        let pos_acc = bin.f32s(&flat3(&pos), "VEC3", json!({"min": min, "max": max}));
        let nrm_acc = bin.f32s(&flat3(&nrm), "VEC3", json!({}));
        let uv_acc = bin.f32s(&uv, "VEC2", json!({}));
        let col_acc = bin.accessor(&rgba, 5121, pos.len(), "VEC4", json!({"normalized": true}));
        let idx_acc = bin.u32s(&d.tris, json!({}));
        primitives.push(json!({
            "attributes": {"POSITION": pos_acc, "NORMAL": nrm_acc, "TEXCOORD_0": uv_acc, "COLOR_0": col_acc},
            "indices": idx_acc,
            "material": materials.len(),
            "mode": 4
        }));
        materials.push(json!({"name": t.layer_textures[k], "alphaMode": "BLEND"}));
    }
    crate::gltf_bin::rigid_glb(
        bin,
        json!({"name": format!("{}_layers", t.name), "primitives": primitives}),
        &format!("{}_layers", t.name),
        Some(json!(materials)),
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::terrain::{PatchMesh, TerrainInfo};

    /// A 16x16-cell terrain (1 m cells, flat at raw 0) whose 2x2 render
    /// patches are all plain except patch 1, which ships a three-vertex
    /// replacement mesh — the shape a cutter-clipped patch has on the disc.
    fn terrain_with_one_replaced_patch(flag: bool) -> mungefs::terrain::Terrain {
        let n = 16usize;
        mungefs::terrain::Terrain {
            name: "t".into(),
            info: TerrainInfo {
                grid_unit: 1.0,
                height_scale: 0.5,
                height_floor: -10.0,
                height_ceiling: 10.0,
                grid_size: n as u32,
            },
            raw: vec![0; n * n],
            lumi: Some(vec![128; n * n]),
            colr: None,
            layer_textures: vec!["snow".into()],
            layer_scale: vec![16.0],
            layer_axis: vec![0],
            layer_rotation: vec![0.0],
            patch_layer_masks: vec![1; 4],
            patch_edge: 8,
            patch_alpha: vec![vec![0xe; 81]; 4],
            patch_meshes: (0..4)
                .map(|p| {
                    (flag && p == 1).then(|| PatchMesh {
                        // The patch corners and its centre, one step up.
                        positions: vec![[0, 0, 0], [0xffff, 0, 0xffff], [0x8000, 0x100, 0x8000]],
                        strips: vec![0x8000, 0x8001, 2],
                        height_min: 0,
                        shift: 8,
                        colr: None,
                        alpha: vec![0xe; 3],
                    })
                })
                .collect(),
            cutters: Vec::new(),
            lowres: None,
        }
    }

    /// Read a GLB's one primitive back as (positions, indices).
    fn read_back(bytes: &[u8]) -> (Vec<[f32; 3]>, Vec<u32>) {
        let (doc, buffers, _) = gltf::import_slice(bytes).expect("gltf reads it");
        let mesh = doc.meshes().next().expect("one mesh");
        let prim = mesh.primitives().next().expect("one primitive");
        let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
        (
            reader.read_positions().expect("positions").collect(),
            reader.read_indices().expect("indices").into_u32().collect(),
        )
    }

    /// Data-gated, and the defect itself: on BF1 HOT1 the terrain the bake
    /// emits must be a HOLE at control point 3, and must sit 1.72 m BELOW the
    /// floor at control point 4 — the two numbers the RE measured on the disc
    /// for what the PS2 draws there (analysis/re-terrain-cutters.md §2). The
    /// full-grid bake covered both posts with a snow mound metres above the
    /// bunker floor (HOLD 19(a)).
    #[test]
    fn bf1_hot1_outpost_rooms_are_no_longer_roofed_with_snow() {
        let lvl = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL"
        );
        let Ok(buf) = std::fs::read(lvl) else {
            eprintln!("SKIP: BF1 HOT1.LVL not present");
            return;
        };
        let inner = mungefs::open(&buf).unwrap();
        let tern = mungefs::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern")
            .expect("tern");
        let t = Terrain::parse(&tern).expect("parses");
        let (pos, idx) = read_back(&terrain_to_glb(&t, false).unwrap());
        let n = t.info.grid_size as usize;
        assert_eq!(pos.len(), n * n + 2536, "grid + the 23 replacement meshes");

        // Every height the terrain covers a world XZ with.
        let cover = |x: f32, z: f32| -> Vec<f32> {
            let mut out = Vec::new();
            for tri in idx.as_chunks::<3>().0 {
                let (a, b, c) = (
                    pos[tri[0] as usize],
                    pos[tri[1] as usize],
                    pos[tri[2] as usize],
                );
                let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
                if d.abs() < 1e-9 {
                    continue;
                }
                let l1 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
                let l2 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
                let l3 = 1.0 - l1 - l2;
                if l1 >= -1e-6 && l2 >= -1e-6 && l3 >= -1e-6 {
                    out.push(l1 * a[1] + l2 * b[1] + l3 * c[1]);
                }
            }
            out
        };
        // cp3 (-213.014, -4.699, 192.011): a hole in the PS2's own surface.
        assert!(cover(-213.014, 192.011).is_empty(), "cp3 is still covered");
        // cp4 (-325.971, -0.837, -52.213): covered, but at -2.56 m — under
        // the floor, where the cutter's volume has taken the terrain away.
        let at_cp4 = cover(-325.971, -52.213);
        assert!(!at_cp4.is_empty(), "cp4 lost its floor");
        for y in &at_cp4 {
            assert!((y + 2.56).abs() < 0.05, "cp4 at {y} m, want -2.56");
        }
    }

    /// The cutter-clipped patch loses its 64 quads and gains the shipped
    /// triangles, which are appended AFTER the n x n grid so the sim still
    /// reads the grid (analysis/re-terrain-cutters.md §0.3, §4).
    #[test]
    fn a_flagged_patch_replaces_its_quads_with_the_shipped_triangles() {
        let n = 16u32;
        let plain =
            read_back(&terrain_to_glb(&terrain_with_one_replaced_patch(false), false).unwrap());
        assert_eq!(plain.0.len(), (n * n) as usize);
        assert_eq!(plain.1.len(), ((n - 1) * (n - 1) * 6) as usize);

        let (pos, idx) =
            read_back(&terrain_to_glb(&terrain_with_one_replaced_patch(true), false).unwrap());
        // The grid is untouched and still first.
        assert_eq!(&pos[..(n * n) as usize], &plain.0[..]);
        assert_eq!(pos.len(), (n * n) as usize + 3);

        // Patch 1 covers cells x 8..16, z 0..8, so the quads with a lower-left
        // corner in x 8..15, z 0..8 are gone: 7 * 8 = 56 of the 225.
        assert_eq!(idx.len(), (225 - 56) * 6 + 3);
        let interior: Vec<u32> = (1..8)
            .flat_map(|z: u32| (9..15).map(move |x| z * n + x))
            .collect();
        assert!(
            !idx.iter().any(|i| interior.contains(i)),
            "a replaced quad survived"
        );

        // The shipped triangle is the last three indices, and its vertices are
        // the patch's own world box: cells (8, 0) to (16, 8) = world x 0..8,
        // z -7..1, at heightMin 0 / one step of 0x100 >> 8 = 1 raw unit.
        assert_eq!(&idx[idx.len() - 3..], &[256, 257, 258]);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(
            near(pos[256][0], 0.0) && near(pos[256][2], -7.0),
            "{:?}",
            pos[256]
        );
        assert!(
            near(pos[257][0], 8.0) && near(pos[257][2], 1.0),
            "{:?}",
            pos[257]
        );
        assert!(near(pos[258][1], 0.5), "{:?}", pos[258]);
    }

    /// COLR c5 = 31 / 16 / 0 (decoded by bit replication to 255 / 132 / 0)
    /// reaches the GS as c5 << 3 on BF1 and c5 << 2 on BF2
    /// (re-terrain-lighting.md §1, §3), whatever LUMI says.
    #[test]
    fn the_vertex_colour_is_colr_as_the_gs_byte_per_game() {
        let mut t = terrain_with_one_replaced_patch(false);
        t.lumi = Some(vec![7; 16 * 16]);
        t.colr = Some(vec![[255, 132, 0]; 16 * 16]);
        for (bf2, want) in [(false, [248u8, 128, 0, 255]), (true, [124, 64, 0, 255])] {
            let bytes = terrain_to_glb(&t, bf2).unwrap();
            let (doc, buffers, _) = gltf::import_slice(&bytes).unwrap();
            let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let cols: Vec<[u8; 4]> = reader.read_colors(0).unwrap().into_rgba_u8().collect();
            assert!(cols.iter().all(|c| *c == want), "bf2={bf2}: {:?}", cols[0]);
        }
    }

    /// The base mesh splits each quad the PS2's way: diagonal (x,z)–(x+1,z+1)
    /// on even z, (x,z+1)–(x+1,z) on odd z (re-terrain-layers.md §2.4).
    #[test]
    fn the_quads_split_the_ps2_way() {
        let (_, idx) =
            read_back(&terrain_to_glb(&terrain_with_one_replaced_patch(false), false).unwrap());
        let n = 16u32;
        let (a, b, c, d) = (0, 1, n, n + 1);
        assert_eq!(&idx[..6], &[a, c, d, a, d, b], "z = 0");
        let row1 = ((n - 1) * 6) as usize;
        let (a, b, c, d) = (n, n + 1, 2 * n, 2 * n + 1);
        assert_eq!(&idx[row1..row1 + 6], &[a, c, b, b, c, d], "z = 1");
    }

    /// One primitive read back: (material, positions, rgba, triangles).
    type LayerPrim = (String, Vec<[f32; 3]>, Vec<[u8; 4]>, usize);

    /// Read a layers GLB back, one [`LayerPrim`] per primitive.
    fn read_layers(bytes: &[u8]) -> Vec<LayerPrim> {
        let (doc, buffers, _) = gltf::import_slice(bytes).expect("gltf reads it");
        let mesh = doc.meshes().next().expect("one mesh");
        mesh.primitives()
            .map(|prim| {
                let r = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
                (
                    prim.material().name().unwrap_or_default().to_string(),
                    r.read_positions().unwrap().collect(),
                    r.read_colors(0).unwrap().into_rgba_u8().collect(),
                    r.read_indices().unwrap().into_u32().count() / 3,
                )
            })
            .collect()
    }

    /// Patch 0 paints LTEX[0] and LTEX[1]; LTEX[1] is opaque everywhere but
    /// at the patch's corner (0, 0), where it is absent (0xf). Only LTEX[1]
    /// is emitted (a layer-0 pass over the opaque LTEX[0] base changes
    /// nothing), and only patch 0's. BF1 drops the two triangles of quad
    /// (0, 0) that touch the corner; BF2 draws its TOP layer whole, with
    /// alpha 0 at the corner (re-terrain-layers.md §2.3).
    #[test]
    fn layers_follow_the_skip_rule_per_game() {
        let mut t = terrain_with_one_replaced_patch(false);
        t.layer_textures = vec!["sand".into(), "grass".into()];
        t.layer_scale = vec![16.0, 8.0];
        t.layer_axis = vec![0, 0];
        t.patch_layer_masks = vec![0b11, 1, 1, 1];
        let mut a = vec![0xe; 2 * 81];
        a[81] = 0xf;
        t.patch_alpha[0] = a;
        for (bf2, tris) in [(false, 126), (true, 128)] {
            let bytes = terrain_layers_to_glb(&t, bf2).unwrap().expect("a layer");
            let prims = read_layers(&bytes);
            assert_eq!(prims.len(), 1);
            let (mat, pos, rgba, n) = &prims[0];
            assert_eq!((mat.as_str(), *n), ("grass", tris), "bf2={bf2}");
            // Patch 0 is cells x 0..8, z 0..8 = world x -8..0, z -7..1.
            assert!(pos.iter().all(|p| p[0] <= 0.0 && p[2] <= 1.0));
            let corner = pos.iter().position(|p| p[0] == -8.0 && p[2] == -7.0);
            assert_eq!(corner.is_some(), bf2);
            if let Some(c) = corner {
                assert_eq!(rgba[c][3], 0);
            }
            assert!(rgba.iter().filter(|c| c[3] != 0).all(|c| c[3] == 255));
        }
        // No layer above 0 painted anywhere: no GLB.
        t.patch_layer_masks = vec![1; 4];
        assert!(terrain_layers_to_glb(&t, false).unwrap().is_none());
    }

    /// §3: k = round(gridUnit / SCAL · 2048) / 2048 repeats per cell; AXIS
    /// ≥ 6 negates S, then ≥ 3 swaps S and T, 1 / 2 take T / S from the
    /// height; a replacement patch reads only 1 and 2.
    #[test]
    fn layer_uv_follows_scal_and_axis() {
        let mut t = terrain_with_one_replaced_patch(false);
        t.layer_scale = vec![16.0; 6];
        t.layer_axis = vec![0, 1, 2, 3, 6, 9];
        let g = [2.0, 4.0, 6.0];
        let uv = |k, r| layer_uv(&t, k, r, g);
        // gridUnit 1 / SCAL 16 = 128 ST units = 1/16 repeat per cell.
        let q = 1.0 / 16.0;
        assert_eq!(uv(0, false), [2.0 * q, 4.0 * q]);
        assert_eq!(uv(1, false), [2.0 * q, 6.0 * q]);
        assert_eq!(uv(2, false), [6.0 * q, 4.0 * q]);
        assert_eq!(uv(3, false), [4.0 * q, 2.0 * q]);
        assert_eq!(uv(4, false), [-2.0 * q, 4.0 * q]);
        assert_eq!(uv(5, false), [4.0 * q, -2.0 * q]);
        assert_eq!(uv(5, true), [2.0 * q, 4.0 * q]);
    }

    /// Data-gated: BF1 KAM1 paints one layer on all 1024 patches, so it gets
    /// no layers GLB (KOC stays its own card); BF1 KAS2 gets its three upper
    /// layers, the pale sand kas2_main_2 among them (F6).
    #[test]
    fn kam1_has_no_layers_and_kas2_has_its_sand() {
        let lvl = |p: &str| {
            std::fs::read(format!(
                "{}/../../../extracted/bf1/lvl/DATA/_LVL_PS2/{p}.LVL",
                env!("CARGO_MANIFEST_DIR")
            ))
            .ok()
        };
        let (Some(kam), Some(kas)) = (lvl("KAM/KAM1"), lvl("KAS/KAS2")) else {
            eprintln!("SKIP: BF1 LVLs not present");
            return;
        };
        let parse = |buf: &[u8]| {
            let inner = mungefs::open(buf).unwrap();
            let tern = mungefs::chunks(inner)
                .flatten()
                .find(|c| &c.tag == b"tern")
                .expect("tern");
            Terrain::parse(&tern).expect("parses")
        };
        assert!(terrain_layers_to_glb(&parse(&kam), false)
            .unwrap()
            .is_none());
        let bytes = terrain_layers_to_glb(&parse(&kas), false)
            .unwrap()
            .expect("layers");
        let mats: Vec<String> = read_layers(&bytes).into_iter().map(|p| p.0).collect();
        assert_eq!(mats, ["kas2_main_2", "kas2_cobbles_02", "kas2_main_3"]);
    }
}
