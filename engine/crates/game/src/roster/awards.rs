//! BF2's award weapons on BF1 classes (`analysis/award-weapons.md`).
//!
//! [ours] BF1 has no award weapons. A BF1 class of the four trooper sides
//! gains BF2's award for each weapon it carries that BF2 has one for
//! ([`BF1_AWARDS`]). The award is built from BF2's own class data
//! ([`ClassDb::donor_classdb`]), so it is the same weapon on every map; on
//! the shared health and damage scale (`crate::normalize`) it keeps BF2's
//! advantage over the weapon it stands for. It is held, sounds and hits the
//! ground as that BF1 weapon does; only its purple bolt is the award's own.
//! BF1's Blast Cannon is the one exception: its award is our own Heavy Blast
//! Cannon ([`heavy_blast_cannon`]), the BF1 gun with the flechette's
//! advantage over BF2's shotgun. The switch rule is BF2's on both games
//! (`core::sim::awards`).

use super::*;

/// A BF1 weapon and the BF2 award that stands for it.
pub struct AwardRow {
    /// The BF1 weapon's name after its side.
    pub weapon: &'static str,
    /// BF2's award class after the side.
    pub award: &'static str,
    /// The award's medal (BF2's `MedalsTypeToUnlock`).
    pub medal: i8,
    /// The award takes the weapon's place in the switch cycle (BF2's
    /// `MedalsTypeToLock` on the weapon). The guided launcher is an extra
    /// slot beside the launcher, as in BF2.
    pub replaces: bool,
    /// The award is BF2's class of that name. `false`: our own award, built
    /// from the BF1 weapon ([`heavy_blast_cannon`], the only one).
    pub bf2: bool,
}

pub const BF1_AWARDS: &[AwardRow] = &[
    AwardRow {
        weapon: "_weap_inf_rifle",
        award: "_weap_award_rifle",
        medal: 1,
        replaces: true,
        bf2: true,
    },
    AwardRow {
        weapon: "_weap_inf_pistol",
        award: "_weap_award_pistol",
        medal: 0,
        replaces: true,
        bf2: true,
    },
    AwardRow {
        weapon: "_weap_inf_commando_pistol",
        award: "_weap_award_pistol",
        medal: 0,
        replaces: true,
        bf2: true,
    },
    AwardRow {
        weapon: "_weap_inf_sniperrifle",
        award: "_weap_award_sniper_rifle",
        medal: 4,
        replaces: true,
        bf2: true,
    },
    AwardRow {
        weapon: "_weap_inf_shotgun",
        award: "_weap_award_blast_cannon",
        medal: 5,
        replaces: true,
        bf2: false,
    },
    AwardRow {
        weapon: "_weap_inf_torpedo_launcher",
        award: "_weap_award_rocket_launcher",
        medal: 2,
        replaces: false,
        bf2: true,
    },
];

/// `[ours]` The Heavy Blast Cannon (the maintainer 2026-11-10: "blast cannon
/// award: sure, I'm on board"; "I don't want to lose the blast cannon effect
/// or its equivalent power"): the award for BF1's Blast Cannon in place of
/// BF2's flechette shotgun. It is the BF1 gun, its model, bolts, sound and
/// impacts, with the flechette's advantage over BF2's shotgun on each number
/// BF2 changes (`analysis/award-weapons.md` section 3):
///
/// - a bolt to a soldier x[`HBC_DAMAGE`] (85 over 65): 113.33 on the shared
///   scale, and the same to a shield; to vehicles and buildings as the base;
/// - the time between shots x[`HBC_SHOT_DELAY`] (1.2 s over 1.0 s): 0.9 s;
/// - the yaw spread x[`HBC_YAW_SPREAD`] (4 over 4.5): 3.56 degrees; the
///   pitch spread as the base's 2 (BF2's is 3 on both);
/// - [`HBC_ROUNDS`] rounds a clip, one a bolt: seven shots, where BF2's
///   award fires six to its shotgun's five.
///
/// It keeps the base's five bolts a shot (BF2 trades a pellet for the
/// damage; five fat bolts are the Blast Cannon's look) and its clips.
/// measure: a playtest of the award against the Blast Cannon on a BF1 map.
pub fn heavy_blast_cannon(base: &WeaponClass, name: &str, medal: i8) -> WeaponClass {
    let mut w = base.clone();
    w.name = name.to_owned();
    w.medal_unlock = Some(medal);
    w.medal_lock = None;
    for s in &mut w.ordnance.scales[..3] {
        *s *= HBC_DAMAGE;
    }
    w.ordnance.shield_scale *= HBC_DAMAGE;
    for d in &mut w.shot_delay {
        *d *= HBC_SHOT_DELAY;
    }
    w.yaw_spread *= HBC_YAW_SPREAD;
    w.rounds_per_clip = HBC_ROUNDS;
    w
}

