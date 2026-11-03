//! Placed explosives in the sim: placing, flight and landing, arming,
//! proximity / contact / timer / remote triggers and the blast
//! (`crate::mine`; issue #43).

use super::{Event, Sim};
use crate::collision::{ray_terrain_ps2, HullFilter, Layers};
use crate::mine::{Detonation, Mine};
use crate::projectile::Target;
use crate::sticky::{Anchor, Surface};
use crate::unit::UnitHandle;
use glam::Vec3;

fn square(v: f32) -> f32 {
    v * v
}

/// `y` held to a capsule's axis `[lo, hi]`. A capsule no taller than its two
/// caps has `lo == hi`, and rounding can leave `lo` a hair above `hi`, where
/// `f32::clamp` panics (a crouched or prone soldier beside a BF2 mine).
fn clamp_axis(y: f32, lo: f32, hi: f32) -> f32 {
    if lo <= hi {
        y.clamp(lo, hi)
    } else {
        (lo + hi) * 0.5
    }
}

pub(super) fn oldest_beyond<T>(items: &[T], max: u32, own: impl Fn(&T) -> bool) -> Option<usize> {
    if max == 0 {
        return None;
    }
    let mut owned = items.iter().enumerate().filter(|(_, t)| own(t));
    let first = owned.next()?.0;
    (owned.count() + 1 >= max as usize).then_some(first)
}

impl Sim {
    pub(super) fn throw_from(&self, i: usize, weapon: usize, speed: f32) -> (Vec3, Vec3) {
        let u = &self.units[i];
        let (aim, _, _) = u.aim_frame();
        let pos = u.launch_origin(self.weapon_classes[weapon].thrown.and_then(|t| t.hand));
        (pos, aim * speed + u.soldier.vel)
    }

    pub(super) fn has_remote_items(&self, i: usize, weapon: usize) -> bool {
        if self.weapon_classes[weapon].remote.is_some() {
            return self.droid_of(self.handle(i), weapon).is_some();
        }
        self.weapon_classes[weapon]
            .mine
            .as_ref()
            .is_some_and(|d| d.item.remote)
            && self.mines.iter().any(|m| {
                m.flight.owner == self.handle(i)
                    && m.flight.weapon_class == weapon
                    && m.health > 0.0
                    && !m.triggered
            })
    }

    /// A remote (detpack) weapon: a fresh press with packs out sets them
    /// all off and fires nothing; with none out the press places one.
    /// Returns whether the trigger must be kept from the weapon.
    pub(super) fn remote_input(
        &mut self,
        i: usize,
        slot: Option<usize>,
        held: bool,
        blocked: bool,
    ) -> bool {
        let weapon = slot.map_or(self.units[i].weapon_class, |k| self.units[i].held[k].class);
        if self.weapon_classes[weapon].guided.is_some() {
            return self.guided_input(i, slot, held);
        }
        if self.weapon_classes[weapon].remote.is_some() {
            return self.droid_input(i, slot, weapon, held, blocked);
        }
        if !self.weapon_classes[weapon]
            .mine
            .as_ref()
            .is_some_and(|d| d.item.remote)
        {
            return false;
        }
        let has = self.has_remote_items(i, weapon);
        let owner = self.handle(i);
        let state = match slot {
            Some(k) => &mut self.units[i].held[k].state,
            None => &mut self.units[i].weapon,
        };
        let fresh = held && state.remote_press == 0;
        if !held {
            state.remote_press = 0;
        } else if fresh {
            state.remote_press = if has { 2 } else { 1 };
        }
        let suppress = has || state.remote_press == 2;
        if fresh && has && !blocked {
            state.refire_accum = 0.0;
            state.salvo_counter = 0;
            for m in &mut self.mines {
                if m.flight.owner == owner && m.flight.weapon_class == weapon && m.health > 0.0 {
                    m.triggered = true;
                }
            }
        }
        suppress
    }

