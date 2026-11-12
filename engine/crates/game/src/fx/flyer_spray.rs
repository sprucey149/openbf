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
        let nodes: Vec<_> = walk
            .sim
            .vehicles
            .iter()
            .map(|v| {
                if !active(v)
                    || walk
                        .flyer_exhaust
                        .get(v.class as usize)
                        .is_none_or(Vec::is_empty)
                {
                    return None;
                }
                walk.flyer_rigs
                    .get(v.class as usize)?
                    .as_ref()?
                    .flyer_nodes(v.flyer.lift)
            })
            .collect();
        let frame = |vehicle_index: usize,
                     v: &core::vehicle::Vehicle,
                     exhaust: &crate::vehicle_pads::FlyerExhaust| {
            let local = exhaust
                .node
                .as_deref()
                .and_then(|name| {
                    let rig = walk.flyer_rigs.get(v.class as usize)?.as_ref()?;
                    let bone = rig.skeleton.index_of(name)?;
                    nodes.get(vehicle_index)?.as_ref()?.get(bone).copied()
                })
                .map_or(exhaust.local, |node| {
                    node * glam::Mat4::from_translation(exhaust.offset)
                });
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
                effect.frame = frame(handle.index as usize, vehicle, exhaust);
                if let Some(p) = walk
                    .sim
                    .flyer_params
                    .get(vehicle.class as usize)
                    .and_then(Option::as_ref)
                {
                    let speed = vehicle.vel.dot(vehicle.rot * Vec3::Z);
                    effect.set_size(p.exhaust_scale(exhaust.scale, speed));
                }
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
                if self.spawn(&exhaust.effect, frame(index, vehicle, exhaust)) {
                    let live = self.live.last_mut().unwrap();
                    live.flyer_exhaust = Some((handle, slot));
                    live.set_size(exhaust.scale[0]);
                }
            }
        }
    }

    pub(super) fn flyer_spray(&mut self, sim: &Sim) {
        self.flyer_phases.resize(sim.vehicles.len(), None);
        self.flyer_spray_depth.resize(sim.vehicles.len(), None);
        let mut hits = Vec::new();
        for (index, vehicle) in sim.vehicles.iter().enumerate() {
            let state = (vehicle.gen, vehicle.flyer.phase);
            let previous = self.flyer_phases[index].replace(state);
            let transition = previous.map(|(gen, phase)| (gen, phase, vehicle.flyer.phase));
            let takeoff = matches!(transition, Some((gen, FlightPhase::Landed, FlightPhase::Takeoff)) if gen == vehicle.gen);
            let landing = vehicle.flyer.phase == FlightPhase::Landing
                || matches!(transition, Some((gen, FlightPhase::Landing, FlightPhase::Landed)) if gen == vehicle.gen);
            if !vehicle.alive
                || vehicle.kind != VehicleKind::Flyer
                || (vehicle.flyer.phase != FlightPhase::Takeoff && !landing)
            {
                self.flyer_spray_depth[index] = None;
                continue;
            }
            let ground = |origin, layers| {
                let body = sim
                    .collision
                    .ray_with(
                        origin,
                        Vec3::NEG_Y,
                        1024.0,
                        layers,
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
                distance.map(|d| origin - Vec3::Y * d)
            };
            let sphere = sim
                .blast_setup
                .get(vehicle.class as usize)
                .and_then(Option::as_ref);
            let params = sim
                .flyer_params
                .get(vehicle.class as usize)
                .and_then(Option::as_ref);
            let bf2_landing = landing && params.is_some_and(|p| p.bf2);
            let ray_origin = if bf2_landing {
                vehicle.pos + vehicle.rot * sphere.map_or(Vec3::ZERO, |s| s.sphere_centre)
            } else {
                vehicle.pos + Vec3::Y
            };
            if let Some(hit) = ground(
                ray_origin,
                if bf2_landing {
                    Layers::VEHICLE
                } else {
                    Layers::ORDNANCE
                },
            ) {
                let clearance = vehicle.pos.y - hit.y;
                let threshold = params.map_or(0.0, |p| p.landed_height)
                    + sphere.map_or(0.0, |s| s.sphere_radius);
                let previous_depth =
                    self.flyer_spray_depth[index].replace((vehicle.gen, clearance));
                let was_approaching = matches!(previous,
                    Some((gen, FlightPhase::Takeoff | FlightPhase::Landing)) if gen == vehicle.gen
                );
                let crossed = if params.is_some_and(|p| p.bf2) {
                    landing
                        && clearance <= threshold
                        && (!was_approaching
                            || previous_depth
                                .is_none_or(|(gen, depth)| gen != vehicle.gen || depth > threshold))
                } else {
                    matches!(transition, Some((gen, FlightPhase::Landing, FlightPhase::Landed)) if gen == vehicle.gen)
                };
                if takeoff || crossed {
                    if let Some(effect_hit) = ground(vehicle.pos + Vec3::Y, Layers::ORDNANCE) {
                        hits.push(effect_hit);
                    }
                }
            } else {
                self.flyer_spray_depth[index] = None;
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
    fn flyer_exhaust_follows_the_animated_attachment() {
        use crate::skin::{BoneXform, Clip, Skeleton};
        use crate::viewer::walker_pose::WalkerRig;
        let root = crate::assets::baked_root();
        let mut fx = Fx::load(&root, &root.join("bf2_spa6"), "bf2", &["cis"]);
        let mut walk = crate::walk::WalkSim::new(42, Vec3::ZERO, None);
        let skeleton = Skeleton::new(vec!["engine".into()], vec![None], vec![Mat4::IDENTITY]);
        let clip = Clip {
            name: "takeoff".into(),
            fps: 30.0,
            frames: 2,
            tracks: vec![Some(vec![
                BoneXform::IDENTITY,
                BoneXform {
                    rotation: glam::Quat::IDENTITY,
                    translation: Vec3::X * 2.0,
                },
            ])],
        };
        walk.flyer_rigs = vec![Some(std::sync::Arc::new(WalkerRig::new_flyer(
            skeleton,
            vec![clip],
        )))];
        walk.flyer_exhaust = vec![vec![crate::vehicle_pads::FlyerExhaust {
            effect: "com_sfx_exhaust_jedistarfighter".into(),
            local: Mat4::IDENTITY,
            node: Some("engine".into()),
            offset: Vec3::Y,
            scale: [1.0, 1.0, 0.0],
        }]];
        let handle = walk.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![],
        });
        walk.sim.vehicle_mut(handle).unwrap().flyer.phase = FlightPhase::Takeoff;
        fx.follow_flyer_exhaust(&walk);
        assert_eq!(fx.live[0].frame.pos, Vec3::Y);
        walk.sim.vehicle_mut(handle).unwrap().flyer.lift = 0.5;
        fx.follow_flyer_exhaust(&walk);
        assert!(fx.live[0]
            .frame
            .pos
            .abs_diff_eq(Vec3::new(1.0, 1.0, 0.0), 1e-5));
    }

    #[test]
    fn flyer_exhaust_follows_the_craft_and_stops_on_landing() {
        let root = crate::assets::baked_root();
        let mut fx = Fx::load(&root, &root.join("bf2_spa6"), "bf2", &["cis"]);
        let mut walk = crate::walk::WalkSim::new(42, Vec3::ZERO, None);
        walk.flyer_exhaust = vec![vec![crate::vehicle_pads::FlyerExhaust {
            effect: "com_sfx_exhaust_jedistarfighter".into(),
            local: Mat4::from_translation(Vec3::new(0.0, 1.0, -3.0)),
            node: None,
            offset: Vec3::ZERO,
            scale: [1.0, 1.0, 0.0],
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
        sim.flyer_params = vec![Some(core::vehicle::flyer::FlyerParams {
            landed_height: 1.0,
            ..core::vehicle::flyer::FlyerParams::bf2()
        })];
        sim.blast_setup = vec![Some(core::vehicle::BlastSetup {
            explosion: None,
            sphere_centre: Vec3::ZERO,
            sphere_radius: 2.0,
        })];
        let vehicle = sim.vehicle_mut(handle).unwrap();
        vehicle.flyer.phase = FlightPhase::Landing;
        vehicle.pos.y = 4.0;
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 1);
        sim.vehicle_mut(handle).unwrap().pos.y = 2.9;
        fx.flyer_spray(&sim);
        assert_eq!(fx.live.len(), 2);
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
