//! Card TBLD on real data (the maintainer 2026-10-28: "When a turret is destroyed, the
//! normal looking turrets should just show the little square on the ground
//! ... the hoth turrets or geonosis ... have destroyed looking looks ...
//! The fusion cutter should be able to heal turrets"). Per game, a plain
//! gun turret and a custom one:
//!
//! - destroyed, it is no vehicle; its instance's bodies are its fallen
//!   look's (`DestroyedGeometryName`'s collision, or none for the pad) and
//!   the view draws that look (`game::turret_fall::fallen_models`);
//! - the side's own fusion cutter heals the standing turret by
//!   `BuildingHealth` a shot and builds the fallen one by `BuildingRebuild`
//!   (wreck) / `BuildingBuild` (pad) a shot, one shot per 0.5 s (and nothing
//!   else: no regeneration, the maintainer 2026-10-28); one shot short of full it is
//!   still down, at full it stands again (intact, its own bodies, neutral).
//!
//! `analysis/re-build-zones.md` 1-2 [O][V]; disc amounts there.

use core::sim::{buttons, PadState, Sim, TurretFall};
use core::unit::{Controller, UnitHandle};
use core::vehicle::{VehicleHandle, VehicleKind};
use glam::Vec3;

struct Case {
    map: &'static str,
    factions: [&'static str; 2],
    class: &'static str,
    fall: TurretFall,
    model: &'static str,
    /// The first side's soldier who carries its cutter, and the cutter.
    engineer: &'static str,
    cutter: &'static str,
    /// BuildingHealth, and BuildingRebuild / BuildingBuild for the fall.
    heal: f32,
    build: f32,
}

const CASES: [Case; 5] = [
    Case {
        map: "bf1_yav2",
        factions: ["imp", "all"],
        class: "com_weap_gunturret",
        fall: TurretFall::Pad,
        model: "com_prop_buildzone",
        engineer: "imp_inf_pilottie",
        cutter: "imp_weap_inf_fusioncutter",
        heal: 60.0,
        build: 30.0,
    },
    Case {
        map: "bf1_hot1",
        factions: ["imp", "all"],
        class: "hoth_bldg_dish_turret",
        fall: TurretFall::Wreck,
        model: "hoth_bldg_dish_turret_destroyed",
        engineer: "imp_inf_pilottie",
        cutter: "imp_weap_inf_fusioncutter",
        heal: 60.0,
        build: 20.0,
    },
    Case {
        map: "bf2_kam1",
        factions: ["rep", "cis"],
        class: "tur_weap_built_gunturret",
        fall: TurretFall::Pad,
        model: "com_prop_buildzone",
        engineer: "rep_inf_ep2_engineer",
        cutter: "rep_weap_inf_fusioncutter",
        heal: 100.0,
        build: 50.0,
    },
    Case {
        map: "bf2_hot1",
        factions: ["imp", "all"],
        class: "tur_bldg_hoth_dishturret",
        fall: TurretFall::Wreck,
        model: "hoth_bldg_dish_turret_destroyed",
        engineer: "imp_inf_engineer_snow",
        cutter: "imp_weap_inf_fusioncutter",
        heal: 100.0,
        build: 50.0,
    },
    Case {
        map: "bf2_geo1",
        factions: ["rep", "cis"],
        class: "tur_bldg_geoturret",
        fall: TurretFall::Wreck,
        model: "geo_bldg_geoturret_dest",
        engineer: "rep_inf_ep2_engineer",
        cutter: "rep_weap_inf_fusioncutter",
        heal: 100.0,
        build: 50.0,
    },
];

struct World {
    scene: game::assets::Scene,
    sim: Sim,
    build: game::vehicle_pads::PadBuild,
    me: UnitHandle,
}

fn world(c: &Case) -> Option<World> {
    let root = game::assets::baked_root();
    let dir = root.join(c.map);
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: {} is not baked", c.map);
        return None;
    }
    let mut scene = game::assets::load_scene(&dir).expect("load scene");
    scene.apply_mode_layer("con");
    scene.merge_common(
        game::assets::load_common(&root.join(format!("common_{}", scene.manifest.source_game)))
            .expect("common assets"),
    );
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let f = c.factions.map(str::to_owned);
    let build =
        game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
            .expect("install");
    let game_name = scene.manifest.source_game.clone();
    let db = game::roster::load_classdb(&root.join("classes"), &game_name, &c.factions).unwrap();
    for t in &mut sim.battle.teams {
        t.reinforcements = 200.0;
    }
    sim.cheats.invincible = true;
    sim.cheats.infinite_stamina = true;
    let mut names = Vec::new();
    let roster = game::roster::ClassRoster::build(&db, c.engineer, 0, None).unwrap();
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
    // BF2's cutter is a primary (slot 2, channel 0): switch to it.
    let cutter = sim
        .weapon_classes
        .iter()
        .position(|k| k.name.eq_ignore_ascii_case(c.cutter));
    if game_name == "bf2" {
        for _ in 0..8 {
            if Some(sim.unit(me).unwrap().weapon_class) == cutter {
                break;
            }
            sim.step(&[PadState {
                buttons: buttons::SWITCH_WEAPON,
                ..Default::default()
            }]);
            for _ in 0..30 {
                sim.step(&[PadState::default()]);
            }
        }
        assert_eq!(
            Some(sim.unit(me).unwrap().weapon_class),
            cutter,
            "{}",
            c.map
        );
    }
    Some(World {
        scene,
        sim,
        build,
        me,
    })
}

