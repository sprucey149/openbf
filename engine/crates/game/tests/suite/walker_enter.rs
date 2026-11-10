//! Playtest C7 (2026-11-05): on BF1 Hoth the Imperial player could not get
//! into the AT-AT, which the original lets him pilot. The playtest build
//! tested the use press against a 3 m sphere at the walker's ground origin,
//! under its belly; the port now tests it against the class's collision
//! sphere, high round the hull, as the PS2's entry scoring does
//! (`analysis/re-vehicles.md` §5). Both Hoths: an Imperial at each of the
//! walker's own spawn points, facing it, enters and drives it; BF1's
//! second seat (TURRET1, the head guns) takes a second Imperial; a Rebel is
//! refused.

use core::sim::{buttons, PadState, Sim};
use core::unit::{Controller, UnitHandle};
use core::vehicle::{SeatKind, VehicleHandle};
use glam::Vec3;

const ATAT: &str = "imp_walk_atat";

struct World {
    sim: Sim,
    db: game::classdb::ClassDb,
    names: Vec<(String, usize)>,
    classes: Vec<String>,
}

/// `map`'s conquest world with its pads installed; team 0 is the Empire.
fn world(map: &str) -> Option<World> {
    let root = game::assets::baked_root();
    let dir = root.join(map);
    let mission_file = root
        .parent()
        .unwrap()
        .join(format!("missions/{map}_con.ron"));
    if !dir.join("manifest.ron").exists() || !mission_file.exists() {
        eprintln!("SKIP: {map} needs its baked map and mission");
        return None;
    }
    let mission = game::mission::MissionDef::load(&mission_file).unwrap();
    let factions = mission.factions();
    assert_eq!(factions[0], "imp", "{map}: the Empire is team 0");
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
    let pads = game::vehicle_pads::install(
        &mut sim,
        &scene.manifest,
        &dir,
        &root.join("classes"),
        &factions,
    )
    .expect("install");
    let f = [factions[0].as_str(), factions[1].as_str()];
    let db =
        game::roster::load_classdb(&root.join("classes"), &scene.manifest.source_game, &f).unwrap();
    for t in &mut sim.battle.teams {
        t.reinforcements = 200.0;
    }
    sim.cheats.invincible = true;
    sim.step(&[PadState::default(); 2]);
    Some(World {
        sim,
        db,
        names: Vec::new(),
        classes: pads.classes,
    })
}

impl World {
    fn soldier(&mut self, class: &str, team: u8, slot: u8) -> UnitHandle {
        let roster = game::roster::ClassRoster::build(&self.db, class, 0, None).unwrap();
        let c = roster.register(&mut self.sim, &mut self.names);
        let pos = Vec3::new(0.0, 1000.0, 0.0);
        let controller = Controller::Player { slot };
        self.sim
            .spawn(&c.spec(team, pos, 0.0, controller, 0, 0))
            .unwrap()
    }

    fn atats(&self) -> Vec<VehicleHandle> {
        self.sim
            .alive_vehicles()
            .filter(|(_, v)| self.classes[v.class as usize] == ATAT)
            .map(|(h, _)| h)
            .collect()
    }

    /// The walker's spawn points in the world, each on the ground and level
    /// with the walker (a point over a drop is skipped).
    fn doors(&self, vh: VehicleHandle) -> Vec<Vec3> {
        let v = self.sim.vehicle(vh).unwrap();
        let params = self.sim.walker_params[v.class as usize].as_ref().unwrap();
        params
            .spawn_points
            .iter()
            .filter_map(|&(p, _)| {
                let mut at = v.pos + v.rot * p;
                at.y = self.sim.spawn_ground(at);
                ((at.y - v.pos.y).abs() < 3.0).then_some(at)
            })
            .collect()
    }

