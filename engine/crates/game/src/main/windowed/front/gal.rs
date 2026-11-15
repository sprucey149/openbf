//! 3D spinning planet backdrop for the campaign battle screen (`bf2_gal1`).
//!
//! Matches PS2 `ifs_campaign_battle` + `ifs_campaign_main` zoom-2 state
//! (`MoveCameraToEntity("<planet>_camera")` + `SnapMapCamera()`, galaxy mesh hidden,
//! non-selected planets hidden), `GAL1.LVL` `anmg` animations (`rotate` / `tilt` 120s
//! period with ±5° axial tilt; `gal_rotate` 300s period on `gal_prp_galaxy`'s 32
//! `star`/`starblue` `Glow` beams), and PCSX2 Slot 8 `myg_camera` projection (`fov_y`).

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::Path;

use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, ParticleDraw, PointLight, Renderer};

const PLANET_GLOBES: &[&str] = &[
    "geo",
    "myg",
    "cor",
    "fel",
    "kas",
    "uta",
    "nab",
    "mus",
    "kam",
    "dea",
    "pol",
    "tat",
    "yav",
    "yav_giant",
    "hot",
];

/// `GAL1.LVL` `anmg` `rotate` / `tilt`: 0..2π over 120.0s, ±5° X-axis axial tilt.
const PLANET_SPIN_RAD_PER_SEC: f32 = TAU / 120.0;
const PLANET_AXIAL_TILT_RAD: f32 = 0.087_266_46;
/// `GAL1.LVL` `anmg` `gal_rotate`: 0..2π over 300.0s.
const GALAXY_SPIN_RAD_PER_SEC: f32 = TAU / 300.0;
/// `slot8` `eeMemory.bin` (`0x15ce370`): projection `Y_scale = 2.9299` -> `2 * atan(1/2.9299)`.
const PS2_GAL_FOV_Y_RAD: f32 = 0.657_782_2;

struct PlanetGroup {
    center: Vec3,
    draws: Vec<(usize, Mat4)>,
}

struct GalaxyStars {
    pivot: Vec3,
    base_matrix: Mat4,
    effects: Vec<manifest::ManifestAttachedEffect>,
}

/// Uploaded `bf2_gal1` scene containing only the campaign planet globes,
/// starfield/sky dome, per-planet camera poses, per-planet local lights, and
/// rotating `gal_prp_galaxy` glow stars.
pub(super) struct GalPlanetView {
    gpu: game::viewer::SceneGpu,
    planets: HashMap<String, PlanetGroup>,
    cameras: HashMap<String, Camera>,
    lights: HashMap<String, Vec<PointLight>>,
    galaxy: Option<GalaxyStars>,
    fx_lib: game::fx::Library,
    spin_angle: f32,
    galaxy_angle: f32,
}

fn planet_anim(name: &str) -> (f32, f32) {
    match name {
        // `GAL1.LVL` `anmg` `tilt`: `rhn`, `yav`, `nab`, `bes`
        "yav" | "nab" => (-PLANET_AXIAL_TILT_RAD, PLANET_SPIN_RAD_PER_SEC),
        // `gal_prp_death_star` has no rotation animation in `GAL1.LVL`.
        "dea" => (0.0, 0.0),
        // `GAL1.LVL` `anmg` `rotate`: all other campaign globes + `yav_giant`.
        _ => (PLANET_AXIAL_TILT_RAD, PLANET_SPIN_RAD_PER_SEC),
    }
}

/// Converts a PS2 `gal_prp_camera_pos` instance (`+Z` forward, `+Y` up, `-X` right)
/// into a [`render::Camera`] (`-Z` forward).
fn camera_from_instance(rotation: [[f32; 3]; 3], translation: [f32; 3]) -> Camera {
    let right = -Vec3::from(rotation[0]).normalize_or(Vec3::X);
    let fwd = Vec3::from(rotation[2]).normalize_or(Vec3::NEG_Z);
    let pitch = fwd.y.clamp(-1.0, 1.0).asin();
    let yaw = (-fwd.x).atan2(-fwd.z);
    let (sy, cy) = yaw.sin_cos();
    let right0 = Vec3::new(cy, 0.0, -sy);
    let up0 = right0.cross(fwd);
    let roll = (-right.dot(up0)).atan2(right.dot(right0));
    Camera {
        position: Vec3::from(translation),
        yaw,
        pitch,
        roll,
        fov_y: PS2_GAL_FOV_Y_RAD,
        near: 0.1,
        far: 2000.0,
    }
}

