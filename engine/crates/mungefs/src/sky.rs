//! `sky_` chunk: a map's sky dome, sun, ambient and fog.
//!
//! Every BF1 and BF2 ground map LVL carries exactly one `sky_` (measured over
//! all 285 LVLs on both discs, 2026-08-31). Like `lght` it is a munged config
//! tree ([`crate::config`]), so property names are stored only as
//! `fnv1a_lowercase` hashes; every name below is a **confirmed preimage**
//! (pinned by `hash_preimages_are_confirmed`). Hashes we could not confirm are
//! kept raw in [`Sky::unknown`] and never guessed at.
//!
//! Shape, byte-verified on BF1 TAT1/HOT1/BES1 and BF2 TAT2/DEA1/GEO1:
//! ```text
//! SkyInfo()
//!   FogColor(r, g, b)                  // 0..255
//!   FogRange(near, far)                // metres
//!   NearSceneRange(a, b, c) FarSceneRange(d)
//!   AmbientColor(r, g, b)              // 0..255
//!   CharacterAmbientColor(r, g, b) VehicleAmbientColor(r, g, b)
//! SunInfo()
//!   Angle(a0, a1)  Color(r, g, b)      // 0..255
//! DomeInfo()
//!   Texture("SKY_tatooine") Angle(-90) Ambient(r, g, b) Intensity(50) ...
//!   DomeModel() { Geometry("tat1_sky_dome") OffSet(100) MovementScale(0.995) }
//!   LowResTerrain() { Texture("tat1") MaxDistance(3000)
//!                     FogNear(400) FogFar(3500) FogColor(r, g, b, a) }
//! SkyObject() { Geometry("imp_fly_destroyer_DOME") NumObjects(2) ... }
//! ```
//!
//! **`SunInfo.Angle` is BF1's sun, read as the PS2 reads it** (BF1
//! `FUN_002e4dc8` -> `FUN_004931e0`, `analysis/re-sun-direction.md` §1
//! `[O]`): with a = `Angle` in degrees (`Angle[1]` = 0 when only one value is
//! stored), the light TRAVELS along `L = (−sin a0·cos a1, cos a0,
//! −sin a0·sin a1)` in the engine's world axes — [`SunInfo::light_vector`].
//! The PS2's RAM (0x69c590) equals this to ≤ 0.004° on all 16 BF1 maps with a
//! PCSX2 state. BF2 never reads `SunInfo` (§5): its sun is the `lght`
//! Light1 ([`crate::light`]).
//!
//! **Colour scales are per field and were measured, not assumed:**
//! `SkyInfo.*Color`, `SunInfo.Color`, `DomeInfo.Ambient` and `lght`
//! `GlobalLights.Top/Bottom` are 0..255; `lght` `Light.Color` and
//! `RainInfo.DomeColor` are 0..1.
//!
//! The FULL confirmed dictionary — 122 of the 123 `sky_` hashes and all 23
//! `lght` hashes, with per-map values — is `analysis/re-sky-lighting.md`.
//! This module parses what the bake consumes (and a few sun and light keys it
//! records without a consumer, marked on their fields); every other top-level,
//! `SkyInfo`, `SunInfo` and `DomeInfo` key lands in [`Sky::unknown`] with its
//! values,
//! including the weather blocks
//! (`RainInfo`, `PuffInfo`, `SplatInfo`, `FlatInfo`, `StarsInfo`, `WaterInfo`,
//! `MirrorInfo`) and `SkyInfo`'s `Enable` (0xaf8bb8ce), which is a confirmed
//! preimage that stores **1559.0** on eight BF1 maps and 1.0 on the other
//! eight — byte-verified, so either the name is a 32-bit collision or the
//! field is not a boolean. Not used.
//!
//! One collision is proven and matters: **`Type` and `StaticReflectionName`
//! both hash to 0x5127f14d.** The field is `Type` (1 directional, 2 omni,
//! 3 spot: BF2 `FUN_00596978`, analysis/re-point-lights.md §1 `[O]` —
//! [`crate::light::Light::kind`]). A hash match alone is therefore
//! evidence, not proof — which is why the names above are the ones that also
//! appear verbatim in a real shipped source file or a curated hash table.

