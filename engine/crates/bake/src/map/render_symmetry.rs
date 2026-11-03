//! Reconstruct a verified missing symmetric half from the player's donor art.
//!
//! DEA1's Xbox blast door carries the upper frame and each upper leaf only.
//! The PS2's frame and leaves have both halves; its corrected diagonal leaf
//! primitives also extend below y=0. Reflecting the existing donor groups in
//! that plane restores the leaves. The lower housing has a simpler opening:
//! rounded corner pairs become their own midpoints, the upper-only ribs are
//! flattened, and its inner walls join the donor's front/back opening vertices.
//! This runs only after a fingerprinted non-PS2 correction, and after
//! the donor bind pose. The named groups and their measured envelope are guards
//! against applying the recipe to a different asset revision.

use anyhow::{ensure, Result};
use manifest::ManifestDoor;
use mungefs::model::{Model, Segment};
use sha2::{Digest, Sha256};

/// Public reconstruction instructions: model, reflection axis/plane and the
/// original donor groups. Original groups keep their order, reflected copies
/// follow in the same order and retain the original bone/material/UV binding.
const MODEL: &str = "dea1_prop_door_blast";
const INPUT_SHA256: &str = "8d61ec5958cee2375c8aca13142c2708323030f58dc05ea499430eeae28a702e";
const AXIS: usize = 1;
const PLANE: f32 = 0.0;
const BONES: [&str; 3] = ["dummyroot", "bone_rightdoors", "bone_leftdoors"];
/// The Xbox's f32 vertices on the symmetry plane deviate by less than 1 um.
const PLANE_EPSILON: f32 = 0.000001;
/// Public donor-profile instructions. Each pair of adjacent rounded opening
/// corners collapses to its midpoint. The two centre points stay in place.
/// These are selection bands in the fingerprinted donor, not target vertices.
const CORNER_BANDS: [(f32, f32); 2] = [(2.9, 3.3), (3.8, 4.1)];
const PROFILE_EPSILON: f32 = 0.001;
/// Quantization leaves nominally depth-only triangles with tiny X/Y area.
const FRONT_PROJECTED_AREA: f32 = 0.001;

fn inner_opening(point: [f32; 3], centre_x: f32) -> bool {
    let x = (point[0] - centre_x).abs();
    let h = -point[1];
    ((x - 4.0).abs() < PROFILE_EPSILON && (1.29..3.71).contains(&h))
        || ((h - 0.25).abs() < PROFILE_EPSILON && x < 2.951)
        || ((h - 4.75).abs() < PROFILE_EPSILON && x < 2.951)
        || ((x - 3.2494).abs() < PROFILE_EPSILON
            && ((h - 0.37396).abs() < PROFILE_EPSILON || (h - 4.626).abs() < PROFILE_EPSILON))
        || ((x - 3.87608).abs() < PROFILE_EPSILON
            && ((h - 1.0006).abs() < PROFILE_EPSILON || (h - 3.99927).abs() < PROFILE_EPSILON))
}

fn push_face(out: &mut Segment, donor: &Segment, corners: [(usize, [f32; 3]); 3]) {
    let a = corners[0].1;
    let b = corners[1].1;
    let c = corners[2].1;
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length < 0.000001 {
        return; // Collapsing a rounded corner intentionally removes empty faces.
    }
    let first = out.positions.len() as u32;
    for (index, point) in corners {
        out.positions.push(point);
        out.normals.push(n.map(|x| x / length));
        out.uvs.push(donor.uvs[index]);
        if !donor.bind.is_empty() {
            out.bind.push(donor.bind[index]);
        }
        if !donor.wght.is_empty() {
            out.wght.push(donor.wght[index]);
        }
        if let (Some(colors), Some(old)) = (&mut out.colors, &donor.colors) {
            colors.push(old[index]);
        }
    }
    out.triangles.push([first, first + 1, first + 2]);
}

