//! An armed building's model as rigid per-node parts (card VTURD,
//! `analysis/re-turret-draw.md` 1, 2.4, 6.1 [O]), so the viewer can turn a
//! turret with its aim the way it slides a door's leaves.
//!
//! The PS2 keeps one whole model-space matrix per named node of the model
//! and draws each segment through its node's; the turret's tick writes the
//! `TurretNodeName` node, the aimer its `AimerNodeName` node and each barrel
//! its own, and every other node stays at its bind matrix. Two things are
//! baked for that:
//!
//! - **The model, one GLB primitive per (segment, node).** A hard-skinned
//!   segment (`BNAM`) is on one node already. A soft one (`BMAP` + `BIND`)
//!   is cut by its vertices' node: on every ground turret of both discs each
//!   vertex is on one node at full weight, and no triangle has vertices on
//!   two but on two models (`lead_tmp/vturd/softcensus.py`: 241 of BF1's
//!   6,838 triangles, all on the fambaa, and 8 of BF2's 15,796, on the
//!   space-map `imp_bldg_boxturret`; `analysis/re-turret-draw.md`). A model
//!   with such a triangle is left whole and gets no rig.
//! - **The rig** ([`manifest::ManifestTurretRig`]): every `skel` node's
//!   model-space bind matrix and parent, and the node of each primitive of
//!   the drawn model.
//!
//! Which classes: an `entc` whose BASE chain ends in `armedbuilding` /
//! `armedbuildingdynamic` and that authors `TurretNodeName` itself or by a
//! parent in the same LVL; and every health / ammo / vehicle droid (base
//! `powerupstation`, card PWST, `analysis/re-powerup-station.md` §6 [O]),
//! whose idle turn, wobble, arms and socket move its nodes the same way
//! (both droid models are soft over their bones, one node a vertex). The class keys the pose needs (the turret, aimer
//! and barrel node names, `PitchTurnFactor`, `BarrelRecoil`) stay in the
//! class db, where the game reads the seat's section.

use manifest::{ManifestClass, ManifestRigNode, ManifestTurretRig};
use mungefs::entity::EntityClass;
use mungefs::model::{Model, Segment};
use mungefs::skel::Skeleton;
use std::collections::{HashMap, HashSet};

/// fnv1a_lowercase of `TurretNodeName` (`re-turret-draw.md` 2.1).
pub const PROP_TURRETNODENAME: u32 = 0xb69a_7cb4;

/// The engine bases of a building turret (`game::vehicle_class::TURRET_BASES`).
const BASES: [&str; 2] = ["armedbuilding", "armedbuildingdynamic"];

/// The engine base of a health / ammo / vehicle droid (card PWST).
pub const STATION_BASE: &str = "powerupstation";

/// An LVL's turret classes with what their rigs need, from the map bake's
/// flattened walk (first of a name wins, the walk's own rule).
pub struct TurretRigs {
    skels: crate::hardpoints::Skeletons,
    /// (class, geometry), lower-cased.
    classes: Vec<(String, String)>,
    /// Lower-cased names of every model a turret class draws (its geometry's
    /// own name and LOD models).
    models: HashSet<String>,
    station_models: HashSet<String>,
}

