//! Phase B: what every AI unit is FOR — the PS2's Conquest goal
//! (`analysis/re-conquest-goal.md`, audited PARTLY 2026-10-02 - its banner and
//! section 10; its section 8 is this module's build list). One [`TeamPlan`] per team holds each unit's target —
//! attack that post, hold this one, or go hunt — and `core::ai`'s follower
//! obeys it instead of Phase A's "walk at the nearest post that is not mine".
//!
//! The PS2 has NO team planner. Each bot draws ONE post that is not its
//! team's, at random by the post's authored value, and keeps it; nothing in
//! the draw looks at distance, enemies or crowding (doc sections 2, 3, 6).
//! [`draw_post`] is that draw; [`plan`] is the goal's 1 s tick, which draws
//! only for a unit that has no target or whose target is done:
//!
//! | a unit draws when | BF2 | BF1 |
//! |---|---|---|
//! | it has no target (a new life) | yes | yes |
//! | its target is its own team's | when `value_def` is 0 | at once (and when an own target is lost) |
//! | 5 s inside an own target's capture region | yes | yes, excluding that post |
//! | no route to the target | yes | PORT GUARD |
//! | a post lost to neutral | — | up to five units within 60 m retake it |
//!
//! OURS, each PROVISIONAL:
//!
//! - the 5 s clock restarts when the unit leaves the region or has a fight
//!   target: the PS2's capturers stay until an order interrupts the capture
//!   order and leave 5 s after it resumes, and the port's fight is not an
//!   order (doc section 8, item 2c);
//! - "the team can spawn at it" is read as "the team owns it";
//! - [`Objective::Hunt`] when a unit can route to no post that is left to
//!   take, and instead
//!   of holding a post once a side is down to its last reinforcements
//!   ([`crate::ai::HOLD_ENDGAME`]) — the port's endgame (card STEND); the PS2
//!   draws an own post there;
//! - the reachability drop on a BF1 map (BF1's draw has none), and the
//!   [`DWELL_GIVE_UP`] report, which re-draws without the post given up on;
//! - card DEFS, a NAMED DEVIATION (the maintainer, INBOX 2026-10-12: the PS2's
//!   capturers all hold the post, and the port's all stood on its one Cover
//!   hint): a post keeps at most a quarter of the team's living units as
//!   defenders, the nearest ([`DEFEND_SHARE`]); one once it has been quiet
//!   for 5 s and none after 20 s ([`defend_cap`], [`post_threatened`]); a
//!   defender over the cap draws again at once; and an authored hint is ONE
//!   defender's ([`TeamPlan::spots`], [`pick_spot`]);
//! - a vehicle's ban flags are not ported; the PS2's driver-shares-its-draw
//!   rule is: a passenger takes its AI driver's target.
//!
//! Properties that are not negotiable, because the replay spine rests on them:
//!
//! - it is SIM STATE, hashed with everything else ([`crate::sim::Sim::hash`]);
//! - it runs at a fixed cadence — team `t` plans when `tick` reaches its
//!   `next_plan_tick`, staggered so the two teams never plan on the same tick;
//! - teams in index order, units in index order, posts in index order, and the
//!   only randomness is a per-unit HASH ([`crate::ai::route_rand`]'s shape) —
//!   never the shared LCG, which `think` may not touch either.

use crate::collision::STEP_HEIGHT;
use crate::nav::Mover;
use crate::sim::Sim;
use crate::soldier::Stance;
use glam::Vec3;

/// Ticks between plans for one team (30 = 1 s): the PS2 goal manager's tick
/// (BF2 `FUN_002e7cf8`). Team `t` plans on ticks `t, t + 30, t + 60, ...`, so
/// the two teams never plan on the same tick and the pass costs at most one
/// team's worth of work per tick.
pub const PLAN_PERIOD: u64 = 30;
/// Plans (seconds) a unit spends inside its target's capture region before the
/// at-post helper's one test: 5.0 s (BF2 `FUN_002d5c30`, BF1 `FUN_00255fd0`).
pub const AT_POST_PLANS: u8 = 5;
/// [`TeamPlan::at_post`] once that test has run for this stay.
pub const AT_POST_TESTED: u8 = u8::MAX;
/// BF1's retake call (`FUN_00262090`): a post lost to neutral takes up to
/// this many AI units of the team ...
pub const RETAKE_UNITS: u32 = 5;
/// ... within this many metres of it.
pub const RETAKE_RADIUS: f32 = 60.0;
/// Defenders inside their post's capture volume take a Cover/Snipe hint within
/// this many metres OF THE POST. PROVISIONAL.
pub const HINT_RADIUS: f32 = 25.0;
/// Only a hint on the CENTRE's own level is offered ([`hint_on_level`]):
/// `false` restores the pre-HDL rule, where a hint 8 m up a wall was as good
/// as one on the floor (POL1's cp6con defenders, queue STK). PROVISIONAL: a
/// port routing rule — how the PS2 picks a defender's hint is not in
/// analysis/.
pub const HINT_SAME_LEVEL: bool = true;
/// Units on the same objective within this of each other are one squad.
/// PROVISIONAL.
pub const SQUAD_RADIUS: f32 = 20.0;
/// A unit within this of the post of an objective it has not completed is
/// DWELLING on it (`unreachable_dwell_max`). PROVISIONAL.
pub const DWELL_RADIUS: f32 = 10.0;
/// After this many ticks (10 s) of dwelling, the unit REPORTS the objective as
/// undoable and draws again without it. PROVISIONAL, ours.
///
/// MEASURED, the shape the pin takes: BF1 KAS2 units 5-15 spent 39,000-56,000
/// ticks directly UNDER cp1, 22 m below a post the graph says is four hops
/// away, and BF1 KAM1's units the same under cp1 and cp5. The route is fine;
/// the last leg is a cliff.
pub const DWELL_GIVE_UP: u16 = 300;

