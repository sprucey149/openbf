//! Bake one map LVL to an output directory:
//!   models/<name>.glb, textures/<name>.ktx2, manifest.ron, terrain_<name>.glb,
//!   water.glb (map bake, when the map ships `WATR`),
//!   collision/<geometry>.glb (one `coll` chunk each, bare triangle soup) and
//!   the manifest's `collision` + `terrain_cutters` sections,
//!   nav/routes_<layer>.bin + nav/plnp_<layer>.bin with the manifest's `nav`
//!   sections (one per world that authors a plan, a Hint or a BARR),
//!   lowres_<name>.glb (the `LOWR` far-LOD sheet, when the map ships one —
//!   bake::lowres_glb),
//!   foliage.bin + the manifest's `foliage` (the terrain foliage the map's
//!   `tern -> FOLG` and `prp_` place, and the `prp_` path props —
//!   bake::foliage),
//!   common.ron (the `bake common` variant)
//! Terrain is emitted as terrain_<name>.glb through bake::terrain_glb (heights
//! per analysis/re-terrain-heights.md, textured with LTEX layer 0), and its
//! texture layers above 0 as terrain_layers_<name>.glb (card TALPH); a stub
//! `tern` with no heightfield is skipped.

use anyhow::{anyhow, Result};
use std::path::Path;

mod content;

use content::bake_content;

/// The manifest schema lives in the shared `manifest` crate so the bake and
/// the runtime cannot drift: ONE definition, `Serialize` for this side and
/// `Deserialize` for `game::assets`. Re-exported under their old paths.
///
/// Field order there IS the RON wire format — see that crate.
pub use manifest::{
    CommonManifest, Manifest, ManifestBarrier, ManifestClass, ManifestCollision, ManifestCutter,
    ManifestGeometry, ManifestHint, ManifestInstance, ManifestLayer, ManifestNav, ManifestNavArc,
    ManifestNavHub, ManifestNavPlan, ManifestPath, ManifestPlnp, ManifestPrim, ManifestRegion,
    ManifestWater,
};

/// One world's planning graph, and the two nav sidecars that hold its bulk.
///
/// The route tables are far too big for RON (a single BF1 map's would be
/// megabytes of text), so they go to `nav/routes_<layer>.bin`:
///
/// ```text
/// u16 hubCount; u8 typeCount (5)
/// for t in 0..5, for hub in 0..hubCount:  u8 rows; rows x hubCount bytes
/// ```
///
/// and the inflated `plnp` patches to `nav/plnp_<layer>.bin`:
///
/// ```text
/// u16 recordCount; per record: u32 len; len bytes
/// ```
///
/// `key` is `base` for the base world and the layer's name for a BF2 mode
/// layer. Layer names are NOT unique on the discs (BF2 HOT1 ships two
/// `hoth_objective` layers, NAB2 and YAV1 two `*commondesign` ones — and in
/// each case BOTH carry a plan), so the caller passes a key already made
/// unique; see `unique_nav_key`. A world with NO plan gets no sidecar at all.
fn nav_plan_entry(
    out: &Path,
    key: &str,
    plan: &mungefs::plan::Plan,
    plnp: Option<&mungefs::plan::Plnp>,
) -> Result<ManifestNavPlan> {
    let dir = out.join("nav");
    std::fs::create_dir_all(&dir)?;

    // routes_<key>.bin
    let hub_count = plan.routes.hub_count;
    let mut routes_bin = Vec::new();
    routes_bin.extend((hub_count as u16).to_le_bytes());
    routes_bin.push(5u8);
    for t in 0..5 {
        for h in 0..hub_count {
            let rows = plan.routes.hub_rows(t, h);
            // rowCount is a u8 on the disc (BF2) or at most four unpacked
            // alternatives (BF1), so this never truncates — but say so.
            let n = u8::try_from(rows.len())
                .map_err(|_| anyhow!("nav {key}: hub {h} type {t} has {} rows", rows.len()))?;
            routes_bin.push(n);
            for r in rows {
                routes_bin.extend_from_slice(r);
            }
        }
    }
    let routes_file = format!("routes_{key}.bin");
    std::fs::write(dir.join(&routes_file), routes_bin)?;

    // plnp_<key>.bin
    let plnp = match plnp {
        Some(pp) => {
            let mut bin = Vec::new();
            bin.extend((pp.records.len() as u16).to_le_bytes());
            for r in &pp.records {
                bin.extend((r.len() as u32).to_le_bytes());
                bin.extend_from_slice(r);
            }
            let file = format!("plnp_{key}.bin");
            std::fs::write(dir.join(&file), bin)?;
            Some(ManifestPlnp {
                groups: pp.groups.clone(),
                records: format!("nav/{file}"),
            })
        }
        None => None,
    };

    Ok(ManifestNavPlan {
        hubs: plan
            .hubs
            .iter()
            .map(|h| ManifestNavHub {
                name: h.name.clone(),
                pos: h.pos,
                radius: h.radius,
                // Slots are prefix-packed; 0xff means "no arc here".
                arcs: h
                    .slots
                    .iter()
                    .filter(|&&s| s != 0xff)
                    .map(|&s| u16::from(s))
                    .collect(),
            })
            .collect(),
        arcs: plan
            .arcs
            .iter()
            .map(|a| ManifestNavArc {
                name: a.name.clone(),
                a: a.a,
                b: a.b,
                type_mask: a.type_mask,
                flags: a.flags,
            })
            .collect(),
        routes: format!("nav/{routes_file}"),
        plnp,
    })
}

