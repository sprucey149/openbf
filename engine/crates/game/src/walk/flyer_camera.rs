use super::*;
use crate::vehicle_class::SeatSpec;
use core::fmath;
use core::soldier::GroundRules;
use core::vehicle::hover::{quat_x, quat_y};
use core::vehicle::{Vehicle, VehicleHandle, VehicleKind};

const BF2_TILT: f32 = 0.087_665;
const BF1_AIM_TENSION: f32 = 30.0;
const DEFAULT_MOVE_TENSION: [f32; 2] = [30.0; 2];
const DEGREES: f32 = 0.017_453_294;
const DEFAULT_THIRD_FOV: f32 = 48.0;
const GUNNER_AIM_TENSION: f32 = 1000.0;
/// The camera detach factor during Falling / Crashed (SWBFRNT2.ELF FUN_0020d450 line 122651:
/// move tension scaled by 0.25f, allowing camera to lag behind falling/crashing craft).
pub const FLYER_CRASH_DETACH_RATE: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyerCam {
    pub vehicle: VehicleHandle,
    pub eye: Vec3,
    pub rot: Quat,
    pub fov: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunnerCam {
    pub vehicle: VehicleHandle,
    pub seat: u8,
    pub eye: Vec3,
    pub rot: Quat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyerSection {
    pub center: Vec3,
    pub offset: Vec3,
    pub tilt: f32,
    pub aim_tension: Option<f32>,
    pub move_tension: [[f32; 2]; 3],
    pub third_fov: f32,
    pub fast_fov: f32,
    pub slow_offset: Vec3,
    pub fast_offset: Vec3,
}

impl FlyerSection {
    pub fn of(seat: Option<&SeatSpec>, rules: GroundRules) -> Self {
        let center = seat.and_then(|s| s.track_center.or(s.eye_point_offset));
        let offset = seat.and_then(|s| s.track_offset);
        let tension = |axis: usize| {
            seat.and_then(|s| s.move_tension[axis])
                .unwrap_or(DEFAULT_MOVE_TENSION)
        };
        let bf2 = rules == GroundRules::Bf2;
        FlyerSection {
            center: center.map_or(Vec3::ZERO, Vec3::from),
            offset: offset.map_or(Vec3::ZERO, Vec3::from),
            tilt: if bf2 {
                BF2_TILT
            } else {
                seat.and_then(|s| s.tilt_value).unwrap_or(0.0) * DEGREES
            },
            aim_tension: (!bf2)
                .then(|| seat.and_then(|s| s.aim_tension).unwrap_or(BF1_AIM_TENSION)),
            move_tension: [tension(0), tension(1), tension(2)],
            third_fov: seat
                .and_then(|s| s.third_person_fov)
                .unwrap_or(DEFAULT_THIRD_FOV),
            fast_fov: seat.and_then(|s| s.fast_fov).unwrap_or(0.0),
            slow_offset: seat
                .and_then(|s| s.slow_offset)
                .map_or(Vec3::ZERO, Vec3::from),
            fast_offset: seat
                .and_then(|s| s.fast_offset)
                .map_or(Vec3::ZERO, Vec3::from),
        }
    }

    pub fn gunner(seat: Option<&SeatSpec>) -> Self {
        FlyerSection {
            tilt: seat.and_then(|s| s.tilt_value).unwrap_or(0.0) * DEGREES,
            aim_tension: Some(
                seat.and_then(|s| s.aim_tension)
                    .unwrap_or(GUNNER_AIM_TENSION),
            ),
            ..Self::of(seat, GroundRules::Bf1)
        }
    }

    /// A hover's pilot section (`analysis/re-hover-camera.md` 2): the
    /// BODY's `TrackCenter` on the untilted body, `TrackOffset` along the
    /// view, `TiltValue` in degrees, the section's `AimTension` (ctor 1000)
    /// and `MoveTension`, and `BoostFOV` as the fast end of the FOV (B2
    /// `FUN_00221388`). No slow or fast offset: the hover passes none.
    pub fn hover(seat: Option<&SeatSpec>) -> Self {
        FlyerSection {
            tilt: seat.and_then(|s| s.tilt_value).unwrap_or(0.0) * DEGREES,
            aim_tension: Some(
                seat.and_then(|s| s.aim_tension)
                    .unwrap_or(GUNNER_AIM_TENSION),
            ),
            fast_fov: seat.and_then(|s| s.boost_fov).unwrap_or(0.0),
            slow_offset: Vec3::ZERO,
            fast_offset: Vec3::ZERO,
            ..Self::of(seat, GroundRules::Bf1)
        }
    }

    pub fn speed_fraction(&self, speed: f32, top: f32) -> f32 {
        if speed > 0.0 && self.fast_fov != 0.0 && top > 0.0 {
            (speed / top).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn offset_at(&self, fraction: f32) -> Vec3 {
        let offset = self.slow_offset * (1.0 - fraction) + self.fast_offset * fraction;
        if offset == Vec3::ZERO {
            self.offset
        } else {
            offset
        }
    }

    pub fn fov_at(&self, fraction: f32) -> f32 {
        if self.fast_fov == 0.0 {
            self.third_fov
        } else {
            self.third_fov * (1.0 - fraction) + self.fast_fov * fraction
        }
    }

    /// The move tensions while a craft falls or has crashed.
    fn crash_tension(&self) -> [[f32; 2]; 3] {
        self.move_tension
            .map(|axis| axis.map(|t| t * FLYER_CRASH_DETACH_RATE))
    }
}

pub fn flyer_view(frame: Quat, tilt: f32) -> Quat {
    frame * quat_y(std::f32::consts::PI) * quat_x(-tilt)
}

pub fn lag_eye(previous: Vec3, ideal: Vec3, rot: Quat, tension: &[[f32; 2]; 3], dt: f32) -> Vec3 {
    let step = rot.inverse() * (ideal - previous);
    let eased = |value: f32, axis: &[f32; 2]| {
        let rate = if value > 0.0 { axis[0] } else { axis[1] };
        value * (1.0 - fmath::exp(-rate * dt))
    };
    previous
        + rot
            * Vec3::new(
                eased(step.x, &tension[0]),
                eased(step.y, &tension[1]),
                eased(step.z, &tension[2]),
            )
}

/// Is a held eye close enough to its view pivot to keep easing toward it?
fn within_snap(centre: Vec3, from: Vec3) -> bool {
    centre.distance_squared(from) <= CAMERA_SNAP_SQUARED
}

/// The eye `from` eased toward `settled`, or `settled` at once when `held` is
/// false (the pivot jumped past the snap distance).
fn kept_eye(held: bool, from: Vec3, settled: Vec3, rot: Quat, tension: &[[f32; 2]; 3]) -> Vec3 {
    if held {
        lag_eye(from, settled, rot, tension, core::TICK_DT)
    } else {
        settled
    }
}

/// The PS2's FOV ease toward `goal`.
fn ease_fov(goal: f32, was: f32) -> f32 {
    goal + (was - goal) * fmath::exp(-ZOOM_EASE_RATE * core::TICK_DT)
}

pub fn camera_at(position: Vec3, rot: Quat, fov_y: f32) -> Camera {
    let forward = rot * Vec3::NEG_Z;
    let right = rot * Vec3::X;
    let yaw = fmath::atan2(-forward.x, -forward.z);
    let (sin, cos) = fmath::sin_cos(yaw);
    let flat = Vec3::new(cos, 0.0, -sin);
    Camera {
        position,
        yaw,
        pitch: fmath::asin(forward.y.clamp(-1.0, 1.0)),
        roll: fmath::atan2(right.dot(forward.cross(flat)), right.dot(flat)),
        fov_y,
        ..Camera::default()
    }
}

impl WalkSim {
    fn flyer_pilot(&self) -> Option<(&Vehicle, VehicleHandle)> {
        if self.sim.seat_zoom(self.player).is_some() {
            return None;
        }
        let mount = self.sim.unit(self.player)?.mount?;
        let v = self.sim.vehicle(mount.vehicle)?;
        (matches!(v.kind, VehicleKind::Flyer | VehicleKind::Hover) && mount.seat == 0)
            .then_some((v, mount.vehicle))
    }

    /// The body frame a pilot's camera hangs on: the craft or hover untilted.
    fn pilot_frame(v: &Vehicle) -> Quat {
        match v.kind {
            VehicleKind::Hover => core::vehicle::hover::untilted_rot(v),
            _ => core::vehicle::flyer::untilted(v),
        }
    }

    /// A pilot's view: a flyer's along its body; a hover's along its aim
    /// (E+0x350, the aim's pitch on the body: re-hover §5.1), each tilted
    /// down by the section's tilt.
    fn pilot_view(v: &Vehicle, frame: Quat, section: &FlyerSection) -> Quat {
        let pitch = match v.kind {
            VehicleKind::Hover => v
                .guns
                .first()
                .and_then(Option::as_ref)
                .map_or(0.0, |g| g.look.1),
            _ => 0.0,
        };
        flyer_view(frame * quat_x(-pitch), section.tilt)
    }

    pub(super) fn flyer_fov(&self) -> Option<f32> {
        let (_, handle) = self.flyer_pilot()?;
        self.cam_flyer
            .filter(|c| c.vehicle == handle)
            .map(|c| c.fov)
    }

    fn flyer_speed_fraction(&self, v: &Vehicle, section: &FlyerSection) -> f32 {
        if v.kind == VehicleKind::Hover {
            let Some(p) = self
                .sim
                .hover_params
                .get(v.class as usize)
                .and_then(Option::as_ref)
            else {
                return 0.0;
            };
            let (fs, bs) = (p.forward_speed, p.boost_speed);
            let forward = v.vel.dot(v.rot * Vec3::Z);
            return if section.fast_fov != 0.0 && forward > fs && bs > fs {
                ((forward - fs) / (bs - fs)).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
        let top = self
            .sim
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |p| {
                let region = self.sim.landing_regions.iter().any(|r| r.contains(v.pos));
                p.speeds(region)[2].max(p.boost_speed)
            });
        section.speed_fraction(v.vel.dot(v.rot * Vec3::Z), top)
    }

    fn flyer_section(&self, v: &Vehicle) -> FlyerSection {
        let seat = self
            .vehicle_seats
            .get(v.class as usize)
            .and_then(|seats| seats.first());
        if v.kind == VehicleKind::Hover {
            return FlyerSection::hover(seat);
        }
        FlyerSection::of(seat, self.sim.ground_rules)
    }

    fn flyer_ideal(
        &self,
        v: &Vehicle,
        section: &FlyerSection,
        frame: Quat,
        rot: Quat,
    ) -> (Vec3, Vec3) {
        let target = v.pos + frame * self.seat_pull_origin(v, 0);
        let target = self.pull_point_on_own_side(v, target);
        let offset = section.offset_at(self.flyer_speed_fraction(v, section));
        (target, v.pos + frame * section.center + rot * offset)
    }

    pub(super) fn flyer_seat_camera(&self) -> Option<(Camera, Option<Track>)> {
        let (v, handle) = self.flyer_pilot()?;
        let section = self.flyer_section(v);
        let frame = Self::pilot_frame(v);
        let held = self.cam_flyer.filter(|c| c.vehicle == handle);
        let rot = held.map_or_else(|| Self::pilot_view(v, frame, &section), |c| c.rot);
        let (target, ideal) = self.flyer_ideal(v, &section, frame, rot);
        let (settled, track) =
            self.settle_camera(CamKind::Seat, target, ideal, v.instance, Some(handle));
        let position = held.map_or(settled, |c| c.eye);
        Some((camera_at(position, rot, self.fov_y()), Some(track)))
    }

    pub(super) fn next_cam_flyer(&self, previous: Vec3) -> Option<FlyerCam> {
        let (v, handle) = self.flyer_pilot()?;
        let section = self.flyer_section(v);
        let frame = Self::pilot_frame(v);
        let view = Self::pilot_view(v, frame, &section);
        let center = v.pos + frame * section.center;
        let held = self
            .cam_flyer
            .filter(|c| c.vehicle == handle && within_snap(center, c.eye));
        let rot = match (held, section.aim_tension) {
            (Some(c), Some(aim)) => c.rot.lerp(view, 1.0 - fmath::exp(-aim * core::TICK_DT)),
            _ => view,
        };
        let (target, ideal) = self.flyer_ideal(v, &section, frame, rot);
        let (settled, _) =
            self.settle_camera(CamKind::Seat, target, ideal, v.instance, Some(handle));
        let move_tension = if matches!(
            v.flyer.phase,
            core::vehicle::flyer::FlightPhase::Falling | core::vehicle::flyer::FlightPhase::Crashed
        ) {
            section.crash_tension()
        } else {
            section.move_tension
        };
        let from = held.map_or(previous, |c| c.eye);
        let eye = kept_eye(within_snap(center, from), from, settled, rot, &move_tension);
        let goal = section.fov_at(self.flyer_speed_fraction(v, &section));
        let was = self
            .cam_flyer
            .filter(|c| c.vehicle == handle)
            .map_or_else(|| chase_fovs().0, |c| c.fov);
        Some(FlyerCam {
            vehicle: handle,
            eye,
            rot,
            fov: ease_fov(goal, was),
        })
    }
}

impl WalkSim {
    fn flyer_gunner(&self) -> Option<(&Vehicle, VehicleHandle, u8, FlyerSection, Mat4)> {
        if self.sim.seat_zoom(self.player).is_some() {
            return None;
        }
        let mount = self.sim.unit(self.player)?.mount?;
        let v = self.sim.vehicle(mount.vehicle)?;
        if !matches!(v.kind, VehicleKind::Flyer | VehicleKind::Hover) || mount.seat == 0 {
            return None;
        }
        let seat = self
            .vehicle_seats
            .get(v.class as usize)?
            .get(usize::from(mount.seat))?;
        let l = self.sim.seat_look(self.player)?;
        let [x, y, z] = core::vehicle::gun::frame(l.look);
        let pivot = if l.gun.framed {
            l.gun.pivot
        } else {
            Vec3::ZERO
        };
        let frame = Mat4::from_rotation_translation(v.rot, v.pos)
            * Mat4::from_cols(
                x.extend(0.0),
                y.extend(0.0),
                z.extend(0.0),
                pivot.extend(1.0),
            );
        Some((
            v,
            mount.vehicle,
            mount.seat,
            FlyerSection::gunner(Some(seat)),
            frame,
        ))
    }

    fn gunner_settled(
        &self,
        v: &Vehicle,
        handle: VehicleHandle,
        section: &FlyerSection,
        frame: Mat4,
        rot: Quat,
    ) -> (Vec3, Track) {
        let target = frame.transform_point3(Vec3::Y * CAMERA_PULL_Y);
        let ideal = frame.transform_point3(section.center) + rot * section.offset;
        self.settle_camera(CamKind::Seat, target, ideal, v.instance, Some(handle))
    }

    pub(super) fn flyer_gunner_camera(&self) -> Option<(Camera, Option<Track>)> {
        let (v, handle, seat, section, frame) = self.flyer_gunner()?;
        let held = self
            .cam_gunner
            .filter(|c| c.vehicle == handle && c.seat == seat);
        let rot = held.map_or_else(
            || flyer_view(frame_rotation(frame), section.tilt),
            |c| c.rot,
        );
        let (settled, track) = self.gunner_settled(v, handle, &section, frame, rot);
        let position = held.map_or(settled, |c| c.eye);
        Some((camera_at(position, rot, self.fov_y()), Some(track)))
    }

    pub(super) fn next_cam_gunner(&self, previous: &Camera) -> Option<GunnerCam> {
        let (v, handle, seat, section, frame) = self.flyer_gunner()?;
        let view = flyer_view(frame_rotation(frame), section.tilt);
        let center = frame.transform_point3(section.center);
        let (from_eye, from_rot) = self
            .cam_gunner
            .filter(|c| c.vehicle == handle && c.seat == seat)
            .map_or_else(
                || {
                    (
                        previous.position,
                        camera::camera_rotation(previous.yaw, previous.pitch, previous.roll),
                    )
                },
                |c| (c.eye, c.rot),
            );
        let held = within_snap(center, from_eye);
        let rot = if held {
            let aim = section.aim_tension.unwrap_or(GUNNER_AIM_TENSION);
            crate::sim_pose::quat_slerp(from_rot, view, 1.0 - fmath::exp(-aim * core::TICK_DT))
        } else {
            view
        };
        let (settled, _) = self.gunner_settled(v, handle, &section, frame, rot);
        let eye = kept_eye(held, from_eye, settled, rot, &section.move_tension);
        Some(GunnerCam {
            vehicle: handle,
            seat,
            eye,
            rot,
        })
    }
}

fn frame_rotation(frame: Mat4) -> Quat {
    Quat::from_mat3(&glam::Mat3::from_mat4(frame)).normalize()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidedCam {
    pub rocket: u64,
    pub eye: Vec3,
    pub rot: Quat,
    pub fov: f32,
}

pub fn guided_section(view: &core::guided::GuidedView) -> FlyerSection {
    let mut seat = SeatSpec::new(core::vehicle::SeatKind::Pilot);
    seat.eye_point_offset = view.eye_point_offset;
    seat.track_center = view.track_center;
    seat.track_offset = view.track_offset;
    seat.slow_offset = view.slow_offset;
    seat.fast_offset = view.fast_offset;
    seat.fast_fov = view.fast_fov;
    seat.third_person_fov = view.third_person_fov;
    seat.move_tension = view.move_tension;
    FlyerSection::of(Some(&seat), GroundRules::Bf2)
}

impl WalkSim {
    fn guided_flown(&self) -> Option<(&core::guided::GuidedRocket, &core::guided::GuidedClass)> {
        let r = self.sim.controlled_guided(self.player)?;
        Some((r, self.sim.guided_class(r)?))
    }

    pub(super) fn guided_fov(&self) -> Option<f32> {
        let (r, _) = self.guided_flown()?;
        self.cam_guided.filter(|c| c.rocket == r.id).map(|c| c.fov)
    }

    fn guided_ideal(
        &self,
        r: &core::guided::GuidedRocket,
        c: &core::guided::GuidedClass,
        section: &FlyerSection,
        rot: Quat,
    ) -> (Vec3, Vec3) {
        let frame = r.frame();
        let top = self.guided_tops.get(r.weapon_class).copied().flatten();
        let target = r.pos + frame * (Vec3::Y * vehicle_pull_height(VehicleKind::Flyer, top));
        let fraction = section.speed_fraction(r.speed(), c.top_speed());
        let ideal = r.pos + frame * section.center + rot * section.offset_at(fraction);
        let (settled, _) = self.settle_camera(CamKind::Seat, target, ideal, None, None);
        (target, settled)
    }

    pub(super) fn guided_seat_camera(&self) -> Option<Camera> {
        let (r, c) = self.guided_flown()?;
        let held = self.cam_guided.filter(|h| h.rocket == r.id);
        let (eye, rot) = match held {
            Some(h) => (h.eye, h.rot),
            None => {
                let section = guided_section(&c.view);
                let rot = flyer_view(r.frame(), section.tilt);
                (self.guided_ideal(r, c, &section, rot).1, rot)
            }
        };
        Some(camera_at(eye, rot, self.fov_y()))
    }

    fn guided_hold_clear(&self, from: Vec3, to: Vec3) -> bool {
        if self
            .camera_blockers
            .fraction(from, to, None, |i| self.station_pose(i), &self.sim.doors)
            < 1.0
        {
            return false;
        }
        let Some(h) = &self.sim.heightfield else {
            return true;
        };
        let (dir, len) = ((to - from).normalize_or_zero(), (to - from).length());
        core::collision::ray_heightfield(h, &self.sim.collision.cutters, from, dir, len, 0.25)
            .is_none()
    }

    pub(super) fn next_cam_guided(&self, previous: Vec3) -> Option<GuidedCam> {
        let (r, c) = self.guided_flown()?;
        let held = self.cam_guided.filter(|h| h.rocket == r.id);
        if !r.live() {
            let mut cam = held?;
            let back = -(r.rot * Vec3::Z);
            if (cam.eye - r.pos).dot(back) < c.hold_distance {
                let to = cam.eye + back * (c.hold_speed * core::TICK_DT);
                if self.guided_hold_clear(cam.eye, to) {
                    cam.eye = to;
                }
            }
            return Some(cam);
        }
        let section = guided_section(&c.view);
        let frame = r.frame();
        let rot = flyer_view(frame, section.tilt);
        let (_, settled) = self.guided_ideal(r, c, &section, rot);
        let center = r.pos + frame * section.center;
        let from = held.map_or(previous, |h| h.eye);
        let eye = kept_eye(
            within_snap(center, from),
            from,
            settled,
            rot,
            &section.move_tension,
        );
        let goal = section.fov_at(section.speed_fraction(r.speed(), c.top_speed()));
        let was = held.map_or_else(|| chase_fovs().0, |h| h.fov);
        Some(GuidedCam {
            rocket: r.id,
            eye,
            rot,
            fov: ease_fov(goal, was),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIE: [[f32; 2]; 3] = [[2.7, 2.7], [2.7, 10.0], [4.25, 4.25]];
    const DT: f32 = 1.0 / 30.0;

    #[test]
    fn the_view_rolls_and_pitches_with_the_flight_frame() {
        let frame = Quat::from_rotation_z(1.0) * Quat::from_rotation_x(-0.6);
        let view = flyer_view(frame, 0.0);
        assert!((view * Vec3::NEG_Z).abs_diff_eq(frame * Vec3::Z, 1e-5));
        assert!((view * Vec3::Y).abs_diff_eq(frame * Vec3::Y, 1e-5));
    }

    #[test]
    fn a_drawn_camera_gives_back_the_turn_it_was_made_from() {
        for (x, y, z) in [(0.0, 0.0, 0.0), (0.5, 2.0, -1.0), (-1.2, -2.5, 2.9)] {
            let rot = Quat::from_euler(glam::EulerRot::YXZ, y, x, z);
            let cam = camera_at(Vec3::ZERO, rot, 1.0);
            let back = camera::camera_rotation(cam.yaw, cam.pitch, cam.roll);
            assert!(back.dot(rot).abs() > 0.9999, "{x} {y} {z}");
        }
    }

    #[test]
    fn a_gunner_section_keeps_its_own_tilt_and_the_ps2_default_tensions() {
        let mut seat = SeatSpec::new(core::vehicle::SeatKind::Gunner(0));
        seat.track_offset = Some([0.0, 0.0, 20.0]);
        seat.tilt_value = Some(-10.0);
        let s = FlyerSection::gunner(Some(&seat));
        assert!((s.tilt + 10.0 * DEGREES).abs() < 1e-6);
        assert_eq!(s.aim_tension, Some(1000.0));
        assert_eq!(s.move_tension, [[30.0; 2]; 3]);
        assert_eq!(
            (s.center, s.offset),
            (Vec3::ZERO, Vec3::new(0.0, 0.0, 20.0))
        );
    }

    #[test]
    fn the_tilt_looks_below_the_flight_direction() {
        let view = flyer_view(Quat::IDENTITY, BF2_TILT);
        let forward = view * Vec3::NEG_Z;
        assert!(forward.y < 0.0);
        assert!((forward.dot(Vec3::Z) - BF2_TILT.cos()).abs() < 1e-5);
    }

    #[test]
    fn a_camera_holds_any_view_through_yaw_pitch_and_roll() {
        for (x, y, z) in [
            (0.0, 0.0, 0.0),
            (0.5, 2.0, -1.0),
            (-1.4, -2.5, 2.9),
            (1.57, 0.3, 0.7),
        ] {
            let rot = Quat::from_euler(glam::EulerRot::YXZ, y, x, z);
            let cam = camera_at(Vec3::ONE, rot, 1.0);
            assert!(
                cam.forward().abs_diff_eq(rot * Vec3::NEG_Z, 1e-3),
                "{x} {y} {z}"
            );
            assert!(cam.right().abs_diff_eq(rot * Vec3::X, 1e-3), "{x} {y} {z}");
        }
    }

    #[test]
    fn the_eye_falls_behind_a_ship_by_speed_over_its_tension() {
        let view = flyer_view(Quat::IDENTITY, 0.0);
        let speed = 60.0;
        let mut eye = Vec3::ZERO;
        let mut ideal = Vec3::ZERO;
        for _ in 0..300 {
            ideal += Vec3::Z * speed * DT;
            eye = lag_eye(eye, ideal, view, &TIE, DT);
        }
        let a = 1.0 - (-4.25f32 * DT).exp();
        let want = speed * DT * (1.0 - a) / a;
        assert!(((ideal - eye).z - want).abs() < 1e-2, "{}", (ideal - eye).z);
    }

    #[test]
    fn each_direction_of_an_axis_has_its_own_tension() {
        let up = lag_eye(Vec3::ZERO, Vec3::Y, Quat::IDENTITY, &TIE, DT);
        let down = lag_eye(Vec3::ZERO, Vec3::NEG_Y, Quat::IDENTITY, &TIE, DT);
        assert!((up.y - (1.0 - (-2.7f32 * DT).exp())).abs() < 1e-5);
        assert!((down.y + (1.0 - (-10.0f32 * DT).exp())).abs() < 1e-5);
    }

    #[test]
    fn a_bf2_section_keeps_its_fixed_tilt_and_no_aim_lag() {
        let mut seat = SeatSpec::new(core::vehicle::SeatKind::Pilot);
        seat.track_center = Some([0.0, 5.0, -8.0]);
        seat.tilt_value = Some(20.0);
        seat.aim_tension = Some(20.0);
        seat.move_tension = [Some([2.7, 2.7]), Some([2.7, 10.0]), Some([4.25, 4.25])];
        let bf2 = FlyerSection::of(Some(&seat), GroundRules::Bf2);
        assert_eq!(bf2.tilt, BF2_TILT);
        assert_eq!(bf2.aim_tension, None);
        assert_eq!(bf2.center, Vec3::new(0.0, 5.0, -8.0));
        assert_eq!(bf2.move_tension, TIE);
        let bf1 = FlyerSection::of(Some(&seat), GroundRules::Bf1);
        assert!((bf1.tilt - 20.0 * DEGREES).abs() < 1e-6);
        assert_eq!(bf1.aim_tension, Some(20.0));
    }

    fn interceptor_section() -> FlyerSection {
        let mut seat = SeatSpec::new(core::vehicle::SeatKind::Pilot);
        seat.track_center = Some([0.0, 3.5, -4.0]);
        seat.third_person_fov = Some(38.0);
        seat.fast_fov = Some(90.0);
        seat.slow_offset = Some([0.0, 2.0, 10.0]);
        seat.fast_offset = Some([0.0, 5.0, -18.0]);
        seat.move_tension = [Some([3.1, 3.1]), Some([2.9, 15.0]), Some([4.25, 4.25])];
        FlyerSection::of(Some(&seat), GroundRules::Bf2)
    }

    #[test]
    fn the_offset_and_the_view_follow_the_speed_as_the_save_state_has_them() {
        let section = interceptor_section();
        let fraction = section.speed_fraction(80.0, 170.0);
        let offset = section.offset_at(fraction);
        assert!(
            offset.abs_diff_eq(Vec3::new(0.0, 58.0 / 17.0, -54.0 / 17.0), 1e-3),
            "{offset}"
        );
        let half = (section.fov_at(fraction) * 0.5).to_radians().tan();
        assert!((half - 0.6065).abs() < 0.001, "{half}");
        assert_eq!(
            section.offset_at(section.speed_fraction(0.0, 170.0)),
            Vec3::new(0.0, 2.0, 10.0)
        );
        assert_eq!(section.fov_at(section.speed_fraction(-5.0, 170.0)), 38.0);
        assert_eq!(section.fov_at(section.speed_fraction(400.0, 170.0)), 90.0);
    }

    #[test]
    fn a_class_without_the_speed_keys_keeps_its_section() {
        let mut seat = SeatSpec::new(core::vehicle::SeatKind::Pilot);
        seat.track_offset = Some([0.0, 1.0, 6.0]);
        seat.third_person_fov = Some(55.0);
        let section = FlyerSection::of(Some(&seat), GroundRules::Bf2);
        let fraction = section.speed_fraction(80.0, 170.0);
        assert_eq!(fraction, 0.0);
        assert_eq!(section.offset_at(fraction), Vec3::new(0.0, 1.0, 6.0));
        assert_eq!(section.fov_at(fraction), 55.0);
    }

    #[test]
    fn the_ship_sits_in_the_view_where_the_save_state_has_it() {
        let section = interceptor_section();
        let fraction = section.speed_fraction(80.0, 170.0);
        let offset = section.offset_at(fraction);
        let view = flyer_view(Quat::IDENTITY, section.tilt);
        let mut pos = Vec3::ZERO;
        let mut eye = section.center + view * offset;
        for _ in 0..600 {
            pos += Vec3::Z * 80.0 * DT;
            let ideal = pos + section.center + view * offset;
            eye = lag_eye(eye, ideal, view, &section.move_tension, DT);
        }
        let seen = view.inverse() * (pos - eye);
        assert!((seen.y + 4.5).abs() < 0.5, "{seen}");
        assert!((-seen.z - 18.5).abs() < 0.5, "{seen}");
    }

    #[test]
    fn a_section_with_no_center_uses_its_eye_point() {
        let mut seat = SeatSpec::new(core::vehicle::SeatKind::Pilot);
        seat.eye_point_offset = Some([1.0, 2.0, 3.0]);
        let section = FlyerSection::of(Some(&seat), GroundRules::Bf2);
        assert_eq!(section.center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(section.offset, Vec3::ZERO);
        assert_eq!(section.move_tension, [DEFAULT_MOVE_TENSION; 3]);
    }

    fn tie_pilot() -> (WalkSim, VehicleHandle) {
        use core::vehicle::{SeatKind, VehicleSpec};
        let mut w = WalkSim::new(1, Vec3::ZERO, None);
        let h = w.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let mut seat = SeatSpec::new(SeatKind::Pilot);
        seat.track_center = Some([0.0, 5.0, -8.0]);
        seat.track_offset = Some([0.0, 0.0, 0.0]);
        seat.move_tension = [Some([2.7, 2.7]), Some([2.7, 10.0]), Some([4.25, 4.25])];
        w.vehicle_seats = vec![vec![seat]];
        w.sim.ground_rules = GroundRules::Bf2;
        let player = w.player;
        assert!(w.sim.seat_unit(player, h, 0));
        (w, h)
    }

    #[test]
    fn a_pilots_camera_turns_with_the_ship_in_every_axis() {
        let (mut w, h) = tie_pilot();
        let frame = Quat::from_rotation_z(0.9) * Quat::from_rotation_x(-0.5);
        w.sim.vehicle_mut(h).unwrap().rot = frame;
        let cam = w.camera();
        let view = flyer_view(frame, BF2_TILT);
        assert!(cam.forward().abs_diff_eq(view * Vec3::NEG_Z, 1e-3));
        assert!(cam.right().abs_diff_eq(view * Vec3::X, 1e-3));
        let ideal = frame * Vec3::new(0.0, 5.0, -8.0);
        assert!(cam.position.abs_diff_eq(ideal, 1e-3), "{}", cam.position);
    }

    #[test]
    fn a_pilots_camera_ignores_the_ships_drawn_bank() {
        let (mut w, h) = tie_pilot();
        let frame = Quat::from_rotation_y(0.4);
        {
            let v = w.sim.vehicle_mut(h).unwrap();
            v.flyer.tilt_pitch = 0.2;
            v.flyer.tilt_roll = -0.3;
            v.rot = frame * core::vehicle::hover::quat_z(-0.3) * core::vehicle::hover::quat_x(0.2);
        }
        let cam = w.camera();
        let view = flyer_view(frame, BF2_TILT);
        assert!(cam.forward().abs_diff_eq(view * Vec3::NEG_Z, 1e-3));
        assert!(cam.right().abs_diff_eq(view * Vec3::X, 1e-3));
    }

    #[test]
    fn a_held_eye_trails_the_ship_and_a_far_one_snaps() {
        let (mut w, h) = tie_pilot();
        w.cam_flyer = w.next_cam_flyer(w.camera().position);
        let start = w.cam_flyer.unwrap().eye;
        w.sim.vehicle_mut(h).unwrap().pos = Vec3::Z * 2.0;
        let moved = w.next_cam_flyer(w.camera().position).unwrap().eye;
        let a = 1.0 - (-4.25f32 * core::TICK_DT).exp();
        assert!((moved.z - (start.z + 2.0 * a)).abs() < 1e-3, "{moved}");
        w.cam_flyer = Some(FlyerCam {
            eye: moved,
            ..w.cam_flyer.unwrap()
        });
        assert!((w.camera().position - moved).length() < 1e-5);
        w.sim.vehicle_mut(h).unwrap().pos = Vec3::Z * 300.0;
        let far = w.next_cam_flyer(w.camera().position).unwrap().eye;
        assert!(far.abs_diff_eq(Vec3::new(0.0, 5.0, 292.0), 1e-3), "{far}");
    }

    #[test]
    fn leaving_the_pilot_seat_ends_the_held_camera() {
        let (mut w, _) = tie_pilot();
        w.cam_flyer = w.next_cam_flyer(w.camera().position);
        assert!(w.cam_flyer.is_some());
        let player = w.player;
        w.sim.exit_vehicle(player);
        assert!(w.next_cam_flyer(w.camera().position).is_none());
    }

    #[test]
    fn boarding_eases_the_eye_from_the_previous_camera() {
        let (w, _) = tie_pilot();
        assert!(w.cam_flyer.is_none());
        let settled = w.next_cam_flyer(w.camera().position).unwrap().eye;
        let previous = settled + Vec3::X * 10.0;
        let eye = w.next_cam_flyer(previous).unwrap().eye;
        let kept = 10.0 * (-2.7f32 * core::TICK_DT).exp();
        assert!(((eye - settled).x - kept).abs() < 1e-3, "{eye} {settled}");
        let far = settled + Vec3::X * 400.0;
        assert!(w
            .next_cam_flyer(far)
            .unwrap()
            .eye
            .abs_diff_eq(settled, 1e-3));
    }
}
