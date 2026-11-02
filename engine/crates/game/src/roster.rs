//! Faction rosters: baked class data → runtime unit specs. "Modes are
//! composable rulesets over rosters" (PLAN Architecture #6): a team is a
//! faction, a faction is a soldier class LIST from its side LVL, and each
//! class's constants/health/weapon come from the ODF chain — so era mix
//! (`--teams all,cis`) is just two rosters from two games.
//!
//! v1 (session 12): EVERY soldier class of a faction, and the mission's own
//! mix when the map ships one — BF1 `AddUnitClass(team, class, count)`, BF2
//! `SetupTeams{ side = { soldier = {class, min, max}, … } }`
//! (`game::mission::TeamDef::classes`). Class RONs come from `bake classes`
//! (assets_baked/classes/*.ron): both discs bake all four sides, and a round
//! loads the ones its MAP's disc holds — BF1 ALL/IMP/REP/CIS + COMMON, BF2
//! ALL/IMP/REP/CIS + INGAME (com_inf_default lives there).
//!
//! What is PROVISIONAL here is labelled at its definition: the DEFAULT class
//! lists, the `label` rule, and the `jet` route flag (no bot jets yet).

mod mines;
mod remotes;
#[cfg(test)]
mod secondary_tests;
mod turrets;
pub use remotes::{BLASTER_HUD, DESTRUCT_HUD};

use crate::classdb::{ClassDb, WeaponSlot};
use crate::mission::MissionDef;
use anyhow::{anyhow, Context, Result};
use core::flash::MuzzleFlashLook;
use core::impact::{ExplosionLook, ImpactLook};
use core::laser::LaserLook;
use core::sim::{DropTable, ItemClass, StationClass};
use core::soldier::SoldierConstants;
use core::unit::{SlotSpec, UnitClass};
use core::weapon::{
    ExplosionClass, HealthType, OrdKind, OrdnanceClass, PropSource, Secondary, Thrown, WeaponClass,
};
use std::path::Path;

/// A class viewed through the db's BASE chain.
pub struct ClassSrc<'a> {
    pub db: &'a ClassDb,
    pub class: &'a str,
}

impl PropSource for ClassSrc<'_> {
    fn prop(&self, key: &str) -> Option<&str> {
        self.db.prop(self.class, key)
    }
    fn prop_hash(&self, hash: u32) -> Option<&str> {
        self.db.prop_hash(self.class, hash)
    }
    fn prop_hash_all(&self, hash: u32) -> Vec<&str> {
        self.db.props_all_hash(self.class, hash)
    }
}

/// One soldier class of a faction, resolved through its ODF chain.
#[derive(Debug, Clone)]
pub struct ClassRoster {
    pub class: String,
    /// Display name. PROVISIONAL (lead): the ODF `Label` when it has one,
    /// else the capitalised `UnitType`, else the class name. BF2's real
    /// per-class names live in the localisation string tables, which are not
    /// baked — deferred.
    pub label: String,
    pub unit_type: String,
    pub consts: SoldierConstants,
    pub max_health: f32,
    pub health_type: HealthType,
    pub weapon: WeaponClass,
    /// Authored WeaponAmmo: total clips including a loaded clip; zero is unlimited.
    pub total_clips: u32,
    /// Every weapon slot of the class, in slot order
    /// ([`ClassDb::weapon_slots`]); `weapon` is the first channel-0 one.
    pub slots: Vec<WeaponSlot>,
    /// `weapon`'s slot index.
    pub weapon_slot: u8,
    /// The other slots the sim can run today ([`held_slots`]), in slot order.
    pub held: Vec<HeldSlot>,
    /// Class `GeometryName` (the soldier model to draw), if any.
    pub geometry: Option<String>,
    /// Primary weapon class `GeometryName` (drawn at `hp_weapons`), if any.
    pub weapon_geometry: Option<String>,
    pub first_person: Option<String>,
    pub first_person_weapon: crate::first_person::FpWeapon,
    /// The primary weapon in the HUD's weapon slot ([`weapon_hud`]).
    pub weapon_hud: crate::walk::WeaponHud,
    /// The hash of the class's `FoleyFXClass` (card FOLY), 0 without one.
    pub foley_class: u32,
    /// The class's `HurtSound` / `DeathSound` (card UCRY).
    pub cries: crate::walk::Cries,
    /// What picks the class's animation sets ([`class_anim`]).
    pub anim: crate::animator::ClassAnim,
    /// The class's override textures ([`class_skins`], card USKN).
    pub skins: ClassSkins,
    /// Rechargeable jet fuel (`SoldierConstants::has_jet`): permits
    /// `Arc::JET_JUMP` routes. A Jedi double jump does not grant jet routes.
    pub jet: bool,
    /// The jet's effect and engine sounds ([`jet_look`], card JETP2).
    pub jet_look: JetLook,
    /// What the class drops when it dies ([`drop_entries`], card PWUP).
    pub drops: Vec<DropEntry>,
    /// The droid's chunk death ([`chunk_death`], card DROIDDEATH).
    pub chunk: Option<core::unit::ChunkDeath>,
    /// Mission minimum (`AddUnitClass` count / `SetupTeams` slot min).
    pub min: u32,
    /// Mission maximum — BF2 only.
    pub max: Option<u32>,
    /// The ODF's `PointsToUnlock` (0 = none; `UnitClass::unlock_points`).
    pub unlock_points: u32,
    pub droideka: Option<std::sync::Arc<core::droideka::DroidekaClass>>,
    /// A droideka's shield in the HUD's secondary row: its `HUDTag`, from the
    /// play db, so both games show BF2's `hud_droideka_shield`.
    pub droideka_shield: Option<crate::walk::WeaponHud>,
}

/// One slot of a class's drop table: the item (`None` when the slot names a
/// class the db does not hold — its share drops nothing) and its probability.
#[derive(Debug, Clone)]
pub struct DropEntry {
    pub item: Option<ItemClass>,
    pub prob: f32,
}

/// A class slot past the primary, built for the sim (card GREN).
#[derive(Debug, Clone)]
pub struct HeldSlot {
    pub slot: u8,
    pub channel: u8,
    /// `WeaponAmmo` in clips; 0 = infinite.
    pub clips: u32,
    pub weapon: WeaponClass,
    /// The slot in the HUD's weapon rows ([`weapon_hud`]).
    pub hud: crate::walk::WeaponHud,
    /// The model drawn at `hp_weapons` while this slot is in hand
    /// ([`weapon_model`], card WHELD), if any.
    pub geometry: Option<String>,
    pub first_person: crate::first_person::FpWeapon,
}

/// Every soldier class a faction fields this round, in MISSION order.
#[derive(Debug, Clone)]
pub struct FactionRoster {
    pub faction: String,
    pub classes: Vec<ClassRoster>,
}

/// Why a class named by a mission (or a default list) did not make the
/// roster — reported, never silent.
#[derive(Debug, Clone)]
pub struct SkippedClass {
    pub class: String,
    pub reason: String,
}

/// The game a faction is native to — the era it fights in on its OWN disc.
/// This is NOT the db a round loads it from: both discs ship all four
/// factions (BF2 fields `all`/`imp` in its GCW missions, BF1 fields
/// `rep`/`cis` in its CW ones), and the db is chosen by the MAP's source
/// game instead (`class_files`).
pub fn faction_game(faction: &str) -> Option<&'static str> {
    Some(match faction {
        "all" | "imp" => "bf1",
        "rep" | "cis" => "bf2",
        _ => return None,
    })
}

/// Default matchup for a map's source game.
pub fn default_factions(game: &str) -> [&'static str; 2] {
    if game == "bf2" {
        ["rep", "cis"]
    } else {
        ["all", "imp"]
    }
}

/// Faction order inside a db's file list, so what a db contains is a
/// function of the SET of factions in play and not of the order the mission
/// happens to name them in.
pub const FACTION_ORDER: [&str; 4] = ["all", "imp", "rep", "cis"];

/// A game's shared class RON, loaded LAST so a side's own definition of a
/// name wins over it: BF1 COMMON, BF2 INGAME (`com_inf_default` lives there).
fn shared_file(game: &str) -> &'static str {
    if game == "bf2" {
        "bf2_ingame.ron"
    } else {
        "bf1_common.ron"
    }
}

/// A faction's side RON inside one game's bake.
fn side_file(game: &str, faction: &str) -> Option<&'static str> {
    Some(match (game, faction) {
        ("bf1", "all") => "bf1_all.ron",
        ("bf1", "imp") => "bf1_imp.ron",
        ("bf1", "rep") => "bf1_rep.ron",
        ("bf1", "cis") => "bf1_cis.ron",
        ("bf2", "all") => "bf2_all.ron",
        ("bf2", "imp") => "bf2_imp.ron",
        ("bf2", "rep") => "bf2_rep.ron",
        ("bf2", "cis") => "bf2_cis.ron",
        _ => return None,
    })
}

/// The class RONs a round needs, in load order (first definition wins): the
/// side RON of every faction in play, in [`FACTION_ORDER`], then the game's
/// shared RON.
///
/// `game` is the MAP's source game, not the faction's native one. Both discs
/// bake all four sides (`bake classes`), and a mission's class names are the
/// names ITS disc spells — BF2's GCW missions name `imp_inf_rifleman`, which
/// exists in `bf2_imp.ron` and in no BF1 file, and BF1's CW missions name
/// `rep_inf_clone_trooper`, which exists only in `bf1_rep.ron`. Resolving
/// either against the faction's native game is what made 18 of the 32 swept
/// maps fall back to the default list.
pub fn class_files(game: &str, factions: &[&str]) -> Vec<&'static str> {
    let mut files: Vec<&'static str> = Vec::new();
    for f in FACTION_ORDER {
        if factions.contains(&f) {
            if let Some(file) = side_file(game, f) {
                files.push(file);
            }
        }
    }
    files.push(shared_file(game));
    files
}

/// Load the class db for `factions` on a `game` map from `classes_dir`;
/// errors if none of its files exist.
pub fn load_classdb(classes_dir: &Path, game: &str, factions: &[&str]) -> Result<ClassDb> {
    let mut db = ClassDb::default();
    let mut loaded = 0;
    for f in class_files(game, factions) {
        let p = classes_dir.join(f);
        if pak::exists(&p) {
            db.load(&p)?;
            loaded += 1;
        }
    }
    if loaded == 0 {
        return Err(anyhow!(
            "no {game} class files in {} (run `bake classes` on the side LVLs)",
            classes_dir.display()
        ));
    }
    db.load_droid_chunks(&classes_dir.join("droid_chunks.ron"))?;
    // A BF1 side whose class takes a BF2 class's weapons loads BF2's side too.
    let mut donors: Vec<&str> = WEAPON_DONORS
        .iter()
        .filter_map(|d| d.class.split('_').next())
        .filter(|side| factions.contains(side))
        .collect();
    donors.sort_unstable();
    donors.dedup();
    if game == "bf1" && !donors.is_empty() {
        db.donor_weapons = Some(Box::new(load_classdb(classes_dir, "bf2", &donors)?));
    }
    Ok(db)
}