    /// Stand `u` at `at`, looking at the walker's collision sphere.
    fn stand(&mut self, u: UnitHandle, vh: VehicleHandle, at: Vec3) {
        let v = self.sim.vehicle(vh).unwrap();
        let centre = v.pos
            + v.rot
                * self.sim.blast_setup[v.class as usize]
                    .as_ref()
                    .unwrap()
                    .sphere_centre;
        let s = &mut self.sim.unit_mut(u).unwrap().soldier;
        s.pos = at;
        s.vel = Vec3::ZERO;
        s.grounded = true;
        let eye = self.sim.unit(u).unwrap().muzzle();
        let d = centre - eye;
        let s = &mut self.sim.unit_mut(u).unwrap().soldier;
        s.yaw = (-d.x).atan2(-d.z);
        s.pitch = (d.y / d.length()).asin();
    }

    fn press(&mut self, slot: usize) {
        let mut pads = [PadState::default(); 2];
        pads[slot].buttons = buttons::ACTION;
        self.sim.step(&pads);
        self.sim.step(&[PadState::default(); 2]);
    }
}

fn check(map: &str, me_class: &str, rebel_class: &str, two_seats: bool) {
    let Some(mut w) = world(map) else {
        return;
    };
    let atats = w.atats();
    assert!(!atats.is_empty(), "{map}: an AT-AT is fielded");
    let me = w.soldier(me_class, 0, 0);
    let rebel = w.soldier(rebel_class, 1, 1);
    // One walker is enough: the rest share the class.
    if let Some(&vh) = atats.first() {
        let class = w.sim.vehicle(vh).unwrap().class as usize;
        let setup = w.sim.seat_setup[class].as_ref().unwrap();
        let boardable = setup.boardable.clone();
        if two_seats {
            // BF1: BODY (pilot, chin cannon) and TURRET1 (head guns), both
            // of PilotType vehicle by BF1's default.
            assert_eq!(boardable, [true, true], "{map}");
        } else {
            assert_eq!(boardable, [true], "{map}");
        }
        let doors = w.doors(vh);
        assert!(
            doors.len() >= 2,
            "{map}: {} level spawn points",
            doors.len()
        );
        for (i, &at) in doors.iter().enumerate() {
            w.stand(me, vh, at);
            assert_eq!(
                w.sim.enter_target(me),
                Some((vh, 0)),
                "{map} door {i} at {at}"
            );
            w.stand(rebel, vh, at + Vec3::new(0.0, 0.0, 0.5));
            assert_eq!(w.sim.enter_target(rebel), None, "{map} door {i}: a Rebel");
            w.stand(rebel, vh, Vec3::new(0.0, 1000.0, 0.0));
        }
        // Into the pilot seat, and the walker walks.
        w.stand(me, vh, doors[0]);
        w.press(0);
        assert_eq!(w.sim.seat_of(me), Some((vh, SeatKind::Pilot)), "{map}");
        if two_seats {
            let gunner = w.soldier(me_class, 0, 1);
            w.stand(gunner, vh, doors[1]);
            assert_eq!(w.sim.enter_target(gunner), Some((vh, 1)), "{map}");
            w.press(1);
            assert!(
                w.sim
                    .seat_of(gunner)
                    .is_some_and(|(h, k)| h == vh && k != SeatKind::Pilot),
                "{map}: the gunner seat"
            );
        }
        let start = w.sim.vehicle(vh).unwrap().pos;
        let drive = PadState {
            ly: i16::MAX,
            ..PadState::default()
        };
        for _ in 0..150 {
            w.sim.step(&[drive, PadState::default()]);
        }
        let v = w.sim.vehicle(vh).unwrap();
        let ran = (v.pos - start).length();
        eprintln!("{map}: the AT-AT walked {ran:.2} m in 5 s");
        assert!(ran > 1.0, "{map}: driven {ran} m");
    }
}

#[test]
fn the_bf1_hoth_atat_is_entered_driven_and_gunned_by_an_imperial() {
    check(
        "bf1_hot1",
        "imp_inf_storm_troopersnow",
        "all_inf_soldiersnow",
        true,
    );
}

#[test]
fn the_bf2_hoth_atat_is_entered_and_driven_by_an_imperial() {
    check(
        "bf2_hot1",
        "imp_inf_rifleman_snow",
        "all_inf_rifleman_snow",
        false,
    );
}