/// Card DEFS, rule 2 (OURS, a named deviation: the maintainer, INBOX 2026-10-12): at
/// most this share of the team's living units defends one post -
/// `max(1, round(share x alive))`, the nearest to the post first; a defender
/// over the cap draws again at once with the post excluded. The PS2 has no
/// cap: every unit that targeted a post holds it once it is taken (5 of 17
/// on one Cover hint on BF2 Kamino, 4-8 a metre apart on BF2 Hoth). This
/// and the six constants of rules 1, 3 and 4 below are PROVISIONAL.
pub const DEFEND_SHARE: f32 = 0.25;
/// Rule 3: a post is QUIET when it has had no threat ([`post_threatened`])
/// and no capture for this many ticks (5 s); the cap of a quiet post is
/// [`DEFEND_QUIET_CAP`], and the share's again from the plan a threat is
/// seen. OURS.
pub const DEFEND_QUIET_FALL: u64 = 150;
pub const DEFEND_QUIET_CAP: usize = 1;
/// Rule 4: after this many ticks (20 s) without a threat or a capture the
/// cap is 0 - the last defender draws again too, wherever it stands (the
/// PS2's 5 s test runs only inside the capture region, and a Cover hint is
/// mostly outside it). OURS.
pub const DEFEND_QUIET_LEAVE: u64 = 600;
/// Rule 3's distance: an enemy within this fraction of the distance between
/// the map's two farthest posts threatens a post ([`threat_radius`]), and
/// never less than [`THREAT_RADIUS_MIN`]. OURS.
pub const THREAT_FRAC: f32 = 0.2;
pub const THREAT_RADIUS_MIN: f32 = 30.0;
/// Rule 1: two defenders' spots are at least this far apart
/// ([`crate::ai::HOLD_SPACING`]'s value, the hub hold's): one defender a
/// hint, and a defender with no hint left walks to its own point of the
/// capture region. OURS.
pub const DEFEND_SPACING: f32 = 2.0;

/// `Hint::kind` values a defender will stand at (0 Snipe, 2 Cover — the
/// authored kinds `core::nav::Hint` documents).
pub const HINT_SNIPE: u8 = 0;
pub const HINT_COVER: u8 = 2;

/// What a unit is FOR.
///
/// `Escort(UnitHandle)` (Phase D, squad commands) and `Board(VehicleHandle)`
/// (Phase E, vehicles) are the two the design leaves the enum open for; they
/// are NOT built here, because nothing in the sim can carry them out yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objective {
    /// Take command post `cp` (an index into `Sim::cps`).
    Attack(u32),
    /// Hold command post `cp`, which my team owns: the same kept target as
    /// `Attack(cp)`, relabelled by [`plan`] while the team owns the post.
    Defend(u32),
    /// Nothing is left to take: go at the nearest enemy.
    Hunt,
}

impl Objective {
    /// The five bytes that go into the hash: a tag and the post index.
    pub fn bytes(self) -> [u8; 5] {
        let (tag, cp) = match self {
            Objective::Attack(c) => (1u8, c),
            Objective::Defend(c) => (2, c),
            Objective::Hunt => (3, 0),
        };
        let b = cp.to_le_bytes();
        [tag, b[0], b[1], b[2], b[3]]
    }

    /// A single number naming this objective — the SQUAD id.
    pub fn key(self) -> u32 {
        match self {
            Objective::Attack(c) => c,
            Objective::Defend(c) => 0x4000_0000 | c,
            Objective::Hunt => 0x8000_0000,
        }
    }

    pub fn label(self) -> String {
        match self {
            Objective::Attack(c) => format!("Attack cp{c}"),
            Objective::Defend(c) => format!("Defend cp{c}"),
            Objective::Hunt => "Hunt".to_owned(),
        }
    }

    /// The post this objective is about, if it is about one.
    pub fn cp(self) -> Option<u32> {
        match self {
            Objective::Attack(c) | Objective::Defend(c) => Some(c),
            Objective::Hunt => None,
        }
    }
}

/// One team's plan: what every unit SLOT is for, and when to think again.
///
/// `assignment` is indexed by unit slot exactly like `Sim::units`, so a slot
/// that is dead, on the other team, or not AI holds `None` — as does a unit
/// that spawned since the last plan, which is what makes it fall back to the
/// Phase A rule until the goal's next tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamPlan {
    pub assignment: Vec<Option<Objective>>,
    /// Per slot, the generation of the unit `assignment` was made for: a
    /// respawned slot holds no target. A slot past the end counts as current.
    pub gens: Vec<u32>,
    /// Per slot, the plans it has spent inside its target's capture region
    /// with nobody to fight, up to [`AT_POST_PLANS`]; then [`AT_POST_TESTED`].
    pub at_post: Vec<u8>,
    /// Every post's owner at the last plan: BF1's owner-change events.
    pub owners: Vec<Option<u8>>,
    /// Card DEFS: per slot, a defender's own hint and its allowed-stance
    /// mask; no two defenders of a team within [`DEFEND_SPACING`]. `None`: no
    /// defender, or no hint left for it.
    pub spots: Vec<Option<(Vec3, u8)>>,
    /// Card DEFS: per post, the last tick it was threatened
    /// ([`post_threatened`]) or became this team's.
    pub threat: Vec<u64>,
    pub next_plan_tick: u64,
}

impl TeamPlan {
    /// The plan for a fresh team: team `t` first plans on tick `t`.
    pub fn new(team: u8) -> TeamPlan {
        TeamPlan {
            next_plan_tick: u64::from(team),
            ..TeamPlan::default()
        }
    }

