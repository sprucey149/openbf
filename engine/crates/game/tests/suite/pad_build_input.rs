//! Issue #79 through a player's own input. The maintainer's report: a CIS
//! engineer on BF2 Coruscant, cutter up, at the pad `tur_bldg_laser8`
//! (-50.0, 7.03, -12.9), could not build it. The match log's build (745938ba)
//! was compiled before #79 landed (7912871d): that build's loader kept no
//! turret for a pad, so the cutter had no target and only the static model was
//! there. These runs keep that spot and every other pad buildable.
//!
//! The PS2's rule (`analysis/re-build-zones.md` 1-2): a pad is team 0, neutral
//! to every side and tied to no command post. A cutter takes it inside its
//! acquisition (4 m + radius, the cone) and adds BuildingBuild, 50, per 0.5 s.
//!
//! In the walk-up runs the engineer starts from 7 m out, energy full; everything
//! after that is stick, mouse look and buttons. `sim.cheats.invincible` is the
//! only cheat and the energy bar is real. Bots and posts run only where a test
//! says so. Without baked maps these skip, as `squad_vehicle_maps` does.

use core::sim::{buttons, Event, PadState, Sim};
use core::unit::{Controller, UnitHandle};
use core::vehicle::VehicleKind;
use glam::Vec3;

struct World {
    sim: Sim,
    me: UnitHandle,
    cutter: usize,
}

/// Whether `map` is baked here; a public checkout has no game data.
fn baked(map: &str) -> bool {
    let there = game::assets::baked_root()
        .join(map)
        .join("manifest.ron")
        .exists();
    if !there {
        eprintln!("SKIP: {map} is not baked");
    }
    there
}

fn world(map: &str, sides: [&str; 2], engineer: &str, cutter: &str) -> World {
    let root = game::assets::baked_root();
    let dir = root.join(map);
    let (scene, _) = game::assets::load_battle_scene(&dir, "con").expect("real map data required");
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game("bf2");
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let f = sides.map(str::to_owned);
    game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
        .expect("install");
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &sides).unwrap();
    for t in &mut sim.battle.teams {
        t.reinforcements = 200.0;
    }
    sim.cheats.invincible = true;
    let mut names = Vec::new();
    let roster = game::roster::ClassRoster::build(&db, engineer, 0, None).unwrap();
    let rc = roster.register(&mut sim, &mut names);
    let me = sim
        .spawn(&rc.spec(
            0,
            Vec3::new(0.0, 500.0, 0.0),
            0.0,
            Controller::Player { slot: 0 },
            0,
            0,
        ))
        .unwrap();
    let cutter = sim
        .weapon_classes
        .iter()
        .position(|k| k.name.eq_ignore_ascii_case(cutter))
        .expect("the side's cutter");
    let mut w = World { sim, me, cutter };
    for _ in 0..8 {
        if w.sim.unit(me).unwrap().weapon_class == w.cutter {
            break;
        }
        press(&mut w, buttons::SWITCH_WEAPON, 1);
        press(&mut w, 0, 30);
    }
    assert_eq!(w.sim.unit(me).unwrap().weapon_class, w.cutter, "{map}");
    w
}

fn press(w: &mut World, b: u32, ticks: usize) {
    for _ in 0..ticks {
        w.sim.step(&[PadState {
            buttons: b,
            ..Default::default()
        }]);
    }
}

