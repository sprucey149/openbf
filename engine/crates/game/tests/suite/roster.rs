//! Tier-1 (data-gated): factions build from the baked side classes — every
//! soldier class of a faction, the mission's own mix when the map ships one,
//! the rifles reproducing the byte-verified fixtures, and soldier constants
//! from the ODF chain (era mix loads both games).

use core::weapon::{HealthType, WeaponClass};
use game::mission::MissionDef;
use game::normalize::BF1_SCALE;
use game::roster::{ClassRoster, FactionRoster};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

type Gcl16ClassRow<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

#[test]
fn hero_sprint_reads_authored_speed_and_camera_sections() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_rep.ron").is_file() {
        return;
    }
    let db =
        game::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    for hero in [
        "rep_hero_aalya",
        "cis_hero_countdooku",
        "rep_hero_obiwan",
        "imp_hero_emperor",
    ] {
        let c = game::roster::soldier_constants(&db, hero);
        assert_eq!(c.sprint_control, [2.5, 0.5]);
        assert_eq!(c.sprint_accelerate_time, 0.35);
        let standing = c.hero_cameras[0].unwrap();
        let sprint = c.hero_cameras[6].unwrap();
        assert_eq!((standing.eye_y, standing.offset.z), (2.0, 5.2));
        assert_eq!((sprint.eye_y, sprint.offset.z), (1.8, 4.0));
        let mut soldier = core::soldier::Soldier::new(glam::Vec3::ZERO);
        let pad = core::sim::PadState {
            ly: i16::MAX,
            buttons: core::sim::buttons::SPRINT,
            ..Default::default()
        };
        for _ in 0..60 {
            let y = soldier.pos.y;
            soldier.step(core::TICK_DT, &pad, &c, core::soldier::GroundRules::Bf2);
            soldier.resolve_ground(
                core::TICK_DT,
                0.0,
                glam::Vec3::Y,
                y,
                core::soldier::GroundRules::Bf2,
            );
        }
        assert_eq!(soldier.stance, core::soldier::Stance::Sprint);
        assert!((-soldier.vel.z - c.max_speed * 2.5).abs() < 0.001);
    }
    for game in ["bf1", "bf2"] {
        let db = game::roster::load_classdb(&root.join("classes"), game, &["all"]).unwrap();
        let c = game::roster::soldier_constants(&db, "all_inf_soldier");
        assert_eq!(c.sprint_control, [1.5, 0.5]);
        assert!(c.hero_cameras.iter().all(Option::is_none));
    }
}

/// Asset-free class RONs exercise the same load path as baked classes.
fn gcl16_db(rows: &[Gcl16ClassRow<'_>]) -> game::classdb::ClassDb {
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
        "openbf-gcl16-{}-{:?}.ron",
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
fn gcl16_animation_banks_resolve_slot_case_and_preserve_priority() {
    use game::roster::class_anim;
    for spelling in ["AuthoredWeapon", "aUtHoReDwEaPoN"] {
        let db = gcl16_db(&[
            ("trooper", "soldier", &[("WeaponName1", spelling)]),
            (
                "bank_parent",
                "cannon",
                &[("AnimationBank", "Pistol extra")],
            ),
            ("AuthoredWeapon", "bank_parent", &[]),
        ]);
        let anim = class_anim(&db, "trooper", &db.weapon_slots("trooper"));
        assert_eq!(anim.bank_of(spelling), "pistol", "{spelling}");
        assert_eq!(anim.bank_of("AUTHOREDWEAPON"), "pistol");
        assert_eq!(anim.bank_of("absent"), "rifle");
    }
    for (props, expected) in [
        (
            vec![
                ("AnimationBank", "Pistol"),
                ("CustomAnimationBank", "Tool"),
                ("ComboAnimationBank", "Saber"),
            ],
            "saber",
        ),
        (
            vec![("AnimationBank", "Pistol"), ("CustomAnimationBank", "Tool")],
            "tool",
        ),
        (vec![], "rifle"),
    ] {
        let db = gcl16_db(&[
            ("trooper", "soldier", &[("WeaponName1", "weapon")]),
            ("weapon", "cannon", &props),
        ]);
        assert_eq!(
            class_anim(&db, "trooper", &db.weapon_slots("trooper")).bank_of("weapon"),
            expected
        );
    }
}

#[test]
fn gcl16_duplicate_weapon_keeps_selected_primary_slot() {
    let db = gcl16_db(&[
        (
            "parent",
            "soldier",
            &[
                ("WeaponName1", "grenade_weapon"),
                ("WeaponChannel1", "1"),
                ("WeaponAmmo1", "3"),
                ("WeaponName2", "grenade_weapon"),
                ("WeaponChannel2", "0"),
                ("WeaponAmmo2", "9"),
            ],
        ),
        ("trooper", "parent", &[("WeaponAmmo2", "7")]),
        ("grenade_weapon", "grenade", &[("OrdnanceName", "shell")]),
        ("shell", "sticky", &[]),
    ]);
    let roster = ClassRoster::build(&db, "trooper", 1, None).unwrap();
    assert_eq!(
        game::roster::primary_weapon(&db, "trooper"),
        Some(("grenade_weapon".into(), 7))
    );
    assert_eq!((roster.weapon_slot, roster.total_clips), (1, 7));
    assert_eq!(roster.weapon.name, "grenade_weapon");
    assert_eq!(roster.slots, db.weapon_slots("trooper"));
    let held: Vec<_> = roster
        .held
        .iter()
        .map(|h| (h.slot, h.channel, h.clips, h.weapon.name.as_str()))
        .collect();
    assert_eq!(held, [(0, 1, 3, "grenade_weapon")]);
}

#[test]
fn gcl16_primary_slot_controls() {
    for (channel, selected, clips, held_slot) in [(0, 1, 9, 0), (1, 0, 3, 1)] {
        let db = gcl16_db(&[
            (
                "trooper",
                "soldier",
                &[
                    ("WeaponName1", "first"),
                    ("WeaponChannel1", "1"),
                    ("WeaponAmmo1", "3"),
                    ("WeaponName2", "second"),
                    ("WeaponChannel2", &channel.to_string()),
                    ("WeaponAmmo2", "9"),
                ],
            ),
            ("first", "grenade", &[("OrdnanceName", "shell")]),
            ("second", "grenade", &[("OrdnanceName", "shell")]),
            ("shell", "sticky", &[]),
        ]);
        let roster = ClassRoster::build(&db, "trooper", 1, None).unwrap();
        let name = if selected == 0 { "first" } else { "second" };
        assert_eq!((roster.weapon_slot, roster.total_clips), (selected, clips));
        assert_eq!(roster.weapon.name, name);
        assert_eq!(
            game::roster::primary_weapon(&db, "trooper"),
            Some((name.into(), clips))
        );
        assert_eq!(roster.held.len(), 1);
        assert_eq!(roster.held[0].slot, held_slot);
    }
}

/// The baked class RONs, baked ONCE per test binary: every test reads the
/// same ten files, so the first caller bakes them and the rest share the dir.
fn classes_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(bake_classes_dir).clone()
}

/// The shared dir's pid-scoped path. A static is never dropped, so the dir
/// goes at process exit instead (queue CLN3: `/tmp` is a tmpfs and ci runs
/// many times a night); `OPENBF_KEEP_SCRATCH=1` keeps it for triage.
fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!("openbf-roster-classes-{}", std::process::id()))
}

extern "C" fn remove_scratch_dir() {
    let _ = std::fs::remove_dir_all(scratch_dir());
}

extern "C" {
    fn atexit(f: extern "C" fn()) -> std::os::raw::c_int;
}

fn bake_classes_dir() -> Option<PathBuf> {
    let ex = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted"));
    // All four sides of BOTH discs: a BF2 GCW map fields `all`/`imp` out of
    // BF2's own SIDE LVLs and a BF1 CW map fields `rep`/`cis` out of BF1's.
    let files = [
        ("bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL", "bf1_all.ron"),
        ("bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL", "bf1_imp.ron"),
        ("bf1/lvl/DATA/_LVL_PS2/SIDE/REP.LVL", "bf1_rep.ron"),
        ("bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL", "bf1_cis.ron"),
        (
            "bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL",
            "bf1_common.ron",
        ),
        ("bf2/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL", "bf2_all.ron"),
        ("bf2/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL", "bf2_imp.ron"),
        ("bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL", "bf2_rep.ron"),
        ("bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL", "bf2_cis.ron"),
        ("bf2/lvl/DATA/_LVL_PS2/INGAME.LVL", "bf2_ingame.ron"),
    ];
    if files.iter().any(|(rel, _)| !ex.join(rel).exists()) {
        eprintln!("SKIP: extracted side LVLs not present");
        return None;
    }
    let dir = scratch_dir();
    std::fs::create_dir_all(&dir).unwrap();
    if std::env::var_os("OPENBF_KEEP_SCRATCH").is_none() {
        // SAFETY: libc's `atexit` with a plain `extern "C"` callback that
        // cannot unwind (it discards the only fallible call's result).
        unsafe { atexit(remove_scratch_dir) };
    }
    for (rel, out) in files {
        bake::classes::bake_classes(
            ex.join(rel).to_str().unwrap(),
            dir.join(out).to_str().unwrap(),
        )
        .expect("bake classes");
    }
    Some(dir)
}

fn missions_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
}

/// The baked missions, sorted. `None`, with a SKIP line, when this tree has no
/// mission bake at all: a clone holding the discs but not yet baked, or an empty
/// data link. A bake that has missions but too few, or lacks a named one, still
/// fails in its test: that is a stale bake.
fn mission_files() -> Option<Vec<std::path::PathBuf>> {
    let mut files: Vec<_> = std::fs::read_dir(missions_dir())
        .into_iter()
        .flatten()
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    if files.is_empty() {
        eprintln!("SKIP: no baked missions in {}", missions_dir().display());
        return None;
    }
    files.sort();
    Some(files)
}

/// One baked mission by file name; `None` as for `mission_files`.
fn mission(file: &str) -> Option<MissionDef> {
    mission_files()?;
    Some(MissionDef::load(&missions_dir().join(file)).expect(file))
}

/// Every faction, for the tests that want one disc's WHOLE class db rather
/// than the two sides a particular round fields.
const FOUR: [&str; 4] = ["all", "imp", "rep", "cis"];

fn load_classdb_all(dir: &Path, game: &str) -> game::classdb::ClassDb {
    let mut db = game::roster::load_classdb(dir, game, &FOUR).expect("class db");
    // Class values are freshly extracted above; hero model/animation companions
    // belong to the asset overlay, not the temporary class directory's parent.
    db.set_assets_root(&game::assets::baked_root());
    db
}

fn row(c: &ClassRoster) -> String {
    format!(
        "  {:<28} {:<22} {:<28} dmg {:>6.1}  vel {:>6.1}  clip {:>3}  delay {:?}  hp {:>7.1}  \
         spd {:.1}  jet {}  min {} max {:?}",
        c.class,
        c.label,
        c.weapon.name,
        c.weapon.ordnance.max_damage,
        c.weapon.ordnance.velocity,
        c.weapon.rounds_per_clip,
        c.weapon.shot_delay,
        c.max_health,
        c.consts.max_speed,
        c.jet,
        c.min,
        c.max,
    )
}

/// Every faction's DEFAULT list builds, with distinct primary weapons and a
/// real ordnance behind each — the class table the README prints.
#[test]
fn every_faction_builds_its_whole_default_class_list() {
    let Some(dir) = classes_dir() else { return };
    for (game, a, b) in [
        ("bf1", "all", "imp"),
        ("bf1", "rep", "cis"),
        ("bf2", "all", "imp"),
        ("bf2", "rep", "cis"),
    ] {
        let ([ra, rb], _) = FactionRoster::pair(&dir, game, [a, b], None).expect("default rosters");
        println!("== {game} map: {a} vs {b}");
        for r in [&ra, &rb] {
            println!("faction {} — {} classes", r.faction, r.classes.len());
            for c in &r.classes {
                println!("{}", row(c));
            }
            assert!(
                r.classes.len() >= 5,
                "{}: only {} classes",
                r.faction,
                r.classes.len()
            );
            let mut weapons: Vec<&str> = r.classes.iter().map(|c| c.weapon.name.as_str()).collect();
            let n = weapons.len();
            weapons.sort_unstable();
            weapons.dedup();
            assert_eq!(
                weapons.len(),
                n,
                "{}: two classes share a primary",
                r.faction
            );
            for c in &r.classes {
                assert!(
                    c.weapon.ordnance.max_damage > 0.0 && c.weapon.ordnance.velocity > 0.0,
                    "{} {}: ordnance {:?}",
                    r.faction,
                    c.class,
                    c.weapon.ordnance
                );
                assert!(!c.label.is_empty());
            }
        }
    }
}

/// The class-0 (fill) class of each faction still reproduces the v0 numbers:
/// the byte-verified rifle fixtures and the ODF-chain soldier constants.
#[test]
fn fill_classes_keep_the_v0_fixtures() {
    let Some(dir) = classes_dir() else { return };
    let ([all, imp], _) =
        FactionRoster::pair(&dir, "bf1", ["all", "imp"], None).expect("bf1 rosters");
    let a = all.fill_class();
    assert_eq!(a.class, "all_inf_soldier");
    // BF1's trooper takes the shared 400 health (`game::normalize`).
    assert_eq!(a.max_health, 400.0);
    assert_eq!(a.consts.max_speed, 6.5);
    assert_eq!(a.consts.max_strafe_speed, 4.5);
    assert_eq!(a.consts.max_turn_speed, 4.0);
    assert_eq!(a.consts.acceleration, 70.0);
    assert_eq!(a.total_clips, 6);
    // The disc's rifle, hitting soldiers and shields 4/3 as hard: 80.
    let mut want = WeaponClass::bf1_rifle();
    for scale in &mut want.ordnance.scales[..3] {
        *scale *= BF1_SCALE;
    }
    want.ordnance.shield_scale *= BF1_SCALE;
    // It gives way to BF2's award rifle, which the class carries.
    want.medal_lock = Some(1);
    assert_eq!(a.weapon, want, "all rifle == §8.1 fixture, normalized");
    assert_eq!(
        a.weapon.ordnance.max_damage * a.weapon.ordnance.scales[HealthType::Person as usize],
        80.0
    );
    let i = imp.fill_class();
    assert_eq!(i.weapon.name, "imp_weap_inf_rifle");
    assert_eq!(i.weapon.ordnance.max_damage, 60.0);
    assert_eq!(i.total_clips, 5);

    // BF2 default matchup: constants through the four-level chain.
    let ([rep, cis], _) =
        FactionRoster::pair(&dir, "bf2", ["rep", "cis"], None).expect("bf2 rosters");
    let r = rep.fill_class();
    assert_eq!(r.class, "rep_inf_ep3_rifleman");
    assert_eq!(r.max_health, 400.0);
    assert_eq!(r.consts.max_speed, 7.0);
    assert_eq!(r.consts.max_turn_speed, 5.0);
    assert_eq!(
        r.consts.roll_speed_factor, 1.5,
        "ODF overrides the 1.7 ctor default"
    );
    assert_eq!(r.consts.energy_cost_roll, 25.0);
    assert_eq!(r.consts.sprint_accelerate_time, 0.35);
    let mut want = WeaponClass::bf2_rifle();
    want.name = "rep_weap_inf_rifle".into();
    // The disc's rifle is the one the award rifle replaces.
    want.medal_lock = Some(1);
    assert_eq!(
        r.weapon, want,
        "rep rifle == §8.2 fixture via the ordc reskin chain"
    );
    assert_eq!(r.total_clips, 4);
    let c = cis.fill_class();
    assert_eq!(c.health_type, HealthType::Droid);
    assert_eq!(c.weapon.name, "cis_weap_inf_wrist_blaster");
    assert!(c.weapon.ordnance.velocity > 0.0 && c.weapon.ordnance.max_damage > 0.0);

    // Era mix (`--teams all,cis`): BOTH sides come off the MAP's disc, so the
    // same pair of faction keys fields different classes on a BF1 map than on
    // a BF2 one — which is the whole point of choosing the db by map.
    let ([a, c], _) = FactionRoster::pair(&dir, "bf1", ["all", "cis"], None).expect("bf1 era mix");
    assert_eq!(
        (a.fill_class().class.as_str(), c.fill_class().class.as_str()),
        ("all_inf_soldier", "cis_inf_battledroid")
    );
    let ([a2, c2], _) =
        FactionRoster::pair(&dir, "bf2", ["all", "cis"], None).expect("bf2 era mix");
    assert_eq!(
        (
            a2.fill_class().class.as_str(),
            c2.fill_class().class.as_str()
        ),
        ("all_inf_rifleman", "cis_inf_rifleman")
    );
    assert!(FactionRoster::pair(&dir, "bf1", ["all", "ewok"], None).is_err());
}

