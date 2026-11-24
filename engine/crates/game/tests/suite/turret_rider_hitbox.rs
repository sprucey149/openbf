//! A BF2 build-zone turret's gunner is hit where he is drawn (playtest
//! 2026-11-14, BF2 Coruscant, Conquest CW, Hard). The `tur_bldg_laser`
//! "Minigun Turret" at (24.46, 6.99, -117.74) had a droid at its gun; the
//! player sniped him from (-3.44, 7.08, -30.16). Aimed at the drawn head the
//! shots missed; aimed lower they counted as head hits; the logged ray
//! (yaw -0.3094, pitch -0.0018, ticks 5028 and 5147) struck the turret for 30
//! twice and then the gunner's head.
//!
//! The cause: the sim looked up the gunner's seat through the class's build
//! zone (`com_prop_buildzone`, no rig), found none and stood his body at the
//! turret's origin facing the yaw he boarded with, 0.34 m under and beside
//! the drawn gunner, who stands at his seat on the turret node and turns
//! with the gun. The seat now comes from the class row the scene draws.
//!
//! The playtest's body shots killing full-health droids were the War Hero
//! award (x1.5, on with every cheat that round): the army sniper rifle does
//! 280 to a person (`normalize::SHARED_RULES`), 420 with War Hero.
//!
//! Real map, classes and soldier models from the bake; skips without them.

use core::sim::{buttons, Event, PadState};
use core::unit::{Controller, UnitHandle};
use glam::Vec3;

const TURRET: Vec3 = Vec3::new(24.46, 6.99, -117.74);
const ME: Vec3 = Vec3::new(-3.44, 7.08, -30.16);
/// The logged shot (match log ticks 5028 and 5147).
const LOGGED: (f32, f32) = (-0.3094, -0.0018);
/// Gun looks off the player, in degrees: the gunner turns with the gun.
const LOOKS: [f32; 7] = [0.0, 45.0, 90.0, 135.0, 180.0, -90.0, -45.0];

struct Scene {
    walk: game::walk::WalkSim,
    vis: game::soldier_vis::SoldierVisuals,
    gunner: UnitHandle,
    turret: core::vehicle::VehicleHandle,
    /// The gunner's class health.
    health: f32,
    /// The gun has a drawn gunner's seat (`GunSetup::gunner`).
    seat: bool,
    look: (f32, f32),
}

