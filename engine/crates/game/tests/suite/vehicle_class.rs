//! Tier-1 (data-gated): a hover vehicle's typed class numbers and seat ring,
//! read from the disc's own classes — the map-local TAT1 skiff (the map
//! bake's `classes.ron`) and BF1's side speeder bike
//! (analysis/re-vehicles.md §3.1, §5).

use core::weapon::HealthType;
use game::classdb::ClassDb;
use game::vehicle_class::{
    damage_effects, explosion_name, health_type, neutral_when_empty, seats, HoverClass, PilotType,
    SeatKind,
};
use std::path::PathBuf;

/// Bake `rel` (under extracted/bf1/lvl/DATA/_LVL_PS2) to a scratch RON and
/// load it, or `false` when the disc is absent.
fn load(db: &mut ClassDb, rel: &str, tag: &str) -> bool {
    load_from(db, "bf1", rel, tag)
}

/// [`load`] from `game`'s disc (`bf1` / `bf2`).
fn load_from(db: &mut ClassDb, game: &str, rel: &str, tag: &str) -> bool {
    let lvl = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted"))
        .join(game)
        .join("lvl/DATA/_LVL_PS2")
        .join(rel);
    if !lvl.exists() {
        eprintln!("SKIP: {} not present", lvl.display());
        return false;
    }
    let out = std::env::temp_dir().join(format!("openbf-vcls-{tag}-{}.ron", std::process::id()));
    bake::classes::bake_classes(lvl.to_str().unwrap(), out.to_str().unwrap()).expect("bake");
    db.load(&out).expect("load");
    if std::env::var_os("OPENBF_KEEP_SCRATCH").is_none() {
        let _ = std::fs::remove_file(&out);
    }
    true
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn tat1_skiff_hover_numbers_and_seats() {
    let mut db = ClassDb::default();
    // The map's own classes, then the common LVL (the gunners' weapon).
    if !load(&mut db, "TAT/TAT1.LVL", "tat1") || !load(&mut db, "COMMON/ENG/COMMON.LVL", "com") {
        return;
    }
    let c = HoverClass::from_db(&db, "tat_hover_skiff").expect("skiff is a hover");
    eprintln!("{c:#?}");
    let p = c.params();
    assert_eq!(p.set_altitude, 1.0);
    assert_eq!(p.gravity_scale, 4.0);
    assert_eq!(c.lift_spring, 10.0);
    assert_eq!(p.forward_speed, 16.0);
    assert_eq!(p.traction, 30.0);
    assert_eq!(p.strafe_speed, 4.0);
    // BODY's own rates, not TURRET1's TurnRate 1.3 / PitchRate 0.8.
    assert_eq!((p.turn_rate, p.pitch_rate, p.turn_filter), (1.0, 1.5, 2.0));
    // "3//low": the comment is not part of the number.
    assert_eq!(
        (p.bank_filter, p.level_spring, c.level_damp),
        (3.0, 7.0, 1.0)
    );
    // BODY's PitchLimits "-12 8" in degrees → radians.
    assert!(close(c.pitch_limits[0], (-12.0f32).to_radians()));
    assert!(close(c.pitch_limits[1], 8.0f32.to_radians()));
    // "30.0" fills the first of three; the rest keep the ctor's 0, 0.
    assert_eq!(p.collision_scale, [30.0, 0.0, 0.0]);
    assert_eq!(p.collision_threshold, [7.5, 2.5, 0.0]);
    assert_eq!(
        (
            p.velocity_spring,
            p.velocity_damp,
            p.omega_x_spring,
            p.omega_z_spring
        ),
        (5.0, 2.5, 4.0, 6.0)
    );
    assert!(p.random_spring);
    // Three spring points; the rear one's X factor authored (2), the
    // others the lever-arm defaults.
    assert_eq!(c.springs().len(), 3);
    assert_eq!(c.springs()[0].pos.to_array(), [-1.8, -0.7, 1.7]);
    assert_eq!(c.springs()[0].radius, 2.0);
    assert!(c.springs().iter().all(|p| p.length == 1.0));
    assert_eq!(
        (c.springs()[0].omega_x_spring, c.springs()[0].omega_z_spring),
        (-1.0, -1.0)
    );
    assert_eq!(
        (c.springs()[1].omega_x_spring, c.springs()[1].omega_z_spring),
        (-1.0, 1.0)
    );
    assert_eq!(
        (c.springs()[2].omega_x_spring, c.springs()[2].omega_z_spring),
        (2.0, 0.0)
    );

    let ring = seats(&db, "tat_hover_skiff");
    let kinds: Vec<SeatKind> = ring.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            SeatKind::Pilot,
            SeatKind::Gunner(0),
            SeatKind::Gunner(1),
            SeatKind::Gunner(2)
        ]
    );
    assert!(
        core::vehicle::is_ring(&kinds),
        "the sim takes the ring as it is"
    );
    assert!(ring.iter().all(|s| s.pilot_type == PilotType::Vehicle));
    assert_eq!(ring[0].pilot_position.as_deref(), Some("hp_pilot"));
    assert!(ring[0].weapons.is_empty(), "the skiff's driver has no gun");
    for (s, hp) in ring[1..]
        .iter()
        .zip(["hp_activel", "hp_active", "hp_activer"])
    {
        assert_eq!(s.pilot_position.as_deref(), Some(hp));
        assert_eq!(s.weapons.len(), 1);
        assert_eq!(s.weapons[0].class, "com_weap_bldg_gunturret");
        assert_eq!(s.weapons[0].ammo, 1);
        assert!(db.get_kind("wpnc", &s.weapons[0].class).is_some());
    }
    // The seat cameras (card VSEAT): the driver's has no ThirdPersonFOV.
    assert_eq!(ring[0].track_center, Some([0.0, 1.0, -2.0]));
    assert_eq!(ring[0].track_offset, Some([0.0, 1.0, 8.0]));
    assert_eq!(ring[0].tilt_value, Some(0.0));
    assert_eq!(ring[0].third_person_fov, None);
    assert_eq!(ring[1].track_center, Some([0.0, 1.5, -3.0]));
    assert_eq!(
        (ring[1].tilt_value, ring[1].third_person_fov),
        (Some(3.0), Some(58.0))
    );
    // What the use input reads: every seat boards; the springs' bound is
    // the rear sphere's |(0, 0.3, −2)| + 3.
    let setup = game::vehicle_pads::seat_setup(&db, "tat_hover_skiff", &ring, "bf1");
    assert_eq!(setup.boardable, vec![true; 4]);
    assert!((setup.radius - (0.09f32 + 4.0).sqrt() - 3.0).abs() < 1e-5);
    assert_eq!(setup.exit_drop, 10.0);
}

