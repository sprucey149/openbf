//! PC Battlefront II `tern -> PCHS -> PTCH` terrain patch decoder.
//!
//! PC BF2 terrain patches share `HGT8`, `HEXP`, `LTEX`, `SCAL`, `AXIS`, `ROTN`,
//! and `COMN -> IBUF` with the PS2/Xbox terrain container, but encode per-patch
//! layer blends and vertex colours across multiple DX `VBUF`/`IBUF` streams
//! inside each `PTCH` chunk rather than PS2 `ALPH`/`COLR` RLE streams:
//! - `INFO`: `u8 k`, `[u8; k]` layer indices (`list`), followed by `i16 height_min`,
//!   `u16 height_max`, `u16 shift`.
//! - `VBUF[0]` (stride 16, flags e.g. `0x5122`): `(edge+1)^2` vertices carrying
//!   quantised position, packed weights (`[15]=list[0]`, `[11]=list[1]`,
//!   `[6]=list[2]`), normal, and `[B, G, R]` vertex colour at `[12..15]`.
//! - `VBUF[1]` (stride 28, flags e.g. `0x122`): `(edge+1)^2` float vertices with
//!   `[B, G, R, A]` at `[24..28]` where `A` (`[27]`) is the sequential over-blend
//!   alpha for `list[1]` over `list[0]`.
//! - Additional `(VBUF, IBUF)` pairs (stride 16, low flag byte `0x82`): one per
//!   extra layer `list[2..]`, holding patch-local `(f32 x, y, z)` and `[0,0,0,A]`
//!   where `A` (`[15]`) is that layer's sequential over-blend alpha at the
//!   patch grid vertex `(round(x / grid_unit), round(z / grid_unit))`.

use super::{PatchMesh, OPAQUE_NIBBLE};
use crate::{le, Chunk, UcfbError};

const MAX_LAYERS: usize = 16;
const PC_BASE_STRIDE: usize = 16;
const PC_FLOAT_STRIDE: usize = 28;
const PC_EXTRA_FLAG_LOW: u32 = 0x82;

pub(super) struct PcPatches {
    pub colr: Vec<[u8; 3]>,
    pub masks: Vec<u16>,
    pub meshes: Vec<Option<PatchMesh>>,
    pub alpha: Vec<Vec<u8>>,
}

/// True when `pchs` holds PC BF2 per-patch vertex buffers rather than PS2 RLE
/// streams or Xbox packed-weight buffers (`XBOX_PATCH_WEIGHTS`).
pub(super) fn is_pc_pchs(pchs: &Chunk<'_>) -> bool {
    if super::is_xbox_pchs(pchs) || pchs.child(b"COMN").is_none() {
        return false;
    }
    pchs.children_tagged(b"PTCH")
        .next()
        .and_then(|p| p.child(b"VBUF"))
        .is_some_and(|v| {
            v.payload.len() >= 12
                && matches!(
                    le::u32_at(v.payload, 4) as usize,
                    PC_BASE_STRIDE | PC_FLOAT_STRIDE
                )
        })
}

struct VbufHeader<'a> {
    count: usize,
    stride: usize,
    flags: u32,
    data: &'a [u8],
}

fn parse_vbuf_header<'a>(chunk: &Chunk<'a>) -> Result<VbufHeader<'a>, UcfbError> {
    let p = chunk.payload;
    if p.len() < 12 {
        return Err(UcfbError::Truncated(chunk.offset));
    }
    let count = le::u32_at(p, 0) as usize;
    let stride = le::u32_at(p, 4) as usize;
    let flags = le::u32_at(p, 8);
    let end = count
        .checked_mul(stride)
        .and_then(|b| b.checked_add(12))
        .ok_or_else(|| UcfbError::Malformed("pc terrain VBUF size overflow".into()))?;
    if stride == 0 || end > p.len() {
        return Err(UcfbError::Truncated(chunk.offset));
    }
    Ok(VbufHeader {
        count,
        stride,
        flags,
        data: &p[12..end],
    })
}

