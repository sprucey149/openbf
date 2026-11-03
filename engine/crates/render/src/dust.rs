//! BF2 DustEffect's shared class pool and placed volumes, `analysis/re-hoth-atmosphere.md`.
use crate::{Camera, ParticleDraw, ParticleShape, TextureHandle};
use glam::{Mat4, Vec3};

#[derive(Debug, Clone)]
pub struct DustConfig {
    pub display_gain: f32,
    pub min_pos: Vec3,
    pub max_pos: Vec3,
    pub min_vel: Vec3,
    pub max_vel: Vec3,
    pub size: [f32; 2],
    pub lifetime: [f32; 2],
    pub color: [f32; 4],
    pub count: usize,
    pub max_distance: f32,
    pub height_scale: f32,
    pub radius_fade: [f32; 2],
    pub camera_distance: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Particle {
    pos: Vec3,
    vel: Vec3,
    size: f32,
    life: f32,
    age: f32,
}

pub struct Dust {
    pub config: DustConfig,
    pub texture: TextureHandle,
    pub instances: Vec<Mat4>,
    particles: Vec<Particle>,
    seed: u32,
    time: f64,
}

fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let hi = *seed >> 16;
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    f32::from_bits(0x3f80_0000 | ((hi | (*seed & 0xffff_0000)) >> 9)) - 1.0
}

fn spawn(config: &DustConfig, seed: &mut u32) -> Particle {
    let mut vector =
        |lo: Vec3, hi: Vec3| lo + (hi - lo) * Vec3::new(random(seed), random(seed), random(seed));
    let pos = vector(config.min_pos, config.max_pos);
    let vel = vector(config.min_vel, config.max_vel);
    Particle {
        pos,
        vel,
        size: config.size[0] + (config.size[1] - config.size[0]) * random(seed),
        life: (config.lifetime[0] + (config.lifetime[1] - config.lifetime[0]) * random(seed))
            .max(0.001),
        age: 0.0,
    }
}

pub fn lifetime_alpha(age: f32, life: f32) -> f32 {
    (age / (life * 0.3))
        .min((life - age) / (life * 0.5))
        .clamp(0.0, 1.0)
}

impl Dust {
    pub fn new(
        config: DustConfig,
        texture: TextureHandle,
        instances: Vec<Mat4>,
        mut seed: u32,
    ) -> Self {
        let particles = (0..config.count.min(4096))
            .map(|_| {
                let mut p = spawn(&config, &mut seed);
                p.age = p.life * random(&mut seed);
                p
            })
            .collect();
        Self {
            config,
            texture,
            instances,
            particles,
            seed,
            time: 0.0,
        }
    }

