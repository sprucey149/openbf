//! Card ARCM: failure memory on nav links, and a back-and-forth detector
//! (`analysis/bot-design.md` 6.3, 9 D20). OURS: the PS2 retries a failed
//! walk's arc (BF2) or stalls on it (BF1), and its stuck test is a clock.
//!
//! - **The bot's own memory** ([`ArcmState::closed`]): an arc whose walk
//!   fails twice running (a progress window failed on it with an earlier
//!   failure since the walk last got anywhere — `MoverState::recover` — or
//!   the back-and-forth detector fired on it) is closed to that bot for
//!   [`ARC_CLOSED`], so the Recover ladder's re-plan (`route::replan`) does
//!   not draw it again. Pogamut's path executor reports which link failed.
//! - **The team's** (`Sim::arc_fails`): a closing counts against the arc for
//!   the bot's team, for the round, once per bot in turn; at
//!   [`ARC_TEAM_FAILS`] the arc leaves the team's route draw. Both are a
//!   preference: `route::plan_leg` draws round a shunned arc only while
//!   another way to the goal exists, and never strands a bot.
//! - **Back and forth** ([`Osc`]): on an arc leg, a bot no rule excuses
//!   from the walk (not fighting a seen enemy, holding, fleeing or under an
//!   order: `MoveRequest::excused`), the distance to the waypoint the
//!   progress window measures turns between closing and opening by over
//!   [`OSC_AMP`] three times inside [`OSC_SPAN`] with no new ground — the
//!   window closes then as a failed one (`Why::Oscillation`), so Recover
//!   starts early, and the arc is closed to the bot. The window's own test passes a bot shoved to and fro at a
//!   door that nets [`super::STUCK_PROGRESS`] in a window now and then.
//!   Pogamut's distance detector: "if it oscillates more than two times ->
//!   STUCK".

use super::state::AiState;
use crate::fmath::FloatExt;
use crate::TICK_HZ;
use glam::{Vec2, Vec3};

/// How long an arc whose walk failed stays closed to the bot (ticks): 60 s.
/// PROVISIONAL (ours; `analysis/bot-design.md` 9 D20).
pub const ARC_CLOSED: u64 = 60 * TICK_HZ as u64;
/// Closings of one arc by the team's bots (each counted once in turn) that
/// take it out of the team's route draw for the round. PROVISIONAL (ours).
pub const ARC_TEAM_FAILS: u8 = 3;
/// Arcs a bot remembers at once (the oldest closing makes room).
pub const ARC_SLOTS: usize = 3;
/// A turn of the distance to the waypoint counts once it has run this far
/// back (m) — over a stride's sway and a side-step's. MEASURED (6 seeds x 34
/// maps, card ARCM): 0.75 m fired a third more often and cost more time in
/// Recover (1.45 % against 1.34 %) and more recycles (33 against 23).
pub const OSC_AMP: f32 = 1.0;
/// Three turns inside this many ticks (5 s) are a back-and-forth.
pub const OSC_SPAN: u64 = 5 * TICK_HZ as u64;
/// Getting this much nearer the waypoint than ever before (m) is new ground:
/// the count starts again.
pub const OSC_GAIN: f32 = 1.0;
/// The waypoint watched moved by more than this (m): a new one.
const OSC_KEY_MOVE: f32 = 0.5;

/// The back-and-forth detector's books for the waypoint being walked at.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Osc {
    /// The waypoint (XZ); the books start again when it moves.
    pub key: Vec2,
    /// The distance at the last turn, run on to the extreme since.
    pub ext: f32,
    /// The nearest the walk has been, as of the last new ground.
    pub best: f32,
    /// +1 opening, -1 closing, 0 not yet known.
    pub dir: i8,
    /// The ticks of the last two turns (0 = none), oldest first.
    pub turns: [u64; 2],
}

