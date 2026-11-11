//! The bot census (bot rework stage 0, `analysis/bot-rework-plan.md` §4):
//! what each think did with its target and which tier levers worked, and
//! what the thinks cost. The rework's later stages are judged against it.
//!
//! DIAGNOSTIC OUTPUT only. The census functions are pure: no rays, no
//! draws, no writes to the sim or to the bot's state. They fill
//! [`NavTick`]'s `d1` and `levers`, which `Sim::step` folds into
//! [`BotCensus`] and [`ThinkHist`]. Neither is hashed and no rule reads them.
//!
//! The D1 line ("far-human preference", BOTD D1): on a think whose target is
//! a soldier in sight, is he a human, and is some other engageable enemy bot
//! at most half as far away? The same rate over bot targets is the
//! comparison: a bot that keeps a far human while a near bot shoots at it
//! should show up as a higher human rate than bot rate.
//!
//! Levers below [`EVENT_LEVERS`] are sampled on thinks (a count of thinks
//! where the lever held); the rest are events (a count of times it fired).

use crate::ai::state::{AiState, NavTick};
use crate::perception::{self, Contact};
use crate::sim::Sim;
use crate::skill::{Difficulty, Tier};
use crate::unit::{Unit, UnitHandle};

/// [`NavTick::d1`]: the think holds an enemy soldier in sight as its target.
pub const D1_TARGET: u8 = 1;
/// ...the target is a human (a player or a proxy).
pub const D1_HUMAN: u8 = 2;
/// ...another engageable enemy bot is at most half the target's distance.
pub const D1_FAR: u8 = 4;
/// ...the think took this target (it is new this think).
pub const D1_TAKEN: u8 = 8;

/// Think-sampled: the target is in sight but the tier's reaction time has
/// not passed.
pub const LV_REACT_WAIT: u32 = 1 << 0;
/// Think-sampled: the target is still under the far-hit search
/// (`perception::search_ticks`).
pub const LV_SEARCH: u32 = 1 << 1;
/// Think-sampled: the tier's pressure on humans decided the pick: the bot
/// holds the contact that scores best under the pressure, and another
/// scores best without it (`perception::choose_target`'s score, before its
/// hold margin and retarget wait).
pub const LV_PRESSURE: u32 = 1 << 2;
/// Think-sampled: the target in sight is beyond Normal's plain sight.
pub const LV_BEYOND_VIEW: u32 = 1 << 3;
/// Think-sampled: the target in sight is beyond this unit's plain sight
/// (only the scope reaches it).
pub const LV_SCOPE: u32 = 1 << 4;
/// Think-sampled: the hunt rule (`proxy::front`) overrides the commander.
pub const LV_HUNT: u32 = 1 << 5;
/// Think-sampled: card DFOCUS gives this bot a human to focus on.
pub const LV_DFOCUS: u32 = 1 << 6;
/// Think-sampled: the threat pick (`proxy::pick`) names a target.
pub const LV_PICK: u32 = 1 << 7;
/// Think-sampled: the bot is going to a health droid (`proxy::mend`).
pub const LV_MEND: u32 = 1 << 8;
/// Think-sampled: a focuser walks to where its human was last seen.
pub const LV_PUSH: u32 = 1 << 9;
/// Think-sampled: running from a grenade.
pub const LV_GRENADE_FLEE: u32 = 1 << 10;
/// Think-sampled: a grenade is in range but the bot is not running (the
/// tier's coin said no, or its delay has not passed).
pub const LV_GRENADE_IGNORE: u32 = 1 << 11;
/// Think-sampled: the bot holds a Cover role (`ai::team::board`).
pub const LV_COVER_ROLE: u32 = 1 << 12;
/// The first event lever's bit index.
pub const EVENT_LEVERS: u32 = 16;
/// Event: a hit started a flinch (playtest C1).
pub const LV_FLINCH: u32 = 1 << 16;
/// Event: a bot on a position answered a hit (card REACT) by strafing...
pub const LV_REACT_STRAFE: u32 = 1 << 17;
/// ...by rolling...
pub const LV_REACT_ROLL: u32 = 1 << 18;
/// ...by moving to a new position...
pub const LV_REACT_MOVE: u32 = 1 << 19;
/// ...or by staying.
pub const LV_REACT_STAY: u32 = 1 << 20;
/// Event: a hit counted against a Tactician's position (`tactic::hit`).
pub const LV_POSITION_HIT: u32 = 1 << 21;
/// Event: a bot walking its leg side-stepped a hit (the skill's coin).
pub const LV_DODGE: u32 = 1 << 22;

