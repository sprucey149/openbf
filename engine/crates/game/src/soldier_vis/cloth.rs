//! Per-wearer cloth state and dynamic ordinary meshes. A soldier whose bake
//! wrote a cloth companion opts in; any other keeps its rigid cloth parts.

use super::*;
use crate::cloth::{Definition, State};
use anyhow::{Context, Result};
use std::collections::HashSet;

struct Packet {
    key: (usize, usize, usize),
    vertices: Vec<render::Vertex>,
    indices: Vec<u32>,
    texture: Option<TextureHandle>,
    layer: render::DrawLayer,
    model: Mat4,
}

impl SoldierVisuals {
    pub(super) fn load_cloths(side: &Path, model: &str, asset: &SkinnedAsset) -> Vec<Definition> {
        let path = manifest::cloth::path(side, model);
        let read = || -> Result<Vec<Definition>> {
            let raw: Vec<manifest::cloth::Cloth> = ron::from_str(&pak::read_to_string(&path)?)?;
            let mut parts = HashSet::new();
            raw.into_iter()
                .map(|c| {
                    if c.part >= asset.mesh.submeshes.len() || !parts.insert(c.part) {
                        anyhow::bail!("cloth {}: invalid/duplicate part {}", c.name, c.part);
                    }
                    Definition::new(c, &asset.skeleton)
                })
                .collect()
        };
        if !pak::exists(&path) {
            return Vec::new();
        }
        read()
            .with_context(|| path.display().to_string())
            .unwrap_or_else(|e| {
                eprintln!("{e:#}; keeping rigid cloth fallback");
                Vec::new()
            })
    }

    pub(super) fn step_cloths(&mut self, walk: &WalkSim, dt: f32) {
        for (i, u) in walk.sim.units.iter().enumerate() {
            let body = self.asset_index(u.team, u.spec.class);
            let Some(v) = self.units.get_mut(i) else {
                continue;
            };
            if v.cloth_alive != Some(u.alive) || v.gen != u.gen || body != v.cloth_body {
                v.cloth_states.clear();
                v.cloth_body = body;
                v.cloth_alive = Some(u.alive);
            }
            if v.gen != u.gen
                || (!u.alive
                    && (u.burst || v.gone || v.dead_for > CORPSE_SECS || !v.animator.posed()))
            {
                v.cloth_states.clear();
                continue;
            }
            let Some(body) = body else { continue };
            let sa = &self.assets[body];
            if sa.cloths.is_empty() {
                continue;
            }
            let model = match (u.mount, v.seat_model) {
                (Some(_), Some(m)) => m,
                (Some(_), None) => {
                    v.cloth_states.clear();
                    continue;
                }
                _ => model_of(&u.soldier),
            };
            let compute = || -> std::sync::Arc<[Mat4]> {
                let pose = v.animator.pose(&sa.asset.skeleton);
                sa.asset.skeleton.palette(&pose.locals()).into()
            };
            let (posed_for, memo) = v.palette.get_or_init(|| (body, compute()));
            let palette = if *posed_for == body {
                memo.clone()
            } else {
                compute()
            };
            let globals: Vec<_> = palette
                .iter()
                .zip(&sa.asset.skeleton.global_bind)
                .map(|(p, b)| *p * *b)
                .collect();
            if v.cloth_states.is_empty() {
                v.cloth_states = sa
                    .cloths
                    .iter()
                    .map(|c| {
                        let seed = c.raw.name.bytes().fold(u.gen ^ i as u32, |h, b| {
                            (h ^ u32::from(b)).wrapping_mul(0x1000193)
                        });
                        State::new(c, model, &globals, seed)
                    })
                    .collect();
            }
            for (state, def) in v.cloth_states.iter_mut().zip(&sa.cloths) {
                state.step(def, model, &globals, dt);
            }
        }
    }

    /// Upload live cloth after the animation step and draw it with the same
    /// character lighting/material as its replaced skinned GLB part.
    pub fn cloth_draws(&mut self, r: &mut Renderer, walk: &WalkSim) -> Vec<render::DrawCmd> {
        let mut packets = Vec::new();
        for (i, sa, model, palette) in self.posed_for(walk, true) {
            if self.lod_view.choice(i) == super::lod::Choice::Low {
                continue;
            }
            let u = &walk.sim.units[i];
            let ai = self.asset_index(u.team, u.spec.class).unwrap();
            let globals: Vec<_> = palette
                .iter()
                .zip(&sa.asset.skeleton.global_bind)
                .map(|(p, b)| *p * *b)
                .collect();
            let row = &self.skins[usize::from(u.team.min(1))];
            let skin = row
                .get(usize::from(u.spec.class))
                .map_or(&[][..], Vec::as_slice);
            for (k, (def, state)) in sa
                .cloths
                .iter()
                .zip(&self.units[i].cloth_states)
                .enumerate()
            {
                let Some((_, tex, layer, _)) = sa.meshes.get(def.raw.part) else {
                    continue;
                };
                let tex = skin.get(def.raw.part).copied().flatten().or(*tex);
                packets.push(Packet {
                    key: (i, ai, k),
                    vertices: state.vertices(def, &globals),
                    indices: def
                        .raw
                        .triangles
                        .iter()
                        .flatten()
                        .copied()
                        .collect::<Vec<_>>(),
                    texture: tex,
                    layer: *layer,
                    model,
                });
            }
        }
        let mut out = Vec::new();
        for Packet {
            key,
            vertices,
            indices,
            texture,
            layer,
            model,
        } in packets
        {
            let uploaded =
                crate::cloth::upload(r, self.cloth_meshes.get(&key).copied(), &vertices, &indices);
            let mesh = match uploaded {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("cloth upload: {e}");
                    continue;
                }
            };
            self.cloth_meshes.insert(key, mesh);
            let mut cmd = match texture {
                Some(t) => render::DrawCmd::textured(mesh, t, model),
                None => render::DrawCmd::flat(mesh, [1.0; 4], model),
            }
            .with_light(render::LightPath::Character);
            cmd.layer = layer;
            cmd.lit = Some(crate::point_light::soldier_sphere(
                model.transform_point3(Vec3::ZERO),
            ));
            out.push(cmd);
        }
        out
    }
}
