//! Walker/hover contact priority and the walker's seven-and-a-half-second hold.
//! Priority: BF2 FUN_002a8c30:229192-229313, BF1 FUN_00236e58:150955-151069.
//! Writers: BF2 FUN_0025c2b0:177324-177337, BF1 FUN_00217d00:129003-129021.
//! OURS, PROVISIONAL: stable sequential hull sweeps stop translation; nearest-side
//! bypass resolves ties/stationary blockage without inventing a yield timer.
//! measure: PS2 opposing/crossing/stationary contacts and original pair response.

use super::Sim;
use crate::unit::Controller;
use crate::vehicle::{Vehicle, VehicleHandle, VehicleKind};
use glam::Vec3;

/// Exact fixed-tick representation of the PS2's 7.5 seconds.
const HOLD_TICKS: u64 = crate::TICK_HZ as u64 * 15 / 2;

fn yields(a: &Vehicle, b: &Vehicle, ra: f32, rb: f32, delta: Vec3) -> bool {
    if !matches!(b.kind, VehicleKind::Walker | VehicleKind::Hover) || ra > 3.0 * rb {
        return false;
    }
    if rb > 3.0 * ra {
        return true;
    }
    let horizontal = |v: Vec3| v.x * v.x + v.z * v.z;
    if horizontal(a.vel) <= 0.0001 || horizontal(b.vel) <= 0.0001 {
        return false;
    }
    let n = delta.normalize_or_zero();
    -n.dot(a.vel.normalize_or_zero()) < n.dot(b.vel.normalize_or_zero())
}

impl Sim {
    pub(crate) fn traffic_radius(&self, h: VehicleHandle) -> f32 {
        let Some(v) = self.vehicle(h) else { return 0.0 };
        // The model sphere when installed; legacy fixtures have only a seat radius.
        self.blast_setup
            .get(v.class as usize)
            .and_then(|s| s.as_ref())
            .map(|s| s.sphere_radius)
            .or_else(|| {
                self.seat_setup
                    .get(v.class as usize)
                    .and_then(|s| s.as_ref())
                    .map(|s| s.radius)
            })
            .unwrap_or(0.0)
    }

    /// Only ground pairs involving a walker belong to this lifecycle.
    fn traffic_pair(&self, a: VehicleHandle, b: VehicleHandle) -> bool {
        let (Some(a), Some(b)) = (self.vehicle(a), self.vehicle(b)) else {
            return false;
        };
        matches!(a.kind, VehicleKind::Walker | VehicleKind::Hover)
            && matches!(b.kind, VehicleKind::Walker | VehicleKind::Hover)
            && (a.kind == VehicleKind::Walker || b.kind == VehicleKind::Walker)
    }

    fn ground_ai_pilot(&self, h: VehicleHandle) -> bool {
        self.vehicle(h)
            .and_then(|v| v.pilot_seat())
            .and_then(|s| s.occupant)
            .and_then(|h| self.unit(h))
            .is_some_and(|u| u.controller == Controller::Ai)
    }

