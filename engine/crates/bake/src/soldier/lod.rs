//! GeometryLowRes is a distinct authored mesh, normalized to the high rig once.
use super::{bones_of, find_soldier, model_skins, to_skinned_model};
use crate::classes::ClassProps;
use anyhow::{bail, Context, Result};
use glam::Vec3;
use mungefs::{model::Model, skel::Skeleton};
use std::path::Path;

fn low_name(classes: &[manifest::ClassDef], model: &str) -> Result<Option<String>> {
    let mut found = None;
    for class in classes.iter().filter(|c| c.kind == "entc") {
        let mut chain = vec![class];
        while let Some(parent) = classes
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&chain.last().unwrap().base))
        {
            if chain.len() >= 16 {
                break;
            }
            chain.push(parent);
        }
        let prop = |name| chain.iter().find_map(|c| c.prop(name)).map(str::trim);
        if !prop("GeometryName").is_some_and(|n| n.eq_ignore_ascii_case(model)) {
            continue;
        }
        if let Some(low) = prop("GeometryLowRes") {
            if found
                .as_deref()
                .is_some_and(|n: &str| !n.eq_ignore_ascii_case(low))
            {
                bail!("{model}: classes disagree on GeometryLowRes");
            }
            found = Some(low.to_ascii_lowercase());
        }
    }
    Ok(found)
}

/// How far a low bone's rest point may miss the high joint of the same name,
/// once its parent's map has carried it onto the high rig, metres. OURS: the
/// census in `real_low_rigs_land_on_their_high_joints` (103 low rigs of both
/// discs) finds every collapsed neck 0.110-0.310 m off, the `leialz` heads
/// 0.211 m, and every other head within 0.054 m once its neck is fitted;
/// measure: that census on a new disc's sides.
const JOINT_SLACK: f32 = 0.08;

/// How a low bone's rest was fitted to the high rig ([`low_to_high`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fit {
    /// Its own bind lands on its high joint: the plain map.
    Own,
    /// Carried by its moved parent at its own bind offset, it lands.
    Follow,
    /// Moved onto its high joint (in its parent's frame).
    Snap,
}

/// Each low bone's rigid map from the low model's bind space to the high
/// rig's: `high bind * low rest^-1`. Every low rig of both discs hangs
/// `bone_neck` from the pelvis at ribcage height. Most fold the neck's
/// length into the head bone's offset, so their head bone still sits on the
/// head; the BF2 `_low1` bodies the classes drive with the `leialz` low-res
/// rig (the Alliance sniper and Hoth sniper, Leia, the Naboo queen) do not,
/// and their head bone sits 21 cm under the head their own mesh draws:
/// mapped bone to bone, that head rose 22 cm and the neck stretched up to it.
///
/// So the rest of a bone is its own low bind, except in the neck chain
/// (`bone_neck`, `bone_head`) when that, carried by its parent's map, misses
/// the high joint of the same name by more than [`JOINT_SLACK`]: then its
/// parent's rest at the bone's own bind offset, when that lands, else its own
/// bind moved onto the joint. Other bones keep the plain map: BF1 Vader's
/// low rig sits 0.26 m off at the pelvis as a whole, and bone to bone keeps
/// his low head within 9 cm of the high one. Also returns each bone's fit and
/// the miss of its own bind, metres (0 for a root).
fn low_to_high(
    low_bones: &[crate::skin_glb::Bone],
    low_bind: &[glam::Mat4],
    high_bind: &[glam::Mat4],
    remap: &[usize],
) -> (Vec<glam::Mat4>, Vec<(Fit, f32)>) {
    use glam::Mat4;
    fn resolve(
        j: usize,
        bones: &[crate::skin_glb::Bone],
        low_bind: &[Mat4],
        high_bind: &[Mat4],
        remap: &[usize],
        memo: &mut [Option<(Mat4, Fit, f32)>],
    ) -> (Mat4, Fit, f32) {
        if let Some(r) = memo[j] {
            return r;
        }
        let out = match bones[j].parent {
            None => (low_bind[j], Fit::Own, 0.0),
            Some(p) => {
                let (parent_rest, _, _) = resolve(p, bones, low_bind, high_bind, remap, memo);
                let parent_map = high_bind[remap[p]] * parent_rest.inverse();
                let joint = high_bind[remap[j]].w_axis.truncate();
                let miss = |rest: Mat4| {
                    parent_map
                        .transform_point3(rest.w_axis.truncate())
                        .distance(joint)
                };
                let own = low_bind[j];
                let follow = parent_rest * (low_bind[p].inverse() * low_bind[j]);
                let own_miss = miss(own);
                let neck_chain = ["bone_neck", "bone_head"]
                    .iter()
                    .any(|n| bones[j].name.eq_ignore_ascii_case(n));
                if own_miss <= JOINT_SLACK || !neck_chain {
                    (own, Fit::Own, own_miss)
                } else if miss(follow) <= JOINT_SLACK {
                    (follow, Fit::Follow, own_miss)
                } else {
                    let mut rest = own;
                    rest.w_axis = parent_map.inverse().transform_point3(joint).extend(1.0);
                    (rest, Fit::Snap, own_miss)
                }
            }
        };
        memo[j] = Some(out);
        out
    }
    let mut memo = vec![None; low_bones.len()];
    (0..low_bones.len())
        .map(|j| {
            let (rest, fit, miss) = resolve(j, low_bones, low_bind, high_bind, remap, &mut memo);
            (high_bind[remap[j]] * rest.inverse(), (fit, miss))
        })
        .unzip()
}

