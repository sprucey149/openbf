//! The animated props drawn (`core::animated_prop`, `crate::animated_props`):
//! each rig's skinned model (`animated/<model>.glb` and its `.skin.glb`),
//! CPU-skinned to the sim prop's clip, time and blend, at the prop's world
//! matrix. Drawn with the vehicles so every draw site has them.
//!
//! The pose follows the disc's update (B2 `FUN_002312c0`): the clip is
//! sampled at the prop's time and the drawn pose moves toward it by the
//! blend once a tick, `pose = pose.blend(clip, blend)`, the clip whole once
//! the blend is 1. A frame between two ticks shows the last tick's pose.

use super::vehicles::{pose_class, upload_parts, ClassVis, PosedMeshes, VehicleSides};
use super::walker_pose::WalkerRig;
use crate::assets::{load_glb_joints, load_skinned_glb, Scene};
use crate::skin::{Clip, Pose, Skeleton};
use render::{DrawCmd, Renderer, TextureHandle};
use std::collections::HashMap;

struct Model {
    class: String,
    vis: ClassVis,
    skeleton: Skeleton,
    /// The rig's clips in slot order (`None` for one the skin lacks).
    clips: Vec<Option<Clip>>,
}

#[derive(Default)]
pub(super) struct PropVisuals {
    models: Vec<Model>,
    /// Per sim prop: its drawn pose and the tick it was made for.
    poses: HashMap<usize, (u64, Pose)>,
    posed: HashMap<usize, PosedMeshes<usize>>,
    cull: (Option<f32>, Option<f32>),
}

impl PropVisuals {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn load(
        r: &mut Renderer,
        scene: &Scene,
        sides: &VehicleSides,
        textures: &mut HashMap<(Option<super::vehicles::TexSource>, String), Option<TextureHandle>>,
        uploads: &mut usize,
        hits: &mut usize,
        cull: (Option<f32>, Option<f32>),
    ) -> Self {
        let mut out = Self {
            cull,
            ..Self::default()
        };
        let animated = scene.animated_props();
        for rig in &animated.rigs {
            let Some(model) = rig.model.as_deref() else {
                continue;
            };
            let dir = scene.dir.join("animated");
            let mut load = || -> anyhow::Result<Model> {
                let (cpu, joints) = load_glb_joints(&dir.join(format!("{model}.glb")))?;
                let skin = load_skinned_glb(&dir.join(format!("{model}.skin.glb")))?;
                let clips = rig
                    .clips
                    .iter()
                    .map(|c| {
                        skin.clips
                            .iter()
                            .find(|k| k.name.eq_ignore_ascii_case(&c.name))
                            .cloned()
                    })
                    .collect();
                let walker = WalkerRig::new_flyer(skin.skeleton.clone(), Vec::new(), None);
                let vis = upload_parts(
                    r,
                    scene,
                    sides,
                    textures,
                    uploads,
                    hits,
                    &cpu,
                    None,
                    Some(walker),
                    joints,
                )?;
                Ok(Model {
                    class: rig.class.clone(),
                    vis,
                    skeleton: skin.skeleton,
                    clips,
                })
            };
            match load() {
                Ok(m) => out.models.push(m),
                Err(e) => eprintln!("animated prop {}: {e:#}", rig.class),
            }
        }
        out
    }

    pub(super) fn draws(&mut self, r: &mut Renderer, sim: &core::sim::Sim) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        if self.models.is_empty() {
            return out;
        }
        for (k, p) in sim.props.iter().enumerate() {
            let Some(mi) = self
                .models
                .iter()
                .position(|m| m.class.eq_ignore_ascii_case(&p.setup.class))
            else {
                continue;
            };
            let model = &self.models[mi];
            let clip = usize::from(p.anim.unwrap_or(0));
            let target = match model.clips.get(clip).and_then(Option::as_ref) {
                Some(c) => c.sample(&model.skeleton, p.time * c.fps, clip == 0 && p.looping),
                None => Pose::bind(&model.skeleton),
            };
            let pose = match self.poses.get(&k) {
                Some((tick, last)) if *tick == sim.tick => last.clone(),
                Some((_, last)) if p.blend < 1.0 && last.0.len() == target.0.len() => {
                    last.blend(&target, p.blend.clamp(0.0, 1.0))
                }
                _ => target,
            };
            let palette = model.skeleton.palette(&pose.locals());
            self.poses.insert(k, (sim.tick, pose));
            match pose_class(r, &mut self.posed, k, mi, &model.vis, &palette, true) {
                Ok(meshes) => {
                    model
                        .vis
                        .draws(&mut out, Some(&meshes), p.setup.model, self.cull, None)
                }
                Err(e) => eprintln!("animated prop {k}: {e:#}"),
            }
        }
        out
    }
}
