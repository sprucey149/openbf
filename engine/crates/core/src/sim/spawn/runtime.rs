//! Spawn placement, roster admission, respawning and spawn-screen input.
use super::{
    class_share, flood_key, walker_point, Carousel, ClassRefusal, SideChoice, SpawnStage, CAROUSEL,
    CLASS_FLOOR, CLASS_SLIDE_TICKS, PEN_CELL, PEN_CELLS, PEN_CLEAR, PEN_DROP, SEAT_REACH,
    SPAWN_LIFT,
};
use crate::aim_assist::AssistState;
use crate::battle::SpawnOutcome;
use crate::sim::input::{buttons, PadState};
use crate::sim::state::{Event, Sim};
use crate::unit::{Controller, SpawnPost, UnitHandle, UnitSpec};
use glam::{Vec2, Vec3};

impl Sim {
    /// The floor a unit spawned at `node` stands on: the ground under the
    /// node's own height, and when there is none — a node authored under its
    /// floor (TAT1's `(-257.9, 10.6, -11.2)` is 2.06 m under the street, past
    /// the terrain's bury rule) — the ground under [`SPAWN_LIFT`] over it.
    /// Without this such a unit stood on the no-ground floor at -300 for the
    /// round (and now dies at the kill floor every time it spawns).
    ///
    /// The same when the ground under the node is another floor, OFF the
    /// playable level ([`Sim::level_at`], card LVLF): BF2 Utapau's
    /// `(-46.0, 0.0, 225.2)` is 1.01 m under its landing and 74.8 m over the
    /// sinkhole's floor, where every unit that drew the node spawned and was
    /// given up as lost 45 s later (61 of the map's 80 lost deaths in 8
    /// rounds). PROVISIONAL (ours): how the PS2 seats a spawn is not read.
    ///
    /// A post's node whose floor is a PEN stands on its seat instead
    /// ([`Sim::resolve_spawn_seats`], card LLEG).
    pub fn spawn_ground(&self, node: Vec3) -> f32 {
        match self
            .spawn_seats
            .iter()
            .find(|(n, _)| n.x == node.x && n.z == node.z)
        {
            Some(&(_, y)) => y,
            None => self.node_floor(node),
        }
    }

    /// Card LLEG: derive [`Sim::spawn_seats`] — for every post's spawn node
    /// whose floor is a PEN ([`Sim::floor_reach`] under [`PEN_CELLS`]: the
    /// node is inside a solid body), the lowest floor over it, within
    /// [`SEAT_REACH`], that is none. Asked when a post is added, with no
    /// vehicle on the field yet.
    ///
    /// BF1 Naboo Plains' cp3 node `(-107.2, -108.5, 63.2)` lies inside a
    /// fallen column 3 m thick: the PS2's soldier spawns ON it, at y -104.24,
    /// stands 1.5 s, walks off its end and drops to the ground (live RAM, one
    /// spawn in 464 s, `lead_tmp/lleg/ps2/nab1_soldiers.csv`); the port's
    /// stood on the ground inside the mesh and walked its walls until it was
    /// given up (384 of the map's 415 stalled windows in 8 rounds, 12 % of
    /// all 32 maps'). BF2 Mustafar's `(-66.2, 66.7, 23.1)` is inside a crate
    /// (not measured on the PS2); no other node of the 32 maps moves.
    /// PROVISIONAL (ours): the pen test and the reach — how the PS2 seats a
    /// spawn is not read.
    pub fn resolve_spawn_seats(&mut self) {
        let seats = self
            .cps
            .iter()
            .flat_map(|c| c.def.spawn_nodes.iter())
            .filter_map(|&(n, _)| self.pen_seat(n).map(|y| (n, y)))
            .collect();
        self.spawn_seats = seats;
    }

    fn pen_seat(&self, node: Vec3) -> Option<f32> {
        let y = self.node_floor(node);
        let at = |y: f32| Vec3::new(node.x, y, node.z);
        let mut over = (1..=SEAT_REACH)
            .map(|k| self.ground_under(node + Vec3::Y * k as f32, false))
            .filter(|&f| f > y + crate::collision::STEP_HEIGHT)
            .peekable();
        if over.peek().is_none() || self.floor_reach(at(y), PEN_CELLS) >= PEN_CELLS {
            return None;
        }
        let mut last = y;
        over.find(|&f| {
            let new = f > last;
            last = last.max(f);
            new && self.floor_reach(at(f), PEN_CELLS) >= PEN_CELLS
        })
    }

    fn node_floor(&self, node: Vec3) -> f32 {
        let y = self.ground_under(node, false);
        let lifted = || self.ground_under(node + Vec3::Y * SPAWN_LIFT, false);
        if y <= crate::collision::NO_GROUND {
            return lifted();
        }
        if self.level_at(Vec3::new(node.x, y, node.z)).is_off() {
            let up = lifted();
            if up > y && !self.level_at(Vec3::new(node.x, up, node.z)).is_off() {
                return up;
            }
        }
        y
    }

    /// How many [`PEN_CELL`] cells of floor a soldier standing at `from`
    /// walks to, counted up to `cap`: a flood over the grid, a step between
    /// two cells being one with a floor he stands on at its end (no steeper
    /// than 45 degrees up, no more than [`PEN_DROP`] down) and no wall
    /// between — the sweep of the stepped capsule, from the higher floor of
    /// the two, and the wall pass leaving him where he then stands. Card
    /// LLEG: under [`PEN_CELLS`] the place is a pen. PROVISIONAL (ours).
    pub fn floor_reach(&self, from: Vec3, cap: usize) -> usize {
        self.floor_flood(from, cap, &|_| false).0
    }

    /// [`Sim::floor_reach`]'s flood, stopped at the first cell whose floor
    /// point `goal` accepts: the cells counted, and that point with the
    /// length of the flood's shortest walk to it (m, in [`PEN_CELL`] steps
    /// along the grid). Card SPNC's
    /// census asks it whether a soldier on a spawn node walks to his first
    /// leg's hub at all, round walls and down no more than [`PEN_DROP`].
    pub fn floor_flood(
        &self,
        from: Vec3,
        cap: usize,
        goal: &dyn Fn(Vec3) -> bool,
    ) -> (usize, Option<(Vec3, f32)>) {
        let mut seen = std::collections::HashSet::from([flood_key(0, 0, from.y)]);
        let mut queue = std::collections::VecDeque::from([(0i32, 0i32, from.y, 0u32)]);
        let mut cells = 0;
        while let Some((ix, iz, y, d)) = queue.pop_front() {
            cells += 1;
            if cells >= cap {
                break;
            }
            let p = Vec3::new(
                from.x + ix as f32 * PEN_CELL,
                y,
                from.z + iz as f32 * PEN_CELL,
            );
            if goal(p) {
                return (cells, Some((p, d as f32 * PEN_CELL)));
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if let Some(f) = self.flood_step(p, dx as f32, dz as f32) {
                    if seen.insert(flood_key(ix + dx, iz + dz, f)) {
                        queue.push_back((ix + dx, iz + dz, f, d + 1));
                    }
                }
            }
        }
        (cells, None)
    }