/// [`bake`], for the normal build: the low model is a look, so one that cannot be
/// read leaves this soldier at full detail, as the game draws an old bake, rather
/// than refusing the build. Returns the textures it wrote.
pub(super) fn bake_or_skip(
    inner: &[u8],
    buf: &[u8],
    high: &Model,
    skeleton: &Skeleton,
    out: &Path,
    pc: Option<&crate::tier::PcTextures>,
) -> Vec<String> {
    bake(inner, buf, high, skeleton, out, pc).unwrap_or_else(|e| {
        eprintln!("soldier {}: {e:#}, no low model", high.name);
        Vec::new()
    })
}

fn bake(
    inner: &[u8],
    buf: &[u8],
    high: &Model,
    skeleton: &Skeleton,
    out: &Path,
    pc: Option<&crate::tier::PcTextures>,
) -> Result<Vec<String>> {
    std::fs::create_dir_all(out.join("soldiers"))?;
    std::fs::create_dir_all(out.join("textures"))?;
    let classes = crate::anim_sets::bf2::side_classes(buf)?;
    let Some(name) = low_name(&classes, &high.name)? else {
        return Ok(Vec::new());
    };
    let (model, low_skeleton) = find_soldier(inner, &name)?;
    let mut low = to_skinned_model(&model, &low_skeleton)?;
    let high_bones = bones_of(skeleton);
    let high_bind = crate::skin_glb::global_binds(&high_bones);
    let low_bind = crate::skin_glb::global_binds(&low.bones);
    let remap: Vec<_> = low
        .bones
        .iter()
        .map(|b| {
            skeleton
                .index_of(&b.name)
                .with_context(|| format!("{name}: low bone {} absent from {}", b.name, high.name))
        })
        .collect::<Result<_>>()?;
    let (transforms, _) = low_to_high(&low.bones, &low_bind, &high_bind, &remap);
    // Rigid per-vertex PS2 skinning: rewrite bind positions and joint IDs so
    // the existing animator/palette can drive either mesh with no extra work.
    for part in &mut low.segments {
        for i in 0..part.positions.len() {
            let j = usize::from(part.joints[i][0]);
            let transform = transforms[j];
            part.positions[i] = transform
                .transform_point3(Vec3::from(part.positions[i]))
                .to_array();
            part.normals[i] = transform
                .transform_vector3(Vec3::from(part.normals[i]))
                .normalize_or_zero()
                .to_array();
            part.joints[i][0] = u8::try_from(remap[j]).context("soldier LOD exceeds 256 joints")?;
        }
    }
    // Adjacent opaque segments with the same material need one draw, even
    // when the PS2 split them for its per-packet bone palette. All joints now
    // share the high rig; retain vertex/triangle order and skin overrides.
    let mut merged: Vec<crate::skin_glb::SkinnedSegment> = Vec::new();
    for mut part in low.segments {
        if let Some(previous) = merged.last_mut().filter(|previous| {
            !part.blend
                && !previous.blend
                && previous.texture == part.texture
                && previous.spec == part.spec
                && previous.cloth == part.cloth
        }) {
            let offset = previous.positions.len() as u32;
            previous.positions.append(&mut part.positions);
            previous.normals.append(&mut part.normals);
            previous.uvs.append(&mut part.uvs);
            previous.joints.append(&mut part.joints);
            previous.weights.append(&mut part.weights);
            previous
                .triangles
                .extend(part.triangles.into_iter().map(|t| t.map(|i| i + offset)));
        } else {
            merged.push(part);
        }
    }
    low.segments = merged;
    low.bones = high_bones;
    let companion = format!("{}.low", high.name);
    low.name = companion.clone();
    std::fs::write(
        out.join("soldiers").join(format!("{companion}.glb")),
        crate::skin_glb::skinned_to_glb(&low, &[])?,
    )?;
    let skins = model_skins(&model, &classes);
    crate::write_ron(
        &out.join("soldiers").join(format!("{companion}.skins.ron")),
        &manifest::SoldierSkins {
            groups: skins.groups,
            overrides: Vec::new(),
        },
    )?;
    let sphere = high.sphere.unwrap_or_else(|| {
        let lo = Vec3::from(high.vis_box.min);
        let hi = Vec3::from(high.vis_box.max);
        ((lo + hi) * 0.5).extend(lo.distance(hi) * 0.5).to_array()
    });
    crate::write_ron(
        &out.join("soldiers").join(format!("{}.lod.ron", high.name)),
        &manifest::SoldierLod {
            geometry: name,
            high_count: high.segments.iter().map(|s| s.index_count).sum(),
            sphere,
        },
    )?;
    let textures: Vec<_> = low
        .segments
        .iter()
        .map(|s| s.texture.to_ascii_lowercase())
        .collect();
    let mut flags = super::bake_flags(high);
    for (texture, bits) in super::bake_flags(&model) {
        *flags.entry(texture).or_default() |= bits;
    }
    super::bake_textures_flagged(inner, &textures, &flags, out, pc)
}