impl TurretRigs {
    pub fn of(chunks: &[mungefs::Chunk<'_>]) -> TurretRigs {
        let skels = crate::hardpoints::Skeletons::of(chunks);
        let mut entcs: Vec<EntityClass> = Vec::new();
        for c in chunks {
            if &c.tag == b"entc" {
                if let Ok(e) = EntityClass::parse(c) {
                    if !entcs.iter().any(|k| k.name.eq_ignore_ascii_case(&e.name)) {
                        entcs.push(e);
                    }
                }
            }
        }
        // A class's chain inside this LVL, itself first (a cycle ends it).
        let chain = |e: &'_ EntityClass| {
            let mut out: Vec<usize> = Vec::new();
            let mut cur = entcs.iter().position(|k| std::ptr::eq(k, e));
            while let Some(i) = cur.filter(|i| !out.contains(i)) {
                out.push(i);
                cur = entcs
                    .iter()
                    .position(|k| k.name.eq_ignore_ascii_case(&entcs[i].base));
            }
            out
        };
        let mut classes = Vec::new();
        let mut models = HashSet::new();
        let mut station_models = HashSet::new();
        for e in &entcs {
            let links = chain(e);
            let root = &entcs[*links.last().expect("itself")].base;
            let armed = BASES.iter().any(|b| b.eq_ignore_ascii_case(root));
            let node = links.iter().any(|&i| {
                entcs[i]
                    .props
                    .iter()
                    .any(|p| p.key_hash == PROP_TURRETNODENAME)
            });
            let geom = links.iter().find_map(|&i| entcs[i].geometry_name());
            let station = STATION_BASE.eq_ignore_ascii_case(root);
            if station {
                for key in [
                    mungefs::entity::PROP_GEOMETRYNAME,
                    mungefs::fnv1a_lowercase("DestroyedGeometryName"),
                ] {
                    if let Some(geometry) = links.iter().find_map(|&i| {
                        entcs[i]
                            .props
                            .iter()
                            .find(|p| p.key_hash == key)
                            .map(|p| p.value.trim())
                    }) {
                        station_models.insert(geometry.to_ascii_lowercase());
                        if let Some(g) = skels.geometry(geometry) {
                            station_models
                                .extend(g.lods.iter().map(|l| l.model.to_ascii_lowercase()));
                        }
                    }
                }
            }
            let (true, Some(geom)) = ((armed && node) || station, geom) else {
                continue;
            };
            let geom = geom.to_ascii_lowercase();
            models.insert(geom.clone());
            if let Some(g) = skels.geometry(&geom) {
                models.extend(g.lods.iter().map(|l| l.model.to_ascii_lowercase()));
            }
            classes.push((e.name.to_ascii_lowercase(), geom));
        }
        TurretRigs {
            skels,
            classes,
            models,
            station_models,
        }
    }

    pub fn station_model(&self, name: &str) -> bool {
        self.station_models.contains(&name.to_ascii_lowercase())
    }

    pub(crate) fn has_model(&self, name: &str) -> bool {
        self.models.contains(&name.to_ascii_lowercase())
    }

    /// A turret class's model with every soft segment cut into one segment
    /// per node. `Ok(None)` for any other model and for one that needs no
    /// cut; `Err` names a model that cannot be cut (a triangle across nodes,
    /// a vertex with no node) — it is written whole and gets no rig.
    pub fn split(&self, m: &Model) -> Result<Option<Model>, String> {
        if !self.models.contains(&m.name.to_ascii_lowercase())
            || self.skels.find_model(&m.name, true).is_none()
        {
            return Ok(None);
        }
        let mut segments = Vec::new();
        let mut cut = false;
        for seg in &m.segments {
            let parts = split_segment(seg).map_err(|e| format!("{}: {e}", m.name))?;
            cut |= parts.len() > 1;
            segments.extend(parts);
        }
        Ok(cut.then(|| Model {
            name: m.name.clone(),
            segments,
            ..*m
        }))
    }

    /// Fill [`ManifestClass::turret_rig`] for every turret class whose drawn
    /// model is among `models` (the bake's written models, lower-cased
    /// names, already [`Self::split`]). Returns one line per class for the
    /// bake's report.
    pub fn attach<'a>(
        &self,
        models: &HashMap<String, Model>,
        classes: impl Iterator<Item = &'a mut ManifestClass>,
    ) -> Vec<String> {
        let mut report = Vec::new();
        for k in classes {
            let lower = k.name.to_ascii_lowercase();
            let Some((_, geom)) = self.classes.iter().find(|(c, _)| *c == lower) else {
                continue;
            };
            let drawn = self
                .skels
                .geometry(geom)
                .and_then(|g| g.best_model())
                .map_or(geom.clone(), str::to_ascii_lowercase);
            let (Some(m), Some(skel)) = (models.get(&drawn), self.skels.find_model(&drawn, true))
            else {
                report.push(format!("{}: no model or skel for {drawn} - no rig", k.name));
                continue;
            };
            match rig(m, skel) {
                Ok(r) => {
                    let bound = r.parts.iter().flatten().count();
                    report.push(format!(
                        "{}: {} nodes, {} of {} parts on a node ({drawn})",
                        k.name,
                        r.nodes.len(),
                        bound,
                        r.parts.len()
                    ));
                    k.turret_rig = Some(r);
                }
                Err(e) => report.push(format!("{}: {e} - no rig", k.name)),
            }
        }
        report
    }
}

/// The skeleton node a vertex of `seg` is on, when the segment is skinned.
fn soft_node(seg: &Segment, v: u32) -> Result<usize, String> {
    seg.vertex_bone(v as usize)
        .ok_or_else(|| format!("vertex {v} has no node"))
}

