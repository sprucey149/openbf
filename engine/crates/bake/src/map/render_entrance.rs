//! Fingerprinted donor-only repairs for the Rhen Var tunnel entrance.
//!
//! The outer rock has two displaced corner welds and nonplanar quad diagonals.
//! Snap those welds to vertices of the player's canonical collision mesh. The
//! floor's existing coarse corners can form its simpler contact sheet; remove
//! the detailed donor faces and join those same corners. The PC front rim also
//! needs a small dimensional cross-section profile. No UV-to-position cage or
//! replacement vertex stream is stored here; textures and model headers stay
//! the player's own. Pure PS2 never selects a recipe.

use anyhow::{anyhow, ensure, Result};
use glam::Vec3;
use mungefs::{model::Model, Chunk};
use sha2::{Digest, Sha256};

const NAME: &str = "rhn1_prop_tunnel_entrance";
const WELD_EPSILON: f32 = 0.00001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Xbox,
    Pc,
}

const FLOOR_FACES: &[[u32; 3]] = &[
    [70, 72, 63],
    [63, 72, 65],
    [63, 65, 42],
    [42, 65, 44],
    [42, 44, 21],
    [21, 44, 23],
    [21, 23, 2],
    [2, 23, 1],
    [67, 69, 46],
    [46, 69, 48],
    [46, 48, 25],
    [25, 48, 27],
    [25, 27, 5],
    [5, 27, 6],
    [74, 76, 67],
    [67, 76, 69],
    [72, 74, 65],
    [65, 74, 67],
    [65, 67, 44],
    [44, 67, 46],
    [44, 46, 23],
    [23, 46, 25],
    [23, 25, 1],
    [1, 25, 5],
];

const XBOX_EDGES: &[[u32; 2]] = &[
    [138, 149],
    [140, 154],
    [139, 151],
    [113, 133],
    [109, 118],
    [91, 119],
    [98, 108],
    [23, 32],
    [35, 37],
    [34, 43],
    [36, 59],
    [53, 89],
    [45, 59],
    [88, 138],
    [88, 112],
    [119, 136],
    [123, 135],
    [127, 136],
    [123, 136],
    [58, 99],
    [125, 137],
    [125, 136],
    [51, 65],
    [65, 81],
    [99, 124],
    [100, 126],
    [99, 126],
    [47, 52],
    [49, 57],
    [125, 135],
    [58, 98],
    [106, 125],
    [82, 110],
    [65, 79],
    [47, 49],
    [84, 104],
    [129, 136],
];

const PC_EDGES: &[[u32; 2]] = &[
    [138, 149],
    [140, 154],
    [139, 151],
    [113, 133],
    [109, 118],
    [91, 119],
    [98, 108],
    [23, 32],
    [35, 37],
    [34, 43],
    [36, 59],
    [53, 89],
    [45, 59],
    [88, 138],
    [88, 112],
    [119, 136],
    [123, 135],
    [127, 136],
    [123, 136],
    [58, 99],
    [125, 137],
    [125, 136],
    [51, 65],
    [65, 81],
    [99, 124],
    [99, 126],
    [100, 126],
    [98, 100],
    [52, 60],
    [65, 79],
    [111, 120],
    [103, 120],
    [102, 120],
    [78, 144],
    [47, 49],
    [82, 110],
    [82, 104],
    [84, 104],
    [5, 6],
    [51, 52],
    [3, 4],
    [49, 52],
    [6, 11],
];

const PC_WALL_EDGES: &[[u32; 2]] = &[[39, 41]];

pub(super) struct Entrance(Option<Source>);

impl Entrance {
    pub(super) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = None;
        if let Some(original) = original {
            for chunk in mungefs::content_chunks(mungefs::open(original)?)? {
                if chunk.tag != *b"modl"
                    || !chunk
                        .child_string(b"NAME")
                        .is_some_and(|n| n.eq_ignore_ascii_case(NAME))
                {
                    continue;
                }
                let hash = format!("{:x}", Sha256::digest(chunk.payload));
                selected = match hash.as_str() {
                    "c635fe32e26b57b71d6bb8141a02b0dcadf46e5e16d8aa1e7a1ad471cd2ec3bc" => {
                        Some(Source::Xbox)
                    }
                    "3c5a59a2b610a1505c2b890f23f672bca4c19ba324fd4568c7b912087e121555" => {
                        Some(Source::Pc)
                    }
                    _ => selected,
                };
            }
        }
        Ok(Self(selected))
    }

    pub(super) fn contact_surface(&self, name: &str) -> bool {
        self.0.is_some() && name.eq_ignore_ascii_case(NAME)
    }

    pub(super) fn apply(&self, model: &mut Model, items: &[Chunk<'_>]) -> Result<bool> {
        if !self.contact_surface(&model.name) {
            return Ok(false);
        }
        let source = self.0.unwrap();
        ensure!(!model.bone_space, "entrance alignment needs posed geometry");
        let chunk = items
            .iter()
            .find(|c| {
                c.tag == *b"coll"
                    && c.child_string(b"NAME")
                        .is_some_and(|n| n.eq_ignore_ascii_case(NAME))
            })
            .ok_or_else(|| anyhow!("entrance alignment needs canonical collision mesh"))?;
        let collision = mungefs::coll::CollisionMesh::parse(chunk).map_err(|e| anyhow!(e))?;
        ensure!(
            collision.vertices.len() == 129 && collision.triangles().len() == 204,
            "changed entrance collision topology"
        );
        let vertices = collision
            .vertices
            .iter()
            .copied()
            .map(Vec3::from)
            .collect::<Vec<_>>();
        let corrected = corrected_model(model, &vertices, source)?;
        *model = corrected;
        Ok(true)
    }
}

