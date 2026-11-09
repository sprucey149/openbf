use super::self_turrets::{
    threat_score, COMBAT_THINK_FACTOR, LEVEL_THINK, RAY_INTERVAL_BY_LEVEL, RAY_REQUESTS_BY_LEVEL,
    THREAT_SWITCH_MARGIN,
};
use super::Sim;
use crate::ai::flyer_weapons::{choose, object_foe};
use crate::collision::HullFilter;
use crate::perception::{in_cone, los_seat};
use crate::vehicle::flyer::{
    FlightPhase, PilotThreat, PilotThreats, ThreatObject, THREAT_FORCED, THREAT_SEEN, THREAT_SLOTS,
    THREAT_UNSEEN,
};
use crate::vehicle::{VehicleHandle, VehicleKind};
use glam::Vec3;

const FLYER_RELATIVE: i32 = 20;
const ARMED_RELATIVE: i32 = 15;
const BUILDING_RELATIVE: i32 = 10;
const FLYER_SEER_VIEW: f32 = 2.0;
const HUMAN_TARGET_VIEW: f32 = 2.0;
const OMNI_SIGHT: f32 = 10.0;

#[derive(Clone, Copy)]
struct Seer {
    handle: VehicleHandle,
    seat: usize,
    pos: Vec3,
    eye: Vec3,
    nose: Vec3,
    team: u8,
    skill: u8,
    view_mult: f32,
    instance: Option<u32>,
}

struct Info {
    pos: Vec3,
    at: Vec3,
    nose: Vec3,
    human: bool,
    relative: i32,
    radius: f32,
    hull: Option<VehicleHandle>,
    instance: Option<u32>,
}

impl Sim {
    pub(super) fn tick_pilot_threats(&mut self) {
        if self.space_assault.is_none() {
            return;
        }
        for i in 0..self.vehicles.len() {
            let v = &self.vehicles[i];
            if !v.alive
                || v.kind != VehicleKind::Flyer
                || v.flyer.phase != FlightPhase::Flying
                || self.tick < v.flyer.threats.next_think
            {
                continue;
            }
            let h = VehicleHandle {
                index: i as u32,
                gen: v.gen,
            };
            if let Some(seer) = self.flyer_seer(h) {
                self.pilot_think(&seer);
            }
        }
    }

    fn flyer_seer(&self, h: VehicleHandle) -> Option<Seer> {
        let v = self
            .vehicle(h)
            .filter(|v| v.kind == VehicleKind::Flyer && v.flyer.phase == FlightPhase::Flying)?;
        self.flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .filter(|p| p.bf2)?;
        let (seat, pilot) = v
            .seats
            .iter()
            .enumerate()
            .find_map(|(i, s)| s.occupant.map(|o| (i, o)))?;
        let u = self.unit(pilot).filter(|u| !u.is_human())?;
        let team = v.team?;
        let look = self.seat_look(pilot)?;
        Some(Seer {
            handle: h,
            seat,
            pos: v.pos,
            eye: look.eye,
            nose: v.rot * Vec3::Z,
            team,
            skill: self.bot_skill(team),
            view_mult: self.unit_tier(u).view_mult,
            instance: v.instance,
        })
    }

