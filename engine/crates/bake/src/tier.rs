//! Texture source tiers: where a baked texture's pixels come from.
//!
//! The maintainer's chain (PLAN §Classic-plus assets, confirmed 2026-08-31):
//!
//! > **Harrisonfog HD > PC stock (only where LARGER) > our upscale > PS2
//! > original**
//!
//! This module implements the two tiers that exist today — PS2 and PC stock —
//! and is shaped so the other two slot in as extra resolvers ahead of PC.
//! BF1 is unaffected: no PC copy of BF1 exists here, so every BF1 texture
//! stays [`Tier::Ps2`].
//!
//! **"Only where larger" is the maintainer's rule and it is not caution for its own
//! sake.** PC textures are lossy DXT1/DXT3; the PS2 ones are palettized. The
//! artifacts are different, not ordered, so at EQUAL dimensions the PS2
//! texture may well be the cleaner one. Real 2x source art beats a 2x upscale;
//! a same-size DXT1 re-encode does not beat a palette.
//!
//! Measured over all 17 BF2 maps we bake plus INGAME (independently
//! re-measured by the lead before this landed — the per-map counts reproduce
//! exactly): 1,063 baked textures, 1,040 name-matched by direct lowercase
//! name, **370 (34.8 %) strictly larger in both dimensions**, 3 smaller in
//! some dimension, the rest identical. The modal improvement is a clean 2x in
//! each dimension.
//!
//! ## The PC art must be the SAME art
//!
//! "Larger" is not the whole rule, and this is measured rather than assumed.
//! Downsampling each PC candidate back to the PS2 dimensions and comparing
//! them over all 346 candidates: the median mean-absolute difference is
//! **4.5 of 255** and the median luma cross-correlation **0.988** — three
//! quarters of the set really is the same painting at higher resolution, which
//! is exactly what the maintainer is buying.
//!
//! The tail is not. BF2 Hoth's `sky_hoth` is a hazy white BLIZZARD on the PS2
//! and a blue sky with distinct clouds on the PC (correlation −0.12);
//! `hoth_mountain`, `sky_endor`, `sky_kam1`, `fel1_sky_clouds`, the Geonosis
//! and Tatooine terrain sheets and `uta1_prop_rock_1_ps2` (a texture the disc
//! literally names `_ps2`) are all different paintings under a shared name.
//! Taking those changes the map's CONTENT, and content fidelity is
//! non-negotiable (PLAN Architecture #5) — a clear blue Hoth is not a
//! "classic-plus" Hoth.
//!
//! So a candidate is rejected when it is neither the same levels NOR the same
//! structure: mean absolute difference above [`MAX_MEAN_DIFF`] **and** luma
//! cross-correlation below [`MIN_CORRELATION`]. Both conditions are needed —
//! correlation alone rejects smooth textures where it is meaningless
//! (`dea1_bldg_blast_corridor02` correlates at 0.82 while differing by 1.4 of
//! 255), and difference alone rejects the same art merely graded differently
//! (`uta1_bldg_*` are systematically brighter on the PS2). PROVISIONAL
//! thresholds; they reject 24 of the 346 candidates.
//!
//! ## Three hard exclusions, each measured
//!
//! 1. **A detail-compressed base is never overridden.** `<name>_dtl` (RE_LOG
//!    "BF1 detail textures") is a 4x companion registered to the PS2 base's
//!    own pixels; swapping the base for HD art while keeping the companion
//!    would modulate detail the HD art already contains. Eleven of the
//!    replacement candidates are such bases (BF2 COR1 alone has three:
//!    `cor1_prop_bust`, `cor1_prop_bust02`, `cor1_prop_statue`).
//! 2. **Cube maps are never overridden.** Their faces are not comparable to a
//!    2D texture by dimension, and nothing in the bake consumes a cube.
//! 3. **A PC surface whose mip chain does not decode cleanly is skipped**
//!    rather than partially written (the munger truncates block counts, so a
//!    mip's byte length can disagree with its dimensions).
//!
//! ## Both tiers, side by side
//!
//! The maintainer's decision of 2026-09-03 (PLAN §Classic-plus assets) makes render
//! quality a RUNTIME toggle in the pause menu, flipped mid-level: "a mid-round
//! flip is a pure renderer event". So the map bake no longer picks one source
//! and writes it — it writes the PS2 texture as `<name>.ktx2` and, where the
//! chain has something better, that as `<name>.tier2.ktx2` beside it
//! ([`tier2`]). The base file is always the disc's own pixels, which is what
//! makes the toggle safe: the sim reads only the PS2 bake, and no override can
//! reach it. The soldier/weapon bake writes the same pair by the same rule
//! (`crate::soldier::bake_textures_flagged`), so no single-source resolver is
//! left in the bake.
//!
//! The alpha handling is the subtle part and it is deliberate: the material
//! flags that decide opaque-vs-cutout-vs-composite come from PS2 `segm MTRL`
//! geometry, NOT from the texture, so they stay valid for any tier — but they
//! must be applied to the RESOLVED pixels. Measured on the replacement set,
//! 30 PC textures gain alpha their PS2 counterpart does not have and 13 of
//! those have over 80 % of texels below the alpha test, so writing PC pixels
//! without the same opaque gate would delete surfaces. With both tiers
//! resident the rule is sharper still: the two files must draw the SAME
//! silhouette, so tier 2 takes the PS2 texture's alpha whenever a draw depends
//! on it ([`Resolved::take_alpha_from`]) and is forced opaque whenever the
//! base is.

