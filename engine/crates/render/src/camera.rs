//! Perspective fly-camera.
//!
//! Convention (documented once, here): **Y-up, right-handed, -Z forward**.
//! - `yaw = 0, pitch = 0` looks down **-Z**.
//! - `yaw` rotates around **+Y**; positive yaw turns **left** (toward -X).
//! - `pitch` rotates around the camera's right axis; positive pitch looks **up**.
//! - Projection is `glam::Mat4::perspective_rh` (0..1 depth, wgpu clip space).
//!
//! Adapted from an earlier project's camera (`build_uniform`: `look_to_rh` + `perspective_rh`,
//! view_proj = proj * view), simplified to a
//! yaw/pitch fly-cam — the viewer has no vehicle to follow.

use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    /// Radians, rotation around +Y. 0 = looking down -Z; positive turns left.
    pub yaw: f32,
    /// Radians, positive looks up. Clamp near ±PI/2 yourself when driving from input.
    pub pitch: f32,
    pub roll: f32,
    /// Vertical field of view, radians. The port is widescreen-native hor+
    /// (PLAN §Presentation): fov_y holds the traced PS2 vertical FOV and the
    /// aspect ratio widens the horizontal view — never stretch. The default
    /// 70° is for free / test / referee cameras; the soldier's chase camera
    /// takes the PS2's measured value (`game::walk::ps2_fov_y`).
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_y: 70f32.to_radians(),
            near: 0.1,
            far: 2000.0,
        }
    }
}

impl Camera {
    pub fn reaching(&self, far: f32) -> Camera {
        Camera {
            far: self.far.max(far),
            ..*self
        }
    }

    /// World-space look direction. R_y(yaw) then pitch: unit length by construction.
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }

    /// World-space right axis (ignores pitch; horizontal strafe direction).
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        glam::Quat::from_axis_angle(self.forward(), self.roll) * Vec3::new(cy, 0.0, -sy)
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_to_rh(
            self.position,
            self.forward(),
            self.right().cross(self.forward()),
        )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect, self.near, self.far) * self.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    #[test]
    fn reaching_only_ever_extends_the_far_plane() {
        let cam = Camera {
            far: 2000.0,
            ..Camera::default()
        };
        assert_eq!(cam.reaching(9000.0).far, 9000.0);
        assert_eq!(cam.reaching(500.0).far, 2000.0);
        assert_eq!(cam.reaching(9000.0).near, cam.near);
    }

    #[test]
    fn forward_follows_the_documented_convention() {
        let mut cam = Camera::default();
        assert!(
            cam.forward().abs_diff_eq(Vec3::NEG_Z, EPS),
            "yaw=0 must look down -Z"
        );
        assert!(cam.right().abs_diff_eq(Vec3::X, EPS));

        cam.yaw = std::f32::consts::FRAC_PI_2;
        assert!(
            cam.forward().abs_diff_eq(Vec3::NEG_X, EPS),
            "positive yaw turns left (-X)"
        );

        cam.yaw = 0.0;
        cam.pitch = std::f32::consts::FRAC_PI_2;
        assert!(
            cam.forward().abs_diff_eq(Vec3::Y, EPS),
            "positive pitch looks up (+Y)"
        );
    }

    #[test]
    fn view_proj_maps_a_point_ahead_to_ndc_center_with_wgpu_depth() {
        let cam = Camera::default();
        let vp = cam.view_proj(16.0 / 9.0);

        let clip = vp * Vec3::new(0.0, 0.0, -10.0).extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(
            ndc.x.abs() < EPS && ndc.y.abs() < EPS,
            "point ahead lands on screen center"
        );
        assert!(
            ndc.z > 0.0 && ndc.z < 1.0,
            "wgpu depth range is [0,1], got {}",
            ndc.z
        );

        // Depth must increase toward the far plane (perspective_rh is 0-at-near).
        let near_clip = vp * Vec3::new(0.0, 0.0, -1.0).extend(1.0);
        let far_clip = vp * Vec3::new(0.0, 0.0, -1000.0).extend(1.0);
        assert!(near_clip.z / near_clip.w < far_clip.z / far_clip.w);
    }
}
