//! Card TEAM (`analysis/bot-design.md` 4.1, 4.3) — what a team knows
//! together. Built so far: the UNIT GRID, the one answer to "who is within
//! r of p" for the thinks (`perception::scan`'s look and listen and
//! `perception::tell`, the gunner's friend-in-line test, the follow gap),
//! which each walked every unit; and the FOLLOW GAP ([`follow_ease`]). The
//! contacts a friend passes on are `perception::tell`'s (every tier): a TOLD
//! record is aimed at by a bot that is not walking a leg (on a ground round a
//! walking bot faces its way, playtest C3); it keeps nobody on a position
//! (`ai::behave`) and never overwrites the bot's own record. Fire lanes: the gunner's friend-in-line
//! test clears every shot's line by a tick of walking and its flight
//! (`gunner_foot::shot_late`). The TEAM BOARD ([`board`]) gives the roles
//! inside a post's group: COVER (while a friend captures, up to two others
//! guard positions facing the approach; Hard and Elite) and FLANK (card
//! FLNK: the match's share of a post pair's attackers walks the pair's flank
//! route, `nav::flank`; every tier) - Overwatch is the sniper's
//! spot (`ai::snipe`, card SNPR), counted beside them in the sweep's `roles`
//! and `flank` lines. Not yet: the Gunner role (card TURR) and the lanes
//! held on the board (the Tactician asks each friend's line itself).
//!
//! The grid is [`CELL`] m cells over the XZ plane, hashed into [`BUCKETS`]
//! buckets: the alive units are sorted by bucket with a counting sort into
//! buffers sized by the unit slots (no allocation in a tick once they are
//! grown). It is built in `Sim::step` just before the think loop and cleared
//! just after it — inside that window the sim is borrowed immutably, so the
//! feet it holds are every unit's feet. Outside it ([`UnitGrid::live`]
//! false: a test that calls a think by hand, the commander) the same answer
//! comes from the plain loop over the units ([`Sim::units_within`]). Both
//! answer in unit-index order, the order the loops they replace walked, so
//! the grid moves no draw and no hash. DERIVED, never hashed.
//!
//! A query whose box covers more cells than there are alive units walks the
//! alive units instead: at today's 33 units a 49 m sight is 169 cells.

use crate::sim::Sim;
use crate::unit::Unit;
use glam::{Vec2, Vec3};

/// The grid's cell (m): the bot design's 4.1.
pub const CELL: f32 = 8.0;
/// Hash buckets the cells share (a power of two).
pub const BUCKETS: usize = 256;

/// The unit grid (module header).
#[derive(Debug, Default, Clone)]
pub struct UnitGrid {
    live: bool,
    /// Per unit slot: its feet (XZ) when the grid was built.
    feet: Vec<Vec2>,
    /// Per unit slot: its bucket, or `u32::MAX` when not alive.
    bucket: Vec<u32>,
    /// `items[start[b]..start[b + 1]]` = the alive units in bucket `b`,
    /// ascending.
    start: Vec<u32>,
    items: Vec<u32>,
}

