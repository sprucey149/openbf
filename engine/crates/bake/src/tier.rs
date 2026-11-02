//! Texture source tiers: where a baked texture's pixels come from.
//!
//! The maintainer's chain (PLAN §Classic-plus assets, confirmed 2026-08-31):
//!
//! > **Harrisonfog HD > PC stock (only where LARGER) > our upscale > PS2
//! > original**
//!
//! This module implements the tiers that exist today — PS2, PC stock and the
//! Xbox's copy — and is shaped so the others slot in as extra resolvers ahead
//! of them.
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
//! A check that only asked for the same levels OR the same structure let through
//! a dark interior going near white (`kam_bldg_*_lowrez` correlates at 0.97), a
//! night sky going bright blue (`rhenvar2_sky`), a grey building going orange
//! (`geo_bldg_technounion`) and colour under a mask the donor never painted
//! (`kas2_mountains`, a white halo along the ridge). So [`same_art`] compares the
//! two at the PS2's size and on a small grid, for layout and colour, by the PS2's
//! alpha where a draw carries it:
//!
//! - a sky sets a map's whole look, so it must be the same picture at the same
//!   exposure; 12 of 54 candidates pass;
//! - anything else fails on another hue, on a brightness step past 1.8x, on a
//!   layout that no longer lines up, or on a halo.
//!
//! Calibrated over a whole bake of both games with every donor, each change viewed
//! on contact sheets: textures with a second tier went from 764 to 715 of 849,
//! and all of the named wrong picks above are refused. The largest copy failing
//! is not the end, because a name's other copies are tried next: PC's
//! `imp_1st_weap_inf_launcher` is a repaint and the Xbox's is the same sheet.
//! A rejection is a line in the bake's log.
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

/// What a draw does with a texture, which decides how hard the same-art check is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// A sky shell draws it, so it sets the look of the whole map.
    pub sky: bool,
    /// A draw reads its alpha, so tier 2 carries the PS2 mask over the donor's
    /// colour, and only what that mask shows can be seen.
    pub alpha_carried: bool,
}

/// The long side the layout comparison shrinks both images to.
const LAYOUT_CELLS: u32 = 16;

/// The numbers the same-art check decides on. Both images are compared at the PS2's
/// size and at a small [`LAYOUT_CELLS`] grid, weighted by the PS2 alpha when it is
/// carried: the texels it hides have no colour anyone sees.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Metrics {
    /// Mean absolute per-channel difference at the PS2's size, of 255.
    pub diff: f64,
    /// Luma correlation at the PS2's size.
    pub corr: f64,
    /// Mean absolute per-channel difference on the small grid.
    pub layout_diff: f64,
    /// Luma correlation on the small grid.
    pub layout_corr: f64,
    /// Standard deviation of the PS2's luma on the small grid: below a few of 255
    /// the image is flat and a correlation is noise.
    pub layout_std: f64,
    /// Mean luma of each image.
    pub luma: [f64; 2],
    /// Mean per-cell chroma distance on the small grid: a part of the image
    /// painted another colour shows here, even when the average agrees.
    pub layout_chroma: f64,
    /// Share of the PS2's texels with a middling alpha. A cut-out mask has few; a
    /// soldier or weapon sheet keeps gloss there and its alpha test decides nothing.
    pub soft_alpha: f64,
    /// Of the texels the PS2 mask shows, the share the donor's own alpha hides:
    /// its colour there was never meant to be seen. 0 without a donor mask.
    pub hidden: f64,
    /// Mean absolute per-channel difference over those [`Self::hidden`] texels. Small
    /// where the donor's alpha is gloss over the same colours; large where the
    /// colour under its mask was never painted, which a carried PS2 mask would show
    /// as a halo.
    pub hidden_diff: f64,
}

