//! Soldier CollisionScale immunity through the same class reader as a match.
use std::path::PathBuf;

type ClassRow<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

fn class_db(rows: &[ClassRow<'_>]) -> game::classdb::ClassDb {
    use game::classdb::{fnv1a_lowercase, ClassDb, ClassDef};
    let defs: Vec<_> = rows
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
        "openbf-collision-immunity-{}-{:?}.ron",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, ron::to_string(&defs).unwrap()).unwrap();
    let mut db = ClassDb::default();
    let loaded = db.load(&path);
    std::fs::remove_file(path).unwrap();
    assert_eq!(loaded.unwrap(), rows.len());
    db
}

#[test]
fn collision_damage_immunity_replays_partial_vector_overrides_in_order() {
    let db = class_db(&[
        ("ordinary", "soldier", &[]),
        ("immune", "ordinary", &[("CollisionScale", "0 0 0")]),
        ("scalar_zero", "ordinary", &[("CollisionScale", "0")]),
        ("directional", "ordinary", &[("CollisionScale", "1 2 0")]),
        ("partial", "directional", &[("CollisionScale", "0")]),
        ("restored", "immune", &[("CollisionScale", "1")]),
        (
            "ordered",
            "ordinary",
            &[("CollisionScale", "0 0 0"), ("CollisionScale", "1")],
        ),
    ]);
    for (name, immune) in [
        ("ordinary", false),
        ("immune", true),
        ("scalar_zero", true),
        ("directional", false),
        ("partial", false),
        ("restored", false),
        ("ordered", false),
    ] {
        assert_eq!(
            game::roster::soldier_constants(&db, name).collision_damage_immune,
            immune,
            "{name}"
        );
    }
}

#[test]
#[ignore = "requires an owned class bake"]
fn collision_damage_immunity_reads_the_owned_dark_trooper_classes() {
    let root = std::env::var_os("OPENBF_COLLISION_CLASSES")
        .map(PathBuf::from)
        .unwrap_or_else(|| game::assets::baked_root().join("classes"));
    assert!(root.join("bf1_imp.ron").is_file());
    let db = game::roster::load_classdb(&root, "bf1", &["imp"]).unwrap();
    for class in ["imp_inf_dark_trooper", "imp_inf_dark_troopersnow"] {
        assert!(db.get(class).is_some());
        assert!(
            game::roster::soldier_constants(&db, class).collision_damage_immune,
            "{class}"
        );
    }
    assert!(db.get("imp_inf_storm_trooper").is_some());
    assert!(!game::roster::soldier_constants(&db, "imp_inf_storm_trooper").collision_damage_immune);
}
