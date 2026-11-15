use game::classdb::{fnv1a_lowercase as h, ClassDb};
use game::vehicle_class::FlyerClass;
use std::path::PathBuf;

#[path = "../examples/support/walker_battle.rs"]
mod seat_fixture;

fn isolated_flyer(
    root: &std::path::Path,
    name: &str,
    frame: glam::Quat,
    wall_distance: f32,
    seat: u8,
) -> (game::walk::WalkSim, usize, core::vehicle::VehicleHandle) {
    use core::sim::PadState;
    use glam::{Mat4, Vec3};
    let (mut walk, _) = seat_fixture::battle_on(root, "bf1_bes1", "bf1_bes1_con_cw").unwrap();
    let class = walk.vehicle_classes.iter().position(|c| c == name).unwrap();
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == class)
        .unwrap();
    let handle = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    walk.sim.flyer_params[class] = None;
    walk.sim.heightfield = None;
    walk.sim.collision = Default::default();
    walk.sim.vehicle_pads.clear();
    for (i, v) in walk.sim.vehicles.iter_mut().enumerate() {
        if i != slot {
            v.alive = false;
        }
    }
    for (i, u) in walk.sim.units.iter_mut().enumerate() {
        if i != walk.player.index as usize {
            u.alive = false;
        }
    }
    let pos = Vec3::new(0.0, 100.0, 0.0);
    let v = walk.sim.vehicle_mut(handle).unwrap();
    v.pos = pos;
    v.rot = frame;
    v.flyer.phase = core::vehicle::flyer::FlightPhase::Flying;
    v.flyer.lift = 1.0;
    walk.sim.collision.push_body(core::collision::Body::new(
        Mat4::from_rotation_translation(frame, pos + frame * Vec3::Z * wall_distance),
        core::collision::Shape::Box {
            half: Vec3::new(200.0, 200.0, 0.5),
        },
    ));
    assert!(walk.sim.seat_unit(walk.player, handle, seat));
    for _ in 0..60 {
        walk.step_tick(&[PadState::default()]);
    }
    (walk, class, handle)
}

#[test]
fn laat_pilot_cannons_meet_the_chase_reticle_at_finite_range() {
    use core::sim::{buttons, Event, PadState};
    use glam::{Quat, Vec3};
    let root = game::assets::baked_root();
    if !root.join("bf1_bes1/manifest.ron").is_file() {
        return;
    }
    // At 35 m the elevated pilot camera asks for more than the authored
    // cannon's 20-degree pitch travel. These targets are inside that arc.
    let pos = Vec3::new(0.0, 100.0, 0.0);
    let frame = Quat::from_rotation_x(0.45);
    for distance in [60.0, 120.0] {
        let (mut walk, class, _handle) =
            isolated_flyer(&root, "rep_fly_gunship", frame, distance, 0);
        let look = walk.sim.seat_look(walk.player).unwrap();
        let section = walk.sim.seat_chase[class][0].unwrap();
        let cam = section.camera_on(pos, frame, (0.0, 0.0));
        assert!(
            (look.eye - cam).cross(look.view).length() < 0.001,
            "pilot ray must pass through chase camera"
        );
        let local_eye = frame.inverse() * (look.eye - pos);
        let target = look.eye + look.view * (distance - 0.5 - local_eye.z);
        let mut shots = 0;
        let mut outlets = std::collections::BTreeSet::new();
        for _ in 0..30 {
            walk.step_tick(&[PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            }]);
            for event in &walk.sim.events {
                if let Event::Shot { origin, dir, .. } = event {
                    let miss = (target - *origin).cross(*dir).length();
                    assert!(miss < 0.1, "LAAT cannon misses {distance}m reticle by {miss}m; origin {origin:?}, dir {dir:?}, target {target:?}");
                    outlets.insert(((frame.inverse() * (*origin - pos)).x > 0.0) as u8);
                    shots += 1;
                }
            }
        }
        assert!(shots >= 2);
        assert_eq!(outlets.len(), 2);
    }
}

