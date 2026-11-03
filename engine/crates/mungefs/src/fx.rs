//! `fx__` chunk, `Effect("Water")`, `Effect("HDR")`, `Effect("Lightning")`,
//! `Effect("Precipitation")` and `Effect("Wind")`: how a map's water surface
//! is painted, tiled and scrolled, whether BF2 draws the map's self-lit
//! segments glowing and blooms them ([`HdrConfig`]), and the map's weather.
//!
//! Every LVL carries several `fx__` chunks (3–9 on the twelve water maps,
//! measured) and each is a munged config tree ([`crate::config`]) of
//! `Effect("<name>")` records — `ColorControl`, `Wind`, `Precipitation`,
//! `Lightning`, `Water`, and more. **Only `Effect("Water")`,
//! `Effect("HDR")`, `Effect("Lightning")` and its `LightningBolt`,
//! `Effect("Precipitation")` and `Effect("Wind")` are decoded here**
//! ([`LightningConfig`], card KSKY: the effect that darkens the sky dome
//! between flashes).
//! `Effect("Precipitation")` and `Effect("Wind")` are [`PrecipitationConfig`]
//! and [`WindConfig`] (card WTHR, `analysis/re-weather.md`). Every other
//! effect is unparsed (`ColorControl` is read by neither ELF); this module
//! skips them without looking inside. The `fx__` chunks that hold
//! `ParticleEmitter` trees (impacts, explosions, trails) are
//! [`crate::particle`]'s.
//!
//! The key names are confirmed preimages from `analysis/re-water.md` §5 (`mod
//! key` below holds the 26 Water keys this module consumes),
//! which cracked them by brute force against the decomp's key switch
//! (`FUN_004763d0` 538852–539000, `FUN_00489c40` 550335–550410). Every hash
//! below is recomputed from its name by `hash_preimages_are_confirmed`, so a
//! typo in a constant cannot pass.
//!
//! ## What the PS2 reads out of it (`re-water.md` §3, [V])
//!
//! - **`MainTexture` paints the surface** — NOT `WATR -> LAYR`'s name, which
//!   both ELFs read into a dead stack buffer. This is why the port painted
//!   Kamino with `kam1_water`, its FOAM texture, instead of `kam1_water_2`.
//! - **Metres of world per texture repeat = `cell / (Tile · INFO5)`**, with
//!   `cell` and `INFO5` from `WATR -> INFO` ([`crate::water::WaterInfo`]).
//! - **Scroll = `Velocity / INFO5`** UV units per second. The ELF's texture
//!   matrix translates by the NEGATIVE of it (§3, 551310–551313); which sign
//!   a port that adds `scroll · t` to a world-XZ UV needs is the same
//!   question as the GS's UV handedness, which no `analysis/` document
//!   settles. The magnitude is what is recorded here — see
//!   [`WaterConfig::scroll_per_second`].
//!
//! ## The census (this module's own, all 12 water maps, 2026-09-09)
//!
//! `Effect("Water")` is present on exactly the twelve maps that ship a
//! `WATR`, and on no other map (BF1 TAT1/HOT1 and BF2 TAT2 checked and
//! carry none). Reproduces `re-water.md` §5 with one correction: **BF2 DAG1
//! authors `DisableLowRes` too** — the §5 table lists it only for KAS1 and
//! both YAV1s.
//!
//! ```text
//! map        Ocean  LowRes  Tile  Velocity     MainTexture     Min.a/Max.a
//! BF1 KAM1     1    far on   4,4  0.01,0.05    kam1_water_2      255/255
//! BF1 KAS1     0    DISABLE  4,4  0.02,0.02    kas1_water         80/80
//! BF1 KAS2     0    far on   2,2  0.05,0.05    kas2_water        188/188
//! BF1 NAB2     0    far on   2,2  0.05,0.05    nab2_water        128/128
//! BF1 YAV1     0    DISABLE  2,2  0.1,0.02     yav_water         255/255
//! BF2 KAM1     1    far on   4,4  0.01,0.05    kam1_water_2      255/255
//! BF2 KAS2     0    far on   1,1  0,0          kas2_water        255/255
//! BF2 NAB2     0    far on   2,2  0.05,0.05    nab2_water        128/128
//! BF2 YAV1     0    DISABLE  2,2  0.1,0.02     yav_water         255/255
//! BF2 DEA1     0    far on   2,2  0.001,0.001  dea1_water        100/200
//! BF2 FEL1     0    far on   2,2  0,0          fel1_water        188/188
//! BF2 DAG1     0    DISABLE  2,2  0,0          dag1_water        255/255
//! ```
//!
//! `SpecularEnable` is authored by **no map on either disc** — which is why
//! BF2 draws neither the speckle layer nor the far-water fog pass (§1). The
//! only map authoring `OscillationEnable` is BF2 FEL1, and it authors 0.
//!
//! ## Unauthored keys have no recorded default
//!
//! `re-water.md` names the 27 keys but not the ELF's defaults for the ones a
//! map leaves out, so every field here is an `Option` and absent means
//! absent. A consumer that needs a value when one is unauthored picks its own
//! and says so — `bake::water_glb` falls back to the pre-`fx__` rule and
//! prints when it does.
//!
//! Three colour keys (`0x3e68131d`, `0x399cb557`, `0x19fb4b6e` — the first is
//! per map, 148,170,192 on ten and 148,30,0 on BF2 DEA1) and `0xbf2b02c2(4)`
//! on both KAM1s are authored and consumed by NEITHER ELF (§5). They land in
//! [`WaterConfig::unknown`] with their values, unnamed.

use crate::config::{parse_tree, ConfigNode};
use crate::{Chunk, UcfbError};

