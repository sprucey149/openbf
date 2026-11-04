//! The environment schema: weather, local lights and HDR, foliage and the
//! `foliage.bin` codec, water, lighting, fog and sky. Each type cites its own
//! `analysis/` doc (`re-water.md`, `re-fog.md`, `re-foliage.md`, ...).

use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::Manifest;
use crate::ManifestInstance;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestFogCloud {
    pub texture: Option<String>,
    pub range: Option<[f32; 2]>,
    pub color: Option<[f32; 4]>,
    pub velocity: Option<[f32; 2]>,
    pub rotation: Option<f32>,
    pub height: Option<f32>,
    pub particle_size: Option<f32>,
    pub particle_density: Option<f32>,
}

/// The Precipitation effect's authored numbers
/// (`mungefs::fx::PrecipitationConfig`; the ELF ctor's defaults where a map
/// leaves a key out). `game::weather` runs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPrecipitation {
    /// `Type("Streaks")`: lines (rain); else textured squares (flakes).
    pub streaks: bool,
    /// `Texture`: the flake's sheet.
    pub texture: Option<String>,
    pub particle_size: f32,
    pub range: f32,
    /// `Color`, 0..255 RGBA.
    pub color: [u8; 4],
    pub alpha_min_max: [f32; 2],
    /// `ParticleDensity`: the particle count of the one tiled cube.
    pub particle_density: f32,
    pub particle_density_range: f32,
    /// Fall speed, m/s, down positive.
    pub velocity: f32,
    pub velocity_range: f32,
    /// `StreakLength` as authored (the PS2 stores it / 30, seconds).
    pub streak_length: f32,
    pub camera_cross_velocity_scale: f32,
    pub camera_axial_velocity_scale: f32,
    pub rotation: f32,
    pub rotation_range: f32,
    /// `GroundEffect`: the splash's particle-effect name.
    pub ground_effect: Option<String>,
    pub ground_effects_per_sec: f32,
    pub ground_effect_spread: f32,
}

/// The Wind effect's authored numbers (`mungefs::fx::WindConfig`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestWind {
    /// `Velocity(a, b)`: the base vector is (a, 0, b), m/s.
    pub velocity: [f32; 2],
    pub velocity_range: f32,
    pub velocity_change_rate: f32,
}

/// The Lightning effect's authored numbers (`mungefs::fx::LightningConfig`;
/// the ELF ctor's defaults where a map leaves a key out). The viewer consumes
/// [`Self::dome_tint`] at rest; the flash fields wait for the lightning card
/// (WTHL), which animates `brightness`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestLightning {
    /// `SkyDomeDarkenFactor`: dome shell 0 draws at white·(1 − f) between
    /// flashes; every other shell stays white.
    pub sky_dome_darken_factor: f32,
    /// `SunlightFadeFactor`.
    pub sunlight_fade_factor: f32,
    /// `Color`, 0..255 as authored: what a flash lifts the lights toward.
    pub color: [f32; 3],
    /// `BrightnessMin`.
    pub brightness_min: f32,
    /// `FadeTime`, seconds.
    pub fade_time: f32,
    /// `TimeBetweenFlashesMinMax`, seconds.
    pub time_between_flashes: [f32; 2],
    /// `TimeBetweenSubFlashesMinMax`, seconds.
    pub time_between_sub_flashes: [f32; 2],
    /// `NumSubFlashesMinMax`.
    pub num_sub_flashes: [f32; 2],
    /// `HorizonAngleMinMax`, degrees as authored.
    pub horizon_angle: [f32; 2],
    /// `SoundCrack` / `SoundSubCrack`.
    pub sound_crack: Option<String>,
    pub sound_sub_crack: Option<String>,
}

impl ManifestLightning {
    /// The colour byte the PS2 stores for dome shell 0 while a flash is at
    /// `brightness` (0 = between flashes, the resting look):
    /// white·(1 − f) + black·f with f = factor·(1 − brightness), truncated
    /// (B2 `FUN_0045ae80` 520152–520170, 520289–520300). 153 at rest for the
    /// discs' 0.4 — what PS2 RAM holds.
    pub fn dome_byte(&self, brightness: f32) -> u8 {
        let f = self.sky_dome_darken_factor.clamp(0.0, 1.0) * (1.0 - brightness.clamp(0.0, 1.0));
        (255.0 * (1.0 - f)) as u8
    }

