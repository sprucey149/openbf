//! The bake <-> runtime manifest schema, in one place.
//!
//! `bake` writes `manifest.ron` / `common.ron`; `game` reads them back. Until
//! now each side carried its own copy of every struct — the bake's deriving
//! `Serialize`, the runtime's deriving `Deserialize` — and the two had to be
//! kept in step by hand. They are ONE definition here, deriving both.
//!
//! Two rules this crate exists to hold:
//!
//! * **Field order is the wire format.** RON writes fields in declaration
//!   order, so reordering a struct rewrites every manifest. The order below is
//!   the order the bake has always written; a new field goes at the END.
//! * **Every `#[serde(default)]` is load-bearing.** It is what lets a manifest
//!   baked before a field existed still load. Each one carries the note saying
//!   which bake it keeps readable. Defaults do not affect what the bake
//!   WRITES — serde only consults them when reading.
//!
//! This crate depends on nothing but `serde`: it is data, not behaviour.
//! Anything that needs `bake`'s or `game`'s internals stays there.

use serde::{Deserialize, Serialize};

mod environment;
pub use environment::{
    foliage_bin, FoliageKind, LeafLayout, ManifestFilter, ManifestFog, ManifestFoliage,
    ManifestFoliageMesh, ManifestGrassClass, ManifestHdr, ManifestLeafClass, ManifestLight,
    ManifestLighting, ManifestLightning, ManifestPointLight, ManifestPrecipitation, ManifestSky,
    ManifestSkyModel, ManifestSkyObject, ManifestSpotCone, ManifestStars, ManifestWater,
    ManifestWaterConfig, ManifestWetMask, ManifestWind,
};

pub mod anim_sets;
pub mod cloth;
pub mod combo;
pub mod fpm;
pub mod hud;
pub mod shell;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestInstance {
    pub class: String,
    pub name: String,
    /// 3x3 rotation rows + translation, Z-up PS2 world space as stored.
    pub rotation: [[f32; 3]; 3],
    pub translation: [f32; 3],
    pub props: Vec<(u32, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestRegion {
    pub shape: String,
    pub name: String,
    pub rotation: [[f32; 3]; 3],
    pub translation: [f32; 3],
    pub size: [f32; 3],
    pub props: Vec<(u32, String)>,
}

/// Entity class (entc): the instance-class side of the render-geometry chain
/// instance.class -> class.geometry -> geometry.model, plus what the world
/// needs per class: its collision layers, a door's rig and a vehicle's
/// hardpoints. The gameplay props live in `classes/<game>_<lvl>.ron`
/// ([`ClassDef`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestClass {
    pub name: String,
    /// Parent class (entc BASE), e.g. "prop", "hover", "commandpost".
    pub base: String,
    /// GeometryName prop -> key into `geometries`. None = no render geometry.
    pub geometry: Option<String>,
    /// The class's COLLISION-LAYER properties, `(layer, body)` in entc PROP
    /// order — the order is load-bearing, because a property REPLACES its
    /// layer's subset with the bodies accumulated so far
    /// (analysis/re-collision-layers.md §2).
    ///
    /// `layer` is one of `soldier | vehicle | building | terrain | ordnance |
    /// targetable | vehicle_only` (the seven hashes the two ELFs consume);
    /// `body` is the property's value lowercased verbatim — a primitive NAME,
    /// the literal `collisionmesh` (the `coll` mesh body), or the sentinels
    /// `none` / `clear` meaning "this layer's subset is EMPTY".
    ///
    /// Defaulted for bakes made before the layers were read (a manifest
    /// without it behaves as "no collision property" = every body in every
    /// layer, which is exactly the engine's default).
    #[serde(default)]
    pub collision: Vec<(String, String)>,
    /// The DOOR rig of a class whose BASE is `door` (queue DR2,
    /// `analysis/re-bf2-doors.md` §2–§3 [V][O]): the trigger spheres and the
    /// per-frame motion the `open` clip gives each collision primitive.
    /// `None` on every other class, and then not written at all, so a
    /// manifest with no door is byte-identical to one baked before the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<ManifestDoor>,
    /// Every node of the model's `skel` in MODEL space at the bind pose, for
    /// a class whose BASE is a vehicle label or `armedbuilding` (card VTUR):
    /// the seats' `FirePointName`, `TurretNodeName` and `PilotPosition` name
    /// these (`analysis/re-vehicles.md` §5, §8). Empty on every other class
    /// and on a bake from before VTUR, and then not written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hardpoints: Vec<ManifestHardpoint>,
    /// The `leafpatch` objects the class hangs on its model's hardpoints
    /// (card LEAF, `analysis/re-leafpatch.md` §2 [O]): `AttachOdf` +
    /// `AttachToHardpoint`, replayed as the PS2's setter does
    /// (`bake::leafpatch::attach`). Empty on every other class, and then not
    /// written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attach: Vec<ManifestAttach>,
    /// The class drawn IN PLACE of this one until it is built (card VTUR2):
    /// its [`BUILD_ZONE_PROP`]. `None` on every other class, and then not
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_zone: Option<String>,
    /// The EntityLights the class hangs on its model's hardpoints (card BPNT,
    /// `analysis/re-point-lights.md` §2 [O]): `AttachOdf` naming a class
    /// whose BASE is `Light`, then `AttachToHardpoint`
    /// (`bake::entity_light::attach`). Only lights with `OmniRadius` > 0 —
    /// the PS2 registers no other. Empty on every other class, and then not
    /// written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<ManifestClassLight>,
    /// Particle effects attached at model-space bind frames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ManifestAttachedEffect>,
    /// The node rig of an armed building's model (card VTURD,
    /// `analysis/re-turret-draw.md` 2 [O]) and of a health / ammo / vehicle
    /// droid's (base `powerupstation`, card PWST): every `skel` node's bind
    /// matrix and which node each drawn submesh rides, so the viewer can turn
    /// the gun with its aim and move the droid's nodes. `None` on every other
    /// class, and then not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turret_rig: Option<ManifestTurretRig>,
}

/// A prop's attached effect and optional repeat-delay bounds in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestAttachedEffect {
    pub effect: String,
    pub hardpoint: String,
    pub pos: [f32; 3],
    pub axes: [[f32; 3]; 3],
    pub delay: Option<[f32; 2]>,
}

/// An armed building's or a powerup droid's model as rigid parts
/// ([`ManifestClass::turret_rig`]).
/// The PS2 keeps one whole model-space matrix per named node and draws each
/// segment through its node's (`re-turret-draw.md` 2.4), so a part drawn at
/// the bind pose moves by `node(now) · bind⁻¹`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestTurretRig {
    /// The model's `skel` nodes, in `skel` order.
    pub nodes: Vec<ManifestRigNode>,
    /// Per primitive of the drawn model's GLB (its segments in order after
    /// the bake has cut each soft segment into one per node, bar the empty
    /// ones): the index in `nodes` of the node it is bound to, `None` for a
    /// segment bound to none.
    pub parts: Vec<Option<u32>>,
}