/// The first building turret of the case's class: its index in
/// `Sim::building_turrets`.
fn first(w: &World, c: &Case) -> usize {
    let ci = w
        .build
        .classes
        .iter()
        .position(|k| k.eq_ignore_ascii_case(c.class))
        .unwrap_or_else(|| panic!("{}: no {} among {:?}", c.map, c.class, w.build.classes));
    w.sim
        .building_turrets
        .iter()
        .position(|t| t.spec.class as usize == ci)
        .expect("a building turret of the class")
}

fn standing(w: &World, i: usize) -> Option<VehicleHandle> {
    let h = w.sim.building_turrets[i].handle;
    w.sim
        .vehicle(h)
        .filter(|v| v.alive && v.kind == VehicleKind::Turret)
        .map(|_| h)
}

/// The instance's live bodies: (count, highest top).
fn bodies(w: &World, i: usize) -> (usize, f32) {
    let inst = w.sim.building_turrets[i].instance;
    let live: Vec<_> = w
        .sim
        .collision
        .bodies()
        .iter()
        .filter(|b| b.instance == Some(inst) && b.layers != core::collision::Layers::NONE)
        .collect();
    let top = live.iter().map(|b| b.aabb.max.y).fold(f32::MIN, f32::max);
    (live.len(), top)
}

#[test]
fn a_destroyed_turret_takes_its_wreck_or_pad_look_and_bodies_on_both_games() {
    for c in &CASES {
        let Some(mut w) = world(c) else {
            continue;
        };
        let i = first(&w, c);
        let t = &w.sim.building_turrets[i];
        assert_eq!(t.fall, c.fall, "{} {}", c.map, c.class);
        let inst = t.instance;
        let models = game::turret_fall::fallen_models(&w.scene).unwrap();
        assert_eq!(
            models.get(&(inst as usize)).cloned().flatten().as_deref(),
            Some(c.model),
            "{} {}: the fallen look's model is baked",
            c.map,
            c.class
        );
        let fallen_parts = t.fallen_parts.len();
        match c.fall {
            TurretFall::Wreck => assert!(fallen_parts > 0, "{}: the wreck's coll", c.map),
            _ => assert_eq!(fallen_parts, 0, "{}: the pad has no coll", c.map),
        }
        let up = bodies(&w, i);
        assert!(up.0 > 0, "{}: standing bodies", c.map);
        let h = standing(&w, i).unwrap();
        w.sim.destroy_vehicle(h, None);
        assert!(standing(&w, i).is_none());
        assert!(w.sim.building_down(inst));
        assert_eq!(w.sim.collision.turret(inst), None);
        let down = bodies(&w, i);
        println!(
            "{} {}: standing {} bodies, top {:.2}; fallen {:?} {} bodies, top {:.2}",
            c.map, c.class, up.0, up.1, c.fall, down.0, down.1
        );
        assert_eq!(down.0, fallen_parts, "{}: the fallen look's bodies", c.map);
        if down.0 > 0 {
            assert!(down.1 < up.1, "{}: the wreck stands lower", c.map);
        }
    }
}

