//! `tern` chunk parsing → world-space heightfield (RE: analysis/
//! re-terrain-heights.md, decomp BF1 FUN_00382338/FUN_00473710, BF2
//! FUN_00465be8/FUN_0055ebc0; lead-verified empirically on TAT1).
//!
//! HGT8 = RLE u8 heights stored PATCH-MAJOR: 16×16-cell patches, cell
//! (x,z) lives at HGT8[(x&15) + ((z&15)<<4) + (patch<<8)],
//! patch = (x>>4) + ((z>>4)<<4)  (for a 256-cell grid; general: patches
//! per row = gridSize/16).
//! HEXP = one 8-byte record per patch: {u8 shift, u8 cutterQuadMask,
//! i16 offset, i16 minHeight, i16 maxHeight} (min/max are munge metadata).
//!   raw(x,z) = (HGT8[..] << shift) + offset
//!   worldY   = raw * heightScale
//! World placement (from the BF2/BF1 height query):
//!   worldX = x*gridUnit - gridSize/2*gridUnit
//!   worldZ = z*gridUnit - (gridSize/2 - 1)*gridUnit   (one-cell z offset)
//!
//! Layer textures: LTEX names them (`hoth_main_1`, ...), SCAL/AXIS/ROTN carry
//! 16 per-layer values each. **SCAL is world metres per texture repeat**,
//! read off the ELF (analysis/re-terrain-layers.md §3, [O]; checked by
//! texture registration against PS2 frames, §9) - as the data had indicated:
//! values are 8..128 on 8 m-cell maps and 512 on the two 40 m-cell (10 km)
//! maps, i.e. they track cell size rather than map size. AXIS picks the
//! layer's projection (§3; decoded by `bake::terrain_glb`); it is 0 for every
//! base layer in both games. ROTN is read by neither game (§3) and is 0 on
//! all but one layer of BF2 DAG1.
//! WHICH layers a patch paints is per-patch data (PCHS -> PTCH) and is parsed
//! into [`Terrain::patch_layer_masks`]; the per-vertex "over" alphas between
//! them (`PCHS -> ALPH`) into [`Terrain::patch_alpha`] (card TALPH).
//!
//! ## `PCHS -> ALPH` is the layers' alphas (card TALPH, 2026-09-28)
//!
//! analysis/re-terrain-layers.md §1 [O] (B2 `FUN_0056bbf0` 703321-703334,
//! `FUN_0056e898`, `FUN_00572c98`; B1 twins): ONE byte-RLE stream walked per
//! `PTCH` in file order. A patch painting k ≥ 2 layers takes the next
//! k·(P+1)·(P/2+1) decoded bytes — per painted layer, ascending bit, P+1 rows
//! (row = z) of P/2+1 bytes, vertex x the low nibble of byte x/2 for even x
//! and the high one for odd x; the tenth nibble is padding. A 1-layer patch
//! takes none and the PS2 fills 0xee (opaque); a mask-0 patch builds nothing.
//! The walk ends exactly at the chunk's end on all 27 shipped terrains.
//! A cutter-replaced patch carries its own `ALPH` (one nibble per `POSI`
//! vertex, (count+1)/2 bytes per layer) and `COLR` (one u16 per vertex), §4.
//!
//! `CUTS -> CUTR` gives the terrain CUTTERS — the convex volumes the terrain is
//! punched out of (cave mouths, hangar floors), each a plane set plus a
//! BOUNDING SPHERE. See [`Cutter`]; measured by the lead, RE_LOG 2026-09-01
//! coll/prim/CUTR measurement (Fable session 7).
//!
//! ## `PCHS -> COLR` is LUMI in colour (cracked 2026-08-31)
//!
//! A2 recorded the patch-level `COLR`/`ALPH` as "not plain RLE arrays — tried
//! elem 1/2/4, none decode to a grid size". `COLR` decodes at **elem 2**, and
//! it does so on every one of the 26 shipped terrains, to a length that is
//! exactly `patches × 81 × 2` — 165,888 B on the 32×32-patch maps, 41,472 on
//! the 16×16 ones, 10,368 on BF2 TAT2's 8×8. 81 = 9×9, i.e. a patch's 8 cells
//! plus the shared edge, so it is ONE u16 PER VERTEX.
//!
//! The u16 is GS `PSMCT16` — R5 G5 B5 A1 — and the alpha bit is set on
//! 100.0 % of all 82,944 entries of every map checked, which is what an
//! opaque colour looks like and not what the top bit of an RGB565 blue does.
//!
//! Layout: the patch index and the 9×9 sample index are both COLUMN-major.
//! Proven by reconstructing LUMI from it: under that ordering the mean
//! absolute difference between `COLR`'s green channel × 8 and `LUMI` is
//! **0.31 of 255 on BF1 END1, 3.3–4.2 on TAT1/KAM1/GEO1** — the 5-bit
//! quantisation error and nothing else — while all three other orderings land
//! at 8–28. Per-map means agree too: `mean(COLR) × 8 ≈ mean(LUMI)` on all 26.
//!
//! So `COLR` and `LUMI` are the same lightmap: LUMI is the 8-bit intensity,
//! COLR the 5-bit colour. The bake keeps LUMI's precision and takes only the
//! HUE from COLR (`bake::terrain_glb`).
//!
//! ## LUMI and COLR are x-major (card TLUMT, 2026-09-26)
//!
//! "Column-major" above is relative to LUMI's own storage, and LUMI is not
//! stored [z][x] like the heights: the PS2's lookup (B1 `FUN_00473d28`
//! 537200, B2 `FUN_0055f1d0` 695200) indexes it `iz + ix·n`. Read that way
//! it reproduces the RAM tints of all 211 BF1 foliage instances (71/211
//! the other way) and correlates with a hillshade of the heights (TAT1 best
//! r 0.307 vs 0.059, BF2 NAB2 0.383 vs 0.030; `lead_tmp/re/folg/`). Both
//! are transposed at decode, so [`Terrain::lumi`] / [`Terrain::colr`] are
//! [z][x] like [`Terrain::raw`].
//!
//! ## The Xbox `PCHS` (2026-11-01)
//!
//! `HGT8`, `HEXP`, `INFO`, `LTEX`, `SCAL`, `FOLG` and friends keep the PS2
//! layout. `PCHS` does not: it holds `COMN` (the shared `IBUF` of a plain patch)
//! and `LOWR`, no `COLR` or `ALPH`, and every `PTCH` draws from its own `VBUF`.
//! Measured against the PS2 twins of BF1 GEO1 and HOT1 (see [`parse_xbox_patches`]):
//!
//! - `PTCH -> INFO` is `u8 k, u8 layer[k], i16 heightMin, i16 heightMax, u16
//!   shift`: the same three numbers as the PS2's, and at most 3 layers where
//!   the PS2 paints up to 5, listed in no fixed order.
//! - `VBUF` is 81 vertices of 16 B (flags 0x58a2, BF2 0x5922), row = z: `u16 x, y, z` with the
//!   top bit flipped from the PS2's `POSI`, a packed normal, a B G R A colour and
//!   two weights. A cutter-replaced patch has an `IBUF` and its own vertices.
//! - The two weights belong to `layer[1]` and `layer[2]`; `layer[0]` takes the
//!   rest of 255. These are final blend weights, where the PS2 has "over" alphas
//!   per ascending layer, so they are converted to the PS2's nibbles.
//!
//! ## `PCHS -> LOWR` is the FAR-LOD sheet, and it is the same footprint
//!
//! Sixteen of the 32 shipped terrains carry `PCHS -> LOWR`: a coarse
//! triangle-strip mesh over the terrain's OWN extent, which is the only
//! terrain either game draws past the near scene (analysis/re-lowr.md, [V]).
//! Parsed by [`parse_lowres`] into [`LowResSheet`]; `bake::lowres_glb` turns
//! it into the map's `lowres_<name>.glb`.

use crate::{le, rle, Chunk, UcfbError};

/// Largest accepted `gridSize`; see the check in `parse`.
const MAX_GRID: usize = 1024;

/// Number of vertices along one edge of a `COLR` patch: 8 cells + the shared
/// edge.
const COLR_PATCH_EDGE: usize = 9;

/// Turn a decoded `PCHS -> COLR` stream into RGB8 on the same `n × n` grid
/// in `LUMI`'s storage order (x-major; the caller transposes both). Returns `None` if the stream is not
/// `patches × 81` u16 for a square patch grid that covers `n`.
fn unpack_colr(bytes: &[u8], n: usize) -> Option<Vec<[u8; 3]>> {
    let samples = COLR_PATCH_EDGE * COLR_PATCH_EDGE;
    let count = bytes.len() / 2;
    if count == 0 || !count.is_multiple_of(samples) {
        return None;
    }
    let patches = count / samples;
    let pside = (patches as f64).sqrt().round() as usize;
    if pside * pside != patches || pside == 0 || !n.is_multiple_of(pside) {
        return None;
    }
    let cells = n / pside;
    let mut out = vec![[0u8; 3]; n * n];
    for p in 0..patches {
        // Patch index is COLUMN-major, and so is the sample index inside it.
        let (px, py) = (p / pside, p % pside);
        for i in 0..samples {
            let (sx, sy) = (i / COLR_PATCH_EDGE, i % COLR_PATCH_EDGE);
            let (gx, gy) = (px * cells + sx, py * cells + sy);
            if gx >= n || gy >= n {
                continue; // the shared edge of the last patch in each row
            }
            let at = (p * samples + i) * 2;
            let u = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            out[gy * n + gx] = psmct16_rgb(u);
        }
    }
    Some(out)
}

/// GS PSMCT16 (R5 G5 B5 A1) to RGB8, 5 bits to 8 by replicating the top 3.
fn psmct16_rgb(u: u16) -> [u8; 3] {
    let ch = |v: u16| ((v << 3) | (v >> 2)) as u8;
    [ch(u & 31), ch((u >> 5) & 31), ch((u >> 10) & 31)]
}

/// The nibble a 1-layer patch's every vertex gets: the PS2 fills its buffer
/// with 0xee (B2 704680, B1 547912), alpha 128 = opaque.
pub const OPAQUE_NIBBLE: u8 = 0xe;