    fn threat_info(&self, object: ThreatObject, team: u8) -> Option<Info> {
        match object {
            ThreatObject::Station(i) => {
                let s = self
                    .stations
                    .get(usize::from(i))
                    .filter(|s| !s.destroyed && s.health.alive())?;
                s.team.filter(|t| self.hostile(team, *t))?;
                Some(Info {
                    pos: s.pos,
                    at: s.sphere_centre,
                    nose: Vec3::Z,
                    human: false,
                    relative: BUILDING_RELATIVE,
                    radius: s.sphere_radius.max(s.radius),
                    hull: None,
                    instance: s.instance,
                })
            }
            ThreatObject::Vehicle(h) => {
                let v = self.vehicle(h).filter(|v| v.alive && v.health.alive())?;
                v.team.filter(|t| self.hostile(team, *t))?;
                let human = v
                    .seats
                    .iter()
                    .filter_map(|s| s.occupant)
                    .filter_map(|o| self.unit(o))
                    .any(|o| o.is_human());
                let relative = if matches!(v.kind, VehicleKind::Flyer | VehicleKind::Walker) {
                    FLYER_RELATIVE
                } else if v.kind == VehicleKind::Turret {
                    ARMED_RELATIVE
                } else {
                    BUILDING_RELATIVE
                };
                let (at, radius) = self.vehicle_blast_sphere(v);
                Some(Info {
                    pos: v.pos,
                    at,
                    nose: v.rot * Vec3::Z,
                    human,
                    relative,
                    radius,
                    hull: Some(h),
                    instance: v.instance,
                })
            }
        }
    }

    fn pilot_visual_key(eye: Vec3, at: Vec3, relative: i32) -> i32 {
        (at.distance(eye) as i32).min(1500) * (21 - relative) * 2
    }

    fn pilot_priority(&self, seer: &Seer, t: &PilotThreat) -> i32 {
        let Some(i) = self.threat_info(t.object, seer.team) else {
            return 0;
        };
        if (i.pos - seer.pos).dot(seer.nose) <= 0.0 {
            return 0;
        }
        let key = Self::pilot_visual_key(seer.eye, i.at, i.relative);
        let facing = i.nose.dot((seer.eye - i.at).normalize_or_zero());
        let age = self.tick.saturating_sub(t.seen_at) as f32 / crate::TICK_HZ as f32;
        threat_score(t.status, key, i.human, facing, seer.skill, age)
    }

    fn update_pilot_threat(
        &self,
        seer: &Seer,
        threats: &mut PilotThreats,
        object: ThreatObject,
        status: u8,
    ) {
        let seen = status != THREAT_UNSEEN;
        let found = threats
            .slots
            .iter()
            .position(|t| t.is_some_and(|t| t.object == object));
        let slot = match found {
            Some(i) => i,
            None => {
                if !seen {
                    return;
                }
                let new = PilotThreat {
                    object,
                    status,
                    tested_at: self.tick,
                    seen_at: self.tick,
                };
                let mut worst = self.pilot_priority(seer, &new);
                let mut pick = None;
                for (i, t) in threats.slots.iter().enumerate() {
                    match t {
                        None if worst > 0 => {
                            worst = 0;
                            pick = Some(i);
                        }
                        None => {}
                        Some(t) => {
                            let priority = self.pilot_priority(seer, t);
                            if priority < worst {
                                worst = priority;
                                pick = Some(i);
                            }
                        }
                    }
                }
                let Some(i) = pick else { return };
                i
            }
        };
        let seen_at = if seen {
            self.tick
        } else {
            threats.slots[slot].map_or(0, |t| t.seen_at)
        };
        threats.slots[slot] = Some(PilotThreat {
            object,
            status,
            tested_at: self.tick,
            seen_at,
        });
    }

    fn pilot_can_engage(&self, seer: &Seer, object: ThreatObject) -> bool {
        let Some(v) = self.vehicle(seer.handle) else {
            return false;
        };
        object_foe(self, object, seer.eye)
            .is_some_and(|foe| choose(self, v, seer.handle, seer.seat, &foe, false).is_some())
    }

    pub(super) fn force_pilot_threat(&mut self, target: VehicleHandle, by: ThreatObject) {
        let Some(seer) = self.flyer_seer(target) else {
            return;
        };
        let Some(mut threats) = self.vehicle(target).map(|v| v.flyer.threats) else {
            return;
        };
        if threats.visible_object().is_some() || !self.pilot_can_engage(&seer, by) {
            return;
        }
        self.update_pilot_threat(&seer, &mut threats, by, THREAT_FORCED);
        if let Some(v) = self.vehicle_mut(target) {
            v.flyer.threats = threats;
        }
    }