#[test]
fn bf1_speederbike_hover_numbers_and_seat() {
    let mut db = ClassDb::default();
    if !load(&mut db, "SIDE/IMP.LVL", "imp") {
        return;
    }
    let c = HoverClass::from_db(&db, "imp_hover_speederbike").expect("bike is a hover");
    let p = c.params();
    assert_eq!(p.strafe_speed, 0.0);
    assert_eq!(p.forward_speed, 40.0);
    assert!(close(p.set_altitude, 0.85));
    assert_eq!(p.collision_scale, [1.0, -0.5, 0.0]);
    assert_eq!(c.springs().len(), 3);
    assert_eq!(
        c.springs()[0].omega_x_spring,
        -2.0,
        "authored after the first point"
    );
    assert_eq!(
        c.springs()[1].omega_x_spring,
        1.0,
        "z = -1.0: the lever-arm default"
    );
    assert!(close(c.springs()[1].length, 0.7));
    // No section lines past BODY: one pilot seat, carrying the cannon.
    let ring = seats(&db, "imp_hover_speederbike");
    assert_eq!(ring.len(), 1);
    assert_eq!(ring[0].kind, SeatKind::Pilot);
    assert_eq!(ring[0].pilot_type, PilotType::Vehicle);
    assert_eq!(ring[0].weapons.len(), 1);
    assert_eq!(
        ring[0].weapons[0].class,
        "imp_weap_hover_speederbike_cannon"
    );
    assert_eq!(ring[0].weapons[0].ammo, 0);
    assert_eq!(ring[0].eye_point_offset, Some([0.0, 1.0, 0.0]));
    // BF1 authors no boost, energy or jump: its bike neither boosts nor jumps.
    assert_eq!((p.boost_speed, p.boost_acceleration), (0.0, 0.0));
    assert_eq!(p.jump.force, 0.0);
    // Not a hover: a soldier and an unknown class.
    assert!(HoverClass::from_db(&db, "imp_inf_stormtrooper").is_none());
    assert!(HoverClass::from_db(&db, "no_such_class").is_none());
    assert!(seats(&db, "no_such_class").is_empty());
}

