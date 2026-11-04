//! Swept ground-vehicle hull contacts for walker/hover traffic.
//! OURS, PROVISIONAL: translating SOLDIER-subset prims and conservative world
//! boxes for mesh movers stand in for PS2 vehicle-subset pair dispatch.
//! measure: matched PS2 walker/hover contacts, including rotating and mesh hulls.

use super::{vector, CollisionWorld, Isometry, Layers, Shape};
use crate::vehicle::VehicleHandle;
use glam::Vec3;

impl CollisionWorld {
    /// Earliest translation contact, in stable part order. The normal points
    /// from the other hull toward the mover. Tangential/separating travel is free.
    pub(crate) fn ground_hull_cast(
        &self,
        moving: VehicleHandle,
        other: VehicleHandle,
        step: Vec3,
    ) -> Option<(f32, Vec3)> {
        let mut best: Option<(f32, Vec3)> = None;
        let velocity = vector(step);
        let still = vector(Vec3::ZERO);
        let options = parry3d::query::ShapeCastOptions {
            max_time_of_impact: 1.0,
            target_distance: 0.0,
            stop_at_penetration: true,
            compute_impact_geometry_on_penetration: true,
        };
        for a in self
            .hull(moving)
            .iter()
            .filter(|a| a.layers.contains(Layers::SOLDIER))
        {
            let swept_min = a.aabb.min.min(a.aabb.min + step);
            let swept_max = a.aabb.max.max(a.aabb.max + step);
            for b in self
                .hull(other)
                .iter()
                .filter(|b| b.layers.contains(Layers::SOLDIER))
            {
                if swept_max.cmplt(b.aabb.min).any() || swept_min.cmpgt(b.aabb.max).any() {
                    continue;
                }
                let cast = |shape: &dyn parry3d::shape::Shape, iso: &Isometry| {
                    b.shape.with_parry(&mut |other| {
                        parry3d::query::cast_shapes(
                            iso, &velocity, shape, &b.iso, &still, other, options,
                        )
                        .ok()
                        .flatten()
                    })
                };
                let hit = if matches!(a.shape, Shape::Mesh(_)) {
                    let half = (a.aabb.max - a.aabb.min) * 0.5;
                    let centre = (a.aabb.max + a.aabb.min) * 0.5;
                    cast(
                        &parry3d::shape::Cuboid::new(vector(half)),
                        &Isometry::translation(centre.x, centre.y, centre.z),
                    )
                } else {
                    a.shape.with_parry(&mut |shape| cast(shape, &a.iso))
                };
                let Some(hit) = hit else { continue };
                // Parry returns the normal in the mover's local frame.
                let local = -hit.normal1.into_inner();
                let world = if matches!(a.shape, Shape::Mesh(_)) {
                    local // the conservative mesh box is world-aligned
                } else {
                    a.iso.rotation * local
                };
                let n = Vec3::new(world.x, world.y, world.z);
                if step.dot(n) >= -1.0e-6 || best.is_some_and(|(t, _)| hit.time_of_impact >= t) {
                    continue;
                }
                best = Some((hit.time_of_impact, n));
            }
        }
        best
    }
}
