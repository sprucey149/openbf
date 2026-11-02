//! BF1 Bespin Platforms: a pad's next craft comes up on a soldier standing on
//! the pad. Before this he was inside its hull, and every way he walked was
//! blocked for good. The census's roam traps at the Y-wing pads, 0.2 m in 30 s
//! of stick. No bots run here.

use core::sim::{PadState, Sim};
use core::unit::{UnitHandle, UnitSpec};
use glam::Vec3;

const MAP: &str = "bf1_bes1";

fn world() -> Option<(Sim, UnitHandle, Vec<String>)> {
    let root = game::assets::baked_root();
    let dir = root.join(MAP);
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: {MAP} is not baked");
        return None;
    }
    let mut scene = game::assets::load_scene(&dir).expect("load scene");
    scene.apply_mode_layer("con");
    scene.merge_common(
        game::assets::load_common(&root.join(format!("common_{}", scene.manifest.source_game)))
            .expect("common assets"),
    );
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let f = ["all", "imp"].map(str::to_owned);
    let pads =
        game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
            .expect("install");
    sim.cheats.invincible = true;
    let me = sim
        .spawn(&UnitSpec::player(1, Vec3::new(0.0, 500.0, 0.0)))
        .expect("the player");
    sim.step(&[PadState::default()]);
    Some((sim, me, pads.classes))
}

#[test]
fn a_soldier_on_a_pad_is_not_shut_in_by_its_next_craft() {
    let Some((mut sim, me, classes)) = world() else {
        return;
    };
    let (pad, craft) = (0..sim.vehicle_pads.len())
        .find_map(|pi| {
            sim.alive_vehicles()
                .find(|(_, v)| {
                    v.spawn_pad == Some(pi as u32)
                        && classes
                            .get(v.class as usize)
                            .is_some_and(|c| c == "all_fly_ywing")
                })
                .map(|(h, _)| (pi, h))
        })
        .expect("a Y-wing pad");
    let at = sim.vehicle(craft).expect("the craft").pos;
    sim.destroy_vehicle(craft, None);
    let i = me.index as usize;
    let floor = sim.ground_under(at, false);
    sim.units[i].soldier.pos = Vec3::new(at.x, floor, at.z);
    sim.units[i].soldier.vel = Vec3::ZERO;
    let wait = sim.vehicle_pads[pad].def.spawn_time as usize * 30 + 90;
    for _ in 0..wait {
        sim.step(&[PadState::default()]);
    }
    // Whatever the pad did meanwhile, he walks away from where he stood.
    let from = sim.units[i].soldier.pos;
    for _ in 0..(30 * 5) {
        sim.step(&[PadState {
            ly: i16::MAX,
            ..Default::default()
        }]);
    }
    let walked = (sim.units[i].soldier.pos - from).with_y(0.0).length();
    assert!(walked > 5.0, "shut in on the pad: walked {walked:.2} m");
    // And the pad's craft comes once he has gone.
    for _ in 0..(30 * 3) {
        sim.step(&[PadState::default()]);
    }
    assert!(
        sim.alive_vehicles()
            .any(|(_, v)| v.spawn_pad == Some(pad as u32)),
        "the pad made its craft after he left"
    );
}
