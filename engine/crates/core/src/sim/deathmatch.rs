use super::{Event, Sim};
use crate::unit::UnitHandle;
use crate::TICK_HZ;

pub const ASSAULT_SCORE_LIMIT: u32 = 180;
pub const XL_SCORE_LIMIT: u32 = 350;

/// The lowest Hunt ease percent (the maintainer 2026-11-21): no side's bots
/// go below it.
pub const HUNT_SKILL_FLOOR: u32 = 30;

/// The longest reaction an eased Hunt bot may have, ticks (1 s): it still
/// fires within about a second of a clear sight (the maintainer 2026-11-21).
pub const HUNT_REACTION_MAX: u64 = TICK_HZ as u64;

const CLASS_RESERVE_SECS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deathmatch {
    /// Points each team needs to win, in team order. Hero Assault and an
    /// even Hunt use one number for both; a Hunt may set a side its own
    /// target (OURS, the maintainer 2026-11-20: balance by points to win
    /// within 60/40, never by health or team size).
    pub score_limit: [u32; 2],
    pub members: [u32; 2],
    pub points: [u32; 2],
    /// Hero Assault, Hunt or XL. Every enemy kill scores 1 in each.
    kind: Kind,
    /// Hunt-only bot ease per side, in percent, [`HUNT_SKILL_FLOOR`]..=100
    /// (100 plays the tier as it is). OURS, the maintainer 2026-11-20/21:
    /// Hunt is for fun, so its bots may be easier than the tier; no other
    /// mode reads it. An eased bot keeps its skill level and mainly misses
    /// more ([`Sim::hunt_aim_widen`]); it reacts a little slower, never past
    /// [`HUNT_REACTION_MAX`] ([`Sim::ai_tier`]), and cycles a weapon that
    /// needs no aim more slowly ([`Sim::hunt_weapon_dt`]).
    /// measure: the Hunt easing refit's bots-only win shares per percent.
    skill_pct: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    HeroAssault,
    Hunt,
    Xl,
}

impl Deathmatch {
    pub fn new(score_limit: u32, members: [u32; 2]) -> Self {
        Self {
            score_limit: [score_limit; 2],
            members,
            points: [0; 2],
            kind: Kind::HeroAssault,
            skill_pct: [100; 2],
        }
    }

    pub fn hunt(score_limit: u32, members: [u32; 2]) -> Self {
        Self {
            kind: Kind::Hunt,
            ..Self::new(score_limit, members)
        }
    }

    /// A Hunt where team 0 needs `targets[0]` points and team 1 `targets[1]`.
    pub fn hunt_with_targets(targets: [u32; 2], members: [u32; 2]) -> Self {
        Self {
            score_limit: targets,
            ..Self::hunt(targets[0], members)
        }
    }

    /// This Hunt with each side's bots eased to `pct` percent (each clamped
    /// to [`HUNT_SKILL_FLOOR`]..=100). Hero Assault keeps 100.
    pub fn with_skill_pct(self, pct: [u32; 2]) -> Self {
        Self {
            skill_pct: if self.kind == Kind::Hunt {
                pct.map(|p| p.clamp(HUNT_SKILL_FLOOR, 100))
            } else {
                [100; 2]
            },
            ..self
        }
    }

    /// Each side's Hunt bot skill scale in percent (100 = the tier's own).
    pub fn skill_pct(&self) -> [u32; 2] {
        self.skill_pct
    }

    /// Which side leads when the clock runs out: the larger share of its own
    /// target, compared exactly (points[0] / target[0] against
    /// points[1] / target[1]). Even targets compare raw points.
    pub fn timer_leader(&self) -> std::cmp::Ordering {
        let [a, b] = self.points.map(u64::from);
        let [x, y] = self.score_limit.map(|n| u64::from(n.max(1)));
        (a * y).cmp(&(b * x))
    }

    pub fn is_hero_assault(&self) -> bool {
        self.kind == Kind::HeroAssault
    }

