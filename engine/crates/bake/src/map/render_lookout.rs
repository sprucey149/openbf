//! Raise the donor Citadel lookout's collapsed roof to its own intact rim.
//!
//! Four floor corners and eight copies in the adjoining walls sit below the
//! unchanged PS2 contact roof. The opposite broken-wall corner is also low. Keep the donor topology, materials and UVs;
//! level those welds using the height of an existing donor floor vertex. A
//! retracted ridge corner also needs a small translation, using its own welds.
//! Only the fingerprinted Xbox/PC model selects this drawing-only profile.

use anyhow::{ensure, Result};
use glam::Vec3;
use mungefs::model::{Model, Segment};
use sha2::{Digest, Sha256};

const NAME: &str = "rhn_bldg_look_out";
const INPUTS: [&str; 2] = [
    "5e2b71ab1c201f9989aaa1fa900833f4e2ad418c8acfed104b4f748ee3c309e0",
    "af34285691be31150cb78bd981ddf5c40d5d7fe46cedde7bda4a941b2c420959",
];
const SHAPES: [(usize, usize); 5] = [(168, 99), (17, 14), (86, 54), (594, 393), (80, 55)];
const WELD_EPSILON: f32 = 0.00001;
// Measured height of the existing opposite broken-wall collision corner.
// The donor keeps the same X/Z corner but depresses it by about 1.65 m.
const RIGHT_CORNER_Y: f32 = 9.531_482;
// Measured displacement restoring the donor's retracted ridge corner. This is
// one donor-weld transform, not a stored reference vertex or replacement face.
const RIDGE_TRANSLATION: Vec3 = Vec3::new(0.2673, -0.3591, 1.36);
// Extend the donor's existing upper-beam triangle by one copied corner. Its
// intact edge supplies the placement; the displaced copy restores the missing
// lip without moving the existing beam, wall or collision vertices.
const UPPER_LIP_TRANSLATION: Vec3 = Vec3::new(0.904_555_3, 0.107_238_77, 0.343_709_47);