    pub fn of(&self, unit_index: u32) -> Option<Objective> {
        self.assignment.get(unit_index as usize).copied().flatten()
    }

    /// Card DEFS: the spot unit `unit_index` defends from, if it has one.
    pub fn spot(&self, unit_index: u32) -> Option<(Vec3, u8)> {
        self.spots.get(unit_index as usize).copied().flatten()
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.next_plan_tick.to_le_bytes());
        eat(&(self.assignment.len() as u32).to_le_bytes());
        for a in &self.assignment {
            // Length-prefixed, and `None` is a sentinel byte rather than a
            // skip: "unit 3 has no objective" and "there is no unit 3" are
            // different states and must not hash alike.
            match a {
                Some(o) => {
                    eat(&[1]);
                    eat(&o.bytes());
                }
                None => eat(&[0]),
            }
        }
        eat(&(self.gens.len() as u32).to_le_bytes());
        for g in &self.gens {
            eat(&g.to_le_bytes());
        }
        eat(&(self.at_post.len() as u32).to_le_bytes());
        eat(&self.at_post);
        eat(&(self.owners.len() as u32).to_le_bytes());
        for o in &self.owners {
            eat(&[o.unwrap_or(u8::MAX)]);
        }
        eat(&(self.spots.len() as u32).to_le_bytes());
        for s in &self.spots {
            match s {
                Some((p, m)) => {
                    eat(&[1, *m]);
                    for v in p.to_array() {
                        eat(&v.to_bits().to_le_bytes());
                    }
                }
                None => eat(&[0]),
            }
        }
        eat(&(self.threat.len() as u32).to_le_bytes());
        for t in &self.threat {
            eat(&t.to_le_bytes());
        }
    }
}

/// Which game's Conquest rules a map runs (doc sections 2-3 against 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalRules {
    Bf1,
    Bf2,
}

impl GoalRules {
    /// The sim's game: [`Sim::ground_rules`] is the flag every sim builder
    /// sets from the manifest's `source_game`.
    pub fn of(sim: &Sim) -> GoalRules {
        match sim.ground_rules {
            crate::soldier::GroundRules::Bf1 => GoalRules::Bf1,
            crate::soldier::GroundRules::Bf2 => GoalRules::Bf2,
        }
    }
}

/// One post as [`draw_post`] sees it, for the drawing unit's team.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawPost {
    /// The unit's team owns it.
    pub friendly: bool,
    /// Nobody owns it.
    pub neutral: bool,
    /// `CpDef::value_atk` / `value_def` for the unit's team.
    pub atk: i32,
    pub def: i32,
    /// The unit can route to it and it is not excluded.
    pub open: bool,
}

/// The Conquest goal's target draw (BF2 `FUN_002e8318`, BF1 `FUN_002618b8`):
/// the post index, `None` when there is nothing to draw. `r` is one uniform
/// number in `[0, 1)`.
///
/// Candidates are the open posts NOT friendly to the team. BF2 drops a value
/// `<= 0` and weighs `Value_ATK` (hostile) or the mean of ATK and DEF
/// (neutral); BF1 weighs `Value_ATK` for the PS2's team 1 (port team 0) and
/// `Value_DEF` for its team 2, whoever owns the post. The draw is the first
/// candidate, in post order, whose running weight passes `r x` the sum. With
/// no candidate BF2 draws among its own posts by `Value_DEF`; then both games
/// draw uniformly over all (open) posts — BF2 only when there are two.
pub fn draw_post(rules: GoalRules, team: u8, posts: &[DrawPost], r: f32) -> Option<u32> {
    let weight = |p: &DrawPost| -> f32 {
        let w = match rules {
            GoalRules::Bf2 if p.friendly => p.def as f32,
            GoalRules::Bf2 if p.neutral => (p.atk + p.def) as f32 * 0.5,
            GoalRules::Bf2 => p.atk as f32,
            GoalRules::Bf1 if team == 0 => p.atk as f32,
            GoalRules::Bf1 => p.def as f32,
        };
        w.max(0.0)
    };
    let pass = |friendly: bool| -> Option<u32> {
        let ok = |p: &DrawPost| p.open && p.friendly == friendly && weight(p) > 0.0;
        let sum: f32 = posts.iter().filter(|p| ok(p)).map(weight).sum();
        let (mut run, mut last) = (0.0f32, None);
        for (i, p) in posts.iter().enumerate().filter(|(_, p)| ok(p)) {
            run += weight(p);
            last = Some(i as u32);
            if r * sum < run {
                break;
            }
        }
        last
    };
    if let Some(c) = pass(false) {
        return Some(c);
    }
    if rules == GoalRules::Bf2 {
        if let Some(c) = pass(true) {
            return Some(c);
        }
        if posts.len() < 2 {
            return None;
        }
    }
    let n = posts.iter().filter(|p| p.open).count();
    let k = ((r * n as f32) as usize).min(n.saturating_sub(1));
    posts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.open)
        .nth(k)
        .map(|(i, _)| i as u32)
}

/// The draw's uniform number in `[0, 1)` for one unit at one plan: a hash,
/// not the shared LCG, so the plan is a pure function of sim state
/// ([`crate::ai::unit_rand`]'s FNV shape; the top 24 bits, so 1.0 is never
/// reached).
fn roll(unit_index: u32, tick: u64, gen: u32) -> f32 {
    let mut h = crate::ai::route_rand(unit_index, tick, gen);
    // FNV's last multiply leaves the low byte's change in the low bits only.
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h >> 8) as f32 / 16_777_216.0
}

