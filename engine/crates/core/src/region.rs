//! World regions (wrld `regn`): box / sphere / cylinder volumes used for CP
//! capture + control zones, kill zones and sound triggers.
//!
//! Math transcribed from the decomp (analysis/re-cp-capture.md §5,
//! lead-verified on BF1 FUN_004c2b08 point test; loader FUN_00177828 /
//! init FUN_004c2318; BF2 FUN_00536410 / FUN_00537a78 identical):
//! - rows of the stored 3×3 = local axes (row-vector convention, as for
//!   instances); rows are normalized at init and SIZE components scaled by
//!   the original row lengths;
//! - `w = |SIZE|` (vector length) is stored by the loader — it is the sphere
//!   radius (TAT1 authors spheres as SIZE (0.1, r, 0.1));
//! - box: half-extents, `|d·row_i| < size_i`;
//! - cylinder: axis = row1, half-height `size_y`, radius
//!   `r = sqrt(size_x² + size_z²)`;
//! - every test is STRICT (`<`). The engine's axis-aligned fast paths (enums
//!   5–8) give identical results and are not replicated.

use glam::{Mat3, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Box,
    Sphere,
    Cylinder,
}

impl Shape {
    pub fn parse(s: &str) -> Option<Shape> {
        Some(match s.to_ascii_lowercase().as_str() {
            "box" => Shape::Box,
            "sphere" => Shape::Sphere,
            "cylinder" => Shape::Cylinder,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub shape: Shape,
    pub center: Vec3,
    /// Columns = normalized local axes in world space.
    pub basis: Mat3,
    /// Box: half-extents. Cylinder: (r, half-height, r). Sphere: as stored.
    pub size: Vec3,
    /// |SIZE| as stored — the sphere radius.
    pub w: f32,
}

impl Region {
    /// From the baked manifest fields (rotation rows + SIZE as stored).
    pub fn from_rows(
        shape: &str,
        rotation: [[f32; 3]; 3],
        translation: [f32; 3],
        size: [f32; 3],
    ) -> Option<Region> {
        let shape = Shape::parse(shape)?;
        let raw = Vec3::from(size);
        let rows: [(Vec3, f32); 3] = std::array::from_fn(|i| {
            let v = Vec3::from(rotation[i]);
            let len = v.length();
            if len > 1e-6 {
                (v / len, len)
            } else {
                (Vec3::AXES[i], 1.0)
            }
        });
        let scaled = Vec3::new(raw.x * rows[0].1, raw.y * rows[1].1, raw.z * rows[2].1);
        let size = match shape {
            Shape::Sphere => raw,
            Shape::Box => scaled,
            Shape::Cylinder => {
                let r = (scaled.x * scaled.x + scaled.z * scaled.z).sqrt();
                Vec3::new(r, scaled.y, r)
            }
        };
        Some(Region {
            shape,
            center: Vec3::from(translation),
            basis: Mat3::from_cols(rows[0].0, rows[1].0, rows[2].0),
            size,
            w: raw.length(),
        })
    }

    /// Sphere of radius `r` at `center` (the `Radius` key's default regions).
    pub fn sphere(center: Vec3, r: f32) -> Region {
        Region {
            shape: Shape::Sphere,
            center,
            basis: Mat3::IDENTITY,
            size: Vec3::new(r, 0.0, 0.0),
            w: r,
        }
    }

    pub fn to_local(&self, p: Vec3) -> Vec3 {
        self.basis.transpose() * (p - self.center)
    }

    /// Point-in-region (FUN_004c2b08 / FUN_00536410), strict.
    pub fn contains(&self, p: Vec3) -> bool {
        let d = p - self.center;
        match self.shape {
            Shape::Sphere => d.length_squared() < self.w * self.w,
            Shape::Box => {
                let l = self.to_local(p);
                l.x.abs() < self.size.x && l.y.abs() < self.size.y && l.z.abs() < self.size.z
            }
            Shape::Cylinder => {
                let l = self.to_local(p);
                l.y.abs() < self.size.y && l.x * l.x + l.z * l.z < self.size.x * self.size.x
            }
        }
    }

    /// Does a sphere touch the region? BF2's gather for objects in a region
    /// (`FUN_00536f10`, `analysis/re-kill-floor.md`): strict, like [`Self::contains`].
    pub fn touches_sphere(&self, c: Vec3, r: f32) -> bool {
        match self.shape {
            Shape::Sphere => (c - self.center).length_squared() < (self.w + r) * (self.w + r),
            Shape::Box => {
                let l = self.to_local(c);
                let out = (l.abs() - self.size).max(Vec3::ZERO);
                out.length_squared() < r * r
            }
            Shape::Cylinder => {
                let l = self.to_local(c);
                let radial = (l.x * l.x + l.z * l.z).sqrt() - self.size.x;
                let axial = l.y.abs() - self.size.y;
                let (a, b) = (radial.max(0.0), axial.max(0.0));
                a * a + b * b < r * r
            }
        }
    }

    /// Nominal footprint radius (placement/AI heuristics).
    pub fn radius(&self) -> f32 {
        match self.shape {
            Shape::Box => self.size.x.max(self.size.z),
            Shape::Sphere => self.w,
            Shape::Cylinder => self.size.x,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn rotated_box_containment_is_strict_half_extents() {
        // 45° yaw box, half-extents (2, 1, 1) at (10, 0, 0).
        let c = std::f32::consts::FRAC_1_SQRT_2;
        let r = Region::from_rows(
            "box",
            [[c, 0.0, -c], [0.0, 1.0, 0.0], [c, 0.0, c]],
            [10.0, 0.0, 0.0],
            [2.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(r.contains(Vec3::new(10.0 + 1.9 * c, 0.0, -1.9 * c)));
        assert!(!r.contains(Vec3::new(10.0 + 2.1 * c, 0.0, -2.1 * c)));
        assert!(r.contains(Vec3::new(10.0 + 0.9 * c, 0.5, 0.9 * c)));
        assert!(!r.contains(Vec3::new(10.0 + 1.1 * c, 0.5, 1.1 * c)));
        assert!(!r.contains(Vec3::new(10.0, 1.0, 0.0)), "strict at the face");
        // Row lengths scale the extents (non-unit rows).
        let s = Region::from_rows(
            "box",
            [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0; 3],
            [1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(s.contains(Vec3::new(1.9, 0.0, 0.0)) && !s.contains(Vec3::new(2.1, 0.0, 0.0)));
    }

    /// re-cp-capture.md §8 vector 10: TAT1 regions.
    #[test]
    fn tat1_reference_regions() {
        // cp10capture: cylinder SIZE (4.287898, 10, 20.576218), rows flipped
        // about Y → r = 21.018, half-height 10.
        let c = Vec3::new(13.773029, 14.277857, 161.66124);
        let cyl = Region::from_rows(
            "cylinder",
            [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
            c.to_array(),
            [4.287898, 10.0, 20.576218],
        )
        .unwrap();
        assert!((cyl.size.x - 21.018).abs() < 1e-3, "r {}", cyl.size.x);
        assert!(cyl.contains(c + Vec3::new(15.0, 0.0, 14.0)));
        assert!(!cyl.contains(c + Vec3::new(15.0, 0.0, 15.0)));
        assert!(
            !cyl.contains(c + Vec3::new(0.0, 10.0, 0.0)),
            "10 is not < 10"
        );
        assert!(cyl.contains(c + Vec3::new(0.0, 9.99, 0.0)));

        // cp2capture: sphere SIZE (0.1, 8.14867, 0.1) → r = |SIZE| = 8.1499.
        let s = Region::from_rows("sphere", ID, [0.0; 3], [0.1, 8.14867, 0.1]).unwrap();
        assert!((s.w - 8.1499).abs() < 1e-3);
        assert!(s.contains(Vec3::new(8.1, 0.0, 0.0)));
        assert!(!s.contains(Vec3::new(8.2, 0.0, 0.0)));

        // cp4capture: slightly rotated box, half-extents (7.3148, 2.781, 4.1026).
        let rows = [
            [-0.9948392, -0.0200407, -0.09946558],
            [-0.01974382, 0.9997972, -0.003968304],
            [0.09952493, -0.001983994, -0.9950331],
        ];
        let b = Region::from_rows("box", rows, [0.0; 3], [7.314838, 2.781044, 4.102591]).unwrap();
        let row0 = Vec3::from(rows[0]).normalize();
        let row1 = Vec3::from(rows[1]).normalize();
        assert!(b.contains(row0 * 7.0));
        assert!(!b.contains(row0 * 7.4));
        assert!(!b.contains(row1 * 2.8));
        assert!(b.contains(row1 * 2.7));
        assert_eq!(Shape::parse("Cube"), None);
        assert_eq!(Region::sphere(Vec3::ZERO, 3.0).radius(), 3.0);
    }
}
