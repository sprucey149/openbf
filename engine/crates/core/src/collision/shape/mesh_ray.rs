//! Mesh rays retain parry's best-first traversal and triangle intersections.
//! Only the ray's reciprocals are shared between the BVH's bounding boxes.

use parry3d::partitioning::BvhNode;
use parry3d::query::{Ray, RayCast, RayIntersection};
use parry3d::shape::{FeatureId, TriMesh};

pub(super) fn cast(mesh: &TriMesh, ray: &Ray, max_t: f32) -> Option<RayIntersection> {
    let bounds = RayBounds::new(ray);
    let tested = std::cell::Cell::new(0u32);
    let best = mesh.bvh().find_best(
        max_t,
        |node, limit| bounds.cast(node, limit),
        |triangle, limit| {
            tested.set(tested.get() + 1);
            mesh.triangle(triangle)
                .cast_local_ray_and_get_normal(ray, limit, false)
        },
    );
    super::super::cost::mesh_cast(tested.get());
    best.map(|(triangle, mut hit)| {
        // Keep TriMesh's backface feature convention as well as its
        // normal, even though the world currently only reads the latter.
        hit.feature = if hit.feature == FeatureId::Face(1) {
            FeatureId::Face(triangle + mesh.indices().len() as u32)
        } else {
            FeatureId::Face(triangle)
        };
        hit
    })
}

struct RayBounds {
    origin: [f32; 3],
    inverse: [f32; 3],
    parallel: [bool; 3],
}

impl RayBounds {
    fn new(ray: &Ray) -> Self {
        let parallel = [ray.dir.x == 0.0, ray.dir.y == 0.0, ray.dir.z == 0.0];
        Self {
            origin: [ray.origin.x, ray.origin.y, ray.origin.z],
            inverse: std::array::from_fn(|i| if parallel[i] { 0.0 } else { 1.0 / ray.dir[i] }),
            parallel,
        }
    }

    /// Parry 0.22.1's AABB ray cost with `solid = true`: keep axis order,
    /// subtract/multiply order, strict comparisons and f32 min/max semantics.
    /// An exact zero direction tests the slab instead of multiplying by an
    /// infinite reciprocal (including at the slab's boundary).
    #[inline]
    fn cast(&self, node: &BvhNode, max_t: f32) -> f32 {
        let aabb = node.aabb();
        let mut near = 0.0f32;
        let mut far = max_t;
        for i in 0..3 {
            if self.parallel[i] {
                if self.origin[i] < aabb.mins[i] || self.origin[i] > aabb.maxs[i] {
                    return f32::MAX;
                }
            } else {
                let mut a = (aabb.mins[i] - self.origin[i]) * self.inverse[i];
                let mut b = (aabb.maxs[i] - self.origin[i]) * self.inverse[i];
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                near = near.max(a);
                far = far.min(b);
                if near > far {
                    return f32::MAX;
                }
            }
        }
        near
    }
}

#[cfg(test)]
mod tests;
