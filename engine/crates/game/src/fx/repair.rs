//! The fusion cutter's repair-pulse muzzle effects: they follow the owner, and
//! the first burst sits at the drawn weapon tip, placed every drawn frame
//! from the pose that frame draws ([`Fx::advance_frame`]). Card TBLD; no
//! analysis doc covers the effect placement.

use super::{emitter::Frame, Fx};
use core::sim::{Event, Sim};
use glam::{Mat3, Vec3};

fn frame(u: &core::unit::Unit) -> Frame {
    let (forward, right, up) = u.aim_frame();
    Frame {
        pos: u.muzzle(),
        rot: Mat3::from_cols(-right, up, forward),
        vel: u.soldier.vel,
    }
}

impl Fx {
    pub(super) fn follow_repairs(&mut self, sim: &Sim) {
        for effect in &mut self.live {
            let Some((h, class)) = effect.repair else {
                continue;
            };
            let owner = sim.unit(h).filter(|_| {
                crate::repair_feedback::active(sim, h).is_some_and(|(c, _)| c == class)
            });
            if let Some(u) = owner {
                effect.frame = frame(u);
            } else {
                effect.stop();
                effect.repair = None;
            }
        }
        for event in &sim.events {
            let Event::Repair {
                unit, weapon_class, ..
            } = event
            else {
                continue;
            };
            let Some(u) = sim.unit(*unit) else {
                continue;
            };
            let Some(wc) = sim.weapon_classes.get(*weapon_class as usize) else {
                continue;
            };
            let Some(name) = wc.muzzle_effect.as_deref() else {
                continue;
            };
            if self.spawn(name, frame(u)) {
                self.live.last_mut().unwrap().repair = Some((*unit, *weapon_class as usize));
            }
        }
    }

    /// Put the repair pulses and the charge glows (`fx/charge.rs`) on the
    /// drawn weapon tips, `points` by unit index.
    pub fn set_repair_origins(&mut self, points: &[Option<Vec3>]) {
        for effect in &mut self.live {
            if let Some((h, _)) = effect.repair.or(effect.charge) {
                if let Some(Some(pos)) = points.get(h.index as usize) {
                    if effect.t == 0.0 {
                        let delta = *pos - effect.frame.pos;
                        for instance in &mut effect.instances {
                            instance.translate(delta);
                        }
                    }
                    effect.frame.pos = *pos;
                }
            }
        }
    }
}
