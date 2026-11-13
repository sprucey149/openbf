//! A map's look as render state, decided from its manifest alone: the water's
//! tint, fog and reach ([`water_look`]), its speckle layer ([`speckle_look`]),
//! what the frame clears to, the fog
//! density scale, the far-scene cull and the `LOWR` sheet's range (both behind
//! their env flags), BF2's fade into the sky, and [`apply_lighting`], which
//! pushes the result into the renderer. Manifest-to-look calculations are pure
//! and unit-tested without a GPU; env-gate helpers and `frame_filter_from`
//! also read process environment.

use crate::assets::Scene;
use glam::Vec3;
use render::Renderer;

// Only for the item docs below, which link `upload_scene` (the `scene` part).
#[cfg(doc)]
use super::scene::upload_scene;

/// The fraction of `FarSceneRange` the FAR water reaches, flat water and
/// ocean. `analysis/re-water.md` §1 [V]: the far pass's frustum is
/// `[NearSceneRange.b, 0.9·FarSceneRange]` (BF1 551290–551294,
/// `DAT_00522a7c` = 0.9) and the ocean's own driver uses 0.8 (551039).
const FAR_WATER_FRACTION: f32 = 0.9;
const OCEAN_FAR_FRACTION: f32 = 0.8;

/// BF2's LIT ocean, three numbers fitted to the PS2's frames — all
/// **PROVISIONAL** (card KSEAL, `analysis/re-kamino.md` §2.6): the weight of
/// the sun's diffuse term on the sea's GS colour, the foam layer's blend
/// weight, and the fraction of its colour the sea keeps at the far pass's
/// end (it loses the rest linearly from `FogRange`'s near distance).
const OCEAN_SUN_WEIGHT: f32 = 0.75;
const OCEAN_LIT_FOAM_ALPHA: f32 = 0.5;
const OCEAN_FAR_KEEP: f32 = 0.5;

/// How a map's water is drawn, decided per map and consumed by
/// [`upload_scene`] and [`apply_lighting`].
///
/// `tint`, `foam_tint`'s colour, `fog_begin` and `far` cite
/// `analysis/re-water.md` §1/§3 [V] and `re-water-colour.md` [O]. Ours are
/// the collapse of the disc's per-vertex diffuse LUT noise to its mean (see
/// [`water_look`]) and BF2's lit-ocean fit — `sun`, `dim` and the foam's
/// alpha there, PROVISIONAL (`analysis/re-kamino.md` §2.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterLook {
    /// The main layer's flat tint in GS terms (byte / 255, GAMMA space — the
    /// shader adds the view-angle term to it before linearising), with the
    /// blend weight in alpha — `DrawCmd::water`'s instance colour.
    pub tint: [f32; 4],
    /// The ocean's foam layer, `None` on a flat-water map. Unity: the ocean's
    /// vertex colour is the engine's white halved (`re-water-colour.md` §1.9
    /// `[O]`), and the foam is composited by ITS OWN texture alpha (record
    /// 0x69c3e0, ALPHA 0x44), not by the diffuse LUT's. On BF2's lit ocean
    /// its alpha is the fitted blend weight ([`water_look`], **Lit ocean**).
    pub foam_tint: Option<[f32; 4]>,
    /// Metres past which the water takes the world's fog ramp; `None` = this
    /// map's water is never fogged. The far water only.
    pub fog_begin: Option<f32>,
    /// Metres past which the water is not drawn at all: `NearSceneRange.c` on
    /// a `DisableLowRes` map (no far pass exists), else the far pass's own
    /// reach. `None` only when the manifest authors no `NearSceneRange`.
    ///
    /// On FLAT water it is applied through [`render::DrawCmd::with_cull`]
    /// and, like every other distance cull, ONLY when [`far_cull_enabled`] —
    /// see the note in [`upload_scene`]. An OCEAN always ends there (card
    /// KSEA): `crate::water_anim` draws whole cells that reach inside it.
    pub far: Option<f32>,
    /// The weight of the sun's diffuse term on the water's colour, base and
    /// foam; 0 = unlit. BF2's ocean only ([`water_look`], **Lit ocean**).
    pub sun: f32,
    /// A lit ocean's loss of colour with distance, `[begin m, rate per m]`.
    pub dim: Option<[f32; 2]>,
}

