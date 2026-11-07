//! The chase, eye, seat and turret cameras of a walk, with the view pull, field of view and see-through rules.

use super::*;

/// The death view hold time in seconds (SWBFRNT2.ELF line 333652: *(float *)(param_1 + 0x158) = 4.0f).
pub const DEATH_VIEW_HOLD: f32 = 4.0;

impl WalkSim {
    pub fn camera(&self) -> Camera {
        self.camera_track().0
    }

    /// The drawn camera, and for a camera the world pulls in, which one it
    /// is, its pull point and its track's clear fraction this frame.
    pub(super) fn camera_track(&self) -> (Camera, Option<Track>) {
        // The end of the round stands on its camera shot (BF1, issue #33).
        if let Some(shot) = self.round_end.as_ref().and_then(|r| r.shot) {
            let cam = Camera {
                fov_y: self.fov_y(),
                ..shot.camera()
            };
            return (cam, None);
        }
        if let Some(b) = &self.backdrop {
            return (b.camera(), None);
        }
        if let Some(view) = self.held_seat_view() {
            return (view, None);
        }
        if let Some(r) = self.sim.controlled_guided(self.player) {
            if let Some(c) = self.sim.guided_class(r) {
                return (guided_camera(r, c, self.fov_y()), None);
            }
        }
        if let Some(got) = self.droid_camera() {
            return got;
        }
        if let Some(got) = self.flyer_seat_camera() {
            return got;
        }
        if let Some(got) = self.seat_camera() {
            return got;
        }
        let me = &self.sim.units[self.player.index as usize];
        let ideal = me.ideal_camera(self.sim.ground_rules, self.sim.views());
        if me.eye_view() {
            return (self.eye_camera(&ideal), None);
        }
        let (center, position) = self.drawn_eye(me, &ideal);
        let position = position - self.cam_jet_lag;
        // A wall pulls the camera in toward the PS2's 1.5 m point: it keeps
        // its view (pitch and yaw, the look along his aim) and never drops
        // under that height to the floor at his feet (playtest 2026-10-28).
        let pull = me.soldier.pos + Vec3::Y * CAMERA_PULL_Y;
        let (position, mut track) =
            self.settle_camera(CamKind::Soldier, pull, position, None, None);
        let body = me.soldier.pos + Vec3::Y * CAMERA_SIGHT_BODY_Y.min(center.y - me.soldier.pos.y);
        track.sights = [center, body];
        let cam = Camera {
            position,
            yaw: me.soldier.yaw,
            pitch: core::fmath::atan2(ideal.forward.y, ideal.forward.with_y(0.0).length()),
            fov_y: self.fov_y(),
            ..Camera::default()
        };
        (cam, Some(track))
    }

    pub(super) fn next_seat_view(&self) -> Option<Camera> {
        let me = self.sim.units.get(self.player.index as usize)?;
        if !me.alive {
            return (self.seat_view_timer > 0.0).then_some(()).and(self.seat_view);
        }
        me.mount.is_some().then(|| self.camera())
    }

    fn held_seat_view(&self) -> Option<Camera> {
        let me = self.sim.units.get(self.player.index as usize)?;
        if self.seat_view_timer <= 0.0 {
            return None;
        }
        self.seat_view.filter(|_| !me.alive && !me.body)
    }

    pub fn eye_camera(&self, eye: &core::unit::IdealCamera) -> Camera {
        look_camera(eye.position, eye.forward, self.fov_y())
    }

    pub fn first_person(&self) -> bool {
        self.sim
            .unit(self.player)
            .is_some_and(|u| u.first_person_view())
    }

    /// Is the player's view looking from the soldier's eye (first person or
    /// scope) rather than an external camera (scenic shot, backdrop, guided
    /// missile, remote droid, or vehicle seat)?
    pub fn in_eye_view(&self) -> bool {
        if self.round_end.as_ref().is_some_and(|r| r.shot.is_some())
            || self.backdrop.is_some()
            || self.sim.controlled_guided(self.player).is_some()
            || self.sim.controlled_droid(self.player).is_some()
        {
            return false;
        }
        self.sim.unit(self.player).is_some_and(|u| u.eye_view())
    }