/// PROVISIONAL (lead, no RE): the class list a faction fields when the map
/// has no mission RON, or when none of the mission's classes are in the baked
/// db. Keyed by (MAP SOURCE GAME, faction), because the two discs spell the
/// same faction's classes differently — BF1's Alliance trooper is
/// `all_inf_soldier`, BF2's is `all_inf_rifleman`.
///
/// Each list is the MODAL mission mix for that pair, measured over the 57
/// shipped mission RONs (`engine/missions/*.ron`, `TeamDef::classes`): the
/// plain trooper FIRST (it is the fill class `ai_class` hands every spawn
/// past the minimums), then the heavy, the pilot/engineer, the sniper and the
/// specialist. `cis_inf_droideka` is in the modal mix of both CIS pairs; its
/// BASE chain ends at `walkerdroid`, so it builds as a droideka.
pub const DEFAULT_CLASSES: &[(&str, &str, &[&str])] = &[
    (
        "bf1",
        "all",
        &[
            "all_inf_soldier",
            "all_inf_vanguard",
            "all_inf_pilot",
            "all_inf_marksman",
            "all_inf_smuggler",
        ],
    ),
    (
        "bf1",
        "imp",
        &[
            "imp_inf_storm_trooper",
            "imp_inf_shock_trooper",
            "imp_inf_pilottie",
            "imp_inf_scout_trooper",
            "imp_inf_dark_trooper",
        ],
    ),
    (
        "bf1",
        "rep",
        &[
            "rep_inf_clone_trooper",
            "rep_inf_arc_trooper",
            "rep_inf_clone_pilot",
            "rep_inf_clone_sharpshooter",
            "rep_inf_jet_trooper",
        ],
    ),
    (
        "bf1",
        "cis",
        &[
            "cis_inf_battledroid",
            "cis_inf_assault",
            "cis_inf_pilotdroid",
            "cis_inf_assassindroid",
            "cis_inf_droideka",
        ],
    ),
    (
        "bf2",
        "all",
        &[
            "all_inf_rifleman",
            "all_inf_rocketeer",
            "all_inf_engineer",
            "all_inf_sniper",
            "all_inf_officer",
            "all_inf_wookiee",
        ],
    ),
    (
        "bf2",
        "imp",
        &[
            "imp_inf_rifleman",
            "imp_inf_rocketeer",
            "imp_inf_engineer",
            "imp_inf_sniper",
            "imp_inf_officer",
            "imp_inf_dark_trooper",
        ],
    ),
    (
        "bf2",
        "rep",
        &[
            "rep_inf_ep3_rifleman",
            "rep_inf_ep3_rocketeer",
            "rep_inf_ep3_engineer",
            "rep_inf_ep3_sniper",
            "rep_inf_ep3_officer",
            "rep_inf_ep3_jettrooper",
        ],
    ),
    (
        "bf2",
        "cis",
        &[
            "cis_inf_rifleman",
            "cis_inf_rocketeer",
            "cis_inf_engineer",
            "cis_inf_sniper",
            "cis_inf_officer",
            "cis_inf_droideka",
        ],
    ),
];

/// The default class list of a faction key on a `game` map.
pub fn default_classes(game: &str, faction: &str) -> Option<&'static [&'static str]> {
    DEFAULT_CLASSES
        .iter()
        .find(|(g, f, _)| *g == game && *f == faction)
        .map(|(_, _, c)| *c)
}

/// Does `class`'s BASE chain end at the engine's `soldier` base? The mission
/// lists name things that are not soldiers — `cis_inf_droideka` is a
/// `walkerdroid` ([`is_droideka_chain`]), BF1's heroes are soldiers but never
/// appear in a conquest `AddUnitClass` list.
pub fn is_soldier_chain(db: &ClassDb, class: &str) -> bool {
    db.engine_base(class) == Some("soldier")
}

pub fn is_droideka_chain(db: &ClassDb, class: &str) -> bool {
    db.engine_base(class)
        .is_some_and(|b| b.eq_ignore_ascii_case("walkerdroid"))
}

/// Primary weapon: the class's first channel-0 slot — the PS2's cur[0]
/// (`analysis/re-grenades.md` §2.2) — with its `WeaponAmmo` (0 when the slot
/// authors none or is infinite). A class with no channel-0 slot (BF1
/// `gun_inf_defender`'s neighbours) has cur[0] = -1 on the PS2; the port
/// takes its first slot (PROVISIONAL, no fielded class is one).
pub fn primary_weapon(db: &ClassDb, soldier: &str) -> Option<(String, u32)> {
    let slots = db.weapon_slots(soldier);
    let s = primary_slot(&slots)?;
    Some((s.weapon.clone(), s.ammo.unwrap_or(0)))
}

/// Keep the chosen slot's identity when multiple slots name one weapon.
fn primary_slot(slots: &[WeaponSlot]) -> Option<&WeaponSlot> {
    slots
        .iter()
        .find(|s| s.channel == 0)
        .or_else(|| slots.first())
}

/// BF2's size term of the aim error per soldier model (card CLSE2,
/// `analysis/re-soldier-ai.md` 18.3: B2 `FUN_002a76b8`), as the factor the
/// radius takes: 2 x class +0x370 = half the horizontal diagonal of the
/// model's bind-pose box. The box is the baked GLB's POSITION bounds less
/// its cloth segments (the PS2's RAM to four decimals), so the table is the bake's and
/// `tests/roster.rs` holds it to the models; a class takes its row at
/// [`ClassRoster::build`], and a model not listed keeps 1.0. It follows the
/// arm span: 0.93-1.02 arms out, 0.63-0.70 arms hanging, 0.48 the super
/// battle droid. BF1 has no error vector and no rows.
pub const AIM_SIZE: &[(&str, f32)] = &[
    ("all_inf_bothanspy", 0.955),
    ("all_inf_chewbacca", 0.9166),
    ("all_inf_deathstarleia", 0.8667),
    ("all_inf_engineer", 0.9707),
    ("all_inf_hothlukeskywalker", 0.9476),
    ("all_inf_luke", 0.9242),
    ("all_inf_marksperson", 0.6305),
    ("all_inf_markspersonsnow", 0.6416),
    ("all_inf_snowtrooper", 0.9705),
    ("all_inf_soldier", 0.6918),
    ("all_inf_stormtrooperhansolo", 0.9521),
    ("all_inf_stormtrooperlukeskywalker", 0.9521),
    ("all_inf_tantive4trooper", 0.676),
    ("all_inf_tatooinehansolo", 0.9261),
    ("all_inf_vanguard", 0.6651),
    ("all_inf_wookie", 1.021),
    ("cis_inf_bdroid", 0.9252),
    ("cis_inf_countdooku", 0.9474),
    ("cis_inf_darthmaul", 0.9478),
    ("cis_inf_generalgrevious", 0.9813),
    ("cis_inf_jangofett", 0.9586),
    ("cis_inf_magnaguard", 0.9793),
    ("cis_inf_sbdroid", 0.4805),
    ("imp_inf_atatcommander", 0.7025),
    ("imp_inf_atatpilot", 0.6697),
    ("imp_inf_bobafett", 0.9586),
    ("imp_inf_darktrooper", 0.9897),
    ("imp_inf_darthsidious", 0.9542),
    ("imp_inf_darthvader", 0.7986),
    ("imp_inf_gunner", 0.6825),
    ("imp_inf_scout", 0.961),
    ("imp_inf_shocktrooper", 0.9899),
    ("imp_inf_snowtrooper", 0.9702),
    ("imp_inf_stormtrooper", 0.954),
    ("rep_inf_aaylasecura", 0.9991),
    ("rep_inf_arctrooper", 0.9514),
    ("rep_inf_clonecommander", 0.9538),
    ("rep_inf_ep3anakin", 0.9426),
    ("rep_inf_ep3armoredpilot", 0.9519),
    ("rep_inf_ep3cloakedanakin", 0.9426),
    ("rep_inf_ep3heavytrooper", 0.9536),
    ("rep_inf_ep3jettrooper", 0.9669),
    ("rep_inf_ep3obiwan", 0.9426),
    ("rep_inf_ep3sniper", 0.9565),
    ("rep_inf_ep3trooper", 0.9548),
    ("rep_inf_feluciatrooper", 0.9633),
    ("rep_inf_jettrooper", 0.9673),
    ("rep_inf_kiadimundi", 0.9321),
    ("rep_inf_macewindu", 0.9426),
    ("rep_inf_pilot", 0.9507),
    ("rep_inf_trooper", 0.952),
    ("rep_inf_yoda", 0.5641),
];

/// A class's sound key (`HurtSound`, `DeathSound`: card UCRY) through its
/// base chain; None when unset or empty.
pub fn sound_key(db: &ClassDb, class: &str, key: &str) -> Option<String> {
    let v = db.prop(class, key)?.trim().trim_matches('"');
    (!v.is_empty()).then(|| v.to_owned())
}

/// A soldier class's jet look and sound (card JETP2, `analysis/re-jetpack.md`
/// §8): `JetEffect` (the effect a jet stance carries) and the class's
/// engine block - `EngineSound` (the parameterized loop), `TurnOnSound`,
/// `TurningOffSound`, `TurnOffSound` and `TurnOffTime` (the engine
/// component's 3.0 when unset; every jet class on both discs authors 0.0).
/// Presentation only: the sim never reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct JetLook {
    pub effect: Option<String>,
    pub engine: Option<String>,
    pub turn_on: Option<String>,
    pub turning_off: Option<String>,
    pub turn_off: Option<String>,
    pub turn_off_time: f32,
}

impl Default for JetLook {
    fn default() -> Self {
        Self {
            effect: None,
            engine: None,
            turn_on: None,
            turning_off: None,
            turn_off: None,
            turn_off_time: crate::engine_sound::TURN_OFF_TIME,
        }
    }
}

impl JetLook {
    /// Has the class an engine block worth running (a loop or a one-shot)?
    pub fn sounds(&self) -> bool {
        self.engine.is_some() || self.turn_on.is_some() || self.turn_off.is_some()
    }
}

/// [`JetLook`] of `class` through its base chain. Only a class with a jet
/// (`JetFuelRechargeRate` > 0, `SoldierConstants::has_jet`) or a hover
/// (`JetType hover`: Vader, the Emperor) gets one: a Jedi's double jump is
/// no jet stance and the PS2's other `EngineSound` readers are vehicles.
pub fn jet_look(db: &ClassDb, class: &str) -> JetLook {
    let c = SoldierConstants::from_props(&ClassSrc { db, class });
    if !c.has_jet() && !c.jet_hover {
        return JetLook::default();
    }
    engine_block(db, class)
}

pub fn engine_block(db: &ClassDb, class: &str) -> JetLook {
    JetLook {
        effect: sound_key(db, class, "JetEffect"),
        engine: sound_key(db, class, "EngineSound"),
        turn_on: sound_key(db, class, "TurnOnSound"),
        turning_off: sound_key(db, class, "TurningOffSound"),
        turn_off: sound_key(db, class, "TurnOffSound"),
        turn_off_time: db
            .prop_f32(class, "TurnOffTime")
            .unwrap_or(crate::engine_sound::TURN_OFF_TIME),
    }
}

/// A soldier class's voice ([`crate::walk::Cries`]): the sound property
/// each of its cry, call and squad-talk keys names through its base chain,
/// and its `LowHealthThreshold`.
pub fn cries(db: &ClassDb, class: &str) -> crate::walk::Cries {
    crate::walk::Cries {
        hero: crate::heroes::HeroAudio::from_db(db, class),
        hurt: sound_key(db, class, "HurtSound"),
        death: sound_key(db, class, "DeathSound"),
        acquired: sound_key(db, class, "AcquiredTargetSound"),
        flee: sound_key(db, class, "FleeSound"),
        heard: sound_key(db, class, "HeardEnemySound"),
        follow: sound_key(db, class, "AISCFieldFollowSound"),
        move_out: sound_key(db, class, "AISCFieldMoveOutSound"),
        need_gunner: sound_key(db, class, "AISCDriverGetInSound"),
        yes: sound_key(db, class, "AISCResponseYesSirSound"),
        low_health: sound_key(db, class, "LowHealthSound"),
        low_threshold: db
            .prop_f32(class, "LowHealthThreshold")
            .unwrap_or(crate::walk::LOW_HEALTH_THRESHOLD),
        vo: std::sync::Arc::new(crate::vo::class_lines(db, class)),
    }
}

