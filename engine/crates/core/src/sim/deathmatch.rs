use super::{Event, Sim};
use crate::unit::UnitHandle;
use crate::TICK_HZ;

pub const ASSAULT_SCORE_LIMIT: u32 = 180;

const CLASS_RESERVE_SECS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deathmatch {
    pub score_limit: u32,
    pub members: [u32; 2],
    pub points: [u32; 2],
}

impl Deathmatch {
    pub fn new(score_limit: u32, members: [u32; 2]) -> Self {
        Self {
            score_limit,
            members,
            points: [0; 2],
        }
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"DEATHMATCH");
        eat(&self.score_limit.to_le_bytes());
        for team in 0..2 {
            eat(&self.members[team].to_le_bytes());
            eat(&self.points[team].to_le_bytes());
        }
    }
}

impl Sim {
    pub(super) fn deathmatch_kill(&mut self, killer: UnitHandle, victim_team: u8) {
        if self.outcome.is_some() {
            return;
        }
        let Some(killer_team) = self.units.get(killer.index as usize).map(|u| u.team) else {
            return;
        };
        if killer_team == victim_team || killer_team > 1 {
            return;
        }
        let Some(deathmatch) = self.deathmatch.as_mut() else {
            return;
        };
        let team = usize::from(killer_team);
        deathmatch.points[team] += 1;
        if deathmatch.points[team] >= deathmatch.score_limit {
            self.outcome = Some(killer_team);
            self.events.push(Event::TeamDefeated {
                team: 1 - killer_team,
            });
        }
    }

    pub(super) fn step_deathmatch(&mut self) {
        let Some(members) = self.deathmatch.as_ref().map(|d| d.members) else {
            return;
        };
        if self.outcome.is_some() || !self.tick.is_multiple_of(u64::from(TICK_HZ)) {
            return;
        }
        for team in 0..2u8 {
            let fielded = self.units.iter().filter(|u| u.team == team).count() as u32;
            if fielded >= members[usize::from(team)]
                || self.battle.teams[usize::from(team)].alive_ai >= self.ai_limit(team)
            {
                continue;
            }
            let points = self.spawn_points(team);
            if points.is_empty() {
                continue;
            }
            let (pos, yaw) = points[self.lcg.index(points.len() as u32) as usize];
            self.spawn_ai_roster_slot(team, pos, yaw);
        }
    }

    pub(crate) fn assault_class(&mut self, team: u8, asking: Option<usize>) -> Option<u8> {
        let table = usize::from(team.min(1));
        let classes = self.classes[table].len();
        let mut assigned = vec![0u32; classes];
        let mut reserve = self.tick < CLASS_RESERVE_SECS * u64::from(TICK_HZ);
        for (i, unit) in self.units.iter().enumerate() {
            if unit.team != team {
                continue;
            }
            reserve |= unit.is_human();
            if Some(i) != asking {
                if let Some(count) = assigned.get_mut(usize::from(unit.spec.class)) {
                    *count += 1;
                }
            }
        }
        let mut order: Vec<usize> = (0..classes).collect();
        for left in (2..=classes).rev() {
            let pick = self.lcg.index(left as u32) as usize;
            order.swap(pick, left - 1);
        }
        let table = &self.classes[table];
        let eligible = |class: usize| {
            let most = table[class].max.unwrap_or(u32::MAX);
            !reserve || assigned[class].saturating_add(1) < most
        };
        order
            .iter()
            .copied()
            .find(|&class| eligible(class) && assigned[class] < table[class].min)
            .or_else(|| {
                order.iter().copied().find(|&class| {
                    eligible(class) && assigned[class] < table[class].max.unwrap_or(u32::MAX)
                })
            })
            .map(|class| class as u8)
    }

    pub fn team_points(&self) -> Option<([u32; 2], u32)> {
        self.deathmatch.as_ref().map(|d| (d.points, d.score_limit))
    }
}

#[cfg(test)]
mod tests;
