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

#[test]
fn missile_class_uses_the_last_authored_keys_before_the_next_charge() {
    let d = db(&[
        (
            "base_missile",
            "missile",
            &[("TurnRate", "1"), ("WaverTurn", "0")],
        ),
        (
            "stinger",
            "base_missile",
            &[
                ("TurnRate", "0"),
                ("TurnRate", "0.5"),
                ("WaverRate", "2"),
                ("WaverTurn", "0.25"),
            ],
        ),
        (
            "charged",
            "base_missile",
            &[
                ("TurnRate", "1.25"),
                ("NextCharge", "0.5"),
                ("TurnRate", "2"),
            ],
        ),
    ]);
    let m = weapon_db::fired_ordnance(&d, "stinger").missile.unwrap();
    assert_eq!(m.turn_rate, 0.5);
    assert_eq!(m.waver_rate, 2.0);
    assert_eq!(m.waver_turn, 0.25);
    assert_eq!(
        weapon_db::fired_ordnance(&d, "charged")
            .missile
            .unwrap()
            .turn_rate,
        1.25
    );
}

#[test]
fn bf1_tap_mortar_lifespan_uses_the_default_only_when_omitted() {
    let uncharged = db(&[
        (
            "imp_weap_inf_mortar_launcher_ord1",
            "shell",
            &[("Velocity", "40"), ("Gravity", "1.75")],
        ),
        ("zero_life", "shell", &[("LifeSpan", "0")]),
        ("cis_weap_inf_mortar_launcher_ord1", "zero_life", &[]),
        ("other_shell", "shell", &[]),
    ]);
    let fired = |name| super::weapon_db::fired_ordnance(&uncharged, name);
    let mortar = fired("imp_weap_inf_mortar_launcher_ord1");
    assert_eq!(
        (mortar.velocity, mortar.gravity, mortar.life_span),
        (40.0, 1.75, 3.0)
    );
    assert_eq!(fired("cis_weap_inf_mortar_launcher_ord1").life_span, 0.0);
    assert_eq!(fired("other_shell").life_span, 2.0);

    let charged = db(&[(
        "imp_weap_inf_mortar_launcher_ord1",
        "shell",
        &[("NextCharge", "1"), ("Velocity", "25")],
    )]);
    let shell = super::weapon_db::fired_ordnance(&charged, "imp_weap_inf_mortar_launcher_ord1");
    assert_eq!(shell.life_span, 2.0);
    assert_eq!(shell.charged(1.0).life_span, 2.0);
}

#[test]
fn sticky_charge_points_survive_both_weapon_loaders() {
    let d = db(&[
        (
            "base_ord",
            "sticky",
            &[("Velocity", "5"), ("LifeSpan", "0.25")],
        ),
        (
            "test_sticky",
            "base_ord",
            &[
                ("NextCharge", "1"),
                ("Velocity", "25"),
                ("LifeSpan", "1.25"),
            ],
        ),
        ("test_throw", "grenade", &[("OrdnanceName", "test_sticky")]),
    ]);
    for w in [
        weapon_from_db(&d, "test_throw").unwrap(),
        thrown_weapon_from_db(&d, "test_throw").unwrap().unwrap(),
    ] {
        assert!(w.ordnance.sticky.is_some());
        assert_eq!((w.ordnance.velocity, w.ordnance.life_span), (5.0, 0.25));
        let blend = w.ordnance.charged(0.5);
        assert_eq!((blend.velocity, blend.life_span), (15.0, 0.75));
    }
}

#[test]
fn sticky_charge_segments_keep_shell_defaults_until_each_key_is_authored() {
    let d = db(&[
        ("base_ord", "sticky", &[]),
        (
            "test_sticky",
            "base_ord",
            &[
                ("NextCharge", "0.25"),
                ("NextCharge", "0.5"),
                ("Velocity", "70"),
                ("NextCharge", "1"),
                ("LifeSpan", "5"),
            ],
        ),
        ("test_throw", "grenade", &[("OrdnanceName", "test_sticky")]),
        ("plain_throw", "grenade", &[("OrdnanceName", "base_ord")]),
        (
            "test_bolt",
            "bolt",
            &[("NextCharge", "1"), ("Velocity", "70")],
        ),
        ("test_gun", "cannon", &[("OrdnanceName", "test_bolt")]),
    ]);
    for thrown in [false, true] {
        let load = |name: &str| {
            if thrown {
                thrown_weapon_from_db(&d, name).unwrap().unwrap()
            } else {
                weapon_from_db(&d, name).unwrap()
            }
        };
        let w = load("test_throw");
        assert_eq!((w.ordnance.velocity, w.ordnance.life_span), (35.0, 3.0));
        assert_eq!(w.ordnance.knots.len(), 3);
        for (charge, speed, life) in [
            (0.0, 35.0, 3.0),
            (0.25, 35.0, 3.0),
            (0.375, 52.5, 3.0),
            (0.5, 70.0, 3.0),
            (0.75, 70.0, 4.0),
            (1.0, 70.0, 5.0),
        ] {
            let blend = w.ordnance.charged(charge);
            assert_eq!((blend.velocity, blend.life_span), (speed, life));
        }
        let plain = load("plain_throw");
        assert!(plain.ordnance.knots.is_empty());
        let blend = plain.ordnance.charged(1.0);
        assert_eq!((blend.velocity, blend.life_span), (35.0, 3.0));
    }
    // The sticky defaults must not change the ordinary bolt parser.
    let bolt = weapon_from_db(&d, "test_gun").unwrap().ordnance;
    assert_eq!((bolt.velocity, bolt.life_span), (150.0, 2.0));
    let full = bolt.charged(1.0);
    assert_eq!((full.velocity, full.life_span), (70.0, 2.0));
}
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
fn skeleton_scale_follows_authored_order_into_ai_and_player_specs() {
    use core::unit::Controller;
    use glam::Vec3;

    let d = db(&[
        ROUND,
        ("test_gun", "cannon", &[("OrdnanceName", "test_ord")]),
        ("base_unit", "soldier", &[("WeaponName1", "test_gun")]),
        (
            "scaled_unit",
            "base_unit",
            &[
                ("SkeletonRootScale", "1.15"),
                ("CollisionRootScale", "0.73"),
            ],
        ),
        ("inherited_unit", "scaled_unit", &[]),
        (
            "override_unit",
            "scaled_unit",
            &[("SkeletonRootScale", "1.2"), ("SkeletonRootScale", "0.95")],
        ),
    ]);
    for (name, root_scale, collision_scale) in [
        ("base_unit", 1.0, 1.0),
        ("inherited_unit", 1.15, 0.73),
        ("override_unit", 0.95, 0.73),
    ] {
        let class = ClassRoster::build(&d, name, 1, None).unwrap();
        let mut sim = core::sim::Sim::empty(7);
        let registered = class.register(&mut sim, &mut Vec::new());
        let roster = FactionRoster {
            faction: "all".into(),
            classes: vec![class],
        };
        let template = crate::walk::TeamTemplate::from_roster(&roster, &registered, Vec::new());
        let ai = registered.spec(0, Vec3::ZERO, 0.0, Controller::Ai, 0, 0);
        let player = template.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0);
        for spec in [ai, player] {
            let unit = spec.build(1, &sim.weapon_classes);
            assert_eq!(unit.consts.skeleton_root_scale, root_scale, "{name}");
            assert_eq!(unit.consts.collision_scale, collision_scale, "{name}");
        }
    }
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