/// Walk the tern-level `PCHS -> ALPH` stream over the patches' masks
/// (module header): per patch, its painted layers' `(P+1)²` nibbles, row =
/// z, and the stream bytes the walk consumed. `None` when it does not walk.
fn walk_alph(src: &[u8], masks: &[u16], p: usize) -> Option<(Vec<Vec<u8>>, usize)> {
    let (edge, row_bytes) = (p + 1, p / 2 + 1);
    let per_layer = edge * row_bytes;
    let mut out = Vec::with_capacity(masks.len());
    let (mut at, mut total) = (0usize, 0usize);
    for &m in masks {
        let k = m.count_ones() as usize;
        // INFO's patch edge and the PTCH count are the file's word: no
        // shipped terrain holds a tenth of the cap (ours, not the PS2's).
        total = total.saturating_add(k.saturating_mul(edge.saturating_mul(edge)));
        if total > rle::MAX_OUTPUT {
            return None;
        }
        match k {
            0 => out.push(Vec::new()),
            1 => out.push(vec![OPAQUE_NIBBLE; edge * edge]),
            _ => {
                let (bytes, used) =
                    rle::decode_prefix(&src[at.min(src.len())..], 1, k * per_layer).ok()?;
                at += used;
                let mut nib = Vec::with_capacity(k * edge * edge);
                for block in bytes.chunks_exact(per_layer) {
                    for row in block.chunks_exact(row_bytes) {
                        nib.extend((0..edge).map(|x| (row[x / 2] >> (4 * (x & 1))) & 0xf));
                    }
                }
                out.push(nib);
            }
        }
    }
    Some((out, at))
}

/// A replacement patch's own `ALPH` as nibbles: per painted layer,
/// `(count+1)/2` bytes, vertex i the low nibble of byte i/2 for even i
/// (re-terrain-layers.md §4, B2 `FUN_0056dcd8` 704324-704327). 0xe for a
/// 1-layer patch; empty when mask 0 or the stream is short.
fn replacement_alpha(src: &[u8], mask: u16, count: usize) -> Vec<u8> {
    let k = mask.count_ones() as usize;
    match k {
        0 => Vec::new(),
        1 => vec![OPAQUE_NIBBLE; count],
        _ => {
            let per_layer = count.div_ceil(2);
            let Ok(bytes) = rle::decode(src, 1) else {
                return Vec::new();
            };
            if count == 0 || bytes.len() < k * per_layer {
                return Vec::new();
            }
            bytes
                .chunks_exact(per_layer)
                .take(k)
                .flat_map(|b| (0..count).map(move |i| (b[i / 2] >> (4 * (i & 1))) & 0xf))
                .collect()
        }
    }
}

/// Swap the axes of an `n × n` grid: `out[a·n + b] = g[b·n + a]`.
fn transpose<T: Copy>(g: &[T], n: usize) -> Vec<T> {
    (0..n * n).map(|i| g[(i % n) * n + i / n]).collect()
}

/// Bytes per Xbox terrain vertex: position, normal, colour, two weights.
const XBOX_PATCH_STRIDE: usize = 16;

/// The `VBUF` flag for the two layer weights. BF1 Xbox writes 0x58a2 and BF2
/// 0x5922. The PC's `PTCH` looks alike but carries 0x5122 and a second float
/// `VBUF`, with no weights: it is not this layout.
const XBOX_PATCH_WEIGHTS: u32 = 0x0800;

/// True when this `PCHS` holds the Xbox's per-patch vertex buffers.
fn is_xbox_pchs(pchs: &Chunk<'_>) -> bool {
    pchs.child(b"COMN").is_some()
        && pchs
            .children_tagged(b"PTCH")
            .next()
            .and_then(|p| p.child(b"VBUF"))
            .is_some_and(|v| {
                v.payload.len() >= 12 && le::u32_at(v.payload, 8) & XBOX_PATCH_WEIGHTS != 0
            })
}

/// The Xbox `PCHS` in the fields the PS2's fills.
struct XboxPatches {
    colr: Vec<[u8; 3]>,
    masks: Vec<u16>,
    meshes: Vec<Option<PatchMesh>>,
    alpha: Vec<Vec<u8>>,
}

/// One vertex's final layer weights to the PS2's per-ascending-layer "over"
/// nibbles (`LAYER_ALPHA` / 128 in `bake::terrain_glb`, 14 steps): the lowest
/// layer is opaque and each one above covers what is under it.
fn weights_to_nibbles(list: &[u8], b0: u8, b1: u8, ascending: &[u8], out: &mut Vec<u8>) {
    let mut w = [0u32; 16];
    let total = 255u32;
    let first = total
        .saturating_sub(u32::from(b0))
        .saturating_sub(u32::from(b1));
    for (i, &layer) in list.iter().enumerate() {
        w[layer as usize & 15] = match i {
            0 => first,
            1 => u32::from(b0),
            _ => u32::from(b1),
        };
    }
    let mut under = 0u32;
    for (rank, &layer) in ascending.iter().enumerate() {
        let own = w[layer as usize & 15];
        under += own;
        out.push(if rank == 0 || under == 0 {
            if rank == 0 {
                OPAQUE_NIBBLE
            } else {
                0
            }
        } else {
            ((own * 14 + under / 2) / under).min(14) as u8
        });
    }
}

/// See "The Xbox `PCHS`" in the module header. `n` is the grid size, `edge` the
/// render patch's edge in cells.
fn parse_xbox_patches(pchs: &Chunk<'_>, n: usize, edge: usize) -> Result<XboxPatches, UcfbError> {
    let bad = || UcfbError::Truncated(pchs.offset);
    if edge == 0 || n == 0 || !n.is_multiple_of(edge) {
        return Err(bad());
    }
    let side = n / edge;
    let samples = (edge + 1) * (edge + 1);
    let mut colr = vec![[128u8; 3]; n * n];
    let mut masks = Vec::new();
    let mut meshes = Vec::new();
    let mut alpha = Vec::new();
    for (p, patch) in pchs.children_tagged(b"PTCH").enumerate() {
        // `k * samples` is allocated per patch below, from a few bytes of file.
        if p >= side * side {
            return Err(UcfbError::Malformed(format!(
                "xbox terrain: more than {side} x {side} patches"
            )));
        }
        let info = patch.child(b"INFO").ok_or_else(bad)?.payload;
        let k = *info.first().ok_or_else(bad)? as usize;
        if k > 16 {
            return Err(UcfbError::Malformed(format!(
                "xbox terrain: {k} layers on patch {p}"
            )));
        }
        if info.len() < 1 + k + 6 {
            return Err(bad());
        }
        let list = &info[1..1 + k];
        let mut ascending = list.to_vec();
        ascending.sort_unstable();
        let mask = list.iter().fold(0u16, |m, &l| m | 1u16 << (l & 15));
        let height_min = le::u16_at(info, 1 + k) as i16;
        let shift = le::u16_at(info, 1 + k + 4);
        masks.push(mask);

        let vp = patch.child(b"VBUF").ok_or_else(bad)?.payload;
        if vp.len() < 12 {
            return Err(bad());
        }
        let count = le::u32_at(vp, 0) as usize;
        let stride = le::u32_at(vp, 4) as usize;
        if stride != XBOX_PATCH_STRIDE
            || count
                .checked_mul(stride)
                .and_then(|b| b.checked_add(12))
                .is_none_or(|b| b > vp.len())
        {
            return Err(UcfbError::Unsupported(format!(
                "xbox terrain VBUF: {count} vertices of {stride} B"
            )));
        }
        let vertex = |v: usize| &vp[12 + v * stride..12 + (v + 1) * stride];
        let rgb = |v: &[u8]| [v[12], v[11], v[10]];
        let mut nibbles = Vec::with_capacity(count * k);
        // Per vertex first, then regrouped per layer like the PS2's.
        for v in 0..count {
            let vx = vertex(v);
            weights_to_nibbles(list, vx[14], vx[15], &ascending, &mut nibbles);
        }
        let per_layer: Vec<u8> = (0..k)
            .flat_map(|rank| {
                let nib = &nibbles;
                (0..count).map(move |v| nib[v * k + rank])
            })
            .collect();

        if let Some(ibuf) = patch.child(b"IBUF") {
            // A replacement mesh. Its strip restarts by repeated indices, so
            // each triangle becomes its own PS2 strip to keep one reader.
            let ip = ibuf.payload;
            if ip.len() < 4 {
                return Err(bad());
            }
            let icount = le::u32_at(ip, 0) as usize;
            if ip.len() < icount.saturating_mul(2).saturating_add(4) {
                return Err(bad());
            }
            let index = |i: usize| le::u16_at(ip, 4 + i * 2);
            let mut strips = Vec::with_capacity(icount.saturating_mul(3));
            for t in 0..icount.saturating_sub(2) {
                let (a, b, c) = (index(t), index(t + 1), index(t + 2));
                if a == b || b == c || a == c {
                    continue;
                }
                let (a, b) = if t % 2 == 0 { (a, b) } else { (b, a) };
                strips.extend([a | 0x8000, b | 0x8000, c]);
            }
            let positions = (0..count)
                .map(|v| {
                    let vx = vertex(v);
                    std::array::from_fn(|i| le::u16_at(vx, i * 2) ^ 0x8000)
                })
                .collect();
            meshes.push(Some(PatchMesh {
                positions,
                strips,
                height_min,
                shift,
                colr: Some((0..count).map(|v| rgb(vertex(v))).collect()),
                alpha: if k == 0 { Vec::new() } else { per_layer },
            }));
            alpha.push(vec![OPAQUE_NIBBLE; k * samples]);
            continue;
        }
        if count != samples {
            return Err(bad());
        }
        meshes.push(None);
        let (px, pz) = (p % side, p / side);
        for sz in 0..=edge {
            for sx in 0..=edge {
                let (gx, gz) = (px * edge + sx, pz * edge + sz);
                if gx < n && gz < n {
                    colr[gz * n + gx] = rgb(vertex(sz * (edge + 1) + sx));
                }
            }
        }
        alpha.push(if k == 0 { Vec::new() } else { per_layer });
    }
    Ok(XboxPatches {
        colr,
        masks,
        meshes,
        alpha,
    })
}

pub struct TerrainInfo {
    pub grid_unit: f32,
    pub height_scale: f32,
    pub height_floor: f32,
    pub height_ceiling: f32,
    pub grid_size: u32,
}

