mod aim;
mod preview;

use super::look::far_cull;
use super::scene::spec_cull;
use super::upload::upload_cpu_texture;
use super::vehicles::{
    find_model, pose_class, upload_parts, ClassVis, ModelSource, TexSource, VehicleSides,
};
use super::walker_pose::WalkerRig;
use crate::assets::{load_glb_joints, load_skinned_glb, Scene};
use aim::GunAim;
use anyhow::Result;
use core::droideka::{Droideka, DroidekaClass, ROLLING};
use core::vehicle::walker::WalkerState;
use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, ParticleDraw, ParticleShape, Renderer, TextureHandle};
use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub struct DroidekaVisuals {
    models: Vec<ClassVis>,
    aims: Vec<GunAim>,
    preview: preview::Preview,
    by_class: [Vec<Option<usize>>; 2],
    posed: HashMap<usize, super::vehicles::PosedMeshes<(u32, usize)>>,
    /// The frame's eye, for how often a far droideka re-skins.
    pub eye: Option<glam::Vec3>,
    cull: Option<f32>,
    spec_cull: Option<f32>,
    bubble_texture: Option<TextureHandle>,
    bubble_spec: Option<TextureHandle>,
    bubble_color: [u8; 4],
    bubbles: HashMap<usize, (u32, bool, f32)>,
}

pub const BUBBLE_GROW: f32 = 0.15;
pub const BUBBLE_SHRINK: f32 = 0.15;
pub const BUBBLE_INSIDE_ALPHA: u8 = 128;
pub const BUBBLE_SEGMENTS: usize = 24;
pub const BUBBLE_RINGS: usize = 8;
/// The specular table's column for the bubble's tex-gen (B2 `FUN_0046e378`).
pub const BUBBLE_SPEC_POWER: f32 = 16.0;
/// The disc's `SpecularColor`, the same on both.
pub const BUBBLE_SPEC_COLOR: [u8; 4] = [216, 240, 255, 255];

/// [ours] The whole shell's share of the colour outside the rim, a little cloud
/// over the disc's clear centre (maintainer 2026-10-30: "just a little bit").
const BUBBLE_HAZE: f32 = 0.12;

/// The rim's share of the colour at `theta` from the pole that points at the
/// camera: B2 `FUN_0046ce30`'s vertex alpha.
pub fn rim(theta: f32) -> f32 {
    theta.sin().abs().powi(6)
}

/// One vertex of the bubble: where, its unit normal, its ring's angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BubbleVertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub theta: f32,
}

/// The bubble's grid: the disc's procedural sphere (24 x 8 quads, B2
/// `FUN_0046ce30`), its pole along the camera's back axis, so it turns with
/// the camera, not with the droideka (B2 `FUN_0046e378`). `[ours]` one layer
/// only, where the disc's facing bits leave both open and a PS2 frame's rim
/// fits one: the half toward the camera from outside, the half ahead of it
/// from inside, which keeps the draw at half the triangles.
pub fn bubble_grid(centre: Vec3, radius: f32, cam: &Camera, inside: bool) -> Vec<BubbleVertex> {
    let (mut right, mut back) = (cam.right(), -cam.forward());
    let first = if inside { BUBBLE_RINGS / 2 } else { 0 };
    let half = std::f32::consts::FRAC_PI_2;
    // [ours] From outside the pole points at the eye and the grid ends on the
    // sphere's true silhouette, with the rim's peak on it. With the disc's
    // pole along the view axis a close chase camera saw the rim off the edge
    // and the far side folded over it: a thick crescent (maintainer 2026-10-30).
    let mut edge = half;
    if !inside {
        let to_eye = cam.position - centre;
        let distance = to_eye.length();
        if distance > radius {
            back = to_eye / distance;
            right = Vec3::Y.cross(back).normalize_or(right);
            edge = (radius / distance).acos();
        }
    }
    let up = back.cross(right);
    let mut out = Vec::with_capacity((BUBBLE_RINGS / 2 + 1) * (BUBBLE_SEGMENTS + 1));
    for ring in first..=first + BUBBLE_RINGS / 2 {
        let theta = ring as f32 / BUBBLE_RINGS as f32 * std::f32::consts::PI;
        let at = theta * edge / half;
        let (st, ct) = at.sin_cos();
        for seg in 0..=BUBBLE_SEGMENTS {
            let phi = seg as f32 / BUBBLE_SEGMENTS as f32 * std::f32::consts::TAU;
            let (sp, cp) = phi.sin_cos();
            let normal = right * (st * cp) + up * (st * sp) + back * ct;
            out.push(BubbleVertex {
                pos: centre + normal * radius,
                normal,
                theta,
            });
        }
    }
    out
}

