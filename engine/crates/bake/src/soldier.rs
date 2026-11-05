//! Bake a skinned soldier (skel + bone-space modl) from a SIDE LVL into a
//! model-space skinned GLB (`skin_glb`) plus its textures as KTX2:
//! `<outdir>/soldiers/<model>.glb`, `<outdir>/textures/<tex>.ktx2`, with its
//! clips' `<model>.anims.ron` and its animation sets' `<model>.sets.ron`
//! (`crate::anim_sets`). A BF2 model's worn cloth is baked as extra segments
//! (`crate::cloth`). A model whose classes name their own skeleton
//! (`SkeletonName`) has its clips posed on that rig's bone lengths with
//! bone_root x `SkeletonRootScale` ([`PoseRig`]), and its classes' override
//! textures are baked beside it with `<model>.skins.ron` ([`ModelSkins`];
//! card USKN, `analysis/re-soldier-skeleton-skins.md`).
//! Vertices are moved from bone bind space to model space at bake time
//! (`mungefs::model::to_model_space`), so the runtime uses standard
//! `joint_global × inverse_bind` skinning (analysis/re-softskin.md §5).
//!
//! **ALPHA RULE v2 does not reach this path, and the soldiers follow the
//! disc's own rule instead** (design D6, answered for them by queue SKA:
//! `analysis/re-side-alpha.md`, `[O]`). A soldier segment's material carries
//! the GS state the disc gives that segment ([`segment_blends`]): `BLEND` when
//! its `MTRL` word has 0x04, and no `alphaMode` — glTF's OPAQUE — otherwise,
//! because the disc alpha-tests no side model. The TEXTURES are still decided
//! per texture: the side bake ORs the material flags of every segment naming
//! one and force-opaques those no material marks (`bake_textures_flagged`
//! below) — the disc's "the alpha is ignored", in the only place the port can
//! say it (§6.4) — except a sheet a specular-pass segment draws, whose alpha
//! is the pass's gloss weight and is kept (queue DSHN, `bake_flags`). The WEAPON GLBs carry that per-texture OR as their
//! `alphaMode` (`crate::alpha::legacy_per_texture_or`); no baked weapon
//! segment has 0x02 or 0x04 (§1), so every weapon draws OPAQUE.

#[cfg(test)]
use crate::anim_sets;
#[cfg(test)]
use crate::skin_glb::skinned_to_glb;
use crate::skin_glb::{Bone, ClipTrack, SkinnedModel, SkinnedSegment};
use anyhow::{anyhow, bail, Context, Result};
use glam::{Mat3, Quat, Vec3};
use mungefs::anim::{crc32_lower, AnimBank, DecodedClip};
use mungefs::model::Model;
use mungefs::skel::Skeleton;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::path::Path;

mod lod;
mod output;
mod render_clips;
pub(crate) use output::bake_flags;
pub use output::{
    bake_soldier, bake_soldier_anims, bake_textures_flagged, bake_weapon, texture_flags,
    weapon_nodes, BakedWeapon,
};

pub struct BakedSoldier {
    pub name: String,
    pub clips: usize,
    pub missing_clips: Vec<String>,
    pub bones: usize,
    pub vertices: usize,
    pub triangles: usize,
    pub textures: Vec<String>,
    /// The cloths baked as extra segments (BF2, `crate::cloth`).
    pub cloths: Vec<String>,
    /// Model-space AABB over every baked segment, worn cloth included (so
    /// wider than the modl vis box on a BF2 cloth wearer).
    pub aabb: ([f32; 3], [f32; 3]),
}

/// Find the modl + skel pair named `model` in an LVL.
pub fn find_soldier(inner: &[u8], model: &str) -> Result<(Model, Skeleton)> {
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    let modl = chunks
        .iter()
        .filter(|c| &c.tag == b"modl")
        .filter_map(|c| Model::parse(c).ok())
        .find(|m| m.name.eq_ignore_ascii_case(model))
        .ok_or_else(|| anyhow!("modl {model} not found"))?;
    let skel = chunks
        .iter()
        .filter(|c| &c.tag == b"skel")
        .filter_map(|c| mungefs::skel::parse_skel(c).ok())
        .find(|s| s.name.eq_ignore_ascii_case(model))
        .ok_or_else(|| anyhow!("skel {model} not found"))?;
    Ok((modl, skel))
}