use crate::config::{parse_tree, ConfigNode};
use crate::{Chunk, UcfbError};

/// Confirmed `fnv1a_lowercase` preimages (see `hash_preimages_are_confirmed`).
mod key {
    pub const SKY_INFO: u32 = 0x06c0_d3e6; // "SkyInfo"
    pub const SUN_INFO: u32 = 0x3780_c8f9; // "SunInfo"
    pub const DOME_INFO: u32 = 0x185a_b6c2; // "DomeInfo"
    pub const DOME_MODEL: u32 = 0x8268_1057; // "DomeModel"
    pub const LOW_RES_TERRAIN: u32 = 0xddf0_c29e; // "LowResTerrain"
    pub const SKY_OBJECT: u32 = 0xb7e0_c3f5; // "SkyObject"
    pub const GEOMETRY: u32 = 0x2a6d_8591; // "Geometry"
    pub const FOG_COLOR: u32 = 0x023b_1fce; // "FogColor"
    pub const FOG_RANGE: u32 = 0xcf49_a8d4; // "FogRange"
    pub const FOG_NEAR: u32 = 0xcc60_f1a1; // "FogNear"
    pub const FOG_FAR: u32 = 0x7e28_d534; // "FogFar"
    pub const AMBIENT_COLOR: u32 = 0x627e_bb4a; // "AmbientColor"
    pub const CHARACTER_AMBIENT_COLOR: u32 = 0xcda1_2257; // "CharacterAmbientColor"
    pub const VEHICLE_AMBIENT_COLOR: u32 = 0x47c6_806c; // "VehicleAmbientColor"
    pub const NEAR_SCENE_RANGE: u32 = 0x5055_1c9a; // "NearSceneRange"
    pub const FAR_SCENE_RANGE: u32 = 0x0119_2b7b; // "FarSceneRange"
    pub const ANGLE: u32 = 0xad54_4418; // "Angle"
    pub const COLOR: u32 = 0x3d7e_6258; // "Color"
    pub const TEXTURE: u32 = 0x3c64_68f4; // "Texture"
    pub const AMBIENT: u32 = 0x1c96_40eb; // "Ambient"
    pub const INTENSITY: u32 = 0x8563_e50a; // "Intensity"
    pub const FILTER: u32 = 0xc7e1_6877; // "Filter"
    pub const THRESHOLD: u32 = 0x4939_f3f8; // "Threshold"
    pub const OFFSET: u32 = 0x14c8_d3ca; // "Offset"
    pub const MOVEMENT_SCALE: u32 = 0xfd58_fbfa; // "MovementScale"
    pub const MAX_DISTANCE: u32 = 0xe691_5cf6; // "MaxDistance"
    pub const TEXTURE_SCALE: u32 = 0x6d9b_436e; // "TextureScale"
    pub const NUM_OBJECTS: u32 = 0x3079_26c1; // "NumObjects"
    pub const HEIGHT: u32 = 0xd5bd_bb42; // "Height"
    pub const DISTANCE: u32 = 0x2eb3_1462; // "Distance"
    pub const DEGREE: u32 = 0x98d3_be69; // "Degree"
    pub const BACK_ANGLE: u32 = 0xccf2_5af1; // "BackAngle"
    pub const BACK_COLOR: u32 = 0x9744_e135; // "BackColor"
    pub const BACK_DEGREE: u32 = 0xfad2_260a; // "BackDegree"
    pub const TERRAIN_ENABLE: u32 = 0xae29_1b6d; // "TerrainEnable"
    pub const REFLECT: u32 = 0x92c7_78aa; // "Reflect"
    pub const REFLECTION_COLOR: u32 = 0x8a81_de8b; // "ReflectionColor"
    pub const STARS: u32 = 0x622b_0026; // "Stars"
    pub const NUM_STARS: u32 = 0x836b_de54; // "NumStars"
    pub const RANDOM_SEED: u32 = 0x69bf_5463; // "RandomSeed"
    pub const TWINKLE_FACTOR: u32 = 0xa435_aaa4; // "TwinkleFactor"
    pub const TWINKLE_FREQUENCY: u32 = 0xa9ee_e423; // "TwinkleFrequency"
    pub const STAR_TEXTURE: u32 = 0xda68_9440; // "StarTexture"
    pub const BRIGHT_STAR_PERCENT: u32 = 0x7368_37de; // "BrightStarPercent"
    pub const ALPHA_MIN: u32 = 0x7de4_197d; // "AlphaMin"
    pub const COLOR_SATURATION: u32 = 0x0d7e_c5c0; // "ColorSaturation"
    pub const ENABLE_BOTTOM: u32 = 0x02ff_3675; // "EnableBottom"
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkyInfo {
    /// 0..255.
    pub fog_color: Option<[f32; 3]>,
    /// (near, far) metres.
    pub fog_range: Option<[f32; 2]>,
    /// 0..255.
    pub ambient_color: Option<[f32; 3]>,
    pub character_ambient_color: Option<[f32; 3]>,
    pub vehicle_ambient_color: Option<[f32; 3]>,
    pub near_scene_range: Option<[f32; 3]>,
    pub far_scene_range: Option<f32>,
}

/// `SunInfo`. `angle` is as stored — use [`SunInfo::light_vector`] rather
/// than reading it directly.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SunInfo {
    pub angle: Option<[f32; 2]>,
    /// 0..255.
    pub color: Option<[f32; 3]>,
    /// `Degree` — the sun sprite's size/spread in degrees (-50..90 as
    /// stored). Not consumed; recorded because it belongs to the sun.
    pub degree: Option<f32>,
    /// A second, opposing light: `BackAngle` (2), `BackColor` (0..255, 3),
    /// `BackDegree`. Every BF1 map authors one — TAT1 stores
    /// BackAngle(180, 0) with BackColor(150, 150, 150). Parsed, never
    /// consumed: neither ELF reads the three `Back*` keys
    /// (`analysis/re-sun-direction.md` §5 `[O]`).
    pub back_angle: Option<[f32; 2]>,
    pub back_color: Option<[f32; 3]>,
    pub back_degree: Option<f32>,
}