pub struct Terrain {
    pub name: String,
    pub info: TerrainInfo,
    /// Raw integer heights, row-major [z][x], already patch-unswizzled.
    pub raw: Vec<i32>,
    /// Lightmap u8 per vertex, row-major [z][x] like [`Terrain::raw`]
    /// (transposed from the disc's x-major order), if present.
    pub lumi: Option<Vec<u8>>,
    /// `PCHS -> COLR`: the SAME lightmap as [`Terrain::lumi`] in colour,
    /// 5 bits per channel, decoded here to RGB8 on the same [z][x] grid.
    /// `None` when the terrain ships none or it does not decode.
    ///
    /// See the module header for the layout proof. Most maps store it grey
    /// (it is LUMI with no hue), but BF1 KAM1 and BES1 tint 86 % of their
    /// vertices — cool blue, which is exactly Kamino's storm — BF2 DAG1 22 %
    /// and BF1 RHN1 10 %.
    pub colr: Option<Vec<[u8; 3]>>,
    /// Layer texture names (LTEX).
    pub layer_textures: Vec<String>,
    /// SCAL: 16 per-layer values, one per possible texture layer: **world
    /// metres per texture repeat** (see the module header).
    pub layer_scale: Vec<f32>,
    /// AXIS: 16 per-layer projection selectors. 0 on every base layer in both
    /// games; non-zero only on layers >= 2 of nine maps. The decode is
    /// `bake::terrain_glb`'s (re-terrain-layers.md §3).
    pub layer_axis: Vec<u8>,
    /// ROTN: 16 per-layer rotations. Zero everywhere except one layer of
    /// BF2 DAG1; neither game reads it (re-terrain-layers.md §3).
    pub layer_rotation: Vec<f32>,
    /// Per-patch layer bitmask from `PCHS -> PTCH -> INFO` field 0 (low u16),
    /// row-major over (gridSize/8)^2 patches. Bit i = LTEX[i] is painted in
    /// this patch; 0 = the patch carries no layers at all.
    ///
    /// Measured on all 50 shipped PS2 terrains before the ELF was read
    /// (re-terrain-layers.md §1 has the walk that uses it): every mask's
    /// set bits are below `textureCount`, and the high u16 of that field is 0
    /// on every patch except one apiece on YAV1/YAV2 (0x0001_0001 /
    /// 0x0001_0005), so it is a separate flag word. Mask 0 maps out the area
    /// outside the level: plotted for KAS1 (92 % zero) and END1 (85 %) the
    /// non-zero patches form exactly the island / forest clearing.
    /// LTEX[0] is set in 81-100 % of the non-zero patches of every map.
    /// How the layers listed in a mask are blended is [`Terrain::patch_alpha`]
    /// (card TALPH).
    pub patch_layer_masks: Vec<u16>,
    /// The PRE-CLIPPED replacement mesh each `PCHS -> PTCH` render patch
    /// ships when a terrain cutter touches it, in the same row-major patch
    /// order as [`Terrain::patch_layer_masks`]; `None` on the patches that
    /// draw as the plain procedural grid. See [`PatchMesh`].
    pub patch_meshes: Vec<Option<PatchMesh>>,
    /// `INFO`'s u16 at +18: the render-patch edge P in cells, 8 on every
    /// shipped terrain (B2 `DAT_00661550`, re-terrain-layers.md §1).
    pub patch_edge: usize,
    /// `PCHS -> ALPH`, per `PTCH` in the order of
    /// [`Terrain::patch_layer_masks`]: the painted layers' 4-bit "over"
    /// alphas, per layer (ascending bit) `(P+1)²` nibbles, row-major with
    /// row = z (module header). 0xe everywhere on a 1-layer patch; EMPTY on a
    /// mask-0 patch, which no layer pass draws. Every entry is empty when the
    /// stream does not walk (no shipped terrain).
    pub patch_alpha: Vec<Vec<u8>>,
    /// `CUTS -> CUTR` terrain cutters, in file order. Empty on a map with no
    /// `CUTS` (BF2 MUS1). See [`Cutter`].
    pub cutters: Vec<Cutter>,
    /// `PCHS -> LOWR`, the far-LOD sheet: `None` on the 16 shipped terrains
    /// that carry none. See [`LowResSheet`].
    pub lowres: Option<LowResSheet>,
}

/// `tern -> PCHS -> LOWR`: the coarse far-LOD sheet of the terrain's OWN
/// footprint — the only terrain EITHER game draws past the near scene
/// (analysis/re-lowr.md §0-§1, [V]; BF1 loader `FUN_00481608` decomp
/// 546096-546240, BF2 703386-703534).
///
/// Sixteen of the 32 shipped terrains carry one, and on every one `extent`
/// equals `gridUnit * gridSize`, so the sheet covers the heightfield's own
/// footprint and nothing outside it (§0.1, census of both discs). It is a
/// LOD, not an outside fill: a sheet vertex sits a median 0.00-1.08 m from
/// the HGT8/HEXP height under it.
#[derive(Debug, Clone, PartialEq)]
pub struct LowResSheet {
    /// `INFO` grid: the sheet holds `grid * grid` `LPTC` patches (8-18).
    pub grid: u32,
    /// `INFO` resolution — the same number the map's sky authors as
    /// `LowResTerrain.PatchResolution` (5-11 on the disc).
    pub resolution: u32,
    /// `INFO` extent, metres.
    pub extent: f32,
    /// `INFO` y range: the sheet box's floor and ceiling, which is what the
    /// `POSI` y dequantises over.
    pub y_min: f32,
    pub y_max: f32,
    /// One entry per `LOWR -> PCHS -> LPTC`, in file order.
    pub patches: Vec<LowResPatch>,
}

impl LowResSheet {
    pub fn vertex_count(&self) -> usize {
        self.patches.iter().map(|p| p.positions.len()).sum()
    }

    pub fn triangle_count(&self) -> usize {
        self.patches.iter().map(|p| p.triangles.len()).sum()
    }
}

/// One `LOWR -> PCHS -> LPTC`: a strip-meshed piece of the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct LowResPatch {
    /// World-space vertices in `POSI` order, dequantised over the SHEET box —
    /// NOT over the patch's own `INFO` box, which is the cull volume the
    /// per-frame patch loop tests (re-lowr.md §1, `FUN_004915a0` at 549558)
    /// and is `c = extent/2 - half` off the world frame (0 on every shipped
    /// map).
    pub positions: Vec<[f32; 3]>,
    /// `STRP` unstitched into triangles indexing [`LowResPatch::positions`].
    pub triangles: Vec<[u32; 3]>,
}

/// One terrain cutter: a convex volume the terrain is punched out of, stored
/// as planes. A point p is INSIDE iff `nx·px + ny·py + nz·pz + d < 0` for
/// EVERY plane (strict) — the PS2 height query returns the −300.0 sentinel for
/// a point inside one (analysis/re-terrain-heights.md §HEXP, FUN_00473db0).
///
/// Measured by the lead (RE_LOG 2026-09-01 coll/prim/CUTR measurement, Fable
/// session 7): each `CUTR` carries `INFO` (20 B: `u32 planeCount` + 4 × f32)
/// and `PLNS` (planeCount × f32[4] = normal + d). The cap planes of some
/// cutters have |n| = 1.002, so the planes are stored exactly as read rather
/// than renormalised.
///
/// The four `INFO` floats are the cutter's BOUNDING SPHERE `(r, cx, cy, cz)` —
/// measured by the lead on the raw discs: BF2 DAG1 cutter 3 = (33.64, 8.26,
/// −2.20, −49.30), a sphere around the cave post at (16, −2, −50); BF2 POL1
/// cutter 6 = (24.7, 109.0, 45.3, −51.0). It is a conservative bound, so it is
/// only ever a cheap REJECT before the plane test
/// (`core::collision::is_cut_by`).
#[derive(Debug, Clone, Default)]
pub struct Cutter {
    /// `(radius, cx, cy, cz)` — the cutter's bounding sphere, from `INFO`.
    pub sphere: [f32; 4],
    pub planes: Vec<[f32; 4]>,
}

/// A flagged render patch's PRE-CLIPPED replacement mesh: what the PS2 draws
/// in place of that patch's 8x8 procedural grid because a terrain cutter
/// reaches into it (analysis/re-terrain-cutters.md §0.3 and §1.1: BF1 loader
/// `FUN_00481608` decomp 546287-546333, builder `FUN_00485f20` 548676-548710;
/// BF2 703562-703628 / 706055-706085). The munge clips the patch by the
/// cutter's 3-D VOLUME — measured over the four Hoth outpost rooms, no
/// surviving surface point lies inside a cutter and no hole lies outside every
/// cutter (§2).
///
/// The vertices are stored quantised because the dequant needs the patch's own
/// origin: [`Terrain::patch_mesh_world`] does it.
#[derive(Debug, Clone)]
pub struct PatchMesh {
    /// `POSI`, one `u16 x, y, z` per vertex, in the order `STRP` indexes.
    pub positions: Vec<[u16; 3]>,
    /// `STRP`: PS2 strip indices, bit 15 = the strip flag — a strip restarts
    /// at a flagged PAIR ([`crate::model::unstitch_strips`]).
    pub strips: Vec<u16>,
    /// `INFO`'s heightMin, in the same raw height units as [`Terrain::raw`].
    pub height_min: i16,
    /// `INFO`'s shift: one `POSI` y count is `(65536 >> shift) / 65536` raw
    /// height units (§1.2, proven on the disc to under 5 mm at every cell
    /// corner).
    pub shift: u16,
    /// `COLR`, one RGB8 per `POSI` vertex (PSMCT16, decoded like the grid's),
    /// or `None` when it does not decode to exactly that many
    /// (analysis/re-terrain-layers.md §4: it does on all 122 shipped).
    pub colr: Option<Vec<[u8; 3]>>,
    /// `ALPH` as nibbles: per painted layer (ascending bit), one per `POSI`
    /// vertex. 0xe for every vertex of a 1-layer patch (the PS2's 0xee
    /// fill); empty for a mask-0 patch or a stream that does not decode.
    pub alpha: Vec<u8>,
}

impl Terrain {
    /// True when this `tern` is a stub — NAME only (COR1/MYG1/TAN1/UTA1) or
    /// NAME + WATR (DEA1). Those BF2 maps are built entirely from models
    /// (city platforms, interiors) and ship no heightfield, so there is
    /// nothing to parse and nothing to bake.
    pub fn is_stub(tern: &Chunk<'_>) -> bool {
        tern.child(b"INFO").is_none() || tern.child(b"HGT8").is_none()
    }

