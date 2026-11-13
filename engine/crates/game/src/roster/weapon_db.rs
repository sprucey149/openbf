//! Weapons read from the class db: the HUD key, charge stages, ordnance, thrown and sticky weapons, and their donors.

use super::*;

/// What the HUD's weapon slot is told about `weapon` (`analysis/re-hud.md`
/// §3 `[V][O]`, BF2 `FUN_00326e28`): its `HUDTag` — inherited through the
/// BASE chain — else the class's OWN `GeometryName` (not inherited: a class
/// without one sends its own name), else its name. `icon` is the BF1
/// `IconTexture`, through the chain. A BF1 grenade sends
/// [`BF2_GRENADE_HUD_KEY`]: the HUD is BF2's on every map.
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
    let key = if is_bf1_grenade(db, weapon) {
        BF2_GRENADE_HUD_KEY.to_owned()
    } else {
        key
    };
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
pub(super) struct ListSrc<'a> {
    pub(super) props: &'a [(u32, &'a str)],
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
pub(super) const NEXT_CHARGE: u32 = 0x8316_f444;

pub(super) const TILT_COUNT: u32 = 0xa4df_afc0;

pub(super) const TILT: u32 = 0xa412_0aa8;

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
    // A BF1 weapon leaves the cycle for BF2's award that stands for it.
    if !db.is_bf2() {
        if let Some((row, _)) = awards::bf1_award(&w.name) {
            w.medals_type = Some(row.medal);
            if row.replaces && w.medal_lock.is_none() {
                w.medal_lock = Some(row.medal);
            }
        }
    }
    Ok(w)
}

