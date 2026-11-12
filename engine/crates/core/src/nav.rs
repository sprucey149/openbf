//! The authored AI planning graph (`plan` / `BARR`) and the PS2's router over
//! it — Phase A of the infantry AI (PLAN Log "PHASE A DESIGN — navigation on
//! the authored graphs").
//!
//! The graph is an INPUT to the sim like the heightfield and the collision
//! world: built once by `game::layout::nav_graph` from the baked manifest,
//! immutable for the round, and NOT part of [`crate::sim::Sim::hash`].
//!
//! Everything here is measured — RE_LOG "`plan` / `plnp` / `Hint` / `BARR`":
//!
//! - a hub is a position, a radius and up to eight arc slots; hub `y` is the
//!   editor's PICK HEIGHT, not a floor (BF2 POL1's 189 hubs are all at y = 0
//!   under floors 13–33 m up). Goal locate uses the authored geometry below;
//!   the follower resolves floors and uses the named XZ unit-locate deviation;
//! - an arc joins two hubs, is walked in both directions, and carries the
//!   `AISizeType` mask of the units allowed on it (bit 0 SOLDIER, 1 HOVER,
//!   2 SMALL, 3 MEDIUM, 4 HUGE, 5 FLYER — the five tables cover bits 0..4);
//! - "where am I on the graph" (`FUN_00266678` / `FUN_002eb918`) is the
//!   containing hub with the SMALLEST radius (BF1 sphere, BF2 XZ disc), else
//!   a containing 3-D arc corridor, else the nearest arc — see [`NavGraph::locate`];
//! - the route table IS the router: one byte per alternative, `weight << 3 |
//!   slot`, walked by the weighted chooser in [`NavGraph::next_hop`]. There is
//!   no graph search anywhere in either ELF; [`NavGraph::route_bfs`] is the
//!   PORT'S OWN fallback for the entries the authors left empty.
//!
//! PROVISIONAL/ours, and labelled as such where they appear: the BFS fallback,
//! the connected-component cache that keeps the "is that post reachable at
//! all" test O(1), and the flank table ([`flank`], card FLNK: per post pair
//! the best other way, which a share of the attackers walks).

mod barriers;

use glam::{Vec2, Vec3};

pub mod flank;
pub use flank::{FlankRoute, FlankTable};

/// Routed unit types on the disc: the five route tables (SOLDIER, HOVER,
/// SMALL, MEDIUM, HUGE). FLYER has no table — flyers do not route.
pub const TYPE_COUNT: usize = 5;

/// Type INDEX of SOLDIER (mask bit 0). Every infantry unit routes as this in
/// Phase A (design decision 7).
pub const SOLDIER: usize = 0;

/// Who is asking the graph: the `AISizeType` slot the unit routes as, and
/// whether it wears a jet pack.
///
/// The jet flag exists because BF2 marks some arcs `jet_jump` ([`Arc::JET_JUMP`],
/// PROVISIONAL bit — the PC tool's names, not the RE's): a leap only a jet
/// class can make. Routing a jetless soldier over one hands it a waypoint it
/// can never reach, which is one of the two shapes the A.1 sweep's deep stalls
/// took (PLAN "PHASE A.2 DESIGN" decision 4). The follower selects the vehicle
/// type or the infantry class's jet flag; this flag does not implement jet physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mover {
    /// Index into the five route tables — [`SOLDIER`] and friends.
    pub ty: usize,
    /// Does its class wear a jet pack?
    pub jet: bool,
}

impl Mover {
    /// Infantry without a jet pack.
    pub const SOLDIER: Mover = Mover {
        ty: SOLDIER,
        jet: false,
    };

    pub const fn new(ty: usize, jet: bool) -> Mover {
        Mover { ty, jet }
    }
}

// ---------------------------------------------------------------------------
// The corner walk (PLAN Log "A.1 addendum — barriers are the PS2's Obstacles").
// A barrier NEVER touches the routing tables; it is an obstacle for the local
// layer. Every hub-to-hub leg is tested against the barriers that block the
// unit's type, and a leg that crosses one is re-routed over the barriers'
// silhouette corners.
// ---------------------------------------------------------------------------

/// How far outside the TESTED rectangle a detour corner sits (m). The PS2
/// tests the leg with the unit radius and expands its corners by `r + 0.5`
/// (`FUN_00268060`), so a corner is always clear of the rectangle its own
/// edge test uses — without that gap every silhouette edge grazes its own
/// barrier and the search finds nothing. Ours keeps the same 0.5 m gap
/// relative to whatever margin the caller passes. PROVISIONAL.
pub const CORNER_CLEARANCE: f32 = 0.5;

/// Half-width of the search band around the leg: `max(BAND_MIN, leg length)`,
/// measured perpendicular to the leg. The PS2's `DAT_00588a60`.
pub const DETOUR_BAND_MIN: f32 = 25.0;

/// Corner-chain depth cap — the PS2's `FUN_002691a0` (`depth < 32`).
pub const DETOUR_MAX_DEPTH: u32 = 32;

/// The PS2's PathNode pool: a search makes at most this many nodes
/// (`DAT_00588a14`, 1024).
pub const DETOUR_MAX_NODES: usize = 1024;

/// OURS, PROVISIONAL: a search gives up (no way: the leg is walked straight)
/// after this many segment tests. The PS2 spends a budget of ten fine
/// segment tests a tick over a queue of path requests (`DAT_00518298`) and is
/// bounded only by its node pool; we answer a leg in one call, so the work is
/// bounded here.
pub const DETOUR_MAX_TESTS: u32 = 2000;

/// One planning hub.
#[derive(Debug, Clone, PartialEq)]
pub struct Hub {
    /// World position AS AUTHORED. `y` is the editor's pick height.
    pub pos: Vec3,
    pub radius: f32,
    /// The hub's arc slots in order, empty (0xff) slots dropped — slot `k` of
    /// a route-table entry indexes THIS list.
    pub arcs: Vec<u16>,
}

impl Hub {
    pub(crate) fn xz(&self) -> Vec2 {
        Vec2::new(self.pos.x, self.pos.z)
    }
}

/// One arc between two hubs, walked in both directions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub a: u16,
    pub b: u16,
    /// `AISizeType` mask of the units allowed on this arc.
    pub type_mask: u32,
    /// BF2 only (0 on every BF1 arc): bits 0..2 mark an arc a unit cannot be
    /// localised on; the PC tool names them `one_way = 1, jump = 2,
    /// jet_jump = 4` [H].
    pub flags: u32,
}

impl Arc {
    /// `flags` bit 0 — the PC tool's `one_way` [H]: never a unit's NEAREST
    /// arc ([`NavGraph::locate`] step 3, BF2 `FUN_002ebb48`).
    pub const ONE_WAY: u32 = 1;
    /// `flags` bit 1 — the PC tool's `jump` [H]: a hop. STAYS ROUTABLE for a
    /// soldier; the originals' infantry jump.
    pub const JUMP: u32 = 2;
    /// `flags` bit 2 — the PC tool's `jet_jump` [H]: a leap that needs a jet
    /// pack. Excluded for a mover without one. PROVISIONAL: which bit is which
    /// is the PC tool's naming, not the RE's.
    pub const JET_JUMP: u32 = 4;

    /// May `m` walk this arc? The authored `AISizeType` mask, and — the Phase
    /// A.2 addition — a [`Arc::JET_JUMP`] arc only with a jet pack.
    pub fn allows(&self, m: Mover) -> bool {
        self.type_mask & (1u32 << m.ty) != 0 && (m.jet || self.flags & Arc::JET_JUMP == 0)
    }