/// Skeleton bones for the GLB: local bind rotation/translation. XFRM rows
/// are the row-vector rotation R (`v' = v·R + t`); the column-vector form is
/// Rᵀ, whose columns are the rows r0, r1, r2.
pub fn bones_of(skel: &Skeleton) -> Vec<Bone> {
    skel.bones
        .iter()
        .map(|b| {
            let m = Mat3::from_cols(
                Vec3::from_array(b.rows[0]),
                Vec3::from_array(b.rows[1]),
                Vec3::from_array(b.rows[2]),
            );
            Bone {
                name: b.name.clone(),
                parent: b.parent,
                translation: b.t,
                rotation: Quat::from_mat3(&m).normalize().to_array(),
            }
        })
        .collect()
}

pub fn to_skinned_model(m: &Model, skel: &Skeleton) -> Result<SkinnedModel> {
    if !m.bone_space {
        bail!("{} is not a bone-space (skinned) model", m.name);
    }
    rigid_or_skinned_model(m, skel)
}

pub fn rigid_or_skinned_model(m: &Model, skel: &Skeleton) -> Result<SkinnedModel> {
    model_from_rigs(m, skel, skel, false)
}

pub(crate) fn skinned_model_from_donor(
    m: &Model,
    canonical: &Skeleton,
    donor: &Skeleton,
) -> Result<SkinnedModel> {
    model_from_rigs(m, canonical, donor, true)
}

fn model_from_rigs(
    m: &Model,
    canonical: &Skeleton,
    donor: &Skeleton,
    remap: bool,
) -> Result<SkinnedModel> {
    let mut segments = Vec::new();
    for (k, seg) in m.segments.iter().enumerate() {
        if seg.triangles.is_empty() {
            continue;
        }
        let multiple = !m.bone_space
            && seg.bone.is_none()
            && seg.wght.iter().any(|w| w[1..].iter().any(|&w| w != 0));
        let (positions, normals, joints, weights) = if multiple {
            donor.bind_matrices().map_err(|e| anyhow!("{e}"))?;
            let mut joints = Vec::with_capacity(seg.positions.len());
            let mut weights = Vec::with_capacity(seg.positions.len());
            for v in 0..seg.positions.len() {
                let bind = seg
                    .bind
                    .get(v)
                    .context("missing model-space skin binding")?;
                let weight = seg.wght.get(v).context("missing model-space skin weight")?;
                let (mut joint, weight) =
                    model_space_influences(&seg.bmap, *bind, *weight, donor.bones.len())
                        .with_context(|| format!("{} segment {k} vertex {v}", m.name))?;
                if remap {
                    let mut indices = joint.map(usize::from);
                    crate::bind_pose::remap_bone_indices(donor, canonical, &mut indices)
                        .map_err(|e| anyhow!(e))?;
                    if indices.iter().any(|&b| b > 255) {
                        bail!("{}: more than 256 bones", m.name);
                    }
                    joint = indices.map(|b| b as u8);
                }
                joints.push(joint);
                weights.push(weight);
            }
            (seg.positions.clone(), seg.normals.clone(), joints, weights)
        } else {
            // Preserve the existing rigid and bone-space conversion byte for
            // byte. Retail models use one influence per vertex.
            let (positions, normals, mut bones) =
                mungefs::model::to_model_space(m, seg, donor).map_err(|e| anyhow!("{e}"))?;
            if remap {
                crate::bind_pose::remap_bone_indices(donor, canonical, &mut bones)
                    .map_err(|e| anyhow!(e))?;
            }
            if bones.iter().any(|&b| b > 255) {
                bail!("{}: more than 256 bones", m.name);
            }
            let joints = bones.iter().map(|&b| [b as u8, 0, 0, 0]).collect();
            let weights = vec![[1.0, 0.0, 0.0, 0.0]; bones.len()];
            (positions, normals, joints, weights)
        };
        let texture = crate::alpha::segment_texture(seg)
            .unwrap_or_default()
            .to_string();
        let blend = segment_blends(seg.material_flags)
            .with_context(|| format!("{} segment {k}", m.name))?;
        segments.push(SkinnedSegment {
            positions,
            normals,
            uvs: seg.uvs.clone(),
            joints,
            weights,
            triangles: seg.triangles.clone(),
            texture,
            blend,
            spec: crate::spec::spec_pass(seg),
            cloth: false,
        });
    }
    Ok(SkinnedModel {
        name: m.name.clone(),
        bones: bones_of(canonical),
        segments,
    })
}

