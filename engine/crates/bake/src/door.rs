//! BF2 `door` rigs (queue DR2): what a `door`-based class needs at runtime to
//! open by itself — its trigger spheres and the motion its `open` clip gives
//! each collision primitive — baked into [`ManifestDoor`].
//!
//! The disc's rule (`analysis/re-bf2-doors.md`, all [V][O]):
//! - The class ODF names the rig (§2 hop 2, `FUN_001ec5d0`): `AnimationName`
//!   = the `zaa_` bank, `Animation` = the clip in it, `AnimationTrigger` =
//!   up to THREE `"<hardpoint> <radius>"` pairs (format `"%s %f"`).
//! - The hardpoint is a node of the model's `skel` (§3 hop 8: `hp_door`, a
//!   child of `boneroot`), and a collision `prim` hangs off a bone by its
//!   `PRNT` (`p_door` → `bone_door`); the clip animates that bone (§3 hop 9:
//!   3.965 m in 1.00 s on MUS1's `mus1_prop_door_tri`).
//!
//! What this bakes, per prim, is the MODEL-space delta of its `PRNT` bone,
//! `D(f) = B(f)·B(0)⁻¹`, where `B` walks the skeleton's parent chain with each
//! bone's clip key where the clip has a track for it and the `skel` rest pose
//! where it does not. Measuring from frame 0 rather than from the `skel` rest
//! makes the shut door exactly the box the bake rests in model space: a
//! door's prims and `BNAM` segments are stored in their bone's frame and the
//! bake puts them through its bind matrix (`crate::bind_pose`, card STDOR —
//! DEA1's blast door binds its leaves at x ∓5, MUS1's doors bind tilted
//! 20.8°). PROVISIONAL [D]: every clip read so far starts at that bind
//! (§3 hop 9, frame 0 = (0, −0, 0.0001)).
//! The hardpoint is placed at frame 0 too — PROVISIONAL [H]: the tick resolves
//! it through the ANIMATED model every frame (§2 hop 5, `FUN_0057dc60`), which
//! only matters for a door whose clip moves the hardpoint's own parent
//! (`mus1_prop_door_garage` lifts `boneroot`).
//!
//! The DRAWN door (queue DRV) follows the same rule: every door model's
//! segments are hard-bound to one bone each by `BNAM` (census over the 13
//! rigged classes, 2026-09-25: no `bone_space` model, no soft segment) and
//! rested through the same bind matrix, so each drawn submesh rides its
//! bone's `D(f)`
//! ([`ManifestDoor::parts`]). The ODF's `OpenSound` / `CloseSound` names ride
//! along for the game to play (§2 hop 5).

use glam::{Mat3, Mat4, Quat, Vec3};
use manifest::{ManifestDoor, ManifestDoorPart, ManifestDoorPrim, ManifestDoorTrigger};
use mungefs::anim::{crc32_lower, AnimBank, DecodedClip};
use mungefs::entity::EntityClass;
use mungefs::skel::Skeleton;

/// fnv1a_lowercase of the three door ODF keys (`re-bf2-doors.md` §2 hop 2).
pub const PROP_ANIMATION: u32 = 0xE145_EE5D;
pub const PROP_ANIMATIONNAME: u32 = 0x9855_7A5E;
pub const PROP_ANIMATIONTRIGGER: u32 = 0x1D56_B73B;
/// fnv1a_lowercase of `OpenSound` / `CloseSound` (`re-bf2-doors.md` §2 hop 2:
/// class+0x360 / +0x368).
pub const PROP_OPENSOUND: u32 = 0x323F_7450;
pub const PROP_CLOSESOUND: u32 = 0x4FCF_4562;
pub const PROP_LOCKEDSOUND: u32 = 0x027B_5CB2;

/// The disc's cap on trigger pairs (`FUN_001ec5d0` loops over three).
pub const MAX_TRIGGERS: usize = 3;

