//! Load a baked map directory (bake's manifest.ron + GLB + KTX2) into
//! CPU-side scene data for the renderer. Runtime never touches LVL.

mod mesh;
mod render_ground;
pub use mesh::{load_glb, load_glb_joints, load_skinned_glb, CpuSkinnedMesh, SkinnedAsset};
mod visual_clips;
pub use visual_clips::load_visual_skinned_glb;

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// The manifest schema is shared with the bake: ONE definition in the
/// `manifest` crate, `Serialize` for `bake` and `Deserialize` here, so the
/// writer and the reader cannot drift. Re-exported under their old paths.
///
/// Every `#[serde(default)]` over there is what lets a manifest baked before
/// a field existed still load; that crate says which bake each one keeps
/// readable.
pub use manifest::{
    CommonManifest, LoadScreen, LoadStyle, Manifest, ManifestClass, ManifestCollision,
    ManifestCutter, ManifestDoor, ManifestDoorPrim, ManifestDoorTrigger, ManifestFog,
    ManifestGeometry, ManifestHead, ManifestInstance, ManifestLayer, ManifestLighting, ManifestNav,
    ManifestPath, ManifestPrim, ManifestRegion, ManifestSky, ManifestSkyModel, ManifestSkyObject,
    ManifestWater,
};

/// The bake this working tree ships (`engine/assets_baked`), resolved from
/// the crate's own source path.
///
/// FOR IN-TREE TOOLING ONLY — the tests and the census/capture examples,
/// which all read the one bake next to their source. The shipped binary takes
/// `--assets <dir>` and defaults to a RELATIVE `assets_baked` (`main.rs`), so
/// nothing on a player's machine goes through here.
pub fn baked_root() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets_baked"))
}

/// Which of the two baked texture tiers the by-name loaders resolve.
///
/// `bake::tier` writes the PS2 disc's own pixels as `<name>.ktx2` and, for
/// the textures a PC install could better (363 files today, counted with
/// `find engine/assets_baked -name '*.tier2.ktx2'`), `<name>.tier2.ktx2` beside it
/// (`Manifest::texture_tier2`). Both draw the same silhouette; only the
/// resolution differs, so this is a look toggle and never a sim input.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextureTier {
    /// `<name>.ktx2` always — the strict-faithful tier. Every committed
    /// golden renders this one, so the referee never depends on whether a PC
    /// install was present at bake time.
    Ps2,
    /// `<name>.tier2.ktx2` where the bake wrote one, else the PS2 base.
    /// The DEFAULT: PLAN §LOCKED FEATURE SET puts the modern look on by
    /// default and strict-faithful behind the toggle.
    #[default]
    Pc,
}

/// 0 = not resolved yet, else `Ps2 as u8 + 1` / `Pc as u8 + 1`.
static TEXTURE_TIER: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// `OPENBF_TEXTURE_TIER=1|2`, as the environment spells it. Split out so the
/// parse is testable without mutating a process-wide environment.
fn tier_from_env_value(v: Option<&str>) -> TextureTier {
    match v {
        None | Some("") => TextureTier::default(),
        Some("1") => TextureTier::Ps2,
        Some("2") => TextureTier::Pc,
        Some(other) => {
            eprintln!(
                "OPENBF_TEXTURE_TIER={other:?} is neither 1 (PS2) nor 2 (PC) \
                 — using the default"
            );
            TextureTier::default()
        }
    }
}

/// The tier this process loads textures at: [`set_texture_tier`] when
/// something has set one, else `OPENBF_TEXTURE_TIER`, else the default.
///
/// Read here rather than threaded through `load_scene`/`load_common` so that
/// no call site has to carry a look setting it does not care about — a
/// texture flip is a LOAD-TIME choice, never a mid-round one.
pub fn texture_tier() -> TextureTier {
    use std::sync::atomic::Ordering::Relaxed;
    match TEXTURE_TIER.load(Relaxed) {
        1 => TextureTier::Ps2,
        2 => TextureTier::Pc,
        _ => {
            // Two threads racing here resolve the same environment to the
            // same answer, so the last store wins with nothing lost.
            let t = tier_from_env_value(std::env::var("OPENBF_TEXTURE_TIER").ok().as_deref());
            set_texture_tier(t);
            t
        }
    }
}