    /// [`Self::dome_byte`] as the tint a shell is drawn with. The GS
    /// multiplies in gamma space, so the factor is linearised for the port's
    /// linear pipeline (the rule `ManifestSky::reflection_color` follows) —
    /// 255 stays exactly 1.0.
    pub fn dome_tint(&self, brightness: f32) -> f32 {
        let u = self.dome_byte(brightness) as f32 / 255.0;
        // sRGB EOTF, IEC 61966-2-1: the curve the dome's texture is sampled
        // through.
        if u <= 0.040_45 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    }
}

/// A spot's axis and outer cone ([`ManifestPointLight::cone`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManifestSpotCone {
    /// Unit direction the spot points, engine world space.
    pub dir: [f32; 3],
    /// `Cone`'s second value: the outer cone's FULL angle, radians.
    pub outer: f32,
}

/// One authored omni or spot light ([`Manifest::point_lights`]). The PS2
/// lights a model with a spot as a point: no cone
/// (`analysis/re-point-lights.md` §6 [O]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPointLight {
    /// The `Light` block's name.
    pub name: String,
    /// `Type(3)`: a model draw takes at most one spot beside its four omnis.
    pub spot: bool,
    /// A one-way spot's axis and cone, for the gather's back-cone test
    /// ([`ManifestSpotCone`]); `None` on an omni and on a `Bidirectional`
    /// spot, which the test never rejects.
    pub cone: Option<ManifestSpotCone>,
    /// Engine world space (the PS2's load negates the stored z).
    pub pos: [f32; 3],
    /// `Color` as authored, 0..1.
    pub color: [f32; 3],
    /// `Range`, metres.
    pub range: f32,
    /// `Static()`: a prelit model and the terrain skip the light.
    pub is_static: bool,
    /// `PS2Overbright()`: packed ×255 instead of ×128 — twice the colour.
    pub overbright: bool,
}

/// The bloom's authored numbers (`mungefs::fx::HdrConfig`; the ELF's defaults
/// where a map leaves a key out).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManifestHdr {
    /// Blur iterations (each two five-tap sub-passes).
    pub num_bloom_passes: u32,
    /// Each tap weighs `int(int(w · 127) · 0.2) / 128`.
    pub max_total_weight: f32,
    /// Luma cut in the half-scale frame: `2 ·` this of display white blooms.
    pub glow_threshold: f32,
}

/// How a `leafpatch` template places its sprites (class +0x94).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LeafLayout {
    /// The upper half-ellipsoid of `radius` × `height` (the default).
    Ball,
    /// `0x35a4393b` (w, h, d): uniform in that box.
    Box([f32; 3]),
    /// `0xcdbc383d` (x, z wobble): the recursive branching vine.
    Vine([f32; 2]),
}

/// A `leafpatch` class: its ODF properties as the PS2's setter stores them
/// (B1 `FUN_001b27b8`, defaults from the constructor `FUN_001b21e0`;
/// `analysis/re-leafpatch.md` §3). The template the PS2 builds from these
/// (`FUN_001b38f8`) is the runtime's (`game::viewer::leaf`). Names in the
/// docs of hashed fields are the RE's, not the disc's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestLeafClass {
    pub name: String,
    /// `Texture`, as the ODF spells it.
    pub texture: String,
    /// Airborne leaf slots per visible patch (`0xfd7fae90`, at most 50).
    #[serde(default)]
    pub falling_count: u8,
    #[serde(default)]
    pub bird_count: u8,
    /// `NumParticles` (NOT divided by the view count — LEAF's decision).
    pub particles: u32,
    pub min_size: f32,
    pub max_size: f32,
    pub radius: f32,
    pub height: f32,
    /// `0xcaaa3778`: the object's up axis × this at draw; template y ÷ it.
    pub scale: f32,
    /// `Offset`, added to every sprite.
    pub offset: [f32; 3],
    /// `0xeaaaa7a9`: the top metres that narrow toward the crown.
    pub taper: f32,
    /// `Seed`, wrapped to 32 bits; `None` = the PS2 draws one per session.
    pub seed: Option<u32>,
    pub layout: LeafLayout,
    /// `0xa8c0d41d`: a vine's straight-run length range.
    pub vine_run: [i32; 2],
    /// `0x45328dc6`: a vine's step, × the sprite size.
    pub vine_step: f32,
    pub darkness_min: f32,
    pub darkness_max: f32,
    pub alpha: f32,
    pub max_distance: f32,
    /// `NumParts`: 1 = the whole sheet turned by 90° per part, else quarters.
    pub num_parts: i32,
    /// `0xd4cbd35b` / `0x7cbb796c`: the sway's phase rate and amplitude.
    pub wind: [f32; 2],
}

