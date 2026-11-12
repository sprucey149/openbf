//! The playable level of a map, from data the disc already carries (cards
//! KFLR and BNDS; the maintainer, 2026-10-01): the floor a soldier walks at each hub of
//! the planning graph ([`Sim::resolve_nav_floors`]), whether a point is
//! joined to that level or off it ([`Sim::level_at`]), the map's kill floor
//! ([`Sim::kill_floor`]) and the landing under the map
//! ([`Sim::lands_under_the_map`]). The tests live here.
//!
//! ## Why the floors are resolved and not read
//! A hub's authored `y` is the editor's pick height, not a floor (RE_LOG):
//! BF2 Kamino authors all 143 hubs at y 69.66 over decks at 51–78, BF2 Polis
//! Massa all 189 at 0. So the floor is WALKED: from every command post and
//! spawn node (whose heights are real) along the straight line to the hub it
//! stands in, then hub to hub along the arcs, following the ground one short
//! step at a time ([`TRACE_STEP`]) — up a kerb, down a ledge, over a slit at
//! the height it left; a line with more than [`TRACE_GAP`] of open air is no
//! walk, unless the arc is a JUMP. Breadth-first from all the posts at once;
//! the first front to reach a hub sets it. A hub no arc reaches from a post stays
//! unresolved and nothing here judges a unit against it. Then every hub
//! hears all its resolved neighbours — the first front may have walked UNDER
//! a walkway's hub — and the floor most of them walk to wins. Over the 32
//! maps 3,018 of 3,086 hubs resolve. The same walk says which arcs no
//! soldier can walk ([`Sim::arc_unwalkable`]: 57 arc directions over the 32
//! maps), and card TGAP's wall test adds the arcs a soldier walking round
//! the walls cannot get through both ways (15 more; BF1 Tatooine's footpath
//! 78 - 77 meets a leaning slab, shut on the PS2 too). Over all of those, the
//! hubs a soldier would be TRAPPED in ([`Sim::hub_trapped`], 17 on 5 maps):
//! the router sends him into none of them.
//!
//! Card STLVL, two rules of that walk. A post or spawn node PINS only the
//! hub whose disc it stands in; one that merely has the hub as its nearest
//! starts the walk there and the neighbours may out-vote it (BF2 Mygeeto's
//! cp1 is on the ground 12 m from hub 32, a balcony 4 m up; BF2 Dagobah's
//! cave mouth hub 16 took a surface spawn node's height). And a seam between
//! two models is not a drop: where the walk's one ray falls more than a step,
//! the foot's ring is asked ([`CRACK_TOL`]) — Mygeeto's pier meets that
//! balcony at a crack, and the walk fell through it to the ground.
//! [`Sim::floor_reaches_hub`] is the same walk from a unit's feet: is it on
//! that hub's storey (`ai::route::level_hub`)?
//!
//! ## What "on the level" means
//! The disc bounds its walkable areas for the AI with `BARR` barriers — BF2
//! Kamino rings every platform's rim rail with them and lines every bridge
//! (759 on the map) — and the PS2's walk step never crosses one (RE_LOG
//! "Barriers are the PS2's Obstacles"). So the level at a point is the floor
//! a walk FROM a near hub's centre TOWARD the point reaches, stopping at the
//! first barrier: on a deck, a ramp or a room it arrives at the point's own
//! height; past a rim it stops at the rail, on the deck. A point more than
//! [`BOUND_DROP`] under such a floor — and level with none that a walk
//! brings within [`LEVEL_SLACK`] of it — is OFF the level: down a hull,
//! under a bridge, under a deck. The terrain is always on it, and so is a
//! trench (card LVLF): a floor in a hole the map's own models cut into the
//! terrain, no more than [`TRENCH_DEPTH`] under the heightfield over it and
//! under the open sky. Both Hoths' trench floors lie 2.0–2.5 m under the
//! snow between barriers, 1–45 cm more than [`BOUND_DROP`] under where the
//! walk from a hub stops: 1,085 of BF1 Hoth's 4,067 trench cells read OFF,
//! its bots spent 18 % of their time "lost" in them and 33 died of it in 8
//! rounds. A roofed floor in a cut is no trench: the slab 2.6 m under BF2
//! Polis Massa's cut terrain, 5 m under its rooms, stays off.
//!
//! A walk a barrier STOPPED short of the point vouches for less (card BOWL;
//! The maintainer, 2026-10-01: "figure out why the bots are going on this bowl"): only
//! for a point within [`RIM_DROP`] under where it stopped. With the full
//! [`BOUND_DROP`] the first 4.5 m of a BF2 Kamino hull past the rim rail
//! read ON, so the steering's edge test took the rail for no wall, a bot out
//! there was never lost, and bots spent 13–18 % of their time on the hulls
//! (`artifacts/lead_tmp/bowl/README.md`). A hole the map's own models CUT
//! into the terrain keeps the full allowance: BF2 Hoth's trenches lie 2 m
//! under the snow between barriers, and with the short one the bots that
//! defend from them died as lost. [`Sim::rim_drop`] is the census
//! that counts them (`round_to_outcome`'s `rim` share).
//!
//! ## What the PS2 does (`analysis/re-kill-floor.md`, 2026-10-01, `[O]`)
//! - A death region kills at once and does NOT ask the invincibility cheat
//!   (BF1 `FUN_002af0d0`, BF2 `FUN_00370fd8`: the kill's third argument is 0).
//! - The soldier's own tick kills below a hard-coded plane — BF1 y < −500,
//!   BF2 y < −1000 (`FUN_00205cc8` B1:116586, `FUN_00240bf8` B2:159373) — also
//!   past the cheat. That plane is what kills on BF1 Bespin Platforms, which
//!   has no death region and no terrain.
//! - Nothing kills a soldier who stands on Kamino's hidden collision under
//!   the decks; a landing only costs (v − 13)² health.
//!
//! OURS (PROVISIONAL), all three: the kill floor sits [`KILL_FLOOR_DROP`]
//! under the lowest floor the map's own graph and posts stand on instead of
//! 500 m under the origin; a hard landing under the map kills (the maintainer: "hitting
//! the hidden ground under the map kills you, with or without invincibility");
//! and the level test itself.

