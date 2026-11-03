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
    r.lighting.glow_gain = tune.map_or(1.0, |t| t.glow_gain);
    r.set_frame_filter(frame_filter_from(&m.lighting));
    r.set_bloom(m.hdr.map(|h| bloom_from(h, tune)));
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
        water_lit: water_look.as_ref().map_or(0.0, |w| w.sun),
        water_dim: water_look.as_ref().and_then(|w| w.dim),
        water: water_shading(l, water),
        glow_gain: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR: manifest::ManifestHdr = manifest::ManifestHdr {
        num_bloom_passes: 5,
        max_total_weight: 1.2,
        glow_threshold: 0.5,
    };

    /// Mustafar's tune softens its bloom and self-lit gain; the HDR block's
    /// other numbers are the disc's.
    #[test]
    fn mustafar_takes_its_softer_look() {
        let t = look_tune("bf2", "mus1").expect("mus1 is tuned");
        assert_eq!((t.bloom_weight, t.glow_gain), (0.93, 0.9));
        let b = bloom_from(HDR, Some(t));
        assert_eq!((b.passes, b.threshold), (5, 0.5));
        assert!((b.weight - 1.116).abs() < 1e-5, "{}", b.weight);
    }

    /// Every other map, HDR ones with Mustafar's very block included, draws
    /// exactly as authored.
    #[test]
    fn no_other_map_is_tuned() {
        assert_eq!(LOOK_TUNES.len(), 1);
        for (game, map) in [
            ("bf2", "kas2"),
            ("bf2", "nab2"),
            ("bf2", "fel1"),
            ("bf1", "mus1"),
        ] {
            assert_eq!(look_tune(game, map), None, "{game} {map}");
            let b = bloom_from(HDR, look_tune(game, map));
            assert_eq!(
                b,
                render::Bloom {
                    passes: 5,
                    weight: 1.2,
                    threshold: 0.5
                },
                "{game} {map}"
            );
        }
        let l = render_lighting_from("bf2", &ranges(None, None), None);
        assert_eq!(l.glow_gain, 1.0);
    }

    /// The sheet's range, on the numbers the discs author
    /// (`analysis/re-lowr.md` §4's table). The rule is pure — the env gate is
    /// the only thing between it and `upload_scene`.
    #[test]
    fn the_sheet_runs_from_the_heightfields_end_to_maxdistance() {
        let with_far = |near: Option<[f32; 3]>,
                        far_scene: Option<f32>,
                        tex: Option<&str>,
                        max: Option<f32>| {
            lowres_range(
                near.map(|n| FarCull {
                    terrain: n[2],
                    world: n[2],
                }),
                &manifest::ManifestLighting {
                    near_scene_range: near,
                    far_scene_range: far_scene,
                    ..manifest::ManifestLighting::default()
                },
                Some(&manifest::ManifestSky {
                    texture: None,
                    models: Vec::new(),
                    ambient: [0.0; 3],
                    low_res_terrain: tex.map(str::to_owned),
                    low_res_max_distance: max,
                    objects: Vec::new(),
                    terrain_enable: None,
                    reflection_color: None,
                    stars: None,
                }),
            )
        };
        let with = |near: Option<[f32; 3]>, tex: Option<&str>, max: Option<f32>| {
            with_far(near, Some(2000.0), tex, max)
        };

        // BF1 TAT1: NearSceneRange(1, 100, 150), MaxDistance 3000 — the sheet
        // starts where the heightfield stops, not at the authored 100.
        assert_eq!(
            with(Some([1.0, 100.0, 150.0]), Some("tat1"), Some(3000.0)),
            Some((150.0, 3000.0))
        );
        // BF1 BES2's authored 400, and BF1 HOT1, which authors the scope but
        // no MaxDistance and so takes the ELF's 2000.
        assert_eq!(
            with(
                Some([0.8, 120.0, 180.0]),
                Some("bespin2_lowresterrain"),
                Some(400.0)
            ),
            Some((180.0, 400.0))
        );
        assert_eq!(
            with(Some([1.0, 200.0, 270.0]), Some("hoth"), None),
            Some((270.0, DEFAULT_LOW_RES_MAX_DISTANCE))
        );

        // BF2 DAG1 and NAB2: a sheet in the `tern`, no `LowResTerrain` scope
        // in the sky, so the disc never draws it and neither do we.
        assert_eq!(with(Some([0.2, 20.0, 60.0]), None, None), None);
        // BF2 END1: it ships a sheet AND names it (`end1_lowresterrain`, no
        // MaxDistance), but authors `FarSceneRange(0)` — no far layer, so the
        // disc composites nothing from the far scene and draws no sheet.
        // Its NearSceneRange (0.7, 90, 150) from the baked manifest.
        assert_eq!(
            with_far(
                Some([0.7, 90.0, 150.0]),
                Some(0.0),
                Some("end1_lowresterrain"),
                None
            ),
            None
        );
        // The same map with any real far layer would draw it.
        assert_eq!(
            with_far(
                Some([0.7, 90.0, 150.0]),
                Some(150.0),
                Some("end1_lowresterrain"),
                None
            ),
            Some((150.0, DEFAULT_LOW_RES_MAX_DISTANCE))
        );
        // No cull — `OPENBF_FAR_CULL` off, or a pre-item-5 manifest with no
        // `NearSceneRange`: no sheet, because there is no far scene to fill
        // and the coarse LOD would poke through the ground instead.
        assert_eq!(with(None, Some("tat1"), Some(3000.0)), None);
        // A MaxDistance inside the near scene draws nothing — no shipped map
        // authors one, and the ELF's own gate is `0 < MaxDistance`.
        assert_eq!(
            with(Some([1.0, 100.0, 150.0]), Some("tat1"), Some(0.0)),
            None
        );
        assert_eq!(
            with(Some([1.0, 100.0, 150.0]), Some("tat1"), Some(120.0)),
            None
        );
    }

    /// The cull's two distances, on the numbers the discs actually author
    /// (`analysis/re-fog.md` §6's table). The rule is pure — the env gate is
    /// the only thing between it and `upload_scene`.
    #[test]
    fn the_cull_takes_the_terrain_to_the_near_scene_and_the_world_to_the_far() {
        let with = |near: Option<[f32; 3]>, far: Option<f32>| {
            far_cull_from(&manifest::ManifestLighting {
                near_scene_range: near,
                far_scene_range: far,
                ..manifest::ManifestLighting::default()
            })
        };

        // BF1 TAT1: NearSceneRange(1, 100, 150), FarSceneRange 2000 — the
        // values `mungefs/tests/suite/tat1_lvl.rs` pins against the disc bytes.
        assert_eq!(
            with(Some([1.0, 100.0, 150.0]), Some(2000.0)),
            Some(FarCull {
                terrain: 150.0,
                world: 2000.0
            })
        );
        // BF1 KAS2 (1500) and BF2 KAM1 (800), the two maps the gate scans.
        assert_eq!(
            with(Some([1.0, 80.0, 100.0]), Some(1500.0)).unwrap().world,
            1500.0
        );
        assert_eq!(
            with(Some([1.0, 60.0, 90.0]), Some(800.0)).unwrap().terrain,
            90.0
        );

        // `FarSceneRange(0)` is "no far layer", NOT "cull everything at 0":
        // the near scene still draws models and water to NearSceneRange[2].
        assert_eq!(
            with(Some([1.0, 100.0, 150.0]), Some(0.0)),
            Some(FarCull {
                terrain: 150.0,
                world: 150.0
            })
        );
        // An unauthored FarSceneRange takes the ELF's 2000 m default.
        assert_eq!(with(Some([1.0, 100.0, 150.0]), None).unwrap().world, 2000.0);

        // No NearSceneRange (a manifest baked before the field existed) =
        // no cull at all, so a stale bake renders unchanged with the flag on.
        assert_eq!(with(None, Some(1500.0)), None);
        // Degenerate values are refused rather than deleting the world.
        assert_eq!(with(Some([1.0, 100.0, 0.0]), Some(1500.0)), None);
        assert_eq!(with(Some([1.0, 100.0, -5.0]), Some(1500.0)), None);
        assert_eq!(with(Some([1.0, 100.0, f32::NAN]), Some(1500.0)), None);
    }

    /// Which maps fade into the sky instead of into a fog colour, on the
    /// numbers the discs author (`analysis/re-fog.md` §6's table, which is
    /// what the baked manifests carry).
    #[test]
    fn only_a_bf2_map_with_no_far_layer_fades_into_the_sky() {
        let with = |game: &str, near: Option<[f32; 3]>, far: Option<f32>| {
            far_fade_from(
                game,
                &manifest::ManifestLighting {
                    near_scene_range: near,
                    far_scene_range: far,
                    ..manifest::ManifestLighting::default()
                },
            )
        };
        let fade = |begin: f32, end: f32| Some(render::FarFade { begin, end });

        // The six baked BF2 maps that author FarSceneRange(0). The first
        // three also author fog, which is the deviation this replaces
        // (Felucia's green-grey wall, Endor's grey one); the last three
        // author none and fade into the sky just the same.
        for (map, near, b, c) in [
            ("END1", [0.7, 90.0, 150.0], 90.0, 150.0),
            ("FEL1", [0.1, 28.0, 52.0], 28.0, 52.0),
            ("YAV1", [0.7, 50.0, 100.0], 50.0, 100.0),
            ("DAG1", [0.2, 20.0, 60.0], 20.0, 60.0),
            ("DEA1", [0.1, 120.0, 180.0], 120.0, 180.0),
            ("NAB2", [1.75, 100.0, 150.0], 100.0, 150.0),
        ] {
            assert_eq!(with("bf2", Some(near), Some(0.0)), fade(b, c), "BF2 {map}");
        }

        // BF1 END1 (100..150), YAV1 (50..100) and RHN2 (90..110) author
        // FarSceneRange(0) too and do NOT fade: BF1's fog gate has no layer
        // test, so BF1 fogs its near scene (re-fog.md §2.2 [V]).
        assert_eq!(with("bf1", Some([0.7, 100.0, 150.0]), Some(0.0)), None);
        assert_eq!(with("bf1", Some([0.7, 50.0, 100.0]), Some(0.0)), None);
        assert_eq!(with("bf1", Some([0.7, 90.0, 110.0]), Some(0.0)), None);

        // A BF2 map with a real far layer never fades — TAT2's 3000 m and
        // TAN1's 150 m are both far layers, however short.
        assert_eq!(with("bf2", Some([0.1, 70.0, 100.0]), Some(3000.0)), None);
        assert_eq!(with("bf2", Some([0.5, 60.0, 110.0]), Some(150.0)), None);
        // An unauthored FarSceneRange is not a zero one.
        assert_eq!(with("bf2", Some([0.1, 100.0, 150.0]), None), None);
        // Missing or degenerate NearSceneRange: no fade, rather than a band
        // that swallows the map.
        assert_eq!(with("bf2", None, Some(0.0)), None);
        assert_eq!(with("bf2", Some([0.1, 100.0, 100.0]), Some(0.0)), None);
        assert_eq!(with("bf2", Some([0.1, 150.0, 100.0]), Some(0.0)), None);
        assert_eq!(with("bf2", Some([0.1, 100.0, f32::NAN]), Some(0.0)), None);
    }

    /// The fade and the fog are mutually exclusive, and only the fade maps
    /// lose their fog: every other map keeps the exact ramp it had.
    #[test]
    fn the_fade_replaces_the_fog_and_only_on_the_maps_that_take_it() {
        let fogged = manifest::ManifestFog {
            color: [0.2, 0.3, 0.4],
            near: 90.0,
            far: 350.0,
        };

        // BF2 END1: fog authored, no far layer -> the fade, and NO fog.
        let l = manifest::ManifestLighting {
            near_scene_range: Some([0.7, 90.0, 150.0]),
            far_scene_range: Some(0.0),
            fog: Some(fogged),
            ..manifest::ManifestLighting::default()
        };
        let look = render_lighting_from("bf2", &l, None);
        assert_eq!(
            look.far_fade,
            Some(render::FarFade {
                begin: 90.0,
                end: 150.0
            })
        );
        assert_eq!(look.fog, None, "the disc draws no fog colour on END1");

        // BF1 END1: the same numbers on the other disc keep the fog ramp,
        // stretched by FOG_DENSITY_SCALE exactly as before.
        let look = render_lighting_from("bf1", &l, None);
        assert_eq!(look.far_fade, None);
        let fog = look.fog.expect("BF1 keeps its fog");
        assert_eq!(fog.near, 90.0);
        assert!((fog.far - (90.0 + (350.0 - 90.0) / FOG_DENSITY_SCALE)).abs() < 1e-3);

        // BF2 HOT1: fog and a real far layer -> unchanged, fade off.
        let look = render_lighting_from(
            "bf2",
            &manifest::ManifestLighting {
                near_scene_range: Some([0.5, 200.0, 270.0]),
                far_scene_range: Some(800.0),
                fog: Some(fogged),
                ..manifest::ManifestLighting::default()
            },
            None,
        );
        assert_eq!(look.far_fade, None);
        assert_eq!(look.fog.map(|f| f.near), Some(90.0));
    }

    /// BF1 lights its soldiers with `CharacterAmbientColor`
    /// (`analysis/re-model-lighting.md` §12); BF2 has one ambient for every
    /// draw, so the same manifest field is dropped there.
    #[test]
    fn only_bf1_takes_the_character_ambient() {
        // BF1 GEO1's baked pair.
        let l = manifest::ManifestLighting {
            ambient_sky: [0.187_820_76, 0.130_136_49, 0.072_271_86],
            ambient_ground: [0.187_820_76, 0.130_136_49, 0.072_271_86],
            character_ambient: Some([0.637_596_96, 0.332_451_58, 0.066_625_95]),
            ..manifest::ManifestLighting::default()
        };
        let bf1 = render_lighting_from("bf1", &l, None);
        assert_eq!(bf1.character_ambient, l.character_ambient);
        assert_eq!(
            bf1.ambient_sky, l.ambient_sky,
            "the world keeps AmbientColor"
        );
        assert_eq!(
            render_lighting_from("bf2", &l, None).character_ambient,
            None
        );
        let none = manifest::ManifestLighting {
            character_ambient: None,
            ..l
        };
        assert_eq!(
            render_lighting_from("bf1", &none, None).character_ambient,
            None
        );
    }

    /// One water config helper for the cases below: the LUT ends and the two
    /// switches, exactly as the twelve baked manifests carry them.
    fn water_cfg(
        min: [f32; 4],
        max: [f32; 4],
        ocean: bool,
        disable_low_res: bool,
    ) -> manifest::ManifestWater {
        manifest::ManifestWater {
            glb: "water.glb".into(),
            texture: Some("kas2_water".into()),
            height: -5.0,
            uv_velocity: [0.0125, 0.0125],
            layer_color: [0; 4],
            wet_cells: 1,
            patches: 0,
            triangles: 2,
            config: Some(manifest::ManifestWaterConfig {
                main_texture: Some("kas2_water".into()),
                foam_texture: ocean.then(|| "kam1_water".to_string()),
                min_diffuse: Some(min),
                max_diffuse: Some(max),
                ocean,
                disable_low_res,
                // The PS2's reset value, which every baked manifest carries.
                specular_enable: true,
                oscillation_enable: true,
                ..manifest::ManifestWaterConfig::default()
            }),
            wet: None,
        }
    }

    /// The speckle layer on BF1 KAS2's authored numbers (the manifest's
    /// `fx__` block): 25 frames at 4 fps, `SpeckleTile(5, 5)` over 128 m cells
    /// whose mesh UV repeats every 16 m.
    #[test]
    fn the_speckle_takes_the_maps_tile_and_rate() {
        let mut w = water_cfg([0.0; 4], [0.0; 4], false, false);
        let c = w.config.as_mut().unwrap();
        c.metres_per_repeat = Some([16.0, 16.0]);
        c.speckle_textures = (0..25).map(|i| format!("water_specularmask_{i}")).collect();
        c.speckle_fps = Some(4.0);
        c.speckle_tile = Some([5.0, 5.0]);
        c.speckle_scroll = Some([0.11, 0.11]);
        c.speckle_specular_color = Some([200.0, 200.0, 200.0, 150.0]);
        let s = speckle_look("bf1", &w, 128.0).expect("BF1 flat water speckles");
        assert_eq!(s.frames.len(), 25);
        assert_eq!(s.fps, 4.0);
        assert_eq!(s.uv_scale, [0.625, 0.625], "5 repeats per 128 m cell");
        assert_eq!(s.scroll, [0.11, 0.11]);

        // BF2 on the flag's reset value (every shipped BF2 map) draws the
        // same layer; an authored `SpecularEnable(0)`, an ocean and no frames
        // draw none.
        assert_eq!(speckle_look("bf2", &w, 128.0), Some(s));
        w.config.as_mut().unwrap().specular_enable = false;
        assert_eq!(speckle_look("bf2", &w, 128.0), None);
        assert!(speckle_look("bf1", &w, 128.0).is_some(), "BF1 has no gate");
        w.config.as_mut().unwrap().specular_enable = true;
        w.config.as_mut().unwrap().ocean = true;
        assert_eq!(speckle_look("bf1", &w, 128.0), None);
        w.config.as_mut().unwrap().ocean = false;
        w.config.as_mut().unwrap().speckle_textures.clear();
        assert_eq!(speckle_look("bf1", &w, 128.0), None);
    }

    /// The flipbook: ⌊t·fps⌋ mod n and the next, cross-faded by the fraction.
    #[test]
    fn the_speckle_frames_wrap_and_cross_fade() {
        assert_eq!(speckle_frames(0.0, 4.0, 25), (0, 1, 0.0));
        assert_eq!(speckle_frames(0.375, 4.0, 25), (1, 2, 0.5));
        assert_eq!(speckle_frames(6.0, 4.0, 25), (24, 0, 0.0));
        assert_eq!(speckle_frames(6.25, 4.0, 25), (0, 1, 0.0));
        assert_eq!(speckle_frames(1.0, 4.0, 0), (0, 0, 0.0));
    }

    /// The water's look, on the four shapes the discs ship
    /// (`analysis/re-water.md` §1/§3, the §5 census).
    #[test]
    fn the_water_takes_the_luts_mean_and_the_discs_own_fog_rule() {
        // The LUT every map but BF2 KAS2 and BF2 DEA1 authors.
        let (min, max) = ([32.0, 40.0, 32.0, 188.0], [128.0, 130.0, 128.0, 188.0]);

        // BF1 KAS2: far water on, so the fog starts where the near scene
        // ends (100 m) and the surface reaches 0.9 * 1500.
        let l = manifest::ManifestLighting {
            near_scene_range: Some([2.0, 80.0, 100.0]),
            far_scene_range: Some(1500.0),
            ..manifest::ManifestLighting::default()
        };
        let w = water_look("bf1", &l, &water_cfg(min, max, false, false));
        // The LUT's mean is MaxDiffuse, in GS terms (byte / 255, gamma —
        // `fs_water` linearises it after the view-angle term).
        assert_eq!(w.tint[..3], [128.0 / 255.0, 130.0 / 255.0, 128.0 / 255.0]);
        assert!(
            (w.tint[3] - 188.0 / 255.0).abs() < 1e-6,
            "alpha is a weight"
        );
        assert_eq!(w.fog_begin, Some(100.0));
        assert_eq!(w.far, Some(1350.0));
        assert_eq!(w.foam_tint, None, "flat water has no foam layer");

        // BF1 KAS1: `DisableLowRes` -> no far pass at all, so nothing to fog
        // and the surface stops with the near scene.
        let l = manifest::ManifestLighting {
            near_scene_range: Some([0.7, 150.0, 180.0]),
            far_scene_range: Some(1000.0),
            ..manifest::ManifestLighting::default()
        };
        let w = water_look("bf1", &l, &water_cfg(min, max, false, true));
        assert_eq!(w.fog_begin, None);
        assert_eq!(w.far, Some(180.0));

        // BF2 KAS2's twin numbers: `SpecularEnable` is on (the reset's 1),
        // so the far water takes the fog past the near scene as BF1's does;
        // an authored 0 or `DisableLowRes` fogs none.
        let l = manifest::ManifestLighting {
            near_scene_range: Some([0.5, 85.0, 100.0]),
            far_scene_range: Some(5000.0),
            ..manifest::ManifestLighting::default()
        };
        let w = water_look("bf2", &l, &water_cfg(min, max, false, false));
        assert_eq!(w.fog_begin, Some(100.0));
        assert_eq!(w.far, Some(4500.0));
        let mut off = water_cfg(min, max, false, false);
        off.config.as_mut().unwrap().specular_enable = false;
        assert_eq!(water_look("bf2", &l, &off).fog_begin, None);
        assert_eq!(
            water_look("bf1", &l, &off).fog_begin,
            Some(100.0),
            "BF1 has no gate"
        );
        let w = water_look("bf2", &l, &water_cfg(min, max, false, true));
        assert_eq!(w.fog_begin, None, "DisableLowRes: no far water to fog");
        let w = water_look("bf2", &l, &water_cfg(min, max, true, false));
        assert_eq!(
            w.fog_begin,
            Some(100.0),
            "the gate is the flat renderer's, not the ocean's"
        );
        let mut ocean_off = water_cfg(min, max, true, false);
        ocean_off.config.as_mut().unwrap().specular_enable = false;
        assert_eq!(water_look("bf2", &l, &ocean_off).fog_begin, Some(100.0));

        // BF2 NAB2: `FarSceneRange(0)` floors the water at the near scene's
        // own end rather than at zero.
        let l = manifest::ManifestLighting {
            near_scene_range: Some([1.75, 100.0, 150.0]),
            far_scene_range: Some(0.0),
            ..manifest::ManifestLighting::default()
        };
        assert_eq!(
            water_look("bf2", &l, &water_cfg(min, max, false, false)).far,
            Some(150.0)
        );

        // BF1 KAM1, the ocean: base OPAQUE and at UNITY whatever the LUT
        // says (the engine's white halved), the foam at unity too, and 0.8
        // of the far range.
        let l = manifest::ManifestLighting {
            near_scene_range: Some([0.8, 60.0, 90.0]),
            far_scene_range: Some(800.0),
            ..manifest::ManifestLighting::default()
        };
        let w = water_look(
            "bf1",
            &l,
            &water_cfg(min, [128.0, 130.0, 128.0, 60.0], true, false),
        );
        assert_eq!(w.tint, [1.0; 4], "unity, and the base record is 0x4100");
        assert_eq!(w.foam_tint, Some([1.0; 4]));
        assert_eq!(w.far, Some(640.0));
        assert_eq!((w.sun, w.dim), (0.0, None), "BF1's ocean is unlit");

        // BF2 KAM1 (card KSEAL): the same ocean is lit by the sun, its foam
        // blends at half weight, and from FogRange's 400 m it loses colour
        // down to half at the far pass's 640 m. Without a fog there is no
        // distance to start from, and no loss.
        let ocean = water_cfg(min, [128.0, 130.0, 128.0, 60.0], true, false);
        let fogged = manifest::ManifestLighting {
            fog: Some(manifest::ManifestFog {
                color: [0.0; 3],
                near: 400.0,
                far: 1356.0,
            }),
            ..l.clone()
        };
        let w = water_look("bf2", &fogged, &ocean);
        assert_eq!(w.tint, [1.0; 4]);
        assert_eq!(w.foam_tint, Some([1.0, 1.0, 1.0, OCEAN_LIT_FOAM_ALPHA]));
        assert_eq!(w.sun, OCEAN_SUN_WEIGHT);
        let [begin, rate] = w.dim.expect("a lit ocean under a fog dims");
        assert_eq!(begin, 400.0);
        assert!((1.0 - rate * (640.0 - begin) - OCEAN_FAR_KEEP).abs() < 1e-6);
        assert_eq!(water_look("bf2", &l, &ocean).dim, None);
        assert_eq!(water_look("bf1", &fogged, &ocean).dim, None);

        // A manifest baked before the `fx__` reader landed: no tint, no foam,
        // and the two distances still come from `sky_`.
        let mut old = water_cfg(min, max, false, false);
        old.config = None;
        let w = water_look("bf1", &l, &old);
        assert_eq!(w.tint, [1.0; 4], "no config invents no colour");
        assert_eq!(w.foam_tint, None);
    }

    /// The overlay's constant block on BF1 YAV1's numbers: the baked sun IS
    /// the PS2's L (the RAM vector of the YAV1 state, `re-water-colour.md`
    /// §1.4), ĥ its horizontal, and the authored colours / 255. An ocean and
    /// a dry map get no term at all.
    #[test]
    fn the_water_shading_is_the_overlays_constant_block() {
        let l = manifest::ManifestLighting {
            sun_dir: [-0.556_670_4, -0.766_044_4, 0.321_393_8],
            ..manifest::ManifestLighting::default()
        };
        let ram = Vec3::new(-0.5567, -0.766, 0.3214);

        let mut w = water_cfg(
            [32.0, 40.0, 32.0, 255.0],
            [128.0, 130.0, 128.0, 255.0],
            false,
            false,
        );
        let c = w.config.as_mut().unwrap();
        c.specular_color = Some([192.0; 4]);
        c.speckle_ambient_color = Some([0.0, 0.0, 0.0, 255.0]);
        c.speckle_specular_color = Some([160.0, 160.0, 160.0, 255.0]);
        c.speckle_coord_shift = Some([10.0, 10.0]);
        c.light_azim_and_elev = Some([1.0, 0.0]);
        let s = water_shading(&l, Some(&w));
        let h = Vec3::new(ram.x, 0.0, ram.z).normalize();
        assert!(
            (s.eye_shift - 10.0 * h).abs().max_element() < 1e-3,
            "{:?}",
            s.eye_shift
        );
        assert_eq!(s.specular, [192.0 / 255.0; 4]);
        assert_eq!(s.speckle_ambient, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(
            s.speckle_specular,
            [160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 1.0]
        );
        assert_eq!(s.speckle_axis, Vec3::new(1.0, -0.0, 1.0));
        // LightAzimAndElev (1, 0): l̂ = −ĥ, the lobe points at the sun's azimuth.
        assert!((s.speckle_light + h).abs().max_element() < 1e-3);
        assert!((s.speckle_shift - 10.0 * h).abs().max_element() < 1e-3);

        w.config.as_mut().unwrap().ocean = true;
        assert_eq!(water_shading(&l, Some(&w)), render::WaterShading::default());
        assert_eq!(water_shading(&l, None), render::WaterShading::default());
    }

    fn ranges(near: Option<[f32; 3]>, far: Option<f32>) -> manifest::ManifestLighting {
        manifest::ManifestLighting {
            near_scene_range: near,
            far_scene_range: far,
            ..manifest::ManifestLighting::default()
        }
    }

    /// Card LOD: the switch's band is `NearSceneRange[1]..[2]` (BF1 NAB1
    /// authors 100 / 130 / 1000); a map with no far scene draws no LOWD.
    #[test]
    fn lod_bands_come_from_the_scene_ranges() {
        let nab1 = lod_bands(&ranges(Some([1.0, 100.0, 130.0]), Some(1000.0)));
        assert_eq!(
            nab1,
            Some(LodBands {
                begin: 100.0,
                end: 130.0
            })
        );
        // FarSceneRange(0): no far layer, no LOWD (BF1 YAV1 50 / 100 / 0).
        assert_eq!(
            lod_bands(&ranges(Some([1.0, 50.0, 100.0]), Some(0.0))),
            None
        );
        // A far scene that never reaches past NearSceneRange[1].
        assert_eq!(
            lod_bands(&ranges(Some([1.0, 100.0, 150.0]), Some(100.0))),
            None
        );
        // No NearSceneRange (a stale bake): nothing to switch on.
        assert_eq!(lod_bands(&ranges(None, Some(1000.0))), None);
        // No FarSceneRange authored: the ELF's default, which reaches past.
        assert!(lod_bands(&ranges(Some([1.0, 100.0, 130.0]), None)).is_some());
        let anchor = glam::Vec3::new(1.0, 2.0, 3.0);
        let (m, l) = nab1.unwrap().sides(LodMode::ClassicPlus, None, anchor, 5.0);
        assert_eq!(
            (m.side, l.side),
            (render::LodSide::Model, render::LodSide::Lowd)
        );
        assert_eq!((m.at, m.anchor, m.radius), (l.at, l.anchor, l.radius));
        // Classic-plus switches at twice the near scene's end; the PS2 mode
        // between the far scene's reach and the near scene's end.
        assert_eq!(m.at, 260.0);
        assert_eq!(nab1.unwrap().at(LodMode::Ps2, None), 115.0);
        // A leaf-patch canopy caps classic-plus at its reach, never nearer
        // than the PS2's switch.
        let b = nab1.unwrap();
        assert_eq!(b.at(LodMode::ClassicPlus, Some(180.0)), 180.0);
        assert_eq!(b.at(LodMode::ClassicPlus, Some(80.0)), 115.0);
        assert_eq!(b.at(LodMode::ClassicPlus, Some(400.0)), 260.0);
        assert_eq!(b.at(LodMode::Ps2, Some(80.0)), 115.0);
    }
}