use anyhow::{Context, Result};
use mungefs::texture::Texture;
use mungefs::texture_pc;
use std::path::Path;

/// Mean absolute per-channel difference (of 255), measured between the PS2
/// texture and the PC candidate downsampled to the same size, above which the
/// two are not obviously the same image. PROVISIONAL — chosen from the
/// measured distribution (median 4.5, p75 9.7).
pub const MAX_MEAN_DIFF: f64 = 12.0;

/// Luma cross-correlation below which the two do not share a structure.
/// PROVISIONAL — measured median 0.988, p25 0.938.
pub const MIN_CORRELATION: f64 = 0.5;

/// Is the PC candidate the SAME ART as the PS2 texture, or a different
/// painting under the same name? See the module doc.
///
/// `pc` is the candidate's mip 0 (RGBA8, `pw` x `ph`), `ps2` the PS2 mip 0
/// (`w` x `h`). Returns true when the two agree on levels OR on structure.
pub fn same_art(ps2: &[u8], w: u32, h: u32, pc: &[u8], pw: u32, ph: u32) -> bool {
    let down = box_down(pc, pw, ph, w, h);
    if down.len() != ps2.len() {
        return false;
    }
    let mut sum = 0u64;
    for (a, b) in ps2.as_chunks::<4>().0.iter().zip(down.as_chunks::<4>().0) {
        for c in 0..3 {
            sum += u64::from(a[c].abs_diff(b[c]));
        }
    }
    let mean_diff = sum as f64 / (ps2.len() as f64 / 4.0 * 3.0);
    if mean_diff <= MAX_MEAN_DIFF {
        return true;
    }
    correlation(ps2, &down) >= MIN_CORRELATION
}

fn luma(px: &[u8]) -> f64 {
    0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2])
}

/// Normalised cross-correlation of the two images' luma — insensitive to
/// brightness and contrast, so it answers "same picture?" rather than "same
/// exposure?". A constant image correlates with everything (returns 1.0),
/// which is right: there is no structure to disagree about.
fn correlation(a: &[u8], b: &[u8]) -> f64 {
    let (xa, xb): (Vec<f64>, Vec<f64>) = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .map(|(p, q)| (luma(p), luma(q)))
        .unzip();
    let n = xa.len() as f64;
    if n < 1.0 {
        return 1.0;
    }
    let (ma, mb) = (xa.iter().sum::<f64>() / n, xb.iter().sum::<f64>() / n);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..xa.len() {
        let (u, v) = (xa[i] - ma, xb[i] - mb);
        num += u * v;
        da += u * u;
        db += v * v;
    }
    if da <= 1e-9 || db <= 1e-9 {
        return 1.0;
    }
    num / (da.sqrt() * db.sqrt())
}