    /// One step of the floor flood from the floor point `a` by one
    /// [`PEN_CELL`] along (`dx`, `dz`): the floor a soldier stands on at its
    /// end ([`Sim::floor_flood`]'s rule), `None` when there is none or a wall
    /// is between.
    fn flood_step(&self, a: Vec3, dx: f32, dz: f32) -> Option<f32> {
        use crate::collision::Capsule;
        let height = crate::unit::capsule_height(crate::soldier::Stance::Stand);
        let radius = crate::unit::SOLDIER_RADIUS;
        let b = a + Vec3::new(dx, 0.0, dz) * PEN_CELL;
        let (f, n) = self.ground_under_n(b, false, a.y + PEN_CELL);
        if f <= crate::collision::NO_GROUND
            || n.y <= crate::soldier::FLOOR_MIN_NY
            || f - a.y > PEN_CELL
            || a.y - f > PEN_DROP
        {
            return None;
        }
        let base = Vec3::new(a.x, a.y.max(f), a.z);
        let cap = Capsule {
            base,
            height,
            radius,
        }
        .stepped();
        if self
            .collision
            .wall_sweep(&cap, Vec3::new(dx, 0.0, dz), PEN_CELL)
            .is_some()
        {
            return None;
        }
        let end = Capsule {
            base: Vec3::new(b.x, f, b.z),
            height,
            radius,
        };
        (self.collision.push_out_soldier(&end).length() <= PEN_CLEAR).then_some(f)
    }

    /// Is a soldier standing at `from` (feet on a floor) clear of every body
    /// - the wall pass leaves him where he is?
    pub fn soldier_clear(&self, from: Vec3) -> bool {
        let cap = crate::collision::Capsule {
            base: from,
            height: crate::unit::capsule_height(crate::soldier::Stance::Stand),
            radius: crate::unit::SOLDIER_RADIUS,
        };
        self.collision.push_out_soldier(&cap).length() <= PEN_CLEAR
    }

    /// Card TGAP: can a soldier standing at `from` WALK to within `reach` of
    /// `to` (XZ) on a floor within `tol` of `to_y` - round walls, by
    /// [`Sim::floor_flood`]'s steps, never more than `band` from the straight
    /// line between the two? A best-first search over the flood's grid (the
    /// cell nearest `to` first), given up after `cap` cells: `None` when it
    /// ran out of cells before it could answer, else whether it got there.
    /// PROVISIONAL (ours): the PS2 has no such test; it is the port's load-time
    /// check of an authored arc against its collision (BF1 Tatooine's arc
    /// 78 -> 77 runs into a leaning slab, shut on the PS2 too).
    #[allow(clippy::too_many_arguments)]
    pub fn floor_path(
        &self,
        from: Vec3,
        to: Vec2,
        to_y: f32,
        reach: f32,
        tol: f32,
        band: f32,
        cap: usize,
    ) -> Option<bool> {
        self.floor_path_near(from, to, to_y, reach, tol, band, cap)
            .0
    }

    /// [`Sim::floor_path`], and the cell nearest `to` (XZ) it walked to with
    /// its floor: what a census prints for an arc it calls walled.
    #[allow(clippy::too_many_arguments)]
    pub fn floor_path_near(
        &self,
        from: Vec3,
        to: Vec2,
        to_y: f32,
        reach: f32,
        tol: f32,
        band: f32,
        cap: usize,
    ) -> (Option<bool>, Vec3) {
        use std::cmp::Reverse;
        let start = Vec2::new(from.x, from.z);
        let line = to - start;
        let len2 = line.length_squared().max(1e-6);
        let off_line = |p: Vec2| {
            let t = ((p - start).dot(line) / len2).clamp(0.0, 1.0);
            (p - (start + line * t)).length()
        };
        // Integer priority (mm to the goal) keeps the order exact.
        let prio = |p: Vec2| ((p - to).length() * 1000.0) as u32;
        let mut seen = std::collections::HashSet::from([flood_key(0, 0, from.y)]);
        let mut heap = std::collections::BinaryHeap::from([(
            Reverse(prio(start)),
            0i32,
            0i32,
            from.y.to_bits(),
        )]);
        let mut cells = 0;
        let mut near = (f32::INFINITY, from);
        while let Some((_, ix, iz, yb)) = heap.pop() {
            let y = f32::from_bits(yb);
            let p = Vec2::new(from.x + ix as f32 * PEN_CELL, from.z + iz as f32 * PEN_CELL);
            let d = (p - to).length();
            if d < near.0 {
                near = (d, Vec3::new(p.x, y, p.y));
            }
            if d <= reach && (y - to_y).abs() <= tol {
                return (Some(true), near.1);
            }
            cells += 1;
            if cells >= cap {
                return (None, near.1);
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let q = p + Vec2::new(dx as f32, dz as f32) * PEN_CELL;
                if off_line(q) > band {
                    continue;
                }
                let Some(f) = self.flood_step(Vec3::new(p.x, y, p.y), dx as f32, dz as f32) else {
                    continue;
                };
                if seen.insert(flood_key(ix + dx, iz + dz, f)) {
                    heap.push((Reverse(prio(q)), ix + dx, iz + dz, f.to_bits()));
                }
            }
        }
        (Some(false), near.1)
    }

    /// Where `team` may spawn: the spawn nodes of the posts it may spawn at,
    /// in post order, then the points of its live command walkers
    /// ([`Sim::walker_spawn_points`]).
    pub fn spawn_points(&self, team: u8) -> Vec<(Vec3, f32)> {
        let mut points: Vec<(Vec3, f32)> = self
            .cps
            .iter()
            .filter(|c| c.spawnable_by(team))
            .flat_map(|c| c.def.spawn_nodes.iter().copied())
            .collect();
        points.extend(self.walker_spawn_points(team));
        points
    }

    /// The live command walkers `team` may spawn at, in vehicle order: a
    /// walker whose class authors spawn points and whose team is `team` — its
    /// pad's, from the spawn to the kill (re-command-walker-post §3 [O]: no
    /// ally, no capture). Destroyed, it is gone from here at once; the pad's
    /// next walker is here again.
    pub fn command_walkers(
        &self,
        team: u8,
    ) -> impl Iterator<Item = (crate::vehicle::VehicleHandle, &crate::vehicle::Vehicle)> {
        self.alive_vehicles().filter(move |(_, v)| {
            v.team == Some(team) && !self.class_spawn_points(v.class).is_empty()
        })
    }

    /// Vehicle class `class`'s spawn points (a command walker's
    /// `WalkerParams::spawn_points`; empty for anything else).
    fn class_spawn_points(&self, class: u32) -> &[(Vec3, f32)] {
        self.walker_params
            .get(class as usize)
            .and_then(Option::as_ref)
            .map_or(&[], |p| &p.spawn_points)
    }

    /// The spawn points of `team`'s live command walkers (card CWSP,
    /// re-command-walker-post §4 [O]): each class point through the walker's
    /// LIVE frame ([`walker_point`]). Not a `CommandPost`: nothing captures
    /// it, and BF2's bleed and all-posts rule — the port's on every map —
    /// leave it out (§7).
    pub fn walker_spawn_points(&self, team: u8) -> Vec<(Vec3, f32)> {
        self.command_walkers(team)
            .flat_map(|(_, v)| {
                self.class_spawn_points(v.class)
                    .iter()
                    .map(|&p| walker_point(v.pos, v.rot, p))
            })
            .collect()
    }