/// re-vehicles §7.1: the hull's HealthType is the class's (BF1's tauntaun is
/// "animal"), and only the command bases keep their team when emptied. Read
/// from the baked BF1 side classes.
#[test]
fn bf1_health_type_and_neutral_when_empty_per_class() {
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets_baked/classes"
    ));
    let mut db = ClassDb::default();
    for side in ["bf1_all.ron", "bf1_imp.ron"] {
        let p = dir.join(side);
        if !p.exists() {
            eprintln!("SKIP: {} not present", p.display());
            return;
        }
        db.load(&p).expect("load");
    }
    assert_eq!(health_type(&db, "all_walk_tauntaun"), HealthType::Animal);
    assert_eq!(
        health_type(&db, "imp_hover_speederbike"),
        HealthType::Vehicle
    );
    assert_eq!(health_type(&db, "imp_walk_atat"), HealthType::Vehicle);
    assert!(neutral_when_empty(&db, "imp_hover_speederbike"));
    assert!(neutral_when_empty(&db, "all_walk_tauntaun"));
    assert!(neutral_when_empty(&db, "no_such_class"), "the PS2 default");
    assert_eq!(db.engine_base("imp_walk_atat"), Some("commandwalker"));
    assert!(!neutral_when_empty(&db, "imp_walk_atat"));
    assert_eq!(db.engine_base("all_hover_hovernaut"), Some("commandhover"));
    assert!(!neutral_when_empty(&db, "all_hover_hovernaut"));
}

/// re-vehicles §6 on the disc: the bike's three bands (sparks 60→20 %,
/// smoke 30→0 %, flame 2→0 %) and the AT-ST's seven, and each class's
/// `_exp` explosion.
#[test]
fn bf1_damage_bands_and_explosions() {
    // The setter's own arithmetic: percent × 0.01 in f32.
    let pct = |p: f32| p * 0.01;
    let mut db = ClassDb::default();
    if !load(&mut db, "SIDE/IMP.LVL", "imp-dmg") {
        return;
    }
    let bike = damage_effects(&db, "imp_hover_speederbike");
    let got: Vec<(f32, f32, &str, f32)> = bike
        .iter()
        .map(|d| (d.start, d.stop, d.effect.as_str(), d.scale))
        .collect();
    assert_eq!(
        got,
        [
            (pct(60.0), pct(20.0), "vehiclespark", 0.4),
            (pct(30.0), 0.0, "vehiclesmoke", 0.4),
            (pct(2.0), 0.0, "vehicleflame", 0.2)
        ]
    );
    assert!(bike
        .iter()
        .all(|d| d.attach.as_deref() == Some("hp_damage_1") && d.inherit_velocity == 0.75));
    // At 15 % health the bike smokes and no longer sparks.
    let on: Vec<&str> = bike
        .iter()
        .filter(|d| d.active(0.15))
        .map(|d| d.effect.as_str())
        .collect();
    assert_eq!(on, ["vehiclesmoke"]);

    let atst = damage_effects(&db, "imp_walk_atst");
    assert_eq!(atst.len(), 7);
    assert_eq!(atst[0].attach.as_deref(), Some("bone_l_calf"));
    assert_eq!(
        (atst[0].start, atst[0].stop, atst[0].scale),
        (pct(75.0), pct(30.0), 1.0)
    );
    assert_eq!(atst[6].effect, "vehicleflame");
    assert_eq!(atst[6].attach.as_deref(), Some("hp_damage_3"));
    assert_eq!(atst[6].start, pct(19.0));

    assert_eq!(
        explosion_name(&db, "imp_hover_speederbike").as_deref(),
        Some("imp_hover_speederbike_exp")
    );
    assert_eq!(
        explosion_name(&db, "imp_walk_atst").as_deref(),
        Some("imp_walk_atst_exp")
    );
    assert!(db.get_kind("expc", "imp_walk_atst_exp").is_some());
    assert_eq!(
        db.prop_f32("imp_hover_speederbike", "MaxHealth"),
        Some(600.0)
    );
    assert_eq!(db.prop_f32("imp_walk_atst", "MaxHealth"), Some(6000.0));
}

/// re-hover §1, §6.3, §6.9 (card HOVX): BF2's speeder bike authors boost,
/// energy and the jump — the two unnamed jump hashes included — and the
/// fighter tank boost and energy but no jump.
#[test]
fn bf2_speederbike_boost_energy_and_jump() {
    let mut db = ClassDb::default();
    if !load_from(&mut db, "bf2", "SIDE/IMP.LVL", "imp2") {
        return;
    }
    let p = HoverClass::from_db(&db, "imp_hover_speederbike")
        .expect("bike is a hover")
        .params();
    assert_eq!(
        (p.boost_speed, p.boost_acceleration, p.boost_turn_multiplier),
        (40.0, 100.0, 0.3)
    );
    assert_eq!((p.energy_auto_restore, p.energy_boost_drain), (15.0, 20.0));
    let j = p.jump;
    assert_eq!((j.time_min, j.time_max, j.force), (0.1, 0.35, 50.0));
    assert_eq!((j.speed_factor, j.energy_drain), (0.4, 100.0));
    let t = HoverClass::from_db(&db, "imp_hover_fightertank")
        .expect("tank is a hover")
        .params();
    assert_eq!((t.boost_speed, t.boost_acceleration), (20.0, 100.0));
    assert_eq!((t.energy_auto_restore, t.energy_boost_drain), (7.5, 20.0));
    assert_eq!(t.jump.force, 0.0);
    assert_eq!(t.jump.speed_factor, 0.3, "the ctor default");
}

