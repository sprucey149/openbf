//! Ordnance that flies itself (card UORD): missiles steer
//! (`crate::missile`, `analysis/re-ordnance-kinds.md` §4) and a launcher's
//! dwell lock decides whether one leaves guided (§5). Shells' gravity is the
//! same per-tick flight path, so [`Sim::probe_flight`] is shared.

use super::input::AimTarget;
use super::state::Sim;
use crate::missile::MissileTarget;
use crate::projectile::{Bolt, Hit};
use crate::unit::Controller;
use glam::Vec3;

impl Sim {
    /// The map's reaction to bolt `k`'s flight this tick: the terrain, the
    /// water and the world's bodies along `pos -> pos + dir x speed x dt`.
    /// A shell and a missile ask this every tick - neither has a predictive
    /// ray (`re-ordnance-kinds.md` §4.3, §6) - while a bolt asks it once, at
    /// the press, in [`Bolt::fire`]. Soldiers and hulls are
    /// [`Sim::sweep_bolts`].
    pub(super) fn probe_flight(&mut self, k: usize, dt: f32) {
        let b = &self.bolts[k];
        if b.hit != Hit::None {
            return;
        }
        let speed = b.vel.length();
        if speed <= 1e-4 {
            return;
        }
        let (pos, dir, step) = (b.pos, b.vel / speed, dt.min(b.remaining.max(0.0)));
        let who = b.shooter();
        let hulls = b.hulls;
        // `Bolt::fire`'s rays: a rider's round passes his own building.
        let probe = Bolt::fire(
            pos,
            dir,
            speed,
            step,
            who,
            self.heightfield.as_ref(),
            self.water_plane(),
            &self.collision,
            hulls,
        );
        if probe.hit != Hit::None {
            let b = &mut self.bolts[k];
            (b.hit, b.end, b.remaining) = (probe.hit, probe.end, probe.remaining);
        }
    }

    /// Where a missile's target is this tick (`re-ordnance-kinds.md` §4.4):
    /// a soldier's feet + the stance's 1.4 / 0.6 / 0.2 m
    /// ([`crate::unit::Unit::aim_point`]), a vehicle's collision-sphere
    /// centre in its frame. `None` when the target is gone - the missile drops
    /// it (the handle test is the only one in flight, §4.3). A round a human
    /// locked on a vehicle's weak part steers to that part instead
    /// ([`Sim::fly_missiles`], §5.1).
    pub(super) fn missile_aim(&self, t: MissileTarget) -> Option<Vec3> {
        match t {
            MissileTarget::Unit(h) => self.unit(h).map(|u| u.aim_point()),
            MissileTarget::Vehicle(h) => self.aim_candidate(AimTarget::Vehicle(h)).map(|c| c.aim),
            MissileTarget::Turret(id) => self
                .turrets
                .iter()
                .find(|t| t.id == id && t.alive())
                .map(|t| t.centre()),
            MissileTarget::Station(s) => self
                .stations
                .get(usize::from(s))
                .filter(|st| !st.destroyed)
                .map(|st| st.sphere_centre),
        }
    }

    fn missile_guidance_aim(&self, t: MissileTarget, pos: Vec3, dir: Vec3) -> Option<Vec3> {
        let fallback = self.missile_aim(t)?;
        let (crate::soldier::GroundRules::Bf2, MissileTarget::Vehicle(h)) = (self.ground_rules, t)
        else {
            return Some(fallback);
        };
        let v = self.vehicle(h)?;
        let Some(points) = self
            .missile_target_points
            .get(v.class as usize)
            .filter(|p| !p.is_empty())
        else {
            return Some(fallback);
        };
        let mut aim = v.pos;
        let mut best = 0.0;
        for local in points {
            let point = v.pos + v.rot * *local;
            let score = (point - pos).normalize_or_zero().dot(dir);
            if score > best {
                best = score;
                aim = point;
            }
        }
        Some(aim)
    }

    /// Stage 5's missiles (card UORD): each steers toward its target's point
    /// (`re-ordnance-kinds.md` §4.3), then meets the world on this tick's
    /// chord like a shell; soldiers and hulls are the sweep that follows.
    pub(super) fn fly_missiles(&mut self, dt: f32) {
        for k in 0..self.bolts.len() {
            let Some(m) = self.bolts[k].missile else {
                continue;
            };
            if self.bolts[k].hit != Hit::None {
                continue;
            }
            let Some(c) = self.weapon_classes[self.bolts[k].weapon_class]
                .ordnance
                .missile
            else {
                continue;
            };
            // A round sent at a weak part follows it while the hull has
            // it, else the vehicle's own point.
            let part = match (m.target, m.part) {
                (Some(MissileTarget::Vehicle(h)), Some(k)) => self.weak_part_point(h, k),
                _ => None,
            };
            let aim = m
                .target
                .and_then(|t| self.missile_guidance_aim(t, self.bolts[k].pos, m.dir))
                .map(|p| part.unwrap_or(p));
            let b = &mut self.bolts[k];
            b.guide(&c, aim, dt, &mut self.lcg);
        }
        for k in 0..self.bolts.len() {
            if self.bolts[k].missile.is_some() {
                self.probe_flight(k, dt);
            }
        }
    }

    /// The dwell lock (`re-ordnance-kinds.md` §5): a launcher with
    /// `LockTime > 0` holds a candidate target and counts the seconds; at the
    /// class's limit the next missile it fires homes. A human's candidate is
    /// the on-foot lock ([`Sim::tick_foot_lock`]) on every ground map and the
    /// magnet (`re-aim-assist.md` §3) in space rounds; a bot's is its combat
    /// target. An on-foot lock past 100 m fills slower. A new target
    /// restarts the clock, and [`Sim::soldier_shots`] consumes the lock.
    pub(super) fn tick_locks(&mut self, dt: f32) {
        for i in 0..self.units.len() {
            if !self.units[i].alive {
                continue;
            }
            let class = self.units[i].weapon_class;
            let c = &self.weapon_classes[class];
            let launcher = c.lock_time > 0.0 && c.ordnance.missile.is_some();
            if !launcher {
                let st = &mut self.units[i].weapon;
                st.lock_target = None;
                st.lock_held = 0.0;
                continue;
            }
            let controller = self.units[i].controller;
            let ai_target = self.units[i].ai.target;
            // A far on-foot lock counts slower (`foot_dwell_scale`).
            let mut scale = 1.0;
            let candidate = match controller {
                Controller::Player { slot } if self.foot_lock_rules() => {
                    self.foot_dwell_candidate(slot as usize, i).map(|(t, k)| {
                        scale = k;
                        t
                    })
                }
                Controller::Player { slot } => self
                    .magnet
                    .get(slot as usize)
                    .copied()
                    .flatten()
                    .and_then(|t| match t {
                        AimTarget::Unit(h) => Some(MissileTarget::Unit(h)),
                        AimTarget::Vehicle(h) => Some(MissileTarget::Vehicle(h)),
                        AimTarget::Turret(id) => Some(MissileTarget::Turret(id)),
                        AimTarget::Droid(_) | AimTarget::Station(_) => None,
                    }),
                _ => ai_target.and_then(|f| f.unit()).map(MissileTarget::Unit),
            };
            // A dead handle is no target (BF1's and BF2's one in-flight test).
            let candidate = candidate.filter(|t| self.missile_aim(*t).is_some());
            let st = &mut self.units[i].weapon;
            if candidate != st.lock_target {
                st.lock_target = candidate;
                st.lock_held = 0.0;
            } else if candidate.is_some() {
                st.lock_held += dt / scale;
            }
        }
    }
}

#[cfg(test)]
mod tests;