/// One node of a [`ManifestTurretRig`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestRigNode {
    /// As the `skel` spells it.
    pub name: String,
    /// Its parent's index in the rig; `None` for a root.
    pub parent: Option<u32>,
    /// Its MODEL-space bind matrix: the X, Y and Z axes, then the origin.
    pub bind: [[f32; 3]; 4],
}

/// One EntityLight a class carries ([`ManifestClass::lights`]): an omni at
/// the hardpoint's MODEL-space bind origin — its world position is that
/// point through the instance matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestClassLight {
    /// The light class, as the ODF spells it.
    pub class: String,
    pub pos: [f32; 3],
    /// The light class's `Color` bytes (never Overbright).
    pub color: [u8; 3],
    /// `OmniRadius`, metres.
    pub radius: f32,
    /// The light class's `Static`.
    pub is_static: bool,
}

/// The entc PROP (fnv 0x97C931C5; its name is not cracked) by which BF2's
/// buildable turrets — `tur_bldg_laser`, `_beam`, `_chaingun`,
/// `_guided_rocket` in SIDE/TUR.LVL — name `com_prop_buildzone`, beside a
/// `com_holo_turret` hologram (0x48542041). The PS2 shows that flat pad, not
/// the turret, where one stands at battle start; their `built` twins
/// (`tur_weap_built_gunturret`, `tur_bldg_built_beam`, …) author neither and
/// stand as turrets. PROVISIONAL [D]: PCSX2 frames of BF2 COR1, TAT2, YAV1
/// (card VTUR2); how an engineer builds one is untraced. Two map classes
/// author it too, in no conquest world: DEA1's `dea1_prop_hack_panel`
/// (→ `dea1_prop_panel`, [H] the same "until built" meaning) and END1's
/// `end_weap_treesmash` (itself). The game applies it on BF2 only: BF1's
/// `com_weap_gunturret` and `com_prop_barrier_02` author it as well, and
/// whether BF1's turrets start built is open (card BZONE1).
pub const BUILD_ZONE_PROP: u32 = 0x97C9_31C5;

/// One attached `leafpatch` object: the class it makes and where, as rows 1
/// (up) and 3 (origin) of the hardpoint's MODEL-space bind matrix — the only
/// rows the leaf object keeps (re-leafpatch §5: yaw and scale are dropped).
/// Its world frame is these rows times the instance matrix (row vectors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestAttach {
    /// The `leafpatch` class, as the ODF spells it.
    pub class: String,
    /// The hardpoint, as the ODF spells it.
    pub hardpoint: String,
    pub up: [f32; 3],
    pub pos: [f32; 3],
}

/// One `skel` node of a vehicle or armed-building model: its name (as the
/// skel spells it) and its bind-pose origin in model space (+x right, +y up,
/// +z forward — the frame the model is drawn in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestHardpoint {
    pub name: String,
    pub pos: [f32; 3],
    /// Its model-space bind AXES (X, Y, Z), beside `pos` the bind matrix a
    /// rider hangs by (card VRIDE, `analysis/re-vehicle-riders.md` 3: the
    /// Jedi starfighter's `hp_active` is pitched 22 degrees). `None` for a
    /// weapon's nodes from older bakes and a bake from before the card, and then not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axes: Option<[[f32; 3]; 3]>,
}

/// A hand-held weapon model's `skel` nodes (card FXMF,
/// `analysis/re-muzzle-flash.md` §4.1), baked beside its GLB by `bake
/// weapon` at [`weapon_nodes_path`]: each node's origin in its PARENT node's
/// frame — what the PS2 resolves `FirePointName` to. Every soldier weapon's
/// nodes hang off an identity root, so that is also the model frame the
/// weapon is drawn in. Empty for a model with no `skel`.
pub type WeaponNodes = Vec<ManifestHardpoint>;

/// `<side dir>/weapons/<model>.nodes.ron`.
pub fn weapon_nodes_path(side_dir: &std::path::Path, model: &str) -> std::path::PathBuf {
    side_dir.join("weapons").join(format!("{model}.nodes.ron"))
}

/// A music stream's ADPCM (`baked: false` in a stream index), copied out of
/// its sound LVL byte for byte by the bake (card PACK), so the game streams it
/// from its own data: `<index dir>/music/<lvl stem>_<data offset hex>.bin`.
/// A segment's `offset` is inside this file.
pub fn music_body(source: &str, data_offset: u64) -> String {
    let stem = source.rsplit_once('.').map_or(source, |(s, _)| s);
    format!("music/{}_{data_offset:x}.bin", stem.to_ascii_lowercase())
}

/// A BF2 `door` class's rig, baked from its ODF (`AnimationTrigger`,
/// `AnimationName`, `Animation`), its model's `skel` and the named clip of
/// its `zaa_` bank (`analysis/re-bf2-doors.md` §2 hops 2–5, §3 hops 7–9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDoor {
    /// The `AnimationTrigger` pairs, ≤ 3, in ODF order — a soldier inside
    /// any of the spheres opens the door.
    pub triggers: Vec<ManifestDoorTrigger>,
    /// The clip's frame count; BF2 plays it end to end in (frames − 1)/30 s.
    pub frames: u16,
    /// Every collision primitive the clip MOVES, by prim name, with its
    /// MODEL-space motion per frame: the delta `D(f) = B(f)·B(0)⁻¹` of its
    /// `PRNT` bone, as (rotation xyzw, translation) — the prim's world
    /// transform at frame f is `instance · D(f) · XFRM`. Frame 0 is the
    /// identity (the door shut, where the prim was authored).
    pub prims: Vec<ManifestDoorPrim>,
    /// Every DRAWN submesh the clip moves (queue DRV): a door model's
    /// segments are hard-bound to a `skel` bone by their `BNAM` and stored in
    /// model space, so a submesh's world transform at frame f is
    /// `instance · D(f)` with `D` its bone's delta, the rule [`Self::prims`]
    /// follows. `submesh` is the index of the model GLB's primitive — the
    /// model's segments in order, bar the empty ones the GLB skips. Empty on
    /// a bake from before DRV, and then not written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ManifestDoorPart>,
    /// The ODF's `OpenSound` / `CloseSound` (`re-bf2-doors.md` §2 hop 5,
    /// §3): played on the trigger's rising and falling edge. `None` where
    /// the ODF names none, and on a bake from before DRV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_sound: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_sound: Option<String>,
}

/// One drawn submesh of a door the clip moves (see [`ManifestDoor::parts`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDoorPart {
    pub submesh: u32,
    /// The segment's `BNAM` bone.
    pub bone: String,
    /// `frames` entries of (rotation xyzw, translation), MODEL space.
    pub motion: Vec<([f32; 4], [f32; 3])>,
}