/// Everything one plan needs to know about one command post, gathered once so
/// the per-unit loop is arithmetic only.
struct PostInfo {
    /// Where this post's hop-distance row starts in `plan`'s flat field
    /// (`hub_count` entries, `u16::MAX` = never); `None` when the post has no
    /// hub or the map has no graph.
    dist: Option<usize>,
    friendly: bool,
    neutral: bool,
    atk: i32,
    def: i32,
    /// Card MAIS: the mission lets this team's bots capture it
    /// ([`crate::cp::CommandPost::ai_no_capture`]).
    capturable: bool,
}

/// The Conquest goal's tick for team `team`. Called by
/// [`crate::sim::Sim::step`] phase 1, BEFORE the think loop, so the units
/// think against the plan made this same tick.
pub fn plan(sim: &mut Sim, team: u8) {
    let t = usize::from(team).min(1);
    let rules = GoalRules::of(sim);
    sim.commander[t].next_plan_tick = sim.tick + PLAN_PERIOD;
    sim.nav_stats.plans += 1;
    let old = std::mem::take(&mut sim.commander[t]);

    // --- the team's units, in index order, with the hub each starts from ---
    let mut units: Vec<(usize, u32, Vec3, Option<u16>)> = Vec::new();
    for i in 0..sim.units.len() {
        let u = &sim.units[i];
        // A Follow or Hold bot is the PLAYER'S while the order stands (brief
        // D5), so it is not in the pool at all: `assignment[i]` stays `None`,
        // the squad bookkeeping below skips it, and no plan can take the order
        // off it.
        if !u.alive
            || u.team != team
            || u.controller != crate::unit::Controller::Ai
            || u.command.is_some()
        {
            continue;
        }
        let (pos, gen) = (u.soldier.pos, u.gen);
        let hub = sim
            .nav
            .as_ref()
            .and_then(|g| crate::ai::start_hub(sim, g, sim.handle(i)));
        units.push((i, gen, pos, hub));
    }

    // --- the posts ---
    let mover = Mover::SOLDIER;
    // One jetless hop-distance field for every post, end to end, with one
    // scratch queue for every BFS: the commander's shared infantry policy.
    let hub_count = sim.nav.as_ref().map_or(0, |g| g.hubs.len());
    let mut hop_field: Vec<u16> = Vec::with_capacity(hub_count * sim.cps.len());
    let mut bfs_queue: Vec<u16> = Vec::new();
    let mut posts: Vec<PostInfo> = Vec::with_capacity(sim.cps.len());
    for c in &sim.cps {
        let dist = match (sim.nav.as_ref(), c.nav_hub) {
            (Some(g), Some(h)) => {
                let base = hop_field.len();
                g.hop_distances_into(h, mover, &mut hop_field, &mut bfs_queue);
                Some(base)
            }
            _ => None,
        };
        posts.push(PostInfo {
            dist,
            friendly: c.owner == Some(team),
            neutral: c.owner.is_none(),
            atk: c.def.value_atk[t],
            def: c.def.value_def[t],
            capturable: !c.ai_no_capture(team),
        });
    }
    // Unreachable from where the unit stands. No graph (or no hub for the
    // post, or for the unit): every post is open and the follower walks
    // straight at it, exactly as before Phase A.
    let reach = |hub: Option<u16>, p: &PostInfo| -> bool {
        match (hub, p.dist) {
            (Some(h), Some(base)) if usize::from(h) < hub_count => {
                hop_field[base + usize::from(h)] != u16::MAX
            }
            _ => true,
        }
    };
    // And the brain's endgame, a side down to its last reinforcements: only
    // kills end the round, so nobody holds a post — a unit whose target is
    // its team's draws again, and hunts when the draw is an own post
    // (MEASURED: BF2 POL1 seed 3 stood 39,227 ticks at 1 v 1 without it).
    let endgame = sim
        .battle
        .teams
        .iter()
        .any(|t| t.tickets() <= crate::ai::HOLD_ENDGAME);

    // --- card DEFS: each own post's cap of defenders, and who is over it ---
    let radius = threat_radius(sim);
    let alive = sim.alive_units().filter(|(_, u)| u.team == team).count();
    let share = defend_share(sim.aggressiveness(team));
    let mut threat: Vec<u64> = Vec::with_capacity(posts.len());
    let mut cap: Vec<usize> = Vec::with_capacity(posts.len());
    for (ci, p) in posts.iter().enumerate() {
        let taken = p.friendly && old.owners.get(ci).is_some_and(|o| *o != Some(team));
        let last = if taken || (p.friendly && post_threatened(sim, team, ci as u32, radius)) {
            sim.tick
        } else {
            old.threat.get(ci).copied().unwrap_or(0)
        };
        threat.push(last);
        cap.push(defend_cap(alive, sim.tick.saturating_sub(last), share));
    }
    // The defenders a post keeps are the nearest to it (ties: the lower
    // slot); `held` counts them, and a draw that lands on a full post is
    // made again without it.
    let mut over: Vec<bool> = vec![false; sim.units.len()];
    let mut held: Vec<usize> = vec![0; posts.len()];
    for (ci, p) in posts.iter().enumerate() {
        if !p.friendly {
            continue;
        }
        let at = sim.cps[ci].def.pos;
        let mut holders: Vec<(f32, usize)> = units
            .iter()
            .filter(|&&(i, gen, _, _)| {
                old.gens.get(i).is_none_or(|g| *g == gen)
                    && old.of(i as u32).and_then(Objective::cp) == Some(ci as u32)
            })
            .map(|&(i, _, pos, _)| (crate::nav::xz_dist(pos, at), i))
            .collect();
        holders.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for &(_, i) in holders.iter().skip(cap[ci]) {
            over[i] = true;
        }
        held[ci] = holders.len().min(cap[ci]);
    }

    let mut assignment: Vec<Option<Objective>> = vec![None; sim.units.len()];
    let mut gens: Vec<u32> = vec![0; sim.units.len()];
    let mut at_post: Vec<u8> = vec![0; sim.units.len()];
    let mut changes = 0u64;
    // Hoisted: refilled per draw instead of allocated per draw.
    let mut offered: Vec<DrawPost> = Vec::with_capacity(posts.len());
    for &(i, gen, pos, hub) in &units {
        let (fighting, dwell) = {
            let u = &sim.units[i];
            (u.ai.target.is_some(), u.ai.unreachable_dwell)
        };
        // Card MOVR: the post this unit's walk gave up (`behave::recover`),
        // read once.
        let gave_up = std::mem::take(&mut sim.units[i].ai.mv.give_up).checked_sub(1);
        gens[i] = gen;
        let kept = old
            .of(i as u32)
            .filter(|_| old.gens.get(i).is_none_or(|g| *g == gen));

        // --- does it draw? ---
        let target = kept
            .and_then(Objective::cp)
            .filter(|&c| (c as usize) < posts.len());
        let mut clock = 0u8;
        let mut exclude: Option<u32> = None;
        let draw = match target {
            None => true,
            Some(c) => {
                let p = &posts[c as usize];
                // The at-post helper's 5 s test, once per stay.
                let inside = sim.cps[c as usize].def.capture.contains(pos) && !fighting;
                clock = match old.at_post.get(i).copied().unwrap_or(0) {
                    _ if !inside => 0,
                    AT_POST_TESTED => AT_POST_TESTED,
                    n => n + 1,
                };
                let tested = clock != AT_POST_TESTED && clock >= AT_POST_PLANS;
                if tested {
                    clock = AT_POST_TESTED;
                }
                if dwell >= DWELL_GIVE_UP || gave_up == Some(c) {
                    // It has stood beside the post for 10 s without arriving,
                    // or its walk there has failed twice (card MOVR).
                    exclude = Some(c);
                    true
                } else if !reach(hub, p) || (endgame && p.friendly) {
                    true
                } else if p.friendly && over[i] {
                    // Card DEFS: over the post's cap of defenders.
                    exclude = Some(c);
                    true
                } else if tested && p.friendly {
                    if rules == GoalRules::Bf1 {
                        exclude = Some(c);
                    }
                    true
                } else {
                    match rules {
                        GoalRules::Bf2 => p.friendly && p.def == 0,
                        // The owner-change event: a target that became the
                        // team's, or an own one that stopped being it.
                        GoalRules::Bf1 => p.friendly != matches!(kept, Some(Objective::Defend(_))),
                    }
                }
            }
        };

        let chosen = if draw {
            clock = 0;
            offered.clear();
            offered.extend(posts.iter().enumerate().map(|(ci, p)| DrawPost {
                friendly: p.friendly,
                neutral: p.neutral,
                atk: p.atk,
                def: p.def,
                // Card MAIS (OURS): a post its bots may not capture
                // (`AICanCaptureCP`) is not drawn to take; held, it is
                // still defended.
                open: reach(hub, p) && exclude != Some(ci as u32) && (p.friendly || p.capturable),
            }));
            let r = roll(i as u32, sim.tick, gen);
            let mut got = None;
            // Nothing this unit can route to is left to take: it hunts (the
            // port's endgame). Rare since card HUBM put every post and unit
            // on a hub its mover has an arc at (TAT1: 42.5 -> 0.6 % of on-foot
            // unit-seconds hunting); kept for a unit on a graph that joins no
            // post left to take.
            if posts.iter().any(|p| !p.friendly && reach(hub, p)) {
                got = draw_post(rules, team, &offered, r);
                // Card DEFS: an own post that has its defenders is not drawn.
                let full =
                    |c: u32| posts[c as usize].friendly && held[c as usize] >= cap[c as usize];
                if !endgame && got.is_some_and(full) {
                    for (ci, o) in offered.iter_mut().enumerate() {
                        o.open &= !(posts[ci].friendly && held[ci] >= cap[ci]);
                    }
                    got = draw_post(rules, team, &offered, r).filter(|&c| !full(c));
                }
                if let Some(c) = got.filter(|&c| posts[c as usize].friendly) {
                    held[c as usize] += 1;
                }
            }
            match got {
                Some(c) if posts[c as usize].friendly && endgame => Objective::Hunt,
                Some(c) if posts[c as usize].friendly => Objective::Defend(c),
                Some(c) => Objective::Attack(c),
                None => Objective::Hunt,
            }
        } else {
            // Kept, and named by who owns it now.
            match target {
                Some(c) if posts[c as usize].friendly => Objective::Defend(c),
                Some(c) => Objective::Attack(c),
                None => Objective::Hunt,
            }
        };
        if kept.map(Objective::cp) != Some(chosen.cp()) {
            changes += 1;
            // A new target starts a fresh dwell window, so giving up on one
            // does not immediately give up on the next.
            sim.units[i].ai.unreachable_dwell = 0;
        }
        assignment[i] = Some(chosen);
        at_post[i] = clock;
    }

    // --- a passenger takes its AI driver's target (BF2 `FUN_002e80b0`) ---
    for &(i, _, _, _) in &units {
        let Some(m) = sim.units[i].mount else {
            continue;
        };
        let driver = sim
            .vehicle(m.vehicle)
            .and_then(|v| v.pilot_seat())
            .and_then(|s| s.occupant)
            .filter(|d| d.index as usize != i)
            .and_then(|d| assignment.get(d.index as usize).copied().flatten())
            .filter(|o| o.cp().is_some());
        if let Some(o) = driver {
            if assignment[i].map(Objective::cp) != Some(o.cp()) {
                assignment[i] = Some(o);
                at_post[i] = 0;
            }
        }
    }

    // --- BF1's retake call: a post the team lost to neutral ---
    if rules == GoalRules::Bf1 {
        for (ci, p) in posts.iter().enumerate() {
            if !p.neutral || old.owners.get(ci).copied().flatten() != Some(team) {
                continue;
            }
            let at = sim.cps[ci].def.pos;
            let mut sent = 0;
            for &(i, _, pos, hub) in &units {
                if sent >= RETAKE_UNITS {
                    break;
                }
                let flying = sim.units[i]
                    .mount
                    .and_then(|m| sim.vehicle(m.vehicle))
                    .is_some_and(|v| v.kind == crate::vehicle::VehicleKind::Flyer);
                if flying || (pos - at).length() > RETAKE_RADIUS || !reach(hub, p) {
                    continue;
                }
                sent += 1;
                if assignment[i].and_then(Objective::cp) != Some(ci as u32) {
                    changes += 1;
                    assignment[i] = Some(Objective::Attack(ci as u32));
                    at_post[i] = 0;
                    sim.units[i].ai.unreachable_dwell = 0;
                }
            }
        }
    }
    sim.nav_stats.assignment_changes += changes;
    for a in &assignment {
        if let Some(Objective::Attack(c)) = a {
            if *c < 64 {
                sim.nav_stats.attack_used[t] |= 1u64 << c;
            }
        }
    }

    // --- squads: same objective, within SQUAD_RADIUS of another such unit ---
    for &(i, _, pos, _) in &units {
        let Some(o) = assignment[i] else {
            sim.units[i].ai.squad = None;
            continue;
        };
        let together = units.iter().any(|&(j, _, p, _)| {
            j != i && assignment[j] == Some(o) && (p - pos).length() <= SQUAD_RADIUS
        });
        sim.units[i].ai.squad = together.then(|| o.key());
    }
    for (u, a) in sim.units.iter_mut().zip(assignment.iter()) {
        if u.team == team && a.is_none() {
            u.ai.squad = None;
        }
    }

    // --- card DEFS: one spot a defender, kept while it defends the post ---
    let mut spots: Vec<Option<(Vec3, u8)>> = vec![None; sim.units.len()];
    let defends = |i: usize| match assignment[i] {
        Some(Objective::Defend(c)) if sim.units[i].mount.is_none() => Some(c),
        _ => None,
    };
    let mut taken: Vec<Vec3> = Vec::new();
    for &(i, gen, _, _) in &units {
        let kept = old.gens.get(i).is_none_or(|g| *g == gen)
            && defends(i).is_some()
            && old.of(i as u32) == assignment[i];
        if let Some(s) = old.spot(i as u32).filter(|_| kept) {
            spots[i] = Some(s);
            taken.push(s.0);
        }
    }
    for &(i, _, pos, _) in &units {
        let Some(c) = defends(i).filter(|_| spots[i].is_none()) else {
            continue;
        };
        spots[i] = pick_spot(sim, c, i, pos, &taken);
        taken.extend(spots[i].map(|s| s.0));
    }

    let owners = sim.cps.iter().map(|c| c.owner).collect();
    sim.commander[t] = TeamPlan {
        assignment,
        gens,
        at_post,
        owners,
        spots,
        threat,
        next_plan_tick: old.next_plan_tick,
    };
}

