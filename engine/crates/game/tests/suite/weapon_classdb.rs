//! Tier-1 (data-gated): weapon/ordnance classes reach the runtime `ClassDb`
//! with their BASE chains intact.
//!
//! BF2 munges weapons as wpnc->wpnc (rep_weap_inf_rifle skins
//! com_weap_inf_rifle, which holds the stats), exactly like the BF2 entc
//! soldier chain. Byte-verified values: analysis/re-blaster-damage.md §8.

use game::classdb::{fnv1a_lowercase, ClassDb};
use std::path::PathBuf;

fn lvl(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted")).join(rel)
}

/// Bake the given LVLs into one ClassDb, or `None` if the data is absent.
pub(super) fn db(rels: &[&str], tag: &str) -> Option<ClassDb> {
    let paths: Vec<PathBuf> = rels.iter().map(|r| lvl(r)).collect();
    if let Some(missing) = paths.iter().find(|p| !p.exists()) {
        eprintln!("SKIP: {} not present", missing.display());
        return None;
    }
    let dir = std::env::temp_dir().join(format!("openbf-weapondb-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut db = ClassDb::default();
    for (i, p) in paths.iter().enumerate() {
        let ron = dir.join(format!("{i}.ron"));
        bake::classes::bake_classes(p.to_str().unwrap(), ron.to_str().unwrap()).expect("bake");
        db.load(&ron).expect("load");
    }
    let _ = std::fs::remove_dir_all(&dir);
    Some(db)
}

/// BF2: rep_weap_inf_rifle -> com_weap_inf_rifle -> engine base "cannon".
#[test]
fn bf2_weapon_base_chain_resolves_through_wpnc() {
    let Some(db) = db(
        &[
            "bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL",
            "bf2/lvl/DATA/_LVL_PS2/INGAME.LVL",
        ],
        "bf2",
    ) else {
        return;
    };
    let c = "rep_weap_inf_rifle";
    assert_eq!(db.get(c).map(|d| d.kind.as_str()), Some("wpnc"));
    assert!(db.get_kind("wpnc", c).is_some());
    assert!(db.get_kind("entc", c).is_none());
    assert_eq!(db.engine_base(c), Some("cannon"));
    // Inherited from com_weap_inf_rifle (§8.2).
    assert_eq!(db.prop_f32(c, "ShotDelay"), Some(0.2));
    assert_eq!(db.prop_f32(c, "SpreadPerShot"), Some(1.2));
    assert_eq!(db.prop_f32(c, "SpreadLimit"), Some(5.0));
    assert_eq!(db.prop_f32(c, "SpreadRecoverRate"), Some(4.8));
    assert_eq!(db.prop_f32(c, "LockOnRange"), Some(40.0));
    assert_eq!(db.prop(c, "RoundsPerClip"), Some("50"));
    // Overridden by the child skin.
    assert_eq!(db.prop(c, "OrdnanceName"), Some("rep_weap_inf_rifle_ord"));
    assert_eq!(db.prop(c, "FireSound"), Some("rep_weap_inf_rifle_fire"));

    // Ordnance chain: rep_..._ord is a colour reskin over com_..._ord.
    let o = "rep_weap_inf_rifle_ord";
    assert_eq!(db.get(o).map(|d| d.kind.as_str()), Some("ordc"));
    assert_eq!(db.engine_base(o), Some("bolt"));
    assert_eq!(db.prop_f32(o, "MaxDamage"), Some(80.0));
    assert_eq!(db.prop_f32(o, "Velocity"), Some(160.0));
    assert_eq!(db.prop_f32(o, "LifeSpan"), Some(0.8));
    assert_eq!(db.prop_f32(o, "VehicleScale"), Some(0.1));
    // Falloff block, keys known only by hash (§7.3).
    assert_eq!(db.prop_hash_f32(o, 0xA745_61B2), Some(0.4));
    assert_eq!(db.prop_hash_f32(o, 0xEE08_D1BA), Some(0.2));
    assert_eq!(db.prop_hash_f32(o, 0x955F_71E9), Some(50.0));
    assert_eq!(db.prop_hash_f32(o, 0x9748_1656), Some(0.25));
}

/// BF1 rifle: flat chain (wpnc -> engine base "cannon"), no falloff keys.
#[test]
fn bf1_weapon_classes_load_flat() {
    let Some(db) = db(
        &[
            "bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL",
            "bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL",
        ],
        "bf1",
    ) else {
        return;
    };
    let c = "all_weap_inf_rifle";
    assert_eq!(db.get(c).map(|d| d.kind.as_str()), Some("wpnc"));
    assert_eq!(db.engine_base(c), Some("cannon"));
    assert_eq!(db.prop_f32(c, "ShotDelay"), Some(0.2));
    assert_eq!(db.prop_f32(c, "LockOnRange"), Some(64.0));
    let o = "all_weap_inf_rifle_ord";
    assert_eq!(db.engine_base(o), Some("bolt"));
    assert_eq!(db.prop_f32(o, "MaxDamage"), Some(60.0));
    assert_eq!(db.prop_f32(o, "VehicleScale"), Some(0.0));
    assert_eq!(db.prop_hash(o, 0xA745_61B2), None); // BF1 has no falloff
                                                    // entc and wpnc coexist in one db without shadowing each other.
    assert_eq!(
        db.get("all_inf_soldier").map(|d| d.kind.as_str()),
        Some("entc")
    );
    assert_eq!(db.prop_f32("all_inf_soldier", "MaxSpeed"), Some(6.5));
    // The soldier's slot-1 weapon resolves to the rifle wpnc.
    let w = db
        .prop("all_inf_soldier", "WeaponName1")
        .expect("WeaponName1");
    assert_eq!(w, c);
    assert_eq!(fnv1a_lowercase("WeaponName1"), 0x2CE1_A1D1);
}

#[test]
fn bf1_stormtrooper_uses_disc_ammo_and_exhausts_it_by_firing() {
    use core::lcg::GameLcg;
    use core::sim::Sim;
    use core::unit::Controller;
    use core::weapon::{FireInput, SPARE_INFINITE};
    use game::roster::ClassRoster;
    use glam::Vec3;

    let Some(db) = db(
        &[
            "bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL",
            "bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL",
        ],
        "bf1-ammo",
    ) else {
        return;
    };
    let roster = ClassRoster::build(&db, "imp_inf_storm_trooper", 1, None).unwrap();
    assert_eq!(roster.weapon.name, "imp_weap_inf_rifle");
    assert_eq!(roster.weapon.rounds_per_clip, 55);
    assert_eq!(roster.total_clips, 5);
    let mut sim = Sim::empty(1);
    let class = roster.register(&mut sim, &mut Vec::new());
    let spec = class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0);
    let mut u = spec.build(0, &sim.weapon_classes);
    let wc = &sim.weapon_classes[u.weapon_class];
    assert_eq!((u.weapon.clip, u.weapon.spare), (55, 220));
    assert_eq!(u.weapon.rounds(wc), Some(275));
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    for _ in 0..30 * 100 {
        u.weapon.update(
            core::TICK_DT,
            wc,
            FireInput {
                held: true,
                ..FireInput::default()
            },
            &mut lcg,
            &mut shots,
        );
    }
    assert_eq!(shots.len(), 275);
    assert_eq!(u.weapon.rounds(wc), Some(0));
    let pistol = u
        .held
        .iter()
        .find(|w| sim.weapon_classes[w.class].name == "imp_weap_inf_pistol")
        .unwrap();
    assert_eq!(pistol.state.spare, SPARE_INFINITE);
    assert_eq!(pistol.state.rounds(&sim.weapon_classes[pistol.class]), None);
}

/// Overheat click: each game's pistols name their own `FireEmptySound`, which
/// the class loader keeps. A hand weapon the disc gives no click stays without
/// one. `sounds::tests` resolves the two takes in each map's sound bank.
#[test]
fn both_games_pistols_click_with_their_own_take() {
    let classes = game::assets::baked_root().join("classes");
    let ammo = "com_weap_inf_ammo_empty";
    let weapon = "com_weap_inf_weapon_empty";
    let cases = [
        (
            "bf1",
            [
                ("rep_weap_inf_pistol", Some(ammo)),
                ("imp_weap_inf_pistol", Some(weapon)),
                ("rep_weap_inf_commando_pistol", None),
            ],
        ),
        (
            "bf2",
            [
                ("rep_weap_inf_pistol", Some(ammo)),
                ("imp_weap_inf_pistol", Some(weapon)),
                ("rep_weap_inf_remotedroid_blaster", None),
            ],
        ),
    ];
    for (game_id, weapons) in cases {
        let Ok(db) = game::roster::load_classdb(&classes, game_id, &["all", "imp", "rep", "cis"])
        else {
            eprintln!("SKIP: {game_id} classes are not baked");
            continue;
        };
        for (name, want) in weapons {
            let w = game::roster::weapon_from_db(&db, name).expect(name);
            assert_eq!(w.empty_sound.as_deref(), want, "{game_id} {name}");
            assert!(w.heat_per_shot > 0.0, "{game_id} {name}: a heat weapon");
        }
    }
}
