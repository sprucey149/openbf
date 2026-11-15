//! One set of soldier health and damage numbers for both games
//! (`analysis/gameplay-normalization.md`).
//!
//! [ours] BF2's numbers are the reference: a trooper has 400 health and a
//! rifle bolt does 80. BF1's trooper and rifle are exactly 3/4 of that
//! (300 and 60), so BF1's class data is rewritten as it is read
//! ([`ClassDb::load`](crate::classdb::ClassDb::load), `bf1_*` files, and
//! [`ClassDb::load_map`](crate::classdb::ClassDb::load_map) on a BF1 map) and
//! the same kind of soldier and weapon then behaves the same on every map.
//! BF2's files are touched only by [`SHARED_RULES`], which sets both games alike.
//!
//! The rules:
//! - a soldier class takes its BF2 counterpart's health ([`HEALTH`]); a
//!   class BF2 has no counterpart for keeps its strength relative to its own
//!   game's trooper ([`BF1_SCALE`]);
//! - an infantry weapon's damage to soldiers, animals, droids and shields
//!   takes its BF2 counterpart's ([`PERSON_DAMAGE`]), or else
//!   [`BF1_SCALE`];
//! - a few keys follow their BF2 counterpart outright ([`KEYS`]);
//! - the maintainer's own values for both games ([`SHARED_RULES`]): the one place a
//!   BF2 class is rewritten too;
//! - the vehicle pass ([`vehicles`]): vehicles and turrets take their BF2
//!   twin's health and guns, and infantry weapons their twin's damage to
//!   vehicles and buildings;
//! - every other BF1 damage class (starfighter guns, death blasts, droids;
//!   a BF1 map's own class file too, [`covers_map`]) scales its damage to
//!   soldiers by [`BF1_SCALE`], so a gun that killed BF1's trooper in one
//!   hit still does. replace: a flyer pass for the starfighters.
//!
//! measure: BF1-map win rate and kills a minute for the frozen stand-in
//! human against the pre-change baseline, and the same on the BF2 twin map.

use crate::classdb::{fnv1a_lowercase, ClassDef};

pub mod vehicles;

/// The reference trooper's health (BF2 `*_inf_rifleman`).
pub const REFERENCE_HEALTH: f32 = 400.0;

/// BF1's trooper health as authored.
pub const BF1_TROOPER_HEALTH: f32 = 300.0;

/// What a BF1 number with no BF2 counterpart is multiplied by.
pub const BF1_SCALE: f32 = REFERENCE_HEALTH / BF1_TROOPER_HEALTH;

/// BF1's stand-in heroes author 1000000 health; no mission fields them and
/// they are left alone.
const UNSCALED_HEALTH: f32 = 100_000.0;

/// Is `file` (a class file's name) one this pass rewrites?
pub fn covers(file: &str) -> bool {
    file.starts_with("bf1_")
}

/// Is a map of `source_game` one whose own class file this pass rewrites?
pub fn covers_map(source_game: &str) -> bool {
    source_game.eq_ignore_ascii_case("bf1")
}

/// Health a group of BF1 soldier classes takes, and the BF2 class it comes
/// from.
pub struct HealthRow {
    pub health: f32,
    pub twin: &'static str,
    pub classes: &'static [&'static str],
}

