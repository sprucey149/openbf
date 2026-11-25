//! Placement and pointer hit testing for the game's flat frames in OpenXR.

use super::actions::Pose;
use extensions::runtime::MenuCursorFrame;
use glam::{Quat, Vec2, Vec3};
use openxr as xr;

const MENU_DISTANCE: f32 = 1.45;
const MENU_WIDTH: f32 = 1.65;
// OURS: convert a stick held off center to wheel lines per second.
// measure: compare reading speed and stick drift on Touch and Index.
const SCROLL_SPEED: f32 = 8.0;
const SCROLL_DEAD_ZONE: f32 = 0.2;
// OURS: the menu glides around to the view's heading at eye height, reaching
// 63% of a head turn after this time. It ignores pitch, unlike the visor HUD.
// measure: reading and pointing comfort on Quest 3 and Index.
const FOLLOW_SECONDS: f32 = 0.15;

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub position: Vec3,
    pub orientation: Quat,
    width: f32,
}

impl Placement {
    pub fn menu(head: Pose) -> Self {
        Self::at(head.position, facing(head.orientation))
    }

    pub fn follow(self, head: Pose, seconds: f32) -> Self {
        let t = 1.0 - (-seconds.max(0.0) / FOLLOW_SECONDS).exp();
        let eye = self.position + self.orientation * Vec3::Z * MENU_DISTANCE;
        let orientation = self
            .orientation
            .slerp(facing(head.orientation), t)
            .normalize();
        Self::at(eye.lerp(head.position, t), orientation)
    }

    pub fn is_finite(self) -> bool {
        self.position.is_finite() && self.orientation.is_finite()
    }

    fn at(eye: Vec3, orientation: Quat) -> Self {
        Self {
            position: eye - orientation * Vec3::Z * MENU_DISTANCE,
            orientation,
            width: MENU_WIDTH,
        }
    }

    pub fn pose(self) -> xr::Posef {
        let [x, y, z, w] = self.orientation.to_array();
        xr::Posef {
            position: xr::Vector3f {
                x: self.position.x,
                y: self.position.y,
                z: self.position.z,
            },
            orientation: xr::Quaternionf { x, y, z, w },
        }
    }

    pub fn extent(self, pixels: [u32; 2]) -> xr::Extent2Df {
        xr::Extent2Df {
            width: self.width,
            height: self.width * pixels[1] as f32 / pixels[0].max(1) as f32,
        }
    }

    pub fn hit(self, origin: Vec3, direction: Vec3, pixels: [u32; 2]) -> Option<[f32; 2]> {
        let inverse = self.orientation.conjugate();
        let origin = inverse * (origin - self.position);
        let direction = inverse * direction;
        if origin.z <= 0.0 || direction.z >= -1e-5 || pixels.contains(&0) {
            return None;
        }
        let hit = origin - direction * (origin.z / direction.z);
        let extent = self.extent(pixels);
        let uv = [0.5 + hit.x / extent.width, 0.5 - hit.y / extent.height];
        uv.iter().all(|v| (0.0..1.0).contains(v)).then_some(uv)
    }
}

fn facing(rotation: Quat) -> Quat {
    Quat::from_rotation_y(super::rig::heading(rotation))
}

#[derive(Default)]
pub(super) struct MenuCursor {
    armed: bool,
    down: bool,
    last_hit: Option<i64>,
}