/// An ordnance's charge points (issue #29, `analysis/re-weapon-charge.md`
/// §4): a `NextCharge` in the ordnance class clones it as it stands and the
/// keys after it go to the clone, so the root is the keys before the first
/// and each point the keys up to the next (a key the clone re-authors wins);
/// and an emitter's Damage when it authors MaxDamage beside it
/// (its damage blends between the two by the charge).
pub(super) fn charge_points(db: &ClassDb, ord_name: &str, ord: &mut OrdnanceClass) {
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
        let sticky = db
            .engine_base(name)
            .is_some_and(|base| matches!(base, "sticky" | "grenade"));
        let upto = |end: usize| {
            let src = ListSrc {
                props: &props[..end],
            };
            if sticky {
                sticky_props(&src)
            } else {
                OrdnanceClass::from_props(&src)
            }
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

pub(super) fn fired_ordnance(db: &ClassDb, ord_name: &str) -> OrdnanceClass {
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
        let mut m = if db.is_bf2() {
            let props = chain_props(db, ord_name);
            let root_end = props
                .iter()
                .position(|p| p.0 == NEXT_CHARGE)
                .unwrap_or(props.len());
            core::missile::MissileClass::from_props(&ListSrc {
                props: &props[..root_end],
            })
        } else {
            core::missile::MissileClass::from_props(&ClassSrc {
                db,
                class: ord_name,
            })
        };
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
    if db
        .engine_base(ord_name)
        .is_some_and(|base| base.eq_ignore_ascii_case("towcable"))
    {
        if let (Some(initial_length), Some(length)) = (
            db.prop_f32(ord_name, "InitialCableLength"),
            db.prop_f32(ord_name, "CableLength"),
        ) {
            if initial_length.is_finite()
                && length.is_finite()
                && initial_length > 0.0
                && length > 0.0
            {
                ord.tow = Some(core::vehicle::tow::TowClass {
                    initial_length,
                    length,
                });
            }
        }
    }
    if ord.kind == OrdKind::Shell {
        // Only shells have this fall (re-ordnance-kinds §6).
        ord.gravity = db.prop_f32(ord_name, "Gravity").unwrap_or(0.0);
        // The two stock BF1 pilot tap shells omit LifeSpan. The native
        // constructor supplies 3 s (FUN_0027e598, B1:203546–203562).
        if !db.is_bf2()
            && ord.knots.is_empty()
            && [
                "imp_weap_inf_mortar_launcher_ord1",
                "cis_weap_inf_mortar_launcher_ord1",
            ]
            .iter()
            .any(|name| ord_name.eq_ignore_ascii_case(name))
            && db.prop(ord_name, "LifeSpan").is_none()
        {
            ord.life_span = 3.0;
        }
    }
    // Base ordnance expiry, shared by every family (B1 FUN_00272e98,
    // B1:195363–195367; B2 FUN_0030f988, B2:301804–301833).
    ord.expire = db
        .prop(ord_name, "ExplosionExpire")
        .and_then(|e| explosion_class(db, e));
    ord
}

/// A weapon's secondary ordnance (card UORD, `analysis/re-ordnance-kinds.md`
/// §7): `SecondaryOrdnanceName` / `SecondaryOrdnancePeriod` (B2:324171-324181),
/// the TRACER spawned beside the primary every `period`-th round. Read from
/// the stage's replayed keys `src`, as the other weapon keys are. `None` when
/// the class names no ordnance, names one the db does not hold, or authors a
/// period under 1 (never); a missing period is the PS2 class default, 1.
pub(super) fn secondary_ordnance(db: &ClassDb, src: &ListSrc<'_>) -> Option<Secondary> {
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
pub(super) fn weapon_stage(
    db: &ClassDb,
    weapon: &str,
    props: &[(u32, &str)],
) -> Result<WeaponClass> {
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

/// Sticky inherits the shell defaults before any charge segment's keys
/// are applied (`analysis/re-grenades.md` §4.1).
fn sticky_props(src: &dyn PropSource) -> OrdnanceClass {
    let mut ord = OrdnanceClass::from_props(src);
    if src.prop("Velocity").is_none() {
        ord.velocity = 35.0;
    }
    if src.prop("LifeSpan").is_none() {
        ord.life_span = 3.0;
    }
    ord
}

/// A `sticky` ordnance class (a grenade's or a charged mortar round's): the
/// shell's defaults (`LifeSpan` 3.0, `Velocity` 35.0, §4.1) where the
/// bolt's would apply, the sticky's own keys replayed in file order, and its
/// `ExplosionExpire` class.
pub(super) fn sticky_ordnance(db: &ClassDb, ord_name: &str) -> OrdnanceClass {
    let osrc = ClassSrc {
        db,
        class: ord_name,
    };
    let mut ord = sticky_props(&osrc);
    charge_points(db, ord_name, &mut ord);
    ord.impact = impact_look(db, ord_name);
    let mut sticky = core::sticky::StickyClass::replay(chain_props(db, ord_name), db.is_bf2());
    if db.engine_base(ord_name) == Some("grenade") {
        // BF1 grenade ordnance uses authored friction and rebound, and never
        // sticks. See FUN_0027b068 and FUN_0027b640.
        sticky.stick_mask = 0;
        sticky.grenade_bounce = Some((
            db.prop_f32(ord_name, "Rebound").unwrap_or(0.5),
            db.prop_f32(ord_name, "Friction").unwrap_or(0.2),
        ));
    }
    sticky.explosion = db
        .prop(ord_name, "ExplosionExpire")
        .and_then(|e| explosion_class(db, e));
    ord.sticky = Some(sticky);
    ord.kind = OrdKind::Lobbed;
    ord
}

/// A thrown weapon (card GREN, `analysis/re-grenades.md` §3, §4): a
/// `grenade` weapon class with a sticky or bouncing grenade projectile.
/// The weapon is the base parse plus the grenade class's defaults and keys
/// ([`Thrown::apply`]); the ordnance takes the shell's defaults (`LifeSpan`
/// 3.0, `Velocity` 35.0, §4.1) where the bolt's would apply, the sticky's
/// own keys replayed in file order, and its `ExplosionExpire` class.
pub fn thrown_weapon_from_db(db: &ClassDb, weapon: &str) -> Result<Option<WeaponClass>> {
    thrown_weapon(db, weapon, false)
}

pub(super) fn thrown_weapon(
    db: &ClassDb,
    weapon: &str,
    allow_bouncing: bool,
) -> Result<Option<WeaponClass>> {
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
    if !(is(ord_name, "sticky") || allow_bouncing && is(ord_name, "grenade")) {
        return Ok(None);
    }
    let ord = sticky_ordnance(db, ord_name);
    let bf2 = db.is_bf2();
    let mut w = WeaponClass::from_props(weapon, &wsrc, ord);
    Thrown::apply(&mut w, &wsrc, bf2);
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