/// Pin the tier for this process, overriding the environment. The golden
/// tests call this with [`TextureTier::Ps2`]: a committed golden must not
/// move because the machine that ran it had a PC install.
pub fn set_texture_tier(tier: TextureTier) {
    let v = match tier {
        TextureTier::Ps2 => 1u8,
        TextureTier::Pc => 2u8,
    };
    TEXTURE_TIER.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// The `n` of the row-major `n x n` heightfield grid at the HEAD of a baked
/// terrain mesh's vertices, or `None` when the mesh is not one.
///
/// `bake::terrain_glb` appends the shipped `PTCH` replacement meshes — the
/// pre-clipped terrain the PS2 draws where a cutter reaches into a render
/// patch (analysis/re-terrain-cutters.md §0.3) — AFTER the grid, on the 13
/// terrains that ship any. So the vertex count stopped being `n²` and `sqrt`
/// stopped finding `n`.
///
/// The grid states its own width instead: the bake writes every row from the
/// same `(x - n/2) * gridUnit`, bit for bit, so row 1 starts at the first
/// vertex after 0 that repeats `positions[0]`'s world X — and row 0 cannot
/// contain one, because X increases across it.
pub fn terrain_grid_size(positions: &[[f32; 3]]) -> Option<usize> {
    let x0 = positions.first()?[0];
    let n = 1 + positions.iter().skip(1).position(|p| p[0] == x0)?;
    (n >= 2 && n * n <= positions.len()).then_some(n)
}

/// Every baked map under `assets_dir`: (map KEY = directory name, source
/// game, directory). The key is the directory, not the world name inside the
/// LVL — both discs ship worlds called hoth/yavin1/kas2/naboo2/geo1/end1.
pub fn discover_maps(assets_dir: &Path) -> Vec<(String, String, std::path::PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = pak::read_dir(assets_dir) else {
        return out;
    };
    for dir in rd {
        let m = dir.join("manifest.ron");
        if let Ok(txt) = pak::read_to_string(&m) {
            if let Ok(h) = ron::from_str::<ManifestHead>(&txt) {
                // Key by DIRECTORY name, not the world name inside the LVL:
                // BF1 and BF2 both ship worlds called "hoth", "yavin1",
                // "kas2", "naboo2", "geo1", "end1" and "kamino1", so the
                // manifest name is not unique and cannot address a mission
                // file. The bake names directories <game>_<lvl> (bf1_hot1).
                let key = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&h.map)
                    .to_owned();
                out.push((key, h.source_game, dir));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub struct CpuMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
    /// Per index range: (start, count, material name, alpha mode, MTRL
    /// flags).
    ///
    /// The alpha mode is the glTF `alphaMode` the bake wrote on that
    /// primitive's material (ALPHA RULE v2, `bake::alpha`) mapped straight
    /// onto the renderer's pass — OPAQUE -> `Opaque`, MASK -> `Mask`, BLEND
    /// -> `Blend`. It is a property of the DRAW, not of the texture: the same
    /// texture can appear in submeshes of all three modes. **The world path
    /// routes on this and nothing else.**
    ///
    /// The flags are the segment's RAW PS2 MTRL word, which the bake also
    /// writes on the material (`extras.flags`, `bake::glb`). Only the SKY
    /// path reads them, because the ELF draws a sky shell with the GS state
    /// its material asks for rather than through one composited pass — see
    /// `render::SkyMode` and `analysis/re-sky-dome-placement.md` §5.
    pub submeshes: Vec<(u32, u32, String, render::DrawLayer, u32)>,
    /// Per submesh (aligned with `submeshes`, or empty for a mesh with no
    /// GLB materials): the specular second pass the bake wrote as
    /// `extras.spec_pass` (`bake::spec`, queue SPEC), `None` where the
    /// segment draws none. Read through [`CpuMesh::spec_pass`].
    pub spec: Vec<Option<SpecPass>>,
    /// Per submesh (aligned with `submeshes`, or empty): the PS2 lighting
    /// path the bake wrote as `extras.light` (`bake::glb::light_path`, queue
    /// LITEB) — `Lit` where it wrote none, which is every non-map GLB and
    /// every map GLB baked before LITEB. Read through [`CpuMesh::light_path`].
    pub light: Vec<render::LightPath>,
    pub scroll: Vec<[f32; 2]>,
    /// The model's own sphere `[x, y, z, r]`, model space, as the map bake
    /// wrote it on the mesh (`extras.sphere`, `bake::glb::model_sphere`,
    /// card BPNT); `None` on every other GLB.
    pub sphere: Option<[f32; 4]>,
    /// The top of the model's vis box, model space (`extras.vis_top`); `None` from
    /// a bake older than it.
    pub vis_top: Option<f32>,
    /// A corrected presentation grid after the unchanged simulation grid:
    /// (first vertex, vertices per side). Absent on PS2 and older bakes.
    pub render_grid: Option<(usize, usize)>,
    /// Donor drawing reconstructed from canonical contact surfaces. Keep it
    /// visible and let it pull the chase camera clear instead of fading it.
    /// Absent on PS2 and older bakes; never read by the simulation.
    pub contact_surface: bool,
}

/// One submesh's specular second pass as the bake wrote it
/// (`analysis/re-specular-pass.md` §2–§3, `[O]`; `bake::spec`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct SpecPass {
    /// The MTRL's specular power; the table's `s = power / 255`.
    pub power: u32,
    /// The flat pass colour, 1.0 = the table texel as is; `None` = the
    /// map's sun colour (BF1's white or zero `B`), which the viewer resolves.
    pub rgb: Option<[f32; 3]>,
}

impl SpecPass {
    /// The pass's flat colour under a map whose `SunInfo` colour is
    /// `sun_color` (the manifest's: LINEAR): the bake's own, or — BF1's
    /// white or zero `B` — the sun's bytes raw over 128, so up to ~2×
    /// (`analysis/re-specular-pass.md` §2, `[O]`). The GS gets the bytes, so
    /// the linear colour is taken back to display units first (card B1DK,
    /// `analysis/re-kamino-decks.md` §7 `[O]`: Kamino's (65,85,105) is
    /// (0.508,0.664,0.820), where the linear colour gave a pass 3-5× too
    /// weak). One rule for the world models and the side models alike: the
    /// side bake knows no map, so a soldier's sun-coloured pass resolves
    /// here too (queue SPEC2).
    pub fn rgb_under(&self, sun_color: [f32; 3]) -> [f32; 3] {
        self.rgb
            .unwrap_or(sun_color.map(|c| render::srgb_encode(c) * 255.0 / 128.0))
    }
}

impl CpuMesh {
    pub(crate) fn mesh_data<'a>(&'a self, indices: &'a [u32]) -> render::MeshData<'a> {
        render::MeshData {
            positions: &self.positions,
            normals: &self.normals,
            uvs: &self.uvs,
            colors: Some(&self.colors),
            indices,
        }
    }

    /// Submesh `i`'s specular second pass, or `None` when it draws none.
    pub fn spec_pass(&self, i: usize) -> Option<SpecPass> {
        self.spec.get(i).copied().flatten()
    }

    /// Submesh `i`'s lighting path, `Lit` when the bake wrote none.
    pub fn light_path(&self, i: usize) -> render::LightPath {
        self.light.get(i).copied().unwrap_or_default()
    }

    pub fn uv_scroll(&self, i: usize) -> [f32; 2] {
        self.scroll.get(i).copied().unwrap_or_default()
    }

    /// The V range of submesh `i`'s own vertices — the UVs its index range
    /// touches, not the whole model's.
    ///
    /// The sky path needs it to decide the segment's V address mode
    /// (`game::viewer::sky_wrap_v`): a shell segment whose V stays inside
    /// [0, 1] must not wrap, and one that genuinely tiles (BF2 POL1, BF2
    /// TAN1) must. It lives here rather than in `viewer` so nothing outside
    /// this type re-derives a submesh's vertices from its indices.
    ///
    /// `None` for an out-of-range submesh or one whose index range is empty.
    pub fn submesh_v_range(&self, i: usize) -> Option<(f32, f32)> {
        let (start, count, ..) = *self.submeshes.get(i)?;
        let idx = self.indices.get(start as usize..(start + count) as usize)?;
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &v in idx {
            let uv = self.uvs.get(v as usize)?;
            lo = lo.min(uv[1]);
            hi = hi.max(uv[1]);
        }
        (lo <= hi).then_some((lo, hi))
    }
}

pub struct CpuTexture {
    pub width: u32,
    pub height: u32,
    /// Mip 0 RGBA8.
    pub rgba: Vec<u8>,
    /// Further mip levels (1..), tightly packed RGBA8, as baked from the
    /// PS2 source mips.
    pub extra_mips: Vec<Vec<u8>>,
    /// PS2 detail companion (`<base>.dtl.ktx2`), modulated x2 over the base
    /// at the same UVs — see `bake::detail` / RE_LOG "BF1 detail textures".
    pub detail: Option<Box<CpuTexture>>,
}

impl CpuTexture {
    pub fn mips(&self) -> Vec<&[u8]> {
        std::iter::once(self.rgba.as_slice())
            .chain(self.extra_mips.iter().map(Vec::as_slice))
            .collect()
    }
}

pub struct Scene {
    /// The baked map directory this scene was loaded from — the root the
    /// manifest's own relative paths (the nav sidecars) resolve against.
    pub dir: std::path::PathBuf,
    pub manifest: Manifest,
    pub meshes: HashMap<String, CpuMesh>,
    pub terrain: Option<CpuMesh>,
    /// The water surface mesh, when the map ships one.
    pub water: Option<CpuMesh>,
    /// The `LOWR` far-LOD terrain sheet, when the map's `tern` ships one
    /// (`bake::lowres_glb`). Loaded whenever it is baked; whether it is DRAWN
    /// is `viewer::lowres_range`'s answer.
    pub lowres: Option<CpuMesh>,
    /// The terrain's texture layers above LTEX[0], one submesh per layer in
    /// ascending LTEX order (`bake::terrain_glb::terrain_layers_to_glb`,
    /// card TALPH) — drawn over `terrain`, read by nothing in the sim.
    pub terrain_layers: Option<CpuMesh>,
    /// Baked textures, keyed by LOWERCASE name — see `load_scene`.
    pub textures: HashMap<String, CpuTexture>,
    /// Model collision by LOWERCASE geometry name. Never iterated in the sim
    /// path — [`Scene::collision_world`] walks the manifest's instance order
    /// and only LOOKS UP here.
    pub collision: HashMap<String, CollisionGeom>,
    /// Terrain cutters, straight off the manifest.
    pub cutters: Vec<ManifestCutter>,
}

/// One geometry's collision as loaded: the `coll` mesh (a bare triangle soup)
/// and its `prim` primitives.
pub struct CollisionGeom {
    pub mesh: Option<CpuMesh>,
    pub prims: Vec<ManifestPrim>,
}

/// Find a layer whose authored world names explicitly identify `mode`.
/// Unlike [`mode_layer_index`], this never substitutes a generic design layer.
/// Diagnostics use it to distinguish a measured mode from fallback content.
pub fn authored_mode_layer_index(m: &Manifest, mode: &str) -> Option<usize> {
    // A BF2 map's `lvl_` sublevels each hold one or more worlds, and the
    // mode is spelled in a WORLD name, not reliably in the first one: the
    // bake names a layer after `worlds[0]`, so Mygeeto's conquest layer
    // reads as "myg1_turrets" and only its second world is
    // "myg1_cp-conquest". Match against every world in the layer, and
    // accept the spellings the discs actually use — measured over the 16
    // baked BF2 maps: `_con` (tat2 only), `_conquest`, `_cp-conquest`.
    //
    // PROVISIONAL. Which sublevels a mission really loads is mission-Lua
    // data (HOLD FOR FABLE); this is a name match, not a claim about the
    // PS2's loading rules.
    let aliases: Vec<&str> = match mode {
        "con" => vec!["con", "conquest", "cp-conquest"],
        "ctf" | "1flag" => vec![
            "1flag_flags",
            "1flag_paths",
            "2_flag_ctf",
            "ctf",
            "1flag",
            "1ctf",
            "ctfcp",
        ],
        // Space Assault's layer, as the six scripts and worlds spell it: `spa6_obj_hangar`
        // with `spa6_obj_spawn`, `spa3_swing_obj`, `spa8_ass_spawn`, `spa1_swingassault`.
        "ass" => vec!["obj", "obj_spawn", "ass_spawn", "swing_obj", "swingassault"],
        "eli" => vec!["tdm"],
        other => vec![other],
    };
    let map = m.map.clone();
    let matches_alias = |l: &ManifestLayer| {
        l.worlds.iter().chain(std::iter::once(&l.name)).any(|w| {
            aliases.iter().any(|a| {
                w.eq_ignore_ascii_case(&format!("{map}_{a}"))
                    || w.to_ascii_lowercase().ends_with(&format!("_{a}"))
            })
        })
    };
    m.layers.iter().position(matches_alias)
}

/// Which layer a battle merges, including maps whose Conquest posts live in
/// a generic design layer. Existing callers retain that fallback behavior.
pub fn mode_layer_index(m: &Manifest, mode: &str) -> Option<usize> {
    authored_mode_layer_index(m, mode).or_else(|| {
        // Layers named for a DIFFERENT mode must never be picked as fallback.
        const MODE_WORDS: [&str; 9] = [
            "con", "conquest", "ctf", "1flag", "1ctf", "hunt", "tdm", "eli", "assault",
        ];
        // No layer names this mode (Felucia and Mustafar have no conquest
        // sublevel at all): fall back to the biggest layer that is not
        // named for another mode — in practice the map's `_design` pass,
        // which is where those maps keep their command posts.
        let other_mode = |w: &String| {
            let w = w.to_ascii_lowercase();
            MODE_WORDS
                .iter()
                .any(|m| w.ends_with(&format!("_{m}")) || w.contains(&format!("_{m}_")))
                || w.contains("campaign")
        };
        m.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.worlds.iter().any(other_mode) && !other_mode(&l.name))
            .max_by_key(|(_, l)| l.instances.len())
            .map(|(i, _)| i)
    })
}

impl Scene {
    /// Merge the world layer for `mode` ("con", "ctf", ...) into the base
    /// instances/regions. The mode is spelled in a WORLD name and a layer may
    /// hold several, so EVERY world in a layer is matched, with the spellings
    /// the discs use and a fallback for the maps that ship no mode sublevel
    /// — see the body. Naming a layer after `worlds[0]` alone found the
    /// conquest posts on 1 of 16 BF2 maps. BF1 maps have no layers
    /// (everything is in the base world). Returns the layer name.
    pub fn apply_mode_layer(&mut self, mode: &str) -> Option<String> {
        self.manifest.spawn_only_posts = mode == "eli";
        let idx = mode_layer_index(&self.manifest, mode)?;
        let layer = self.manifest.layers.remove(idx);
        self.manifest.regions.extend(layer.regions);
        self.manifest.instances.extend(layer.instances);
        // The layer's AI data merges the same way the runtime stacks it (PLAN
        // Phase A design decision 1): its PLAN REPLACES the base graph when it
        // ships one (BF2 frees the previous graph on every `plan` chunk — last
        // loaded wins), and its hints and barriers are ADDED to the base
        // world's. A layer with hints but no plan keeps the base graph.
        if let Some(ln) = layer.nav {
            let base = self.manifest.nav.get_or_insert_with(|| ManifestNav {
                plan: None,
                hints: Vec::new(),
                barriers: Vec::new(),
            });
            if ln.plan.is_some() {
                base.plan = ln.plan;
            }
            base.hints.extend(ln.hints);
            base.barriers.extend(ln.barriers);
        }
        Some(layer.name)
    }

    /// Build the sim-side ground-height query from the baked terrain mesh
    /// (world-space grid positions, row-major z*size+x — bake emits them in
    /// exactly that order). Returns None when the map has no terrain.
    pub fn heightfield(&self) -> Option<core::heightfield::Heightfield> {
        let t = self.terrain.as_ref()?;
        let size = terrain_grid_size(&t.positions)? as u32;
        let p0 = t.positions[0];
        let unit = t.positions[1][0] - p0[0];
        // `unit <= 0.0` is FALSE for NaN, so a non-finite grid step would slip
        // through and reach f32::clamp in height_at, which panics on a NaN
        // bound — a crash on the first sim tick. `grid_unit` is read raw from
        // the disc with no finiteness check, and bake::terrain_glb already
        // uses `is_finite() && > 0.0` for the layer scale; match it.
        if !(unit.is_finite() && unit > 0.0 && p0.iter().all(|c| c.is_finite())) {
            return None;
        }
        // Only the leading size² grid participates; appended PTCH replacement
        // vertices remain render geometry.
        // The donor may draw more terrain than the PS2 reference. Keep that
        // mesh intact, but use the reference's centered query frame: adding a
        // larger grid offset rounds float32 cell fractions differently even
        // at ordinary in-bounds spawn points.
        let query_size = self.manifest.terrain_query_size.unwrap_or(size) as usize;
        let render_size = size as usize;
        if query_size < 2
            || query_size > render_size
            || !(render_size - query_size).is_multiple_of(2)
        {
            return None;
        }
        let start = (render_size - query_size) / 2;
        let first = t.positions[start * render_size + start];
        if self.manifest.terrain_query_size.is_some()
            && (first[0] != -(query_size as f32 / 2.0) * unit
                || first[2] != -(query_size as f32 / 2.0 - 1.0) * unit)
        {
            return None;
        }
        let heights = if start == 0 {
            t.positions[..query_size * query_size]
                .iter()
                .map(|p| p[1])
                .collect()
        } else {
            (start..start + query_size)
                .flat_map(|z| {
                    t.positions[z * render_size + start..z * render_size + start + query_size]
                        .iter()
                        .map(|p| p[1])
                })
                .collect()
        };
        Some(core::heightfield::Heightfield {
            size: query_size as u32,
            unit,
            offset_x: -first[0],
            offset_z: -first[2],
            heights,
        })
    }

    /// Resolve an instance class to its `entc` GeometryName — the part of
    /// [`Scene::resolve_model`] before the gmod lookup, and the key the baked
    /// `coll`/`prim` data is filed under. Case-insensitive, like the munged
    /// data (the engine hashes names lowercased).
    pub fn resolve_geometry(&self, class: &str) -> Option<&str> {
        self.resolve_class(class)?.geometry.as_deref()
    }

    /// The manifest's class row, case-insensitively — the render-geometry
    /// link AND the class's collision-layer properties, which
    /// [`Scene::collision_world`] needs together. On BF2 a class with a
    /// `build_zone` resolves to that zone's row except for armed buildings:
    /// their intact model/rig/collision must be available for building them.
    /// The sim's BuildingTurret state selects the pad while down. Other
    /// unbuilt props keep the static zone substitution (card VTUR2).
    /// BF1 keeps the class's own row (card BZONE1).
    pub fn resolve_class(&self, class: &str) -> Option<&ManifestClass> {
        let find = |n: &str| {
            self.manifest
                .classes
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(n))
        };
        let c = find(class)?;
        match &c.build_zone {
            Some(zone)
                if self.manifest.source_game.eq_ignore_ascii_case("bf2")
                    && !crate::vehicle_class::TURRET_BASES
                        .iter()
                        .any(|b| c.base.eq_ignore_ascii_case(b)) =>
            {
                find(zone)
            }
            _ => Some(c),
        }
    }

    /// Build the sim's static collision world from the manifest's INSTANCE
    /// ORDER (which is the body order the sim then visits — see
    /// `core::collision`'s determinism notes).
    ///
    /// MUST be called AFTER [`Scene::apply_mode_layer`]: BF2 keeps its conquest
    /// instances in a mode layer, so a world built before the merge is missing
    /// most of the map.
    ///
    /// The world is the UNION of the SOLDIER and ORDNANCE subsets of each
    /// instance's class — the coll mesh and the primitives that class's `entc`
    /// collision properties (and, on BF2, the primitives' own `MASK` byte) put
    /// in either — and every body carries a [`core::collision::Layers`] mask
    /// saying which of the two it is in. A body in NEITHER is not built at
    /// all. See [`crate::collision_layers`] for the rule and its RE source
    /// (`analysis/re-collision-layers.md` §2, §4); the port's old
    /// `*vehicle*`/`*flyer*`/`*ordnance*` NAME rule is gone, and the VEHICLE
    /// subset is not built (P7).
    ///
    /// BODY ORDER (the sim's, and `core::collision`'s determinism contract):
    /// the manifest's instance order after [`Scene::apply_mode_layer`] — base
    /// instances, then the merged layer's in merge order — and within an
    /// instance the `coll` mesh first, then the prims in `prims` order,
    /// skipping the bodies in neither subset and those of a kind this port
    /// does not build. The SOLDIER-masked subsequence of that list is exactly
    /// the list this built before the ordnance subset was added.
    ///
    /// DOORS (queue DR2): a door is built as authored — shut, in the subsets
    /// its class names — and every primitive its baked rig
    /// ([`ManifestDoor`]) moves is a MOVER of the world, driven by
    /// a [`core::door::Door`] that [`Scene::collision_and_doors`] returns
    /// beside it (`analysis/re-bf2-doors.md` §2, §3 [V][O]). This world alone
    /// has every door shut; a sim wants [`Scene::install_collision`].
    pub fn collision_world(&self) -> core::collision::CollisionWorld {
        self.collision_and_doors().0
    }

    /// The world and its doors, into a sim: what every caller that steps a
    /// [`core::sim::Sim`] on this scene does.
    pub fn install_collision(&self, sim: &mut core::sim::Sim) {
        let (world, doors) = self.collision_and_doors();
        sim.collision = world;
        sim.doors = doors;
        let key = self.dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        sim.boundary = crate::map_boundary::boundary(key, &self.manifest.paths);
    }

    /// Install the map's authored damage regions (a region name's `damage` prefix).
    pub fn install_damage_regions(&self, sim: &mut core::sim::Sim) {
        sim.damage_regions = self
            .manifest
            .regions
            .iter()
            .filter_map(|r| {
                let volume =
                    core::region::Region::from_rows(&r.shape, r.rotation, r.translation, r.size)?;
                core::sim::DamageRegion::parse(&r.name, volume)
            })
            .collect();
    }

    /// Install the water level and WMAP wet cells together for every sim loader.
    /// Older manifests without a mask retain the whole-plane fallback. A scene
    /// without water clears all water inputs; render-only scenes need not call this.
    pub fn install_water(&self, sim: &mut core::sim::Sim) {
        sim.water_level = self.manifest.water.as_ref().map(|w| w.height);
        sim.water_oscillates = self.manifest.water.as_ref().is_some_and(|w| {
            w.config.as_ref().is_none_or(|c| {
                !c.ocean
                    && (self.manifest.source_game.eq_ignore_ascii_case("bf1")
                        || c.oscillation_enable)
            })
        });
        sim.water_mask = self
            .manifest
            .water
            .as_ref()
            .and_then(ManifestWater::gameplay_mask)
            .and_then(|m| {
                core::water::WetMask::new(m.grid, m.cell_size, m.half_extent, &m.cells())
            });
    }

    /// [`Scene::collision_world`] and the door entities that move its bodies.
    /// `OPENBF_DOORS_SOLID=1` returns no door, so every door stays shut —
    /// the port before DR1, kept as the reversal.
    pub fn collision_and_doors(&self) -> (core::collision::CollisionWorld, Vec<core::door::Door>) {
        self.collision_and_doors_with(std::env::var("OPENBF_DOORS_SOLID").as_deref() != Ok("1"))
    }

    /// [`Scene::collision_and_doors`] with the door rule chosen by the
    /// caller: `doors_open = false` builds no door entity.
    pub fn collision_and_doors_with(
        &self,
        doors_open: bool,
    ) -> (core::collision::CollisionWorld, Vec<core::door::Door>) {
        let (world, doors, _) = self.collision_parts(doors_open);
        (world, doors)
    }

    /// Each body of [`Scene::collision_world`] as `<class> <instance name>`,
    /// by body index: what a diagnostic calls the thing a soldier walks into
    /// (the playtest black box, card BLKB). It builds the world again; a dev
    /// tool's cost, paid once per battle.
    pub fn collision_body_labels(&self) -> Vec<String> {
        let doors_open = std::env::var("OPENBF_DOORS_SOLID").as_deref() != Ok("1");
        self.collision_parts(doors_open)
            .2
            .into_iter()
            .map(|ii| {
                let inst = &self.manifest.instances[ii as usize];
                format!("{} {}", inst.class, inst.name)
            })
            .collect()
    }

    /// [`Scene::collision_and_doors_with`], and the instance each body is of.
    fn collision_parts(
        &self,
        doors_open: bool,
    ) -> (
        core::collision::CollisionWorld,
        Vec<core::door::Door>,
        Vec<u32>,
    ) {
        use core::collision::{Body, CollisionWorld, Cutter, Layers, Shape, TriMesh};
        use core::door::{Door, DoorBody, Trigger};
        // Build cache only — one TriMesh per geometry, shared by Arc. Never
        // iterated; the output Vec's order is the instance order.
        let mut built: HashMap<String, Option<std::sync::Arc<TriMesh>>> = HashMap::new();
        let mut bodies = Vec::new();
        // The instance each body is of, by body index.
        let mut of: Vec<u32> = Vec::new();
        let mut doors: Vec<Door> = Vec::new();
        let mut movers: Vec<(usize, core::collision::Aabb)> = Vec::new();
        for (ii, inst) in self.manifest.instances.iter().enumerate() {
            let Some(class) = self.resolve_class(&inst.class) else {
                continue;
            };
            let Some(geom) = class.geometry.as_deref() else {
                continue;
            };
            // Card VDROP: a `trap` (Endor's end_weap_treesmash, both games)
            // has no body. Its two log cylinders hang from bones out in the
            // trees; the bake keeps them bone-relative, which laid them
            // crossed on the ground at the instance — on the speeder bikes'
            // path (160 of 171 riders lost in 8 BF2 rounds died grinding on
            // them). The PS2's bikes pass 0.3-0.8 m from both origins at
            // 28-40 m/s (`lead_tmp/hais/live_end1.csv`, `live_b1end1.csv`).
            // PROVISIONAL until the trap itself is built (card TRAP).
            if class.base.eq_ignore_ascii_case("trap") {
                continue;
            }
            let key = geom.to_ascii_lowercase();
            let Some(cg) = self.collision.get(&key) else {
                continue;
            };
            let mut next = body_layers(cg, &class.collision).into_iter();

            let m = crate::viewer::instance_matrix(inst.rotation, inst.translation);
            // DR2: this instance's door, when its class has a rig.
            let rig = class.door.as_ref().filter(|_| doors_open);
            let mut door = rig.map(|r| Door {
                locked: r.locked,
                triggers: r
                    .triggers
                    .iter()
                    .map(|t| Trigger {
                        center: m.transform_point3(glam::Vec3::from(t.position)),
                        radius: t.radius,
                    })
                    .collect(),
                frames: r.frames,
                instance: ii,
                origin: m.w_axis.truncate(),
                open_sound: r.open_sound.clone(),
                close_sound: r.close_sound.clone(),
                locked_sound: r.locked_sound.clone(),
                ..Door::default()
            });
            if let Some(cpu) = cg.mesh.as_ref() {
                let layers = next.next().unwrap_or(Layers::NONE);
                // Degenerate meshes are skipped, like `CameraBlockers` does.
                let arc = built
                    .entry(key)
                    .or_insert_with(|| core::collision::trimesh(&cpu.positions, &cpu.indices))
                    .clone();
                if let (true, Some(arc)) = (layers != Layers::NONE, arc) {
                    bodies.push(Body::with_layers(m, Shape::Mesh(arc), layers));
                    of.push(ii as u32);
                }
            }
            for p in &cg.prims {
                let layers = next.next().unwrap_or(Layers::NONE);
                if layers == Layers::NONE {
                    continue;
                }
                let Some(shape) = prim_shape(p) else {
                    continue;
                };
                // The prim's XFRM is composed straight onto the instance
                // matrix — PROVISIONAL: its `PRNT` node's own transform is
                // assumed identity (see `mungefs::coll`); a door's prims come
                // from the bake already through their bones' bind matrices
                // (`bake::bind_pose`, card STDOR).
                let local = crate::viewer::instance_matrix(p.rotation, p.translation);
                // An armed building's prim stands through its `PRNT` node's
                // bind matrix (card TCOL, `analysis/re-turret-collision.md`
                // 1.1, 2: every static node binds at the identity; the gun's
                // nodes do not - Geonosis's cannon's gun is bound 1.60 m
                // up). The sim then turns the gun's ones with the gun
                // (`core::sim::PosedParts`, `vehicle_pads::posed_parts`).
                let local = match armed_node(class, &p.parent) {
                    Some((rig, n)) => crate::viewer::turret_pose::bind_of(rig, n) * local,
                    None => local,
                };
                let pm = m * local;
                // DR2: a prim the door's clip moves gets its world pose per
                // frame, `instance · D(f) · XFRM`, and an envelope holding
                // them all.
                let motion = rig.and_then(|r| {
                    r.prims
                        .iter()
                        .find(|q| q.name.eq_ignore_ascii_case(&p.name))
                });
                if let (Some(dm), Some(door)) = (motion, door.as_mut()) {
                    let frames: Vec<(glam::Quat, glam::Vec3)> = dm
                        .motion
                        .iter()
                        .map(|(q, t)| {
                            let d = glam::Mat4::from_rotation_translation(
                                glam::Quat::from_array(*q),
                                glam::Vec3::from(*t),
                            );
                            let (_, r, t) = (m * d * local).to_scale_rotation_translation();
                            (r, t)
                        })
                        .collect();
                    let mut env = core::collision::Aabb::empty();
                    for (r, t) in &frames {
                        let a = Body::with_layers(
                            glam::Mat4::from_rotation_translation(*r, *t),
                            shape.clone(),
                            layers,
                        )
                        .aabb;
                        env.add(a.min);
                        env.add(a.max);
                    }
                    movers.push((bodies.len(), env));
                    door.bodies.push(DoorBody {
                        body: bodies.len(),
                        frames,
                    });
                }
                bodies.push(Body::with_layers(pm, shape, layers));
                of.push(ii as u32);
            }
            if let Some(d) = door.filter(|d| !d.bodies.is_empty() && d.frames >= 2) {
                doors.push(d);
            }
        }
        // Each body knows its instance (card TURR: a building turret's
        // camera ray passes its own building).
        for (b, &ii) in bodies.iter_mut().zip(&of) {
            b.instance = Some(ii);
        }
        // `new`, not the struct literal: it builds the broad phase over the
        // bodies (`core::collision::world::BodyGrid`), which every soldier and
        // ordnance query on this world then reads instead of scanning all of
        // them. The answers are the same either way.
        let world = CollisionWorld::with_movers(
            bodies,
            self.cutters
                .iter()
                .map(|c| Cutter {
                    sphere: c.sphere,
                    planes: c.planes.iter().map(|p| glam::Vec4::from(*p)).collect(),
                })
                .collect(),
            movers,
        );
        (world, doors, of)
    }

    /// Resolve an instance class to the modl name to draw:
    /// class -> entc GeometryName -> gmod best LOD. Classes without geometry
    /// (sounds, item pickups, effect emitters) resolve to None.
    /// Names are case-insensitive in the munged data (the engine hashes them
    /// lowercased): TAT2's `tat2_bldg_wall_40M` class geometry names a
    /// lowercase `tat2_bldg_wall_40m` gmod/modl.
    pub fn resolve_model(&self, class: &str) -> Option<&str> {
        let cls = self.resolve_class(class)?;
        let geom = cls.geometry.as_deref()?;
        self.model_for_geometry(geom)
    }

    pub fn model_for_geometry<'a>(&'a self, geom: &'a str) -> Option<&'a str> {
        let model = match self
            .manifest
            .geometries
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(geom))
        {
            Some(g) => g.model.as_str(),
            // Some geometries have no gmod (single-LOD models keep modl name).
            None => geom,
        };
        if self.meshes.contains_key(model) {
            return Some(model);
        }
        self.meshes
            .keys()
            .find(|k| k.eq_ignore_ascii_case(model))
            .map(String::as_str)
    }

    /// The `LOWD` model of `class`'s geometry — what the PS2's far scene
    /// draws it as (card LOD, `analysis/re-lod.md` §4 [O]) — when its gmod
    /// has one other than the model [`Scene::resolve_model`] draws, and its
    /// mesh loaded.
    pub fn resolve_lowd(&self, class: &str) -> Option<&str> {
        let geom = self.resolve_class(class)?.geometry.as_deref()?;
        let g = self
            .manifest
            .geometries
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(geom))?;
        let (_, lowd) = g
            .lods
            .iter()
            .find(|(tier, _)| tier.eq_ignore_ascii_case("LOWD"))?;
        if lowd.eq_ignore_ascii_case(&g.model) {
            return None;
        }
        if let Some((k, _)) = self.meshes.get_key_value(lowd.as_str()) {
            return Some(k.as_str());
        }
        self.meshes
            .keys()
            .find(|k| k.eq_ignore_ascii_case(lowd))
            .map(String::as_str)
    }

    /// The far scene's LOWD extension of `class`'s geometry
    /// ([`manifest::ManifestGeometry::far`], card UTAS), when it has one and
    /// its mesh loaded — drawn only past `NearSceneRange[1]`.
    pub fn resolve_far(&self, class: &str) -> Option<&str> {
        let cls = self
            .manifest
            .classes
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(class))?;
        let geom = cls.geometry.as_deref()?;
        let far = self
            .manifest
            .geometries
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(geom))?
            .far
            .as_deref()?;
        self.meshes.get_key_value(far).map(|(k, _)| k.as_str())
    }
}

