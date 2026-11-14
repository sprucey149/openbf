//! Authored barrier containment, crossing and silhouette geometry.

use super::{Barrier, Vec2};
use crate::fmath::FloatExt;

impl Barrier {
    pub fn stops(&self, ty: usize) -> bool {
        self.mask & (1 << ty) != 0
    }

    /// Is `p` (XZ) inside the rectangle?
    pub fn contains(&self, p: Vec2) -> bool {
        self.contains_r(p, 0.0)
    }

    /// Is a disc of radius `r` at `p` (XZ) inside the rectangle? This IS the
    /// PS2's own containment test (`FUN_00267920`: two dot products with the
    /// stored rows, each compared with `SIZE + r`) — `SIZE` is a HALF extent.
    pub fn contains_r(&self, p: Vec2, r: f32) -> bool {
        let d = p - self.centre;
        d.dot(self.axes[0]).abs() <= self.half.x + r && d.dot(self.axes[1]).abs() <= self.half.y + r
    }

    /// Does the XZ segment `a → b` touch the rectangle? Slab clip in the
    /// barrier's own frame.
    pub fn crosses(&self, a: Vec2, b: Vec2) -> bool {
        self.crosses_r(a, b, 0.0)
    }

    /// Does the XZ segment `a → b` touch the rectangle grown by `r` on every
    /// side — the PS2's segment query (`FUN_00267d18`, the `r`-expanded
    /// rectangle against the leg)? Slab clip in the barrier's own frame; a
    /// segment that lies wholly INSIDE the rectangle counts as touching (the
    /// PS2's own edge test does not, because it only ever runs from a point it
    /// already knows is clear — [`Self::contains_r`] is what answers that).
    pub fn crosses_r(&self, a: Vec2, b: Vec2, r: f32) -> bool {
        self.cross_t_r(a, b, r).is_some()
    }

    /// [`Self::crosses_r`], and WHERE along `a → b` (0..1) the segment first
    /// touches the grown rectangle: 0 from inside it.
    pub fn cross_t_r(&self, a: Vec2, b: Vec2, r: f32) -> Option<f32> {
        self.clip_t(a, b, self.half + Vec2::splat(r))
    }

    /// Where along the XZ segment `a → b` (0..1) it first touches the
    /// rectangle; `None` when it does not, or when `a` is already inside.
    pub fn entry_t(&self, a: Vec2, b: Vec2) -> Option<f32> {
        if self.contains(a) {
            return None;
        }
        self.clip_t(a, b, self.half)
    }

    fn clip_t(&self, a: Vec2, b: Vec2, half: Vec2) -> Option<f32> {
        let local = |p: Vec2| {
            let d = p - self.centre;
            Vec2::new(d.dot(self.axes[0]), d.dot(self.axes[1]))
        };
        let (p0, p1) = (local(a), local(b));
        let d = p1 - p0;
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for k in 0..2 {
            let (p, q, h) = (d[k], p0[k], half[k]);
            if p.abs() < 1e-9 {
                if q.abs() > h {
                    return None; // parallel and outside the slab
                }
                continue;
            }
            let (mut lo, mut hi) = ((-h - q) / p, (h - q) / p);
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            t0 = t0.fmax(lo);
            t1 = t1.fmin(hi);
            if t0 > t1 {
                return None;
            }
        }
        Some(t0)
    }

    /// The four corners of the rectangle grown by `r`, in the PS2's order
    /// (`FUN_002679f0`): +A+B, +A−B, −A−B, −A+B, where A is the `axes[0]`
    /// (`SIZE.x`) side and B the `axes[1]` (`SIZE.z`) side.
    pub fn corners_r(&self, r: f32) -> [Vec2; 4] {
        let a = self.axes[0] * (self.half.x + r);
        let b = self.axes[1] * (self.half.y + r);
        [
            self.centre + a + b,
            self.centre + a - b,
            self.centre - a - b,
            self.centre - a + b,
        ]
    }

    /// The PS2's `FUN_00268060`: the two SILHOUETTE corners of the rectangle
    /// seen from `p` looking at `to` — per corner v = norm(corner − p), the
    /// corner with the least dot with the line of sight on each side of it
    /// (`[0]` the cross < 0 side, `[1]` the cross > 0 side; `None`: no corner
    /// on that side) — placed on the rectangle grown by `place` (the PS2:
    /// the unit radius + 0.5 m, `FUN_00267ab0`).
    ///
    /// OURS, PROVISIONAL: WHICH corners is asked of the rectangle grown by
    /// `seen` — the one the caller's segment test uses, which `p` is outside
    /// of — where the PS2 asks the placed one. A viewer between the two
    /// (within the corner clearance of a barrier, as every corner of a ring
    /// of rails is of the next rail) sees another pair on the larger one.
    /// Measured (card OBST2): both ways resolve 95 % of the discs' crossing
    /// legs (3,448 of 65,297 fail for 3,619 of 70,081), and the maintainer's Kamino
    /// scene has 7 of 416 trails with over 6 reversals in 10 s the PS2's way
    /// for 2 of 387 this way (main 4 of 422). Asked of the BARE rectangle
    /// 17,282 of 66,497 fail: the segment to such a corner crosses the
    /// tested rectangle.
    /// measure: matched PS2 obstacle detours and port reversal tails before replacing the
    /// separate seen/place corner margins.
    pub fn silhouette(&self, p: Vec2, to: Vec2, seen: f32, place: f32) -> [Option<Vec2>; 2] {
        let dir = (to - p).try_normalize().unwrap_or(Vec2::X);
        let mut best = [(1.0f32, None); 2];
        for (c, at) in self.corners_r(seen).into_iter().zip(self.corners_r(place)) {
            let Some(v) = (c - p).try_normalize() else {
                continue;
            };
            let cross = dir.perp_dot(v);
            if cross == 0.0 {
                continue;
            }
            let k = usize::from(cross > 0.0);
            let d = dir.dot(v);
            if d < best[k].0 {
                best[k] = (d, Some(at));
            }
        }
        best.map(|(_, c)| c)
    }

    /// Half the diagonal of the rectangle — the PS2 keeps `hypot(SIZE.x,
    /// SIZE.z)` at +0x8c for exactly this coarse reject.
    pub fn bound_radius(&self) -> f32 {
        self.half.length()
    }

    /// Does this barrier stop `ty` right now? Enabled AND masked.
    pub(crate) fn blocks(&self, ty: usize) -> bool {
        self.enabled && self.stops(ty)
    }
}
