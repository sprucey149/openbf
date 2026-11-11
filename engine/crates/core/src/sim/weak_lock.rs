//! A launcher's lock goes to the vehicle's weak spot, and a strike on a weak
//! spot deals one factor (`analysis/re-ordnance-kinds.md` §5.1). A weak part
//! is a hull body whose class gives it a direct-hit factor over 1
//! (`HitLocation`: BF2's x4 critical prims, and BF1's through the port's
//! table). While a human's on-foot lock holds a vehicle, the lock point is
//! the weak part he can see that is nearest his reticle; with none in sight
//! it stays the hull centre. A rocket fired on a full lock steers to that
//! part.
//!
//! OURS: neither original aims a lock at a critical spot (BF2's launcher
//! candidate carries no body part, `FUN_0033a658` B2:331689-331750; BF1's
//! target point is a targetable body or the sphere centre, `FUN_001e2500`
//! B1:91606-91683). The hit is the originals' rule with the port's factor:
//! any round that strikes a weak part deals its vehicle class's factor on a
//! ground map ([`HEAVY_WALKER_WEAK_SPOT_FACTOR`] x4 on the AT-TE, the AT-AT
//! and the spider walker, [`GROUND_WEAK_SPOT_FACTOR`] x2 on the rest),
//! locked or not, whatever the ordnance; a hit elsewhere on the hull is
//! plain ([`struck_factor`]). Blast damage is never located on a
//! prim (both originals' radius damage takes the no-prim factor, 1.0). Bots
//! never lock a vehicle and get no factor over 1 on a ground round
//! (`Sim::hit_scale`); space rounds keep the centre and their data's
//! factors.
use super::input::AimTarget;
use super::lock_on::LockTarget;
use super::state::Sim;
use crate::collision::{Body, HullFilter, Layers, Shape};
use crate::perception;
use crate::vehicle::VehicleHandle;
use glam::Vec3;

/// OURS: the factor of a strike on a vehicle's weak part on a ground map,
/// for every weak spot of every vehicle in both games but the three big
/// walkers' ([`HEAVY_WALKER_WEAK_SPOT_FACTOR`]). The maintainer 2026-11-10:
/// "If a rocket hits the weak spot it should do the increased damage,
/// regardless of it being a "lock" or something. Even a grenade or regular
/// bullet in the weak spot can do the increased damage." and then "ATTE,
/// atat, and spider walker: make weak spot hits x4. Everything else x2 and
/// that seems good." The originals author x4 everywhere (BF2's 15 classes;
/// BF1 none, the port gives its armour BF2's). It is the default of
/// `Sim::weak_spot_factor`. measure: maintainer playtest of rockets to kill
/// (`re-ordnance-kinds.md` §5.1's table).
pub const GROUND_WEAK_SPOT_FACTOR: f32 = 2.0;

/// OURS: the factor of a strike on the weak part of an AT-TE, an AT-AT or a
/// spider walker on a ground map, every class of them in both games (the
/// game layer reads the class chain, `game::vehicle_class::weak_spot_factor`).
/// The originals' own x4, kept for these three only (the maintainer
/// 2026-11-10, [`GROUND_WEAK_SPOT_FACTOR`]). measure: maintainer playtest of
/// rockets to kill (`re-ordnance-kinds.md` §5.1's table).
pub const HEAVY_WALKER_WEAK_SPOT_FACTOR: f32 = 4.0;

/// A weak part: a round can strike it and it scales the hit up.
fn weak(b: &Body) -> bool {
    b.layers.contains(Layers::ORDNANCE) && b.hit_mult > 1.0
}

/// The factor a round deals that strikes a vehicle's hull body with data
/// factor `hit_mult`: on a ground map (`ground` is the vehicle class's
/// weak-spot factor, `Sim::weak_spot_factor`) a weak part's is `ground`,
/// any other body keeps its own (1, or BF1's x0); a space round (`ground`
/// `None`) keeps the data's.
pub(super) fn struck_factor(hit_mult: f32, ground: Option<f32>) -> f32 {
    match ground {
        Some(f) if hit_mult > 1.0 => f,
        _ => hit_mult,
    }
}

/// A class's row of `Sim::weak_spot_factor`, else the default.
pub(super) fn class_factor(rows: &[f32], class: u32) -> f32 {
    rows.get(class as usize)
        .copied()
        .unwrap_or(GROUND_WEAK_SPOT_FACTOR)
}

/// The middle of a hull body: a prim is centred on its own origin.
fn centre(b: &Body) -> Vec3 {
    match b.shape {
        Shape::Mesh(_) => (b.aabb.min + b.aabb.max) * 0.5,
        _ => b.to_world.transform_point3(Vec3::ZERO),
    }
}

impl Sim {
    /// The factor a strike on a weak part of a vehicle of class `class`
    /// deals on a ground map: its [`Sim::weak_spot_factor`] row, else
    /// [`GROUND_WEAK_SPOT_FACTOR`].
    pub fn ground_weak_spot_factor(&self, class: u32) -> f32 {
        class_factor(&self.weak_spot_factor, class)
    }

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
