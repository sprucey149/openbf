//! Per-map lighting, fog and sky, from the map's own `sky_` and `lght` chunks.
//!
//! Until 2026-08-31 every map rendered under ONE hardcoded sun + ambient,
//! which is why every planet read as the same mid-afternoon. The discs author
//! this per map; [`mungefs::sky`] and [`mungefs::light`] read it and this
//! module turns it into the manifest's `lighting` and `sky` sections.
//!
//! **The reduction below is OURS and PROVISIONAL** — a defensible rendering
//! decision from measured data, not a claim about how the PS2 shaded. What is
//! measured is stated inline; what is chosen is labelled.
//!
//! **The exception is FOG**, which since 2026-09-03 is the ELF's own rule,
//! one per game, read out of both decompiled renderers rather than inferred
//! from the data: see [`fog_from`] and `analysis/re-fog.md` §3/§6. The two
//! games read DIFFERENT chunks — BF2 `SkyInfo.FogRange/FogColor`, BF1
//! `DomeInfo.LowResTerrain.FogNear/FogFar/FogColor` — so the reduction takes
//! the map's `source_game`.
//!
//! Where each value comes from (measured over all 34 ground-map LVLs):
//! | value | BF1 (no `lght` on any map) | BF2 |
//! |---|---|---|
//! | sun direction | `sky_ SunInfo.Angle` | `lght` `GlobalLights.Light1` -> that Light's `Rotation` |
//! | sun colour | `SunInfo.Color` / 255 | that Light's `Color` (0..1) |
//! | second light | none | `GlobalLights.Light2` -> that Light (type 1), queue LITEB |
//! | `Static()` | none (no `lght`) | the key light's and Light2's, queue LITEB |
//! | ambient | `SkyInfo.AmbientColor` / 255 | `GlobalLights.Top`/`Bottom` / 255 |
//! | fog | `DomeInfo.LowResTerrain.FogColor/FogNear/FogFar` | `SkyInfo.FogColor` + `FogRange` |
//!
//! **The sun's direction is the PS2's own vector** (`analysis/re-sun-direction.md`
//! `[O]`, pinned in `mungefs`): BF1 [`mungefs::sky::SunInfo::light_vector`],
//! BF2 [`mungefs::light::Light::light_vector`] for Light1 and Light2. Both are
//! the direction the light TRAVELS in the port's world axes (the PS2's own), so
//! `sun_dir` is the PS2's vector with no conversion. BF2 never reads
//! `SunInfo` (§5), so `lght` wins wherever it exists.
//!
//! **Colour space: the authored 0..255 values are GAMMA-space, and the bake
//! stores them linearised.** This follows the rule the `_dtl` detail
//! modulation already proved on the GPU (RE_LOG "BF1 detail textures"): the
//! PS2 multiplies texel by light in 8-bit gamma space, while our sampler hands
//! the shader a LINEARISED texel, so a gamma-space multiplier `L` must enter
//! the linear pipeline as `srgb_to_linear(L)` for `T_gamma * L_gamma` to come
//! out right. The same transfer also makes an authored fog colour of
//! 128/128/128 actually *appear* as mid-grey on an sRGB target, which the
//! naive `128/255 = 0.502` does not (that lands at sRGB 188).
//!
//! It also balances better, which is a check rather than a reason: a typical
//! map lands at ambient 0.13 + sun 0.89 = 1.02 at normal incidence, against
//! 1.25 for the hardcoded default it replaces. BF1 HOT1 authors a white
//! ambient and comes out at 1.35; it is left as authored (a bright Hoth)
//! rather than silently rescaled.

use mungefs::light::Lights;
use mungefs::sky::Sky;

/// Lighting, fog and sky schema: one definition in the shared `manifest`
/// crate, re-exported here under its old path. See `bake::map`.
pub use manifest::{
    ManifestFilter, ManifestFog, ManifestLight, ManifestLighting, ManifestSky, ManifestSkyModel,
    ManifestSkyObject, ManifestStars,
};

/// The engine's pre-2026-08-31 hardcoded look, kept as the fallback for a map
/// that authors nothing (mirrors `render::Lighting::default`). PROVISIONAL
/// and ours — nothing on either disc authors these.
pub const DEFAULT_SUN_DIR: [f32; 3] = [-0.400_51, -0.801_02, -0.450_57];
pub const DEFAULT_SUN_COLOR: [f32; 3] = [1.0, 1.0, 0.95];
pub const DEFAULT_AMBIENT: [f32; 3] = [0.25, 0.25, 0.28];

