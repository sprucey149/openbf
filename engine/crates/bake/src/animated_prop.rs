//! BF2's `animatedprop` classes (EntityPropAnimated) and the animated props
//! other classes hang on their hardpoints, written to
//! `manifest::animated_prop::FILE` beside the map's manifest.
//!
//! The class's lines (B2 setter `FUN_002321a0` [V][O]):
//! - `AnimationName` names the `zaa_` bank; each `Animation` fills the next
//!   of five clip slots with the bank's clip of that name. A name the bank
//!   lacks fills nothing, so the next line takes the same slot. Slot 0 is
//!   the idle loop.
//! - `AnimationTrigger "<clip> <hardpoint> <radius>"` (format `"%s %s %f"`,
//!   up to three): the clip is matched to a slot by its CRC; one the slots
//!   lack leaves slot 0.
//! - `AttachTrigger "<hardpoint> <radius> <clip> <soldier anim> <time> <…>
//!   <event>"` (up to three).
//! - `KillSoldierSound`, `TrackDeathOnAttach`, `DisableForCloneWars`,
//!   `IdleDelay` (`"%f"`).
//!
//! The update (B2 `FUN_002312c0` [V][O]) is the runtime's
//! (`core::animated_prop`). What it needs from the disc is baked here:
//! - each trigger's hardpoint at the BIND pose: the trigger reads the
//!   model's static hardpoint table (`FUN_0057dc60`), not the pose;
//! - each attach hardpoint's position at every frame of every clip: the
//!   catch reads the animated pose (`FUN_00539150` on the pose's matrices);
//! - each collision prim's `PRNT` bone at every frame of every clip, so the
//!   prim rides it (Jabba's Palace's trapdoor drops its two leaves).
//!   PROVISIONAL [D]: the PS2 adds the prop to the collision manager as a
//!   static object; the prims follow the bones here because the trapdoor's
//!   leaves are the only floor over its hole (the palace's own mesh has
//!   none there) and its `open` clip swings their bones down.
//!
//! The bones' transforms per frame are `crate::door::pose`'s: the clip's key
//! where the clip has a track for the bone, the `skel` rest otherwise.
//!
//! **The attach rule** for an animated prop another class carries is
//! `crate::leafpatch`'s (B2 `FUN_0022f6e8`): `AttachOdf` sets one current
//! class for the walk, `AttachToHardpoint` appends it at the carrier
//! model's hardpoint (bind matrix; 16 a class), `AttachEffect` clears it.
//! Instance creation puts the child at `hardpoint × instance`
//! (`re-leafpatch.md` §2.1).

use glam::{Mat4, Quat, Vec3};
use manifest::animated_prop::{
    AnimatedProps, AnimatedRig, AttachedProp, PropAttach, PropClip, PropPrim, PropTrigger,
};
use mungefs::anim::{crc32_lower, AnimBank};
use mungefs::entity::EntityClass;
use mungefs::skel::Skeleton;

use crate::hardpoints::Skeletons;
use crate::leafpatch::{ATTACH_EFFECT, ATTACH_ODF, ATTACH_TO_HARDPOINT};

pub const PROP_ANIMATIONNAME: u32 = 0x9855_7A5E;
pub const PROP_ANIMATION: u32 = 0xE145_EE5D;
pub const PROP_ANIMATIONTRIGGER: u32 = 0x1D56_B73B;
pub const PROP_ATTACHTRIGGER: u32 = 0x7230_6628;
pub const PROP_KILLSOLDIERSOUND: u32 = 0xF949_D898;
pub const PROP_TRACKDEATHONATTACH: u32 = 0x0550_A3B2;
pub const PROP_DISABLEFORCLONEWARS: u32 = 0x6B01_4234;
pub const PROP_IDLEDELAY: u32 = 0xFF70_1082;

/// The disc's slot counts: five clips, three triggers of each kind.
pub const MAX_CLIPS: usize = 5;
pub const MAX_TRIGGERS: usize = 3;
/// BF2's attachments a class (`0xf < count` returns).
pub const MAX_ATTACH: usize = 16;

pub fn is_animated_base(base: &str) -> bool {
    base.eq_ignore_ascii_case("animatedprop")
}

