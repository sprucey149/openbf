//! Card VTURG: an open building turret shows its gunner
//! (`analysis/re-turret-draw.md` 3 [O]; `game::vehicle_class::SeatSpec::gunner_draw`,
//! `game::viewer::turret_pose::{GunnerHang, nine_pose}`, `game::soldier_vis`).
//!
//! 1. The nine-pose numbers on a table of cases against the PS2's formula
//!    (B1:137688-137730, B2:189522-189566).
//! 2. Data-gated, through the BAKED class dbs: the gunner verdict of every
//!    armed-building class equals the PS2's census of both discs
//!    (`tests/data/turret_gunner_census.txt`), and per baked map the placed
//!    buildings by verdict.
//! 3. The PS2's own gunner (`tests/data/turret_gunner_ps2.txt`: the soldier's
//!    matrix in RAM beside the turret's and aimer's nodes, one row per two
//!    game updates, none filtered - `lead_tmp/vturg/tools/mk_gunner_fixture.py`)
//!    against `hardpoint x node` with the BAKED rig's hardpoint.
//! 4. Data-gated, through the baked maps, soldiers and `SoldierVisuals`: a
//!    seated gunner is drawn at his seat as the turret turns, posed by the
//!    seat's clip, with no hand weapon; the laser tower's is not drawn.

use game::vehicle_class::GunnerDraw;
use game::viewer::turret_pose as tp;
use glam::{Mat4, Vec3};

/// A matrix from 12 numbers: the X, Y, Z rows of the PS2's, then its row 3.
fn m12(f: &[f32]) -> Mat4 {
    let v = |i: usize| Vec3::new(f[3 * i], f[3 * i + 1], f[3 * i + 2]);
    Mat4::from_cols(
        v(0).extend(0.0),
        v(1).extend(0.0),
        v(2).extend(0.0),
        v(3).extend(1.0),
    )
}

