//! Card VRIDE: a vehicle shows its riders (`analysis/re-vehicle-riders.md`
//! [O]; `game::vehicle_class::{SeatSpec::gunner_draw, RiderHang}`,
//! `game::viewer::turret_pose::HoverNine`, `game::soldier_vis`).
//!
//! 1. Data-gated, through the BAKED class dbs: every vehicle seat's verdict
//!    (drawn / hidden / no soldier) equals the PS2 census of both discs
//!    (`tests/data/vehicle_rider_census.txt`, re-vehicle-riders 1).
//! 2. Data-gated, through the baked side bakes: where the riders hang,
//!    against the PS2's RAM (re-vehicle-riders 3): the speeder bike's rider
//!    at the bike's own matrix, BF1's Jedi starfighter pilot at his pitched
//!    hardpoint, a fighter tank's gunner at the turret's rest, BF2's one-man
//!    AT-ST's rider on the walker skin's animated `bone_root`.
//! 3. The hover rider's numbers replayed over the PS2's own rows (two live
//!    speeder-bike rides, `tests/data/vehicle_rider_bike_<game>.txt`, one row
//!    per game update: the clock, the inputs, the drive's p and y and the
//!    seat's a / b / c; `lead_tmp/vride/ps2/b{1,2}bike_rows.json`).
//! 4. Data-gated, end to end through `SoldierVisuals`: a soldier seated on a
//!    speeder bike is drawn at the bike's hang, a hover tank's pilot is not.

use game::vehicle_class::{GunnerDraw, RiderHang, SeatKind};
use game::viewer::turret_pose as tp;
use glam::{Mat4, Vec3};

use super::class_dbs::class_dbs;

/// The census's seat label as the port's ring kind.
fn kind_of(label: &str) -> SeatKind {
    match label.strip_prefix("TURRET") {
        Some("") | Some("1") => SeatKind::Gunner(0),
        Some(n) => SeatKind::Gunner(n.parse::<u8>().unwrap() - 1),
        None => SeatKind::Pilot,
    }
}

fn word(d: GunnerDraw) -> &'static str {
    match d {
        GunnerDraw::Drawn => "DRAWN",
        GunnerDraw::Hidden => "HIDDEN",
        GunnerDraw::NoSoldier => "NONE",
    }
}

/// Card VRIDE acceptance 1: through the baked class dbs, every seat of
/// every vehicle class the census names is drawn, hidden or empty as the
/// PS2's census of the discs says (re-vehicle-riders 1: pilot seats BF1
/// 14 / 24 / 5, BF2 6 / 41 / 10 over every LVL). Mutation: hide every rider
/// -> the bike's pilot fails.
#[test]
fn the_rider_verdicts_are_the_ps2_census() {
    let census = include_str!("../data/vehicle_rider_census.txt");
    let mut ran = false;
    for game in ["bf1", "bf2"] {
        let dbs = class_dbs(game);
        if dbs.is_empty() {
            eprintln!("SKIP {game}: no baked maps");
            continue;
        }
        ran = true;
        let bf2 = game == "bf2";
        let rows: Vec<Vec<&str>> = census
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split_whitespace().collect::<Vec<_>>())
            .filter(|w| w[0] == game)
            .collect();
        let mut tally = std::collections::BTreeMap::<(bool, &str), (usize, usize)>::new();
        let mut absent = std::collections::BTreeSet::new();
        for w in &rows {
            let (class, seat, want) = (w[1], kind_of(w[2]), w[3]);
            let ring = dbs.iter().find_map(|(_, db)| {
                let name = db.resolve(class)?;
                Some(game::vehicle_class::seats(db, name))
            });
            let Some(ring) = ring else {
                absent.insert(class);
                continue;
            };
            let got = ring
                .iter()
                .find(|s| s.kind == seat)
                .unwrap_or_else(|| panic!("{game} {class}: no {seat:?} seat in {ring:?}"));
            let got = word(got.gunner_draw(bf2));
            let t = tally.entry((seat == SeatKind::Pilot, want)).or_default();
            t.0 += 1;
            t.1 += usize::from(got == want);
            assert_eq!(got, want, "{game} {class} {seat:?}");
        }
        println!(
            "{game}: {} census seats; in the bake ((pilot seat, verdict): found / matched) {tally:?}; \
             classes in no baked db: {absent:?}",
            rows.len()
        );
        assert!(
            tally.get(&(true, "DRAWN")).is_some_and(|t| t.0 >= 4)
                && tally.get(&(true, "HIDDEN")).is_some_and(|t| t.0 >= 4)
                && tally.get(&(false, "DRAWN")).is_some_and(|t| t.0 >= 2),
            "{game}: {tally:?}"
        );
    }
    if !ran {
        eprintln!("SKIP: no bake");
    }
}