#[test]
fn laat_and_maf_beams_converge_on_the_same_finite_reticle_target() {
    use core::sim::{buttons, PadState};
    let root = game::assets::baked_root();
    if !root.join("bf1_bes1/manifest.ron").is_file() {
        return;
    }
    for (name, outlets) in [
        ("rep_fly_gunship", ["wing_turret_2", "wing_turret_1"]),
        ("cis_fly_maf", ["hp_laser2", "hp_laser1"]),
    ] {
        for distance in [35.0, 120.0] {
            let (mut walk, class, handle) =
                isolated_flyer(&root, name, glam::Quat::IDENTITY, distance, 1);
            let rig = walk.flyer_rigs[class].as_ref().unwrap().clone();
            let mut seen = std::collections::BTreeSet::new();
            let mut together = false;
            for _ in 0..45 {
                walk.step_tick(&[PadState {
                    buttons: buttons::FIRE,
                    rx: 512,
                    ..Default::default()
                }]);
                let v = walk.sim.vehicle(handle).unwrap();
                let target = v.guns[1]
                    .as_ref()
                    .unwrap()
                    .target
                    .expect("one shared target");
                assert!(
                    (target.z - (distance - 0.5)).abs() < 0.01,
                    "{name} {distance}m fixture must hit finite wall; target {target:?}, look {:?}",
                    v.guns[1].as_ref().unwrap().look
                );
                let target = v.pos + v.rot * target;
                let nodes = rig.flyer_aimed_nodes(v, None);
                let mut count = 0;
                for beam in walk.sim.beams.iter().filter(|b| {
                    b.seat
                        .is_some_and(|s| s.mount.vehicle == handle && s.mount.seat == 1)
                }) {
                    let p = beam.seat.unwrap().point.unwrap() as usize;
                    seen.insert(p);
                    count += 1;
                    let miss = (target - beam.origin).cross(beam.dir).length();
                    assert!(
                        miss < 0.03,
                        "{name} {distance}m emitter {p} misses reticle by {miss}m"
                    );
                    let end = beam.origin + beam.dir * beam.len;
                    assert!(
                        end.distance(target) < 0.05,
                        "{name} emitter {p} hit {:?} instead of {target:?}",
                        end
                    );
                    let node = rig
                        .skeleton
                        .index_of(outlets[p])
                        .expect("drawn outlet node");
                    let drawn = v.pos + v.rot * nodes[node].w_axis.truncate();
                    assert!(
                        drawn.distance(beam.origin) < 0.05,
                        "{name} emitter {p}: drawn outlet {drawn:?}, beam {:?}",
                        beam.origin
                    );
                }
                together |= count == 2;
            }
            assert_eq!(seen, [0, 1].into_iter().collect());
            assert!(together, "{name} must converge both beams together");
        }
    }
}

#[test]
fn gunship_wing_beams_fire_together_and_respect_each_sides_arc() {
    use core::sim::{buttons, PadState};
    let root = game::assets::baked_root();
    if !root.join("bf1_bes1/manifest.ron").is_file() {
        return;
    }
    for game in ["bf1", "bf2"] {
        let mut db = ClassDb::disc();
        db.load(&root.join(format!("classes/{game}_rep.ron")))
            .unwrap();
        let seats = game::vehicle_class::seats(&db, "rep_fly_gunship");
        assert_eq!(
            seats[1].weapons.len(),
            1,
            "{game} wing section is one weapon"
        );
        assert_eq!(
            seats[1]
                .aimers
                .iter()
                .filter(|a| a.weapon == 0 && !a.node.is_empty())
                .count(),
            2
        );
    }
    let (mut walk, _) = seat_fixture::battle_on(&root, "bf1_bes1", "bf1_bes1_con_cw").unwrap();
    let class = walk
        .vehicle_classes
        .iter()
        .position(|c| c == "rep_fly_gunship")
        .unwrap();
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == class)
        .unwrap();
    let handle = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    walk.sim.flyer_params[class] = None;
    walk.sim.heightfield = None;
    walk.sim.collision = Default::default();
    walk.sim.vehicle_mut(handle).unwrap().pos.y += 40.0;
    walk.sim.vehicle_mut(handle).unwrap().flyer.phase = core::vehicle::flyer::FlightPhase::Flying;
    assert!(walk.sim.seat_unit(walk.player, handle, 1));
    walk.sim.arm(slot);
    let setup = walk.sim.seat_setup[class].as_ref().unwrap().guns[1]
        .as_ref()
        .unwrap();
    assert_eq!(setup.weapons.len(), 1);
    assert_eq!(setup.weapons[0].fire_points.len(), 2);
    assert_ne!(
        setup.weapons[0].fire_points[0],
        setup.weapons[0].fire_points[1]
    );
    for (degrees, expected) in [(0.0f32, vec![0, 1]), (-70.0, vec![0]), (70.0, vec![1])] {
        walk.sim.beams.clear();
        let yaw = degrees.to_radians();
        {
            let state = walk.sim.vehicle_mut(handle).unwrap().guns[1]
                .as_mut()
                .unwrap();
            state.look = (yaw, 0.0);
            state.aim = (yaw, 0.0);
            state.rel = (yaw, 0.0);
            for w in &mut state.weapons {
                w.heat = 0.0;
            }
        }
        for _ in 0..60 {
            walk.step_tick(&[PadState::default()]);
        }
        let mut points = std::collections::BTreeSet::new();
        let mut simultaneous = false;
        for _ in 0..30 {
            walk.step_tick(&[PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            }]);
            let mut current = std::collections::BTreeSet::new();
            let v = walk.sim.vehicle(handle).unwrap();
            for beam in walk.sim.beams.iter().filter(|b| {
                b.seat
                    .is_some_and(|s| s.mount.vehicle == handle && s.mount.seat == 1)
            }) {
                let p = beam
                    .seat
                    .unwrap()
                    .point
                    .expect("wing beam retains its emitter");
                points.insert(p);
                current.insert(p);
                let local = v.rot.inverse() * (beam.origin - v.pos);
                let setup = walk.sim.seat_setup[class].as_ref().unwrap().guns[1]
                    .as_ref()
                    .unwrap();
                let outlet = setup.weapons[0].fire_points[p as usize];
                assert!(
                    local.distance(outlet) < 0.01,
                    "beam is off wing outlet: {local:?} / {outlet:?}"
                );
                let dir = v.rot.inverse() * beam.dir;
                let beam_yaw = dir.x.atan2(dir.z).to_degrees();
                assert!(
                    (beam_yaw - degrees).abs() < 2.0,
                    "wing beam points away from aim: {beam_yaw} / {degrees}"
                );
            }
            simultaneous |= current.len() == 2;
        }
        assert_eq!(
            points.into_iter().collect::<Vec<_>>(),
            expected,
            "look {degrees} deg"
        );
        if degrees == 0.0 {
            assert!(simultaneous, "both beams must exist together");
        }
    }
}

