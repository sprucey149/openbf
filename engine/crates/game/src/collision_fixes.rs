//! Faces this port adds to a map model's collision mesh where the shipped
//! mesh leaves a hole that a soldier can fall into.
//!
//! Each fix names a geometry and the four corners of one quad in model
//! space. Every corner must already be a vertex of the player's own mesh. The
//! fix joins those vertices with two triangles, wound like the mesh's other
//! faces, and adds no vertex of its own. A mesh that lacks any corner (another
//! extraction, another revision) is left as it is. The table stores no mesh
//! data: it only says which existing vertices to join.
//!
//! The fixed mesh is the mesh body's own shape, so it keeps that body's
//! layers and index. Black box body numbers and the instance order do not
//! change.
//!
//! OURS (issue #182, the BF1 Theed playtest of 2026-11-16): Theed's
//! `nab2_bldg_i_20` has two stairs along its walls, and both games ship the
//! same mesh. On the wall side of each stair, a 0.7 m balustrade has a top
//! face and an end cap, but no face toward the steps. The open side's
//! balustrade has both. A roll's low capsule passes under that top face. Its
//! axis leaves the step's edge, and the soldier drops through the slot to the
//! street. He can then walk around under the steps. The fix closes each
//! slot with the face the open side already has, plus a floor across the
//! slot at step height. On the bake, a headless sweep of 84,672 runs per map
//! (walk, sprint, roll, jump, sprint jump and sprint roll across all four
//! stairs) ended under a stair 1,278 times on BF1 and 1,047 times on BF2,
//! every time after a roll. With the face alone, 0 BF1 runs and 2 BF2 runs
//! still did. With the face and the floor, no run did on either map.
//! trace: a PS2 BF1 Theed soldier rolling at the wall side of either
//! `nab2_bldg_i_20` stair (does the original drop into the slot?).

/// One added quad: the geometry it belongs to and its corners in model
/// space, in winding order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddedFace {
    pub geometry: &'static str,
    pub corners: [[f32; 3]; 4],
}

/// How far (m) a corner may lie from the mesh vertex it names. The bake
/// writes the disc's f32s, and this tolerance covers their decimal spelling.
pub const CORNER_TOLERANCE: f32 = 0.01;

/// The added faces. Each stair gets two: the wall-side balustrade's inner
/// face, from the step edge up to the balustrade's top, and a floor across
/// the slot at step height. The floor catches a roll that still pokes into
/// the slot in one tick (2 of 84,672 BF2 runs with the face alone): the
/// soldier stands on it at step height instead of dropping.
pub const ADDED_FACES: &[AddedFace] = &[
    // The stair down the building's +X face (Theed BF1: the one by the
    // street at (-105, 82)).
    AddedFace {
        geometry: "nab2_bldg_i_20",
        corners: [
            [24.0, 0.0, 0.7],
            [8.0, 8.0, 0.7],
            [8.0, 9.25, 0.7],
            [24.0, 1.25, 0.7],
        ],
    },
    AddedFace {
        geometry: "nab2_bldg_i_20",
        corners: [
            [24.0, 0.0, 0.0],
            [24.0, 0.0, 0.7],
            [8.0, 8.0, 0.7],
            [8.0, 8.0, 0.0],
        ],
    },
    // The stair down the building's -Z face.
    AddedFace {
        geometry: "nab2_bldg_i_20",
        corners: [
            [-0.71, 0.0, -24.0],
            [-0.71, 1.25, -24.0],
            [-0.71, 9.25, -8.0],
            [-0.71, 8.0, -8.0],
        ],
    },
    AddedFace {
        geometry: "nab2_bldg_i_20",
        corners: [
            [0.0, 0.0, -24.0],
            [0.0, 8.0, -8.0],
            [-0.71, 8.0, -8.0],
            [-0.71, 0.0, -24.0],
        ],
    },
];