    /// The end that is not `hub` (`FUN_00267540`: `next = a == cur ? b : a`).
    pub fn other(&self, hub: u16) -> u16 {
        if self.a == hub {
            self.b
        } else {
            self.a
        }
    }

    pub fn touches(&self, hub: u16) -> bool {
        self.a == hub || self.b == hub
    }
}

/// One `BARR` barrier: an oriented rectangle in XZ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Barrier {
    /// XZ of the loader's world position — the bake already negated z.
    pub centre: Vec2,
    /// The rectangle's two axes (unit length), from the stored rotation rows.
    pub axes: [Vec2; 2],
    /// Half sizes along `axes`.
    pub half: Vec2,
    /// The `AISizeType` mask this barrier stops. FLAG polarity is "blocks
    /// these types" (RE_LOG: every query gates on `(FLAG & unitMask) != 0`).
    pub mask: u32,
    /// Open or closed. Every barrier on the discs LOADS enabled (the BF1
    /// loader ignores the disc bit, BF2 ORs 0x20 in); BF2's mission Lua
    /// opens and closes door/shield barriers at runtime. The port's mission
    /// setup applies `EnableBarriers` / `DisableBarriers` through
    /// [`NavGraph::set_barriers`].
    pub enabled: bool,
}

/// One authored `Hint` node, consumed by the commander for defence spots
/// and by `ai::tactic` for combat positions and stance masks.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub name: String,
    /// 0 Snipe, 1 Patrol, 2 Cover, 4 JetJump, 5 Minefield, 6 Land, 7/8 BF2.
    pub kind: u8,
    /// Translation AS STORED — a hint's translation is NOT z-negated.
    pub pos: Vec3,
    pub rows: [[f32; 3]; 3],
    pub radius: Option<f32>,
    pub mode: Option<u8>,
    /// The ALLOWED-STANCE MASK this hint's authored `PrimaryStance` asks for
    /// (brief D13; the field is a BITMASK - RE lead-verified, see
    /// [`crate::commander::stance_mask`], which is what the LOADER applies to
    /// the authored value). Never 0: a hint with no authored stance asks for
    /// stand. The per-game half of the rule (BF2 masks `& 3`, BF1 keeps the raw
    /// byte) is applied where the manifest's `source_game` is in hand -
    /// `game::layout::nav_graph` - so the sim reads one mask and needs no game
    /// flag of its own. The manifests have carried the authored value since the
    /// bake was written (`ManifestHint::primary_stance`), so no rebake is
    /// needed for either field.
    pub stance_mask: u8,
    /// Authored `SecondaryStance` mask, read into combat positions by `ai::tactic`.
    pub secondary_stance_mask: u8,
    /// Authored peek sides ([`crate::commander::peek_sides`]: bit 0 −right,
    /// bit 1 +right). Carried without a Cover peek-state consumer yet.
    pub peek_sides: u8,
    pub command_post: Option<String>,
}

/// The per-type, per-hub route rows: `rows[t][hub]` is a run of `n ×
/// hub_count` bytes, row `k` being the `k`-th alternative for every possible
/// destination. An entry byte is `weight << 3 | slot`; 0 = no route.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteTable {
    pub hub_count: usize,
    pub types: usize,
    /// Indexed `t * hub_count + hub`; each element holds that hub's rows back
    /// to back.
    rows: Vec<Vec<u8>>,
}

impl RouteTable {
    /// Parse the bake's `nav/routes_<key>.bin`:
    ///
    /// ```text
    /// u16 hubCount; u8 typeCount (5)
    /// for t in 0..typeCount, for hub in 0..hubCount:  u8 rows; rows x hubCount bytes
    /// ```
    ///
    /// `Err` names what did not add up — a truncated sidecar is a bake bug,
    /// not something to route on.
    pub fn parse(bytes: &[u8]) -> Result<RouteTable, String> {
        if bytes.len() < 3 {
            return Err(format!("routes sidecar is {} bytes", bytes.len()));
        }
        let hub_count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
        let types = bytes[2] as usize;
        let mut at = 3usize;
        let mut rows = Vec::with_capacity(types * hub_count);
        for t in 0..types {
            for h in 0..hub_count {
                let n = *bytes
                    .get(at)
                    .ok_or_else(|| format!("routes: truncated at type {t} hub {h}"))?
                    as usize;
                at += 1;
                let len = n * hub_count;
                let end = at + len;
                if end > bytes.len() {
                    return Err(format!(
                        "routes: type {t} hub {h} wants {len} bytes, {} left",
                        bytes.len() - at
                    ));
                }
                rows.push(bytes[at..end].to_vec());
                at = end;
            }
        }
        if at != bytes.len() {
            return Err(format!(
                "routes: {} trailing bytes after {hub_count} hubs x {types} types",
                bytes.len() - at
            ));
        }
        Ok(RouteTable {
            hub_count,
            types,
            rows,
        })
    }

    /// Build one directly (tests, and graphs whose sidecar is missing).
    pub fn from_rows(hub_count: usize, types: usize, rows: Vec<Vec<u8>>) -> RouteTable {
        RouteTable {
            hub_count,
            types,
            rows,
        }
    }

    /// An empty table: every lookup finds no route (the BFS fallback answers).
    pub fn empty(hub_count: usize) -> RouteTable {
        RouteTable {
            hub_count,
            types: TYPE_COUNT,
            rows: vec![Vec::new(); TYPE_COUNT * hub_count],
        }
    }

    /// How many alternative rows `hub` has for type `ty` (`None` out of
    /// range) — what a `plnp` patch is validated against.
    pub(crate) fn hub_rows_len(&self, ty: usize, hub: usize) -> Option<usize> {
        (ty < self.types && hub < self.hub_count)
            .then(|| self.rows[ty * self.hub_count + hub].len() / self.hub_count.max(1))
    }

    /// Row `k` of `hub` for type `ty`: one entry byte per destination hub.
    pub fn row(&self, ty: usize, hub: usize, k: usize) -> Option<&[u8]> {
        if ty >= self.types || hub >= self.hub_count {
            return None;
        }
        let w = self.hub_count;
        self.rows[ty * w + hub].get(k * w..(k + 1) * w)
    }

    /// Row `k` of `hub` for type `ty`, for a `plnp` patch to XOR onto.
    pub(crate) fn hub_row_mut(&mut self, ty: usize, hub: usize, k: usize) -> Option<&mut [u8]> {
        if ty >= self.types || hub >= self.hub_count {
            return None;
        }
        let w = self.hub_count;
        self.rows[ty * w + hub].get_mut(k * w..(k + 1) * w)
    }

    /// The non-zero entries for `dest` at `hub`, in ROW ORDER (row 0 is the
    /// primary alternative). Borrowed rather than collected: the one caller
    /// ([`NavGraph::next_hop`]) walks the answer twice — once for the budget,
    /// once for the pick — and it is on the per-think path, so a `Vec` here
    /// was an allocation per chooser step.
    pub fn entries(&self, ty: usize, hub: u16, dest: u16) -> impl Iterator<Item = u8> + '_ {
        let (hub, dest) = (hub as usize, dest as usize);
        let run: &[Vec<u8>] = if ty >= self.types || hub >= self.hub_count || dest >= self.hub_count
        {
            &[]
        } else {
            std::slice::from_ref(&self.rows[ty * self.hub_count + hub])
        };
        let width = self.hub_count.max(1);
        run.iter().flat_map(move |r| {
            r.chunks_exact(width)
                .filter_map(move |row| Some(row[dest]).filter(|&e| e != 0))
        })
    }
}

