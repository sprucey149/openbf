//! A placed model in its skeleton's bind pose (card TBIND): what the map bake
//! writes for a model that has a `skel`, so its moving parts sit where the PS2
//! draws them at rest.
//!
//! - A `bone_space` model (`modl INFO[0]` = 1): every vertex through its
//!   bone's bind matrix, `mungefs::model::to_model_space` — the vehicle and
//!   soldier bakes' rule (`analysis/re-softskin.md` §3, §12: flag 1 draws the
//!   pose matrices raw, and the rest pose is the bind pose).
//! - Any other model: a segment hard-bound by `BNAM` is stored in its bone's
//!   frame and goes through that bone's bind matrix; a segment with no `BNAM`
//!   stays as stored. PROVISIONAL [D]: `re-softskin.md` §3 reads flag 0 as
//!   model space (invBind × pose), but the PS2 draws BF2 HOT1's laser mortar
//!   with its head on the pillar, 3.57 m up at bone `turret_y`, where the
//!   stored segment sits at y −0.43..0.57
//!   (`lead_tmp/audit-vtur2/pose/bf2_hot1__cp1_cp6_postA_spawn__pair.png`),
//!   and the bone frame is the reading every checked BNAM segment fits
//!   (`lead_tmp/tbind/bnscan2.py`: UTA1's second barrel lands under its own
//!   `hp_fire3`, END1's treesmash trees ±11 m out, YAV1's watchtower gun on
//!   its 15 m tower, the space maps' ship turret barrels at their bones).
//!   The side vehicles fit it too, rotation included (card VBIND): BF1's
//!   Hailfire stores both hoop wheels flat at the origin, and through their
//!   bones they stand upright at x ±2.5..4.65, y 0.06..7.87 — inside the
//!   class's own model-space `coll` mesh (x ±4.72, y 0.01..7.92).
//! - A `door` class follows the same rule (card STDOR, closing BNR's
//!   question), and so do its collision prims: a prim's `XFRM` is in its
//!   `PRNT` bone's frame ([`BindPose::rest_prims`]). Both then rest in model
//!   space, where `crate::door` moves them by `B(f)·B(0)⁻¹`. [D], from the
//!   disc's three door shapes: DEA1's blast door (`bone_leftdoors` binds at
//!   x −5, its `open` clip runs −5 → −10) stores its leaves at x ±0.74..7.36
//!   — past the model's own 12 m frame with a 1.48 m slit — and through the
//!   bones they meet on the diagonal seam inside the frame and open to x
//!   ±2.64, the band the PS2's soldiers cross in (`re-round-stalls.md` §2
//!   C3); MUS1's tri and garage doors bind rotated 20.8° and their clips
//!   travel exactly along the bone's own y axis, so the panel slides in its
//!   plane only when it wears the bone's tilt; MUS1's window stores its two
//!   panes at y 0..3.24 and its shield at y 1.18..4.34, and through their
//!   bones all three lie in one slanted plane, y 1.40..4.47.
//!
//! Map-local vehicles use the side vehicle bake's collision rule too (RKHULT;
//! `analysis/re-walker.md` §17): `XFRM · bind`, including their LOD prims.
//!
//! A bone whose bind matrix is exactly the identity moves nothing, so a model
//! bound only to such bones is written byte for byte as stored.

use mungefs::model::Model;
use mungefs::skel::{xform_dir, xform_point, RowMat4, Skeleton};
use std::collections::HashSet;

const IDENTITY: RowMat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// An LVL's skeletons, geometry LOD lists and door/vehicle geometries, from the map
/// bake's flattened walk (top level first; first of a name wins).
pub struct BindPose {
    skels: crate::hardpoints::Skeletons,
    render_skels: Vec<crate::hardpoints::Skeletons>,
    /// Lower-cased geometry/LOD names whose prims use the bind pose.
    prim_geoms: HashSet<String>,
}

