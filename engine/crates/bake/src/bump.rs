//! PC Battlefront II normal-mapped materials: a PC-authored segment whose BF2
//! `MTRL` word carries 0x20, and the PC core's `specularcubemap` that its
//! specular pass samples.
//!
//! The rule is the PC game's own, read from `core.lvl -> SHDR` (D3D9 bytecode,
//! disassembled with `fxc /dumpbin`) and `BattlefrontII.exe` (GOG build, read
//! with capstone; there is no decomp). `analysis/re-pc-bump.md` is the full
//! trace.
//! - **Which segments**: PC geometry (`Segment::pc_geometry`) with a BF2
//!   material, the 0x20 bit, and a normal map in `TNAM` slot 1. The exe binds
//!   that slot to stage 0 of the spec pass when it is set (`0x702480`, the
//!   `[material + 8]` test), and the `bump` RTYP's lit pixel shaders sample it
//!   as `tex t1` beside the diffuse (`bump_shader.xml` ps037).
//! - **The normal map** is baked beside the segment's diffuse as
//!   `<diffuse>.bump.ktx2` (a name no `tex_` can spell), so every draw path
//!   that loads the diffuse finds it the way it finds a `.dtl.ktx2`.
//! - **Gloss**: the bump spec pixel shader weights the add by
//!   `lrp(c0.w, t0.w, 1)` with t0 the NORMAL map (`specularlighting_shader.xml`
//!   ps004), and `c0.w` is 1 exactly when the material has 0x08, glossmap
//!   (`0x702e68`). The companion stores that weight in its alpha: the normal
//!   map's own alpha under 0x08, 255 without it.
//! - **The cube**: the spec pass binds `specularcubemap` on stage 3 for every
//!   non-spot light (`0x702449`, the global set at `0x7880f0`). Only its +Y
//!   face is non-zero (all 6 × 16384 texels read), so the bake keeps that face.

use anyhow::{anyhow, Result};
use mungefs::model::{Model, Segment};
use mungefs::texture::Texture;
use std::collections::HashMap;
use std::path::Path;

/// BF2 `MTRL` bit 0x20: the segment is normal-mapped.
pub const BUMP: u32 = 0x20;

/// BF2 `MTRL` bit 0x08: glossmap, the spec pass's `c0.w` (`0x702e68`).
pub const GLOSSMAP: u32 = 0x08;

/// What a PC bump segment adds to its diffuse sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Companion {
    /// The `TNAM` slot 1 texture, lowercase.
    pub normal: String,
    /// The material carries [`GLOSSMAP`].
    pub gloss: bool,
}

/// File name of the baked normal-map companion of `base`.
pub fn bump_file(base: &str) -> String {
    format!("{base}.bump.ktx2")
}

/// `(diffuse, companion)` for a PC bump segment, both lowercase; `None` for
/// every other segment, PS2 and Xbox geometry included.
pub fn companion(seg: &Segment) -> Option<(String, Companion)> {
    if !seg.pc_geometry || !seg.bf2_material || seg.material_flags & BUMP == 0 {
        return None;
    }
    let slot = |n: u32| {
        seg.textures
            .iter()
            .find(|(s, name)| *s == n && !name.is_empty())
            .map(|(_, name)| name.to_ascii_lowercase())
    };
    Some((
        slot(0)?,
        Companion {
            normal: slot(1)?,
            gloss: seg.material_flags & GLOSSMAP != 0,
        },
    ))
}

/// Every PC bump companion `models` draw, keyed by diffuse. A diffuse two
/// materials pair with different normal maps keeps the first; the gloss bit
/// is the OR of every material that pairs it with that map.
///
/// OURS: one companion per diffuse sheet, because the port binds a material
/// per texture; the PC binds per segment.
/// replace: a per-draw material once a shipped or modded sheet is measured
/// to carry two normal maps or two gloss states.
pub fn companions<'a>(models: impl IntoIterator<Item = &'a Model>) -> HashMap<String, Companion> {
    let mut out: HashMap<String, Companion> = HashMap::new();
    for seg in models.into_iter().flat_map(|m| &m.segments) {
        let Some((diffuse, c)) = companion(seg) else {
            continue;
        };
        match out.get_mut(&diffuse) {
            Some(have) if have.normal == c.normal => have.gloss |= c.gloss,
            Some(_) => {}
            None => {
                out.insert(diffuse, c);
            }
        }
    }
    out
}