    pub fn xl(members: [u32; 2]) -> Self {
        Self {
            kind: Kind::Xl,
            ..Self::new(XL_SCORE_LIMIT, members)
        }
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"DEATHMATCH");
        match self.kind {
            Kind::HeroAssault => {}
            Kind::Hunt => eat(b"HUNT"),
            Kind::Xl => eat(b"XL"),
        }
        eat(&self.score_limit[0].to_le_bytes());
        // Per-side targets are hashed only when they differ, so an even
        // round keeps its old layout.
        if self.score_limit[0] != self.score_limit[1] {
            eat(b"TARGETS");
            eat(&self.score_limit[1].to_le_bytes());
        }
        for team in 0..2 {
            eat(&self.members[team].to_le_bytes());
            eat(&self.points[team].to_le_bytes());
        }
        // Hashed only when a side is eased, so an unscaled Hunt keeps its
        // old layout.
        if self.skill_pct != [100; 2] {
            eat(b"SKILLPCT");
            for pct in self.skill_pct {
                eat(&pct.to_le_bytes());
            }
        }
    }
}

impl Sim {
    pub fn is_hunt(&self) -> bool {
        self.deathmatch
            .as_ref()
            .is_some_and(|round| round.kind == Kind::Hunt)
    }

    pub fn is_xl(&self) -> bool {
        self.deathmatch
            .as_ref()
            .is_some_and(|round| round.kind == Kind::Xl)
    }

    /// Team `team`'s Hunt bot skill scale in percent: the Hunt's own
    /// ([`Deathmatch::with_skill_pct`]) on teams 0 and 1 of a Hunt, 100 in
    /// every other mode and for any other team.
    pub fn hunt_skill_pct(&self, team: u8) -> u32 {
        match self.deathmatch.as_ref() {
            Some(d) if d.kind == Kind::Hunt && team < 2 => d.skill_pct[usize::from(team)],
            _ => 100,
        }
    }

    /// Unit `u`'s Hunt ease percent: its team's [`Sim::hunt_skill_pct`] for
    /// a bot, 100 for a human or a human stand-in (`proxy_human`).
    fn hunt_ease_of(&self, u: &crate::unit::Unit) -> u32 {
        if u.is_player || u.controller != crate::unit::Controller::Ai || u.proxy_human.is_some() {
            return 100;
        }
        self.hunt_skill_pct(u.team)
    }

    /// How many times farther from its target an eased Hunt bot's aim
    /// point lands: (100 / percent) squared, so 2x at 71 %, 4x at 50 % and
    /// about 11x at the 30 % floor; 1 for everyone else and in every other
    /// mode. OURS (the maintainer 2026-11-21: ease mainly by missing more).
    pub fn hunt_aim_widen(&self, u: &crate::unit::Unit) -> f32 {
        let pct = self.hunt_ease_of(u);
        if pct >= 100 {
            return 1.0;
        }
        let f = 100.0 / pct as f32;
        f * f
    }

    /// The step `dt` of unit `u`'s weapon of class `wc`: an eased Hunt bot's
    /// weapon that needs no aim (an emitter: the arc casters, the
    /// incinerator, the flamethrowers) runs at its percent of real time, so
    /// it fires that much less often; every other weapon and unit, and every
    /// other mode, keeps `dt`. OURS (the maintainer 2026-11-21): aim error
    /// cannot ease a weapon that needs no aim.
    pub fn hunt_weapon_dt(
        &self,
        u: &crate::unit::Unit,
        wc: &crate::weapon::WeaponClass,
        dt: f32,
    ) -> f32 {
        if wc.emitter.is_none() || wc.force.is_some() {
            return dt;
        }
        let pct = self.hunt_ease_of(u);
        if pct >= 100 {
            return dt;
        }
        dt * pct as f32 / 100.0
    }

    pub fn hero_assault(&self) -> bool {
        self.deathmatch
            .as_ref()
            .is_some_and(Deathmatch::is_hero_assault)
    }

    pub(super) fn deathmatch_kill(&mut self, killer: UnitHandle, victim_team: u8) {
        if self.outcome.is_some() {
            return;
        }
        let Some(deathmatch) = self.deathmatch.as_mut() else {
            return;
        };
        let Some(killer_team) = self.units.get(killer.index as usize).map(|u| u.team) else {
            return;
        };
        // A native's death scores for nobody, as it costs no ticket in
        // Conquest (maintainer, 2026-11-20: the Jawas).
        if killer_team == victim_team || killer_team > 1 || victim_team > 1 {
            return;
        }
        // No weighted scoring: every enemy kill is one point, whoever dies.
        let team = usize::from(killer_team);
        deathmatch.points[team] += 1;
        if deathmatch.points[team] >= deathmatch.score_limit[team] {
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

    /// Each team's points and the points each needs to win.
    pub fn team_points(&self) -> Option<([u32; 2], [u32; 2])> {
        self.deathmatch.as_ref().map(|d| (d.points, d.score_limit))
    }
}

#[cfg(test)]
mod tests;
