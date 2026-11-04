//! The scene upload: [`upload_scene`] turns a loaded `Scene` into the static
//! draw list the renderer consumes every frame ([`SceneGpu`]) — the terrain in
//! draw chunks, the `LOWR` sheet, the water, the instanced models, the sky
//! shells and `SkyObject` flights — through one by-name texture cache, and
//! counts every draw it could not texture ([`Untextured`]); a specular
//! segment's second pass rides beside its base draw, with the map's lookup
//! table installed on the renderer (queue SPEC); the water's speckle
//! flipbook rides beside the water ([`SpeckleAnim`], queue W3).

use super::camera::instance_matrix;
use super::look::{
    apply_lighting, clear_color, far_cull, far_cull_from, lod_bands, lod_mode, lowres_enabled,
    lowres_range, speckle_frames, speckle_look, water_look,
};
use super::upload::{
    compact_submesh, upload_cpu_texture_inner, upload_cpu_texture_scrolling, upload_submeshes,
    SubmeshPart,
};
use crate::assets::{CpuMesh, CpuTexture, Scene, SpecPass};
use anyhow::{anyhow, Result};
use glam::{Mat4, Vec3};
use render::{DrawCmd, DrawLayer, MeshHandle, Renderer, TextureHandle};
use std::collections::HashMap;
use std::path::Path;

/// Fallback tint for a water surface whose texture the map did not name and
/// whose `_water` fallback was ambiguous. Unused on all twelve shipped water
/// maps — every one of them resolves a texture — but a bake without the
/// texture must still put a surface there rather than nothing.
const WATER_TINT: [f32; 4] = [0.16, 0.28, 0.34, 1.0];

/// GPU-uploaded scene: a static draw list the renderer consumes every frame.
pub struct SceneGpu {
    pub draws: Vec<DrawCmd>,
    /// (class, count) skipped for having no drawable model — for one-time logging.
    pub skipped: Vec<(String, usize)>,
    /// Draws that resolved NO texture and fell back to a flat colour, as
    /// (what it is, material name or `<none>`, draw count). Collected here
    /// rather than derived by a separate walk so a census cannot drift from
    /// what actually renders — see `--scene-report`.
    pub untextured: Vec<(String, String, usize)>,
    /// Segments the DISC itself left untextured, SKIPPED rather than drawn —
    /// see [`Untextured::DiscUntextured`]. `--scene-report` prints this as
    /// "untextured on the disc".
    pub disc_untextured: usize,
    /// Draws skipped because the material names a texture that is on NEITHER
    /// disc — see [`Untextured::Dangling`] and [`DANGLING_ON_DISC`].
    pub dangling: usize,
    /// Terrain draw chunks dropped because the map's `LTEX` names a texture
    /// that ships in ANOTHER map's LVL. 64 on BF1 BES1 (`kam1_water`, only in
    /// KAM1.LVL) and 0 on the other 31 maps. The count is in OUR chunks (see
    /// [`terrain::TERRAIN_CHUNK_CELLS`]), not the disc's — it is one sheet of terrain.
    pub terrain_dropped: usize,
    /// What the frame clears to for THIS map (linear RGBA) — see
    /// [`clear_color`]. The viewer, the goldens and BOTH of the gallery's map
    /// panels pass this: since 2026-09-03 the sheet's first panel is a fogged
    /// vantage camera rather than an unfogged orbital one (`gallery.rs`).
    pub clear: [f64; 4],
    /// Sky shell segments whose texture was uploaded with V clamped rather
    /// than repeated — see [`sky_wrap_v`]. One line per segment,
    /// `"<geometry> prim<N> v=[lo,hi] tex=<material>"`, in upload order.
    /// This is the census behind the sheets' "what moved it" column and
    /// `--scene-report`'s clamp line; it changes no draw.
    pub sky_clamped: Vec<String>,
    /// Textures this map uploaded TWICE because of the clamp: once REPEAT for
    /// the world and once CLAMPED for a sky shell (`sky_naboo2` is both a
    /// shell and a world texture, which is why the clamped upload gets its own
    /// cache key). A clamped texture no world draw uses costs nothing extra
    /// and is not counted here.
    pub sky_clamp_extra_uploads: usize,
    /// The map's water, able to move — `None` on a dry map. Nothing moves
    /// until [`SceneGpu::animate_water`] is called (`crate::water_anim`).
    pub water_motion: Option<crate::water_anim::WaterMotion>,
    /// The water's speckle flipbook — `None` where the map draws no speckle
    /// ([`speckle_look`]). Stepped with the water by
    /// [`SceneGpu::animate_water`]; until then it shows frame 0.
    pub speckle: Option<SpeckleAnim>,
    /// Every BF2 door's moving submeshes (queue DRV), drawn shut until
    /// [`SceneGpu::pose_doors`] is called — the goldens never call it.
    pub doors: Vec<DoorDraw>,
    /// Every armed building's parts that ride a node (card VTURD), drawn at
    /// the bind pose until [`SceneGpu::pose_turrets`] is called — the
    /// goldens never call it.
    pub turrets: Vec<super::turret_pose::TurretDraw>,
    /// Every health / ammo / vehicle droid's draws (card PWST), as placed
    /// until [`SceneGpu::pose_stations`] is called — the goldens never call
    /// it.
    pub stations: Vec<super::stations::StationDraw>,
    /// Each world instance's draws (its manifest index; the model, its LOWD
    /// and its far extension each push a range): what the chase camera's
    /// see-through props fade ([`SceneGpu::see_through`]).
    pub instance_draws: Vec<(u32, std::ops::Range<usize>)>,
    /// The props drawn see-through now, by manifest index and opacity
    /// (`crate::camera_collide`, set by [`SceneGpu::see_through`]).
    pub(super) faded: Vec<(u32, f32)>,
    /// Every building turret's intact draws and fallen look (card TBLD):
    /// [`SceneGpu::view_draws`] swaps them while the sim has it down.
    pub fallen: Vec<super::turret_fall::FallenDraw>,
    /// BF2's local lights the scene itself holds (card BPNT,
    /// `crate::point_light`): the authored ones and the EntityLights. The
    /// upload hands the renderer these plus the posts' at their starting
    /// owners; a battle re-submits them each frame
    /// (`crate::point_light::submit`).
    pub lights: Vec<render::PointLight>,
    /// The non-Static omnis among them, as the terrain pass takes them.
    pub ground_lights: Vec<crate::bolt_light::OmniLight>,
    /// A BF2 map: its command posts carry a light.
    pub post_lights: bool,
    /// What the posts' holograms draw with (card CPHOL, `crate::holo`).
    pub holo: crate::holo::HoloArt,
}

/// One placed door's draws that ride its `open` clip: the manifest instance,
/// its matrix, and per moving submesh the draws it pushed (the base draw and
/// its specular pass) with the bone's model-space track (`ManifestDoor::parts`).
pub struct DoorDraw {
    pub instance: usize,
    model: Mat4,
    parts: Vec<DoorPart>,
}

/// One moving submesh's draws and its bone's per-frame (rotation,
/// translation) track.
type DoorPart = (std::ops::Range<usize>, Vec<(glam::Quat, Vec3)>);

/// The water's speckle layer as pushed: per water submesh, TWO
/// [`DrawCmd::speckle`] draws at `first[k]` and `first[k] + 1` — the frame
/// `⌊t·fps⌋` and the next, weighted `1 − w` and `w` ([`speckle_frames`]).
/// Two adds of one surface sum to the PS2's one cross-faded add. The
/// instance alpha is that weight alone; the layer's colour and add weight
/// are `fs_speckle`'s ([`super::look::water_shading`]).
pub struct SpeckleAnim {
    first: Vec<usize>,
    frames: Vec<TextureHandle>,
    fps: f32,
    /// Normal water has one depth-writing draw, with a discrete frame.
    normal: bool,
}

impl SpeckleAnim {
    fn step(&self, draws: &mut [DrawCmd], t: f32) {
        let (i, j, w) = speckle_frames(t, self.fps, self.frames.len());
        for &k in &self.first {
            draws[k].texture = Some(self.frames[i]);
            if self.normal {
                continue;
            }
            draws[k].color[3] = 1.0 - w;
            draws[k + 1].texture = Some(self.frames[j]);
            draws[k + 1].color[3] = w;
        }
    }
}

impl SceneGpu {
    /// Pose every drawn door at its sim door's `open` (`core::door`): each
    /// moving submesh at `instance · D(open)`, the collision bodies' own
    /// rule. A door with no sim twin (`OPENBF_DOORS_SOLID=1`) stays shut.
    pub fn pose_doors(&mut self, doors: &[core::door::Door]) {
        for d in doors {
            let Some(dd) = self.doors.iter().find(|x| x.instance == d.instance) else {
                continue;
            };
            for (range, frames) in &dd.parts {
                let m = dd.model * core::door::sample(frames, d.open);
                for k in range.clone() {
                    self.draws[k].model = m;
                }
            }
        }
    }

    /// [`Self::animate_water`] at the renderer's own unwrapped water clock
    /// (or `OPENBF_WATER_TIME`): what the game loop calls every frame.
    pub fn animate_water_live(&mut self, r: &mut Renderer, eye: Vec3) -> Result<()> {
        let t = crate::water_anim::clock(r.water_time);
        self.animate_water(r, t, eye)
    }

    /// Move the map's water to `t` seconds, seen from `eye` (the camera's
    /// position) — a fixed time; a live loop calls [`Self::animate_water_live`].
    /// The flat-water bob rides the water draws' model matrices. On an ocean
    /// map every call re-uploads the FFT patches and instances them over the
    /// wet cells that reach inside the water's far range; the first call also
    /// takes the still plane out. The speckle flipbook turns to `t`'s frames.
    /// A no-op on a dry map. The gallery's sheets and the walkabout (the
    /// goldens) call it once per eye at the clock's 0 s
    /// (`crate::gallery`'s `step_water`).
    pub fn animate_water(&mut self, r: &mut Renderer, t: f64, eye: Vec3) -> Result<()> {
        if let Some(s) = &self.speckle {
            s.step(&mut self.draws, t as f32);
        }
        let Some(w) = &mut self.water_motion else {
            return Ok(());
        };
        let before = w.draw_range();
        w.step(r, &mut self.draws, t, eye)?;
        let after = w.draw_range();
        if after.len() != before.len() {
            reindex_after_splice(
                &mut self.doors,
                &mut self.turrets,
                &mut self.stations,
                &mut self.instance_draws,
                &mut self.speckle,
                &before,
                after.len(),
            );
            for f in &mut self.fallen {
                f.shift(&before, after.len());
            }
        }
        Ok(())
    }