impl MenuCursor {
    pub fn sample(
        &mut self,
        position: Option<[f32; 2]>,
        trigger: bool,
        sticks: [Vec2; 2],
        now: i64,
    ) -> MenuCursorFrame {
        let hit = position.is_some();
        let down = self.armed && hit && trigger;
        let click = down && !self.down;
        // A press needs a prior release on the panel. Entering a menu or
        // moving onto the panel with the trigger held cannot click.
        self.armed = hit && (!trigger || down);
        self.down = down;
        let [left, right] = sticks.map(|v| {
            if v.is_finite() && v.y.abs() > v.x.abs() {
                v.y.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        });
        let axis = if right.abs() >= left.abs() {
            right
        } else {
            left
        };
        let strength = ((axis.abs() - SCROLL_DEAD_ZONE) / (1.0 - SCROLL_DEAD_ZONE)).max(0.0);
        let elapsed = self.last_hit.filter(|_| hit).map_or(0.0, |last| {
            now.saturating_sub(last).clamp(0, 66_666_667) as f32 * 1e-9
        });
        self.last_hit = hit.then_some(now);
        MenuCursorFrame {
            position,
            down,
            click,
            wheel: axis.signum() * strength * SCROLL_SPEED * elapsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_match_panel_pixels_at_a_turn_and_resizing_changes_height() {
        let head = Pose {
            position: Vec3::new(0.1, 1.7, 0.3),
            orientation: Quat::from_rotation_y(1.2),
        };
        let screen = Placement::menu(head);
        let size = [1600, 900];
        let hit = screen.position
            + screen.orientation
                * Vec3::new(
                    0.25 * screen.extent(size).width,
                    0.25 * screen.extent(size).height,
                    0.0,
                );
        let origin = head.position - Vec3::Y * 0.4;
        let uv = screen.hit(origin, hit - origin, size).unwrap();
        assert!((uv[0] - 0.75).abs() < 1e-5);
        assert!((uv[1] - 0.25).abs() < 1e-5);
        assert!(screen.hit(origin, origin - hit, size).is_none());
        assert!(screen
            .hit(
                screen.position - screen.orientation * Vec3::Z,
                screen.orientation * Vec3::Z,
                size
            )
            .is_none());
        let tall = screen.hit(origin, hit - origin, [900, 900]).unwrap();
        assert!(tall[1] > uv[1]);
    }

    #[test]
    fn menu_glides_to_the_view_heading_and_ignores_pitch() {
        let behind = Placement::menu(Pose {
            position: Vec3::new(0.0, 0.4, 0.0),
            orientation: Quat::from_rotation_y(std::f32::consts::PI),
        });
        let head = Pose {
            position: Vec3::new(0.2, 1.6, -0.1),
            orientation: Quat::from_rotation_y(0.5) * Quat::from_rotation_x(-0.3),
        };
        let target = Placement::menu(head);
        let forward = Quat::from_rotation_y(0.5) * Vec3::NEG_Z;
        let ahead = (target.position - head.position).normalize();
        assert!(ahead.dot(forward) > 0.9999);
        assert!((target.position.y - head.position.y).abs() < 1e-5);
        assert!(((target.position - head.position).length() - MENU_DISTANCE).abs() < 1e-4);

        let step = behind.follow(head, 0.011);
        let turned = |p: Placement| p.orientation.angle_between(target.orientation);
        assert!(turned(step) < turned(behind));
        assert!(turned(step) > 0.1);

        let mut placement = behind;
        for _ in 0..200 {
            placement = placement.follow(head, 0.011);
        }
        assert!(turned(placement) < 1e-3);
        assert!((placement.position - target.position).length() < 1e-3);

        let still = target.follow(head, 0.011);
        assert!((still.position - target.position).length() < 1e-5);
    }

    #[test]
    fn entering_menu_or_reacquiring_panel_with_held_trigger_cannot_click() {
        let point = Some([600.0, 400.0]);
        let mut cursor = MenuCursor::default();
        let sample = |c: &mut MenuCursor, p, t| c.sample(p, t, [Vec2::ZERO; 2], 0);
        assert!(!sample(&mut cursor, point, true).click);
        assert!(!sample(&mut cursor, point, false).down);
        assert!(sample(&mut cursor, point, true).click);
        assert!(!sample(&mut cursor, point, true).click);
        assert!(!sample(&mut cursor, None, true).down);
        assert!(!sample(&mut cursor, point, true).click);
        sample(&mut cursor, point, false);
        assert!(sample(&mut cursor, point, true).click);
    }

    #[test]
    fn scrolling_uses_either_stick_and_only_accumulates_while_pointing_at_the_panel() {
        let point = Some([600.0, 400.0]);
        let mut cursor = MenuCursor::default();
        let left = [Vec2::Y, Vec2::ZERO];
        let right = [Vec2::ZERO, Vec2::NEG_Y];
        assert_eq!(cursor.sample(point, false, left, 0).wheel, 0.0);
        assert!((cursor.sample(point, false, left, 20_000_000).wheel - 0.16).abs() < 1e-5);
        assert!((cursor.sample(point, false, right, 30_000_000).wheel + 0.08).abs() < 1e-5);
        assert_eq!(cursor.sample(None, false, right, 40_000_000).wheel, 0.0);
        assert_eq!(cursor.sample(point, false, right, 2_000_000_000).wheel, 0.0);
        let drift = [Vec2::Y * 0.1, Vec2::new(1.0, 0.3)];
        assert_eq!(cursor.sample(point, false, drift, 2_010_000_000).wheel, 0.0);
    }
}