/// The model a soldier class holds at `hp_weapons`: its [`primary_weapon`]'s
/// `GeometryName` through the weapon's BASE chain. `None` when the weapon has
/// none — BF2 `cis_inf_rifleman`'s `cis_weap_inf_wrist_blaster` fires from
/// the super battle droid's wrists and draws no gun.
pub fn weapon_geometry(db: &ClassDb, soldier: &str) -> Option<String> {
    let (weapon, _) = primary_weapon(db, soldier)?;
    weapon_model(db, &weapon)
}

/// The model a weapon class is drawn as in a soldier's hand: its
/// `GeometryName` through the BASE chain (the soldier render draws the
/// active weapon at `hp_weapons`, `analysis/re-muzzle-flash.md` §5;
/// `re-grenades.md` §2.3 item 4). `None` = drawn as nothing.
pub fn weapon_model(db: &ClassDb, weapon: &str) -> Option<String> {
    let mine = db
        .prop(weapon, "OrdnanceName")
        .filter(|n| db.engine_base(n.trim()) == Some("mine"));
    db.prop(weapon, "GeometryName")
        .or_else(|| mine.and_then(|_| db.prop(weapon, "OffhandGeometryName")))
        .or_else(|| mine.and_then(|n| db.prop(n.trim(), "GeometryName")))
        .map(|g| g.trim().to_owned())
}

pub fn first_person_model(db: &ClassDb, class: &str) -> Option<String> {
    let model = db.prop(class, "FirstPerson")?.rsplit(';').next()?.trim();
    (!model.is_empty()).then(|| model.to_ascii_lowercase())
}

pub fn first_person_weapon(
    db: &ClassDb,
    weapon: &str,
    offhand: bool,
) -> crate::first_person::FpWeapon {
    let geometry = db
        .prop(weapon, "HighResGeometry")
        .map(|g| g.trim().to_ascii_lowercase())
        .filter(|g| !g.is_empty());
    let bank = db
        .prop(weapon, "AnimationBank")
        .and_then(|v| v.split_whitespace().next().map(str::to_ascii_lowercase));
    let row = if db.is_bf2() {
        let base = bank
            .as_deref()
            .map(|b| b.split_once('_').map_or(b, |(_, tail)| tail));
        crate::first_person::bf2_row(offhand, base)
    } else {
        let set = bank
            .as_deref()
            .and_then(manifest::anim_sets::bf1::weapon_set)
            .unwrap_or(0);
        crate::first_person::bf1_row(set)
    };
    crate::first_person::FpWeapon { geometry, row }
}

/// A soldier class's skins (card USKN, `analysis/re-soldier-skeleton-skins.md`
/// 9 [O]): `OverrideTexture` replaces the texture of its model's
/// `override_texture` group, `OverrideTexture2` that of `override_texture2` -
/// the second only with the first (BF1 `FUN_002013a0`, BF2 `FUN_00239bb0`).
/// Lowercase texture names; `None` = the model's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassSkins {
    pub texture: Option<String>,
    pub texture2: Option<String>,
}

impl ClassSkins {
    /// The override of a model group, if the class has one.
    pub fn of_group(&self, group: &str) -> Option<&str> {
        match group {
            "override_texture" => self.texture.as_deref(),
            "override_texture2" => self.texture2.as_deref(),
            _ => None,
        }
    }
}

/// [`ClassSkins`] of a class through its BASE chain.
pub fn class_skins(db: &ClassDb, class: &str) -> ClassSkins {
    let tex = |key: &str| {
        db.prop(class, key)
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| !v.is_empty())
    };
    let texture = tex("OverrideTexture");
    ClassSkins {
        texture2: texture.as_ref().and(tex("OverrideTexture2")),
        texture,
    }
}

/// What picks a soldier class's animation sets
/// (`analysis/re-soldier-anim-layers.md` §4, §6.2): the first word of its
/// `AnimationName` / `SkeletonName` (BF2's skeleton family; `human` without
/// one) and, per weapon slot, the first word of the weapon's
/// `ComboAnimationBank` / `CustomAnimationBank` / `AnimationBank` through its
/// BASE chain. A weapon without one (grenades, detpacks) has no entry.
pub fn class_anim(
    db: &ClassDb,
    class: &str,
    slots: &[crate::classdb::WeaponSlot],
) -> crate::animator::ClassAnim {
    let word = |v: &str| v.split_whitespace().next().map(str::to_ascii_lowercase);
    let family = ["AnimationName", "SkeletonName"]
        .iter()
        .find_map(|k| db.prop(class, k).and_then(word))
        .unwrap_or_else(|| "human".to_owned());
    let banks = slots
        .iter()
        .filter_map(|s| {
            let weapon = db.resolve(&s.weapon)?;
            let bank = ["ComboAnimationBank", "CustomAnimationBank", "AnimationBank"]
                .iter()
                .find_map(|k| db.prop(weapon, k).and_then(word))?;
            Some((s.weapon.to_ascii_lowercase(), bank))
        })
        .collect();
    // SkeletonRootScale counts only with an own skeleton (BF1 B1:123186-123194,
    // BF2 B2:172070-172080; re-soldier-skeleton-skins 3).
    let own = db
        .prop(class, "SkeletonName")
        .and_then(word)
        .is_some_and(|s| !s.starts_with("human"));
    let root_scale = own
        .then(|| db.prop_f32(class, "SkeletonRootScale"))
        .flatten()
        .unwrap_or(1.0);
    crate::animator::ClassAnim {
        family,
        root_scale,
        banks,
        custom: Default::default(),
    }
}

/// What the HUD's weapon slot is told about `weapon` (`analysis/re-hud.md`
/// §3 `[V][O]`, BF2 `FUN_00326e28`): its `HUDTag` — inherited through the
/// BASE chain — else the class's OWN `GeometryName` (not inherited: a class
/// without one sends its own name), else its name. `icon` is the BF1
/// `IconTexture`, through the chain.
pub fn weapon_hud(db: &ClassDb, weapon: &str) -> crate::walk::WeaponHud {
    let clean = |s: &str| s.trim().to_ascii_lowercase();
    let own_geometry = db.get(weapon).and_then(|d| {
        let h = crate::classdb::fnv1a_lowercase("GeometryName");
        d.props.iter().find(|p| p.0 == h).map(|p| clean(&p.1))
    });
    let key = db
        .prop(weapon, "HUDTag")
        .map(clean)
        .filter(|t| !t.is_empty())
        .or(own_geometry.filter(|g| !g.is_empty()))
        .unwrap_or_else(|| clean(weapon));
    crate::walk::WeaponHud {
        key,
        class: clean(weapon),
        bf2: db.is_bf2(),
        icon: db
            .prop(weapon, "IconTexture")
            .map(clean)
            .filter(|i| !i.is_empty()),
        droid: (db.engine_base(weapon) == Some("remote"))
            .then(|| remotes::droid_hud(db, weapon))
            .flatten()
            .map(Box::new),
    }
}

/// A weapon's keys as one charge stage sees them (issue #29): the chain's,
/// base first, cut at the stage's end; the last of a key wins.
struct ListSrc<'a> {
    props: &'a [(u32, &'a str)],
}

impl PropSource for ListSrc<'_> {
    fn prop(&self, key: &str) -> Option<&str> {
        self.prop_hash(crate::classdb::fnv1a_lowercase(key))
    }
    fn prop_hash(&self, hash: u32) -> Option<&str> {
        self.props.iter().rev().find(|p| p.0 == hash).map(|p| p.1)
    }
}

/// `NextCharge`, the shot-offset count and an offset pair's keys.
const NEXT_CHARGE: u32 = 0x8316_f444;
const TILT_COUNT: u32 = 0xa4df_afc0;
const TILT: u32 = 0xa412_0aa8;

/// Build a weapon class (+ its ordnance) from the db, with its charge
/// stages (issue #29, `analysis/re-weapon-charge.md` §1): a `NextCharge` in
/// the weapon's keys clones the class as it stands, and every key after it
/// goes to the clone; the stage's threshold is the `NextCharge` value.
pub fn weapon_from_db(db: &ClassDb, weapon: &str) -> Result<WeaponClass> {
    db.get(weapon)
        .with_context(|| format!("weapon class {weapon} not in class db"))?;
    let props = chain_props(db, weapon);
    let cuts: Vec<usize> = props
        .iter()
        .enumerate()
        .filter(|(_, p)| p.0 == NEXT_CHARGE)
        .map(|(i, _)| i)
        .collect();
    let mut w = weapon_stage(
        db,
        weapon,
        &props[..cuts.first().copied().unwrap_or(props.len())],
    )?;
    for (k, &at) in cuts.iter().enumerate() {
        let end = cuts.get(k + 1).copied().unwrap_or(props.len());
        w.stages.push(core::weapon::ChargeStage {
            threshold: props[at].1.trim().parse().unwrap_or(1.0),
            class: Box::new(weapon_stage(db, weapon, &props[..end])?),
            index: usize::MAX,
        });
    }
    w.give_sniper_assist();
    Ok(w)
}

/// An ordnance's charge points (issue #29, `analysis/re-weapon-charge.md`
/// §4): a `NextCharge` in the ordnance class clones it as it stands and the
/// keys after it go to the clone, so the root is the keys before the first
/// and each point the keys up to the next (a key the clone re-authors wins);
/// and an emitter's Damage when it authors MaxDamage beside it
/// (its damage blends between the two by the charge).
fn charge_points(db: &ClassDb, ord_name: &str, ord: &mut OrdnanceClass) {
    let Some(name) = db.resolve(ord_name) else {
        return;
    };
    let props = chain_props(db, name);
    let cuts: Vec<usize> = props
        .iter()
        .enumerate()
        .filter(|(_, p)| p.0 == NEXT_CHARGE)
        .map(|(i, _)| i)
        .collect();
    if let Some(&first) = cuts.first() {
        let upto = |end: usize| {
            OrdnanceClass::from_props(&ListSrc {
                props: &props[..end],
            })
        };
        let root = upto(first);
        (ord.velocity, ord.life_span, ord.max_damage) =
            (root.velocity, root.life_span, root.max_damage);
        (ord.scales, ord.shield_scale) = (root.scales, root.shield_scale);
        for (k, &at) in cuts.iter().enumerate() {
            let o = upto(cuts.get(k + 1).copied().unwrap_or(props.len()));
            ord.knots.push(core::weapon::OrdKnot {
                at: props[at].1.trim().parse().unwrap_or(1.0),
                velocity: o.velocity,
                life_span: o.life_span,
                damage: o.max_damage,
                scales: o.scales,
                shield_scale: o.shield_scale,
            });
        }
    }
    if db
        .engine_base(name)
        .is_some_and(|b| b.eq_ignore_ascii_case("emitterordnance"))
        && db.prop(name, "MaxDamage").is_some()
    {
        ord.min_damage = db.prop_f32(name, "Damage");
    }
}

