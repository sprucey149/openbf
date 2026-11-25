//! The line a seat's next shot leaves on, and the point it reaches. The
//! shot loop ([`Sim::drive_seat_gun`]) and the read-only sight query
//! ([`Sim::seat_sight_point`], the headset's world-space reticle) take
//! each barrel's origin and frame from the same [`Barrels::line`], so the
//! sight follows the rounds. Spread, drop and a missile's homing are the
//! round's own and are not predicted.

use super::{barrel_aim, ring_aim};
use crate::sim::state::Sim;
use crate::unit::UnitHandle;
use crate::vehicle::gun::{self, GunSetup, GunWeapon};
use crate::vehicle::{VehicleHandle, VehicleKind};
use glam::{Quat, Vec3};

/// OURS: ring barrels within 5 deg of the next one's line share the sight,
/// which then stands still while a twin or quad mount alternates barrels.
/// measure: compare the sight with the original's centre reticle over
/// each multi-barrel seat's bolt pattern.
const PAIRED_COS: f32 = 0.996_194_7;

/// What every barrel of one seat shares on this tick.
pub(super) struct Barrels<'a> {
    pub gun: &'a GunSetup,
    pub pos: Vec3,
    pub rot: Quat,
    pub look: (f32, f32),
    pub rel: (f32, f32),
    pub aim: (f32, f32),
    /// The aim point in the model frame where a barrel aims on its own.
    pub aimed: Option<Vec3>,
    /// The camera ray's aim point, world.
    pub target: Vec3,
    /// A flyer pilot's guns.
    pub fixed: bool,
}

/// One shot's start and frame, before spread.
pub(super) struct BarrelLine {
    pub point: Option<usize>,
    pub origin: Vec3,
    pub frame: (Vec3, Vec3, Vec3),
}

impl Barrels<'_> {
    /// Weapon `k`'s shot of ring turn `turn`.
    pub fn line(&self, k: usize, w: &GunWeapon, turn: u64) -> BarrelLine {
        let g = self.gun;
        let (pos, rot) = (self.pos, self.rot);
        let point =
            (!w.fire_points.is_empty()).then(|| (turn % w.fire_points.len() as u64) as usize);
        // A barrel on an aimer of its own (the Spider Walker's two
        // turrets, a hover pilot's side guns) turns about that
        // aimer's node toward the aim point, inside its limits, and
        // fires along its own line (issue #182: from the muzzle a
        // near target fell behind it and the shot went up and back).
        let own_aim = point.filter(|_| !self.fixed).and_then(|p| {
            if g.turn.is_some() {
                barrel_aim(g, w, p, self.look, self.rel, self.aimed)
            } else {
                let target = rot.inverse() * (self.target - pos);
                GunSetup::line_aim(w, p, target).map(|(aim, _)| aim)
            }
        });
        let origin = point.map_or_else(
            || pos + rot * g.muzzle(self.aim),
            |p| pos + rot * g.shot_point(k, Some(p), own_aim.unwrap_or(self.aim), self.fixed),
        );
        let fwd = (rot * gun::dir(self.aim.0, self.aim.1)).normalize();
        let frame = if let Some(aim) = own_aim {
            let d = (rot * gun::dir(aim.0, aim.1)).normalize();
            let right = d.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
            (d, right, right.cross(d).normalize())
        } else if g.turn.is_none()
            && !w.fire_points.is_empty()
            && (!self.fixed || point.is_some_and(|p| w.fire_limits.get(p).is_some()))
        {
            ring_aim(
                g,
                w.fire_limits.get(point.unwrap_or(0)),
                rot,
                origin,
                self.target,
                fwd,
            )
        } else {
            let right = fwd.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
            (fwd, right, right.cross(fwd).normalize())
        };
        BarrelLine {
            point,
            origin,
            frame,
        }
    }

    /// The line the sight follows: weapon `k`'s next barrel from `turn`,
    /// averaged with the ring's other barrels that fire alongside it.
    fn sight(&self, k: usize, w: &GunWeapon, turn: u64) -> (Vec3, Vec3) {
        let next = self.line(k, w, turn);
        let ahead = next.frame.0;
        let (mut origin, mut dir, mut n) = (next.origin, ahead, 1.0);
        for t in 1..w.fire_points.len() as u64 {
            let other = self.line(k, w, turn.wrapping_add(t));
            if other.frame.0.dot(ahead) >= PAIRED_COS {
                origin += other.origin;
                dir += other.frame.0;
                n += 1.0;
            }
        }
        (origin / n, dir.try_normalize().unwrap_or(ahead))
    }
}

impl Sim {
    /// The line `u`'s seat fires its first weapon along now, as
    /// (origin, direction), or `None` before the seat is armed and for a
    /// seat with no weapon. Reads only.
    pub fn seat_sight_ray(&self, u: UnitHandle) -> Option<(Vec3, Vec3)> {
        let m = self.unit(u)?.mount?;
        let g = self.seat_gun_setup(u)?;
        let v = self.vehicle(m.vehicle)?;
        let st = v.guns.get(usize::from(m.seat))?.as_ref()?;
        let w = g.weapons.first()?;
        let (pos, rot) = (v.pos, v.rot);
        let zoomed = st.zoom.is_some();
        let eye = pos + rot * g.ray_eye(st.look, zoomed);
        let view = rot * gun::dir(st.look.0, st.look.1);
        let eye =
            self.chase_ray_origin(m, &g, Some(u.index as usize), st.look, zoomed, (eye, view));
        let target = self.aim_point(eye, view, &self.riders(m.vehicle), m.vehicle);
        let barrels = Barrels {
            gun: &g,
            pos,
            rot,
            look: st.look,
            rel: st.rel,
            aim: st.aim,
            aimed: st.target.map(|_| rot.inverse() * (target - pos)),
            target,
            fixed: g.pilot && v.kind == VehicleKind::Flyer,
        };
        Some(barrels.sight(0, w, st.weapons.first()?.turns()))
    }

    /// Where `u`'s seat sight lands: the first surface or soldier along
    /// [`Sim::seat_sight_ray`], else the aim ray's end.
    pub fn seat_sight_point(&self, u: UnitHandle) -> Option<Vec3> {
        let (origin, dir) = self.seat_sight_ray(u)?;
        let own = self.unit(u)?.mount?.vehicle;
        Some(self.aim_point(origin, dir, &self.riders(own), own))
    }

    /// Where a standing droideka's next shot lands: along the player's own
    /// shot frame from the barrel next to fire. `None` in the ball and while
    /// it folds, when it cannot fire.
    pub fn droideka_sight_point(&self, u: UnitHandle) -> Option<Vec3> {
        let unit = self.unit(u)?;
        if unit.droideka.as_ref()?.no_fire() {
            return None;
        }
        let (origin, (dir, _, _)) = self.human_shot(u.index as usize);
        Some(self.sight_point(u, origin, dir))
    }

    /// Where a shot along `dir` from `origin` lands for `shooter` on foot
    /// or flying a remote: the first surface or other soldier, else the aim
    /// ray's end.
    pub fn sight_point(&self, shooter: UnitHandle, origin: Vec3, dir: Vec3) -> Vec3 {
        self.first_along(origin, dir, &[shooter], None)
    }

    fn riders(&self, v: VehicleHandle) -> Vec<UnitHandle> {
        self.vehicle(v).map_or_else(Vec::new, |v| {
            v.seats.iter().filter_map(|s| s.occupant).collect()
        })
    }
}

#[cfg(test)]
mod tests;
