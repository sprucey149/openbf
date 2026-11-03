//! How the PS2 draws an effect's particles (card FXP2, `analysis/re-fx.md`
//! §6, [O]; BF2 §8): the per-effect distance fade and density thinning, the
//! size multiplier, and each Type's shape as a [`render::ParticleDraw`].
//!
//! - **Fades** (§6.8, B1 `FUN_002b8750`): the far distance D by
//!   BoundingRadius — 50 m (≤ 5), 150 m (≤ 20), 500 m — and alpha ×
//!   `1 − (d − 0.8D)/(0.2D)` beyond 0.8D, d the depth of the emitter along
//!   the view; `lod = max(0, (d − r − 20)·4/(min(D, 90) − 20))` keeps
//!   particles by `idx & 3` ({1111, 1110, 1010, 1000}) and fades the next to
//!   go by `1 − frac(lod)`; every non-GEOMETRY particle is drawn at
//!   **size′ = size·(1 + 0.2·clamp(lod, 1, 3))** — never below 1.2×. A
//!   PARTICLE nearer than 1 m + size′ fades out toward the eye (B1
//!   246005–246009).
//! - **PARTICLE** (§6.1): a screen-aligned square √2·size′ wide (corners on
//!   a circle of radius size′), turned by π·frac(0.00314·rot). BLUR uses the
//!   same sprite as an alpha mask over a low-resolution scene copy (§16).
//! - **ANIMATED** (§6.3, §8): BF1 the PARTICLE square over a fixed 4×4 atlas
//!   (u = (f & 3)/4, v = ((f >> 2) & 3)/4); BF2 a CPU quad 2·size′ wide
//!   turned by rot·2π/512 over the grid FrameSize cuts (all 7 BF2 ANIMATED
//!   emitters make 4×4, which the port assumes).
//! - **BILLBOARD** (§6.4): the world-axis plane k = ⌊rot⌋ at angle
//!   φ = 2π·frac(rot) (BF1 in 1/512 steps), half-width size′.
//! - **SPARK** (§6.5): a ribbon tail `pos − vel·L` → mid → head `pos`, L =
//!   SparkLength·sizeMul, half-width size′, U 0 / 0.5 / 1.
//! - **STREAK** (§6.5): one ribbon through the emitter's particles in spawn
//!   order (a point within √0.1 m of the last is dropped; fewer than 3 draw
//!   nothing), half-width size′, U = the particle's rotation; no thinning,
//!   no distance alpha. Its colour is the newest point's — PROVISIONAL (the
//!   PS2 colours each vertex).
//! - **GEOMETRY** (§6.6): the cached model at T(pos)·basis·R_y(raw radians)·S(size),
//!   with the particle tint and fade, no sprite size multiplier. The caller
//!   resolves its model and blend; children share that resolver.
//!
//! Colour (§6.8): HSV → RGB, then bytes `(int)(c·255)` — clamped here where
//! the PS2 byte wraps past 1.0. The per-view fades are taken against one
//! camera (the first view) for every view.

use super::emitter::{hsv_to_rgb, Era, Particle};
use glam::{Mat4, Vec3};
use manifest::{FxBlend, FxEmitter, FxType};
use render::{ParticleDraw, ParticleShape, TextureHandle};

/// The far distance of a BoundingRadius class (B1 `FUN_001a90d8` / `9110`
/// / `9148`).
pub fn far_distance(bounding_radius: f32) -> f32 {
    if bounding_radius > 20.0 {
        500.0
    } else if bounding_radius > 5.0 {
        150.0
    } else {
        50.0
    }
}

/// The keep table (`DAT_00518408`) and the fade table (`DAT_00518448`), by
/// level then `idx & 3`.
const KEEP: [[bool; 4]; 4] = [
    [true, true, true, true],
    [true, true, true, false],
    [true, false, true, false],
    [true, false, false, false],
];
const FADING: [[bool; 4]; 4] = [
    [false, false, false, true],
    [false, true, false, false],
    [false, false, true, false],
    [false, false, false, false],
];

/// One emitter's view terms at depth `d` (m along the view).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTerms {
    /// The far fade (≤ 0: not drawn).
    pub fade: f32,
    pub level: usize,
    /// The next-to-go particles' alpha factor.
    pub frac: f32,
    pub size_mul: f32,
}

