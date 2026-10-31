//! `modl` chunk parsing → dequantized triangle meshes (audit A2 §4).
//!
//! modl INFO: 68 B (BF1) or 72 B (BF2, on PS2 too — one extra leading u32,
//! boxes at +16; A2's "68 B in both" was an erratum). u32[3|4], f32[3]
//! vertexBoxMin, f32[3] vertexBoxMax, f32[3] visBoxMin, f32[3] visBoxMax,
//! u32 faceCount.
//! modl SPHR (optional, 16 B): f32 x y z r, the model's collision sphere —
//! what the PS2 measures a vehicle by (B1 FUN_00497150 B1:558104-558118,
//! overriding the vis-box sphere; `analysis/re-vehicle-death.md` §3 [O]).
//! segm: INFO (u32 vertexCount, u32 indexCount), MTRL (BF1 4/12 B, BF2 25/44 B),
//! RTYP (BF1 string / BF2 u32 — parsed into [`RenderType`]), TNAM (u32 slot +
//! name), _PS2 (u32, ignored),
//! STRP (u16 strip indices; a new strip is marked by bit 0x8000 set on its
//! FIRST TWO indices), POSI (3×u16, dequantized into vertexBox), NORM
//! (3×i8 ÷127), TEX0 (2×i16 ÷2048), COL0 (u32 RGBA, optional),
//! BNAM (hard-skin bone, optional), MNAM (the segment's model group,
//! optional: `override_texture` / `override_texture2`, card USKN).
//!
//! Skinned models (analysis/re-softskin.md, lead-verified): modl INFO word 0
//! = 1 means POSI/NORM are stored in the PRIMARY BONE'S BIND SPACE, not model
//! space. Soft segments add `BMAP` (u32 n + n×u8 skeleton bone indices per
//! VU matrix slot), `WGHT` (4×u8 weights, 255 = 1.0 — every shipped vertex is
//! 255/0/0/0) and `BIND` (4×u8 = slot × 4); `BONE` (u8 slot per vertex) is
//! ignored by both ELFs. `bone = BMAP[BIND[v][0] >> 2]`;
//! `p_model = p_bone · M0[bone]` (row-vector bind matrix from the `skel`).
//! Hard-skin `BNAM` segments are bone-space too, bound to that one bone.

use crate::{le, Chunk, UcfbError};

#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// A segment's `RTYP` chunk: the render type its material asks for.
///
/// The two games encode it differently and the discs are unambiguous —
/// MEASURED over every `segm` of all 243 LVLs on both discs (14,746 segments,
/// none without an RTYP): BF1 writes a NUL-terminated string in a 5/7/8-byte
/// payload (`Normal` 3,988, `Scroll` 312, `Specmap` 179, `Bump` 15, `Detail`
/// 5, `glow` 1) and BF2 a u32 in a 4-byte one (`0` 9,623, `3` 532, `25` 73,
/// `6` 9, `22` 8, `1` 1). Every BF1 segment carries a 4/12-byte MTRL and every
/// BF2 one a 25/44-byte MTRL, so the two encodings never overlap.
///
/// BF2's `3` behaves exactly like BF1's `Scroll` — the alpha census measured
/// P(alpha is a modulation channel, not a mask) = 0.76 for `Scroll` and 0.81
/// for BF2 `3` (PLAN Log "Texture alpha census … ALPHA RULE v2"). What the
/// other BF2 numbers mean is UNTRACED (card GSD, the old HOLD 10(g)); the
/// string is kept verbatim for BF1 and the number verbatim for BF2 so nothing
/// is lost.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RenderType {
    /// No `RTYP` chunk in this `segm` (not observed on either disc).
    #[default]
    Absent,
    /// BF1: the render-type name as written.
    Bf1(String),
    /// BF2: the render-type enum value as written.
    Bf2(u32),
}

impl RenderType {
    /// The ALPHA RULE v2 predicate: this render type uses the texture's alpha
    /// as a scroll/specular MODULATION channel rather than as a mask.
    /// PROVISIONAL — measured correlation, not read out of the ELF (card GSD,
    /// the old HOLD 10(g)). See the PLAN Log entry "Texture alpha census".
    pub fn is_scroll_like(&self) -> bool {
        match self {
            RenderType::Bf1(s) => s == "Scroll" || s == "Specmap",
            RenderType::Bf2(v) => *v == 3,
            RenderType::Absent => false,
        }
    }

    /// A short label for reports and for the GLB's material `extras`:
    /// the BF1 string, the BF2 number as decimal, or the empty string.
    pub fn label(&self) -> String {
        match self {
            RenderType::Bf1(s) => s.clone(),
            RenderType::Bf2(v) => v.to_string(),
            RenderType::Absent => String::new(),
        }
    }
}

#[derive(Clone)]
pub struct Segment {
    pub vertex_count: u32,
    pub index_count: u32,
    /// MTRL is 25 B in BF2 munges (4 B in BF1) — the cheapest source-game tell.
    pub bf2_material: bool,
    /// First u32 of MTRL: the material flags word, same field in both games.
    /// Bit 0x02 marks a cutout ("hardedged") material whose texture alpha is a
    /// mask; bit 0x04 marks a transparent one. MEASURED over all 10,060
    /// segment/texture pairs on the two discs: with 0x02 set the texture is a
    /// real cutout 72-77 % of the time (BF2/BF1), with neither bit set only
    /// 6 %. See RE_LOG "Material flags decide whether texture alpha is a mask".
    pub material_flags: u32,
    /// The `RTYP` chunk of this segment (BF1 string / BF2 u32).
    pub render_type: RenderType,
    /// The MTRL's specular fields, which only the specular second pass reads
    /// (`analysis/re-specular-pass.md` §2 `[O]`); 0 where the MTRL is too
    /// short to carry them. The two games lay them out differently:
    /// - **BF2** (25/44-byte MTRL `{flags, diffuse RGBA, specular RGBA, power,
    ///   ..}`): `spec_color` = bytes 8..12, byte 0 = R; `spec_power` = the
    ///   u32 at bytes 12..16.
    /// - **BF1** (12-byte MTRL `{flags, A, B}`): `spec_power` = A (bytes
    ///   4..8), `spec_color` = B (bytes 8..12, byte 0 = R). A 4-byte MTRL
    ///   carries neither.
    pub spec_power: u32,
    pub spec_color: u32,
    pub uv_anim: [u8; 2],
    /// Texture names by slot (TNAM chunks).
    pub textures: Vec<(u32, String)>,
    /// Hard-skin bone name (BNAM), if present.
    pub bone: Option<String>,
    /// The segment's model group (MNAM), if present: a soldier class's
    /// `OverrideTexture` replaces the texture of the `override_texture`
    /// group, `OverrideTexture2` of `override_texture2` (BF1 `FUN_002013a0`,
    /// BF2 `FUN_00239bb0`; re-soldier-skeleton-skins 9, card USKN).
    pub group: Option<String>,
    /// Softskin: skeleton bone index per VU matrix slot (BMAP).
    pub bmap: Vec<u8>,
    /// Softskin: per-vertex matrix addresses (BIND, slot × 4 each).
    pub bind: Vec<[u8; 4]>,
    /// Softskin: per-vertex weights (WGHT, 255 = 1.0).
    pub wght: Vec<[u8; 4]>,
    /// Dequantized positions (model space).
    pub positions: Vec<[f32; 3]>,
    /// Unit-ish normals (i8/127).
    pub normals: Vec<[f32; 3]>,
    /// UVs (i16/2048).
    pub uvs: Vec<[f32; 2]>,
    /// Vertex colors RGBA8, if COL0 present.
    pub colors: Option<Vec<[u8; 4]>>,
    /// Triangle list indices (unstitched from strips, degenerates dropped).
    pub triangles: Vec<[u32; 3]>,
}

