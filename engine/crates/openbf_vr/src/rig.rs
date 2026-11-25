//! Transform tracked poses into game space and write replayable infantry input.

use super::actions::{Hand, Input, Pose};
use core::sim::{buttons, PadState};
use core::unit::{Unit, UnitHandle};
use extensions::runtime::{SimView, TrackedGun};
use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};
use render::Camera;

const SNAP: f32 = std::f32::consts::FRAC_PI_6;
const DEAD_ZONE: f32 = 0.18;
const AIM_BUTTONS: u32 = buttons::FIRE | buttons::SECONDARY_FIRE;
/// The controller buttons a seat or remote takes as they are. RELOAD is left
/// out: there the right stick aims.
const SEAT_BUTTONS: u32 = AIM_BUTTONS | buttons::JUMP | buttons::ACTION | buttons::SWITCH_WEAPON;
/// OURS: keep the weapon origin 7 cm forward and up from the tracked palm
/// in aim space. measure: line up the authored hand with the controller in a headset.
const HAND_MOUNT_OFFSET: Vec3 = Vec3::new(0.0, 0.07, -0.07);
// The same offset expressed from the weapon mount to the palm in model space.
pub(super) const REAR_GRIP: Vec3 = Vec3::new(0.0, -HAND_MOUNT_OFFSET.y, HAND_MOUNT_OFFSET.z);
/// OURS: metre tolerance to grab the foregrip.
/// measure: check pickup reach on the rifle, sniper and launcher in a headset.
const GRAB_RADIUS: f32 = 0.18;

#[derive(Clone, Copy)]
struct Support {
    unit: UnitHandle,
    gun: TrackedGun,
}

#[derive(Default)]
pub(super) struct Rig {
    tracking_origin: Option<Pose>,
    foot: Option<UnitHandle>,
    yaw: f32,
    snap_held: bool,
    held: u32,
    pending: u32,
    blocked: u32,
    active: bool,
    right_tracked: bool,
    support: Option<Support>,
    boost: bool,
    boost_down: bool,
    boost_moved: bool,
    pub motion_epoch: u16,
}

#[derive(Clone, Copy)]
pub(super) struct World {
    rotation: Quat,
    translation: Vec3,
}

impl World {
    pub fn vector(self, v: Vec3) -> Vec3 {
        self.rotation * v
    }

    pub fn pose(self, pose: Pose) -> Pose {
        Pose {
            position: self.translation + self.rotation * pose.position,
            orientation: self.rotation * pose.orientation,
        }
    }

    pub fn camera(self, pose: Pose, base: Camera) -> Camera {
        let pose = self.pose(pose);
        let (yaw, pitch, roll) = pose.orientation.to_euler(EulerRot::YXZ);
        Camera {
            position: pose.position,
            yaw,
            pitch,
            roll: -roll,
            near: 0.05,
            ..base
        }
    }

    pub fn hand_mounts(self, [left, right]: [Hand; 2]) -> [Option<Mat4>; 2] {
        [self.hand_mount(left, false), self.hand_mount(right, true)]
    }

    pub fn weapon_hand_mounts(self, hands: [Hand; 2], foregrip: Option<Vec3>) -> [Option<Mat4>; 2] {
        let mut mounts = self.hand_mounts(hands);
        let (Some(foregrip), Some(left), Some(right)) = (foregrip, hands[0].grip, hands[1].grip)
        else {
            return mounts;
        };
        let left = self.pose(left).position;
        let right = self.pose(right).position;
        let (Some(rear_mount), Some(front_mount), Some(direction)) =
            (mounts[1], mounts[0], (left - right).try_normalize())
        else {
            return mounts;
        };
        let model_span = rear_mount
            .transform_vector3(foregrip - REAR_GRIP)
            .normalize();
        let axis = model_span.cross(direction);
        let w = 1.0 + model_span.dot(direction);
        let turn = if w <= f32::EPSILON {
            let axis = model_span.any_orthonormal_vector();
            Quat::from_xyzw(axis.x, axis.y, axis.z, 0.0)
        } else {
            Quat::from_xyzw(axis.x, axis.y, axis.z, w).normalize()
        };
        let (_, rotation, _) = rear_mount.to_scale_rotation_translation();
        let rotation = turn * rotation;
        let weapon = Mat4::from_rotation_translation(rotation, right - rotation * REAR_GRIP);
        let left_palm = weapon.transform_point3(foregrip);
        mounts[0] = Some(Mat4::from_translation(left_palm - left) * front_mount);
        mounts[1] = Some(weapon);
        mounts
    }

