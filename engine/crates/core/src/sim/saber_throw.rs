//! The thrown primary saber keeps its real attachment, rotating edges and
//! per-leg hit cache while the owner changes secondary weapons.

use super::{Event, Sim};
use crate::collision::{ray_terrain_ps2, HullFilter, Layers};
use crate::fmath::FloatExt;
use crate::hero_weapon::{SaberFlight, SaberThrow};
use crate::projectile::{building_hit, Hit, Target};
use crate::TICK_DT;
use glam::{Mat4, Vec3};

fn stops_saber_throw(target: Target) -> bool {
    matches!(
        target,
        Target::Shield(_) | Target::Vehicle(_) | Target::Station(_) | Target::Turret(_)
    )
}

impl Sim {
    fn saber_attachment(&self, i: usize, blade_class: usize, throw_class: usize) -> Option<Mat4> {
        let u = &self.units[i];
        let class = self.weapon_classes[blade_class].saber.as_ref()?;
        let name = &self.weapon_classes[throw_class].name;
        let local = class
            .throw_bindings
            .iter()
            .find(|b| b.weapon.eq_ignore_ascii_case(name))?
            .attachment;
        let rider = u.mount.and_then(|mount| {
            let vehicle = self.vehicle(mount.vehicle)?;
            if vehicle.kind == crate::vehicle::VehicleKind::Turret
                || vehicle
                    .seats
                    .get(usize::from(mount.seat))
                    .is_none_or(|seat| seat.occupant != Some(self.handle(i)))
            {
                return None;
            }
            self.hero_rider_frames
                .as_ref()?
                .frame(vehicle, mount.seat)
                .filter(|frame| frame.is_finite())
        });
        if let Some(world) = rider {
            return Some(world * local);
        }
        let (s, c) = crate::fmath::sin_cos(u.soldier.yaw);
        let world = Mat4::from_cols(
            Vec3::new(-c, 0.0, s).extend(0.0),
            Vec3::Y.extend(0.0),
            Vec3::new(-s, 0.0, -c).extend(0.0),
            u.soldier.pos.extend(1.0),
        );
        Some(world * local)
    }

    /// Entity removal retires ordnance; ordinary health death keeps its owner.
    pub(in crate::sim) fn retire_saber_flights(&mut self, i: usize) {
        let unit = &mut self.units[i];
        unit.weapon.saber_flight = None;
        for held in &mut unit.held {
            held.state.saber_flight = None;
        }
    }

    pub(super) fn launch_saber_throw(&mut self, i: usize, slot: Option<usize>, class: usize) {
        let u = &self.units[i];
        if u.weapon.saber_flight.is_some() || u.held.iter().any(|w| w.state.saber_flight.is_some())
        {
            return;
        }
        let blade_class = u.weapon_class;
        let Some(saber) = self.weapon_classes[blade_class].saber.as_ref() else {
            return;
        };
        let Some(edge) = saber.throw_edges.first() else {
            return;
        };
        let Some(attachment) = self.saber_attachment(i, blade_class, class) else {
            return;
        };
        let edge_direction = attachment
            .transform_vector3(edge.end - edge.start)
            .normalize_or_zero();
        let direction = if u.is_human() {
            self.human_shot(i).1 .0
        } else {
            u.aim_frame().0
        };
        let pos = attachment.w_axis.truncate();
        let mut basis = attachment;
        basis.w_axis = Vec3::ZERO.extend(1.0);
        let spin_axis = edge_direction.cross(direction).normalize_or_zero();
        let data = self.weapon_classes[class]
            .saber_throw
            .as_ref()
            .unwrap()
            .clone();
        if !self.units[i].soldier.pay_energy(data.energy) {
            return;
        }
        let distance = data.velocity * data.outbound_seconds;
        let world = self
            .collision
            .ray_with(
                pos,
                direction,
                distance,
                Layers::ORDNANCE,
                HullFilter::Static,
            )
            .map_or(distance, |hit| hit.t);
        let ground = self
            .heightfield
            .as_ref()
            .and_then(|hf| {
                ray_terrain_ps2(hf, &self.collision.cutters, pos, pos + direction * distance)
            })
            .map_or(distance, |fraction| distance * fraction);
        let outgoing = world.fmin(ground) / data.velocity;
        let flight = SaberFlight {
            pos,
            velocity: direction * data.velocity,
            basis,
            spin_axis,
            angle: 0.0,
            blade_class,
            remaining: outgoing,
            outbound_seconds: outgoing,
            return_age: 0.0,
            returning: false,
            hit: Vec::new(),
        };
        let state = if let Some(k) = slot {
            &mut self.units[i].held[k].state
        } else {
            &mut self.units[i].weapon
        };
        state.saber_flight = Some(flight);
        self.events.push(Event::Ability {
            unit: self.handle(i),
            weapon_class: class as u32,
            pos,
            charge: 1.0,
        });
    }