use super::state::Sim;
use crate::collision::{Layers, NO_GROUND, STEP_HEIGHT};
use crate::nav::{Mover, SOLDIER};
use glam::{Vec2, Vec3};
use std::collections::VecDeque;

/// Card TGAP's wall test walks an arc no farther than this off its line (m).
/// PROVISIONAL (ours).
pub const WALL_BAND: f32 = 8.0;
/// ...and gives up (judges nothing) after this many cells. PROVISIONAL.
pub const WALL_CELLS: usize = 3000;
/// ...and judges no arc longer than this (m): over a long arc a soldier's
/// way round leaves the band (both Hoths' 140 - 190 m arcs, walked by the
/// port's bots 1 - 65 times in 3 rounds each). PROVISIONAL (ours).
pub const WALL_SPAN: f32 = 30.0;
/// One step of the floor walk between two hubs (m). PROVISIONAL (ours).
pub const TRACE_STEP: f32 = 0.75;
/// The walk climbs at most this much in one step (m): a 58° ramp.
pub const TRACE_UP: f32 = 1.2;
/// ...and follows the floor down at most this much (m) — the AI's own ledge
/// rule ([`crate::ai::LEDGE_DROP`]). A deeper drop is a gap: the walk goes on
/// at the height it left.
pub const TRACE_DROP: f32 = crate::ai::LEDGE_DROP;
/// A seam between two models lets the walk's one ray through; the foot's
/// ring finds the floor on both sides within this of the walk's height (m).
/// PROVISIONAL (ours, card STLVL): BF2 Mygeeto's pier meets its balcony at a
/// crack over a 4 m drop.
pub const CRACK_TOL: f32 = 0.3;
/// A walk with more than this much open air in a row is not a walk (m): an
/// arc drawn across a gap (BF2 Kamino's 123 ↔ 121, 20 m of sea) resolved its
/// far hub onto the roof the line met on the other side.
pub const TRACE_GAP: f32 = 3.0;
/// A point is on the level when the floor walked to it from a hub arrives no
/// more than this over it (m). PROVISIONAL (ours): over a kerb, a rail and
/// a trench's depth, under the 4.7 m to BF2 Kamino's first stranding spots.
/// Past a barrier only [`RIM_DROP`] is allowed.
pub const BOUND_DROP: f32 = 2.0;
/// A floor in a hole cut into the terrain, under the open sky, is a trench
/// and on the level down to this far under the heightfield over it (m).
/// PROVISIONAL (ours, card LVLF): both Hoths' trench floors lie 2.0–2.5 m
/// under the snow; a hangar's or a cave's floor is roofed and judged by its
/// hubs.
pub const TRENCH_DEPTH: f32 = 3.0;
/// ...the open sky: nothing over the head within this (m). PROVISIONAL.
pub const TRENCH_SKY: f32 = 60.0;
/// A walk that a barrier stopped more than [`RIM_GAP`] short of the point
/// vouches only for a point no more than this under its stop (m).
/// PROVISIONAL (ours, card BOWL): a kerb or a step, not a hull — BF2 Kamino's
/// hulls fall 0.67 m per metre past the rail.
pub const RIM_DROP: f32 = 0.75;
/// ...and a walk that stopped within this of the point (m) reached it.
pub const RIM_GAP: f32 = 0.3;
/// [`Sim::rim_drop`] asks this many nearest hubs. PROVISIONAL (ours).
pub const RIM_HUBS: usize = 3;
/// The hubs a point is judged against: this many, nearest first by the XZ
/// distance to the hub's disc. PROVISIONAL (ours).
pub const LEVEL_HUBS: usize = 6;
/// ...a metre of height between a hub's floor and the point counting as this
/// many metres of distance. PROVISIONAL (ours): BF2 Polis Massa's lower
/// corridors (floor 13.1) run under rooms at 21.1 whose hubs are nearer in XZ.
pub const LEVEL_RISE: f32 = 3.0;
/// A walk that stops at a barrier still vouches for a point this close to
/// where it stopped (m). PROVISIONAL (ours): a room's width; under the 14 m
/// from a BF2 Kamino rim to the foot of its hull, where a NEIGHBOURING
/// platform's deck is level with the feet.
pub const LEVEL_SLACK: f32 = 10.0;
/// A unit coming back to the level cycles through this many nearest hubs.
pub const RETURN_HUBS: usize = 4;
/// Card RETW: a lost bot's floor walk back ([`Sim::return_path`]) floods a
/// grid of this cell (m). PROVISIONAL (ours): card LVLF's classifier's.
pub const RETURN_CELL: f32 = 0.75;
/// ...steps down at most this much from one cell to the next (m): a
/// platform's kerb, the 1.5 m from BF2 Kamino's pod pad to its deck.
/// PROVISIONAL (ours).
pub const RETURN_STEP_DOWN: f32 = BOUND_DROP;
/// ...no farther than this from the feet (m), over at most this many cells.
/// PROVISIONAL (ours): BF2 Utapau's way out from under the landing walks
/// 37-59 m (card LVLF).
pub const RETURN_RANGE: f32 = 60.0;
pub const RETURN_CELLS: usize = 12_000;
/// ...to a cell this close to a hub's centre (or its own radius, when
/// larger) and this close to its floor (m). PROVISIONAL (ours).
pub const RETURN_HUB_RADIUS: f32 = 1.5;
pub const RETURN_HUB_TOL: f32 = 1.0;
/// ...with no wall this far over the floor between two cells (m): over a
/// step, under a soldier's head.
pub const RETURN_CHEST: f32 = 1.4;
/// ...and pulls the walk straight past a wall no nearer than the soldier's
/// radius less this (m). PROVISIONAL (ours).
pub const RETURN_SLACK: f32 = 0.1;
/// ...and keeps at most this many points of the walk (the hub's last).
pub const RETURN_PATH: usize = 12;

/// Card RETW: the walk [`Sim::return_path`] finds, in walking order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReturnPath {
    pub pts: [Vec3; RETURN_PATH],
    pub len: u8,
}

/// The kill floor sits this far under the lowest resolved floor (m).
/// PROVISIONAL (ours); the PS2's plane is at −500 / −1000.
pub const KILL_FLOOR_DROP: f32 = 15.0;
/// A landing at or beyond this speed (m/s) can be a landing under the map.
/// The PS2's own landing-damage threshold on flat ground (10 + 3·up·n,
/// re-soldier-ground.md §4): a 4.7 m drop at g = 18.
pub const UNDER_MAP_SPEED: f32 = 13.0;
/// Two walks agree on a hub's floor within this (m), and the neighbours are
/// heard this many times over.
const VOTE_TOL: f32 = 1.0;
const VOTE_ROUNDS: usize = 3;
/// A soldier's head over its feet for the cover ray (m).
const HEAD: f32 = 1.9;

