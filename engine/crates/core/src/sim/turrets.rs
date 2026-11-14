//! The auto-turret in the sim: throw and `MaxItems` replacement, the fall, the
//! stationary controller's target choice, aim and fire, and its end and
//! explosion. Follows `analysis/re-portable-turret.md` (card ATUR); friendly-fire
//! checks are card TKFIX.

use super::mines::oldest_beyond;
use super::{Event, Sim};
use crate::autoturret::{
    Aim, Turret, TurretBody, TurretClass, TurretEnd, FIGHT_MARGIN, LAND_NY, LOOK_EVERY,
    LOOK_SPREAD, THINK, VIEW,
};
use crate::collision::HullFilter;
use crate::fmath::FloatExt;
use crate::projectile::Shooter;
use crate::unit::UnitHandle;
use crate::weapon::{FireInput, HealthType};
use glam::Vec3;

/// [`Sim::turret_supported`]'s ray: from this far above the origin, to this
/// far below it.
const SUPPORT_RISE: f32 = 0.2;
const SUPPORT_GAP: f32 = 0.1;

impl Sim {
    /// Soldier contact with the authored `p_-so_sphere` (radius 0.25 on
    /// boneroot, mask 17: soldiers + ordnance; re-portable-turret §10).
    /// As with world walls, resolve horizontally in half-radius substeps
    /// and leave vertical motion to the ground pass. A flying dispenser
    /// skips soldier contacts (BF2 FUN_0022d5b0); only landed shells block.
    pub(super) fn push_turrets(&mut self, i: usize, from: crate::collision::Capsule) {
        if !self.turrets.iter().any(|t| t.alive() && t.landed) {
            return;
        }
        let to = self.units[i].capsule();
        let delta = to.base - from.base;
        let steps = (delta.length() / (to.radius * 0.5)).ceil().fmax(1.0) as usize;
        let mut push = Vec3::ZERO;
        for step in 0..steps {
            let mut at = from.base + delta * ((step + 1) as f32 / steps as f32) + push;
            for t in self.turrets.iter().filter(|t| t.alive() && t.landed) {
                let centre = t.centre();
                let r = to.radius.fmin(to.height * 0.5);
                let lo = at.y + r;
                let hi = lo + (to.height - 2.0 * r).fmax(0.0);
                let y = centre.y.clamp(lo, hi);
                let dy = centre.y - y;
                let radius = r + crate::autoturret::RADIUS;
                if dy.abs() >= radius {
                    continue;
                }
                let reach = (radius * radius - dy * dy).sqrt();
                let away = (at - centre).with_y(0.0);
                let len = away.length();
                if len < reach {
                    let normal = away.try_normalize().unwrap_or_else(|| {
                        (from.base - centre)
                            .with_y(0.0)
                            .try_normalize()
                            .unwrap_or(Vec3::X)
                    });
                    let correction = normal * (reach - len);
                    at += correction;
                    push += correction;
                }
            }
        }
        self.units[i].soldier.pos += push;
        self.units[i].wall_push += push;
        if let Some(normal) = push.try_normalize() {
            let vel = &mut self.units[i].soldier.vel;
            *vel -= normal * vel.dot(normal).fmin(0.0);
        }
    }

    pub fn turret_of(&self, h: UnitHandle, weapon: usize) -> Option<&Turret> {
        self.turrets
            .iter()
            .find(|t| t.owner == h && t.weapon_class == weapon)
    }

    /// The standing auto-turret `id`, as a target.
    pub fn live_turret(&self, id: u64) -> Option<&Turret> {
        self.turrets.iter().find(|t| t.id == id && t.alive())
    }

    pub fn turret_class(&self, t: &Turret) -> Option<&TurretClass> {
        self.weapon_classes[t.weapon_class]
            .turret
            .as_ref()
            .map(|d| &d.item)
    }