/// One tick: the mouse turns the view at `at`, `walk` pushes the stick
/// forward, `b` are the held buttons.
fn aimed(w: &mut World, at: Vec3, walk: bool, b: u32) {
    let u = w.sim.unit(w.me).unwrap();
    let d = at - u.muzzle();
    let (yaw, pitch) = ((-d.x).atan2(-d.z), (d.y / d.length()).asin());
    let wrap = |a: f32| {
        (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    };
    let (dy, dp) = (wrap(yaw - u.soldier.yaw), pitch - u.soldier.pitch);
    w.sim.step(&[PadState {
        ly: if walk { i16::MAX } else { 0 },
        buttons: b | buttons::MOUSE_AIM,
        mx: PadState::quantize_look(f64::from(-dy)),
        my: PadState::quantize_look(f64::from(dp)),
        ..Default::default()
    }]);
}

/// From 7 m out on side `k`, walk with the stick to 3.6 m.
fn approach(w: &mut World, bi: usize, k: usize) {
    let centre = w.sim.building_turrets[bi].sphere().0;
    let a = k as f32 * std::f32::consts::TAU / 8.0;
    let mut p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * 7.0;
    // The pad's own floor: `spawn_ground` can answer another storey. A pad
    // sunk under the terrain is walked to on the terrain above it.
    let terrain = w
        .sim
        .heightfield
        .as_ref()
        .map_or(f32::MIN, |h| h.height_at(p.x, p.z));
    p.y = w.sim.building_turrets[bi].spec.pos.y.max(terrain) + 0.1;
    let u = w.sim.unit_mut(w.me).unwrap();
    u.soldier.pos = p;
    u.soldier.vel = Vec3::ZERO;
    u.soldier.energy = u.consts.energy_bar;
    press(w, 0, 10);
    for _ in 0..120 {
        let u = w.sim.unit(w.me).unwrap();
        if (u.soldier.pos - centre).with_y(0.0).length() <= 3.6 {
            break;
        }
        aimed(w, centre, true, 0);
    }
}

/// Walk up on side `k` of 8 (3.6 m is inside the cutter's reach, and short of
/// the floor's edge at Coruscant's `tur_bldg_laser13`, which hangs over the
/// end of the library's floor) and hold the trigger: every repair amount until
/// the turret stands or `limit` ticks pass.
fn walk_and_hold(w: &mut World, bi: usize, k: usize, limit: u32) -> Vec<(u64, f32)> {
    let centre = w.sim.building_turrets[bi].sphere().0;
    approach(w, bi, k);
    let mut out = Vec::new();
    for _ in 0..limit {
        aimed(w, centre, false, buttons::FIRE);
        for e in &w.sim.events {
            if let Event::Repair { amount, .. } = e {
                out.push((w.sim.tick, *amount));
            }
        }
        if w.sim.building_turrets[bi].down.is_none() {
            break;
        }
    }
    out
}

/// A side of the pad's ring the walk completes the build from.
fn build_pad(w: &mut World, bi: usize, what: &str) -> usize {
    for k in 0..8 {
        let before = w.sim.building_turrets[bi].down;
        let shots = walk_and_hold(w, bi, k, 900);
        if w.sim.building_turrets[bi].down.is_none() {
            assert!(shots.iter().all(|s| s.1 == 50.0), "{what}: {shots:?}");
            if before == Some(0.0) {
                let n = (w.sim.building_turrets[bi].spec.max_health / 50.0).ceil() as usize;
                assert_eq!(shots.len(), n, "{what}: one 50 per shot from 0");
                assert_eq!(
                    shots[n - 1].0 - shots[0].0,
                    15 * (n as u64 - 1),
                    "{what}: 0.5 s apart"
                );
            }
            return k;
        }
    }
    panic!("{what}: no side of the pad's ring builds it");
}

fn standing(w: &World, bi: usize) -> core::vehicle::VehicleHandle {
    let h = w.sim.building_turrets[bi].handle;
    let v = w.sim.vehicle(h).expect("built turret");
    assert!(v.alive && v.kind == VehicleKind::Turret);
    h
}

/// The maintainer's own place and heading (his match log, ticks 900-1050; the
/// log has no pitch, so this looks down at the pad), trigger held: the same
/// pad, the same sixteen shots. Both sides, for the team rule: a pad is neutral
/// to either.
#[test]
fn coruscant_pad_builds_from_the_reported_stance_for_either_side() {
    if !baked("bf2_cor1") {
        return;
    }
    for (sides, engineer, cutter) in [
        (
            ["cis", "rep"],
            "cis_inf_engineer",
            "cis_weap_inf_fusioncutter",
        ),
        (
            ["rep", "cis"],
            "rep_inf_ep3_engineer",
            "rep_weap_inf_fusioncutter",
        ),
    ] {
        let mut w = world("bf2_cor1", sides, engineer, cutter);
        let bi = w
            .sim
            .building_turrets
            .iter()
            .position(|t| t.spec.pos.distance(Vec3::new(-50.003, 7.029, -12.905)) < 0.01)
            .expect("the reported pad is a building turret");
        assert_eq!(w.sim.building_turrets[bi].down, Some(0.0), "starts unbuilt");
        let u = w.sim.unit_mut(w.me).unwrap();
        u.soldier.pos = Vec3::new(-51.8, 7.03, -12.1);
        u.soldier.vel = Vec3::ZERO;
        u.soldier.yaw = -1.43;
        u.soldier.pitch = -0.48;
        press(&mut w, 0, 2);
        let i = w.sim.units.iter().position(|u| u.is_player).unwrap();
        assert!(w.sim.cutter_target(i).is_some(), "{engineer}: pad acquired");
        let mut shots = Vec::new();
        for _ in 0..300 {
            press(&mut w, buttons::FIRE, 1);
            for e in &w.sim.events {
                if let Event::Repair { amount, .. } = e {
                    shots.push((w.sim.tick, *amount));
                }
            }
            if w.sim.building_turrets[bi].down.is_none() {
                break;
            }
        }
        assert_eq!(shots.len(), 16, "{engineer}: {shots:?}");
        assert!(shots.iter().all(|s| s.1 == 50.0));
        standing(&w, bi);
        // The turret rose around him: he can still walk away.
        let from = w.sim.unit(w.me).unwrap().soldier.pos;
        for _ in 0..60 {
            w.sim.step(&[PadState {
                ly: i16::MIN + 1,
                ..Default::default()
            }]);
        }
        let moved = w.sim.unit(w.me).unwrap().soldier.pos.distance(from);
        assert!(
            moved > 2.0,
            "{engineer}: stuck in the built turret, {moved} m"
        );
    }
}

/// Coruscant's pads in Conquest, the report's moment: the Republic has just
/// taken the post nearest the pad, by standing on it, and the CIS engineer
/// stands where he stood. The sim's capture path runs, and the pad does not
/// care. Then the view: which looks from there take the pad.
#[test]
fn coruscant_pad_builds_just_after_the_republic_captures_the_nearest_post() {
    if !baked("bf2_cor1") {
        return;
    }
    let (map, sides) = ("bf2_cor1", ["cis", "rep"]);
    let mut w = world(map, sides, "cis_inf_engineer", "cis_weap_inf_fusioncutter");
    let root = game::assets::baked_root();
    let mut scene = game::assets::load_scene(&root.join(map)).unwrap();
    scene.apply_mode_layer("con");
    let mission = game::mission::MissionDef::load(
        &root.parent().unwrap().join("missions/bf2_cor1_con_cw.ron"),
    )
    .unwrap();
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    layout.apply_mission(&mission);
    let pad = Vec3::new(-50.003, 7.029, -12.905);
    let bi = w
        .sim
        .building_turrets
        .iter()
        .position(|t| t.spec.pos.distance(pad) < 0.01)
        .unwrap();
    for cp in &layout.cps {
        let (capture_time, neutralize_time) = core::cp::CpDef::class_times(cp.major);
        let owner = match cp.team {
            1 => Some(0),
            2 => Some(1),
            _ => None,
        };
        w.sim.add_cp(
            core::cp::CpDef {
                name: cp.name.clone(),
                pos: cp.pos,
                capture: cp
                    .capture
                    .unwrap_or_else(|| core::region::Region::sphere(cp.pos, 8.0)),
                control: cp.control,
                value_bleed: cp.value_bleed,
                value_atk: [0, 0],
                value_def: [0, 0],
                spawn_nodes: Vec::new(),
                capture_time,
                neutralize_time,
            },
            owner,
        );
    }
    let post = (0..w.sim.cps.len())
        .min_by(|&a, &b| {
            let d = |i: usize| w.sim.cps[i].def.pos.distance(pad);
            d(a).total_cmp(&d(b))
        })
        .expect("Coruscant's posts");
    println!(
        "nearest post {} at {} m, owner {:?}",
        w.sim.cps[post].def.name,
        w.sim.cps[post].def.pos.distance(pad),
        w.sim.cps[post].owner
    );
    w.sim.cps[post].owner = Some(0);
    // The engineer waits at the pad; a Republic soldier stands on the post.
    let spot = w.sim.cps[post].def.capture.center;
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &sides).unwrap();
    let roster = game::roster::ClassRoster::build(&db, "rep_inf_ep3_rifleman", 1, None).unwrap();
    let mut names = Vec::new();
    let rc = roster.register(&mut w.sim, &mut names);
    let rep = w
        .sim
        .spawn(&rc.spec(1, spot, 0.0, Controller::Ai, 1, 0))
        .unwrap();
    let mut captured = false;
    for _ in 0..30 * 90 {
        let u = w.sim.unit_mut(rep).unwrap();
        u.soldier.pos = spot;
        u.soldier.vel = Vec3::ZERO;
        let m = w.sim.unit_mut(w.me).unwrap();
        m.soldier.pos = Vec3::new(-51.8, 7.03, -12.1);
        m.soldier.vel = Vec3::ZERO;
        press(&mut w, 0, 1);
        captured = w
            .sim
            .events
            .iter()
            .any(|e| matches!(e, Event::Captured { team: 1, .. }));
        if captured {
            break;
        }
    }
    assert!(captured, "the Republic takes the post");
    assert_eq!(w.sim.cps[post].owner, Some(1));
    assert_eq!(w.sim.building_turrets[bi].down, Some(0.0), "still a pad");

    let i = w.sim.units.iter().position(|u| u.is_player).unwrap();
    let u = w.sim.unit_mut(w.me).unwrap();
    u.soldier.pos = Vec3::new(-51.8, 7.03, -12.1);
    u.soldier.vel = Vec3::ZERO;
    u.soldier.yaw = -1.43;
    u.soldier.energy = u.consts.energy_bar;
    let mut taken = Vec::new();
    for tenth in -12..=6 {
        let pitch = tenth as f32 / 10.0;
        w.sim.unit_mut(w.me).unwrap().soldier.pitch = pitch;
        press(&mut w, 0, 1);
        if w.sim.cutter_target(i).is_some() {
            taken.push(pitch);
        }
    }
    println!("looks that take the pad from the stance: {taken:?}");
    w.sim.unit_mut(w.me).unwrap().soldier.pitch = -0.48;
    let mut shots = Vec::new();
    for _ in 0..300 {
        press(&mut w, buttons::FIRE, 1);
        for e in &w.sim.events {
            if let Event::Repair { amount, .. } = e {
                shots.push(*amount);
            }
        }
        if w.sim.building_turrets[bi].down.is_none() {
            break;
        }
    }
    assert_eq!(shots, [50.0; 16], "the pad builds under a Republic post");
    standing(&w, bi);
}