    fn hand_mount(self, hand: Hand, aiming: bool) -> Option<Mat4> {
        let grip = self.pose(hand.grip.or(hand.aim)?);
        // OURS: rotate the grip frame's -Z (along the fingers, toward the
        // thumb) to +Y for an aim-like frame. measure: check free-hand roll
        // on each controller profile against the real grip.
        let grip_to_aim = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let orientation = match (aiming, hand.grip, hand.aim) {
            (true, _, Some(aim)) | (_, None, Some(aim)) => self.pose(aim).orientation,
            _ => grip.orientation * grip_to_aim,
        };
        // Authored weapon mounts face +Z, OpenXR aim faces -Z.
        let model_rotation = orientation * Quat::from_rotation_y(std::f32::consts::PI);
        Some(Mat4::from_rotation_translation(
            model_rotation,
            grip.position + orientation * HAND_MOUNT_OFFSET,
        ))
    }
}

pub(super) fn infantry<'a>(view: &'a SimView<'a>) -> Option<&'a Unit> {
    view.sim.unit(view.player).filter(|u| {
        u.alive
            && u.mount.is_none()
            && u.droideka.is_none()
            && view
                .sim
                .spawn_stage
                .get(usize::from(view.local_slot))
                != Some(&core::sim::SpawnStage::Alive)
            && view.sim.controlled_droid(view.player).is_none()
            && view.sim.controlled_guided(view.player).is_none()
            && view.sim.outcome.is_none()
    })
}

/// The player drives something instead of walking: a vehicle or turret
/// seat, a remote droid, a guided missile, or a droideka's own legs and ball.
pub(super) fn piloted(view: &SimView<'_>) -> bool {
    view.sim.unit(view.player).is_some_and(|u| {
        u.alive
            && (u.mount.is_some()
                || u.droideka.is_some()
                || view.sim.controlled_droid(view.player).is_some()
                || view.sim.controlled_guided(view.player).is_some())
    }) && view.sim.spawn_stage.get(usize::from(view.local_slot))
        != Some(&core::sim::SpawnStage::Alive)
        && view.sim.outcome.is_none()
}

/// The player's own body is a droideka, with no remote flown from it.
fn rolls(view: &SimView<'_>) -> bool {
    view.sim
        .unit(view.player)
        .is_some_and(|u| u.droideka.is_some() && u.mount.is_none())
        && view.sim.controlled_droid(view.player).is_none()
        && view.sim.controlled_guided(view.player).is_none()
}

/// The view pitches and rolls with what is flown, a flyer's pilot seat or a
/// guided missile; everywhere else it keeps a level horizon.
fn flies(view: &SimView<'_>) -> bool {
    piloted(view)
        && (view.sim.controlled_guided(view.player).is_some()
            || view.sim.seat_of(view.player).is_some_and(|(v, seat)| {
                seat == core::vehicle::SeatKind::Pilot
                    && view
                        .sim
                        .vehicle(v)
                        .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer)
            }))
}

/// Put the controllers' sticks where a gamepad's would be, ahead of the
/// flight-stick mapping. A centred stick leaves the gamepad's and keys' axes.
pub(super) fn seat_sticks(input: Input, mut pad: PadState, look: f32) -> PadState {
    let drive = input.movement.unwrap_or_default();
    let (lx, ly) = (input::drive_axis(drive.x), input::drive_axis(drive.y));
    if (lx, ly) != (0, 0) {
        (pad.lx, pad.ly) = (lx, ly);
    }
    let aim = input.right_stick;
    let (rx, ry) = (input::look_axis(aim.x, look), input::look_axis(aim.y, look));
    if (rx, ry) != (0, 0) {
        (pad.rx, pad.ry) = (rx, ry);
    }
    pad
}

impl Rig {
    pub fn recenter(&mut self) {
        self.tracking_origin = None;
        self.foot = None;
        self.support = None;
        self.motion_epoch = self.motion_epoch.wrapping_add(1);
    }

    pub fn track(
        &mut self,
        view: Option<&SimView<'_>>,
        head: Option<Pose>,
        input: Input,
        playing: bool,
    ) {
        let foot = view.and_then(|v| infantry(v).map(|u| (v.player, u.soldier.yaw)));
        if let Some(head) = head {
            if self.tracking_origin.is_none() || foot.map(|f| f.0) != self.foot {
                let yaw = foot.map_or_else(|| view.map_or(0.0, |v| v.camera.yaw), |f| f.1);
                self.tracking_origin = Some(head);
                self.foot = foot.map(|f| f.0);
                self.yaw = yaw - heading(head.orientation);
                self.motion_epoch = self.motion_epoch.wrapping_add(1);
            }
        }
        let seated = view.is_some_and(piloted);
        let active = playing && head.is_some() && (foot.is_some() || seated);
        if active != self.active {
            self.motion_epoch = self.motion_epoch.wrapping_add(1);
        }
        let turn = input.turn();
        if input.right_stick.x.abs() < 0.4 {
            self.snap_held = false;
        }
        if turn.abs() > 0.7 && !self.snap_held {
            self.snap_held = true;
            if active && foot.is_some() && !input.pause {
                self.yaw -= turn.signum() * SNAP;
                self.motion_epoch = self.motion_epoch.wrapping_add(1);
            }
        }
        let buttons = input.battle_buttons();
        self.blocked &= buttons;
        // Focus and pose loss can temporarily report all buttons as released.
        // Quarantine any button still held when its input returns.
        let right_tracked = input.hands[1].aim.is_some();
        if !active || !self.active {
            self.blocked |= buttons;
            self.pending = 0;
        } else if foot.is_some() && (!right_tracked || !self.right_tracked) {
            self.blocked |= buttons & AIM_BUTTONS;
            self.pending &= !AIM_BUTTONS;
        }
        let held = if active { buttons & !self.blocked } else { 0 };
        self.pending |= held & !self.held;
        self.held = held;
        self.latch_boost(active && seated, held, input.movement);
        self.active = active;
        self.right_tracked = right_tracked;
    }