impl BindPose {
    pub fn of(chunks: &[mungefs::Chunk<'_>]) -> BindPose {
        let skels = crate::hardpoints::Skeletons::of(chunks);
        let mut rest_geoms: Vec<String> = Vec::new();
        for c in chunks {
            if &c.tag == b"entc" {
                if let Ok(e) = mungefs::entity::EntityClass::parse(c) {
                    if crate::door::DoorSpec::from_entc(&e).is_some()
                        || crate::vehicle::is_vehicle_base(&e.base)
                    {
                        rest_geoms.extend(e.geometry_name().map(str::to_ascii_lowercase));
                    }
                }
            }
        }
        let mut prim_geoms: HashSet<String> = rest_geoms.iter().cloned().collect();
        for g in skels.geometries() {
            if rest_geoms.iter().any(|d| d.eq_ignore_ascii_case(&g.name)) {
                prim_geoms.extend(g.lods.iter().map(|l| l.model.to_ascii_lowercase()));
            }
        }
        BindPose {
            skels,
            render_skels: Vec::new(),
            prim_geoms,
        }
    }

    /// Original donor bones decode its render vertices. These bytes must come
    /// from gameplay::read_with_original, which verifies the same source revision.
    pub(crate) fn add_render_source(&mut self, original: &[u8]) -> Result<(), String> {
        let inner = mungefs::open(original).map_err(|e| e.to_string())?;
        let chunks = mungefs::content_chunks(inner).map_err(|e| e.to_string())?;
        self.render_skels
            .push(crate::hardpoints::Skeletons::of(&chunks));
        Ok(())
    }

    fn render_skel(&self, model: &str) -> Option<&Skeleton> {
        self.render_skels
            .iter()
            .find_map(|s| s.find_model(model, false))
            .or_else(|| self.skels.find_model(model, false))
    }

    /// The model in its bind pose, or `None` when that moves nothing (no
    /// skeleton, every bound bone at the identity).
    pub fn pose(&self, m: &Model) -> Result<Option<Model>, String> {
        let Some(skel) = self.render_skel(&m.name) else {
            return Ok(None);
        };
        if m.bone_space {
            let mut out = m.clone();
            for seg in out.segments.iter_mut() {
                let (p, n, _) = mungefs::model::to_model_space(m, seg, skel)
                    .map_err(|e| format!("{}: {e}", m.name))?;
                seg.positions = p;
                seg.normals = n;
            }
            out.bone_space = false;
            return Ok(Some(out));
        }
        pose_bnam(m, skel)
    }

    /// Turret/station parts are posed with donor bones, then attach by name to
    /// the canonical gameplay rig. Other rigid models do not consume BMAP again.
    pub(crate) fn canonical_model_nodes(&self, m: &mut Model) -> Result<(), String> {
        if self.render_skels.is_empty() {
            return Ok(());
        }
        let Some(source) = self.render_skel(&m.name) else {
            return Ok(());
        };
        let Some(target) = self.skels.find_model(&m.name, false) else {
            return Ok(());
        };
        for segment in &mut m.segments {
            let mut indices: Vec<_> = segment.bmap.iter().map(|&n| usize::from(n)).collect();
            remap_bone_indices(source, target, &mut indices)?;
            segment.bmap = indices
                .into_iter()
                .map(|n| {
                    u8::try_from(n)
                        .map_err(|_| format!("{}: canonical node exceeds BMAP range", m.name))
                })
                .collect::<Result<_, _>>()?;
        }
        Ok(())
    }

    /// A door or vehicle geometry's collision prims in model space at bind:
    /// each `XFRM` through its `PRNT` bone's bind matrix (STDOR / RKHUL).
    /// This includes map-local vehicles (RKHULT), using the side bake's rule
    /// (`analysis/re-walker.md` §17). Other class types stay as stored.
    pub fn rest_prims(&self, geometry: &str, prims: &mut [manifest::ManifestPrim]) {
        if !self.prim_geoms.contains(&geometry.to_ascii_lowercase()) {
            return;
        }
        if let Some(skel) = self.skels.find_model(geometry, true) {
            prims_at_bind(skel, prims);
        }
    }
}

/// Resolve donor vertex bindings into a canonical rig. Extra detail bones stay
/// at their donor bind position and inherit their nearest canonical ancestor's
/// motion through the canonical inverse bind; they never become physics nodes.
pub(crate) fn remap_bone_indices(
    source: &Skeleton,
    target: &Skeleton,
    indices: &mut [usize],
) -> Result<(), String> {
    let mapped = indices
        .iter()
        .map(|&index| {
            let mut node = Some(index);
            for _ in 0..=source.bones.len() {
                let Some(current) = node else {
                    break;
                };
                let bone = source
                    .bones
                    .get(current)
                    .ok_or_else(|| format!("{}: donor bone {current} out of range", source.name))?;
                if let Some(found) = target.index_of(&bone.name) {
                    return Ok(found);
                }
                node = bone.parent;
            }
            // Only a renamed root gets here: the walk above ended at the donor's root. Both rigs hang from one root
            // bone, so it binds to the canonical one (BF2 Xbox space turrets call theirs `rootnode`).
            match target.bones.iter().position(|b| b.parent.is_none()) {
                Some(root) if target.bones.iter().filter(|b| b.parent.is_none()).count() == 1 => {
                    Ok(root)
                }
                _ => Err(format!(
                    "{}: donor bone {index} has no canonical node or ancestor",
                    source.name
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    indices.copy_from_slice(&mapped);
    Ok(())
}

/// Collision prims in model space at `skel`'s bind pose: each prim's `XFRM`
/// through its `PRNT` bone's bind matrix. A prim whose `PRNT` the skeleton
/// lacks, or whose bone binds at the identity, is left as stored. The door
/// rule ([`BindPose::rest_prims`]) and the vehicle bake's (card RKHUL,
/// `crate::vehicle`: every walker's legs and body, the BARC speeder's hull).
pub fn prims_at_bind(skel: &Skeleton, prims: &mut [manifest::ManifestPrim]) {
    let Ok(m0) = skel.bind_matrices() else {
        return;
    };
    for p in prims.iter_mut() {
        let Some(mat) = skel.index_of(&p.parent).map(|b| &m0[b]) else {
            continue;
        };
        if *mat == IDENTITY {
            continue;
        }
        // Row vectors: the prim's frame is `XFRM · bind`.
        for row in p.rotation.iter_mut() {
            *row = xform_dir(mat, *row);
        }
        p.translation = xform_point(mat, p.translation);
    }
}

/// The module doc's rule for a model that is not `bone_space`: each segment
/// hard-bound by `BNAM` through its bone's bind matrix. `None` when that moves
/// nothing (no `BNAM` names a bone of `skel`, or every bound bone sits at the
/// identity). The map bake's step ([`BindPose::pose`]) and the vehicle bake's
/// (`crate::vehicle`, card VBIND).
pub fn pose_bnam(m: &Model, skel: &Skeleton) -> Result<Option<Model>, String> {
    let m0 = skel
        .bind_matrices()
        .map_err(|e| format!("{}: {e}", m.name))?;
    let mut out = m.clone();
    let mut moved = false;
    for seg in out.segments.iter_mut() {
        let Some(b) = seg.bone.as_deref().and_then(|b| skel.index_of(b)) else {
            continue;
        };
        let mat = &m0[b];
        if *mat == IDENTITY {
            continue;
        }
        for p in seg.positions.iter_mut() {
            *p = xform_point(mat, *p);
        }
        for n in seg.normals.iter_mut() {
            *n = xform_dir(mat, *n);
        }
        moved = true;
    }
    Ok(moved.then_some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_renamed_root_binds_to_the_only_canonical_root() {
        let bone = |name: &str, parent| mungefs::skel::SkelBone {
            name: name.into(),
            crc: mungefs::anim::crc32_lower(name),
            parent,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t: [0.0; 3],
        };
        let source = Skeleton {
            name: "turret".into(),
            bones: vec![
                bone("rootnode", None),
                bone("base", Some(0)),
                bone("aim", Some(1)),
            ],
        };
        let mut target = Skeleton {
            name: "turret".into(),
            bones: vec![bone("base_ps2", None), bone("aim", Some(0))],
        };
        let mut indices = [0, 1, 2];
        remap_bone_indices(&source, &target, &mut indices).unwrap();
        assert_eq!(indices, [0, 0, 1]);
        // With two roots there is no telling which one it means.
        target.bones.push(bone("other", None));
        let mut indices = [0];
        assert!(remap_bone_indices(&source, &target, &mut indices).is_err());
    }

    #[test]
    fn donor_map_pose_keeps_render_binding_separate_from_collision_binding() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let path = crate::join_ci(
            &root,
            "PC_swbf1_exe/Star Wars Battlefront (Classic 2004)/GameData/Data/_LVL_PC/SIDE/cis.lvl",
        );
        if !path.is_file() {
            return;
        }
        let (corrected, original) = crate::gameplay::read_with_original(&path).unwrap();
        let original = original.expect("the supplied PC source must exercise donor binding");
        let a = mungefs::content_chunks(mungefs::open(&corrected).unwrap()).unwrap();
        let b = mungefs::content_chunks(mungefs::open(&original).unwrap()).unwrap();
        let m = model(&a, "cis_tread_hailfire");
        let mut posed = BindPose::of(&a);
        assert!(posed.pose(&m).is_err());
        let mut collision = prims_of(&a, "cis_tread_hailfire");
        posed.rest_prims("cis_tread_hailfire", &mut collision);
        posed.add_render_source(&original).unwrap();
        let actual = posed.pose(&m).unwrap().unwrap();
        let expected = BindPose::of(&b)
            .pose(&model(&b, "cis_tread_hailfire"))
            .unwrap()
            .unwrap();
        for (x, y) in actual.segments.iter().zip(&expected.segments) {
            assert_eq!(x.positions, y.positions);
            assert_eq!(x.normals, y.normals);
        }
        let mut after = prims_of(&a, "cis_tread_hailfire");
        posed.rest_prims("cis_tread_hailfire", &mut after);
        assert_eq!(after, collision);
    }

    /// RKHULT: run class/LOD selection from serialized chunks without disc data.
    #[test]
    fn map_vehicle_prims_use_bind_for_geometry_and_lod() {
        fn build_chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = tag.to_vec();
            out.extend((payload.len() as u32).to_le_bytes());
            out.extend(payload);
            out.resize((out.len() + 3) & !3, 0);
            out
        }
        let mut entc = build_chunk(b"BASE", b"HoVeR\0");
        entc.extend(build_chunk(b"TYPE", b"local_vehicle\0"));
        let mut prop = mungefs::entity::PROP_GEOMETRYNAME.to_le_bytes().to_vec();
        prop.extend(b"vehicle\0");
        entc.extend(build_chunk(b"PROP", &prop));
        let mut buf = build_chunk(b"entc", &entc);
        let mut gmod = build_chunk(b"NAME", b"vehicle\0");
        let mut lod = b"vehicle_lod0\0".to_vec();
        lod.extend(0u32.to_le_bytes());
        gmod.extend(build_chunk(b"LOD0", &lod));
        buf.extend(build_chunk(b"gmod", &gmod));
        let mut info = b"vehicle\0".to_vec();
        info.extend(1u32.to_le_bytes());
        let mut skel = build_chunk(b"INFO", &info);
        skel.extend(build_chunk(b"NAME", b"rudder\0"));
        skel.extend(build_chunk(b"PRNT", b"\0"));
        let rows: [f32; 12] = [
            -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0, 10.0, 2.0, -4.0,
        ];
        let xfrm: Vec<u8> = rows.iter().flat_map(|x| x.to_le_bytes()).collect();
        skel.extend(build_chunk(b"XFRM", &xfrm));
        buf.extend(build_chunk(b"skel", &skel));
        let chunks: Vec<_> = mungefs::chunks(&buf).map(Result::unwrap).collect();
        let bp = BindPose::of(&chunks);
        let p = manifest::ManifestPrim {
            name: "p_fin".into(),
            parent: "rudder".into(),
            kind: "box".into(),
            dims: [1.0, 2.0, 3.0],
            rotation: [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
            translation: [1.0, 2.0, 3.0],
            mask: 0x15,
            mask_own: true,
        };
        for geometry in ["vehicle", "VEHICLE_LOD0"] {
            let mut prims = vec![p.clone()];
            bp.rest_prims(geometry, &mut prims);
            let mut expected = p.clone();
            expected.translation = [9.0, 4.0, -7.0];
            expected.rotation = [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]];
            assert_eq!(prims, vec![expected], "{geometry}");
        }
        // An ordinary building using this skeleton keeps the prior rule.
        let mut building_buf = buf[build_chunk(b"entc", &entc).len()..].to_vec();
        let mut ordinary = build_chunk(b"BASE", b"building\0");
        ordinary.extend(build_chunk(b"TYPE", b"ordinary\0"));
        ordinary.extend(build_chunk(b"PROP", &prop));
        building_buf.extend(build_chunk(b"entc", &ordinary));
        let chunks: Vec<_> = mungefs::chunks(&building_buf).map(Result::unwrap).collect();
        let mut prims = vec![p.clone()];
        BindPose::of(&chunks).rest_prims("vehicle", &mut prims);
        assert_eq!(prims, vec![p]);
    }

    /// RKHULT's real-map acceptance; synthetic fixtures above do not replace it.
    #[test]
    fn tat1_skiff_fins_rest_at_bind() {
        let path = format!(
            "{}/../../../extracted/bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL",
            env!("CARGO_MANIFEST_DIR")
        );
        let Ok(buf) = std::fs::read(path) else {
            eprintln!("SKIP: BF1 TAT1.LVL not present; skiff acceptance unverified");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        // The class `tat_hover_skiff` draws geometry `all_hover_skiff`; prims
        // and the bake's `rest_prims` key on the geometry (map.rs `p.model`).
        let mut prims = prims_of(&chunks, "all_hover_skiff");
        let stored = prims.clone();
        BindPose::of(&chunks).rest_prims("all_hover_skiff", &mut prims);
        assert_ne!(prims, stored, "the skiff's bone prims must move");
        for (name, x) in [("p_fin1", -1.77), ("p_fin2", 1.77)] {
            let p = prims.iter().find(|p| p.name == name).unwrap();
            assert!(
                (p.translation[0] - x).abs() < 0.02,
                "{name}: {:?}",
                p.translation
            );
            assert!(
                (p.translation[2] + 4.10).abs() < 0.02,
                "{name}: {:?}",
                p.translation
            );
        }
        for (p, raw) in prims
            .iter()
            .zip(&stored)
            .filter(|(_, raw)| raw.parent == "dummyroot")
        {
            assert_eq!(p, raw, "identity hull {}", p.name);
        }
    }

    fn lvl(rel: &str) -> Option<Vec<u8>> {
        let p = format!(
            "{}/../../../extracted/bf2/lvl/DATA/_LVL_PS2/{rel}",
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

    fn y_range(m: &Model, bone: &str) -> (f32, f32) {
        let s = m
            .segments
            .iter()
            .find(|s| s.bone.as_deref() == Some(bone))
            .unwrap();
        s.positions
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                (lo.min(p[1]), hi.max(p[1]))
            })
    }

    /// Data-gated: BF2 HOT1's laser mortar (`SIDE/TUR.LVL`) wears its head
    /// on the pillar — the `turret_y` segment, stored at y −0.43..0.57,
    /// rides its bone 3.57 m up — and the base on `turret_base` (the
    /// identity) stays byte for byte as stored.
    #[test]
    fn the_hoth_mortar_head_sits_on_its_bone() {
        let Some(buf) = lvl("SIDE/TUR.LVL") else {
            eprintln!("SKIP: TUR.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        let m = model(&chunks, "hoth_bldg_hoth_turret");
        let posed = BindPose::of(&chunks).pose(&m).unwrap().unwrap();
        let (lo, hi) = y_range(&posed, "turret_y");
        assert!(
            (lo - (3.57 - 0.43)).abs() < 0.02 && (hi - (3.57 + 0.57)).abs() < 0.02,
            "{lo}..{hi}"
        );
        let base = |m: &Model| {
            m.segments
                .iter()
                .find(|s| s.bone.as_deref() == Some("turret_base"))
                .unwrap()
                .positions
                .clone()
        };
        assert_eq!(base(&posed), base(&m));
    }

    fn range(m: &Model, bone: &str, axis: usize) -> (f32, f32) {
        m.segments
            .iter()
            .filter(|s| s.bone.as_deref() == Some(bone))
            .flat_map(|s| s.positions.iter())
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                (lo.min(p[axis]), hi.max(p[axis]))
            })
    }

    fn near(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
    }

    fn prims_of(chunks: &[mungefs::Chunk<'_>], name: &str) -> Vec<manifest::ManifestPrim> {
        let p = chunks
            .iter()
            .filter(|c| &c.tag == b"prim")
            .filter_map(|c| mungefs::coll::Primitives::parse(c).ok())
            .find(|p| p.model.eq_ignore_ascii_case(name))
            .unwrap();
        crate::map::manifest_prims(&p).0
    }

    /// Data-gated (card STDOR): DEA1's blast door rests in its bones' frame —
    /// the leaves inside the 12 m frame and overlapping on the seam, the
    /// prims with them — and the frame's own segments stay as stored.
    #[test]
    fn the_blast_door_leaves_rest_at_their_bones() {
        let Some(buf) = lvl("DEA/DEA1.LVL") else {
            eprintln!("SKIP: DEA1.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        let m = model(&chunks, "dea1_prop_door_blast");
        let bp = BindPose::of(&chunks);
        let posed = bp.pose(&m).unwrap().unwrap();
        assert!(near(range(&m, "bone_leftdoors", 0), (0.74, 7.365)));
        assert!(near(range(&posed, "bone_leftdoors", 0), (-4.26, 2.365)));
        assert!(near(range(&posed, "bone_rightdoors", 0), (-2.365, 4.26)));
        assert_eq!(range(&posed, "dummyroot", 0), range(&m, "dummyroot", 0));
        let stored = prims_of(&chunks, "dea1_prop_door_blast");
        let mut prims = stored.clone();
        bp.rest_prims("dea1_prop_door_blast", &mut prims);
        for (p, s) in prims.iter().zip(&stored) {
            let d = if p.parent == "bone_leftdoors" {
                -5.0
            } else {
                5.0
            };
            assert_eq!(p.translation[0], s.translation[0] + d, "{}", p.name);
            assert_eq!(p.rotation, s.rotation);
        }
        // A geometry that is not a door's keeps its prims.
        let mut other = stored.clone();
        bp.rest_prims("dea1_prop_trashcompactor", &mut other);
        assert_eq!(other, stored);
    }

    /// Data-gated (card STDOR): MUS1's tri door wears its bone's 20.8° tilt,
    /// so the `open` clip's travel (0, 3.707, 1.409) lies in the panel's own
    /// plane; its prim tilts with it.
    #[test]
    fn the_mustafar_tri_door_slides_in_its_own_plane() {
        let Some(buf) = lvl("MUS/MUS1.LVL") else {
            eprintln!("SKIP: MUS1.LVL not present");
            return;
        };
        let chunks = mungefs::content_chunks(mungefs::open(&buf).unwrap()).unwrap();
        let m = model(&chunks, "mus1_prop_door_tri");
        let bp = BindPose::of(&chunks);
        let posed = bp.pose(&m).unwrap().unwrap();
        let travel = [0.0f32, 3.707, 1.409];
        let len = travel.iter().map(|v| v * v).sum::<f32>().sqrt();
        // The panel's extent along the travel's normal in the y-z plane.
        let across = |m: &Model| {
            let n = [0.0, -travel[2] / len, travel[1] / len];
            let (lo, hi) = m.segments[0]
                .positions
                .iter()
                .map(|p| p[1] * n[1] + p[2] * n[2])
                .fold((f32::MAX, f32::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)));
            hi - lo
        };
        assert!(across(&m) > 1.5, "{}", across(&m)); // stored: the travel cuts through it
        assert!(across(&posed) < 0.13, "{}", across(&posed)); // posed: the panel's thickness
        let mut prims = prims_of(&chunks, "mus1_prop_door_tri");
        bp.rest_prims("mus1_prop_door_tri", &mut prims);
        let y_axis = prims[0].rotation[1];
        assert!((y_axis[1] - 0.9346).abs() < 1e-3 && (y_axis[2] - 0.3556).abs() < 1e-3);
    }
}
