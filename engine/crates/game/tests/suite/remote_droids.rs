use core::heightfield::Heightfield;
use core::remote::{DroidEnd, LIFETIME};
use core::sim::{buttons, Event, PadState, Sim};
use core::unit::Controller;
use core::weapon::{HealthType, WeaponClass, DEG};
use game::roster::{load_classdb, ClassRoster};
use glam::Vec3;

fn flat() -> Heightfield {
    Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    }
}

fn check_disc_values(game: &str, name: &str, w: &WeaponClass) {
    let r = w.remote.as_ref().unwrap();
    let c = &r.droid;
    let bf2 = game == "bf2";
    assert!(r.take_control, "{game} {name}");
    assert_eq!(r.launch_speed(0.0), 3.0, "{game} {name}");
    assert_eq!(c.geometry.as_deref(), Some("com_weap_probedroid"));
    assert_eq!(c.pitch_limits, [-50.0 * DEG, 50.0 * DEG]);
    assert_eq!((c.lift_spring, c.lift_damp), (10.0, 3.0));
    assert_eq!(
        (c.max_speed, c.max_strafe_speed, c.max_turn_speed),
        (9.0, 7.0, 2.0)
    );
    assert_eq!(c.health_type, HealthType::Droid);
    let destruct = r.destruct.as_ref().unwrap();
    assert_eq!(destruct.charge_time, 3.0);
    let blaster = &r.blaster;
    assert!(blaster.trigger_single);
    assert_eq!(blaster.ai_range[1..], [16.0, 32.0]);
    if bf2 {
        assert_eq!((c.max_health, c.add_health), (100.0, -1.0));
        assert_eq!((c.set_altitude, c.acceleration), (2.0, 25.0));
        assert_eq!(c.energy_restore, -1.7);
        assert_eq!(c.track_offset, Vec3::new(0.0, -0.25, 1.8));
        assert_eq!(c.explosion.as_ref().map(|e| e.damage), Some(0.0));
        assert_eq!(blaster.shot_delay, [0.15, 0.15]);
        assert_eq!(
            (blaster.ordnance.velocity, blaster.ordnance.max_damage),
            (160.0, 60.0)
        );
        let burst = destruct.burst.as_ref().expect("incinerator burst");
        assert_eq!((burst.damage, burst.damage_outer), (10000.0, 12.0));
        assert_eq!(burst.scales[HealthType::Person as usize], 1.0);
        assert_eq!(burst.scales[HealthType::Vehicle as usize], 0.0);
        assert!(destruct.salvo.is_none());
        assert_eq!(
            destruct.burst_sound.as_deref(),
            Some("com_sfx_weap_remotedroid_exp")
        );
    } else {
        // As tough as BF2's, and its blaster hits a soldier as hard
        // (`game::normalize`; the disc says 40 health and 10 a hit).
        assert_eq!((c.max_health, c.add_health), (100.0, -1.0));
        assert_eq!(
            blaster.ordnance.max_damage * blaster.ordnance.scales[HealthType::Person as usize],
            60.0
        );
        let altitude = if name.starts_with("imp_") { 2.0 } else { 1.75 };
        assert_eq!((c.set_altitude, c.acceleration), (altitude, 70.0));
        assert_eq!(c.energy_restore, 0.0);
        assert_eq!(c.track_offset, Vec3::new(0.0, -0.25, 1.5));
        let e = c.explosion.as_ref().expect("exp2");
        assert_eq!((e.damage, e.damage_inner, e.damage_outer), (50.0, 3.0, 5.0));
        assert_eq!(blaster.shot_delay, [0.2, 0.2]);
        assert_eq!(
            (blaster.ordnance.velocity, blaster.ordnance.max_damage),
            (60.0, 10.0)
        );
        let s = destruct.salvo.as_ref().expect("orbital salvo");
        assert_eq!(
            (s.count, s.delay, s.distance, s.spread),
            (4, 0.75, 600.0, 2.5)
        );
        assert!((s.pitch - 70.0 * DEG).abs() < 1e-6);
        assert_eq!(
            (s.shell.ordnance.velocity, s.shell.ordnance.max_damage),
            (300.0, 1000.0)
        );
        let x = s.shell.ordnance.explosion.as_ref().expect("salvo exp");
        assert_eq!((x.damage, x.damage_outer), (500.0, 10.5));
        assert!(destruct.burst.is_none());
    }
}