/// bf2_cor1 with the pad built and a `class` droid at its gun.
fn scene(class: &str) -> Option<Scene> {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_cor1");
    let Ok(mut scene) = game::assets::load_scene(&dir) else {
        eprintln!("SKIP: no bf2_cor1 bake");
        return None;
    };
    scene.apply_mode_layer("con");
    scene.merge_common(game::assets::load_common(&root.join("common_bf2")).ok()?);
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["cis", "rep"]).ok()?;
    let mut sim = core::sim::Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    scene.install_collision(&mut sim);
    let f = ["cis".to_owned(), "rep".to_owned()];
    let build =
        game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
            .ok()?;
    let mut names = Vec::new();
    let team = |sim: &mut core::sim::Sim, names: &mut Vec<_>, class: &str, side: &str| {
        let roster = game::roster::ClassRoster::build(&db, class, 1, None).unwrap();
        let uc = roster.register(sim, names);
        let visual = game::walk::ClassVisual::from_roster(&roster);
        let faction = game::roster::FactionRoster {
            faction: side.into(),
            classes: vec![roster.clone()],
        };
        game::walk::TeamTemplate::from_roster(&faction, &uc, vec![visual])
    };
    let t0 = team(&mut sim, &mut names, class, "cis");
    let t1 = team(&mut sim, &mut names, "rep_inf_ep3_sniper", "rep");
    let hf = scene.heightfield();
    let mut walk = game::walk::WalkSim::with_teams_seat(sim, hf, ME, [t0.clone(), t1], None, 1);
    walk.vehicle_seats = build.seats.clone();
    game::seated_bodies::install(&mut walk, &root, "bf2");
    // Aimed by hand, as logged: no magnet.
    walk.sim.assist_intensity = 0.0;
    // The pad starts unbuilt in a bare sim; it stood built in the playtest.
    let pad = walk
        .sim
        .building_turrets
        .iter()
        .position(|t| t.spec.pos.distance(TURRET) < 0.1)
        .expect("the turret's pad");
    walk.sim.repair_building(pad as u16, 1.0e6);
    walk.step_tick(&[PadState::default()]);
    let turret = walk
        .sim
        .alive_vehicles()
        .find(|(_, v)| v.pos.distance(TURRET) < 0.1)
        .map(|(h, _)| h)
        .expect("the built turret");
    let v = walk.sim.vehicle(turret)?;
    assert_eq!(build.classes[v.class as usize], "tur_bldg_laser");
    let setup = walk.sim.seat_setup[v.class as usize].as_ref().unwrap();
    let gun = setup
        .guns
        .first()
        .and_then(Option::as_ref)
        .expect("its gun");
    let seat = gun.gunner.is_some();
    let gunner = walk
        .sim
        .spawn(&t0.spec(0, TURRET + Vec3::Y * 5.0, 0.0, Controller::Idle, 0))
        .unwrap();
    // The yaw unit 1 boarded with.
    walk.sim.unit_mut(gunner).unwrap().soldier.yaw = -9.37;
    assert!(walk.sim.seat_unit(gunner, turret, 0));
    let health = walk.sim.unit(gunner).unwrap().health.max;
    walk.sim.cheats.infinite_ammo = true;
    let mut vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, "bf2", &walk.teams);
    vis.set_turrets(game::viewer::turret_pose::rigged_turrets(&scene));
    let mut s = Scene {
        walk,
        vis,
        gunner,
        turret,
        health,
        seat,
        look: (0.0, 0.0),
    };
    // Scope in along the logged aim.
    s.look = (look_at_player(&s), 0.0);
    for _ in 0..40 {
        aim(&mut s, LOGGED);
        tick(&mut s, 0);
    }
    tick(&mut s, buttons::ZOOM);
    for _ in 0..20 {
        aim(&mut s, LOGGED);
        tick(&mut s, 0);
    }
    assert!(s.walk.sim.unit(s.walk.player).unwrap().scoped(), "scoped");
    Some(s)
}

/// Hold the gun at `Scene::look` (model frame).
fn set_look(s: &mut Scene) {
    let look = s.look;
    let v = s.walk.sim.vehicle_mut(s.turret).unwrap();
    if v.guns.is_empty() {
        v.guns.resize(1, None);
    }
    let g = v.guns[0].get_or_insert_with(|| core::vehicle::gun::GunState {
        target: None,
        look,
        aim: look,
        rate: (0.0, 0.0),
        rel: look,
        weapons: Vec::new(),
        zoom: None,
    });
    g.look = look;
    g.aim = look;
    g.rel = look;
}

fn tick(s: &mut Scene, buttons: u32) {
    set_look(s);
    s.walk.step_tick(&[PadState {
        buttons,
        ..Default::default()
    }]);
    s.vis.update(&s.walk, core::TICK_DT);
}

/// The model-frame yaw that points the gun at the player.
fn look_at_player(s: &Scene) -> f32 {
    let v = s.walk.sim.vehicle(s.turret).unwrap();
    let d = v.rot.inverse() * (ME - TURRET);
    d.x.atan2(d.z)
}

/// The player at the logged spot, looking along (yaw, pitch).
fn aim(s: &mut Scene, (yaw, pitch): (f32, f32)) {
    let u = s.walk.sim.unit_mut(s.walk.player).unwrap();
    u.soldier.pos = ME;
    u.soldier.vel = Vec3::ZERO;
    u.soldier.yaw = yaw;
    u.soldier.pitch = pitch;
}

/// What one scoped shot along `look` struck.
#[derive(Debug, PartialEq)]
enum Struck {
    Gunner(f32),
    Turret,
    Nothing,
}

