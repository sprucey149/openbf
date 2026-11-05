//! Visual clip repairs assembled from the player's own animation channels.
//!
//! The original body and its simulation metadata stay untouched. A meshless
//! companion carries the corrected visual clip; only the soldier renderer
//! reads it. No reference animation keys are stored here.

use crate::skin_glb::{Clip, SkinnedModel};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

const TARGET: &str = "human_rifle_crouch_reload";
const DONOR: &str = "human_pistol_crouch_reload";
// (original, donor, already aligned): the complete twelve-bone rotation AND
// translation channels, with clip name, frame count and rate. The eight rows
// cover the measured retail PC BF1 faction and native lower rigs; no keys are stored here.
const CHANNELS: [(&str, &str, &str); 8] = [
    (
        "c66e721c4ac343537a95ce717e940668027decf2a3091287e1c46e7460990fcb",
        "a699aca767dd7f6acfe941c24bd96c793eb577a4ffbff0c879d1fec1e1affc6f",
        "9d2de70162c487b83bc640d3ecfb1a5c224263f0941193efb770ee089ac1dfe3",
    ),
    (
        "27d0e270e6bfc8ba294104008f00c26d38e692590e35eb13fcb098d8cf823b53",
        "da5e061020f87a18005fc9958f332ec995d6011c4ff8747ecc69e13cab56888f",
        "22bd7e484df9c59a537e91921a978daf9839a9d57d07972c25e479dccc05a81b",
    ),
    (
        "f017b5d72e7cda9a870db02e63395adca6386bf1185a01072234abc82d2dddd5",
        "75b8947e5ada716d3f6e6055bae78bdfaa75beb800649d0eae2ad0959326b68f",
        "073967f3e3d4fd178bc36e891a6ce25e84e5a2be6e97dfa0c5cac635b0b522c0",
    ),
    (
        "7531d4528d87419a55a50746cfb7c50224695ccb26b6f3a2ac9fe7c5da4858a4",
        "a797e04637c44e564c456ecc448b8f977d5fd5eb0ef090e969d9c9b0f1738bcc",
        "720cb8eb84271b8391c79b22b1c641e2175a6baabf699c28ce919bb94bfa109d",
    ),
    // Ewok pose rigs use their own rest translations. Their PC pistol lower
    // channels reproduce the PS2 rifle reload on both ewk_inf and ewk_inf2.
    (
        "fa4a7337e58a9985634eb9984fccb1b1aeacb288a78ea4dd3aa0cd3e81803b5d",
        "b941f7209b978dc0881e26023b61fabb38b35e53f13ebf5343f3468074af5f37",
        "4f7f83f3ce2371781aa8ac762d9e41718b03e33481593a820293493e9d4fe558",
    ),
    // Geonosian, Gungan and Jawa pose rigs, respectively. Each PC pistol
    // lower track exactly reproduces its PS2 rifle crouch reload.
    (
        "fbd3911e05780b161aac40e47ecfb3b4c53a5c54487e702fc3336b92fe5967ca",
        "2e32e64c2cd4b1ad738c5ea03e25c755773a0e7beaa93aa014172c3af1110d94",
        "9bbeaff0fa6e80baf56e4344b0c89437fb4882eedbad9d6490106369fd606205",
    ),
    (
        "83a9e35488f07ec26cc975b10d7e94cb8b264854170162649c51b6cda40871b1",
        "c8f053c00472cf2eaa0b0886c5b361fea16c9b4ab09eec1406896d03756cfa2c",
        "4c0168321022f9cf571576c04216092550c21cd5ecbde8a000c6cdc7db4bff15",
    ),
    (
        "fa78cafb21457ea041e5d4c01c833751dde6b4daff8594360164f424d1b5b062",
        "d6cb92eeb3f50f7e15b4e9aa84f528487f83bb5baf10b0b9ba523833774aa130",
        "3e5eb8a6d17bc103db9d851fa757baabadd61cefeb36fbbd4a31cbebb6087a9e",
    ),
];
const MOVING: [&str; 7] = [
    "bone_pelvis",
    "bone_r_thigh",
    "bone_r_calf",
    "bone_r_foot",
    "bone_l_thigh",
    "bone_l_calf",
    "bone_l_foot",
];
const LOWER: [&str; 12] = [
    "DummyRoot",
    "bone_pelvis",
    "bone_r_thigh",
    "bone_r_calf",
    "bone_r_foot",
    "bone_r_toe",
    "bone_l_thigh",
    "bone_l_calf",
    "bone_l_foot",
    "bone_l_toe",
    "bone_root",
    "hp_weapons",
];