    /// The points of the command walkers `team`'s pads are about to build —
    /// a pad of the team with no vehicle yet and a command-walker class —
    /// in the pad's own pose. For the OPENING wave only: the PS2's walker
    /// stands on its pad before the first unit spawns (BF2: the walker at
    /// 0.06 s, units from 0.9 s; §10.3), where the port's pads build on the
    /// first tick, after the wave is placed.
    pub fn pad_walker_spawn_points(&self, team: u8) -> Vec<(Vec3, f32)> {
        self.vehicle_pads
            .iter()
            .filter(|p| p.team == Some(team) && p.vehicles.is_empty() && p.wants_spawn())
            .flat_map(|p| {
                let class = p.def.class_for(p.team).map_or(u32::MAX, |c| c.spec.class);
                let rot = crate::vehicle::hover::quat_y(-p.def.yaw);
                self.class_spawn_points(class)
                    .iter()
                    .map(move |&pt| walker_point(p.def.pos, rot, pt))
            })
            .collect()
    }

    /// The posts `team` may spawn at, each with its weight in a bot's draw
    /// (card BSPN): its command posts in post order, then its live command
    /// walkers in vehicle order — to the PS2 a walker's post is a post like
    /// any other, constructed with the default weight
    /// (re-command-walker-post §2, §6 [O]).
    pub fn spawn_posts(&self, team: u8) -> Vec<(SpawnPost, f32)> {
        let mut posts: Vec<(SpawnPost, f32)> = self
            .cps
            .iter()
            .enumerate()
            .filter(|(_, c)| c.spawnable_by(team))
            .map(|(i, c)| (SpawnPost::Post(i as u16), c.ai_spawn_weight))
            .collect();
        posts.extend(
            self.command_walkers(team)
                .map(|(h, _)| (SpawnPost::Walker(h), crate::cp::AI_SPAWN_WEIGHT_DEFAULT)),
        );
        posts
    }

    /// Post `post`'s spawn nodes for `team`: a command post's own, or a live
    /// command walker's class points through its live frame. Empty when the
    /// team may not (or no longer) spawn there.
    pub fn spawn_post_nodes(&self, team: u8, post: SpawnPost) -> Vec<(Vec3, f32)> {
        match post {
            SpawnPost::Post(i) => self
                .cps
                .get(usize::from(i))
                .filter(|c| c.spawnable_by(team))
                .map_or_else(Vec::new, |c| c.def.spawn_nodes.clone()),
            SpawnPost::Walker(h) => self
                .command_walkers(team)
                .find(|(w, _)| *w == h)
                .map_or_else(Vec::new, |(_, v)| {
                    self.class_spawn_points(v.class)
                        .iter()
                        .map(|&p| walker_point(v.pos, v.rot, p))
                        .collect()
                }),
        }
    }

    /// One draw of a post for a bot of `team`, as BF2's `FUN_002e7240`
    /// (re-command-walker-post §6 [O]): with S the sum of the eligible
    /// posts' weights, `r = uniform(0, S − 1)` and the first post in list
    /// order at which the running subtraction of the weights reaches ≤ 0.
    /// `None` when the team has no post or every weight is 0. BF1's draw
    /// (`FUN_00261e80`) is uniform over the same list; with BF1's equal
    /// weights this rule is that to within 1 part in S − 1, and like BF1's
    /// it takes no draw when there is one post.
    pub fn draw_spawn_post(&mut self, team: u8) -> Option<SpawnPost> {
        let posts = self.spawn_posts(team);
        let total: f32 = posts.iter().map(|p| p.1).sum();
        if total <= 0.0 {
            return None;
        }
        if let [(only, _)] = posts[..] {
            return Some(only);
        }
        let mut r = self.lcg.uniform(0.0, total - 1.0);
        for &(post, w) in &posts {
            r -= w;
            if r <= 0.0 {
                return Some(post);
            }
        }
        posts.iter().rev().find(|p| p.1 > 0.0).map(|p| p.0)
    }

    /// Dead bot `i`'s post for its next life (card BSPN;
    /// re-command-walker-post §6 [O]): the one it already drew while its
    /// team may still spawn there (the post not captured, the walker alive),
    /// else a fresh draw. Called from the respawn loop once the unit's timer
    /// has run out, so the draw falls where the PS2's does — at the end of
    /// the respawn delay — and is kept through the wave and alive-cap waits.
    pub(in crate::sim) fn keep_or_draw_spawn_post(&mut self, i: usize) -> Option<SpawnPost> {
        let team = self.units[i].team;
        let kept = self.units[i]
            .spawn_post
            .filter(|&p| self.spawn_posts(team).iter().any(|q| q.0 == p));
        let post = kept.or_else(|| self.draw_spawn_post(team));
        self.units[i].spawn_post = post;
        post
    }

    /// The posts a HUMAN of `team` may take on the spawn map, with where each
    /// is now (card SPSEL): [`Sim::spawn_posts`]' list, in its order, less a
    /// post with no spawn node for the team — the PS2's `CanTeamSpawnAt` needs
    /// a SpawnPath or class spawn points (re-command-walker-post §3 [O]), and
    /// BF2 Hoth's spawn map skips the one team post without either
    /// (`lead_tmp/spsel/fit_dir.py`). A walker's position is its live one.
    pub fn player_spawn_posts(&self, team: u8) -> Vec<(SpawnPost, Vec3)> {
        self.spawn_posts(team)
            .into_iter()
            .filter(|&(p, _)| !self.spawn_post_nodes(team, p).is_empty())
            .filter_map(|(p, _)| Some((p, self.spawn_post_pos(p)?)))
            .collect()
    }

    /// Where post `post` is: a command post's own position, a command
    /// walker's live one (its slot's last while the slot is not yet re-used,
    /// so a walker that just died still has a place to be nearest to).
    pub fn spawn_post_pos(&self, post: SpawnPost) -> Option<Vec3> {
        match post {
            SpawnPost::Post(i) => self.cps.get(usize::from(i)).map(|c| c.def.pos),
            SpawnPost::Walker(h) => self
                .vehicles
                .get(h.index as usize)
                .filter(|v| v.gen == h.gen)
                .map(|v| v.pos),
        }
    }

    /// The spawn map's pick, both games (B2 `FUN_0039a058`, B1
    /// `FUN_002e03b0`; re-command-walker-post §5 [O]): from post `from` at
    /// `origin`, over every other post in `posts`, a candidate needs a
    /// positive projection `along` on the push `dir` (a world xz direction,
    /// normalised here); the lowest `along + 0.25 × perp²` wins. `None` when
    /// no post is ahead: the caller keeps its selection.
    pub fn pick_spawn_post(
        posts: &[(SpawnPost, Vec3)],
        from: Option<SpawnPost>,
        origin: Vec3,
        dir: Vec2,
    ) -> Option<SpawnPost> {
        let d = dir.normalize_or_zero();
        if d == Vec2::ZERO {
            return None;
        }
        let mut best = None;
        let mut best_v = f32::INFINITY;
        for &(p, pos) in posts {
            if Some(p) == from {
                continue;
            }
            let off = Vec2::new(pos.x - origin.x, pos.z - origin.z);
            let along = off.dot(d);
            if along <= 0.0 {
                continue;
            }
            let v = along + 0.25 * (off - d * along).length_squared();
            if v < best_v {
                best_v = v;
                best = Some(p);
            }
        }
        best
    }

    /// A human's post for the next life (B2 `FUN_0036ca08`, B1
    /// `FUN_002ac6f0`; re-command-walker-post §6 [O]): `chosen` while the team
    /// may still spawn there, else the eligible post nearest to where
    /// `chosen` is (a captured post's own place, a dead walker's last). With
    /// no place for `chosen` at all, the list's first post — the PS2 takes its
    /// own default pick there (`FUN_0035b6f8`, not traced: PROVISIONAL).
    /// `None` when the team has no post.
    pub fn keep_or_nearest_post(&self, team: u8, chosen: SpawnPost) -> Option<SpawnPost> {
        let posts = self.player_spawn_posts(team);
        if posts.iter().any(|q| q.0 == chosen) {
            return Some(chosen);
        }
        let Some(at) = self.spawn_post_pos(chosen) else {
            return posts.first().map(|q| q.0);
        };
        nearest_post(&posts, at)
    }