fn parse_ibuf_strips(chunk: &Chunk<'_>) -> Result<Vec<u16>, UcfbError> {
    let ip = chunk.payload;
    if ip.len() < 4 {
        return Err(UcfbError::Truncated(chunk.offset));
    }
    let icount = le::u32_at(ip, 0) as usize;
    let end = icount
        .checked_mul(2)
        .and_then(|b| b.checked_add(4))
        .ok_or_else(|| UcfbError::Malformed("pc terrain IBUF size overflow".into()))?;
    if end > ip.len() {
        return Err(UcfbError::Truncated(chunk.offset));
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
    Ok(strips)
}

/// Convert sequential over-blend alphas (`list[0]` opaque, `list[j]` blended
/// over result with alpha `seq_alphas[j]` in `0..=255`) into PS2 ascending-bit
/// "over" nibbles (`0..=14`) ordered along `painted_bits`.
fn seq_alphas_to_nibbles(list: &[u8], seq_alphas: &[u8], painted_bits: &[u8], out: &mut Vec<u8>) {
    let mut weights = [0.0f32; MAX_LAYERS];
    let mut rem = 1.0f32;
    for j in (1..list.len()).rev() {
        let a = f32::from(*seq_alphas.get(j).unwrap_or(&0)) / 255.0;
        let idx = (list[j] as usize) & 15;
        weights[idx] += rem * a;
        rem *= 1.0 - a;
    }
    if let Some(&base) = list.first() {
        weights[(base as usize) & 15] += rem;
    }
    final_weights_to_nibbles(&weights, painted_bits, out);
}

fn final_weights_to_nibbles(weights: &[f32; MAX_LAYERS], painted_bits: &[u8], out: &mut Vec<u8>) {
    let mut under = 0.0f32;
    for (rank, &layer) in painted_bits.iter().enumerate() {
        let own = weights[(layer as usize) & 15].max(0.0);
        under += own;
        if rank == 0 {
            out.push(OPAQUE_NIBBLE);
        } else if under <= 1e-6 {
            out.push(0);
        } else {
            let nib = ((own / under) * 14.0).round().clamp(0.0, 14.0) as u8;
            out.push(nib);
        }
    }
}

/// Parse all `PTCH` chunks inside a PC BF2 `tern -> PCHS` container.
pub(super) fn parse_pc_patches(
    pchs: &Chunk<'_>,
    n: usize,
    edge: usize,
    grid_unit: f32,
) -> Result<PcPatches, UcfbError> {
    let bad = || UcfbError::Truncated(pchs.offset);
    if edge == 0 || n == 0 || !n.is_multiple_of(edge) || !grid_unit.is_finite() || grid_unit <= 0.0
    {
        return Err(bad());
    }
    let side = n / edge;
    let samples = (edge + 1) * (edge + 1);
    let mut colr = vec![[128u8; 3]; n * n];
    let mut masks = Vec::new();
    let mut meshes = Vec::new();
    let mut alpha = Vec::new();

    for (p, patch) in pchs.children_tagged(b"PTCH").enumerate() {
        if p >= side * side {
            return Err(UcfbError::Malformed(format!(
                "pc terrain: more than {side} x {side} patches"
            )));
        }
        let info = patch.child(b"INFO").ok_or_else(bad)?.payload;
        let k = *info.first().ok_or_else(bad)? as usize;
        if k > MAX_LAYERS {
            return Err(UcfbError::Malformed(format!(
                "pc terrain: {k} layers on patch {p}"
            )));
        }
        if info.len() < 1 + k + 6 {
            return Err(bad());
        }
        let list = &info[1..1 + k];
        let mask = list.iter().fold(0u16, |m, &l| m | (1u16 << (l & 15)));
        let painted_bits: Vec<u8> = (0..16u8).filter(|&b| (mask >> b) & 1 != 0).collect();
        let bit_count = painted_bits.len();
        let height_min = le::u16_at(info, 1 + k) as i16;
        let shift = le::u16_at(info, 1 + k + 4);
        masks.push(mask);

        // Classify child VBUFs and check for a cutter replacement IBUF that
        // belongs to the base patch mesh rather than an extra 0x82 blend pass.
        let mut base_vb: Option<VbufHeader<'_>> = None;
        let mut float_vb: Option<VbufHeader<'_>> = None;
        let mut extra_vbs: Vec<VbufHeader<'_>> = Vec::new();
        let mut replacement_ibuf: Option<Chunk<'_>> = None;

        for child in patch.children().flatten() {
            match &child.tag {
                b"VBUF" => {
                    let vb = parse_vbuf_header(&child)?;
                    if vb.stride == PC_BASE_STRIDE
                        && (vb.flags & PC_EXTRA_FLAG_LOW) != PC_EXTRA_FLAG_LOW
                        && base_vb.is_none()
                    {
                        base_vb = Some(vb);
                    } else if vb.stride == PC_FLOAT_STRIDE && float_vb.is_none() {
                        float_vb = Some(vb);
                    } else if vb.stride == PC_BASE_STRIDE {
                        extra_vbs.push(vb);
                    }
                }
                b"IBUF" => {
                    // Only treat IBUF before any extra layer pass as replacement geometry.
                    if extra_vbs.is_empty() && replacement_ibuf.is_none() {
                        replacement_ibuf = Some(child);
                    }
                }
                _ => {}
            }
        }

        // Handle cutter replacement patches when an explicit replacement IBUF
        // follows base VBUF[0]/VBUF[1] and quantized positions are present.
        if let (Some(ibuf), Some(bvb)) = (replacement_ibuf.as_ref(), base_vb.as_ref()) {
            let strips = parse_ibuf_strips(ibuf)?;
            let count = bvb.count;
            let vertex = |v: usize| &bvb.data[v * PC_BASE_STRIDE..(v + 1) * PC_BASE_STRIDE];
            let positions: Vec<[u16; 3]> = (0..count)
                .map(|v| {
                    let vx = vertex(v);
                    std::array::from_fn(|i| le::u16_at(vx, i * 2) ^ 0x8000)
                })
                .collect();
            let repl_colr: Vec<[u8; 3]> = (0..count)
                .map(|v| {
                    let vx = vertex(v);
                    [vx[14], vx[13], vx[12]]
                })
                .collect();
            let mut per_vert = Vec::with_capacity(count * bit_count);
            for v in 0..count {
                let vx = vertex(v);
                let mut w = [0.0f32; MAX_LAYERS];
                for (idx, weight) in [vx[15], vx[11], vx[6]].into_iter().enumerate() {
                    if let Some(&layer) = list.get(idx) {
                        w[(layer as usize) & 15] += f32::from(weight) / 255.0;
                    }
                }
                final_weights_to_nibbles(&w, &painted_bits, &mut per_vert);
            }
            let repl_alpha: Vec<u8> = (0..bit_count)
                .flat_map(|rank| {
                    let buf = &per_vert;
                    (0..count).map(move |v| buf[v * bit_count + rank])
                })
                .collect();
            meshes.push(Some(PatchMesh {
                positions,
                strips,
                height_min,
                shift,
                colr: Some(repl_colr),
                alpha: repl_alpha,
            }));
            alpha.push(vec![OPAQUE_NIBBLE; bit_count * samples]);
            continue;
        }

        // Standard grid patch: verify primary vertex buffer has (edge + 1)^2 samples.
        let primary_ok = base_vb.as_ref().is_none_or(|b| b.count == samples)
            && float_vb.as_ref().is_none_or(|f| f.count == samples);
        if !primary_ok || (base_vb.is_none() && float_vb.is_none()) {
            return Err(bad());
        }

        // Populate RGB vertex color on the grid.
        let (px, pz) = (p % side, p / side);
        let grid_rgb = |vi: usize| -> [u8; 3] {
            if let Some(bvb) = &base_vb {
                let vx = &bvb.data[vi * PC_BASE_STRIDE..];
                [vx[14], vx[13], vx[12]]
            } else if let Some(fvb) = &float_vb {
                let vx = &fvb.data[vi * PC_FLOAT_STRIDE..];
                [vx[26], vx[25], vx[24]]
            } else {
                [128, 128, 128]
            }
        };
        for sz in 0..=edge {
            for sx in 0..=edge {
                let (gx, gz) = (px * edge + sx, pz * edge + sz);
                if gx < n && gz < n {
                    colr[gz * n + gx] = grid_rgb(sz * (edge + 1) + sx);
                }
            }
        }

        if bit_count == 0 {
            meshes.push(None);
            alpha.push(Vec::new());
            continue;
        }
        if bit_count == 1 {
            meshes.push(None);
            alpha.push(vec![OPAQUE_NIBBLE; samples]);
            continue;
        }

        // Map extra 0x82 vertex buffers onto the (edge+1)x(edge+1) patch grid
        // for layers list[2..].
        let mut extra_grids: Vec<Vec<u8>> = Vec::with_capacity(extra_vbs.len());
        for evb in &extra_vbs {
            let mut grid = vec![0u8; samples];
            for v in 0..evb.count {
                let vx = &evb.data[v * evb.stride..];
                if evb.stride >= 16 {
                    let x = le::f32_at(vx, 0);
                    let z = le::f32_at(vx, 8);
                    if x.is_finite() && z.is_finite() {
                        let sx = (x / grid_unit).round().clamp(0.0, edge as f32) as usize;
                        let sz = (z / grid_unit).round().clamp(0.0, edge as f32) as usize;
                        grid[sz * (edge + 1) + sx] = vx[15];
                    }
                }
            }
            extra_grids.push(grid);
        }

        let mut nibbles = Vec::with_capacity(samples * bit_count);
        let mut seq_buf = vec![0u8; k];
        for vi in 0..samples {
            if let Some(fvb) = &float_vb {
                seq_buf[0] = 255;
                if k > 1 {
                    seq_buf[1] = fvb.data[vi * PC_FLOAT_STRIDE + 27];
                }
                for (j, extra) in extra_grids.iter().enumerate() {
                    if j + 2 < k {
                        seq_buf[j + 2] = extra[vi];
                    }
                }
                seq_alphas_to_nibbles(list, &seq_buf, &painted_bits, &mut nibbles);
            } else if let Some(bvb) = &base_vb {
                let vx = &bvb.data[vi * PC_BASE_STRIDE..];
                let mut w = [0.0f32; MAX_LAYERS];
                for (idx, weight) in [vx[15], vx[11], vx[6]].into_iter().enumerate() {
                    if let Some(&layer) = list.get(idx) {
                        w[(layer as usize) & 15] += f32::from(weight) / 255.0;
                    }
                }
                final_weights_to_nibbles(&w, &painted_bits, &mut nibbles);
            }
        }

        let per_layer: Vec<u8> = (0..bit_count)
            .flat_map(|rank| {
                let buf = &nibbles;
                (0..samples).map(move |v| buf[v * bit_count + rank])
            })
            .collect();
        meshes.push(None);
        alpha.push(per_layer);
    }

    Ok(PcPatches {
        colr,
        masks,
        meshes,
        alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_chunk as chunk;

    fn make_pc_vbuf0(colors: [u8; 3], weights: [u8; 3], count: usize) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend((count as u32).to_le_bytes());
        p.extend(16u32.to_le_bytes());
        p.extend(0x5122u32.to_le_bytes());
        for _ in 0..count {
            let mut v = [0u8; 16];
            v[6] = weights[2];
            v[11] = weights[1];
            v[12] = colors[2]; // B
            v[13] = colors[1]; // G
            v[14] = colors[0]; // R
            v[15] = weights[0];
            p.extend_from_slice(&v);
        }
        chunk(b"VBUF", &p)
    }

    fn make_pc_vbuf1(colors: [u8; 3], a1: u8, count: usize) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend((count as u32).to_le_bytes());
        p.extend(28u32.to_le_bytes());
        p.extend(0x0122u32.to_le_bytes());
        for i in 0..count {
            let mut v = [0u8; 28];
            let sx = (i % 9) as f32 * 8.0;
            let sz = (i / 9) as f32 * 8.0;
            v[0..4].copy_from_slice(&sx.to_le_bytes());
            v[8..12].copy_from_slice(&sz.to_le_bytes());
            v[24] = colors[2];
            v[25] = colors[1];
            v[26] = colors[0];
            v[27] = a1;
            p.extend_from_slice(&v);
        }
        chunk(b"VBUF", &p)
    }

    fn make_extra_vbuf(points: &[(f32, f32, u8)]) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend((points.len() as u32).to_le_bytes());
        p.extend(16u32.to_le_bytes());
        p.extend(0x0082u32.to_le_bytes());
        for &(x, z, alpha) in points {
            let mut v = [0u8; 16];
            v[0..4].copy_from_slice(&x.to_le_bytes());
            v[8..12].copy_from_slice(&z.to_le_bytes());
            v[15] = alpha;
            p.extend_from_slice(&v);
        }
        chunk(b"VBUF", &p)
    }

    fn make_ptch(layers: &[u8], body: &[u8]) -> Vec<u8> {
        let mut info = vec![layers.len() as u8];
        info.extend_from_slice(layers);
        info.extend_from_slice(&[0u8; 6]);
        let mut inner = chunk(b"INFO", &info);
        inner.extend_from_slice(body);
        chunk(b"PTCH", &inner)
    }

    #[test]
    fn parses_pc_four_layer_patch_and_vertex_color() {
        let mut pchs_body = chunk(b"COMN", &chunk(b"IBUF", &0u32.to_le_bytes()));
        // Patch 0: 1 layer
        let mut b0 = make_pc_vbuf0([120, 100, 60], [255, 0, 0], 81);
        b0.extend(make_pc_vbuf1([120, 100, 60], 0, 81));
        pchs_body.extend(make_ptch(&[0], &b0));
        // Patch 1: 4 layers [0, 1, 3, 2]
        let mut b1 = make_pc_vbuf0([140, 110, 70], [100, 100, 55], 81);
        b1.extend(make_pc_vbuf1([140, 110, 70], 128, 81));
        b1.extend(make_extra_vbuf(&[(0.0, 0.0, 64)]));
        b1.extend(chunk(b"IBUF", &0u32.to_le_bytes()));
        b1.extend(make_extra_vbuf(&[(8.0, 16.0, 128)]));
        b1.extend(chunk(b"IBUF", &0u32.to_le_bytes()));
        pchs_body.extend(make_ptch(&[0, 1, 3, 2], &b1));
        // Remaining 2 of 4 patches on a 16x16 grid (edge=8)
        for _ in 0..2 {
            pchs_body.extend(make_ptch(&[0], &b0));
        }

        let pchs_bytes = chunk(b"PCHS", &pchs_body);
        let pchs = crate::chunks(&pchs_bytes).next().unwrap().unwrap();
        assert!(is_pc_pchs(&pchs));

        let parsed = parse_pc_patches(&pchs, 16, 8, 8.0).unwrap();
        assert_eq!(parsed.masks, [0b0001, 0b1111, 0b0001, 0b0001]);
        assert_eq!(parsed.colr[0], [120, 100, 60]);
        assert_eq!(parsed.colr[8], [140, 110, 70]);
        // Patch 1 has 4 layers -> 4 * 81 nibbles; lowest ascending layer is OPAQUE_NIBBLE.
        assert_eq!(parsed.alpha[1].len(), 4 * 81);
        assert!(parsed.alpha[1][..81].iter().all(|&a| a == OPAQUE_NIBBLE));
        // At vertex (0,0) on patch 1, layer 3 has alpha ~64/255 (~25%) over (layer0+1).
        assert!(parsed.alpha[1][3 * 81] > 0);
        // At vertex (1,2) -> idx 2*9+1 = 19, layer 2 (rank 2) has alpha ~128/255.
        assert!(parsed.alpha[1][2 * 81 + 19] > 0);
    }

    #[test]
    fn rejects_oversized_or_truncated_pc_patches() {
        let bad_vbuf = chunk(
            b"VBUF",
            &[0xff, 0xff, 0xff, 0x0f, 16, 0, 0, 0, 0x22, 0x51, 0, 0],
        );
        let ptch = make_ptch(&[0], &bad_vbuf);
        let mut pchs_body = chunk(b"COMN", &chunk(b"IBUF", &0u32.to_le_bytes()));
        pchs_body.extend(ptch);
        let pchs_bytes = chunk(b"PCHS", &pchs_body);
        let pchs = crate::chunks(&pchs_bytes).next().unwrap().unwrap();
        assert!(parse_pc_patches(&pchs, 16, 8, 8.0).is_err());

        // Excessive layer count k = 20
        let mut bad_info = vec![20u8];
        bad_info.extend(vec![0u8; 26]);
        let mut bad_ptch = chunk(b"INFO", &bad_info);
        bad_ptch.extend(make_pc_vbuf0([1, 2, 3], [255, 0, 0], 81));
        let mut pchs_body2 = chunk(b"COMN", &chunk(b"IBUF", &0u32.to_le_bytes()));
        pchs_body2.extend(chunk(b"PTCH", &bad_ptch));
        let pchs_bytes2 = chunk(b"PCHS", &pchs_body2);
        let pchs2 = crate::chunks(&pchs_bytes2).next().unwrap().unwrap();
        assert!(matches!(
            parse_pc_patches(&pchs2, 16, 8, 8.0),
            Err(UcfbError::Malformed(_))
        ));
    }
}
