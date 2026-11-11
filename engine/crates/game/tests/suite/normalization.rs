//! Tier-1 (data-gated): one set of soldier health and damage numbers on
//! both games' maps (`game::normalize`, `analysis/gameplay-normalization.md`).
//! `OPENBF_CLASSES=<dir>` points the tests at another class bake (one with
//! the native sides, which not every bake carries).

use core::weapon::{DamageDesc, ExplosionClass, Health, HealthType, OrdnanceClass};
use game::classdb::{fnv1a_lowercase, ClassDb, ClassDef};
use game::normalize::{
    health_row, is_infantry_damage, BF1_SCALE, HEALTH, KEYS, PERSON_DAMAGE, REFERENCE_HEALTH,
};
use game::roster::{load_classdb, weapon_from_db, ClassRoster, ClassSrc};
use std::path::PathBuf;

const SIDES: [&str; 4] = ["all", "imp", "rep", "cis"];

fn dir() -> Option<PathBuf> {
    let dir = std::env::var_os("OPENBF_CLASSES")
        .map(PathBuf::from)
        .unwrap_or_else(|| game::assets::baked_root().join("classes"));
    if dir.join("bf1_all.ron").is_file() && dir.join("bf2_all.ron").is_file() {
        Some(dir)
    } else {
        eprintln!("SKIP: class bakes absent");
        None
    }
}

/// Every class file of one game, the shared one last (a side's own
/// definition of a name wins).
fn files(game: &str) -> Vec<PathBuf> {
    let Some(dir) = dir() else { return Vec::new() };
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            name.starts_with(&format!("{game}_")) && name.ends_with(".ron")
        })
        .collect();
    let shared = |p: &PathBuf| {
        let name = p.file_name().unwrap().to_str().unwrap();
        name == "bf1_common.ron" || name == "bf2_ingame.ron"
    };
    files.sort_by_key(|p| (shared(p), p.clone()));
    files
}

/// One game's whole class data: as the game plays it, or as the disc wrote it.
fn all_classes(game: &str, disc: bool) -> ClassDb {
    let mut db = if disc {
        ClassDb::disc()
    } else {
        ClassDb::default()
    };
    for f in files(game) {
        db.load(&f).unwrap();
    }
    db
}

fn sorted_names(db: &ClassDb) -> Vec<String> {
    let mut names: Vec<String> = db.names().map(str::to_owned).collect();
    names.sort();
    names
}

fn is_soldier(def: &ClassDef) -> bool {
    def.kind == "entc"
        && (def.base.eq_ignore_ascii_case("soldier")
            || def.base.eq_ignore_ascii_case("walkerdroid"))
}

fn authors_damage(db: &ClassDb, class: &str) -> bool {
    ["MaxDamage", "Damage"]
        .iter()
        .any(|k| db.prop_f32(class, k).is_some_and(|d| d > 0.0))
}

/// What a hit of an ordnance or explosion class deals, as the sim applies
/// it: BF1's emitter ignores the class's scales.
fn desc(db: &ClassDb, class: &str) -> DamageDesc {
    let class = db.resolve(class).unwrap_or(class);
    let src = ClassSrc { db, class };
    if db.get(class).is_some_and(|d| d.kind == "expc") {
        return ExplosionClass::from_props(&src).desc();
    }
    let o = OrdnanceClass::from_props(&src);
    let emitter = db.engine_base(class) == Some("emitterordnance") && !db.is_bf2();
    DamageDesc {
        damage: o.max_damage,
        scales: if emitter { [1.0; 7] } else { o.scales },
        shield_scale: if emitter { 1.0 } else { o.shield_scale },
    }
}

fn to_person(db: &ClassDb, class: &str) -> f32 {
    let d = desc(db, class);
    d.damage * d.scales[HealthType::Person as usize]
}

fn hits_to_kill(desc: &DamageDesc, health: f32, health_type: HealthType) -> u32 {
    let mut h = Health::new(health, health_type);
    let mut n = 0;
    while h.alive() {
        assert!(h.take_damage(desc, 1.0, false, 1.0) > 0.0, "no damage");
        n += 1;
    }
    n
}

