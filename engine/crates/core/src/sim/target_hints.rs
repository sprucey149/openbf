use super::lock_on::{lock_score, LockTarget};
use super::state::Sim;
use crate::fmath::FloatExt;
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
    /// Every player slot's hint list, in slot order. The lists grow with
    /// `players` (a caller may assign `players` without
    /// [`Sim::sync_slot_tables`]); a list past the last slot is kept.
    pub(super) fn tick_target_hints(&mut self) {
        if self.target_hints.len() < self.players.len() {
            self.target_hints
                .resize_with(self.players.len(), Default::default);
        }
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
            r.point = p.pos + p.up * (p.radius * LIFT_FACTOR).fmin(LIFT_MAX);
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
                    point: p.pos + p.up * (p.radius * LIFT_FACTOR).fmin(LIFT_MAX),
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
            if self.target_hints.is_empty() {
                self.target_hints.push(TargetHints::default());
            }
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

    fn fighter(s: &mut Sim, team: u8, pos: Vec3) -> crate::vehicle::VehicleHandle {
        use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
        s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(team),
            neutral_when_empty: false,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 100.0,
            health_type: crate::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        })
    }

    /// Issue #168: a human in slot 2 (no one in 0 and 1) gets his own hint
    /// list, and only his: the lists are per slot, not the first four.
    #[test]
    fn a_third_slots_pilot_gets_his_own_target_hints() {
        use crate::unit::UnitSpec;
        use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon};
        let mut s = Sim::empty(1);
        s.flyer_params = vec![Some(crate::vehicle::flyer::FlyerParams::default())];
        let w = &mut s.weapon_classes[0];
        w.target_kind = super::super::lock_on::VEHICLE_KIND_BIT;
        w.target_lock_max = 1000.0;
        w.target_lock_lose = 1100.0;
        s.seat_setup = vec![Some(crate::vehicle::SeatSetup {
            radius: 5.0,
            body_active: vec![],
            boardable: vec![true],
            exit_drop: 10.0,
            ai_size: 1,
            guns: vec![Some(GunSetup {
                weapons: vec![GunWeapon {
                    class: 0,
                    infinite: true,
                    reserve_clips: 0,
                    fire_points: Vec::new(),
                }],
                fire_point: Vec3::ZERO,
                pivot: Vec3::ZERO,
                eye: Vec3::ZERO,
                aimer: Aimer::default(),
                pilot: true,
                turn: None,
                framed: false,
                aimer_bind: glam::Quat::IDENTITY,
                gunner: None,
            })],
        })];
        let own = fighter(&mut s, 0, Vec3::ZERO);
        let p = s
            .spawn(&UnitSpec::player_in_slot(2, 0, Vec3::ZERO))
            .unwrap();
        assert!(s.seat_unit(p, own, 0));
        assert_eq!(s.players.len(), 3);
        assert!(s.target_hints.len() >= 3, "grown with the slot tables");
        let (pos, dir) = s.flyer_ray(p);
        let foe = fighter(&mut s, 1, pos + dir * 200.0);
        s.tick_target_hints();
        let target = LockTarget::Aim(crate::sim::input::AimTarget::Vehicle(foe));
        let listed: Vec<bool> = s
            .target_hints
            .iter()
            .map(|l| l.records().iter().any(|r| r.target == target))
            .collect();
        assert_eq!(listed, [false, false, true]);
        assert!(s.on_a_hint_list(target));
    }
}