pub fn view_terms(d: f32, bounding_radius: f32, zoom: f32) -> ViewTerms {
    // Camera scale (B1 FUN_002b8750, B1:245811–245815).
    let far = far_distance(bounding_radius) * zoom;
    let fade = if d > 0.8 * far {
        1.0 - (d - 0.8 * far) / (0.2 * far)
    } else {
        1.0
    };
    let lod = ((d - bounding_radius - 20.0) * 4.0 / (far.min(90.0) - 20.0)).max(0.0);
    let level = (lod as usize).min(3);
    let frac = if lod < 3.0 { 1.0 - lod.fract() } else { 1.0 };
    ViewTerms {
        fade,
        level,
        frac,
        size_mul: 1.0 + 0.2 * lod.clamp(1.0, 3.0),
    }
}

impl ViewTerms {
    /// Is spawn index `idx & 3` drawn at this level?
    pub fn keep(&self, i: u32) -> bool {
        KEEP[self.level][(i & 3) as usize]
    }

    /// The alpha factor of a kept, non-STREAK particle.
    pub fn alpha(&self, idx: u32) -> f32 {
        if FADING[self.level][(idx & 3) as usize] {
            self.frac * self.fade
        } else {
            self.fade
        }
    }
}

/// The PS2's vertex bytes for a particle colour.
pub fn bytes(p: &Particle, alpha_mul: f32) -> [u8; 4] {
    let rgb = if p.hsv {
        hsv_to_rgb(p.rgba[0], p.rgba[1], p.rgba[2])
    } else {
        [p.rgba[0], p.rgba[1], p.rgba[2]]
    };
    let b = |c: f32| (c * 255.0).clamp(0.0, 255.0) as u8;
    [b(rgb[0]), b(rgb[1]), b(rgb[2]), b(p.rgba[3] * alpha_mul)]
}

/// A BILLBOARD's plane vectors (A, B) for rotation `rot` (B1
/// `FUN_0047e108` case 6, plane table 0x558900).
pub fn billboard_axes(era: Era, rot: f32) -> (Vec3, Vec3) {
    let k = rot as i32;
    let phi = match era {
        Era::Bf1 => std::f32::consts::TAU * (((rot * 512.0) as i32 - 512 * k) as f32 / 512.0),
        Era::Bf2 => std::f32::consts::TAU * (rot - k as f32),
    };
    let (s, c) = core::fmath::sin_cos(phi);
    match k {
        0 => (Vec3::X, Vec3::new(0.0, s, c)),
        1 => (Vec3::Y, Vec3::new(s, 0.0, c)),
        2 => (Vec3::Z, Vec3::new(c, s, 0.0)),
        3 => (Vec3::new(s, c, 0.0), Vec3::new(c, -s, 0.0)),
        4 => (Vec3::new(s, 0.0, c), Vec3::new(c, 0.0, -s)),
        _ => (Vec3::new(0.0, s, c), Vec3::new(0.0, c, -s)),
    }
}

/// The PARTICLE sprite's screen angle, π·frac(0.00314·rot) (FTOI0
/// truncates, so the fraction keeps the sign).
pub fn sprite_angle(rot: f32) -> f32 {
    let x = rot * 0.00314;
    std::f32::consts::PI * (x - x.trunc())
}

const FULL_UV: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

fn cell_4x4(f: u32) -> [f32; 4] {
    let (u, v) = ((f & 3) as f32 * 0.25, ((f >> 2) & 3) as f32 * 0.25);
    [u, v, u + 0.25, v + 0.25]
}

/// A GEOMETRY emitter's model: its triangles in model space, one list per
/// sheet (`<layer>/models/<name>.glb`).
#[derive(Debug, Clone, Default)]
pub struct EffectModel {
    pub parts: Vec<(Option<TextureHandle>, crate::soldier_vis::Tris, u32)>,
}

impl EffectModel {
    /// The model placed by `m`, one draw per sheet.
    pub fn draws(
        &self,
        m: Mat4,
        color: [u8; 4],
        key: Vec3,
        additive: bool,
        era: Era,
        out: &mut Vec<ParticleDraw>,
    ) {
        for (tex, tris, flags) in &self.parts {
            let tris = tris
                .iter()
                .map(|(p, uv)| (m.transform_point3(*p), *uv))
                .collect();
            let (shape, additive, color) = match geometry_look(era, additive, *flags, color) {
                Some((additive, color)) => (ParticleShape::Mesh { tris }, additive, color),
                None => (ParticleShape::SolidMesh { tris }, false, [255; 4]),
            };
            out.push(ParticleDraw {
                shape,
                key,
                color,
                texture: *tex,
                additive,
            });
        }
    }
}