/// A door class's ODF lines, kept from its `entc` until the walk has seen
/// every `skel` and `zaa_`.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorSpec {
    pub class: String,
    pub locked: bool,
    /// `AnimationName` (the bank), `Animation` (the clip).
    pub bank: Option<String>,
    pub clip: Option<String>,
    /// `(hardpoint, radius)`, ODF order, at most [`MAX_TRIGGERS`].
    pub triggers: Vec<(String, f32)>,
    /// `OpenSound`, `CloseSound`.
    pub open_sound: Option<String>,
    pub close_sound: Option<String>,
    pub locked_sound: Option<String>,
}

impl DoorSpec {
    /// `Some` for a class whose BASE is `door`.
    pub fn from_entc(e: &EntityClass) -> Option<DoorSpec> {
        if !e.base.eq_ignore_ascii_case("door") {
            return None;
        }
        let last = |key: u32| {
            e.props
                .iter()
                .rev()
                .find(|p| p.key_hash == key)
                .map(|p| p.value.clone())
        };
        let triggers = e
            .props
            .iter()
            .filter(|p| p.key_hash == PROP_ANIMATIONTRIGGER)
            .filter_map(|p| parse_trigger(&p.value))
            .take(MAX_TRIGGERS)
            .collect();
        Some(DoorSpec {
            class: e.name.clone(),
            locked: last(mungefs::fnv1a_lowercase("IsLocked"))
                .and_then(|value| value.trim().parse::<i32>().ok())
                .is_some_and(|value| value != 0),
            bank: last(PROP_ANIMATIONNAME),
            clip: last(PROP_ANIMATION),
            triggers,
            open_sound: last(PROP_OPENSOUND),
            close_sound: last(PROP_CLOSESOUND),
            locked_sound: last(PROP_LOCKEDSOUND),
        })
    }
}

/// `"%s %f"`: a name, whitespace, a float (`"hp_door 4"`, `"hp_door 3.0"`).
pub fn parse_trigger(v: &str) -> Option<(String, f32)> {
    let mut it = v.split_whitespace();
    let name = it.next()?.to_owned();
    let radius = it.next()?.parse::<f32>().ok()?;
    Some((name, radius))
}

/// A skeleton bone's rest pose, column-vector (`soldier::bones_of`'s rule: the
/// row-vector rotation rows are the column-vector matrix's columns).
fn rest_local(skel: &Skeleton, i: usize) -> Mat4 {
    let b = &skel.bones[i];
    let m = Mat3::from_cols(
        Vec3::from_array(b.rows[0]),
        Vec3::from_array(b.rows[1]),
        Vec3::from_array(b.rows[2]),
    );
    Mat4::from_rotation_translation(Quat::from_mat3(&m).normalize(), Vec3::from_array(b.t))
}

/// Every bone's MODEL-space transform at `frame` of `clip`. `None` on a
/// skeleton whose parent links do not form a tree.
fn pose(skel: &Skeleton, clip: &DecodedClip, frame: usize) -> Option<Vec<Mat4>> {
    let n = skel.bones.len();
    let local: Vec<Mat4> = (0..n)
        .map(|i| {
            let crc = skel.bones[i].crc;
            match clip.tracks.iter().find(|t| t.bone_crc == crc) {
                Some(t) => {
                    let q = t
                        .rotations
                        .get(frame)
                        .copied()
                        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
                    let p = t
                        .translations
                        .get(frame)
                        .copied()
                        .unwrap_or(skel.bones[i].t);
                    Mat4::from_rotation_translation(
                        Quat::from_array(q).normalize(),
                        Vec3::from_array(p),
                    )
                }
                None => rest_local(skel, i),
            }
        })
        .collect();
    let mut out: Vec<Option<Mat4>> = vec![None; n];
    // Parents may follow their children in `skel` order: resolve by
    // repeated passes, at most `n` of them (a tree needs no more).
    for _ in 0..=n {
        let mut progress = false;
        for i in 0..n {
            if out[i].is_some() {
                continue;
            }
            match skel.bones[i].parent {
                None => out[i] = Some(local[i]),
                Some(p) => match out.get(p).copied().flatten() {
                    Some(pm) => out[i] = Some(pm * local[i]),
                    None => continue,
                },
            }
            progress = true;
        }
        if !progress {
            break;
        }
    }
    out.into_iter().collect()
}