/// BF2's tread scroll (`analysis/re-hover.md` §6.18): the disc's two
/// treaded classes, through the whole class parse; no BF1 class has any.
#[test]
fn bf2_treads_are_the_authored_segments_and_rates() {
    let mut db = ClassDb::default();
    if !load_from(&mut db, "bf2", "SIDE/CIS.LVL", "cis2") {
        return;
    }
    let treads = |c: &str| {
        HoverClass::from_db(&db, c)
            .expect("a hover")
            .treads
            .list()
            .to_vec()
    };
    let snail = treads("cis_tread_snailtank");
    assert_eq!(snail.len(), 1);
    assert_eq!(snail[0].segment, "cis_tank_tread_tracks");
    assert_eq!(
        (snail[0].veloc, snail[0].omega),
        ([0.0, -0.8333333], [0.0, 0.0])
    );
    let hail = treads("cis_tread_hailfire");
    let rows: Vec<_> = hail
        .iter()
        .map(|t| (t.segment.as_str(), t.veloc, t.omega))
        .collect();
    assert_eq!(
        rows,
        [
            ("wheel_left", [0.4, 0.0], [-1.1, 0.0]),
            ("wheel_right", [0.4, 0.0], [1.1, 0.0]),
        ]
    );
    let mut bf1 = ClassDb::default();
    if load(&mut bf1, "SIDE/CIS.LVL", "cis1") {
        let h = HoverClass::from_db(&bf1, "cis_tread_hailfire").expect("a hover");
        assert!(h.treads.list().is_empty());
    }
}

/// The snail tank authors its five damage bands after its eight
/// `CHUNKSECTION`s; a chunk section closes nothing (B2 FUN_00214600
/// B2:126998, B2 FUN_0026a340 B2:186058), so all five are the hull's.
#[test]
fn bf2_damage_rows_after_chunk_sections_are_the_hulls() {
    let mut db = ClassDb::default();
    if !load_from(&mut db, "bf2", "SIDE/CIS.LVL", "cis2-dmg") {
        return;
    }
    let pct = |p: f32| p * 0.01;
    let got: Vec<_> = damage_effects(&db, "cis_tread_snailtank")
        .into_iter()
        .map(|d| (d.start, d.stop, d.effect, d.attach.unwrap_or_default()))
        .collect();
    let row = |a: f32, b: f32, e: &str, n: &str| (pct(a), pct(b), e.to_owned(), n.to_owned());
    assert_eq!(
        got,
        [
            row(60.0, 20.0, "com_sfx_vehiclespark", "hp_damage2"),
            row(70.0, 20.0, "com_sfx_vehiclespark", "hp_damage3"),
            row(70.0, 20.0, "com_sfx_vehiclespark", "hp_damage2"),
            row(50.0, 0.0, "com_sfx_vehiclesmoke", "hp_damage1"),
            row(5.0, 0.0, "com_sfx_vehicleflame", "hp_damage1"),
        ]
    );
}

/// Card VDTH (`analysis/re-vehicle-death.md` §3 [O]): the sphere a blast
/// measures each BF1 vehicle class by — `max(modl SPHR r, the farthest
/// AddSpringBody / ODF collision primitive)` about the SPHR centre — against
/// the PS2's own `w`, read out of 50 BF1 save states (`veh_w.py`): the SPHR
/// alone for the walkers, flyers and treads, the springs for the bike, STAP
/// and fighter tank, the aacylinder for the AT-ST.
#[test]
fn bf1_blast_spheres_match_the_ps2() {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted"))
        .join("bf1/lvl/DATA/_LVL_PS2/SIDE");
    let mut db = ClassDb::default();
    let mut sphr = std::collections::HashMap::new();
    for side in ["IMP", "REP", "CIS", "ALL"] {
        if !load(&mut db, &format!("SIDE/{side}.LVL"), &format!("w{side}")) {
            return;
        }
        let buf = std::fs::read(root.join(format!("{side}.LVL"))).unwrap();
        let inner = mungefs::open(&buf).unwrap();
        for c in mungefs::content_chunks(inner).unwrap() {
            if &c.tag == b"modl" {
                let m = mungefs::model::Model::parse(&c).unwrap();
                sphr.entry(m.name.to_ascii_lowercase()).or_insert(m.sphere);
            }
        }
    }
    let oracle = [
        ("imp_hover_speederbike", 3.1467),
        ("cis_hover_stap", 4.0043),
        ("imp_hover_fightertank", 6.3306),
        ("imp_walk_atst", 6.39988),
        ("imp_walk_atat", 15.9937),
        ("rep_walk_atte", 10.9674),
        ("cis_hover_aat", 7.5965),
        ("cis_walk_spider", 15.1219),
        ("cis_tread_hailfire", 7.3617),
        ("all_walk_tauntaun", 2.4406),
    ];
    for (class, want) in oracle {
        let geo = db
            .prop(class, "GeometryName")
            .expect(class)
            .trim()
            .to_ascii_lowercase();
        let s = *sphr
            .get(&geo)
            .unwrap_or_else(|| panic!("{class}: no modl {geo}"));
        assert!(s.is_some(), "{class}: {geo} has no SPHR");
        let (_, w) = game::vehicle_class::blast_sphere(&db, class, s);
        eprintln!("{class}: w {w} (PS2 {want})");
        assert!((w - want).abs() < 2e-4, "{class}: {w} vs {want}");
    }
}

