//! Pure camera and FOV math for a walk: the PS2 FOVs, the chase/vehicle/droid
//! cameras and the team tints. No WalkSim state, no wall clock, no sim input.
//!
//! Owned here: the FOV constants/helpers, vehicle_camera, droid_camera,
//! look_camera, turret_scope_camera, vehicle_pull_height and the constants
//! the camera children read. The camera methods that mutate a walk stay in
//! camera.rs, flyer_camera.rs, death_camera.rs and walker_view.rs.

use super::CAMERA_PULL_Y;
use core::soldier::Stance;
use glam::Vec3;
use render::Camera;

/// Team tints (placeholder until faction colors come from data): team 0,
/// team 1, neutral.
pub const TEAM_COLORS: [[f32; 3]; 3] = [[0.95, 0.35, 0.15], [0.3, 0.9, 0.3], [0.9, 0.9, 0.9]];

/// How far the chase camera keeps from the terrain sheet, either side.
pub(crate) const CAMERA_TERRAIN_GAP: f32 = 0.3;

pub fn team_color(team: Option<u8>) -> [f32; 3] {
    match team {
        Some(0) => TEAM_COLORS[0],
        Some(1) => TEAM_COLORS[1],
        _ => TEAM_COLORS[2],
    }
}

/// The PS2 soldier camera's tan(fov_y / 2), third person, standing, zoom 1,
/// read from PS2 RAM on every `pcsx2/ref/states/` state (FINDINGS F5): BF1
/// cam+0x8c, BF2 cam+0x13c (offsets named by `analysis/re-sky-pass.md` §2/§4)
/// — 37.69° / 52.06° vertical. They are tan(FOV / 2) x 0.7666 of the
/// class's third-person FOV, 48° and 65° (`analysis/re-zoom.md` §5; card
/// CFOVM: vehicles may differ).
pub const PS2_TAN_HALF_FOV_Y_BF1: f32 = 0.34131;
pub const PS2_TAN_HALF_FOV_Y_BF2: f32 = 0.48838;

/// A soldier class's `ThirdPersonFOV` and `FirstPersonFOV` in degrees
/// (`re-zoom.md` §5): the zoomed view takes the second. No BF1 soldier
/// authors either (48° both); BF2's soldiers 65° / 70°.
pub fn ps2_soldier_fovs(rules: core::soldier::GroundRules) -> (f32, f32) {
    match rules {
        core::soldier::GroundRules::Bf1 => (48.0, 48.0),
        core::soldier::GroundRules::Bf2 => (65.0, 70.0),
    }
}

/// The chase camera's (third-person, zoomed) FOVs in degrees: BF2's on every
/// map - OURS, the one default camera of [`core::unit::camera_section`]
/// (playtest 2026-10-28). trace: BF1's own chase FOVs, if the one-camera
/// rule is revisited.
pub fn chase_fovs() -> (f32, f32) {
    ps2_soldier_fovs(core::soldier::GroundRules::Bf2)
}

/// The chase camera's vertical FOV (radians) on every map: BF2's measured
/// 52.06 deg - OURS, as [`chase_fovs`]. [`ps2_fov_y`] keeps each game's.
/// trace: as [`chase_fovs`].
pub fn chase_fov_y() -> f32 {
    ps2_fov_y("bf2")
}

/// The PS2 camera eases its projection scale S and its FOV toward their
/// targets by exp(-25 dt) per update (`re-zoom.md` §5).
pub const ZOOM_EASE_RATE: f32 = 25.0;

/// The PS2's vertical tangent scale (`0.7666` = `tan(FOV / 2)` multiplier, `re-zoom.md` §5).
pub const PS2_FOV_TAN_SCALE: f32 = 0.7666;

pub(crate) const CAMERA_MOVE_TENSION: f32 = 15.0;
pub(crate) const VIEW_SETTLE_EPSILON: f32 = 1e-4;
/// A camera target further than this from its held eye snaps instead of
/// easing (250 m; the flyer cameras and the exit camera).
pub(crate) const CAMERA_SNAP_SQUARED: f32 = 250.0 * 250.0;

pub(crate) fn ease_camera_place(from: (f32, Vec3), to: (f32, Vec3), dt: f32) -> (f32, Vec3) {
    let k = core::fmath::exp(-CAMERA_MOVE_TENSION * dt);
    let y = to.0 + (from.0 - to.0) * k;
    let offset = to.1 + (from.1 - to.1) * k;
    if (y - to.0).abs() < VIEW_SETTLE_EPSILON
        && (offset - to.1).abs().max_element() < VIEW_SETTLE_EPSILON
    {
        to
    } else {
        (y, offset)
    }
}