pub const HEALTH: &[HealthRow] = &[
    HealthRow {
        health: 400.0,
        twin: "all_inf_rifleman",
        classes: &[
            "all_inf_soldier",
            "all_inf_soldierdesert",
            "all_inf_soldierjungle",
            "all_inf_soldiersnow",
            "all_inf_soldierurban",
            "imp_inf_storm_trooper",
            "imp_inf_storm_troopersnow",
            "rep_inf_clone_trooper",
            "cis_inf_battledroid",
            "cis_inf_battledroid_inf",
        ],
    },
    HealthRow {
        health: 400.0,
        twin: "all_inf_engineer",
        classes: &[
            "all_inf_pilot",
            "imp_inf_pilotatat",
            "imp_inf_pilotatst",
            "imp_inf_pilottie",
            "rep_inf_clone_pilot",
            "cis_inf_pilotdroid",
            "cis_inf_battledroid_pilot",
        ],
    },
    HealthRow {
        health: 550.0,
        twin: "all_inf_rocketeer",
        classes: &[
            "all_inf_vanguard",
            "all_inf_vanguardjungle",
            "all_inf_vanguardsnow",
            "imp_inf_shock_trooper",
            "imp_inf_shock_troopersnow",
            "rep_inf_arc_trooper",
            "cis_inf_assault",
            "cis_inf_battledroid_av",
        ],
    },
    HealthRow {
        health: 300.0,
        twin: "all_inf_sniper",
        classes: &[
            "all_inf_marksman",
            "all_inf_marksmanjungle",
            "all_inf_marksmansnow",
            "imp_inf_scout_trooper",
            "imp_inf_scout_troopersnow",
            "rep_inf_clone_sharpshooter",
            "cis_inf_assassindroid",
            "cis_inf_battledroid_sniper",
        ],
    },
    HealthRow {
        health: 800.0,
        twin: "all_inf_wookiee",
        classes: &["all_inf_smuggler", "all_inf_smugglersnow"],
    },
    HealthRow {
        health: 300.0,
        twin: "imp_inf_dark_trooper",
        classes: &["imp_inf_dark_trooper", "imp_inf_dark_troopersnow"],
    },
    HealthRow {
        health: 300.0,
        twin: "rep_inf_ep3_jettrooper",
        classes: &["rep_inf_jet_trooper"],
    },
    HealthRow {
        health: 550.0,
        twin: "cis_inf_droideka",
        classes: &["cis_inf_droideka"],
    },
    HealthRow {
        health: 300.0,
        twin: "wok_inf_warrior",
        classes: &["wok_inf_warrior", "wok_inf_mechanic", "wok_inf_rocketeer"],
    },
    HealthRow {
        health: 200.0,
        twin: "ewk_inf_trooper",
        classes: &["ewk_inf_trooper", "ewk_inf_repair", "ewk_inf_scout"],
    },
    HealthRow {
        health: 125.0,
        twin: "geo_inf_geonosian",
        classes: &["geo_inf_geonosian"],
    },
    HealthRow {
        health: 400.0,
        twin: "tat_inf_tuskenraider",
        classes: &["tat_inf_tuskenraider"],
    },
    HealthRow {
        health: 300.0,
        twin: "tat_inf_tuskenhunter",
        classes: &["tat_inf_tuskenhunter"],
    },
    HealthRow {
        health: 250.0,
        twin: "tat_inf_jawa",
        classes: &["tat_inf_jawa"],
    },
    HealthRow {
        health: 200.0,
        twin: "gun_inf_soldier",
        classes: &["gun_inf_soldier", "gun_inf_rider", "gun_inf_defender"],
    },
];

/// Damage to a person a BF1 ordnance or explosion class takes, and the BF2
/// class it comes from. A `class` starting with `*` matches every side's.
pub struct DamageRow {
    pub class: &'static str,
    pub person: f32,
    pub twin: &'static str,
}

pub const PERSON_DAMAGE: &[DamageRow] = &[
    // The Imperial rifle is BF2's lighter, faster one ([`KEYS`]); the other
    // sides' land on BF2's 80 by the scale.
    DamageRow {
        class: "imp_weap_inf_rifle_ord",
        person: 70.0,
        twin: "imp_weap_inf_rifle_ord",
    },
    DamageRow {
        class: "*_weap_inf_pistol_ord",
        person: 60.0,
        twin: "all_weap_inf_pistol_ord",
    },
    DamageRow {
        class: "rep_weap_inf_commando_pistol_ord",
        person: 60.0,
        twin: "rep_weap_inf_commando_pistol_ord",
    },
    // [ours] 280, not BF2's 300 ([`SHARED_RULES`] sets BF2's rifle alike).
    // measure: bot snipers' one-hit kills of a sniper-class stand-in (0 at 280).
    DamageRow {
        class: "*_weap_inf_sniperrifle_ord",
        person: 280.0,
        twin: "all_weap_inf_sniper_rifle_ord",
    },
    DamageRow {
        class: "*_weap_inf_thermaldetonator_exp",
        person: 450.0,
        twin: "all_weap_inf_thermaldetonator_exp",
    },
    DamageRow {
        class: "*_weap_inf_torpedo_launcher_exp",
        person: 400.0,
        twin: "all_weap_inf_rocket_launcher_exp",
    },
    DamageRow {
        class: "*_weap_inf_mortar_launcher_exp",
        person: 400.0,
        twin: "all_weap_inf_mortar_launcher_exp",
    },
    DamageRow {
        class: "*_weap_inf_remotedroid_blaster_ord",
        person: 60.0,
        twin: "all_weap_inf_remotedroid_blaster_ord",
    },
    DamageRow {
        class: "tat_weap_inf_tuskenrifle_ord",
        person: 80.0,
        twin: "tat_weap_inf_tuskenrifle_ord",
    },
    DamageRow {
        class: "tat_weap_inf_tuskensniperrifle_ord",
        person: 200.0,
        twin: "tat_weap_inf_tuskensniperrifle_ord",
    },
    DamageRow {
        class: "geo_weap_inf_blunderbuss_ord",
        person: 100.0,
        twin: "geo_weap_inf_blunderbuss_hidden",
    },
    DamageRow {
        class: "ewk_weap_inf_spear_ord",
        person: 200.0,
        twin: "ewk_weap_inf_spear_ord",
    },
    // Already BF2's: kept as authored.
    DamageRow {
        class: "com_weap_inf_empmine_exp",
        person: 100.0,
        twin: "com_weap_inf_empmine_exp",
    },
    DamageRow {
        class: "com_weap_inf_seismic_mine_exp",
        person: 5000.0,
        twin: "com_weap_inf_seismic_mine_exp",
    },
];

