//! Terrain height query, transcribed from the decomp (BF1 FUN_00473710;
//! BF2 FUN_0055ebc0 is the same routine per audit A2/RE_LOG).
//!
//! The game does NOT bilinear-filter: each grid cell is split into two
//! triangles and the point is lerped on the triangle plane, with the split
//! diagonal ALTERNATING by z-row parity (`iz & 1`):
//! - even row: diagonal from (ix,iz) to (ix+1,iz+1) (xfrac vs zfrac);
//! - odd row: diagonal from (ix+1,iz) to (ix,iz+1) (xfrac vs 1-zfrac).
//!
//! Globals in the original: DAT_00564698/9c = world->grid offsets,
//! DAT_00564690 = gridSize, DAT_00564694 = unit, DAT_005647bc = 1/unit,
//! DAT_005647c0 = heightScale (folded into `heights` here — lerp is linear,
//! prescaling is bit-identical).
//!
//! [`HeightProbe`] answers the same "over or under" question along a marched
//! ray ([`crate::collision::ray_heightfield`]) without the lerp where a cell's
//! corners already decide it, and says how far along the ray that holds.

use glam::Vec3;

/// Row-major (z * size + x) grid of world-space heights.
pub struct Heightfield {
    /// Grid vertices per side.
    pub size: u32,
    /// World units per cell.
    pub unit: f32,
    /// Added to world x/z before the cell lookup (grid min corner negated).
    pub offset_x: f32,
    pub offset_z: f32,
    pub heights: Vec<f32>,
}

impl Heightfield {
    fn h(&self, ix: u32, iz: u32) -> f32 {
        // Clamp like the original's index math: the query clamps the world
        // coordinate to (size-1)*unit, but the +1 neighbors of the last cell
        // still need a guard when the coordinate sits exactly on the edge.
        let ix = ix.min(self.size - 1);
        let iz = iz.min(self.size - 1);
        self.heights[(iz * self.size + ix) as usize]
    }

    /// Clamped XZ coordinates in metres from the grid's minimum corner.
    pub(crate) fn grid_point(&self, x: f32, z: f32) -> (f32, f32) {
        let max = (self.size - 1) as f32 * self.unit;
        (
            (x + self.offset_x).clamp(0.0, max),
            (z + self.offset_z).clamp(0.0, max),
        )
    }

    /// The clamped cell under (x, z) and the fractions inside it.
    fn cell(&self, x: f32, z: f32) -> (u32, u32, f32, f32) {
        let (fx, fz) = self.grid_point(x, z);
        let inv = 1.0 / self.unit;
        let ix = (fx * inv) as u32;
        let iz = (fz * inv) as u32;
        (ix, iz, fx * inv - ix as f32, fz * inv - iz as f32)
    }

    /// (x, z) held on the sheet's rectangle; unchanged when already on it.
    pub fn clamp_xz(&self, x: f32, z: f32) -> (f32, f32) {
        let max = (self.size - 1) as f32 * self.unit;
        let hold = |v: f32, offset: f32| {
            if v + offset < 0.0 {
                -offset
            } else if v + offset > max {
                max - offset
            } else {
                v
            }
        };
        (hold(x, self.offset_x), hold(z, self.offset_z))
    }

    /// The unit up-facing normal of the triangle [`Heightfield::height_at`]
    /// lerps on at (x, z) — BF1 FUN_00473e08, which picks the triangle by the
    /// same z-row parity split (analysis/re-foliage.md §6: the terrain foliage
    /// stands on it).
    pub fn normal_at(&self, x: f32, z: f32) -> [f32; 3] {
        let (ix, iz, xfrac, zfrac) = self.cell(x, z);
        let h = |dx: u32, dz: u32| self.h(ix + dx, iz + dz);
        // d(height)/d(fraction) along x and z, per triangle (see height_at).
        let (gx, gz) = if iz & 1 == 0 {
            if xfrac < zfrac {
                (h(1, 1) - h(0, 1), h(0, 1) - h(0, 0))
            } else {
                (h(1, 0) - h(0, 0), h(1, 1) - h(1, 0))
            }
        } else if xfrac < 1.0 - zfrac {
            (h(1, 0) - h(0, 0), h(0, 1) - h(0, 0))
        } else {
            (h(1, 1) - h(0, 1), h(1, 1) - h(1, 0))
        };
        let n = [-gx, self.unit, -gz];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        [n[0] / len, n[1] / len, n[2] / len]
    }