/// Teleport the player beside the turret's sphere, facing its centre.
fn stand_by(w: &mut World, centre: Vec3, radius: f32, k: usize) {
    let a = k as f32 * std::f32::consts::TAU / 8.0;
    let mut p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * (radius + 1.0);
    p.y = w.sim.spawn_ground(p);
    let u = w.sim.unit_mut(w.me).unwrap();
    u.soldier.pos = p;
    u.soldier.vel = Vec3::ZERO;
    let d = centre - u.muzzle();
    let u = w.sim.unit_mut(w.me).unwrap();
    u.soldier.yaw = (-d.x).atan2(-d.z);
    u.soldier.pitch = (d.y / d.length()).asin();
}

/// The cutter's trigger: BF1's is the secondary (the tool's channel 1).
fn fire(w: &World) -> PadState {
    let bf1 = w.scene.manifest.source_game == "bf1";
    PadState {
        buttons: if bf1 {
            buttons::SECONDARY_FIRE
        } else {
            buttons::FIRE
        },
        ..Default::default()
    }
}

/// Hold the cutter from side `k` for one tick; the health it gave.
fn cut(w: &mut World, i: usize, k: usize) -> f32 {
    let (centre, radius) = w.sim.building_turrets[i].sphere();
    stand_by(w, centre, radius, k);
    let pad = fire(w);
    w.sim.step(&[pad]);
    w.sim
        .events
        .iter()
        .filter_map(|e| match e {
            core::sim::Event::Repair { unit, amount, .. } if *unit == w.me => Some(*amount),
            _ => None,
        })
        .sum()
}

/// A side the cutter reaches the turret from (the first shot lands).
fn reach(w: &mut World, i: usize) -> usize {
    for k in 0..8 {
        for _ in 0..20 {
            if cut(w, i, k) > 0.0 {
                return k;
            }
        }
    }
    panic!("the cutter reaches the turret from no side");
}