/// Card DEFS, rules 2-4: how many defenders a post keeps, of a team with
/// `alive` living units, `quiet` ticks after its last threat or capture, at
/// `share` of the team ([`defend_share`]).
pub fn defend_cap(alive: usize, quiet: u64, share: f32) -> usize {
    if quiet >= DEFEND_QUIET_LEAVE {
        0
    } else if quiet >= DEFEND_QUIET_FALL {
        DEFEND_QUIET_CAP
    } else {
        ((share * alive as f32).round() as usize).max(1)
    }
}

/// Card MAIS (OURS, a named deviation): the share of a team that defends one
/// threatened post at the mission's `SetTeamAggressiveness` `aggr` -
/// [`DEFEND_SHARE`] over it, at most all of the team. The PS2 stores the
/// value and no reader of it is known in either game (re-ai-difficulty 3,
/// open question 5); a less aggressive side (BF1 and BF2 Endor's Imperials,
/// 0.7) holds what it has with more of its men. 1.0 (the default) is
/// [`DEFEND_SHARE`].
pub fn defend_share(aggr: f32) -> f32 {
    (DEFEND_SHARE / aggr.max(DEFEND_SHARE)).min(1.0)
}

/// Card DEFS, rule 3: the distance inside which an enemy threatens a post -
/// [`THREAT_FRAC`] of the distance between the map's two farthest posts, at
/// least [`THREAT_RADIUS_MIN`].
pub fn threat_radius(sim: &Sim) -> f32 {
    let mut far = 0.0f32;
    for (i, a) in sim.cps.iter().enumerate() {
        for b in &sim.cps[i + 1..] {
            far = far.max(crate::nav::xz_dist(a.def.pos, b.def.pos));
        }
    }
    (THREAT_FRAC * far).max(THREAT_RADIUS_MIN)
}

