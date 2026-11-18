//! The mission script's AI goals: BF2's `AddAIGoal` / `DeleteAIGoal` /
//! `ClearAIGoals` and the goal manager they fill
//! (`analysis/re-conquest-goal.md` §1).
//!
//! - **The list.** `FUN_002e79a8` takes the first free of 20 slots at
//!   manager+0x1c for a weight above 0; Destroy and Defend need a resolved
//!   target and Follow a player or object one, else nothing is made and the
//!   script gets 0. [`AiGoals::add`].
//! - **Delete.** `FUN_00191ce8` → `FUN_002e6f50` (the members leave) →
//!   `FUN_002e7bf0` (the slot frees). [`AiGoals::delete`].
//! - **Clear.** `FUN_002e7c58`: the same for every goal of one team.
//!   [`AiGoals::clear`].
//! - **Filing.** The manager ticks when its stamp passes and sets it one
//!   second on (`FUN_002e7cf8`). Each tick every AI record in no goal of its own
//!   team (`FUN_002e7808`: one in another team's goal leaves it first) joins
//!   the enabled goal of its team with the smallest
//!   `members / (team size × weight / sum of weights)` (`FUN_002e7580`; the
//!   first such goal on a tie, from a start of 1e11). The team size (team
//!   +0x34) is the same for every goal of the team, so it leaves the choice
//!   alone and is not kept. A record is re-filed when it is readied for a
//!   respawn (`FUN_0034ffe0` → `FUN_002e7708`): here, the first step that
//!   sees its new life.
//! - **Enabled.** Goal +0x18 is 1 from the ctor `FUN_002e6e98`; its only
//!   other writer `FUN_002e71f0` is called from two other goal classes'
//!   updates, which are not ported, so every goal stays enabled.
//!   trace: `FUN_002e71f0`'s callers.
//!
//! What a member does is [`Steer`]: Deathmatch (`FUN_002e9458` posts order
//! 10, the roaming walk `ai::route::retarget_deathmatch`) and Defend
//! (`FUN_002e96c8` posts order 0xb with the goal's target). Conquest is the
//! port's commander (`core::commander`), which owns a member of a Conquest
//! goal as it owns every bot of a team with no script goal. OURS: Destroy
//! (order 0xc, `FUN_002e9948`), the two CTF goals and Follow also keep the
//! commander's behaviour. replace: those goals' orders, once their agents
//! are traced.

use crate::sim::Sim;
use crate::unit::{Controller, UnitHandle};
use glam::Vec3;

/// `FUN_002e79a8`'s slot count (manager+0x1c, 0x14 slots).
pub const SLOTS: usize = 20;
/// `FUN_002e7cf8`: the manager's period, 1.0 s.
pub const TICK: u64 = crate::TICK_HZ as u64;

/// `AddAIGoal`'s name hashes (B2:36558-36690), in the manager's type order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalKind {
    Conquest = 1,
    Destroy = 2,
    Defend = 3,
    Deathmatch = 4,
    CtfOffense = 5,
    CtfDefense = 6,
    Follow = 7,
}

impl GoalKind {
    /// The script's goal name, as `AddAIGoal` hashes it (FNV-1a over
    /// `byte | 0x20`, so case does not matter).
    pub fn named(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "conquest" => Self::Conquest,
            "destroy" => Self::Destroy,
            "defend" => Self::Defend,
            "deathmatch" => Self::Deathmatch,
            "ctfoffense" => Self::CtfOffense,
            "ctfdefense" => Self::CtfDefense,
            "follow" => Self::Follow,
            _ => return None,
        })
    }

    /// `FUN_002e79a8`: Destroy and Defend need a target (type nonzero),
    /// Follow a player or an object one (type 2 or 3).
    fn needs_target(self) -> bool {
        matches!(self, Self::Destroy | Self::Defend | Self::Follow)
    }
}

/// The object `AddAIGoal`'s fourth argument resolved to: a player record
/// (type 2) or an object (type 3, here a post or a placed object's point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GoalTarget {
    Post(u32),
    Unit(UnitHandle),
    Point(Vec3),
    Station(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goal {
    /// The handle the script keeps (the PS2's goal pointer).
    pub id: u32,
    pub kind: GoalKind,
    /// Goal +0x19, a sim team.
    pub team: u8,
    /// Goal +0x14.
    pub weight: f32,
    pub target: Option<GoalTarget>,
}

/// What a member of a goal is told to do. `None` from [`AiGoals::steer`]
/// leaves the bot to the commander.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Steer {
    Deathmatch,
    Defend(GoalTarget),
    Destroy(GoalTarget),
}