fn fly(game: &str, name: &str, roster: &ClassRoster, slot: u8, max_health: f32) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(flat());
    let class = roster.register(&mut sim, &mut Vec::new());
    let owner = sim
        .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
        .unwrap();
    let u = &mut sim.units[owner.index as usize];
    for h in &mut u.held {
        h.state.active = h.slot == slot;
    }
    u.cur1 = Some(u.held.iter().position(|h| h.slot == slot).unwrap() as u8);
    let pad = |buttons: u32, ly: i16| PadState {
        buttons,
        ly,
        ..Default::default()
    };
    sim.step(&[pad(buttons::SECONDARY_FIRE, 0)]);
    for _ in 0..30 {
        if !sim.droids.is_empty() {
            break;
        }
        sim.step(&[pad(0, 0)]);
    }
    assert_eq!(sim.droids.len(), 1, "{game} {name}: launched");
    assert!(
        sim.controlled_droid(owner).is_some(),
        "{game} {name}: control"
    );
    let launched = sim.tick;
    let feet = sim.unit(owner).unwrap().soldier.pos;
    for _ in 0..60 {
        sim.step(&[pad(0, i16::MAX)]);
    }
    let d = &sim.droids[0];
    assert!(d.pos.z < -8.0, "{game} {name}: steered to {:?}", d.pos);
    assert!(sim.unit(owner).unwrap().soldier.pos.distance(feet) < 0.05);
    sim.step(&[pad(buttons::ACTION, 0)]);
    assert!(sim.controlled_droid(owner).is_none(), "{game} {name}: back");
    let mut end = None;
    let mut health = 0.0;
    for _ in 0..((LIFETIME + 2.0) * core::TICK_HZ as f32) as usize {
        health = sim.droids.first().map_or(health, |d| d.health);
        sim.step(&[pad(0, 0)]);
        end = end.or(sim.events.iter().find_map(|e| match e {
            Event::DroidGone { end, .. } => Some((*end, sim.tick)),
            _ => None,
        }));
        if end.is_some() {
            break;
        }
    }
    let (end, tick) = end.unwrap_or_else(|| panic!("{game} {name}: never expired"));
    assert_eq!(end, DroidEnd::Expired);
    // Ticks flown, the launch tick included: exactly 90 s.
    assert_eq!(
        tick - launched + 1,
        90 * core::TICK_HZ as u64,
        "{game} {name}"
    );
    assert_eq!(health, max_health, "{game} {name}: no drain");
}

#[test]
fn every_remote_droid_class_flies_the_discs_droid() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires both local baked discs");
        return;
    }
    // [ours] 90 s in both games; health (40 / 100) is only for damage.
    for (game, want, max_health) in [("bf1", 8, 100.0), ("bf2", 13, 100.0)] {
        let db = load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"]).unwrap();
        let model = root.join(format!("common_{game}/models/com_weap_probedroid.glb"));
        assert!(!game::assets::load_glb(&model).unwrap().indices.is_empty());
        let mut names: Vec<String> = db
            .names()
            .filter(|n| game::roster::is_soldier_chain(&db, n))
            .map(str::to_owned)
            .collect();
        names.sort();
        names.dedup();
        let mut found = 0;
        for name in &names {
            let Ok(roster) = ClassRoster::build(&db, name, 1, None) else {
                continue;
            };
            let Some(held) = roster.held.iter().find(|h| h.weapon.remote.is_some()) else {
                continue;
            };
            found += 1;
            assert_eq!((held.channel, held.clips), (1, 1), "{game} {name}");
            check_disc_values(game, name, &held.weapon);
            fly(game, name, &roster, held.slot, max_health);
        }
        assert_eq!(found, want, "{game}: classes with a remote droid");
    }
}

