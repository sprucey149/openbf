//! The portable turret's PS2 damage bands: sparks at 50..30%, smoke at
//! 30..0%, read from its class rather than hard-coded in the effect pass.
//! `hp_smoke_1` is absent from the original dropturret skeleton; use the
//! entity frame, the same fallback as an unnamed attach point.
use super::{emitter::Frame, Fx};
use core::sim::Sim;

impl Fx {
    pub(super) fn follow_turrets(&mut self, sim: &Sim) {
        for effect in &mut self.live {
            let Some((id, slot)) = effect.turret else {
                continue;
            };
            let owner = sim.live_turret(id).filter(|t| {
                sim.turret_class(t).is_some_and(|c| {
                    c.damage_effects
                        .get(slot)
                        .is_some_and(|e| e.active(t.health / c.body.max_health))
                })
            });
            if let Some(t) = owner {
                effect.frame = Frame {
                    pos: t.pos,
                    vel: t.vel,
                    rot: glam::Mat3::from_quat(t.rotation()),
                };
            } else {
                effect.stop();
                effect.turret = None;
            }
        }
        for t in sim.turrets.iter().filter(|t| t.alive()) {
            let Some(c) = sim.turret_class(t) else {
                continue;
            };
            for (slot, effect) in c.damage_effects.iter().enumerate() {
                if !effect.active(t.health / c.body.max_health)
                    || self.live.iter().any(|e| e.turret == Some((t.id, slot)))
                {
                    continue;
                }
                let frame = Frame {
                    pos: t.pos,
                    vel: t.vel,
                    rot: glam::Mat3::from_quat(t.rotation()),
                };
                if self.spawn(&effect.effect, frame) {
                    self.live.last_mut().unwrap().turret = Some((t.id, slot));
                }
            }
        }
    }
}