/// The report's name of each lever bit ("" = unused).
pub const LEVER_NAMES: [&str; 32] = [
    "reaction-wait",
    "search",
    "pressure-decisive",
    "beyond-view",
    "scope",
    "hunt",
    "dfocus-take",
    "pick",
    "mend",
    "dfocus-push",
    "grenade-flee",
    "grenade-ignore",
    "cover-role",
    "",
    "",
    "",
    "flinch",
    "react-strafe",
    "react-roll",
    "react-move",
    "react-stay",
    "position-hit",
    "dodge",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// The D1 bits for bot `me` after this think's target choice. `taken`: the
/// think took its target. 0 with no enemy soldier in sight as the target.
///
/// "Engageable" is worked out from what the bot already perceives: a contact
/// seen this think, admitted (`perception::target_admitted`), an enemy bot
/// (not a human), within the bot's fight reach.
pub fn d1(sim: &Sim, me: UnitHandle, st: &AiState, taken: bool) -> u8 {
    let Some(u) = sim.unit(me) else {
        return 0;
    };
    let Some(t) = st
        .target
        .and_then(|h| perception::contact_of(st, h))
        .filter(|c| c.seen_now)
    else {
        return 0;
    };
    let Some(o) = t.handle.live_unit(sim) else {
        return 0;
    };
    let eye = u.ai_eye();
    let dt = (t.last_pos - eye).length();
    let reach = super::behave::fight_reach(sim, u);
    let far = st.contacts.iter().flatten().any(|c| {
        c.handle != t.handle
            && c.seen_now
            && c.handle
                .live_unit(sim)
                .is_some_and(|e| sim.hostile(u.team, e.team) && !e.is_human())
            && perception::target_admitted(sim, me, c)
            && {
                let d = (c.last_pos - eye).length();
                d <= reach && 2.0 * d <= dt
            }
    });
    let mut bits = D1_TARGET;
    if o.is_human() {
        bits |= D1_HUMAN;
    }
    if far {
        bits |= D1_FAR;
    }
    if taken {
        bits |= D1_TAKEN;
    }
    bits
}

/// The think-sampled levers that can be read off the state after the
/// target choice (the others are set where they happen in the chain).
pub fn think_levers(sim: &Sim, me: UnitHandle, u: &Unit, st: &AiState, tier: &Tier) -> u32 {
    let mut bits = 0;
    let tick = sim.tick;
    let eye = u.ai_eye();
    if let Some(c) = st.target.and_then(|h| perception::contact_of(st, h)) {
        if c.first_seen > tick {
            bits |= LV_SEARCH;
        } else if c.seen_now && !perception::fireable(&c, tick, tier.reaction_ticks) {
            bits |= LV_REACT_WAIT;
        }
        if let Some(o) = c.handle.live_unit(sim).filter(|_| c.seen_now) {
            let d = (o.capsule().center() - eye).length();
            let normal = perception::view_distance(o.soldier.stance, sim.mission_ai.view_mult);
            let plain = perception::view_distance(o.soldier.stance, tier.view_mult);
            let vis = o.cloak.visibility();
            if d >= normal * vis {
                bits |= LV_BEYOND_VIEW;
            }
            if d >= plain * vis {
                bits |= LV_SCOPE;
            }
        }
    }
    if tier.pressure != 1.0 && st.target.is_some() {
        let pressed = best_by(sim, me, u, st, tier, tier.pressure);
        if pressed == st.target && pressed != best_by(sim, me, u, st, tier, 1.0) {
            bits |= LV_PRESSURE;
        }
    }
    if u.team < 2 && sim.deathmatch.is_none() && sim.space_assault.is_none() {
        if let Some(f) = super::proxy::front(sim, u) {
            if Some(f) != sim.commander[usize::from(u.team)].of(me.index) {
                bits |= LV_HUNT;
            }
        }
    }
    if st.role.post.is_some() {
        bits |= LV_COVER_ROLE;
    }
    bits
}

/// `perception::choose_target`'s best contact with `pressure` on humans (a
/// copy of its score, so the chooser's own arithmetic is left alone).
fn best_by(
    sim: &Sim,
    me: UnitHandle,
    u: &Unit,
    st: &AiState,
    tier: &Tier,
    pressure: f32,
) -> Option<perception::Foe> {
    let eye = u.ai_eye();
    let scale = perception::view_distance(crate::soldier::Stance::Stand, tier.view_mult);
    let score = |c: &Contact| -> f32 {
        let d = (c.last_pos - eye).length();
        let human = c.handle.live_unit(sim).is_some_and(|o| o.is_human());
        let base = (200.0 - 100.0 * d / scale) * if human { pressure } else { 1.0 };
        if c.seen_now {
            base + c
                .handle
                .live_unit(sim)
                .map_or(0.0, |o| crate::ai::vehicle_bonus(sim, u, o))
        } else {
            base - 10.0 * c.age_s(sim.tick)
        }
    };
    let mut best: Option<(f32, perception::Foe)> = None;
    for c in st
        .contacts
        .iter()
        .flatten()
        .filter(|c| perception::target_admitted(sim, me, c))
    {
        let s = score(c);
        if best.is_none_or(|(bs, _)| s > bs) {
            best = Some((s, c.handle));
        }
    }
    best.map(|b| b.1)
}

/// Is a grenade on the ground near unit `u` that it did not throw (inside
/// its warning radius, as `hazard_flee` tests)?
pub fn hazard_near(sim: &Sim, me: UnitHandle, u: &Unit) -> bool {
    let pos = u.soldier.pos;
    sim.hazards.iter().any(|z| {
        let d = pos - z.pos;
        z.owner != Some(me)
            && glam::Vec2::new(d.x, d.z).length() < z.radius
            && d.y.abs() <= z.radius
    })
}

/// One census group's counts ([`BotCensus`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CensusCell {
    /// AI ticks on foot.
    pub foot_ticks: u64,
    /// Thinks on foot.
    pub thinks: u64,
    /// Thinks with an enemy soldier in sight as the target, by
    /// `(d1 >> 1) & 7`: bit 0 human, bit 1 far, bit 2 taken.
    pub d1: [u64; 8],
    /// Per lever bit ([`LEVER_NAMES`]): think-sampled levers count thinks,
    /// event levers count events.
    pub levers: [u64; 32],
}