#[test]
fn one_tap_calls_the_destruct_three_seconds_later() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires both local baked discs");
        return;
    }
    for (game, name) in [("bf1", "all_inf_marksman"), ("bf2", "rep_inf_ep3_officer")] {
        let db = load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"]).unwrap();
        let roster = ClassRoster::build(&db, name, 1, None).unwrap();
        let slot = roster
            .held
            .iter()
            .find(|h| h.weapon.remote.is_some())
            .unwrap()
            .slot;
        let mut sim = Sim::empty(42);
        sim.heightfield = Some(flat());
        let class = roster.register(&mut sim, &mut Vec::new());
        let owner = sim
            .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
            .unwrap();
        let u = &mut sim.units[owner.index as usize];
        for h in &mut u.held {
            h.state.active = h.slot == slot;
        }
        u.cur1 = Some(u.held.iter().position(|h| h.slot == slot).unwrap() as u8);
        let pad = |buttons: u32| PadState {
            buttons,
            ..Default::default()
        };
        for _ in 0..30 {
            sim.step(&[pad(buttons::SECONDARY_FIRE)]);
        }
        assert!(sim.controlled_droid(owner).is_some(), "{game} {name}");
        sim.step(&[pad(0)]);
        sim.step(&[pad(buttons::SECONDARY_FIRE)]);
        let tapped = sim.tick;
        let mut end = None;
        for _ in 0..120 {
            sim.step(&[pad(0)]);
            end = end.or(sim.events.iter().find_map(|e| match e {
                Event::DroidGone { end, .. } => Some((*end, sim.tick)),
                _ => None,
            }));
        }
        let (end, tick) = end.unwrap_or_else(|| panic!("{game} {name}: no destruct"));
        assert_eq!(end, DroidEnd::Destruct, "{game} {name}");
        let secs = (tick - tapped) as f32 / core::TICK_HZ as f32;
        assert!((secs - 3.0).abs() < 0.05, "{game} {name}: {secs} s");
        assert!(sim.controlled_droid(owner).is_none());
    }
}

/// A walk with one class on both teams, its HUD rows as the battle builds them.
pub(super) fn walk_of(roster: &ClassRoster) -> game::walk::WalkSim {
    let mut sim = Sim::empty(42);
    let class = roster.register(&mut sim, &mut Vec::new());
    let mut visual = game::walk::ClassVisual::from_roster(roster);
    visual.held_hud = roster
        .held
        .iter()
        .map(|h| (h.slot, h.hud.clone()))
        .collect();
    let faction = game::roster::FactionRoster {
        faction: "rep".into(),
        classes: vec![roster.clone()],
    };
    let t = game::walk::TeamTemplate::from_roster(&faction, &class, vec![visual]);
    game::walk::WalkSim::with_teams(sim, Some(flat()), Vec3::ZERO, [t.clone(), t], None)
}

