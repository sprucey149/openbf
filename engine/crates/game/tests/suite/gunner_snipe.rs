//! A scoped shot at a turret gunner's head (playtest 2026-10-29, BF2 Kamino:
//! scoped on a droid's head and gun showing over a low block 33 m off, the
//! shots did nothing). The droid (unit 11, `cis_inf_sniper`) was the gunner
//! of the `tur_weap_built_gunturret` at (-45, 60.275, 41); the low block is
//! the turret's own wall. The match log's shots (ticks 1356, 1484, 1563,
//! 1939) passed 0.09-0.14 m from the drawn head bone, 1.85-1.89 m over the
//! building's origin, and flew on to a building 47 m out.
//!
//! The cause: the sim stood every rider at its vehicle's origin, while a
//! drawn gunner is drawn - and on the PS2 is a body - at his seat
//! (`analysis/re-turret-collision.md` 3.2, `re-turret-draw.md` 3.2). This
//! gunner's seat is 0.26 m over the origin: his head prim (1.65 m over his
//! feet, r 0.17) topped out at 1.82 m, under the ray. With his body at his
//! seat every logged shot strikes his head.
//!
//! Real map, classes and soldier models from the bake; skips without them.

use core::sim::{buttons, Event, PadState};
use core::unit::{Controller, UnitHandle};
use glam::Vec3;

const TURRET: Vec3 = Vec3::new(-45.0, 60.275, 41.0);

/// (tick, the player's feet from the black box, the shot's yaw and pitch
/// from the match log).
const SHOTS: [(u32, Vec3, f32, f32); 4] = [
    (1356, Vec3::new(-21.84, 61.26, 64.89), 0.7668, -0.0251),
    (1484, Vec3::new(-22.01, 61.25, 64.71), 0.7668, -0.0251),
    (1563, Vec3::new(-22.01, 61.25, 64.71), 0.7668, -0.0237),
    (1939, Vec3::new(-22.01, 61.25, 64.71), 0.7685, -0.0237),
];

struct Scene {
    walk: game::walk::WalkSim,
    vis: game::soldier_vis::SoldierVisuals,
    gunner: UnitHandle,
    turret: core::vehicle::VehicleHandle,
    at: Vec3,
    /// Where the gun points; `None` = at the player.
    aim_at: Option<Vec3>,
}

/// The map with the player at `start` and a `class` droid seated in the
/// building turret at `at`; `crouched` = he boards crouched, facing `yaw`.
fn scene(at: Vec3, class: &str, start: Vec3, crouched: Option<f32>) -> Option<Scene> {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_kam1");
    let Ok(mut scene) = game::assets::load_scene(&dir) else {
        eprintln!("SKIP: no bf2_kam1 bake");
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
    for t in &mut sim.battle.teams {
        t.reinforcements = 200.0;
    }
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
    let mut walk = game::walk::WalkSim::with_teams_seat(sim, hf, start, [t0.clone(), t1], None, 1);
    walk.vehicle_seats = build.seats.clone();
    // Aimed by hand, as logged: no magnet.
    walk.sim.assist_intensity = 0.0;
    let turret = walk
        .sim
        .alive_vehicles()
        .find(|(_, v)| v.pos.distance(at) < 0.1)
        .map(|(h, _)| h)?;
    let v = walk.sim.vehicle(turret)?;
    assert_eq!(
        build.classes[v.class as usize], "tur_weap_built_gunturret",
        "the turret by the spot"
    );
    let gunner = walk
        .sim
        .spawn(&t0.spec(0, at + Vec3::Y * 5.0, 0.0, Controller::Idle, 0))
        .unwrap();
    if let Some(yaw) = crouched {
        let g = walk.sim.unit_mut(gunner).unwrap();
        g.soldier.stance = core::soldier::Stance::Crouch;
        g.soldier.yaw = yaw;
    }
    assert!(walk.sim.seat_unit(gunner, turret, 0));
    // He takes every shot and lives.
    let gu = walk.sim.unit_mut(gunner).unwrap();
    gu.health.max = 1.0e9;
    gu.health.cur = 1.0e9;
    walk.sim.cheats.invincible = true;
    walk.sim.cheats.infinite_ammo = true;
    let mut vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, "bf2", &walk.teams);
    vis.set_turrets(game::viewer::turret_pose::rigged_turrets(&scene));
    Some(Scene {
        walk,
        vis,
        gunner,
        turret,
        at,
        aim_at: None,
    })
}