#[test]
fn the_fusion_cutter_heals_a_turret_and_builds_a_destroyed_one_at_the_disc_rates() {
    for c in &CASES {
        let Some(mut w) = world(c) else {
            continue;
        };
        let i = first(&w, c);
        // The side's own cutter, as the class carries it.
        let wc = w
            .sim
            .weapon_classes
            .iter()
            .find(|k| k.name.eq_ignore_ascii_case(c.cutter))
            .unwrap_or_else(|| panic!("{}: {} not registered", c.map, c.engineer));
        let repair = wc.repair.unwrap();
        assert_eq!(
            (repair.health[4], wc.shot_delay[0]),
            (c.heal, 0.5),
            "{}",
            c.map
        );
        // Standing and damaged: BuildingHealth a shot.
        let h = standing(&w, i).unwrap();
        let max = w.sim.vehicle(h).unwrap().health.max;
        w.sim.vehicle_mut(h).unwrap().health.cur = 1.0;
        let k = reach(&mut w, i);
        let v = w.sim.vehicle(h).unwrap();
        assert_eq!(v.health.cur, 1.0 + c.heal, "{}: one shot", c.map);
        assert!(!w.sim.building_down(w.sim.building_turrets[i].instance));
        // Destroyed: built shot by shot; still down one shot short.
        w.sim.destroy_vehicle(h, None);
        for _ in 0..20 {
            w.sim.step(&[PadState::default()]);
        }
        let mut shots = Vec::new();
        let mut last = w.sim.building_turrets[i].down.unwrap();
        let mut at = 0u64;
        loop {
            let gave = cut(&mut w, i, k);
            at += 1;
            assert!(at < 400 * 30, "{}: never built", c.map);
            let Some(now) = w.sim.building_turrets[i].down else {
                shots.push((at, gave));
                break;
            };
            if gave > 0.0 {
                assert!(
                    (gave - c.build).abs() < 0.01,
                    "{}: a build shot {gave}",
                    c.map
                );
                shots.push((at, gave));
            }
            assert!(
                (now - last - gave).abs() < 0.01,
                "{}: {last} -> {now} with {gave}",
                c.map
            );
            last = now;
            assert!(now < max, "{}: down below MaxHealth", c.map);
        }
        assert!(shots.windows(2).all(|p| p[1].0 - p[0].0 == 15), "{}", c.map);
        let need = (max / c.build).ceil() as usize;
        println!(
            "{} {}: {} HP built in {} shots of {}",
            c.map,
            c.class,
            max,
            shots.len(),
            c.build
        );
        assert!(shots.len() == need, "{}: {} shots", c.map, shots.len());
        let h = standing(&w, i).expect("built again");
        let v = w.sim.vehicle(h).unwrap();
        assert_eq!((v.health.cur, v.team), (max, None));
        assert_eq!(
            w.sim.collision.turret(w.sim.building_turrets[i].instance),
            Some(h)
        );
        assert_eq!(
            bodies(&w, i).0,
            w.sim.building_turrets[i].standing_parts.len()
        );
    }
}

/// The view (GPU): standing, every draw; down, its intact draws left out and
/// its fallen look's drawn at the instance; one cutter shot short of
/// MaxHealth still the fallen look (the PS2 builds only at MaxHealth,
/// `FUN_001e3458` B2:93483-93492); built, every draw again.
#[test]
fn the_view_draws_the_fallen_look_until_the_turret_is_built_again() {
    for c in [&CASES[0], &CASES[3]] {
        let Some(mut w) = world(c) else {
            continue;
        };
        let i = first(&w, c);
        let inst = w.sim.building_turrets[i].instance as usize;
        let mut r = render::Renderer::new_headless(64, 64).expect("headless renderer");
        let gpu = game::viewer::upload_scene(&mut r, &w.scene).expect("upload");
        let f = gpu
            .fallen
            .iter()
            .find(|f| f.instance == inst)
            .expect("the turret's draws are kept");
        let intact: usize = f.intact.iter().map(|r| r.len()).sum();
        assert!(intact > 0 && !f.fallen.is_empty(), "{}", c.map);
        let matrix = {
            let m = &w.scene.manifest.instances[inst];
            game::viewer::instance_matrix(m.rotation, m.translation)
        };
        assert!(
            f.fallen.iter().all(|d| d.model == matrix),
            "rigid at the instance"
        );
        let all = gpu.draws.len();
        let view = |w: &World| gpu.view_draws(&w.sim, w.me);
        assert_eq!(view(&w).len(), all, "{}: standing", c.map);
        let h = standing(&w, i).unwrap();
        w.sim.destroy_vehicle(h, None);
        let down = view(&w);
        assert_eq!(down.len(), all - intact + f.fallen.len(), "{}: down", c.map);
        let max = w.sim.building_turrets[i].spec.max_health;
        assert_eq!(w.sim.repair_building(i as u16, max - 1.0), max - 1.0);
        assert_eq!(
            view(&w).len(),
            down.len(),
            "{}: one short is still down",
            c.map
        );
        w.sim.repair_building(i as u16, 1.0);
        assert!(standing(&w, i).is_some());
        assert_eq!(view(&w).len(), all, "{}: built, intact", c.map);
    }
}

