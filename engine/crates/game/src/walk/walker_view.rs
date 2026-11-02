use super::{Camera, Mat4, Vec3, WalkSim};
use crate::vehicle_class::SeatSpec;
use core::vehicle::{Vehicle, VehicleKind};

pub fn camera_on_frame(
    frame: Mat4,
    center: Vec3,
    offset: Vec3,
    tilt: f32,
    fov_y: f32,
) -> (Vec3, Camera) {
    let forward = frame.z_axis.truncate().normalize();
    let up = frame.y_axis.truncate().normalize();
    let right = -frame.x_axis.truncate().normalize();
    let (s, c) = core::fmath::sin_cos(tilt.to_radians());
    let forward = forward * c - up * s;
    let pitch = core::fmath::asin(forward.y.clamp(-1.0, 1.0));
    let yaw = core::fmath::atan2(-forward.x, -forward.z);
    let (sy, cy) = core::fmath::sin_cos(yaw);
    let base_right = Vec3::new(cy, 0.0, -sy);
    let base_up = base_right.cross(forward);
    let roll = core::fmath::atan2(-right.dot(base_up), right.dot(base_right));
    let target = frame.transform_point3(center);
    let mut camera = Camera {
        yaw,
        pitch,
        roll,
        fov_y,
        ..Default::default()
    };
    camera.position = target - camera.forward() * offset.z
        + camera.right() * offset.x
        + camera.right().cross(camera.forward()) * offset.y;
    (target, camera)
}

impl WalkSim {
    pub fn load_walker_views(&mut self, manifest: &manifest::Manifest, root: &std::path::Path) {
        self.walker_rigs = crate::viewer::walker_rigs(manifest, root, &self.vehicle_geometry);
    }

    /// Read each vehicle class's model top, once [`Self::vehicle_geometry`] is set.
    pub fn load_vehicle_tops(
        &mut self,
        manifest: &manifest::Manifest,
        map_dir: &std::path::Path,
        root: &std::path::Path,
    ) {
        self.vehicle_tops =
            crate::viewer::model_tops(manifest, map_dir, root, &self.vehicle_geometry);
    }

    pub(super) fn walker_fovs(&self) -> Option<(f32, f32)> {
        let mount = self.sim.unit(self.player)?.mount?;
        let v = self.sim.vehicle(mount.vehicle)?;
        if v.kind != VehicleKind::Walker {
            return None;
        }
        let s = self
            .vehicle_seats
            .get(v.class as usize)?
            .get(mount.seat as usize)?;
        Some((
            s.third_person_fov.unwrap_or(48.0),
            s.first_person_fov.unwrap_or(48.0),
        ))
    }

    pub(super) fn walker_seat_camera(
        &self,
        v: &Vehicle,
        seat: Option<&SeatSpec>,
        look: Option<(f32, f32)>,
    ) -> Option<(Vec3, Camera)> {
        if v.kind != VehicleKind::Walker {
            return None;
        }
        let seat = seat?;
        let look = look.unwrap_or((0.0, 0.0));
        let mount = self.sim.unit(self.player)?.mount?;
        let model = Mat4::from_rotation_translation(v.rot, v.pos);
        let frame = if mount.seat == 0 {
            model * crate::viewer::turret_pose::turret_node(&Mat4::IDENTITY, look.0, -look.1)
        } else if let Some(rig) = self
            .walker_rigs
            .get(v.class as usize)
            .and_then(Option::as_ref)
        {
            let scale = self
                .sim
                .walker_params
                .get(v.class as usize)
                .and_then(Option::as_ref)
                .map_or(1.0, |p| p.root_scale);
            model * rig.seat_frame(v, scale, seat.turret_node.as_deref(), look)
        } else {
            let pivot = self
                .sim
                .seat_look(self.player)
                .filter(|l| l.gun.framed)
                .map_or(Vec3::ZERO, |l| l.gun.pivot);
            model
                * crate::viewer::turret_pose::turret_node(
                    &Mat4::from_translation(pivot),
                    look.0,
                    -look.1,
                )
        };
        let (_, cam) = camera_on_frame(
            frame,
            Vec3::from(seat.track_center.unwrap_or([0.0, 2.0, 0.0])),
            Vec3::from(seat.track_offset.unwrap_or([0.0, 1.0, 8.0])),
            seat.tilt_value.unwrap_or(0.0),
            self.fov_y(),
        );
        // The pilot's pull point by the walker's model, a gunner's his own
        // turret's fixed point, each on the frame the camera hangs on.
        let height = if mount.seat == 0 {
            let top = self.vehicle_tops.get(v.class as usize).copied().flatten();
            super::vehicle_pull_height(v.kind, top)
        } else {
            super::CAMERA_PULL_Y
        };
        Some((frame.transform_point3(Vec3::Y * height), cam))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_matches_paused_geonosis_main_gunner_frame() {
        let frame = Mat4::from_cols(
            Vec3::new(-0.95425355, -0.018550225, -0.29840407).extend(0.0),
            Vec3::new(-0.093857266, 0.96620506, 0.24007882).extend(0.0),
            Vec3::new(0.28386605, 0.25710633, -0.9237457).extend(0.0),
            Vec3::new(40.235588, -56.557163, 172.20389).extend(1.0),
        );
        let (_, camera) = camera_on_frame(frame, Vec3::new(0.0, 3.0, -7.0), Vec3::ZERO, 0.0, 0.76);
        assert!(camera
            .position
            .abs_diff_eq(Vec3::new(37.96695, -55.45828, 179.39032), 0.002));
        assert!(camera
            .forward()
            .abs_diff_eq(frame.z_axis.truncate(), 0.0001));
        assert!(camera.right().abs_diff_eq(-frame.x_axis.truncate(), 0.0001));
    }
}
