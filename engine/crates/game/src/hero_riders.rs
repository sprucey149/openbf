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
}