/// An `animatedprop` class's ODF lines, kept until the walk has seen every
/// `skel` and `zaa_`.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub class: String,
    pub geometry: Option<String>,
    pub bank: Option<String>,
    pub clips: Vec<String>,
    /// (clip, hardpoint, radius).
    pub triggers: Vec<(String, String, f32)>,
    /// (hardpoint, radius, clip, soldier animation, time).
    pub attaches: Vec<(String, f32, String, String, f32)>,
    pub kill_sound: Option<String>,
    pub track_death: bool,
    pub disable_for_clone_wars: bool,
    pub idle_delay: f32,
}

fn flag(v: &str) -> bool {
    v.trim().parse::<i64>().is_ok_and(|n| n != 0)
}

impl Spec {
    pub fn from_entc(e: &EntityClass) -> Option<Spec> {
        if !is_animated_base(&e.base) {
            return None;
        }
        let mut s = Spec {
            class: e.name.clone(),
            geometry: e.geometry_name().map(str::to_owned),
            bank: None,
            clips: Vec::new(),
            triggers: Vec::new(),
            attaches: Vec::new(),
            kill_sound: None,
            track_death: false,
            disable_for_clone_wars: false,
            idle_delay: 0.0,
        };
        for p in &e.props {
            let v = p.value.as_str();
            match p.key_hash {
                PROP_ANIMATIONNAME => s.bank = Some(v.trim().to_owned()),
                PROP_ANIMATION => s.clips.push(v.trim().to_owned()),
                PROP_ANIMATIONTRIGGER => {
                    let mut it = v.split_whitespace();
                    if let (Some(c), Some(hp), Some(r)) = (it.next(), it.next(), it.next()) {
                        if let Ok(r) = r.parse::<f32>() {
                            if s.triggers.len() < MAX_TRIGGERS {
                                s.triggers.push((c.to_owned(), hp.to_owned(), r));
                            }
                        }
                    }
                }
                PROP_ATTACHTRIGGER => {
                    let t: Vec<&str> = v.split_whitespace().collect();
                    if t.len() >= 5 && s.attaches.len() < MAX_TRIGGERS {
                        if let (Ok(r), Ok(time)) = (t[1].parse::<f32>(), t[4].parse::<f32>()) {
                            s.attaches.push((
                                t[0].to_owned(),
                                r,
                                t[2].to_owned(),
                                t[3].to_owned(),
                                time,
                            ));
                        }
                    }
                }
                PROP_KILLSOLDIERSOUND => s.kill_sound = Some(v.trim().to_owned()),
                PROP_TRACKDEATHONATTACH => s.track_death = flag(v),
                PROP_DISABLEFORCLONEWARS => s.disable_for_clone_wars = flag(v),
                PROP_IDLEDELAY => {
                    if let Ok(d) = v.trim().parse::<f32>() {
                        s.idle_delay = d;
                    }
                }
                _ => {}
            }
        }
        Some(s)
    }
}

/// The bank's clip index for an ODF clip name: by its own name, else the
/// bank-prefixed one.
fn clip_index(bank: &AnimBank<'_>, name: &str) -> Option<usize> {
    bank.clip_index(name)
        .or_else(|| bank.clip_index(&format!("{}_{name}", bank.name)))
}

/// The clips the five slots hold: each `Animation` line the bank has, in
/// ODF order, at most five (bank index, name).
pub fn slots(spec: &Spec, bank: &AnimBank<'_>) -> Vec<(usize, String)> {
    spec.clips
        .iter()
        .filter_map(|n| clip_index(bank, n).map(|k| (k, n.clone())))
        .take(MAX_CLIPS)
        .collect()
}

/// The slot whose clip has `name`'s CRC; slot 0 when none (the zeroed
/// field the setter leaves).
fn slot_of(slots: &[(usize, String)], name: &str) -> u8 {
    let crc = crc32_lower(name);
    slots
        .iter()
        .position(|(_, n)| crc32_lower(n) == crc)
        .unwrap_or(0) as u8
}

fn qt(m: Mat4) -> ([f32; 4], [f32; 3]) {
    let (_, q, t) = m.to_scale_rotation_translation();
    (q.normalize().to_array(), t.to_array())
}