    /// Draw the props in `faded` (manifest index, opacity) see-through, as an
    /// ordered dither (`render::Fade::see_through`; 0 hides one) and every
    /// other world instance whole: the chase camera's props in its way
    /// (`crate::camera_collide`, ours). A broken droid's wreck takes its
    /// instance's opacity in [`SceneGpu::view_draws`].
    pub fn see_through(&mut self, faded: &[(u32, f32)]) {
        if faded.is_empty() && self.faded.is_empty() {
            return;
        }
        let fade_of = |list: &[(u32, f32)], ii: u32| {
            list.iter()
                .find(|(i, _)| *i == ii)
                .map(|&(_, o)| render::Fade::see_through(o))
        };
        for (ii, range) in &self.instance_draws {
            let (was, now) = (fade_of(&self.faded, *ii), fade_of(faded, *ii));
            if was != now {
                for d in &mut self.draws[range.clone()] {
                    d.fade = now;
                }
            }
        }
        self.faded = faded.to_vec();
    }
}

/// An ocean step splices the water's draws (`old`) with `new_len` patch
/// draws, which moves every draw after them (`WaterMotion::step`). The door
/// parts and the speckle frames hold draw indices from upload, so without
/// this `pose_doors` wrote door poses onto whatever draws had slid under
/// its indices (a turret's parts hold them the same way, card VTURD) — ramps, decks and towers drawn at the doors' places, and
/// the doors themselves never moved (the maintainer's BF2 Kamino, 2026-09-30). Indices
/// past the range shift by the length change; a speckle index inside it
/// pointed at the still plane, which the patches replaced, so it goes.
fn reindex_after_splice(
    doors: &mut [DoorDraw],
    turrets: &mut [super::turret_pose::TurretDraw],
    stations: &mut [super::stations::StationDraw],
    instances: &mut [(u32, std::ops::Range<usize>)],
    speckle: &mut Option<SpeckleAnim>,
    old: &std::ops::Range<usize>,
    new_len: usize,
) {
    let delta = new_len as isize - old.len() as isize;
    let shift = |k: usize| -> usize {
        if k >= old.end {
            (k as isize + delta) as usize
        } else {
            k
        }
    };
    for d in doors.iter_mut() {
        for (range, _) in &mut d.parts {
            *range = shift(range.start)..shift(range.end);
        }
    }
    for t in turrets.iter_mut() {
        for (range, _) in &mut t.parts {
            *range = shift(range.start)..shift(range.end);
        }
        t.whole = shift(t.whole.start)..shift(t.whole.end);
    }
    for t in stations.iter_mut() {
        for (range, _) in &mut t.parts {
            *range = shift(range.start)..shift(range.end);
        }
    }
    for (_, range) in instances.iter_mut() {
        *range = shift(range.start)..shift(range.end);
    }
    if let Some(s) = speckle {
        s.first.retain(|k| !old.contains(k));
        for k in &mut s.first {
            *k = shift(*k);
        }
        if s.first.is_empty() {
            *speckle = None;
        }
    }
}

/// One uploaded submesh of a model: its mesh, the texture it resolved (if
/// any), the material name it asked for (kept for the untextured census),
/// the pass its material's `alphaMode` puts it in, its specular second
/// pass, if its segment draws one, and the PS2 lighting path its segment
/// takes (`render::LightPath`, queue LITEB).
/// One model to draw where [`upload_scene`] instances models: a world
/// instance or path prop (no tint, no fade; `door` = its manifest index and
/// rig when it has one) or a foliage model (`super::foliage`).
struct PlacedModel<'a> {
    model: &'a str,
    matrix: Mat4,
    door: Option<(usize, &'a manifest::ManifestDoor)>,
    /// Card VTURD: its manifest index, class, translation and node rig.
    turret: Option<(
        usize,
        &'a manifest::ManifestInstance,
        &'a manifest::ManifestTurretRig,
    )>,
    /// Card PWST: a droid's manifest index, translation and (its main
    /// model's) node rig.
    station: Option<(usize, Vec3, Option<&'a manifest::ManifestTurretRig>, bool)>,
    /// A world instance's manifest index ([`SceneGpu::instance_draws`]).
    instance: Option<usize>,
    color: Option<[f32; 4]>,
    fade: Option<render::Fade>,
    /// Metres nearer than which the draw is discarded — the far scene's
    /// reach, for a geometry's LOWD extension alone ([`upload_scene`]).
    near: Option<f32>,
    /// Card LOD: this draw's side of the far scene's model / LOWD switch.
    lod: Option<render::LodSwitch>,
    /// Card TBLD: a building turret's manifest index, and whether this is
    /// its fallen look (kept out of the scene's draws) or an intact draw.
    building: Option<(usize, bool)>,
}

type ModelPart = (
    MeshHandle,
    Option<BoundTexture>,
    Option<String>,
    DrawLayer,
    Option<SpecPass>,
    render::LightPath,
);

/// Build one textured draw in the pass its material asked for. `Water` (and
/// its `Speckle`), `FarSheet` and `TerrainLayer` cannot reach here — no glTF
/// `alphaMode` maps to them (the water plane, the `LOWR` sheet and the
/// terrain layers are built by their own blocks in [`upload_scene`], never
/// from a material) — and they fall back
/// to the opaque pass rather than silently disabling depth.
pub(super) fn draw_in(
    layer: DrawLayer,
    mesh: MeshHandle,
    texture: TextureHandle,
    model: Mat4,
) -> DrawCmd {
    match layer {
        DrawLayer::Mask => DrawCmd::textured_masked(mesh, texture, model),
        DrawLayer::Blend => DrawCmd::textured_blended(mesh, texture, model),
        DrawLayer::Opaque
        | DrawLayer::Water
        | DrawLayer::Spec
        | DrawLayer::Speckle
        | DrawLayer::FarSheet
        | DrawLayer::TerrainLayer => DrawCmd::textured(mesh, texture, model),
    }
}

/// Install `game`'s specular lookup table: `common_<game>/specular.ktx2`
/// under the baked tree `assets_dir` (`bake specular`), level 0 only — it is
/// a lookup, never mipmapped. No table (a tree baked before queue SPEC) leaves
/// the pass off, which the renderer does by skipping every [`DrawLayer::Spec`]
/// draw. The disc's mirrored light is the default (`render::Renderer::spec_mirror`;
/// DECIDED by the SPEC RE, PLAN Log 2026-09-24; PROVISIONAL, `[O]`,
/// `analysis/re-specular-pass.md` §4). A map installs its own game's
/// ([`upload_scene`]); the gallery's unit sheet, which has no map, the side's.
pub fn install_spec_table(r: &mut Renderer, assets_dir: &Path, game: &str) -> Result<()> {
    r.spec_mirror = true;
    let path = assets_dir
        .join(format!("common_{game}"))
        .join("specular.ktx2");
    let table = match Some(path).filter(|p| pak::is_file(p)) {
        Some(p) => Some(crate::assets::load_ktx2(&pak::read(&p)?)?),
        None => None,
    };
    r.set_specular_table(table.as_ref().map(|t| (t.width, t.height, &t.rgba[..])))
        .map_err(|e| anyhow!("specular table: {e}"))
}

/// Install BF2's terrain light texture, `common_bf2/com_sfx_omniproj.ktx2`
/// (`bake omniproj`, card FXBL), level 0 only. BF1 (whose terrain draws no
/// light, `analysis/re-bolt.md` §5.1) or a tree baked before FXBL installs
/// none, and no light pass is drawn.
pub fn install_omniproj(r: &mut Renderer, assets_dir: &Path, game: &str) -> Result<()> {
    let path = assets_dir.join("common_bf2").join("com_sfx_omniproj.ktx2");
    let on = game == "bf2";
    let tex = match Some(path).filter(|p| on && pak::is_file(p)) {
        Some(p) => Some(crate::assets::load_ktx2(&pak::read(&p)?)?),
        None => None,
    };
    r.set_omniproj(tex.as_ref().map(|t| (t.width, t.height, &t.rgba[..])))
        .map_err(|e| anyhow!("com_sfx_omniproj: {e}"))
}

/// Where a map's specular draws stop, metres from the eye. BF2 draws the pass
/// only on NEAR-scene models (`analysis/re-specular-pass.md` §7, `[O]`), and
/// the port's split is `NearSceneRange`'s c, the heightfield's own end
/// (`far_cull_from`) — PROVISIONAL, the entry's decision; BF1 draws it in
/// both scenes and takes the base draw's own cull.
pub(super) fn spec_cull(scene: &Scene, base: Option<f32>) -> Option<f32> {
    if scene.manifest.source_game == "bf2" {
        far_cull_from(&scene.manifest.lighting).map(|c| c.terrain)
    } else {
        base
    }
}

/// Does this texture carry ANY transparency? Used by the ocean's foam layer,
/// which is only worth a second draw of the whole surface if it can let the
/// base show through — see [`upload_scene`].
fn texture_has_alpha(t: &CpuTexture) -> bool {
    t.rgba.iter().skip(3).step_by(4).any(|&a| a < 255)
}

/// A texture bound to a draw. It carries NO alpha mode: under ALPHA RULE v2
/// the pass is a property of the DRAW (the GLB material's `alphaMode`), so
/// one cached texture serves Opaque, Mask and Blend draws alike — which is
/// what makes the by-name cache below still correct.
#[derive(Clone, Copy)]
struct BoundTexture {
    handle: TextureHandle,
}

/// Texture names a segment on the discs asks for that exist in NO `tex_`
/// chunk on EITHER PS2 disc — dangling on the disc, so the PS2 could not have
/// textured them from that LVL either.
///
/// PROVISIONAL AND OURS as a list. The census
/// (`engine/artifacts/briefs/untextured_list.csv`, read off the raw disc
/// bytes) found three names across 32 maps, accounting for 8 draws: `yav_prop_ivy` (6 draws, both YAV1s), `Cor1_bldg_flowers` (1, BF2
/// COR1) and `noIcon` (1, the `geo_sky_explosions` sky shell). What the PS2
/// actually put on screen for a dangling material is UNTRACED, so the draw is
/// skipped rather than painted an invented colour.
///
/// `yav_prop_ivy` DOES ship in the PC install — reinstating it is a
/// classic-plus question, not a fidelity one, and is not this list's job.
///
/// Lowercase; matched case-insensitively, like [`get_texture`]'s own lookup.
///
/// `tat_propcz1_parts_head` (1 draw, BF2 TAT3's droid parts), found after that
/// census, is on no PS2, Xbox or PC copy either, and the model's other sheets do
/// not hold its layout.
const DANGLING_ON_DISC: &[&str] = &[
    "yav_prop_ivy",
    "cor1_bldg_flowers",
    "noicon",
    "tat_propcz1_parts_head",
];

/// The full-size texture a missing `_lowrez` names, for a far model that asks
/// for a reduced copy no disc holds: `com_item_healthrecharge_lowrez` (a draw per
/// station on 12 BF1 maps; BF2's copy of the same model, UV for UV, names the
/// full sheet) and `hoth_hangar_platform_lowrez` (1 draw on either Hoth). The
/// lookup is by name only, so this adds no sheet: `None` for any other name.
fn lowrez_source(name: &str) -> Option<&str> {
    name.strip_suffix("_lowrez")
}

/// Why a draw resolved no texture — which decides whether it is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Untextured {
    /// The DISC named no texture for this segment: all four `TNAM` slots
    /// empty (BF2) or no `TNAM` chunk at all (BF1), so the baked GLB material
    /// name is empty and [`upload_submeshes`] hands us `None`. 76 draws over
    /// 27 model/segment pairs on 8 maps, every one a 1-18-triangle stub —
    /// effect anchors and collision placeholders (`kas2_prop_dirt` x31,
    /// `geo_prop_splash`, `dea1_bldg_*_collision_*`, `uta1_bldg_cliffs*`,
    /// `mus1_fx_largeexplosion`, `dag1_prop_effectvine_1`,
    /// `tat1_prop_tuskcampfire1`) — not art.
    ///
    /// NOT DRAWN. Flat grey put solid stubs in the world that the disc never
    /// asked to be visible; skipping is what the terrain arm already does for
    /// the same reason. PROVISIONAL: what the PS2 does with an untextured
    /// segment is untraced.
    ///
    /// One of the 76 differs and is recorded here rather than special-cased:
    /// BF2 TAN1 `tan4_bldg_engine_b` seg1 has an empty slot but DOES carry
    /// `COL0` (8 colours, MTRL `0x00010201`), so it may be real
    /// vertex-coloured geometry. The renderer does not draw `COL0` today, so
    /// the only choice on offer is grey or nothing — revisit when it does.
    DiscUntextured,
    /// The material names a texture that is on neither disc — see
    /// [`DANGLING_ON_DISC`]. Also not drawn.
    Dangling,
    /// The material names a texture we simply failed to find: a lookup bug on
    /// OUR side, not the disc's. Still painted flat grey and counted, so it
    /// stays visible. Reads 0 on all 32 maps as of session 10.
    LookupFailed,
}