/// The PS2's water look for one map (pure — no GPU, no scene).
///
/// **Tint and alpha.** The disc paints the flat water per VERTEX from a
/// 16-entry diffuse LUT indexed uniformly over entries 5..15 — Min … 2·Max −
/// Min, whose mean is `MaxDiffuse` — and a Min.a → Max.a alpha table whose
/// mean is their midpoint (`analysis/re-water-colour.md` §1.5/§1.6 `[O]`).
/// The port has no per-vertex index, so it takes the two MEANS: colour
/// `MaxDiffuse / 255`, alpha `mean(Min.a, Max.a) / 255` — **PROVISIONAL**
/// `[O]`, the mean of a noise pattern, not the pattern. Both are GS terms
/// (the byte the GS reads, 0x80 = 1.0 in MODULATE): `fs_water` adds the
/// view-angle term to them in that space ([`water_shading`]) and linearises
/// the sum; alpha is a weight and takes no transfer. The water is UNLIT —
/// routine B reads no light colour and no normal (§1.2).
///
/// **Ocean.** An `OceanEnable` map draws its base layer OPAQUE (record
/// 0x69c3c0 = ALPHA 0x4100, `re-water.md` §3 cit. 5) at UNITY: its vertex
/// colour is the engine's global white halved to GS 128, and its path never
/// loads the overlay, so no LUT and no view-angle term (BF1 `FUN_0048a6c8`,
/// `re-water-colour.md` §1.9 `[O]`).
///
/// **Lit ocean (BF2).** BF2's ocean packer writes the FFT normals into the
/// packet and submits a normal stream (`FUN_00574a70` 708056–708058, 708105
/// [V]), and PCSX2 shows what they are for: with the lightning frozen the sea
/// reads the texture × the sun's colour (`analysis/re-kamino.md` §2.1, §2.3,
/// §2.6). Which VU program lights it is unread, so the rule is a fit,
/// **PROVISIONAL** [H], kept here in three constants: the base and the foam
/// are scaled by `sun byte / 128 · max(n·l, 0) · OCEAN_SUN_WEIGHT` in GS
/// terms (no ambient — the PS2's sea has the sun's hue); the foam layer
/// blends at `OCEAN_LIT_FOAM_ALPHA` (the PS2's RAM holds the same foam bytes
/// the port steps, and its frame shows them faintly); and past `FogRange`'s
/// near distance the sea loses colour linearly, keeping `OCEAN_FAR_KEEP` at
/// the far pass's end — the PS2's sea darkens from 400 m by more than the
/// fog its colour pokes measure, cause unread. BF1's ocean stays the unity
/// draw (PS2 (17,30,33) against the port's (20,33,29), card KOC).
///
/// **Fog.** The near water is NEVER fogged in either game, the far water is
/// fogged past `NearSceneRange.c` in BF1 and in BF2 under `SpecularEnable`
/// (BF2's flat renderer gates that sub-pass on it; the level reset stores 1
/// and no disc map authors the key, so it is ON — `re-water-flags.md` §1–§5
/// `[O]`), and a `DisableLowRes` map has no far water to fog (§1 [V]). That
/// gate is in BF2's FLAT renderer. Its OCEAN (KAM1) is the other branch and
/// has no such gate: the far pass of `FUN_00575670` lays the world fog colour
/// over its tiles through `fogspread` whenever it drew one (708400–708451),
/// and PCSX2 shows it — the fog colour poked green / red / white adds 47 / 43
/// / 39–46 levels at the sea's edge, 31 at 585 m, 19 at 521 m, 7 at 470 m and
/// 0 at 428 m, which is the world's own ramp from `FogRange`'s 400 m
/// (`analysis/re-kamino.md` §2.1–§2.2 `[O]`, card KSEA). So an ocean is
/// fogged past `NearSceneRange.c` in both games. On the shipped BF2 FLAT-water
/// maps the far fog shows nowhere: KAS2's fog is off, NAB2, DEA1 and FEL1
/// have no far layer, YAV1 and DAG1 author `DisableLowRes`
/// (`re-water-flags.md` §5).
///
/// A manifest with no `config` — anything baked before the `fx__` reader
/// landed — gets the untinted, opaque water it had before this item, rather
/// than an invented tint; its fog and far distances still come from `sky_`
/// (BF1: fogged past `NearSceneRange.c`; BF2: unfogged, the gate needs the
/// config).
pub fn water_look(
    source_game: &str,
    lighting: &manifest::ManifestLighting,
    water: &manifest::ManifestWater,
) -> WaterLook {
    let cfg = water.config.as_ref();
    let ocean = cfg.is_some_and(|c| c.ocean);
    let lut = cfg.and_then(|c| c.min_diffuse.zip(c.max_diffuse));
    let tint = match lut {
        Some((min, max)) if !ocean => [
            max[0] / 255.0,
            max[1] / 255.0,
            max[2] / 255.0,
            ((min[3] + max[3]) * 0.5 / 255.0).clamp(0.0, 1.0),
        ],
        _ => [1.0; 4],
    };
    let lit = ocean && source_game != "bf1";
    let foam_alpha = if lit { OCEAN_LIT_FOAM_ALPHA } else { 1.0 };
    let foam_tint = (ocean && cfg.is_some_and(|c| c.foam_texture.is_some()))
        .then_some([1.0, 1.0, 1.0, foam_alpha]);

    let near_c = lighting
        .near_scene_range
        .map(|r| r[2])
        .filter(|c| c.is_finite() && *c > 0.0);
    let disable_low_res = cfg.is_some_and(|c| c.disable_low_res);
    let far = near_c.map(|c| {
        if disable_low_res {
            c
        } else {
            let k = if ocean {
                OCEAN_FAR_FRACTION
            } else {
                FAR_WATER_FRACTION
            };
            // The same floor `FarCull::world` takes: `FarSceneRange(0)` means
            // "no far layer at all", and the near scene still draws the
            // water inside its own range.
            (k * lighting
                .far_scene_range
                .filter(|f| f.is_finite())
                .unwrap_or(DEFAULT_FAR_SCENE_RANGE))
            .max(c)
        }
    });
    let gated_on = source_game == "bf1" || cfg.is_some_and(|c| c.ocean || c.specular_enable);
    let fog_begin = match gated_on && !disable_low_res {
        true => near_c,
        false => None,
    };
    let sun = if lit { OCEAN_SUN_WEIGHT } else { 0.0 };
    WaterLook {
        tint,
        foam_tint,
        fog_begin,
        far,
        sun,
        dim: lit
            .then(|| lighting.fog.map(|f| f.near).zip(far))
            .flatten()
            .filter(|(begin, end)| end > begin)
            .map(|(begin, end)| [begin, (1.0 - OCEAN_FAR_KEEP) / (end - begin)]),
    }
}

/// The PS2's default `SpeckleTextures` rate when a map authors no fps:
/// `DAT_005649b0 = 0x3dcccccd` (0.1) at the config reset, BF1 550217
/// (`FUN_00489c40`'s reset block; `analysis/re-water.md` §3). Every flat
/// water map authors its own (2 or 4), so it is the fallback only.
const SPECKLE_DEFAULT_FPS: f32 = 0.1;

/// The water's SPECKLE layer, decided per map: an additive flipbook of the
/// map's `SpeckleTextures` over the water surface (queue W3).
#[derive(Debug, Clone, PartialEq)]
pub struct SpeckleLook {
    /// The frame textures, in the disc's order.
    pub frames: Vec<String>,
    /// Frames per second — `SpeckleTextures`' rate. The layer's colour and
    /// add weight are per fragment, from [`water_shading`].
    pub fps: f32,
    /// Multiplier on the water mesh's UV: the speckle repeats `SpeckleTile`
    /// times per WATR cell where the mesh's UV repeats once per
    /// `metres_per_repeat`.
    pub uv_scale: [f32; 2],
    /// UV units per second, `SpeckleScrollSpeed`.
    pub scroll: [f32; 2],
}

