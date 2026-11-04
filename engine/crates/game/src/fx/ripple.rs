//! Flat-water ripples: the PS2's 100-record pool, spawned by bolt water hits
//! and water blasts, drawn as the `FXrippl1` ring; plus the splash bob lift.
//! Follows `analysis/re-fx.md` §15.

use super::Fx;
use glam::Vec3;
use render::{ParticleDraw, ParticleShape, TextureHandle};

pub const BOLT: (f32, f32) = (1.5, 0.01);
pub const TEXTURE: &str = "fxrippl1";
const CAP: usize = 100;
const SEEN_RANGE: f32 = 120.0;
const SPREAD: f32 = 1.5;
const COLUMNS: usize = 21;
const STEP: f32 = 0.314_159_27;
const BAND: [f32; 4] = [-0.375, -0.1275, 0.1275, 0.375];
const V: [f32; 4] = [0.0, 675.0 / 2048.0, 1372.0 / 2048.0, 1.0];
const SLOPE: f32 = 2.0;
const LIFT: f32 = 0.01;

pub type Vertex = (Vec3, [f32; 2], [u8; 4]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub water: [f32; 3],
    pub specular: [f32; 3],
    pub ambient: [f32; 3],
    pub sun: [f32; 3],
    pub light: Vec3,
}

