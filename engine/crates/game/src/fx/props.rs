//! Static prop effects at their baked hardpoint frames, including repeat delays.
use super::{EffectDef, Era, Frame, Fx, Lcg, Library, LiveEffect};
use glam::{Mat3, Mat4, Vec3};
use std::sync::Arc;

impl Fx {
    pub(super) fn follow_station_damage(&mut self, sim: &core::sim::Sim) {
        let active = |station: usize, slot: usize| {
            let s = sim.stations.get(station)?;
            let effect = sim
                .station_classes
                .get(usize::from(s.class))?
                .damage_effects
                .get(slot)?;
            (!s.destroyed && effect.band.active(s.health.cur / s.health.max)).then_some((s, effect))
        };
        for live in &mut self.live {
            let Some((station, slot)) = live.station else {
                continue;
            };
            if active(station, slot).is_none() {
                live.stop();
                live.station = None;
            }
        }
        for (station, s) in sim
            .stations
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.destroyed)
        {
            for slot in 0..sim.station_classes[usize::from(s.class)]
                .damage_effects
                .len()
            {
                let Some((s, effect)) = active(station, slot) else {
                    continue;
                };
                if self
                    .live
                    .iter()
                    .any(|live| live.station == Some((station, slot)))
                {
                    continue;
                }
                let m = s.pose() * effect.local;
                let frame = Frame {
                    pos: m.w_axis.truncate(),
                    rot: Mat3::from_mat4(m),
                    vel: Vec3::ZERO,
                };
                if self.spawn(&effect.band.effect, frame) {
                    let live = self.live.last_mut().unwrap();
                    live.station = Some((station, slot));
                    for emitter in &mut live.instances {
                        emitter.size *= effect.scale;
                    }
                }
            }
        }
    }
}

pub(super) struct Glow {
    beam: render::Beam,
    color: [u8; 4],
}

impl Glow {
    fn new(def: &manifest::FxBeam, frame: Frame, era: Era, view: Option<render::FarFade>) -> Self {
        let axis = frame.rot * Vec3::from(def.direction);
        Self {
            beam: render::Beam {
                base: frame.pos + frame.rot * Vec3::from(def.position),
                axis,
                length: def.length,
                r0: def.initial_radius[0],
                r1: def.final_radius[0],
                soft: def.fade_length,
                visible: def.draw_distance,
                view,
                gs_rgb: match era {
                    Era::Bf1 => 0.5,
                    Era::Bf2 => 1.0,
                },
                top: def.fade_factor,
                cross: Some(render::Cross {
                    x: frame.rot.x_axis,
                    z: frame.rot.z_axis,
                    r0z: def.initial_radius[1],
                    r1z: def.final_radius[1],
                }),
            },
            color: def.color,
        }
    }

    pub(super) fn draw(&self) -> render::ParticleDraw {
        render::ParticleDraw {
            shape: render::ParticleShape::Beam(self.beam),
            key: self.beam.base + self.beam.axis * (self.beam.length * 0.5),
            color: self.color,
            texture: None,
            additive: true,
        }
    }
}

pub(super) struct Attached {
    def: Arc<EffectDef>,
    frame: Frame,
    delay: Option<[f32; 2]>,
    wait: f32,
    finished: bool,
    rng: Lcg,
    pub(super) live: Option<LiveEffect>,
}

impl Attached {
    pub(super) fn new(
        def: Arc<EffectDef>,
        frame: Frame,
        delay: Option<[f32; 2]>,
        seed: u32,
    ) -> Self {
        let mut rng = Lcg(seed);
        let wait = delay
            .filter(|[lo, hi]| *lo >= 0.0 && *hi > 0.0)
            .map_or(0.0, |[_, hi]| rng.float() * hi);
        let live = (wait <= 0.0).then(|| LiveEffect::new(def.clone(), frame, &mut rng));
        Self {
            def,
            frame,
            delay,
            wait,
            finished: false,
            rng,
            live,
        }
    }
    pub(super) fn step(&mut self, dt: f32) {
        if let Some(live) = &mut self.live {
            live.advance(dt);
            if live.is_dead() {
                self.live = None;
                match self.delay {
                    Some([lo, hi]) => {
                        let lo = lo.trunc().max(0.0);
                        self.wait = lo + self.rng.float() * (hi - lo).max(0.0);
                    }
                    None => self.finished = true,
                }
            }
            return;
        }
        if self.finished {
            return;
        }
        self.wait -= dt;
        if self.wait <= 0.0 {
            self.live = Some(LiveEffect::new(self.def.clone(), self.frame, &mut self.rng));
        }
    }
}