pub struct CommonAssets {
    pub manifest: CommonManifest,
    pub meshes: HashMap<String, CpuMesh>,
    pub textures: HashMap<String, CpuTexture>,
    /// Model collision by LOWERCASE geometry name, like `Scene::collision`.
    pub collision: HashMap<String, CollisionGeom>,
}

/// Load a `bake common` directory (COMMON.LVL / INGAME.LVL output).
pub fn load_common(dir: &Path) -> Result<CommonAssets> {
    let manifest: CommonManifest =
        ron::from_str(&pak::read_to_string(dir.join("common.ron")).context("common.ron")?)
            .context("parse common.ron")?;
    let mut meshes = HashMap::new();
    for name in &manifest.models {
        let path = dir.join("models").join(format!("{name}.glb"));
        meshes.insert(name.clone(), load_glb(&path).with_context(|| name.clone())?);
    }
    // Lowercase keys, for the reason `load_scene` gives.
    let mut textures = HashMap::new();
    for name in &manifest.textures {
        textures.insert(
            name.to_ascii_lowercase(),
            load_texture(&dir.join("textures"), name)?,
        );
    }
    let collision = load_collision(dir, &manifest.collision)?;
    Ok(CommonAssets {
        manifest,
        meshes,
        textures,
        collision,
    })
}