impl SunInfo {
    /// BF1's light vector L: the unit direction the sun's light TRAVELS, as
    /// BF1 `FUN_004931e0` computes it from `Angle` (degrees; see the module
    /// doc). `None` if `Angle` is absent.
    pub fn light_vector(&self) -> Option<[f32; 3]> {
        let [a0, a1] = self.angle?;
        let (s0, c0) = a0.to_radians().sin_cos();
        let (s1, c1) = a1.to_radians().sin_cos();
        Some([-s0 * c1, c0, -s0 * s1])
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DomeModel {
    /// `modl` name; empty on placeholder entries the discs also ship.
    pub geometry: String,
    /// Vertical offset of the shell, metres.
    pub offset: Option<f32>,
    /// How much of the camera's motion the shell follows (0.995 = near-fixed).
    pub movement_scale: Option<f32>,
    /// `Reflect` (no arguments): the PS2 also draws this shell mirrored under
    /// the eye. Authored only on BF1 KAS1 and KAS2 / KAS2_H
    /// (`analysis/re-sky-reflect.md` §1 [V]).
    pub reflect: bool,
    pub effects: Vec<DomeEffect>,
    pub rotation: Option<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomeEffect {
    pub effect: String,
    pub hardpoint: String,
    pub parameter: Option<f32>,
    pub max_delay: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LowResTerrain {
    pub texture: Option<String>,
    pub max_distance: Option<f32>,
    /// `TextureScale` — how often [`LowResTerrain::texture`] repeats across
    /// the sheet. 20 on BES1/BES2, 19 on END1/YAV1 (both discs), absent (=
    /// the loader's 1.0) everywhere else. The PS2 folds it into the VU's UV
    /// scale (analysis/re-lowr.md §1: `k = extent/65536 * S58 * 2048 / gu /
    /// gridSize`, `S58` = this key); `bake::lowres_glb` bakes it into the
    /// sheet's UVs.
    pub texture_scale: Option<f32>,
    pub fog_near: Option<f32>,
    pub fog_far: Option<f32>,
    /// 0..255 with a fourth value the discs also store (BF1 TAT1: 164, 174,
    /// 194, 80). The fourth value's meaning is NOT decoded.
    pub fog_color: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DomeInfo {
    pub texture: Option<String>,
    pub angle: Option<f32>,
    /// 0..255.
    pub ambient: Option<[f32; 3]>,
    /// `Filter`, `Threshold` (0..255) and `Intensity` (/128): the frame
    /// filter both games run over the 3D scene before the HUD when `Filter`
    /// ≠ 0 — frame += Intensity/128 · max(0, low-pass(frame) − Threshold)
    /// in display bytes (B1 `FUN_002e29e0` → `FUN_0049fe60`, B2
    /// `FUN_003bcd50` → `FUN_00585890`; `analysis/re-glow.md` §3 `[O]`).
    /// Every BF1 map authors it (HOT1 at Intensity 0); on BF2 only TAT2
    /// and TAT3.
    pub filter: Option<f32>,
    pub threshold: Option<f32>,
    pub intensity: Option<f32>,
    pub models: Vec<DomeModel>,
    pub low_res_terrain: Option<LowResTerrain>,
    /// `TerrainEnable` as stored — authored only as 0, on BF1 KAM1 and BES1.
    /// BF1 reads it into `DAT_005229a8` (default 1) and at 0 draws no
    /// terrain at all; BF2's ELF never reads the key
    /// (`analysis/re-terrain-enable.md` §1-§2 `[O]`).
    pub terrain_enable: Option<f32>,
    /// `ReflectionColor`, 0..255 — the tint of the mirrored [`DomeModel::reflect`]
    /// copies. Authored only on BF1 KAS1, (200, 200, 255); BF1's loader
    /// clamps it to bytes and defaults it to white (`analysis/re-sky-reflect.md` §1 [V]).
    pub reflection_color: Option<[f32; 3]>,
    /// `Stars()`: the star field drawn in the sky pass, as authored.
    pub stars: Option<Stars>,
}

/// `DomeInfo.Stars()` as stored (`analysis/re-stars.md` §1). Authored on BF1
/// KAS1 and RHN2 and BF2 COR1 and NAB2 among the ground maps. Each field is
/// `None` when the block leaves it out; the loaders' defaults are the
/// consumer's (`manifest::ManifestStars`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stars {
    pub num_stars: Option<f32>,
    pub random_seed: Option<f32>,
    pub twinkle_factor: Option<f32>,
    pub twinkle_frequency: Option<f32>,
    /// 0..255, three or four values.
    pub color: Option<Vec<f32>>,
    /// Authored `fx_star`; the draw is untextured (re-stars §3).
    pub texture: Option<String>,
    pub bright_star_percent: Option<f32>,
    pub alpha_min: Option<f32>,
    pub color_saturation: Option<f32>,
    /// BF2 only: the field is also drawn mirrored under the horizon.
    pub enable_bottom: Option<f32>,
}

/// A prop that circles in the sky (capital ships over Tatooine, etc.).
/// Consumed: `bake::lighting` (crates/bake/src/lighting.rs:342) turns these
/// into `ManifestSky::objects` and `game::viewer::scene::upload_sky_objects` draws
/// them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkyObject {
    pub geometry: String,
    pub num_objects: Option<f32>,
    pub height: Option<[f32; 2]>,
    pub distance: Option<f32>,
    pub velocity_x: Option<[f32; 2]>,
    pub velocity_y: Option<[f32; 2]>,
    pub velocity_z: Option<[f32; 2]>,
    pub acceleration: Option<[f32; 3]>,
    pub direction_factor: Option<f32>,
    pub lifetime: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sky {
    pub info: SkyInfo,
    pub sun: Option<SunInfo>,
    pub dome: Option<DomeInfo>,
    pub objects: Vec<SkyObject>,
    /// Top-level and in-block hashes whose names are not confirmed, with the
    /// values as stored.
    pub unknown: Vec<(u32, Vec<f32>)>,
}

impl Sky {
    pub fn parse(sky: &Chunk<'_>) -> Result<Sky, UcfbError> {
        let mut out = Sky::default();
        for node in parse_tree(sky)? {
            match node.hash {
                key::SKY_INFO => out.info = parse_sky_info(&node, &mut out.unknown),
                key::SUN_INFO => out.sun = Some(parse_sun(&node, &mut out.unknown)),
                key::DOME_INFO => out.dome = Some(parse_dome(&node, &mut out.unknown)),
                key::SKY_OBJECT => out.objects.push(parse_object(&node)),
                h => out.unknown.push((h, node.floats())),
            }
        }
        Ok(out)
    }
}

fn parse_sky_info(node: &ConfigNode, unknown: &mut Vec<(u32, Vec<f32>)>) -> SkyInfo {
    let mut s = SkyInfo::default();
    for c in &node.children {
        match c.hash {
            key::FOG_COLOR => s.fog_color = c.f3(),
            key::FOG_RANGE => s.fog_range = Some(c.floats_or_zero()),
            key::AMBIENT_COLOR => s.ambient_color = c.f3(),
            key::CHARACTER_AMBIENT_COLOR => s.character_ambient_color = c.f3(),
            key::VEHICLE_AMBIENT_COLOR => s.vehicle_ambient_color = c.f3(),
            key::NEAR_SCENE_RANGE => s.near_scene_range = c.f3(),
            key::FAR_SCENE_RANGE => s.far_scene_range = c.f32(0),
            h => unknown.push((h, c.floats())),
        }
    }
    s
}

fn parse_sun(node: &ConfigNode, unknown: &mut Vec<(u32, Vec<f32>)>) -> SunInfo {
    let mut s = SunInfo::default();
    for c in &node.children {
        match c.hash {
            key::ANGLE => s.angle = Some(c.floats_or_zero()),
            key::COLOR => s.color = c.f3(),
            key::DEGREE => s.degree = c.f32(0),
            key::BACK_ANGLE => s.back_angle = Some(c.floats_or_zero()),
            key::BACK_COLOR => s.back_color = c.f3(),
            key::BACK_DEGREE => s.back_degree = c.f32(0),
            key::TEXTURE => {}
            h => unknown.push((h, c.floats())),
        }
    }
    s
}

fn parse_dome(node: &ConfigNode, unknown: &mut Vec<(u32, Vec<f32>)>) -> DomeInfo {
    let mut d = DomeInfo::default();
    for c in &node.children {
        match c.hash {
            key::TEXTURE => d.texture = non_empty(c.string.clone()),
            key::ANGLE => d.angle = c.f32(0),
            key::AMBIENT => d.ambient = c.f3(),
            key::INTENSITY => d.intensity = c.f32(0),
            key::FILTER => d.filter = c.f32(0),
            key::THRESHOLD => d.threshold = c.f32(0),
            key::DOME_MODEL => {
                let mut m = DomeModel::default();
                for k in &c.children {
                    match k.hash {
                        key::GEOMETRY => m.geometry = k.string.clone().unwrap_or_default(),
                        key::OFFSET => m.offset = k.f32(0),
                        key::MOVEMENT_SCALE => m.movement_scale = k.f32(0),
                        key::REFLECT => m.reflect = true,
                        0x6e6e_8d54 => {
                            if let (Some(effect), Some(hardpoint)) = (k.str_arg(0), k.str_arg(1)) {
                                m.effects.push(DomeEffect {
                                    effect: effect.to_owned(),
                                    hardpoint: hardpoint.to_owned(),
                                    parameter: k.f32(2),
                                    max_delay: k.f32(3),
                                });
                            }
                        }
                        0x3204_8fba => m.rotation = k.f4(),
                        h => unknown.push((h, k.floats())),
                    }
                }
                if !m.geometry.is_empty() {
                    d.models.push(m);
                }
            }
            key::LOW_RES_TERRAIN => {
                let mut l = LowResTerrain::default();
                for k in &c.children {
                    match k.hash {
                        key::TEXTURE => l.texture = non_empty(k.string.clone()),
                        key::MAX_DISTANCE => l.max_distance = k.f32(0),
                        key::TEXTURE_SCALE => l.texture_scale = k.f32(0),
                        key::FOG_NEAR => l.fog_near = k.f32(0),
                        key::FOG_FAR => l.fog_far = k.f32(0),
                        key::FOG_COLOR => l.fog_color = Some(k.floats_or_zero()),
                        h => unknown.push((h, k.floats())),
                    }
                }
                d.low_res_terrain = Some(l);
            }
            key::TERRAIN_ENABLE => d.terrain_enable = c.f32(0),
            key::REFLECTION_COLOR => d.reflection_color = c.f3(),
            key::STARS => d.stars = Some(parse_stars(c, unknown)),
            h => unknown.push((h, c.floats())),
        }
    }
    d
}

fn parse_stars(node: &ConfigNode, unknown: &mut Vec<(u32, Vec<f32>)>) -> Stars {
    let mut s = Stars::default();
    for k in &node.children {
        match k.hash {
            key::NUM_STARS => s.num_stars = k.f32(0),
            key::RANDOM_SEED => s.random_seed = k.f32(0),
            key::TWINKLE_FACTOR => s.twinkle_factor = k.f32(0),
            key::TWINKLE_FREQUENCY => s.twinkle_frequency = k.f32(0),
            key::COLOR => s.color = Some(k.floats()),
            key::STAR_TEXTURE => s.texture = non_empty(k.string.clone()),
            key::BRIGHT_STAR_PERCENT => s.bright_star_percent = k.f32(0),
            key::ALPHA_MIN => s.alpha_min = k.f32(0),
            key::COLOR_SATURATION => s.color_saturation = k.f32(0),
            key::ENABLE_BOTTOM => s.enable_bottom = k.f32(0),
            h => unknown.push((h, k.floats())),
        }
    }
    s
}

fn parse_object(node: &ConfigNode) -> SkyObject {
    let mut o = SkyObject::default();
    for c in &node.children {
        match c.hash {
            key::GEOMETRY => o.geometry = c.string.clone().unwrap_or_default(),
            key::NUM_OBJECTS => o.num_objects = c.f32(0),
            key::HEIGHT => o.height = Some(c.floats_or_zero()),
            key::DISTANCE => o.distance = c.f32(0),
            0xb6c8_887e => o.velocity_x = Some(c.floats_or_zero()),
            0xb7c8_8a11 => o.velocity_y = Some(c.floats_or_zero()),
            0xb4c8_8558 => o.velocity_z = Some(c.floats_or_zero()),
            0x2fa0_ba9d => o.acceleration = c.f3(),
            0xc91c_0e2c => o.direction_factor = c.f32(0),
            0x5212_abee => o.lifetime = c.f32(0),
            _ => {}
        }
    }
    o
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fnv1a_lowercase;

    #[test]
    fn hash_preimages_are_confirmed() {
        for (name, h) in [
            ("SkyInfo", key::SKY_INFO),
            ("SunInfo", key::SUN_INFO),
            ("DomeInfo", key::DOME_INFO),
            ("DomeModel", key::DOME_MODEL),
            ("LowResTerrain", key::LOW_RES_TERRAIN),
            ("SkyObject", key::SKY_OBJECT),
            ("Geometry", key::GEOMETRY),
            ("FogColor", key::FOG_COLOR),
            ("FogRange", key::FOG_RANGE),
            ("FogNear", key::FOG_NEAR),
            ("FogFar", key::FOG_FAR),
            ("AmbientColor", key::AMBIENT_COLOR),
            ("CharacterAmbientColor", key::CHARACTER_AMBIENT_COLOR),
            ("VehicleAmbientColor", key::VEHICLE_AMBIENT_COLOR),
            ("NearSceneRange", key::NEAR_SCENE_RANGE),
            ("FarSceneRange", key::FAR_SCENE_RANGE),
            ("Angle", key::ANGLE),
            ("Color", key::COLOR),
            ("Texture", key::TEXTURE),
            ("Ambient", key::AMBIENT),
            ("Intensity", key::INTENSITY),
            ("Filter", key::FILTER),
            ("Threshold", key::THRESHOLD),
            ("OffSet", key::OFFSET),
            ("Stars", key::STARS),
            ("NumStars", key::NUM_STARS),
            ("RandomSeed", key::RANDOM_SEED),
            ("TwinkleFactor", key::TWINKLE_FACTOR),
            ("TwinkleFrequency", key::TWINKLE_FREQUENCY),
            ("StarTexture", key::STAR_TEXTURE),
            ("BrightStarPercent", key::BRIGHT_STAR_PERCENT),
            ("AlphaMin", key::ALPHA_MIN),
            ("ColorSaturation", key::COLOR_SATURATION),
            ("EnableBottom", key::ENABLE_BOTTOM),
            ("MovementScale", key::MOVEMENT_SCALE),
            ("MaxDistance", key::MAX_DISTANCE),
            ("NumObjects", key::NUM_OBJECTS),
            ("Height", key::HEIGHT),
            ("Distance", key::DISTANCE),
            ("Degree", key::DEGREE),
            ("BackAngle", key::BACK_ANGLE),
            ("BackColor", key::BACK_COLOR),
            ("BackDegree", key::BACK_DEGREE),
            ("TerrainEnable", key::TERRAIN_ENABLE),
            ("Reflect", key::REFLECT),
            ("ReflectionColor", key::REFLECTION_COLOR),
        ] {
            assert_eq!(fnv1a_lowercase(name), h, "{name}");
        }
    }

    /// BF1's L against the PS2's RAM vector at 0x69c590 on four maps
    /// (`analysis/re-sun-direction.md` §6.1, `lead_tmp/re/sund/ramcheck.out`).
    #[test]
    fn bf1_light_vector_matches_the_ps2_ram() {
        for (angle, ram) in [
            ([140.0, -30.0], [-0.5567, -0.7660, 0.3214]), // YAV1
            ([145.0, 280.0], [-0.0996, -0.8192, 0.5648]), // KAS2
            ([145.0, -30.0], [-0.4967, -0.8192, 0.2868]), // NAB2
            ([180.0, 0.0], [0.0, -1.0, 0.0]),             // KAS1
        ] {
            let sun = SunInfo {
                angle: Some(angle),
                ..SunInfo::default()
            };
            let l = sun.light_vector().unwrap();
            for i in 0..3 {
                assert!((l[i] - ram[i]).abs() < 2e-4, "{angle:?}: {l:?} vs {ram:?}");
            }
        }
    }
}