    /// World-space ground height at (x, z) — FUN_00473710 transcription.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let (ix, iz, xfrac, zfrac) = self.cell(x, z);
        self.height_in(ix, iz, xfrac, zfrac)
    }

    /// [`Heightfield::height_at`]'s lerp inside cell (ix, iz).
    fn height_in(&self, ix: u32, iz: u32, xfrac: f32, zfrac: f32) -> f32 {
        if iz & 1 == 0 {
            if xfrac < zfrac {
                // triangle (ix,iz) (ix,iz+1) (ix+1,iz+1), base at (ix,iz+1)
                let base = self.h(ix, iz + 1);
                base + (self.h(ix + 1, iz + 1) - base) * xfrac
                    + (self.h(ix, iz) - base) * (1.0 - zfrac)
            } else {
                // triangle (ix,iz) (ix+1,iz) (ix+1,iz+1), base at (ix+1,iz)
                let base = self.h(ix + 1, iz);
                base + (self.h(ix + 1, iz + 1) - base) * zfrac
                    + (self.h(ix, iz) - base) * (1.0 - xfrac)
            }
        } else if xfrac < 1.0 - zfrac {
            // triangle (ix,iz) (ix+1,iz) (ix,iz+1), base at (ix,iz)
            let base = self.h(ix, iz);
            base + (self.h(ix + 1, iz) - base) * xfrac + (self.h(ix, iz + 1) - base) * zfrac
        } else {
            // triangle (ix+1,iz) (ix,iz+1) (ix+1,iz+1), base at (ix+1,iz+1)
            let base = self.h(ix + 1, iz + 1);
            base + (self.h(ix, iz + 1) - base) * (1.0 - xfrac)
                + (self.h(ix + 1, iz) - base) * (1.0 - zfrac)
        }
    }
}

/// Is a point over the terrain (`y - height_at(x, z) >= 0`, bit for bit),
/// asked along a marched ray, and how far along the ray that answer holds
/// without asking again. The probe keeps the last cell's lowest and highest
/// corner: each of a cell's triangles lerps its corners with weights that sum
/// to at most 1, so its height lies between them, and a point clear of both
/// is answered without the lerp; it stays answered while the ray keeps
/// `slack` clear of the corners' height and of the cell's sides (card HULLC:
/// the terrain ray's march was the sim's largest single cost).
pub struct HeightProbe<'a> {
    hf: &'a Heightfield,
    cell: (u32, u32),
    lo: f32,
    hi: f32,
}

impl<'a> HeightProbe<'a> {
    pub fn new(hf: &'a Heightfield) -> Self {
        // No cell yet: an empty range answers nothing.
        Self {
            hf,
            cell: (u32::MAX, u32::MAX),
            lo: f32::INFINITY,
            hi: f32::NEG_INFINITY,
        }
    }

