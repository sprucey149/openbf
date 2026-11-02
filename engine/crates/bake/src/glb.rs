//! Minimal GLB (glTF 2.0 binary) writer for baked models. Hand-rolled JSON via
//! serde_json; validated by round-trip with the `gltf` reader crate in tests.
//!
//! Materials carry a NAME (the source texture name) — the runtime maps
//! material names to baked texture files via the RON manifest — plus the
//! ALPHA RULE v2 verdict for that primitive: glTF `alphaMode`
//! (OPAQUE/MASK/BLEND), `alphaCutoff` on MASK, and the inputs the verdict was
//! taken on in `extras` (flags, rtyp, mid, a0, amax, rule) so a census can
//! re-derive every verdict from the GLBs alone. No image data is embedded.
//!
//! One material per PRIMITIVE, so two primitives sharing a texture but
//! disagreeing on the verdict get two materials with the same `name` — which
//! is the point of moving the decision per draw (`crate::alpha`).
//!
//! A MAP bake also writes the PS2's per-vertex lighting inputs (queue LITEB,
//! `analysis/re-model-lighting.md` §2/§3/§6 `[O]`): the disc's COL0 as
//! `COLOR_0` and the segment's VU1 lighting path as `extras.light`
//! ([`LightPath`]). Common, side and sky-object models do not (`vertex_light`
//! off), so their GLBs are what they were.

use anyhow::Result;
use mungefs::model::Model;
use serde_json::json;

use crate::gltf_bin::{flat3, min_max3, Bin};

/// Serialize one Model (all segments as primitives of one mesh) to GLB bytes.
/// `alphas` is the ALPHA RULE v2 verdict per segment, aligned with
/// `m.segments` (`crate::alpha::AlphaTable::model`). Each segment's specular
/// second pass is written as `extras.spec_pass` (`crate::spec`) — every bake
/// since queue SPEC2: map, common and the side weapons.
///
/// `vertex_light` writes each segment's COL0 as `COLOR_0` ([`col0_linear`])
/// and its [`LightPath`] as `extras.light` — the map bake's world models only.
pub fn model_to_glb(
    m: &Model,
    alphas: &[crate::alpha::SegmentAlpha],
    vertex_light: bool,
) -> Result<Vec<u8>> {
    write_model(m, alphas, vertex_light, false, None, false, vertex_light)
}

pub fn station_model_to_glb(m: &Model, alphas: &[crate::alpha::SegmentAlpha]) -> Result<Vec<u8>> {
    write_model(m, alphas, false, false, None, false, true)
}

/// [`model_to_glb`] for a HUD mesh (card BHUD2): each segment's COL0 goes out
/// as `COLOR_0` in DISPLAY values with its alpha — GS units, 0x80 = 1.0, not
/// linearised — because the HUD pass multiplies and blends display values
/// (`render::hud`). The weapon backdrops are shaped by it: grey at 83/128
/// fading to alpha 0 along the pill.
pub fn hud_model_to_glb(m: &Model, alphas: &[crate::alpha::SegmentAlpha]) -> Result<Vec<u8>> {
    write_model(m, alphas, false, false, None, true, false)
}

/// [`model_to_glb`] for a map bake's world model, `vertex_light` on, on a map
/// whose BF2 `Effect("HDR")` is enabled when `hdr` (card GLOW): a self-lit
/// segment's `extras.light` is then [`glow_path`]'s, and its `COLOR_0` alpha
/// carries the VU1's vertex alpha (see [`glow_path`]).
pub fn world_model_to_glb(
    m: &Model,
    alphas: &[crate::alpha::SegmentAlpha],
    hdr: bool,
) -> Result<Vec<u8>> {
    write_model(m, alphas, true, hdr, None, false, true)
}

/// [`model_to_glb`], plus — when `joints` is given, one list per segment of
/// `m` — each vertex's skeleton bone as `JOINTS_0` (weight 1 in
/// `WEIGHTS_0`): a walker's model (card WSKN), still written in model space
/// at the bind pose, which the runtime skins with the skeleton and clips
/// baked beside it (`crate::walker::skin_glb`). No node carries a skin, so
/// any other reader draws it rigid, as before.
pub fn model_to_glb_joints(
    m: &Model,
    alphas: &[crate::alpha::SegmentAlpha],
    vertex_light: bool,
    joints: Option<&[Vec<usize>]>,
) -> Result<Vec<u8>> {
    write_model(m, alphas, vertex_light, false, joints, false, vertex_light)
}

