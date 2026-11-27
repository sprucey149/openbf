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

pub mod awards;
mod mines;
mod remotes;
#[cfg(test)]
mod secondary_tests;
mod turrets;
use core::fmath::FloatExt;
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
    /// The class's `MinFootHeight` (`crate::foley::MIN_FOOT_HEIGHT` unset).
    pub min_foot_height: Option<f32>,
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
    pub flee_like_a_hero: bool,
    pub droideka: Option<std::sync::Arc<core::droideka::DroidekaClass>>,
    /// A droideka's shield in the HUD's secondary row: its `HUDTag`, from the
    /// play db, so both games show BF2's `hud_droideka_shield`.
    pub droideka_shield: Option<crate::walk::WeaponHud>,
    /// Built from BF2's side for a BF1 Hunt adaptation (a mission class BF1
    /// has no counterpart for, [`FactionRoster::pair`]): its weapons come
    /// from BF2's class data and it draws with BF2's models, animations and
    /// first-person arms (`ClassVisual::asset_game`).
    pub from_bf2_side: bool,
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
    if game == "bf2" {
        for extra in ["bf2_gar.ron", "bf2_jed.ron"] {
            let p = classes_dir.join(extra);
            if pak::exists(&p) {
                db.load(&p)?;
                loaded += 1;
            }
        }
    }
    if loaded == 0 {
        return Err(anyhow!(
            "no {game} class files in {} (run `bake classes` on the side LVLs)",
            classes_dir.display()
        ));
    }
    db.load_droid_chunks(&classes_dir.join("droid_chunks.ron"))?;
    // A BF1 side loads BF2's side too: its classes carry BF2's award weapons
    // (`awards`), and a `WEAPON_DONORS` class takes a BF2 class's whole kit.
    // Only a donor class needs BF2's data; without it the others keep their
    // disc kits.
    let sides: Vec<&str> = FACTION_ORDER
        .into_iter()
        .filter(|side| factions.contains(side))
        .collect();
    let donor = WEAPON_DONORS
        .iter()
        .filter_map(|d| d.class.split('_').next())
        .any(|side| sides.contains(&side));
    if game == "bf1" && !sides.is_empty() {
        match load_classdb(classes_dir, "bf2", &sides) {
            Ok(bf2) => db.donor_weapons = Some(Box::new(bf2)),
            Err(e) if donor => return Err(e),
            Err(_) => {}
        }
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
    ("all_inf_pilot", 0.6176),
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
    ("imp_inf_pilot", 0.674),
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
    ("rep_inf_ep3spacepilot", 0.9588),
    ("rep_inf_ep3trooper", 0.9548),
    ("rep_inf_feluciatrooper", 0.9633),
    ("rep_inf_jettrooper", 0.9673),
    ("rep_inf_kiadimundi", 0.9321),
    ("rep_inf_macewindu", 0.9426),
    ("rep_inf_pilot", 0.9507),
    ("rep_inf_trooper", 0.952),
    ("rep_inf_yoda", 0.5641),
];

