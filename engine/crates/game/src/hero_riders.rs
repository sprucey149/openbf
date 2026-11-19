//! Collision frames for exposed seats and tow targets from the baked walker rig.

use crate::vehicle_class::RiderHang;
use crate::viewer::{walker_pose::WalkerRig, walker_rigs};
use core::collision::{Body, Layers, Shape};
use core::sim::{HeroRiderFrames, Sim};
use core::vehicle::tow::{TowFrames, TowPrimitive};
use core::vehicle::{Vehicle, VehicleKind};
use glam::{Mat4, Vec3};
use std::{path::Path, sync::Arc};

struct Frames {
    riders: Vec<Vec<RiderHang>>,
    rigs: Vec<Option<Arc<WalkerRig>>>,
    root_scales: Vec<f32>,
    tow_parts: Vec<Vec<TowPart>>,
}

impl HeroRiderFrames for Frames {
    fn frame(&self, vehicle: &Vehicle, seat: u8) -> Option<Mat4> {
        let class = vehicle.class as usize;
        let hang = self.riders.get(class)?.get(usize::from(seat))?;
        let walker = vehicle.kind == VehicleKind::Walker;
        let rig = self
            .rigs
            .get(class)
            .and_then(Option::as_deref)
            .filter(|_| walker);
        let rotation = if walker && rig.is_none() {
            core::vehicle::walker::legs_rot(vehicle)
        } else {
            vehicle.rot
        };
        let local = match hang {
            RiderHang::Bind(bind) => *bind,
            RiderHang::Node(name, bind) => rig
                .and_then(|rig| {
                    rig.node_for_sim(
                        &vehicle.walker,
                        self.root_scales.get(class).copied().unwrap_or(1.0),
                        name,
                    )
                })
                .unwrap_or(*bind),
        };
        Some(Mat4::from_rotation_translation(rotation, vehicle.pos) * local)
    }
}

pub(crate) struct TowPart {
    bone: String,
    bind: Mat4,
    shape: Shape,
    offset: Vec3,
    leg: Option<u8>,
    layers: Layers,
}

/// Resolve the class's authored list without reordering the leg groups.
/// BF2 FUN_00263f68 stores up to 12 body indexes and local offsets, and
/// FUN_002669f8 partitions the list into four equal groups.
pub(crate) fn tow_parts(
    rows: &[&str],
    geometry: &crate::assets::CollisionGeom,
    subsets: &[(String, String)],
) -> Option<Vec<TowPart>> {
    if !matches!(rows.len(), 4 | 8 | 12) {
        return None;
    }
    let layers = crate::assets::body_layers(geometry, subsets);
    let mesh = usize::from(geometry.mesh.is_some());
    let part = |index: usize, prim: &manifest::ManifestPrim, leg, offset| {
        Some(TowPart {
            bone: prim.parent.clone(),
            bind: crate::viewer::instance_matrix(prim.rotation, prim.translation),
            shape: crate::assets::prim_shape(prim)?,
            offset,
            leg,
            layers: layers[index + mesh],
        })
    };
    let mut ordered = Vec::with_capacity(rows.len());
    let mut indices = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let mut fields = row.split_ascii_whitespace();
        let name = fields.next()?;
        let (index, prim) = geometry
            .prims
            .iter()
            .enumerate()
            .find(|(_, p)| p.name.eq_ignore_ascii_case(name))?;
        let mut coord = || fields.next()?.parse::<f32>().ok().filter(|n| n.is_finite());
        let leg = Some((i / (rows.len() / 4)) as u8);
        ordered.push(part(
            index,
            prim,
            leg,
            Vec3::new(coord()?, coord()?, coord()?),
        )?);
        indices.push(index);
    }
    // These bodies can obstruct a tow shot or block movement during the
    // death clip. Keep the class's ordinary per-primitive masks intact.
    for (i, prim) in geometry.prims.iter().enumerate() {
        if !indices.contains(&i) && layers[i + mesh] != Layers::NONE {
            if let Some(blocker) = part(i, prim, None, Vec3::ZERO) {
                ordered.push(blocker);
            }
        }
    }
    if let Some(mesh) = geometry.mesh.as_ref().filter(|_| layers[0] != Layers::NONE) {
        if let Some(shape) = core::collision::trimesh(&mesh.positions, &mesh.indices) {
            ordered.push(TowPart {
                bone: String::new(),
                bind: Mat4::IDENTITY,
                shape: Shape::Mesh(shape),
                offset: Vec3::ZERO,
                leg: None,
                layers: layers[0],
            });
        }
    }
    Some(ordered)
}