/// A key a BF1 class takes from its BF2 counterpart: the class (`*` as in
/// [`DamageRow`]), the key under each name the discs spell it, the value and
/// the BF2 class it comes from.
pub struct KeyRow {
    pub class: &'static str,
    pub keys: &'static [&'static str],
    pub value: f32,
    pub twin: &'static str,
}

pub const KEYS: &[KeyRow] = &[
    // One launcher lock time on every map: BF1's torpedo launchers took
    // 1.5 s, BF2's rocket launcher 1.0 s.
    KeyRow {
        class: "*_weap_inf_torpedo_launcher",
        keys: &["LockTime"],
        value: 1.0,
        twin: "com_weap_inf_rocket_launcher",
    },
    // A weapon that takes BF2's damage takes BF2's rate of fire with it.
    KeyRow {
        class: "imp_weap_inf_rifle",
        keys: &["ShotDelay"],
        value: 0.166,
        twin: "imp_weap_inf_rifle",
    },
    KeyRow {
        class: "*_weap_inf_sniperrifle",
        keys: &["ShotDelay"],
        value: 1.0,
        twin: "all_weap_inf_sniper_rifle",
    },
    // Every army sniper rifle zooms in BF2's two steps, x2 then x8 (BF1's
    // Imperial and Republic rifles had one x6 step). The turn divisors follow
    // the zoom (`WeaponClass::from_props`).
    KeyRow {
        class: "*_weap_inf_sniperrifle",
        keys: &["ZoomMin"],
        value: 2.0,
        twin: "all_weap_inf_sniper_rifle",
    },
    KeyRow {
        class: "*_weap_inf_sniperrifle",
        keys: &["ZoomMax"],
        value: 8.0,
        twin: "all_weap_inf_sniper_rifle",
    },
    KeyRow {
        class: "rep_weap_inf_commando_pistol",
        keys: &["ShotDelay"],
        value: 0.1,
        twin: "rep_weap_inf_commando_pistol",
    },
    KeyRow {
        class: "tat_weap_inf_tuskenrifle",
        keys: &["ShotDelay"],
        value: 0.33,
        twin: "tat_weap_inf_tuskenrifle",
    },
    KeyRow {
        class: "tat_weap_inf_tuskensniperrifle",
        keys: &["ShotDelay"],
        value: 1.0,
        twin: "tat_weap_inf_tuskensniperrifle",
    },
    KeyRow {
        class: "geo_weap_inf_blunderbuss",
        keys: &["ShotDelay"],
        value: 1.0,
        twin: "geo_weap_inf_blunderbuss",
    },
    // The rocket's blast reaches as far as BF2's.
    KeyRow {
        class: "*_weap_inf_torpedo_launcher_exp",
        keys: &["DamageRadiusInner"],
        value: 2.0,
        twin: "all_weap_inf_rocket_launcher_exp",
    },
    KeyRow {
        class: "*_weap_inf_torpedo_launcher_exp",
        keys: &["DamageRadiusOuter", "DamageRadius"],
        value: 4.0,
        twin: "all_weap_inf_rocket_launcher_exp",
    },
    // The recon droid is as tough as BF2's.
    KeyRow {
        class: "*_weap_inf_remotedroid_ord",
        keys: &["MaxHealth"],
        value: 100.0,
        twin: "all_weap_inf_remotedroid_ord",
    },
    // The four sides' fusion cutter reaches as far and as wide as BF2's, so
    // repairing and slicing take the same approach on every map.
    KeyRow {
        class: "all_weap_inf_fusioncutter",
        keys: &["LockOnRange"],
        value: 4.0,
        twin: "all_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "all_weap_inf_fusioncutter",
        keys: &["LockOnAngle"],
        value: 90.0,
        twin: "all_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "imp_weap_inf_fusioncutter",
        keys: &["LockOnRange"],
        value: 4.0,
        twin: "imp_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "imp_weap_inf_fusioncutter",
        keys: &["LockOnAngle"],
        value: 90.0,
        twin: "imp_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "rep_weap_inf_fusioncutter",
        keys: &["LockOnRange"],
        value: 4.0,
        twin: "rep_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "rep_weap_inf_fusioncutter",
        keys: &["LockOnAngle"],
        value: 90.0,
        twin: "rep_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "cis_weap_inf_fusioncutter",
        keys: &["LockOnRange"],
        value: 4.0,
        twin: "cis_weap_inf_fusioncutter",
    },
    KeyRow {
        class: "cis_weap_inf_fusioncutter",
        keys: &["LockOnAngle"],
        value: 90.0,
        twin: "cis_weap_inf_fusioncutter",
    },
    // A health droid heals as fast as BF2's.
    KeyRow {
        class: "com_item_healthrecharge",
        keys: &["SoldierHealth"],
        value: 25.0,
        twin: "com_item_healthrecharge",
    },
];