/// Load the `coll` meshes named by a manifest's collision list, keyed
/// LOWERCASE by geometry name (first entry wins — the bake already merged
/// duplicates, this only guards a hand-edited manifest).
fn load_collision(
    dir: &Path,
    entries: &[ManifestCollision],
) -> Result<HashMap<String, CollisionGeom>> {
    let mut out: HashMap<String, CollisionGeom> = HashMap::new();
    for e in entries {
        if let std::collections::hash_map::Entry::Vacant(v) =
            out.entry(e.geometry.to_ascii_lowercase())
        {
            v.insert(load_collision_geom(dir, e)?);
        }
    }
    Ok(out)
}

/// One geometry's collision: its `coll` mesh GLB (relative to `dir`, the
/// bake that wrote the entry) and its prims.
pub fn load_collision_geom(dir: &Path, e: &ManifestCollision) -> Result<CollisionGeom> {
    let mesh = match e.glb.as_deref() {
        Some(rel) => Some(load_glb(&dir.join(rel)).with_context(|| rel.to_owned())?),
        None => None,
    };
    Ok(CollisionGeom {
        mesh,
        prims: e.prims.clone(),
    })
}

/// An armed building's rig node named `parent` (a prim's `PRNT`), with
/// the rig; `None` for any other class or a name the rig lacks (card TCOL).
pub fn armed_node<'a>(
    class: &'a manifest::ManifestClass,
    parent: &str,
) -> Option<(&'a manifest::ManifestTurretRig, usize)> {
    if !class.base.eq_ignore_ascii_case("armedbuilding") {
        return None;
    }
    let rig = class.turret_rig.as_ref()?;
    let n = rig
        .nodes
        .iter()
        .position(|x| x.name.eq_ignore_ascii_case(parent.trim()))?;
    Some((rig, n))
}

