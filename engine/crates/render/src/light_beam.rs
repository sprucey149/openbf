//! The PS2's "lightbeam" renderer (card CPHOL, `analysis/re-point-lights.md`
//! §10 and §15 [O]; BF2 `FUN_00469628`, its tables `FUN_004691f0`; BF1's twin
//! `FUN_00386170`): the column of a command post's hologram.
//!
//! A beam with a length is NOT a cone in space: it is one flat fan of 22
//! vertices in the plane through its axis that faces the eye — x along
//! `side = normalize((base − eye) × axis)`, y along the axis — so it turns
//! about its axis with the viewer:
//! - 3 vertices at y 0, x −0.6 / 0 / +0.6 of the base radius, alpha 0;
//! - 6 at y 0.2·r0, x from −r0 to +r0, alpha A·f·√(1 − (x/r0)²);
//! - 7 at y L, x from −r1 to +r1, alpha A·f·0.25·√(1 − (x/r1)²);
//! - 6 on a round cap over the top, `cap` high, alpha 0.
//!
//! `f` is the view's own factor: 1 − g² (g = the axial fraction of the
//! eye's offset: looked at from the side the beam is full, along its axis it
//! is gone), times a fade as the eye comes inside the cone's radius, times
//! one far fade band: the earlier of the scene's fade start and 0.81 times
//! the effect's visible distance squared, to the shorter far distance squared.
//!
//! **Near the eye** ([`crosses_near`], PROVISIONAL [D]): the PS2 hands the fan
//! to its packed-strip draw with a box — the fan's own extent and ±1 m along
//! its normal (`FUN_00469628` 529237–529238) — and its frames show NO beam
//! where that box reaches the near plane: at the 12 stored poses with the
//! eye within 1.11 m of a post's axis the PS2 draws none (a beam there would
//! wash the whole frame, 5 to 21 levels), at 1.79 m and beyond it does.
//!
//! GS state: ALPHA 0x48 (Cs·As + Cd), TEST 0x51001 (depth tested, never
//! written), Gouraud, no fog — the particles' additive pipeline. The vertex
//! colour is the beam's bytes halved (`(b + 1) >> 1`) and [`Beam::gs_rgb`]
//! is what a halved byte of 128 shows as: 1.0 in BF2, which binds
//! `white.tga` and modulates (128 = ×1.0); 0.5 in BF1, which binds no sheet
//! (`FUN_00386170`: `FUN_0049e300(0, 0, 0)`), so the colour goes out as it
//! is. Seen in the PS2's frames: BF2 Death Star cp3's column adds 86 / 42 /
//! 40 levels of (200, 100, 100); BF1 Kashyyyk Docks cp3's adds 42 / 82 / 35
//! of (100, 200, 100).

use crate::grass::GrassVertex;
use crate::{Camera, FarFade};
use glam::Vec3;

/// The top row's alpha against the bottom row's (beam +0x18c).
pub const TOP_ALPHA: f32 = 0.25;

/// One beam in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub base: Vec3,
    /// Unit, the beam's own +Y.
    pub axis: Vec3,
    pub length: f32,
    /// The radius at the base and at `length`.
    pub r0: f32,
    pub r1: f32,
    /// The soft end's height over `length` seen from the side.
    pub soft: f32,
    /// The visible distance, metres (0 = no limit of its own).
    pub visible: f32,
    /// The scene's distance band, or None for the extended world reach.
    /// The camera's finite far distance always caps it.
    pub view: Option<FarFade>,
    /// The displayed value of a colour byte of 128 (module doc).
    pub gs_rgb: f32,
    /// PC `BeamIntensity` scalar on view alpha (default 1.0 for PS2 / holograms).
    pub intensity: f32,
    pub top: f32,
    pub cross: Option<Cross>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cross {
    pub x: Vec3,
    pub z: Vec3,
    pub r0z: f32,
    pub r1z: f32,
}

impl Beam {
    fn seen_from(&self, side: Vec3) -> Beam {
        let Some(c) = self.cross else {
            return *self;
        };
        let mut out = Beam {
            cross: None,
            ..*self
        };
        if self.r0 == c.r0z && self.r1 == c.r1z {
            return out;
        }
        let (a, z) = (side.dot(c.x).abs(), side.dot(c.z).abs());
        let fit = |rx: f32, rz: f32| {
            if rx == rz {
                rx
            } else {
                (rx * rx * a * a + rz * rz * z * z).sqrt()
            }
        };
        out.r0 = fit(self.r0, c.r0z);
        out.r1 = fit(self.r1, c.r1z);
        out
    }
}