/// Every `modl` among an LVL's content chunks that parses.
pub fn lvl_models(chunks: &[mungefs::Chunk<'_>]) -> Vec<Model> {
    chunks
        .iter()
        .filter(|c| &c.tag == b"modl")
        .filter_map(|c| Model::parse(c).ok())
        .collect()
}

/// The companion's texels: the normal map's mips, alpha replaced by 255
/// unless the material is a glossmap.
pub fn companion_mips(normal: &Texture, gloss: bool) -> Vec<Vec<u8>> {
    (0..normal.mips.len())
        .map(|i| {
            let mut rgba = normal.rgba8(i);
            if !gloss {
                for px in rgba.chunks_exact_mut(4) {
                    px[3] = 255;
                }
            }
            rgba
        })
        .collect()
}

/// Write `<dir>/<base>.bump.ktx2` when `companions` pairs `base` with a
/// normal map that `all` decodes, removing an obsolete companion otherwise.
/// Returns the normal map's name when one was written.
pub fn write_bump_for(
    dir: &Path,
    base: &str,
    companions: &HashMap<String, Companion>,
    all: &[Texture],
) -> Result<Option<String>> {
    let base = base.to_ascii_lowercase();
    let file = dir.join(bump_file(&base));
    let found = companions.get(&base).and_then(|c| {
        all.iter()
            .find(|t| t.name.eq_ignore_ascii_case(&c.normal))
            .map(|t| (t, c.gloss))
    });
    let Some((normal, gloss)) = found else {
        crate::remove_optional_file(&file)?;
        return Ok(None);
    };
    let bytes = crate::ktx2w::write_rgba8(
        normal.info.width,
        normal.info.height,
        &companion_mips(normal, gloss),
    );
    std::fs::write(&file, bytes).map_err(|e| anyhow!("{}: {e}", file.display()))?;
    Ok(Some(normal.name.to_ascii_lowercase()))
}

/// The PC core's cube, as `core.lvl` names it.
pub const CUBE_NAME: &str = "specularcubemap";

/// The baked +Y face, in a common bake.
pub const CUBE_FILE: &str = "pc_specularcube.ktx2";

/// D3D's cube face order is +X, -X, +Y, -Y, +Z, -Z: +Y is the third.
const PLUS_Y: usize = 2;

/// The +Y face of a PC `core.lvl`'s `specularcubemap` (mip 0, RGBA8): the
/// surface `mungefs::texture_pc::Texture::best` decodes (DXT3 over its
/// A4R4G4B4 sibling on the shipped core).
///
/// OURS: the best-fidelity surface, as every other PC texture import here.
/// trace: which `FMT_` the PC exe's loader picks on a card with both.
pub fn cube_face(core_lvl: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let inner = mungefs::open(core_lvl).map_err(|e| anyhow!("{e}"))?;
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    for c in chunks
        .iter()
        .filter(|c| &c.tag == b"tex_" && c.child(b"FMT_").is_some())
    {
        let Ok(t) = mungefs::texture_pc::Texture::parse(c) else {
            continue;
        };
        if !t.name.eq_ignore_ascii_case(CUBE_NAME) {
            continue;
        }
        let s = t.best().ok_or_else(|| anyhow!("{CUBE_NAME}: no surface"))?;
        if !s.is_cube() {
            return Err(anyhow!("{CUBE_NAME}: not a cube map"));
        }
        let m = s
            .faces
            .get(PLUS_Y)
            .and_then(|f| f.mips.first())
            .ok_or_else(|| anyhow!("{CUBE_NAME}: no +Y face"))?;
        return Ok((m.width, m.height, m.rgba.clone()));
    }
    Err(anyhow!("no tex_ named {CUBE_NAME:?}"))
}

/// Bake [`cube_face`] of the PC `core.lvl` at `core_lvl` into
/// `<outdir>/`[`CUBE_FILE`], one level (the cube carries one, and the pass
/// samples it as a lookup).
pub fn bake_cube(core_lvl: &str, outdir: &str) -> Result<()> {
    let bytes = std::fs::read(core_lvl).map_err(|e| anyhow!("{core_lvl}: {e}"))?;
    let (w, h, rgba) = cube_face(&bytes).map_err(|e| anyhow!("{core_lvl}: {e}"))?;
    std::fs::create_dir_all(outdir)?;
    let ktx = crate::ktx2w::write_rgba8(w, h, std::slice::from_ref(&rgba));
    std::fs::write(Path::new(outdir).join(CUBE_FILE), ktx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::RenderType;

    fn seg(pc: bool, flags: u32, textures: &[(u32, &str)]) -> Segment {
        Segment {
            vertex_count: 0,
            index_count: 0,
            bf2_material: true,
            pc_geometry: pc,
            material_flags: flags,
            render_type: RenderType::Absent,
            spec_power: 1,
            spec_color: 0xffff_ffff,
            uv_anim: [0, 0],
            textures: textures
                .iter()
                .map(|(s, n)| (*s, (*n).to_owned()))
                .collect(),
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

    #[test]
    fn only_pc_bump_segments_with_a_normal_map_pair() {
        let atrt = [(0, "ATRT_diff"), (1, "ATRT_normal"), (2, ""), (3, "")];
        assert_eq!(
            companion(&seg(true, 169, &atrt)),
            Some((
                "atrt_diff".into(),
                Companion {
                    normal: "atrt_normal".into(),
                    gloss: true
                }
            ))
        );
        assert_eq!(
            companion(&seg(true, 33, &atrt)).map(|c| c.1.gloss),
            Some(false)
        );
        assert_eq!(companion(&seg(false, 169, &atrt)), None);
        assert_eq!(companion(&seg(true, 169 & !BUMP, &atrt)), None);
        assert_eq!(companion(&seg(true, 169, &[(0, "a"), (1, "")])), None);
    }

    #[test]
    fn a_non_glossmap_companion_is_opaque() {
        let t = Texture {
            name: "n".into(),
            info: mungefs::texture::TextureInfo {
                width: 1,
                height: 1,
                bpp: 32,
                detail_compressed: false,
                mip_distance: 0.0,
                mip_count: 1,
            },
            palette: Vec::new(),
            mips: vec![vec![128, 128, 255, 40]],
        };
        assert_eq!(companion_mips(&t, true), vec![vec![128, 128, 255, 40]]);
        assert_eq!(companion_mips(&t, false), vec![vec![128, 128, 255, 255]]);
    }
}