    pub fn draws(&mut self, camera: &Camera, time: f64) -> Vec<ParticleDraw> {
        let dt = (time - self.time).max(0.0) as f32;
        self.time = time;
        for p in &mut self.particles {
            p.age += dt;
            p.pos += p.vel * dt;
            if p.age >= p.life
                || (self.config.radius_fade[0] > 0.0
                    && p.pos.x * p.pos.x + p.pos.z * p.pos.z > self.config.radius_fade[1].powi(2))
            {
                *p = spawn(&self.config, &mut self.seed);
            }
        }
        let config = &self.config;
        let forward = camera.forward();
        let mut out = Vec::new();
        for instance in &self.instances {
            let mut origin = instance.w_axis.truncate();
            if config.camera_distance > 0.0 {
                origin.x = camera.position.x;
                origin.z = camera.position.z;
            }
            let radius = if config.radius_fade[0] > 0.0 {
                config.radius_fade[1] + config.size[1] * 0.5
            } else {
                config.max_pos.length().max(config.min_pos.length())
                    + config.max_vel.length().max(config.min_vel.length()) * config.lifetime[1]
                    + config.size[1] * 0.5
            };
            let depth = (origin - camera.position).dot(forward) + radius;
            let fade = ((config.max_distance - depth) / 200.0).min(1.0);
            if depth < 0.0 || fade <= 0.0 {
                continue;
            }
            for p in &self.particles {
                let local = Vec3::new(p.pos.x, p.pos.y * config.height_scale, p.pos.z);
                let pos = origin + local;
                let radius = p.pos.x.hypot(p.pos.z);
                let radial = if config.radius_fade[0] > 0.0 && radius > config.radius_fade[0] {
                    ((config.radius_fade[1] - radius)
                        / (config.radius_fade[1] - config.radius_fade[0]).max(0.001))
                    .clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let alpha = config.color[3] * lifetime_alpha(p.age, p.life) * radial * fade;
                if alpha <= 0.0 {
                    continue;
                }
                let half = p.size * 0.5;
                let scale = Vec3::new(1.0, config.height_scale, 1.0);
                let right = camera.right();
                let up = right.cross(forward).normalize_or_zero();
                out.push(ParticleDraw {
                    shape: ParticleShape::AtmosphereQuad {
                        pos,
                        a: right * scale * half,
                        b: up * scale * half,
                        gain: config.display_gain,
                    },
                    key: pos,
                    color: [config.color[0], config.color[1], config.color[2], alpha]
                        .map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8),
                    texture: Some(self.texture),
                    additive: false,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hoth_packet_uses_world_wind_and_squashes_the_camera_plane() {
        let config = DustConfig {
            display_gain: 2.0,
            min_pos: Vec3::new(-40.0, 0.0, -40.0),
            max_pos: Vec3::new(30.0, 15.0, 30.0),
            min_vel: Vec3::new(2.0, 0.0, 2.0),
            max_vel: Vec3::new(10.0, 1.0, 10.0),
            size: [15.0, 20.0],
            lifetime: [2.0, 3.0],
            color: [1.0; 4],
            count: 1,
            max_distance: 150.0,
            height_scale: 0.2,
            radius_fade: [-1.0; 2],
            camera_distance: -1.0,
        };
        let instance = Mat4::from_scale_rotation_translation(
            Vec3::new(-1.0, 1.0, -1.0),
            glam::Quat::IDENTITY,
            Vec3::new(48.259, 2.466, -42.439),
        );
        let mut dust = Dust::new(config, TextureHandle(0), vec![instance], 0);
        // PS2 Hoth slot 3, class pool 0x1b69310, FUN_003c3988 expansion.
        dust.particles[0] = Particle {
            pos: Vec3::new(19.517_792, 5.646_857, 38.039_543),
            vel: Vec3::new(2.204_276, 0.719_098_1, 6.748_229),
            size: 15.384_785,
            life: 2.131_808_5,
            age: 1.966_665_3,
        };
        let camera = Camera {
            position: Vec3::new(12.230_314, 1.223_096, -12.849_821),
            yaw: (-95.644_43f32).to_radians(),
            pitch: 10.024_16f32.to_radians(),
            ..Default::default()
        };
        let draws = dust.draws(&camera, 0.0);
        assert_eq!(draws.len(), 1);
        let ParticleShape::AtmosphereQuad { pos, a, b, gain } = draws[0].shape else {
            panic!("dust must preserve the compressed view-plane axes");
        };
        assert!(pos.abs_diff_eq(Vec3::new(67.776_794, 3.595_371_2, -4.399_456), 0.0001));
        assert!(a.abs_diff_eq(Vec3::new(-0.756_58, 0.0, 7.655_1), 0.002));
        assert!(b.abs_diff_eq(Vec3::new(-1.332_47, 1.515_03, -0.131_69), 0.002));
        assert_eq!(gain, 2.0);
        let before = pos;
        let after = dust.draws(&camera, 1.0 / 30.0)[0].key;
        assert!(after.x > before.x && after.z > before.z);
        assert_eq!(dust.draws(&camera, 1.0 / 30.0)[0].key, after);
    }

    #[test]
    fn life_fades_in_for_thirty_percent_and_out_for_half() {
        assert_eq!(lifetime_alpha(0.0, 10.0), 0.0);
        assert_eq!(lifetime_alpha(1.5, 10.0), 0.5);
        assert_eq!(lifetime_alpha(3.0, 10.0), 1.0);
        assert_eq!(lifetime_alpha(5.0, 10.0), 1.0);
        assert_eq!(lifetime_alpha(7.5, 10.0), 0.5);
        assert_eq!(lifetime_alpha(10.0, 10.0), 0.0);
    }
}