    /// Are first-person arms and gun drawn in the player's view?
    pub fn shows_first_person_arms(&self) -> bool {
        self.in_eye_view() && self.sim.unit(self.player).is_some_and(|u| u.zoom.is_none())
    }

    /// One tick of the camera collision's stored fraction
    /// ([`WalkSim::cam_pull`]).
    pub(super) fn step_cam_pull(&mut self) {
        let Some(track) = self.camera_track().1 else {
            self.cam_pull = 1.0;
            self.cam_pull_track = None;
            return;
        };
        self.cam_pull = if self.same_cam_track(&track) {
            crate::camera_collide::ease_pull(self.cam_pull, track.raw, core::TICK_DT)
        } else {
            track.raw
        };
        self.cam_pull_track = Some((track.kind, track.target));
    }

    pub(super) fn same_cam_track(&self, track: &Track) -> bool {
        self.cam_pull_track.is_some_and(|(kind, target)| {
            kind == track.kind
                && target.distance_squared(track.target) <= CAM_PULL_SNAP_M * CAM_PULL_SNAP_M
        })
    }

    pub(super) fn drawn_eye(
        &self,
        me: &core::unit::Unit,
        ideal: &core::unit::IdealCamera,
    ) -> (Vec3, Vec3) {
        let sec = me.section();
        let Some((eye_y, offset)) = self.cam_place.filter(|_| !me.eye_view()) else {
            return (ideal.center, ideal.position);
        };
        if (eye_y, offset) == (sec.eye_y, sec.offset) {
            return (ideal.center, ideal.position);
        }
        let lift = Vec3::Y * (eye_y - sec.eye_y);
        let d = offset - sec.offset;
        let right = ideal.forward.cross(ideal.up);
        (
            ideal.center + lift,
            ideal.position + lift + right * d.x + ideal.up * d.y - ideal.forward * d.z,
        )
    }

    pub(super) fn next_cam_jet_lag(&self) -> Vec3 {
        let Some(me) = self.sim.units.get(self.player.index as usize) else {
            return Vec3::ZERO;
        };
        if self.backdrop.is_some() || me.mount.is_some() || me.eye_view() {
            return Vec3::ZERO;
        }
        let vel = me.soldier.stance.jetting().then_some(me.soldier.vel);
        core::unit::jet_camera_lag(self.cam_jet_lag, vel)
    }

    pub(super) fn next_cam_place(&self) -> Option<(f32, Vec3)> {
        let me = self.sim.unit(self.player)?;
        if self.backdrop.is_some() || me.mount.is_some() || me.eye_view() {
            return None;
        }
        let sec = me.section();
        let to = (sec.eye_y, sec.offset);
        match self.cam_place {
            Some(from) if me.zoom.is_none() => Some(ease_camera_place(from, to, core::TICK_DT)),
            _ => Some(to),
        }
    }

    /// The end of the round, one frame (issue #33): it starts the first time
    /// the round is decided - the viewer's result, a shot drawn from the
    /// mission's (seeded by the round's tick), the banner for `music_secs` -
    /// and steps with this frame's presses on the stats pages (issue #50;
    /// the caller makes them when [`crate::round_end::RoundEnd::wants_stats`]).
    /// True once it is left (the next playlist map, else the shell).
    pub fn follow_round_end(
        &mut self,
        dt: f32,
        pad: ui::stats::Pad,
        music_secs: Option<f32>,
    ) -> bool {
        let Some(winner) = self.sim.outcome else {
            return false;
        };
        let r = self.round_end.get_or_insert_with(|| {
            use crate::round_end::{Result, RoundEnd};
            let me = self.sim.units[self.player.index as usize].team;
            let result = if winner == core::sim::OUTCOME_DRAW {
                Result::Draw
            } else if winner == me {
                Result::Victory
            } else {
                Result::Defeat
            };
            RoundEnd::start(result, &self.end_shots, self.sim.tick as u32, music_secs)
        });
        r.step(dt, pad);
        r.done()
    }