/// One segment as its per-node pieces, in the order their first triangle
/// comes: a static or hard-skinned segment is one piece (itself), a soft one
/// with all its triangles on one node too.
fn split_segment(seg: &Segment) -> Result<Vec<Segment>, String> {
    if seg.bone.is_some() || seg.bmap.is_empty() || seg.triangles.is_empty() {
        return Ok(vec![seg.clone()]);
    }
    let mut order: Vec<usize> = Vec::new();
    let mut by_node: HashMap<usize, Vec<[u32; 3]>> = HashMap::new();
    for t in &seg.triangles {
        let n = soft_node(seg, t[0])?;
        if soft_node(seg, t[1])? != n || soft_node(seg, t[2])? != n {
            return Err("a triangle has vertices on two nodes".into());
        }
        if !by_node.contains_key(&n) {
            order.push(n);
        }
        by_node.entry(n).or_default().push(*t);
    }
    if order.len() == 1 {
        return Ok(vec![seg.clone()]);
    }
    let pick = |n: usize| {
        let tris = &by_node[&n];
        // Its vertices, in the segment's order, renumbered.
        let mut used: Vec<u32> = tris.iter().flatten().copied().collect();
        used.sort_unstable();
        used.dedup();
        let at = |v: u32| used.binary_search(&v).expect("a used vertex") as u32;
        fn take<T: Copy>(src: &[T], used: &[u32]) -> Vec<T> {
            if src.is_empty() {
                return Vec::new();
            }
            used.iter().map(|&v| src[v as usize]).collect()
        }
        let mut s = seg.clone();
        s.positions = take(&seg.positions, &used);
        s.normals = take(&seg.normals, &used);
        s.uvs = take(&seg.uvs, &used);
        s.colors = seg.colors.as_ref().map(|c| take(c, &used));
        s.bind = take(&seg.bind, &used);
        s.wght = take(&seg.wght, &used);
        s.triangles = tris.iter().map(|t| t.map(at)).collect();
        s.vertex_count = used.len() as u32;
        s.index_count = (s.triangles.len() * 3) as u32;
        s
    };
    Ok(order.into_iter().map(pick).collect())
}