    /// L3 holds a seat's boost on until it is clicked again or the drive
    /// stick comes back to the middle after a push.
    fn latch_boost(&mut self, seated: bool, held: u32, drive: Option<Vec2>) {
        let down = held & buttons::SPRINT != 0;
        let clicked = down && !self.boost_down;
        self.boost_down = down;
        if !seated {
            self.boost = false;
            return;
        }
        if clicked {
            self.boost = !self.boost;
            self.boost_moved = false;
        }
        if drive.is_some_and(|stick| stick.length() > DEAD_ZONE) {
            self.boost_moved = true;
        } else if self.boost_moved && !down {
            self.boost = false;
        }
    }

    /// What the controllers press in a seat. B steps to the next seat as
    /// SWITCH_WEAPON; Y steps back, as the seat switch reads
    /// SPAWN_CLASS_PREV beside it. A droideka has no seats: B is its
    /// ROLL_STANCE, which folds it into the ball and back.
    pub fn seat_buttons(&self, view: &SimView<'_>) -> u32 {
        let pressed = self.buttons();
        let mut seat = pressed & (SEAT_BUTTONS | buttons::SPRINT);
        if rolls(view) {
            seat &= !buttons::SWITCH_WEAPON;
            if pressed & buttons::SWITCH_WEAPON != 0 {
                seat |= buttons::ROLL_STANCE;
            }
        } else if pressed & buttons::SWITCH_SECONDARY != 0 {
            seat |= buttons::SWITCH_WEAPON | buttons::SPAWN_CLASS_PREV;
        }
        if self.boost {
            seat |= buttons::SPRINT;
        }
        seat
    }

    pub fn consumed(&mut self) {
        self.pending = 0;
    }

    pub fn buttons(&self) -> u32 {
        self.held | self.pending
    }

    pub fn update_support(
        &mut self,
        view: Option<&SimView<'_>>,
        input: Input,
        guns: [Option<TrackedGun>; 2],
    ) {
        self.support = view
            .filter(|_| self.active && input.support_grip)
            .and_then(|view| {
                let unit = infantry(view)?;
                let gun = guns[usize::from(unit.active_channel == 1)]?;
                let foregrip = gun.foregrip?;
                let (left, right) = (input.hands[0].grip?, input.hands[1].grip?);
                input.hands[1].aim?;
                let span = left.position - right.position;
                span.try_normalize()?;
                if self.foregrip(view).is_none() {
                    let local = World {
                        rotation: Quat::IDENTITY,
                        translation: Vec3::ZERO,
                    };
                    let mount = local.hand_mount(input.hands[1], true)?;
                    if left
                        .position
                        .distance_squared(mount.transform_point3(foregrip))
                        > GRAB_RADIUS * GRAB_RADIUS
                    {
                        return None;
                    }
                }
                Some(Support {
                    unit: view.player,
                    gun,
                })
            });
    }

    pub fn foregrip(&self, view: &SimView<'_>) -> Option<Vec3> {
        let support = self.support.filter(|s| s.unit == view.player)?;
        let (class, slot, _) = infantry(view)?.active_weapon();
        (support.gun.class == class && support.gun.slot == slot)
            .then_some(support.gun.foregrip)
            .flatten()
    }