fn write_model(
    m: &Model,
    alphas: &[crate::alpha::SegmentAlpha],
    vertex_light: bool,
    hdr: bool,
    joints: Option<&[Vec<usize>]>,
    hud_colors: bool,
    sphere: bool,
) -> Result<Vec<u8>> {
    let mut bin = Bin::default();
    let mut primitives = Vec::new();
    let mut materials = Vec::new();

    for (si, seg) in m.segments.iter().enumerate() {
        if seg.triangles.is_empty() {
            continue;
        }
        let (min, max) = min_max3(&seg.positions);
        let pos_acc = bin.f32s(
            &flat3(&seg.positions),
            "VEC3",
            json!({"min": min, "max": max}),
        );
        // normalize defensively (i8 quantization)
        let normals: Vec<f32> = seg
            .normals
            .iter()
            .flat_map(|&nrm| {
                let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
                let s = if len > 1e-6 { 1.0 / len } else { 0.0 };
                nrm.map(|v| v * s)
            })
            .collect();
        let nrm_acc = bin.f32s(&normals, "VEC3", json!({}));
        let uvs: Vec<f32> = seg.uvs.iter().flatten().copied().collect();
        let uv_acc = bin.f32s(&uvs, "VEC2", json!({}));
        let idx: Vec<u32> = seg.triangles.iter().flatten().copied().collect();
        let idx_acc = bin.u32s(&idx, json!({}));

        let tex_name = crate::alpha::segment_texture(seg).unwrap_or_default();
        let a = alphas
            .get(si)
            .ok_or_else(|| anyhow::anyhow!("{}: no alpha verdict for segment {si}", m.name))?;
        let mut mat = json!({
            "name": tex_name,
            "alphaMode": a.mode.as_str(),
            "extras": {
                "flags": a.flags,
                "rtyp": a.rtyp.label(),
                "mid": a.stats.map(|s| s.mid),
                "a0": a.stats.map(|s| s.a0),
                "amax": a.stats.map(|s| s.amax),
                "rule": a.rule.as_str(),
            },
        });
        if let Some(p) = crate::spec::spec_pass(seg) {
            mat["extras"]["spec_pass"] = p.to_json();
        }
        if let Some(v) = seg.uv_scroll() {
            mat["extras"]["uv_scroll"] = json!(v);
        }
        let glow = if vertex_light {
            glow_path(seg, hdr)
        } else {
            None
        };
        if vertex_light {
            let path = glow.unwrap_or_else(|| light_path(m, seg));
            mat["extras"]["light"] = json!(path.as_str());
        }
        if a.mode == crate::alpha::AlphaMode::Mask {
            // glTF's own default, written explicitly rather than implied.
            // It is NOT the threshold the shader tests at — the world's
            // cutouts test at the disc's `MASK_CUTOFF` (0.753) since queue
            // AT — and no reader consumes it: `game::assets` routes on
            // `alphaMode` alone. Moving it would rebake every model GLB for
            // a number nothing reads.
            mat["alphaCutoff"] = json!(0.5);
        }
        materials.push(mat);

        let mut attributes = json!({"POSITION": pos_acc, "NORMAL": nrm_acc, "TEXCOORD_0": uv_acc});
        if let Some(c) = seg.colors.as_ref().filter(|_| vertex_light) {
            let glow_av = glow == Some(LightPath::Glow);
            let lin: Vec<f32> = c
                .iter()
                .flat_map(|v| {
                    let mut l = col0_linear(*v);
                    if glow_av {
                        l[3] = glow_vertex_alpha(v[3]);
                    }
                    l
                })
                .collect();
            attributes["COLOR_0"] = json!(bin.f32s(&lin, "VEC4", json!({})));
        }
        if let Some(c) = seg.colors.as_ref().filter(|_| hud_colors) {
            let raw: Vec<f32> = c
                .iter()
                .flat_map(|v| v.map(|b| (f32::from(b) / 128.0).min(1.0)))
                .collect();
            attributes["COLOR_0"] = json!(bin.f32s(&raw, "VEC4", json!({})));
        }
        if let Some(bones) = joints.map(|j| &j[si]) {
            if bones.len() != seg.positions.len() || bones.iter().any(|&b| b > 255) {
                anyhow::bail!("{}: segment {si}'s bones do not fit JOINTS_0", m.name);
            }
            let j: Vec<u8> = bones.iter().flat_map(|&b| [b as u8, 0, 0, 0]).collect();
            let w: Vec<f32> = bones.iter().flat_map(|_| [1.0, 0.0, 0.0, 0.0]).collect();
            attributes["JOINTS_0"] = json!(bin.accessor(&j, 5121, bones.len(), "VEC4", json!({})));
            attributes["WEIGHTS_0"] = json!(bin.f32s(&w, "VEC4", json!({})));
        }
        primitives.push(json!({
            "attributes": attributes,
            "indices": idx_acc,
            "material": materials.len()-1,
            "mode": 4
        }));
    }

    let mut mesh = json!({"name": m.name, "primitives": primitives});
    // The vis box's top: a vehicle's camera pulls in toward a point scaled from it
    // (`game::walk::vehicle_pull_height`), and it can stand far above the vertices.
    mesh["extras"] = json!({ "vis_top": m.vis_box.max[1] });
    // Card BPNT: a world model's sphere, which the PS2 gathers its local
    // lights by (`model_sphere`).
    if sphere {
        mesh["extras"]["sphere"] = json!(model_sphere(m));
    }
    crate::gltf_bin::rigid_glb(bin, mesh, &m.name, Some(json!(materials)))
}

