//! Bakes first-person arm models (FPM) and animation tables into GLBs.
//!
//! Adds `hp_camera` from clip tracks when not present in skeleton (`analysis/re-first-person-arms.md`).

use crate::skin_glb::{skinned_to_glb, Clip};
use crate::soldier::{bake_flags, bake_textures_flagged, clip_to_tracks, rigid_or_skinned_model};
use anyhow::{anyhow, bail, Context, Result};
use glam::{Mat3, Quat};
use mungefs::model::Model;
use std::path::Path;

pub struct BakedFpm {
    pub name: String,
    pub bones: usize,
    pub clips: usize,
    pub missing: Vec<String>,
    pub textures: Vec<String>,
}

pub fn arm_model(inner: &[u8]) -> Result<Option<(Model, mungefs::skel::Skeleton)>> {
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    let skels: Vec<mungefs::skel::Skeleton> = chunks
        .iter()
        .filter(|c| &c.tag == b"skel")
        .filter_map(|c| mungefs::skel::parse_skel(c).ok())
        .collect();
    for m in chunks
        .iter()
        .filter(|c| &c.tag == b"modl")
        .filter_map(|c| Model::parse(c).ok())
    {
        let Some(s) = skels.iter().find(|s| s.name.eq_ignore_ascii_case(&m.name)) else {
            continue;
        };
        let has = |n: &str| s.bones.iter().any(|b| b.name.eq_ignore_ascii_case(n));
        if has("bone_shoulders") && has("hp_weapons") {
            return Ok(Some((m, s.clone())));
        }
    }
    Ok(None)
}

fn add_camera_bone(
    skel: &mut mungefs::skel::Skeleton,
    bank: &mungefs::anim::AnimBank<'_>,
) -> Result<()> {
    let cam = mungefs::anim::crc32_lower("hp_camera");
    if skel.bones.iter().any(|b| b.crc == cam) {
        return Ok(());
    }
    let root = mungefs::anim::crc32_lower("DummyRoot");
    let parent = skel
        .bones
        .iter()
        .position(|b| b.crc == root)
        .ok_or_else(|| anyhow!("{}: no dummyroot", skel.name))?;
    for name in manifest::fpm::clip_names(&bank.name) {
        let Some(k) = bank.clip_index(name) else {
            continue;
        };
        let decoded = bank.decode_clip(k).map_err(|e| anyhow!("{name}: {e}"))?;
        let Some(t) = decoded.tracks.iter().find(|t| t.bone_crc == cam) else {
            continue;
        };
        let (Some(r), Some(p)) = (t.rotations.first(), t.translations.first()) else {
            continue;
        };
        let m = Mat3::from_quat(Quat::from_array(*r).normalize());
        skel.bones.push(mungefs::skel::SkelBone {
            name: "hp_camera".into(),
            crc: cam,
            parent: Some(parent),
            rows: [m.x_axis.into(), m.y_axis.into(), m.z_axis.into()],
            t: *p,
        });
        return Ok(());
    }
    bail!("{}: no hp_camera bone and no clip carries one", skel.name)
}

pub fn bake_fpm_with_bank(
    fpm_lvl: &str,
    bank: &mungefs::anim::AnimBank<'_>,
    outdir: &str,
    donors: Option<&crate::tier::PcTextures>,
) -> Result<Option<BakedFpm>> {
    let buf = std::fs::read(fpm_lvl).with_context(|| fpm_lvl.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let Some((m, mut skel)) = arm_model(inner)? else {
        return Ok(None);
    };
    add_camera_bone(&mut skel, bank)?;
    let mut posed = m.clone();
    posed.bone_space = true;
    let sm = rigid_or_skinned_model(&posed, &skel)?;
    let mut clips = Vec::new();
    let mut missing = Vec::new();
    for name in manifest::fpm::clip_names(&bank.name) {
        let Some(k) = bank.clip_index(name) else {
            missing.push(name.to_string());
            continue;
        };
        let decoded = bank.decode_clip(k).map_err(|e| anyhow!("{name}: {e}"))?;
        let (mut tracks, _) = clip_to_tracks(&decoded, &skel, None);
        let mut seen = Vec::new();
        tracks.retain(|t| {
            let first = !seen.contains(&t.bone);
            seen.push(t.bone);
            first
        });
        clips.push(Clip {
            name: name.to_string(),
            fps: 30.0,
            frames: u32::from(decoded.frames),
            tracks,
        });
    }
    if clips.is_empty() {
        bail!("{}: no first-person clip in {}", m.name, bank.name);
    }
    let out = Path::new(outdir);
    let glb = manifest::fpm::glb_path(out, &m.name);
    std::fs::create_dir_all(glb.parent().expect("fpm dir"))?;
    std::fs::create_dir_all(out.join("textures"))?;
    std::fs::write(&glb, skinned_to_glb(&sm, &clips)?)?;
    let wanted: Vec<String> = sm
        .segments
        .iter()
        .map(|s| s.texture.to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    let textures = bake_textures_flagged(inner, &wanted, &bake_flags(&m), out, donors)?;
    Ok(Some(BakedFpm {
        name: m.name.clone(),
        bones: skel.bones.len(),
        clips: clips.len(),
        missing,
        textures,
    }))
}

/// Each donor tree's copy of an FPM LVL, comma-joined for [`crate::tier::PcTextures`]:
/// the same path under `FPM/`, in any case. `None` when no tree holds one.
fn fpm_donors(fpm_lvl: &str, donor_roots: &[String]) -> Option<String> {
    let lower = fpm_lvl.replace('\\', "/").to_ascii_lowercase();
    let rel = &fpm_lvl[lower.rfind("/fpm/")? + 1..];
    let found: Vec<String> = donor_roots
        .iter()
        .map(|root| crate::join_ci(Path::new(root), rel))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    (!found.is_empty()).then(|| found.join(","))
}

/// Bake every FPM LVL. `donor_roots` are the other copies' data roots: an LVL's copy
/// under the same relative path offers its larger arm textures to tier 2.
pub fn bake_fpms(
    fpm_lvls: &[String],
    anim_lvl: &str,
    outdir: &str,
    donor_roots: &[String],
) -> Result<()> {
    let abuf = std::fs::read(anim_lvl).with_context(|| anim_lvl.to_string())?;
    let ainner = mungefs::open(&abuf).map_err(|e| anyhow!("{e}"))?;
    let banks = mungefs::anim::find_banks(ainner).map_err(|e| anyhow!("{e}"))?;
    let bank = banks
        .iter()
        .find(|b| {
            b.name.eq_ignore_ascii_case(manifest::fpm::BF1_BANK)
                || b.name.eq_ignore_ascii_case(manifest::fpm::BF2_BANK)
        })
        .ok_or_else(|| anyhow!("{anim_lvl}: no first-person animation bank"))?;
    for lvl in fpm_lvls {
        let donors = fpm_donors(lvl, donor_roots)
            .map(|list| crate::tier::PcTextures::load(&list))
            .transpose()?;
        match bake_fpm_with_bank(lvl, bank, outdir, donors.as_ref())? {
            Some(b) => println!(
                "baked first-person arms {}: {} bones, {} clips (missing {:?}), textures {:?} -> {}/fpm",
                b.name, b.bones, b.clips, b.missing, b.textures, outdir
            ),
            None => println!("{lvl}: no first-person arm model"),
        }
    }
    Ok(())
}