/// One world's whole nav block: its graph (when it has one) and the `Hint` /
/// `BARR` nodes of the `wrld`s it owns. `None` only when the world authors
/// none of the three — the three are authored independently, and seven BF2
/// maps keep hints in a mode layer that ships no `plan`.
fn nav_block(
    plan: Option<ManifestNavPlan>,
    worlds: &[&mungefs::world::World],
) -> Option<ManifestNav> {
    let hints: Vec<ManifestHint> = worlds
        .iter()
        .flat_map(|w| w.hints.iter())
        .map(|h| ManifestHint {
            name: h.name.clone(),
            kind: h.kind,
            // Rows then translation, AS STORED: a hint's z is NOT flipped.
            xform: [
                h.transform.rotation[0],
                h.transform.rotation[1],
                h.transform.rotation[2],
                h.transform.translation,
            ],
            primary_stance: h.primary_stance,
            secondary_stance: h.secondary_stance,
            mode: h.mode,
            radius: h.radius,
            command_post: h.command_post.clone(),
            props: h
                .props
                .iter()
                .map(|p| (p.key_hash, p.value.clone()))
                .collect(),
        })
        .collect();
    let barriers: Vec<ManifestBarrier> = worlds
        .iter()
        .flat_map(|w| w.barriers.iter())
        .map(|b| ManifestBarrier {
            name: b.name.clone(),
            // The loaders NEGATE z here (BF1 FUN_00177828 / BF2
            // FUN_00178ae8) and the disc only fits that way.
            pos: [
                b.transform.translation[0],
                b.transform.translation[1],
                -b.transform.translation[2],
            ],
            rows: b.transform.rotation,
            size: b.size,
            flags: b.flags,
        })
        .collect();
    if plan.is_none() && hints.is_empty() && barriers.is_empty() {
        return None;
    }
    Some(ManifestNav {
        plan,
        hints,
        barriers,
    })
}

/// A filesystem-safe, unique key for a nav sidecar pair. Layer names repeat on
/// the discs, so a repeat gets `_2`, `_3`, ... in layer order.
fn unique_nav_key(name: &str, used: &mut Vec<String>) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = if safe.is_empty() {
        "layer".to_string()
    } else {
        safe
    };
    let mut key = safe.clone();
    let mut n = 1;
    while used.iter().any(|u| u == &key) {
        n += 1;
        key = format!("{safe}_{n}");
    }
    used.push(key.clone());
    key
}

fn region_entry(r: &mungefs::world::Region) -> ManifestRegion {
    ManifestRegion {
        shape: r.shape.clone(),
        name: r.name.clone(),
        rotation: r.transform.rotation,
        translation: r.transform.translation,
        size: r.size,
        props: r
            .props
            .iter()
            .map(|p| (p.key_hash, p.value.clone()))
            .collect(),
    }
}

fn instance_entry(i: &mungefs::world::Instance) -> ManifestInstance {
    ManifestInstance {
        class: i.class.clone(),
        name: i.name.clone(),
        rotation: i.transform.rotation,
        translation: i.transform.translation,
        props: i
            .props
            .iter()
            .map(|p| (p.key_hash, p.value.clone()))
            .collect(),
    }
}

