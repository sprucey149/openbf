//! The mission script's AI goals: BF2's `AddAIGoal` / `DeleteAIGoal` /
//! `ClearAIGoals` and the goal manager they fill
//! (`analysis/re-conquest-goal.md` §1).
//!
//! A mission can add weighted goals, remove one goal, or clear a team's
//! goals. Unassigned bots join the least filled goal relative to its
//! weight, with registration order breaking ties. Bots reconsider their
//! assignments after respawning. A bot whose goal was removed waits for
//! another assignment. See `FUN_002e79a8`, `FUN_00191ce8`, `FUN_002e7c58`,
//! `FUN_002e7cf8`, `FUN_002e7580`, and `FUN_002e7708`.
//!
//! Deathmatch bots roam. CTF offense bots recover a flag, escort its
//! carrier, or chase it down. Defenders stay near their target and look
//! for a free hint or cover when it stays put. They resume defending
//! after a fight and move with a target that leaves the area. See
//! `FUN_002e9458`, `FUN_002e8be8`, and `FUN_002d2f98`
//! (`SWBFRNT2.ELF-decomp.c` lines 246552-246615).
//!
//! The commander handles Conquest and supplies orders before the mission
//! adds any scripted goals. OURS: Destroy, CTFDefense, and Follow also
//! use the commander. replace: trace the agents for those orders, starting
//! with Destroy in `FUN_002e9948`.
//! trace: goal enablement in the callers of `FUN_002e71f0`.

use crate::sim::Sim;
use crate::unit::{Controller, UnitHandle};
use glam::{Vec3, Vec3Swizzles};

/// Maximum number of active goal assignments (`FUN_002e79a8`).
pub const SLOTS: usize = 20;
/// `FUN_002e7cf8`: the manager's period, 1.0 s.
pub const TICK: u64 = crate::TICK_HZ as u64;
/// `FUN_002c53c8` state 1: a friendly carrier within this (m, 2500.0
/// squared) is escorted.
pub const ESCORT_NEAR: f32 = 50.0;
/// A soldier normally has four escort slots (`FUN_00249f30`).
/// PROVISIONAL: four slots for every carrier. trace: the campaign player
/// override in `FUN_00360120`.
pub const ESCORT_SLOTS: usize = 4;
/// `FUN_002e0610` kind 3: a follower's place in the carrier's frame, to the
/// side by 0.75 (left for an even slot, right for an odd one), back by
/// slot / 2 + 0.25, each axis moved by 0.2 of a draw in [-1, 1); the draw's
/// seed changes every 8 s (time × 0.125). `FUN_002c5a88` state 4 doubles
/// the place.
pub const ESCORT_SIDE: f32 = 0.75;
pub const ESCORT_ROW: f32 = 0.25;
pub const ESCORT_JITTER: f32 = 0.2;
pub const ESCORT_SCALE: f32 = 2.0;
pub const ESCORT_RESEED: u64 = 8 * TICK;
/// `FUN_002d2f98`: the first hint search once the target has stayed this
/// long (10.0 s) within [`HINT_ANCHOR`] of the anchor.
pub const HINT_WAIT: u64 = 10 * TICK;
/// `FUN_002d2f98`: after a search the clock is set to 8.0 s, so the next
/// search is 2 s on.
pub const HINT_RETRY: u64 = 2 * TICK;
/// `FUN_002ade38`: a hint is taken while the clock is under its last stamp
/// plus 2.0 s (10.0 s for a Mine hint, which no Defend query returns).
pub const HINT_KEEP: u64 = 2 * TICK;
/// `FUN_002d31a8` states 5 and 6: a walk that ends within this (m, 25.0
/// squared) of the hint stands at it (state 3); farther, the helper walks to
/// the target again (state 2).
pub const HINT_NEAR: f32 = 5.0;
/// `FUN_002d31a8` state 3: at its hint the helper steps out (state 6) when
/// `FUN_002a3f08(0, 100)` draws over 0x5a.
pub const SHUFFLE_OVER: u32 = 90;
/// `FUN_002d2168` state 6: the clear test's radius (`FUN_002eea68`, 5.0) for
/// a barrier to step round.
pub const SHUFFLE_BARRIER: f32 = 5.0;
/// `FUN_002d2168` state 6: the random step is ±0.5 × 2 × 15.0 m on X and Z.
pub const SHUFFLE_REACH: f32 = 15.0;
/// `FUN_002d2168` state 6: the type "7" query around the step's point
/// (`FUN_002dfcf8` with 0x14 and at most 10 hints).
pub const SHUFFLE_BOX: f32 = 20.0;
/// `FUN_002d2110`: the agent's think factor in the defend helper's states
/// 3, 5 and 8, and in its others (`FUN_002c70b8`; 0.8 in the combat
/// helper, which is the port's fight).
pub const THINK_STAND: f32 = 2.0;
pub const THINK_WALK: f32 = 1.5;
/// `FUN_002c70b8`: the think factor while the combat helper runs, here its
/// walk to a noise and its look round.
pub const THINK_FIGHT: f32 = 0.8;
/// `FUN_002de708`: a unit that fired since its last pass is heard out to
/// 100.0 m, cut to 30.0 m.
pub const NOISE_SHOT: f32 = 30.0;
/// `FUN_002de708`: an unfired soldier is heard out to 5.0 × its speed,
/// clamped to 0-50.0 m, cut to [`NOISE_SHOT`] and raised to 8.0 m.
pub const NOISE_SPEED: f32 = 5.0;
pub const NOISE_SPEED_MAX: f32 = 50.0;
pub const NOISE_FLOOR: f32 = 8.0;
/// `FUN_002de708`: a listener whose way to the emitter is within 0.87 of
/// straight up or down (`DAT_0073d8a0` = (0, 1, 0), set at decomp 657090)
/// hears it only under 3.5 m (12.25 squared).
pub const NOISE_UP: f32 = 0.87;
pub const NOISE_UP_NEAR: f32 = 3.5;
/// `FUN_002de708`: within 3.0 m (9.0 squared) the pass sends message 5, a
/// sighting; farther, message 6, the noise.
pub const NOISE_SEEN: f32 = 3.0;
/// `FUN_002af788`: a human's noise pass runs every 15 steps; a bot's runs at
/// its think.
pub const NOISE_HUMAN: u64 = 15;
/// `FUN_002cf458` state 4: the walk to a noise gives up after 10.0 s.
pub const NOISE_WALK: u64 = 10 * TICK;
/// `FUN_002cef98`: the first look turns U(30°, 75°) one way or the other.
pub const LOOK_MIN: f32 = 30.0;
pub const LOOK_MAX: f32 = 75.0;
/// `FUN_002cf458` state 3: faced once the level cosine to the look point is
/// over 0.939, held 0.4 s, and two looks are taken (state 100 after).
pub const LOOK_FACED: f32 = 0.939;
pub const LOOK_HOLD: f32 = 0.4;
pub const LOOK_TURNS: u8 = 2;
/// `FUN_002cef98`: the look point is 100.0 m along the turned facing.
pub const LOOK_REACH: f32 = 100.0;
/// `FUN_002d2f98` / `FUN_002d3118`: a target farther than this (m) from the
/// anchor moves the anchor and starts the clock again (625.0 squared).
pub const HINT_ANCHOR: f32 = 25.0;
/// `FUN_002d2f98`: the hint query's half-size (m) per axis around the
/// target (`FUN_002df6c0`'s box, 0x32).
pub const HINT_BOX: f32 = 50.0;
/// `FUN_002dfcf8`: the first query's mask 0x40, hint TYPE "7"
/// (`analysis/re-soldier-ai.md` 3).
pub const HINT_DEFEND: u8 = 7;
/// `FUN_002df9a8`: the second query's mask 4, hint TYPE "2" Cover.
pub const HINT_COVER: u8 = 2;

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
    /// A CTFOffense goal (`FUN_002e8ab8`): its capture region's centre
    /// (+0x30) and its flag (+0x20, an index into `CtfState::flags`).
    /// PROVISIONAL: with no flag the ctor takes the first entity not on the
    /// goal's team; the port takes none, and the members stand.
    /// trace: `FUN_002e8ab8`'s fallback scan.
    Ctf {
        region: Vec3,
        flag: Option<u16>,
    },
    /// A flag of `CtfState::flags`, where it is now.
    Flag(u16),
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
    /// A walk to the target (`FUN_002bc378`): the CTF offense agent's
    /// states 2, 3 and 5.
    Go(GoalTarget),
    /// The CTF offense agent's state 4: the walk after a friendly carrier to
    /// `offset` in his frame (`FUN_002bc800`).
    Escort {
        carrier: UnitHandle,
        offset: Vec3,
    },
    /// No order: the bot stays where it is. PROVISIONAL (ours): it fights
    /// what it sees, as the wait agent (state 1) a deleted goal leaves does;
    /// a new life that never had an order has no agent to take the message
    /// (`FUN_002b7e58`). trace: a controller with no agent on a sighting.
    Stand,
}