/// The chase camera's vertical FOV (radians) for a manifest's `source_game`.
/// The port keeps the PS2's VERTICAL framing and widens the horizontal with
/// the window (Hor+, `render::Camera::fov_y`) — never the 4:3 horizontal.
pub fn ps2_fov_y(source_game: &str) -> f32 {
    let t = if source_game.eq_ignore_ascii_case("bf1") {
        PS2_TAN_HALF_FOV_Y_BF1
    } else {
        PS2_TAN_HALF_FOV_Y_BF2
    };
    2.0 * core::fmath::atan2(t, 1.0)
}

/// Where the PS2 draws the reticle with the standing camera at its ideal
/// place, as a fraction of the height from the top (`ui::hud::reticle_y`):
/// BF1 line 178.5 of 480, BF2 153.35 (`re-aim-assist.md` §10.4 [V-RAM]).
pub fn ps2_reticle_y(source_game: &str) -> f32 {
    let rules = core::soldier::GroundRules::for_game(source_game);
    let t = match rules {
        core::soldier::GroundRules::Bf1 => PS2_TAN_HALF_FOV_Y_BF1,
        core::soldier::GroundRules::Bf2 => PS2_TAN_HALF_FOV_Y_BF2,
    };
    ui::hud::reticle_y(
        core::unit::ps2_camera_section(rules, Stance::Stand, false).tilt,
        t,
    )
}

/// How high over a vehicle's origin, in its own frame, the pilot's camera is pulled
/// in toward, from the top of its model: the PS2 replaces the camera section's fixed
/// point when it reads a class's `GeometryName`, with 0.9 of the vis box's top on a
/// hover and a flyer and 0.8 on a walker (hover B1 `FUN_001f0648` B1:101919, B2
/// `FUN_002223a0` B2:137376; flyer B1 `FUN_001df3f0` B1:89841, B2 `FUN_0020fd58`
/// B2:125118; walker B1 `FUN_0021d980` B1:134004, B2 `FUN_00263f68` B2:183946). A
/// building turret and a class with no model keep the fixed point.
pub fn vehicle_pull_height(kind: core::vehicle::VehicleKind, top: Option<f32>) -> f32 {
    use core::vehicle::VehicleKind::{Flyer, Hover, Turret, Walker};
    match (kind, top) {
        (Hover | Flyer, Some(top)) => top * 0.9,
        (Walker, Some(top)) => top * 0.8,
        (Turret, _) | (_, None) => CAMERA_PULL_Y,
    }
}

/// A seat's third-person camera (card VSEAT) as `(pull point, camera)`.
/// PROVISIONAL (trace: the vehicle camera frame of `FUN_001b58f0`) — the PS2's camera
/// frame for a vehicle is not ported, only its pull
/// point: `origin` on the vehicle, as the PS2's camera update takes its collision
/// origin from the section's point on the frame (`FUN_001b58f0`, `FUN_001cef68`),
/// which [`WalkSim::seat_pull_origin`] gives. The rest reads the seat's props the way
/// [`WalkSim::camera`] reads the soldier's CAMERASECTION: the track centre is
/// `TrackCenter` in the vehicle's HEADING
/// frame (x right, y up, z forward; default 0 2 0), the camera
/// `TrackOffset.z` behind it along the
/// view, `.y` above and `.x` right (default 0 1 8), pitched down by
/// `TiltValue` degrees. The heading ignores the hover's pitch and roll so
/// the view does not rock with the springs. `ThirdPersonFOV` is not applied
/// (its unit is untraced, card CFOVM): the chase FOV `fov_y` is. A seat
/// with a gun (card VTUR) passes its `look` (yaw, pitch in the model frame,
/// `core::vehicle::gun`): the heading frame turns by the yaw and the view
/// pitches by the pitch, so the camera looks where the seat's aim ray goes.
pub fn vehicle_camera(
    v: &core::vehicle::Vehicle,
    seat: Option<&crate::vehicle_class::SeatSpec>,
    look: Option<(f32, f32)>,
    fov_y: f32,
    origin: Vec3,
) -> (Vec3, Camera) {
    let (look_yaw, look_pitch) = look.unwrap_or((0.0, 0.0));
    let f = v.rot * core::vehicle::gun::dir(look_yaw, 0.0);
    let fwd = Vec3::new(f.x, 0.0, f.z).try_normalize().unwrap_or(Vec3::Z);
    let right = Vec3::new(fwd.z, 0.0, -fwd.x);
    let c = seat.and_then(|s| s.track_center).unwrap_or([0.0, 2.0, 0.0]);
    let o = seat.and_then(|s| s.track_offset).unwrap_or([0.0, 1.0, 8.0]);
    let tilt = seat.and_then(|s| s.tilt_value).unwrap_or(0.0);
    let cam = Camera {
        yaw: core::fmath::atan2(-fwd.x, -fwd.z),
        pitch: look_pitch - tilt.to_radians(),
        fov_y,
        ..Camera::default()
    };
    let center = v.pos + right * c[0] + Vec3::Y * c[1] + fwd * c[2];
    let position = center - cam.forward() * o[2] + Vec3::Y * o[1] + cam.right() * o[0];
    (v.pos + v.rot * origin, Camera { position, ..cam })
}