/// Model-space addon skins already hold the common rest position, so all
/// authored influences can use standard GLB skinning. BIND stores VU slots
/// times four; inactive slots may contain padding and must never be indexed.
fn model_space_influences(
    bmap: &[u8],
    bind: [u8; 4],
    raw: [u8; 4],
    bone_count: usize,
) -> Result<([u8; 4], [f32; 4])> {
    let sum: u16 = raw.iter().map(|&w| u16::from(w)).sum();
    if sum == 0 {
        bail!("model-space skin has zero total weight");
    }
    let mut joints = [0; 4];
    let mut weights = [0.0; 4];
    for i in 0..4 {
        if raw[i] == 0 {
            continue;
        }
        let bone = *bmap
            .get(usize::from(bind[i] >> 2))
            .context("skin slot outside BMAP")?;
        if usize::from(bone) >= bone_count {
            bail!("skin bone {bone} outside skeleton");
        }
        joints[i] = bone;
        weights[i] = f32::from(raw[i]) / f32::from(sum);
    }
    Ok((joints, weights))
}

/// `MTRL` bits that change how a 0x04 segment composites on one disc or the
/// other (`analysis/re-material-flags.md` §2): 0x08 = BF2 `ALPHA 0x64`, a
/// fixed alpha; 0x40 = BF2 `ALPHA 0x48`, additive; 0x80 = BF1 `ALPHA 0x48`,
/// and in BF2 a second, additive specular pass.
const BLEND_MODIFIERS: u32 = 0x08 | 0x40 | 0x80;

/// Does a side model's segment COMPOSITE (`true`) or draw OPAQUE with its
/// sheet's alpha ignored (`false`)? The disc's own per-segment rule, read from
/// the segment's `MTRL` word alone — both ELFs pick a model draw's GS state
/// from that word and nothing else, a skinned draw and a rigid one alike
/// (`analysis/re-side-alpha.md` §3–§4 [V][O]):
///   * 0x04 → `ALPHA 0x44` = `Cs·As + Cd·(1−As)`, `As` = the sheet's alpha,
///     and `TEST 0x51001` — Z-tested, no Z write;
///   * neither 0x04 nor 0x02 → `TEST 0x50000`, no blend.
///
/// Those are the only two states the skinned path draws, so a word asking for
/// another is REFUSED rather than drawn wrong: 0x02 is the alpha TEST
/// (`0x5000b` BF2, `0x5100b` BF1) and a 0x04 word with a [`BLEND_MODIFIERS`]
/// bit composites some other way. None of the 52 soldiers the port bakes
/// carries either — zero 0x02 on both discs, and one 0x04 segment per game,
/// `all_inf_wookie`'s fur, words `0x5` / `0x10005` (§1's census) — but P7's
/// vehicles and the hero units do (§1), and they will need that pass first.
pub fn segment_blends(flags: u32) -> Result<bool> {
    use crate::detail::{CUTOUT, TRANSPARENT};
    if flags & CUTOUT != 0 {
        bail!("MTRL {flags:#x} has the cutout bit 0x02: the disc alpha-tests this segment, and the skinned path has no alpha test");
    }
    if flags & TRANSPARENT != 0 && flags & BLEND_MODIFIERS != 0 {
        bail!("MTRL {flags:#x} composites with a blend other than ALPHA 0x44 (bits {BLEND_MODIFIERS:#x}), which the skinned BLEND pass does not draw");
    }
    Ok(flags & TRANSPARENT != 0)
}