/// Is `m` the identity to within the clip's own quantisation?
fn is_identity(q: Quat, t: Vec3) -> bool {
    q.dot(Quat::IDENTITY).abs() > 1.0 - 1e-6 && t.length() < 1e-4
}

/// Bone `bone`'s MODEL-space delta from frame 0, `B(f)·B(0)⁻¹`, per frame
/// as (rotation xyzw, translation); `None` when it never moves.
fn delta(poses: &[Vec<Mat4>], bone: usize) -> Option<Vec<([f32; 4], [f32; 3])>> {
    let rest_inv = poses[0][bone].inverse();
    let motion: Vec<([f32; 4], [f32; 3])> = poses
        .iter()
        .enumerate()
        .map(|(f, pose)| {
            if f == 0 {
                return ([0.0, 0.0, 0.0, 1.0], [0.0; 3]); // exactly shut
            }
            let (_, q, t) = (pose[bone] * rest_inv).to_scale_rotation_translation();
            (q.normalize().to_array(), t.to_array())
        })
        .collect();
    let still = motion
        .iter()
        .all(|(q, t)| is_identity(Quat::from_array(*q), Vec3::from_array(*t)));
    (!still).then_some(motion)
}

/// The rig of one door class. `skel` is the class model's skeleton, `bank`
/// the clip's bank, `prims` the model's collision primitives as (NAME,
/// PRNT), `segments` the drawn model's `BNAM` per GLB primitive (see
/// [`ManifestDoor::parts`]). `Err` says why not, for the bake's report.
pub fn rig(
    spec: &DoorSpec,
    skel: &Skeleton,
    bank: &AnimBank<'_>,
    prims: &[(String, String)],
    segments: &[Option<String>],
) -> Result<ManifestDoor, String> {
    let clip_name = spec.clip.as_deref().ok_or("no Animation line")?;
    let k = bank
        .clip_index(clip_name)
        .ok_or_else(|| format!("bank {} has no clip {clip_name}", bank.name))?;
    let clip = bank
        .decode_clip(k)
        .map_err(|e| format!("clip {clip_name}: {e}"))?;
    let frames = usize::from(clip.frames);
    if frames < 2 {
        return Err(format!("clip {clip_name} has {frames} frame(s)"));
    }
    let poses: Vec<Vec<Mat4>> = (0..frames)
        .map(|f| pose(skel, &clip, f))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("skel {}: not a tree", skel.name))?;
    let triggers = spec
        .triggers
        .iter()
        .filter_map(|(hp, radius)| {
            let i = skel.index_of(hp)?;
            let p = poses[0][i].w_axis.truncate();
            Some(ManifestDoorTrigger {
                hardpoint: hp.clone(),
                radius: *radius,
                position: p.to_array(),
            })
        })
        .collect::<Vec<_>>();
    if triggers.is_empty() {
        return Err(format!(
            "no AnimationTrigger hardpoint found in skel {}",
            skel.name
        ));
    }
    let mut moving = Vec::new();
    for (name, parent) in prims {
        let Some(bone) = skel.index_of(parent) else {
            continue; // a PRNT the skeleton lacks: the prim stays put
        };
        if let Some(motion) = delta(&poses, bone) {
            moving.push(ManifestDoorPrim {
                name: name.clone(),
                motion,
            });
        }
    }
    let mut parts = Vec::new();
    for (k, bnam) in segments.iter().enumerate() {
        let Some(bone) = bnam.as_deref().and_then(|b| skel.index_of(b)) else {
            continue; // unbound, or a bone the skeleton lacks: drawn shut
        };
        if let Some(motion) = delta(&poses, bone) {
            parts.push(ManifestDoorPart {
                submesh: k as u32,
                bone: skel.bones[bone].name.clone(),
                motion,
            });
        }
    }
    Ok(ManifestDoor {
        locked: spec.locked,
        triggers,
        frames: clip.frames,
        prims: moving,
        parts,
        open_sound: spec.open_sound.clone(),
        close_sound: spec.close_sound.clone(),
        locked_sound: spec.locked_sound.clone(),
    })
}

