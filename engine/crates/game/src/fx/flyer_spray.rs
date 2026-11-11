use super::{emitter::Frame, Fx};
use core::collision::{HullFilter, Layers};
use core::sim::Sim;
use core::vehicle::{flyer::FlightPhase, VehicleHandle, VehicleKind};
use glam::Vec3;

impl Fx {
    pub(super) fn flyer_spray(&mut self, sim: &Sim) {
        self.flyer_phases.resize(sim.vehicles.len(), None);
        let mut hits = Vec::new();
        for (index, vehicle) in sim.vehicles.iter().enumerate() {
            let state = (vehicle.gen, vehicle.flyer.phase);
            let previous = self.flyer_phases[index].replace(state);
            if !vehicle.alive
                || vehicle.kind != VehicleKind::Flyer
                || previous != Some((vehicle.gen, FlightPhase::Landed))
                || vehicle.flyer.phase != FlightPhase::Takeoff
            {
                continue;
            }
            let origin = vehicle.pos + Vec3::Y;
            let body = sim
                .collision
                .ray_with(
                    origin,
                    Vec3::NEG_Y,
                    1024.0,
                    Layers::ORDNANCE,
                    HullFilter::except(Some(VehicleHandle {
                        index: index as u32,
                        gen: vehicle.gen,
                    })),
                )
                .map(|hit| hit.t);
            let terrain = sim.terrain_ray(origin, Vec3::NEG_Y, 1024.0, 0.5);
            let distance = match (body, terrain) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            if let Some(distance) = distance {
                hits.push(origin - Vec3::Y * distance);
            }
        }
        for hit in hits {
            self.spawn("FlyerSpray", Frame::at(hit));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::collision::{Body, CollisionWorld, Shape};
    use core::vehicle::VehicleSpec;
    use glam::Mat4;

    #[test]
    fn takeoff_emits_one_authored_plume_on_the_surface() {
        let root = crate::assets::baked_root();
        let map = root.join("bf2_spa6");
        assert!(map.join("effects.ron").is_file());
        let mut fx = Fx::load(&root, &map, "bf2", &[]);
        let mut sim = Sim::empty(42);
        sim.collision = CollisionWorld::new(
            vec![Body::new(
                Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
                Shape::Box {
                    half: Vec3::new(20.0, 1.0, 20.0),
                },
            )],
            vec![],
        );
        let handle = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 2.0, 0.0),
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![],
        });
        fx.flyer_spray(&sim);
        assert!(fx.live.is_empty());
        sim.vehicle_mut(handle).unwrap().flyer.phase = FlightPhase::Takeoff;
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].name(), Some("FlyerSpray"));
        assert!(fx.live[0].frame.pos.y.abs() < 0.001);
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 1);
    }
}