/// The PS2's speckle layer for one map, or `None` where it draws none (pure).
///
/// **What is [V]** (`analysis/re-water.md` §1/§3, BF1 `FUN_0048b9c0`): a
/// flat-water map draws, after its main layer, the frame
/// `SpeckleTextures[(t·fps) mod n]` ADDITIVELY (record 0x69c3b0, ALPHA 0x48,
/// no Z write); the near pass cross-fades that frame with the next by
/// `frac(t·fps)` (551452–551455). Its UV is the cell's own 0..1 times
/// `SpeckleTile` plus `frac(t·SpeckleScrollSpeed)` (551399–551402 — note: NOT
/// times INFO5, which scales only the main layer's `Tile`, 538831–538838), so
/// against the mesh UV (`world / metres_per_repeat`) the multiplier is
/// `SpeckleTile · metres_per_repeat / cell`. The ocean path (`OceanEnable`,
/// `FUN_0048b280`) draws no speckle, so both KAM1s have none. The near
/// speckle's sheet alpha never reaches the blend (TCC cleared,
/// `re-water-alpha.md` §7.1 `[O]`).
///
/// **BF2** draws the layer only under `SpecularEnable` (708831/708897), a
/// byte the Water effect's level reset sets to 1 before the map's `fx__` is
/// read; no disc map authors the key, so every BF2 flat-water map draws it
/// (`analysis/re-water-flags.md` §1–§4 `[O]`). BF1 has no gate.
///
/// **The colour** is the VU1 overlay's routine A, per vertex on the PS2 and
/// per fragment here: `SpeckleAmbient + SpeckleSpecular · lobe³²`, a sun
/// lobe, its alpha the add weight (`re-water-colour.md` §1.7 `[O]`) — built
/// by [`water_shading`] and `fs_speckle`, so it is not a field here.
///
/// `cell` is the WATR cell side in metres (read off the baked mesh).
pub fn speckle_look(
    source_game: &str,
    water: &manifest::ManifestWater,
    cell: f32,
) -> Option<SpeckleLook> {
    let c = water.config.as_ref()?;
    let drawn = source_game == "bf1" || c.specular_enable;
    if c.ocean || !drawn || c.speckle_textures.is_empty() || cell.is_nan() || cell <= 0.0 {
        return None;
    }
    c.speckle_specular_color?;
    let mpr = c.metres_per_repeat?;
    let tile = c.speckle_tile.unwrap_or([1.0, 1.0]);
    Some(SpeckleLook {
        frames: c.speckle_textures.clone(),
        fps: c.speckle_fps.unwrap_or(SPECKLE_DEFAULT_FPS),
        uv_scale: [tile[0] * mpr[0] / cell, tile[1] * mpr[1] / cell],
        scroll: c.speckle_scroll.unwrap_or([0.0, 0.0]),
    })
}

/// The two frames the speckle shows at `t` seconds and the second one's
/// weight: `(i, i + 1 mod n, frac(t·fps))`, `i = ⌊t·fps⌋ mod n` — BF1
/// 551452–551455 (the PS2 quantises the weight to 1/128; the port does not).
pub fn speckle_frames(t: f32, fps: f32, n: usize) -> (usize, usize, f32) {
    if n == 0 {
        return (0, 0, 0.0);
    }
    let f = (t * fps).max(0.0);
    let i = (f.floor() as usize) % n;
    (i, (i + 1) % n, f - f.floor())
}

/// The metres routine B shifts its view origin against the light:
/// `DAT_00564984` (BF1) / `DAT_0066168c` (BF2) = 0x41200000, stored only by
/// the Water effect's reset (`re-water-colour.md` §1.4 `[O]`).
const WATER_EYE_SHIFT: f32 = 10.0;

/// The `SpecularColor` the Water effect's reset stores before a map's `fx__`
/// is read, BF1 `FUN_00489798` 550214–550226 (`re-water-colour.md` §1.4) —
/// the fallback only: every water map authors its own.
const SPECULAR_DEFAULT: [f32; 4] = [192.0, 160.0, 192.0, 192.0];

/// The flat water's per-map colour constants — the VU1 water overlay's
/// constant block, BF1 `FUN_0048c4f0` 551560–551622, BF2 `FUN_00576d28`
/// (`analysis/re-water-colour.md` §1.4/§1.7 `[O]`):
///
/// - `ĥ = normalize(L.x, 0, L.z)`, L = the manifest's `sun_dir`, which is
///   the PS2's own light vector (`analysis/re-sun-direction.md` `[O]`);
/// - routine B's eye shift `10·ĥ` and its weight `SpecularColor / 255`;
/// - routine A's `SpeckleAmbient / 255`, `SpeckleSpecular / 255`, axis
///   `(b4, −b8, b4)` from `LightAzimAndElev` (b4, b8), light
///   `l̂ = normalize(−L.x·b4, −L.y·b8, −L.z·b4)` and shift
///   `(ĥx·SCS.x, 0, ĥz·SCS.y)`, SCS = `SpeckleCoordShift`.
///
/// An ocean (its path never loads the overlay, §1.9), a dry map and a
/// pre-`fx__` bake get the zero default: no view-angle term, no speckle.
/// A missing `SpeckleAmbientColor` is 0, `LightAzimAndElev` (1, 0) (what
/// every map authors), `SpeckleCoordShift` (10, 10) — fallbacks only.
pub fn water_shading(
    lighting: &manifest::ManifestLighting,
    water: Option<&manifest::ManifestWater>,
) -> render::WaterShading {
    if let Some(n) = water
        .filter(|w| w.texture.is_none())
        .and_then(|w| w.config.as_ref())
        .and_then(|c| c.normal_maps.as_ref())
    {
        return render::WaterShading {
            specular: n.reflection.map(|v| (v / 255.0).clamp(0.0, 1.0)),
            speckle_axis: Vec3::new(
                n.fresnel[0].clamp(0.0, 1.0),
                n.fresnel[1].clamp(0.0, 1.0),
                0.0,
            ),
            ..Default::default()
        };
    }
    let Some(c) = water.and_then(|w| w.config.as_ref()).filter(|c| !c.ocean) else {
        return render::WaterShading::default();
    };
    let l = Vec3::from(lighting.sun_dir);
    let h = Vec3::new(l.x, 0.0, l.z).normalize_or_zero();
    let [b4, b8] = c.light_azim_and_elev.unwrap_or([1.0, 0.0]);
    let scs = c.speckle_coord_shift.unwrap_or([10.0, 10.0]);
    let gs = |v: [f32; 4]| v.map(|x| x / 255.0);
    render::WaterShading {
        far_diffuse: render::WaterDiffuse::default(),
        far_range: if c.disable_low_res || lighting.far_scene_range.is_some_and(|f| f <= 0.0) {
            [0.0; 2]
        } else {
            lighting.near_scene_range.map_or([0.0; 2], |r| [r[1], r[2]])
        },
        eye_shift: WATER_EYE_SHIFT * h,
        specular: gs(c.specular_color.unwrap_or(SPECULAR_DEFAULT)),
        speckle_ambient: gs(c.speckle_ambient_color.unwrap_or([0.0; 4])),
        speckle_specular: gs(c.speckle_specular_color.unwrap_or([0.0; 4])),
        speckle_axis: Vec3::new(b4, -b8, b4),
        speckle_light: Vec3::new(-l.x * b4, -l.y * b8, -l.z * b4).normalize_or_zero(),
        speckle_shift: Vec3::new(h.x * scs[0], 0.0, h.z * scs[1]),
    }
}

