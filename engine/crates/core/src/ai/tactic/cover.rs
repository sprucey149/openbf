//! Cover direction masks and spatial position-table queries.

use super::{
    cell_of, fmath, nav, Position, PositionTable, Source, TableStats, Vec2, Vec3, DIRS, FACING_COS,
};

#[cfg(doc)]
use super::NEAR;

impl Position {
    /// Any cover at all.
    pub fn covered(&self) -> bool {
        self.low | self.corner != 0
    }
    /// Something shut at crouch height toward `d` within [`NEAR`]: the
    /// position hides a soldier from a target that way (the line to fire
    /// along is the Tactician's confirming ray, not the 30 m lane).
    pub fn shields_toward(&self, d: Vec2) -> bool {
        self.shut & sector_pair(d) != 0
    }
    /// The PS2's own cover test of an authored Cover hint
    /// (`analysis/re-soldier-ai.md` 4.6, `FUN_002d1928`): its arrow points at
    /// the target, `d` away, within 60 degrees. No ray - the hint's direction
    /// is its only cover geometry.
    pub fn hint_faces(&self, d: Vec2) -> bool {
        self.source == Source::Cover && self.facing.dot(d.normalize_or_zero()) >= FACING_COS
    }
    /// Cover toward `d` for the Tactician: something shut at crouch height
    /// that way ([`Self::shields_toward`]), or a Cover hint facing it
    /// ([`Self::hint_faces`]).
    pub fn hides_toward(&self, d: Vec2) -> bool {
        self.shields_toward(d) || self.hint_faces(d)
    }
}
/// The sector bit of direction `d` (XZ); 0 for a zero vector.
pub fn sector_bit(d: Vec2) -> u16 {
    if d.length_squared() < 1e-8 {
        return 0;
    }
    let a = fmath::atan2(d.y, d.x).rem_euclid(std::f32::consts::TAU);
    let k = (a / std::f32::consts::TAU * DIRS as f32).round() as usize % DIRS;
    1 << k
}
/// The bits of the two sector rays either side of direction `d` (one bit
/// when it lies on a ray; 0 for a zero vector): the masks sample one ray per
/// sector, and a direction between two is covered when either is.
pub fn sector_pair(d: Vec2) -> u16 {
    if d.length_squared() < 1e-8 {
        return 0;
    }
    let a = fmath::atan2(d.y, d.x).rem_euclid(std::f32::consts::TAU);
    let x = a / std::f32::consts::TAU * DIRS as f32;
    (1 << (x.floor() as usize % DIRS)) | (1 << (x.ceil() as usize % DIRS))
}
impl PositionTable {
    /// Issue #168: does this table share its positions and cells with
    /// `other` (a clone)?
    pub fn shares_with(&self, other: &PositionTable) -> bool {
        crate::shared::Shared::ptr_eq(&self.positions, &other.positions)
            && crate::shared::Shared::ptr_eq(&self.cells, &other.cells)
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// The indices of the positions within `r` (XZ) of `p`, in table order.
    pub fn within(&self, p: Vec3, r: f32, out: &mut Vec<u32>) {
        out.clear();
        let (x0, z0) = cell_of(p.x - r, p.z - r);
        let (x1, z1) = cell_of(p.x + r, p.z + r);
        for cx in x0..=x1 {
            for (_, &(a, b)) in self.cells.range((cx, z0)..=(cx, z1)) {
                for i in a..b {
                    if nav::xz_dist(self.positions[i as usize].pos, p) <= r {
                        out.push(i);
                    }
                }
            }
        }
    }

    pub(crate) fn index(mut positions: Vec<Position>, stats: TableStats) -> Self {
        // A stable sort: one cell's points keep their build order.
        positions.sort_by_key(|q| cell_of(q.pos.x, q.pos.z));
        let mut cells = std::collections::BTreeMap::new();
        let mut a = 0usize;
        while a < positions.len() {
            let c = cell_of(positions[a].pos.x, positions[a].pos.z);
            let mut b = a + 1;
            while b < positions.len() && cell_of(positions[b].pos.x, positions[b].pos.z) == c {
                b += 1;
            }
            cells.insert(c, (a as u32, b as u32));
            a = b;
        }
        Self {
            positions: positions.into(),
            cells: cells.into(),
            stats,
        }
    }
}