/// Every turret of a building class on the nine BF2 ground maps that have any,
/// both eras on Naboo: built
/// by walking up with the trigger held. On the first pad of each map: the built
/// turret seats him and fires, takes the cutter when damaged, falls back to its
/// pad, and builds again.
#[test]
fn every_bf2_pad_builds_works_repairs_and_rebuilds_through_normal_input() {
    type Run = (&'static str, [&'static str; 2], &'static str, &'static str);
    const RUNS: [Run; 10] = [
        (
            "bf2_cor1",
            ["cis", "rep"],
            "cis_inf_engineer",
            "cis_weap_inf_fusioncutter",
        ),
        (
            "bf2_nab2",
            ["rep", "cis"],
            "rep_inf_ep3_engineer",
            "rep_weap_inf_fusioncutter",
        ),
        (
            "bf2_nab2",
            ["all", "imp"],
            "imp_inf_engineer",
            "imp_weap_inf_fusioncutter",
        ),
        (
            "bf2_kam1",
            ["rep", "cis"],
            "rep_inf_ep2_engineer",
            "rep_weap_inf_fusioncutter",
        ),
        (
            "bf2_yav1",
            ["all", "imp"],
            "all_inf_engineer",
            "all_weap_inf_fusioncutter",
        ),
        (
            "bf2_end1",
            ["all", "imp"],
            "imp_inf_engineer",
            "imp_weap_inf_fusioncutter",
        ),
        (
            "bf2_kas2",
            ["all", "imp"],
            "all_inf_engineer",
            "all_weap_inf_fusioncutter",
        ),
        (
            "bf2_geo1",
            ["cis", "rep"],
            "cis_inf_engineer",
            "cis_weap_inf_fusioncutter",
        ),
        (
            "bf2_myg1",
            ["rep", "cis"],
            "rep_inf_ep3_engineer",
            "rep_weap_inf_fusioncutter",
        ),
        (
            "bf2_hot1",
            ["all", "imp"],
            "all_inf_engineer",
            "all_weap_inf_fusioncutter",
        ),
    ];
    for (map, sides, engineer, cutter) in RUNS {
        if baked(map) {
            pads_work_through_input(map, sides, engineer, cutter);
        }
    }
}