/// A CTF offense bot picks a job based on who has the flag and whether it
/// can pick it up. Escorts stay near a friendly carrier when a follower
/// slot is available. A bot reconsiders its job when the flag changes
/// hands or it can no longer help. See `FUN_002c53c8` and `FUN_002c5a88`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtfStep {
    /// State 2: the walk to the flag; it ends when the flag is taken or the
    /// walk is done (`FUN_002bc2b8`).
    Fetch = 2,
    /// State 3: the walk to the goal's region while the bot carries.
    Carry = 3,
    /// State 4: the walk after the carrier, while a friend carries.
    Escort = 4,
    /// State 5: after a carrier that is no friend.
    Chase = 5,
    /// State 6: the defend helper (`FUN_002d20a0`) on the flag (order
    /// target type 3), until a friend carries it.
    Guard = 6,
    /// State 7: the defend helper on the flag's home region (+0x458,
    /// `HomeRegion`) or, with none, on the flag (type 1, a point), while
    /// another carries it.
    Cover = 7,
}

/// A CTFOffense member's agent: its goal, its life, its state and, while it
/// escorts, the carrier and its place behind him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CtfMember {
    pub goal: u32,
    pub life: u32,
    pub step: CtfStep,
    pub carrier: Option<UnitHandle>,
    pub offset: Vec3,
}

/// The defend helper's state (`FUN_002d2168`, `FUN_002d31a8`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldState {
    /// State 2, walking to the target, and state 3 at it with no hint: both
    /// search.
    Walk,
    /// State 5, walking to the hint.
    ToHint,
    /// State 3 with a hint.
    AtHint,
    /// State 6, the step out to another hint.
    Shuffle,
    /// The combat helper runs (`FUN_002c7308`); the defend helper waits and
    /// walks again when it is done (`FUN_002c7218`).
    Fight,
    /// The combat helper's state 4 after a noise (message 6, order type 3):
    /// the walk to where the noise was.
    Noise,
    /// The combat helper's state 3 after that walk: it stops and looks one
    /// way, then the other.
    Look,
}

