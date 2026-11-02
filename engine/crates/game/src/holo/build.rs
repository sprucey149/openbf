//! BF2 build hologram: disc `com_holo_turret`, shown per screen only to a
//! living foot soldier carrying a repair tool (re-build-zones §4).
use super::*;

pub(super) const BUILD: HoloClass = HoloClass {
    height: 0.75,
    size: 1.0,
    beam_intensity: 0.3,
    visible: 35.0,
    ..CONTROLZONE
};

pub(super) fn unbuilt(t: &core::sim::BuildingTurret) -> bool {
    t.down.is_some() && t.fall == core::sim::TurretFall::Pad
}

pub(super) fn initial(t: &core::sim::BuildingTurret) -> Holo {
    let mut h = Holo::steady(None);
    h.on = false;
    h.fade = 0.0;
    h.intensity = 0.0;
    h.switch(unbuilt(t), &BUILD);
    h
}

pub(super) fn draws(
    states: &[Holo],
    art: &HoloArt,
    walk: &WalkSim,
    camera: &Camera,
    out: &mut Vec<ParticleDraw>,
) {
    let Some(u) = walk
        .sim
        .unit(walk.player)
        .filter(|u| u.alive && u.mount.is_none())
    else {
        return;
    };
    if core::ai::fix::cutter(&walk.sim, u).is_none() {
        return;
    }
    for (h, t) in states.iter().zip(&walk.sim.building_turrets) {
        // The PS2 hides build holograms once the building stands. A wreck
        // with a destroyed model has no build hologram either.
        if !unbuilt(t) {
            continue;
        }
        draw_holo(
            &BUILD,
            h,
            t.spec.pos,
            art.icons.get("build"),
            art.bf1,
            [0, 225, 98],
            camera,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::unit::{SlotSpec, UnitSpec};
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use core::weapon::{HealthType, Repair, WeaponClass};

    #[test]
    fn build_hologram_is_green_rotates_and_is_visible_only_to_a_living_cutter_carrier() {
        let mut walk = WalkSim::new(1, Vec3::new(0.0, 0.0, 3.0), None);
        let spec = VehicleSpec {
            kind: VehicleKind::Turret,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 800.0,
            health_type: HealthType::Building,
            seats: vec![SeatKind::Gunner(0)],
        };
        walk.sim.add_building_turret(
            spec,
            glam::Quat::IDENTITY,
            None,
            42,
            (core::sim::TurretFall::Pad, vec![]),
        );
        walk.sim.start_building_unbuilt(42);
        let tool = walk.sim.add_weapon_class(WeaponClass {
            repair: Some(Repair { health: [50.0; 8] }),
            ..Default::default()
        });
        let engineer = walk
            .sim
            .spawn(&UnitSpec {
                slots: vec![SlotSpec {
                    slot: 2,
                    channel: 1,
                    weapon_class: tool,
                    clips: 0,
                }]
                .into(),
                ..UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, 3.0))
            })
            .unwrap();
        let plain = walk.player;
        let mut art = HoloArt::default();
        art.icons.insert(
            "build",
            vec![IconPart {
                texture: None,
                tris: vec![
                    (Vec3::X, [0.0; 2]),
                    (Vec3::Y, [0.0; 2]),
                    (Vec3::Z, [0.0; 2]),
                ],
                scroll: [0.0; 2],
            }],
        );
        let camera = Camera {
            position: Vec3::new(0.0, 1.0, 5.0),
            ..Default::default()
        };
        let mut holos = Holos::default();
        holos.advance(&walk);
        walk.sim.tick = 90;
        holos.advance(&walk);
        assert!(
            holos.draws(&art, &walk, &camera).is_empty(),
            "rifleman sees no build icon"
        );
        walk.player = engineer;
        let draws = holos.draws(&art, &walk, &camera);
        assert_eq!(draws.len(), 2, "beam plus real-icon draw path");
        assert_eq!(&draws[0].color[..3], &[0, 225, 98]);
        let ParticleShape::Beam(beam) = &draws[0].shape else {
            panic!("beam")
        };
        assert_eq!((beam.length, beam.visible), (0.75, 35.0));
        let before = icon_matrix(&holos.builds[0], &BUILD, Vec3::ZERO);
        walk.sim.tick += 15;
        holos.advance(&walk);
        let after = icon_matrix(&holos.builds[0], &BUILD, Vec3::ZERO);
        assert_ne!(before, after, "0.2 turns/s animation");
        walk.player = plain;
        assert!(
            holos.draws(&art, &walk, &camera).is_empty(),
            "independent second screen"
        );
        walk.player = engineer;
        walk.sim.units[engineer.index as usize].alive = false;
        assert!(
            holos.draws(&art, &walk, &camera).is_empty(),
            "dead engineer"
        );
        walk.sim.units[engineer.index as usize].alive = true;
        walk.sim.repair_building(0, 800.0);
        assert!(
            holos.draws(&art, &walk, &camera).is_empty(),
            "built turret hides icon immediately"
        );
        let handle = walk.sim.building_turrets[0].handle;
        walk.sim.destroy_vehicle(handle, None);
        walk.sim.units[engineer.index as usize].mount = Some(core::vehicle::Mount {
            vehicle: handle,
            seat: 0,
        });
        assert!(
            holos.draws(&art, &walk, &camera).is_empty(),
            "mounted viewer is not a foot cutter carrier"
        );
    }
}