/// Turn the gunner's gun at the player, as the droid shooting at him had it
/// (or at `Scene::aim_at`).
fn face_player(s: &mut Scene) {
    let me = s
        .aim_at
        .unwrap_or_else(|| s.walk.sim.unit(s.walk.player).unwrap().soldier.pos);
    let v = s.walk.sim.vehicle_mut(s.turret).unwrap();
    let d = v.rot.inverse() * (me - s.at);
    let look = (d.x.atan2(d.z), 0.0);
    if v.guns.is_empty() {
        v.guns.resize(1, None);
    }
    let g = v.guns[0].get_or_insert_with(|| core::vehicle::gun::GunState {
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

fn tick(s: &mut Scene, buttons: u32) {
    face_player(s);
    s.walk.step_tick(&[PadState {
        buttons,
        ..Default::default()
    }]);
    s.vis.update(&s.walk, core::TICK_DT);
}

/// Each logged shot, from the logged spot along the logged aim, scoped,
/// strikes the gunner in the head (it did nothing in play); the ray passes
/// within 0.2 m of his drawn head bone, and his body stands under his drawn
/// one (feet within 5 cm of the drawn model's origin).
#[test]
fn a_scoped_shot_at_a_turret_gunners_drawn_head_hits_it() {
    let Some(mut s) = scene(TURRET, "cis_inf_sniper", SHOTS[0].1, None) else {
        return;
    };
    let me = s.walk.player;
    let mut zoomed = false;
    for (t, feet, yaw, pitch) in SHOTS {
        let u = s.walk.sim.unit_mut(me).unwrap();
        u.soldier.pos = feet;
        u.soldier.vel = glam::Vec3::ZERO;
        for _ in 0..40 {
            let u = s.walk.sim.unit_mut(me).unwrap();
            u.soldier.yaw = yaw;
            u.soldier.pitch = pitch;
            tick(&mut s, 0);
        }
        if !zoomed {
            tick(&mut s, buttons::ZOOM);
            for _ in 0..20 {
                tick(&mut s, 0);
            }
            zoomed = true;
        }
        let u = s.walk.sim.unit(me).unwrap();
        assert!(u.scoped(), "scoped");
        let (eye, dir) = u.aim_ray(s.walk.sim.ground_rules, s.walk.sim.views());
        let g = s.walk.sim.unit(s.gunner).unwrap().soldier.pos;
        let (_, bones) = s
            .vis
            .drawn_body(&s.walk, s.gunner.index as usize)
            .expect("the gunner is drawn");
        let head = bones.iter().find(|b| b.0 == "bone_head").unwrap().1;
        let v = head - eye;
        let off = (v - dir * v.dot(dir)).length();
        let drawn = s
            .vis
            .drawn(&s.walk)
            .into_iter()
            .find(|d| d.0 == s.gunner.index as usize)
            .unwrap()
            .1
            .w_axis
            .truncate();
        println!(
            "tick {t}: ray {:.3} m off the drawn head ({head:.3}); body feet {g:.3}, drawn {drawn:.3}",
            off
        );
        let mut hit = None;
        for k in 0..20 {
            let u = s.walk.sim.unit_mut(me).unwrap();
            u.soldier.yaw = yaw;
            u.soldier.pitch = pitch;
            tick(&mut s, if k == 0 { buttons::FIRE } else { 0 });
            for e in &s.walk.sim.events {
                if let Event::Hit { victim, damage, .. } = *e {
                    if victim == s.gunner {
                        hit = Some(damage);
                    }
                }
            }
            if hit.is_some() {
                break;
            }
        }
        println!("tick {t}: hit {hit:?}");
        // The sniper round's 280, x3 on the head.
        assert_eq!(hit, Some(840.0), "tick {t}: a head hit on the gunner");
        assert!(off < 0.2, "tick {t}: aimed at the drawn head");
        assert!(
            g.distance(drawn) < 0.05,
            "tick {t}: the body under the drawn gunner"
        );
    }
}

/// The second turret (black box 2026-10-29, F9 at tick 3256): the same
/// class at (-17, 60.23, 13), its gunner a `cis_inf_rifleman` (unit 12) who
/// boarded crouched (stance 1 in every frame, yaw 2.61). Scoped shots at
/// ticks 3110, 3141, 3174 and 3208 did nothing, all from the F9 spot along
/// the F9 focus (dir 0.47, 0.003, -0.883: yaw -0.4893, pitch 0.003). The
/// other misses (2890, 2921, 3078) have only the rows' 2-decimal aim.
const TURRET2: Vec3 = Vec3::new(-17.0, 60.23, 13.0);
const SHOTS2: [(u32, Vec3, f32, f32); 1] = [(3110, Vec3::new(-31.07, 60.3, 39.33), -0.4893, 0.003)];

/// A crouched droid who takes the seat sits in the seat's pose: the logged
/// shots through his drawn head strike it. They flew on with his body's
/// prims still crouched (head 1.2 m over his feet, drawn head 1.73 m).
#[test]
fn a_scoped_shot_at_a_gunner_who_boarded_crouched_hits_his_drawn_head() {
    let Some(mut s) = scene(TURRET2, "cis_inf_rifleman", SHOTS2[0].1, Some(2.61)) else {
        return;
    };
    let me = s.walk.player;
    let mut zoomed = false;
    for (t, feet, yaw, pitch) in SHOTS2 {
        let aim = |s: &mut Scene| {
            let u = s.walk.sim.unit_mut(me).unwrap();
            u.soldier.pos = feet;
            u.soldier.vel = Vec3::ZERO;
            u.soldier.yaw = yaw;
            u.soldier.pitch = pitch;
        };
        for _ in 0..40 {
            aim(&mut s);
            tick(&mut s, 0);
        }
        if !zoomed {
            tick(&mut s, buttons::ZOOM);
            for _ in 0..20 {
                aim(&mut s);
                tick(&mut s, 0);
            }
            zoomed = true;
        }
        let u = s.walk.sim.unit(me).unwrap();
        assert!(u.scoped(), "scoped");
        let (eye, dir) = u.aim_ray(s.walk.sim.ground_rules, s.walk.sim.views());
        let g = s.walk.sim.unit(s.gunner).unwrap();
        let (_, bones) = s
            .vis
            .drawn_body(&s.walk, s.gunner.index as usize)
            .expect("the gunner is drawn");
        let head = bones.iter().find(|b| b.0 == "bone_head").unwrap().1;
        let v = head - eye;
        let off = (v - dir * v.dot(dir)).length();
        println!(
            "tick {t}: ray {off:.3} m off the drawn head ({head:.3}, {:.2} over his feet {:.3}); stance {:?}",
            head.y - g.soldier.pos.y,
            g.soldier.pos,
            g.soldier.stance
        );
        let mut hit = None;
        for k in 0..20 {
            aim(&mut s);
            tick(&mut s, if k == 0 { buttons::FIRE } else { 0 });
            for e in &s.walk.sim.events {
                if let Event::Hit { victim, damage, .. } = *e {
                    if victim == s.gunner {
                        hit = Some(damage);
                    }
                }
            }
            if hit.is_some() {
                break;
            }
        }
        println!("tick {t}: hit {hit:?}");
        assert_eq!(hit, Some(840.0), "tick {t}: a head hit on the gunner");
        assert!(off < 0.2, "tick {t}: aimed at the drawn head");
    }
}

/// Unit 27, the gunner's target from tick 3120 (black box frames): at
/// each of the last three shots, and between frames 3135 and 3150 at 3141.
const SHOTS27: [(u32, Vec3); 3] = [
    (3141, Vec3::new(-7.9, 60.6, 13.6)),
    (3174, Vec3::new(-9.33, 61.08, 14.54)),
    (3208, Vec3::new(-9.33, 61.08, 14.54)),
];

/// The horizontal gap between the drawn head bone and the centre of the
/// body's standing head prim (0.16 m ahead of the feet along his yaw).
fn head_gap(s: &Scene) -> (f32, Vec3) {
    let g = s.walk.sim.unit(s.gunner).unwrap();
    let (_, bones) = s
        .vis
        .drawn_body(&s.walk, s.gunner.index as usize)
        .expect("the gunner is drawn");
    let head = bones.iter().find(|b| b.0 == "bone_head").unwrap().1;
    let y = g.soldier.yaw;
    let prim = g.soldier.pos + Vec3::new(-y.sin(), 0.0, -y.cos()) * 0.16;
    let d = head - prim;
    (Vec3::new(d.x, 0.0, d.z).length(), g.soldier.pos)
}

/// A drawn gunner's body faces his drawn one: with the gun at the player,
/// at unit 27 (as logged from tick 3120, about 90 degrees off) and at the
/// player turned 90 degrees the other way, the body's head prim stands
/// over the drawn head bone. It kept the yaw he boarded with (2.61).
#[test]
fn a_drawn_gunners_body_turns_with_his_gun() {
    let Some(mut s) = scene(TURRET2, "cis_inf_rifleman", SHOTS2[0].1, Some(2.61)) else {
        return;
    };
    let me = s.walk.player;
    let off = SHOTS2[0].1 - TURRET2;
    let left = TURRET2 + Vec3::new(off.z, 0.0, -off.x);
    for (name, at) in [
        ("the player", None),
        ("unit 27", Some(SHOTS27[1].1)),
        ("90 deg off", Some(left)),
    ] {
        s.aim_at = at;
        for _ in 0..10 {
            let u = s.walk.sim.unit_mut(me).unwrap();
            u.soldier.pos = SHOTS2[0].1;
            u.soldier.vel = Vec3::ZERO;
            tick(&mut s, 0);
        }
        let (gap, feet) = head_gap(&s);
        let yaw = s.walk.sim.unit(s.gunner).unwrap().soldier.yaw;
        println!("gun at {name}: feet {feet:.3}, yaw {yaw:.3}, head gap {gap:.3} m");
        assert!(
            gap < 0.05,
            "gun at {name}: head prim {gap:.3} m off the drawn head"
        );
    }
}

/// The last three logged misses (3141, 3174, 3208) with the gun at unit 27
/// as logged: each strikes the gunner's head (the seat-entry stance alone
/// already lets them; his head prim now also stands under the drawn head).
#[test]
fn the_shots_with_the_gun_at_unit_27_hit_the_gunners_head() {
    let Some(mut s) = scene(TURRET2, "cis_inf_rifleman", SHOTS2[0].1, Some(2.61)) else {
        return;
    };
    let me = s.walk.player;
    let (_, feet, yaw, pitch) = SHOTS2[0];
    let aim = |s: &mut Scene| {
        let u = s.walk.sim.unit_mut(me).unwrap();
        u.soldier.pos = feet;
        u.soldier.vel = Vec3::ZERO;
        u.soldier.yaw = yaw;
        u.soldier.pitch = pitch;
    };
    s.aim_at = Some(SHOTS27[0].1);
    for _ in 0..40 {
        aim(&mut s);
        tick(&mut s, 0);
    }
    tick(&mut s, buttons::ZOOM);
    for (t, at) in &SHOTS27 {
        s.aim_at = Some(*at);
        // The rifle has cycled since the last shot.
        for _ in 0..40 {
            aim(&mut s);
            tick(&mut s, 0);
        }
        let (gap, g) = head_gap(&s);
        let (mut hit, mut fired) = (None, false);
        for k in 0..20 {
            aim(&mut s);
            tick(&mut s, if k == 0 { buttons::FIRE } else { 0 });
            for e in &s.walk.sim.events {
                match *e {
                    Event::Hit { victim, damage, .. } if victim == s.gunner => hit = Some(damage),
                    Event::Shot { unit, .. } if unit == me => fired = true,
                    _ => {}
                }
            }
            if hit.is_some() {
                break;
            }
        }
        assert!(fired, "tick {t}: the rifle fired");
        println!("tick {t}: gunner feet {g:.3} (logged -17.15, 60.49, 13.0), head gap {gap:.3}, hit {hit:?}");
        assert_eq!(hit, Some(840.0), "tick {t}: a head hit on the gunner");
    }
}
