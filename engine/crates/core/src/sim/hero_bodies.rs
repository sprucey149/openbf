//! Physical hero queries include the native exposed-rider collision type.

use super::Sim;
use glam::Vec3;

/// Immutable class data supplies the same seat frame as the figure. The
/// evaluator may read only vehicle state and baked data, never a clock/RNG.
pub trait HeroRiderFrames: Send + Sync {
    fn frame(&self, vehicle: &crate::vehicle::Vehicle, seat: u8) -> Option<glam::Mat4>;
}

/// Issue #182: where a seated rider's drawn head and torso are, by his
/// class and seat. The game fills it at round setup from the soldier's
/// baked skeleton and the seat's pose clip (`game::seated_bodies`): the
/// bone_head and bone_ribcage points of the pose the seat draws him in, in
/// his model frame (+Z his facing, +Y up, origin where he hangs). A size
/// that differs from a trooper's (the MagnaGuard's 1.24 root, a Wookiee, a
/// Jawa) and a gunner leaning over his gun are hit where they are drawn.
/// SETUP data: never written by `step`, so not in `Sim::hash`; each peer
/// samples it from its own copy, so it is in the static digest's "classes"
/// group (`Sim::static_digest_groups`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeatedBodies {
    pub by: std::collections::BTreeMap<SeatedKey, SeatedBody>,
}

/// (team, `UnitSpec::class`, `Vehicle::class`, seat).
pub type SeatedKey = (u8, u8, u32, u8);

/// One class in one seat ([`SeatedBodies`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeatedBody {
    /// The seat's `PitchLimits` / `YawLimits` as authored (degrees), read
    /// by the nine-pose numbers ([`nine_numbers`]).
    pub pitch_limits: Option<[f32; 2]>,
    pub yaw_limits: Option<[f32; 2]>,
    /// [head, torso]: one pair for a seat posed by one frame, else
    /// [`NINE_GRID`]² pairs of a nine-pose seat, row-major by the yaw number
    /// then the pitch number, each over -1 ..= 1.
    pub points: Vec<[Vec3; 2]>,
}

/// Samples per nine-pose number in [`SeatedBody::points`].
pub const NINE_GRID: usize = 5;

/// The nine-pose numbers (yaw, pitch) of a gun at `look` (the port's yaw,
/// pitch up), each -1 ..= 1, as the drawn seat poses its soldier
/// (`game::viewer::turret_pose::nine_pose`): placed on the authored
/// limits, the pitch 0.5 with none; a yaw with no limits inside
/// (0, 270 deg) turns by the stick, which the sim does not keep: 0
/// (PROVISIONAL: the lean of a turning full-circle gunner, a few cm;
/// measure: the drawn head against the prim while a gun turns).
pub fn nine_numbers(
    look: (f32, f32),
    pitch: Option<[f32; 2]>,
    yaw: Option<[f32; 2]>,
) -> (f32, f32) {
    let rad = |l: Option<[f32; 2]>| l.map_or([0.0; 2], |l| l.map(f32::to_radians));
    let place = |x: f32, [lo, hi]: [f32; 2]| (((x - lo) / (hi - lo) - 0.5) * 2.0).clamp(-1.0, 1.0);
    let p = rad(pitch);
    let pf = if p[0] >= p[1] { 0.5 } else { place(-look.1, p) };
    let yf = if nine_yaw_limited(yaw) {
        place(look.0, rad(yaw))
    } else {
        0.0
    };
    (yf, pf)
}

/// Does a seat's yaw number follow its look ([`nine_numbers`]): authored
/// yaw limits spanning more than 0 and under 270 degrees? Otherwise the
/// drawn seat turns it by the stick.
pub fn nine_yaw_limited(yaw: Option<[f32; 2]>) -> bool {
    let [lo, hi] = yaw.map_or([0.0; 2], |l| l.map(f32::to_radians));
    hi - lo > 0.0 && hi - lo < 4.712389
}