/// BF1 `AddUnitClass` order reaches the roster: tat1i's Alliance is five
/// classes with the desert trooper first and its own per-class count.
#[test]
fn tat1_alliance_roster_is_the_missions_mix() {
    let Some(dir) = classes_dir() else { return };
    let Some(m) = mission("tat1_con.ron") else {
        return;
    };
    // tat1i's teams are (imp, all) in MISSION order.
    assert_eq!(m.factions(), ["imp".to_string(), "all".to_string()]);
    let ([imp, all], _) =
        FactionRoster::pair(&dir, "bf1", ["imp", "all"], Some(&m)).expect("tat1 rosters");
    let got: Vec<(&str, u32, Option<u32>)> = all
        .classes
        .iter()
        .map(|c| (c.class.as_str(), c.min, c.max))
        .collect();
    assert_eq!(
        got,
        [
            ("all_inf_soldierdesert", 11, None),
            ("all_inf_vanguard", 3, None),
            ("all_inf_pilot", 4, None),
            ("all_inf_marksman", 4, None),
            ("all_inf_smuggler", 1, None),
        ]
    );
    for c in &all.classes {
        println!("{}", row(c));
    }
    assert_eq!(all.fill_class().class, "all_inf_soldierdesert");
    assert_eq!(all.classes[3].label, "Rebel Scout");
    // The scout takes BF2's sniper health (`game::normalize`; 240 on the disc).
    assert_eq!(all.classes[3].max_health, 300.0);
    assert_eq!(all.classes[4].geometry.as_deref(), Some("all_inf_wookie"));
    let imp_names: Vec<&str> = imp.classes.iter().map(|c| c.class.as_str()).collect();
    assert_eq!(
        imp_names,
        [
            "imp_inf_storm_trooper",
            "imp_inf_shock_trooper",
            "imp_inf_pilottie",
            "imp_inf_scout_trooper",
            "imp_inf_dark_trooper",
        ]
    );
    assert_eq!(imp.classes.iter().map(|c| c.min).sum::<u32>(), 12);
}

/// BF2 `SetupTeams` slot order reaches the roster with both counts, and the
/// CIS `special` slot — the droideka, a `walkerdroid` — builds as a droideka.
#[test]
fn kam1_clone_wars_rosters_carry_slot_order_min_and_max() {
    let Some(dir) = classes_dir() else { return };
    // `kam1c_con` is the clone-wars era of BF2 Kamino.
    let Some(m) = mission("bf2_kam1_con_cw.ron") else {
        return;
    };
    assert_eq!(m.factions(), ["cis".to_string(), "rep".to_string()]);
    let ([cis, rep], _) =
        FactionRoster::pair(&dir, "bf2", ["cis", "rep"], Some(&m)).expect("kam1 rosters");
    let got: Vec<(&str, u32, Option<u32>)> = rep
        .classes
        .iter()
        .map(|c| (c.class.as_str(), c.min, c.max))
        .collect();
    assert_eq!(
        got,
        [
            ("rep_inf_ep3_rifleman", 9, Some(25)),
            ("rep_inf_ep3_rocketeer", 1, Some(4)),
            ("rep_inf_ep3_engineer", 1, Some(4)),
            ("rep_inf_ep3_sniper", 1, Some(4)),
            ("rep_inf_ep3_officer", 1, Some(4)),
            ("rep_inf_ep3_jettrooper", 1, Some(4)),
        ],
        "SetupTeams slot order: soldier, assault, engineer, sniper, officer, special"
    );
    for c in &rep.classes {
        println!("{}", row(c));
    }
    let cis_names: Vec<&str> = cis.classes.iter().map(|c| c.class.as_str()).collect();
    assert_eq!(
        cis_names,
        [
            "cis_inf_rifleman",
            "cis_inf_rocketeer",
            "cis_inf_engineer",
            "cis_inf_sniper",
            "cis_inf_officer",
            "cis_inf_droideka",
        ],
    );
    let deka = &cis.classes[5];
    let c = deka.droideka.as_ref().expect("a droideka class");
    assert_eq!(deka.max_health, 550.0);
    assert_eq!(c.shield.max, 3000.0);
    assert!(!c.shield.drains_full);
    assert!(deka.chunk.is_none());
    assert!(cis.classes[..5].iter().all(|c| c.droideka.is_none()));
}

/// The droideka builds by its BASE chain on either disc, from BF2's gameplay
/// data. BF1 heroes are soldier chains, but the default conquest class lists
/// exclude them.
#[test]
fn droideka_chains_build_as_droidekas() {
    let Some(dir) = classes_dir() else { return };
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    assert!(bf2.get("cis_inf_droideka").is_some(), "the class IS baked");
    assert_eq!(bf2.engine_base("cis_inf_droideka"), Some("walkerdroid"));
    assert!(!game::roster::is_soldier_chain(&bf2, "cis_inf_droideka"));
    assert!(game::roster::is_droideka_chain(&bf2, "cis_inf_droideka"));
    let two = ClassRoster::build(&bf2, "cis_inf_droideka", 1, None).expect("bf2 droideka");
    assert!(game::roster::is_soldier_chain(&bf2, "rep_inf_ep3_rifleman"));

    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    assert!(game::roster::is_droideka_chain(&bf1, "cis_inf_droideka"));
    let one = ClassRoster::build(&bf1, "cis_inf_droideka", 1, None).expect("bf1 droideka");
    for r in [&two, &one] {
        let c = r.droideka.as_ref().expect("droideka class");
        println!(
            "{} hp {} weapon {} shield {:?} ball r {} speed {} walk {} explosion {}",
            r.class,
            r.max_health,
            r.weapon.name,
            c.shield,
            c.ball_radius,
            c.ball_move_speed,
            c.max_speed,
            c.explosion.is_some()
        );
        assert_eq!(r.max_health, 550.0);
        assert_eq!(c.shield.max, 3000.0);
        assert!(!c.shield.drains_full);
        assert!(c.explosion.is_some());
        assert!(r.chunk.is_none());
        assert!(r.geometry.is_none());
        println!("{} engine {:?}", r.class, r.jet_look);
        assert_eq!(
            r.jet_look.engine.as_deref(),
            Some("cis_walk_droideka_engine_parameterized")
        );
        assert_eq!(r.jet_look.turn_on.as_deref(), Some("cis_droideka_fold"));
        assert_eq!(r.jet_look.turn_off.as_deref(), Some("cis_droideka_unfold"));
    }
    for r in [&two, &one] {
        let c = r.droideka.as_ref().unwrap();
        let stand = c
            .camera_section(core::droideka::Form::Walk, false)
            .expect("STAND");
        let ball = c
            .camera_section(core::droideka::Form::Ball, false)
            .expect("CROUCH");
        println!(
            "{} cameras {} stand {stand:?} ball {ball:?}",
            r.class,
            c.cameras.len()
        );
        assert_eq!(
            (stand.eye_y, stand.offset),
            (1.8, glam::Vec3::new(0.0, 0.5, 4.5))
        );
        assert_eq!(
            (ball.eye_y, ball.offset),
            (1.0, glam::Vec3::new(0.0, 0.3, 4.5))
        );
        assert!((stand.tilt - 5f32.to_radians()).abs() < 1e-6);
    }
    let shield = |r: &ClassRoster| r.droideka.as_ref().unwrap().shield.sound.clone();
    assert_eq!(shield(&one).as_deref(), Some("droid_fx7_hum"));
    assert_eq!(shield(&two), None);
    assert_eq!(
        two.droideka.as_ref().unwrap().ball_move_speed,
        one.droideka.as_ref().unwrap().ball_move_speed
    );
    // BF1's heroes ARE soldier chains — they are excluded because no
    // conquest AddUnitClass list names them, not by this test.
    assert!(game::roster::is_soldier_chain(
        &bf1,
        "all_inf_lukeskywalker"
    ));
    assert_eq!(
        bf1.prop_f32("all_inf_lukeskywalker", "MaxHealth"),
        Some(1_000_000.0)
    );
    for (_, _, list) in game::roster::DEFAULT_CLASSES {
        assert!(!list.contains(&"all_inf_lukeskywalker"));
        assert!(!list.contains(&"imp_inf_darthvader"));
    }
}

/// `jet` is `JetJump > 0`, and exactly two default-list classes have one.
#[test]
fn only_the_jet_pack_classes_are_jet() {
    let Some(dir) = classes_dir() else { return };
    let mut jets = Vec::new();
    for (game, a, b) in [("bf1", "all", "imp"), ("bf2", "rep", "cis")] {
        let ([ra, rb], _) = FactionRoster::pair(&dir, game, [a, b], None).expect("rosters");
        for r in [&ra, &rb] {
            for c in &r.classes {
                if c.jet {
                    jets.push(c.class.clone());
                }
            }
        }
    }
    jets.sort();
    assert_eq!(jets, ["imp_inf_dark_trooper", "rep_inf_ep3_jettrooper"]);
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    assert_eq!(bf1.prop_f32("imp_inf_dark_trooper", "JetJump"), Some(21.0));
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    assert_eq!(bf2.prop_f32("rep_inf_ep3_jettrooper", "JetJump"), Some(5.0));
}

/// Card JETP step 1: each jet class's numbers off its disc (re-jetpack.md
/// §2.2), with BF1's missing keys borrowed from its BF2 twin. The jet
/// troopers fly on the one profile (`roster::JET_TROOPER_FUEL_*`).
#[test]
fn the_jet_classes_carry_their_disc_numbers() {
    let Some(dir) = classes_dir() else { return };
    let bf1 = load_classdb_all(&dir, "bf1");
    let bf2 = load_classdb_all(&dir, "bf2");
    // (game, class, jump, push, accel, hover, cost, initial, recharge, border,
    // energy, jet row)
    type Row = (
        &'static str,
        &'static str,
        [f32; 3],
        bool,
        [f32; 5],
        [f32; 3],
    );
    let rows: [Row; 7] = [
        (
            "bf2",
            "rep_inf_ep3_jettrooper",
            [5.0, 8.0, 30.0],
            true,
            // The disc's 0.16 / 0.12 give way to the jet trooper profile.
            [0.1, 0.25, 0.1, 0.24, 0.0],
            [1.5, 1.25, 1.25],
        ),
        (
            "bf2",
            "rep_inf_ep2_jettrooper_sniper",
            [0.0, 0.0, 0.0],
            true,
            [1.0, 1.0, 0.0, 1.04, 0.0],
            [1.5, 1.25, 1.25],
        ),
        (
            "bf2",
            "imp_inf_dark_trooper",
            [21.0, 0.0, 10.0],
            false,
            [0.0, 0.5, 0.075, 0.49, 0.0],
            [0.3, 0.3, 1.0],
        ),
        (
            "bf2",
            "imp_hero_bobafett",
            [7.0, 8.0, 30.0],
            true,
            [0.15, 0.25, 0.12, 0.24, 0.0],
            [2.0, 1.25, 1.25],
        ),
        (
            "bf2",
            "imp_hero_darthvader",
            [7.0, 7.0, 20.0],
            true,
            [0.3, 0.25, 0.9, 1.0, 30.0],
            [1.5, 1.25, 1.25],
        ),
        // BF1 on BF2's rules: its own numbers, the twin's JetAcceleration
        // and jet row; the jet trooper profile is BF1's net cost 0.2 - 0.1.
        (
            "bf1",
            "rep_inf_jet_trooper",
            [5.0, 8.0, 30.0],
            true,
            [0.1, 0.25, 0.1, 0.24, 0.0],
            [1.5, 1.25, 1.25],
        ),
        (
            "bf1",
            "imp_inf_dark_trooper",
            [21.0, 0.0, 10.0],
            false,
            [0.0, 0.5, 0.025, 0.49, 0.0],
            [0.3, 0.3, 1.0],
        ),
    ];
    for (game, class, [jump, push, accel], hover, fuel, row) in rows {
        let db = if game == "bf1" { &bf1 } else { &bf2 };
        let c = game::roster::soldier_constants(db, class);
        let got = [
            c.jet_jump,
            c.jet_push,
            c.jet_acceleration,
            c.jet_fuel_cost,
            c.jet_fuel_initial,
            c.jet_fuel_recharge,
            c.jet_fuel_min_border,
            c.jet_energy_drain,
        ];
        let want = [
            jump, push, accel, fuel[0], fuel[1], fuel[2], fuel[3], fuel[4],
        ];
        assert_eq!(got, want, "{game} {class}");
        assert_eq!(c.jet_hover, hover, "{game} {class} hover");
        assert_eq!(c.jet_control, row, "{game} {class} jet row");
        assert_eq!(c.saber, class.contains("vader"), "{game} {class} sabre");
    }
    // The snow variant keeps its own fuel numbers.
    let snow = game::roster::soldier_constants(&bf1, "imp_inf_dark_troopersnow");
    assert_eq!(
        (snow.jet_fuel_initial, snow.jet_fuel_min_border),
        (0.4, 0.4)
    );
    assert_eq!(snow.jet_acceleration, 10.0);
    // A rifleman has no jet; a Jedi has the sabre and no hover.
    let rifle = game::roster::soldier_constants(&bf2, "rep_inf_ep3_rifleman");
    assert!(!rifle.has_jet() && !rifle.saber);
    let obi = game::roster::soldier_constants(&bf2, "rep_hero_obiwan");
    assert!(obi.saber && !obi.jet_hover && !obi.has_jet());
    assert_eq!((obi.jet_jump, obi.jet_energy_drain), (10.0, 40.0));
    let vader = game::roster::soldier_constants(&bf2, "imp_hero_darthvader");
    assert!(vader.saber && vader.jet_hover);
}