/// The model's sphere `[x, y, z, r]` in model space: its `SPHR` chunk, else
/// the sphere of its vis box (centre, half the diagonal). The PS2 gathers a
/// model draw's local lights by the sphere at model +0x70 (B2 `FUN_0057cf60`
/// 712763–712781, `analysis/re-point-lights.md` §5); that it is this one is
/// PROVISIONAL [D] — BF1 loads `SPHR` over the vis-box sphere
/// (`analysis/re-vehicle-death.md` §3), and on BF2 UTA1 it picks the spot the
/// PS2's floor shows.
pub fn model_sphere(m: &Model) -> [f32; 4] {
    m.sphere.unwrap_or_else(|| {
        let (lo, hi) = (m.vis_box.min, m.vis_box.max);
        let d: f32 = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum();
        [
            (lo[0] + hi[0]) * 0.5,
            (lo[1] + hi[1]) * 0.5,
            (lo[2] + hi[2]) * 0.5,
            d.sqrt() * 0.5,
        ]
    })
}

/// How the PS2's VU1 lights one segment of a world model — the program the
/// segment's draw takes and the light set its model uploads
/// (`analysis/re-model-lighting.md` §2/§3/§6, `[O]`). Written per primitive as
/// `extras.light`; `game::assets` reads it into `render::LightPath`, whose
/// docs give each path's formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightPath {
    /// The hemisphere ambient plus every directional light, then × COL0
    /// (VU1 0x1bb/0x1e6 then 0x521).
    Lit,
    /// A PRELIT set (§3): no ambient, no `Static()` light, then × COL0.
    Prelit,
    /// A prelit set, and COL0 ADDED rather than multiplied (VU1 0x537, the
    /// "vc-lit" prim bit 0x2000).
    PrelitAdd,
    /// MTRL bit 0x01 clear: no lighting, the material constant × COL0 (VU1
    /// 0x2a3 → 0x2da).
    Unlit,
    /// Unlit on a vc-lit segment: the material constant PLUS COL0 (VU1 0x2a3
    /// → 0x2bf).
    UnlitAdd,
    VertexLit,
    /// BF2 self-lit (card GLOW, [`glow_path`]): unlit, then × (1 + As/128)
    /// in display units, clamped.
    Glow,
    /// BF2 MTRL 0x14 on an HDR map: unlit, drawn ×2 in display units.
    Unscaled,
}

impl LightPath {
    pub fn as_str(self) -> &'static str {
        match self {
            LightPath::Lit => "lit",
            LightPath::Prelit => "prelit",
            LightPath::PrelitAdd => "prelit_add",
            LightPath::Unlit => "unlit",
            LightPath::UnlitAdd => "unlit_add",
            LightPath::VertexLit => "vertex_lit",
            LightPath::Glow => "glow",
            LightPath::Unscaled => "unscaled",
        }
    }
}