/// Where a point stands against the playable level ([`Sim::level_at`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    /// No resolved floor to judge by (no graph, or none of it resolved).
    Unknown,
    /// Joined to the graph: on the terrain, or walkable from a hub.
    On,
    /// Off it: under the floor a near hub walks toward it, by this much (m).
    Off(f32),
}

impl Level {
    pub fn is_off(self) -> bool {
        matches!(self, Level::Off(_))
    }
}

impl Sim {
    /// Resolve [`Sim::nav_floors`] and [`Sim::kill_floor`] from the graph, the
    /// collision world and the posts registered so far. [`Sim::add_cp`] calls
    /// it after every post, so a battle built the usual way has them; a sim
    /// with no graph or no post has none, and every rule here is then off.
    pub fn resolve_nav_floors(&mut self) {
        self.nav_floors.clear();
        self.nav_unwalkable.clear();
        self.nav_trapped.clear();
        self.kill_floor = None;
        let Some(g) = self.nav.as_ref() else {
            return;
        };
        let n = g.hubs.len();
        let mut floors = vec![f32::NAN; n];
        // Hubs a post or a spawn node anchors: the walk's own votes below
        // never move them.
        let mut anchored = vec![false; n];
        let mut queue: VecDeque<u16> = VecDeque::new();
        let mut lowest = f32::INFINITY;
        // The anchors: every post, then its spawn nodes, in post order. An
        // anchor INSIDE its hub's disc pins the hub; one that only has the
        // hub as its nearest (second pass) starts the walk there and no
        // more — the neighbours' votes may move it (card STLVL: BF2
        // Mygeeto's cp1 stands on the ground 12 m from hub 32, a balcony
        // 4 m up, and pinned it to the ground under it).
        for inside in [true, false] {
            for cp in &self.cps {
                let anchors =
                    std::iter::once(cp.def.pos).chain(cp.def.spawn_nodes.iter().map(|&(p, _)| p));
                for p in anchors {
                    let y = self.ground_under(p, false);
                    if y <= NO_GROUND {
                        continue;
                    }
                    lowest = lowest.min(y);
                    let Some(h) = g.unit_hub_on(p, Mover::new(SOLDIER, true), &|_| true) else {
                        continue;
                    };
                    let Some(hub) = g.hub(h) else { continue };
                    let to = Vec2::new(hub.pos.x, hub.pos.z);
                    let within = (to - Vec2::new(p.x, p.z)).length() <= hub.radius;
                    if within != inside || !floors[usize::from(h)].is_nan() {
                        continue;
                    }
                    if let Some(f) = self.trace_floor(Vec3::new(p.x, y, p.z), to, false) {
                        floors[usize::from(h)] = f;
                        anchored[usize::from(h)] = inside;
                        queue.push_back(h);
                    }
                }
            }
        }
        // Hub to hub along the arcs a soldier may walk, breadth-first.
        while let Some(a) = queue.pop_front() {
            let Some(ha) = g.hub(a) else { continue };
            let from = Vec3::new(ha.pos.x, floors[usize::from(a)], ha.pos.z);
            for arc in &g.arcs {
                if !arc.touches(a) || arc.type_mask & (1 << SOLDIER) == 0 {
                    continue;
                }
                let b = arc.other(a);
                let Some(hb) = g.hub(b) else { continue };
                if !floors[usize::from(b)].is_nan() {
                    continue;
                }
                let jump = arc.flags & (crate::nav::Arc::JUMP | crate::nav::Arc::JET_JUMP) != 0;
                if let Some(f) = self.trace_floor(from, Vec2::new(hb.pos.x, hb.pos.z), jump) {
                    floors[usize::from(b)] = f;
                    queue.push_back(b);
                }
            }
        }
        // The first front to reach a hub may have walked UNDER it (a line
        // from a lower deck passes beneath a walkway's hub). So every hub
        // then hears all its resolved neighbours: each walks to it, and the
        // floor most of them arrive at wins (within [`VOTE_TOL`]; a tie
        // keeps what the hub has). A few rounds, until nothing moves.
        for _ in 0..VOTE_ROUNDS {
            let mut moved = false;
            for h in 0..n {
                if anchored[h] || floors[h].is_nan() {
                    continue;
                }
                let Some(hub) = g.hub(h as u16) else { continue };
                let to = Vec2::new(hub.pos.x, hub.pos.z);
                let mut votes: Vec<f32> = Vec::new();
                for arc in &g.arcs {
                    if !arc.touches(h as u16) || arc.type_mask & (1 << SOLDIER) == 0 {
                        continue;
                    }
                    let o = arc.other(h as u16);
                    let (Some(ho), Some(fo)) = (g.hub(o), self_floor(&floors, o)) else {
                        continue;
                    };
                    let jump = arc.flags & (crate::nav::Arc::JUMP | crate::nav::Arc::JET_JUMP) != 0;
                    if let Some(f) = self.trace_floor(Vec3::new(ho.pos.x, fo, ho.pos.z), to, jump) {
                        votes.push(f);
                    }
                }
                let support = |f: f32| votes.iter().filter(|v| (**v - f).abs() <= VOTE_TOL).count();
                let mine = support(floors[h]);
                let best = votes.iter().copied().map(|f| (support(f), f)).fold(
                    None,
                    |acc: Option<(usize, f32)>, c| match acc {
                        Some(a) if a.0 >= c.0 => Some(a),
                        _ => Some(c),
                    },
                );
                if let Some((count, f)) = best {
                    if count > mine && (f - floors[h]).abs() > VOTE_TOL {
                        floors[h] = f;
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
        for f in floors.iter().filter(|f| !f.is_nan()) {
            lowest = lowest.min(*f);
        }
        if lowest.is_finite() {
            self.kill_floor = Some(lowest - KILL_FLOOR_DROP);
        }
        self.nav_floors = floors;
        // The arcs no soldier can walk, each way (72 arc directions over
        // the 32 maps, 57 of them by the floor walk: BF2 Kamino's 123 ↔ 121
        // is drawn over 20 m of sea).
        let arcs = self.nav.as_ref().map_or(0, |g| g.arcs.len());
        let mut unwalkable = vec![0u8; arcs];
        if let Some(g) = self.nav.as_ref() {
            // The resolved floors and collision are fixed for this pass. Every
            // arc leaving a hub asks the same clearance and pen flood; compute
            // that pure start check only when the first eligible arc asks it.
            let mut starts = vec![None; g.hubs.len()];
            for (i, arc) in g.arcs.iter().enumerate() {
                if arc.type_mask & (1 << SOLDIER) == 0 {
                    continue;
                }
                for (bit, from) in [(1u8, arc.a), (2u8, arc.b)] {
                    if !self.arc_walkable(i as u16, from, crate::ai::HUB_LEVEL_TOL) {
                        unwalkable[i] |= bit;
                    }
                }
                // Card TGAP: walled only when walled BOTH ways - a one-way
                // failure is a slope the test's 45-degree step misjudges
                // (BF2 Utapau's 20 -> 21, BF1 Bespin's 86 -> 85: walked by
                // the port's bots 11 and 5 times in 3 rounds).
                if self.arc_walled(i as u16, arc.a, &mut starts)
                    && self.arc_walled(i as u16, arc.b, &mut starts)
                {
                    unwalkable[i] = 3;
                }
            }
        }
        self.nav_unwalkable = unwalkable;
        self.nav_trapped = self.trapped_hubs();
    }

    /// Card TGAP: is arc `arc` WALLED for a soldier leaving hub `from` - the
    /// floor walk crosses it, but a soldier who walks it round the walls
    /// ([`Sim::floor_path`], within [`WALL_BAND`] of the line) never gets
    /// within his cell of the far hub's disc on its floor? BF1 Tatooine's
    /// footpath arc 78 -> 77 meets a leaning slab between two rocks: the PS2's
    /// soldier is stopped there walking west (0 of 4 walks, measured
    /// 2026-10-09), the port's both ways, and its bots stood there until the
    /// 120 s recycle. Not judged (`false`): a jump arc, an end the walk did
    /// not resolve, an arc over [`WALL_SPAN`], a hub centre inside a body or
    /// in a pen ([`Sim::floor_reach`]: BF2 Naboo's hub 7 and Kamino's 124
    /// stand in a fountain and a planter), a line past a mover's
    /// envelope (BF2's doors stand closed at load), a search that ran out of
    /// cells. PROVISIONAL (ours): the PS2 has no such test.
    fn arc_walled(&self, arc: u16, from: u16, starts: &mut [Option<bool>]) -> bool {
        let Some(g) = self.nav.as_ref() else {
            return false;
        };
        let Some(a) = g.arcs.get(usize::from(arc)) else {
            return false;
        };
        if a.flags & (crate::nav::Arc::JUMP | crate::nav::Arc::JET_JUMP) != 0 {
            return false;
        }
        let to = a.other(from);
        let (Some(hf), Some(ht)) = (g.hub(from), g.hub(to)) else {
            return false;
        };
        let (Some(ff), Some(ft)) = (self.nav_floor(from), self.nav_floor(to)) else {
            return false;
        };
        let start = Vec3::new(hf.pos.x, ff, hf.pos.z);
        let (p, q) = (hf.xz(), ht.xz());
        if (q - p).length() > WALL_SPAN {
            return false;
        }
        if !*starts[usize::from(from)].get_or_insert_with(|| {
            self.soldier_clear(start)
                && self.floor_reach(start, super::spawn::PEN_CELLS) >= super::spawn::PEN_CELLS
        }) {
            return false;
        }
        let lo = p.min(q) - Vec2::splat(WALL_BAND);
        let hi = p.max(q) + Vec2::splat(WALL_BAND);
        if self
            .collision
            .mover_envelopes()
            .any(|e| e.min.x <= hi.x && e.max.x >= lo.x && e.min.z <= hi.y && e.max.z >= lo.y)
        {
            return false;
        }
        let reach = ht.radius + super::spawn::PEN_CELL;
        self.floor_path(
            start,
            q,
            ft,
            reach,
            crate::ai::HUB_LEVEL_TOL,
            WALL_BAND,
            WALL_CELLS,
        ) == Some(false)
    }

    /// Card TGAP: per hub, is a soldier there TRAPPED - over the arcs he may
    /// walk ([`Sim::arc_unwalkable`] off) he can never get back to the
    /// largest set of hubs of his part of the graph that all walk to each
    /// other? BF1 Tatooine's footpath 79 .. 86 is one: its only way on is the
    /// walled 78 -> 77 and hub 86 is its end. The router sends no soldier
    /// into one and starts none from one (`ai::route`). Empty with no graph.
    fn trapped_hubs(&self) -> Vec<bool> {
        let Some(g) = self.nav.as_ref() else {
            return Vec::new();
        };
        let n = g.hubs.len();
        let mut out: Vec<Vec<u16>> = vec![Vec::new(); n];
        for (i, arc) in g.arcs.iter().enumerate() {
            if !arc.allows(Mover::SOLDIER) {
                continue;
            }
            for from in [arc.a, arc.b] {
                let to = arc.other(from);
                if usize::from(from.max(to)) < n && !self.arc_unwalkable(i as u16, from) {
                    out[usize::from(from)].push(to);
                }
            }
        }
        let reach_from = |h: usize| {
            let mut seen = vec![false; n];
            let mut q = VecDeque::from([h]);
            seen[h] = true;
            while let Some(a) = q.pop_front() {
                for &b in &out[a] {
                    if !seen[usize::from(b)] {
                        seen[usize::from(b)] = true;
                        q.push_back(usize::from(b));
                    }
                }
            }
            seen
        };
        let reach: Vec<Vec<bool>> = (0..n).map(reach_from).collect();
        // Per part of the graph (`NavGraph::reachable` over every soldier
        // arc), its largest set of hubs that walk to each other; ties to the
        // lowest hub.
        let rep: Vec<usize> = (0..n)
            .map(|h| {
                (0..n)
                    .find(|&k| g.reachable(h as u16, k as u16, SOLDIER))
                    .unwrap_or(h)
            })
            .collect();
        let mut main: Vec<Option<(usize, usize)>> = vec![None; n];
        for (h, from_h) in reach.iter().enumerate() {
            let size = (0..n).filter(|&k| from_h[k] && reach[k][h]).count();
            if main[rep[h]].is_none_or(|(s, _)| size > s) {
                main[rep[h]] = Some((size, h));
            }
        }
        (0..n)
            .map(|h| main[rep[h]].is_some_and(|(_, m)| !reach[h][m]))
            .collect()
    }

    /// Card TGAP: is hub `h` one a soldier walks into and never out of
    /// ([`Sim::nav_trapped`])?
    pub fn hub_trapped(&self, h: u16) -> bool {
        self.nav_trapped
            .get(usize::from(h))
            .copied()
            .unwrap_or(false)
    }

    /// Did the floor walk find arc `arc` unwalkable for a soldier leaving
    /// hub `from` ([`Sim::nav_unwalkable`])?
    pub fn arc_unwalkable(&self, arc: u16, from: u16) -> bool {
        let Some(a) = self.nav.as_ref().and_then(|g| g.arcs.get(usize::from(arc))) else {
            return false;
        };
        let bit = if from == a.a { 1 } else { 2 };
        self.nav_unwalkable
            .get(usize::from(arc))
            .is_some_and(|b| b & bit != 0)
    }

    /// Walk the floor from `from` (a point ON a floor) in a straight line to
    /// `to` and return the height it arrives at — `None` when the walk is
    /// not one a soldier could make: more than [`TRACE_GAP`] of it over open
    /// air, or nothing to stand on at its end. A `jump` walk (a JUMP arc, a
    /// unit's own fall) may cross any gap and comes DOWN at its end onto the
    /// floor under it (BF2 Kamino's one-way jump 121 → 122 lands 8.6 m
    /// down).
    fn trace_floor(&self, from: Vec3, to: Vec2, jump: bool) -> Option<f32> {
        crate::ai::think_memo::walk(from, to, jump, || self.walk_floor_line(from, to, jump))
    }

    /// [`Sim::trace_floor`], walked.
    fn walk_floor_line(&self, from: Vec3, to: Vec2, jump: bool) -> Option<f32> {
        let start = Vec2::new(from.x, from.z);
        let len = (to - start).length();
        let steps = (len / TRACE_STEP).ceil().max(1.0) as u32;
        let step = len / steps as f32;
        let mut y = from.y;
        let mut gap = 0.0f32;
        for i in 1..=steps {
            let p = start + (to - start) * (i as f32 / steps as f32);
            let at = Vec3::new(p.x, y + TRACE_UP, p.y);
            let mut f = self.ground_point_under(at, true);
            // A crack narrower than a foot is not a drop (card STLVL): where
            // the one ray falls more than a step, the foot's ring is asked,
            // and a floor level with the walk under it is the floor.
            if f < y - STEP_HEIGHT {
                let foot = self.ground_under(at, true);
                if (foot - y).abs() <= CRACK_TOL {
                    f = foot;
                }
            }
            if f > NO_GROUND && f >= y - TRACE_DROP {
                y = f;
                gap = 0.0;
            } else {
                gap += step;
                if gap > TRACE_GAP && !jump {
                    return None;
                }
            }
        }
        if gap == 0.0 {
            return Some(y);
        }
        // In the air at the end: only a jump comes down.
        let f = self.ground_point_under(Vec3::new(to.x, y + TRACE_UP, to.y), true);
        (jump && f > NO_GROUND).then_some(f)
    }

    /// Can a soldier WALK arc `arc` from hub `from` to its other end — the
    /// floor walked along it arrives within `tol` of the far hub's resolved
    /// floor, with no gap a jump arc does not license? `true` when either
    /// end is unresolved (nothing to judge by).
    pub fn arc_walkable(&self, arc: u16, from: u16, tol: f32) -> bool {
        let Some(g) = self.nav.as_ref() else {
            return true;
        };
        let Some(a) = g.arcs.get(usize::from(arc)) else {
            return true;
        };
        let to = a.other(from);
        let (Some(hf), Some(ht)) = (g.hub(from), g.hub(to)) else {
            return true;
        };
        let (Some(ff), Some(ft)) = (self.nav_floor(from), self.nav_floor(to)) else {
            return true;
        };
        let jump = a.flags & (crate::nav::Arc::JUMP | crate::nav::Arc::JET_JUMP) != 0;
        self.trace_floor(
            Vec3::new(hf.pos.x, ff, hf.pos.z),
            Vec2::new(ht.pos.x, ht.pos.z),
            jump,
        )
        .is_some_and(|f| (f - ft).abs() <= tol)
    }

    /// Does the floor walked from `feet` to hub `h` arrive at the hub's
    /// resolved floor, within `tol` — is a unit standing at `feet` on the
    /// hub's own storey, however far up its slope (card STLVL)? `false`
    /// for an unresolved hub and for a walk over open air.
    pub fn floor_reaches_hub(&self, feet: Vec3, h: u16, tol: f32) -> bool {
        let Some(hub) = self.nav.as_ref().and_then(|g| g.hub(h)) else {
            return false;
        };
        let Some(floor) = self.nav_floor(h) else {
            return false;
        };
        self.trace_floor(feet, Vec2::new(hub.pos.x, hub.pos.z), false)
            .is_some_and(|f| (f - floor).abs() <= tol)
    }

    /// The resolved floor at hub `h`, when the walk reached it.
    pub fn nav_floor(&self, h: u16) -> Option<f32> {
        self.nav_floors
            .get(usize::from(h))
            .copied()
            .filter(|f| !f.is_nan())
    }
    /// Nearest eligible floor hubs; the caller owns floor lookup and filtering.
    pub(crate) fn nearest_floor_hubs<const N: usize>(
        &self,
        g: &crate::nav::NavGraph,
        pos: Vec3,
        floor: impl Fn(usize) -> Option<f32>,
    ) -> [(f32, u16); N] {
        let p = Vec2::new(pos.x, pos.z);
        let mut near = [(f32::INFINITY, u16::MAX); N];
        for (i, hub) in g.hubs.iter().enumerate() {
            let Some(f) = floor(i) else { continue };
            let d =
                ((hub.xz() - p).length() - hub.radius).max(0.0) + LEVEL_RISE * (f - pos.y).abs();
            let mut cand = (d, i as u16);
            for slot in &mut near {
                if cand.0 < slot.0 {
                    std::mem::swap(slot, &mut cand);
                }
            }
        }
        near
    }
    /// Where `feet` stands against the playable level (the module header's
    /// rule): on the terrain, in a trench cut into it, or no more than
    /// [`BOUND_DROP`] under the floor
    /// walked toward it from one of the [`LEVEL_HUBS`] nearest resolved hubs
    /// (nearest in XZ and in height) — a walk that stops at the first barrier,
    /// and then allows only [`RIM_DROP`]. A marked slope can also support
    /// the feet on the static floor inside its authored corridor.
    pub fn level_at(&self, feet: Vec3) -> Level {
        let Some(g) = self.nav.as_ref() else {
            return Level::Unknown;
        };
        if self.nav_floors.is_empty() {
            return Level::Unknown;
        }
        if let Some(hf) = self.heightfield.as_ref() {
            let t = hf.height_at(feet.x, feet.z);
            let cut = self.collision.is_cut(feet);
            if (feet.y - t).abs() <= STEP_HEIGHT && !cut {
                return Level::On;
            }
            // A trench (card LVLF): a floor in a hole the map's own models
            // cut into the terrain, near its surface and under the open sky.
            if cut
                && (-STEP_HEIGHT..=TRENCH_DEPTH).contains(&(t - feet.y))
                && self
                    .collision
                    .ray(feet + Vec3::Y * HEAD, Vec3::Y, TRENCH_SKY, Layers::SOLDIER)
                    .is_none()
            {
                return Level::On;
            }
        }
        // A hole the map's own models cut into the terrain is no hull (card
        // BOWL): BF2 Hoth's trenches lie 2 m under the snow between barriers,
        // and bots defend from them.
        let trench = self.heightfield.is_some() && self.collision.is_cut(feet);
        let p = Vec2::new(feet.x, feet.z);
        // The nearest hubs: by the XZ distance to their discs plus the height
        // between their floor and the feet, weighted ([`LEVEL_RISE`]) so a
        // hub on the feet's own storey outranks one straight overhead (index
        // breaks ties).
        let near = self.nearest_floor_hubs::<LEVEL_HUBS>(g, feet, |i| {
            let f = self.nav_floors[i];
            (!f.is_nan()).then_some(f)
        });
        // Walk from each hub toward the point, as far as the first barrier:
        // the floor reached there, against the feet. A walk that ends near
        // the point and level with it says ON; any walk that ends over it
        // says OFF; walks that end level with it but far away (a room behind
        // a wall's barrier) say nothing.
        let mut over: Option<f32> = None;
        for &(_, h) in near.iter().filter(|c| c.1 != u16::MAX) {
            let hub = &g.hubs[usize::from(h)];
            let floor = self.nav_floors[usize::from(h)];
            let from = hub.xz();
            let stop = from + (p - from) * g.barrier_stop(from, p, SOLDIER);
            let Some(at) = self.trace_floor(Vec3::new(from.x, floor, from.y), stop, false) else {
                continue; // open air between the hub and the point
            };
            let drop = at - feet.y;
            // A walk the barrier stopped short stands at a rim or a wall:
            // what is level with it past the barrier is a room, what is
            // under it is a hull (card BOWL).
            let gap = (stop - p).length();
            let allow = if gap > RIM_GAP && !trench {
                RIM_DROP
            } else {
                BOUND_DROP
            };
            if drop <= allow {
                if gap <= LEVEL_SLACK {
                    return Level::On;
                }
            } else if over.is_none_or(|b| drop < b) {
                over = Some(drop);
            }
        }
        match over {
            Some(_) if self.on_supported_arc_floor(g, feet) => Level::On,
            Some(drop) => Level::Off(drop),
            None => Level::Unknown,
        }
    }

    fn on_supported_arc_floor(&self, g: &crate::nav::NavGraph, feet: Vec3) -> bool {
        g.floor_support_arcs.iter().any(|&index| {
            let Some(arc) = g.arcs.get(usize::from(index)) else {
                return false;
            };
            let (Some(a), Some(b), Some(ay), Some(by)) = (
                g.hub(arc.a),
                g.hub(arc.b),
                self.nav_floor(arc.a),
                self.nav_floor(arc.b),
            ) else {
                return false;
            };
            let line = b.xz() - a.xz();
            let offset = Vec2::new(feet.x, feet.z) - a.xz();
            let t = line.dot(offset) / line.length_squared();
            (0.0..=1.0).contains(&t)
                && (offset - line * t).length_squared() <= a.radius.min(b.radius).powi(2)
                && (feet.y - (ay + (by - ay) * t)).abs() <= BOUND_DROP
                && (self.floor_point_under_static(feet, feet.y + STEP_HEIGHT) - feet.y).abs()
                    <= STEP_HEIGHT
        })
    }

    /// The rim census (card BOWL): is `feet` BEYOND a barrier from each of
    /// its [`RIM_HUBS`] nearest resolved hubs ([`Sim::level_at`]'s order),
    /// by more than [`RIM_GAP`]? Then how far it is under the nearest one's
    /// floor (m; negative over it) — a unit there and well under that floor
    /// stands on a hull. `None` on the level's side of a barrier, on the
    /// terrain or in a hole cut into it (a trench), and with no resolved
    /// graph. A measurement: no rule reads it.
    pub fn rim_drop(&self, feet: Vec3) -> Option<f32> {
        let g = self.nav.as_ref()?;
        if let Some(hf) = self.heightfield.as_ref() {
            let t = hf.height_at(feet.x, feet.z);
            if (feet.y - t).abs() <= STEP_HEIGHT || self.collision.is_cut(feet) {
                return None;
            }
        }
        let p = Vec2::new(feet.x, feet.z);
        let near = self.nearest_floor_hubs::<RIM_HUBS>(g, feet, |i| self.nav_floor(i as u16));
        let first = self.nav_floor(near[0].1)?;
        near.iter()
            .filter(|c| c.1 != u16::MAX)
            .all(|&(_, h)| {
                let from = g.hubs[usize::from(h)].xz();
                let t = g.barrier_stop(from, p, SOLDIER);
                t < 1.0 && (p - from).length() * (1.0 - t) > RIM_GAP
            })
            .then_some(first - feet.y)
    }

    /// Where a unit that is off the level at `feet` walks to come back: the
    /// centre of the nearest resolved hub (XZ) whose floor is not under its
    /// feet by more than [`BOUND_DROP`], at that floor — the `skip`-th
    /// nearest, wrapping, so a caller that got nowhere asks for the next.
    pub fn return_point(&self, feet: Vec3, skip: u8) -> Option<Vec3> {
        let g = self.nav.as_ref()?;
        let p = Vec2::new(feet.x, feet.z);
        let mut near: Vec<(f32, u16)> = g
            .hubs
            .iter()
            .enumerate()
            .filter(|&(i, _)| {
                self.nav_floors
                    .get(i)
                    .is_some_and(|f| !f.is_nan() && *f >= feet.y - BOUND_DROP)
            })
            .map(|(i, h)| ((h.xz() - p).length_squared(), i as u16))
            .collect();
        if near.is_empty() {
            return None;
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let (_, h) = near[usize::from(skip) % near.len().min(RETURN_HUBS)];
        let hub = &g.hubs[usize::from(h)];
        Some(Vec3::new(
            hub.pos.x,
            self.nav_floors[usize::from(h)],
            hub.pos.z,
        ))
    }

    /// Card RETW: the FLOOR walk from `feet` back to the level — a search
    /// over a [`RETURN_CELL`] grid (up a step of [`STEP_HEIGHT`], down at
    /// most [`RETURN_STEP_DOWN`] a cell and never more than [`BOUND_DROP`]
    /// under the feet, onto a face a soldier stands on, with no wall at the
    /// chest between two cells) to the first cell inside a resolved hub's
    /// disc, on its floor and ON the level ([`Sim::level_at`]: a hub's disc
    /// reaches into a walled pit under BF2 Utapau's hub 60 that reads off),
    /// the nearest hub's disc first (A*, the
    /// straight distance to the nearest disc as the estimate), then pulled
    /// straight along what a soldier's body clears ([`Sim::floor_line`])
    /// into at most [`RETURN_PATH`] points, the hub's cell last. `None`: no
    /// such walk within [`RETURN_RANGE`] / [`RETURN_CELLS`] (the caller
    /// falls back on [`Sim::return_point`]). A straight line at the nearest
    /// hub is not enough (card LVLF's v1/v2): BF2 Kamino's pod pad stands
    /// 1.5 m over the deck its hub is on, and the way down is round the pod.
    pub fn return_path(&self, feet: Vec3) -> Option<ReturnPath> {
        let g = self.nav.as_ref()?;
        let p0 = Vec2::new(feet.x, feet.z);
        let low = feet.y - BOUND_DROP;
        let hubs: Vec<(Vec2, f32, f32)> = g
            .hubs
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                let f = self.nav_floor(i as u16)?;
                let r = h.radius.max(RETURN_HUB_RADIUS);
                ((h.xz() - p0).length() <= RETURN_RANGE + r && f >= low).then_some((h.xz(), r, f))
            })
            .collect();
        self.floor_walk(feet, &hubs)
    }

    /// Card RLOOP: the same floor walk from `feet` to ONE point — a cell
    /// within [`RETURN_HUB_RADIUS`] of `to`, at its height and on the level
    /// (a bot given up at one place walks to its leg's waypoint this way).
    /// `None`: no such walk within [`RETURN_RANGE`] / [`RETURN_CELLS`].
    pub fn floor_walk_to(&self, feet: Vec3, to: Vec3) -> Option<ReturnPath> {
        let q = Vec2::new(to.x, to.z);
        if (q - Vec2::new(feet.x, feet.z)).length() > RETURN_RANGE || to.y < feet.y - BOUND_DROP {
            return None;
        }
        self.floor_walk(feet, &[(q, RETURN_HUB_RADIUS, to.y)])
    }

    /// The search of [`Sim::return_path`] / [`Sim::floor_walk_to`] to the
    /// first cell inside one of the discs `hubs` (centre, radius, floor).
    fn floor_walk(&self, feet: Vec3, hubs: &[(Vec2, f32, f32)]) -> Option<ReturnPath> {
        if hubs.is_empty() {
            return None;
        }
        let p0 = Vec2::new(feet.x, feet.z);
        let at = |c: (i32, i32), y: f32| {
            Vec3::new(
                feet.x + c.0 as f32 * RETURN_CELL,
                y,
                feet.z + c.1 as f32 * RETURN_CELL,
            )
        };
        let guess = |p: Vec3| {
            let q = Vec2::new(p.x, p.z);
            hubs.iter()
                .map(|&(h, r, _)| ((h - q).length() - r).max(0.0))
                .fold(f32::INFINITY, f32::min)
        };
        // A cell is a column and a storey of it (2 m bands), so a floor
        // under a deck is a cell of its own.
        type Key = (i32, i32, i32);
        let key = |c: (i32, i32), y: f32| -> Key { (c.0, c.1, (y / 2.0).floor() as i32) };
        // Per cell: the cell it was reached from, its floor, the walk to it.
        let mut seen: std::collections::HashMap<Key, (Key, f32, f32)> =
            std::collections::HashMap::new();
        // The open cells, least estimate first; ties by the order found
        // (deterministic).
        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(u32, u32, Key)>> =
            std::collections::BinaryHeap::new();
        let k0 = key((0, 0), feet.y);
        seen.insert(k0, (k0, feet.y, 0.0));
        open.push(std::cmp::Reverse((0, 0, k0)));
        let mut order = 0u32;
        let mut end = None;
        let mut cells = 0usize;
        let space = self.space_round();
        // The floor a column answers from one height: up to eight cells step
        // onto it from the same floor, and only their walls differ.
        let mut floors: std::collections::HashMap<(i32, i32, u32), Option<f32>> =
            std::collections::HashMap::new();
        while let Some(std::cmp::Reverse((_, _, k))) = open.pop() {
            let (_, y, len) = seen[&k];
            let c = (k.0, k.1);
            let p = at(c, y);
            if k != k0
                && hubs.iter().any(|&(h, r, f)| {
                    (h - Vec2::new(p.x, p.z)).length() <= r && (f - y).abs() <= RETURN_HUB_TOL
                })
                && self.level_at(p) == Level::On
            {
                end = Some(k);
                break;
            }
            cells += 1;
            if cells >= RETURN_CELLS {
                break;
            }
            for (dx, dz) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let n = (c.0 + dx, c.1 + dz);
                let np = at(n, y);
                if (Vec2::new(np.x, np.z) - p0).length() > RETURN_RANGE {
                    continue;
                }
                let f = *floors
                    .entry((n.0, n.1, y.to_bits()))
                    .or_insert_with(|| self.return_floor(p, np));
                let Some(f) = f else {
                    continue;
                };
                if space && !self.return_clear(p, np, f) {
                    continue;
                }
                let nk = key(n, f);
                let nlen = len
                    + if dx != 0 && dz != 0 {
                        RETURN_CELL * std::f32::consts::SQRT_2
                    } else {
                        RETURN_CELL
                    };
                // A ground edge that cannot improve this storey needs no wall ray.
                if seen.get(&nk).is_some_and(|&(_, _, l)| l <= nlen)
                    || (!space && !self.return_clear(p, np, f))
                {
                    continue;
                }
                seen.insert(nk, (k, f, nlen));
                order += 1;
                // Centimetres: an integer order on the estimate.
                let est = ((nlen + guess(at(n, f))) * 100.0) as u32;
                open.push(std::cmp::Reverse((est, order, nk)));
            }
        }
        // The cells back from the hub's, then forward.
        let mut chain = Vec::new();
        let mut k = end?;
        loop {
            let (b, y, _) = seen[&k];
            chain.push(at((k.0, k.1), y));
            if b == k {
                break;
            }
            k = b;
        }
        chain.reverse();
        chain[0] = feet;
        // Pulled straight: from each corner, the farthest cell of the chain
        // a straight walk reaches.
        let mut out = ReturnPath::default();
        let mut a = 0;
        while a + 1 < chain.len() && usize::from(out.len) < RETURN_PATH {
            let mut b = a + 1;
            while b + 1 < chain.len() && self.floor_line(chain[a], chain[b + 1]) {
                b += 1;
            }
            out.pts[usize::from(out.len)] = chain[b];
            out.len += 1;
            a = b;
        }
        Some(out)
    }

    /// One step of [`Sim::return_path`]'s search from the cell at `p` (on
    /// its floor) to the column at `n`, whose floor is `f`
    /// ([`Sim::return_floor`]): a soldier can walk onto it when no wall stands
    /// at the chest from `p` to a soldier's radius past `n`.
    fn return_clear(&self, p: Vec3, n: Vec3, f: f32) -> bool {
        let d = Vec3::new(n.x - p.x, 0.0, n.z - p.z);
        let len = d.length();
        let chest = Vec3::new(p.x, p.y.max(f) + RETURN_CHEST, p.z);
        self.collision
            .ray(
                chest,
                d / len,
                len + crate::unit::SOLDIER_RADIUS,
                Layers::SOLDIER,
            )
            .is_none()
    }

    /// The floor at the column `n` a step from `p` (on its floor), when a
    /// soldier can step onto it: up at most a step, down at most
    /// [`RETURN_STEP_DOWN`], a face he stands on.
    fn return_floor(&self, p: Vec3, n: Vec3) -> Option<f32> {
        let (f, nrm) = self.ground_point_under_n(Vec3::new(n.x, p.y, n.z), p.y + STEP_HEIGHT);
        (f > NO_GROUND
            && f <= p.y + STEP_HEIGHT
            && f >= p.y - RETURN_STEP_DOWN
            && nrm.y >= crate::soldier::FLOOR_MIN_NY)
            .then_some(f)
    }

    /// Can a soldier walk the straight line from `a` to `b` (both on a
    /// floor) — every [`RETURN_CELL`] of it a step [`Sim::return_floor`]
    /// allows, none under the lower end by more than
    /// [`crate::ai::LOST_DOWN`] (a lost bot's steering refuses a step under
    /// that), no wall a standing soldier's body meets on the way, arriving
    /// at `b`'s floor?
    pub fn floor_line(&self, a: Vec3, b: Vec3) -> bool {
        let d = Vec2::new(b.x - a.x, b.z - a.z);
        let steps = (d.length() / RETURN_CELL).ceil().max(1.0) as u32;
        let low = a.y.min(b.y) - crate::ai::LOST_DOWN;
        let mut p = a;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let n = Vec3::new(a.x + d.x * t, p.y, a.z + d.y * t);
            match self.return_floor(p, n) {
                Some(f) if f >= low => p = Vec3::new(n.x, f, n.z),
                _ => return false,
            }
        }
        if (p.y - b.y).abs() > RETURN_HUB_TOL {
            return false;
        }
        // The body swept along it at the higher end's floor (a step lifts
        // it over a kerb), a little thinner than the soldier's: the search's
        // cells may lie a radius from a wall (a corner it went round), and
        // the line from one is still pulled past what it would only graze.
        let dir = Vec3::new(d.x, 0.0, d.y);
        let len = dir.length();
        if len <= 0.0 {
            return true;
        }
        let body = crate::collision::Capsule {
            base: Vec3::new(a.x, a.y.max(b.y), a.z),
            height: crate::unit::capsule_height(crate::soldier::Stance::Stand),
            radius: crate::unit::SOLDIER_RADIUS - RETURN_SLACK,
        };
        self.collision
            .wall_sweep(&body.stepped(), dir / len, len)
            .is_none()
    }

    /// Did a soldier who just landed at `feet` with vertical speed `vy` land
    /// UNDER THE MAP — below the playable level, after a real fall, with
    /// collision over his head no higher than he fell from? That is a fall
    /// through or round a floor onto what lies under it (BF2 Kamino's hidden
    /// discs and platform cores, re-kamino.md §4.1), and not a walk down a
    /// hull or a drop into the open.
    pub fn lands_under_the_map(&self, feet: Vec3, vy: f32) -> bool {
        if vy > -UNDER_MAP_SPEED || !self.level_at(feet).is_off() {
            return false;
        }
        let fell = vy * vy / (2.0 * crate::soldier::GRAVITY);
        self.collision
            .ray(feet + Vec3::Y * HEAD, Vec3::Y, fell, Layers::SOLDIER)
            .is_some()
    }
}

/// `floors[h]` when it is resolved.
fn self_floor(floors: &[f32], h: u16) -> Option<f32> {
    floors.get(usize::from(h)).copied().filter(|f| !f.is_nan())
}

#[cfg(test)]
mod tests;