/// A damage class both games take at the maintainer's value: its damage to
/// soldiers, animals, droids and shields lands on `person`; vehicles and
/// buildings take what they took.
pub struct SharedRow {
    pub class: &'static str,
    pub person: f32,
}

/// [ours] The maintainer 2026-11-14: a sniper's head hit kills any non-hero
/// in one shot and its body hit never kills a full-health soldier of 300
/// health or more (natives under 280, such as Ewoks, Gungans, Jawas and
/// Geonosians, are accepted exceptions). The army
/// sniper rifle does 280 (300-health classes take two body hits; x3 on the
/// head, 840, kills up to the 800 Wookiee), and the award beam rifle keeps
/// BF2's x2 over it, 560 (its five beam ticks land about 622, as BF2's 600
/// lands about 667). BF1's rifle takes 280 through [`PERSON_DAMAGE`]; the
/// Tusken sniper keeps BF2's 200.
///
/// measure: the #171 sniper table (a sniper-class stand-in's deaths and
/// one-hit deaths to bot snipers) and the Hard/Elite stand-in win rates;
/// replace: the maintainer's playtest verdict on sniper duels.
pub const SHARED_RULES: &[SharedRow] = &[
    SharedRow {
        class: "com_weap_inf_sniper_rifle_ord",
        person: 280.0,
    },
    SharedRow {
        class: "com_weap_award_sniper_rifle_ord",
        person: 560.0,
    },
];

/// Apply [`SHARED_RULES`] to one class definition of either game. A side's child
/// class (`all_weap_inf_sniper_rifle_ord`) inherits the `com_` class's
/// damage, so only the class that authors it moves.
pub fn shared_rules(def: &mut ClassDef) {
    if !matches!(def.kind.as_str(), "ordc" | "expc") {
        return;
    }
    if let Some(row) = SHARED_RULES.iter().find(|r| matches(r.class, &def.name)) {
        land(def, Some(row.person));
    }
}

fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_prefix('*') {
        Some(tail) => {
            name.len() >= tail.len()
                && name.is_char_boundary(name.len() - tail.len())
                && name[name.len() - tail.len()..].eq_ignore_ascii_case(tail)
        }
        None => pattern.eq_ignore_ascii_case(name),
    }
}

/// The health a BF1 soldier class takes from [`HEALTH`], if it has a row.
pub fn health_row(class: &str) -> Option<&'static HealthRow> {
    HEALTH
        .iter()
        .find(|r| r.classes.iter().any(|c| c.eq_ignore_ascii_case(class)))
}

/// Is a BF1 ordnance or explosion class named `class` an infantry weapon's?
/// Every one a BF1 soldier or native carries is named so; the rest are
/// vehicle and turret guns and death blasts (the interim rule).
pub fn is_infantry_damage(class: &str) -> bool {
    class.to_ascii_lowercase().contains("_weap_inf_")
}