/// Simplify the reflected housing using its own opening, depths and UVs.
/// No reference mesh or collision geometry is read by this operation.
fn lower_frame(segment: &mut Segment) -> Result<()> {
    let donor = segment.clone();
    let count = donor.positions.len();
    ensure!(
        donor.uvs.len() == count
            && (donor.bind.is_empty() || donor.bind.len() == count)
            && (donor.wght.is_empty() || donor.wght.len() == count)
            && donor.colors.as_ref().is_none_or(|v| v.len() == count),
        "render symmetry: {MODEL} donor vertex streams changed"
    );
    let centre_x = donor
        .positions
        .iter()
        .find(|p| p[0].abs() < PROFILE_EPSILON)
        .map(|p| p[0])
        .ok_or_else(|| anyhow::anyhow!("render symmetry: {MODEL} centre absent"))?;
    let mut corners = [[[[0.0; 2]; 2]; 2]; 2];
    for (side, side_corners) in corners.iter_mut().enumerate() {
        for (half, half_corners) in side_corners.iter_mut().enumerate() {
            for (band, target) in half_corners.iter_mut().enumerate() {
                let points = donor.positions.iter().filter(|&&p| {
                    let x = p[0] - centre_x;
                    inner_opening(p, centre_x)
                        && (x > 0.0) == (side == 1)
                        && (-p[1] > 2.5) == (half == 1)
                        && (CORNER_BANDS[band].0..CORNER_BANDS[band].1).contains(&x.abs())
                });
                let (mut lo, mut hi, mut found) =
                    ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2], false);
                for p in points {
                    found = true;
                    for axis in 0..2 {
                        lo[axis] = lo[axis].min(p[axis]);
                        hi[axis] = hi[axis].max(p[axis]);
                    }
                }
                ensure!(found, "render symmetry: {MODEL} opening corner absent");
                *target = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
            }
        }
    }
    let deform = |mut p: [f32; 3]| {
        if inner_opening(p, centre_x) {
            let x = p[0] - centre_x;
            if let Some(band) = CORNER_BANDS
                .iter()
                .position(|&(lo, hi)| (lo..hi).contains(&x.abs()))
            {
                let q = corners[usize::from(x > 0.0)][usize::from(-p[1] > 2.5)][band];
                p[0] = q[0];
                p[1] = q[1];
            }
        }
        p
    };
    let xmin = donor
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min);
    let xmax = donor
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let ymin = donor
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    let back = donor
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min);
    let front = donor
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max);
    segment.positions.clear();
    segment.normals.clear();
    segment.uvs.clear();
    segment.bind.clear();
    segment.wght.clear();
    if let Some(colors) = &mut segment.colors {
        colors.clear();
    }
    segment.triangles.clear();
    for triangle in &donor.triangles {
        let points = triangle.map(|i| donor.positions[i as usize]);
        let xy = (points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
            - (points[1][1] - points[0][1]) * (points[2][0] - points[0][0]);
        let planar = xy.abs() > FRONT_PROJECTED_AREA;
        let outer_wall = points.iter().all(|p| (p[0] - xmin).abs() < PROFILE_EPSILON)
            || points.iter().all(|p| (p[0] - xmax).abs() < PROFILE_EPSILON)
            || points.iter().all(|p| (p[1] - ymin).abs() < PROFILE_EPSILON);
        if planar || outer_wall {
            push_face(
                segment,
                &donor,
                triangle.map(|i| {
                    let mut p = deform(donor.positions[i as usize]);
                    if planar {
                        p[2] = if xy > 0.0 { front } else { back };
                    }
                    (i as usize, p)
                }),
            );
        }
    }
    // Front and back copies of every opening point already exist in the donor.
    // Join those vertices directly, closing the upper housing's centre-depth gap.
    let mut ring: Vec<(usize, usize, [f32; 3])> = Vec::new();
    for (i, &p) in donor.positions.iter().enumerate() {
        if !inner_opening(p, centre_x) || (p[2] - front).abs() > PROFILE_EPSILON {
            continue;
        }
        let q = deform(p);
        if ring
            .iter()
            .any(|(_, _, old)| (old[0] - q[0]).hypot(old[1] - q[1]) < PROFILE_EPSILON)
        {
            continue;
        }
        let j = donor
            .positions
            .iter()
            .position(|&p| {
                let old = deform(p);
                (p[2] - back).abs() < PROFILE_EPSILON
                    && (old[0] - q[0]).hypot(old[1] - q[1]) < PROFILE_EPSILON
            })
            .ok_or_else(|| anyhow::anyhow!("render symmetry: {MODEL} back opening absent"))?;
        ring.push((i, j, q));
    }
    ensure!(
        ring.len() == 10,
        "render symmetry: {MODEL} opening point count changed"
    );
    let centre_y = (ring.iter().map(|p| p.2[1]).fold(f32::INFINITY, f32::min)
        + ring
            .iter()
            .map(|p| p.2[1])
            .fold(f32::NEG_INFINITY, f32::max))
        * 0.5;
    ring.sort_by(|a, b| {
        (a.2[1] - centre_y)
            .atan2(a.2[0] - centre_x)
            .total_cmp(&(b.2[1] - centre_y).atan2(b.2[0] - centre_x))
    });
    for index in 0..ring.len() {
        let (ai, aj, a) = ring[index];
        let (bi, bj, b) = ring[(index + 1) % ring.len()];
        let mut ab = a;
        let mut bb = b;
        ab[2] = back;
        bb[2] = back;
        push_face(segment, &donor, [(ai, a), (bi, b), (bj, bb)]);
        push_face(segment, &donor, [(ai, a), (bj, bb), (aj, ab)]);
    }
    segment.vertex_count = segment.positions.len() as u32;
    segment.index_count = segment.triangles.len() as u32 * 3;
    Ok(())
}

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

