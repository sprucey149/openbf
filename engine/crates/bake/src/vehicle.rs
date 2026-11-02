//! Bake a SIDE LVL's vehicles (card VBAKE; `analysis/re-vehicles.md` §1, §6,
//! §10 item 3) to `<outdir>/models/<modl>.glb`, `<outdir>/collision/<coll>.glb`,
//! `<outdir>/textures/<tex>.ktx2` and `<outdir>/vehicles.ron`
//! ([`manifest::VehicleManifest`]).
//!
//! A vehicle is an `entc` whose BASE is one of the ELF's vehicle labels
//! ([`VEHICLE_BASES`], re-vehicles §2). Each one brings its `GeometryName`'s
//! gmod (every LOD: LOD0 and LOWD), the gmod of each `CHUNKSECTION`'s
//! `ChunkGeometryName` (§6), the `coll` mesh and `prim` bodies of those
//! geometries, and the class's collision-layer lists in PROP order. A class
//! whose geometry the LVL does not ship (the BF2 ALL `_ord` missiles, which
//! the side LVL files under a `flyer` BASE; BF2 REP's `rep_fly_jedifighter_sc`)
//! is skipped with a line on stderr.
//!
//! **Every model is written RIGID, in its skeleton's bind pose.** Most side
//! vehicle models are bone-space (the speeder bikes, tanks, walkers, most
//! fighters); their vertices are moved to model space through the model's own
//! `skel` (`mungefs::model::to_model_space`, the soldier bake's step). The
//! rest (the hovernaut, the K-wing, the MTT, the Hailfire, the TIE bomber, the
//! troop transport) keep their body as stored and take each `BNAM` segment
//! through its bone's bind matrix (card VBIND, `crate::bind_pose::pose_bnam`).
//! The collision prims rest at the same bind pose: each `XFRM` is in its
//! `PRNT` bone's frame (card RKHUL, [`crate::bind_pose::prims_at_bind`]).
//! The result goes through the world models' writer and ALPHA RULE v2
//! (`crate::glb::model_to_glb`, `crate::alpha`), which draws any `MTRL` word —
//! including the 0x02 alpha test `rep_walk_atte` carries and the 0x04 + 0x80
//! words BF2's tri-fighter and ARC-170 carry, which the skinned soldier path
//! refuses (`crate::soldier::segment_blends`). A walker's top-LOD model also
//! carries each vertex's bone as `JOINTS_0`, and its skeleton and clips go
//! to `walkers/<model>.glb` (card WSKN, `crate::walker::skin_glb`): the
//! runtime poses it on the CPU and draws it through the same world path, so
//! the disc's alpha test on `rep_walk_atte` stays the world's cutout (ALPHA
//! RULE v2) rather than a skinned pass it would need. Every other vehicle's
//! skeleton is not baked (a turret's aim moves bones: card VTUR's). The textures take the side bake's writer
//! (`crate::soldier::bake_textures_flagged`), told per texture whether a drawn
//! segment was judged MASK or BLEND (design D3's rule for the map bake).

use anyhow::{anyhow, Context, Result};
use manifest::{ManifestCollision, ManifestGeometry, ManifestVehicle, VehicleManifest};
use mungefs::model::Model;
use std::collections::HashMap;
use std::path::Path;

/// The ClassLabels whose constructor builds a vehicle entity (re-vehicles §2:
/// `treaded` IS a hover on both discs). `armedbuilding` and the turrets are
/// seat-only buildings (§8), not baked here.
pub const VEHICLE_BASES: [&str; 8] = [
    "hover",
    "treaded",
    "walker",
    "flyer",
    "carrier",
    "commandhover",
    "commandwalker",
    "commandflyer",
];

/// fnv1a("ChunkGeometryName"): one per `CHUNKSECTION` (re-vehicles §6).
const PROP_CHUNK_GEOMETRY: u32 = 0xDA32_8455;
/// fnv1a("AnimationName"): a walker's clip bank (re-walker §5).
const PROP_ANIMATION_NAME: u32 = 0x9855_7A5E;

/// Is `base` (an entc BASE) a vehicle label?
pub fn is_vehicle_base(base: &str) -> bool {
    VEHICLE_BASES.iter().any(|b| b.eq_ignore_ascii_case(base))
}

/// Everything in the LVL the bake can look up by lowercase name; first
/// occurrence wins, as it does in every other bake.
struct Index {
    classes: Vec<mungefs::entity::EntityClass>,
    gmods: HashMap<String, mungefs::entity::GeomModel>,
    models: HashMap<String, Model>,
    skels: HashMap<String, mungefs::skel::Skeleton>,
    /// Original donor node order: only the donor render mesh's BMAP/BNAM
    /// addresses this table. Collision, hardpoints and physical rigs use skels.
    render_skels: HashMap<String, mungefs::skel::Skeleton>,
    colls: HashMap<String, (mungefs::coll::CollisionMesh, usize)>,
    prims: HashMap<String, (mungefs::coll::Primitives, usize)>,
}

