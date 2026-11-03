//! Optional baked visual clips. Simulation readers keep the original body.

use super::{load_skinned_glb, SkinnedAsset};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::Path;

fn overlay(asset: &mut SkinnedAsset, visual: SkinnedAsset) -> Result<()> {
    if !visual.mesh.positions.is_empty() || !visual.mesh.indices.is_empty() {
        bail!("visual clip companion must not contain a mesh");
    }
    if asset.skeleton.names != visual.skeleton.names
        || asset.skeleton.parents != visual.skeleton.parents
        || asset.skeleton.local_bind != visual.skeleton.local_bind
    {
        bail!("visual clip companion changes the body skeleton");
    }
    let mut names = HashSet::new();
    let mut replacements = Vec::new();
    for clip in visual.clips {
        if !names.insert(clip.name.to_ascii_lowercase()) {
            bail!("duplicate visual clip {}", clip.name);
        }
        let matches: Vec<_> = asset
            .clips
            .iter()
            .enumerate()
            .filter(|(_, original)| original.name.eq_ignore_ascii_case(&clip.name))
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            bail!("visual clip {} does not resolve once", clip.name);
        }
        let index = matches[0];
        let original = &asset.clips[index];
        if original.frames != clip.frames
            || original.fps.to_bits() != clip.fps.to_bits()
            || !clip.fps.is_finite()
            || clip.fps <= 0.0
            || clip.frames == 0
        {
            bail!("visual clip {} changes timing", clip.name);
        }
        replacements.push((index, clip));
    }
    if replacements.is_empty() {
        bail!("empty visual clip companion");
    }
    for (index, clip) in replacements {
        asset.clips[index] = clip;
    }
    Ok(())
}

/// Load a renderer's body, retaining the original geometry and clip indices.
/// The separate meshless companion is produced by the bake from donor keys;
/// simulation setup calls `load_skinned_glb` and never reads this file.
pub fn load_visual_skinned_glb(path: &Path) -> Result<SkinnedAsset> {
    let mut asset = load_skinned_glb(path)?;
    let stem = path
        .file_stem()
        .context("visual body has no file stem")?
        .to_string_lossy();
    let companion = path.with_file_name(format!("{stem}.render-clips.glb"));
    if pak::is_file(&companion) {
        overlay(&mut asset, load_skinned_glb(&companion)?)
            .with_context(|| companion.display().to_string())?;
    }
    Ok(asset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bake::skin_glb::{skinned_to_glb, Bone, Clip, ClipTrack, SkinnedModel, SkinnedSegment};
    struct Dir(std::path::PathBuf);
    impl Dir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "openbf-visual-clips-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn bytes(changed: bool, fps: f32, bind: f32) -> Vec<u8> {
        let model = SkinnedModel {
            name: "body".into(),
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                translation: [bind, 0., 0.],
                rotation: [0., 0., 0., 1.],
            }],
            segments: if changed {
                vec![]
            } else {
                vec![SkinnedSegment {
                    positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]],
                    normals: vec![[0., 1., 0.]; 3],
                    uvs: vec![[0., 0.]; 3],
                    joints: vec![[0; 4]; 3],
                    weights: vec![[1., 0., 0., 0.]; 3],
                    triangles: vec![[0, 1, 2]],
                    ..Default::default()
                }]
            },
        };
        skinned_to_glb(
            &model,
            &[Clip {
                name: "reload".into(),
                fps,
                frames: 2,
                tracks: vec![ClipTrack {
                    bone: 0,
                    rotations: vec![
                        if changed {
                            [0., 1., 0., 0.]
                        } else {
                            [0., 0., 0., 1.]
                        };
                        2
                    ],
                    translations: vec![[0.; 3]; 2],
                }],
            }],
        )
        .unwrap()
    }
    #[test]
    fn visual_load_uses_the_companion_and_simulation_load_keeps_original_keys() {
        let dir = Dir::new("original");
        let path = dir.0.join("body.glb");
        std::fs::write(&path, bytes(false, 30., 0.)).unwrap();
        std::fs::write(dir.0.join("body.render-clips.glb"), bytes(true, 30., 0.)).unwrap();
        let sim = load_skinned_glb(&path).unwrap();
        let visual = load_visual_skinned_glb(&path).unwrap();
        assert_eq!(sim.clips[0].tracks[0].as_ref().unwrap()[0].rotation.w, 1.);
        assert_eq!(
            visual.clips[0].tracks[0].as_ref().unwrap()[0].rotation.y,
            1.
        );
        assert_eq!(sim.mesh.positions, visual.mesh.positions);
        assert_eq!(visual.mesh.positions.len(), 3);
    }
    #[test]
    fn changed_timing_or_skeleton_is_refused() {
        let dir = Dir::new("refusal");
        let path = dir.0.join("body.glb");
        std::fs::write(&path, bytes(false, 30., 0.)).unwrap();
        for (fps, bind) in [(15., 0.), (30., 1.)] {
            std::fs::write(dir.0.join("body.render-clips.glb"), bytes(true, fps, bind)).unwrap();
            assert!(load_visual_skinned_glb(&path).is_err());
        }
        std::fs::write(dir.0.join("body.render-clips.glb"), bytes(false, 30., 0.)).unwrap();
        assert!(
            load_visual_skinned_glb(&path).is_err(),
            "a companion cannot replace geometry"
        );
    }
}
