//! Lower-detail artwork for a model proven absent from the donor's entire LVL
//! inventory. Shape comes from the already-corrected collision instructions;
//! materials and projected UVs come from that copy's placed artwork.

use super::render_recovery::{clip, empty_segment, Vertex};
use anyhow::{anyhow, Result};
use glam::{Mat3, Vec3};
use mungefs::{
    model::{Aabb, Model},
    Chunk,
};

/// Closest point on a triangle, returned as barycentric weights. Degenerate
/// faces use their closest edge rather than dividing by a zero area.
fn weights(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
    let ab = b - a;
    let ac = c - a;
    if ab.cross(ac).length_squared() < 1e-12 {
        let mut best = (f32::INFINITY, [1.0, 0.0, 0.0]);
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let vertices = [a, b, c];
            let delta = vertices[j] - vertices[i];
            let t = if delta.length_squared() > 0.0 {
                ((p - vertices[i]).dot(delta) / delta.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance = p.distance_squared(vertices[i] + delta * t);
            if distance < best.0 {
                let mut w = [0.0; 3];
                w[i] = 1.0 - t;
                w[j] = t;
                best = (distance, w);
            }
        }
        return best.1;
    }
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [1.0 - w, 0.0, w];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }
    let inverse = 1.0 / (va + vb + vc);
    let v = vb * inverse;
    let w = vc * inverse;
    [1.0 - v - w, v, w]
}

fn nearest(
    p: Vec3,
    model: &Model,
    only_segment: Option<usize>,
) -> Option<(usize, [u32; 3], [f32; 3])> {
    let mut best = f32::INFINITY;
    let mut found = None;
    for (si, seg) in model.segments.iter().enumerate() {
        if only_segment.is_some_and(|i| i != si) {
            continue;
        }
        for &tri in &seg.triangles {
            let xyz = tri.map(|i| Vec3::from(seg.positions[i as usize]));
            let w = weights(p, xyz[0], xyz[1], xyz[2]);
            let q = xyz[0] * w[0] + xyz[1] * w[1] + xyz[2] * w[2];
            let distance = p.distance_squared(q);
            if distance < best {
                best = distance;
                found = Some((si, tri, w));
            }
        }
    }
    found
}

