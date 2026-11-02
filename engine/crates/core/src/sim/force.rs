//! BF2 emitter Force powers and the held area-effect choke. The same input,
//! energy, visibility and damage paths serve human and bot casters.

use super::{Event, Sim};
use crate::hero_weapon::{Burn, ForceKind, ForcePower};
use crate::unit::UnitHandle;
use crate::weapon::{DamageDesc, FireState};
use crate::TICK_DT;
use glam::Vec3;

#[derive(Clone, Copy)]
enum EmitterTarget {
    Unit(UnitHandle),
    Vehicle(crate::vehicle::VehicleHandle),
    /// A sniper's dropped turret or a remote droid.
    Item(crate::projectile::Target),
}

struct Candidate {
    target: EmitterTarget,
    centre: Vec3,
    distance: f32,
}

impl Sim {
    pub(super) fn force_locks(&mut self, pads: &mut [super::PadState]) {
        for (i, pad) in pads.iter_mut().enumerate() {
            let u = &mut self.units[i];
            if !u.alive {
                u.choked = None;
                continue;
            }
            if let Some(choked) = u.choked.as_mut() {
                if choked.left <= 0.0 {
                    u.choked = None;
                    continue;
                }
                choked.left -= TICK_DT;
                choked.age += TICK_DT;
                *pad = super::PadState::default();
                u.soldier.vel.x = 0.0;
                u.soldier.vel.z = 0.0;
                u.weapon.melee = None;
                for weapon in
                    std::iter::once(&mut u.weapon).chain(u.held.iter_mut().map(|w| &mut w.state))
                {
                    weapon.salvo_counter = 0;
                    weapon.fire_cmd = 0;
                    weapon.state = FireState::Idle;
                    weapon.force_channel = None;
                }
            }
        }
    }

    fn force_can_target(&self, owner: usize, target: usize, power: &ForcePower) -> bool {
        let caster = &self.units[owner];
        let victim = &self.units[target];
        if owner == target || !self.hero_body_active(target) {
            return false;
        }
        // Perception gates the AI's choice to aim/fire. A physical cone or
        // choke ray can still meet a bystander absent from its contacts.
        if caster.team == victim.team {
            power.affect_friends
        } else {
            power.affect_enemies
        }
    }

    pub(super) fn hero_damage(
        &mut self,
        i: usize,
        target: UnitHandle,
        desc: DamageDesc,
        direction: glam::Vec3,
    ) {
        self.hero_damage_phases(i, target, desc, direction, false);
    }

    /// Area damage passes both native shield and health flags. Physical blade
    /// body contacts keep their separate shield-sphere collision rule.
    fn force_damage(
        &mut self,
        i: usize,
        target: UnitHandle,
        desc: DamageDesc,
        direction: glam::Vec3,
    ) {
        self.hero_damage_phases(i, target, desc, direction, true);
    }

    fn hero_damage_phases(
        &mut self,
        i: usize,
        target: UnitHandle,
        mut desc: DamageDesc,
        direction: glam::Vec3,
        shield: bool,
    ) {
        let Some(victim) = self.unit(target) else {
            return;
        };
        let friendly = victim.team == self.units[i].team;
        let caster_is_player = self.units[i].is_player;
        if (victim.is_player && self.cheats.invincible)
            || (victim.is_player && friendly && !caster_is_player)
        {
            return;
        }
        let owner = self.handle(i);
        let seated_body = victim
            .mount
            .and_then(|_| self.hero_body_prims(target.index as usize));
        let multiplier = self.buff_scale(Some(owner), target);
        let friendly_fire = self.friendly_fire_scale;
        // The area-effect weapon supplies its own damage, retaining health
        // type and friendly-fire handling through the common descriptor.
        desc.damage = desc.damage.max(0.0);
        let victim = self.unit_mut(target).unwrap();
        let take = |health: &mut crate::weapon::Health| {
            health.take_damage(&desc, multiplier, friendly, friendly_fire)
        };
        let damage = if shield {
            victim.lend_droideka_shield(take)
        } else {
            take(&mut victim.health)
        };
        if !self.units[target.index as usize].health.alive() {
            if let Some(body) = seated_body {
                let soldier = &mut self.units[target.index as usize].soldier;
                soldier.pos = body.pos;
                soldier.yaw = body.yaw;
            }
        }
        self.finish_unit_damage(target, Some(owner), damage, 0.0, direction);
    }

