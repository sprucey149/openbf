//! PC Battlefront II environment-mapped materials: a PC-authored segment whose
//! BF2 `MTRL` word carries 0x100 and names a cube map in `TNAM` slot 3 (the
//! shine on, for one, a mod's Sith trooper armour).
//!
//! The rule is the PC game's own, read from `core.lvl -> SHDR` (D3D9 bytecode,
//! disassembled with `fxc /dumpbin`) and `BattlefrontII.exe` (GOG build, read
//! with capstone; there is no decomp). `analysis/re-pc-envmap.md` is the full
//! trace.
//! - **The pass**: the specular pass (`0x702480`) draws the cube as one more
//!   light of colour 1 whose lookup vector is the world reflection itself
//!   (`0x702c06`; `specularlighting_shader.xml` ps003 and its vertex shader):
//!   `cube(R) * spec colour`, added One/One where the diffuse alpha passes the
//!   alpha test.
//! - **The slot**: the pass binds `[material + 0xc]`, `TNAM` slot 3
//!   (`0x6f3ed9`).
//! - **The cube** is baked beside the segment's diffuse as
//!   `<diffuse>.env.ktx2` (a name no `tex_` can spell): the six faces in D3D
//!   order stacked top to bottom, every mip of the surface.
//!
//! Not modelled, none of them seen on an environment material measured so far:
//! the map-wide cube the PC binds when slot 3 is empty, the normal-mapped
//! variant (ps005) and the glossmap weight (0x08).
//! trace: `[scene + 0x98]` (`0x702c84`) for the map-wide cube.

use anyhow::{anyhow, Result};
use mungefs::model::{Model, Segment};
use mungefs::Chunk;
use std::collections::HashMap;
use std::path::Path;

/// BF2 `MTRL` bit 0x100: the segment is environment-mapped.
pub const ENVMAP: u32 = 0x100;

/// `MTRL` bit 0x04, a blended base. OURS: no pass over one; the port's
/// specular pass needs the depth its base wrote.
/// replace: a pass for blended bases once one is drawn on a unit or a prop
/// that shows it.
const TRANSPARENT: u32 = 0x04;

/// File name of the baked environment-cube companion of `base`.
pub fn env_file(base: &str) -> String {
    format!("{base}.env.ktx2")
}

/// `(diffuse, cube)` for a PC environment-mapped segment, both lowercase;
/// `None` for every other segment, PS2 and Xbox geometry included.
pub fn companion(seg: &Segment) -> Option<(String, String)> {
    let f = seg.material_flags;
    if !seg.pc_geometry
        || !seg.bf2_material
        || f & ENVMAP == 0
        || f & (TRANSPARENT | crate::bump::BUMP) != 0
    {
        return None;
    }
    let slot = |n: u32| {
        seg.textures
            .iter()
            .find(|(s, name)| *s == n && !name.is_empty())
            .map(|(_, name)| name.to_ascii_lowercase())
    };
    Some((slot(0)?, slot(3)?))
}

/// Every environment companion `models` draw, keyed by diffuse.
///
/// OURS: one cube per diffuse sheet, the first met, because the port binds a
/// material per texture (as `crate::bump::companions`).
pub fn companions<'a>(models: impl IntoIterator<Item = &'a Model>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (diffuse, cube) in models
        .into_iter()
        .flat_map(|m| &m.segments)
        .filter_map(companion)
    {
        out.entry(diffuse).or_insert(cube);
    }
    out
}

/// A cube map as the companion stores it: `mips[m]` is the six faces of mip
/// `m` (RGBA8, `size >> m` square), one after the other.
pub struct Cube {
    pub size: u32,
    pub mips: Vec<Vec<u8>>,
}

/// The cube `tex_` named `name` among `chunks`: the surface
/// `mungefs::texture_pc::Texture::best` picks, and its mips while all six
/// faces keep halving.
///
/// OURS: the best-fidelity surface, as every other PC texture import here.
/// trace: which `FMT_` the PC exe's loader picks.
pub fn find_cube(chunks: &[Chunk<'_>], name: &str) -> Option<Cube> {
    chunks
        .iter()
        .filter(|c| &c.tag == b"tex_" && c.child(b"FMT_").is_some())
        .filter_map(|c| mungefs::texture_pc::Texture::parse(c).ok())
        .filter(|t| t.name.eq_ignore_ascii_case(name))
        .find_map(|t| {
            let s = t.best()?;
            if !s.is_cube() || s.faces.len() != 6 {
                return None;
            }
            let size = s.faces[0].mips.first()?.width;
            let mut mips = Vec::new();
            for m in 0.. {
                let side = size >> m;
                let level: Vec<u8> = s
                    .faces
                    .iter()
                    .filter_map(|f| f.mips.get(m))
                    .filter(|x| side > 0 && x.width == side && x.height == side)
                    .flat_map(|x| x.rgba.iter().copied())
                    .collect();
                if level.len() != (side * side * 4 * 6) as usize || side == 0 {
                    break;
                }
                mips.push(level);
            }
            (!mips.is_empty()).then_some(Cube { size, mips })
        })
}

/// Write `<dir>/<base>.env.ktx2` when `companions` pairs `base` with a cube
/// `chunks` hold, removing an obsolete companion otherwise.
pub fn write_env_for(
    dir: &Path,
    base: &str,
    companions: &HashMap<String, String>,
    chunks: &[Chunk<'_>],
) -> Result<()> {
    let base = base.to_ascii_lowercase();
    let cube = companions.get(&base).and_then(|n| find_cube(chunks, n));
    write_cube(dir, &base, cube.as_ref())
}

/// [`write_env_for`] with the cube already found.
pub fn write_cube(dir: &Path, base: &str, cube: Option<&Cube>) -> Result<()> {
    let file = dir.join(env_file(base));
    let Some(cube) = cube else {
        return Ok(crate::remove_optional_file(&file)?);
    };
    let bytes = crate::ktx2w::write_rgba8(cube.size, cube.size * 6, &cube.mips);
    std::fs::write(&file, bytes).map_err(|e| anyhow!("{}: {e}", file.display()))
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
            spec_power: 10,
            spec_color: 0xff33_3333,
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
    fn only_an_opaque_pc_segment_with_a_slot_3_cube_pairs() {
        let sith = [(0, "Sith"), (1, ""), (2, ""), (3, "Sithtr_envmap")];
        assert_eq!(
            companion(&seg(true, 0x101, &sith)),
            Some(("sith".into(), "sithtr_envmap".into()))
        );
        assert_eq!(companion(&seg(false, 0x101, &sith)), None);
        assert_eq!(companion(&seg(true, 0x001, &sith)), None);
        assert_eq!(companion(&seg(true, 0x105, &sith)), None);
        assert_eq!(companion(&seg(true, 0x121, &sith)), None);
        assert_eq!(companion(&seg(true, 0x101, &[(0, "a"), (3, "")])), None);
    }
}
