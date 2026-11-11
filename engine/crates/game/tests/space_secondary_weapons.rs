use core::collision::{HullPart, Layers, Shape};
use core::missile::MissileTarget;
use core::sim::{buttons, PadState, Sim};
use core::unit::UnitSpec;
use core::vehicle::flyer::FlightPhase;
use core::vehicle::{SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use core::weapon::{HealthType, OrdKind, WeaponClass};
use game::classdb::{fnv1a_lowercase, ClassDb};
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn root(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(fallback))
}

fn spec(class: usize, team: u8, pos: Vec3) -> VehicleSpec {
    VehicleSpec {
        kind: VehicleKind::Flyer,
        class: class as u32,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 100000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
}

fn scenario(setup: &Sim, class: usize, seed: u32, speed: f32) -> (Sim, VehicleHandle) {
    let mut sim = Sim::empty(seed);
    sim.weapon_classes = setup.weapon_classes.clone();
    sim.hull_parts = setup.hull_parts.clone();
    sim.blast_setup = setup.blast_setup.clone();
    sim.missile_target_points = setup.missile_target_points.clone();
    sim.seat_setup = setup.seat_setup.clone();
    sim.flyer_params = vec![None; setup.flyer_params.len()];
    let mut p = setup.flyer_params[class].clone().unwrap();
    p.min_speed = speed;
    p.mid_speed = speed;
    p.max_speed = speed;
    sim.flyer_params[class] = Some(p);
    let own = sim.spawn_vehicle(&spec(class, 0, Vec3::Y * 100.0));
    let me = sim.spawn(&UnitSpec::player(0, Vec3::Y * 100.0)).unwrap();
    assert!(sim.seat_unit(me, own, 0));
    sim.vehicle_mut(own).unwrap().flyer.phase = FlightPhase::Flying;
    (sim, own)
}

fn select(sim: &mut Sim) {
    sim.step(&[PadState {
        buttons: buttons::RELOAD,
        ..Default::default()
    }]);
    assert!(sim.lock_on_target(0).is_some());
}

fn check_authored(db: &ClassDb, weapon: &WeaponClass) {
    let ord_name = db.prop(&weapon.name, "OrdnanceName").unwrap();
    let chain = db.chain(ord_name);
    let props: Vec<_> = chain
        .iter()
        .rev()
        .flat_map(|d| d.props.iter())
        .take_while(|p| p.0 != fnv1a_lowercase("NextCharge"))
        .collect();
    for (key, actual) in [
        ("Velocity", weapon.ordnance.velocity),
        ("LifeSpan", weapon.ordnance.life_span),
        ("MaxDamage", weapon.ordnance.max_damage),
        ("Gravity", weapon.ordnance.gravity),
    ]
    .into_iter()
    .chain(weapon.ordnance.missile.into_iter().flat_map(|m| {
        [
            ("TurnRate", m.turn_rate),
            ("WaverRate", m.waver_rate),
            ("WaverTurn", m.waver_turn),
            ("MinSpeed", m.min_speed),
            ("Acceleration", m.acceleration),
        ]
    })) {
        if key == "Gravity" && weapon.ordnance.kind != OrdKind::Shell {
            continue;
        }
        if let Some(authored) = props.iter().rev().find(|p| p.0 == fnv1a_lowercase(key)) {
            assert_eq!(
                actual,
                authored.1.parse::<f32>().unwrap(),
                "{} {ord_name} {key}",
                weapon.name
            );
        }
    }
    assert!(
        weapon.ordnance.velocity > 0.0
            && weapon.ordnance.life_span > 0.0
            && weapon.ordnance.max_damage > 0.0
    );
    if let Some(m) = weapon.ordnance.missile {
        assert!(
            m.turn_rate > 0.0 && m.ramps && weapon.lock_time > 0.0,
            "{} must steer",
            weapon.name
        );
    } else {
        assert_eq!(weapon.ordnance.kind, OrdKind::Shell);
        assert!(weapon.ordnance.gravity > 0.0 && weapon.ordnance.explosion.is_some());
    }
}

fn unguided_case(setup: &Sim, class: usize, target_class: usize, wc: usize, picked: bool) {
    let (mut sim, _) = scenario(setup, class, 4, 0.0);
    sim.spawn_vehicle(&spec(target_class, 1, Vec3::new(0.0, 100.0, 350.0)));
    if picked {
        select(&mut sim);
        assert!(sim.lock_on_charge(0, 1).is_some_and(|v| v < 1.0));
    }
    let initial_dwell = sim.lock_on_charge(0, 1).unwrap_or(0.0) * sim.weapon_classes[wc].lock_time;
    let mut rounds = 0;
    for tick in 0..50 {
        sim.step(&[PadState {
            buttons: if tick == 0 {
                buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        }]);
        for b in sim.bolts.iter().filter(|b| {
            b.weapon_class == wc && b.visual_shot.is_some_and(|s| s.tick + 1 == sim.tick)
        }) {
            if !picked
                || (rounds == 0
                    && initial_dwell + (tick + 1) as f32 * core::TICK_DT
                        < sim.weapon_classes[wc].lock_time)
            {
                assert_eq!(
                    b.missile.unwrap().target,
                    None,
                    "{} picked={picked}: first shot before full dwell must not guide",
                    sim.weapon_classes[wc].name
                );
            }
            rounds += 1;
        }
    }
    assert!(
        rounds > 0,
        "{} picked={picked}: secondary must fire",
        sim.weapon_classes[wc].name
    );
}

fn locked_case(
    setup: &Sim,
    class: usize,
    target_class: usize,
    wc: usize,
    seed: u32,
    motion: usize,
) -> bool {
    let speed = if motion == 3 { 70.0 } else { 0.0 };
    let (mut sim, own) = scenario(setup, class, seed, 0.0);
    let target = sim.spawn_vehicle(&spec(target_class, 1, Vec3::new(0.0, 100.0, 350.0)));
    select(&mut sim);
    let dwell = (sim.weapon_classes[wc].lock_time / core::TICK_DT).ceil() as usize + 2;
    for _ in 0..dwell {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(
        sim.lock_on_charge(0, 1),
        Some(1.0),
        "{} full dwell",
        sim.weapon_classes[wc].name
    );
    let p = sim.flyer_params[class].as_mut().unwrap();
    p.min_speed = speed;
    p.mid_speed = speed;
    p.max_speed = speed;
    sim.vehicle_mut(own).unwrap().vel = Vec3::Z * speed;
    let velocity = [
        Vec3::ZERO,
        Vec3::X * 35.0,
        Vec3::Z * 70.0,
        Vec3::new(35.0, 0.0, 70.0),
    ][motion];
    let mut rounds = 0;
    for tick in 0..210 {
        let v = sim.vehicle_mut(target).unwrap();
        v.vel = velocity;
        v.pos += velocity * core::TICK_DT;
        sim.step(&[PadState {
            buttons: if tick == 0 {
                buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        }]);
        for b in sim.bolts.iter().filter(|b| {
            b.weapon_class == wc && b.visual_shot.is_some_and(|s| s.tick + 1 == sim.tick)
        }) {
            assert_eq!(
                b.missile.unwrap().target,
                Some(MissileTarget::Vehicle(target)),
                "{} seed {seed} motion {motion}: whole salvo keeps its lock",
                sim.weapon_classes[wc].name
            );
            rounds += 1;
        }
    }
    let w = &sim.weapon_classes[wc];
    assert_eq!(
        rounds,
        w.salvo_count * w.shots_per_salvo,
        "{} complete salvo",
        w.name
    );
    sim.vehicle(target).unwrap().health.cur < 100000.0
}

fn bomb_case(setup: &Sim, class: usize, wc: usize, seed: u32) {
    let (mut sim, _) = scenario(setup, class, seed, 70.0);
    let target_class = sim.hull_parts.len();
    sim.hull_parts.push(Some(
        vec![HullPart::new(
            Mat4::IDENTITY,
            Shape::Box {
                half: Vec3::new(500.0, 2.0, 1000.0),
            },
            Layers::ORDNANCE,
        )]
        .into(),
    ));
    let target = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        seats: vec![SeatKind::Gunner(0)],
        health_type: HealthType::Building,
        ..spec(target_class, 1, Vec3::new(0.0, 0.0, 1000.0))
    });
    let mut rounds = 0;
    let mut fell = false;
    for tick in 0..330 {
        sim.step(&[PadState {
            buttons: if tick == 0 {
                buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        }]);
        for b in sim.bolts.iter().filter(|b| b.weapon_class == wc) {
            assert!(b.missile.is_none() && b.gravity > 0.0);
            fell |= b.vel.y < -20.0;
            rounds += usize::from(b.visual_shot.is_some_and(|s| s.tick + 1 == sim.tick));
        }
    }
    assert!(
        rounds > 0 && fell,
        "{}: launched bombs must fall",
        sim.weapon_classes[wc].name
    );
    assert!(
        sim.vehicle(target).unwrap().health.cur < 100000.0,
        "{}: ballistic bomb must damage hull below",
        sim.weapon_classes[wc].name
    );
}

fn reload_case(setup: &Sim, class: usize, wc: usize) {
    let (mut sim, own) = scenario(setup, class, 8, 0.0);
    sim.step(&[PadState::default()]);
    let initial = sim.vehicle(own).unwrap().guns[0].as_ref().unwrap().weapons[1].clone();
    let mut reloaded = false;
    let mut fired_again = false;
    for tick in 0..1800 {
        sim.step(&[PadState {
            buttons: if tick % 30 < 3 {
                buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        }]);
        reloaded |= sim.events.iter().any(|e| matches!(e, core::sim::Event::Reload { weapon_class, .. } if *weapon_class as usize == wc));
        let state = &sim.vehicle(own).unwrap().guns[0].as_ref().unwrap().weapons[1];
        fired_again =
            state.shots_fired as u32 * sim.weapon_classes[wc].ammo_per_shot > initial.clip;
        if reloaded && fired_again {
            break;
        }
    }
    let v = sim.vehicle(own).unwrap();
    let gun = v.guns[0].as_ref().unwrap();
    assert_eq!(
        gun.weapons[0].shots_fired, 0,
        "secondary input must not fire the primary"
    );
    assert!(
        reloaded && fired_again,
        "{} must fire again after exhausting its first clip",
        sim.weapon_classes[wc].name
    );
    if initial.spare == core::weapon::SPARE_INFINITE {
        assert_eq!(gun.weapons[1].spare, initial.spare);
    } else {
        assert!(
            gun.weapons[1].spare < initial.spare,
            "finite reserve must pay for reload"
        );
    }
}

fn native_contact_probe(setup: &Sim, class: usize) {
    let Some(paths) = std::env::var_os("OPENBF_NATIVE_CONTACTS") else {
        return;
    };
    for path in paths.to_string_lossy().split(';') {
        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let rows = data["rows"].as_array().unwrap();
        assert!(
            setup.hull_parts[class]
                .as_ref()
                .unwrap()
                .iter()
                .any(|p| matches!(p.shape, Shape::Mesh(_)) && p.layers.contains(Layers::ORDNANCE)),
            "X-wing fuselage must answer projectile queries"
        );
        let decode = |row: &serde_json::Value, read: usize| -> Vec<u8> {
            row["reads"][read]["hex"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .collect()
        };
        let float = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let pos = |b: &[u8]| Vec3::new(float(b, 0x100), float(b, 0x104), float(b, 0x108));
        let mut world = core::collision::CollisionWorld::new(Vec::new(), Vec::new());
        let mut hit = false;
        let mut native_hit = false;
        for pair in rows.windows(2) {
            let a = decode(&pair[0], 0);
            let b = decode(&pair[1], 0);
            let t = decode(&pair[1], 1);
            let prev_t = decode(&pair[0], 1);
            if u32::from_le_bytes(a[0x14..0x18].try_into().unwrap()) != 0x627648 {
                break;
            }
            if float(&b, 0x40) <= float(&a, 0x40) {
                continue;
            }
            native_hit |= float(&prev_t, 0x170) - float(&t, 0x170) > 1000.0;
            let matrix = std::array::from_fn(|i| float(&t, 0x120 + 4 * i));
            world.set_hull(
                VehicleHandle { index: 0, gen: 1 },
                setup.hull_parts[class].as_ref().unwrap(),
                Mat4::from_cols_array(&matrix),
            );
            let end = Vec3::new(float(&b, 0x50), float(&b, 0x54), float(&b, 0x58));
            let off = end - pos(&b);
            hit |= world
                .ray(
                    pos(&b),
                    off.normalize_or_zero(),
                    off.length() + 0.01,
                    Layers::ORDNANCE,
                )
                .is_some();
        }
        println!("native collision replay {path}: native={native_hit} port={hit}");
        assert!(
            native_hit && hit,
            "native impact must intersect the baked X-wing hull"
        );
    }
}

#[test]
fn every_space_secondary_loadout_fires_and_hits() {
    let root = root("OPENBF_ASSETS_BAKED", "../../assets_baked");
    let missions = self::root("OPENBF_MISSIONS", "../../missions");
    if !root.join("bf2_spa1").exists() {
        assert!(
            std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
            "space assets required"
        );
        eprintln!("skipping space secondaries: local baked assets unavailable");
        return;
    }
    let mut seen = BTreeMap::<String, WeaponClass>::new();
    let mut report = Vec::new();
    let (mut maps, mut mounts, mut missile_cases, mut bomb_cases) = (0, 0, 0, 0);
    for map in [
        "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
    ] {
        for mode in ["ass", "1flag"] {
            let mission_path = missions.join(format!("{map}_{mode}.ron"));
            assert!(
                root.join(map).exists() && mission_path.exists(),
                "missing {map} {mode}"
            );
            let mut scene = game::assets::load_scene(&root.join(map)).unwrap();
            scene.apply_mode_layer(mode).expect("the mission layer");
            let mission = game::mission::MissionDef::load(&mission_path).unwrap();
            let mut setup = Sim::empty(290);
            let build = game::vehicle_pads::install_with_objective(
                &mut setup,
                &scene.manifest,
                &scene.dir,
                &root.join("classes"),
                &mission.factions(),
                None,
            )
            .unwrap();
            let mut db = ClassDb::default();
            db.load_map(&scene.dir).unwrap();
            for file in game::roster::class_files(
                "bf2",
                &mission
                    .factions()
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ) {
                db.load(&root.join("classes").join(file)).unwrap();
            }
            if map == "bf2_spa1" && mode == "ass" {
                native_contact_probe(
                    &setup,
                    build
                        .classes
                        .iter()
                        .position(|n| n == "all_fly_xwing_sc")
                        .unwrap(),
                );
            }
            let mut map_mounts = 0;
            for (class, name) in build.classes.iter().enumerate() {
                if setup.flyer_params[class].is_none() {
                    continue;
                }
                let seats = setup.seat_setup[class].as_ref().unwrap();
                for (seat, gun) in seats.guns.iter().enumerate() {
                    let Some(gun) = gun else {
                        continue;
                    };
                    assert!(
                        gun.weapons.len() <= 2,
                        "{map} {name} seat {seat}: inaccessible extra weapon"
                    );
                    if seat > 0 {
                        assert_eq!(gun.weapons.len(), 1, "new gunner secondary needs coverage");
                    }
                }
                let gun = seats.guns[0].as_ref().unwrap();
                assert_eq!(gun.weapons.len(), 2, "{map} {name}: primary and secondary");
                if name == "cis_fly_tridroidfighter" {
                    let points = &gun.weapons[1].fire_points;
                    assert_eq!(points.len(), 3);
                    assert!(points[0].distance(points[1]) > 2.0);
                    assert!(points[1].distance(points[2]) > 3.0);
                    let exhaust = &build.flyer_exhaust[class];
                    assert_eq!(exhaust.len(), 3);
                    assert!(exhaust
                        .iter()
                        .all(|e| e.effect == "com_sfx_exhaust_jedistarfighter"));
                    assert!(exhaust[0].local.w_axis.distance(exhaust[1].local.w_axis) > 0.5);
                }
                if name == "all_fly_awing" {
                    for weapon in &gun.weapons {
                        assert_eq!(weapon.fire_points.len(), 2);
                        assert!(weapon.fire_points[0].distance(weapon.fire_points[1]) > 5.0);
                    }
                }
                let wc = gun.weapons[1].class;
                let w = &setup.weapon_classes[wc];
                check_authored(&db, w);
                map_mounts += 1;
                if let Some(old) = seen.get(name) {
                    assert_eq!(old, w, "{map} {mode} {name}: loadout differs across maps");
                    continue;
                }
                seen.insert(name.clone(), w.clone());
                reload_case(&setup, class, wc);
                let (mut hits, mut cases) = ([0; 4], [0; 4]);
                if w.ordnance.missile.is_some() {
                    let enemy = if name.starts_with("rep_") {
                        "cis_"
                    } else if name.starts_with("cis_") {
                        "rep_"
                    } else if name.starts_with("all_") {
                        "imp_"
                    } else {
                        "all_"
                    };
                    let targets: Vec<_> = build
                        .classes
                        .iter()
                        .enumerate()
                        .filter(|(c, n)| n.starts_with(enemy) && setup.flyer_params[*c].is_some())
                        .collect();
                    assert_eq!(targets.len(), 4);
                    unguided_case(&setup, class, targets[0].0, wc, false);
                    unguided_case(&setup, class, targets[0].0, wc, true);
                    for (target_class, target_name) in targets {
                        for motion in 0..4 {
                            let mut target_hits = 0;
                            for seed in 0..4 {
                                let hit =
                                    locked_case(&setup, class, target_class, wc, seed, motion);
                                target_hits += usize::from(hit);
                                hits[motion] += usize::from(hit);
                                cases[motion] += 1;
                                missile_cases += 1;
                            }
                            if motion == 0 || motion == 2 {
                                assert!(target_hits > 0, "{name} -> {target_name} motion {motion}: no hits in four locked salvos");
                            }
                        }
                    }
                    assert!(
                        hits.iter().all(|n| *n > 0),
                        "{name}: no hits in a motion category {hits:?}"
                    );
                } else {
                    for seed in 0..4 {
                        bomb_case(&setup, class, wc, seed);
                        bomb_cases += 1;
                    }
                    hits[0] = 4;
                    cases[0] = 4;
                }
                println!("{name}: {} hits {hits:?} of {cases:?}", w.name);
                report.push(serde_json::json!({"craft":name,"weapon":w.name,"kind":format!("{:?}",w.ordnance.kind),"hits":hits,"cases":cases}));
            }
            assert!(
                (6..=8).contains(&map_mounts),
                "{map} {mode}: cover every fielded craft secondary"
            );
            println!("{map} {mode}: {map_mounts} secondary mounts");
            mounts += map_mounts;
            maps += 1;
        }
    }
    assert_eq!(
        (maps, mounts, seen.len(), missile_cases, bomb_cases),
        (12, 86, 16, 768, 16)
    );
    println!("{maps} map/modes, {mounts} mounts, {} unique secondaries, {missile_cases} locked missile and {bomb_cases} bomb cases, 24 no/partial-lock cases", seen.len());
    if let Some(path) = std::env::var_os("OPENBF_SPACE_SECONDARY_REPORT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
}