/// Copy the named `modl`s — and the textures they reference — out of the SIDE
/// LVLs beside this map, into the map's own bake.
///
/// `sky_ SkyObject` names distant capital ships (`imp_fly_destroyer_DOME`,
/// `rep_fly_assault_DOME`, ...) and **their geometry is not in the map's own
/// LVL**. That was VERIFIED rather than assumed: all 17 distinct names the
/// 20 shipped `SkyObject` blocks reference were searched for across every
/// `modl` in every LVL on both discs. On every GROUND map they resolve to a
/// SIDE LVL — `SIDE/ALL.LVL`, `CIS.LVL`, `IMP.LVL`, `REP.LVL` — and only the
/// space maps (SPA2-SPA9, not baked) carry theirs locally. Two names resolve
/// nowhere at all: BF2 COR1 authors an EMPTY geometry, and BF2 KAS2 authors
/// `rcis_fly_gunship_dome`, which exists in no LVL on either disc.
///
/// The SIDE directory is derived from the map's own path
/// (`.../_LVL_PS2/TAT/TAT1.LVL` -> `.../_LVL_PS2/SIDE`) rather than passed
/// in, so no caller has to change. A bake that cannot reach it simply gets
/// no sky objects.
fn pull_side_models(
    lvl_path: &str,
    wanted: &[String],
    out: &Path,
    all_textures: &mut Vec<mungefs::texture::Texture>,
    usage: &mut crate::alpha::AlphaUsage,
) -> Result<Vec<String>> {
    let Some(side_dir) = Path::new(lvl_path)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| crate::join_ci(p, "SIDE"))
        .filter(|p| p.is_dir())
    else {
        eprintln!("sky objects: no SIDE directory beside {lvl_path}");
        return Ok(Vec::new());
    };

    let mut still_wanted: Vec<String> = wanted.to_vec();
    let mut found = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&side_dir)?.flatten().collect();
    entries.sort_by_key(|e| e.path()); // deterministic across filesystems
    for entry in entries {
        if still_wanted.is_empty() {
            break;
        }
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lvl"))
        {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let Ok(inner) = mungefs::open(&bytes) else {
            continue;
        };
        // SIDE LVLs nest their content inside `lvl_` sublevel wrappers, so a
        // top-level `chunks` walk finds ZERO models in them —
        // `content_chunks` is the one that descends (same call
        // `bake::soldier` makes).
        let Ok(side_chunks) = mungefs::content_chunks(inner) else {
            continue;
        };
        // A texture is only worth decoding if one of the models we take
        // actually uses it, so walk models first and textures second.
        let mut taken_here: Vec<mungefs::model::Model> = Vec::new();
        for c in &side_chunks {
            if &c.tag != b"modl" {
                continue;
            }
            let Ok(m) = mungefs::model::Model::parse(c) else {
                continue;
            };
            if still_wanted.iter().any(|w| w.eq_ignore_ascii_case(&m.name)) {
                still_wanted.retain(|w| !w.eq_ignore_ascii_case(&m.name));
                taken_here.push(m);
            }
        }
        if taken_here.is_empty() {
            continue;
        }
        let needed: Vec<String> = taken_here
            .iter()
            .flat_map(|m| m.segments.iter())
            .flat_map(|s| s.textures.iter())
            .map(|(_, t)| t.to_ascii_lowercase())
            .collect();
        for c in &side_chunks {
            if &c.tag != b"tex_" {
                continue;
            }
            if let Ok(t) = mungefs::texture::Texture::parse(c) {
                let lower = t.name.to_ascii_lowercase();
                let wanted_tex =
                    needed.contains(&lower) || needed.iter().any(|n| lower == format!("{n}_dtl"));
                if wanted_tex && !all_textures.iter().any(|e| e.name == t.name) {
                    all_textures.push(t);
                }
            }
        }
        // The verdicts for these models are taken on the textures we just
        // pulled in beside them, so the table is built here, after the push.
        let table = crate::alpha::AlphaTable::new(all_textures);
        for m in taken_here {
            let alphas = table.model(&m);
            usage.record_model(&m, &alphas);
            let glb = crate::glb::model_to_glb(&m, &alphas, false)?;
            std::fs::write(out.join("models").join(format!("{}.glb", m.name)), glb)?;
            found.push(m.name);
        }
    }
    if !still_wanted.is_empty() {
        eprintln!(
            "sky objects: no modl for {still_wanted:?} in {}",
            side_dir.display()
        );
    }
    Ok(found)
}