/// The fan's strip as the PS2 indexes it (`FUN_004691f0`): `true` = this
/// vertex ends no triangle.
const STRIP: [(u8, bool); 34] = [
    (8, true),
    (7, true),
    (2, false),
    (6, false),
    (1, false),
    (5, false),
    (0, false),
    (4, false),
    (3, false),
    (9, true),
    (3, true),
    (10, false),
    (4, false),
    (11, false),
    (5, false),
    (12, false),
    (6, false),
    (13, false),
    (7, false),
    (14, false),
    (8, false),
    (15, false),
    (21, true),
    (14, false),
    (20, false),
    (13, false),
    (19, false),
    (12, false),
    (18, false),
    (11, false),
    (17, false),
    (10, false),
    (16, false),
    (9, false),
];

// BF2 FUN_00469628: the earlier fade start and the shorter far limit,
// in squared metres, form ONE band (528909-528916, 529211-529218 [V]).
fn distance_factor(b: &Beam, camera: &Camera, d2: f32) -> Option<f32> {
    if !d2.is_finite() || !camera.far.is_finite() || camera.far <= 0.0 {
        return None;
    }
    // [ours] With world culling relaxed, keep the same extended reach,
    // bounded radially by the projection's far distance; fade its last 10%.
    let mut end = camera.far * camera.far;
    if !end.is_finite() {
        return None;
    }
    let mut begin = 0.81 * end;
    if let Some(view) = b.view {
        if !view.begin.is_finite()
            || !view.end.is_finite()
            || view.begin < 0.0
            || view.end <= 0.0
            || view.begin > view.end
        {
            return None;
        }
        begin = begin.min(view.begin * view.begin);
        end = end.min(view.end * view.end);
    }
    if b.visible > 0.0 {
        let own = b.visible * b.visible;
        begin = begin.min(0.81 * own);
        end = end.min(own);
    }
    if d2 >= end {
        return None;
    }
    Some(if d2 > begin {
        (end - d2) / (end - begin)
    } else {
        1.0
    })
}

/// The view's factor `f` and axial fraction `g`, or `None` when culled.
pub fn view_factor(b: &Beam, camera: &Camera) -> Option<(f32, f32)> {
    if !b.intensity.is_finite() || b.intensity <= 0.0 {
        return None;
    }
    let v = b.base - camera.position;
    let d2 = v.length_squared();
    let distance = distance_factor(b, camera, d2)?;
    let ax = v.dot(b.axis);
    // The cone's radius at the eye's height (an eye over the base).
    let r_eye = if ax < 0.0 {
        b.r0 - (b.r1 - b.r0) * ax / b.length
    } else {
        b.r0
    };
    let off_axis = (v - b.axis * ax).length() - camera.near;
    let g = ax / if b.r0 * b.r0 < d2 { d2.sqrt() } else { b.r0 };
    let mut f = (1.0 - g * g) * distance * b.intensity;
    if off_axis < 0.0 {
        f = 0.0;
    } else if off_axis < r_eye {
        f *= off_axis / r_eye;
    }
    Some((f.max(0.0), g))
}

/// The fan's 22 points as (x, y, alpha factor) — alpha = A·f·factor.
pub fn fan(b: &Beam, g: f32) -> [(f32, f32, f32); 22] {
    let mut out = [(0.0, 0.0, 0.0); 22];
    for (i, p) in out.iter_mut().take(3).enumerate() {
        *p = ((i as f32 * 0.6 - 0.6) * b.r0, 0.0, 0.0);
    }
    for i in 0..6 {
        let x = i as f32 * 0.4 - 1.0;
        out[3 + i] = (x * b.r0, 0.2 * b.r0, (1.0 - x * x).max(0.0).sqrt());
    }
    for i in 0..7 {
        let x = i as f32 / 3.0 - 1.0;
        out[9 + i] = (x * b.r1, b.length, b.top * (1.0 - x * x).max(0.0).sqrt());
    }
    // The cap: `soft` high from the side, the top's own radius along the axis.
    let cap = b.soft + g.abs() * (b.r1 - b.soft);
    for i in 0..6 {
        let c = (i as f32 + 0.5) / 3.0 - 1.0;
        let c = c + (1.0 / 18.0_f32).copysign(c);
        out[16 + i] = (c * b.r1, b.length + cap * (1.0 - c * c).sqrt(), 0.0);
    }
    out
}