/// A bot of the case's engineer class, `d` m from turret `i`'s centre on
/// the first level side found (of 16).
fn engineer(w: &mut World, c: &Case, i: usize, d: f32, from: usize) -> UnitHandle {
    let root = game::assets::baked_root();
    let game_name = w.scene.manifest.source_game.clone();
    let db = game::roster::load_classdb(&root.join("classes"), &game_name, &c.factions).unwrap();
    let mut names = Vec::new();
    let rc = game::roster::ClassRoster::build(&db, c.engineer, 0, None)
        .unwrap()
        .register(&mut w.sim, &mut names);
    let (centre, _) = w.sim.building_turrets[i].sphere();
    let ground = w.sim.building_turrets[i].spec.pos.y;
    for k in (0..16).map(|k| (k + from) % 16) {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let mut p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * d;
        p.y = w.sim.spawn_ground(p);
        if (p.y - ground).abs() < 2.0 && w.sim.shut_in_solid(p + Vec3::Y).is_none() {
            return w
                .sim
                .spawn(&rc.spec(0, p, 0.0, Controller::Ai, 0, 0))
                .unwrap();
        }
    }
    panic!("{}: no level ground {d} m from the turret", c.map);
}

/// Who holds a claim on turret `i` now.
fn fixers(w: &World, i: usize) -> Vec<UnitHandle> {
    w.sim
        .alive_units()
        .filter(|(_, u)| {
            u.ai.fix
                .cur
                .is_some_and(|f| f.target == core::ai::fix::FixTarget::Turret(i as u16))
        })
        .map(|(h, _)| h)
        .collect()
}

/// The maintainer 2026-10-28: an engineer bot near a destroyed turret goes and builds
/// it, and only ONE of two takes it (`core::ai::fix`): the other's claim is
/// never on that turret. BF1 Yavin 4 Arena's gun turret (a pad), BF2 Hoth's
/// dish (a wreck).
#[test]
fn engineer_bots_rebuild_a_destroyed_turret_one_at_a_time() {
    for c in [&CASES[0], &CASES[3]] {
        let Some(mut w) = world(c) else {
            continue;
        };
        // The player is out of the way, far above.
        let i = first(&w, c);
        let h = standing(&w, i).unwrap();
        w.sim.destroy_vehicle(h, None);
        let a = engineer(&mut w, c, i, 8.0, 0);
        let b = engineer(&mut w, c, i, 12.0, 8);
        let start = w.sim.tick;
        let mut both = 0;
        let mut held_by = std::collections::BTreeSet::new();
        while w.sim.building_down(w.sim.building_turrets[i].instance) {
            assert!(
                w.sim.tick < start + 120 * 30,
                "{}: not built in 120 s (down at {:?}; fixers {:?})",
                c.map,
                w.sim.building_turrets[i].down,
                fixers(&w, i)
            );
            w.sim.step(&[PadState::default()]);
            let f = fixers(&w, i);
            both += usize::from(f.len() > 1);
            // (After the first second: both think at once on their first
            // tick, and the later claim gives way at its next think.)
            if w.sim.tick >= start + 30 {
                held_by.extend(f.iter().map(|h| h.index));
            }
        }
        let secs = (w.sim.tick - start) as f32 / 30.0;
        println!(
            "{} {}: built by bots in {secs:.1} s; claim holders {held_by:?}, ticks with two claims {both}",
            c.map, c.class
        );
        // Two that pick on one think pick blind of each other: the later
        // gives way at its next think (a second at most).
        assert!(both <= 30, "{}: {both} ticks with two fixers", c.map);
        assert_eq!(held_by.len(), 1, "{}: one engineer took it", c.map);
        assert!(held_by.contains(&a.index) || held_by.contains(&b.index));
        let v = w.sim.vehicle(standing(&w, i).unwrap()).unwrap();
        assert_eq!(v.health.cur, v.health.max);
        // Whole: nobody keeps a claim on it.
        for _ in 0..60 {
            w.sim.step(&[PadState::default()]);
        }
        assert!(fixers(&w, i).is_empty(), "{}: the claim ends", c.map);
    }
}