/// Desert-sky clear color (linear RGB; the sRGB target encodes it) for the
/// render paths with no map behind them (unit sheets, the pose preview).
/// A map's frame clears to [`MAP_CLEAR`] instead.
pub const CLEAR: [f64; 4] = [0.35, 0.52, 0.80, 1.0];

/// What every map's frame clears to: black, as on the PS2. BF1's clear packet
/// `FUN_0049aa68` writes RGBAQ 0 at all four call sites and BF2's
/// `FUN_00581830` does the same to its far target (`analysis/re-fog.md` §9.2,
/// `re-sky-pass.md` item 3(b)); the map's fog colour is read only by the fog
/// passes. Alpha stays 1
/// (the GS's 0 is a destination alpha the port does not composite with).
///
/// It shows only where nothing is drawn — chiefly below the three domes that
/// stop at the horizon (BF1 KAS1, KAS2, BF2 UTA1 — `mungefs --example
/// sky_dome_census`), past the world's edge. Until 2026-09-28 the port cleared
/// to the fog colour, which drew BF1 KAS2's live but never-visible (255,0,0)
/// fog as a pure red band there (card CLRB).
pub const MAP_CLEAR: [f64; 4] = [0.0, 0.0, 0.0, 1.0];

/// What to clear a map's frame to — [`MAP_CLEAR`] on every map.
pub fn clear_color(_scene: &Scene) -> [f64; 4] {
    MAP_CLEAR
}

/// Global fog-density scale. **PROVISIONAL, OURS, and a deliberate departure
/// from the authored values** — the maintainer asked for it by name (2026-08-31): "a
/// 5-10 % reduction in fog density would make the level more enjoyable. Just a
/// little though, not a lot. I think the original actually has a little too
/// much fog anyway."
///
/// 0.92 = an 8 % shallower ramp, the middle of the range he asked for.
///
/// It scales the ramp's SPAN, not the blend factor: `far` moves out to
/// `near + (far - near) / SCALE` and `near` stays put, so a fragment at any
/// distance takes 8 % less fog. (The frame clears to black, as on the PS2 —
/// [`MAP_CLEAR`] — so where the world ends before the sky starts, full fog
/// meets black on both.)
///
/// Since REDB (2026-09-29) the fog itself is the PS2's, and much lighter than
/// the linear-light mix the maintainer judged this against (`analysis/re-fog-amount.md`
/// §5.3; even past `far` it stops at 248/255). Whether he still wants the 8 %
/// off is his call, asked in the REDB report; 1.0 turns it off.
///
/// The manifest keeps every map's authored `FogColor` and `FogRange`
/// untouched; this is applied once, here, where the manifest becomes render
/// state. It is deliberately ONE number and not a per-map table.
///
/// It is NOT related to `bake::lighting::fog_from`, which decides WHICH maps
/// get fog at all and with which numbers — since 2026-09-03 that is the
/// ELF's own rule, one per game (`analysis/re-fog.md` §3/§6). This scale
/// remains a deliberate departure from it, applied on top.
pub const FOG_DENSITY_SCALE: f32 = 0.92;

/// The far-scene cull's two distances for one map, metres from the eye.
///
/// The PS2 draws its world in TWO scenes (`analysis/re-fog.md` §2.5,
/// `analysis/re-lowr.md` §2.2, both [V]). The NEAR scene is a camera frustum
/// whose far plane is `NearSceneRange[2]` and it is where the HEIGHTFIELD
/// stops — the terrain driver's near branch. Past it the FAR scene draws
/// world models (bounding-sphere culled at `FarSceneRange`), the water's
/// low-res pass and the `LOWR` sheet, and nothing else.
///
/// So the port needs two numbers, not one: the terrain's clip is the shorter
/// of the two on every shipped map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarCull {
    /// `NearSceneRange[2]` — where the heightfield ends.
    pub terrain: f32,
    /// Where world models and the water end: `FarSceneRange`, but never
    /// nearer than [`FarCull::terrain`], because the near scene draws them
    /// too and `FarSceneRange` bounds only what is ADDED beyond it. The floor
    /// decides the value on ten of the 32 baked maps: the nine that author
    /// `FarSceneRange(0)` — "no far layer at all" — plus BF1 RHN1, whose
    /// authored 100 m is inside its own 150 m near scene (re-fog.md §6).
    pub world: f32,
}