fn shapes(name: &str, items: &[Chunk<'_>]) -> Result<Vec<[Vec3; 3]>> {
    let mut triangles = Vec::new();
    let bind = crate::bind_pose::BindPose::of(items);
    // A mesh describes the piece's surface more closely than an overlapping
    // coarse primitive. Drawing both can duplicate coplanar faces.
    let has_mesh = items.iter().any(|c| {
        &c.tag == b"coll"
            && c.child_string(b"NAME")
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
    });
    for c in items {
        if &c.tag == b"coll"
            && c.child_string(b"NAME")
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        {
            let mesh = mungefs::coll::CollisionMesh::parse(c).map_err(|e| anyhow!("{e}"))?;
            triangles.extend(
                mesh.triangles()
                    .into_iter()
                    .map(|t| t.map(|i| mesh.vertices[i as usize].into())),
            );
        } else if &c.tag == b"prim" && !has_mesh {
            let prims = mungefs::coll::Primitives::parse(c).map_err(|e| anyhow!("{e}"))?;
            if !prims.model.eq_ignore_ascii_case(name) {
                continue;
            }
            let (mut prims, _) = super::manifest_prims(&prims);
            bind.rest_prims(name, &mut prims);
            for prim in prims {
                let transform = Mat3::from_cols_array_2d(&prim.rotation);
                let translate = Vec3::from(prim.translation);
                let mut faces = Vec::new();
                match prim.kind.as_str() {
                    "box" => {
                        let half = Vec3::from(prim.dims);
                        let points: Vec<_> = (0..8)
                            .map(|i| {
                                Vec3::new(
                                    if i & 1 == 0 { -half.x } else { half.x },
                                    if i & 2 == 0 { -half.y } else { half.y },
                                    if i & 4 == 0 { -half.z } else { half.z },
                                )
                            })
                            .collect();
                        for quad in [
                            [0, 4, 6, 2],
                            [1, 3, 7, 5],
                            [0, 1, 5, 4],
                            [2, 6, 7, 3],
                            [0, 2, 3, 1],
                            [4, 5, 7, 6],
                        ] {
                            faces.push([points[quad[0]], points[quad[1]], points[quad[2]]]);
                            faces.push([points[quad[0]], points[quad[2]], points[quad[3]]]);
                        }
                    }
                    "cylinder" => {
                        let point = |i: u32, y: f32| {
                            let a = (i % 32) as f32 * std::f32::consts::TAU / 32.0;
                            Vec3::new(a.cos() * prim.dims[0], y, a.sin() * prim.dims[0])
                        };
                        let half = prim.dims[1] * 0.5;
                        for i in 0..32 {
                            let a = point(i, -half);
                            let b = point(i + 1, -half);
                            let c = point(i, half);
                            let d = point(i + 1, half);
                            faces.extend([
                                [a, c, d],
                                [a, d, b],
                                [Vec3::new(0.0, -half, 0.0), a, b],
                                [Vec3::new(0.0, half, 0.0), d, c],
                            ]);
                        }
                    }
                    "sphere" => {
                        let point = |lat: u32, lon: u32| {
                            let y = -std::f32::consts::FRAC_PI_2
                                + lat as f32 * std::f32::consts::PI / 16.0;
                            let a = (lon % 32) as f32 * std::f32::consts::TAU / 32.0;
                            Vec3::new(y.cos() * a.cos(), y.sin(), y.cos() * a.sin()) * prim.dims[0]
                        };
                        for lat in 0..16 {
                            for lon in 0..32 {
                                let a = point(lat, lon);
                                let b = point(lat, lon + 1);
                                let c = point(lat + 1, lon);
                                let d = point(lat + 1, lon + 1);
                                faces.extend([[a, c, d], [a, d, b]]);
                            }
                        }
                    }
                    _ => {}
                }
                triangles.extend(
                    faces
                        .into_iter()
                        .map(|t| t.map(|p| transform * p + translate)),
                );
            }
        }
    }
    Ok(triangles)
}

pub(super) fn fit(
    name: &str,
    items: &[Chunk<'_>],
    artwork: &Model,
    bounds: Aabb,
) -> Result<Option<Model>> {
    let mut result = artwork.clone();
    result.segments = artwork.segments.iter().map(empty_segment).collect();
    // The donor's baked shadow colors belong to its old surface, not this new
    // geometry. Let the generated normals use the map's ordinary lighting.
    for segment in &mut result.segments {
        segment.colors = None;
        segment.material_flags &= if segment.bf2_material { !0x200 } else { !0x40 };
    }
    for triangle in shapes(name, items)? {
        let normal = (triangle[1] - triangle[0])
            .cross(triangle[2] - triangle[0])
            .normalize_or_zero();
        if normal == Vec3::ZERO {
            continue;
        }
        let polygon = clip(
            triangle
                .into_iter()
                .map(|p| Vertex {
                    p,
                    n: normal,
                    uv: [0.0; 2],
                    color: [255.0; 4],
                })
                .collect(),
            bounds.min.into(),
            bounds.max.into(),
        );
        for i in 1..polygon.len().saturating_sub(1) {
            let vertices = [polygon[0].p, polygon[i].p, polygon[i + 1].p];
            if (vertices[1] - vertices[0])
                .cross(vertices[2] - vertices[0])
                .length_squared()
                < 1e-12
            {
                continue;
            }
            let center = (vertices[0] + vertices[1] + vertices[2]) / 3.0;
            let Some((segment, source_face, _)) = nearest(center, artwork, None) else {
                continue;
            };
            let source = &artwork.segments[segment];
            // Collision leaf winding is not a facing guarantee. Keep its
            // geometric normal oriented toward the donor artwork's normal.
            let donor_normal = source_face
                .into_iter()
                .map(|v| Vec3::from(source.normals[v as usize]))
                .sum::<Vec3>();
            let normal = if normal.dot(donor_normal) < 0.0 {
                -normal
            } else {
                normal
            };
            let output = &mut result.segments[segment];
            let base = output.positions.len() as u32;
            for p in vertices {
                let (_, source_tri, w) = nearest(p, artwork, Some(segment)).unwrap();
                let uv = std::array::from_fn(|axis| {
                    (0..3)
                        .map(|j| source.uvs[source_tri[j] as usize][axis] * w[j])
                        .sum()
                });
                output.positions.push(p.into());
                output.normals.push(normal.into());
                output.uvs.push(uv);
            }
            output.triangles.push([base, base + 1, base + 2]);
        }
    }
    result.segments.retain(|s| !s.triangles.is_empty());
    if result.segments.is_empty() {
        return Ok(None);
    }
    for s in &mut result.segments {
        s.vertex_count = s.positions.len() as u32;
        s.index_count = (s.triangles.len() * 3) as u32;
    }
    result.face_count = result
        .segments
        .iter()
        .map(|s| s.triangles.len() as u32)
        .sum();
    eprintln!(
        "render recovery: {name}: lower-detail textured collision stand-in, {} triangles",
        result.face_count
    );
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standin_uses_canonical_shape_and_only_the_donors_artwork() {
        use mungefs::model::{RenderType, Segment};
        let resource = |tag: &[u8; 4], body: &[u8]| {
            let mut out = tag.to_vec();
            out.extend((body.len() as u32).to_le_bytes());
            out.extend(body);
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
            out
        };
        let bounds = Aabb {
            min: [0.0; 3],
            max: [1.0, 1.0, 0.0],
        };
        let mut info = Vec::new();
        for v in [3u32, 0, 1, 3] {
            info.extend(v.to_le_bytes());
        }
        for v in bounds.min.into_iter().chain(bounds.max) {
            info.extend(v.to_le_bytes());
        }
        let mut points = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for v in p {
                points.extend(v.to_le_bytes());
            }
        }
        let leaf = [3u8, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 2, 0];
        let payload = [
            resource(b"NAME", b"missing\0"),
            resource(b"INFO", &info),
            resource(b"POSI", &points),
            resource(b"TREE", &resource(b"LEAF", &leaf)),
        ]
        .concat();
        let chunk = Chunk {
            tag: *b"coll",
            payload: &payload,
            offset: 0,
        };
        let segment = Segment {
            vertex_count: 3,
            index_count: 3,
            bf2_material: false,
            pc_geometry: false,
            material_flags: 0,
            render_type: RenderType::Bf1("Normal".into()),
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0; 2],
            textures: vec![(0, "own_stone".into())],
            bone: None,
            group: None,
            bmap: vec![],
            bind: vec![],
            wght: vec![],
            positions: vec![[0.0, 0.0, 5.0], [1.0, 0.0, 5.0], [0.0, 1.0, 5.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: Some(vec![[128; 4]; 3]),
            triangles: vec![[0, 1, 2]],
        };
        let model = Model {
            name: "missing".into(),
            bone_space: false,
            vertex_box: bounds,
            vis_box: bounds,
            face_count: 1,
            sphere: None,
            segments: vec![segment],
        };
        let result = fit("missing", &[chunk], &model, bounds).unwrap().unwrap();
        assert_eq!(
            result.segments[0].positions,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(result.segments[0].uvs, model.segments[0].uvs);
        assert_eq!(result.segments[0].textures, vec![(0, "own_stone".into())]);
        assert_eq!(result.segments[0].colors, None);
    }

    #[test]
    fn nearest_projection_preserves_interior_uv_coordinates_and_handles_degeneracy() {
        assert_eq!(
            weights(Vec3::new(0.25, 0.5, 2.0), Vec3::ZERO, Vec3::X, Vec3::Y),
            [0.25, 0.25, 0.5]
        );
        assert_eq!(
            weights(Vec3::new(0.5, 1.0, 0.0), Vec3::ZERO, Vec3::X, Vec3::X),
            [0.5, 0.5, 0.0]
        );
    }
}