fn fire(s: &mut Scene, look: (f32, f32)) -> Struck {
    let me = s.walk.player;
    for _ in 0..60 {
        aim(s, look);
        s.walk.sim.unit_mut(me).unwrap().weapon.clip = 5;
        tick(s, 0);
    }
    for k in 0..12 {
        aim(s, look);
        tick(s, if k == 0 { buttons::FIRE } else { 0 });
        for e in &s.walk.sim.events {
            match *e {
                Event::Hit { victim, damage, .. } if victim == s.gunner => {
                    return Struck::Gunner(damage)
                }
                Event::VehicleHit { vehicle, .. } if vehicle == s.turret => return Struck::Turret,
                _ => {}
            }
        }
    }
    Struck::Nothing
}

fn eye(s: &Scene) -> Vec3 {
    let u = s.walk.sim.unit(s.walk.player).unwrap();
    u.aim_ray(s.walk.sim.ground_rules, s.walk.sim.views()).0
}

/// The look from the player's scoped eye at `p` (the eye moves a little with
/// the aim, so twice).
fn look_toward(s: &mut Scene, p: Vec3) -> (f32, f32) {
    let to = |eye: Vec3| {
        let d = (p - eye).normalize();
        ((-d.x).atan2(-d.z), d.y.asin())
    };
    let mut look = to(eye(s));
    for _ in 0..2 {
        aim(s, look);
        tick(s, 0);
        look = to(eye(s));
    }
    look
}

/// The drawn gunner's model origin, head and chest bones.
fn drawn(s: &Scene) -> (Vec3, Vec3, Vec3) {
    let i = s.gunner.index as usize;
    let (_, bones) = s.vis.drawn_body(&s.walk, i).expect("the gunner is drawn");
    let bone = |n: &str| bones.iter().find(|b| b.0 == n).map(|b| b.1);
    let origin = s
        .vis
        .drawn(&s.walk)
        .into_iter()
        .find(|d| d.0 == i)
        .unwrap()
        .1
        .w_axis
        .truncate();
    (
        origin,
        bone("bone_head").unwrap(),
        bone("bone_ribcage").unwrap(),
    )
}

/// The sim's head prim centre: the seat pose's (`BoltPrims::posed`), else
/// the standing posture's (1.65 m up, 0.16 m forward).
fn head_prim(s: &Scene) -> Vec3 {
    prim_head(&s.walk.sim.body_prims(s.gunner).expect("a body to hit"))
}

fn prim_head(p: &core::unit::BoltPrims) -> Vec3 {
    let (sy, cy) = p.yaw.sin_cos();
    let right = Vec3::new(cy, 0.0, -sy);
    let fwd = Vec3::new(-sy, 0.0, -cy);
    let h = p
        .posed
        .map_or(Vec3::new(0.0, 1.65 * p.scale, 0.16), |q| q[0]);
    p.pos + right * h.x + Vec3::Y * h.y + fwd * h.z
}

/// At every gun look the gunner's body stands under the drawn one (feet
/// within 5 cm, head prim within 12 cm of the drawn head bone, under the
/// 0.17 m head radius), a shot at the drawn head is a head hit (280 x3), one
/// at the drawn chest a body hit, and the logged ray is no head hit.
#[test]
fn a_bf2_build_zone_turret_gunner_is_hit_where_he_is_drawn() {
    // Issue #182: the MagnaGuard (`cis_inf_officer`, 1.24 root on his own
    // longer rig) too.
    for class in ["cis_inf_rocketeer", "cis_inf_officer"] {
        build_zone_gunner_is_hit_where_drawn(class);
    }
}

