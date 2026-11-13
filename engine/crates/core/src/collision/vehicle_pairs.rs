//! A vehicle's moving primitives against another vehicle's VEHICLE subset.
//! Collision meshes are targets, never movers (re-hover.md §13.1).

use super::{CollisionWorld, HullFilter, HullPart, Layers, Shape};
use crate::vehicle::VehicleHandle;
use glam::{Mat4, Vec3};

impl CollisionWorld {
    /// Contacts in stable body order, with normals pointing out of `other`.
    pub(crate) fn vehicle_part_contacts(
        &self,
        part: &HullPart,
        pose: Mat4,
        other: VehicleHandle,
        out: &mut Vec<(f32, Vec3)>,
    ) {
        let transform = pose * part.local;
        match part.shape {
            Shape::Sphere { radius } => self.sphere_contacts_filtered(
                transform.w_axis.truncate(),
                radius,
                Layers::VEHICLE,
                HullFilter::Only(other),
                out,
            ),
            _ => self.shape_contacts_filtered(
                &part.shape,
                transform,
                Layers::VEHICLE,
                HullFilter::Only(other),
                out,
            ),
        }
    }
}