/// A [`bubble_grid`] row by row as triangles: six indices a quad.
pub fn bubble_indices() -> impl Iterator<Item = usize> {
    let row = BUBBLE_SEGMENTS + 1;
    (0..BUBBLE_RINGS / 2).flat_map(move |ring| {
        (0..BUBBLE_SEGMENTS).flat_map(move |seg| {
            let (a, b) = (ring * row + seg, ring * row + seg + 1);
            let (c, d) = (a + row, b + row);
            [a, c, b, b, c, d]
        })
    })
}

pub fn bubble_color(game: &str) -> [u8; 4] {
    if game.eq_ignore_ascii_case("bf2") {
        [96, 160, 192, 255]
    } else {
        [96, 160, 192, 192]
    }
}

pub fn bubble_scale(on: bool, t: f32) -> Option<f32> {
    if on {
        let s = (t / BUBBLE_GROW).min(1.0);
        Some(if s < 0.94 {
            s * 1.127_659_6
        } else {
            (1.0 - s) * 0.999_999 + 1.0
        })
    } else if t < BUBBLE_SHRINK {
        Some(1.0 - t / BUBBLE_SHRINK)
    } else {
        None
    }
}

pub fn spin_angle(d: &Droideka) -> f32 {
    2.0 * core::fmath::atan2(d.spin[1], d.spin[0])
}

pub fn roll_time(d: &Droideka, c: &DroidekaClass) -> f32 {
    let turn = spin_angle(d) / std::f32::consts::TAU;
    (turn - turn.floor()) * c.duration[usize::from(ROLLING)]
}

pub fn walker_state(d: &Droideka, c: &DroidekaClass) -> WalkerState {
    WalkerState {
        anim: d.state,
        clip_time: if d.state == ROLLING {
            roll_time(d, c)
        } else {
            d.clip_t
        },
        h: (1.0, 0.0),
        pitch: 0.0,
        roll: (1.0, 0.0),
        ..WalkerState::default()
    }
}

pub fn model_matrix(pos: Vec3, yaw: f32, d: &Droideka, c: &DroidekaClass) -> Mat4 {
    let base = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw + std::f32::consts::PI);
    if d.state != ROLLING {
        return base;
    }
    let centre = Vec3::new(0.0, c.ball_radius, 0.0);
    base * Mat4::from_translation(centre)
        * Mat4::from_rotation_z(d.lean.clamp(-1.0, 1.0).asin())
        * Mat4::from_translation(-centre)
}

impl DroidekaVisuals {
    pub fn load(
        r: &mut Renderer,
        scene: &Scene,
        assets_dir: &Path,
        walk: &crate::walk::WalkSim,
    ) -> Self {
        Self::load_teams(
            r,
            scene,
            assets_dir,
            &walk.teams,
            &scene.manifest.source_game,
        )
    }

    fn load_teams(
        r: &mut Renderer,
        scene: &Scene,
        assets_dir: &Path,
        teams: &[crate::walk::TeamTemplate; 2],
        game: &str,
    ) -> Self {
        let game = game.to_ascii_lowercase();
        let cull = far_cull(&scene.manifest.lighting).map(|c| c.world);
        let mut out = Self {
            cull,
            spec_cull: spec_cull(scene, cull),
            bubble_color: bubble_color(&game),
            ..Self::default()
        };
        let bakes = crate::droideka::bakes(&scene.dir, &game);
        if bakes.is_empty() {
            return out;
        }
        out.bubble_texture = upload_cpu_texture(
            r,
            &crate::assets::CpuTexture {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
                extra_mips: Vec::new(),
                detail: None,
            },
            "droideka_shield",
        )
        .map_err(|e| eprintln!("droideka shield: {e:#}"))
        .ok();
        // The disc's shared `specular` table (`bake specular`), BF2's on both.
        out.bubble_spec = pak::read(assets_dir.join("common_bf2/specular.ktx2"))
            .map_err(anyhow::Error::from)
            .and_then(|b| crate::assets::load_ktx2(&b))
            .and_then(|t| upload_cpu_texture(r, &t, "droideka_shield_specular"))
            .map_err(|e| eprintln!("droideka shield table: {e:#}"))
            .ok();
        let sides = VehicleSides::load(assets_dir, &game);
        let classes = crate::roster::load_classdb(&assets_dir.join("classes"), &game, &["cis"])
            .unwrap_or_else(|e| {
                eprintln!("droideka gun nodes: {e:#}");
                crate::classdb::ClassDb::default()
            });
        let mut loaded: HashMap<String, Option<usize>> = HashMap::new();
        let mut textures: HashMap<(Option<TexSource>, String), Option<TextureHandle>> =
            HashMap::new();
        for (t, team) in teams.iter().enumerate() {
            out.by_class[t] = team
                .classes
                .iter()
                .map(|cv| {
                    let key = cv.name.to_ascii_lowercase();
                    if let Some(&k) = loaded.get(&key) {
                        return k;
                    }
                    let k = bakes.get(&key).and_then(|b| {
                        match upload(r, scene, &sides, &mut textures, &b.record, &classes) {
                            Ok((m, aim)) => {
                                out.models.push(m);
                                out.aims.push(aim);
                                Some(out.models.len() - 1)
                            }
                            Err(e) => {
                                eprintln!("droideka {key}: {e:#}");
                                None
                            }
                        }
                    });
                    loaded.insert(key, k);
                    k
                })
                .collect();
        }
        out
    }