fn fired_ordnance(db: &ClassDb, ord_name: &str) -> OrdnanceClass {
    let mut ord = OrdnanceClass::from_props(&ClassSrc {
        db,
        class: ord_name,
    });
    ord.laser = laser_look(db, ord_name);
    ord.impact = impact_look(db, ord_name);
    ord.explosion = impact_explosion(db, ord_name).and_then(|e| explosion_class(db, e));
    ord.kind = db
        .engine_base(ord_name)
        .map_or(OrdKind::Other, OrdKind::from_base);
    charge_points(db, ord_name, &mut ord);
    if ord.kind == OrdKind::Missile {
        // Its steering keys (card UORD §4.1): TurnRate, the waver pair, the
        // ramp period, and BF2's MinSpeed / Acceleration.
        let mut m = core::missile::MissileClass::from_props(&ClassSrc {
            db,
            class: ord_name,
        });
        m.ramps = db.is_bf2();
        ord.missile = Some(m);
    }
    if db
        .engine_base(ord_name)
        .is_some_and(|b| b.eq_ignore_ascii_case("fatray"))
    {
        ord.fat_ray = Some(core::weapon::FatRayClass::from_props(&ClassSrc {
            db,
            class: ord_name,
        }));
    }
    if ord.kind == OrdKind::Beam {
        let beam = core::weapon::BeamClass::from_props(&ClassSrc {
            db,
            class: ord_name,
        });
        let look = ord.impact.get_or_insert_with(ImpactLook::default);
        look.terrain = None;
        look.water = None;
        for slot in [
            &mut look.soft,
            &mut look.rigid,
            &mut look.building,
            &mut look.shield,
        ] {
            slot.clone_from(&beam.impact);
        }
        ord.beam = Some(beam);
    }
    if ord.kind == OrdKind::Shell {
        // Its fall, and its burst when its life runs out in the air
        // (re-ordnance-kinds §6).
        ord.gravity = db.prop_f32(ord_name, "Gravity").unwrap_or(0.0);
        ord.expire = db
            .prop(ord_name, "ExplosionExpire")
            .and_then(|e| explosion_class(db, e));
    }
    ord
}

/// A weapon's secondary ordnance (card UORD, `analysis/re-ordnance-kinds.md`
/// §7): `SecondaryOrdnanceName` / `SecondaryOrdnancePeriod` (B2:324171-324181),
/// the TRACER spawned beside the primary every `period`-th round. Read from
/// the stage's replayed keys `src`, as the other weapon keys are. `None` when
/// the class names no ordnance, names one the db does not hold, or authors a
/// period under 1 (never); a missing period is the PS2 class default, 1.
fn secondary_ordnance(db: &ClassDb, src: &ListSrc<'_>) -> Option<Secondary> {
    let name = src.prop("SecondaryOrdnanceName")?.trim();
    if name.is_empty() {
        return None;
    }
    let period = src
        .prop("SecondaryOrdnancePeriod")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(1.0);
    if period < 1.0 {
        return None;
    }
    let name = db.resolve(name)?;
    Some(Secondary {
        name: name.to_owned(),
        ordnance: fired_ordnance(db, name),
        period: period as u32,
        index: usize::MAX,
    })
}

/// One charge stage of a weapon (the root: every key before the first
/// `NextCharge`), from its replayed keys `props` (base first, file order).
fn weapon_stage(db: &ClassDb, weapon: &str, props: &[(u32, &str)]) -> Result<WeaponClass> {
    let wsrc = ListSrc { props };
    if db.is_bf2() && db.engine_base(weapon) == Some("melee") {
        let mut w = WeaponClass::from_props(weapon, &wsrc, OrdnanceClass::from_props(&wsrc));
        // Native base ctor0x327e98 sets TriggerSingle; melee keeps it.
        // Ordinary weapon defaults remain on their existing paths.
        w.trigger_single = wsrc
            .prop("TriggerSingle")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .is_none_or(|v| v != 0.0);
        w.ordnance.explosion = wsrc
            .prop("ExplosionName")
            .and_then(|n| explosion_class(db, n.trim()));
        w.rounds_per_clip = 0;
        w.flash = None;
        return Ok(w);
    }
    if db.is_bf2() && db.engine_base(weapon) == Some("areaeffectweapon") {
        let mut w = WeaponClass::from_props(weapon, &wsrc, OrdnanceClass::default());
        w.force = Some(
            core::hero_weapon::ForcePower::choke(&wsrc)
                .with_context(|| format!("{weapon}: unsupported area effect"))?,
        );
        w.rounds_per_clip = 0;
        w.flash = None;
        return Ok(w);
    }
    if db.engine_base(weapon) == Some("remote") {
        return remotes::remote_weapon(db, weapon, &wsrc);
    }
    if matches!(db.engine_base(weapon), Some("dispenser" | "detonator")) {
        let name = WeaponClass::ordnance_name(&wsrc)
            .with_context(|| format!("{weapon}: no OrdnanceName"))?;
        if db.engine_base(name.trim()) == Some("portableturret") {
            return turrets::turret_weapon(db, weapon, name.trim(), &wsrc);
        }
        if db.engine_base(name.trim()) == Some("mine") {
            let mut item = mines::mine_class(db, name.trim());
            item.remote = db.engine_base(weapon) == Some("detonator");
            let mut w = WeaponClass::from_props(weapon, &wsrc, OrdnanceClass::default());
            w.ordnance.geometry = item.geometry.clone();
            w.mine = Some(core::weapon::Dispenser::new(
                &mut w,
                &wsrc,
                db.is_bf2(),
                item,
            ));
            return Ok(w);
        }
        let item = item_class_from_db(db, &name)
            .with_context(|| format!("{weapon}: {name} is not a pickup"))?;
        let mut w = WeaponClass::from_props(weapon, &wsrc, OrdnanceClass::default());
        w.dispenser = Some(core::weapon::Dispenser::new(
            &mut w,
            &wsrc,
            db.is_bf2(),
            item,
        ));
        return Ok(w);
    }
    if db.engine_base(weapon) == Some("invisibility") {
        let mut w = WeaponClass::from_props(weapon, &wsrc, OrdnanceClass::default());
        w.cloak = Some(core::cloak::CloakClass::from_props(&wsrc));
        w.rounds_per_clip = 0;
        w.flash = None;
        return Ok(w);
    }
    if db.engine_base(weapon) == Some("repair") {
        let mut w = WeaponClass::from_props(weapon, &wsrc, OrdnanceClass::default());
        w.repair = Some(core::weapon::Repair::from_props(&wsrc));
        w.rounds_per_clip = 0;
        w.flash = None;
        return Ok(w);
    }
    let ord_name =
        WeaponClass::ordnance_name(&wsrc).with_context(|| format!("{weapon}: no OrdnanceName"))?;
    db.get(&ord_name)
        .with_context(|| format!("ordnance class {ord_name} not in class db"))?;
    if db
        .resolve(&ord_name)
        .and_then(|o| db.engine_base(o))
        .is_some_and(|b| b.eq_ignore_ascii_case("sticky"))
    {
        // A charged mortar round (issue #29): launched as the grenades are.
        let ord = sticky_ordnance(db, db.resolve(&ord_name).unwrap_or(&ord_name));
        return Ok(WeaponClass::from_props(weapon, &wsrc, ord));
    }
    let ord = fired_ordnance(db, &ord_name);
    let mut w = WeaponClass::from_props(weapon, &wsrc, ord);
    // An emitter's cone-and-chain law (card UORDE): the ordnance class
    // authors its own keys, so it is read separately from `fired_ordnance`.
    if db.engine_base(&ord_name) == Some("emitterordnance") {
        w.emitter = Some(core::weapon::EmitterOrdnance::from_props(
            &ClassSrc {
                db,
                class: &ord_name,
            },
            db.is_bf2(),
        ));
    }
    if db.is_bf2() && db.engine_base(weapon) == Some("meleethrow") {
        w.saber_throw = Some(
            core::hero_weapon::SaberThrow::from_props(
                &wsrc,
                &ClassSrc {
                    db,
                    class: &ord_name,
                },
            )
            .with_context(|| format!("{weapon}: incomplete saber throw data"))?,
        );
        w.rounds_per_clip = 0;
        w.flash = None;
    }
    if db.is_bf2() && db.engine_base(&ord_name) == Some("emitterordnance") {
        w.ability = core::weapon::Ability::from_props(&ClassSrc {
            db,
            class: &ord_name,
        });
        if wsrc
            .prop("EnergyDrain")
            .and_then(|v| v.parse::<f32>().ok())
            .is_some_and(|energy| energy > 0.0)
        {
            w.force = core::hero_weapon::ForcePower::emitter(
                &wsrc,
                &ClassSrc {
                    db,
                    class: &ord_name,
                },
            );
        }
    }
    // The dwell lock (card UORD §5): the PS2 parses `LockTime` in the
    // launcher setter alone, so a cannon that authors it (the wrist rockets)
    // never holds a lock. `weapon_stage` holds the replayed stage keys, so a
    // stage does not inherit its root's.
    if db.engine_base(weapon) == Some("launcher") {
        w.lock_time = wsrc
            .prop("LockTime")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0);
    }
    // The offsets: the count key starts a stage's table and its pairs follow
    // (the bowcaster authors one per stage, re-blaster-damage.md §3.3); a
    // stage that authors none after its count keeps the copied pairs.
    let last_count = props.iter().rposition(|p| p.0 == TILT_COUNT);
    let tilt_count = last_count
        .and_then(|i| props[i].1.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let pairs = |from: usize| {
        props[from..]
            .iter()
            .filter(|p| p.0 == TILT)
            .map(|p| p.1)
            .collect::<Vec<_>>()
    };
    let after = last_count.map_or_else(Vec::new, &pairs);
    let tilts = if after.len() >= tilt_count {
        after
    } else {
        pairs(0)
    };
    w.shot_tilts = tilts
        .into_iter()
        .take(tilt_count)
        .filter_map(|v| {
            let mut pair = v.split_whitespace().filter_map(|n| n.parse::<f32>().ok());
            Some((
                pair.next()? * core::weapon::DEG,
                pair.next()? * core::weapon::DEG,
            ))
        })
        .collect();
    w.flash = db
        .engine_base(weapon)
        .is_some_and(|b| {
            w.ability.is_none() && !NO_FLASH_BASES.iter().any(|n| b.eq_ignore_ascii_case(n))
        })
        .then(|| flash_look(db, weapon));
    // The tracer that rides this weapon's rounds (BF2's shotgun, chaingun and
    // award weapons; `analysis/re-ordnance-kinds.md` §7).
    w.secondary = secondary_ordnance(db, &wsrc);
    Ok(w)
}

/// A `sticky` ordnance class (a grenade's or a charged mortar round's): the
/// shell's defaults (`LifeSpan` 3.0, `Velocity` 35.0, §4.1) where the
/// bolt's would apply, the sticky's own keys replayed in file order, and its
/// `ExplosionExpire` class.
fn sticky_ordnance(db: &ClassDb, ord_name: &str) -> OrdnanceClass {
    let osrc = ClassSrc {
        db,
        class: ord_name,
    };
    let mut ord = OrdnanceClass::from_props(&osrc);
    if db.prop(ord_name, "Velocity").is_none() {
        ord.velocity = 35.0;
    }
    if db.prop(ord_name, "LifeSpan").is_none() {
        ord.life_span = 3.0;
    }
    ord.impact = impact_look(db, ord_name);
    let mut sticky = core::sticky::StickyClass::replay(chain_props(db, ord_name), db.is_bf2());
    sticky.explosion = db
        .prop(ord_name, "ExplosionExpire")
        .and_then(|e| explosion_class(db, e));
    ord.sticky = Some(sticky);
    ord.kind = OrdKind::Lobbed;
    ord
}

