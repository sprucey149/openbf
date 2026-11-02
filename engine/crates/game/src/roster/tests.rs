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