/// Confirmed `fnv1a_lowercase` preimages (`re-water.md` §5; pinned by
/// `hash_preimages_are_confirmed`).
mod key {
    pub const EFFECT: u32 = 0x6e6e_8d54; // "Effect"
    pub const TILE: u32 = 0x713a_7cc9; // "Tile"
    pub const VELOCITY: u32 = 0x3274_1c32; // "Velocity"
    pub const PATCH_DIVISIONS: u32 = 0x0090_36eb; // "PatchDivisions"
    pub const LOD_DECIMATION: u32 = 0x807b_1583; // "LODDecimation"
    pub const MAIN_TEXTURE: u32 = 0x35a8_3d43; // "MainTexture"
    pub const FOAM_TEXTURE: u32 = 0x56e7_acb7; // "FoamTexture"
    pub const FOAM_TILE: u32 = 0xdcb9_1b88; // "FoamTile"
    pub const OCEAN_ENABLE: u32 = 0xa69c_431a; // "OceanEnable"
    pub const DISABLE_LOW_RES: u32 = 0xa01d_9243; // "DisableLowRes"
    pub const OSCILLATION_ENABLE: u32 = 0x12e3_9af3; // "OscillationEnable"
    pub const SPECULAR_ENABLE: u32 = 0xd4dd_ec5f; // "SpecularEnable"
    pub const PHILLIPS_CONSTANT: u32 = 0xd25e_f90a; // "PhillipsConstant"
    pub const WIND_SPEED: u32 = 0x1390_1a72; // "WindSpeed"
    pub const WIND_DIRECTION: u32 = 0x415e_2b30; // "WindDirection"
    pub const MIN_DIFFUSE_COLOR: u32 = 0xc2a2_6ee6; // "MinDiffuseColor"
    pub const MAX_DIFFUSE_COLOR: u32 = 0x7021_73b4; // "MaxDiffuseColor"
    pub const BORDER_DIFFUSE_COLOR: u32 = 0x44b4_8fc6; // "BorderDiffuseColor"
    pub const SPECULAR_COLOR: u32 = 0x01e4_aaf7; // "SpecularColor"
    pub const SPECKLE_SPECULAR_COLOR: u32 = 0xa6b2_74b0; // "SpeckleSpecularColor"
    pub const SPECKLE_AMBIENT_COLOR: u32 = 0xba5f_7683; // "SpeckleAmbientColor"
    pub const SPECKLE_TEXTURES: u32 = 0x2fa9_5ff4; // "SpeckleTextures"
    pub const SPECKLE_TILE: u32 = 0xdba6_9cc0; // "SpeckleTile"
    pub const SPECKLE_SCROLL_SPEED: u32 = 0xe9b9_1268; // "SpeckleScrollSpeed"
    pub const SPECKLE_COORD_SHIFT: u32 = 0x6ee8_0757; // "SpeckleCoordShift"
    pub const LIGHT_AZIM_AND_ELEV: u32 = 0x9a4f_3cc1; // "LightAzimAndElev"
                                                      // `Effect("HDR")`, B2 FUN_00455f30 (`analysis/re-glow.md` §2.1).
    pub const ENABLE: u32 = 0xaf8b_b8ce; // "Enable"
    pub const NUM_BLOOM_PASSES: u32 = 0xd4ef_5803; // "NumBloomPasses"
    pub const MAX_TOTAL_WEIGHT: u32 = 0xeafd_07ed; // "MaxTotalWeight"
    pub const GLOW_THRESHOLD: u32 = 0xb12c_9b1f; // "GlowThreshold"
                                                 // `Effect("Lightning")`, B2 FUN_0045aa30 (`analysis/re-kamino.md` §1).
    pub const COLOR: u32 = 0x3d7e_6258; // "Color"
    pub const SUNLIGHT_FADE_FACTOR: u32 = 0x15f3_deb6; // "SunlightFadeFactor"
    pub const SKY_DOME_DARKEN_FACTOR: u32 = 0x034a_ebfd; // "SkyDomeDarkenFactor"
    pub const BRIGHTNESS_MIN: u32 = 0xdb50_529e; // "BrightnessMin"
    pub const FADE_TIME: u32 = 0xa40f_930a; // "FadeTime"
    pub const TIME_BETWEEN_FLASHES: u32 = 0x260e_9234; // "TimeBetweenFlashesMinMax"
    pub const TIME_BETWEEN_SUB_FLASHES: u32 = 0xd71d_3c32; // "TimeBetweenSubFlashesMinMax"
    pub const NUM_SUB_FLASHES: u32 = 0x67f9_208d; // "NumSubFlashesMinMax"
    pub const HORIZON_ANGLE: u32 = 0xfc6b_14b1; // "HorizonAngleMinMax"
    pub const SOUND_CRACK: u32 = 0xe46b_f7a2; // "SoundCrack"
    pub const SOUND_SUB_CRACK: u32 = 0x882e_50b8; // "SoundSubCrack"
                                                  // `LightningBolt("<name>")`, a top-level record beside the effects.
    pub const LIGHTNING_BOLT: u32 = 0x1f67_8786; // "LightningBolt"
    pub const TEXTURE: u32 = 0x3c64_68f4; // "Texture"
    pub const WIDTH: u32 = 0x9587_6e1f; // "Width"
    pub const BREAK_DISTANCE: u32 = 0x8f81_3993; // "BreakDistance"
    pub const TEXTURE_SIZE: u32 = 0x3098_78ad; // "TextureSize"
    pub const SPREAD_FACTOR: u32 = 0xdbe2_76e9; // "SpreadFactor"
    pub const MAX_BRANCHES: u32 = 0xfd12_7d8d; // "MaxBranches"
    pub const BRANCH_FACTOR: u32 = 0xcec9_b326; // "BranchFactor"
    pub const BRANCH_SPREAD_FACTOR: u32 = 0x4352_3a69; // "BranchSpreadFactor"
    pub const BRANCH_LENGTH: u32 = 0xf609_f595; // "BranchLength"
    pub const INTERPOLATION_SPEED: u32 = 0x9b2b_b6e8; // "InterpolationSpeed"
    pub const NUM_CHILDREN: u32 = 0xd80b_db3c; // "NumChildren"
    pub const CHILD_BREAK_DISTANCE: u32 = 0xc37b_68ab; // "ChildBreakDistance"
    pub const CHILD_TEXTURE_SIZE: u32 = 0x17e4_0be5; // "ChildTextureSize"
    pub const CHILD_WIDTH: u32 = 0x30bb_2f27; // "ChildWidth"
    pub const CHILD_SPREAD_FACTOR: u32 = 0x8366_bef1; // "ChildSpreadFactor"
    pub const CHILD_COLOR: u32 = 0x3327_1ff0; // "ChildColor"
                                              // `Effect("Precipitation")`, B2 FUN_00462c70, and `Effect("Wind")`, B2
                                              // FUN_00461ee0 (`analysis/re-weather.md` §2.1, §6).
    pub const TYPE: u32 = 0x5127_f14d; // "Type"
    pub const STREAKS: u32 = 0x4a59_a280; // "Streaks", the one `Type` value the parser tests
    pub const PARTICLE_SIZE: u32 = 0x55e4_1b16; // "ParticleSize"
    pub const RANGE: u32 = 0xfadc_0cd2; // "Range"
    pub const ALPHA_MIN_MAX: u32 = 0x220c_0c21; // "AlphaMinMax"
    pub const PARTICLE_DENSITY: u32 = 0xc393_fff1; // "ParticleDensity"
    pub const PARTICLE_DENSITY_RANGE: u32 = 0x2214_ef7e; // "ParticleDensityRange"
    pub const VELOCITY_RANGE: u32 = 0x5469_303f; // "VelocityRange"
    pub const STREAK_LENGTH: u32 = 0xb16a_1adb; // "StreakLength"
    pub const CAMERA_CROSS_VELOCITY_SCALE: u32 = 0x8f36_9c79; // "CameraCrossVelocityScale"
    pub const CAMERA_AXIAL_VELOCITY_SCALE: u32 = 0x1c45_64ca; // "CameraAxialVelocityScale"
    pub const ROTATION: u32 = 0x21ac_415f; // "Rotation"
    pub const ROTATION_RANGE: u32 = 0xf261_7b74; // "RotationRange"
    pub const GROUND_EFFECT: u32 = 0x5168_9473; // "GroundEffect"
    pub const GROUND_EFFECTS_PER_SEC: u32 = 0x885a_c868; // "GroundEffectsPerSec"
    pub const GROUND_EFFECT_SPREAD: u32 = 0xd23d_97a4; // "GroundEffectSpread"
    pub const VELOCITY_CHANGE_RATE: u32 = 0xecec_52a4; // "VelocityChangeRate"
}

/// Largest `SpeckleTextures(prefix, count, fps)` run expanded. The discs
/// author 25; `count` is an authored f32 that sizes an allocation below.
const MAX_SPECKLE_FRAMES: usize = 4096;

/// The additive animated highlight layer (`re-water.md` §3). The bake carries
/// it into the manifest (`bake::water_glb`) and the viewer draws it
/// (`game::viewer::look::speckle_look`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Speckle {
    /// Frame texture names in order. `SpeckleTextures` comes in two authored
    /// forms and both land here: `("water_specularmask_", 25, 4)` expands to
    /// `water_specularmask_0 .. _24` (the names the bake writes — measured
    /// against `bf1_kas2/manifest.ron`, 25 of them, decimal and unpadded),
    /// and BF1/BF2 KAM1's explicit list `("water_specles0", .., "3")` is
    /// taken as it stands.
    pub textures: Vec<String>,
    /// Frames per second, from the third argument of the prefix form. `None`
    /// on the list form — both KAM1s author no rate.
    pub fps: Option<f32>,
    pub tile: Option<[f32; 2]>,
    pub scroll_speed: Option<[f32; 2]>,
    pub coord_shift: Option<[f32; 2]>,
}