impl GalPlanetView {
    pub(super) fn load(assets_dir: &Path, r: &mut Renderer) -> Option<Self> {
        let dir = assets_dir.join("bf2_gal1");
        let mut scene = match game::assets::load_scene(&dir) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("campaign planet scene load failed: {e:#}");
                return None;
            }
        };

        let mut cameras = HashMap::new();
        let mut galaxy_inst = None;
        for inst in &scene.manifest.instances {
            let lower = inst.name.to_ascii_lowercase();
            if let Some(stem) = lower.strip_suffix("_camera") {
                cameras.insert(
                    stem.to_owned(),
                    camera_from_instance(inst.rotation, inst.translation),
                );
            } else if lower == "gal_prp_galaxy" {
                galaxy_inst = Some((
                    Vec3::from(inst.translation),
                    game::viewer::instance_matrix(inst.rotation, inst.translation),
                ));
            }
        }

        let galaxy_effects = scene
            .resolve_class("gal_prp_galaxy")
            .map(|c| c.effects.clone())
            .unwrap_or_default();
        let galaxy = galaxy_inst
            .filter(|_| !galaxy_effects.is_empty())
            .map(|(pivot, base_matrix)| GalaxyStars {
                pivot,
                base_matrix,
                effects: galaxy_effects,
            });
        let fx_lib = game::fx::Library::load(assets_dir, &dir, "bf2", &[]);

        let mut lights: HashMap<String, Vec<PointLight>> = HashMap::new();
        let authored = game::point_light::authored(&scene.manifest);
        for (raw, light) in scene.manifest.point_lights.iter().zip(authored) {
            let lower = raw.name.to_ascii_lowercase();
            if let Some(stem) = lower
                .strip_suffix("_star")
                .or_else(|| lower.strip_suffix("_amb"))
            {
                lights.entry(stem.to_owned()).or_default().push(light);
            }
        }

        // At campaign battle zoom, `gal_prp_galaxy`'s mesh and all unselected planets are hidden.
        scene.manifest.instances.retain(|inst| {
            if inst.class.eq_ignore_ascii_case("gal_prp_camera_pos") {
                return false;
            }
            let lower = inst.name.to_ascii_lowercase();
            PLANET_GLOBES.contains(&lower.as_str())
        });

        let gpu = match game::viewer::upload_scene(r, &scene) {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("campaign planet scene upload failed: {e:#}");
                return None;
            }
        };

        let mut planets: HashMap<String, PlanetGroup> = HashMap::new();
        for &(inst_idx, ref range) in &gpu.instance_draws {
            let Some(inst) = scene.manifest.instances.get(inst_idx as usize) else {
                continue;
            };
            let key = inst.name.to_ascii_lowercase();
            let center = Vec3::from(inst.translation);
            let group = planets.entry(key).or_insert_with(|| PlanetGroup {
                center,
                draws: Vec::new(),
            });
            for draw_idx in range.clone() {
                if let Some(cmd) = gpu.draws.get(draw_idx) {
                    group.draws.push((draw_idx, cmd.model));
                }
            }
        }

        Some(Self {
            gpu,
            planets,
            cameras,
            lights,
            galaxy,
            fx_lib,
            spin_angle: 0.0,
            galaxy_angle: 0.0,
        })
    }

    pub(super) fn advance_and_build(
        &mut self,
        planet: &str,
        dt: f32,
    ) -> (Camera, Vec<DrawCmd>, Vec<PointLight>, Vec<ParticleDraw>) {
        self.spin_angle = (self.spin_angle + dt * PLANET_SPIN_RAD_PER_SEC).rem_euclid(TAU);
        self.galaxy_angle = (self.galaxy_angle + dt * GALAXY_SPIN_RAD_PER_SEC).rem_euclid(TAU);
        let key = planet.to_ascii_lowercase();
        let mut draws = Vec::new();
        let mut push_group = |name: &str| {
            let Some(group) = self.planets.get(name) else {
                return;
            };
            let (tilt, spin_rate) = planet_anim(name);
            let spin = if spin_rate == 0.0 && tilt == 0.0 {
                Mat4::IDENTITY
            } else {
                let angle = if spin_rate == 0.0 { 0.0 } else { self.spin_angle };
                Mat4::from_translation(group.center)
                    * Mat4::from_rotation_x(tilt)
                    * Mat4::from_rotation_y(angle)
                    * Mat4::from_translation(-group.center)
            };
            for &(idx, base_model) in &group.draws {
                let mut cmd = self.gpu.draws[idx];
                cmd.model = spin * base_model;
                cmd.cull = None;
                cmd.lod = None;
                draws.push(cmd);
            }
        };
        push_group(&key);
        if key == "yav" {
            push_group("yav_giant");
        }

        let camera = self.cameras.get(&key).copied().unwrap_or_default();
        let lights = self.lights.get(&key).cloned().unwrap_or_default();
        let particles = self
            .galaxy
            .as_ref()
            .map(|gal| {
                let gal_spin = Mat4::from_translation(gal.pivot)
                    * Mat4::from_rotation_y(self.galaxy_angle)
                    * Mat4::from_translation(-gal.pivot);
                let gal_matrix = gal_spin * gal.base_matrix;
                gal.effects
                    .iter()
                    .filter_map(|eff| game::fx::prop_glow_draw(&self.fx_lib, gal_matrix, eff, None))
                    .collect()
            })
            .unwrap_or_default();
        (camera, draws, lights, particles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn myg_camera_basis_matches_ps2_slot8() {
        let cam = camera_from_instance(
            [
                [0.33887953, -0.000043987424, 0.94082975],
                [-0.1920871, 0.9789327, 0.06923405],
                [-0.92101204, -0.20418328, 0.3317318],
            ],
            [76.995, 2.201, -90.337],
        );
        let fwd = cam.forward();
        let right = cam.right();
        let up = right.cross(fwd);
        assert!(fwd.abs_diff_eq(Vec3::new(-0.921012, -0.204183, 0.331732), 1e-4));
        assert!(right.abs_diff_eq(Vec3::new(-0.338871, 0.0, -0.940833), 1e-4));
        assert!(up.abs_diff_eq(Vec3::new(-0.192102, 0.978933, 0.069192), 1e-4));
    }

    #[test]
    #[ignore = "requires GPU adapter and baked bf2_gal1"]
    fn gal1_renders_all_campaign_planets_centered_and_spinning() {
        let root = game::assets::baked_root();
        let mut r = Renderer::new_headless(640, 480).expect("headless GPU");
        let mut gal = GalPlanetView::load(&root, &mut r).expect("bf2_gal1 loads");

        let mut render_planet = |planet: &str, dt: f32| -> Vec<u8> {
            (r.time, r.water_time) = render::advance_clocks(r.time, r.water_time, dt);
            let (cam, draws, lights, particles) = gal.advance_and_build(planet, dt);
            assert!(!draws.is_empty(), "missing draws for planet {planet}");
            r.set_point_lights(lights);
            r.set_particles(particles);
            r.render(&cam, &draws, [0.0, 0.0, 0.0, 1.0]).expect("render");
            r.capture_rgba().expect("capture").0
        };

        let myg0 = render_planet("myg", 0.0);
        let myg1 = render_planet("myg", 2.0);
        assert_ne!(myg0, myg1, "planet globe should rotate over time");
        if let Some(out) = std::env::var_os("OPENBF_GAL_PREVIEW_PNG") {
            let png = render::encode_png_rgb(&myg0, 640, 480).expect("encode png");
            std::fs::write(out, png).expect("write png");
        }

        for planet in [
            "geo", "myg", "cor", "fel", "kas", "uta", "nab", "mus", "kam", "dea", "pol", "tat",
            "yav", "hot",
        ] {
            let rgba = render_planet(planet, 0.1);
            let lit = rgba
                .chunks_exact(4)
                .filter(|px| px[0] > 20 || px[1] > 20 || px[2] > 20)
                .count();
            assert!(
                lit > 5000,
                "planet {planet} only lit {lit} pixels — framing or lighting broken"
            );
        }
    }
}