/// With no cutter near, a destroyed turret never comes back (the maintainer
/// 2026-10-28: "It shouldn't magically heal itself after some set time";
/// the PS2 regenerates it over its RespawnTime, `[ours]`): every case, 300 s.
#[test]
fn a_destroyed_turret_with_no_engineer_stays_down() {
    for c in &CASES {
        let Some(mut w) = world(c) else {
            continue;
        };
        let i = first(&w, c);
        let h = standing(&w, i).unwrap();
        w.sim.destroy_vehicle(h, None);
        for _ in 0..300 * 30 {
            w.sim.step(&[PadState::default()]);
        }
        assert_eq!(w.sim.building_turrets[i].down, Some(0.0), "{}", c.map);
        assert!(standing(&w, i).is_none());
    }
}

fn build_case(map: &'static str, cw: bool) -> Case {
    Case {
        map,
        factions: if cw { ["rep", "cis"] } else { ["all", "imp"] },
        class: "tur_bldg_laser",
        fall: TurretFall::Pad,
        model: "com_prop_buildzone",
        engineer: if cw {
            "rep_inf_ep3_engineer"
        } else {
            "all_inf_engineer"
        },
        cutter: if cw {
            "rep_weap_inf_fusioncutter"
        } else {
            "all_weap_inf_fusioncutter"
        },
        heal: 100.0,
        build: 50.0,
    }
}

#[test]
fn initial_bf2_pads_build_through_cutter_on_real_maps_in_both_eras() {
    for map in ["bf2_yav1", "bf2_nab2"] {
        for cw in [true, false] {
            let c = build_case(map, cw);
            let mut w = world(&c).expect("real PS2 map data required");
            let i = first(&w, &c);
            let t = &w.sim.building_turrets[i];
            assert_eq!(t.down, Some(0.0), "{map} cw={cw}: starts unbuilt");
            assert_eq!(t.spec.max_health, 800.0);
            assert!(standing(&w, i).is_none(), "no live/aliased vehicle handle");
            assert!(!t.standing_parts.is_empty(), "intact collision retained");
            assert_eq!(bodies(&w, i).0, 0, "pad has no authored collision");
            let old = t.handle;
            let side = reach(&mut w, i);
            assert_eq!(w.sim.building_turrets[i].down, Some(50.0));
            let mut shots = vec![w.sim.tick];
            for _ in 0..10 * core::TICK_HZ {
                let gain = cut(&mut w, i, side);
                if gain > 0.0 {
                    assert_eq!(gain, 50.0);
                    shots.push(w.sim.tick);
                }
                if w.sim.building_turrets[i].down.is_none() {
                    break;
                }
            }
            assert_eq!(shots.len(), 16, "{map} cw={cw}: sixteen cutter shots");
            assert!(shots.windows(2).all(|p| p[1] - p[0] == 15));
            assert_eq!(shots[15] - shots[0], 225, "7.5 s from first to last");
            let h = standing(&w, i).expect("built at full health");
            assert_ne!(h, old, "fresh generation, no stale handle aliases");
            assert_eq!(w.sim.vehicle(h).unwrap().health.cur, 800.0);
            assert_eq!(
                bodies(&w, i).0,
                w.sim.building_turrets[i].standing_parts.len()
            );
            assert_eq!(
                w.sim.collision.turret(w.sim.building_turrets[i].instance),
                Some(h)
            );
            w.sim.destroy_vehicle(h, None);
            assert_eq!(w.sim.building_turrets[i].down, Some(0.0));
            println!(
                "{map} cw={cw}: initial pad, 16 pulses/225 ticks, rebuilt collision, back to pad"
            );
        }
    }
}

