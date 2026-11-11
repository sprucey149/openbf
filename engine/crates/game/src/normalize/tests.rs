//! The rewrite rules on in-memory definitions; the baked classes' own
//! values are the data-gated suite's (`tests/suite/normalization.rs`).

use super::*;

fn def(name: &str, kind: &str, base: &str, props: &[(&str, &str)]) -> ClassDef {
    ClassDef {
        name: name.into(),
        kind: kind.into(),
        base: base.into(),
        props: props
            .iter()
            .map(|(k, v)| (fnv1a_lowercase(k), (*v).into()))
            .collect(),
    }
}

/// Every value of `key`, in file order.
fn all(d: &ClassDef, key: &str) -> Vec<f32> {
    let h = fnv1a_lowercase(key);
    d.props
        .iter()
        .filter(|p| p.0 == h)
        .map(|p| p.1.parse().unwrap())
        .collect()
}

#[test]
fn the_bf1_baseline_converts_exactly() {
    assert_eq!(BF1_TROOPER_HEALTH * BF1_SCALE, REFERENCE_HEALTH);
    // The rifle's 60 lands on 80 exactly, so five hits still kill.
    assert_eq!(60.0 * BF1_SCALE, 80.0);
    assert_eq!(5.0 * (60.0 * BF1_SCALE), REFERENCE_HEALTH);
}

#[test]
fn only_bf1_class_files_are_covered() {
    assert!(covers("bf1_all.ron"));
    assert!(covers("bf1_common.ron"));
    assert!(!covers("bf2_all.ron"));
    assert!(!covers("bf2_ingame.ron"));
    assert!(!covers("classes.ron"));
    assert!(!covers("droid_chunks.ron"));
    // A map's own class file goes by the map's game.
    assert!(covers_map("bf1"));
    assert!(covers_map("BF1"));
    assert!(!covers_map("bf2"));
}

#[test]
fn a_named_soldier_takes_its_row_and_others_scale() {
    let mut heavy = def(
        "all_inf_vanguardSnow",
        "entc",
        "soldier",
        &[("MaxHealth", "300.0"), ("MaxSpeed", "6.0")],
    );
    apply(&mut heavy);
    assert_eq!(all(&heavy, "MaxHealth"), [550.0]);
    assert_eq!(all(&heavy, "MaxSpeed"), [6.0]);

    let mut gungan = def(
        "gun_inf_soldier",
        "entc",
        "soldier",
        &[("MaxHealth", "125.0")],
    );
    apply(&mut gungan);
    assert_eq!(all(&gungan, "MaxHealth"), [200.0]);

    let mut hero = def(
        "imp_inf_darthvader",
        "entc",
        "soldier",
        &[("MaxHealth", "1000000.0")],
    );
    apply(&mut hero);
    assert_eq!(all(&hero, "MaxHealth"), [1_000_000.0]);

    let mut tank = def(
        "imp_hover_fightertank",
        "entc",
        "hover",
        &[("MaxHealth", "3000")],
    );
    apply(&mut tank);
    assert_eq!(
        all(&tank, "MaxHealth"),
        [3000.0],
        "vehicles are not soldiers"
    );
}

#[test]
fn soft_scales_move_and_hard_scales_stay() {
    let mut rifle = def(
        "all_weap_inf_rifle_ord",
        "ordc",
        "bolt",
        &[
            ("MaxDamage", "60.0"),
            ("PersonScale", "1.0"),
            ("DroidScale", "1.0"),
            ("VehicleScale", "0.0"),
            ("BuildingScale", "0.1"),
            ("ShieldScale", "1.0"),
        ],
    );
    apply(&mut rifle);
    assert_eq!(all(&rifle, "MaxDamage"), [60.0]);
    assert_eq!(all(&rifle, "PersonScale"), [BF1_SCALE]);
    assert_eq!(all(&rifle, "DroidScale"), [BF1_SCALE]);
    // An animal took the default 1.0: the key is added.
    assert_eq!(all(&rifle, "AnimalScale"), [BF1_SCALE]);
    assert_eq!(all(&rifle, "ShieldScale"), [BF1_SCALE]);
    assert_eq!(all(&rifle, "VehicleScale"), [0.0]);
    assert_eq!(all(&rifle, "BuildingScale"), [0.1]);
    assert_eq!(60.0 * all(&rifle, "PersonScale")[0], 80.0);
}

#[test]
fn a_combined_health_scale_moves_alone() {
    let mut blast = def(
        "imp_weap_inf_remotedroid_exp2",
        "expc",
        "explosion",
        &[
            ("Damage", "50.0"),
            ("HealthScale", "1.0"),
            ("DroidScale", "0.5"),
            ("VehicleScale", "0.5"),
        ],
    );
    apply(&mut blast);
    assert_eq!(all(&blast, "HealthScale"), [BF1_SCALE]);
    assert_eq!(all(&blast, "DroidScale"), [0.5 * BF1_SCALE]);
    assert!(all(&blast, "PersonScale").is_empty());
    assert!(all(&blast, "AnimalScale").is_empty());
    assert_eq!(all(&blast, "ShieldScale"), [BF1_SCALE]);
    assert_eq!(all(&blast, "VehicleScale"), [0.5]);
}

