//! Native blade query against the stance hit primitives (B2 00168318 /
//! 0016a710): move the ray start back by width, expand each primitive by
//! that radius, and preserve the supplied length. No headshot multiplier.

use super::{ray_sphere, ray_upright_cylinder, ray_yawed_box, BoltPrims, Stance, Vec3};
use crate::fmath::FloatExt;

impl BoltPrims {
    pub fn melee_hit(&self, start: Vec3, direction: Vec3, length: f32, radius: f32) -> bool {
        self.melee_contact(start, direction, length, radius)
            .is_some()
    }

    /// The nearest contact's distance along the ray. The native fraction is
    /// of the original length from the unshifted start (FUN_00340c60 places
    /// its hit effect there), so `start + direction * t` is the contact.
    pub fn melee_contact(
        &self,
        start: Vec3,
        direction: Vec3,
        length: f32,
        radius: f32,
    ) -> Option<f32> {
        if !length.is_finite()
            || !radius.is_finite()
            || length < 0.0
            || radius < 0.0
            || !start.is_finite()
            || !direction.is_finite()
            || direction.length_squared() == 0.0
        {
            return None;
        }
        let (sy, cy) = crate::fmath::sin_cos(self.yaw);
        let right = Vec3::new(cy, 0.0, -sy);
        let forward = Vec3::new(-sy, 0.0, -cy);
        let p = start - direction * radius - self.pos;
        let from = Vec3::new(p.dot(right), p.y, p.dot(forward));
        let dir = Vec3::new(direction.dot(right), direction.y, direction.dot(forward));
        let hit = |t: Option<f32>| t.filter(|&t| t == 0.0 || t < length);
        let sphere = |center, r| hit(ray_sphere(from, dir, center, r + radius));
        let cylinder = |center, r, hh| {
            hit(ray_upright_cylinder(
                from,
                dir,
                center,
                r + radius,
                hh + radius,
            ))
        };
        let nearest = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(a), Some(b)) => Some(a.fmin(b)),
            (a, b) => a.or(b),
        };
        let s = self.scale;
        let head = 0.17 * s.fmax(1.0);
        match self.stance {
            Stance::Prone | Stance::Roll | Stance::Dead => nearest(
                sphere(Vec3::new(0.0, 0.28 * s, 0.75 * s), head),
                hit(ray_yawed_box(
                    from,
                    dir,
                    Vec3::new(0.0, 0.2, -0.25 * s),
                    Vec3::X,
                    Vec3::Z,
                    Vec3::new(0.25, 0.2, 0.87 * s) + Vec3::splat(radius),
                )),
            ),
            Stance::Crouch => nearest(
                nearest(
                    sphere(Vec3::new(0.0, 1.2 * s, 0.25), head),
                    sphere(Vec3::new(0.0, 0.7 * s, 0.0), 0.4 * s),
                ),
                cylinder(Vec3::new(0.0, 0.35 * s, 0.0), 0.35, 0.35 * s),
            ),
            _ => nearest(
                nearest(
                    sphere(Vec3::new(0.0, 1.65 * s, 0.16), head),
                    sphere(Vec3::new(0.0, 1.15 * s, 0.0), 0.4 * s),
                ),
                cylinder(Vec3::new(0.0, 0.55 * s, 0.0), 0.35, 0.55 * s),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body() -> BoltPrims {
        BoltPrims {
            pos: Vec3::ZERO,
            yaw: 0.0,
            stance: Stance::Stand,
            scale: 1.0,
        }
    }

    #[test]
    fn melee_width_is_radius_and_inside_contacts_have_no_ray_gap() {
        let body = body();
        assert!(body.melee_hit(Vec3::new(0.49, 1.15, -2.0), Vec3::Z, 4.0, 0.1));
        assert!(!body.melee_hit(Vec3::new(0.51, 1.15, -2.0), Vec3::Z, 4.0, 0.1));
        assert!(body.melee_hit(Vec3::new(0.0, 1.15, 0.0), Vec3::Z, 0.01, 0.1));
        // yaw0 faces -Z: the standing head's local +.16 is world -.16.
        assert!(body.melee_hit(Vec3::new(-2.0, 2.0, -0.16), Vec3::X, 4.0, 0.2));
        assert!(!body.melee_hit(Vec3::new(-2.0, 2.03, -0.16), Vec3::X, 4.0, 0.2));
    }

    #[test]
    fn the_contact_is_the_nearest_prim_along_the_blade() {
        let body = body();
        let t = body
            .melee_contact(Vec3::new(0.0, 1.15, -2.0), Vec3::Z, 4.0, 0.1)
            .unwrap();
        // Torso sphere radius .4 + .1, ray start shifted back .1: 2.1 - .5.
        assert!((t - 1.6).abs() < 1e-4, "{t}");
    }

    #[test]
    fn native_width_moves_start_back_without_extending_the_length() {
        let body = body();
        let start = Vec3::new(0.0, 1.15, -1.0);
        // Expanded torso radius .5: native axis begins -1.1, so .55
        // misses; an unshifted rounded capsule incorrectly reaches it.
        assert!(!body.melee_hit(start, Vec3::Z, 0.55, 0.1));
        assert!(body.melee_hit(start, Vec3::Z, 0.61, 0.1));
    }

    #[test]
    fn prone_box_uses_expanded_square_corners_in_its_rotated_frame() {
        let mut body = body();
        body.stance = Stance::Prone;
        let local_start = Vec3::new(0.34, 0.49, -1.0);
        assert!(body.melee_hit(local_start, Vec3::Z, 2.0, 0.1));
        assert!(!body.melee_hit(Vec3::new(0.36, 0.49, -1.0), Vec3::Z, 2.0, 0.1));
        body.pos = Vec3::new(3.0, 2.0, -4.0);
        body.yaw = 0.7;
        let rotate = glam::Mat4::from_rotation_y(body.yaw);
        assert!(body.melee_hit(
            body.pos + rotate.transform_vector3(local_start),
            rotate.transform_vector3(Vec3::Z),
            2.0,
            0.1
        ));
    }
}