/// A thrown weapon (card GREN, `analysis/re-grenades.md` §3, §4): a
/// `grenade` weapon class whose ordnance is a `sticky`. `Ok(None)` for any
/// other pair — the Gungan `grenade` ordnance base, a dispenser, a cannon.
/// The weapon is the base parse plus the grenade class's defaults and keys
/// ([`Thrown::apply`]); the ordnance takes the shell's defaults (`LifeSpan`
/// 3.0, `Velocity` 35.0, §4.1) where the bolt's would apply, the sticky's
/// own keys replayed in file order, and its `ExplosionExpire` class. A BF1
/// class is then thrown as BF2 throws ([`Thrown::BF1_THROWS_AS_BF2`], card
/// GRNP).
pub fn thrown_weapon_from_db(db: &ClassDb, weapon: &str) -> Result<Option<WeaponClass>> {
    let is = |class: &str, base: &str| {
        db.engine_base(class)
            .is_some_and(|b| b.eq_ignore_ascii_case(base))
    };
    if !is(weapon, "grenade") {
        return Ok(None);
    }
    let wsrc = ClassSrc { db, class: weapon };
    let ord_name =
        WeaponClass::ordnance_name(&wsrc).with_context(|| format!("{weapon}: no OrdnanceName"))?;
    let ord_name = db
        .resolve(&ord_name)
        .with_context(|| format!("ordnance class {ord_name} not in class db"))?;
    if !is(ord_name, "sticky") {
        return Ok(None);
    }
    let ord = sticky_ordnance(db, ord_name);
    let bf2 = db.is_bf2();
    let mut w = WeaponClass::from_props(weapon, &wsrc, ord);
    Thrown::apply(&mut w, &wsrc, bf2);
    if !bf2 && Thrown::BF1_THROWS_AS_BF2 {
        Thrown::throw_as_bf2(&mut w);
    }
    Ok(Some(w))
}

/// A BF1 class that carries a BF2 class's weapons (one gameplay from the
/// better game, issue #27 / PR #38): BF1's `class` fields `donor`'s weapon
/// slots, ammunition and all, out of BF2's class data; every `twins` pair is
/// (the BF2 weapon, the BF1 weapon whose sounds and animation bank it takes
/// on a BF1 map). Add a row here for another class; no other code names one.
pub struct WeaponDonor {
    pub class: &'static str,
    pub donor: &'static str,
    pub twins: &'static [(&'static str, &'static str)],
}

pub const WEAPON_DONORS: &[WeaponDonor] = &[
    WeaponDonor {
        class: "cis_inf_battledroid",
        donor: "cis_inf_rifleman",
        twins: &[("cis_weap_inf_wrist_trishot", "cis_weap_inf_wrist_shrapnel")],
    },
    WeaponDonor {
        class: "cis_inf_droideka",
        donor: "cis_inf_droideka",
        twins: &[],
    },
];

/// How a weapon slot's item works in the port, read from its class data
/// (issue #42): the weapon's engine base and its ordnance's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// A `grenade` weapon whose ordnance is a `sticky`: thrown (card GREN).
    Thrown,
    /// A `repair` weapon (the fusion cutter).
    Repair,
    Dispenser,
    Ability,
    Cloak,
    Guided,
    Remote,
    Melee,
    Force,
    SaberThrow,
    /// Fired by the soldier's weapon: a `cannon` or `launcher`, whatever its
    /// channel, and the mortar launchers (a `grenade` weapon on channel 0):
    /// each charge stage's round flies as its ordnance does, a `sticky` one
    /// launched (BF1's smuggler's, a mortar's held round; issue #29).
    Fired,
    /// An item the port does not build yet; the slot is left out (the
    /// reason's issue).
    NotBuilt(&'static str),
}

/// [`SlotKind`] of `weapon` (resolved) in a slot of `channel`.
pub fn slot_kind(db: &ClassDb, weapon: &str, channel: u8) -> SlotKind {
    let base = db.engine_base(weapon).unwrap_or("").to_ascii_lowercase();
    let ord = db
        .prop(weapon, "OrdnanceName")
        .and_then(|o| db.resolve(o.trim()))
        .and_then(|o| db.engine_base(o))
        .unwrap_or("")
        .to_ascii_lowercase();
    if (base.as_str(), ord.as_str()) != ("remote", "flyer")
        && db
            .prop_f32(weapon, "MedalsTypeToUnlock")
            .is_some_and(|m| m != -1.0)
    {
        return SlotKind::NotBuilt("an award weapon, unlocked by a medal (no medals yet)");
    }
    match (base.as_str(), ord.as_str()) {
        ("repair", _) => SlotKind::Repair,
        ("grenade", "sticky") if channel == 1 => SlotKind::Thrown,
        ("dispenser" | "detonator", "mine") => SlotKind::Dispenser,
        ("remote", "droid") => SlotKind::Remote,
        ("remote", "flyer") => SlotKind::Guided,
        ("remote", _) => SlotKind::NotBuilt("an unsupported remote entity"),
        ("dispenser", "portableturret") => SlotKind::Dispenser,
        ("dispenser", "powerupitem") => SlotKind::Dispenser,
        ("cannon", "emitterordnance") if channel == 1 => {
            let ability = db.prop(weapon, "OrdnanceName").and_then(|name| {
                core::weapon::Ability::from_props(&ClassSrc {
                    db,
                    class: name.trim(),
                })
            });
            if db.is_bf2() && ability.is_some() {
                SlotKind::Ability
            } else if db.is_bf2()
                && db.prop(weapon, "OrdnanceName").is_some_and(|name| {
                    core::hero_weapon::ForcePower::emitter(
                        &ClassSrc { db, class: weapon },
                        &ClassSrc {
                            db,
                            class: name.trim(),
                        },
                    )
                    .is_some()
                })
            {
                SlotKind::Force
            } else {
                SlotKind::NotBuilt("officer and hero abilities (issue #47)")
            }
        }
        ("invisibility", _) => SlotKind::Cloak,
        ("melee", _) if db.is_bf2() => SlotKind::Melee,
        ("meleethrow", _) if db.is_bf2() => SlotKind::SaberThrow,
        ("areaeffectweapon", _) if db.is_bf2() => SlotKind::Force,
        ("melee" | "meleethrow" | "areaeffectweapon", _) => {
            SlotKind::NotBuilt("sabres and force powers (the hero work)")
        }
        ("cannon" | "launcher" | "grenade", _) => SlotKind::Fired,
        _ => SlotKind::NotBuilt("an item type with no rule"),
    }
}

pub fn class_slot_kind(db: &ClassDb, soldier: &str, weapon: &str, channel: u8) -> SlotKind {
    match slot_kind(db, weapon, channel) {
        SlotKind::Ability if !soldier.contains("officer") && !soldier.contains("_hero_") => {
            SlotKind::NotBuilt("hero abilities (the hero work)")
        }
        kind => kind,
    }
}

pub fn held_slots(db: &ClassDb, soldier: &str, primary: u8) -> Vec<HeldSlot> {
    db.weapon_slots(soldier)
        .into_iter()
        .filter(|s| s.slot != primary)
        .filter_map(|s| {
            let name = db.resolve(&s.weapon)?.to_owned();
            let weapon = match class_slot_kind(db, soldier, &name, s.channel) {
                SlotKind::Thrown => thrown_weapon_from_db(db, &name).ok()??,
                SlotKind::Repair
                | SlotKind::Fired
                | SlotKind::Dispenser
                | SlotKind::Ability
                | SlotKind::Cloak
                | SlotKind::Guided
                | SlotKind::Remote
                | SlotKind::Melee
                | SlotKind::Force
                | SlotKind::SaberThrow => weapon_from_db(db, &name).ok()?,
                SlotKind::NotBuilt(_) => return None,
            };
            Some(HeldSlot {
                slot: s.slot,
                channel: s.channel,
                clips: s.ammo.unwrap_or(0),
                hud: weapon_hud(db, &name),
                geometry: weapon_model(db, &name),
                first_person: first_person_weapon(db, &name, weapon.offhand_weapon),
                weapon,
            })
        })
        .collect()
}

/// Weapon labels whose render never reaches the flash draw, whatever their
/// `MuzzleFlash` says (`analysis/re-muzzle-flash.md` §5): the melee family
/// (WeaponMelee overrides the render slot — the sabres) and the thrown or
/// placed types, whose weapon+0xbc skips the render (Detonator, Dispenser,
/// Remote, Grenade).
const NO_FLASH_BASES: [&str; 6] = [
    "melee",
    "meleethrow",
    "grenade",
    "detonator",
    "dispenser",
    "remote",
];

/// A weapon class's muzzle flash (card FXMF, `analysis/re-muzzle-flash.md`
/// §1): the chain's PROPs replayed parent first, each class a copy that
/// attaches the db's game's flash model.
pub fn flash_look(db: &ClassDb, weapon: &str) -> MuzzleFlashLook {
    let chain = db.chain(weapon);
    MuzzleFlashLook::replay(
        chain
            .iter()
            .rev()
            .map(|d| d.props.iter().map(|p| (p.0, p.1.as_str()))),
        db.is_bf2(),
    )
}

/// An OrdnanceLaser class's look (card FXB, `analysis/re-bolt.md` §1): the
/// chain's PROPs replayed parent first, each class's in file order, through
/// the setters — `LaserLength` resets `GlowLength` / `BlurLength`, so the
/// order is the value. `None` unless the chain ends at the engine's `bolt`
/// or `laser` label.
pub fn laser_look(db: &ClassDb, ordnance: &str) -> Option<LaserLook> {
    let base = db.engine_base(ordnance)?;
    if !["bolt", "laser", "beam"]
        .iter()
        .any(|b| base.eq_ignore_ascii_case(b))
    {
        return None;
    }
    Some(LaserLook::replay(chain_props(db, ordnance)))
}

/// Engine labels whose setter chains into the bullet base that reads the
/// `ImpactEffect*` surface keys (callers of B1 FUN_002774b0 / B2
/// FUN_00314b98; `analysis/re-fx.md` §7, AGENT_C §2 note 8).
const BULLET_BASES: [&str; 8] = [
    "bullet",
    "bolt",
    "laser",
    "missile",
    "shell",
    "towcable",
    "fatray",
    "grapplinghook",
];

/// A class chain's PROPs, parent first, each class's in file order.
pub(crate) fn chain_props<'a>(db: &'a ClassDb, class: &str) -> Vec<(u32, &'a str)> {
    db.chain(class)
        .iter()
        .rev()
        .flat_map(|d| d.props.iter().map(|p| (p.0, p.1.as_str())))
        .collect()
}

/// An ordnance class's impact effects (card FXP3, [`core::impact`]): the
/// bullet base's surface slots (ctor defaults, then the chain), its
/// `TrailEffect` and its `ExplosionName` class's look. `None` when the class
/// is not in the db.
pub fn impact_look(db: &ClassDb, ordnance: &str) -> Option<ImpactLook> {
    let base = db.engine_base(ordnance)?;
    let bf2 = db.is_bf2();
    let mut l = ImpactLook::replay(chain_props(db, ordnance), bf2);
    if !BULLET_BASES.iter().any(|b| base.eq_ignore_ascii_case(b)) {
        // Only the ordnance base's TrailEffect applies.
        l = ImpactLook {
            trail: l.trail,
            ..ImpactLook::default()
        };
    }
    l.explosion = impact_explosion(db, ordnance).and_then(|e| explosion_look(db, e));
    Some(l)
}

/// The keys that set an ordnance's impact explosion (re-blaster-damage 7.3,
/// class +0x38 / +0x4c): `Explosion`, `ExplosionImpact` and `ExplosionName`.
const IMPACT_EXPLOSION_KEYS: [u32; 3] = [0x02bb_1fe0, 0x61a8_bb12, 0x9b30_e763];

/// An ordnance's impact explosion class name (card RKEX): the last of the
/// three keys in the chain's file order, base first - each writes the same
/// field. The BF2 EMP launcher names it by `ExplosionImpact`, the rocket
/// launchers by `ExplosionName`.
fn impact_explosion<'a>(db: &'a ClassDb, ordnance: &str) -> Option<&'a str> {
    chain_props(db, ordnance)
        .into_iter()
        .rev()
        .find(|(h, _)| IMPACT_EXPLOSION_KEYS.contains(h))
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty())
}

/// An `expc` class's numbers and look, `None` when the class is not in the
/// db.
/// Explosions whose push moves only bodies (the maintainer 2026-10-27, a named
/// deviation): the Magnaguard's Bulldog (BF2 `cis_weap_inf_bulldog`, Push 5)
/// sent troops flying since every push knocks the living (issue #32); it hits
/// harder than a rifle but does not throw like a grenade. Matched along the
/// class chain.
pub const PUSH_SPARES_THE_LIVING: &[&str] =
    &["com_weap_inf_bulldog_exp", "com_weap_inf_incinerator_exp"];

