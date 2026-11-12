//! The items on the field drawn (card PWUP, `analysis/re-powerups.md` §4):
//! every `Sim::pickups` entry as its class's model (`GeometryName`, baked in
//! the common layer: `common_<game>/models/`), upright at the item, facing
//! the heading it was dropped with. BF1 stops drawing an item when 1 s of
//! life is left (hidden for its last 2 s, still there); BF2 draws it to the
//! end. PROVISIONAL: BF2's 1 s fade and the `player_shadow` blob under the
//! item are not drawn.
//!
//! Battle classes can be preloaded before the first frame; a class not
//! preloaded uploads lazily when its item first lies on the field. A model
//! that is not found is reported once and draws nothing.

use super::scene::draw_in;
use super::upload::{upload_submeshes, upload_texture_file};
use crate::assets::load_glb;
use anyhow::Result;
use glam::{Mat4, Quat};
use render::{DrawCmd, DrawLayer, LightPath, MeshHandle, Renderer, TextureHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// BF1 draws an item only while its life is over this (s; B1 `FUN_001f3cb0`).
pub const BF1_HIDE_LIFE: f32 = 1.0;

/// One drawn submesh of an item model.
struct Part {
    mesh: MeshHandle,
    texture: Option<TextureHandle>,
    layer: DrawLayer,
    light: LightPath,
    additive: bool,
}

impl Part {
    fn new(
        cpu: &crate::assets::CpuMesh,
        k: usize,
        mesh: MeshHandle,
        texture: Option<TextureHandle>,
        layer: DrawLayer,
        additive: bool,
    ) -> Self {
        Self {
            mesh,
            texture,
            layer,
            light: cpu.light_path(k),
            additive,
        }
    }
}

#[derive(Default)]
pub struct FlagVisuals {
    loose: Vec<Part>,
    carried: Vec<Part>,
    cull: Option<f32>,
    home_rotation: Quat,
}

fn flag_pose(
    sim: &core::sim::Sim,
    flag: &core::sim::Flag,
    home_rotation: Quat,
) -> Option<(bool, Mat4)> {
    match flag.status {
        core::sim::FlagStatus::AtHome | core::sim::FlagStatus::Dropped { .. } => Some((
            false,
            Mat4::from_rotation_translation(home_rotation, flag.current_pos),
        )),
        core::sim::FlagStatus::Carried { carrier } => {
            let unit = sim.unit(carrier)?;
            let (pos, rot) = match unit.mount {
                Some(mount) => {
                    let vehicle = sim.vehicle(mount.vehicle)?;
                    (vehicle.pos, vehicle.rot)
                }
                None => (unit.soldier.pos, Quat::from_rotation_y(unit.soldier.yaw)),
            };
            Some((
                true,
                Mat4::from_rotation_translation(rot, pos + rot * flag.carried_offset),
            ))
        }
    }
}

impl FlagVisuals {
    pub fn load(r: &mut Renderer, scene: &crate::assets::Scene, cull: Option<f32>) -> Self {
        let mut out = Self {
            cull,
            ..Self::default()
        };
        let Some(instance) = scene
            .manifest
            .instances
            .iter()
            .find(|i| i.class.eq_ignore_ascii_case("com_icon_spaceflag"))
        else {
            return out;
        };
        out.home_rotation = Quat::from_mat4(&super::instance_matrix(
            instance.rotation,
            instance.translation,
        ));
        let additive_bit = crate::fx::draw::additive_bit(
            if scene.manifest.source_game.eq_ignore_ascii_case("bf1") {
                crate::fx::emitter::Era::Bf1
            } else {
                crate::fx::emitter::Era::Bf2
            },
        );
        for (name, carried) in [
            ("com_icon_spaceflag", false),
            ("com_icon_spaceflag_carried", true),
        ] {
            let Some(cpu) = scene.meshes.get(name) else {
                continue;
            };
            let Ok(meshes) = upload_submeshes(r, cpu) else {
                continue;
            };
            let mut parts = Vec::new();
            for (k, (mesh, mat, layer, flags)) in meshes.into_iter().enumerate() {
                let texture = mat.and_then(|name| {
                    scene
                        .textures
                        .get(&name.to_ascii_lowercase())
                        .and_then(|t| super::upload::upload_cpu_texture(r, t, name).ok())
                });
                if texture.is_some() {
                    parts.push(Part::new(
                        cpu,
                        k,
                        mesh,
                        texture,
                        layer,
                        layer == DrawLayer::Blend && flags & additive_bit != 0,
                    ));
                }
            }
            if carried {
                out.carried = parts;
            } else {
                out.loose = parts;
            }
        }
        out
    }

    pub fn draws(&self, sim: &core::sim::Sim) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        let Some(ctf) = &sim.ctf else { return out };
        for flag in &ctf.flags {
            let Some((carried, m)) = flag_pose(sim, flag, self.home_rotation) else {
                continue;
            };
            let parts = if carried { &self.carried } else { &self.loose };
            for part in parts {
                if let Some(texture) = part.texture {
                    let mut draw = draw_in(part.layer, part.mesh, texture, m)
                        .with_cull(self.cull)
                        .with_light(part.light);
                    draw.additive = part.additive;
                    out.push(draw);
                }
            }
        }
        out
    }
}

/// The item models uploaded so far, by geometry name.
#[derive(Default)]
pub struct PickupVisuals {
    /// `common_<game>`, the map's game's; `None` draws nothing.
    dir: Option<PathBuf>,
    bf1: bool,
    models: HashMap<String, Vec<Part>>,
    textures: HashMap<String, Option<TextureHandle>>,
    cull: Option<f32>,
}