/// Does the fan's box — its extent in its plane (×1.01) and ±1 m along its
/// normal — reach the view's near plane (module doc)?
pub fn crosses_near(b: &Beam, side: Vec3, g: f32, camera: &Camera) -> bool {
    let (fwd, normal) = (camera.forward(), side.cross(b.axis));
    let x = 1.01 * b.r0.max(b.r1);
    let cap = b.soft + g.abs() * (b.r1 - b.soft);
    let depth = |x: f32, y: f32, z: f32| {
        (b.base + side * x + b.axis * y + normal * z - camera.position).dot(fwd)
    };
    [-x, x].iter().any(|&x| {
        [-0.01, 1.01 * (b.length + cap)]
            .iter()
            .any(|&y| [-1.0, 1.0].iter().any(|&z| depth(x, y, z) < camera.near))
    })
}

/// The beam's triangles for this view, coloured `rgba` (its bytes before the
/// GS halving; alpha 255 = ×1.0).
pub fn vertices(b: &Beam, rgba: [u8; 4], camera: &Camera) -> Vec<GrassVertex> {
    if b.length <= 0.0 {
        return glow_vertices(b, rgba, camera);
    }
    let eye = camera.position;
    let side = (b.base - eye).cross(b.axis);
    if side.length_squared() < 1e-12 {
        return Vec::new();
    }
    let side = side.normalize();
    let b = &b.seen_from(side);
    let Some((f, g)) = view_factor(b, camera) else {
        return Vec::new();
    };
    if f <= 0.0 {
        return Vec::new();
    }
    if crosses_near(b, side, g, camera) {
        return Vec::new();
    }
    let half = |v: u8| f32::from((u16::from(v) + 1) >> 1);
    let rgb = [rgba[0], rgba[1], rgba[2]].map(|c| half(c) / 128.0 * b.gs_rgb);
    let a = f32::from(rgba[3]);
    let points = fan(b, g).map(|(x, y, k)| {
        // The alpha byte is truncated before the GS halves it.
        let alpha = half((a * f * k) as u8) / 128.0;
        GrassVertex {
            position: (b.base + side * x + b.axis * y).to_array(),
            uv: [0.0, 0.0],
            color: [rgb[0], rgb[1], rgb[2], alpha],
            up: 0.0,
        }
    });
    let mut out = Vec::with_capacity(29 * 3);
    for w in STRIP.windows(3) {
        if !w[2].1 {
            out.extend(w.iter().map(|k| points[usize::from(k.0)]));
        }
    }
    out
}

