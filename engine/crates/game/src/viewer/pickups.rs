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
use glam::{Mat4, Quat, Vec3};
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
    /// The model's sphere in its own space: every model draw gathers the
    /// local lights for it (`re-point-lights.md` §5), the item's own
    /// `LightOdf` omni among them (§4).
    sphere: Option<render::LitSphere>,
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
            sphere: crate::point_light::model_sphere(cpu, Mat4::IDENTITY),
        }
    }

    fn lit(&self, m: Mat4) -> Option<render::LitSphere> {
        self.sphere.map(|s| render::LitSphere {
            centre: m.transform_point3(s.centre),
            radius: s.radius * m.transform_vector3(Vec3::X).length(),
        })
    }
}

#[derive(Default)]
pub struct FlagVisuals {
    loose: Vec<Part>,
    carried: Vec<Part>,
    cull: Option<f32>,
    home_rotation: Quat,
    campaign: HashMap<String, (Vec<Part>, Vec<Part>)>,
    glows: HashMap<String, FlagGlow>,
    /// The map's `Effect("HDR")` is on, so a lens flare lands ×2
    /// (`glow_draws`).
    hdr: bool,
}

struct FlagGlow {
    color: [u8; 4],
    intensity: f32,
    radius: f32,
    distance: f32,
    omni: f32,
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
            hdr: scene.manifest.hdr.is_some(),
            ..Self::default()
        };
        if let Some(instance) = scene
            .manifest
            .instances
            .iter()
            .find(|i| i.class.eq_ignore_ascii_case("com_icon_spaceflag"))
        {
            out.home_rotation = Quat::from_mat4(&super::instance_matrix(
                instance.rotation,
                instance.translation,
            ));
        }
        let mut db = crate::classdb::ClassDb::default();
        if let Some(root) = scene.dir.parent() {
            let _ = db.load_map(&scene.dir);
            let _ = db.load(
                &root
                    .join("classes")
                    .join(format!("{}_ingame.ron", scene.manifest.source_game)),
            );
        }
        for class in db
            .names()
            .filter(|name| db.engine_base(name) == Some("flag"))
        {
            if let Some(light) = db.prop(class, "LightOdf") {
                let color: Vec<u8> = db
                    .prop(light, "Color")
                    .unwrap_or("255 255 255 255")
                    .split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect();
                if color.len() == 4 {
                    out.glows.insert(
                        class.to_owned(),
                        FlagGlow {
                            color: [color[0], color[1], color[2], color[3]],
                            intensity: db.prop_f32(light, "FlareIntensity").unwrap_or(0.0),
                            // EntityLight defaults, re-point-lights.md §10.
                            radius: db.prop_f32(light, "ConeInitialWidth").unwrap_or(1.0) * 0.5,
                            distance: db.prop_f32(light, "DrawDistance").unwrap_or(200.0),
                            omni: db.prop_f32(light, "OmniRadius").unwrap_or(0.0),
                        },
                    );
                }
            }
            out.campaign.insert(
                class.to_owned(),
                (
                    load_flag_parts(r, scene, db.prop(class, "GeometryName")),
                    load_flag_parts(r, scene, db.prop(class, "CarriedGeometryName")),
                ),
            );
        }
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
                let mut draw = match part.texture {
                    Some(texture) => draw_in(part.layer, part.mesh, texture, m),
                    None => DrawCmd::flat(part.mesh, [0.6, 0.6, 0.6, 1.0], m),
                }
                .with_cull(self.cull)
                .with_light(part.light)
                .with_lit(part.lit(m));
                draw.additive = part.additive;
                out.push(draw);
            }
        }
        out
    }

    /// The item light's lens flare (re-point-lights.md §10). The light ctor
    /// `FUN_00227150` bakes the flare's alpha byte as `A × FlareIntensity`
    /// (B2 139719–139724; FlareIntensity clamped to 0..1 by the setter,
    /// 140625–140632) and keeps `DrawDistance²` for the cull. The manager
    /// `FUN_00466d08` fades the star's centre alpha by
    /// `(1 − d²/D²) / 0.19` past `0.81·D²` (515485–515488); the ring keeps
    /// the full byte (`FUN_00468238` → `FUN_00468a28`, 515993–516002).
    /// `flaglight`'s 225 255 255 64 × 0.5 gives byte 32, GS alpha 16.
    ///
    /// On an HDR map the flare's bytes skip the HDR scale (no
    /// `FUN_0057fd50` read in `FUN_00466d08` / `FUN_00468238`) while the
    /// scene draws at ×0.5 and the bloom composite (pass priority 0x271a,
    /// B2 504385, after the managers) multiplies by 255/128
    /// (re-glow.md §2.2), so the flare lands ×2; the port's frame is not
    /// half-scale, so the alpha byte doubles here. [D] measured: the PS2
    /// tutorial frame lifts the holocron's surroundings by about +50 luma;
    /// this model predicts about +56 at the centre. PROVISIONAL: that the
    /// LensFlare stage draws before the composite is inferred from the
    /// pass priority, not read off the frame loop. trace: the caller of
    /// `FUN_00466ce8`.
    fn glow_draws(
        &self,
        walk: &crate::walk::WalkSim,
        camera: &render::Camera,
    ) -> Vec<render::ParticleDraw> {
        let (Some(campaign), Some(ctf)) = (&walk.campaign, &walk.sim.ctf) else {
            return Vec::new();
        };
        campaign
            .flag_objects()
            .filter_map(|(index, class)| {
                let def = self.glows.get(class)?;
                let flag = ctf.flags.get(index)?;
                let (_, matrix) = flag_pose(&walk.sim, flag, Quat::IDENTITY)?;
                let pos = matrix.w_axis.truncate();
                let d2 = pos.distance_squared(camera.position);
                let far = def.distance * def.distance;
                if d2 >= far || def.intensity <= 0.0 {
                    return None;
                }
                let alpha = f32::from(def.color[3]) * core::fmath::FloatExt::fmin(def.intensity, 1.0);
                let alpha = core::fmath::FloatExt::fmin(alpha * if self.hdr { 2.0 } else { 1.0 }, 255.0) as u8;
                let fade = if d2 > far * 0.80999994 {
                    (1.0 - d2 / far) * 5.2631564
                } else {
                    1.0
                };
                crate::holo::lens_flare(
                    pos,
                    def.radius,
                    alpha,
                    fade,
                    [def.color[0], def.color[1], def.color[2]],
                    camera,
                )
            })
            .collect()
    }
    /// The item's `LightOdf` EntityLight as a light (EntityItem ctor
    /// `FUN_00224fb8`, re-point-lights.md §4): a non-Static omni of the
    /// light class's Color and OmniRadius — `flaglight`'s 225 255 255, r 10
    /// — that lights the models and the terrain around the holocron.
    /// PROVISIONAL: at the item's origin, the light's own offset is not
    /// traced. trace: `FUN_00224fb8`'s light placement.
    fn item_lights(&self, walk: &crate::walk::WalkSim) -> Vec<(Vec3, [u8; 3], f32)> {
        let (Some(campaign), Some(ctf)) = (&walk.campaign, &walk.sim.ctf) else {
            return Vec::new();
        };
        campaign
            .flag_objects()
            .filter_map(|(index, class)| {
                let def = self.glows.get(class).filter(|d| d.omni > 0.0)?;
                let flag = ctf.flags.get(index)?;
                let (_, matrix) = flag_pose(&walk.sim, flag, Quat::IDENTITY)?;
                Some((
                    matrix.w_axis.truncate(),
                    [def.color[0], def.color[1], def.color[2]],
                    def.omni,
                ))
            })
            .collect()
    }
    pub fn campaign_draws(
        &self,
        sim: &core::sim::Sim,
        campaign: &crate::campaign_runtime::CampaignRuntime,
    ) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        let Some(ctf) = &sim.ctf else { return out };
        for (index, class) in campaign.flag_objects() {
            let Some(flag) = ctf.flags.get(index) else {
                continue;
            };
            let Some((loose, carried_parts)) = self.campaign.get(class) else {
                continue;
            };
            let Some((carried, matrix)) = flag_pose(sim, flag, Quat::IDENTITY) else {
                continue;
            };
            let parts = if carried { carried_parts } else { loose };
            for part in parts {
                let mut draw = match part.texture {
                    Some(texture) => draw_in(part.layer, part.mesh, texture, matrix),
                    None => DrawCmd::flat(part.mesh, [0.6, 0.6, 0.6, 1.0], matrix),
                }
                .with_cull(self.cull)
                .with_light(part.light)
                .with_lit(part.lit(matrix));
                draw.additive = part.additive;
                out.push(draw);
            }
        }
        out
    }
}