    fn pilot_candidates(&self, seer: &Seer) -> Vec<(i32, u64, ThreatObject, bool)> {
        let turrets = self
            .building_turrets
            .iter()
            .filter(|t| t.down.is_none())
            .map(|t| ThreatObject::Vehicle(t.handle));
        let stations = (0..self.stations.len()).map(|i| ThreatObject::Station(i as u16));
        let flyers = self
            .alive_vehicles()
            .filter(|(h, v)| {
                *h != seer.handle
                    && v.kind == VehicleKind::Flyer
                    && v.seats.iter().any(|s| s.occupant.is_some())
            })
            .map(|(h, _)| ThreatObject::Vehicle(h));
        let mut list: Vec<(i32, u64, ThreatObject, bool)> = turrets
            .chain(stations)
            .chain(flyers)
            .filter_map(|object| {
                let i = self.threat_info(object, seer.team)?;
                let off = i.at - seer.eye;
                let d2 = off.length_squared();
                let far = (20.0 * i.radius + 30.0)
                    * seer.view_mult
                    * FLYER_SEER_VIEW
                    * if i.human { HUMAN_TARGET_VIEW } else { 1.0 };
                if d2 >= far * far || !in_cone(seer.nose, off, d2, OMNI_SIGHT) {
                    return None;
                }
                if !self.pilot_can_engage(seer, object) {
                    return None;
                }
                let order = match object {
                    ThreatObject::Vehicle(h) => u64::from(h.index),
                    ThreatObject::Station(s) => (1 << 32) + u64::from(s),
                };
                Some((
                    Self::pilot_visual_key(seer.eye, i.at, i.relative),
                    order,
                    object,
                    i.human,
                ))
            })
            .collect();
        list.sort_by_key(|c| (c.0, c.1));
        list
    }