/// One `AnimationTrigger` pair: the hardpoint (a `skel` node) and the radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDoorTrigger {
    pub hardpoint: String,
    pub radius: f32,
    /// The hardpoint's MODEL-space position at the clip's frame 0.
    pub position: [f32; 3],
}

/// One moving collision primitive of a door (see [`ManifestDoor::prims`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDoorPrim {
    pub name: String,
    /// `frames` entries of (rotation xyzw, translation).
    pub motion: Vec<([f32; 4], [f32; 3])>,
}

/// Geometry LOD table (gmod). `model` = best-detail modl name (LOD0 first).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestGeometry {
    pub name: String,
    pub model: String,
    /// All LOD entries as stored: (tier, modl name), e.g. ("LOWD", "xLOWD").
    pub lods: Vec<(String, String)>,
    /// The LOWD triangles that `model` does not have — the far scene's
    /// extension (card UTAS, `analysis/re-far-lowd.md` §6, `bake::far_lowd`):
    /// a baked model drawn per instance only past `NearSceneRange[1]`. `None`
    /// on every geometry but BF2 UTA1's cliffs, and skipped when `None` so the
    /// other manifests do not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub far: Option<String>,
    /// A vehicle chunk geometry's `skel` nodes in model space (issue #49):
    /// where its `ChunkSmokeNodeName` smoke rides. Empty on every other
    /// geometry, and skipped when empty so the other manifests do not change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hardpoints: Vec<ManifestHardpoint>,
}

/// Zero Editor path (spawn paths, AI paths): nodes = (position, quaternion
/// as stored — (w, x, y, z), see mungefs::path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPath {
    pub name: String,
    pub nodes: Vec<([f32; 3], [f32; 4])>,
    /// Defaulted: absent in bakes made before path props were read.
    #[serde(default)]
    pub props: Vec<(String, u32)>,
}

/// Game-mode world layer (BF2 maps: per-mode `wrld`s inside `lvl_` wrappers,
/// e.g. tat2_con + tat2_turrets). The runtime merges the layer matching the
/// selected mode into the base world at load (`game::assets::Scene`'s
/// `apply_mode_layer`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestLayer {
    /// Key = first world's name in the wrapper ("tat2_con").
    pub name: String,
    /// Defaulted: absent in bakes made before the world list was recorded.
    #[serde(default)]
    pub worlds: Vec<String>,
    pub regions: Vec<ManifestRegion>,
    pub instances: Vec<ManifestInstance>,
    /// This layer's own AI data — its `Hint`/`BARR` nodes, and its planning
    /// graph when it ships a `plan` of its own. `None` only when the layer
    /// authors none of the three; a layer with hints but no plan is
    /// `Some((plan: None, ...))` and falls back to the base graph. APPENDED
    /// after `instances`, and defaulted so a manifest baked before the nav
    /// data still loads.
    #[serde(default)]
    pub nav: Option<ManifestNav>,
}