/// The rig of one class: `skel` its model's skeleton, `bank` its bank,
/// `prims` its geometry's collision prims as (NAME, PRNT).
pub fn rig(
    spec: &Spec,
    skel: &Skeleton,
    bank: &AnimBank<'_>,
    prims: &[(String, String)],
) -> Result<AnimatedRig, String> {
    let geometry = spec.geometry.clone().ok_or("no GeometryName")?;
    let slots = slots(spec, bank);
    if slots.is_empty() {
        return Err(format!("bank {} has none of its clips", bank.name));
    }
    let mut clips = Vec::new();
    // Per slot, per frame: every bone's model-space transform.
    let mut poses: Vec<Vec<Vec<Mat4>>> = Vec::new();
    for (k, name) in &slots {
        let d = bank
            .decode_clip(*k)
            .map_err(|e| format!("clip {name}: {e}"))?;
        let frames = usize::from(d.frames.max(1));
        let p: Vec<Vec<Mat4>> = (0..frames)
            .map(|f| crate::door::pose(skel, &d, f))
            .collect::<Option<_>>()
            .ok_or_else(|| format!("skel {}: not a tree", skel.name))?;
        clips.push(PropClip {
            name: name.clone(),
            frames: d.frames.max(1),
        });
        poses.push(p);
    }
    let bind = skel.bind_matrices().map_err(|e| e.to_string())?;
    let triggers = spec
        .triggers
        .iter()
        .filter_map(|(clip, hp, radius)| {
            let i = skel.index_of(hp)?;
            Some(PropTrigger {
                clip: slot_of(&slots, clip),
                hardpoint: hp.clone(),
                radius: *radius,
                position: [bind[i][3][0], bind[i][3][1], bind[i][3][2]],
            })
        })
        .collect();
    let attaches = spec
        .attaches
        .iter()
        .filter_map(|(hp, radius, clip, anim, time)| {
            let i = skel.index_of(hp)?;
            Some(PropAttach {
                hardpoint: hp.clone(),
                radius: *radius,
                clip: slot_of(&slots, clip),
                soldier_animation: anim.clone(),
                time: *time,
                track: poses
                    .iter()
                    .map(|frames| {
                        frames
                            .iter()
                            .map(|pose| pose[i].w_axis.truncate().to_array())
                            .collect()
                    })
                    .collect(),
            })
        })
        .collect();
    let mut moving = Vec::new();
    for (name, parent) in prims {
        let Some(bone) = skel.index_of(parent) else {
            continue;
        };
        let frames: Vec<Vec<([f32; 4], [f32; 3])>> = poses
            .iter()
            .map(|clip| clip.iter().map(|pose| qt(pose[bone])).collect())
            .collect();
        let still = frames.iter().flatten().all(|(q, t)| {
            Quat::from_array(*q).dot(Quat::IDENTITY).abs() > 1.0 - 1e-6
                && Vec3::from_array(*t).length() < 1e-4
        });
        if !still {
            moving.push(PropPrim {
                name: name.clone(),
                bone: skel.bones[bone].name.clone(),
                frames,
            });
        }
    }
    Ok(AnimatedRig {
        class: spec.class.clone(),
        geometry,
        model: None,
        bank: bank.name.clone(),
        clips,
        triggers,
        attaches,
        prims: moving,
        kill_sound: spec.kill_sound.clone(),
        track_death: spec.track_death,
        disable_for_clone_wars: spec.disable_for_clone_wars,
        idle_delay: spec.idle_delay,
    })
}