    pub(super) fn place_turret(&mut self, i: usize, weapon: usize, charge: f32) {
        let Some(d) = self.weapon_classes[weapon].turret.as_deref() else {
            return;
        };
        let (speed, max) = (d.speed(charge), d.max_items);
        let owner = self.handle(i);
        if let Some(k) = oldest_beyond(&self.turrets, max, |t| {
            t.owner == owner && t.weapon_class == weapon
        }) {
            let old = self.turrets.remove(k);
            self.turret_gone(&old, TurretEnd::Replaced);
        }
        let (pos, vel) = self.throw_from(i, weapon, speed);
        let u = &self.units[i];
        self.next_turret_id = self
            .next_turret_id
            .checked_add(1)
            .expect("turret identity exhausted");
        let c = &self.weapon_classes[weapon].turret.as_deref().unwrap().item;
        let turret = Turret::new(
            self.next_turret_id,
            owner,
            u.team,
            weapon,
            c,
            pos,
            vel,
            u.soldier.yaw,
        );
        self.turrets.push(turret);
        self.events.push(Event::Throw {
            unit: owner,
            weapon_class: weapon as u32,
            origin: pos,
        });
    }

    pub(super) fn tick_turrets(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.turrets.len() {
            let mut t = self.turrets[k].clone();
            let Some((body, gun)) = self.turret_class(&t).map(|c| (c.body, c.gun_index)) else {
                self.turrets.remove(k);
                continue;
            };
            let mut end = t.grow_old(&body, dt);
            if end.is_none() && t.landed && !self.turret_supported(&t) {
                // What it stood on moved or went: it falls again.
                t.landed = false;
                t.vel = Vec3::ZERO;
                t.target = None;
                t.fighting = false;
            }
            if end.is_none() && !t.landed {
                self.turret_fall(&mut t, &body, dt);
            }
            if end.is_none() && t.pos.y < crate::sticky::WORLD_FLOOR {
                end = Some(TurretEnd::Fell);
            }
            if end.is_none() && t.landed {
                self.turret_fight(&mut t, &body, gun, dt);
            }
            if let Some(end) = end {
                self.turrets.remove(k);
                self.turret_gone(&t, end);
                continue;
            }
            self.turrets[k] = t;
            k += 1;
        }
    }

    /// `[ours]` (PR #61 review): a landed turret still has something under
    /// its base - terrain, a building or a hull - within [`SUPPORT_GAP`] of
    /// its origin, along its up. A turret on a vehicle that drove off, or on
    /// a hull that was destroyed, has none and falls (the PS2's rule is not
    /// traced).
    fn turret_supported(&self, t: &Turret) -> bool {
        let from = t.pos + t.up * SUPPORT_RISE;
        self.sweep_item(
            from,
            -t.up,
            SUPPORT_RISE + SUPPORT_GAP,
            HullFilter::All,
            None,
        )
        .is_some()
    }

    fn turret_fall(&self, t: &mut Turret, c: &TurretBody, dt: f32) {
        let from = t.pos;
        crate::mine::fall(&mut t.pos, &mut t.vel, c.gravity, dt);
        let step = t.pos - from;
        let len = step.length();
        if len <= 0.0 {
            return;
        }
        let Some((point, n, _)) = self.sweep_item(from, step / len, len, HullFilter::All, None)
        else {
            return;
        };
        if t.vel.dot(n) >= 0.0 {
            return;
        }
        if n.y > LAND_NY {
            t.land(point, n);
        } else {
            t.bounce(c, point, n, self.push_off_room(point, n).fmin(0.01));
        }
    }

