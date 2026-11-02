//! Kit Fisto's separately authored head tentacles, folded into his selective
//! body bake. The addon attaches this mesh to bone_head; no retail body uses
//! this path. The tentacles keep their own joints, renamed `tentacle_*` and
//! appended after the body's so every body joint index stays. No source holds
//! a clip for them, so no clip moves them: `game::skin` sways them.

use crate::skin_glb::{global_binds, Bone, SkinnedModel};
use anyhow::{ensure, Context, Result};
use glam::{Mat3, Quat, Vec3};

/// The pack's PS2 and PC body skeletons round bind matrices differently. Its
/// own animation rigs carry identical rest transforms across those formats.
/// Use those transforms for these two new bodies before physical saber tracks
/// are sampled, so input format cannot change gameplay geometry.
pub(crate) fn canonical_bind(inner: &[u8], body: &mut mungefs::skel::Skeleton) -> Result<()> {
    let rig_name = match body.name.as_str() {
        "rep_inf_kitfisto" => "fisto",
        "cis_inf_asajjventress" => "ventress",
        _ => return Ok(()),
    };
    let rigs = mungefs::anim::find_rigs(inner)?;
    let rig = rigs
        .iter()
        .find(|rig| rig.name == rig_name)
        .with_context(|| format!("{}: missing canonical {rig_name} rest rig", body.name))?;
    let parent_crcs: Vec<_> = body
        .bones
        .iter()
        .map(|bone| bone.parent.map_or(0, |parent| body.bones[parent].crc))
        .collect();
    for (bone, parent_crc) in body.bones.iter_mut().zip(parent_crcs) {
        let Some(joint) = rig.joint(bone.crc) else {
            // This camera attachment is not animated or sampled by saber
            // collision. Retain its source presentation transform.
            ensure!(
                bone.name == "hp_camera",
                "{}: {rig_name} rig has no {}",
                body.name,
                bone.name
            );
            continue;
        };
        ensure!(
            joint.parent_crc == parent_crc,
            "{}: {} parent differs from {rig_name} rig",
            body.name,
            bone.name
        );
        bone.rows =
            Mat3::from_quat(Quat::from_array(joint.rest_rot).normalize()).to_cols_array_2d();
        bone.t = joint.rest_pos;
    }
    Ok(())
}

pub(crate) fn append_fisto_tentacles(inner: &[u8], body: &mut SkinnedModel) -> Result<()> {
    if !body.name.eq_ignore_ascii_case("rep_inf_kitfisto") {
        return Ok(());
    }
    let head = body
        .bones
        .iter()
        .position(|bone| bone.name.eq_ignore_ascii_case("bone_head"))
        .context("Kit Fisto body has no bone_head for its tentacles")?;
    let bind = global_binds(&body.bones)[head];
    let (model, skeleton) = crate::soldier::find_soldier(inner, "rep_inf_kitfisto_tenticles")
        .context("Kit Fisto's separately attached tentacles are missing")?;
    // The donor's model space is its attachment space: its root hangs off the
    // head, and its positions move with that bind.
    let tentacles = crate::soldier::rigid_or_skinned_model(&model, &skeleton)?;
    let base = body.bones.len();
    ensure!(
        base + tentacles.bones.len() <= u8::MAX as usize,
        "Kit Fisto's tentacle joints exceed the joint index range"
    );
    body.bones
        .extend(tentacles.bones.into_iter().map(|bone| Bone {
            name: format!("tentacle_{}", bone.name),
            parent: Some(bone.parent.map_or(head, |parent| base + parent)),
            ..bone
        }));
    for mut segment in tentacles.segments {
        for position in &mut segment.positions {
            *position = bind
                .transform_point3(Vec3::from_array(*position))
                .to_array();
        }
        for normal in &mut segment.normals {
            *normal = bind
                .transform_vector3(Vec3::from_array(*normal))
                .normalize_or_zero()
                .to_array();
        }
        for (joints, weights) in segment.joints.iter_mut().zip(&segment.weights) {
            for (joint, weight) in joints.iter_mut().zip(weights) {
                *joint = if *weight > 0.0 {
                    *joint + base as u8
                } else {
                    0
                };
            }
        }
        body.segments.push(segment);
    }
    Ok(())
}