    pub fn pad(
        &self,
        unit: &Unit,
        head: Pose,
        input: Input,
        direction: Option<Vec3>,
        mut pad: PadState,
    ) -> PadState {
        pad.buttons |= self.buttons();
        // Keep the body animation and movement on the normal infantry input.
        // The separately recorded muzzle ray has the full controller pitch.
        pad.buttons = (pad.buttons | buttons::FIRST_PERSON) & !buttons::ZOOM;
        if let Some(aim) = input.hands[1].aim {
            let direction = direction
                .unwrap_or_else(|| Quat::from_rotation_y(self.yaw) * aim.orientation * Vec3::NEG_Z);
            let (yaw, pitch) = aim_angles(direction);
            body_look(unit, &mut pad, yaw, pitch);
        } else {
            pad.buttons &= !AIM_BUTTONS;
        }
        if let Some(motion) = input.movement.and_then(|stick| self.movement(head, stick)) {
            let (yaw, _) = unit
                .soldier
                .mouse_turned(pad.look(), unit.look_scale(), &unit.consts);
            let local = Quat::from_rotation_y(-yaw) * motion;
            pad.lx = PadState::quantize(local.x);
            pad.ly = PadState::quantize(-local.z);
        }
        pad
    }

    fn movement(&self, head: Pose, stick: Vec2) -> Option<Vec3> {
        let len = stick.length();
        if !len.is_finite() || len <= DEAD_ZONE {
            return None;
        }
        let travel = (len.min(1.0) - DEAD_ZONE) / (1.0 - DEAD_ZONE);
        Some(
            Quat::from_rotation_y(self.yaw + heading(head.orientation))
                * (Vec3::new(stick.x, 0.0, -stick.y) * (travel / len)),
        )
    }

    pub fn sprint_pad(
        &self,
        view: &SimView<'_>,
        head: Pose,
        stick: Vec2,
        latch: &mut input::SprintLatch,
        mut pad: PadState,
    ) -> PadState {
        let motion = self.movement(head, stick);
        if motion.is_none() && (pad.lx != 0 || pad.ly != 0) {
            return view.apply_sprint_latch(latch, pad);
        }
        let motion = motion.unwrap_or_default();
        // The latch measures stick travel before it is projected into the
        // weapon-facing body's axes. Preserve its ordinary cancellation rules.
        let travel = PadState::quantize(motion.length());
        let sprint =
            view.apply_sprint_latch_with_buffer(latch, PadState { ly: travel, ..pad }, true);
        pad.buttons = sprint.buttons;
        if travel < input::SPRINT_KEEP_LY {
            pad.buttons &= !buttons::SPRINT;
        } else if pad.pressed(buttons::SPRINT) {
            // Keep the ordinary forward sprint, including its acceleration
            // and stamina, by facing the body into travel. The headset view
            // and recorded muzzle ray still use their own poses.
            let unit = view.sim.unit(view.player).unwrap();
            let (_, pitch) = unit
                .soldier
                .mouse_turned(pad.look(), unit.look_scale(), &unit.consts);
            body_look(unit, &mut pad, aim_angles(motion).0, pitch);
            pad.lx = 0;
            pad.ly = i16::MAX;
        }
        pad
    }

    pub fn world(&self, view: Option<&SimView<'_>>, base: Camera, head: Pose) -> World {
        let origin = self.tracking_origin.unwrap_or(head);
        let unit = view.and_then(infantry);
        let (base_position, yaw) = match unit {
            Some(unit) => (unit.soldier.pos + Vec3::Y * unit.section().eye_y, self.yaw),
            // Keep a level horizon in the desktop chase/death/spectator view.
            None => (base.position, base.yaw - heading(origin.orientation)),
        };
        let rotation = if view.is_some_and(flies) {
            Quat::from_rotation_y(base.yaw)
                * Quat::from_rotation_x(base.pitch)
                * Quat::from_rotation_z(-base.roll)
                * Quat::from_rotation_y(-heading(origin.orientation))
        } else {
            Quat::from_rotation_y(yaw)
        };
        let mut translation = base_position - rotation * origin.position;
        if let Some(view) = view.filter(|_| unit.is_some()) {
            let eye = translation + rotation * head.position;
            translation += view.clip_tracked_eye(base_position, eye) - eye;
        }
        World {
            rotation,
            translation,
        }
    }
}

fn body_look(unit: &Unit, pad: &mut PadState, yaw: f32, pitch: f32) {
    let scale = f64::from(unit.look_scale() * unit.soldier.mouse_scale());
    pad.clear_look();
    if scale > 0.0 {
        pad.mx = PadState::quantize_look(-f64::from(wrap(yaw - unit.soldier.yaw)) / scale);
        pad.my = PadState::quantize_look(f64::from(pitch - unit.soldier.pitch) / scale);
    }
    pad.rx = 0;
    pad.ry = 0;
    pad.buttons |= buttons::MOUSE_AIM;
}

fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub(super) fn heading(rotation: Quat) -> f32 {
    let forward = rotation * Vec3::NEG_Z;
    if forward.x * forward.x + forward.z * forward.z > 1e-4 {
        aim_angles(forward).0
    } else {
        let right = rotation * Vec3::X;
        core::fmath::atan2(-right.z, right.x)
    }
}