    /// Slot `slot`'s spawn-map selection as the next respawn would take it:
    /// its pick through keep-or-nearest; `None` while it has never picked
    /// (the respawn then draws from every post's nodes).
    pub fn spawn_post_selected(&self, slot: usize) -> Option<SpawnPost> {
        let chosen = self.spawn_post_choice.get(slot).copied().flatten()?;
        let team = self.players.get(slot).copied().flatten().map_or(0, |h| {
            self.units.get(h.index as usize).map_or(0, |u| u.team)
        });
        self.keep_or_nearest_post(team, chosen)
    }

    /// Remove `n` alive AI of `team` from the field (no-overspawn shedding).
    pub(in crate::sim) fn shed_ai(&mut self, team: u8, n: u32) {
        let mut left = n;
        for i in 0..self.units.len() {
            if left == 0 {
                break;
            }
            let u = &mut self.units[i];
            if u.alive && !u.is_player && u.team == team {
                u.alive = false;
                u.respawn_at = None;
                let h = UnitHandle {
                    index: i as u32,
                    gen: u.gen,
                };
                self.events.push(Event::Despawn { unit: h });
                self.retire_saber_flights(i);
                self.unseat_unit(h);
                // Phase D: a bot the no-overspawn rule sheds must not come back
                // carrying the order it had (it comes back from its `UnitSpec`).
                crate::squad::on_unit_died(self, h);
                left -= 1;
            }
        }
    }

    /// Battle admission for a (re)spawn: ticket gate + shedding.
    fn admit(&mut self, team: u8, is_player: bool) -> bool {
        match self.battle.try_spawn(team as usize, is_player) {
            SpawnOutcome::Refused => false,
            SpawnOutcome::Spawned { despawned_ai } => {
                if despawned_ai > 0 {
                    self.shed_ai(team, despawned_ai);
                }
                true
            }
        }
    }

    /// Spawn through the battle's reinforcement gate (death consumes the
    /// ticket, spawn only bumps the alive count; AI refused while tickets <=
    /// alive; a PLAYER taking the last slots sheds AI — core::battle). Feet
    /// snapped to the terrain. Returns None if refused.
    pub fn spawn(&mut self, spec: &UnitSpec) -> Option<UnitHandle> {
        self.spawn_with_slot_policy(spec, false)
    }

    /// Add a roster slot without taking a dead unit's pending respawn slot.
    pub fn spawn_roster_slot(&mut self, spec: &UnitSpec) -> Option<UnitHandle> {
        self.spawn_with_slot_policy(spec, true)
    }

    fn spawn_with_slot_policy(
        &mut self,
        spec: &UnitSpec,
        preserve_respawns: bool,
    ) -> Option<UnitHandle> {
        if !self.admit(spec.team, spec.is_player()) {
            return None;
        }
        // A dead unit a player slot still holds is that player's (its next
        // life, or the spawn screens' held soldier, card SSELA): not free.
        let held = |i: usize| self.players.iter().flatten().any(|p| p.index as usize == i);
        let slot = (0..self.units.len()).find(|&i| {
            !self.units[i].alive
                && !held(i)
                && (!preserve_respawns || self.units[i].respawn_at.is_none())
        });
        let gen = match slot {
            Some(i) => self.units[i].gen + 1,
            None => 1,
        };
        let mut unit = spec.build(gen, &self.weapon_classes);
        if self.weapon_classes[unit.weapon_class].saber.is_some()
            || unit
                .held
                .iter()
                .any(|held| self.weapon_classes[held.class].saber.is_some())
        {
            self.melee_lcg_active = true;
        }
        // D2: "every bot receives the tier's skill at spawn" — enforced HERE
        // so it holds for every spawn path (the class table's `spawn_ai`, the
        // team template's, a test's hand-built spec), and mirrored into the
        // spec so a respawn from it starts from the same place. Players and
        // dummies keep 0: `Unit.skill` is a BOT's number.
        unit.skill = crate::skill_for(spec.controller, self.bot_skill(spec.team));
        unit.spec.skill = unit.skill;
        // The probe starts from the AUTHORED node y: spawn nodes are authored
        // ON the platforms (BF1 KAM1 `CP4SpawnPath` nodes sit at y ~ 61.4-62.3
        // for a post at y = 61), so the model floor under them is what grounds
        // the unit, not the terrain far below.
        unit.soldier.pos.y = self.spawn_ground(spec.pos);
        let index = match slot {
            Some(i) => {
                self.units[i] = unit;
                i
            }
            None => {
                self.units.push(unit);
                self.units.len() - 1
            }
        };
        let h = UnitHandle {
            index: index as u32,
            gen,
        };
        if let Some(slot) = spec.player_slot {
            let slot = usize::from(slot);
            self.reserve_slot(slot);
            if self.players[slot].is_none() {
                self.players[slot] = Some(h);
            }
        }
        Some(h)
    }

    /// Take player slot `slot`'s live unit OFF the field and free the slot,
    /// without charging the team a reinforcement. Returns where it stood
    /// (`pos`, `yaw`) so the caller can put an ordinary bot in its place;
    /// `None` when the slot holds no live unit.
    ///
    /// This is a HEADLESS-REFEREE hook (queue S0), not a game mechanic. The
    /// referees (`game/examples/round_to_outcome.rs`, `game/examples/arena.rs`)
    /// build their sim through `WalkSim`, which always seats a player in slot
    /// 0, and then never press a button. An idle human is not a unit either
    /// disc's round contains, and it is not neutral: `Battle::try_spawn` lets
    /// a player displace AI to take the last reinforcement slots, `alive()`
    /// counts it against the `alive < tickets` ticket rule for the whole
    /// round because it never dies, and [`Sim::apply_team_handicap`] reads
    /// `is_player` over `units` to decide which side is the human's. The game
    /// binary never calls this.
    ///
    /// Not a death: `Battle::on_death` spends a reinforcement and this does
    /// not — the unit is being un-spawned, so only the live count moves. The
    /// battle invariant can only relax (`alive()` drops with
    /// `alive_players`), so nothing is shed.
    pub fn vacate_player_slot(&mut self, slot: usize) -> Option<(Vec3, f32)> {
        let h = (*self.players.get(slot)?)?;
        let u = self.unit(h)?;
        let (pos, yaw, team) = (u.soldier.pos, u.soldier.yaw, u.team);
        let i = h.index as usize;
        self.units[i].alive = false;
        // A vacated unit must not come back: the respawn loop takes any dead
        // unit whose `respawn_at` is due, exactly as `shed_ai` guards.
        self.units[i].respawn_at = None;
        let t = &mut self.battle.teams[usize::from(team.min(1))];
        t.alive_players = t.alive_players.saturating_sub(1);
        debug_assert!(t.invariant_holds());
        self.events.push(Event::Despawn { unit: h });
        self.retire_saber_flights(i);
        self.unseat_unit(h);
        self.class_change_life_ended(h);
        // Phase D: the orders this slot gave go with it (`on_unit_died` reads
        // the still-set `spec.player_slot`), so clear before freeing the slot.
        crate::squad::on_unit_died(self, h);
        self.players[slot] = None;
        Some((pos, yaw))
    }

