//! Retained map/class witness for the bound minigun interaction. This is a
//! controlled port scene, not an original-game or exact input replay.

use core::collision::Layers;
use core::sim::{buttons, Event, PadState, Sim};
use core::unit::Controller;
use glam::Vec3;

#[test]
#[ignore = "requires CORUSCANT_TURRET_ASSETS with a complete retained BF2 bake"]
fn stock_maul_destroys_the_bound_minigun_from_the_reported_pose() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CORUSCANT_TURRET_ASSETS").expect("CORUSCANT_TURRET_ASSETS"),
    );
    let dir = root.join("bf2_cor1");
    let mut scene = game::assets::load_scene(&dir).expect("Coruscant scene");
    scene.apply_mode_layer("con");
    scene.merge_common(
        game::assets::load_common(&root.join("common_bf2")).expect("BF2 common collision"),
    );
    let mut sim = Sim::empty(43);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let factions = ["cis".to_owned(), "rep".to_owned()];
    let build = game::vehicle_pads::install(
        &mut sim,
        &scene.manifest,
        &dir,
        &root.join("classes"),
        &factions,
    )
    .expect("real map turret installation");
    // Fresh build zones are unbuilt. The F9 emplacement was already standing;
    // complete its ordinary rebuild path, restoring its own bodies and pool.
    let building = sim
        .building_turrets
        .iter()
        .position(|t| t.spec.pos.distance(Vec3::new(-20.19, 7.0, -34.76)) < 0.1)
        .expect("reported build zone");
    if sim.building_turrets[building].down.is_some() {
        let health = sim.building_turrets[building].spec.max_health;
        sim.repair_building(building as u16, health);
    }
    let (turret, vehicle) = sim
        .alive_vehicles()
        .find(|(_, v)| v.pos.distance(Vec3::new(-20.19, 7.0, -34.76)) < 0.1)
        .expect("reported minigun");
    let instance = vehicle.instance.expect("placed turret binding");
    assert_eq!(build.classes[vehicle.class as usize], "tur_bldg_laser");
    assert_eq!(vehicle.health.cur, 800.0);
    assert_eq!(vehicle.team, None);
    assert!(vehicle.seats.iter().all(|s| s.occupant.is_none()));
    assert!(sim.hull_parts[vehicle.class as usize].is_none());
    assert_eq!(sim.collision.turret(instance), Some(turret));
    let prims = sim
        .collision
        .bodies()
        .iter()
        .filter(|b| b.instance == Some(instance) && b.layers.contains(Layers::ORDNANCE))
        .count();
    assert!(prims > 0, "authored ordnance collision required");
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["cis", "rep"])
        .expect("stock classes");
    let roster =
        game::roster::ClassRoster::build(&db, "cis_hero_darthmaul", 0, None).expect("stock Maul");
    let class = roster.register(&mut sim, &mut Vec::new());
    let owner = sim
        .spawn(&class.spec(
            0,
            Vec3::new(-18.8, 7.01, -34.2),
            -5.05,
            Controller::Player { slot: 0 },
            0,
            0,
        ))
        .expect("Maul");
    sim.unit_mut(owner).unwrap().soldier.pitch = -0.46;
    assert_eq!(
        sim.weapon_classes[sim.unit(owner).unwrap().weapon_class].name,
        "cis_weap_doublesaber"
    );
    let mut hits = Vec::new();
    let mut destroyed = None;
    for tick in 0..360 {
        sim.step(&[PadState {
            buttons: if tick % 12 == 0 { buttons::FIRE } else { 0 },
            ..Default::default()
        }]);
        for event in &sim.events {
            match event {
                Event::VehicleHit {
                    shooter,
                    vehicle,
                    damage,
                    ..
                } if *shooter == Some(owner) && *vehicle == turret => {
                    hits.push((tick, *damage));
                }
                Event::VehicleDestroyed {
                    vehicle, killer, ..
                } if *vehicle == turret && *killer == Some(owner) => destroyed = Some(tick),
                _ => {}
            }
        }
        if destroyed.is_some() {
            break;
        }
    }
    println!("instance={instance} ordnance_bodies={prims} hits={hits:?} destroyed={destroyed:?}");
    assert!(
        !hits.is_empty(),
        "no player blade hit on the reported minigun"
    );
    assert!(destroyed.is_some(), "minigun not destroyed by stock Maul");
    assert!(sim.vehicle(turret).is_none());
    assert!(sim.building_down(instance));
    assert_eq!(sim.collision.turret(instance), None);
}
