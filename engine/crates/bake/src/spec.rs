//! The specular second pass: which segments draw it, with what power and
//! colour, and the disc's shared `specular` lookup table (queue SPEC).
//!
//! Every claim here is `analysis/re-specular-pass.md`'s, tagged `[O]` (one
//! Opus pass plus one Opus verifier, no Fable audit), so the rule is
//! PROVISIONAL:
//! - **Which segments** (§1): BF2 a word with 0x80 (`FUN_0058ed70`
//!   723879–723880) — but NOT one that also carries 0x04, whose record blends
//!   by a source alpha of 1/128 and so adds at most 2/255 (§6: invisible, so
//!   skipped); BF1 its Specular class, word bit 0x20, which the class factory
//!   `FUN_004aa9f8` tests before any other class bit.
//! - **The colour** (§2): flat and unlit. BF2 `((b + 1) >> 1) / 128` per
//!   channel of the MTRL's specular RGBA (0xff → 1.0). BF1 the MTRL's B raw
//!   (`/128`, so up to ~2×) unless B is 0 or `B & 0xffffff == 0xffffff`, when
//!   it is the map's `SunInfo` colour raw — resolved at DRAW time
//!   (`rgb: None`), because the lighting is read later in the map's LVL than
//!   its models and the side bake (queue SPEC2) knows no map at all.
//! - **The power** is the MTRL's own, written as is; the pass samples the
//!   table at `s = power / 255` (§3).
//!
//! The GLB carries it as the material's `extras.spec_pass` (ours): `{"power":
//! p, "rgb": [r, g, b]}` or `{"power": p, "rgb": null}` for "the sun colour".
//! No BF2 word on the disc carries 0x100 beside 0x80 (0/469, §1), so the env
//! pass that would displace this one is not modelled.

use anyhow::{anyhow, Result};
use mungefs::model::Segment;
use serde_json::{json, Value};
use std::path::Path;

/// One segment's specular pass, as the bake writes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecPass {
    /// The MTRL's specular power; the table's `s = power / 255`.
    pub power: u32,
    /// The pass colour in GS units / 128 (1.0 = the texel as is), or `None`
    /// for "the map's sun colour / 128" (BF1 only).
    pub rgb: Option<[f32; 3]>,
}

impl SpecPass {
    /// The material's `extras.spec_pass` value.
    pub fn to_json(self) -> Value {
        json!({ "power": self.power, "rgb": self.rgb })
    }
}

/// The pass `seg` draws, or `None` when it draws none. See the module doc.
pub fn spec_pass(seg: &Segment) -> Option<SpecPass> {
    let f = seg.material_flags;
    let bytes = seg.spec_color.to_le_bytes();
    if seg.bf2_material {
        if f & 0x80 == 0 || f & 0x04 != 0 {
            return None;
        }
        let half = |b: u8| ((u32::from(b) + 1) >> 1) as f32 / 128.0;
        Some(SpecPass {
            power: seg.spec_power,
            rgb: Some([half(bytes[0]), half(bytes[1]), half(bytes[2])]),
        })
    } else {
        if f & 0x20 == 0 {
            return None;
        }
        let b = seg.spec_color;
        let own = b != 0 && b & 0xff_ffff != 0xff_ffff;
        Some(SpecPass {
            power: seg.spec_power,
            rgb: own.then(|| [0, 1, 2].map(|i| f32::from(bytes[i]) / 128.0)),
        })
    }
}

/// The textures (lowercase) that a specular-pass segment of `models` draws —
/// each such segment's base sheet (`crate::alpha::segment_texture`).
///
/// These keep their disc alpha in every bake (queue DSHN,
/// `analysis/re-spec-gloss.md` §1/§7, `[O]`, PROVISIONAL): the PS2's pass
/// record blends `Cs·Ad + Cd`, and Ad is the base draw's texel alpha — on the
/// droid, dark-trooper, weapon and deck sheets a dark gloss mask (the BF1 SBD
/// averages 34/255). An opaque bake makes the pass add at weight 1.0: chrome.
/// The material MODES do not change — an OPAQUE draw never reads the alpha,
/// so only the pass (`fs_spec`'s `base.a`) and the target's alpha plane see it.
pub fn pass_textures<'a>(
    models: impl IntoIterator<Item = &'a mungefs::model::Model>,
) -> std::collections::HashSet<String> {
    models
        .into_iter()
        .flat_map(|m| &m.segments)
        .filter(|s| spec_pass(s).is_some())
        .filter_map(crate::alpha::segment_texture)
        .map(str::to_ascii_lowercase)
        .collect()
}

/// The `specular` table's name in CORE.LVL (both discs; §5).
pub const TABLE_NAME: &str = "specular";

/// Bake the `specular` lookup table out of a CORE.LVL into
/// `<outdir>/specular.ktx2`: 128×128 grey, ONE level (a lookup table, never
/// mipmapped — the GS samples it bilinear with no mips, §3/§5). The texels
/// are byte-identical on the two discs (§5); each game's common bake takes
/// its own disc's.
pub fn bake_table(core_lvl: &str, outdir: &str) -> Result<()> {
    bake_core_texture(core_lvl, TABLE_NAME, outdir)
}