/// Where a point sits on the graph ([`NavGraph::locate`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Locus {
    /// Inside that hub (the smallest such hub).
    Hub(u16),
    /// In that arc's corridor, `t` along it from `a` to `b`.
    Arc(u16, f32),
    /// Neither: the nearest arc, and `t` of the point of it nearest `pos`.
    Nearest(u16, f32),
}

/// The result of one chooser step.
pub type Hop = (u16, u16);

/// The whole authored graph for one world.
#[derive(Debug, Clone, Default)]
pub struct NavGraph {
    pub hubs: Vec<Hub>,
    pub arcs: Vec<Arc>,
    pub routes: RouteTable,
    pub barriers: Vec<Barrier>,
    /// Per barrier, the FNV-1a hash of its lower-cased name — what the
    /// mission's `EnableBarriers` / `DisableBarriers` name
    /// ([`NavGraph::set_barriers`]). Empty on a graph built without names.
    pub barrier_names: Vec<u32>,
    /// Authored hints for defence spots, combat positions and stance selection.
    pub hints: Vec<Hint>,
    /// BF2: a hub contains a point by its XZ disc (`FUN_002ea878`); `false`,
    /// BF1: by its sphere (`FUN_00265d38`). Set by the loader from the
    /// manifest's game ([`NavGraph::locate`]).
    pub hub_discs: bool,
    /// BF2's `plnp` route patches and the blocked-group mask in force
    /// (`crate::route_patch`); `None` on BF1 and on maps that ship none.
    pub(crate) patches: Option<crate::route_patch::RoutePatches>,
    /// PROVISIONAL (ours, not on the PS2): per type, the connected component
    /// of each hub over the arcs that type may use. Purely derived — it makes
    /// "can this unit reach that post at all" O(1) instead of a BFS per post
    /// per think. `comp[t][h] == comp[t][k]` iff [`NavGraph::route_bfs`] can
    /// get from `h` to `k`.
    comp: Vec<Vec<u32>>,
    /// Per hub, its arcs sorted by ARC INDEX — the deterministic expansion
    /// order of `route_bfs`, built from EVERY arc that touches the hub (a hub
    /// has only eight slots; the arc list is the whole truth).
    adj: Vec<Vec<u16>>,
}

/// One node of [`NavGraph::detour`]'s A\* while it runs: its point, the cost
/// so far and the parent that gave it (`u16::MAX`: the start).
#[derive(Clone, Copy)]
struct DetourNode {
    p: Vec2,
    g: f32,
    parent: u16,
}

/// Rounding allowance of [`DetourSearch`]'s culling, in metres: far above the
/// float dust of map coordinates, so that it can only keep a barrier too many.
const DETOUR_CULL_SLACK: f32 = 0.05;

/// The box a culled segment `a -> b` lies in, with [`DETOUR_CULL_SLACK`]. A leg
/// that is not finite gets the whole plane: `Vec2::min` would drop a NaN end, and
/// every barrier's own test answers such a leg (with 0, usually), near or far.
fn cull_box(a: Vec2, b: Vec2) -> (Vec2, Vec2) {
    if !(a.is_finite() && b.is_finite()) {
        return (Vec2::NEG_INFINITY, Vec2::INFINITY);
    }
    (
        a.min(b) - Vec2::splat(DETOUR_CULL_SLACK),
        a.max(b) + Vec2::splat(DETOUR_CULL_SLACK),
    )
}

/// One barrier a corner search may meet.
#[derive(Clone, Copy)]
struct DetourCand {
    centre: Vec2,
    /// The barrier's rectangle grown by the leg's radius lies in this circle.
    reach: f32,
    index: u16,
}

/// A running corner search ([`NavGraph::detour`]).
struct DetourSearch<'a> {
    g: &'a NavGraph,
    /// The barriers that stop the walker and hold neither end of the leg, and
    /// can be reached from inside the search's band, by the X of their centres.
    cand: Vec<DetourCand>,
    /// The largest `reach` of `cand`.
    max_reach: f32,
    radius: f32,
    goal: Vec2,
    /// The leg's perpendicular and the band's half-width.
    perp: Vec2,
    band: f32,
    nodes: Vec<DetourNode>,
    open: Vec<u16>,
    /// The points already tried while the current node is expanded.
    visited: Vec<Vec2>,
    tests: u32,
}

/// The PS2's "same point" (`FUN_00269148`: |d|^2 in XZ under 1e-8).
fn same_point(a: Vec2, b: Vec2) -> bool {
    (a - b).length_squared() < 1e-8
}

impl DetourSearch<'_> {
    /// The nearest barrier the segment `a -> b` crosses (grown by the leg's
    /// radius): the PS2's `FUN_00268c98`. OURS: nearest by where the segment
    /// enters it, ties to the lower index.
    fn nearest_crossed(&mut self, a: Vec2, b: Vec2) -> Option<usize> {
        self.tests += 1;
        let mut hit: Option<(f32, usize)> = None;
        // A barrier's circle clear of the segment's box is clear of the
        // segment: most of the map, for far less than the distance below.
        let (lo, hi) = cull_box(a, b);
        let first = self
            .cand
            .partition_point(|c| c.centre.x < lo.x - self.max_reach);
        for c in &self.cand[first..] {
            if c.centre.x > hi.x + self.max_reach {
                break;
            }
            if c.centre.x + c.reach < lo.x
                || c.centre.x - c.reach > hi.x
                || c.centre.y + c.reach < lo.y
                || c.centre.y - c.reach > hi.y
                || dist_to_segment(c.centre, a, b) > c.reach
            {
                continue;
            }
            let i = c.index as usize;
            if let Some(t) = self.g.barriers[i].cross_t_r(a, b, self.radius) {
                // Nearest; of equals the lowest index (`cand` is not in that order).
                if hit.is_none_or(|(h, j)| t < h || (t == h && i < j)) {
                    hit = Some((t, i));
                }
            }
        }
        hit.map(|(_, i)| i)
    }

    /// The PS2's expansion `FUN_002691a0(parent, point, depth)`: a `point`
    /// the parent sees becomes a node (or betters one); a point behind a
    /// barrier is replaced by that barrier's two silhouette corners, one
    /// level deeper.
    fn expand(&mut self, parent: usize, point: Vec2, depth: u32) {
        if depth >= DETOUR_MAX_DEPTH
            || self.tests >= DETOUR_MAX_TESTS
            || self.perp.dot(point - self.goal).abs() > self.band
            || self.visited.iter().any(|&v| same_point(v, point))
        {
            return;
        }
        self.visited.push(point);
        let from = self.nodes[parent].p;
        if let Some(i) = self.nearest_crossed(from, point) {
            let grown = self.radius + CORNER_CLEARANCE;
            let corners = self.g.barriers[i].silhouette(from, point, self.radius, grown);
            for c in corners.into_iter().flatten() {
                self.expand(parent, c, depth + 1);
            }
            return;
        }
        let cost = self.nodes[parent].g + (point - from).length();
        if let Some(n) = self.nodes.iter_mut().find(|n| same_point(n.p, point)) {
            // Open or closed, a cheaper way to it is taken (a closed node is
            // not opened again: the PS2's). A way through a node's own
            // descendant is never cheaper, so the chains stay trees.
            if cost < n.g {
                n.g = cost;
                n.parent = parent as u16;
            }
        } else if self.nodes.len() < DETOUR_MAX_NODES {
            self.open.push(self.nodes.len() as u16);
            self.nodes.push(DetourNode {
                p: point,
                g: cost,
                parent: parent as u16,
            });
        }
    }
}

