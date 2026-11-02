//! The water plane as the sim sees it: one level plus the WMAP wet-cell mask
//! (a nibble per 32 m cell, the PS2's clamped lookup) and the downward crossing
//! tested at its cell. Follows `analysis/re-fx.md` §15.

use glam::Vec3;

/// PS2 flat-water LAYR lift: 6 cm, 0.3 turns/s (both water update functions,
/// re-water §2 / re-water-flags §7). The fixed sim clock starts at phase 0;
/// synchronizing that phase to a captured PS2 mission clock remains unmeasured.
pub fn bob(seconds: f32) -> f32 {
    0.06 * crate::fmath::sin_cos(1.884_955_8 * seconds).0
}

#[derive(Clone, Debug, PartialEq)]
pub struct WetMask {
    grid: u32,
    inv_cell: f32,
    half: [f32; 2],
    wet: Vec<bool>,
}

impl WetMask {
    pub fn new(
        grid: u32,
        cell_size: f32,
        half_extent: [f32; 2],
        nibbles: &[u8],
    ) -> Option<WetMask> {
        let n = grid as usize * grid as usize;
        (grid > 0 && cell_size > 0.0 && nibbles.len() == n).then(|| WetMask {
            grid,
            inv_cell: 1.0 / cell_size,
            half: half_extent,
            wet: nibbles.iter().map(|&b| b & 0xf != 0).collect(),
        })
    }

    pub fn wet_cells(&self) -> usize {
        self.wet.iter().filter(|&&w| w).count()
    }

    fn index(&self, v: f32, half: f32) -> usize {
        let f = (v + half) * self.inv_cell;
        let last = (self.grid - 1) as f32;
        if f.is_nan() || f < 0.0 {
            0
        } else if f < last {
            f as usize
        } else {
            self.grid as usize - 1
        }
    }

    pub fn is_wet(&self, x: f32, z: f32) -> bool {
        let ix = self.index(x, self.half[0]);
        let iz = self.index(z, self.half[1]);
        self.wet[iz * self.grid as usize + ix]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Plane<'a> {
    pub level: f32,
    pub mask: Option<&'a WetMask>,
}

impl Plane<'_> {
    pub fn wet(&self, x: f32, z: f32) -> bool {
        self.mask.is_none_or(|m| m.is_wet(x, z))
    }

    pub fn under(&self, p: Vec3) -> Option<f32> {
        self.wet(p.x, p.z).then_some(self.level)
    }

    pub fn crossing(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        if origin.y < self.level || dir.y >= 0.0 {
            return None;
        }
        let t = (origin.y - self.level) / -dir.y;
        let at = origin + dir * t;
        self.wet(at.x, at.z).then_some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_clamp_like_the_ps2_lookup() {
        let mut nibbles = vec![0u8; 16];
        nibbles[0] = 1;
        nibbles[4 + 3] = 9;
        nibbles[15] = 8;
        let m = WetMask::new(4, 32.0, [64.0, 64.0], &nibbles).unwrap();
        assert_eq!(m.wet_cells(), 3);
        assert!(m.is_wet(-64.0, -64.0));
        assert!(m.is_wet(-500.0, -500.0), "below the sheet clamps to cell 0");
        assert!(!m.is_wet(-31.9, -64.0));
        assert!(m.is_wet(63.0, -20.0), "x 3, z 1");
        assert!(
            m.is_wet(5000.0, 5000.0),
            "past the sheet clamps to the last cell"
        );
        assert!(m.is_wet(f32::NAN, f32::NAN));
        assert!(WetMask::new(4, 32.0, [64.0, 64.0], &nibbles[1..]).is_none());
    }

    #[test]
    fn a_ray_meets_the_plane_only_over_a_wet_cell() {
        let mut nibbles = vec![0u8; 4];
        nibbles[0] = 1;
        let m = WetMask::new(2, 10.0, [10.0, 10.0], &nibbles).unwrap();
        let p = Plane {
            level: 0.0,
            mask: Some(&m),
        };
        let down = Vec3::new(0.0, -1.0, 0.0);
        assert_eq!(p.crossing(Vec3::new(-5.0, 2.0, -5.0), down), Some(2.0));
        assert_eq!(p.crossing(Vec3::new(5.0, 2.0, -5.0), down), None);
        assert_eq!(
            p.crossing(Vec3::new(5.0, 2.0, -5.0), Vec3::new(-5.0, -1.0, 0.0)),
            Some(2.0),
            "the crossing's cell decides, not the origin's"
        );
        assert_eq!(p.crossing(Vec3::new(-5.0, -1.0, -5.0), down), None);
        assert_eq!(p.under(Vec3::new(5.0, -3.0, 5.0)), None);
        assert_eq!(p.under(Vec3::new(-5.0, -3.0, -5.0)), Some(0.0));
        let open = Plane {
            level: 0.0,
            mask: None,
        };
        assert_eq!(open.crossing(Vec3::new(5.0, 2.0, -5.0), down), Some(2.0));
    }
}