/// A map's terrain foliage: the mesh table, the placed instances in a binary
/// sidecar ([`foliage_bin`]), and the path-block props.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestFoliage {
    /// `foliage.bin`, relative to the map dir: the instances FOLG places.
    pub instances: String,
    /// How many records it holds.
    pub count: u32,
    /// One row per `prp_` `Mesh()` entry, layers in order; a record's `mesh`
    /// indexes this.
    pub meshes: Vec<ManifestFoliageMesh>,
    /// The path block's props (re-foliage §9): world instances of their
    /// class, matrix as the PS2 builds it. Drawn like any instance; NOT in
    /// the collision world yet (queue FOLGC).
    pub path_props: Vec<ManifestInstance>,
    /// Every `grasspatch` class the map ships (re-foliage §7): what a
    /// `GrassPatch` mesh row names and what a world instance of the class
    /// draws. APPENDED; defaulted so a pre-FOLGG bake still loads.
    #[serde(default)]
    pub grass: Vec<ManifestGrassClass>,
}

/// A `grasspatch` class: its ODF properties as the PS2's setter stores them
/// (B1 `FUN_001adcd8`, defaults from the constructor `FUN_001ad820`;
/// analysis/re-foliage.md §7) plus the atlas the draw picks by `NumParts`.
/// The template the PS2 builds from these (`FUN_001ae120`) is the runtime's
/// (`game::viewer::grass`). Named fields carry the known property names; the
/// rest say their hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestGrassClass {
    pub name: String,
    /// `Texture`, as the ODF spells it (the texture table is case-blind).
    pub texture: String,
    /// `NumParticles` (NOT divided by the camera count — FOLG's decision).
    pub particles: u32,
    pub min_size: f32,
    pub max_size: f32,
    /// `0x3d5d0aef`: every sprite's base height.
    pub y_offset: f32,
    pub alpha: f32,
    /// `MaxDistance`: the draw distance of a patch placed as a world object.
    pub max_distance: f32,
    pub radius_fade_min: f32,
    pub radius_fade_max: f32,
    pub darkness_min: f32,
    pub darkness_max: f32,
    pub num_parts: i32,
    /// `0x35a4393b` (w, d): either > 0 places the sprites in that rectangle.
    pub rect: [f32; 2],
    /// `0x231fcc79`: a tall blade's rise range, × `MinSize`.
    pub raise: [f32; 2],
    /// `0xa9b14958`: a tall blade's size factor.
    pub tall_size: f32,
    /// `0xf6582b9c`: a tall blade's lean away from the camera.
    pub tall_lean: f32,
    /// `0xc75263e1`: a tall blade's darker ground quad, this far down
    /// (0 = none; no class on either disc sets it).
    pub shadow: f32,
    /// `0x97133eeb`: a normal sprite's random static lean, ±.
    pub lean: f32,
    /// `0x4b6d5171`: the part cycle's extra length.
    pub cycle: i32,
    /// `0xff7bb39a`: a normal sprite's width factor when `NumParts` is 3 or
    /// ≥ 5.
    pub thin: f32,
    /// `0x604db82e` present: tall blades do not sway.
    pub still: bool,
    /// The atlas the draw indexes by part (ELF .data, read at bake time):
    /// per part, four (u, v) corners — the sprite's top-left, top-right,
    /// bottom-left, bottom-right.
    pub atlas: Vec<[[f32; 2]; 4]>,
}

/// What a foliage mesh entry draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoliageKind {
    /// A model (`File`), drawn by the world pipeline.
    Model,
    /// A `grasspatch` class's sprite cluster (queue FOLGG).
    GrassPatch,
    /// A `leafpatch` class (queue LEAF).
    LeafPatch,
}

/// One `prp_` `Mesh()` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestFoliageMesh {
    /// Its layer, 0..4.
    pub layer: u8,
    pub kind: FoliageKind,
    /// The model name (`Model`) or class name (a patch), as the LVL spells
    /// it; `None` when it names nothing this LVL ships (the PS2 draws
    /// nothing there either).
    pub name: Option<String>,
    /// Fade distance d, metres: a model fades out over [d, 1.45·d] of
    /// horizontal distance; a grass patch over the last 20 m before d.
    pub distance: f32,
    /// Models nearer than this (horizontal) fade toward 0.1²; 0 = none.
    pub near_fade: f32,
    /// Uniform scale (models).
    pub scale: f32,
    /// Up = lerp(terrain normal, world up, stiffness).
    pub stiffness: f32,
    /// `0x26edc40c`: the model's collision mesh is solid on the PS2.
    pub collision: bool,
}