fn pads_work_through_input(map: &str, sides: [&str; 2], engineer: &str, cutter: &str) {
    let mut w = world(map, sides, engineer, cutter);
    let pads: Vec<_> = (0..w.sim.building_turrets.len())
        .filter(|&b| w.sim.building_turrets[b].down.is_some())
        .collect();
    // A map whose turrets stand at the start (Kamino's self-built guns):
    // bring them down, so the same run builds their fallen looks.
    let pads = if pads.is_empty() {
        for b in 0..w.sim.building_turrets.len() {
            let h = w.sim.building_turrets[b].handle;
            w.sim.destroy_vehicle(h, None);
        }
        (0..w.sim.building_turrets.len()).collect()
    } else {
        pads
    };
    assert!(!pads.is_empty(), "{map}: no turrets");
    println!(
        "{map} {sides:?}: {} pads of {} turrets",
        pads.len(),
        w.sim.building_turrets.len()
    );
    let mut first_side = 0;
    for &bi in &pads {
        let t = &w.sim.building_turrets[bi];
        let what = format!("{map} {engineer} pad at {}", t.spec.pos);
        let k = build_pad(&mut w, bi, &what);
        println!("  {what}: built from side {k}");
        if bi == pads[0] {
            first_side = k;
        }
    }
    let bi = pads[0];
    let h = standing(&w, bi);

    // In: the use press, as a player does at the built turret, from the
    // first side a gunner can reach it (Hoth's dish has a wall on some).
    let centre = w.sim.building_turrets[bi].sphere().0;
    let mut seated = false;
    for k in (0..8).map(|k| (k + first_side) % 8) {
        approach(&mut w, bi, k);
        for _ in 0..60 {
            aimed(&mut w, centre, false, 0);
            press(&mut w, buttons::ACTION, 1);
            press(&mut w, 0, 1);
            seated = w.sim.unit(w.me).unwrap().mount.is_some();
            if seated {
                break;
            }
        }
        if seated {
            first_side = k;
            break;
        }
    }
    assert!(seated, "{map}: the built turret takes a gunner");
    // A bolt or Kashyyyk's beam while the trigger is down, or on release for
    // the charge of Hoth's dish.
    let fired = (0..150).any(|t| {
        press(&mut w, if t < 120 { buttons::FIRE } else { 0 }, 1);
        !w.sim.bolts.is_empty() || !w.sim.beams.is_empty()
    });
    assert!(fired, "{map}: the built turret fires");
    press(&mut w, 0, 5);
    press(&mut w, buttons::ACTION, 1);
    press(&mut w, 0, 30);
    assert!(
        w.sim.unit(w.me).unwrap().mount.is_none(),
        "{map}: out again"
    );
    // A gunner's turret keeps his team a moment; let it go back to empty.
    press(&mut w, 0, 60);
    approach(&mut w, bi, first_side);

    // Damaged: +100 a shot (BuildingHealth), to full.
    let v = w.sim.vehicle_mut(h).unwrap();
    v.health.cur = v.health.max - 250.0;
    let mut gains = Vec::new();
    for _ in 0..240 {
        aimed(&mut w, centre, false, buttons::FIRE);
        for e in &w.sim.events {
            if let Event::Repair { amount, .. } = e {
                gains.push(*amount);
            }
        }
        if gains.len() == 3 {
            break;
        }
    }
    assert_eq!(
        gains,
        [100.0, 100.0, 50.0],
        "{map}: repair of a damaged turret"
    );
    assert_eq!(
        w.sim.vehicle(h).unwrap().health.cur,
        w.sim.vehicle(h).unwrap().health.max
    );

    // Shot down: back to its pad, and the cutter builds it again.
    press(&mut w, 0, 20);
    w.sim.destroy_vehicle(h, None);
    assert_eq!(
        w.sim.building_turrets[bi].down,
        Some(0.0),
        "{map}: back to its pad"
    );
    build_pad(&mut w, bi, &format!("{map} rebuilt"));
    standing(&w, bi);
}