/// Route one draw whose texture did not resolve. Pure, and the unit test
/// drives it directly — the three arms are the whole policy.
pub fn route_untextured(material: Option<&str>) -> Untextured {
    match material {
        Some(name) if !name.is_empty() => {
            if DANGLING_ON_DISC
                .iter()
                .any(|d| d.eq_ignore_ascii_case(name))
            {
                Untextured::Dangling
            } else {
                Untextured::LookupFailed
            }
        }
        // `None`, or an empty name: the disc named nothing.
        _ => Untextured::DiscUntextured,
    }
}

/// Whether the map's heightfield is drawn. BF1 reads `DomeInfo
/// TerrainEnable` into `DAT_005229a8` (default 1), and at 0 its terrain draw
/// `FUN_00487ad8` draws nothing — no patches, no detail pass, no far sheet
/// (`analysis/re-terrain-enable.md` §2 `[O]`). The discs author it only on
/// BF1 KAM1 (whose sea is then the ocean draw alone) and BES1; BF2's ELF
/// never reads the key. Only the DRAW goes: `Scene::heightfield` still gives
/// the sim a ground (card TENS).
fn terrain_drawn(source_game: &str, sky: Option<&manifest::ManifestSky>) -> bool {
    !(source_game == "bf1" && sky.and_then(|s| s.terrain_enable) == Some(0.0))
}

/// The untextured census `upload_scene` accumulates as it uploads, so it
/// cannot drift from what actually renders.
#[derive(Default)]
struct Census {
    /// Real lookup failures — (what it is, material name) -> draw count.
    failed: HashMap<(String, String), usize>,
    disc_untextured: usize,
    dangling: usize,
    terrain_dropped: usize,
    /// See [`SceneGpu::sky_clamped`].
    sky_clamped: Vec<String>,
}

impl Census {
    /// Record one draw that resolved no texture, and say what to do with it.
    fn route(&mut self, what: &str, material: Option<&str>) -> Untextured {
        let r = route_untextured(material);
        match r {
            Untextured::DiscUntextured => self.disc_untextured += 1,
            Untextured::Dangling => self.dangling += 1,
            Untextured::LookupFailed => self.failed(what, material),
        }
        r
    }

    /// Record a lookup failure directly — for a draw that keeps a fallback
    /// colour of its own (the water plane's tint) rather than being routed.
    fn failed(&mut self, what: &str, material: Option<&str>) {
        let m = match material {
            Some(m) if !m.is_empty() => m.to_ascii_lowercase(),
            _ => "<none>".to_string(),
        };
        *self.failed.entry((what.to_string(), m)).or_default() += 1;
    }
}