/// Every baked soldier model's drawn head and torso per posture
/// ([`core::unit::StanceBody`]; OURS, `core::unit` body.rs): (game, model,
/// [stand head, crouch head, prone head, stand torso, crouch torso]) in
/// metres, x right, y up from the feet, z his facing, to the millimetre.
/// BF2's head and torso points on the baked rig in the rifle idles
/// (`human_rifle_{stand,crouch,prone}_idle_emote`, BF2's `_full`) at frame 4
/// (0.1333 s, BF2's sample time), `tests/roster.rs` holding it to the
/// models. Not listed: `geo_inf_acklay` (BF2 gives it prims of its own,
/// `re-hitzones.md` Part B 1), Ventress and Kit Fisto (no prone idle). A
/// class takes its model's row of the game it is drawn from
/// ([`stance_body`]). One pose per model, the rifle's; measure: the
/// head points in a hero's saber idles against these.
#[rustfmt::skip]
pub const STANCE_BODY: &[(&str, &str, [[f32; 3]; 5])] = &[
    ("bf2", "all_inf_bothanspy", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_chewbacca", [[-0.033, 1.922, 0.134], [-0.078, 1.450, 0.223], [-0.094, 0.349, 0.799], [0.118, 1.461, -0.164], [0.094, 1.068, -0.169]]),
    ("bf2", "all_inf_deathstarleia", [[-0.017, 1.541, 0.093], [-0.051, 1.157, 0.157], [-0.073, 0.280, 0.608], [0.109, 1.182, -0.150], [0.091, 0.862, -0.158]]),
    ("bf2", "all_inf_engineer", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_hothlukeskywalker", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.085, 0.311, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_luke", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_marksperson", [[-0.017, 1.541, 0.093], [-0.051, 1.157, 0.157], [-0.073, 0.280, 0.608], [0.109, 1.182, -0.150], [0.091, 0.862, -0.158]]),
    ("bf2", "all_inf_markspersonsnow", [[-0.017, 1.541, 0.093], [-0.051, 1.157, 0.157], [-0.073, 0.280, 0.608], [0.109, 1.182, -0.150], [0.091, 0.862, -0.158]]),
    ("bf2", "all_inf_pilot", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_snowtrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.085, 0.311, 0.700], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_soldier", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_stormtrooperhansolo", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_stormtrooperlukeskywalker", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_tantive4trooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_tatooinehansolo", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_vanguard", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "all_inf_wookie", [[-0.033, 1.922, 0.134], [-0.078, 1.450, 0.223], [-0.094, 0.349, 0.799], [0.118, 1.461, -0.164], [0.094, 1.068, -0.169]]),
    ("bf2", "cis_inf_bdroid", [[0.010, 1.788, 0.088], [-0.061, 1.386, 0.177], [-0.117, 0.447, 0.702], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "cis_inf_countdooku", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "cis_inf_darthmaul", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.085, 0.312, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "cis_inf_generalgrevious", [[0.066, 1.398, -0.117], [0.074, 1.054, -0.151], [-0.070, 1.070, 0.089], [0.059, 1.123, -0.241], [0.047, 0.772, -0.254]]),
    ("bf2", "cis_inf_jangofett", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "cis_inf_magnaguard", [[0.003, 1.895, 0.073], [-0.039, 1.408, 0.135], [-0.074, 0.339, 0.668], [0.124, 1.543, -0.173], [0.099, 1.119, -0.181]]),
    ("bf2", "cis_inf_sbdroid", [[0.010, 1.788, 0.088], [-0.061, 1.386, 0.177], [-0.117, 0.447, 0.702], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "ewk_inf", [[-0.028, 1.053, 0.143], [-0.062, 0.827, 0.208], [-0.071, 0.221, 0.579], [0.083, 0.712, -0.108], [0.069, 0.547, -0.107]]),
    ("bf2", "ewk_inf2", [[-0.028, 1.053, 0.143], [-0.062, 0.827, 0.208], [-0.071, 0.221, 0.579], [0.083, 0.712, -0.108], [0.069, 0.547, -0.107]]),
    ("bf2", "gam_inf_gamorreanguard", [[-0.024, 1.631, 0.128], [-0.063, 1.221, 0.201], [-0.085, 0.305, 0.657], [0.113, 1.210, -0.156], [0.094, 0.874, -0.167]]),
    ("bf2", "gar_inf_jediguard", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "gar_inf_nabooguard", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "gar_inf_nabooqueen", [[-0.017, 1.541, 0.093], [-0.051, 1.157, 0.157], [-0.073, 0.280, 0.608], [0.109, 1.182, -0.150], [0.091, 0.862, -0.158]]),
    ("bf2", "gen_inf_geonosian", [[0.017, 1.432, 0.073], [-0.017, 1.044, 0.110], [-0.062, 0.272, 0.446], [0.115, 1.131, -0.161], [0.100, 0.799, -0.179]]),
    ("bf2", "hot_inf_wampa", [[-0.044, 1.757, 0.157], [-0.088, 1.336, 0.250], [-0.097, 0.326, 0.789], [0.111, 1.279, -0.152], [0.089, 0.939, -0.156]]),
    ("bf2", "imp_inf_atatcommander", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_atatpilot", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_bobafett", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_darktrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_darthsidious", [[-0.016, 1.826, 0.116], [-0.058, 1.362, 0.189], [-0.086, 0.335, 0.699], [0.121, 1.405, -0.168], [0.099, 1.015, -0.178]]),
    ("bf2", "imp_inf_darthvader", [[-0.037, 1.942, 0.129], [-0.077, 1.436, 0.215], [-0.098, 0.344, 0.721], [0.128, 1.444, -0.181], [0.108, 1.023, -0.199]]),
    ("bf2", "imp_inf_gunner", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_pilot", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_scout", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_shocktrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_snowtrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "imp_inf_stormtrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "jed_inf_jedi", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "jed_inf_sith", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.085, 0.312, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "nab_inf_gungan", [[0.005, 1.677, 0.088], [-0.035, 1.243, 0.143], [-0.072, 0.312, 0.593], [0.118, 1.331, -0.164], [0.098, 0.960, -0.175]]),
    ("bf2", "rep_inf_aaylasecura", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_arctrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_clonecommander", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3anakin", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3armoredpilot", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3cloakedanakin", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3heavytrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3jettrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3obiwan", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3sniper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3spacepilot", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_ep3trooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_feluciatrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_jettrooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_kiadimundi", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_macewindu", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_pilot", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_trooper", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "rep_inf_yoda", [[-0.025, 0.638, 0.093], [-0.040, 0.487, 0.134], [-0.053, 0.132, 0.313], [0.080, 0.364, -0.104], [0.076, 0.264, -0.118]]),
    ("bf2", "tat_inf_jawa", [[-0.005, 0.972, 0.064], [-0.026, 0.730, 0.102], [-0.050, 0.185, 0.382], [0.092, 0.724, -0.123], [0.081, 0.530, -0.133]]),
    ("bf2", "tat_inf_tusken", [[-0.024, 1.673, 0.131], [-0.066, 1.261, 0.207], [-0.086, 0.313, 0.699], [0.112, 1.252, -0.154], [0.092, 0.914, -0.161]]),
    ("bf2", "wok_inf_wookiewarrior", [[-0.041, 1.770, 0.148], [-0.085, 1.349, 0.240], [-0.094, 0.327, 0.799], [0.110, 1.309, -0.150], [0.087, 0.967, -0.151]]),
    ("bf1", "all_inf_marksperson", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "all_inf_marksperson_snow", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "all_inf_pilot", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "all_inf_snowtrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "all_inf_soldier", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "all_inf_trooperfleet", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "all_inf_wookie", [[-0.073, 1.880, 0.124], [-0.158, 1.368, 0.199], [-0.094, 0.349, 0.799], [0.144, 1.471, -0.216], [0.138, 1.104, -0.214]]),
    ("bf1", "cis_inf_bdroid", [[-0.038, 1.754, 0.121], [-0.139, 1.305, 0.217], [-0.117, 0.447, 0.702], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "cis_inf_sbdroid", [[-0.038, 1.754, 0.121], [-0.139, 1.305, 0.217], [-0.117, 0.447, 0.702], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "ewk_inf", [[-0.070, 1.021, 0.131], [-0.128, 0.765, 0.184], [-0.071, 0.221, 0.579], [0.108, 0.723, -0.144], [0.107, 0.581, -0.142]]),
    ("bf1", "ewk_inf2", [[-0.070, 1.021, 0.131], [-0.128, 0.765, 0.184], [-0.071, 0.221, 0.579], [0.108, 0.723, -0.144], [0.107, 0.581, -0.142]]),
    ("bf1", "gar_inf_nabooguard", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "gen_inf_geonosian", [[-0.036, 1.403, 0.056], [-0.081, 0.988, 0.092], [-0.062, 0.272, 0.446], [0.128, 1.142, -0.196], [0.131, 0.831, -0.204]]),
    ("bf1", "imp_inf_atatpilot", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_atstpilot", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_darktrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_pilot", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_scout", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_shocktrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_snowtrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "imp_inf_stormtrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "nab_inf_gungan", [[-0.042, 1.644, 0.068], [-0.101, 1.180, 0.118], [-0.072, 0.312, 0.593], [0.138, 1.342, -0.209], [0.136, 0.994, -0.212]]),
    ("bf1", "rep_inf_arctrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "rep_inf_jettrooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "rep_inf_pilot", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "rep_inf_trooper", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "tat_inf_jawa", [[-0.021, 0.955, 0.053], [-0.058, 0.698, 0.086], [-0.050, 0.185, 0.382], [0.108, 0.736, -0.153], [0.112, 0.563, -0.157]]),
    ("bf1", "tat_inf_tusken", [[-0.071, 1.634, 0.120], [-0.145, 1.184, 0.185], [-0.086, 0.313, 0.699], [0.134, 1.262, -0.200], [0.132, 0.948, -0.200]]),
    ("bf1", "wok_inf_wookie", [[-0.073, 1.880, 0.124], [-0.158, 1.368, 0.199], [-0.094, 0.349, 0.799], [0.144, 1.471, -0.216], [0.138, 1.104, -0.214]]),
];

/// The [`STANCE_BODY`] row of `model` drawn from `game` ("bf1" / "bf2"),
/// else the other game's.
pub fn stance_body(game: &str, model: &str) -> Option<core::unit::StanceBody> {
    let row = |g: &str| {
        STANCE_BODY
            .iter()
            .find(|(rg, m, _)| *rg == g && m.eq_ignore_ascii_case(model))
    };
    let other = if game == "bf2" { "bf1" } else { "bf2" };
    let (_, _, p) = row(game).or_else(|| row(other))?;
    let v = |i: usize| glam::Vec3::from_array(p[i]);
    Some(core::unit::StanceBody {
        head: [v(0), v(1), v(2)],
        torso: [v(3), v(4)],
    })
}

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
        choke: sound_key(db, class, "ChokeSound"),
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
    let model = |key| {
        db.prop(weapon, key)
            .map(|g| g.trim().to_ascii_lowercase())
            .filter(|g| !g.is_empty())
    };
    // BF2 draws a weapon with no `HighResGeometry` (class+0x78) with its
    // `GeometryName` (+0x74) in first person too (`FUN_0032b228`,
    // `FUN_0033e018`). Every soldier weapon on the discs names one; a mod's
    // may not.
    let geometry = model("HighResGeometry")
        .or_else(|| db.is_bf2().then(|| model("GeometryName")).flatten());
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
    let mut banks = Vec::new();
    let mut fallback_banks = Vec::new();
    for s in slots {
        let Some(weapon) = db.resolve(&s.weapon) else {
            continue;
        };
        let Some(val) = ["ComboAnimationBank", "CustomAnimationBank", "AnimationBank"]
            .iter()
            .find_map(|k| db.prop(weapon, k))
        else {
            continue;
        };
        let mut words = val.split_whitespace();
        let Some(first) = words.next() else {
            continue;
        };
        let wname = s.weapon.to_ascii_lowercase();
        banks.push((wname.clone(), first.to_ascii_lowercase()));
        if let Some(second) = words.next() {
            if !second.eq_ignore_ascii_case("none") && !second.eq_ignore_ascii_case(first) {
                fallback_banks.push((wname, second.to_ascii_lowercase()));
            }
        }
    }
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
        fallback_banks,
        custom: Default::default(),
    }
}

/// The BF2 Ewok weapons a BF1 Hunt Ewok throws in place of its own
/// (`FactionRoster::pair`, issue #161).
pub const BF2_EWOK_WEAPONS: [&str; 2] = ["ewk_weap_inf_spear", "ewk_weap_inf_rock"];

/// Give a BF1 Hunt Ewok BF2's spear and rock in their own slots: the class,
/// its HUD row (BF2's, by class name), its model in hand and its first-person
/// row come from `donor`, BF2's Ewok side. The spear throws as a sticky;
/// the rock fires as BF2's shell with its blast.
fn donate_ewok_weapons(donor: &ClassDb, class: &mut ClassRoster) -> Result<()> {
    let swap = |weapon: &WeaponClass| -> Result<Option<WeaponClass>> {
        let Some(name) = BF2_EWOK_WEAPONS
            .iter()
            .find(|n| weapon.name.eq_ignore_ascii_case(n))
        else {
            return Ok(None);
        };
        let thrown = weapon_db::thrown_weapon(donor, name, true)?;
        Ok(Some(match thrown {
            Some(w) => w,
            None => weapon_from_db(donor, name)?,
        }))
    };
    if let Some(w) = swap(&class.weapon)? {
        class.weapon_hud = weapon_hud(donor, &w.name);
        class.first_person_weapon = first_person_weapon(donor, &w.name, w.offhand_weapon);
        class.weapon_geometry = donor.prop(&w.name, "GeometryName").map(str::to_owned);
        class.weapon = w;
    }
    for held in &mut class.held {
        if let Some(w) = swap(&held.weapon)? {
            held.hud = weapon_hud(donor, &w.name);
            held.first_person = first_person_weapon(donor, &w.name, w.offhand_weapon);
            held.geometry = donor.prop(&w.name, "GeometryName").map(str::to_owned);
            held.weapon = w;
        }
    }
    Ok(())
}

/// `[ours]` (issue #161): a distinct name for each Hunt native class. Both
/// games' text tables give one name to every class of a native faction
/// ("Ewok", "Wookiee", "Gungan"; BF1 Theed's guard and its pilot are both
/// "Naboo Guard"), and the classes' `UnitType`s collide too
/// (BF1's Ewok trooper, who carries the spear and the repair tool, and its
/// repair Ewok are both `repair`), so the class screen offered two Ewoks
/// with one name. The Hunt class screen shows these where the text table's
/// names repeat (`EventFeed::for_walk`). The names describe the class's
/// kit; neither game's tables have per-class native names.
pub const HUNT_CLASS_LABELS: &[(&str, &str)] = &[
    ("ewk_inf_trooper", "Ewok Warrior"),
    ("ewk_inf_scout", "Ewok Scout"),
    ("ewk_inf_repair", "Ewok Repair"),
    ("wok_inf_warrior", "Wookiee Warrior"),
    ("wok_inf_mechanic", "Wookiee Mechanic"),
    ("wok_inf_rocketeer", "Wookiee Rocketeer"),
    ("gun_inf_soldier", "Gungan Soldier"),
    ("gun_inf_defender", "Gungan Defender"),
    ("gun_inf_rider", "Gungan Rider"),
    ("gar_inf_soldier", "Naboo Guard"),
    ("gar_inf_pilot", "Naboo Guard Pilot"),
];

/// [`HUNT_CLASS_LABELS`]' name for `class`.
pub fn hunt_class_label(class: &str) -> Option<&'static str> {
    HUNT_CLASS_LABELS
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(class))
        .map(|(_, l)| *l)
}

/// The row of BF2's `hudtransforms` table every BF1 grenade reads, so that a
/// thermal, a crack grenade and a haywire detonator all show BF2's grenade
/// model (the maintainer, 2026-11-01). `[ours]`: BF2's table has rows only
/// for the four thermal detonators.
pub const BF2_GRENADE_HUD_KEY: &str = "rep_weap_inf_thermaldetonator";

/// A BF1 hand grenade: a `grenade` weapon on the grenade's own animation set
/// whose ordnance detonates (`ExplosionExpire`). The mortar launchers share
/// the base class and hold the rifle's set. The Ewok's spear and rock share
/// the grenade's set but carry no explosion: they keep their own key, which
/// BF2's table reads by class name (`ewk_weap_inf_spear` -> `ewk_spear`,
/// `ewk_weap_inf_rock` -> `ewk_rock`; issue #161).
fn is_bf1_grenade(db: &ClassDb, weapon: &str) -> bool {
    !db.is_bf2()
        && db.engine_base(weapon) == Some("grenade")
        && db
            .prop(weapon, "AnimationBank")
            .and_then(|b| b.split_whitespace().next())
            .is_some_and(|b| b.eq_ignore_ascii_case("grenade"))
        && db
            .prop(weapon, "OrdnanceName")
            .map(str::trim)
            .and_then(|ord| db.prop(ord, "ExplosionExpire"))
            .is_some_and(|e| !e.trim().is_empty())
}

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

/// BF2's jet rules on every map (the maintainer, 2026-10-28; card JETP, a
/// named deviation `[ours]`): a BF1 class keeps its own disc numbers, and the
/// keys BF1 never authors - `JetAcceleration` and the `ControlSpeed jet` row
/// (re-jetpack.md §2.1) - come from BF2's class of the same role
/// (re-jetpack.md §2.2, §12). Rows: (BF1 class, its BF2 twin,
/// JetAcceleration, the jet row or None for the ctor default 0.3 / 0.3 / 1.0).
/// `tests/suite/roster.rs` holds the numbers to the BF2 twins' baked classes.
/// Every jet trooper flies on one fuel profile ([`JET_TROOPER_FUEL_COST`]),
/// Jango and Boba on theirs ([`JET_HERO_FUEL_COST`]); another BF1 hover class keeps BF1's net drain in flight
/// ([`bf1_net_jet_fuel_cost`]).
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
    (
        "geo_inf_geonosian",
        "geo_inf_geonosian",
        20.0,
        Some([2.0, 1.25, 1.25]),
    ),
];

/// `[ours]` one leap refill (the maintainer 2026-11-10: "make it the faster
/// refill stat, again we're trying to normalize things across both games"):
/// BF1's dark troopers refill their leap at their BF2 twin's
/// `JetFuelRechargeRate`, 0.075 / s in place of BF1's 0.025. To the lift-off
/// border 0.49 that is 6.5 s, not 19.6 s (the snow variant's 0.4: 5.3 s, not
/// 16 s). The rest of the leap stays each disc's own (re-jetpack.md §6, §13).
/// Rows: (BF1 class, its BF2 twin, the refill a second).
/// `tests/suite/roster.rs` holds the value to the twin's baked class.
/// measure: a playtest of the leap's wait on a BF1 map.
pub const BF1_LEAP_REFILL: &[(&str, &str, f32)] = &[
    ("imp_inf_dark_trooper", "imp_inf_dark_trooper", 0.075),
    ("imp_inf_dark_troopersnow", "imp_inf_dark_trooper", 0.075),
];

/// The jet trooper classes' roots: BF1's own class and the BF2 base that
/// the ep2, ep3, training, rifleman and sniper troopers inherit.
pub const JET_TROOPER_ROOTS: &[&str] = &["rep_inf_jet_trooper", "rep_inf_default_jettrooper"];

/// `[ours]` one jet trooper (the maintainer 2026-11-10, "one game"): every
/// jet trooper on every map pays BF1's net in-flight drain, 0.2 - 0.1 per
/// second ([`bf1_net_jet_fuel_cost`]). With the disc's lift-off cost 0.25
/// (the same in both games) that is (1 - 0.25) / 0.1 = 7.5 s from full and
/// (0.5 - 0.25) / 0.1 = 2.5 s from the spawn's half tank. BF2's disc drain
/// is 0.16 (4.7 s / 1.6 s; re-jetpack.md §13).
/// measure: a playtest hover from a full tank on a BF2 map against 7.5 s.
pub const JET_TROOPER_FUEL_COST: f32 = 0.1;

/// `[ours]` every jet pack's refill per second outside the jet (the
/// maintainer 2026-11-10: BF1's flight time, BF2's refill), BF2's disc rate
/// (BF1's is 0.1): empty to the lift-off border 0.24 in 2.0 s, to full in
/// 8.3 s. The jet troopers and Jango / Boba Fett share it.
/// measure: a playtest of the refill against BF1's 10 s empty to full.
pub const JET_PACK_FUEL_RECHARGE: f32 = 0.12;

/// The jet trooper's refill ([`JET_PACK_FUEL_RECHARGE`]).
pub const JET_TROOPER_FUEL_RECHARGE: f32 = JET_PACK_FUEL_RECHARGE;

/// Jango and Boba Fett's classes (BF2 only; BF1 has neither).
pub const JET_HERO_ROOTS: &[&str] = &["cis_hero_jangofett", "imp_hero_bobafett"];

/// `[ours]` Jango and Boba Fett fly a little longer than a jet trooper (the
/// maintainer 2026-11-10: "say 8.5s vs the standard 7.5s"): on the disc's
/// lift-off cost 0.25, (1 - 0.25) / 8.5 per second, 8.5 s from full and
/// 2.8 s from the spawn's half tank. BF2's disc drain is 0.15 (5.0 s).
/// measure: a playtest hover from a full tank against 8.5 s.
pub const JET_HERO_FUEL_COST: f32 = (1.0 - 0.25) / 8.5;

/// Jango or Boba Fett: a hover jet that can start, on a chain through
/// [`JET_HERO_ROOTS`].
pub fn is_jet_hero(db: &ClassDb, class: &str, c: &SoldierConstants) -> bool {
    c.jet_hover
        && c.jet_jump > 0.0
        && c.jet_fuel_min_border <= 1.0
        && db.chain(class).iter().any(|d| {
            JET_HERO_ROOTS
                .iter()
                .any(|r| r.eq_ignore_ascii_case(&d.name))
        })
}

/// A jet trooper class: a hover jet that can start (JetJump over 0, a
/// border a full tank reaches) on a chain through [`JET_TROOPER_ROOTS`].
/// The BF2 sniper jet trooper (JetJump 0, border 1.04) never jets and is
/// not one.
pub fn is_jet_trooper(db: &ClassDb, class: &str, c: &SoldierConstants) -> bool {
    c.jet_hover
        && c.jet_jump > 0.0
        && c.jet_fuel_min_border <= 1.0
        && db.chain(class).iter().any(|d| {
            JET_TROOPER_ROOTS
                .iter()
                .any(|r| r.eq_ignore_ascii_case(&d.name))
        })
}

/// A soldier class's movement constants through its chain: the props, the
/// collision-damage immunity, the jet's class data ([`jet_class_data`]) and whether a channel-0 weapon is a
/// lightsaber (a `melee` weapon; re-jetpack.md §3.3).
pub fn soldier_constants(db: &ClassDb, class: &str) -> SoldierConstants {
    let mut c = SoldierConstants::from_props(&ClassSrc { db, class });
    jet_class_data(db, class, &mut c);
    for def in db.chain(class).into_iter().rev() {
        for (key, value) in &def.props {
            let slots = if *key == crate::classdb::fnv1a_lowercase("CollisionScale") {
                &mut c.collision_damage_scale
            } else if *key == crate::classdb::fnv1a_lowercase("CollisionThreshold") {
                &mut c.collision_threshold
            } else {
                continue;
            };
            for (slot, value) in slots.iter_mut().zip(value.split_whitespace()) {
                match value.parse::<f32>() {
                    Ok(value) if value.is_finite() => *slot = value,
                    _ => break,
                }
            }
        }
    }
    c.collision_damage_immune = c.collision_damage_scale == [0.0; 3];
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
/// rows over its parent's, one stance at a time), the jet trooper's fuel
/// profile on both discs ([`is_jet_trooper`]), Jango's and Boba's
/// ([`is_jet_hero`]), and on a BF1 disc another
/// hover class's net in-flight drain ([`bf1_net_jet_fuel_cost`]), the
/// [`BF1_LEAP_REFILL`] and the [`BF1_JET_BORROW`] numbers.
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
    if is_jet_trooper(db, class, c) {
        c.jet_fuel_cost = JET_TROOPER_FUEL_COST;
        c.jet_fuel_recharge = JET_TROOPER_FUEL_RECHARGE;
    } else if is_jet_hero(db, class, c) {
        c.jet_fuel_cost = JET_HERO_FUEL_COST;
        c.jet_fuel_recharge = JET_PACK_FUEL_RECHARGE;
    } else if !db.is_bf2() {
        c.jet_fuel_cost = bf1_net_jet_fuel_cost(c);
    }
    if db.is_bf2() {
        return;
    }
    if let Some(&(_, _, refill)) = BF1_LEAP_REFILL
        .iter()
        .find(|(bf1, ..)| bf1.eq_ignore_ascii_case(class))
    {
        c.jet_fuel_recharge = refill;
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

/// `[ours]` (playtest 2026-11-05 B4): BF1 refills the tank in every stance,
/// the hover included (B1:117346-117350), and a BF1 hover class's
/// JetFuelCost was set for that. Under BF2's no-refill rule a BF1 hover
/// class that is not a jet trooper (the Geonosian) pays only the net cost
/// (JetFuelCost - JetFuelRechargeRate), so it flies as long as on BF1
/// (re-jetpack.md §6, §16). The jet trooper's 0.2 - 0.1 is
/// [`JET_TROOPER_FUEL_COST`]. A leap class (cost 0) keeps its numbers.
/// measure: a BF1 PS2 jet trooper's hover time from a full tank, against
/// the 7.5 s this gives.
pub fn bf1_net_jet_fuel_cost(c: &SoldierConstants) -> f32 {
    if c.jet_hover && c.jet_fuel_recharge > 0.0 && c.jet_fuel_cost > c.jet_fuel_recharge {
        c.jet_fuel_cost - c.jet_fuel_recharge
    } else {
        c.jet_fuel_cost
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
        let mut slots = weapon_db.weapon_slots(weapon_owner);
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
            // An award weapon BF1 has no class for sounds as the weapon it
            // replaces does.
            let replaced: Vec<(i8, String)> = std::iter::once(&weapon)
                .chain(held.iter().map(|s| &s.weapon))
                .filter_map(|w| Some((w.medal_lock?, w.name.clone())))
                .collect();
            for selected in
                std::iter::once(&mut weapon).chain(held.iter_mut().map(|s| &mut s.weapon))
            {
                let own = match selected.medal_unlock {
                    Some(t) if db.resolve(&selected.name).is_none() => replaced
                        .iter()
                        .find(|(lock, _)| *lock == t)
                        .map_or(selected.name.as_str(), |(_, name)| name.as_str()),
                    _ => selected.name.as_str(),
                };
                let name = twins
                    .iter()
                    .find(|(bf2, _)| *bf2 == own)
                    .map_or(own, |(_, bf1)| *bf1);
                let native = weapon_from_db(db, name)?;
                awards::sound_as(selected, &native);
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
            // Drawn from BF2's bake on a BF2 map, and a hero on any map
            // (`walk::visual` `asset_game`).
            let game = if db.is_bf2() || class.contains("_hero_") {
                "bf2"
            } else {
                "bf1"
            };
            consts.stance_body = stance_body(game, g);
        }
        let first_person_weapon =
            first_person_weapon(weapon_db, &weapon_name, weapon.offhand_weapon);
        if let (false, false, Some(bf2)) = (donated, db.is_bf2(), db.donor_weapons.as_deref()) {
            awards::add(
                bf2,
                awards::Kit {
                    weapon: &weapon,
                    weapon_slot,
                    weapon_hud: weapon_hud(db, &weapon_name),
                    weapon_geometry: weapon_geometry(db, class),
                    first_person_weapon: first_person_weapon.clone(),
                    held: &mut held,
                    slots: &mut slots,
                    anim: &mut anim,
                },
            );
        }
        consts.pilot_skill = unit_type.eq_ignore_ascii_case("pilot");
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
            min_foot_height: db.prop_f32(class, "MinFootHeight"),
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
                .map(|p| p.fmax(0.0) as u32)
                .or_else(|| {
                    (!db.is_bf2())
                        .then(|| crate::unlocks::bf1_cost(class))
                        .flatten()
                })
                .unwrap_or(0),
            flee_like_a_hero: db
                .prop(class, "FleeLikeAHero")
                .and_then(|value| value.trim().parse::<i32>().ok())
                .is_some_and(|value| value & 1 != 0),
            droideka_shield: walker_droid
                .then(|| crate::droideka::shield_weapon(weapon_db, class))
                .flatten()
                .map(|w| weapon_hud(weapon_db, &w)),
            droideka: walker_droid.then(|| {
                std::sync::Arc::new(crate::droideka::droideka_class(db, weapon_db, class))
            }),
            from_bf2_side: false,
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
        Self::build_with_fallback(db, None, game, faction, mission_classes, true)
    }

    /// `kits`: a second db a mission class missing from `db` is built from
    /// (a BF1 Hunt's BF2 side, [`FactionRoster::pair`]).
    fn build_with_fallback(
        db: &ClassDb,
        kits: Option<&ClassDb>,
        game: &str,
        faction: &str,
        mission_classes: &[(String, u32, Option<u32>)],
        allow_defaults: bool,
    ) -> Result<(FactionRoster, Vec<SkippedClass>)> {
        let defaults = if allow_defaults {
            default_classes(game, faction).with_context(|| {
                format!("unknown faction {faction} on a {game} map (all|imp|rep|cis)")
            })?
        } else {
            &[]
        };
        let mut skipped = Vec::new();
        let mut classes = Vec::new();
        if !mission_classes.is_empty() {
            for (name, min, max) in mission_classes {
                let name = name.to_ascii_lowercase();
                let source = match kits {
                    Some(kits) if db.resolve(&name).is_none() && kits.resolve(&name).is_some() => {
                        Some(kits)
                    }
                    _ => None,
                };
                match ClassRoster::build(source.unwrap_or(db), &name, *min, *max) {
                    Ok(mut c) => {
                        c.from_bf2_side = source.is_some();
                        classes.push(c);
                    }
                    Err(e) => skipped.push(SkippedClass {
                        class: name,
                        reason: format!("{e:#}"),
                    }),
                }
            }
            // The fill class has to be the mission's OWN first class.
            let fill = mission_classes[0].0.to_ascii_lowercase();
            if allow_defaults && classes.first().is_none_or(|c| c.class != fill) {
                classes.clear();
            }
        }
        if allow_defaults && classes.is_empty() {
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
                "{faction}: no soldier class could be built: {skipped:?}"
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

    /// The class the AI fill with and the human starts as: Conquest's fill
    /// trooper, or the first usable authored hero in Hero Assault.
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
    /// Hero Assault keeps every usable authored class, even if the first is
    /// missing, and refuses an empty team instead of substituting infantry.
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
        let mut sides = factions.to_vec();
        for load in mission
            .filter(|m| m.is_hero_assault())
            .into_iter()
            .flat_map(|m| &m.class_load_order)
        {
            if !sides.contains(&load.side.as_str()) {
                sides.push(&load.side);
            }
        }
        let hunt = mission.filter(|m| m.is_hunt());
        let db = if let Some(m) = hunt {
            let mut db = ClassDb::default();
            let mut loaded = std::collections::BTreeSet::new();
            for side in &m.class_load_order {
                let key = side.side.to_ascii_lowercase();
                if loaded.insert(key.clone()) {
                    let path = classes_dir.join(format!("{game}_{key}.ron"));
                    if pak::is_file(&path) {
                        db.load(&path)?;
                    }
                }
            }
            for file in class_files(game, &sides) {
                let path = classes_dir.join(file);
                if pak::is_file(&path) {
                    db.load(&path)?;
                }
            }
            db.load_droid_chunks(&classes_dir.join("droid_chunks.ron"))?;
            db
        } else {
            load_classdb(classes_dir, game, &sides)?
        };
        let hunt = hunt.is_some();
        let jawa = if hunt && game == "bf1" && factions.contains(&"jawa") {
            // [ours] Every Jawa carries BF2's arc caster beside its repair
            // tool (`crate::jawa`); in Hunt at full strength.
            Some(crate::jawa::donor(classes_dir)?)
        } else {
            None
        };
        let ewoks = if hunt && game == "bf1" && factions.contains(&"ewk") {
            // [ours] BF1's Ewok spear and rock were AI-only weapons: the
            // spear flies at 25 m/s under 1.5 g with no lift, the rock is a
            // 20 m/s shell with no blast. A human Ewok in BF1 Hunt throws
            // BF2's (50 m/s, 2 g, 0.15 lift; the rock 40 m/s with its 25
            // blast), as BF2's own Hunt Ewoks do; BF1's repair tool stays.
            // Issue #161. measure: BF1 Endor Hunt both sides and tiers.
            let mut ewk = ClassDb::default();
            ewk.load(&classes_dir.join("bf2_ewk.ron"))?;
            let ingame = classes_dir.join("bf2_ingame.ron");
            if pak::is_file(&ingame) {
                ewk.load(&ingame)?;
            }
            Some(ewk)
        } else {
            None
        };
        // [ours] A BF1 Hunt adaptation fields its planet's BF2 Hunt roster
        // (issue #161). A class BF1 has no counterpart for (BF2 Kashyyyk
        // Hunt's MagnaGuard, `cis_inf_officer_hunt`) is built from BF2's
        // side, as BF1 Conquest's heroes are (`heroes::append`).
        let kits = if hunt && game == "bf1" {
            let missing = mission.is_some_and(|m| {
                m.teams
                    .iter()
                    .flat_map(|t| &t.classes)
                    .any(|(c, _, _)| db.resolve(&c.to_ascii_lowercase()).is_none())
            });
            missing
                .then(|| load_classdb(classes_dir, "bf2", &sides))
                .transpose()?
        } else {
            None
        };
        let mut warnings = Vec::new();
        let mut out = Vec::new();
        for (i, f) in factions.into_iter().enumerate() {
            let mix: &[(String, u32, Option<u32>)] = mission
                .and_then(|m| m.teams.get(i))
                .map(|t| t.classes.as_slice())
                .unwrap_or(&[]);
            let assault = mission.is_some_and(MissionDef::is_hero_assault);
            let (mut roster, skipped) =
                Self::build_with_fallback(&db, kits.as_ref(), game, f, mix, !assault && !hunt)
                    .with_context(|| {
                        format!(
                            "{}team {} ({f})",
                            if assault { "Hero Assault " } else { "" },
                            i + 1
                        )
                    })?;
            if let Some(donor) = jawa.as_ref().filter(|_| f == "jawa") {
                for class in &mut roster.classes {
                    crate::jawa::arm(donor, class).context("BF1 Hunt Jawa")?;
                }
            }
            if hunt && !skipped.is_empty() {
                anyhow::bail!("Hunt team {} ({f}) has missing classes: {skipped:?}", i + 1);
            }
            if hunt {
                for class in &mut roster.classes {
                    // Each class's weapons read the class data it was built
                    // from: BF2's for a class a BF1 Hunt fields from BF2's
                    // side.
                    let source = match kits.as_ref() {
                        Some(kits) if class.from_bf2_side => kits,
                        _ => &db,
                    };
                    for weapon in std::iter::once(&mut class.weapon)
                        .chain(class.held.iter_mut().map(|slot| &mut slot.weapon))
                    {
                        if weapon.thrown.is_none()
                            && source.engine_base(&weapon.name) == Some("grenade")
                        {
                            if let Some(thrown) =
                                weapon_db::thrown_weapon(source, &weapon.name, true)?
                            {
                                *weapon = thrown;
                            }
                        }
                    }
                    class.unlock_points = 0;
                    if let Some(label) = hunt_class_label(&class.class) {
                        class.label = label.to_owned();
                    }
                }
            }
            if let Some(donor) = ewoks.as_ref().filter(|_| f == "ewk") {
                for class in &mut roster.classes {
                    donate_ewok_weapons(donor, class)?;
                }
            }
            for s in &skipped {
                warnings.push(format!(
                    "roster {f}: skipped class {} — {}",
                    s.class, s.reason
                ));
            }
            if !assault
                && !mix.is_empty()
                && roster.fill_class().class != mix[0].0.to_ascii_lowercase()
            {
                warnings.push(format!(
                    "roster {f}: the mission's fill class {} is not in the {game} class db \
                     ({} of {} classes missing) — falling back to the default list",
                    mix[0].0,
                    skipped.len(),
                    mix.len()
                ));
            }
            if mission.is_none_or(|m| m.mode == "con") {
                // OURS: in ground Conquest, the E-11 takes five close-range
                // body hits against a trooper, as in BF1. Only Person damage
                // changes. measure: paired faction wins on baked rounds.
                let damage = crate::normalize::REFERENCE_HEALTH / 5.0;
                for class in &mut roster.classes {
                    let weapons = std::iter::once(&mut class.weapon)
                        .chain(class.held.iter_mut().map(|held| &mut held.weapon));
                    for w in weapons
                        .filter(|w| w.name == "imp_weap_inf_rifle" && w.ordnance.max_damage > 0.0)
                    {
                        w.ordnance.scales[core::weapon::HealthType::Person as usize] =
                            damage / w.ordnance.max_damage;
                    }
                }
            }
            out.push(roster);
        }
        let b = out.pop().unwrap();
        let a = out.pop().unwrap();
        Ok(([a, b], warnings))
    }
}

mod weapon_db;
pub use weapon_db::*;
mod looks;
pub use looks::*;
mod items;
pub use items::*;

#[cfg(test)]
mod tests;