impl NavGraph {
    /// Build the graph and its derived caches. `None` when there are no hubs
    /// (BF2 KAM1 ships an EMPTY `plan` chunk) — a graph with nothing to route
    /// on is the same as no graph at all.
    pub fn new(
        hubs: Vec<Hub>,
        arcs: Vec<Arc>,
        routes: RouteTable,
        barriers: Vec<Barrier>,
        hints: Vec<Hint>,
    ) -> Option<NavGraph> {
        if hubs.is_empty() {
            return None;
        }
        let n = hubs.len();
        // Every arc that touches a hub, in arc-index order. Out-of-range
        // endpoints are dropped (the discs have none; a corrupt bake must not
        // panic the sim).
        let mut adj: Vec<Vec<u16>> = vec![Vec::new(); n];
        for (i, arc) in arcs.iter().enumerate() {
            let (a, b) = (arc.a as usize, arc.b as usize);
            if a >= n || b >= n {
                continue;
            }
            adj[a].push(i as u16);
            if b != a {
                adj[b].push(i as u16);
            }
        }
        let mut g = NavGraph {
            hubs,
            arcs,
            routes,
            barriers,
            barrier_names: Vec::new(),
            hints,
            hub_discs: false,
            patches: None,
            comp: Vec::new(),
            adj,
        };
        g.comp = (0..TYPE_COUNT).map(|t| g.components(t)).collect();
        Some(g)
    }

    /// Flood the graph over the arcs `ty` may use; component ids are assigned
    /// in hub order, so they are stable for a given bake.
    ///
    /// JETLESS connectivity: [`Arc::JET_JUMP`] arcs are left out, so
    /// [`NavGraph::reachable`] answers for jetless movers of each type.
    /// PROVISIONAL: jet-only connections require a search, not this cache.
    fn components(&self, ty: usize) -> Vec<u32> {
        let n = self.hubs.len();
        let mut comp = vec![u32::MAX; n];
        let mut next = 0u32;
        let mut stack: Vec<u16> = Vec::new();
        for start in 0..n {
            if comp[start] != u32::MAX {
                continue;
            }
            comp[start] = next;
            stack.push(start as u16);
            while let Some(h) = stack.pop() {
                for &ai in &self.adj[h as usize] {
                    let arc = self.arcs[ai as usize];
                    if !arc.allows(Mover::new(ty, false)) {
                        continue;
                    }
                    let o = arc.other(h) as usize;
                    if comp[o] == u32::MAX {
                        comp[o] = next;
                        stack.push(o as u16);
                    }
                }
            }
            next += 1;
        }
        comp
    }

    /// Can `ty` get from `from` to `to` over the arcs at all, ignoring
    /// [`Arc::JET_JUMP`] arcs (see [`NavGraph::components`])? O(1).
    pub fn reachable(&self, from: u16, to: u16, ty: usize) -> bool {
        if from == to {
            return true;
        }
        match self.comp.get(ty) {
            Some(c) => match (c.get(from as usize), c.get(to as usize)) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
            None => false,
        }
    }

    /// Connectivity for this mover. Jetless connectivity uses the cache;
    /// only a jet-only connection needs the deterministic BFS fallback.
    pub(crate) fn reachable_by(&self, from: u16, to: u16, m: Mover) -> bool {
        self.reachable(from, to, m.ty)
            || (m.jet && self.route_bfs(from, to, m, &|_| true).is_some())
    }

    /// Whether the route table, as the mission's `plnp` patches left it, leads
    /// from `from` towards `to`: `FUN_002eb000` (B2:263410) with no arc
    /// barred answers a hop iff one of `from`'s rows holds a non-zero entry
    /// for `to`. `FUN_002ec380` (B2:264272) draws a Deathmatch hub only when
    /// it does, so a hub a blocked group cuts off (the doors spa6 one-flag
    /// locks with `BlockPlanningGraphArcs("connection54")`) is never drawn,
    /// though its arcs still join it. Without a table for this hub it falls
    /// back to [`NavGraph::reachable_by`], the port's BFS stand-in.
    pub(crate) fn routed_by(&self, from: u16, to: u16, m: Mover) -> bool {
        if from == to {
            return true;
        }
        if self.routes.row(m.ty, from as usize, 0).is_none() {
            return self.reachable_by(from, to, m);
        }
        self.routes.entries(m.ty, from, to).next().is_some()
    }

    pub fn hub(&self, i: u16) -> Option<&Hub> {
        self.hubs.get(i as usize)
    }

    /// Arrival radius of a hub, floored at [`crate::ai::ARRIVE_MIN_RADIUS`]'s
    /// caller — the raw authored radius (1–6 m on the discs, 11 hubs at 0).
    pub fn radius(&self, i: u16) -> f32 {
        self.hubs.get(i as usize).map_or(0.0, |h| h.radius)
    }

    /// `EnableBarriers(name)` / `DisableBarriers(name)`: open or close every
    /// barrier of that name hash (BF1 `FUN_001923a0`, BF2 `FUN_002f0650` →
    /// `FUN_002ee330` / `FUN_002ee300`, RE_LOG "Barriers are the PS2's
    /// Obstacles"). Returns how many it switched.
    pub fn set_barriers(&mut self, name: u32, enabled: bool) -> usize {
        let mut n = 0;
        for (b, &h) in self.barriers.iter_mut().zip(&self.barrier_names) {
            if h == name {
                b.enabled = enabled;
                n += 1;
            }
        }
        n
    }

    /// Where `pos` is on the graph for the mover, as the PS2 locates (BF1
    /// `FUN_00266678`, BF2 `FUN_002eb918`; RE_LOG "Arrival and locate" and
    /// its audit of 2026-10-03, held against 1,004 live path objects - card
    /// LOC3, `game/tests/locate_ps2.rs`):
    ///
    /// 1. the CONTAINING hub with the smallest radius (ties → lowest index)
    ///    among the hubs with an arc for the mover - a SPHERE in BF1, an XZ
    ///    disc in BF2 ([`NavGraph::hub_discs`]);
    /// 2. else the arc whose corridor contains the point, in 3-D - the
    ///    projection on the segment within its ends and the distance to it
    ///    < `rB·t + rA·(1−t)` - with the smallest `rA + rB` (ties → lowest
    ///    index); BF2 skips an arc with any of flag bits 0..2;
    /// 3. else the nearest arc, by the 3-D distance left outside its
    ///    corridor (beyond an end: outside that end's sphere), floored at 0;
    ///    BF2 skips an arc with flag bit 0 (BF1 `FUN_00266840`, BF2
    ///    `FUN_002ebb48`).
    ///
    /// `None` only on an empty graph, which [`NavGraph::new`] rejects.
    ///
    /// Not carried: BF2's bound circle / sphere per arc (a prune; it holds
    /// the corridor). OURS: [`Arc::allows`] also drops a jet leap for a
    /// jetless mover in steps 1 and 3 (BF2 Mustafar has 2 hubs whose only
    /// soldier arcs are jet leaps), and a type with no arc anywhere - the
    /// PS2 locates nothing for it - keeps every hub and answers the nearest.
    pub fn locate(&self, pos: Vec3, m: Mover) -> Option<Locus> {
        self.locate_on(pos, m, &|_| true)
    }

