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

mod output;
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
    let mut segments = Vec::new();
    for (k, seg) in m.segments.iter().enumerate() {
        if seg.triangles.is_empty() {
            continue;
        }
        let (positions, normals, bones) =
            mungefs::model::to_model_space(m, seg, skel).map_err(|e| anyhow!("{e}"))?;
        if bones.iter().any(|&b| b > 255) {
            bail!("{}: more than 256 bones", m.name);
        }
        let texture = crate::alpha::segment_texture(seg)
            .unwrap_or_default()
            .to_string();
        let blend = segment_blends(seg.material_flags)
            .with_context(|| format!("{} segment {k}", m.name))?;
        segments.push(SkinnedSegment {
            positions,
            normals,
            uvs: seg.uvs.clone(),
            joints: bones.iter().map(|&b| [b as u8, 0, 0, 0]).collect(),
            weights: vec![[1.0, 0.0, 0.0, 0.0]; bones.len()],
            triangles: seg.triangles.clone(),
            texture,
            blend,
            spec: crate::spec::spec_pass(seg),
            cloth: false,
        });
    }
    Ok(SkinnedModel {
        name: m.name.clone(),
        bones: bones_of(skel),
        segments,
    })
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
mod tests {
    use super::*;

    /// The two states a skinned segment can take, read off its `MTRL` word
    /// alone, and the words the skinned path refuses. The first two rows are
    /// every word a baked soldier segment carries on either disc
    /// (`analysis/re-side-alpha.md` §0 item 3).
    #[test]
    fn segment_blends_draws_two_states_and_refuses_the_rest() {
        for word in [0x1, 0x31, 0x1_0001, 0x1_0089, 0x1_0101, 0x1_0010] {
            assert!(!segment_blends(word).unwrap(), "{word:#x} is opaque");
        }
        for word in [0x5, 0x1_0005] {
            assert!(segment_blends(word).unwrap(), "{word:#x} composites");
        }
        // The cutout bit in any company: the disc alpha-TESTS it.
        for word in [0x2, 0x3, 0x6, 0x1_0003] {
            assert!(segment_blends(word).is_err(), "{word:#x} is refused");
        }
        // A transparent word with a bit that changes its blend.
        for word in [0xc, 0x44, 0x85, 0x1_008d] {
            assert!(segment_blends(word).is_err(), "{word:#x} is refused");
        }
        // Those bits are harmless on an opaque segment (0x1_0089 above).
        assert!(!segment_blends(0xc8).unwrap());
    }