/// (what, pitch down, yaw, stick, pitch limits, yaw limits, (a, b, c)).
type NineCase = (
    &'static str,
    f32,
    f32,
    f32,
    Option<[f32; 2]>,
    Option<[f32; 2]>,
    (f32, f32, f32),
);
/// (map, factions, soldier, class, the PS2's offset in the parent node's
/// frame, or None = hidden).
type SeatCase = (
    &'static str,
    [&'static str; 2],
    &'static str,
    &'static str,
    Option<[f32; 3]>,
);

fn close(a: tp::NinePose, b: (f32, f32, f32)) -> bool {
    (a.a - b.0).abs() < 1e-5 && (a.b - b.1).abs() < 1e-5 && (a.c - b.2).abs() < 1e-5
}

/// The numbers from (pitch, yaw, stick, limits), each case worked by hand
/// from the formula in `re-turret-draw.md` 3.3.
#[test]
fn the_nine_pose_numbers_are_the_ps2_formula() {
    let d = |x: f32| x.to_radians();
    let full = Some([-180.0, 180.0]);
    let gun = Some([-45.0, 45.0]);
    let barge = Some([-70.0, 70.0]);
    #[rustfmt::skip]
    let cases: [NineCase; 13] = [
        ("level, full circle, no stick", 0.0, 0.0, 0.0, gun, full, (4.0, 1.0, 0.0)),
        ("pitch at its upper limit (nose down)", d(45.0), 0.0, 0.0, gun, full, (4.0, 7.0, 1.0)),
        ("pitch at its lower limit (nose up)", d(-45.0), 0.0, 0.0, gun, full, (4.0, 1.0, 1.0)),
        ("pitch past its limit clamps", d(80.0), 0.0, 0.0, gun, full, (4.0, 7.0, 1.0)),
        ("a quarter down, the stick half right", d(22.5), 1.0, 0.5, gun, full, (4.5, 7.5, 0.5)),
        ("full circle, the stick full left", 0.0, 2.0, -1.0, gun, full, (3.0, 0.0, 0.0)),
        ("the stick is clamped", 0.0, 0.0, 3.0, gun, full, (5.0, 2.0, 0.0)),
        ("bounded yaw at its high end", 0.0, d(70.0), 0.9, gun, barge, (5.0, 2.0, 0.0)),
        ("bounded yaw at its low end, pitched down", d(45.0), d(-70.0), 0.9, gun, barge, (3.0, 6.0, 1.0)),
        ("bounded yaw half way", 0.0, d(35.0), 0.0, gun, barge, (4.5, 1.5, 0.0)),
        ("the dish's pitch range, its middle", d(12.5), 0.0, 0.0, Some([-10.0, 35.0]), full, (4.0, 1.0, 0.0)),
        ("no pitch limits: pf 0.5", d(30.0), 0.0, 0.0, None, full, (4.0, 7.0, 0.5)),
        ("pitch limits lo >= hi: pf 0.5", d(-30.0), 0.0, 0.0, Some([10.0, 10.0]), full, (4.0, 7.0, 0.5)),
    ];
    for (what, pitch, yaw, stick, pl, yl, want) in cases {
        let got = tp::nine_pose(pitch, yaw, stick, pl, yl);
        println!("{what}: {got:?}");
        assert!(close(got, want), "{what}: {got:?}, want {want:?}");
    }
    // A yaw range of 270 degrees or more is a full circle: the stick.
    let wide = tp::nine_pose(0.0, d(100.0), -0.5, gun, Some([-140.0, 140.0]));
    assert!(close(wide, (3.5, 0.5, 0.0)), "{wide:?}");
    let narrow = tp::nine_pose(0.0, d(100.0), -0.5, gun, Some([-120.0, 120.0]));
    assert!(narrow.a > 4.8, "{narrow:?}");
}

use super::class_dbs::class_dbs;

fn verdict(db: &game::classdb::ClassDb, class: &str, bf2: bool) -> Option<GunnerDraw> {
    let name = db.resolve(class)?;
    let seats = game::vehicle_class::seats(db, name);
    let gunner = seats
        .iter()
        .find(|s| s.kind == game::vehicle_class::SeatKind::Gunner(0))?;
    Some(gunner.gunner_draw(bf2))
}

/// Card VTURG acceptance 1: through the baked class dbs, every
/// armed-building class's gunner is drawn, hidden or absent as the PS2's
/// census of the discs says (BF1 6 / 1 / 2, BF2 15 / 7 / 47), and each map
/// lists its placed buildings by verdict. Mutation: draw every rider (the
/// verdict ignoring the pose key) fails at the laser tower.
#[test]
fn the_gunner_verdicts_are_the_ps2_census() {
    let census = include_str!("../data/turret_gunner_census.txt");
    let mut ran = false;
    for game in ["bf1", "bf2"] {
        let dbs = class_dbs(game);
        if dbs.is_empty() {
            eprintln!("SKIP {game}: no baked maps");
            continue;
        }
        ran = true;
        let bf2 = game == "bf2";
        let rows: Vec<(&str, &str)> = census
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split_whitespace().collect::<Vec<_>>())
            .filter(|w| w[0] == game)
            .map(|w| (w[1], w[2]))
            .collect();
        let mut tally = std::collections::BTreeMap::<&str, (usize, usize)>::new();
        let mut absent = Vec::new();
        for (class, want) in &rows {
            let Some(got) = dbs.iter().find_map(|(_, db)| verdict(db, class, bf2)) else {
                absent.push(*class);
                continue;
            };
            let got = match got {
                GunnerDraw::Drawn => "DRAWN",
                GunnerDraw::Hidden => "HIDDEN",
                GunnerDraw::NoSoldier => "NONE",
            };
            let t = tally.entry(want).or_default();
            t.0 += 1;
            if got == *want {
                t.1 += 1;
            }
            assert_eq!(got, *want, "{game} {class}");
        }
        println!(
            "{game}: {} census classes; in the bake (census verdict: matched / found) {tally:?}; not in any baked db: {absent:?}",
            rows.len()
        );
        // A class is absent from the bake only where no baked map places it
        // (the per-map walk below asserts that): space-map hulls, BF1's
        // fambaa (placed nowhere) and barge turret (an unbaked map). Each
        // verdict is reached, the hidden one at Hoth's laser tower.
        assert!(
            tally.get("DRAWN").is_some_and(|t| t.0 >= 5)
                && tally.get("HIDDEN").is_some_and(|t| t.0 >= 1),
            "{game}: {tally:?}"
        );
        let tower = if bf2 {
            "tur_bldg_hoth_lasermortar"
        } else {
            "hoth_bldg_hoth_turret"
        };
        assert_eq!(
            dbs.iter().find_map(|(_, db)| verdict(db, tower, bf2)),
            Some(GunnerDraw::Hidden),
            "{game}: the laser tower"
        );
        // Per map: the placed buildings by verdict.
        for (map, db) in &dbs {
            let text =
                std::fs::read_to_string(game::assets::baked_root().join(map).join("manifest.ron"))
                    .unwrap();
            let m: manifest::Manifest = ron::from_str(&text).unwrap();
            let insts = m
                .instances
                .iter()
                .chain(m.layers.iter().flat_map(|l| l.instances.iter()));
            let mut per = std::collections::BTreeMap::<String, (String, usize)>::new();
            for i in insts {
                let Some((class, want)) =
                    rows.iter().find(|(c, _)| c.eq_ignore_ascii_case(&i.class))
                else {
                    continue;
                };
                let got = verdict(db, class, bf2);
                assert!(got.is_some(), "{map}: {class} placed but not in the db");
                per.entry((*class).to_string())
                    .or_insert(((*want).to_string(), 0))
                    .1 += 1;
            }
            if !per.is_empty() {
                let line: Vec<String> = per
                    .iter()
                    .map(|(c, (v, n))| format!("{c} x{n} {v}"))
                    .collect();
                println!("  {map}: {}", line.join(", "));
            }
        }
    }
    if !ran {
        eprintln!("SKIP: no bake");
    }
}

