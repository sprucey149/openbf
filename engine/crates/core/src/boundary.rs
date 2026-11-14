//! The map boundary's point test (`analysis/re-boundary.md` 2 [O]): the shapes
//! a map's `bnd_` chunk names, BF1 `FUN_002956d8`, BF2 `FUN_0034c550`. The remote
//! droid (issue #59, `Sim::tick_droids`) and flyers (`Sim::tick_flyers`) read it, with
//! the warning the HUD and the commander give while it counts down; the rule for
//! soldiers on foot and ground vehicles is card BNDY.

use crate::soldier::GroundRules;

/// One edge: a point on it and the normal its outside lies along.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edge {
    x: f32,
    z: f32,
    nx: f32,
    nz: f32,
}

/// The shapes of one map. Empty on a map with none: then nothing is outside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boundary {
    shapes: Vec<Vec<Edge>>,
}

impl Boundary {
    /// Add a shape from its path's (x, z) nodes, closed last to first.
    pub fn add_shape(&mut self, nodes: &[[f32; 2]]) {
        let n = nodes.len();
        let edges = (0..n).map(|i| {
            let (a, b) = (nodes[(i + n - 1) % n], nodes[i]);
            Edge {
                x: a[0],
                z: a[1],
                nx: a[1] - b[1],
                nz: b[0] - a[0],
            }
        });
        self.shapes.push(edges.collect());
    }

    /// Outside every shape, height aside. A shape is an intersection of half
    /// planes even where its path is not convex, as on the PS2.
    pub fn outside(&self, x: f32, z: f32) -> bool {
        let inside = |s: &Vec<Edge>| s.iter().all(|e| (x - e.x) * e.nx + (z - e.z) * e.nz <= 0.0);
        !self.shapes.is_empty() && !self.shapes.iter().any(inside)
    }

    /// Does the map have a boundary at all?
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }
}

/// The seconds a controllable lives outside: BF1 10, BF2 6.5 (`re-boundary.md`
/// T1, T2, measured M2).
pub fn leaving_time(rules: GroundRules) -> f32 {
    match rules {
        GroundRules::Bf1 => 10.0,
        GroundRules::Bf2 => 6.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clockwise in (x right, z up), as every disc shape is.
    const SQUARE: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0]];

    #[test]
    fn no_shape_means_never_outside() {
        assert!(!Boundary::default().outside(1e6, -1e6));
    }

    #[test]
    fn a_point_is_inside_when_any_shape_holds_it() {
        let mut b = Boundary::default();
        b.add_shape(&SQUARE);
        assert!(!b.outside(5.0, 5.0));
        assert!(!b.outside(10.0, 10.0), "the edge is inside");
        assert!(b.outside(15.0, 5.0));
        b.add_shape(&SQUARE.map(|[x, z]| [x + 10.0, z]));
        assert!(!b.outside(15.0, 5.0));
        assert!(b.outside(25.0, 5.0));
    }

    /// The PS2 admits less than a non-convex drawing: 6 points of BF1 KAM1 and
    /// HOT1 inside the path read outside on it (`re-boundary.md` M1).
    #[test]
    fn a_non_convex_shape_admits_only_its_half_planes() {
        let mut b = Boundary::default();
        b.add_shape(&[
            [0.0, 0.0],
            [0.0, 10.0],
            [5.0, 5.0],
            [10.0, 10.0],
            [10.0, 0.0],
        ]);
        assert!(!b.outside(5.0, 2.0));
        assert!(b.outside(1.0, 7.0), "inside the drawn path, past its dent");
    }
}