    fn turret_fight(&mut self, t: &mut Turret, c: &TurretBody, gun: usize, dt: f32) {
        t.think -= dt;
        if t.think <= 0.0 {
            t.think += THINK;
            t.thinks += 1;
            self.turret_think(t, gun);
            if t.target.is_none() && t.thinks % LOOK_EVERY == 1 {
                t.look = t.base_yaw + self.lcg.uniform(-LOOK_SPREAD, LOOK_SPREAD);
            }
        }
        let point = t.target.and_then(|a| self.turret_aim(a, t.team));
        if point.is_none() {
            t.target = None;
            t.fighting = false;
        }
        t.turn(c, dt, point.map_or((t.look, 0.0), |p| t.angles_to(p)));
        let wc = &self.weapon_classes[gun];
        let reach = wc.ai_range[2];
        // Card TKFIX (ours): no shot through a friend on foot, as a bot's.
        let fire = t.fighting
            && point.is_some_and(|p| {
                let spread = t.gun.yaw_spread(wc).fmax(t.gun.pitch_spread(wc));
                t.on_target(p)
                    && p.distance(t.muzzle()) <= reach
                    && !crate::ai::item_line_blocked(
                        self,
                        t.team,
                        t.muzzle(),
                        t.aim_frame(),
                        p,
                        spread,
                    )
            });
        let mut shots = Vec::new();
        t.gun.update(
            dt,
            wc,
            FireInput {
                held: fire,
                ..FireInput::default()
            },
            &mut self.lcg,
            &mut shots,
        );
        if !shots.is_empty() {
            let owner = self.item_owner(t.owner, t.team);
            let who = Shooter {
                owner,
                team: t.team,
                weapon_class: gun,
                is_player: self
                    .units
                    .get(owner.index as usize)
                    .is_some_and(|u| u.gen == owner.gen && u.is_player),
            };
            self.free_shots(who, t.muzzle(), t.aim_frame(), &shots);
        }
    }

    fn turret_think(&self, t: &mut Turret, gun: usize) {
        let wc = &self.weapon_classes[gun];
        let [min, _, max] = wc.ai_range;
        let eye = t.centre();
        let seen = |p: Vec3| crate::perception::los(self, eye, p);
        if t.fighting {
            let held = t.target.and_then(|a| self.turret_aim(a, t.team));
            if held.is_some_and(|p| p.distance(eye) <= max + FIGHT_MARGIN && seen(p)) {
                return;
            }
        }
        let kinds = wc.ai_target_kind;
        let units = self
            .alive_units()
            .filter(|(_, u)| {
                self.hostile(t.team, u.team)
                    && u.mount.is_none()
                    && u.cloak.visible()
                    && kinds & (1 << u.kind()) != 0
            })
            .map(|(h, u)| (Aim::Unit(h), u.capsule().center()));
        let vehicles = self
            .alive_vehicles()
            .filter(|(_, v)| {
                v.health.alive()
                    && v.team.is_some_and(|team| self.hostile(t.team, team))
                    && kinds & (1 << HealthType::Vehicle as u32) != 0
            })
            .map(|(h, v)| (Aim::Vehicle(h), self.vehicle_blast_sphere(v).0));
        let mut near: Vec<(f32, Aim, Vec3)> = units
            .chain(vehicles)
            .map(|(a, p)| (p.distance(eye), a, p))
            .filter(|&(d, _, _)| d <= VIEW)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let found = near.into_iter().find(|&(_, _, p)| seen(p));
        t.target = found.map(|(_, a, _)| a);
        t.fighting = found.is_some_and(|(d, _, _)| (min..=max).contains(&d));
    }

    /// Where a held target is, while it is still one for a turret of
    /// `team`. A vehicle is hostile only by its live team, so an emptied
    /// building turret (neutral again) is dropped (playtest 2026-10-29).
    fn turret_aim(&self, a: Aim, team: u8) -> Option<Vec3> {
        match a {
            Aim::Unit(h) => self
                .unit(h)
                .filter(|u| self.hostile(team, u.team) && u.mount.is_none() && u.cloak.visible())
                .map(|u| u.capsule().center()),
            Aim::Vehicle(h) => self
                .vehicle(h)
                .filter(|v| v.team.is_some_and(|vt| self.hostile(team, vt)))
                .map(|v| self.vehicle_blast_sphere(v).0),
        }
    }

    fn turret_gone(&mut self, t: &Turret, end: TurretEnd) {
        self.events.push(Event::TurretGone {
            pos: t.centre(),
            weapon_class: t.weapon_class as u32,
            end,
        });
        let e = self
            .turret_class(t)
            .and_then(|c| c.end_explosion(end).cloned());
        if let Some(e) = e {
            let owner = self.item_owner(t.owner, t.team);
            self.explode(&e, t.centre(), Some(owner), Some(t.team));
        }
    }
}

#[cfg(test)]
mod tests;