/// A baked prim as a body shape, `None` for a kind the port does not build.
pub fn prim_shape(p: &ManifestPrim) -> Option<core::collision::Shape> {
    use core::collision::Shape;
    Some(match p.kind.as_str() {
        "sphere" => Shape::Sphere { radius: p.dims[0] },
        // `dims[1]` is the FULL height (mungefs::coll).
        "cylinder" => Shape::Cylinder {
            radius: p.dims[0],
            half_height: p.dims[1] * 0.5,
        },
        "box" => Shape::Box {
            half: glam::Vec3::from(p.dims),
        },
        _ => return None,
    })
}

/// The subsets each body of `cg` is in under a class's collision lists, in
/// the model's BODY LIST order (mesh first when there is one, then every
/// prim): the order the sim visits bodies in, and the list the class's
/// properties name bodies out of. Every prim is listed, whether or not its
/// kind is one this port builds, so a property naming one resolves the way
/// the engine's does. `Layers::NONE` = in neither subset, not built.
pub fn body_layers(
    cg: &CollisionGeom,
    class_collision: &[(String, String)],
) -> Vec<core::collision::Layers> {
    body_layers_for(cg, class_collision, false)
}

fn body_layers_for(
    cg: &CollisionGeom,
    class_collision: &[(String, String)],
    flyer: bool,
) -> Vec<core::collision::Layers> {
    use crate::collision_layers::{layer_bodies, Layer, FLYER_USES_VEHICLE};
    use core::collision::Layers;
    let refs = body_refs(cg);
    // The three subsets, from the same body list and the same rule.
    let select = if flyer {
        crate::collision_layers::bf2_flyer_layer_bodies
    } else {
        layer_bodies
    };
    let soldier = select(&refs, class_collision, Layer::Soldier);
    let ordnance = select(&refs, class_collision, Layer::Ordnance);
    let vehicle = layer_bodies(&refs, class_collision, Layer::Vehicle);
    let flyer = if class_collision.iter().any(|(k, _)| k == FLYER_USES_VEHICLE) {
        &vehicle
    } else {
        &soldier
    };
    (0..refs.len())
        .map(|i| {
            (if soldier[i] {
                Layers::SOLDIER
            } else {
                Layers::NONE
            }) | (if ordnance[i] {
                Layers::ORDNANCE
            } else {
                Layers::NONE
            }) | (if vehicle[i] {
                Layers::VEHICLE
            } else {
                Layers::NONE
            }) | (if flyer[i] {
                Layers::FLYER
            } else {
                Layers::NONE
            })
        })
        .collect()
}