/// Card DEFS, rule 3 (and card TURR's defence clause): is post `cp` under
/// threat for `team` - a living enemy within `radius` of it, or a living unit
/// of the team within `radius` of it with a target? Like the hunt, the first
/// is not bounded by what anybody sees.
pub fn post_threatened(sim: &Sim, team: u8, cp: u32, radius: f32) -> bool {
    let Some(post) = sim.cps.get(cp as usize) else {
        return false;
    };
    let at = post.def.pos;
    sim.alive_units().any(|(_, u)| {
        (u.soldier.pos - at).length_squared() <= radius * radius
            && (u.team != team || u.ai.target.is_some())
    })
}

/// Card DEFS, rule 1: the spot unit slot `i`, at `from`, defends post `cp`
/// from - the nearest authored Cover / Snipe hint of the post
/// ([`nearest_hint`]) that is not within [`DEFEND_SPACING`] of a `taken`
/// spot. With no hint left it has none: its walk ends at its own drawn point
/// of the capture region, as an attacker's does (card PARR's point: on the
/// post's floor, one a soldier can stand on), where no stance is asked and
/// the PS2's 5 s test sends it on.
fn pick_spot(sim: &Sim, cp: u32, i: usize, from: Vec3, taken: &[Vec3]) -> Option<(Vec3, u8)> {
    let post = sim.cps.get(cp as usize)?;
    let free = |p: Vec3| {
        taken
            .iter()
            .all(|t| crate::nav::xz_dist(*t, p) >= DEFEND_SPACING)
    };
    let scoped = sim.weapon_classes[sim.units[i].weapon_class].ai_distance_table();
    nearest_hint(sim, post.def.pos, HINT_RADIUS, from, scoped, &free)
        .map(|h| (h.pos, h.stance_mask))
}

