use super::shape::{Aabb, Body, HullPart, Shape};
use super::world::{CollisionWorld, HullFilter};
use crate::vehicle::VehicleHandle;
use glam::{Mat4, Vec3};
use parry3d::query::DefaultQueryDispatcher;
use parry3d::shape::CompositeShapeRef;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimContact {
    pub normal: Vec3,
    pub depth: f32,
    pub owner: Option<VehicleHandle>,
}

pub fn bounding_radius(parts: &[HullPart]) -> f32 {
    let mut bounds = Aabb::empty();
    for part in parts {
        let placed = Aabb::transformed(part.shape.local_aabb(), part.local);
        bounds.add(placed.min);
        bounds.add(placed.max);
    }
    if bounds.is_empty() {
        return 0.0;
    }
    let center = (bounds.min + bounds.max) * 0.5;
    parts
        .iter()
        .map(|part| {
            let (at, reach) = part_sphere(part);
            at.distance(center) + reach
        })
        .fold(0.0, crate::fmath::max)
}

fn part_sphere(part: &HullPart) -> (Vec3, f32) {
    match &part.shape {
        Shape::Sphere { radius } => (part.local.w_axis.truncate(), *radius),
        Shape::Box { half } => (part.local.w_axis.truncate(), half.length()),
        Shape::Cylinder {
            radius,
            half_height,
        } => (
            part.local.w_axis.truncate(),
            (radius * radius + half_height * half_height).sqrt(),
        ),
        Shape::Mesh(_) => {
            let a = Aabb::transformed(part.shape.local_aabb(), part.local);
            ((a.min + a.max) * 0.5, (a.max - a.min).length() * 0.5)
        }
    }
}

impl HullPart {
    pub fn lowest_point(&self, pose: Mat4) -> Option<Vec3> {
        let world = pose * self.local;
        let center = world.w_axis.truncate();
        let [x, y, z] = [
            world.x_axis.truncate(),
            world.y_axis.truncate(),
            world.z_axis.truncate(),
        ];
        match &self.shape {
            Shape::Mesh(_) => None,
            Shape::Sphere { radius } => Some(center - Vec3::Y * *radius),
            Shape::Cylinder {
                radius,
                half_height,
            } => {
                let side = Vec3::Y - y * y.y;
                let side = if side.length_squared() > 0.01 {
                    side.normalize()
                } else {
                    Vec3::ZERO
                };
                let end = if y.y > 0.0 { -y } else { y };
                Some(center + end * *half_height - side * *radius)
            }
            Shape::Box { half } => {
                let toward = |axis: Vec3, half: f32| {
                    if axis.y.abs() <= 0.0001 {
                        Vec3::ZERO
                    } else if axis.y > 0.0 {
                        -axis * half
                    } else {
                        axis * half
                    }
                };
                Some(center + toward(x, half.x) + toward(y, half.y) + toward(z, half.z))
            }
        }
    }
}

impl CollisionWorld {
    pub fn deepest_contact(
        &self,
        part: &HullPart,
        pose: Mat4,
        hulls: HullFilter,
        admits: impl Fn(&Body) -> bool,
    ) -> Option<PrimContact> {
        let placed = Body::placed(
            pose * part.local,
            part.shape.clone(),
            part.layers,
            part.closed,
        );
        let mut best: Option<PrimContact> = None;
        let area = placed.aabb;
        self.visit_xz(
            area.min.x,
            area.max.x,
            area.min.z,
            area.max.z,
            hulls,
            |body| {
                if !body.aabb.overlaps(&area) || !admits(body) {
                    return;
                }
                let Some((normal, dist)) = meet(&placed, body) else {
                    return;
                };
                let depth = -dist;
                if depth > 0.0 && best.is_none_or(|b| depth > b.depth) {
                    best = Some(PrimContact {
                        normal,
                        depth,
                        owner: body.owner,
                    });
                }
            },
        );
        best
    }
}