/// `fx__ -> Effect("Water")`. Every field is exactly what the map authored;
/// `None` is "the map authored no such key" and never a substituted default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaterConfig {
    /// Xbox/PC material: normals, never a diffuse image. Same prefix/count/rate
    /// encoding as SpeckleTextures; absent on PS2.
    pub normal_maps: Option<Speckle>,
    pub refraction_color: Option<[f32; 4]>,
    pub reflection_color: Option<[f32; 4]>,
    pub fresnel_min_max: Option<[f32; 2]>,
    /// The texture that paints the surface (§3). Authored on all 12.
    pub main_texture: Option<String>,
    /// Blended over the base on an `OceanEnable` map (§3). Both KAM1s only.
    pub foam_texture: Option<String>,
    /// Texture repeats per `WATR` cell, before the `INFO5` multiplier —
    /// see [`WaterConfig::metres_per_repeat`].
    pub tile: Option<[f32; 2]>,
    pub foam_tile: Option<[f32; 2]>,
    /// UV units per second BEFORE the `INFO5` divide — see
    /// [`WaterConfig::scroll_per_second`].
    pub velocity: Option<[f32; 2]>,
    /// Mesh subdivisions per cell on the PS2. The port bakes two triangles
    /// per flat cell instead; this sizes only the KAM1 ocean's runtime grid
    /// (`game::water_anim`).
    pub patch_divisions: Option<[f32; 2]>,
    /// Divides `patch_divisions` for the far pass. Recorded, not consumed.
    pub lod_decimation: Option<f32>,
    /// 1 on both KAM1s, 0 on the other ten. An ocean map draws a Tessendorf
    /// FFT surface (`analysis/re-water.md` §3–§3b), which the port animates
    /// in `game::water_anim`.
    pub ocean_enable: Option<f32>,
    /// Authored with NO arguments — presence is the value. True on BF1 KAS1,
    /// both YAV1s and BF2 DAG1: those maps draw no far water at all (§1).
    pub disable_low_res: bool,
    /// BF2 only, and only BF2 FEL1 authors it (as 0).
    pub oscillation_enable: Option<f32>,
    /// BF2 only. **No map on either disc authors it**, which is what makes
    /// BF2's far water unfogged and its speckle layer absent (§1).
    pub specular_enable: Option<f32>,
    /// Ocean-only: the Phillips spectrum's `A` (1e-5 on both KAM1s).
    pub phillips_constant: Option<f32>,
    pub wind_speed: Option<f32>,
    pub wind_direction: Option<[f32; 2]>,
    /// 0..255 RGBA. The PS2 builds a 16-entry per-vertex diffuse LUT
    /// Border → Min → Max and takes vertex alpha from a Min.a → Max.a table
    /// (§3); the port's tint and alpha are built from them.
    pub min_diffuse_color: Option<[f32; 4]>,
    pub max_diffuse_color: Option<[f32; 4]>,
    pub border_diffuse_color: Option<[f32; 4]>,
    pub specular_color: Option<[f32; 4]>,
    pub speckle_specular_color: Option<[f32; 4]>,
    pub speckle_ambient_color: Option<[f32; 4]>,
    pub speckle: Option<Speckle>,
    /// `(azimuth, elevation)` of the specular light. `(1, 0)` on all 12.
    pub light_azim_and_elev: Option<[f32; 2]>,
    /// Keys inside the `Water` scope whose names are not confirmed, with the
    /// values as stored — the three unconsumed colour keys and both KAM1s'
    /// `0xbf2b02c2(4)`. Never guessed at.
    pub unknown: Vec<(u32, Vec<f32>)>,
}

impl WaterConfig {
    /// Parse one `fx__` chunk. `Ok(None)` when it carries no
    /// `Effect("Water")` — which is every `fx__` on the 22 dry maps and most
    /// of them on the twelve wet ones, so a caller scans the chunks and
    /// keeps the first `Some`.
    pub fn parse(fx: &Chunk<'_>) -> Result<Option<WaterConfig>, UcfbError> {
        Ok(effect_named(fx, "water")?.map(|node| from_scope(&node)))
    }

    /// Metres of world per texture repeat, given the `WATR -> INFO` cell size
    /// and `INFO5` (`re-water.md` §3, [V]): `cell / (Tile · INFO5)`.
    /// `None` when the map authors no `Tile` — no disc map does that, so the
    /// caller's fallback is for data we have never seen.
    ///
    /// All twelve author a matched `Tile` pair, so the two axes are equal on
    /// every shipped map; they are computed separately anyway because the key
    /// stores two numbers.
    pub fn metres_per_repeat(&self, cell_size: f32, info5: u32) -> Option<[f32; 2]> {
        let tile = self.tile?;
        let n = info5.max(1) as f32;
        let per = |t: f32| {
            let d = t * n;
            (d.is_finite() && d > 0.0).then(|| cell_size / d)
        };
        Some([per(tile[0])?, per(tile[1])?])
    }

    /// UV units per second: `Velocity / INFO5` (`re-water.md` §3).
    ///
    /// **The sign is PROVISIONAL.** §3 reads the ELF's texture matrix as
    /// translating by `−Velocity/INFO5 · t`; the port adds `scroll · t` to a
    /// UV built from world XZ (`shader.wgsl`'s `uv + material.scroll.xy * globals.time`), and whether that composes
    /// to the same apparent flow direction depends on the GS's UV handedness,
    /// which no `analysis/` document settles. The magnitude is [V]; this
    /// returns it unnegated.
    pub fn scroll_per_second(&self, info5: u32) -> Option<[f32; 2]> {
        let v = self.velocity?;
        let n = info5.max(1) as f32;
        Some([v[0] / n, v[1] / n])
    }
}

/// BF2's `fx__ -> Effect("HDR")` (`analysis/re-glow.md` §2, [O]): when
/// enabled, the PS2 draws the scene at half scale and doubles it back, so an
/// MTRL 0x10 segment glows ×(1 + As/128) and 0x14 lands ×2 (§1), and pixels
/// past `2 · glow_threshold` of display white bloom (§2.2). BF1 has no such
/// effect (§5). The fields the pass never reads (`DownSizeFactor`,
/// `GlowFactor`, §2.2) are not kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrConfig {
    /// `Enable(1)` — 13 of the port's 17 BF2 maps (§2.1).
    pub enable: bool,
    /// Blur iterations, each two five-tap sub-passes. ELF default 5.
    pub num_bloom_passes: u32,
    /// Sets each tap's weight, `int(int(w · 127) · 0.2) / 128`. Default 1.2.
    pub max_total_weight: f32,
    /// The bloom's luma cut in the half-scale frame. Default 0.5.
    pub glow_threshold: f32,
}

impl Default for HdrConfig {
    /// The ctor's values, B2 FUN_00455da8 (516640–516645): off, 5 passes,
    /// weight 1.2, threshold 0.5.
    fn default() -> Self {
        HdrConfig {
            enable: false,
            num_bloom_passes: 5,
            max_total_weight: 1.2,
            glow_threshold: 0.5,
        }
    }
}

impl HdrConfig {
    /// Parse one `fx__` chunk: `Ok(None)` when it carries no
    /// `Effect("HDR")` (the name matched case-insensitively — the discs
    /// spell it both `HDR` and `hdr`). Unauthored keys keep the ctor's
    /// defaults; a key authored twice takes the later value, as the parser
    /// does (DEA1 and TAN1 author `MaxTotalWeight` twice).
    pub fn parse(fx: &Chunk<'_>) -> Result<Option<HdrConfig>, UcfbError> {
        let Some(node) = effect_named(fx, "hdr")? else {
            return Ok(None);
        };
        let mut h = HdrConfig::default();
        for c in &node.children {
            match (c.hash, c.f32(0)) {
                (key::ENABLE, Some(v)) => h.enable = v != 0.0,
                (key::NUM_BLOOM_PASSES, Some(v)) if v.is_finite() && v >= 0.0 => {
                    h.num_bloom_passes = (v as u32).min(16)
                }
                (key::MAX_TOTAL_WEIGHT, Some(v)) if v.is_finite() => h.max_total_weight = v,
                (key::GLOW_THRESHOLD, Some(v)) if v.is_finite() => h.glow_threshold = v,
                _ => {}
            }
        }
        Ok(Some(h))
    }
}

/// `fx__ -> Effect("Lightning")` (`analysis/re-kamino.md` §1, [O]): BF1 KAM1,
/// BF1 YAV2 and BF2 KAM1 author it, no other map. While enabled the PS2
/// draws sky dome shell 0 at white · (1 − [`Self::sky_dome_darken_factor`])
/// between flashes ([`Self::resting_dome_colour`]) and lifts it, the fog
/// colour and the lights toward [`Self::color`] during one.
///
/// A key a map leaves out keeps the ctor's value (B2 `FUN_0045a7d0`,
/// 519747–519758 — [`Default`]); the three maps author every key. Only the
/// dome factor is consumed today (card KSKY); the flash fields are carried
/// for the lightning card (WTHL; re-weather.md).
#[derive(Debug, Clone, PartialEq)]
pub struct LightningConfig {
    /// `Enable(1)` registers the effect's tick; without it nothing is darkened.
    pub enable: bool,
    /// `Color`, 0..255 as authored: what a flash lifts the lights toward.
    pub color: [f32; 3],
    /// How far a flash fades the sun (L+0x34).
    pub sunlight_fade_factor: f32,
    /// How dark shell 0 is between flashes: its colour is white·(1 − f) (L+0x38).
    pub sky_dome_darken_factor: f32,
    /// A flash's least brightness (L+0x3c).
    pub brightness_min: f32,
    /// Seconds a flash takes to fade (L+0x40).
    pub fade_time: f32,
    /// Seconds between flashes, min and max (L+0x44, +0x48).
    pub time_between_flashes: [f32; 2],
    /// Seconds between one flash's sub-flashes (L+0x4c, +0x50).
    pub time_between_sub_flashes: [f32; 2],
    /// Sub-flashes per flash (L+0x54, +0x58).
    pub num_sub_flashes: [f32; 2],
    /// A bolt's elevation above the horizon, DEGREES as authored (the ctor
    /// stores radians, L+0x5c, +0x60).
    pub horizon_angle: [f32; 2],
    pub sound_crack: Option<String>,
    pub sound_sub_crack: Option<String>,
}