/// The baked rig and the gunner seat of `class`.
fn rig_and_seat(
    class: &str,
) -> Option<(manifest::ManifestTurretRig, game::vehicle_class::SeatSpec)> {
    let root = game::assets::baked_root();
    for (map, db) in class_dbs("bf1").into_iter().chain(class_dbs("bf2")) {
        let Some(name) = db.resolve(class) else {
            continue;
        };
        let seat = game::vehicle_class::seats(&db, name).into_iter().next()?;
        let mut scene = game::assets::load_scene(&root.join(&map)).ok()?;
        let common = root.join(format!("common_{}", scene.manifest.source_game));
        if common.join("common.ron").exists() {
            scene.merge_common(game::assets::load_common(&common).ok()?);
        }
        if let Some(rig) = scene
            .resolve_class(class)
            .and_then(|c| c.turret_rig.clone())
        {
            return Some((rig, seat));
        }
    }
    None
}

/// Card VTURG acceptance 2, the PS2's half: on every row of three captures
/// (BF1 Naboo's gun turret and BF2 Hoth's dish: the turret-node seats; BF2
/// Mygeeto's anti-aircraft gun: an aimer-node seat) the soldier's matrix in
/// RAM is `L x node` with `L` from the BAKED rig's `PilotPosition`, within
/// 1 cm and 2e-3 on the axes - the aimer seat with the node of the update
/// before. Printed beside it: the same update's node, and the building's
/// origin (the hardpoint at its bind, no node).
#[test]
fn the_ps2_gunner_hangs_at_his_hardpoint_on_the_posed_node() {
    let data = include_str!("../data/turret_gunner_ps2.txt");
    let mut ran = 0;
    let mut current: Option<(String, Option<(tp::GunnerHang, bool)>)> = None;
    let mut tally = std::collections::BTreeMap::<String, [usize; 4]>::new();
    for l in data.lines().filter(|l| !l.starts_with('#')) {
        let w: Vec<&str> = l.split_whitespace().collect();
        if w[0] == "G" {
            let (tag, class, hp, kind) = (w[1], w[3], w[4], w[5]);
            let hang = rig_and_seat(class).map(|(rig, seat)| {
                assert_eq!(seat.pilot_position.as_deref(), Some(hp), "{tag}");
                let h = tp::GunnerHang::of(&rig, &seat).expect("the hardpoint is a rig node");
                let spec = tp::PoseSpec::of(&rig, &seat);
                let want = if kind == "aimer" {
                    spec.aimer
                } else {
                    spec.turret
                };
                assert_eq!(
                    h.parent, want,
                    "{tag}: the hardpoint's parent is the {kind} node"
                );
                (h, kind == "aimer")
            });
            if hang.is_none() {
                eprintln!("SKIP {tag}: {class} not baked with a rig");
            }
            current = Some((tag.to_string(), hang));
            continue;
        }
        let Some((tag, Some((hang, lags)))) = &current else {
            continue;
        };
        let f: Vec<f32> = w[3..].iter().map(|x| x.parse().unwrap()).collect();
        let (node, prev, sol) = (m12(&f[0..12]), m12(&f[12..24]), m12(&f[24..36]));
        let fits_m = |m: Mat4| {
            let dp = m.w_axis.truncate().distance(sol.w_axis.truncate());
            let dr = (0..12)
                .map(|i| (m.to_cols_array()[i] - sol.to_cols_array()[i]).abs())
                .fold(0.0, f32::max);
            dp < 0.01 && dr < 2e-3
        };
        // `GunnerHang::matrix` with the building at the identity and the
        // parent node at `e`.
        let fits = |e: Mat4| {
            let mut now = vec![Mat4::IDENTITY; hang.parent.map_or(0, |p| p + 1)];
            if let Some(p) = hang.parent {
                now[p] = e;
            }
            fits_m(hang.matrix(Mat4::IDENTITY, &now))
        };
        let t = tally.entry(tag.clone()).or_default();
        t[0] += 1;
        t[1] += usize::from(fits(if *lags { prev } else { node }));
        t[2] += usize::from(fits(if *lags { node } else { prev }));
        // Left at the building's origin.
        t[3] += usize::from(fits_m(Mat4::IDENTITY));
        ran += 1;
    }
    for (tag, [n, rule, other, origin]) in &tally {
        println!(
            "{tag}: {rule} of {n} rows fit L x node (the other update's node: {other}; no node: {origin})"
        );
        let lagged = tag == "myg1_lg";
        if lagged {
            // 334 of 341 rows in the RE (7 follow a sampling gap).
            assert!(*rule * 100 >= *n * 95, "{tag}: {rule} of {n}");
            assert!(other < rule, "{tag}: the update before fits better");
        } else {
            assert!(*rule * 100 >= *n * 99, "{tag}: {rule} of {n}");
        }
        assert!(
            *origin * 2 < *n,
            "{tag}: the building's origin must not fit"
        );
    }
    if ran == 0 {
        eprintln!("SKIP: no rigs baked");
    }
}

