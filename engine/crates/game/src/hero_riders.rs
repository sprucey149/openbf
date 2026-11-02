//! Collision frames for exposed seats, from the same baked rig as the figure.

use crate::vehicle_class::RiderHang;
use crate::viewer::{walker_pose::WalkerRig, walker_rigs};
use core::sim::{HeroRiderFrames, Sim};
use core::vehicle::{Vehicle, VehicleKind};
use glam::Mat4;
use std::{path::Path, sync::Arc};

struct Frames {
    riders: Vec<Vec<RiderHang>>,
    rigs: Vec<Option<Arc<WalkerRig>>>,
    root_scales: Vec<f32>,
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

pub(crate) fn install(
    sim: &mut Sim,
    manifest: &manifest::Manifest,
    map_dir: &Path,
    build: &crate::vehicle_pads::PadBuild,
) {
    sim.hero_rider_frames = Some(Arc::new(Frames {
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
    }));
}