impl Default for LightningConfig {
    /// The ctor's values, B2 `FUN_0045a7d0`: off, colour (239, 248, 255)/255,
    /// sun fade 0.1, dome 0.4, brightness 0.9, fade 0.2 s, flashes 3..8 s,
    /// sub-flashes 0.08..0.16 s, 2..5 of them, 30..60 degrees.
    fn default() -> Self {
        LightningConfig {
            enable: false,
            color: [239.0, 248.0, 255.0],
            sunlight_fade_factor: 0.1,
            sky_dome_darken_factor: 0.4,
            brightness_min: 0.9,
            fade_time: 0.2,
            time_between_flashes: [3.0, 8.0],
            time_between_sub_flashes: [0.08, 0.16],
            num_sub_flashes: [2.0, 5.0],
            horizon_angle: [30.0, 60.0],
            sound_crack: None,
            sound_sub_crack: None,
        }
    }
}

impl LightningConfig {
    /// Parse one `fx__` chunk: `Ok(None)` when it carries no
    /// `Effect("Lightning")` (matched case-insensitively, like the others).
    pub fn parse(fx: &Chunk<'_>) -> Result<Option<LightningConfig>, UcfbError> {
        let Some(node) = effect_named(fx, "lightning")? else {
            return Ok(None);
        };
        let mut l = LightningConfig::default();
        for c in &node.children {
            let finite = c.f32(0).filter(|v| v.is_finite());
            match c.hash {
                key::ENABLE => l.enable = c.f32(0).is_some_and(|v| v != 0.0),
                // Clamped to a byte and truncated, as the parser stores it
                // (519878–519900).
                key::COLOR => {
                    l.color = c
                        .f3()
                        .map_or(l.color, |v| v.map(|x| x.clamp(0.0, 255.0).trunc()))
                }
                key::SUNLIGHT_FADE_FACTOR => {
                    l.sunlight_fade_factor = finite.unwrap_or(l.sunlight_fade_factor)
                }
                key::SKY_DOME_DARKEN_FACTOR => {
                    l.sky_dome_darken_factor = finite.unwrap_or(l.sky_dome_darken_factor)
                }
                key::BRIGHTNESS_MIN => l.brightness_min = finite.unwrap_or(l.brightness_min),
                key::FADE_TIME => l.fade_time = finite.unwrap_or(l.fade_time),
                key::TIME_BETWEEN_FLASHES => l.time_between_flashes = c.floats_or_zero(),
                key::TIME_BETWEEN_SUB_FLASHES => l.time_between_sub_flashes = c.floats_or_zero(),
                key::NUM_SUB_FLASHES => l.num_sub_flashes = c.floats_or_zero(),
                key::HORIZON_ANGLE => l.horizon_angle = c.floats_or_zero(),
                key::SOUND_CRACK => l.sound_crack = non_empty(c.string.clone()),
                key::SOUND_SUB_CRACK => l.sound_sub_crack = non_empty(c.string.clone()),
                _ => {}
            }
        }
        Ok(Some(l))
    }

    /// The colour byte the PS2 holds for dome shell 0 between flashes:
    /// white·(1 − f) + black·f, truncated as the tick stores it (B2
    /// `FUN_0045ae80` 520152–520170) — 153 for the discs' 0.4, which is what
    /// PS2 RAM holds (`re-kamino.md` §1). `None` when the effect is off: the
    /// shell keeps the constant white.
    pub fn resting_dome_colour(&self) -> Option<u8> {
        if !self.enable {
            return None;
        }
        let f = self.sky_dome_darken_factor.clamp(0.0, 1.0);
        Some((255.0 * (1.0 - f)) as u8)
    }
}

/// A top-level `LightningBolt("<name>")` record: the bolt the Lightning
/// effect throws (texture, branching, colours), as authored — both KAM1s
/// author one `skybolt`, BF1 YAV2 none. Parsed and carried, not drawn
/// (card WTHL); a key the map leaves out is `None`, the ctor's defaults for
/// these are not read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LightningBoltConfig {
    pub name: String,
    pub texture: Option<String>,
    pub width: Option<f32>,
    pub fade_time: Option<f32>,
    pub break_distance: Option<f32>,
    pub texture_size: Option<f32>,
    pub spread_factor: Option<f32>,
    pub max_branches: Option<f32>,
    pub branch_factor: Option<f32>,
    pub branch_spread_factor: Option<f32>,
    pub branch_length: Option<f32>,
    pub interpolation_speed: Option<f32>,
    pub num_children: Option<f32>,
    pub child_break_distance: Option<f32>,
    pub child_texture_size: Option<f32>,
    pub child_width: Option<f32>,
    pub child_spread_factor: Option<f32>,
    /// 0..255 RGBA as authored.
    pub color: Option<[f32; 4]>,
    pub child_color: Option<[f32; 4]>,
}

impl LightningBoltConfig {
    /// Every `LightningBolt` of one `fx__` chunk, in order.
    pub fn parse_all(fx: &Chunk<'_>) -> Result<Vec<LightningBoltConfig>, UcfbError> {
        let mut out = Vec::new();
        for node in parse_tree(fx)? {
            if node.hash != key::LIGHTNING_BOLT {
                continue;
            }
            let mut b = LightningBoltConfig {
                name: node.string.clone().unwrap_or_default(),
                ..Default::default()
            };
            for c in &node.children {
                let v = c.f32(0);
                match c.hash {
                    key::TEXTURE => b.texture = non_empty(c.string.clone()),
                    key::WIDTH => b.width = v,
                    key::FADE_TIME => b.fade_time = v,
                    key::BREAK_DISTANCE => b.break_distance = v,
                    key::TEXTURE_SIZE => b.texture_size = v,
                    key::SPREAD_FACTOR => b.spread_factor = v,
                    key::MAX_BRANCHES => b.max_branches = v,
                    key::BRANCH_FACTOR => b.branch_factor = v,
                    key::BRANCH_SPREAD_FACTOR => b.branch_spread_factor = v,
                    key::BRANCH_LENGTH => b.branch_length = v,
                    key::INTERPOLATION_SPEED => b.interpolation_speed = v,
                    key::NUM_CHILDREN => b.num_children = v,
                    key::CHILD_BREAK_DISTANCE => b.child_break_distance = v,
                    key::CHILD_TEXTURE_SIZE => b.child_texture_size = v,
                    key::CHILD_WIDTH => b.child_width = v,
                    key::CHILD_SPREAD_FACTOR => b.child_spread_factor = v,
                    key::COLOR => b.color = c.f4(),
                    key::CHILD_COLOR => b.child_color = c.f4(),
                    _ => {}
                }
            }
            out.push(b);
        }
        Ok(out)
    }
}

/// The first `Effect("<name>")` record of one `fx__` chunk, matched
/// case-insensitively like the others.
fn effect_named(fx: &Chunk<'_>, name: &str) -> Result<Option<ConfigNode>, UcfbError> {
    Ok(parse_tree(fx)?.into_iter().find(|n| {
        n.hash == key::EFFECT
            && n.string
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(name))
    }))
}