impl Frames {
    fn posed_parts(&self, walker: &Vehicle) -> Option<Vec<(&TowPart, Mat4)>> {
        let class = walker.class as usize;
        let parts = self.tow_parts.get(class).filter(|p| !p.is_empty())?;
        let rig = self.rigs.get(class).and_then(Option::as_deref)?;
        let scale = self.root_scales.get(class).copied().unwrap_or(1.0);
        let palette = rig.palette_for_sim(&walker.walker, scale);
        let world = Mat4::from_rotation_translation(walker.rot, walker.pos);
        parts
            .iter()
            .map(|part| {
                let pose = if part.bone.is_empty() {
                    Mat4::IDENTITY
                } else {
                    palette[rig.skeleton.index_of(&part.bone)?]
                };
                Some((part, world * pose * part.bind))
            })
            .collect()
    }
}

impl TowFrames for Frames {
    fn primitives(&self, walker: &Vehicle) -> Vec<TowPrimitive> {
        self.posed_parts(walker)
            .into_iter()
            .flatten()
            .filter(|(part, _)| part.layers.contains(Layers::ORDNANCE))
            .map(|(part, frame)| TowPrimitive {
                frame,
                shape: part.shape.clone(),
                offset: part.offset,
                leg: part.leg,
            })
            .collect()
    }

    fn fallen_bodies(&self, walker: &Vehicle) -> Vec<Body> {
        self.posed_parts(walker)
            .into_iter()
            .flatten()
            .filter(|(part, _)| part.layers != Layers::NONE)
            .map(|(part, frame)| Body::with_layers(frame, part.shape.clone(), part.layers))
            .collect()
    }
}