/// Card HHUL (re-hover §13.2 [O]): the class parse replayed over every hover
/// class of both discs. Each names its `AddSpringBody` spheres into the
/// terrain and the building subset and nothing else — but the two BF2
/// fighter tanks, whose `BuildingCollision = p_cube1` adds that box to the
/// buildings. The PS2's RAM holds exactly these subsets on 18 of 18 live
/// classes (§13.4; `lead_tmp/hhul/census.py` is the same replay): 12 BF1
/// classes with the TAT1 skiff, 10 BF2.
#[test]
fn hover_hull_names_follow_the_class_parse() {
    let mut counts = Vec::new();
    for game in ["bf1", "bf2"] {
        let mut db = ClassDb::default();
        let mut files = vec![
            "SIDE/ALL.LVL",
            "SIDE/CIS.LVL",
            "SIDE/IMP.LVL",
            "SIDE/REP.LVL",
        ];
        if game == "bf1" {
            files.push("TAT/TAT1.LVL");
        }
        for (i, f) in files.iter().enumerate() {
            if !load_from(&mut db, game, f, &format!("hhul{game}{i}")) {
                return;
            }
        }
        let mut names: Vec<String> = db.names().map(str::to_owned).collect();
        names.sort();
        let mut n = 0;
        for name in names {
            let Some(c) = HoverClass::from_db(&db, &name) else {
                continue;
            };
            n += 1;
            let springs: Vec<String> = (1..=c.springs().len())
                .map(|i| format!("springbody{i}"))
                .collect();
            assert!(!springs.is_empty(), "{game} {name}: no spring");
            assert_eq!(c.terrain_bodies, springs, "{game} {name}: terrain");
            let (terrain, building) = c.hull_names();
            assert!(terrain.is_empty(), "{game} {name}");
            let tank = game == "bf2" && name.ends_with("_hover_fightertank");
            if tank {
                assert_eq!(building, ["p_cube1"], "{game} {name}");
                assert_eq!(c.building_bodies.len(), 5, "{name}: p_cube1 + 4 springs");
            } else {
                assert!(building.is_empty(), "{game} {name}: {building:?}");
                assert_eq!(c.building_bodies, springs, "{game} {name}: building");
            }
            assert_eq!(c.names_a_hull_body(), tank);
            assert!(c.params().hull.is_empty(), "no geometry, no body");
        }
        counts.push(n);
    }
    assert_eq!(counts, [12, 10], "hover classes per disc");
}

/// Card HHUL: `p_cube1` out of the baked BF2 fighter tank geometry — the box
/// §13.2's census gives, half-extents (1.511, 0.491, 0.956) at (0, 0.511,
/// 1.559) with no rotation, in the building subset only; its front face
/// 2.515 m ahead of the origin. A class that names only its springs has no
/// hull body whatever its primitives.
#[test]
fn bf2_fighter_tank_hull_is_p_cube1() {
    use core::vehicle::hover::{HullBody, HullShape};
    let dir = game::assets::baked_root().join("side_bf2/vehicles/imp");
    let Ok(text) = std::fs::read_to_string(dir.join("vehicles.ron")) else {
        eprintln!("SKIP: no baked {}", dir.display());
        return;
    };
    let vm: manifest::VehicleManifest = ron::from_str(&text).expect("vehicles.ron");
    let mut db = ClassDb::default();
    if !load_from(&mut db, "bf2", "SIDE/IMP.LVL", "hhulimp") {
        return;
    }
    let prims = |geometry: &str| {
        &vm.collision
            .iter()
            .find(|e| e.geometry.eq_ignore_ascii_case(geometry))
            .expect(geometry)
            .prims
    };
    let tank = HoverClass::from_db(&db, "imp_hover_fightertank").unwrap();
    let hull = tank.hull(prims("imp_hover_fightertank"));
    eprintln!("{hull:#?}");
    let [HullBody {
        shape: HullShape::Box { half },
        pos,
        rot,
        terrain: false,
        building: true,
    }] = hull[..]
    else {
        panic!("one box, buildings only: {hull:?}");
    };
    assert!((half - glam::Vec3::new(1.5108, 0.4913, 0.9555)).length() < 1e-3);
    assert!((pos - glam::Vec3::new(0.0, 0.5106, 1.5593)).length() < 1e-3);
    assert!(rot.abs_diff_eq(glam::Mat3::IDENTITY, 1e-5), "{rot:?}");
    assert!((pos.z + half.z - 2.5149).abs() < 1e-3, "the front face");
    let bike = HoverClass::from_db(&db, "imp_hover_speederbike").unwrap();
    assert!(!prims("imp_hover_speederbike").is_empty());
    assert!(bike.hull(prims("imp_hover_speederbike")).is_empty());
}