fn bone(model: &SkinnedModel, name: &str) -> Result<usize> {
    let found: Vec<_> = model
        .bones
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name.eq_ignore_ascii_case(name))
        .map(|(i, _)| i)
        .collect();
    if found.len() != 1 {
        bail!(
            "{}: visual clip bone {name} resolves {} times",
            model.name,
            found.len()
        );
    }
    Ok(found[0])
}

fn fingerprint(model: &SkinnedModel, clip: &Clip) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(clip.name.as_bytes());
    hash.update([0]);
    hash.update(clip.frames.to_le_bytes());
    hash.update(clip.fps.to_le_bytes());
    for name in LOWER {
        let index = bone(model, name)?;
        let tracks: Vec<_> = clip.tracks.iter().filter(|t| t.bone == index).collect();
        if tracks.len() != 1 {
            bail!("{}: visual clip {name} has ambiguous tracks", clip.name);
        }
        let track = tracks[0];
        if track.rotations.len() != clip.frames as usize
            || track.translations.len() != clip.frames as usize
        {
            bail!("{}: incomplete lower track", clip.name);
        }
        hash.update(name.as_bytes());
        hash.update([0]);
        for value in track.rotations.iter().flatten() {
            hash.update(value.to_le_bytes());
        }
        for value in track.translations.iter().flatten() {
            hash.update(value.to_le_bytes());
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn copy_lower(model: &SkinnedModel, target: &Clip, donor: &Clip) -> Result<Clip> {
    if target.frames != donor.frames || target.fps.to_bits() != donor.fps.to_bits() {
        bail!("visual reload channel copy has incompatible timing");
    }
    let mut corrected = target.clone();
    for name in LOWER {
        let index = bone(model, name)?;
        let source: Vec<_> = donor.tracks.iter().filter(|t| t.bone == index).collect();
        let destination: Vec<_> = corrected
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.bone == index)
            .map(|(i, _)| i)
            .collect();
        if source.len() != 1 || destination.len() != 1 {
            bail!("{name}: incomplete visual reload channel pair");
        }
        let source = source[0];
        let destination = &mut corrected.tracks[destination[0]];
        if source.rotations.len() != target.frames as usize
            || source.translations.len() != target.frames as usize
        {
            bail!("{name}: incomplete visual reload channel data");
        }
        // The root and attachment channels must already agree. Never move
        // them to conceal a change in the actor's position or weapon hang.
        if matches!(name, "DummyRoot" | "bone_root" | "hp_weapons")
            && (source.rotations != destination.rotations
                || source.translations != destination.translations)
        {
            bail!("{name}: visual reload changes a root or attachment channel");
        }
        destination.rotations.clone_from(&source.rotations);
        destination.translations.clone_from(&source.translations);
    }
    Ok(corrected)
}

pub(super) fn corrected_clips(
    pc: bool,
    bf1: bool,
    model: &SkinnedModel,
    clips: &[Clip],
) -> Result<Option<Vec<Clip>>> {
    if !pc || !bf1 || clips.is_empty() {
        return Ok(None);
    }
    // This instruction addresses the human lower rig. Other rigs remain in
    // the alignment inventory and need their own proof/correction.
    if MOVING.iter().any(|name| {
        !model
            .bones
            .iter()
            .any(|b| b.name.eq_ignore_ascii_case(name))
    }) {
        return Ok(None);
    }
    let target = clips
        .iter()
        .find(|c| c.name == TARGET)
        .context("missing visual rifle crouch reload")?;
    let target_sha = fingerprint(model, target)?;
    // A geometry/animation mix may already supply the aligned lower clip.
    if CHANNELS.iter().any(|row| target_sha == row.2) {
        return Ok(None);
    }
    let donor = clips
        .iter()
        .find(|c| c.name == DONOR)
        .context("missing donor pistol crouch reload")?;
    let donor_sha = fingerprint(model, donor)?;
    if !CHANNELS
        .iter()
        .any(|row| target_sha == row.0 && donor_sha == row.1)
    {
        bail!(
            "{}: unrecognized PC BF1 lower reload channels; refusing visual repair",
            model.name
        );
    }
    Ok(Some(vec![copy_lower(model, target, donor)?]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin_glb::{Bone, ClipTrack};
    fn fixture() -> (SkinnedModel, Clip, Clip) {
        let mut names = LOWER.to_vec();
        names.push("upper_arm");
        let model = SkinnedModel {
            name: "body".into(),
            bones: names
                .iter()
                .map(|name| Bone {
                    name: (*name).into(),
                    parent: None,
                    translation: [0.; 3],
                    rotation: [0., 0., 0., 1.],
                })
                .collect(),
            segments: vec![],
        };
        let make = |name: &str, changed: bool| Clip {
            name: name.into(),
            fps: 30.,
            frames: 2,
            tracks: names
                .iter()
                .enumerate()
                .map(|(bone, name)| ClipTrack {
                    bone,
                    rotations: vec![
                        if changed && MOVING.contains(name) {
                            [0., 1., 0., 0.]
                        } else {
                            [0., 0., 0., 1.]
                        };
                        2
                    ],
                    translations: vec![[0.; 3]; 2],
                })
                .collect(),
        };
        (model, make(TARGET, false), make(DONOR, true))
    }
    #[test]
    fn lower_copy_keeps_upper_timing_and_original_clips() {
        let (model, target, donor) = fixture();
        let corrected = copy_lower(&model, &target, &donor).unwrap();
        assert_eq!(corrected.name, target.name);
        assert_eq!(corrected.frames, target.frames);
        assert_eq!(corrected.tracks[1].rotations, donor.tracks[1].rotations);
        assert_eq!(
            corrected.tracks.last().unwrap().rotations,
            target.tracks.last().unwrap().rotations
        );
        assert_ne!(target.tracks[1].rotations, donor.tracks[1].rotations);
    }
    #[test]
    fn native_paths_return_before_any_repair_and_unknown_donor_is_refused() {
        let (model, target, donor) = fixture();
        let clips = [target, donor];
        assert!(corrected_clips(false, true, &model, &clips)
            .unwrap()
            .is_none());
        assert!(corrected_clips(true, false, &model, &clips)
            .unwrap()
            .is_none());
        assert!(corrected_clips(true, true, &model, &clips).is_err());
    }
    #[test]
    fn root_and_attachment_motion_cannot_be_copied() {
        let (model, target, mut donor) = fixture();
        donor.tracks[0].translations[0][0] = 1.;
        assert!(copy_lower(&model, &target, &donor).is_err());
    }
    #[test]
    fn fingerprints_cover_toe_rotations_and_foot_translations_too() {
        let (model, _target, donor) = fixture();
        let before = fingerprint(&model, &donor).unwrap();
        let mut changed = donor.clone();
        let toe = bone(&model, "bone_l_toe").unwrap();
        changed
            .tracks
            .iter_mut()
            .find(|t| t.bone == toe)
            .unwrap()
            .rotations[0] = [0., 1., 0., 0.];
        assert_ne!(fingerprint(&model, &changed).unwrap(), before);
        let mut changed = donor;
        let foot = bone(&model, "bone_l_foot").unwrap();
        changed
            .tracks
            .iter_mut()
            .find(|t| t.bone == foot)
            .unwrap()
            .translations[0][1] = 1.;
        assert_ne!(fingerprint(&model, &changed).unwrap(), before);
    }

    // Decode the player's retail files rather than storing animation keys in
    // the test. Explicit inputs make a missing corpus a failure when requested.
    fn retail_clips(root: &std::path::Path, side: &str, name: &str) -> (SkinnedModel, Vec<Clip>) {
        let side_path = crate::join_ci(root, &format!("SIDE/{side}.LVL"));
        let common_path = crate::join_ci(root, "COMMON/ENG/COMMON.LVL");
        let side = crate::gameplay::read(side_path.to_str().unwrap()).unwrap();
        let common = crate::gameplay::read(common_path.to_str().unwrap()).unwrap();
        let inner = mungefs::open(&side).unwrap();
        let (_, mut skel) = super::super::find_soldier(inner, name).unwrap();
        crate::dlc_attachment::canonical_bind(inner, &mut skel).unwrap();
        let common_inner = mungefs::open(&common).unwrap();
        let classes = crate::anim_sets::bf2::side_classes(&side).unwrap();
        let mut rigs = mungefs::anim::find_rigs(inner).unwrap();
        rigs.extend(mungefs::anim::find_rigs(common_inner).unwrap());
        let pose = super::super::pose_rig(&classes, name, &rigs, false);
        if let Some(pose) = &pose {
            super::super::append_rig_attachment(&mut skel, &pose.rig, "hp_weapons").unwrap();
        }
        let model = SkinnedModel {
            name: name.into(),
            bones: super::super::bones_of(&skel),
            segments: vec![],
        };
        let banks = mungefs::anim::find_banks(mungefs::open(&common).unwrap()).unwrap();
        let clips = [TARGET, DONOR].map(|name| {
            let (bank, index) = crate::anim_sets::bf1::BANKS
                .iter()
                .find_map(|bank_name| {
                    banks
                        .iter()
                        .find(|b| b.name == *bank_name)
                        .and_then(|b| Some((b, b.clip_index(name)?)))
                })
                .expect("retail crouch reload");
            let decoded = bank.decode_clip(index).unwrap();
            let (tracks, _) = super::super::clip_to_tracks(&decoded, &skel, pose.as_ref());
            Clip {
                name: name.into(),
                fps: 30.,
                frames: u32::from(decoded.frames),
                tracks,
            }
        });
        (model, clips.into())
    }

    #[test]
    #[ignore = "requires OPENBF_RELOAD_PC1 and OPENBF_RELOAD_PS2 retail BF1 LVL roots"]
    fn retail_ewok_reload_repairs_to_ps2_and_refuses_changed_channels() {
        retail_reload_repairs(&[("EWK", "ewk_inf"), ("EWK", "ewk_inf2")]);
    }

    #[test]
    #[ignore = "requires OPENBF_RELOAD_PC1 and OPENBF_RELOAD_PS2 retail BF1 LVL roots"]
    fn retail_native_reload_repairs_to_ps2_and_refuses_changed_channels() {
        retail_reload_repairs(&[
            ("GEO", "gen_inf_geonosian"),
            ("GUN", "nab_inf_gungan"),
            ("DES", "tat_inf_jawa"),
            ("DES", "tat_inf_tusken"),
        ]);
    }

    fn retail_reload_repairs(soldiers: &[(&str, &str)]) {
        let pc = std::path::PathBuf::from(
            std::env::var_os("OPENBF_RELOAD_PC1").expect("PC BF1 LVL root"),
        );
        let ps2 = std::path::PathBuf::from(
            std::env::var_os("OPENBF_RELOAD_PS2").expect("PS2 BF1 LVL root"),
        );
        for &(side, name) in soldiers {
            let (model, clips) = retail_clips(&pc, side, name);
            let (native_model, native) = retail_clips(&ps2, side, name);
            let expected = fingerprint(&native_model, &native[0]).unwrap();
            let copied = copy_lower(&model, &clips[0], &clips[1]).unwrap();
            println!(
                "{name}: original={} donor={} aligned={} ps2={expected}",
                fingerprint(&model, &clips[0]).unwrap(),
                fingerprint(&model, &clips[1]).unwrap(),
                fingerprint(&model, &copied).unwrap()
            );
            assert_ne!(fingerprint(&model, &clips[0]).unwrap(), expected);
            assert_eq!(fingerprint(&model, &copied).unwrap(), expected);
            let fixed = corrected_clips(true, true, &model, &clips)
                .unwrap()
                .expect("retail PC BF1 lower reload needs repair");
            assert_eq!(fingerprint(&model, &fixed[0]).unwrap(), expected);
            assert_eq!(fixed[0].tracks.len(), clips[0].tracks.len());
            for original in &clips[0].tracks {
                if !LOWER
                    .iter()
                    .any(|name| model.bones[original.bone].name.eq_ignore_ascii_case(name))
                {
                    let after = fixed[0]
                        .tracks
                        .iter()
                        .find(|t| t.bone == original.bone)
                        .unwrap();
                    assert_eq!(after.rotations, original.rotations);
                    assert_eq!(after.translations, original.translations);
                }
            }
            let mut aligned = clips.clone();
            aligned[0] = fixed[0].clone();
            assert!(corrected_clips(true, true, &model, &aligned)
                .unwrap()
                .is_none());
            // Each fingerprint stream matters: mutate both target and donor,
            // including toes, roots and attachments outside the seven movers.
            for clip in 0..2 {
                for name in LOWER {
                    let bone = bone(&model, name).unwrap();
                    for rotation in [false, true] {
                        let mut changed = clips.clone();
                        let track = changed[clip]
                            .tracks
                            .iter_mut()
                            .find(|t| t.bone == bone)
                            .unwrap();
                        if rotation {
                            track.rotations[0][0] += 0.01;
                        } else {
                            track.translations[0][0] += 0.01;
                        }
                        assert!(
                            corrected_clips(true, true, &model, &changed).is_err(),
                            "accepted changed clip {clip} {name} rotation={rotation}"
                        );
                    }
                }
            }
        }
    }
}