/// The [`LightPath`] of `seg`, a segment of `m`. The two games reach the same
/// VU1 programs by different bits (`analysis/re-model-lighting.md`):
/// - **lit** is MTRL bit 0x01 on both (BF2 prim 0x4000, 724813; BF1 568929).
/// - **vc-lit** (COL0 added) is BF2 0x200 with 0x01 and a COL0 stream
///   (724822–724826 — so a BF2 unlit segment never adds), BF1 0x40 (prim
///   0x2000, 568929–568935, without the 0x01 test); the VU1 takes the add
///   only when the stream is there (§2.5).
/// - **prelit set**: BF2 — the whole MODEL, when any segment carries 0x200
///   (`FUN_0057de00`, model +0x48 & 1, §3 `[O]`); BF1 — each 0x40 segment,
///   drawn with the second published set (§6). The BF1 half is **[D],
///   PROVISIONAL**: that set's ambient word is black in the one RAM state
///   read, and no BF1 patched-state pair has checked it.
pub fn light_path(m: &Model, seg: &mungefs::model::Segment) -> LightPath {
    let f = seg.material_flags;
    let vertex_bit = if seg.bf2_material { 0x200 } else { 0x40 };
    if seg.pc_geometry && f & vertex_bit != 0 && seg.colors.is_some() {
        return LightPath::VertexLit;
    }
    let (vc_lit, prelit) = if seg.bf2_material {
        let model_prelit = m
            .segments
            .iter()
            .any(|s| s.bf2_material && s.material_flags & 0x200 != 0);
        (f & 0x201 == 0x201, model_prelit)
    } else {
        (f & 0x40 != 0, f & 0x40 != 0)
    };
    let add = vc_lit && seg.colors.is_some();
    match (f & 0x01 != 0, prelit, add) {
        (false, _, false) => LightPath::Unlit,
        (false, _, true) => LightPath::UnlitAdd,
        (true, false, _) => LightPath::Lit,
        (true, true, false) => LightPath::Prelit,
        (true, true, true) => LightPath::PrelitAdd,
    }
}

/// A BF2 segment's self-lit path on a map whose `Effect("HDR")` is enabled
/// (`analysis/re-glow.md` §1, [O]); `None` for every other segment, which
/// keeps [`light_path`]'s. B2 FUN_0058ed70 draws the whole scene at the HDR
/// scale 0.5 and the composite doubles it back, except that:
/// - **0x10 without 0x04** draws with GS ALPHA 8 (`Cs + Cs·As/128`) and the
///   colour 0xff404040 in place of the computed one (723539–723548): the
///   unlit colour × (1 + As/128), up to ×2.98, with the TEXTURE's alpha as
///   the glow mask (As ≈ disc alpha, §1.2) — [`LightPath::Glow`];
/// - **0x14** skips the 0.5 (723501–723505), so it lands ×2 —
///   [`LightPath::Unscaled`].
///
/// Every 0x10 segment on the discs is unlit (none carries 0x01, §1.3), so
/// both paths are unlit ones; a lit one is left to [`light_path`] rather
/// than guessed at. BF1 has no such override (§5).
pub fn glow_path(seg: &mungefs::model::Segment, hdr: bool) -> Option<LightPath> {
    let f = seg.material_flags;
    if !hdr || !seg.bf2_material || f & 0x01 != 0 {
        return None;
    }
    if f & 0x14 == 0x14 {
        Some(LightPath::Unscaled)
    } else if f & 0x10 != 0 && f & 0x04 == 0 {
        Some(LightPath::Glow)
    } else {
        None
    }
}

/// A glow segment's VU1 vertex alpha as `COLOR_0`'s alpha, Av / 255: the
/// unlit program writes Av = 0xff × COL0.a / 128, clamped at 0xff
/// (`re-glow.md` §1.2, VU1 0x2da–0x2ef) — 253 for the discs' 127, 0 at the
/// faded edges of MUS1's lava. The shader's `As = At·Av >> 7` reads it.
pub fn glow_vertex_alpha(a: u8) -> f32 {
    (f32::from(a) / 128.0).min(1.0)
}

