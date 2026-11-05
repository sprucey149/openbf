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
    pub pc_geometry: bool,
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
                pairs.sort_by_key(|p| std::cmp::Reverse(p.0));
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

pub fn is_pc_geometry(segm: &Chunk<'_>) -> bool {
    (segm.child(b"VBUF").is_some()
        || segm.child(b"VDAT").is_some()
        || segm.child(b"IBUF").is_some())
        && !is_xbox_geometry(segm)
}

/// The `VBUF` format bit that says positions are quantised to i16: the Xbox's
/// one vertex layout, where the PC ships float buffers beside any such copy.
const VBUF_PACKED_POSITION: u32 = 0x1000;

/// True when this `segm` holds an Xbox vertex buffer: every `VBUF` packed, and
/// no PC `VDAT`. See [`parse_xbox_geometry`].
pub fn is_xbox_geometry(segm: &Chunk<'_>) -> bool {
    let mut any = false;
    for v in segm.children_tagged(b"VBUF") {
        if v.payload.len() < 12 || le::u32_at(v.payload, 8) & VBUF_PACKED_POSITION == 0 {
            return false;
        }
        any = true;
    }
    any && segm.child(b"VDAT").is_none()
}

/// A D3D "NORMPACKED3": 11 bit x, 11 bit y, 10 bit z, each signed. Verified
/// against the PS2 NORM of the same vertices (`xbox_geometry_matches_the_ps2`).
pub fn unpack_normal(word: u32) -> [f32; 3] {
    let x = ((word << 21) as i32 >> 21) as f32 / 1023.0;
    let y = ((word << 10) as i32 >> 21) as f32 / 1023.0;
    let z = (word as i32 >> 22) as f32 / 511.0;
    [x, y, z]
}

/// Parse an Xbox `segm`. `INFO` is `u32 primitive, u32 vertexCount, u32
/// triangleCount` (the primitive is Xbox D3D's: 5 list, 6 strip) and `IBUF`
/// `u32 count + u16s`, in one strip stitched by repeated indices. `VBUF` is
/// `u32 count, u32 stride, u32 flags` and packed vertices, whose fields follow
/// each other in this order when the flag says so:
///
/// | flag | field | bytes |
/// |---|---|---|
/// | 0x02 | position, i16×3 over the model's vertex box | 6 |
/// | 0x08 | weights: u8 w0, w1 (the third is the rest of 255) | 2 |
/// | 0x04 | bone index: one u8, or three when weights are stored | 1, 3 |
/// | 0x20 | normal, [`unpack_normal`] | 4 |
/// | 0x40 | packed tangent + binormal | 8 |
/// | 0x80 | colour, B G R A | 4 |
/// | 0x100 | second colour (the BF2 vertex light), B G R A | 4 |
/// | 0x200 | uv, i16×2 / 2048 as on the PS2 | 4 |
///
/// A bone index is a BMAP slot, so `index << 2` is the PS2 BIND value.
fn parse_xbox_geometry(
    segm: &Chunk<'_>,
    vertex_count: u32,
    prim_type: u32,
    vbox: &Aabb,
) -> Result<PcGeometry, UcfbError> {
    let bad = || UcfbError::Truncated(segm.offset);
    let n = vertex_count as usize;
    let vp = segm.child(b"VBUF").ok_or_else(bad)?.payload;
    if vp.len() < 12 || le::u32_at(vp, 0) as usize != n {
        return Err(bad());
    }
    let stride = le::u32_at(vp, 4) as usize;
    let flags = le::u32_at(vp, 8);
    let has = |bit: u32| flags & bit != 0;
    let weight_bytes = if has(0x08) { 2 } else { 0 };
    let index_bytes = match (has(0x04), has(0x08)) {
        (false, _) => 0,
        (true, false) => 1,
        (true, true) => 3,
    };
    let weights_at = 6;
    let index_at = weights_at + weight_bytes;
    let normal_at = index_at + index_bytes;
    let tangent_at = normal_at + if has(0x20) { 4 } else { 0 };
    let color_at = tangent_at + if has(0x40) { 8 } else { 0 };
    let light_at = color_at + if has(0x80) { 4 } else { 0 };
    let uv_at = light_at + if has(0x100) { 4 } else { 0 };
    let end = uv_at + if has(0x200) { 4 } else { 0 };
    if !has(0x02)
        || stride != end
        || n.checked_mul(stride).and_then(|b| b.checked_add(12)) != Some(vp.len())
    {
        return Err(UcfbError::Unsupported(format!(
            "xbox VBUF flags {flags:#x}, stride {stride}"
        )));
    }

    let scale: [f32; 3] = std::array::from_fn(|i| (vbox.max[i] - vbox.min[i]) / 65535.0);
    let i16_at = |off: usize| le::u16_at(vp, off) as i16;
    let mut positions = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);
    let mut uvs = Vec::with_capacity(n);
    let mut colors = (has(0x80) || has(0x100)).then(|| Vec::with_capacity(n));
    let mut bind = Vec::new();
    let mut wght = Vec::new();
    for v in 0..n {
        let base = 12 + v * stride;
        positions.push(std::array::from_fn(|i| {
            let q = i32::from(i16_at(base + i * 2)) + 32768;
            vbox.min[i] + q as f32 * scale[i]
        }));
        normals.push(if has(0x20) {
            unpack_normal(le::u32_at(vp, base + normal_at))
        } else {
            [0.0, 1.0, 0.0]
        });
        uvs.push(if has(0x200) {
            [
                f32::from(i16_at(base + uv_at)) / 2048.0,
                f32::from(i16_at(base + uv_at + 2)) / 2048.0,
            ]
        } else {
            [0.0, 0.0]
        });
        if let Some(colors) = colors.as_mut() {
            let at = if has(0x80) { color_at } else { light_at };
            let [b, g, r, a] = [
                vp[base + at],
                vp[base + at + 1],
                vp[base + at + 2],
                vp[base + at + 3],
            ];
            // The PS2's COL0 of the same vertices is exactly half (0xC0 -> 0x60,
            // alpha 0xFF -> 0x7F).
            colors.push([r >> 1, g >> 1, b >> 1, a >> 1]);
        }
        if has(0x04) {
            let idx = &vp[base + index_at..base + index_at + index_bytes];
            if has(0x08) {
                let (w0, w1) = (vp[base + weights_at], vp[base + weights_at + 1]);
                let w2 = 255u8.saturating_sub(w0).saturating_sub(w1);
                // Largest first, as the PS2's WGHT and the PC import keep it.
                let mut pairs = [(w0, idx[0]), (w1, idx[1]), (w2, idx[2])];
                pairs.sort_by_key(|p| std::cmp::Reverse(p.0));
                bind.push([pairs[0].1 << 2, pairs[1].1 << 2, pairs[2].1 << 2, 0]);
                wght.push([pairs[0].0, pairs[1].0, pairs[2].0, 0]);
            } else {
                bind.push([idx[0] << 2, 0, 0, 0]);
                wght.push([255, 0, 0, 0]);
            }
        }
    }

    let ip = segm.child(b"IBUF").ok_or_else(bad)?.payload;
    if ip.len() < 4 {
        return Err(bad());
    }
    let icount = le::u32_at(ip, 0) as usize;
    if ip.len() < icount.saturating_mul(2).saturating_add(4) {
        return Err(bad());
    }
    let index = |i: usize| u32::from(le::u16_at(ip, 4 + i * 2));
    let mut triangles = Vec::with_capacity(icount);
    let mut push = |a: u32, b: u32, c: u32| {
        if a != b && b != c && a != c && a < vertex_count && b < vertex_count && c < vertex_count {
            triangles.push([a, b, c]);
        }
    };
    match prim_type {
        5 => {
            for t in (0..icount.saturating_sub(2)).step_by(3) {
                push(index(t), index(t + 1), index(t + 2));
            }
        }
        6 => {
            for t in 0..icount.saturating_sub(2) {
                if t % 2 == 0 {
                    push(index(t), index(t + 1), index(t + 2));
                } else {
                    push(index(t + 1), index(t), index(t + 2));
                }
            }
        }
        other => {
            return Err(UcfbError::Unsupported(format!("xbox primitive {other}")));
        }
    }

    Ok(PcGeometry {
        index_count: icount as u32,
        bind,
        wght,
        positions,
        normals,
        uvs,
        colors,
        triangles,
    })
}