    pub fn parse(tern: &Chunk<'_>) -> Result<Terrain, UcfbError> {
        let bad = || UcfbError::Truncated(tern.offset);
        let name = tern.child_string(b"NAME").ok_or_else(bad)?;
        let ip = tern.child(b"INFO").ok_or_else(bad)?.payload;
        if ip.len() < 28 {
            return Err(bad());
        }
        let f = |o: usize| le::f32_at(ip, o);
        let info = TerrainInfo {
            grid_unit: f(0),
            height_scale: f(4),
            height_floor: f(8),
            height_ceiling: f(12),
            grid_size: u16::from_le_bytes([ip[16], ip[17]]) as u32,
        };
        let n = info.grid_size as usize;
        if n == 0 || !n.is_multiple_of(16) {
            return Err(bad());
        }
        // gridSize is a raw u16 and squares into every allocation below, so an
        // unchecked 65520 would ask for a 17 GB `raw`. The largest shipped
        // terrain is 256 (the 10 km maps, at 40 m cells).
        if n > MAX_GRID {
            return Err(UcfbError::Malformed(format!("tern {name}: grid size {n}")));
        }

        let h8 = rle::decode(tern.child(b"HGT8").ok_or_else(bad)?.payload, 1)?;
        if h8.len() != n * n {
            return Err(bad());
        }
        let hexp = tern.child(b"HEXP").ok_or_else(bad)?.payload;
        let patches_per_row = n / 16;
        let patch_count = patches_per_row * patches_per_row;
        if hexp.len() < patch_count * 8 {
            return Err(bad());
        }

        // Unswizzle patch-major HGT8 into a row-major raw grid, applying the
        // per-patch (h8 << shift) + offset.
        let mut raw = vec![0i32; n * n];
        for z in 0..n {
            for x in 0..n {
                let patch = (x >> 4) + ((z >> 4) * patches_per_row);
                let idx = (x & 15) + ((z & 15) << 4) + (patch << 8);
                let rec = &hexp[patch * 8..patch * 8 + 8];
                let shift = rec[0] as u32;
                let offset = i16::from_le_bytes([rec[2], rec[3]]) as i32;
                // Wrapping, as the MIPS sllv/addu do: a corrupt HEXP shift of
                // 32+ must not panic a debug build.
                raw[z * n + x] = (h8[idx] as i32).wrapping_shl(shift).wrapping_add(offset);
            }
        }

        // LUMI is stored x-major, `iz + ix·n` (B1 FUN_00473d28, B2
        // FUN_0055f1d0 — see "LUMI and COLR are x-major" above): transposed
        // here to the [z][x] order of `raw`.
        let lumi = tern
            .child(b"LUMI")
            .filter(|c| c.payload.len() >= n * n)
            .map(|c| transpose(&c.payload[..n * n], n));

        // PCHS -> COLR: the same lightmap in colour. See the module header.
        let colr = tern
            .child(b"PCHS")
            .and_then(|p| p.child(b"COLR"))
            .and_then(|c| rle::decode(c.payload, 2).ok())
            .and_then(|v| unpack_colr(&v, n))
            .map(|v| transpose(&v, n));

        let mut layer_textures = Vec::new();
        if let Some(ltex) = tern.child(b"LTEX") {
            for part in ltex.payload.split(|&b| b == 0) {
                if !part.is_empty() {
                    if let Ok(s) = std::str::from_utf8(part) {
                        layer_textures.push(s.to_owned());
                    }
                }
            }
        }

        let f32s = |tag: &[u8; 4]| -> Vec<f32> {
            tern.child(tag)
                .map(|c| {
                    c.payload
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .take(16)
                        .map(|b| f32::from_le_bytes(*b))
                        .collect()
                })
                .unwrap_or_default()
        };
        let layer_scale = f32s(b"SCAL");
        let layer_rotation = f32s(b"ROTN");
        let layer_axis = tern
            .child(b"AXIS")
            .map(|c| c.payload.iter().copied().take(16).collect())
            .unwrap_or_default();

        let patch_edge = u16::from_le_bytes([ip[18], ip[19]]) as usize;
        let xbox = tern
            .child(b"PCHS")
            .filter(|pchs| is_xbox_pchs(pchs))
            .map(|pchs| parse_xbox_patches(&pchs, n, patch_edge))
            .transpose()?;

        // PCHS -> PTCH, one record per 8x8-cell RENDER patch, row-major.
        // PS2 `INFO` is 14 B: `u16 layerMask, u16 count, i16 heightMin,
        // i16 heightMax, u16 shift, u32 flag` (re-terrain-cutters.md §1.1).
        // PC/Xbox records can stop after 8 or 10 B; their layer mask is
        // still valid. Only the full PS2 form carries the replacement flag.
        // A non-zero flag means the record also carries POSI/STRP — the
        // replacement mesh the PS2 draws instead of the grid there.
        let mut patch_layer_masks = Vec::new();
        let mut patch_meshes = Vec::new();
        if let Some(pchs) = tern.child(b"PCHS").filter(|_| xbox.is_none()) {
            for patch in pchs.children().flatten() {
                if &patch.tag != b"PTCH" {
                    continue;
                }
                let info = patch.child(b"INFO").filter(|i| i.payload.len() >= 8);
                let u16_at =
                    |i: &Chunk<'_>, o: usize| u16::from_le_bytes([i.payload[o], i.payload[o + 1]]);
                let m = info.as_ref().map(|i| u16_at(i, 0)).unwrap_or(0);
                patch_layer_masks.push(m);
                let mesh = info.as_ref().and_then(|i| {
                    let flag = u32::from_le_bytes(i.payload.get(10..14)?.try_into().unwrap());
                    if flag == 0 {
                        return None;
                    }
                    let posi = patch.child(b"POSI")?;
                    let strp = patch.child(b"STRP")?;
                    Some(PatchMesh {
                        positions: posi
                            .payload
                            .as_chunks::<6>()
                            .0
                            .iter()
                            .map(|c| {
                                [
                                    u16::from_le_bytes([c[0], c[1]]),
                                    u16::from_le_bytes([c[2], c[3]]),
                                    u16::from_le_bytes([c[4], c[5]]),
                                ]
                            })
                            .collect(),
                        strips: strp
                            .payload
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect(),
                        height_min: u16_at(i, 4) as i16,
                        shift: u16_at(i, 8),
                        colr: patch
                            .child(b"COLR")
                            .and_then(|c| rle::decode(c.payload, 2).ok())
                            .filter(|v| v.len() == posi.payload.len() / 6 * 2)
                            .map(|v| {
                                v.as_chunks::<2>()
                                    .0
                                    .iter()
                                    .map(|u| psmct16_rgb(u16::from_le_bytes([u[0], u[1]])))
                                    .collect()
                            }),
                        alpha: replacement_alpha(
                            patch.child(b"ALPH").map(|c| c.payload).unwrap_or_default(),
                            m,
                            posi.payload.len() / 6,
                        ),
                    })
                });
                patch_meshes.push(mesh);
            }
        }

        // PCHS -> ALPH, walked per PTCH in file order (module header).
        let alph = tern.child(b"PCHS").and_then(|p| p.child(b"ALPH"));
        let patch_alpha = walk_alph(
            alph.as_ref().map(|a| a.payload).unwrap_or_default(),
            &patch_layer_masks,
            patch_edge,
        )
        .map_or_else(|| vec![Vec::new(); patch_layer_masks.len()], |(a, _)| a);
        let (colr, patch_layer_masks, patch_meshes, patch_alpha) = match xbox {
            Some(x) => (Some(x.colr), x.masks, x.meshes, x.alpha),
            None => (colr, patch_layer_masks, patch_meshes, patch_alpha),
        };

        // CUTS holds its own INFO (the cutter count) followed by one CUTR per
        // cutter; a map with no CUTS simply has none.
        let mut cutters = Vec::new();
        if let Some(cuts) = tern.child(b"CUTS") {
            for cutr in cuts.children().flatten() {
                if &cutr.tag != b"CUTR" {
                    continue;
                }
                let Some(plns) = cutr.child(b"PLNS") else {
                    continue;
                };
                let info = cutr.child(b"INFO").filter(|i| i.payload.len() >= 4);
                let count = info
                    .as_ref()
                    .map(|i| u32::from_le_bytes(i.payload[0..4].try_into().unwrap()) as usize)
                    .unwrap_or(plns.payload.len() / 16);
                // The four floats after the plane count are the bounding
                // sphere (r, cx, cy, cz); a short INFO leaves it zeroed, which
                // `core::collision` reads as "no bound, test the planes".
                let sphere = info
                    .filter(|i| i.payload.len() >= 20)
                    .map(|i| {
                        let g = |o: usize| le::f32_at(i.payload, o);
                        [g(4), g(8), g(12), g(16)]
                    })
                    .unwrap_or_default();
                if plns.payload.len() / 16 < count {
                    return Err(UcfbError::Truncated(cutr.offset));
                }
                cutters.push(Cutter {
                    sphere,
                    planes: plns.payload[..count * 16]
                        .as_chunks::<16>()
                        .0
                        .iter()
                        .map(|c| {
                            let g = |o: usize| le::f32_at(c, o);
                            [g(0), g(4), g(8), g(12)]
                        })
                        .collect(),
                });
            }
        }

        // PCHS -> LOWR, the far-LOD sheet. Sixteen terrains ship one; the
        // other sixteen simply have none.
        let lowres = tern
            .child(b"PCHS")
            .and_then(|pchs| pchs.children().flatten().find(|c| &c.tag == b"LOWR"))
            .and_then(|lowr| parse_lowres(&lowr, info.grid_unit * n as f32 / 2.0));

        Ok(Terrain {
            name,
            info,
            raw,
            lumi,
            colr,
            layer_textures,
            layer_scale,
            layer_axis,
            layer_rotation,
            patch_layer_masks,
            patch_meshes,
            patch_edge,
            patch_alpha,
            cutters,
            lowres,
        })
    }

    /// World-space height at integer cell (x, z).
    pub fn world_y(&self, x: u32, z: u32) -> f32 {
        self.raw[(z * self.info.grid_size + x) as usize] as f32 * self.info.height_scale
    }

    /// `(patches per row, cells per patch)` of the `PCHS -> PTCH` render-patch
    /// grid. Derived from the record count the way the builder's own indexing
    /// implies — 8 cells per patch on every shipped terrain
    /// (`DAT_0056485c` = 8, analysis/re-terrain-cutters.md §1.2), i.e. 32x32
    /// patches on a 256-cell map. `None` when the count is not a square that
    /// divides the heightfield, which no shipped terrain is.
    pub fn patch_grid(&self) -> Option<(usize, usize)> {
        let count = self.patch_layer_masks.len();
        let per_row = (count as f64).sqrt().round() as usize;
        if per_row == 0 || per_row * per_row != count {
            return None;
        }
        let n = self.info.grid_size as usize;
        let cells = n / per_row;
        (cells > 0 && cells * per_row == n).then_some((per_row, cells))
    }