/// One `lvl_` mode-layer wrapper: its worlds, and the AI graph it carries (a
/// wrapper's `plan`, and the `plnp` that follows it inside the same wrapper).
struct ModeLayer {
    group: Vec<mungefs::world::World>,
    plan: Option<mungefs::plan::Plan>,
    plnp: Option<mungefs::plan::Plnp>,
}

/// One `entc` -> the manifest's class row: the render-geometry link, and the
/// collision-layer properties in PROP ORDER (analysis/re-collision-layers.md
/// §2 — a property replaces its own layer's subset with the bodies named so
/// far, so the order is the wire format, not a set). Both the map bake and
/// `bake_common` go through here.
pub(crate) fn class_from_entc(e: &mungefs::entity::EntityClass) -> ManifestClass {
    ManifestClass {
        effects: Vec::new(),
        name: e.name.clone(),
        base: e.base.clone(),
        geometry: e.geometry_name().map(str::to_owned),
        collision: e.collision_layers(),
        // Filled after the walk by `crate::door::attach`, which needs the
        // model's `skel` and the clip's `zaa_` bank.
        door: None,
        // Filled after the walk by `crate::hardpoints::attach`.
        hardpoints: Vec::new(),
        // Filled after the walk by `crate::leafpatch::attach`.
        attach: Vec::new(),
        build_zone: e
            .props
            .iter()
            .find(|p| p.key_hash == manifest::BUILD_ZONE_PROP)
            .map(|p| p.value.clone()),
        // Filled after the walk by `crate::entity_light::attach`.
        lights: Vec::new(),
        // Filled after the walk by `crate::turret_rig::TurretRigs::attach`.
        turret_rig: None,
    }
}

fn bail_no_world(lvl_path: &str) -> Result<()> {
    Err(anyhow!("no wrld chunk in {lvl_path}"))
}

/// Which bake `bake_content` is doing. It decides these things: whether a
/// `wrld` chunk is mandatory (a map's is; a common bake's is synthesized),
/// whether a texture NO segment names keeps its disc alpha (queue item SM's
/// clause — `crate::alpha::keep_alpha`'s `unnamed` input, scoped to the
/// common bakes), and whether the sheets the LVL's own `fx__
/// Effect("Water")` names keep theirs (queue item WA's clause —
/// `keep_alpha`'s `water_named` input, scoped to the map bakes) — and a
/// fourth: only a map bake writes its world models' per-vertex lighting
/// (`crate::glb::model_to_glb`'s `vertex_light`, queue LITEB).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    /// `COMMON.LVL` / `INGAME.LVL`: the shared models, the HUD sheets.
    Common,
    /// One map LVL.
    Map,
}

pub fn bake_common(lvl_path: &str, outdir: &str, pc_lvl: Option<&str>) -> Result<CommonManifest> {
    let m = bake_content(lvl_path, outdir, Content::Common, pc_lvl)?;
    let cm = CommonManifest {
        source: m.map,
        models: m.models,
        textures: m.textures,
        classes: m.classes,
        geometries: m.geometries,
        collision: m.collision,
        texture_tier2: m.texture_tier2,
    };
    crate::write_ron(&Path::new(outdir).join("common.ron"), &cm)?;
    // BF2's HUD layouts, fonts and extra sheets (card BHUD); a no-op for an
    // LVL with no `hud_` chunk.
    if let Some(h) = crate::hud::bake_hud(lvl_path, outdir)? {
        eprintln!(
            "hud: {} layouts, {} fonts, {} sheets from COMMON/CORE",
            h.layouts.len(),
            h.fonts.len(),
            h.textures.len()
        );
    }
    Ok(cm)
}

/// Bake one map. `pc_lvl` is the matching PC-format LVL, if the local PC
/// install is present: eligible larger textures become optional tier-2
/// siblings beside the PS2 base (see `crate::tier`). `None` bakes PS2-only,
/// as every BF1 map does.
///
/// Beside the manifest it writes [`MAP_CLASSES_FILE`]: the map LVL's own
/// entc/wpnc/ordc/expc PROPS in the side-classes format (`crate::classes`).
/// `ManifestClass` keeps only a class's geometry and collision, and a
/// map-local vehicle (TAT1's `tat_hover_skiff`, BES1's cloud car) has its
/// drive and seat numbers nowhere else (analysis/re-vehicles.md §10 item 2).
/// Always the PS2 LVL's — the PC tier is textures only.
pub fn bake_map(lvl_path: &str, outdir: &str, pc_lvl: Option<&str>) -> Result<Manifest> {
    let m = bake_content(lvl_path, outdir, Content::Map, pc_lvl)?;
    crate::write_ron(&Path::new(outdir).join("manifest.ron"), &m)?;
    let classes = Path::new(outdir).join(MAP_CLASSES_FILE);
    crate::classes::bake_classes(lvl_path, &classes.to_string_lossy())?;
    Ok(m)
}