/// Playtest 2026-11-05 B4: a BF1 hover class pays its disc JetFuelCost less
/// its JetFuelRechargeRate in flight (BF1 refills in the hover; BF2's rule
/// does not), so the jet trooper flies 7.5 s from full and 2.5 s from the
/// spawn's half tank, as on BF1. Leap classes keep the disc cost; every jet
/// trooper flies on that same profile (`every_jet_trooper_flies_as_long_as_bf1s`).
#[test]
fn bf1_hover_classes_keep_bf1s_net_jet_drain() {
    let Some(dir) = classes_dir() else { return };
    let mut bf1 = load_classdb_all(&dir, "bf1");
    let bf2 = load_classdb_all(&dir, "bf2");
    let natives = game::assets::baked_root().join("classes");
    match game::native_teams::native_side(&natives, "bf1", "geo_inf_geonosian") {
        Some((_, file)) => {
            bf1.load(&file).expect("native classes");
        }
        None => eprintln!("SKIP geo_inf_geonosian: no baked native classes"),
    }
    for class in ["rep_inf_jet_trooper", "geo_inf_geonosian"] {
        if bf1.get(class).is_none() && class.starts_with("geo_") {
            continue;
        }
        let c = game::roster::soldier_constants(&bf1, class);
        let cost = bf1.prop_f32(class, "JetFuelCost").expect("cost");
        let recharge = bf1
            .prop_f32(class, "JetFuelRechargeRate")
            .expect("recharge");
        assert!(c.jet_hover, "{class}");
        assert!(
            (c.jet_fuel_cost - (cost - recharge)).abs() < 1e-6,
            "{class}"
        );
    }
    let c = game::roster::soldier_constants(&bf1, "rep_inf_jet_trooper");
    let full = (1.0 - c.jet_fuel_initial) / c.jet_fuel_cost;
    let spawn = (0.5 - c.jet_fuel_initial) / c.jet_fuel_cost;
    assert!((full - 7.5).abs() < 1e-3 && (spawn - 2.5).abs() < 1e-3);
    for class in ["imp_inf_dark_trooper", "imp_inf_dark_troopersnow"] {
        assert_eq!(
            game::roster::soldier_constants(&bf1, class).jet_fuel_cost,
            0.0
        );
    }
    let ep3 = game::roster::soldier_constants(&bf2, "rep_inf_ep3_jettrooper");
    assert_eq!(ep3.jet_fuel_cost, game::roster::JET_TROOPER_FUEL_COST);
}

/// The maintainer 2026-11-10 ("one game"): every jet trooper class on every
/// map flies BF1's 7.5 s from full and 2.5 s from spawn on one fuel profile.
/// The disc's lift-off cost and border already agree across the games; the
/// profile replaces only the drain and the refill. The other hover and leap
/// classes keep their own numbers.
#[test]
fn every_jet_trooper_flies_as_long_as_bf1s() {
    let Some(dir) = classes_dir() else { return };
    let mut bf1 = load_classdb_all(&dir, "bf1");
    let bf2 = load_classdb_all(&dir, "bf2");
    for key in ["JetFuelInitialCost", "JetFuelMinBorder"] {
        assert_eq!(
            bf1.prop_f32("rep_inf_jet_trooper", key),
            bf2.prop_f32("rep_inf_default_jettrooper", key),
            "{key}"
        );
    }
    let troopers = [
        (&bf1, "rep_inf_jet_trooper"),
        (&bf2, "rep_inf_ep2_jettrooper"),
        (&bf2, "rep_inf_ep3_jettrooper"),
        (&bf2, "rep_inf_ep2_jettrooper_training"),
        (&bf2, "rep_inf_ep2_jettrooper_rifleman"),
    ];
    let fuel = |c: &core::soldier::SoldierConstants| {
        [
            c.jet_fuel_cost,
            c.jet_fuel_initial,
            c.jet_fuel_recharge,
            c.jet_fuel_min_border,
        ]
    };
    let first = fuel(&game::roster::soldier_constants(
        &bf1,
        "rep_inf_jet_trooper",
    ));
    assert_eq!(
        first,
        [
            game::roster::JET_TROOPER_FUEL_COST,
            0.25,
            game::roster::JET_TROOPER_FUEL_RECHARGE,
            0.24
        ]
    );
    for (db, class) in troopers {
        let c = game::roster::soldier_constants(db, class);
        assert!(game::roster::is_jet_trooper(db, class, &c), "{class}");
        assert_eq!(fuel(&c), first, "{class}");
        let full = (1.0 - c.jet_fuel_initial) / c.jet_fuel_cost;
        let spawn = (core::soldier::Soldier::spawn_fuel(&c) - c.jet_fuel_initial) / c.jet_fuel_cost;
        assert!((full - 7.5).abs() < 1e-3, "{class} full {full}");
        assert!((spawn - 2.5).abs() < 1e-3, "{class} spawn {spawn}");
    }
    // The sniper never jets.
    let sniper = game::roster::soldier_constants(&bf2, "rep_inf_ep2_jettrooper_sniper");
    assert!(!game::roster::is_jet_trooper(
        &bf2,
        "rep_inf_ep2_jettrooper_sniper",
        &sniper
    ));
    assert_eq!(
        (sniper.jet_fuel_cost, sniper.jet_fuel_min_border),
        (1.0, 1.04)
    );
    // Heroes and leapers keep their own.
    for (class, cost, recharge) in [
        ("cis_hero_jangofett", 0.15, 0.12),
        ("imp_hero_bobafett", 0.15, 0.12),
        ("imp_hero_darthvader", 0.3, 0.9),
        ("imp_hero_emperor", 0.3, 0.9),
        ("imp_inf_dark_trooper", 0.0, 0.075),
    ] {
        let c = game::roster::soldier_constants(&bf2, class);
        assert!(!game::roster::is_jet_trooper(&bf2, class, &c), "{class}");
        assert_eq!(
            (c.jet_fuel_cost, c.jet_fuel_recharge),
            (cost, recharge),
            "{class}"
        );
    }
    let dark = game::roster::soldier_constants(&bf1, "imp_inf_dark_trooper");
    assert_eq!((dark.jet_fuel_cost, dark.jet_fuel_recharge), (0.0, 0.025));
    let natives = game::assets::baked_root().join("classes");
    match game::native_teams::native_side(&natives, "bf1", "geo_inf_geonosian") {
        Some((_, file)) => {
            bf1.load(&file).expect("native classes");
            let geo = game::roster::soldier_constants(&bf1, "geo_inf_geonosian");
            assert!(!game::roster::is_jet_trooper(
                &bf1,
                "geo_inf_geonosian",
                &geo
            ));
            assert!((geo.jet_fuel_cost - (0.1 - 0.03)).abs() < 1e-6);
            assert_eq!(geo.jet_fuel_recharge, 0.03);
        }
        None => eprintln!("SKIP geo_inf_geonosian: no baked native classes"),
    }
}

/// The borrowed BF1 numbers are BF2's twins' own (re-jetpack.md §12).
#[test]
fn bf1_jet_borrowing_matches_the_bf2_twins() {
    let Some(dir) = classes_dir() else { return };
    let natives = game::assets::baked_root().join("classes");
    let [bf1, bf2] = ["bf1", "bf2"].map(|g| {
        let mut db = load_classdb_all(&dir, g);
        let geo = natives.join(format!("{g}_geo.ron"));
        if geo.is_file() {
            db.load(&geo).expect("native classes");
        }
        db
    });
    for &(class, twin, accel, row) in game::roster::BF1_JET_BORROW {
        if bf1.get(class).is_none() && class.starts_with("geo_") {
            eprintln!("SKIP {class}: no baked native classes");
            continue;
        }
        assert!(bf1.get(class).is_some(), "BF1 has {class}");
        assert_eq!(bf1.prop(class, "JetAcceleration"), None, "{class}");
        let t = game::roster::soldier_constants(&bf2, twin);
        assert_eq!(t.jet_acceleration, accel, "{twin}");
        assert_eq!(t.jet_control, row.unwrap_or([0.3, 0.3, 1.0]), "{twin}");
    }
}

/// Card RKEX: the rocket launchers' ordnance carries its impact explosion
/// from the disc - BF2's by `ExplosionName` (Damage 400, DamageRadiusInner
/// 2, Outer 4, VehicleScale 0), BF1's (a `bullet` ordnance) by
/// `ExplosionImpact` (300, 0.5, 2.5) - and a rifle's none.
#[test]
fn the_rocket_launchers_carry_their_impact_explosion() {
    let Some(dir) = classes_dir() else { return };
    let ex = |game: &str, weapon: &str| {
        let db = game::roster::load_classdb(&dir, game, &FOUR).expect("db");
        game::roster::weapon_from_db(&db, weapon)
            .expect(weapon)
            .ordnance
            .explosion
            .map(|e| (e.damage, e.damage_inner, e.damage_outer, e.scales[3]))
    };
    assert_eq!(
        ex("bf2", "rep_weap_inf_rocket_launcher"),
        Some((400.0, 2.0, 4.0, 0.0))
    );
    assert_eq!(
        ex("bf1", "rep_weap_inf_rocket_launcher"),
        Some((300.0, 0.5, 2.5, 0.0))
    );
    assert_eq!(ex("bf2", "rep_weap_inf_rifle"), None);
    assert_eq!(ex("bf1", "rep_weap_inf_rifle"), None);
}

/// The BF2 GCW case, which used to be the reason the fallback rule existed:
/// tat2's mission names BF2 classes, and against the db its own disc supplies
/// every one of them resolves — no skip, no fallback.
#[test]
fn a_bf2_gcw_mission_resolves_against_its_own_disc() {
    let Some(dir) = classes_dir() else { return };
    let Some(m) = mission("tat2_con.ron") else {
        return;
    };
    assert_eq!(m.source_game, "bf2");
    assert_eq!(m.factions(), ["imp".to_string(), "all".to_string()]);
    let bf2 = game::roster::load_classdb(&dir, "bf2", &["imp", "all"]).expect("bf2 db");

    for (t, faction) in [(0usize, "imp"), (1, "all")] {
        let mix = &m.teams[t].classes;
        let (r, skipped) = FactionRoster::build(&bf2, "bf2", faction, mix).expect("mission roster");
        for s in &skipped {
            println!("skipped {} — {}", s.class, s.reason);
        }
        assert!(skipped.is_empty(), "{faction}: {} skipped", skipped.len());
        assert_eq!(
            r.fill_class().class,
            mix[0].0,
            "{faction}: the mission's own fill class"
        );
        assert_eq!(r.classes.len(), mix.len());
    }
    // And the classes really are BF2's: `all_inf_rifleman` exists on the BF2
    // disc and on no BF1 one.
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    assert!(bf2.get("all_inf_rifleman").is_some());
    assert!(bf1.get("all_inf_rifleman").is_none(), "not in BF1 ALL.LVL");
}

/// The fallback RULE itself is unchanged and still whole-mix: a mix whose
/// FIRST entry cannot be built is discarded even when a later entry can.
///
/// The db mismatch is now constructed deliberately (tat2's BF2 mix against
/// the BF1 db) rather than being what every BF2 GCW map did: exactly one of
/// the Empire's six BF2 names, `imp_inf_dark_trooper`, also exists on the BF1
/// disc, and keeping that partial mix fielded sixteen shotgun dark troopers.
#[test]
fn a_mission_mix_without_its_fill_class_falls_back_whole() {
    let Some(dir) = classes_dir() else { return };
    let Some(m) = mission("tat2_con.ron") else {
        return;
    };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");

    // The Alliance: not one of its six BF2 classes is in the BF1 db.
    assert_eq!(m.teams[1].classes[0].0, "all_inf_rifleman", "a BF2 class");
    let (r, skipped) =
        FactionRoster::build(&bf1, "bf1", "all", &m.teams[1].classes).expect("fallback roster");
    assert_eq!(
        skipped.len(),
        m.teams[1].classes.len(),
        "every class skipped"
    );
    assert_eq!(r.fill_class().class, "all_inf_soldier", "the default list");
    assert_eq!(r.classes.len(), 5);

    // The Empire: the fill class is missing but `imp_inf_dark_trooper` is
    // not, and the partial mix is discarded anyway.
    let imp = &m.teams[0].classes;
    assert_eq!(imp[0].0, "imp_inf_rifleman");
    assert!(bf1.get("imp_inf_rifleman").is_none());
    assert!(
        imp.iter().any(|c| bf1.get(&c.0).is_some()),
        "one BF2 name collides with a BF1 class"
    );
    let (r, skipped) = FactionRoster::build(&bf1, "bf1", "imp", imp).expect("fallback roster");
    assert_eq!(r.fill_class().class, "imp_inf_storm_trooper", "the default");
    assert_eq!(r.classes.len(), 5);
    assert!(skipped.len() < imp.len(), "not every class failed to build");

    // A mix that KEEPS its fill class stays the mission's, short or not.
    let Some(t1) = mission("tat1_con.ron") else {
        return;
    };
    let (r, _) = FactionRoster::build(&bf1, "bf1", "all", &t1.teams[1].classes).expect("tat1 all");
    assert_eq!(r.fill_class().class, "all_inf_soldierdesert");
}

/// `pair` RETURNS its warnings instead of printing them, so the sweep can put
/// them in the map's own ordered block (its workers interleave stderr).
#[test]
fn pair_returns_its_warnings_rather_than_printing_them() {
    let Some(dir) = classes_dir() else { return };
    let Some(t2) = mission("tat2_con.ron") else {
        return;
    };

    // The mission on its own map: nothing to say.
    let (_, quiet) =
        FactionRoster::pair(&dir, "bf2", ["imp", "all"], Some(&t2)).expect("tat2 rosters");
    assert!(quiet.is_empty(), "{quiet:#?}");

    // The same mission against the wrong disc: every class skipped and both
    // sides falling back, all of it handed back to the caller.
    let (_, loud) =
        FactionRoster::pair(&dir, "bf1", ["imp", "all"], Some(&t2)).expect("mismatched rosters");
    assert_eq!(
        loud.iter().filter(|w| w.contains("falling back")).count(),
        2,
        "{loud:#?}"
    );
    assert!(loud.len() > 2);
    for w in &loud {
        assert!(w.starts_with("roster "), "{w}");
    }
}

