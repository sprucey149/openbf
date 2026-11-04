//! The map's light, as the renderer holds it: one directional sun, a second
//! directional light where the map authors one ([`DirLight`]) and a
//! hemisphere ambient ([`Lighting`]), the world's fog ([`Fog`]) and, on
//! a map that draws no far layer, the near scene's fade into the sky
//! ([`FarFade`]). `game::viewer::apply_lighting` sets it per map; `render_frame`
//! (`frame.rs`) uploads it in the shader's globals every frame. Also the
//! sRGB transfer pair (`encode` / `decode`) and [`light_sum`], the CPU
//! twin of the shader's light sum in GS units (card LSUM).

// Only for `Lighting::water_fog_begin`'s docs, which link `DrawLayer::Water`.
#[cfg(doc)]
use crate::DrawLayer;

pub(crate) const CHARACTER_LIGHT_REGION: u32 = 0x200;

impl crate::Renderer {
    pub fn set_character_light_regions(&mut self, regions: Vec<CharacterLightRegion>) {
        self.character_light_regions = regions;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterLightRegion {
    pub centre: glam::Vec3,
    pub axes: glam::Mat3,
    pub volume: CharacterLightVolume,
    pub top: [f32; 3],
    pub bottom: [f32; 3],
    pub directional: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CharacterLightVolume {
    Box(glam::Vec3),
    Sphere(f32),
    Cylinder { radius: f32, half_height: f32 },
}

impl CharacterLightRegion {
    fn weight(self, sphere: crate::LitSphere) -> f32 {
        let delta = sphere.centre - self.centre;
        let distance = match self.volume {
            CharacterLightVolume::Sphere(radius) => delta.length() - radius,
            CharacterLightVolume::Box(half_extent) => {
                let local = self.axes.transpose() * delta;
                boundary_distance(local.abs() - half_extent)
            }
            CharacterLightVolume::Cylinder {
                radius,
                half_height,
            } => {
                let local = self.axes.transpose() * delta;
                let radial = glam::Vec2::new(local.x, local.z).length() - radius;
                let outside = glam::Vec2::new(radial, local.y.abs() - half_height);
                outside.max(glam::Vec2::ZERO).length() + outside.max_element().min(0.0)
            }
        };
        if sphere.radius <= 0.0 {
            return f32::from(u8::from(distance < 0.0));
        }
        (0.5 - 0.5 * distance / sphere.radius).clamp(0.0, 1.0)
    }
}

fn boundary_distance(outside: glam::Vec3) -> f32 {
    outside.max(glam::Vec3::ZERO).length() + outside.max_element().min(0.0)
}

pub(crate) fn character_region_light(
    regions: &[CharacterLightRegion],
    light: &Lighting,
    sphere: crate::LitSphere,
) -> Option<[[f32; 4]; 2]> {
    let mut sum = 0.0;
    let mut count = 0;
    let mut result = [[0.0; 4]; 2];
    for region in regions {
        let weight = region.weight(sphere);
        if weight <= 0.0 {
            continue;
        }
        sum += weight;
        count += 1;
        for (i, rgb) in [region.top, region.bottom].into_iter().enumerate() {
            for (dst, c) in result[i][..3].iter_mut().zip(rgb) {
                *dst += weight * c;
            }
            result[i][3] += weight * region.directional[i];
        }
        if count == 4 {
            break;
        }
    }
    if count == 0 {
        return None;
    }
    let global = if count == 1 || sum <= (count - 1) as f32 * 0.9 {
        count as f32 - sum
    } else {
        0.0
    };
    for (i, rgb) in [light.ambient_sky, light.ambient_ground]
        .into_iter()
        .enumerate()
    {
        for (dst, c) in result[i][..3].iter_mut().zip(rgb) {
            *dst = (*dst + global * encode(c)) / (sum + global);
        }
        result[i][3] = (result[i][3] + global) / (sum + global);
    }
    Some(result)
}

/// The world's fog. `color` is linear RGB; `near`/`far` are metres of PLANAR
/// view depth. The shader's `apply_fog` takes the PS2's amount, the
/// `fogspread` table at f = (depth - near) / (far - near), and lerps on
/// sRGB-encoded values (`analysis/re-fog-amount.md` §8): nothing is fogged
/// nearer than `near`, and even past `far` the amount stops at 248/255.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    pub color: [f32; 3],
    pub near: f32,
    pub far: f32,
}

/// A distance fade band in metres from the eye, also used by [`crate::Beam::view`].
///
/// For [`Lighting::far_fade`], this fades the near scene into the sky on a map
/// that draws NO far layer. `begin`/`end` are `NearSceneRange`'s b and c: the
/// PS2 composites its near-scene buffer over the far-scene buffer through a
/// depth-keyed alpha palette — alpha 0 nearer than b, a ramp b..c, fully the
/// far buffer past c (`analysis/re-fog.md` §2.5 [V code, D look]). A map that
/// authors `FarSceneRange(0)` draws nothing INTO that far buffer, so what the
/// near scene fades into is the sky, and the authored fog colour never
/// appears — which is why `game::viewer::apply_lighting` sets this INSTEAD of
/// [`Fog`], never beside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarFade {
    pub begin: f32,
    pub end: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterDiffuse {
    pub origin: [f32; 2],
    pub step: [f32; 2],
    pub dims: [u32; 2],
    pub range: [f32; 2],
    pub colors: [[u32; 4]; 16],
}

/// The flat water's per-map colour constants — the PS2's VU1 water overlay's
/// constant block (`analysis/re-water-colour.md` §1.4 `[O]`), consumed by
/// `fs_water` (routine B, the view-angle term) and `fs_speckle` (routine A,
/// the sun lobe). Colours are in GS terms, the authored byte / 255 (gamma
/// space; the shader adds them there and linearises the sum). The default is
/// all zeros: no view-angle term and a black speckle — what a dry map, an
/// ocean (whose path never loads the overlay, §1.9) and a pre-`fx__` bake get.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterShading {
    pub far_diffuse: WaterDiffuse,
    pub far_range: [f32; 2],
    /// `10·ĥ` in metres: routine B's view vector runs from the fragment to
    /// `eye − eye_shift` (VU[146], §1.4).
    pub eye_shift: glam::Vec3,
    /// `SpecularColor` / 255, rgba: the view-angle term's weight in colour
    /// and alpha (VU[148]).
    pub specular: [f32; 4],
    /// `SpeckleAmbientColor` / 255 and `SpeckleSpecularColor` / 255, rgba
    /// (VU[149], VU[150]).
    pub speckle_ambient: [f32; 4],
    pub speckle_specular: [f32; 4],
    /// `(b4, −b8, b4)` from `LightAzimAndElev` (VU[144].xyz).
    pub speckle_axis: glam::Vec3,
    /// The lobe's light direction `l̂` (VU[144..146].w).
    pub speckle_light: glam::Vec3,
    /// `(ĥx·SCS.x, 0, ĥz·SCS.y)`, `SCS` = `SpeckleCoordShift` (VU[145]).
    pub speckle_shift: glam::Vec3,
}

/// A directional light beside the sun ([`Lighting::light2`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirLight {
    /// The direction the light TRAVELS, normalized by the setter side.
    pub dir: glam::Vec3,
    /// Linear RGB.
    pub color: [f32; 3],
    /// `Static()` authored: skipped by a [`crate::LightPath::Prelit`] /
    /// [`crate::LightPath::PrelitAdd`] draw.
    pub is_static: bool,
}