#[test]
fn bf1_gunship_ball_glass_riders_beam_outlets_and_passenger_look() {
    use core::sim::{buttons, PadState};
    use glam::Vec3;
    let root = game::assets::baked_root();
    if !root.join("bf1_bes1/manifest.ron").is_file() {
        assert!(std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none());
        return;
    }
    let (mut walk, _) = seat_fixture::battle_on(&root, "bf1_bes1", "bf1_bes1_con_cw").unwrap();
    let class = walk
        .vehicle_classes
        .iter()
        .position(|c| c == "rep_fly_gunship")
        .unwrap();
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == class)
        .unwrap();
    let handle = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    walk.sim.flyer_params[class] = None;
    let rig = walk.flyer_rigs[class]
        .as_ref()
        .expect("rebake the BF1 Republic vehicles")
        .clone();
    let model = root.join("side_bf1/vehicles/rep/models/rep_la_at_gunship.glb");
    let (mesh, joints) = game::assets::load_glb_joints(&model).unwrap();
    assert!(
        !joints.is_empty(),
        "the gunship must carry its bone assignments"
    );
    assert!(mesh
        .submeshes
        .iter()
        .any(|s| s.2.eq_ignore_ascii_case("REP_LA_AT_BALLS") && s.3 == render::DrawLayer::Blend));
    assert!(mesh
        .submeshes
        .iter()
        .any(|s| s.2.eq_ignore_ascii_case("REP_LA_AT_BALLS") && s.3 == render::DrawLayer::Opaque));
    for (seat, point, rider, yaw) in [
        (2, "hp_turret_right", "hp_gunner2", -0.8),
        (3, "hp_turret_left", "hp_gunner1", 0.8),
    ] {
        assert!(walk.sim.seat_unit(walk.player, handle, seat));
        walk.sim.arm(slot);
        walk.sim.vehicle_mut(handle).unwrap().flyer.phase =
            core::vehicle::flyer::FlightPhase::Flying;
        let node = rig.skeleton.index_of(point).unwrap();
        let passenger = rig.skeleton.index_of(rider).unwrap();
        let before = rig.flyer_aimed_nodes(walk.sim.vehicle(handle).unwrap(), None);
        {
            let v = walk.sim.vehicle_mut(handle).unwrap();
            let gun = v.guns[seat as usize].as_mut().unwrap();
            gun.look = (yaw, 0.2);
            gun.aim = gun.look;
        }
        let v = walk.sim.vehicle(handle).unwrap();
        let now = rig.flyer_aimed_nodes(v, None);
        assert!(
            before[node].w_axis.distance(now[node].w_axis) > 0.01,
            "{point} did not turn"
        );
        assert!(
            before[passenger].w_axis.distance(now[passenger].w_axis) > 0.01,
            "{rider} did not follow the ball"
        );
        let setup = walk.sim.seat_setup[class].as_ref().unwrap().guns[seat as usize]
            .as_ref()
            .unwrap();
        let state = v.guns[seat as usize].as_ref().unwrap();
        let origin = setup.shot_point(0, setup.next_point(0, 0), state.aim, false);
        let drawn = now[node].w_axis.truncate();
        assert!(
            origin.distance(drawn) < 0.01,
            "seat {seat}: beam {origin:?}, drawn outlet {drawn:?}"
        );
        let mut beams = 0;
        for _ in 0..20 {
            walk.step_tick(&[PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            }]);
            let v = walk.sim.vehicle(handle).unwrap();
            let posed = rig.flyer_aimed_nodes(v, None);
            let outlet = v.pos + v.rot * posed[node].w_axis.truncate();
            for beam in walk.sim.beams.iter().filter(|b| {
                b.shooter.owner == walk.player && b.seat.is_some_and(|s| s.mount.seat == seat)
            }) {
                assert!(
                    beam.origin.distance(outlet) < 0.03,
                    "seat {seat}: live beam {:?}, outlet {outlet:?}",
                    beam.origin
                );
                beams += 1;
            }
        }
        assert!(beams > 0, "seat {seat} never fired its beam");
        walk.sim.unseat_unit(walk.player);
    }
    assert!(walk.sim.seat_unit(walk.player, handle, 4));
    let before = walk.camera();
    for _ in 0..30 {
        walk.step_tick(&[PadState {
            rx: i16::MAX,
            ry: i16::MAX,
            ..Default::default()
        }]);
    }
    let after = walk.camera();
    assert!(
        before.forward().distance(after.forward()) > 0.1,
        "standing passenger view is fixed"
    );
    let v = walk.sim.vehicle(handle).unwrap();
    let local = v.rot.inverse() * (after.position - v.pos);
    assert!(
        local.distance(Vec3::new(1.5, -0.75, 1.75)) < 0.05,
        "passenger mount camera {local:?}"
    );
    assert!(v.guns[4].as_ref().unwrap().weapons.is_empty());
    walk.step_tick(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    assert!(
        !walk
            .sim
            .events
            .iter()
            .any(|e| matches!(e, core::sim::Event::Shot { .. })),
        "unarmed passenger fired"
    );
}