fn build_zone_gunner_is_hit_where_drawn(class: &str) {
    let Some(mut s) = scene(class) else {
        return;
    };
    assert!(s.seat, "the drawn gunner has a seat on the turret's rig");
    // He takes every shot and lives.
    let g = s.walk.sim.unit_mut(s.gunner).unwrap();
    g.health.max = 1.0e9;
    g.health.cur = 1.0e9;
    let face = look_at_player(&s);
    for dl in LOOKS {
        s.look = (face + dl.to_radians(), 0.0);
        for _ in 0..3 {
            aim(&mut s, LOGGED);
            tick(&mut s, 0);
        }
        let feet = s.walk.sim.unit(s.gunner).unwrap().soldier.pos;
        let (origin, head, chest) = drawn(&s);
        let gap = head.distance(head_prim(&s));
        println!(
            "{class} look {dl:+.0}: feet {:.3} off the drawn origin, head prim {gap:.3} off the drawn head",
            feet.distance(origin)
        );
        assert!(
            feet.distance(origin) < 0.05,
            "look {dl}: feet {feet:.3}, drawn {origin:.3}"
        );
        assert!(
            gap < 0.12,
            "look {dl}: head prim {gap:.3} m off the drawn head"
        );
        let at_head = look_toward(&mut s, head);
        assert_eq!(
            fire(&mut s, at_head),
            Struck::Gunner(840.0),
            "look {dl}: at the drawn head"
        );
        let at_chest = look_toward(&mut s, chest);
        assert_eq!(
            fire(&mut s, at_chest),
            Struck::Gunner(280.0),
            "look {dl}: at the drawn chest"
        );
        let logged = fire(&mut s, LOGGED);
        println!("look {dl:+.0}: logged ray -> {logged:?}");
        assert_ne!(logged, Struck::Gunner(840.0), "look {dl}: the logged ray");
    }
    if class == "cis_inf_officer" {
        // Without the seat poses his bolt prims stand on his model's
        // standing idle (`STANCE_BODY`, the MagnaGuard's 1.895 m head):
        // this gunner stands at his gun, so the head prim is near his
        // drawn head too. The posture lists' 1.65 m head was 0.18 m off.
        s.walk.sim.seated_bodies = None;
        s.look = (face, 0.0);
        for _ in 0..3 {
            aim(&mut s, LOGGED);
            tick(&mut s, 0);
        }
        let (_, head, _) = drawn(&s);
        let gap = head.distance(head_prim(&s));
        println!("{class} without seat poses: head prim {gap:.3} off the drawn head");
        assert!(gap < 0.12, "without seat poses the gap is {gap:.3} m");
    }
}

/// The army sniper rifle's body hit does 280 and leaves a full-health
/// soldier alive; only a head hit kills him in one shot. With the War Hero
/// award (x1.5, on in the playtest) the body hit does 420 and kills a
/// 400-health droid.
#[test]
fn a_sniper_body_shot_does_not_kill_a_full_health_soldier() {
    let Some(mut s) = scene("cis_inf_rifleman") else {
        return;
    };
    let full = s.health;
    assert!(full >= 300.0, "a regular soldier: {full}");
    s.look = (look_at_player(&s), 0.0);
    let reset = |s: &mut Scene| {
        let g = s.walk.sim.unit_mut(s.gunner).unwrap();
        g.health.cur = g.health.max;
    };
    let alive = |s: &Scene| {
        s.walk
            .sim
            .unit(s.gunner)
            .is_some_and(|u| u.health.cur > 0.0)
    };

    reset(&mut s);
    let (_, head, chest) = drawn(&s);
    let at_chest = look_toward(&mut s, chest);
    assert_eq!(fire(&mut s, at_chest), Struck::Gunner(280.0), "body hit");
    assert!(alive(&s), "a body hit leaves him alive");
    let left = s.walk.sim.unit(s.gunner).unwrap().health.cur;
    assert_eq!(left, full - 280.0);

    reset(&mut s);
    let at_head = look_toward(&mut s, head);
    assert_eq!(fire(&mut s, at_head), Struck::Gunner(840.0), "head hit");
    assert!(!alive(&s), "a head hit kills him");

    // War Hero, as the playtest round had it: a fresh droid at the gun.
    let gunner = s
        .walk
        .sim
        .spawn(&s.walk.teams[0].spec(0, TURRET + Vec3::Y * 5.0, 0.0, Controller::Idle, 0))
        .unwrap();
    assert!(s.walk.sim.seat_unit(gunner, s.turret, 0));
    s.gunner = gunner;
    s.walk.sim.cheats.damage_award = true;
    for _ in 0..3 {
        aim(&mut s, LOGGED);
        tick(&mut s, 0);
    }
    let (_, _, chest) = drawn(&s);
    let at_chest = look_toward(&mut s, chest);
    assert_eq!(
        fire(&mut s, at_chest),
        Struck::Gunner(420.0),
        "body hit x1.5"
    );
    assert_eq!(full <= 420.0, !alive(&s), "War Hero's body hit");
}