    /// Grow the per-slot tables (`players`, `assist`, `magnet`,
    /// `reticle_pick`, `spawn_choice`, `prev_spawn_bits`, `spawn_post_choice`)
    /// so `slot` is addressable. They are
    /// kept the same length so a slot index is valid in all of them.
    fn reserve_slot(&mut self, slot: usize) {
        if self.players.len() <= slot {
            self.players.resize(slot + 1, None);
        }
        self.sync_slot_tables();
    }

    /// Re-align `assist`/`magnet`/`reticle_pick`/`spawn_choice`/`prev_spawn_bits`/
    /// `spawn_post_choice` with
    /// `players` (callers may assign into `players` directly — it is a public
    /// field).
    pub fn sync_slot_tables(&mut self) {
        let n = self.players.len();
        self.assist.resize(n, AssistState::default());
        self.assist_lead.resize(n, None);
        self.magnet.resize(n, None);
        self.magnet_hidden.resize(n, 0);
        self.reticle_pick.resize(n, None);
        self.spawn_choice.resize(n, 0);
        self.prev_spawn_bits.resize(n, 0);
        self.spawn_post_choice.resize(n, None);
        self.spawn_stage.resize(n, SpawnStage::Accepted);
        self.side_choice.resize(n, SideChoice::Team1);
        self.class_carousel.resize(n, Carousel::default());
        self.class_ammo.resize(n, Vec::new());
    }

    /// Card SSELA: put human slot `slot` behind the spawn screens - the game
    /// binary's level start. Turns [`Sim::spawn_screens`] on and takes the
    /// slot's live unit OFF the field without spending a reinforcement (as
    /// [`Sim::vacate_player_slot`] does), but the slot keeps it: the held
    /// dead unit is what the class accept brings to life
    /// ([`Sim::respawn_slot`]), so the slot is never without a unit to point
    /// at. The screens open on the side select when `side_choice` (the PS2's
    /// "can switch sides": Instant Action), else on the spawn map (the
    /// campaign, `B2:392469-392471`). `false` when the slot has no live unit.
    pub fn open_spawn_screens(&mut self, slot: usize, side_choice: bool) -> bool {
        let Some(h) = self.players.get(slot).copied().flatten() else {
            return false;
        };
        let Some(u) = self.unit(h) else {
            return false;
        };
        let team = u.team;
        let i = h.index as usize;
        self.units[i].alive = false;
        self.units[i].respawn_at = None;
        self.units[i].spawn_slot = None;
        let t = &mut self.battle.teams[usize::from(team.min(1))];
        t.alive_players = t.alive_players.saturating_sub(1);
        self.events.push(Event::Despawn { unit: h });
        self.retire_saber_flights(i);
        self.unseat_unit(h);
        self.class_change_life_ended(h);
        crate::squad::on_unit_died(self, h);
        self.spawn_screens = true;
        self.sync_slot_tables();
        self.side_choice[slot] = SideChoice::Team1;
        if side_choice {
            self.spawn_stage[slot] = SpawnStage::Side;
        } else {
            self.open_spawn_map(slot, team);
        }
        true
    }

    /// Open slot `s`'s spawn map for `team`: its post is the one it chose
    /// while that is still the team's, else the nearest (the PS2 copies the
    /// member's chosen post, keep-or-nearest `FUN_0036ca08`, re-spawn-flow
    /// §1 / §3), else the team's first spawn post - the PS2's default pick
    /// `FUN_0036cca0` is not read (PROVISIONAL).
    pub(in crate::sim) fn open_spawn_map(&mut self, s: usize, team: u8) {
        self.spawn_stage[s] = SpawnStage::Map;
        let post = self.spawn_post_choice[s]
            .and_then(|c| self.keep_or_nearest_post(team, c))
            .or_else(|| self.player_spawn_posts(team).first().map(|q| q.0));
        self.spawn_post_choice[s] = post;
    }