/// Allowed-stance mask bit: the unit may STAND at this hint.
pub const MASK_STAND: u8 = 1 << 0;
/// Allowed-stance mask bit: it may CROUCH.
pub const MASK_CROUCH: u8 = 1 << 1;
/// Allowed-stance mask bit: it may go PRONE (BF1 only — see [`stance_mask`]).
pub const MASK_PRONE: u8 = 1 << 2;

/// The ALLOWED-STANCE MASK a hint's authored `PrimaryStance` asks for:
/// [`MASK_STAND`] | [`MASK_CROUCH`] | [`MASK_PRONE`].
///
/// `PrimaryStance` is a BITMASK, not an enum (RE lead-verified 2026-09-08
/// evening: `analysis/RE_LOG.md`, `engine/artifacts/lead_tmp/s15/re/primary_stance.md`
/// §1-5 and its answer table). The PS2 draws ONE of the set bits uniformly when
/// the unit arrives at the hint — BF2 `FUN_002deed0` / BF1 `FUN_0023f600`:
/// `do r = rand(0,2); while (((mask >> r) & 1) == 0)` — and HOLDS it, so
/// `PrimaryStance 3` is "stand or crouch", NOT prone. What reaches that picker
/// differs per game, which is why this takes `source_game` the way
/// [`crate::sim::omni_radius_for`] does:
///
/// - **BF2** masks the authored value `& 3` in the setter (`FUN_002defe0`
///   267212-267219) and stores nothing when the result is 0, leaving the hint
///   ctor's default of stand (`FUN_002deb88` 266934, `| 0x500`). BF2 AI
///   therefore never prones, three times over: the setter drops bit 2, the
///   Cover tick folds a prone pick back to stand (257788-257790), and the
///   soldier's go-prone slot is a stub (`FUN_00246ec8`).
/// - **BF1** stores the raw byte whenever it is non-zero (`FUN_0023f648`
///   157262-157265), so 4-7 are the only values that can produce prone, and
///   only on BF1.
///
/// A hint that authors NO `PrimaryStance` asks for stand on both games (the
/// ctor default), so the answer is never "no wish" — a hint always wants
/// something. PORT GUARD, not from the disc: a value whose low three bits are
/// all zero (8, 16, ...) would spin the PS2's rejection sampler forever; no
/// disc authors one (the phase-C census has 1-7 only) and this answers stand.
pub fn stance_mask(primary_stance: Option<u8>, source_game: &str) -> u8 {
    let bits = match primary_stance {
        Some(v) if source_game.eq_ignore_ascii_case("bf1") => v & 7,
        Some(v) => v & 3,
        None => 0,
    };
    if bits == 0 {
        MASK_STAND
    } else {
        bits
    }
}

/// The allowed PEEK SIDES a hint's authored `SecondaryStance` carries: bit 0 =
/// the hint's −right, bit 1 = its +right (the authored bits 3 and 4 — BF2
/// stores them at bits 12-13 of the hint word, `(v & 0x18) << 9`
/// `FUN_002defe0` 267199-267208; BF1 at +0x7e as `(v & 0x18) >> 3`).
///
/// CARRIED ONLY: the Cover peek state that consumes it is Phase D work and
/// nothing reads this yet. `SecondaryStance`'s low bits are a stance mask like
/// the primary one ([`stance_mask`] reads them the same way).
pub fn peek_sides(secondary_stance: Option<u8>) -> u8 {
    (secondary_stance.unwrap_or(0) >> 3) & 3
}

/// The `Stance` a stance CODE means; `None` for the 0 sentinel ("no wish") and
/// for anything a mask cannot produce. The code is the mask bit's INDEX plus
/// one, so 1/2/3 are always stand/crouch/prone whatever the authored value was.
pub fn stance_of_code(code: u8) -> Option<Stance> {
    match code {
        1 => Some(Stance::Stand),
        2 => Some(Stance::Crouch),
        3 => Some(Stance::Prone),
        _ => None,
    }
}

/// The mask bit a stance CODE occupies, and 0 for the sentinel: the bridge
/// between `AiState::want_stance` (a code) and a hint's mask.
pub fn stance_bit(code: u8) -> u8 {
    match code {
        1..=3 => 1 << (code - 1),
        _ => 0,
    }
}