#[test]
fn baked_flyer_takeoff_clips_move_the_models() {
    use game::viewer::walker_pose::{skin, WalkerRig};
    let root = game::assets::baked_root();
    if !root.join("side_bf2/vehicles/all/vehicles.ron").is_file()
        || !root.join("side_bf2/vehicles/cis/vehicles.ron").is_file()
    {
        assert!(std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none());
        return;
    }
    for (side, model) in [("all", "all_fly_xwing"), ("cis", "cis_fly_droidfighter")] {
        let dir = root.join("side_bf2/vehicles").join(side);
        let path = dir.join("walkers").join(format!("{model}.glb"));
        assert!(
            path.is_file(),
            "missing rebaked flyer rig: {}",
            path.display()
        );
        let (mesh, joints) =
            game::assets::load_glb_joints(&dir.join("models").join(format!("{model}.glb")))
                .unwrap();
        let asset = game::assets::load_skinned_glb(&path).unwrap();
        assert!(
            asset
                .clips
                .iter()
                .find(|c| c.name == "takeoff")
                .unwrap()
                .frames
                > 1
        );
        let rig = WalkerRig::new_flyer(asset.skeleton, asset.clips, None);
        let (landed, _) = skin(
            &rig.flyer_palette(0.0, None).unwrap(),
            &mesh.positions,
            &mesh.normals,
            &joints,
        );
        let (flying, _) = skin(
            &rig.flyer_palette(1.0, None).unwrap(),
            &mesh.positions,
            &mesh.normals,
            &joints,
        );
        let moved = landed
            .iter()
            .zip(&flying)
            .filter(|(a, b)| glam::Vec3::from(**a).distance(glam::Vec3::from(**b)) > 0.01)
            .count();
        assert!(moved > 100, "{model}: only {moved} vertices moved");
    }
}

#[test]
fn baked_snowspeeder_fins_move_its_flaps_by_speed_and_turn() {
    use game::viewer::turret_pose::fin_pose;
    use game::viewer::walker_pose::{skin, WalkerRig};
    let root = game::assets::baked_root();
    for game in ["bf1", "bf2"] {
        let dir = root.join(format!("side_{game}/vehicles/all"));
        let Ok(text) = std::fs::read_to_string(dir.join("vehicles.ron")) else {
            assert!(std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none());
            continue;
        };
        let model = "all_fly_snowspeeder";
        assert!(
            text.contains("fin: Some(\"all_snowspeeder_9pose\")"),
            "{game}: the snowspeeder's fins clip is not baked"
        );
        let (mesh, joints) =
            game::assets::load_glb_joints(&dir.join("models").join(format!("{model}.glb")))
                .unwrap();
        let asset =
            game::assets::load_skinned_glb(&dir.join("walkers").join(format!("{model}.glb")))
                .unwrap();
        let rig = WalkerRig::new_flyer(asset.skeleton, asset.clips, Some("all_snowspeeder_9pose"));
        let speeds = [20.0, 30.0, 40.0];
        let at = |turn, speed| {
            skin(
                &rig.flyer_palette(1.0, Some(fin_pose(turn, speed, speeds)))
                    .unwrap(),
                &mesh.positions,
                &mesh.normals,
                &joints,
            )
            .0
        };
        let moved = |a: &[[f32; 3]], b: &[[f32; 3]]| {
            a.iter()
                .zip(b)
                .filter(|(a, b)| glam::Vec3::from(**a).distance(glam::Vec3::from(**b)) > 0.01)
                .count()
        };
        let cruise = at(0.0, 30.0);
        let bind = skin(
            &rig.skeleton
                .palette(&game::skin::Pose::bind(&rig.skeleton).locals()),
            &mesh.positions,
            &mesh.normals,
            &joints,
        )
        .0;
        assert_eq!(moved(&cruise, &bind), 0, "{game}: frame 4 is the bind");
        for speed in [0.0, 40.0] {
            let n = moved(&cruise, &at(0.0, speed));
            assert!(n > 20, "{game} speed {speed}: only {n} vertices moved");
        }
        for speed in [0.0, 30.0, 40.0] {
            let (right, left) = (at(1.0, speed), at(-1.0, speed));
            assert!(moved(&at(0.0, speed), &right) > 0, "{game} speed {speed}");
            assert!(moved(&right, &left) > 0, "{game} speed {speed}");
        }
    }
}