/// `source_game` for a manifest baked before the field existed: those are all
/// BF1 (pre-P5).
fn default_game() -> String {
    "bf1".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub map: String,
    /// "bf1" | "bf2" — from the model material record size (BF2 = 25 B),
    /// falling back to "bf2" when per-mode world layers exist. Defaulted to
    /// "bf1" for pre-P5 bakes.
    #[serde(default = "default_game")]
    pub source_game: String,
    pub terrain_name: Option<String>,
    pub sky_name: Option<String>,
    pub terrain_glb: Option<String>,
    pub models: Vec<String>,
    pub textures: Vec<String>,
    /// Textures at least one DRAW composites — DERIVED from the ALPHA RULE v2
    /// verdicts the bake writes on the GLB materials (`bake::alpha`), and
    /// **READ BY NOTHING**. The renderer picks a draw's pass from its
    /// material's `alphaMode`, not from this list, because a texture can be
    /// OPAQUE on one draw and BLEND on another; the field is kept for
    /// manifest compatibility (and so a report can see the rollup at a
    /// glance). Defaulted so a manifest baked before this field still loads.
    #[serde(default)]
    pub transparent: Vec<String>,
    /// Which source filled a texture's classic-plus TIER 2: (name, "pc").
    /// Only textures that HAVE a second tier are listed, so a BF1 bake — and
    /// any bake run without the PC install — writes an empty list. The base
    /// tier is not listed because there is nothing to say about it: every
    /// `textures` entry has a `<name>.ktx2` and it is always the PS2 disc's
    /// own pixels. See `bake::tier`, and [`Manifest::texture_tier2`] for the
    /// override's file name. Defaulted for bakes made before the tier system.
    #[serde(default)]
    pub texture_tiers: Vec<(String, String)>,
    pub classes: Vec<ManifestClass>,
    pub geometries: Vec<ManifestGeometry>,
    pub regions: Vec<ManifestRegion>,
    pub instances: Vec<ManifestInstance>,
    /// Absent in pre-P4 bakes.
    #[serde(default)]
    pub paths: Vec<ManifestPath>,
    /// Absent in BF1 bakes (single world).
    #[serde(default)]
    pub layers: Vec<ManifestLayer>,
    /// The map's own sun/ambient/fog, reduced from its `sky_` and `lght`
    /// chunks by `bake::lighting` (PROVISIONAL — see that module). Absent in
    /// pre-2026-08-31 bakes, which fall back to [`ManifestLighting::default`]
    /// — the engine's old hardcoded look, so they render as they used to.
    #[serde(default)]
    pub lighting: ManifestLighting,
    /// The map's sky dome shells and the low-res-terrain reference. Absent in
    /// pre-2026-08-31 bakes.
    #[serde(default)]
    pub sky: Option<ManifestSky>,
    /// The map's water surface, from `tern -> WATR`. `None` on the 22 ground
    /// maps that ship none. See `bake::water_glb`. Absent in pre-2026-08-31
    /// bakes.
    #[serde(default)]
    pub water: Option<ManifestWater>,
    /// The map's `coll` meshes and `prim` primitives, merged by LOWERCASE
    /// geometry name. APPENDED after `water` so a manifest's existing RON text
    /// is unchanged up to this point; defaulted so a pre-collision manifest
    /// still loads.
    #[serde(default)]
    pub collision: Vec<ManifestCollision>,
    /// `tern -> CUTS -> CUTR`: one entry per cutter, plane order as read. A
    /// point inside a cutter has no terrain under it (the PS2's height query
    /// returns its −300.0 sentinel — analysis/re-terrain-heights.md).
    #[serde(default)]
    pub terrain_cutters: Vec<ManifestCutter>,
    /// The authored AI data: the base world's planning graph (`plan`/`plnp`,
    /// when it has one) plus the `Hint` and `BARR` nodes of every unwrapped
    /// `wrld`. APPENDED
    /// after `terrain_cutters` so a manifest's existing RON text is unchanged
    /// up to this point; defaulted so a pre-nav manifest still loads. The
    /// runtime builds the sim's nav graph from it (`game::layout`).
    #[serde(default)]
    pub nav: Option<ManifestNav>,
    /// Classic-plus tier 2: `(texture name, file)` for every texture the bake
    /// wrote a higher-resolution override for, the file relative to
    /// `textures/` as its base `<name>.ktx2` is — e.g.
    /// `("sand", "sand.tier2.ktx2")`. Both files are always present together
    /// and draw the same silhouette (`bake::tier`); which one the renderer
    /// binds is the maintainer's runtime toggle (PLAN §Classic-plus assets), and the sim
    /// reads neither. APPENDED after `nav` so a manifest's existing RON text
    /// is unchanged up to this point; defaulted so a single-tier bake still
    /// loads.
    #[serde(default)]
    pub texture_tier2: Vec<(String, String)>,
    /// `lowres_<terrain>.glb`, the far-LOD terrain sheet the map's `tern`
    /// ships — what both PS2s draw past the near scene (`bake::lowres_glb`,
    /// analysis/re-lowr.md). `None` on the 16 terrains that carry no `LOWR`
    /// and on every model-built map.
    ///
    /// This says the sheet EXISTS, not that it is drawn: the draw is gated on
    /// `sky.low_res_terrain` being authored, which BF2 DAG1 and NAB2 leave
    /// out although they ship a sheet (re-lowr.md §0.4). APPENDED after
    /// `texture_tier2` so a manifest's existing RON text is unchanged up to
    /// this point; defaulted so a pre-sheet bake still loads.
    #[serde(default)]
    pub lowres_glb: Option<String>,
    /// The terrain foliage the map's `tern -> FOLG` and `prp_` place
    /// (`bake::foliage`, analysis/re-foliage.md). `None` on a map whose FOLG
    /// places nothing and whose `prp_` has no path line. APPENDED after
    /// `lowres_glb`; defaulted so a pre-foliage bake still loads.
    #[serde(default)]
    pub foliage: Option<ManifestFoliage>,
    /// `terrain_layers_<terrain>.glb`: the terrain's texture layers above
    /// LTEX[0], one primitive per layer with the `ALPH` "over" alpha in the
    /// vertex colour, drawn blended over `terrain_glb` in ascending order
    /// (`bake::terrain_glb`, analysis/re-terrain-layers.md, card TALPH).
    /// `None` where no patch paints a second layer and on every model-built
    /// map. APPENDED after `foliage`; defaulted so a pre-TALPH bake still
    /// loads (and draws LTEX[0] alone, as it did).
    #[serde(default)]
    pub terrain_layers_glb: Option<String>,
    /// Every `leafpatch` class the map ships (card LEAF,
    /// `analysis/re-leafpatch.md` §3): what a class's [`ManifestAttach`], a
    /// world instance and a FOLG `LeafPatch` mesh draw. APPENDED after
    /// `terrain_layers_glb` and written only when non-empty, so a map without
    /// leaves bakes byte-identical to before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leaf: Vec<ManifestLeafClass>,
    /// BF2's `Effect("HDR")` when the map ENABLES it (card GLOW,
    /// `analysis/re-glow.md` §2.1 [O]): the 13 BF2 maps whose self-lit
    /// segments the bake wrote as `glow` / `unscaled` (`bake::glb`) and
    /// whose bright pixels bloom. `None` on every BF1 map and on BF2 DAG1,
    /// END1, TAT2, YAV1. APPENDED after `leaf` and written only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr: Option<ManifestHdr>,
    /// BF2's authored local lights (card BPNT, `analysis/re-point-lights.md`
    /// §1 [O]): every omni and spot `Light` of every non-stub `lght` chunk,
    /// in chunk order. The directional ones stay in [`Manifest::lighting`].
    /// Empty on every BF1 map and on a BF2 map that authors none. APPENDED
    /// after `hdr` and written only when non-empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub point_lights: Vec<ManifestPointLight>,
    /// `Effect("Lightning")` when the map ENABLES it (card KSKY,
    /// `analysis/re-kamino.md` §1 [O]): BF1 KAM1, BF1 YAV2 and BF2 KAM1, whose
    /// sky dome the PS2 darkens between flashes. `None` on the other 29 maps.
    /// APPENDED after `point_lights` and written only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lightning: Option<ManifestLightning>,
    /// `Effect("Precipitation")` when the map ENABLES it (card WTHR,
    /// `analysis/re-weather.md` §1 [O]): nine maps. Its rain shadows are the
    /// [`Manifest::regions`] whose name begins with `rainshadow`. APPENDED
    /// after `lightning` and written only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precipitation: Option<ManifestPrecipitation>,
    /// `Effect("Wind")` when the map enables it (both Kaminos, both Hoths):
    /// only the precipitation reads it. APPENDED, written only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wind: Option<ManifestWind>,
}

/// One world's authored AI data, from `mungefs::plan` and `mungefs::world`.
///
/// Present whenever the world authors ANY of the three — a planning graph, a
/// `Hint` or a `BARR` — because the three are authored independently: seven
/// BF2 maps keep hints in a `lvl_` mode layer that ships no `plan` of its own
/// (HOT1 77, MUS1 73, KAM1 34, MYG1 6, GEO1 4, FEL1 3, TAN1 2), and folding
/// those into the plan would lose them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestNav {
    /// The world's own planning graph: the last top-level `plan` for the base
    /// world, a mode layer's own `plan` for a layer. `None` on a world that
    /// ships none — at runtime such a layer falls back to the base graph, and
    /// no route sidecar is written for it.
    #[serde(default)]
    pub plan: Option<ManifestNavPlan>,
    #[serde(default)]
    pub hints: Vec<ManifestHint>,
    #[serde(default)]
    pub barriers: Vec<ManifestBarrier>,
}

/// One world's planning graph.
///
/// The big tables do not go in the RON — a single map's would be megabytes of
/// text — so they live in two binary sidecars under `<map>/nav/` whose paths
/// `routes` and `ManifestPlnp::records` name. Both are written by `bake::map`
/// and documented there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestNavPlan {
    pub hubs: Vec<ManifestNavHub>,
    pub arcs: Vec<ManifestNavArc>,
    /// `nav/routes_<layer>.bin`, the hub route tables.
    pub routes: String,
    /// The `plnp` patches that follow this world's `plan`, when it ships any
    /// (BF2 only, 20 chunks over 11 ground maps).
    #[serde(default)]
    pub plnp: Option<ManifestPlnp>,
}

