use super::{emitter::Lcg, FxAssets};
use glam::Vec3;
use render::{Camera, ParticleDraw, ParticleShape};
use std::path::Path;

struct Sample {
    pos: Vec3,
    life: f32,
    size: f32,
}

pub(super) struct SpaceDust {
    texture: String,
    values: Vec<(u32, f32)>,
    particles: Vec<Sample>,
    elapsed: f32,
    rng: Lcg,
    enabled: bool,
    speed: f32,
}

impl SpaceDust {
    pub(super) fn load(map: &Path) -> Option<Self> {
        let bytes = pak::read_to_string(map.join("space_dust.ron")).ok()?;
        let (texture, values): (String, Vec<(u32, f32)>) = ron::from_str(&bytes).ok()?;
        let required = [
            0x5c980bb3, 0x0dc9368a, 0x1c5d2dda, 0xa4c9a8d7, 0x7efe380d, 0xde77e423, 0x23b8373d,
            0x0e0c9efa, 0xe26ba52a, 0x006e0a20, 0xd9f25d54, 0x7bf23692,
        ];
        if !required
            .iter()
            .all(|key| values.iter().any(|(k, v)| k == key && v.is_finite()))
        {
            return None;
        }
        Some(Self {
            texture,
            values,
            particles: Vec::new(),
            elapsed: 0.0,
            rng: Lcg(42),
            enabled: false,
            speed: 0.0,
        })
    }

    fn value(&self, key: u32) -> f32 {
        self.values
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0.0, |(_, v)| *v)
    }

    pub(super) fn update(&mut self, camera: &Camera, velocity: Option<Vec3>, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let Some(velocity) = velocity else {
            self.particles.clear();
            self.elapsed = 0.0;
            self.enabled = false;
            return;
        };
        if !self.enabled {
            self.particles.clear();
            self.elapsed = 0.0;
            self.enabled = true;
        }
        let forward = camera.forward();
        let drift = -forward * self.value(0x0e0c9efa) * dt;
        for p in &mut self.particles {
            p.life -= dt;
            p.pos += drift;
        }
        self.particles
            .retain(|p| p.life >= 0.0 && (p.pos - camera.position).dot(forward) >= 0.0);
        self.speed = velocity.length();
        if self.speed <= 0.01 {
            self.elapsed = 0.0;
            return;
        }
        self.elapsed += dt;
        let interval = self.value(0xa4c9a8d7) * self.value(0x23b8373d) / self.speed;
        if interval <= 0.0 || self.elapsed <= interval || self.particles.len() >= 80 {
            return;
        }
        let count = (self.elapsed / interval) as usize;
        self.elapsed -= count as f32 * interval;
        let axis = velocity / self.speed;
        let mut side = forward.cross(axis).normalize_or_zero();
        if side == Vec3::ZERO {
            side = camera.right();
        }
        let up = axis.cross(side).normalize_or_zero();
        let centre = camera.position + axis * self.value(0x5c980bb3);
        let spread = self.value(0x0dc9368a);
        let hole = self.value(0x1c5d2dda);
        let low = self.value(0x7efe380d);
        let high = self.value(0xde77e423);
        // `FUN_00460928` records at most 20 dead slots per update (the
        // `iVar7 < 0x14` guard) even though each bank holds 80.
        for _ in 0..count.min(20).min(80 - self.particles.len()) {
            let mut x = (1.0 - 2.0 * self.rng.float()) * spread;
            let mut y = (1.0 - 2.0 * self.rng.float()) * spread;
            if x.abs() < hole && y.abs() < hole {
                x += if x < 0.0 { -hole } else { hole };
                y += if y < 0.0 { -hole } else { hole };
            }
            let size = low + (high - low) * self.rng.float();
            self.particles.push(Sample {
                pos: centre + side * x + up * y,
                life: 30000.0,
                size,
            });
        }
    }

    pub(super) fn draws(
        &self,
        camera: &Camera,
        assets: &mut dyn FxAssets,
        out: &mut Vec<ParticleDraw>,
    ) {
        if !self.enabled {
            return;
        }
        let Some(texture) = assets.texture(&self.texture) else {
            return;
        };
        let t = ((self.speed - self.value(0xd9f25d54))
            / (self.value(0x7bf23692) - self.value(0xd9f25d54)))
        .clamp(0.0, 1.0);
        let length = self.value(0xe26ba52a) * (1.0 - t) + self.value(0x006e0a20) * t;
        for p in &self.particles {
            let depth = (p.pos - camera.position).dot(camera.forward());
            let alpha = (1.0 + 254.0 * ((depth - 50.0) / 30.0).clamp(0.0, 1.0)) as u8;
            out.push(ParticleDraw {
                shape: ParticleShape::CappedRibbon {
                    head: p.pos,
                    head_half: p.size * 0.5,
                    tail: p.pos + camera.forward() * length,
                    tail_half: p.size * 0.5,
                },
                key: p.pos,
                color: [255, 255, 255, alpha],
                texture: Some(texture),
                additive: true,
            });
        }
    }
}