/// The clips `game::animator` plays today, baked into every soldier GLB under
/// the disc's own name (BF1 `human_rifle_*`; BF2 ships the same names with a
/// `_full` suffix on full-body clips, which `game::animator::find_clip`
/// tries). Every clip of the soldier's animation sets is baked beside them
/// (`crate::anim_sets`, card ANIMS), which `game::animator` plays.
pub const SOLDIER_CLIPS: &[&str] = &[
    "human_rifle_stand_idle_emote",
    "human_rifle_stand_walkforward",
    "human_rifle_stand_runforward",
    "human_rifle_stand_walkbackward",
    "human_rifle_stand_runbackward",
    "human_rifle_stand_turnleft",
    "human_rifle_stand_turnright",
    "human_rifle_stand_shoot",
    "human_rifle_stand_reload",
    "human_rifle_crouch_idle_emote",
    "human_rifle_crouch_walkforward",
    "human_rifle_crouch_walkbackward",
    "human_rifle_crouch_shoot",
    "human_rifle_crouch_reload",
    "human_rifle_prone_idle_emote",
    "human_rifle_prone_walkforward",
    "human_rifle_prone_shoot",
    "human_rifle_prone_reload",
    "human_rifle_diveforward",
    "human_rifle_jump",
    "human_rifle_fall",
    "human_rifle_landsoft",
    "human_rifle_landhard",
    "human_rifle_stand_getdown_prone",
    "human_rifle_crouch_getdown_prone",
    "human_rifle_prone_getup_stand",
    "human_rifle_prone_getup_crouch",
    "human_rifle_stand_getupfront",
    "human_rifle_stand_getupback",
    "human_rifle_die",
    "human_rifle_stand_hitfront",
    "human_rifle_stand_hitback",
    "human_rifle_stand_hitleft",
    "human_rifle_stand_hitright",
    "human_rifle_stand_death_forward",
    "human_rifle_stand_death_backward",
    "human_rifle_stand_death_left",
    "human_rifle_stand_death_right",
];

/// The seat clips a building turret's gunner is posed with (card VTURG,
/// `analysis/re-turret-draw.md` 3.3 [O]): `human_<value>` of the
/// `Pilot9Pose` / `PilotAnimation` keys the armed buildings author - nine
/// frames, nine frames, one. BF1 keeps them in `human_animation_1`
/// (COMMON.LVL), BF2 in `human_4` (INGAME.LVL). Then the drawn vehicle
/// riders' (card VRIDE, `analysis/re-vehicle-riders.md` 1, 6): the speeder
/// bike's and the BARC's nine-poses, the STAP's, the tauntaun's / kaadu's
/// `ride`, the skiff pilot's, the gunship passenger's and BF2's one-man
/// AT-ST's. A game whose banks lack one lists it in `missing`.
pub const SEAT_CLIPS: &[&str] = &[
    "human_minigun_9pose",
    "human_dishcannon_9pose",
    "human_drive",
    "human_speederbike_9pose",
    "human_stap_ride",
    "human_ride",
    "human_man_minigun",
    "human_standing",
    "human_barc_9pose",
    "human_drive_1manatst",
];

/// Per-clip facts the runtime needs beside the GLB animation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ClipMeta {
    pub name: String,
    pub frames: u32,
    /// Bank the clip came from.
    pub bank: String,
    /// DummyRoot displacement over the clip (x, y, z; metres) — stripped from
    /// the baked keys so playback is in place; z is the forward run/walk.
    pub root_motion: [f32; 3],
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SoldierAnims {
    pub clips: Vec<ClipMeta>,
    pub missing: Vec<String>,
}

/// Find `name` (or `name_full`) across the banks: the disc's name of the
/// clip, its bank and its index there.
fn find_clip<'a, 'b>(
    banks: &[&'a AnimBank<'b>],
    name: &str,
) -> Option<(String, &'a AnimBank<'b>, usize)> {
    for suffix in ["", "_full"] {
        let full = format!("{name}{suffix}");
        for b in banks {
            if let Some(k) = b.clip_index(&full) {
                return Some((full, b, k));
            }
        }
    }
    None
}