/// Is an item with `life` seconds left drawn (§4)?
pub fn item_drawn(bf1: bool, life: f32) -> bool {
    !bf1 || life > BF1_HIDE_LIFE
}

impl PickupVisuals {
    pub fn new(assets_dir: &Path, game: &str, cull: Option<f32>) -> Self {
        Self {
            dir: Some(assets_dir.join(format!("common_{game}"))),
            bf1: game.eq_ignore_ascii_case("bf1"),
            cull,
            ..Self::default()
        }
    }

    fn upload(&mut self, r: &mut Renderer, geometry: &str) -> Result<Vec<Part>> {
        let dir = self.dir.clone().unwrap_or_default();
        let cpu = load_glb(&dir.join("models").join(format!("{geometry}.glb")))?;
        let mut parts = Vec::new();
        for (k, (mesh, mat, layer, _)) in upload_submeshes(r, &cpu)?.into_iter().enumerate() {
            let texture = match mat.map(str::to_ascii_lowercase) {
                Some(name) => match self.textures.get(&name) {
                    Some(&t) => t,
                    None => {
                        let t = upload_texture_file(r, &dir.join("textures"), &name)?;
                        self.textures.insert(name, t);
                        t
                    }
                },
                None => None,
            };
            if texture.is_none() && super::route_untextured(mat) != super::Untextured::LookupFailed
            {
                continue;
            }
            parts.push(Part::new(&cpu, k, mesh, texture, layer, false));
        }
        Ok(parts)
    }

    /// Upload every item class `sim` can drop (card HITCH): at the battle's
    /// load, so the first powerup a death leaves reads no file in a frame.
    pub fn preload(&mut self, r: &mut Renderer, sim: &core::sim::Sim) {
        if self.dir.is_none() {
            return;
        }
        for c in &sim.item_classes {
            if c.geometry.is_empty() || self.models.contains_key(&c.geometry) {
                continue;
            }
            let parts = self.upload(r, &c.geometry).unwrap_or_else(|e| {
                eprintln!("pickups: {}: {e:#}", c.geometry);
                Vec::new()
            });
            self.models.insert(c.geometry.clone(), parts);
        }
    }

    /// The draws of every item of `sim` that is drawn now.
    pub fn draws(&mut self, r: &mut Renderer, sim: &core::sim::Sim) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        if self.dir.is_none() {
            return out;
        }
        for p in &sim.pickups {
            let Some(c) = sim.item_classes.get(usize::from(p.class)) else {
                continue;
            };
            if c.geometry.is_empty() || !item_drawn(self.bf1, p.life) {
                continue;
            }
            if !self.models.contains_key(&c.geometry) {
                let parts = self.upload(r, &c.geometry).unwrap_or_else(|e| {
                    eprintln!("pickups: {}: {e:#}", c.geometry);
                    Vec::new()
                });
                self.models.insert(c.geometry.clone(), parts);
            }
            let m = Mat4::from_rotation_translation(Quat::from_rotation_y(p.yaw), p.pos);
            for part in &self.models[&c.geometry] {
                let base = match part.texture {
                    Some(t) => draw_in(part.layer, part.mesh, t, m),
                    None => DrawCmd::flat(part.mesh, [0.6, 0.6, 0.6, 1.0], m),
                };
                out.push(base.with_cull(self.cull).with_light(part.light));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BF1 hides an item for its last 2 s (life 1 -> -1); BF2 never.
    #[test]
    fn bf1_hides_the_item_for_its_last_two_seconds() {
        assert!(item_drawn(true, 30.0));
        assert!(item_drawn(true, 1.01));
        assert!(!item_drawn(true, 1.0));
        assert!(!item_drawn(true, -0.5));
        assert!(item_drawn(false, 1.0));
        assert!(item_drawn(false, -0.9));
    }

    #[test]
    fn space_flag_uses_live_state_and_carrier_pose() {
        let mut sim = core::sim::Sim::empty(1);
        let mut flag = core::sim::Flag::new_space_neutral(glam::Vec3::new(1.0, 2.0, 3.0));
        let (carried, home) = flag_pose(&sim, &flag, Quat::IDENTITY).unwrap();
        assert!(!carried);
        assert_eq!(home.w_axis.truncate(), flag.home_pos);
        flag.current_pos = glam::Vec3::new(20.0, 30.0, 40.0);
        flag.status = core::sim::FlagStatus::Dropped {
            timer: 5.0,
            pickup_cooldown: 0.0,
        };
        let (carried, dropped) = flag_pose(&sim, &flag, Quat::IDENTITY).unwrap();
        assert!(!carried);
        assert_eq!(dropped.w_axis.truncate(), flag.current_pos);
        let unit = sim
            .spawn(&core::unit::UnitSpec::trooper(0, glam::Vec3::ZERO))
            .unwrap();
        let flyer = sim.spawn_vehicle(&core::vehicle::VehicleSpec {
            kind: core::vehicle::VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: glam::Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        });
        assert!(sim.enter_vehicle(unit, flyer, 0));
        flag.status = core::sim::FlagStatus::Carried { carrier: unit };
        let pos = glam::Vec3::new(-8.0, 5.0, 7.0);
        let rot = Quat::from_rotation_z(0.5) * Quat::from_rotation_y(1.0);
        let vehicle = sim.vehicle_mut(flyer).unwrap();
        (vehicle.pos, vehicle.rot) = (pos, rot);
        let (carried, pose) = flag_pose(&sim, &flag, Quat::IDENTITY).unwrap();
        assert!(carried);
        assert_eq!(pose.w_axis.truncate(), pos + rot * flag.carried_offset);
        assert!((pose.transform_vector3(glam::Vec3::Z) - rot * glam::Vec3::Z).length() < 0.001);
    }
}