pub fn geometry_look(
    era: Era,
    forced: bool,
    flags: u32,
    color: [u8; 4],
) -> Option<(bool, [u8; 4])> {
    let additive_bit = match era {
        Era::Bf1 => 0x80,
        Era::Bf2 => 0x40,
    };
    (forced || color[3] != 255 || flags & 0x06 != 0)
        .then_some((forced || flags & additive_bit != 0, color))
}

/// A GEOMETRY particle's model matrix (§6.6): `T(pos)·basis·R_y(rot)·S(size)`
/// — B1 245040–245110 builds the rows (cos·s, 0, −sin·s), (0, s, 0),
/// (sin·s, 0, cos·s), multiplies them by the node's rotation rows and puts
/// the particle's position in the last row.
pub fn geometry_matrix(p: &Particle) -> Mat4 {
    Mat4::from_translation(p.pos)
        * Mat4::from_mat3(p.basis)
        * Mat4::from_rotation_y(p.rot)
        * Mat4::from_scale(Vec3::splat(p.size))
}

/// One emitter instance's particles as draws, `origin` its frame's
/// position (the key of every unsorted type), `forward` / `eye` the view.
#[allow(clippy::too_many_arguments)]
pub fn emitter_draws(
    em: &FxEmitter,
    era: Era,
    particles: &[Particle],
    terms: ViewTerms,
    origin: Vec3,
    eye: Vec3,
    forward: Vec3,
    texture: Option<TextureHandle>,
    out: &mut Vec<ParticleDraw>,
) {
    let g = &em.geometry;
    let additive = g.blend == FxBlend::Additive;
    let m = terms.size_mul;
    let push = |out: &mut Vec<ParticleDraw>, shape, key, color| {
        out.push(ParticleDraw {
            shape,
            key,
            color,
            texture,
            additive,
        })
    };
    match g.kind {
        FxType::Particle | FxType::Animated => {
            for p in particles {
                let size = p.size * m;
                let mut a = terms.alpha(p.idx);
                let near = (p.pos - eye).dot(forward) - 1.0;
                if near <= 0.0 {
                    continue;
                }
                if near < size {
                    a *= near / size;
                }
                let (half, angle, uv) = match (g.kind, g.blend, era) {
                    (FxType::Particle, _, _) => (
                        size * std::f32::consts::FRAC_1_SQRT_2,
                        sprite_angle(p.rot),
                        FULL_UV,
                    ),
                    (_, _, Era::Bf1) => (
                        size * std::f32::consts::FRAC_1_SQRT_2,
                        sprite_angle(p.rot),
                        cell_4x4(p.frame.unwrap_or(0)),
                    ),
                    (_, _, Era::Bf2) => (
                        size,
                        p.rot * std::f32::consts::TAU / 512.0,
                        cell_4x4(p.frame.unwrap_or(0)),
                    ),
                };
                let key = if g.kind == FxType::Particle {
                    p.pos
                } else {
                    origin
                };
                let color = bytes(p, a);
                let shape = if g.blend == FxBlend::Blur {
                    if color[3] == 0 {
                        continue;
                    }
                    ParticleShape::Blur {
                        pos: p.pos,
                        half,
                        angle,
                    }
                } else {
                    ParticleShape::Sprite {
                        pos: p.pos,
                        half,
                        angle,
                        uv,
                    }
                };
                push(out, shape, key, color);
            }
        }
        FxType::Billboard => {
            for p in particles {
                let (a, b) = billboard_axes(era, p.rot);
                let size = p.size * m;
                push(
                    out,
                    ParticleShape::Quad {
                        pos: p.pos,
                        a: a * size,
                        b: b * size,
                    },
                    origin,
                    bytes(p, terms.alpha(p.idx)),
                );
            }
        }
        FxType::Spark => {
            let l = g.spark_length * m;
            for p in particles {
                let w = p.size * m;
                let points = vec![
                    (p.pos - p.vel * l, w, 0.0),
                    (p.pos - p.vel * (0.5 * l), w, 0.5),
                    (p.pos, w, 1.0),
                ];
                push(
                    out,
                    ParticleShape::Ribbon { points },
                    origin,
                    bytes(p, terms.alpha(p.idx)),
                );
            }
        }
        FxType::Streak => {
            let mut sorted: Vec<&Particle> = particles.iter().collect();
            sorted.sort_by_key(|p| p.idx);
            let mut points: Vec<(Vec3, f32, f32)> = Vec::new();
            for p in &sorted {
                if let Some(last) = points.last() {
                    if last.0.distance_squared(p.pos) < 0.1 {
                        continue;
                    }
                }
                points.push((p.pos, p.size * m, p.rot));
            }
            if points.len() >= 3 {
                let newest = sorted[sorted.len() - 1];
                push(
                    out,
                    ParticleShape::Ribbon { points },
                    origin,
                    bytes(newest, 1.0),
                );
            }
        }
        FxType::Geometry | FxType::Emitter => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §6.8's numbers: D by BoundingRadius, the far fade over the last 20 %,
    /// the lod and sizeMul (never below 1.2, 1.6 at lod 3).
    #[test]
    fn fades_lod_and_the_size_multiplier() {
        assert_eq!(
            (
                far_distance(5.0),
                far_distance(5.1),
                far_distance(20.0),
                far_distance(21.0)
            ),
            (50.0, 150.0, 150.0, 500.0)
        );
        let t = view_terms(10.0, 5.0, 1.0);
        assert_eq!((t.fade, t.level, t.size_mul), (1.0, 0, 1.2));
        // d = 45 of D = 50: fade = 1 − 5/10 = 0.5; lod = (45 − 25)·4/30 = 2.667.
        let t = view_terms(45.0, 5.0, 1.0);
        assert!((t.fade - 0.5).abs() < 1e-6);
        assert_eq!(t.level, 2);
        assert!((t.size_mul - (1.0 + 0.2 * 8.0 / 3.0)).abs() < 1e-5);
        assert!((t.frac - (1.0 - 2.0 / 3.0)).abs() < 1e-5);
        assert!(!t.keep(1) && !t.keep(3) && t.keep(0) && t.keep(2));
        assert!(
            (t.alpha(2) - t.frac * t.fade).abs() < 1e-6,
            "idx 2 goes next"
        );
        assert_eq!(t.alpha(0), t.fade);
        let scoped_near = view_terms(45.0, 5.0, 2.0);
        assert_eq!(scoped_near.fade, 1.0);
        assert_eq!(scoped_near.level, 1);
        assert!((scoped_near.size_mul - (1.0 + 0.2 * 80.0 / 70.0)).abs() < 1e-6);
        let scoped = view_terms(90.0, 5.0, 2.0);
        assert_eq!(scoped.fade, 0.5);
        // Thinning uses min(D*S, 90), not min(D,90)*S.
        let lod = (90.0_f32 - 5.0 - 20.0) * 4.0 / 70.0;
        assert_eq!(scoped.level, 3);
        assert_eq!(scoped.size_mul, 1.0 + 0.2 * lod.clamp(1.0, 3.0));
        let t = view_terms(1000.0, 30.0, 1.0);
        assert!(t.fade <= 0.0);
        assert_eq!(t.level, 3);
        assert!((t.size_mul - 1.6).abs() < 1e-6);
    }

    /// The rotation mapping (§6.1): 0.5652° per authored degree, snapping
    /// back every 318.47 units; the BILLBOARD plane k = 4 is horizontal.
    #[test]
    fn rotations_and_planes() {
        assert!((sprite_angle(100.0).to_degrees() - 56.52).abs() < 1e-3);
        assert!(sprite_angle(320.0) < 0.02, "wrapped");
        let (a, b) = billboard_axes(Era::Bf1, 4.25);
        assert!(a.y == 0.0 && b.y == 0.0);
        assert!(a.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-6), "{a}");
        let (a, _) = billboard_axes(Era::Bf1, 0.5);
        assert_eq!(a, Vec3::X);
    }

    /// The 4×4 atlas cells, row-major, and the colour bytes (HSV converted,
    /// clamped, alpha × the fade).
    #[test]
    fn atlas_cells_and_bytes() {
        assert_eq!(cell_4x4(0), [0.0, 0.0, 0.25, 0.25]);
        assert_eq!(cell_4x4(6), [0.5, 0.25, 0.75, 0.5]);
        assert_eq!(cell_4x4(16), cell_4x4(0), "f & 15");
        let p = Particle {
            basis: glam::Mat3::IDENTITY,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            size: 1.0,
            rot: 0.0,
            rgba: [0.0, 1.0, 1.0, 1.0],
            hsv: true,
            age: 0.0,
            idx: 0,
            frame: None,
        };
        assert_eq!(bytes(&p, 0.5), [255, 0, 0, 127]);
        let p = Particle {
            basis: glam::Mat3::IDENTITY,
            rgba: [1.5, 0.5, -1.0, 1.0],
            hsv: false,
            ..p
        };
        assert_eq!(bytes(&p, 1.0), [255, 127, 0, 255]);
    }
}