pub fn upload_scene(r: &mut Renderer, scene: &Scene) -> Result<SceneGpu> {
    let mut textures: HashMap<String, BoundTexture> = HashMap::new();
    let mut draws = Vec::new();
    let mut census = Census::default();
    // The far-scene cull, off unless `OPENBF_FAR_CULL=1`. It is decided ONCE
    // here and carried per draw (`render::DrawCmd::cull`), because the two
    // distances belong to different draws: the terrain stops at the near
    // scene's far plane, the models and the water at `FarSceneRange`. The
    // sky's own upload path takes none.
    let cull = far_cull(&scene.manifest.lighting);

    // Terrain at identity: bake emits world-space positions. Its material is
    // LTEX layer 0, tiled by SCAL[0] in the baked UVs (bake::terrain_glb).
    // SPLIT into draw chunks here rather than in the bake — see
    // `upload_terrain_chunks`.
    //
    // A chunk whose layer texture does not resolve is NOT DRAWN. There used
    // to be a `TERRAIN_TINT` fallback (a tan we picked), and exactly one map
    // ever hit it: BF1 Bespin Platforms, whose `tern` names `LTEX("kam1_water")`
    // and `DTEX("yav_decal")` — both textures from OTHER maps' LVLs, in no
    // chunk BES1 ships, so the PS2 cannot have textured that sheet from
    // BES1.LVL either. Painting it tan laid a flat invented floor over the
    // cloud art Bespin DOES ship below its horizon
    // (`bes1_sky_dome_clouds_2`, 60 vertices, every one below the horizon).
    // `Scene::heightfield` still reads `scene.terrain`, so the sim's ground
    // is untouched — this only decides what is DRAWN.
    //
    // A BF1 map authoring `DomeInfo TerrainEnable(0)` (KAM1, BES1) draws no
    // terrain at all — neither the base nor the layers: see `terrain_drawn`.
    let terrain = scene
        .terrain
        .as_ref()
        .filter(|_| terrain_drawn(&scene.manifest.source_game, scene.manifest.sky.as_ref()));
    if let Some(t) = terrain {
        for (mesh, mat, layer, _) in upload_terrain_chunks(r, t)? {
            let tex = match mat {
                Some(m) => get_terrain_texture(r, scene, &mut textures, &m.to_ascii_lowercase())?,
                None => None,
            };
            draws.push(match tex {
                // UNLIT with the GS vertex byte, per game: no ambient, sun
                // or Light2 reaches the PS2's terrain (re-terrain-lighting.md
                // §0, [O]).
                Some(b) => draw_in(layer, mesh, b.handle, Mat4::IDENTITY)
                    .with_cull(cull.map(|c| c.terrain))
                    .with_light(render::LightPath::Terrain),
                None => {
                    // Its own report line: this is a whole SHEET of terrain
                    // whose layer texture lives in another map's LVL, not a
                    // per-segment authoring gap. See `terrain_dropped`.
                    census.terrain_dropped += 1;
                    continue;
                }
            });
        }
    }

    // The terrain's texture LAYERS above LTEX[0] (card TALPH,
    // `analysis/re-terrain-layers.md`): each layer's triangles composited
    // over the base terrain by their `ALPH` alpha, in ascending LTEX order —
    // the order `upload_terrain_layer_chunks` uploads them in, which the
    // renderer's `TerrainLayer` pass keeps. Chunked like the base for the
    // frustum cull; a layer whose texture does not resolve is dropped like a
    // base chunk (none on the disc: every LTEX name ships in its own LVL).
    if let (Some(t), Some(layers)) = (terrain, &scene.terrain_layers) {
        for (mesh, mat) in upload_terrain_layer_chunks(r, t, layers)? {
            match get_terrain_texture(r, scene, &mut textures, &mat.to_ascii_lowercase())? {
                Some(b) => draws.push(
                    DrawCmd::terrain_layer(mesh, b.handle, Mat4::IDENTITY)
                        .with_cull(cull.map(|c| c.terrain)),
                ),
                None => census.terrain_dropped += 1,
            }
        }
    }

    // The `LOWR` far-LOD sheet at identity, like the terrain, and behind
    // `OPENBF_LOWRES=1`: the only terrain either PS2 draws past its near
    // scene (`analysis/re-lowr.md` §0.2 [V]). Its range is
    // [`lowres_range`] — MaxDistance out, and the heightfield's own end in,
    // which is the port's one deviation and is written up there.
    //
    // Drawn on its own pass, `DrawLayer::FarSheet` (queue CLN5): UNLIT, as
    // the disc's base colour 0x80808080 is GS unity; alpha-tested at the
    // disc's AREF 0x10, which cuts real holes in BF1 KAS1's and KAS2's
    // sheets (`bake::lowres_glb`); and fogged like any far-scene draw — the
    // sheet is drawn with the `fogspread` program, never as sky
    // (re-lowr.md §2.2 [V]).
    // `TerrainEnable(0)` switches it off with the rest (`terrain_drawn`).
    if lowres_enabled() && terrain_drawn(&scene.manifest.source_game, scene.manifest.sky.as_ref()) {
        if let (Some(mesh), Some((near, far))) = (
            &scene.lowres,
            lowres_range(cull, &scene.manifest.lighting, scene.manifest.sky.as_ref()),
        ) {
            let name = scene
                .manifest
                .sky
                .as_ref()
                .and_then(|s| s.low_res_terrain.as_deref())
                .unwrap_or_default();
            // WrapV::Repeat because the two maps that author a big
            // `TextureScale` (BES2 20, END1 19) tile their sheet.
            match get_texture(r, scene, &mut textures, name, render::WrapV::Repeat)? {
                Some(b) => {
                    let handle = r
                        .add_mesh(&mesh.mesh_data(&mesh.indices))
                        .map_err(|e| anyhow!("lowres mesh upload: {e}"))?;
                    draws.push(
                        DrawCmd::far_sheet(handle, b.handle, Mat4::IDENTITY)
                            .with_cull(Some(far))
                            .with_cull_near(Some(near)),
                    );
                }
                // Same reasoning as the terrain's dropped sheet: a whole
                // surface with no texture is not worth an invented colour.
                None => census.failed("lowres", Some(name)),
            }
        }
    }

    // Water at identity, like the terrain: `bake::water_glb` emits world-space
    // positions at the plane's own height. Its texture gets a DEDICATED
    // handle so the authored UV scroll cannot leak into a model that happens
    // to share the name.
    //
    // Its LOOK is [`water_look`] — the disc's tint and blend weight, its own
    // fog rule (carried in `render::Lighting`, not per draw) and its own far
    // distance. On FLAT water that distance rides the same `with_cull` switch
    // as everything else and so is OFF unless `OPENBF_FAR_CULL=1`: culling
    // the water while the terrain under it runs on to the horizon would open
    // a hole in the sea and show the seabed, which is a worse picture than
    // the one the flag keeps off by default (`far_cull_enabled`; queue L's
    // landing). An OCEAN always ends there (card KSEA,
    // `analysis/re-kamino.md` §2): the PS2 draws no water past the ocean's
    // far pass and the frame behind it is black, and neither Kamino draws a
    // seabed. A stepped ocean ends on whole cells (`water_anim`); this still
    // plane, which has no eye to pick cells by, is cut per fragment.
    let mut water_motion = None;
    let mut speckle = None;
    if let (Some(info), Some(mesh)) = (&scene.manifest.water, &scene.water) {
        let first = draws.len();
        let look = water_look(&scene.manifest.source_game, &scene.manifest.lighting, info);
        let ocean = info.config.as_ref().is_some_and(|c| c.ocean);
        let far = if ocean { look.far } else { cull.and(look.far) };
        let mut handle = |name: Option<&str>, uv_scale| -> Result<Option<TextureHandle>> {
            let Some(t) = name.and_then(|n| scene.textures.get(&n.to_ascii_lowercase())) else {
                return Ok(None);
            };
            let upload = if ocean {
                upload_cpu_texture_scrolling
            } else {
                super::upload::upload_flat_water_texture
            };
            Ok(Some(upload(
                r,
                t,
                name.unwrap_or("water"),
                info.uv_velocity,
                uv_scale,
            )?))
        };
        let tex = handle(info.texture.as_deref(), [1.0, 1.0])?;
        // The ocean's foam layer: a SECOND draw of the same surface with the
        // map's `FoamTexture`, blended over the opaque base by its sheet's
        // alpha × its vertex alpha (`analysis/re-water.md` §3 cit. 5 [V],
        // `re-water-alpha.md` §0 [O]). Both KAM1s and nothing else. That
        // vertex alpha is the disc's per-point foam byte, which a STEPPED
        // ocean carries (`water_anim::Foam`, queue OCF). The still plane is
        // flat, and on a flat field the disc's rule decays every foam byte to
        // 0, so this still draw carries no foam (alpha 0, set below once
        // `WaterMotion::new` has taken its templates at the look's foam
        // alpha: 1, or the lit ocean's 0.5) — what an
        // unstepped scene shows; `WaterMotion::step` replaces it. Its colour is
        // unity (WCOLB): the baked mesh's placeholder white alpha under a
        // unity foam would paint the whole sea with the sheet.
        //
        // It is drawn only if the sheet carries transparency: blending an
        // opaque layer is a PAINT that would hide `kam1_water_2` and put back
        // the flat grey plane W1 removed. Both KAM1s' do since WA (`19975d0`,
        // 2026-09-23) — `kam1_water` keeps its disc alpha, 0..218
        // (`re-water-alpha.md` §4), where the bake before it forced 255.
        let foam = match look.foam_tint {
            Some(_) => {
                let name = info.config.as_ref().and_then(|c| c.foam_texture.as_deref());
                match name
                    .and_then(|n| scene.textures.get(&n.to_ascii_lowercase()))
                    .is_some_and(texture_has_alpha)
                {
                    true => handle(name, foam_uv_scale(info.config.as_ref()))?,
                    false => None,
                }
            }
            None => None,
        };
        let normal_cfg = info
            .config
            .as_ref()
            .filter(|_| info.texture.is_none())
            .and_then(|c| c.normal_maps.as_ref());
        let mut normals = Vec::new();
        if let Some(n) = normal_cfg {
            for name in &n.textures {
                let Some(t) = scene.textures.get(&name.to_ascii_lowercase()) else {
                    normals.clear();
                    break;
                };
                let mips = t.mips();
                normals.push(
                    r.add_texture(&render::TextureDesc {
                        width: t.width,
                        height: t.height,
                        mips: &mips,
                        sampler: render::SamplerMode::Linear,
                        wrap_v: render::WrapV::Repeat,
                        srgb: false,
                        detail: None,
                        scroll: info.uv_velocity,
                        uv_scale: [1.0, 1.0],
                        label: Some(name),
                    })
                    .map_err(|e| anyhow!("normal water {name}: {e}"))?,
                );
            }
        }
        // The speckle layer ([`speckle_look`], queue W3): its frames get
        // their own handles, carrying the layer's tiling and scroll. The
        // WATR cell side comes from the wet mask, or from the first lattice
        // quad on older bakes. A frame the bake
        // did not ship drops the whole layer — 0 on the shipped maps.
        let cell = info.wet.as_ref().map(|w| w.cell_size).or_else(|| {
            mesh.indices
                .get(..6)
                .and_then(|q| crate::water_anim::lattice_cells(&mesh.positions, q))
                .map(|l| l.cell)
        });
        let look_s = cell.and_then(|cell| speckle_look(&scene.manifest.source_game, info, cell));
        let mut frames = Vec::new();
        if let Some(sp) = &look_s {
            for n in &sp.frames {
                match scene.textures.get(&n.to_ascii_lowercase()) {
                    Some(t) => frames.push(super::upload::upload_flat_water_texture(
                        r,
                        t,
                        n,
                        sp.scroll,
                        sp.uv_scale,
                    )?),
                    None => {
                        frames.clear();
                        break;
                    }
                }
            }
        }
        let mut speckle_first = Vec::new();
        let mut normal_first = Vec::new();
        let mut still_foam = Vec::new();
        for (m, _, _, _) in upload_submeshes(r, mesh)? {
            let tint = if info.vertex_diffuse {
                [1.0, 1.0, 1.0, look.tint[3]]
            } else {
                look.tint
            };
            if let (Some(n), Some(h)) = (normal_cfg, normals.first()) {
                normal_first.push(draws.len());
                draws.push(
                    DrawCmd::water(
                        m,
                        *h,
                        n.refraction.map(|v| (v / 255.0).clamp(0.0, 1.0)),
                        Mat4::IDENTITY,
                    )
                    .with_light(render::LightPath::WaterNormal)
                    .with_cull(far),
                );
                continue;
            }
            draws.push(match tex {
                Some(h) => DrawCmd::water(m, h, tint, Mat4::IDENTITY).with_cull(far),
                None => {
                    // The water plane keeps its own tint rather than being
                    // routed: the manifest always names a texture, so a miss
                    // here IS a lookup failure. 0 on all 32 maps.
                    census.failed("<water>", info.texture.as_deref());
                    DrawCmd::flat(m, WATER_TINT, Mat4::IDENTITY).with_cull(far)
                }
            });
            if let (Some(h), Some(tint)) = (foam, look.foam_tint) {
                still_foam.push(draws.len());
                draws.push(DrawCmd::water(m, h, tint, Mat4::IDENTITY).with_cull(far));
            }
            if let (Some(_), false) = (&look_s, frames.is_empty()) {
                let c = |w: f32| [1.0, 1.0, 1.0, w];
                let next = frames[1 % frames.len()];
                speckle_first.push(draws.len());
                draws.push(DrawCmd::speckle(m, frames[0], c(1.0), Mat4::IDENTITY).with_cull(far));
                draws.push(
                    DrawCmd::speckle(m, next, [0.0, 1.0, 1.0, 0.0], Mat4::IDENTITY).with_cull(far),
                );
            }
        }
        if let (Some(sp), false) = (&look_s, speckle_first.is_empty()) {
            speckle = Some(SpeckleAnim {
                first: speckle_first,
                frames,
                fps: sp.fps,
                normal: false,
            });
        }
        if let (Some(n), false) = (normal_cfg, normal_first.is_empty()) {
            speckle = Some(SpeckleAnim {
                first: normal_first,
                frames: normals,
                fps: n.fps,
                normal: true,
            });
        }
        // What moves it: the bob, or the ocean's FFT patch. Built here, and
        // inert until a caller steps it — these draws stay exactly as pushed.
        let ranges = crate::water_anim::WaterRanges {
            near: scene
                .manifest
                .lighting
                .near_scene_range
                .map_or(0.0, |r| r[1]),
            far: look.far.unwrap_or(f32::INFINITY),
        };
        water_motion = Some(crate::water_anim::WaterMotion::new(
            first..draws.len(),
            &draws,
            mesh,
            info.config.as_ref(),
            ranges,
        ));
        for k in still_foam {
            draws[k].color[3] = 0.0;
        }
    }

    // Models: upload each drawable model's submeshes once, then instance them.
    // Three sources, in this order: the world's instances (`ii` is the
    // manifest index a door rig keys on), the foliage's path-block props
    // (world instances of their class, re-foliage §9) and the foliage's
    // model instances (`super::foliage`), which carry their tint and fade.
    let mut uploaded: HashMap<String, Vec<ModelPart>> = HashMap::new();
    let mut skipped: HashMap<String, usize> = HashMap::new();
    let mut doors: Vec<DoorDraw> = Vec::new();
    let mut turrets: Vec<super::turret_pose::TurretDraw> = Vec::new();
    let mut stations: Vec<super::stations::StationDraw> = Vec::new();
    let mut instance_draws: Vec<(u32, std::ops::Range<usize>)> = Vec::new();
    let wreck_models = crate::stations::wreck_models(scene)?;
    let fallen_models = crate::turret_fall::fallen_models(scene)?;
    let mut fallen: Vec<super::turret_fall::FallenDraw> = Vec::new();
    let path_props = scene
        .manifest
        .foliage
        .as_ref()
        .map_or(&[][..], |f| &f.path_props[..]);
    let mut placed: Vec<PlacedModel<'_>> = Vec::new();
    let mode = lod_mode();
    let bands = (mode != super::look::LodMode::Off)
        .then(|| lod_bands(&scene.manifest.lighting))
        .flatten();
    for (ii, inst) in scene
        .manifest
        .instances
        .iter()
        .chain(path_props)
        .enumerate()
    {
        let Some(model) = scene.resolve_model(&inst.class) else {
            *skipped.entry(inst.class.clone()).or_default() += 1;
            continue;
        };
        let world = ii < scene.manifest.instances.len();
        // Card LOD (`analysis/re-lod.md` §4, §7 [O]): on a map with a far
        // scene the PS2 draws a geometry that has a LOWD as its model to the
        // near scene's end and as its LOWD past the far scene's reach; the
        // two draws below switch whole at one distance ([`render::LodSwitch`],
        // `LodBands::at`). The PS2
        // culls the LOWD at `FarSceneRange` by the OBJECT's sphere; the port
        // gives it the map's model cull like every other draw (per fragment,
        // off by default), since a per-fragment cut at `FarSceneRange` drops
        // BF2 UTA1's far sinkhole wall, a ring whose sphere is near. A geometry with no LOWD is drawn as its
        // model at every distance — the PS2 does not draw it past
        // `NearSceneRange[2]`; the port keeps it, a named deviation (draw
        // distance, re-lod §7.2). Path props are world instances of their
        // class (re-foliage §9) and take the same rule.
        let matrix = instance_matrix(inst.rotation, inst.translation);
        // The switch's sphere: the model's, shared by both sides so the
        // instance switches whole.
        let lowd = bands.and_then(|b| {
            let (anchor, radius) = crate::point_light::model_sphere(&scene.meshes[model], matrix)
                .map_or((Vec3::from(inst.translation), 0.0), |s| {
                    (s.centre, s.radius)
                });
            let reach = super::leaf::attach_reach(&scene.manifest, &inst.class);
            Some((
                scene.resolve_lowd(&inst.class)?,
                b.sides(mode, reach, anchor, radius),
            ))
        });
        // PWST: a droid turns whole, its rig's nodes by its own animation.
        let class = world.then(|| scene.resolve_class(&inst.class)).flatten();
        // TBLD: a building turret, drawn as its fallen look while down.
        let building = (world && fallen_models.contains_key(&ii)).then_some(ii);
        let station = class
            .filter(|c| crate::stations::is_station_base(&c.base))
            .map(|c| {
                (
                    ii,
                    Vec3::from(inst.translation),
                    c.turret_rig.as_ref(),
                    false,
                )
            });
        placed.push(PlacedModel {
            model,
            matrix: instance_matrix(inst.rotation, inst.translation),
            // DRV: the submeshes this instance's door clip moves.
            door: world
                .then(|| {
                    scene
                        .resolve_class(&inst.class)
                        .and_then(|c| c.door.as_ref())
                })
                .flatten()
                .map(|d| (ii, d)),
            // VTURD: the parts this instance's turret turns.
            turret: class
                .filter(|_| station.is_none())
                .and_then(|c| c.turret_rig.as_ref())
                .map(|t| (ii, inst, t)),
            station,
            instance: world.then_some(ii),
            color: None,
            fade: None,
            near: None,
            lod: lowd.map(|(_, s)| s.0),
            building: building.map(|ii| (ii, false)),
        });
        if let Some((ii, Some(model))) = building.map(|ii| (ii, fallen_models.get(&ii))) {
            if let Some(model) = model.as_deref().and_then(|m| scene.meshes.get_key_value(m)) {
                placed.push(PlacedModel {
                    model: model.0,
                    matrix: instance_matrix(inst.rotation, inst.translation),
                    door: None,
                    turret: None,
                    station: None,
                    instance: None,
                    color: None,
                    fade: None,
                    near: None,
                    lod: None,
                    building: Some((ii, true)),
                });
            }
        }
        if let Some(model) = wreck_models.get(&ii).filter(|_| station.is_some()) {
            placed.push(PlacedModel {
                model,
                matrix: instance_matrix(inst.rotation, inst.translation),
                door: None,
                turret: None,
                station: Some((ii, Vec3::from(inst.translation), None, true)),
                instance: None,
                color: None,
                fade: None,
                near: None,
                lod: None,
                building: None,
            });
        }
        if let Some((lowd, sides)) = lowd {
            placed.push(PlacedModel {
                model: lowd,
                matrix: instance_matrix(inst.rotation, inst.translation),
                door: None,
                turret: None,
                station: station.map(|(i, at, _, dead)| (i, at, None, dead)),
                instance: world.then_some(ii),
                color: None,
                fade: None,
                near: None,
                lod: Some(sides.1),
                building: building.map(|ii| (ii, false)),
            });
        }
        // The far scene's LOWD extension (card UTAS, `analysis/re-far-lowd.md`
        // §0/§6 [O]): the LOWD triangles its LOD0 lacks — BF2 UTA1's far
        // sinkhole and canyon walls — with the fragments nearer than
        // `NearSceneRange[1]` discarded. It goes with the MODEL: drawn
        // whenever the model is (the switch's model side), and dropped once
        // the whole LOWD, which holds those triangles, takes over.
        let reach = scene.manifest.lighting.near_scene_range.map(|r| r[1]);
        if let (true, Some(far), Some(near)) = (world, scene.resolve_far(&inst.class), reach) {
            placed.push(PlacedModel {
                model: far,
                matrix: instance_matrix(inst.rotation, inst.translation),
                door: None,
                turret: None,
                station: station.map(|(i, at, _, dead)| (i, at, None, dead)),
                instance: world.then_some(ii),
                color: None,
                fade: None,
                near: Some(near),
                lod: lowd.map(|(_, s)| s.0),
                building: building.map(|ii| (ii, false)),
            });
        }
    }
    let foliage_records = super::foliage::records(scene);
    placed.extend(
        super::foliage::models(scene, &foliage_records)
            .into_iter()
            .map(|f| PlacedModel {
                model: f.model,
                matrix: f.matrix,
                door: None,
                turret: None,
                station: None,
                instance: None,
                color: Some(f.color),
                fade: f.fade,
                near: None,
                lod: None,
                building: None,
            }),
    );
    // Card BPNT: each world model's sphere, in model space, for the local
    // lights' gather (`crate::point_light::model_sphere`).
    let mut spheres: HashMap<String, Option<render::LitSphere>> = HashMap::new();
    for p in &placed {
        let model = p.model;
        // The terrain foliage's models (tinted) take none: PROVISIONAL, their
        // draw path is not the model draw's (re-foliage §6).
        let lit = match p.color {
            Some(_) => None,
            None => spheres
                .entry(model.to_owned())
                .or_insert_with(|| {
                    crate::point_light::model_sphere(&scene.meshes[model], Mat4::IDENTITY)
                })
                .map(|s| render::LitSphere {
                    centre: p.matrix.transform_point3(s.centre),
                    radius: s.radius * p.matrix.transform_vector3(Vec3::X).length(),
                }),
        };
        if !uploaded.contains_key(model) {
            let mut parts = Vec::new();
            let cpu = &scene.meshes[model];
            for (k, (mesh, mat, layer, _)) in upload_submeshes(r, cpu)?.into_iter().enumerate() {
                let tex = match mat {
                    Some(m) => get_material_texture(
                        r,
                        scene,
                        &mut textures,
                        m,
                        render::WrapV::Repeat,
                        cpu.uv_scroll(k),
                    )?,
                    None => None,
                };
                parts.push((
                    mesh,
                    tex,
                    mat.map(|m| m.to_string()),
                    layer,
                    cpu.spec_pass(k),
                    cpu.light_path(k),
                ));
            }
            uploaded.insert(model.to_owned(), parts);
        }
        let m = p.matrix;
        let far = cull.map(|c| c.world);
        let lowd = matches!(p.lod, Some(l) if l.side == render::LodSide::Lowd);
        let rig = p.door.map(|(_, d)| d);
        let mut moving = Vec::new();
        let mut turning = Vec::new();
        let whole_start = draws.len();
        for (k, (mesh, tex, mat, layer, spec, light)) in uploaded[model].iter().enumerate() {
            let first = draws.len();
            // The specular second pass rides only a textured draw: its add is
            // weighted by the base texel's alpha. A far-scene draw (`p.near`)
            // takes none: BF2's pass is the near scene's (`spec_cull`). Nor
            // does a LOWD (card LOD), drawn only in the far scene.
            if let (Some(b), Some(sp), None, false) = (tex, spec, p.near, lowd) {
                let rgb = sp.rgb_under(scene.manifest.lighting.sun_color);
                // `light` too: `fs_spec` recomputes its base draw's colour.
                draws.push(
                    DrawCmd::specular(*mesh, b.handle, rgb, sp.power, m)
                        .with_cull(spec_cull(scene, far))
                        .with_light(*light)
                        .with_fade(p.fade)
                        .with_lod(p.lod)
                        .with_lit(lit),
                );
            }
            let mut base = match tex {
                Some(b) => draw_in(*layer, *mesh, b.handle, m).with_cull(far),
                None => match census.route(model, mat.as_deref()) {
                    // Nothing the disc left untextured, and nothing whose
                    // texture is on neither disc, is drawn.
                    Untextured::DiscUntextured | Untextured::Dangling => continue,
                    Untextured::LookupFailed => {
                        DrawCmd::flat(*mesh, [0.6, 0.6, 0.6, 1.0], m).with_cull(far)
                    }
                },
            }
            .with_cull_near(p.near)
            .with_lod(p.lod)
            .with_light(*light)
            .with_fade(p.fade)
            .with_lit(lit);
            if let Some(c) = p.color {
                base.color = std::array::from_fn(|i| base.color[i] * c[i]);
            }
            if *light == render::LightPath::Glow {
                base.color[3] *= super::look::hallway_lamp_bloom(&scene.manifest, mat.as_deref());
            }
            draws.push(base);
            if let Some(part) = rig.and_then(|r| r.parts.iter().find(|q| q.submesh as usize == k)) {
                let frames = part
                    .motion
                    .iter()
                    .map(|(q, t)| (glam::Quat::from_array(*q), Vec3::from(*t)))
                    .collect();
                moving.push((first..draws.len(), frames));
            }
            if p.turret
                .is_some_and(|(_, _, t)| matches!(t.parts.get(k), Some(Some(_))))
            {
                turning.push((first..draws.len(), k));
            }
            if let Some((ii, at, rig, false)) = p.station {
                let at_k = match stations.iter().position(|s| s.instance == ii) {
                    Some(j) => j,
                    None => {
                        stations.push(super::stations::StationDraw::new(ii, at, m, rig));
                        stations.len() - 1
                    }
                };
                stations[at_k]
                    .parts
                    .push((first..draws.len(), rig.map(|_| k)));
            }
        }
        if let Some((ii, _, _, true)) = p.station {
            if let Some(s) = stations.iter_mut().find(|s| s.instance == ii) {
                s.wreck.extend(draws.drain(whole_start..));
            }
        }
        if let Some((ii, look)) = p.building {
            let at = match fallen.iter().position(|f| f.instance == ii) {
                Some(k) => k,
                None => {
                    fallen.push(super::turret_fall::FallenDraw {
                        instance: ii,
                        intact: Vec::new(),
                        fallen: Vec::new(),
                    });
                    fallen.len() - 1
                }
            };
            if look {
                fallen[at].fallen.extend(draws.drain(whole_start..));
            } else {
                fallen[at].intact.push(whole_start..draws.len());
            }
        }
        if let (false, Some((ii, inst, rig))) = (turning.is_empty(), p.turret) {
            turrets.push(super::turret_pose::TurretDraw::new(
                ii,
                inst,
                m,
                rig,
                turning,
                whole_start..draws.len(),
            ));
        }
        if let (false, Some((ii, _))) = (moving.is_empty(), p.door) {
            doors.push(DoorDraw {
                instance: ii,
                model: m,
                parts: moving,
            });
        }
        if let Some(ii) = p.instance.filter(|_| whole_start < draws.len()) {
            instance_draws.push((ii as u32, whole_start..draws.len()));
        }
    }

    let mut skipped: Vec<(String, usize)> = skipped.into_iter().collect();
    skipped.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    apply_lighting(r, scene);
    // Set on every upload, so a map without lights clears the last map's.
    r.set_point_lights(crate::point_light::still_lights(scene));
    if let Some(assets_dir) = scene.dir.parent() {
        install_spec_table(r, assets_dir, &scene.manifest.source_game)?;
        install_omniproj(r, assets_dir, &scene.manifest.source_game)?;
    }
    let (sky, dome_shells) = upload_sky(r, scene, &mut textures, &mut census)?;
    r.set_sky(sky);
    upload_atmosphere(r, scene, &mut textures)?;
    r.set_sky_particles(Vec::new());
    r.set_lightning(None);
    // Set on every upload, so a map without stars clears the last map's.
    r.set_stars(star_field(&scene.manifest), dome_shells);
    // The grass patches (queue FOLGG, `super::grass`): the FOLG records'
    // patches and the world instances of a grass class, drawn by the
    // renderer's own pass every frame. Set on every upload, so a map
    // without grass clears the last map's.
    let (grass_classes, grass_patches) = super::grass::build(scene, &foliage_records, |name| {
        Ok(get_texture(r, scene, &mut textures, name, render::WrapV::Repeat)?.map(|t| t.handle))
    })?;
    r.set_grass(grass_classes, grass_patches);
    // The leaf patches (card LEAF, `super::leaf`): the props' attached
    // clusters, the world and FOLG ones; they ride the grass pass and fade
    // by NearSceneRange (b, c). Set on every upload, as the grass.
    let (leaf_classes, leaf_objects) = super::leaf::build(scene, &foliage_records, |name| {
        Ok(get_texture(r, scene, &mut textures, name, render::WrapV::Repeat)?.map(|t| t.handle))
    })?;
    let near = scene
        .manifest
        .lighting
        .near_scene_range
        .map(|[_, b, c]| (b, c));
    r.set_leaves(leaf_classes, leaf_objects, near);
    let emitters = scene
        .manifest
        .leaf
        .iter()
        .map(|g| render::LeafEmitter {
            count: g.falling_count,
            birds: g.bird_count,
            radius: g.radius,
            height: g.height,
            scale: g.scale,
            distance: g.max_distance,
        })
        .collect();
    let falling_texture = if scene.manifest.leaf.iter().any(|g| g.falling_count > 0) {
        get_texture(r, scene, &mut textures, "leaf", render::WrapV::Repeat)?.map(|t| t.handle)
    } else {
        None
    };
    let ground = scene.heightfield().map(|g| {
        Box::new(move |x, z| g.height_at(x, z)) as Box<dyn Fn(f32, f32) -> f32 + Send + Sync>
    });
    r.set_leaf_fall(emitters, falling_texture, ground);
    if let Some(wind) = &scene.manifest.wind {
        r.set_leaf_wind(Vec3::new(wind.velocity[0], 0.0, wind.velocity[1]));
    }
    crate::forest::configure_default(r, scene)?;
    let mut untextured: Vec<(String, String, usize)> = census
        .failed
        .into_iter()
        .map(|((a, b), n)| (a, b, n))
        .collect();
    untextured.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    Ok(SceneGpu {
        draws,
        skipped,
        untextured,
        disc_untextured: census.disc_untextured,
        dangling: census.dangling,
        terrain_dropped: census.terrain_dropped,
        clear: clear_color(scene),
        sky_clamp_extra_uploads: textures
            .keys()
            .filter_map(|k| k.strip_prefix(CLAMP_V_KEY_PREFIX))
            .filter(|bare| textures.contains_key(*bare))
            .count(),
        sky_clamped: census.sky_clamped,
        water_motion,
        speckle,
        doors,
        turrets,
        stations,
        instance_draws,
        faded: Vec::new(),
        fallen,
        lights: crate::point_light::scene_lights(scene),
        ground_lights: crate::point_light::ground_lights(scene),
        post_lights: scene.manifest.source_game == "bf2",
        holo: crate::holo::HoloArt::load(r, scene),
    })
}