/// Issue #182 ("MagnaGuard hit box broken in turrets"): every soldier class
/// of a game, of every size, in every building turret seat that draws its
/// gunner, is hit where he is drawn. Each map's turrets are built and every
/// class is seated in each at gun looks across its yaw and pitch; a level
/// ray from either side at the drawn bone_head, and one down his spine,
/// meets the head prim, and at his drawn chest (between bone_ribcage and
/// bone_pelvis) the torso or body, never the head. Every vehicle seat that
/// shows its rider is held to the same head. Without the seat poses
/// (`Sim::seated_bodies` = None) the MagnaGuard's drawn head is over the
/// head prim and a leaning trooper's under it.
#[test]
fn every_class_in_every_drawn_seat_is_hit_where_drawn() {
    const BF2: &[&str] = &[
        "bf2_cor1", "bf2_geo1", "bf2_hot1", "bf2_kam1", "bf2_kas2", "bf2_myg1", "bf2_yav1",
    ];
    const BF1: &[&str] = &["bf1_bes1", "bf1_geo1", "bf1_hot1", "bf1_tat2", "bf1_yav1"];
    let mut checked = 0;
    let mut sizes = std::collections::BTreeSet::new();
    for (game, maps, heroes) in [
        (
            "bf2",
            BF2,
            &[
                "rep_hero_yoda",
                "cis_hero_grievous",
                "all_hero_chewbacca",
                "imp_hero_darthvader",
            ][..],
        ),
        ("bf1", BF1, &[][..]),
    ] {
        for map in maps {
            let Some(n) = every_class_on(map, game, heroes, &mut sizes) else {
                continue;
            };
            checked += n;
        }
    }
    println!("{checked} class x seat (x look) cases; root scales {sizes:?}");
    if checked == 0 {
        eprintln!("SKIP: no map bake");
        return;
    }
    // The MagnaGuard's 1.24, the Wookiee's 1.15 and Yoda's small rig were
    // among them.
    assert!(
        sizes.contains("1.24") && sizes.contains("1.15"),
        "{sizes:?}"
    );
}