/// One map's battle world as `tests/suite/turret_pose.rs` builds it, in a
/// `WalkSim` whose two teams draw `geometry`.
struct World {
    scene: game::assets::Scene,
    walk: game::walk::WalkSim,
    build: game::vehicle_pads::PadBuild,
}

fn world(map: &str, factions: [&str; 2], geometry: &str) -> Option<World> {
    let dir = game::assets::baked_root().join(map);
    let mut scene = game::assets::load_scene(&dir).ok()?;
    scene.apply_mode_layer("con");
    let common = dir
        .parent()?
        .join(format!("common_{}", scene.manifest.source_game));
    if common.join("common.ron").exists() {
        scene.merge_common(game::assets::load_common(&common).ok()?);
    }
    let mut sim = core::sim::Sim::empty(1);
    scene.install_collision(&mut sim);
    let classes = game::assets::baked_root().join("classes");
    let f = [factions[0].to_owned(), factions[1].to_owned()];
    let build = game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &classes, &f).ok()?;
    let mut t = game::walk::TeamTemplate::default_trooper();
    t.classes = vec![game::walk::ClassVisual {
        name: "gunner".into(),
        label: "gunner".into(),
        geometry: Some(geometry.into()),
        ..Default::default()
    }];
    let mut walk = game::walk::WalkSim::with_teams(sim, None, Vec3::ZERO, [t.clone(), t], None);
    walk.vehicle_seats = build.seats.clone();
    Some(World { scene, walk, build })
}