/// A map's own classes, beside its `manifest.ron` (`game::classdb`'s
/// `ClassDb::load_map` reads it).
pub const MAP_CLASSES_FILE: &str = "classes.ron";

/// Bake everything in an LVL. `kind` says which bake this is: a map's `wrld`
/// chunk is mandatory, a common bake's is synthesized, only a common bake
/// keeps the disc alpha of a texture no segment names (item SM), and only a
/// map bake keeps the disc alpha of the sheets its `Effect("Water")` names
/// (item WA).
/// The `lvl_` mode-layer name contest, written out four times before this
/// (`tex_`, `modl`, `entc`, `gmod`): a layer copy of a name the top level
/// already baked is dropped, counted and announced. `taken` is evaluated by
/// the caller so the lookup still only happens for a layer chunk.
fn layer_loses(
    tag: &'static str,
    offset: usize,
    name: &str,
    taken: bool,
    skips: &mut std::collections::BTreeMap<&'static str, usize>,
) -> bool {
    if !taken {
        return false;
    }
    eprintln!("lvl_ layer {tag} 0x{offset:X}: {name} already baked — first occurrence wins");
    *skips.entry(tag).or_default() += 1;
    true
}

/// The manifest collision row for `name`, created empty on first sight.
/// `coll` (the mesh) and `prim` (the boxes) each fill in their own half and
/// may arrive in either order, so both go through here.
pub(crate) fn collision_slot(
    name: &str,
    collision: &mut Vec<ManifestCollision>,
    index: &mut std::collections::HashMap<String, usize>,
) -> usize {
    *index.entry(name.to_ascii_lowercase()).or_insert_with(|| {
        collision.push(ManifestCollision {
            geometry: name.to_string(),
            node: String::new(),
            glb: None,
            prims: Vec::new(),
        });
        collision.len() - 1
    })
}

/// Write one `coll` mesh as `collision/<name>.glb` (bare triangle soup) and
/// return the manifest's path to it — `None` for a mesh with no polygons.
pub(crate) fn write_coll(
    m: &mungefs::coll::CollisionMesh,
    out: &Path,
    offset: usize,
) -> Result<Option<String>> {
    let tris = m.triangles();
    if tris.is_empty() {
        eprintln!("coll 0x{offset:X}: {} has no polygons", m.name);
        return Ok(None);
    }
    for (leaves, n) in m.repaired() {
        eprintln!(
            "coll 0x{offset:X}: {} leaves {leaves:?} are a signed fan: {n} triangles for the region they sum to",
            m.name
        );
    }
    let file = format!("{}.glb", m.name);
    let glb = crate::glb::triangles_to_glb(&m.name, &m.vertices, &tris)?;
    std::fs::write(out.join("collision").join(&file), glb)?;
    Ok(Some(format!("collision/{file}")))
}

/// A `prim` chunk's bodies as manifest rows, and how many of an undecoded
/// kind were dropped.
pub(crate) fn manifest_prims(p: &mungefs::coll::Primitives) -> (Vec<ManifestPrim>, usize) {
    let mut prims = Vec::new();
    let mut unknown = 0usize;
    for prim in &p.prims {
        let (kind, dims) = match prim.kind {
            mungefs::coll::PrimKind::Sphere { radius } => ("sphere", [radius, 0.0, 0.0]),
            mungefs::coll::PrimKind::Cylinder { radius, height } => {
                ("cylinder", [radius, height, 0.0])
            }
            mungefs::coll::PrimKind::Box { half } => ("box", half),
            mungefs::coll::PrimKind::Unknown(_) => {
                unknown += 1;
                continue;
            }
        };
        prims.push(ManifestPrim {
            name: prim.name.clone(),
            parent: prim.parent.clone(),
            kind: kind.to_owned(),
            dims,
            rotation: prim.rotation,
            translation: prim.translation,
            // BF2's layer mask, already carrying the loader's stale-byte
            // quirk (mungefs::coll::Primitive::mask).
            mask: prim.mask,
            mask_own: prim.mask_own,
        });
    }
    (prims, unknown)
}