/// One planning hub. `pos` is the world frame AS STORED (hub y is the
/// editor's pick height, not a floor — RE_LOG).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestNavHub {
    pub name: String,
    pub pos: [f32; 3],
    pub radius: f32,
    /// The hub's arc slots in order, empty (0xff) slots dropped. Slot k of the
    /// route tables indexes THIS list.
    pub arcs: Vec<u16>,
}

/// One arc between two hubs, walked in both directions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestNavArc {
    pub name: String,
    pub a: u16,
    pub b: u16,
    /// `AISizeType` mask: bit0 SOLDIER, 1 HOVER, 2 SMALL, 3 MEDIUM, 4 HUGE,
    /// 5 FLYER.
    pub type_mask: u32,
    /// BF2 only; 0 on BF1 arcs, whose record has no such field.
    pub flags: u32,
}

/// The `plnp` patch set: `groups` are the fnv1a hashes of the arc names each
/// blockable group holds, and `records` names the sidecar holding the
/// `1 << groups.len()` inflated XOR patches, indexed by the blocked-group
/// bitmask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPlnp {
    pub groups: Vec<Vec<u32>>,
    /// `nav/plnp_<layer>.bin`.
    pub records: String,
}

/// One `Hint` node. `xform` is the 3x3 rotation rows then the translation, AS
/// STORED — a hint's translation is NOT z-negated (unlike a barrier's).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestHint {
    pub name: String,
    /// 0 Snipe, 1 Patrol, 2 Cover, 4 JetJump, 5 Minefield, 6 Land, 7/8
    /// BF2-only. Neither game handles 3 and no disc authors it.
    pub kind: u8,
    pub xform: [[f32; 3]; 4],
    /// The five PROP keys both games resolve, as AUTHORED (the loaders' own
    /// masking and folding is consumer behaviour — see `mungefs::world`).
    #[serde(default)]
    pub primary_stance: Option<u8>,
    #[serde(default)]
    pub secondary_stance: Option<u8>,
    #[serde(default)]
    pub mode: Option<u8>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub command_post: Option<String>,
    /// Every PROP as stored: (fnv1a key hash, value string).
    #[serde(default)]
    pub props: Vec<(u32, String)>,
}

/// One `BARR` barrier: a 2-D oriented rectangle in XZ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestBarrier {
    pub name: String,
    /// The loader's WORLD position: the stored translation with z NEGATED
    /// (BF1 `FUN_00177828` / BF2 `FUN_00178ae8`, and the disc agrees).
    pub pos: [f32; 3],
    /// The 3x3 rotation rows as stored.
    pub rows: [[f32; 3]; 3],
    pub size: [f32; 3],
    /// The `AISizeType` mask this barrier stops.
    pub flags: u32,
}

/// One terrain cutter: its `CUTR INFO` bounding sphere `(r, cx, cy, cz)` and
/// its planes (`nx, ny, nz, d`; inside = every plane strictly < 0). See
/// `mungefs::terrain::Cutter`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestCutter {
    /// All zeroes on a manifest baked before the sphere was decoded — read as
    /// "no bound" by `core::collision::is_cut_by`.
    #[serde(default)]
    pub sphere: [f32; 4],
    pub planes: Vec<[f32; 4]>,
}

/// One geometry's collision: the `coll` mesh (baked to
/// `collision/<geometry>.glb`) and/or its `prim` primitives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestCollision {
    /// Geometry name as authored (matched case-insensitively at runtime).
    pub geometry: String,
    /// The `coll` chunk's NODE name (informational). Defaulted for bakes made
    /// before it was recorded.
    #[serde(default)]
    pub node: String,
    /// `collision/<geometry>.glb`, when this geometry ships a `coll` mesh.
    pub glb: Option<String>,
    /// Defaulted for bakes made before primitives were read.
    #[serde(default)]
    pub prims: Vec<ManifestPrim>,
}

/// One collision primitive in its model's space. `dims` is (radius, _, _) for a
/// sphere, (radius, FULL height, _) for a cylinder and the HALF extents for a
/// box — see `mungefs::coll` for how those conventions were measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPrim {
    pub name: String,
    /// Defaulted for bakes made before the parent node was recorded.
    #[serde(default)]
    pub parent: String,
    /// "sphere" | "cylinder" | "box".
    pub kind: String,
    pub dims: [f32; 3],
    pub rotation: [[f32; 3]; 3],
    pub translation: [f32; 3],
    /// BF2's per-primitive layer mask as the ENGINE sees it — 0 = no MASK
    /// read = all five subsets; bits s 0x01, v 0x02, b 0x04, t 0x08, o 0x10
    /// (`mungefs::coll::Primitive::mask`, which also carries the loader's
    /// stale-byte quirk). Always 0 on BF1. Defaulted for bakes made before
    /// the byte was read.
    #[serde(default)]
    pub mask: u8,
    /// True iff this primitive authored its own `MASK` chunk rather than
    /// inheriting the previous one's. Informational (census only).
    #[serde(default)]
    pub mask_own: bool,
}

/// Shared-asset bake (BF1 COMMON/ENG/COMMON.LVL, BF2 INGAME.LVL): the com_*
/// classes + models + textures every map references (command posts, recharge
/// droids, turrets...). No world; the game merges it under each map's scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommonManifest {
    pub source: String,
    pub models: Vec<String>,
    pub textures: Vec<String>,
    pub classes: Vec<ManifestClass>,
    pub geometries: Vec<ManifestGeometry>,
    /// Collision for the shared props maps instance (command posts, crates,
    /// turrets): merged under the map's own by `Scene::merge_common`.
    /// Defaulted so a pre-collision common bake still loads.
    #[serde(default)]
    pub collision: Vec<ManifestCollision>,
    /// Classic-plus tier 2, as [`Manifest::texture_tier2`]: the common bake
    /// writes overrides too (BF2's INGAME.LVL against the PC's `ingame.lvl`).
    /// No `texture_tiers` twin here — which SOURCE filled the tier has never
    /// been recorded for the common bake. APPENDED after `collision`;
    /// defaulted so a single-tier common bake still loads.
    #[serde(default)]
    pub texture_tier2: Vec<(String, String)>,
}