/// `fx__ -> Effect("Precipitation")` (`analysis/re-weather.md` §1–§2, [O]):
/// rain on both Kaminos and BF1 Yavin 2, snow on the Hoths and BF1 Rhen Var,
/// ash on BF2 Mygeeto, embers on BF2 Mustafar, pollen on BF2 Felucia. A key
/// a map leaves out keeps the ctor's value (B2 `FUN_00462928` — [`Default`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PrecipitationConfig {
    pub enable: bool,
    /// `Type("Streaks")`: a particle is a line; any other value a textured
    /// square ("Quads").
    pub streaks: bool,
    /// `Texture`, the flake's sheet (Quads only).
    pub texture: Option<String>,
    pub particle_size: f32,
    /// How far along the view axis the cells are drawn, metres.
    pub range: f32,
    /// 0..255 RGBA; alpha 255 when the map gives three numbers.
    pub color: [u8; 4],
    pub alpha_min_max: [f32; 2],
    /// The particle COUNT of the one tiled cube, and its (unused: 0 on every
    /// map) range.
    pub particle_density: f32,
    pub particle_density_range: f32,
    /// Fall speed, m/s, down positive.
    pub velocity: f32,
    pub velocity_range: f32,
    /// As authored; the PS2 stores it / 30: seconds of travel.
    pub streak_length: f32,
    pub camera_cross_velocity_scale: f32,
    pub camera_axial_velocity_scale: f32,
    pub rotation: f32,
    pub rotation_range: f32,
    /// The splash's particle-effect class.
    pub ground_effect: Option<String>,
    pub ground_effects_per_sec: f32,
    pub ground_effect_spread: f32,
}

impl Default for PrecipitationConfig {
    /// The ctor's values (re-weather.md §2.1's default column).
    fn default() -> Self {
        PrecipitationConfig {
            enable: false,
            streaks: true,
            texture: None,
            particle_size: 0.02,
            range: 10.0,
            color: [192, 192, 192, 160],
            alpha_min_max: [0.5, 0.5],
            particle_density: 30.0,
            particle_density_range: 0.0,
            velocity: 10.0,
            velocity_range: 1.0,
            streak_length: 1.0,
            camera_cross_velocity_scale: 1.0,
            camera_axial_velocity_scale: 1.0,
            rotation: 0.0,
            rotation_range: 2.0,
            ground_effect: None,
            ground_effects_per_sec: 10.0,
            ground_effect_spread: 20.0,
        }
    }
}

impl PrecipitationConfig {
    /// Parse one `fx__` chunk: `Ok(None)` when it carries no
    /// `Effect("Precipitation")`.
    pub fn parse(fx: &Chunk<'_>) -> Result<Option<PrecipitationConfig>, UcfbError> {
        let Some(node) = effect_named(fx, "precipitation")? else {
            return Ok(None);
        };
        let mut p = PrecipitationConfig::default();
        for c in &node.children {
            let v = c.f32(0).filter(|v| v.is_finite());
            let set = |slot: &mut f32| *slot = v.unwrap_or(*slot);
            match c.hash {
                key::ENABLE => p.enable = c.f32(0).is_some_and(|v| v != 0.0),
                key::TYPE => {
                    p.streaks = c
                        .string
                        .as_deref()
                        .is_some_and(|s| crate::fnv1a_lowercase(s) == key::STREAKS)
                }
                key::TEXTURE => p.texture = non_empty(c.string.clone()),
                key::PARTICLE_SIZE => set(&mut p.particle_size),
                key::RANGE => set(&mut p.range),
                key::COLOR => {
                    let byte = |i: usize, d: u8| {
                        c.f32(i)
                            .filter(|v| v.is_finite())
                            .map_or(d, |v| v.clamp(0.0, 255.0) as u8)
                    };
                    p.color = [
                        byte(0, p.color[0]),
                        byte(1, p.color[1]),
                        byte(2, p.color[2]),
                        byte(3, 255),
                    ];
                }
                // One argument sets both.
                key::ALPHA_MIN_MAX => {
                    if let Some(a) = v {
                        let b = c.f32(1).filter(|v| v.is_finite()).unwrap_or(a);
                        p.alpha_min_max = [a, b];
                    }
                }
                key::PARTICLE_DENSITY => set(&mut p.particle_density),
                key::PARTICLE_DENSITY_RANGE => set(&mut p.particle_density_range),
                key::VELOCITY => set(&mut p.velocity),
                key::VELOCITY_RANGE => set(&mut p.velocity_range),
                key::STREAK_LENGTH => set(&mut p.streak_length),
                key::CAMERA_CROSS_VELOCITY_SCALE => set(&mut p.camera_cross_velocity_scale),
                key::CAMERA_AXIAL_VELOCITY_SCALE => set(&mut p.camera_axial_velocity_scale),
                key::ROTATION => set(&mut p.rotation),
                key::ROTATION_RANGE => set(&mut p.rotation_range),
                key::GROUND_EFFECT => p.ground_effect = non_empty(c.string.clone()),
                key::GROUND_EFFECTS_PER_SEC => set(&mut p.ground_effects_per_sec),
                key::GROUND_EFFECT_SPREAD => set(&mut p.ground_effect_spread),
                _ => {}
            }
        }
        Ok(Some(p))
    }
}

/// `fx__ -> Effect("Wind")` (re-weather.md §6, [O]): the wandering vector
/// the precipitation falls along — both Kaminos and both Hoths enable one.
/// Fields a map leaves out are 0 (the four maps author all three).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindConfig {
    pub enable: bool,
    /// `Velocity(a, b)`: the base vector is (a, 0, b), m/s.
    pub velocity: [f32; 2],
    pub velocity_range: f32,
    pub velocity_change_rate: f32,
}

impl WindConfig {
    /// Parse one `fx__` chunk: `Ok(None)` when it carries no `Effect("Wind")`.
    pub fn parse(fx: &Chunk<'_>) -> Result<Option<WindConfig>, UcfbError> {
        let Some(node) = effect_named(fx, "wind")? else {
            return Ok(None);
        };
        let mut w = WindConfig::default();
        for c in &node.children {
            let v = c.f32(0).filter(|v| v.is_finite());
            match c.hash {
                key::ENABLE => w.enable = c.f32(0).is_some_and(|v| v != 0.0),
                key::VELOCITY => w.velocity = c.floats_or_zero(),
                key::VELOCITY_RANGE => w.velocity_range = v.unwrap_or(w.velocity_range),
                key::VELOCITY_CHANGE_RATE => {
                    w.velocity_change_rate = v.unwrap_or(w.velocity_change_rate)
                }
                _ => {}
            }
        }
        Ok(Some(w))
    }
}

fn from_scope(effect: &ConfigNode) -> WaterConfig {
    let mut w = WaterConfig::default();
    let mut sp = Speckle::default();
    let mut any_speckle = false;
    for c in &effect.children {
        match c.hash {
            0xeac5_87ac => {
                let (textures, fps) = speckle_frames(c);
                w.normal_maps = Some(Speckle {
                    textures,
                    fps,
                    ..Default::default()
                });
            }
            0x5aa5_a3bd => w.refraction_color = c.f4(),
            0x8a81_de8b => w.reflection_color = c.f4(),
            0x8f7c_fa82 => w.fresnel_min_max = Some(c.floats_or_zero()),
            key::MAIN_TEXTURE => w.main_texture = non_empty(c.string.clone()),
            key::FOAM_TEXTURE => w.foam_texture = non_empty(c.string.clone()),
            key::TILE => w.tile = Some(c.floats_or_zero()),
            key::FOAM_TILE => w.foam_tile = Some(c.floats_or_zero()),
            key::VELOCITY => w.velocity = Some(c.floats_or_zero()),
            key::PATCH_DIVISIONS => w.patch_divisions = Some(c.floats_or_zero()),
            key::LOD_DECIMATION => w.lod_decimation = c.f32(0),
            key::OCEAN_ENABLE => w.ocean_enable = c.f32(0),
            // Authored with an empty argument list: it is a flag, and the
            // value is whether the key is there at all.
            key::DISABLE_LOW_RES => w.disable_low_res = true,
            key::OSCILLATION_ENABLE => w.oscillation_enable = c.f32(0),
            key::SPECULAR_ENABLE => w.specular_enable = c.f32(0),
            key::PHILLIPS_CONSTANT => w.phillips_constant = c.f32(0),
            key::WIND_SPEED => w.wind_speed = c.f32(0),
            key::WIND_DIRECTION => w.wind_direction = Some(c.floats_or_zero()),
            key::MIN_DIFFUSE_COLOR => w.min_diffuse_color = c.f4(),
            key::MAX_DIFFUSE_COLOR => w.max_diffuse_color = c.f4(),
            key::BORDER_DIFFUSE_COLOR => w.border_diffuse_color = c.f4(),
            key::SPECULAR_COLOR => w.specular_color = c.f4(),
            key::SPECKLE_SPECULAR_COLOR => w.speckle_specular_color = c.f4(),
            key::SPECKLE_AMBIENT_COLOR => w.speckle_ambient_color = c.f4(),
            key::LIGHT_AZIM_AND_ELEV => w.light_azim_and_elev = Some(c.floats_or_zero()),
            key::SPECKLE_TEXTURES => {
                let (names, fps) = speckle_frames(c);
                sp.textures = names;
                sp.fps = fps;
                any_speckle = true;
            }
            key::SPECKLE_TILE => {
                sp.tile = Some(c.floats_or_zero());
                any_speckle = true;
            }
            key::SPECKLE_SCROLL_SPEED => {
                sp.scroll_speed = Some(c.floats_or_zero());
                any_speckle = true;
            }
            key::SPECKLE_COORD_SHIFT => {
                sp.coord_shift = Some(c.floats_or_zero());
                any_speckle = true;
            }
            h => w.unknown.push((h, c.floats())),
        }
    }
    if any_speckle {
        w.speckle = Some(sp);
    }
    w
}