/// Tracks where a defender last settled and which hint it has reserved
/// (`FUN_002d1fa8`).
///
/// PROVISIONAL. Defenders use ordinary combat cover. replace: trace the
/// defender's sniper stance, nearby head turns, and cover peeks in
/// `FUN_002d7c30`, `FUN_002d3aa8`, and `FUN_002d0608`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefendHold {
    /// The goal and the life this helper belongs to.
    pub goal: u32,
    pub life: u32,
    pub state: HoldState,
    pub anchor: Vec3,
    /// Ticks the target has stayed by the anchor ([`HINT_WAIT`] to search);
    /// `u64::MAX` after a state 2 entry.
    pub clock: u64,
    /// Index into `NavGraph::hints`.
    pub hint: Option<u32>,
    /// The tick `hint` was last stamped.
    pub stamp: u64,
    /// A step's point (state 6) until it is reached, then its end.
    pub via: Option<Vec3>,
    pub end: Vec3,
    /// The ticks of the helper's last and next update.
    pub last: u64,
    pub next: u64,
    /// [`HoldState::Noise`]: the walk's end (+0x150) and its start (+0x14c);
    /// [`HoldState::Look`]: where the bot stopped.
    pub noise: Vec3,
    pub since: u64,
    /// [`HoldState::Look`]: the look point, its turn (rad), the looks taken
    /// (+0x16c) and the tick the hold on it ends.
    pub gaze: Vec3,
    pub turn: f32,
    pub turns: u8,
    pub until: Option<u64>,
}