fn index(chunks: &[mungefs::Chunk<'_>]) -> Result<Index> {
    let mut ix = Index {
        classes: Vec::new(),
        gmods: HashMap::new(),
        models: HashMap::new(),
        skels: HashMap::new(),
        render_skels: HashMap::new(),
        colls: HashMap::new(),
        prims: HashMap::new(),
    };
    for c in chunks {
        let at = |e| anyhow!("{} 0x{:X}: {e}", String::from_utf8_lossy(&c.tag), c.offset);
        match &c.tag {
            b"entc" => {
                let e = mungefs::entity::EntityClass::parse(c).map_err(at)?;
                if !ix
                    .classes
                    .iter()
                    .any(|k| k.name.eq_ignore_ascii_case(&e.name))
                {
                    ix.classes.push(e);
                }
            }
            b"gmod" => {
                let g = mungefs::entity::GeomModel::parse(c).map_err(at)?;
                ix.gmods.entry(g.name.to_ascii_lowercase()).or_insert(g);
            }
            b"modl" => {
                let m = Model::parse(c).map_err(at)?;
                ix.models.entry(m.name.to_ascii_lowercase()).or_insert(m);
            }
            b"skel" => {
                let s = mungefs::skel::parse_skel(c).map_err(at)?;
                ix.skels.entry(s.name.to_ascii_lowercase()).or_insert(s);
            }
            b"coll" => {
                let m = mungefs::coll::CollisionMesh::parse(c).map_err(at)?;
                ix.colls
                    .entry(m.name.to_ascii_lowercase())
                    .or_insert((m, c.offset));
            }
            b"prim" => {
                let p = mungefs::coll::Primitives::parse(c).map_err(at)?;
                ix.prims
                    .entry(p.model.to_ascii_lowercase())
                    .or_insert((p, c.offset));
            }
            _ => {}
        }
    }
    Ok(ix)
}

impl Index {
    /// A geometry's LOD rows: its gmod's, else the bare modl of that name
    /// (a chunk may name a modl directly). `None` when the LVL has neither.
    fn lods(&self, geometry: &str) -> Option<Vec<(String, String)>> {
        let key = geometry.to_ascii_lowercase();
        let rows: Vec<(String, String)> = match self.gmods.get(&key) {
            Some(g) => g
                .lods
                .iter()
                .map(|l| (l.tier.clone(), l.model.clone()))
                .collect(),
            None => vec![("LOD0".to_owned(), geometry.to_owned())],
        };
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|(_, m)| self.models.contains_key(&m.to_ascii_lowercase()))
            .collect();
        (!rows.is_empty()).then_some(rows)
    }

    /// A walker geometry's `prim` bodies and skeleton (card WALK): named as
    /// the geometry, else as one of its LOD models.
    fn walker_rig(
        &self,
        geometry: &str,
    ) -> (
        Option<&mungefs::coll::Primitives>,
        Option<&mungefs::skel::Skeleton>,
    ) {
        let mut names = vec![geometry.to_ascii_lowercase()];
        if let Some(lods) = self.lods(geometry) {
            names.extend(lods.into_iter().map(|(_, m)| m.to_ascii_lowercase()));
        }
        let prims = names.iter().find_map(|n| self.prims.get(n)).map(|(p, _)| p);
        let skel = names.iter().find_map(|n| self.skels.get(n));
        (prims, skel)
    }

    /// The model in its skeleton's bind pose — the skeleton named as the
    /// model, else as its geometry: a bone-space model's every vertex through
    /// its bone's bind matrix, and any other model's `BNAM` segments through
    /// theirs (`crate::bind_pose::pose_bnam`, the map bake's rule, PROVISIONAL
    /// [D] — card VBIND: the hovernaut's barrels, the trooptrans's landing
    /// gear and the Hailfire's wheels are stored in their bones' frames). A
    /// model with no skeleton, or bound only to identity bones, is as stored.
    ///
    /// Also each segment's per-vertex skeleton bones (empty for a model
    /// that is not bone-space): a walker's `JOINTS_0` (card WSKN).
    fn model_space(&self, name: &str, geometry: &str) -> Result<(Model, Vec<Vec<usize>>)> {
        let m = &self.models[&name.to_ascii_lowercase()];
        let mut out = m.clone();
        if !m.bone_space {
            if let Some(skel) = self.render_skel_of(name, geometry) {
                if let Some(p) = crate::bind_pose::pose_bnam(m, skel).map_err(|e| anyhow!(e))? {
                    out = p;
                }
            }
            return Ok((out, Vec::new()));
        }
        let skel = self
            .render_skel_of(name, geometry)
            .ok_or_else(|| anyhow!("{name}: bone-space model with no skel"))?;
        let mut bones = Vec::new();
        for seg in out.segments.iter_mut() {
            let (p, n, b) =
                mungefs::model::to_model_space(m, seg, skel).map_err(|e| anyhow!("{name}: {e}"))?;
            seg.positions = p;
            seg.normals = n;
            bones.push(b);
        }
        out.bone_space = false;
        Ok((out, bones))
    }

    /// A geometry's `prim` bodies in model space at the bind pose (card
    /// RKHUL): each through its `PRNT` bone's bind matrix, by the skeleton
    /// named as the prims' model, else as the geometry. Also the count of
    /// undecoded prims dropped.
    fn rest_prims(
        &self,
        p: &mungefs::coll::Primitives,
        geometry: &str,
    ) -> (Vec<manifest::ManifestPrim>, usize) {
        let (mut prims, unknown) = crate::map::manifest_prims(p);
        if let Some(skel) = self.skel_of(&p.model, geometry) {
            crate::bind_pose::prims_at_bind(skel, &mut prims);
        }
        (prims, unknown)
    }

    /// The skeleton a model is posed by: named as the model, else as its
    /// geometry.
    fn skel_of(&self, name: &str, geometry: &str) -> Option<&mungefs::skel::Skeleton> {
        self.skels
            .get(&name.to_ascii_lowercase())
            .or_else(|| self.skels.get(&geometry.to_ascii_lowercase()))
    }

    fn render_skel_of(&self, name: &str, geometry: &str) -> Option<&mungefs::skel::Skeleton> {
        self.render_skels
            .get(&name.to_ascii_lowercase())
            .or_else(|| self.render_skels.get(&geometry.to_ascii_lowercase()))
            .or_else(|| self.skel_of(name, geometry))
    }

    fn use_render_skeletons(&mut self, original: &[u8]) -> Result<()> {
        let inner = mungefs::open(original).map_err(|e| anyhow!("{e}"))?;
        for chunk in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
            if &chunk.tag == b"skel" {
                let skel = mungefs::skel::parse_skel(&chunk).map_err(|e| anyhow!("{e}"))?;
                self.render_skels
                    .entry(skel.name.to_ascii_lowercase())
                    .or_insert(skel);
            }
        }
        Ok(())
    }

    /// A skinned render mesh must address the canonical rig GLB by bone name,
    /// while its stored vertex indices are decoded through the donor rig.
    fn canonical_joints(
        &self,
        name: &str,
        geometry: &str,
        joints: &mut [Vec<usize>],
    ) -> Result<()> {
        if self.render_skels.is_empty() {
            return Ok(());
        }
        let donor = self
            .render_skel_of(name, geometry)
            .context("missing render skeleton")?;
        let canonical = self
            .skel_of(name, geometry)
            .context("missing canonical skeleton")?;
        for segment in joints {
            crate::bind_pose::remap_bone_indices(donor, canonical, segment)
                .map_err(|e| anyhow!(e))?;
        }
        Ok(())
    }
}

