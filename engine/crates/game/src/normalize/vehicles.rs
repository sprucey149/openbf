//! The vehicle pass: one set of vehicle, turret and anti-vehicle numbers for
//! both games (`analysis/vehicle-normalization.md`).
//!
//! [ours] As for soldiers, BF2's numbers are the reference. It runs inside
//! [`super::apply`], so it rewrites BF1's side files and a BF1 map's own
//! class file. The BF2 Hailfire's duplicate damage keys are resolved when
//! its gun is installed in a ground round.
//!
//! - V1, twins: a BF1 vehicle or turret with a BF2 twin takes the twin's
//!   `MaxHealth` ([`HEALTH`]), and each of its guns takes the BF2 gun of the
//!   same role ([`TWINS`]): damage, scales, fall-off, rate, salvo, heat,
//!   clip, reload, a missile's turn rate and a beam's pass-through. The
//!   round keeps BF1's look, sound, speed, life span and blast radii. A gun
//!   slot BF2 has and BF1 does not is not added. The weak spots are
//!   `assets::BF1_WEAK_SPOTS`.
//! - V2, vehicles only one game has: health by the twin family's ratio
//!   ([`HEALTH`]); their guns keep the soldier rule's x4/3 and do
//!   [`VEHICLE_FACTOR`] more to vehicles ([`VEHICLE_FACTOR_CLASSES`]).
//! - V3, infantry weapons against vehicles and buildings: the launcher,
//!   grenade, mines, remote charge and mortar take their twin's damage and
//!   scales whole ([`TWINS`]); blasters do [`LIGHT_ARMS_SCALE`] to vehicles
//!   and buildings ([`LIGHT_ARMS`]); BF1-only grenades do
//!   [`VEHICLE_FACTOR`] more to vehicles.
//! - V4, turrets: twins as V1.
//! - V5, flyers: only the snowspeeder has a twin here. BF1's starfighters
//!   keep BF1's numbers and the soldier rule's x4/3 (a named difference;
//!   replace: a flyer pass).

use super::{first, matches, scale};
use crate::classdb::{fnv1a_lowercase, ClassDb, ClassDef};

pub(crate) fn ground_gun(db: &ClassDb, gun: &mut core::weapon::WeaponClass) {
    if db.is_bf2()
        && gun
            .name
            .eq_ignore_ascii_case("cis_weap_tread_hailfire_chaingun")
    {
        // The inherited ordnance writes MaxDamage twice. The original
        // setter applies both writes, leaving the last value.
        if let Some(damage) = db
            .props_all("cis_weap_tread_hailfire_chaingun_ord", "MaxDamage")
            .last()
            .and_then(|value| value.trim().parse().ok())
        {
            gun.ordnance.max_damage = damage;
        }
    }
}

/// A BF1 class that takes its BF2 twin's numbers.
pub struct Twin {
    /// The BF1 class (`*` as in [`super::DamageRow`]).
    pub class: &'static str,
    /// The BF2 class the numbers come from. `None` for a blast whose BF2 gun
    /// has none: damage is zero and the blast keeps its look and impulse.
    pub twin: Option<&'static str>,
    /// `Key value, Key value ...`: each key is set on the class, under
    /// whichever name it spells it, or added. `MaxDamage` also covers the
    /// older `Damage`. A key with several values sets its occurrences in
    /// order (a charge weapon's points). A `0x` key is a hash whose name is
    /// not known (BF2's fall-off).
    pub keys: &'static str,
}

impl Twin {
    const fn new(class: &'static str, twin: Option<&'static str>, keys: &'static str) -> Self {
        Self { class, twin, keys }
    }

    /// Each key's hash, the names it may be spelled under and its values.
    pub fn values(&self) -> impl Iterator<Item = (&'static str, Vec<f32>)> {
        self.keys.split(',').map(|item| {
            let mut words = item.split_whitespace();
            let key = words.next().expect("twin key");
            let values = words
                .map(|w| w.parse().expect("twin value"))
                .collect::<Vec<f32>>();
            assert!(!values.is_empty(), "{key} has no value");
            (key, values)
        })
    }
}