/// World radius the base sky dome is scaled to, metres. PROVISIONAL and OURS.
///
/// The shipped dome meshes are tiny (`tat1_sky_dome` is 13.6 units across) —
/// they are drawn around the camera, so their own size says nothing about how
/// big the engine makes them. The disc's own scale is 1.0 — it draws the mesh
/// at its authored ~20 m (`analysis/re-sky-dome-placement.md` §3, S = 1.0
/// [D]) — and the port scales it up only so the shell clears the near plane.
///
/// Since 2026-09-03 the scale is angle-invariant: the shell's lift scales
/// with the mesh (`render::Placement::Dome`), so `SKY_SHELL_RADIUS` no longer
/// changes any elevation angle and reproducing the ELF's picture does not
/// depend on its value. It used to set what `DomeModel.Offset` meant in
/// proportion to the shell, which is how Mustafar's volcano band ended up
/// 5.5 degrees too high (RE §7).
const SKY_SHELL_RADIUS: f32 = 1000.0;

/// How a sky shell SEGMENT addresses its texture in V, from the V range of
/// that segment's own vertices: `ClampToEdge` when the range stays inside
/// [-0.25, 1.25], `Repeat` otherwise. U is always REPEAT (`render::WrapV`).
///
/// **What this fixes.** The port samples every texture REPEAT on all three
/// axes, and the shipped domes' bottom rings sit AT or a hair past v = 1.0:
/// `hoth_sky_dome` ends at v = 1.0010, `mus1_sky_dome` prim1 at 1.0767,
/// BF1 NAB1's `sky_water` at 1.1304, `cor1_sky_dome` at 1.0107. Those rows
/// wrap round to the texture's TOP row — zenith sky painted as a hard stripe
/// along the horizon, which is the line the maintainer reported across both Hoth skies
/// and Kamino's. Even a ring that stops exactly at v = 1.0 draws it, because
/// the bilinear footprint of the last row straddles the wrap.
///
/// **The measurement** is `engine/artifacts/sky_wrap/dome_uv.txt` (the lead,
/// 2026-09-03): the per-segment U/V ranges of every `DomeModel` on the 32
/// baked map dirs. Two base domes genuinely TILE in V and must keep
/// repeating — BF2 POL1's `pol1_skydome` (v = -1.708..2.708) and BF2 TAN1's
/// `tan1_sky_dome` (v = -0.503..1.502) — and many tile in U, which is why U
/// is untouched.
///
/// **OURS — a stated deviation** (`analysis/PLAN_LOG_2026.md`, "Stated deviations from the PS2", deviation 3): the
/// PS2 does not clamp; it REPEATs every model draw's texture, sky shells
/// included, and would show that row-0 line wherever nothing covers the ring
/// (`analysis/re-texture-wrap.md` §0, RE_LOG "Texture wrap mode",
/// 2026-09-03). The 0.25 tolerance is PROVISIONAL, the lead's own with no RE
/// behind it: wide enough to swallow the 1.0..1.13 overshoots and narrow
/// enough to leave the two tiling domes alone. This function is the ONE place
/// the rule lives.
pub fn sky_wrap_v(v_min: f32, v_max: f32) -> render::WrapV {
    if v_min >= -0.25 && v_max <= 1.25 {
        render::WrapV::ClampToEdge
    } else {
        render::WrapV::Repeat
    }
}