    /// World-space vertices of a flagged patch's replacement mesh, in `POSI`
    /// order so `STRP` indexes straight into it. Empty when the patch ships
    /// none.
    ///
    /// The dequant is the VU box the builder sets up (re-terrain-cutters.md
    /// §1.2, decomp 548703-548710): the patch spans
    /// `cells * gridUnit * 65536/65535` in X and Z from its own origin, and Y
    /// is `(heightMin + y * (65536 >> shift) / 65536) * heightScale` in the
    /// same raw units as [`Terrain::world_y`]. Measured on the disc: every
    /// vertex that lands on a cell corner reproduces the heightfield to under
    /// 5 mm (186/186 over the six Hoth patches).
    pub fn patch_mesh_world(&self, patch: usize) -> Vec<[f32; 3]> {
        let Some(pm) = self.patch_meshes.get(patch).and_then(|p| p.as_ref()) else {
            return Vec::new();
        };
        let Some((per_row, cells)) = self.patch_grid() else {
            return Vec::new();
        };
        let gu = self.info.grid_unit;
        let half = self.info.grid_size as f32 / 2.0 * gu;
        let (px, pz) = (patch % per_row, patch / per_row);
        let x0 = px as f32 * cells as f32 * gu - half;
        let z0 = pz as f32 * cells as f32 * gu - (half - gu);
        let span = cells as f32 * gu * (65536.0 / 65535.0);
        // `65536 >> shift` exactly as the ELF computes it, so a shift past 16
        // collapses the patch to its heightMin plane rather than panicking.
        let step = 65536u32.checked_shr(pm.shift as u32).unwrap_or(0) as f32 / 65536.0;
        pm.positions
            .iter()
            .map(|&[x, y, z]| {
                [
                    x0 + x as f32 / 65536.0 * span,
                    (self.raw_to_y(pm.height_min as f32 + y as f32 * step)),
                    z0 + z as f32 / 65536.0 * span,
                ]
            })
            .collect()
    }

    /// Raw height units to world Y — the same scale [`Terrain::world_y`] uses.
    fn raw_to_y(&self, raw: f32) -> f32 {
        raw * self.info.height_scale
    }

    /// World-space X/Z of cell (x, z) (RE: one-cell z offset).
    pub fn world_xz(&self, x: u32, z: u32) -> (f32, f32) {
        let half = self.info.grid_size as f32 / 2.0;
        (
            (x as f32 - half) * self.info.grid_unit,
            (z as f32 - (half - 1.0)) * self.info.grid_unit,
        )
    }
}

/// Parse `tern -> PCHS -> LOWR` (analysis/re-lowr.md §1, [V]).
///
/// `INFO` is `u32 grid, u32 resolution, f32 extent, f32 ymin, f32 ymax`, then
/// `PCHS` holds `grid * grid` `LPTC`s of `INFO` (6 f32 cull box), `POSI`
/// (`u16 x, y, z`) and `STRP` (the usual PS2 strips).
///
/// The dequant is the SHEET box the renderer hands the VU (546113-546127,
/// 549503; BF2 706855): `x, z` in `[-half, extent - half]` and `y` in
/// `[ymin, ymax]`, each `u16 / 65536` of that span. The /65536 (rather than
/// the /65535 a `segm` or a water patch uses) is the reading the neighbouring
/// `PTCH` path forces — its box is padded by `65536/65535` at 546004 — and it
/// is the one the session-14 census measured the sheet against: a vertex
/// lands a median 0.00-1.08 m from the heightfield under it on all 16 maps.
///
/// Returns `None` when the chunk has no usable `INFO`, which no shipped
/// terrain does.
fn parse_lowres(lowr: &Chunk<'_>, half: f32) -> Option<LowResSheet> {
    let info = lowr.child(b"INFO").filter(|i| i.payload.len() >= 20)?;
    let u32_at = |o: usize| u32::from_le_bytes(info.payload[o..o + 4].try_into().unwrap());
    let f = |o: usize| le::f32_at(info.payload, o);
    let (grid, resolution, extent, y_min, y_max) = (u32_at(0), u32_at(4), f(8), f(12), f(16));

    let mut patches = Vec::new();
    if let Some(pchs) = lowr.child(b"PCHS") {
        // The Xbox's `COMN -> IBUF` is the one strip every `LPTC`'s own 6-byte
        // `VBUF` draws, and its `u16`s are the PS2's with the top bit flipped.
        let shared: Option<Vec<[u32; 3]>> = pchs
            .child(b"COMN")
            .and_then(|c| c.child(b"IBUF"))
            .filter(|i| i.payload.len() >= 4)
            .and_then(|i| {
                let count = le::u32_at(i.payload, 0) as usize;
                (i.payload.len() >= count.saturating_mul(2).saturating_add(4)).then(|| {
                    let index = |t: usize| u32::from(le::u16_at(i.payload, 4 + t * 2));
                    (0..count.saturating_sub(2))
                        .filter_map(|t| {
                            let (a, b, c) = (index(t), index(t + 1), index(t + 2));
                            if a == b || b == c || a == c {
                                None
                            } else if t % 2 == 0 {
                                Some([a, b, c])
                            } else {
                                Some([b, a, c])
                            }
                        })
                        .collect()
                })
            });
        for lptc in pchs.children_tagged(b"LPTC") {
            if let (Some(tris), Some(vbuf)) = (&shared, lptc.child(b"VBUF")) {
                let vp = vbuf.payload;
                let count = if vp.len() >= 12 {
                    le::u32_at(vp, 0) as usize
                } else {
                    0
                };
                if vp.len() < 12 || le::u32_at(vp, 4) != 6 || vp.len() < 12 + count * 6 {
                    patches.push(LowResPatch {
                        positions: Vec::new(),
                        triangles: Vec::new(),
                    });
                    continue;
                }
                let positions: Vec<[f32; 3]> = (0..count)
                    .map(|v| {
                        let q = |i: usize| {
                            f32::from(le::u16_at(vp, 12 + v * 6 + i * 2) ^ 0x8000) / 65536.0
                        };
                        [
                            -half + q(0) * extent,
                            y_min + q(1) * (y_max - y_min),
                            -half + q(2) * extent,
                        ]
                    })
                    .collect();
                let triangles = tris
                    .iter()
                    .filter(|t| t.iter().all(|&i| (i as usize) < positions.len()))
                    .copied()
                    .collect();
                patches.push(LowResPatch {
                    positions,
                    triangles,
                });
                continue;
            }
            let (Some(posi), Some(strp)) = (lptc.child(b"POSI"), lptc.child(b"STRP")) else {
                // No shipped LPTC is empty; one would simply draw nothing.
                patches.push(LowResPatch {
                    positions: Vec::new(),
                    triangles: Vec::new(),
                });
                continue;
            };
            let positions: Vec<[f32; 3]> = posi
                .payload
                .as_chunks::<6>()
                .0
                .iter()
                .map(|c| {
                    let q =
                        |i: usize| u16::from_le_bytes([c[i * 2], c[i * 2 + 1]]) as f32 / 65536.0;
                    [
                        -half + q(0) * extent,
                        y_min + q(1) * (y_max - y_min),
                        -half + q(2) * extent,
                    ]
                })
                .collect();
            let raw: Vec<u16> = strp
                .payload
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            // Bit 15 is the strip flag; bit 14 is NOT an index bit and is not
            // set in ANY shipped sheet (0 of 415,277 indices over both discs,
            // measured — the `0x4000` in re-lowr.md §1 is the VU word the
            // loader BUILDS, not what the disc stores), so the standard
            // `& 0x7fff` mask is exact here.
            let triangles = crate::model::unstitch_strips(&raw)
                .into_iter()
                .filter(|t| t.iter().all(|&i| (i as usize) < positions.len()))
                .collect();
            patches.push(LowResPatch {
                positions,
                triangles,
            });
        }
    }

    Some(LowResSheet {
        grid,
        resolution,
        extent,
        y_min,
        y_max,
        patches,
    })
}

#[cfg(test)]
mod tests {
    use super::Terrain;