    /// Data-gated, queue SKA: `all_inf_wookie` bakes with its FUR segment
    /// blended and its other two opaque, on both discs — BF1 segment 2 (word
    /// `0x5`, 261 vertices), BF2 segment 1 (`0x10005`, 288)
    /// (`analysis/re-side-alpha.md` §0 item 2, [O]) — and only that material
    /// states an `alphaMode` in the GLB. The verdict is per SEGMENT: the fur
    /// shares its sheet with an opaque segment in both games (BF1's third
    /// segment is the face, `wookieface02`; BF2's three all name one sheet).
    #[test]
    fn the_wookies_fur_is_its_only_blended_segment_on_both_discs() {
        let side = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        let cases = [
            ("bf1", ["wookieface02", "wookie", "wookie"], 2, 261),
            ("bf2", ["all_inf_wookie"; 3], 1, 288),
        ];
        for (game, sheets, fur, fur_verts) in cases {
            let all = format!("{side}/{game}/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL");
            if !std::path::Path::new(&all).exists() {
                eprintln!("SKIP: {all} not present");
                continue;
            }
            let buf = std::fs::read(&all).unwrap();
            let (m, skel) = find_soldier(mungefs::open(&buf).unwrap(), "all_inf_wookie").unwrap();
            let sm = to_skinned_model(&m, &skel).unwrap();
            let blends: Vec<bool> = sm.segments.iter().map(|s| s.blend).collect();
            let expected: Vec<bool> = (0..3).map(|k| k == fur).collect();
            assert_eq!(blends, expected, "{game}: only the fur composites");
            assert_eq!(
                sm.segments[fur].positions.len(),
                fur_verts,
                "{game}: the fur segment"
            );
            let named: Vec<&str> = sm.segments.iter().map(|s| s.texture.as_str()).collect();
            assert_eq!(named, sheets, "{game}: the sheets the segments name");

            let glb = skinned_to_glb(&sm, &[]).unwrap();
            let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
            let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + len]).unwrap();
            let modes: Vec<Option<&str>> = doc["materials"]
                .as_array()
                .unwrap()
                .iter()
                .map(|mat| mat.get("alphaMode").and_then(|v| v.as_str()))
                .collect();
            let expected: Vec<Option<&str>> =
                (0..3).map(|k| (k == fur).then_some("BLEND")).collect();
            assert_eq!(modes, expected, "{game}: the GLB's materials");
        }
    }

    /// World position of `bone` at frame 0 of `tracks` (DummyRoot at its
    /// rest): the skinned draw's forward kinematics.
    fn fk_frame0(skel: &Skeleton, tracks: &[ClipTrack], bone: &str) -> Vec3 {
        let binds = bones_of(skel);
        let local = |i: usize| {
            let (mut t, mut r) = (binds[i].translation, binds[i].rotation);
            if let Some(k) = tracks.iter().find(|k| k.bone == i) {
                (t, r) = (k.translations[0], k.rotations[0]);
            }
            glam::Mat4::from_rotation_translation(Quat::from_array(r), Vec3::from_array(t))
        };
        let mut i = skel.bones.iter().position(|b| b.name == bone).unwrap();
        let mut m = local(i);
        while let Some(p) = skel.bones[i].parent {
            m = local(p) * m;
            i = p;
        }
        m.w_axis.truncate()
    }

    /// Card USKN acceptance 1 (`analysis/re-soldier-skeleton-skins.md` §2-§4,
    /// §10): an own-skeleton class is posed on its rig's bone lengths with
    /// bone_root x SkeletonRootScale, the idle frame 0 head at the PS2 rule's
    /// height (`lead_tmp/uskn/fk_idle.txt`); the clone trooper (human rig) is
    /// unchanged; the listed bones hold the rig's JONT rest in every frame.
    /// Control: without the rule every model's head is the human 1.605 / 1.643.
    #[test]
    fn own_rig_classes_pose_at_their_rig_lengths() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        let b1 = format!("{root}/bf1/lvl/DATA/_LVL_PS2");
        let b2 = format!("{root}/bf2/lvl/DATA/_LVL_PS2");
        let a1 = format!("{b1}/COMMON/ENG/COMMON.LVL");
        let a2 = format!("{b2}/INGAME.LVL");
        if !Path::new(&a1).exists() || !Path::new(&a2).exists() {
            eprintln!("SKIP: COMMON.LVL / INGAME.LVL not present");
            return;
        }
        // (game, side, model, idle clip, rig, scale, PS2 head y, human head y)
        let rows = [
            (
                1,
                "ALL",
                "all_inf_wookie",
                "all_inf_wookie",
                1.15,
                1.852,
                1.605,
            ),
            (
                1,
                "CIS",
                "cis_inf_bdroid",
                "cis_inf_bdroid",
                1.0,
                1.726,
                1.605,
            ),
            (1, "REP", "rep_inf_trooper", "", 1.0, 1.605, 1.605),
            (
                2,
                "CIS",
                "cis_inf_magnaguard",
                "magnaguard",
                1.24,
                1.865,
                1.643,
            ),
            (2, "CIS", "cis_inf_bdroid", "bdroid", 1.0, 1.758, 1.643),
            (2, "ALL", "all_inf_wookie", "wookie", 1.15, 1.892, 1.643),
        ];
        let abufs = [std::fs::read(&a1).unwrap(), std::fs::read(&a2).unwrap()];
        for (game, side, model, rig, scale, ps2, human) in rows {
            let (dir, abuf) = if game == 1 {
                (&b1, &abufs[0])
            } else {
                (&b2, &abufs[1])
            };
            let buf = std::fs::read(format!("{dir}/SIDE/{side}.LVL")).unwrap();
            let inner = mungefs::open(&buf).unwrap();
            let ainner = mungefs::open(abuf).unwrap();
            let (_, skel) = find_soldier(inner, model).unwrap();
            let mut rigs = mungefs::anim::find_rigs(inner).unwrap();
            rigs.extend(mungefs::anim::find_rigs(ainner).unwrap());
            let classes = anim_sets::bf2::side_classes(&buf).unwrap();
            let pose = pose_rig(&classes, model, &rigs, game == 2);
            match &pose {
                None => assert_eq!(rig, "", "{model}: no pose rig"),
                Some(p) => {
                    assert!(
                        p.rig.name.eq_ignore_ascii_case(rig),
                        "{model}: {}",
                        p.rig.name
                    );
                    assert_eq!(p.root_scale, scale, "{model}");
                }
            }
            let banks = mungefs::anim::find_banks(ainner).unwrap();
            let clip = [
                "human_rifle_stand_idle_emote",
                "human_rifle_stand_idle_emote_full",
            ]
            .iter()
            .find_map(|n| {
                banks
                    .iter()
                    .find_map(|b| Some(b.decode_clip(b.clip_index(n)?)))
            })
            .unwrap()
            .unwrap();
            let (tracks, _) = clip_to_tracks(&clip, &skel, pose.as_ref());
            let head = fk_frame0(&skel, &tracks, "bone_head").y;
            assert!(
                (head - ps2).abs() < 0.01,
                "{model}: head {head} vs PS2 {ps2}"
            );
            let (plain, _) = clip_to_tracks(&clip, &skel, None);
            let head0 = fk_frame0(&skel, &plain, "bone_head").y;
            assert!(
                (head0 - human).abs() < 0.01,
                "{model}: control head {head0}"
            );
            let Some(p) = &pose else { continue };
            let mut listed = 0;
            for t in &tracks {
                let b = &skel.bones[t.bone];
                if !p.takes_rest(&b.name) {
                    continue;
                }
                let Some(j) = p.rig.joint(b.crc) else {
                    continue;
                };
                listed += 1;
                assert!(
                    t.translations.iter().all(|v| *v == j.rest_pos),
                    "{model} {}",
                    b.name
                );
            }
            assert!(listed >= 22, "{model}: {listed} bones on the rig's rest");
        }
    }

    /// Card USKN acceptance 3 (re-soldier-skeleton-skins 9): over the four
    /// soldier sides of both discs, every class with an `OverrideTexture`
    /// whose model is in its side LVL has that model's `override_texture`
    /// group to draw it on (and `override_texture2` for a second), and every
    /// override texture is in the side LVL for the bake to write; a second
    /// override without a first is never taken.
    #[test]
    fn every_class_override_has_its_group_and_its_texture() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        let mut checked = 0;
        let mut no_group = Vec::new();
        for game in ["bf1", "bf2"] {
            for side in ["ALL", "IMP", "REP", "CIS"] {
                let path = format!("{root}/{game}/lvl/DATA/_LVL_PS2/SIDE/{side}.LVL");
                let Ok(buf) = std::fs::read(&path) else {
                    eprintln!("SKIP: {path}");
                    return;
                };
                let inner = mungefs::open(&buf).unwrap();
                let classes = anim_sets::bf2::side_classes(&buf).unwrap();
                let chunks = mungefs::content_chunks(inner).unwrap();
                let texs: Vec<String> = chunks
                    .iter()
                    .filter(|c| &c.tag == b"tex_")
                    .filter_map(|c| mungefs::texture::Texture::parse(c).ok())
                    .map(|t| t.name.to_ascii_lowercase())
                    .collect();
                for c in classes.iter().filter(|c| c.kind == "entc") {
                    let skin = class_skin(&classes, &c.name);
                    let Some(tex) = &skin.texture else {
                        continue;
                    };
                    let Ok((m, _)) = find_soldier(inner, &skin.geometry) else {
                        continue; // not a soldier model of this side
                    };
                    let skins = model_skins(&m, &classes);
                    let has = |g: &str| skins.groups.get(g).is_some_and(|l| !l.is_empty());
                    if !has("override_texture") {
                        // The model has no such group: the PS2's override
                        // list finds nothing to replace.
                        no_group.push(format!("{game} {}", c.name));
                        continue;
                    }
                    assert!(texs.contains(tex), "{game} {}: {tex} not in {side}", c.name);
                    if let Some(t2) = &skin.texture2 {
                        assert!(has("override_texture2"), "{game} {}: {}", c.name, m.name);
                        assert!(texs.contains(t2), "{game} {}: {t2} not in {side}", c.name);
                    }
                    assert!(skins
                        .overrides
                        .contains(&(tex.clone(), "override_texture".into())));
                    checked += 1;
                }
            }
        }
        // 16 BF1 IA classes and 9 BF2 ones author one (unit-audit-bf1 /
        // bf2 3.3), plus the classes outside the IA lists.
        assert!(checked >= 25, "{checked} classes with an override");
        println!("{checked} classes draw an override; none to replace: {no_group:?}");
        // [D]: `all_inf_engineer` has no MNAM group, so the snow engineer's
        // `all_inf_engineer_snow` replaces nothing on the PS2 either.
        assert_eq!(no_group, ["bf2 all_inf_engineer_snow"]);
    }

    /// The second override counts only with the first (both draws test it
    /// inside the first's branch).
    #[test]
    fn a_second_override_alone_is_not_drawn() {
        let def = |name: &str, base: &str, props: &[(&str, &str)]| manifest::ClassDef {
            name: name.into(),
            kind: "entc".into(),
            base: base.into(),
            props: props
                .iter()
                .map(|(k, v)| (mungefs::fnv1a_lowercase(k), (*v).to_owned()))
                .collect(),
        };
        let classes = [
            def(
                "a",
                "soldier",
                &[("GeometryName", "m"), ("OverrideTexture2", "x2")],
            ),
            def("b", "a", &[("OverrideTexture", "X1")]),
        ];
        assert_eq!(class_skin(&classes, "a").texture2, None);
        let b = class_skin(&classes, "b");
        assert_eq!(
            (
                b.geometry.as_str(),
                b.texture.as_deref(),
                b.texture2.as_deref()
            ),
            ("m", Some("x1"), Some("x2"))
        );
    }

    /// re-softskin.md §8 acceptance: the `human` rig's rest pose (zaf_ JONT
    /// quats/positions) equals the model skel's bind pose bone for bone.
    #[test]
    fn human_rig_rest_matches_stormtrooper_bind() {
        let imp = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
        );
        let common = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL"
        );
        if !std::path::Path::new(imp).exists() || !std::path::Path::new(common).exists() {
            eprintln!("SKIP: IMP.LVL / COMMON.LVL not present");
            return;
        }
        let ibuf = std::fs::read(imp).unwrap();
        let (_, skel) =
            find_soldier(mungefs::open(&ibuf).unwrap(), "imp_inf_stormtrooper").unwrap();
        let cbuf = std::fs::read(common).unwrap();
        let rigs = mungefs::anim::find_rigs(mungefs::open(&cbuf).unwrap()).unwrap();
        let rig = rigs.iter().find(|r| r.name == "human_animation_0").unwrap();
        let bones = bones_of(&skel);
        let mut matched = 0;
        for (b, sb) in bones.iter().zip(&skel.bones) {
            let Some(j) = rig.joint(sb.crc) else { continue };
            matched += 1;
            let q = Quat::from_array(b.rotation);
            let r = Quat::from_array(j.rest_rot);
            assert!(
                q.dot(r).abs() > 0.999,
                "{}: model {q:?} vs rig {r:?}",
                sb.name
            );
            let dp = Vec3::from_array(b.translation) - Vec3::from_array(j.rest_pos);
            assert!(dp.length() < 1e-3, "{}: {dp:?}", sb.name);
        }
        assert_eq!(matched, 25, "every model bone has a rig joint");
    }

    /// Data-gated: the side bake writes BOTH tiers side by side, and the
    /// classic-plus install cannot reach the base file — the PS2 pixels are
    /// what `<name>.ktx2` holds whether or not a PC SIDE LVL was supplied,
    /// and the override sits beside it as `<name>.tier2.ktx2`.
    ///
    /// The super battle droid is the case with a PC counterpart AND a sheet a
    /// specular pass draws, so both files must carry the PS2's gloss mask.
    #[test]
    fn side_textures_write_both_tiers_and_the_base_is_always_ps2() {
        let cis = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL"
        );
        let pc_cis = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/side/cis.lvl"
        );
        if !std::path::Path::new(cis).exists() || !std::path::Path::new(pc_cis).exists() {
            eprintln!("SKIP: BF2 CIS.LVL or the PC side/cis.lvl is not present");
            return;
        }
        let root = std::env::temp_dir().join(format!("openbf_tier2_{}", std::process::id()));
        let (with_pc, ps2_only) = (root.join("with_pc"), root.join("ps2_only"));
        let _ = std::fs::remove_dir_all(&root);
        bake_soldier_anims(
            cis,
            "cis_inf_sbdroid",
            None,
            with_pc.to_str().unwrap(),
            Some(pc_cis),
        )
        .unwrap();
        bake_soldier(cis, "cis_inf_sbdroid", ps2_only.to_str().unwrap()).unwrap();

        let base = with_pc.join("textures/cis_inf_sbdroid.ktx2");
        let tier2 = with_pc.join("textures/cis_inf_sbdroid.tier2.ktx2");
        let dims = |p: &std::path::Path| {
            let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let r = ktx2::Reader::new(&bytes).unwrap();
            (r.header().pixel_width, r.header().pixel_height)
        };
        assert_eq!(
            dims(&base),
            (256, 256),
            "the base is the disc's own texture"
        );
        assert_eq!(dims(&tier2), (512, 512), "the PC override sits beside it");
        assert_eq!(
            std::fs::read(&base).unwrap(),
            std::fs::read(ps2_only.join("textures/cis_inf_sbdroid.ktx2")).unwrap(),
            "the PC install must not change one byte of the base file"
        );
        // The droid's sheet is drawn by a specular pass, so BOTH tiers carry
        // the PS2's gloss mask (queue DSHN: disc mean 34/255) — the override's
        // own alpha is a different channel (see `tier`), so it takes the PS2's.
        for p in [&base, &tier2] {
            let bytes = std::fs::read(p).unwrap();
            let r = ktx2::Reader::new(&bytes).unwrap();
            let level0 = r.levels().next().unwrap();
            let n = level0.data.len() / 4;
            let mean = level0
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|px| f64::from(px[3]))
                .sum::<f64>()
                / n as f64;
            assert!(
                (20.0..50.0).contains(&mean),
                "{}: the PS2 gloss mask, got alpha mean {mean:.1}",
                p.display()
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Data-gated regression, both halves of the droid's sheet. Its alpha is
    /// NOT a cutout: every texel is below the 0.5 alpha test the renderer
    /// once ran on every draw (the whole model went invisible), so the GLB
    /// material stays OPAQUE — no segment's word has 0x02 or 0x04. But it IS
    /// the specular pass's gloss weight (queue DSHN, `analysis/re-spec-gloss.md`
    /// §3: disc mean 34/255), so the baked sheet keeps it; at 255 the pass adds
    /// at full weight and the droid reads as chrome.
    #[test]
    fn a_pass_sheet_keeps_its_gloss_alpha_and_draws_opaque() {
        let cis = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL"
        );
        if !std::path::Path::new(cis).exists() {
            eprintln!("SKIP: CIS.LVL not present");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_alpha_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        bake_soldier(cis, "cis_inf_sbdroid", dir.to_str().unwrap()).unwrap();
        let bytes = std::fs::read(dir.join("textures/cis_inf_sbdroid.ktx2")).unwrap();
        let r = ktx2::Reader::new(&bytes).unwrap();
        let level0 = r.levels().next().unwrap();
        let alpha: Vec<u8> = level0
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[3])
            .collect();
        let mean = alpha.iter().map(|&a| f64::from(a)).sum::<f64>() / alpha.len() as f64;
        assert!(
            (20.0..50.0).contains(&mean),
            "the sheet keeps the disc's gloss mask (mean 34), got mean {mean:.1}"
        );
        let glb = std::fs::read(dir.join("soldiers/cis_inf_sbdroid.glb")).unwrap();
        assert!(
            !glb.windows(9).any(|w| w == b"alphaMode"),
            "the droid's materials stay OPAQUE"
        );
        // And the flag words really are what decide it: no cutout or
        // transparent material, and a specular pass on the sheet.
        let buf = std::fs::read(cis).unwrap();
        let inner = mungefs::open(&buf).unwrap();
        let m = mungefs::content_chunks(inner)
            .unwrap()
            .iter()
            .filter(|c| &c.tag == b"modl")
            .map(|c| mungefs::model::Model::parse(c).unwrap())
            .find(|m| m.name == "cis_inf_sbdroid")
            .expect("droid model");
        assert!(
            m.segments
                .iter()
                .all(|s| s.material_flags & crate::detail::ALPHA_IS_MEANINGFUL == 0),
            "droid segments claim no cutout/transparent material"
        );
        assert!(crate::spec::pass_textures([&m]).contains("cis_inf_sbdroid"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Data-gated: a BF1 base texture with detailCompressed gets its `_dtl`
    /// companion baked as `<base>.dtl.ktx2` at 4x the base size, and the
    /// companion does not also appear under its own source name.
    #[test]
    fn bf1_soldier_textures_get_detail_companions() {
        let all = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL"
        );
        if !std::path::Path::new(all).exists() {
            eprintln!("SKIP: ALL.LVL not present");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_detail_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let b = bake_soldier(all, "all_inf_soldier", dir.to_str().unwrap()).unwrap();
        assert!(b.textures.contains(&"all_inf_trooper".to_string()));
        let tex = dir.join("textures");
        let dims = |file: &str| {
            let bytes = std::fs::read(tex.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
            let r = ktx2::Reader::new(&bytes).unwrap();
            (r.header().pixel_width, r.header().pixel_height)
        };
        assert_eq!(dims("all_inf_trooper.ktx2"), (64, 64));
        assert_eq!(dims("all_inf_trooper.dtl.ktx2"), (256, 256));
        assert_eq!(dims("rebel_face01.ktx2"), (32, 32));
        assert_eq!(dims("rebel_face01.dtl.ktx2"), (128, 128));
        assert!(
            !tex.join("all_inf_trooper_dtl.ktx2").exists(),
            "the companion is baked as <base>.dtl.ktx2, not as its own texture"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Clips bake onto the stormtrooper: in place (DummyRoot stripped), root
    /// motion recorded, idle 71 fr / run 19 fr.
    #[test]
    fn stormtrooper_bakes_with_clips() {
        let imp = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
        );
        let common = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL"
        );
        if !std::path::Path::new(imp).exists() || !std::path::Path::new(common).exists() {
            eprintln!("SKIP: IMP.LVL / COMMON.LVL not present");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_soldier_anims_{}", std::process::id()));
        let b = bake_soldier_anims(
            imp,
            "imp_inf_stormtrooper",
            Some(common),
            dir.to_str().unwrap(),
            None,
        )
        .unwrap();
        // BF1's banks hold every seat clip but BF2's two (card VRIDE).
        assert_eq!(
            b.missing_clips,
            ["human_barc_9pose", "human_drive_1manatst"],
            "missing clips"
        );
        // The list and the eight seat clips first, in their order, then the
        // other clips of BF1's sets: 151 in the tables, 33 of them in the
        // list (`human_rifle_die` is in no table), none a seat clip.
        assert_eq!(b.clips, 160);
        let meta: SoldierAnims = ron::from_str(
            &std::fs::read_to_string(dir.join("soldiers/imp_inf_stormtrooper.anims.ron")).unwrap(),
        )
        .unwrap();
        for (c, name) in meta.clips.iter().zip(
            SOLDIER_CLIPS
                .iter()
                .chain(SEAT_CLIPS)
                .filter(|n| !b.missing_clips.iter().any(|m| m == *n)),
        ) {
            assert_eq!(c.name, *name);
        }
        let get = |n: &str| meta.clips.iter().find(|c| c.name == n).unwrap().clone();
        // The seat clips (cards VTURG, VRIDE): the turrets' 9, 9 and 1
        // frames, the riders' (re-vehicle-riders 4.2: `human_ride` 61), from
        // BF1's bank.
        for (n, f) in [
            ("human_minigun_9pose", 9),
            ("human_dishcannon_9pose", 9),
            ("human_drive", 1),
            ("human_speederbike_9pose", 9),
            ("human_ride", 61),
        ] {
            let c = get(n);
            println!("{n}: {} frames, bank {}", c.frames, c.bank);
            assert_eq!((c.frames, c.bank.as_str()), (f, "human_animation_1"), "{n}");
        }
        for n in ["human_stap_ride", "human_man_minigun", "human_standing"] {
            let c = get(n);
            println!("{n}: {} frames, bank {}", c.frames, c.bank);
        }
        assert_eq!(get("human_rifle_stand_idle_emote").frames, 71);
        let run = get("human_rifle_stand_runforward");
        assert_eq!(run.frames, 19);
        assert!(
            (run.root_motion[2] - 4.014).abs() < 0.01,
            "{:?}",
            run.root_motion
        );
        assert!((get("human_rifle_diveforward").root_motion[2] - 5.404).abs() < 0.01);
        let (doc, _, _) = gltf::import(dir.join("soldiers/imp_inf_stormtrooper.glb")).unwrap();
        assert_eq!(doc.animations().count(), 160);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Data-gated Tier-1: the BF1 stormtrooper bakes to a 25-joint skinned
    /// GLB whose model-space extents equal the modl vis box, and its texture.
    #[test]
    fn stormtrooper_bakes() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
        );
        if !std::path::Path::new(p).exists() {
            eprintln!("SKIP: IMP.LVL not present");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_soldier_{}", std::process::id()));
        let b = bake_soldier(p, "imp_inf_stormtrooper", dir.to_str().unwrap()).unwrap();
        assert_eq!((b.bones, b.vertices, b.triangles), (25, 1387, 1630));
        assert!((b.aabb.1[1] - 2.08769).abs() < 1e-3, "{:?}", b.aabb);
        assert!((b.aabb.0[0] + 0.929476).abs() < 1e-3);
        assert_eq!(b.textures, vec!["imp_inf_trooper".to_string()]);
        let glb = dir.join("soldiers/imp_inf_stormtrooper.glb");
        let (doc, buffers, _) = gltf::import(&glb).unwrap();
        let skin = doc.skins().next().unwrap();
        assert_eq!(skin.joints().count(), 25);
        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
        assert_eq!(reader.read_positions().unwrap().count(), 1387);
        let joints: Vec<[u16; 4]> = reader.read_joints(0).unwrap().into_u16().collect();
        assert_eq!(joints[0], [15, 0, 0, 0]);
        assert!(dir.join("textures/imp_inf_trooper.ktx2").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The BF1 stormtrooper rifle bakes as a static model with its texture.
    #[test]
    fn imp_rifle_bakes() {
        let imp = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
        );
        if !std::path::Path::new(imp).exists() {
            eprintln!("SKIP: IMP.LVL not present");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_weapon_{}", std::process::id()));
        let w = bake_weapon(imp, "imp_weap_inf_rifle", dir.to_str().unwrap(), None).unwrap();
        assert!(
            w.vertices > 50 && w.triangles > 30,
            "{} / {}",
            w.vertices,
            w.triangles
        );
        assert!(!w.textures.is_empty(), "rifle texture");
        assert!(dir.join("weapons/imp_weap_inf_rifle.glb").exists());
        assert!(dir.join("weapons/imp_weap_inf_rifle.nodes.ron").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Card FXMF: the fire points equal the PS2 RAM's class+0x30
    /// (`analysis/re-muzzle-flash.md` §4.1, `ramflash.py`).
    #[test]
    fn weapon_fire_points_are_the_ram_values() {
        let side = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/"
        );
        for (lvl, model, want) in [
            ("REP.LVL", "rep_weap_inf_rifle", [0.0, 0.0533, 1.097]),
            ("CIS.LVL", "cis_weap_inf_pistol", [0.0103, 0.0394, 0.2601]),
            ("ALL.LVL", "all_weap_inf_rifle", [0.0, 0.0597, 0.6926]),
            ("IMP.LVL", "imp_weap_inf_rifle", [0.0, 0.0422, 0.4082]),
        ] {
            let Ok(buf) = std::fs::read(format!("{side}{lvl}")) else {
                eprintln!("SKIP: {lvl} not present");
                continue;
            };
            let inner = mungefs::open(&buf).unwrap();
            let chunks = mungefs::content_chunks(inner).unwrap();
            let nodes = weapon_nodes(&chunks, model);
            let fp = nodes
                .iter()
                .find(|n| n.name.eq_ignore_ascii_case("hp_fire"))
                .unwrap_or_else(|| panic!("{model}: no hp_fire in {nodes:?}"));
            for (got, want) in fp.pos.iter().zip(want) {
                assert!((got - want).abs() < 5e-4, "{model}: {:?}", fp.pos);
            }
        }
    }
}