/// `foliage.bin`: the placed foliage instances, one definition for the bake
/// (writer) and the runtime (reader).
///
/// ```text
/// b"FOLG" u32 version (1) u32 count
/// count × { u16 mesh, u16 0, f32 x, f32 z, f32 yaw, u8 r, g, b, a }  (20 B)
/// ```
///
/// x and z are world metres, yaw radians (models only), rgba the PS2's
/// per-instance tint (255 = unity). Height and tilt are NOT stored: the
/// runtime takes them from the terrain it loads (re-foliage §6).
pub mod foliage_bin {
    /// One placed instance.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Record {
        pub mesh: u16,
        pub x: f32,
        pub z: f32,
        pub yaw: f32,
        pub rgba: [u8; 4],
    }

    const MAGIC: &[u8; 4] = b"FOLG";
    const VERSION: u32 = 1;
    const RECORD: usize = 20;

    pub fn write(records: &[Record]) -> Vec<u8> {
        let mut out = Vec::with_capacity(12 + records.len() * RECORD);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for r in records {
            out.extend_from_slice(&r.mesh.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&r.x.to_le_bytes());
            out.extend_from_slice(&r.z.to_le_bytes());
            out.extend_from_slice(&r.yaw.to_le_bytes());
            out.extend_from_slice(&r.rgba);
        }
        out
    }

    /// `None` on a wrong magic, version or length.
    pub fn read(b: &[u8]) -> Option<Vec<Record>> {
        if b.len() < 12 || &b[..4] != MAGIC {
            return None;
        }
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let f32_at = |o: usize| f32::from_bits(u32_at(o));
        let n = u32_at(8) as usize;
        if u32_at(4) != VERSION || b.len() != 12 + n * RECORD {
            return None;
        }
        Some(
            (0..n)
                .map(|i| {
                    let o = 12 + i * RECORD;
                    Record {
                        mesh: u16::from_le_bytes([b[o], b[o + 1]]),
                        x: f32_at(o + 4),
                        z: f32_at(o + 8),
                        yaw: f32_at(o + 12),
                        rgba: b[o + 16..o + 20].try_into().unwrap(),
                    }
                })
                .collect(),
        )
    }
}

/// A map's water surface. The mesh is baked flat at `height`; the renderer
/// scrolls its UVs by `uv_velocity`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestWater {
    /// GLB filename beside the manifest.
    pub glb: String,
    /// Texture painting the surface; `None` when the map names one we did
    /// not bake (never happens on the twelve shipped water maps).
    #[serde(default)]
    pub texture: Option<String>,
    /// World Y of the plane. PROVEN — see `mungefs::water`.
    pub height: f32,
    /// UV units per second the renderer scrolls the surface by. Since
    /// 2026-09-09 this is the PS2's `fx__ Velocity / INFO5` on every map that
    /// authors a `Velocity` (all twelve); the pre-`fx__` value read `WATR ->
    /// LAYR`'s dead copy, which both ELFs drop. The SIGN is PROVISIONAL —
    /// see `mungefs::fx::WaterConfig::scroll_per_second`.
    #[serde(default)]
    pub uv_velocity: [f32; 2],
    /// `WATR -> LAYR`'s colour word. **DEAD DATA** — both ELFs read and drop
    /// it (`analysis/re-water.md` §1), so its byte order is moot and nothing
    /// may consume it. The PS2's tint is `config`'s diffuse LUT. Recorded
    /// because it is what the bytes hold.
    #[serde(default)]
    pub layer_color: [u8; 4],
    /// Informational: wet `WMAP` cells, `CPAT` patches, triangles.
    #[serde(default)]
    pub wet_cells: usize,
    #[serde(default)]
    pub patches: usize,
    #[serde(default)]
    pub triangles: usize,
    /// What the map's `fx__ Effect("Water")` authors. APPENDED after
    /// `triangles` so a manifest's existing RON text is unchanged up to this
    /// point; defaulted so a pre-`fx__` manifest still loads. `None` would
    /// mean a water map with no `Effect("Water")`, which neither disc ships.
    #[serde(default)]
    pub config: Option<ManifestWaterConfig>,
    /// Authored wet-cell grid used for projectile/water intersections.
    #[serde(default)]
    pub wet: Option<ManifestWetMask>,
    /// The water GLB carries diffuse LUT RGB; do not apply its mean tint again.
    #[serde(default)]
    pub vertex_diffuse: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub far_fog_color: Option<[f32; 3]>,
}