#[test]
fn engineers_build_initial_pads_one_at_a_time_in_both_eras() {
    for cw in [true, false] {
        let c = build_case("bf2_yav1", cw);
        let mut w = world(&c).expect("real PS2 map data required");
        let i = first(&w, &c);
        // Make the selected nearby pad the only outstanding repair objective.
        for j in 0..w.sim.building_turrets.len() {
            if j != i {
                w.sim.repair_building(j as u16, f32::MAX);
            }
        }
        let _a = engineer(&mut w, &c, i, 8.0, 0);
        let _b = engineer(&mut w, &c, i, 9.0, 8);
        let mut saw_claim = false;
        for _ in 0..40 * core::TICK_HZ {
            w.sim.step(&[PadState::default()]);
            let claims = fixers(&w, i).len();
            // Claims chosen in one think settle by index on the next think.
            if w.sim.tick.is_multiple_of(core::TICK_HZ as u64) {
                assert!(claims <= 1);
            }
            saw_claim |= claims == 1;
            if standing(&w, i).is_some() {
                break;
            }
        }
        assert!(saw_claim, "cw={cw}: engineer claimed the initial pad");
        assert!(standing(&w, i).is_some(), "cw={cw}: engineer built it");
    }
}

#[test]
fn real_build_hologram_draws_above_the_pad_and_stops_when_built() {
    let c = build_case("bf2_yav1", true);
    let w = world(&c).expect("real PS2 map data required");
    let i = first(&w, &c);
    let pos = w.sim.building_turrets[i].spec.pos;
    let mut renderer = render::Renderer::new_headless(256, 256).expect("headless GPU");
    let art = game::holo::HoloArt::load(&mut renderer, &w.scene);
    assert!(!art
        .icons
        .get("build")
        .expect("original turret hologram model")
        .is_empty());
    let mut walk = game::walk::WalkSim::new(1, Vec3::ZERO, None);
    walk.sim = w.sim;
    walk.player = w.me;
    let mut holos = game::holo::Holos::default();
    holos.advance(&walk);
    walk.sim.tick += 90;
    holos.advance(&walk);
    let camera = render::Camera {
        position: pos + Vec3::new(0.0, 1.5, 5.0),
        pitch: -0.15,
        ..Default::default()
    };
    let draws = holos.draws(&art, &walk, &camera);
    assert!(!draws.is_empty());
    let icons: Vec<_> = draws
        .iter()
        .filter(|d| matches!(d.shape, render::ParticleShape::Mesh { .. }))
        .cloned()
        .collect();
    assert!(!icons.is_empty(), "original icon triangles submitted");
    renderer.set_particles(icons);
    renderer
        .render(&camera, &[], [0.02, 0.02, 0.02, 1.0])
        .unwrap();
    let (icon_rgba, _, _) = renderer.capture_rgba().unwrap();
    let icon_green = icon_rgba
        .chunks_exact(4)
        .filter(|p| i16::from(p[1]) - i16::from(p[0]) > 20 && p[1] > p[2])
        .count();
    assert!(
        icon_green > 10,
        "turret icon itself reached the frame: {icon_green} pixels"
    );
    renderer.set_particles(draws);
    renderer
        .render(&camera, &[], [0.02, 0.02, 0.02, 1.0])
        .unwrap();
    let (rgba, width, height) = renderer.capture_rgba().unwrap();
    let green = rgba
        .chunks_exact(4)
        .filter(|p| i16::from(p[1]) - i16::from(p[0]) > 20 && p[1] > p[2])
        .count();
    assert!(
        green > 40,
        "green beam and turret icon reached frame: {green} pixels"
    );
    if let Some(dir) = std::env::var_os("OPENBF_BUILD_CAPTURE") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("build-hologram.png"),
            render::encode_png(&rgba, width, height).unwrap(),
        )
        .unwrap();
    }
    for j in 0..walk.sim.building_turrets.len() {
        walk.sim.repair_building(j as u16, f32::MAX);
    }
    assert!(
        holos.draws(&art, &walk, &camera).is_empty(),
        "built turrets show no holograms"
    );
}