/// Kashyyyk's `com_weap_gunturret10` stands 2 m under the terrain: the ground
/// above it is the nearest a soldier gets. Its sphere still comes up through
/// the ground. The LOS ray stops that radius short of the centre on the PS2
/// (`re-aim-assist.md` 3.3), so a player's cutter and a bot's both reach it.
#[test]
fn kashyyyk_pad_under_the_terrain_builds_from_the_ground_above_it() {
    if !baked("bf2_kas2") {
        return;
    }
    let sides = ["all", "imp"];
    let cutter = "all_weap_inf_fusioncutter";
    let pad = Vec3::new(128.479, -0.988, 109.973);
    let find = |w: &World| {
        w.sim
            .building_turrets
            .iter()
            .position(|t| t.spec.pos.distance(pad) < 0.01)
            .expect("Kashyyyk's sunk pad")
    };
    let mut w = world("bf2_kas2", sides, "all_inf_engineer", cutter);
    let bi = find(&w);
    let ground = w.sim.ground_under(pad + Vec3::Y * 5.0, false);
    let (centre, r) = w.sim.building_turrets[bi].sphere();
    assert!(centre.y + 1.0 < ground && centre.y + r > ground, "sunk");
    assert_eq!(w.sim.building_turrets[bi].down, Some(0.0));
    build_pad(&mut w, bi, "the sunk pad");
    standing(&w, bi);

    // A bot, with every other pad done, as nothing else needs him.
    let mut w = world("bf2_kas2", sides, "all_inf_engineer", cutter);
    let bi = find(&w);
    for j in 0..w.sim.building_turrets.len() {
        if j != bi {
            w.sim.repair_building(j as u16, f32::MAX);
        }
    }
    let root = game::assets::baked_root();
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &sides).unwrap();
    let mut names = Vec::new();
    let rc = game::roster::ClassRoster::build(&db, "all_inf_engineer", 0, None)
        .unwrap()
        .register(&mut w.sim, &mut names);
    let mut at = pad + Vec3::new(0.0, 0.0, -8.0);
    at.y = w.sim.spawn_ground(at);
    w.sim
        .spawn(&rc.spec(0, at, 0.0, Controller::Ai, 0, 0))
        .unwrap();
    for _ in 0..60 * core::TICK_HZ {
        w.sim.step(&[PadState::default()]);
        if w.sim.building_turrets[bi].down.is_none() {
            break;
        }
    }
    assert_eq!(w.sim.building_turrets[bi].down, None, "a bot builds it");
}