    /// The level start's backdrop follows slot 0's spawn screen and ends
    /// with the player's first spawn (card SSELB): after that a spawn screen
    /// stands over the place he died.
    pub fn follow_backdrop(&mut self) {
        let alive = self.sim.units[self.player.index as usize].alive;
        if alive || !self.sim.spawn_screens {
            self.backdrop = None;
        }
        if let Some(b) = self.backdrop.as_mut() {
            b.follow(self.sim.spawn_stage.first().copied().unwrap_or_default());
        }
    }

    /// Where the reticle sits, as a fraction of the window's height from the
    /// top (§10.4): the point 1024 m along the player's aim ray through the
    /// drawn camera. With the camera at its ideal place that is tan(tilt) /
    /// tan(fov_y / 2) of the half height above the centre, at any aspect.
    /// A rider's stays on the centre (the seat cameras are PROVISIONAL).
    pub fn reticle_y(&self) -> f32 {
        let me = &self.sim.units[self.player.index as usize];
        if let Some(look) = self.sim.seat_look(self.player) {
            let point = look.eye + look.view * 1024.0;
            let clip = self.camera().view_proj(1.0) * point.extend(1.0);
            return 0.5 - 0.5 * clip.y / clip.w;
        }
        let (a, d) = if let Some(r) = self.sim.controlled_guided(self.player) {
            (r.pos, r.forward())
        } else if let Some(droid) = self.sim.controlled_droid(self.player) {
            (droid.muzzle(), droid.aim_frame().0)
        } else {
            me.aim_ray(self.sim.ground_rules, self.sim.views())
        };
        let clip = self.camera().view_proj(1.0) * (a + d * 1024.0).extend(1.0);
        0.5 - 0.5 * clip.y / clip.w
    }

    pub(super) fn droid_camera(&self) -> Option<(Camera, Option<Track>)> {
        let d = self.sim.controlled_droid(self.player)?;
        let (target, cam) = droid_camera(d, self.sim.droid_class(d)?, self.fov_y());
        let (position, track) =
            self.settle_camera(CamKind::Droid, target, cam.position, None, None);
        Some((Camera { position, ..cam }, Some(track)))
    }

    /// The player's seat camera, when the player rides (card VSEAT). A
    /// building turret's gunner is in third person always, the camera on the
    /// gun's look; zoomed (card VTURZ) it is [`turret_scope_camera`].
    pub(super) fn seat_camera(&self) -> Option<(Camera, Option<Track>)> {
        if self.sim.seat_zoom(self.player).is_some() {
            let look = self.sim.seat_look(self.player)?;
            return Some((turret_scope_camera(&look, self.fov_y()), None));
        }
        let m = self.sim.unit(self.player)?.mount?;
        let v = self.sim.vehicle(m.vehicle)?;
        let seat = self
            .vehicle_seats
            .get(v.class as usize)
            .and_then(|r| r.get(usize::from(m.seat)));
        let gun_look = v
            .guns
            .get(usize::from(m.seat))
            .and_then(|g| g.as_ref())
            .map(|g| g.look);
        // A walker's pilot looks along the walker's aim (card WALK), armed
        // seat or not.
        let walker_pilot = v.kind == core::vehicle::VehicleKind::Walker && m.seat == 0;
        let look = if walker_pilot {
            Some((v.walker.aim_yaw, v.walker.aim_pitch))
        } else {
            gun_look
        };
        let (target, cam) = self
            .walker_seat_camera(v, seat, look)
            .or_else(|| self.turret_seat_camera(v, seat))
            .unwrap_or_else(|| {
                let origin = self.seat_pull_origin(v, m.seat);
                vehicle_camera(v, seat, look, self.fov_y(), origin)
            });
        let target = if m.seat == 0 && v.kind != core::vehicle::VehicleKind::Turret {
            self.pull_point_on_own_side(v, target)
        } else {
            target
        };
        // A building turret's camera passes its own building, as its aim
        // ray does (`Sim::aim_point`); a rider's, his own vehicle's hull.
        let (position, track) = self.settle_camera(
            CamKind::Seat,
            target,
            cam.position,
            v.instance,
            Some(m.vehicle),
        );
        Some((Camera { position, ..cam }, Some(track)))
    }