    /// Place one item; past `MaxItems` the owner's oldest is destroyed first.
    pub(super) fn place_mine(&mut self, i: usize, weapon: usize, charge: f32) {
        let d = self.weapon_classes[weapon].mine.as_ref().unwrap().clone();
        let owner = self.handle(i);
        if let Some(k) = oldest_beyond(&self.mines, d.max_items, |m| {
            m.flight.owner == owner && m.flight.weapon_class == weapon
        }) {
            let old = self.mines.remove(k);
            self.mine_blast(&old, Detonation::Destroyed);
        }
        let (pos, vel) = self.throw_from(i, weapon, d.speed(charge));
        let u = &self.units[i];
        self.next_mine_id = self
            .next_mine_id
            .checked_add(1)
            .expect("mine identity exhausted");
        let mut mine = Mine::new(self.next_mine_id, pos, vel, &d.item, owner, u.team, weapon);
        mine.rotation = crate::vehicle::hover::quat_y(u.soldier.yaw);
        self.mines.push(mine);
        self.events.push(Event::Throw {
            unit: owner,
            weapon_class: weapon as u32,
            origin: pos,
        });
    }

    pub(super) fn mine_damage(
        &self,
        m: &Mine,
        damage: f32,
        scales: &[f32; 7],
        team: Option<u8>,
    ) -> f32 {
        let health_type = self.weapon_classes[m.flight.weapon_class]
            .mine
            .as_ref()
            .and_then(|d| d.item.health_type);
        self.item_damage(damage, scales, health_type, m.flight.team, team)
    }

    /// The unit a placed or flown item (auto-turret, remote droid and its
    /// strike, mine, guided rocket) credits: its owner's CURRENT life in the
    /// same unit slot, so a kill after the owner respawned or changed class
    /// (a new generation) still reaches him. A slot that now plays for
    /// another team keeps the stale handle, which credits nobody.
    pub(super) fn item_owner(&self, owner: UnitHandle, team: u8) -> UnitHandle {
        match self.units.get(owner.index as usize) {
            Some(u) if u.team == team && u.gen >= owner.gen => self.handle(owner.index as usize),
            _ => owner,
        }
    }

    pub(super) fn item_damage(
        &self,
        damage: f32,
        scales: &[f32; 7],
        health_type: Option<crate::weapon::HealthType>,
        own_team: u8,
        team: Option<u8>,
    ) -> f32 {
        let ff = if team == Some(own_team) {
            self.friendly_fire_scale
        } else {
            1.0
        };
        (damage * health_type.map_or(1.0, |h| scales[h as usize]) * ff).max(0.0)
    }

    pub(super) fn damage_item(
        &mut self,
        target: Target,
        damage: f32,
        scales: &[f32; 7],
        team: Option<u8>,
        attacker: Option<UnitHandle>,
    ) {
        match target {
            Target::Mine(id) => {
                if let Some(k) = self.mines.iter().position(|m| m.id == id) {
                    self.mines[k].health -= self.mine_damage(&self.mines[k], damage, scales, team);
                }
            }
            Target::Droid(id) => {
                if let Some(k) = self.droids.iter().position(|d| d.id == id) {
                    self.droids[k].health -=
                        self.droid_damage(&self.droids[k], damage, scales, team);
                }
            }
            Target::Turret(id) => {
                if let Some(k) = self.turrets.iter().position(|t| t.id == id) {
                    let t = &self.turrets[k];
                    let ht = self.turret_class(t).map(|c| c.health_type);
                    let dealt = self.item_damage(damage, scales, ht, t.team, team);
                    self.turrets[k].health -= dealt;
                    if let Some(shooter) = attacker.filter(|_| dealt > 0.0) {
                        self.events.push(Event::TurretHit {
                            shooter,
                            turret: id,
                            team: self.turrets[k].team,
                            damage: dealt,
                            health_left: self.turrets[k].health,
                        });
                    }
                }
            }
            Target::Guided(id) => {
                if let Some(k) = self.guided.iter().position(|r| r.id == id) {
                    let r = &self.guided[k];
                    let ht = self.guided_class(r).map(|c| c.health_type);
                    self.guided[k].health -= self.item_damage(damage, scales, ht, r.team, team);
                }
            }
            Target::Unit(_)
            | Target::Vehicle(_)
            | Target::Station(_)
            | Target::Building(_)
            | Target::Shield(_) => {}
        }
    }