/// A geometry's top LOD model (LOD0, else LOD1, LOD2, LOWD, else its first).
fn top_lod(ix: &Index, geometry: &str) -> Option<String> {
    let lods = ix.lods(geometry)?;
    Some(preferred_lod(&lods).to_owned())
}

/// Select from the nonempty rows already filtered by `Index::lods`.
fn preferred_lod(lods: &[(String, String)]) -> &str {
    &["LOD0", "LOD1", "LOD2", "LOWD"]
        .iter()
        .find_map(|t| lods.iter().find(|(tier, _)| tier == t))
        .unwrap_or(&lods[0])
        .1
}

fn animation_name(e: &mungefs::entity::EntityClass) -> String {
    e.props
        .iter()
        .find(|p| p.key_hash == PROP_ANIMATION_NAME)
        .map(|p| p.value.clone())
        .unwrap_or_default()
}

fn bake_skin(
    ix: &Index,
    geometry: &str,
    out: &Path,
    skinned: &mut Vec<String>,
    what: &str,
    glb: impl FnOnce(&mungefs::skel::Skeleton) -> Result<Vec<u8>>,
) -> Result<Option<String>> {
    let top = top_lod(ix, geometry);
    let Some((t, sk)) = top
        .as_deref()
        .and_then(|t| Some((t, ix.skel_of(t, geometry)?)))
        .filter(|(t, _)| ix.models[&t.to_ascii_lowercase()].bone_space)
    else {
        return Ok(None);
    };
    let key = t.to_ascii_lowercase();
    if !skinned.contains(&key) {
        let glb = glb(sk).with_context(|| format!("{what} skin {t}"))?;
        std::fs::write(out.join("walkers").join(format!("{t}.glb")), glb)?;
        skinned.push(key);
    }
    Ok(Some(t.to_owned()))
}