/// `[ours]` The remote droid's chase offset (maintainer, 2026-10-28: "3rd
/// person ... like the rest of the game"): the camera sits this far from
/// the track centre in the tilted view frame (x right, y up, z back), as the
/// soldier's CAMERASECTION offset does: straight back at the eye point's
/// height, as the soldier's stand section (0, 0, 3.5 BF1 / 3.2 BF2) is.
/// Picked so the 0.5 m probe droid sits in the lower middle of the frame,
/// under the reticle and clear of it, about where the soldier stands. The
/// disc's own `TrackOffset` (0, -0.25, 1.5 BF1 / 1.8 BF2,
/// `analysis/re-remote-droid.md` §3; how the PS2 applies it is untraced)
/// is read (`DroidClass::track_offset`) and not used.
pub const DROID_CHASE: Vec3 = Vec3::new(0.0, 0.0, 2.2);

/// A flown remote droid's chase camera as `(target, camera)`, before the
/// world pulls it in: the target is the disc's `EyePointOffset` above the
/// droid (0.5 m), the view the droid's aim pitched down by its `TiltValue`
/// (5°), the camera [`DROID_CHASE`] from the target in that tilted frame,
/// so it swings round the droid as it pitches, like the soldier's
/// (`core::unit::Unit::ideal_camera`).
pub fn droid_camera(
    d: &core::remote::Droid,
    c: &core::remote::DroidClass,
    fov_y: f32,
) -> (Vec3, Camera) {
    let cam = Camera {
        yaw: d.yaw,
        pitch: d.pitch - c.tilt,
        fov_y,
        ..Camera::default()
    };
    let target = d.pos + Vec3::Y * c.eye_offset.y;
    let (fwd, right) = (cam.forward(), cam.right());
    let up = right.cross(fwd);
    let o = DROID_CHASE;
    let position = target + right * o.x + up * o.y - fwd * o.z;
    (target, Camera { position, ..cam })
}

/// Construct a camera at `position` looking along `forward` with zero tilt.
pub fn look_camera(position: Vec3, forward: Vec3, fov_y: f32) -> Camera {
    let flat = forward.with_y(0.0).length();
    Camera {
        position,
        yaw: core::fmath::atan2(-forward.x, -forward.z),
        pitch: core::fmath::atan2(forward.y, flat),
        fov_y,
        ..Camera::default()
    }
}

/// A zoomed building turret's view (card VTURZ, `analysis/re-turret-draw.md`
/// 4 [O]; B1 `FUN_00225988`, B2 `FUN_0026fdf8`): at the eye point
/// (`EyePointOffset` on the aim frame, `SeatLook::scope_eye`), looking along
/// the look, no pull-in and no floor (the PS2's look-along frame has
/// neither).
pub fn turret_scope_camera(look: &core::sim::SeatLook, fov_y: f32) -> Camera {
    look_camera(look.scope_eye, look.view, fov_y)
}

/// The FOV option's range (profile `Settings::fov_scale`, the pause menu):
/// a multiplier on the PS2 angle, 1.0 = the PS2's framing. The result is
/// held under [`MAX_FOV_Y`] so a BF2 map at the top of the range stays sane.
pub use ui::menu::FOV_SCALE_RANGE;
pub const MAX_FOV_Y: f32 = ui::menu::MAX_FOV_DEG * std::f32::consts::PI / 180.0;

pub fn clamp_fov_scale(scale: f32) -> f32 {
    ui::menu::clamp_fov_scale(chase_fov_y().to_degrees(), scale)
}