/// The item's referee: EVERY shipped mission RON's class mix resolves against
/// the db its own map's source game supplies. No mission falls back and no
/// class is skipped.
#[test]
fn every_mission_mix_resolves_against_its_maps_own_disc() {
    let Some(dir) = classes_dir() else { return };
    let Some(files) = mission_files() else {
        return;
    };
    assert!(files.len() >= 32, "expected a mission per baked map");

    let mut dbs: Vec<(String, game::classdb::ClassDb)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut fell_back: Vec<String> = Vec::new();
    let mut not_soldier: Vec<String> = Vec::new();
    for path in &files {
        let m = MissionDef::load(path).expect("mission");
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let factions = m.factions();
        if m.is_hero_assault() {
            continue;
        }
        let key = format!("{}:{}+{}", m.source_game, factions[0], factions[1]);
        if !dbs.iter().any(|(k, _)| *k == key) {
            let db = game::roster::load_classdb(
                &dir,
                &m.source_game,
                &[factions[0].as_str(), factions[1].as_str()],
            )
            .expect("class db");
            dbs.push((key.clone(), db));
        }
        let db = &dbs.iter().find(|(k, _)| *k == key).unwrap().1;
        for (t, faction) in factions.iter().enumerate() {
            let mix = &m.teams[t].classes;
            let (r, skipped) = FactionRoster::build(db, &m.source_game, faction, mix)
                .unwrap_or_else(|e| panic!("{stem} {faction}: {e:#}"));
            for c in &r.classes {
                println!(
                    "ROSTER {stem} {faction} {} primary={} ammo={} held={:?}",
                    c.class,
                    c.weapon_slot,
                    c.total_clips,
                    c.held.iter().map(|h| h.slot).collect::<Vec<_>>()
                );
                for slot in &c.slots {
                    println!(
                        "ROSTER {stem} {faction} {} slot={} channel={} name={} ammo={:?} bank={}",
                        c.class,
                        slot.slot,
                        slot.channel,
                        slot.weapon,
                        slot.ammo,
                        c.anim.bank_of(&slot.weapon)
                    );
                }
            }
            for s in &skipped {
                if s.reason.contains("not in the baked class db") {
                    missing.push(format!("{stem} {faction} {}", s.class));
                } else {
                    not_soldier.push(format!("{stem} {faction} {}", s.class));
                }
            }
            if !mix.is_empty() && r.fill_class().class != mix[0].0.to_ascii_lowercase() {
                fell_back.push(format!("{stem} {faction} {}", mix[0].0));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "classes not in their map's db: {missing:#?}"
    );
    assert!(
        fell_back.is_empty(),
        "missions still falling back: {fell_back:#?}"
    );
    assert!(not_soldier.is_empty(), "{not_soldier:#?}");
    println!("{} missions, 0 missing, 0 fallbacks, 0 skips", files.len());
}

/// The soldier MODEL follows the class db onto the map's own disc. A class
/// fielded on a `game` map carries the `GeometryName` ITS disc spells, so
/// `SoldierVisuals::load` has to look in `side_<map game>/soldiers/` and not
/// in the side bake of the faction's native disc.
///
/// Data-gated twice: on the extracted side LVLs (the class db) and on the
/// shipped bake (the models).
#[test]
fn every_mission_class_model_is_in_its_maps_own_side_bake() {
    let Some(dir) = classes_dir() else { return };
    let baked = game::assets::baked_root();
    if !baked.join("side_bf1/soldiers").is_dir() || !baked.join("side_bf2/soldiers").is_dir() {
        eprintln!("SKIP: no side bakes under {}", baked.display());
        return;
    }
    let native = |faction: &str| game::roster::faction_game(faction).expect("known faction");
    let has = |g: &str, geom: &str| baked.join(format!("side_{g}/soldiers/{geom}.glb")).exists();

    let Some(files) = mission_files() else {
        return;
    };
    let mut dbs: Vec<(String, game::classdb::ClassDb)> = Vec::new();
    let (mut slots, mut on_map, mut on_native) = (0usize, 0usize, 0usize);
    let mut missing: Vec<String> = Vec::new();
    for path in &files {
        let m = MissionDef::load(path).expect("mission");
        let g = m.source_game.clone();
        if !dbs.iter().any(|(k, _)| *k == g) {
            dbs.push((g.clone(), load_classdb_all(&dir, &g)));
        }
        let db = &dbs.iter().find(|(k, _)| *k == g).unwrap().1;
        for (t, faction) in m.factions().iter().enumerate() {
            if native(faction) == g {
                continue; // the two agree; nothing to tell apart
            }
            for (class, _, _) in &m.teams[t].classes {
                let Some(geom) = db.prop(class, "GeometryName") else {
                    continue;
                };
                let geom = geom.trim();
                slots += 1;
                on_map += usize::from(has(&g, geom));
                on_native += usize::from(has(native(faction), geom));
                if !has(&g, geom) {
                    missing.push(geom.to_owned());
                }
            }
        }
    }
    println!("{slots} class slots on a non-native (map, faction) pair: {on_map} models in the map's own side bake, {on_native} in the faction's native one");
    assert!(slots > 0, "no cross-era mission in the corpus?");
    assert!(
        on_map > on_native,
        "the map's own side bake must resolve more models, not fewer"
    );
    // What is left is the droideka, and the droideka is not a fieldable class
    // (`non_soldier_chains_are_rejected`).
    missing.sort();
    missing.dedup();
    assert_eq!(missing, ["cis_walk_droideka"], "unbaked soldier models");
}

/// Card UWPN: in the battle every class holds its OWN primary's model. The
/// battle's draw plan (`game::soldier_vis::DrawPlan`) over every shipped
/// mission's rosters draws each class with the roster's `weapon_geometry`,
/// while classes still share one body upload. The count printed is what the
/// old per-body cache got wrong — the first class with a body lent its gun
/// to every later class with that body (UAUD1 / UAUD2's MISMATCH lines).
#[test]
fn battle_classes_hold_their_own_weapon_model() {
    let Some(dir) = classes_dir() else { return };
    let Some(files) = mission_files() else {
        return;
    };
    // Per game: (classes, drawn with another gun now, with another gun under
    // the per-body cache).
    let mut tally = [("bf1", 0usize, 0usize, 0usize), ("bf2", 0, 0, 0)];
    let mut wrong: Vec<String> = Vec::new();
    for path in &files {
        let m = MissionDef::load(path).expect("mission");
        let f = m.factions();
        let (rosters, _) = FactionRoster::pair(
            &dir,
            &m.source_game,
            [f[0].as_str(), f[1].as_str()],
            Some(&m),
        )
        .expect("rosters");
        // The battle's own templates (`main/battle.rs`).
        let teams = [&rosters[0], &rosters[1]].map(|r| {
            let mut t = game::walk::TeamTemplate::default_trooper();
            t.classes = r
                .classes
                .iter()
                .map(game::walk::ClassVisual::from_roster)
                .collect();
            t
        });
        let plan = game::soldier_vis::DrawPlan::of(&teams);
        let row = tally
            .iter_mut()
            .find(|t| t.0 == m.source_game)
            .expect("bf1 or bf2");
        // The per-body cache was one for both teams: the first class with a
        // body, in team order, set the gun.
        let mut first_by_body: Vec<(&str, Option<&str>)> = Vec::new();
        for (t, r) in rosters.iter().enumerate() {
            for (k, c) in r.classes.iter().enumerate() {
                row.1 += 1;
                let own = c.weapon_geometry.as_deref();
                if plan.weapon_of(t, k) != own {
                    row.2 += 1;
                    wrong.push(format!(
                        "{} {}: draws {:?}, its primary's model is {own:?}",
                        path.display(),
                        c.class,
                        plan.weapon_of(t, k)
                    ));
                }
                let Some(body) = c.geometry.as_deref() else {
                    continue;
                };
                match first_by_body.iter().find(|(b, _)| *b == body) {
                    Some((_, w)) => row.3 += usize::from(*w != own),
                    None => first_by_body.push((body, own)),
                }
            }
        }
    }
    for (game, classes, now, before) in tally {
        println!(
            "{game}: {classes} class slots, another class's gun: {now} (per-body cache: {before})"
        );
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    // The rosters do hold the case: classes sharing a body, not a gun.
    assert!(tally.iter().all(|t| t.3 > 0), "{tally:?}");
}

/// Card WHELD acceptance 1-2: every class slot of every class a shipped
/// mission or a default list fields, per disc, with the model its weapon
/// is drawn as in the hand (`roster::weapon_model`) and whether the map's
/// own side bake holds it. Every slot the sim can put in a soldier's hand
/// (the primary and `held_slots`) must have its model baked: a slot whose
/// weapon names no model draws none, as on the PS2. The table is printed
/// (`-- --nocapture`); its rows are in the card.
#[test]
fn every_held_slots_hand_model_is_baked() {
    let Some(dir) = classes_dir() else { return };
    let baked = game::assets::baked_root();
    if !baked.join("side_bf1/weapons").is_dir() || !baked.join("side_bf2/weapons").is_dir() {
        eprintln!("SKIP: no side bakes under {}", baked.display());
        return;
    }
    // A side bake from before card WHELD's `rebake.sh side` holds none of
    // the models it added (land.py's branch ci runs on that bake before its
    // rebake): skip. A bake holding some of them and not others fails below.
    let added = [
        "side_bf1/weapons/all_weap_inf_pistol.glb",
        "side_bf1/weapons/imp_weap_inf_pistol.glb",
        "side_bf1/weapons/rep_weap_inf_pistol.glb",
        "side_bf1/weapons/cis_weap_inf_pistol.glb",
        "side_bf1/weapons/com_weap_inf_grenadethermal.glb",
        "side_bf2/weapons/com_weap_inf_grenadethermal.glb",
    ];
    if added.iter().all(|f| !baked.join(f).exists()) {
        eprintln!("SKIP: the side bake predates card WHELD (run scripts/rebake.sh side)");
        return;
    }
    let Some(files) = mission_files() else {
        return;
    };
    let mut fielded: Vec<(String, String)> = Vec::new();
    let mut add = |game: &str, class: &str| {
        let k = (game.to_owned(), class.to_ascii_lowercase());
        if !fielded.contains(&k) {
            fielded.push(k);
        }
    };
    for game in ["bf1", "bf2"] {
        for f in FOUR {
            for class in game::roster::default_classes(game, f).into_iter().flatten() {
                add(game, class);
            }
        }
    }
    for path in &files {
        let m = MissionDef::load(path).expect("mission");
        let f = m.factions();
        let (rosters, _) = FactionRoster::pair(
            &dir,
            &m.source_game,
            [f[0].as_str(), f[1].as_str()],
            Some(&m),
        )
        .expect("rosters");
        for r in &rosters {
            for c in &r.classes {
                add(&m.source_game, &c.class);
            }
        }
    }
    fielded.sort();
    let dbs = [
        ("bf1", load_classdb_all(&dir, "bf1")),
        ("bf2", load_classdb_all(&dir, "bf2")),
    ];
    let (mut drawn, mut none, mut missing) = (0usize, 0usize, Vec::new());
    for (game, class) in &fielded {
        let db = &dbs.iter().find(|(g, _)| g == game).unwrap().1;
        let Ok(c) = game::roster::ClassRoster::build(db, class, 0, None) else {
            continue;
        };
        let run: Vec<u8> = std::iter::once(c.weapon_slot)
            .chain(c.held.iter().map(|h| h.slot))
            .collect();
        for s in db.weapon_slots(class) {
            let name = db.resolve(&s.weapon).unwrap_or(&s.weapon).to_owned();
            let model = game::roster::weapon_model(db, &name);
            let has = model
                .as_deref()
                .map(|g| baked.join(format!("side_{game}/weapons/{g}.glb")).exists());
            let runs = run.contains(&s.slot);
            println!(
                "WHELD {game} {class:<26} slot {} ch{} {:<34} model {:<30} baked {:<5} sim {}",
                s.slot + 1,
                s.channel,
                name,
                model.as_deref().unwrap_or("-"),
                has.map_or("-", |b| if b { "yes" } else { "NO" }),
                if runs { "yes" } else { "no" },
            );
            if !runs {
                continue;
            }
            match (model, has) {
                (Some(_), Some(true)) => drawn += 1,
                (None, _) => none += 1,
                (Some(g), _) => missing.push(format!("{game} {class} slot {}: {g}", s.slot + 1)),
            }
        }
    }
    println!(
        "WHELD: {} classes; slots the sim runs: {drawn} with a baked model, {none} with none, {} unbaked",
        fielded.len(),
        missing.len()
    );
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "unbaked hand models: {missing:#?}");
}

/// Card ZOOM acceptance 1, data-gated: the press sequence the PS2 gave live
/// (`analysis/re-zoom.md` §3, §9), from the BAKED classes through the sim -
/// a rifle on, off; a sniper rifle 2, 8, off in the scope view; BF1's
/// Empire one 6, off; a shotgun 2; a thermal detonator in hand: no zoom.
#[test]
fn the_baked_weapons_zoom_as_the_ps2s_do() {
    use core::sim::{buttons, PadState, Sim};
    let Some(dir) = classes_dir() else { return };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    let rifle: &[Option<f32>] = &[Some(2.5), None, Some(2.5), None];
    let sniper: &[Option<f32>] = &[Some(2.0), Some(8.0), None, Some(2.0)];
    let six: &[Option<f32>] = &[Some(6.0), None, Some(6.0), None];
    let two: &[Option<f32>] = &[Some(2.0), None, Some(2.0), None];
    let none: &[Option<f32>] = &[None; 4];
    for (game, db, weapon, want, scope) in [
        ("bf1", &bf1, "all_weap_inf_rifle", rifle, false),
        ("bf1", &bf1, "all_weap_inf_sniperrifle", sniper, true),
        ("bf1", &bf1, "imp_weap_inf_sniperrifle", six, true),
        ("bf1", &bf1, "all_weap_inf_shotgun", two, false),
        ("bf1", &bf1, "all_weap_inf_thermaldetonator", none, false),
        ("bf2", &bf2, "rep_weap_inf_rifle", rifle, false),
        ("bf2", &bf2, "rep_weap_inf_sniper_rifle", sniper, true),
        ("bf2", &bf2, "rep_weap_inf_thermaldetonator", none, false),
    ] {
        let w = game::roster::weapon_from_db(db, weapon).expect(weapon);
        let mut s = Sim::empty(1);
        s.ground_rules = core::soldier::GroundRules::for_game(game);
        let class = s.add_weapon_class(w.clone());
        s.spawn(&core::unit::UnitSpec {
            weapon_class: class,
            ..core::unit::UnitSpec::player(0, glam::Vec3::ZERO)
        })
        .unwrap();
        let mut seq = Vec::new();
        for _ in 0..4 {
            for buttons in [buttons::ZOOM, 0] {
                s.step(&[PadState {
                    buttons,
                    ..Default::default()
                }]);
            }
            let z = s.units[0].zoom;
            assert!(z.is_none_or(|z| z.divisor == z.value && z.scope == scope));
            seq.push(z.map(|z| z.value));
        }
        println!(
            "{game} {weapon}: ZoomMin {} ZoomMax {} ZoomRate {} first person {} -> {seq:?}",
            w.zoom_min, w.zoom_max, w.zoom_rate, w.zoom_first_person
        );
        assert_eq!(seq, want, "{game} {weapon}");
        assert_eq!(w.zoom_first_person, scope, "{game} {weapon}");
    }
}

/// Phase C D9 (brief LEAD ADDENDUM 4b (iv)), data-gated: the four sniper
/// primaries on the discs hit the AI distance table through the real ODF
/// chains, and the two rifle controls do not — the byte-verified rows of
/// `analysis/re-ai-sniper.md` §3, reproduced by the port's own loader.
#[test]
fn sniper_primaries_hit_the_ai_distance_table() {
    let Some(dir) = classes_dir() else { return };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    // `imp_weap_inf_sniperrifle` ships ZoomMax exactly 6.0: it is in the
    // table only because the PS2 test is `6.0 <= zoom`.
    for (db, weapon, zoom) in [
        (&bf1, "all_weap_inf_sniperrifle", 8.0),
        (&bf1, "imp_weap_inf_sniperrifle", 6.0),
        (&bf2, "rep_weap_inf_sniper_rifle", 8.0),
        (&bf2, "cis_weap_inf_sniper_rifle", 8.0),
    ] {
        let w = game::roster::weapon_from_db(db, weapon).expect(weapon);
        assert_eq!(w.zoom_max, zoom, "{weapon} zoom_max");
        assert_eq!(w.yaw_spread, 0.0, "{weapon} yaw_spread");
        assert!(w.trigger_single, "{weapon} trigger_single");
        assert!(w.ai_distance_table(), "{weapon} should use the table");
    }
    for (db, weapon) in [(&bf1, "all_weap_inf_rifle"), (&bf2, "rep_weap_inf_rifle")] {
        let w = game::roster::weapon_from_db(db, weapon).expect(weapon);
        assert_eq!(w.zoom_max, 2.5, "{weapon} zoom_max");
        assert!(
            (w.yaw_spread - 0.25f32.to_radians()).abs() < 1e-7,
            "{weapon} yaw_spread {}",
            w.yaw_spread
        );
        assert!(!w.trigger_single, "{weapon} trigger_single");
        assert!(!w.ai_distance_table(), "{weapon} must not use the table");
    }
}

/// The unit gallery (card UGAL) shows the gun the GAME draws for a body —
/// its class's primary weapon geometry — not the side's rifle: the super
/// battle droid fires from its wrists and holds nothing in either game, the
/// wookiee holds its bowcaster, the BF2 battle droid body stands for the
/// rocketeer (first in the CIS default mix to wear it).
#[test]
fn gallery_units_hold_their_classs_own_weapon() {
    let Some(dir) = classes_dir() else { return };
    let bf1 = load_classdb_all(&dir, "bf1");
    let bf2 = load_classdb_all(&dir, "bf2");
    let held = |db: &game::classdb::ClassDb, game: &str, model: &str| {
        let class = game::gallery::model_class(db, game, model).expect("a class wears it");
        let weapon = game::roster::weapon_geometry(db, &class);
        (class, weapon)
    };
    assert_eq!(
        held(&bf2, "bf2", "cis_inf_sbdroid"),
        ("cis_inf_rifleman".to_owned(), None)
    );
    assert_eq!(
        held(&bf1, "bf1", "cis_inf_sbdroid"),
        ("cis_inf_battledroid".to_owned(), None)
    );
    assert_eq!(
        held(&bf2, "bf2", "cis_inf_bdroid"),
        (
            "cis_inf_rocketeer".to_owned(),
            Some("cis_weap_inf_launcher".to_owned())
        )
    );
    assert_eq!(
        held(&bf1, "bf1", "all_inf_wookie"),
        (
            "all_inf_smuggler".to_owned(),
            Some("all_weap_inf_bowcaster".to_owned())
        )
    );
    assert_eq!(
        held(&bf2, "bf2", "imp_inf_stormtrooper"),
        (
            "imp_inf_rifleman".to_owned(),
            Some("imp_weap_inf_rifle".to_owned())
        )
    );
}

/// Card FXB: the laser keys through the real chains reproduce re-bolt §2's
/// census rows — the BF2 Republic rifle inherits `com_`'s red End colours,
/// the pistol's later GlowLength / BlurLength beat LaserLength's reset, the
/// snipers fade out over 1 s — and the key hashes are the loader's fnv.
#[test]
fn laser_looks_are_the_census_rows() {
    use core::laser::*;
    let h = game::classdb::fnv1a_lowercase;
    for (name, hash) in [
        ("LaserWidth", HASH_LASER_WIDTH),
        ("LaserLength", HASH_LASER_LENGTH),
        ("LaserTexture", HASH_LASER_TEXTURE),
        ("LaserGlowColor", HASH_LASER_GLOW_COLOR),
        ("EndLaserGlowColor", HASH_END_LASER_GLOW_COLOR),
        ("GlowLength", HASH_GLOW_LENGTH),
        ("BlurLength", HASH_BLUR_LENGTH),
        ("FadeOutTime", HASH_FADE_OUT_TIME),
        ("FadeLightColor", HASH_FADE_LIGHT_COLOR),
        ("LightColor", HASH_LIGHT_COLOR),
        ("LightRadius", HASH_LIGHT_RADIUS),
        ("EndLightColor", HASH_END_LIGHT_COLOR),
    ] {
        assert_eq!(h(name), hash, "{name}");
    }
    let Some(dir) = classes_dir() else { return };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    let look = |db, w| {
        game::roster::weapon_from_db(db, w)
            .expect(w)
            .ordnance
            .laser
            .unwrap_or_else(|| panic!("{w}: no laser look"))
    };
    let l = look(&bf1, "rep_weap_inf_rifle");
    assert_eq!(
        l.texture.as_deref().map(str::to_ascii_lowercase).as_deref(),
        Some("bluelaser_d")
    );
    assert_eq!(
        (l.width, l.length, l.glow_length, l.blur_length),
        (0.1, 1.0, 3.0, 5.0)
    );
    assert_eq!(l.glow_color, [92, 136, 250, 100]);
    assert!(!l.fade_light);
    let l = look(&bf1, "imp_weap_inf_pistol");
    assert_eq!(
        (l.width, l.length, l.glow_length, l.blur_length),
        (0.09, 0.75, 3.0, 5.0)
    );
    let l = look(&bf1, "all_weap_inf_sniperrifle");
    assert_eq!(
        (l.width, l.length, l.glow_length, l.blur_length),
        (0.15, 20.0, 24.0, 32.0)
    );
    assert_eq!(l.fade_out_rate, 1.0);
    let l = look(&bf2, "rep_weap_inf_rifle");
    assert_eq!(l.texture.as_deref(), Some("com_sfx_laser_blue"));
    assert_eq!(l.glow_color, [92, 136, 250, 100]);
    assert_eq!(l.end_glow_color, [160, 43, 43, 100]);
    assert!(l.fade_light);
    assert_eq!((l.fade_start, l.fade_duration), (0.4, 0.2));
    // The light (re-bolt §2, card FXBL): blue, red End inherited from `com_`.
    assert_eq!(l.light(0.0), Some(([36, 53, 98], 4.0)));
    assert_eq!(l.light(1.0), Some(([94, 25, 25], 4.0)));
    let l = look(&bf2, "cis_weap_inf_rifle");
    assert_eq!(l.texture.as_deref(), Some("com_sfx_laser_red"));
    assert_eq!(l.glow_color, [253, 43, 43, 100]);
    assert_eq!(l.end_glow_color, [160, 43, 43, 100]);
    assert_eq!(l.light(0.0), Some(([148, 25, 25], 4.0)));
}

/// Card FXMF: the muzzle flash through the real chains reproduces
/// re-muzzle-flash §2's census — BF1's Republic rifle violet, BF2's
/// `com_weap_inf_rifle` pink inherited by the side rifles, the `med` preset
/// on the vehicle guns, the per-game model — and, where the RE's census
/// files are on disk (`lead_tmp/fxmf/census_bf{1,2}.txt`), every census row
/// of a class the db holds.
#[test]
fn flash_looks_are_the_census_rows() {
    use core::flash::*;
    let h = game::classdb::fnv1a_lowercase;
    for (name, hash) in [
        ("MuzzleFlash", HASH_MUZZLE_FLASH),
        ("FlashRadius", HASH_FLASH_RADIUS),
        ("FlashLength", HASH_FLASH_LENGTH),
        ("FlashColor", HASH_FLASH_COLOR),
        ("MuzzleFlashModel", HASH_MUZZLE_FLASH_MODEL),
        ("FirePointName", HASH_FIRE_POINT_NAME),
    ] {
        assert_eq!(h(name), hash, "{name}");
    }
    let Some(dir) = classes_dir() else {
        eprintln!("SKIP: flash census: 0 games tested, 2 skipped (side LVLs absent)");
        return;
    };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    let row = |l: &MuzzleFlashLook| (l.radius, l.length, l.color, l.model.clone());
    let m1 = Some(BF1_MODEL.to_owned());
    let m2 = Some(BF2_MODEL.to_owned());
    for (db, w, want) in [
        (
            &bf1,
            "rep_weap_inf_rifle",
            (0.6, 0.025, [80, 80, 255, 255], m1.clone()),
        ),
        (
            &bf1,
            "all_weap_inf_rifle",
            (0.6, 0.025, [255, 100, 100, 255], m1.clone()),
        ),
        (
            &bf2,
            "com_weap_inf_rifle",
            (0.6, 0.025, [255, 100, 100, 255], m2.clone()),
        ),
        (
            &bf2,
            "rep_weap_inf_rifle",
            (0.6, 0.025, [100, 100, 255, 255], m2.clone()),
        ),
    ] {
        let got = game::roster::flash_look(db, w);
        assert_eq!(row(&got), want, "{w}");
        assert_eq!(got.fire_point, DEFAULT_FIRE_POINT, "{w}");
    }
    // The loader attaches it to a fielded weapon.
    let w = game::roster::weapon_from_db(&bf1, "rep_weap_inf_rifle").unwrap();
    assert_eq!(
        w.flash,
        Some(game::roster::flash_look(&bf1, "rep_weap_inf_rifle"))
    );
    let census = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/lead_tmp/fxmf"
    ));
    let (mut tested, mut skipped) = (0, 0);
    for (game, db) in [("bf1", &bf1), ("bf2", &bf2)] {
        let path = census.join(format!("census_{game}.txt"));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("SKIP: {} not present", path.display());
                skipped += 1;
                continue;
            }
            Err(e) => panic!("read {}: {e}", path.display()),
        };
        let (mut matched, mut draws) = (0, 0);
        for line in text.lines().filter(|l| !l.starts_with("--")) {
            let name = line.split_whitespace().next().unwrap();
            if db.get_kind("wpnc", name).is_none() {
                continue;
            }
            let field = |k: &str| {
                let at = line.find(&format!(" {k}=")).unwrap() + k.len() + 2;
                line[at..].split_whitespace().next().unwrap().to_owned()
            };
            let c0 = line.find(" C=(").unwrap() + 4;
            let c: Vec<u8> = line[c0..line[c0..].find(')').unwrap() + c0]
                .split(", ")
                .map(|v| v.parse().unwrap())
                .collect();
            let l = game::roster::flash_look(db, name);
            assert_eq!(
                l.radius,
                field("R").parse::<f32>().unwrap(),
                "{game} {name}"
            );
            assert_eq!(
                l.length,
                field("L").parse::<f32>().unwrap(),
                "{game} {name}"
            );
            assert_eq!(l.color.to_vec(), c, "{game} {name}");
            assert_eq!(l.draws(), line.contains(" DRAWS "), "{game} {name}");
            assert_eq!(l.fire_point, field("fp"), "{game} {name}");
            matched += 1;
            draws += usize::from(l.draws());
        }
        eprintln!("{game}: {matched} census rows matched, {draws} draw a flash");
        assert!(matched > 20, "{game}: only {matched} census rows in the db");
        tested += 1;
    }
    eprintln!("flash census: {tested} games tested, {skipped} skipped");
}