impl Osc {
    /// One tick of a walk at `target` from `pos`: is this the third turn
    /// inside [`OSC_SPAN`]?
    pub fn step(&mut self, target: Vec3, pos: Vec3, tick: u64) -> bool {
        let key = Vec2::new(target.x, target.z);
        let d = (key - Vec2::new(pos.x, pos.z)).length();
        if self.key == Vec2::ZERO || (key - self.key).length() > OSC_KEY_MOVE {
            *self = Osc {
                key,
                ext: d,
                best: d,
                ..Osc::default()
            };
            return false;
        }
        if d < self.best - OSC_GAIN {
            self.best = d;
            self.turns = [0; 2];
        }
        let to = match self.dir {
            1 if d < self.ext - OSC_AMP => -1,
            -1 if d > self.ext + OSC_AMP => 1,
            0 if d > self.ext + OSC_AMP => 1,
            0 if d < self.ext - OSC_AMP => -1,
            1 => {
                self.ext = self.ext.fmax(d);
                return false;
            }
            -1 => {
                self.ext = self.ext.fmin(d);
                return false;
            }
            _ => return false,
        };
        let turned = self.dir != 0;
        self.dir = to;
        self.ext = d;
        if !turned {
            return false;
        }
        if self.turns[0] != 0 && tick.saturating_sub(self.turns[0]) <= OSC_SPAN {
            self.turns = [0; 2];
            self.best = d;
            return true;
        }
        self.turns = [self.turns[1], tick];
        false
    }
}

/// A bot's card-ARCM books: the arcs closed to it and the back-and-forth
/// detector. Hashed only when not the default, after `flank`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ArcmState {
    /// (arc, closed until this tick); until 0 = an empty slot.
    pub closed: [(u16, u64); ARC_SLOTS],
    pub osc: Osc,
}

impl ArcmState {
    /// Is `arc` closed to this bot at `tick`?
    pub fn is_closed(&self, arc: u16, tick: u64) -> bool {
        self.closed.iter().any(|&(a, t)| a == arc && tick < t)
    }

    /// Does the bot remember any arc closed at `tick`?
    pub fn any_closed(&self, tick: u64) -> bool {
        self.closed.iter().any(|&(_, t)| tick < t)
    }

    /// Close `arc` until `tick + ARC_CLOSED`: in its own slot when it has
    /// one, else the slot that runs out first.
    pub fn close(&mut self, arc: u16, tick: u64) {
        let until = tick + ARC_CLOSED;
        let slot = match self.closed.iter().position(|&(a, t)| a == arc && t != 0) {
            Some(i) => i,
            None => (0..ARC_SLOTS)
                .min_by_key(|&i| self.closed[i].1)
                .unwrap_or(0),
        };
        self.closed[slot] = (arc, until);
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"ARCM");
        for (a, t) in self.closed {
            eat(&a.to_le_bytes());
            eat(&t.to_le_bytes());
        }
        let o = &self.osc;
        for v in [o.key.x, o.key.y, o.ext, o.best] {
            eat(&v.hash_bits().to_le_bytes());
        }
        eat(&[o.dir as u8]);
        for t in o.turns {
            eat(&t.to_le_bytes());
        }
    }
}

/// One team's books on one arc for the round (`Sim::arc_fails`): the
/// closings counted, and the bot (slot + 1) that made the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArcFails {
    pub n: u8,
    pub last: u32,
}

impl crate::sim::Sim {
    /// Card ARCM: has team `team` shunned `arc` this round?
    pub fn arc_shunned(&self, team: u8, arc: u16) -> bool {
        self.arc_fails
            .get(usize::from(team))
            .and_then(|t| t.get(usize::from(arc)))
            .is_some_and(|f| f.n >= ARC_TEAM_FAILS)
    }

    /// Does team `team` shun any arc?
    pub fn any_shunned(&self, team: u8) -> bool {
        self.arc_fails
            .get(usize::from(team))
            .is_some_and(|t| t.iter().any(|f| f.n >= ARC_TEAM_FAILS))
    }

    /// The write-back (unit-index order): unit `unit` of `team` closed
    /// `arc`. Counted unless the same bot made the arc's last count.
    /// Answers whether the arc became shunned now.
    pub(crate) fn note_arc_closed(&mut self, team: u8, arc: u16, unit: u32) -> bool {
        let Some(t) = self.arc_fails.get_mut(usize::from(team)) else {
            return false;
        };
        let n = self.nav.as_ref().map_or(0, |g| g.arcs.len());
        if t.len() < n {
            t.resize(n, ArcFails::default());
        }
        let Some(f) = t.get_mut(usize::from(arc)) else {
            return false;
        };
        if f.last == unit + 1 {
            return false;
        }
        f.last = unit + 1;
        f.n = f.n.saturating_add(1);
        f.n == ARC_TEAM_FAILS
    }
}

/// Is `arc` to be drawn round by this bot at `tick`: closed to it, or
/// shunned by its team? (A soldier on foot only: `route::plan_leg`.)
pub(super) fn shunned(sim: &crate::sim::Sim, st: &AiState, team: u8, arc: u16) -> bool {
    st.arcm.is_closed(arc, sim.tick) || sim.arc_shunned(team, arc)
}

#[cfg(test)]
mod tests;