/// Authored water-cell mask; hexadecimal nibbles preserve the disc grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestWetMask {
    pub grid: u32,
    pub cell_size: f32,
    pub half_extent: [f32; 2],
    pub nibbles: String,
}

impl ManifestWetMask {
    pub fn from_nibbles(grid: u32, cell_size: f32, half_extent: [f32; 2], cells: &[u8]) -> Self {
        Self {
            grid,
            cell_size,
            half_extent,
            nibbles: cells
                .iter()
                .map(|c| char::from_digit(u32::from(c & 0xf), 16).unwrap())
                .collect(),
        }
    }

    pub fn cells(&self) -> Vec<u8> {
        self.nibbles
            .chars()
            .map(|c| c.to_digit(16).map_or(0, |d| d as u8))
            .collect()
    }
}

/// The serde default of a water flag the PS2 resets to 1 (`specular_enable`,
/// `oscillation_enable`). The derived `Default` still gives `false` — that is
/// a test fixture's "unset", not a disc value.
fn flag_on() -> bool {
    true
}

/// The PS2's own water configuration, from the LVL's `fx__` chunk
/// (`mungefs::fx::WaterConfig`, `analysis/re-water.md` §5). `None` on a field
/// means the map authored no such key — never a substituted default.
///
/// The bake consumes `main_texture` and `metres_per_repeat` (both are already
/// applied to `glb`'s material and UVs) and records the rest for the runtime's
/// water look — tint, alpha and the speckle pass (`game::viewer::look`) and the
/// KAM1 ocean (`game::water_anim`). Colours are 0..255 RGBA, as the discs
/// store them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ManifestWaterConfig {
    /// `MainTexture` — what actually paints the surface. On both KAM1s this
    /// differs from `LAYR`'s name, which is the FOAM texture.
    #[serde(default)]
    pub main_texture: Option<String>,
    /// `FoamTexture`, blended over the base on an ocean map. Both KAM1s only.
    #[serde(default)]
    pub foam_texture: Option<String>,
    /// Metres of world per texture repeat, `cell / (Tile · INFO5)` — the
    /// value `glb`'s UVs were built with. `None` when the map authors no
    /// `Tile` (no disc map does).
    #[serde(default)]
    pub metres_per_repeat: Option<[f32; 2]>,
    /// `OceanEnable` — 1 on both KAM1s. Those maps draw a Tessendorf FFT
    /// surface on the PS2, and so does the port (`game::water_anim`).
    #[serde(default)]
    pub ocean: bool,
    /// `DisableLowRes` — BF1 KAS1, both YAV1s and BF2 DAG1. Those maps draw
    /// no far water at all (`re-water.md` §1).
    #[serde(default)]
    pub disable_low_res: bool,
    /// `SpecularEnable` — the byte BF2's flat-water renderer reads to gate its
    /// speckle layer and its far-water fog: 1 unless the map authors 0. The
    /// Water effect's level reset stores 1 (BF2 ELF 0x56027c) and no map on
    /// either disc authors the key (`analysis/re-water-flags.md` §1–§4
    /// `[O]`). BF1 has no gate, so the field means nothing there.
    #[serde(default = "flag_on")]
    pub specular_enable: bool,
    /// `OscillationEnable` — the byte that gates the flat water's 6 cm bob:
    /// 1 unless the map authors 0, which only BF2 FEL1 does. The same reset
    /// stores 1 (`re-water-flags.md` §7 `[O]`); BF1 has no gate.
    #[serde(default = "flag_on")]
    pub oscillation_enable: bool,
    /// The per-vertex diffuse LUT's ends, Border → Min → Max (`re-water.md`
    /// §3). W2's tint and alpha come from these.
    #[serde(default)]
    pub min_diffuse: Option<[f32; 4]>,
    #[serde(default)]
    pub max_diffuse: Option<[f32; 4]>,
    #[serde(default)]
    pub border_diffuse: Option<[f32; 4]>,
    #[serde(default)]
    pub specular_color: Option<[f32; 4]>,
    /// The additive animated highlight layer (W3): its frames in order, the
    /// rate, and the UV tiling/scroll/shift.
    #[serde(default)]
    pub speckle_textures: Vec<String>,
    #[serde(default)]
    pub speckle_fps: Option<f32>,
    #[serde(default)]
    pub speckle_tile: Option<[f32; 2]>,
    #[serde(default)]
    pub speckle_scroll: Option<[f32; 2]>,
    #[serde(default)]
    pub speckle_coord_shift: Option<[f32; 2]>,
    #[serde(default)]
    pub speckle_specular_color: Option<[f32; 4]>,
    #[serde(default)]
    pub speckle_ambient_color: Option<[f32; 4]>,
    /// `LightAzimAndElev` — (1, 0) on all twelve.
    #[serde(default)]
    pub light_azim_and_elev: Option<[f32; 2]>,
    /// The PS2 mesh's subdivisions per cell and its far-pass decimation. The
    /// port bakes two triangles per cell instead; `patch_divisions` sizes only
    /// the KAM1 ocean's runtime grid (`game::water_anim`), `lod_decimation`
    /// is recorded and not consumed.
    #[serde(default)]
    pub patch_divisions: Option<[f32; 2]>,
    #[serde(default)]
    pub lod_decimation: Option<f32>,
    /// Metres of world per repeat of the FOAM layer, `cell / FoamTile`: the
    /// PS2 builds the foam's texture matrix from `FoamTile` as authored,
    /// without the base's `× INFO5` (B1 `FUN_0048b280` 551026-551035, B2
    /// 708397-708398; `analysis/re-water.md` §3d). Both KAM1s: 160 / 5 =
    /// 32 m against the base's 40. `None` when the map authors no
    /// `FoamTile`; absent in pre-2026-10-28 bakes.
    /// Appended last (the wire-format rule, `manifest::tests`).
    #[serde(default)]
    pub foam_metres_per_repeat: Option<[f32; 2]>,
    /// Donor normal-map water. Omitted on PS2 to preserve its bake wire bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_maps: Option<ManifestNormalWater>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestNormalWater {
    pub textures: Vec<String>,
    pub fps: f32,
    pub refraction: [f32; 4],
    pub reflection: [f32; 4],
    pub fresnel: [f32; 2],
}

