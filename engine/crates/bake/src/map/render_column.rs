//! Dimensional deformation of the donor Utapau column, from its own vertices.
//!
//! The donor ball's latitude spacing, stem direction and stem profile differ
//! from the PS2 artwork and collision. A rigid transform and uniform stretch
//! leave a measured excess above 0.6 m. Keep its ball faces and UVs, deform the
//! existing latitude groups, and form the stem bands from copies of its eight
//! equator directions. The table describes radii and axial band transitions,
//! not a replacement mesh or a list of PS2 vertices. Texture and UV extents are
//! taken exclusively from the player's donor model. Collision and model bounds
//! are not changed. Only the fingerprinted non-PS2 input enters this path.

use anyhow::{ensure, Result};
use glam::Vec3;
use mungefs::model::{Model, Segment};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const MODEL: &str = "uta1_prop_col";
const INPUT_SHA256: &str = "83de312bcf01796789e7e0828da363543896ee39208a09a8cd7225c3b12957c5";
const DONOR_CENTER_XZ: [f32; 2] = [-0.09712195, -0.1141746];
const DONOR_YAW_DEGREES: f32 = -16.309;
const SPHERE_RADIUS: [f32; 7] = [0., 1., 1.732_050_8, 2., 1.732_050_8, 1., 0.];
const SPHERE_Y: [f32; 7] = [15.5, 15.232_051, 14.5, 13.5, 12.5, 11.767_949, 11.5];
// Axial transition heights and radial scales, measured from reference artwork.
const STEM_PROFILE: [(f32, f32); 8] = [
    (-15.5, 0.3),
    (-7.753_19, 0.3),
    (-7.500_12, 0.485),
    (-3.258_46, 0.485),
    (-2.905_11, 0.773),
    (-0.127_48, 0.773),
    (0.225_4, 0.485),
    (13.062_48, 0.485),
];
const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
// Counterclockwise X/Z directions. Selection rotates donor directions; output
// coordinates use these exact unit axes rather than host trigonometric output.
const DIRECTIONS: [[f32; 2]; 8] = [
    [-1., 0.],
    [-DIAGONAL, -DIAGONAL],
    [0., -1.],
    [DIAGONAL, -DIAGONAL],
    [1., 0.],
    [DIAGONAL, DIAGONAL],
    [0., 1.],
    [-DIAGONAL, DIAGONAL],
];

pub(super) fn enabled(original: Option<&[u8]>) -> Result<bool> {
    let Some(original) = original else {
        return Ok(false);
    };
    for chunk in mungefs::content_chunks(mungefs::open(original)?)? {
        if chunk.tag == *b"modl" && chunk.child_string(b"NAME").as_deref() == Some(MODEL) {
            return Ok(format!("{:x}", Sha256::digest(chunk.payload)) == INPUT_SHA256);
        }
    }
    Ok(false)
}

fn direction(point: [f32; 3]) -> usize {
    let x = point[0] - DONOR_CENTER_XZ[0];
    let z = -(point[2] - DONOR_CENTER_XZ[1]);
    let (sine, cosine) = DONOR_YAW_DEGREES.to_radians().sin_cos();
    let [x, z] = [cosine * x - sine * z, sine * x + cosine * z];
    DIRECTIONS
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| (a[0] * x + a[1] * z).total_cmp(&(b[0] * x + b[1] * z)))
        .map(|(index, _)| index)
        .unwrap()
}

fn component(segment: &Segment, first: usize) -> HashSet<usize> {
    let mut component = HashSet::from([first]);
    loop {
        let before = component.len();
        for triangle in &segment.triangles {
            if triangle
                .iter()
                .any(|&index| component.contains(&(index as usize)))
            {
                component.extend(triangle.iter().map(|&index| index as usize));
            }
        }
        if before == component.len() {
            return component;
        }
    }
}