/// Every animated prop over an LVL's content chunks (the map bake's
/// flattened walk, top level first; first of a class name wins): the rigs,
/// and the attachments `entcs` (`(name, attach props)` in walk order) make.
/// `drawn` names each rig's written model, if any (`rig class → model`).
/// Lines say why a class got no rig.
pub fn collect(
    chunks: &[mungefs::Chunk<'_>],
    entcs: &[(String, Vec<(u32, String)>)],
    drawn: &dyn Fn(&str) -> Option<String>,
) -> (AnimatedProps, Vec<String>) {
    let skels = Skeletons::of(chunks);
    let mut specs: Vec<Spec> = Vec::new();
    let mut geometry_of: Vec<(String, Option<String>)> = Vec::new();
    let mut banks: Vec<AnimBank<'_>> = Vec::new();
    let mut prims: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for c in chunks {
        match &c.tag {
            b"entc" => {
                let Ok(e) = EntityClass::parse(c) else {
                    continue;
                };
                if geometry_of
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(&e.name))
                {
                    continue;
                }
                geometry_of.push((e.name.clone(), e.geometry_name().map(str::to_owned)));
                if let Some(s) = Spec::from_entc(&e) {
                    specs.push(s);
                }
            }
            b"zaa_" => {
                if let Ok(b) = mungefs::anim::parse_zaa(c) {
                    banks.push(b);
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
    let mut out = AnimatedProps::default();
    let mut notes = Vec::new();
    if specs.is_empty() {
        return (out, notes);
    }
    for spec in &specs {
        let r = (|| {
            let geom = spec.geometry.as_deref().ok_or("no GeometryName")?;
            let skel = skels
                .find(geom)
                .ok_or_else(|| format!("no skel for {geom}"))?;
            let bank_name = spec.bank.as_deref().ok_or("no AnimationName")?;
            let bank = crate::door::find_bank(&banks, bank_name)
                .ok_or_else(|| format!("no zaa_ bank {bank_name}"))?;
            let body = prims
                .iter()
                .find(|(m, _)| m.eq_ignore_ascii_case(geom))
                .map(|(_, l)| l.as_slice())
                .unwrap_or_default();
            rig(spec, skel, bank, body)
        })();
        match r {
            Ok(mut rig) => {
                rig.model = drawn(&rig.class);
                out.rigs.push(rig);
            }
            Err(e) => notes.push(format!("{}: no rig ({e})", spec.class)),
        }
    }
    // The attach replay: one current class for the whole walk.
    let mut current: Option<String> = None;
    for (name, props) in entcs {
        let skel = geometry_of
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .and_then(|(_, g)| g.as_deref())
            .and_then(|g| skels.find(g));
        let binds = skel.and_then(|s| s.bind_matrices().ok());
        let mut count = 0usize;
        for (h, v) in props {
            match *h {
                ATTACH_ODF => {
                    let n = v.trim();
                    current = geometry_of
                        .iter()
                        .find(|(c, _)| c.eq_ignore_ascii_case(n))
                        .map(|(c, _)| c.clone());
                }
                ATTACH_EFFECT => current = None,
                ATTACH_TO_HARDPOINT => {
                    let (Some(cur), Some(s), Some(m)) = (&current, skel, &binds) else {
                        continue;
                    };
                    let Some(hp) = v.split_whitespace().next() else {
                        continue;
                    };
                    if count >= MAX_ATTACH {
                        continue;
                    }
                    let Some(i) = s.index_of(hp) else {
                        continue;
                    };
                    count += 1;
                    if out.rig(cur).is_none() {
                        continue;
                    }
                    let r = m[i];
                    out.attached.push(AttachedProp {
                        parent: name.clone(),
                        class: cur.clone(),
                        hardpoint: hp.to_owned(),
                        rotation: [
                            [r[0][0], r[0][1], r[0][2]],
                            [r[1][0], r[1][1], r[1][2]],
                            [r[2][0], r[2][1], r[2][2]],
                        ],
                        translation: [r[3][0], r[3][1], r[3][2]],
                    });
                }
                _ => {}
            }
        }
    }
    (out, notes)
}

/// Write `out`'s animated props file, or remove a stale one when there are none.
pub fn write(out: &std::path::Path, props: &AnimatedProps) -> anyhow::Result<()> {
    let path = out.join(manifest::animated_prop::FILE);
    if props.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(());
    }
    crate::write_ron(&path, props)
}

/// Each vertex's bone for a model posed by `skel`, segment by segment: a
/// bone-space model's own per-vertex bone, else the segment's `BNAM` bone,
/// else the skeleton's root. `None` when a vertex names no bone.
pub fn joints(m: &mungefs::model::Model, skel: &Skeleton) -> Option<Vec<Vec<usize>>> {
    let root = skel
        .bones
        .iter()
        .position(|b| b.parent.is_none())
        .unwrap_or(0);
    m.segments
        .iter()
        .map(|s| {
            if m.bone_space {
                mungefs::model::to_model_space(m, s, skel)
                    .ok()
                    .map(|(_, _, b)| b)
            } else {
                let b = s
                    .bone
                    .as_deref()
                    .and_then(|b| skel.index_of(b))
                    .unwrap_or(root);
                Some(vec![b; s.positions.len()])
            }
        })
        .collect()
}

/// The models an LVL's `animatedprop` classes draw, and their banks: what
/// the map bake's model walk writes to `animated/` ([`Models::write`]).
pub struct Models<'a> {
    /// (drawn model, lower case; its class's spec).
    models: Vec<(String, Spec)>,
    banks: Vec<AnimBank<'a>>,
    /// (class, model) of every pair written.
    pub written: Vec<(String, String)>,
}

impl<'a> Models<'a> {
    pub fn of(chunks: &[mungefs::Chunk<'a>]) -> Models<'a> {
        let skels = Skeletons::of(chunks);
        let mut models: Vec<(String, Spec)> = Vec::new();
        let mut banks = Vec::new();
        for c in chunks {
            match &c.tag {
                b"entc" => {
                    let Some(s) = EntityClass::parse(c).ok().and_then(|e| Spec::from_entc(&e))
                    else {
                        continue;
                    };
                    let Some(geom) = s.geometry.clone() else {
                        continue;
                    };
                    let model = skels
                        .geometry(&geom)
                        .and_then(|g| g.best_model().map(str::to_owned))
                        .unwrap_or(geom)
                        .to_ascii_lowercase();
                    if !models
                        .iter()
                        .any(|(m, k)| *m == model || k.class.eq_ignore_ascii_case(&s.class))
                    {
                        models.push((model, s));
                    }
                }
                b"zaa_" => {
                    if let Ok(b) = mungefs::anim::parse_zaa(c) {
                        banks.push(b);
                    }
                }
                _ => {}
            }
        }
        if models.is_empty() {
            banks.clear();
        }
        Models {
            models,
            banks,
            written: Vec::new(),
        }
    }

    /// Write `animated/<model>.glb` and `.skin.glb` when `m` (as parsed,
    /// before its bind pose) is an animated prop's model.
    pub fn write(
        &mut self,
        m: &mungefs::model::Model,
        bind_pose: &crate::bind_pose::BindPose,
        alpha_table: &crate::alpha::AlphaTable,
        out: &std::path::Path,
    ) -> anyhow::Result<()> {
        let key = m.name.to_ascii_lowercase();
        let Some((_, spec)) = self.models.iter().find(|(n, _)| *n == key) else {
            return Ok(());
        };
        if self
            .written
            .iter()
            .any(|(_, n)| n.eq_ignore_ascii_case(&m.name))
        {
            return Ok(());
        }
        let Some(skel) = bind_pose.skeleton(&m.name) else {
            eprintln!("animated prop {}: no skel for {}", spec.class, m.name);
            return Ok(());
        };
        let Some(bank) = spec
            .bank
            .as_deref()
            .and_then(|b| crate::door::find_bank(&self.banks, b))
        else {
            eprintln!("animated prop {}: no bank", spec.class);
            return Ok(());
        };
        let Some(joints) = joints(m, skel) else {
            eprintln!(
                "animated prop {}: {} has a vertex with no bone",
                spec.class, m.name
            );
            return Ok(());
        };
        let posed = bind_pose
            .pose(m)
            .map_err(|e| anyhow::anyhow!(e))?
            .unwrap_or_else(|| m.clone());
        let alphas = alpha_table.model(&posed);
        let glb = crate::glb::model_to_glb_joints(&posed, &alphas, false, Some(&joints))?;
        let clips: Vec<PropClip> = slots(spec, bank)
            .into_iter()
            .filter_map(|(k, name)| {
                let d = bank.decode_clip(k).ok()?;
                Some(PropClip {
                    name,
                    frames: d.frames.max(1),
                })
            })
            .collect();
        let skin = skin_glb(skel, bank, &clips)?;
        let dir = out.join("animated");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{}.glb", m.name)), glb)?;
        std::fs::write(dir.join(format!("{}.skin.glb", m.name)), skin)?;
        self.written.push((spec.class.clone(), m.name.clone()));
        Ok(())
    }

    /// The model written for `class`, if any.
    pub fn drawn(&self, class: &str) -> Option<String> {
        self.written
            .iter()
            .find(|(c, _)| c.eq_ignore_ascii_case(class))
            .map(|(_, m)| m.clone())
    }
}

/// The skeleton-and-clips GLB of a rig's model (`animated/<model>.skin.glb`).
pub fn skin_glb(
    skel: &Skeleton,
    bank: &AnimBank<'_>,
    clips: &[PropClip],
) -> anyhow::Result<Vec<u8>> {
    let entries: Vec<(&str, bool)> = clips
        .iter()
        .enumerate()
        .map(|(k, c)| (c.name.as_str(), k == 0))
        .collect();
    crate::walker::skin_glb_of(skel, Some(bank), &entries, &[""], |_, _| {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_are_their_names() {
        for (name, key) in [
            ("AnimationName", PROP_ANIMATIONNAME),
            ("Animation", PROP_ANIMATION),
            ("AnimationTrigger", PROP_ANIMATIONTRIGGER),
            ("AttachTrigger", PROP_ATTACHTRIGGER),
            ("KillSoldierSound", PROP_KILLSOLDIERSOUND),
            ("TrackDeathOnAttach", PROP_TRACKDEATHONATTACH),
            ("DisableForCloneWars", PROP_DISABLEFORCLONEWARS),
            ("IdleDelay", PROP_IDLEDELAY),
        ] {
            assert_eq!(mungefs::fnv1a_lowercase(name), key, "{name}");
        }
    }

    /// Data-gated: BF2 Jabba's Palace. The rancor hangs in its pit at
    /// `hp_rancor`, Jabba on his bed at `hp_jabba`; the rancor grabs within
    /// 2 m of `hp_hotspot` and catches within 1.5 m of `hp_active` up to
    /// 2.5 s into `grab`; the trapdoor's two leaves ride their bones.
    #[test]
    fn tat3_rancor_jabba_and_trapdoor() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/TAT/TAT3.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: TAT3.LVL not present");
            return;
        };
        let inner = mungefs::open(&buf).unwrap();
        let chunks = mungefs::content_chunks(inner).unwrap();
        let mut entcs = Vec::new();
        for c in &chunks {
            if &c.tag == b"entc" {
                let e = EntityClass::parse(c).unwrap();
                entcs.push((
                    e.name.clone(),
                    e.props
                        .iter()
                        .filter(|p| crate::attached_effects::is_prop(p.key_hash))
                        .map(|p| (p.key_hash, p.value.clone()))
                        .collect::<Vec<_>>(),
                ));
            }
        }
        let (props, notes) = collect(&chunks, &entcs, &|_| None);
        assert!(notes.is_empty(), "{notes:?}");
        let rancor = props.rig("tat3_bldg_rancor").expect("rancor rig");
        let names: Vec<_> = rancor
            .clips
            .iter()
            .map(|c| (c.name.as_str(), c.frames))
            .collect();
        assert_eq!(names, [("idle", 298), ("grab", 261)]);
        assert_eq!(rancor.triggers.len(), 1);
        let t = &rancor.triggers[0];
        assert_eq!(
            (t.clip, t.hardpoint.as_str(), t.radius),
            (1, "hp_hotspot", 2.0)
        );
        assert!((Vec3::from_array(t.position) - Vec3::new(-0.018, 0.0, 6.394)).length() < 1e-2);
        assert_eq!(rancor.attaches.len(), 1);
        let a = &rancor.attaches[0];
        assert_eq!(
            (
                a.hardpoint.as_str(),
                a.radius,
                a.clip,
                a.soldier_animation.as_str(),
                a.time
            ),
            ("hp_active", 1.5, 1, "thrown_flail", 2.5)
        );
        assert_eq!(a.track[1].len(), 261);
        assert_eq!(rancor.kill_sound.as_deref(), Some("rancor_belch"));
        assert!(rancor.track_death);
        let pit: Vec<_> = props
            .attached
            .iter()
            .map(|a| (a.parent.as_str(), a.class.as_str(), a.hardpoint.as_str()))
            .collect();
        assert!(pit.contains(&("tat3_bldg_rancor_pit", "tat3_bldg_rancor", "hp_rancor")));
        assert!(pit.contains(&("tat3_prop_bed", "tat3_bldg_jabba", "hp_jabba")));
        let at = props
            .attached
            .iter()
            .find(|a| a.class == "tat3_bldg_rancor")
            .unwrap();
        assert!(
            (Vec3::from_array(at.translation) - Vec3::new(-41.27, -17.802, -0.571)).length() < 1e-2
        );
        let trap = props.rig("tat3_prop_trapdoor").expect("trapdoor rig");
        assert_eq!(
            trap.clips.iter().map(|c| c.frames).collect::<Vec<_>>(),
            [2, 206]
        );
        assert_eq!(trap.triggers[0].clip, 1);
        assert_eq!(trap.triggers[0].radius, 1.0);
        let mut leaves: Vec<_> = trap.prims.iter().map(|p| p.bone.as_str()).collect();
        leaves.sort_unstable();
        assert_eq!(leaves, ["bone_door1", "bone_door2"]);
        // Each leaf swings away during `open` and is back by its end.
        for p in &trap.prims {
            let (q0, t0) = p.frames[1][0];
            let swing = p.frames[1]
                .iter()
                .map(|(q, _)| Quat::from_array(*q).dot(Quat::from_array(q0)).abs())
                .fold(1.0f32, |a, b| if b < a { b } else { a });
            assert!(swing < 0.95, "{} never swings ({swing})", p.name);
            assert!((Vec3::from_array(p.frames[0][0].1) - Vec3::from_array(t0)).length() < 1e-3);
        }
    }
}