#[test]
fn the_weapon_rows_show_the_droids_blaster_and_destruct_while_it_flies() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires both local baked discs");
        return;
    }
    // `[ours]` BF2's meshes on every map (maintainer, 2026-10-28): the BF1
    // class's rows too, with no BF1 icon sheet.
    let cases = [("bf1", "all_inf_marksman"), ("bf2", "rep_inf_ep3_officer")];
    for (game, name) in cases {
        let db = load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"]).unwrap();
        let roster = ClassRoster::build(&db, name, 1, None).unwrap();
        let remote = roster
            .held
            .iter()
            .find(|h| h.weapon.remote.is_some())
            .unwrap();
        let slot = remote.slot;
        // The droid class's own map icon.
        let icon = game::minimap::droid_icon(&db, &remote.weapon.name);
        assert_eq!(icon.texture.as_deref(), Some("troop_icon"), "{game} {name}");
        assert_eq!(icon.scale, 1.0, "{game} {name}");
        let mut walk = walk_of(&roster);
        let owner = walk.player;
        let u = &mut walk.sim.units[owner.index as usize];
        for h in &mut u.held {
            h.state.active = h.slot == slot;
        }
        u.cur1 = Some(u.held.iter().position(|h| h.slot == slot).unwrap() as u8);
        let pad = |buttons: u32| PadState {
            buttons,
            ..Default::default()
        };
        let on_foot = game::hud_feed::weapon_rows(&walk);
        assert!(
            on_foot[0].is_some() && on_foot[1].is_some(),
            "{game} {name}"
        );
        for _ in 0..30 {
            walk.sim.step(&[pad(buttons::SECONDARY_FIRE)]);
        }
        assert!(walk.sim.controlled_droid(owner).is_some(), "{game} {name}");
        let [primary, secondary] = game::hud_feed::weapon_rows(&walk);
        let primary = primary.expect("blaster row");
        let secondary = secondary.expect("destruct row");
        assert_eq!(primary.key, game::roster::BLASTER_HUD, "{game} {name}");
        assert_eq!(primary.icon, None, "{game} {name}");
        assert_eq!(primary.total, None, "{game} {name}: the blaster is endless");
        assert_eq!(secondary.key, game::roster::DESTRUCT_HUD, "{game} {name}");
        assert_eq!(secondary.icon, None, "{game} {name}");
        assert_eq!(secondary.total, Some(1), "{game} {name}: one destruct");
        // The BF2 HUD (loaded on every map) draws both rows' meshes.
        let model = ui::hud::HudModel {
            hp: 40.0,
            hp_max: 40.0,
            weapon: Some(primary.clone()),
            weapon2: Some(secondary.clone()),
            ..Default::default()
        };
        let mut view = game::hud_feed::HudView::load(&root, render::HEADLESS_FORMAT, None).unwrap();
        view.update(&model, 10.0, [1280, 720]);
        view.update(&model, 10.0, [1280, 720]);
        for row in ["player1weapon1mesh", "player1weapon2mesh"] {
            let b = view
                .bounds(row)
                .unwrap_or_else(|| panic!("{game} {name}: {row} drawn"));
            assert!(b[2] > b[0] && b[3] > b[1], "{game} {name}: {row} {b:?}");
        }
        // Control back: the soldier's own rows return.
        walk.sim.step(&[pad(0)]);
        walk.sim.step(&[pad(buttons::ACTION)]);
        assert!(walk.sim.controlled_droid(owner).is_none());
        let back = game::hud_feed::weapon_rows(&walk);
        assert_eq!(back[0], on_foot[0], "{game} {name}");
        assert!(back[1].is_some(), "{game} {name}");
    }
}

/// Issue #59 on the real maps: the shapes the disc's `bnd_` chunks name all come
/// out of the bake and hold what the discs put in them (`re-boundary.md` D2:
/// every conquest post on the maps that have a boundary), BF2 HOT1's walk state's
/// outside point reads outside, and BF2 MYG1 has no boundary.
#[test]
fn every_maps_boundary_comes_out_of_its_bake_round_its_posts() {
    let root = game::assets::baked_root();
    let maps = game::assets::discover_maps(&root);
    if maps.is_empty() {
        eprintln!("SKIP: no baked maps");
        return;
    }
    let (mut with, mut posts) = (0, 0);
    for (key, _, dir) in &maps {
        let scene = game::assets::load_scene(dir).expect("scene");
        let names = game::map_boundary::shape_names(key);
        let paths = &scene.manifest.paths;
        for name in names {
            assert!(
                paths.iter().any(|p| p.name.eq_ignore_ascii_case(name)),
                "{key}: no path {name}"
            );
        }
        let b = game::map_boundary::boundary(key, paths);
        if names.is_empty() {
            assert!(!b.outside(1e5, 1e5), "{key}: no boundary, never outside");
            continue;
        }
        with += 1;
        assert!(b.outside(1e5, 1e5), "{key}: nothing is outside");
        for cp in game::layout::MapLayout::from_manifest(&scene.manifest).cps {
            // BF2 KAS2's `baserush` belongs to another mode (D2).
            if cp.name.eq_ignore_ascii_case("baserush") {
                continue;
            }
            assert!(
                !b.outside(cp.pos.x, cp.pos.z),
                "{key}: {} at {}",
                cp.name,
                cp.pos
            );
            posts += 1;
        }
    }
    println!("{with} maps with a boundary, {posts} posts inside");
    assert!(with >= 20, "{with} maps with a boundary");
    if let Ok(hoth) = game::assets::load_scene(&root.join("bf2_hot1")) {
        let b = game::map_boundary::boundary("bf2_hot1", &hoth.manifest.paths);
        assert!(b.outside(295.43, 190.01), "the PS2 trial's outside point");
    }
}