fn parse_segment(segm: &Chunk<'_>, vbox: &Aabb) -> Result<Segment, UcfbError> {
    let bad = || UcfbError::Truncated(segm.offset);
    let info = segm.child(b"INFO").ok_or_else(bad)?.payload;
    if info.len() < 8 {
        return Err(bad());
    }
    let is_pc = is_pc_geometry(segm);
    let is_xbox = is_xbox_geometry(segm);
    let (vertex_count, index_count, prim_type) = if (is_pc || is_xbox) && info.len() >= 12 {
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
                if (is_pc || is_xbox) && bf2_material {
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

    if is_pc || is_xbox {
        let pc = if is_xbox {
            parse_xbox_geometry(segm, vertex_count, prim_type, vbox)?
        } else {
            parse_pc_geometry(segm, vertex_count, prim_type)?
        };
        return Ok(Segment {
            vertex_count,
            index_count: pc.index_count,
            bf2_material,
            pc_geometry: is_pc,
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
        pc_geometry: false,
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
mod tests;

impl Segment {
    /// Skeleton bone index driving vertex `v` (`BMAP[BIND[v][0] >> 2]`);
    /// `None` for static or hard-skin segments.
    pub fn vertex_bone(&self, v: usize) -> Option<usize> {
        let slot = usize::from(self.bind.get(v)?[0] >> 2);
        self.bmap.get(slot).map(|&b| usize::from(b))
    }

    pub fn uv_scroll(&self) -> Option<[f32; 2]> {
        if self.bf2_material {
            (self.material_flags & 0x0100_0000 != 0 && self.uv_anim != [0, 0])
                .then(|| self.uv_anim.map(|b| -f32::from(b) / 255.0))
        } else if self.material_flags & 0x430 == 0x410 {
            // BF1 ps2ScrollSegment reuses the two MTRL float slots that
            // specular materials interpret as their power/colour words.
            let rate = [
                -f32::from_bits(self.spec_power),
                -f32::from_bits(self.spec_color),
            ];
            (rate != [0.0; 2] && rate.iter().all(|v| v.is_finite())).then_some(rate)
        } else {
            None
        }
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
mod skin_tests;