    /// `[ours]` A pilot's pull point, stopped short of any wall or terrain between it and
    /// the section's fixed point on the vehicle. A tall model's point stands metres off
    /// the hull, so without this it passes a hangar's sheet or, banked, a wall beside
    /// the flyer, and the camera settles on the far side of them.
    pub(super) fn pull_point_on_own_side(&self, v: &core::vehicle::Vehicle, point: Vec3) -> Vec3 {
        let base = v.pos + v.rot * (Vec3::Y * CAMERA_PULL_Y);
        let f = self.camera_blockers.fraction(
            base,
            point,
            v.instance,
            |i| self.station_pose(i),
            &self.sim.doors,
        );
        let end = base + (point - base) * f;
        let Some(h) = &self.sim.heightfield else {
            return end;
        };
        let (dir, len) = ((end - base).normalize_or_zero(), (end - base).length());
        let cutters = &self.sim.collision.cutters;
        match core::collision::ray_heightfield(h, cutters, base, dir, len, 0.25) {
            Some(t) => base + dir * (t - CAMERA_TERRAIN_GAP).max(0.0),
            None => end,
        }
    }

    /// The point a seat's camera is pulled in toward, in the vehicle's frame: the
    /// pilot's by [`vehicle_pull_height`], and a gunner's the fixed point of his own
    /// turret's section, which no class writes (BF2 `FUN_00270540`, BF1
    /// `FUN_00225cd8`), on his gun's pivot.
    pub(crate) fn seat_pull_origin(&self, v: &core::vehicle::Vehicle, seat: u8) -> Vec3 {
        if seat == 0 {
            let top = self.vehicle_tops.get(v.class as usize).copied().flatten();
            return Vec3::Y * vehicle_pull_height(v.kind, top);
        }
        let pivot = self
            .sim
            .seat_look(self.player)
            .filter(|l| l.gun.framed)
            .map_or(Vec3::ZERO, |l| l.gun.pivot);
        pivot + Vec3::Y * CAMERA_PULL_Y
    }

    /// A building turret gunner's unzoomed camera as `(pull point, camera)`
    /// (`analysis/re-turret-draw.md` 4 [V]): `TrackCenter` and `TrackOffset` on
    /// the aim frame at the turret node, where the zoomed eye stands too
    /// ([`core::vehicle::gun::GunSetup::scope_eye`]); a class with no turret
    /// node has it at the building's origin. Hung on the origin instead,
    /// `tur_bldg_geoturret`'s centre (0, 0, -2) lay under the sheet where the
    /// ground rises behind the turret (card SEATCAM). The pull point is the
    /// section's fixed (0, [`CAMERA_PULL_Y`], 0) on the same frame.
    pub(super) fn turret_seat_camera(
        &self,
        v: &core::vehicle::Vehicle,
        seat: Option<&crate::vehicle_class::SeatSpec>,
    ) -> Option<(Vec3, Camera)> {
        if v.kind != core::vehicle::VehicleKind::Turret {
            return None;
        }
        let (seat, l) = (seat?, self.sim.seat_look(self.player)?);
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
        let (_, cam) = walker_view::camera_on_frame(
            frame,
            Vec3::from(seat.track_center.unwrap_or([0.0, 2.0, 0.0])),
            Vec3::from(seat.track_offset.unwrap_or([0.0, 1.0, 8.0])),
            seat.tilt_value.unwrap_or(0.0),
            self.fov_y(),
        );
        Some((frame.transform_point3(Vec3::Y * CAMERA_PULL_Y), cam))
    }

