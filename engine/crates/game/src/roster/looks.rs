//! Muzzle flash, laser, impact and explosion looks read from the class db.

use super::*;

/// Weapon labels whose render never reaches the flash draw, whatever their
/// `MuzzleFlash` says (`analysis/re-muzzle-flash.md` §5): the melee family
/// (WeaponMelee overrides the render slot — the sabres) and the thrown or
/// placed types, whose weapon+0xbc skips the render (Detonator, Dispenser,
/// Remote, Grenade).
pub(super) const NO_FLASH_BASES: [&str; 6] = [
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
/// or `laser` label, or a `beam`, whose own setter it replays
/// ([`LaserLook::replay_beam`]). The root look stops at the first
/// `NextCharge`; the renderer currently uses one look for all charges.
pub fn laser_look(db: &ClassDb, ordnance: &str) -> Option<LaserLook> {
    let base = db.engine_base(ordnance)?;
    let beam = base.eq_ignore_ascii_case("beam");
    if !beam
        && !["bolt", "laser"]
            .iter()
            .any(|b| base.eq_ignore_ascii_case(b))
    {
        return None;
    }
    let props = chain_props(db, ordnance)
        .into_iter()
        .take_while(|(hash, _)| *hash != super::weapon_db::NEXT_CHARGE);
    Some(if beam {
        LaserLook::replay_beam(props, db.is_bf2())
    } else {
        LaserLook::replay(props)
    })
}

/// Engine labels whose setter chains into the bullet base that reads the
/// `ImpactEffect*` surface keys (callers of B1 FUN_002774b0 / B2
/// FUN_00314b98; `analysis/re-fx.md` §7, AGENT_C §2 note 8).
pub(super) const BULLET_BASES: [&str; 8] = [
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
/// bullet base's surface slots (ctor defaults, then the chain), or a beam's
/// one `ImpactEffect` ([`ImpactLook::beam`]), its `TrailEffect` and its
/// `ExplosionName` class's look. `None` when the class is not in the db.
pub fn impact_look(db: &ClassDb, ordnance: &str) -> Option<ImpactLook> {
    let base = db.engine_base(ordnance)?;
    let bf2 = db.is_bf2();
    let mut l = ImpactLook::replay(chain_props(db, ordnance), bf2);
    if base.eq_ignore_ascii_case("beam") {
        l = ImpactLook::beam(chain_props(db, ordnance));
    } else if !BULLET_BASES.iter().any(|b| base.eq_ignore_ascii_case(b)) {
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
pub(super) const IMPACT_EXPLOSION_KEYS: [u32; 3] = [0x02bb_1fe0, 0x61a8_bb12, 0x9b30_e763];

/// An ordnance's impact explosion class name (card RKEX): the last of the
/// three keys in the chain's file order, base first - each writes the same
/// field. The BF2 EMP launcher names it by `ExplosionImpact`, the rocket
/// launchers by `ExplosionName`.
pub(super) fn impact_explosion<'a>(db: &'a ClassDb, ordnance: &str) -> Option<&'a str> {
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
            .prop(class, "ExplosionDestruct")
            .or_else(|| db.prop(class, "ExplosionName"))
            .or_else(|| db.prop(class, "Explosion"))
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
    Some(ExplosionLook {
        class: explosion.trim().to_ascii_lowercase(),
        ..ExplosionLook::replay(chain_props(db, explosion), db.is_bf2())
    })
}

pub(super) fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
