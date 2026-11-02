//! BF1 Bespin Platforms: every flyer leaves its deck on the pilot's input. The TIE bomber's `LandedHeight` is -0.0115, so its origin
//! stood 1 cm under the deck: its ground ray found no deck, its contact sweep
//! took the deck for a ceiling, and the craft slid about inside the platform
//! until its pilot gave up and fell out of the sky. No bots run here.

use core::sim::{buttons, PadState, Sim};
use core::unit::{UnitHandle, UnitSpec};
use core::vehicle::flyer::FlightPhase;
use core::vehicle::{VehicleHandle, VehicleKind};
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

fn flyers(sim: &Sim) -> Vec<(VehicleHandle, u32)> {
    sim.alive_vehicles()
        .filter(|(_, v)| v.kind == VehicleKind::Flyer)
        .map(|(h, v)| (h, v.class))
        .collect()
}

fn press(sim: &mut Sim, b: u32) {
    sim.step(&[PadState {
        buttons: b,
        ..Default::default()
    }]);
}

#[test]
fn every_bespin_flyer_lifts_off_its_deck() {
    let Some((mut sim, me, classes)) = world() else {
        return;
    };
    let all = flyers(&sim);
    let kinds: Vec<&str> = all
        .iter()
        .map(|(_, c)| classes[*c as usize].as_str())
        .collect();
    assert!(kinds.contains(&"imp_fly_tiebomber"), "{kinds:?}");
    for (vh, class) in all {
        let name = &classes[class as usize];
        let parked = sim.vehicle(vh).unwrap().pos;
        sim.unit_mut(me).unwrap().soldier.pos = parked;
        assert!(sim.enter_vehicle(me, vh, 0), "{name} takes a pilot");
        press(&mut sim, 0);
        press(&mut sim, buttons::JUMP);
        for _ in 0..120 {
            press(&mut sim, 0);
        }
        let v = sim.vehicle(vh).unwrap();
        assert_eq!(v.flyer.phase, FlightPhase::Flying, "{name} at {:?}", v.pos);
        let risen = v.pos.y - parked.y;
        assert!(risen > 1.5, "{name} rose only {risen} m from {parked:?}");
        press(&mut sim, 0);
        sim.exit_vehicle(me);
        sim.remove_vehicle(vh);
    }
}

/// The kill floor sits 24 m under Bespin's decks (15 m under its lowest post),
/// and the mission lets a pilot fly to y -200: he dived under it and died with
/// his craft.
#[test]
fn a_pilot_flies_under_bespins_decks_and_lives() {
    let Some((mut sim, me, _)) = world() else {
        return;
    };
    let (vh, _) = flyers(&sim)[0];
    let parked = sim.vehicle(vh).unwrap().pos;
    sim.kill_floor = Some(parked.y - 24.0);
    sim.unit_mut(me).unwrap().soldier.pos = parked;
    assert!(sim.enter_vehicle(me, vh, 0));
    press(&mut sim, 0);
    press(&mut sim, buttons::JUMP);
    for _ in 0..120 {
        press(&mut sim, 0);
    }
    let v = sim.vehicle_mut(vh).unwrap();
    assert_eq!(v.flyer.phase, FlightPhase::Flying);
    v.pos.y = parked.y - 40.0;
    for _ in 0..30 {
        press(&mut sim, 0);
    }
    assert!(sim.vehicle(vh).is_some(), "the craft flies on");
    assert!(sim.unit(me).is_some_and(|u| u.alive), "and so does he");
}
