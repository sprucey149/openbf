//! The named walker collision bodies used by tow ordnance. The vehicle's
//! ordinary hull uses a different subset and remains rigid in the sim.

use super::Vehicle;
use crate::collision::{Body, Shape};
use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TowClass {
    pub initial_length: f32,
    pub length: f32,
}

/// A posed primitive in the walker's tow-only subset (BF2 walker class
/// property setter, `FUN_00263f68`, property hash 0x2436d722).
pub struct TowPrimitive {
    pub frame: Mat4,
    pub shape: Shape,
    /// Local wrap adjustment authored alongside the primitive name.
    pub offset: Vec3,
    /// Index in the authored four-leg ring, or None for a blocking body.
    pub leg: Option<u8>,
}

impl TowPrimitive {
    /// First surface of this posed primitive along a world-space ray.
    /// Transforming the direction without normalizing preserves the ray parameter.
    pub fn ray_hit(&self, origin: Vec3, direction: Vec3, max_t: f32) -> Option<f32> {
        let inverse = self.frame.inverse();
        self.shape.ray_distance(
            inverse.transform_point3(origin),
            inverse.transform_vector3(direction),
            max_t,
        )
    }
}

/// Immutable class data and deterministic animation samples supply the
/// current tow primitives. This query reads only the given walker state.
pub trait TowFrames: Send + Sync {
    fn primitives(&self, walker: &Vehicle) -> Vec<TowPrimitive>;
    /// Posed ordinary building collision after a tow kill.
    fn fallen_bodies(&self, _walker: &Vehicle) -> Vec<Body> {
        Vec::new()
    }
}