fn sky_emission(flags: u32, hdr: bool) -> Option<render::LightPath> {
    if !hdr || flags & 0x10 == 0 {
        None
    } else if flags & 0x04 != 0 {
        Some(render::LightPath::Unscaled)
    } else {
        Some(render::LightPath::Glow)
    }
}

/// Build the map's sky shells: each `DomeModel` from the manifest, uploaded
/// once and scaled so the FIRST shell (the dome) has `SKY_SHELL_RADIUS`. All
/// shells share that scale so their authored proportions survive.
///
/// The shipped domes cover a BAND of elevation, not a closed sphere. Censused
/// at the scale the engine draws them (`mungefs --example sky_dome_census`,
/// session 5): 29 of 32 base domes reach BELOW the horizon and every one
/// reaches the zenith; only BF1 KAS1, BF1 KAS2 and BF2 UTA1 stop at 0 degrees.
/// Below UTA1's the frame keeps its clear colour, black as on the PS2
/// ([`MAP_CLEAR`](super::MAP_CLEAR)); both Kashyyyks author their shells
/// `Reflect`, so there the mirrored copies fill it.
///
/// A `Reflect` shell is drawn twice in the PS2's order: its mirrored copy
/// (every segment), then the shell itself (BF1 `FUN_002f2308` 280242–280284,
/// `analysis/re-sky-reflect.md` §2). The copy is tinted by
/// `ReflectionColor` alone — the PS2 passes sky+0x1c, not the dome ambient —
/// and keeps the shell's own [`render::SkyMode`] (§3). The port has no far
/// target, so this one copy stands for both of the PS2's (§5).
fn upload_sky(
    r: &mut Renderer,
    scene: &Scene,
    textures: &mut HashMap<String, BoundTexture>,
    census: &mut Census,
) -> Result<(Vec<render::SkyShell>, usize)> {
    let Some(sky) = &scene.manifest.sky else {
        return Ok((Vec::new(), 0));
    };
    let ambient = [sky.ambient[0], sky.ambient[1], sky.ambient[2], 1.0];
    let reflection = sky.reflection_color.unwrap_or([1.0; 3]);
    let reflection = [reflection[0], reflection[1], reflection[2], 1.0];
    let mut scale = None;
    let mut shells = Vec::new();
    // The shells model 0 (the base dome) drew, mirror included: the stars go
    // right after them (`analysis/re-stars.md` §3).
    let mut dome_shells = 0;
    for (i, m) in sky.models.iter().enumerate() {
        let mut ambient = sky_shell_tint(scene.manifest.lightning.as_ref(), i, ambient);
        ambient[3] *= super::look::sky_layer_opacity(
            &scene.manifest.source_game,
            &scene.manifest.map,
            &m.geometry,
        );
        let Some(cpu) = scene.meshes.get(&m.geometry) else {
            // The discs reference four dome models that exist in no LVL
            // (`hoth_mountain_2`, `kas_moon`, `geo_sky_dome_arena`); the bake
            // already drops those, so this is belt and braces.
            continue;
        };
        let scale = *scale.get_or_insert_with(|| {
            let radius = cpu
                .positions
                .iter()
                .map(|p| p[0].abs().max(p[2].abs()))
                .fold(0.0f32, f32::max);
            if radius > 1e-3 {
                SKY_SHELL_RADIUS / radius
            } else {
                1.0
            }
        });
        // Sky shells keep their own pass (`upload_sky`), so the glTF alpha
        // mode — ALPHA RULE v2's census verdict, which is the WORLD path's
        // rule — is not consulted here. The RAW MTRL word is, because the ELF
        // draws a shell with the GS state its own material asks for
        // (`render::SkyMode`, `analysis/re-sky-dome-placement.md` §5).
        let mut own = Vec::new();
        for (k, (mesh, mat, _, flags)) in upload_submeshes(r, cpu)?.into_iter().enumerate() {
            // The V address mode is decided PER SEGMENT, off that segment's
            // own UVs — see [`sky_wrap_v`]. A segment whose range cannot be
            // read (an empty index run) keeps the old REPEAT behaviour.
            let v_range = cpu.submesh_v_range(k);
            let wrap = v_range.map_or(render::WrapV::Repeat, |(lo, hi)| sky_wrap_v(lo, hi));
            let tex = match &mat {
                Some(name) => get_texture(r, scene, textures, &name.to_ascii_lowercase(), wrap)?,
                None => None,
            };
            if wrap == render::WrapV::ClampToEdge && tex.is_some() {
                let (lo, hi) = v_range.unwrap_or((0.0, 0.0));
                census.sky_clamped.push(format!(
                    "{} prim{k} v=[{lo:.4},{hi:.4}] tex={}",
                    m.geometry,
                    mat.unwrap_or("<none>")
                ));
            }
            if tex.is_none() {
                let what = format!("<sky> {}", m.geometry);
                // A shell the disc never textured, or one whose texture is on
                // neither disc, is NOT DRAWN — otherwise it paints a flat
                // ambient-coloured shell right across the sky. The only shell
                // on any of the 32 maps that hits this is BF2 GEO1's
                // `geo_sky_explosions`, whose material is the dangling
                // `noIcon`.
                if census.route(&what, mat) != Untextured::LookupFailed {
                    continue;
                }
            }
            own.push(render::SkyShell {
                mesh,
                texture: tex.map(|b| b.handle),
                color: ambient,
                scale,
                // The ELF's dome rule, with the port's own uniform scale (an
                // angle-invariant scale about the eye) — see
                // `render::Placement::Dome` and
                // `analysis/re-sky-dome-placement.md` §3/§4/§8.1.
                placement: m.rotation.map_or(
                    render::Placement::Dome {
                        offset_y: m.offset,
                        movement_scale: m.movement_scale,
                    },
                    |[speed, x, y, z]| render::Placement::SpinningDome {
                        offset_y: m.offset,
                        movement_scale: m.movement_scale,
                        speed,
                        axis: Vec3::new(x, y, z),
                    },
                ),
                yaw: 0.0,
                // No fog flag: the sky draw flag 0x20 excludes every shell
                // from the fog pass in both ELFs (`analysis/re-fog.md` §2.4).
                mode: render::SkyMode::from_flags(flags),
                lit: false,
                depth: false,
                emission: sky_emission(
                    flags,
                    scene.manifest.hdr.is_some() && sky.glow_masks_preserved,
                ),
            });
        }
        if m.reflect {
            shells.extend(own.iter().map(|s| render::SkyShell {
                color: reflection,
                placement: render::Placement::Mirror {
                    offset_y: m.offset,
                    movement_scale: m.movement_scale,
                },
                ..*s
            }));
        }
        shells.extend(own);
        if i == 0 {
            dome_shells = shells.len();
        }
    }
    shells.extend(upload_sky_objects(r, scene, textures, ambient, census)?);
    Ok((shells, dome_shells))
}