pub(crate) fn install(
    sim: &mut Sim,
    manifest: &manifest::Manifest,
    map_dir: &Path,
    build: &crate::vehicle_pads::PadBuild,
    tow_parts: Vec<Vec<TowPart>>,
) {
    let has_tow_targets = tow_parts.iter().any(|parts| !parts.is_empty());
    let frames = Arc::new(Frames {
        riders: build.riders.clone(),
        rigs: walker_rigs(
            manifest,
            map_dir.parent().unwrap_or(map_dir),
            &build.geometry,
        ),
        root_scales: sim
            .walker_params
            .iter()
            .map(|p| p.as_ref().map_or(1.0, |p| p.root_scale))
            .collect(),
        tow_parts,
    });
    sim.hero_rider_frames = Some(frames.clone());
    sim.tow_frames = has_tow_targets.then_some(frames as Arc<dyn TowFrames>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::vehicle::{SeatKind, VehicleSpec};

    #[test]
    fn hoth_tow_targets_follow_the_authored_legs() {
        let root = crate::assets::baked_root();
        for game in ["bf1", "bf2"] {
            let map = root.join(format!("{game}_hot1"));
            let path = map.join("manifest.ron");
            if !path.is_file() {
                eprintln!("SKIP: {game} Hoth bake required");
                continue;
            }
            let mut manifest: manifest::Manifest =
                ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            if let Some(i) = crate::assets::mode_layer_index(&manifest, "con") {
                let layer = manifest.layers.remove(i);
                manifest.regions.extend(layer.regions);
                manifest.instances.extend(layer.instances);
            }
            let mut sim = Sim::empty(73);
            let build = crate::vehicle_pads::install(
                &mut sim,
                &manifest,
                &map,
                &root.join("classes"),
                &["imp".to_owned(), "all".to_owned()],
            )
            .unwrap();
            let speeder = build
                .classes
                .iter()
                .position(|class| class.eq_ignore_ascii_case("all_fly_snowspeeder"))
                .unwrap_or_else(|| panic!("{game} Hoth is missing its snowspeeder class"));
            let guns = &sim.seat_setup[speeder].as_ref().unwrap().guns;
            assert!(
                guns.iter()
                    .flatten()
                    .flat_map(|gun| &gun.weapons)
                    .any(|weapon| {
                        let ord = &sim.weapon_classes[weapon.class].ordnance;
                        ord.velocity == 250.0
                            && ord.tow
                                == Some(core::vehicle::tow::TowClass {
                                    initial_length: 60.0,
                                    length: 60.0,
                                })
                    }),
                "{game} rear seat is missing the authored harpoon"
            );
            let class = build
                .classes
                .iter()
                .position(|class| class.eq_ignore_ascii_case("imp_walk_atat"))
                .unwrap_or_else(|| panic!("{game} Hoth is missing its AT-AT class"));
            let clip = &sim.walker_params[class].as_ref().unwrap().clips
                [usize::from(core::vehicle::walker::ANIM_DEATH)];
            assert!((clip.as_ref().unwrap().duration - 260.0 / 30.0).abs() < 0.001);
            let h = sim.spawn_vehicle(&VehicleSpec {
                kind: VehicleKind::Walker,
                class: class as u32,
                home_team: Some(1),
                neutral_when_empty: false,
                spawn_pad: None,
                pos: Vec3::ZERO,
                yaw: 0.0,
                max_health: 1000.0,
                health_type: core::weapon::HealthType::Vehicle,
                seats: vec![SeatKind::Pilot],
            });
            let mut vehicle = sim.vehicle(h).unwrap().clone();
            let frames = sim.tow_frames.as_ref().expect("AT-AT tow parts");
            let rest = frames.primitives(&vehicle);
            assert_eq!(
                rest.iter().filter_map(|p| p.leg).collect::<Vec<_>>(),
                [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3],
                "{game}",
            );
            assert_eq!(rest[0].offset, Vec3::new(0.36, 0.0, 1.06));
            assert_eq!(rest[11].offset, Vec3::new(0.5, 0.0, -1.1));
            vehicle.walker.anim = core::vehicle::walker::ANIM_WALK as u8;
            vehicle.walker.clip_time = 0.43;
            vehicle.walker.layer_blend[0] = 0.29;
            let current = frames.primitives(&vehicle);
            let repeated = frames.primitives(&vehicle);
            assert_eq!(current.iter().filter_map(|p| p.leg).count(), 12);
            assert!(current.iter().any(|p| p.leg.is_none()));
            vehicle.walker.anim = core::vehicle::walker::ANIM_DEATH;
            vehicle.walker.clip_time = 0.0;
            let start = frames.fallen_bodies(&vehicle);
            vehicle.walker.clip_time = 6.0;
            let end = frames.fallen_bodies(&vehicle);
            assert!(start.iter().any(|b| b.layers.contains(Layers::SOLDIER)));
            assert!(start.iter().any(|b| b.layers.contains(Layers::ORDNANCE)));
            assert_eq!(start.len(), end.len());
            assert!(start
                .iter()
                .zip(&end)
                .any(|(a, b)| a.aabb.max.distance(b.aabb.max) > 0.05));
            assert!(rest
                .iter()
                .zip(&current)
                .any(|(old, new)| { old.frame.w_axis.distance(new.frame.w_axis) > 0.05 }));
            for (now, again) in current.iter().zip(&repeated) {
                assert_eq!(
                    now.frame.to_cols_array().map(f32::to_bits),
                    again.frame.to_cols_array().map(f32::to_bits)
                );
                let center = now.frame.w_axis.truncate();
                let t = now
                    .ray_hit(center + Vec3::X * 10.0, Vec3::NEG_X, 10.0)
                    .expect("a ray toward the primitive center hits its surface");
                assert!(t > 0.0 && t < 10.0, "{game} leg {:?}: {t}", now.leg);
            }
        }
    }

    fn pilot_pad(
        v: &core::vehicle::Vehicle,
        class: &core::vehicle::flyer::FlyerParams,
        tether: bool,
        opening: &mut bool,
    ) -> core::sim::PadState {
        use core::ai::{flyer_drive::stick_to, hover_drive::Body};
        use core::sim::{buttons, PadState};
        use core::vehicle::flyer::{untilted, FlightPhase};

        if v.flyer.phase == FlightPhase::Landed {
            return PadState {
                buttons: buttons::JUMP,
                ..PadState::default()
            };
        }
        if v.flyer.phase != FlightPhase::Flying {
            return PadState::default();
        }
        let flat = v.pos.with_y(0.0);
        let radius = flat.length();
        let radial = flat.normalize();
        let ahead = radial * 0.8253356 - Vec3::Y.cross(radial) * 0.5646425;
        *opening = !tether && radius < if *opening { 42.0 } else { 25.2 };
        let at = if tether {
            ahead * 25.2
        } else if *opening {
            v.pos + ahead * 25.2
        } else {
            ahead * 18.6
        };
        let body = Body {
            pos: v.pos,
            left: v.rot * Vec3::X,
            fwd: v.rot * Vec3::Z,
            vel: v.vel,
            aim_origin: v.pos,
            aim_dir: untilted(v) * Vec3::Z,
        };
        let (yaw, pitch) = stick_to(class, &body, at.with_y(9.6));
        PadState {
            ly: if radius < 50.4 || tether {
                -i16::MAX
            } else {
                0
            },
            rx: PadState::quantize(yaw),
            ry: PadState::quantize(pitch),
            ..PadState::default()
        }
    }

    fn gunner_pad(
        sim: &Sim,
        rear: core::unit::UnitHandle,
        target: core::vehicle::VehicleHandle,
    ) -> core::sim::PadState {
        use core::sim::{buttons, PadState};
        use core::vehicle::gun::{angles, wrap_angle};

        if !sim.tow_cables.is_empty() {
            return PadState::default();
        }
        let sight = sim.seat_look(rear).unwrap();
        let turn = sight.gun.turn.unwrap();
        let parts = sim
            .tow_frames
            .as_ref()
            .unwrap()
            .primitives(sim.vehicle(target).unwrap());
        let aim = parts
            .iter()
            .filter(|p| p.leg.is_some())
            .filter_map(|p| {
                let at = p.frame.w_axis.truncate();
                let (yaw, pitch) = angles(sight.rot.inverse() * (at - sight.eye));
                let yaw = wrap_angle(yaw, turn.yaw);
                let distance = sight.muzzle.distance_squared(at);
                (distance <= 60.0 * 60.0
                    && (turn.yaw[0]..=turn.yaw[1]).contains(&yaw)
                    && (turn.pitch[0]..=turn.pitch[1]).contains(&pitch))
                .then_some((distance, yaw, pitch))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let (_, yaw, pitch) = match aim {
            Some(aim) => aim,
            None => return PadState::default(),
        };
        let fire = parts
            .iter()
            .filter_map(|p| {
                p.ray_hit(sight.muzzle, sight.barrel, 60.0)
                    .map(|distance| (distance, p.leg))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .is_some_and(|(_, leg)| leg.is_some());
        PadState {
            rx: PadState::quantize((sight.look.0 - yaw) * 3.0 / turn.yaw_speed),
            ry: PadState::quantize((pitch - sight.look.1) * 3.0 / turn.pitch_speed),
            buttons: if fire { buttons::FIRE } else { 0 },
            ..PadState::default()
        }
    }

    #[test]
    fn hoth_bot_tow_crew_from_parked_speeder() {
        use core::vehicle::flyer::FlightPhase;
        use core::weapon::HealthType;

        let root = crate::assets::baked_root();
        for game in ["bf1", "bf2"] {
            let map = root.join(format!("{game}_hot1"));
            let path = map.join("manifest.ron");
            if !path.is_file() {
                eprintln!("SKIP: {game} Hoth bake required");
                continue;
            }
            let mut manifest: manifest::Manifest =
                ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            if let Some(i) = crate::assets::mode_layer_index(&manifest, "con") {
                let layer = manifest.layers.remove(i);
                manifest.regions.extend(layer.regions);
                manifest.instances.extend(layer.instances);
            }
            for (pilot_human, rear_human) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let mut sim = Sim::empty(103);
                if game == "bf2" {
                    sim.ground_rules = core::soldier::GroundRules::Bf2;
                }
                sim.ground_flyer_map = true;
                sim.heightfield = Some(core::heightfield::Heightfield {
                    size: 2,
                    unit: 8000.0,
                    offset_x: 4000.0,
                    offset_z: 4000.0,
                    heights: vec![0.0; 4],
                });
                let build = crate::vehicle_pads::install(
                    &mut sim,
                    &manifest,
                    &map,
                    &root.join("classes"),
                    &["imp".to_owned(), "all".to_owned()],
                )
                .unwrap();
                sim.flyer_bots = true;
                sim.vehicle_pads.clear();
                sim.flyer_paths = vec![core::vehicle::flyer::FlyerPath {
                    nodes: vec![Vec3::new(0.0, 30.0, -120.0), Vec3::new(100.0, 30.0, 180.0)],
                    ..Default::default()
                }];
                let class = |name: &str| {
                    build
                        .classes
                        .iter()
                        .position(|class| class.eq_ignore_ascii_case(name))
                        .unwrap() as u32
                };
                let target = sim.spawn_vehicle(&VehicleSpec {
                    kind: VehicleKind::Walker,
                    class: class("imp_walk_atat"),
                    home_team: Some(0),
                    neutral_when_empty: false,
                    spawn_pad: None,
                    pos: Vec3::ZERO,
                    yaw: 0.0,
                    max_health: 60_000.0,
                    health_type: HealthType::Vehicle,
                    seats: vec![SeatKind::Pilot],
                });
                let start = Vec3::new(31.0, 2.0, -53.0);
                let flyer_class = class("all_fly_snowspeeder");
                let flyer_params = sim.flyer_params[flyer_class as usize].clone().unwrap();
                let craft = sim.spawn_vehicle(&VehicleSpec {
                    kind: VehicleKind::Flyer,
                    class: flyer_class,
                    home_team: Some(1),
                    neutral_when_empty: false,
                    spawn_pad: None,
                    pos: start,
                    yaw: 0.0,
                    max_health: 3_000.0,
                    health_type: HealthType::Vehicle,
                    seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
                });
                let spec = |human, slot| {
                    if human {
                        core::unit::UnitSpec::player_in_slot(slot, 1, start)
                    } else {
                        core::unit::UnitSpec {
                            controller: core::unit::Controller::Ai,
                            ..core::unit::UnitSpec::trooper(1, start)
                        }
                    }
                };
                let pilot = sim.spawn(&spec(pilot_human, 0)).unwrap();
                let rear = sim.spawn(&spec(rear_human, u8::from(pilot_human))).unwrap();
                assert!(sim.seat_unit(pilot, craft, 0));
                assert!(sim.seat_unit(rear, craft, 1));

                let label = format!("{game}, human pilot {pilot_human}, human gunner {rear_human}");
                let (mut flying, mut first_shot, mut fatal) = (None, None, None);
                let mut opening_shot = false;
                for _ in 0..1800 {
                    let v = sim
                        .vehicle(craft)
                        .unwrap_or_else(|| panic!("{label} crashed at {}", sim.tick));
                    let mut pads = [core::sim::PadState::default(); 2];
                    if pilot_human {
                        pads[0] = pilot_pad(
                            v,
                            &flyer_params,
                            !sim.tow_cables.is_empty(),
                            &mut opening_shot,
                        );
                    }
                    if rear_human && v.flyer.phase == FlightPhase::Flying {
                        pads[usize::from(pilot_human)] = gunner_pad(&sim, rear, target);
                    }
                    sim.step(&pads);
                    if sim
                        .vehicle(craft)
                        .is_some_and(|v| v.flyer.phase == FlightPhase::Flying)
                    {
                        flying.get_or_insert(sim.tick);
                    }
                    if !sim.tow_cables.is_empty() {
                        first_shot.get_or_insert(sim.tick);
                    }
                    if sim.vehicle(target).unwrap().walker.dead {
                        fatal = Some(sim.tick);
                        break;
                    }
                }
                eprintln!("{label}, flying {flying:?}, first shot {first_shot:?}, trip {fatal:?}");
                assert!(
                    flying.is_some() && first_shot.is_some() && fatal.is_some(),
                    "{label}"
                );
            }
        }
    }
}