#[test]
fn turret_first_refusal_and_chunk_fallback_preserve_flight_properties() {
    let file = std::env::temp_dir().join(format!("openbf-flyer-scope-{}.ron", std::process::id()));
    std::fs::write(&file, format!(r#"[
      (name:"parent",kind:"entc",base:"flyer",props:[({},"1.25"),({},"50")]),
      (name:"child",kind:"entc",base:"parent",props:[({},"82"),({},"TURRET1"),({},"99"),({},"CHUNK1"),({},"101"),({},"33"),({},"-5")])
    ]"#, h("TurnRate"), h("MidSpeed"), h("MaxSpeed"), h("FLYERSECTION"), h("TurnRate"), h("CHUNKSECTION"), h("MidSpeed"), h("Acceleration"), h("EnergyBar"))).unwrap();
    let mut db = ClassDb::default();
    db.load(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    let p = FlyerClass::from_db(&db, "child", false)
        .unwrap()
        .params(2.5);
    assert_eq!(
        (p.turn_rate, p.mid_speed, p.max_speed, p.landed_height),
        (1.25, 101.0, 82.0, 2.5)
    );
    assert_eq!(p.acceleration, 33.0);
    assert_eq!(
        p.energy_max, 100.0,
        "negative EnergyBar must not reach float::clamp"
    );
    assert!(FlyerClass::from_db(&db, "missing", false).is_none());
}

#[test]
#[ignore = "requires both PS2 ALL.LVL extracts; bakes only temporary class RONs"]
fn ps2_disc_flyer_classes_supply_the_actual_flight_numbers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for game in ["bf1", "bf2"] {
        let source = root.join(format!("extracted/{game}/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL"));
        assert!(source.is_file(), "missing PS2 source {}", source.display());
        let out =
            std::env::temp_dir().join(format!("openbf-flyer-{game}-{}.ron", std::process::id()));
        bake::classes::bake_classes(source.to_str().unwrap(), out.to_str().unwrap()).unwrap();
        let mut db = ClassDb::default();
        db.load(&out).unwrap();
        std::fs::remove_file(&out).unwrap();
        let name = if game == "bf1" {
            "all_fly_xwing"
        } else {
            "all_fly_xwing_sc"
        };
        let p = FlyerClass::from_db(&db, name, game == "bf2")
            .unwrap()
            .params(0.0);
        if game == "bf1" {
            assert_eq!(
                (p.acceleration, p.min_speed, p.mid_speed, p.max_speed),
                (30.0, 20.0, 50.0, 82.0)
            );
            assert_eq!(
                (p.pitch_rate, p.pitch_filter, p.turn_rate, p.turn_filter),
                (1.0, 2.9, 1.25, 1.75)
            );
        } else {
            assert_eq!(
                (p.acceleration, p.min_speed, p.mid_speed, p.max_speed),
                (50.0, 35.0, 70.0, 95.0)
            );
            assert_eq!((p.pitch_damp, p.turn_damp, p.level_damp), (4.0, 4.0, 5.0));
            assert_eq!(
                (p.boost_speed, p.boost_acceleration, p.energy_max),
                (150.0, 80.0, 80.0)
            );
            assert_eq!((p.energy_auto_restore, p.energy_boost_drain), (7.5, 20.0));
        }
    }
}

#[test]
fn flight_admission_metadata_reaches_sim_with_retail_names_and_follower_class() {
    let m: manifest::Manifest = ron::from_str(&format!(
        r#"(
        map:"test",source_game:"bf2",models:[],textures:[],classes:[],geometries:[],instances:[],regions:[],
        paths:[(name:"type_EntityPath test",nodes:[],props:[],flight_props:[
            ({},"fighter 2"),({},"bomber"),({},"3"),({},"-1.9"),
            ({},"1"),({},"0.25"),({},"group")]),
            (name:"entitypathbranch excluded",nodes:[],props:[])]
    )"#,
        h("Class"),
        h("Class1"),
        h("MaxEntities"),
        h("Direction"),
        h("SingleDirection"),
        h("Frequency"),
        h("Name")
    ))
    .unwrap();
    let paths = game::vehicle_pads::flight_paths(&m);
    assert_eq!(paths.len(), 1);
    let p = &paths[0];
    assert_eq!(p.classes, vec![(h("fighter"), 2), (h("bomber"), u32::MAX)]);
    assert_eq!(
        (
            p.name,
            p.max_entities,
            p.direction,
            p.single_direction,
            p.frequency,
            p.enabled
        ),
        (h("group"), 3, -1, true, 0.25, true)
    );
    let file = std::env::temp_dir().join(format!("openbf-route-class-{}.ron", std::process::id()));
    std::fs::write(
        &file,
        format!(
            r#"[(name:"test",kind:"entc",base:"flyer",props:[({},"fighter")])]"#,
            h("PathFollowerClass")
        ),
    )
    .unwrap();
    let mut db = ClassDb::default();
    db.load(&file).unwrap();
    std::fs::remove_file(file).unwrap();
    assert_eq!(
        FlyerClass::from_db(&db, "test", true)
            .unwrap()
            .params(0.0)
            .follower
            .class_id,
        h("fighter")
    );
    let mut bf1 = m;
    bf1.source_game = "bf1".into();
    assert!(game::vehicle_pads::flight_paths(&bf1).is_empty());
}