/// Every baked vehicle class's `skel` nodes of `game`, by lowercase name:
/// each map's own and the side bakes (`vehicle_pads::hardpoints`).
fn nodes(game: &str) -> std::collections::HashMap<String, Vec<manifest::ManifestHardpoint>> {
    let root = game::assets::baked_root();
    let mut out = std::collections::HashMap::new();
    for (map, _) in class_dbs(game) {
        let dir = root.join(&map);
        let Ok(scene) = game::assets::load_scene(&dir) else {
            continue;
        };
        for (k, v) in game::vehicle_pads::hardpoints(&scene.manifest, &dir, game) {
            out.entry(k).or_insert(v);
        }
    }
    out
}

/// (game, class, seat, the PS2's position, its up axis).
type PlaceCase = (&'static str, &'static str, SeatKind, [f32; 3], [f32; 3]);

/// The hang of `class`'s `seat` through the baked db and nodes.
fn hang(
    dbs: &[(String, game::classdb::ClassDb)],
    nodes: &std::collections::HashMap<String, Vec<manifest::ManifestHardpoint>>,
    class: &str,
    seat: SeatKind,
    bf2: bool,
) -> Option<RiderHang> {
    let (db, name) = dbs
        .iter()
        .find_map(|(_, db)| Some((db, db.resolve(class)?)))?;
    let ring = game::vehicle_class::seats(db, name);
    let s = ring.iter().find(|s| s.kind == seat)?;
    assert_eq!(s.gunner_draw(bf2), GunnerDraw::Drawn, "{class} {seat:?}");
    let hp = nodes.get(&class.to_ascii_lowercase())?;
    Some(RiderHang::of(s, hp))
}

/// Card VRIDE acceptance 2: where a rider hangs in his vehicle's frame,
/// against the PS2's RAM (re-vehicle-riders 3, `R = M_soldier x
/// inverse(M_vehicle)`), within 1 cm: the bike's rider at the bike's own
/// matrix (+0.0002 m up), BF1's Jedi starfighter pilot at (0, 0.058,
/// -1.162) with up (0, 0.930, -0.368) - his hardpoint's ROTATION - and a
/// fighter tank's gunner at `hp_active`'s bind (0, 0.658, -3.010) at the
/// turret's rest. Mutation: drop the hardpoint's rotation -> the starfighter
/// pilot's up fails.
#[test]
fn a_rider_hangs_where_the_ps2_hangs_him() {
    let cases: [PlaceCase; 6] = [
        (
            "bf1",
            "imp_hover_speederbike",
            SeatKind::Pilot,
            [0.0, 0.0002, 0.0],
            [0.0, 1.0, 0.0],
        ),
        (
            "bf2",
            "imp_hover_speederbike",
            SeatKind::Pilot,
            [0.0, 0.0002, 0.0],
            [0.0, 1.0, 0.0],
        ),
        (
            "bf1",
            "rep_fly_jedifighter",
            SeatKind::Pilot,
            [0.0, 0.0580, -1.1617],
            [0.0, 0.930, -0.368],
        ),
        (
            "bf1",
            "rep_hover_fightertank",
            SeatKind::Gunner(0),
            [0.0, 0.6581, -3.010],
            [0.0, 1.0, 0.0],
        ),
        (
            "bf2",
            "rep_hover_fightertank",
            SeatKind::Gunner(0),
            [0.0, 0.6581, -3.010],
            [0.0, 1.0, 0.0],
        ),
        (
            "bf1",
            "cis_hover_stap",
            SeatKind::Pilot,
            [0.0, 0.360, 0.0],
            [0.0, 1.0, 0.0],
        ),
    ];
    let mut ran = 0;
    for game in ["bf1", "bf2"] {
        let dbs = class_dbs(game);
        let nodes = nodes(game);
        if !nodes.values().flatten().any(|h| h.axes.is_some()) {
            eprintln!("SKIP {game}: a bake from before card VRIDE (no node axes)");
            continue;
        }
        for (g, class, seat, pos, up) in cases.iter().filter(|c| c.0 == game) {
            let Some(h) = hang(&dbs, &nodes, class, *seat, *g == "bf2") else {
                eprintln!("SKIP {g} {class}: not baked");
                continue;
            };
            let RiderHang::Bind(m) = h else {
                panic!("{g} {class}: a hardpoint seat, got {h:?}");
            };
            let at = m.w_axis.truncate();
            let y = m.y_axis.truncate();
            println!("{g} {class} {seat:?}: at {at:.4}, up {y:.4}");
            assert!(
                at.distance(Vec3::from(*pos)) < 0.01,
                "{g} {class}: at {at}, the PS2 {pos:?}"
            );
            assert!(
                y.distance(Vec3::from(*up)) < 0.01,
                "{g} {class}: up {y}, the PS2 {up:?}"
            );
            ran += 1;
        }
    }
    if ran == 0 {
        eprintln!("SKIP: no side bake");
    } else if ran < cases.len() {
        eprintln!("{ran} of {} cases ran", cases.len());
    }
}

