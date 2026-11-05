//! Bounded capsule-clear floor floods and paths used by spawn and navigation checks.
#[cfg(doc)]
use crate::sim::spawn::PEN_CELLS;

use crate::sim::spawn::{flood_key, PEN_CELL, PEN_CLEAR, PEN_DROP};
use crate::sim::Sim;
use glam::{Vec2, Vec3};
use std::collections::HashSet;

impl Sim {
    /// How many [`PEN_CELL`] cells of floor a soldier standing at `from`
    /// walks to, counted up to `cap`: a flood over the grid, a step between
    /// two cells being one with a floor he stands on at its end (no steeper
    /// than 45 degrees up, no more than [`PEN_DROP`] down) and no wall
    /// between — the sweep of the stepped capsule, from the higher floor of
    /// the two, and the wall pass leaving him where he then stands. Card
    /// LLEG: under [`PEN_CELLS`] the place is a pen. PROVISIONAL (ours).
    /// measure: original-game footing and exits at flagged spawn pens before replacing the
    /// floor-cell spacing and reach cap.
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
        let mut seen = HashSet::from([flood_key(0, 0, from.y)]);
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
                let next = (ix + dx, iz + dz);
                if let Some(f) = self.flood_step(p, (dx, dz), next, &mut seen) {
                    queue.push_back((next.0, next.1, f, d + 1));
                }
            }
        }
        (cells, None)
    }

    /// One step of the floor flood from the floor point `a` by one
    /// [`PEN_CELL`] along (`dx`, `dz`): the floor a soldier stands on at its
    /// end ([`Sim::floor_flood`]'s rule). Record a new cell only after the
    /// step succeeds so another edge can still reach it when this one fails.
    fn flood_step(
        &self,
        a: Vec3,
        (dx, dz): (i32, i32),
        next: (i32, i32),
        seen: &mut HashSet<(i32, i32, i32)>,
    ) -> Option<f32> {
        use crate::collision::Capsule;
        let height = crate::unit::capsule_height(crate::soldier::Stance::Stand);
        let radius = crate::unit::SOLDIER_RADIUS;
        let dir = Vec3::new(dx as f32, 0.0, dz as f32);
        let b = a + dir * PEN_CELL;
        let (f, n) = self.ground_under_n(b, false, a.y + PEN_CELL);
        if f <= crate::collision::NO_GROUND
            || n.y <= crate::soldier::FLOOR_MIN_NY
            || f - a.y > PEN_CELL
            || a.y - f > PEN_DROP
        {
            return None;
        }
        // Resolve the floor before checking the key: another storey at this
        // XZ is still a new cell. An accepted cell cannot enter the queue again.
        let key = flood_key(next.0, next.1, f);
        if seen.contains(&key) {
            return None;
        }
        let base = Vec3::new(a.x, a.y.max(f), a.z);
        let cap = Capsule {
            base,
            height,
            radius,
        }
        .stepped();
        if self.collision.wall_sweep(&cap, dir, PEN_CELL).is_some() {
            return None;
        }
        let end = Capsule {
            base: Vec3::new(b.x, f, b.z),
            height,
            radius,
        };
        (self.collision.push_out_soldier(&end).length() <= PEN_CLEAR).then(|| {
            seen.insert(key);
            f
        })
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
    /// measure: accepted and rejected authored foot arcs with full-capsule departure witnesses
    /// before replacing the path band or cell budget.
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
        let mut seen = HashSet::from([flood_key(0, 0, from.y)]);
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
            let cell = Vec3::new(p.x, y, p.y);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let q = p + Vec2::new(dx as f32, dz as f32) * PEN_CELL;
                if off_line(q) > band {
                    continue;
                }
                let next = (ix + dx, iz + dz);
                if let Some(f) = self.flood_step(cell, (dx, dz), next, &mut seen) {
                    heap.push((Reverse(prio(q)), next.0, next.1, f.to_bits()));
                }
            }
        }
        (Some(false), near.1)
    }
}