impl SeatedBody {
    /// [head, torso] in his model frame at the nine-pose numbers (yaw,
    /// pitch), between the grid's samples by their bilinear blend.
    pub fn at(&self, (yf, pf): (f32, f32)) -> Option<[Vec3; 2]> {
        let n = NINE_GRID;
        if self.points.len() != n * n {
            return self.points.first().copied();
        }
        let cell = |f: f32| {
            let x = (f.clamp(-1.0, 1.0) + 1.0) * 0.5 * (n - 1) as f32;
            let i = (x as usize).min(n - 2);
            (i, x - i as f32)
        };
        let ((r, ry), (c, cx)) = (cell(yf), cell(pf));
        let p = |r: usize, c: usize| self.points[r * n + c];
        let lerp = |a: [Vec3; 2], b: [Vec3; 2], t: f32| [a[0].lerp(b[0], t), a[1].lerp(b[1], t)];
        Some(lerp(
            lerp(p(r, c), p(r, c + 1), cx),
            lerp(p(r + 1, c), p(r + 1, c + 1), cx),
            ry,
        ))
    }
}

impl Sim {
    /// The bolt prims a shot at unit `h` meets now: his posture's, or a
    /// seated rider's where his seat shows him; `None` with no body to hit.
    pub fn body_prims(&self, h: crate::unit::UnitHandle) -> Option<crate::unit::BoltPrims> {
        self.unit(h)?;
        self.hero_body_prims(h.index as usize)
    }

    pub(super) fn hero_body_prims(&self, i: usize) -> Option<crate::unit::BoltPrims> {
        if !self.hero_body_active(i) {
            return None;
        }
        let unit = &self.units[i];
        let mut prims = unit.bolt_prims();
        if let Some(mount) = unit.mount {
            let vehicle = self.vehicle(mount.vehicle)?;
            // His drawn model frame in the world, where it is known.
            let mut frame = None;
            if vehicle.kind != crate::vehicle::VehicleKind::Turret {
                if let Some(frames) = &self.hero_rider_frames {
                    let f = frames
                        .frame(vehicle, mount.seat)
                        .filter(|m| m.is_finite())?;
                    prims.pos = f.w_axis.truncate();
                    let forward = f.z_axis.truncate();
                    prims.yaw = crate::fmath::atan2(-forward.x, -forward.z);
                    frame = Some(f);
                }
            } else {
                frame = self.gunner_frame(vehicle, mount.seat);
            }
            prims.posed =
                frame.and_then(|f| self.seated_points(unit, vehicle, mount.seat, f, &prims));
        }
        Some(prims)
    }

    /// Issue #182: a seated rider's drawn head and torso ([`SeatedBodies`])
    /// in the frame of `prims` (x right, y up, z his facing).
    fn seated_points(
        &self,
        unit: &crate::unit::Unit,
        vehicle: &crate::vehicle::Vehicle,
        seat: u8,
        frame: glam::Mat4,
        prims: &crate::unit::BoltPrims,
    ) -> Option<[Vec3; 2]> {
        let body = self.seated_bodies.as_ref()?.by.get(&(
            unit.team,
            unit.spec.class,
            vehicle.class,
            seat,
        ))?;
        // A hover's pilot is posed by the drive's smoothed throttle and turn,
        // which only the draw keeps: its rest numbers (PROVISIONAL; measure:
        // the drawn pilot's head against the prim at full throttle).
        let hover_pilot = vehicle.kind == crate::vehicle::VehicleKind::Hover
            && vehicle
                .seats
                .get(usize::from(seat))
                .is_some_and(|s| s.kind == crate::vehicle::SeatKind::Pilot);
        let numbers = match self.gun_look(vehicle, seat) {
            Some(look) if !hover_pilot => nine_numbers(look, body.pitch_limits, body.yaw_limits),
            _ if hover_pilot => (0.0, 0.0),
            _ => nine_numbers((0.0, 0.0), body.pitch_limits, body.yaw_limits),
        };
        let (sy, cy) = crate::fmath::sin_cos(prims.yaw);
        let right = Vec3::new(cy, 0.0, -sy);
        let fwd = Vec3::new(-sy, 0.0, -cy);
        let local = |m: Vec3| {
            let d = frame.transform_point3(m) - prims.pos;
            Vec3::new(d.dot(right), d.y, d.dot(fwd))
        };
        let [head, torso] = body.at(numbers)?;
        let out = [local(head), local(torso)];
        out.iter().all(|p| p.is_finite()).then_some(out)
    }

