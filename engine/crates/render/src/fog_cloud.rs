use crate::rand::{step, unit as random};
use crate::{Camera, ParticleDraw, ParticleShape, TextureHandle};
use glam::Vec3;

pub struct FogCloud {
    pub texture: TextureHandle,
    pub range: [f32; 2],
    pub color: [u8; 4],
    pub velocity: [f32; 2],
    pub rotation: f32,
    pub height: f32,
    pub size: f32,
    pub density: u32,
    pub display_gain: f32,
    pub ground: Box<dyn Fn(f32, f32) -> f32 + Send + Sync>,
}

impl FogCloud {
    pub fn alpha(&self, distance_squared: f32) -> f32 {
        let [near, far] = self.range.map(|v| v * v);
        if far <= near {
            return 0.0;
        }
        let d = (distance_squared - (near + far) * 0.5) * 2.0 / (far - near);
        (1.0 - d * d).max(0.0)
    }

    pub fn draws(&self, camera: &Camera, time: f64) -> Vec<ParticleDraw> {
        let mut rng = 0xc71c_2831u32;
        let drift = self
            .velocity
            .map(|v| (f64::from(v) * time).rem_euclid(256.0) as f32);
        let rotation = ((f64::from(self.rotation) * time + 1.0).rem_euclid(2.0) - 1.0) as f32;
        let reach = self.range[1].max(0.0) + self.size;
        let lo = ((camera.position - Vec3::splat(reach)) / 256.0).floor();
        let hi = ((camera.position + Vec3::splat(reach)) / 256.0).floor();
        let mut out = Vec::new();
        for _ in 0..self.density.min(4096) {
            let x = (random(&mut rng) * 256.0 + drift[0]).rem_euclid(256.0);
            let z = (random(&mut rng) * 256.0 + drift[1]).rem_euclid(256.0);
            let phase = random(&mut rng) * 2.0;
            rng = step(rng);
            let sign = if rng & 0x0010_0000 != 0 { -1.0 } else { 1.0 };
            rng = step(rng);
            let angle = (phase + sign * rotation).rem_euclid(2.0) * std::f32::consts::PI;
            for ix in lo.x as i32..=hi.x as i32 {
                for iz in lo.z as i32..=hi.z as i32 {
                    let px = ix as f32 * 256.0 + x;
                    let pz = iz as f32 * 256.0 + z;
                    let pos = Vec3::new(px, (self.ground)(px, pz) + self.height, pz);
                    let alpha = self.alpha(pos.distance_squared(camera.position));
                    if alpha <= 0.0 {
                        continue;
                    }
                    let mut color = self.color;
                    color[3] = (f32::from(color[3]) * alpha) as u8;
                    out.push(ParticleDraw {
                        shape: ParticleShape::Cloud {
                            pos,
                            half: self.size * std::f32::consts::FRAC_1_SQRT_2,
                            angle,
                            gain: self.display_gain,
                        },
                        key: pos,
                        color,
                        texture: Some(self.texture),
                        additive: false,
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_fades_at_both_authored_ranges_and_follows_the_ground() {
        let f = FogCloud {
            texture: TextureHandle(0),
            range: [90.0, 120.0],
            color: [168, 172, 180, 128],
            velocity: [5.0, 0.0],
            rotation: 0.1,
            height: 16.0,
            size: 28.0,
            density: 100,
            display_gain: 1.0,
            ground: Box::new(|_, _| -60.0),
        };
        assert_eq!(f.alpha(90.0 * 90.0), 0.0);
        assert_eq!(f.alpha(120.0 * 120.0), 0.0);
        assert_eq!(f.alpha((90.0 * 90.0 + 120.0 * 120.0) * 0.5), 1.0);
        let camera = Camera {
            position: Vec3::new(0.0, -60.0, 0.0),
            ..Camera::default()
        };
        let before = f.draws(&camera, 0.0);
        let after = f.draws(&camera, 1.0);
        assert!(!before.is_empty());
        assert_ne!(before, after);
        assert!(before.iter().all(|d| (d.key.y + 44.0).abs() < 0.001));
    }

    #[test]
    fn cloud_pattern_keeps_the_original_eight_step_random_stride() {
        let mut seed = 0xc71c_2831;
        for ([x, z, phase], reverse) in [
            ([9.064392, 146.35062, 0.10061216], false),
            ([92.33786, 43.474884, 1.5446286], true),
            ([133.65067, 237.14651, 0.9301851], true),
            ([239.27304, 121.83685, 1.8311534], false),
        ] {
            assert!((random(&mut seed) * 256.0 - x).abs() < 0.0001);
            assert!((random(&mut seed) * 256.0 - z).abs() < 0.0001);
            assert!((random(&mut seed) * 2.0 - phase).abs() < 0.000001);
            seed = step(seed);
            assert_eq!(seed & 0x0010_0000 != 0, reverse);
            seed = step(seed);
        }
    }
}