/// [ours] The chance a droid bursts into chunks when it dies, for every class
/// with a chunk death (maintainer 2026-10-30: "65% body fall, 35% chunks").
/// Both discs author `ChunkFrequency` 0.6 on every chunked droid.
pub const DROID_CHUNK_CHANCE: f32 = 0.35;

/// A soldier class's chunk death (card DROIDDEATH, `analysis/re-droid-death.md`
/// §1-§2): only a class with chunk geometry (`ChunkGeometryName` in its
/// chain: the CIS soldier droids of both discs) has one, with
/// [`DROID_CHUNK_CHANCE`] in place of its `ChunkFrequency` and its
/// `ExplosionName` class.
pub fn chunk_death(db: &ClassDb, class: &str) -> Option<core::unit::ChunkDeath> {
    db.prop(class, "ChunkGeometryName")
        .map(str::trim)
        .filter(|g| !g.is_empty())?;
    Some(core::unit::ChunkDeath {
        frequency: DROID_CHUNK_CHANCE,
        explosion: db
            .prop(class, "ExplosionName")
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .and_then(|n| explosion_class(db, n)),
    })
}

pub(crate) fn explosion_class(db: &ClassDb, name: &str) -> Option<ExplosionClass> {
    let e = db.resolve(name.trim())?;
    let mut x = ExplosionClass::from_props(&ClassSrc { db, class: e });
    x.spares_living = db.chain(e).iter().any(|d| {
        PUSH_SPARES_THE_LIVING
            .iter()
            .any(|n| d.name.eq_ignore_ascii_case(n))
    });
    if let Some(look) = explosion_look(db, e) {
        x.look = look;
    }
    Some(x)
}

