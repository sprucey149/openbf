//! Native blade query against the stance hit primitives (B2 00168318 /
//! 0016a710): move the ray start back by width, expand each primitive by
//! that radius, and preserve the supplied length. No headshot multiplier.

use super::{BoltPrims, Vec3};
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
        let (from, dir) = self.to_local(start - direction * radius, direction);
        self.prims()
            .iter()
            .filter_map(|(prim, _)| {
                prim.ray(from, dir, radius)
                    .filter(|&t| t == 0.0 || t < length)
            })
            .reduce(|a, b| a.fmin(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Posture;

    fn body() -> BoltPrims {
        BoltPrims::new(Vec3::ZERO, 0.0, Posture::Stand, 1.0, None)
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
        let mut body = BoltPrims::new(Vec3::ZERO, 0.0, Posture::Prone, 1.0, None);
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