pub(super) fn model(model: &mut Model) -> Result<()> {
    if !model.name.eq_ignore_ascii_case(MODEL) {
        return Ok(());
    }
    ensure!(
        !model.bone_space && model.segments.len() == 1,
        "render column: {MODEL} segmentation changed"
    );
    let old = &model.segments[0];
    ensure!(
        old.positions.len() == 138
            && old.uvs.len() == 138
            && old.normals.len() == 138
            && old.bone.as_deref() == Some("uta1_bldg_generatorC"),
        "render column: {MODEL} donor groups changed"
    );
    ensure!(
        old.positions.iter().flatten().all(|x| x.is_finite()),
        "render column: nonfinite donor position"
    );
    let sphere = component(old, 0);
    ensure!(
        sphere.len() == 49,
        "render column: {MODEL} ball topology changed"
    );
    let mut heights: Vec<_> = sphere
        .iter()
        .map(|&index| old.positions[index][1])
        .collect();
    heights.sort_by(f32::total_cmp);
    heights.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
    ensure!(
        heights.len() == 7,
        "render column: {MODEL} ball latitude groups changed"
    );
    let latitude = |index: usize| {
        heights
            .iter()
            .position(|height| (old.positions[index][1] - height).abs() < 0.0001)
            .unwrap()
    };
    let mut ring = [None; 8];
    for index in 0..old.positions.len() {
        if sphere.contains(&index) && latitude(index) == 3 {
            ring[direction(old.positions[index])].get_or_insert(index);
        }
    }
    ensure!(
        ring.iter().all(Option::is_some),
        "render column: incomplete donor equator"
    );

    let mut segment = old.clone();
    segment.positions.clear();
    segment.normals.clear();
    segment.uvs.clear();
    if let Some(colors) = &mut segment.colors {
        colors.clear();
    }
    segment.triangles.clear();
    let mut map = [usize::MAX; 138];
    for index in 0..old.positions.len() {
        if !sphere.contains(&index) {
            continue;
        }
        let row = latitude(index);
        let [x, z] = DIRECTIONS[direction(old.positions[index])];
        let position = [
            x * SPHERE_RADIUS[row],
            SPHERE_Y[row],
            z * SPHERE_RADIUS[row],
        ];
        map[index] = segment.positions.len();
        segment.positions.push(position);
        segment.normals.push(
            (Vec3::from(position) - Vec3::new(0., 13.5, 0.))
                .normalize()
                .to_array(),
        );
        segment.uvs.push(old.uvs[index]);
        if let (Some(colors), Some(original)) = (&mut segment.colors, &old.colors) {
            colors.push(original[index]);
        }
    }
    for triangle in &old.triangles {
        if triangle
            .iter()
            .all(|&index| sphere.contains(&(index as usize)))
        {
            segment
                .triangles
                .push(triangle.map(|index| map[index as usize] as u32));
        }
    }
    // This donor stem's axial/circumferential UV bounds, read from its own two
    // end rings. Interpolation retains its texture atlas and stretches it over
    // the corrected bands. No reference UVs or pixels are imported.
    let uv_indices = (61..70).chain([112, 114, 115, 116, 119, 120, 121]);
    let (mut u_min, mut u_max, mut v_min, mut v_max) = (
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
    );
    for index in uv_indices {
        let [u, v] = old.uvs[index];
        u_min = u_min.min(u);
        u_max = u_max.max(u);
        v_min = v_min.min(v);
        v_max = v_max.max(v);
    }
    let base = segment.positions.len() as u32;
    for (row, &(y, radius)) in STEM_PROFILE.iter().enumerate() {
        let slope = if row == 0 || row == STEM_PROFILE.len() - 1 {
            0.
        } else {
            (STEM_PROFILE[row + 1].1 - STEM_PROFILE[row - 1].1)
                / (STEM_PROFILE[row + 1].0 - STEM_PROFILE[row - 1].0)
        };
        let fraction = (y - STEM_PROFILE[0].0) / (STEM_PROFILE[7].0 - STEM_PROFILE[0].0);
        for index in 0..=8 {
            let k = index % 8;
            let [x, z] = DIRECTIONS[k];
            let original = ring[k].unwrap();
            segment.positions.push([x * radius, y, z * radius]);
            segment
                .normals
                .push(Vec3::new(x, -slope, z).normalize().to_array());
            segment.uvs.push([
                u_max + (u_min - u_max) * fraction,
                v_min + (v_max - v_min) * index as f32 / 8.,
            ]);
            if let (Some(colors), Some(original_colors)) = (&mut segment.colors, &old.colors) {
                colors.push(original_colors[original]);
            }
        }
    }
    for row in 0..7 {
        for index in 0..8 {
            let a = base + row * 9 + index;
            let b = a + 9;
            segment.triangles.extend([[a, b, a + 1], [a + 1, b, b + 1]]);
        }
    }
    // The lower end is closed with copies of the donor-derived first ring.
    // Give the cap its own downward normals and the donor end-cap's UV extent,
    // instead of collapsing all its UVs onto the stem's seam line.
    let cap = component(old, 125);
    ensure!(cap.len() == 7, "render column: donor end cap changed");
    let (mut cap_min, mut cap_max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for index in cap {
        for axis in 0..2 {
            cap_min[axis] = cap_min[axis].min(old.uvs[index][axis]);
            cap_max[axis] = cap_max[axis].max(old.uvs[index][axis]);
        }
    }
    let cap_base = segment.positions.len() as u32;
    for (index, &[x, z]) in DIRECTIONS.iter().enumerate() {
        segment
            .positions
            .push(segment.positions[base as usize + index]);
        segment.normals.push([0., -1., 0.]);
        segment.uvs.push([
            cap_min[0] + (cap_max[0] - cap_min[0]) * (x + 1.) * 0.5,
            cap_min[1] + (cap_max[1] - cap_min[1]) * (z + 1.) * 0.5,
        ]);
        if let Some(colors) = &mut segment.colors {
            colors.push(colors[base as usize + index]);
        }
    }
    for index in 1..7 {
        segment
            .triangles
            .push([cap_base, cap_base + index, cap_base + index + 1]);
    }
    segment.vertex_count = segment.positions.len() as u32;
    segment.index_count = segment.triangles.len() as u32 * 3;
    model.segments[0] = segment;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_donor_keeps_ball_faces_and_uvs_while_reaching_reference_profile() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../extracted/xbox_bf2/uta/uta1.lvl");
        if !path.is_file() {
            eprintln!("SKIP: Xbox Utapau input absent");
            return;
        }
        let (corrected, original) =
            crate::gameplay::read_with_original(path.to_str().unwrap()).unwrap();
        assert!(enabled(original.as_deref()).unwrap());
        assert!(!enabled(None).unwrap());
        let chunks = mungefs::content_chunks(mungefs::open(&corrected).unwrap()).unwrap();
        let chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.tag == *b"modl" && chunk.child_string(b"NAME").as_deref() == Some(MODEL)
            })
            .unwrap();
        let mut output = Model::parse(chunk).unwrap();
        let before = output.clone();
        let sphere = component(&before.segments[0], 0);
        let ball_faces = before.segments[0]
            .triangles
            .iter()
            .filter(|tri| tri.iter().all(|&i| sphere.contains(&(i as usize))))
            .count();
        model(&mut output).unwrap();
        let segment = &output.segments[0];
        assert_eq!(segment.positions.len(), 49 + STEM_PROFILE.len() * 9 + 8);
        assert_eq!(segment.triangles.len(), ball_faces + 7 * 8 * 2 + 6);
        assert_eq!(segment.textures, before.segments[0].textures);
        let original_uvs: Vec<_> = (0..138)
            .filter(|i| sphere.contains(i))
            .map(|i| before.segments[0].uvs[i])
            .collect();
        assert_eq!(&segment.uvs[..49], original_uvs.as_slice());
        for point in &segment.positions {
            assert!(point.iter().all(|v| v.is_finite()));
            assert!(point[1] >= -15.5 && point[1] <= 15.5);
        }
        assert!(model(&mut output).is_err());
    }
}