pub(super) fn enabled(original: Option<&[u8]>) -> Result<bool> {
    let Some(bytes) = original else {
        return Ok(false);
    };
    for chunk in mungefs::content_chunks(mungefs::open(bytes)?)? {
        if chunk.tag == *b"modl"
            && chunk.child_string(b"NAME").as_deref() == Some(NAME)
            && INPUTS.contains(&format!("{:x}", Sha256::digest(chunk.payload)).as_str())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn same(a: [f32; 3], b: [f32; 3]) -> bool {
    a.into_iter()
        .zip(b)
        .all(|(x, y)| (x - y).abs() <= WELD_EPSILON)
}

pub(super) fn model(model: &mut Model) -> Result<()> {
    if !model.name.eq_ignore_ascii_case(NAME) {
        return Ok(());
    }
    ensure!(
        !model.bone_space && model.segments.len() == SHAPES.len(),
        "Citadel lookout pose or segmentation changed"
    );
    for (segment, &(vertices, faces)) in model.segments.iter().zip(&SHAPES) {
        ensure!(
            segment.positions.len() == vertices
                && segment.normals.len() == vertices
                && segment.uvs.len() == vertices
                && segment.triangles.len() == faces
                && segment
                    .colors
                    .as_ref()
                    .is_none_or(|colors| colors.len() == vertices)
                && segment.positions.iter().flatten().all(|v| v.is_finite())
                && segment
                    .triangles
                    .iter()
                    .flatten()
                    .all(|&i| (i as usize) < vertices),
            "Citadel lookout donor streams changed"
        );
    }
    let floor = &model.segments[1].positions;
    let height = floor[10][1];
    let anchors: [[f32; 3]; 4] = std::array::from_fn(|i| floor[i + 6]);
    ensure!(
        anchors.iter().all(|p| p[1] > floor[0][1] && p[1] < height)
            && floor[10..]
                .iter()
                .all(|p| (p[1] - height).abs() <= WELD_EPSILON),
        "Citadel lookout floor profile changed"
    );
    let mut result = model.clone();
    for (index, expected) in [(0, 8), (1, 4)] {
        let segment = &mut result.segments[index];
        let mut moved = 0;
        for point in &mut segment.positions {
            if anchors.iter().any(|&anchor| same(*point, anchor)) {
                point[1] = height;
                moved += 1;
            }
        }
        ensure!(moved == expected, "Citadel lookout floor welds changed");
        // The lifted floor is flat. Update its corner normals while leaving
        // the separate vertical-wall corner streams and all UV seams intact.
        for triangle in &segment.triangles {
            if triangle
                .iter()
                .all(|&i| (segment.positions[i as usize][1] - height).abs() <= WELD_EPSILON)
            {
                for &i in triangle {
                    segment.normals[i as usize] = [0., 1., 0.];
                }
            }
        }
    }
    let corner = model.segments[4].positions[58];
    ensure!(
        same(corner, model.segments[4].positions[59]) && corner[1] > 7. && corner[1] < 8.,
        "Citadel lookout opposite corner changed"
    );
    for (index, expected) in [(3, 1), (4, 2)] {
        let segment = &mut result.segments[index];
        let mut moved = vec![false; segment.positions.len()];
        for (i, point) in segment.positions.iter_mut().enumerate() {
            if same(*point, corner) {
                point[1] = RIGHT_CORNER_Y;
                moved[i] = true;
            }
        }
        ensure!(
            moved.iter().filter(|&&v| v).count() == expected,
            "Citadel lookout opposite corner welds changed"
        );
        if index == 4 {
            repair_floor(segment)?;
        }
    }
    let ridge = model.segments[4].positions[49];
    ensure!(
        ridge[1] > 15. && ridge[1] < 16.,
        "Citadel lookout ridge changed"
    );
    for (index, expected) in [(3, 2), (4, 1)] {
        let segment = &mut result.segments[index];
        let mut moved = vec![false; segment.positions.len()];
        for (i, point) in segment.positions.iter_mut().enumerate() {
            if same(*point, ridge) {
                *point = (Vec3::from(*point) + RIDGE_TRANSLATION).to_array();
                moved[i] = true;
            }
        }
        ensure!(
            moved.iter().filter(|&&v| v).count() == expected,
            "Citadel lookout ridge welds changed"
        );
        update_normals(segment, &moved);
    }
    let segment = &mut result.segments[3];
    ensure!(
        segment.triangles.get(372) == Some(&[202, 211, 206])
            && segment.bmap.is_empty()
            && segment.bind.is_empty()
            && segment.wght.is_empty(),
        "Citadel lookout upper-beam donor changed"
    );
    let corners = [202, 211, 206];
    let mut positions = corners.map(|i| segment.positions[i]);
    positions[1] = (Vec3::from(positions[0]) + UPPER_LIP_TRANSLATION).to_array();
    let [a, b, c] = positions.map(Vec3::from);
    let normal = (b - a).cross(c - a).normalize_or_zero();
    ensure!(
        normal.is_finite() && normal.length_squared() > 0.,
        "invalid Citadel upper-beam cap"
    );
    let start = segment.positions.len() as u32;
    for (i, position) in positions.into_iter().enumerate() {
        segment.positions.push(position);
        segment.normals.push(normal.to_array());
        segment.uvs.push(segment.uvs[corners[i]]);
        if let Some(colors) = &mut segment.colors {
            colors.push(colors[corners[i]]);
        }
    }
    segment.triangles.push([start, start + 1, start + 2]);
    segment.vertex_count = segment.positions.len() as u32;
    segment.index_count = (segment.triangles.len() * 3) as u32;
    *model = result;
    Ok(())
}

// The donor inserts a depressed subdivision between these same rim corners.
// Rejoin the donor's own corners along the contact-floor diagonals, leaving
// its unused interior vertices and every UV/colour stream intact.
const FLOOR_REMOVED: [[u32; 3]; 15] = [
    [69, 55, 71],
    [71, 55, 54],
    [71, 54, 72],
    [72, 54, 57],
    [72, 57, 58],
    [58, 57, 45],
    [67, 70, 55],
    [55, 70, 53],
    [55, 53, 48],
    [48, 53, 51],
    [70, 63, 53],
    [45, 57, 47],
    [47, 57, 54],
    [47, 54, 48],
    [48, 54, 55],
];
const FLOOR_ADDED: [[u32; 3]; 4] = [[58, 63, 53], [58, 72, 63], [45, 58, 51], [51, 58, 53]];

fn repair_floor(segment: &mut Segment) -> Result<()> {
    for old in FLOOR_REMOVED {
        let at = segment.triangles.iter().position(|face| *face == old);
        ensure!(at.is_some(), "Citadel lookout floor subdivision changed");
        segment.triangles.remove(at.unwrap());
    }
    segment.triangles.extend(FLOOR_ADDED);
    segment.index_count = (segment.triangles.len() * 3) as u32;
    let mut affected = vec![false; segment.positions.len()];
    for triangle in FLOOR_ADDED {
        for i in triangle {
            affected[i as usize] = true;
        }
    }
    update_normals(segment, &affected);
    Ok(())
}

fn update_normals(segment: &mut Segment, moved: &[bool]) {
    let mut affected = vec![false; moved.len()];
    for triangle in &segment.triangles {
        if triangle.iter().any(|&i| moved[i as usize]) {
            for &i in triangle {
                affected[i as usize] = true;
            }
        }
    }
    let mut normals = vec![Vec3::ZERO; moved.len()];
    for triangle in &segment.triangles {
        let [a, b, c] = triangle.map(|i| Vec3::from(segment.positions[i as usize]));
        let normal = (b - a).cross(c - a);
        for &i in triangle {
            if affected[i as usize] {
                normals[i as usize] += normal;
            }
        }
    }
    for (i, normal) in normals.into_iter().enumerate() {
        if affected[i] && normal.length_squared() > 0. {
            segment.normals[i] = normal.normalize().to_array();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType, Segment};

    fn fixture() -> Model {
        let segments = SHAPES
            .iter()
            .map(|&(vertices, faces)| Segment {
                vertex_count: vertices as u32,
                index_count: (faces * 3) as u32,
                bf2_material: false,
                pc_geometry: false,
                material_flags: 1,
                render_type: RenderType::Bf1("Normal".into()),
                spec_power: 0,
                spec_color: 0,
                uv_anim: [0; 2],
                textures: vec![(0, "donor_roof".into())],
                bone: None,
                group: None,
                bmap: vec![],
                bind: vec![],
                wght: vec![],
                positions: (0..vertices).map(|i| [100. + i as f32, 0., 0.]).collect(),
                normals: vec![[0.5, 0.5, 0.5]; vertices],
                uvs: vec![[0.25, 0.75]; vertices],
                colors: Some(vec![[34, 56, 78, 128]; vertices]),
                triangles: vec![[0, 1, 2]; faces],
            })
            .collect();
        let mut m = Model {
            name: NAME.into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [-13.; 3],
                max: [22.; 3],
            },
            vis_box: Aabb {
                min: [-14.; 3],
                max: [23.; 3],
            },
            sphere: Some([0., 1., 2., 40.]),
            face_count: 777,
            segments,
        };
        for i in 0..4 {
            let point = [i as f32, 13. + i as f32 * 0.5, i as f32];
            m.segments[1].positions[i + 6] = point;
            m.segments[0].positions[i * 2] = point;
            m.segments[0].positions[i * 2 + 1] = point;
        }
        for point in &mut m.segments[1].positions[10..] {
            point[1] = 16.;
        }
        m.segments[1].triangles[0] = [6, 7, 10];
        let corner = [11., 7.88, 9.];
        m.segments[3].positions[467] = corner;
        m.segments[4].positions[58] = corner;
        m.segments[4].positions[59] = corner;
        m.segments[4].positions[45] = [9.42, 8.78, 7.545];
        m.segments[4].positions[57] = [11., 9.144, 7.594];
        m.segments[4].positions[51] = [10.0646, 10., 1.8207];
        m.segments[4].positions[53] = [11., 11.0933, 1.8207];
        m.segments[4].positions[63] = [12., 11., 1.];
        m.segments[4].positions[72] = [12.17, 9., 7.32];
        m.segments[4].positions[78] = [13., 8., 8.];
        m.segments[4].triangles[..FLOOR_REMOVED.len()].copy_from_slice(&FLOOR_REMOVED);
        let ridge = [10.0559, 15.6469, -2.8974];
        m.segments[3].positions[430] = ridge;
        m.segments[3].positions[431] = ridge;
        m.segments[4].positions[49] = ridge;
        m.segments[3].positions[202] = [-3., 20., -6.];
        m.segments[3].positions[211] = [-2.5, 19., -8.];
        m.segments[3].positions[206] = [-2.8, 21.5, -6.2];
        m.segments[3].triangles[372] = [202, 211, 206];
        m
    }

    #[test]
    fn native_and_unknown_inputs_do_not_select_the_profile() {
        assert!(!enabled(None).unwrap());
        assert!(!enabled(Some(b"ucfb\0\0\0\0")).unwrap());
    }

    #[test]
    fn levels_both_weld_streams_without_replacing_donor_attributes() {
        let mut m = fixture();
        let old = m.clone();
        model(&mut m).unwrap();
        for i in 0..4 {
            assert_eq!(m.segments[1].positions[i + 6][1], 16.);
            assert_eq!(m.segments[0].positions[i * 2][1], 16.);
            assert_eq!(m.segments[0].positions[i * 2 + 1][1], 16.);
        }
        assert_eq!(m.segments[1].normals[6], [0., 1., 0.]);
        assert_eq!(m.segments[3].positions[467][1], RIGHT_CORNER_Y);
        assert_eq!(m.segments[4].positions[58][1], RIGHT_CORNER_Y);
        assert_eq!(m.segments[4].positions[59][1], RIGHT_CORNER_Y);
        assert!(m.segments[4].normals[58][1] > 0.);
        let ridge = Vec3::from(old.segments[4].positions[49]) + RIDGE_TRANSLATION;
        assert_eq!(m.segments[4].positions[49], ridge.to_array());
        assert_eq!(m.segments[3].positions[430], ridge.to_array());
        assert_eq!(m.segments[3].positions[431], ridge.to_array());
        assert_eq!(m.segments[0].positions[8..], old.segments[0].positions[8..]);
        assert_eq!(m.vertex_box.min, old.vertex_box.min);
        assert_eq!(m.vis_box.max, old.vis_box.max);
        assert_eq!(m.sphere, old.sphere);
        assert_eq!(m.face_count, old.face_count);
        assert_eq!(
            m.segments[3].triangles.len(),
            old.segments[3].triangles.len() + 1
        );
        assert_eq!(
            m.segments[3].positions.len(),
            old.segments[3].positions.len() + 3
        );
        let lip = Vec3::from(old.segments[3].positions[202]) + UPPER_LIP_TRANSLATION;
        assert_eq!(m.segments[3].positions[595], lip.to_array());
        for (index, (a, b)) in m.segments.iter().zip(&old.segments).enumerate() {
            assert_eq!(&a.uvs[..b.uvs.len()], b.uvs);
            assert_eq!(
                a.colors.as_ref().map(|c| &c[..b.positions.len()]),
                b.colors.as_deref()
            );
            assert_eq!(a.textures, b.textures);
            if index == 4 {
                let mut faces = b.triangles.clone();
                faces.retain(|face| !FLOOR_REMOVED.contains(face));
                faces.extend(FLOOR_ADDED);
                assert_eq!(a.triangles, faces);
            } else {
                assert_eq!(&a.triangles[..b.triangles.len()], b.triangles);
            }
        }
    }

    #[test]
    fn a_later_weld_failure_leaves_every_position_and_normal_unchanged() {
        let mut m = fixture();
        m.segments[1].positions[1] = m.segments[1].positions[6];
        let old = m.clone();
        assert!(model(&mut m).is_err());
        for (a, b) in m.segments.iter().zip(&old.segments) {
            assert_eq!(a.positions, b.positions);
            assert_eq!(a.normals, b.normals);
        }
    }
    #[test]
    fn an_opposite_corner_failure_does_not_commit_the_first_roof_repair() {
        let mut m = fixture();
        m.segments[4].positions[60] = m.segments[4].positions[58];
        let old = m.clone();
        assert!(model(&mut m).is_err());
        for (a, b) in m.segments.iter().zip(&old.segments) {
            assert_eq!(a.positions, b.positions);
            assert_eq!(a.normals, b.normals);
        }
    }
    #[test]
    fn a_changed_floor_face_rejects_all_welds_atomically() {
        let mut m = fixture();
        m.segments[4].triangles[0] = [0, 1, 2];
        let old = m.clone();
        assert!(model(&mut m).is_err());
        for (a, b) in m.segments.iter().zip(&old.segments) {
            assert_eq!(a.positions, b.positions);
            assert_eq!(a.normals, b.normals);
            assert_eq!(a.triangles, b.triangles);
        }
    }
}