/// [`every_class_in_every_drawn_seat_is_hit_where_drawn`] on one
/// map: the cases checked, `None` without its bake.
fn every_class_on(
    map: &str,
    game: &str,
    heroes: &[&str],
    sizes: &mut std::collections::BTreeSet<String>,
) -> Option<usize> {
    use core::unit::HitZone;
    let root = game::assets::baked_root();
    let dir = root.join(map);
    let Ok(mut scene) = game::assets::load_scene(&dir) else {
        eprintln!("SKIP: no {map} bake");
        return None;
    };
    scene.apply_mode_layer("con");
    scene.merge_common(game::assets::load_common(&root.join(format!("common_{game}"))).ok()?);
    let factions = ["all", "cis", "imp", "rep"];
    let db = game::roster::load_classdb(&root.join("classes"), game, &factions).ok()?;
    let mut sim = core::sim::Sim::empty(42);
    sim.ground_rules = if game == "bf2" {
        core::soldier::GroundRules::Bf2
    } else {
        core::soldier::GroundRules::Bf1
    };
    scene.install_collision(&mut sim);
    let mut out = 0;
    for pair in [["rep", "cis"], ["all", "imp"]] {
        let f = pair.map(str::to_owned);
        let mut sim = sim.clone();
        let Ok(build) =
            game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
        else {
            continue;
        };
        let mut names = Vec::new();
        let classes: Vec<_> = factions
            .iter()
            .flat_map(|f| game::roster::default_classes(game, f).unwrap_or(&[]))
            .chain(heroes)
            .filter(|c| game::roster::is_soldier_chain(&db, c))
            .filter_map(|c| game::roster::ClassRoster::build(&db, c, 1, None).ok())
            .collect();
        let team = |sim: &mut core::sim::Sim,
                    names: &mut Vec<_>,
                    t: usize,
                    classes: Vec<game::roster::ClassRoster>| {
            let table: Vec<_> = classes.iter().map(|c| c.register(sim, names)).collect();
            let visuals = classes
                .iter()
                .map(game::walk::ClassVisual::from_roster)
                .collect();
            let faction = game::roster::FactionRoster {
                faction: "cis".into(),
                classes,
            };
            sim.classes[t] = table;
            game::walk::TeamTemplate::from_roster(&faction, &sim.classes[t][0], visuals)
        };
        let t0 = team(&mut sim, &mut names, 0, classes);
        let sniper = default_sniper(&db, game);
        let t1 = team(&mut sim, &mut names, 1, vec![sniper]);
        let far = Vec3::new(0.0, 500.0, 0.0);
        let mut walk = game::walk::WalkSim::with_teams_seat(
            sim,
            scene.heightfield(),
            far,
            [t0.clone(), t1],
            None,
            1,
        );
        walk.vehicle_seats = build.seats.clone();
        walk.vehicle_riders = build.riders.clone();
        for i in 0..walk.sim.building_turrets.len() {
            walk.sim.repair_building(i as u16, 1.0e6);
        }
        walk.step_tick(&[PadState::default()]);
        game::seated_bodies::install(&mut walk, &root, game);
        let mut vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, game, &walk.teams);
        vis.set_turrets(game::viewer::turret_pose::rigged_turrets(&scene));
        let turrets: Vec<_> = walk
            .sim
            .alive_vehicles()
            .filter(|(_, v)| v.kind == core::vehicle::VehicleKind::Turret)
            .filter(|(_, v)| {
                walk.sim.seat_setup[v.class as usize]
                    .as_ref()
                    .and_then(|s| s.guns.first()?.as_ref())
                    .is_some_and(|g| g.gunner.is_some())
            })
            .map(|(h, v)| (h, v.class))
            .collect();
        // Every vehicle seat that shows its rider (a speeder bike, the
        // fighter tanks' gunners, the AT-TE's, the LAAT's door gunner): his
        // head prim on his drawn head, met from his side.
        let vehicles: Vec<_> = walk
            .sim
            .alive_vehicles()
            .filter(|(_, v)| v.kind != core::vehicle::VehicleKind::Turret)
            .map(|(h, v)| (h, v.class))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        for (vh, vc) in vehicles {
            if !seen.insert(vc) {
                continue;
            }
            for (si, seat) in build.seats[vc as usize].iter().enumerate() {
                if seat.gunner_draw(game == "bf2") != game::vehicle_class::GunnerDraw::Drawn {
                    continue;
                }
                for (k, class) in t0.classes.iter().enumerate() {
                    let pos = walk.sim.vehicle(vh).unwrap().pos;
                    let spec = t0.spec(0, pos + Vec3::Y * 5.0, 0.0, Controller::Idle, k as u8);
                    let Some(g) = walk.sim.spawn(&spec) else {
                        continue;
                    };
                    if walk.sim.seat_unit(g, vh, si as u8) {
                        for _ in 0..3 {
                            walk.step_tick(&[PadState::default()]);
                            vis.update(&walk, core::TICK_DT);
                        }
                        let drawn = vis
                            .drawn_body(&walk, g.index as usize)
                            .and_then(|(_, b)| b.iter().find(|x| x.0 == "bone_head").map(|x| x.1));
                        if let (Some(head), Some(p)) = (drawn, walk.sim.body_prims(g)) {
                            let case = format!(
                                "{map} {} seat {si} {}",
                                build.classes[vc as usize], class.name
                            );
                            let gap = prim_head(&p).distance(head);
                            assert!(
                                gap < 0.08,
                                "{case}: head prim {gap:.3} m off the drawn head"
                            );
                            let (sy, cy) = p.yaw.sin_cos();
                            let right = Vec3::new(cy, 0.0, -sy);
                            let z = p.hit(head + right * 20.0, -right, 40.0).map(|h| h.1);
                            assert_eq!(z, Some(HitZone::Head), "{case}: at the drawn head");
                            out += 1;
                        }
                        walk.sim.unseat_unit(g);
                    }
                    walk.sim.unit_mut(g).unwrap().alive = false;
                }
            }
        }
        // One turret of each class.
        let mut seen = std::collections::BTreeSet::new();
        for (turret, vclass) in turrets {
            if !seen.insert(vclass) {
                continue;
            }
            let tname = &build.classes[vclass as usize];
            for (k, class) in t0.classes.iter().enumerate() {
                let tpos = walk.sim.vehicle(turret).unwrap().pos;
                let Some(g) = walk.sim.spawn(&t0.spec(
                    0,
                    tpos + Vec3::Y * 5.0,
                    0.0,
                    Controller::Idle,
                    k as u8,
                )) else {
                    continue;
                };
                if !walk.sim.seat_unit(g, turret, 0) {
                    walk.sim.unit_mut(g).unwrap().alive = false;
                    continue;
                }
                let scale = format!("{:.2}", class.anim.root_scale);
                for (yaw, pitch) in [
                    (0.0, 0.0),
                    (1.2, 0.0),
                    (0.0, 0.35),
                    (-0.8, -0.3),
                    (0.0, 0.6),
                ] {
                    {
                        let v = walk.sim.vehicle_mut(turret).unwrap();
                        if v.guns.is_empty() {
                            v.guns.resize(1, None);
                        }
                        let rest = (yaw, pitch);
                        let gs = v.guns[0].get_or_insert_with(|| core::vehicle::gun::GunState {
                            target: None,
                            look: rest,
                            aim: rest,
                            rate: (0.0, 0.0),
                            rel: rest,
                            weapons: Vec::new(),
                            zoom: None,
                        });
                        gs.look = rest;
                        gs.aim = rest;
                        gs.rel = rest;
                    }
                    // The draw holds an aimer-hung gunner a tick back.
                    for _ in 0..3 {
                        walk.step_tick(&[PadState::default()]);
                        vis.update(&walk, core::TICK_DT);
                    }
                    let Some((_, bones)) = vis.drawn_body(&walk, g.index as usize) else {
                        // Not drawn in this seat: nothing to match.
                        break;
                    };
                    let bone = |n: &str| bones.iter().find(|b| b.0 == n).map(|b| b.1);
                    let (Some(head), Some(rib), Some(pelvis)) =
                        (bone("bone_head"), bone("bone_ribcage"), bone("bone_pelvis"))
                    else {
                        break;
                    };
                    // His chest: between the ribcage and pelvis bones (on
                    // Grievous the ribcage bone sits 0.13 m under his head).
                    let chest = rib.lerp(pelvis, 0.5);
                    let p = walk.sim.body_prims(g).expect("a seated gunner has a body");
                    let (sy, cy) = p.yaw.sin_cos();
                    let right = Vec3::new(cy, 0.0, -sy);
                    let zone = |at: Vec3, from: Vec3| {
                        let start = at + from * 20.0;
                        p.hit(start, -from, 40.0).map(|h| h.1)
                    };
                    let case = format!(
                        "{map} {tname} {} (root {scale}) look ({yaw}, {pitch}): head prim {:.3} off the drawn head",
                        class.name,
                        prim_head(&p).distance(head)
                    );
                    // From either side and from over his head along his
                    // spine: views where his own chest does not cover his
                    // head (a reclined AA gunner's does, seen from in front).
                    let crown = (head - chest).normalize();
                    for from in [right, -right, crown] {
                        assert_eq!(
                            zone(head, from),
                            Some(HitZone::Head),
                            "{case}: at the drawn head"
                        );
                    }
                    for from in [right, -right] {
                        let z = zone(chest, from);
                        assert!(
                            matches!(z, Some(HitZone::Torso | HitZone::Body)),
                            "{case}: at the drawn chest {z:?}"
                        );
                    }
                    sizes.insert(scale.clone());
                    out += 1;
                }
                walk.sim.unseat_unit(g);
                walk.sim.unit_mut(g).unwrap().alive = false;
            }
        }
    }
    Some(out)
}

fn default_sniper(db: &game::classdb::ClassDb, game: &str) -> game::roster::ClassRoster {
    let name = if game == "bf2" {
        "rep_inf_ep3_sniper"
    } else {
        "rep_inf_clone_sharpshooter"
    };
    game::roster::ClassRoster::build(db, name, 1, None).unwrap()
}
