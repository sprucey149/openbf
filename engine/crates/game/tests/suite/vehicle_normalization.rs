//! Tier-1 (data-gated): the vehicle pass (`game::normalize::vehicles`,
//! `analysis/vehicle-normalization.md`). A BF1 vehicle or turret and its
//! guns play as their BF2 twin's on every map; vehicles only one game has
//! keep their place by a ratio; infantry weapons hit vehicles as BF2's do.

use super::normalization::{
    all_classes, desc, dir, files, hits_to_kill, map_classes, sorted_names, to_person, SIDES,
};
use core::weapon::{DamageDesc, HealthType, WeaponClass};
use game::classdb::ClassDb;
use game::normalize::{vehicles, BF1_SCALE, REFERENCE_HEALTH};
use game::roster::{load_classdb, weapon_from_db, ClassRoster};

/// BF1's classes as a BF1 map plays them: its own class file, then the
/// side files.
fn bf1_map(key: &str) -> Option<ClassDb> {
    let map_dir = game::assets::baked_root().join(key);
    if !map_dir.is_dir() {
        eprintln!("SKIP: {key} not baked");
        return None;
    }
    let mut db = ClassDb::default();
    db.load_map(&map_dir).unwrap();
    for f in files("bf1") {
        db.load(&f).unwrap();
    }
    Some(db)
}

/// The BF1 maps whose own class files hold a vehicle or turret of the pass.
const MAPS: [&str; 6] = [
    "bf1_bes1", "bf1_geo1", "bf1_hot1", "bf1_yav1", "bf1_tat2", "tat1",
];

/// Every BF1 class db the twin rows can meet: the side files alone, then
/// each map with its own classes.
fn bf1_dbs() -> Vec<(String, ClassDb)> {
    let mut out = vec![("sides".to_owned(), all_classes("bf1", false))];
    for key in MAPS {
        if let Some(db) = bf1_map(key) {
            out.push((key.to_owned(), db));
        }
    }
    out
}

fn explosion(w: &WeaponClass) -> Option<DamageDesc> {
    w.ordnance
        .explosion
        .as_ref()
        .map(|e| e.desc())
        .filter(|d| d.damage > 0.0)
}

fn bf2_ground_gun(db: &ClassDb, name: &str) -> WeaponClass {
    if name == "cis_weap_tread_hailfire_chaingun" {
        let mut sim = core::sim::Sim::new(42);
        let seats = game::vehicle_class::seats(db, "cis_tread_hailfire");
        let gun = game::vehicle_pads::seat_gun(&mut sim, db, &seats[0], &[]).unwrap();
        return sim.weapon_classes[gun.weapons[0].class].clone();
    }
    weapon_from_db(db, name).unwrap()
}

/// What a gun does: the weapon's fire, the round's damage and the blast's,
/// as the class loader builds them.
fn assert_same_gun(name: &str, a: &WeaponClass, b: &WeaponClass) {
    let fire = |w: &WeaponClass| {
        (
            w.shot_delay,
            w.salvo_count,
            w.shots_per_salvo,
            w.salvo_delay,
            w.initial_salvo_delay,
            w.heat_per_shot,
            w.heat_recover_rate,
            w.heat_threshold,
            w.rounds_per_clip,
            w.reload_time,
            w.max_pressed_time,
        )
    };
    assert_eq!(fire(a), fire(b), "{name}: fire");
    let (o, p) = (&a.ordnance, &b.ordnance);
    let fat = |o: &core::weapon::OrdnanceClass| o.fat_ray.as_ref().map(|f| f.max_damage);
    if o.fat_ray.is_some() {
        assert_eq!(fat(o), fat(p), "{name}: ray");
    } else {
        assert_eq!(o.max_damage, p.max_damage, "{name}: damage");
    }
    assert_eq!(o.scales, p.scales, "{name}: scales");
    assert_eq!(o.shield_scale, p.shield_scale, "{name}: shield scale");
    assert_eq!(o.falloff, p.falloff, "{name}: fall-off");
    let knots = |o: &core::weapon::OrdnanceClass| {
        o.knots
            .iter()
            .map(|k| (k.at, k.damage, k.scales))
            .collect::<Vec<_>>()
    };
    assert_eq!(knots(o), knots(p), "{name}: charge points");
    if let (Some(m), Some(n)) = (&o.missile, &p.missile) {
        assert_eq!(m.turn_rate, n.turn_rate, "{name}: turn rate");
    }
    if let (Some(m), Some(n)) = (&o.beam, &p.beam) {
        assert_eq!(m.pierce, n.pierce, "{name}: pass-through");
    }
    let blast = |d: Option<DamageDesc>| d.map(|d| (d.damage, d.scales, d.shield_scale));
    assert_eq!(blast(explosion(a)), blast(explosion(b)), "{name}: blast");
}

