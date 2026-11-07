use super::building_turrets::TurretFall;
use super::state::{Event, Sim};
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::VehicleHandle;

pub const DEFAULT_SCORE_LIMIT: u32 = 180;
pub const FIGHTER_KILL_POINTS: i32 = 3;
pub const UNMANNED_FLYER_POINTS: i32 = 1;
pub const KILL_AI_POINTS: i32 = 1;
pub const KILL_HUMAN_POINTS: i32 = 2;
pub const KILL_HERO_POINTS: i32 = 5;
pub const KILL_AT_POST_POINTS: i32 = 2;
pub const SELF_KILL_POINTS: i32 = -1;
pub const TEAM_KILL_POINTS: i32 = -2;
pub const POST_KILL_RANGE_SQ: f32 = 400.0;
pub const SPENT_MAX_HEALTH: f32 = 99_999_997_952.0;

#[derive(Clone, Debug, PartialEq)]
pub struct CriticalSystem {
    pub station: u16,
    pub points: i32,
    pub full_health: f32,
    pub objective: Option<u16>,
    pub shown: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShieldGroup {
    pub objects: Vec<u16>,
    pub controller: Option<u16>,
    pub max: f32,
    pub add: f32,
    pub rate: f32,
    pub shield: f32,
    pub all_down: bool,
    pub lock_when_down: bool,
}

impl ShieldGroup {
    pub fn new(objects: Vec<u16>, controller: Option<u16>, max: f32, add: f32) -> Self {
        Self {
            objects,
            controller,
            max,
            add,
            rate: add,
            shield: max,
            all_down: false,
            lock_when_down: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkedTurretSet {
    pub mainframe: Option<u16>,
    pub team: u8,
    pub turrets: Vec<u32>,
    pub disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrigateTurrets {
    pub frigate: u16,
    pub turrets: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CapitalMeter {
    pub shield: f32,
    pub hull: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpaceAssault {
    pub score_limit: u32,
    pub points: [i32; 2],
    pub critical: Vec<CriticalSystem>,
    pub shields: Vec<ShieldGroup>,
    pub destroyables: Vec<Vec<Vec<u16>>>,
    pub turrets: Vec<LinkedTurretSet>,
    pub frigates: Vec<FrigateTurrets>,
    pub regen: Vec<(u16, f32)>,
    pub objectives_left: Vec<u16>,
    pub lock_on: Vec<u16>,
    pub balance: super::space_balance::SpaceBalance,
}

impl SpaceAssault {
    pub fn new(score_limit: u32) -> Self {
        Self {
            score_limit,
            points: [0; 2],
            critical: Vec::new(),
            shields: Vec::new(),
            destroyables: Vec::new(),
            turrets: Vec::new(),
            frigates: Vec::new(),
            regen: Vec::new(),
            objectives_left: Vec::new(),
            lock_on: Vec::new(),
            balance: super::space_balance::SpaceBalance::default(),
        }
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for p in self.points {
            eat(&p.to_le_bytes());
        }
        for g in &self.shields {
            eat(&g.shield.to_bits().to_le_bytes());
            eat(&g.rate.to_bits().to_le_bytes());
            eat(&[u8::from(g.all_down)]);
        }
        for t in &self.turrets {
            eat(&[u8::from(t.disabled)]);
        }
        for left in &self.objectives_left {
            eat(&left.to_le_bytes());
        }
        self.balance.hash_into(eat);
    }
}

impl Sim {
    pub fn install_space_assault(&mut self, sa: SpaceAssault) {
        for g in &sa.shields {
            for &o in &g.objects {
                if let Some(s) = self.stations.get_mut(usize::from(o)) {
                    s.health.shield = g.shield;
                    s.health.shield_max = g.max;
                }
            }
        }
        self.battle.unlimited_reinforcements = true;
        self.space_assault = Some(sa);
    }

    pub fn space_team_points(&self) -> Option<([i32; 2], u32)> {
        self.space_assault
            .as_ref()
            .map(|sa| (sa.points, sa.score_limit))
    }

    pub fn capital_meters(&self) -> Option<[CapitalMeter; 2]> {
        let sa = self.space_assault.as_ref()?;
        let mut meters = [CapitalMeter::default(); 2];
        for g in &sa.shields {
            let Some(team) = g
                .controller
                .and_then(|c| self.stations.get(usize::from(c)))
                .and_then(|s| s.team)
                .filter(|t| *t < 2)
            else {
                continue;
            };
            if g.max > 0.0 {
                meters[usize::from(team)].shield = (g.shield / g.max).clamp(0.0, 1.0);
            }
        }
        let mut hull = [[0.0f32; 2]; 2];
        for c in &sa.critical {
            let Some(s) = self.stations.get(usize::from(c.station)) else {
                continue;
            };
            let Some(team) = s.team.filter(|t| *t < 2) else {
                continue;
            };
            hull[usize::from(team)][0] += s.health.cur.clamp(0.0, c.full_health);
            hull[usize::from(team)][1] += c.full_health;
        }
        for (m, h) in meters.iter_mut().zip(hull) {
            if h[1] > 0.0 {
                m.hull = (h[0] / h[1]).clamp(0.0, 1.0);
            }
        }
        Some(meters)
    }

    fn space_add_points(&mut self, team: u8, points: i32) {
        let Some(sa) = self.space_assault.as_mut() else {
            return;
        };
        let t = usize::from(team);
        if t >= sa.points.len() || points == 0 {
            return;
        }
        sa.points[t] += points;
        let reached = sa.points[t] >= sa.score_limit as i32;
        if reached && self.outcome.is_none() {
            self.outcome = Some(team);
            self.events.push(Event::TeamDefeated { team: 1 - team });
        }
    }

    fn space_credit_team(&self, killer: UnitHandle) -> Option<u8> {
        if killer.index == u32::MAX {
            return self.vehicles.get(killer.gen as usize).and_then(|v| v.team);
        }
        self.units
            .get(killer.index as usize)
            .filter(|u| u.gen == killer.gen)
            .map(|u| u.team)
    }

    fn space_kill_near_post(&self, victim: UnitHandle, killer_team: u8) -> bool {
        let Some(v) = self.unit(victim) else {
            return false;
        };
        let (pos, team) = (v.soldier.pos, v.team);
        self.cps.iter().take(16).any(|cp| {
            let not_friend = cp.owner.is_none_or(|owner| self.hostile(team, owner));
            let defended =
                cp.owner == Some(killer_team) && not_friend && cp.def.capture.contains(pos);
            defended || (not_friend && (cp.def.pos - pos).length_squared() < POST_KILL_RANGE_SQ)
        })
    }

    pub(super) fn space_unit_killed(&mut self, victim: UnitHandle, killer: Option<UnitHandle>) {
        if self.space_assault.is_none() {
            return;
        }
        let Some(v) = self.unit(victim) else {
            return;
        };
        let (victim_team, victim_human) = (v.team, v.is_human());
        let credited = killer.and_then(|k| self.space_credit_team(k).map(|team| (k, team)));
        let Some((k, team)) = credited else {
            if victim_human {
                self.space_add_points(victim_team, SELF_KILL_POINTS);
            }
            return;
        };
        let killer_human = self.unit(k).is_some_and(|u| u.is_human());
        if k == victim || self.allied(team, victim_team) {
            if killer_human {
                let points = if k == victim {
                    SELF_KILL_POINTS
                } else {
                    TEAM_KILL_POINTS
                };
                self.space_add_points(team, points);
            }
            return;
        }
        let points = if k.index != u32::MAX && self.space_kill_near_post(victim, team) {
            KILL_AT_POST_POINTS
        } else if self.is_hero(victim) {
            KILL_HERO_POINTS
        } else if victim_human {
            KILL_HUMAN_POINTS
        } else {
            KILL_AI_POINTS
        };
        self.space_add_points(team, points);
    }

    pub(super) fn space_flyer_destroyed(&mut self, h: VehicleHandle, killer: Option<UnitHandle>) {
        if self.space_assault.is_none() {
            return;
        }
        let Some(team) = killer.and_then(|k| self.space_credit_team(k)) else {
            return;
        };
        let Some(v) = self.vehicle(h) else {
            return;
        };
        let flyer = self
            .flyer_params
            .get(v.class as usize)
            .is_some_and(Option::is_some);
        if v.team == Some(team) {
            return;
        }
        if !flyer {
            let armed_building = self.building_turrets.iter().any(|t| t.handle == h);
            let credited = killer.is_some_and(|k| k.index != u32::MAX);
            if armed_building && credited && v.seats.iter().all(|s| s.occupant.is_none()) {
                self.space_add_points(team, UNMANNED_FLYER_POINTS);
            }
            return;
        }
        let manned = v.seats.iter().any(|s| s.occupant.is_some());
        self.space_add_points(
            team,
            if manned {
                FIGHTER_KILL_POINTS
            } else {
                UNMANNED_FLYER_POINTS
            },
        );
    }

    pub(super) fn space_station_damaged(&mut self, station: u16) {
        let Some(sa) = self.space_assault.as_mut() else {
            return;
        };
        let Some(i) = sa.shields.iter().position(|g| g.objects.contains(&station)) else {
            return;
        };
        let shield = self.stations[usize::from(station)].health.shield;
        let g = &mut sa.shields[i];
        g.shield = shield;
        if shield <= 0.0 {
            g.rate = 0.0;
        }
        let collapse = shield <= 0.0 && !g.all_down;
        let controller = g.controller;
        if collapse {
            g.all_down = true;
        }
        self.space_write_shield(i);
        if collapse {
            self.space_shields_changed(i, false);
            if let Some(c) = controller {
                self.space_kill_station(c);
            }
        }
    }

    fn space_write_shield(&mut self, group: usize) {
        let Some(sa) = self.space_assault.as_ref() else {
            return;
        };
        let g = &sa.shields[group];
        for &o in &g.objects {
            if let Some(s) = self.stations.get_mut(usize::from(o)) {
                s.health.shield = g.shield;
            }
        }
    }

    pub(super) fn space_kill_station(&mut self, station: u16) {
        let Some(s) = self.stations.get(usize::from(station)) else {
            return;
        };
        if s.destroyed {
            return;
        }
        self.station_dies(station, None);
    }

    pub(super) fn space_station_destroyed(&mut self, station: u16, attacker: Option<UnitHandle>) {
        let Some(sa) = self.space_assault.as_ref() else {
            return;
        };
        let system = sa.critical.iter().find(|c| c.station == station);
        let critical = system.map(|c| c.points);
        let objective = system.and_then(|c| c.objective);
        let collapsing: Vec<usize> = sa
            .shields
            .iter()
            .enumerate()
            .filter(|(_, g)| g.controller == Some(station) && !g.all_down)
            .map(|(i, _)| i)
            .collect();
        let linked: Vec<u16> = sa
            .destroyables
            .iter()
            .filter(|sets| sets.iter().any(|set| set.contains(&station)))
            .flat_map(|sets| {
                sets.iter()
                    .filter(|set| {
                        set.contains(&station)
                            && set.iter().all(|&o| self.stations[usize::from(o)].destroyed)
                    })
                    .flat_map(move |_| sets.iter().flatten().copied())
            })
            .filter(|&o| o != station)
            .collect();
        let hidden: Vec<u32> = sa
            .frigates
            .iter()
            .filter(|f| f.frigate == station)
            .flat_map(|f| f.turrets.iter().copied())
            .collect();
        let already_spent = self.stations[usize::from(station)].health.max == SPENT_MAX_HEALTH;
        if let Some(points) = critical {
            if !already_spent {
                self.space_objective_progress(station, objective);
                self.space_score_system(station, points, attacker);
                self.stations[usize::from(station)].health.max = SPENT_MAX_HEALTH;
            }
        }
        for i in collapsing {
            if let Some(g) = self.space_assault.as_mut().map(|sa| &mut sa.shields[i]) {
                g.all_down = true;
                g.shield = 0.0;
                g.rate = 0.0;
            }
            self.space_shields_changed(i, false);
            self.space_write_shield(i);
        }
        for o in linked {
            self.space_kill_station(o);
        }
        for instance in hidden {
            self.space_hide_turret(instance);
        }
    }

    fn space_shields_changed(&mut self, group: usize, up: bool) {
        self.events.push(Event::SpaceLinkChanged {
            kind: 0,
            index: group as u16,
            up,
        });
        let Some(sa) = self.space_assault.as_mut() else {
            return;
        };
        if !sa.shields[group].lock_when_down {
            return;
        }
        let objects = sa.shields[group].objects.clone();
        if up {
            sa.lock_on.retain(|o| !objects.contains(o));
        } else {
            for o in objects {
                if !sa.lock_on.contains(&o) {
                    sa.lock_on.push(o);
                }
            }
        }
    }

    fn space_objective_progress(&mut self, station: u16, objective: Option<u16>) {
        let Some(objective) = objective else {
            return;
        };
        let Some(left) = self
            .space_assault
            .as_mut()
            .and_then(|sa| sa.objectives_left.get_mut(usize::from(objective)))
        else {
            return;
        };
        if *left == 0 {
            return;
        }
        *left -= 1;
        if *left > 0 {
            return;
        }
        if let Some(owner) = self.stations[usize::from(station)].team {
            self.events
                .push(Event::SpaceObjectiveDown { objective, owner });
        }
    }

    fn space_score_system(&mut self, station: u16, points: i32, attacker: Option<UnitHandle>) {
        let Some(u) = attacker.and_then(|k| self.unit(k)) else {
            return;
        };
        let (team, ai) = (u.team, u.controller == Controller::Ai);
        let own = self.stations[usize::from(station)].team == Some(team);
        let points = if own { -points } else { points };
        if ai && points < 1 {
            return;
        }
        self.space_add_points(team, points);
    }

    fn space_hide_turret(&mut self, instance: u32) {
        let Some(i) = self
            .building_turrets
            .iter()
            .position(|t| t.instance == instance)
        else {
            return;
        };
        let t = &mut self.building_turrets[i];
        t.fall = TurretFall::Hidden;
        t.fallen_parts.clear();
        t.spec.max_health = SPENT_MAX_HEALTH;
        let (handle, pose) = (t.handle, t.pose());
        if t.down.is_some() {
            self.collision.replace_instance(instance, &[], pose);
        } else {
            self.destroy_vehicle(handle, None);
        }
    }

    pub(super) fn tick_space_assault(&mut self, dt: f32) {
        let Some(mut sa) = self.space_assault.take() else {
            return;
        };
        for &(station, add) in &sa.regen {
            if let Some(s) = self.stations.get_mut(usize::from(station)) {
                if !s.destroyed && s.health.cur < s.health.max {
                    s.health.cur = (s.health.cur + dt * add).min(s.health.max);
                }
            }
        }
        let mut returned = Vec::new();
        for (group, g) in sa.shields.iter_mut().enumerate() {
            let controller_up = g
                .controller
                .and_then(|c| self.stations.get(usize::from(c)))
                .is_some_and(|s| !s.destroyed);
            if g.all_down && controller_up {
                returned.push(group);
                g.all_down = false;
                g.rate = g.add;
                g.shield = g.max;
            } else if g.rate != 0.0 && g.shield < g.max {
                g.shield = (g.shield + dt * g.rate).clamp(0.0, g.max);
            }
            for &o in &g.objects {
                if let Some(s) = self.stations.get_mut(usize::from(o)) {
                    s.health.shield = g.shield;
                }
            }
        }
        for (index, set) in sa.turrets.iter_mut().enumerate() {
            let down = set
                .mainframe
                .and_then(|m| self.stations.get(usize::from(m)))
                .is_some_and(|s| s.destroyed);
            if !down && !set.disabled {
                continue;
            }
            if down != set.disabled {
                self.events.push(Event::SpaceLinkChanged {
                    kind: 1,
                    index: index as u16,
                    up: !down,
                });
            }
            let team = (!down).then_some(set.team);
            set.disabled = down;
            for &instance in &set.turrets {
                let Some(h) = self
                    .building_turrets
                    .iter()
                    .find(|t| t.instance == instance)
                    .map(|t| t.handle)
                else {
                    continue;
                };
                if let Some(v) = self.vehicle_mut(h) {
                    v.team = team;
                }
            }
        }
        self.space_assault = Some(sa);
        for group in returned {
            self.space_shields_changed(group, true);
        }
    }
}

#[cfg(test)]
mod tests;