    fn pilot_think(&mut self, seer: &Seer) {
        let Some(mut t) = self.vehicle(seer.handle).map(|v| v.flyer.threats) else {
            return;
        };
        let level = self.self_level(seer.pos, seer.eye, seer.nose);
        let engaged = t.visible_object().is_some();
        for slot in &mut t.slots {
            if slot.is_some_and(|k| self.pilot_priority(seer, &k) == 0) {
                *slot = None;
            }
        }
        let priority = |sim: &Self, t: &PilotThreats, i: usize| {
            t.slots[i].map_or(0, |k| sim.pilot_priority(seer, &k))
        };
        let mut best: Option<(u8, i32)> = None;
        for i in 0..THREAT_SLOTS {
            if t.slots[i].is_some_and(|k| k.status != THREAT_UNSEEN) {
                let score = priority(self, &t, i);
                if best.is_none_or(|(_, b)| score > b) {
                    best = Some((i as u8, score));
                }
            }
        }
        let best_slot = best.map(|(i, _)| i);
        if best_slot != t.visible {
            match (best, t.visible) {
                (Some((i, score)), Some(v)) => {
                    if score > priority(self, &t, usize::from(v)) + THREAT_SWITCH_MARGIN {
                        t.visible = Some(i);
                    }
                }
                _ => t.visible = best_slot,
            }
        }
        t.thinks = t.thinks.wrapping_add(1);
        let factor = if engaged { COMBAT_THINK_FACTOR } else { 1.0 };
        t.next_think = self.tick + (LEVEL_THINK[level] * factor * crate::TICK_HZ as f32) as u64;
        let list = self.pilot_candidates(seer);
        for i in 0..THREAT_SLOTS {
            let Some(k) = t.slots[i] else { continue };
            if k.status == THREAT_SEEN && !list.iter().any(|c| c.2 == k.object) {
                self.update_pilot_threat(seer, &mut t, k.object, THREAT_UNSEEN);
            }
        }
        let visible_human = t
            .visible_object()
            .and_then(|o| self.threat_info(o, seer.team))
            .map(|i| i.human);
        let interval = (RAY_INTERVAL_BY_LEVEL[level] * crate::TICK_HZ as f32) as u64;
        let mut requests = 0;
        let mut seen_flyers: Vec<VehicleHandle> = Vec::new();
        for &(_, _, object, human) in &list {
            if requests >= RAY_REQUESTS_BY_LEVEL[level] {
                break;
            }
            let due = t
                .slots
                .iter()
                .flatten()
                .find(|k| k.object == object)
                .is_none_or(|k| self.tick - k.tested_at >= interval);
            let test = match (human, visible_human) {
                (false, None) => true,
                (false, Some(true)) => false,
                _ => due,
            };
            if !test {
                continue;
            }
            requests += 1;
            let Some(i) = self.threat_info(object, seer.team) else {
                continue;
            };
            let seen = los_seat(
                self,
                seer.eye,
                i.at,
                HullFilter::Except([Some(seer.handle), i.hull]),
                [seer.instance, i.instance],
            );
            let status = if seen { THREAT_SEEN } else { THREAT_UNSEEN };
            self.update_pilot_threat(seer, &mut t, object, status);
            if seen && human {
                t.next_think = self.tick + 1;
            }
            if let (true, ThreatObject::Vehicle(h)) = (seen, object) {
                if self
                    .vehicle(h)
                    .is_some_and(|v| v.kind == VehicleKind::Flyer)
                {
                    seen_flyers.push(h);
                }
            }
        }
        if let Some(v) = self.vehicle_mut(seer.handle) {
            v.flyer.threats = t;
        }
        for h in seen_flyers {
            self.force_pilot_threat(h, ThreatObject::Vehicle(seer.handle));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::UnitSpec;
    use crate::vehicle::flyer::FlyerParams;
    use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon};
    use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
    use crate::weapon::{HealthType, WeaponClass};
    use glam::Quat;

    fn ship(s: &mut Sim, team: u8, pos: Vec3) -> VehicleHandle {
        let h = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(team),
            neutral_when_empty: false,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 100.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let v = s.vehicle_mut(h).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.team = Some(team);
        h
    }

    fn world(foes: &[Vec3]) -> (Sim, Seer, Vec<VehicleHandle>) {
        let mut s = Sim::empty(42);
        s.space_assault = Some(crate::sim::space_assault::SpaceAssault::new(10));
        s.flyer_params = vec![Some(FlyerParams {
            bf2: true,
            ..FlyerParams::default()
        })];
        let mut wc = WeaponClass::bf1_rifle();
        wc.ai_range = [0.0, 500.0, 1000.0];
        wc.ai_target_kind = u32::MAX;
        wc.ordnance.max_damage = 1000.0;
        wc.ordnance.scales.fill(1.0);
        s.weapon_classes[0] = wc;
        s.seat_setup = vec![Some(SeatSetup {
            radius: 1.0,
            boardable: vec![true],
            body_active: vec![],
            exit_drop: 15.0,
            ai_size: 1,
            guns: vec![Some(GunSetup {
                weapons: vec![GunWeapon {
                    class: 0,
                    infinite: true,
                    reserve_clips: 0,
                }],
                fire_point: Vec3::Z,
                pivot: Vec3::ZERO,
                eye: Vec3::ZERO,
                aimer: Aimer::default(),
                pilot: true,
                turn: None,
                framed: false,
                aimer_bind: Quat::IDENTITY,
                gunner: None,
            })],
        })];
        let me = ship(&mut s, 0, Vec3::ZERO);
        let pilot = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        assert!(s.seat_unit(pilot, me, 0));
        s.vehicle_mut(me).unwrap().flyer.phase = FlightPhase::Flying;
        s.arm(me.index as usize);
        let foes = foes.iter().map(|p| ship(&mut s, 1, *p)).collect();
        let seer = s.flyer_seer(me).unwrap();
        (s, seer, foes)
    }

    fn hold(s: &mut Sim, seer: &Seer, slots: &[(VehicleHandle, u8)], visible: Option<u8>) {
        let tick = s.tick;
        let v = s.vehicle_mut(seer.handle).unwrap();
        for (i, (h, status)) in slots.iter().enumerate() {
            v.flyer.threats.slots[i] = Some(PilotThreat {
                object: ThreatObject::Vehicle(*h),
                status: *status,
                tested_at: tick,
                seen_at: tick,
            });
        }
        v.flyer.threats.visible = visible;
    }

    #[test]
    fn a_pilot_drops_a_target_once_it_is_behind_the_nose() {
        let (mut s, seer, foes) = world(&[Vec3::new(0.0, 0.0, 300.0)]);
        hold(&mut s, &seer, &[(foes[0], THREAT_SEEN)], Some(0));
        s.pilot_think(&seer);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert_eq!(t.visible_object(), Some(ThreatObject::Vehicle(foes[0])));
        s.vehicle_mut(foes[0]).unwrap().pos = Vec3::new(0.0, 0.0, -300.0);
        s.pilot_think(&seer);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert!(t.slots.iter().all(Option::is_none));
        assert_eq!(t.visible_object(), None);
    }

    #[test]
    fn a_pilot_switches_only_for_a_clearly_better_target() {
        let at = [Vec3::new(0.0, 0.0, 300.0), Vec3::new(0.0, 0.0, 280.0)];
        let (mut s, seer, foes) = world(&at);
        hold(
            &mut s,
            &seer,
            &[(foes[0], THREAT_SEEN), (foes[1], THREAT_SEEN)],
            Some(0),
        );
        s.pilot_think(&seer);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert_eq!(t.visible_object(), Some(ThreatObject::Vehicle(foes[0])));
        hold(
            &mut s,
            &seer,
            &[(foes[0], THREAT_FORCED), (foes[1], THREAT_SEEN)],
            Some(0),
        );
        s.pilot_think(&seer);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert_eq!(t.visible_object(), Some(ThreatObject::Vehicle(foes[1])));
    }

    #[test]
    fn an_engaged_pilot_thinks_at_the_combat_rate() {
        let (mut s, seer, foes) = world(&[Vec3::new(0.0, 0.0, 300.0)]);
        let level = s.self_level(seer.pos, seer.eye, seer.nose);
        let idle = (LEVEL_THINK[level] * crate::TICK_HZ as f32) as u64;
        let busy = (LEVEL_THINK[level] * COMBAT_THINK_FACTOR * crate::TICK_HZ as f32) as u64;
        s.pilot_think(&seer);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert_eq!(t.next_think, s.tick + idle);
        hold(&mut s, &seer, &[(foes[0], THREAT_SEEN)], Some(0));
        s.pilot_think(&seer);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert_eq!(t.next_think, s.tick + busy);
    }

    #[test]
    fn a_busy_pilot_ignores_a_forced_threat() {
        let at = [Vec3::new(0.0, 0.0, 300.0), Vec3::new(0.0, 0.0, 280.0)];
        let (mut s, seer, foes) = world(&at);
        hold(&mut s, &seer, &[(foes[0], THREAT_SEEN)], Some(0));
        s.force_pilot_threat(seer.handle, ThreatObject::Vehicle(foes[1]));
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        assert!(t.slots[1..].iter().all(Option::is_none));
    }

    #[test]
    fn an_idle_pilot_turns_on_whoever_saw_him() {
        let (mut s, seer, foes) = world(&[Vec3::new(0.0, 0.0, 300.0)]);
        let foe = ThreatObject::Vehicle(foes[0]);
        assert!(s.pilot_can_engage(&seer, foe));
        s.force_pilot_threat(seer.handle, foe);
        let t = s.vehicle(seer.handle).unwrap().flyer.threats;
        let slot = t.slots.iter().flatten().find(|k| k.object == foe).unwrap();
        assert_eq!(slot.status, THREAT_FORCED);
    }
}
