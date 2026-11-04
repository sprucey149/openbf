//! ALPHA RULE v2 — the per-DRAW verdict on what a texture's alpha means.
//!
//! **PROVISIONAL.** The rule is the lead's spec (PLAN Log, 2026-09-01,
//! "Texture alpha census over every baked texture — and ALPHA RULE v2"),
//! derived from a measured census of 2,182 baked textures joined to 14,746
//! `segm` materials (`engine/artifacts/texture_census/`). HOLD 10(g) — what
//! the PS2 actually does with a material's alpha — is read out of the ELF at
//! the CPU-visible level (`analysis/re-material-flags.md`); the census rule
//! stands for the BF1 and BF2 words the overrides below do not cover until that document's §0 item 7 (the ELF rule per game and class, with a
//! PCSX2 capture for its §8) is built. Do not tune the thresholds here
//! without the lead.
//!
//! Why per draw: `bake::map` used to OR the material flag word of every
//! segment naming a texture and decide once per TEXTURE. That OR is lossy —
//! 33 % of textures with a real material are named by segments with different
//! flag words, and `kam_prop_crate`'s body strip (`0x05 Normal`) inherited
//! `Scroll` and `Specmap` from three other segments of the same model. The
//! GLB already has per-draw granularity (one primitive per non-empty `segm`),
//! so the verdict is computed here and written on the primitive's material.
//!
//! **The SKY path is OUT OF SCOPE (design D7, PROVISIONAL, deferred).** Sky
//! shells are drawn through their own pipeline (`render`'s `fs_sky`,
//! ALPHA_BLENDING) which composites by alpha and never
//! looks at the material — so for a sky texture the baked ALPHA CHANNEL is
//! what decides how the shell reads, and applying D3 to it changes the sky.
//! Two goldens showed it: BF1 BES1's `sky_bespin1_clouds_2` (one draw, 0x04
//! Normal, mid 0.896 → rule 3 OPAQUE) lost its alpha and the translucent cloud
//! sea became a solid sheet, and BF2 MUS1's `mus1_sky_dome` (a0 0.94) gained
//! one, so the base dome composited 94 % nothing over the clear colour. A
//! texture the sky path uses therefore keeps the PRE-v2 alpha state — see
//! [`keep_alpha`]. The GLB `alphaMode`/`extras` on sky-model primitives are
//! still written by the rule; nothing reads them, because `game::viewer`
//! uploads sky shells down their own path. What the PS2 clears to behind a
//! transparent base dome is a HOLD 10 question, not this build's.
//!
//! **BF1 overrides (card ELFR, 2026-09-29):** where BF1's ELF segment rule
//! (`analysis/re-material-flags.md` §0.4 [V]) contradicts the census on a
//! Normal word with neither 0x02 nor 0x04 (OPAQUE, no test) or a Scroll word
//! with 0x04 (BLEND), the ELF wins — [`bf1_elf_verdict`]; the GLB's
//! `extras.rule` says `"elf"` there. PCSX2 walkabout pairs settled both.
//!
//! **BF2 override (sprint, 2026-11-01):** BF2's draw is the same on the point
//! that matters: a word with neither 0x02 nor 0x04 gets TEST 0x50000, no test
//! and no blend (`re-material-flags.md` §0.3 and §2's BF2 table [V]). The
//! census's class-D clause cut those draws out anyway, which took most of
//! Dagobah's cave wall (`dag1_prop_cave_C`, 0x10201, 53 % of its texels at
//! alpha 0) and showed the sky through it. See [`bf2_elf_verdict`].
//!
//! The stats are taken from the DECODED PS2 texture, never from the PC tier:
//! where a PC file replaced the pixels its alpha is a different channel
//! (gloss/spec), and judging a cutout from it invents three false ones (the
//! census README, "Class D is decided on the PS2 alpha").

use mungefs::model::{Model, RenderType, Segment};
use mungefs::texture::Texture;
use std::collections::HashMap;

/// Flag bits that mark the alpha channel a MODULATION channel (scroll,
/// specular, glow) rather than a mask — the rule's `mod` term. MEASURED
/// P(alpha is not a mask): 0x10 → 0.69, 0x40 → 0.56, 0x80 → 0.71,
/// 0x400 → 0.76, 0x1000000 → 0.81, against a base rate of 0.29.
/// PROVISIONAL (HOLD 10(g)).
pub const MODULATION_BITS: u32 = 0x10 | 0x40 | 0x80 | 0x400 | 0x1000000;

/// `segm MTRL` bit 0x400, the one modulation bit the rule reads on its own
/// (rule 2's BLEND clause). PROVISIONAL (HOLD 10(g), as [`MODULATION_BITS`]).
pub const SCROLL_BIT: u32 = 0x400;

/// A BF1 segment's draw class, which the loader picks from the flag word
/// alone — the RTYP string is hashed and never forwarded
/// (`analysis/re-material-flags.md` §0.4 and §2 [V], BF1 568105–568160).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bf1Class {
    /// 0x20 (checked first): opaque + an additive spec pass; 0x02 and 0x04
    /// ignored.
    Specular,
    /// 0x400 without 0x20 or 0x200: 0x02 ignored, 0x04 blends.
    Scroll,
    /// 0x800 | 0x4000 without the above (no shipped word uses them).
    Blur,
    /// Everything else, 0x200 words with 0x400 included: 0x02 tests
    /// (AFAIL FB_ONLY), 0x04 blends, neither is opaque with no test.
    Normal,
}

impl Bf1Class {
    /// The class the BF1 loader builds for this flag word, in its test
    /// order: 0x20, then 0x200 (Normal, whatever else), then 0x400, then
    /// 0x800|0x4000.
    pub fn of(flags: u32) -> Bf1Class {
        if flags & 0x20 != 0 {
            Bf1Class::Specular
        } else if flags & 0x200 != 0 {
            Bf1Class::Normal
        } else if flags & 0x400 != 0 {
            Bf1Class::Scroll
        } else if flags & (0x800 | 0x4000) != 0 {
            Bf1Class::Blur
        } else {
            Bf1Class::Normal
        }
    }
}

/// Card ELFR: BF1 draws take the ELF's segment rule where the census
/// disagrees with it (see [`bf1_elf_verdict`]). On by default; set
/// `OPENBF_BF1_ALPHA_CENSUS=1` for the bake to fall back to the census alone.
pub fn bf1_elf_rule_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("OPENBF_BF1_ALPHA_CENSUS").is_none())
}

/// What BF1's ELF draws where it contradicts the census, or `None` where it
/// does not rule (card ELFR, `analysis/re-material-flags.md` §0.4 [V] and
/// §2's BF1 draw per class):
/// - a Normal-class word with neither 0x02 nor 0x04 draws OPAQUE with no
///   test — the texture alpha only reaches the frame buffer — so the census's
///   class-D cutouts (KAS2 City Hall's thatch walls, the platform huts'
///   bamboo, GEO1's wrecked lattice) are drawn solid, as PCSX2 shows them;
/// - a Scroll-class word with 0x04 BLENDS by texture alpha whatever its
///   histogram (YAV1's temple and BES1's control-room holo panels,
///   `scanlines` 0x415).
///
/// The other §0.6 disagreements are not taken here: a Normal 0x04 word
/// (`kam_crate`'s 0x05 body) stays the census's, and so does every 0x02
/// word (the leaves/bamboo 0x07 MASK agree already).
pub fn bf1_elf_verdict(flags: u32) -> Option<AlphaMode> {
    match Bf1Class::of(flags) {
        Bf1Class::Normal if flags & crate::detail::ALPHA_IS_MEANINGFUL == 0 => {
            Some(AlphaMode::Opaque)
        }
        Bf1Class::Scroll if flags & crate::detail::TRANSPARENT != 0 => Some(AlphaMode::Blend),
        _ => None,
    }
}

/// What BF2's ELF draws where it contradicts the census (see the module doc):
/// a word with neither 0x02 nor 0x04 is OPAQUE with no test. A translucent
/// word without the cutout bit is BLEND regardless of its texture histogram
/// or specular flag. Words carrying 0x02 retain the existing census verdict.
pub fn bf2_elf_verdict(flags: u32) -> Option<AlphaMode> {
    match flags & crate::detail::ALPHA_IS_MEANINGFUL {
        0 => Some(AlphaMode::Opaque),
        crate::detail::TRANSPARENT => Some(AlphaMode::Blend),
        _ => None,
    }
}