/// One SIDE LVL's vehicles (`bake vehicle`, card VBAKE;
/// `analysis/re-vehicles.md` §1, §6, §10 item 3): every class whose BASE is a
/// vehicle label, its models (every LOD), its destruction chunks' models and
/// its collision, written to `side_<game>/vehicles/<side>/vehicles.ron` beside
/// `models/`, `collision/` and `textures/`. No world; `game::viewer::vehicles`
/// draws from it and `game::vehicle_pads` spawns from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleManifest {
    /// The side LVL's stem, lowercased (`imp`, `rep`, …).
    pub source: String,
    pub vehicles: Vec<ManifestVehicle>,
    /// The gmod rows of every geometry a vehicle or chunk names.
    pub geometries: Vec<ManifestGeometry>,
    /// Every `models/<name>.glb` written, in bake order.
    pub models: Vec<String>,
    /// Every `textures/<name>.ktx2` written (lowercase).
    pub textures: Vec<String>,
    /// The `coll` meshes and `prim` bodies of those geometries.
    pub collision: Vec<ManifestCollision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub droidekas: Vec<ManifestDroideka>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDroideka {
    pub class: String,
    pub geometry: String,
    pub bank: String,
    pub clips: Vec<Option<ManifestWalkerClip>>,
    pub skin: Option<String>,
    pub prims: Vec<ManifestPrimPoint>,
}

/// One vehicle class of a [`VehicleManifest`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestVehicle {
    /// Name, BASE, GeometryName and collision-layer lists, as a map's class.
    pub class: ManifestClass,
    /// The `ChunkGeometryName` of each `CHUNKSECTION`, in ODF order
    /// (re-vehicles §6: up to 9) — keys into `geometries`.
    pub chunks: Vec<String>,
    /// The geometry's `modl SPHR` sphere `[x, y, z, r]` in model space (card
    /// VDTH, `analysis/re-vehicle-death.md` §3): the centre a blast measures
    /// the class from, and the floor of its radius. `None` (not written)
    /// when the model has no SPHR, or on a bake from before VDTH.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sphere: Option<[f32; 4]>,
    /// The min y of the geometry's top LOD `modl INFO` vis box, in model
    /// space (card VREST, `analysis/re-vehicle-rest.md` §2): a flyer class's
    /// `LandedHeight` is minus this unless the ODF authors one after its
    /// `GeometryName`. `None` (not written) on a bake from before VREST.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vis_min_y: Option<f32>,
    /// A walker class's anim table and prim origins (card WALK); `None`
    /// (not written) for every other class, or a bake from before WALK.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walker: Option<ManifestWalker>,
}

/// What a walker's integrator reads from its bake (card WALK,
/// `analysis/re-walker.md` §5, §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestWalker {
    /// The class's `AnimationName` bank.
    pub bank: String,
    /// The anim table's 11 fixed entries (FUN_002663e0), by entry; `None`
    /// where the bank lacks the entry's clip.
    pub clips: Vec<Option<ManifestWalkerClip>>,
    /// Every collision primitive of the class's geometry at the bind pose:
    /// the leg pairs' foot prims and the `BuildingCollision` spheres.
    pub prims: Vec<ManifestPrimPoint>,
    /// Card WSKN: the drawn walker's skeleton and clips,
    /// `walkers/<skin>.glb` beside `vehicles.ron` — the geometry's LOD0
    /// model, whose GLB carries each vertex's bone as `JOINTS_0`; `None`
    /// when the bake found no skeleton.
    #[serde(default)]
    pub skin: Option<String>,
    /// Card WSKN: each `WalkerLegPair`'s two layer bones (the class entry's
    /// +0x28 / +0x30 masks, props 0x580f4d75 / 0x9eb9fc8e, re-walker §6):
    /// the `_leftup` / `_rightup` layers move these bones' subtrees.
    #[serde(default)]
    pub layer_bones: Vec<[String; 2]>,
}

/// One anim-table entry's clip (FUN_002a0c78, BF2's clock).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestWalkerClip {
    /// The bare clip name the bank was searched for.
    pub name: String,
    pub frames: u16,
    /// (frames − 1)/30 s.
    pub duration: f32,
    pub looping: bool,
    /// Track 0's root translation z at each frame (metres, before
    /// SkeletonRootScale).
    pub root_z: Vec<f32>,
}

/// A collision primitive's origin in model space at the skeleton's bind
/// pose, its parent bone and its bounding radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPrimPoint {
    pub name: String,
    pub bone: String,
    pub pos: [f32; 3],
    pub radius: f32,
}

/// Just the identity fields of a manifest (cheap map discovery for the
/// shell): a `Manifest` deserialized through this reads only `map` and
/// `source_game` and ignores the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestHead {
    pub map: String,
    #[serde(default = "default_game")]
    pub source_game: String,
}

// ---------------------------------------------------------------------------
// `<map>/sounds/sounds.ron` — the per-map sound bank index.
// `bake::sounds` writes it, `game::sounds` reads it back. Same two rules as
// the manifest above: declaration order IS the wire format, and every
// `#[serde(default)]` keeps an older bake readable.
// ---------------------------------------------------------------------------