    pub fn append_test_class(
        &mut self,
        r: &mut Renderer,
        scene: &Scene,
        root: &Path,
        walk: &crate::walk::WalkSim,
        request: &crate::walk::ClassAssetRequest,
    ) {
        let crate::walk::ClassAssetRequest { team, class, game } = *request;
        self.by_class[team].resize(class, None);
        if walk.sim.classes[team][class].droideka.is_none() {
            self.by_class[team].push(None);
            return;
        }
        let teams = request.templates(walk);
        let mut added = Self::load_teams(r, scene, root, &teams, game);
        let offset = self.models.len();
        self.by_class[team].push(added.by_class[team].pop().flatten().map(|k| k + offset));
        self.models.extend(added.models);
        self.aims.extend(added.aims);
        if self.bubble_texture.is_none() {
            self.bubble_texture = added.bubble_texture;
            self.bubble_spec = added.bubble_spec;
            self.bubble_color = added.bubble_color;
        }
    }

    pub fn draws(&mut self, r: &mut Renderer, sim: &core::sim::Sim) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        for (slot, u) in sim.units.iter().enumerate() {
            let (Some(d), Some(c)) = (u.droideka.as_deref(), u.droideka_class()) else {
                continue;
            };
            if !u.alive || u.burst {
                continue;
            }
            let Some(k) = self
                .by_class
                .get(usize::from(u.team).min(1))
                .and_then(|t| t.get(usize::from(u.spec.class)))
                .copied()
                .flatten()
            else {
                continue;
            };
            let Some(palette) = self.palette(k, d, c, u.soldier.pitch) else {
                continue;
            };
            let model = &self.models[k];
            let due = self.eye.is_none_or(|eye| {
                super::vehicles::skin_due(slot, sim.tick, u.soldier.pos.distance(eye))
            });
            let meshes =
                match pose_class(r, &mut self.posed, slot, (u.gen, k), model, &palette, due) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("droideka {slot}: {e:#}");
                        continue;
                    }
                };
            let m = model_matrix(u.soldier.pos, u.soldier.yaw, d, c);
            model.draws(
                &mut out,
                Some(&meshes),
                m,
                (self.cull, self.spec_cull),
                None,
            );
        }
        out
    }

    fn palette(
        &self,
        model: usize,
        d: &Droideka,
        c: &DroidekaClass,
        pitch: f32,
    ) -> Option<Vec<Mat4>> {
        let rig = self.models.get(model)?.rig()?;
        Some(self.aims[model].palette(rig, d, c, pitch))
    }

    /// Overlay deployed droidekas' animated and aimed muzzle positions.
    pub fn fire_points(&self, sim: &core::sim::Sim, points: &mut [Option<Vec3>]) {
        for (slot, u) in sim.units.iter().enumerate() {
            let (Some(d), Some(c)) = (u.droideka.as_deref(), u.droideka_class()) else {
                continue;
            };
            if !u.alive || u.burst || d.no_fire() {
                continue;
            }
            let Some(k) = self
                .by_class
                .get(usize::from(u.team).min(1))
                .and_then(|t| t.get(usize::from(u.spec.class)))
                .copied()
                .flatten()
            else {
                continue;
            };
            let Some(palette) = self.palette(k, d, c, u.soldier.pitch) else {
                continue;
            };
            let rig = self.models[k].rig().unwrap();
            let name = crate::droideka::GUNS
                [usize::from(d.barrel.wrapping_add(1)) % crate::droideka::GUNS.len()];
            if let (Some(n), Some(point)) = (rig.skeleton.index_of(name), points.get_mut(slot)) {
                let frame = model_matrix(u.soldier.pos, u.soldier.yaw, d, c)
                    * palette[n]
                    * rig.skeleton.global_bind[n];
                *point = Some(frame.w_axis.truncate());
            }
        }
    }
}

