use game::native_teams::native_side;
use game::roster::{load_classdb, ClassRoster};

fn roster(game: &str, class: &str) -> (&'static str, ClassRoster) {
    let classes = game::assets::baked_root().join("classes");
    let (side, file) =
        native_side(&classes, game, class).unwrap_or_else(|| panic!("{game} {class}"));
    let mut db = load_classdb(&classes, game, &[]).unwrap();
    db.load(&file).unwrap();
    (side, ClassRoster::build(&db, class, 1, None).unwrap())
}

#[test]
fn every_fielded_native_class_resolves_to_its_side_and_baked_model() {
    let root = game::assets::baked_root();
    for (game, class, side, collision, rig) in [
        ("bf1", "ewk_inf_trooper", "ewk", 1.0, 0.5),
        ("bf1", "wok_inf_warrior", "wok", 1.15, 1.15),
        ("bf1", "gun_inf_soldier", "gun", 1.0, 1.1),
        ("bf1", "gun_inf_rider", "gun", 1.0, 1.1),
        ("bf1", "geo_inf_geonosian", "geo", 1.0, 1.0),
        ("bf1", "tat_inf_jawa", "des", 0.6, 0.6),
        ("bf1", "tat_inf_tuskenraider", "des", 1.0, 1.0),
        ("bf1", "tat_inf_tuskenhunter", "des", 1.0, 1.0),
        ("bf2", "ewk_inf_trooper", "ewk", 1.0, 0.5),
        ("bf2", "wok_inf_warrior", "wok", 1.15, 1.0),
        ("bf2", "geo_inf_geonosian", "geo", 1.0, 1.0),
        ("bf2", "tat_inf_jawa", "des", 0.6, 0.6),
    ] {
        let (found, r) = roster(game, class);
        assert_eq!(found, side, "{game} {class}");
        let model = r.geometry.as_deref().unwrap();
        assert!(
            pak::is_file(
                root.join(format!("side_{game}/locals/soldiers"))
                    .join(format!("{model}.glb"))
            ),
            "{game} {class}: {model}"
        );
        assert_eq!(r.consts.collision_scale, collision, "{game} {class}");
        assert_eq!(r.anim.root_scale, rig, "{game} {class}");
    }
}

#[test]
fn a_soldier_class_outside_the_native_sides_is_not_fielded_as_native() {
    let classes = game::assets::baked_root().join("classes");
    assert!(native_side(&classes, "bf1", "imp_droid_probe").is_none());
    assert!(native_side(&classes, "bf1", "rep_inf_clone_trooper").is_none());
}

#[test]
fn geonosians_fly_on_the_hover_jet_bots_already_schedule() {
    for game in ["bf1", "bf2"] {
        let (_, r) = roster(game, "geo_inf_geonosian");
        assert!(r.jet && r.consts.has_jet() && r.consts.jet_hover, "{game}");
        assert_eq!(r.consts.jet_jump, 8.0, "{game}");
        assert_eq!(r.consts.jet_push, 8.0, "{game}");
        assert_eq!(r.jet_look.effect.as_deref(), Some("geo_wingsfly"), "{game}");
    }
}

#[test]
fn native_thrown_primaries_fly_and_aim_at_their_full_charge() {
    for (class, weapon, velocity, reach) in [
        (
            "gun_inf_soldier",
            "gun_weap_inf_haywiredetonator",
            25.0,
            22.5,
        ),
        ("gun_inf_rider", "gun_weap_inf_haywiredetonator", 25.0, 22.5),
        ("ewk_inf_trooper", "ewk_weap_inf_spear", 25.0, 50.0),
        ("ewk_inf_trooper", "ewk_weap_inf_rock", 20.0, 60.0),
    ] {
        let (_, r) = roster("bf1", class);
        let w = std::iter::once(&r.weapon)
            .chain(r.held.iter().map(|h| &h.weapon))
            .find(|w| w.name == weapon)
            .unwrap_or_else(|| panic!("{class} {weapon}"));
        assert_eq!(w.max_pressed_time, 0.0, "{weapon}");
        assert_eq!(w.shot_ordnance().velocity, velocity, "{weapon}");
        assert!(
            (w.shot_range() - reach).abs() < 1e-3,
            "{weapon} {}",
            w.shot_range()
        );
    }
}

#[test]
fn bf1_geonosians_fly_on_their_bf2_twins_jet_rules() {
    let (_, bf1) = roster("bf1", "geo_inf_geonosian");
    let (_, bf2) = roster("bf2", "geo_inf_geonosian");
    assert_eq!(
        (bf2.consts.jet_acceleration, bf2.consts.jet_control),
        (20.0, [2.0, 1.25, 1.25])
    );
    assert_eq!(bf1.consts.jet_acceleration, bf2.consts.jet_acceleration);
    assert_eq!(bf1.consts.jet_control, bf2.consts.jet_control);
}