/// Returns the original primitive count when the recipe was applied.
pub(super) fn model(model: &mut Model) -> Result<Option<usize>> {
    if !model.name.eq_ignore_ascii_case(MODEL) {
        return Ok(None);
    }
    ensure!(!model.bone_space, "render symmetry: {MODEL} is not at bind");
    ensure!(
        model.segments.len() == BONES.len(),
        "render symmetry: {MODEL} group count changed"
    );
    for (segment, bone) in model.segments.iter().zip(BONES) {
        ensure!(
            segment.bone.as_deref() == Some(bone),
            "render symmetry: {MODEL} group binding changed"
        );
        ensure!(
            !segment.positions.is_empty() && !segment.triangles.is_empty(),
            "render symmetry: {MODEL} empty donor group"
        );
        ensure!(
            segment
                .positions
                .iter()
                .all(|point| point.iter().all(|v| v.is_finite())
                    && point[AXIS] >= PLANE - PLANE_EPSILON
                    && point[AXIS] <= 6.00001
                    && point[0].abs() <= 6.0003
                    && point[2].abs() <= 0.98001),
            "render symmetry: {MODEL} donor envelope changed"
        );
        ensure!(
            segment.positions.iter().any(|point| point[AXIS] > 4.99),
            "render symmetry: {MODEL} upper half absent"
        );
    }
    let original_count = model.segments.len();
    let mut reflected = model
        .segments
        .iter()
        .cloned()
        .map(|mut segment| {
            // Do not duplicate faces lying in the join plane: they would fight the
            // original face at exactly the same depth. The original is retained.
            segment.triangles.retain(|triangle| {
                !triangle.iter().all(|&index| {
                    (segment.positions[index as usize][AXIS] - PLANE).abs() <= PLANE_EPSILON
                })
            });
            for position in &mut segment.positions {
                position[AXIS] = 2.0 * PLANE - position[AXIS];
            }
            for normal in &mut segment.normals {
                normal[AXIS] = -normal[AXIS];
            }
            for triangle in &mut segment.triangles {
                triangle.swap(1, 2);
            }
            segment.index_count = segment.triangles.len() as u32 * 3;
            segment
        })
        .collect::<Vec<_>>();
    lower_frame(&mut reflected[0])?;
    model.segments.extend(reflected);
    Ok(Some(original_count))
}