/// Card VRIDE acceptance 2, the animated node: BF2's one-man AT-ST rider
/// takes `bone_root` as the walker skin poses it now (`E x O`, no
/// hardpoint), not its bind (0, 2.942, -0.545), upright: on the PS2 two
/// walking walkers carried him tilted, up (0, 0.991, 0.136), at z -0.5445 /
/// -0.5446 (re-vehicle-riders 3). The port's walk clip tilts the node to
/// that up within 0.005 at the PS2's z; the bind's up is (0, 1, 0).
/// Mutation: hang him at the bind -> fails. Not refereed: the node's height
/// (the PS2's y 2.717 / 2.852 against the port's 2.915 - 2.942) and its
/// sideways sway (x 0.082 / -0.075 against +-0.029) - the walker skin is
/// drawn without its DummyRoot translation (`viewer::walker_pose`), [H].
#[test]
fn an_animated_rider_rides_the_walker_skins_node() {
    let root = game::assets::baked_root();
    let map = root.join("bf2_uta1");
    let Ok(scene) = game::assets::load_scene(&map) else {
        eprintln!("SKIP: bf2_uta1 not baked");
        return;
    };
    let dbs = class_dbs("bf2");
    let nodes = nodes("bf2");
    let Some(h) = hang(&dbs, &nodes, "rep_walk_oneman_atst", SeatKind::Pilot, true) else {
        eprintln!("SKIP: the one-man AT-ST is not baked");
        return;
    };
    let RiderHang::Node(name, bind) = h else {
        panic!("an animated seat, got {h:?}");
    };
    assert!(name.eq_ignore_ascii_case("bone_root"), "{name}");
    let b = bind.w_axis.truncate();
    assert!(b.distance(Vec3::new(0.0, 2.942, -0.545)) < 0.01, "bind {b}");
    let rigs = game::viewer::walker_rigs(
        &scene.manifest,
        &root,
        &[Some("rep_walk_oneman_atst".to_string())],
    );
    let Some(rig) = rigs[0].as_deref() else {
        eprintln!("SKIP: no walker skin baked");
        return;
    };
    let ps2_up = Vec3::new(0.0, 0.991, 0.136);
    let mut tilted = Vec::new();
    let mut ys = Vec::new();
    for anim in 0..16u8 {
        for k in 0..8 {
            let w = core::vehicle::walker::WalkerState {
                anim,
                clip_time: k as f32 * 0.25,
                ..Default::default()
            };
            let Some(n) = rig
                .skeleton
                .index_of(&name)
                .map(|n| rig.palette(&w, 1.0)[n] * rig.skeleton.global_bind[n])
            else {
                panic!("no {name} in the skin");
            };
            let at = n.w_axis.truncate();
            let up = n.y_axis.truncate().normalize();
            ys.push(at.y);
            if up.distance(ps2_up) < 0.005 && (at.z + 0.5445).abs() < 0.005 {
                tilted.push((anim, k, at));
            }
        }
    }
    let lo = ys.iter().copied().fold(f32::MAX, f32::min);
    let hi = ys.iter().copied().fold(f32::MIN, f32::max);
    println!(
        "bone_root: {} of {} states at the PS2's walking tilt, e.g. {:?}; y {lo:.4} .. {hi:.4} (bind {:.4})",
        tilted.len(),
        ys.len(),
        tilted.first(),
        b.y
    );
    assert!(
        !tilted.is_empty(),
        "the walk never tilts the rider as the PS2's"
    );
    assert!(
        bind.y_axis.truncate().normalize().distance(ps2_up) > 0.1,
        "the bind is upright: a bind hang cannot pass"
    );
}