/// Is the donor the SAME ART as the PS2 texture? See the module doc. Each number
/// here was set against the whole set's contact sheets; a rule below only ever
/// narrows the one before, so nothing the plain check refused can pass.
pub fn same_art(m: &Metrics, usage: Usage) -> bool {
    // The plain check: neither the same levels nor the same structure.
    if m.diff > 12.0 && m.corr < 0.5 {
        return false;
    }
    let flat = m.layout_std < 6.0;
    let luma_step = (m.luma[0] - m.luma[1]).abs();
    // A sky sets the look of the whole map: only the same picture, at the same exposure.
    if usage.sky {
        return m.layout_diff <= 6.0
            && m.layout_chroma <= 8.0
            && luma_step <= 6.0
            && (flat || (m.layout_corr >= 0.9 && m.corr >= 0.9))
            && !halo(m);
    }
    if halo(m) || m.layout_chroma > 20.0 {
        return false;
    }
    // Dark to near white, whatever the structure says. The floor keeps a near-black
    // texture from failing on a step nobody can see.
    let darker = m.luma[0].min(m.luma[1]).max(16.0);
    if m.luma[0].max(m.luma[1]) / darker > 1.8 || luma_step > 60.0 {
        return false;
    }
    if flat {
        return m.layout_diff <= 25.0;
    }
    // Colours this close look the same whatever a correlation of their noise says.
    m.layout_diff <= 6.0 || (m.layout_corr >= 0.75 && (m.layout_corr >= 0.88 || m.corr >= 0.8))
}

/// Colour the donor never painted, under a mask that would now show it.
fn halo(m: &Metrics) -> bool {
    m.soft_alpha < 0.5 && m.hidden > 0.02 && m.hidden_diff > 32.0
}

fn luma_of(px: &[u8]) -> f64 {
    0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2])
}

/// Measure a donor (mip 0, `dw` x `dh`) against the PS2 mip 0 (`w` x `h`).
pub fn compare(
    ps2: &[u8],
    w: u32,
    h: u32,
    donor: &[u8],
    dw: u32,
    dh: u32,
    usage: Usage,
) -> Metrics {
    let down = box_down(donor, dw, dh, w, h);
    let (cw, ch) = layout_grid(w, h);
    let (small_a, small_b) = (box_down(ps2, w, h, cw, ch), box_down(donor, dw, dh, cw, ch));
    let full = pair_stats(ps2, &down, usage);
    let small = pair_stats(&small_a, &small_b, usage);
    let (mut soft, mut shown, mut hidden, mut hidden_sum) = (0u64, 0u64, 0u64, 0u64);
    for (a, b) in ps2.as_chunks::<4>().0.iter().zip(down.as_chunks::<4>().0) {
        let ps2_shows = a[3] >= 128;
        soft += u64::from((32..224).contains(&a[3]));
        shown += u64::from(ps2_shows);
        if ps2_shows && b[3] < 128 {
            hidden += 1;
            hidden_sum += (0..3).map(|c| u64::from(a[c].abs_diff(b[c]))).sum::<u64>();
        }
    }
    Metrics {
        diff: full.diff,
        corr: full.corr,
        layout_diff: small.diff,
        layout_corr: small.corr,
        layout_std: small.std,
        luma: small.luma,
        layout_chroma: small.chroma,
        soft_alpha: soft as f64 / (ps2.len() / 4).max(1) as f64,
        hidden: hidden as f64 / shown.max(1) as f64,
        hidden_diff: hidden_sum as f64 / (hidden.max(1) * 3) as f64,
    }
}

/// The small grid for an image of this shape: the long side [`LAYOUT_CELLS`], the
/// short side in proportion, and no larger than the image.
fn layout_grid(w: u32, h: u32) -> (u32, u32) {
    let long = w.max(h).max(1);
    let scale = |side: u32| (side * LAYOUT_CELLS).div_ceil(long).clamp(1, side.max(1));
    (scale(w), scale(h))
}

/// Two same-sized images compared, by the weight [`Usage::alpha_carried`] gives each texel.
struct Pair {
    diff: f64,
    corr: f64,
    /// The first image's luma deviation.
    std: f64,
    luma: [f64; 2],
    /// Mean per-texel distance in chroma (`r-g`, `b-g`).
    chroma: f64,
}

