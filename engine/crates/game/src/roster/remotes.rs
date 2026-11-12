//! Resolves the remote weapon's droid, blaster and destruct (salvo shell or
//! burst) through the baked class chains. Follows `analysis/re-remote-droid.md`.

use super::{explosion_class, fired_ordnance, weapon_from_db, weapon_hud, ClassDb, ClassSrc};
use crate::walk::{DroidHud, WeaponHud};
use anyhow::{Context, Result};
use core::remote::{Destruct, DroidClass, RemoteWeapon, Salvo};
use core::weapon::{OrdnanceClass, PropSource, WeaponClass};

const SALVO_ORDNANCE: u32 = 0x6fd9_2be8;
const BURST_SOUND: u32 = 0x76f3_beea;

pub(super) fn remote_weapon(
    db: &ClassDb,
    weapon: &str,
    wsrc: &dyn PropSource,
) -> Result<WeaponClass> {
    let ord =
        WeaponClass::ordnance_name(wsrc).with_context(|| format!("{weapon}: no OrdnanceName"))?;
    let droid_name = db
        .resolve(ord.trim())
        .with_context(|| format!("{weapon}: droid class {ord} not in class db"))?
        .to_owned();
    if db.engine_base(&droid_name) == Some("flyer") {
        return guided_weapon(db, weapon, wsrc, &droid_name);
    }
    let mut droid = DroidClass::from_props(
        &ClassSrc {
            db,
            class: &droid_name,
        },
        db.is_bf2(),
    );
    droid.explosion = db
        .prop(&droid_name, "ExplosionName")
        .and_then(|n| explosion_class(db, n.trim()));
    let slots = db.weapon_slots(&droid_name);
    let blaster_name = slots
        .iter()
        .find(|s| s.channel == 0)
        .and_then(|s| db.resolve(&s.weapon))
        .with_context(|| format!("{droid_name}: no blaster"))?;
    let blaster = weapon_from_db(db, blaster_name)?;
    let mut w = WeaponClass::from_props(weapon, wsrc, OrdnanceClass::default());
    core::weapon::Dispenser::new(&mut w, wsrc, db.is_bf2(), ());
    w.ordnance.geometry = droid.geometry.clone();
    let mut remote = RemoteWeapon::new(wsrc, droid, blaster);
    remote.destruct = slots
        .iter()
        .find(|s| s.channel == 1)
        .and_then(|s| db.resolve(&s.weapon))
        .map(|name| destruct(db, name));
    w.remote = Some(Box::new(remote));
    Ok(w)
}

/// BF2's `HUDTag` on its remote droid's blaster (`analysis/re-remote-droid.md`
/// §8): the row's mesh on every map.
pub const BLASTER_HUD: &str = "hud_vehicle_recoiless";
/// BF2's `HUDTag` on its droid's destruct: the probe droid's card.
pub const DESTRUCT_HUD: &str = "hud_detpack_plunger";

/// A droid row with BF2's mesh. `[ours]` (maintainer, 2026-10-28: "the bf1
/// remote droid hud icons look poor"): a BF1 droid's rows show BF2's
/// meshes too, not its `IconTexture` sheets (`IMP_pistol_icon`,
/// `HUD_orbital_strike_icon`); the HUD is BF2's on every map and loads them
/// from `common_bf2`.
fn bf2_row(mut hud: WeaponHud, key: &str) -> WeaponHud {
    hud.key = key.to_owned();
    hud.icon = None;
    hud
}

/// The rows a flying droid shows (`analysis/re-remote-droid.md` §8): its
/// channel-0 and channel-1 weapons, each with BF2's mesh ([`bf2_row`]).
pub(super) fn droid_hud(db: &ClassDb, weapon: &str) -> Option<DroidHud> {
    let droid = db.resolve(db.prop(weapon, "OrdnanceName")?.trim())?;
    let slots = db.weapon_slots(droid);
    let row = |channel: u8| {
        slots
            .iter()
            .find(|s| s.channel == channel)
            .and_then(|s| Some((s.ammo.unwrap_or(0), db.resolve(&s.weapon)?)))
    };
    let (_, blaster) = row(0)?;
    let destruct = row(1);
    Some(DroidHud {
        blaster: bf2_row(weapon_hud(db, blaster), BLASTER_HUD),
        destruct: destruct.map(|(_, name)| bf2_row(weapon_hud(db, name), DESTRUCT_HUD)),
        destruct_rounds: destruct.and_then(|(clips, name)| {
            let rounds = db.prop_f32(name, "RoundsPerClip").unwrap_or(1.0).max(0.0) as u32;
            (clips > 0).then_some(clips * rounds)
        }),
    })
}