/// Whether the disc alpha-tests this draw — the rule's `mod` term never
/// outranks it (queue item 9, 2026-09-24). BF2 tests every word with 0x02
/// and without 0x04 (TEST 0x5000b, `re-material-flags.md` §2's BF2 draw
/// table, the 0x02 row [V], 723478–723479); its other bits pick no other
/// ALPHA/TEST state without 0x04 (the same table and §4: 0x40 and 0x08 act
/// only when translucent, 0x80/0x100 are second passes, 0x0f000000 is the UV
/// animation, 0x10 only under the pause dim). BF1 tests 0x02 in the Normal class only (§0.4 [V]). A word with 0x04
/// is left to the rule as before. COR1's sky traffic (`cor1_sky_cars`,
/// 0x1000002) was the tell: a white card whose alpha is the cars, which rule
/// 2 drew OPAQUE as white bands across the sky.
pub fn disc_alpha_tests(flags: u32, rtyp: &RenderType) -> bool {
    let class_honours_it = match rtyp {
        RenderType::Bf1(_) => Bf1Class::of(flags) == Bf1Class::Normal,
        RenderType::Bf2(_) | RenderType::Absent => true,
    };
    flags & crate::detail::CUTOUT != 0
        && flags & crate::detail::TRANSPARENT == 0
        && class_honours_it
}

/// Rule 2's and rule 3's mid-band threshold: a mid-band fraction at or above
/// this says the alpha is a smooth ramp, below it says a mask. The census's
/// threshold sweep chose 0.30 because 0.15 would box Endor's 471 trees
/// (mid 0.254) and Kashyyyk's bamboo (0.214) while 0.30 flips the Kamino
/// crate (0.550) and boxes nothing the contact sheets show as a real cutout.
/// PROVISIONAL.
pub const MID_BAND: f32 = 0.30;

/// Rule 3's unflagged (class D) clause: at least this fraction of texels must
/// be essentially transparent before an unflagged texture is treated as a
/// cutout. PROVISIONAL.
pub const CLASS_D_A0: f32 = 0.05;

/// Rule 3's unflagged clause: and its mid band must be at most this — a class
/// D cutout is sharply bimodal. PROVISIONAL.
pub const CLASS_D_MID: f32 = 0.10;

/// Rule 1: a draw whose word asks for its alpha (0x02 or 0x04) and whose
/// texture's alpha max is at or below this is composited, whatever else the
/// word says. The disc has no such clause (`analysis/re-material-flags.md`
/// §6, point 4); it is the port's, and `myg1_mountains` (0x03, alpha max 115,
/// which an alpha test would delete) is why. PROVISIONAL.
///
/// A word with neither bit is not rule 1's (queue R1U, 2026-09-24): the disc
/// draws it opaque whatever its alpha, which only reaches the frame buffer
/// (ibid. §2 [V]). Rule 1 used to catch it too, and composited 37 such draws
/// in the shipped bake — COR1's security-room doors and DEA1's reactor walls
/// among them, whose alpha is a glow mask (max 53 and 78), so they drew
/// nearly invisible.
///
/// It is NOT the renderer's alpha test, by decision (queue NT, 2026-09-24).
/// It was the old 0.5 test until queue AT (`a55688f`) moved the world's test
/// to the disc's (`crate::detail::ALPHA_TEST`, disc alpha 193), and following
/// it was measured and declined: rule 1 runs BEFORE rules 2 and 3, so at 193
/// it re-routes every draw it catches whose alpha max lies in 129..192,
/// whatever its verdict. Measured before R1U, the raise turned 23 OPAQUE
/// draws to BLEND, and the disc blends only 8 of them (`kam_bldg_window_01`'s
/// 0x04 words); the other 15 were words with neither bit, which the disc
/// draws opaque and R1U took out of rule 1's reach. Measured again on top of
/// R1U, it turns exactly those 8 to BLEND and moves no other verdict — and
/// queue KW (2026-09-24) gave the 8 their BLEND in rule 3's 0x04 clause
/// instead, where the disc's reading of the bit belongs, so this stays 128:
/// the raise would also composite a 0x02 word in 129..192, which BF2's disc
/// draws nothing of (AFAIL KEEP, ibid. §2 [V]) — exactly what MASK draws.
pub const NEVER_TESTS: u8 = 128;

/// Alpha statistics of one texture's mip 0, on the PS2 pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaStats {
    /// Fraction of texels with alpha in 32..=223 — neither clearly on nor
    /// clearly off. A cutout mask is bimodal (near 0); grime or specular
    /// noise is not.
    pub mid: f32,
    /// Fraction of texels with alpha <= 31.
    pub a0: f32,
    /// Maximum alpha over mip 0.
    pub amax: u8,
}

impl AlphaStats {
    /// Measure mip 0 of a decoded PS2 texture.
    pub fn of(t: &Texture) -> AlphaStats {
        Self::of_rgba8(&t.rgba8(0))
    }

    /// As [`AlphaStats::of`], over a tightly-packed RGBA8 buffer.
    pub fn of_rgba8(rgba: &[u8]) -> AlphaStats {
        let mut mid = 0usize;
        let mut lo = 0usize;
        let mut amax = 0u8;
        let mut n = 0usize;
        for px in rgba.as_chunks::<4>().0 {
            let a = px[3];
            n += 1;
            if a <= 31 {
                lo += 1;
            } else if a <= 223 {
                mid += 1;
            }
            amax = amax.max(a);
        }
        if n == 0 {
            return AlphaStats {
                mid: 0.0,
                a0: 0.0,
                amax: 255,
            };
        }
        AlphaStats {
            mid: mid as f32 / n as f32,
            a0: lo as f32 / n as f32,
            amax,
        }
    }
}

/// The verdict for one draw: which glTF alpha mode its material gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    /// Alpha ignored: no discard, depth written.
    Opaque,
    /// Alpha-tested ("hardedged" on the disc), depth written, at `render`'s
    /// `MASK_CUTOFF` (disc alpha 193 since AT). World draws only: a skinned
    /// draw is never tested, because the disc tests no side model
    /// (`crate::soldier::segment_blends`).
    Mask,
    /// Composited by the blend pass, no depth write.
    Blend,
}

impl AlphaMode {
    /// The glTF 2.0 `alphaMode` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            AlphaMode::Opaque => "OPAQUE",
            AlphaMode::Mask => "MASK",
            AlphaMode::Blend => "BLEND",
        }
    }
}

/// Which clause of the rule decided a draw — written into the GLB's material
/// `extras` so a census can re-derive every verdict from the GLBs alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// Rule 1: a word with 0x02 or 0x04 and `amax <= 128` → BLEND.
    NeverTests,
    /// Rule 2: the `mod` term is set.
    Modulation,
    /// Rule 3: no `mod` term — the flag bits and the histogram decide (and,
    /// for a word without 0x02, the alpha max).
    Mask,
    /// The segment names no texture, or one this LVL does not carry.
    None,
    /// The ELF's segment rule overrode the census ([`bf1_elf_verdict`], card
    /// ELFR; [`bf2_elf_verdict`]).
    Elf,
}

impl Rule {
    /// The `extras.rule` value: the clause number, or `"none"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::NeverTests => "1",
            Rule::Modulation => "2",
            Rule::Mask => "3",
            Rule::None => "none",
            Rule::Elf => "elf",
        }
    }
}

/// Everything the bake writes about one drawn segment's material.
#[derive(Debug, Clone)]
pub struct SegmentAlpha {
    /// Lowercase name of the texture the verdict was taken on ("" for a
    /// segment that names none).
    pub texture: String,
    pub mode: AlphaMode,
    pub rule: Rule,
    pub flags: u32,
    pub rtyp: RenderType,
    /// `None` when the segment names no texture, or one the LVL does not
    /// carry — that draw is OPAQUE by the rule's last line.
    pub stats: Option<AlphaStats>,
}