fn cell_of(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

fn bucket_of(cx: i32, cz: i32) -> usize {
    let h = (cx as u32).wrapping_mul(0x9E37_79B1) ^ (cz as u32).wrapping_mul(0x85EB_CA77);
    (h.wrapping_mul(0xC2B2_AE35) >> (32 - BUCKETS.trailing_zeros())) as usize
}

/// The one distance test both answers use: feet within `r` of `p`, XZ.
fn inside(feet: Vec2, p: Vec2, r: f32) -> bool {
    (feet - p).length_squared() <= r * r
}

impl UnitGrid {
    /// Sort the alive units by cell (a counting sort, unit-index order kept
    /// within a bucket) and open the grid for queries.
    pub fn build(&mut self, units: &[Unit]) {
        let n = units.len();
        self.feet.clear();
        self.bucket.clear();
        self.start.clear();
        self.start.resize(BUCKETS + 1, 0);
        for u in units {
            let f = Vec2::new(u.soldier.pos.x, u.soldier.pos.z);
            self.feet.push(f);
            let b = if u.alive {
                let b = bucket_of(cell_of(f.x), cell_of(f.y));
                self.start[b + 1] += 1;
                b as u32
            } else {
                u32::MAX
            };
            self.bucket.push(b);
        }
        for b in 0..BUCKETS {
            self.start[b + 1] += self.start[b];
        }
        let alive = self.start[BUCKETS] as usize;
        self.items.clear();
        self.items.resize(alive, 0);
        let mut fill: [u32; BUCKETS] = [0; BUCKETS];
        fill.copy_from_slice(&self.start[..BUCKETS]);
        for i in 0..n {
            let b = self.bucket[i];
            if b != u32::MAX {
                self.items[fill[b as usize] as usize] = i as u32;
                fill[b as usize] += 1;
            }
        }
        self.live = true;
    }

    /// Close the grid: the units may move after this.
    pub fn clear(&mut self) {
        self.live = false;
    }

    /// Is the grid's answer the sim's (built this think window)?
    pub fn live(&self) -> bool {
        self.live
    }

    /// Every unit the grid holds with its feet within `r` of `p` (XZ), in
    /// ascending index, into `out` (cleared first).
    pub fn within(&self, p: Vec3, r: f32, out: &mut Vec<u32>) {
        out.clear();
        let q = Vec2::new(p.x, p.z);
        let alive = self.items.len();
        // The box's cells, counted in floats: an endless radius (a missile's
        // line, `gunner_foot`) or a NaN walks the units, never 2^64 cells.
        let span = 2.0 * r / CELL + 2.0;
        if !(r >= 0.0 && span * span < alive.min(BUCKETS) as f32) {
            // Wider than the crowd (or than every bucket): walk the units.
            for (i, &b) in self.bucket.iter().enumerate() {
                if b != u32::MAX && inside(self.feet[i], q, r) {
                    out.push(i as u32);
                }
            }
            return;
        }
        let (x0, x1) = (cell_of(q.x - r), cell_of(q.x + r));
        let (z0, z1) = (cell_of(q.y - r), cell_of(q.y + r));
        let mut seen = [0u64; BUCKETS / 64];
        for cx in x0..=x1 {
            for cz in z0..=z1 {
                let b = bucket_of(cx, cz);
                if seen[b / 64] >> (b % 64) & 1 != 0 {
                    continue;
                }
                seen[b / 64] |= 1 << (b % 64);
                let (s, e) = (self.start[b] as usize, self.start[b + 1] as usize);
                for &i in &self.items[s..e] {
                    if inside(self.feet[i as usize], q, r) {
                        out.push(i);
                    }
                }
            }
        }
        out.sort_unstable();
    }
}

thread_local! {
    static NEAR_LISTS: std::cell::Cell<Vec<Vec<u32>>> =
        const { std::cell::Cell::new(Vec::new()) };
}

/// A unit list for [`Sim::units_within`] whose buffer comes back for the next
/// query: every think asks several times, and each asked the allocator first.
/// A list per live query, so a query inside a query still has its own.
pub struct NearList(Vec<u32>);

impl NearList {
    pub fn new() -> Self {
        let mut lists = NEAR_LISTS.with(std::cell::Cell::take);
        let list = lists.pop().unwrap_or_default();
        NEAR_LISTS.with(|c| c.set(lists));
        Self(list)
    }
}

impl Default for NearList {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NearList {
    fn drop(&mut self) {
        let list = std::mem::take(&mut self.0);
        let mut lists = NEAR_LISTS.with(std::cell::Cell::take);
        lists.push(list);
        NEAR_LISTS.with(|c| c.set(lists));
    }
}

impl std::ops::Deref for NearList {
    type Target = Vec<u32>;
    fn deref(&self) -> &Vec<u32> {
        &self.0
    }
}

impl<'a> IntoIterator for &'a NearList {
    type Item = &'a u32;
    type IntoIter = std::slice::Iter<'a, u32>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl std::ops::DerefMut for NearList {
    fn deref_mut(&mut self) -> &mut Vec<u32> {
        &mut self.0
    }
}

impl Sim {
    /// Every alive unit with its feet within `r` of `p` (XZ), in ascending
    /// index, into `out` — from the unit grid inside the think window, else
    /// from the units themselves (the same answer, card TEAM).
    pub fn units_within(&self, p: Vec3, r: f32, out: &mut Vec<u32>) {
        if self.unit_grid.live() {
            self.unit_grid.within(p, r, out);
            return;
        }
        out.clear();
        let q = Vec2::new(p.x, p.z);
        for (i, u) in self.units.iter().enumerate() {
            if u.alive && inside(Vec2::new(u.soldier.pos.x, u.soldier.pos.z), q, r) {
                out.push(i as u32);
            }
        }
    }
}

/// Card TEAM (the maintainer, 2026-10-02: bots "all standing next to each other";
/// bar B3): a team-mate walking the same way this near ahead of a bot on
/// its walk (m) slows it, so a file walks a stride apart and not at the
/// heels - on main 5.9 of Death Star's 14.3 % and 4.1 of Polis Massa's
/// 9.0 % "a team-mate within 1.5 m" were files of walkers (8 seeds,
/// `lead_tmp/team/tools/files.py`). OURS and PROVISIONAL: the PS2 has no
/// team sense (SAIR 6).
pub const FOLLOW_GAP: f32 = 2.0;
/// ...to this share of the stick (a walk, `HOLD_WALK_STICK`'s).
pub const FOLLOW_STICK: f32 = 0.5;
/// "Ahead": inside this cosine of the bot's heading (40 degrees), and the
/// mate walking within it too, over [`FOLLOW_SPEED`] (m/s).
const AHEAD_COS: f32 = 0.766;
const FOLLOW_SPEED: f32 = 1.0;
/// A mate abreast - along the heading within this of the bot (m) - leads
/// the higher index: of two walking side by side one drops behind.
const ABREAST: f32 = 0.3;
/// Of another storey beyond this (m).
const STOREY: f32 = 2.0;

/// Card TEAM: the share of the stick for bot `me` walking the heading `dir`
/// (XZ): [`FOLLOW_STICK`] when a team-mate on foot walking the same way is
/// ahead within [`FOLLOW_GAP`], else 1.
pub fn follow_ease(sim: &Sim, me: crate::unit::UnitHandle, dir: Vec3) -> f32 {
    let Some(u) = sim.unit(me) else {
        return 1.0;
    };
    let d = Vec2::new(dir.x, dir.z).normalize_or_zero();
    if d == Vec2::ZERO {
        return 1.0;
    }
    let pos = u.soldier.pos;
    let mut near = NearList::new();
    sim.units_within(pos, FOLLOW_GAP, &mut near);
    let leads = near.iter().any(|&i| {
        let o = &sim.units[i as usize];
        if i == me.index
            || o.team != u.team
            || o.mount.is_some()
            || (o.soldier.pos.y - pos.y).abs() > STOREY
        {
            return false;
        }
        let off = Vec2::new(o.soldier.pos.x - pos.x, o.soldier.pos.z - pos.z);
        let v = Vec2::new(o.soldier.vel.x, o.soldier.vel.z);
        let (dist, speed) = (off.length(), v.length());
        let along = off.dot(d);
        let ahead = along > dist * AHEAD_COS || (along.abs() <= ABREAST && i < me.index);
        dist < FOLLOW_GAP && ahead && speed > FOLLOW_SPEED && v.dot(d) > speed * AHEAD_COS
    });
    if leads {
        FOLLOW_STICK
    } else {
        1.0
    }
}

/// Card TEAM (`analysis/bot-design.md` 4.3): a role, once given, is kept at
/// least this long while its task lasts (ticks).
pub const ROLE_KEEP: u64 = 20 * crate::TICK_HZ as u64;
/// COVER: the most bots of a team guarding one post while a friend
/// captures it (the design's capacity).
pub const COVER_CAP: usize = 2;
/// ...whose guard positions lie within the capture region's radius and this
/// much more of the post (m), on its storey ([`COVER_STOREY`])...
pub const COVER_REACH: f32 = 4.0;
const COVER_STOREY: f32 = 2.5;
/// ...not within this of the capturer's feet (m) nor within the Tactician's
/// `SPACING` of a position another bot holds or guards.
pub const COVER_CLEAR: f32 = 3.0;

/// Card TEAM — the TEAM BOARD (`analysis/bot-design.md` 4.3; OURS: the PS2
/// has no team sense). Once a second per team (team `t` on the ticks with
/// `tick % TICK_HZ == t`), in unit-index order, before the thinks: it ends
/// the roles whose task is over and gives the FLANK role ([`flank_pass`],
/// card FLNK) and the COVER role (only a team whose tier has `Tier::cover`,
/// Hard and Elite).
///
/// A friend CAPTURES post P while P is not its team's and a soldier of the
/// team counts in P's region (`Sim::counts_in_cp`); the capturer is the
/// lowest-index one with no role. Up to [`COVER_CAP`] of the others counting
/// there - AI soldiers on foot in the commander's pool, no sniper's spot -
/// take the role, each with a guard position ([`guard_spot`]): inside the
/// region first, covered toward the approach (the nearest of other posts
/// not the team's and live enemies), clear of the capturer and of the
/// positions held. Inside the region they still count for the capture (the
/// PS2's presence weight). A role ends when P is the team's, when its bot
/// dies, rides or takes an order or a sniper's spot, and - once
/// [`ROLE_KEEP`] has passed - when nobody of the team counts in P.
pub fn board(sim: &mut Sim) {
    let hz = crate::TICK_HZ as u64;
    for t in 0..2u8 {
        if sim.tick % hz != u64::from(t) {
            continue;
        }
        let cover = sim.difficulty_of(t).tier().cover;
        let n = sim.units.len();
        // Who counts in each post's region, per post (unit-index order).
        let inside: Vec<Vec<usize>> = (0..sim.cps.len())
            .map(|c| {
                (0..n)
                    .filter(|&i| sim.units[i].team == t && sim.counts_in_cp(i, c))
                    .collect()
            })
            .collect();
        // 1. End.
        for i in 0..n {
            let u = &sim.units[i];
            let Some(c) = u.ai.role.post.filter(|_| u.team == t) else {
                continue;
            };
            let cp = sim.cps.get(usize::from(c));
            let done = !cover
                || !u.alive
                || u.mount.is_some()
                || u.command.is_some()
                || u.ai.snipe.cur.is_some()
                || cp.is_none_or(|cp| cp.owner == Some(t))
                || (inside[usize::from(c)].is_empty() && sim.tick >= u.ai.role.since + ROLE_KEEP);
            if done {
                sim.units[i].ai.role = super::RoleState::default();
            }
        }
        flank_pass(sim, t);
        if !cover {
            continue;
        }
        // 2. Give.
        for (c, here) in inside.iter().enumerate() {
            if sim.cps[c].owner == Some(t) {
                continue;
            }
            let holds = |i: usize| sim.units[i].ai.role.post == Some(c as u8);
            let Some(&capturer) = here.iter().find(|&&i| sim.units[i].ai.role.post.is_none())
            else {
                continue;
            };
            let mut room = COVER_CAP.saturating_sub((0..n).filter(|&i| holds(i)).count());
            for &i in here {
                if room == 0 {
                    break;
                }
                let u = &sim.units[i];
                if i == capturer
                    || u.ai.role.post.is_some()
                    || u.controller != crate::unit::Controller::Ai
                    || u.command.is_some()
                    || u.ai.snipe.cur.is_some()
                {
                    continue;
                }
                if let Some((at, look)) = guard_spot(sim, i, c, capturer) {
                    sim.units[i].ai.role = super::RoleState {
                        post: Some(c as u8),
                        since: sim.tick,
                        at,
                        look,
                    };
                    sim.nav_stats.cover_given += 1;
                    room -= 1;
                }
            }
        }
    }
}

/// Card FLNK: the Flank role on (the switch for an A/B; OURS).
pub const FLANK: bool = true;
/// ...given in a group of at least this many attackers (the design's 4.3).
pub const FLANK_GROUP_MIN: usize = 3;

/// The hub of the post unit `i` attacks by the commander's plan.
fn attacked_hub(sim: &Sim, i: usize) -> Option<u16> {
    let u = &sim.units[i];
    match sim.commander[usize::from(u.team).min(1)].of(i as u32)? {
        crate::commander::Objective::Attack(cp) => sim.cps.get(cp as usize)?.nav_hub,
        _ => None,
    }
}

/// How many of a group of `n` attackers flank at `share`: the share
/// rounded, none in a group under [`FLANK_GROUP_MIN`].
pub fn flank_cap(n: usize, share: f32) -> usize {
    if n < FLANK_GROUP_MIN {
        return 0;
    }
    (share * n as f32).round() as usize
}

/// Card FLNK (`analysis/bot-design.md` 4.3, 6.2) — the board's FLANK role
/// for team `t`, after its Cover pass ends what is over. A pair's GROUP is
/// the team's AI soldiers on foot in the commander's pool attacking post
/// hub T by the plan whose attack starts from post S - planned from a hub
/// whose `FlankTable::origin` is S's (`FlankState`,
/// `route::retarget_to_post`); while the pair has a flank route
/// (`Sim::flank`) and its tier has `Tier::flank`, [`flank_cap`] of the group
/// hold the role - given in unit-index order to members still on their
/// first leg out of their start hub with no other role, each re-planned
/// from there onto the route (`plan_leg`: to the route's nearest hub, then
/// along it, then the authored route). The role ends with the attack: a
/// death, a seat, an order, another objective, or the last leg into the
/// post - and outlives a detour of the route to another hub (a vehicle
/// claim, an enemy in the post's region).
fn flank_pass(sim: &mut Sim, t: u8) {
    use crate::unit::Controller;
    let on = FLANK;
    let n = sim.units.len();
    let in_group = |sim: &Sim, i: usize| {
        let u = &sim.units[i];
        u.team == t
            && u.alive
            && u.mount.is_none()
            && u.controller == Controller::Ai
            && u.command.is_none()
            && attacked_hub(sim, i) == Some(u.ai.flank.to)
    };
    // 1. End.
    for i in 0..n {
        let u = &sim.units[i];
        if u.team != t || u.ai.flank.route.is_none() {
            continue;
        }
        let arrived =
            u.ai.route
                .is_some_and(|r| r.is_last() && r.dest_hub == u.ai.flank.to);
        let left = !u.alive || attacked_hub(sim, i) != Some(u.ai.flank.to);
        let died = !u.alive;
        let done = !on || arrived || left || !in_group(sim, i);
        if done {
            sim.units[i].ai.flank.route = None;
            let ends = &mut sim.nav_stats.flank_ends;
            ends[if arrived {
                0
            } else if died {
                1
            } else if left {
                2
            } else {
                3
            }] += 1;
        }
    }
    if !on {
        return;
    }
    // 2. Give, route by route.
    for k in 0..sim.flank.routes.len() {
        let (from, to, share) = {
            let r = &sim.flank.routes[k];
            (r.from, r.to, r.share)
        };
        let group: Vec<usize> = (0..n)
            .filter(|&i| {
                let f = sim.units[i].ai.flank;
                f.to == to && sim.flank.origin(f.from) == Some(from) && in_group(sim, i)
            })
            .collect();
        let cap = flank_cap(group.len(), share);
        let mut held = group
            .iter()
            .filter(|&&i| sim.units[i].ai.flank.route.is_some())
            .count();
        for &i in &group {
            if held >= cap {
                break;
            }
            let u = &sim.units[i];
            let start = u.ai.flank.from;
            let at = sim.nav.as_ref().and_then(|g| g.hub(start)).map(|h| h.xz());
            let first_leg =
                u.ai.route
                    .is_some_and(|r| r.dest_hub == to && !r.is_last() && Some(r.from) == at);
            if u.ai.flank.route.is_some()
                || u.ai.role.post.is_some()
                || u.ai.snipe.cur.is_some()
                || !first_leg
            {
                continue;
            }
            let mut st = u.ai;
            st.flank.route = Some(k as u16);
            st.flank.since = sim.tick;
            let mut nt = super::NavTick::default();
            let leg = super::plan_leg(sim, &st, sim.handle(i), start, to, None, &mut nt);
            if let Some(r) = leg.filter(|r| r.source == super::RouteSource::Flank) {
                st.route = Some(r);
                sim.units[i].ai = st;
                sim.nav_stats.flank_given += 1;
                held += 1;
            }
        }
    }
}

/// Card TEAM: the guard position for unit `i` covering post `c` while unit
/// `capturer` captures it, and the point it faces; `None` when the post has
/// no approach (no other post not the team's and no enemy alive) or no
/// position passes. Of the positions within the region's radius +
/// [`COVER_REACH`] of the post on its storey, clear of the capturer's feet
/// by [`COVER_CLEAR`] and of every position another bot holds (the
/// Tactician's) or guards by `SPACING`: the best by inside the region (4),
/// cover toward the approach (`Position::hides_toward`, 3), on the
/// approach's side of the post (1), less its distance from the bot over the
/// reach; the first in table order on a tie.
pub fn guard_spot(sim: &Sim, i: usize, c: usize, capturer: usize) -> Option<(u32, Vec3)> {
    let u = &sim.units[i];
    let cp = &sim.cps[c];
    let post = cp.def.pos;
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let near = |a: Vec3, b: Vec3| flat(a).distance(flat(b));
    // The approach: nearest to this post among other posts not the team's
    // and live enemies.
    let from = sim
        .cps
        .iter()
        .enumerate()
        .filter(|&(k, o)| k != c && o.owner != Some(u.team))
        .map(|(_, o)| o.def.pos)
        .chain(
            sim.units
                .iter()
                .filter(|o| o.alive && o.team != u.team && o.team < 2)
                .map(|o| o.soldier.pos),
        )
        .min_by(|a, b| near(*a, post).total_cmp(&near(*b, post)))?;
    let d = (flat(from) - flat(post)).normalize_or_zero();
    if d == Vec2::ZERO {
        return None;
    }
    let reach = cp.def.capture.radius() + COVER_REACH;
    let mut cand: Vec<u32> = Vec::new();
    sim.positions.within(post, reach, &mut cand);
    let mut taken: Vec<Vec3> = Vec::new();
    for (k, o) in sim.units.iter().enumerate() {
        if k == i || !o.alive {
            continue;
        }
        let ts = &o.ai.tpos;
        for (held, until) in [(ts.held, ts.until), (ts.prev, ts.prev_until)] {
            if let Some(q) = held.filter(|_| until > sim.tick) {
                taken.extend(sim.positions.positions.get(q as usize).map(|q| q.pos));
            }
        }
        if o.ai.role.post.is_some() {
            taken.extend(
                sim.positions
                    .positions
                    .get(o.ai.role.at as usize)
                    .map(|q| q.pos),
            );
        }
    }
    let feet = sim.units[capturer].soldier.pos;
    let mut best: Option<(f32, u32)> = None;
    for &k in &cand {
        let q = &sim.positions.positions[k as usize];
        if (q.pos.y - post.y).abs() > COVER_STOREY
            || near(q.pos, feet) < COVER_CLEAR
            || taken
                .iter()
                .any(|p| near(*p, q.pos) < super::tactic::SPACING)
        {
            continue;
        }
        let score = 4.0 * f32::from(u8::from(cp.def.capture.contains(q.pos)))
            + 3.0 * f32::from(u8::from(q.hides_toward(d)))
            + f32::from(u8::from((flat(q.pos) - flat(post)).dot(d) > 0.0))
            - near(q.pos, u.soldier.pos) / reach;
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, k));
        }
    }
    best.map(|(_, k)| {
        let q = sim.positions.positions[k as usize].pos;
        (k, q + Vec3::new(d.x, 0.0, d.y) * 20.0 + Vec3::Y * 1.4)
    })
}

#[cfg(test)]
mod tests;