/// One COL0 vertex colour as the linear RGBA the runtime multiplies (or adds)
/// in: GS units, 0x80 = 1.0 (§2 — no disc component is above 0x80, §4, and
/// the clamp keeps it that way), linearised like every other gamma-space
/// multiplier (`crate::lighting` module doc). Alpha is written OPAQUE: the
/// disc's COL0 alpha does not reach the port's alpha (the BF2 loader clears it
/// on 0x200 segments, 724632–724651, and what it does elsewhere is unread),
/// so a draw's coverage stays its texture's.
pub fn col0_linear(c: [u8; 4]) -> [f32; 4] {
    let lin = |b: u8| {
        let u = (f32::from(b) / 128.0).min(1.0);
        crate::lighting::srgb_to_linear(u)
    };
    [lin(c[0]), lin(c[1]), lin(c[2]), 1.0]
}

/// Serialize a bare triangle soup (collision geometry) to GLB bytes: one mesh,
/// one primitive, POSITION + indices, and NOTHING else — no normals, uvs,
/// materials or colours. `game::assets::load_glb` synthesises defaults for all
/// three of those, so the runtime loads this like any other mesh.
pub fn triangles_to_glb(
    name: &str,
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
) -> Result<Vec<u8>> {
    let mut bin = Bin::default();
    let (min, max) = if positions.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        min_max3(positions)
    };
    let pos_acc = bin.f32s(&flat3(positions), "VEC3", json!({"min": min, "max": max}));
    let idx: Vec<u32> = triangles.iter().flatten().copied().collect();
    let idx_acc = bin.u32s(&idx, json!({}));
    crate::gltf_bin::rigid_glb(
        bin,
        json!({"name": name, "primitives": [
            {"attributes": {"POSITION": pos_acc}, "indices": idx_acc, "mode": 4}
        ]}),
        name,
        None,
    )
}