/// V1, V4, V5: every twinned gun is its BF2 twin's, on the side files and
/// on each map with its own turrets.
#[test]
fn vehicle_twins_equal_on_both_maps() {
    if dir().is_none() {
        return;
    }
    let bf2 = all_classes("bf2", false);
    let mut guns = 0;
    for (key, db) in bf1_dbs() {
        for row in vehicles::TWINS {
            let (Some(def), Some(twin)) = (db.get(row.class), row.twin) else {
                continue;
            };
            if def.kind != "wpnc" {
                continue;
            }
            let a = weapon_from_db(&db, row.class).unwrap();
            let b = bf2_ground_gun(&bf2, twin);
            assert_same_gun(&format!("{key}: {} / {twin}", row.class), &a, &b);
            guns += 1;
        }
        for row in vehicles::HEALTH {
            let (Some(_), Some(twin)) = (db.get(row.class), row.twin) else {
                continue;
            };
            assert_eq!(
                db.prop_f32(row.class, "MaxHealth"),
                bf2.prop_f32(twin, "MaxHealth"),
                "{key}: {}",
                row.class
            );
        }
    }
    assert!(guns >= 26, "{guns} twinned guns checked");
}

/// Every twin row names a BF2 class that exists, and every twinned BF1
/// class is on some bake.
#[test]
fn every_twin_row_names_real_classes() {
    if dir().is_none() {
        return;
    }
    let bf2 = all_classes("bf2", false);
    let dbs = bf1_dbs();
    for row in vehicles::TWINS {
        if let Some(twin) = row.twin {
            assert!(bf2.get(twin).is_some(), "BF2 {twin}");
        }
        if row.class.starts_with('*') {
            continue;
        }
        assert!(
            dbs.iter().any(|(_, db)| db.get(row.class).is_some()),
            "BF1 {}",
            row.class
        );
    }
    for row in vehicles::HEALTH {
        if let Some(twin) = row.twin {
            assert!(bf2.prop_f32(twin, "MaxHealth").is_some(), "BF2 {twin}");
        }
    }
}

/// V2: a vehicle only BF1 has keeps its place against its twin family by
/// the ratio, and its guns do x8000/6600 to vehicles and x4/3 to soldiers.
#[test]
fn one_game_vehicles_follow_their_ratio() {
    if dir().is_none() {
        return;
    }
    let mut checked = 0;
    let mut dbs: Vec<(String, ClassDb, ClassDb)> = vec![(
        "sides".to_owned(),
        all_classes("bf1", false),
        all_classes("bf1", true),
    )];
    dbs.extend(map_classes("bf1"));
    for (key, now, disc) in &dbs {
        for row in vehicles::HEALTH.iter().filter(|r| r.twin.is_none()) {
            let Some(before) = disc.prop_f32(row.class, "MaxHealth") else {
                continue;
            };
            assert_eq!(
                now.prop_f32(row.class, "MaxHealth"),
                Some(row.health),
                "{key}: {}",
                row.class
            );
            assert!(
                (before * row.ratio - row.health).abs() <= 0.5,
                "{}: {before} x {} is not {}",
                row.class,
                row.ratio,
                row.health
            );
            checked += 1;
        }
        for name in sorted_names(disc) {
            if !vehicles::VEHICLE_FACTOR_CLASSES
                .iter()
                .any(|c| super::normalization::pattern_matches(c, &name))
            {
                continue;
            }
            let (a, b) = (desc(disc, &name), desc(now, &name));
            let v = HealthType::Vehicle as usize;
            assert!(
                (b.damage * b.scales[v] - a.damage * a.scales[v] * vehicles::VEHICLE_FACTOR).abs()
                    < 1e-2,
                "{key}: {name} to a vehicle"
            );
            let p = HealthType::Person as usize;
            assert!(
                (b.damage * b.scales[p] - a.damage * a.scales[p] * BF1_SCALE).abs() < 1e-2,
                "{key}: {name} to a soldier"
            );
            // A standard tank takes as many hits as BF1's did on the disc
            // (a weak round may need one more: rounding in the running
            // health, as for the soldier scale).
            if a.damage * a.scales[v] > 0.0 {
                let (now, then) = (
                    hits_to_kill(&b, 8000.0, HealthType::Vehicle),
                    hits_to_kill(&a, 6600.0, HealthType::Vehicle),
                );
                assert!(
                    now == then || (then > 12 && now == then + 1),
                    "{key}: {name} on a tank: {then} -> {now}"
                );
            }
        }
    }
    // MTT, hovernaut, skiff, landspeeder and Bespin's AAA turret on this
    // bake (the Gian speeder is in none).
    if game::assets::baked_root().join("bf1_bes1").is_dir() {
        assert!(checked >= 5, "{checked} one-game vehicles checked");
    }
}