    /// [`NavGraph::locate`] with a caller's SAME-LEVEL preference on the
    /// containing hubs: the smallest containing hub that passes `on_floor`
    /// wins, and only if none passes does the plain smallest-radius answer
    /// stand. OURS (PLAN "PHASE A.2 DESIGN" decision 3); the PS2's is
    /// `&|_| true`. Steps 2 and 3 are untouched.
    pub fn locate_on(&self, pos: Vec3, m: Mover, on_floor: &dyn Fn(u16) -> bool) -> Option<Locus> {
        // The PS2 starts both "smallest" searches at 10000.
        const START: f32 = 10000.0;
        if let Some(i) = self.containing_hub(
            m,
            on_floor,
            |r| r >= START,
            |h| {
                let d = h.pos - pos;
                let d2 = if self.hub_discs {
                    d.x * d.x + d.z * d.z
                } else {
                    d.length_squared()
                };
                // (A NaN is outside, as the PS2's compare has it.)
                let inside = d2 < h.radius * h.radius;
                !inside
            },
        ) {
            return Some(Locus::Hub(i));
        }
        let mut best: Option<(f32, u16, f32)> = None;
        for (i, arc) in self.arcs.iter().enumerate() {
            if arc.flags & 7 != 0 || arc.type_mask & (1u32 << m.ty) == 0 {
                continue;
            }
            let (Some(ha), Some(hb)) = (self.hub(arc.a), self.hub(arc.b)) else {
                continue;
            };
            let rr = ha.radius + hb.radius;
            let smaller = rr < best.map_or(START, |b| b.0);
            if !smaller {
                continue;
            }
            if let Some((t, out)) = arc_reach(ha, hb, pos) {
                if (0.0..=1.0).contains(&t) && out < 0.0 {
                    best = Some((rr, i as u16, t));
                }
            }
        }
        if let Some((_, i, t)) = best {
            return Some(Locus::Arc(i, t));
        }
        let mut near: Option<(f32, u16, f32)> = None;
        for (i, arc) in self.arcs.iter().enumerate() {
            if !arc.allows(m) || arc.flags & Arc::ONE_WAY != 0 {
                continue;
            }
            let (Some(ha), Some(hb)) = (self.hub(arc.a), self.hub(arc.b)) else {
                continue;
            };
            if let Some((t, out)) = arc_reach(ha, hb, pos) {
                let d = out.max(0.0);
                if near.is_none_or(|(bd, _, _)| d < bd) {
                    near = Some((d, i as u16, t.clamp(0.0, 1.0)));
                }
            }
        }
        if let Some((_, i, t)) = near {
            return Some(Locus::Nearest(i, t));
        }
        // OURS: a type with no arc it can be located on - the nearest hub.
        self.nearest_hub(Vec2::new(pos.x, pos.z))
    }