#[cfg(test)]
mod tests {
    use super::Fit::{Follow, Own, Snap};
    use crate::skin_glb::{global_binds, Bone};
    use glam::Vec3;

    fn bone(name: &str, parent: Option<usize>, y: f32) -> Bone {
        Bone {
            name: name.into(),
            parent,
            translation: [0.0, y, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// A low rig whose neck hangs from the pelvis at ribcage height with the
    /// head at its own short offset: its head vertices stay where the low mesh
    /// draws them (they rose 0.2 m when each bone was mapped on its own). A rig
    /// that matches, and one that folds the neck into the head's offset, keep
    /// the plain map for every bone that lands, bit for bit.
    #[test]
    fn a_collapsed_low_neck_keeps_the_head_where_the_low_mesh_draws_it() {
        let high = vec![
            bone("bone_pelvis", None, 1.0),
            bone("bone_ribcage", Some(0), 0.3),
            bone("bone_neck", Some(1), 0.2),
            bone("bone_head", Some(2), 0.08),
        ];
        let high_bind = global_binds(&high);
        // The PS2's collapsed shape: the neck at the ribcage, from the pelvis.
        let low = vec![
            bone("bone_pelvis", None, 1.0),
            bone("bone_ribcage", Some(0), 0.3),
            bone("bone_neck", Some(0), 0.3),
            bone("bone_head", Some(2), 0.08),
        ];
        let low_bind = global_binds(&low);
        let remap = [0, 1, 2, 3];
        let (maps, fit) = super::low_to_high(&low, &low_bind, &high_bind, &remap);
        let kinds: Vec<_> = fit.iter().map(|f| f.0).collect();
        assert_eq!(kinds, [Own, Own, Snap, Follow]);
        assert!((fit[2].1 - 0.2).abs() < 1e-5, "{fit:?}");
        // A head vertex the low mesh draws at 1.62 m stays at 1.62 m.
        let y = maps[3].transform_point3(Vec3::new(0.0, 1.62, 0.0)).y;
        assert!((y - 1.62).abs() < 1e-5, "{y}");
        let plain = high_bind[3] * low_bind[3].inverse();
        assert!((plain.transform_point3(Vec3::new(0.0, 1.62, 0.0)).y - 1.82).abs() < 1e-5);

        let (same, fit) = super::low_to_high(&high, &high_bind, &high_bind, &remap);
        assert!(fit.iter().all(|f| *f == (Own, 0.0)), "{fit:?}");
        // The common low rig: the neck collapsed, its length folded into the
        // head's offset. The head keeps its own bind and its plain map.
        let folded = vec![
            bone("bone_pelvis", None, 1.0),
            bone("bone_ribcage", Some(0), 0.3),
            bone("bone_neck", Some(0), 0.3),
            bone("bone_head", Some(2), 0.28),
        ];
        let folded_bind = global_binds(&folded);
        let (maps2, fit) = super::low_to_high(&folded, &folded_bind, &high_bind, &remap);
        assert_eq!(
            fit.iter().map(|f| f.0).collect::<Vec<_>>(),
            [Own, Own, Snap, Own]
        );
        assert_eq!(maps2[3], high_bind[3] * folded_bind[3].inverse());
        for j in 0..4 {
            assert_eq!(same[j], high_bind[j] * high_bind[j].inverse());
        }
    }

    /// Census over every side LVL of both discs: each class's `GeometryLowRes`
    /// rig against its high rig. Only neck-chain bones that carry vertices
    /// change (20 bodies); no head moves away from the high head, and the three
    /// Alliance bodies whose heads rose 21-22 cm now crown within 3 cm of it.
    #[test]
    #[ignore = "requires the extracted PS2 discs"]
    fn real_low_rigs_land_on_their_high_joints() {
        use super::super::{bones_of, find_soldier, to_skinned_model};
        use crate::classes::ClassProps;
        let mut moved_rigs = Vec::new();
        let mut checked = 0;
        for game in ["bf1", "bf2"] {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../extracted")
                .join(game)
                .join("lvl/DATA/_LVL_PS2/SIDE");
            let dir = crate::resolve_ci(dir.to_str().unwrap());
            let mut lvls: Vec<_> = std::fs::read_dir(&dir)
                .expect("real PS2 side LVLs required")
                .map(|e| e.unwrap().path())
                .collect();
            lvls.sort();
            for path in lvls {
                let bytes = std::fs::read(&path).unwrap();
                let Ok(inner) = mungefs::open(&bytes) else {
                    continue;
                };
                let Ok(classes) = crate::anim_sets::bf2::side_classes(&bytes) else {
                    continue;
                };
                let mut pairs = std::collections::BTreeSet::new();
                for c in classes.iter().filter(|c| c.kind == "entc") {
                    if let Some(high) = c.prop("GeometryName") {
                        if let Ok(Some(low)) = super::low_name(&classes, high.trim()) {
                            pairs.insert((high.trim().to_ascii_lowercase(), low));
                        }
                    }
                }
                for (high, low) in pairs {
                    let (Ok((hm, hs)), Ok((lm, ls))) =
                        (find_soldier(inner, &high), find_soldier(inner, &low))
                    else {
                        continue;
                    };
                    let Ok(lowm) = to_skinned_model(&lm, &ls) else {
                        continue;
                    };
                    let Ok(highm) = to_skinned_model(&hm, &hs) else {
                        continue;
                    };
                    let high_bind = global_binds(&bones_of(&hs));
                    let low_bind = global_binds(&lowm.bones);
                    let Some(remap) = lowm
                        .bones
                        .iter()
                        .map(|b| hs.index_of(&b.name))
                        .collect::<Option<Vec<_>>>()
                    else {
                        continue;
                    };
                    checked += 1;
                    let (maps, fit) =
                        super::low_to_high(&lowm.bones, &low_bind, &high_bind, &remap);
                    // Only bones that carry vertices change what is drawn.
                    let skinned: std::collections::BTreeSet<usize> = lowm
                        .segments
                        .iter()
                        .flat_map(|s| s.joints.iter().map(|j| usize::from(j[0])))
                        .collect();
                    let names: Vec<_> = (0..fit.len())
                        .filter(|&j| fit[j].0 != Own && skinned.contains(&j))
                        .map(|j| {
                            let n = lowm.bones[j].name.to_ascii_lowercase();
                            format!("{n} {:?} {:.3}", fit[j].0, fit[j].1)
                        })
                        .collect();
                    if names.is_empty() {
                        continue;
                    }
                    // The head's vertical extent, low (mapped) against high.
                    let span = |m: &crate::skin_glb::SkinnedModel,
                                map: &dyn Fn(usize, Vec3) -> Vec3| {
                        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                        for seg in &m.segments {
                            for (p, j) in seg.positions.iter().zip(&seg.joints) {
                                let j = usize::from(j[0]);
                                if m.bones[j].name.eq_ignore_ascii_case("bone_head") {
                                    let y = map(j, Vec3::from(*p)).y;
                                    lo = lo.min(y);
                                    hi = hi.max(y);
                                }
                            }
                        }
                        (lo, hi)
                    };
                    let h = span(&highm, &|_, p| p);
                    let l = span(&lowm, &|j, p| maps[j].transform_point3(p));
                    let plain = span(&lowm, &|j, p| {
                        (high_bind[remap[j]] * low_bind[j].inverse()).transform_point3(p)
                    });
                    eprintln!(
                        "{game} {}: {high} / {low}: moved {names:?}; head high {:.3}..{:.3} low {:.3}..{:.3} (bone to bone {:.3}..{:.3})",
                        path.file_name().unwrap().to_string_lossy(),
                        h.0,
                        h.1,
                        l.0,
                        l.1,
                        plain.0,
                        plain.1
                    );
                    moved_rigs.push((high, names, (l.1 - h.1).abs(), (plain.1 - h.1).abs()));
                }
            }
        }
        eprintln!("{checked} low rigs, {} moved", moved_rigs.len());
        assert!(checked > 100, "{checked}");
        assert_eq!(moved_rigs.len(), 20);
        for (high, names, top, plain) in &moved_rigs {
            assert!(
                names
                    .iter()
                    .all(|n| n.starts_with("bone_neck") || n.starts_with("bone_head")),
                "{high}: {names:?}"
            );
            // The top of the head (the meshes' chins and helmets differ between
            // LODs, up to 15 cm on rigs the refit leaves alone): never further
            // from the high head's than bone to bone put it.
            assert!(
                *top <= plain + 0.005,
                "{high}: head top {top} against {plain}"
            );
        }
        for body in [
            "all_inf_marksperson",
            "all_inf_markspersonsnow",
            "all_inf_deathstarleia",
        ] {
            let (_, _, top, plain) = moved_rigs.iter().find(|r| r.0 == body).unwrap();
            assert!(
                *top < 0.03 && *plain > 0.2,
                "{body}: {top} (bone to bone {plain})"
            );
        }
    }

    /// A low model that cannot be read, here because its class names one this
    /// LVL does not hold, leaves the soldier at full detail and the bake running.
    #[test]
    #[ignore = "requires the extracted PS2 discs"]
    fn an_unreadable_low_model_does_not_stop_the_bake() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE");
        let read = |lvl: &str| {
            std::fs::read(crate::resolve_ci(root.join(lvl).to_str().unwrap()))
                .expect("real PS2 side LVL required")
        };
        let (rep, cis) = (read("REP.LVL"), read("CIS.LVL"));
        let inner = mungefs::open(&rep).unwrap();
        let (model, skeleton) = super::find_soldier(inner, "rep_inf_trooper").unwrap();
        let elsewhere = mungefs::open(&cis).unwrap();
        let dir = std::env::temp_dir().join(format!("openbf-lod-skip-{}", std::process::id()));
        assert!(super::bake(elsewhere, &rep, &model, &skeleton, &dir, None).is_err());
        let baked = super::bake_or_skip(elsewhere, &rep, &model, &skeleton, &dir, None);
        assert!(baked.is_empty());
        assert!(!dir.join("soldiers/rep_inf_trooper.lod.ron").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore = "requires the extracted PS2 discs"]
    fn real_low_models_share_the_high_rig() {
        for game in ["bf1", "bf2"] {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../extracted")
                .join(game);
            let path = crate::resolve_ci(
                root.join("lvl/DATA/_LVL_PS2/SIDE/REP.LVL")
                    .to_str()
                    .unwrap(),
            );
            let bytes = std::fs::read(&path).expect("real PS2 REP.LVL required");
            let inner = mungefs::open(&bytes).unwrap();
            let geometry = if game == "bf1" {
                "rep_inf_trooper"
            } else {
                "rep_inf_ep3trooper"
            };
            let (model, skeleton) = super::find_soldier(inner, geometry).unwrap();
            let dir = std::env::temp_dir()
                .join(format!("openbf-soldier-lod-{game}-{}", std::process::id()));
            super::bake(inner, &bytes, &model, &skeleton, &dir, None).unwrap();
            let meta: manifest::SoldierLod = ron::from_str(
                &std::fs::read_to_string(dir.join(format!("soldiers/{geometry}.lod.ron"))).unwrap(),
            )
            .unwrap();
            assert!(meta.geometry.contains("low1"));
            assert!(meta.high_count > 2000);
            eprintln!(
                "{game} {geometry}: {} strip indices, sphere {:?}, low {}",
                meta.high_count, meta.sphere, meta.geometry
            );
            let (doc, _, _) =
                gltf::import(dir.join(format!("soldiers/{geometry}.low.glb"))).unwrap();
            assert_eq!(
                doc.skins().next().unwrap().joints().count(),
                skeleton.bones.len()
            );
            assert_eq!(doc.animations().count(), 0);
            assert_eq!(
                doc.meshes().flat_map(|m| m.primitives()).count(),
                1,
                "compatible low segments share a draw"
            );
            let low_vertices: usize = doc
                .meshes()
                .flat_map(|m| m.primitives())
                .map(|p| p.get(&gltf::Semantic::Positions).unwrap().count())
                .sum();
            assert!(
                low_vertices
                    < model
                        .segments
                        .iter()
                        .map(|s| s.positions.len())
                        .sum::<usize>()
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