    fn tern_of(path: &str) -> Option<Vec<u8>> {
        std::fs::read(format!(
            "{}/../../../extracted/{path}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .ok()
    }

    /// Data-gated: the layer header the bake now needs. HOT1 names three snow
    /// layers scaled 16/32/64 m per repeat, and every one of its 1024 patches
    /// paints layer 0 (`hoth_main_1`) — which is why the terrain bake uses
    /// LTEX[0] as its single base layer.
    #[test]
    fn hot1_layer_header_and_patch_masks() {
        let Some(buf) = tern_of("bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL") else {
            eprintln!("SKIP: HOT1.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let tern = crate::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern")
            .expect("tern");
        let t = Terrain::parse(&tern).expect("parses");
        assert_eq!(
            t.layer_textures,
            ["hoth_main_1", "hoth_main_2", "hoth_main_4"]
        );
        assert_eq!(&t.layer_scale[..3], &[16.0, 32.0, 64.0]);
        assert!(t.layer_axis[..3].iter().all(|&a| a == 0));
        assert!(t.layer_rotation[..3].iter().all(|&r| r == 0.0));
        assert_eq!(t.patch_layer_masks.len(), 1024);
        assert_eq!(t.patch_layer_masks.iter().filter(|&&m| m == 0).count(), 0);
        assert!(
            t.patch_layer_masks.iter().all(|m| m & 1 != 0),
            "layer 0 is painted in every HOT1 patch"
        );
        assert!(
            t.patch_layer_masks.iter().all(|m| m >> 3 == 0),
            "no mask bit past textureCount = 3"
        );
    }

    /// Data-gated: the flagged `PTCH` replacement meshes on BF1 HOT1 — the
    /// pre-clipped terrain the PS2 draws inside the two outpost rooms
    /// (analysis/re-terrain-cutters.md §0.5 and §2). 23 of the 1024 render
    /// patches carry one; PTCH 588 is the one under control point 3.
    ///
    /// The dequant is pinned the way the RE proves it: every replacement
    /// vertex that lands on a cell corner (POSI x and z multiples of 0x2000 =
    /// 65536/8) reproduces the heightfield's own height to under 5 mm.
    #[test]
    fn hot1_flagged_patches_carry_a_replacement_mesh() {
        let Some(buf) = tern_of("bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL") else {
            eprintln!("SKIP: HOT1.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let tern = crate::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern")
            .expect("tern");
        let t = Terrain::parse(&tern).expect("parses");
        assert_eq!(t.patch_meshes.len(), t.patch_layer_masks.len());
        assert_eq!(t.patch_grid(), Some((32, 8)));
        let flagged: Vec<usize> = t
            .patch_meshes
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.as_ref().map(|_| i))
            .collect();
        assert_eq!(flagged.len(), 23, "{flagged:?}");
        assert!(flagged.contains(&588) && flagged.contains(&458) && flagged.contains(&490));

        let pm = t.patch_meshes[588].as_ref().unwrap();
        assert_eq!(pm.positions.len(), 123);
        assert_eq!(pm.strips.len(), 235);
        assert_eq!((pm.height_min, pm.shift), (-35, 9));
        assert_eq!(pm.colr.as_ref().map(Vec::len), Some(123));
        let k = t.patch_layer_masks[588].count_ones() as usize;
        assert_eq!(pm.alpha.len(), k * 123);

        // Every strip index is inside POSI on every flagged patch, so the
        // strips can be unstitched straight into the mesh.
        for &p in &flagged {
            let m = t.patch_meshes[p].as_ref().unwrap();
            assert!(
                m.strips
                    .iter()
                    .all(|i| ((i & 0x7fff) as usize) < m.positions.len()),
                "patch {p}: STRP index past POSI"
            );
        }

        // The cell-corner check, on all 23 patches at once.
        let (per_row, cells) = t.patch_grid().unwrap();
        let mut corners = 0;
        for &p in &flagged {
            let m = t.patch_meshes[p].as_ref().unwrap();
            let world = t.patch_mesh_world(p);
            assert_eq!(world.len(), m.positions.len());
            let (px, pz) = (p % per_row, p / per_row);
            for (q, w) in m.positions.iter().zip(&world) {
                let step = 65536 / cells as u32;
                if !(q[0] as u32).is_multiple_of(step) || !(q[2] as u32).is_multiple_of(step) {
                    continue;
                }
                let (cx, cz) = (
                    px as u32 * cells as u32 + q[0] as u32 / step,
                    pz as u32 * cells as u32 + q[2] as u32 / step,
                );
                if cx >= t.info.grid_size || cz >= t.info.grid_size {
                    continue;
                }
                let dy = (w[1] - t.world_y(cx, cz)).abs();
                assert!(dy < 0.005, "patch {p} cell ({cx},{cz}): {dy} m");
                let (wx, wz) = t.world_xz(cx, cz);
                assert!((w[0] - wx).abs() < 0.01 && (w[2] - wz).abs() < 0.01);
                corners += 1;
            }
        }
        assert!(corners > 100, "only {corners} cell-corner vertices checked");
    }

    /// The base layer covers most of every map, and mask 0 marks the ground
    /// outside the level (KAS1 is an island in an ocean of empty patches).
    #[test]
    fn base_layer_covers_the_textured_patches() {
        let (mut tested, mut skipped) = (0, 0);
        for (name, min_pct) in [
            ("bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL", 80),
            ("bf1/lvl/DATA/_LVL_PS2/KAS/KAS1.LVL", 80),
            ("bf2/lvl/DATA/_LVL_PS2/YAV/YAV1.LVL", 80),
        ] {
            let Some(buf) = tern_of(name) else {
                eprintln!("SKIP: {name} not present");
                skipped += 1;
                continue;
            };
            tested += 1;
            let inner = crate::open(&buf).unwrap();
            let tern = crate::chunks(inner)
                .flatten()
                .find(|c| &c.tag == b"tern")
                .unwrap_or_else(|| panic!("{name}: tern"));
            let t = Terrain::parse(&tern).unwrap_or_else(|e| panic!("{name}: {e}"));
            let textured: Vec<u16> = t
                .patch_layer_masks
                .iter()
                .copied()
                .filter(|&m| m != 0)
                .collect();
            assert!(!textured.is_empty(), "{name}");
            let with_base = textured.iter().filter(|m| *m & 1 != 0).count();
            let pct = 100 * with_base / textured.len();
            assert!(
                pct >= min_pct,
                "{name}: LTEX[0] painted in only {pct}% of textured patches"
            );
            assert!(
                !t.layer_textures.is_empty() && t.layer_scale[0] > 0.0,
                "{name}"
            );
        }
        eprintln!(
            "base_layer_covers_the_textured_patches: {tested} maps tested, {skipped} skipped"
        );
    }

    /// A synthetic `PCHS` with one flagged and one plain `PTCH`: the flag word
    /// is what carries the replacement mesh, and the dequant places it in the
    /// patch's own world box (analysis/re-terrain-cutters.md §1.2).
    #[test]
    fn flagged_ptch_parses_into_a_world_space_mesh() {
        use crate::build_chunk;
        // 16x16 cells, 1 m units, 2x2 render patches of 8 cells each.
        let n = 16u16;
        let mut info = vec![0u8; 28];
        info[0..4].copy_from_slice(&1f32.to_le_bytes()); // gridUnit
        info[4..8].copy_from_slice(&0.5f32.to_le_bytes()); // heightScale
        info[16..18].copy_from_slice(&n.to_le_bytes());
        // PTCH INFO: mask, count, heightMin, heightMax, shift, flag.
        let ptch_info = |min: i16, shift: u16, flag: u32| {
            let mut v = 1u16.to_le_bytes().to_vec();
            v.extend(0u16.to_le_bytes());
            v.extend(min.to_le_bytes());
            v.extend(64i16.to_le_bytes());
            v.extend(shift.to_le_bytes());
            v.extend(flag.to_le_bytes());
            v
        };
        // Three vertices: the patch's own corner, its far corner, and its
        // centre one height step up.
        let posi: Vec<u8> = [[0u16, 0, 0], [0xffff, 0, 0xffff], [0x8000, 0x100, 0x8000]]
            .iter()
            .flat_map(|v| v.iter().flat_map(|c| c.to_le_bytes()))
            .collect();
        let strp: Vec<u8> = [0x8000u16, 0x8001, 2]
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let mut flagged = build_chunk(b"INFO", &ptch_info(-4, 8, 1));
        flagged.extend(build_chunk(b"POSI", &posi));
        flagged.extend(build_chunk(b"STRP", &strp));
        // COLR: three u16 (a run of 3 x 0x801f = red, alpha bit set).
        flagged.extend(build_chunk(b"COLR", &[0x82, 0x1f, 0x80]));
        // The mask (1) paints one layer, so ALPH is not read: 0xe fill.
        flagged.extend(build_chunk(b"ALPH", &[4, 5]));
        let mut pchs = Vec::new();
        for p in 0..4 {
            // Patch 1 (px=1, pz=0) is the flagged one.
            pchs.extend(build_chunk(
                b"PTCH",
                &if p == 1 {
                    flagged.clone()
                } else {
                    {
                        let mut short = ptch_info(0, 8, 0);
                        // Console PC/Xbox patches omit the PS2 replacement
                        // flag (and sometimes the shift), retaining the mask.
                        if p == 0 {
                            short.truncate(8);
                        } else if p == 2 {
                            short.truncate(10);
                        }
                        build_chunk(b"INFO", &short)
                    }
                },
            ));
        }
        let mut body = build_chunk(b"NAME", b"t\0");
        body.extend(build_chunk(b"INFO", &info));
        // RLE: 0xFF = repeat the next byte 128 times; twice = 256 zero cells.
        body.extend(build_chunk(b"HGT8", &[0xFFu8, 0x00, 0xFF, 0x00]));
        body.extend(build_chunk(b"HEXP", &[0u8; 8]));
        body.extend(build_chunk(b"PCHS", &pchs));
        let buf = build_chunk(b"tern", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let t = Terrain::parse(&c).expect("parses");

        assert_eq!(t.patch_grid(), Some((2, 8)));
        assert_eq!(t.patch_layer_masks, vec![1, 1, 1, 1]);
        assert_eq!(t.patch_meshes.len(), 4);
        assert!(t.patch_meshes[0].is_none() && t.patch_meshes[2].is_none());
        let pm = t.patch_meshes[1].as_ref().expect("patch 1 is flagged");
        assert_eq!(pm.positions.len(), 3);
        assert_eq!(pm.strips, vec![0x8000, 0x8001, 2]);
        assert_eq!((pm.height_min, pm.shift), (-4, 8));
        assert_eq!(pm.colr.as_deref(), Some(&[[255u8, 0, 0]; 3][..]));
        assert_eq!(pm.alpha, vec![0xe; 3]);

        // Patch (1, 0) starts at cell (8, 0) = world (0, -7) and spans 8 m.
        let w = t.patch_mesh_world(1);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(near(w[0][0], 0.0) && near(w[0][2], -7.0), "{:?}", w[0]);
        assert!(near(w[1][0], 8.0) && near(w[1][2], 1.0), "{:?}", w[1]);
        assert!(near(w[2][0], 4.0) && near(w[2][2], -3.0), "{:?}", w[2]);
        // Y = (heightMin + y*(65536 >> shift)/65536) * heightScale:
        // corners at -4 raw = -2 m, the centre 0x100 counts up = -3 raw.
        assert!(near(w[0][1], -2.0) && near(w[1][1], -2.0), "{:?}", w);
        assert!(near(w[2][1], -1.5), "{:?}", w[2]);
        // An unflagged patch has no mesh at all.
        assert!(t.patch_mesh_world(0).is_empty());
    }

    /// Data-gated, the TALPH census (analysis/re-terrain-layers.md §1, §4,
    /// `lead_tmp/re/talph/census.txt`, `repl.txt`): on all 27 shipped
    /// terrains the per-patch `ALPH` walk ends exactly at the chunk's end,
    /// and the 122 cutter-replaced patches' own `COLR` / `ALPH` decode to
    /// one entry per `POSI` vertex (per layer).
    #[test]
    fn alph_walks_every_shipped_terrain_exactly() {
        let (mut tested, mut skipped) = (0, 0);
        let lvls = [
            "bf1/BES/BES1",
            "bf1/BES/BES2",
            "bf1/END/END1",
            "bf1/GEO/GEO1",
            "bf1/HOT/HOT1",
            "bf1/KAM/KAM1",
            "bf1/KAS/KAS1",
            "bf1/KAS/KAS2",
            "bf1/NAB/NAB1",
            "bf1/NAB/NAB2",
            "bf1/RHN/RHN1",
            "bf1/RHN/RHN2",
            "bf1/TAT/TAT1",
            "bf1/TAT/TAT2",
            "bf1/TEST/TEST",
            "bf1/YAV/YAV1",
            "bf1/YAV/YAV2",
            "bf2/DAG/DAG1",
            "bf2/END/END1",
            "bf2/FEL/FEL1",
            "bf2/GEO/GEO1",
            "bf2/HOT/HOT1",
            "bf2/KAS/KAS2",
            "bf2/NAB/NAB2",
            "bf2/POL/POL1",
            "bf2/TAT/TAT2",
            "bf2/YAV/YAV1",
        ];
        let (mut replaced, mut multi_replaced, mut multi) = (0, 0, 0);
        for l in lvls {
            let (game, rest) = l.split_at(3);
            let Some(buf) = tern_of(&format!("{game}/lvl/DATA/_LVL_PS2{rest}.LVL")) else {
                eprintln!("SKIP: {l} not present");
                skipped += 1;
                continue;
            };
            tested += 1;
            let inner = crate::open(&buf).unwrap();
            let tern = crate::chunks(inner)
                .flatten()
                .find(|c| &c.tag == b"tern" && !Terrain::is_stub(c))
                .unwrap_or_else(|| panic!("{l}: tern"));
            let t = Terrain::parse(&tern).unwrap_or_else(|e| panic!("{l}: {e}"));
            assert_eq!(t.patch_edge, 8, "{l}");
            let alph = tern.child(b"PCHS").and_then(|p| p.child(b"ALPH"));
            let src = alph.as_ref().map(|a| a.payload).unwrap_or_default();
            let (_, used) = super::walk_alph(src, &t.patch_layer_masks, t.patch_edge)
                .unwrap_or_else(|| panic!("{l}: the ALPH walk fails"));
            assert_eq!(used, src.len(), "{l}: the walk ends inside the chunk");
            for (m, a) in t.patch_layer_masks.iter().zip(&t.patch_alpha) {
                assert_eq!(a.len(), m.count_ones() as usize * 81, "{l}");
                multi += usize::from(m.count_ones() > 1);
            }
            for (m, pm) in t.patch_layer_masks.iter().zip(&t.patch_meshes) {
                let Some(pm) = pm else { continue };
                replaced += 1;
                let n = pm.positions.len();
                assert_eq!(pm.colr.as_ref().map(Vec::len), Some(n), "{l}: COLR");
                assert_eq!(pm.alpha.len(), m.count_ones() as usize * n, "{l}: ALPH");
                multi_replaced += usize::from(m.count_ones() > 1);
            }
        }
        eprintln!(
            "alph_walks_every_shipped_terrain_exactly: {tested} maps tested, {skipped} skipped"
        );
        if skipped != 0 {
            eprintln!("SKIP: full 27-map aggregate census requires every terrain");
            return;
        }
        assert_eq!((replaced, multi_replaced), (122, 104));
        assert_eq!(multi, 4703, "multi-layer patches, census.txt's sum");
    }

    /// The in-patch layout (re-terrain-layers.md §1.4): per painted layer a
    /// block of 9 rows (row = z) of 5 bytes, vertex x the low nibble of byte
    /// x/2 for even x; a 1-layer patch reads no bytes and is 0xe throughout.
    #[test]
    fn nibble_order_is_low_first_row_z() {
        use crate::build_chunk;
        // One 16-cell terrain, P = 8: 2x2 patches, patch 0 paints layers 0
        // and 2; its ALPH is 2 x 45 literal bytes.
        let mut info = vec![0u8; 28];
        info[0..4].copy_from_slice(&1f32.to_le_bytes());
        info[16..18].copy_from_slice(&16u16.to_le_bytes());
        info[18..20].copy_from_slice(&8u16.to_le_bytes());
        let ptch = |mask: u16| {
            let mut v = mask.to_le_bytes().to_vec();
            v.extend([0u8; 12]);
            build_chunk(b"PTCH", &build_chunk(b"INFO", &v))
        };
        let mut pchs = ptch(0b101);
        for _ in 0..3 {
            pchs.extend(ptch(1));
        }
        // Layer block 0: byte b = b (so row r's bytes are 5r..5r+5); block 1
        // all 0x3c. Literal runs of at most 128.
        let mut raw: Vec<u8> = (0..45u8).collect();
        raw.extend([0x3c; 45]);
        let mut alph = vec![89u8];
        alph.extend(&raw);
        pchs.extend(build_chunk(b"ALPH", &alph));
        let mut body = build_chunk(b"NAME", b"t\0");
        body.extend(build_chunk(b"INFO", &info));
        body.extend(build_chunk(b"HGT8", &[0xFFu8, 0x00, 0xFF, 0x00]));
        body.extend(build_chunk(b"HEXP", &[0u8; 8]));
        body.extend(build_chunk(b"PCHS", &pchs));
        let buf = build_chunk(b"tern", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let t = Terrain::parse(&c).expect("parses");
        let a = &t.patch_alpha[0];
        assert_eq!(a.len(), 2 * 81);
        // Row z = 1, x = 0..9 reads bytes 5..10: 5 = (lo 5, hi 0), 6 = (6, 0) …
        assert_eq!(&a[9..18], &[5, 0, 6, 0, 7, 0, 8, 0, 9]);
        assert!(a[81..]
            .iter()
            .enumerate()
            .all(|(i, &v)| v == if i % 9 % 2 == 0 { 0xc } else { 3 }));
        assert_eq!(t.patch_alpha[1], vec![0xe; 81]);
    }

    /// Data-gated: BF2's model-built maps ship a NAME-only `tern` (COR1) or
    /// NAME + WATR (DEA1). Those are stubs, not truncated terrain — the bake
    /// used to abort on them.
    #[test]
    fn bf2_model_built_maps_have_stub_terrain() {
        let (mut tested, mut skipped) = (0, 0);
        for (name, stub) in [("COR/COR1", true), ("DEA/DEA1", true), ("HOT/HOT1", false)] {
            let p = format!(
                "{}/../../../extracted/bf2/lvl/DATA/_LVL_PS2/{name}.LVL",
                env!("CARGO_MANIFEST_DIR")
            );
            let Ok(buf) = std::fs::read(&p) else {
                eprintln!("SKIP: {name} not present");
                skipped += 1;
                continue;
            };
            tested += 1;
            let inner = crate::open(&buf).unwrap();
            let tern = crate::chunks(inner)
                .flatten()
                .find(|c| &c.tag == b"tern")
                .unwrap_or_else(|| panic!("{name} has a tern"));
            assert_eq!(Terrain::is_stub(&tern), stub, "{name}");
            if !stub {
                Terrain::parse(&tern).unwrap_or_else(|e| panic!("{name}: {e}"));
            }
        }
        eprintln!(
            "bf2_model_built_maps_have_stub_terrain: {tested} maps tested, {skipped} skipped"
        );
    }

    /// `gridSize` is a raw u16 that squares into every allocation, so 65520
    /// asks for a 17 GB `raw` grid. Reject it before allocating anything.
    #[test]
    fn absurd_grid_size_errors() {
        use crate::build_chunk;
        let mut info = vec![0u8; 28];
        info[0..4].copy_from_slice(&8f32.to_le_bytes()); // gridUnit
        info[4..8].copy_from_slice(&0.05f32.to_le_bytes()); // heightScale
        info[16..18].copy_from_slice(&65520u16.to_le_bytes()); // gridSize
        let mut body = build_chunk(b"NAME", b"t\0");
        body.extend(build_chunk(b"INFO", &info));
        body.extend(build_chunk(b"HGT8", &[0x80, 0x00]));
        body.extend(build_chunk(b"HEXP", &[0u8; 8]));
        let buf = build_chunk(b"tern", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let Err(e) = Terrain::parse(&c) else {
            panic!("expected an error")
        };
        assert!(matches!(e, crate::UcfbError::Malformed(_)), "{e}");
    }

    /// A point is inside a cutter iff EVERY plane evaluates strictly < 0.
    fn inside(c: &super::Cutter, p: [f32; 3]) -> bool {
        c.planes
            .iter()
            .all(|q| q[0] * p[0] + q[1] * p[1] + q[2] * p[2] + q[3] < 0.0)
    }

    /// Synthetic `CUTS`: a unit box around the origin as six planes, plus a
    /// terrain with no `CUTS` at all (BF2 MUS1 ships none — parse must not
    /// fail).
    #[test]
    fn synthetic_cuts_parses_and_absent_cuts_is_empty() {
        use crate::build_chunk;
        let planes: [[f32; 4]; 6] = [
            [1.0, 0.0, 0.0, -1.0],
            [-1.0, 0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0, -1.0],
            [0.0, -1.0, 0.0, -1.0],
            [0.0, 0.0, 1.0, -1.0],
            [0.0, 0.0, -1.0, -1.0],
        ];
        let mut plns = Vec::new();
        for pl in &planes {
            for v in pl {
                plns.extend(v.to_le_bytes());
            }
        }
        let mut cutr = build_chunk(b"INFO", &{
            let mut i = 6u32.to_le_bytes().to_vec();
            i.extend([0u8; 16]);
            i
        });
        cutr.extend(build_chunk(b"PLNS", &plns));
        let mut cuts = build_chunk(b"INFO", &1u32.to_le_bytes());
        cuts.extend(build_chunk(b"CUTR", &cutr));

        let mut info = vec![0u8; 28];
        info[0..4].copy_from_slice(&1f32.to_le_bytes());
        info[4..8].copy_from_slice(&1f32.to_le_bytes());
        info[16..18].copy_from_slice(&16u16.to_le_bytes());
        // RLE: 0xFF = repeat the next element 128 times; twice = 256 zeros.
        let hgt8 = [0xFFu8, 0x00, 0xFF, 0x00];
        let mut body = build_chunk(b"NAME", b"t\0");
        body.extend(build_chunk(b"INFO", &info));
        body.extend(build_chunk(b"HGT8", &hgt8));
        body.extend(build_chunk(b"HEXP", &[0u8; 8]));
        let with_cuts = {
            let mut b = body.clone();
            b.extend(build_chunk(b"CUTS", &cuts));
            build_chunk(b"tern", &b)
        };
        let c = crate::chunks(&with_cuts).next().unwrap().unwrap();
        let t = Terrain::parse(&c).expect("parses");
        assert_eq!(t.cutters.len(), 1);
        assert_eq!(t.cutters[0].planes.len(), 6);
        // This synthetic INFO zeroes the four sphere floats: no bound.
        assert_eq!(t.cutters[0].sphere, [0.0; 4]);
        assert!(inside(&t.cutters[0], [0.0, 0.0, 0.0]));
        assert!(!inside(&t.cutters[0], [2.0, 0.0, 0.0]));
        // On the plane is NOT inside (the test is strict).
        assert!(!inside(&t.cutters[0], [1.0, 0.0, 0.0]));

        let without = build_chunk(b"tern", &body);
        let c = crate::chunks(&without).next().unwrap().unwrap();
        assert!(Terrain::parse(&c)
            .expect("parses without CUTS")
            .cutters
            .is_empty());
    }

    /// Pinned by the lead on the raw disc (RE_LOG 2026-09-01 coll/prim/CUTR
    /// measurement, Fable session 7): BF2 DAG1 ships 4 cutters with plane
    /// counts 18/18/18/6, and the cave post cp4's capture centre is inside
    /// the 6-plane one while the terrain surface above it is not.
    #[test]
    fn dag1_pinned_cutters() {
        let Some(buf) = tern_of("bf2/lvl/DATA/_LVL_PS2/DAG/DAG1.LVL") else {
            eprintln!("SKIP: BF2 DAG1.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let tern = crate::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern")
            .expect("tern");
        let t = Terrain::parse(&tern).expect("parses");
        let counts: Vec<usize> = t.cutters.iter().map(|c| c.planes.len()).collect();
        assert_eq!(counts, vec![18, 18, 18, 6]);
        let c3 = &t.cutters[3];
        let near = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 1e-4);
        // INFO's four floats are the bounding sphere (r, cx, cy, cz): a
        // 33.6 m sphere around the cave, which contains the post at
        // (16, −2, −50).
        assert!(
            near(c3.sphere, [33.636_5, 8.264_658, -2.201_917_6, -49.301_994]),
            "{:?}",
            c3.sphere
        );
        let d2 = (16.0f32 - c3.sphere[1]).powi(2)
            + (-2.0f32 - c3.sphere[2]).powi(2)
            + (-50.0f32 - c3.sphere[3]).powi(2);
        assert!(d2 < c3.sphere[0] * c3.sphere[0], "post inside the bound");
        assert!(
            near(c3.planes[0], [-0.9870979, 0.0, 0.16536956, -5.759_383]),
            "{:?}",
            c3.planes[0]
        );
        // |n| = 1.002 on the caps: stored as read, never renormalised.
        assert!(
            near(c3.planes[1], [0.0, -1.0020342, 0.0, -6.186556]),
            "{:?}",
            c3.planes[1]
        );
        assert!(
            near(c3.planes[2], [0.0, 1.0020342, 0.0, -1.8135252]),
            "{:?}",
            c3.planes[2]
        );
        // The cave post's capture centre is inside; the terrain surface above
        // it is not (the test is 3-D on the unit's own position).
        assert!(inside(c3, [16.0, -2.0, -50.0]));
        assert!(!inside(c3, [16.0, 4.7, -50.0]));
        // ... and no surface post is inside any cutter.
        assert!(!t.cutters.iter().any(|c| inside(c, [16.0, 4.7, -50.0])));
    }
}

#[cfg(test)]
mod xbox_tests {
    use super::*;

    /// Real data: BF1 GEO1 and HOT1's Xbox terrain keeps the PS2's heights and
    /// layer paint, its lightmap and hue read in the same orientation, the
    /// cutter patches ship the same vertices, and the far sheet the same land.
    #[test]
    fn xbox_terrain_matches_the_ps2() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        for (xbox, ps2) in [
            (
                "xbox_bf1/geo/geo1.lvl",
                "bf1/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL",
            ),
            (
                "xbox_bf1/hot/hot1.lvl",
                "bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL",
            ),
        ] {
            let (Ok(xb), Ok(pb)) = (
                std::fs::read(format!("{root}/{xbox}")),
                std::fs::read(format!("{root}/{ps2}")),
            ) else {
                eprintln!("SKIP: {xbox} not present");
                return;
            };
            let parse = |buf: &[u8]| {
                let c = crate::content_chunks(crate::open(buf).unwrap())
                    .unwrap()
                    .into_iter()
                    .find(|c| &c.tag == b"tern")
                    .unwrap();
                Terrain::parse(&c).unwrap()
            };
            let (x, p) = (parse(&xb), parse(&pb));
            // BF1 HOT1's heights really differ on the two discs; GEO1's do not.
            let height_off = x.raw.iter().zip(&p.raw).filter(|(a, b)| a != b).count();
            eprintln!("{xbox}: heights off {height_off}/{}", x.raw.len());
            if xbox.contains("geo") {
                assert_eq!(height_off, 0);
            }
            assert_eq!(x.patch_layer_masks.len(), p.patch_layer_masks.len());
            assert_eq!(x.patch_meshes.len(), p.patch_meshes.len());

            // Lightmap: Xbox and PS2 are the same painting, so the transposed
            // read must correlate far better than the raw one.
            let (xl, pl) = (x.lumi.as_ref().unwrap(), p.lumi.as_ref().unwrap());
            let n = x.info.grid_size as usize;
            let diff = |a: &[u8], b: &[u8]| -> f64 {
                a.iter()
                    .zip(b)
                    .map(|(u, v)| f64::from(u.abs_diff(*v)))
                    .sum::<f64>()
                    / a.len() as f64
            };
            let pearson = |a: &[u8], b: &[u8]| -> f64 {
                let m = |c: &[u8]| c.iter().map(|&v| f64::from(v)).sum::<f64>() / c.len() as f64;
                let (ma, mb) = (m(a), m(b));
                let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
                for (u, v) in a.iter().zip(b) {
                    let (u, v) = (f64::from(*u) - ma, f64::from(*v) - mb);
                    ab += u * v;
                    aa += u * u;
                    bb += v * v;
                }
                ab / (aa * bb).sqrt().max(1e-9)
            };
            let swapped = transpose(xl, n);
            let (straight, across) = (pearson(xl, pl), pearson(&swapped, pl));
            eprintln!(
                "{xbox}: lumi r {straight:.3}, transposed {across:.3}, mean diff {:.1}",
                diff(xl, pl)
            );
            assert!(straight > across, "{straight} vs {across}");

            // Hue: per-vertex Xbox colour against the same PS2 COLR cell.
            let (xc, pc) = (x.colr.as_ref().unwrap(), p.colr.as_ref().unwrap());
            let green = |c: &[[u8; 3]]| c.iter().map(|v| v[1]).collect::<Vec<u8>>();
            let right = pearson(&green(xc), &green(pc));
            let wrong = pearson(&green(&transpose(xc, n)), &green(pc));
            let own = pearson(&green(xc), xl);
            eprintln!(
                "{xbox}: colr r {right:.3}, transposed {wrong:.3}, against own lumi {own:.3}"
            );
            assert!(right > wrong, "{right} vs {wrong}");

            // Layer paint: over every patch where both discs name the same
            // layers, the nibbles agree closely (the Xbox's are re-derived).
            let (mut same, mut near) = (0usize, 0usize);
            for i in 0..x.patch_layer_masks.len() {
                if x.patch_layer_masks[i] != p.patch_layer_masks[i]
                    || x.patch_meshes[i].is_some()
                    || p.patch_meshes[i].is_some()
                {
                    continue;
                }
                let (a, b) = (&x.patch_alpha[i], &p.patch_alpha[i]);
                assert_eq!(a.len(), b.len(), "patch {i}");
                for (u, v) in a.iter().zip(b) {
                    same += 1;
                    let v = if *v == 0xf { 0 } else { *v };
                    near += usize::from(u.abs_diff(v) <= 2);
                }
            }
            eprintln!("{xbox}: nibbles within 2 steps {near}/{same}");
            assert!(near * 100 >= same * 80, "{near}/{same}");

            // Cutter patches: the same vertices and triangle count, on the
            // patches both discs agree are the same height range.
            let (mut both, mut alike) = (0usize, 0usize);
            for i in 0..x.patch_meshes.len() {
                if let (Some(a), Some(b)) = (&x.patch_meshes[i], &p.patch_meshes[i]) {
                    if a.height_min != b.height_min || a.shift != b.shift {
                        continue;
                    }
                    both += 1;
                    let tris = |s: &[u16]| crate::model::unstitch_strips(s).len();
                    let (ta, tb) = (tris(&a.strips), tris(&b.strips));
                    alike += usize::from(
                        a.positions.len() == b.positions.len()
                            && ta.abs_diff(tb) * 10 <= tb.max(10),
                    );
                }
            }
            eprintln!("{xbox}: cutter patches alike {alike}/{both}");
            assert!(alike * 10 >= both * 8, "{alike}/{both}");

            // Far sheet: same grid, and every patch covers the same land.
            let (xs, ps) = (x.lowres.as_ref().unwrap(), p.lowres.as_ref().unwrap());
            assert_eq!((xs.grid, xs.resolution), (ps.grid, ps.resolution));
            assert_eq!(xs.patches.len(), ps.patches.len());
            let y_mean = |s: &LowResSheet| {
                let (mut sum, mut c) = (0.0f64, 0usize);
                for v in s.patches.iter().flat_map(|p| &p.positions) {
                    sum += f64::from(v[1]);
                    c += 1;
                }
                sum / c.max(1) as f64
            };
            assert!((y_mean(xs) - y_mean(ps)).abs() < 2.0, "{xbox} sheet height");
            assert!(xs.triangle_count() > 0);
        }
    }

    /// One `PCHS` of `count` cutter patches, each listing `k` layers.
    fn cutter_pchs(count: usize, k: u8) -> Vec<u8> {
        let mut info = vec![k];
        info.extend(vec![0u8; k as usize + 6]);
        let mut vbuf = 0u32.to_le_bytes().to_vec();
        vbuf.extend(16u32.to_le_bytes());
        vbuf.extend(0x58a2u32.to_le_bytes());
        let mut ptch = crate::build_chunk(b"INFO", &info);
        ptch.extend(crate::build_chunk(b"VBUF", &vbuf));
        ptch.extend(crate::build_chunk(b"IBUF", &0u32.to_le_bytes()));
        let mut pchs = crate::build_chunk(b"COMN", &[]);
        for _ in 0..count {
            pchs.extend(crate::build_chunk(b"PTCH", &ptch));
        }
        crate::build_chunk(b"PCHS", &pchs)
    }

    #[test]
    fn xbox_patches_are_bounded_by_the_grid() {
        let parse = |buf: &[u8], n, edge| {
            let c = crate::chunks(buf).next().unwrap().unwrap();
            parse_xbox_patches(&c, n, edge).map(|x| x.masks.len())
        };
        let four = cutter_pchs(4, 1);
        assert_eq!(parse(&four, 16, 8).unwrap(), 4);
        assert!(matches!(
            parse(&cutter_pchs(5, 1), 16, 8),
            Err(UcfbError::Malformed(_))
        ));
        assert!(matches!(
            parse(&cutter_pchs(1, 17), 16, 8),
            Err(UcfbError::Malformed(_))
        ));
        assert!(parse(&four, 0, 8).is_err());
        assert!(parse(&four, 16, 0).is_err());
    }

    #[test]
    fn xbox_weights_become_over_nibbles() {
        let mut out = Vec::new();
        // Two layers listed as (0, 1): half and half.
        weights_to_nibbles(&[0, 1], 128, 0, &[0, 1], &mut out);
        assert_eq!(out, [OPAQUE_NIBBLE, 7]);
        // Listed out of order: the listed-first layer takes what is left.
        out.clear();
        weights_to_nibbles(&[1, 0], 255, 0, &[0, 1], &mut out);
        assert_eq!(out, [OPAQUE_NIBBLE, 0]);
        // Three layers: all weight on the top one.
        out.clear();
        weights_to_nibbles(&[0, 1, 2], 0, 255, &[0, 1, 2], &mut out);
        assert_eq!(out, [OPAQUE_NIBBLE, 0, 14]);
    }
}