    fn mine_blast(&mut self, mine: &Mine, cause: Detonation) {
        let g = &mine.flight;
        let e = self.weapon_classes[g.weapon_class]
            .mine
            .as_ref()
            .unwrap()
            .item
            .explosion(cause)
            .cloned();
        if let Some(e) = e {
            self.events.push(Event::MineBlast {
                pos: g.pos,
                weapon_class: g.weapon_class as u32,
                cause,
            });
            let owner = self.item_owner(g.owner, g.team);
            self.explode(&e, g.pos, Some(owner), Some(g.team));
        }
    }

    /// One tick of every placed item. An armed item fires when an enemy is
    /// in its trigger radius and not moving towards it (BF1 `FUN_0028d958`),
    /// unless a friend is within its friend radius.
    pub(super) fn tick_mines(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.mines.len() {
            let mut m = self.mines[i].clone();
            let c = self.weapon_classes[m.flight.weapon_class]
                .mine
                .as_ref()
                .unwrap()
                .item
                .motion();
            let cause = if m.flight.life <= dt + 0.0001 {
                Some(Detonation::Expire)
            } else if m.health <= 0.0 {
                Some(Detonation::Destroyed)
            } else if m.triggered {
                Some(Detonation::Trigger)
            } else {
                None
            };
            if let Some(cause) = cause {
                self.mines.remove(i);
                self.mine_blast(&m, cause);
                continue;
            }
            let old_life = m.flight.life;
            m.flight.life -= dt;
            m.age += dt;
            if c.life_span < 60.0 && old_life.ceil() != m.flight.life.ceil() {
                self.events.push(Event::MineTick {
                    pos: m.flight.pos,
                    weapon_class: m.flight.weapon_class as u32,
                });
            }
            if let Some(Anchor::Vehicle { handle, local }) = m.flight.anchor {
                if let Some(v) = self.vehicle(handle) {
                    m.flight.pos = v.pos + v.rot * local;
                    m.rotation = v.rot * m.anchor_rotation;
                }
            }
            if m.flight.moving {
                let prev = m.flight.pos;
                crate::mine::fall(&mut m.flight.pos, &mut m.flight.vel, c.gravity, dt);
                let step = m.flight.pos - prev;
                let len = step.length();
                if len > 0.0 {
                    if let Some((point, normal, surface)) =
                        self.sweep_mine(&m, prev, step / len, len + 0.01)
                    {
                        self.events.push(Event::StickyContact {
                            pos: point,
                            weapon_class: m.flight.weapon_class as u32,
                            water: false,
                        });
                        let vn = m.flight.vel.dot(normal);
                        let stick = c.bf2 && !matches!(surface, Surface::Unit { .. });
                        if stick || (!c.bf2 && vn > -1.0) {
                            m.flight.pos = point + normal * crate::sticky::STICK_OFF;
                            let hull = match surface {
                                Surface::Vehicle { handle, .. } => {
                                    self.vehicle(handle).map(|v| (handle, v.pos, v.rot))
                                }
                                _ => None,
                            };
                            let anchor =
                                hull.map_or(Anchor::World, |(handle, pos, rot)| Anchor::Vehicle {
                                    handle,
                                    local: rot.inverse() * (m.flight.pos - pos),
                                });
                            m.anchor(normal, anchor, hull.map(|(_, _, rot)| rot));
                        } else {
                            m.flight.vel =
                                crate::mine::rebound(m.flight.vel, normal, c.friction, c.rebound);
                            m.flight.pos =
                                point + normal * self.push_off_room(point, normal).min(0.01);
                        }
                    }
                }
            }
            if m.flight.armed {
                let mut triggered = false;
                let mut friend = false;
                for (_, u) in self.alive_units().filter(|(_, u)| u.mount.is_none()) {
                    if c.bf2 && u.consts.immune_to_mines {
                        continue;
                    }
                    let (lift, radius) = super::soldier_blast_sphere(u.soldier.stance);
                    let d = u.soldier.pos + Vec3::Y * lift - m.flight.pos;
                    if self.allied(m.flight.team, u.team) {
                        friend |= c.friend_radius > 0.0
                            && d.length_squared() <= square(c.friend_radius + radius);
                    } else if self.hostile(m.flight.team, u.team) {
                        triggered |= c.trigger_radius > 0.0
                            && d.length_squared() <= square(c.trigger_radius + radius)
                            && u.soldier.vel.dot(d) >= 0.0;
                        triggered |= c.contact_trigger && {
                            let cap = u.capsule();
                            let r = cap.radius.min(cap.height * 0.5);
                            let p = Vec3::new(
                                cap.base.x,
                                clamp_axis(
                                    m.flight.pos.y,
                                    cap.base.y + r,
                                    cap.base.y + cap.height - r,
                                ),
                                cap.base.z,
                            );
                            p.distance_squared(m.flight.pos) <= square(cap.radius + 0.05)
                        };
                    }
                }
                for (_, v) in self.alive_vehicles() {
                    let (centre, radius) = self.vehicle_blast_sphere(v);
                    let d = centre - m.flight.pos;
                    if v.team == Some(m.flight.team) {
                        friend |= c.friend_radius > 0.0
                            && d.length_squared() <= square(c.friend_radius + radius);
                    } else if v.team.is_some() {
                        triggered |= c.trigger_radius > 0.0
                            && d.length_squared() <= square(c.trigger_radius + radius)
                            && v.vel.dot(d) >= 0.0;
                        triggered |= c.contact_trigger && d.length_squared() <= radius * radius;
                    }
                }
                if triggered && !friend {
                    self.mines.remove(i);
                    self.mine_blast(&m, Detonation::Trigger);
                    continue;
                }
            }
            if m.flight.pos.y < crate::sticky::WORLD_FLOOR {
                self.mines.remove(i);
            } else {
                self.mines[i] = m;
                i += 1;
            }
        }
    }