#[derive(Clone)]
pub struct Model {
    pub name: String,
    /// INFO word 0: vertices are stored in bone bind space (skinned models).
    pub bone_space: bool,
    pub vertex_box: Aabb,
    pub vis_box: Aabb,
    pub face_count: u32,
    /// The SPHR chunk's `[x, y, z, r]`, when the modl has one.
    pub sphere: Option<[f32; 4]>,
    pub segments: Vec<Segment>,
}

impl Model {
    /// True when any segment carries the BF2 25-byte material record.
    pub fn is_bf2(&self) -> bool {
        self.segments.iter().any(|s| s.bf2_material)
    }

    pub fn parse(modl: &Chunk<'_>) -> Result<Model, UcfbError> {
        let bad = || UcfbError::Truncated(modl.offset);
        let name = modl.child_string(b"NAME").ok_or_else(bad)?;
        let info = modl.child(b"INFO").ok_or_else(bad)?.payload;
        if info.len() < 68 {
            return Err(bad());
        }
        // BF1 INFO = 68 B (3×u32 then the boxes); BF2 INFO = 72 B with ONE
        // extra leading u32 (the PC-BF2 layout) — byte-verified on TAT2.LVL
        // (every modl 72 B; the 68-B offsets give negative extents and
        // billion-face counts, the +4 offsets give cantina 65×13×38 m).
        // A2's "PS2 BF2 keeps 68 B" was an erratum.
        let base = if info.len() >= 72 { 16 } else { 12 };
        let bone_space = u32::from_le_bytes(info[0..4].try_into().unwrap()) == 1;
        let f32le = |o: usize| le::f32_at(info, o);
        let vec3 = |o: usize| [f32le(o), f32le(o + 4), f32le(o + 8)];
        let vertex_box = Aabb {
            min: vec3(base),
            max: vec3(base + 12),
        };
        let vis_box = Aabb {
            min: vec3(base + 24),
            max: vec3(base + 36),
        };
        let face_count = u32::from_le_bytes(info[base + 48..base + 52].try_into().unwrap());
        let sphere = modl
            .child(b"SPHR")
            .filter(|c| c.payload.len() >= 16)
            .map(|c| std::array::from_fn(|i| le::f32_at(c.payload, 4 * i)));

        let mut segments = Vec::new();
        for segm in modl.children_tagged(b"segm") {
            segments.push(parse_segment(&segm, &vertex_box)?);
        }
        Ok(Model {
            name,
            bone_space,
            vertex_box,
            vis_box,
            face_count,
            sphere,
            segments,
        })
    }
}

struct PcGeometry {
    index_count: u32,
    bind: Vec<[u8; 4]>,
    wght: Vec<[u8; 4]>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Option<Vec<[u8; 4]>>,
    triangles: Vec<[u32; 3]>,
}