/// `SpeckleTextures` in either authored form (see [`Speckle::textures`]).
fn speckle_frames(c: &ConfigNode) -> (Vec<String>, Option<f32>) {
    let Some(first) = c.str_arg(0) else {
        return (Vec::new(), None);
    };
    // The list form: every argument is a string.
    if c.str_arg(1).is_some() {
        let names = (0..c.values.len())
            .filter_map(|i| c.str_arg(i).map(str::to_owned))
            .collect();
        return (names, None);
    }
    // The prefix form: (prefix, count, fps). A count the authoring never
    // produces is clamped rather than allocated.
    let count = c.f32(1).unwrap_or(0.0);
    let n = if count.is_finite() && count > 0.0 {
        (count as usize).min(MAX_SPECKLE_FRAMES)
    } else {
        0
    };
    ((0..n).map(|i| format!("{first}{i}")).collect(), c.f32(2))
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_chunk, fnv1a_lowercase};

    /// Every constant in `key` is the hash of the name in its comment, so a
    /// mistyped digit cannot silently stop matching a disc key.
    #[test]
    fn hash_preimages_are_confirmed() {
        for (name, want) in [
            ("Effect", key::EFFECT),
            ("Tile", key::TILE),
            ("Velocity", key::VELOCITY),
            ("PatchDivisions", key::PATCH_DIVISIONS),
            ("LODDecimation", key::LOD_DECIMATION),
            ("MainTexture", key::MAIN_TEXTURE),
            ("FoamTexture", key::FOAM_TEXTURE),
            ("FoamTile", key::FOAM_TILE),
            ("OceanEnable", key::OCEAN_ENABLE),
            ("DisableLowRes", key::DISABLE_LOW_RES),
            ("OscillationEnable", key::OSCILLATION_ENABLE),
            ("SpecularEnable", key::SPECULAR_ENABLE),
            ("PhillipsConstant", key::PHILLIPS_CONSTANT),
            ("WindSpeed", key::WIND_SPEED),
            ("WindDirection", key::WIND_DIRECTION),
            ("MinDiffuseColor", key::MIN_DIFFUSE_COLOR),
            ("MaxDiffuseColor", key::MAX_DIFFUSE_COLOR),
            ("BorderDiffuseColor", key::BORDER_DIFFUSE_COLOR),
            ("SpecularColor", key::SPECULAR_COLOR),
            ("SpeckleSpecularColor", key::SPECKLE_SPECULAR_COLOR),
            ("SpeckleAmbientColor", key::SPECKLE_AMBIENT_COLOR),
            ("SpeckleTextures", key::SPECKLE_TEXTURES),
            ("SpeckleTile", key::SPECKLE_TILE),
            ("SpeckleScrollSpeed", key::SPECKLE_SCROLL_SPEED),
            ("SpeckleCoordShift", key::SPECKLE_COORD_SHIFT),
            ("LightAzimAndElev", key::LIGHT_AZIM_AND_ELEV),
            ("Enable", key::ENABLE),
            ("NumBloomPasses", key::NUM_BLOOM_PASSES),
            ("MaxTotalWeight", key::MAX_TOTAL_WEIGHT),
            ("GlowThreshold", key::GLOW_THRESHOLD),
            ("Color", key::COLOR),
            ("SunlightFadeFactor", key::SUNLIGHT_FADE_FACTOR),
            ("SkyDomeDarkenFactor", key::SKY_DOME_DARKEN_FACTOR),
            ("BrightnessMin", key::BRIGHTNESS_MIN),
            ("FadeTime", key::FADE_TIME),
            ("TimeBetweenFlashesMinMax", key::TIME_BETWEEN_FLASHES),
            ("TimeBetweenSubFlashesMinMax", key::TIME_BETWEEN_SUB_FLASHES),
            ("NumSubFlashesMinMax", key::NUM_SUB_FLASHES),
            ("HorizonAngleMinMax", key::HORIZON_ANGLE),
            ("SoundCrack", key::SOUND_CRACK),
            ("SoundSubCrack", key::SOUND_SUB_CRACK),
            ("LightningBolt", key::LIGHTNING_BOLT),
            ("Texture", key::TEXTURE),
            ("Width", key::WIDTH),
            ("BreakDistance", key::BREAK_DISTANCE),
            ("TextureSize", key::TEXTURE_SIZE),
            ("SpreadFactor", key::SPREAD_FACTOR),
            ("MaxBranches", key::MAX_BRANCHES),
            ("BranchFactor", key::BRANCH_FACTOR),
            ("BranchSpreadFactor", key::BRANCH_SPREAD_FACTOR),
            ("BranchLength", key::BRANCH_LENGTH),
            ("InterpolationSpeed", key::INTERPOLATION_SPEED),
            ("NumChildren", key::NUM_CHILDREN),
            ("ChildBreakDistance", key::CHILD_BREAK_DISTANCE),
            ("ChildTextureSize", key::CHILD_TEXTURE_SIZE),
            ("ChildWidth", key::CHILD_WIDTH),
            ("ChildSpreadFactor", key::CHILD_SPREAD_FACTOR),
            ("ChildColor", key::CHILD_COLOR),
            ("Type", key::TYPE),
            ("Streaks", key::STREAKS),
            ("ParticleSize", key::PARTICLE_SIZE),
            ("Range", key::RANGE),
            ("AlphaMinMax", key::ALPHA_MIN_MAX),
            ("ParticleDensity", key::PARTICLE_DENSITY),
            ("ParticleDensityRange", key::PARTICLE_DENSITY_RANGE),
            ("VelocityRange", key::VELOCITY_RANGE),
            ("StreakLength", key::STREAK_LENGTH),
            ("CameraCrossVelocityScale", key::CAMERA_CROSS_VELOCITY_SCALE),
            ("CameraAxialVelocityScale", key::CAMERA_AXIAL_VELOCITY_SCALE),
            ("Rotation", key::ROTATION),
            ("RotationRange", key::ROTATION_RANGE),
            ("GroundEffect", key::GROUND_EFFECT),
            ("GroundEffectsPerSec", key::GROUND_EFFECTS_PER_SEC),
            ("GroundEffectSpread", key::GROUND_EFFECT_SPREAD),
            ("VelocityChangeRate", key::VELOCITY_CHANGE_RATE),
        ] {
            assert_eq!(fnv1a_lowercase(name), want, "{name}");
        }
    }

    /// One DATA record: `[hash][argc][argc*4 slots][blobLen][blob]`, with
    /// string slots holding `4*argc + offset-into-blob` (see `crate::config`).
    fn data(hash: u32, nums: &[f32], strs: &[&str]) -> Vec<u8> {
        let argc = (nums.len() + strs.len()) as u8;
        let mut blob = Vec::new();
        let mut slots: Vec<u32> = Vec::new();
        for s in strs {
            slots.push(4 * argc as u32 + blob.len() as u32);
            blob.extend(s.as_bytes());
            blob.push(0);
        }
        for n in nums {
            slots.push(n.to_bits());
        }
        let mut p = hash.to_le_bytes().to_vec();
        p.push(argc);
        for s in &slots {
            p.extend(s.to_le_bytes());
        }
        p.extend((blob.len() as u32).to_le_bytes());
        p.extend(blob);
        build_chunk(b"DATA", &p)
    }

    /// An `fx__` holding `Effect("Wind")` first, then `Effect("Water")` with
    /// the given keys — the shipped shape, where Water is never the first
    /// effect in the chunk.
    fn fx(records: Vec<Vec<u8>>) -> Vec<u8> {
        let mut body = data(key::EFFECT, &[], &["Wind"]);
        body.extend(build_chunk(b"SCOP", &data(0xdead_beef, &[1.0], &[])));
        body.extend(data(key::EFFECT, &[], &["Water"]));
        let scope: Vec<u8> = records.concat();
        body.extend(build_chunk(b"SCOP", &scope));
        build_chunk(b"fx__", &body)
    }

    fn parse(records: Vec<Vec<u8>>) -> WaterConfig {
        let buf = fx(records);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        WaterConfig::parse(&c).unwrap().expect("a Water effect")
    }

    #[test]
    fn normal_water_is_a_sequence_of_vectors_with_authored_colours() {
        let w = parse(vec![
            data(0xeac5_87ac, &[16.0, 8.0], &["water_normalmap_"]),
            data(0x5aa5_a3bd, &[90.0, 112.0, 90.0, 150.0], &[]),
            data(0x8a81_de8b, &[200.0; 4], &[]),
            data(0x8f7c_fa82, &[0.2, 0.5], &[]),
        ]);
        let n = w.normal_maps.unwrap();
        assert_eq!(n.textures.len(), 16);
        assert_eq!(n.textures[15], "water_normalmap_15");
        assert_eq!(n.fps, Some(8.0));
        assert_eq!(w.refraction_color, Some([90.0, 112.0, 90.0, 150.0]));
        assert_eq!(w.reflection_color, Some([200.0; 4]));
        assert_eq!(w.fresnel_min_max, Some([0.2, 0.5]));
        assert!(
            w.main_texture.is_none(),
            "a normal is never an albedo binding"
        );
    }

    #[test]
    fn repeated_named_effects_keep_the_first_scope() {
        for (name, field) in [
            ("Water", key::TILE),
            ("HDR", key::GLOW_THRESHOLD),
            ("Lightning", key::BRIGHTNESS_MIN),
        ] {
            let mut body = Vec::new();
            for value in [2.0, 3.0] {
                body.extend(data(key::EFFECT, &[], &[name]));
                body.extend(build_chunk(b"SCOP", &data(field, &[value], &[])));
            }
            let buf = build_chunk(b"fx__", &body);
            let c = crate::chunks(&buf).next().unwrap().unwrap();
            let value = match name {
                "Water" => WaterConfig::parse(&c).unwrap().unwrap().tile.unwrap()[0],
                "HDR" => HdrConfig::parse(&c).unwrap().unwrap().glow_threshold,
                _ => LightningConfig::parse(&c).unwrap().unwrap().brightness_min,
            };
            assert_eq!(value, 2.0);
        }
    }

    #[test]
    fn the_water_effect_is_found_past_another_effect() {
        let w = parse(vec![
            data(key::MAIN_TEXTURE, &[], &["kas2_water"]),
            data(key::TILE, &[2.0, 2.0], &[]),
            data(key::OCEAN_ENABLE, &[0.0], &[]),
        ]);
        assert_eq!(w.main_texture.as_deref(), Some("kas2_water"));
        assert_eq!(w.tile, Some([2.0, 2.0]));
        assert_eq!(w.ocean_enable, Some(0.0));
        // `Effect("Wind")`'s own key must not leak into the Water scope.
        assert!(w.unknown.is_empty(), "{:?}", w.unknown);
    }

    /// An `fx__` with no `Effect("Water")` — every chunk on the 22 dry maps,
    /// and most of the 3–9 on each wet one.
    #[test]
    fn a_chunk_without_the_water_effect_is_none() {
        let mut body = data(key::EFFECT, &[], &["Lightning"]);
        body.extend(build_chunk(b"SCOP", &data(key::TILE, &[9.0, 9.0], &[])));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        assert_eq!(WaterConfig::parse(&c).unwrap(), None);
    }

    /// DEA1's shape: a lowercase `hdr`, `MaxTotalWeight` authored twice (the
    /// later wins), an unread key skipped, the threshold left at its default.
    #[test]
    fn the_hdr_effect_takes_the_later_key_and_the_defaults() {
        let mut body = data(key::EFFECT, &[], &["Water"]);
        body.extend(build_chunk(b"SCOP", &data(key::TILE, &[1.0, 1.0], &[])));
        body.extend(data(key::EFFECT, &[], &["hdr"]));
        let scope = [
            data(key::ENABLE, &[1.0], &[]),
            data(key::NUM_BLOOM_PASSES, &[4.0], &[]),
            data(key::MAX_TOTAL_WEIGHT, &[1.0], &[]),
            data(0x7e6c_7187, &[0.25], &[]), // DownSizeFactor, unread
            data(key::MAX_TOTAL_WEIGHT, &[1.1], &[]),
        ]
        .concat();
        body.extend(build_chunk(b"SCOP", &scope));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let h = HdrConfig::parse(&c).unwrap().expect("an HDR effect");
        assert_eq!(
            h,
            HdrConfig {
                enable: true,
                num_bloom_passes: 4,
                max_total_weight: 1.1,
                glow_threshold: 0.5
            }
        );
        assert_eq!(
            WaterConfig::parse(&c).unwrap().map(|w| w.tile),
            Some(Some([1.0, 1.0]))
        );
    }

    /// `DisableLowRes` carries no arguments, so its presence is its value.
    #[test]
    fn disable_low_res_is_a_bare_flag() {
        assert!(!parse(vec![data(key::TILE, &[1.0, 1.0], &[])]).disable_low_res);
        assert!(parse(vec![data(key::DISABLE_LOW_RES, &[], &[])]).disable_low_res);
    }

    /// The prefix form expands to the names the bake writes, and the list
    /// form (both KAM1s) is taken as authored.
    #[test]
    fn speckle_textures_have_two_authored_forms() {
        let w = parse(vec![
            data(
                key::SPECKLE_TEXTURES,
                &[25.0, 4.0],
                &["water_specularmask_"],
            ),
            data(key::SPECKLE_TILE, &[5.0, 5.0], &[]),
        ]);
        let s = w.speckle.expect("speckle");
        assert_eq!(s.textures.len(), 25);
        assert_eq!(s.textures[0], "water_specularmask_0");
        assert_eq!(s.textures[24], "water_specularmask_24");
        assert_eq!(s.fps, Some(4.0));
        assert_eq!(s.tile, Some([5.0, 5.0]));

        let w = parse(vec![data(
            key::SPECKLE_TEXTURES,
            &[],
            &["water_specles0", "water_specles1"],
        )]);
        let s = w.speckle.expect("speckle");
        assert_eq!(s.textures, ["water_specles0", "water_specles1"]);
        assert_eq!(s.fps, None, "the list form authors no rate");
    }

    /// A `count` no authoring produces must not size an allocation.
    #[test]
    fn a_wild_speckle_count_is_clamped() {
        let w = parse(vec![data(
            key::SPECKLE_TEXTURES,
            &[1.0e9, 4.0],
            &["frame_"],
        )]);
        assert_eq!(w.speckle.unwrap().textures.len(), MAX_SPECKLE_FRAMES);
        let w = parse(vec![data(key::SPECKLE_TEXTURES, &[-3.0, 4.0], &["frame_"])]);
        assert!(w.speckle.unwrap().textures.is_empty());
    }

    /// The two formulas of §3, on BF1 KAS2's authored numbers: 128 m cells,
    /// `INFO5` 4, `Tile(2,2)`, `Velocity(0.05, 0.05)`.
    #[test]
    fn the_tile_and_velocity_formulas_take_info5() {
        let w = parse(vec![
            data(key::TILE, &[2.0, 2.0], &[]),
            data(key::VELOCITY, &[0.05, 0.05], &[]),
        ]);
        assert_eq!(w.metres_per_repeat(128.0, 4), Some([16.0, 16.0]));
        assert_eq!(w.scroll_per_second(4), Some([0.0125, 0.0125]));
        // BF1 YAV1: 32 m cells, INFO5 1, Tile(2,2).
        assert_eq!(w.metres_per_repeat(32.0, 1), Some([16.0, 16.0]));
        // A zero or absent Tile must not divide by zero.
        let z = parse(vec![data(key::TILE, &[0.0, 0.0], &[])]);
        assert_eq!(z.metres_per_repeat(32.0, 1), None);
        let n = parse(vec![data(key::OCEAN_ENABLE, &[1.0], &[])]);
        assert_eq!(n.metres_per_repeat(32.0, 1), None);
        assert_eq!(n.scroll_per_second(1), None);
    }

    /// Unnamed keys keep their values rather than being dropped or guessed.
    #[test]
    fn unconfirmed_keys_are_recorded_raw() {
        let w = parse(vec![data(0x3e68_131d, &[148.0, 170.0, 192.0, 255.0], &[])]);
        assert_eq!(w.unknown, [(0x3e68_131d, vec![148.0, 170.0, 192.0, 255.0])]);
    }

    /// An `fx__` holding `Effect("Lightning")` with the given keys and one
    /// `LightningBolt("skybolt")` after it — both KAM1s' shape.
    fn lightning_fx(records: Vec<Vec<u8>>) -> Vec<u8> {
        let mut body = data(key::EFFECT, &[], &["Wind"]);
        body.extend(build_chunk(b"SCOP", &data(key::ENABLE, &[0.0], &[])));
        body.extend(data(key::EFFECT, &[], &["Lightning"]));
        body.extend(build_chunk(b"SCOP", &records.concat()));
        body.extend(data(key::LIGHTNING_BOLT, &[], &["skybolt"]));
        let bolt = [
            data(key::TEXTURE, &[], &["lightning"]),
            data(key::WIDTH, &[30.0], &[]),
            data(key::CHILD_COLOR, &[255.0, 255.0, 255.0, 150.0], &[]),
        ];
        body.extend(build_chunk(b"SCOP", &bolt.concat()));
        build_chunk(b"fx__", &body)
    }

    /// Card KSKY's pin: the discs' `SkyDomeDarkenFactor(0.4)` rests the dome
    /// at 255·(1 − 0.4) = 153, the byte PS2 RAM holds (`re-kamino.md` §1).
    #[test]
    fn the_lightning_effect_rests_the_dome_at_153() {
        let buf = lightning_fx(vec![
            data(key::ENABLE, &[1.0], &[]),
            data(key::COLOR, &[255.0, 255.0, 255.0], &[]),
            data(key::SKY_DOME_DARKEN_FACTOR, &[0.4], &[]),
            data(key::TIME_BETWEEN_FLASHES, &[3.0, 5.0], &[]),
            data(key::SOUND_CRACK, &[], &["kam_amb_thunder"]),
        ]);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let l = LightningConfig::parse(&c).unwrap().expect("a Lightning");
        // `Effect("Wind")`'s `Enable(0)` must not have switched it off.
        assert!(l.enable);
        assert_eq!(l.resting_dome_colour(), Some(153));
        assert_eq!(l.color, [255.0; 3]);
        assert_eq!(l.time_between_flashes, [3.0, 5.0]);
        assert_eq!(l.sound_crack.as_deref(), Some("kam_amb_thunder"));
        // Unauthored keys keep the ctor's values.
        assert_eq!(l.fade_time, 0.2);
        assert_eq!(l.num_sub_flashes, [2.0, 5.0]);

        let bolts = LightningBoltConfig::parse_all(&c).unwrap();
        assert_eq!(bolts.len(), 1);
        assert_eq!(bolts[0].name, "skybolt");
        assert_eq!(bolts[0].texture.as_deref(), Some("lightning"));
        assert_eq!(bolts[0].width, Some(30.0));
        assert_eq!(bolts[0].child_color, Some([255.0, 255.0, 255.0, 150.0]));
        assert_eq!(bolts[0].branch_length, None);
    }

    /// An effect that is authored but not enabled darkens nothing, and a map
    /// without one parses to `None`.
    #[test]
    fn a_disabled_or_absent_lightning_leaves_the_dome_white() {
        let buf = lightning_fx(vec![data(key::SKY_DOME_DARKEN_FACTOR, &[0.4], &[])]);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let l = LightningConfig::parse(&c).unwrap().unwrap();
        assert!(!l.enable);
        assert_eq!(l.resting_dome_colour(), None);

        let buf = fx(vec![data(key::TILE, &[2.0, 2.0], &[])]);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        assert_eq!(LightningConfig::parse(&c).unwrap(), None);
        assert!(LightningBoltConfig::parse_all(&c).unwrap().is_empty());
    }

    /// Card WTHR: BF2 KAM1's rain and wind blocks, as the disc authors them
    /// (re-weather.md §1's row), and a snow block's defaults.
    #[test]
    fn precipitation_and_wind_parse() {
        let mut body = data(key::EFFECT, &[], &["Wind"]);
        let wind = [
            data(key::ENABLE, &[1.0], &[]),
            data(key::VELOCITY, &[1.0, 0.3], &[]),
            data(key::VELOCITY_RANGE, &[1.0], &[]),
            data(key::VELOCITY_CHANGE_RATE, &[0.1], &[]),
        ];
        body.extend(build_chunk(b"SCOP", &wind.concat()));
        body.extend(data(key::EFFECT, &[], &["Precipitation"]));
        let rain = [
            data(key::ENABLE, &[1.0], &[]),
            data(key::TYPE, &[], &["Streaks"]),
            data(key::RANGE, &[12.5], &[]),
            data(key::COLOR, &[216.0, 220.0, 228.0], &[]),
            data(key::VELOCITY, &[8.0], &[]),
            data(key::PARTICLE_DENSITY, &[80.0], &[]),
            data(key::STREAK_LENGTH, &[1.7], &[]),
            data(key::ALPHA_MIN_MAX, &[0.8, 1.0], &[]),
            data(key::PARTICLE_SIZE, &[0.06], &[]),
            data(key::CAMERA_CROSS_VELOCITY_SCALE, &[0.2], &[]),
            data(key::GROUND_EFFECT, &[], &["com_sfx_rainsplash"]),
            data(key::GROUND_EFFECTS_PER_SEC, &[7.0], &[]),
            data(key::GROUND_EFFECT_SPREAD, &[8.0], &[]),
        ];
        body.extend(build_chunk(b"SCOP", &rain.concat()));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let w = WindConfig::parse(&c).unwrap().expect("a Wind");
        assert!(w.enable);
        assert_eq!(
            (w.velocity, w.velocity_range, w.velocity_change_rate),
            ([1.0, 0.3], 1.0, 0.1)
        );
        let p = PrecipitationConfig::parse(&c).unwrap().expect("a block");
        assert!(p.enable && p.streaks);
        // The wind's Velocity(1, 0.3) must not have leaked into the rain's.
        assert_eq!((p.velocity, p.range, p.particle_density), (8.0, 12.5, 80.0));
        assert_eq!(p.color, [216, 220, 228, 255], "three numbers: alpha 255");
        assert_eq!(p.alpha_min_max, [0.8, 1.0]);
        assert_eq!(p.streak_length, 1.7);
        assert_eq!(p.camera_cross_velocity_scale, 0.2);
        assert_eq!(p.camera_axial_velocity_scale, 1.0, "the ctor's");
        assert_eq!(p.ground_effect.as_deref(), Some("com_sfx_rainsplash"));
        assert_eq!(
            (p.ground_effects_per_sec, p.ground_effect_spread),
            (7.0, 8.0)
        );

        let mut body = data(key::EFFECT, &[], &["Precipitation"]);
        let snow = [
            data(key::TYPE, &[], &["Quads"]),
            data(key::TEXTURE, &[], &["fx_snow"]),
            data(key::ALPHA_MIN_MAX, &[0.4], &[]),
        ];
        body.extend(build_chunk(b"SCOP", &snow.concat()));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let p = PrecipitationConfig::parse(&c).unwrap().unwrap();
        assert!(!p.enable && !p.streaks);
        assert_eq!(p.texture.as_deref(), Some("fx_snow"));
        assert_eq!(p.alpha_min_max, [0.4, 0.4], "one argument sets both");
        assert_eq!(p.color, [192, 192, 192, 160]);
        assert_eq!(WindConfig::parse(&c).unwrap(), None);
    }
}