/// The Heavy Blast Cannon's damage to a soldier over the Blast Cannon's:
/// BF2's flechette pellet over its shotgun pellet (85 / 65).
pub const HBC_DAMAGE: f32 = 85.0 / 65.0;

/// Its time between shots over the Blast Cannon's: BF2's 1.2 s over 1.0 s.
pub const HBC_SHOT_DELAY: f32 = 1.2;

/// Its yaw spread over the Blast Cannon's: BF2's 4 over 4.5.
pub const HBC_YAW_SPREAD: f32 = 4.0 / 4.5;

/// Its rounds a clip (the Blast Cannon 30): one more shot than the base's
/// six, about BF2's x1.2 (five shots to six).
pub const HBC_ROUNDS: u32 = 35;

/// Its name on the HUD and the unit screen, `[ours]` (BF2's award
/// weapons read "AWARD <weapon>"; no disc has a text for it).
pub const HBC_NAME: &str = "AWARD Blast Cannon";

/// Its line when the medal is earned, `[ours]` in BF2's words for an award
/// (`game.awards.bonus.*`, "Award: Flechette Shotgun" for this medal on a
/// BF2 map; no disc has a text for it).
pub const HBC_BONUS: &str = "Award: Heavy Blast Cannon";

/// Our own award class `weapon`'s name and earned line ([`HBC_NAME`],
/// [`HBC_BONUS`]), for a class no disc has a text for. `None` for any other
/// class.
pub fn own_texts(weapon: &str) -> Option<(&'static str, &'static str)> {
    let weapon = weapon.to_ascii_lowercase();
    let (side, tail) = weapon.split_at(weapon.find('_')?);
    (FACTION_ORDER.contains(&side) && BF1_AWARDS.iter().any(|r| !r.bf2 && r.award == tail))
        .then_some((HBC_NAME, HBC_BONUS))
}

/// The BF2 classes whose kit gives an award its clips on a BF1 class, after
/// the side (and the Republic's `ep3_`): the first that carries the award.
const KITS: [&str; 6] = [
    "rifleman",
    "marine",
    "rocketeer",
    "sniper",
    "engineer",
    "pilot",
];

/// A BF1 weapon's award row and BF2's award class for it.
pub fn bf1_award(weapon: &str) -> Option<(&'static AwardRow, String)> {
    let weapon = weapon.to_ascii_lowercase();
    let (side, tail) = weapon.split_at(weapon.find('_')?);
    if !FACTION_ORDER.contains(&side) {
        return None;
    }
    let row = BF1_AWARDS.iter().find(|r| r.weapon == tail)?;
    Some((row, format!("{side}{}", row.award)))
}

/// The medal whose award takes a BF1 weapon's place (`WeaponClass::medal_lock`).
pub(super) fn bf1_medal_lock(weapon: &str) -> Option<i8> {
    bf1_award(weapon)
        .filter(|(row, _)| row.replaces)
        .map(|(row, _)| row.medal)
}

/// The clips BF2 gives `award` (a `side` class's): its count on the first
/// [`KITS`] class that carries it.
fn bf2_clips(bf2: &ClassDb, side: &str, award: &str) -> Option<u32> {
    let era = if side == "rep" { "ep3_" } else { "" };
    KITS.iter().find_map(|kit| {
        bf2.weapon_slots(&format!("{side}_inf_{era}{kit}"))
            .into_iter()
            .find(|s| s.weapon.eq_ignore_ascii_case(award))
            .map(|s| s.ammo.unwrap_or(0))
    })
}

/// `selected` sounds as `native` does: a BF2 weapon on a BF1 map, where
/// BF2's sounds are not loaded.
pub(super) fn sound_as(selected: &mut WeaponClass, native: &WeaponClass) {
    selected.fire_sound.clone_from(&native.fire_sound);
    selected.fire_loop_sound.clone_from(&native.fire_loop_sound);
    selected.reload_sound.clone_from(&native.reload_sound);
    selected.empty_sound.clone_from(&native.empty_sound);
    selected.change_sound.clone_from(&native.change_sound);
    selected
        .ordnance
        .ordnance_sound
        .clone_from(&native.ordnance.ordnance_sound);
    selected
        .ordnance
        .collision_sound
        .clone_from(&native.ordnance.collision_sound);
    if let (Some(selected), Some(native)) = (&mut selected.ordnance.impact, &native.ordnance.impact)
    {
        if let (Some(selected), Some(native)) = (&mut selected.explosion, &native.explosion) {
            selected.sound.clone_from(&native.sound);
        }
    }
    if let (Some(selected), Some(native)) =
        (&mut selected.ordnance.explosion, &native.ordnance.explosion)
    {
        selected.look.sound.clone_from(&native.look.sound);
    }
}