impl DroidekaVisuals {
    /// The raised shields' two passes: the rim, flat colour under the
    /// sin^6 alpha (from inside, half the colour everywhere), and the additive
    /// specular table in `SpecularColor` over it. Charge and hits change
    /// neither: the disc has no ripple, no flash and no scroll on this shape.
    pub fn bubbles(&mut self, sim: &core::sim::Sim, cam: &Camera, sun: Vec3) -> Vec<ParticleDraw> {
        let now = sim.tick as f32 * core::TICK_DT;
        let mut out = Vec::new();
        for (slot, u) in sim.units.iter().enumerate() {
            let (Some(d), Some(c)) = (u.droideka.as_deref(), u.droideka_class()) else {
                self.bubbles.remove(&slot);
                continue;
            };
            let on = u.alive && d.shield.on;
            let settled = (
                u.gen,
                on,
                now - if on { BUBBLE_GROW } else { BUBBLE_SHRINK },
            );
            let e = self.bubbles.entry(slot).or_insert(settled);
            if e.0 != u.gen {
                *e = settled;
            } else if e.1 != on {
                *e = (u.gen, on, now);
            }
            let Some(f) = bubble_scale(on, now - e.2) else {
                continue;
            };
            if u.burst {
                continue;
            }
            let pos = u.droideka_point(c.shield.sphere.center);
            let radius = c.shield.sphere.radius;
            let inside = pos.distance_squared(cam.position) < radius * radius;
            let shell = bubble_grid(pos, radius * f, cam, inside);
            // Untextured on the GS: the halved byte IS the frame's (peak 48 80
            // 96), where this path's `gs_unit` reads 128 as 1.0: half again.
            let [cr, cg, cb, ca] = self.bubble_color;
            let [cr, cg, cb] = [cr, cg, cb].map(|b| b / 2);
            let colours: Vec<[u8; 4]> = shell
                .iter()
                .map(|v| {
                    let a = if inside {
                        BUBBLE_INSIDE_ALPHA
                    } else {
                        (f32::from(ca) * (BUBBLE_HAZE + (1.0 - BUBBLE_HAZE) * rim(v.theta))) as u8
                    };
                    [cr, cg, cb, a]
                })
                .collect();
            let tris = bubble_indices()
                .map(|i| (shell[i].pos, [0.0, 0.0], colours[i]))
                .collect();
            out.push(ParticleDraw {
                shape: ParticleShape::Shaded { tris },
                key: pos,
                color: self.bubble_color,
                texture: self.bubble_texture,
                additive: true,
            });
            if let Some(table) = self.bubble_spec {
                let right = cam.right();
                let uvs: Vec<[f32; 2]> = shell
                    .iter()
                    .map(|v| {
                        render::spec_texcoord(
                            BUBBLE_SPEC_POWER,
                            v.normal,
                            cam.position,
                            v.pos,
                            right,
                            sun,
                            true,
                        )
                    })
                    .collect();
                let tris = bubble_indices().map(|i| (shell[i].pos, uvs[i])).collect();
                out.push(ParticleDraw {
                    shape: ParticleShape::Mesh { tris },
                    key: pos,
                    color: BUBBLE_SPEC_COLOR,
                    texture: Some(table),
                    additive: true,
                });
            }
        }
        out
    }
}

fn upload(
    r: &mut Renderer,
    scene: &Scene,
    sides: &VehicleSides,
    textures: &mut HashMap<(Option<TexSource>, String), Option<TextureHandle>>,
    record: &manifest::ManifestDroideka,
    classes: &crate::classdb::ClassDb,
) -> Result<(ClassVis, GunAim)> {
    let Some(ModelSource::Side(i, model)) = find_model(&scene.manifest, sides, &record.geometry)
    else {
        anyhow::bail!("no side model for {}", record.geometry);
    };
    let dir = sides.dir(i);
    let (cpu, joints) = load_glb_joints(&dir.join("models").join(format!("{model}.glb")))?;
    if joints.is_empty() {
        anyhow::bail!("{model}: no JOINTS_0");
    }
    let skin_name = record.skin.as_deref().unwrap_or(&model);
    let a = load_skinned_glb(&dir.join("walkers").join(format!("{skin_name}.glb")))?;
    let aim = GunAim::new(
        &a.skeleton,
        classes.props_all(&record.class, "AimerNodeName"),
    );
    let rig = WalkerRig::new(a.skeleton, a.clips, &crate::droideka::as_walker(record));
    let (mut uploads, mut hits) = (0, 0);
    upload_parts(
        r,
        scene,
        sides,
        textures,
        &mut uploads,
        &mut hits,
        &cpu,
        Some(i),
        Some(rig),
        joints,
    )
    .map(|class| (class, aim))
}

#[cfg(test)]
mod tests;