fn deg(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d).to_degrees()
}

/// Card VTURG acceptance 2, the port's half, and the draw rule end to end:
/// on each map the player is seated in one turret of the class and a bot in
/// another; the turret's gun is set by hand (the turn law is card VTURL's)
/// and `SoldierVisuals` updated a tick at a time. A drawn class's gunner
/// stands at the hardpoint the PS2 hangs him at (the offset measured in RAM,
/// in the drawn node's frame) at three yaws 90 degrees apart, or two
/// pitches for the aimer-node seat, within 1 cm; the turret node's yaw
/// carries him round, he stays upright on a turret-node seat and tilts on
/// the aimer's; his pose is the seat clip's (the nine-pose at its pitch
/// ends is frame 7 / frame 1); he has no hand weapon. The laser tower's
/// gunner is not drawn. Mutations: draw every rider -> the tower fails;
/// the gunner at the building's origin -> the offset fails.
#[test]
fn a_seated_gunner_is_drawn_at_his_seat() {
    // (map, factions, soldier, class, the PS2's offset in the parent node's
    // frame - re-turret-draw.md 3.2 and lag_test.py - or None = hidden).
    let cases: [SeatCase; 4] = [
        (
            "bf2_hot1",
            ["all", "imp"],
            "all_inf_soldier",
            "tur_bldg_hoth_dishturret",
            Some([-0.003, -0.350, -1.880]),
        ),
        (
            "bf1_nab1",
            ["rep", "cis"],
            "rep_inf_trooper",
            "com_weap_gunturret",
            Some([0.0, -0.338, -0.152]),
        ),
        (
            "bf2_myg1",
            ["rep", "cis"],
            "rep_inf_trooper",
            "tur_bldg_recoilless_lg",
            Some([2.345, 2.501, -1.928]),
        ),
        (
            "bf2_hot1",
            ["all", "imp"],
            "all_inf_soldier",
            "tur_bldg_hoth_lasermortar",
            None,
        ),
    ];
    let mut ran = 0;
    for (map, factions, soldier, class, offset) in cases {
        let game_of = &map[..3];
        let root = game::assets::baked_root();
        let glb = root.join(format!("side_{game_of}/soldiers/{soldier}.glb"));
        let Some(mut w) = world(map, factions, soldier).filter(|_| glb.exists()) else {
            eprintln!("SKIP {map}: not baked");
            continue;
        };
        let anims = std::fs::read_to_string(
            root.join(format!("side_{game_of}/soldiers/{soldier}.anims.ron")),
        )
        .unwrap_or_default();
        if !anims.contains("human_minigun_9pose") {
            eprintln!("SKIP {map}: {soldier}'s bake has no seat clips (older than card VTURG)");
            continue;
        }
        let Some(ci) = w
            .build
            .classes
            .iter()
            .position(|c| c.eq_ignore_ascii_case(class))
        else {
            panic!("{map}: no {class} turret among {:?}", w.build.classes);
        };
        let turrets: Vec<core::vehicle::VehicleHandle> = w
            .walk
            .sim
            .alive_vehicles()
            .filter(|(_, v)| v.class as usize == ci)
            .map(|(h, _)| h)
            .collect();
        assert!(turrets.len() >= 2, "{map}: {} {class}", turrets.len());
        let seat = w.walk.sim.seat_setup[ci]
            .as_ref()
            .and_then(|s| s.guns.iter().position(Option::is_some))
            .expect("a gunner seat with a gun") as u8;
        let section = w.build.seats[ci][usize::from(seat)].clone();
        let rigged = game::viewer::turret_pose::rigged_turrets(&w.scene);
        let mut vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, game_of, &w.walk.teams);
        vis.set_turrets(rigged.clone());
        let player = w.walk.player;
        let bot = w
            .walk
            .sim
            .spawn(&core::unit::UnitSpec {
                controller: core::unit::Controller::Ai,
                ..core::unit::UnitSpec::trooper(1, Vec3::new(0.0, 500.0, 0.0))
            })
            .expect("a bot");
        assert!(w.walk.sim.seat_unit(player, turrets[0], seat));
        assert!(w.walk.sim.seat_unit(bot, turrets[1], seat));
        let setup = w.walk.sim.seat_setup[ci].as_ref().unwrap().guns[usize::from(seat)]
            .clone()
            .unwrap();
        let rest = setup.rest().0;
        // Set both seats' guns, then let the visuals see two ticks of it.
        let at_for = |w: &mut World,
                      vis: &mut game::soldier_vis::SoldierVisuals,
                      look: (f32, f32),
                      ticks: u32| {
            for &h in &turrets[..2] {
                let v = w.walk.sim.vehicle_mut(h).unwrap();
                if v.guns.len() <= usize::from(seat) {
                    v.guns.resize(usize::from(seat) + 1, None);
                }
                let g =
                    v.guns[usize::from(seat)].get_or_insert_with(|| core::vehicle::gun::GunState {
                        look,
                        aim: look,
                        rate: (0.0, 0.0),
                        rel: look,
                        weapons: Vec::new(),
                        zoom: None,
                    });
                g.look = look;
                g.aim = look;
            }
            for _ in 0..ticks {
                w.walk.sim.tick += 1;
                vis.update(&w.walk, core::TICK_DT);
            }
        };
        let at = |w: &mut World, vis: &mut game::soldier_vis::SoldierVisuals, look: (f32, f32)| {
            at_for(w, vis, look, 2)
        };
        let units = [player.index as usize, bot.index as usize];
        let Some(off) = offset else {
            at(&mut w, &mut vis, rest);
            let drawn: Vec<usize> = vis.drawn(&w.walk).iter().map(|d| d.0).collect();
            println!("{map} {class}: seated {units:?}, drawn {drawn:?}");
            assert!(
                units.iter().all(|u| !drawn.contains(u)),
                "{map} {class}: the gunner is hidden"
            );
            assert_eq!(section.gunner_draw(true), GunnerDraw::Hidden);
            ran += 1;
            continue;
        };
        let k = rigged
            .iter()
            .find(|t| t.class.eq_ignore_ascii_case(class))
            .expect("a rigged building");
        let spec = tp::PoseSpec::of(&k.rig, &section);
        let hang = tp::GunnerHang::of(&k.rig, &section).expect("a hang");
        let aimer_seat = hang.parent == spec.aimer;
        let d = std::f32::consts::FRAC_PI_2;
        let looks: Vec<(f32, f32)> = if aimer_seat {
            vec![(rest.0 + 0.3, -0.3), (rest.0 + 0.3, 0.25)]
        } else {
            vec![
                (rest.0, rest.1),
                (rest.0 + d, rest.1),
                (rest.0 + 2.0 * d, rest.1),
            ]
        };
        let clips = section.pilot_pose.clone().expect("a pose key");
        let mut ups: [Vec<Vec3>; 2] = Default::default();
        for look in looks {
            at(&mut w, &mut vis, look);
            let drawn = vis.drawn(&w.walk);
            for (n, &u) in units.iter().enumerate() {
                let v = w.walk.sim.vehicle(turrets[n]).unwrap();
                let td = rigged.iter().find(|t| t.pos == v.pos).unwrap();
                let m = drawn
                    .iter()
                    .find(|d| d.0 == u)
                    .unwrap_or_else(|| panic!("{map} {class}: unit {u} not drawn"))
                    .1;
                // The parent node drawn as the turret draws it.
                let now = tp::pose(&td.rig, &spec, look, look, &[]);
                let e = td.model * now[hang.parent.unwrap()];
                let p = e.inverse().transform_point3(m.w_axis.truncate());
                let miss = p.distance(Vec3::from(off));
                let up = m.y_axis.truncate().normalize();
                println!(
                    "{map} {class} unit {u} look ({:.2}, {:.2}): in the node's frame {p:.3} (PS2 {off:?}, miss {:.4} m); up {up:.3}; heading {:.1} deg off the node's",
                    look.0, look.1, miss,
                    deg(m.z_axis.x.atan2(m.z_axis.z), e.z_axis.x.atan2(e.z_axis.z))
                );
                assert!(miss < 0.01, "{map} {class}: {miss}");
                if aimer_seat {
                    // The hardpoint's own bind tilt rides on the aimer's.
                    let want = (e * hang.local).y_axis.truncate().normalize();
                    assert!(up.dot(want) > 0.9999, "tilts with the aimer");
                    ups[n].push(up);
                } else {
                    // Upright in the building's frame (a placed building
                    // may lean with the ground).
                    let b = td.model.y_axis.truncate().normalize();
                    assert!(up.dot(b) > 0.999, "upright: {up} against {b}");
                }
                assert!(deg(m.z_axis.x.atan2(m.z_axis.z), e.z_axis.x.atan2(e.z_axis.z)) < 6.0);
                assert!(vis.units[u].animator.seated(), "posed by the seat");
            }
            let points = vis.fire_points(&w.walk);
            assert!(units.iter().all(|&u| points[u].is_none()), "no hand weapon");
        }
        // The aimer seat's gunner hangs from the aimer node of the update
        // before (re-turret-draw.md 3.2): one tick after a turn he is at the
        // hardpoint of the old node, not of the new one.
        if aimer_seat {
            let (old, new) = ((rest.0 + 0.3, 0.25), (rest.0 - 0.2, -0.1));
            at(&mut w, &mut vis, old);
            at_for(&mut w, &mut vis, new, 1);
            let v = w.walk.sim.vehicle(turrets[0]).unwrap();
            let td = rigged.iter().find(|t| t.pos == v.pos).unwrap();
            let m = vis
                .drawn(&w.walk)
                .iter()
                .find(|d| d.0 == units[0])
                .unwrap()
                .1;
            let miss = |look: (f32, f32)| {
                let e = td.model * tp::pose(&td.rig, &spec, look, look, &[])[hang.parent.unwrap()];
                e.inverse()
                    .transform_point3(m.w_axis.truncate())
                    .distance(Vec3::from(off))
            };
            println!("{map} {class}: a tick after a turn, {:.4} m off the old node's hardpoint, {:.3} off the new one's", miss(old), miss(new));
            assert!(miss(old) < 0.01 && miss(new) > 0.1);
        }
        // The aimer seat's gunner tilts by the pitch between the two looks.
        for u in ups.iter().filter(|u| u.len() == 2) {
            let tilt = u[0].angle_between(u[1]);
            println!("{map} {class}: tilted {tilt:.3} rad between the two pitches (0.55 apart)");
            assert!((tilt - 0.55).abs() < 0.02, "{tilt}");
        }
        // The pose: the seat's clip; a nine-pose at its pitch ends is frame 7
        // (nose down) or frame 1 (nose up), frame 4 between.
        let sa = &vis.assets[0];
        let clip = game::animator::find_clip(&sa.asset.clips, &clips.clip())
            .unwrap_or_else(|| panic!("{soldier}: no {} baked", clips.clip()));
        let frames = sa.asset.clips[clip].frames;
        let _ = sa;
        println!("{map} {class}: clip {} ({frames} frames)", clips.clip());
        if clips.nine {
            assert_eq!(frames, 9);
            let pl = section.pitch_limits.expect("pitch limits");
            for (pitch_down, frame) in [(pl[1], 7.0), (pl[0], 1.0)] {
                at(&mut w, &mut vis, (rest.0, -pitch_down.to_radians()));
                at(&mut w, &mut vis, (rest.0, -pitch_down.to_radians()));
                let sa = &vis.assets[0];
                let want = sa.asset.clips[clip].sample(&sa.asset.skeleton, frame, false);
                let got = vis.units[units[0]].animator.pose(&sa.asset.skeleton);
                let worst_of = |want: &game::skin::Pose| {
                    want.0
                        .iter()
                        .zip(&got.0)
                        .map(|(a, b)| a.rotation.angle_between(b.rotation))
                        .fold(0.0, f32::max)
                };
                let worst = worst_of(&want);
                // The control: the level row's frame 4 is far from it - on
                // the minigun's clip. The dish's three rows are one pose on
                // the disc (frames 0-2 = 3-5 = 6-7, `dump_seat_clip_frames`):
                // its gunner leans with the stick only.
                let level = worst_of(&sa.asset.clips[clip].sample(&sa.asset.skeleton, 4.0, false));
                println!("  pitch {pitch_down} deg down: frame {frame}, worst bone {worst:.5} rad (frame 4: {level:.3})");
                if clips.name.contains("minigun") {
                    assert!(level > 0.5, "{map} {class}: frame {frame} is not frame 4");
                }
                assert!(worst < 4e-3, "{map} {class}: frame {frame}: {worst}");
            }
            // A full-circle seat's yaw number on a turn at the full rate
            // toward +yaw, level: BF2's is the rate over the speed, +1; BF1's
            // the raw stick, which the PS2 (and the port's `rx`) turns the
            // yaw against: -1. The pose is the nine-pose of that number and
            // not of the other sign.
            let yf = if game_of == "bf2" { 1.0 } else { -1.0 };
            at(&mut w, &mut vis, (rest.0, 0.0));
            at_for(
                &mut w,
                &mut vis,
                (rest.0 + setup.yaw_speed() * core::TICK_DT, 0.0),
                1,
            );
            let sa = &vis.assets[0];
            let got = vis.units[units[0]].animator.pose(&sa.asset.skeleton);
            let off = |yf: f32| {
                let n = tp::nine_pose(0.0, 0.0, yf, section.pitch_limits, section.yaw_limits);
                let c = &sa.asset.clips[clip];
                let want = c
                    .sample(&sa.asset.skeleton, n.a, false)
                    .blend(&c.sample(&sa.asset.skeleton, n.b, false), n.c);
                want.0
                    .iter()
                    .zip(&got.0)
                    .map(|(a, b)| a.rotation.angle_between(b.rotation))
                    .fold(0.0, f32::max)
            };
            println!(
                "  turning toward +yaw at the full rate ({game_of}: yaw number {yf:+}): {:.4} rad off it, {:.3} off the other sign's",
                off(yf),
                off(-yf)
            );
            assert!(off(yf) < 4e-3 && off(-yf) > 0.1);
        } else {
            assert_eq!(frames, 1);
        }
        ran += 1;
    }
    if ran == 0 {
        eprintln!("SKIP: nothing baked");
    }
}