impl super::Fx {
    pub fn advance_space_dust(&mut self, walk: &crate::walk::WalkSim, dt: f32) {
        if let Some(dust) = self.space_dust.as_mut() {
            let velocity = walk
                .sim
                .unit(walk.player)
                .and_then(|u| u.mount)
                .and_then(|m| walk.sim.vehicle(m.vehicle))
                .filter(|v| v.kind == core::vehicle::VehicleKind::Flyer)
                .map(|v| v.vel);
            dust.update(&walk.camera(), velocity, dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dust() -> SpaceDust {
        SpaceDust {
            texture: "spacedust1".into(),
            values: vec![
                (0x5c980bb3, 150.0),
                (0x0dc9368a, 70.0),
                (0x1c5d2dda, 20.0),
                (0xa4c9a8d7, 0.02),
                (0x7efe380d, 0.1),
                (0xde77e423, 0.5),
                (0x23b8373d, 35.0),
                (0x0e0c9efa, 100.0),
                (0xe26ba52a, 2.0),
                (0x006e0a20, 12.0),
                (0xd9f25d54, 35.0),
                (0x7bf23692, 170.0),
            ],
            particles: vec![],
            elapsed: 0.0,
            rng: Lcg(42),
            enabled: false,
            speed: 0.0,
        }
    }

    #[test]
    fn flight_samples_recycle_and_pause_without_resampling() {
        let mut dust = dust();
        let camera = Camera::default();
        dust.update(&camera, Some(Vec3::ZERO), 1.0);
        assert!(dust.particles.is_empty());
        dust.update(&camera, Some(Vec3::NEG_Z * 100.0), 0.1);
        assert_eq!(dust.particles.len(), 14);
        let first = dust.particles[0].pos;
        let seed = dust.rng.0;
        dust.update(&camera, Some(Vec3::NEG_Z * 100.0), 0.0);
        assert_eq!(dust.particles[0].pos, first);
        assert_eq!(dust.rng.0, seed);
        dust.update(&camera, Some(Vec3::NEG_Z * 100.0), 0.1);
        assert!((dust.particles[0].pos.z - first.z - 10.0).abs() < 0.001);
        dust.update(&camera, None, 0.1);
        assert!(dust.particles.is_empty());
    }

    #[test]
    fn baked_space_maps_carry_the_native_streak_configuration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
        let mut checked = 0;
        for map in [
            "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
        ] {
            let dir = root.join(map);
            if !dir.join("space_dust.ron").is_file() {
                continue;
            }
            let dust = SpaceDust::load(&dir)
                .unwrap_or_else(|| panic!("{map}: the baked SpaceDust scope must parse"));
            assert_eq!(dust.texture, "spacedust1", "{map}: texture");
            for (key, want) in [
                (0x5c980bb3, 150.0),
                (0x0dc9368a, 70.0),
                (0x1c5d2dda, 20.0),
                (0xa4c9a8d7, 0.02),
                (0x7efe380d, 0.1),
                (0xde77e423, 0.5),
                (0x23b8373d, 35.0),
                (0x0e0c9efa, 100.0),
                (0xe26ba52a, 2.0),
                (0x006e0a20, 12.0),
                (0xd9f25d54, 35.0),
                (0x7bf23692, 170.0),
            ] {
                assert_eq!(dust.value(key), want, "{map}: {key:#010x}");
            }
            checked += 1;
        }
        if checked == 0 {
            assert!(
                std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
                "space assets required"
            );
            eprintln!("skipping: no baked space maps");
        } else {
            assert_eq!(checked, 6, "all six local space maps carry the scope");
        }
    }

    #[test]
    fn flight_update_takes_at_most_twenty_dead_slots() {
        let mut dust = dust();
        let camera = Camera::default();
        dust.update(&camera, Some(Vec3::NEG_Z * 100.0), 1.0);
        assert_eq!(dust.particles.len(), 20);
    }

    #[test]
    fn flight_bank_holds_the_native_eighty_samples() {
        let mut dust = dust();
        let mut camera = Camera::default();
        for _ in 0..10 {
            camera.position -= Vec3::Z * 10.0;
            dust.update(&camera, Some(Vec3::NEG_Z * 100.0), 0.1);
        }
        assert_eq!(dust.particles.len(), 80);
        camera.position -= Vec3::Z * 10.0;
        dust.update(&camera, Some(Vec3::NEG_Z * 100.0), 0.1);
        assert_eq!(
            dust.particles.len(),
            80,
            "recycled slots keep the bank full"
        );
    }
}