/// V3: an infantry weapon does to vehicles and buildings what its BF2 twin
/// does.
#[test]
fn infantry_weapons_hit_vehicles_as_bf2s() {
    if dir().is_none() {
        return;
    }
    let (bf1, bf2) = (all_classes("bf1", false), all_classes("bf2", false));
    let (v, b) = (HealthType::Vehicle as usize, HealthType::Building as usize);
    let hard = |db: &ClassDb, class: &str| {
        let d = desc(db, class);
        (d.damage * d.scales[v], d.damage * d.scales[b])
    };
    for (one, two) in [
        (
            "all_weap_inf_torpedo_launcher_ord",
            "all_weap_inf_rocket_launcher_ord",
        ),
        (
            "imp_weap_inf_torpedo_launcher_ord",
            "imp_weap_inf_rocket_launcher_ord",
        ),
        (
            "all_weap_inf_thermaldetonator_exp",
            "all_weap_inf_thermaldetonator_exp",
        ),
        ("com_weap_inf_landmine_exp", "com_weap_inf_landmine_exp"),
        (
            "all_weap_inf_remote_charge_dispenser_exp",
            "all_weap_inf_detpack_exp",
        ),
        (
            "imp_weap_inf_mortar_launcher_ord",
            "imp_weap_inf_mortar_launcher_ord1",
        ),
        (
            "imp_weap_inf_mortar_launcher_ord1",
            "imp_weap_inf_mortar_launcher_ord",
        ),
        (
            "cis_weap_inf_mortar_launcher_ord1",
            "cis_weap_inf_mortar_launcher_ord",
        ),
        (
            "cis_weap_inf_mortar_launcher_exp",
            "cis_weap_inf_mortar_launcher_exp",
        ),
        ("all_weap_inf_rifle_ord", "all_weap_inf_rifle_ord"),
        ("imp_weap_inf_pistol_ord", "imp_weap_inf_pistol_ord"),
        (
            "rep_weap_inf_sniperrifle_ord",
            "rep_weap_inf_sniper_rifle_ord",
        ),
        ("all_weap_inf_shotgun_ord", "all_weap_inf_shotgun_ord"),
        ("all_weap_inf_bowcaster_ord", "all_weap_inf_bowcaster_ord"),
    ] {
        let (a, c) = (hard(&bf1, one), hard(&bf2, two));
        // A blaster's damage itself is each game's (80 and 60 a bolt);
        // its scales are BF2's.
        let blaster = one.ends_with("rifle_ord")
            || one.ends_with("pistol_ord")
            || one.ends_with("shotgun_ord")
            || one.ends_with("bowcaster_ord");
        if blaster {
            let (d1, d2) = (desc(&bf1, one), desc(&bf2, two));
            assert_eq!(d1.scales[v], d2.scales[v], "{one} vehicle scale");
            assert_eq!(d1.scales[b], d2.scales[b], "{one} building scale");
        } else {
            assert_eq!(a, c, "{one} / {two}");
        }
    }
    // The rocket rises from 500 to 1500 by age on both maps: a BF2 tank
    // (8000) takes 16 at the muzzle and 6 from half a second of flight.
    let rocket = |db: &ClassDb, weapon: &str, age: f32| {
        weapon_from_db(db, weapon)
            .unwrap()
            .ordnance
            .damage_desc(age, false)
    };
    for (db, weapon) in [
        (&bf1, "all_weap_inf_torpedo_launcher"),
        (&bf2, "all_weap_inf_rocket_launcher"),
    ] {
        assert_eq!(
            hits_to_kill(&rocket(db, weapon, 0.0), 8000.0, HealthType::Vehicle),
            16,
            "{weapon}"
        );
        assert_eq!(
            hits_to_kill(&rocket(db, weapon, 0.5), 8000.0, HealthType::Vehicle),
            6,
            "{weapon}"
        );
    }
    // BF1-only grenades: x8000/6600 to vehicles, so a tank takes as many.
    let disc = all_classes("bf1", true);
    for class in [
        "imp_weap_inf_crackgrenade_exp",
        "rep_weap_inf_crackgrenade_exp",
    ] {
        assert_eq!(
            hits_to_kill(&desc(&bf1, class), 8000.0, HealthType::Vehicle),
            hits_to_kill(&desc(&disc, class), 6600.0, HealthType::Vehicle),
            "{class}"
        );
    }
}

