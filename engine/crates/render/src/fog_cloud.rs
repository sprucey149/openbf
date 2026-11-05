use crate::rand::{step, unit as random};
use crate::{Camera, ParticleDraw, ParticleShape, TextureHandle};
use glam::Vec3;

// Bound the tiled work even if an input declares a huge range.
const MAX_CANDIDATES: u64 = 262_144;

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
    /// Whether this extent can exceed the runtime tile-work limit at some
    /// camera positions. Checked once when loading an authored cloud; the
    /// per-frame guard still bounds the actual tile span.
    pub fn may_exceed_tile_budget(&self) -> bool {
        let density = self.density.min(4096);
        let reach = self.range[1].max(0.0) + self.size;
        if density == 0 || reach < 0.0 || reach.is_nan() {
            return false;
        }
        if reach.is_infinite() {
            return true;
        }
        // An interval can straddle one more tile than its rounded-up width.
        // Use f64 for the estimate so even the largest finite f32 stays finite.
        let tiles = ((2.0 * f64::from(reach) / 256.0).ceil() + 1.0) as u64;
        tiles > MAX_CANDIDATES / tiles / u64::from(density)
    }

    pub fn alpha(&self, distance_squared: f32) -> f32 {
        let [near, far] = self.range.map(|v| v * v);
        if far <= near {
            return 0.0;
        }
        let d = (distance_squared - (near + far) * 0.5) * 2.0 / (far - near);
        (1.0 - d * d).max(0.0)
    }

    pub fn draws(&self, camera: &Camera, time: f64) -> Vec<ParticleDraw> {
        let density = self.density.min(4096);
        let reach = self.range[1].max(0.0) + self.size;
        let lo = ((camera.position - Vec3::splat(reach)) / 256.0).floor();
        let hi = ((camera.position + Vec3::splat(reach)) / 256.0).floor();
        if density == 0 || !lo.is_finite() || !hi.is_finite() || reach < 0.0 {
            return Vec::new();
        }
        let (x0, x1) = (lo.x as i32, hi.x as i32);
        let (z0, z1) = (lo.z as i32, hi.z as i32);
        let columns = u64::from(x0.abs_diff(x1)) + 1;
        let rows = u64::from(z0.abs_diff(z1)) + 1;
        if columns > MAX_CANDIDATES / rows / u64::from(density) {
            return Vec::new();
        }
        let mut rng = 0xc71c_2831u32;
        let drift = self
            .velocity
            .map(|v| (f64::from(v) * time).rem_euclid(256.0) as f32);
        let rotation = ((f64::from(self.rotation) * time + 1.0).rem_euclid(2.0) - 1.0) as f32;
        let mut out = Vec::new();
        for _ in 0..density {
            let x = (random(&mut rng) * 256.0 + drift[0]).rem_euclid(256.0);
            let z = (random(&mut rng) * 256.0 + drift[1]).rem_euclid(256.0);
            let phase = random(&mut rng) * 2.0;
            rng = step(rng);
            let sign = if rng & 0x0010_0000 != 0 { -1.0 } else { 1.0 };
            rng = step(rng);
            let angle = (phase + sign * rotation).rem_euclid(2.0) * std::f32::consts::PI;
            for ix in x0..=x1 {
                for iz in z0..=z1 {
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
        assert!(!f.may_exceed_tile_budget());
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

    #[test]
    fn oversized_range_is_rejected_before_visiting_the_ground() {
        let mut cloud = FogCloud {
            texture: TextureHandle(0),
            range: [0.0, 1.0e8],
            color: [255; 4],
            velocity: [0.0; 2],
            rotation: 0.0,
            height: 0.0,
            size: 28.0,
            density: 30,
            display_gain: 1.0,
            ground: Box::new(|_, _| panic!("oversized cloud must not visit tiles")),
        };
        assert!(cloud.may_exceed_tile_budget());
        assert!(cloud.draws(&Camera::default(), 0.0).is_empty());
        cloud.density = 0;
        assert!(!cloud.may_exceed_tile_budget());
        cloud.density = 4096;
        cloud.range[1] = 896.0;
        cloud.size = 0.0;
        assert!(!cloud.may_exceed_tile_budget(), "eight tiles fit exactly");
        cloud.range[1] = 896.001;
        assert!(
            cloud.may_exceed_tile_budget(),
            "a ninth tile can exceed the cap"
        );
        cloud.range[1] = f32::MAX;
        cloud.size = f32::MAX;
        assert!(
            cloud.may_exceed_tile_budget(),
            "overflowed reach is oversized"
        );
    }
}