/// Health a BF1 vehicle or turret takes.
pub struct HealthRow {
    pub class: &'static str,
    pub health: f32,
    /// The BF2 twin it is taken from; `None` for a vehicle only BF1 has.
    pub twin: Option<&'static str>,
    /// For a vehicle only BF1 has: the factor its disc health is multiplied
    /// by, from the twin family named in the note
    /// (`analysis/vehicle-normalization.md`).
    pub ratio: f32,
}

const fn twin(class: &'static str, health: f32, twin: &'static str) -> HealthRow {
    HealthRow {
        class,
        health,
        twin: Some(twin),
        ratio: 0.0,
    }
}

/// PROVISIONAL: a vehicle BF2 has no twin for, multiplied by `ratio`.
/// measure: the frozen stand-in human's kills a minute in and against the
/// vehicle on its BF1 maps against the pre-change baseline.
const fn one_game(class: &'static str, health: f32, ratio: f32) -> HealthRow {
    HealthRow {
        class,
        health,
        twin: None,
        ratio,
    }
}

/// BF2's standard tank (8000) over BF1's (6600).
pub const VEHICLE_FACTOR: f32 = 8000.0 / 6600.0;

pub const HEALTH: &[HealthRow] = &[
    twin("imp_hover_fightertank", 8000.0, "imp_hover_fightertank"),
    twin("rep_hover_fightertank", 8000.0, "rep_hover_fightertank"),
    twin("cis_hover_aat", 8000.0, "cis_hover_aat"),
    twin("imp_walk_atst", 8000.0, "imp_walk_atst"),
    twin("imp_walk_atst_jungle", 8000.0, "imp_walk_atst_jungle"),
    twin("imp_walk_atst_snow", 8000.0, "imp_walk_atst_snow"),
    twin("imp_walk_atat", 60000.0, "imp_walk_atat"),
    twin("rep_walk_atte", 40000.0, "rep_walk_atte"),
    twin("cis_walk_spider", 20000.0, "cis_walk_spider"),
    twin("cis_tread_hailfire", 8000.0, "cis_tread_hailfire"),
    twin("all_hover_combatspeeder", 8000.0, "all_hover_combatspeeder"),
    twin("imp_hover_speederbike", 80.0, "imp_hover_speederbike"),
    twin("rep_hover_speederbike", 80.0, "rep_hover_barcspeeder"),
    twin("cis_hover_stap", 150.0, "cis_hover_stap"),
    twin("all_fly_snowspeeder", 1400.0, "all_fly_snowspeeder"),
    // Turrets, V4.
    twin("com_weap_gunturret", 800.0, "tur_bldg_laser"),
    twin("geo_bldg_geoturret", 3000.0, "tur_bldg_geoturret"),
    twin("hoth_bldg_dish_turret", 3000.0, "tur_bldg_hoth_dishturret"),
    twin("hoth_bldg_hoth_turret", 3000.0, "tur_bldg_hoth_lasermortar"),
    twin("yav_bldg_tower_turret", 800.0, "tur_bldg_tower"),
    twin("tat_bldg_barge_turret", 400.0, "tur_bldg_tat_barge"),
    // V2: the AT-TE's ratio (40000 / 15000) for the heavy hovers.
    one_game("cis_hover_mtt", 40000.0, 8.0 / 3.0),
    one_game("all_hover_hovernaut", 33333.0, 8.0 / 3.0),
    // The combat speeder's (8000 / 5400): the Gian speeder is its kind.
    one_game("nab_hover_gianspeeder", 8000.0, 8000.0 / 5400.0),
    // The tank's (8000 / 6600).
    one_game("tat_hover_skiff", 5091.0, VEHICLE_FACTOR),
    // The speeder bike's (80 / 600).
    one_game("tat_hover_landspeeder", 100.0, 80.0 / 600.0),
    // The gun turret's (800 / 300).
    one_game("bes_bldg_AAA_turret", 1333.0, 800.0 / 300.0),
];