#[test]
fn every_charge_stage_moves() {
    let mut rock = def(
        "ewk_weap_inf_rock_ord",
        "ordc",
        "shell",
        &[
            ("MaxDamage", "50.0"),
            ("PersonScale", "1.0"),
            ("NextCharge", "1.0"),
            ("PersonScale", "2.0"),
        ],
    );
    apply(&mut rock);
    assert_eq!(all(&rock, "PersonScale"), [BF1_SCALE, 2.0 * BF1_SCALE]);
    // Added keys go in front, where every stage reads them.
    let front = rock.props[..3].iter().map(|p| p.0).collect::<Vec<_>>();
    for key in ["AnimalScale", "DroidScale", "ShieldScale"] {
        assert!(front.contains(&fnv1a_lowercase(key)), "{key}");
    }
}

#[test]
fn a_named_weapon_lands_on_its_target() {
    for (name, damage, scale, target) in [
        ("cis_weap_inf_pistol_ord", "40.0", "1.0", 60.0),
        ("rep_weap_inf_sniperrifle_ord", "160.0", "1.0", 300.0),
        ("all_weap_inf_thermaldetonator_exp", "320.0", "1.0", 450.0),
        ("cis_weap_inf_mortar_launcher_exp", "300.0", "1.0", 400.0),
        ("imp_weap_inf_remotedroid_blaster_ord", "10.0", "1.0", 60.0),
        ("ewk_weap_inf_spear_ord", "150.0", "1.0", 200.0),
        ("geo_weap_inf_blunderbuss_ord", "25.0", "1.0", 100.0),
        ("tat_weap_inf_tuskensniperrifle_ord", "160.0", "1.0", 200.0),
        ("tat_weap_inf_tuskenrifle_ord", "40.0", "1.0", 80.0),
        ("imp_weap_inf_rifle_ord", "60.0", "1.0", 70.0),
    ] {
        let mut d = def(
            name,
            "ordc",
            "bolt",
            &[("MaxDamage", damage), ("PersonScale", scale)],
        );
        apply(&mut d);
        let got = damage.parse::<f32>().unwrap() * all(&d, "PersonScale")[0];
        assert_eq!(got, target, "{name}");
    }
    // The Tusken sniper is not the factions' sniper rifle.
    assert!(!matches(
        "*_weap_inf_sniperrifle_ord",
        "tat_weap_inf_tuskensniperrifle_ord"
    ));
    assert!(!matches(
        "*_weap_inf_pistol_ord",
        "rep_weap_inf_commando_pistol_ord"
    ));
}

#[test]
fn a_weapon_already_at_its_target_is_left_as_written() {
    let props = [
        ("Damage", "400.0"),
        ("PersonScale", "0.25"),
        ("DroidScale", "1.0"),
    ];
    let mut mine = def("com_weap_inf_empmine_exp", "expc", "explosion", &props);
    apply(&mut mine);
    assert_eq!(
        mine,
        def("com_weap_inf_empmine_exp", "expc", "explosion", &props)
    );
}

#[test]
fn an_emitter_moves_its_damage_not_its_scales() {
    let mut arc = def(
        "rep_weap_inf_arccaster_ord",
        "ordc",
        "emitterordnance",
        &[
            ("MaxDamage", "250"),
            ("Damage", "100"),
            ("PersonScale", "1.0"),
            ("VehicleScale", "0.0"),
        ],
    );
    apply(&mut arc);
    assert_eq!(all(&arc, "MaxDamage"), [250.0 * BF1_SCALE]);
    assert_eq!(all(&arc, "Damage"), [100.0 * BF1_SCALE]);
    assert_eq!(all(&arc, "PersonScale"), [1.0]);
    assert!(all(&arc, "ShieldScale").is_empty());
}