/// The goal manager `DAT_005fe7a8`. SIM STATE: hashed once a goal is made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiGoals {
    pub slots: [Option<Goal>; SLOTS],
    /// Per unit slot, the goal it is in and the life it joined in.
    pub member: Vec<Option<(u32, u32)>>,
    pub next_tick: u64,
    pub next_id: u32,
}

impl AiGoals {
    /// No goal has ever been made: the manager has nothing to do.
    pub fn idle(&self) -> bool {
        self.next_id == 0
    }

    /// `FUN_002e79a8`: the new goal's handle, or `None` (the script's 0).
    pub fn add(
        &mut self,
        team: u8,
        kind: GoalKind,
        weight: f32,
        target: Option<GoalTarget>,
    ) -> Option<u32> {
        if weight <= 0.0 || (kind.needs_target() && target.is_none()) {
            return None;
        }
        let slot = self.slots.iter_mut().find(|s| s.is_none())?;
        self.next_id += 1;
        let id = self.next_id;
        *slot = Some(Goal {
            id,
            kind,
            team,
            weight,
            target,
        });
        Some(id)
    }

    /// `FUN_00191ce8`: the members leave, the slot frees.
    pub fn delete(&mut self, id: u32) {
        if let Some(s) = self
            .slots
            .iter_mut()
            .find(|s| s.is_some_and(|g| g.id == id))
        {
            *s = None;
            self.leave_all(id);
        }
    }

    /// `FUN_002e7c58`: every goal of `team`.
    pub fn clear(&mut self, team: u8) {
        for s in &mut self.slots {
            if let Some(g) = s.filter(|g| g.team == team) {
                *s = None;
                for m in &mut self.member {
                    if m.is_some_and(|(id, _)| id == g.id) {
                        *m = None;
                    }
                }
            }
        }
    }

    fn leave_all(&mut self, id: u32) {
        for m in &mut self.member {
            if m.is_some_and(|(g, _)| g == id) {
                *m = None;
            }
        }
    }

    fn goal(&self, id: u32) -> Option<&Goal> {
        self.slots.iter().flatten().find(|g| g.id == id)
    }

    fn members(&self, id: u32) -> usize {
        self.member
            .iter()
            .filter(|m| m.is_some_and(|(g, _)| g == id))
            .count()
    }

    /// The goal unit slot `i` is in during life `gen`, if any.
    pub fn of(&self, i: usize, gen: u32) -> Option<&Goal> {
        let (id, life) = (*self.member.get(i)?)?;
        if life != gen {
            return None;
        }
        self.goal(id)
    }

    /// What unit `h`'s goal tells it to do.
    pub fn steer(&self, h: UnitHandle) -> Option<Steer> {
        let g = self.of(h.index as usize, h.gen)?;
        match g.kind {
            GoalKind::Deathmatch => Some(Steer::Deathmatch),
            GoalKind::Destroy => g.target.map(Steer::Destroy),
            GoalKind::Defend => g.target.map(Steer::Defend),
            _ => None,
        }
    }