    /// Pull a camera wanted at `desired` looking at `target` in along the
    /// track until the world's big models are
    /// [`crate::camera_collide::CAMERA_RADIUS`] clear of it (`own_instance`,
    /// a gunner's building, excepted; props and vehicles never pull, they
    /// are seen through, [`WalkSim::camera_see_through`]; a shorter track at
    /// once, a longer one at the PS2's return rate), then
    /// in short of the terrain sheet where the track crosses it
    /// (the sheet is a surface, met from above or below; a cutter's hole is
    /// none), then keep it [`CAMERA_TERRAIN_GAP`] off the sheet on the side
    /// it is on. It never moves through the sheet: BF2 GEO1's hangar is 31 m
    /// under uncut terrain, and its entrance deck lies in a cutter's hole
    /// beside the hill, where a floor alone lifted the camera onto the hill.
    pub(super) fn settle_camera(
        &self,
        kind: CamKind,
        target: Vec3,
        desired: Vec3,
        own_instance: Option<u32>,
        own_vehicle: Option<core::vehicle::VehicleHandle>,
    ) -> (Vec3, Track) {
        // The world's big drawn models: pull the camera in along the track
        // line before flooring, so a wall behind the player shortens the
        // track instead of putting the camera inside the room next door
        // (`camera_collide`).
        let raw = if self.camera_blockers.is_empty() {
            1.0
        } else {
            self.camera_blockers.fraction(
                target,
                desired,
                own_instance,
                |i| self.station_pose(i),
                &self.sim.doors,
            )
        };
        let track = Track {
            kind,
            target,
            raw,
            own_instance,
            own_vehicle,
            sights: [target; 2],
        };
        // In at once, back out at the PS2's rate (`step_cam_pull`).
        let f = if self.same_cam_track(&track) {
            raw.min(self.cam_pull)
        } else {
            raw
        };
        let mut position = target + (desired - target) * f;
        if let Some(h) = &self.sim.heightfield {
            let track = position - target;
            let len = track.length();
            if len > 1e-3 {
                let dir = track / len;
                let cutters = &self.sim.collision.cutters;
                if let Some(t) =
                    core::collision::ray_heightfield(h, cutters, target, dir, len, 0.25)
                {
                    position = target + dir * (t - CAMERA_TERRAIN_GAP).max(0.0);
                }
            }
            let y = h.height_at(position.x, position.z);
            if !self
                .sim
                .collision
                .is_cut(Vec3::new(position.x, y, position.z))
            {
                position.y = if position.y >= y {
                    position.y.max(y + CAMERA_TERRAIN_GAP)
                } else {
                    position.y.min(y - CAMERA_TERRAIN_GAP)
                };
            }
        }
        (position, track)
    }

    /// The chase camera's vertical FOV: the PS2's, times the player's option.
    /// The PS2's is 2 atan(tan(FOV / 2) x 0.7666 / S) (`re-zoom.md` §5):
    /// [`chase_fov_y`] is it for the third-person FOV at S = 1, on every map.
    /// The seat cameras take it too (card VSEAT, PROVISIONAL).
    pub fn fov_y(&self) -> f32 {
        let k = self.fov_scale.clamp(FOV_SCALE_RANGE.0, FOV_SCALE_RANGE.1);
        if let Some(degrees) = self.flyer_fov() {
            let (s, c) = core::fmath::sin_cos(degrees.to_radians() * 0.5);
            return (2.0 * core::fmath::atan2(PS2_FOV_TAN_SCALE * s / c, 1.0) * k).min(MAX_FOV_Y);
        }
        let mut fov = self.walker_fovs().map_or_else(chase_fov_y, |(third, _)| {
            let (s, c) = core::fmath::sin_cos(third.to_radians() * 0.5);
            2.0 * core::fmath::atan2(PS2_FOV_TAN_SCALE * s / c, 1.0)
        });
        if self.zoom_s != 1.0 || self.zoom_fov_mix != 0.0 {
            let (third, first) = self.zoom_fovs();
            let half = |rad: f32| {
                let (s, c) = core::fmath::sin_cos(rad * 0.5);
                s / c
            };
            let eased = half((third + (first - third) * self.zoom_fov_mix).to_radians())
                / half(third.to_radians());
            fov = 2.0 * core::fmath::atan2(half(fov) * eased, self.zoom_s);
        }
        (fov * k).min(MAX_FOV_Y)
    }

