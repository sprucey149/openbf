use super::{emitter::Frame, Fx};
use core::collision::{HullFilter, Layers};
use core::sim::Sim;
use core::vehicle::{flyer::FlightPhase, VehicleHandle, VehicleKind};
use glam::Vec3;

impl Fx {
    pub(super) fn follow_flyer_exhaust(&mut self, walk: &crate::walk::WalkSim) {
        let active = |v: &core::vehicle::Vehicle| {
            v.alive
                && v.kind == VehicleKind::Flyer
                && matches!(
                    v.flyer.phase,
                    FlightPhase::Takeoff | FlightPhase::Flying | FlightPhase::Landing
                )
        };
        let frame = |v: &core::vehicle::Vehicle, local: glam::Mat4| {
            let m = glam::Mat4::from_rotation_translation(v.rot, v.pos) * local;
            Frame {
                pos: m.w_axis.truncate(),
                rot: glam::Mat3::from_mat4(m),
                vel: v.vel,
            }
        };
        for effect in &mut self.live {
            let Some((handle, slot)) = effect.flyer_exhaust else {
                continue;
            };
            let owner = walk
                .sim
                .vehicle(handle)
                .filter(|v| active(v))
                .and_then(|v| {
                    walk.flyer_exhaust
                        .get(v.class as usize)?
                        .get(slot)
                        .map(|e| (v, e))
                });
            if let Some((vehicle, exhaust)) = owner {
                effect.frame = frame(vehicle, exhaust.local);
            } else {
                effect.stop();
                effect.flyer_exhaust = None;
            }
        }
        for (index, vehicle) in walk
            .sim
            .vehicles
            .iter()
            .enumerate()
            .filter(|(_, v)| active(v))
        {
            let handle = VehicleHandle {
                index: index as u32,
                gen: vehicle.gen,
            };
            let Some(exhausts) = walk.flyer_exhaust.get(vehicle.class as usize) else {
                continue;
            };
            for (slot, exhaust) in exhausts.iter().enumerate() {
                if self
                    .live
                    .iter()
                    .any(|e| e.flyer_exhaust == Some((handle, slot)))
                {
                    continue;
                }
                if self.spawn(&exhaust.effect, frame(vehicle, exhaust.local)) {
                    self.live.last_mut().unwrap().flyer_exhaust = Some((handle, slot));
                }
            }
        }
    }

    pub(super) fn flyer_spray(&mut self, sim: &Sim) {
        self.flyer_phases.resize(sim.vehicles.len(), None);
        let mut hits = Vec::new();
        for (index, vehicle) in sim.vehicles.iter().enumerate() {
            let state = (vehicle.gen, vehicle.flyer.phase);
            let previous = self.flyer_phases[index].replace(state);
            let transition = previous.map(|(gen, phase)| (gen, phase, vehicle.flyer.phase));
            let spray = matches!(transition,
                Some((gen, FlightPhase::Landed, FlightPhase::Takeoff))
                | Some((gen, FlightPhase::Landing, FlightPhase::Landed)) if gen == vehicle.gen
            );
            if !vehicle.alive || vehicle.kind != VehicleKind::Flyer || !spray {
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
    fn flyer_exhaust_follows_the_craft_and_stops_on_landing() {
        let root = crate::assets::baked_root();
        let mut fx = Fx::load(&root, &root.join("bf2_spa6"), "bf2", &["cis"]);
        let mut walk = crate::walk::WalkSim::new(42, Vec3::ZERO, None);
        walk.flyer_exhaust = vec![vec![crate::vehicle_pads::FlyerExhaust {
            effect: "com_sfx_exhaust_jedistarfighter".into(),
            local: Mat4::from_translation(Vec3::new(0.0, 1.0, -3.0)),
        }]];
        let handle = walk.sim.spawn_vehicle(&VehicleSpec {
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
        fx.follow_flyer_exhaust(&walk);
        assert!(fx.live.is_empty());
        walk.sim.vehicle_mut(handle).unwrap().flyer.phase = FlightPhase::Flying;
        fx.follow_flyer_exhaust(&walk);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].flyer_exhaust, Some((handle, 0)));
        assert_eq!(fx.live[0].frame.pos, Vec3::new(0.0, 3.0, -3.0));
        walk.sim.vehicle_mut(handle).unwrap().pos.x = 5.0;
        fx.follow_flyer_exhaust(&walk);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].frame.pos, Vec3::new(5.0, 3.0, -3.0));
        walk.sim.vehicle_mut(handle).unwrap().flyer.phase = FlightPhase::Landed;
        fx.follow_flyer_exhaust(&walk);
        assert_eq!(fx.live[0].flyer_exhaust, None);
        assert!(fx.live[0].stopped);
    }

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
        sim.vehicle_mut(handle).unwrap().flyer.phase = FlightPhase::Landing;
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 1);
        sim.vehicle_mut(handle).unwrap().flyer.phase = FlightPhase::Landed;
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 2);
        assert_eq!(fx.live[1].name(), Some("FlyerSpray"));
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 2);
        assert_eq!(fx.live[0].name(), Some("FlyerSpray"));
        assert!(fx.live[0].frame.pos.y.abs() < 0.001);
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 2);
    }
}
