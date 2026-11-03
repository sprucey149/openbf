use super::state::Sim;
use crate::unit::UnitHandle;
use crate::weapon::DamageDesc;

struct Strike {
    bolt: usize,
    victim: UnitHandle,
    damage: f32,
}

impl Sim {
    pub(super) fn sweep_fat_rays(&mut self, dt: f32) {
        let mut strikes = Vec::new();
        for k in 0..self.bolts.len() {
            let b = &self.bolts[k];
            let Some(class) = self.weapon_classes[b.weapon_class].ordnance.fat_ray else {
                continue;
            };
            let step = dt.min(b.remaining.max(0.0));
            let speed = b.vel.length();
            if step <= 0.0 || speed <= 0.0 {
                continue;
            }
            let (from, dir, len, owner) = (b.pos, b.vel / speed, speed * step, b.owner);
            let mut damage = b.fat.unwrap_or_else(|| class.damage(b.charge));
            for (h, u) in self.alive_units() {
                if h == owner {
                    continue;
                }
                let Some(body) = self.hero_body_prims(h.index as usize) else {
                    continue;
                };
                let p = u.aim_point() + body.pos - u.soldier.pos;
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
                    strikes.push(Strike {
                        bolt: k,
                        victim: h,
                        damage: damage * f,
                    });
                    damage = (damage - class.damage_deduction * f).max(class.min_damage);
                }
            }
            self.bolts[k].fat = Some(damage);
        }
        for s in strikes {
            self.fat_ray_strike(s);
        }
    }

    fn fat_ray_strike(&mut self, s: Strike) {
        let mut b = self.bolts[s.bolt].clone();
        let Some(victim) = self.unit(s.victim) else {
            return;
        };
        b.pos = victim.aim_point();
        // BF2 FUN_00317f20: acceptance skips this victim's damage, while
        // the original wide ray and its deduction continue to later victims.
        if self.deflect_bolt(s.victim, &b) {
            return;
        }
        let desc = DamageDesc {
            damage: s.damage,
            scales: [1.0; 7],
            shield_scale: 1.0,
        };
        if let Some(damage) = self.bolt_strikes_unit(&b, s.victim, &desc, 1.0) {
            let dir = b.vel.normalize_or_zero();
            self.finish_round_damage(s.victim, Some(b.owner), damage, b.age, dir);
        }
    }
}

#[cfg(test)]
mod tests;