    /// `FUN_002e7580`: the goal of `team` with the fewest members for its
    /// share of the weight.
    fn pick(&self, team: u8) -> Option<u32> {
        let goals = || self.slots.iter().flatten().filter(|g| g.team == team);
        let sum: f32 = goals().map(|g| g.weight).sum();
        if sum == 0.0 {
            return None;
        }
        let mut best = (1e11_f32, None);
        for g in goals() {
            let score = self.members(g.id) as f32 / (g.weight / sum);
            if score < best.0 {
                best = (score, Some(g.id));
            }
        }
        best.1
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"GOAL");
        eat(&self.next_tick.to_le_bytes());
        eat(&self.next_id.to_le_bytes());
        for s in &self.slots {
            match s {
                Some(g) => {
                    eat(&[1, g.kind as u8, g.team]);
                    eat(&g.id.to_le_bytes());
                    eat(&g.weight.to_bits().to_le_bytes());
                    match g.target {
                        None => eat(&[0]),
                        Some(GoalTarget::Post(c)) => {
                            eat(&[1]);
                            eat(&c.to_le_bytes());
                        }
                        Some(GoalTarget::Unit(h)) => {
                            eat(&[2]);
                            eat(&h.index.to_le_bytes());
                            eat(&h.gen.to_le_bytes());
                        }
                        Some(GoalTarget::Point(p)) => {
                            eat(&[3]);
                            for c in p.to_array() {
                                eat(&c.to_bits().to_le_bytes());
                            }
                        }
                        Some(GoalTarget::Station(s)) => {
                            eat(&[4]);
                            eat(&s.to_le_bytes());
                        }
                    }
                }
                None => eat(&[0]),
            }
        }
        eat(&(self.member.len() as u32).to_le_bytes());
        for m in &self.member {
            match m {
                Some((g, life)) => {
                    eat(&[1]);
                    eat(&g.to_le_bytes());
                    eat(&life.to_le_bytes());
                }
                None => eat(&[0]),
            }
        }
    }
}

/// The manager's step, before the commander plans: a new life is re-filed
/// at once, and on the 1 s tick every AI record in no goal of its team is.
pub fn step(sim: &mut Sim) {
    if sim.ai_goals.idle() {
        return;
    }
    let n = sim.units.len();
    let tick = sim.tick >= sim.ai_goals.next_tick;
    if tick {
        sim.ai_goals.next_tick = sim.tick + TICK;
    }
    let goals = &mut sim.ai_goals;
    goals.member.resize(n, None);
    for (i, u) in sim.units.iter().enumerate() {
        if u.controller != Controller::Ai {
            goals.member[i] = None;
            continue;
        }
        let filed = goals.member[i].and_then(|(id, life)| {
            let g = goals.goal(id)?;
            (g.team == u.team).then_some(life)
        });
        let refile = match filed {
            Some(life) => life != u.gen,
            None => tick,
        };
        if refile {
            goals.member[i] = None;
            goals.member[i] = goals.pick(u.team).map(|id| (id, u.gen));
        }
    }
}

/// Where a Defend member holds: the post, the player, the station or the object.
pub fn target_pos(sim: &Sim, t: GoalTarget) -> Option<Vec3> {
    match t {
        GoalTarget::Post(c) => sim.cps.get(c as usize).map(|c| c.def.pos),
        GoalTarget::Unit(h) => sim.unit(h).map(|u| u.soldier.pos),
        GoalTarget::Point(p) => Some(p),
        GoalTarget::Station(s) => sim
            .stations
            .get(usize::from(s))
            .filter(|s| !s.destroyed)
            .map(|s| Vec3::new(s.sphere_centre.x, s.pos.y, s.sphere_centre.z)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_spread_over_a_team_s_goals_by_weight() {
        let mut g = AiGoals::default();
        let dm = g.add(1, GoalKind::Deathmatch, 50.0, None).unwrap();
        let def = g
            .add(1, GoalKind::Defend, 50.0, Some(GoalTarget::Post(3)))
            .unwrap();
        assert_eq!(g.add(1, GoalKind::Defend, 10.0, None), None);
        assert_eq!(g.add(1, GoalKind::Deathmatch, 0.0, None), None);
        g.member = vec![None; 4];
        for i in 0..4 {
            g.member[i] = g.pick(1).map(|id| (id, 0));
        }
        let ids: Vec<u32> = g.member.iter().map(|m| m.unwrap().0).collect();
        assert_eq!(ids, vec![dm, def, dm, def]);
        g.delete(dm);
        assert_eq!(g.member.iter().filter(|m| m.is_none()).count(), 2);
        assert_eq!(g.pick(1), Some(def));
        g.clear(1);
        assert!(g.member.iter().all(Option::is_none));
        assert_eq!(g.pick(1), None);
    }

    #[test]
    fn the_list_holds_twenty_goals() {
        let mut g = AiGoals::default();
        for _ in 0..SLOTS {
            assert!(g.add(0, GoalKind::Deathmatch, 1.0, None).is_some());
        }
        assert_eq!(g.add(0, GoalKind::Deathmatch, 1.0, None), None);
    }
}