fn body_refs(cg: &CollisionGeom) -> Vec<crate::collision_layers::BodyRef> {
    use crate::collision_layers::BodyRef;
    let mut refs = Vec::with_capacity(cg.prims.len() + 1);
    if cg.mesh.is_some() {
        refs.push(BodyRef::mesh());
    }
    refs.extend(cg.prims.iter().map(|p| BodyRef::prim(&p.name, p.mask)));
    refs
}

pub fn missile_target_points(
    cg: &CollisionGeom,
    class_collision: &[(String, String)],
) -> Vec<glam::Vec3> {
    use crate::collision_layers::{layer_bodies, Layer};
    let refs = body_refs(cg);
    let points = cg
        .mesh
        .iter()
        .map(|_| glam::Vec3::ZERO)
        .chain(cg.prims.iter().map(|p| glam::Vec3::from(p.translation)));
    layer_bodies(&refs, class_collision, Layer::Targetable)
        .into_iter()
        .zip(points)
        .filter_map(|(targetable, point)| targetable.then_some(point))
        .collect()
}

/// A vehicle class's hull (card VHULL): the bodies of its geometry's
/// collision `cg` in model space, each in the subsets the class's lists put
/// it in (the vehicle classes read the same six properties as a building,
/// analysis/re-collision-layers.md §8) — the `coll` mesh, then the prims
/// the port builds, each at its `XFRM`. The bake has already put each
/// `XFRM` through its `PRNT` bone's bind matrix, for a side vehicle (card
/// RKHUL) and a map's own vehicle class alike (card RKHULT,
/// `bake::bind_pose::prims_at_bind`).
/// Bodies in neither subset are left out.
pub fn hull_parts(
    cg: &CollisionGeom,
    class_collision: &[(String, String)],
    hit_locations: &[(String, f32)],
) -> Vec<core::collision::HullPart> {
    hull_parts_for(cg, class_collision, hit_locations, false)
}

pub fn bf2_flyer_hull_parts(
    cg: &CollisionGeom,
    class_collision: &[(String, String)],
    hit_locations: &[(String, f32)],
) -> Vec<core::collision::HullPart> {
    hull_parts_for(cg, class_collision, hit_locations, true)
}

fn hull_parts_for(
    cg: &CollisionGeom,
    class_collision: &[(String, String)],
    hit_locations: &[(String, f32)],
    flyer: bool,
) -> Vec<core::collision::HullPart> {
    use crate::collision_layers::{layer_bodies, Layer};
    use core::collision::{HullPart, Layers, Shape};
    let mut out = Vec::new();
    let refs = body_refs(cg);
    let building = layer_bodies(&refs, class_collision, Layer::Building);
    let terrain = layer_bodies(&refs, class_collision, Layer::Terrain);
    let mut subsets = building.into_iter().zip(terrain);
    let mut next = body_layers_for(cg, class_collision, flyer).into_iter();
    if let Some(cpu) = cg.mesh.as_ref() {
        let layers = next.next().unwrap_or(Layers::NONE);
        let (building, terrain) = subsets.next().unwrap_or((false, false));
        if let (true, Some(arc)) = (
            layers != Layers::NONE || building || terrain,
            core::collision::trimesh(&cpu.positions, &cpu.indices),
        ) {
            out.push(
                HullPart::new(glam::Mat4::IDENTITY, Shape::Mesh(arc), layers)
                    .with_flyer_subsets(building, terrain),
            );
        }
    }
    for p in &cg.prims {
        let layers = next.next().unwrap_or(Layers::NONE);
        let (building, terrain) = subsets.next().unwrap_or((false, false));
        // The class's HitLocation for this prim (issue #41: the x4 critical
        // prims), matched by name.
        let mult = hit_locations
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(&p.name))
            .map_or(1.0, |(_, m)| *m);
        // A weak spot must be strikable: BF2 lists every one of its own in
        // the ordnance subset; a BF1 prim given a factor ([`BF1_WEAK_SPOTS`])
        // joins it here.
        let layers = if mult > 1.0 {
            layers | Layers::ORDNANCE
        } else {
            layers
        };
        let in_any = layers != Layers::NONE || building || terrain;
        let Some(shape) = prim_shape(p).filter(|_| in_any) else {
            continue;
        };
        out.push(
            HullPart::new(
                crate::viewer::instance_matrix(p.rotation, p.translation),
                shape,
                layers,
            )
            .with_hit_mult(mult)
            .with_flyer_subsets(building, terrain),
        );
    }
    out
}

/// The factor of a weak spot this port gives a vehicle: BF2's own, on every
/// one of its critical prims.
pub const WEAK_SPOT_FACTOR: f32 = 4.0;

/// OURS: the weak spots of BF1's armoured ground vehicles, as collision
/// prims of their own models. BF1's classes author no factor over 1 (its
/// only `HitLocation` lines are two x0), though its models carry these
/// critical prims; BF2 gives each twin x4. One game: a tank has a weak spot
/// on every map, so a launcher's lock has a part to go to
/// (`analysis/re-hitzones.md` Part A §3.1). The hovernaut and the MTT have
/// no BF2 twin and take the same factor on the prim their models carry.
/// The fighter tanks and the AT-TE are in [`BF1_ADDED_WEAK_SPOTS`]. Bikes,
/// animals and flyers have none in either game.
/// measure: maintainer playtest of rockets against BF1 vehicles (B7).
pub const BF1_WEAK_SPOTS: &[(&str, &[&str])] = &[
    ("all_hover_combatspeeder", &["p_crithit"]),
    ("all_hover_hovernaut", &["p_crithit"]),
    ("cis_hover_aat", &["p_crithit"]),
    ("cis_hover_mtt", &["p_crithit"]),
    ("cis_tread_hailfire", &["p_crithit"]),
    ("cis_walk_spider", &["p_crithit1", "p_crithit2"]),
    ("imp_walk_atat", &["p_neck_soldier"]),
    ("imp_walk_atst", &["p_crit_hit"]),
    ("imp_walk_atst_jungle", &["p_crit_hit"]),
    ("imp_walk_atst_snow", &["p_crit_hit"]),
];