    /// How likely each class of `team` is to be the next AI spawn (the maintainer,
    /// 2026-10-01 — ours, replacing the walk of the `AddUnitClass` minimums;
    /// the PS2's own rule is untraced, card AICLS).
    ///
    /// The team's alive-AI cap is shared out by [`class_share`]: the default
    /// class 4 parts, a standard class 2, a special class 1 — on a BF2 team
    /// of 16 that is 5.3 riflemen, 2.7 of each standard class and 1.3 of each
    /// special one; on a BF1 team of 32, 11.6 / 5.8 / 2.9. A class's weight
    /// is the part of its share not yet alive, plus a small floor
    /// ([`CLASS_FLOOR`] of an EVEN share, the same for every class): each one
    /// alive takes one unit off its class's weight; a class at or past its
    /// share keeps only the floor, so sixteen riflemen or sixteen droidekas
    /// can happen and almost never do. [ours] (the maintainer 2026-10-29) a
    /// class's authored `max` (BF2 `SetupTeams`, the droideka's 4) caps
    /// neither AI rolls nor human selection. Human refusal checks points only.
    pub fn ai_class_weights(&self, team: u8) -> Vec<f32> {
        let table = &self.classes[usize::from(team.min(1))];
        let cap = self.ai_limit(team).max(1) as f32;
        let hero = self.hero_class(team).map(usize::from);
        let (len, hero) = self.test_rosters[usize::from(team.min(1))]
            .map_or((table.len(), hero), |(len, hero)| {
                (len, hero.map(usize::from))
            });
        let ordinary = len - usize::from(hero.is_some());
        let parts: f32 = (0..len).filter(|c| Some(*c) != hero).map(class_share).sum();
        let floor = CLASS_FLOOR * cap / ordinary.max(1) as f32;
        (0..table.len())
            .map(|c| {
                if c >= len || Some(c) == hero {
                    return 0.0;
                }
                let alive = self
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive && !u.is_player && u.team == team && usize::from(u.spec.class) == c
                    })
                    .count() as u32;
                let share = cap * class_share(c) / parts;
                (share - alive as f32).max(0.0) + floor
            })
            .collect()
    }

    /// Which class the team's NEXT AI spawn takes: one draw of the sim's LCG
    /// over [`Sim::ai_class_weights`]. Class 0 only for an empty or zero-weight table.
    pub fn ai_class(&mut self, team: u8) -> u8 {
        let w = self.ai_class_weights(team);
        let total: f32 = w.iter().sum();
        if total <= 0.0 {
            return 0;
        }
        let mut r = self.lcg.unit() * total;
        let mut last = 0;
        for (c, &wc) in w.iter().enumerate() {
            if wc <= 0.0 {
                continue;
            }
            last = c;
            if r < wc {
                return c as u8;
            }
            r -= wc;
        }
        last as u8
    }

    /// Spawn one AI of the team's next class at `pos`/`yaw`. `None` (and no
    /// cursor advance) when the battle refuses the spawn.
    pub fn spawn_ai(&mut self, team: u8, pos: Vec3, yaw: f32) -> Option<UnitHandle> {
        self.spawn_ai_with_slot_policy(team, pos, yaw, false)
    }

    /// Grow a team's roster while preserving every pending respawn and its wave.
    pub fn spawn_ai_roster_slot(&mut self, team: u8, pos: Vec3, yaw: f32) -> Option<UnitHandle> {
        self.spawn_ai_with_slot_policy(team, pos, yaw, true)
    }

    fn spawn_ai_with_slot_policy(
        &mut self,
        team: u8,
        pos: Vec3,
        yaw: f32,
        preserve_respawns: bool,
    ) -> Option<UnitHandle> {
        let t = usize::from(team.min(1));
        if self.classes[t].is_empty() {
            return None;
        }
        let c = self.ai_class(team);
        let spec = self.classes[t][usize::from(c)].spec(
            team,
            pos,
            yaw,
            Controller::Ai,
            c,
            self.bot_skill(team),
        );
        let h = self.spawn_with_slot_policy(&spec, preserve_respawns)?;
        self.ai_spawned[t] += 1;
        Some(h)
    }

    /// Rebuild dead slot `i` at a friendly spawn node (new generation).
    ///
    /// A BOT spawns at one node of the post it drew
    /// ([`Sim::keep_or_draw_spawn_post`], card BSPN): post first, then node,
    /// so a post's share of the spawns does not grow with its node count.
    /// A HUMAN spawns at a node of the post the spawn map selected for the
    /// slot, through keep-or-nearest (card SPSEL, [`Sim::keep_or_nearest_post`]);
    /// one who never picked still draws a node from the flat list of every
    /// post's ([`Sim::spawn_points`]).
    pub(crate) fn respawn_slot(&mut self, i: usize) {
        self.respawn_slot_with_class(i, None);
    }

    pub(in crate::sim) fn respawn_slot_with_class(&mut self, i: usize, forced: Option<u8>) {
        let team = self.units[i].team;
        let is_player = self.units[i].is_player;
        let slot = self.units[i].spec.player_slot.map(usize::from);
        let forced =
            forced.filter(|c| usize::from(*c) < self.classes[usize::from(team.min(1))].len());
        let player_class = if let Some(s) =
            slot.filter(|_| forced.is_none() && !self.classes[usize::from(team.min(1))].is_empty())
        {
            let Some(class) = self.spawn_class_choice(s, team) else {
                return;
            };
            self.spawn_choice[s] = class as u8;
            Some(class)
        } else {
            None
        };
        let chosen = slot.and_then(|s| self.spawn_post_choice.get(s).copied().flatten());
        let points = if let (Some(s), Some(chosen)) = (slot, chosen) {
            // The PS2 writes the kept-or-nearest post back to the member.
            let post = self.keep_or_nearest_post(team, chosen);
            self.spawn_post_choice[s] = post;
            post.map_or_else(Vec::new, |p| self.spawn_post_nodes(team, p))
        } else if is_player {
            self.spawn_points(team)
        } else {
            match self.keep_or_draw_spawn_post(i) {
                Some(post) => self.spawn_post_nodes(team, post),
                None => Vec::new(),
            }
        };
        if points.is_empty() {
            return;
        }
        if !self.admit(team, is_player) {
            return;
        }
        // Uniform node pick via the inline LCG (BF1 FUN_0029df70 / BF2
        // FUN_00356998 draw), then ground snap.
        let (pos, yaw) = points[self.lcg.index(points.len() as u32) as usize];
        let gen = self.units[i].gen + 1;
        let old = self.units[i].spec.clone();
        // A PLAYER comes back as the class their spawn screen chose (D5). A
        // choice past the end of the table clamps to the last class, so a
        // table that shrank between missions cannot index out of bounds.
        // An AI re-rolls its class by the same weights (the maintainer, 2026-10-01); it
        // is dead here, so it does not count against its old class.
        let ai_class = (old.player_slot.is_none()
            && old.controller == Controller::Ai
            && !self.classes[usize::from(old.team.min(1))].is_empty())
        .then(|| self.ai_class(old.team));
        let base = self.bot_skill(old.team);
        let table = &self.classes[usize::from(old.team.min(1))];
        let spec = if let Some(class) = forced.filter(|c| usize::from(*c) < table.len()) {
            table[usize::from(class)].spec(old.team, pos, yaw, old.controller, class, base)
        } else {
            match old.player_slot.filter(|_| !table.is_empty()) {
                Some(_) => {
                    let c = player_class.expect("unforced player class resolved before admission");
                    table[c].spec(old.team, pos, yaw, old.controller, c as u8, base)
                }
                None => match ai_class {
                    // [ours] A proxy with a class picks it, as a player does (the
                    // draw above is still spent).
                    Some(c) => {
                        let c = self.units[i]
                            .proxy_human
                            .and_then(|p| p.class)
                            .filter(|&k| {
                                usize::from(k) < table.len() && Some(k) != self.hero_class(old.team)
                            })
                            .unwrap_or(c);
                        table[usize::from(c)].spec(old.team, pos, yaw, old.controller, c, base)
                    }
                    None => UnitSpec { pos, yaw, ..old },
                },
            }
        };
        let mut unit = spec.build(gen, &self.weapon_classes);
        // An AI respawn re-derives the tier's skill (D2): a difficulty change
        // between deaths reaches the bot at its next life, and a bot never
        // comes back with a stale level.
        unit.skill = crate::skill_for(spec.controller, self.bot_skill(spec.team));
        unit.spec.skill = unit.skill;
        unit.soldier.pos.y = self.spawn_ground(pos);
        unit.kills = self.units[i].kills;
        unit.deaths = self.units[i].deaths;
        // Card DPRX: a proxy comes back a proxy.
        unit.proxy_human = self.units[i].proxy_human;
        crate::ai::proxy::hand_weapons(&mut unit);
        self.units[i] = unit;
        let h = UnitHandle {
            index: i as u32,
            gen,
        };
        // The respawned unit keeps its player slot.
        if let Some(slot) = self.units[i].spec.player_slot {
            let slot = usize::from(slot);
            if self.players.get(slot).copied().flatten().map(|p| p.index) == Some(i as u32) {
                self.players[slot] = Some(h);
            }
        }
        self.events.push(Event::Respawn { unit: h });
    }

    /// Step each DEAD player's class choice from its pad (D5). Runs at the
    /// top of `step`, before the respawn loop, so a press this tick reaches
    /// the respawn this tick.
    ///
    /// Edges come from `prev_spawn_bits` rather than `Soldier::prev_buttons`
    /// because a dead soldier does not step, so its own button history goes
    /// stale. The bits are recorded for EVERY slot with a pad, alive or dead,
    /// so a press held across a death does not fire the moment the player
    /// dies. At most one step per tick; `NEXT` wins if both rise. An alive
    /// player's press is ignored outright.
    ///
    /// A slot with no unit at all takes team 0's table — the only team a
    /// fresh sim's first slot can belong to.
    ///
    /// The POST bits (`SPAWN_POST_*`, card SPSEL) move the slot's
    /// `spawn_post_choice` the same way, edge-triggered: the arms held this
    /// tick combine into one push (Right +x, Down +z on the map — BF2 Hoth's
    /// spawn map, `lead_tmp/spsel/fit_dir.py`: every map angle within −12° to
    /// +26° of that fits the live presses; the PS2 turns the push by the map's
    /// angle, `FUN_003a8bb0`, not read: 0 is PROVISIONAL), taken from the
    /// selected post by [`Sim::pick_spawn_post`]. A slot that never picked
    /// pushes from the post nearest its body, and takes that post when none
    /// is ahead.
    ///
    /// With [`Sim::spawn_screens`] on (card SSELA) the bits go to the screen
    /// the slot has up instead ([`Sim::spawn_screen_input`]), and
    /// `SPAWN_ACCEPT` / `SPAWN_BACK` join the edge mask.
    pub(in crate::sim) fn spawn_select(&mut self, pads: &[PadState]) {
        if self.spawn_choice.len() != self.players.len()
            || self.spawn_post_choice.len() != self.players.len()
            || self.spawn_stage.len() != self.players.len()
            || self.class_carousel.len() != self.players.len()
        {
            self.sync_slot_tables();
        }
        const CLASS: u32 = buttons::SPAWN_CLASS_NEXT | buttons::SPAWN_CLASS_PREV;
        const POST: u32 = buttons::SPAWN_POST_UP
            | buttons::SPAWN_POST_DOWN
            | buttons::SPAWN_POST_LEFT
            | buttons::SPAWN_POST_RIGHT;
        const SCREEN: u32 = buttons::SPAWN_ACCEPT | buttons::SPAWN_BACK;
        // Issue #28: a live slot's class screen reads the screen bits too.
        let mask = if self.spawn_screens || self.spawn_stage.contains(&SpawnStage::Alive) {
            CLASS | POST | SCREEN
        } else {
            CLASS | POST
        };
        for s in 0..self.players.len() {
            let held = pads.get(s).map_or(0, |p| p.buttons);
            let bits = held & mask;
            let rose = bits & !self.prev_spawn_bits[s];
            self.prev_spawn_bits[s] = bits;
            let alive_screen = self.spawn_stage[s] == SpawnStage::Alive;
            let carousel =
                alive_screen || (self.spawn_screens && self.spawn_stage[s] == SpawnStage::Class);
            if rose == 0 && !carousel {
                continue;
            }
            let unit = self.players[s].and_then(|h| self.units.get(h.index as usize));
            match unit {
                // Alive: no spawn screen is up, so the bits do nothing -
                // unless the slot opened the class screen at a post (#28).
                Some(u) if u.alive && !alive_screen => continue,
                _ => {}
            }
            let team = unit.map_or(0, |u| u.team);
            let body = unit.map(|u| u.soldier.pos);
            if carousel {
                // The step runs every update on the HELD push, before the
                // accept (`FUN_00398000` B2:393025-393027, then its X).
                self.class_carousel_step(s, team, bits);
                if rose == 0 {
                    continue;
                }
            }
            if self.spawn_screens || alive_screen {
                self.spawn_screen_input(s, team, body, bits, rose, held);
                continue;
            }
            if rose & POST != 0 {
                self.spawn_post_push(s, team, body, bits);
            }
            if rose & CLASS != 0 {
                self.move_spawn_class(s, team, rose & buttons::SPAWN_CLASS_NEXT != 0);
            }
        }
    }

    /// Card SSELA: one tick's rising bits `rose` (held: `bits`, the whole pad
    /// `held`) on the screen slot `s` has up, the PS2's table
    /// (re-spawn-flow §1-§4): X accepts (side -> map; map -> class only with
    /// a post selected; class -> the spawn, this same tick), Triangle backs
    /// only on the class screen, and only while X is not rising. Side: Up /
    /// Down over team 1, team 2, Auto Assign. Map: the post push. Class:
    /// D-pad Left = next class, Right = previous (the PS2's carousel), and
    /// `SPAWN_CLASS_NEXT` / `PREV` as before.
    fn spawn_screen_input(
        &mut self,
        s: usize,
        team: u8,
        body: Option<Vec3>,
        bits: u32,
        rose: u32,
        held: u32,
    ) {
        let accept = rose & buttons::SPAWN_ACCEPT != 0;
        let back = rose & buttons::SPAWN_BACK != 0;
        match self.spawn_stage[s] {
            SpawnStage::Side if accept => {
                // Named deviation (re-spawn-flow §9, reversible): the PS2
                // offline keeps team 1 on Auto Assign; the port flips the
                // PS2's own coin (`FUN_0036fc40`: `((LCG >> 16) & 1) + 1`)
                // on the sim's LCG, as the maintainer asked for "a side (or random)".
                let team = match self.side_choice[s] {
                    SideChoice::Team1 => 0,
                    SideChoice::Team2 => 1,
                    SideChoice::Auto => ((self.lcg.step() >> 16) & 1) as u8,
                };
                if let Some(h) = self.players[s] {
                    let u = &mut self.units[h.index as usize];
                    u.team = team;
                    u.spec.team = team;
                }
                self.open_spawn_map(s, team);
            }
            SpawnStage::Side => {
                let step = i32::from(rose & buttons::SPAWN_POST_DOWN != 0)
                    - i32::from(rose & buttons::SPAWN_POST_UP != 0);
                self.side_choice[s] = self.side_choice[s].moved(step);
            }
            SpawnStage::Map if accept => {
                if self.spawn_post_selected(s).is_some() {
                    self.spawn_stage[s] = SpawnStage::Class;
                    // The hero's owner finds the hero chosen (FUN_00396a00
                    // selects its row on entry); any class stays his to pick.
                    if let Some(hero) = self.owned_hero_class(s, team) {
                        self.spawn_choice[s] = hero;
                    }
                    // The arm is shared with the map's push (`SD+0x25`,
                    // `FUN_0039a058`): a push held into the class screen
                    // steps only after it is let go.
                    self.class_carousel[s].armed = bits & CAROUSEL != 0;
                }
            }
            SpawnStage::Map => {
                const POST: u32 = buttons::SPAWN_POST_UP
                    | buttons::SPAWN_POST_DOWN
                    | buttons::SPAWN_POST_LEFT
                    | buttons::SPAWN_POST_RIGHT;
                if rose & POST != 0 {
                    self.spawn_post_push(s, team, body, bits);
                }
            }
            SpawnStage::Class if accept => {
                let Some(h) = self.players[s] else {
                    return;
                };
                // A refused class plays the back sound and stays
                // (`FUN_00398000`, B2:393070-393077).
                let n = self.classes[usize::from(team.min(1))].len();
                let c = usize::from(self.spawn_choice[s]).min(n.saturating_sub(1));
                if self.class_refusal(s, team, c).is_some() {
                    return;
                }
                let i = h.index as usize;
                self.respawn_slot(i);
                if self.units[i].alive {
                    self.spawn_stage[s] = SpawnStage::Accepted;
                    // The X that spawned it is not the new soldier's jump.
                    self.units[i].soldier.set_prev_buttons(held);
                }
            }
            SpawnStage::Class if back => self.spawn_stage[s] = SpawnStage::Map,
            // The carousel steps in `class_carousel_step`, every tick.
            SpawnStage::Class => {}
            // Issue #28, the PS2's state 4: X accepts as on the class screen
            // (`B2:393063-393132`), Triangle goes back to play (`B2:393175`).
            SpawnStage::Alive if accept => self.class_change_accept(s, team, held),
            SpawnStage::Alive if back => self.class_change_close(s, held),
            SpawnStage::Alive => {}
            SpawnStage::Accepted => {}
        }
    }

    /// Card SSELB: one update of slot `s`'s class carousel on the class
    /// screen, `bits` held (the PS2's `FUN_00399dd8`, re-spawn-flow §4.1):
    /// D-pad Left / `SPAWN_CLASS_NEXT` = the next class, Right / `PREV` = the
    /// previous, wrapping; a push steps once (re-armed when let go), never
    /// while the line-up slides, and a push held through the slide steps
    /// again as it ends. Then the slide advances (`FUN_0039a980`, run only
    /// on the class screen, so a slide left by Back resumes on return).
    fn class_carousel_step(&mut self, s: usize, team: u8, bits: u32) {
        let next = bits & (buttons::SPAWN_CLASS_NEXT | buttons::SPAWN_POST_LEFT) != 0;
        let prev = bits & CAROUSEL != 0 && !next;
        if let Some(class) = self.spawn_class_choice(s, team) {
            self.spawn_choice[s] = class as u8;
        }
        let mut k = self.class_carousel[s];
        if k.armed {
            // Let go: re-armed (B2:393680-393682).
            if !(next || prev) {
                k.armed = false;
            }
        } else if (next || prev) && k.slide == 0 {
            self.move_spawn_class(s, team, next);
            k.armed = true;
            k.slide = CLASS_SLIDE_TICKS;
        }
        k.slide = k.slide.saturating_sub(1);
        self.class_carousel[s] = k;
    }

    /// Classes shown to this member. BF2 0x0039a888/0x00399dd8 exposes
    /// the hero row only to the team's current hero owner.
    pub fn spawn_class_indices(&self, s: usize, team: u8) -> Vec<usize> {
        (0..self.classes[usize::from(team.min(1))].len())
            .filter(|&class| self.spawn_class_available(s, team, class))
            .collect()
    }

    /// Class membership, shared by the carousel, UI and unforced respawn.
    /// Ordinary unlock costs remain the separate confirmation refusal.
    pub fn spawn_class_available(&self, s: usize, team: u8, class: usize) -> bool {
        if let Some((len, hero)) = self.test_rosters[usize::from(team.min(1))] {
            if class >= len
                || (hero.map(usize::from) == Some(class)
                    && self.hero_class(team).map(usize::from) != Some(class))
            {
                return false;
            }
        }
        if class >= self.classes[usize::from(team.min(1))].len() {
            return false;
        }
        if self.hero_class(team).map(usize::from) != Some(class) {
            return true;
        }
        let Some(member) = self.players.get(s).copied().flatten() else {
            return false;
        };
        self.units
            .get(member.index as usize)
            .is_some_and(|u| u.gen == member.gen && u.team == team)
            && self.heroes.as_ref().is_some_and(|heroes| {
                let state = &heroes.teams[usize::from(team)];
                state.unlocked && state.owner == Some(member.index)
            })
    }

    /// The selected raw class index, falling back to the first visible row
    /// if the old hero reservation ended or a mission's table shrank.
    pub fn spawn_class_choice(&self, s: usize, team: u8) -> Option<usize> {
        let n = self.classes[usize::from(team.min(1))].len();
        let c =
            usize::from(self.spawn_choice.get(s).copied().unwrap_or(0)).min(n.saturating_sub(1));
        if self.spawn_class_available(s, team, c) {
            Some(c)
        } else {
            (0..n).find(|&class| self.spawn_class_available(s, team, class))
        }
    }

    fn move_spawn_class(&mut self, s: usize, team: u8, next: bool) {
        let classes = self.spawn_class_indices(s, team);
        if classes.is_empty() {
            return;
        }
        let selected = self.spawn_class_choice(s, team);
        let c = classes
            .iter()
            .position(|&c| Some(c) == selected)
            .unwrap_or(0);
        let pick = if next {
            (c + 1) % classes.len()
        } else {
            (c + classes.len() - 1) % classes.len()
        };
        self.spawn_choice[s] = classes[pick] as u8;
    }

    /// Card SSELB: player slot `s`'s points ([`Sim::spawn_points`]).
    pub fn set_spawn_points(&mut self, s: usize, points: i32) {
        if self.spawn_points.len() <= s {
            self.spawn_points.resize(s + 1, 0);
        }
        self.spawn_points[s] = points;
    }

    /// Card UNLK3: player slot `s`'s cost of each of `team`'s classes
    /// ([`Sim::spawn_costs`]).
    pub fn set_spawn_costs(&mut self, s: usize, team: u8, costs: Vec<u32>) {
        if self.spawn_costs.len() <= s {
            self.spawn_costs.resize(s + 1, Default::default());
        }
        self.spawn_costs[s][usize::from(team.min(1))] = costs;
    }

    /// Player slot `s`'s cost of `team`'s class `class`: its career's
    /// ([`Sim::spawn_costs`]), else the class's `unlock_points`.
    pub fn spawn_cost(&self, s: usize, team: u8, class: usize) -> u32 {
        let own = self.classes[usize::from(team.min(1))]
            .get(class)
            .map_or(0, |c| c.unlock_points);
        self.spawn_costs
            .get(s)
            .and_then(|t| t[usize::from(team.min(1))].get(class))
            .copied()
            .unwrap_or(own)
    }

    /// How many of `team`'s units are on the field in class `class` (the
    /// panel's "Unit Count", `FUN_0036f538`: members with a live unit).
    pub fn class_count(&self, team: u8, class: usize) -> u32 {
        self.units
            .iter()
            .filter(|u| u.alive && u.team == team && usize::from(u.spec.class) == class)
            .count() as u32
    }

    /// Card SSELB: whether the class screen refuses player slot `s` class
    /// `class` of `team`, as the PS2's `FUN_00364290` does
    /// (`analysis/re-spawn-flow.md` §4.3): under the class's
    /// `PointsToUnlock` with the slot's [`Sim::spawn_points`]. Bots never ask
    /// (the PS2 checks a human only). `[ours]` (card UNLK3): the points test
    /// reads the slot's career cost ([`Sim::spawn_cost`]). `[ours]`
    /// (2026-10-29): no "too many" test - the PS2 refuses an unlock class
    /// whose team fields its max; here a class is never full, for players or
    /// bots.
    pub fn class_refusal(&self, s: usize, team: u8, class: usize) -> Option<ClassRefusal> {
        if self.hero_class(team).map(usize::from) == Some(class)
            && !self.spawn_class_available(s, team, class)
        {
            return Some(ClassRefusal::HeroUnavailable);
        }
        let cost = i64::from(self.spawn_cost(s, team, class));
        let points = i64::from(self.spawn_points.get(s).copied().unwrap_or(0));
        (cost != 0 && points < cost).then(|| ClassRefusal::Points((cost - points) as u32))
    }

    /// One spawn-map push for slot `s` of `team` from the arms held in
    /// `bits` (see [`Sim::spawn_select`]).
    fn spawn_post_push(&mut self, s: usize, team: u8, body: Option<Vec3>, bits: u32) {
        let arm = |b: u32| if bits & b != 0 { 1.0 } else { 0.0 };
        let dir = Vec2::new(
            arm(buttons::SPAWN_POST_RIGHT) - arm(buttons::SPAWN_POST_LEFT),
            arm(buttons::SPAWN_POST_DOWN) - arm(buttons::SPAWN_POST_UP),
        );
        let posts = self.player_spawn_posts(team);
        let current = match self.spawn_post_choice[s] {
            Some(c) => self.keep_or_nearest_post(team, c),
            None => body.and_then(|at| nearest_post(&posts, at)),
        };
        let Some(current) = current.or_else(|| posts.first().map(|q| q.0)) else {
            return;
        };
        let Some(origin) = posts.iter().find(|q| q.0 == current).map(|q| q.1) else {
            return;
        };
        let pick = Self::pick_spawn_post(&posts, Some(current), origin, dir);
        self.spawn_post_choice[s] = Some(pick.unwrap_or(current));
    }
}

/// The post in `posts` nearest to `at` (the first on a tie).
fn nearest_post(posts: &[(SpawnPost, Vec3)], at: Vec3) -> Option<SpawnPost> {
    posts
        .iter()
        .min_by(|a, b| {
            a.1.distance_squared(at)
                .total_cmp(&b.1.distance_squared(at))
        })
        .map(|q| q.0)
}