/// Card FXP3 (`analysis/re-fx.md` §7's census): what each ordnance's hits
/// spawn. Every showcase infantry bolt reads the game's rifle impact on all
/// five surfaces; a vehicle cannon keeps the ctor's terrain default and
/// detonates its explosion's effect; a thrown grenade has no surface slots,
/// only its trail.
#[test]
fn impact_looks_are_the_census_rows() {
    use core::impact::Surface;
    let Some(dir) = classes_dir() else { return };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    let five = [
        Surface::Soldier,
        Surface::Vehicle,
        Surface::Building,
        Surface::Terrain,
        Surface::Shield,
    ];
    let ord = |db: &game::classdb::ClassDb, w: &str| {
        game::roster::weapon_from_db(db, w)
            .expect(w)
            .ordnance
            .impact
            .expect(w)
    };
    for w in [
        "all_weap_inf_rifle",
        "imp_weap_inf_rifle",
        "rep_weap_inf_rifle",
        "cis_weap_inf_pistol",
        "all_weap_inf_sniperrifle",
    ] {
        let l = ord(&bf1, w);
        for s in five {
            assert_eq!(l.effect(s), Some("ord_explosion"), "{w} {s:?}");
        }
        assert_eq!(l.effect(Surface::Water), Some("watersplash_ord"), "{w}");
        assert_eq!((&l.trail, &l.explosion), (&None, &None), "{w}");
    }
    let l = ord(&bf2, "rep_weap_inf_rifle");
    for s in &five[..4] {
        assert_eq!(l.effect(*s), Some("com_sfx_ord_exp"), "{s:?}");
    }
    assert_eq!(l.effect(Surface::Shield), Some("com_sfx_shieldimpact_sm"));

    let l = ord(&bf1, "cis_weap_hover_aat_cannon");
    assert_eq!(l.effect(Surface::Soldier), None);
    assert_eq!(l.effect(Surface::Terrain), Some("dirtspray"), "the ctor's");
    assert_eq!(l.effect(Surface::Water), Some("watersplash_ord_lg"));
    let x = l.explosion.expect("cis_weap_hover_aat_cannon_exp");
    assert_eq!(x.effects, ["com_sfx_weap_redbolt_exp_md"]);
    assert_eq!(x.water_effect.as_deref(), Some("watersplash_md"));

    let l = ord(&bf1, "rep_weap_inf_thermaldetonator");
    assert!(five.iter().all(|s| l.effect(*s).is_none()), "{l:?}");
    assert_eq!(l.trail.as_deref(), Some("com_sfx_weap_grenade_red"));
    // Its fuse's blast is `ExplosionExpire` (0x2e51_081f), the thrown
    // round's own key — card GREN's, not the impact's `ExplosionName`.
    assert_eq!(l.explosion, None);
}

/// One row of the disc census (`tests/data/weapon_slots_bf{1,2}.txt`,
/// written by `lead_tmp/gren/slots.py` straight from the SIDE LVLs, not from
/// the bake): `class [base]: 1:weapon xAMMO chN; 2:…`. `xNone` = no
/// `WeaponAmmo` on the slot.
fn census_rows(text: &str) -> Vec<(String, Vec<game::classdb::WeaponSlot>)> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (head, slots) = l.split_once("]: ").expect("census row");
            let class = head.split_whitespace().next().unwrap().to_ascii_lowercase();
            let slots = slots
                .split("; ")
                .map(|s| {
                    let mut it = s.split_whitespace();
                    let (n, weapon) = it.next().unwrap().split_once(':').unwrap();
                    let ammo = it.next().unwrap().strip_prefix('x').unwrap();
                    let chan = it.next().unwrap().strip_prefix("ch").unwrap();
                    game::classdb::WeaponSlot {
                        slot: n.parse::<u8>().unwrap() - 1,
                        weapon: weapon.to_ascii_lowercase(),
                        ammo: ammo.parse().ok(),
                        channel: chan.parse().unwrap(),
                    }
                })
                .collect();
            (class, slots)
        })
        .collect()
}