/// The inverse of [`stance_of_code`]: the code that asks for `s`, and 0 for
/// a stance no `PrimaryStance` can author. `AiState::want_stance` stores the
/// CODE rather than the `Stance` because 0 there has to mean "no wish", and
/// `Stance::Stand`'s own discriminant is 0.
pub fn stance_code(s: Stance) -> u8 {
    match s {
        Stance::Stand => 1,
        Stance::Crouch => 2,
        Stance::Prone => 3,
        _ => 0,
    }
}

/// The nearest authored Cover or Snipe hint within [`HINT_RADIUS`] of
/// `centre`, measured from `from`.
///
/// `scoped` is the unit's own "my weapon has a scope"
/// ([`crate::weapon::WeaponClass::ai_distance_table`], D9's predicate and the
/// only scope test the PS2 AI has): a unit without one is not offered a Snipe
/// hint at all, so the marksman posts stay for the marksmen (brief D13).
fn nearest_hint<'a>(
    sim: &'a Sim,
    centre: Vec3,
    radius: f32,
    from: Vec3,
    scoped: bool,
    free: &dyn Fn(Vec3) -> bool,
) -> Option<&'a crate::nav::Hint> {
    let g = sim.nav.as_ref()?;
    let mut best: Option<(f32, &crate::nav::Hint)> = None;
    for h in &g.hints {
        let usable = match h.kind {
            HINT_COVER => true,
            HINT_SNIPE => scoped,
            _ => false,
        };
        if !usable {
            continue;
        }
        if (h.pos - centre).length() > radius || !free(h.pos) {
            continue;
        }
        if HINT_SAME_LEVEL && !hint_on_level(sim, centre.y, h.pos) {
            continue;
        }
        let d = (h.pos - from).length();
        // Strictly less: ties keep the earlier hint (hints are in authored
        // order), so no float equality decides anything.
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, h));
        }
    }
    best.map(|(_, h)| h)
}

/// Does the hint at `hint` stand on the level of a centre at height
/// `centre_y`? The floor the hint stands on (`ground_under` from the hint
/// itself) must be the floor found under the hint's XZ from the centre's
/// height (the [`crate::ai::route`] `hub_has_floor` idiom), within
/// [`STEP_HEIGHT`].
///
/// A hint on a ledge or walkway ABOVE the centre fails — the probe from the
/// centre's height finds the floor under it — and a unit sent at it walks a
/// straight last leg into the wall (queue STK: POL1's `hintnode153`, 8.5 m
/// above cp6con, three defenders stuck in a crate stack all round). A hint
/// BELOW the centre with no floor over it passes when a bot can drop to it —
/// no deeper than the AI's own ledge rule ([`crate::ai::LEDGE_DROP`], card
/// STLVL: BF2 DEA1's cp2 at 52 m has a hint 6 m under its gallery, and its
/// defenders stood on the rim over it until they were recycled).
fn hint_on_level(sim: &Sim, centre_y: f32, hint: Vec3) -> bool {
    let own = sim.ground_under(hint, false);
    let from_centre = sim.ground_under(Vec3::new(hint.x, centre_y, hint.z), false);
    (own - from_centre).abs() <= STEP_HEIGHT && own >= centre_y - crate::ai::LEDGE_DROP
}

/// Where a LONE unit defending `cp` would stand, and in what stance (since
/// card DEFS a defender's own spot is the plan's, [`TeamPlan::spot`], drawn
/// by [`pick_spot`] with every other defender's spot taken; this is the same
/// hint rule with nothing taken): the nearest authored Cover or Snipe
/// hint within [`HINT_RADIUS`] of the POST, or `None` when no hint qualifies.
/// The capture-region fallback belongs to [`pick_spot`].
///
/// Brief D13: the hint's ALLOWED-STANCE MASK comes back with it (never 0 — a
/// hint with no authored `PrimaryStance` asks for stand; see [`stance_mask`]),
/// and the caller draws one of its bits ([`crate::ai::stance_hold`]). A Snipe
/// hint is only offered to a `scoped` unit. `Hint::command_post` — the disc's
/// own "this hint belongs to that post" — is still unread: the design says a
/// radius, and a radius is what this uses.
pub fn defend_spot(sim: &Sim, cp: u32, from: Vec3, scoped: bool) -> Option<(Vec3, u8)> {
    let post = sim.cps.get(cp as usize)?;
    hint_spot(sim, post.def.pos, HINT_RADIUS, from, scoped)
}

/// [`defend_spot`]'s body, over ANY centre and radius (brief D8): the nearest
/// authored Cover or Snipe hint within `radius` of `centre`, measured from
/// `from`, with its allowed-stance mask.
///
/// Defend asks it about a POST at [`HINT_RADIUS`]; a squad `Hold` asks it about
/// the bot's ANCHOR at 10 m (the earlier port's hold order), and a holder that finds
/// one stands there in the stance the hint asks for, exactly as a defender
/// does. One body, so the two cannot drift — and so the sweep, which only ever
/// walks the Defend path, is the referee for both.
pub fn hint_spot(
    sim: &Sim,
    centre: Vec3,
    radius: f32,
    from: Vec3,
    scoped: bool,
) -> Option<(Vec3, u8)> {
    let h = nearest_hint(sim, centre, radius, from, scoped, &|_| true)?;
    Some((h.pos, h.stance_mask))
}

/// The allowed-stance mask an IDLE unit takes from where it stands (brief
/// D13): the same hint rule as [`defend_spot`], centred on the unit itself
/// rather than on a post, and **0 when no hint is in range** (no wish). It
/// answers a mask only — an idle unit is not sent walking to a hint, it uses
/// the one it is already standing in.
pub fn hint_stance(sim: &Sim, from: Vec3, scoped: bool) -> u8 {
    nearest_hint(sim, from, HINT_RADIUS, from, scoped, &|_| true).map_or(0, |h| h.stance_mask)
}

#[cfg(test)]
mod tests;