/// One `alpha:` census line — sorted, space separated, silent when empty.
/// `sentence` takes the count because each of the four lines puts it in a
/// different place.
fn alpha_report(mut names: Vec<String>, sentence: impl FnOnce(usize) -> String) {
    if names.is_empty() {
        return;
    }
    names.sort();
    eprintln!("alpha: {}: {}", sentence(names.len()), names.join(" "));
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::world::Prop;

    /// BF1 NAB2.LVL entc `nab2_bldg_b_19` @0x3E99B4 (and BF2 NAB2.LVL
    /// @0x3AAC90, identical): four collision lines in this order. The class
    /// row must carry them verbatim, lowercased, in order — soldiers and
    /// bolts see only the mesh, vehicles only the two boxes
    /// (analysis/re-collision-layers.md §0.4).
    #[test]
    fn a_class_carries_its_collision_lines_in_prop_order() {
        let h = mungefs::fnv1a_lowercase;
        let e = mungefs::entity::EntityClass {
            name: "nab2_bldg_b_19".into(),
            base: "prop".into(),
            props: vec![
                Prop {
                    key_hash: h("GeometryName"),
                    value: "nab2_bldg_b_19".into(),
                },
                Prop {
                    key_hash: h("VehicleCollision"),
                    value: "p_cube".into(),
                },
                Prop {
                    key_hash: h("VehicleCollision"),
                    value: "p_cube1".into(),
                },
                Prop {
                    key_hash: h("SoldierCollision"),
                    value: "CollisionMesh".into(),
                },
                Prop {
                    key_hash: h("OrdnanceCollision"),
                    value: "CollisionMesh".into(),
                },
                // Not a collision key: ignored, as every other prop is.
                Prop {
                    key_hash: h("FoleyFXGroup"),
                    value: "stone".into(),
                },
            ],
        };
        let c = class_from_entc(&e);
        assert_eq!(c.name, "nab2_bldg_b_19");
        assert_eq!(c.base, "prop");
        assert_eq!(c.geometry.as_deref(), Some("nab2_bldg_b_19"));
        assert_eq!(
            c.collision,
            vec![
                ("vehicle".to_string(), "p_cube".to_string()),
                ("vehicle".to_string(), "p_cube1".to_string()),
                ("soldier".to_string(), "collisionmesh".to_string()),
                ("ordnance".to_string(), "collisionmesh".to_string()),
            ]
        );
    }

    /// A class with no collision property gets an EMPTY vector, which is the
    /// engine's default: every body in all five subsets (§0.3). KAM1's
    /// `kam_bldg_mushplatform1` is the case that matters — its
    /// `p_collision_floor` IS the platform floor.
    #[test]
    fn a_class_without_collision_props_carries_nothing() {
        let e = mungefs::entity::EntityClass {
            name: "kam_bldg_mushplatform1".into(),
            base: "prop".into(),
            props: vec![Prop {
                key_hash: mungefs::fnv1a_lowercase("GeometryName"),
                value: "kam_bldg_mushplatform1".into(),
            }],
        };
        assert!(class_from_entc(&e).collision.is_empty());
    }

    // ---- design D1: a `lvl_` mode layer's own content is baked -------------
    //
    // The LVLs below are built in memory in the disc's own container format
    // (`[tag][u32 size][payload]`, padded to 4 — analysis/a2-bake-formats.md
    // §1) so the walk can be exercised without a disc: `ucfb { ... lvl_ {
    // <hashed sub-level> { entc, gmod, modl, wrld } } }`, which is the shape
    // BF2 HOT1 ships four of.

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::from(*tag);
        v.extend((payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        while !v.len().is_multiple_of(4) {
            v.push(0);
        }
        v
    }

    fn zstr(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    }

    /// One `modl` with a single one-vertex segment. `seed` goes into the
    /// vertex so two models of the SAME NAME can be told apart by their GLB.
    fn synth_modl(name: &str, seed: u16) -> Vec<u8> {
        let mut segm = chunk(b"INFO", &[1, 0, 0, 0, 1, 0, 0, 0]);
        segm.extend(chunk(b"MTRL", &1u32.to_le_bytes()));
        segm.extend(chunk(b"RTYP", b"Normal\0"));
        segm.extend(chunk(b"TNAM", &{
            let mut t = 0u32.to_le_bytes().to_vec();
            t.extend(zstr("atex"));
            t
        }));
        let mut posi = Vec::new();
        for _ in 0..3 {
            posi.extend(seed.to_le_bytes());
        }
        segm.extend(chunk(b"POSI", &posi));
        segm.extend(chunk(b"STRP", &[0u8; 2]));

        let mut info = vec![0u8; 68];
        // A unit vertex box, so the dequantized position is finite.
        for (i, v) in [-1.0f32, -1.0, -1.0, 1.0, 1.0, 1.0].iter().enumerate() {
            info[12 + i * 4..16 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        let mut p = chunk(b"NAME", &zstr(name));
        p.extend(chunk(b"INFO", &info));
        p.extend(chunk(b"segm", &segm));
        chunk(b"modl", &p)
    }

    fn synth_gmod(name: &str, model: &str) -> Vec<u8> {
        let mut lod = zstr(model);
        while !lod.len().is_multiple_of(4) {
            lod.push(0);
        }
        lod.extend(1.0f32.to_le_bytes());
        let mut p = chunk(b"NAME", &zstr(name));
        p.extend(chunk(b"LOD0", &lod));
        chunk(b"gmod", &p)
    }

    fn synth_entc(name: &str, geometry: &str) -> Vec<u8> {
        let mut prop = mungefs::fnv1a_lowercase("GeometryName")
            .to_le_bytes()
            .to_vec();
        prop.extend(zstr(geometry));
        let mut p = chunk(b"BASE", &zstr("prop"));
        p.extend(chunk(b"TYPE", &zstr(name)));
        p.extend(chunk(b"PROP", &prop));
        chunk(b"entc", &p)
    }

    /// A `wrld` with one instance of `class`, so the world is non-empty.
    fn synth_wrld(name: &str, class: &str) -> Vec<u8> {
        let mut info = chunk(b"TYPE", &zstr(class));
        info.extend(chunk(b"NAME", &zstr("inst0")));
        info.extend(chunk(b"XFRM", &[0u8; 48]));
        let inst = chunk(b"inst", &chunk(b"INFO", &info));
        let mut p = chunk(b"NAME", &zstr(name));
        p.extend(inst);
        chunk(b"wrld", &p)
    }

    /// `lvl_ { <fnv(name)> { body } }` — the wrapper BF2 puts a mode layer in.
    fn synth_lvl_wrapper(name: &str, body: &[u8]) -> Vec<u8> {
        let tag = mungefs::fnv1a_lowercase(name).to_le_bytes();
        chunk(b"lvl_", &chunk(&tag, body))
    }

    fn synth_ucfb(body: &[u8]) -> Vec<u8> {
        let mut v = Vec::from(*b"ucfb");
        v.extend((body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    /// Per-process scratch dir for a synthetic bake — two `cargo test` runs
    /// at once must not bake into the same directory (queue item 3).
    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{tag}-{}", std::process::id()))
    }

    fn bake_synthetic(lvl: &[u8], tag: &str) -> Manifest {
        let out = scratch_dir(tag);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let path = out.join("synth.lvl");
        std::fs::write(&path, lvl).unwrap();
        bake_map(path.to_str().unwrap(), out.to_str().unwrap(), None).expect("bake")
    }

    /// D1: a layer's `entc` / `gmod` / `modl` are baked exactly like top-level
    /// content — BOTH GLBs land, the class and geometry rows exist — while its
    /// `wrld` still arrives as a mode layer and NOT as a base world. Before
    /// this change the walk took only `wrld`/`plan`/`plnp` out of a wrapper,
    /// so `b`'s three chunks were silently dropped and the layer placed an
    /// instance of a class with no row (BF2 HOT1's bridges).
    #[test]
    fn a_mode_layers_own_content_is_baked_like_top_level_content() {
        let mut body = synth_modl("a", 100);
        body.extend(synth_wrld("base", "a_class"));
        let mut layer = synth_entc("b_class", "b");
        layer.extend(synth_gmod("b", "b"));
        layer.extend(synth_modl("b", 200));
        layer.extend(synth_wrld("layer_con", "b_class"));
        body.extend(synth_lvl_wrapper("layer_con", &layer));
        let out = scratch_dir("openbf-bake-test-layer-content");
        let m = bake_synthetic(&synth_ucfb(&body), "openbf-bake-test-layer-content");

        assert_eq!(m.models, vec!["a".to_string(), "b".to_string()]);
        assert!(out.join("models/a.glb").is_file());
        assert!(out.join("models/b.glb").is_file());
        assert_eq!(m.classes.len(), 1);
        assert_eq!(m.classes[0].name, "b_class");
        assert_eq!(m.classes[0].geometry.as_deref(), Some("b"));
        assert_eq!(m.geometries.len(), 1);
        assert_eq!(m.geometries[0].name, "b");
        assert_eq!(m.geometries[0].model, "b");
        // The world inside the wrapper is still a MODE LAYER, not a base world.
        assert_eq!(m.map, "base");
        assert_eq!(m.layers.len(), 1);
        assert_eq!(m.layers[0].name, "layer_con");
        assert_eq!(m.layers[0].instances.len(), 1);
        let _ = std::fs::remove_dir_all(&out);
    }

    /// D2: first wins, by name. A layer `modl` that repeats a top-level name
    /// is skipped, and the GLB on disk is byte-for-byte the one the same LVL
    /// bakes WITHOUT the layer — the property that lets BF1 and TAT2, which
    /// have no layer content to add, come out of this change unchanged.
    #[test]
    fn a_layer_model_never_overwrites_the_top_level_one_of_the_same_name() {
        let mut plain = synth_modl("a", 100);
        plain.extend(synth_wrld("base", "a_class"));
        let m0 = bake_synthetic(&synth_ucfb(&plain), "openbf-bake-test-layer-dup-none");
        let glb0 =
            std::fs::read(scratch_dir("openbf-bake-test-layer-dup-none").join("models/a.glb"))
                .unwrap();

        let mut layer = synth_modl("a", 900); // same name, different geometry
        layer.extend(synth_wrld("layer_con", "a_class"));
        let mut with_layer = plain.clone();
        with_layer.extend(synth_lvl_wrapper("layer_con", &layer));
        let m1 = bake_synthetic(&synth_ucfb(&with_layer), "openbf-bake-test-layer-dup");
        let glb1 =
            std::fs::read(scratch_dir("openbf-bake-test-layer-dup").join("models/a.glb")).unwrap();

        assert_eq!(
            m1.models,
            vec!["a".to_string()],
            "the layer copy is skipped"
        );
        assert_eq!(m0.models, m1.models);
        assert_eq!(glb0, glb1, "the TOP-LEVEL model is the one on disk");
        // ...and the layer itself still arrives.
        assert_eq!(m1.layers.len(), 1);
        let _ = std::fs::remove_dir_all(scratch_dir("openbf-bake-test-layer-dup-none"));
        let _ = std::fs::remove_dir_all(scratch_dir("openbf-bake-test-layer-dup"));
    }

    /// Card VTUR2: a class the map's worlds place but do not define comes
    /// from `SIDE/TUR.LVL` beside the map's directory — its whole sublevel,
    /// model included — while a TUR sublevel nobody places stays out, and a
    /// class the map defines itself keeps the map's geometry.
    #[test]
    fn a_placed_turret_class_is_pulled_from_the_side_tur_lvl() {
        let root = scratch_dir("openbf-bake-test-tur-pull");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("MAP")).unwrap();
        std::fs::create_dir_all(root.join("SIDE")).unwrap();
        let out = root.join("out");

        let mut tur = Vec::new();
        for (class, model, seed) in [
            ("tur_placed", "tp", 1),
            ("tur_unplaced", "tu", 2),
            ("own_class", "side_own", 3),
        ] {
            let mut sub = synth_modl(model, seed);
            sub.extend(synth_entc(class, model));
            tur.extend(synth_lvl_wrapper(class, &sub));
        }
        std::fs::write(root.join("SIDE/TUR.LVL"), synth_ucfb(&tur)).unwrap();

        let mut body = synth_entc("own_class", "own");
        body.extend(synth_modl("own", 4));
        body.extend(synth_wrld("base", "own_class"));
        body.extend(synth_lvl_wrapper(
            "layer_con",
            &synth_wrld("layer_con", "TUR_Placed"),
        ));
        let map = root.join("MAP/map.lvl");
        std::fs::write(&map, synth_ucfb(&body)).unwrap();
        let m = bake_map(map.to_str().unwrap(), out.to_str().unwrap(), None).expect("bake");

        assert_eq!(m.models, vec!["own".to_string(), "tp".to_string()]);
        assert!(out.join("models/tp.glb").is_file());
        assert!(!out.join("models/tu.glb").exists());
        let geometry = |n: &str| {
            m.classes
                .iter()
                .find(|k| k.name == n)
                .map(|k| k.geometry.clone())
        };
        assert_eq!(geometry("tur_placed"), Some(Some("tp".into())));
        assert_eq!(geometry("own_class"), Some(Some("own".into())));
        assert_eq!(geometry("tur_unplaced"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