    pub(crate) fn ground_traffic(&mut self, i: usize, from: Vec3) {
        let h = VehicleHandle {
            index: i as u32,
            gen: self.vehicles[i].gen,
        };
        let step = self.vehicles[i].pos - from;
        if step.length_squared() < 1.0e-12 {
            self.sync_hull(i);
            return;
        }
        // Cast twice only on contact: first find the nearest fraction, then
        // apply every tied priority in stable slot order without a scratch Vec.
        let contact = |sim: &Self, j: usize| {
            let other = VehicleHandle {
                index: j as u32,
                gen: sim.vehicles[j].gen,
            };
            if i == j || !sim.traffic_pair(h, other) {
                return None;
            }
            sim.collision
                .ground_hull_cast(h, other, step)
                .map(|(fraction, _)| (fraction, other))
        };
        let best = (0..self.vehicles.len())
            .filter_map(|j| contact(self, j).map(|(fraction, _)| fraction))
            .min_by(f32::total_cmp);
        let ai = self.ground_ai_pilot(h);
        if let Some(first) = best {
            for j in 0..self.vehicles.len() {
                let Some((fraction, other)) = contact(self, j) else {
                    continue;
                };
                if fraction > first {
                    continue;
                }
                // Decide BOTH priorities before altering either velocity.
                let (a, b) = (&self.vehicles[i], &self.vehicles[j]);
                let delta = b.pos - (from + step * first);
                let (ra, rb) = (self.traffic_radius(h), self.traffic_radius(other));
                let ya = ai && a.kind == VehicleKind::Walker && yields(a, b, ra, rb, delta);
                let yb = self.ground_ai_pilot(other)
                    && b.kind == VehicleKind::Walker
                    && yields(b, a, rb, ra, -delta);
                if ya {
                    self.vehicles[i].yield_until = self.tick + HOLD_TICKS;
                }
                if yb {
                    self.vehicles[j].yield_until = self.tick + HOLD_TICKS;
                }
            }
            // This port traffic response is AI recovery. Human and unpiloted
            // vehicles retain the existing locomotion/contact response.
            if ai {
                let v = &mut self.vehicles[i];
                // Retain a tiny spatial gap so contact round-off cannot alternate
                // between penetration and a missed cast. Uses the existing tolerance.
                let safe = (first - crate::collision::SWEEP_CONTACT_DIST / step.length()).max(0.0);
                v.pos = from + step * safe;
                v.vel.x = 0.0;
                v.vel.z = 0.0;
            }
        }
        self.sync_hull(i);
    }

    /// OURS: bypass an occupied route on the nearer side, right on a tie. No priority tie
    /// starts a hold. The authored route remains intact and resumes when clear.
    /// measure: original-game tied/stationary congestion trajectories.
    pub(crate) fn ground_traffic_goal(&self, h: VehicleHandle, goal: Vec3) -> Vec3 {
        let Some(v) = self.vehicle(h) else {
            return goal;
        };
        if !self.ground_ai_pilot(h) {
            return goal;
        }
        let flat = Vec3::new(goal.x - v.pos.x, 0.0, goal.z - v.pos.z);
        let distance = flat.length();
        if distance < 1.0e-4 {
            return goal;
        }
        let dir = flat / distance;
        let reach = distance.min(crate::ai::walker_drive::ROUTE_REACH);
        let mut nearest: Option<(f32, VehicleHandle)> = None;
        for (other, o) in self.alive_vehicles() {
            if h == other || !self.traffic_pair(h, other) || (o.pos - v.pos).dot(dir) <= 0.0 {
                continue;
            }
            if let Some((fraction, _)) = self.collision.ground_hull_cast(h, other, dir * reach) {
                if nearest.is_none_or(|(t, _)| fraction < t) {
                    nearest = Some((fraction, other));
                }
            }
        }
        let Some((_, other)) = nearest else {
            return goal;
        };
        let right = dir.cross(Vec3::Y);
        // A point exactly on the inflated hull makes the chord from an
        // existing contact cut through it. One own-radius of clearance keeps
        // the bypass target outside that ring, so the winner can separate.
        let own = self.traffic_radius(h);
        let clearance = own + self.traffic_radius(other) + own;
        let o = self.vehicle(other).expect("live candidate");
        // Use the side already occupied by this driver; crossing traffic can
        // approach an obstruction from its left. Forcing it across to the right
        // cuts through the hull. Exact head-on/collinear ties take the right.
        let side = if (v.pos - o.pos).dot(right) < -1.0e-4 {
            -1.0
        } else {
            1.0
        };
        let target = Vec3::new(o.pos.x, v.pos.y, o.pos.z) + right * (side * clearance);
        let away = target - v.pos;
        // This is a steering direction, not an arrival node. Keep it outside
        // the walker's 4 m arrival branch: close bypass targets let the plain
        // law walk while facing sideways (and reverse the authored turn factor).
        v.pos + away.normalize_or_zero() * away.length().max(crate::ai::walker_drive::ROUTE_REACH)
    }
}

#[cfg(test)]
mod tests;