/// The rig of a (split) model over its skeleton.
fn rig(m: &Model, skel: &Skeleton) -> Result<ManifestTurretRig, String> {
    let bind = skel
        .bind_matrices()
        .map_err(|e| format!("skel {}: {e}", skel.name))?;
    let nodes = skel
        .bones
        .iter()
        .zip(&bind)
        .map(|(b, m)| ManifestRigNode {
            name: b.name.clone(),
            parent: b.parent.map(|p| p as u32),
            bind: std::array::from_fn(|r| [m[r][0], m[r][1], m[r][2]]),
        })
        .collect();
    let mut parts = Vec::new();
    for seg in m.segments.iter().filter(|s| !s.triangles.is_empty()) {
        let node = match &seg.bone {
            Some(b) => skel.index_of(b),
            None if seg.bmap.is_empty() => None,
            None => {
                let n = soft_node(seg, seg.triangles[0][0])?;
                if seg
                    .triangles
                    .iter()
                    .flatten()
                    .any(|&v| seg.vertex_bone(v as usize) != Some(n))
                {
                    return Err(format!("{}: a soft segment on two nodes", m.name));
                }
                (n < skel.bones.len()).then_some(n)
            }
        };
        parts.push(node.map(|n| n as u32));
    }
    Ok(ManifestTurretRig { nodes, parts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_split_preserves_model_metadata_and_first_node_order() {
        let seg = Segment {
            vertex_count: 6,
            index_count: 6,
            bf2_material: true,
            pc_geometry: false,
            material_flags: 2,
            render_type: mungefs::model::RenderType::Absent,
            spec_power: 4,
            spec_color: 7,
            uv_anim: [0, 0],
            textures: vec![(0, "sheet".into())],
            bone: None,
            group: None,
            bmap: vec![0, 1],
            bind: vec![[0; 4], [0; 4], [0; 4], [4; 4], [4; 4], [4; 4]],
            wght: vec![[255, 0, 0, 0]; 6],
            positions: vec![[1.0, 2.0, 3.0]; 6],
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: None,
            triangles: vec![[3, 4, 5], [0, 1, 2]],
        };
        let mut m = Model {
            name: "fixture".into(),
            bone_space: true,
            vertex_box: mungefs::model::Aabb {
                min: [-1.0, -2.0, -3.0],
                max: [1.0, 2.0, 3.0],
            },
            vis_box: mungefs::model::Aabb {
                min: [-4.0, -5.0, -6.0],
                max: [4.0, 5.0, 6.0],
            },
            face_count: 17,
            sphere: Some([2.0, 3.0, 4.0, 5.0]),
            segments: vec![seg],
        };
        let rigs = TurretRigs {
            station_models: HashSet::new(),
            skels: crate::hardpoints::Skeletons::from_skeleton(Skeleton {
                name: m.name.clone(),
                bones: Vec::new(),
            }),
            classes: Vec::new(),
            models: HashSet::from([m.name.clone()]),
        };
        let out = rigs.split(&m).unwrap().unwrap();
        assert_eq!(out.name, m.name);
        assert_eq!(out.bone_space, m.bone_space);
        assert_eq!(out.vertex_box.min, m.vertex_box.min);
        assert_eq!(out.vertex_box.max, m.vertex_box.max);
        assert_eq!(out.vis_box.min, m.vis_box.min);
        assert_eq!(out.vis_box.max, m.vis_box.max);
        assert_eq!(out.face_count, m.face_count);
        assert_eq!(out.sphere, m.sphere);
        assert_eq!(out.segments.len(), 2);
        for (part, node) in out.segments.iter().zip([1, 0]) {
            assert_eq!(soft_node(part, 0).unwrap(), node);
            assert_eq!(part.triangles, [[0, 1, 2]]);
            assert_eq!(part.positions, m.segments[0].positions[..3]);
            assert_eq!(part.material_flags, 2);
            assert_eq!(part.spec_color, 7);
            assert_eq!(part.spec_power, 4);
        }
        m.segments[0].triangles = vec![[0, 1, 2]];
        assert!(rigs.split(&m).unwrap().is_none());
        m.segments[0].triangles = vec![[0, 1, 3]];
        assert_eq!(
            rigs.split(&m).err().as_deref(),
            Some("fixture: a triangle has vertices on two nodes")
        );
    }

    fn lvl(game: &str, rel: &str) -> Option<Vec<u8>> {
        let p = format!(
            "{}/../../../extracted/{game}/lvl/DATA/_LVL_PS2/{rel}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(p).ok()
    }

    fn model(chunks: &[mungefs::Chunk<'_>], name: &str) -> Model {
        chunks
            .iter()
            .filter(|c| &c.tag == b"modl")
            .filter_map(|c| Model::parse(c).ok())
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .unwrap()
    }

    fn class(name: &str) -> ManifestClass {
        class_named(name, "armedbuilding")
    }

    fn class_named(name: &str, base: &str) -> ManifestClass {
        ManifestClass {
            effects: Vec::new(),
            name: name.into(),
            base: base.into(),
            geometry: None,
            collision: vec![],
            door: None,
            hardpoints: vec![],
            attach: vec![],
            build_zone: None,
            lights: vec![],
            turret_rig: None,
        }
    }

    /// Data-gated: BF2's Mygeeto anti-aircraft gun (`SIDE/TUR.LVL`,
    /// `kam_prop_antiaircraft`) is soft-skinned over five nodes. Cut, it
    /// keeps every triangle and vertex position, each piece is on one node,
    /// and the rig names the two barrels' pieces under the aimer node.
    #[test]
    fn the_anti_aircraft_gun_cuts_into_one_part_per_node() {
        let Some(buf) = lvl("bf2", "SIDE/TUR.LVL") else {
            eprintln!("SKIP: TUR.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        let rigs = TurretRigs::of(&chunks);
        let stored = model(&chunks, "kam_prop_antiaircraft");
        let posed = crate::bind_pose::BindPose::of(&chunks)
            .pose(&stored)
            .unwrap()
            .unwrap_or(stored);
        let cut = rigs.split(&posed).unwrap().expect("a soft model is cut");
        let tris = |m: &Model| m.segments.iter().map(|s| s.triangles.len()).sum::<usize>();
        assert_eq!(tris(&cut), tris(&posed));
        assert!(cut.segments.len() > posed.segments.len());
        // The same triangles in space: the sum of every corner's position.
        let sum = |m: &Model| {
            m.segments
                .iter()
                .flat_map(|s| {
                    s.triangles
                        .iter()
                        .flatten()
                        .map(|&v| s.positions[v as usize])
                })
                .fold([0.0f64; 3], |a, p| {
                    [a[0] + p[0] as f64, a[1] + p[1] as f64, a[2] + p[2] as f64]
                })
        };
        let (a, b) = (sum(&cut), sum(&posed));
        assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-3), "{a:?} {b:?}");

        let mut k = class("tur_bldg_recoilless_lg");
        let models = HashMap::from([("kam_prop_antiaircraft".to_string(), cut.clone())]);
        let report = rigs.attach(&models, std::iter::once(&mut k));
        eprintln!("{report:?}");
        let r = k.turret_rig.expect("a rig");
        assert_eq!(r.parts.len(), cut.segments.len());
        assert!(r.parts.iter().all(Option::is_some), "{:?}", r.parts);
        let idx = |n: &str| r.nodes.iter().position(|x| x.name == n).unwrap() as u32;
        for b in ["barrel1", "barrel2"] {
            assert!(r.parts.contains(&Some(idx(b))), "{b}");
            assert_eq!(r.nodes[idx(b) as usize].parent, Some(idx("aimer_x")));
        }
        assert_eq!(
            r.nodes[idx("aimer_x") as usize].parent,
            Some(idx("aimer_y"))
        );
    }

    /// Data-gated: a model no turret class draws is not cut, and BF1's gun
    /// turret (hard-skinned throughout, `COMMON.LVL`) needs none but gets
    /// its rig.
    #[test]
    fn a_hard_skinned_turret_keeps_its_segments_and_gets_a_rig() {
        let Some(buf) = lvl("bf1", "COMMON/ENG/COMMON.LVL") else {
            eprintln!("SKIP: COMMON.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        let rigs = TurretRigs::of(&chunks);
        let m = model(&chunks, "com_weap_gunturret");
        assert!(rigs.split(&m).unwrap().is_none());
        let other = chunks
            .iter()
            .filter(|c| &c.tag == b"modl")
            .filter_map(|c| Model::parse(c).ok())
            .find(|x| !x.name.eq_ignore_ascii_case("com_weap_gunturret"))
            .unwrap();
        assert!(rigs.split(&other).unwrap().is_none());
        let mut k = class("com_weap_gunturret");
        let models = HashMap::from([("com_weap_gunturret".to_string(), m.clone())]);
        rigs.attach(&models, std::iter::once(&mut k));
        let r = k.turret_rig.expect("a rig");
        let name = |p: &Option<u32>| p.map(|i| r.nodes[i as usize].name.to_ascii_lowercase());
        let on: Vec<_> = r.parts.iter().map(name).collect();
        assert!(on.contains(&Some("turret_main".into())), "{on:?}");
        assert!(on.contains(&Some("main_cylinder".into())), "{on:?}");
    }

    /// Data-gated: BF1's health and ammo droids (`COMMON.LVL`, card PWST)
    /// are cut into one part per node, every triangle kept, and the rig puts
    /// a part on each node the class's animation names (re-powerup-station
    /// §6: the FX-7's two arms and socket, the gonk's body and feet).
    #[test]
    fn the_droids_cut_into_their_animated_nodes() {
        let Some(buf) = lvl("bf1", "COMMON/ENG/COMMON.LVL") else {
            eprintln!("SKIP: COMMON.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        let rigs = TurretRigs::of(&chunks);
        let poser = crate::bind_pose::BindPose::of(&chunks);
        for (class, geom, nodes) in [
            (
                "com_item_healthrecharge",
                "com_item_healthrecharge",
                &[
                    "bone_root",
                    "ph_tester_arm10",
                    "ph_tester_arm6",
                    "equipment_operator_arm_socket",
                ][..],
            ),
            (
                "com_item_weaponrecharge",
                "com_item_weaponrecharge",
                &["gonk1", "foot_l2", "foot_r1"][..],
            ),
        ] {
            let stored = model(&chunks, geom);
            let posed = poser.pose(&stored).unwrap().unwrap_or(stored);
            let cut = rigs.split(&posed).unwrap().expect("a soft droid is cut");
            let tris = |m: &Model| m.segments.iter().map(|s| s.triangles.len()).sum::<usize>();
            assert_eq!(tris(&cut), tris(&posed), "{geom}");
            let mut k = class_named(class, STATION_BASE);
            let models = HashMap::from([(geom.to_string(), cut.clone())]);
            eprintln!("{:?}", rigs.attach(&models, std::iter::once(&mut k)));
            let r = k.turret_rig.expect("a rig");
            let on: HashSet<String> = r
                .parts
                .iter()
                .flatten()
                .map(|&i| r.nodes[i as usize].name.to_ascii_lowercase())
                .collect();
            for n in nodes {
                assert!(on.contains(*n), "{geom}: {n} not in {on:?}");
            }
        }
    }
}