fn bake_droidekas(
    ix: &Index,
    banks: &[mungefs::anim::AnimBank<'_>],
    out: &Path,
    geometries: &mut Vec<ManifestGeometry>,
    skinned: &mut Vec<String>,
) -> Result<Vec<manifest::ManifestDroideka>> {
    let mut droidekas = Vec::new();
    for e in ix
        .classes
        .iter()
        .filter(|e| crate::droideka::is_droideka_base(&e.base))
    {
        let Some(geometry) = e.geometry_name() else {
            eprintln!("droideka {}: no GeometryName — skipped", e.name);
            continue;
        };
        let Some(_) = ix.lods(geometry) else {
            eprintln!(
                "droideka {}: geometry {geometry} not in this LVL — skipped",
                e.name
            );
            continue;
        };
        // Droidekas use the same authored CHUNKSECTION geometry as vehicles.
        for name in std::iter::once(geometry).chain(
            e.props
                .iter()
                .filter(|p| p.key_hash == PROP_CHUNK_GEOMETRY)
                .map(|p| p.value.as_str()),
        ) {
            if geometries.iter().any(|g| g.name.eq_ignore_ascii_case(name)) {
                continue;
            }
            let Some(lods) = ix.lods(name) else {
                eprintln!("droideka {}: chunk geometry {name} not in this LVL", e.name);
                continue;
            };
            geometries.push(ManifestGeometry {
                name: name.to_owned(),
                model: preferred_lod(&lods).to_owned(),
                lods,
                far: None,
                hardpoints: Vec::new(),
            });
        }
        let bank = animation_name(e);
        let (prims, skel) = ix.walker_rig(geometry);
        let b = banks.iter().find(|b| b.name.eq_ignore_ascii_case(&bank));
        let skin = bake_skin(ix, geometry, out, skinned, "droideka", |sk| {
            crate::droideka::skin_glb(sk, b)
        })?;
        droidekas.push(crate::droideka::bake(
            e, geometry, &bank, banks, prims, skel, skin,
        ));
    }
    Ok(droidekas)
}

/// Bake every vehicle class of one SIDE LVL into `outdir`. `pc_lvl` is the
/// PC install's matching side LVL: its larger textures are written beside the
/// PS2 ones as `<name>.tier2.ktx2` (`crate::tier`).
pub fn bake_vehicles(
    lvl_path: &str,
    outdir: &str,
    pc_lvl: Option<&str>,
) -> Result<VehicleManifest> {
    let pc = pc_lvl.map(crate::tier::PcTextures::load).transpose()?;
    let (buf, original) =
        crate::gameplay::read_with_original(lvl_path).with_context(|| lvl_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    let mut ix = index(&chunks)?;
    if let Some(original) = &original {
        ix.use_render_skeletons(original)?;
    }
    let out = Path::new(outdir);
    for d in ["models", "collision", "textures", "walkers"] {
        std::fs::create_dir_all(out.join(d))?;
    }

    // Card WALK: the walkers' clip banks.
    let banks = mungefs::anim::find_banks(inner).unwrap_or_default();

    // Classes first: which geometries, in first-named order.
    let mut vehicles = Vec::new();
    let mut geometries: Vec<ManifestGeometry> = Vec::new();
    // Card WSKN: the models whose skin is written (lowercase).
    let mut skinned: Vec<String> = Vec::new();
    for e in ix.classes.iter().filter(|e| is_vehicle_base(&e.base)) {
        let Some(geometry) = e.geometry_name() else {
            eprintln!("vehicle {}: no GeometryName — skipped", e.name);
            continue;
        };
        if ix.lods(geometry).is_none() {
            eprintln!(
                "vehicle {}: geometry {geometry} not in this LVL — skipped",
                e.name
            );
            continue;
        }
        let chunk_names: Vec<String> = e
            .props
            .iter()
            .filter(|p| p.key_hash == PROP_CHUNK_GEOMETRY)
            .map(|p| p.value.clone())
            .collect();
        for g in std::iter::once(geometry).chain(chunk_names.iter().map(String::as_str)) {
            if geometries.iter().any(|k| k.name.eq_ignore_ascii_case(g)) {
                continue;
            }
            let Some(lods) = ix.lods(g) else {
                eprintln!("vehicle {}: chunk geometry {g} not in this LVL", e.name);
                continue;
            };
            let best = preferred_lod(&lods).to_owned();
            geometries.push(ManifestGeometry {
                name: g.to_owned(),
                model: best,
                lods,
                far: None,
                hardpoints: Vec::new(),
            });
        }
        // The class's model is its geometry's top LOD (as the chunk loop
        // above picks it); its SPHR is what the PS2 measures the class by.
        // Its `INFO` vis box's floor is what a flyer class's LandedHeight is
        // read from (card VREST, re-vehicle-rest §2).
        let top = top_lod(&ix, geometry).map(|top| &ix.models[&top.to_ascii_lowercase()]);
        let sphere = top.and_then(|m| m.sphere);
        let vis_min_y = top.map(|m| m.vis_box.min[1]);
        // Card WALK: a walker's anim table and prim origins; card WSKN: its
        // top LOD's skin (the skeleton and the bank's clips), once per model.
        let walker = if crate::walker::is_walker_base(&e.base) {
            let bank = animation_name(e);
            let (prims, skel) = ix.walker_rig(geometry);
            let b = banks.iter().find(|b| b.name.eq_ignore_ascii_case(&bank));
            let skin = bake_skin(&ix, geometry, out, &mut skinned, "walker", |sk| {
                crate::walker::skin_glb(sk, b, e)
            })?;
            Some(crate::walker::bake(e, &bank, &banks, prims, skel, skin))
        } else {
            None
        };
        vehicles.push(ManifestVehicle {
            class: crate::map::class_from_entc(e),
            chunks: chunk_names,
            sphere,
            vis_min_y,
            walker,
        });
    }

    let droidekas = bake_droidekas(&ix, &banks, out, &mut geometries, &mut skinned)?;

    // Card VTUR: each class's skel nodes (the seats' fire points).
    crate::hardpoints::attach(&chunks, vehicles.iter_mut().map(|v| &mut v.class));
    let skels = crate::hardpoints::Skeletons::of(&chunks);
    for g in geometries.iter_mut().filter(|g| {
        vehicles
            .iter()
            .any(|v| v.chunks.iter().any(|c| c.eq_ignore_ascii_case(&g.name)))
            || ix
                .classes
                .iter()
                .filter(|e| crate::droideka::is_droideka_base(&e.base))
                .any(|e| {
                    e.geometry_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case(&g.name))
                        || e.props.iter().any(|p| {
                            p.key_hash == PROP_CHUNK_GEOMETRY
                                && p.value.eq_ignore_ascii_case(&g.name)
                        })
                })
    }) {
        if let Some(s) = skels.find(&g.name) {
            g.hardpoints = crate::hardpoints::of(s);
        }
    }

    // Models: every LOD of every geometry, once each.
    let mut textures_decoded: Vec<mungefs::texture::Texture> = Vec::new();
    let mut to_write: Vec<Model> = Vec::new();
    let mut joints: Vec<Vec<Vec<usize>>> = Vec::new();
    for g in &geometries {
        for (_, name) in &g.lods {
            if to_write.iter().any(|m| m.name.eq_ignore_ascii_case(name)) {
                continue;
            }
            let (m, mut bones) = ix.model_space(name, &g.name)?;
            if skinned.contains(&m.name.to_ascii_lowercase()) {
                ix.canonical_joints(name, &g.name, &mut bones)?;
            }
            to_write.push(m);
            joints.push(bones);
        }
    }

    // The authored closed volumes, beside the vehicle models they belong to. The
    // Xbox and PC give soldiers and weapons shadow meshes too; only these draw one.
    for c in chunks.iter().filter(|c| &c.tag == b"modl") {
        let vehicle = c
            .child_str(b"NAME")
            .is_some_and(|n| to_write.iter().any(|m| m.name.eq_ignore_ascii_case(n)));
        if vehicle {
            crate::shadow::write_model(c, out, &skels)?;
        }
    }
    let wanted: Vec<String> = to_write
        .iter()
        .flat_map(|m| m.segments.iter())
        .filter_map(crate::alpha::segment_texture)
        .map(str::to_ascii_lowercase)
        .collect();
    for c in chunks.iter().filter(|c| &c.tag == b"tex_") {
        if let Ok(t) = mungefs::texture::Texture::parse(c) {
            let lower = t.name.to_ascii_lowercase();
            if wanted.contains(&lower)
                && !textures_decoded
                    .iter()
                    .any(|k| k.name.eq_ignore_ascii_case(&t.name))
            {
                textures_decoded.push(t);
            }
        }
    }
    let table = crate::alpha::AlphaTable::new(&textures_decoded);
    let mut usage = crate::alpha::AlphaUsage::default();
    let mut models = Vec::new();
    for (m, bones) in to_write.iter().zip(&joints) {
        let alphas = table.model(m);
        usage.record_model(m, &alphas);
        // A walker's skinned model carries its vertices' bones (card WSKN).
        let skin = skinned.contains(&m.name.to_ascii_lowercase());
        let glb =
            crate::glb::model_to_glb_joints(m, &alphas, false, skin.then_some(bones.as_slice()))?;
        std::fs::write(out.join("models").join(format!("{}.glb", m.name)), glb)?;
        models.push(m.name.clone());
    }

    // Textures: alpha kept iff a drawn segment is MASK or BLEND (design D3),
    // or a specular-pass segment draws it — the pass's gloss weight (queue
    // DSHN, `crate::spec::pass_textures`).
    let pass = crate::spec::pass_textures(&to_write);
    let flags: HashMap<String, u32> = wanted
        .iter()
        .map(|t| {
            let keep = usage.needs_alpha(t) || pass.contains(t);
            (
                t.clone(),
                if keep {
                    crate::detail::ALPHA_IS_MEANINGFUL
                } else {
                    0
                },
            )
        })
        .collect();
    let textures = crate::soldier::bake_textures_flagged(inner, &wanted, &flags, out, pc.as_ref())?;

    // Collision: the coll mesh and prim bodies named as a geometry or any of
    // its LOD models, in geometry order. A prim's `XFRM` is in its `PRNT`
    // bone's frame (card RKHUL): it is written in model space at the bind
    // pose, as the drawn model is. [D] BF2's spider walker: through their
    // bones its four foot spheres (r 2.5) sit at y 2.50, on the ground under
    // the drawn feet, and its body sphere 6.19 m up. The stored bone-local centres
    // misplaced the bodies, and rockets passed through the drawn body.
    let mut collision: Vec<ManifestCollision> = Vec::new();
    let mut collision_index = HashMap::new();
    for g in &geometries {
        let names = std::iter::once(g.name.as_str()).chain(g.lods.iter().map(|(_, m)| m.as_str()));
        for n in names {
            let key = n.to_ascii_lowercase();
            if let Some((m, offset)) = ix.colls.get(&key) {
                let slot =
                    crate::map::collision_slot(&m.name, &mut collision, &mut collision_index);
                if collision[slot].glb.is_none() {
                    collision[slot].node = m.node.clone();
                    collision[slot].glb = crate::map::write_coll(m, out, *offset)?;
                }
            }
            if let Some((p, offset)) = ix.prims.get(&key) {
                let slot =
                    crate::map::collision_slot(&p.model, &mut collision, &mut collision_index);
                if collision[slot].prims.is_empty() {
                    let (prims, unknown) = ix.rest_prims(p, &g.name);
                    if unknown > 0 {
                        eprintln!(
                            "prim 0x{offset:X}: {} dropped {unknown} primitive(s) of an undecoded kind",
                            p.model
                        );
                    }
                    collision[slot].prims = prims;
                }
            }
        }
    }

    let manifest = VehicleManifest {
        source: Path::new(lvl_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default(),
        vehicles,
        geometries,
        models,
        textures,
        collision,
        droidekas,
    };
    crate::write_ron(&out.join("vehicles.ron"), &manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donor_vertex_indices_use_original_rig_without_changing_physics() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for relative in [
            "PC_swbf1_exe/Star Wars Battlefront (Classic 2004)/GameData/Data/_LVL_PC/SIDE/cis.lvl",
            "PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/SIDE/cis.lvl",
        ] {
            let path = crate::join_ci(&root, relative);
            if !path.is_file() {
                continue;
            }
            let (corrected, original) = crate::gameplay::read_with_original(&path).unwrap();
            let original = original.expect("the supplied PC source must exercise the donor path");
            let original_chunks =
                mungefs::content_chunks(mungefs::open(&original).unwrap()).unwrap();
            let corrected_chunks =
                mungefs::content_chunks(mungefs::open(&corrected).unwrap()).unwrap();
            let mut canonical = index(&corrected_chunks).unwrap();
            let donor = index(&original_chunks).unwrap();
            let geometry = "cis_tread_hailfire";
            // This is the original defect: BMAP addresses donor nodes that the
            // shorter reference skeleton does not contain.
            assert!(canonical.model_space(geometry, geometry).is_err());
            let physical = canonical
                .rest_prims(&canonical.prims[geometry].0, geometry)
                .0;
            canonical.use_render_skeletons(&original).unwrap();
            let (actual, _) = canonical.model_space(geometry, geometry).unwrap();
            let (expected, _) = donor.model_space(geometry, geometry).unwrap();
            for (a, b) in actual.segments.iter().zip(&expected.segments) {
                assert_eq!(a.positions, b.positions);
                assert_eq!(a.normals, b.normals);
            }
            assert_eq!(
                canonical
                    .rest_prims(&canonical.prims[geometry].0, geometry)
                    .0,
                physical
            );
        }
    }

    #[test]
    #[ignore = "requires native CIS side LVLs; writes a scratch side bake"]
    fn both_native_droidekas_bake_the_authored_death_chunks() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for game in ["bf1", "bf2"] {
            let input = root.join(format!("extracted/{game}/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL"));
            let bytes = std::fs::read(&input).unwrap();
            let root_chunk = mungefs::open(&bytes).unwrap();
            let chunks = mungefs::content_chunks(root_chunk).unwrap();
            let ix = index(&chunks).unwrap();
            let class = ix
                .classes
                .iter()
                .find(|c| c.name == "cis_inf_droideka")
                .unwrap();
            let names: Vec<_> = class
                .props
                .iter()
                .filter(|p| p.key_hash == PROP_CHUNK_GEOMETRY)
                .map(|p| p.value.clone())
                .collect();
            assert!(!names.is_empty());
            let out = root.join(format!("engine/artifacts/lead_tmp/droideka85/{game}"));
            let baked =
                bake_vehicles(input.to_str().unwrap(), out.to_str().unwrap(), None).unwrap();
            for name in &names {
                let g = baked
                    .geometries
                    .iter()
                    .find(|g| g.name.eq_ignore_ascii_case(name))
                    .unwrap_or_else(|| panic!("{game}: missing {name}"));
                assert!(!g.lods.is_empty());
                for (_, model) in &g.lods {
                    let bytes =
                        std::fs::read(out.join("models").join(format!("{model}.glb"))).unwrap();
                    let gltf = gltf::Gltf::from_slice(&bytes).unwrap();
                    assert!(gltf.meshes().next().is_some());
                }
            }
            eprintln!(
                "{game}: {} authored chunk sections baked: {names:?}",
                names.len()
            );
        }
    }

    #[test]
    fn skin_joints_resolve_donor_indices_by_name_into_canonical_rig() {
        use mungefs::skel::{SkelBone, Skeleton};
        let bone = |name: &str| SkelBone {
            name: name.into(),
            crc: mungefs::anim::crc32_lower(name),
            parent: None,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t: [0.0; 3],
        };
        let mut ix = index(&[]).unwrap();
        ix.skels.insert(
            "mesh".into(),
            Skeleton {
                name: "mesh".into(),
                bones: vec![bone("root"), bone("arm")],
            },
        );
        ix.render_skels.insert(
            "mesh".into(),
            Skeleton {
                name: "mesh".into(),
                bones: vec![bone("extra"), bone("Arm"), bone("Root")],
            },
        );
        let mut joints = vec![vec![2, 1, 2]];
        ix.canonical_joints("mesh", "mesh", &mut joints).unwrap();
        assert_eq!(joints, [vec![0, 1, 0]]);
        ix.render_skels.get_mut("mesh").unwrap().bones[0].parent = Some(1);
        let mut detail = vec![vec![0]];
        ix.canonical_joints("mesh", "mesh", &mut detail).unwrap();
        assert_eq!(detail, [vec![1]]);
        ix.render_skels.get_mut("mesh").unwrap().bones[0].parent = None;
        assert!(ix
            .canonical_joints("mesh", "mesh", &mut [vec![0]])
            .unwrap_err()
            .to_string()
            .contains("no canonical node or ancestor"));
    }

    // RKHULT: exact synthetic axes make the two composition orders differ.
    // Expectations below are worked out by following the prim's axes, then
    // the bone's axes; they do not call the matrix helper being tested.
    fn check_rest_prim(bone_space: bool, chunk: bool) {
        use mungefs::coll::{PrimKind, Primitive, Primitives};
        use mungefs::entity::{GeomLod, GeomModel};
        use mungefs::skel::{SkelBone, Skeleton};
        let geom = if chunk { "vehicle_chunk" } else { "vehicle" };
        let model = format!("{geom}_lod0");
        let mut ix = index(&[]).unwrap();
        ix.gmods.insert(
            geom.into(),
            GeomModel {
                name: geom.into(),
                cloths: Vec::new(),
                lods: vec![GeomLod {
                    tier: "LOD0".into(),
                    model: model.clone(),
                    tris: 0,
                }],
            },
        );
        ix.models.insert(
            model.clone(),
            Model {
                name: model,
                bone_space,
                vertex_box: mungefs::model::Aabb {
                    min: [0.0; 3],
                    max: [1.0; 3],
                },
                vis_box: mungefs::model::Aabb {
                    min: [0.0; 3],
                    max: [1.0; 3],
                },
                face_count: 0,
                sphere: None,
                segments: Vec::new(),
            },
        );
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        ix.skels.insert(
            geom.into(),
            Skeleton {
                name: geom.into(),
                bones: vec![
                    SkelBone {
                        name: "root".into(),
                        crc: mungefs::anim::crc32_lower("root"),
                        parent: None,
                        rows: identity,
                        t: [0.0; 3],
                    },
                    SkelBone {
                        name: "bone".into(),
                        crc: mungefs::anim::crc32_lower("bone"),
                        parent: Some(0),
                        rows: [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
                        t: [10.0, 20.0, 30.0],
                    },
                ],
            },
        );
        let make = |parent: &str| Primitive {
            name: format!("p_{parent}"),
            parent: parent.into(),
            rotation: [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
            translation: [1.0, 2.0, 3.0],
            kind: PrimKind::Box {
                half: [2.0, 3.0, 4.0],
            },
            mask: 0x15,
            mask_own: true,
        };
        let stored = Primitives {
            model: geom.into(),
            prims: vec![make("bone"), make("root"), make("missing")],
        };
        let (raw, _) = crate::map::manifest_prims(&stored);
        let (posed, unknown) = ix.rest_prims(&stored, geom);
        assert_eq!(unknown, 0);
        assert_eq!(posed.len(), 3);
        assert_eq!(posed[0].translation, [13.0, 22.0, 29.0]);
        assert_eq!(
            posed[0].rotation,
            [[0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]]
        );
        let mut expected = raw[0].clone();
        expected.translation = posed[0].translation;
        expected.rotation = posed[0].rotation;
        assert_eq!(posed[0], expected); // dimensions, mask and names survive
        assert_eq!(posed[1..], raw[1..]); // identity and unresolved controls
        assert_eq!(crate::map::manifest_prims(&stored).0, raw);
    }

    #[test]
    fn rest_prim_rotation_applies_prim_before_bone() {
        check_rest_prim(true, false);
    }

    #[test]
    fn rest_prim_on_non_bone_space_geometry_uses_bind() {
        check_rest_prim(false, false);
    }

    #[test]
    fn rest_prim_on_chunk_geometry_uses_bind() {
        check_rest_prim(false, true);
    }

    #[test]
    fn filtered_lods_keep_priority_first_matches_and_unknown_fallback() {
        use mungefs::entity::{GeomLod, GeomModel};
        let rows: Vec<_> = [
            ("unknown", "fallback"),
            ("LOWD", "low"),
            ("LOD2", "second"),
            ("LOD1", "first"),
            ("LOD0", "missing"),
            ("LOD1", "duplicate"),
        ]
        .into_iter()
        .map(|(t, m)| (t.to_owned(), m.to_owned()))
        .collect();
        let mut ix = index(&[]).unwrap();
        ix.gmods.insert(
            "geom".into(),
            GeomModel {
                name: "geom".into(),
                cloths: Vec::new(),
                lods: rows
                    .iter()
                    .map(|(tier, model)| GeomLod {
                        tier: tier.clone(),
                        model: model.clone(),
                        tris: 0,
                    })
                    .collect(),
            },
        );
        for (_, name) in rows.iter().filter(|(_, m)| m != "missing") {
            ix.models.insert(
                name.clone(),
                Model {
                    name: name.clone(),
                    bone_space: false,
                    vertex_box: mungefs::model::Aabb {
                        min: [0.0; 3],
                        max: [1.0; 3],
                    },
                    vis_box: mungefs::model::Aabb {
                        min: [0.0; 3],
                        max: [1.0; 3],
                    },
                    face_count: 0,
                    sphere: None,
                    segments: Vec::new(),
                },
            );
        }
        assert_eq!(preferred_lod(&rows), "missing");
        assert_eq!(top_lod(&ix, "geom").as_deref(), Some("first"));
        let filtered = ix.lods("geom").unwrap();
        assert!(!filtered.iter().any(|(_, m)| m == "missing"));
        assert_eq!(preferred_lod(&filtered), "first");
        assert_eq!(
            preferred_lod(&[
                ("other".into(), "fallback".into()),
                ("lod0".into(), "case-sensitive".into())
            ]),
            "fallback"
        );
        assert_eq!(
            preferred_lod(&[("unknown".into(), "fallback".into())]),
            "fallback"
        );
        ix.models.clear();
        assert!(top_lod(&ix, "geom").is_none());
    }

    #[test]
    fn chunk_geometry_hash_is_the_property_name() {
        assert_eq!(
            mungefs::fnv1a_lowercase("ChunkGeometryName"),
            PROP_CHUNK_GEOMETRY
        );
    }

    #[test]
    fn animation_name_hash_is_the_property_name() {
        assert_eq!(
            mungefs::fnv1a_lowercase("AnimationName"),
            PROP_ANIMATION_NAME
        );
    }

    /// One primitive-like range of a posed model's `bone` segment(s).
    fn bounds(m: &Model, bone: &str) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for s in m
            .segments
            .iter()
            .filter(|s| s.bone.as_deref() == Some(bone))
        {
            for p in &s.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        (lo, hi)
    }

    /// `a`'s mirror image across x = 0 equals `b` (rotation and centre).
    fn mirrored(a: &manifest::ManifestPrim, b: &manifest::ManifestPrim, tol: f32) -> bool {
        let s = [-1.0f32, 1.0, 1.0];
        (0..3).all(|i| {
            (a.translation[i] * s[i] - b.translation[i]).abs() < tol
                && (0..3).all(|j| (a.rotation[i][j] * s[i] * s[j] - b.rotation[i][j]).abs() < tol)
        })
    }

    /// The bake's prims for `geom` (the prim chunk named as the geometry).
    fn baked_pair(
        ix: &Index,
        geom: &str,
        l: &str,
        r: &str,
    ) -> [(manifest::ManifestPrim, manifest::ManifestPrim); 2] {
        let (stored, _) = &ix.prims[geom];
        let raw = crate::map::manifest_prims(stored).0;
        let (posed, _) = ix.rest_prims(stored, geom);
        let get =
            |v: &[manifest::ManifestPrim], n: &str| v.iter().find(|p| p.name == n).unwrap().clone();
        [
            (get(&raw, l), get(&raw, r)),
            (get(&posed, l), get(&posed, r)),
        ]
    }

    /// Data-gated (card RKHULT): BF2's MAF (bone-space) stores its right
    /// flap's wing and cannon rotated on `bone_r_flap`. At bind the left and
    /// right prims mirror each other, as the ship does; the stored XFRMs do
    /// not. Composing in the wrong order (`bind · XFRM`) turns the right wing.
    #[test]
    fn the_mafs_flap_prims_mirror_at_bind() {
        let Some(cis) = side("bf2", "CIS.LVL") else {
            eprintln!("SKIP: BF2 CIS.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&cis).unwrap()).unwrap();
        let ix = index(&chunks).unwrap();
        let geom = "cis_fly_maf";
        let top = top_lod(&ix, geom).unwrap();
        assert!(ix.models[&top.to_ascii_lowercase()].bone_space);
        for (l, r) in [("p_wingL", "p_wingR"), ("p_cannonL", "p_cannonR")] {
            let [raw, posed] = baked_pair(&ix, geom, l, r);
            assert_ne!(raw.1.rotation[0], [1.0, 0.0, 0.0], "{r} stored rotation");
            assert!(!mirrored(&raw.0, &raw.1, 1e-3), "{l}/{r} stored");
            assert!(mirrored(&posed.0, &posed.1, 1e-4), "{l}/{r}: {posed:?}");
        }
        let [_, (_, wing)] = baked_pair(&ix, geom, "p_wingL", "p_wingR");
        assert!(
            (wing.rotation[0][1] - 0.4537).abs() < 1e-3,
            "{:?}",
            wing.rotation
        );
        assert!(
            (wing.translation[0] + 3.0729).abs() < 1e-3,
            "{:?}",
            wing.translation
        );
    }

    /// Data-gated (card RKHULT): vehicles that are not bone-space compose
    /// their prims too. BF1's troop transport stores both wing boxes on the
    /// +x side; through their bones they mirror at x ±5.19, z −4.85. BF1's
    /// MTT stores its two cannon boxes at the origin; at bind they sit at
    /// x ±4.4 on the barrels, 11.9 m forward.
    #[test]
    fn a_non_bone_space_vehicles_prims_rest_at_bind() {
        let (Some(imp), Some(cis)) = (side("bf1", "IMP.LVL"), side("bf1", "CIS.LVL")) else {
            eprintln!("SKIP: BF1 SIDE LVLs not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&imp).unwrap()).unwrap();
        let ix = index(&chunks).unwrap();
        let geom = "imp_fly_trooptrans";
        let top = top_lod(&ix, geom).unwrap();
        assert!(!ix.models[&top.to_ascii_lowercase()].bone_space);
        let [raw, posed] = baked_pair(&ix, geom, "p_wing_L", "p_wing_R");
        assert!(!mirrored(&raw.0, &raw.1, 1e-3));
        assert!(mirrored(&posed.0, &posed.1, 1e-4), "{posed:?}");
        let t = posed.0.translation;
        assert!(
            (t[0] - 5.1916).abs() < 1e-3 && (t[2] + 4.8456).abs() < 1e-3,
            "{t:?}"
        );

        let chunks = mungefs::content_chunks(mungefs::open(&cis).unwrap()).unwrap();
        let ix = index(&chunks).unwrap();
        let geom = "cis_hover_mtt";
        let top = top_lod(&ix, geom).unwrap();
        assert!(!ix.models[&top.to_ascii_lowercase()].bone_space);
        let [raw, posed] = baked_pair(&ix, geom, "p_cannon1", "p_cannon2");
        assert!(raw.0.translation[0].abs() < 0.1 && raw.1.translation[0].abs() < 0.1);
        for (p, sign) in [(&posed.0, 1.0f32), (&posed.1, -1.0)] {
            let t = p.translation;
            assert!(
                (t[0] * sign - 4.4).abs() < 0.05 && (t[2] - 11.9).abs() < 0.1,
                "{} {t:?}",
                p.name
            );
        }
    }

    fn side(game: &str, lvl: &str) -> Option<Vec<u8>> {
        let p = format!(
            "{}/../../../extracted/{game}/lvl/DATA/_LVL_PS2/SIDE/{lvl}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(p).ok()
    }

    /// Data-gated (card VBIND): a vehicle that is not bone-space wears its
    /// `BNAM` parts on their bones. BF1's hovernaut front barrel, stored at
    /// the origin along +z, sits at its bone 5.55 m up; the Hailfire's two
    /// wheels, stored flat (thin in y) at the origin, stand upright (thin in
    /// x) 3.6 m either side; the hovernaut's unbound hull stays as stored.
    #[test]
    fn a_model_space_vehicle_wears_its_parts_on_their_bones() {
        let (Some(all), Some(cis)) = (side("bf1", "ALL.LVL"), side("bf1", "CIS.LVL")) else {
            eprintln!("SKIP: BF1 SIDE LVLs not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&all).unwrap()).unwrap();
        let ix = index(&chunks).unwrap();
        let stored = &ix.models["all_hover_hovernaut"];
        let (m, _) = ix
            .model_space("all_hover_hovernaut", "all_hover_hovernaut")
            .unwrap();
        let (lo, hi) = bounds(&m, "turret_front_barrel");
        assert!(lo[1] > 5.3 && hi[1] < 5.8, "barrel y {lo:?}..{hi:?}");
        for (a, b) in stored.segments.iter().zip(&m.segments) {
            if a.bone.is_none() {
                assert_eq!(a.positions, b.positions);
            }
        }

        let chunks = mungefs::content_chunks(mungefs::open(&cis).unwrap()).unwrap();
        let ix = index(&chunks).unwrap();
        let (m, _) = ix
            .model_space("cis_tread_hailfire", "cis_tread_hailfire")
            .unwrap();
        for (bone, sign) in [("wheel_left", 1.0f32), ("wheel_right", -1.0)] {
            let (lo, hi) = bounds(&m, bone);
            let mid = (lo[0] + hi[0]) * 0.5 * sign;
            assert!((2.5..4.5).contains(&mid), "{bone} x {lo:?}..{hi:?}");
            assert!(
                hi[0] - lo[0] < hi[1] - lo[1] && hi[1] - lo[1] > 7.0,
                "{bone} upright {lo:?}..{hi:?}"
            );
        }
    }

    #[test]
    fn treaded_is_a_vehicle_and_a_turret_is_not() {
        assert!(is_vehicle_base("Treaded"));
        assert!(is_vehicle_base("commandflyer"));
        assert!(!is_vehicle_base("armedbuilding"));
        assert!(!is_vehicle_base("prop"));
    }

    /// Data-gated (card RKHUL): BF2's spider walker's collision prims rest
    /// on its drawn model at the bind pose. The four foot spheres (r 2.5)
    /// touch the ground the drawn feet stand on, the body sphere sits 6.2 m
    /// up inside the drawn body, and every prim's centre is inside the drawn
    /// model's box; the stored bone-local centres misplaced those bodies.
    #[test]
    fn a_walkers_prims_rest_on_its_drawn_model() {
        let Some(cis) = side("bf2", "CIS.LVL") else {
            eprintln!("SKIP: BF2 CIS.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&cis).unwrap()).unwrap();
        let ix = index(&chunks).unwrap();
        let geom = "cis_walk_spider";
        let top = top_lod(&ix, geom).unwrap();
        let (m, _) = ix.model_space(&top, geom).unwrap();
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in m.segments.iter().flat_map(|s| &s.positions) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let (stored, _) = &ix.prims[geom];
        let (prims, _) = ix.rest_prims(stored, geom);
        assert_eq!(prims.len(), 17);
        for p in &prims {
            let t = p.translation;
            assert!(
                (0..3).all(|k| t[k] > lo[k] - 0.01 && t[k] < hi[k] + 0.01),
                "{} at {t:?} outside the drawn {lo:?}..{hi:?}",
                p.name
            );
        }
        let at = |n: &str| prims.iter().find(|p| p.name == n).unwrap().translation;
        for foot in ["p_foot1", "p_foot2", "p_foot3", "p_foot4"] {
            let t = at(foot);
            assert!(
                (t[1] - 2.5 - lo[1]).abs() < 0.3,
                "{foot} y {} drawn floor {}",
                t[1],
                lo[1]
            );
            assert!(t[0].abs() > 6.5, "{foot} x {}", t[0]);
        }
        assert!((at("p_main")[1] - 6.19).abs() < 0.05);
        // Control: the stored XFRMs are the ones the hull missed by.
        let raw = crate::map::manifest_prims(stored).0;
        let main = raw.iter().find(|p| p.name == "p_main").unwrap();
        assert!(main.translation[1].abs() < 0.01);
    }
}