/// BF1 `FUN_0022f428`'s list (strings 0x5352a0-0x535400): the body bones a
/// class with its own `SkeletonName` poses at the rig's rest translation,
/// not the clip's (`analysis/re-soldier-skeleton-skins.md` §2 [O]).
const BF1_RIG_BONES: [&str; 22] = [
    "bone_pelvis",
    "bone_a_spine",
    "bone_b_spine",
    "bone_ribcage",
    "bone_neck",
    "bone_head",
    "bone_l_clavicle",
    "bone_l_upperarm",
    "bone_l_forearm",
    "bone_l_hand",
    "bone_r_clavicle",
    "bone_r_upperarm",
    "bone_r_forearm",
    "bone_r_hand",
    "bone_l_thigh",
    "bone_l_calf",
    "bone_l_foot",
    "bone_l_toe",
    "bone_r_thigh",
    "bone_r_calf",
    "bone_r_foot",
    "bone_r_toe",
];

/// The skeleton a soldier model is posed on (card USKN,
/// `analysis/re-soldier-skeleton-skins.md` §1-§3 [O]): the `zaf_` rig its
/// classes' `SkeletonName` names, with `SkeletonRootScale` on `bone_root`.
/// The PS2 keeps the flags in the rig, and one rig and one scale per soldier
/// model hold on both discs (§4, `census_model_rig.txt`), so the bake poses
/// per model.
#[derive(Debug, Clone)]
pub struct PoseRig {
    pub rig: mungefs::anim::Rig,
    /// bone_root's clip translation × this (BF1 `FUN_00441ca8`, BF2
    /// `FUN_00535658`: the bone's +0x68, `VMULx.xyz` in the pose builder).
    pub root_scale: f32,
    /// BF2's bone rule (`FUN_00281e68`) rather than BF1's (`FUN_0022f428`).
    pub bf2: bool,
}

impl PoseRig {
    /// Does `bone` take the rig's rest translation instead of the clip's?
    /// BF1: the 22 named body bones. BF2: every bone but bone 0 (DummyRoot),
    /// with bone_root and hp_weapons set back (0x620a90); a rig with
    /// `bone_r_finger3` (the Geonosian) sets back bone_root only (0x620ac0).
    /// The droideka's list (0x620aa0, a rig with `bone_abdomen`) is not
    /// built: the port fields no droideka soldier ([`pose_rig`] refuses it).
    pub fn takes_rest(&self, bone: &str) -> bool {
        if !self.bf2 {
            return BF1_RIG_BONES.iter().any(|b| b.eq_ignore_ascii_case(bone));
        }
        let geonosian = self.rig.joint(crc32_lower("bone_r_finger3")).is_some();
        !(bone.eq_ignore_ascii_case("DummyRoot")
            || bone.eq_ignore_ascii_case("bone_root")
            || (!geonosian && bone.eq_ignore_ascii_case("hp_weapons")))
    }
}

/// Mesh skeletons may omit an unskinned attachment joint. The animation rig
/// still owns that joint (Yoda's hp_weapons); append it without changing the
/// existing skin joint indices.
pub(crate) fn append_rig_attachment(
    skel: &mut Skeleton,
    rig: &mungefs::anim::Rig,
    name: &str,
) -> Result<bool> {
    let crc = crc32_lower(name);
    if skel.index_of(name).is_some() {
        return Ok(false);
    }
    let Some(joint) = rig.joint(crc) else {
        return Ok(false);
    };
    let parent = if joint.parent_crc == 0 {
        None
    } else {
        Some(
            skel.bones
                .iter()
                .position(|b| b.crc == joint.parent_crc)
                .ok_or_else(|| {
                    anyhow!(
                        "{}: rig attachment {name} has a parent absent from the model",
                        skel.name
                    )
                })?,
        )
    };
    let rotation = Mat3::from_quat(Quat::from_array(joint.rest_rot).normalize());
    skel.bones.push(mungefs::skel::SkelBone {
        name: name.to_owned(),
        crc,
        parent,
        rows: rotation.to_cols_array_2d(),
        t: joint.rest_pos,
    });
    Ok(true)
}

