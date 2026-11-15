//! Shared controller flight layout and BF1 Geonosis natural spawn witnesses.
use core::sim::{PadState, LOOK_SCALE};
use core::vehicle::flyer::{FlightPhase, FlyerParams};
use core::vehicle::{SeatKind, Vehicle, VehicleKind, VehicleSpec};
use game::classdb::ClassDb;
use game::vehicle_class::FlyerClass;
use glam::Vec3;

use super::walker_battle as battle;

fn flight(params: &FlyerParams, controller: PadState) -> Vehicle {
    let mut walk = game::walk::WalkSim::new(42, Vec3::ZERO, None);
    walk.sim.ground_rules =
        core::soldier::GroundRules::for_game(if params.bf2 { "bf2" } else { "bf1" });
    walk.sim.flyer_params = vec![Some(params.clone())];
    let h = walk.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::Y * 50.0,
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    assert!(walk.sim.seat_unit(walk.player, h, 0));
    let v = walk.sim.vehicle_mut(h).unwrap();
    v.flyer.phase = FlightPhase::Flying;
    v.flyer.lift = 1.0;
    v.vel = Vec3::Z * params.mid_speed;
    let mut kbm = game::kbm::Kbm::default();
    for _ in 0..8 {
        let pad = kbm.flyer_pad(
            walk.map_flight_sticks(input::FlightControls::default(), controller)
                .unwrap(),
        );
        walk.step_tick(&[pad]);
    }
    walk.sim.vehicle(h).unwrap().clone()
}

#[test]
fn classic_left_stick_moves_every_loaded_flyer_class() {
    let root = game::assets::baked_root();
    let classes = root.join("classes");
    if !classes.join("bf1_cis.ron").is_file() {
        return;
    }
    let mut checked = [0; 2];
    let mut reported = [false; 3];
    let mut hovering = Vec::new();
    for (index, game) in ["bf1", "bf2"].into_iter().enumerate() {
        let mut db = ClassDb::default();
        let mut files: Vec<_> = std::fs::read_dir(&classes)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("{game}_"))
                    && p.extension().is_some_and(|e| e == "ron")
            })
            .collect();
        files.sort();
        for f in files {
            db.load(&f).unwrap();
        }
        let mut maps: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.join("manifest.ron").is_file())
            .collect();
        maps.sort();
        for map in maps {
            let text = std::fs::read_to_string(map.join("manifest.ron")).unwrap();
            let manifest: manifest::Manifest = ron::from_str(&text).unwrap();
            if manifest.source_game.eq_ignore_ascii_case(game) {
                db.load_map(&map).unwrap();
            }
        }
        let mut names: Vec<_> = db.names().map(str::to_owned).collect();
        names.sort();
        for name in names {
            let Some(class) = FlyerClass::from_db(&db, &name, game == "bf2") else {
                continue;
            };
            let params = class.params(0.0);
            let hover = !params.bf2
                && params.min_speed < 0.0
                && params.mid_speed == 0.0
                && params.strafe_speed > 0.0;
            if hover {
                hovering.push(format!("{game}/{name}"));
            }
            for (axis, pad) in [
                (
                    "left X",
                    PadState {
                        lx: PadState::quantize(0.8),
                        ..Default::default()
                    },
                ),
                (
                    "left Y",
                    PadState {
                        ly: PadState::quantize(0.8),
                        ..Default::default()
                    },
                ),
                (
                    "right X",
                    PadState {
                        rx: PadState::quantize(0.8 * LOOK_SCALE),
                        ..Default::default()
                    },
                ),
                (
                    "right Y",
                    PadState {
                        ry: PadState::quantize(0.8 * LOOK_SCALE),
                        ..Default::default()
                    },
                ),
            ] {
                let v = flight(&params, pad);
                let channels = [
                    v.input.turn,
                    v.input.pitch,
                    v.input.strafe,
                    v.input.throttle,
                ];
                let channel = match (hover, axis) {
                    (false, "left X") | (true, "right X") => 0,
                    (false, "left Y") | (true, "right Y") => 1,
                    (false, "right X") | (true, "left X") => 2,
                    _ => 3,
                };
                for (i, value) in channels.into_iter().enumerate() {
                    assert_eq!(
                        value.abs() > 0.1,
                        i == channel,
                        "{game} {name}, {axis}: {channels:?}"
                    );
                }
            }
            if name == "cis_fly_geofighter" && game == "bf1" {
                let neutral = flight(&params, PadState::default());
                let turn = flight(
                    &params,
                    PadState {
                        lx: 32767,
                        ..Default::default()
                    },
                );
                let pitch = flight(
                    &params,
                    PadState {
                        ly: 32767,
                        ..Default::default()
                    },
                );
                let throttle = flight(
                    &params,
                    PadState {
                        ry: PadState::quantize(LOOK_SCALE),
                        ..Default::default()
                    },
                );
                assert!((turn.rot * Vec3::Z).x.abs() > 0.01);
                assert!((pitch.rot * Vec3::Z).y.abs() > 0.01);
                assert!(throttle.vel.length() > neutral.vel.length() + 0.1);
                reported[0] = true;
            }
            if game == "bf1" && ["rep_fly_gunship", "cis_fly_maf"].contains(&name.as_str()) {
                assert!(hover, "{name} must retain hovering translation");
                let neutral = flight(&params, PadState::default());
                let forward = flight(
                    &params,
                    PadState {
                        ly: 32767,
                        ..Default::default()
                    },
                );
                let reverse = flight(
                    &params,
                    PadState {
                        ly: -32767,
                        ..Default::default()
                    },
                );
                let sideways = flight(
                    &params,
                    PadState {
                        lx: 32767,
                        ..Default::default()
                    },
                );
                let aim = flight(
                    &params,
                    PadState {
                        rx: PadState::quantize(LOOK_SCALE),
                        ry: PadState::quantize(LOOK_SCALE),
                        ..Default::default()
                    },
                );
                assert!(forward.vel.z > neutral.vel.z + 0.1, "{name}: forward");
                assert!(reverse.vel.z < -0.1, "{name}: reverse");
                assert!(sideways.vel.x.abs() > 0.1, "{name}: strafe");
                assert!(
                    (forward.rot * Vec3::Z).x.abs() < 0.001,
                    "{name}: movement must not yaw"
                );
                assert!(
                    (forward.rot * Vec3::Z).y.abs() < 0.001,
                    "{name}: movement must not pitch"
                );
                assert!((aim.rot * Vec3::Z).x.abs() > 0.01, "{name}: aim yaw");
                assert!((aim.rot * Vec3::Z).y.abs() > 0.01, "{name}: aim pitch");
                reported[if name == "rep_fly_gunship" { 1 } else { 2 }] = true;
            }
            checked[index] += 1;
        }
    }
    assert!(
        reported.iter().all(|v| *v),
        "all three reported aircraft must be checked: {reported:?}"
    );
    assert!(checked[0] >= 10 && checked[1] >= 10, "{checked:?}");
    eprintln!("Hover translation: {hovering:?}");
    eprintln!(
        "Classic controls checked on {} BF1 and {} BF2 flyer classes",
        checked[0], checked[1]
    );
}

