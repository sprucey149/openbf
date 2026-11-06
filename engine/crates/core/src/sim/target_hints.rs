use super::lock_on::{lock_score, LockTarget};
use super::state::Sim;
use glam::Vec3;

const MAX_RECORDS: usize = 64;
const KEEP_SECS: f32 = 2.0;
const KEEP_DOT: f32 = 0.4;
const ADD_DOT: f32 = 0.5;
const LIFT_FACTOR: f32 = 0.7;
const LIFT_MAX: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetHint {
    pub target: LockTarget,
    pub point: Vec3,
    pub friendly: bool,
    pub visible: bool,
    timer: f32,
    probed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetHints {
    records: Vec<TargetHint>,
    tick: Option<u64>,
}

impl TargetHints {
    pub fn records(&self) -> &[TargetHint] {
        &self.records
    }
}

impl Sim {
    pub(super) fn tick_target_hints(&mut self) {
        let mut hints = std::mem::take(&mut self.target_hints);
        for (slot, list) in hints.iter_mut().enumerate() {
            self.update_target_hints(list, slot);
        }
        self.target_hints = hints;
    }

    pub(super) fn on_a_hint_list(&self, target: LockTarget) -> bool {
        self.target_hints
            .iter()
            .any(|list| list.records.iter().any(|r| r.target == target))
    }

    pub fn update_target_hints(&self, hints: &mut TargetHints, slot: usize) {
        if hints.tick == Some(self.tick) {
            return;
        }
        let dt = hints.tick.map_or(0, |t| self.tick.saturating_sub(t)) as f32 * crate::TICK_DT;
        hints.tick = Some(self.tick);
        let mounted = self
            .players
            .get(slot)
            .copied()
            .flatten()
            .and_then(|h| Some((h, self.flyer_of(h)?, self.lock_gun(h)?)));
        let Some((h, own, gun)) = mounted else {
            hints.records.clear();
            return;
        };
        let c = &self.weapon_classes[gun];
        let (mask, add_range, keep_range) = (c.target_kind, c.target_lock_max, c.target_lock_lose);
        let team = self.units[h.index as usize].team;
        let (pos, dir) = self.flyer_ray(h);
        let own_building = self.vehicle(own).and_then(|v| v.instance);
        hints.records.retain_mut(|r| {
            let Some(p) = self.lock_probe(r.target).filter(|p| mask & p.kind_bit != 0) else {
                return false;
            };
            let off = p.pos - pos;
            let held = off.normalize_or_zero().dot(dir) >= KEEP_DOT
                && off.length_squared() <= keep_range * keep_range
                && self.lock_clear(&p, own, own_building, pos);
            if held {
                r.timer = 0.0;
            } else {
                r.timer += dt;
                if r.timer >= KEEP_SECS {
                    return false;
                }
            }
            r.point = p.pos + p.up * (p.radius * LIFT_FACTOR).min(LIFT_MAX);
            r.friendly = self.relation_to(team, p.team) >= 0;
            true
        });
        if hints.records.len() < MAX_RECORDS {
            let mut best: Option<(f32, LockTarget)> = None;
            for t in self.lock_candidates(own, h) {
                if hints.records.iter().any(|r| r.target == t) {
                    continue;
                }
                let Some(p) = self.lock_probe(t).filter(|p| mask & p.kind_bit != 0) else {
                    continue;
                };
                let off = p.pos - pos;
                if off.length_squared() > add_range * add_range
                    || off.normalize_or_zero().dot(dir) < ADD_DOT
                {
                    continue;
                }
                let score = lock_score(off, dir);
                if best.is_some_and(|(b, _)| b <= score) {
                    continue;
                }
                if self.lock_clear(&p, own, own_building, pos) {
                    best = Some((score, t));
                }
            }
            if let Some((target, p)) = best.and_then(|(_, t)| Some((t, self.lock_probe(t)?))) {
                hints.records.push(TargetHint {
                    target,
                    point: p.pos + p.up * (p.radius * LIFT_FACTOR).min(LIFT_MAX),
                    friendly: self.relation_to(team, p.team) >= 0,
                    visible: false,
                    timer: 0.0,
                    probed: true,
                });
            }
        }
        match hints.records.iter_mut().find(|r| !r.probed) {
            Some(r) => {
                r.probed = true;
                r.visible = self
                    .lock_probe(r.target)
                    .is_some_and(|p| self.lock_clear(&p, own, own_building, pos));
            }
            None => {
                for r in &mut hints.records {
                    r.probed = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Sim {
        pub(crate) fn target_hint_for_test(
            &mut self,
            vehicle: crate::vehicle::VehicleHandle,
        ) -> bool {
            let target = LockTarget::Aim(crate::sim::input::AimTarget::Vehicle(vehicle));
            self.target_hints[0].records.push(TargetHint {
                target,
                point: Vec3::ZERO,
                friendly: false,
                visible: true,
                timer: 0.0,
                probed: true,
            });
            self.on_a_hint_list(target)
        }
    }
}