/// `FarSceneRange` when a map's `sky_` authors none: the ELF's own default
/// (`analysis/re-sky-pass.md` §"Ranges", BF1 `FUN_00479cf0` / 540538-540541,
/// defaults 72/120/2000 [V]). Dead on the shipped discs — all 32 baked maps
/// author the key — and kept only so the rule is the ELF's rather than ours.
const DEFAULT_FAR_SCENE_RANGE: f32 = 2000.0;

/// Is the far-scene cull switched on? `OPENBF_FAR_CULL=1`, and nothing else.
///
/// DEFAULT OFF: the cull is PS2-faithful in WHERE it stops the world, but
/// what it leaves behind is not — the PS2 covers the gap with its `LOWR`
/// sheet and its far-scene buffer, while the port leaves the frame's clear
/// colour. Queue L built the sheet and its landing kept this OFF (PLAN Log
/// 2026-09-17): 18 of the 32 maps draw no sheet, and there the cull still
/// shows the clear colour (BF1 END1 loses 11.8 % of its pixels).
pub fn far_cull_enabled() -> bool {
    std::env::var("OPENBF_FAR_CULL").as_deref() == Ok("1")
}

/// The map's cull distances, or `None` when it takes no cull.
///
/// A manifest with no `NearSceneRange` gets NO cull at all rather than the
/// ELF's 120 m default: a manifest baked before the field existed reads
/// `None` for it, and clipping every map's terrain at 120 m because the bake
/// is stale is a worse failure than not culling. All 32 baked maps author it.
pub fn far_cull_from(lighting: &manifest::ManifestLighting) -> Option<FarCull> {
    let terrain = lighting.near_scene_range?[2];
    if !terrain.is_finite() || terrain <= 0.0 {
        return None;
    }
    let world = lighting
        .far_scene_range
        .filter(|f| f.is_finite())
        .unwrap_or(DEFAULT_FAR_SCENE_RANGE)
        .max(terrain);
    Some(FarCull { terrain, world })
}

/// [`far_cull_from`] behind [`far_cull_enabled`] — what [`upload_scene`] uses.
pub fn far_cull(lighting: &manifest::ManifestLighting) -> Option<FarCull> {
    far_cull_enabled()
        .then(|| far_cull_from(lighting))
        .flatten()
}

/// Is the `LOWR` far-LOD sheet drawn? `OPENBF_LOWRES=1`, and nothing else.
///
/// DEFAULT OFF, and it REQUIRES [`far_cull_enabled`] — [`lowres_range`] takes
/// its near edge from the cull, so with the cull off there is no sheet at
/// all. The two defaults were decided together at queue L's landing, and
/// both stay OFF ([`far_cull_enabled`]).
pub fn lowres_enabled() -> bool {
    std::env::var("OPENBF_LOWRES").as_deref() == Ok("1")
}

/// `LowResTerrain.MaxDistance` for a map whose sky authors the scope but not
/// the key — the ELF's own default, 2000 m in BOTH games
/// (`analysis/re-lowr.md` §0.3 [V], B1:269943 / B2:416065). Nine of the 14
/// drawn sheets take it.
const DEFAULT_LOW_RES_MAX_DISTANCE: f32 = 2000.0;

pub fn terrain_lod_range(
    enabled: bool,
    available: bool,
    lighting: &manifest::ManifestLighting,
    sky: Option<&manifest::ManifestSky>,
) -> Option<(f32, f32)> {
    if !enabled || !available {
        return None;
    }
    lowres_range(far_cull_from(lighting), lighting, sky)
}

/// Where the `LOWR` sheet is drawn: `(near, far)` metres from the eye, or
/// `None` when this map draws none.
///
/// **far** is the PS2's own: `LowResTerrain.MaxDistance`, or 2000 when the
/// scope authors no key. A map whose sky authors NO `LowResTerrain` scope at
/// all draws no sheet — the `tern` load zeroes MaxDistance and only the scope
/// end restores it, and `tern` precedes `sky_` in every LVL, so the gate
/// `0 < MaxDistance` fails (re-lowr.md §0.4 [V loader, D order]). BF2 DAG1
/// and NAB2 ship a sheet the disc never draws for exactly this reason; the
/// bake writes their GLB and this returns `None` for it.
///
/// **near is OURS, and it is a deviation** [D]: the PS2 draws the sheet from
/// `NearSceneRange[1] * zoom`, which OVERLAPS the heightfield by the 30-70 m
/// between `NearSceneRange[1]` and `[2]` on every shipped sheet map (TAT1
/// 100..150, BF1 HOT1 200..270, BES2 120..180). That costs the PS2 nothing:
/// its far scene is a separate buffer the near scene composites OVER, so the
/// heightfield wins the band and the sheet under it is never seen. The port
/// has one depth buffer, so it takes the near edge from the cull instead —
/// [`FarCull::terrain`], where its own heightfield stops.
///
/// Which is why there is no sheet AT ALL without the cull: `cull` is `None`
/// when `OPENBF_FAR_CULL` is off, and the heightfield then runs to the
/// horizon in front of a sheet that is not behind it everywhere. The sheet is
/// a COARSE LOD — it cuts ridge corners, so it stands up to 4-58 m ABOVE the
/// heightfield in places (re-lowr.md §4's dy column) and wins the depth test
/// there. Measured before this gate existed: drawing the sheet with the cull
/// off moved 0.01-3.3 % of the walkabout's pixels on 13 of 20 maps, worst on
/// BF1 HOT1 — the sheet's ridges poking through the ground.
/// Reverse (to draw the disc's own range instead): take
/// `lighting.near_scene_range[1]` here, and expect the z-fight.
///
/// **A map with no far layer draws no sheet** (queue CLN5, [D]): the sheet
/// is a far-scene draw (layer 0x20000, `analysis/re-fog.md` §0 item 2
/// and re-lowr.md §2.1/§3 [V]), and `FarSceneRange(0)` sets the far cull to 0
/// and clears `DAT_0060c305`, which skips the far buffer's composite
/// (`FUN_00581a98`, re-fog.md §2.5 [V]) — so nothing drawn into the far
/// scene reaches the screen, and re-lowr.md §5 already lists END1 among the
/// maps WITHOUT a drawn sheet. BF2 END1 is the one map that both ships a
/// sheet and authors 0; before CLN5 the port drew it through the fade
/// pipeline, invisible (alpha 0 past `NearSceneRange[2]`) but depth-writing.
///
/// **far is capped at the camera's own far plane**, 2000 m
/// (`render::Camera::default`), which every camera takes: TAT1's authored
/// 3000 draws to 2000. A recorded deviation (CLN5 (4)) — raising the plane
/// for every map would move what the default renders past 2000 m, and the
/// sheet is behind `OPENBF_LOWRES`, off by default.
pub fn lowres_range(
    cull: Option<FarCull>,
    lighting: &manifest::ManifestLighting,
    sky: Option<&manifest::ManifestSky>,
) -> Option<(f32, f32)> {
    let sky = sky?;
    // No authored scope = no sheet, whatever the tern ships.
    sky.low_res_terrain.as_ref()?;
    // No far layer = no sheet: nothing in the far scene is composited.
    if lighting.far_scene_range == Some(0.0) {
        return None;
    }
    let far = sky
        .low_res_max_distance
        .filter(|d| d.is_finite())
        .unwrap_or(DEFAULT_LOW_RES_MAX_DISTANCE);
    let near = cull?.terrain;
    (near.is_finite() && near > 0.0 && far > near).then_some((near, far))
}