/// The baked seat clips' frames against frame 4 and the bind pose, per
/// game and body (`--ignored --nocapture`): the card's "view the nine
/// frames first". Data-gated.
#[test]
#[ignore]
fn dump_seat_clip_frames() {
    let root = game::assets::baked_root();
    for (g, s) in [
        ("bf2", "all_inf_soldier"),
        ("bf1", "rep_inf_trooper"),
        ("bf2", "rep_inf_ep3trooper"),
    ] {
        let a = game::assets::load_skinned_glb(&root.join(format!("side_{g}/soldiers/{s}.glb")))
            .unwrap();
        for name in [
            "human_minigun_9pose",
            "human_dishcannon_9pose",
            "human_drive",
        ] {
            let k = game::animator::find_clip(&a.clips, name).unwrap();
            let c = &a.clips[k];
            let tracked = c.tracks.iter().filter(|t| t.is_some()).count();
            let bind = game::skin::Pose::bind(&a.skeleton);
            let f4 = c.sample(&a.skeleton, 4.0, false);
            let mut line = format!(
                "{g} {s} {name}: {} frames, {tracked} of {} bones tracked;",
                c.frames,
                a.skeleton.len()
            );
            for f in 0..c.frames {
                let p = c.sample(&a.skeleton, f as f32, false);
                let d = |q: &game::skin::Pose| {
                    p.0.iter()
                        .zip(&q.0)
                        .enumerate()
                        .map(|(b, (x, y))| (x.rotation.angle_between(y.rotation), b))
                        .fold((0.0f32, 0), |m, v| if v.0 > m.0 { v } else { m })
                };
                let (v4, b4) = d(&f4);
                let (vb, _) = d(&bind);
                line += &format!(
                    " f{f}: vs4 {v4:.3}@{} vsbind {vb:.2};",
                    a.skeleton.names[b4]
                );
            }
            println!("{line}");
        }
    }
}