/// One row of a live ride: (clock, [thr, str, trn], [p, y], [a, b, c]).
type BikeRow = (f64, [f32; 3], [f32; 2], [f32; 3]);

fn bike_rows(text: &str) -> Vec<BikeRow> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| {
            let f: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            let g = |i: usize| f[i] as f32;
            (f[0], [g(1), g(2), g(3)], [g(4), g(5)], [g(6), g(7), g(8)])
        })
        .collect()
}

/// Card VRIDE acceptance 3: the drive's smoothing replayed over the PS2's
/// own rows (each consecutive pair: the row's inputs, the clock step as
/// dt) lands within 2e-4 of the next row's p and y on at least 240 of the
/// 276 consecutive updates per game (re-vehicle-riders 4.1: BF1 244 with
/// the strafe-into-turn remix, BF2 258 with `trn + 0.5 str`), and a / b / c
/// follow from p and y on every row. Mutation: BF2's 0.5 -> 1.0 fails.
#[test]
fn the_bike_riders_numbers_are_the_ps2_drives() {
    for (game, text) in [
        ("bf1", include_str!("../data/vehicle_rider_bike_bf1.txt")),
        ("bf2", include_str!("../data/vehicle_rider_bike_bf2.txt")),
    ] {
        let bf2 = game == "bf2";
        let rows = bike_rows(text);
        let (mut fit, mut pairs) = (0, 0);
        for (r0, r1) in rows.iter().zip(&rows[1..]) {
            let n = tp::HoverNine {
                p: r1.2[0],
                y: r1.2[1],
            }
            .pose();
            assert!(
                (n.a - r1.3[0]).abs() < 1e-5
                    && (n.b - r1.3[1]).abs() < 1e-5
                    && (n.c - r1.3[2]).abs() < 1e-5,
                "{game}: a b c {n:?} against {:?}",
                r1.3
            );
            let dt = r1.0 - r0.0;
            if !(0.0..0.05).contains(&dt) || dt == 0.0 {
                continue;
            }
            let next = tp::HoverNine {
                p: r0.2[0],
                y: r0.2[1],
            }
            // The bike has no StrafeSpeed (asserted through the db below).
            .step(r1.1, true, bf2, dt as f32);
            pairs += 1;
            fit += usize::from((next.p - r1.2[0]).abs() < 2e-4 && (next.y - r1.2[1]).abs() < 2e-4);
        }
        println!("{game}: {fit} of {pairs} consecutive updates within 2e-4");
        assert!(pairs >= 270, "{game}: {pairs} pairs");
        assert!(fit >= 240, "{game}: {fit} of {pairs}");
    }
    // The remix's gate: the BF1 bike's class has no StrafeSpeed.
    let dbs = class_dbs("bf1");
    if let Some((db, name)) = dbs
        .iter()
        .find_map(|(_, db)| Some((db, db.resolve("imp_hover_speederbike")?)))
    {
        let h = game::vehicle_class::HoverClass::from_db(db, name).expect("a hover");
        assert!(
            h.params().strafe_speed < 1e-4,
            "{}",
            h.params().strafe_speed
        );
    }
}