/// V1 against soldiers: a twinned gun kills a trooper and a heavy in as
/// many hits on a BF1 map as on a BF2 map; a starfighter gun keeps BF1's
/// count under the x4/3.
#[test]
fn twinned_guns_kill_soldiers_as_their_twins() {
    let Some(dir) = dir() else { return };
    let bf1 = load_classdb(&dir, "bf1", &SIDES).unwrap();
    let bf2 = all_classes("bf2", false);
    let body = |db: &ClassDb, class: &str| {
        let c = ClassRoster::build(db, class, 0, None).unwrap();
        (c.max_health, c.health_type)
    };
    let (trooper, heavy) = (
        body(&bf1, "all_inf_soldier"),
        body(&bf1, "all_inf_vanguard"),
    );
    assert_eq!(trooper.0, REFERENCE_HEALTH);
    assert_eq!(heavy.0, 550.0);
    let mut checked = 0;
    for row in vehicles::TWINS {
        let (Some(def), Some(twin)) = (bf1.get(row.class), row.twin) else {
            continue;
        };
        if def.kind != "wpnc" {
            continue;
        }
        let a = weapon_from_db(&bf1, row.class)
            .unwrap()
            .ordnance
            .damage_desc(0.0, false);
        let b = bf2_ground_gun(&bf2, twin).ordnance.damage_desc(0.0, false);
        if a.damage <= 0.0 {
            continue;
        }
        for (h, t) in [trooper, heavy] {
            assert_eq!(
                hits_to_kill(&a, h, t),
                hits_to_kill(&b, h, t),
                "{} on {h}",
                row.class
            );
        }
        checked += 1;
    }
    assert!(checked >= 20, "{checked} guns checked");
    // A light cannon now takes two hits on a trooper at its muzzle, as
    // BF2's (200 a bolt); a starfighter's cannon still kills in one.
    let now = all_classes("bf1", false);
    assert_eq!(
        to_person(&now, "imp_weap_hover_fightertank_cannon_ord"),
        200.0
    );
    assert_eq!(to_person(&now, "rep_weap_fly_gunship_cannon_ord"), 400.0);
    assert_eq!(
        hits_to_kill(
            &desc(&now, "rep_weap_fly_gunship_cannon_ord"),
            trooper.0,
            trooper.1
        ),
        1
    );
}