impl CensusCell {
    /// Fold one AI unit's tick on foot in.
    fn add(&mut self, nt: &NavTick) {
        self.foot_ticks += 1;
        self.thinks += u64::from(nt.thought != 0);
        if nt.d1 & D1_TARGET != 0 {
            self.d1[usize::from(nt.d1 >> 1 & 7)] += 1;
        }
        if nt.levers != 0 {
            for (b, n) in self.levers.iter_mut().enumerate() {
                *n += u64::from(nt.levers >> b & 1);
            }
        }
    }

    /// Add another cell's counts.
    pub fn merge(&mut self, o: &CensusCell) {
        self.foot_ticks += o.foot_ticks;
        self.thinks += o.thinks;
        for (a, b) in self.d1.iter_mut().zip(o.d1) {
            *a += b;
        }
        for (a, b) in self.levers.iter_mut().zip(o.levers) {
            *a += b;
        }
    }

    /// The D1 counts: `[human, human far, human taken, human taken far,
    /// bot, bot far, bot taken, bot taken far]`.
    pub fn d1_split(&self) -> [u64; 8] {
        let mut out = [0u64; 8];
        for (i, &n) in self.d1.iter().enumerate() {
            let (human, far, taken) = (i & 1 != 0, i & 2 != 0, i & 4 != 0);
            let base = if human { 0 } else { 4 };
            out[base] += n;
            if far {
                out[base + 1] += n;
            }
            if taken {
                out[base + 2] += n;
                if far {
                    out[base + 3] += n;
                }
            }
        }
        out
    }
}

/// The round's census, by group: the row the unit's team plays
/// (`Sim::difficulty_of`), whether a hostile team has a human (a player or
/// a proxy, `Sim::humans_per_team`), and bot or proxy. In a proxy round
/// the proxy's allies play a lower row than the bots facing him (card
/// DTEAM), so the groups keep the two apart. A diagnostic OUTPUT like
/// `Sim::nav_stats`: not hashed, never read by the sim. Space rounds are
/// not counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BotCensus {
    /// `[row][vs_human][proxy]`, the row in `Difficulty::ALL` order.
    pub cells: [[[CensusCell; 2]; 2]; 4],
}

/// `Difficulty`'s index in `Difficulty::ALL`.
fn row_index(d: Difficulty) -> usize {
    usize::from(d.bytes()[0])
}