/// PROVISIONAL: the damage classes that do [`VEHICLE_FACTOR`] more to
/// vehicles, so a vehicle only BF1 has, or a grenade only BF1 has, takes as
/// many hits to kill a standard tank as on its disc. measure: as
/// [`one_game`].
pub const VEHICLE_FACTOR_CLASSES: &[&str] = &[
    "openbf_bf1_mtt_cannon_ord",
    "all_weap_hover_hovernaut_blaster_ord",
    "all_weap_hover_hovernaut_blaster_exp",
    "all_weap_hover_hovernaut_missile_ord",
    "all_weap_hover_hovernaut_missile_exp",
    "all_weap_hover_hovernaut_cannon1_ord",
    "nab_weap_hover_gianspeeder_cannon_ord",
    "nab_weap_hover_gianspeeder_cannon_exp",
    "nab_weap_hover_gianspeeder_cannon1_ord",
    "kam_weap_bldg_AAA_turret_cannon_ord",
    "kam_weap_bldg_AAA_turret_cannon_exp",
    "*_weap_inf_crackgrenade_exp",
    "*_weap_inf_haywiredetonator_exp",
];

// BF1 reuses these classes in slots with different damage rules. Copy the
// authored class for the named owner so its other users keep their own rule.
const VARIANTS: &[(&str, &str, &str)] = &[
    (
        "cis_tread_hailfire",
        "cis_weap_walk_spider_cannon",
        "openbf_bf1_hailfire_cannon",
    ),
    (
        "openbf_bf1_hailfire_cannon",
        "cis_weap_walk_spider_cannon_ord",
        "openbf_bf1_hailfire_cannon_ord",
    ),
    (
        "cis_hover_mtt",
        "cis_weap_fly_maf_cannon",
        "openbf_bf1_mtt_cannon",
    ),
    (
        "openbf_bf1_mtt_cannon",
        "cis_weap_fly_maf_cannon_ord",
        "openbf_bf1_mtt_cannon_ord",
    ),
    (
        "com_weap_bldg_gunturret_ord",
        "com_weap_bldg_gunturret_exp",
        "openbf_bf1_gunturret_impact_exp",
    ),
];

pub(crate) fn variant(def: &ClassDef) -> Option<ClassDef> {
    let (_, _, name) = VARIANTS
        .iter()
        .find(|(_, original, _)| def.name.eq_ignore_ascii_case(original))?;
    let mut variant = def.clone();
    variant.name = (*name).into();
    Some(variant)
}

pub(super) fn link_variants(def: &mut ClassDef) {
    for &(owner, original, name) in VARIANTS {
        if def.name.eq_ignore_ascii_case(owner) {
            for (_, value) in &mut def.props {
                if value.eq_ignore_ascii_case(original) {
                    *value = name.into();
                }
            }
        }
    }
}

/// What BF2's blasters do to vehicles and buildings (every BF2 rifle,
/// pistol, sniper rifle, shotgun and bowcaster bolt authors 0.1 for both).
pub const LIGHT_ARMS_SCALE: f32 = 0.1;

/// BF1's blaster bolts, which do [`LIGHT_ARMS_SCALE`] to vehicles and
/// buildings as BF2's do.
pub const LIGHT_ARMS: &[&str] = &[
    "*_weap_inf_rifle_ord",
    "*_weap_inf_pistol_ord",
    "rep_weap_inf_commando_pistol_ord",
    "*_weap_inf_sniperrifle_ord",
    "*_weap_inf_shotgun_ord",
    "*_weap_inf_bowcaster_ord",
    "*_weap_inf_bowcaster_ord_high",
    "cis_weap_inf_repeater_ord",
    "cis_weap_inf_wrist_blaster_ord",
    "*_weap_inf_remotedroid_blaster_ord",
];