pub(super) fn frame(matrix: Mat4, local: &manifest::ManifestAttachedEffect) -> Frame {
    let axes = Mat3::from_cols_array_2d(&local.axes);
    let full = matrix
        * Mat4::from_cols(
            axes.x_axis.extend(0.0),
            axes.y_axis.extend(0.0),
            axes.z_axis.extend(0.0),
            Vec3::from(local.pos).extend(1.0),
        );
    Frame {
        pos: full.w_axis.truncate(),
        rot: Mat3::from_mat4(full),
        vel: Vec3::ZERO,
    }
}

// [ours] Follow the world's current distance policy, including its relaxed
// prop reach. None uses the camera's finite reach in render::light_beam.
fn view_fade(
    source_game: &str,
    lighting: &manifest::ManifestLighting,
    cull: bool,
) -> Option<render::FarFade> {
    crate::viewer::far_fade_from(source_game, lighting).or_else(|| {
        let reach = cull
            .then(|| crate::viewer::far_cull_from(lighting))
            .flatten()?;
        let begin = lighting.near_scene_range?[1];
        (begin.is_finite() && begin >= 0.0).then_some(render::FarFade {
            begin: begin.min(reach.world),
            end: reach.world,
        })
    })
}

pub(super) fn load(scene: &crate::assets::Scene, library: &Library) -> (Vec<Attached>, Vec<Glow>) {
    let mut out = Vec::new();
    let mut glows = Vec::new();
    let view = view_fade(
        &scene.manifest.source_game,
        &scene.manifest.lighting,
        crate::viewer::far_cull_enabled(),
    );
    let mut missing = std::collections::HashSet::new();
    let mut rng = Lcg(crate::classdb::fnv1a_lowercase(&scene.manifest.map));
    let paths = scene
        .manifest
        .foliage
        .as_ref()
        .map_or(&[][..], |f| &f.path_props[..]);
    for inst in scene.manifest.instances.iter().chain(paths) {
        let Some(class) = scene
            .manifest
            .classes
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&inst.class))
        else {
            continue;
        };
        let matrix = crate::viewer::instance_matrix(inst.rotation, inst.translation);
        for effect in &class.effects {
            if let Some(def) = library.get(&effect.effect) {
                out.push(Attached::new(
                    def.clone(),
                    frame(matrix, effect),
                    effect.delay,
                    rng.step(),
                ));
            } else if let Some(def) = library.beam(&effect.effect) {
                glows.push(Glow::new(def, frame(matrix, effect), library.era, view));
            } else if missing.insert(effect.effect.to_ascii_lowercase()) {
                eprintln!("prop effect {}: not in the map's library", effect.effect);
            }
        }
    }
    (out, glows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mustafar_console_damage_drives_its_authored_smoke() {
        let root = crate::assets::baked_root();
        let map = root.join("bf2_mus1");
        if !pak::exists(map.join("manifest.ron")) {
            return;
        }
        let (scene, _) = crate::assets::load_battle_scene(&map, "con").unwrap();
        let mut sim = core::sim::Sim::empty(1);
        scene.install_collision(&mut sim);
        crate::vehicle_pads::install(
            &mut sim,
            &scene.manifest,
            &scene.dir,
            &root.join("classes"),
            &["rep".into(), "cis".into()],
        )
        .unwrap();
        let station = sim
            .stations
            .iter()
            .position(|s| scene.manifest.instances[s.instance.unwrap() as usize].name == "dingdong")
            .unwrap();
        let s = sim.stations[station];
        let class = &sim.station_classes[usize::from(s.class)];
        assert_eq!(s.health.cur, 150.0);
        assert_eq!(s.radius, 0.0);
        assert_eq!(
            sim.collision.station(s.instance.unwrap()),
            Some(station as u16)
        );
        if class.damage_effects.is_empty() {
            eprintln!("SKIP: Mustafar must be rebaked for its damage hardpoints");
            return;
        }
        let mut fx = Fx::load(&root, &map, "bf2", &["rep", "cis"]);
        let smoke = |fx: &Fx| {
            fx.live
                .iter()
                .filter(|e| e.name() == Some("com_sfx_vehiclesmoke") && e.station.is_some())
                .map(|e| (e.frame.pos, e.instances[0].size))
                .collect::<Vec<_>>()
        };
        fx.ingest_sim(&sim);
        assert!(smoke(&fx).is_empty());
        let blast = core::weapon::ExplosionClass {
            damage: 32.0,
            damage_inner: 1.0,
            damage_outer: 5.0,
            ..Default::default()
        };
        sim.explode(&blast, s.sphere_centre, None, Some(0));
        assert_eq!(sim.stations[station].health.cur, 118.0);
        fx.ingest_sim(&sim);
        fx.ingest_sim(&sim);
        let active = smoke(&fx);
        assert_eq!(active.len(), 3);
        for ((pos, scale), (x, expected)) in
            active
                .into_iter()
                .zip([(-63.5076, 2.0), (-67.0152, 3.0), (-59.8152, 3.0)])
        {
            assert!(pos.abs_diff_eq(Vec3::new(x, 67.1399, 7.9246), 0.001));
            assert_eq!(scale, expected);
        }
        sim.repair_station(station as u16, 32.0);
        fx.ingest_sim(&sim);
        assert!(smoke(&fx).is_empty());
        let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
        let grenade =
            crate::roster::explosion_class(&db, "rep_weap_inf_thermaldetonator_exp").unwrap();
        sim.explode(&grenade, s.sphere_centre, None, Some(0));
        assert!(sim.stations[station].destroyed);
        assert_eq!(sim.collision.station(s.instance.unwrap()), None);
        fx.ingest_sim(&sim);
        assert!(smoke(&fx).is_empty());
    }

    #[test]
    fn hardpoint_and_instance_axes_both_place_the_effect() {
        let local = manifest::ManifestAttachedEffect {
            effect: "fixture".into(),
            hardpoint: "hp".into(),
            pos: [2.0, 0.0, 0.0],
            axes: Mat3::from_rotation_x(1.0).to_cols_array_2d(),
            delay: None,
        };
        let matrix = Mat4::from_rotation_translation(
            glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(10.0, 3.0, 5.0),
        );
        let f = frame(matrix, &local);
        assert!((f.pos - Vec3::new(10.0, 3.0, 3.0)).length() < 0.00001);
        assert!(
            (f.rot * Vec3::Y - matrix.transform_vector3(Mat3::from_rotation_x(1.0) * Vec3::Y))
                .length()
                < 0.00001
        );
    }
    #[test]
    fn finite_effects_repeat_only_when_the_hardpoint_authors_a_delay() {
        let mut emitter = super::super::tests::one_shot_emitter();
        emitter.transformer.lifetime = 0.1;
        let def = Arc::new(EffectDef::new(
            1,
            None,
            vec![emitter],
            super::super::Era::Bf2,
            &mut Lcg(1),
        ));
        let mut once = Attached::new(def.clone(), Frame::at(Vec3::ZERO), None, 1);
        once.step(1.0);
        assert!(once.live.is_none());
        once.step(10.0);
        assert!(once.live.is_none());
        let mut repeat = Attached::new(def, Frame::at(Vec3::ZERO), Some([2.0, 2.0]), 1);
        assert!(repeat.wait >= 0.0 && repeat.wait < 2.0);
        repeat.step(2.0);
        assert!(repeat.live.is_some());
        repeat.step(1.0);
        assert!(repeat.live.is_none());
        repeat.step(1.99);
        assert!(repeat.live.is_none());
        repeat.step(0.02);
        assert!(repeat.live.is_some());
    }

    #[test]
    fn halo_distance_takes_the_same_authored_fade_as_the_world() {
        let lighting = manifest::ManifestLighting {
            near_scene_range: Some([0.1, 80.0, 100.0]),
            far_scene_range: Some(0.0),
            ..Default::default()
        };
        for cull in [false, true] {
            assert_eq!(
                view_fade("bf2", &lighting, cull),
                Some(render::FarFade {
                    begin: 80.0,
                    end: 100.0
                })
            );
        }
        let extended = manifest::ManifestLighting {
            far_scene_range: Some(800.0),
            ..lighting
        };
        assert_eq!(view_fade("bf2", &extended, false), None);
        assert_eq!(
            view_fade("bf2", &extended, true),
            Some(render::FarFade {
                begin: 80.0,
                end: 800.0
            })
        );
        assert_eq!(
            view_fade("bf1", &extended, true),
            Some(render::FarFade {
                begin: 80.0,
                end: 800.0
            })
        );
        assert_eq!(view_fade("bf1", &extended, false), None);
        assert_eq!(
            view_fade("bf2", &manifest::ManifestLighting::default(), true),
            None
        );
    }
}