/// Box-filter downsample of an RGBA8 image.
fn box_down(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    let (fx, fy) = (sw as f32 / dw as f32, sh as f32 / dh as f32);
    for y in 0..dh {
        for x in 0..dw {
            let x0 = (x as f32 * fx) as u32;
            let x1 = ((((x + 1) as f32) * fx) as u32).min(sw).max(x0 + 1);
            let y0 = (y as f32 * fy) as u32;
            let y1 = ((((y + 1) as f32) * fy) as u32).min(sh).max(y0 + 1);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = ((yy.min(sh - 1) * sw + xx.min(sw - 1)) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += u32::from(src[i + c]);
                    }
                    n += 1;
                }
            }
            let o = ((y * dw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / n.max(1)) as u8;
            }
        }
    }
    out
}

/// Where a baked texture's pixels came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The PS2 disc's own texture.
    Ps2,
    /// PC stock, taken only because it is strictly larger in both dimensions.
    Pc,
    /// The Xbox disc's copy, by the same rule.
    Xbox,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Ps2 => "ps2",
            Tier::Pc => "pc",
            Tier::Xbox => "xbox",
        }
    }
}

/// A texture's pixels after the chain has chosen a source.
pub struct Resolved {
    pub width: u32,
    pub height: u32,
    /// RGBA8 mip levels, largest first.
    pub mips: Vec<Vec<u8>>,
    pub tier: Tier,
}

impl Resolved {
    /// The PS2 texture, decoded.
    pub fn from_ps2(t: &Texture) -> Resolved {
        Resolved {
            width: t.info.width,
            height: t.info.height,
            mips: (0..t.mips.len()).map(|i| t.rgba8(i)).collect(),
            tier: Tier::Ps2,
        }
    }

    /// Write the PS2 base and its current optional tier-2 sibling.
    pub(crate) fn write_pair(&self, dir: &Path, name: &str, tier2: Option<&Self>) -> Result<()> {
        self.write_ktx2(dir, &format!("{name}.ktx2"))?;
        let file = format!("{name}.tier2.ktx2");
        match tier2 {
            Some(t) => t.write_ktx2(dir, &file),
            None => Ok(crate::remove_optional_file(&dir.join(file))?),
        }
    }

    /// Force every texel opaque (the `segm MTRL` alpha gate — see
    /// [`crate::detail::ALPHA_IS_MEANINGFUL`]). Applied AFTER resolution so
    /// it lands on whichever tier's pixels are written.
    pub fn make_opaque(&mut self) {
        for m in &mut self.mips {
            for px in m.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
        }
    }