/// Generated from the baked class files: each BF1 gun beside its BF2
/// twin's resolved numbers (the weapon as the class loader reads it, the
/// last value over the chain; a round or a blast as `ClassDb::prop` reads
/// it, the child-most). `normalization::vehicle_twins_equal_on_both_maps`
/// checks every row against BF2's own class.
pub const TWINS: &[Twin] = &[
    // Imperial fighter tank: light cannon.
    Twin::new("imp_weap_hover_fightertank_cannon", Some("imp_weap_hover_fightertank_cannon"), "ShotDelay 0.22, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.12, HeatRecoverRate 0.25, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("imp_weap_hover_fightertank_cannon_ord", Some("imp_weap_hover_fightertank_cannon_ord"), "MaxDamage 200, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("imp_weap_hover_fightertank_cannon_exp", None, "MaxDamage 0"),
    // Imperial fighter tank: missiles.
    Twin::new("imp_weap_hover_fightertank_missile", Some("imp_weap_hover_fightertank_missile"), "ShotDelay 3, SalvoCount 2, ShotsPerSalvo 1, SalvoDelay 0.05, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 0, ReloadTime 4"),
    Twin::new("imp_weap_hover_fightertank_missile_ord", Some("imp_weap_hover_fightertank_missile_ord"), "MaxDamage 1500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("imp_weap_hover_fightertank_missile_exp", Some("imp_weap_hover_fightertank_missile_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 1"),
    // Imperial fighter tank: gunner beam.
    Twin::new("imp_weap_hover_fightertank_laser", Some("imp_weap_hover_fightertank_laser"), "ShotDelay 2, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("imp_weap_hover_fightertank_laser_ord", Some("imp_weap_hover_fightertank_laser_ord"), "MaxDamage 1000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.5, BuildingScale 0.5, ShieldScale 1, PassThrough 1"),
    // Republic fighter tank: light cannon.
    Twin::new("rep_weap_hover_fightertank_cannon", Some("rep_weap_hover_fightertank_cannon"), "ShotDelay 0.22, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.12, HeatRecoverRate 0.25, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("rep_weap_hover_fightertank_cannon_ord", Some("rep_weap_hover_fightertank_cannon_ord"), "MaxDamage 200, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("rep_weap_hover_fightertank_cannon_exp", None, "MaxDamage 0"),
    // Republic fighter tank: missiles.
    Twin::new("rep_weap_hover_fightertank_missile", Some("rep_weap_hover_fightertank_missile"), "ShotDelay 3, SalvoCount 2, ShotsPerSalvo 1, SalvoDelay 0.05, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 0, ReloadTime 4"),
    Twin::new("rep_weap_hover_fightertank_missile_ord", Some("rep_weap_hover_fightertank_missile_ord"), "MaxDamage 1500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("rep_weap_hover_fightertank_missile_exp", Some("rep_weap_hover_fightertank_missile_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 1"),
    // Republic fighter tank: gunner beam.
    Twin::new("rep_weap_hover_fightertank_laser", Some("rep_weap_hover_fightertank_laser"), "ShotDelay 2, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("rep_weap_hover_fightertank_laser_ord", Some("rep_weap_hover_fightertank_laser_ord"), "MaxDamage 1000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.5, BuildingScale 0.5, ShieldScale 1, PassThrough 1"),
    // AAT: light cannon.
    Twin::new("cis_weap_hover_aat_cannon", Some("cis_weap_hover_aat_cannon"), "ShotDelay 0.22, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.15, HeatRecoverRate 0.25, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("cis_weap_hover_aat_cannon_ord", Some("cis_weap_hover_aat_cannon_ord"), "MaxDamage 200, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("cis_weap_hover_aat_cannon_exp", None, "MaxDamage 0"),
    // AAT: shell launcher.
    Twin::new("cis_weap_hover_aat_launcher", Some("cis_weap_hover_aat_launcher"), "ShotDelay 4, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0.1, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0"),
    Twin::new("cis_weap_hover_aat_launcher_ord", Some("cis_weap_hover_aat_launcher_ord"), "MaxDamage 2000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("cis_weap_hover_aat_launcher_exp", Some("cis_weap_hover_aat_launcher_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 1"),
    // AT-ST: chin cannon.
    Twin::new("imp_weap_walk_atst_cannon_chin", Some("imp_weap_walk_atst_cannon_chin"), "ShotDelay 4, SalvoCount 2, ShotsPerSalvo 1, SalvoDelay 0.1, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0"),
    Twin::new("imp_weap_walk_atst_cannon_chin1_ord", Some("imp_weap_walk_atst_cannon_chin1_ord"), "MaxDamage 1000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("imp_weap_walk_atst_cannon_chin1_exp", Some("imp_weap_walk_atst_cannon_chin1_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 1"),
    // AT-ST: head blasters.
    Twin::new("imp_weap_walk_atst_cannon_head", Some("imp_weap_walk_atst_cannon_head"), "ShotDelay 0.18, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.14, HeatRecoverRate 0.38, HeatThreshold 0.02, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("imp_weap_walk_atst_cannon_head_ord", Some("imp_weap_walk_atst_cannon_head_ord"), "MaxDamage 200, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("imp_weap_walk_atst_cannon_head_exp", None, "MaxDamage 0"),
    // AT-AT: chin cannon.
    Twin::new("imp_weap_walk_atat_cannon_chin", Some("imp_weap_walk_atat_cannon_chin"), "ShotDelay 2, SalvoCount 2, ShotsPerSalvo 1, SalvoDelay 0.1, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0"),
    Twin::new("imp_weap_walk_atat_cannon_chin_ord1", Some("imp_weap_walk_atat_cannon_chin_ord1"), "MaxDamage 5000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("imp_weap_walk_atat_cannon_chin_exp4", Some("imp_weap_walk_atat_cannon_chin_exp4"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0, ShieldScale 1"),
    // AT-AT: head blasters.
    Twin::new("imp_weap_walk_atat_cannon_head", Some("imp_weap_walk_atat_cannon_head"), "ShotDelay 0.25, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.08, HeatRecoverRate 0.1, HeatThreshold 0.25, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("imp_weap_walk_atat_cannon_head_ord", Some("imp_weap_walk_atat_cannon_head_ord"), "MaxDamage 1000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1, 0xa74561b2 2, 0xee08d1ba 0.5, 0x955f71e9 500"),
    Twin::new("imp_weap_walk_atat_cannon_head_exp", Some("imp_weap_walk_atat_cannon_head_exp"), "MaxDamage 50, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 0"),
    // AT-TE: the pilot's heavy cannon, BF2's recoilless gun.
    Twin::new("rep_weap_walk_atte_cannon_heavy", Some("rep_weap_walk_atte_recoilless"), "ShotDelay 1.5, SalvoCount 2, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0"),
    Twin::new("rep_weap_walk_atte_cannon_heavy_ord", Some("rep_weap_walk_atte_recoilless_ord"), "MaxDamage 1250, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("rep_weap_walk_atte_cannon_heavy_exp", Some("rep_weap_walk_atte_recoilless_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 0"),
    // AT-TE: the turret cannon.
    Twin::new("rep_weap_walk_atte_turretcannon", Some("rep_weap_walk_atte_turret_cannon"), "ShotDelay 1.5, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0"),
    Twin::new("rep_weap_walk_atte_turretcannon_ord1", Some("rep_weap_walk_atte_turret_cannon_ord"), "MaxDamage 2000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("rep_weap_walk_atte_turretcannon_exp4", Some("rep_weap_walk_atte_turret_cannon_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 0"),
    // AT-TE: the side guns, BF2's laser.
    Twin::new("rep_weap_walk_atte_cannon", Some("rep_weap_walk_atte_laser"), "ShotDelay 0.25, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.1, HeatRecoverRate 0.1, HeatThreshold 0.25, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("rep_weap_walk_atte_cannon_ord", Some("rep_weap_walk_atte_laser_ord"), "MaxDamage 500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.2, BuildingScale 0.4, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("rep_weap_walk_atte_cannon_exp", Some("rep_weap_walk_atte_laser_exp"), "MaxDamage 50, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 0"),
    // Spider walker: beam.
    Twin::new("cis_weap_walk_spider_laser", Some("cis_weap_walk_spider_beam"), "ShotDelay 2, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("cis_weap_walk_spider_laser_ord", Some("cis_weap_walk_spider_beam_ord"), "MaxDamage 3000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.5, BuildingScale 0.5, ShieldScale 1, PassThrough 1"),
    // Spider walker: blaster, BF2's STAP cannon.
    Twin::new("cis_weap_walk_spider_cannon", Some("cis_weap_hover_stap_cannon"), "ShotDelay 0.25, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.15, HeatRecoverRate 0.28, HeatThreshold 0.05, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("cis_weap_walk_spider_cannon_ord", Some("cis_weap_hover_stap_cannon_ord"), "MaxDamage 250, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("cis_weap_walk_spider_cannon_exp", None, "MaxDamage 0"),
    // Hailfire: its own chaingun numbers on BF1's shared spider cannon.
    Twin::new("openbf_bf1_hailfire_cannon", Some("cis_weap_tread_hailfire_chaingun"), "ShotDelay 0.05, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.03, HeatRecoverRate 0.35, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("openbf_bf1_hailfire_cannon_ord", Some("cis_weap_tread_hailfire_chaingun_ord"), "MaxDamage 50, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 0.1"),
    // Hailfire: missiles.
    Twin::new("cis_weap_tread_hailfire_missile", Some("cis_weap_tread_hailfire_missile"), "ShotDelay 0.25, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0.05, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 16, ReloadTime 4"),
    Twin::new("cis_weap_tread_hailfire_missile_ord", Some("cis_weap_tread_hailfire_missile_ord"), "MaxDamage 1500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1, TurnRate 0"),
    Twin::new("cis_weap_tread_hailfire_missile_exp", Some("cis_weap_tread_hailfire_missile_exp"), "MaxDamage 300, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 1"),
    // Combat speeder: light blaster, BF2's cannon.
    Twin::new("all_weap_hover_combatspeeder_cannon1", Some("all_weap_hover_combatspeeder_cannon"), "ShotDelay 0.15, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.125, HeatRecoverRate 0.5, HeatThreshold 0.05, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("all_weap_hover_combatspeeder_cannon1_ord", Some("all_weap_hover_combatspeeder_cannon_ord"), "MaxDamage 150, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("all_weap_hover_combatspeeder_cannon1_exp", None, "MaxDamage 0"),
    // Combat speeder: missiles.
    Twin::new("all_weap_hover_combatspeeder_missile", Some("all_weap_hover_combatspeeder_missile"), "ShotDelay 0.25, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0.05, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 12, ReloadTime 5"),
    Twin::new("all_weap_hover_combatspeeder_missile_ord", Some("all_weap_hover_combatspeeder_missile_ord"), "MaxDamage 1500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("all_weap_hover_combatspeeder_missile_exp", Some("all_weap_hover_combatspeeder_missile_exp"), "MaxDamage 150, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 1"),
    // Combat speeder: main cannon, BF2's cannon1.
    Twin::new("all_weap_hover_combatspeeder_cannon", Some("all_weap_hover_combatspeeder_cannon1"), "ShotDelay 0.5, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0"),
    Twin::new("all_weap_hover_combatspeeder_cannon_ord", Some("all_weap_hover_combatspeeder_cannon1_ord"), "MaxDamage 500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("all_weap_hover_combatspeeder_cannon_exp", Some("all_weap_hover_combatspeeder_cannon1_exp"), "MaxDamage 125, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 0"),
    // Imperial speeder bike.
    Twin::new("imp_weap_hover_speederbike_cannon", Some("imp_weap_hover_speederbike_cannon"), "ShotDelay 0.15, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.15, HeatRecoverRate 0.5, HeatThreshold 0.05, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("imp_weap_hover_speederbike_cannon_ord", Some("imp_weap_hover_speederbike_cannon_ord"), "MaxDamage 150, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    // Republic speeder bike (BF2 rep_hover_barcspeeder).
    Twin::new("rep_weap_hover_speederbike_cannon", Some("rep_weap_hover_speederbike_cannon"), "ShotDelay 0.15, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.15, HeatRecoverRate 0.5, HeatThreshold 0.05, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("rep_weap_hover_speederbike_cannon_ord", Some("rep_weap_hover_speederbike_cannon_ord"), "MaxDamage 100, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("rep_weap_hover_speederbike_cannon_exp", None, "MaxDamage 0"),
    // STAP.
    Twin::new("cis_weap_hover_stap_cannon", Some("cis_weap_hover_stap_cannon"), "ShotDelay 0.25, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.15, HeatRecoverRate 0.28, HeatThreshold 0.05, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("cis_weap_hover_stap_cannon_ord", Some("cis_weap_hover_stap_cannon_ord"), "MaxDamage 250, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("cis_weap_hover_stap_cannon_exp", None, "MaxDamage 0"),
    // Snowspeeder: cannon.
    Twin::new("all_weap_fly_snowspeeder_cannon", Some("all_weap_fly_snowspeeder_cannon"), "ShotDelay 0.25, SalvoCount 2, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.08, HeatRecoverRate 0.25, HeatThreshold 0.9, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("all_weap_fly_snowspeeder_cannon_ord", Some("all_weap_fly_snowspeeder_cannon_ord"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.25, ShieldScale 1"),
    Twin::new("all_weap_fly_snowspeeder_cannon_exp", Some("all_weap_fly_snowspeeder_cannon_exp"), "MaxDamage 0, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0, ShieldScale 0.5"),
    // Snowspeeder: tow cable.
    Twin::new("all_weap_fly_snowspeeder_reargun", Some("all_weap_fly_snowspeeder_reargun"), "ShotDelay 3, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 20, ReloadTime 3"),
    Twin::new("all_weap_fly_snowspeeder_reargun_ord", Some("all_weap_fly_snowspeeder_reargun_ord"), "MaxDamage 50, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    // Gun turret (also the barge turret's and the skiff's gun): BF2's laser turret.
    Twin::new("com_weap_bldg_gunturret", Some("tur_weap_laser"), "ShotDelay 0.22, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.12, HeatRecoverRate 0.25, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("com_weap_bldg_gunturret_ord", Some("tur_weap_laser_ord"), "MaxDamage 100, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    Twin::new("openbf_bf1_gunturret_impact_exp", None, "MaxDamage 0"),
    // Geonosis turret (bf1_geo1): BF2's Geonosis turret, the ray's damage on the MaxDamage it reads.
    Twin::new("geo_weap_bldg_geoturret", Some("tur_weap_geoturret"), "ShotDelay 1, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0.17, InitialSalvoDelay 0, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 0, ReloadTime 2"),
    Twin::new("geo_weap_bldg_geoturret_ord", Some("tur_weap_geoturret_ord"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0, BuildingScale 0, ShieldScale 5"),
    // Hoth dish turret (bf1_hot1): BF2's dish turret, four charge points.
    Twin::new("hoth_weap_bldg_dishturret_cannon", Some("tur_weap_hoth_dishturret"), "ShotDelay 0.5, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0.2, HeatPerShot 0, HeatRecoverRate 0, HeatThreshold 0, RoundsPerClip 2, ReloadTime 0, MaxPressedTime 2"),
    Twin::new("hoth_weap_bldg_dishturret_cannon_ord", Some("tur_weap_hoth_dishturret_ord"), "MaxDamage 1000 1250 1500 3000, NextCharge 0.5 0.75 2, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("hoth_weap_bldg_dishturret_cannon_exp1", Some("tur_weap_recoilless_rifle_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 0, ShieldScale 1"),
    Twin::new("hoth_weap_bldg_dishturret_cannon_exp2", Some("tur_weap_recoilless_rifle_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 0, ShieldScale 1"),
    Twin::new("hoth_weap_bldg_dishturret_cannon_exp3", Some("tur_weap_recoilless_rifle_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 0, ShieldScale 1"),
    Twin::new("hoth_weap_bldg_dishturret_cannon_exp4", Some("tur_weap_recoilless_rifle_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 0, ShieldScale 1"),
    // Hoth turret (bf1_hot1): BF2's laser-mortar turret's laser.
    Twin::new("hoth_weap_bldg_hothturret_cannon", Some("tur_weap_hoth_lasermortar_laser"), "ShotDelay 0.22, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.12, HeatRecoverRate 0.25, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("hoth_weap_bldg_hothturret_cannon_ord", Some("tur_weap_hoth_lasermortar_laser_ord"), "MaxDamage 300, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    // Yavin tower turret (bf1_yav1): BF2's tower turret laser.
    Twin::new("yav_weap_bldg_tower_minigun", Some("tur_weap_laser"), "ShotDelay 0.22, SalvoCount 1, ShotsPerSalvo 1, SalvoDelay 0, InitialSalvoDelay 0, HeatPerShot 0.12, HeatRecoverRate 0.25, HeatThreshold 0.2, RoundsPerClip 0, ReloadTime 0"),
    Twin::new("yav_weap_bldg_tower_minigun_ord", Some("tur_weap_laser_ord"), "MaxDamage 100, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 0.1, BuildingScale 0.1, ShieldScale 1, 0xa74561b2 0.75, 0xee08d1ba 0.5, 0x955f71e9 100"),
    // V3: infantry weapons that take their BF2 twin's damage and scales whole.
    Twin::new("*_weap_inf_torpedo_launcher_ord", Some("com_weap_inf_rocket_launcher_ord"), "MaxDamage 500, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1, 0xa74561b2 0, 0xee08d1ba 0.5, 0x955f71e9 1500, TurnRate 2"),
    Twin::new("*_weap_inf_thermaldetonator_exp", Some("com_weap_inf_thermaldetonator_exp"), "MaxDamage 450, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("com_weap_inf_landmine_exp", Some("com_weap_inf_landmine_exp"), "MaxDamage 8000, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1.5, BuildingScale 0.1, ShieldScale 1"),
    Twin::new("all_weap_inf_remote_charge_dispenser_exp", Some("com_weap_inf_detpack_exp"), "MaxDamage 3000, PersonScale 0.4, AnimalScale 0.4, DroidScale 0.4, VehicleScale 1, BuildingScale 0.75, ShieldScale 1"),
    Twin::new("*_weap_inf_mortar_launcher_ord", Some("com_weap_inf_mortar_launcher_ord1"), "MaxDamage 100, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("*_weap_inf_mortar_launcher_ord1", Some("com_weap_inf_mortar_launcher_ord"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
    Twin::new("*_weap_inf_mortar_launcher_exp", Some("com_weap_inf_mortar_launcher_exp"), "MaxDamage 400, PersonScale 1, AnimalScale 1, DroidScale 1, VehicleScale 1, BuildingScale 1, ShieldScale 1"),
];

/// The [`HEALTH`] row of a BF1 vehicle or turret class.
pub fn health_row(class: &str) -> Option<&'static HealthRow> {
    HEALTH.iter().find(|r| r.class.eq_ignore_ascii_case(class))
}

/// The [`TWINS`] row of a BF1 class.
pub fn twin_row(class: &str) -> Option<&'static Twin> {
    TWINS.iter().find(|r| matches(r.class, class))
}

/// The names a twin key may be spelled under.
fn spellings(key: &str) -> Vec<u32> {
    if let Some(hex) = key.strip_prefix("0x") {
        return vec![u32::from_str_radix(hex, 16).expect("twin hash key")];
    }
    if key == "MaxDamage" {
        return vec![fnv1a_lowercase("MaxDamage"), fnv1a_lowercase("Damage")];
    }
    vec![fnv1a_lowercase(key)]
}

/// Write `row`'s numbers on `def`.
pub fn take(def: &mut ClassDef, row: &Twin) {
    let max_damage = fnv1a_lowercase("MaxDamage");
    for (key, values) in row.values() {
        let names = spellings(key);
        let at: Vec<usize> = (0..def.props.len())
            .filter(|&i| names.contains(&def.props[i].0))
            .collect();
        if values.len() > 1 {
            assert_eq!(at.len(), values.len(), "{} {key}", def.name);
            for (i, v) in at.into_iter().zip(values) {
                def.props[i].1 = format!("{v}");
            }
            continue;
        }
        let v = values[0];
        for &i in &at {
            def.props[i].1 = format!("{v}");
        }
        // A round that spells its damage `Damage` alone gets `MaxDamage`
        // too: the fat ray reads only that one.
        if at.is_empty()
            || (key == "MaxDamage" && !at.iter().any(|&i| def.props[i].0 == max_damage))
        {
            def.props.insert(0, (names[0], format!("{v}")));
        }
    }
}

/// [`VEHICLE_FACTOR`] on a [`VEHICLE_FACTOR_CLASSES`] class, and
/// [`LIGHT_ARMS_SCALE`] on a [`LIGHT_ARMS`] one.
pub fn against_vehicles(def: &mut ClassDef) {
    let [vehicle, armor] = ["VehicleScale", "ArmorScale"].map(fnv1a_lowercase);
    if VEHICLE_FACTOR_CLASSES.iter().any(|c| matches(c, &def.name))
        && !scale(def, vehicle, VEHICLE_FACTOR)
    {
        // `ArmorScale` sets vehicles and buildings together; the explicit
        // key moves vehicles alone.
        let v = first(def, armor).unwrap_or(1.0) * VEHICLE_FACTOR;
        def.props.insert(0, (vehicle, format!("{v}")));
    }
    if LIGHT_ARMS.iter().any(|c| matches(c, &def.name)) {
        super::set(def, &["VehicleScale"], LIGHT_ARMS_SCALE);
        super::set(def, &["BuildingScale"], LIGHT_ARMS_SCALE);
    }
}