/// The reflection commutes with this door's horizontal leaf motion. Give each
/// reflected leaf the same canonical motion, addressing its new GLB primitive.
/// An authored rotation or non-horizontal translation invalidates that proof.
pub(super) fn door_parts(door: &mut ManifestDoor, original_count: usize) -> Result<()> {
    ensure!(
        original_count == BONES.len(),
        "render symmetry: invalid original door group count"
    );
    for part in &door.parts {
        ensure!(
            (part.submesh as usize) < original_count,
            "render symmetry: invalid donor door part"
        );
        ensure!(
            part.motion
                .iter()
                .all(|(rotation, translation)| rotation[0].abs() < PLANE_EPSILON
                    && rotation[1].abs() < PLANE_EPSILON
                    && rotation[2].abs() < PLANE_EPSILON
                    && (rotation[3].abs() - 1.0).abs() < PLANE_EPSILON
                    && translation[1].abs() < PLANE_EPSILON
                    && translation[2].abs() < PLANE_EPSILON),
            "render symmetry: {MODEL} leaf motion no longer commutes with reflection"
        );
    }
    let copies = door
        .parts
        .iter()
        .cloned()
        .map(|mut part| {
            part.submesh += original_count as u32;
            part
        })
        .collect::<Vec<_>>();
    door.parts.extend(copies);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real, corrected disc models: the donor omits exactly the lower half;
    /// PS2 controls bypass the reconstruction entirely in bake_content.
    #[test]
    fn restores_missing_door_halves_and_moves_each_leaf() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../extracted/xbox_bf2/dea/dea1.lvl");
        if !root.is_file() {
            eprintln!("SKIP: Xbox DEA1 input absent");
            return;
        }
        let (corrected, original) =
            crate::gameplay::read_with_original(root.to_str().unwrap()).unwrap();
        let original = original.expect("Xbox source must take fingerprinted corrections");
        assert!(enabled(Some(&original)).unwrap());
        assert!(!enabled(None).unwrap());
        let chunks = mungefs::content_chunks(mungefs::open(&corrected).unwrap()).unwrap();
        let mut bind = crate::bind_pose::BindPose::of(&chunks);
        bind.add_render_source(&original).unwrap();
        let raw = chunks
            .iter()
            .find(|c| c.tag == *b"modl" && c.child_string(b"NAME").as_deref() == Some(MODEL))
            .unwrap();
        let parsed = Model::parse(raw).unwrap();
        let mut posed = bind.pose(&parsed).unwrap().unwrap_or(parsed);
        let before = posed.clone();
        let original_count = model(&mut posed).unwrap().unwrap();
        assert_eq!(posed.segments.len(), 6);
        // The upper groups stay unchanged; both leaves remain exact reflections.
        // Only the copied housing uses the donor-profile simplification.
        for (a, b) in before.segments.iter().zip(&posed.segments) {
            assert_eq!(a.positions, b.positions);
            assert_eq!(a.triangles, b.triangles);
        }
        for (old, new) in before
            .segments
            .iter()
            .zip(&posed.segments[original_count..])
            .skip(1)
        {
            for (a, b) in old.positions.iter().zip(&new.positions) {
                assert_eq!(a[0], b[0]);
                assert_eq!(a[1], -b[1]);
                assert_eq!(a[2], b[2]);
            }
            assert_eq!(old.bone, new.bone);
            assert_eq!(old.textures, new.textures);
        }
        let frame = &posed.segments[original_count];
        assert_eq!(frame.bone, before.segments[0].bone);
        assert_eq!(frame.textures, before.segments[0].textures);
        assert!(frame.triangles.len() < before.segments[0].triangles.len());
        assert_eq!(frame.vertex_count as usize, frame.positions.len());
        assert_eq!(frame.positions.len(), frame.normals.len());
        assert_eq!(frame.positions.len(), frame.uvs.len());
        assert!(frame
            .uvs
            .iter()
            .all(|uv| before.segments[0].uvs.contains(uv)));
        assert!(frame.normals.iter().all(|n| {
            let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
            (length - 1.0).abs() < 0.00001
        }));
        let mut rig = crate::door::rigs(&chunks)
            .into_iter()
            .find(|(name, _)| name == MODEL)
            .unwrap()
            .1
            .unwrap();
        let old = rig.parts.clone();
        door_parts(&mut rig, original_count).unwrap();
        assert_eq!(rig.parts.len(), old.len() * 2);
        for (a, b) in old.iter().zip(&rig.parts[old.len()..]) {
            assert_eq!(a.submesh + original_count as u32, b.submesh);
            assert_eq!(a.bone, b.bone);
            assert_eq!(a.motion, b.motion);
        }
        // Re-applying must fail rather than silently duplicating six groups.
        assert!(model(&mut posed).is_err());
        rig.parts[0].motion[1].1[1] = 1.0;
        assert!(door_parts(&mut rig, original_count).is_err());
    }
}