/// Points the PS2's own test read, one on each side of an edge where the map has
/// a boundary (`lead_tmp/bndy/points`, `re-boundary.md` M1): `(map, x, z, outside)`.
/// All 404 rows of those captures agreed when this test was added.
const PS2_POINTS: [(&str, f32, f32, bool); 14] = [
    ("bf1_hot1", -524.75, 310.28, false),
    ("bf1_hot1", -530.21, 312.78, true),
    ("bf1_kam1", 303.94, 70.43, false),
    ("bf1_kam1", 309.64, 68.56, true),
    ("bf1_nab1", -78.22, 265.72, false),
    ("bf1_nab1", -82.44, 269.99, true),
    ("bf2_hot1", 290.1, 187.25, false),
    ("bf2_hot1", 295.43, 190.01, true),
    ("tat1", 166.17, 301.17, false),
    ("tat1", 171.92, 302.88, true),
    ("bf1_yav2", 286.97, 101.98, false),
    ("bf2_myg1", 213.79, 187.03, false),
    ("bf2_uta1", 215.76, 271.36, false),
    ("bf2_uta1", 69.12, 448.05, false),
];

#[test]
fn the_boundary_reads_as_the_ps2_read_it() {
    for (map, x, z, outside) in PS2_POINTS {
        let Ok(scene) = game::assets::load_scene(&game::assets::baked_root().join(map)) else {
            eprintln!("SKIP: {map} is not baked");
            continue;
        };
        let b = game::map_boundary::boundary(map, &scene.manifest.paths);
        assert_eq!(b.outside(x, z), outside, "{map} at ({x}, {z})");
    }
}

/// `Scene::install_collision`, which every sim loader calls, hands the shapes on.
#[test]
fn a_loaded_map_hands_its_boundary_to_the_sim() {
    let dir = game::assets::baked_root().join("bf2_hot1");
    let Ok(scene) = game::assets::load_scene(&dir) else {
        eprintln!("SKIP: bf2_hot1 is not baked");
        return;
    };
    let mut sim = Sim::empty(1);
    scene.install_collision(&mut sim);
    assert!(sim.boundary.outside(295.43, 190.01));
}