#[test]
#[ignore = "requires owned before/after bakes at OPENBF_ROUTE_BAKES"]
fn flight_bakes_change_only_genuinely_authored_manifest_properties() {
    let root =
        PathBuf::from(std::env::var_os("OPENBF_ROUTE_BAKES").expect("set OPENBF_ROUTE_BAKES"));
    for key in ["bf1_bes1", "bf2_hot1", "bf2_spa1"] {
        let a = root.join("assets-before").join(key).join("manifest.ron");
        let b = root.join("assets-after").join(key).join("manifest.ron");
        let before = std::fs::read(&a).unwrap();
        let after = std::fs::read(&b).unwrap();
        let original: manifest::Manifest = ron::de::from_bytes(&before).unwrap();
        let mut current: manifest::Manifest = ron::de::from_bytes(&after).unwrap();
        if key.starts_with("bf1") {
            assert_eq!(before, after);
        } else {
            assert_ne!(before, after);
            let routes = game::vehicle_pads::flight_paths(&current);
            assert!(!routes.is_empty());
            if key == "bf2_hot1" {
                assert_eq!(routes.len(), 6);
                assert_eq!(routes.iter().filter(|p| p.single_direction).count(), 2);
            } else {
                assert!(routes.iter().any(|p| !p.classes.is_empty()));
            }
        }
        for p in &mut current.paths {
            p.flight_props.clear();
        }
        assert_eq!(
            original, current,
            "every other manifest field stays equal: {key}"
        );
    }
}