    /// Whether `p` is over the terrain, and the distance along `dir` (a
    /// unit vector) over which every point is on the same side, with every
    /// error of computing those points under `slack` metres; 0 when the
    /// point was lerped or lies outside the grid.
    pub fn side(&mut self, p: Vec3, dir: Vec3, slack: f32) -> (bool, f32) {
        let hf = self.hf;
        let (ix, iz, xfrac, zfrac) = hf.cell(p.x, p.z);
        if xfrac.is_nan() || zfrac.is_nan() {
            return (p.y - hf.height_at(p.x, p.z) >= 0.0, 0.0);
        }
        if (ix, iz) != self.cell {
            let c = [
                hf.h(ix, iz),
                hf.h(ix + 1, iz),
                hf.h(ix, iz + 1),
                hf.h(ix + 1, iz + 1),
            ];
            let lo = c.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = c.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            // A few ulps of the lerp's rounding, with room to spare.
            let m = 1e-3 + 1e-5 * (lo.abs() + hi.abs());
            (self.cell, self.lo, self.hi) = ((ix, iz), lo - m, hi + m);
        }
        let (above, gap, closing) = if p.y > self.hi {
            (true, p.y - self.hi, -dir.y)
        } else if p.y < self.lo {
            (false, self.lo - p.y, dir.y)
        } else {
            return (p.y - hf.height_in(ix, iz, xfrac, zfrac) >= 0.0, 0.0);
        };
        // A clamped point's cell runs on past the grid: no run there.
        let max = (hf.size - 1) as f32 * hf.unit;
        let (fx, fz) = (p.x + hf.offset_x, p.z + hf.offset_z);
        if !(fx > 0.0 && fx < max && fz > 0.0 && fz < max) {
            return (above, 0.0);
        }
        let gap = gap - slack;
        let mut run = if closing > 0.0 {
            gap / closing
        } else {
            f32::INFINITY
        };
        let side = |f: f32, i: u32, d: f32| {
            let lo = i as f32 * hf.unit;
            if d > 0.0 {
                (lo + hf.unit - f - slack) / d
            } else if d < 0.0 {
                (f - lo - slack) / -d
            } else {
                f32::INFINITY
            }
        };
        run = run.min(side(fx, ix, dir.x)).min(side(fz, iz, dir.z));
        (above, if gap > 0.0 && run > 0.0 { run } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3x3 grid, unit 1, min corner at world (0,0): heights = x + 10*z.
    fn hf() -> Heightfield {
        Heightfield {
            size: 3,
            unit: 1.0,
            offset_x: 0.0,
            offset_z: 0.0,
            heights: (0..9)
                .map(|i| (i % 3) as f32 + 10.0 * (i / 3) as f32)
                .collect(),
        }
    }

    #[test]
    fn exact_grid_points_and_planar_interior() {
        let f = hf();
        assert_eq!(f.height_at(0.0, 0.0), 0.0);
        assert_eq!(f.height_at(2.0, 0.0), 2.0);
        assert_eq!(f.height_at(1.0, 2.0), 21.0);
        // The test surface is a plane, so every triangle reproduces it exactly
        // regardless of the diagonal — checks the lerp arithmetic.
        for &(x, z, want) in &[(0.25, 0.75, 7.75), (1.5, 0.5, 6.5), (0.5, 1.5, 15.5)] {
            let got = f.height_at(x, z);
            assert!(
                (got - want).abs() < 1e-5,
                "({x},{z}): got {got}, want {want}"
            );
        }
    }

    #[test]
    fn diagonal_alternates_with_row_parity() {
        // Non-planar cell: spike one corner and probe both sides of the split.
        let mut f = hf();
        f.heights[4] = 100.0; // spike vertex (x=1, z=1)

        // Even row (iz=0), cell (1,0), sampled near corner (2,0) with
        // xfrac > zfrac: that triangle is (1,0)(2,0)(2,1) — the spike at
        // (1,1) is not one of its vertices and must not leak in.
        let got = f.height_at(1.99, 0.01);
        assert!((2.0..3.0).contains(&got), "spike must not leak: {got}");
        // Odd row (iz=1), cell (1,1): near corner (1,1) with xfrac<1-zfrac the
        // base IS the spike vertex.
        let got = f.height_at(1.01, 1.01);
        assert!(
            got > 95.0,
            "odd-row triangle starts at the spiked base: {got}"
        );
    }

    /// The normal is the slope `height_at` itself has on each of the four
    /// triangles of a non-planar grid: a finite difference inside the
    /// triangle, turned into (−∂h/∂x, 1, −∂h/∂z) and normalised, agrees.
    #[test]
    fn normal_is_the_height_query_s_own_triangle() {
        let mut f = hf();
        f.heights[4] = 3.0;
        f.heights[8] = -4.0;
        // One probe well inside each triangle kind: even row both halves
        // (cell (0,0)), odd row both halves (cell (1,1)).
        for &(x, z) in &[(0.2, 0.7), (0.7, 0.2), (1.2, 1.3), (1.8, 1.7)] {
            let e = 1e-3;
            let gx = (f.height_at(x + e, z) - f.height_at(x - e, z)) / (2.0 * e);
            let gz = (f.height_at(x, z + e) - f.height_at(x, z - e)) / (2.0 * e);
            let l = (gx * gx + 1.0 + gz * gz).sqrt();
            let want = [-gx / l, 1.0 / l, -gz / l];
            let got = f.normal_at(x, z);
            for k in 0..3 {
                assert!(
                    (got[k] - want[k]).abs() < 1e-3,
                    "({x},{z}): {got:?} vs {want:?}"
                );
            }
        }
        // A plane's normal everywhere: h = x + 10 z.
        let n = hf().normal_at(0.5, 0.5);
        let l = (1.0f32 + 1.0 + 100.0).sqrt();
        assert!((n[0] + 1.0 / l).abs() < 1e-6 && (n[1] - 1.0 / l).abs() < 1e-6);
        assert!((n[2] + 10.0 / l).abs() < 1e-6);
    }

    /// The probe answers exactly what `height_at` answers, on a rough grid,
    /// inside and outside it, at points near and far from the surface; and
    /// every point inside the run it reports, computed as a marched ray
    /// computes it, gets the same answer from `height_at`.
    #[test]
    fn probe_matches_height_at_along_its_run() {
        let n = 9u32;
        let mut seed = 0x2545_F491u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let f = Heightfield {
            size: n,
            unit: 2.5,
            offset_x: 7.0,
            offset_z: 3.0,
            heights: (0..n * n).map(|_| 40.0 * rnd() - 15.0).collect(),
        };
        let slack = 0.05;
        let mut probe = HeightProbe::new(&f);
        let (mut runs, mut checked) = (0, 0);
        for _ in 0..100_000 {
            let o = Vec3::new(30.0 * rnd() - 10.0, 0.0, 30.0 * rnd() - 6.0);
            let h = f.height_at(o.x, o.z);
            // Exactly on the surface, an ulp either side, and anywhere.
            let y = match (rnd() * 4.0) as u32 {
                0 => h,
                1 => f32::from_bits(h.to_bits() + 1),
                2 => f32::from_bits(h.to_bits() - 1),
                _ => 60.0 * rnd() - 25.0,
            };
            let o = Vec3::new(o.x, y, o.z);
            let dir = Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalize();
            let (above, run) = probe.side(o, dir, slack);
            assert_eq!(above, y - h >= 0.0, "{o}, h {h}");
            if run > 0.0 {
                runs += 1;
                let mut t = 0.0f32;
                while t < run {
                    let p = o + dir * t;
                    assert_eq!(
                        p.y - f.height_at(p.x, p.z) >= 0.0,
                        above,
                        "{o} {dir} t {t} of {run}"
                    );
                    checked += 1;
                    t += 0.05;
                }
            }
        }
        assert!(
            runs > 3_000 && checked > 50_000,
            "the probe skips: {runs} runs, {checked} points"
        );
        for y in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let (above, run) = probe.side(Vec3::new(1.0, y, 1.0), Vec3::X, slack);
            assert_eq!(above, y - f.height_at(1.0, 1.0) >= 0.0);
            assert!(run == 0.0 || y.is_infinite());
        }
        let (above, run) = probe.side(Vec3::new(f32::NAN, 1e9, 0.0), Vec3::X, slack);
        assert!(
            !above && run == 0.0,
            "a NaN place is under, as height_at has it"
        );
        // Outside the grid the clamped cell has no run.
        assert_eq!(
            probe.side(Vec3::new(-100.0, 1e3, 0.0), Vec3::X, slack).1,
            0.0
        );
    }

    #[test]
    fn clamps_outside_the_grid() {
        let f = hf();
        assert_eq!(f.height_at(-5.0, -5.0), 0.0);
        assert_eq!(f.height_at(50.0, 50.0), 22.0);
    }
}
