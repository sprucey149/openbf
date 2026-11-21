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

/// A solid this port adds to one map, in world space (issue #182, leak
/// closure): an invisible blocker or a ground patch where the shipped
/// collision lets a soldier into a gap the census or a playtest shows. It is
/// a SOLDIER-layer body only (bolts, grenades and the camera pass it, as
/// they pass the air it fills), appended after every instance's bodies, so
/// no shipped body's index moves. A map that names no solid builds the
/// world it always did.
///
/// OURS: the maintainer allowed added collision geometry for leak spots
/// (2026-11-20). Each entry says where it is and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddedSolid {
    /// The map's folder name (`bf1_rhn2`), matched without regard to case.
    pub map: &'static str,
    /// A short name for diagnostics (the black box's body label).
    pub name: &'static str,
    pub shape: AddedShape,
}

/// An added solid's shape, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AddedShape {
    /// A box: centre, half extents, and a turn about +Y (deg).
    Box {
        center: [f32; 3],
        half: [f32; 3],
        yaw_deg: f32,
    },
    /// Triangles (a ground patch, a wedge's fill), each wound clockwise
    /// seen from the side a soldier stands on, like the bake's meshes.
    Tris(&'static [[[f32; 3]; 3]]),
    /// The closed convex hull of these points (a prop's grooves filled).
    Hull(&'static [[f32; 3]]),
}

/// The added solids, by map. See each entry's comment for its site.
pub const ADDED_SOLIDS: &[AddedSolid] = &[];

/// A solid this port adds to every instance of one geometry, in that
/// geometry's MODEL space (placed by each instance's own matrix, so a tilted
/// instance tilts it too): a hollow the shipped mesh leaves open inside a
/// pier, a slot between an obelisk's fins. Like [`AddedSolid`] it is a
/// SOLDIER-layer body only, built after every instance's own bodies (in
/// instance order), so no shipped body's index moves; it is of its instance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddedModelSolid {
    /// The geometry (`entc` GeometryName), matched without regard to case.
    pub geometry: &'static str,
    pub name: &'static str,
    pub shape: AddedShape,
}

/// The added model-space solids. See each entry's comment for its site.
pub const ADDED_MODEL_SOLIDS: &[AddedModelSolid] = &[
    // Rhen Var's bridge (`rhn_bldg_bridge`, BF1 Rhen Var Citadel's ravine
    // and harbour bridges): each of its two piers is four walls round an
    // open shaft 2 x 8 m, from 24 m under the deck's foot up to the deck,
    // with no floor or lid. A soldier sliding down the ravine past the
    // inner wall's top dropped into the shaft and hung on its steep floor
    // of terrain for good (the census's `float` runs at the Citadel ravine
    // bridge, 140 at one pier). Each shaft is filled to the deck's
    // underside, so the pier is the solid column it looks.
    AddedModelSolid {
        geometry: "rhn_bldg_bridge",
        name: "bridge pier shaft +x",
        shape: AddedShape::Box {
            center: [3.0, -8.0, 0.0],
            half: [1.0, 16.0, 4.0],
            yaw_deg: 0.0,
        },
    },
    AddedModelSolid {
        geometry: "rhn_bldg_bridge",
        name: "bridge pier shaft -x",
        shape: AddedShape::Box {
            center: [-3.0, -8.0, 0.0],
            half: [1.0, 16.0, 4.0],
            yaw_deg: 0.0,
        },
    },
    // Rhen Var's obelisk (`rhn_bldg_obelisk`, BF1 Rhen Var Citadel): a
    // ribbed spire 18 m wide at its foot, tapering to 10 m at 62 m. Its
    // grooves are open shells 0.6-1.0 m deep and under 1 m wide, a
    // soldier's width: he wedged in one by the leaning obelisk and could
    // neither stand nor leave (the census's `float` and `inshell` runs
    // there). The spire is filled to its convex hull (the rib tops at its
    // foot and its top ring), so the grooves are solid. The foot's widest
    // ring (5.75 m up the model) is carried straight down 12 m: under it the
    // foot narrows, and on the slope by the leaning obelisk the terrain runs
    // under that overhang, where a soldier wedged too.
    AddedModelSolid {
        geometry: "rhn_bldg_obelisk",
        name: "obelisk grooves",
        shape: AddedShape::Hull(&[
            [-9.1489, -6.0, -6.9849],
            [-9.1489, -6.0, 6.9849],
            [9.1489, -6.0, -6.9849],
            [9.1489, -6.0, 6.9849],
            [-5.3333, -6.0, -9.0701],
            [-5.3333, -6.0, 9.0701],
            [5.3333, -6.0, -9.0701],
            [5.3333, -6.0, 9.0701],
            [-9.1489, 5.7535, -6.9849],
            [-9.1489, 5.7535, 6.9849],
            [-8.1205, 4.022, -6.9849],
            [-8.1205, 4.022, 6.9849],
            [-7.4881, 3.996, -6.057],
            [-7.4881, 3.996, 6.057],
            [-5.3333, 3.996, -6.9849],
            [-5.3333, 3.996, 6.9849],
            [-5.3333, 4.022, -8.1628],
            [-5.3333, 4.022, 8.1628],
            [-5.3333, 5.7535, -9.0701],
            [-5.3333, 5.7535, 9.0701],
            [-4.876, 62.0, -3.5537],
            [-4.876, 62.0, 3.5537],
            [-4.8533, 62.0, -4.0981],
            [-4.8533, 62.0, 4.0981],
            [-3.1291, 62.0, -4.9551],
            [-3.1291, 62.0, 4.9551],
            [3.1291, 62.0, -4.9551],
            [3.1291, 62.0, 4.9551],
            [4.8533, 62.0, -4.0981],
            [4.8533, 62.0, 4.0981],
            [4.876, 62.0, -3.5537],
            [4.876, 62.0, 3.5537],
            [5.3333, 3.996, -6.9849],
            [5.3333, 3.996, 6.9849],
            [5.3333, 4.022, -8.1628],
            [5.3333, 4.022, 8.1628],
            [5.3333, 5.7535, -9.0701],
            [5.3333, 5.7535, 9.0701],
            [7.4881, 3.996, -6.057],
            [7.4881, 3.996, 6.057],
            [8.1205, 4.022, -6.9849],
            [8.1205, 4.022, 6.9849],
            [9.1489, 5.7535, -6.9849],
            [9.1489, 5.7535, 6.9849],
        ]),
    },
];