/// The one factor a damage class's hits on soldiers, animals, droids and
/// shields moved by (1.0 when it deals them nothing); vehicles and
/// buildings take what they took.
fn soft_factor(disc: &ClassDb, now: &ClassDb, name: &str) -> f32 {
    let (d0, d1) = (desc(disc, name), desc(now, name));
    let emitter = disc
        .get(name)
        .unwrap()
        .base
        .eq_ignore_ascii_case("emitterordnance");
    if emitter {
        // BF1's emitter moves its damage itself; only the arc caster is one.
        assert!(is_infantry_damage(name), "{name} is an emitter");
    } else {
        assert_eq!(d0.damage, d1.damage, "{name} damage");
        for i in 3..7 {
            assert_eq!(d0.scales[i], d1.scales[i], "{name} scale {i}");
        }
    }
    let soft = |d: &DamageDesc| {
        [
            d.damage * d.scales[0],
            d.damage * d.scales[1],
            d.damage * d.scales[2],
            d.damage * d.shield_scale,
        ]
    };
    let mut k: Option<f32> = None;
    for (before, after) in soft(&d0).into_iter().zip(soft(&d1)) {
        if before == 0.0 {
            assert_eq!(after, 0.0, "{name}");
            continue;
        }
        let r = after / before;
        let k = *k.get_or_insert(r);
        assert!((r - k).abs() <= 1e-3 * k, "{name}: x{r} and x{k}");
    }
    k.unwrap_or(1.0)
}

/// Every baked map of `game` that has its own class file: (key, its
/// classes as played, as the disc wrote them).
fn map_classes(game: &str) -> Vec<(String, ClassDb, ClassDb)> {
    let mut out = Vec::new();
    for (key, source, map_dir) in game::assets::discover_maps(&game::assets::baked_root()) {
        if !source.eq_ignore_ascii_case(game) {
            continue;
        }
        let (mut now, mut disc) = (ClassDb::default(), ClassDb::disc());
        if now.load_map(&map_dir).unwrap() > 0 {
            disc.load_map(&map_dir).unwrap();
            out.push((key, now, disc));
        }
    }
    out
}

fn pattern_matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_prefix('*') {
        Some(tail) => name.to_ascii_lowercase().ends_with(tail),
        None => pattern.eq_ignore_ascii_case(name),
    }
}

#[test]
fn equivalent_classes_have_equal_health() {
    let Some(dir) = dir() else { return };
    let bf1 = load_classdb(&dir, "bf1", &SIDES).unwrap();
    let bf2 = load_classdb(&dir, "bf2", &SIDES).unwrap();
    for (one, two, health) in [
        ("all_inf_soldier", "all_inf_rifleman", 400.0),
        ("imp_inf_storm_trooper", "imp_inf_rifleman", 400.0),
        ("rep_inf_clone_trooper", "rep_inf_ep3_rifleman", 400.0),
        ("cis_inf_battledroid", "cis_inf_rifleman", 400.0),
        ("all_inf_vanguard", "all_inf_rocketeer", 550.0),
        ("imp_inf_shock_trooper", "imp_inf_rocketeer", 550.0),
        ("rep_inf_arc_trooper", "rep_inf_ep3_rocketeer", 550.0),
        ("cis_inf_assault", "cis_inf_rocketeer", 550.0),
        ("all_inf_marksman", "all_inf_sniper", 300.0),
        ("imp_inf_scout_trooper", "imp_inf_sniper", 300.0),
        ("rep_inf_clone_sharpshooter", "rep_inf_ep3_sniper", 300.0),
        ("cis_inf_assassindroid", "cis_inf_sniper", 300.0),
        ("all_inf_pilot", "all_inf_engineer", 400.0),
        ("imp_inf_pilottie", "imp_inf_engineer", 400.0),
        ("rep_inf_clone_pilot", "rep_inf_ep3_engineer", 400.0),
        ("cis_inf_pilotdroid", "cis_inf_engineer", 400.0),
        ("all_inf_smuggler", "all_inf_wookiee", 800.0),
        ("imp_inf_dark_trooper", "imp_inf_dark_trooper", 300.0),
        ("rep_inf_jet_trooper", "rep_inf_ep3_jettrooper", 300.0),
        ("cis_inf_droideka", "cis_inf_droideka", 550.0),
    ] {
        let a = ClassRoster::build(&bf1, one, 0, None).unwrap();
        let b = ClassRoster::build(&bf2, two, 0, None).unwrap();
        assert_eq!(a.max_health, health, "BF1 {one}");
        assert_eq!(b.max_health, health, "BF2 {two}");
    }
    // The natives, where the bake carries their sides.
    let (bf1, bf2) = (all_classes("bf1", false), all_classes("bf2", false));
    let mut natives = 0;
    for class in [
        "wok_inf_warrior",
        "ewk_inf_trooper",
        "geo_inf_geonosian",
        "tat_inf_tuskenraider",
        "tat_inf_tuskenhunter",
        "tat_inf_jawa",
        "gun_inf_soldier",
        "gun_inf_rider",
        "gun_inf_defender",
    ] {
        if let (Some(a), Some(b)) = (
            bf1.prop_f32(class, "MaxHealth"),
            bf2.prop_f32(class, "MaxHealth"),
        ) {
            assert_eq!(a, b, "{class}");
            natives += 1;
        }
    }
    eprintln!("{natives} native classes compared");
}

