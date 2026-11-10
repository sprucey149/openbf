//! A launcher's lock goes to the vehicle's weak spot
//! (`analysis/re-ordnance-kinds.md` §5.1). A weak part is a hull body whose
//! class gives it a direct-hit factor over 1 (`HitLocation`, the x4 critical
//! prims). While a human's on-foot lock holds a vehicle, the lock point is
//! the weak part he can see that is nearest his reticle; with none in sight
//! it stays the hull centre. A rocket fired on a full lock steers to that
//! part, so a hit there is the critical hit.
//!
//! OURS: neither original aims a lock at a critical spot (BF2's launcher
//! candidate carries no body part, `FUN_0033a658` B2:331689-331750; BF1's
//! target point is a targetable body or the sphere centre, `FUN_001e2500`
//! B1:91606-91683). A rocket sent at a part takes the part's factor
//! wherever it meets that hull: steering alone left one part lock in ten
//! on the plate beside the part (the survey in
//! `game/tests/suite/rocket_weakspot.rs`). Any other round keeps the struck
//! prim's factor. Bots never lock a vehicle and space rounds keep the
//! centre. measure: maintainer playtest of the weak-part lock (B7).

use super::input::AimTarget;
use super::lock_on::LockTarget;
use super::state::Sim;
use crate::collision::{Body, CollisionWorld, HullFilter, Layers, Shape};
use crate::missile::MissileTarget;
use crate::perception;
use crate::projectile::Bolt;
use crate::vehicle::VehicleHandle;
use glam::Vec3;

/// A weak part: a round can strike it and it scales the hit up.
fn weak(b: &Body) -> bool {
    b.layers.contains(Layers::ORDNANCE) && b.hit_mult > 1.0
}

/// The middle of a hull body: a prim is centred on its own origin.
fn centre(b: &Body) -> Vec3 {
    match b.shape {
        Shape::Mesh(_) => (b.aabb.min + b.aabb.max) * 0.5,
        _ => b.to_world.transform_point3(Vec3::ZERO),
    }
}

/// The factor of the weak part round `b` was sent at, when `v` is the
/// vehicle it locked and the hull still has the part.
pub(super) fn part_factor(world: &CollisionWorld, b: &Bolt, v: VehicleHandle) -> Option<f32> {
    let m = b.missile.as_ref()?;
    if m.target != Some(MissileTarget::Vehicle(v)) {
        return None;
    }
    let part = world.hull(v).get(usize::from(m.part?))?;
    weak(part).then_some(part.hit_mult)
}

impl Sim {
    /// Where weak part `k` of `vh`'s hull is now; `None` when the hull has
    /// no such part.
    pub(super) fn weak_part_point(&self, vh: VehicleHandle, k: u16) -> Option<Vec3> {
        let b = self.collision.hull(vh).get(usize::from(k))?;
        weak(b).then(|| centre(b))
    }

    /// The weak part of `vh` in sight from `eye` that is nearest the aim
    /// `dir`: its index in the hull and its centre. In sight means the line
    /// from the eye to the part's centre meets a weak part before any other
    /// body of that hull, and nothing else stands in it. `None` when no
    /// part passes; the caller keeps the hull centre.
    pub(super) fn weak_part(&self, vh: VehicleHandle, eye: Vec3, dir: Vec3) -> Option<(u16, Vec3)> {
        let building = self.vehicle(vh)?.instance;
        let mut best: Option<(f32, u16, Vec3)> = None;
        for (k, b) in self.collision.hull(vh).iter().enumerate() {
            if !weak(b) {
                continue;
            }
            let c = centre(b);
            let off = c - eye;
            let len = off.length();
            if len <= 1e-4 {
                continue;
            }
            let to = off / len;
            let dot = to.dot(dir);
            if best.is_some_and(|(d, ..)| d >= dot) {
                continue;
            }
            let first =
                self.collision
                    .ray_with(eye, to, len, Layers::ORDNANCE, HullFilter::Only(vh));
            if !first.is_some_and(|h| h.hit_mult > 1.0) {
                continue;
            }
            let clear = perception::los_seat(
                self,
                eye,
                c,
                HullFilter::Except([None, Some(vh)]),
                [None, building],
            );
            if clear {
                best = Some((dot, k as u16, c));
            }
        }
        best.map(|(_, k, c)| (k, c))
    }

    /// The weak part unit `i`'s lock on `t` goes to: `t` is a vehicle and
    /// one of its weak parts is in sight along his aim.
    pub(super) fn foot_weak_part(&self, t: LockTarget, i: usize) -> Option<(u16, Vec3)> {
        let LockTarget::Aim(AimTarget::Vehicle(vh)) = t else {
            return None;
        };
        let (eye, dir) = self.units[i].aim_ray(self.ground_rules, self.views());
        self.weak_part(vh, eye, dir)
    }
}

#[cfg(test)]
mod tests;