    /// A saber or Force shove. A droideka's mover owns its body and never
    /// steps the soldier stance, so a knock would stay in its stance and hold
    /// its fire for good; blasts leave it unthrown the same way
    /// (`Sim::kick_and_push`). [ours]
    pub(super) fn hero_knock(&mut self, target: UnitHandle, direction: Vec3, scale: f32) {
        if self.unit(target).is_none_or(|u| u.droideka.is_some()) {
            return;
        }
        let draw = self.lcg.unit();
        self.units[target.index as usize]
            .soldier
            .knock(direction, scale, self.ground_rules, draw);
    }

    /// An emitter is immediate ordnance, not a projectile with zero speed.
    pub(super) fn fire_force(&mut self, i: usize, class: usize, charge: f32) {
        let power = self.weapon_classes[class].force.as_ref().unwrap().clone();
        let ForceKind::Emitter(emitter) = power.kind else {
            return;
        };
        if !self.units[i].soldier.pay_energy(power.energy) {
            return;
        }
        let owner = self.handle(i);
        let team = self.units[i].team;
        let from = self.units[i].muzzle();
        let direction = self.units[i].aim_frame().0;
        self.events.push(Event::Ability {
            unit: owner,
            weapon_class: class as u32,
            pos: from,
            charge,
        });
        self.units[i].last_shot_tick = self.tick;
        let mut targets: Vec<_> = (0..self.units.len())
            .filter(|&t| self.force_can_target(i, t, &power))
            .filter_map(|t| {
                let (lift, _) = super::blast::soldier_blast_sphere(self.units[t].soldier.stance);
                let centre = self.hero_body_prims(t)?.pos + Vec3::Y * lift;
                power
                    .contains(from, direction, centre)
                    .then_some(Candidate {
                        target: EmitterTarget::Unit(self.handle(t)),
                        centre,
                        distance: from.distance(centre),
                    })
            })
            .collect();
        for (handle, vehicle) in self.alive_vehicles() {
            let friend = vehicle.team == Some(self.units[i].team);
            if (friend && !power.affect_friends)
                || (!friend && !power.affect_enemies)
                || self.units[i].mount.is_some_and(|m| m.vehicle == handle)
            {
                continue;
            }
            let (centre, _) = self.vehicle_blast_sphere(vehicle);
            if power.contains(from, direction, centre) {
                targets.push(Candidate {
                    target: EmitterTarget::Vehicle(handle),
                    centre,
                    distance: from.distance(centre),
                });
            }
        }
        let desc_has_damage = self.weapon_classes[class]
            .ordnance
            .damage_desc_charged(0.0, false, charge)
            .damage
            > 0.0;
        // [ours] (the maintainer, 2026-10-30): "Make sure lightsabers and heroes
        // can destroy them too". A power with damage reaches the sniper's
        // turret and a remote droid; the caster's own are spared.
        if desc_has_damage {
            let caster_team = self.units[i].team;
            let items = self
                .droids
                .iter()
                .filter(|d| d.owner.index as usize != i)
                .map(|d| (crate::projectile::Target::Droid(d.id), d.team, d.pos))
                .chain(
                    self.turrets
                        .iter()
                        .filter(|t| t.owner.index as usize != i)
                        .map(|t| (crate::projectile::Target::Turret(t.id), t.team, t.centre())),
                );
            for (target, item_team, centre) in items {
                let wanted = if item_team == caster_team {
                    power.affect_friends
                } else {
                    power.affect_enemies
                };
                if wanted && power.contains(from, direction, centre) {
                    targets.push(Candidate {
                        target: EmitterTarget::Item(target),
                        centre,
                        distance: from.distance(centre),
                    });
                }
            }
        }
        // The PS2 heap key is negative 3-D distance (sqrt input at0x316508).
        // [ours] Stable gather order resolves exact ties; addresses do not.
        targets.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        targets.truncate(96);
        let mut desc = self.weapon_classes[class]
            .ordnance
            .damage_desc_charged(0.0, false, charge);
        let mut damage = desc.damage;
        if emitter.hit_all && !targets.is_empty() {
            damage = (damage / targets.len() as f32)
                .max(self.weapon_classes[class].ordnance.charged(0.0).damage);
        }
        let mut origin = from;
        let mut previous_distance = 0.0;
        let mut hits = 0;
        for candidate in targets {
            if hits >= power.max_targets || damage < emitter.damage_threshold {
                break;
            }
            if !emitter.hit_all
                && candidate.distance - previous_distance
                    > if hits == 0 {
                        power.range
                    } else {
                        emitter.max_jump_distance
                    }
            {
                break;
            }
            let ignore = match candidate.target {
                EmitterTarget::Vehicle(h) => self.vehicle(h).and_then(|v| v.instance),
                EmitterTarget::Unit(_) | EmitterTarget::Item(_) => None,
            };
            if !crate::perception::los_seat(
                self,
                origin,
                candidate.centre,
                crate::collision::HullFilter::Static,
                [ignore, None],
            ) {
                continue;
            }
            let incoming = (candidate.centre - origin).normalize_or_zero();
            let blocked = match candidate.target {
                EmitterTarget::Unit(h) => self.saber_deflect(h, incoming, candidate.centre),
                EmitterTarget::Vehicle(_) | EmitterTarget::Item(_) => false,
            };
            if !blocked && !emitter.hit_all && hits == 0 && emitter.first_radius > 0.0 {
                let delta = candidate.centre - from;
                let radial = delta - direction * delta.dot(direction);
                if radial.length_squared() > emitter.first_radius * emitter.first_radius {
                    continue;
                }
            }
            self.events.push(Event::ForceHit {
                owner,
                target: match candidate.target {
                    EmitterTarget::Unit(h) => crate::projectile::Target::Unit(h),
                    EmitterTarget::Vehicle(h) => crate::projectile::Target::Vehicle(h),
                    EmitterTarget::Item(t) => t,
                },
                from: origin,
                pos: candidate.centre,
                weapon_class: class as u32,
                blocked,
                charge,
            });
            if !blocked {
                desc.damage = damage;
                match candidate.target {
                    EmitterTarget::Unit(target) => {
                        if damage > 0.0 {
                            self.force_damage(i, target, desc, incoming);
                        }
                        if self.unit(target).is_some() {
                            let victim = &mut self.units[target.index as usize];
                            // A droideka does not burn, as in `Sim::ignite`.
                            if damage > 0.0
                                && emitter.burn_seconds > 0.0
                                && victim.droideka.is_none()
                            {
                                let left = victim
                                    .burn
                                    .map_or(0.0, |b| b.left)
                                    .max(emitter.burn_seconds);
                                victim.burn = Some(Burn {
                                    left,
                                    damage_per_second: emitter.burn_damage_per_second,
                                    owner,
                                    team,
                                    weapon_class: class,
                                });
                            }
                            if power.push != 0.0 {
                                self.force_push(target, from, power.range, power.push);
                            }
                        }
                    }
                    EmitterTarget::Vehicle(target) => {
                        if damage > 0.0 {
                            self.damage_vehicle_by(
                                target,
                                &desc,
                                1.0,
                                Some(owner),
                                Some(self.units[i].team),
                                Some(incoming),
                            );
                        }
                    }
                    EmitterTarget::Item(target) => {
                        let team = self.units[i].team;
                        self.damage_item(target, damage, &desc.scales, Some(team), Some(owner));
                    }
                }
                if damage > 0.0 {
                    if let Some(explosion) = self.weapon_classes[class].ordnance.explosion.clone() {
                        self.explode(
                            &explosion,
                            candidate.centre,
                            Some(owner),
                            Some(self.units[i].team),
                        );
                        self.events.push(Event::WeaponBlast {
                            pos: candidate.centre,
                            weapon_class: class as u32,
                        });
                    }
                }
                previous_distance = candidate.distance;
                if !emitter.hit_all && emitter.lightning && (hits == 0 || self.lcg.unit() > 0.3) {
                    origin = candidate.centre;
                }
            }
            if !emitter.hit_all {
                damage -= emitter.jump_deduction;
            }
            hits += 1;
        }
    }