impl BotCensus {
    /// Fold one AI unit's tick in. `row`: its team's row; `vs_human`: a
    /// hostile team has a human; `foot`: it was on foot this tick.
    pub fn add(&mut self, row: Difficulty, vs_human: bool, proxy: bool, foot: bool, nt: &NavTick) {
        if foot {
            self.cells[row_index(row)][usize::from(vs_human)][usize::from(proxy)].add(nt);
        }
    }

    /// One group's counts.
    pub fn cell(&self, row: Difficulty, vs_human: bool, proxy: bool) -> &CensusCell {
        &self.cells[row_index(row)][usize::from(vs_human)][usize::from(proxy)]
    }

    /// The groups `keep(row, vs_human, proxy)` accepts, added together.
    pub fn sum(&self, keep: impl Fn(Difficulty, bool, bool) -> bool) -> CensusCell {
        let mut out = CensusCell::default();
        for row in Difficulty::ALL {
            for vs in [false, true] {
                for proxy in [false, true] {
                    if keep(row, vs, proxy) {
                        out.merge(self.cell(row, vs, proxy));
                    }
                }
            }
        }
        out
    }

    /// Add another round's census.
    pub fn merge(&mut self, o: &BotCensus) {
        for (a, b) in self
            .cells
            .iter_mut()
            .flatten()
            .flatten()
            .zip(o.cells.iter().flatten().flatten())
        {
            a.merge(b);
        }
    }
}

/// Buckets of [`Hist`]: four per power of two of nanoseconds, up to 2^32.
pub const HIST_BUCKETS: usize = 128;

/// A log-bucketed histogram of think wall times (a 25 % bucket width).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hist {
    pub n: u64,
    pub sum_ns: u64,
    pub max_ns: u32,
    pub buckets: [u64; HIST_BUCKETS],
}

impl Default for Hist {
    fn default() -> Self {
        Hist {
            n: 0,
            sum_ns: 0,
            max_ns: 0,
            buckets: [0; HIST_BUCKETS],
        }
    }
}

impl Hist {
    fn bucket(ns: u32) -> usize {
        if ns < 4 {
            return ns as usize;
        }
        let e = 31 - ns.leading_zeros();
        let sub = (ns >> (e - 2)) & 3;
        (4 * e + sub) as usize
    }

    /// The exclusive upper edge of bucket `i`, in nanoseconds.
    fn upper(i: usize) -> u64 {
        if i < 8 {
            return i as u64 + 1;
        }
        let (e, sub) = ((i / 4) as u64, (i % 4) as u64);
        (4 + sub + 1) << (e - 2)
    }

    pub fn add(&mut self, ns: u32) {
        self.n += 1;
        self.sum_ns += u64::from(ns);
        self.max_ns = self.max_ns.max(ns);
        self.buckets[Self::bucket(ns).min(HIST_BUCKETS - 1)] += 1;
    }

    pub fn merge(&mut self, o: &Hist) {
        self.n += o.n;
        self.sum_ns += o.sum_ns;
        self.max_ns = self.max_ns.max(o.max_ns);
        for (a, b) in self.buckets.iter_mut().zip(o.buckets) {
            *a += b;
        }
    }

    /// The `q` quantile (0..=1) as its bucket's upper edge, in
    /// microseconds; 0 when empty.
    pub fn quantile_us(&self, q: f64) -> f64 {
        if self.n == 0 {
            return 0.0;
        }
        let want = ((self.n as f64 * q).ceil() as u64).clamp(1, self.n);
        let mut seen = 0;
        for (i, &c) in self.buckets.iter().enumerate() {
            seen += c;
            if seen >= want {
                return Self::upper(i) as f64 / 1000.0;
            }
        }
        f64::from(self.max_ns) / 1000.0
    }
}

/// What the bots' thinks cost: every call, the calls that ran a think, and
/// the proxies' calls. A measurement of the machine like `Sim::tick_cost`:
/// never read by the sim, not hashed, not repeatable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThinkHist {
    pub all: Hist,
    pub thinks: Hist,
    pub proxy: Hist,
}

impl ThinkHist {
    pub fn add(&mut self, nt: &NavTick, proxy: bool) {
        self.all.add(nt.think_ns);
        if nt.thought != 0 {
            self.thinks.add(nt.think_ns);
        }
        if proxy {
            self.proxy.add(nt.think_ns);
        }
    }

    pub fn merge(&mut self, o: &ThinkHist) {
        self.all.merge(&o.all);
        self.thinks.merge(&o.thinks);
        self.proxy.merge(&o.proxy);
    }
}

#[cfg(test)]
mod tests;
