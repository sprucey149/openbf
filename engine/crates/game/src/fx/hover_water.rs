//! A hover's `WaterEffect`: the wake it keeps on the water while it is over a
//! wet cell (`re-hover.md` §5.1 I12). The hover's update asks the water layer
//! for the surface under its position each frame; over water it keeps one
//! instance of the class's effect riding (x, surface + 0.02, z) with world
//! axes (B2 FUN_0021b470 B2:131944-132003, the play FUN_004522a0 with the
//! position tracked), and off the water it stops it (B2:132005-132027). BF1
//! puts it on the surface itself (B1 FUN_001ea9f8 B1:97566-97618).
use super::{emitter::Frame, Era, Fx};
use crate::walk::WalkSim;
use core::vehicle::{Vehicle, VehicleHandle, VehicleKind};
use glam::Vec3;

const BF2_LIFT: f32 = 0.02;

impl Fx {
    pub(super) fn follow_hover_water(&mut self, walk: &WalkSim) {
        let lift = match self.library.as_ref().map(|l| l.era) {
            Some(Era::Bf1) => 0.0,
            _ => BF2_LIFT,
        };
        let at = |v: &Vehicle| -> Option<Vec3> {
            if !v.alive || v.kind != VehicleKind::Hover {
                return None;
            }
            walk.hover_water.get(v.class as usize)?.as_ref()?;
            let h = walk.sim.water_under(v.pos)?;
            Some(Vec3::new(v.pos.x, h + lift, v.pos.z))
        };
        for effect in &mut self.live {
            let Some(handle) = effect.hover_water else {
                continue;
            };
            match walk.sim.vehicle(handle).and_then(|v| at(v)) {
                Some(pos) => effect.frame = Frame::at(pos),
                None => {
                    effect.stop();
                    effect.hover_water = None;
                }
            }
        }
        for (index, v) in walk.sim.vehicles.iter().enumerate() {
            let Some(pos) = at(v) else {
                continue;
            };
            let handle = VehicleHandle {
                index: index as u32,
                gen: v.gen,
            };
            if self.live.iter().any(|e| e.hover_water == Some(handle)) {
                continue;
            }
            let Some(name) = walk.hover_water.get(v.class as usize).cloned().flatten() else {
                continue;
            };
            if self.spawn(&name, Frame::at(pos)) {
                self.live.last_mut().unwrap().hover_water = Some(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::vehicle::VehicleSpec;
    use core::weapon::HealthType;

    const WAKE: &str = "com_sfx_waterwake_sm";

    fn fx() -> Option<Fx> {
        let root = crate::assets::baked_root();
        let common = root.join("common_bf2");
        if !common.join("effects.ron").exists() {
            eprintln!("SKIP: {} is not baked", common.display());
            return None;
        }
        Some(Fx::with_library(super::super::Library::load(
            &root,
            &common,
            "bf2",
            &["rep"],
        )))
    }

    fn barc(walk: &mut WalkSim, pos: Vec3) -> VehicleHandle {
        walk.hover_water = vec![Some(WAKE.into())];
        walk.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 100.0,
            health_type: HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        })
    }

    /// Save state 8 of 2026-11-13 (KAS2, the BARC on the lake): the hover at
    /// y 1.2655 holds one live `com_sfx_waterwake_sm` (class+0xca4 =
    /// 0x69f2e933) at (x, 1.079, z) = the level 1.0 + the bob 0.059 + 0.02.
    #[test]
    fn a_hover_keeps_its_wake_on_the_water_under_it_and_drops_it_on_land() {
        let Some(mut fx) = fx() else { return };
        let mut walk = WalkSim::new(1, Vec3::ZERO, None);
        walk.sim.water_level = Some(1.0);
        let h = barc(&mut walk, Vec3::new(-45.2, 1.2655, 86.6));
        fx.follow_hover_water(&walk);
        fx.follow_hover_water(&walk);
        let wakes: Vec<_> = fx.live.iter().filter(|e| e.hover_water == Some(h)).collect();
        assert_eq!(wakes.len(), 1);
        let surface = walk.sim.water_under(Vec3::ZERO).unwrap();
        assert_eq!(wakes[0].frame.pos, Vec3::new(-45.2, surface + 0.02, 86.6));
        walk.sim.vehicle_mut(h).unwrap().pos.x += 3.0;
        fx.follow_hover_water(&walk);
        let wake = fx.live.iter().find(|e| e.hover_water == Some(h)).unwrap();
        assert_eq!(wake.frame.pos.x, -42.2);
        walk.sim.water_level = None;
        fx.follow_hover_water(&walk);
        assert!(fx.live.iter().all(|e| e.hover_water.is_none()));
    }
}
