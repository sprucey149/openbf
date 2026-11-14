//! Poses and placement: the TAT1 golden pose, a `(x, y, z, yaw°, pitch°)`
//! pose as a [`render::Camera`], the XFRM instance transform (the convention
//! `viewer.rs`'s header records), and the robust auto-framing behind the
//! overview camera, the orbit and the gallery.

use crate::assets::Scene;
use core::fmath::FloatExt;
use glam::{Mat3, Mat4, Vec3};

/// TAT1 golden/default camera pose (x, y, z, yaw°, pitch°): dune overlook
/// framing the sandcrawler (-117, 8, 208) among the rock spires. Shared by
/// the viewer default and the Tier-2 golden test — change = new golden.
pub const GOLDEN_CAM: (f32, f32, f32, f32, f32) = (-100.0, 60.0, 330.0, 8.0, -23.0);

pub fn camera_from(pose: (f32, f32, f32, f32, f32)) -> render::Camera {
    render::Camera {
        position: Vec3::new(pose.0, pose.1, pose.2),
        yaw: pose.3.to_radians(),
        pitch: pose.4.to_radians(),
        ..render::Camera::default()
    }
}

pub fn instance_matrix(rotation: [[f32; 3]; 3], translation: [f32; 3]) -> Mat4 {
    let rot = Mat3::from_cols(
        Vec3::from(rotation[0]),
        Vec3::from(rotation[1]),
        Vec3::from(rotation[2]),
    );
    Mat4::from_translation(Vec3::from(translation)) * Mat4::from_mat3(rot)
}

/// Axis-aligned bounds that a single stray instance cannot blow up.
///
/// Death Star ships exactly one `dea1_prop_tiefighter` at x = 828 while 95 % of
/// the map sits at x <= -46; taken literally that stretches the map from ~250 m
/// to ~1025 m and the overview frames a speck. Reject per axis with Tukey's
/// "far out" fence (3x the interquartile range beyond the quartiles), which is
/// the standard robust-outlier rule and, crucially, drops NOTHING on a map
/// whose instances are spread normally — so a well-behaved map's framing (and
/// its golden) is untouched. If the fence would reject more than 5 % of the
/// instances the map is simply spread out, not strayed, and the raw bounds
/// stand.
pub(crate) fn robust_bounds(pts: &[Vec3]) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    if pts.is_empty() {
        return (lo, hi);
    }
    let mut kept: Vec<bool> = vec![true; pts.len()];
    if pts.len() >= 8 {
        for axis in 0..3 {
            let mut v: Vec<f32> = pts.iter().map(|p| p[axis]).collect();
            v.sort_by(f32::total_cmp);
            let q = |f: f32| v[((v.len() as f32 - 1.0) * f).round() as usize];
            let (q1, q3) = (q(0.25), q(0.75));
            let fence = 3.0 * (q3 - q1);
            if fence <= 0.0 {
                continue;
            }
            let (min_ok, max_ok) = (q1 - fence, q3 + fence);
            let rejected = pts
                .iter()
                .filter(|p| p[axis] < min_ok || p[axis] > max_ok)
                .count();
            if rejected * 20 > pts.len() {
                continue; // >5 % outside: spread out, not strayed.
            }
            for (k, p) in pts.iter().enumerate() {
                if p[axis] < min_ok || p[axis] > max_ok {
                    kept[k] = false;
                }
            }
        }
    }
    for (k, p) in pts.iter().enumerate() {
        if kept[k] {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    if lo.x > hi.x {
        // Everything rejected (cannot happen with the 5 % guard, but the
        // caller must never see an inverted box).
        for p in pts {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    (lo, hi)
}

/// What a map's auto-framing is built on: the centre of its (robust) instance
/// bounds and a radius that covers them.
///
/// Exposed because more than one caller needs the FRAMING, not just the
/// camera. `capture_orbit` used to recover these by inverting the eye
/// placement below, which silently assumed every map's bounds were centred on
/// the world origin — they are not, and the maps furthest off-origin (BF2
/// TAN1, BF1 KAM1, BF2 DEA1) got both a wrong centre and a wrong radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overview {
    pub center: Vec3,
    pub radius: f32,
}

/// The framing [`overview_camera`] uses: robust instance bounds, falling back
/// to the terrain's extent only when there are no instances at all.
///
/// `None` for a scene with neither (the unit sheets and the pose preview).
pub fn overview_bounds(scene: &Scene) -> Option<Overview> {
    let pts: Vec<Vec3> = scene
        .manifest
        .instances
        .iter()
        .map(|i| Vec3::from(i.translation))
        .collect();
    let (mut lo, mut hi) = robust_bounds(&pts);
    // Terrain only frames the shot when there is nothing else: a map like
    // BES1 (Bespin Platforms) has a cloud-sea terrain spanning +-7 km around
    // 400 m of actual platforms, and framing that shows nothing but sky.
    if lo.x > hi.x {
        if let Some(t) = &scene.terrain {
            for p in &t.positions {
                let p = Vec3::from(*p);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    if lo.x > hi.x {
        return None;
    }
    Some(Overview {
        center: (lo + hi) * 0.5,
        radius: ((hi - lo) * 0.5).length().fmax(10.0),
    })
}

/// A 3/4 overview camera framing the whole map, from the baked instance
/// positions (and the terrain's extent when there is one). Used by
/// `--cam-overview` to eyeball a freshly baked map; the committed TAT1 golden
/// keeps its own fixed GOLDEN_CAM.
pub fn overview_camera(scene: &Scene) -> render::Camera {
    let Some(o) = overview_bounds(scene) else {
        return render::Camera::default();
    };
    // Behind (+Z), right (+X) and above, looking back at the centre.
    let eye = o.center + Vec3::new(o.radius * 0.75, o.radius * 0.55, o.radius * 0.75);
    look_at(eye, o.center)
}

/// Where the orbit camera sits at bearing `a` (radians) and height fraction
/// `lift` of the radius. Shared by the flythrough and by the referee test
/// `game/tests/all_maps_load.rs::every_map_orbits_its_own_centre` so the
/// measurement matches the shot.
pub fn orbit_eye(o: Overview, a: f32, lift: f32) -> Vec3 {
    o.center
        + Vec3::new(
            a.sin() * o.radius * 0.6,
            o.radius * lift,
            a.cos() * o.radius * 0.6,
        )
}

/// A camera at `eye` pointed at `target`.
pub fn look_at(eye: Vec3, target: Vec3) -> render::Camera {
    let d = target - eye;
    let flat = (d.x * d.x + d.z * d.z).sqrt();
    render::Camera {
        position: eye,
        yaw: (-d.x).atan2(-d.z),
        pitch: d.y.atan2(flat),
        ..render::Camera::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_matrix_places_rows_as_basis_vectors() {
        // Identity round-trips.
        let m = instance_matrix(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [10.0, 20.0, 30.0],
        );
        assert_eq!(m.transform_point3(Vec3::ZERO), Vec3::new(10.0, 20.0, 30.0));

        // Row-vector convention: row 0 is where the object's +X axis lands.
        // A 90° yaw whose stored row0 = (0,0,-1) must send +X to -Z.
        let m = instance_matrix(
            [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
            [0.0; 3],
        );
        let x = m.transform_vector3(Vec3::X);
        assert!((x - Vec3::NEG_Z).length() < 1e-6, "got {x}");
    }
}