/// A BF1 vehicle's slice time is its BF2 counterpart's own
/// `TimeRequiredToEject`, and a BF1 class with no row has the same default
/// as BF2's class of that name.
#[test]
fn bf1_slice_times_match_the_bf2_twins() {
    use game::vehicle_class::{slice_time, BF1_SLICE_TIMES};
    let dir = game::assets::baked_root().join("classes");
    if !dir.join("bf1_all.ron").is_file() || !dir.join("bf2_all.ron").is_file() {
        eprintln!("SKIP: class bakes absent");
        return;
    }
    let [bf1, bf2] = ["bf1", "bf2"]
        .map(|g| game::roster::load_classdb(&dir, g, &game::roster::FACTION_ORDER).unwrap());
    for &(class, twin, time) in BF1_SLICE_TIMES {
        assert!(bf1.get(class).is_some(), "BF1 has {class}");
        assert_eq!(bf1.prop(class, "TimeRequiredToEject"), None, "{class}");
        assert_eq!(
            bf2.prop_f32(twin, "TimeRequiredToEject"),
            Some(time),
            "BF2 {twin}"
        );
        assert_eq!(slice_time(&bf1, class), time, "{class}");
        assert_eq!(slice_time(&bf2, twin), time, "{twin}");
    }
    // A class both games name alike is sliced alike, table or default.
    let mut same = 0;
    let mut names: Vec<&str> = bf1.names().collect();
    names.sort_unstable();
    for class in names {
        let sliced = slice_time(&bf1, class);
        if sliced == 0.0 || bf2.get(class).is_none() || slice_time(&bf2, class) == 0.0 {
            continue;
        }
        assert_eq!(sliced, slice_time(&bf2, class), "{class}");
        same += 1;
    }
    assert!(same >= 12, "{same} same-named vehicles compared");
    // Command vehicles are never sliced on either.
    for class in ["imp_walk_atat", "rep_walk_atte", "cis_hover_mtt"] {
        assert_eq!(slice_time(&bf1, class), 0.0, "{class}");
    }
}

/// The Technician award repairs a BF1 flyer at its BF2 twin's
/// `PilotSkillRepairScale`, and every ground vehicle of both games at the
/// 0.005 default (neither game authors one there).
#[test]
fn bf1_repair_scales_match_the_bf2_twins() {
    use game::vehicle_class::{repair_scale, BF1_REPAIR_SCALES};
    let dir = game::assets::baked_root().join("classes");
    if !dir.join("bf1_all.ron").is_file() || !dir.join("bf2_all.ron").is_file() {
        eprintln!("SKIP: class bakes absent");
        return;
    }
    let [bf1, bf2] = ["bf1", "bf2"]
        .map(|g| game::roster::load_classdb(&dir, g, &game::roster::FACTION_ORDER).unwrap());
    let twin = |class: &str| match class {
        "all_fly_snowspeeder" | "rep_fly_gunship" => class.to_owned(),
        "cis_fly_geofighter" => "cis_fly_droidfighter_sc".to_owned(),
        _ => format!("{class}_sc"),
    };
    for &(class, scale) in BF1_REPAIR_SCALES {
        assert!(bf1.get(class).is_some(), "BF1 has {class}");
        assert_eq!(repair_scale(&bf1, class), scale, "{class}");
        assert_eq!(
            repair_scale(&bf2, &twin(class)),
            scale,
            "BF2 {}",
            twin(class)
        );
    }
    for class in ["imp_walk_atst", "cis_hover_aat", "rep_hover_fightertank"] {
        assert_eq!(repair_scale(&bf1, class), 0.005, "{class}");
        assert_eq!(repair_scale(&bf2, class), 0.005, "BF2 {class}");
    }
}