#[test]
fn geonosis_geofighters_spawn_naturally_at_both_spire_pads() {
    let root = game::assets::baked_root();
    let (walk, _) = battle::battle_on(&root, "bf1_geo1", "bf1_geo1_con").unwrap();
    let ci = walk
        .vehicle_classes
        .iter()
        .position(|c| c == "cis_fly_geofighter")
        .unwrap();
    let live: Vec<_> = walk
        .sim
        .alive_vehicles()
        .filter(|(_, v)| v.class as usize == ci)
        .collect();
    assert_eq!(
        live.len(),
        2,
        "two natural starfighters without a vehicle shortcut"
    );
    for (_, v) in live {
        let pad = &walk.sim.vehicle_pads[v.spawn_pad.expect("natural pad provenance") as usize];
        assert!(["geofighter", "geofighter1"].contains(&pad.def.name.as_str()));
        let cp = &walk.sim.cps[pad.def.cp.unwrap()];
        assert!(cp.def.name.eq_ignore_ascii_case("cp6"));
        assert_eq!(cp.owner, Some(1));
        assert!((v.pos - pad.def.pos).with_y(0.0).length() < 0.01);
        assert_eq!(v.flyer.phase, FlightPhase::Landed);
        assert!(v.seats.iter().all(|s| s.occupant.is_none()));
        eprintln!(
            "{} at {:?}, CP {}, owner {:?}",
            pad.def.name, v.pos, cp.def.name, cp.owner
        );
    }
}

#[test]
#[ignore = "requires local BF1 Geonosis assets and a headless GPU"]
fn natural_geofighters_are_visible_at_the_spire_spawn_pads() {
    let root = game::assets::baked_root();
    let (mut walk, scene) = battle::battle_on(&root, "bf1_geo1", "bf1_geo1_con").unwrap();
    let ci = walk
        .vehicle_classes
        .iter()
        .position(|c| c == "cis_fly_geofighter")
        .unwrap();
    let live: Vec<_> = walk
        .sim
        .alive_vehicles()
        .filter(|(_, v)| v.class as usize == ci)
        .map(|(h, _)| h)
        .collect();
    assert_eq!(live.len(), 2);
    for v in &mut walk.sim.vehicles {
        v.alive &= v.class as usize == ci;
    }
    let centre = live
        .iter()
        .map(|&h| walk.sim.vehicle(h).unwrap().pos)
        .sum::<Vec3>()
        / 2.0;
    let mut r = render::Renderer::new_headless(640, 480).unwrap();
    let gpu = game::viewer::upload_scene(&mut r, &scene).unwrap();
    let mut vis = game::viewer::VehicleVisuals::load(&mut r, &scene, &root, &walk.vehicle_geometry);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/starfighter-controls");
    std::fs::create_dir_all(&out).unwrap();
    let mut most_visible = 0;
    for (i, offset) in [
        Vec3::new(0.0, 5.0, 18.0),
        Vec3::new(18.0, 5.0, 0.0),
        Vec3::new(0.0, 5.0, -18.0),
        Vec3::new(-18.0, 5.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let position = centre + offset;
        let dir = (centre + Vec3::Y - position).normalize();
        let cam = render::Camera {
            position,
            yaw: (-dir.x).atan2(-dir.z),
            pitch: dir.y.asin(),
            ..Default::default()
        };
        r.render_frame(
            &cam,
            &gpu.draws,
            &[],
            game::viewer::clear_color(&scene),
            None,
        )
        .unwrap();
        let (empty, _, _) = r.capture_rgba().unwrap();
        let mut draws = gpu.draws.clone();
        draws.extend(vis.draws(&mut r, &walk.sim, &[]));
        r.render_frame(&cam, &draws, &[], game::viewer::clear_color(&scene), None)
            .unwrap();
        let (pixels, w, h) = r.capture_rgba().unwrap();
        let changed = pixels
            .chunks_exact(4)
            .zip(empty.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        most_visible = most_visible.max(changed);
        std::fs::write(
            out.join(format!("natural-pads-{i}.png")),
            render::encode_png(&pixels, w, h).unwrap(),
        )
        .unwrap();
        eprintln!("Geonosian natural spawn view {i}: {changed} vehicle pixels");
    }
    assert!(
        most_visible > 100,
        "naturally spawned models must be visible in the actual map"
    );
}