/// The report's battle in the game's own order (`main/battle.rs`: posts, mission
/// AI, pads, the opening wave) with sixteen bots a side: the player stands at
/// the pad for as long as the match log has him there, then holds the trigger.
#[test]
fn coruscant_pad_builds_in_a_full_battle_with_bots() {
    use game::walk::{ClassVisual, TeamTemplate, WalkSim};
    if !baked("bf2_cor1") {
        return;
    }
    let root = game::assets::baked_root();
    let dir = root.join("bf2_cor1");
    let (loaded, _) = game::assets::load_battle_scene(&dir, "con").unwrap();
    let mission = game::mission::MissionDef::load(
        &root.parent().unwrap().join("missions/bf2_cor1_con_cw.ron"),
    )
    .unwrap();
    let factions = mission.factions();
    assert_eq!(factions[0], "cis", "the log's team 0");
    let (mut rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        &loaded.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        Some(&mission),
    )
    .unwrap();
    // The player spawns as class 0: make that the engineer.
    let e = rosters[0]
        .classes
        .iter()
        .position(|c| c.class.eq_ignore_ascii_case("cis_inf_engineer"))
        .expect("the CIS roster has the engineer");
    rosters[0].classes.swap(0, e);
    let mut sim = Sim::empty(45);
    sim.ground_rules = core::soldier::GroundRules::for_game(&loaded.manifest.source_game);
    sim.battle = core::battle::Battle::new(mission.tickets());
    sim.cheats.invincible = true;
    let mut names = Vec::new();
    let mut team = 0;
    let teams = rosters.map(|r| {
        let table: Vec<_> = r
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
        let template = TeamTemplate::from_roster(
            &r,
            &table[0],
            r.classes.iter().map(ClassVisual::from_roster).collect(),
        );
        sim.classes[team] = table;
        team += 1;
        template
    });
    let mut nav = game::layout::nav_graph(&loaded.manifest, &loaded.dir);
    mission.apply_arc_blocks(nav.as_mut());
    mission.apply_barriers(nav.as_mut());
    let mut walk = WalkSim::with_teams(sim, loaded.heightfield(), Vec3::ZERO, teams, nav);
    loaded.install_collision(&mut walk.sim);
    loaded.install_water(&mut walk.sim);
    let mut layout = game::layout::MapLayout::from_manifest(&loaded.manifest);
    walk.sim.death_regions = layout.apply_mission(&mission).death_regions;
    walk.add_posts(&layout);
    mission.apply_ai(&mut walk.sim);
    game::vehicle_pads::install(
        &mut walk.sim,
        &loaded.manifest,
        &dir,
        &root.join("classes"),
        &factions,
    )
    .unwrap();
    walk.opening_wave([16, 16], false);
    let place = |walk: &mut WalkSim| {
        let u = walk.sim.unit_mut(walk.player).unwrap();
        u.soldier.pos = Vec3::new(-51.8, 7.03, -12.1);
        u.soldier.vel = Vec3::ZERO;
        u.soldier.yaw = -1.43;
        u.soldier.pitch = -0.48;
    };
    place(&mut walk);
    let cutter = walk
        .sim
        .weapon_classes
        .iter()
        .position(|k| k.name.eq_ignore_ascii_case("cis_weap_inf_fusioncutter"))
        .unwrap();
    for _ in 0..8 {
        if walk.sim.unit(walk.player).unwrap().weapon_class == cutter {
            break;
        }
        walk.step_tick(&[PadState {
            buttons: buttons::SWITCH_WEAPON,
            ..Default::default()
        }]);
        for _ in 0..30 {
            walk.step_tick(&[PadState::default()]);
        }
    }
    assert_eq!(walk.sim.unit(walk.player).unwrap().weapon_class, cutter);
    let pad = Vec3::new(-50.003, 7.029, -12.905);
    let bi = walk
        .sim
        .building_turrets
        .iter()
        .position(|t| t.spec.pos.distance(pad) < 0.01)
        .unwrap();
    for _ in 0..525 {
        place(&mut walk);
        walk.step_tick(&[PadState::default()]);
    }
    assert_eq!(walk.sim.building_turrets[bi].down, Some(0.0), "still a pad");
    let mut mine = 0;
    for _ in 0..300 {
        walk.step_tick(&[PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        mine += walk
            .sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::Repair { unit, .. } if *unit == walk.player))
            .count();
        if walk.sim.building_turrets[bi].down.is_none() {
            break;
        }
    }
    assert_eq!(walk.sim.building_turrets[bi].down, None);
    assert_eq!(mine, 16, "his sixteen shots");
}