/// Card GREN step 1: the slot model against the disc census. Every census
/// class the four-side db holds resolves the disc's slot list (weapon, ammo,
/// channel per slot), and every class a mission RON fields is among them.
#[test]
fn weapon_slots_match_the_disc_census() {
    let Some(dir) = classes_dir() else { return };
    // The census half needs only the discs; the fielded half needs the missions.
    let missions = mission_files();
    let lower = |mut s: Vec<game::classdb::WeaponSlot>| {
        for x in &mut s {
            x.weapon = x.weapon.to_ascii_lowercase();
        }
        s
    };
    let mut fielded = 0;
    for (game, text) in [
        ("bf1", include_str!("../data/weapon_slots_bf1.txt")),
        ("bf2", include_str!("../data/weapon_slots_bf2.txt")),
    ] {
        let db = load_classdb_all(&dir, game);
        let rows = census_rows(text);
        let mut checked = 0;
        for (class, want) in &rows {
            let Some(name) = db.resolve(class) else {
                continue; // a local side's class (gun, gar, ewk, wok, tat …)
            };
            assert_eq!(&lower(db.weapon_slots(name)), want, "{game} {class}");
            checked += 1;
        }
        println!(
            "{game}: {checked} of {} census classes in the db match",
            rows.len()
        );
        assert!(
            checked >= if game == "bf1" { 40 } else { 120 },
            "{game}: {checked}"
        );

        let mut seen: Vec<String> = Vec::new();
        for path in missions.iter().flatten() {
            let m = MissionDef::load(path).expect("mission");
            if m.source_game != game {
                continue;
            }
            for (class, _, _) in m.teams.iter().flat_map(|t| &t.classes) {
                let class = class.to_ascii_lowercase();
                if seen.contains(&class) {
                    continue;
                }
                let want = &rows
                    .iter()
                    .find(|(c, _)| *c == class)
                    .unwrap_or_else(|| panic!("{game} {class}: fielded, not in the census"))
                    .1;
                let name = db
                    .resolve(&class)
                    .unwrap_or_else(|| panic!("{game} {class}: fielded, not in the db"));
                assert_eq!(&lower(db.weapon_slots(name)), want, "{game} {class}");
                seen.push(class);
            }
        }
        seen.sort();
        println!("{game}: {} fielded classes resolve their slots", seen.len());
        // The classes written with numbered keys, where the old whole-list
        // read saw slot 1 only (BF2: the heroes).
        let h1 = game::classdb::fnv1a_lowercase("WeaponName1");
        for (c, _) in rows.iter().filter(|_| game == "bf2") {
            let Some(def) = db.resolve(c).and_then(|n| db.get(n)) else {
                continue;
            };
            if def.props.iter().any(|p| p.0 == h1) {
                let row: Vec<String> = db
                    .weapon_slots(&def.name)
                    .iter()
                    .map(|s| format!("{}:{} x{:?} ch{}", s.slot + 1, s.weapon, s.ammo, s.channel))
                    .collect();
                println!("  numbered: {c}: {}", row.join("; "));
            }
        }
        fielded += seen.len();
    }
    if missions.is_some() {
        assert!(fielded >= 40, "{fielded} fielded classes");
    }
}

/// Card GREN: the thermal detonator out of the baked class db is the PS2's
/// (`analysis/re-grenades.md` §3.1, §4.2, §9 — the RAM rows): the cycle, the
/// throw, the `Stick*` mask, the gravity, the fuse and the explosion; BF1's
/// crack grenade sticks to vehicles and buildings. Each game keeps its own
/// numbers, BF1's among them its 0.3 s wind-up, fire point, rest rule and fuse.
#[test]
fn thrown_weapons_come_out_of_the_class_db_as_the_ps2_holds_them() {
    let Some(dir) = classes_dir() else { return };
    use core::sticky::stick;
    for (game, weapon, delay, windup, fuse, mask, damage) in [
        (
            "bf1",
            "imp_weap_inf_thermaldetonator",
            1.4,
            0.3,
            0.9,
            0x7fff_ff80u32,
            320.0,
        ),
        (
            "bf2",
            "imp_weap_inf_thermaldetonator",
            1.3,
            0.51,
            0.6,
            0x7fff_ffd8,
            450.0,
        ),
    ] {
        let db = load_classdb_all(&dir, game);
        let w = game::roster::thrown_weapon_from_db(&db, weapon)
            .expect("builds")
            .expect("a thrown weapon");
        assert_eq!(w.shot_delay, [delay; 2], "{game} ShotDelay");
        assert_eq!(w.initial_salvo_delay, windup, "{game} InitialSalvoDelay");
        assert!(w.trigger_single, "{game}: the class default");
        assert_eq!((w.rounds_per_clip, w.reload_time), (1, 0.01), "{game}");
        let t = w.thrown.expect("grenade keys");
        assert_eq!((t.shot_elevate, t.hide_on_fire), (0.21, true), "{game}");
        let bf2 = game == "bf2";
        assert_eq!(
            t.hand,
            bf2.then_some(core::weapon::Thrown::BF2_HAND),
            "{game}: BF2 throws from the hand, BF1 from the fire point"
        );
        let o = &w.ordnance;
        assert_eq!((o.velocity, o.life_span), (25.0, fuse), "{game}");
        let s = o.sticky.as_ref().expect("sticky");
        assert_eq!(s.gravity, -12.25, "{game}");
        assert_eq!(s.stick_mask, mask, "{game}: mask {:#x}", s.stick_mask);
        assert_eq!(s.bf2, bf2, "{game}: its own rest rule");
        let x = s.explosion.as_ref().expect("ExplosionExpire");
        assert_eq!(
            (x.damage, x.damage_inner, x.damage_outer),
            (damage, 4.5, 6.5),
            "{game}"
        );
    }
    let db = load_classdb_all(&dir, "bf1");
    let crack = game::roster::thrown_weapon_from_db(&db, "imp_weap_inf_crackgrenade")
        .unwrap()
        .unwrap();
    let mask = crack.ordnance.sticky.unwrap().stick_mask;
    assert_eq!(
        mask & 0x8000_007f,
        stick::VEHICLE | stick::BUILDING | stick::BUILDING_DEAD,
        "the crack grenade's own mask: {mask:#x}"
    );
    // A rifle is not thrown; a dispenser is not this card's.
    for w in ["imp_weap_inf_rifle", "imp_weap_inf_mine_dispenser"] {
        assert!(game::roster::thrown_weapon_from_db(&db, w)
            .unwrap()
            .is_none());
    }
}

/// The maintainer's rule (2026-11-01): every class any BF1 mission fields
/// throws BF1's own grenade by BF1's own numbers, and each row reads BF2's
/// grenade HUD.
#[test]
fn every_class_a_bf1_mission_fields_throws_bf1s_grenade_under_bf2s_hud() {
    let Some(dir) = classes_dir() else { return };
    let db = load_classdb_all(&dir, "bf1");
    let missions = game::assets::baked_root()
        .parent()
        .unwrap()
        .join("missions");
    let Ok(files) = std::fs::read_dir(missions) else {
        return;
    };
    let mut classes = std::collections::BTreeSet::new();
    for file in files {
        let mission = MissionDef::load(&file.unwrap().path()).unwrap();
        if mission.source_game == "bf1" {
            classes.extend(mission.teams.into_iter().flat_map(|t| t.classes));
        }
    }
    assert!(classes.len() > 30, "{classes:?}");
    let mut grenades = 0;
    for (class, ..) in &classes {
        let roster = ClassRoster::build(&db, class, 0, None).unwrap();
        for h in &roster.held {
            let Some(t) = h.weapon.thrown.filter(|_| !h.weapon.dispenses()) else {
                continue;
            };
            let what = format!("{class} {}", h.weapon.name);
            grenades += 1;
            assert_eq!(h.channel, 1, "{what}");
            assert_eq!(t.hand, None, "{what}");
            assert_eq!(h.weapon.initial_salvo_delay, 0.3, "{what}");
            assert!(!h.weapon.offhand_weapon, "{what}");
            assert!(!h.weapon.ordnance.sticky.as_ref().unwrap().bf2, "{what}");
            assert_eq!(h.hud.key, game::roster::BF2_GRENADE_HUD_KEY, "{what}");
            assert!(!h.hud.bf2, "{what}");
        }
    }
    assert!(grenades > 25, "{grenades}");
}

/// Card GREN: what each fielded class holds besides its primary — the
/// channel-0 sidearm and the channel-1 grenades — for four default faction pairs.
#[test]
fn fielded_classes_hold_their_sidearm_and_grenades() {
    let Some(dir) = classes_dir() else { return };
    for (game, f) in [
        ("bf1", ["all", "imp"]),
        ("bf1", ["rep", "cis"]),
        ("bf2", ["all", "imp"]),
        ("bf2", ["rep", "cis"]),
    ] {
        let ([a, b], _) = FactionRoster::pair(&dir, game, f, None).expect("rosters");
        for r in [&a, &b] {
            let mut thrown = 0;
            for c in &r.classes {
                let row: Vec<String> = c
                    .held
                    .iter()
                    .map(|h| {
                        format!(
                            "{}:{} x{} ch{}",
                            h.slot + 1,
                            h.weapon.name,
                            h.clips,
                            h.channel
                        )
                    })
                    .collect();
                println!(
                    "{game} {}: [{}] {}",
                    c.class,
                    c.weapon_slot + 1,
                    row.join("; ")
                );
                for h in &c.held {
                    assert_ne!(h.slot, c.weapon_slot);
                    // Thrown exactly when the data says so (issue #42).
                    let db = game::roster::load_classdb(&dir, game, &f).unwrap();
                    let kdb = match db.donor_classdb() {
                        Some(d) if db.resolve(&h.weapon.name).is_none() => d,
                        _ => &db,
                    };
                    let kind = game::roster::slot_kind(kdb, &h.weapon.name, h.channel);
                    assert_eq!(
                        matches!(
                            kind,
                            game::roster::SlotKind::Thrown
                                | game::roster::SlotKind::Dispenser
                                | game::roster::SlotKind::Remote
                        ),
                        h.weapon.thrown.is_some(),
                        "{}",
                        c.class
                    );
                    assert!(
                        !h.weapon.name.contains("award") || h.weapon.unlock_medal().is_some(),
                        "{}: {}",
                        c.class,
                        h.weapon.name
                    );
                }
                thrown += c
                    .held
                    .iter()
                    .filter(|h| h.weapon.ordnance.sticky.is_some())
                    .count();
            }
            let fill = r.fill_class();
            let has = fill.held.iter().any(|h| h.channel == 1);
            assert!(has, "{game} {}: {}", r.faction, fill.class);
            assert!(thrown >= 2, "{game} {}: {thrown} thrown slots", r.faction);
        }
    }
}

/// A class's drop table as one line: `item prob, ...` with the common
/// prefix of the item names cut.
fn drop_line(db: &game::classdb::ClassDb, class: &str) -> String {
    db.drop_table(class)
        .iter()
        .map(|(item, p)| format!("{} {p}", item.trim_start_matches("com_item_powerup_")))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Card PWUP (`analysis/re-powerups.md` §2.1, §3.1): every soldier class's
/// drop table resolved the PS2's way, counted per table, and every item
/// class's numbers — from the baked classes of both discs.
#[test]
fn drop_tables_are_the_discs() {
    let Some(dir) = classes_dir() else { return };
    use std::collections::BTreeMap;
    let census = |game: &str| -> BTreeMap<String, Vec<String>> {
        let db = load_classdb_all(&dir, game);
        let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut names: Vec<&str> = db.names().collect();
        names.sort_unstable();
        for n in names {
            if game::roster::is_soldier_chain(&db, n) {
                by.entry(drop_line(&db, n)).or_default().push(n.to_owned());
            }
        }
        for (t, c) in &by {
            println!("{game} [{t}] {}: {}", c.len(), c.join(" "));
        }
        by
    };
    let bf1 = census("bf1");
    let a = &bf1["ammo 0.4, health100 0.3, dual 0.1"];
    let b = &bf1["ammo 0.05, health25 0.1, health100 0.1"];
    // Table B: the 11 Imperial classes and two of the Alliance's (§3.1).
    assert_eq!(b.iter().filter(|c| c.starts_with("imp_")).count(), 11);
    assert!(b.contains(&"all_inf_marksman".to_owned()));
    assert!(b.contains(&"all_inf_vanguardsnow".to_owned()));
    assert_eq!(b.len(), 13);
    // Table A: 21 classes, every one ALL / REP / CIS, no Imperial.
    assert_eq!(a.len(), 21, "{a:?}");
    assert!(a.iter().all(|c| !c.starts_with("imp_")));
    let bf2 = census("bf2");
    let regular = &bf2[
        "ammo 0.25, health25 0.15, offense 0.02, defense 0.02, health100 0.08, dual 0.08, energy 0.1"
    ];
    assert!(regular.contains(&"rep_inf_ep3_rifleman".to_owned()));
    assert!(regular.contains(&"com_inf_default".to_owned()));
    let hero = &bf2["ammo 0.05, health100 0.05, dual 0.2, energy 0.1, offense 0.15, defense 0.15"];
    assert!(hero.contains(&"com_jedi_default".to_owned()));
    // Of the disc's 121 / 33 (§3.1), the four sides and INGAME hold these;
    // the rest are the locals' side LVLs.
    assert_eq!((regular.len(), hero.len()), (107, 23));
    // BF1's other tables: the heroes' and the battle droids' (§3.1 C, D).
    assert_eq!(bf1["ammo 0.2, health25 0.05, health100 0"].len(), 5);
    assert_eq!(bf1["ammo 0.07, health25 0.08, health100 0"].len(), 4);

    // The item classes (§2.1): amounts per game, the life span, the boxes.
    for (game, want) in [
        (
            "bf1",
            &[
                ("com_item_powerup_ammo", 0.0, 1, 0.1255),
                // The disc's 50, 75 and 100, as the same share of the
                // shared 400-health trooper (`game::normalize`).
                ("com_item_powerup_health25", 50.0 * BF1_SCALE, 0, 0.0965),
                ("com_item_powerup_health100", 75.0 * BF1_SCALE, 0, 0.1206),
                ("com_item_powerup_dual", 100.0 * BF1_SCALE, 2, 0.1565),
            ][..],
        ),
        (
            "bf2",
            &[
                ("com_item_powerup_ammo", 0.0, 1, 0.1255),
                ("com_item_powerup_health25", 100.0, 0, 0.0965),
                ("com_item_powerup_health100", 200.0, 0, 0.0965),
                ("com_item_powerup_dual", 200.0, 2, 0.1565),
                ("com_item_powerup_energy", 0.0, 0, 0.0965),
                ("com_item_powerup_offense", 0.0, 0, 0.0965),
                ("com_item_powerup_defense", 0.0, 0, 0.0965),
            ][..],
        ),
    ] {
        let db = load_classdb_all(&dir, game);
        for (name, health, ammo, half_x) in want {
            let c = game::roster::item_class_from_db(&db, name).expect(name);
            println!("{game} {c:?}");
            assert_eq!(
                (c.health, c.ammo, c.life_span),
                (*health, *ammo, 30.0),
                "{name}"
            );
            assert_eq!(c.half.x, *half_x, "{game} {name}");
        }
        if game == "bf2" {
            let item = |n: &str| game::roster::item_class_from_db(&db, n).unwrap();
            assert_eq!(item("com_item_powerup_energy").energy, 150.0);
            assert_eq!(item("com_item_powerup_offense").offense, (12.0, 1.5));
            assert_eq!(item("com_item_powerup_defense").defense, (20.0, 0.5));
        }
        // A soldier is not an item.
        assert!(game::roster::item_class_from_db(&db, "com_inf_default").is_none());
    }
    // A fielded class registers its table with the sim.
    let db = load_classdb_all(&dir, "bf1");
    let c = ClassRoster::build(&db, "rep_inf_clone_trooper", 1, None).unwrap();
    let mut sim = core::sim::Sim::empty(1);
    let uc = c.register(&mut sim, &mut Vec::new());
    assert_eq!(uc.drops.count, 3);
    assert_eq!(uc.drops.prob[..3], [0.4, 0.3, 0.1]);
    let names: Vec<&str> = sim.item_classes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "com_item_powerup_ammo",
            "com_item_powerup_health100",
            "com_item_powerup_dual"
        ]
    );
    assert_eq!(game::roster::item_pool("bf1", "naboo1"), 40);
    assert_eq!(game::roster::item_pool("bf1", "tat1"), 60);
    assert_eq!(game::roster::item_pool("bf1", "kamino1"), 25);
    assert_eq!(game::roster::item_pool("bf2", "tat1"), 25);
}