/// [`PoseRig`] of `model` from the level's soldier classes: the first class
/// (in `classes` order) whose `GeometryName` is the model and whose chain
/// names a `SkeletonName`. `None` = the human rig, whose rest is the human
/// clips' own base, so the rule changes nothing there (§2) - also for a
/// `SkeletonName` of the default `human` rig or a rig no LVL carries.
/// `SkeletonRootScale` counts only with an own rig (BF1 123186-123194, BF2
/// 172070-172080).
pub fn pose_rig(
    classes: &[manifest::ClassDef],
    model: &str,
    rigs: &[mungefs::anim::Rig],
    bf2: bool,
) -> Option<PoseRig> {
    use crate::classes::ClassProps;
    let entc: std::collections::HashMap<String, &manifest::ClassDef> = classes
        .iter()
        .filter(|c| c.kind == "entc")
        .rev() // the first definition of a name wins
        .map(|c| (c.name.to_ascii_lowercase(), c))
        .collect();
    for c in classes.iter().filter(|c| c.kind == "entc") {
        let mut chain = vec![c];
        while let Some(p) = entc.get(&chain[chain.len() - 1].base.to_ascii_lowercase()) {
            if chain.len() > 16 {
                break;
            }
            chain.push(p);
        }
        let first = |key: &str| chain.iter().find_map(|d| d.prop(key)).map(str::trim);
        if !first("GeometryName").is_some_and(|g| g.eq_ignore_ascii_case(model)) {
            continue;
        }
        let Some(name) = first("SkeletonName").and_then(|v| v.split_whitespace().next()) else {
            continue;
        };
        if name.to_ascii_lowercase().starts_with("human") {
            return None;
        }
        let rig = rigs.iter().find(|r| r.name.eq_ignore_ascii_case(name))?;
        if rig.joint(crc32_lower("bone_abdomen")).is_some() {
            return None; // the droideka's list: not built (see `takes_rest`)
        }
        let root_scale = first("SkeletonRootScale")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(1.0);
        return Some(PoseRig {
            rig: rig.clone(),
            root_scale,
            bf2,
        });
    }
    None
}

/// Turn a decoded clip into GLB tracks on the model skeleton: tracks join
/// bones by CRC; DummyRoot translation is stripped (returned as root motion).
/// With a [`PoseRig`] the PS2's pose rule is applied (card USKN): the bones
/// [`PoseRig::takes_rest`] lists hold the rig's rest translation in every
/// frame (a listed bone the clip does not animate gets a track at the
/// model's bind rotation), and bone_root's translation is × the root scale.
pub fn clip_to_tracks(
    clip: &DecodedClip,
    skel: &Skeleton,
    pose: Option<&PoseRig>,
) -> (Vec<ClipTrack>, [f32; 3]) {
    let dummy = crc32_lower("DummyRoot");
    let root = crc32_lower("bone_root");
    let mut root_motion = [0.0f32; 3];
    let mut tracks = Vec::new();
    let frames = usize::from(clip.frames);
    for t in &clip.tracks {
        let Some(bone) = skel.bones.iter().position(|b| b.crc == t.bone_crc) else {
            continue; // rig helper joints the model does not have
        };
        let mut translations = t.translations.clone();
        if t.bone_crc == dummy {
            if let (Some(a), Some(z)) = (translations.first(), translations.last()) {
                root_motion = [z[0] - a[0], z[1] - a[1], z[2] - a[2]];
            }
            let rest = skel.bones[bone].t;
            translations = vec![rest; translations.len()];
        }
        tracks.push(ClipTrack {
            bone,
            rotations: t.rotations.clone(),
            translations,
        });
    }
    if let Some(p) = pose {
        let binds = bones_of(skel);
        for (bone, b) in skel.bones.iter().enumerate() {
            let k = match tracks.iter().position(|t| t.bone == bone) {
                Some(k) => k,
                None if p.takes_rest(&b.name) && p.rig.joint(b.crc).is_some() => {
                    tracks.push(ClipTrack {
                        bone,
                        rotations: vec![binds[bone].rotation; frames],
                        translations: vec![b.t; frames],
                    });
                    tracks.len() - 1
                }
                None => continue,
            };
            let t = &mut tracks[k];
            if b.crc == root {
                for v in &mut t.translations {
                    *v = v.map(|x| x * p.root_scale);
                }
            } else if p.takes_rest(&b.name) {
                if let Some(j) = p.rig.joint(b.crc) {
                    t.translations.iter_mut().for_each(|v| *v = j.rest_pos);
                }
            }
        }
    }
    (tracks, root_motion)
}