impl super::vehicles::VehicleVisuals {
    pub fn campaign_glows(
        &self,
        walk: &crate::walk::WalkSim,
        camera: &render::Camera,
    ) -> Vec<render::ParticleDraw> {
        self.flags.glow_draws(walk, camera)
    }
    pub fn campaign_lights(&self, walk: &crate::walk::WalkSim) -> Vec<(Vec3, [u8; 3], f32)> {
        self.flags.item_lights(walk)
    }
    pub fn campaign_flags(&self, walk: &crate::walk::WalkSim) -> Vec<DrawCmd> {
        walk.campaign
            .as_ref()
            .map(|c| self.flags.campaign_draws(&walk.sim, c))
            .unwrap_or_default()
    }
}

/// Script-created flags need no placed instance. Their class selects both
/// models, including the tutorial holocron in the common bake.
fn load_flag_parts(
    r: &mut Renderer,
    scene: &crate::assets::Scene,
    geometry: Option<&str>,
) -> Vec<Part> {
    let Some(geometry) = geometry else {
        return Vec::new();
    };
    let Some(root) = scene.dir.parent() else {
        return Vec::new();
    };
    let common = root.join(format!("common_{}", scene.manifest.source_game));
    let loaded = [scene.dir.as_path(), common.as_path()]
        .into_iter()
        .find_map(|dir| load_glb(&dir.join("models").join(format!("{geometry}.glb"))).ok());
    let Some(cpu) = scene.meshes.get(geometry).or(loaded.as_ref()) else {
        eprintln!("campaign flag model missing: {geometry}");
        return Vec::new();
    };
    let Ok(meshes) = upload_submeshes(r, cpu) else {
        return Vec::new();
    };
    let additive_bit = crate::fx::draw::additive_bit(crate::fx::emitter::Era::Bf2);
    meshes
        .into_iter()
        .enumerate()
        .map(|(k, (mesh, material, layer, flags))| {
            let texture = material.and_then(|name| {
                scene
                    .textures
                    .get(&name.to_ascii_lowercase())
                    .and_then(|t| super::upload::upload_cpu_texture(r, t, name).ok())
                    .or_else(|| {
                        [scene.dir.as_path(), common.as_path()]
                            .into_iter()
                            .find_map(|dir| {
                                upload_texture_file(r, &dir.join("textures"), name)
                                    .ok()
                                    .flatten()
                            })
                    })
            });
            Part::new(
                cpu,
                k,
                mesh,
                texture,
                layer,
                layer == DrawLayer::Blend && flags & additive_bit != 0,
            )
        })
        .collect()
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
                out.push(
                    base.with_cull(self.cull)
                        .with_light(part.light)
                        .with_lit(part.lit(m)),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GPU and BF2 common/Geonosis bakes"]
    fn tutorial_holocron_uploads_without_a_placed_space_flag() {
        let root = crate::assets::baked_root();
        let scene = crate::assets::load_scene(&root.join("bf2_geo1")).unwrap();
        let mut r = Renderer::new_headless(320, 240).unwrap();
        let art = FlagVisuals::load(&mut r, &scene, None);
        let (loose, carried) = art.campaign.get("com_item_holocron").expect("flag class");
        assert!(!loose.is_empty(), "invisible loose holocron");
        assert!(!carried.is_empty(), "invisible carried holocron");
        assert!(loose.iter().chain(carried).all(|p| p.texture.is_some()));
        let missions = root.parent().unwrap().join("missions");
        let mission = crate::mission::MissionDef::load_campaign(&missions, "geo1c_c").unwrap();
        let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
        let mut sim = core::sim::Sim::empty(1);
        sim.ctf = Some(core::sim::CtfState::new_ground_one_flag(
            1,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
        ));
        let mut campaign = crate::campaign_runtime::CampaignRuntime::new(
            mission.campaign.unwrap(),
            &layout,
            &scene.manifest,
            &[],
            Vec::new(),
            &sim,
        );
        campaign.w.flags.push(crate::campaign_runtime::FlagEnt {
            name: "tutorial_holocron".into(),
            class: "com_item_holocron".into(),
            ctf: 0,
            dead: false,
        });
        let draws = art.campaign_draws(&sim, &campaign);
        assert!(!draws.is_empty(), "live campaign flag produced no draws");
        let camera = render::Camera {
            position: glam::Vec3::new(0.0, 0.5, 3.0),
            ..Default::default()
        };
        r.render(&camera, &draws, [0.0, 0.0, 0.0, 1.0]).unwrap();
        let (rgba, w, h) = r.capture_rgba().unwrap();
        assert!(
            rgba.chunks_exact(4)
                .filter(|p| p[..3].iter().any(|c| *c > 20))
                .count()
                > 100,
            "holocron rendered blank"
        );
        let out = root.parent().unwrap().join("artifacts/tutorial-ui");
        std::fs::create_dir_all(&out).unwrap();
        image::RgbaImage::from_raw(w, h, rgba)
            .unwrap()
            .save(out.join("holocron.png"))
            .unwrap();
        let mut walk = crate::walk::WalkSim::new(1, glam::Vec3::ZERO, None);
        walk.sim = sim;
        walk.campaign = Some(campaign);
        let glows = art.glow_draws(&walk, &camera);
        assert_eq!(glows.len(), 1, "missing LightOdf lens flare");
        r.set_particles(glows);
        r.render(&camera, &draws, [0.0, 0.0, 0.0, 1.0]).unwrap();
        let (lit, w, h) = r.capture_rgba().unwrap();
        image::RgbaImage::from_raw(w, h, lit)
            .unwrap()
            .save(out.join("holocron-glow.png"))
            .unwrap();
        let mut campaign = walk.campaign.take().unwrap();
        let sim = walk.sim;
        campaign.w.flags[0].dead = true;
        assert!(art.campaign_draws(&sim, &campaign).is_empty());
    }

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

    /// Regression test for Mustafar (`mus1c_c`) orbital strike beacon:
    /// `mus_flag_bomb`'s loose model `mus1_icon_bomb` references textures
    /// (`tan4_prop_console`, `tan4_prop_console_fx`) that stock `MUS1.LVL`
    /// omits. `FlagVisuals::campaign_draws` must still emit flat-fallback draws
    /// instead of dropping the model while placed.
    #[test]
    #[ignore = "requires GPU and BF2 Mustafar bake"]
    fn mus1_beacon_draws_when_loose_and_when_carried() {
        let root = crate::assets::baked_root();
        let map = root.join("bf2_mus1");
        if !map.join("manifest.ron").exists() {
            return;
        }
        let scene = crate::assets::load_scene(&map).unwrap();
        let mut r = Renderer::new_headless(64, 64).unwrap();
        let art = FlagVisuals::load(&mut r, &scene, None);
        let (loose, carried) = art
            .campaign
            .get("mus_flag_bomb")
            .expect("mus_flag_bomb class loaded");
        assert!(!loose.is_empty(), "mus1_icon_bomb loose parts loaded");
        assert!(
            !carried.is_empty(),
            "mus_icon_bomb_carried parts loaded"
        );
        let missions = root.parent().unwrap().join("missions");
        let mission = crate::mission::MissionDef::load_campaign(&missions, "mus1c_c").unwrap();
        let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
        let mut sim = core::sim::Sim::empty(1);
        let unit = sim
            .spawn(&core::unit::UnitSpec::trooper(0, glam::Vec3::ZERO))
            .unwrap();
        sim.ctf = Some(core::sim::CtfState::new_ground_one_flag(
            1,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
        ));
        let mut campaign = crate::campaign_runtime::CampaignRuntime::new(
            mission.campaign.unwrap(),
            &layout,
            &scene.manifest,
            &[],
            Vec::new(),
            &sim,
        );
        campaign.w.flags.push(crate::campaign_runtime::FlagEnt {
            name: "bomb2".into(),
            class: "mus_flag_bomb".into(),
            ctf: 0,
            dead: false,
        });
        let loose_draws = art.campaign_draws(&sim, &campaign);
        assert_eq!(
            loose_draws.len(),
            loose.len(),
            "loose beacon must draw even when its textures are absent from MUS1.LVL"
        );
        sim.ctf.as_mut().unwrap().flags[0].status =
            core::sim::FlagStatus::Carried { carrier: unit };
        let carried_draws = art.campaign_draws(&sim, &campaign);
        assert_eq!(
            carried_draws.len(),
            carried.len(),
            "carried beacon must draw"
        );
    }
}