/// Card CLSE2 (`analysis/re-soldier-ai.md` 18.4): BF2's weapon classes carry
/// the disc's AIBubble keys, a side's own class by its BASE; BF1 authors none.
#[test]
fn bf2_weapon_classes_carry_the_ai_bubble_keys() {
    let Some(dir) = classes_dir() else { return };
    let db = load_classdb_all(&dir, "bf2");
    let keys = |name: &str| {
        let w = game::roster::weapon_from_db(&db, name).expect(name);
        (w.ai_bubble_size, w.ai_bubble_dist, w.ai_bubble_clamp)
    };
    for side in ["all", "imp", "rep", "cis"] {
        assert_eq!(
            keys(&format!("{side}_weap_inf_shotgun")),
            (1.25, 20.0, 10.0)
        );
        assert_eq!(
            keys(&format!("{side}_weap_inf_sniper_rifle")),
            (0.6, 20.0, 10.0)
        );
        assert_eq!(keys(&format!("{side}_weap_inf_rifle")), (1.0, 20.0, 10.0));
    }
    let db1 = load_classdb_all(&dir, "bf1");
    let w = game::roster::weapon_from_db(&db1, "rep_weap_inf_sniperrifle").expect("bf1 sniper");
    assert_eq!(
        (w.ai_bubble_size, w.ai_bubble_dist, w.ai_bubble_clamp),
        (1.0, 20.0, 10.0)
    );
}

/// Card CLSE2 (18.3): the size term of the aim error is half the horizontal
/// diagonal of the model's bind-pose box - `AIM_SIZE` row by row against the
/// baked models' POSITION bounds, every baked BF2 soldier model in it, and a
/// class takes its model's. A cloth segment (`extras.cloth`, card CLOTH) is
/// not the model's: the PS2 draws it as its own object.
#[test]
fn aim_sizes_are_the_baked_models_boxes() {
    let Some(dir) = classes_dir() else { return };
    let soldiers = game::assets::baked_root().join("side_bf2/soldiers");
    if !soldiers.is_dir() {
        eprintln!("SKIP: no side bake under {}", soldiers.display());
        return;
    }
    let size = |model: &str| {
        let (doc, _, _) = gltf::import(soldiers.join(format!("{model}.glb"))).expect(model);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let cloth = |prim: &gltf::Primitive<'_>| {
            prim.material().extras().as_ref().is_some_and(|raw| {
                serde_json::from_str::<serde_json::Value>(raw.get())
                    .is_ok_and(|v| v["cloth"] == true)
            })
        };
        for prim in doc.meshes().flat_map(|m| m.primitives()) {
            if cloth(&prim) {
                continue;
            }
            let b = prim.bounding_box();
            for i in 0..3 {
                lo[i] = lo[i].min(b.min[i]);
                hi[i] = hi[i].max(b.max[i]);
            }
        }
        0.5 * (hi[0] - lo[0]).hypot(hi[2] - lo[2])
    };
    for (model, v) in game::roster::AIM_SIZE {
        let s = size(model);
        assert!(
            (s - v).abs() < 1e-4,
            "{model}: the table's {v}, the model's {s}"
        );
    }
    let mut baked = 0;
    for e in std::fs::read_dir(&soldiers).expect("soldiers dir") {
        let name = e.expect("entry").file_name().to_string_lossy().into_owned();
        if name.ends_with(".low.glb") {
            continue;
        }
        if let Some(model) = name.strip_suffix(".glb") {
            baked += 1;
            assert!(
                game::roster::AIM_SIZE.iter().any(|(m, _)| *m == model),
                "{model} has no AIM_SIZE row"
            );
        }
    }
    assert_eq!(baked, game::roster::AIM_SIZE.len());
    let db = load_classdb_all(&dir, "bf2");
    let class = |name: &str| ClassRoster::build(&db, name, 0, None).expect(name);
    let droid = class("cis_inf_rifleman");
    assert_eq!(droid.geometry.as_deref(), Some("cis_inf_sbdroid"));
    assert!(
        (droid.consts.aim_size - 0.4806).abs() < 1e-3,
        "{}",
        droid.consts.aim_size
    );
    let clone = class("rep_inf_ep3_rifleman");
    assert!(
        (clone.consts.aim_size - 0.9550).abs() < 1e-3,
        "{}",
        clone.consts.aim_size
    );
    let rebel = class("all_inf_rifleman");
    assert!(
        (rebel.consts.aim_size - 0.692).abs() < 1e-3,
        "{}",
        rebel.consts.aim_size
    );
    for name in ["all_hero_chewbacca", "rep_hero_yoda", "imp_hero_darthvader"] {
        let hero = class(name);
        let model = hero.geometry.as_deref().unwrap();
        assert!(
            (hero.consts.aim_size - size(model)).abs() < 1e-4,
            "{name}: its target size must come from {model}'s noncloth box"
        );
    }
}

/// Card CLSE4 (`analysis/re-soldier-ai.md` 20.3), data-gated: the burst keys
/// reach the sim's weapon classes from the BAKED class db. BF2 stores them by
/// hash; its flyer weapons (here a child of `com_weap_veh_fly_chaingun`) and
/// the defence turret write them, every infantry weapon keeps the
/// constructor's 3 / 6 / 2.0 s, and no BF1 class writes one.
#[test]
fn the_baked_weapons_carry_their_barrage_keys() {
    use game::classdb::fnv1a_lowercase;
    assert_eq!(fnv1a_lowercase("BarrageMin"), 0x1aa5_ec35);
    assert_eq!(fnv1a_lowercase("BarrageMax"), 0x04ba_e44b);
    assert_eq!(fnv1a_lowercase("BarrageDelay"), 0xffcc_84b2);
    let Some(dir) = classes_dir() else { return };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    for (db, weapon, want) in [
        (&bf2, "rep_weap_fly_vwing_cannon", ([10, 20], 0.5)),
        (&bf2, "com_weap_inf_defenseturret", ([4, 8], 0.5)),
        (&bf2, "rep_weap_inf_rifle", ([3, 6], 2.0)),
        (&bf2, "rep_weap_inf_pistol", ([3, 6], 2.0)),
        (&bf1, "all_weap_inf_rifle", ([3, 6], 2.0)),
    ] {
        let w = game::roster::weapon_from_db(db, weapon).expect(weapon);
        assert_eq!((w.barrage, w.barrage_delay), want, "{weapon}");
    }
}

/// Card HVYW (`analysis/re-soldier-ai.md` 22), data-gated: what the weapon
/// choice's vehicle and crowd rules read reaches the sim's classes from the
/// BAKED db - the ordnance chain's engine base (`OrdKind`), the AI mask's
/// person and vehicle bits, the range triple. The launchers of both games
/// are missiles whose mask refuses a person and takes a vehicle; the wrist
/// rocket is a missile for persons only, and not the droid's first weapon;
/// BF1's jet trooper "rocket launcher" is a bullet weapon and BF2's bulldog
/// a missile in the first slot - neither is a heavy weapon of the port's.
#[test]
fn the_baked_launchers_are_missiles_for_vehicles() {
    use core::weapon::OrdKind;
    let Some(dir) = classes_dir() else { return };
    let bf1 = game::roster::load_classdb(&dir, "bf1", &FOUR).expect("bf1 db");
    let bf2 = game::roster::load_classdb(&dir, "bf2", &FOUR).expect("bf2 db");
    // (weapon, kind, the mask takes a person, a vehicle, the range triple)
    for (db, weapon, kind, person, vehicle, range) in [
        (
            &bf2,
            "rep_weap_inf_rocket_launcher",
            OrdKind::Missile,
            false,
            true,
            [15.0, 75.0, 300.0],
        ),
        (
            &bf2,
            "imp_weap_inf_rocket_launcher",
            OrdKind::Missile,
            false,
            true,
            [15.0, 75.0, 300.0],
        ),
        (
            &bf2,
            "cis_weap_inf_wrist_rocket",
            OrdKind::Missile,
            true,
            false,
            [0.0, 16.0, 48.0],
        ),
        (
            &bf2,
            "cis_weap_inf_bulldog",
            OrdKind::Missile,
            true,
            false,
            [0.0, 24.0, 40.0],
        ),
        (
            &bf2,
            "rep_weap_inf_rifle",
            OrdKind::Other,
            true,
            false,
            [0.0, 24.0, 64.0],
        ),
        (
            &bf1,
            "all_weap_inf_torpedo_launcher",
            OrdKind::Missile,
            false,
            true,
            [6.0, 75.0, 150.0],
        ),
        (
            &bf1,
            "cis_weap_inf_torpedo_launcher",
            OrdKind::Missile,
            false,
            true,
            [10.0, 75.0, 150.0],
        ),
        (
            &bf1,
            "cis_weap_inf_wrist_rocket",
            OrdKind::Missile,
            true,
            false,
            [0.0, 16.0, 48.0],
        ),
        (
            &bf1,
            "rep_weap_inf_rocket_launcher",
            OrdKind::Other,
            true,
            false,
            [0.0, 30.0, 60.0],
        ),
    ] {
        let w = game::roster::weapon_from_db(db, weapon).expect(weapon);
        assert_eq!(w.ordnance.kind, kind, "{weapon}");
        assert_eq!(w.ai_target_kind & 1 != 0, person, "{weapon}: person");
        assert_eq!(w.ai_target_kind & 8 != 0, vehicle, "{weapon}: vehicle");
        assert_eq!(w.ai_range, range, "{weapon}");
    }
    // The super battle droid's class: BF1's `cis_inf_battledroid`, BF2's
    // CIS rifleman.
    for (db, droid) in [
        (&bf1, "cis_inf_battledroid"),
        (&bf2, "cis_inf_default_rifleman"),
    ] {
        let th = game::roster::thrown_weapon_from_db(db, "rep_weap_inf_thermaldetonator")
            .expect("thermal")
            .expect("a thrown weapon");
        assert_eq!(th.ordnance.kind, OrdKind::Lobbed);
        // It holds its wrist rocket behind its blaster.
        let slots = db.weapon_slots(droid);
        let slot = |name: &str| {
            slots
                .iter()
                .find(|s| s.weapon.eq_ignore_ascii_case(name))
                .map(|s| s.slot)
        };
        let (blaster, rocket) = (
            slot("cis_weap_inf_wrist_blaster").expect("the blaster"),
            slot("cis_weap_inf_wrist_rocket").expect("the wrist rocket"),
        );
        assert!(blaster < rocket, "{blaster} {rocket}");
    }
}

/// Card USKN (re-soldier-skeleton-skins 3, 5): a class with its own
/// skeleton carries its SkeletonRootScale to the animator (the PS2 RAM's
/// bone_root values), a human-rig class 1; a scale on a human-rig class
/// would not count (the setters test SkeletonName first).
#[test]
fn own_skeleton_classes_carry_their_root_scale() {
    let Some(dir) = classes_dir() else { return };
    for (game, class, scale) in [
        ("bf1", "all_inf_smuggler", 1.15),
        ("bf1", "cis_inf_assault", 1.0),
        ("bf1", "rep_inf_trooper", 1.0),
        ("bf2", "cis_inf_officer", 1.24),
        ("bf2", "all_inf_wookiee", 1.15),
        ("bf2", "all_inf_sniper", 0.95),
        ("bf2", "rep_inf_ep3_rifleman", 1.0),
    ] {
        let db = load_classdb_all(&dir, game);
        let anim = game::roster::class_anim(&db, class, &db.weapon_slots(class));
        assert_eq!(anim.root_scale, scale, "{game} {class}");
    }
}

/// Card SSELB (re-spawn-flow §4.3): a class's `PointsToUnlock` - every BF2
/// officer 8, the BF2 specials 12, every other class 0. BF1 has no unlocks;
/// `[ours]` (card UNLK1) its specials take BF2's 12 and nothing else costs.
#[test]
fn only_the_bf2_officers_and_specials_cost_points() {
    let Some(dir) = classes_dir() else { return };
    let mut costs = Vec::new();
    for (game, a, b) in [
        ("bf2", "imp", "all"),
        ("bf2", "rep", "cis"),
        ("bf1", "all", "imp"),
    ] {
        let ([ra, rb], _) = FactionRoster::pair(&dir, game, [a, b], None).expect("rosters");
        for r in [&ra, &rb] {
            for c in &r.classes {
                if c.unlock_points != 0 {
                    costs.push((game, c.class.clone(), c.unlock_points));
                }
            }
        }
    }
    costs.sort();
    eprintln!("{costs:?}");
    let bf1: Vec<_> = costs.iter().filter(|(g, ..)| *g == "bf1").collect();
    assert_eq!(
        bf1,
        [
            &("bf1", "all_inf_smuggler".to_owned(), 12),
            &("bf1", "imp_inf_dark_trooper".to_owned(), 12)
        ]
    );
    for (_, class, cost) in &costs {
        let want = if class.contains("officer") { 8 } else { 12 };
        assert_eq!(*cost, want, "{class}");
    }
    let has = |n: &str| costs.iter().any(|(_, c, _)| c == n);
    assert!(
        has("imp_inf_officer") && has("imp_inf_dark_trooper"),
        "{costs:?}"
    );
}

/// The maintainer 2026-10-27 and 2026-10-29 (`roster::PUSH_SPARES_THE_LIVING`): out
/// of BF2's disc data, the Magnaguard's Bulldog and the Bothan spy's
/// incinerator explode with a push that moves only bodies; the rocket
/// launcher's, the thermal detonator's and the mortar's still throw soldiers.
#[test]
fn only_the_bulldog_and_incinerator_pushes_spare_the_living() {
    let Some(dir) = classes_dir() else { return };
    let db = load_classdb_all(&dir, "bf2");
    let blast = |w: &str| {
        let w = game::roster::weapon_from_db(&db, w)
            .ok()
            .and_then(|w| w.ordnance.explosion)
            .or_else(|| {
                game::roster::thrown_weapon_from_db(&db, w)
                    .ok()
                    .flatten()
                    .and_then(|w| w.ordnance.sticky?.explosion)
            })
            .unwrap_or_else(|| panic!("{w}: no explosion"));
        (w.push, w.spares_living)
    };
    for w in ["cis_weap_inf_bulldog", "all_weap_inf_incinerator"] {
        let (push, spares) = blast(w);
        assert!(push > 0.0 && spares, "{w}: push {push}, spares {spares}");
    }
    for w in [
        "cis_weap_inf_rocket_launcher",
        "rep_weap_inf_thermaldetonator",
        "cis_weap_inf_mortar_launcher",
    ] {
        let (push, spares) = blast(w);
        assert!(push > 0.0 && !spares, "{w}: push {push}, spares {spares}");
    }
}