/// The shape of an [`AddedWeakSpot`]: a cylinder (radius, full height along
/// its own Y) or a box (half extents).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AddedShape {
    Cylinder(f32, f32),
    Box([f32; 3]),
}

/// A weak spot this port adds to a class's hull, in model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddedWeakSpot {
    pub class: &'static str,
    pub shape: AddedShape,
    /// Rotation rows, as a baked prim's.
    pub rows: [[f32; 3]; 3],
    pub at: [f32; 3],
}

const UPRIGHT: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The fighter tank's disc: its axis along the hull, tilted 9 degrees up.
const fn tank_disc(class: &'static str) -> AddedWeakSpot {
    AddedWeakSpot {
        class,
        shape: AddedShape::Cylinder(0.5, 0.2),
        rows: [
            [1.0, 0.0, 0.0],
            [0.0, 0.156_586_29, 0.987_664_3],
            [0.0, -0.987_664_3, 0.156_586_29],
        ],
        at: [0.0, 0.899_732_65, -3.852_299_7],
    }
}

/// OURS: weak spots for the BF1 vehicles whose own models give none to
/// strike, each BF2's part for the same hull, at BF2's place.
/// - The two fighter tanks carry no critical prim. Their hulls are BF2's
///   prim for prim, and BF2 added a disc on the rear face.
/// - The AT-TE's own critical prim sits inside its hull boxes, where no
///   round reaches it. BF2 moved it to a box under the front of the belly.
///
/// measure: maintainer playtest of rockets against BF1 vehicles (B7).
pub const BF1_ADDED_WEAK_SPOTS: &[AddedWeakSpot] = &[
    tank_disc("imp_hover_fightertank"),
    tank_disc("rep_hover_fightertank"),
    AddedWeakSpot {
        class: "rep_walk_atte",
        shape: AddedShape::Box([0.958_399_8, 0.707_916_14, 2.081_097_8]),
        rows: UPRIGHT,
        at: [0.0, 3.113_692_3, 3.141_827],
    },
];

/// The hull parts this port adds to a BF1 class ([`BF1_ADDED_WEAK_SPOTS`]):
/// in the ordnance subset only, so only rounds and sight rays meet them.
pub fn added_weak_parts(
    db: &crate::classdb::ClassDb,
    class: &str,
) -> Vec<core::collision::HullPart> {
    use core::collision::{HullPart, Layers, Shape};
    if db.is_bf2() {
        return Vec::new();
    }
    let chain = db.chain(class);
    BF1_ADDED_WEAK_SPOTS
        .iter()
        .filter(|w| chain.iter().any(|d| d.name.eq_ignore_ascii_case(w.class)))
        .map(|w| {
            let shape = match w.shape {
                AddedShape::Cylinder(radius, height) => Shape::Cylinder {
                    radius,
                    half_height: height * 0.5,
                },
                AddedShape::Box(half) => Shape::Box {
                    half: glam::Vec3::from(half),
                },
            };
            HullPart::new(
                crate::viewer::instance_matrix(w.rows, w.at),
                shape,
                Layers::ORDNANCE,
            )
            .with_hit_mult(WEAK_SPOT_FACTOR)
        })
        .collect()
}

/// A class's `HitLocation` lines (`"<collision prim> <factor>"`, BF2
/// `FUN_00214600`, at most 10, only those not 1.0; `analysis/re-ai-difficulty.md`
/// 9.2): the vehicles' critical prims (issue #41). A BF1 class in
/// [`BF1_WEAK_SPOTS`] takes its rows from there, over any authored factor
/// for the same prim (the hailfire's x0).
pub fn hit_locations(db: &crate::classdb::ClassDb, class: &str) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = db
        .props_all(class, "HitLocation")
        .into_iter()
        .filter_map(|v| {
            let mut w = v.split_whitespace();
            let name = w.next()?.to_ascii_lowercase();
            let m: f32 = w.next()?.parse().ok()?;
            (m != 1.0).then_some((name, m))
        })
        .take(10)
        .collect();
    if !db.is_bf2() {
        let ours = db.chain(class).into_iter().find_map(|d| {
            BF1_WEAK_SPOTS
                .iter()
                .find(|(c, _)| c.eq_ignore_ascii_case(&d.name))
        });
        for prim in ours.map_or(&[][..], |(_, prims)| prims) {
            out.retain(|(n, _)| n != prim);
            out.push(((*prim).to_owned(), WEAK_SPOT_FACTOR));
        }
    }
    out
}

impl Scene {
    /// Merge shared assets under the map's own (map definitions win, like
    /// the engine's level stacking): meshes, textures, classes, geometries.
    pub fn merge_common(&mut self, common: CommonAssets) -> usize {
        let mut added = 0;
        for (k, v) in common.meshes {
            if let std::collections::hash_map::Entry::Vacant(e) = self.meshes.entry(k) {
                e.insert(v);
                added += 1;
            }
        }
        for (k, v) in common.textures {
            self.textures.entry(k).or_insert(v);
        }
        for c in common.manifest.classes {
            if !self
                .manifest
                .classes
                .iter()
                .any(|x| x.name.eq_ignore_ascii_case(&c.name))
            {
                self.manifest.classes.push(c);
            }
        }
        for g in common.manifest.geometries {
            if !self
                .manifest
                .geometries
                .iter()
                .any(|x| x.name.eq_ignore_ascii_case(&g.name))
            {
                self.manifest.geometries.push(g);
            }
        }
        // Common props (command posts, crates, turrets) get their collision the
        // same way: vacant-only, so a map's own copy always wins.
        for (k, v) in common.collision {
            if let std::collections::hash_map::Entry::Vacant(e) = self.collision.entry(k) {
                e.insert(v);
            }
        }
        added
    }
}

/// Baked loading screen textures in display order.
pub struct LoadScreenAssets {
    pub sequence: Vec<(String, CpuTexture)>,
    /// BF2 stills: show ONE of `sequence` for the load instead of stepping.
    pub pool: bool,
}

fn loadscreen_index(map_dir: &Path) -> Option<LoadScreen> {
    let text = pak::read_to_string(map_dir.join("load/load.ron")).ok()?;
    ron::from_str(&text).ok()
}

/// Read one baked loading image from an unpacked or packaged map.
pub fn load_loading_image(map_dir: &Path, name: String) -> Option<(String, CpuTexture)> {
    let path = map_dir.join("load").join(format!("{name}.ktx2"));
    let bytes = pak::read(path).ok()?;
    let tex = load_ktx2(&bytes).ok()?;
    let len = (tex.width as usize)
        .checked_mul(tex.height as usize)?
        .checked_mul(4)?;
    (len != 0 && tex.rgba.len() == len).then_some((name, tex))
}

/// `<map dir>/load/load.ron` + KTX2s, or None when the map has no load screen.
pub fn load_loadscreen(map_dir: &Path) -> Option<LoadScreenAssets> {
    let ls = loadscreen_index(map_dir)?;
    let sequence = ls
        .sequence
        .into_iter()
        .map(|name| load_loading_image(map_dir, name))
        .collect::<Option<Vec<_>>>()?;
    (!sequence.is_empty()).then_some(LoadScreenAssets {
        sequence,
        pool: ls.style == LoadStyle::Pool,
    })
}

/// Read only the selected BF2 still, trying the other stills if it is missing.
pub fn load_loading_still(map_dir: &Path, preferred: &str) -> Option<(String, CpuTexture)> {
    let ls = loadscreen_index(map_dir)?;
    if ls.style != LoadStyle::Pool {
        return None;
    }
    let mut names = ls.sequence;
    if let Some(index) = names.iter().position(|name| name == preferred) {
        names[..=index].rotate_right(1);
    }
    let (name, mut texture) = names
        .into_iter()
        .find_map(|name| load_loading_image(map_dir, name))?;
    // The authored BF2 stills have 1/8-height black bars above and below
    // their 4:3 content. Remove those for the shared 4:3 zoom canvas.
    if texture.width == texture.height && texture.height.is_multiple_of(8) {
        let row = texture.width as usize * 4;
        let bar = texture.height as usize / 8 * row;
        texture.rgba.truncate(texture.rgba.len() - bar);
        texture.rgba.drain(..bar);
        texture.height -= texture.height / 4;
        texture.extra_mips.clear();
    }
    Some((name, texture))
}