fn parse_pc_geometry(
    segm: &Chunk<'_>,
    vertex_count: u32,
    prim_type: u32,
) -> Result<PcGeometry, UcfbError> {
    let bad = || UcfbError::Truncated(segm.offset);
    let n = vertex_count as usize;
    let ip = segm.child(b"IBUF").ok_or_else(bad)?.payload;
    if ip.len() < 4 {
        return Err(bad());
    }
    let icount = le::u32_at(ip, 0) as usize;
    let remaining = ip.len().saturating_sub(4);
    let raw_indices: Vec<u32> = if remaining >= icount * 4 {
        (0..icount).map(|i| le::u32_at(ip, 4 + i * 4)).collect()
    } else if remaining >= icount * 2 {
        (0..icount)
            .map(|i| u32::from(le::u16_at(ip, 4 + i * 2)))
            .collect()
    } else {
        return Err(bad());
    };

    let mut triangles = Vec::new();
    let push_tri = |tris: &mut Vec<[u32; 3]>, a: u32, b: u32, c: u32| {
        if a != b && b != c && a != c && a < vertex_count && b < vertex_count && c < vertex_count {
            tris.push([a, b, c]);
        }
    };
    if prim_type == 5 || (prim_type != 4 && !raw_indices.len().is_multiple_of(3)) {
        for t in 0..raw_indices.len().saturating_sub(2) {
            let (a, b, c) = (raw_indices[t], raw_indices[t + 1], raw_indices[t + 2]);
            if t % 2 == 0 {
                push_tri(&mut triangles, a, b, c);
            } else {
                push_tri(&mut triangles, b, a, c);
            }
        }
    } else {
        for t in (0..raw_indices.len().saturating_sub(2)).step_by(3) {
            push_tri(
                &mut triangles,
                raw_indices[t],
                raw_indices[t + 1],
                raw_indices[t + 2],
            );
        }
    }

    let vbuf_chunk = segm
        .child(b"VDAT")
        .or_else(|| {
            segm.children_tagged(b"VBUF")
                .find(|c| c.payload.len() >= 12 && (le::u32_at(c.payload, 8) & 0xF000) == 0)
        })
        .or_else(|| {
            segm.children_tagged(b"VBUF")
                .filter(|c| c.payload.len() >= 12)
                .last()
        })
        .ok_or_else(bad)?;

    let vp = vbuf_chunk.payload;
    if vp.len() < 12 {
        return Err(bad());
    }
    let stride = le::u32_at(vp, 4) as usize;
    let fvf = le::u32_at(vp, 8);
    let weight_bytes = if (fvf & 0x008) != 0 { 8 } else { 0 };
    let index_bytes = if (fvf & 0x004) != 0 { 4 } else { 0 };
    let norm_offset = 12 + weight_bytes + index_bytes;
    let has_norm = (fvf & 0x020) != 0;
    let norm_bytes = if has_norm { 12 } else { 0 };
    let tan_bytes = if (fvf & 0x040) != 0 { 24 } else { 0 };
    let color0_bytes = if (fvf & 0x080) != 0 { 4 } else { 0 };
    let color1_bytes = if (fvf & 0x100) != 0 { 4 } else { 0 };
    let color_offset = ((fvf & 0x180) != 0).then_some(norm_offset + norm_bytes + tan_bytes);
    let has_uv = (fvf & 0x200) != 0;
    let uv_offset = norm_offset + norm_bytes + tan_bytes + color0_bytes + color1_bytes;
    let min_stride = uv_offset + if has_uv { 8 } else { 0 };
    if stride < min_stride || vp.len() < 12 + n * stride {
        return Err(bad());
    }

    let to_gs = |x: u8| ((u16::from(x) * 128 + 127) / 255) as u8;
    let mut positions = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);
    let mut uvs = Vec::with_capacity(n);
    let mut colors_vec = Vec::with_capacity(if color_offset.is_some() { n } else { 0 });

    for i in 0..n {
        let off = 12 + i * stride;
        positions.push([
            le::f32_at(vp, off),
            le::f32_at(vp, off + 4),
            le::f32_at(vp, off + 8),
        ]);
        normals.push(if has_norm {
            [
                le::f32_at(vp, off + norm_offset),
                le::f32_at(vp, off + norm_offset + 4),
                le::f32_at(vp, off + norm_offset + 8),
            ]
        } else {
            [0.0, 1.0, 0.0]
        });
        uvs.push(if has_uv {
            [
                le::f32_at(vp, off + uv_offset),
                le::f32_at(vp, off + uv_offset + 4),
            ]
        } else {
            [0.0, 0.0]
        });
        if let Some(coff) = color_offset {
            let [b, g, r, a] = [
                vp[off + coff],
                vp[off + coff + 1],
                vp[off + coff + 2],
                vp[off + coff + 3],
            ];
            colors_vec.push([to_gs(r), to_gs(g), to_gs(b), to_gs(a)]);
        }
    }

    let mut bind = Vec::new();
    let mut wght = Vec::new();
    if let Some(skin) = segm.child(b"SKIN").filter(|s| s.payload.len() >= 8) {
        let bpv = le::u32_at(skin.payload, 4);
        if bpv == 1 && skin.payload.len() >= 8 + n {
            for v in 0..n {
                bind.push([skin.payload[8 + v] << 2, 0, 0, 0]);
                wght.push([255, 0, 0, 0]);
            }
        } else if bpv == 3 && skin.payload.len() >= 8 + n * 5 {
            for v in 0..n {
                let base = 8 + v * 5;
                let w0 = skin.payload[base];
                let w1 = skin.payload[base + 1];
                let w2 = 255u8.saturating_sub(w0).saturating_sub(w1);
                let mut pairs = [
                    (w0, skin.payload[base + 2]),
                    (w1, skin.payload[base + 3]),
                    (w2, skin.payload[base + 4]),
                ];
                pairs.sort_by(|a, b| b.0.cmp(&a.0));
                bind.push([pairs[0].1 << 2, pairs[1].1 << 2, pairs[2].1 << 2, 0]);
                wght.push([pairs[0].0, pairs[1].0, pairs[2].0, 0]);
            }
        }
    }

    Ok(PcGeometry {
        index_count: icount as u32,
        bind,
        wght,
        positions,
        normals,
        uvs,
        colors: color_offset.map(|_| colors_vec),
        triangles,
    })
}