    pub(super) fn hero_body_active(&self, i: usize) -> bool {
        let Some(unit) = self.units.get(i).filter(|u| u.alive) else {
            return false;
        };
        let Some(mount) = unit.mount else {
            return true;
        };
        let Some(vehicle) = self.vehicle(mount.vehicle).filter(|v| v.alive) else {
            return false;
        };
        vehicle
            .seats
            .get(usize::from(mount.seat))
            .is_some_and(|seat| seat.occupant == Some(self.handle(i)))
            && self
                .seat_setup
                .get(vehicle.class as usize)
                .and_then(Option::as_ref)
                .and_then(|setup| setup.body_active.get(usize::from(mount.seat)))
                .copied()
                .unwrap_or(false)
    }
}

#[cfg(test)]
pub(super) fn seat_fixture(
    sim: &mut Sim,
    unit: crate::unit::UnitHandle,
    exposed: bool,
) -> crate::vehicle::VehicleHandle {
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    let vehicle = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(sim.unit(unit).unwrap().team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: sim.unit(unit).unwrap().soldier.pos,
        yaw: 0.0,
        max_health: 500.0,
        health_type: crate::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    sim.seat_setup = vec![Some(SeatSetup {
        radius: 1.0,
        boardable: vec![true],
        body_active: vec![exposed],
        exit_drop: 15.0,
        ai_size: 1,
        guns: vec![],
    })];
    assert!(sim.seat_unit(unit, vehicle, 0));
    vehicle
}

#[cfg(test)]
pub(super) fn pose_fixture(sim: &mut Sim) {
    struct Frames;
    impl HeroRiderFrames for Frames {
        fn frame(&self, vehicle: &crate::vehicle::Vehicle, _: u8) -> Option<glam::Mat4> {
            Some(glam::Mat4::from_rotation_translation(
                vehicle.rot,
                vehicle.pos,
            ))
        }
    }
    sim.hero_rider_frames = Some(std::sync::Arc::new(Frames));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #182: the nine-pose numbers place the gun's look on its
    /// limits; a seat's grid gives back its exact samples at the corners
    /// and the centre and blends between them; one sample is the seat's
    /// at every look.
    #[test]
    fn a_seated_body_is_read_off_its_nine_pose_grid() {
        // Pitch -20..40 deg: level is a third of the way up; the port's
        // pitch is up-negative. No pitch limits: 0.5. Yaw full circle: 0.
        let (yf, pf) = nine_numbers((0.0, 0.0), Some([-20.0, 40.0]), Some([-30.0, 30.0]));
        assert!(
            yf.abs() < 1e-6 && (pf + 1.0 / 3.0).abs() < 1e-5,
            "{yf} {pf}"
        );
        let (_, pf) = nine_numbers((0.0, -40f32.to_radians()), Some([-20.0, 40.0]), None);
        assert!((pf - 1.0).abs() < 1e-5, "{pf}");
        let (yf, _) = nine_numbers((1.0, 0.0), Some([-20.0, 40.0]), Some([-30.0, 30.0]));
        assert_eq!(yf, 1.0, "past the limit: clamped");
        assert_eq!(
            nine_numbers((1.0, 0.3), None, Some([-180.0, 180.0])),
            (0.0, 0.5)
        );

        let n = NINE_GRID;
        let sample = |r: usize, c: usize| {
            let v = Vec3::new(c as f32, r as f32, 0.0);
            [v, v + Vec3::Y]
        };
        let body = SeatedBody {
            pitch_limits: None,
            yaw_limits: None,
            points: (0..n * n).map(|i| sample(i / n, i % n)).collect(),
        };
        let last = n - 1;
        assert_eq!(body.at((-1.0, -1.0)), Some(sample(0, 0)));
        assert_eq!(body.at((-1.0, 1.0)), Some(sample(0, last)));
        assert_eq!(body.at((1.0, -1.0)), Some(sample(last, 0)));
        assert_eq!(body.at((1.0, 1.0)), Some(sample(last, last)));
        assert_eq!(body.at((0.0, 0.0)), Some(sample(n / 2, n / 2)));
        assert_eq!(body.at((5.0, -5.0)), Some(sample(last, 0)), "clamped");
        // Halfway between grid columns 0 and 1 (pf -0.75): x 0.5.
        let mid = body.at((-1.0, -0.75)).unwrap();
        assert!(
            (mid[0] - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-5,
            "{mid:?}"
        );
        let one = SeatedBody {
            points: vec![sample(2, 3)],
            ..body.clone()
        };
        assert_eq!(one.at((0.7, -0.2)), Some(sample(2, 3)));
    }

    #[test]
    fn mounted_hero_targets_require_a_live_matching_exposed_seat() {
        let mut sim = Sim::empty(19);
        let unit = sim
            .spawn(&crate::unit::UnitSpec::trooper(1, glam::Vec3::ZERO))
            .unwrap();
        let i = unit.index as usize;
        assert!(sim.hero_body_active(i));
        let vehicle = seat_fixture(&mut sim, unit, false);
        assert!(!sim.hero_body_active(i));
        sim.seat_setup[0].as_mut().unwrap().body_active[0] = true;
        assert!(sim.hero_body_active(i));
        sim.vehicle_mut(vehicle).unwrap().seats[0].occupant = None;
        assert!(!sim.hero_body_active(i));
        sim.vehicle_mut(vehicle).unwrap().seats[0].occupant = Some(unit);
        sim.vehicle_mut(vehicle).unwrap().alive = false;
        assert!(!sim.hero_body_active(i));
        sim.unit_mut(unit).unwrap().mount = None;
        assert!(sim.hero_body_active(i));
        sim.unit_mut(unit).unwrap().alive = false;
        assert!(!sim.hero_body_active(i));
    }

    #[test]
    fn hero_queries_use_the_seat_frame_without_moving_the_ordinary_body() {
        let mut sim = Sim::empty(20);
        sim.heightfield = Some(crate::sim::tests::flat_ground());
        let unit = sim
            .spawn(&crate::unit::UnitSpec::trooper(1, glam::Vec3::ZERO))
            .unwrap();
        let vehicle = seat_fixture(&mut sim, unit, true);
        pose_fixture(&mut sim);
        sim.vehicle_mut(vehicle).unwrap().pos = glam::Vec3::new(3.0, 5.0, 7.0);
        let prims = sim.hero_body_prims(unit.index as usize).unwrap();
        assert_eq!(prims.pos, glam::Vec3::new(3.0, 5.0, 7.0));
        assert_eq!(sim.unit(unit).unwrap().bolt_prims().pos, glam::Vec3::ZERO);
        let before = sim.hash();
        sim.vehicle_mut(vehicle).unwrap().pos.x += 1.0;
        assert_ne!(sim.hash(), before);
        assert_eq!(sim.hero_body_prims(unit.index as usize).unwrap().pos.x, 4.0);
    }

    #[test]
    fn a_hero_kill_uses_the_visible_seat_position_for_rider_death_bookkeeping() {
        let mut sim = Sim::empty(21);
        sim.heightfield = Some(crate::sim::tests::flat_ground());
        let owner = sim
            .spawn(&crate::unit::UnitSpec::player(0, glam::Vec3::ZERO))
            .unwrap();
        let unit = sim
            .spawn(&crate::unit::UnitSpec::trooper(1, glam::Vec3::splat(100.0)))
            .unwrap();
        let vehicle = seat_fixture(&mut sim, unit, true);
        pose_fixture(&mut sim);
        let position = glam::Vec3::new(2.0, 3.0, -4.0);
        sim.vehicle_mut(vehicle).unwrap().pos = position;
        let mut damage = sim.weapon_classes[0].ordnance.damage_desc(0.0, false);
        damage.damage = 400.0;
        sim.hero_damage(owner.index as usize, unit, damage, glam::Vec3::NEG_Z);
        assert!(
            sim.unit(unit).is_none(),
            "dead handles cannot target a live unit"
        );
        let unit = &sim.units[unit.index as usize];
        assert!(!unit.alive);
        assert!(unit.mount.is_none());
        assert_eq!(unit.soldier.pos, position);
        assert!(sim.vehicle(vehicle).unwrap().seats[0].occupant.is_none());
    }
}