/// A vehicle's gun, a turret's and a death blast hit the tougher soldier
/// as hard as they hit BF1's, and a vehicle no harder.
#[test]
fn other_guns_and_blasts_keep_their_strength_against_soldiers() {
    let mut cannon = def(
        "imp_weap_hover_fightertank_cannon_ord",
        "ordc",
        "bolt",
        &[
            ("MaxDamage", "300.0"),
            ("PersonScale", "1.0"),
            ("AnimalScale", "1.0"),
            ("DroidScale", "1.0"),
            ("VehicleScale", "0.19"),
            ("ShieldScale", "1.0"),
        ],
    );
    apply(&mut cannon);
    assert_eq!(all(&cannon, "MaxDamage"), [300.0]);
    assert_eq!(all(&cannon, "VehicleScale"), [0.19]);
    // One hit killed BF1's 300-health trooper; one kills the shared 400.
    assert_eq!(300.0 * all(&cannon, "PersonScale")[0], REFERENCE_HEALTH);
    assert_eq!(all(&cannon, "DroidScale"), [BF1_SCALE]);
    assert_eq!(all(&cannon, "ShieldScale"), [BF1_SCALE]);

    // A turret's bolt that writes no scale at all, and a death blast with
    // the combined scale.
    let mut minigun = def(
        "yav_weap_bldg_tower_minigun_ord",
        "ordc",
        "bolt",
        &[("MaxDamage", "100.0")],
    );
    apply(&mut minigun);
    for key in ["PersonScale", "AnimalScale", "DroidScale", "ShieldScale"] {
        assert_eq!(all(&minigun, key), [BF1_SCALE], "{key}");
    }
    assert!(all(&minigun, "VehicleScale").is_empty());
    assert!(all(&minigun, "BuildingScale").is_empty());
    let mut blast = def(
        "imp_walk_atat_exp",
        "expc",
        "explosion",
        &[("Damage", "400.0"), ("HealthScale", "1.0")],
    );
    apply(&mut blast);
    assert_eq!(all(&blast, "HealthScale"), [BF1_SCALE]);
    assert_eq!(all(&blast, "Damage"), [400.0]);
}

#[test]
fn weapons_and_damageless_classes_are_untouched() {
    for d in [
        def(
            "all_weap_inf_rifle",
            "wpnc",
            "cannon",
            &[("ShotDelay", "0.2"), ("RoundsPerClip", "50")],
        ),
        // A thrown grenade's own class authors no damage.
        def(
            "all_weap_inf_thermaldetonator_ord",
            "ordc",
            "sticky",
            &[("Velocity", "25.0")],
        ),
    ] {
        let mut after = d.clone();
        apply(&mut after);
        assert_eq!(after, d, "{}", d.name);
    }
}

#[test]
fn key_rows_set_or_add_their_key() {
    let mut sniper = def(
        "imp_weap_inf_sniperrifle",
        "wpnc",
        "cannon",
        &[("ShotDelay", "0.5"), ("RoundsPerClip", "6")],
    );
    apply(&mut sniper);
    assert_eq!(all(&sniper, "ShotDelay"), [1.0]);
    assert_eq!(all(&sniper, "RoundsPerClip"), [6.0]);

    // The Imperial rifle alone takes BF2's faster rate.
    let mut rifle = def(
        "imp_weap_inf_rifle",
        "wpnc",
        "cannon",
        &[("ShotDelay", "0.2"), ("RoundsPerClip", "55")],
    );
    apply(&mut rifle);
    assert_eq!(all(&rifle, "ShotDelay"), [0.166]);
    assert_eq!(all(&rifle, "RoundsPerClip"), [55.0]);

    // The blast takes BF2's radii under whichever name it spells the outer.
    let mut blast = def(
        "wok_weap_inf_torpedo_launcher_exp",
        "expc",
        "explosion",
        &[("Damage", "400.0"), ("DamageRadius", "5.0")],
    );
    apply(&mut blast);
    assert_eq!(all(&blast, "DamageRadius"), [4.0]);
    assert!(all(&blast, "DamageRadiusOuter").is_empty());
    assert_eq!(all(&blast, "DamageRadiusInner"), [2.0]);

    let mut droid = def(
        "cis_weap_inf_remotedroid_ord",
        "entc",
        "droid",
        &[("MaxHealth", "40.0")],
    );
    apply(&mut droid);
    assert_eq!(all(&droid, "MaxHealth"), [100.0]);

    let mut station = def(
        "com_item_healthrecharge",
        "entc",
        "powerupstation",
        &[("MaxHealth", "80"), ("SoldierHealth", "15.0")],
    );
    apply(&mut station);
    assert_eq!(all(&station, "SoldierHealth"), [25.0]);
    assert_eq!(all(&station, "MaxHealth"), [80.0]);
}

#[test]
fn a_pickup_heals_the_same_share_of_a_trooper() {
    let mut bacta = def(
        "com_item_powerup_health100",
        "entc",
        "powerupitem",
        &[("SoldierHealth", "75"), ("LifeSpan", "30")],
    );
    apply(&mut bacta);
    assert_eq!(all(&bacta, "SoldierHealth"), [100.0]);
    assert_eq!(all(&bacta, "LifeSpan"), [30.0]);
    let mut ammo = def(
        "com_item_powerup_ammo",
        "entc",
        "powerupitem",
        &[("SoldierAmmo", "1")],
    );
    apply(&mut ammo);
    assert!(all(&ammo, "SoldierHealth").is_empty());
}

#[test]
fn the_tables_name_each_class_once() {
    let mut seen: Vec<&str> = Vec::new();
    for class in HEALTH.iter().flat_map(|r| r.classes) {
        assert!(!seen.contains(class), "{class} twice");
        seen.push(class);
    }
    for (i, a) in PERSON_DAMAGE.iter().enumerate() {
        for b in &PERSON_DAMAGE[i + 1..] {
            assert_ne!(a.class, b.class);
        }
    }
}