    pub(super) fn tick_saber_throws(&mut self) {
        for i in 0..self.units.len() {
            for n in 0..=self.units[i].held.len() {
                let slot = n.checked_sub(1);
                let (class, flight) = if let Some(k) = slot {
                    let w = &mut self.units[i].held[k];
                    (w.class, w.state.saber_flight.take())
                } else {
                    let u = &mut self.units[i];
                    (u.weapon_class, u.weapon.saber_flight.take())
                };
                let Some(mut flight) = flight else { continue };
                let Some(data) = self.weapon_classes[class].saber_throw.as_ref().cloned() else {
                    continue;
                };
                let Some(attachment) = self.saber_attachment(i, flight.blade_class, class) else {
                    continue;
                };
                let owner = attachment.w_axis.truncate();
                let before_position = flight.pos;
                let before_angle = flight.angle;
                let step_dt = if flight.remaining > 0.0 && flight.remaining - TICK_DT <= 0.0 {
                    flight.remaining - TICK_DT
                } else {
                    TICK_DT
                };
                let alive = flight.advance(&data, owner, TICK_DT);
                if alive {
                    let Some(pieces) = data.contact_substeps(step_dt) else {
                        continue;
                    };
                    let mut before = flight.transform_at(before_position, before_angle);
                    for piece in 0..pieces {
                        let fraction = (piece + 1) as f32 / pieces as f32;
                        let after = flight.transform_at(
                            before_position.lerp(flight.pos, fraction),
                            before_angle + (flight.angle - before_angle) * fraction,
                        );
                        let outbound = !flight.returning;
                        let struck =
                            self.saber_throw_contacts(i, class, &data, &mut flight, before, after);
                        before = after;
                        if struck && outbound {
                            // [ours] A hard stop ends the leg, past the launch
                            // ray the PS2 has: see `saber_throw_contacts`.
                            if !self.space_round() {
                                // Only the blocking contacts carry into the return leg.
                                flight.hit.retain(|&target| stops_saber_throw(target));
                            }
                            flight.turn_back(
                                after.w_axis.truncate(),
                                before_angle + (flight.angle - before_angle) * fraction,
                                step_dt.fmax(0.0) * (1.0 - fraction),
                            );
                            break;
                        }
                    }
                }
                if alive {
                    let state = if let Some(k) = slot {
                        &mut self.units[i].held[k].state
                    } else {
                        &mut self.units[i].weapon
                    };
                    state.saber_flight = Some(flight);
                } else if let Some(melee) = self.units[i].weapon.melee.as_mut() {
                    // Caught: FUN_0031cb78's return end calls FUN_00345c10
                    // (B2:310980-310985), which the melee weapon reads on
                    // its next update.
                    melee.catch_pending = true;
                }
            }
        }
    }

