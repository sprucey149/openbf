//! Charge survives stock roster loading and the real FIRE/release path.
//! Retained class data establishes authoring, not a retail throw capture.

use core::sim::{buttons, Event, PadState, Sim};
use core::unit::UnitSpec;
use core::weapon::WeaponClass;
use glam::Vec3;

fn fire(mut weapon: WeaponClass, hold: u32, bf2: bool) -> (bool, Vec3, f32) {
    let mut sim = Sim::empty(82);
    sim.ground_rules = if bf2 {
        core::soldier::GroundRules::Bf2
    } else {
        core::soldier::GroundRules::Bf1
    };
    sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 1000.0,
        offset_x: 500.0,
        offset_z: 500.0,
        heights: vec![0.0; 4],
    });
    for stage in &mut weapon.stages {
        stage.index = sim.add_weapon_class((*stage.class).clone());
    }
    let wc = sim.add_weapon_class(weapon);
    let owner = sim
        .spawn(&UnitSpec {
            weapon_class: wc,
            ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
        })
        .unwrap();
    sim.unit_mut(owner).unwrap().soldier.pitch = 0.0;
    for tick in 0..hold + 100 {
        sim.step(&[PadState {
            buttons: if tick < hold { buttons::FIRE } else { 0 },
            ..Default::default()
        }]);
        for event in &sim.events {
            match event {
                Event::Throw {
                    weapon_class,
                    origin,
                    ..
                } => {
                    let g = sim.stickies.last().expect("throw created an instance");
                    assert_eq!(g.weapon_class, *weapon_class as usize);
                    assert!(!g.armed && g.touched.is_none());
                    assert!(
                        (g.pos - *origin - g.vel * core::TICK_DT).length() < 1e-5,
                        "measurement must observe exactly one flight step"
                    );
                    let launch = g.vel - Vec3::Y * (g.gravity * core::TICK_DT);
                    return (true, launch, g.life);
                }
                Event::Shot { weapon_class, .. } => {
                    let b = sim.bolts.last().expect("shot created a shell");
                    assert_eq!(b.weapon_class, *weapon_class as usize);
                    return (
                        false,
                        b.vel + Vec3::Y * (b.gravity * core::TICK_DT),
                        b.remaining + b.age,
                    );
                }
                _ => {}
            }
        }
    }
    panic!("FIRE/release did not create a round");
}

#[test]
#[ignore = "requires CHARGED_STICKY_ASSETS pointing to a retained source profile"]
fn stock_thrown_rounds_use_authored_charge() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CHARGED_STICKY_ASSETS").expect("explicit retained profile required"),
    );
    // Local side installation belongs to its own lane. Load the retained
    // Ewok rows explicitly through ClassDb, without changing team ownership.
    let load = |era: &str, common: &str| {
        let mut db = game::classdb::ClassDb::default();
        for side in [common, "ewk", "all", "imp", "cis"] {
            db.load(&root.join("classes").join(format!("{era}_{side}.ron")))
                .unwrap();
        }
        db
    };
    let bf1 = load("bf1", "common");
    let bf2 = load("bf2", "ingame");
    for (era, db, name, speed, life) in [
        ("bf1", &bf1, "ewk_weap_inf_spear", 25.0, 2.0),
        ("bf1", &bf1, "ewk_weap_inf_rock", 20.0, 3.0),
        ("bf2", &bf2, "ewk_weap_inf_spear", 50.0, 3.0),
        ("bf1", &bf1, "all_weap_inf_mortar_launcher", 25.0, 0.5),
    ] {
        let weapon = game::roster::weapon_from_db(db, name).unwrap();
        let (sticky, velocity, actual_life) = fire(weapon, 1, db.is_bf2());
        println!("{era} {name} sticky={sticky} velocity={velocity:?} life={actual_life}");
        assert_eq!(sticky, !name.ends_with("rock"));
        assert!(
            (velocity.length() - speed).abs() < 1e-5,
            "{name}: {velocity:?}"
        );
        assert_eq!(actual_life, life);
    }
    // Both public roster entry points must retain the spear's root and knots.
    let spear = game::roster::thrown_weapon_from_db(&bf1, "ewk_weap_inf_spear")
        .unwrap()
        .unwrap();
    assert_eq!(spear.ordnance.velocity, 5.0);
    assert_eq!(spear.ordnance.knots.len(), 1);
    assert_eq!(spear.ordnance.charged(1.0).velocity, 25.0);
    let (_, velocity, _) = fire(spear, 1, false);
    assert!((velocity.length() - 25.0).abs() < 1e-5);
    for (era, db, fuse) in [("bf1", &bf1, 0.9), ("bf2", &bf2, 0.6)] {
        let weapon = game::roster::thrown_weapon_from_db(db, "imp_weap_inf_thermaldetonator")
            .unwrap()
            .unwrap();
        assert!(weapon.ordnance.knots.is_empty());
        let (sticky, velocity, life) = fire(weapon, 1, db.is_bf2());
        println!("{era} ordinary thermal sticky={sticky} velocity={velocity:?} life={life}");
        assert!(sticky);
        assert!((velocity.length() - 25.0).abs() < 1e-5);
        assert_eq!(life, fuse);
    }
    for (era, db, fuse) in [("bf1", &bf1, 0.5), ("bf2", &bf2, 0.1)] {
        for hold in [1, 40] {
            let weapon = game::roster::weapon_from_db(db, "imp_weap_inf_mortar_launcher").unwrap();
            let (sticky, velocity, life) = fire(weapon, hold, db.is_bf2());
            println!(
                "{era} stock mortar hold={hold} sticky={sticky} velocity={velocity:?} life={life}"
            );
            assert_eq!(sticky, hold == 40);
            assert!((velocity.length() - 40.0).abs() < 1e-5);
            if sticky {
                assert_eq!(life, fuse);
            }
        }
    }
}
