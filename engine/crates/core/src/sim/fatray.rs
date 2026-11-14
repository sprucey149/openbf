use super::state::Sim;
use crate::fmath::FloatExt;
use crate::projectile::Bolt;
use crate::unit::UnitHandle;
use crate::weapon::{DamageDesc, FatRayClass};
use glam::Vec3;

struct Strike {
    bolt: usize,
    victim: UnitHandle,
    damage: f32,
}

/// One tick of fat ray `b`'s pass through `points` (each unit's
/// [`Sim::fat_point`], in `alive_units` order): who it strikes and for how
/// much, its damage left written back into `b.fat`.
pub(super) fn fat_ray_pass(
    b: &mut Bolt,
    class: &FatRayClass,
    dt: f32,
    points: &[(UnitHandle, Vec3)],
) -> Vec<(UnitHandle, f32)> {
    let mut hits = Vec::new();
    let step = dt.fmin(b.remaining.fmax(0.0));
    let speed = b.vel.length();
    if step <= 0.0 || speed <= 0.0 {
        return hits;
    }
    let (from, dir, len, owner) = (b.pos, b.vel / speed, speed * step, b.owner);
    let mut damage = b.fat.unwrap_or_else(|| class.damage(b.charge));
    for &(h, p) in points {
        if h == owner {
            continue;
        }
        let t = (p - from).dot(dir);
        if t <= 0.0 || t >= len {
            continue;
        }
        let d = (p - (from + dir * t)).length();
        if d >= class.radius {
            continue;
        }
        let f = 1.0 - d / class.radius;
        if damage > 0.0 {
            hits.push((h, damage * f));
            damage = (damage - class.damage_deduction * f).fmax(class.min_damage);
        }
    }
    b.fat = Some(damage);
    hits
}

impl Sim {
    /// Every live unit with a body and its [`Sim::fat_point`], in
    /// `alive_units` order.
    fn fat_points(&self) -> Vec<(UnitHandle, Vec3)> {
        self.alive_units()
            .filter_map(|(h, u)| {
                let body = self.hero_body_prims(h.index as usize)?;
                Some((h, Self::fat_point(u, body.pos)))
            })
            .collect()
    }

    pub(super) fn sweep_fat_rays(&mut self, dt: f32) {
        let classes = &self.weapon_classes;
        if !self
            .bolts
            .iter()
            .any(|b| classes[b.weapon_class].ordnance.fat_ray.is_some())
        {
            return;
        }
        let points = self.fat_points();
        let mut strikes = Vec::new();
        for k in 0..self.bolts.len() {
            let Some(class) = self.weapon_classes[self.bolts[k].weapon_class]
                .ordnance
                .fat_ray
            else {
                continue;
            };
            for (victim, damage) in fat_ray_pass(&mut self.bolts[k], &class, dt, &points) {
                strikes.push(Strike {
                    bolt: k,
                    victim,
                    damage,
                });
            }
        }
        for s in strikes {
            let b = self.bolts[s.bolt].clone();
            self.fat_ray_strike(b, s.victim, s.damage);
        }
    }

    /// Fat ray `b` strikes `victim` for `damage` (`b` as it was on the
    /// pass that met him).
    pub(super) fn fat_ray_strike(&mut self, mut b: Bolt, victim: UnitHandle, damage: f32) {
        let Some(v) = self.unit(victim) else {
            return;
        };
        b.pos = v.aim_point();
        // BF2 FUN_00317f20: acceptance skips this victim's damage, while
        // the original wide ray and its deduction continue to later victims.
        if self.deflect_bolt(victim, &b) {
            return;
        }
        let desc = DamageDesc {
            damage,
            scales: [1.0; 7],
            shield_scale: 1.0,
        };
        if let Some(damage) = self.bolt_strikes_unit(&b, victim, &desc, 1.0) {
            let dir = b.vel.normalize_or_zero();
            self.finish_unit_damage(victim, Some(b.owner), damage, b.age, dir);
        }
    }
}

#[cfg(test)]
mod tests;