/// The weak spot's factor on a ground map, per class: x4 for every AT-TE,
/// AT-AT and spider walker class of both games (BF2's Kashyyyk spider too),
/// x2 for every other vehicle with a weak spot; the class chain decides, so
/// a map's own child of a big walker is one.
#[test]
fn heavy_walkers_take_x4_on_their_weak_spots() {
    use game::vehicle_class::{weak_spot_factor, HEAVY_WALKERS};
    let dir = game::assets::baked_root().join("classes");
    if !dir.join("bf1_all.ron").is_file() || !dir.join("bf2_all.ron").is_file() {
        eprintln!("SKIP: class bakes absent");
        return;
    }
    let [bf1, bf2] = ["bf1", "bf2"]
        .map(|g| game::roster::load_classdb(&dir, g, &game::roster::FACTION_ORDER).unwrap());
    let heavy = core::sim::HEAVY_WALKER_WEAK_SPOT_FACTOR;
    let plain = core::sim::GROUND_WEAK_SPOT_FACTOR;
    assert_eq!((heavy, plain), (4.0, 2.0));
    for (db, game) in [(&bf1, "BF1"), (&bf2, "BF2")] {
        for class in ["rep_walk_atte", "imp_walk_atat", "cis_walk_spider"] {
            assert!(db.get(class).is_some(), "{game} has {class}");
            assert_eq!(weak_spot_factor(db, class), heavy, "{game} {class}");
        }
        for class in [
            "imp_walk_atst",
            "imp_walk_atst_snow",
            "imp_walk_atst_jungle",
            "cis_hover_aat",
            "cis_tread_hailfire",
            "rep_hover_fightertank",
            "imp_hover_fightertank",
            "all_hover_combatspeeder",
        ] {
            assert!(db.get(class).is_some(), "{game} has {class}");
            assert_eq!(weak_spot_factor(db, class), plain, "{game} {class}");
        }
    }
    assert!(bf2.get("cis_walk_spiderkas").is_some());
    assert_eq!(weak_spot_factor(&bf2, "cis_walk_spiderkas"), heavy);
    for class in ["rep_walk_oneman_atst", "cis_tread_snailtank"] {
        assert_eq!(weak_spot_factor(&bf2, class), plain, "BF2 {class}");
    }
    assert_eq!(HEAVY_WALKERS.len(), 4);
    // A child class of a big walker, as a map's own would be.
    let tmp = std::env::temp_dir().join(format!("openbf-weak-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("classes.ron");
    std::fs::write(
        &file,
        r#"[
    (name: "kas2_walk_spider_local", kind: "entc", base: "cis_walk_spider", props: []),
    (name: "kas2_walk_atst_local", kind: "entc", base: "imp_walk_atst", props: []),
]"#,
    )
    .unwrap();
    let mut db = game::roster::load_classdb(&dir, "bf2", &game::roster::FACTION_ORDER).unwrap();
    db.load(&file).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(weak_spot_factor(&db, "kas2_walk_spider_local"), heavy);
    assert_eq!(weak_spot_factor(&db, "kas2_walk_atst_local"), plain);
}

/// Every side hover's `AddSpringBody` spheres in its hull, each as
/// `(radius, soldier, vehicle, ordnance)`, by class (`analysis/re-hover.md`
/// §13.8), from the baked side vehicles and their class files.
fn spring_hulls(game: &str) -> Vec<(String, Vec<(f32, bool, bool, bool)>)> {
    use core::collision::{Layers, Shape};
    let root = game::assets::baked_root();
    let mut out = Vec::new();
    let mut db = ClassDb::default();
    let Ok(classes) = std::fs::read_dir(root.join("classes")) else {
        return out;
    };
    let mut files: Vec<_> = classes
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{game}_")))
        })
        .collect();
    files.sort();
    for f in &files {
        db.load(f).unwrap();
    }
    let Ok(sides) = std::fs::read_dir(root.join(format!("side_{game}")).join("vehicles")) else {
        return out;
    };
    let mut sides: Vec<_> = sides.flatten().map(|e| e.path()).collect();
    sides.sort();
    for dir in sides {
        let Ok(text) = std::fs::read_to_string(dir.join("vehicles.ron")) else {
            continue;
        };
        let vm: manifest::VehicleManifest = ron::from_str(&text).unwrap();
        for v in &vm.vehicles {
            let k = &v.class;
            let Some(entry) = k.geometry.as_deref().and_then(|g| {
                vm.collision
                    .iter()
                    .find(|e| e.geometry.eq_ignore_ascii_case(g))
            }) else {
                continue;
            };
            let cg = game::assets::load_collision_geom(&dir, entry).unwrap();
            let parts = game::assets::spring_hull_parts(&db, &k.name, &cg, &k.collision);
            let spheres = HoverClass::from_db(&db, &k.name)
                .map(|h| {
                    let subsets = h.spring_subsets(
                        &cg.mesh
                            .iter()
                            .map(|_| "collisionmesh".to_owned())
                            .chain(cg.prims.iter().map(|p| p.name.to_ascii_lowercase()))
                            .collect::<Vec<_>>(),
                    );
                    h.springs()
                        .iter()
                        .zip(subsets)
                        .map(|(s, [a, b, c])| (s.radius, a, b, c))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let built: Vec<_> = parts
                .iter()
                .map(|p| match p.shape {
                    Shape::Sphere { radius } => (
                        radius,
                        p.layers.contains(Layers::SOLDIER),
                        p.layers.contains(Layers::VEHICLE),
                        p.layers.contains(Layers::ORDNANCE),
                    ),
                    _ => panic!("a spring part is a sphere"),
                })
                .collect();
            let kept: Vec<_> = spheres
                .iter()
                .copied()
                .filter(|s| s.1 || s.2 || s.3)
                .collect();
            assert_eq!(built, kept, "{}", k.name);
            if !spheres.is_empty() {
                out.push((k.name.clone(), spheres));
            }
        }
    }
    out
}

/// The spheres a hover keeps in its soldier, vehicle and ordnance subsets
/// are the ones PCSX2 RAM holds (`analysis/re-hover.md` §13.8: BF2 slot 8
/// and the `water_fidelity` state on Kashyyyk, the `yavin-slot10` state, BF1
/// `bf1_geo1_sbd_reference`). A class whose collision lines come after its
/// springs drops them; one with none keeps all three.
#[test]
fn hover_spring_spheres_join_the_subsets_ram_shows() {
    let ram: [(&str, &str, &[f32], bool); 8] = [
        ("bf2", "rep_hover_barcspeeder", &[1.4; 3], true),
        ("bf2", "all_hover_combatspeeder", &[1.75; 6], true),
        (
            "bf2",
            "cis_tread_snailtank",
            &[2.0, 2.0, 1.0, 1.0, 1.0, 0.75],
            true,
        ),
        ("bf2", "rep_hover_fightertank", &[2.3; 4], false),
        ("bf2", "imp_hover_fightertank", &[2.3; 4], false),
        ("bf2", "imp_hover_speederbike", &[1.4; 3], false),
        ("bf2", "cis_hover_stap", &[3.0; 3], false),
        ("bf1", "cis_tread_hailfire", &[4.0, 3.5, 3.5], false),
    ];
    for game in ["bf2", "bf1"] {
        let all = spring_hulls(game);
        if all.is_empty() {
            eprintln!("SKIP: no baked {game} side vehicles");
            continue;
        }
        for (g, class, radii, kept) in ram.iter().filter(|r| r.0 == game) {
            let (_, spheres) = all
                .iter()
                .find(|(n, _)| n == class)
                .unwrap_or_else(|| panic!("{g} {class} has springs"));
            let got: Vec<f32> = spheres.iter().map(|s| s.0).collect();
            assert_eq!(got, radii.to_vec(), "{class}");
            for s in spheres {
                assert_eq!((s.1, s.2, s.3), (*kept, *kept, *kept), "{class}");
            }
        }
    }
}

/// re-hover §13.12: on the PS2's KAS2 the guided-rocket ordnance that loads
/// after the BARC writes its `DamageEffectScale 1` into the BARC's last slot
/// (its 20 % flame), and the CIS vehicles loaded later keep their own (read
/// from a KAS2 save state).
#[test]
fn kas2_load_order_writes_the_rocket_scale_into_the_barcs_last_flame() {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let dir = root.join("assets_baked/classes");
    let mut db = ClassDb::default();
    for side in ["bf2_rep.ron", "bf2_cis.ron", "bf2_ingame.ron"] {
        let p = dir.join(side);
        if !p.exists() || !dir.join("load_order").join(side).exists() {
            eprintln!("SKIP: {} or its load order not present", p.display());
            return;
        }
        db.load(&p).expect("load");
    }
    let mission = game::mission::MissionDef::load(&root.join("missions/bf2_kas2_con_cw.ron"))
        .expect("mission");
    let bands = |db: &ClassDb, class: &str| -> Vec<(String, f32, f32)> {
        damage_effects(db, class)
            .iter()
            .map(|d| (d.effect.clone(), d.scale, d.inherit_velocity))
            .collect()
    };
    let barc_alone = bands(&db, "rep_hover_barcspeeder");
    assert_eq!(
        barc_alone[4],
        ("com_sfx_vehicleflame".to_owned(), 0.2, 0.75)
    );
    let stap = bands(&db, "cis_hover_stap");
    let snail = bands(&db, "cis_tread_snailtank");
    db.replay_damage_loads(&dir, "bf2", &mission.class_load_order);
    let barc = bands(&db, "rep_hover_barcspeeder");
    assert_eq!(barc[4], ("com_sfx_vehicleflame".to_owned(), 1.0, 0.75));
    assert_eq!(barc[..4], barc_alone[..4]);
    assert_eq!(bands(&db, "cis_hover_stap"), stap);
    assert_eq!(bands(&db, "cis_tread_snailtank"), snail);
}