/// Where the far scene's model / LOWD switch sits (card LOD), from
/// `OPENBF_LOD`: off (`0`, every geometry its model at every distance), the
/// PS2's distance (`ps2`), or the default, classic-plus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodMode {
    Off,
    Ps2,
    ClassicPlus,
}

pub fn lod_mode() -> LodMode {
    match std::env::var("OPENBF_LOD").as_deref() {
        Ok("0") => LodMode::Off,
        Ok("ps2") => LodMode::Ps2,
        _ => LodMode::ClassicPlus,
    }
}

/// How much farther than the PS2 the classic-plus switch sits: a modern GPU
/// draws the full models across the whole near scene and past it, so a LOWD
/// only takes over at twice the near scene's end, where an object is half as
/// tall on screen and the swap is hard to see. PROVISIONAL: chosen by eye on
/// BF2 MUS1; PERF.md's per-map numbers are the check.
pub const CLASSIC_PLUS_LOD_FACTOR: f32 = 2.0;

/// Where a map switches a LOWD-bearing geometry from its model to its `LOWD`
/// (card LOD, `analysis/re-lod.md` §4, §7 [O]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodBands {
    /// `NearSceneRange[1]`: the far scene's reach.
    pub begin: f32,
    /// `NearSceneRange[2]`: the near scene's end — no model past it.
    pub end: f32,
}

impl LodBands {
    /// The switch distance: the PS2 composites its near scene (the model,
    /// to `end`) over its far scene (the LOWD, from `begin`) through a depth
    /// ramp, so the middle of the two is where it reads as switched; the
    /// classic-plus switch is [`CLASSIC_PLUS_LOD_FACTOR`] × `end`.
    ///
    /// `leaf_reach` caps the classic-plus distance for a prop whose canopy
    /// is leaf patches (`leaf::attach_reach`): past their `MaxDistance` the
    /// model is a bare trunk and only the LOWD has the canopy (BF1 NAB1's
    /// ridge trees), so the LOWD takes over there — never nearer than the
    /// PS2's own switch.
    pub fn at(self, mode: LodMode, leaf_reach: Option<f32>) -> f32 {
        let ps2 = 0.5 * (self.begin + self.end);
        match mode {
            LodMode::Ps2 => ps2,
            LodMode::ClassicPlus | LodMode::Off => {
                let plus = CLASSIC_PLUS_LOD_FACTOR * self.end;
                leaf_reach.map_or(plus, |r| plus.min(r).max(ps2))
            }
        }
    }

    /// The model's and the LOWD's sides of the switch for an instance whose
    /// model sphere is `anchor`, `radius` ([`render::LodSwitch`]).
    pub fn sides(
        self,
        mode: LodMode,
        leaf_reach: Option<f32>,
        anchor: glam::Vec3,
        radius: f32,
    ) -> (render::LodSwitch, render::LodSwitch) {
        let side = |side| render::LodSwitch {
            at: self.at(mode, leaf_reach),
            anchor,
            radius,
            side,
        };
        (side(render::LodSide::Model), side(render::LodSide::Lowd))
    }
}

/// The map's [`LodBands`], or `None` when it has no far scene to draw a LOWD
/// in: `FarSceneRange(0)` (nine of the 32 baked maps, re-fog §6), a
/// `FarSceneRange` that does not reach past `NearSceneRange[1]` (BF2 inserts
/// nothing then, re-lod §4.1; BF1's walker culls it), or no `NearSceneRange`.
/// A map that authors no `FarSceneRange` takes the ELF's default, as
/// [`far_cull_from`] does.
///
/// The PS2 scales `NearSceneRange` by the camera's zoom (re-lod §3.1, §4.1:
/// `b·S`); these bands do not — the port's other scene-range culls do not
/// either. A DEVIATION: through a zoomed scope the switch stays at its
/// unzoomed distance.
pub fn lod_bands(lighting: &manifest::ManifestLighting) -> Option<LodBands> {
    let [_, begin, end] = lighting.near_scene_range?;
    let far = lighting.far_scene_range.unwrap_or(DEFAULT_FAR_SCENE_RANGE);
    let ok = |v: f32| v.is_finite() && v > 0.0;
    (ok(begin) && ok(end) && ok(far) && far > begin.min(end)).then_some(LodBands {
        begin: begin.min(end),
        end,
    })
}