/// BF2's terrain light-pass texture (card FXBL, `analysis/re-bolt.md` §5.3
/// [O]): 64×64 grey, a cos² blob, projected down onto the terrain under
/// every omni light. One level, like the specular table.
pub const OMNIPROJ_NAME: &str = "com_sfx_omniproj";

fn synth_core_texture(name: &str) -> Option<(u32, u32, Vec<u8>)> {
    if name.eq_ignore_ascii_case(TABLE_NAME) {
        let mut rgba = Vec::with_capacity(128 * 128 * 4);
        for y in 0..128u32 {
            let d = (2.0 * (y as f32 / 127.0) - 1.0).abs();
            for x in 0..128u32 {
                let v = if x == 0 {
                    ((0.5 + 0.5 * d) * 255.0).round() as u8
                } else {
                    (d.powf(2.0 * x as f32) * 255.0).round().clamp(0.0, 255.0) as u8
                };
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        Some((128, 128, rgba))
    } else if name.eq_ignore_ascii_case(OMNIPROJ_NAME) {
        let mut rgba = Vec::with_capacity(64 * 64 * 4);
        for y in 0..64u32 {
            let v = (y as f32 + 0.5) / 32.0 - 1.0;
            for x in 0..64u32 {
                let u = (x as f32 + 0.5) / 32.0 - 1.0;
                let r = u.hypot(v);
                let g = if r >= 1.0 {
                    0
                } else {
                    ((r * std::f32::consts::FRAC_PI_2).cos().powi(2) * 255.0).round() as u8
                };
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
        }
        Some((64, 64, rgba))
    } else {
        None
    }
}

/// Bake the CORE.LVL texture `name` into `<outdir>/<name>.ktx2`, its level 0
/// only.
pub fn bake_core_texture(core_lvl: &str, name: &str, outdir: &str) -> Result<()> {
    let bytes = std::fs::read(core_lvl).map_err(|e| anyhow!("{core_lvl}: {e}"))?;
    let inner = mungefs::open(&bytes).map_err(|e| anyhow!("{core_lvl}: {e}"))?;
    let found = mungefs::content_chunks(inner)
        .map_err(|e| anyhow!("{core_lvl}: {e}"))?
        .into_iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| mungefs::texture::Texture::parse(&c).ok())
        .find(|t| t.name.eq_ignore_ascii_case(name));
    let (w, h, level0) = if let Some(tex) = found {
        let (w, h) = tex.mip_dims(0);
        (w, h, tex.rgba8(0))
    } else if let Some(synth) = synth_core_texture(name) {
        synth
    } else {
        return Err(anyhow!("{core_lvl}: no tex_ named {name:?}"));
    };
    std::fs::create_dir_all(outdir)?;
    let ktx = crate::ktx2w::write_rgba8(w, h, std::slice::from_ref(&level0));
    std::fs::write(Path::new(outdir).join(format!("{name}.ktx2")), ktx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::RenderType;

    fn seg(bf2: bool, flags: u32, power: u32, color: u32) -> Segment {
        Segment {
            vertex_count: 0,
            index_count: 0,
            bf2_material: bf2,
            material_flags: flags,
            render_type: RenderType::Absent,
            spec_power: power,
            spec_color: color,
            uv_anim: [0, 0],
            textures: Vec::new(),
            bone: None,
            group: None,
            bmap: Vec::new(),
            bind: Vec::new(),
            wght: Vec::new(),
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: None,
            triangles: Vec::new(),
        }
    }

    /// BF2: 0x80 draws the pass, halved into GS units (0xff → 1.0, 0x80 →
    /// (129 >> 1) / 128 = 0.5, 0 → 0), and 0x04 beside it draws none (`re-specular-pass.md` §6).
    #[test]
    fn bf2_takes_0x80_without_0x04_and_halves_the_colour() {
        let p = spec_pass(&seg(true, 0x1_0089, 20, 0xff00_80ff)).unwrap();
        assert_eq!(p.power, 20);
        assert_eq!(p.rgb, Some([1.0, 0.5, 0.0]));
        assert_eq!(spec_pass(&seg(true, 0x8d, 20, 0xffff_ffff)), None);
        assert_eq!(spec_pass(&seg(true, 0x09, 20, 0xffff_ffff)), None);
        // BF1's class bit means nothing on BF2.
        assert_eq!(spec_pass(&seg(true, 0x20, 20, 0xffff_ffff)), None);
    }

    /// BF1: bit 0x20 draws the pass; B raw over 128, or the sun (`None`) when
    /// B is 0 or white — the census's 0xffffff and 0 and 0xffc9d0.
    #[test]
    fn bf1_takes_its_class_bit_and_b_raw_or_the_sun() {
        assert_eq!(
            spec_pass(&seg(false, 0x31, 4, 0xff_ffff)),
            Some(SpecPass {
                power: 4,
                rgb: None
            })
        );
        assert_eq!(spec_pass(&seg(false, 0x30, 50, 0)).unwrap().rgb, None);
        assert_eq!(
            spec_pass(&seg(false, 0x31, 15, 0xffc9d0)).unwrap().rgb,
            Some([208.0 / 128.0, 201.0 / 128.0, 255.0 / 128.0])
        );
        assert_eq!(spec_pass(&seg(false, 0x11, 15, 0xffc9d0)), None);
        // BF2's bit means nothing on BF1.
        assert_eq!(spec_pass(&seg(false, 0x80, 15, 0xffc9d0)), None);
    }
}
