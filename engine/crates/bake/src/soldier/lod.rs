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
    let transforms: Vec<_> = remap
        .iter()
        .enumerate()
        .map(|(j, &high)| high_bind[high] * low_bind[j].inverse())
        .collect();
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