    /// Where a UNIT stands - a route's start, a hunt's quarry, a vehicle to
    /// board, an order's goal - OURS, a named deviation
    /// (`analysis/bot-design.md` D17, card LOC3): everything in XZ (a hub is
    /// a disc in both games), the FIRST containing corridor, then the
    /// nearest arc by the XZ distance left outside its corridor, with the
    /// caller's same-level preference on the containing hubs. The PS2 asks
    /// [`NavGraph::locate`] for a unit as for a goal, and starts a route on
    /// an arc from the end its draw runs along, else the farther end.
    /// Measured over 32 seeds x 32 maps against this: the PS2's (no level
    /// rule) 1,315 recycles against 838, two rounds that never ended and 12
    /// maps worse beyond seed noise (BF2 Utapau kills / min 27.8 -> 16.9,
    /// BF1 Bespin Platforms 30.0 -> 19.7), one better (BF2 Dagobah, recycles
    /// per round 2.50 -> 0.25) - the port's walk does not yet get a bot off
    /// the spots a 3-D start sends it across. PROVISIONAL until the mover
    /// cards (MOVR, OBST, LLEG) land and the measurement is run again (card
    /// LOC3B).
    pub fn locate_unit_on(
        &self,
        pos: Vec3,
        m: Mover,
        on_floor: &dyn Fn(u16) -> bool,
    ) -> Option<Locus> {
        let p = Vec2::new(pos.x, pos.z);
        if let Some(i) = self.containing_hub(
            m,
            on_floor,
            |r| r <= 0.0,
            |h| (h.xz() - p).length_squared() >= h.radius * h.radius,
        ) {
            return Some(Locus::Hub(i));
        }
        let mut near: Option<(f32, u16, f32)> = None;
        for (i, arc) in self.arcs.iter().enumerate() {
            if !arc.allows(m) {
                continue;
            }
            let (Some(ha), Some(hb)) = (self.hub(arc.a), self.hub(arc.b)) else {
                continue;
            };
            let (a, b) = (ha.xz(), hb.xz());
            let ab = b - a;
            let len2 = ab.length_squared();
            let t = if len2 > 1e-9 {
                ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = (p - (a + ab * t)).length() - (ha.radius * (1.0 - t) + hb.radius * t);
            if d < 0.0 {
                return Some(Locus::Arc(i as u16, t));
            }
            if near.is_none_or(|(bd, _, _)| d < bd) {
                near = Some((d, i as u16, t));
            }
        }
        if let Some((_, i, t)) = near {
            return Some(Locus::Nearest(i, t));
        }
        self.nearest_hub(p)
    }

    /// The hub of a point where a unit stands: [`NavGraph::locate_unit_on`]'s
    /// hub, or the nearer end of its arc.
    pub fn unit_hub_on(&self, pos: Vec3, m: Mover, on_floor: &dyn Fn(u16) -> bool) -> Option<u16> {
        Some(self.locus_hub(self.locate_unit_on(pos, m, on_floor)?))
    }

    /// ONE hub for `pos`: its own hub, else the NEARER end of its arc (its
    /// corridor's or the nearest) - what the PS2 falls back to for a goal on
    /// an arc when the route draw does not reach an end first (BF1
    /// `FUN_00266e50`, 0x26716c). A post's hub; a point where a unit stands
    /// is [`NavGraph::unit_hub_on`]'s.
    pub fn hub_for(&self, pos: Vec3, m: Mover) -> Option<u16> {
        self.hub_for_on(pos, m, &|_| true)
    }

    /// [`NavGraph::hub_for`] with the same-level preference — see
    /// [`NavGraph::locate_on`].
    pub fn hub_for_on(&self, pos: Vec3, m: Mover, on_floor: &dyn Fn(u16) -> bool) -> Option<u16> {
        Some(self.locus_hub(self.locate_on(pos, m, on_floor)?))
    }

    fn containing_hub(
        &self,
        m: Mover,
        on_floor: &dyn Fn(u16) -> bool,
        skip_radius: impl Fn(f32) -> bool,
        outside: impl Fn(&Hub) -> bool,
    ) -> Option<u16> {
        // A hub with no arc for the mover is not a hub for it: at load the
        // PS2 ORs each hub's arcs' type masks into the hub (BF1
        // `FUN_002663a8`, BF2 `FUN_002eb2b0`) and its locate skips a hub
        // whose mask lacks the mover's bit - no soldier exception (card
        // HUBM). TAT1's 525 m dune-sea hub has no arc, BF2 Hoth's AT-AT
        // stands inside a soldier hub laid over its own path.
        let typed = self.arcs.iter().any(|a| a.allows(m));
        let serves = |h: &Hub| {
            !typed
                || h.arcs
                    .iter()
                    .any(|&j| self.arcs.get(j as usize).is_some_and(|a| a.allows(m)))
        };
        let mut best_hub: Option<(f32, u16)> = None;
        let mut best_level: Option<(f32, u16)> = None;
        for (i, h) in self.hubs.iter().enumerate() {
            let r = h.radius;
            if skip_radius(r) || !serves(h) || outside(h) {
                continue;
            }
            if best_hub.is_none_or(|(br, _)| r < br) {
                best_hub = Some((r, i as u16));
            }
            if best_level.is_none_or(|(br, _)| r < br) && on_floor(i as u16) {
                best_level = Some((r, i as u16));
            }
        }
        best_level.or(best_hub).map(|(_, i)| i)
    }

    fn nearest_hub(&self, p: Vec2) -> Option<Locus> {
        let mut hub: Option<(f32, u16)> = None;
        for (i, h) in self.hubs.iter().enumerate() {
            let d2 = (h.xz() - p).length_squared();
            if hub.is_none_or(|(bd, _)| d2 < bd) {
                hub = Some((d2, i as u16));
            }
        }
        hub.map(|(_, i)| Locus::Hub(i))
    }

    fn locus_hub(&self, locus: Locus) -> u16 {
        match locus {
            Locus::Hub(i) => i,
            Locus::Arc(j, t) | Locus::Nearest(j, t) => {
                let arc = self.arcs[j as usize];
                if t < 0.5 {
                    arc.a
                } else {
                    arc.b
                }
            }
        }
    }

    /// The PS2 chooser (`FUN_00266150` / `FUN_002eb000`): pick the arc out of
    /// `cur` toward `dest` for type `ty`, given the arc the unit ARRIVED on
    /// and a per-unit random word `r`.
    ///
    /// - alternatives = the non-zero entries for `dest` in `rows[ty][cur]`;
    /// - if the arrival arc is one of them, `budget = 31 − its weight`
    ///   (weights of one entry sum to 31), else 31;
    /// - `budget < 1` means the arrival arc is the only way out: pick another
    ///   arc of the hub whose mask allows `ty` (`FUN_002659e8` picks it at
    ///   random; ours is `r % n`), `None` when there is no other;
    /// - else `k = r % budget`, walk the alternatives in row order subtracting
    ///   weights (the arrival slot is skipped — its weight is what `budget`
    ///   already took out) and take the first at which the running budget
    ///   reaches ≤ 0.
    ///
    /// Returns `(arc, next hub)`. `None` = the table has nothing here, which
    /// is the port's cue to try [`NavGraph::route_bfs`].
    ///
    /// Phase A.2 adds two more BARRED slots to the arrival arc's, and takes
    /// their weights out of the budget the same way the arrival arc's is (with
    /// only the arrival arc barred the arithmetic is byte-for-byte the PS2's):
    ///
    /// - an arc `m` may not walk at all — a [`Arc::JET_JUMP`] without a jet;
    /// - `exclude`, the arc a caller has just REJECTED. That is the leg whose
    ///   waypoint turned out to have no floor at the unit's level, re-asked of
    ///   the chooser with the bad arc off the table (PLAN "PHASE A.2 DESIGN"
    ///   decision 3).
    pub fn next_hop(
        &self,
        cur: u16,
        dest: u16,
        m: Mover,
        arrival_arc: Option<u16>,
        exclude: Option<u16>,
        r: u32,
    ) -> Option<Hop> {
        let hub = self.hub(cur)?;
        if cur == dest {
            return None;
        }
        let arrival_slot = arrival_arc.and_then(|a| hub.arcs.iter().position(|&x| x == a));
        let barred = |slot: usize| -> bool {
            if Some(slot) == arrival_slot {
                return true;
            }
            match hub.arcs.get(slot) {
                Some(&a) => match self.arcs.get(a as usize) {
                    Some(arc) => Some(a) == exclude || !arc.allows(m),
                    None => true,
                },
                None => true,
            }
        };
        let mut budget = 31i32;
        for e in self.routes.entries(m.ty, cur, dest) {
            if barred((e & 7) as usize) {
                budget -= i32::from(e >> 3);
            }
        }
        if budget < 1 {
            // Dead end: the PS2 leaves by any other arc of this hub.
            return self.any_other_arc(cur, arrival_arc, exclude, m, r);
        }
        let mut running = (r % budget as u32) as i32;
        for e in self.routes.entries(m.ty, cur, dest) {
            let slot = (e & 7) as usize;
            if barred(slot) {
                continue;
            }
            running -= i32::from(e >> 3);
            if running <= 0 {
                let arc = *hub.arcs.get(slot)?;
                let a = self.arcs.get(arc as usize)?;
                if !a.touches(cur) {
                    return None;
                }
                return Some((arc, a.other(cur)));
            }
        }
        None
    }

    /// The PS2's "leave by any other arc" (`FUN_002659e8`): an arc of `cur`
    /// this mover may take that is neither the one it arrived on nor one the
    /// caller has excluded, picked with the unit's random word (`r % n`; the
    /// disc's own pick is random too).
    ///
    /// [`NavGraph::next_hop`] reaches it when the routing budget runs out, and
    /// the follower asks for it directly as the LAST alternative to a leg
    /// whose waypoint has no floor at the unit's level (PLAN "PHASE A.2
    /// DESIGN" decision 3).
    pub fn any_other_arc(
        &self,
        cur: u16,
        arrival_arc: Option<u16>,
        exclude: Option<u16>,
        m: Mover,
        r: u32,
    ) -> Option<Hop> {
        let hub = self.hub(cur)?;
        // Two passes over the eight arc slots — count, then take the
        // `r % n`-th — where this collected the survivors into a Vec first.
        // Same list, same order, same pick, no allocation.
        let takeable = |a: u16| -> bool {
            Some(a) != arrival_arc
                && Some(a) != exclude
                && self
                    .arcs
                    .get(a as usize)
                    .is_some_and(|x| x.allows(m) && x.touches(cur))
        };
        let n = hub.arcs.iter().filter(|&&a| takeable(a)).count();
        if n == 0 {
            return None;
        }
        let pick = *hub
            .arcs
            .iter()
            .filter(|&&a| takeable(a))
            .nth(r as usize % n)?;
        Some((pick, self.arcs[pick as usize].other(cur)))
    }

    /// PROVISIONAL, OURS — the PS2 has no graph search. Breadth-first over the
    /// arcs whose mask has bit `ty`, expanding each hub's arcs in ARC-INDEX
    /// order so ties are deterministic; returns the FIRST hop of a shortest
    /// (fewest-arcs) path from `cur` to `dest`.
    pub fn route_bfs(
        &self,
        cur: u16,
        dest: u16,
        m: Mover,
        on_floor: &dyn Fn(u16) -> bool,
    ) -> Option<Hop> {
        self.route_bfs_avoiding(cur, dest, m, on_floor, &|_| false)
    }

    /// [`NavGraph::route_bfs`] over the arcs `shun` does not name (card
    /// ARCM: the arcs a bot or its team found it cannot walk).
    pub fn route_bfs_avoiding(
        &self,
        cur: u16,
        dest: u16,
        m: Mover,
        on_floor: &dyn Fn(u16) -> bool,
        shun: &dyn Fn(u16) -> bool,
    ) -> Option<Hop> {
        if cur == dest || self.hub(cur).is_none() || self.hub(dest).is_none() {
            return None;
        }
        // The component cache excludes jet arcs; it cannot reject a jet mover.
        if !m.jet && !self.reachable(cur, dest, m.ty) {
            return None;
        }
        let mut first: Vec<Option<Hop>> = vec![None; self.hubs.len()];
        let mut seen = vec![false; self.hubs.len()];
        let mut queue: std::collections::VecDeque<u16> = std::collections::VecDeque::new();
        seen[cur as usize] = true;
        queue.push_back(cur);
        while let Some(h) = queue.pop_front() {
            for &ai in &self.adj[h as usize] {
                let arc = self.arcs[ai as usize];
                if !arc.allows(m) || shun(ai) {
                    continue;
                }
                let o = arc.other(h);
                if seen[o as usize] {
                    continue;
                }
                // Every hub on the path has to be somewhere this unit can
                // stand — `on_floor` is the caller's "there is ground at MY
                // level under it" (PLAN "PHASE A.2 DESIGN" decision 3). The
                // DESTINATION is exempt: it is the goal, and the last leg
                // walks at the post itself, not at the hub.
                if o != dest && !on_floor(o) {
                    continue;
                }
                seen[o as usize] = true;
                first[o as usize] = if h == cur {
                    Some((ai, o))
                } else {
                    first[h as usize]
                };
                if o == dest {
                    return first[o as usize];
                }
                queue.push_back(o);
            }
        }
        None
    }

    /// PROVISIONAL, OURS — the hop DISTANCE FIELD from `from`: `d[h]` is the
    /// fewest arcs between `from` and hub `h` over the arcs `m` may walk, and
    /// [`u16::MAX`] where no such path exists. Same expansion as
    /// [`NavGraph::route_bfs`] (each hub's arcs in ARC-INDEX order, so ties
    /// are deterministic), one pass instead of one per query — the commander
    /// asks "how far is every unit from this post" once per plan, which is
    /// eight of these instead of `units x posts` route searches.
    ///
    /// Unlike `route_bfs` this takes NO `on_floor` filter: it is a coarse
    /// ranking input, and the per-unit level test belongs to the leg the
    /// follower actually plans (`core::ai::plan_leg`). A hub the field says is
    /// 4 hops away may still be unwalkable for one unit on one storey; that
    /// unit's route will say so and the next plan drops the objective.
    /// APPENDS the field to `out` (one `u16` per hub, `hubs.len()` of them)
    /// and returns nothing: the caller keeps the base offset. `queue` is a
    /// caller-owned scratch buffer, cleared here, so a caller asking for one
    /// field per command post allocates twice per plan instead of twice per
    /// post. It is a plain `Vec` walked with a head cursor — the same FIFO
    /// order a `VecDeque` gives, which is what makes the expansion order
    /// (`self.adj[h]`, arc-index order) deterministic.
    pub fn hop_distances_into(
        &self,
        from: u16,
        m: Mover,
        out: &mut Vec<u16>,
        queue: &mut Vec<u16>,
    ) {
        let base = out.len();
        out.resize(base + self.hubs.len(), u16::MAX);
        if self.hub(from).is_none() {
            return;
        }
        out[base + from as usize] = 0;
        queue.clear();
        queue.push(from);
        let mut head = 0usize;
        while head < queue.len() {
            let h = queue[head];
            head += 1;
            let next = out[base + h as usize].saturating_add(1);
            for &ai in &self.adj[h as usize] {
                let arc = self.arcs[ai as usize];
                if !arc.allows(m) {
                    continue;
                }
                let o = arc.other(h);
                if out[base + o as usize] != u16::MAX {
                    continue;
                }
                out[base + o as usize] = next;
                queue.push(o);
            }
        }
    }

    /// How many arcs of this type carry each PROVISIONAL BF2 `flags` bit that
    /// routing cares about: `(jump, jet_jump)`. The second is the number of
    /// arcs Phase A.2 takes OFF a jetless soldier's graph, which is what the
    /// sweep reports per map; the first is kept and is reported beside it so
    /// the two are never confused.
    pub fn arc_flag_census(&self, ty: usize) -> (usize, usize) {
        let bit = 1u32 << ty;
        let mut jump = 0;
        let mut jet = 0;
        for a in &self.arcs {
            if a.type_mask & bit == 0 {
                continue;
            }
            if a.flags & Arc::JUMP != 0 {
                jump += 1;
            }
            if a.flags & Arc::JET_JUMP != 0 {
                jet += 1;
            }
        }
        (jump, jet)
    }

    /// The barriers this leg must PRETEND are not there: the ones whose
    /// `radius`-expanded rectangle already contains an end point. Twenty hubs
    /// on the discs are authored inside a SOLDIER barrier (RE_LOG "dead
    /// hubs"), and without this a unit routed to one of them can never find a
    /// clear leg at all. In barrier-index order.
    pub(crate) fn leg_ignores(&self, a: Vec2, b: Vec2, radius: f32, ty: usize) -> Vec<u16> {
        let mut out = Vec::new();
        for (i, br) in self.barriers.iter().enumerate() {
            if br.blocks(ty) && (br.contains_r(a, radius) || br.contains_r(b, radius)) {
                out.push(i as u16);
            }
        }
        out
    }

    /// The first barrier (in index order) that the XZ segment `a → b` crosses
    /// with a `radius` margin, skipping `ignore` and anything that does not
    /// block `ty`.
    fn first_crossed(
        &self,
        a: Vec2,
        b: Vec2,
        radius: f32,
        ty: usize,
        ignore: &[u16],
    ) -> Option<usize> {
        (0..self.barriers.len()).find(|&i| {
            let br = &self.barriers[i];
            br.blocks(ty) && !ignore.contains(&(i as u16)) && br.crosses_r(a, b, radius)
        })
    }

    /// How far along `a → b` (0..1) a walker gets before the first barrier
    /// that stops `ty`: 1.0 when none is in the way. A barrier `a` stands in
    /// does not count (card BNDS, `Sim::level_at`).
    pub fn barrier_stop(&self, a: Vec2, b: Vec2, ty: usize) -> f32 {
        let mut t = 1.0f32;
        // A rectangle's corners are its half sizes along two unit axes from
        // its centre, so no part of it lies further along X or Z than their
        // sum: clear of the segment's box, it is clear of the segment.
        let (lo, hi) = cull_box(a, b);
        for br in &self.barriers {
            if !br.blocks(ty) {
                continue;
            }
            let bound = br.half.x.abs() + br.half.y.abs();
            if br.centre.x + bound < lo.x
                || br.centre.x - bound > hi.x
                || br.centre.y + bound < lo.y
                || br.centre.y - bound > hi.y
            {
                continue;
            }
            if let Some(e) = br.entry_t(a, b) {
                t = t.min(e);
            }
        }
        t
    }

    /// Does the straight leg `a → b` (XZ, with a `radius` margin) cross a
    /// barrier that stops `ty`? `Some(index)` is the first such barrier in
    /// index order.
    ///
    /// This is the PS2's `FUN_00268f68` — the test the leg builder
    /// (`FUN_0026c0d0`) runs before it hands a two-point path to the mover.
    /// A barrier that CONTAINS either end is ignored for the leg, so a hub
    /// authored inside a barrier cannot deadlock it.
    pub fn leg_crosses_barrier(&self, a: Vec2, b: Vec2, radius: f32, ty: usize) -> Option<usize> {
        let ignore = self.leg_ignores(a, b, radius, ty);
        self.first_crossed(a, b, radius, ty, &ignore)
    }

    /// Walk `a → b` around the barriers that stop `ty`: the INTERMEDIATE
    /// waypoints of a corner path, empty when the straight leg is already
    /// clear, `None` when no chain of corners gets there.
    ///
    /// The PS2's local planner (`FUN_00269690` / `FUN_002691a0` /
    /// `FUN_00269ac8`, RE_LOG "Barriers are the PS2's Obstacles"), in its own
    /// LAZY shape (card OBST2): an A\* from the start whose best node is
    /// expanded toward the goal — a point the node sees (no barrier on the
    /// segment, tested with `radius`) becomes a node, a point behind a
    /// barrier is replaced by the two SILHOUETTE CORNERS of that barrier
    /// grown by `radius + `[`CORNER_CLEARANCE`], each tried the same way one
    /// level deeper — inside a band of `max(`[`DETOUR_BAND_MIN`]`, |b − a|)`
    /// perpendicular to the leg, to a depth (the node's chain plus the
    /// levels) under [`DETOUR_MAX_DEPTH`] and [`DETOUR_MAX_NODES`] nodes.
    /// So the corners in play are those of the barriers something actually
    /// crosses, however many stand in the band.
    ///
    /// (Until card OBST2 the port listed every corner of the first 24
    /// barriers of the band, in index order, up front. On the discs the band
    /// of a crossing leg holds 30-60 barriers at the median and up to 421,
    /// so the barrier a leg crossed was often not among them: 22,268 of
    /// 68,186 crossing legs in 64 rounds came back "the straight leg is
    /// clear" and were walked through the barrier.)
    ///
    /// Ours, PROVISIONAL: the nearest crossed barrier is the one the segment
    /// enters first; ties in the open list go to the node made first; the
    /// work is capped by [`DETOUR_MAX_TESTS`]. The answer is a pure function
    /// of the graph and the two points.
    pub fn detour(&self, a: Vec2, b: Vec2, radius: f32, ty: usize) -> Option<Vec<Vec2>> {
        self.detour_culled(a, b, radius, ty, true)
    }

    /// [`Self::detour`], `cull`ing the barriers it cannot meet (only a test
    /// turns that off, to prove it changes no answer).
    fn detour_culled(
        &self,
        a: Vec2,
        b: Vec2,
        radius: f32,
        ty: usize,
        cull: bool,
    ) -> Option<Vec<Vec2>> {
        let ignore = self.leg_ignores(a, b, radius, ty);
        if self.first_crossed(a, b, radius, ty, &ignore).is_none() {
            return Some(Vec::new()); // the straight leg is clear
        }
        let leg = b - a;
        let len = leg.length();
        // The PS2's band test: the perpendicular of the leg, measured from the
        // goal (`DAT_00588a50` / `DAT_00588a60`).
        let perp = if len > 1e-6 {
            Vec2::new(leg.y, -leg.x) / len
        } else {
            Vec2::X
        };
        let band = len.max(DETOUR_BAND_MIN);
        // Every segment tried joins points inside the band, so a barrier whose
        // circle lies wholly outside it cannot be crossed. (A leg that is not
        // finite has no band to speak of: see `cull_box`.)
        let finite = a.is_finite() && b.is_finite();
        let mut cand: Vec<DetourCand> = (0..self.barriers.len() as u16)
            .filter(|i| self.barriers[*i as usize].blocks(ty) && !ignore.contains(i))
            .map(|index| {
                let br = &self.barriers[index as usize];
                DetourCand {
                    centre: br.centre,
                    reach: if cull {
                        br.bound_radius() + radius * std::f32::consts::SQRT_2
                    } else {
                        f32::INFINITY
                    },
                    index,
                }
            })
            .filter(|c| {
                !finite || perp.dot(c.centre - b).abs() <= band + c.reach + DETOUR_CULL_SLACK
            })
            .collect();
        cand.sort_unstable_by(|p, q| p.centre.x.total_cmp(&q.centre.x));
        let max_reach = cand.iter().fold(0.0, |m: f32, c| m.max(c.reach));
        let mut s = DetourSearch {
            g: self,
            cand,
            max_reach,
            radius,
            goal: b,
            perp,
            band,
            nodes: vec![DetourNode {
                p: a,
                g: 0.0,
                parent: u16::MAX,
            }],
            open: vec![0],
            visited: Vec::new(),
            tests: 0,
        };
        while !s.open.is_empty() && s.tests < DETOUR_MAX_TESTS {
            // Best f = g + h, ties to the LOWEST node index. The open list is
            // a plain Vec scanned linearly: deterministic.
            let mut at = 0usize;
            let mut best = (f32::INFINITY, u16::MAX);
            for (k, &i) in s.open.iter().enumerate() {
                let n = &s.nodes[i as usize];
                let f = (n.g + (n.p - b).length(), i);
                if f < best {
                    best = f;
                    at = k;
                }
            }
            let cur = s.open.swap_remove(at) as usize;
            // The chain from the start to this node, itself included.
            let mut chain = Vec::new();
            let mut p = cur as u16;
            while p != u16::MAX && chain.len() <= s.nodes.len() {
                chain.push(s.nodes[p as usize].p);
                p = s.nodes[p as usize].parent;
            }
            if cur != 0 && same_point(s.nodes[cur].p, b) {
                // Reached the goal: the chain without its two ends.
                chain.reverse();
                return Some(chain[1..chain.len() - 1].to_vec());
            }
            s.visited.clear();
            s.expand(cur, b, chain.len() as u32);
        }
        None
    }

    /// Arcs that a `ty` barrier crosses — a data check for the diagnostics
    /// (design decision 4: the authored corridors do not cross a barrier that
    /// stops the type, and ROUND_VERBOSE says so when they do).
    pub fn arcs_crossing_barriers(&self, ty: usize) -> usize {
        let bit = 1u32 << ty;
        self.arcs
            .iter()
            .filter(|arc| {
                if arc.type_mask & bit == 0 {
                    return false;
                }
                let (Some(a), Some(b)) = (self.hub(arc.a), self.hub(arc.b)) else {
                    return false;
                };
                self.barriers
                    .iter()
                    .any(|br| br.stops(ty) && br.crosses(a.xz(), b.xz()))
            })
            .count()
    }
}

/// `p` against the arc from hub `a` to hub `b`, in 3-D: `t` of its projection
/// on the segment (unclamped) and the distance left OUTSIDE the corridor -
/// within the ends, the distance to the segment less `rB·t + rA·(1−t)`;
/// beyond an end, the distance to that end's centre less its radius
/// (negative = inside). `None` for an arc of no length (the PS2's divide
/// gives a NaN that fails every test). BF1 `FUN_00265d90` / `FUN_00265eb8`,
/// BF2 `FUN_002ea908` / `FUN_002eaae8`.
fn arc_reach(a: &Hub, b: &Hub, p: Vec3) -> Option<(f32, f32)> {
    let ab = b.pos - a.pos;
    let den = ab.length_squared();
    if den == 0.0 {
        return None;
    }
    let t = ab.dot(p - a.pos) / den;
    if !(0.0..=1.0).contains(&t) {
        let end = if t >= 0.0 { b } else { a };
        return Some((t, (p - end.pos).length() - end.radius));
    }
    let d = (p - (a.pos + ab * t)).length();
    Some((t, d - (b.radius * t + a.radius * (1.0 - t))))
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 1e-9 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).length()
}

/// XZ distance between two world points, used by the follower's horizontal
/// geometry; goal locate separately uses 3-D arc reach.
pub fn xz_dist(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

#[cfg(test)]
mod tests;