/// The model-space solids added to `geometry`, in table order.
pub fn added_model_solids(geometry: &str) -> impl Iterator<Item = &'static AddedModelSolid> + '_ {
    ADDED_MODEL_SOLIDS
        .iter()
        .filter(move |s| s.geometry.eq_ignore_ascii_case(geometry))
}

/// The solids added to `map` (its folder name), in table order.
pub fn added_solids(map: &str) -> impl Iterator<Item = &'static AddedSolid> + '_ {
    ADDED_SOLIDS
        .iter()
        .filter(move |s| s.map.eq_ignore_ascii_case(map))
}

/// `solid` as a SOLDIER-layer body. `None` for an empty triangle list.
pub fn added_body(solid: &AddedSolid) -> Option<core::collision::Body> {
    shape_body(&solid.shape, glam::Mat4::IDENTITY)
}

/// `shape` placed by `to_world` as a SOLDIER-layer body. `None` for an
/// empty triangle list.
pub fn shape_body(shape: &AddedShape, to_world: glam::Mat4) -> Option<core::collision::Body> {
    use core::collision::{Body, Layers, Shape};
    let (m, shape) = match *shape {
        AddedShape::Box {
            center,
            half,
            yaw_deg,
        } => (
            glam::Mat4::from_rotation_translation(
                glam::Quat::from_rotation_y(yaw_deg.to_radians()),
                glam::Vec3::from(center),
            ),
            Shape::Box {
                half: glam::Vec3::from(half),
            },
        ),
        AddedShape::Hull(points) => (
            glam::Mat4::IDENTITY,
            Shape::Mesh(core::collision::convex_hull_mesh(points)?),
        ),
        AddedShape::Tris(tris) => {
            let positions: Vec<[f32; 3]> = tris.iter().flatten().copied().collect();
            let indices: Vec<u32> = (0..positions.len() as u32).collect();
            (
                glam::Mat4::IDENTITY,
                Shape::Mesh(core::collision::trimesh(&positions, &indices)?),
            )
        }
    };
    Some(Body::with_layers(to_world * m, shape, Layers::SOLDIER))
}

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