/// A soldier model's override groups and the override textures its classes
/// name (card USKN, `analysis/re-soldier-skeleton-skins.md` 9 [O]).
#[derive(Debug, Default, PartialEq)]
pub struct ModelSkins {
    /// Group (`MNAM`: `override_texture` / `override_texture2`) -> the
    /// textures of the model's segments in it, lowercase. Written into
    /// `soldiers/<model>.skins.ron` ([`manifest::SoldierSkins`]): the runtime
    /// swaps a part whose texture is listed under a group for the class's
    /// override of that group.
    pub groups: std::collections::BTreeMap<String, Vec<String>>,
    /// (texture, group) for every override a class on this model names;
    /// `OverrideTexture2` only with `OverrideTexture` (both games' draw tests
    /// the second inside the first's branch).
    pub overrides: Vec<(String, String)>,
}

/// The [`ModelSkins`] of `m` from its side LVL's classes.
pub fn model_skins(m: &Model, classes: &[manifest::ClassDef]) -> ModelSkins {
    let mut out = ModelSkins::default();
    for seg in &m.segments {
        let Some(group) = seg.group.as_deref().map(str::to_ascii_lowercase) else {
            continue;
        };
        if !group.starts_with("override_texture") {
            continue;
        }
        let Some(tex) = crate::alpha::segment_texture(seg) else {
            continue;
        };
        let list = out.groups.entry(group).or_default();
        let tex = tex.to_ascii_lowercase();
        if !list.contains(&tex) {
            list.push(tex);
        }
    }
    for c in classes.iter().filter(|c| c.kind == "entc") {
        let skin = class_skin(classes, &c.name);
        if !skin.geometry.eq_ignore_ascii_case(&m.name) {
            continue;
        }
        for (tex, group) in [
            (skin.texture, "override_texture"),
            (skin.texture2, "override_texture2"),
        ] {
            let Some(tex) = tex else { continue };
            let row = (tex, group.to_owned());
            if !out.overrides.contains(&row) {
                out.overrides.push(row);
            }
        }
    }
    out
}

/// One class's model and override textures through its BASE chain (the
/// first value wins; lowercase; `texture2` only with `texture`).
#[derive(Debug, Default, PartialEq)]
pub struct ClassSkin {
    pub geometry: String,
    pub texture: Option<String>,
    pub texture2: Option<String>,
}

/// [`ClassSkin`] of class `name` among `classes` (`entc` records).
pub fn class_skin(classes: &[manifest::ClassDef], name: &str) -> ClassSkin {
    use crate::classes::ClassProps;
    let find = |n: &str| {
        classes
            .iter()
            .find(|c| c.kind == "entc" && c.name.eq_ignore_ascii_case(n))
    };
    let mut chain = Vec::new();
    let mut cur = find(name);
    while let Some(c) = cur {
        if chain.len() > 16 {
            break;
        }
        chain.push(c);
        cur = find(&c.base);
    }
    let first = |key: &str| {
        chain
            .iter()
            .find_map(|d| d.prop(key))
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| !v.is_empty())
    };
    let texture = first("OverrideTexture");
    ClassSkin {
        geometry: first("GeometryName").unwrap_or_default(),
        texture2: texture.as_ref().and(first("OverrideTexture2")),
        texture,
    }
}

#[cfg(test)]
mod tests;