fn value(v: &str) -> Option<f32> {
    v.trim().parse().ok()
}

fn first(def: &ClassDef, key: u32) -> Option<f32> {
    def.props
        .iter()
        .find(|p| p.0 == key)
        .and_then(|p| value(&p.1))
}

/// Multiply every occurrence of `key`; false when the class has none.
fn scale(def: &mut ClassDef, key: u32, k: f32) -> bool {
    let mut found = false;
    for p in def.props.iter_mut().filter(|p| p.0 == key) {
        found = true;
        if let Some(v) = value(&p.1) {
            p.1 = format!("{}", v * k);
        }
    }
    found
}

/// Set a key under whichever of `names` the class spells it (every
/// occurrence), or add it under the first.
fn set(def: &mut ClassDef, names: &[&str], v: f32) {
    let keys: Vec<u32> = names.iter().map(|n| fnv1a_lowercase(n)).collect();
    let mut found = false;
    for p in def.props.iter_mut().filter(|p| keys.contains(&p.0)) {
        found = true;
        p.1 = format!("{v}");
    }
    if !found {
        def.props.insert(0, (keys[0], format!("{v}")));
    }
}

/// Rewrite one BF1 class definition to the shared numbers. BF1 classes
/// never inherit from one another, so one definition at a time is exact.
pub fn apply(def: &mut ClassDef) {
    vehicles::link_variants(def);
    if let Some(row) = vehicles::twin_row(&def.name) {
        // A gun, round or blast that takes its BF2 twin's numbers whole.
        vehicles::take(def, row);
    } else {
        match def.kind.as_str() {
            "entc" => entity(def),
            // An infantry weapon by its row or the scale; any other gun or
            // blast by the scale.
            "ordc" | "expc" => {
                damage(def);
                vehicles::against_vehicles(def);
            }
            _ => {}
        }
    }
    for row in KEYS {
        if matches(row.class, &def.name) {
            set(def, row.keys, row.value);
        }
    }
}

fn entity(def: &mut ClassDef) {
    let base = def.base.to_ascii_lowercase();
    if let Some(row) = vehicles::health_row(&def.name) {
        set(def, &["MaxHealth"], row.health);
    } else if base == "soldier" || base == "walkerdroid" {
        let max_health = fnv1a_lowercase("MaxHealth");
        if let Some(row) = health_row(&def.name) {
            set(def, &["MaxHealth"], row.health);
        } else if first(def, max_health).is_some_and(|h| h < UNSCALED_HEALTH) {
            scale(def, max_health, BF1_SCALE);
        }
    } else if base == "powerupitem" {
        // A dropped bacta tank heals the same share of a trooper as before.
        scale(def, fnv1a_lowercase("SoldierHealth"), BF1_SCALE);
    }
}

fn damage(def: &mut ClassDef) {
    let person = PERSON_DAMAGE
        .iter()
        .find(|r| matches(r.class, &def.name))
        .map(|r| r.person);
    land(def, person);
}

/// Move a damage class's hits on soldiers, animals, droids and shields so a
/// hit on a person deals `target`, or by [`BF1_SCALE`] when `None`.
fn land(def: &mut ClassDef, target: Option<f32>) {
    let [max_damage, plain_damage] = ["MaxDamage", "Damage"].map(fnv1a_lowercase);
    let [health, person, animal, droid, shield] = [
        "HealthScale",
        "PersonScale",
        "AnimalScale",
        "DroidScale",
        "ShieldScale",
    ]
    .map(fnv1a_lowercase);
    let Some(authored) = first(def, max_damage).or_else(|| first(def, plain_damage)) else {
        return;
    };
    let to_person = authored
        * first(def, person)
            .or_else(|| first(def, health))
            .unwrap_or(1.0);
    let k = target
        .filter(|_| to_person > 0.0)
        .map_or(BF1_SCALE, |p| p / to_person);
    if k == 1.0 {
        return;
    }
    if def.base.eq_ignore_ascii_case("emitterordnance") {
        // BF1's emitter ignores the scales, so its damage itself moves.
        scale(def, max_damage, k);
        scale(def, plain_damage, k);
        return;
    }
    let combined = scale(def, health, k);
    for key in [person, animal, droid] {
        if !scale(def, key, k) && !combined {
            def.props.insert(0, (key, format!("{k}")));
        }
    }
    if !scale(def, shield, k) {
        def.props.insert(0, (shield, format!("{k}")));
    }
}

#[cfg(test)]
mod tests;