/// Every door class's rig over an LVL's content chunks (the map bake's
/// flattened walk, top level first): `(class, rig or why not)`, one row per
/// `door`-based `entc`, first of a name wins like the walk's own rule. The
/// skeleton is the one named like the class's `GeometryName` or, failing
/// that, its `gmod`'s best LOD; the prims are the `prim` chunk of that model.
pub fn rigs(chunks: &[mungefs::Chunk<'_>]) -> Vec<(String, Result<ManifestDoor, String>)> {
    let mut specs: Vec<(DoorSpec, Option<String>)> = Vec::new();
    let skels = crate::hardpoints::Skeletons::of(chunks);
    let mut banks: Vec<AnimBank<'_>> = Vec::new();
    let mut prims: Vec<(String, Vec<(String, String)>)> = Vec::new();
    // Per drawn model: its segments' `BNAM`, one per GLB primitive
    // (`crate::glb::model_to_glb` skips a segment with no triangles).
    let mut models: Vec<(String, Vec<Option<String>>)> = Vec::new();
    for c in chunks {
        match &c.tag {
            b"entc" => {
                let Ok(e) = EntityClass::parse(c) else {
                    continue;
                };
                if specs
                    .iter()
                    .any(|(s, _)| s.class.eq_ignore_ascii_case(&e.name))
                {
                    continue;
                }
                if let Some(s) = DoorSpec::from_entc(&e) {
                    specs.push((s, e.geometry_name().map(str::to_owned)));
                }
            }
            b"zaa_" => {
                if let Ok(b) = mungefs::anim::parse_zaa(c) {
                    banks.push(b);
                }
            }
            b"modl" => {
                if let Ok(m) = mungefs::model::Model::parse(c) {
                    if !models.iter().any(|(n, _)| n.eq_ignore_ascii_case(&m.name)) {
                        let bones = m
                            .segments
                            .iter()
                            .filter(|s| !s.triangles.is_empty())
                            .map(|s| s.bone.clone());
                        models.push((m.name.clone(), bones.collect()));
                    }
                }
            }
            b"prim" => {
                if let Ok(p) = mungefs::coll::Primitives::parse(c) {
                    if !prims.iter().any(|(m, _)| m.eq_ignore_ascii_case(&p.model)) {
                        let list = p.prims.iter().map(|q| (q.name.clone(), q.parent.clone()));
                        prims.push((p.model.clone(), list.collect()));
                    }
                }
            }
            _ => {}
        }
    }
    specs
        .into_iter()
        .map(|(spec, geom)| {
            let class = spec.class.clone();
            let r = (|| {
                let geom = geom.ok_or("no GeometryName")?;
                let model = skels
                    .geometry(&geom)
                    .and_then(|g| g.best_model().map(str::to_owned));
                let skel = skels
                    .find(&geom)
                    .ok_or_else(|| format!("no skel for {geom}"))?;
                let bank_name = spec.bank.as_deref().ok_or("no AnimationName line")?;
                let bank = find_bank(&banks, bank_name)
                    .ok_or_else(|| format!("no zaa_ bank {bank_name}"))?;
                let body = prims
                    .iter()
                    .find(|(m, _)| m.eq_ignore_ascii_case(&geom))
                    .map(|(_, l)| l.as_slice())
                    .unwrap_or_default();
                let drawn = model.as_deref().unwrap_or(&geom);
                let segments = models
                    .iter()
                    .find(|(m, _)| m.eq_ignore_ascii_case(drawn))
                    .map(|(_, l)| l.as_slice())
                    .unwrap_or_default();
                rig(&spec, skel, bank, body, segments)
            })();
            (class, r)
        })
        .collect()
}

/// Find a bank by its NAME (case-insensitive).
pub fn find_bank<'a, 'b>(banks: &'a [AnimBank<'b>], name: &str) -> Option<&'a AnimBank<'b>> {
    banks
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case(name))
        .or_else(|| {
            let crc = crc32_lower(name);
            banks.iter().find(|b| crc32_lower(&b.name) == crc)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trigger_line_is_a_name_and_a_radius() {
        assert_eq!(parse_trigger("hp_door 4"), Some(("hp_door".into(), 4.0)));
        assert_eq!(
            parse_trigger("hp_door   3.0"),
            Some(("hp_door".into(), 3.0))
        );
        assert_eq!(parse_trigger("hp_door"), None);
        assert_eq!(parse_trigger("hp_door x"), None);
    }

    /// Data-gated: MUS1's hangar door, against the RE's own numbers
    /// (`re-bf2-doors.md` §3 hops 7–9): trigger `hp_door` 4 m at the bind
    /// (0, 0, −1.1098); a 31-frame `open`; `p_door` rides `bone_door`
    /// 3.965 m, to (0, 3.707, 1.409) from its frame-0 position.
    #[test]
    fn mus1_hangar_door_rig_matches_the_disc() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/MUS/MUS1.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: MUS1.LVL not present");
            return;
        };
        let inner = mungefs::open(&buf).unwrap();
        let chunks = mungefs::content_chunks(inner).unwrap();
        let doors = rigs(&chunks);
        let d = doors
            .iter()
            .find(|(c, _)| c.eq_ignore_ascii_case("mus1_prop_door_tri"))
            .map(|(_, d)| d.as_ref().unwrap())
            .expect("mus1_prop_door_tri has a rig");
        assert_eq!(d.frames, 31);
        assert_eq!(d.triggers.len(), 1);
        let t = &d.triggers[0];
        assert_eq!((t.hardpoint.as_str(), t.radius), ("hp_door", 4.0));
        assert!((Vec3::from_array(t.position) - Vec3::new(0.0, 0.0, -1.1098)).length() < 1e-3);
        assert_eq!(d.prims.len(), 1);
        assert_eq!(d.prims[0].name, "p_door");
        let (q, end) = d.prims[0].motion[30];
        assert!(Quat::from_array(q).dot(Quat::IDENTITY).abs() > 0.9999);
        let travel = Vec3::from_array(end);
        assert!(
            (travel - Vec3::new(0.0, 3.707, 1.409)).length() < 2e-3,
            "{travel}"
        );
        assert!((travel.length() - 3.965).abs() < 2e-3);
        assert_eq!(d.prims[0].motion[0].1, [0.0; 3]);
        // DRV: the one drawn segment rides `bone_door` exactly as `p_door`
        // does, and the ODF's sounds (§3) come along.
        assert_eq!(d.parts.len(), 1);
        assert_eq!(
            (d.parts[0].submesh, d.parts[0].bone.as_str()),
            (0, "bone_door")
        );
        assert_eq!(d.parts[0].motion, d.prims[0].motion);
        assert_eq!(d.open_sound.as_deref(), Some("door_open"));
        assert_eq!(d.locked_sound.as_deref(), Some("vehicle_equip"));
        assert_eq!(d.close_sound.as_deref(), Some("door_close"));
    }

    /// The sound keys are the fnv1a preimages the RE names (§2 hop 2).
    #[test]
    fn the_sound_keys_are_their_names() {
        assert_eq!(mungefs::fnv1a_lowercase("OpenSound"), PROP_OPENSOUND);
        assert_eq!(mungefs::fnv1a_lowercase("CloseSound"), PROP_CLOSESOUND);
        assert_eq!(mungefs::fnv1a_lowercase("LockedSound"), PROP_LOCKEDSOUND);
    }
}