fn corrected_model(model: &Model, collision: &[Vec3], source: Source) -> Result<Model> {
    let layout: &[(usize, usize)] = match source {
        Source::Xbox => &[(77, 120), (77, 106), (175, 230)],
        Source::Pc => &[(77, 120), (3, 1), (77, 105), (175, 230)],
    };
    ensure!(
        model.segments.len() == layout.len(),
        "changed entrance segment layout"
    );
    for (index, (segment, &(vertices, faces))) in model.segments.iter().zip(layout).enumerate() {
        ensure!(
            !segment.bf2_material
                && segment.positions.len() == vertices
                && segment.normals.len() == vertices
                && segment.uvs.len() == vertices
                && segment.triangles.len() == faces
                && segment.colors.as_ref().is_none_or(|c| c.len() == vertices)
                && segment.positions.iter().flatten().all(|p| p.is_finite())
                && segment.normals.iter().flatten().all(|p| p.is_finite())
                && segment.uvs.iter().flatten().all(|p| p.is_finite())
                && segment
                    .triangles
                    .iter()
                    .flatten()
                    .all(|&i| (i as usize) < vertices),
            "changed entrance donor streams at {index}: pos {} norm {} uv {} faces {} bf2 {} colors {:?} finite-pos {} finite-norm {} finite-uv {} indices-valid {}",
            segment.positions.len(), segment.normals.len(), segment.uvs.len(), segment.triangles.len(),
            segment.bf2_material, segment.colors.as_ref().map(Vec::len),
            segment.positions.iter().flatten().all(|p| p.is_finite()),
            segment.normals.iter().flatten().all(|p| p.is_finite()),
            segment.uvs.iter().flatten().all(|p| p.is_finite()),
            segment.triangles.iter().flatten().all(|&i| (i as usize) < vertices)
        );
    }
    ensure!(
        !collision.is_empty() && collision.iter().all(|p| p.is_finite()),
        "invalid entrance canonical vertices"
    );
    let mut result = model.clone();
    let outer = layout.len() - 1;
    snap_welds(&mut result.segments[outer].positions, collision)?;
    if source == Source::Pc {
        rim_profile(&mut result.segments[0].positions)?;
        for &edge in PC_WALL_EDGES {
            super::render_faces::flip(&mut result.segments[2].triangles, edge)?;
        }
    }
    for &edge in if source == Source::Xbox {
        XBOX_EDGES
    } else {
        PC_EDGES
    } {
        super::render_faces::flip(&mut result.segments[outer].triangles, edge)?;
    }
    result.segments[0].triangles = FLOOR_FACES.to_vec();
    for segment in &mut result.segments {
        segment.index_count = u32::try_from(segment.triangles.len() * 3)?;
    }
    Ok(result)
}

fn snap_welds(positions: &mut [[f32; 3]], collision: &[Vec3]) -> Result<()> {
    ensure!(
        positions.len() == 175 && !collision.is_empty() && collision.iter().all(|p| p.is_finite()),
        "changed entrance corner inputs"
    );
    let original = positions.to_vec();
    let mut changes = Vec::new();
    for (representative, expected) in [(70, 1), (88, 3)] {
        let point = Vec3::from(original[representative]);
        ensure!(point.is_finite(), "nonfinite entrance corner");
        let nearest = collision
            .iter()
            .min_by(|a, b| {
                a.distance_squared(point)
                    .total_cmp(&b.distance_squared(point))
            })
            .unwrap();
        ensure!(
            nearest.distance(point) <= 0.5,
            "entrance corner no longer near its canonical weld"
        );
        let weld = original
            .iter()
            .enumerate()
            .filter(|(_, p)| Vec3::from(**p).distance(point) < WELD_EPSILON)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        ensure!(
            weld.len() == expected,
            "changed entrance corner seam copies"
        );
        changes.extend(weld.into_iter().map(|i| (i, nearest.to_array())));
    }
    for (i, point) in changes {
        positions[i] = point;
    }
    Ok(())
}

