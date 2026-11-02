use super::*;
use parry3d::bounding_volume::Aabb;
use parry3d::na::{Point3, Vector3};

fn next(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / (1u32 << 24) as f32 * 40.0 - 20.0
}

#[test]
fn cached_bounds_cost_is_bit_identical_to_parry() {
    let mut seed = 0x0093_ca57u32;
    let special = [
        -0.0,
        0.0,
        1.0,
        -1.0,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        -f32::from_bits(1),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    let mut hits = 0;
    for k in 0..50_000 {
        let min = Point3::new(next(&mut seed), next(&mut seed), next(&mut seed));
        let max = min
            + Vector3::new(
                next(&mut seed).abs(),
                next(&mut seed).abs(),
                next(&mut seed).abs(),
            );
        let node = BvhNode::leaf(Aabb::new(min, max), 0);
        let mut origin = Point3::new(next(&mut seed), next(&mut seed), next(&mut seed));
        // Include slab boundaries, corners and origins inside the box.
        if k % 3 == 0 {
            origin = min;
        } else if k % 3 == 1 {
            origin = min + (max - min) * 0.5;
        }
        let mut direction = Vector3::new(next(&mut seed), next(&mut seed), next(&mut seed));
        for axis in 0..3 {
            if k % (axis + 2) == 0 {
                direction[axis] = special[(k / (axis + 1)) % special.len()];
            }
        }
        let ray = Ray::new(origin, direction);
        let bounds = RayBounds::new(&ray);
        for limit in [-1.0, 0.0, 1.0, 100.0, f32::MAX, f32::INFINITY, f32::NAN] {
            let want = node.cast_ray(&ray, limit);
            let got = bounds.cast(&node, limit);
            assert_eq!(got.to_bits(), want.to_bits(), "{k}: {ray:?}, {limit}");
            hits += usize::from(want != f32::MAX);
        }
    }
    assert!(
        hits > 100_000,
        "must include inside and boundary hits: {hits}"
    );
}

fn bits(hit: Option<RayIntersection>) -> Option<(u32, [u32; 3], FeatureId)> {
    hit.map(|h| {
        (
            h.time_of_impact.to_bits(),
            [
                h.normal.x.to_bits(),
                h.normal.y.to_bits(),
                h.normal.z.to_bits(),
            ],
            h.feature,
        )
    })
}

#[test]
fn cached_mesh_rays_keep_hits_normals_features_and_ties() {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for z in 0..12 {
        for x in 0..12 {
            vertices.push(Point3::new(
                x as f32 - 6.0,
                (x * z % 7) as f32 * 0.25,
                z as f32 - 6.0,
            ));
        }
    }
    for z in 0..11 {
        for x in 0..11 {
            let i = z * 12 + x;
            indices.push([i, i + 12, i + 1]);
            indices.push([i + 1, i + 12, i + 13]);
            // Coincident front/back faces make traversal-order ties visible
            // in the hit feature and normal, not merely its distance.
            if x % 3 == 0 {
                indices.push([i, i + 1, i + 12]);
            }
        }
    }
    let mesh = TriMesh::new(vertices, indices).unwrap();
    let mut seed = 0x93fa_ce01u32;
    let mut hits = 0;
    for k in 0..12_000 {
        let origin = Point3::new(
            next(&mut seed) * 0.25,
            next(&mut seed),
            next(&mut seed) * 0.25,
        );
        let dir = if k % 2 == 0 {
            Vector3::new(0.0, if k % 4 == 0 { -1.0 } else { 1.0 }, -0.0)
        } else {
            Vector3::new(next(&mut seed), next(&mut seed), next(&mut seed))
        };
        let ray = Ray::new(origin, dir);
        let reference = mesh.cast_local_ray_and_get_normal(&ray, 100.0, false);
        hits += usize::from(reference.is_some());
        // Exact-length exclusion is part of find_best's strict tie policy.
        let endpoint = reference.map_or(0.5, |hit| hit.time_of_impact);
        for limit in [0.0, endpoint, 100.0] {
            let want = mesh.cast_local_ray_and_get_normal(&ray, limit, false);
            assert_eq!(
                bits(cast(&mesh, &ray, limit)),
                bits(want),
                "{k}: {ray:?}, {limit}"
            );
        }
    }
    assert!(hits > 2000, "must compare actual mesh hits: {hits}");
}
