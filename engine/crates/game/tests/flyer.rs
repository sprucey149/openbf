use game::classdb::{fnv1a_lowercase as h, ClassDb};
use game::vehicle_class::FlyerClass;
use std::path::PathBuf;

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