/// The goal manager `DAT_005fe7a8`. SIM STATE: hashed once a goal is made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiGoals {
    pub slots: [Option<Goal>; SLOTS],
    /// Per unit slot, the goal it is in and the life it joined in.
    pub member: Vec<Option<(u32, u32)>>,
    /// Per unit slot, a Defend member's hint search.
    pub hold: Vec<Option<DefendHold>>,
    /// Per unit slot, a CTFOffense member's agent.
    pub ctf: Vec<Option<CtfMember>>,
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
        let Some(g) = self.of(h.index as usize, h.gen) else {
            return (!self.idle()).then_some(Steer::Stand);
        };
        match g.kind {
            GoalKind::Deathmatch => Some(Steer::Deathmatch),
            GoalKind::Destroy => g.target.map(Steer::Destroy),
            GoalKind::Defend => g.target.map(Steer::Defend),
            GoalKind::CtfOffense | GoalKind::CtfDefense => Some(self.ctf_steer(h, g)),
            _ => None,
        }
    }

    /// Unit `h`'s CTF agent, while it is a member of a CTFOffense or CtfDefense
    /// goal.
    pub fn ctf_of(&self, h: UnitHandle) -> Option<CtfMember> {
        let g = self.of(h.index as usize, h.gen)?;
        let m = (*self.ctf.get(h.index as usize)?)?;
        (matches!(g.kind, GoalKind::CtfOffense | GoalKind::CtfDefense)
            && m.goal == g.id
            && m.life == h.gen)
            .then_some(m)
    }

    fn ctf_steer(&self, h: UnitHandle, g: &Goal) -> Steer {
        let Some(m) = self.ctf_of(h) else {
            return Steer::Stand;
        };
        let (region, flag) = match g.target {
            Some(GoalTarget::Ctf {
                region,
                flag: Some(f),
            }) => (region, f),
            Some(GoalTarget::Flag(f)) => (Vec3::ZERO, f),
            _ => return Steer::Stand,
        };
        match (m.step, m.carrier) {
            (CtfStep::Fetch, _) => Steer::Go(GoalTarget::Flag(flag)),
            (CtfStep::Carry, _) => Steer::Go(GoalTarget::Point(region)),
            (CtfStep::Escort, Some(c)) => Steer::Escort {
                carrier: c,
                offset: m.offset,
            },
            (CtfStep::Chase, Some(c)) => Steer::Go(GoalTarget::Unit(c)),
            (CtfStep::Guard | CtfStep::Cover, _) => Steer::Defend(GoalTarget::Flag(flag)),
            (CtfStep::Escort | CtfStep::Chase, None) => Steer::Stand,
        }
    }

    /// Unit `h`'s defend helper, while it is a member of a Defend goal or a
    /// CTF member guards or covers its flag.
    pub fn hold_of(&self, h: UnitHandle) -> Option<DefendHold> {
        let g = self.of(h.index as usize, h.gen)?;
        let hold = (*self.hold.get(h.index as usize)?)?;
        let defends = match g.kind {
            GoalKind::Defend => true,
            GoalKind::CtfOffense | GoalKind::CtfDefense => self
                .ctf_of(h)
                .is_some_and(|m| matches!(m.step, CtfStep::Guard | CtfStep::Cover)),
            _ => false,
        };
        (defends && hold.goal == g.id && hold.life == h.gen).then_some(hold)
    }

    /// The hint unit `h` holds for its Defend goal, if it has found one.
    pub fn hint_of(&self, h: UnitHandle) -> Option<u32> {
        self.hold_of(h)?.hint
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
                        Some(GoalTarget::Ctf { region, flag }) => {
                            eat(&[5]);
                            for c in region.to_array() {
                                eat(&c.to_bits().to_le_bytes());
                            }
                            eat(&flag.unwrap_or(u16::MAX).to_le_bytes());
                        }
                        Some(GoalTarget::Flag(f)) => {
                            eat(&[6]);
                            eat(&f.to_le_bytes());
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
        eat(&(self.hold.len() as u32).to_le_bytes());
        for h in &self.hold {
            match h {
                Some(h) => {
                    eat(&[1, h.state as u8]);
                    eat(&h.goal.to_le_bytes());
                    eat(&h.life.to_le_bytes());
                    for c in h.anchor.to_array() {
                        eat(&c.to_bits().to_le_bytes());
                    }
                    eat(&h.clock.to_le_bytes());
                    eat(&h.hint.unwrap_or(u32::MAX).to_le_bytes());
                    eat(&h.stamp.to_le_bytes());
                    for c in h.via.unwrap_or(Vec3::NAN).to_array() {
                        eat(&c.to_bits().to_le_bytes());
                    }
                    for c in h.end.to_array() {
                        eat(&c.to_bits().to_le_bytes());
                    }
                    eat(&h.last.to_le_bytes());
                    eat(&h.next.to_le_bytes());
                    for c in h.noise.to_array().into_iter().chain(h.gaze.to_array()) {
                        eat(&c.to_bits().to_le_bytes());
                    }
                    eat(&h.since.to_le_bytes());
                    eat(&h.turn.to_bits().to_le_bytes());
                    eat(&[h.turns]);
                    eat(&h.until.unwrap_or(u64::MAX).to_le_bytes());
                }
                None => eat(&[0]),
            }
        }
        if self.ctf.iter().any(Option::is_some) {
            eat(b"CTFO");
            eat(&(self.ctf.len() as u32).to_le_bytes());
            for m in &self.ctf {
                match m {
                    Some(m) => {
                        eat(&[1, m.step as u8]);
                        eat(&m.goal.to_le_bytes());
                        eat(&m.life.to_le_bytes());
                        let (ci, cg) = m.carrier.map_or((u32::MAX, u32::MAX), |c| (c.index, c.gen));
                        eat(&ci.to_le_bytes());
                        eat(&cg.to_le_bytes());
                        for c in m.offset.to_array() {
                            eat(&c.to_bits().to_le_bytes());
                        }
                    }
                    None => eat(&[0]),
                }
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
    ctf_agents(sim);
    defend_hints(sim);
}

/// The CTF offense agent of every CTFOffense member, in unit order. A new
/// state starts a new defend helper (`FUN_002c53c8` builds it on entry to
/// states 6 and 7).
///
/// PROVISIONAL (ours): a state's test runs at the member's think
/// (`FUN_002c5a88` on the agent's clock); the fight (state 8, entered from
/// the message handler `FUN_002c5e38`) is the port's fight, and the aim on
/// the way (`FUN_002c58b0`) the port's aim. replace: state 8 and
/// `FUN_002c58b0`.
fn ctf_agents(sim: &mut Sim) {
    let n = sim.units.len();
    sim.ai_goals.ctf.resize(n, None);
    for i in 0..n {
        let prev = sim.ai_goals.ctf[i];
        let next = ctf_agent(sim, i);
        let key = |m: Option<CtfMember>| m.map(|m| (m.goal, m.life, m.step));
        if key(next) != key(prev) {
            if let Some(h) = sim.ai_goals.hold.get_mut(i) {
                *h = None;
            }
        }
        sim.ai_goals.ctf[i] = next;
    }
    for i in 0..n {
        let Some(mut m) = sim.ai_goals.ctf[i] else {
            continue;
        };
        let Some(c) = m.carrier.filter(|_| m.step == CtfStep::Escort) else {
            continue;
        };
        let slot = sim.ai_goals.ctf[..i]
            .iter()
            .flatten()
            .filter(|o| o.step == CtfStep::Escort && o.carrier == Some(c))
            .count();
        m.offset = escort_offset(c, slot, sim.tick);
        sim.ai_goals.ctf[i] = Some(m);
    }
}

/// `FUN_002c5a88`'s tests and `FUN_002c53c8` state 1's choice for unit
/// slot `i`. `None` (no flag: state 100) leaves the bot standing.
fn ctf_agent(sim: &Sim, i: usize) -> Option<CtfMember> {
    let u = &sim.units[i];
    if !u.alive || u.controller != Controller::Ai {
        return None;
    }
    let g = sim
        .ai_goals
        .of(i, u.gen)
        .filter(|g| matches!(g.kind, GoalKind::CtfOffense | GoalKind::CtfDefense))?;
    if g.kind == GoalKind::CtfDefense {
        let Some(GoalTarget::Flag(f)) = g.target else {
            return None;
        };
        let flag = sim.ctf.as_ref()?.flags.get(usize::from(f))?;
        let carrier = flag.carrier();
        let step = match flag.status {
            crate::sim::ctf::FlagStatus::AtHome => CtfStep::Guard,
            crate::sim::ctf::FlagStatus::Dropped { .. } => CtfStep::Fetch,
            crate::sim::ctf::FlagStatus::Carried { .. } => CtfStep::Chase,
        };
        return Some(CtfMember {
            goal: g.id,
            life: u.gen,
            step,
            carrier,
            offset: Vec3::ZERO,
        });
    }
    let Some(GoalTarget::Ctf { flag: Some(f), .. }) = g.target else {
        return None;
    };
    let flag = sim.ctf.as_ref()?.flags.get(usize::from(f))?;
    let me = sim.handle(i);
    let carrier = flag.carrier();
    let team_of = |c: UnitHandle| sim.units.get(c.index as usize).map(|o| o.team);
    let mine = carrier == Some(me);
    let friend =
        carrier.is_some_and(|c| c != me && team_of(c).is_some_and(|t| sim.allied(u.team, t)));
    let enemy = carrier.is_some_and(|c| team_of(c).is_some_and(|t| sim.hostile(u.team, t)));
    let prev = sim.ai_goals.ctf[i].filter(|m| m.goal == g.id && m.life == u.gen);
    let thinks = sim.tick > 0 && u.ai.last_think == sim.tick - 1;
    let stays = |step: CtfStep| match step {
        CtfStep::Fetch => carrier.is_none() && !walked(u, flag.current_pos),
        CtfStep::Carry => mine,
        CtfStep::Escort => friend,
        CtfStep::Chase => enemy,
        CtfStep::Guard => !friend,
        CtfStep::Cover => !mine && carrier.is_some(),
    };
    let step = match prev {
        Some(m) if !thinks || stays(m.step) => m.step,
        _ => match carrier {
            None if flag.allow_ai_pickup => CtfStep::Fetch,
            None => CtfStep::Guard,
            Some(_) if mine => CtfStep::Carry,
            Some(c) if friend => {
                let near = sim.unit(c).is_some_and(|o| {
                    o.soldier.pos.distance_squared(u.soldier.pos) < ESCORT_NEAR * ESCORT_NEAR
                });
                let escorts = sim
                    .ai_goals
                    .ctf
                    .iter()
                    .enumerate()
                    .filter(|(j, o)| {
                        *j != i
                            && o.is_some_and(|o| o.step == CtfStep::Escort && o.carrier == Some(c))
                            && sim.units.get(*j).is_some_and(|o| o.team == u.team)
                    })
                    .count();
                if near && escorts < ESCORT_SLOTS {
                    CtfStep::Escort
                } else {
                    CtfStep::Cover
                }
            }
            Some(_) => CtfStep::Chase,
        },
    };
    Some(CtfMember {
        goal: g.id,
        life: u.gen,
        step,
        carrier,
        offset: prev.map_or(Vec3::ZERO, |m| m.offset),
    })
}

/// `FUN_002e0610` kind 3 times [`ESCORT_SCALE`]: escort `slot`'s place
/// behind carrier `c`, in his frame (the port's local +Z is behind him, as
/// the disc's -Z is).
///
/// PROVISIONAL: the disc seeds its draw with the carrier's address and the
/// clock; the port draws per carrier, slot and 8 s window. The slot is the
/// escort's order among the carrier's escorts in unit order (the disc's
/// claim table order). trace: the frame `FUN_002bc800` applies the place in.
fn escort_offset(c: UnitHandle, slot: usize, tick: u64) -> Vec3 {
    let window = tick / ESCORT_RESEED;
    let draw = |axis: u32| {
        let salt = crate::ai::SALT_FORMATION ^ ((slot as u32) << 4 | axis);
        crate::ai::unit_unit(c.index, window, c.gen, salt) * 2.0 - 1.0
    };
    let side = if slot % 2 == 1 {
        -ESCORT_SIDE
    } else {
        ESCORT_SIDE
    };
    let row = (slot / 2) as f32 + ESCORT_ROW;
    Vec3::new(
        side + draw(1) * ESCORT_JITTER,
        0.0,
        row + draw(2) * ESCORT_JITTER,
    ) * ESCORT_SCALE
}

/// The defend helper of every Defend member, in unit order (`FUN_002d31a8`
/// on the agent's think clock).
fn defend_hints(sim: &mut Sim) {
    let n = sim.units.len();
    sim.ai_goals.hold.resize(n, None);
    for i in 0..n {
        let next = defend_hold(sim, i);
        sim.ai_goals.hold[i] = next;
    }
}

/// Resume defending after an interruption. Keep the reserved hint and
/// check for a nearby position on the first walk update (`FUN_002d2168`).
fn walk(hold: &mut DefendHold) {
    hold.state = HoldState::Walk;
    hold.clock = u64::MAX;
    hold.via = None;
}

/// The mover is done with a walk to `p`: arrived at a destination the
/// request to `p` set (`FUN_002bc2b8` no longer busy).
fn walked(u: &crate::unit::Unit, p: Vec3) -> bool {
    u.ai.mv.arrived && crate::nav::xz_dist(u.ai.mv.dest, p) <= crate::ai::ARRIVED_SLACK
}

fn hint_pos(sim: &Sim, k: Option<u32>) -> Option<Vec3> {
    Some(sim.nav.as_ref()?.hints.get(k? as usize)?.pos)
}

/// The end of the walk to hint `k` (state 5, `FUN_002bc378`): the hint,
/// moved out of a barrier it stands in ([`crate::nav::NavGraph::clear_goal`],
/// `FUN_002eefc8`). BF2 DEA1's type "7" hints 14-16 stand in the barriers of
/// the gallery 8.5 m over CAM_CP1's hallway: 14 and 15 move out onto the
/// hallway's floor, so their walks end over [`HINT_NEAR`] from the hint and
/// the helper walks to the target again (state 2); 16 moves out onto the
/// gallery's walkway.
fn hint_goal(sim: &Sim, k: Option<u32>) -> Option<Vec3> {
    let nav = sim.nav.as_ref()?;
    let p = nav.hints.get(k? as usize)?.pos;
    let q = nav.clear_goal(p.xz(), crate::unit::SOLDIER_RADIUS, crate::nav::SOLDIER);
    Some(Vec3::new(q.x, p.y, q.y))
}

/// PROVISIONAL (ours): the helper's state 2 also walks again when the bot is
/// over 50 m from a target that has moved more than 25 m from the walk's end
/// (`FUN_002d2d30`, 5 m in state 3); the port's walk re-reads the target at
/// every think instead, so a moved target does not restart the clock.
/// replace: `FUN_002d2d30` with the walk's end kept.
fn defend_hold(sim: &Sim, i: usize) -> Option<DefendHold> {
    let u = &sim.units[i];
    if !u.alive || u.controller != Controller::Ai {
        return None;
    }
    let g = sim.ai_goals.of(i, u.gen)?;
    let held = match g.kind {
        GoalKind::Defend => g.target?,
        GoalKind::CtfOffense => match (sim.ai_goals.ctf_of(sim.handle(i))?.step, g.target?) {
            (CtfStep::Guard | CtfStep::Cover, GoalTarget::Ctf { flag: Some(f), .. }) => {
                GoalTarget::Flag(f)
            }
            _ => return None,
        },
        GoalKind::CtfDefense => match (sim.ai_goals.ctf_of(sim.handle(i))?.step, g.target?) {
            (CtfStep::Guard, GoalTarget::Flag(f)) => GoalTarget::Flag(f),
            _ => return None,
        },
        _ => return None,
    };
    let target = target_pos(sim, held)?;
    let tick = sim.tick;
    let mut hold = sim.ai_goals.hold[i]
        .filter(|h| h.goal == g.id && h.life == u.gen)
        .unwrap_or(DefendHold {
            goal: g.id,
            life: u.gen,
            state: HoldState::Walk,
            anchor: Vec3::ZERO,
            clock: u64::MAX,
            hint: None,
            stamp: 0,
            via: None,
            end: Vec3::ZERO,
            last: tick,
            next: tick,
            noise: Vec3::ZERO,
            since: 0,
            gaze: Vec3::ZERO,
            turn: 0.0,
            turns: 0,
            until: None,
        });
    if u.ai.behaviour == crate::ai::Behaviour::Fight {
        hold.state = HoldState::Fight;
        return Some(hold);
    }
    if hold.state == HoldState::Fight {
        walk(&mut hold);
        hold.last = tick;
        hold.next = tick;
    }
    if hold.via.is_some_and(|v| walked(u, v)) {
        hold.via = None;
    }
    if !matches!(hold.state, HoldState::Noise | HoldState::Look) {
        if let Some(p) = noise(sim, i) {
            hold.state = HoldState::Noise;
            hold.via = None;
            hold.noise = p;
            hold.since = tick;
            hold.next = tick;
        }
    }
    if tick < hold.next {
        return Some(hold);
    }
    let dt = tick - hold.last;
    hold.last = tick;
    match hold.state {
        HoldState::Walk => {
            let saber = |c: usize| sim.weapon_classes.get(c).is_some_and(|w| w.saber.is_some());
            let melee = saber(u.weapon_class) || u.held.iter().any(|w| saber(w.class));
            if u.mount.is_none() && !melee {
                search(sim, &mut hold, target, dt);
            }
        }
        HoldState::ToHint | HoldState::Shuffle => {
            hold.stamp = tick;
            if !moved(&mut hold, target) && hold.via.is_none() {
                let end = match hold.state {
                    HoldState::Shuffle => Some(hold.end),
                    _ => hint_goal(sim, hold.hint),
                };
                if end.is_none_or(|e| walked(u, e)) {
                    let near = hint_pos(sim, hold.hint).is_some_and(|h| {
                        h.distance_squared(u.soldier.pos) <= HINT_NEAR * HINT_NEAR
                    });
                    if near {
                        hold.state = HoldState::AtHint;
                    } else {
                        walk(&mut hold);
                    }
                }
            }
        }
        HoldState::AtHint => {
            hold.stamp = tick;
            let draw = crate::ai::unit_rand(i as u32, tick, u.gen, crate::ai::SALT_DEFEND) % 101;
            if !moved(&mut hold, target) && draw > SHUFFLE_OVER {
                shuffle(sim, i, &mut hold);
            }
        }
        HoldState::Noise => {
            if walked(u, hold.noise) || tick > hold.since + NOISE_WALK {
                hold.state = HoldState::Look;
                hold.noise = u.soldier.pos;
                hold.since = tick;
                hold.turns = 0;
                hold.until = None;
                let unit = crate::ai::unit_unit(i as u32, tick, u.gen, crate::ai::SALT_DEFEND ^ 5);
                let a = (LOOK_MIN + (LOOK_MAX - LOOK_MIN) * unit).to_radians();
                let left =
                    crate::ai::unit_rand(i as u32, tick, u.gen, crate::ai::SALT_DEFEND ^ 6) & 1;
                look(u, &mut hold, if left != 0 { a } else { -a });
            }
        }
        HoldState::Look => {
            let to = (hold.gaze - u.soldier.pos).xz();
            let (fwd, _, _) = u.aim_frame();
            let faced = to.normalize_or_zero().dot(fwd.xz().normalize_or_zero()) > LOOK_FACED;
            if hold.until.is_none() && faced {
                hold.until = Some(tick + (LOOK_HOLD * crate::TICK_HZ as f32) as u64);
            }
            if hold.until.is_some_and(|t| tick > t) {
                if hold.turns < LOOK_TURNS {
                    let turn = hold.turn * -2.0;
                    look(u, &mut hold, turn);
                } else {
                    walk(&mut hold);
                }
            }
        }
        HoldState::Fight => {}
    }
    let stand = match hold.state {
        HoldState::ToHint | HoldState::AtHint => true,
        HoldState::Walk => u.ai.mv.arrived,
        HoldState::Shuffle | HoldState::Fight | HoldState::Noise | HoldState::Look => false,
    };
    let factor = match hold.state {
        HoldState::Noise | HoldState::Look => THINK_FIGHT,
        _ if stand => THINK_STAND,
        _ => THINK_WALK,
    };
    let level = sim.self_level(u.soldier.pos, u.ai_eye(), u.aim_frame().0);
    let secs = factor * crate::sim::self_turrets::LEVEL_THINK[level];
    hold.next = tick + (secs * crate::TICK_HZ as f32) as u64;
    Some(hold)
}

/// `FUN_002cef98`: the next look point, [`LOOK_REACH`] along the bot's
/// facing turned by `turn` about the vertical, at its own height.
fn look(u: &crate::unit::Unit, hold: &mut DefendHold, turn: f32) {
    let (s, c) = crate::fmath::sin_cos(u.soldier.yaw + turn);
    hold.gaze = u.soldier.pos + Vec3::new(-s, 0.0, -c) * LOOK_REACH;
    hold.turn = turn;
    hold.turns += 1;
    hold.until = None;
}

/// A defender on foot investigates a noise from a hostile unit. A shot
/// carries farther than footsteps. The defender walks toward the first
/// eligible sound in unit order and approaches as far as the terrain
/// allows. See `FUN_002de708`, `FUN_002a8ef0`, and `FUN_002ce648`.
///
/// PROVISIONAL. The port uses fixed hearing checks for humans and handles
/// close sightings in the vision pass. replace: trace weapon noise range,
/// shot timing, human hearing cadence and the eligibility checks in
/// `FUN_002de708`, `FUN_002accb0`, and `FUN_002a9e38`.
fn noise(sim: &Sim, i: usize) -> Option<Vec3> {
    let u = &sim.units[i];
    if u.mount.is_some() {
        return None;
    }
    let tick = sim.tick;
    let nav = sim.nav.as_ref()?;
    sim.units.iter().enumerate().find_map(|(j, o)| {
        if j == i || !o.alive || o.mount.is_some() || !sim.hostile(o.team, u.team) {
            return None;
        }
        let human = o.controller != Controller::Ai || o.is_human();
        let every = if human {
            NOISE_HUMAN
        } else {
            crate::ai::THINK_PERIOD
        };
        let pass = if human {
            tick % NOISE_HUMAN == 0
        } else {
            tick > 0 && o.ai.last_think == tick - 1
        };
        if !pass {
            return None;
        }
        let fired = tick
            .checked_sub(o.last_shot_tick)
            .is_some_and(|d| d < every);
        let r = if fired {
            NOISE_SHOT
        } else {
            let v = crate::fmath::min(o.soldier.vel.length() * NOISE_SPEED, NOISE_SPEED_MAX);
            crate::fmath::max(crate::fmath::min(v, NOISE_SHOT), NOISE_FLOOR)
        };
        let d = o.soldier.pos - u.soldier.pos;
        let d2 = d.length_squared();
        let up = d.normalize_or_zero().y.abs() >= NOISE_UP;
        let heard = d2 <= r * r && (!up || d2 < NOISE_UP_NEAR * NOISE_UP_NEAR);
        if !heard || d2 <= NOISE_SEEN * NOISE_SEEN {
            return None;
        }
        let q = nav.clear_goal(
            o.soldier.pos.xz(),
            crate::unit::SOLDIER_RADIUS,
            crate::nav::SOLDIER,
        );
        Some(Vec3::new(q.x, o.soldier.pos.y, q.y))
    })
}

/// `FUN_002d3118`: a target more than [`HINT_ANCHOR`] from the anchor moves
/// the anchor there and sends the helper back to its walk.
fn moved(hold: &mut DefendHold, target: Vec3) -> bool {
    if hold.anchor.distance_squared(target) <= HINT_ANCHOR * HINT_ANCHOR {
        return false;
    }
    hold.anchor = target;
    walk(hold);
    true
}

/// `FUN_002d2f98`: the clock runs while the target stays within
/// [`HINT_ANCHOR`] of the anchor (a farther one moves the anchor and zeroes
/// it); past [`HINT_WAIT`] it is set back by [`HINT_RETRY`] and the nearest
/// free hint is taken (`FUN_002d2e40`: stamped, state 5).
fn search(sim: &Sim, hold: &mut DefendHold, target: Vec3, dt: u64) {
    if hold.anchor.distance_squared(target) < HINT_ANCHOR * HINT_ANCHOR {
        hold.clock = hold.clock.saturating_add(dt);
    } else {
        hold.clock = 0;
        hold.anchor = target;
    }
    if hold.clock <= HINT_WAIT {
        return;
    }
    hold.clock = HINT_WAIT - HINT_RETRY;
    if let Some(k) = free_hint(sim, target) {
        hold.hint = Some(k);
        hold.stamp = sim.tick;
        hold.state = HoldState::ToHint;
    }
}

/// `FUN_002a62e0` over `FUN_002ade38`: hint `k` is taken while any unit's
/// last stamp on it, the asking unit's own included, is under [`HINT_KEEP`]
/// old. OURS: a commander defender's spot on it also holds it.
/// replace: measure how long the PS2 keeps a hint reserved after its holder
/// stops stamping it, then drop the defender hold if it is not needed.
fn taken(sim: &Sim, k: u32, at: Vec3) -> bool {
    let tick = sim.tick;
    sim.ai_goals
        .hold
        .iter()
        .flatten()
        .any(|h| h.hint == Some(k) && tick < h.stamp.saturating_add(HINT_KEEP))
        || sim.commander.iter().any(|plan| {
            (0..sim.units.len()).any(|j| {
                plan.spot(j as u32)
                    .is_some_and(|(p, _)| p.distance_squared(at) < 0.01)
            })
        })
}

/// `FUN_002d2e40` over the type "7" query, then over the Cover one: the
/// nearest free hint to `target` in the [`HINT_BOX`]. PROVISIONAL (ours):
/// the disc's query returns at most 30 hints in its tree's order, and a hint
/// is also taken by a unit stamping it for a fight or a snipe stand; every
/// hint in the box is a candidate here and only the Defend members' stamps
/// count. replace: the tree's order and the hint stamps of the other
/// helpers.
fn free_hint(sim: &Sim, target: Vec3) -> Option<u32> {
    let nav = sim.nav.as_ref()?;
    for kind in [HINT_DEFEND, HINT_COVER] {
        let mut best: Option<(f32, usize)> = None;
        for (k, h) in nav.hints.iter().enumerate() {
            let d = h.pos - target;
            let out = d.x.abs() > HINT_BOX || d.y.abs() > HINT_BOX || d.z.abs() > HINT_BOX;
            if h.kind != kind || out {
                continue;
            }
            let d2 = d.length_squared();
            if best.is_none_or(|(b, _)| d2 < b) && !taken(sim, k as u32, h.pos) {
                best = Some((d2, k));
            }
        }
        if let Some((_, k)) = best {
            return Some(k as u32);
        }
    }
    None
}

/// A defender occasionally looks for a reachable hint near its current
/// position. Nearby barrier corners offer places to search from, and a bot
/// keeps its current hint if it finds no better position (`FUN_002d2168`).
///
/// PROVISIONAL. Search uses map index order and movement uses the usual
/// route planner. replace: trace the original query order, barrier
/// clearance (`FUN_002eed40`), and movement flags for a direct step.
fn shuffle(sim: &Sim, i: usize, hold: &mut DefendHold) {
    let u = &sim.units[i];
    let (me, tick, gen) = (i as u32, sim.tick, u.gen);
    let draw = |k: u32| crate::ai::unit_rand(me, tick, gen, crate::ai::SALT_DEFEND ^ k);
    let unit = |k: u32| crate::ai::unit_unit(me, tick, gen, crate::ai::SALT_DEFEND ^ k);
    let pos = u.soldier.pos;
    let r = crate::unit::SOLDIER_RADIUS;
    let ty = crate::nav::SOLDIER;
    let Some(nav) = sim.nav.as_ref() else {
        return;
    };
    let mut via = None;
    if draw(1) & 1 != 0 {
        let near = nav
            .barriers
            .iter()
            .find(|b| b.blocks(ty) && b.contains_r(pos.xz(), SHUFFLE_BARRIER));
        if let Some(b) = near {
            let first = draw(2) & 3;
            via = (0..4)
                .map(|k| b.corner(r * 1.1 + 0.5, first + k))
                .find(|c| !nav.leg_hits_barrier(pos.xz(), *c, r, ty))
                .map(|c| Vec3::new(c.x, pos.y, c.y));
        }
    }
    let via = via.unwrap_or_else(|| {
        let step = |k: u32| (unit(k) - 0.5) * 2.0 * SHUFFLE_REACH;
        let to = Vec3::new(pos.x + step(3), pos.y, pos.z + step(4));
        let t = nav.barrier_stop(pos.xz(), to.xz(), ty);
        if t >= 1.0 {
            return to;
        }
        let d = to - pos;
        let len = d.length();
        pos + d * (crate::fmath::max(t * len - r, 0.0) / crate::fmath::max(len, f32::EPSILON))
    });
    let next = nav
        .hints
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            let d = h.pos - via;
            h.kind == HINT_DEFEND
                && d.x.abs() <= SHUFFLE_BOX
                && d.y.abs() <= SHUFFLE_BOX
                && d.z.abs() <= SHUFFLE_BOX
        })
        .take(10)
        .find(|(k, h)| {
            !taken(sim, *k as u32, h.pos) && !nav.leg_hits_barrier(via.xz(), h.pos.xz(), r, ty)
        });
    hold.end = pos;
    if let Some((k, h)) = next {
        hold.hint = Some(k as u32);
        hold.stamp = tick;
        hold.end = h.pos;
    }
    hold.via = Some(via);
    hold.state = HoldState::Shuffle;
}

/// Where unit `h`'s defend helper walks or stands, and the stances it
/// allows there, while it has a hint to go to, and where it heard a noise
/// (the walk there, then the stop); `None` while it walks to its target or
/// fights.
pub fn defend_walk(sim: &Sim, h: UnitHandle) -> Option<(Vec3, u8)> {
    let hold = sim.ai_goals.hold_of(h)?;
    if matches!(hold.state, HoldState::Noise | HoldState::Look) {
        return Some((hold.noise, 0));
    }
    let hint = sim.nav.as_ref()?.hints.get(hold.hint? as usize)?;
    match hold.state {
        HoldState::ToHint | HoldState::AtHint => {
            Some((hint_goal(sim, hold.hint)?, hint.stance_mask))
        }
        HoldState::Shuffle => Some(match hold.via {
            Some(v) => (v, 0),
            None if hold.end.distance_squared(hint.pos) < 0.01 => (hint.pos, hint.stance_mask),
            None => (hold.end, 0),
        }),
        HoldState::Walk | HoldState::Fight | HoldState::Noise | HoldState::Look => None,
    }
}

/// Where unit `h`'s combat helper looks while it stands after a noise
/// (state 3, `FUN_002cef98`).
pub fn defend_look(sim: &Sim, h: UnitHandle) -> Option<Vec3> {
    let hold = sim.ai_goals.hold_of(h)?;
    (hold.state == HoldState::Look).then_some(hold.gaze)
}

/// Where a Defend member holds: the post, the player, the station, the
/// object or the flag; a CTFOffense goal's region.
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
        GoalTarget::Ctf { region, .. } => Some(region),
        GoalTarget::Flag(f) => Some(sim.ctf.as_ref()?.flags.get(usize::from(f))?.current_pos),
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