// --- lighting, fog and sky (baked by `bake::lighting`) ----------------------

/// Linear fog between `near` and `far` metres. Colour 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManifestFog {
    pub color: [f32; 3],
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestLighting {
    /// Where each value came from, for the artifact READMEs and for anyone
    /// wondering why a map looks the way it does: "lght", "sky_", "mixed" or
    /// "default". Defaulted (empty) for bakes made before it was recorded.
    #[serde(default)]
    pub source: String,
    /// Unit direction the sunlight TRAVELS (sun -> scene), engine Y-up world.
    pub sun_dir: [f32; 3],
    /// Linear, 0..1 — both games' authored colour through sRGB -> linear
    /// (BF2's `Light.Color` since queue LITEB, `bake::lighting`).
    pub sun_color: [f32; 3],
    /// Hemisphere ambient, 0..1. `ambient_ground` equals `ambient_sky` when
    /// the map authors only one ambient colour (every BF1 map).
    pub ambient_sky: [f32; 3],
    pub ambient_ground: [f32; 3],
    /// `SkyInfo.CharacterAmbientColor` / 255 where authored — the discs light
    /// soldiers with a separate, usually brighter ambient. The runtime takes
    /// it on BF1 maps only (`game::viewer::look`).
    #[serde(default)]
    pub character_ambient: Option<[f32; 3]>,
    #[serde(default)]
    pub fog: Option<ManifestFog>,
    /// `SkyInfo.NearSceneRange(a, b, c)` in metres, as authored. `c` is where
    /// the PS2's NEAR scene ends: the heightfield is drawn with a camera
    /// frustum whose far plane is `c`, and `b..c` is the fade band into the
    /// far-scene buffer (`analysis/re-fog.md` §2.5, `analysis/re-lowr.md`
    /// §2.2 [V]). Every shipped ground map authors it; `None` for a manifest
    /// baked before this field existed, or a map with no `sky_ SkyInfo`.
    #[serde(default)]
    pub near_scene_range: Option<[f32; 3]>,
    /// `SkyInfo.FarSceneRange` in metres, as authored — the distance the PS2
    /// culls FAR-SCENE draws at (world models by bounding sphere, the water's
    /// low-res pass, the `LOWR` sheet). `0.0` is authored and MEANS "no far
    /// layer at all" on nine of the 32 baked maps (`analysis/re-fog.md`
    /// §2.5/§6 [V] lists ten; BF2 TAT3 is not a baked ground map), so it is
    /// kept as `Some(0.0)` and never folded into `None`.
    #[serde(default)]
    pub far_scene_range: Option<f32>,
    /// The key light (`GlobalLights.Light1`) authors `Static()` — BF2 only;
    /// BF1 has no `lght`. A PRELIT model's light set skips every Static light
    /// (`analysis/re-model-lighting.md` §3 `[O]`); a normal model takes it.
    /// `false` for a manifest baked before queue LITEB.
    #[serde(default)]
    pub sun_static: bool,
    /// `GlobalLights.Light2`, when it names a directional (`Type(1)`) light:
    /// the PS2's second directional light on every model (§3 `[O]`: BF2
    /// COR1, END1, MUS1, NAB2, UTA1). `None` on BF1, on a BF2 map whose
    /// `Light2` names nothing, and on a manifest baked before queue LITEB.
    #[serde(default)]
    pub light2: Option<ManifestLight>,
    /// `DomeInfo`'s frame filter where `Filter` ≠ 0 and `Intensity` ≥ 1 —
    /// every BF1 map but HOT1, and BF2 TAT2 (`analysis/re-glow.md` §3
    /// `[O]`). `None` = no pass, and for a manifest baked before card BFLT.
    #[serde(default)]
    pub filter: Option<ManifestFilter>,
}