/// sRGB electro-optical transfer function, IEC 61966-2-1. Used rather than a
/// plain 2.2 power because the textures these colours multiply are sampled
/// through an sRGB view, which uses exactly this curve.
pub(crate) fn srgb_to_linear(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    if u <= 0.040_45 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

/// An authored 0..255 gamma-space colour as a linear RGB triple.
fn scale255(c: [f32; 3]) -> [f32; 3] {
    [
        srgb_to_linear(c[0] / 255.0),
        srgb_to_linear(c[1] / 255.0),
        srgb_to_linear(c[2] / 255.0),
    ]
}

/// An authored 0..1 gamma-space colour (BF2 `Light.Color`) as linear RGB.
/// The same transfer as [`scale255`]: the PS2 multiplies it into the texel
/// in gamma space like every other colour (`analysis/re-model-lighting.md`
/// §0, queue LITEB; until then it was used raw, as if already linear).
fn linear01(c: [f32; 3]) -> [f32; 3] {
    c.map(srgb_to_linear)
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n <= 1e-6 {
        return DEFAULT_SUN_DIR;
    }
    [v[0] / n, v[1] / n, v[2] / n]
}

/// Which maps get fog, and with which numbers: **ONE rule per game, the
/// ELF's own** (`analysis/re-fog.md` §3 the enable, §6 the per-map table and
/// "What the bake can implement"). `bf2` selects the rule and comes from the
/// manifest's `source_game`, not from the chunk.
///
/// **BF2** (`FUN_00580500` 715498–715535, §1/§3 [V]): fog is on iff
/// `SkyInfo.FogRange(near, far)` is authored with `near < far` — that
/// comparison is the whole enable, and nothing else writes the flag. The
/// colour is `SkyInfo.FogColor` RGB; its 4th component never reaches the
/// store (the ELF forces 0xff) and never reaches us either
/// (`mungefs::sky::SkyInfo::fog_color` is three floats). Black is legal
/// (KAM1 400..1356, TAT3 20..40), `0..0` is off (GEO1, KAS2, POL1), and a
/// `FogColor` with no `FogRange` is off (DAG1, MUS1). `Enable`,
/// `WorldFogRange` and `FogRamp` are stored by the disc and read by nobody.
///
/// **BF1** (§1/§3 [V]): the world fog is `DomeInfo.LowResTerrain`, not
/// `SkyInfo`. Fog is on iff that scope authors BOTH `FogNear` and `FogFar`
/// (14 of the 16 maps; RHN1 and RHN2 have no `LowResTerrain` scope at all).
/// The colour is `LowResTerrain.FogColor` RGB and its 4th component is a
/// DENSITY that stretches the far distance (decomp 270621–270623):
/// with `a` the 4th component, `far' = near + (far − near)·255/a` when `a`
/// is 1..=254, and `far` unchanged at 0 or 255. BF1 KAM1's
/// `400..1000, a=160` is exactly BF2 KAM1's authored `400..1356` through
/// that formula, which is the cross-check that the formula is right (§4).
/// **`SkyInfo.FogColor` / `FogRange` / `FogRamp` are never parsed by the BF1
/// ELF** (hash 0x023b1fce occurs once in the whole BF1 decomp, at 270546,
/// inside the `LowResTerrain` scope; 0xcf49a8d4 and 0xfaaa0e17 occur
/// nowhere), so the ten `FogRange(0, 2700..30000)` maps are dead data on the
/// disc.
///
/// There is no `near < far` test in the BF1 ELF — the rows are simply written
/// — but a `far <= near` triple would give the renderer a degenerate ramp, so
/// it is refused here as a guard. No disc value hits it.
///
/// **HISTORY.** Until 2026-09-03 this was three PROVISIONAL heuristics
/// standing in for an undecoded enable: (1) `far <= near` is not a range;
/// (2) a pure-black fog colour is not authored fog; (3) a ramp starting at
/// the eye and reaching past 2000 m (`UNEDITED_FOG_FAR`) is the disc's
/// unedited default. They existed because BF1's `SkyInfo` fog was being read
/// at all: rendered as authored it turned Tatooine black (0,0,10), Bespin red
/// (255,0,0) and Geonosis white (252,252,252). The RE's answer is that those
/// values were never a problem to solve — **the BF1 ELF does not parse that
/// field**, so no placeholder filter was ever needed; and rules 2 and 3 were
/// wrong for BF2, where black fog is real (KAM1, TAT3). All three are gone.
///
/// **NOT reproduced** (recorded here and in the report, not built):
/// - BF2 fogs only FAR-SCENE draws — beyond `NearSceneRange[2]` — and blends
///   near→far through a depth LUT (§2.5); BF1 fogs near-scene models too
///   (§2.2). The port fogs everything from `near`.
/// - Maps with `FarSceneRange(0)` (re-fog.md §6: BF1 END1/RHN2/YAV1, BF2
///   DAG1/DEA1/END1/FEL1/NAB2/TAT3/YAV1 — nine of them among the 32 baked,
///   TAT3 is not baked; the assertion at the bottom of this file pins the
///   list) have no far layer. The bake keeps their authored fog colour
///   either way; at run time a BF2 one fades its near scene into the sky
///   INSTEAD of fogging (`game::viewer::far_fade_from`, queue 6 — the disc
///   draws no fog colour there, §2.5/§6), while a BF1 one keeps its fog,
///   because BF1 fogs near-scene draws too (§2.2).
/// - The split-screen `FogRange` in BF2 scope 0x23e10337 (KAS2 5..140,
///   MYG1 5..200) is ignored: single player only (§1).
fn fog_from(sky: &Sky, bf2: bool) -> Option<ManifestFog> {
    let (color, near, far) = if bf2 {
        let c = sky.info.fog_color?;
        let [near, far] = sky.info.fog_range?;
        (c, near, far)
    } else {
        let lrt = sky.dome.as_ref()?.low_res_terrain.as_ref()?;
        let near = lrt.fog_near?;
        let far = lrt.fog_far?;
        // The 4th component is the far-stretch density. `mungefs` stores 0.0
        // when the disc omits it, and 0 and 255 both mean "no stretch", so
        // the absent case needs no special value here.
        let [r, g, b, a] = lrt.fog_color.unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let far = if (1.0..=254.0).contains(&a) {
            near + (far - near) * 255.0 / a
        } else {
            far
        };
        ([r, g, b], near, far)
    };
    if !far.is_finite() || !near.is_finite() || far <= near {
        return None;
    }
    Some(ManifestFog {
        color: scale255(color),
        // BF2 FEL1 authors -10..62. The ramp is measured from the eye, so a
        // negative near is clamped; unchanged from the previous rule.
        near: near.max(0.0),
        far,
    })
}

/// `DomeInfo.Ambient`, as a multiplier over the dome's own art. PROVISIONAL
/// and ours, and what it mostly does is REFUSE to darken.
///
/// Measured over all 34 ground maps, the field takes exactly four states and
/// every one of them is a NEUTRAL GREY — never a tint:
///
/// | value | maps |
/// |---|---|
/// | `255, 255, 255` | 23 |
/// | `256, 256, 256` | 2 (BF1 KAS2, BF1 NAB1 — over the 0..255 range) |
/// | `128, 128, 128` | 4 (BF1 and BF2 HOT1, BF1 and BF2 GEO1) |
/// | absent | 5 |
///
/// So it carries no per-map look: it is one of the two values the PS2
/// pipelines use for "no change" (the same pair as the terrain's per-game
/// vertex scale, `analysis/re-terrain-lighting.md` §0). Reading 128 literally made
/// it a 50 % darkening, and singled out exactly Hoth and Geonosis, on both
/// discs, for a half-brightness sky that nothing in their authoring asks for.
/// Both neutrals render as unity; a value that is neither is applied, so a map
/// that really does author a dome tint would still get it.
fn dome_ambient(c: [f32; 3]) -> [f32; 3] {
    // The discs store these as floats and they arrive as 128.00003 /
    // 256.00003, so compare with a tolerance rather than for equality.
    let grey = (c[0] - c[1]).abs() < 0.5 && (c[1] - c[2]).abs() < 0.5;
    let neutral = grey && ((c[0] - 128.0).abs() < 0.5 || c[0] >= 254.5);
    if neutral {
        [1.0; 3]
    } else {
        scale255(c)
    }
}

/// Reduce a map's `sky_` (+ `lght` on BF2) to the manifest's lighting section.
///
/// `bf2` picks the game's fog rule — see [`fog_from`]. It is the manifest's
/// own `source_game` (`bake::map`), never re-derived from the chunk: the two
/// games read DIFFERENT fields out of the same `sky_` layout, so the reduction
/// cannot be game-agnostic.
pub fn lighting_from(sky: Option<&Sky>, lights: Option<&Lights>, bf2: bool) -> ManifestLighting {
    let mut source: Vec<&str> = Vec::new();
    let key = lights.and_then(|l| l.key_light());

    let (sun_dir, sun_color) = match key.and_then(|k| k.light_vector().map(|l| (k, l))) {
        Some((k, l)) => {
            source.push("lght");
            (l, k.color.map(linear01).unwrap_or(DEFAULT_SUN_COLOR))
        }
        None => match sky.and_then(|s| s.sun.as_ref()) {
            Some(sun) => match sun.light_vector() {
                Some(l) => {
                    source.push("sky_");
                    (
                        normalize(l),
                        sun.color.map(scale255).unwrap_or(DEFAULT_SUN_COLOR),
                    )
                }
                None => (DEFAULT_SUN_DIR, DEFAULT_SUN_COLOR),
            },
            None => (DEFAULT_SUN_DIR, DEFAULT_SUN_COLOR),
        },
    };

    let global = lights.and_then(|l| l.global.as_ref());
    let (ambient_sky, ambient_ground) =
        match (global.and_then(|g| g.top), global.and_then(|g| g.bottom)) {
            (Some(t), Some(b)) => {
                if !source.contains(&"lght") {
                    source.push("lght");
                }
                (scale255(t), scale255(b))
            }
            _ => match sky.and_then(|s| s.info.ambient_color) {
                Some(a) => {
                    if !source.contains(&"sky_") {
                        source.push("sky_");
                    }
                    (scale255(a), scale255(a))
                }
                None => (DEFAULT_AMBIENT, DEFAULT_AMBIENT),
            },
        };

    let fog = sky.and_then(|s| fog_from(s, bf2));
    if fog.is_some() && !source.contains(&"sky_") {
        source.push("sky_");
    }

    // `GlobalLights.Light2`: a second directional light, rendered by the PS2
    // on every model while it names a light (the fade DAT_0060c6f4 = 1.0,
    // `analysis/re-model-lighting.md` §3 `[O]`). Type 1 is what the key
    // light and every "sun" author; a Light2 of any other type is not a
    // directional light and is left out, as is one with no rotation.
    let light2 = lights
        .and_then(|l| l.fill_light())
        .filter(|l| l.kind == Some(1.0))
        .and_then(|l| {
            Some(ManifestLight {
                dir: l.light_vector()?,
                color: linear01(l.color?),
                is_static: l.is_static,
            })
        });

    ManifestLighting {
        sun_static: key.is_some_and(|k| k.is_static && k.light_vector().is_some()),
        light2,
        ambient_sky,
        ambient_ground,
        source: if source.is_empty() {
            "default".into()
        } else {
            source.join("+")
        },
        sun_dir,
        sun_color,
        character_ambient: sky
            .and_then(|s| s.info.character_ambient_color)
            .map(scale255),
        fog,
        // Copied through UNCHANGED, in metres, for the runtime's far-scene
        // cull (`game::viewer::far_cull`): the ELF reads both keys off
        // `SkyInfo` on both games (`analysis/re-fog.md` §2.2/§2.5 [V]) and
        // neither is scaled, filtered or defaulted here. A `FarSceneRange` of
        // 0 is real authored data ("no far layer") and survives as `Some(0.0)`
        // — `mungefs::sky` already distinguishes it from an absent key.
        near_scene_range: sky.and_then(|s| s.info.near_scene_range),
        far_scene_range: sky.and_then(|s| s.info.far_scene_range),
        filter: sky.and_then(|s| filter_from(s.dome.as_ref()?)),
    }
}

/// A `lght` chunk's local lights (card BPNT, `analysis/re-point-lights.md`
/// §1 [O]): every `Type(2)` omni and `Type(3)` spot with a `Position` and a
/// `Range` > 0 and no `Texture`, in block order. The position is the PS2's — its loader
/// stores (x, y, −z) (B2 728627) — and the colour is as authored: the
/// runtime packs it as the VU does (`game::point_light`). A spot keeps its
/// axis and outer cone for the gather's sphere-against-cone test (the spot's
/// vtable +0x1c, 0x59c3a8). The axis is the directional light's reduction
/// of `Rotation` ([`mungefs::light::Light::light_vector`]), PROVISIONAL [D]:
/// UTA1's 11 lamp spots all point down by it.
pub fn point_lights_from(lights: &Lights) -> Vec<manifest::ManifestPointLight> {
    lights
        .lights
        .iter()
        .filter_map(|l| {
            let spot = match l.kind? as i32 {
                2 => false,
                3 => true,
                _ => return None,
            };
            // A textured light is the set's PROJECTOR, which the upload
            // keeps out of the point slots (B2 715304).
            if l.texture.as_deref().is_some_and(|t| !t.is_empty()) {
                return None;
            }
            let [x, y, z] = l.position?;
            let range = l.range.filter(|r| *r > 0.0)?;
            Some(manifest::ManifestPointLight {
                name: l.name.clone(),
                spot,
                cone: match (spot, l.bidirectional.unwrap_or(0.0) != 0.0) {
                    (true, false) => l.light_vector().map(|dir| manifest::ManifestSpotCone {
                        dir,
                        outer: l.cone.map_or(std::f32::consts::FRAC_PI_2, |c| c[1]),
                    }),
                    _ => None,
                },
                pos: [x, y, -z],
                color: l.color.unwrap_or([1.0; 3]),
                range,
                is_static: l.is_static,
                overbright: l.overbright,
            })
        })
        .collect()
}

/// `DomeInfo`'s frame filter: both ELFs run it only while `Filter` ≠ 0, and
/// the pass returns at once for `Intensity` < 1 (B1 `FUN_004a57e0` /
/// `FUN_0049fe60`, `analysis/re-glow.md` §3.1 `[O]`). Every map that sets
/// `Filter` authors both values; a missing one is taken as 0 (no lift).
fn filter_from(dome: &mungefs::sky::DomeInfo) -> Option<ManifestFilter> {
    let intensity = dome.intensity.unwrap_or(0.0);
    (dome.filter.unwrap_or(0.0) != 0.0 && intensity >= 1.0).then(|| ManifestFilter {
        threshold: dome.threshold.unwrap_or(0.0),
        intensity,
    })
}

const MAX_SKY_OBJECTS: u32 = 1024;

/// Reduce a map's `sky_ DomeInfo` to the manifest's sky section. A dome
/// referencing geometry that is not in this LVL is dropped here instead of
/// failing at load.
///
/// `resolve` maps an authored geometry name to the name the bake actually
/// wrote, or `None` if it wrote none. It returns the BAKED name rather than a
/// bool because the two differ in case: a `modl` chunk stores its name
/// lowercased while `sky_` keeps the authored spelling
/// (`rep_fly_assault_DOME`), and the runtime looks meshes up by exact name.
pub fn sky_from(
    sky: Option<&Sky>,
    resolve: impl Fn(&str) -> Option<String>,
) -> Option<ManifestSky> {
    let objects: Vec<ManifestSkyObject> = sky
        .map(|s| s.objects.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter_map(|o| {
            let geometry = resolve(&o.geometry)?;
            // A flight with no count, no altitude or no distance is not
            // placeable; every shipped one authors all three.
            let count = o.num_objects.filter(|n| *n >= 1.0)? as u32;
            Some(ManifestSkyObject {
                geometry,
                count: count.min(MAX_SKY_OBJECTS),
                height: o.height?,
                distance: o.distance?,
                velocity_x: o.velocity_x,
                velocity_y: o.velocity_y,
                velocity_z: o.velocity_z,
                acceleration: o.acceleration,
                direction_factor: o.direction_factor,
                lifetime: o.lifetime,
            })
        })
        .collect();
    let dome = sky?.dome.as_ref()?;
    let models: Vec<ManifestSkyModel> = dome
        .models
        .iter()
        .filter_map(|m| {
            Some(ManifestSkyModel {
                geometry: resolve(&m.geometry)?,
                offset: m.offset.unwrap_or(0.0),
                movement_scale: m.movement_scale.unwrap_or(1.0),
                reflect: m.reflect,
                effects: Vec::new(),
                rotation: m.rotation,
            })
        })
        .collect();
    let lrt = dome.low_res_terrain.as_ref();
    let stars = dome.stars.as_ref().and_then(stars);
    if models.is_empty()
        && lrt.is_none()
        && objects.is_empty()
        && dome.terrain_enable.is_none()
        && stars.is_none()
    {
        return None;
    }
    Some(ManifestSky {
        texture: dome.texture.clone(),
        models,
        ambient: dome.ambient.map(dome_ambient).unwrap_or([1.0; 3]),
        low_res_terrain: lrt.and_then(|l| l.texture.clone()),
        low_res_max_distance: lrt.and_then(|l| l.max_distance),
        objects,
        terrain_enable: dome.terrain_enable,
        reflection_color: dome.reflection_color.map(reflection_tint),
        stars,
        glow_masks_preserved: true,
    })
}

/// `DomeInfo.Stars()` with the loaders' defaults (B1 `FUN_002e6a50`, B2
/// `FUN_003bfc30`; `analysis/re-stars.md` §1). `NumStars` absent or 0 draws
/// nothing on either game.
fn stars(st: &mungefs::sky::Stars) -> Option<ManifestStars> {
    let count = st.num_stars.filter(|n| *n >= 1.0)? as u32;
    let byte = |v: f32| v.clamp(0.0, 255.0) as u8;
    let mut color = [255u8; 4];
    if let Some(c) = st.color.as_ref().filter(|c| c.len() >= 3) {
        for (dst, v) in color.iter_mut().zip(c) {
            *dst = byte(*v);
        }
    }
    Some(ManifestStars {
        count,
        seed: st.random_seed.unwrap_or(0.0).max(0.0) as u32,
        twinkle_factor: st.twinkle_factor.unwrap_or(0.5),
        twinkle_frequency: st.twinkle_frequency.unwrap_or(20.0),
        color,
        bright_fraction: st.bright_star_percent.map_or(0.08, |p| p * 0.01),
        // The ELF scales by its own 1/255 constant (`* 0.003921569`).
        alpha_min: st.alpha_min.map_or(0.05, |a| a * 0.003_921_569),
        color_saturation: st.color_saturation.unwrap_or(1.0),
        enable_bottom: st.enable_bottom.is_some_and(|b| b != 0.0),
    })
}

/// `DomeInfo.ReflectionColor` as the GS takes it: BF1's loader clamps each
/// value to a byte (`FUN_002e29e0`), the draw halves it to GS scale,
/// `(c + 1) >> 1`, and 128 is unity (`analysis/re-sky-reflect.md` §1, §3
/// [V]). The GS multiplies in gamma space, so the factor is linearised for
/// the port's linear pipeline — 255 stays exactly 1.0.
fn reflection_tint(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| {
        let byte = v.clamp(0.0, 255.0) as u32;
        srgb_to_linear(((byte + 1) >> 1) as f32 / 128.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_object_count_preserves_authored_geonosis_flights() {
        let sky = mungefs::sky::Sky {
            dome: Some(Default::default()),
            objects: vec![mungefs::sky::SkyObject {
                geometry: "fighter".into(),
                num_objects: Some(20.0),
                height: Some([80.0, 140.0]),
                distance: Some(300.0),
                ..Default::default()
            }],
            ..Default::default()
        };
        let baked = sky_from(Some(&sky), |name| Some(name.into())).unwrap();
        assert_eq!(baked.objects[0].count, 20);
    }

    /// `DomeInfo.Ambient` only ever holds one of the two PS2 neutrals, so it
    /// never darkens a dome — but a real tint would still be applied.
    #[test]
    fn dome_ambient_treats_both_neutrals_as_unity() {
        // The four values the 34 ground maps actually author, as stored.
        assert_eq!(dome_ambient([128.00003; 3]), [1.0; 3]); // BF1/BF2 HOT1, GEO1
        assert_eq!(dome_ambient([255.0; 3]), [1.0; 3]);
        assert_eq!(dome_ambient([256.00003; 3]), [1.0; 3]); // BF1 KAS2, NAB1
                                                            // Anything else is applied, linearised like every other authored
                                                            // colour.
        assert_eq!(dome_ambient([64.0; 3]), scale255([64.0; 3]));
        assert_eq!(
            dome_ambient([128.0, 128.0, 200.0]),
            scale255([128.0, 128.0, 200.0])
        );
    }

    /// KAS1's (200, 200, 255) is the GS's (100, 100, 128) / 128; the
    /// default white is exactly unity.
    #[test]
    fn reflection_tint_is_the_gs_value_linearised() {
        let t = reflection_tint([200.0, 200.0, 255.0]);
        assert_eq!(t[2], 1.0);
        assert!((t[0] - srgb_to_linear(100.0 / 128.0)).abs() < 1e-6);
        assert_eq!(t[0], t[1]);
    }

    /// The transfer is the sRGB one, not a 2.2 power: white and black are
    /// fixed points and mid-grey lands at 0.216, not 0.502.
    #[test]
    fn authored_255_colors_linearise() {
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(128.0 / 255.0) - 0.2158).abs() < 1e-3);
    }
    use mungefs::light::{GlobalLights, Light};
    use mungefs::sky::{DomeInfo, LowResTerrain, SkyInfo, SunInfo};

    fn bf1_tat1_sky() -> Sky {
        Sky {
            info: SkyInfo {
                fog_color: Some([0.0, 0.0, 10.0]),
                fog_range: Some([0.0, 2700.0]),
                ambient_color: Some([80.0, 75.0, 100.0]),
                character_ambient_color: Some([100.0, 95.0, 120.0]),
                ..SkyInfo::default()
            },
            sun: Some(SunInfo {
                angle: Some([130.0, -55.0]),
                color: Some([242.0, 219.0, 95.0]),
                ..SunInfo::default()
            }),
            ..Sky::default()
        }
    }

    /// Data-gated: UTA1's three `lght` chunks hold 50 local lights
    /// (re-point-lights §1's census): 6 Overbright omnis, 11 red spots — one
    /// over each `uta1_prop_post`, which pins the z sign against the world
    /// instances — and 33 Overbright yellow spots.
    #[test]
    fn utapau_bakes_its_fifty_local_lights() {
        let lvl = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/UTA/UTA1.LVL"
        );
        let Ok(buf) = std::fs::read(lvl) else {
            eprintln!("SKIP: {lvl} not present");
            return;
        };
        let inner = mungefs::open(&buf).unwrap();
        let chunks = mungefs::content_chunks(inner).unwrap();
        let mut all = Vec::new();
        let mut posts: Vec<[f32; 3]> = Vec::new();
        for c in &chunks {
            match &c.tag {
                b"lght" if c.payload.len() > 12 => {
                    all.extend(point_lights_from(&Lights::parse(c).unwrap()));
                }
                b"wrld" => {
                    let w = mungefs::world::World::parse(c).unwrap();
                    posts.extend(
                        w.instances
                            .iter()
                            .filter(|i| i.class.eq_ignore_ascii_case("uta1_prop_post"))
                            .map(|i| i.transform.translation),
                    );
                }
                _ => {}
            }
        }
        let n = |spot: bool, ob: bool| {
            all.iter()
                .filter(|l| l.spot == spot && l.overbright == ob && l.is_static)
                .count()
        };
        assert_eq!(all.len(), 50);
        assert_eq!((n(false, true), n(true, false), n(true, true)), (6, 11, 33));
        assert_eq!(posts.len(), 11);
        // The 11 lamp spots are one-way and point down; a `Bidirectional(1)`
        // spot (15 of the 33 yellow ones) keeps no cone.
        assert_eq!(
            all.iter().filter(|l| l.spot && l.cone.is_none()).count(),
            15
        );
        for l in all.iter().filter(|l| l.spot && !l.overbright) {
            let c = l.cone.unwrap_or_else(|| panic!("{}: no cone", l.name));
            assert!(c.dir[1] < -0.5 && c.outer > 0.1 && c.outer < 3.2, "{c:?}");
        }
        for l in all.iter().filter(|l| l.spot && !l.overbright) {
            let d = posts
                .iter()
                .map(|p| (p[0] - l.pos[0]).hypot(p[2] - l.pos[2]))
                .fold(f32::INFINITY, f32::min);
            // The lamp head overhangs its post by up to 4.7 m; the mirrored
            // z would be 500 m away.
            assert!(
                d < 6.0,
                "{}: {:?} is {d} m from the nearest post",
                l.name,
                l.pos
            );
        }
    }

    /// BF1 has no `lght`, so TAT1's whole look comes out of `sky_`: a warm,
    /// low-ish sun 40 degrees up and a cool ambient.
    #[test]
    fn bf1_map_lights_from_sky_only() {
        let l = lighting_from(Some(&bf1_tat1_sky()), None, false);
        assert_eq!(l.source, "sky_");
        // The PS2's own vector (RAM 0x69c590 on TAT1, `mungefs::sky`).
        assert!((l.sun_dir[1] - 130.0f32.to_radians().cos()).abs() < 1e-5);
        assert!((l.sun_color[0] - srgb_to_linear(242.0 / 255.0)).abs() < 1e-6);
        assert_eq!(l.ambient_sky, l.ambient_ground);
        assert!((l.ambient_sky[2] - srgb_to_linear(100.0 / 255.0)).abs() < 1e-6);
        // TAT1's `SkyInfo` FogRange(0, 2700) is dead data — the BF1 ELF
        // never parses it — and this fixture authors no `LowResTerrain`, so
        // the map takes no fog here. Its real fog is in
        // `bf1_tat1_low_res_terrain_fog_stretches_far`.
        assert!(l.fog.is_none());
    }

    /// The two scene ranges are copied through in metres, both games, with
    /// no filter: `NearSceneRange[2]` is where the PS2's near scene (and so
    /// its heightfield) ends, `FarSceneRange` where it culls far-scene draws
    /// (`analysis/re-fog.md` §2.5 [V]). The values are BF1 TAT1's, pinned by
    /// `mungefs/tests/suite/tat1_lvl.rs` against the disc bytes.
    #[test]
    fn the_scene_ranges_are_copied_through_unfiltered() {
        let mut sky = bf1_tat1_sky();
        sky.info.near_scene_range = Some([1.0, 100.0, 150.0]);
        sky.info.far_scene_range = Some(2000.0);
        let l = lighting_from(Some(&sky), None, false);
        assert_eq!(l.near_scene_range, Some([1.0, 100.0, 150.0]));
        assert_eq!(l.far_scene_range, Some(2000.0));

        // `FarSceneRange(0)` is AUTHORED on ten of re-fog.md §6's maps —
        // BF1 END1/RHN2/YAV1, BF2 DAG1/DEA1/END1/FEL1/NAB2/TAT3/YAV1, nine of
        // them among the 32 baked ones (TAT3 is not baked) — and means
        // "no far layer", which is not the same as an absent key: both
        // survive distinguishably.
        sky.info.far_scene_range = Some(0.0);
        assert_eq!(
            lighting_from(Some(&sky), None, false).far_scene_range,
            Some(0.0)
        );
        sky.info.far_scene_range = None;
        sky.info.near_scene_range = None;
        let l = lighting_from(Some(&sky), None, false);
        assert_eq!(l.far_scene_range, None);
        assert_eq!(l.near_scene_range, None);
        // A map with no `sky_` at all takes neither.
        let l = lighting_from(None, None, true);
        assert_eq!((l.near_scene_range, l.far_scene_range), (None, None));
    }

    /// A BF2 map prefers its `lght`: the key light's quaternion and colour,
    /// and the hemisphere ambient from GlobalLights.
    #[test]
    fn bf2_map_prefers_lght() {
        let lights = Lights {
            lights: vec![Light {
                name: "sun".into(),
                rotation: Some([0.672_421, 0.538_74, 0.083_629, -0.500_614]),
                color: Some([1.0, 1.0, 1.0]),
                ..Light::default()
            }],
            global: Some(GlobalLights {
                light1: Some("Sun".into()),
                top: Some([90.0, 90.0, 90.0]),
                bottom: Some([120.0, 120.0, 120.0]),
                ..GlobalLights::default()
            }),
            unknown: Vec::new(),
        };
        // Read as a BF2 map the fixture's FogRange(0, 2700) IS fog
        // (`near < far` is BF2's whole enable), which would list `sky_` as a
        // source; drop it so this test stays about the LIGHT preference.
        let mut sky = bf1_tat1_sky();
        sky.info.fog_range = None;
        let l = lighting_from(Some(&sky), Some(&lights), true);
        // Sun and ambient both come from `lght`, so `sky_` contributes
        // nothing here.
        assert_eq!(l.source, "lght");
        assert!(l.fog.is_none());
        // BF2 TAT2's sun: Light1's row 2 of (−a3, a0, −a1, a2).
        assert!(l.sun_dir[1] < -0.7, "{:?}", l.sun_dir);
        assert!((l.ambient_sky[0] - srgb_to_linear(90.0 / 255.0)).abs() < 1e-6);
        assert!((l.ambient_ground[0] - srgb_to_linear(120.0 / 255.0)).abs() < 1e-6);
    }

    // ------------------------------------------------------------------
    // THE FOG RULE (`fog_from`), pinned to `analysis/re-fog.md` §6's per-map
    // table — the disc bytes put through the ELF's own rule. Every number
    // below is a value one of the 34 ground maps actually authors.
    // ------------------------------------------------------------------

    /// A BF1 sky whose fog lives where the BF1 ELF actually reads it:
    /// `DomeInfo.LowResTerrain`. `alpha` is the `FogColor` 4th component.
    fn bf1_lrt(color: [f32; 3], near: f32, far: f32, alpha: f32) -> Sky {
        let mut s = bf1_tat1_sky();
        s.dome = Some(DomeInfo {
            low_res_terrain: Some(LowResTerrain {
                texture: Some("hoth".into()),
                max_distance: Some(1000.0),
                texture_scale: None,
                fog_near: Some(near),
                fog_far: Some(far),
                fog_color: Some([color[0], color[1], color[2], alpha]),
            }),
            ..DomeInfo::default()
        });
        s
    }

    /// The frame filter bakes only while `Filter` ≠ 0 and `Intensity` ≥ 1
    /// (TAT1 50/50; HOT1 authors Intensity 0; BF2 COR1 Filter 0).
    #[test]
    fn the_dome_filter_bakes_only_where_it_draws() {
        let with = |f: Option<f32>, t: Option<f32>, i: Option<f32>| {
            let mut s = bf1_tat1_sky();
            s.dome = Some(DomeInfo {
                filter: f,
                threshold: t,
                intensity: i,
                ..DomeInfo::default()
            });
            lighting_from(Some(&s), None, false).filter
        };
        assert_eq!(
            with(Some(1.0), Some(50.0), Some(50.0)),
            Some(ManifestFilter {
                threshold: 50.0,
                intensity: 50.0
            })
        );
        assert_eq!(with(Some(1.0), Some(0.0), Some(0.0)), None, "HOT1");
        assert_eq!(with(Some(0.0), Some(150.0), Some(50.0)), None);
        assert_eq!(with(None, Some(150.0), Some(50.0)), None);
    }

    fn bf1_fog(sky: &Sky) -> Option<ManifestFog> {
        lighting_from(Some(sky), None, false).fog
    }

    fn bf2_fog(color: Option<[f32; 3]>, range: Option<[f32; 2]>) -> Option<ManifestFog> {
        let mut s = bf1_tat1_sky();
        s.info.fog_color = color;
        s.info.fog_range = range;
        lighting_from(Some(&s), None, true).fog
    }

    /// The colour is the authored 0..255 linearised, exactly as before.
    fn assert_color(fog: &ManifestFog, c: [f32; 3]) {
        for k in 0..3 {
            assert!(
                (fog.color[k] - srgb_to_linear(c[k] / 255.0)).abs() < 1e-6,
                "channel {k}: {:?} != {c:?}",
                fog.color
            );
        }
    }

    /// BF1 HOT1: `LowResTerrain` 203,201,203 / 100 / 1000 with alpha 255 —
    /// no stretch. §6, and the map that has to match BF2 HOT1 exactly.
    #[test]
    fn bf1_hot1_low_res_terrain_fog() {
        let fog =
            bf1_fog(&bf1_lrt([203.0, 201.0, 203.0], 100.0, 1000.0, 255.0)).expect("BF1 Hoth fogs");
        assert_eq!((fog.near, fog.far), (100.0, 1000.0));
        assert_color(&fog, [203.0, 201.0, 203.0]);
        // ...and BF2 HOT1's own SkyInfo pair reduces to the same numbers.
        let bf2 =
            bf2_fog(Some([203.0, 201.0, 203.0]), Some([100.0, 1000.0])).expect("BF2 Hoth fogs");
        assert_eq!((bf2.near, bf2.far), (fog.near, fog.far));
        assert_eq!(bf2.color, fog.color);
    }

    /// BF1 KAM1: black is real fog, and alpha 160 stretches 400..1000 to
    /// 400..1356.25 — which is what BF2 KAM1 authors outright (§4). The
    /// cross-disc agreement is the evidence the stretch formula is right.
    #[test]
    fn bf1_kam1_black_fog_stretches_to_bf2s_authored_far() {
        let fog = bf1_fog(&bf1_lrt([0.0, 0.0, 0.0], 400.0, 1000.0, 160.0)).expect("Kamino fogs");
        assert_eq!((fog.near, fog.far), (400.0, 1356.25));
        assert_eq!(fog.color, [0.0, 0.0, 0.0]);
        // BF2 KAM1 as shipped: the same ramp to the integer the disc stores.
        let bf2 = bf2_fog(Some([0.0, 0.0, 0.0]), Some([400.0, 1356.0])).expect("BF2 Kamino fogs");
        assert_eq!((bf2.near, bf2.far), (400.0, 1356.0));
    }

    /// BF1 KAS2 — the map that turned "bright red" in the session-4
    /// experiment (`engine/artifacts/fog_rule/README.md`). Its 255,0,0 IS
    /// what the disc authors and what the PS2 draws; alpha 128 pushes the far
    /// plane out to 2992.1875 m, so it is a faint red horizon at the 1500 m
    /// cull, not a red screen. The far value is the exact float, not 2992.
    #[test]
    fn bf1_kas2_red_fog_is_real_and_the_alpha_pushes_it_far_out() {
        let fog = bf1_fog(&bf1_lrt([255.0, 0.0, 0.0], 1000.0, 2000.0, 128.0)).expect("KAS2 fogs");
        assert_eq!((fog.near, fog.far), (1000.0, 2992.1875));
        assert_color(&fog, [255.0, 0.0, 0.0]);
    }

    /// BF1 TAT1: 164,174,194 / 400 / 3500 with alpha 80 → 10281.25 m. The
    /// `SkyInfo` 0,0,10 that used to make Tatooine black is in the same
    /// fixture and is ignored.
    #[test]
    fn bf1_tat1_low_res_terrain_fog_stretches_far() {
        let sky = bf1_lrt([164.0, 174.0, 194.0], 400.0, 3500.0, 80.0);
        assert_eq!(sky.info.fog_color, Some([0.0, 0.0, 10.0])); // the dead field
        let fog = bf1_fog(&sky).expect("Tatooine fogs");
        assert_eq!((fog.near, fog.far), (400.0, 10281.25));
        assert_color(&fog, [164.0, 174.0, 194.0]);
    }

    /// BF1 NAB2 authors alpha 0, which is NOT a density: 0 and 255 both leave
    /// `FogFar` as written (decomp 270621, `(a - 1) & 0xff < 0xfe`).
    #[test]
    fn bf1_nab2_alpha_zero_does_not_stretch() {
        let fog = bf1_fog(&bf1_lrt([70.0, 70.0, 70.0], 3000.0, 6000.0, 0.0)).expect("Naboo fogs");
        assert_eq!((fog.near, fog.far), (3000.0, 6000.0));
    }

    /// BF1 RHN1 and RHN2 author no `LowResTerrain` scope at all, so they have
    /// no fog — the only two BF1 ground maps without it.
    #[test]
    fn bf1_rhn1_has_no_low_res_terrain_and_no_fog() {
        let mut s = bf1_tat1_sky();
        s.dome = Some(DomeInfo {
            texture: Some("rhn_sky".into()),
            ..DomeInfo::default()
        });
        assert!(bf1_fog(&s).is_none());
        // Nor does a scope that omits one of the two distances.
        let mut half = bf1_lrt([128.0, 128.0, 128.0], 100.0, 1000.0, 255.0);
        half.dome
            .as_mut()
            .unwrap()
            .low_res_terrain
            .as_mut()
            .unwrap()
            .fog_far = None;
        assert!(bf1_fog(&half).is_none());
    }

    /// **The rule that retires the placeholder heuristics.** A BF1 map's
    /// `SkyInfo.FogColor` / `FogRange` are never read: hash 0x023b1fce is
    /// compared exactly once in the BF1 decomp and it is inside the
    /// `LowResTerrain` scope. Ten of the sixteen BF1 maps ship
    /// `FogRange(0, 2700..30000)` there and it is dead data.
    #[test]
    fn bf1_never_reads_sky_info_fog() {
        let mut s = bf1_tat1_sky();
        s.info.fog_color = Some([128.0, 128.0, 128.0]);
        s.info.fog_range = Some([0.0, 3000.0]);
        s.dome = None;
        assert!(bf1_fog(&s).is_none(), "BF1 must ignore SkyInfo fog");
        // The same sky read as BF2 DOES fog: near < far is BF2's whole enable.
        assert!(lighting_from(Some(&s), None, true).fog.is_some());
    }

    /// BF2 GEO1 ships `FogRange(0, 0)`: `near < far` fails, so fog is off.
    /// KAS2 and POL1 are the same shape.
    #[test]
    fn bf2_zero_range_is_off() {
        assert!(bf2_fog(Some([0.0, 0.0, 0.0]), Some([0.0, 0.0])).is_none());
        assert!(bf2_fog(Some([180.0, 180.0, 200.0]), Some([0.0, 0.0])).is_none());
        // POL1
    }

    /// BF2 DAG1 and MUS1 author a `FogColor` and no `FogRange`: the colour is
    /// stored, the enable flag is never set, fog is off.
    #[test]
    fn bf2_color_without_range_is_off() {
        assert!(bf2_fog(Some([95.0, 111.0, 126.0]), None).is_none()); // DAG1
        assert!(bf2_fog(Some([198.0, 12.0, 8.0]), None).is_none()); // MUS1
    }

    /// BF2 FEL1 ships -10..62. `near < far`, so it fogs; the negative near is
    /// clamped to the eye (ours, unchanged from the previous rule).
    /// Its `FogColor` carries a 4th component of 128, which the ELF forces to
    /// 0xff before the store and which `mungefs` does not even keep — the
    /// BF2 alpha is not a density and cannot reach here.
    #[test]
    fn bf2_fel1_negative_near_is_clamped_and_the_alpha_is_ignored() {
        let fog = bf2_fog(Some([80.0, 100.0, 80.0]), Some([-10.0, 62.0])).expect("Felucia fogs");
        assert_eq!((fog.near, fog.far), (0.0, 62.0));
        assert_color(&fog, [80.0, 100.0, 80.0]);
    }

    /// A BF2 map's `LowResTerrain` fog keys are never read either — BF2's
    /// `LowResTerrain` handler parses no fog at all (§1) — so a BF2 map with
    /// both takes the `SkyInfo` pair and nothing of the other.
    #[test]
    fn bf2_ignores_low_res_terrain_fog() {
        let mut s = bf1_lrt([255.0, 0.0, 0.0], 10.0, 20.0, 128.0);
        s.info.fog_color = Some([203.0, 201.0, 203.0]);
        s.info.fog_range = Some([100.0, 1000.0]);
        let fog = lighting_from(Some(&s), None, true)
            .fog
            .expect("BF2 fogs from SkyInfo");
        assert_eq!((fog.near, fog.far), (100.0, 1000.0));
        assert_color(&fog, [203.0, 201.0, 203.0]);
    }

    /// A `far <= near` triple is refused. The BF1 ELF has no such test (it
    /// just writes the rows), but a degenerate ramp is meaningless to the
    /// shader; no disc value reaches this.
    #[test]
    fn degenerate_fog_range_is_dropped() {
        assert!(bf2_fog(Some([128.0; 3]), Some([500.0, 500.0])).is_none());
        assert!(bf1_fog(&bf1_lrt([128.0; 3], 500.0, 400.0, 255.0)).is_none());
    }

    /// A map's ambient is the authored one, black included: COR1 stores
    /// GlobalLights Top / Bottom 0/0/0 and the PS2 holds 0 in RAM
    /// (`analysis/re-kamino-decks.md` §3). The port once floored it at
    /// 60/255 (card LSUM removed the floor).
    #[test]
    fn a_black_ambient_ships_black() {
        let lights = Lights {
            lights: vec![Light {
                name: "sun".into(),
                rotation: Some([1.0, 0.0, 0.0, 0.0]),
                color: Some([1.0, 1.0, 1.0]),
                ..Light::default()
            }],
            global: Some(GlobalLights {
                light1: Some("sun".into()),
                top: Some([0.0, 0.0, 0.0]),
                bottom: Some([0.0, 0.0, 0.0]),
                ..GlobalLights::default()
            }),
            unknown: Vec::new(),
        };
        let l = lighting_from(None, Some(&lights), true);
        assert_eq!(l.ambient_sky, [0.0; 3]);
        assert_eq!(l.ambient_ground, [0.0; 3]);
    }

    /// BF2's `lght` (queue LITEB, `analysis/re-model-lighting.md` §0/§3):
    /// the key light's `Color` is gamma-space and linearised; its `Static()`
    /// reaches the manifest; `Light2` becomes a second directional light
    /// only when it names a type-1 light, and carries its own `Static()`.
    #[test]
    fn bf2_lght_carries_light2_static_and_a_linear_sun() {
        let sun = Light {
            name: "sun".into(),
            kind: Some(1.0),
            rotation: Some([1.0, 0.0, 0.0, 0.0]),
            color: Some([0.5, 0.5, 0.5]),
            is_static: true,
            ..Light::default()
        };
        let back = Light {
            name: "Back".into(),
            kind: Some(1.0),
            // a2 = 1 is q = identity: row 2 = (0, 0, 1).
            rotation: Some([0.0, 0.0, 1.0, 0.0]),
            color: Some([1.0, 0.5, 0.0]),
            ..Light::default()
        };
        let lights = |l2: &str, extra: Vec<Light>| Lights {
            lights: [vec![sun.clone(), back.clone()], extra].concat(),
            global: Some(GlobalLights {
                light1: Some("sun".into()),
                light2: Some(l2.into()),
                top: Some([10.0; 3]),
                bottom: Some([10.0; 3]),
            }),
            unknown: Vec::new(),
        };
        let l = lighting_from(None, Some(&lights("back", vec![])), true);
        assert!((l.sun_color[0] - 0.2140).abs() < 1e-3, "{:?}", l.sun_color);
        assert!(l.sun_static);
        let l2 = l.light2.expect("Light2 names a type-1 light");
        assert!((l2.dir[2] - 1.0).abs() < 1e-5, "travels +z: {:?}", l2.dir);
        // a0 = 1 is q = (0, 1, 0, 0), 180 degrees about y: row 2 = (0, 0, −1).
        assert!(
            (l.sun_dir[2] + 1.0).abs() < 1e-5,
            "travels -z: {:?}",
            l.sun_dir
        );
        assert_eq!(l2.color[0], 1.0);
        assert!((l2.color[1] - 0.2140).abs() < 1e-3);
        assert!(!l2.is_static);
        // An empty Light2, or one naming a non-directional light: none.
        assert!(lighting_from(None, Some(&lights("", vec![])), true)
            .light2
            .is_none());
        let spot = Light {
            name: "lamp".into(),
            kind: Some(3.0),
            ..back.clone()
        };
        assert!(lighting_from(None, Some(&lights("lamp", vec![spot])), true)
            .light2
            .is_none());
        // BF1 has no `lght`: nothing is Static and there is no Light2.
        let b1 = lighting_from(Some(&bf1_tat1_sky()), None, false);
        assert!(!b1.sun_static && b1.light2.is_none());
    }

    /// A map that authors nothing keeps exactly the old hardcoded look, so
    /// "no data" can never be mistaken for "a dark map".
    #[test]
    fn empty_sky_falls_back_to_the_old_default() {
        let l = lighting_from(None, None, false);
        assert_eq!(l.source, "default");
        assert_eq!(l.sun_dir, DEFAULT_SUN_DIR);
        assert_eq!(l.ambient_sky, DEFAULT_AMBIENT);
        assert!(l.fog.is_none());
    }
}