fn pair_stats(a: &[u8], b: &[u8], usage: Usage) -> Pair {
    let pairs = || a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0);
    // A mask that hides everything shows nothing to compare: judge the colours as they are.
    let weigh = usage.alpha_carried && pairs().any(|(p, _)| p[3] > 0);
    let weight = |p: &[u8; 4]| if weigh { f64::from(p[3]) / 255.0 } else { 1.0 };
    let total = pairs().map(|(p, _)| weight(p)).sum::<f64>().max(1e-9);
    let chroma = |p: &[u8; 4]| {
        [
            f64::from(p[0]) - f64::from(p[1]),
            f64::from(p[2]) - f64::from(p[1]),
        ]
    };
    let (mut sum, mut per_texel, mut ma, mut mb) = (0.0, 0.0, 0.0, 0.0);
    for (p, q) in pairs() {
        let k = weight(p);
        sum += k * (0..3).map(|c| f64::from(p[c].abs_diff(q[c]))).sum::<f64>();
        ma += k * luma_of(p);
        mb += k * luma_of(q);
        let (x, y) = (chroma(p), chroma(q));
        per_texel += k * (x[0] - y[0]).hypot(x[1] - y[1]);
    }
    let (ma, mb) = (ma / total, mb / total);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for (p, q) in pairs() {
        let k = weight(p);
        let (u, v) = (luma_of(p) - ma, luma_of(q) - mb);
        num += k * u * v;
        da += k * u * u;
        db += k * v * v;
    }
    Pair {
        diff: sum / (total * 3.0),
        // A constant image correlates with everything: there is no structure to disagree about.
        corr: if da <= 1e-9 || db <= 1e-9 {
            1.0
        } else {
            num / (da.sqrt() * db.sqrt())
        },
        std: (da / total).sqrt(),
        luma: [ma, mb],
        chroma: per_texel / total,
    }
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
    /// Every donor's copy of each name, largest first; on a tie the earlier donor first.
    by_name: std::collections::HashMap<String, Vec<Resolved>>,
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
    /// [`donor_paths`]), and every copy of each name is kept, largest first and the
    /// earlier donor first on a tie, because the largest may not be the same art.
    /// Cube maps and surfaces whose mip chain does not decode cleanly are dropped
    /// here, so the resolver never has to think about them.
    pub fn load(lvl_paths: &str) -> Result<PcTextures> {
        let mut by_name: std::collections::HashMap<String, Vec<Resolved>> =
            std::collections::HashMap::new();
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
                let copies = by_name.entry(key).or_default();
                let at = copies.partition_point(|have| area(have) >= area(&r));
                copies.insert(at, r);
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

    /// Every copy of `name` strictly larger than `w` x `h`, largest first.
    fn larger_than<'a>(&'a self, name: &str, w: u32, h: u32) -> impl Iterator<Item = &'a Resolved> {
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .filter(move |r| r.width > w && r.height > h)
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
pub fn tier2(t: &Texture, pc: Option<&PcTextures>, usage: Usage) -> Option<Resolved> {
    // Exclusion 1: a detail-compressed base's `_dtl` companion is registered
    // to the PS2 pixels (see the module doc).
    if t.info.detail_compressed {
        return None;
    }
    let name = t.name.to_ascii_lowercase();
    let base = t.rgba8(0);
    let (w, h) = (t.info.width, t.info.height);
    pc?.larger_than(&name, w, h).find_map(|better| {
        let m = compare(&base, w, h, &better.mips[0], better.width, better.height, usage);
        let same = same_art(&m, usage);
        if !same {
            println!(
                "tier 2: {name}: the {} copy at {}x{} is other art (differs by {:.0} of 255, layout {:.2})",
                better.tier.as_str(),
                better.width,
                better.height,
                m.layout_diff,
                m.layout_corr,
            );
        }
        same.then(|| Resolved {
            width: better.width,
            height: better.height,
            mips: better.mips.clone(),
            tier: better.tier,
        })
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

    fn same(a: &[u8], w: u32, h: u32, b: &[u8], dw: u32, dh: u32, usage: Usage) -> bool {
        same_art(&compare(a, w, h, b, dw, dh, usage), usage)
    }

    fn pc_with(name: &str, w: u32, h: u32) -> PcTextures {
        let mut by_name = std::collections::HashMap::new();
        by_name.insert(name.to_string(), vec![res(w, h, Tier::Pc)]);
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
            .filter_map(|t| tier2(t, Some(&donors), Usage::default()).map(|r| (t, r)))
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

    /// Playtest on BF2 Geonosis: a faint straight red line across a cliff, on the bunker
    /// mesh whose UVs reach the bottom of `geo_bldg_spire_01`. The PC copy ends in three
    /// red rows the PS2's does not have. It must stay refused, and what replaces it
    /// must end in the colour of the rows behind that edge.
    #[test]
    fn the_geonosis_cliff_takes_no_red_edge_from_a_donor() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
        let ps2 = format!("{root}/extracted/bf2/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL");
        let xbox = format!("{root}/extracted/xbox_bf2/geo/geo1.lvl");
        let pc = format!(
            "{root}/PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/geo/geo1.lvl"
        );
        let (has_pc, has_xbox) = (Path::new(&pc).is_file(), Path::new(&xbox).is_file());
        let (Ok(buf), true) = (std::fs::read(&ps2), has_pc || has_xbox) else {
            eprintln!("SKIP: no BF2 Geonosis with a donor");
            return;
        };
        let sheet = mungefs::content_chunks(mungefs::open(&buf).unwrap())
            .unwrap()
            .into_iter()
            .filter(|c| &c.tag == b"tex_")
            .filter_map(|c| Texture::parse(&c).ok())
            .find(|t| t.name.eq_ignore_ascii_case("geo_bldg_spire_01"))
            .expect("the sheet");
        if has_pc {
            let pc_only = PcTextures::load(&pc).unwrap();
            assert!(tier2(&sheet, Some(&pc_only), Usage::default()).is_none());
        }
        if !has_xbox {
            eprintln!("SKIP: no Xbox Geonosis, so no replacement to check");
            return;
        }
        let donors = if has_pc { format!("{pc},{xbox}") } else { xbox };
        let both = PcTextures::load(&donors).unwrap();
        let got = tier2(&sheet, Some(&both), Usage::default()).expect("the Xbox copy");
        assert_eq!(got.tier, Tier::Xbox);
        let (w, h) = (got.width as usize, got.height as usize);
        let hue = |from: usize, to: usize| {
            let rows = &got.mips[0][from * w * 4..to * w * 4];
            let n = (rows.len() / 4) as f64;
            let sum = |c: usize| {
                rows.iter()
                    .skip(c)
                    .step_by(4)
                    .map(|&v| f64::from(v))
                    .sum::<f64>()
            };
            (sum(0) - sum(1)) / n
        };
        let step = (hue(h - 3, h) - hue(h - 11, h - 3)).abs();
        assert!(step < 8.0, "red minus green steps by {step} at the edge");
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
        assert!(pc.larger_than("x", 256, 256).next().is_some());
        // Equal in one dimension is not larger.
        assert!(pc.larger_than("x", 256, 512).next().is_none());
        assert!(pc.larger_than("x", 512, 512).next().is_none());
        assert!(pc.larger_than("x", 1024, 1024).next().is_none());
        // A name the PC LVL does not carry.
        assert!(pc.larger_than("y", 8, 8).next().is_none());
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
        assert!(same(&ps2, 2, 2, &pc, 4, 4, Usage::default()));
        assert!(!same(&ps2, 2, 2, &inv, 4, 4, Usage::default()));
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
        assert!(same(&ps2, 4, 4, &pc, 8, 8, Usage::default()));
    }

    /// A flat colour has no structure to disagree about, so correlation must
    /// not veto it (`pol1_sky` differs by 2.4 of 255 and correlates at 0.00).
    #[test]
    fn a_flat_texture_is_never_rejected_for_lack_of_correlation() {
        let ps2 = [100u8, 100, 100, 255].repeat(4);
        let pc = [101u8, 101, 101, 255].repeat(16);
        assert!(same(&ps2, 2, 2, &pc, 4, 4, Usage::default()));
    }

    const SKY: Usage = Usage {
        sky: true,
        alpha_carried: false,
    };

    /// A horizontal ramp of `w` x `h`, each texel `f(ramp value)`.
    fn ramp(w: u32, h: u32, f: impl Fn(u8) -> [u8; 4]) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| f(((i % w) * 255 / w.max(2).saturating_sub(1)) as u8))
            .collect()
    }

    /// `rhenvar2_sky`: a night sky brightened on the PC keeps its structure and
    /// still changes the whole map. The same regrade passes on a wall.
    #[test]
    fn a_sky_must_keep_its_exposure() {
        let night = ramp(32, 16, |v| [v / 8, v / 8, v / 6, 255]);
        let day = ramp(64, 32, |v| [v / 3, v / 2, v, 255]);
        assert!(!same(&night, 32, 16, &day, 64, 32, SKY));
        assert!(same(
            &night,
            32,
            16,
            &ramp(64, 32, |v| [v / 8, v / 8, v / 6, 255]),
            64,
            32,
            SKY
        ));
    }

    /// `kam_bldg_*_lowrez`: dark to near white correlates perfectly.
    #[test]
    fn dark_to_near_white_is_not_the_same_art() {
        let grey = |base: u8| move |v: u8| [base + v / 8, base + v / 8, base + v / 8, 255];
        let dark = ramp(32, 32, grey(50));
        let white = ramp(64, 64, grey(200));
        assert!(!same(&dark, 32, 32, &white, 64, 64, Usage::default()));
    }

    /// `geo_bldg_technounion`: a surface repainted another hue.
    #[test]
    fn another_hue_is_not_the_same_art() {
        let grey = ramp(32, 32, |v| [80 + v / 4, 80 + v / 4, 80 + v / 4, 255]);
        let orange = ramp(64, 64, |v| [130 + v / 4, 80 + v / 4, 30 + v / 4, 255]);
        assert!(!same(&grey, 32, 32, &orange, 64, 64, Usage::default()));
    }

    /// `kas2_mountains`: the donor's mask hides colour it never painted, and the PS2
    /// mask carried over it shows that colour. A soldier sheet's alpha is gloss,
    /// where the same disagreement decides nothing.
    #[test]
    fn a_cut_out_shows_a_halo_and_gloss_does_not() {
        let carried = Usage {
            sky: false,
            alpha_carried: true,
        };
        let paint = |v: u8, a: u8| [100 + v / 4, 90 + v / 4, 80 + v / 4, a];
        // The right eighth is shown by the PS2 and hidden, in white, by the donor.
        let ps2 = ramp(32, 32, |v| paint(v, 255));
        let donor = ramp(64, 64, |v| {
            if v >= 224 {
                [255, 255, 255, 0]
            } else {
                paint(v, 255)
            }
        });
        assert!(!same(&ps2, 32, 32, &donor, 64, 64, carried));
        let gloss = ramp(32, 32, |v| paint(v, 96));
        assert!(same(
            &gloss,
            32,
            32,
            &ramp(64, 64, |v| paint(v, 96)),
            64,
            64,
            carried
        ));
        let m = compare(&gloss, 32, 32, &donor, 64, 64, carried);
        assert!(m.soft_alpha > 0.9 && !halo(&m));
    }

    /// The largest donor failing leaves the next one to try.
    #[test]
    fn a_failed_largest_donor_falls_through_to_the_next() {
        let good = ramp(16, 16, |v| [v, v, v, 255]);
        let repaint = ramp(32, 32, |v| [255 - v, 255 - v, 255 - v, 255]);
        let resolved = |w, h, mips: Vec<u8>, tier| Resolved {
            width: w,
            height: h,
            mips: vec![mips],
            tier,
        };
        let mut donors = PcTextures::default();
        donors.by_name.insert(
            "wall".into(),
            vec![
                resolved(32, 32, repaint, Tier::Pc),
                resolved(16, 16, good.clone(), Tier::Xbox),
            ],
        );
        let base = ps2("wall", 8, 8, ramp(8, 8, |v| [v, v, v, 255]));
        let got = tier2(&base, Some(&donors), Usage::default()).expect("the smaller copy passes");
        assert_eq!((got.width, got.tier), (16, Tier::Xbox));
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
        let black = [0u8, 0, 0, 255];
        let t = ps2("wall", 2, 2, black.repeat(4));
        let pc = pc_with("wall", 4, 4);
        // `pc_with` makes an all-zero image: flat, and the same black.
        let o =
            tier2(&t, Some(&pc), Usage::default()).expect("the PC copy is larger and the same art");
        assert_eq!((o.width, o.height), (4, 4));
        assert_eq!(o.tier, Tier::Pc);
        assert_eq!(Resolved::from_ps2(&t).tier, Tier::Ps2);
        // No PC LVL, or a PC copy that is not larger: no second tier at all.
        assert!(tier2(&t, None, Usage::default()).is_none());
        assert!(tier2(
            &ps2("wall", 4, 4, black.repeat(16)),
            Some(&pc),
            Usage::default()
        )
        .is_none());
        // Exclusion 1: a detail-compressed base keeps the PS2 pixels its
        // `_dtl` companion is registered to, so it gets no second tier.
        let mut dtl = ps2("wall", 2, 2, black.repeat(4));
        dtl.info.detail_compressed = true;
        assert!(tier2(&dtl, Some(&pc), Usage::default()).is_none());
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