/// The PS2's DomeInfo frame filter, as authored: over the finished 3D frame,
/// before the HUD, per channel in display bytes,
/// `out = min(255, v + intensity/128 · max(0, low-pass(v) − threshold))`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManifestFilter {
    /// `Threshold`, a display byte 0..255.
    pub threshold: f32,
    /// `Intensity`; the gain is `intensity / 128`.
    pub intensity: f32,
}

/// One directional light beside the key sun (`ManifestLighting::light2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManifestLight {
    /// Unit direction the light TRAVELS, engine Y-up world — the same
    /// reduction as `ManifestLighting::sun_dir`.
    pub dir: [f32; 3],
    /// Linear, 0..1 (the authored `Color`, sRGB -> linear).
    pub color: [f32; 3],
    /// `Static()` authored — see `ManifestLighting::sun_static`.
    pub is_static: bool,
}

/// The engine's pre-2026-08-31 hardcoded look, for a manifest baked before
/// `lighting` existed (mirrors `render::Lighting::default` and
/// `bake::lighting`'s `DEFAULT_*` constants). PROVISIONAL and ours — nothing
/// on either disc authors these.
impl Default for ManifestLighting {
    fn default() -> Self {
        Self {
            source: "default".into(),
            sun_dir: [-0.400_51, -0.801_02, -0.450_57],
            sun_color: [1.0, 1.0, 0.95],
            ambient_sky: [0.25, 0.25, 0.28],
            ambient_ground: [0.25, 0.25, 0.28],
            character_ambient: None,
            fog: None,
            near_scene_range: None,
            far_scene_range: None,
            sun_static: false,
            light2: None,
            filter: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestSkyModel {
    /// `modl` name; the bake checks it against the map's models.
    pub geometry: String,
    /// Vertical offset of the shell in metres (0 when unauthored).
    pub offset: f32,
    /// Fraction of camera motion the shell follows; 1.0 = fully camera-locked.
    pub movement_scale: f32,
    /// `DomeModel.Reflect`: the shell is also drawn mirrored under the eye,
    /// tinted by [`ManifestSky::reflection_color`]
    /// (`analysis/re-sky-reflect.md` §2, §5). Absent in pre-2026-09-29 bakes.
    #[serde(default)]
    pub reflect: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ManifestSkyEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestSkyEffect {
    pub effect: String,
    pub hardpoint: String,
    pub parameter: Option<f32>,
    pub pos: [f32; 3],
    pub axes: [[f32; 3]; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_delay: Option<f32>,
}

/// One `sky_ SkyObject`: a flight of distant capital ships, drawn in the sky
/// around the map. The geometry lives in a SIDE LVL, not the map's own — see
/// `bake::map`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestSkyObject {
    /// `modl` name, copied into the map's own models by the bake.
    pub geometry: String,
    pub count: u32,
    /// `Height` — an authored altitude RANGE in metres.
    pub height: [f32; 2],
    /// `Distance` — metres from the viewer.
    pub distance: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity_x: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity_y: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity_z: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceleration: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifetime: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestSky {
    /// `DomeInfo.Texture` — the dome's own sheet, where authored.
    #[serde(default)]
    pub texture: Option<String>,
    /// `DomeModel` shells, outermost first as stored.
    pub models: Vec<ManifestSkyModel>,
    /// `DomeInfo.Ambient` / 255 — the port's flat tint on every shell. Neither
    /// ELF reads the key (hash 0x1c9640eb is in neither; `re-kamino.md` §1)
    /// and it bakes to white on all 32 maps, so it is inert; kept because
    /// dropping it would move every manifest for nothing (card KSKY). What
    /// the PS2 does tint a shell with is [`Manifest::lightning`].
    pub ambient: [f32; 3],
    /// `LowResTerrain.Texture` and `MaxDistance` as stored. The texture being
    /// authored is what gates the far `LOWR` sheet's draw
    /// ([`Manifest::lowres_glb`], `game::viewer::look`); the distance is the
    /// far end of the range the sheet is drawn over (the same function; 2000
    /// m where the map authors none).
    #[serde(default)]
    pub low_res_terrain: Option<String>,
    #[serde(default)]
    pub low_res_max_distance: Option<f32>,
    /// `SkyObject` flights whose geometry the bake resolved out of a SIDE
    /// LVL. Empty on the 23 maps that author none (and on any bake that
    /// could not reach the SIDE LVLs), and absent in pre-2026-08-31 bakes.
    #[serde(default)]
    pub objects: Vec<ManifestSkyObject>,
    /// `DomeInfo.TerrainEnable` as stored, on either game — the discs author
    /// it only as 0, on BF1 KAM1 and BES1. Only BF1 reads it: at 0 its
    /// terrain is not drawn at all (`analysis/re-terrain-enable.md` §2 `[O]`),
    /// which `game::viewer::scene::terrain_drawn` applies. Absent in
    /// pre-2026-09-28 bakes.
    #[serde(default)]
    pub terrain_enable: Option<f32>,
    /// `DomeInfo.ReflectionColor` as the tint the mirrored `reflect` shells
    /// are drawn with: the GS value `((c + 1) >> 1) / 128`, linearised like
    /// every other authored colour. `None` = the loader's default white
    /// (`analysis/re-sky-reflect.md` §1). Absent in pre-2026-09-29 bakes.
    #[serde(default)]
    pub reflection_color: Option<[f32; 3]>,
    /// `DomeInfo.Stars()` with the loaders' defaults filled in; `None` where
    /// the map authors no stars (or `NumStars` 0). Absent in pre-2026-10-27
    /// bakes.
    #[serde(default)]
    pub stars: Option<ManifestStars>,
    #[serde(default)]
    pub glow_masks_preserved: bool,
}

/// A map's star field as both games' loaders read it (BF1 `FUN_002e6a50`,
/// BF2 `FUN_003bfc30`; `analysis/re-stars.md` §1). The renderer rebuilds the
/// stars from these with the PS2's own generator (`render::stars`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestStars {
    /// `NumStars`, ≥ 1.
    pub count: u32,
    /// `RandomSeed` (default 0).
    pub seed: u32,
    /// `TwinkleFactor` (default 0.5): how far a star's alpha dips.
    pub twinkle_factor: f32,
    /// `TwinkleFrequency` (default 20): new twinkle targets per second.
    pub twinkle_frequency: f32,
    /// `Color` bytes, 0..255 (the loaders clamp; four values with alpha).
    pub color: [u8; 4],
    /// `BrightStarPercent` / 100 (default 0.08): the share at full brightness.
    pub bright_fraction: f32,
    /// `AlphaMin` / 255 (default 0.05): the dimmest star's brightness.
    pub alpha_min: f32,
    /// `ColorSaturation` (default 1.0): how far the red / blue tint goes.
    pub color_saturation: f32,
    /// `EnableBottom` ≠ 0 as authored: the field is drawn a second time
    /// mirrored under the horizon. Only BF2's draw reads it (the BF1 loader
    /// has no such key); the game applies that gate.
    pub enable_bottom: bool,
}