/// sRGB encode of one linear channel (the exact curve, the shader's
/// `linear_to_srgb`); a value past 1 keeps the power law.
pub fn encode(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB decode of one display channel (the shader's `srgb_to_linear`).
pub fn decode(d: f32) -> f32 {
    if d <= 0.040_45 {
        d / 12.92
    } else {
        ((d + 0.055) / 1.055).powf(2.4)
    }
}

/// The CPU twin of the world shader's light sum (`shade_unfogged` /
/// `gs_lit` in `shader.wgsl`; card LSUM, `analysis/re-kamino-decks.md` §2
/// `[O]`). It is not compiled from the shader: the GPU referee
/// `light_path::the_lit_sum_is_made_in_display_units` is what holds the two
/// together. The PS2's VU1 adds the ambient, the directional lights and the
/// local lights in GS (display) units, clamps at 255/128, and the GS
/// multiplies the sum into the texel once.
///
/// `texel` is linear; each `terms` entry is a LINEAR light colour (as
/// [`Lighting`] holds them) and its weight (n.l, d⁴, the hemisphere blend);
/// `point` is the local lights' sum, already in GS units. Returns the linear
/// colour the shader writes.
pub fn light_sum(texel: f32, terms: &[(f32, f32)], point: f32) -> f32 {
    let sum: f32 = terms.iter().map(|&(c, w)| encode(c) * w).sum::<f32>() + point;
    texel * decode(sum.clamp(0.0, 255.0 / 128.0))
}

/// One directional sun plus a hemisphere ambient, and optional distance fog.
/// `sun_dir` is the direction the light TRAVELS (sun -> scene), normalized by
/// the setter side (kept as-is here).
///
/// The ambient is a hemisphere: `ambient_sky` lights surfaces facing up,
/// `ambient_ground` those facing down, interpolated by the world normal's Y.
/// The maps author exactly this pair (`lght` `GlobalLights.Top`/`Bottom`);
/// a map that authors a single ambient sets both to the same colour, which
/// reduces to the flat ambient this used to be.
#[derive(Debug, Clone, Copy)]
pub struct Lighting {
    pub sun_dir: glam::Vec3,
    pub sun_color: [f32; 3],
    /// A prelit draw skips the sun ([`crate::LightPath`]): BF2's sun authors
    /// `Static()` (`analysis/re-model-lighting.md` §3 `[O]`), and BF1's
    /// prelit (0x40) light set carries no sun at all
    /// (`analysis/re-sun-direction.md` §1.10 `[O]`).
    pub sun_static: bool,
    /// The sun's term is BF1's VU1 form `((1 + n·l)/2)⁴` rather than BF2's
    /// `max(0, n·l)` (`analysis/re-sun-direction.md` §1.8 `[O]`).
    pub sun_d4: bool,
    /// The PS2's second directional light (BF2 `GlobalLights.Light2`), or
    /// `None` — every BF1 map and most BF2 ones.
    pub light2: Option<DirLight>,
    pub ambient_sky: [f32; 3],
    pub ambient_ground: [f32; 3],
    /// The flat ambient a [`crate::LightPath::Character`] draw takes in place
    /// of the hemisphere, or `None` to light it as `Lit`: BF1's
    /// `SkyInfo.CharacterAmbientColor` (`analysis/re-model-lighting.md` §12,
    /// `[O]`). BF2 has one ambient for every draw (§3) and leaves it `None`.
    pub character_ambient: Option<[f32; 3]>,
    pub fog: Option<Fog>,
    /// Set INSTEAD of `fog` on a map with no far layer — see [`FarFade`].
    pub far_fade: Option<FarFade>,
    /// Metres from the eye past which a [`DrawLayer::Water`] draw takes the
    /// [`Fog`] ramp, or `None` when this map's water is never fogged at all.
    ///
    /// The PS2 never fogs its NEAR water in either game — the near branch of
    /// the water renderer loads no `fogspread` (`analysis/re-water.md` §1
    /// [V]) — and fogs the far pass in BF1 always and in BF2 under
    /// `SpecularEnable`, which is on unless a map authors 0 and no disc map
    /// does (`analysis/re-water-flags.md` §1–§5 `[O]`). So the water cannot
    /// share the world's fog gate: `game::viewer::water_look` decides the
    /// crossover per map and this carries the one distance. `None` and "the
    /// map has no fog" are different states — a map can fog its world and
    /// never its water, as every `DisableLowRes` map does.
    pub water_fog_begin: Option<f32>,
    pub water_fog: Option<Fog>,
    pub water_gain: f32,
    /// The weight of the sun's diffuse term on a [`DrawLayer::Water`] draw:
    /// the water's colour is scaled by `sun byte / 128 · max(n·l, 0) · this`
    /// in GS terms. **0 = the water is unlit**, every map's but BF2's ocean
    /// (`game::viewer::water_look` decides it; card KSEAL).
    pub water_lit: f32,
    /// A lit water draw's loss of colour with distance, `[begin, rate]`: past
    /// `begin` metres of planar view depth its GS colour is scaled by
    /// `1 − rate · (depth − begin)`, floored at 0, before the fog. `None` on
    /// every map but BF2's ocean (`game::viewer::water_look`; card KSEAL).
    pub water_dim: Option<[f32; 2]>,
    /// The flat water's colour constants ([`WaterShading`]).
    pub water: WaterShading,
    /// A factor on BF2's self-lit display gain ([`crate::LightPath::Glow`]
    /// and [`crate::LightPath::Unscaled`], drawn and gathered for bloom
    /// alike). 1 is the disc's gain; `game::viewer::look`'s per-map tunes
    /// are the only callers that set another ([ours]).
    pub glow_gain: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            // Mid-afternoon sun from behind-right of the default camera —
            // the fallback for a map that authors no lighting of its own.
            sun_dir: glam::Vec3::new(-0.4, -0.8, -0.45).normalize(),
            sun_color: [1.0, 1.0, 0.95],
            sun_static: false,
            sun_d4: false,
            light2: None,
            ambient_sky: [0.25, 0.25, 0.28],
            ambient_ground: [0.25, 0.25, 0.28],
            character_ambient: None,
            fog: None,
            far_fade: None,
            water_fog_begin: None,
            water_fog: None,
            water_gain: 1.0,
            water_lit: 0.0,
            water_dim: None,
            water: WaterShading::default(),
            glow_gain: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_light_region_blends_across_the_sphere_radius() {
        let mut region = CharacterLightRegion {
            centre: glam::Vec3::ZERO,
            axes: glam::Mat3::IDENTITY,
            volume: CharacterLightVolume::Box(glam::Vec3::splat(2.0)),
            top: [0.5; 3],
            bottom: [0.1; 3],
            directional: [0.0; 2],
        };
        let light = Lighting {
            ambient_sky: [0.0; 3],
            ambient_ground: [0.0; 3],
            ..Lighting::default()
        };
        let at = |region, centre| {
            character_region_light(
                &[region],
                &light,
                crate::LitSphere {
                    centre,
                    radius: 1.0,
                },
            )
        };
        let full = Some([[0.5, 0.5, 0.5, 0.0], [0.1, 0.1, 0.1, 0.0]]);
        let half = Some([[0.25, 0.25, 0.25, 0.5], [0.05, 0.05, 0.05, 0.5]]);
        for volume in [
            CharacterLightVolume::Box(glam::Vec3::splat(2.0)),
            CharacterLightVolume::Sphere(2.0),
            CharacterLightVolume::Cylinder {
                radius: 2.0,
                half_height: 2.0,
            },
        ] {
            region.volume = volume;
            assert_eq!(at(region, glam::Vec3::ZERO), full);
            assert_eq!(at(region, glam::Vec3::X * 2.0), half);
            assert_eq!(at(region, glam::Vec3::Y * 2.0), half);
            assert_eq!(at(region, glam::Vec3::X * 3.0), None);
        }
        assert_eq!(at(region, glam::Vec3::new(2.75, 2.75, 0.0)), None);
    }

    /// Kamino's deck (BF2 KAM1: ambient 100/255, sun 105/255 x n.l 0.906,
    /// blue channel): the display-unit sum is A + S x n.l of the texel's
    /// display value to a level, and the linear sum the port made before
    /// card LSUM is 1.4 x darker.
    #[test]
    fn the_sum_is_made_in_display_units() {
        let (a, s, nl) = (100.0 / 255.0, 105.0 / 255.0, 0.906);
        let t = decode(0.5);
        let got = light_sum(t, &[(decode(a), 1.0), (decode(s), nl)], 0.0);
        // A pure power law would make this exact; the sRGB toe leaves 2 %.
        let want = 0.5 * (a + s * nl);
        assert!(
            (encode(got) - want).abs() < 0.01,
            "{} vs {want}",
            encode(got)
        );
        let linear = t * (decode(a) + decode(s) * nl);
        assert!(got / linear > 1.35, "{got} vs the linear sum {linear}");
        // One term alone is the product either way.
        let one = light_sum(t, &[(decode(s), 1.0)], 0.0);
        assert!((one - t * decode(s)).abs() < 1e-6);
    }

    /// The VU clamps the sum at 255/128 before the GS multiplies.
    #[test]
    fn the_sum_clamps_at_the_gs_maximum() {
        let over = light_sum(0.25, &[(1.0, 1.0), (1.0, 1.0)], 1.0);
        assert_eq!(over, 0.25 * decode(255.0 / 128.0));
    }
}