    /// Give this texture `base`'s alpha channel, resampled to its own
    /// dimensions — box-averaged where it shrinks, nearest where it grows.
    ///
    /// Tier 2 is a RENDER override for the SAME draw (PLAN §Classic-plus
    /// assets: a mid-round tier flip "is a pure renderer event"), and what a
    /// draw does with alpha was decided from PS2 geometry — `segm MTRL` via
    /// ALPHA RULE v2 — not from the texture. The PC file's own alpha is a
    /// different channel on several shipped sheets (gloss/spec; 30 PC
    /// textures carry alpha their PS2 counterpart does not, 13 of them with
    /// over 80 % of texels below the alpha test), so keeping it would make a
    /// MASK draw cut out along a mask nobody authored, and the two tiers
    /// would not draw the same silhouette.
    ///
    /// PROVISIONAL: the PS2 mask is carried at the PS2's own resolution, so a
    /// cutout edge is as coarse on tier 2 as on the disc. The alternative —
    /// the PC alpha where it agrees — remains a fidelity call for the lead.
    pub fn take_alpha_from(&mut self, base: &Resolved) {
        let Some(src) = base.mips.first() else { return };
        let (mut w, mut h) = (self.width.max(1), self.height.max(1));
        for m in &mut self.mips {
            let a = box_down(src, base.width, base.height, w, h);
            debug_assert_eq!(a.len(), m.len(), "mip {w}x{h} is not the size it claims");
            for (px, s) in m
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(a.as_chunks::<4>().0)
            {
                px[3] = s[3];
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
    }

    pub fn write_ktx2(&self, dir: &Path, file: &str) -> Result<()> {
        let bytes = crate::ktx2w::write_rgba8(self.width, self.height, &self.mips);
        std::fs::write(dir.join(file), bytes)?;
        Ok(())
    }
}

/// The PC LVL's textures, decoded once and keyed by lowercase name. `None`
/// when no PC LVL was supplied — every BF1 bake, and any BF2 bake run without
/// a PC install.
#[derive(Default)]
pub struct PcTextures {
    by_name: std::collections::HashMap<String, Resolved>,
}

/// The paths in a comma-joined donor list. A folder name can hold a comma, so a
/// piece that is no file is joined with the next until it is one; what never
/// becomes a file stays in the list, for the read to report.
pub fn donor_paths(list: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut held = String::new();
    for piece in list.split(',') {
        if !held.is_empty() {
            held.push(',');
        }
        held.push_str(piece);
        if std::path::Path::new(&held).is_file() {
            paths.push(std::mem::take(&mut held));
        }
    }
    if !held.is_empty() {
        paths.push(held);
    }
    paths
}

impl PcTextures {
    /// Decode every usable 2D texture of the donor LVLs: `lvl_paths` is one
    /// path, or several joined by commas (the PC's copy, then the Xbox's; see
    /// [`donor_paths`]), and the largest copy of each name wins, the earlier
    /// donor on a tie. Cube maps
    /// and surfaces whose mip chain does not decode cleanly are dropped here, so
    /// the resolver never has to think about them.
    pub fn load(lvl_paths: &str) -> Result<PcTextures> {
        let mut by_name = std::collections::HashMap::new();
        for lvl_path in donor_paths(lvl_paths) {
            let lvl_path = lvl_path.as_str();
            let buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_owned())?;
            let inner = mungefs::open(&buf).map_err(|e| anyhow::anyhow!("{lvl_path}: {e}"))?;
            // Within one LVL a repeated name replaces the earlier copy.
            let mut own = std::collections::HashMap::new();
            Self::load_pc(inner, &mut own);
            Self::load_xbox(inner, &mut own).map_err(|e| anyhow::anyhow!("{lvl_path}: {e}"))?;
            for (key, r) in own {
                let area = |x: &Resolved| u64::from(x.width) * u64::from(x.height);
                if by_name
                    .get(&key)
                    .is_none_or(|have: &Resolved| area(&r) > area(have))
                {
                    by_name.insert(key, r);
                }
            }
        }
        Ok(PcTextures { by_name })
    }

    /// The Xbox's plain surfaces, by the same rules as the PC's.
    fn load_xbox(
        inner: &[u8],
        by_name: &mut std::collections::HashMap<String, Resolved>,
    ) -> Result<(), mungefs::UcfbError> {
        for c in mungefs::content_chunks(inner)? {
            if &c.tag != b"tex_" || !mungefs::texture_xbox::is_xbox(&c) {
                continue;
            }
            let Ok(t) = mungefs::texture_xbox::Texture::parse(&c) else {
                continue;
            };
            if t.kind != mungefs::texture_xbox::KIND_PLAIN {
                continue;
            }
            let Some(mips) = t.faces.into_iter().next() else {
                continue;
            };
            by_name.insert(
                t.name.to_ascii_lowercase(),
                Resolved {
                    width: t.width,
                    height: t.height,
                    mips,
                    tier: Tier::Xbox,
                },
            );
        }
        Ok(())
    }

    fn load_pc(inner: &[u8], by_name: &mut std::collections::HashMap<String, Resolved>) {
        for t in texture_pc::find_textures(inner) {
            let Some(s) = t.best() else { continue };
            if s.is_cube() {
                continue;
            }
            let Some(face) = s.faces.first() else {
                continue;
            };
            // Keep only the leading mips whose byte length matches their
            // dimensions: the munger writes truncating block counts, so a
            // chain can go wrong partway down.
            let mips: Vec<Vec<u8>> = face
                .mips
                .iter()
                .enumerate()
                .take_while(|(i, m)| {
                    let (w, h) = s.mip_dims(*i);
                    m.rgba.len() == (w * h * 4) as usize
                })
                .map(|(_, m)| m.rgba.clone())
                .collect();
            if mips.is_empty() {
                continue;
            }
            by_name.insert(
                t.name.to_ascii_lowercase(),
                Resolved {
                    width: s.width,
                    height: s.height,
                    mips,
                    tier: Tier::Pc,
                },
            );
        }
    }

    fn larger_than(&self, name: &str, w: u32, h: u32) -> Option<&Resolved> {
        let r = self.by_name.get(name)?;
        (r.width > w && r.height > h).then_some(r)
    }
}

/// The classic-plus TIER 2 for this texture, or `None` when the chain has
/// nothing better than the disc: PC stock where strictly larger and the same
/// art. The Harrisonfog and upscale tiers slot in ahead of the PC test.
///
/// This replaces nothing. The maintainer's decision of 2026-09-03 (PLAN §Classic-plus
/// assets) makes render quality a RUNTIME toggle, so the bake writes BOTH
/// tiers side by side — the PS2 texture as `<name>.ktx2`, which stays the
/// only thing the sim ever reads, and this one beside it as
/// `<name>.tier2.ktx2`.
pub fn tier2(t: &Texture, pc: Option<&PcTextures>) -> Option<Resolved> {
    // Exclusion 1: a detail-compressed base's `_dtl` companion is registered
    // to the PS2 pixels (see the module doc).
    if t.info.detail_compressed {
        return None;
    }
    let name = t.name.to_ascii_lowercase();
    let better = pc?.larger_than(&name, t.info.width, t.info.height)?;
    same_art(
        &t.rgba8(0),
        t.info.width,
        t.info.height,
        &better.mips[0],
        better.width,
        better.height,
    )
    .then(|| Resolved {
        width: better.width,
        height: better.height,
        mips: better.mips.clone(),
        tier: better.tier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_texture_pair_rebake_clears_only_the_obsolete_override() {
        let dir =
            std::env::temp_dir().join(format!("openbf-tier-lifecycle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = Resolved::from_ps2(&ps2("Base", 1, 1, vec![20, 30, 40, 255]));
        let larger = Resolved {
            width: 2,
            height: 2,
            mips: vec![[50, 60, 70, 255].repeat(4)],
            tier: Tier::Pc,
        };
        let unrelated = dir.join("Other.tier2.ktx2");
        std::fs::write(&unrelated, b"keep").unwrap();
        larger.write_ktx2(&dir, "expected.ktx2").unwrap();
        base.write_pair(&dir, "Base", Some(&larger)).unwrap();
        let optional = dir.join("Base.tier2.ktx2");
        assert_eq!(
            std::fs::read(&optional).unwrap(),
            std::fs::read(dir.join("expected.ktx2")).unwrap()
        );
        let original = std::fs::read(dir.join("Base.ktx2")).unwrap();
        for _ in 0..2 {
            base.write_pair(&dir, "Base", None).unwrap();
            assert!(!optional.exists());
            assert_eq!(std::fs::read(dir.join("Base.ktx2")).unwrap(), original);
            assert_eq!(std::fs::read(&unrelated).unwrap(), b"keep");
        }
        std::fs::create_dir(&optional).unwrap();
        assert!(base.write_pair(&dir, "Base", None).is_err());
        std::fs::remove_dir(&optional).unwrap();
        // If the base cannot be written, its previous optional output survives.
        std::fs::remove_file(dir.join("Base.ktx2")).unwrap();
        std::fs::create_dir(dir.join("Base.ktx2")).unwrap();
        std::fs::write(&optional, b"previous").unwrap();
        assert!(base.write_pair(&dir, "Base", None).is_err());
        assert_eq!(std::fs::read(optional).unwrap(), b"previous");
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn res(w: u32, h: u32, tier: Tier) -> Resolved {
        Resolved {
            width: w,
            height: h,
            mips: vec![vec![0u8; (w * h * 4) as usize]],
            tier,
        }
    }

    fn pc_with(name: &str, w: u32, h: u32) -> PcTextures {
        let mut by_name = std::collections::HashMap::new();
        by_name.insert(name.to_string(), res(w, h, Tier::Pc));
        PcTextures { by_name }
    }

    /// Real data: BF2 Geonosis's Xbox disc fills tier 2 of the PS2 bake, and a
    /// missing comma list entry is refused rather than skipped. BF1's map gains
    /// nothing this way: its larger Xbox art sits on detail-compressed PS2 bases,
    /// which exclusion 1 keeps.
    #[test]
    fn an_xbox_lvl_is_a_donor_too() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        let xbox = format!("{root}/xbox_bf2/geo/geo1.lvl");
        let ps2 = format!("{root}/bf2/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL");
        let (Ok(_), Ok(buf)) = (std::fs::metadata(&xbox), std::fs::read(&ps2)) else {
            eprintln!("SKIP: no BF2 Geonosis");
            return;
        };
        let donors = PcTextures::load(&xbox).unwrap();
        let tex = mungefs::content_chunks(mungefs::open(&buf).unwrap())
            .unwrap()
            .into_iter()
            .filter(|c| &c.tag == b"tex_")
            .filter_map(|c| Texture::parse(&c).ok())
            .collect::<Vec<_>>();
        let filled: Vec<_> = tex
            .iter()
            .filter_map(|t| tier2(t, Some(&donors)).map(|r| (t, r)))
            .collect();
        eprintln!("{} of {} textures gain a tier 2", filled.len(), tex.len());
        assert!(filled.len() > 5, "{}", filled.len());
        for (t, r) in &filled {
            assert_eq!(r.tier, Tier::Xbox);
            assert!(r.width > t.info.width && r.height > t.info.height);
            assert_eq!(r.mips[0].len(), (r.width * r.height * 4) as usize);
        }
        assert!(PcTextures::load(&format!("{xbox},{root}/no/such.lvl")).is_err());
    }

    #[test]
    fn a_donor_list_survives_commas_in_a_folder_name() {
        let dir = std::env::temp_dir().join(format!("tier_donors_{}", std::process::id()));
        let folder = dir.join("Battlefront (USA, Europe)");
        std::fs::create_dir_all(&folder).unwrap();
        let (a, b) = (folder.join("a.lvl"), dir.join("b.lvl"));
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        let (a, b) = (a.display().to_string(), b.display().to_string());
        assert_eq!(donor_paths(&a), std::slice::from_ref(&a));
        assert_eq!(donor_paths(&format!("{a},{b}")), [a.clone(), b.clone()]);
        assert_eq!(donor_paths(&format!("{b},{a}")), [b.clone(), a.clone()]);
        assert_eq!(donor_paths(""), Vec::<String>::new());
        // A missing entry is not skipped: the read fails on it.
        let missing = format!("{b},{}/nope.lvl", dir.display());
        assert_eq!(donor_paths(&missing)[0], b);
        assert!(PcTextures::load(&missing).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pc_wins_only_when_strictly_larger_in_both_dimensions() {
        let pc = pc_with("x", 512, 512);
        assert!(pc.larger_than("x", 256, 256).is_some());
        // Equal in one dimension is not larger.
        assert!(pc.larger_than("x", 256, 512).is_none());
        assert!(pc.larger_than("x", 512, 512).is_none());
        assert!(pc.larger_than("x", 1024, 1024).is_none());
        // A name the PC LVL does not carry.
        assert!(pc.larger_than("y", 8, 8).is_none());
    }

    /// A 2x2 checkerboard upscaled to 4x4 is the SAME ART; its inverse is a
    /// different painting even though both have the same mean.
    #[test]
    fn same_art_accepts_a_sharper_copy_and_rejects_a_repaint() {
        let px = |v: u8| [v, v, v, 255u8];
        let mut ps2 = Vec::new();
        for y in 0..2 {
            for x in 0..2 {
                ps2.extend(px(if (x + y) % 2 == 0 { 220 } else { 20 }));
            }
        }
        let mut pc = Vec::new();
        let mut inv = Vec::new();
        for y in 0..4u32 {
            for x in 0..4u32 {
                let same = ((x / 2) + (y / 2)) % 2 == 0;
                pc.extend(px(if same { 220 } else { 20 }));
                inv.extend(px(if same { 20 } else { 220 }));
            }
        }
        assert!(same_art(&ps2, 2, 2, &pc, 4, 4));
        assert!(!same_art(&ps2, 2, 2, &inv, 4, 4));
    }

    /// The same art graded differently is still the same art: correlation
    /// carries it even though the levels are far apart.
    #[test]
    fn same_art_accepts_a_regrade() {
        let mut ps2 = Vec::new();
        let mut pc = Vec::new();
        for i in 0..16u32 {
            let v = (i * 15) as u8;
            ps2.extend([v, v, v, 255]);
            for _ in 0..4 {
                // Same ramp, halved and offset — a pure level change.
                let w = (v / 2).saturating_add(30);
                pc.extend([w, w, w, 255]);
            }
        }
        assert!(same_art(&ps2, 4, 4, &pc, 8, 8));
    }

    /// A flat colour has no structure to disagree about, so correlation must
    /// not veto it (`pol1_sky` differs by 2.4 of 255 and correlates at 0.00).
    #[test]
    fn a_flat_texture_is_never_rejected_for_lack_of_correlation() {
        let ps2 = [100u8, 100, 100, 255].repeat(4);
        let pc = [101u8, 101, 101, 255].repeat(16);
        assert!(same_art(&ps2, 2, 2, &pc, 4, 4));
    }

    /// A 32-bpp PS2 texture, `w` x `h`, mip 0 only.
    fn ps2(name: &str, w: u32, h: u32, px: Vec<u8>) -> Texture {
        Texture {
            name: name.into(),
            info: mungefs::texture::TextureInfo {
                width: w,
                height: h,
                bpp: 32,
                detail_compressed: false,
                mip_distance: 0.0,
                mip_count: 1,
            },
            palette: Vec::new(),
            mips: vec![px],
        }
    }

    /// Both tiers exist side by side: the PS2 texture is never replaced, and
    /// tier 2 is the PC art at the PC's own size.
    #[test]
    fn tier2_is_the_pc_art_and_the_base_stays_ps2() {
        let grey = [128u8, 128, 128, 255];
        let t = ps2("wall", 2, 2, grey.repeat(4));
        let pc = pc_with("wall", 4, 4);
        // `pc_with` makes an all-zero image; that is a flat texture, which
        // `same_art` accepts (no structure to disagree about).
        let o = tier2(&t, Some(&pc)).expect("the PC copy is larger and the same art");
        assert_eq!((o.width, o.height), (4, 4));
        assert_eq!(o.tier, Tier::Pc);
        assert_eq!(Resolved::from_ps2(&t).tier, Tier::Ps2);
        // No PC LVL, or a PC copy that is not larger: no second tier at all.
        assert!(tier2(&t, None).is_none());
        assert!(tier2(&ps2("wall", 4, 4, grey.repeat(16)), Some(&pc)).is_none());
        // Exclusion 1: a detail-compressed base keeps the PS2 pixels its
        // `_dtl` companion is registered to, so it gets no second tier.
        let mut dtl = ps2("wall", 2, 2, grey.repeat(4));
        dtl.info.detail_compressed = true;
        assert!(tier2(&dtl, Some(&pc)).is_none());
    }

    /// The tier flip must not change a draw's silhouette: tier 2 takes the
    /// PS2 alpha, nearest-sampled up to its own size, and box-averaged back
    /// down its own mip chain.
    #[test]
    fn tier2_takes_the_ps2_alpha() {
        // 2x2 PS2 mask: one transparent texel, three opaque.
        let base = Resolved {
            width: 2,
            height: 2,
            mips: vec![vec![
                10, 10, 10, 0, 20, 20, 20, 255, 30, 30, 30, 255, 40, 40, 40, 255,
            ]],
            tier: Tier::Ps2,
        };
        // 4x4 PC art with a full-alpha channel of its own (the gloss case).
        let mut o = Resolved {
            width: 4,
            height: 4,
            mips: vec![[99u8, 99, 99, 255].repeat(16), [99, 99, 99, 255].repeat(4)],
            tier: Tier::Pc,
        };
        o.take_alpha_from(&base);
        let a: Vec<u8> = o.mips[0].as_chunks::<4>().0.iter().map(|p| p[3]).collect();
        #[rustfmt::skip]
        assert_eq!(a, vec![
            0, 0, 255, 255,
            0, 0, 255, 255,
            255, 255, 255, 255,
            255, 255, 255, 255,
        ], "the PS2 mask, nearest-sampled to 4x4");
        // Mip 1 is 2x2 and takes the mask at its own resolution.
        let a1: Vec<u8> = o.mips[1].as_chunks::<4>().0.iter().map(|p| p[3]).collect();
        assert_eq!(a1, vec![0, 255, 255, 255]);
        // The colour is untouched — only the alpha channel moves.
        assert!(o.mips[0]
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[..3] == [99, 99, 99]));
    }

    #[test]
    fn the_opaque_gate_runs_on_the_resolved_pixels() {
        let mut r = Resolved {
            width: 1,
            height: 2,
            mips: vec![vec![9, 9, 9, 10, 9, 9, 9, 20]],
            tier: Tier::Pc,
        };
        r.make_opaque();
        assert_eq!(r.mips[0], vec![9, 9, 9, 255, 9, 9, 9, 255]);
    }
}