fn destruct(db: &ClassDb, name: &str) -> Destruct {
    let src = ClassSrc { db, class: name };
    let mut d = Destruct::from_props(&src);
    let ord = src
        .prop("OrdnanceName")
        .or_else(|| src.prop_hash(SALVO_ORDNANCE))
        .and_then(|o| db.resolve(o.trim()));
    let Some(ord) = ord else {
        return d;
    };
    if db.engine_base(ord) == Some("emitterordnance") {
        let osrc = ClassSrc { db, class: ord };
        let radius = db.prop_f32(ord, "Radius").unwrap_or(0.0);
        let mut e = core::weapon::ExplosionClass {
            damage: db
                .prop_f32(ord, "Damage")
                .or_else(|| db.prop_f32(ord, "MaxDamage"))
                .unwrap_or(0.0),
            damage_inner: radius,
            damage_outer: radius,
            spares_living: true,
            ..Default::default()
        };
        e.scales = OrdnanceClass::from_props(&osrc).scales;
        if let Some(look) = db
            .prop(ord, "Explosion")
            .and_then(|x| explosion_class(db, x.trim()))
        {
            e.look = look.look;
        }
        d.burst = Some(e);
        d.burst_sound = osrc.prop_hash(BURST_SOUND).map(|s| s.trim().to_owned());
    } else {
        let shell = WeaponClass {
            name: format!("{name}_salvo"),
            ordnance: fired_ordnance(db, ord),
            ..WeaponClass::default()
        };
        d.salvo = Some(Salvo::from_props(&src, shell));
    }
    d
}

fn guided_weapon(db: &ClassDb, name: &str, src: &dyn PropSource, ord: &str) -> Result<WeaponClass> {
    let osrc = ClassSrc { db, class: ord };
    let mut class = core::guided::GuidedClass::from_props(&osrc);
    class.explosion = osrc
        .prop_hash(0xe04f_08c1)
        .and_then(|n| explosion_class(db, n.trim()));
    class.noncritical = osrc
        .prop_hash(0x8a32_3f32)
        .and_then(|n| explosion_class(db, n.trim()));
    class.flight = crate::vehicle_pads::flyer_params(db, ord, true, 0.0)
        .with_context(|| format!("{name}: {ord} is not a flyer class"))?;
    if let Some(seat) = crate::vehicle_class::seats(db, ord).first() {
        class.view = core::guided::GuidedView {
            eye_point_offset: seat.eye_point_offset,
            track_center: seat.track_center,
            track_offset: seat.track_offset,
            slow_offset: seat.slow_offset,
            fast_offset: seat.fast_offset,
            fast_fov: seat.fast_fov,
            third_person_fov: seat.third_person_fov,
            move_tension: seat.move_tension,
        };
    }
    let mut w = WeaponClass::from_props(name, src, OrdnanceClass::default());
    core::weapon::Dispenser::new(&mut w, src, true, ());
    w.thrown = None;
    w.ordnance.geometry = db.prop(ord, "GeometryName").map(str::to_owned);
    let f = &class.flight;
    w.ordnance.velocity = f.min_speed;
    anyhow::ensure!(
        f.min_speed > 0.0 && f.energy_max > 0.0 && f.energy_auto_restore < 0.0,
        "{name}: missing guided flyer movement or lifetime properties"
    );
    w.ordnance.life_span = f.energy_max / -f.energy_auto_restore;
    w.ordnance.explosion = class.explosion.clone();
    w.guided = Some(Box::new(core::guided::GuidedWeapon::new(src, class)));
    Ok(w)
}