/// Find the BF1 opening plate even if a later step of that sequence is missing.
pub fn load_loading_galaxy(map_dir: &Path) -> Option<(String, CpuTexture)> {
    let ls = loadscreen_index(map_dir)?;
    let name = ls.sequence.into_iter().next()?;
    (ls.style == LoadStyle::Sequence && name.starts_with("load_galacticmap"))
        .then(|| load_loading_image(map_dir, name))?
}

pub fn load_scene(dir: &Path) -> Result<Scene> {
    let manifest: Manifest =
        ron::from_str(&pak::read_to_string(dir.join("manifest.ron")).context("manifest.ron")?)
            .context("parse manifest.ron")?;

    let mut meshes = HashMap::new();
    for name in &manifest.models {
        let path = dir.join("models").join(format!("{name}.glb"));
        meshes.insert(name.clone(), load_glb(&path).with_context(|| name.clone())?);
    }
    let terrain = manifest
        .terrain_glb
        .as_ref()
        .map(|f| load_glb(&dir.join(f)))
        .transpose()
        .context("terrain glb")?;
    let water = manifest
        .water
        .as_ref()
        .map(|w| load_glb(&dir.join(&w.glb)))
        .transpose()
        .context("water glb")?;
    let lowres = manifest
        .lowres_glb
        .as_ref()
        .map(|f| load_glb(&dir.join(f)))
        .transpose()
        .context("lowres glb")?;
    let terrain_layers = manifest
        .terrain_layers_glb
        .as_ref()
        .map(|f| load_glb(&dir.join(f)))
        .transpose()
        .context("terrain layers glb")?;

    // Keyed LOWERCASE. A `tex_` chunk's NAME and the material name a `modl`
    // segment references are the SAME texture spelled with different case
    // (BF1 RhenVar 2 bakes `rhn_bldg_interior_02` and its walls ask for
    // `Rhn_bldg_interior_02`), so an exact-match lookup silently drops the
    // texture and the draw falls back to flat grey. Every lookup lowercases.
    let mut textures = HashMap::new();
    for name in &manifest.textures {
        textures.insert(
            name.to_ascii_lowercase(),
            load_texture(&dir.join("textures"), name)?,
        );
    }

    load_shared_materials(dir, &manifest.source_game, &meshes, &mut textures)?;
    let collision = load_collision(dir, &manifest.collision)?;
    let cutters = manifest.terrain_cutters.clone();
    Ok(Scene {
        dir: dir.to_path_buf(),
        manifest,
        meshes,
        terrain,
        water,
        lowres,
        terrain_layers,
        textures,
        collision,
        cutters,
    })
}

/// Exact cross-map dependencies in the shipped artwork. Read through `pak`
/// so directory and single-file builds resolve the same texture, without
/// changing any PS2 bake bytes. Existing map images always win. A source
/// is only read for a scene of the same game, from this scene's own bake
/// root (one input profile), so another copy's bake is never borrowed.
fn load_shared_materials(
    dir: &Path,
    source_game: &str,
    meshes: &HashMap<String, CpuMesh>,
    textures: &mut HashMap<String, CpuTexture>,
) -> Result<()> {
    let Some(root) = dir.parent() else {
        return Ok(());
    };
    let wanted: std::collections::BTreeSet<_> = meshes
        .values()
        .flat_map(|m| m.submeshes.iter().map(|s| s.2.to_ascii_lowercase()))
        .filter(|n| !textures.contains_key(n))
        .collect();
    for name in wanted {
        let (game, sources): (&str, &[&str]) = match name.as_str() {
            "dea1_prop_barrel_crate" => ("bf2", &["bf2_dea1"]),
            _ => continue,
        };
        if !source_game.eq_ignore_ascii_case(game) {
            continue;
        }
        for source in sources {
            let path = root.join(source).join("textures");
            if pak::is_file(path.join(format!("{name}.ktx2"))) {
                textures.insert(name.clone(), load_texture(&path, &name)?);
                break;
            }
        }
    }
    Ok(())
}

/// What [`load_battle_scene`] merged into the map, for the caller to report.
pub struct BattleLoad {
    /// The mode layer merged (`Scene::apply_mode_layer`), if the map has one.
    pub layer: Option<String>,
    /// The source game's COMMON bake (`<assets root>/common_<bf1|bf2>`) and
    /// the models it added, or why it would not load. `None` = not baked.
    pub common: Option<(std::path::PathBuf, Result<usize>)>,
}

/// Load a baked map as a battle sees it: the map, its `mode` layer, then the
/// COMMON bake of its source game merged under it ([`Scene::merge_common`]).
/// The command-post console, the recharge droids and the gun turrets are
/// COMMON-LVL classes, so a map loaded without the merge draws none of them
/// (card SKIPC, `analysis/re-skipped-classes.md`). Every view of a map — the
/// battle, the headless bench, the still renders and the gallery — loads it
/// through here. A COMMON bake that exists but will not load is not an
/// error: the map still loads, and `BattleLoad::common` says why.
pub fn load_battle_scene(dir: &Path, mode: &str) -> Result<(Scene, BattleLoad)> {
    let mut scene = load_scene(dir)?;
    anyhow::ensure!(
        mode != "eli" || authored_mode_layer_index(&scene.manifest, mode).is_some(),
        "map has no authored world layer for mode {mode}"
    );
    let layer = scene.apply_mode_layer(mode);
    let common = dir
        .parent()
        .map(|p| p.join(format!("common_{}", scene.manifest.source_game)))
        .filter(|cd| pak::exists(cd.join("common.ron")))
        .map(|cd| {
            let added = load_common(&cd).map(|c| scene.merge_common(c));
            (cd, added)
        });
    Ok((scene, BattleLoad { layer, common }))
}

pub fn load_ktx2(bytes: &[u8]) -> Result<CpuTexture> {
    let reader = ktx2::Reader::new(bytes).map_err(|e| anyhow!("{e:?}"))?;
    let hdr = reader.header();
    let (w, h) = (hdr.pixel_width, hdr.pixel_height);
    let mut levels = reader.levels();
    let level0 = levels.next().ok_or_else(|| anyhow!("no levels"))?;
    // Keep only levels whose size matches a proper RGBA8 chain — guards
    // against malformed bakes feeding the GPU short rows.
    let mut extra_mips = Vec::new();
    for (i, level) in levels.enumerate() {
        let lw = (w >> (i + 1)).max(1) as usize;
        let lh = (h >> (i + 1)).max(1) as usize;
        if level.data.len() != lw * lh * 4 {
            break;
        }
        extra_mips.push(level.data.to_vec());
    }
    Ok(CpuTexture {
        width: w,
        height: h,
        rgba: level0.data.to_vec(),
        extra_mips,
        detail: None,
    })
}

/// Load the texture `name` at [`texture_tier`], plus its `<name>.dtl.ktx2`
/// detail companion when the bake wrote one (the file's presence is the PS2
/// detailCompressed flag).
///
/// At [`TextureTier::Pc`] the override `<name>.tier2.ktx2` wins WHEN IT
/// EXISTS — see [`Manifest::texture_tier2`] for which — and it brings its own
/// mip chain. The detail companion is the PS2's either way: it is a
/// modulated x2 overlay the disc authored, not a resolution tier.
pub fn load_texture(dir: &Path, name: &str) -> Result<CpuTexture> {
    let tier2 = (texture_tier() == TextureTier::Pc)
        .then(|| dir.join(format!("{name}.tier2.ktx2")))
        .filter(|p| pak::is_file(p));
    let mut tex = match &tier2 {
        Some(p) => load_ktx2(&pak::read(p)?).with_context(|| format!("{name}.tier2"))?,
        None => load_ktx2(&pak::read(dir.join(format!("{name}.ktx2")))?)
            .with_context(|| name.to_owned())?,
    };
    if let Ok(bytes) = pak::read(dir.join(format!("{name}.dtl.ktx2"))) {
        tex.detail = Some(Box::new(
            load_ktx2(&bytes).with_context(|| format!("{name}.dtl"))?,
        ));
    }
    Ok(tex)
}

#[cfg(test)]
mod tests;