fn meet(placed: &Body, body: &Body) -> Option<(Vec3, f32)> {
    if let Shape::Mesh(mesh) = &body.shape {
        let relative = body.iso.inv_mul(&placed.iso);
        let (_, c) = placed.shape.with_parry(&mut |own| {
            CompositeShapeRef(mesh.as_ref()).contact_with_shape(
                &DefaultQueryDispatcher,
                &relative,
                own,
                0.0,
            )
        })?;
        let n = body.iso.rotation * c.normal1.into_inner();
        return Some((Vec3::new(n.x, n.y, n.z), c.dist));
    }
    let c = placed.shape.with_parry(&mut |own| {
        body.shape.with_parry(&mut |other| {
            parry3d::query::contact(&placed.iso, own, &body.iso, other, 0.0)
                .ok()
                .flatten()
        })
    })?;
    let n = c.normal1;
    Some((-Vec3::new(n.x, n.y, n.z), c.dist))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{trimesh, Layers};

    fn floor_world() -> CollisionWorld {
        let positions = vec![
            [-50.0, 0.0, -50.0],
            [50.0, 0.0, -50.0],
            [50.0, 0.0, 50.0],
            [-50.0, 0.0, 50.0],
        ];
        let indices = vec![0, 2, 1, 0, 3, 2];
        let mesh = trimesh(&positions, &indices).unwrap();
        let mut body = Body::new(Mat4::IDENTITY, Shape::Mesh(mesh));
        body.layers = Layers::FLYER;
        CollisionWorld::new(vec![body], Vec::new())
    }

    fn ball(radius: f32) -> HullPart {
        HullPart::new(Mat4::IDENTITY, Shape::Sphere { radius }, Layers::VEHICLE)
    }

    #[test]
    fn a_ball_in_a_floor_is_pushed_up_by_its_overlap() {
        let world = floor_world();
        let pose = Mat4::from_translation(Vec3::new(1.0, 3.0, 2.0));
        let hit = world
            .deepest_contact(&ball(4.0), pose, HullFilter::except(None), |b| {
                b.layers.contains(Layers::FLYER)
            })
            .unwrap();
        assert!(hit.normal.abs_diff_eq(Vec3::Y, 1e-4), "{:?}", hit.normal);
        assert!((hit.depth - 1.0).abs() < 1e-4, "{}", hit.depth);
    }

    #[test]
    fn a_ball_clear_of_a_floor_or_a_body_not_admitted_meets_nothing() {
        let world = floor_world();
        let high = Mat4::from_translation(Vec3::Y * 5.0);
        let low = Mat4::from_translation(Vec3::Y * 3.0);
        let flyer = |b: &Body| b.layers.contains(Layers::FLYER);
        assert!(world
            .deepest_contact(&ball(4.0), high, HullFilter::except(None), flyer)
            .is_none());
        assert!(world
            .deepest_contact(&ball(4.0), low, HullFilter::except(None), |b| {
                b.layers.contains(Layers::SOLDIER)
            })
            .is_none());
    }

    #[test]
    fn a_box_and_a_cylinder_meet_a_floor_too() {
        let world = floor_world();
        let pose = Mat4::from_translation(Vec3::Y * 0.5);
        for shape in [
            Shape::Box { half: Vec3::ONE },
            Shape::Cylinder {
                radius: 1.0,
                half_height: 1.0,
            },
        ] {
            let part = HullPart::new(Mat4::IDENTITY, shape, Layers::VEHICLE);
            let hit = world
                .deepest_contact(&part, pose, HullFilter::except(None), |_| true)
                .unwrap();
            assert!(hit.normal.y > 0.99, "{:?}", hit.normal);
            assert!((hit.depth - 0.5).abs() < 0.01, "{}", hit.depth);
        }
    }

    #[test]
    fn the_lowest_point_follows_the_part() {
        let pose = Mat4::from_translation(Vec3::Y * 10.0);
        assert_eq!(ball(2.0).lowest_point(pose), Some(Vec3::Y * 8.0));
        let boxy = HullPart::new(
            Mat4::IDENTITY,
            Shape::Box {
                half: Vec3::new(1.0, 2.0, 3.0),
            },
            Layers::VEHICLE,
        );
        assert_eq!(boxy.lowest_point(pose), Some(Vec3::Y * 8.0));
        let cylinder = HullPart::new(
            Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Shape::Cylinder {
                radius: 1.0,
                half_height: 4.0,
            },
            Layers::VEHICLE,
        );
        let low = cylinder.lowest_point(pose).unwrap();
        assert!((low.y - 9.0).abs() < 1e-3, "{low}");
    }

    #[test]
    fn the_bound_holds_every_part() {
        let parts = [
            ball(2.0),
            HullPart::new(
                Mat4::from_translation(Vec3::X * 2.0),
                Shape::Sphere { radius: 2.0 },
                Layers::VEHICLE,
            ),
        ];
        assert!((bounding_radius(&parts) - 3.0).abs() < 1e-5);
        assert_eq!(bounding_radius(&[]), 0.0);
    }
}