/// One `snd_` property script: the definition the game looks up by the
/// fnv1a_lowercase hash of a class-prop string (FireSound =
/// "all_weap_inf_rifle_fire").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundProp {
    pub name_hash: u32,
    pub kind: u32,
    pub gain: f32,
    pub pitch: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
    pub priority: f32,
    pub looping: bool,
    /// (sample id hash, weight) — samples resolved through the bank index.
    pub samples: Vec<(u32, f32)>,
    /// Layered definitions: the sound property each `Layer` entry starts
    /// (the entries the PS2's parser keeps, analysis/re-weapon-sounds.md §1).
    pub layers: Vec<u32>,
    /// `soundstreamproperties`: the stream this plays from. The audio is NOT
    /// in this map's banks — it is in GLOBAL/CW/GCW.LVL, baked once by
    /// `bake global-sound` (analysis/re-sound-global.md §1.1/§4: about 45 %
    /// of the names a BF2 map resolves are streamed).
    ///
    /// The runtime carries this for the record only — a segment is resolved
    /// through the global index's `files`, NEVER by joining this hash to the
    /// segment id: 172 BF1 (58 BF2) ids appear in more than one stream and
    /// are baked once, under whichever stream came first
    /// (`engine/artifacts/global_sound/README.md`).
    ///
    /// Defaulted: keeps a pre-item-15 bake readable.
    #[serde(default)]
    pub stream: Option<u32>,
    /// (segment id hash, weight) into that stream — the key the OGGs under
    /// `audio/global_bf{1,2}/` are named by. Defaulted with `stream`.
    #[serde(default)]
    pub segments: Vec<(u32, f32)>,
    /// `mutedistance` where it is NOT `max_distance`: silent from here out
    /// (analysis/re-sound-distance.md). The three fields below are written
    /// only when set, so a property without them reads as before card SNDT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute_distance: Option<f32>,
    /// `rollindistance` when above 0: quieter the nearer inside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll_in_distance: Option<f32>,
    /// `mode3d` 0 through the template chain: a 2-D sound, played at its
    /// gain at any distance.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flat: bool,
    /// Per `layers` entry: (delay s, deviation s) - the layer starts at
    /// delay + deviation x U(-1, 1) after the record (card WSND). Empty when
    /// every layer starts at once; else as long as `layers`. Like the fields
    /// below, written only when set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_delays: Vec<(f32, f32)>,
    /// `PitchDev` / `GainDev`: each start draws pitch (gain) +- this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_dev: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_dev: Option<f32>,
    /// `PlayInterval`, `PlayIntervalDev` (s): no start for interval +- dev
    /// after one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_interval: Option<(f32, f32)>,
    /// `PlayProbability` below 1: the chance a start is taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_probability: Option<f32>,
    /// The sub-level that defines it, by name hash (`kas2gcw`; card SNDE):
    /// the PS2 loads the one its mission names, and the same name has
    /// another era's stream and takes in the next one. 0 outside a
    /// sub-level (CORE.LVL) and in a bake made before the card.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub level: u32,
    /// The mixer bus it plays on, by name hash through the template chain
    /// (`soundfx`, `voiceover`, `ambience`, `battlechatter`, ...; card
    /// MIXH): the PS2 scales a voice by its bus's final gain
    /// (analysis/re-mix-headroom.md 1.4). 0 = none named, and in a bake
    /// made before the card.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bus: u32,
    /// Per take of the list it plays (`samples` when it has any, else
    /// `segments`): the take's own (PlayInterval, PlayIntervalDev) - a
    /// drawn take is locked that long (B1 `FUN_0046b290`; card UCALL).
    /// Empty when no take is locked, and in a bake made before the card.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub take_locks: Vec<(f32, f32)>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundsRon {
    pub source: String,
    /// (bank name hash, sample count)
    pub banks: Vec<(u32, usize)>,
    /// sample id hash → relative WAV path under `sounds/`.
    pub samples: Vec<(u32, String)>,
    pub props: Vec<SoundProp>,
    /// Samples whose `frequency` field is outside 4–48 kHz and so cannot be a
    /// playback rate (~1.8 % of every disc bank; values cluster at 322–346 and
    /// 3000–3047). The record parse is NOT slipping on them — `frames ==
    /// size/16*28` holds for every record — so the disc really stores those
    /// values and what the PS2 does with them is unknown. Written as-is; see
    /// card SNDP.
    ///
    /// Defaulted: keeps a bake made before the census readable.
    #[serde(default)]
    pub odd_frequency: usize,
    /// Properties that play from a stream instead of a bank sample.
    /// Defaulted with `odd_frequency`.
    #[serde(default)]
    pub streamed: usize,
    /// The `soundparameterized` records (a vehicle's `EngineSound`),
    /// resolved (card VSND, `analysis/re-vehicle-sounds.md` §1-2). One per
    /// record per sub-level, in file order; written only when the LVL has
    /// one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines: Vec<EngineSound>,
    /// The LVL's `Music` records (card AMBM, `analysis/re-ambience-music.md`
    /// 3.7), one per name in file order; written only when the LVL has one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub music: Vec<MusicRon>,
}

/// A `Music` record: the stream property it plays (a `props` entry with
/// `stream` and `segments`) and the times of its state machine, in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicRon {
    pub name_hash: u32,
    /// `SoundStream`: the name hash of the stream property.
    pub stream: u32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub min_playback: f32,
    pub min_inactive: f32,
    /// BF2's `MaxPlaybackTime`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_playback: Option<f32>,
    pub priority: f32,
}

/// A `soundparameterized` record: the looping layers of its layered record,
/// in that record's order, each with the graphs the record gives the layer
/// of its name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineSound {
    pub name_hash: u32,
    /// The sub-level that defines it (`nab1cw`: a battle loads `<map><era>`,
    /// and the same name's graphs differ between sub-levels); 0 outside one.
    pub level: u32,
    pub layers: Vec<EngineLayer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineLayer {
    /// The sound property the layer loops.
    pub sound: u32,
    /// The layer starts `delay` +- `deviation` s after the record.
    pub delay: f32,
    pub deviation: f32,
    pub graphs: Vec<EngineGraph>,
}

/// `ParameterGraph(parameter, graph, target)` with the graph's points:
/// linear between them, flat outside.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineGraph {
    /// `speed`, `acceleration`, `health`, `turnontime`, `turnofftime`...
    /// (the engine component's table, B1 0x547e00); 8 hex digits for a
    /// hash that is none of them.
    pub parameter: String,
    /// `Gain`, `Pitch`... (the property field the graph multiplies).
    pub target: String,
    pub points: Vec<(f32, f32)>,
}

// ---------------------------------------------------------------------------
// `<map>/sounds/foley.ron` — the soldier foley table (card FOLY,
// `analysis/re-foley.md` §1). `bake::foley` writes it, `game::foley` reads it.
// ---------------------------------------------------------------------------

/// One `FoleyFXSoldier` record: a soldier family's sound properties on one
/// surface, by name hash; 0 = the record has no such slot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoleyRow {
    /// The hash of a soldier class's `FoleyFXClass` value.
    pub class: u32,
    pub walk_left: u32,
    pub walk_right: u32,
    pub run_left: u32,
    pub run_right: u32,
    pub jump: u32,
    pub land: u32,
    pub roll: u32,
    pub prone: u32,
    pub squat: u32,
    pub stand: u32,
    pub body_fall: u32,
}

/// One `FoleyFXGroup`: a surface (`terrain_foley`, `metal_foley`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoleyGroup {
    pub name: u32,
    pub rows: Vec<FoleyRow>,
}

/// One sub-level `<map><era>` (`nab1cw`) of the SOUND LVL.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoleyLevel {
    pub name: u32,
    pub groups: Vec<FoleyGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoleyRon {
    pub levels: Vec<FoleyLevel>,
}

// ---------------------------------------------------------------------------
// `<map>/load/load.ron` — the loading-screen sequence.
// `bake::loadscreen` writes it, `game::assets::load_loadscreen` reads it.
// ---------------------------------------------------------------------------

/// How the game presents the textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LoadStyle {
    /// BF1: the `load_*_0..3` zoom steps play in order (~1 s each).
    #[default]
    Sequence,
    /// BF2: `<planet>_1..N` stills — the game shows one of them for the load
    /// (LOAD/COMMON.LVL's single `loadscreen` is the generic fallback).
    Pool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadScreen {
    /// Texture names in display order (KTX2 files under `load/`).
    pub sequence: Vec<String>,
    /// Extra textures not in the sequence — BF1's planet globe
    /// (`load_planet_tattoine`), which the original composites over the
    /// locale step. DELIBERATELY baked and not yet drawn: the load-screen
    /// layout pass (PLAN §Milestones, M5 polish - the old P10) is where it
    /// lands.
    pub extras: Vec<String>,
    /// Defaulted: keeps a bake made before BF2's still pools readable.
    #[serde(default)]
    pub style: LoadStyle,
}

