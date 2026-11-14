use super::{
    point, vector, Aabb, Capsule, CollisionWorld, HullFilter, Layers, Shape, WALKABLE_MIN_NY,
};
use crate::fmath::FloatExt;
use glam::Vec3;

impl CollisionWorld {
    /// The first ceiling-like face (contact normal `n.y < -WALKABLE_MIN_NY`,
    /// against the motion) a rising capsule sweeps into from `from` to
    /// `to`: `(time of impact in 0..=1, n)`.
    ///
    /// Issue #168 (cost, no answer changed): a body or triangle lying wholly
    /// below the capsule's bottom sphere centre at `from` is not cast. The
    /// capsule is upright and only rises, so the nearest point of its axis
    /// to such a shape is always that bottom centre, which is above it: any
    /// contact normal points up out of it (`n.y > 0`), never a ceiling's.
    /// On BF2 Mygeeto at tick 600 this cut the sweep's instructions by 7%;
    /// the rest of its cost there (40% of a predicted step) is one bot
    /// standing inside a dense mesh, whose faces are all above that centre.
    pub fn ceiling_sweep(&self, from: &Capsule, to: &Capsule) -> Option<(f32, Vec3)> {
        let delta = to.base - from.base;
        if delta.y <= 0.0 {
            return None;
        }
        let a = from.aabb();
        let b = to.aabb();
        let swept = Aabb {
            min: a.min.min(b.min),
            max: a.max.max(b.max),
        };
        let (shape, iso) = from.to_parry();
        // The bottom sphere centre's height at the start (`Capsule::to_parry`).
        let bottom = from.base.y + from.radius.fmin(from.height * 0.5);
        let opts = parry3d::query::ShapeCastOptions {
            max_time_of_impact: 1.0,
            target_distance: 0.001,
            stop_at_penetration: true,
            compute_impact_geometry_on_penetration: true,
        };
        let mut best: Option<(f32, Vec3)> = None;
        self.visit_xz(
            swept.min.x,
            swept.max.x,
            swept.min.z,
            swept.max.z,
            HullFilter::All,
            |body| {
                if !body.layers.contains(Layers::SOLDIER)
                    || !body.aabb.overlaps(&swept)
                    || body.aabb.max.y < bottom
                {
                    return;
                }
                let test = |best: &mut Option<(f32, Vec3)>, other: &dyn parry3d::shape::Shape| {
                    if let Ok(Some(hit)) = parry3d::query::cast_shapes(
                        &iso,
                        &vector(delta),
                        &shape,
                        &body.iso,
                        &vector(Vec3::ZERO),
                        other,
                        opts,
                    ) {
                        let n = Vec3::new(-hit.normal1.x, -hit.normal1.y, -hit.normal1.z);
                        if n.y < -WALKABLE_MIN_NY
                            && delta.dot(n) < 0.0
                            && best.is_none_or(|(t, _)| hit.time_of_impact < t)
                        {
                            *best = Some((hit.time_of_impact, n));
                        }
                    }
                };
                if let Shape::Mesh(mesh) = &body.shape {
                    let local = Aabb::transformed(swept, body.to_local);
                    let query =
                        parry3d::bounding_volume::Aabb::new(point(local.min), point(local.max));
                    let y = |p: parry3d::na::Point3<f32>| {
                        body.to_world.transform_point3(Vec3::new(p.x, p.y, p.z)).y
                    };
                    for i in mesh.bvh().intersect_aabb(&query) {
                        let tri = mesh.triangle(i);
                        if y(tri.a) < bottom && y(tri.b) < bottom && y(tri.c) < bottom {
                            continue;
                        }
                        test(&mut best, &tri);
                    }
                } else {
                    body.shape.with_parry(&mut |s| test(&mut best, s));
                }
            },
        );
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sim::{buttons, PadState, Sim},
        unit::UnitSpec,
    };
    use glam::Mat4;

    #[test]
    fn production_jumps_stay_under_primitive_mesh_and_sloping_ceilings() {
        for mesh in [false, true] {
            for slope in [0.0f32, 0.25, -0.25] {
                let transform = Mat4::from_rotation_translation(
                    glam::Quat::from_rotation_z(slope),
                    Vec3::new(0.0, 3.4, 0.0),
                );
                let body = if mesh {
                    let tris = crate::collision::tests::floor_quad(0.0, 20.0);
                    crate::collision::tests::mesh_body_at(&tris, transform)
                } else {
                    super::super::Body::new(
                        transform,
                        Shape::Box {
                            half: Vec3::new(20.0, 0.05, 20.0),
                        },
                    )
                };
                let mut sim = Sim::empty(198);
                sim.heightfield = Some(crate::heightfield::Heightfield {
                    size: 2,
                    unit: 4000.0,
                    offset_x: 2000.0,
                    offset_z: 2000.0,
                    heights: vec![0.0; 4],
                });
                sim.collision = CollisionWorld::new(vec![body], vec![]);
                let unit = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
                let u = sim.unit_mut(unit).unwrap();
                u.consts.saber = true;
                u.consts.jump_vy = 11.224972;
                u.consts.jet_energy_drain = 0.0;
                u.consts.jet_fuel_min_border = 0.0;
                for tick in 0..300 {
                    let pad = PadState {
                        buttons: if tick % 6 == 0 { buttons::JUMP } else { 0 },
                        ..Default::default()
                    };
                    sim.step(&[pad]);
                    let u = sim.unit(unit).unwrap();
                    let top = u.soldier.pos.y + u.capsule().height;
                    assert!(
                        top <= 3.45,
                        "mesh={mesh} slope={slope} tick={tick} top={top}"
                    );
                    assert!(u.soldier.pos.y >= -0.01);
                }
            }
        }
    }

    #[test]
    fn thin_off_center_underside_is_swept_with_the_whole_capsule() {
        let body = super::super::Body::new(
            Mat4::from_translation(Vec3::new(0.4, 2.4, 0.0)),
            Shape::Box {
                half: Vec3::new(0.1, 0.01, 3.0),
            },
        );
        let world = CollisionWorld::new(vec![body], vec![]);
        let from = Capsule {
            base: Vec3::ZERO,
            height: 1.9,
            radius: 0.5,
        };
        let to = Capsule {
            base: Vec3::Y * 2.0,
            ..from
        };
        assert!(world.ceiling_sweep(&from, &to).is_some());
        assert!(world.ceiling_sweep(&to, &from).is_none());
    }
}