fn rim_profile(positions: &mut [[f32; 3]]) -> Result<()> {
    ensure!(
        positions.len() == 77 && positions.iter().flatten().all(|p| p.is_finite()),
        "changed entrance floor input"
    );
    let mut rim = positions
        .iter()
        .enumerate()
        .filter(|(_, p)| p[2].abs() < 0.001)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    ensure!(rim.len() == 7, "changed entrance floor front rim");
    rim.sort_by(|&a, &b| positions[a][0].total_cmp(&positions[b][0]));
    // Outside edges and the two inner breaks of this one cross-section. The
    // intermediate rim vertices divide each interval evenly. Height is a
    // single shallow plane along the rim, not a UV-indexed artwork cage.
    let [left, inner_left, inner_right, right] = [-5.049_493, -1.810_167_3, 1.926_795, 5.424_298_3];
    let x = [
        left,
        (left + inner_left) / 2.,
        inner_left,
        (inner_left + inner_right) / 2.,
        inner_right,
        (inner_right + right) / 2.,
        right,
    ];
    for (i, x) in rim.into_iter().zip(x) {
        positions[i][0] = x;
        positions[i][1] = -0.007_602_3 * x + 2.785_400_2;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::Aabb;

    fn empty_model() -> Model {
        Model {
            name: NAME.into(),
            bone_space: true,
            vertex_box: Aabb {
                min: [-1.; 3],
                max: [1.; 3],
            },
            vis_box: Aabb {
                min: [-2.; 3],
                max: [2.; 3],
            },
            face_count: 3,
            sphere: Some([1., 2., 3., 4.]),
            segments: vec![],
        }
    }

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend((body.len() as u32).to_le_bytes());
        bytes.extend(body);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }

    #[test]
    fn ps2_bypass_does_not_validate_matching_model_or_collision() {
        let selected = Entrance::of(None).unwrap();
        let mut model = empty_model();
        assert!(!selected.apply(&mut model, &[]).unwrap());
        assert!(model.bone_space);
        assert_eq!(model.sphere, Some([1., 2., 3., 4.]));
        assert!(!selected.contact_surface(NAME));
    }

    #[test]
    fn a_matching_name_without_the_donor_hash_cannot_select() {
        let body = chunk(b"NAME", format!("{NAME}\0").as_bytes());
        let bytes = chunk(b"ucfb", &chunk(b"modl", &body));
        assert!(Entrance::of(Some(&bytes)).unwrap().0.is_none());
    }

    fn rim() -> Vec<[f32; 3]> {
        let mut positions = vec![[20., 10., 10.]; 77];
        for (i, p) in positions[..7].iter_mut().enumerate() {
            *p = [i as f32, 2., 0.];
        }
        positions
    }

    #[test]
    fn front_rim_uses_one_dimensional_profile_and_leaves_other_rows() {
        let mut positions = rim();
        let rest = positions[7..].to_vec();
        rim_profile(&mut positions).unwrap();
        assert_eq!(positions[0][0], -5.049_493);
        assert_eq!(positions[6][0], 5.424_298_3);
        for p in &positions[..7] {
            assert_eq!(p[2], 0.);
            assert!((p[1] - (-0.007_602_3 * p[0] + 2.785_400_2)).abs() < 0.000001);
        }
        assert_eq!(positions[7..], rest);
    }

    #[test]
    fn wrong_rim_count_is_rejected_before_changing_any_vertex() {
        let mut positions = rim();
        positions[6][2] = 1.;
        let old = positions.clone();
        assert!(rim_profile(&mut positions).is_err());
        assert_eq!(positions, old);
    }

    fn corners() -> Vec<[f32; 3]> {
        let mut p = vec![[20., 20., 20.]; 175];
        p[70] = [1., 0., 0.];
        for v in &mut p[88..=90] {
            *v = [2., 0., 0.];
        }
        p
    }

    #[test]
    fn all_corner_seam_copies_move_together_to_own_canonical_vertices() {
        let mut p = corners();
        let old = p.clone();
        let c = [Vec3::new(1.1, 0., 0.), Vec3::new(2.2, 0., 0.)];
        snap_welds(&mut p, &c).unwrap();
        assert_eq!(p[70], c[0].to_array());
        for value in &p[88..=90] {
            assert_eq!(*value, c[1].to_array());
        }
        for i in 0..175 {
            if i != 70 && !(88..=90).contains(&i) {
                assert_eq!(p[i], old[i]);
            }
        }
    }

    #[test]
    fn a_later_weld_guard_failure_cannot_partly_move_an_earlier_corner() {
        let mut p = corners();
        p[91] = p[88];
        let old = p.clone();
        let c = [Vec3::new(1.1, 0., 0.), Vec3::new(2.2, 0., 0.)];
        assert!(snap_welds(&mut p, &c).is_err());
        assert_eq!(p, old);
    }

    #[test]
    fn invalid_canonical_points_are_rejected_before_deformation() {
        let mut p = corners();
        let old = p.clone();
        assert!(snap_welds(&mut p, &[]).is_err());
        assert!(snap_welds(&mut p, &[Vec3::splat(f32::NAN)]).is_err());
        assert_eq!(p, old);
    }
}