/// The verdict the bake writes: ALPHA RULE v2, with BF1's ELF segment rule
/// overriding it where the two disagree (card ELFR) unless
/// [`bf1_elf_rule_enabled`] is off, and BF2's always.
pub fn verdict(flags: u32, rtyp: &RenderType, stats: Option<&AlphaStats>) -> (AlphaMode, Rule) {
    verdict_with(flags, rtyp, stats, bf1_elf_rule_enabled())
}

/// [`verdict`] with the ELFR switch passed in. The override is taken only
/// where it changes the mode, so a draw both rules agree on keeps the
/// census's clause in its `extras`.
pub fn verdict_with(
    flags: u32,
    rtyp: &RenderType,
    stats: Option<&AlphaStats>,
    bf1_elf: bool,
) -> (AlphaMode, Rule) {
    let census = census_verdict(flags, rtyp, stats);
    let elf = match rtyp {
        _ if stats.is_none() => None,
        RenderType::Bf1(_) if bf1_elf => bf1_elf_verdict(flags),
        RenderType::Bf2(_) => bf2_elf_verdict(flags),
        _ => None,
    };
    match elf {
        Some(mode) if mode != census.0 => (mode, Rule::Elf),
        _ => census,
    }
}

/// ALPHA RULE v2 as a pure function. `stats` is `None` for a segment naming
/// no texture, or a texture the LVL does not carry: OPAQUE, rule `none`.
pub fn census_verdict(
    flags: u32,
    rtyp: &RenderType,
    stats: Option<&AlphaStats>,
) -> (AlphaMode, Rule) {
    let Some(s) = stats else {
        return (AlphaMode::Opaque, Rule::None);
    };
    // 1. A word that asks for its alpha, and alpha that never gets past half:
    //    compositing is the only way it draws at all. Half is the old test,
    //    not the renderer's (see NEVER_TESTS for why it stays). A word with
    //    neither bit falls through — the disc draws it opaque (queue R1U).
    if flags & crate::detail::ALPHA_IS_MEANINGFUL != 0 && s.amax <= NEVER_TESTS {
        return (AlphaMode::Blend, Rule::NeverTests);
    }
    // 2. The alpha is a modulation channel. It is a BLEND only for the narrow
    //    scrolling/transparent case (fountains, waterfalls, mist, lamp glow);
    //    everything else — the crate's Scroll strip, antennae, poles, bunker
    //    panels — is OPAQUE, which is the whole point of the rule. A word
    //    the disc alpha-tests is rule 3's cutout whatever its `mod` bits.
    let modulation =
        (flags & MODULATION_BITS != 0 || rtyp.is_scroll_like()) && !disc_alpha_tests(flags, rtyp);
    if modulation {
        let blend = flags & crate::detail::TRANSPARENT != 0
            && (rtyp.is_scroll_like() || flags & SCROLL_BIT != 0)
            && s.mid >= MID_BAND;
        return (
            if blend {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            Rule::Modulation,
        );
    }
    // 3. No modulation: 0x02 is the disc's own cutout bit; 0x04 is a cutout
    //    only when the histogram is bimodal; an unflagged texture is a cutout
    //    only on the sharp class-D signature (banners, grate, rope, deck
    //    edge, Jawa junk; BF2 and BF1's Normal class overrule this in
    //    [`verdict_with`]), and only if a texel passes the world's test —
    //    below `ALPHA_TEST` that MASK would draw nothing, where the disc
    //    draws the word opaque (queue R1U). Below the test a 0x04 word has
    //    nothing to cut out either, so it takes the disc's reading of the bit
    //    and composites (queue KW: Kamino's windows).
    let mode = if flags & crate::detail::CUTOUT != 0 {
        AlphaMode::Mask
    } else if flags & crate::detail::TRANSPARENT != 0 {
        if s.amax < crate::detail::ALPHA_TEST {
            AlphaMode::Blend
        } else if s.mid <= MID_BAND {
            AlphaMode::Mask
        } else {
            AlphaMode::Opaque
        }
    } else if s.a0 >= CLASS_D_A0 && s.mid <= CLASS_D_MID && s.amax >= crate::detail::ALPHA_TEST {
        AlphaMode::Mask
    } else {
        AlphaMode::Opaque
    };
    (mode, Rule::Mask)
}

/// The muzzle flash models every weapon class attaches (card FXMF,
/// `analysis/re-muzzle-flash.md` §1): BF1 `muzzleflash`, BF2
/// `com_sfx_muzzleflash` (`core::flash::BF1_MODEL` / `BF2_MODEL`).
pub const FLASH_MODELS: [&str; 2] = ["muzzleflash", "com_sfx_muzzleflash"];

/// The sheets a flash model's segments draw (lowercase): the flash is drawn
/// additively BY their alpha (§4.3), so a bake keeps it — the caller's
/// `consumer_named` clause of [`keep_alpha`]. Empty for any other model.
pub fn flash_textures(m: &Model) -> Vec<String> {
    if !FLASH_MODELS.iter().any(|f| m.name.eq_ignore_ascii_case(f)) {
        return Vec::new();
    }
    m.segments
        .iter()
        .filter_map(segment_texture)
        .map(str::to_ascii_lowercase)
        .collect()
}

/// The sheets a BF2 self-lit segment draws (lowercase) on a map whose
/// `Effect("HDR")` is enabled (card GLOW, `crate::glb::glow_path`): the glow
/// is × (1 + As/128) and As is the texture's alpha (`analysis/re-glow.md`
/// §1.2), so a bake keeps it even where every draw is OPAQUE — the caller's
/// `consumer_named` clause of [`keep_alpha`]. Empty when `hdr` is off.
pub fn glow_textures(m: &Model, hdr: bool) -> Vec<String> {
    m.segments
        .iter()
        .filter(|s| crate::glb::glow_path(s, hdr) == Some(crate::glb::LightPath::Glow))
        .filter_map(segment_texture)
        .map(str::to_ascii_lowercase)
        .collect()
}

/// PS2 alpha stats for every texture an LVL carries, measured once and shared
/// by every segment that names one.
pub struct AlphaTable {
    stats: HashMap<String, AlphaStats>,
}

/// The texture a segment draws with: its LOWEST TNAM slot.
///
/// The one place the four emitters have to agree. `crate::glb` and
/// `crate::soldier` write it as the GLB material name with the DISC's case,
/// which is why this returns the name unchanged; the two lookup paths (this
/// module's stats table and `soldier`'s wanted-texture list) lowercase it
/// themselves, because `bake::map`'s texture index is lowercase.
pub fn segment_texture(seg: &Segment) -> Option<&str> {
    seg.textures
        .iter()
        .min_by_key(|(slot, _)| *slot)
        .map(|(_, n)| n.as_str())
}

impl AlphaTable {
    /// Measure every decoded texture (mip 0), keyed by lowercase name.
    pub fn new(textures: &[Texture]) -> AlphaTable {
        AlphaTable {
            stats: textures
                .iter()
                .map(|t| (t.name.to_ascii_lowercase(), AlphaStats::of(t)))
                .collect(),
        }
    }

    /// The verdict for one segment.
    pub fn segment(&self, seg: &Segment) -> SegmentAlpha {
        let texture = segment_texture(seg)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let stats = if texture.is_empty() {
            None
        } else {
            self.stats.get(&texture).copied()
        };
        let (mode, rule) = verdict(seg.material_flags, &seg.render_type, stats.as_ref());
        SegmentAlpha {
            texture,
            mode,
            rule,
            flags: seg.material_flags,
            rtyp: seg.render_type.clone(),
            stats,
        }
    }

    /// The verdict for every segment of a model, in segment order (aligned
    /// with `Model::segments`, including the empty ones the GLB writer skips).
    pub fn model(&self, m: &Model) -> Vec<SegmentAlpha> {
        m.segments.iter().map(|s| self.segment(s)).collect()
    }
}

/// The verdicts the OLD per-texture rule implies, for the side WEAPON bake,
/// which ALPHA RULE v2 does NOT cover (design D6): a segment is MASK when the
/// OR of the material flags over every segment naming its texture marks the
/// alpha meaningful, OPAQUE otherwise. That is exactly what the renderer did
/// for those models before the rule landed, so writing it on the material
/// changes nothing for them while the runtime routes from the material rather
/// than from the texture. No baked weapon segment has 0x02 or 0x04
/// (`analysis/re-side-alpha.md` §1), so every one comes out OPAQUE; the
/// soldiers take the disc's own per-segment rule instead
/// (`crate::soldier::segment_blends`).
pub fn legacy_per_texture_or(m: &Model) -> Vec<SegmentAlpha> {
    let flags = crate::soldier::texture_flags(m);
    m.segments
        .iter()
        .map(|seg| {
            let texture = segment_texture(seg)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let or = flags.get(&texture).copied().unwrap_or_default();
            SegmentAlpha {
                texture,
                mode: if or & crate::detail::ALPHA_IS_MEANINGFUL != 0 {
                    AlphaMode::Mask
                } else {
                    AlphaMode::Opaque
                },
                rule: Rule::None,
                flags: seg.material_flags,
                rtyp: seg.render_type.clone(),
                stats: None,
            }
        })
        .collect()
}

/// D7 (PROVISIONAL): should the baked KTX2 keep its alpha channel?
///
/// For a world texture this is design D3: keep the alpha iff at least one
/// DRAW in this bake was judged MASK or BLEND — or, since queue item SM, iff
/// NO segment of any model in the bake names the texture at all (`unnamed`;
/// the CALLER decides which bakes ask that question — `bake::map::Content`
/// passes it only for the common bakes).
///
/// Why the second clause: a sheet no model draws is not a world texture, and
/// D3's "nothing asked for the alpha, so bake it opaque" was wrong for the
/// ones the PS2 does draw. Every HUD bitmap on both discs is composited
/// source-over by exactly this alpha with no alpha test
/// (`analysis/re-hud-icons.md` §2 [V]) and its shape IS that alpha —
/// `squad_cmd_highlight` has 1,594 of 4,096 texels at 0 — so forcing it
/// opaque drew a solid square (211 of BF1 COMMON.LVL's 224 alpha-bearing
/// sheets went that way, §2). A map's terrain, water and effect sheets are
/// named by no segment either, and the lead scoped the clause AWAY from them
/// (2026-09-17): the terrain draws through the opaque pipeline, which never
/// reads texture alpha, but the water pass composites by it (`render`'s
/// `fs_water`), and what the PS2's water layer does with that alpha is
/// unread VU1 (`analysis/re-water.md` §8, HOLD 21). Freeing it there moved
/// four map goldens past their limits — a look change on an unproven rule —
/// so the map half waited on that measurement (queue item WA).
///
/// **That measurement is in, and `consumer_named` is its clause** (queue item
/// WA, 2026-09-17; `analysis/re-water-alpha.md` §0, `[V][O]` UNAUDITED).
/// Every BLENDED water layer on both discs binds its sheet through the
/// engine's one texture path, which sets TEX0 `TCC = 1` and leaves `TFX = 0`
/// MODULATE (BF1 `FUN_0049c5c8` 562034, the bit byte-verified in the ELF at
/// va 0x49c9f0; BF2 `FUN_00582c90` 717181) and then blends `ALPHA 0x44`
/// source-over — so `As = At x Av >> 7` and the sheet's own alpha reaches the
/// composite. The port's `fs_water` multiply of `base.a` is already that
/// arithmetic; it is inert only while the bake forces the sheet opaque.
///
/// For the water, `consumer_named` is TRUE only for a sheet THIS map's own
/// `fx__ Effect("Water")` names — its `MainTexture` or its `FoamTexture`. The lead
/// scoped it that narrowly (2026-09-17) because the `[V]` evidence reaches
/// the water draw and stops there: the wide "every unnamed map sheet" rule
/// would also free terrain DTEX, foliage, mist and the `bird`/`fish` sheets
/// on a warrant that does not cover them. Reversing it is this one clause.
/// The one sheet on either disc that carries alpha 255 everywhere is
/// `kam1_water_2`, the ocean BASE — the one water layer the disc does NOT
/// blend (`re-water-alpha.md` §4), so the clause is a no-op on exactly the
/// layer whose blend does not read it.
///
/// The same input carries a second consumer (queue CLN5, 2026-09-25): the
/// map's `LOWR` far-LOD sheet texture (`DomeInfo.LowResTerrain.Texture`). The
/// PS2 alpha-tests that draw at AREF 0x10 (`analysis/re-lowr.md` §2.2 [V],
/// TEST 0x5010b), and two of the 14 drawn sheets carry real alpha on the disc
/// — BF1 KAS1 `kashyyyk1` (95 % of texels 0) and BF1 KAS2 `kas2` (42 %) —
/// so an opaque bake would fill the holes the disc cuts. The caller ORs the
/// two; the name is `consumer_named` because the clause is "a consumer whose
/// draw reads this sheet's alpha names it", which both are. A third (queue
/// FOLGG): the sheet a map's `grasspatch` class names as its `Texture` — the
/// grass sprites are cutouts composited by it (`analysis/re-foliage.md` §7).
///
/// For a texture the SKY path uses it is the PRE-v2 answer instead — see the
/// sky note in the module doc. `old_or_flags` is the OLD per-texture OR of the
/// material flag word, with `ALPHA_IS_MEANINGFUL` where no material names the
/// texture (exactly what `unwrap_or` did before v2 — so an unnamed sky texture
/// already kept its alpha). A sky texture that a NON-sky draw also needs the
/// alpha of still keeps it, so on a shared texture the carve-out can only ADD
/// alpha relative to v2, never take it away.
pub fn keep_alpha(
    is_sky: bool,
    old_or_flags: u32,
    world_draw_needs_alpha: bool,
    unnamed: bool,
    consumer_named: bool,
) -> bool {
    if is_sky {
        old_or_flags & crate::detail::ALPHA_IS_MEANINGFUL != 0 || world_draw_needs_alpha
    } else {
        world_draw_needs_alpha || unnamed || consumer_named
    }
}

/// What the DRAWN segments of a bake asked of each texture — the input to the
/// baked KTX2's alpha decision (designs D3 and D7): a texture keeps its alpha
/// iff at least one draw is MASK or BLEND, and lands in the manifest's derived
/// `transparent` list iff at least one draw is BLEND.
///
/// Alpha requests retain the MODEL names so D7 can exclude sky dome models.
/// Blended textures need only membership: any blending draw adds the texture.
#[derive(Default, Debug)]
pub struct AlphaUsage {
    needs_alpha: HashMap<String, std::collections::BTreeSet<String>>,
    blended: std::collections::HashSet<String>,
    by_model: HashMap<String, std::collections::BTreeSet<String>>,
}

impl AlphaUsage {
    /// Record the verdicts of one model's DRAWN segments (a segment with no
    /// triangles is written to no primitive, so it asks nothing).
    pub fn record_model(&mut self, m: &Model, alphas: &[SegmentAlpha]) {
        let model = m.name.to_ascii_lowercase();
        for (seg, a) in m.segments.iter().zip(alphas) {
            if seg.triangles.is_empty() || a.texture.is_empty() {
                continue;
            }
            self.by_model
                .entry(model.clone())
                .or_default()
                .insert(a.texture.clone());
            match a.mode {
                AlphaMode::Opaque => {}
                AlphaMode::Mask => self.note(&a.texture, &model, false),
                AlphaMode::Blend => self.note(&a.texture, &model, true),
            }
        }
    }

    fn note(&mut self, texture: &str, model: &str, blend: bool) {
        self.needs_alpha
            .entry(texture.to_owned())
            .or_default()
            .insert(model.to_owned());
        if blend {
            self.blended.insert(texture.to_owned());
        }
    }

    /// Does any draw in this bake need this texture's alpha?
    pub fn needs_alpha(&self, texture_lower: &str) -> bool {
        self.needs_alpha.contains_key(texture_lower)
    }

    /// Does any draw OUTSIDE `exclude` (D7: the sky dome models) need this
    /// texture's alpha?
    pub fn needs_alpha_outside(
        &self,
        texture_lower: &str,
        exclude: &std::collections::HashSet<String>,
    ) -> bool {
        self.needs_alpha
            .get(texture_lower)
            .is_some_and(|models| models.iter().any(|m| !exclude.contains(m)))
    }

    /// Is any draw in this bake compositing this texture?
    pub fn blended(&self, texture_lower: &str) -> bool {
        self.blended.contains(texture_lower)
    }

    /// Every texture the drawn segments of `model_lower` name.
    pub fn textures_of(&self, model_lower: &str) -> impl Iterator<Item = &str> {
        self.by_model
            .get(model_lower)
            .into_iter()
            .flatten()
            .map(String::as_str)
    }

    /// Models with at least one drawn segment naming `texture_lower`.
    pub fn models_using<'a>(&'a self, texture_lower: &'a str) -> impl Iterator<Item = &'a str> {
        self.by_model
            .iter()
            .filter(move |(_, t)| t.contains(texture_lower))
            .map(|(m, _)| m.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Card FXMF: each game's flash model names its flash sheet, and that
    /// sheet's alpha is the flame (mean 38/255 on the disc,
    /// `analysis/re-muzzle-flash.md` §4.3) — what the bake must keep.
    #[test]
    fn flash_models_name_their_alpha_sheet() {
        let ex = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted/");
        for (lvl, model, sheet) in [
            (
                "bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL",
                "muzzleflash",
                "meshmuzzleflash",
            ),
            (
                "bf2/lvl/DATA/_LVL_PS2/INGAME.LVL",
                "com_sfx_muzzleflash",
                "com_sfx_meshmuzzleflash",
            ),
        ] {
            let Ok(buf) = std::fs::read(format!("{ex}{lvl}")) else {
                eprintln!("SKIP: {lvl} not present");
                continue;
            };
            let inner = mungefs::open(&buf).unwrap();
            let chunks = mungefs::content_chunks(inner).unwrap();
            let m = chunks
                .iter()
                .filter(|c| &c.tag == b"modl")
                .filter_map(|c| Model::parse(c).ok())
                .find(|m| m.name.eq_ignore_ascii_case(model))
                .unwrap_or_else(|| panic!("{model} not in {lvl}"));
            let other = chunks
                .iter()
                .filter(|c| &c.tag == b"modl")
                .filter_map(|c| Model::parse(c).ok())
                .find(|o| !o.name.eq_ignore_ascii_case(model))
                .unwrap();
            assert!(flash_textures(&other).is_empty(), "{}", other.name);
            let names = flash_textures(&m);
            assert!(
                !names.is_empty() && names.iter().all(|n| n == sheet),
                "{names:?}"
            );
            let t = chunks
                .iter()
                .filter(|c| &c.tag == b"tex_")
                .filter_map(|c| Texture::parse(c).ok())
                .find(|t| t.name.eq_ignore_ascii_case(sheet))
                .unwrap_or_else(|| panic!("{sheet} not in {lvl}"));
            let rgba = t.rgba8(0);
            let mean = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| f64::from(p[3]))
                .sum::<f64>()
                / (rgba.len() / 4) as f64;
            eprintln!("{sheet}: mean alpha {mean:.1}");
            assert!((mean - 38.0).abs() < 1.5, "{sheet}: mean alpha {mean}");
            assert!(
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3] == [255, 255, 255]),
                "{sheet} is white"
            );
        }
    }

    fn bf1(s: &str) -> RenderType {
        RenderType::Bf1(s.to_owned())
    }
    fn stats(mid: f32, a0: f32, amax: u8) -> AlphaStats {
        AlphaStats { mid, a0, amax }
    }

    /// The PLAN entry's named cases, one per clause of the rule.
    #[test]
    fn the_rule_over_the_plans_named_cases() {
        // The census alone: BF1's ELF override has its own test (ELFR).
        let verdict = census_verdict;
        // `kam_crate` on `kam_prop_crate`'s BODY segment: 0x05 Normal, 72 %
        // of texels below the test, mid-band 0.55. Rule 3's 0x04 clause:
        // mid > 0.30, so the alpha is grime, not a mask — OPAQUE. This is the
        // shredded crate the whole rule exists for.
        let crate_body = stats(0.55, 0.05, 255);
        assert_eq!(
            verdict(0x05, &bf1("Normal"), Some(&crate_body)),
            (AlphaMode::Opaque, Rule::Mask)
        );
        // The same texture's Scroll strip (0x410) and Specmap strip (0x31)
        // are rule 2 and OPAQUE too — 0x04 is not set on either.
        assert_eq!(
            verdict(0x410, &bf1("Scroll"), Some(&crate_body)),
            (AlphaMode::Opaque, Rule::Modulation)
        );
        assert_eq!(
            verdict(0x31, &bf1("Specmap"), Some(&crate_body)),
            (AlphaMode::Opaque, Rule::Modulation)
        );

        // Endor's leaves: 0x07 Normal — bit 0x02 is the disc's cutout flag,
        // and it wins whatever the mid band says (the leaves sit at 0.254).
        assert_eq!(
            verdict(0x07, &bf1("Normal"), Some(&stats(0.254, 0.45, 255))),
            (AlphaMode::Mask, Rule::Mask)
        );
        // BF2's leaves, RTYP 0.
        assert_eq!(
            verdict(
                0x1_0003,
                &RenderType::Bf2(0),
                Some(&stats(0.254, 0.45, 255))
            ),
            (AlphaMode::Mask, Rule::Mask)
        );

        // A fountain: 0x04 + Scroll + a smooth ramp — composited.
        assert_eq!(
            verdict(0x04, &bf1("Scroll"), Some(&stats(0.62, 0.10, 255))),
            (AlphaMode::Blend, Rule::Modulation)
        );
        // The same material with bit 0x400 instead of the Scroll RTYP.
        assert_eq!(
            verdict(0x404, &bf1("Normal"), Some(&stats(0.62, 0.10, 255))),
            (AlphaMode::Blend, Rule::Modulation)
        );
        // …but not when the ramp is a mask: mid below 0.30 → OPAQUE.
        assert_eq!(
            verdict(0x404, &bf1("Normal"), Some(&stats(0.10, 0.10, 255))),
            (AlphaMode::Opaque, Rule::Modulation)
        );

        // `myg1_mountains`: 0x03, alpha max 115 — nothing reaches the test,
        // so it draws NOTHING today. Rule 1 outranks the 0x02 cutout bit.
        assert_eq!(
            verdict(0x03, &RenderType::Bf2(0), Some(&stats(0.30, 0.02, 115))),
            (AlphaMode::Blend, Rule::NeverTests)
        );

        // A Theed banner: unflagged, sharply bimodal — class D, a real cutout
        // that renders as a solid rectangle today.
        assert_eq!(
            verdict(0x01, &bf1("Normal"), Some(&stats(0.02, 0.40, 255))),
            (AlphaMode::Mask, Rule::Mask)
        );
        // Unflagged and NOT bimodal enough: OPAQUE (the class-D clause is
        // deliberately narrow — a0 >= 0.05 AND mid <= 0.10).
        assert_eq!(
            verdict(0x01, &bf1("Normal"), Some(&stats(0.02, 0.02, 255))),
            (AlphaMode::Opaque, Rule::Mask)
        );
        assert_eq!(
            verdict(0x01, &bf1("Normal"), Some(&stats(0.20, 0.40, 255))),
            (AlphaMode::Opaque, Rule::Mask)
        );

        // A segment naming no texture, or one the LVL does not carry.
        assert_eq!(
            verdict(0x07, &bf1("Normal"), None),
            (AlphaMode::Opaque, Rule::None)
        );
    }

    /// The thresholds are boundaries, not ranges: >= for the mid band in rule
    /// 2, <= in rule 3, >= for a0.
    #[test]
    fn the_thresholds_are_inclusive_where_the_spec_says() {
        // The census alone: BF1's ELF override has its own test (ELFR).
        let verdict = census_verdict;
        // rule 2 BLEND: mid >= 0.30
        assert_eq!(
            verdict(0x404, &bf1("Normal"), Some(&stats(0.30, 0.0, 255))).0,
            AlphaMode::Blend
        );
        assert_eq!(
            verdict(0x404, &bf1("Normal"), Some(&stats(0.2999, 0.0, 255))).0,
            AlphaMode::Opaque
        );
        // rule 3, 0x04: MASK when mid <= 0.30
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.30, 0.0, 255))).0,
            AlphaMode::Mask
        );
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.3001, 0.0, 255))).0,
            AlphaMode::Opaque
        );
        // rule 3, unflagged: a0 >= 0.05 and mid <= 0.10
        assert_eq!(
            verdict(0, &bf1("Normal"), Some(&stats(0.10, 0.05, 255))).0,
            AlphaMode::Mask
        );
        assert_eq!(
            verdict(0, &bf1("Normal"), Some(&stats(0.10, 0.0499, 255))).0,
            AlphaMode::Opaque
        );
        // rule 1: amax <= 128 (128 was the first alpha the old 0.5 test
        // KEPT, so a texture whose max is exactly 128 had one surviving
        // texel — the spec still composites it). Past the boundary a 0x02
        // word is rule 3's cutout, and a 0x04 word still composites, now by
        // rule 3's 0x04 clause (queue KW).
        assert_eq!(
            verdict(0x02, &bf1("Normal"), Some(&stats(0.5, 0.0, 128))),
            (AlphaMode::Blend, Rule::NeverTests)
        );
        assert_eq!(
            verdict(0x02, &bf1("Normal"), Some(&stats(0.5, 0.0, 129))),
            (AlphaMode::Mask, Rule::Mask)
        );
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.5, 0.0, 128))),
            (AlphaMode::Blend, Rule::NeverTests)
        );
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.5, 0.0, 129))),
            (AlphaMode::Blend, Rule::Mask)
        );
        // rule 3, 0x04: BLEND while amax < ALPHA_TEST (193); from 193 on,
        // the histogram decides again — a smooth ramp OPAQUE, a mask MASK.
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.5, 0.0, 192))).0,
            AlphaMode::Blend
        );
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.5, 0.0, 193))).0,
            AlphaMode::Opaque
        );
        // Below the test the amax clause wins over the histogram: a bimodal
        // 0x04 mask whose texels all fail the test would draw nothing as
        // MASK, where the disc composites the word (queue KW).
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.1, 0.5, 150))),
            (AlphaMode::Blend, Rule::Mask)
        );
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.1, 0.0, 193))).0,
            AlphaMode::Mask
        );
        // rule 3, unflagged class D: MASK only when amax >= ALPHA_TEST (193,
        // the first alpha the world's test keeps), OPAQUE below it.
        assert_eq!(
            verdict(0, &bf1("Normal"), Some(&stats(0.0, 0.5, 193))).0,
            AlphaMode::Mask
        );
        assert_eq!(
            verdict(0, &bf1("Normal"), Some(&stats(0.0, 0.5, 192))).0,
            AlphaMode::Opaque
        );
    }

    /// Queue R1U: the disc draws a word with neither 0x02 nor 0x04 OPAQUE
    /// whatever its alpha, which only reaches the frame buffer
    /// (`analysis/re-material-flags.md` §2 [V]). So rule 1 no longer
    /// composites it, and rule 3's class-D clause no longer cuts it out when
    /// no texel would pass the world's test — that MASK drew nothing.
    #[test]
    fn a_word_with_neither_bit_below_the_test_draws_opaque() {
        let bf2 = RenderType::Bf2(0);
        // COR1's `Cor1_Bldg_security_roomDoor`: alpha 0 but for a glow strip
        // (max 53), a0 0.979 / mid 0.021 — class D. Rule 1 composited it at
        // most 21 % opaque on all four of its words; now its two unflagged
        // words are rule 3's and its two 0x10 words rule 2's, OPAQUE all.
        let door = stats(0.021, 0.979, 53);
        for w in [0x1_0001, 0x1_0201] {
            assert_eq!(
                verdict(w, &bf2, Some(&door)),
                (AlphaMode::Opaque, Rule::Mask)
            );
        }
        for w in [0x1_0010, 0x1_0210] {
            assert_eq!(
                verdict(w, &bf2, Some(&door)),
                (AlphaMode::Opaque, Rule::Modulation)
            );
        }
        // DEA1's `deal1_bldg_reactor_room02` (max 78, green light bars) and
        // BF1 BES1's `kam_bldg_antenna` (0x1 Normal, max 116): not class D,
        // so rule 3's last line.
        assert_eq!(
            verdict(0x1_0201, &bf2, Some(&stats(0.143, 0.857, 78))),
            (AlphaMode::Opaque, Rule::Mask)
        );
        assert_eq!(
            verdict(0x1, &bf1("Normal"), Some(&stats(0.715, 0.285, 116))),
            (AlphaMode::Opaque, Rule::Mask)
        );
        // The door's class-D stats with a texel past the test: the census
        // still calls it a cutout, and BF2's ELF rule draws it solid.
        assert_eq!(
            census_verdict(0x1_0001, &bf2, Some(&stats(0.021, 0.979, 200))),
            (AlphaMode::Mask, Rule::Mask)
        );
        assert_eq!(
            verdict(0x1_0001, &bf2, Some(&stats(0.021, 0.979, 200))),
            (AlphaMode::Opaque, Rule::Elf)
        );
        // A word that asks for its alpha stays rule 1's: 0x05 at 92 (the
        // Falcon's shadow) and 0x03 at 115 (`myg1_mountains`).
        assert_eq!(
            verdict(0x05, &bf1("Normal"), Some(&stats(0.531, 0.469, 92))),
            (AlphaMode::Blend, Rule::NeverTests)
        );
        assert_eq!(
            verdict(0x03, &bf2, Some(&stats(0.30, 0.02, 115))),
            (AlphaMode::Blend, Rule::NeverTests)
        );
    }

    /// Queue KW: BF2 KAM1's `Kam_bldg_window_01` is glass — alpha 123..183 on
    /// every texel, mid band 1.0 — and the disc composites every 0x04 word
    /// (`analysis/re-material-flags.md` §2 [V]: ALPHA 0x44, Z-tested, no Z
    /// write). No texel passes the world's test, so rule 3's histogram has
    /// nothing to cut out, and the window draws BLEND on its three 0x04 words;
    /// its unflagged word stays OPAQUE, as the disc draws it (queue R1U).
    #[test]
    fn kaminos_window_composites_by_its_0x04_words() {
        let bf2 = RenderType::Bf2(0);
        let window = stats(1.0, 0.0, 183);
        for w in [0x1_0005, 0x205, 0x1_0205] {
            assert_eq!(
                verdict(w, &bf2, Some(&window)),
                (AlphaMode::Blend, Rule::Mask),
                "0x{w:x}"
            );
        }
        assert_eq!(
            verdict(0x1_0001, &bf2, Some(&window)),
            (AlphaMode::Opaque, Rule::Mask)
        );
        assert_eq!(
            verdict(0x1_0005, &bf2, Some(&stats(1.0, 0.0, 193))),
            (AlphaMode::Blend, Rule::Elf)
        );
    }

    /// Card ELFR: BF1's ELF segment rule overrides the census where the two
    /// disagree (`analysis/re-material-flags.md` §0.4 [V]), and only there.
    #[test]
    fn bf1_takes_the_elf_segment_rule_where_the_census_disagrees() {
        let normal = bf1("Normal");
        // KAS2 City Hall's `kas2_bldg_hut_walls` / the platform huts'
        // `kas2_bldg_platform_small`: 0x1, a class-D texture (the thatch's
        // bottom quarter is alpha 0). The census cuts it out; the ELF draws a
        // Normal word with neither bit opaque, no test.
        let thatch = stats(0.02, 0.26, 255);
        assert_eq!(
            verdict_with(0x1, &normal, Some(&thatch), false),
            (AlphaMode::Mask, Rule::Mask)
        );
        assert_eq!(
            verdict_with(0x1, &normal, Some(&thatch), true),
            (AlphaMode::Opaque, Rule::Elf)
        );
        // BF2 Dagobah's cave wall: 0x10201, 53 % of the texels at alpha 0. The
        // census cuts it out and the sky shows through; the ELF draws it solid.
        let wall = stats(0.035, 0.528, 255);
        let bf2 = RenderType::Bf2(0);
        assert_eq!(
            census_verdict(0x10201, &bf2, Some(&wall)),
            (AlphaMode::Mask, Rule::Mask)
        );
        assert_eq!(
            verdict_with(0x10201, &bf2, Some(&wall), false),
            (AlphaMode::Opaque, Rule::Elf)
        );
        // The same texture on the cave's 0x10203 roots stays a cutout.
        assert_eq!(
            verdict_with(0x10203, &bf2, Some(&wall), true),
            (AlphaMode::Mask, Rule::Mask)
        );
        // 0x200 outranks 0x400 in the class dispatch: 0x602 is Normal, so
        // its 0x02 is tested.
        assert_eq!(Bf1Class::of(0x602), Bf1Class::Normal);
        assert!(disc_alpha_tests(0x602, &normal));
        assert!(!disc_alpha_tests(0x402, &normal));
        // YAV1's `scanlines` 0x415 (Scroll | 0x10 | 0x04 | lit): the census's
        // rule 2 wants a smooth ramp before it blends; the ELF blends any
        // Scroll word with 0x04.
        let lines = stats(0.1, 0.5, 255);
        assert_eq!(
            verdict_with(0x415, &bf1("Scroll"), Some(&lines), false),
            (AlphaMode::Opaque, Rule::Modulation)
        );
        assert_eq!(
            verdict_with(0x415, &bf1("Scroll"), Some(&lines), true),
            (AlphaMode::Blend, Rule::Elf)
        );
        // Where the two agree the census's clause stays in the extras.
        let ramp = stats(0.6, 0.1, 255);
        assert_eq!(
            verdict_with(0x414, &bf1("Scroll"), Some(&ramp), true),
            (AlphaMode::Blend, Rule::Modulation)
        );
        // Not ruled here: the leaves' 0x07 MASK, `kam_crate`'s 0x05 body,
        // Scroll without 0x04, Specular, and a segment with no texture.
        let leaves = stats(0.2, 0.4, 255);
        assert_eq!(
            verdict_with(0x07, &normal, Some(&leaves), true),
            (AlphaMode::Mask, Rule::Mask)
        );
        assert_eq!(
            verdict_with(0x05, &normal, Some(&stats(0.55, 0.05, 255)), true),
            (AlphaMode::Opaque, Rule::Mask)
        );
        assert_eq!(
            verdict_with(0x410, &bf1("Scroll"), Some(&lines), true),
            (AlphaMode::Opaque, Rule::Modulation)
        );
        assert_eq!(
            verdict_with(0x435, &bf1("Specmap"), Some(&lines), true),
            verdict_with(0x435, &bf1("Specmap"), Some(&lines), false)
        );
        assert_eq!(
            verdict_with(0x1_0001, &RenderType::Bf2(0), Some(&thatch), true),
            (AlphaMode::Opaque, Rule::Elf)
        );
        assert_eq!(
            verdict_with(0x1, &normal, None, true),
            (AlphaMode::Opaque, Rule::None)
        );
    }

    /// Queue item 9: a word the disc alpha-tests is MASK whatever its `mod`
    /// bits. COR1's sky traffic `cor1_sky_cars` (BF2 0x1000002, a0 0.925,
    /// mid 0.025, the stats the shipped bake measures) and MUS1's lava fall
    /// `mus1_main_3` (0x1010212, a smooth ramp: mid 0.537) were rule 2's
    /// OPAQUE; BF2 tests both (`re-material-flags.md` §2, TEST 0x5000b).
    #[test]
    fn a_word_the_disc_tests_is_a_cutout_whatever_its_mod_bits() {
        let cars = stats(0.025, 0.925, 255);
        let lava = stats(0.537, 0.135, 255);
        assert_eq!(
            verdict(0x100_0002, &RenderType::Bf2(3), Some(&cars)),
            (AlphaMode::Mask, Rule::Mask)
        );
        for w in [0x101_0212, 0x1_0012, 0x12, 0x212, 0x100_0003] {
            assert_eq!(
                verdict(w, &RenderType::Bf2(0), Some(&lava)),
                (AlphaMode::Mask, Rule::Mask),
                "0x{w:x}"
            );
        }
        // BF1 tests 0x02 in its Normal class only: Specular (0x20), Scroll
        // (0x400) and Blur (0x800|0x4000) ignore it, and stay rule 2's.
        assert_eq!(
            verdict(0x12, &bf1("Normal"), Some(&lava)),
            (AlphaMode::Mask, Rule::Mask)
        );
        for w in [0x32, 0x412, 0x812, 0x4012] {
            assert_eq!(
                verdict(w, &bf1("Normal"), Some(&lava)),
                (AlphaMode::Opaque, Rule::Modulation),
                "0x{w:x}"
            );
        }
        // A word with 0x04 is translucent on the disc, not tested: the rule
        // decides it as before (a Scroll-class ramp composites).
        assert_eq!(
            verdict(0x416, &bf1("Scroll"), Some(&lava)),
            (AlphaMode::Blend, Rule::Modulation)
        );
        // Rule 1 still outranks the test: an alpha that never passes half.
        assert_eq!(
            verdict(0x100_0002, &RenderType::Bf2(3), Some(&stats(0.1, 0.9, 100))),
            (AlphaMode::Blend, Rule::NeverTests)
        );
    }

    #[test]
    fn bf2_translucent_shells_do_not_become_opaque_or_cutouts() {
        let bf2 = RenderType::Bf2(0);
        let bladder = stats(0.24868774, 0.017425537, 255);
        assert_eq!(
            census_verdict(0x85, &bf2, Some(&bladder)),
            (AlphaMode::Opaque, Rule::Modulation)
        );
        for flags in [0x4, 0x5, 0x85, 0x1_0085, 0x100_0005] {
            for texture in [bladder, stats(0.0, 0.9, 255), stats(1.0, 0.0, 255)] {
                assert_eq!(verdict(flags, &bf2, Some(&texture)).0, AlphaMode::Blend);
            }
        }
        assert_eq!(verdict(0x1_0098, &bf2, Some(&bladder)).0, AlphaMode::Opaque);
        assert_eq!(verdict(0x1_0001, &bf2, Some(&bladder)).0, AlphaMode::Opaque);
        assert_eq!(verdict(0x1_0007, &bf2, Some(&bladder)).0, AlphaMode::Mask);
        assert_eq!(
            verdict(0x85, &bf1("Normal"), Some(&bladder)).0,
            AlphaMode::Opaque
        );
        assert_eq!(verdict(0x85, &bf2, None), (AlphaMode::Opaque, Rule::None));
    }

    #[test]
    fn stats_are_fractions_of_mip_zero() {
        // four texels: 0 (a0), 128 (mid), 224 (neither), 255 (neither)
        let rgba = [0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 224, 0, 0, 0, 255];
        let s = AlphaStats::of_rgba8(&rgba);
        assert_eq!(s.a0, 0.25);
        assert_eq!(s.mid, 0.25);
        assert_eq!(s.amax, 255);
        // an empty buffer cannot say anything: fully opaque is the safe read
        let s = AlphaStats::of_rgba8(&[]);
        assert_eq!(s.amax, 255);
        assert_eq!((s.mid, s.a0), (0.0, 0.0));
    }

    /// D7: the two sky shells that made the carve-out necessary, as fixtures.
    #[test]
    fn the_sky_carve_out_keeps_the_pre_v2_alpha_state() {
        use crate::detail::ALPHA_IS_MEANINGFUL;

        // BF1 BES1 `sky_bespin1_clouds_2`, the `bes1_sky_dome_clouds_2` shell:
        // ONE draw, 0x04 Normal, mid 0.896 -> rule 3 says OPAQUE, so v2 alone
        // stripped the alpha and the translucent cloud sea became a solid
        // sheet (37 % of the golden). The old OR has 0x04 set, so D7 keeps it.
        assert_eq!(
            verdict(0x04, &bf1("Normal"), Some(&stats(0.896, 0.02, 255))).0,
            AlphaMode::Opaque,
            "the rule itself is unchanged: this draw is still OPAQUE"
        );
        assert!(
            keep_alpha(true, 0x04, false, false, false),
            "sky: the pre-v2 OR flags 0x04 alpha meaningful, so the alpha stays"
        );
        assert!(
            !keep_alpha(false, 0x04, false, false, false),
            "the same texture on a WORLD model would bake opaque under D3"
        );

        // BF2 MUS1 `mus1_sky_dome`, the BASE dome: a0 0.94, amax 173, segments
        // 0x10001 / 0x10010. Before v2 the OR marked neither 0x02 nor 0x04, so
        // it was force-opaque and painted the background; v2's rule 3 made
        // the 0x10001 segment MASK (class D) and the dome composited 94 %
        // nothing. Since queue R1U the rule itself draws that word OPAQUE, as
        // the disc does: no texel reaches the world's test.
        assert_eq!(
            verdict(0x1_0001, &RenderType::Bf2(0), Some(&stats(0.02, 0.94, 173))).0,
            AlphaMode::Opaque,
            "an unflagged word below the world's test is OPAQUE since R1U"
        );
        // Only the dome model draws it, so there is no world draw to override
        // the pre-v2 answer and the alpha goes.
        assert!(
            !keep_alpha(true, 0x1_0001 | 0x1_0010, false, false, false),
            "sky: the pre-v2 OR flags neither 0x02 nor 0x04, so it bakes opaque"
        );

        // A texture BOTH a sky model and a world model draw keeps its alpha
        // whenever the world draw needs it — the carve-out only ever ADDS.
        assert!(keep_alpha(true, 0x00, true, false, false));
        assert!(keep_alpha(true, ALPHA_IS_MEANINGFUL, false, false, false));
        // And a texture no material names took the old `unwrap_or` default.
        assert!(keep_alpha(true, ALPHA_IS_MEANINGFUL, false, true, false));
        // World textures that a segment names are D3 alone, whatever the old
        // flags said.
        assert!(!keep_alpha(false, ALPHA_IS_MEANINGFUL, false, false, false));
        assert!(keep_alpha(false, 0x00, true, false, false));
    }

    /// Queue item SM: a texture NO segment names keeps its disc alpha
    /// (`analysis/re-hud-icons.md` §2 — the PS2 composites every HUD bitmap
    /// by it), while a texture a segment names but no MASK/BLEND draw needs
    /// still bakes opaque, and the sky carve-out is untouched by the clause.
    /// Which bakes ASK the question is the caller's (`bake::map::Content`).
    #[test]
    fn a_texture_no_segment_names_keeps_its_alpha() {
        use crate::detail::ALPHA_IS_MEANINGFUL;
        // `squad_cmd_highlight`: no model in COMMON.LVL names it.
        assert!(keep_alpha(false, ALPHA_IS_MEANINGFUL, false, true, false));
        // Named by an OPAQUE draw only: D3 unchanged, still opaque.
        assert!(!keep_alpha(false, 0x00, false, false, false));
        // The sky answer never reads the clause.
        for unnamed in [false, true] {
            assert!(!keep_alpha(true, 0x1_0001, false, unnamed, false));
            assert!(keep_alpha(true, 0x04, false, unnamed, false));
        }
    }

    /// Queue item WA: the sheet THIS map's own `Effect("Water")` names keeps
    /// its disc alpha, because every blended water layer composites by it
    /// (`analysis/re-water-alpha.md` §0 [V][O]). The clause is NARROW on
    /// purpose — the lead's 2026-09-17 scope decision — so a map sheet the
    /// water config does NOT name stays exactly as design D3 left it.
    #[test]
    fn the_maps_own_water_sheet_keeps_its_alpha() {
        use crate::detail::ALPHA_IS_MEANINGFUL;
        // `yav_water` on BF1 YAV1: the map's `MainTexture`, named by no
        // segment, and 96 flat on the disc. It keeps its alpha now.
        assert!(keep_alpha(false, ALPHA_IS_MEANINGFUL, false, false, true));
        // The SAME sheet is baked into BF1 YAV2, which carries no
        // `Effect("Water")` at all — so there it is still forced opaque.
        // That contrast IS the narrow rule.
        assert!(!keep_alpha(false, ALPHA_IS_MEANINGFUL, false, false, false));
        // A map's terrain / foliage / mist sheets are unnamed too and the
        // water config does not name them: D3's answer, unchanged.
        assert!(!keep_alpha(false, 0x00, false, false, false));
        // The clause only ever ADDS: a draw that already needed the alpha
        // still gets it, and the sky path never reads it.
        assert!(keep_alpha(false, 0x00, true, false, false));
        for consumer_named in [false, true] {
            assert!(!keep_alpha(true, 0x1_0001, false, false, consumer_named));
            assert!(keep_alpha(true, 0x04, false, false, consumer_named));
        }
    }

    /// `AlphaUsage` has to answer the alpha question twice: once over every
    /// draw (D3) and once with the sky dome models left out (D7).
    #[test]
    fn usage_can_leave_the_sky_models_out() {
        use std::collections::HashSet;

        fn seg(tex: &str) -> Segment {
            Segment {
                vertex_count: 3,
                index_count: 3,
                bf2_material: false,
                pc_geometry: false,
                material_flags: 0,
                render_type: RenderType::Absent,
                spec_power: 0,
                spec_color: 0,
                uv_anim: [0, 0],
                textures: vec![(0, tex.to_owned())],
                bone: None,
                group: None,
                bmap: Vec::new(),
                bind: Vec::new(),
                wght: Vec::new(),
                positions: Vec::new(),
                normals: Vec::new(),
                uvs: Vec::new(),
                colors: None,
                triangles: vec![[0, 1, 2]],
            }
        }
        fn model(name: &str, tex: &str) -> Model {
            Model {
                name: name.to_owned(),
                bone_space: false,
                vertex_box: mungefs::model::Aabb {
                    min: [0.0; 3],
                    max: [1.0; 3],
                },
                vis_box: mungefs::model::Aabb {
                    min: [0.0; 3],
                    max: [1.0; 3],
                },
                face_count: 1,
                sphere: None,
                segments: vec![seg(tex)],
            }
        }
        fn masked(tex: &str) -> Vec<SegmentAlpha> {
            vec![SegmentAlpha {
                texture: tex.to_owned(),
                mode: AlphaMode::Mask,
                rule: Rule::Mask,
                flags: 0,
                rtyp: RenderType::Absent,
                stats: None,
            }]
        }

        let mut u = AlphaUsage::default();
        u.record_model(&model("SkyDome", "shared"), &masked("shared"));
        u.record_model(&model("SkyDome", "sky_only"), &masked("sky_only"));
        u.record_model(&model("Hut", "shared"), &masked("shared"));

        let sky: HashSet<String> = ["skydome".to_owned()].into_iter().collect();
        assert!(u.needs_alpha("shared") && u.needs_alpha("sky_only"));
        assert!(
            u.needs_alpha_outside("shared", &sky),
            "the hut's draw is outside the sky models"
        );
        assert!(
            !u.needs_alpha_outside("sky_only", &sky),
            "only the dome draws it, so D7 falls back to the pre-v2 flags"
        );
        assert_eq!(
            u.textures_of("skydome").collect::<Vec<_>>(),
            vec!["shared", "sky_only"]
        );
        let mut users: Vec<_> = u.models_using("shared").collect();
        users.sort_unstable();
        assert_eq!(users, vec!["hut", "skydome"]);

        assert!(!u.blended("shared"));
        assert!(!u.blended("sky_only"));
        let mut blend = masked("shared");
        blend[0].mode = AlphaMode::Blend;
        u.record_model(&model("Glass", "shared"), &blend);
        assert!(u.blended("shared"));
        assert!(!u.blended("sky_only"));
    }

    #[test]
    fn modes_spell_themselves_the_gltf_way() {
        assert_eq!(AlphaMode::Opaque.as_str(), "OPAQUE");
        assert_eq!(AlphaMode::Mask.as_str(), "MASK");
        assert_eq!(AlphaMode::Blend.as_str(), "BLEND");
        assert_eq!(Rule::NeverTests.as_str(), "1");
        assert_eq!(Rule::Modulation.as_str(), "2");
        assert_eq!(Rule::Mask.as_str(), "3");
        assert_eq!(Rule::None.as_str(), "none");
        assert_eq!(Rule::Elf.as_str(), "elf");
    }
}