/// The tint dome shell `index` is drawn with: `base` (the map's flat
/// `sky.ambient`), and on a map whose `Effect("Lightning")` is enabled shell 0
/// is darkened to the PS2's resting dome colour — 153/255 for the discs'
/// `SkyDomeDarkenFactor(0.4)` — while every other shell keeps `base`
/// (`analysis/re-kamino.md` §1 [O]: B2 `FUN_003cb918` draws shell 0 with the
/// stored dome colour and the rest with constant white). Mirrored shells and
/// `SkyObject`s are not shell 0's draw and keep their own tint.
///
/// The hook for the weather card (WTHR): a flash lifts the dome back toward
/// white, `ManifestLightning::dome_tint(brightness)`; this is the look
/// between flashes, `brightness` 0.
fn sky_shell_tint(
    lightning: Option<&manifest::ManifestLightning>,
    index: usize,
    base: [f32; 4],
) -> [f32; 4] {
    match lightning {
        Some(l) if index == 0 => {
            let k = l.dome_tint(0.0);
            [base[0] * k, base[1] * k, base[2] * k, base[3]]
        }
        _ => base,
    }
}

/// Upload decorative flights from both games using authored sky parameters.
fn upload_sky_objects(
    r: &mut Renderer,
    scene: &Scene,
    textures: &mut HashMap<String, BoundTexture>,
    ambient: [f32; 4],
    census: &mut Census,
) -> Result<Vec<render::SkyShell>> {
    let Some(sky) = &scene.manifest.sky else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    let mut placed = 0u32;
    for o in &sky.objects {
        let Some(cpu) = scene.meshes.get(&o.geometry) else {
            continue;
        };
        let parts = upload_submeshes(r, cpu)?;
        for _ in 0..o.count {
            let index = placed;
            placed += 1;
            let seed = crate::classdb::fnv1a_lowercase(&o.geometry)
                ^ crate::classdb::fnv1a_lowercase(&scene.manifest.map).rotate_left(11)
                ^ index.wrapping_mul(0x9e37_79b9);
            let flight = render::SkyFlight::new(
                o.height,
                o.distance,
                [
                    o.velocity_x.unwrap_or([0.0; 2]),
                    o.velocity_y.unwrap_or([0.0; 2]),
                    o.velocity_z.unwrap_or([0.0; 2]),
                ],
                Vec3::from(o.acceleration.unwrap_or([0.0; 3])),
                o.direction_factor.unwrap_or(3.0),
                o.lifetime.unwrap_or(1000.0),
                seed,
            );
            for &(mesh, ref mat, _, flags) in &parts {
                // NOT V-clamped, unlike a dome segment: a capital ship is an
                // ordinary model that may legitimately tile its hull texture,
                // and it shares the world's texture cache. See [`sky_wrap_v`].
                let tex = match mat {
                    Some(name) => get_texture(
                        r,
                        scene,
                        textures,
                        &name.to_ascii_lowercase(),
                        render::WrapV::Repeat,
                    )?,
                    None => None,
                };
                if tex.is_none() {
                    // Routed exactly like a dome shell above. 0 on all 32
                    // maps today.
                    let what = format!("<sky-object> {}", o.geometry);
                    if census.route(&what, mat.as_deref()) != Untextured::LookupFailed {
                        continue;
                    }
                }
                out.push(render::SkyShell {
                    mesh,
                    texture: tex.map(|b| b.handle),
                    color: ambient,
                    scale: 1.0,
                    placement: render::Placement::Flight(flight),
                    yaw: 0.0,
                    // Routed off the raw MTRL word exactly like a dome
                    // shell: a `SkyObject` is a model like any other and the
                    // ELF's material rule is not specific to domes (RE §5).
                    mode: render::SkyMode::from_flags(flags),
                    lit: flags & 1 != 0,
                    depth: true,
                    emission: None,
                });
            }
        }
    }
    Ok(out)
}