/// Issue #42: every soldier class on both discs, slot by slot, against the
/// disc's own class data (baked here from the SIDE LVLs): the weapon, its
/// channel and its clips, what kind of item it is (`roster::slot_kind`, from
/// the weapon's and its ordnance's engine bases) and what the port fields.
/// A slot is fielded with the disc's channel and clips (a `WEAPON_DONORS`
/// class: its donor's), thrown exactly when it is a grenade with a sticky,
/// or left out with its reason; a class that cannot be fielded says why.
/// A BF1 class's rows end with the BF2 award slots it gains
/// (`roster::awards`), marked as such.
/// The table is held to `tests/data/class_kits.txt` (`CLASS_KITS_BLESS=1`
/// rewrites it), so any change to a class's kit is a diff to explain.
#[test]
fn every_class_kit_is_the_discs_or_explained() {
    use game::roster::{class_slot_kind, SlotKind, WEAPON_DONORS};
    use std::fmt::Write as _;
    let Some(dir) = classes_dir() else { return };
    let mut out = String::new();
    let mut counts = [0usize; 4];
    let bf2 = load_classdb_all(&dir, "bf2");
    let saber_models: std::collections::BTreeSet<_> = bf2
        .names()
        .filter(|name| game::roster::is_soldier_chain(&bf2, name))
        .filter_map(|name| {
            let (weapon, _) = game::roster::primary_weapon(&bf2, name)?;
            if game::roster::slot_kind(&bf2, &weapon, 0) != SlotKind::Melee {
                return None;
            }
            Some(
                bf2.prop(name, "GeometryName")
                    .unwrap_or_else(|| panic!("{name}: melee model has no GeometryName"))
                    .to_owned(),
            )
        })
        .collect();
    assert_eq!(saber_models.len(), 14);
    let root = bf2.assets_root().expect("explicit companion root");
    let missing: Vec<_> = saber_models
        .iter()
        .filter(|model| {
            !pak::exists(
                root.join("side_bf2/soldiers")
                    .join(format!("{model}.combos.ron")),
            )
        })
        .collect();
    if missing.len() == saber_models.len() {
        eprintln!("SKIP: no BF2 saber model companions; rebake BF2 heroes");
        return;
    }
    assert!(
        missing.is_empty(),
        "partial BF2 saber companions: {missing:?}"
    );
    for game in ["bf1", "bf2"] {
        let db = load_classdb_all(&dir, game);
        let mut names: Vec<String> = db
            .names()
            .filter(|n| game::roster::is_soldier_chain(&db, n))
            .map(str::to_owned)
            .collect();
        names.sort();
        names.dedup();
        for c in &names {
            let built = ClassRoster::build(&db, c, 1, None);
            let donor = WEAPON_DONORS.iter().find(|d| d.class == c);
            let line = match (&built, donor) {
                (Err(e), _) => format!(
                    "{game} {c}: NOT FIELDED ({})",
                    e.to_string().lines().next().unwrap_or("")
                ),
                (Ok(_), Some(d)) => format!(
                    "{game} {c}: takes BF2 {}'s weapons (WEAPON_DONORS)",
                    d.donor
                ),
                (Ok(_), None) => format!("{game} {c}"),
            };
            writeln!(out, "{line}").unwrap();
            let Ok(r) = &built else { continue };
            // The donor's slots come out of BF2's data; the rows are its.
            let slots = r.slots.clone();
            let kdb = if donor.is_some() { &bf2 } else { &db };
            for s in &slots {
                let w = s.weapon.to_ascii_lowercase();
                let disc = format!("{}:{} x{:?} ch{}", s.slot + 1, w, s.ammo, s.channel);
                let kind = kdb
                    .resolve(&w)
                    .map(|n| class_slot_kind(kdb, c, n, s.channel));
                let got = if r.weapon_slot == s.slot {
                    assert_eq!(r.total_clips, s.ammo.unwrap_or(0), "{game} {c} {disc}");
                    counts[0] += 1;
                    format!("primary x{}", r.total_clips)
                } else if let Some(h) = r.held.iter().find(|h| h.slot == s.slot) {
                    assert_eq!(h.channel, s.channel, "{game} {c} {disc}");
                    assert_eq!(h.clips, s.ammo.unwrap_or(0), "{game} {c} {disc}");
                    let thrown = h.weapon.ordnance.sticky.is_some() && h.weapon.thrown.is_some();
                    if let Some(k) = kind {
                        assert_eq!(thrown, k == SlotKind::Thrown, "{game} {c} {disc}: {k:?}");
                        assert_eq!(
                            h.weapon.repair.is_some(),
                            k == SlotKind::Repair,
                            "{game} {c} {disc}"
                        );
                    }
                    counts[1] += 1;
                    let how = if thrown {
                        "thrown"
                    } else if h.weapon.ordnance.sticky.is_some() {
                        "launched"
                    } else if h.weapon.repair.is_some() {
                        "repair"
                    } else if h.weapon.turret.is_some() {
                        "auto-turret"
                    } else if h.weapon.dispenser.is_some() || h.weapon.mine.is_some() {
                        "dispenser"
                    } else if h.weapon.ability.is_some() {
                        "ability"
                    } else if h.weapon.remote.is_some() {
                        "remote droid"
                    } else {
                        "fired"
                    };
                    // A BF1 class's award slot is BF2's class, not its disc's.
                    if game == "bf1" && donor.is_none() && kind.is_none() {
                        assert!(bf2.resolve(&w).is_some(), "{game} {c} {disc}");
                        assert!(h.weapon.unlock_medal().is_some(), "{game} {c} {disc}");
                        format!("{how} x{} (BF2's award, roster::awards)", h.clips)
                    } else {
                        format!("{how} x{}", h.clips)
                    }
                } else {
                    counts[2] += 1;
                    match kind {
                        Some(SlotKind::NotBuilt(why)) => format!("left out: {why}"),
                        k => panic!("{game} {c} {disc}: left out with no reason ({k:?})"),
                    }
                };
                writeln!(out, "    {disc} -> {got}").unwrap();
            }
            counts[3] += 1;
        }
    }
    println!("{out}");
    println!(
        "classes {} | slots: primary {}, held {}, left out {}",
        counts[3], counts[0], counts[1], counts[2]
    );
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/class_kits.txt");
    if std::env::var_os("CLASS_KITS_BLESS").is_some() {
        std::fs::write(path, &out).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path)
        .expect("tests/data/class_kits.txt (CLASS_KITS_BLESS=1 writes it)");
    let diff: Vec<String> = want
        .lines()
        .zip(out.lines())
        .filter(|(a, b)| a != b)
        .take(20)
        .map(|(a, b)| format!("- {a}\n+ {b}"))
        .collect();
    assert!(
        diff.is_empty() && want.lines().count() == out.lines().count(),
        "class kits moved (CLASS_KITS_BLESS=1 after explaining them):\n{}",
        diff.join("\n")
    );
}

/// Issue #29 (and #42's bowcaster): the charge stages out of both discs'
/// class data (`analysis/re-weapon-charge.md` survey). BF1's bowcaster steps
/// from one bolt to fans of 3, 5 and 7 (0.8 degree steps); BF2's from a
/// 7-bolt fan (0.7 degree steps) to one 600 m/s, 300 damage bolt. Both
/// mortars fire a shell on a tap and a sticky past 0.9 of the charge.
#[test]
fn charge_stages_are_the_discs() {
    let Some(dir) = classes_dir() else { return };
    let yaws = |w: &core::weapon::WeaponClass| -> Vec<i32> {
        w.shot_tilts
            .iter()
            .map(|t| (t.1 / core::weapon::DEG * 10.0).round() as i32)
            .collect()
    };
    let bf1 = load_classdb_all(&dir, "bf1");
    let bow = game::roster::weapon_from_db(&bf1, "all_weap_inf_bowcaster").unwrap();
    let th: Vec<f32> = bow.stages.iter().map(|s| s.threshold).collect();
    assert_eq!(th, [0.15, 0.35, 0.65, 0.95]);
    let shots: Vec<u32> = (0..5).map(|k| bow.stage(k).shots_per_salvo).collect();
    assert_eq!(shots, [1, 1, 3, 5, 7]);
    assert_eq!(yaws(bow.stage(2)), [0, -8, 8]);
    assert_eq!(yaws(bow.stage(4)), [0, -8, 8, -16, 16, -24, 24]);
    assert_eq!(
        (bow.stage_for(0.1), bow.stage_for(0.5), bow.stage_for(1.0)),
        (0, 2, 4)
    );
    let bf2 = load_classdb_all(&dir, "bf2");
    let bow = game::roster::weapon_from_db(&bf2, "all_weap_inf_bowcaster").unwrap();
    assert_eq!(bow.stages.len(), 1);
    assert_eq!(
        (bow.shots_per_salvo, yaws(&bow)),
        (7, vec![0, -7, 7, -14, 14, -21, 21])
    );
    let heavy = bow.stage(1);
    assert_eq!(heavy.shots_per_salvo, 1);
    assert_eq!(
        (heavy.ordnance.velocity, heavy.ordnance.max_damage),
        (600.0, 300.0)
    );
    for (db, w) in [
        (&bf1, "imp_weap_inf_mortar_launcher"),
        (&bf2, "cis_weap_inf_mortar_launcher"),
    ] {
        let m = game::roster::weapon_from_db(db, w).unwrap();
        assert_eq!(
            m.stages.iter().map(|s| s.threshold).collect::<Vec<_>>(),
            [0.9],
            "{w}"
        );
        assert!(
            m.ordnance.sticky.is_none() && m.ordnance.kind == core::weapon::OrdKind::Shell,
            "{w}: a tap's shell"
        );
        assert!(
            m.stage(1).ordnance.sticky.is_some(),
            "{w}: the held round is a sticky"
        );
        assert_eq!(m.ordnance.gravity, 1.75, "{w}: the shell's Gravity");
        assert!(
            m.ordnance.expire.is_some(),
            "{w}: the shell's ExplosionExpire"
        );
    }
}

/// Issue #29: the ordnance charge points and the emitter's damage out of the
/// BF2 disc - the arc caster blends 300 -> 325 by the charge; the Hoth dish
/// turret's ordnance carries its three points after a 1000 root.
#[test]
fn ordnance_charge_points_are_the_discs() {
    let Some(dir) = classes_dir() else { return };
    let bf2 = load_classdb_all(&dir, "bf2");
    let arc = game::roster::weapon_from_db(&bf2, "imp_weap_inf_arccaster").unwrap();
    assert_eq!(
        (arc.ordnance.min_damage, arc.ordnance.max_damage),
        (Some(300.0), 325.0)
    );
    let baked = game::assets::baked_root().join("classes");
    let mut db = game::classdb::ClassDb::default();
    for f in ["bf2_ingame.ron", "bf2_tur.ron"] {
        if !baked.join(f).exists() {
            return;
        }
        db.load(&baked.join(f)).unwrap();
    }
    let dish = game::roster::weapon_from_db(&db, "tur_weap_hoth_dishturret").unwrap();
    let pts: Vec<(f32, f32)> = dish
        .ordnance
        .knots
        .iter()
        .map(|k| (k.at, k.damage))
        .collect();
    assert_eq!(dish.ordnance.max_damage, 1000.0);
    let laser = dish.ordnance.laser.as_ref().expect("dish turret laser");
    assert_eq!(laser.length, 3.0);
    assert_eq!(&laser.light_color[..3], &[75, 75, 18]);
    assert_eq!(pts, [(0.5, 1250.0), (0.75, 1500.0), (2.0, 3000.0)]);
}

/// Card DROIDDEATH (`analysis/re-droid-death.md` §1): of every soldier class
/// on both discs, exactly the CIS soldier droids have a chunk death - BF1's
/// eight droid classes and every BF2 class under `cis_inf_default` (the
/// Magnaguard included) - each with ChunkFrequency 0.6 and its explosion's
/// effect (BF1 `droid_explosion`, BF2 `cis_sfx_droid_explosion`, no sound);
/// no clone, trooper, rebel, hero or local class has one.
#[test]
fn only_the_cis_soldier_droids_have_a_chunk_death() {
    let Some(dir) = classes_dir() else { return };
    for (game, effect) in [
        ("bf1", "droid_explosion"),
        ("bf2", "cis_sfx_droid_explosion"),
    ] {
        let db = load_classdb_all(&dir, game);
        let mut names: Vec<&str> = db
            .names()
            .filter(|c| game::roster::is_soldier_chain(&db, c))
            .collect();
        names.sort_unstable();
        let mut chunked = Vec::new();
        for c in names {
            let Some(d) = game::roster::chunk_death(&db, c) else {
                continue;
            };
            assert_eq!(db.prop_f32(c, "ChunkFrequency"), Some(0.6), "{game} {c}");
            assert_eq!(d.frequency, game::roster::DROID_CHUNK_CHANCE, "{game} {c}");
            let look = &d.explosion.as_ref().expect(c).look;
            assert_eq!(look.effects, [effect], "{game} {c}");
            assert_eq!(look.sound, None, "{game} {c}");
            chunked.push(c.to_owned());
        }
        assert!(
            chunked.iter().all(|c| c.starts_with("cis_inf_")),
            "{chunked:?}"
        );
        if game == "bf1" {
            assert_eq!(
                chunked,
                [
                    "cis_inf_assassindroid",
                    "cis_inf_assault",
                    "cis_inf_battledroid",
                    "cis_inf_battledroid_av",
                    "cis_inf_battledroid_inf",
                    "cis_inf_battledroid_pilot",
                    "cis_inf_battledroid_sniper",
                    "cis_inf_pilotdroid",
                ]
            );
        } else {
            for c in [
                "cis_inf_rifleman",
                "cis_inf_rocketeer",
                "cis_inf_engineer",
                "cis_inf_sniper",
                "cis_inf_officer",
            ] {
                assert!(chunked.iter().any(|x| x == c), "{c} in {chunked:?}");
            }
        }
        for c in [
            "rep_inf_ep3_rifleman",
            "rep_inf_clone_trooper",
            "cis_hero_grievous",
            "cis_inf_countdooku",
            "imp_inf_rifleman",
        ] {
            if db.get(c).is_some() {
                assert!(game::roster::chunk_death(&db, c).is_none(), "{game} {c}");
            }
        }
    }
}

/// The maintainer: "Use the bf2 droideka shield hud icon." Both games' rows
/// carry the BF2 class's `HUDTag`; its mesh and the layout's place for it are
/// in the bake on a stage that has it, and the primary keeps its gun's model.
#[test]
fn a_droideka_shows_the_bf2_shield_in_the_secondary_row_on_both_games() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_cis.ron").exists() {
        eprintln!("SKIP: no baked classes");
        return;
    }
    for game_id in ["bf1", "bf2"] {
        let db = game::roster::load_classdb(&root.join("classes"), game_id, &["cis"]).unwrap();
        let play = game::roster::load_classdb(&root.join("classes"), "bf2", &["cis"]).unwrap();
        let c = if game_id == "bf1" { &play } else { &db };
        let shield = game::droideka::shield_weapon(c, "cis_inf_droideka").unwrap();
        assert_eq!(shield, "cis_weap_walk_droideka_shield");
        let hud = game::roster::weapon_hud(c, &shield);
        assert_eq!(hud.key, "hud_droideka_shield", "{game_id}");
        assert_eq!(hud.icon, None, "{game_id}");
    }
    let layout = std::fs::read_to_string(root.join("common_bf2/hud/1playerhud.ron")).unwrap();
    assert!(layout.contains("hud_droideka_shield"));
    let baked = root.join("side_bf2/weapons/hud_droideka_shield.glb");
    if !baked.exists() {
        eprintln!("SKIP: the rebake's hud_droideka_shield is not in this bake");
    }
    assert!(root
        .join("common_bf2/models/hud_vehicle_lasers.glb")
        .exists());
}