// ---------------------------------------------------------------------------
// `classes/<game>_<lvl>.ron` — the gameplay classes of one LVL (entc / wpnc /
// ordc / expc). `bake::classes` writes it, `game::classdb` reads it.
// ---------------------------------------------------------------------------

/// `side_<game>/soldiers/<model>.skins.ron` (card USKN,
/// `analysis/re-soldier-skeleton-skins.md` 9): a soldier model's override
/// groups and the override textures its classes name. A class's
/// `OverrideTexture` replaces the textures listed under `override_texture`,
/// its `OverrideTexture2` those under `override_texture2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SoldierSkins {
    /// Group (the segment's `MNAM`) -> the textures of the model's
    /// segments in it, lowercase.
    pub groups: std::collections::BTreeMap<String, Vec<String>>,
    /// Every override texture a class on this model names, lowercase,
    /// baked beside the model's own under `textures/`.
    pub overrides: Vec<String>,
}

/// One munged gameplay class: its name, the chunk it came from, its BASE and
/// its PROP list as stored. Keys stay FNV-1a hashes and values raw strings;
/// the lookups live with their callers (`bake::classes::ClassProps`,
/// `game::classdb::ClassDb`), since this crate is data only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassDef {
    pub name: String,
    /// Source chunk tag: "entc" | "wpnc" | "ordc" | "expc".
    ///
    /// Defaulted: keeps a classes.ron baked before the field existed readable
    /// (the reader's rule since 37a7f1e).
    #[serde(default)]
    pub kind: String,
    /// Parent class (BASE), e.g. "soldier", "hover", "cannon", "bolt".
    /// BF2 chains within a kind (rep_weap_inf_rifle -> com_weap_inf_rifle).
    pub base: String,
    /// Full property list as stored: (fnv1a key hash, raw value string).
    pub props: Vec<(u32, String)>,
}

// ---------------------------------------------------------------------------
// `effects.ron` — one LVL layer's `fx__` particle effects (card FXP1,
// `analysis/re-fx.md` §1–§4). `bake::effects` writes it beside a map's
// `manifest.ron`, a common bake's `common.ron`, and under
// `side_<game>/effects/<side>/`; the runtime (card FXP2) looks an effect up by
// hash map layer first, then the sides, then common. Every field is the
// parser's (`mungefs::particle`, which documents each); the values are the
// PS2's own, bit for bit (its RAM, `mungefs/tests/suite/fx_lvl.rs`).
// ---------------------------------------------------------------------------

/// One layer's effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxSet {
    /// `"bf1"` / `"bf2"`: BF1 reads no `FrameSize` (a fixed 4×4 atlas) and
    /// parses no `Looping`; BF2 reads both (re-fx §6.3, §8).
    pub game: String,
    /// The effects in first-seen order, one per NAME hash.
    pub effects: Vec<FxEffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beams: Vec<FxBeam>,
    /// Top-level `LightningBolt` records. Missing keys remain absent: the
    /// particle emitter defaults do not apply to this separate effect type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lightning: Vec<FxLightning>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FxLightning {
    pub hash: u32,
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
    pub color: Option<[f32; 4]>,
    pub child_color: Option<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxBeam {
    pub hash: u32,
    pub name: String,
    pub color: [u8; 4],
    pub flare_intensity: f32,
    pub length: f32,
    pub initial_radius: [f32; 2],
    pub final_radius: [f32; 2],
    pub fade_length: f32,
    pub fade_factor: f32,
    pub draw_distance: f32,
    pub position: [f32; 3],
    pub direction: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxEffect {
    /// `fnv1a_lowercase(name)` — the hash an ODF value names it by.
    pub hash: u32,
    /// The cracked name, where known (`bake/src/effect_names.txt`).
    pub name: Option<String>,
    /// The root `ParticleEmitter`s.
    pub emitters: Vec<FxEmitter>,
}

/// `(base, span)`: a value drawn as `base + span·rand()`; random iff span ≠ 0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxRange(pub f32, pub f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxBlend {
    Normal,
    Additive,
    Blur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxType {
    Particle,
    Streak,
    Emitter,
    Animated,
    Geometry,
    Billboard,
    Spark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxShape {
    None,
    Spread,
    Vortex,
    Circle,
    Type4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxPositionOp {
    None,
    Accelerate,
    Reach,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxScalarOp {
    None,
    Move,
    Reach,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxColorOp {
    None,
    Move,
    Reach,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxPositionNode {
    pub lifetime: f32,
    pub op: FxPositionOp,
    pub value: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxScalarNode {
    pub lifetime: f32,
    pub op: FxScalarOp,
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxColorNode {
    pub lifetime: f32,
    pub op: FxColorOp,
    pub value: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxSpawner {
    pub shape: FxShape,
    pub shape_ranges: [FxRange; 9],
    pub offset: [FxRange; 3],
    pub position_scale: FxRange,
    pub velocity_scale: FxRange,
    pub inherit_velocity_factor: FxRange,
    pub size: FxRange,
    pub color: [FxRange; 4],
    /// The mode the particles' colour is in (Hue/Saturation/Value when set).
    pub hsv: bool,
    pub fade_in_time: f32,
    pub start_rotation: FxRange,
    pub rotation_velocity: FxRange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxTransformer {
    pub lifetime: f32,
    pub position: Vec<FxPositionNode>,
    pub size: Vec<FxScalarNode>,
    pub rotation: Vec<FxScalarNode>,
    pub color: Vec<FxColorNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxGeometry {
    pub blend: FxBlend,
    pub kind: FxType,
    pub texture: Option<String>,
    pub model: Option<String>,
    pub child: Option<Box<FxEmitter>>,
    pub time_per_frame: f32,
    pub frame_size: f32,
    pub total_frames: f32,
    pub blur_value: f32,
    pub blur_res: i32,
    pub spark_length: f32,
    pub looping: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxEmitter {
    pub label: String,
    pub start_delay: FxRange,
    pub burst_delay: FxRange,
    pub burst_count: FxRange,
    pub max_particles: FxRange,
    pub color: [FxRange; 4],
    /// The emitter-level mode word: never read at runtime (re-fx §5.8).
    pub hsv: bool,
    pub size: FxRange,
    pub spawner: FxSpawner,
    pub transformer: FxTransformer,
    pub geometry: FxGeometry,
    pub min_lod_dist: f32,
    pub max_lod_dist: f32,
    pub bounding_radius: f32,
    pub no_register_step: bool,
    pub world_aligned: bool,
    pub sibling: Option<Box<FxEmitter>>,
    pub sound_name: Option<String>,
}

/// Rigid droid pieces and the source soldiers' bind-pose launch nodes.
/// Written by `bake droid-chunks` beside a side's weapons directory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DroidChunkManifest {
    pub collision: Vec<ManifestCollision>,
    pub nodes: Vec<(String, Vec<ManifestHardpoint>)>,
}

#[cfg(test)]
mod tests;