fn aim_angles(direction: Vec3) -> (f32, f32) {
    (
        core::fmath::atan2(-direction.x, -direction.z),
        core::fmath::atan2(direction.y, direction.with_y(0.0).length()),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use core::fmath::FloatExt;
    use core::heightfield::Heightfield;
    use core::sim::{DropTable, Sim};
    use core::soldier::SoldierConstants;
    use core::unit::{Controller, UnitSpec};
    use core::weapon::HealthType;
    use std::sync::Arc;

    const HEAD: Pose = Pose {
        position: Vec3::ZERO,
        orientation: Quat::IDENTITY,
    };
    const EPS: f32 = 0.0001;

    pub(crate) fn test_sim(seed: u32, spawn: Vec3, hf: Option<Heightfield>) -> (Sim, UnitHandle) {
        test_sim_as(seed, spawn, hf, None)
    }

    fn test_sim_as(
        seed: u32,
        spawn: Vec3,
        hf: Option<Heightfield>,
        droideka: Option<Arc<core::droideka::DroidekaClass>>,
    ) -> (Sim, UnitHandle) {
        let mut sim = Sim::empty(seed);
        sim.heightfield = hf;
        let spawn = match &sim.heightfield {
            Some(h) => Vec3::new(spawn.x, spawn.y.fmax(h.height_at(spawn.x, spawn.z)), spawn.z),
            None => spawn,
        };
        let player = sim
            .spawn(&UnitSpec {
                team: 0,
                player_slot: Some(0),
                controller: Controller::Player { slot: 0 },
                pos: spawn,
                yaw: 0.0,
                consts: SoldierConstants::default(),
                max_health: 300.0,
                health_type: HealthType::Person,
                weapon_class: 0,
                total_clips: 6,
                weapon_slot: 0,
                slots: Arc::new([]),
                capture_posts: true,
                drops: DropTable::NONE,
                class: 0,
                skill: 0,
                jet: false,
                droideka,
            })
            .expect("fresh battle has reinforcements");
        (sim, player)
    }

    pub(crate) fn sim_view<'a>(sim: &'a Sim, player: UnitHandle) -> SimView<'a> {
        SimView {
            sim,
            player,
            local_slot: 0,
            camera: Camera::default(),
            spectating: false,
            eye_clip: None,
        }
    }

    #[test]
    fn support_grip_engages_near_the_foregrip_and_aims_with_both_hands() {
        let (mut sim, player) = test_sim(7, Vec3::ZERO, None);
        let (class, slot) = {
            let view = sim_view(&sim, player);
            let unit = infantry(&view).unwrap();
            (unit.weapon_class, unit.weapon_slot)
        };
        let anchor = Vec3::new(0.0, -0.12, 0.45);
        let gun = TrackedGun {
            class,
            slot,
            muzzle: Vec3::Z,
            foregrip: Some(anchor),
        };
        let world = World {
            rotation: Quat::IDENTITY,
            translation: Vec3::ZERO,
        };
        let mut rig = Rig {
            active: true,
            ..Default::default()
        };
        let mut input = Input {
            support_grip: true,
            hands: [
                Hand {
                    grip: Some(Pose {
                        position: Vec3::X,
                        ..HEAD
                    }),
                    aim: None,
                },
                Hand {
                    grip: Some(HEAD),
                    aim: Some(HEAD),
                },
            ],
            ..Default::default()
        };
        let guns = [Some(gun), None];
        {
            let view = sim_view(&sim, player);
            rig.update_support(Some(&view), input, guns);
            assert_eq!(rig.foregrip(&view), None);
        }
        let free = world.hand_mounts(input.hands)[1].unwrap();
        input.hands[0].grip.as_mut().unwrap().position = free.transform_point3(anchor);
        {
            let view = sim_view(&sim, player);
            rig.update_support(Some(&view), input, guns);
            assert_eq!(rig.foregrip(&view), Some(anchor));
        }

        let turn = Quat::from_rotation_y(SNAP);
        let left = turn * input.hands[0].grip.unwrap().position;
        input.hands[0].grip.as_mut().unwrap().position = left;
        {
            let view = sim_view(&sim, player);
            rig.update_support(Some(&view), input, guns);
            let aimed = world.weapon_hand_mounts(input.hands, rig.foregrip(&view))[1].unwrap();
            assert!(aimed
                .transform_point3(REAR_GRIP)
                .abs_diff_eq(Vec3::ZERO, EPS));
            assert!(aimed.transform_point3(anchor).abs_diff_eq(left, EPS));
            assert!(aimed
                .transform_vector3(Vec3::Z)
                .abs_diff_eq(turn * Vec3::NEG_Z, 0.04));

            input.support_grip = false;
            rig.update_support(Some(&view), input, guns);
            assert_eq!(rig.foregrip(&view), None);
            assert_eq!(
                world.weapon_hand_mounts(input.hands, rig.foregrip(&view))[1],
                Some(free)
            );
            input.support_grip = true;
            input.hands[0].grip.as_mut().unwrap().position = free.transform_point3(anchor);
            rig.update_support(Some(&view), input, guns);
            assert!(rig.foregrip(&view).is_some());
        }
        sim.unit_mut(player).unwrap().weapon_slot += 1;
        {
            let view = sim_view(&sim, player);
            assert_eq!(rig.foregrip(&view), None);
            input.hands[0].grip = None;
            rig.update_support(Some(&view), input, guns);
            assert!(rig.support.is_none());
        }
    }

    #[test]
    fn independently_tracked_hands_keep_the_right_mount_on_the_aim_ray() {
        let world = World {
            rotation: Quat::from_rotation_y(SNAP),
            translation: Vec3::new(1.0, 2.0, 3.0),
        };
        let left = Hand {
            grip: Some(Pose {
                position: Vec3::new(-0.3, -0.3, -0.5),
                orientation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            }),
            aim: None,
        };
        let right = Hand {
            grip: Some(Pose {
                position: Vec3::new(0.3, -0.3, -0.5),
                ..HEAD
            }),
            aim: Some(Pose {
                orientation: Quat::from_rotation_y(-SNAP),
                ..HEAD
            }),
        };
        let both = world.hand_mounts([left, right]);
        let [left_mount, right_mount] = both.map(Option::unwrap);
        assert!(left_mount.transform_point3(Vec3::ZERO).abs_diff_eq(
            world.pose(left.grip.unwrap()).position + world.rotation * HAND_MOUNT_OFFSET,
            EPS
        ));
        assert!(right_mount
            .transform_vector3(Vec3::Z)
            .abs_diff_eq(Vec3::NEG_Z, EPS));
        assert!(right_mount.transform_point3(Vec3::ZERO).abs_diff_eq(
            world.pose(right.grip.unwrap()).position + HAND_MOUNT_OFFSET,
            EPS
        ));
        assert_eq!(world.hand_mounts([Hand::default(), right]), [None, both[1]]);
        assert_eq!(world.hand_mounts([left, Hand::default()]), [both[0], None]);
        assert!(world.hand_mounts([left, Hand { aim: None, ..right }])[1].is_some());
        assert!(world.hand_mounts([
            left,
            Hand {
                grip: None,
                ..right
            }
        ])[1]
            .is_some());
    }

    #[test]
    fn hand_aim_and_head_relative_movement_are_independent_and_replayable() {
        let (sim, player) = test_sim(7, Vec3::ZERO, None);
        let view = sim_view(&sim, player);
        let u = infantry(&view).unwrap();
        let rig = Rig {
            yaw: u.soldier.yaw,
            ..Default::default()
        };
        let left = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let input = Input {
            hands: [
                Hand::default(),
                Hand {
                    aim: Some(Pose {
                        orientation: left,
                        ..HEAD
                    }),
                    grip: None,
                },
            ],
            movement: Some(Vec2::Y),
            ..Default::default()
        };
        let pad = rig.pad(u, HEAD, input, None, PadState::default());
        let (yaw, pitch) = u
            .soldier
            .mouse_turned(pad.look(), u.look_scale(), &u.consts);
        assert!((wrap(yaw - rig.yaw) - std::f32::consts::FRAC_PI_2).abs() < EPS);
        assert!(pitch.abs() < EPS);
        assert!(PadState::axis(pad.lx) > 0.99 && PadState::axis(pad.ly).abs() < EPS);
        assert!(pad.mouse_aim());
        assert!(pad.pressed(buttons::FIRST_PERSON));
    }

    #[test]
    fn tracking_loss_discards_a_queued_shot_and_requires_trigger_release() {
        let (sim, player) = test_sim(7, Vec3::ZERO, None);
        let view = sim_view(&sim, player);
        let mut rig = Rig::default();
        let mut input = Input::default();
        input.hands[1].aim = Some(HEAD);
        rig.track(Some(&view), Some(HEAD), input, true);
        input.buttons = buttons::FIRE;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(rig.pending, buttons::FIRE);
        input.hands[1].aim = None;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(rig.pending | rig.held, 0);
        input.hands[1].aim = Some(HEAD);
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(rig.pending | rig.held, 0);
        input.buttons = 0;
        rig.track(Some(&view), Some(HEAD), input, true);
        input.buttons = buttons::FIRE;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(rig.pending, buttons::FIRE);
    }

    #[test]
    fn snap_turns_once_per_flick_and_taps_survive_until_the_next_tick() {
        let (sim, player) = test_sim(7, Vec3::ZERO, None);
        let view = sim_view(&sim, player);
        let mut rig = Rig::default();
        let mut input = Input::default();
        input.hands[1].aim = Some(HEAD);
        rig.track(Some(&view), Some(HEAD), input, true);
        let start = rig.yaw;
        input.right_stick = Vec2::X;
        input.buttons =
            buttons::JUMP | buttons::ACTION | buttons::SWITCH_WEAPON | buttons::SWITCH_SECONDARY;
        let face_buttons = input.buttons;
        rig.track(Some(&view), Some(HEAD), input, true);
        input.buttons = 0;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert!((rig.yaw - start + SNAP).abs() < EPS);
        assert_eq!(rig.buttons(), face_buttons);
        rig.consumed();
        assert_eq!(rig.buttons(), 0);
        input.right_stick = Vec2::NEG_Y;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert!((rig.yaw - start + SNAP).abs() < EPS);
        assert_eq!(rig.buttons(), buttons::RELOAD);
        input.right_stick = Vec2::NEG_X;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert!((rig.yaw - start).abs() < EPS);
    }

    #[test]
    fn l3_can_be_tapped_before_running_in_any_stick_direction() {
        use core::soldier::{GroundRules, Stance};

        let aim = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let shot = core::sim::TrackedShot::new(Vec3::ZERO, Vec3::ZERO, Vec3::NEG_X);
        for direction in [Vec2::Y, Vec2::X, Vec2::NEG_Y] {
            let ground = Heightfield {
                size: 2,
                unit: 100.0,
                offset_x: 50.0,
                offset_z: 50.0,
                heights: vec![0.0; 4],
            };
            let (mut sim, player) = test_sim(7, Vec3::ZERO, Some(ground));
            sim.ground_rules = GroundRules::Bf2;
            let mut rig = Rig::default();
            let mut latch = input::SprintLatch::default();
            let mut sample = |sim: &mut Sim, input: Input| {
                let pad = {
                    let view = sim_view(sim, player);
                    rig.track(Some(&view), Some(HEAD), input, true);
                    let unit = infantry(&view).unwrap();
                    let mut pad = rig.pad(unit, HEAD, input, None, PadState::default());
                    pad.tracked_shots[0] = Some(shot);
                    let pad = rig.sprint_pad(&view, HEAD, input.movement.unwrap(), &mut latch, pad);
                    rig.consumed();
                    assert_eq!(pad.tracked_shots[0], Some(shot));
                    pad
                };
                sim.step(&[pad]);
                pad
            };
            let mut input = Input {
                movement: Some(Vec2::ZERO),
                ..Default::default()
            };
            input.hands[1].aim = Some(Pose {
                orientation: aim,
                ..HEAD
            });
            sample(&mut sim, input);
            input.buttons = buttons::SPRINT;
            assert!(!sample(&mut sim, input).pressed(buttons::SPRINT));
            input.buttons = 0;
            for _ in 0..3 {
                assert!(!sample(&mut sim, input).pressed(buttons::SPRINT));
            }
            input.movement = Some(direction * 0.8);
            for _ in 0..45 {
                let pad = sample(&mut sim, input);
                assert!(pad.pressed(buttons::SPRINT));
                assert_eq!((pad.lx, pad.ly), (0, i16::MAX));
            }
            let soldier = &sim.unit(player).unwrap().soldier;
            assert_eq!(soldier.stance, Stance::Sprint);
            assert!(
                soldier
                    .vel
                    .normalize()
                    .dot(Vec3::new(direction.x, 0.0, -direction.y))
                    > 0.99
            );
            input.movement = Some(Vec2::ZERO);
            assert!(!sample(&mut sim, input).pressed(buttons::SPRINT));
            assert_ne!(sim.unit(player).unwrap().soldier.stance, Stance::Sprint);
            input.movement = Some(direction);
            assert!(!sample(&mut sim, input).pressed(buttons::SPRINT));
        }
    }

    #[test]
    fn vr_sprint_leaves_keyboard_movement_available_with_a_centered_stick() {
        let (sim, player) = test_sim(7, Vec3::ZERO, None);
        let view = sim_view(&sim, player);
        let mut latch = input::SprintLatch::default();
        let rig = Rig::default();
        let mut pad = PadState {
            ly: i16::MAX,
            buttons: buttons::SPRINT,
            ..Default::default()
        };
        assert_eq!(
            rig.sprint_pad(&view, HEAD, Vec2::ZERO, &mut latch, pad),
            pad
        );
        pad.buttons = 0;
        assert!(rig
            .sprint_pad(&view, HEAD, Vec2::ZERO, &mut latch, pad)
            .pressed(buttons::SPRINT));
    }

    #[test]
    fn a_seat_takes_the_controllers_buttons_and_latches_l3_while_the_stick_drives() {
        let (mut sim, player) = test_sim(7, Vec3::ZERO, None);
        sim.unit_mut(player).unwrap().mount = Some(core::vehicle::Mount {
            vehicle: core::vehicle::VehicleHandle { index: 0, gen: 0 },
            seat: 0,
        });
        let view = sim_view(&sim, player);
        assert!(piloted(&view) && infantry(&view).is_none());
        let mut rig = Rig::default();
        let mut input = Input::default();
        rig.track(Some(&view), Some(HEAD), input, true);
        input.buttons = AIM_BUTTONS | buttons::ACTION | buttons::SWITCH_WEAPON | buttons::RELOAD;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(
            rig.seat_buttons(&view),
            AIM_BUTTONS | buttons::ACTION | buttons::SWITCH_WEAPON
        );
        input.buttons = 0;
        rig.track(Some(&view), Some(HEAD), input, true);
        rig.consumed();
        input.buttons = buttons::SWITCH_SECONDARY;
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(
            rig.seat_buttons(&view),
            buttons::SWITCH_WEAPON | buttons::SPAWN_CLASS_PREV
        );
        input.buttons = 0;
        rig.track(Some(&view), Some(HEAD), input, true);
        rig.consumed();

        input.buttons = buttons::SPRINT;
        rig.track(Some(&view), Some(HEAD), input, true);
        input.buttons = 0;
        for drive in [None, Some(Vec2::Y), Some(Vec2::X)] {
            input.movement = drive;
            rig.track(Some(&view), Some(HEAD), input, true);
            rig.consumed();
            assert_eq!(rig.seat_buttons(&view), buttons::SPRINT);
        }
        input.movement = Some(Vec2::ZERO);
        rig.track(Some(&view), Some(HEAD), input, true);
        assert_eq!(rig.seat_buttons(&view), 0);
    }

    #[test]
    fn a_droideka_drives_like_a_seat_and_folds_with_b() {
        let ground = Heightfield {
            size: 2,
            unit: 100.0,
            offset_x: 50.0,
            offset_z: 50.0,
            heights: vec![0.0; 4],
        };
        let class = Arc::new(core::droideka::DroidekaClass::default());
        let (mut sim, player) = test_sim_as(7, Vec3::ZERO, Some(ground), Some(class));
        let mut rig = Rig::default();
        let mut sample = |sim: &mut Sim, input: Input| {
            let pad = {
                let view = sim_view(sim, player);
                assert!(piloted(&view) && infantry(&view).is_none() && !flies(&view));
                rig.track(Some(&view), Some(HEAD), input, true);
                let mut pad = seat_sticks(input, PadState::default(), 1.0);
                pad.buttons |= rig.seat_buttons(&view);
                rig.consumed();
                pad
            };
            sim.step(&[pad]);
            pad
        };
        let mut input = Input::default();
        for _ in 0..5 {
            sample(&mut sim, input);
        }
        let start = sim.unit(player).unwrap().soldier.pos;
        input.movement = Some(Vec2::Y);
        for _ in 0..90 {
            sample(&mut sim, input);
        }
        assert!(sim.unit(player).unwrap().soldier.pos.distance(start) > 0.5);

        input.movement = Some(Vec2::ZERO);
        input.buttons = buttons::SWITCH_WEAPON;
        let pad = sample(&mut sim, input);
        assert_eq!(pad.buttons, buttons::ROLL_STANCE);
        input.buttons = 0;
        for _ in 0..180 {
            sample(&mut sim, input);
        }
        assert_eq!(
            sim.unit(player).unwrap().droideka_form(),
            Some(core::droideka::Form::Ball)
        );
    }

    #[test]
    fn seat_sticks_stand_in_for_a_gamepads_and_leave_it_alone_when_centred() {
        let held = PadState {
            lx: 100,
            ry: -100,
            ..Default::default()
        };
        assert_eq!(seat_sticks(Input::default(), held, 1.0), held);
        let input = Input {
            movement: Some(Vec2::Y),
            right_stick: Vec2::NEG_X,
            ..Default::default()
        };
        let pad = seat_sticks(input, held, 1.0);
        assert_eq!((pad.lx, pad.ly), (0, i16::MAX));
        assert_eq!((pad.rx, pad.ry), (input::look_axis(-1.0, 1.0), 0));
    }

    #[test]
    fn stereo_transform_preserves_eye_spacing_and_head_roll() {
        let rig = Rig::default();
        let base = Camera {
            yaw: SNAP,
            ..Default::default()
        };
        let world = rig.world(None, base, HEAD);
        let roll = Quat::from_rotation_z(0.3);
        let left = world.camera(
            Pose {
                position: Vec3::NEG_X * 0.032,
                orientation: roll,
            },
            base,
        );
        let right = world.camera(
            Pose {
                position: Vec3::X * 0.032,
                orientation: roll,
            },
            base,
        );
        assert!((left.position.distance(right.position) - 0.064).abs() < EPS);
        assert!((left.roll + 0.3).abs() < EPS);
        assert!(left.forward().abs_diff_eq(base.forward(), EPS));
    }
}