fn parse_segment(segm: &Chunk<'_>, vbox: &Aabb) -> Result<Segment, UcfbError> {
    let bad = || UcfbError::Truncated(segm.offset);
    let info = segm.child(b"INFO").ok_or_else(bad)?.payload;
    if info.len() < 8 {
        return Err(bad());
    }
    let is_pc = segm.child(b"VBUF").is_some()
        || segm.child(b"VDAT").is_some()
        || segm.child(b"IBUF").is_some();
    let (vertex_count, index_count, prim_type) = if is_pc && info.len() >= 12 {
        (
            le::u32_at(info, 4),
            le::u32_at(info, 8),
            le::u32_at(info, 0),
        )
    } else {
        (le::u32_at(info, 0), le::u32_at(info, 4), 0u32)
    };
    let n = vertex_count as usize;
    let bf2_material = segm.child(b"MTRL").is_some_and(|c| c.payload.len() >= 25);
    let material_flags = segm
        .child(b"MTRL")
        .filter(|c| c.payload.len() >= 4)
        .map(|c| u32::from_le_bytes(c.payload[0..4].try_into().unwrap()))
        .unwrap_or(0);
    // The specular fields (see `Segment::spec_power`): BF2 spec RGBA at 8
    // and power at 12, BF1 A at 4 and B at 8.
    let (spec_power, spec_color) = segm
        .child(b"MTRL")
        .map(|c| {
            let p = c.payload;
            let word = |at: usize| {
                p.get(at..at + 4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .unwrap_or(0)
            };
            if bf2_material {
                (word(12), word(8))
            } else {
                (word(4), word(8))
            }
        })
        .unwrap_or((0, 0));
    let uv_anim = segm
        .child(b"MTRL")
        .filter(|_| bf2_material)
        .map_or([0, 0], |c| [c.payload[16], c.payload[17]]);
    // RTYP: BF1 a NUL-terminated string, BF2 a u32. Discriminated on the
    // first byte being printable ASCII — the same test the census's
    // `mtrl_census.py` makes, and exact on both discs (BF1's six names all
    // start with a letter; BF2's six values are all < 32).
    let render_type = segm
        .child(b"RTYP")
        .map(|c| {
            let p = c.payload;
            if p.first().is_some_and(|&b| (32..127).contains(&b)) {
                if is_pc && bf2_material {
                    if material_flags & 0x0f00_0000 != 0 {
                        RenderType::Bf2(3)
                    } else {
                        RenderType::Bf2(0)
                    }
                } else {
                    let end = p.iter().position(|&b| b == 0).unwrap_or(p.len());
                    match std::str::from_utf8(&p[..end]) {
                        Ok(s) => RenderType::Bf1(s.to_owned()),
                        Err(_) => RenderType::Absent,
                    }
                }
            } else if p.len() >= 4 {
                RenderType::Bf2(u32::from_le_bytes(p[0..4].try_into().unwrap()))
            } else {
                RenderType::Absent
            }
        })
        .unwrap_or_default();

    let mut textures = Vec::new();
    for t in segm.children_tagged(b"TNAM") {
        let p = t.payload;
        if p.len() >= 5 {
            let slot = u32::from_le_bytes(p[0..4].try_into().unwrap());
            if let Some(end) = p[4..].iter().position(|&b| b == 0) {
                if let Ok(s) = std::str::from_utf8(&p[4..4 + end]) {
                    textures.push((slot, s.to_owned()));
                }
            }
        }
    }
    let group = segm.child(b"MNAM").and_then(|c| {
        let end = c
            .payload
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(c.payload.len());
        std::str::from_utf8(&c.payload[..end])
            .ok()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    });
    let bone = segm.child(b"BNAM").and_then(|c| {
        // BNAM = u32 + name in some variants; sniff a leading printable byte.
        let p = c.payload;
        let start = if p.first().is_some_and(|b| b.is_ascii_graphic()) {
            0
        } else {
            4.min(p.len())
        };
        let sl = &p[start..];
        let end = sl.iter().position(|&b| b == 0)?;
        std::str::from_utf8(&sl[..end]).ok().map(str::to_owned)
    });

    // POSI: 3×u16 dequantized into the model vertex box.
    let bmap = match segm.child(b"BMAP") {
        Some(c) if c.payload.len() >= 4 => {
            let cnt = u32::from_le_bytes(c.payload[0..4].try_into().unwrap()) as usize;
            c.payload.get(4..4 + cnt).ok_or_else(bad)?.to_vec()
        }
        _ => Vec::new(),
    };

    if is_pc {
        let pc = parse_pc_geometry(segm, vertex_count, prim_type)?;
        return Ok(Segment {
            vertex_count,
            index_count: pc.index_count,
            bf2_material,
            material_flags,
            render_type,
            spec_power,
            spec_color,
            uv_anim,
            textures,
            bone,
            group,
            bmap,
            bind: pc.bind,
            wght: pc.wght,
            positions: pc.positions,
            normals: pc.normals,
            uvs: pc.uvs,
            colors: pc.colors,
            triangles: pc.triangles,
        });
    }

    let posi = segm.child(b"POSI").ok_or_else(bad)?.payload;
    if posi.len() < n * 6 {
        return Err(bad());
    }
    let scale: [f32; 3] = std::array::from_fn(|i| (vbox.max[i] - vbox.min[i]) / 65535.0);
    let positions: Vec<[f32; 3]> = (0..n)
        .map(|v| {
            let mut p = [0f32; 3];
            for (i, pi) in p.iter_mut().enumerate() {
                let q = u16::from_le_bytes([posi[v * 6 + i * 2], posi[v * 6 + i * 2 + 1]]);
                *pi = vbox.min[i] + q as f32 * scale[i];
            }
            p
        })
        .collect();

    let normals = match segm.child(b"NORM") {
        Some(c) if c.payload.len() >= n * 3 => (0..n)
            .map(|v| {
                let b = &c.payload[v * 3..v * 3 + 3];
                [
                    b[0] as i8 as f32 / 127.0,
                    b[1] as i8 as f32 / 127.0,
                    b[2] as i8 as f32 / 127.0,
                ]
            })
            .collect(),
        _ => vec![[0.0, 1.0, 0.0]; n],
    };

    let uvs = match segm.child(b"TEX0") {
        Some(c) if c.payload.len() >= n * 4 => (0..n)
            .map(|v| {
                let b = &c.payload[v * 4..v * 4 + 4];
                [
                    i16::from_le_bytes([b[0], b[1]]) as f32 / 2048.0,
                    i16::from_le_bytes([b[2], b[3]]) as f32 / 2048.0,
                ]
            })
            .collect(),
        _ => vec![[0.0, 0.0]; n],
    };

    let colors = segm.child(b"COL0").and_then(|c| {
        (c.payload.len() >= n * 4).then(|| {
            (0..n)
                .map(|v| {
                    let b = &c.payload[v * 4..v * 4 + 4];
                    [b[0], b[1], b[2], b[3]]
                })
                .collect()
        })
    });

    let quads = |tag: &[u8; 4]| -> Vec<[u8; 4]> {
        match segm.child(tag) {
            Some(c) if c.payload.len() >= n * 4 => (0..n)
                .map(|v| {
                    let b = &c.payload[v * 4..v * 4 + 4];
                    [b[0], b[1], b[2], b[3]]
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    let bind = quads(b"BIND");
    let wght = quads(b"WGHT");

    let strp = segm.child(b"STRP").ok_or_else(bad)?.payload;
    if strp.len() < index_count as usize * 2 {
        return Err(bad());
    }
    let raw: Vec<u16> = (0..index_count as usize)
        .map(|i| u16::from_le_bytes([strp[i * 2], strp[i * 2 + 1]]))
        .collect();
    let triangles = unstitch_strips(&raw);

    Ok(Segment {
        vertex_count,
        bf2_material,
        material_flags,
        render_type,
        spec_power,
        spec_color,
        uv_anim,
        index_count,
        textures,
        bone,
        group,
        bmap,
        bind,
        wght,
        positions,
        normals,
        uvs,
        colors,
        triangles,
    })
}

/// Convert PS2 strip indices to a triangle list. A new strip starts where an
/// index AND its successor both have bit 0x8000 set (A2 §4). Winding
/// alternates within a strip; degenerate triangles are dropped.
pub fn unstitch_strips(raw: &[u16]) -> Vec<[u32; 3]> {
    let mut tris = Vec::new();
    let mut i = 0;
    while i + 2 < raw.len() + 1 {
        // find strip start: indices i, i+1 flagged (or start of buffer)
        let strip_start = i;
        i += 2;
        // strip continues until the next flagged PAIR
        while i < raw.len() {
            let flagged = raw[i] & 0x8000 != 0;
            let next_flagged = i + 1 < raw.len() && raw[i + 1] & 0x8000 != 0;
            if flagged && next_flagged && i > strip_start + 1 {
                break;
            }
            i += 1;
        }
        let strip: Vec<u32> = raw[strip_start..i]
            .iter()
            .map(|&x| (x & 0x7FFF) as u32)
            .collect();
        for t in 0..strip.len().saturating_sub(2) {
            let (a, b, c) = (strip[t], strip[t + 1], strip[t + 2]);
            if a == b || b == c || a == c {
                continue;
            }
            // alternate winding
            if t % 2 == 0 {
                tris.push([a, b, c]);
            } else {
                tris.push([b, a, c]);
            }
        }
        if i >= raw.len() {
            break;
        }
    }
    tris
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_chunk as chunk;

    #[test]
    fn unstitch_two_strips() {
        // strip A: 0,1,2,3 ; strip B: 4,5,6 — starts flagged with 0x8000
        let raw = [
            0x8000, 0x8001, 2, 3, // strip A: quads -> tris (0,1,2),(1->2,1,3)
            0x8004, 0x8005, 6, // strip B: (4,5,6)
        ];
        let tris = unstitch_strips(&raw);
        assert_eq!(tris, vec![[0, 1, 2], [2, 1, 3], [4, 5, 6]]);
    }

    #[test]
    fn degenerates_dropped() {
        let raw = [0x8000, 0x8001, 1, 2]; // (0,1,1) degenerate, (1,1,2) degenerate
        assert_eq!(unstitch_strips(&raw), Vec::<[u32; 3]>::new());
    }

    /// One `segm` with the given MTRL and RTYP payloads, one vertex, one
    /// index — the minimum `parse_segment` accepts.
    fn segm(mtrl: &[u8], rtyp: &[u8]) -> Vec<u8> {
        let mut p = chunk(b"INFO", &[1, 0, 0, 0, 1, 0, 0, 0]);
        p.extend(chunk(b"MTRL", mtrl));
        p.extend(chunk(b"RTYP", rtyp));
        p.extend(chunk(b"POSI", &[0u8; 6]));
        p.extend(chunk(b"STRP", &[0u8; 2]));
        chunk(b"segm", &p)
    }

    fn one_segment(mtrl: &[u8], rtyp: &[u8]) -> Segment {
        let mut p = chunk(b"NAME", b"m\0");
        p.extend(chunk(b"INFO", &[0u8; 68]));
        p.extend(segm(mtrl, rtyp));
        let bytes = chunk(b"modl", &p);
        let c = crate::chunks(&bytes).next().unwrap().unwrap();
        let mut m = Model::parse(&c).unwrap();
        m.segments.pop().unwrap()
    }

    /// The MTRL's specular fields in each game's layout
    /// (`analysis/re-specular-pass.md` §2): BF2 `{flags, diffuse, spec RGBA,
    /// power, ..}`, BF1 `{flags, A = power, B = colour}`. Values from the
    /// census (`lead_tmp/re/specular/census_spec.txt`): BF1 `0x31` A 15 B
    /// 0xffc9d0 (SIDE/IMP.LVL), BF2 a 0x80 word with power 300.
    #[test]
    fn the_specular_fields_in_both_layouts() {
        let mut bf1 = Vec::new();
        for w in [0x31u32, 15, 0xffc9d0] {
            bf1.extend(w.to_le_bytes());
        }
        let s = one_segment(&bf1, b"Specmap\0");
        assert_eq!((s.spec_power, s.spec_color), (15, 0xffc9d0));

        let mut bf2 = vec![0u8; 25];
        for (i, w) in [0x80u32, 0xff80_8080, 0xff40_80ff, 300].iter().enumerate() {
            bf2[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let s = one_segment(&bf2, &0u32.to_le_bytes());
        assert_eq!((s.spec_power, s.spec_color), (300, 0xff40_80ff));

        // A 4-byte BF1 MTRL carries neither.
        let s = one_segment(&0x31u32.to_le_bytes(), b"Specmap\0");
        assert_eq!((s.spec_power, s.spec_color), (0, 0));
    }

    #[test]
    fn a_bf2_scroll_word_scrolls_by_its_two_bytes() {
        let mut band = vec![0u8; 25];
        band[0..4].copy_from_slice(&0x0101_0010u32.to_le_bytes());
        band[16..18].copy_from_slice(&[200, 255]);
        let s = one_segment(&band, &3u32.to_le_bytes());
        assert_eq!(s.uv_anim, [200, 255]);
        assert_eq!(s.uv_scroll(), Some([-200.0 / 255.0, -1.0]));
        band[0..4].copy_from_slice(&0x0001_0010u32.to_le_bytes());
        assert_eq!(one_segment(&band, &3u32.to_le_bytes()).uv_scroll(), None);
        let bf1 = one_segment(&0x0100_0410u32.to_le_bytes(), b"Scroll\0");
        assert_eq!((bf1.uv_anim, bf1.uv_scroll()), ([0, 0], None));
    }

    /// RTYP's two on-disc encodings, pinned as raw bytes: BF1 writes a
    /// NUL-terminated string beside a 4-byte MTRL, BF2 a u32 beside a 25-byte
    /// one. Byte layouts taken from the disc (`segm` payloads on KAM1.LVL;
    /// see the survey in the `RenderType` doc).
    #[test]
    fn rtyp_bf1_string_and_bf2_u32() {
        // BF1 kam_prop_crate segment 0: MTRL 0x00000410, RTYP "Scroll\0".
        let bf1 = one_segment(&0x410u32.to_le_bytes(), b"Scroll\0");
        assert_eq!(bf1.material_flags, 0x410);
        assert_eq!(bf1.render_type, RenderType::Bf1("Scroll".into()));
        assert!(bf1.render_type.is_scroll_like());
        assert!(!bf1.bf2_material);

        // BF1 body segment: MTRL 0x00000005, RTYP "Normal\0".
        let body = one_segment(&5u32.to_le_bytes(), b"Normal\0");
        assert_eq!(body.render_type, RenderType::Bf1("Normal".into()));
        assert!(!body.render_type.is_scroll_like());

        // BF1 "Specmap\0" is 8 bytes including the terminator.
        let spec = one_segment(&0x31u32.to_le_bytes(), b"Specmap\0");
        assert_eq!(spec.render_type, RenderType::Bf1("Specmap".into()));
        assert!(spec.render_type.is_scroll_like());

        // BF2 kam_prop_crate segment 0: 25-byte MTRL 0x00010089, RTYP u32 0.
        let mut mtrl = vec![0u8; 25];
        mtrl[0..4].copy_from_slice(&0x1_0089u32.to_le_bytes());
        let bf2 = one_segment(&mtrl, &0u32.to_le_bytes());
        assert_eq!(bf2.material_flags, 0x1_0089);
        assert_eq!(bf2.render_type, RenderType::Bf2(0));
        assert!(!bf2.render_type.is_scroll_like());
        assert!(bf2.bf2_material);

        // BF2's 3 is BF1's Scroll (the census's first evidence for the enum).
        let bf2_scroll = one_segment(&mtrl, &3u32.to_le_bytes());
        assert_eq!(bf2_scroll.render_type, RenderType::Bf2(3));
        assert!(bf2_scroll.render_type.is_scroll_like());
        assert_eq!(bf2_scroll.render_type.label(), "3");

        // No RTYP at all: Absent, never scroll-like.
        let mut p = chunk(b"INFO", &[1, 0, 0, 0, 1, 0, 0, 0]);
        p.extend(chunk(b"MTRL", &7u32.to_le_bytes()));
        p.extend(chunk(b"POSI", &[0u8; 6]));
        p.extend(chunk(b"STRP", &[0u8; 2]));
        let mut modl = chunk(b"NAME", b"m\0");
        modl.extend(chunk(b"INFO", &[0u8; 68]));
        modl.extend(chunk(b"segm", &p));
        let bytes = chunk(b"modl", &modl);
        let c = crate::chunks(&bytes).next().unwrap().unwrap();
        let seg = &Model::parse(&c).unwrap().segments[0];
        assert_eq!(seg.render_type, RenderType::Absent);
        assert!(!seg.render_type.is_scroll_like());
        assert_eq!(seg.render_type.label(), "");
    }

    /// Data-gated Tier-1: the RTYP reading agrees with the census's Python
    /// walk (`engine/artifacts/texture_census/mtrl_census.py`) on the PLAN
    /// entry's known case — `kam_prop_crate`'s four BF1 segments read
    /// `Scroll / Normal / Normal / Specmap` with flags `0x410 / 0x05 / 0x01 /
    /// 0x31` IN SEGMENT ORDER, and BF2's two read RTYP 0 with flags
    /// `0x10089 / 0x10001`.
    #[test]
    fn rtyp_matches_the_census_on_kam_prop_crate() {
        fn segs(path: &str, want: &str) -> Option<Vec<(u32, RenderType)>> {
            let buf = std::fs::read(path).ok()?;
            let inner = crate::open(&buf).unwrap();
            for c in crate::chunks(inner) {
                let c = c.unwrap();
                if &c.tag != b"modl" {
                    continue;
                }
                let m = Model::parse(&c).unwrap();
                if m.name.eq_ignore_ascii_case(want) {
                    return Some(
                        m.segments
                            .iter()
                            .map(|s| (s.material_flags, s.render_type.clone()))
                            .collect(),
                    );
                }
            }
            None
        }
        let k1 = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/KAM/KAM1.LVL"
        );
        let k2 = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/KAM/KAM1.LVL"
        );
        if !std::path::Path::new(k1).exists() || !std::path::Path::new(k2).exists() {
            eprintln!("SKIP: extracted KAM1 not present");
            return;
        }
        let bf1 = |s: &str| RenderType::Bf1(s.to_owned());
        assert_eq!(
            segs(k1, "kam_prop_crate").expect("BF1 kam_prop_crate"),
            vec![
                (0x410, bf1("Scroll")),
                (0x05, bf1("Normal")),
                (0x01, bf1("Normal")),
                (0x31, bf1("Specmap")),
            ]
        );
        assert_eq!(
            segs(k2, "kam_prop_crate").expect("BF2 kam_prop_crate"),
            vec![
                (0x1_0089, RenderType::Bf2(0)),
                (0x1_0001, RenderType::Bf2(0))
            ]
        );
    }

    /// Data-gated Tier-1: the BF2 72-byte INFO layout (extra leading u32)
    /// dequantizes Mos Eisley models to their real extents; BF1 unchanged.
    #[test]
    fn modl_info_layout_bf1_68_vs_bf2_72() {
        fn extents(path: &str, want: &str) -> Option<[f32; 3]> {
            let buf = std::fs::read(path).ok()?;
            let inner = crate::open(&buf).unwrap();
            for c in crate::content_chunks(inner).unwrap() {
                if &c.tag == b"modl" {
                    let m = Model::parse(&c).unwrap();
                    if m.name == want {
                        let b = m.vertex_box;
                        return Some([
                            b.max[0] - b.min[0],
                            b.max[1] - b.min[1],
                            b.max[2] - b.min[2],
                        ]);
                    }
                }
            }
            None
        }
        let t2 = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/TAT/TAT2.LVL"
        );
        let t1 = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL"
        );
        if !std::path::Path::new(t2).exists() || !std::path::Path::new(t1).exists() {
            eprintln!("SKIP: extracted TAT1/TAT2 not present");
            return;
        }
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 0.6);
        let cantina = extents(t2, "tat2_bldg_cantina").expect("cantina");
        assert!(near(cantina, [65.0, 13.0, 37.7]), "cantina {cantina:?}");
        let dock = extents(t2, "tat2_bldg_dockingbay_02").expect("dockingbay");
        assert!(near(dock, [39.5, 15.5, 41.1]), "dockingbay {dock:?}");
        let crawler = extents(t1, "tat1_bldg_crawler").expect("crawler");
        assert!(near(crawler, [23.0, 30.7, 59.1]), "crawler {crawler:?}");
    }
    #[test]
    fn pc_model_parses_d3d_buffers() {
        let mut info_payload = Vec::new();
        info_payload.extend(5u32.to_le_bytes());
        info_payload.extend(3u32.to_le_bytes());
        info_payload.extend(1u32.to_le_bytes());

        let mut ibuf_payload = Vec::new();
        ibuf_payload.extend(3u32.to_le_bytes());
        ibuf_payload.extend(0u16.to_le_bytes());
        ibuf_payload.extend(1u16.to_le_bytes());
        ibuf_payload.extend(2u16.to_le_bytes());

        let mut vbuf_payload = Vec::new();
        vbuf_payload.extend(3u32.to_le_bytes());
        vbuf_payload.extend(32u32.to_le_bytes());
        vbuf_payload.extend(0x222u32.to_le_bytes());
        for i in 0..3 {
            vbuf_payload.extend((i as f32).to_le_bytes());
            vbuf_payload.extend(0.0f32.to_le_bytes());
            vbuf_payload.extend(0.0f32.to_le_bytes());
            vbuf_payload.extend(0.0f32.to_le_bytes());
            vbuf_payload.extend(1.0f32.to_le_bytes());
            vbuf_payload.extend(0.0f32.to_le_bytes());
            vbuf_payload.extend(0.5f32.to_le_bytes());
            vbuf_payload.extend(0.5f32.to_le_bytes());
        }

        let mut segm_p = chunk(b"INFO", &info_payload);
        segm_p.extend(chunk(b"IBUF", &ibuf_payload));
        segm_p.extend(chunk(b"VBUF", &vbuf_payload));

        let mut modl_p = chunk(b"NAME", b"test_pc\0");
        modl_p.extend(chunk(b"INFO", &[0u8; 72]));
        modl_p.extend(chunk(b"segm", &segm_p));

        let bytes = chunk(b"modl", &modl_p);
        let c = crate::chunks(&bytes).next().unwrap().unwrap();
        let m = Model::parse(&c).unwrap();
        assert_eq!(m.name, "test_pc");
        assert_eq!(m.segments.len(), 1);
        let s = &m.segments[0];
        assert_eq!(s.vertex_count, 3);
        assert_eq!(s.positions.len(), 3);
        assert_eq!(s.normals.len(), 3);
        assert_eq!(s.uvs.len(), 3);
        assert_eq!(s.triangles.len(), 1);
        assert_eq!(s.triangles[0], [0, 1, 2]);
    }

    #[test]
    fn pc_model_parses_colors_skin_bpv3_and_bf2_rtyp() {
        let mut info_payload = Vec::new();
        info_payload.extend(4u32.to_le_bytes());
        info_payload.extend(3u32.to_le_bytes());
        info_payload.extend(1u32.to_le_bytes());

        let mut ibuf_payload = Vec::new();
        ibuf_payload.extend(3u32.to_le_bytes());
        ibuf_payload.extend(0u16.to_le_bytes());
        ibuf_payload.extend(1u16.to_le_bytes());
        ibuf_payload.extend(2u16.to_le_bytes());

        let mut vbuf_payload = Vec::new();
        vbuf_payload.extend(3u32.to_le_bytes());
        vbuf_payload.extend(60u32.to_le_bytes());
        vbuf_payload.extend(0x362u32.to_le_bytes());
        for i in 0..3 {
            for f in [i as f32, 1.0, 2.0, 0.0, 1.0, 0.0] {
                vbuf_payload.extend(f.to_le_bytes());
            }
            vbuf_payload.extend([0u8; 24]);
            vbuf_payload.extend([77u8, 128, 221, 255]);
            vbuf_payload.extend(0.25f32.to_le_bytes());
            vbuf_payload.extend(0.75f32.to_le_bytes());
        }

        let mut skin_payload = Vec::new();
        skin_payload.extend(3u32.to_le_bytes());
        skin_payload.extend(3u32.to_le_bytes());
        for _ in 0..3 {
            skin_payload.extend([10u8, 45, 1, 2, 3]);
        }

        let mut mtrl = vec![0u8; 25];
        mtrl[0..4].copy_from_slice(&0x0101_0080u32.to_le_bytes());

        let mut segm_p = chunk(b"INFO", &info_payload);
        segm_p.extend(chunk(b"MTRL", &mtrl));
        segm_p.extend(chunk(b"RTYP", b"Normal\0"));
        segm_p.extend(chunk(b"BMAP", &[4, 0, 0, 0, 5, 6, 7, 8]));
        segm_p.extend(chunk(b"SKIN", &skin_payload));
        segm_p.extend(chunk(b"IBUF", &ibuf_payload));
        segm_p.extend(chunk(b"VBUF", &vbuf_payload));

        let mut modl_p = chunk(b"NAME", b"test_pc_full\0");
        modl_p.extend(chunk(b"INFO", &[0u8; 72]));
        modl_p.extend(chunk(b"segm", &segm_p));

        let bytes = chunk(b"modl", &modl_p);
        let c = crate::chunks(&bytes).next().unwrap().unwrap();
        let m = Model::parse(&c).unwrap();
        let s = &m.segments[0];
        assert_eq!(s.render_type, RenderType::Bf2(3));
        assert_eq!(s.index_count, 3);
        assert_eq!(s.uvs[0], [0.25, 0.75]);
        assert_eq!(s.colors.as_ref().unwrap()[0], [111, 64, 39, 128]);
        assert_eq!(s.wght[0], [200, 45, 10, 0]);
        assert_eq!(s.bind[0], [12, 8, 4, 0]);
        assert_eq!(s.vertex_bone(0), Some(8));
    }
}

impl Segment {
    /// Skeleton bone index driving vertex `v` (`BMAP[BIND[v][0] >> 2]`);
    /// `None` for static or hard-skin segments.
    pub fn vertex_bone(&self, v: usize) -> Option<usize> {
        let slot = usize::from(self.bind.get(v)?[0] >> 2);
        self.bmap.get(slot).map(|&b| usize::from(b))
    }

    pub fn uv_scroll(&self) -> Option<[f32; 2]> {
        (self.bf2_material && self.material_flags & 0x0100_0000 != 0 && self.uv_anim != [0, 0])
            .then(|| self.uv_anim.map(|b| -f32::from(b) / 255.0))
    }
}

/// (model-space positions, model-space normals, skeleton bone per vertex).
pub type ModelSpaceSegment = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<usize>);

/// Model-space positions, normals and per-vertex skeleton bone for a segment
/// of a bone-space model: soft segments through BMAP/BIND, hard-skin (BNAM)
/// segments through the named bone. Errors on a BNAM name missing from the
/// skeleton (the engine silently uses bone 0 — re-softskin.md open item 4).
pub fn to_model_space(
    model: &Model,
    seg: &Segment,
    skel: &crate::skel::Skeleton,
) -> Result<ModelSpaceSegment, UcfbError> {
    let m0 = skel.bind_matrices()?;
    let n = seg.positions.len();
    let hard = match &seg.bone {
        Some(name) => Some(skel.index_of(name).ok_or_else(|| {
            UcfbError::Unsupported(format!("BNAM {name} not in skel {}", skel.name))
        })?),
        None => None,
    };
    let mut bones = Vec::with_capacity(n);
    for v in 0..n {
        let b = match hard {
            Some(b) => b,
            None => seg.vertex_bone(v).ok_or_else(|| {
                UcfbError::Unsupported(format!("{}: vertex {v} has no bone", model.name))
            })?,
        };
        if b >= m0.len() {
            return Err(UcfbError::Unsupported(format!(
                "{}: bone {b} out of range",
                model.name
            )));
        }
        bones.push(b);
    }
    if !model.bone_space {
        return Ok((seg.positions.clone(), seg.normals.clone(), bones));
    }
    let positions = (0..n)
        .map(|v| crate::skel::xform_point(&m0[bones[v]], seg.positions[v]))
        .collect();
    let normals = (0..n)
        .map(|v| crate::skel::xform_dir(&m0[bones[v]], seg.normals[v]))
        .collect();
    Ok((positions, normals, bones))
}

#[cfg(test)]
mod skin_tests {
    use super::*;

    fn close3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < tol)
    }

    fn find(inner: &[u8], name: &str) -> (Model, crate::skel::Skeleton) {
        let chunks = crate::content_chunks(inner).unwrap();
        let modl = chunks
            .iter()
            .filter(|c| &c.tag == b"modl")
            .map(Model::parse)
            .filter_map(Result::ok)
            .find(|m| m.name == name)
            .unwrap();
        let skel = chunks
            .iter()
            .filter(|c| &c.tag == b"skel")
            .map(crate::skel::parse_skel)
            .filter_map(Result::ok)
            .find(|s| s.name == name)
            .unwrap();
        (modl, skel)
    }

    fn aabb(p: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in p {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        (min, max)
    }

    /// re-softskin.md §10 vectors 6–9: BF1 stormtrooper.
    #[test]
    fn imp_lvl_stormtrooper_softskin() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: IMP.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let (m, skel) = find(inner, "imp_inf_stormtrooper");
        assert!(m.bone_space);
        assert!(close3(
            m.vertex_box.min,
            [-0.197186, -0.206013, -0.209799],
            1e-5
        ));
        assert!(close3(m.vis_box.max, [0.929476, 2.08769, 0.215175], 1e-5));
        assert_eq!(m.segments.len(), 1);
        let seg = &m.segments[0];
        assert_eq!((seg.vertex_count, seg.index_count), (1387, 2370));
        assert_eq!(seg.bmap.len(), 21);
        assert_eq!(&seg.bmap[..4], &[1, 2, 3, 4]);
        assert_eq!(&seg.bmap[19..], &[0x15, 0x16]);
        assert_eq!(seg.wght[0], [255, 0, 0, 0]);
        assert!(seg.wght.iter().all(|w| *w == [255, 0, 0, 0]));
        assert_eq!(seg.bind[0], [0x38, 0x34, 0x34, 0x34]);
        assert_eq!(seg.bind[2], [0x48, 0x44, 0x44, 0x44]);
        assert_eq!(seg.vertex_bone(0), Some(15)); // bone_r_toe
        assert_eq!(seg.vertex_bone(2), Some(19)); // bone_l_toe
        assert_eq!(seg.vertex_bone(150), Some(12)); // bone_r_thigh
        assert!(close3(seg.positions[0], [0.0634, 0.0353, 0.0857], 1e-3));
        let (pos, _nrm, bones) = to_model_space(&m, seg, &skel).unwrap();
        assert_eq!(bones[0], 15);
        assert!(
            close3(pos[0], [-0.1770, -0.0012, 0.1237], 1e-3),
            "{:?}",
            pos[0]
        );
        assert!(close3(pos[2], [0.1769, -0.0010, 0.1237], 1e-3));
        assert!(close3(pos[150], [-0.0990, 0.6066, 0.0135], 1e-3));
        let (min, max) = aabb(&pos);
        assert!(
            close3(min, m.vis_box.min, 1e-4),
            "{min:?} vs {:?}",
            m.vis_box.min
        );
        assert!(
            close3(max, m.vis_box.max, 1e-4),
            "{max:?} vs {:?}",
            m.vis_box.max
        );
        assert_eq!(seg.triangles.len(), 1630);
    }

    /// §10 vector 12: BF2 ep3 trooper (72-B INFO, different bone order).
    #[test]
    fn rep_lvl_ep3trooper_softskin() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: REP.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let (m, skel) = find(inner, "rep_inf_ep3trooper");
        assert!(m.bone_space);
        let seg = &m.segments[0];
        assert_eq!((seg.vertex_count, seg.index_count), (1710, 2698));
        assert_eq!(seg.bmap.len(), 21);
        assert_eq!(seg.vertex_bone(0), Some(17));
        let (pos, _, _) = to_model_space(&m, seg, &skel).unwrap();
        assert!(
            close3(pos[0], [-0.1795, -0.0012, 0.1156], 1e-3),
            "{:?}",
            pos[0]
        );
        assert!(close3(pos[2], [0.1794, -0.0010, 0.1156], 1e-3));
        let (min, max) = aabb(&pos);
        assert!(close3(min, [-0.92948, -0.00120, -0.23314], 1e-3), "{min:?}");
        assert!(close3(max, [0.92948, 2.08267, 0.20353], 1e-3), "{max:?}");
    }
}
