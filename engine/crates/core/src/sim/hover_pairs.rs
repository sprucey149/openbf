//! Solid hover/treaded pairs: BF2 FUN_0021fd98 K4, K7-K9
//! (analysis/re-hover.md §13.15, B2:134589-134590, 134651-134827).
//! PROVISIONAL: sequential part queries and depth correction share the static
//! contact approximation.
//! measure: matched PS2 moving-pair trajectories and contact-list constraints.

use crate::collision::{CollisionWorld, HullPart, Layers};
use crate::vehicle::{
    hover::{HoverParams, Impact, HULL_FRICTION, HULL_RESTITUTION},
    Vehicle, VehicleHandle, VehicleKind,
};
use glam::{Mat4, Vec3};
use std::sync::Arc;

pub(super) fn contacts(
    vehicles: &mut [Vehicle],
    index: usize,
    params: &HoverParams,
    class_params: &[Option<HoverParams>],
    parts: &[Option<Arc<[HullPart]>>],
    world: &CollisionWorld,
    impacts: &mut Vec<Impact>,
) {
    let Some(parts) = parts
        .get(vehicles[index].class as usize)
        .and_then(Option::as_ref)
    else {
        return;
    };
    let mut found = Vec::new();
    for j in 0..vehicles.len() {
        // Hover physics class 8 includes hover, treaded and commandhover.
        // Other physics classes take the base response, still a separate gap.
        if j == index || !vehicles[j].alive || vehicles[j].kind != VehicleKind::Hover {
            continue;
        }
        let other = VehicleHandle {
            index: j as u32,
            gen: vehicles[j].gen,
        };
        let other_velocity = vehicles[j].vel;
        let other_mass = class_params
            .get(vehicles[j].class as usize)
            .and_then(Option::as_ref)
            .map_or(1.0, |p| p.mass);
        for part in parts.iter().filter(|p| {
            p.layers.contains(Layers::VEHICLE)
                && !matches!(p.shape, crate::collision::Shape::Mesh(_))
        }) {
            let v = &mut vehicles[index];
            let at = v.pos;
            found.clear();
            world.vehicle_part_contacts(
                part,
                Mat4::from_rotation_translation(v.rot, at),
                other,
                &mut found,
            );
            for &(depth, normal) in &found {
                let depth = depth - (v.pos - at).dot(normal);
                if depth <= 0.0 {
                    continue;
                }
                impacts.push(respond(
                    v,
                    params,
                    other_velocity,
                    other_mass,
                    depth,
                    normal,
                ));
            }
        }
    }
}

/// K4 bypasses the spring hook, even when the participating part is a spring
/// sphere. K7 uses relative velocity and the speed-weighted mass ratio;
/// K9 separates by the entire depth.
fn respond(
    v: &mut Vehicle,
    p: &HoverParams,
    other: Vec3,
    mass: f32,
    depth: f32,
    n: Vec3,
) -> Impact {
    let up = v.rot * Vec3::Y;
    let forward = v.rot * Vec3::Z;
    let relative = v.vel - other;
    let speed = relative.dot(n);
    let impact = Impact {
        normal_speed: speed,
        n_up: n.dot(up),
        n_fwd: n.dot(forward),
    };
    if speed < 0.0 && v.alive {
        let mu = if p.set_altitude == 0.0 {
            let a = n.dot(up).max(0.0);
            (1.0 - a * a * a * a) * HULL_FRICTION
        } else {
            HULL_FRICTION
        };
        let other_mass = mass * (0.5 * (-v.vel.dot(n)).max(0.0) + 1.0);
        let own_mass = p.mass * (0.5 * other.dot(n).max(0.0) + 1.0);
        let impulse = -(1.0 + HULL_RESTITUTION) * speed * other_mass / (own_mass + other_mass);
        let mut tangent = relative - n * speed;
        let length = tangent.length();
        if length > 0.0 {
            tangent -= tangent * (mu * impulse / length).min(1.0);
        }
        v.vel = tangent + n * (impulse + speed) + other;
    }
    if p.set_altitude <= 0.0 && n.y > 0.5 {
        v.hover.g = 1.0;
    }
    v.pos += n * depth;
    impact
}

#[cfg(test)]
mod tests;