/// The near scene's fade into the sky on a BF2 map that draws no far layer,
/// or `None` when the map draws one.
///
/// `analysis/re-fog.md` §2.5/§6 [V code, D look]: `FarSceneRange(0)` gives
/// the map a far-scene cull distance of 0, so NOTHING is drawn into the far
/// buffer, and the near scene — which ends at `NearSceneRange[2]` — is
/// composited over that empty buffer through a depth-keyed alpha ramp between
/// `NearSceneRange[1]` and `[2]`. What shows through is the sky, and the
/// map's authored fog colour is never drawn (§6's "no far layer → no fog
/// drawn" on END1, FEL1, YAV1 and TAT3; DAG1, DEA1 and NAB2 author no fog to
/// begin with and fade the same way).
///
/// **BF2 ONLY, and that is the ELF's asymmetry, not a shortcut.** BF2's fog
/// gate tests the far-scene layer bit — `(flags & 0x20020) == 0x20000` at
/// 723621 — so a BF2 near-scene draw is never fogged and a map with no far
/// layer shows no fog at all. BF1's segment gate has NO layer test
/// (`(flags & 0x20) == 0` at 594590, re-fog.md §2.2 [V]): BF1 fogs the near
/// scene too, so BF1 END1/YAV1/RHN2 keep the fog the port already paints.
/// The BF1 composite still runs — its near scene fades into the far buffer
/// the same way — and modelling that on top of BF1's fog is not this item.
pub fn far_fade_from(
    source_game: &str,
    lighting: &manifest::ManifestLighting,
) -> Option<render::FarFade> {
    if source_game != "bf2" {
        return None;
    }
    // Exactly 0, not "small": the key is authored as a literal 0 on the six
    // BF2 maps that take it (`far_scene_range` in their baked manifests) and
    // every other value — down to TAN1's 150 — means a real far layer.
    if lighting.far_scene_range? != 0.0 {
        return None;
    }
    let [_, begin, end] = lighting.near_scene_range?;
    if !begin.is_finite() || !end.is_finite() || begin < 0.0 || end <= begin {
        return None;
    }
    Some(render::FarFade { begin, end })
}

/// Push the map's baked lighting into the renderer. Called by
/// [`upload_scene`]; kept separate from the upload so a caller can re-apply a
/// map's look without re-uploading it; today `upload_scene` is its only caller.
pub fn apply_lighting(r: &mut Renderer, scene: &Scene) {
    let m = &scene.manifest;
    let tune = look_tune(&m.source_game, &m.map);
    r.lighting = render_lighting_from(&m.source_game, &m.lighting, m.water.as_ref());
    if let (Some(water), Some(mesh)) = (m.water.as_ref(), scene.water.as_ref()) {
        r.lighting.water.far_diffuse =
            crate::water_anim::flat_far_diffuse(mesh, water, &m.lighting).unwrap_or_default();
    }
    r.lighting.water_fog = reference_water_fog(&m.source_game, &m.map, m.water.as_ref());
    r.lighting.water_gain = crate::water_anim::flat_display_gain(m);
    r.lighting.glow_gain = tune.map_or(1.0, |t| t.glow_gain);
    r.set_far_range(m.lighting.far_scene_range.unwrap_or(0.0));
    r.set_frame_filter(frame_filter_from(&m.lighting));
    r.set_softness(match std::env::var("OPENBF_SOFTNESS").as_deref() {
        Ok("1") => m.lighting.softness,
        _ => None,
    });
    r.set_bloom(m.hdr.map(|h| bloom_from(h, tune)));
    r.set_character_light_regions(if m.source_game == "bf2" {
        m.regions
            .iter()
            .filter_map(character_light_region)
            .collect()
    } else {
        Vec::new()
    });
}

fn character_light_region(r: &manifest::ManifestRegion) -> Option<render::CharacterLightRegion> {
    use core::region::{Region, Shape};
    use render::CharacterLightVolume;
    let mut keys = r.name.split_ascii_whitespace();
    if !keys.next()?.eq_ignore_ascii_case("shadow") {
        return None;
    }
    let region = Region::from_rows(&r.shape, r.rotation, r.translation, r.size)?;
    let volume = match region.shape {
        Shape::Box => CharacterLightVolume::Box(region.size),
        Shape::Sphere => CharacterLightVolume::Sphere(region.w),
        Shape::Cylinder => CharacterLightVolume::Cylinder {
            radius: region.size.x,
            half_height: region.size.y,
        },
    };
    let mut light = render::CharacterLightRegion {
        centre: region.center,
        axes: region.basis,
        volume,
        top: [0.0; 3],
        bottom: [0.0; 3],
        directional: [1.0; 2],
    };
    let mut has_lighting = false;
    for (key, value) in keys.filter_map(|key| key.split_once('=')) {
        let rgb = || {
            let mut channels = value.split(',');
            let mut rgb = [0.0; 3];
            for channel in &mut rgb {
                *channel = f32::from(channels.next()?.parse::<u8>().ok()?) / 255.0;
            }
            channels.next().is_none().then_some(rgb)
        };
        match key.to_ascii_lowercase().as_str() {
            "colortop" => light.top = rgb()?,
            "colorbottom" => light.bottom = rgb()?,
            "directional" => light.directional[0] = value.parse::<f32>().ok()?.clamp(0.0, 1.0),
            "directional1" => light.directional[1] = value.parse::<f32>().ok()?.clamp(0.0, 1.0),
            _ => continue,
        }
        has_lighting = true;
    }
    has_lighting.then_some(light)
}

pub(super) fn atmosphere_display_gain(m: &manifest::Manifest) -> f32 {
    if m.hdr.is_some() {
        2.0
    } else {
        1.0
    }
}

pub(super) fn hallway_lamp_bloom(m: &manifest::Manifest, material: Option<&str>) -> f32 {
    if m.source_game == "bf2" && m.map == "mus1" && material == Some("mus1_bldg_control06") {
        (30.0_f32 / 28.0).powi(10)
    } else {
        1.0
    }
}