/// Issue #59, flown for real on every baked map: launched a few metres inside the
/// map's edge and flown straight out, on a map's own collision and terrain. Past a
/// boundary the countdown takes the droid; with none it stops at the sheet's end.
/// Where a wall or a hill stops it first, it never leaves at all.
#[test]
fn a_droid_flown_at_every_maps_edge_cannot_leave_it() {
    let root = game::assets::baked_root();
    let Ok(db) = load_classdb(&root.join("classes"), "bf2", &["all"]) else {
        eprintln!("SKIP: no BF2 class bake");
        return;
    };
    let mut names: Vec<String> = db.names().map(str::to_owned).collect();
    names.sort();
    let (roster, slot) = names
        .iter()
        .filter(|n| game::roster::is_soldier_chain(&db, n))
        .find_map(|n| {
            let r = ClassRoster::build(&db, n, 1, None).ok()?;
            let slot = r.held.iter().find(|h| h.weapon.remote.is_some())?.slot;
            Some((r, slot))
        })
        .expect("a class with a remote droid");
    let (mut killed, mut stopped, mut ran) = (0, 0, 0);
    for (key, game, dir) in game::assets::discover_maps(&root) {
        let scene = game::assets::load_scene(&dir).expect("scene");
        let Some(hf) = scene.heightfield() else {
            println!("{key}: no terrain");
            continue;
        };
        ran += 1;
        let mut sim = Sim::empty(42);
        sim.ground_rules = core::soldier::GroundRules::for_game(&game);
        scene.install_collision(&mut sim);
        let class = roster.register(&mut sim, &mut Vec::new());
        let posts = game::layout::MapLayout::from_manifest(&scene.manifest).cps;
        let home = posts.first().map_or(Vec3::ZERO, |c| c.pos);
        sim.heightfield = Some(hf);
        let owner = sim
            .spawn(&class.spec(0, home, 0.0, Controller::Player { slot: 0 }, 0, 0))
            .unwrap();
        let u = &mut sim.units[owner.index as usize];
        for h in &mut u.held {
            h.state.active = h.slot == slot;
        }
        u.cur1 = Some(u.held.iter().position(|h| h.slot == slot).unwrap() as u8);
        let pad = |buttons: u32, ly: i16| PadState {
            buttons,
            ly,
            ..Default::default()
        };
        sim.step(&[pad(buttons::SECONDARY_FIRE, 0)]);
        for _ in 0..30 {
            if !sim.droids.is_empty() {
                break;
            }
            sim.step(&[pad(0, 0)]);
        }
        assert_eq!(sim.droids.len(), 1, "{key}: launched");

        // The way out: from the post along +x until the map ends, back 5 m.
        let hold = |sim: &Sim, p: Vec3| {
            let (x, z) = sim.heightfield.as_ref().unwrap().clamp_xz(p.x, p.z);
            x == p.x && z == p.z && !sim.boundary.outside(p.x, p.z)
        };
        let mut edge = home;
        while hold(&sim, edge + Vec3::X) {
            edge += Vec3::X;
        }
        let start = edge - Vec3::X * 5.0;
        let d = &mut sim.droids[0];
        d.pos = Vec3::new(
            start.x,
            sim.heightfield
                .as_ref()
                .unwrap()
                .height_at(start.x, start.z)
                + 3.0,
            start.z,
        );
        d.vel = Vec3::ZERO;
        // Heading +x (the droid's forward is -sin yaw, -cos yaw).
        d.yaw = -std::f32::consts::FRAC_PI_2;
        let time = core::boundary::leaving_time(sim.ground_rules);
        let (mut end, mut out, mut left) = (None, 0.0f32, false);
        for _ in 0..((time + 8.0) * core::TICK_HZ as f32) as usize {
            sim.step(&[pad(0, i16::MAX)]);
            end = end.or(sim.events.iter().find_map(|e| match e {
                Event::DroidGone { end, .. } => Some(*end),
                _ => None,
            }));
            let Some(d) = sim.droids.first() else { break };
            let (x, z) = sim.heightfield.as_ref().unwrap().clamp_xz(d.pos.x, d.pos.z);
            assert_eq!((x, z), (d.pos.x, d.pos.z), "{key}: off the sheet");
            if d.leaving.is_some() {
                left = true;
                out += core::TICK_DT;
            }
        }
        assert!(out <= time + 0.1, "{key}: {out} s outside");
        match end {
            Some(DroidEnd::Destroyed) => {
                assert!(left, "{key}: destroyed inside");
                killed += 1;
            }
            None => stopped += 1,
            other => panic!("{key}: {other:?}"),
        }
    }
    println!("{ran} maps: {killed} killed past the boundary, {stopped} held inside");
    if ran > 0 {
        assert!(killed >= 5, "{killed} of {ran} reached the countdown");
    }
}