fn srgb_byte(linear: f32) -> f32 {
    let c = linear.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * core::fmath::powf(c, 1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() / 255.0
}

impl Look {
    pub fn from_manifest(m: &manifest::Manifest) -> Option<Look> {
        let cfg = m.water.as_ref()?.config.as_ref()?;
        if cfg.ocean {
            return None;
        }
        let w = cfg.max_diffuse?;
        let l = &m.lighting;
        Some(Look {
            water: [w[0] / 255.0, w[1] / 255.0, w[2] / 255.0],
            specular: cfg
                .specular_color
                .map_or([0.0; 3], |s| [s[0] / 255.0, s[1] / 255.0, s[2] / 255.0]),
            ambient: l.ambient_sky.map(srgb_byte),
            sun: l.sun_color.map(srgb_byte),
            light: Vec3::from(l.sun_dir),
        })
    }

    fn shade(&self, d: f32, alpha: u8) -> [u8; 4] {
        let ch = |i: usize| {
            let c = self.water[i] * (self.ambient[i] + self.sun[i] * d)
                + self.sun[i] * (d * d) * (d * d);
            (c * 255.0).clamp(0.0, 255.0) as u8
        };
        [ch(0), ch(1), ch(2), alpha]
    }

    fn edge(&self) -> [u8; 4] {
        let ch = |i: usize| ((self.water[i] * 128.0).round() as u8).saturating_mul(2);
        [ch(0), ch(1), ch(2), 0]
    }

    fn wake_shade(&self, water: [f32; 3], d: f32, alpha: u8) -> [u8; 4] {
        let rgb = std::array::from_fn::<_, 3, _>(|i| {
            let ambient = water[i] * self.ambient[i];
            let diffuse = water[i] * self.sun[i] * d;
            let specular = self.sun[i] * d * d * d * d;
            let lit = ambient + diffuse - ambient * diffuse;
            ((lit + specular - lit * specular) * 255.0).clamp(0.0, 255.0) as u8
        });
        [rgb[0], rgb[1], rgb[2], alpha]
    }
}

pub fn ring(look: &Look, centre: Vec3, radius: f32, life: f32, age: f32) -> Vec<Vertex> {
    surface_ring(look, centre, radius, life, age, None)
}

pub fn wake(
    look: &Look,
    centre: Vec3,
    radius: f32,
    life: f32,
    age: f32,
    velocity: Vec3,
) -> Vec<Vertex> {
    surface_ring(look, centre, radius, life, age, Some(velocity))
}

fn surface_ring(
    look: &Look,
    centre: Vec3,
    radius: f32,
    life: f32,
    age: f32,
    velocity: Option<Vec3>,
) -> Vec<Vertex> {
    let a = age / life;
    let q = if a < 0.15 {
        a * 6.666_666_5
    } else {
        (1.0 - a) * 1.176_470_5
    };
    if q < 0.0 {
        return Vec::new();
    }
    let mut p = (q * (1.0 / 3.0) * life).min(1.0);
    let mut water = look.water;
    let mut direction = Vec3::X;
    let mut horizontal_speed = 0.0;
    if let Some(velocity) = velocity {
        let speed = velocity.length();
        if speed <= 0.0 {
            return Vec::new();
        }
        p *= (speed * (2.0 / 3.0)).min(1.0);
        let horizontal = Vec3::new(velocity.x, 0.0, velocity.z);
        horizontal_speed = horizontal.length();
        direction = horizontal.try_normalize().unwrap_or(Vec3::X);
        let relative = horizontal_speed * (2.0 / 3.0);
        let strength = (relative - 1.0) * (relative - 1.0) * 4.0 + 0.4;
        p *= strength / (strength + 1.0);
        if relative >= 1.0 && p > 0.3 {
            let blend = (p - 0.3) * 1.428_571_5;
            water = std::array::from_fn(|i| water[i] + (look.specular[i] - water[i]) * blend);
        }
    }
    let flat = f32::from_bits(0x3f00_a3d7);
    let edge = if velocity.is_some() {
        let rgb = water.map(|v| ((v * 128.0).round() as u8).saturating_mul(2));
        [rgb[0], rgb[1], rgb[2], 0]
    } else {
        look.edge()
    };
    let l = look.light;
    let columns: Vec<[Vertex; 4]> = (0..COLUMNS)
        .map(|j| {
            let (s, c) = core::fmath::sin_cos(j as f32 * STEP);
            let radial = Vec3::new(c, 0.0, s);
            let mut amplitude = p;
            if velocity.is_some() {
                let relative = horizontal_speed * (2.0 / 3.0);
                let facing = radial.dot(direction);
                let shape = if relative > 1.0 {
                    let delta = 1.0 - relative * facing;
                    1.0 - (1.5 / horizontal_speed) * delta * delta
                } else {
                    let delta = relative * (1.0 - facing);
                    (2.0 - relative) * (1.0 - delta * delta)
                };
                amplitude *= shape.max(0.0);
            }
            let amp = amplitude * (2.0 - amplitude) * 0.3;
            let alpha = (amplitude * 255.0).clamp(0.0, 255.0) as u8;
            let n = Vec3::new(amp * SLOPE * c, flat, amp * SLOPE * s).normalize();
            let inner = (n.x * l.x + n.z * l.z - n.y * l.y).max(0.0);
            let outer = (-n.dot(l)).max(0.0);
            let shade = |d| {
                if velocity.is_some() {
                    look.wake_shade(water, d, alpha)
                } else {
                    look.shade(d, alpha)
                }
            };
            let colour = [edge, shade(inner), shade(outer), edge];
            std::array::from_fn(|i| {
                let r = radius + BAND[i];
                (
                    centre + Vec3::new(r * c, LIFT, r * s),
                    [j as f32 * 0.5, V[i]],
                    colour[i],
                )
            })
        })
        .collect();
    let mut out = Vec::with_capacity((COLUMNS - 1) * 3 * 6);
    for pair in columns.windows(2) {
        if velocity.is_some() && pair[0][1].2[3] == 0 && pair[1][1].2[3] == 0 {
            continue;
        }
        for i in 0..3 {
            let (a, b, c, d) = (pair[0][i], pair[1][i], pair[0][i + 1], pair[1][i + 1]);
            out.extend([a, b, c, c, b, d]);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ripple {
    pub at: Vec3,
    pub life: f32,
    pub size: f32,
    pub age: f32,
    pub velocity: Option<Vec3>,
}

#[derive(Debug, Default)]
pub struct Water {
    bobs: bool,
    look: Option<Look>,
    pool: Vec<Ripple>,
}

impl Water {
    pub fn new(bobs: bool, look: Option<Look>) -> Water {
        Water {
            bobs,
            look,
            pool: Vec::new(),
        }
    }

    pub fn from_manifest(m: &manifest::Manifest) -> Water {
        let cfg = m.water.as_ref().and_then(|w| w.config.as_ref());
        Water::new(
            !cfg.is_some_and(|c| c.ocean) && crate::water_anim::oscillates(cfg),
            Look::from_manifest(m),
        )
    }

    pub fn active(&self) -> bool {
        self.look.is_some()
    }

    pub fn lift(&self, t: f32) -> f32 {
        crate::water_anim::flat_lift(self.bobs, t)
    }

    pub fn spawn(&mut self, (life, size): (f32, f32), at: Vec3) {
        if self.look.is_some() && self.pool.len() < CAP {
            self.pool.push(Ripple {
                at,
                life,
                size,
                age: 0.0,
                velocity: None,
            });
        }
    }

    pub fn spawn_wake(&mut self, at: Vec3, velocity: Vec3) {
        if velocity.length_squared() > 0.0 && self.look.is_some() && self.pool.len() < CAP {
            self.pool.push(Ripple {
                at,
                life: 2.0,
                size: 0.25,
                age: 0.0,
                velocity: Some(velocity),
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.pool.len() {
            let r = &mut self.pool[i];
            r.age += dt;
            if r.age > r.life {
                self.pool.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub fn draws(
        &mut self,
        eye: Vec3,
        lift: f32,
        texture: Option<TextureHandle>,
        out: &mut Vec<ParticleDraw>,
    ) {
        let Some(look) = &self.look else {
            return;
        };
        // Culling is a camera operation. A headless replay can update many
        // ticks between captures; absence of a draw does not mean out of view.
        self.pool
            .retain(|r| (r.at + Vec3::Y * lift).distance_squared(eye) < SEEN_RANGE * SEEN_RANGE);
        for r in &mut self.pool {
            let centre = r.at + Vec3::Y * lift;
            let radius = r.size + SPREAD * r.age;
            let tris = match r.velocity {
                Some(velocity) => wake(look, centre, radius, r.life, r.age, velocity),
                None => ring(look, centre, radius, r.life, r.age),
            };
            if !tris.is_empty() {
                out.push(ParticleDraw {
                    shape: ParticleShape::Shaded { tris },
                    key: centre,
                    color: [255; 4],
                    texture,
                    additive: false,
                });
            }
        }
    }

    pub fn ripples(&self) -> &[Ripple] {
        &self.pool
    }

    pub fn clear(&mut self) {
        self.pool.clear();
    }
}

impl Fx {
    pub fn set_water(&mut self, m: &manifest::Manifest) {
        self.water = Water::from_manifest(m);
    }

    pub(super) fn ride_the_water(&mut self, t: f32) -> f32 {
        let lift = self.water.lift(t);
        for e in &mut self.live {
            if let Some(level) = e.float {
                e.frame.pos.y = level + lift + super::SPLASH_LIFT;
            }
        }
        lift
    }

    pub fn ripples(&self) -> &[Ripple] {
        self.water.ripples()
    }
}

#[cfg(test)]
pub(super) fn kas1() -> Look {
    Look {
        water: [128.0 / 255.0, 130.0 / 255.0, 128.0 / 255.0],
        specular: [0.0; 3],
        ambient: [90.0 / 255.0, 90.0 / 255.0, 100.0 / 255.0],
        sun: [120.0 / 255.0, 120.0 / 255.0, 130.0 / 255.0],
        light: Vec3::new(8.742_278e-8, -1.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wading_ripples_keep_velocity_and_open_into_two_side_arcs() {
        let look = kas1();
        let centre = Vec3::ZERO;
        let static_ring = ring(&look, centre, 1.0, 2.0, 0.3);
        let moving = wake(&look, centre, 1.0, 2.0, 0.3, Vec3::Z * 7.0);
        assert!(moving.len() < static_ring.len());
        let visible: Vec<_> = moving.iter().filter(|v| v.2[3] > 0).collect();
        assert!(!visible.is_empty());
        assert!(visible.iter().all(|v| {
            let radial = Vec3::new(v.0.x, 0.0, v.0.z).normalize();
            radial.z > -0.26 && radial.z < 0.68
        }));
        assert!(visible.iter().any(|v| v.0.x > 0.5));
        assert!(visible.iter().any(|v| v.0.x < -0.5));
        assert!(wake(&look, centre, 1.0, 2.0, 0.3, Vec3::ZERO).is_empty());
        let mut water = Water::new(false, Some(look));
        water.spawn(BOLT, centre);
        water.spawn_wake(centre, Vec3::Z * 7.0);
        assert_eq!(water.ripples()[0].velocity, None);
        assert_eq!(water.ripples()[1].velocity, Some(Vec3::Z * 7.0));
    }

    #[test]
    fn the_manifest_light_comes_back_to_the_bytes_the_ps2_holds() {
        assert_eq!((srgb_byte(0.187_820_76) * 255.0).round(), 120.0);
        assert_eq!((srgb_byte(0.223_227_98) * 255.0).round(), 130.0);
        assert_eq!((srgb_byte(0.102_241_75) * 255.0).round(), 90.0);
        assert_eq!((srgb_byte(0.127_437_7) * 255.0).round(), 100.0);
    }

    #[test]
    fn the_ring_is_the_ps2_strip_with_its_measured_light() {
        let look = kas1();
        let centre = Vec3::new(210.79, -45.0, -38.07);
        let tris = ring(&look, centre, 0.4 + SPREAD * 0.5, 3.4, 0.5);
        assert_eq!(tris.len(), 20 * 3 * 6);
        let band = [104, 105, 114, 255];
        let edge = [128, 130, 128, 0];
        let (p, uv, c) = tris[0];
        assert!((p - (centre + Vec3::new(1.15 - 0.375, 0.01, 0.0))).length() < 1e-5);
        assert_eq!((uv, c), ([0.0, 0.0], edge));
        let (p, uv, c) = tris[2];
        assert!((p - (centre + Vec3::new(1.15 - 0.1275, 0.01, 0.0))).length() < 1e-5);
        assert_eq!((uv, c), ([0.0, 675.0 / 2048.0], band));
        let (_, uv, c) = tris[1];
        assert_eq!((uv, c), ([0.5, 0.0], edge));
        let last = tris[tris.len() - 1];
        assert_eq!((last.1, last.2), ([10.0, 1.0], edge));
        assert!((last.0 - (centre + Vec3::new(1.15 + 0.375, 0.01, 0.0))).length() < 1e-4);
        assert!(tris
            .iter()
            .all(|(p, ..)| (p.y - (centre.y + 0.01)).abs() < 1e-6));
        let early = ring(&look, centre, 0.01, 1.5, 0.15);
        let a = (((0.15f32 / 1.5) * 6.666_666_5) * (1.0 / 3.0) * 1.5 * 255.0) as u8;
        assert_eq!(early[2].2[3], a);
        assert!(ring(&look, centre, 1.0, 1.5, 1.6).is_empty());
    }

    #[test]
    fn the_pool_keeps_a_ripple_only_while_it_lives_and_a_camera_is_near() {
        let mut w = Water::new(true, Some(kas1()));
        let at = Vec3::new(0.0, -45.0, 0.0);
        w.spawn(BOLT, at);
        let mut out = Vec::new();
        for _ in 0..44 {
            w.update(1.0 / 30.0);
            out.clear();
            w.draws(at + Vec3::new(0.0, 2.0, 119.0), 0.0, None, &mut out);
            assert_eq!(out.len(), 1);
        }
        w.update(1.0 / 30.0);
        w.draws(at, 0.0, None, &mut out);
        w.update(1.0 / 30.0);
        assert!(w.ripples().is_empty(), "past 1.5 s");
        w.spawn(BOLT, at);
        w.update(1.0 / 30.0);
        assert_eq!(
            w.ripples().len(),
            1,
            "a new ripple outlives its first update"
        );
        w.draws(at + Vec3::new(0.0, 0.0, 121.0), 0.0, None, &mut out);
        w.update(1.0 / 30.0);
        assert!(w.ripples().is_empty(), "no camera within 120 m");
        for i in 0..CAP + 5 {
            w.spawn(BOLT, at + Vec3::X * i as f32);
        }
        assert_eq!(w.ripples().len(), CAP);
        let mut dry = Water::new(true, None);
        dry.spawn(BOLT, at);
        assert!(
            dry.ripples().is_empty(),
            "no ripples on the ocean or a map without water"
        );
    }

    #[test]
    fn updates_before_the_first_capture_preserve_a_young_nearby_ripple() {
        let mut w = Water::new(true, Some(kas1()));
        let at = Vec3::new(0.0, -45.0, 0.0);
        w.spawn(BOLT, at);
        for _ in 0..10 {
            w.update(1.0 / 30.0);
        }
        assert_eq!(w.ripples().len(), 1);
        let mut out = Vec::new();
        w.draws(at + Vec3::Y * 2.0, 0.0, None, &mut out);
        assert_eq!(out.len(), 1);
        w.update(2.0);
        assert!(w.ripples().is_empty());
    }

    #[test]
    fn the_lift_is_the_flat_bob() {
        let t = std::f32::consts::FRAC_PI_2 / 1.884_955_8;
        assert!((Water::new(true, None).lift(t) - 0.06).abs() < 1e-6);
        assert_eq!(Water::new(false, None).lift(t), 0.0);
    }
}