    fn saber_throw_contacts(
        &mut self,
        i: usize,
        class: usize,
        data: &SaberThrow,
        flight: &mut SaberFlight,
        before: Mat4,
        after: Mat4,
    ) -> bool {
        if flight.hit.len() >= 8 {
            return false;
        }
        let saber = self.weapon_classes[flight.blade_class]
            .saber
            .as_ref()
            .unwrap();
        let own_seat = self.units[i].mount.map(|m| m.vehicle);
        let mut hits: Vec<(Target, Vec3)> = Vec::new();
        for (edge, blade) in saber.throw_edges.iter().zip(
            saber
                .blades
                .iter()
                .filter(|b| b.bone.eq_ignore_ascii_case("hp_weapons")),
        ) {
            let edge_length = (edge.end - edge.start).length();
            let length = if data.length_from_edge {
                data.length * edge_length
            } else {
                data.length
            };
            let width = if data.width_from_edge {
                data.width * blade.width
            } else {
                data.width
            };
            let local = (edge.end - edge.start).normalize_or_zero();
            for transform in [before, after] {
                let direction = transform.transform_vector3(local).normalize_or_zero();
                let start = transform.transform_point3(edge.start) - direction * 0.2;
                let met = |hits: &[(Target, Vec3)], target: &Target| {
                    flight.hit.contains(target) || hits.iter().any(|(t, _)| t == target)
                };
                for t in 0..self.units.len() {
                    if i == t || !self.hero_body_active(t) {
                        continue;
                    }
                    let handle = self.handle(t);
                    // A raised bubble meets the blade before the droideka.
                    let (target, contact) = match self.units[t].droideka_shield() {
                        Some(bubble) => (
                            Target::Shield(handle),
                            crate::unit::ray_sphere(
                                start,
                                direction,
                                bubble.center,
                                bubble.radius + width,
                            )
                            .filter(|&t| t < length),
                        ),
                        None => (
                            Target::Unit(handle),
                            self.hero_body_prims(t).and_then(|prims| {
                                prims.melee_contact(start, direction, length, width)
                            }),
                        ),
                    };
                    if let Some(at) = contact.filter(|_| !met(&hits, &target)) {
                        hits.push((target, start + direction * at));
                    }
                }
                // [ours] The thrower's remote droids and turrets stay out of
                // it, as they do for his bolts. The PS2's gather has neither.
                let items = self
                    .droids
                    .iter()
                    .filter(|d| d.owner.index as usize != i)
                    .map(|d| (Target::Droid(d.id), d.pos, crate::remote::RADIUS))
                    .chain(
                        self.turrets
                            .iter()
                            .filter(|t| t.owner.index as usize != i)
                            .map(|t| (Target::Turret(t.id), t.centre(), crate::autoturret::RADIUS)),
                    );
                for (target, at, radius) in items {
                    if let Some(t) = crate::unit::ray_sphere(start, direction, at, radius + width)
                        .filter(|&t| t < length && !met(&hits, &target))
                    {
                        hits.push((target, start + direction * t));
                    }
                }
                // The same blade query includes vehicle hulls and buildings.
                for body in self.collision.bodies_and_hulls() {
                    let target = body.owner.map_or_else(
                        || building_hit(&self.collision, body.instance),
                        |v| Hit::Target(Target::Vehicle(v)),
                    );
                    let Hit::Target(target) = target else {
                        continue;
                    };
                    // The seat he throws from is no collision for his blade.
                    if matches!(target, Target::Vehicle(v) if own_seat == Some(v)) {
                        continue;
                    }
                    if body.layers.contains(Layers::ORDNANCE) && !met(&hits, &target) {
                        if let Some(at) =
                            body.blade_contact_at(start, start + direction * length, width)
                        {
                            hits.push((target, start + direction * at));
                        }
                    }
                }
            }
        }
        let owner = self.handle(i);
        let team = self.units[i].team;
        let desc = self.weapon_classes[class].ordnance.damage_desc(0.0, false);
        let direction = flight.velocity.normalize_or_zero();
        let blade = flight.blade_class as u32;
        // Soldiers, bare droidekas and remote droids take one hit per leg.
        // [ours] A raised bubble, a hull, a station or an auto-turret turns it back.
        let mut stopped = false;
        let mut sound_at = None;
        for (target, pos) in hits.into_iter().take(8 - flight.hit.len()) {
            flight.hit.push(target);
            stopped |= stops_saber_throw(target);
            // Read before the blow: a killed victim still heard it.
            let body = match target {
                Target::Unit(h) | Target::Shield(h) => self.unit(h).map(|u| u.soldier.pos),
                Target::Vehicle(h) => self.vehicle(h).map(|v| v.pos),
                Target::Station(n) => self.stations.get(n as usize).map(|s| s.pos),
                _ => Some(pos),
            };
            match target {
                Target::Unit(victim) => {
                    if self.saber_deflect(victim, direction, pos) {
                        continue;
                    }
                    self.hero_damage(i, victim, desc, direction);
                }
                Target::Shield(victim) => self.saber_on_shield(i, victim, desc),
                Target::Vehicle(vehicle) => {
                    self.damage_vehicle_by(
                        vehicle,
                        &desc,
                        1.0,
                        Some(owner),
                        Some(team),
                        Some(direction),
                    );
                }
                Target::Station(n) => self.damage_station(n, &desc, 1.0, Some(owner), Some(team)),
                Target::Droid(_) | Target::Turret(_) => {
                    self.damage_item(target, desc.damage, &desc.scales, Some(team), Some(owner));
                }
                _ => {}
            }
            self.events.push(Event::SaberContact {
                pos,
                weapon_class: blade,
                deflect: false,
            });
            sound_at = sound_at.or(body);
        }
        // One HitSound an update, at the first victim (B2:312042-312046).
        if let Some(pos) = sound_at {
            self.events.push(Event::SaberThrowHit {
                pos,
                weapon_class: blade,
            });
        }
        stopped
    }

    /// [ours] A blade on a droideka's raised shield, thrown or in hand, as a bolt on it
    /// (`Hit::Target(Target::Shield)` in the bolt's landing): the bubble
    /// absorbs what it can of the ordnance's damage and the overflow is lost.
    pub(in crate::sim) fn saber_on_shield(
        &mut self,
        i: usize,
        victim: crate::unit::UnitHandle,
        desc: crate::weapon::DamageDesc,
    ) {
        let Some(u) = self.unit(victim) else { return };
        let friendly = u.team == self.units[i].team;
        if (u.is_player && self.cheats_of(u).invincible)
            || (u.is_player && friendly && !self.units[i].is_player)
        {
            return;
        }
        let owner = self.handle(i);
        let ff = if friendly {
            self.friendly_fire_scale
        } else {
            1.0
        };
        let damage = desc.damage * self.buff_scale(Some(owner), victim) * ff;
        let tick = self.tick;
        if let Some(u) = self.unit_mut(victim) {
            if let Some(d) = u.droideka.as_mut() {
                d.shield.absorb(damage, desc.shield_scale);
            }
            u.last_hit = Some((owner, tick));
        }
    }
}

#[cfg(test)]
mod tests;