    /// The (third-person, zoomed) FOVs in degrees the zoom eases between:
    /// the soldier class's, or in a seat the seat camera's unzoomed one and
    /// the turret class's `FirstPersonFOV` (card VTURZ, re-turret-draw 4: a
    /// zoomed turret's camera takes the class's +0x1bc, 48 when not
    /// authored - `tur_bldg_recoilless_lg`'s 12.98 deg at zoom 3 against
    /// 13.0-13.5 fitted to the PS2's frame, `lead_tmp/vturz/fit_fov.py`).
    pub(super) fn zoom_fovs(&self) -> (f32, f32) {
        if let Some(fovs) = self.walker_fovs() {
            return fovs;
        }
        let soldier = chase_fovs();
        let Some(m) = self.sim.unit(self.player).and_then(|u| u.mount) else {
            return soldier;
        };
        let first = self
            .sim
            .vehicle(m.vehicle)
            .and_then(|v| self.vehicle_seats.get(v.class as usize))
            .and_then(|r| r.get(usize::from(m.seat)))
            .and_then(|s| s.first_person_fov)
            .unwrap_or(48.0);
        (soldier.0, first)
    }

    /// A droid's drawn pose by its manifest instance.
    pub(super) fn station_pose(&self, instance: u32) -> Option<Mat4> {
        self.sim
            .stations
            .iter()
            .find(|s| s.instance == Some(instance))
            .map(|s| s.pose())
    }

    /// The props and vehicles the drawn chase camera sees through now (ours,
    /// playtest 2026-10-28, `crate::camera_collide`): those between it and
    /// the player's eye or body (a seat's or droid's camera: its target),
    /// and those it stands in. Nothing for a camera the world does not pull
    /// (the round's end, the backdrop, a zoomed turret).
    pub fn camera_see_through(&self) -> crate::camera_collide::SeeThrough {
        let (cam, Some(t)) = self.camera_track() else {
            return Default::default();
        };
        let sights = t.sights;
        let props = if self.camera_blockers.is_empty() {
            Vec::new()
        } else {
            self.camera_blockers.see_through(
                cam.position,
                &sights,
                t.own_instance,
                |i| self.station_pose(i),
                &self.sim.doors,
            )
        };
        let alive = self
            .sim
            .vehicles
            .iter()
            .enumerate()
            .filter(|(_, v)| v.alive);
        let vehicles = crate::camera_collide::see_through_vehicles(
            &self.sim.collision,
            alive.map(|(i, v)| core::vehicle::VehicleHandle {
                index: i as u32,
                gen: v.gen,
            }),
            cam.position,
            &sights,
            t.own_vehicle,
        );
        crate::camera_collide::SeeThrough { props, vehicles }
    }

    /// Is the player's own body left out of the draw (the scope view,
    /// `re-zoom.md` §5)?
    pub fn player_body_hidden(&self) -> bool {
        self.in_eye_view() || self.camera_in_body()
    }

    /// Has the world pulled the chase camera into the soldier
    /// ([`crate::camera_collide::BODY_HIDE_DIST`] from his pull point)? His
    /// body is hidden then (the PS2 ghosts him).
    pub fn camera_in_body(&self) -> bool {
        match self.camera_track() {
            (cam, Some(t)) if t.kind == CamKind::Soldier => {
                cam.position.distance(t.target) < crate::camera_collide::BODY_HIDE_DIST
            }
            _ => false,
        }
    }
}
