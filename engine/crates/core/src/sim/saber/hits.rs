//! Actual posed blade queries against soldiers and destructible hurt primitives.

use super::*;
use crate::fmath::FloatExt;
use crate::{
    collision::Layers,
    hero_weapon::BladeSegment,
    projectile::{building_hit, Hit, Target},
};
use glam::Mat4;

impl Sim {
    pub(in crate::sim) fn saber_hits(&mut self) {
        for i in 0..self.units.len() {
            self.saber_hits_for(i);
            if let Some(state) = self.units[i].weapon.melee.as_mut() {
                state.expire_hits();
            }
        }
    }

    fn saber_hits_for(&mut self, i: usize) {
        let u = &self.units[i];
        if !u.alive || u.mount.is_some() || u.soldier.no_fire() || u.choked.is_some() {
            return;
        }
        let Some(class) = self.weapon_classes[u.weapon_class].saber.as_ref() else {
            return;
        };
        let Some(state) = u.weapon.melee.as_ref() else {
            return;
        };
        let Some(clip) = state
            .animation
            .as_deref()
            .and_then(|name| class.clips.iter().find(|c| c.name == name))
        else {
            return;
        };
        let mut hits = Vec::new();
        let basis = state.blade_frame(u.soldier.pos, u.soldier.yaw, u.soldier.pitch);
        let previous_basis = state.blade_frame(
            state.previous_position,
            state.previous_yaw,
            state.previous_pitch,
        );
        let incoming = basis.z_axis.truncate();
        for attack in &state.attacks {
            let edge = usize::from(attack.edge);
            let Some(blade) = class.blades.get(edge) else {
                continue;
            };
            if blade.bone.eq_ignore_ascii_case("hp_weapons")
                && (u.weapon.saber_flight.is_some()
                    || u.held.iter().any(|w| w.state.saber_flight.is_some()))
            {
                continue;
            }

            let Some(current) = clip.sample(state.runtime.time, edge) else {
                continue;
            };
            let previous = clip
                .sample((state.runtime.time - TICK_DT).fmax(0.0), edge)
                .unwrap_or(current);
            let length = attack.length.map_or(blade.length, |n| {
                if attack.length_from_edge {
                    n * blade.length
                } else {
                    n
                }
            });
            let width = attack.width.map_or(blade.width, |n| {
                if attack.width_from_edge {
                    n * blade.width
                } else {
                    n
                }
            });
            let place = |segment: BladeSegment, basis: Mat4| {
                let direction = (segment.end - segment.start).normalize_or_zero();
                let start = basis.transform_point3(segment.start);
                let end = basis.transform_point3(segment.start + direction * length);
                (start, end)
            };
            let (start, end) = place(current, basis);
            let (old_start, old_end) = place(previous, previous_basis);
            for t in 0..self.units.len() {
                let handle = self.handle(t);
                // [ours] (the maintainer, 2026-10-30: "So the shield has to be
                // worn down before doing damage to the droideka"): a raised
                // bubble meets the blade first.
                let bubble = self.units[t].droideka_shield();
                let target = if bubble.is_some() {
                    Target::Shield(handle)
                } else {
                    Target::Unit(handle)
                };
                if t == i || !self.hero_body_active(t) || state.already_hit(attack, target) {
                    continue;
                }
                let prims = self.hero_body_prims(t);
                if bubble.is_none() && prims.is_none() {
                    continue;
                }
                let travel = (start - old_start).length().fmax((end - old_end).length());
                // [ours] Linear samples of the baked endpoints; the native
                // subframe bone evaluator is not part of the core rig.
                let steps = (travel / 0.5).ceil().fmax(1.0) as usize;
                let touched = (0..=steps).find_map(|step| {
                    let fraction = step as f32 / steps as f32;
                    let a = old_start.lerp(start, fraction);
                    let b = old_end.lerp(end, fraction);
                    let direction = (b - a).normalize_or_zero();
                    let root = a - direction * 0.2;
                    match (&bubble, &prims) {
                        (Some(b), _) => {
                            crate::unit::ray_sphere(root, direction, b.center, b.radius + width)
                                .filter(|&t| t < length)
                        }
                        (None, Some(p)) => p.melee_contact(root, direction, length, width),
                        (None, None) => None,
                    }
                    .map(|t| root + direction * t)
                });
                if let Some(pos) = touched {
                    hits.push((target, *attack, attack.damage.unwrap_or(class.damage), pos));
                }
            }
            // [ours] (the maintainer, 2026-10-30: "Make sure lightsabers and
            // heroes can destroy them too"): the sniper's dropped turret and a
            // remote droid are spheres to the blade, as to a bolt. The native
            // gather's mask does not name these; the wielder's own are spared.
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
                if state.already_hit(attack, target) {
                    continue;
                }
                let travel = (start - old_start).length().fmax((end - old_end).length());
                let steps = (travel / 0.5).ceil().fmax(1.0) as usize;
                let touched = (0..=steps).find_map(|step| {
                    let fraction = step as f32 / steps as f32;
                    let a = old_start.lerp(start, fraction);
                    let b = old_end.lerp(end, fraction);
                    let direction = (b - a).normalize_or_zero();
                    let root = a - direction * 0.2;
                    crate::unit::ray_sphere(root, direction, at, radius + width)
                        .filter(|&t| t < length)
                        .map(|t| root + direction * t)
                });
                if let Some(pos) = touched {
                    hits.push((target, *attack, attack.damage.unwrap_or(class.damage), pos));
                }
            }
            for body in self.collision.bodies_and_hulls() {
                if !body.layers.contains(Layers::ORDNANCE) {
                    continue;
                }
                let target = body.owner.map_or_else(
                    || building_hit(&self.collision, body.instance),
                    |v| Hit::Target(Target::Vehicle(v)),
                );
                let Hit::Target(target) = target else {
                    continue;
                };
                if state.already_hit(attack, target) {
                    continue;
                }
                let travel = (start - old_start).length().fmax((end - old_end).length());
                let steps = (travel / 0.5).ceil().fmax(1.0) as usize;
                let touched = (0..=steps).find_map(|step| {
                    let fraction = step as f32 / steps as f32;
                    let a = old_start.lerp(start, fraction);
                    let b = old_end.lerp(end, fraction);
                    let direction = (b - a).normalize_or_zero();
                    let root = a - direction * 0.2;
                    body.blade_contact_at(root, b - direction * 0.2, width)
                        .map(|t| root + direction * t)
                });
                if let Some(pos) = touched {
                    hits.push((target, *attack, attack.damage.unwrap_or(class.damage), pos));
                }
            }
        }
        let mut struck = false;
        let wc = u.weapon_class;
        for (target, attack, damage, pos) in hits {
            let state = self.units[i].weapon.melee.as_mut().unwrap();
            if !state.record_hit(&attack, target) {
                continue;
            }
            let direction = incoming;
            if let Target::Unit(h) = target {
                if self.saber_deflect(h, direction, pos) {
                    continue;
                }
            }
            let mut desc = self.weapon_classes[wc].ordnance.damage_desc(0.0, false);
            desc.damage = damage;
            desc.scales = [1.0; 7];
            desc.shield_scale = 1.0;
            let owner = self.handle(i);
            let team = self.units[i].team;
            match target {
                Target::Unit(h) => self.hero_damage(i, h, desc, direction),
                Target::Shield(h) => self.saber_on_shield(i, h, desc),
                Target::Vehicle(h) => {
                    self.damage_vehicle_by(h, &desc, 1.0, Some(owner), Some(team), Some(direction));
                }
                Target::Station(n) => self.damage_station(n, &desc, 1.0, Some(owner), Some(team)),
                Target::Droid(_) | Target::Turret(_) => {
                    self.damage_item(target, desc.damage, &desc.scales, Some(team), Some(owner));
                }
                _ => {}
            }
            // HitEffect at every victim (B2:336296-336333).
            self.events.push(crate::sim::Event::SaberContact {
                pos,
                weapon_class: wc as u32,
                deflect: false,
            });
            struck = true;
            if let Target::Unit(h) = target {
                if attack.push != 0.0 {
                    self.hero_knock(h, direction, attack.push);
                }
            }
        }
        // One HitSound an update, whatever it struck (B2:336346-336356).
        if struck {
            if let Some(sound_hash) = self.weapon_classes[wc]
                .saber
                .as_ref()
                .and_then(|c| c.hit_sound.as_deref())
                .map(crate::hero_weapon::sound_hash)
            {
                self.events.push(crate::sim::Event::ComboSound {
                    unit: self.handle(i),
                    weapon_class: wc as u32,
                    sound_hash,
                });
            }
        }
    }
}