    fn force_push(&mut self, target: UnitHandle, from: Vec3, range: f32, strength: f32) {
        let Some(u) = self.unit(target) else { return };
        if u.mount.is_some() || u.droideka.is_some() || u.soldier.stance.knocked() {
            return;
        }
        let (lift, radius) = super::blast::soldier_blast_sphere(u.soldier.stance);
        let centre = u.soldier.pos + Vec3::Y * lift;
        let (origin, inner, outer) = if strength > 0.0 {
            (from, range * 0.5, range * 2.0)
        } else {
            (
                from + (centre - from).normalize_or_zero() * (range * 1.1),
                0.0,
                range * 1.1,
            )
        };
        let delta = centre - origin;
        let length = delta.length();
        if length <= 0.0001 {
            return;
        }
        let distance = length - radius * u.consts.collision_scale * 0.5;
        let falloff = if distance < inner {
            1.0
        } else {
            ((outer - distance) / (outer - inner)).clamp(0.0, 1.0)
        };
        if falloff > 0.0 {
            let draw = self.lcg.unit();
            self.units[target.index as usize].soldier.force_knock(
                delta / length + Vec3::Y * 0.6,
                strength.abs() * falloff,
                draw,
            );
        }
    }

    /// Ordinary damage effects count down before their damage pass. A fresh
    /// shorter application keeps the longest timer but replaces rate/owner.
    pub(super) fn tick_burns(&mut self, dt: f32) {
        for i in 0..self.units.len() {
            let Some(mut burn) = self.units[i].burn else {
                continue;
            };
            if !self.units[i].alive {
                self.units[i].burn = None;
                continue;
            }
            burn.left -= dt;
            if burn.left <= 0.0 {
                self.units[i].burn = None;
                continue;
            }
            self.units[i].burn = Some(burn);
            // A burn caught on foot does not reach him inside a hull.
            if self.units[i].mount.is_some() && !self.hero_body_active(i) {
                continue;
            }
            let victim = self.handle(i);
            let friendly = self.units[i].team == burn.team;
            let owner_is_player = self
                .units
                .get(burn.owner.index as usize)
                .filter(|u| u.gen == burn.owner.gen)
                .is_some_and(|u| u.is_player);
            if (self.units[i].is_player && self.cheats.invincible)
                || (self.units[i].is_player && friendly && !owner_is_player)
            {
                continue;
            }
            let multiplier = self.buff_scale(Some(burn.owner), victim);
            let ff = self.friendly_fire_scale;
            let u = &mut self.units[i];
            if burn.damage_per_second > 0.0 {
                u.cloak.strength = 0.0;
            }
            let damage = u.health.take_damage(
                &DamageDesc {
                    damage: burn.damage_per_second * dt,
                    scales: [1.0; 7],
                    shield_scale: 1.0,
                },
                multiplier,
                friendly,
                ff,
            );
            if damage > 0.0 {
                u.last_hit = Some((burn.owner, self.tick));
            }
            if !u.health.alive() {
                self.unit_killed(victim, Some(burn.owner));
                self.units[i].burn = None;
            }
        }
    }
}

mod choke;
mod rate;

#[cfg(test)]
mod tests;