fn atmosphere_texture(
    r: &mut Renderer,
    scene: &Scene,
    cache: &mut HashMap<String, BoundTexture>,
    name: &str,
) -> Result<Option<render::TextureHandle>> {
    Ok(get_texture(
        r,
        scene,
        cache,
        &name.to_ascii_lowercase(),
        render::WrapV::Repeat,
    )?
    .map(|t| t.handle))
}

fn upload_atmosphere(
    r: &mut Renderer,
    scene: &Scene,
    cache: &mut HashMap<String, BoundTexture>,
) -> Result<()> {
    let display_gain = super::look::atmosphere_display_gain(&scene.manifest);
    let cloud = scene.manifest.fog_cloud.as_ref();
    let cloud_texture = match cloud.and_then(|c| c.texture.as_deref()) {
        Some(name) => atmosphere_texture(r, scene, cache, name)?,
        None => None,
    };
    r.set_fog_cloud(cloud.zip(cloud_texture).map(|(config, texture)| {
        let ground = scene.heightfield();
        render::FogCloud {
            texture,
            range: config.range.unwrap_or([40.0, 90.0]),
            color: config
                .color
                .unwrap_or([192.0, 192.0, 192.0, 160.0])
                .map(|c| c.clamp(0.0, 255.0) as u8),
            velocity: config.velocity.unwrap_or([0.0; 2]),
            rotation: config.rotation.unwrap_or(0.1),
            height: config.height.unwrap_or(16.0),
            size: config.particle_size.unwrap_or(28.0),
            density: config.particle_density.unwrap_or(30.0).max(0.0) as u32,
            display_gain,
            ground: Box::new(move |x, z| ground.as_ref().map_or(0.0, |g| g.height_at(x, z))),
        }
    }));
    let dust = super::dust::load(scene, |name| atmosphere_texture(r, scene, cache, name))?;
    r.set_dust(dust);
    r.set_sun_flare(scene.manifest.sun_flare.as_ref().map(|s| render::SunFlare {
        distance: (scene.manifest.lighting.far_scene_range.unwrap_or(2000.0) * 0.9).min(2000.0),
        display_gain,
        angle: s.angle,
        color: s.color,
        size: s.size,
        flare_out_size: s.flare_out_size,
        num_flare_outs: s.num_flare_outs,
        initial_alpha: s.initial_alpha,
        halo_rings: s.halo_rings,
        spike_color: s.spike_color,
        spike_size: s.spike_size,
    }));
    Ok(())
}

fn get_terrain_texture(
    r: &mut Renderer,
    scene: &Scene,
    cache: &mut HashMap<String, BoundTexture>,
    name: &str,
) -> Result<Option<BoundTexture>> {
    let detail = scene.manifest.terrain_detail.as_ref().filter(|(_, patch)| {
        scene.manifest.source_game == "bf2" && patch.is_finite() && *patch > 0.0
    });
    let Some(((detail_name, patch), detail)) =
        detail.and_then(|row| scene.textures.get(&row.0).map(|texture| (row, texture)))
    else {
        return get_texture(r, scene, cache, name, render::WrapV::Repeat);
    };
    let key = format!("\u{3}terrain:{name}:{detail_name}");
    if let Some(&binding) = cache.get(&key) {
        return Ok(Some(binding));
    }
    if test_dropped_texture(name) {
        return Ok(None);
    }
    let Some(base) = scene.textures.get(name) else {
        return Ok(None);
    };
    let mips = base.mips();
    let detail_mips = detail.mips();
    let handle = r
        .add_texture(&render::TextureDesc {
            width: base.width,
            height: base.height,
            mips: &mips,
            sampler: render::SamplerMode::Linear,
            wrap_v: render::WrapV::Repeat,
            srgb: true,
            detail: Some(render::DetailDesc {
                width: detail.width,
                height: detail.height,
                mips: &detail_mips,
                scale: 8.0,
                world_patch: *patch,
            }),
            scroll: [0.0; 2],
            uv_scale: [1.0; 2],
            label: Some(name),
        })
        .map_err(|error| anyhow!("terrain texture {name}: {error}"))?;
    let binding = BoundTexture { handle };
    cache.insert(key, binding);
    Ok(Some(binding))
}

/// Look up (uploading on first use) one texture; None when the manifest
/// references one we didn't bake. The draw's pass is NOT decided here — it
/// comes from the GLB material (`assets::CpuMesh::submeshes`).
fn get_texture(
    r: &mut Renderer,
    scene: &Scene,
    cache: &mut HashMap<String, BoundTexture>,
    name: &str,
    wrap_v: render::WrapV,
) -> Result<Option<BoundTexture>> {
    get_material_texture(r, scene, cache, name, wrap_v, [0.0; 2])
}

fn get_material_texture(
    r: &mut Renderer,
    scene: &Scene,
    cache: &mut HashMap<String, BoundTexture>,
    name: &str,
    wrap_v: render::WrapV,
    scroll: [f32; 2],
) -> Result<Option<BoundTexture>> {
    // `Scene::textures` is keyed lowercase: a `modl` segment spells its
    // texture with whatever case the artist typed, and the `tex_` chunk that
    // holds it spells it another way.
    let name = &name.to_ascii_lowercase();
    // A CLAMPED upload is a different GPU sampler, so it is a different
    // entry: `sky_naboo2` is both BF1/BF2 NAB2's dome and a world texture,
    // and the world's copy must stay REPEAT. The clamped entry hides behind a
    // prefix no `tex_` name can spell (U+0001 is not a filename character),
    // so the two never collide.
    let key = match wrap_v {
        render::WrapV::Repeat => name.clone(),
        render::WrapV::ClampToEdge => format!("{CLAMP_V_KEY_PREFIX}{name}"),
    };
    // Material rates belong to the binding, not the shared image.
    let key = scrolling_key(key, scroll);
    if let Some(&h) = cache.get(&key) {
        return Ok(Some(h));
    }
    let Some(t) = scene
        .textures
        .get(name)
        .or_else(|| scene.textures.get(lowrez_source(name)?))
    else {
        return Ok(None);
    };
    if test_dropped_texture(name) {
        return Ok(None);
    }
    let bt = BoundTexture {
        handle: upload_cpu_texture_inner(r, t, name, scroll, [1.0, 1.0], wrap_v)?,
    };
    cache.insert(key, bt);
    Ok(Some(bt))
}

fn scrolling_key(key: String, scroll: [f32; 2]) -> String {
    if scroll == [0.0; 2] {
        key
    } else {
        format!(
            "\u{2}scroll:{:08x}:{:08x}:{key}",
            scroll[0].to_bits(),
            scroll[1].to_bits()
        )
    }
}

/// Does `OPENBF_TEST_DROP_TEXTURE` name this (lowercased) texture? TEST-ONLY:
/// the hook `tests/walkabout_stats.rs` proves itself with — the draw takes the
/// untextured fallback, as it would if the texture had stopped binding.
/// Unset, nothing is dropped.
fn test_dropped_texture(name: &str) -> bool {
    std::env::var("OPENBF_TEST_DROP_TEXTURE").is_ok_and(|v| v.eq_ignore_ascii_case(name))
}

/// Cache-key prefix for the CLAMPED upload of a texture — see [`get_texture`].
/// U+0001 cannot appear in a `tex_` chunk name, so a clamped entry can never
/// shadow a real one.
const CLAMP_V_KEY_PREFIX: &str = "\u{1}clampv:";

mod terrain;
#[cfg(test)]
use terrain::{terrain_chunk_indices, TERRAIN_CHUNK_CELLS};
use terrain::{upload_terrain_chunks, upload_terrain_layer_chunks};

#[cfg(test)]
mod tests;

/// The map's `DomeInfo.Stars()` for the renderer (issue #16,
/// `analysis/re-stars.md`). `EnableBottom` is read by BF2's draw only.
pub(crate) fn star_field(m: &manifest::Manifest) -> Option<render::StarField> {
    let st = m.sky.as_ref()?.stars.as_ref()?;
    Some(render::StarField {
        count: st.count,
        seed: st.seed,
        twinkle_factor: st.twinkle_factor,
        twinkle_frequency: st.twinkle_frequency,
        color: st.color,
        bright_fraction: st.bright_fraction,
        alpha_min: st.alpha_min,
        color_saturation: st.color_saturation,
        enable_bottom: st.enable_bottom && m.source_game == "bf2",
    })
}

/// The ocean foam layer's UV scale over the water mesh, whose UVs repeat
/// every `metres_per_repeat`: the base's repeat over the foam's own
/// (`ManifestWaterConfig::foam_metres_per_repeat`; both KAM1s 40 / 32 =
/// 1.25). 1 when either is missing (a pre-2026-10-28 bake).
pub(crate) fn foam_uv_scale(cfg: Option<&manifest::ManifestWaterConfig>) -> [f32; 2] {
    match cfg.and_then(|c| Some((c.metres_per_repeat?, c.foam_metres_per_repeat?))) {
        Some((base, foam)) => [base[0] / foam[0], base[1] / foam[1]],
        None => [1.0, 1.0],
    }
}