pub fn glow_vertices(b: &Beam, rgba: [u8; 4], camera: &Camera) -> Vec<GrassVertex> {
    if !b.intensity.is_finite() || b.intensity <= 0.0 {
        return Vec::new();
    }
    let eye = camera.position;
    let fwd = camera.forward();
    let inner = b.r1;
    let outer = inner + b.soft;
    let v = b.base - eye;
    let d2 = v.length_squared();
    let Some(mut f) = distance_factor(b, camera, d2) else {
        return Vec::new();
    };
    let depth = v.dot(fwd);
    if depth + outer <= camera.near {
        return Vec::new();
    }
    let gap = depth - camera.near;
    if gap < outer {
        if gap < inner {
            return Vec::new();
        }
        f *= (gap - inner) / (outer - inner);
    }
    f *= b.intensity;
    let half = |v: u8| f32::from((u16::from(v) + 1) >> 1);
    let rgb = [rgba[0], rgba[1], rgba[2]].map(|c| half(c) / 128.0 * b.gs_rgb);
    let centre_alpha = (f32::from(rgba[3]) * f + 0.5).clamp(0.0, 255.0) as u8;
    if centre_alpha == 0 {
        return Vec::new();
    }
    let ring_alpha = (f32::from(centre_alpha) * b.top) as u8;
    let right = camera.right();
    let up = right.cross(fwd).normalize_or_zero();
    let lift = -fwd * inner;
    let at = |angle: f32, r: f32, z: Vec3, alpha: u8| {
        let (s, c) = angle.sin_cos();
        GrassVertex {
            position: (b.base + z + (right * -s + up * c) * r).to_array(),
            uv: [0.0, 0.0],
            color: [rgb[0], rgb[1], rgb[2], half(alpha) / 128.0],
            up: 0.0,
        }
    };
    let step = std::f32::consts::PI / 8.0;
    let centre = at(0.0, 0.0, lift, centre_alpha);
    let ring: Vec<_> = (0..16)
        .map(|k| at(k as f32 * step, inner, lift, ring_alpha))
        .collect();
    let rim: Vec<_> = (0..16)
        .map(|k| at((k as f32 + 0.5) * step, outer, Vec3::ZERO, 0))
        .collect();
    let mut out = Vec::with_capacity(48 * 3);
    for k in 0..16 {
        let n = (k + 1) % 16;
        out.extend([ring[k], ring[n], centre]);
        out.extend([ring[k], rim[k], ring[n]]);
        out.extend([rim[k], ring[n], rim[n]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post() -> Beam {
        Beam {
            base: Vec3::new(0.0, 0.0, -10.0),
            axis: Vec3::Y,
            length: 3.5,
            r0: 0.5,
            r1: 1.0,
            soft: 0.7,
            visible: 200.0,
            view: None,
            gs_rgb: 1.0,
            intensity: 1.0,
            top: TOP_ALPHA,
            cross: None,
        }
    }

    /// The fan of a post's beam: base 0.5, top 1.0 at 3.5 m, brightest on the
    /// axis, a quarter of it at the top, nothing at the rim or the cap.
    #[test]
    fn the_fan_is_the_ps2s_22_points() {
        let p = fan(&post(), 0.0);
        assert_eq!((p[0].0, p[2].0, p[1]), (-0.3, 0.3, (0.0, 0.0, 0.0)));
        assert_eq!((p[3].0, p[3].1, p[3].2), (-0.5, 0.1, 0.0));
        assert!((p[5].2 - 0.979_796).abs() < 1e-5, "{}", p[5].2);
        assert_eq!((p[9].0, p[12], p[15].0), (-1.0, (0.0, 3.5, 0.25), 1.0));
        assert!(p[9].2 == 0.0 && p[15].2.abs() < 1e-3);
        assert!((p[16].0 + 0.888_889).abs() < 1e-5 && p[16].2 == 0.0);
        assert!((p[18].1 - (3.5 + 0.7 * (1.0f32 - 0.222_222 * 0.222_222).sqrt())).abs() < 1e-4);
        // Along the axis the cap is the top's radius high.
        assert!(
            (fan(&post(), 1.0)[18].1 - (3.5 + (1.0f32 - 0.222_222 * 0.222_222).sqrt())).abs()
                < 1e-4
        );
    }

    /// 29 triangles in the plane that faces the eye, each vertex's colour
    /// the bytes halved; the same beam from another side is the same fan
    /// turned about its axis.
    #[test]
    fn the_fan_faces_the_eye() {
        let cam = Camera::default();
        let v = vertices(&post(), [200, 100, 100, 255], &cam);
        assert_eq!(v.len(), 29 * 3);
        assert!(v.iter().all(|p| (p.position[2] + 10.0).abs() < 1e-5));
        assert!(v
            .iter()
            .all(|p| p.color[..3] == [100.0 / 128.0, 50.0 / 128.0, 50.0 / 128.0]));
        let top = v.iter().map(|p| p.color[3]).fold(0.0, f32::max);
        // f = 1 − g² with g = 0 (the eye level with the base): 250 → 125.
        assert!((top - 125.0 / 128.0).abs() < 1e-6, "{top}");
        let cam2 = Camera {
            position: Vec3::new(10.0, 0.0, -10.0),
            yaw: std::f32::consts::FRAC_PI_2,
            ..Camera::default()
        };
        let w = vertices(&post(), [200, 100, 100, 255], &cam2);
        assert!(w.iter().all(|p| p.position[0].abs() < 1e-5));
        assert_eq!(w.len(), v.len());
    }

    /// The view's factor: full from the side, gone along the axis, fading as
    /// the eye enters the cone and over the last 19 % of the distance squared.
    #[test]
    fn the_beam_fades_with_the_view() {
        let b = post();
        let factor = |eye| {
            view_factor(
                &b,
                &Camera {
                    position: eye,
                    ..Camera::default()
                },
            )
        };
        let side = factor(Vec3::ZERO).unwrap();
        assert!((side.0 - 1.0).abs() < 1e-6 && side.1 == 0.0);
        // 45° over the base: g² = 0.5.
        let over = factor(Vec3::new(0.0, 10.0, 0.0)).unwrap();
        assert!((over.0 - 0.5).abs() < 1e-5 && over.1 < 0.0, "{over:?}");
        // Inside the cone at its base height: 0.35 m off the axis, near 0.1.
        let inside = factor(Vec3::new(0.35, 0.0, -10.0)).unwrap();
        assert!((inside.0 - 0.25 / 0.5).abs() < 1e-5, "{inside:?}");
        assert_eq!(factor(Vec3::new(0.05, 0.0, -10.0)).unwrap().0, 0.0);
        // 190 m off: (200² − 190²) / (0.19 · 200²).
        let far = factor(Vec3::new(0.0, 0.0, 180.0)).unwrap();
        assert!((far.0 - 3900.0 / 7600.0).abs() < 1e-4, "{far:?}");
        assert!(factor(Vec3::new(0.0, 0.0, 190.0)).is_none());
    }

    /// The PS2 draws no beam where the fan's box reaches the near plane:
    /// none at the stored pose of BF2 Tantive cp3 (the eye 1.11 m from the
    /// axis, looking at it), one at BF2 Mos Eisley cp2's (1.79 m).
    #[test]
    fn no_beam_where_its_box_reaches_the_near_plane() {
        let pose = |eye: [f32; 3], yaw: f32, base: [f32; 3]| {
            let cam = Camera {
                position: Vec3::from(eye),
                yaw: yaw.to_radians(),
                pitch: (-0.64f32).to_radians(),
                ..Camera::default()
            };
            let b = Beam {
                base: Vec3::from(base),
                ..post()
            };
            vertices(&b, [200, 100, 100, 255], &cam).len()
        };
        let tan1 = pose([-277.024, 2.887, 75.037], 179.46, [-277.0, 1.32, 76.15]);
        assert_eq!(tan1, 0);
        let level = |d: f32| pose([0.0, 1.5, d], 0.0, [0.0, 0.0, 0.0]);
        assert_eq!((level(1.0), level(1.79)), (0, 29 * 3));
        // Looking away from it the box is behind the eye: none either.
        assert_eq!(pose([0.0, 1.5, 3.0], 180.0, [0.0, 0.0, 0.0]), 0);
    }

    fn halo() -> Beam {
        Beam {
            base: Vec3::new(0.0, 0.0, -10.0),
            axis: Vec3::Y,
            length: 1.0,
            r0: 0.4,
            r1: 0.65,
            soft: 0.8,
            visible: 300.0,
            view: None,
            gs_rgb: 1.0,
            intensity: 1.0,
            top: 0.3,
            cross: Some(Cross {
                x: Vec3::X,
                z: Vec3::Z,
                r0z: 0.1,
                r1z: 0.4,
            }),
        }
    }

    #[test]
    fn an_elliptic_beam_shows_the_width_the_view_crosses() {
        let width = |eye: Vec3| {
            let cam = Camera {
                position: eye,
                ..Camera::default()
            };
            let side = (halo().base - eye).cross(Vec3::Y).normalize();
            let b = halo().seen_from(side);
            assert_eq!(b.cross, None);
            (
                b.r0,
                b.r1,
                vertices(&halo(), [50, 177, 219, 255], &cam).len(),
            )
        };
        let (r0, r1, n) = width(Vec3::new(0.0, 0.5, 0.0));
        assert!((r0 - 0.4).abs() < 1e-6 && (r1 - 0.65).abs() < 1e-6 && n == 29 * 3);
        let (r0, r1, _) = width(Vec3::new(10.0, 0.5, -10.0));
        assert!(
            (r0 - 0.1).abs() < 1e-6 && (r1 - 0.4).abs() < 1e-6,
            "{r0} {r1}"
        );
        let round = Beam {
            cross: Some(Cross {
                x: Vec3::X,
                z: Vec3::Z,
                r0z: 0.4,
                r1z: 0.65,
            }),
            ..halo()
        };
        assert_eq!(round.seen_from(Vec3::Z).r0, 0.4);
    }

    #[test]
    fn the_top_row_takes_the_beams_own_fade_factor() {
        let p = fan(&halo().seen_from(Vec3::X), 0.0);
        assert_eq!(p[12].2, 0.3);
        assert_eq!(fan(&post(), 0.0)[12].2, TOP_ALPHA);
    }

    fn glow() -> Beam {
        Beam {
            length: 0.0,
            r0: 0.5,
            r1: 0.1,
            soft: 1.0,
            visible: 0.0,
            top: 0.2,
            cross: None,
            ..halo()
        }
    }

    #[test]
    fn a_zero_length_beam_is_a_glow_disc_facing_the_eye() {
        let cam = Camera::default();
        let v = vertices(&glow(), [255, 90, 90, 255], &cam);
        assert_eq!(v.len(), 48 * 3);
        assert!(v
            .iter()
            .all(|p| p.color[..3] == [1.0, 45.0 / 128.0, 45.0 / 128.0]));
        let centre = v[2];
        assert_eq!(centre.position, [0.0, 0.0, -9.9]);
        assert_eq!(centre.color[3], 128.0 / 128.0);
        assert_eq!(v[0].color[3], 26.0 / 128.0);
        let rim = v[4];
        assert_eq!(rim.color[3], 0.0);
        assert!((rim.position[2] + 10.0).abs() < 1e-6);
        assert!(((rim.position[0].powi(2) + rim.position[1].powi(2)).sqrt() - 1.1).abs() < 1e-5);
    }

    #[test]
    fn a_glow_fades_in_past_its_radius_and_vanishes_inside_it() {
        let at = |z: f32| {
            let cam = Camera {
                position: Vec3::new(0.0, 0.0, z),
                ..Camera::default()
            };
            vertices(&glow(), [255, 90, 90, 255], &cam)
        };
        let near = Camera::default().near;
        let d = |gap: f32| -10.0 + gap + near;
        assert!(at(d(0.05)).is_empty());
        let half = at(d(0.6));
        assert_eq!(half[2].color[3], 64.0 / 128.0);
        assert_eq!(at(d(2.0))[2].color[3], 1.0);
        let far = Beam {
            visible: 100.0,
            ..glow()
        };
        let from = |z: f32| {
            let cam = Camera {
                position: Vec3::new(0.0, 0.0, z),
                ..Camera::default()
            };
            vertices(&far, [255, 90, 90, 255], &cam)
        };
        assert_eq!(from(85.0)[2].color[3], 66.0 / 128.0);
        assert!(from(95.0).is_empty());
    }

    #[test]
    fn halo_distance_zero_limit_still_obeys_the_view() {
        let camera = Camera {
            far: 100.0,
            ..Camera::default()
        };
        for visible in [0.0, 300.0] {
            for length in [0.0, 1.0] {
                let b = Beam {
                    base: Vec3::new(75.0, 0.0, -75.0),
                    visible,
                    length,
                    ..glow()
                };
                assert!(vertices(&b, [255; 4], &camera).is_empty(),
                    "halo beyond radial view distance emitted triangles: limit={visible}, length={length}");
            }
        }
    }

    #[test]
    fn halo_distance_zero_and_larger_limits_fade_with_the_view() {
        let camera = Camera {
            far: 100.0,
            ..Camera::default()
        };
        for visible in [0.0, 300.0] {
            let b = Beam {
                base: Vec3::new(0.0, 0.0, -95.0),
                visible,
                ..glow()
            };
            let v = vertices(&b, [255; 4], &camera);
            assert_eq!(
                v[2].color[3],
                66.0 / 128.0,
                "expected the same fade as an explicit 100 m distance: limit={visible}"
            );
        }
    }

    #[test]
    fn halo_distance_larger_effect_limit_still_obeys_the_view() {
        let camera = Camera {
            far: 100.0,
            ..Camera::default()
        };
        let b = Beam {
            base: Vec3::new(75.0, 0.0, -75.0),
            visible: 300.0,
            ..glow()
        };
        assert!(vertices(&b, [255; 4], &camera).is_empty());
    }

    #[test]
    fn halo_distance_larger_effect_limit_still_fades_with_the_view() {
        let camera = Camera {
            far: 100.0,
            ..Camera::default()
        };
        let b = Beam {
            base: Vec3::new(0.0, 0.0, -95.0),
            visible: 300.0,
            ..glow()
        };
        assert_eq!(vertices(&b, [255; 4], &camera)[2].color[3], 66.0 / 128.0);
    }

    #[test]
    fn halo_distance_intersects_both_ends_of_the_authored_view_band() {
        // The scene fades at 80..100 m, independently of the 1000 m projection.
        let camera = Camera {
            far: 1000.0,
            ..Camera::default()
        };
        for visible in [0.0, 300.0] {
            for length in [0.0, 1.0] {
                let b = Beam {
                    base: Vec3::new(0.0, 0.0, -95.0),
                    visible,
                    length,
                    view: Some(FarFade {
                        begin: 80.0,
                        end: 100.0,
                    }),
                    ..glow()
                };
                let v = vertices(&b, [255; 4], &camera);
                // ONE band: (100^2 - 95^2) / (100^2 - 80^2) = 13/48.
                let peak = v.iter().map(|v| v.color[3]).fold(0.0, f32::max);
                let expected = if length == 0.0 { 35.0 } else { 34.0 } / 128.0;
                assert_eq!(peak, expected, "limit={visible}, length={length}");
                let outside = Beam {
                    base: Vec3::new(75.0, 0.0, -75.0),
                    ..b
                };
                assert!(vertices(&outside, [255; 4], &camera).is_empty());
            }
        }
    }

    #[test]
    fn halo_distance_uses_the_earlier_start_even_when_the_effect_limit_is_larger() {
        let camera = Camera::default();
        let b = Beam {
            base: Vec3::new(0.0, 0.0, -95.0),
            visible: 105.0,
            view: Some(FarFade {
                begin: 99.0,
                end: 100.0,
            }),
            ..glow()
        };
        // Effect start 94.5 m beats the view's 99 m; view end 100 beats 105.
        // round(255 * (10000 - 9025) / (10000 - 8930.25)) = 232 -> 116.
        assert_eq!(vertices(&b, [255; 4], &camera)[2].color[3], 116.0 / 128.0);
    }

    #[test]
    fn halo_distance_a_shorter_effect_caps_the_view_without_multiplying_fades() {
        let camera = Camera::default();
        let b = Beam {
            base: Vec3::new(0.0, 0.0, -95.0),
            visible: 100.0,
            view: Some(FarFade {
                begin: 80.0,
                end: 200.0,
            }),
            ..glow()
        };
        assert_eq!(vertices(&b, [255; 4], &camera)[2].color[3], 35.0 / 128.0);
        let outside = Beam {
            base: Vec3::new(0.0, 0.0, -101.0),
            ..b
        };
        assert!(vertices(&outside, [255; 4], &camera).is_empty());
        let near = Beam {
            base: Vec3::new(0.0, 0.0, -10.0),
            ..b
        };
        assert_eq!(vertices(&near, [255; 4], &camera)[2].color[3], 1.0);
    }

    #[test]
    fn halo_distance_also_caps_authored_bands_at_the_camera_far_plane() {
        let camera = Camera {
            far: 100.0,
            ..Camera::default()
        };
        let b = Beam {
            base: Vec3::new(0.0, 0.0, -95.0),
            view: Some(FarFade {
                begin: 300.0,
                end: 400.0,
            }),
            ..glow()
        };
        assert_eq!(vertices(&b, [255; 4], &camera)[2].color[3], 66.0 / 128.0);
        for far in [f32::INFINITY, f32::NAN, 0.0] {
            assert!(vertices(&b, [255; 4], &Camera { far, ..camera }).is_empty());
        }
    }

    #[test]
    fn beam_intensity_scales_both_beam_and_glow_alpha_and_drops_zero() {
        let cam = Camera::default();
        let dim_beam = Beam {
            intensity: 0.05,
            ..post()
        };
        let v = vertices(&dim_beam, [255, 253, 209, 255], &cam);
        let peak = v.iter().map(|p| p.color[3]).fold(0.0, f32::max);
        // 255 * 0.05 * 0.98 ≈ 12 -> half(12) = 6 -> 6 / 128.0
        assert!(peak > 0.0 && peak <= 7.0 / 128.0, "peak={peak}");
        assert!(vertices(
            &Beam {
                intensity: 0.0,
                ..post()
            },
            [255; 4],
            &cam,
        )
        .is_empty());

        let dim_glow = Beam {
            intensity: 0.05,
            ..glow()
        };
        let gv = vertices(&dim_glow, [255, 253, 209, 255], &cam);
        assert_eq!(gv[2].color[3], 7.0 / 128.0);
        assert!(vertices(
            &Beam {
                intensity: 0.0,
                ..glow()
            },
            [255; 4],
            &cam,
        )
        .is_empty());
    }
}