/// V4: the turrets' health and guns, and the Geonosis turret's ray, which
/// reads only `MaxDamage` (BF1's authored `Damage` alone and fell to the
/// ray's default 20).
#[test]
fn turret_twins_play_as_bf2s() {
    if dir().is_none() {
        return;
    }
    let bf2 = all_classes("bf2", false);
    for (key, turret, twin) in [
        ("sides", "com_weap_gunturret", "tur_bldg_laser"),
        ("bf1_geo1", "geo_bldg_geoturret", "tur_bldg_geoturret"),
        (
            "bf1_hot1",
            "hoth_bldg_dish_turret",
            "tur_bldg_hoth_dishturret",
        ),
        (
            "bf1_hot1",
            "hoth_bldg_hoth_turret",
            "tur_bldg_hoth_lasermortar",
        ),
        ("bf1_yav1", "yav_bldg_tower_turret", "tur_bldg_tower"),
        ("bf1_tat2", "tat_bldg_barge_turret", "tur_bldg_tat_barge"),
    ] {
        let db = if key == "sides" {
            Some(all_classes("bf1", false))
        } else {
            bf1_map(key)
        };
        let Some(db) = db else { continue };
        assert_eq!(
            db.prop_f32(turret, "MaxHealth"),
            bf2.prop_f32(twin, "MaxHealth"),
            "{turret}"
        );
        let gun = db.prop(turret, "WeaponName").unwrap();
        let other = bf2.prop(twin, "WeaponName").unwrap();
        let (a, b) = (
            weapon_from_db(&db, gun).unwrap(),
            weapon_from_db(&bf2, other).unwrap(),
        );
        assert_same_gun(turret, &a, &b);
        if key == "bf1_geo1" {
            let ray = a.ordnance.fat_ray.as_ref().expect("a fat ray");
            assert_eq!(ray.max_damage, 400.0);
        }
    }
}

#[test]
fn shared_classes_keep_separate_damage_rules() {
    if dir().is_none() {
        return;
    }
    let bf1 = all_classes("bf1", false);
    let bf2 = all_classes("bf2", false);
    let gun = |owner: &str, slot: usize| {
        let seats = game::vehicle_class::seats(&bf1, owner);
        weapon_from_db(&bf1, &seats[0].weapons[slot].class).unwrap()
    };
    let hailfire = gun("cis_tread_hailfire", 1);
    let spider = gun("cis_walk_spider", 1);
    assert_same_gun(
        "Hailfire",
        &hailfire,
        &bf2_ground_gun(&bf2, "cis_weap_tread_hailfire_chaingun"),
    );
    assert_eq!(hailfire.ordnance.max_damage, 50.0);
    assert_eq!(spider.ordnance.max_damage, 250.0);
    assert_eq!(hailfire.ordnance.velocity, spider.ordnance.velocity);
    let mut space = core::sim::Sim::new(42);
    space.install_space_assault(core::sim::SpaceAssault::new(180));
    let seats = game::vehicle_class::seats(&bf2, "cis_tread_hailfire");
    let space_gun = game::vehicle_pads::seat_gun(&mut space, &bf2, &seats[0], &[]).unwrap();
    assert_eq!(
        space.weapon_classes[space_gun.weapons[0].class]
            .ordnance
            .max_damage,
        0.0
    );

    let mtt = gun("cis_hover_mtt", 0);
    let maf = weapon_from_db(&bf1, "cis_weap_fly_maf_cannon").unwrap();
    let vehicle = HealthType::Vehicle as usize;
    assert_eq!(maf.ordnance.scales[vehicle], 1.0);
    assert_eq!(mtt.ordnance.scales[vehicle], vehicles::VEHICLE_FACTOR);
    assert_eq!(mtt.ordnance.max_damage, maf.ordnance.max_damage);
    assert_eq!(mtt.ordnance.laser, maf.ordnance.laser);

    let turret = weapon_from_db(&bf1, "com_weap_bldg_gunturret").unwrap();
    let impact = turret.ordnance.explosion.unwrap();
    let body = game::vehicle_pads::blast_setup(&bf1, "com_weap_gunturret", None)
        .explosion
        .unwrap();
    assert_eq!(impact.damage, 0.0);
    assert_eq!(impact.look, body.look);
    assert_eq!(
        body.damage * body.scales[HealthType::Person as usize],
        25.0 * BF1_SCALE
    );
}

/// Space rounds play BF2's data as written: no twin row and no health row
/// changes a BF2 class (`normalization::bf2_class_files_are_untouched`
/// covers the files; this covers the tables).
#[test]
fn the_pass_never_reaches_bf2_data() {
    if dir().is_none() {
        return;
    }
    let (now, disc) = (all_classes("bf2", false), all_classes("bf2", true));
    for row in vehicles::TWINS {
        if let Some(def) = disc.get(row.class) {
            assert_eq!(now.get(row.class), Some(def), "{}", row.class);
        }
    }
    for row in vehicles::HEALTH {
        if let Some(def) = disc.get(row.class) {
            assert_eq!(now.get(row.class), Some(def), "{}", row.class);
        }
    }
}
