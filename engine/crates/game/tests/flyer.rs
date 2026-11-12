use game::classdb::{fnv1a_lowercase as h, ClassDb};
use game::vehicle_class::FlyerClass;
use std::path::PathBuf;

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
        let rig = WalkerRig::new_flyer(asset.skeleton, asset.clips);
        let (landed, _) = skin(
            &rig.flyer_palette(0.0).unwrap(),
            &mesh.positions,
            &mesh.normals,
            &joints,
        );
        let (flying, _) = skin(
            &rig.flyer_palette(1.0).unwrap(),
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