#[test]
fn flight_limits_use_ps2_decimal_prefix_and_unsigned_float_readers() {
    for (text, expected) in [
        ("2.5", 2),
        ("-1", u32::MAX),
        ("-2", u32::MAX - 1),
        ("+3tail", 3),
        ("no-number", 0),
        ("", 0),
        (" 4", 4),
    ] {
        let m: manifest::Manifest = ron::from_str(&format!(r#"(
            map:"test",source_game:"bf2",models:[],textures:[],classes:[],geometries:[],instances:[],regions:[],
            paths:[(name:"type_EntityPath test",nodes:[],props:[],flight_props:[({},"fighter {}")])])"#, h("Class"), text)).unwrap();
        assert_eq!(
            game::vehicle_pads::flight_paths(&m)[0].classes,
            vec![(h("fighter"), expected)],
            "{text}"
        );
    }
    for (text, expected) in [("2.5", 2), ("-1", 0), ("4294967296", u32::MAX)] {
        let m: manifest::Manifest = ron::from_str(&format!(r#"(
            map:"test",source_game:"bf2",models:[],textures:[],classes:[],geometries:[],instances:[],regions:[],
            paths:[(name:"type_EntityPath test",nodes:[],props:[],flight_props:[({},"{}")])])"#, h("MaxEntities"), text)).unwrap();
        assert_eq!(
            game::vehicle_pads::flight_paths(&m)[0].max_entities,
            expected,
            "{text}"
        );
    }
}

#[test]
#[ignore = "requires owned PS2 side extracts; bakes classes only into an owned temporary directory"]
fn flight_ps2_follower_class_census() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = std::env::temp_dir().join(format!("openbf-flight-census-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    for game in ["bf1", "bf2"] {
        let source = root.join(format!("extracted/{game}/lvl/DATA/_LVL_PS2"));
        let mut db = ClassDb::default();
        let files = if game == "bf1" {
            vec![
                "SIDE/ALL.LVL",
                "SIDE/CIS.LVL",
                "SIDE/IMP.LVL",
                "SIDE/REP.LVL",
                "COMMON/ENG/COMMON.LVL",
            ]
        } else {
            vec![
                "SIDE/ALL.LVL",
                "SIDE/CIS.LVL",
                "SIDE/IMP.LVL",
                "SIDE/REP.LVL",
                "INGAME.LVL",
                "SIDE/TUR.LVL",
            ]
        };
        for (i, file) in files.iter().enumerate() {
            let target = out.join(format!("{game}-{i}.ron"));
            bake::classes::bake_classes(
                source.join(file).to_str().unwrap(),
                target.to_str().unwrap(),
            )
            .unwrap();
            db.load(&target).unwrap();
        }
        let mut rows = db
            .names()
            .filter(|name| db.chain(name).first().is_some_and(|d| d.kind == "entc"))
            .filter_map(|name| {
                FlyerClass::from_db(&db, name, game == "bf2")
                    .map(|c| (name.to_owned(), c.params(0.0).follower.class_id))
            })
            .collect::<Vec<_>>();
        rows.sort();
        let classified = rows.iter().filter(|(_, c)| *c != 0).count();
        eprintln!(
            "{game} follower census: {} entities, {classified} classified: {rows:?}",
            rows.len()
        );
        let craft_rows = rows
            .iter()
            .filter(|(name, _)| !name.contains("_weap_"))
            .collect::<Vec<_>>();
        eprintln!("{game} vehicle-name records: {} total, {} classified; {} weapon-name records reported separately", craft_rows.len(), craft_rows.iter().filter(|(_, c)| *c != 0).count(), rows.len() - craft_rows.len());
        assert!(!rows.is_empty());
        if game == "bf1" {
            assert_eq!(classified, 0);
        } else {
            assert!(classified > 0);
            assert!(rows.contains(&("all_fly_snowspeeder".into(), 0)));
            assert!(rows.contains(&("all_fly_xwing_sc".into(), h("all"))));
            assert!(rows.contains(&("imp_fly_tiefighter_sc".into(), h("imp"))));
        }
    }
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
#[ignore = "requires owned Space Yavin rebake and OPENBF_ROUTE_MISSIONS; headless flight round"]
fn flight_space_yavin_seeded_round_uses_real_follower_classes() {
    use core::unit::{Controller, UnitSpec};
    use core::vehicle::flyer::FlightPhase;
    use glam::Vec3;
    let root =
        PathBuf::from(std::env::var_os("OPENBF_ROUTE_BAKES").expect("set OPENBF_ROUTE_BAKES"))
            .join("assets-after");
    let dir = root.join("bf2_spa1");
    let mut scene = game::assets::load_scene(&dir).unwrap();
    scene.apply_mode_layer("ass");
    scene.merge_common(game::assets::load_common(&root.join("common_bf2")).unwrap());
    let mission_dir = PathBuf::from(
        std::env::var_os("OPENBF_ROUTE_MISSIONS")
            .expect("set OPENBF_ROUTE_MISSIONS to owned --space --ps2-only mission output"),
    );
    let mission = game::mission::MissionDef::load(&mission_dir.join("bf2_spa1_ass.ron")).unwrap();
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = layout.apply_mission(&mission);
    assert_eq!(mission.teams.len(), 2);
    let factions = [0, 1].map(|i| mission.teams[i].faction.clone());
    for seed in [1, 42] {
        let run = || {
            let mut w = game::walk::WalkSim::new(seed, Vec3::ZERO, scene.heightfield());
            for (team, faction) in w.teams.iter_mut().zip(&factions) {
                team.faction = faction.clone();
            }
            w.add_posts(&layout);
            w.sim.vacate_player_slot(0);
            let mut sim = w.sim;
            sim.flight_limits = applied.flight_limits;
            sim.landing_regions = applied.landing_regions.clone();
            sim.ground_rules = core::soldier::GroundRules::Bf2;
            sim.flyer_bots = true;
            scene.install_collision(&mut sim);
            let pads = game::vehicle_pads::install(
                &mut sim,
                &scene.manifest,
                &dir,
                &root.join("classes"),
                &factions,
            )
            .unwrap();
            assert!(pads.absent.is_empty(), "missing classes: {:?}", pads.absent);
            eprintln!(
                "Space Yavin pads: {:?}",
                pads.pads
                    .iter()
                    .map(|p| (&p.name, p.team, p.spawn_time))
                    .collect::<Vec<_>>()
            );
            for _ in 0..600 {
                sim.step(&[]);
                if [0, 1].iter().all(|team| {
                    sim.alive_vehicles().any(|(_, v)| {
                        v.home_team == Some(*team)
                            && sim
                                .flyer_params
                                .get(v.class as usize)
                                .and_then(Option::as_ref)
                                .is_some_and(|p| p.follower.class_id != 0)
                    })
                }) {
                    break;
                }
            }
            let mut craft = Vec::new();
            // Seat one actual pad-spawned classified craft from each side.
            for team in [0, 1] {
                let (vh, class, pos) = sim
                    .alive_vehicles()
                    .find_map(|(vh, v)| {
                        let p = sim.flyer_params.get(v.class as usize)?.as_ref()?;
                        (v.home_team == Some(team) && p.follower.class_id != 0).then_some((
                            vh,
                            p.follower.class_id,
                            v.pos,
                        ))
                    })
                    .expect("classified retail flyer pad");
                let pilot = sim
                    .spawn(&UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(team, pos)
                    })
                    .unwrap();
                assert!(sim.seat_unit(pilot, vh, 0));
                // Stage in flight outside the hangar: admission is the subject;
                // takeoff/hangar escape and Space Assault scoring are not asserted.
                let path = sim
                    .flyer_paths
                    .iter()
                    .find(|p| p.classes.iter().any(|(c, _)| *c == class))
                    .unwrap();
                let start = path.nodes[0] + Vec3::Y * 20.0;
                let v = sim.vehicle_mut(vh).unwrap();
                v.pos = start;
                v.flyer.phase = FlightPhase::Flying;
                craft.push((vh, class, pilot));
            }
            let mut joined = [0; 2];
            let mut hashes = Vec::new();
            for _ in 0..1800 {
                sim.step(&[]);
                for (i, &(vh, class, pilot)) in craft.iter().enumerate() {
                    let v = sim.vehicle(vh).expect("craft remains alive");
                    assert_eq!(sim.unit(pilot).unwrap().mount.map(|m| m.vehicle), Some(vh));
                    if let Some(c) = v.flyer.ai.cursor {
                        assert!(sim.flyer_paths[usize::from(c.path)]
                            .classes
                            .iter()
                            .any(|(id, _)| *id == class));
                        joined[i] += 1;
                    }
                }
                hashes.push(sim.hash());
            }
            assert!(
                joined.iter().all(|n| *n > 600),
                "class-restricted flight ticks: {joined:?}"
            );
            eprintln!(
                "Space Yavin seed {seed}: joined ticks {joined:?}, hash {:016x}",
                sim.hash()
            );
            hashes
        };
        assert_eq!(run(), run());
    }
}

#[test]
fn stock_heavy_launchers_guide_a_rocket_to_a_hostile_aircraft() {
    use core::missile::MissileTarget;
    use core::sim::{buttons, PadState, Sim, SpaceAssault};
    use core::soldier::GroundRules;
    use core::unit::UnitSpec;
    use core::vehicle::{flyer::FlightPhase, SeatKind, VehicleKind, VehicleSpec};
    use glam::Vec3;

    let root = std::env::var_os("OPENBF_ASSETS_BAKED")
        .map(PathBuf::from)
        .unwrap_or_else(game::assets::baked_root)
        .join("classes");
    if !root.join("bf1_imp.ron").is_file() || !root.join("bf2_imp.ron").is_file() {
        assert!(
            std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
            "missing stock launcher classes in {}",
            root.display()
        );
        return;
    }
    for (game, class, ship, space) in [
        (
            "bf1",
            "imp_weap_inf_torpedo_launcher",
            "all_fly_xwing",
            false,
        ),
        (
            "bf2",
            "imp_weap_inf_rocket_launcher",
            "all_fly_xwing_sc",
            false,
        ),
        (
            "bf2",
            "imp_weap_inf_rocket_launcher",
            "all_fly_xwing_sc",
            true,
        ),
    ] {
        let db = game::roster::load_classdb(&root, game, &["imp", "all"]).unwrap();
        let launcher = game::roster::weapon_from_db(&db, class).unwrap();
        let mut sim = Sim::empty(1);
        sim.ground_rules = if game == "bf1" {
            GroundRules::Bf1
        } else {
            GroundRules::Bf2
        };
        if space {
            sim.install_space_assault(SpaceAssault::new(180));
        }
        sim.heightfield = Some(core::heightfield::Heightfield {
            size: 2,
            unit: 400.0,
            offset_x: 200.0,
            offset_z: 200.0,
            heights: vec![0.0; 4],
        });
        let class = sim.add_weapon_class(launcher);
        let player = sim
            .spawn(&UnitSpec {
                weapon_class: class,
                ..UnitSpec::player(0, Vec3::ZERO)
            })
            .unwrap();
        let target = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(1),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(-20.0, 24.0, -65.0),
            yaw: 0.0,
            max_health: 1_500.0,
            health_type: game::vehicle_class::health_type(&db, ship),
            seats: vec![SeatKind::Pilot],
        });
        sim.vehicle_mut(target).unwrap().flyer.phase = FlightPhase::Flying;
        for tick in 0..60 {
            let pos = Vec3::new(-20.0 + tick as f32 * 0.5, 24.0, -65.0);
            sim.vehicle_mut(target).unwrap().pos = pos;
            let u = sim.unit_mut(player).unwrap();
            let d = pos - (u.soldier.pos + Vec3::Y * 1.7);
            u.soldier.yaw = (-d.x).atan2(-d.z);
            u.soldier.pitch = d.y.atan2(d.x.hypot(d.z));
            sim.step(&[PadState::default()]);
        }
        let u = sim.unit(player).unwrap();
        assert!(
            u.weapon.lock_held >= sim.weapon_classes[class].lock_time,
            "{game} space={space}"
        );
        let fire = PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        };
        let missile = (0..30).find_map(|_| {
            sim.step(&[fire]);
            sim.bolts
                .iter()
                .find(|b| b.owner == player)
                .and_then(|b| b.missile)
        });
        assert_eq!(
            missile
                .expect("the launcher fires after its authored startup")
                .target,
            Some(MissileTarget::Vehicle(target)),
            "{game} space={space}"
        );
    }
}