/// The index of the first vertex within [`CORNER_TOLERANCE`] of `p`.
fn vertex_at(positions: &[[f32; 3]], p: [f32; 3]) -> Option<u32> {
    let t2 = CORNER_TOLERANCE * CORNER_TOLERANCE;
    positions
        .iter()
        .position(|v| {
            let d = [v[0] - p[0], v[1] - p[1], v[2] - p[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= t2
        })
        .and_then(|i| u32::try_from(i).ok())
}

/// The triangles the fixes add to `geometry`'s collision mesh, as indices
/// into its own `positions`. The result is empty when no fix applies.
/// `geometry` is matched without regard to case.
pub fn added_indices(geometry: &str, positions: &[[f32; 3]]) -> Vec<u32> {
    let mut out = Vec::new();
    for face in ADDED_FACES
        .iter()
        .filter(|f| f.geometry.eq_ignore_ascii_case(geometry))
    {
        let mut ids = [0u32; 4];
        let mut found = true;
        for (id, c) in ids.iter_mut().zip(face.corners) {
            match vertex_at(positions, c) {
                Some(i) => *id = i,
                None => found = false,
            }
        }
        if found {
            let [a, b, c, d] = ids;
            out.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    out
}

/// `indices` with the fixes' triangles appended, or `None` when no fix
/// applies to `geometry`'s mesh.
pub fn fixed_indices(geometry: &str, positions: &[[f32; 3]], indices: &[u32]) -> Option<Vec<u32>> {
    let added = added_indices(geometry, positions);
    (!added.is_empty()).then(|| [indices, &added].concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stair corners as a bake writes them, plus vertices nearby that
    /// are not corners.
    fn stair_vertices() -> Vec<[f32; 3]> {
        vec![
            [24.0, 0.0, 5.3],
            [24.000_002, 0.0, 0.699_999_8],
            [8.0, 8.0, 0.7],
            [8.0, 9.25, 0.7],
            [24.0, 1.25, 0.700_001],
            [-0.71, 0.0, -24.0],
            [-0.71, 1.25, -24.0],
            [-0.71, 9.25, -8.0],
            [-0.71, 8.0, -8.0],
            [24.0, 0.0, -0.000_001],
            [8.0, 8.0, 0.0],
            [0.0, 0.0, -24.0],
            [0.000_001, 8.0, -8.0],
        ]
    }

    #[test]
    fn theed_stairs_get_both_wall_side_faces_from_their_own_vertices() {
        let v = stair_vertices();
        assert_eq!(
            added_indices("NAB2_BLDG_I_20", &v),
            vec![1, 2, 3, 1, 3, 4, 9, 1, 2, 9, 2, 10, 5, 6, 7, 5, 7, 8, 11, 12, 8, 11, 8, 5]
        );
        assert!(added_indices("nab2_bldg_b_19", &v).is_empty());
    }

    #[test]
    fn a_mesh_missing_a_corner_is_left_alone() {
        let mut v = stair_vertices();
        v[3] = [8.0, 9.0, 0.7];
        // The first stair's face loses a corner. Its floor and the second
        // stair's faces are still added.
        assert_eq!(
            added_indices("nab2_bldg_i_20", &v),
            vec![9, 1, 2, 9, 2, 10, 5, 6, 7, 5, 7, 8, 11, 12, 8, 11, 8, 5]
        );
        assert_eq!(fixed_indices("nab2_bldg_i_20", &[], &[0, 1, 2]), None);
    }

    /// Each balustrade face points into its stair, like the open side's
    /// inner face, and each slot floor points up. The normal comes from the
    /// mesh's winding.
    #[test]
    fn added_faces_face_the_steps() {
        let up = glam::Vec3::Y;
        let toward = [glam::Vec3::Z, up, glam::Vec3::NEG_X, up];
        for (face, want) in ADDED_FACES.iter().zip(toward) {
            let [a, b, c, _] = face.corners.map(glam::Vec3::from);
            // The bake's meshes wind clockwise seen from the front.
            let n = (c - a).cross(b - a).normalize();
            assert!(n.dot(want) > 0.85, "{face:?}: normal {n}");
        }
    }
}