/// Each table row is BF2's own number, and every BF1 class it names lands
/// on it.
#[test]
fn the_tables_match_bf2_and_bf1_lands_on_them() {
    if dir().is_none() {
        return;
    }
    let (bf1, bf2) = (all_classes("bf1", false), all_classes("bf2", false));
    let disc = all_classes("bf1", true);
    let mut checked = 0;
    for row in HEALTH {
        if let Some(h) = bf2.prop_f32(row.twin, "MaxHealth") {
            assert_eq!(h, row.health, "BF2 {}", row.twin);
        }
        for class in row.classes {
            if bf1.get(class).is_some() {
                assert_eq!(
                    bf1.prop_f32(class, "MaxHealth"),
                    Some(row.health),
                    "{class}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 35, "{checked} BF1 soldier classes checked");

    let mut checked = 0;
    for row in PERSON_DAMAGE {
        if bf2.resolve(row.twin).is_some() {
            assert_eq!(to_person(&bf2, row.twin), row.person, "BF2 {}", row.twin);
        }
        for name in sorted_names(&bf1) {
            if pattern_matches(row.class, &name) {
                assert_eq!(to_person(&bf1, &name), row.person, "{name}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 25, "{checked} BF1 damage classes checked");

    let mut checked = 0;
    for row in KEYS {
        if bf2.resolve(row.twin).is_some() {
            let twin = row
                .keys
                .iter()
                .find_map(|k| bf2.prop_f32(row.twin, k))
                .unwrap_or_else(|| panic!("BF2 {} authors no {:?}", row.twin, row.keys));
            assert_eq!(twin, row.value, "BF2 {} {:?}", row.twin, row.keys);
        }
        let keys: Vec<u32> = row.keys.iter().map(|k| fnv1a_lowercase(k)).collect();
        for name in sorted_names(&bf1) {
            if !pattern_matches(row.class, &name) {
                continue;
            }
            let count = |db: &ClassDb| {
                db.get(&name)
                    .unwrap()
                    .props
                    .iter()
                    .filter(|p| keys.contains(&p.0))
                    .count()
            };
            // One value to set: a key written twice would hide a charge stage.
            assert!(count(&disc) <= 1, "{name} writes {:?} twice", row.keys);
            assert_eq!(count(&bf1), 1, "{name} {:?}", row.keys);
            let now = row.keys.iter().find_map(|k| bf1.prop_f32(&name, k));
            assert_eq!(now, Some(row.value), "{name} {:?}", row.keys);
            checked += 1;
        }
    }
    assert!(checked >= 15, "{checked} BF1 key rows checked");
}

#[test]
fn equivalent_weapons_deal_equal_person_damage() {
    if dir().is_none() {
        return;
    }
    let (bf1, bf2) = (all_classes("bf1", false), all_classes("bf2", false));
    for (one, two, damage) in [
        ("all_weap_inf_rifle_ord", "all_weap_inf_rifle_ord", 80.0),
        ("rep_weap_inf_rifle_ord", "rep_weap_inf_rifle_ord", 80.0),
        ("cis_weap_inf_rifle_ord", "cis_weap_inf_rifle_ord", 80.0),
        // The Imperial rifle is the lighter, faster one on both.
        ("imp_weap_inf_rifle_ord", "imp_weap_inf_rifle_ord", 70.0),
        ("all_weap_inf_pistol_ord", "all_weap_inf_pistol_ord", 60.0),
        (
            "imp_weap_inf_sniperrifle_ord",
            "imp_weap_inf_sniper_rifle_ord",
            300.0,
        ),
        (
            "rep_weap_inf_thermaldetonator_exp",
            "rep_weap_inf_thermaldetonator_exp",
            450.0,
        ),
        (
            "cis_weap_inf_torpedo_launcher_exp",
            "cis_weap_inf_rocket_launcher_exp",
            400.0,
        ),
        (
            "imp_weap_inf_mortar_launcher_exp",
            "imp_weap_inf_mortar_launcher_exp",
            400.0,
        ),
        (
            "all_weap_inf_remotedroid_blaster_ord",
            "all_weap_inf_remotedroid_blaster_ord",
            60.0,
        ),
    ] {
        assert_eq!(to_person(&bf1, one), damage, "BF1 {one}");
        assert_eq!(to_person(&bf2, two), damage, "BF2 {two}");
    }
    // A sniper rifle, each side's rifle and a rocket blast are the same
    // weapon whole: rate and radius too.
    for (one, two, key) in [
        ("all_weap_inf_rifle", "all_weap_inf_rifle", "ShotDelay"),
        ("rep_weap_inf_rifle", "rep_weap_inf_rifle", "ShotDelay"),
        ("cis_weap_inf_rifle", "cis_weap_inf_rifle", "ShotDelay"),
        ("imp_weap_inf_rifle", "imp_weap_inf_rifle", "ShotDelay"),
        (
            "all_weap_inf_sniperrifle",
            "all_weap_inf_sniper_rifle",
            "ShotDelay",
        ),
        (
            "all_weap_inf_torpedo_launcher_exp",
            "all_weap_inf_rocket_launcher_exp",
            "DamageRadiusInner",
        ),
        (
            "all_weap_inf_torpedo_launcher_exp",
            "all_weap_inf_rocket_launcher_exp",
            "DamageRadiusOuter",
        ),
    ] {
        assert_eq!(
            bf1.prop_f32(one, key),
            bf2.prop_f32(two, key),
            "{one} {key}"
        );
    }
    // A health droid heals at one rate, and a recon droid is as tough.
    for (class, key) in [
        ("com_item_healthrecharge", "SoldierHealth"),
        ("imp_weap_inf_remotedroid_ord", "MaxHealth"),
    ] {
        assert_eq!(
            bf1.prop_f32(class, key),
            bf2.prop_f32(class, key),
            "{class}"
        );
    }
}

#[test]
fn hits_to_kill_are_pinned() {
    let Some(dir) = dir() else { return };
    let bf1 = load_classdb(&dir, "bf1", &SIDES).unwrap();
    let bf2 = load_classdb(&dir, "bf2", &SIDES).unwrap();
    let shot = |db: &ClassDb, weapon: &str| {
        weapon_from_db(db, weapon)
            .unwrap()
            .ordnance
            .damage_desc(0.0, false)
    };
    let body = |db: &ClassDb, class: &str| {
        let c = ClassRoster::build(db, class, 0, None).unwrap();
        (c.max_health, c.health_type)
    };
    // (BF1 weapon, BF1 victim, BF2 weapon, BF2 victim, body hits) - the
    // same count on either game's map.
    for (w1, v1, w2, v2, hits) in [
        // Rifle on a trooper, then a heavy.
        (
            "all_weap_inf_rifle",
            "imp_inf_storm_trooper",
            "all_weap_inf_rifle",
            "imp_inf_rifleman",
            5,
        ),
        (
            "all_weap_inf_rifle",
            "imp_inf_shock_trooper",
            "all_weap_inf_rifle",
            "imp_inf_rocketeer",
            7,
        ),
        // The Imperial rifle (70 a bolt) on a trooper, then a heavy.
        (
            "imp_weap_inf_rifle",
            "all_inf_soldier",
            "imp_weap_inf_rifle",
            "all_inf_rifleman",
            6,
        ),
        (
            "imp_weap_inf_rifle",
            "all_inf_vanguard",
            "imp_weap_inf_rifle",
            "all_inf_rocketeer",
            8,
        ),
        (
            "rep_weap_inf_rifle",
            "cis_inf_battledroid",
            "rep_weap_inf_rifle",
            "cis_inf_rifleman",
            5,
        ),
        // Pistol on a trooper.
        (
            "all_weap_inf_pistol",
            "imp_inf_storm_trooper",
            "all_weap_inf_pistol",
            "imp_inf_rifleman",
            7,
        ),
        // Sniper rifle on a trooper, then a sniper.
        (
            "imp_weap_inf_sniperrifle",
            "all_inf_soldier",
            "imp_weap_inf_sniper_rifle",
            "all_inf_rifleman",
            2,
        ),
        (
            "imp_weap_inf_sniperrifle",
            "all_inf_marksman",
            "imp_weap_inf_sniper_rifle",
            "all_inf_sniper",
            1,
        ),
        // Shotgun pellets on a trooper.
        (
            "imp_weap_inf_shotgun",
            "all_inf_soldier",
            "imp_weap_inf_shotgun",
            "all_inf_rifleman",
            5,
        ),
    ] {
        let (h1, t1) = body(&bf1, v1);
        let (h2, t2) = body(&bf2, v2);
        if w2 != "imp_weap_inf_shotgun" {
            assert_eq!(
                hits_to_kill(&shot(&bf2, w2), h2, t2),
                hits,
                "BF2 {w2} on {v2}"
            );
        }
        assert_eq!(
            hits_to_kill(&shot(&bf1, w1), h1, t1),
            hits,
            "BF1 {w1} on {v1}"
        );
    }
    // The weapons BF1's droids take from BF2 meet a 400-health clone on
    // either map: 4 wrist-blaster hits, 5 repeater hits.
    for (class1, class2, hits) in [
        ("cis_inf_battledroid", "cis_inf_rifleman", 4),
        ("cis_inf_droideka", "cis_inf_droideka", 5),
    ] {
        let (h1, t1) = body(&bf1, "rep_inf_clone_trooper");
        let (h2, t2) = body(&bf2, "rep_inf_ep3_rifleman");
        let gun = |db: &ClassDb, class: &str| {
            ClassRoster::build(db, class, 0, None)
                .unwrap()
                .weapon
                .ordnance
                .damage_desc(0.0, false)
        };
        assert_eq!(
            hits_to_kill(&gun(&bf1, class1), h1, t1),
            hits,
            "BF1 {class1}"
        );
        assert_eq!(
            hits_to_kill(&gun(&bf2, class2), h2, t2),
            hits,
            "BF2 {class2}"
        );
    }
    // A thermal detonator at the centre kills a trooper and, as on BF2's
    // maps, not a heavy; a rocket's direct hit kills either.
    for (class, victim, hits) in [
        (
            "all_weap_inf_thermaldetonator_exp",
            "imp_inf_storm_trooper",
            1,
        ),
        (
            "all_weap_inf_thermaldetonator_exp",
            "imp_inf_shock_trooper",
            2,
        ),
        (
            "all_weap_inf_torpedo_launcher_ord",
            "imp_inf_shock_trooper",
            1,
        ),
    ] {
        let (h, t) = body(&bf1, victim);
        assert_eq!(
            hits_to_kill(&desc(&bf1, class), h, t),
            hits,
            "{class} on {victim}"
        );
    }
    let (h, t) = body(&bf2, "imp_inf_rocketeer");
    assert_eq!(
        hits_to_kill(&desc(&bf2, "all_weap_inf_thermaldetonator_exp"), h, t),
        2
    );
    // The disc's own pairing, for the record: BF1's rifle took 5 too.
    let disc = all_classes("bf1", true);
    assert_eq!(
        hits_to_kill(
            &desc(&disc, "all_weap_inf_rifle_ord"),
            300.0,
            HealthType::Person
        ),
        5
    );
}

/// BF2 is the reference: none of its classes change, so no space unit or
/// weapon does.
#[test]
fn bf2_class_files_are_untouched() {
    if dir().is_none() {
        return;
    }
    let (now, disc) = (all_classes("bf2", false), all_classes("bf2", true));
    let names = sorted_names(&disc);
    assert!(names.len() > 900, "{} BF2 classes", names.len());
    assert_eq!(names, sorted_names(&now));
    for name in names {
        assert_eq!(now.get(&name), disc.get(&name), "{name}");
    }
}

/// Only soldiers, pickups, the rows of the key table and damage classes
/// change, and a damage class changes only against soldiers: an infantry
/// weapon by its row or the scale, every other gun and blast by the scale.
#[test]
fn bf1_changes_stay_inside_the_rules() {
    if dir().is_none() {
        return;
    }
    let (now, disc) = (all_classes("bf1", false), all_classes("bf1", true));
    assert_eq!(sorted_names(&now), sorted_names(&disc));
    let (mut soldiers, mut infantry, mut others) = (0, 0, 0);
    for name in sorted_names(&disc) {
        let (a, b) = (disc.get(&name).unwrap(), now.get(&name).unwrap());
        assert_eq!((&a.kind, &a.base), (&b.kind, &b.base), "{name}");
        // BF1 has no class-to-class inheritance: one definition is a class.
        assert!(disc.get(&a.base).is_none(), "{name} inherits {}", a.base);
        if a == b {
            continue;
        }
        let keyed = KEYS.iter().any(|r| pattern_matches(r.class, &name));
        if a.kind == "entc" {
            let item = a.base.eq_ignore_ascii_case("powerupitem");
            assert!(is_soldier(a) || item || keyed, "{name} changed");
            soldiers += usize::from(is_soldier(a));
            continue;
        }
        if a.kind == "wpnc" {
            assert!(keyed, "{name} changed");
            continue;
        }
        let k = soft_factor(&disc, &now, &name);
        assert!(k >= 1.0, "{name} x{k}");
        if is_infantry_damage(&name) {
            infantry += 1;
        } else {
            // The interim rule: the plain scale, no row.
            assert!((k - BF1_SCALE).abs() <= 1e-3 || k == 1.0, "{name} x{k}");
            others += 1;
        }
    }
    assert!(soldiers >= 30, "{soldiers} soldier classes changed");
    assert!(infantry >= 60, "{infantry} infantry damage classes changed");
    assert!(others >= 100, "{others} other damage classes changed");
}

/// A map's own class file follows the map's game: a BF1 map's guns and
/// blasts take the interim scale, a BF2 map's file is read as written.
#[test]
fn a_maps_own_classes_follow_its_game() {
    if dir().is_none() {
        return;
    }
    let mut scaled = 0;
    for (key, now, disc) in map_classes("bf1") {
        assert_eq!(sorted_names(&now), sorted_names(&disc), "{key}");
        for name in sorted_names(&disc) {
            let (a, b) = (disc.get(&name).unwrap(), now.get(&name).unwrap());
            if a == b {
                continue;
            }
            assert!(
                matches!(a.kind.as_str(), "ordc" | "expc") && !is_infantry_damage(&name),
                "{key}: {name} changed"
            );
            let k = soft_factor(&disc, &now, &name);
            assert!((k - BF1_SCALE).abs() <= 1e-3 || k == 1.0, "{name} x{k}");
            scaled += 1;
        }
    }
    let bf2 = map_classes("bf2");
    for (key, now, disc) in &bf2 {
        let names = sorted_names(disc);
        assert_eq!(names, sorted_names(now), "{key}");
        for name in names {
            assert_eq!(now.get(&name), disc.get(&name), "{key}: {name}");
        }
    }
    eprintln!(
        "{scaled} BF1 map classes scaled, {} BF2 maps untouched",
        bf2.len()
    );
    if game::assets::baked_root().join("bf1_hot1").is_dir() {
        assert!(scaled >= 5, "{scaled} BF1 map classes scaled");
    }
}

/// The scale keeps the hit count: a BF1 gun or blast with no BF2 row kills
/// the shared trooper in as many hits as it killed BF1's.
#[test]
fn the_scale_keeps_every_hit_count_on_a_trooper() {
    if dir().is_none() {
        return;
    }
    let mut dbs = vec![(
        "sides".to_owned(),
        all_classes("bf1", false),
        all_classes("bf1", true),
    )];
    dbs.extend(map_classes("bf1"));
    let (mut checked, mut moved) = (0, Vec::new());
    for (key, now, disc) in &dbs {
        for name in sorted_names(disc) {
            let def = disc.get(&name).unwrap();
            if !matches!(def.kind.as_str(), "ordc" | "expc")
                || PERSON_DAMAGE
                    .iter()
                    .any(|r| pattern_matches(r.class, &name))
                || to_person(disc, &name) < 1.0
            {
                continue;
            }
            let before = hits_to_kill(&desc(disc, &name), 300.0, HealthType::Person);
            let after = hits_to_kill(&desc(now, &name), REFERENCE_HEALTH, HealthType::Person);
            // Rounding in the running health can cost a weak blast one
            // more hit (the 20-damage death blasts: 15 on the disc, 16 now).
            if after != before && (before <= 12 || after != before + 1) {
                moved.push(format!("{key}: {name} {before} -> {after}"));
            }
            checked += 1;
        }
    }
    assert!(moved.is_empty(), "{moved:#?}");
    assert!(checked >= 150, "{checked} classes checked");
}

/// INTERIM (replace: the vehicle pass): the guns of BF1's vehicles and
/// turrets against the shared soldiers.
#[test]
fn bf1_vehicle_and_turret_guns_are_pinned() {
    let Some(dir) = dir() else { return };
    let now = all_classes("bf1", false);
    let bf1 = load_classdb(&dir, "bf1", &SIDES).unwrap();
    let body = |class: &str| {
        let c = ClassRoster::build(&bf1, class, 0, None).unwrap();
        (c.max_health, c.health_type)
    };
    for (class, person, victim, hits) in [
        // The light cannons still kill a trooper in one.
        (
            "imp_weap_hover_fightertank_cannon_ord",
            400.0,
            "all_inf_soldier",
            1,
        ),
        (
            "rep_weap_hover_fightertank_cannon_ord",
            400.0,
            "cis_inf_battledroid",
            1,
        ),
        (
            "imp_weap_hover_speederbike_cannon_ord",
            400.0,
            "all_inf_soldier",
            1,
        ),
        (
            "cis_weap_hover_stap_cannon_ord",
            400.0,
            "rep_inf_clone_trooper",
            1,
        ),
        (
            "rep_weap_fly_gunship_cannon_ord",
            400.0,
            "cis_inf_battledroid",
            1,
        ),
        (
            "com_weap_bldg_gunturret_ord",
            400.0 / 3.0,
            "imp_inf_storm_trooper",
            3,
        ),
        // The AT-ST's chin gun still kills a heavy in one; the head guns
        // (400 on the disc) now need two on the 550-health heavy.
        (
            "imp_weap_walk_atst_cannon_chin1_ord",
            600.0,
            "all_inf_vanguard",
            1,
        ),
        (
            "imp_weap_walk_atst_cannon_head_ord",
            1600.0 / 3.0,
            "all_inf_soldier",
            1,
        ),
        (
            "imp_weap_walk_atst_cannon_head_ord",
            1600.0 / 3.0,
            "all_inf_vanguard",
            2,
        ),
    ] {
        let got = to_person(&now, class);
        assert!((got - person).abs() < 1e-3, "{class}: {got}");
        let (h, t) = body(victim);
        assert_eq!(
            hits_to_kill(&desc(&now, class), h, t),
            hits,
            "{class} on {victim}"
        );
    }
    // A map's own turret: Hoth's laser turret kills a trooper in one.
    for (key, map, _) in map_classes("bf1") {
        if key == "bf1_hot1" {
            let class = "hoth_weap_bldg_hothturret_cannon_ord";
            assert_eq!(to_person(&map, class), 400.0);
            let (h, t) = body("imp_inf_storm_troopersnow");
            assert_eq!(hits_to_kill(&desc(&map, class), h, t), 1);
        }
    }
}

/// Every BF1 soldier has a health row or is on the BF1-only list, and
/// every damage class it can reach is one the pass covers.
#[test]
fn every_bf1_soldier_and_its_weapons_are_covered() {
    if dir().is_none() {
        return;
    }
    let (now, disc) = (all_classes("bf1", false), all_classes("bf1", true));
    for name in sorted_names(&disc) {
        let def = disc.get(&name).unwrap();
        if !is_soldier(def) {
            continue;
        }
        let health = disc.prop_f32(&name, "MaxHealth").unwrap();
        if health >= 100_000.0 {
            // The unfielded stand-in heroes.
            assert_eq!(now.prop_f32(&name, "MaxHealth"), Some(health), "{name}");
            continue;
        }
        assert!(health_row(&name).is_some(), "{name} has no health row");
        // Follow every class a property names, weapons and ordnance alike.
        let mut seen = vec![name.clone()];
        let mut next = 0;
        while next < seen.len() {
            let class = seen[next].clone();
            next += 1;
            let d = disc.get(&class).unwrap();
            if matches!(d.kind.as_str(), "ordc" | "expc") && authors_damage(&disc, &class) {
                assert!(is_infantry_damage(&class), "{name} reaches {class}");
            }
            for (_, value) in &d.props {
                let word = value.split_whitespace().next().unwrap_or("");
                if let Some(found) = disc.resolve(word) {
                    if !seen.iter().any(|s| s == found) {
                        seen.push(found.to_owned());
                    }
                }
            }
        }
    }
}

#[test]
fn bf1_pickups_keep_their_share_of_a_trooper() {
    if dir().is_none() {
        return;
    }
    let (now, disc) = (all_classes("bf1", false), all_classes("bf1", true));
    let mut items = 0;
    for name in sorted_names(&disc) {
        if disc.engine_base(&name) != Some("powerupitem") {
            continue;
        }
        if let Some(h) = disc.prop_f32(&name, "SoldierHealth") {
            let after = now.prop_f32(&name, "SoldierHealth").unwrap();
            assert_eq!(after, h * BF1_SCALE, "{name}");
            assert!(
                (after / REFERENCE_HEALTH - h / 300.0).abs() < 1e-6,
                "{name}"
            );
            items += 1;
        }
    }
    assert_eq!(items, 3, "health25, health100 and the dual pack");
}

/// The before/after tables of `analysis/gameplay-normalization.md`:
/// `cargo test -p game --test suite -- normalization::print_table --ignored --nocapture`.
#[test]
#[ignore = "prints the note's tables"]
fn print_table() {
    if dir().is_none() {
        return;
    }
    let (now, disc, bf2) = (
        all_classes("bf1", false),
        all_classes("bf1", true),
        all_classes("bf2", false),
    );
    let n = |v: f32| format!("{}", (v * 100.0).round() / 100.0);
    let n3 = |v: f32| format!("{}", (v * 1000.0).round() / 1000.0);
    println!("\n### Soldier health\n");
    println!("| BF1 class | BF1 disc | Now | From |");
    println!("|---|---|---|---|");
    for name in sorted_names(&disc) {
        if !is_soldier(disc.get(&name).unwrap()) {
            continue;
        }
        let (a, b) = (
            disc.prop_f32(&name, "MaxHealth").unwrap(),
            now.prop_f32(&name, "MaxHealth").unwrap(),
        );
        let from = match health_row(&name) {
            Some(r) => format!("BF2 `{}`", r.twin),
            None if a == b => "left alone".into(),
            None => "x4/3".into(),
        };
        println!("| `{name}` | {} | {} | {from} |", n(a), n(b));
    }

    let rounds = |d: f32, health: f32| -> String {
        if d <= 0.0 {
            "-".into()
        } else {
            format!("{}", (health / d - 1e-4).ceil().max(1.0))
        }
    };
    println!("\n### BF1 infantry damage classes\n");
    println!("| BF1 class | To a person, disc | Hits on 300 | Now | Hits on 400 | Factor | From |");
    println!("|---|---|---|---|---|---|---|");
    for name in sorted_names(&disc) {
        let def = disc.get(&name).unwrap();
        if !matches!(def.kind.as_str(), "ordc" | "expc")
            || !is_infantry_damage(&name)
            || !authors_damage(&disc, &name)
        {
            continue;
        }
        let (a, b) = (to_person(&disc, &name), to_person(&now, &name));
        let (d0, d1) = (desc(&disc, &name), desc(&now, &name));
        let k = (d1.damage * d1.shield_scale) / (d0.damage * d0.shield_scale);
        let from = match PERSON_DAMAGE
            .iter()
            .find(|r| pattern_matches(r.class, &name))
        {
            Some(r) => format!("BF2 `{}`", r.twin),
            None => "x4/3".into(),
        };
        println!(
            "| `{name}` | {} | {} | {} | {} | {:.3} | {from} |",
            n(a),
            rounds(a, 300.0),
            n(b),
            rounds(b, REFERENCE_HEALTH),
            k
        );
    }

    println!("\n### BF1 vehicle, turret and blast classes (interim x4/3)\n");
    println!("| BF1 class | From | To a person, disc | Hits on 300 | Now | Hits on 400 |");
    println!("|---|---|---|---|---|---|");
    let mut dbs = vec![(
        "sides".to_owned(),
        all_classes("bf1", false),
        all_classes("bf1", true),
    )];
    dbs.extend(map_classes("bf1"));
    for (key, now, disc) in &dbs {
        for name in sorted_names(disc) {
            let def = disc.get(&name).unwrap();
            if !matches!(def.kind.as_str(), "ordc" | "expc")
                || is_infantry_damage(&name)
                || !authors_damage(disc, &name)
            {
                continue;
            }
            let (a, b) = (to_person(disc, &name), to_person(now, &name));
            println!(
                "| `{name}` | {key} | {} | {} | {} | {} |",
                n(a),
                rounds(a, 300.0),
                n(b),
                rounds(b, REFERENCE_HEALTH)
            );
        }
    }

    println!("\n### BF2 infantry damage classes (the reference, unchanged)\n");
    println!("| BF2 class | To a person | Hits on 400 |");
    println!("|---|---|---|");
    for name in sorted_names(&bf2) {
        let def = bf2.get(&name).unwrap();
        if !matches!(def.kind.as_str(), "ordc" | "expc")
            || !(is_infantry_damage(&name) || name.contains("_weap_award_"))
            || !authors_damage(&bf2, &name)
        {
            continue;
        }
        let a = to_person(&bf2, &name);
        println!("| `{name}` | {} | {} |", n(a), rounds(a, REFERENCE_HEALTH));
    }

    println!("\n### Keys taken from BF2\n");
    println!("| BF1 class | Key | BF1 disc | Now |");
    println!("|---|---|---|---|");
    for name in sorted_names(&disc) {
        for row in KEYS.iter().filter(|r| pattern_matches(r.class, &name)) {
            let before = row.keys.iter().find_map(|k| disc.prop_f32(&name, k));
            println!(
                "| `{name}` | {} | {} | {} |",
                row.keys[0],
                before.map_or("-".into(), n3),
                n3(row.value)
            );
        }
    }

    println!("\n### Pickups\n");
    println!("| BF1 item | Heals, disc | Now | BF2 |");
    println!("|---|---|---|---|");
    for name in sorted_names(&disc) {
        if let (Some("powerupitem"), Some(h)) = (
            disc.engine_base(&name),
            disc.prop_f32(&name, "SoldierHealth"),
        ) {
            println!(
                "| `{name}` | {} | {} | {} |",
                n(h),
                n(now.prop_f32(&name, "SoldierHealth").unwrap()),
                bf2.prop_f32(&name, "SoldierHealth").map_or("-".into(), n)
            );
        }
    }
}
