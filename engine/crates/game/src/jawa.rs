//! `[ours]` (maintainer, 2026-11-20): one Jawa for both games and every mode.
//!
//! - Every Jawa carries the arc caster and the fusion cutter. BF2's Jawa
//!   carries both as authored; BF1's carries only the fusion cutter, a repair
//!   tool, so it is given BF2's arc caster ([`arm`]), read from the BF2 class
//!   data ([`donor`]) and the same on every map.
//! - In Hunt the Jawas are one of the two sides and the arc caster is at full
//!   strength. In every other mode they are a neutral native team, hostile to
//!   both armies ([`core::sim::Sim::set_native_neutral`]), they take no
//!   command post, and their arc caster does [`NATIVE_ARC_CASTER_SCALE`] of
//!   its damage ([`tame`]).
//! - A Jawa's death costs no army a ticket or a point and is nobody's team
//!   kill; a soldier a Jawa kills costs his side one ticket, like any death.
//!
//! A build always has both games' data (pack.py refuses one game alone).
//! A bake without BF2's desert side leaves BF1's Jawa with its own kit
//! outside Hunt, with a warning: harmless, as on the BF1 disc.

use crate::classdb::ClassDb;
use crate::roster::{first_person_weapon, weapon_from_db, weapon_hud, ClassRoster, HeldSlot};
use anyhow::{Context, Result};
use std::path::Path;

/// The Jawa soldier class, the same name in both games.
pub const CLASS: &str = "tat_inf_jawa";

/// BF2's Jawa arc caster, the one every Jawa fires.
pub const ARC_CASTER: &str = "tat_weap_inf_jawa_arccaster";

/// `[ours]` The share of its damage the arc caster does outside Hunt, where
/// the Jawas are natives fighting both armies: a real but modest threat, no
/// more dangerous than the Dune Sea's Tuskens.
/// measure: on BF1 and BF2 Mos Eisley Conquest, both eras, the Jawas' share
/// of army deaths and kills a minute against the Dune Sea Tuskens' (natives
/// on, bots only, Hard).
pub const NATIVE_ARC_CASTER_SCALE: f32 = 0.6;

/// Is `class` a Jawa?
pub fn is_jawa(class: &str) -> bool {
    class.eq_ignore_ascii_case(CLASS)
}

/// Does this class still lack the arc caster?
pub fn needs_arc_caster(class: &ClassRoster) -> bool {
    is_jawa(&class.class) && !carries(class, ARC_CASTER)
}

fn carries(class: &ClassRoster, weapon: &str) -> bool {
    std::iter::once(&class.weapon)
        .chain(class.held.iter().map(|h| &h.weapon))
        .any(|w| w.name.eq_ignore_ascii_case(weapon))
}

/// The BF2 class data the arc caster is read from: BF2's desert side and
/// the shared weapons it derives from.
pub fn donor(classes_dir: &Path) -> Result<ClassDb> {
    let mut db = ClassDb::default();
    db.load(&classes_dir.join("bf2_des.ron"))?;
    db.load(&classes_dir.join("bf2_ingame.ron"))?;
    Ok(db)
}

/// Give a Jawa that lacks it the arc caster as its primary weapon; its own
/// repair tool moves to the first free slot. A class that already carries
/// it (BF2's Jawa) is left as it is.
pub fn arm(donor: &ClassDb, class: &mut ClassRoster) -> Result<()> {
    if !needs_arc_caster(class) {
        return Ok(());
    }
    let original = class.weapon.clone();
    let original_hud = class.weapon_hud.clone();
    class.weapon = weapon_from_db(donor, ARC_CASTER)?;
    class.weapon_hud = weapon_hud(donor, &class.weapon.name);
    let slot = (0..8)
        .find(|slot| *slot != class.weapon_slot && !class.held.iter().any(|w| w.slot == *slot))
        .context("the Jawa has no free slot for its repair tool")?;
    class.held.push(HeldSlot {
        slot,
        channel: 0,
        clips: 0,
        weapon: original,
        hud: original_hud,
        geometry: class.weapon_geometry.clone(),
        first_person: class.first_person_weapon.clone(),
    });
    class.first_person_weapon =
        first_person_weapon(donor, &class.weapon.name, class.weapon.offhand_weapon);
    class.weapon_geometry = donor
        .prop(&class.weapon.name, "GeometryName")
        .map(str::to_owned);
    Ok(())
}

/// Scale every damage number of an arc caster by `scale`: the shot's root
/// and charge damage, its explosions, and the chain's per-link deduction
/// and stop threshold, so a shot links as many bodies as before.
pub fn scale_arc_caster(weapon: &mut core::weapon::WeaponClass, scale: f32) {
    let ord = &mut weapon.ordnance;
    ord.max_damage *= scale;
    if let Some(min) = &mut ord.min_damage {
        *min *= scale;
    }
    for knot in &mut ord.knots {
        knot.damage *= scale;
    }
    for e in [&mut ord.explosion, &mut ord.expire].into_iter().flatten() {
        e.damage *= scale;
    }
    if let Some(em) = &mut weapon.emitter {
        em.jump_deduction *= scale;
        em.damage_threshold *= scale;
    }
}

/// The arc caster of a Jawa outside Hunt ([`NATIVE_ARC_CASTER_SCALE`]).
pub fn tame(class: &mut ClassRoster) {
    if !is_jawa(&class.class) {
        return;
    }
    for weapon in
        std::iter::once(&mut class.weapon).chain(class.held.iter_mut().map(|h| &mut h.weapon))
    {
        if weapon.name.eq_ignore_ascii_case(ARC_CASTER) {
            scale_arc_caster(weapon, NATIVE_ARC_CASTER_SCALE);
        }
    }
}