/// An explosion class's `Effect` / `WaterEffect` ([`core::impact`]); `None`
/// when the class is not in the db.
pub fn explosion_look(db: &ClassDb, explosion: &str) -> Option<ExplosionLook> {
    db.get(explosion)?;
    Some(ExplosionLook::replay(
        chain_props(db, explosion),
        db.is_bf2(),
    ))
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// A `powerupitem` class's box (`analysis/re-powerups.md` §2.3): the bounds
/// of its model, read from the PS2's RAM — (model, half extents, centre).
const ITEM_BOXES: [(&str, [f32; 3], [f32; 3]); 4] = [
    (
        "item_powerup_ammo",
        [0.1255, 0.2390, 0.1202],
        [0.013, 0.239, 0.0],
    ),
    (
        "com_item_powerup_dual",
        [0.1565, 0.2504, 0.2451],
        [-0.058, 0.240, 0.022],
    ),
    (
        "item_powerup_health100",
        [0.1206, 0.2976, 0.1206],
        [0.0, 0.296, 0.0],
    ),
    (
        "item_powerup_health25",
        [0.0965, 0.2381, 0.0965],
        [0.0, 0.237, 0.0],
    ),
];

/// A `powerupitem` class's numbers (card PWUP, `re-powerups.md` §2.1, §2.2):
/// `SoldierHealth` / `SoldierAmmo` / BF2's `SoldierEnergy` and buff keys,
/// `LifeSpan`, and the box of its `GeometryName` (BF2's five small items
/// share health25's box, its health100 too). `None` when the
/// db has no such class or it is not a `powerupitem`.
pub fn item_class_from_db(db: &ClassDb, item: &str) -> Option<ItemClass> {
    let name = db.resolve(item)?;
    if db.engine_base(name) != Some("powerupitem") {
        return None;
    }
    let f = |key: &str| db.prop_f32(name, key);
    // A sound property's name: the value's first word ("<name> defer").
    let sound = |key: &str| {
        db.prop(name, key)
            .and_then(|v| v.split_whitespace().next())
            .map(str::to_owned)
    };
    let geometry = db
        .prop(name, "GeometryName")
        .map(|g| g.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let (_, half, centre) = ITEM_BOXES
        .iter()
        // BF2's `item_powerup_health100` model is the small one (§2.3).
        .find(|(m, _, _)| *m == geometry && !(db.is_bf2() && *m == "item_powerup_health100"))
        .copied()
        .unwrap_or(ITEM_BOXES[3]);
    Some(ItemClass {
        name: name.to_ascii_lowercase(),
        health: f("SoldierHealth").unwrap_or(0.0),
        ammo: f("SoldierAmmo").unwrap_or(0.0).max(0.0) as u32,
        energy: f("SoldierEnergy").unwrap_or(0.0),
        offense: (
            f("BuffOffenseTimer").unwrap_or(0.0),
            f("BuffOffenseMult").unwrap_or(1.0),
        ),
        defense: (
            f("BuffDefenseTimer").unwrap_or(0.0),
            f("BuffDefenseMult").unwrap_or(1.0),
        ),
        // The base item's default is FLT_MAX (§2.2); every disc item says 30.
        life_span: f("LifeSpan").unwrap_or(f32::MAX),
        half: half.into(),
        centre: centre.into(),
        geometry,
        pickup_sound: sound("PowerupSound"),
        land_sound: sound("CollisionOtherSound"),
    })
}

/// A `powerupstation` class's numbers (card PWST, `re-powerup-station.md`
/// §2.1): `PowerupDelay`, and per health type the health and clips one pass
/// gives (`SoldierHealth` / `SoldierAmmo` fill the person and droid slots,
/// then `Person..BuildingHealth` / `..Ammo` their own; no disc class authors
/// both forms), with the sounds the game layer plays. `None` when the db has
/// no such class or it is not a `powerupstation`.
pub fn station_class_from_db(db: &ClassDb, class: &str) -> Option<StationClass> {
    let name = db.resolve(class)?;
    if db.engine_base(name) != Some("powerupstation") {
        return None;
    }
    let f = |key: &str| db.prop_f32(name, key);
    let sound = |key: &str| {
        db.prop(name, key)
            .and_then(|v| v.split_whitespace().next())
            .map(str::to_owned)
    };
    const TYPES: [&str; 5] = ["Person", "Animal", "Droid", "Vehicle", "Building"];
    let mut health = [0.0f32; 5];
    let mut ammo = [0u32; 5];
    if let Some(h) = f("SoldierHealth") {
        (health[0], health[2]) = (h, h);
    }
    if let Some(a) = f("SoldierAmmo") {
        let a = a.max(0.0) as u32;
        (ammo[0], ammo[2]) = (a, a);
    }
    for (i, t) in TYPES.iter().enumerate() {
        if let Some(h) = f(&format!("{t}Health")) {
            health[i] = h;
        }
        if let Some(a) = f(&format!("{t}Ammo")) {
            ammo[i] = a.max(0.0) as u32;
        }
    }
    Some(StationClass {
        name: name.to_ascii_lowercase(),
        delay: f("PowerupDelay").unwrap_or(core::sim::STATION_DELAY_DEFAULT),
        health,
        ammo,
        geometry: db
            .prop(name, "GeometryName")
            .map(|g| g.trim().to_ascii_lowercase())
            .unwrap_or_default(),
        powerup_sound: sound("PowerupSound"),
        ambient_sound: sound("AmbientSound"),
        anim: station_anim(db, name),
        sphere: None,
        wreck_sphere: None,
        max_health: f("MaxHealth").unwrap_or(0.0),
        health_type: db
            .prop(name, "HealthType")
            .and_then(HealthType::parse)
            .unwrap_or(HealthType::Building),
        destroyed_geometry: db
            .prop(name, "DestroyedGeometryName")
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty()),
        explosion: db
            .prop(name, "ExplosionName")
            .and_then(|s| explosion_class(db, s)),
    })
}

/// A station class's animation keys (re-powerup-station §2.1, §6): the
/// class keeps the first 4 `ActiveRotateNode`s.
fn station_anim(db: &ClassDb, name: &str) -> core::sim::StationAnim {
    let node = |key: &str| {
        db.prop(name, key)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    core::sim::StationAnim {
        rotate_speed: db.prop_f32(name, "IdleRotateSpeed").unwrap_or(0.0),
        wait_time: db.prop_f32(name, "IdleWaitTime").unwrap_or(0.0),
        wobble_node: node("IdleWobbleNode"),
        wobble_factor: db.prop_f32(name, "IdleWobbleFactor").unwrap_or(0.0),
        rotate_nodes: db
            .props_all(name, "ActiveRotateNode")
            .into_iter()
            .map(|v| v.trim().to_owned())
            .take(4)
            .collect(),
        spin_node: node("ActiveSpinNode"),
    }
}

/// The stations of `instances` (card PWST, §2.2, §3): every instance whose
/// class is a `powerupstation`, at its origin with its `Radius` (an
/// instance without one has no region and serves nobody, §3: left out).
/// With `unbuilt_zones` (BF2) a class with a build zone
/// (`manifest::BUILD_ZONE_PROP`, the `*_unbuilt` stations) starts unbuilt
/// and gives nothing (§7): left out too. Registers the classes with `sim`.
pub fn add_stations(
    sim: &mut core::sim::Sim,
    instances: &[manifest::ManifestInstance],
    db: &ClassDb,
    unbuilt_zones: bool,
) -> usize {
    let radius_key = crate::classdb::fnv1a_lowercase("Radius");
    let mut n = 0;
    for (instance, inst) in instances.iter().enumerate() {
        let Some(name) = db.resolve(&inst.class).map(str::to_owned) else {
            continue;
        };
        if unbuilt_zones && db.prop_hash(&name, manifest::BUILD_ZONE_PROP).is_some() {
            continue;
        }
        let Some(class) = station_class_from_db(db, &name) else {
            continue;
        };
        let radius = inst
            .props
            .iter()
            .rev()
            .find(|(k, _)| *k == radius_key)
            .and_then(|(_, v)| v.trim().parse::<f32>().ok())
            .unwrap_or(0.0);
        if radius <= 0.0 {
            continue;
        }
        let c = sim.add_station_class(class);
        let class = &sim.station_classes[usize::from(c)];
        let mut s = core::sim::Station::new(c, glam::Vec3::from(inst.translation), radius);
        s.health = core::weapon::Health::new(class.max_health, class.health_type);
        s.instance = Some(instance as u32);
        s.transform = crate::viewer::instance_matrix(inst.rotation, inst.translation);
        s.team = inst
            .props
            .iter()
            .rev()
            .find(|(k, _)| *k == crate::classdb::fnv1a_lowercase("Team"))
            .and_then(|(_, v)| v.trim().parse::<u8>().ok())
            .filter(|t| (1..=2).contains(t))
            .map(|t| t - 1);
        s.motion = core::sim::StationMotion::new(instance as u32 + 1);
        sim.collision
            .bind_station(instance as u32, sim.stations.len() as u16);
        sim.stations.push(s);
        n += 1;
    }
    n
}

/// A soldier class's drop table with its items resolved
/// ([`ClassDb::drop_table`]).
pub fn drop_entries(db: &ClassDb, soldier: &str) -> Vec<DropEntry> {
    db.drop_table(soldier)
        .into_iter()
        .map(|(item, prob)| DropEntry {
            item: item_class_from_db(db, &item),
            prob,
        })
        .collect()
}

/// The `PowerupItem` pool's capacity on a map (`re-powerups.md` §2.4): 25,
/// but for the BF1 missions whose script calls
/// `SetMemoryPoolSize("PowerupItem", n)`. No BF2 mission names the pool.
pub fn item_pool(game: &str, map: &str) -> usize {
    // Each mission's code and the name its baked manifest carries.
    const BF1: [(&str, usize); 13] = [
        ("bes1", 40),
        ("geo1", 25),
        ("hot1", 60),
        ("hoth", 60),
        ("kas1", 40),
        ("kashyyyk1", 40),
        ("kas2", 30),
        ("nab1", 40),
        ("naboo1", 40),
        ("rhn1", 60),
        ("rhenvar1", 60),
        ("tat1", 60),
        ("yav2", 40),
    ];
    let map = map.to_ascii_lowercase();
    if !game.eq_ignore_ascii_case("bf1") {
        return core::sim::ITEM_POOL_DEFAULT;
    }
    BF1.iter()
        .find(|(m, _)| map.starts_with(m))
        .map_or(core::sim::ITEM_POOL_DEFAULT, |(_, n)| *n)
}

/// BF2's jet rules on every map (the maintainer, 2026-10-28; card JETP, a
/// named deviation `[ours]`): a BF1 class keeps its own disc numbers, and the
/// keys BF1 never authors - `JetAcceleration` and the `ControlSpeed jet` row
/// (re-jetpack.md §2.1) - come from BF2's class of the same role
/// (re-jetpack.md §2.2, §12). Rows: (BF1 class, its BF2 twin,
/// JetAcceleration, the jet row or None for the ctor default 0.3 / 0.3 / 1.0).
/// The Geonosian is a local class the bake does not carry, so it has no row.
/// `tests/suite/roster.rs` holds the numbers to the BF2 twins' baked classes.
pub const BF1_JET_BORROW: &[(&str, &str, f32, Option<[f32; 3]>)] = &[
    (
        "rep_inf_jet_trooper",
        "rep_inf_ep2_jettrooper",
        30.0,
        Some([1.5, 1.25, 1.25]),
    ),
    ("imp_inf_dark_trooper", "imp_inf_dark_trooper", 10.0, None),
    (
        "imp_inf_dark_troopersnow",
        "imp_inf_dark_trooper",
        10.0,
        None,
    ),
];

/// A soldier class's movement constants through its chain: the props, the
/// jet's class data ([`jet_class_data`]) and whether a channel-0 weapon is a
/// lightsaber (a `melee` weapon; re-jetpack.md §3.3).
pub fn soldier_constants(db: &ClassDb, class: &str) -> SoldierConstants {
    let mut c = SoldierConstants::from_props(&ClassSrc { db, class });
    jet_class_data(db, class, &mut c);
    if let Some(row) = db.chain(class).into_iter().find_map(|def| {
        def.props
            .iter()
            .rev()
            .filter(|p| p.0 == crate::classdb::fnv1a_lowercase("ControlSpeed"))
            .filter_map(|p| SoldierConstants::control_row(&p.1))
            .find(|(name, _)| name.eq_ignore_ascii_case("sprint"))
            .map(|(_, row)| row)
    }) {
        c.sprint_control = [row[0], row[1]];
    }
    c.saber = db.weapon_slots(class).iter().any(|s| {
        s.channel == 0
            && db
                .resolve(&s.weapon)
                .and_then(|w| db.engine_base(w))
                .is_some_and(|b| b.eq_ignore_ascii_case("melee"))
    });
    if c.saber {
        for def in db.chain(class).into_iter().rev() {
            for (key, section) in crate::droideka::camera_sections(db, &def.name) {
                if let Some(index) = [
                    "STAND",
                    "STANDZOOM",
                    "CROUCH",
                    "CROUCHZOOM",
                    "PRONE",
                    "PRONEZOOM",
                    "SPRINT",
                ]
                .iter()
                .position(|name| crate::classdb::fnv1a_lowercase(name) == key)
                {
                    c.hero_cameras[index] = Some(section);
                }
            }
        }
    }
    c
}

/// The class data `SoldierConstants::from_props` cannot read alone: the
/// `ControlSpeed jet` row, child-most first (the PS2 applies each class's
/// rows over its parent's, one stance at a time), and on a BF1 disc the
/// [`BF1_JET_BORROW`] numbers.
fn jet_class_data(db: &ClassDb, class: &str, c: &mut SoldierConstants) {
    let h = crate::classdb::fnv1a_lowercase("ControlSpeed");
    let row = db.chain(class).into_iter().find_map(|def| {
        def.props
            .iter()
            .filter(|p| p.0 == h)
            .filter_map(|p| SoldierConstants::control_row(&p.1))
            .find(|(name, _)| name.eq_ignore_ascii_case("jet"))
            .map(|(_, r)| r)
    });
    if let Some(r) = row {
        c.jet_control = r;
    }
    if db.is_bf2() {
        return;
    }
    if let Some(&(_, _, accel, row)) = BF1_JET_BORROW
        .iter()
        .find(|(bf1, ..)| bf1.eq_ignore_ascii_case(class))
    {
        if db.prop(class, "JetAcceleration").is_none() {
            c.jet_acceleration = accel;
        }
        if let Some(r) = row {
            c.jet_control = r;
        }
    }
}

impl ClassRoster {
    /// One class through its ODF chain, or the reason it cannot be fielded.
    pub fn build(db: &ClassDb, class: &str, min: u32, max: Option<u32>) -> Result<ClassRoster> {
        db.get(class)
            .with_context(|| format!("class {class} not in the baked class db"))?;
        let walker_droid = is_droideka_chain(db, class);
        if !is_soldier_chain(db, class) && !walker_droid {
            return Err(anyhow!(
                "{class}: base chain ends at {:?}, not \"soldier\" — not an \
                 infantry class",
                db.engine_base(class)
            ));
        }
        let donor = WEAPON_DONORS
            .iter()
            .find(|d| d.class.eq_ignore_ascii_case(class));
        let (weapon_db, weapon_owner, donated) = match (donor, db.donor_weapons.as_deref()) {
            (Some(d), Some(bf2)) if bf2.get(d.donor).is_some() => (bf2, d.donor, true),
            _ => (db, class, false),
        };
        let twins = donor.map_or(&[][..], |d| d.twins);
        let slots = weapon_db.weapon_slots(weapon_owner);
        let primary = primary_slot(&slots).with_context(|| format!("{class}: no WeaponName"))?;
        let (weapon_name, total_clips, weapon_slot) = (
            primary.weapon.clone(),
            primary.ammo.unwrap_or(0),
            primary.slot,
        );
        let mut weapon = weapon_from_db(weapon_db, &weapon_name)?;
        if db.is_bf2() && weapon_db.engine_base(&weapon_name) == Some("melee") {
            weapon.saber = Some(Box::new(crate::hero_blades::saber_class(
                weapon_db,
                class,
                &weapon_name,
            )?));
        }
        let mut held = held_slots(weapon_db, weapon_owner, weapon_slot);
        if db.is_bf2() && class.contains("_hero_") {
            for selected in
                std::iter::once(&mut weapon).chain(held.iter_mut().map(|s| &mut s.weapon))
            {
                if selected.force.is_none() {
                    if let Some(ord) = weapon_db.prop(&selected.name, "OrdnanceName") {
                        if weapon_db.engine_base(ord) == Some("emitterordnance") {
                            selected.force = core::hero_weapon::ForcePower::emitter(
                                &ClassSrc {
                                    db: weapon_db,
                                    class: &selected.name,
                                },
                                &ClassSrc {
                                    db: weapon_db,
                                    class: ord,
                                },
                            );
                        }
                    }
                }
            }
        }
        if held.iter().any(|slot| slot.weapon.saber_throw.is_some()) {
            anyhow::ensure!(weapon.saber.as_ref().is_some_and(|s| !s.throw_edges.is_empty()),
                "{class}: saber throw requires the authored primary weapon attachment and blade templates");
        }
        if donated {
            for selected in
                std::iter::once(&mut weapon).chain(held.iter_mut().map(|s| &mut s.weapon))
            {
                let name = twins
                    .iter()
                    .find(|(bf2, _)| *bf2 == selected.name)
                    .map_or(selected.name.as_str(), |(_, bf1)| *bf1);
                let native = weapon_from_db(db, name)?;
                selected.fire_sound = native.fire_sound;
                selected.fire_loop_sound = native.fire_loop_sound;
                selected.reload_sound = native.reload_sound;
                selected.empty_sound = native.empty_sound;
                selected.change_sound = native.change_sound;
                selected.ordnance.ordnance_sound = native.ordnance.ordnance_sound;
                selected.ordnance.collision_sound = native.ordnance.collision_sound;
                if let (Some(selected), Some(native)) =
                    (&mut selected.ordnance.impact, native.ordnance.impact)
                {
                    if let (Some(selected), Some(native)) =
                        (&mut selected.explosion, native.explosion)
                    {
                        selected.sound = native.sound;
                    }
                }
                if let (Some(selected), Some(native)) =
                    (&mut selected.ordnance.explosion, native.ordnance.explosion)
                {
                    selected.look.sound = native.look.sound;
                }
            }
        }
        let mut anim = class_anim(db, class, &db.weapon_slots(class));
        if donated {
            for (weapon, _) in &mut anim.banks {
                if let Some((bf2, _)) = twins.iter().find(|(_, bf1)| weapon == bf1) {
                    *weapon = (*bf2).into();
                }
            }
        }
        let unit_type = db
            .prop(class, "UnitType")
            .map(|u| u.trim().to_owned())
            .unwrap_or_default();
        let label = db
            .prop(class, "Label")
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| {
                if unit_type.is_empty() {
                    class.to_owned()
                } else {
                    capitalise(&unit_type)
                }
            });
        let mut consts = soldier_constants(db, class);
        if let Some(g) = db.prop(class, "GeometryName") {
            let g = g.trim();
            if let Some((_, v)) = AIM_SIZE.iter().find(|(m, _)| m.eq_ignore_ascii_case(g)) {
                consts.aim_size = *v;
            }
        }
        let first_person_weapon =
            first_person_weapon(weapon_db, &weapon_name, weapon.offhand_weapon);
        let jet = consts.has_jet();
        Ok(ClassRoster {
            class: class.to_owned(),
            label,
            unit_type,
            consts,
            max_health: if walker_droid { weapon_db } else { db }
                .prop_f32(class, "MaxHealth")
                .unwrap_or(300.0),
            health_type: db
                .prop(class, "HealthType")
                .and_then(HealthType::parse)
                .unwrap_or(HealthType::Person),
            weapon,
            total_clips,
            held,
            anim,
            skins: class_skins(db, class),
            slots,
            weapon_slot,
            geometry: db
                .prop(class, "GeometryName")
                .map(|g| g.trim().to_owned())
                .filter(|_| !walker_droid),
            weapon_geometry: weapon_geometry(db, class),
            first_person: first_person_model(db, class).filter(|_| !walker_droid),
            first_person_weapon,
            weapon_hud: weapon_hud(weapon_db, &weapon_name),
            foley_class: db
                .prop_hash(class, crate::foley::FOLEY_FX_CLASS)
                .map_or(0, |v| crate::classdb::fnv1a_lowercase(v.trim())),
            cries: cries(db, class),
            jet,
            jet_look: if walker_droid {
                engine_block(db, class)
            } else {
                jet_look(db, class)
            },
            drops: drop_entries(db, class),
            chunk: if walker_droid {
                None
            } else {
                chunk_death(db, class)
            },
            min,
            max,
            // `[ours]` (card UNLK1): BF1's specials take their BF2
            // counterpart's cost (BF1 authors none).
            unlock_points: db
                .prop_f32(class, "PointsToUnlock")
                .map(|p| p.max(0.0) as u32)
                .or_else(|| {
                    (!db.is_bf2())
                        .then(|| crate::unlocks::bf1_cost(class))
                        .flatten()
                })
                .unwrap_or(0),
            droideka_shield: walker_droid
                .then(|| crate::droideka::shield_weapon(weapon_db, class))
                .flatten()
                .map(|w| weapon_hud(weapon_db, &w)),
            droideka: walker_droid.then(|| {
                std::sync::Arc::new(crate::droideka::droideka_class(db, weapon_db, class))
            }),
        })
    }

    /// The `core` view of this class. `weapon_class` is the index the caller
    /// registered the primary weapon at (`Sim::add_weapon_class`), `slots`
    /// the held slots with theirs ([`ClassRoster::register`] does both).
    pub fn unit_class(
        &self,
        weapon_class: usize,
        slots: Vec<SlotSpec>,
        drops: DropTable,
    ) -> UnitClass {
        UnitClass {
            drops,
            chunk: self.chunk.clone(),
            consts: self.consts,
            max_health: self.max_health,
            health_type: self.health_type,
            weapon_class,
            total_clips: self.total_clips,
            weapon_slot: self.weapon_slot,
            slots: slots.into(),
            jet: self.jet,
            // Soldiers count toward captures (analysis/re-cp-capture.md); the
            // non-soldier chains are rejected before they get here.
            capture_posts: true,
            min: self.min,
            max: self.max,
            unlock_points: self.unlock_points,
            droideka: self.droideka.clone(),
        }
    }

    /// Register this class's weapons with `sim` — one `Sim::weapon_classes`
    /// entry per weapon NAME, tracked in `names` (a Vec scan: the sim never
    /// iterates a hash map) — and return its [`UnitClass`].
    pub fn register(
        &self,
        sim: &mut core::sim::Sim,
        names: &mut Vec<(String, usize)>,
    ) -> UnitClass {
        let mut index = |w: &WeaponClass| match names.iter().find(|(n, _)| *n == w.name) {
            Some((_, i)) => *i,
            None => {
                // Its charge stages first: each its own class, carried by
                // the rounds it fires (issue #29).
                let mut w = w.clone();
                for st in &mut w.stages {
                    st.index = sim.add_weapon_class((*st.class).clone());
                }
                let name = w.name.clone();
                let i = sim.add_weapon_class(w);
                names.push((name, i));
                i
            }
        };
        let primary = index(&self.weapon);
        let slots = self
            .held
            .iter()
            .map(|h| SlotSpec {
                slot: h.slot,
                channel: h.channel,
                weapon_class: index(&h.weapon),
                clips: h.clips,
            })
            .collect();
        // The drop table's items, one `Sim::item_classes` entry per name.
        let pairs: Vec<(u8, f32)> = self
            .drops
            .iter()
            .map(|d| {
                let item = d
                    .item
                    .as_ref()
                    .map_or(u8::MAX, |c| sim.add_item_class(c.clone()));
                (item, d.prob)
            })
            .collect();
        self.unit_class(primary, slots, DropTable::from_pairs(&pairs))
    }
}

impl FactionRoster {
    /// Every class the faction fields. `mission_classes` is the map's own mix
    /// (`game::mission::TeamDef::classes`).
    ///
    /// Skips are DATA, not errors: a mission naming a class this side LVL does
    /// not carry leaves the roster SHORT, and the caller reports the list. But
    /// a mix whose FIRST entry — the fill class, the plain trooper `ai_class`
    /// hands every spawn past the minimums — cannot be built is not a short
    /// mix, it is a broken one, and the whole thing is discarded for
    /// [`DEFAULT_CLASSES`].
    ///
    /// That rule is not decoration: it was MEASURED. It used to fire on 18 of
    /// the 32 swept maps, because the db was chosen by the FACTION's native
    /// game: every BF2 GCW mission's class list resolved against the BF1 db —
    /// where exactly one of the Empire's six names, `imp_inf_dark_trooper`,
    /// also exists — and without this rule the Imperial team on every BF2 GCW
    /// map came out as sixteen shotgun-armed dark troopers (the 32-map sweep
    /// went 26/32 -> 25/32, `stuck_max` sum 139,200 -> 270,840). The db is now
    /// chosen by the MAP's source game (`class_files`) and all four sides of
    /// both discs are baked, so the mission's own names resolve and the
    /// fallback is left for a map that ships no mission RON.
    pub fn build(
        db: &ClassDb,
        game: &str,
        faction: &str,
        mission_classes: &[(String, u32, Option<u32>)],
    ) -> Result<(FactionRoster, Vec<SkippedClass>)> {
        let defaults = default_classes(game, faction).with_context(|| {
            format!("unknown faction {faction} on a {game} map (all|imp|rep|cis)")
        })?;
        let mut skipped = Vec::new();
        let mut classes = Vec::new();
        if !mission_classes.is_empty() {
            for (name, min, max) in mission_classes {
                let name = name.to_ascii_lowercase();
                match ClassRoster::build(db, &name, *min, *max) {
                    Ok(c) => classes.push(c),
                    Err(e) => skipped.push(SkippedClass {
                        class: name,
                        reason: format!("{e:#}"),
                    }),
                }
            }
            // The fill class has to be the mission's OWN first class.
            let fill = mission_classes[0].0.to_ascii_lowercase();
            if classes.first().is_none_or(|c| c.class != fill) {
                classes.clear();
            }
        }
        if classes.is_empty() {
            // Either the map ships no mission, or none of its classes are in
            // this db. Either way the faction still has to field somebody.
            for name in defaults {
                match ClassRoster::build(db, name, 1, None) {
                    Ok(c) => classes.push(c),
                    Err(e) => skipped.push(SkippedClass {
                        class: (*name).to_owned(),
                        reason: format!("{e:#}"),
                    }),
                }
            }
        }
        if classes.is_empty() {
            return Err(anyhow!(
                "{faction}: no soldier class could be built ({} skipped)",
                skipped.len()
            ));
        }
        Ok((
            FactionRoster {
                faction: faction.to_owned(),
                classes,
            },
            skipped,
        ))
    }

    /// The class the AI fill with and the human starts as — the mission's
    /// first entry, which on every mission censused is the plain trooper.
    pub fn fill_class(&self) -> &ClassRoster {
        &self.classes[0]
    }

    /// Rosters for two faction keys on a `game` map, from ONE db built out of
    /// that map's own disc (`class_files`). `mission` supplies each team's
    /// class mix, matched to the team by POSITION (`MissionDef::factions` and
    /// `factions` are the same two teams in the same order).
    ///
    /// `game` is `Manifest::source_game` / `MissionDef::source_game` — the
    /// disc the MAP came off, which is also the disc its mission's class
    /// names are spelled in. The mission's ERA reaches this through
    /// `factions`: a GCW mission names `all`/`imp`, a CW one `rep`/`cis`, on
    /// either disc.
    ///
    /// The warnings are RETURNED, not printed. The 32-map sweep runs its maps
    /// on a worker pool and writes each map's result through one ordered
    /// buffer; a roster warning printed from here landed on stderr in
    /// whatever order the workers happened to reach it, so two identical
    /// sweeps differed. The caller decides where they go — the sweep puts
    /// them in that map's own block, the game prints them once at startup.
    pub fn pair(
        classes_dir: &Path,
        game: &str,
        factions: [&str; 2],
        mission: Option<&MissionDef>,
    ) -> Result<([FactionRoster; 2], Vec<String>)> {
        let db = load_classdb(classes_dir, game, &factions)?;
        let mut warnings = Vec::new();
        let mut out = Vec::new();
        for (i, f) in factions.into_iter().enumerate() {
            let mix: &[(String, u32, Option<u32>)] = mission
                .and_then(|m| m.teams.get(i))
                .map(|t| t.classes.as_slice())
                .unwrap_or(&[]);
            let (roster, skipped) = FactionRoster::build(&db, game, f, mix)?;
            for s in &skipped {
                warnings.push(format!(
                    "roster {f}: skipped class {} — {}",
                    s.class, s.reason
                ));
            }
            if !mix.is_empty() && roster.fill_class().class != mix[0].0.to_ascii_lowercase() {
                warnings.push(format!(
                    "roster {f}: the mission's fill class {} is not in the {game} class db \
                     ({} of {} classes missing) — falling back to the default list",
                    mix[0].0,
                    skipped.len(),
                    mix.len()
                ));
            }
            out.push(roster);
        }
        let b = out.pop().unwrap();
        let a = out.pop().unwrap();
        Ok(([a, b], warnings))
    }
}

#[cfg(test)]
mod tests {
    //! The secondary ordnance keys (`SecondaryOrdnanceName` /
    //! `SecondaryOrdnancePeriod`), without the discs: the data-gated suite
    //! checks the baked classes' own values.

    use super::*;
    use crate::classdb::{fnv1a_lowercase, ClassDef};

    /// One in-memory class row: name, base, props (the suite's `gcl16_db`
    /// shape, without a file).
    type Row<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

    /// A class db from in-memory rows, through the real loader.
    fn db(rows: &[Row<'_>]) -> ClassDb {
        let defs: Vec<ClassDef> = rows
            .iter()
            .map(|(name, base, props)| ClassDef {
                name: (*name).into(),
                kind: String::new(),
                base: (*base).into(),
                props: props
                    .iter()
                    .map(|(k, v)| (fnv1a_lowercase(k), (*v).into()))
                    .collect(),
            })
            .collect();
        let path = std::env::temp_dir().join(format!(
            "openbf-secondary-{}-{:?}.ron",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, ron::to_string(&defs).unwrap()).unwrap();
        let mut db = ClassDb::default();
        assert_eq!(db.load(&path).unwrap(), rows.len());
        std::fs::remove_file(&path).unwrap();
        db
    }

    const ROUND: Row<'static> = ("test_ord", "bolt", &[("MaxDamage", "65")]);
    const TRACER: Row<'static> = (
        "test_tracer",
        "bolt",
        &[
            ("MaxDamage", "0"),
            ("Velocity", "160"),
            ("LifeSpan", "0.15"),
        ],
    );

    fn gun(props: &[(&str, &str)]) -> WeaponClass {
        let d = db(&[ROUND, TRACER, ("test_gun", "cannon", props)]);
        weapon_from_db(&d, "test_gun").unwrap()
    }

    #[test]
    fn roster_reads_a_weapons_secondary_tracer_and_its_period() {
        let w = gun(&[
            ("OrdnanceName", "test_ord"),
            ("SecondaryOrdnanceName", "test_tracer"),
            ("SecondaryOrdnancePeriod", "5"),
        ]);
        let s = w.secondary.as_ref().expect("the tracer's keys");
        assert_eq!(s.name, "test_tracer");
        assert_eq!(s.period, 5);
        assert_eq!(s.ordnance.velocity, 160.0);
        assert_eq!(s.ordnance.life_span, 0.15);
        assert!(s.ordnance.is_tracer());
        // The sim registration fills this in; the parse leaves it unset.
        assert_eq!(s.index, usize::MAX);
    }

    #[test]
    fn roster_secondary_needs_a_known_class_and_a_period_of_one() {
        // No key at all: no secondary.
        assert!(gun(&[("OrdnanceName", "test_ord")]).secondary.is_none());
        // No period: the PS2 class default, 1 (every round).
        let every = gun(&[
            ("OrdnanceName", "test_ord"),
            ("SecondaryOrdnanceName", "test_tracer"),
        ]);
        assert_eq!(every.secondary.as_ref().map(|s| s.period), Some(1));
        // A period under 1 never spawns (`tur_weap_spa_cannon` authors 0).
        let never = gun(&[
            ("OrdnanceName", "test_ord"),
            ("SecondaryOrdnanceName", "test_tracer"),
            ("SecondaryOrdnancePeriod", "0"),
        ]);
        assert!(never.secondary.is_none());
        // A name the db does not hold is dropped, not defaulted.
        let missing = gun(&[
            ("OrdnanceName", "test_ord"),
            ("SecondaryOrdnanceName", "not_a_class"),
            ("SecondaryOrdnancePeriod", "1"),
        ]);
        assert!(missing.secondary.is_none());
    }
}