/// Card VRIDE, end to end: on BF1 Endor a soldier seated on a speeder bike
/// is drawn at the bike's hang (`O x bind(hp_active)`); on BF2 Mygeeto a fighter
/// tank's pilot is not drawn and its gunner is, at the turret's rest; on
/// BF2 Mygeeto the one-man AT-ST's rider mid-walk is at `O x bone_root` as
/// the skin poses it, tilted. Mutations: hide every rider -> the bike
/// fails; an animated rider at the node's bind -> the AT-ST fails.
#[test]
fn a_seated_rider_is_drawn_on_his_vehicle() {
    // (map, factions, soldier, class, seat, drawn).
    let cases = [
        (
            "bf1_end1",
            ["all", "imp"],
            "imp_inf_stormtrooper",
            "imp_hover_speederbike",
            0u8,
            true,
        ),
        (
            "bf2_myg1",
            ["rep", "cis"],
            "rep_inf_ep3trooper",
            "rep_hover_fightertank",
            0,
            false,
        ),
        (
            "bf2_myg1",
            ["rep", "cis"],
            "rep_inf_ep3trooper",
            "rep_hover_fightertank",
            1,
            true,
        ),
        (
            "bf2_geo1",
            ["rep", "cis"],
            "rep_inf_trooper",
            "rep_walk_atte",
            1,
            true,
        ),
        (
            "bf2_fel1",
            ["rep", "cis"],
            "rep_inf_ep3trooper",
            "rep_walk_oneman_atst",
            0,
            true,
        ),
    ];
    let mut ran = 0;
    for (map, factions, soldier, class, seat, drawn) in cases {
        let game_of = &map[..3];
        let root = game::assets::baked_root();
        let dir = root.join(map);
        let glb = root.join(format!("side_{game_of}/soldiers/{soldier}.glb"));
        let Ok(mut scene) = game::assets::load_scene(&dir) else {
            eprintln!("SKIP {map}: not baked");
            continue;
        };
        if !glb.exists() {
            eprintln!("SKIP {map}: {soldier} not baked");
            continue;
        }
        scene.apply_mode_layer("con");
        let mut sim = core::sim::Sim::empty(1);
        scene.install_collision(&mut sim);
        let f = [factions[0].to_owned(), factions[1].to_owned()];
        let build =
            game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
                .expect("pads");
        let Some(ci) = build
            .classes
            .iter()
            .position(|c| c.eq_ignore_ascii_case(class))
        else {
            panic!("{map}: no {class} among {:?}", build.classes);
        };
        let spec = build
            .pads
            .iter()
            .flat_map(|p| p.classes.iter().flatten())
            .find(|c| c.spec.class as usize == ci)
            .expect("a pad of the class")
            .spec
            .clone();
        let mut t = game::walk::TeamTemplate::default_trooper();
        t.classes = vec![game::walk::ClassVisual {
            name: "rider".into(),
            label: "rider".into(),
            geometry: Some(soldier.into()),
            ..Default::default()
        }];
        let mut walk = game::walk::WalkSim::with_teams(sim, None, Vec3::ZERO, [t.clone(), t], None);
        walk.vehicle_seats = build.seats.clone();
        walk.vehicle_riders = build.riders.clone();
        walk.vehicle_geometry = build.geometry.clone();
        let h = walk.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
            pos: Vec3::new(10.0, 50.0, -20.0),
            yaw: 0.7,
            ..spec
        });
        // A walker mid-step (its walk clip tilts the body, the PS2's up).
        if let Some(v) = walk.sim.vehicle_mut(h) {
            v.walker.anim = 5;
            v.walker.clip_time = 0.5;
        }
        let player = walk.player;
        assert!(walk.sim.seat_unit(player, h, seat), "{map}: seat {seat}");
        if walk
            .sim
            .vehicle(h)
            .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Walker)
        {
            let n = walk.sim.vehicle(h).unwrap().seats.len();
            let setup = walk
                .sim
                .seat_setup
                .get(ci)
                .and_then(Option::as_ref)
                .cloned();
            let guns = (0..n)
                .map(|s| {
                    let g = setup.as_ref().and_then(|st| st.guns.get(s))?.as_ref()?;
                    let (look, rel, aim) = g.rest();
                    Some(core::vehicle::gun::GunState {
                        look,
                        aim,
                        rate: (0.0, 0.0),
                        rel,
                        weapons: g
                            .weapons
                            .iter()
                            .map(|w| {
                                core::weapon::WeaponState::new(
                                    &walk.sim.weapon_classes[w.class],
                                    true,
                                    w.reserve_clips,
                                )
                            })
                            .collect(),
                        zoom: None,
                    })
                })
                .collect();
            if let Some(v) = walk.sim.vehicle_mut(h) {
                if v.guns.is_empty() {
                    v.guns = guns;
                }
                for g in v.guns.iter_mut().flatten() {
                    g.look = (1.0, 0.1);
                    g.aim = g.look;
                }
            }
        }
        let mut vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, game_of, &walk.teams);
        let rigs = game::viewer::walker_rigs(&scene.manifest, &root, &walk.vehicle_geometry);
        walk.walker_rigs = rigs.clone();
        vis.set_walker_rigs(rigs.clone());
        for _ in 0..3 {
            walk.sim.tick += 1;
            vis.update(&walk, core::TICK_DT);
        }
        let mut blend = game::render_blend::Blend::default();
        blend.on = true;
        if walk.sim.vehicle(h).unwrap().kind == core::vehicle::VehicleKind::Walker {
            blend.before_tick(&walk);
            let v = walk.sim.vehicle_mut(h).unwrap();
            v.walker.clip_time += 0.033;
            v.walker.pitch += 0.04;
            for g in v.guns.iter_mut().flatten() {
                g.look.0 += 0.2;
                g.aim.0 += 0.2;
            }
            let hash = walk.sim.hash();
            assert!(blend.apply(&mut walk, 0.5));
            blend.restore(&mut walk);
            assert_eq!(walk.sim.hash(), hash);
            assert!(blend.apply(&mut walk, 0.5));
        }
        let v = walk.sim.vehicle(h).unwrap();
        let want = Mat4::from_rotation_translation(v.rot, v.pos)
            * match &build.riders[ci][usize::from(seat)] {
                RiderHang::Bind(b) => rigs[ci]
                    .as_deref()
                    .and_then(|r| {
                        let scale = walk.sim.walker_params[ci]
                            .as_ref()
                            .map_or(1.0, |p| p.root_scale);
                        r.node_now(
                            v,
                            scale,
                            build.seats[ci][usize::from(seat)]
                                .pilot_position
                                .as_deref()?,
                        )
                    })
                    .unwrap_or(*b),
                RiderHang::Node(name, b) => {
                    let scale = walk.sim.walker_params[ci]
                        .as_ref()
                        .map_or(1.0, |p| p.root_scale);
                    let n = rigs[ci]
                        .as_deref()
                        .and_then(|r| r.node_now(v, scale, name))
                        .expect("a posed walker skin");
                    assert!(!n.abs_diff_eq(*b, 0.01), "{map}: mid-step is off the bind");
                    n
                }
            };
        if v.kind == core::vehicle::VehicleKind::Walker {
            if let Some(rig) = rigs[ci].as_deref() {
                if let Some(name) = build.seats[ci][usize::from(seat)].fire_point.as_deref() {
                    let scale = walk.sim.walker_params[ci]
                        .as_ref()
                        .map_or(1.0, |p| p.root_scale);
                    let local = rig.node_now(v, scale, name).expect("walker muzzle");
                    let want = v.pos + v.rot * local.w_axis.truncate();
                    assert!(vis.fire_points(&walk)[player.index as usize]
                        .unwrap()
                        .abs_diff_eq(want, 1e-4));
                }
            }
        }
        let got = vis
            .drawn(&walk)
            .into_iter()
            .find(|d| d.0 == player.index as usize)
            .map(|d| d.1);
        println!(
            "{map} {class} seat {seat}: drawn at {:?}",
            got.map(|m| m.w_axis.truncate())
        );
        match (drawn, got) {
            (true, Some(m)) => {
                let d = (m - want)
                    .to_cols_array()
                    .iter()
                    .fold(0.0f32, |a, x| a.max(x.abs()));
                assert!(d < 1e-3, "{map} {class}: {m} against {want}");
            }
            (false, None) => {}
            (d, g) => panic!("{map} {class} seat {seat}: drawn {d}, got {g:?}"),
        }
        blend.restore(&mut walk);
        ran += 1;
    }
    if ran == 0 {
        eprintln!("SKIP: nothing baked");
    }
}