/// `award` (BF2's class) presented as the BF1 weapon `base` it stands for:
/// its sounds, muzzle flash and impact effects, which a BF1 map has loaded.
/// A beam keeps its own (none); the guided rocket flies and bursts as the
/// BF1 rocket does.
fn dress(award: &mut WeaponClass, base: &WeaponClass) {
    sound_as(award, base);
    award.muzzle_effect.clone_from(&base.muzzle_effect);
    if award.flash.is_some() {
        award.flash.clone_from(&base.flash);
    }
    if award.ordnance.beam.is_none() && award.ordnance.impact.is_some() {
        award.ordnance.impact.clone_from(&base.ordnance.impact);
    }
    let Some(guided) = award.guided.as_deref_mut() else {
        return;
    };
    let rocket = base.ordnance.impact.as_ref();
    let burst = rocket.and_then(|i| i.explosion.as_ref());
    for e in [&mut guided.class.explosion, &mut guided.class.noncritical]
        .into_iter()
        .flatten()
        .chain(&mut award.ordnance.explosion)
    {
        if let Some(burst) = burst {
            e.look.effects.clone_from(&burst.effects);
            e.look.water_effect.clone_from(&burst.water_effect);
            e.look.sound.clone_from(&burst.sound);
        }
    }
    guided.class.trail = rocket.and_then(|i| i.trail.clone());
    guided.class.death_sound = None;
    if base.ordnance.geometry.is_some() {
        award.ordnance.geometry.clone_from(&base.ordnance.geometry);
    }
}

/// What a BF1 class holds, as [`add`] reads and extends it.
pub(super) struct Kit<'a> {
    pub weapon: &'a WeaponClass,
    pub weapon_slot: u8,
    pub weapon_hud: crate::walk::WeaponHud,
    pub weapon_geometry: Option<String>,
    pub first_person_weapon: crate::first_person::FpWeapon,
    pub held: &'a mut Vec<HeldSlot>,
    pub slots: &'a mut Vec<WeaponSlot>,
    pub anim: &'a mut crate::animator::ClassAnim,
}

/// Append BF2's award slots to a BF1 class's kit: one per channel-0 weapon
/// with a [`BF1_AWARDS`] row, in slot order, after the class's last slot.
pub(super) fn add(bf2: &ClassDb, kit: Kit<'_>) {
    struct Base {
        weapon: WeaponClass,
        clips: u32,
        hud: crate::walk::WeaponHud,
        geometry: Option<String>,
        first_person: crate::first_person::FpWeapon,
    }
    let clips_of = |slot: u8| {
        kit.slots
            .iter()
            .find(|s| s.slot == slot)
            .and_then(|s| s.ammo)
            .unwrap_or(0)
    };
    let mut bases = vec![(
        kit.weapon_slot,
        Base {
            weapon: kit.weapon.clone(),
            clips: clips_of(kit.weapon_slot),
            hud: kit.weapon_hud,
            geometry: kit.weapon_geometry,
            first_person: kit.first_person_weapon,
        },
    )];
    bases.extend(kit.held.iter().filter(|h| h.channel == 0).map(|h| {
        (
            h.slot,
            Base {
                weapon: h.weapon.clone(),
                clips: h.clips,
                hud: h.hud.clone(),
                geometry: h.geometry.clone(),
                first_person: h.first_person.clone(),
            },
        )
    }));
    bases.sort_by_key(|(slot, _)| *slot);
    let mut next = kit.slots.iter().map(|s| s.slot + 1).max().unwrap_or(0);
    for (_, base) in bases {
        let Some((row, name)) = bf1_award(&base.weapon.name) else {
            continue;
        };
        if usize::from(next) >= crate::classdb::MAX_WEAPON_SLOTS
            || kit.held.iter().any(|h| h.weapon.name == name)
        {
            continue;
        }
        let (name, weapon, clips) = if row.bf2 {
            let Some(name) = bf2.resolve(&name).map(str::to_owned) else {
                continue;
            };
            let Ok(mut weapon) = weapon_from_db(bf2, &name) else {
                continue;
            };
            dress(&mut weapon, &base.weapon);
            let side = name.split('_').next().unwrap_or_default();
            let clips = bf2_clips(bf2, side, &name).unwrap_or(base.clips);
            (name, weapon, clips)
        } else {
            let weapon = heavy_blast_cannon(&base.weapon, &name, row.medal);
            (name, weapon, base.clips)
        };
        let bank = kit.anim.bank_of(&base.weapon.name).to_owned();
        kit.anim.banks.push((name.to_ascii_lowercase(), bank));
        kit.slots.push(WeaponSlot {
            slot: next,
            weapon: name.clone(),
            ammo: Some(clips),
            channel: 0,
        });
        kit.held.push(HeldSlot {
            slot: next,
            channel: 0,
            clips,
            // The guided launcher sits beside the launcher: BF2's own row
            // tells the two apart.
            hud: if row.replaces {
                base.hud
            } else {
                weapon_hud(bf2, &name)
            },
            geometry: base.geometry,
            first_person: base.first_person,
            weapon,
        });
        next += 1;
    }
}