/// Pack the JSON and BIN chunks, padding with spaces and zeroes respectively.
pub fn pack_glb(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let jlen = json.len().next_multiple_of(4);
    let blen = bin.len().next_multiple_of(4);
    let total = 12 + 8 + jlen + 8 + blen;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(jlen as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(json);
    out.resize(20 + jlen, b' ');
    out.extend_from_slice(&(blen as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(bin);
    out.resize(total, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_headers_and_padding_cover_every_alignment() {
        for j in 0..=7 {
            for b in 0..=7 {
                let json = vec![b'j'; j];
                let bin = vec![0xab; b];
                let out = pack_glb(&json, &bin);
                let word = |at| u32::from_le_bytes(out[at..at + 4].try_into().unwrap()) as usize;
                assert_eq!(&out[..4], b"glTF");
                assert_eq!(word(4), 2);
                assert_eq!(word(8), out.len());
                let jlen = word(12);
                assert_eq!(jlen % 4, 0);
                assert!((j..j + 4).contains(&jlen));
                assert_eq!(&out[16..20], b"JSON");
                assert_eq!(&out[20..20 + j], json);
                assert!(out[20 + j..20 + jlen].iter().all(|&v| v == b' '));
                let bstart = 28 + jlen;
                let blen = word(20 + jlen);
                assert_eq!(blen % 4, 0);
                assert!((b..b + 4).contains(&blen));
                assert_eq!(&out[24 + jlen..bstart], b"BIN\0");
                assert_eq!(&out[bstart..bstart + b], bin);
                assert!(out[bstart + b..].iter().all(|&v| v == 0));
                assert_eq!(out.len(), bstart + blen);
            }
        }
    }

    use crate::alpha::{AlphaMode, AlphaStats, Rule, SegmentAlpha};
    use mungefs::model::{Aabb, RenderType, Segment};

    fn seg(tex: &str, flags: u32) -> Segment {
        Segment {
            vertex_count: 3,
            index_count: 3,
            bf2_material: false,
            pc_geometry: false,
            material_flags: flags,
            render_type: RenderType::Bf1("Normal".into()),
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0, 0],
            textures: vec![(0, tex.to_owned())],
            bone: None,
            group: None,
            bmap: Vec::new(),
            bind: Vec::new(),
            wght: Vec::new(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            colors: None,
            triangles: vec![[0, 1, 2]],
        }
    }

    /// The ALPHA RULE v2 verdict reaches the glTF reader as `alphaMode` +
    /// `alphaCutoff` + `extras` — the runtime routes on the first and a
    /// census re-derives the verdict from the rest. Two primitives sharing a
    /// texture but disagreeing on the verdict get two materials of the same
    /// NAME, which is the whole point of deciding per draw.
    #[test]
    fn model_materials_carry_the_alpha_verdict() {
        let m = Model {
            name: "kam_prop_crate".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            vis_box: Aabb {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            face_count: 2,
            sphere: None,
            segments: vec![seg("kam_crate", 0x05), seg("kam_crate", 0x07)],
        };
        let stats = AlphaStats {
            mid: 0.55,
            a0: 0.12,
            amax: 255,
        };
        let alphas = vec![
            SegmentAlpha {
                texture: "kam_crate".into(),
                mode: AlphaMode::Opaque,
                rule: Rule::Mask,
                flags: 0x05,
                rtyp: RenderType::Bf1("Normal".into()),
                stats: Some(stats),
            },
            SegmentAlpha {
                texture: "kam_crate".into(),
                mode: AlphaMode::Mask,
                rule: Rule::Mask,
                flags: 0x07,
                rtyp: RenderType::Bf1("Normal".into()),
                stats: Some(stats),
            },
        ];
        let bytes = model_to_glb(&m, &alphas, false).unwrap();
        let (doc, _, _) = gltf::import_slice(&bytes).expect("gltf reads it");
        let mats: Vec<_> = doc.materials().collect();
        assert_eq!(mats.len(), 2, "one material per primitive");
        assert!(mats.iter().all(|x| x.name() == Some("kam_crate")));
        assert_eq!(mats[0].alpha_mode(), gltf::material::AlphaMode::Opaque);
        assert_eq!(mats[1].alpha_mode(), gltf::material::AlphaMode::Mask);
        assert_eq!(mats[1].alpha_cutoff(), Some(0.5));

        // The stats the verdict was taken on ride along in `extras`.
        // The JSON chunk is followed by the BIN chunk, so read it lossily.
        let json = String::from_utf8_lossy(&bytes[20..]).into_owned();
        let at = json.find("\"materials\"").expect("materials in the JSON");
        let tail = &json[at..];
        assert!(tail.contains("\"flags\":5"), "{tail}");
        assert!(tail.contains("\"rtyp\":\"Normal\""));
        assert!(tail.contains("\"rule\":\"3\""));
        assert!(tail.contains("\"amax\":255"));
        assert!(tail.contains("\"alphaCutoff\":0.5"));
    }

    fn model(segments: Vec<Segment>) -> Model {
        Model {
            name: "m".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            vis_box: Aabb {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            face_count: 1,
            sphere: None,
            segments,
        }
    }

    fn bf2(flags: u32, col0: bool) -> Segment {
        Segment {
            bf2_material: true,
            pc_geometry: false,
            colors: col0.then(|| vec![[0x40, 0x80, 0xff, 0x80]; 3]),
            ..seg("t", flags)
        }
    }

    /// Issue #71: the camera's pull point reads the vis box's top, which no vertex need
    /// reach, off the mesh's extras.
    #[test]
    fn the_vis_box_top_is_written_on_the_mesh() {
        let mut m = model(vec![seg("t", 0)]);
        m.vis_box.max[1] = 7.5;
        let alpha = SegmentAlpha {
            texture: "t".into(),
            mode: AlphaMode::Opaque,
            rule: Rule::Mask,
            flags: 0,
            rtyp: RenderType::Bf1("Normal".into()),
            stats: None,
        };
        let glb = model_to_glb(&m, &[alpha], false).unwrap();
        let doc = gltf::Gltf::from_slice(&glb).unwrap();
        let mesh = doc.meshes().next().unwrap();
        let extras: serde_json::Value =
            serde_json::from_str(mesh.extras().as_ref().unwrap().get()).unwrap();
        assert_eq!(extras["vis_top"], 7.5);
    }

    #[test]
    fn pc_vertex_lighting_does_not_take_the_ps2_additive_path() {
        for (bf2_material, flags) in [(false, 0x40), (false, 0x41), (true, 0x200), (true, 0x201)] {
            let mut segment = seg("t", flags);
            segment.bf2_material = bf2_material;
            segment.colors = Some(vec![[64, 64, 64, 128]; 3]);
            let ps2 = model(vec![segment.clone()]);
            assert_ne!(light_path(&ps2, &ps2.segments[0]), LightPath::VertexLit);
            segment.pc_geometry = true;
            let pc = model(vec![segment]);
            assert_eq!(light_path(&pc, &pc.segments[0]), LightPath::VertexLit);
        }
    }

    /// BF2's self-lit override (`analysis/re-glow.md` §1): 0x10 without 0x04
    /// glows and 0x14 lands unscaled, only on an HDR map, only BF2, only
    /// unlit; the glow segment's COL0 alpha becomes the VU1's Av.
    #[test]
    fn glow_path_follows_the_hdr_override() {
        use LightPath::*;
        assert_eq!(glow_path(&bf2(0x10, true), true), Some(Glow));
        assert_eq!(glow_path(&bf2(0x1010210, true), true), Some(Glow));
        assert_eq!(glow_path(&bf2(0x12, true), true), Some(Glow));
        assert_eq!(glow_path(&bf2(0x14, true), true), Some(Unscaled));
        assert_eq!(glow_path(&bf2(0x54, true), true), Some(Unscaled));
        assert_eq!(glow_path(&bf2(0x10, true), false), None, "HDR off");
        assert_eq!(glow_path(&bf2(0x11, true), true), None, "lit");
        assert_eq!(glow_path(&bf2(0x04, true), true), None);
        assert_eq!(glow_path(&seg("t", 0x10), true), None, "BF1");
        assert_eq!(glow_vertex_alpha(127), 127.0 / 128.0);
        assert_eq!(glow_vertex_alpha(0xff), 1.0);

        let m = model(vec![bf2(0x10, true), bf2(0x00, true)]);
        let alphas = crate::alpha::AlphaTable::new(&[]).model(&m);
        let bytes = world_model_to_glb(&m, &alphas, true).unwrap();
        let (doc, buffers, _) = gltf::import_slice(&bytes).expect("gltf reads it");
        let prims: Vec<_> = doc.meshes().next().unwrap().primitives().collect();
        let json = String::from_utf8_lossy(&bytes[20..]).into_owned();
        assert!(json.contains("\"light\":\"glow\""), "{json}");
        assert!(json.contains("\"light\":\"unlit\""), "{json}");
        let alpha = |p: &gltf::Primitive<'_>| {
            let r = p.reader(|b| Some(&buffers[b.index()]));
            r.read_colors(0).unwrap().into_rgba_f32().next().unwrap()[3]
        };
        assert_eq!(alpha(&prims[0]), 1.0, "0x80 → Av 0xff");
        assert_eq!(alpha(&prims[1]), 1.0, "not glow: written opaque");
        let plain = model_to_glb(&m, &alphas, true).unwrap();
        assert!(!String::from_utf8_lossy(&plain[20..]).contains("\"glow\""));
    }

    /// The VU1 path per segment (`analysis/re-model-lighting.md` §2/§3/§6):
    /// BF2 decides "prelit" per MODEL (any 0x200 segment) and "add" per
    /// segment (0x201 with COL0); BF1 decides both per segment on 0x40; bit
    /// 0x01 clear is unlit on both.
    #[test]
    fn light_path_follows_the_vu1_dispatch() {
        use LightPath::*;
        let plain = model(vec![bf2(0x01, true), bf2(0x00, true)]);
        assert_eq!(light_path(&plain, &plain.segments[0]), Lit);
        assert_eq!(light_path(&plain, &plain.segments[1]), Unlit);
        let pre = model(vec![
            bf2(0x01, true),
            bf2(0x201, true),
            bf2(0x201, false),
            bf2(0x200, true),
        ]);
        assert_eq!(light_path(&pre, &pre.segments[0]), Prelit);
        assert_eq!(light_path(&pre, &pre.segments[1]), PrelitAdd);
        assert_eq!(
            light_path(&pre, &pre.segments[2]),
            Prelit,
            "no COL0 stream, no add"
        );
        assert_eq!(
            light_path(&pre, &pre.segments[3]),
            Unlit,
            "BF2 sets 0x2000 only with 0x01"
        );
        let b1 = |flags, col0: bool| Segment {
            colors: col0.then(|| vec![[0x80; 4]; 3]),
            ..seg("t", flags)
        };
        let m1 = model(vec![
            b1(0x01, true),
            b1(0x41, true),
            b1(0x41, false),
            b1(0x40, true),
        ]);
        assert_eq!(
            light_path(&m1, &m1.segments[0]),
            Lit,
            "a BF1 model is never prelit whole"
        );
        assert_eq!(light_path(&m1, &m1.segments[1]), PrelitAdd);
        assert_eq!(light_path(&m1, &m1.segments[2]), Prelit);
        assert_eq!(light_path(&m1, &m1.segments[3]), UnlitAdd);
    }

    /// COL0 is GS units (0x80 = 1.0), clamped there, linearised; its alpha
    /// never reaches the port.
    #[test]
    fn col0_is_gs_units_linearised_with_an_opaque_alpha() {
        let c = col0_linear([0x80, 0xff, 0x40, 0x00]);
        assert_eq!(c[0], 1.0);
        assert_eq!(c[1], 1.0, "above 0x80 clamps to 1.0");
        assert!(
            (c[2] - 0.2140).abs() < 1e-3,
            "0x40 = 0.5 gamma = 0.214 linear"
        );
        assert_eq!(c[3], 1.0);
    }

    /// A map bake writes COLOR_0 on the segments that carry COL0 and the path
    /// on every material; a non-map bake writes neither.
    #[test]
    fn vertex_light_writes_color_0_and_the_path() {
        let m = model(vec![bf2(0x201, true), bf2(0x01, false)]);
        let alphas: Vec<SegmentAlpha> = m
            .segments
            .iter()
            .map(|s| SegmentAlpha {
                texture: "t".into(),
                mode: AlphaMode::Opaque,
                rule: Rule::Mask,
                flags: s.material_flags,
                rtyp: RenderType::Bf1("Normal".into()),
                stats: None,
            })
            .collect();
        let bytes = model_to_glb(&m, &alphas, true).unwrap();
        let (doc, buffers, _) = gltf::import_slice(&bytes).expect("gltf reads it");
        let prims: Vec<_> = doc.meshes().next().unwrap().primitives().collect();
        let read = |i: usize| {
            prims[i]
                .reader(|b| buffers.get(b.index()).map(|d| &d.0[..]))
                .read_colors(0)
                .map(|c| c.into_rgba_f32().collect::<Vec<_>>())
        };
        assert_eq!(read(0).unwrap()[0], col0_linear([0x40, 0x80, 0xff, 0x80]));
        assert!(read(1).is_none(), "no COL0, no COLOR_0");
        let json = String::from_utf8_lossy(&bytes[20..]).into_owned();
        assert!(json.contains("\"light\":\"prelit_add\""), "{json}");
        assert!(json.contains("\"light\":\"prelit\""));
        let plain = model_to_glb(&m, &alphas, false).unwrap();
        let json = String::from_utf8_lossy(&plain[20..]).into_owned();
        assert!(!json.contains("COLOR_0") && !json.contains("\"light\""));
    }

    #[test]
    fn triangles_to_glb_round_trips_through_the_gltf_reader() {
        let positions = [
            [0.0f32, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 0.0, 10.0],
            [0.0, 5.0, 10.0],
        ];
        let triangles = [[0u32, 1, 2], [0, 2, 3]];
        let bytes = triangles_to_glb("floor", &positions, &triangles).unwrap();
        let (doc, buffers, _) = gltf::import_slice(&bytes).expect("gltf reads it");
        let mesh = doc.meshes().next().expect("one mesh");
        assert_eq!(mesh.name(), Some("floor"));
        let prims: Vec<_> = mesh.primitives().collect();
        assert_eq!(prims.len(), 1);
        let reader = prims[0].reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
        let got: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
        assert_eq!(got, positions);
        let idx: Vec<u32> = reader.read_indices().unwrap().into_u32().collect();
        assert_eq!(idx, vec![0, 1, 2, 0, 2, 3]);
        // Deliberately bare: no normals, uvs, colours or materials — the
        // runtime's load_glb synthesises defaults for them.
        assert!(reader.read_normals().is_none());
        assert!(reader.read_tex_coords(0).is_none());
        assert_eq!(doc.materials().count(), 0);
    }
}