fn reference_water_fog(
    source_game: &str,
    map: &str,
    water: Option<&manifest::ManifestWater>,
) -> Option<render::Fog> {
    if source_game != "bf2" || map != "kas2" {
        return None;
    }
    let water = water?;
    let cfg = water.config.as_ref()?;
    if cfg.disable_low_res || !cfg.specular_enable {
        return None;
    }
    Some(render::Fog {
        color: water.far_fog_color?.map(|c| render::srgb_decode(c / 255.0)),
        near: 90.0,
        far: 350.0,
    })
}

/// [ours] A map whose authored look the maintainer asked to soften: factors
/// on the disc's own values, never replacements. One row per map in
/// [`LOOK_TUNES`]; every other map draws exactly as authored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookTune {
    pub game: &'static str,
    pub map: &'static str,
    /// Times the HDR block's `MaxTotalWeight` (the bloom's strength).
    pub bloom_weight: f32,
    /// Times the self-lit surfaces' display gain ([`render::Lighting::glow_gain`]).
    pub glow_gain: f32,
}

/// The per-map tunes (FEEL_NOTES 2026-10-29):
/// - BF2 Mustafar: "the bloom and coloring is too high ... lower the lava
///   brightness a little". The lava is self-lit (BF2 material 0x10, drawn
///   at up to x2.98 and the bloom's source). Gain x0.9 and weight 1.2 ->
///   1.116 (tap 30 -> 28 of 128; the ten blur sub-passes compound it): the
///   lava reads 18-26 % darker in three offscreen views, still the brightest
///   thing on the map.
pub const LOOK_TUNES: &[LookTune] = &[LookTune {
    game: "bf2",
    map: "mus1",
    bloom_weight: 0.93,
    glow_gain: 0.9,
}];

/// The [`LOOK_TUNES`] row for a map, by the manifest's `source_game` and `map`.
pub fn look_tune(game: &str, map: &str) -> Option<&'static LookTune> {
    LOOK_TUNES
        .iter()
        .find(|t| t.game.eq_ignore_ascii_case(game) && t.map.eq_ignore_ascii_case(map))
}

pub struct SkyLayerTune {
    pub game: &'static str,
    pub map: &'static str,
    pub geometry: &'static str,
    pub opacity: f32,
}

pub const SKY_LAYER_TUNES: &[SkyLayerTune] = &[SkyLayerTune {
    game: "bf2",
    map: "fel1",
    geometry: "fel_sky_dome_trees",
    opacity: 0.2,
}];

pub fn sky_layer_opacity(game: &str, map: &str, geometry: &str) -> f32 {
    SKY_LAYER_TUNES
        .iter()
        .find(|tune| {
            tune.game.eq_ignore_ascii_case(game)
                && tune.map.eq_ignore_ascii_case(map)
                && tune.geometry.eq_ignore_ascii_case(geometry)
        })
        .map_or(1.0, |tune| tune.opacity)
}

/// The map's authored HDR block as the renderer's bloom, under its tune.
pub fn bloom_from(h: manifest::ManifestHdr, tune: Option<&LookTune>) -> render::Bloom {
    render::Bloom {
        passes: h.num_bloom_passes,
        weight: h.max_total_weight * tune.map_or(1.0, |t| t.bloom_weight),
        threshold: h.glow_threshold,
    }
}

/// The map's DomeInfo frame filter as the renderer takes it (card BFLT,
/// `analysis/re-glow.md` §3 `[O]`): the bake keeps it only where the PS2
/// draws it (`Filter` ≠ 0, `Intensity` ≥ 1), on both games.
pub fn frame_filter_from(l: &manifest::ManifestLighting) -> Option<render::FrameFilter> {
    l.filter.map(|f| render::FrameFilter {
        threshold: f.threshold,
        intensity: f.intensity,
    })
}

/// The manifest's look as render state — [`apply_lighting`] without the
/// renderer, so the fog/fade decision is testable without a GPU.
pub fn render_lighting_from(
    source_game: &str,
    l: &manifest::ManifestLighting,
    water: Option<&manifest::ManifestWater>,
) -> render::Lighting {
    // The fade REPLACES the fog where it applies: on a BF2 map with no far
    // layer the disc draws no fog colour at all, so painting both would paint
    // the very wall the fade exists to remove ([`far_fade_from`]).
    let far_fade = far_fade_from(source_game, l);
    let water_look = water.map(|w| water_look(source_game, l, w));
    render::Lighting {
        sun_dir: Vec3::from(l.sun_dir).normalize_or_zero(),
        sun_color: l.sun_color,
        // BF1's prelit (0x40) set carries no sun (`analysis/re-sun-direction.md`
        // §1.10 `[O]`); BF2's skips a `Static()` one. BF1's term is d⁴ (§1.8).
        sun_static: l.sun_static || source_game == "bf1",
        sun_d4: source_game == "bf1",
        light2: l.light2.map(|d| render::DirLight {
            dir: Vec3::from(d.dir).normalize_or_zero(),
            color: d.color,
            is_static: d.is_static,
        }),
        ambient_sky: l.ambient_sky,
        ambient_ground: l.ambient_ground,
        // BF1 lights a soldier with its own ambient (`analysis/re-model-lighting.md`
        // §12 `[O]`); an absent key leaves the soldier on `AmbientColor`, as
        // FUN_00493298 copies it into every slot. BF2 has no such split (§3).
        character_ambient: if source_game == "bf1" {
            l.character_ambient
        } else {
            None
        },
        fog: match far_fade {
            Some(_) => None,
            None => l.fog.map(|f| render::Fog {
                color: f.color,
                near: f.near,
                far: f.near + (f.far - f.near) / FOG_DENSITY_SCALE,
            }),
        },
        far_fade,
        // The water's fog gate is NOT the world's: see [`water_look`]. A map
        // with no water leaves it `None`, which is what every non-water draw
        // already ignores.
        water_fog_begin: water_look.as_ref().and_then(|w| w.fog_begin),
        water_fog: None,
        water_gain: 1.0,
        water_lit: water_look.as_ref().map_or(0.0, |w| w.sun),
        water_dim: water_look.as_ref().and_then(|w| w.dim),
        water: water_shading(l, water),
        glow_gain: 1.0,
    }
}

#[cfg(test)]
mod tests;