    fn sweep_mine(
        &self,
        m: &Mine,
        from: Vec3,
        dir: Vec3,
        len: f32,
    ) -> Option<(Vec3, Vec3, Surface)> {
        let c = &self.weapon_classes[m.flight.weapon_class]
            .mine
            .as_ref()?
            .item;
        let hulls = if c.bf2 {
            HullFilter::All
        } else {
            HullFilter::Static
        };
        let units = c.bf2.then_some((m.flight.owner, m.age <= 0.5));
        self.sweep_item(from, dir, len, hulls, units)
    }

    pub(super) fn sweep_item(
        &self,
        from: Vec3,
        dir: Vec3,
        len: f32,
        hulls: HullFilter,
        units: Option<(UnitHandle, bool)>,
    ) -> Option<(Vec3, Vec3, Surface)> {
        let mut best = None;
        if let Some(hf) = &self.heightfield {
            if let Some(f) = ray_terrain_ps2(hf, &self.collision.cutters, from, from + dir * len) {
                let mut p = from + dir * (len * f);
                p.y = hf.height_at(p.x, p.z);
                best = Some((
                    len * f,
                    Vec3::from(hf.normal_at(p.x, p.z)),
                    Surface::Terrain,
                ));
            }
        }
        let mut nearer = |t, normal, surface| {
            if best.is_none_or(|b| t < b.0) {
                best = Some((t, normal, surface));
            }
        };
        if let Some(h) = self
            .collision
            .ray_with(from, dir, len, Layers::ORDNANCE, hulls)
        {
            nearer(
                h.t,
                h.normal,
                h.owner.map_or(Surface::World, |handle| Surface::Vehicle {
                    handle,
                    building: false,
                }),
            );
        }
        if let Some((owner, fresh)) = units {
            for (handle, u) in self.alive_units() {
                if u.mount.is_some() || (handle == owner && fresh) {
                    continue;
                }
                let cap = u.capsule();
                if let Some(t) = cap.ray_hit(from, dir, len) {
                    let p = from + dir * t;
                    let n = (p - (u.soldier.pos + Vec3::Y)).normalize_or_zero();
                    nearer(
                        t,
                        n,
                        Surface::Unit {
                            handle,
                            kind: u.kind(),
                        },
                    );
                }
            }
        }
        best.map(|(t, normal, surface)| {
            let mut p = from + dir * t;
            if surface == Surface::Terrain {
                if let Some(hf) = &self.heightfield {
                    p.y = hf.height_at(p.x, p.z);
                }
            }
            (p, normal, surface)
        })
    }
}

#[cfg(test)]
mod tests;
