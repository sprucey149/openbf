use super::{DeathCause, Event, Sim};
use crate::soldier::SoldierConstants;
use crate::weapon::DamageDesc;
use glam::Vec3;

pub(super) fn damage(c: &SoldierConstants, velocity: Vec3, normal: Vec3, yaw: f32) -> f32 {
    if c.collision_damage_immune {
        return 0.0;
    }
    let (sy, cy) = crate::fmath::sin_cos(yaw);
    let forward = Vec3::new(-sy, 0.0, -cy).dot(normal);
    let evaluate = |v: [f32; 3]| v[0] + v[1] * normal.y + v[2] * forward;
    let approach = -velocity.dot(normal);
    let threshold = evaluate(c.collision_threshold);
    if approach <= threshold {
        return 0.0;
    }
    let excess = approach - threshold;
    (excess * excess * evaluate(c.collision_damage_scale)).max(0.0)
}

impl Sim {
    pub(super) fn landing_damage(&mut self, i: usize, velocity: Vec3, normal: Vec3) {
        let u = &self.units[i];
        if !u.alive || u.is_player && self.cheats.invincible {
            return;
        }
        let amount = damage(&u.consts, velocity, normal, u.soldier.yaw);
        if amount <= 0.0 {
            return;
        }
        let victim = self.handle(i);
        let u = &mut self.units[i];
        let damage = u.health.take_damage(
            &DamageDesc {
                damage: amount,
                scales: [1.0; 7],
                shield_scale: 1.0,
            },
            1.0,
            false,
            1.0,
        );
        if damage > 0.0 {
            u.cloak.strength = 0.0;
            self.events.push(Event::FallDamage {
                victim,
                damage,
                health_left: u.health.cur,
            });
        }
        if !u.health.alive() {
            self.unit_killed_with_cause(victim, None, DeathCause::Fall);
        }
    }
}

#[cfg(test)]
mod tests;
