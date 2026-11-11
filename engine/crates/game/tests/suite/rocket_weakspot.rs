//! Hitmark (the maintainer 2026-10-28, `analysis/re-target-hud.md` §12): the human
//! player's rocket launcher on a vehicle's weak spot - a hull prim its class
//! authors a `HitLocation` x4 for (`analysis/re-hitzones.md` §3: the hits
//! BF2's demolition award counts) - raises `Event::CriticalHit` and lights
//! the reticle's critical-hit mark; the same rocket on the hull elsewhere,
//! a rifle on the weak spot, or a bot's rocket on it does not.
//!
//! BF2 Geonosis, both sides: each pad vehicle whose hull carries a x4 part
//! (the Republic's AT-TE and fighter tank, the CIS hailfire and spider
//! walker), shot from a spot with a clear line at the part by a real
//! rocketeer / rifleman of the other side with his real weapon's round. BF1
//! authors no such zone (its only `HitLocation`s, the hailfire's and the
//! Republic transport's, are x0); the port gives its armoured vehicles BF2's
//! (`game::assets::BF1_WEAK_SPOTS`), tested at the end of this file with
//! the weak-part lock.
//!
//! The round is laid from the soldier's fire point straight at the part
//! (`Bolt::fire`, the class's own ordnance), not through the trigger: the
//! launcher's aim pipeline (reticle ray, fire delay) is not what is tested,
//! and a bot aims by its own think.

use core::collision::{HullFilter, Layers};
use core::projectile::{Bolt, Shooter, Target};
use core::sim::{Event, PadState, Sim};
use core::unit::{Controller, UnitHandle};
use core::vehicle::VehicleHandle;
use glam::Vec3;

struct World {
    sim: Sim,
    db: game::classdb::ClassDb,
    names: Vec<(String, usize)>,
    classes: Vec<String>,
}

fn world(map: &str, factions: [&str; 2]) -> Option<World> {
    let root = game::assets::baked_root();
    let dir = root.join(map);
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: {map} is not baked");
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
    let f = [factions[0].to_owned(), factions[1].to_owned()];
    let build =
        game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &root.join("classes"), &f)
            .expect("install");
    let game = scene.manifest.source_game.to_ascii_lowercase();
    let db = game::roster::load_classdb(&root.join("classes"), &game, &factions).unwrap();
    for t in &mut sim.battle.teams {
        t.reinforcements = 200.0;
    }
    // No posts here: every pad makes its own side's class (the first team it
    // has one for) on the first tick, empty and of no side.
    for p in &mut sim.vehicle_pads {
        p.team = p.team.filter(|&t| p.def.class_for(Some(t)).is_some());
        p.team = p
            .team
            .or_else(|| (0..2).find(|&t| p.def.class_for(Some(t)).is_some()));
    }
    sim.step(&[]);
    Some(World {
        sim,
        db,
        names: Vec::new(),
        classes: build.classes.clone(),
    })
}

fn spawn(w: &mut World, class: &str, team: u8, pos: Vec3, controller: Controller) -> UnitHandle {
    let roster = game::roster::ClassRoster::build(&w.db, class, 0, None).unwrap();
    let c = roster.register(&mut w.sim, &mut w.names);
    w.sim
        .spawn(&c.spec(team, pos, 0.0, controller, 0, 0))
        .unwrap()
}

/// The centres of `vh`'s hull parts in the ORDNANCE subset that take
/// damage, weak spots (factor over 1) or plain (factor 1).
fn parts(w: &World, vh: VehicleHandle, weak: bool) -> Vec<Vec3> {
    w.sim
        .collision
        .hull(vh)
        .iter()
        .filter(|b| b.layers.contains(Layers::ORDNANCE) && b.hit_mult > 0.0)
        .filter(|b| (b.hit_mult > 1.0) == weak)
        .map(|b| b.to_world.transform_point3(Vec3::ZERO))
        .collect()
}

/// Stand `u` on a spot from which a line out of his fire point meets `vh`'s
/// hull first at one of `aims`, on a weak spot (`weak`) or not: the fire
/// point and the direction, `None` when no spot near has one.
fn stand_in_line(
    w: &mut World,
    u: UnitHandle,
    vh: VehicleHandle,
    aims: &[Vec3],
    weak: bool,
) -> Option<(Vec3, Vec3)> {
    let floor = w.sim.vehicle(vh)?.pos.y;
    for &aim in aims {
        for dist in [12.0f32, 18.0, 25.0] {
            for k in 0..32 {
                let a = k as f32 * std::f32::consts::TAU / 32.0;
                let mut p = aim + Vec3::new(a.cos(), 0.0, a.sin()) * dist;
                p.y = w.sim.spawn_ground(p);
                if (p.y - floor).abs() > 4.0 {
                    continue;
                }
                let s = w.sim.unit_mut(u).unwrap();
                s.soldier.pos = p;
                let d = aim - p;
                s.soldier.yaw = (-d.x).atan2(-d.z);
                let from = w.sim.unit(u).unwrap().fire_point();
                let dir = (aim - from).normalize();
                let Some(h) = w.sim.collision.ray_with(
                    from,
                    dir,
                    (aim - from).length() + 1.0,
                    Layers::ORDNANCE,
                    HullFilter::All,
                ) else {
                    continue;
                };
                if h.owner == Some(vh)
                    && (h.hit_mult > 1.0) == weak
                    && h.hit_mult > 0.0
                    && core::perception::los_terrain(&w.sim, from, from + dir * h.t)
                {
                    return Some((from, dir));
                }
            }
        }
    }
    None
}

/// What one round did: the vehicle's drop, a critical-hit event on it, and
/// the HUD feed's mark for the human viewer.
struct Shot {
    drop: f32,
    critical: bool,
    mark: Option<[u8; 3]>,
}

/// `shooter`'s round of `weapon` (`None`: the one in his hand) from `from` along `dir`, followed for 2 s,
/// the feed watching `viewer`.
fn lay_round(
    w: &mut World,
    shooter: UnitHandle,
    viewer: UnitHandle,
    vh: VehicleHandle,
    weapon: Option<&str>,
    (from, dir): (Vec3, Vec3),
) -> Shot {
    let u = w.sim.unit(shooter).unwrap();
    let wc = weapon.map_or(u.weapon_class, |name| {
        w.sim
            .weapon_classes
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("{name} not loaded"))
    });
    let who = Shooter {
        owner: shooter,
        team: u.team,
        weapon_class: wc,
        is_player: u.is_player,
    };
    let mut feed =
        game::event_feed::EventFeed::new(game::event_feed::Texts::builtin(), ["rep", "cis"], &[]);
    let o = &w.sim.weapon_classes[wc].ordnance;
    let b = Bolt::fire(
        from,
        dir,
        o.velocity,
        o.life_span,
        who,
        w.sim.heightfield.as_ref(),
        w.sim.water_plane(),
        &w.sim.collision,
        HullFilter::All,
    );
    w.sim.bolts.push(b);
    let before = w.sim.vehicle(vh).unwrap().health.cur;
    let (mut critical, mut mark) = (false, None);
    for _ in 0..60 {
        w.sim.step(&[PadState::default()]);
        feed.tick(&w.sim, viewer);
        mark = mark.or(feed.hit_critical(w.sim.tick));
        critical |= w.sim.events.iter().any(|e| {
            matches!(e, Event::CriticalHit { shooter: s, victim, .. }
                if *s == shooter && *victim == Target::Vehicle(vh))
        });
    }
    let after = w.sim.vehicle(vh).map_or(0.0, |v| v.health.cur.max(0.0));
    Shot {
        drop: before - after,
        critical,
        mark,
    }
}

/// The side's rocketeer with his launcher, and its rifleman.
fn kit(side: &str) -> [(String, Option<String>); 2] {
    let class = |role: &str| match side {
        "rep" => format!("rep_inf_ep2_{role}"),
        _ => format!("{side}_inf_{role}"),
    };
    [
        (
            class("rocketeer"),
            Some(format!("{side}_weap_inf_rocket_launcher")),
        ),
        // His primary, the side's blaster rifle (a droid's own name).
        (class("rifleman"), None),
    ]
}

#[test]
fn a_rocket_on_a_vehicle_weak_spot_lights_the_critical_mark() {
    let factions = ["rep", "cis"];
    let Some(probe) = world("bf2_geo1", factions) else {
        return;
    };
    let mut cases: Vec<(String, VehicleHandle)> = Vec::new();
    for (vh, v) in probe.sim.alive_vehicles() {
        let c = probe.classes[v.class as usize].clone();
        if !parts(&probe, vh, true).is_empty() && !cases.iter().any(|(k, _)| *k == c) {
            cases.push((c, vh));
        }
    }
    let names: Vec<&str> = cases.iter().map(|(c, _)| c.as_str()).collect();
    eprintln!("bf2_geo1 classes with a weak spot: {names:?}");
    assert!(names.contains(&"rep_walk_atte"), "{names:?}");
    assert!(names.contains(&"cis_tread_hailfire"), "{names:?}");
    let mut lit = Vec::new();
    for (class, vh) in &cases {
        // The vehicle's side by its class prefix; the shooter is the other.
        let home = usize::from(class.starts_with("cis"));
        let team = (1 - home) as u8;
        let [(rocketeer, rocket), (rifleman, rifle)] = kit(factions[1 - home]);
        // Each round in a fresh world: the vehicle at full health.
        let one = |weak: bool, class_name: &str, weapon: Option<&str>, human: bool| {
            let mut w = world("bf2_geo1", factions).unwrap();
            let far = Vec3::new(5000.0, 0.0, 5000.0);
            let (me, viewer) = if human {
                let me = spawn(
                    &mut w,
                    class_name,
                    team,
                    far,
                    Controller::Player { slot: 0 },
                );
                (me, me)
            } else {
                // The human is elsewhere; the round is a bot's.
                let viewer = spawn(
                    &mut w,
                    class_name,
                    team,
                    far,
                    Controller::Player { slot: 0 },
                );
                (spawn(&mut w, class_name, team, far, Controller::Ai), viewer)
            };
            let aims = parts(&w, *vh, weak);
            let line = stand_in_line(&mut w, me, *vh, &aims, weak)?;
            let s = lay_round(&mut w, me, viewer, *vh, weapon, line);
            let held = &w.sim.weapon_classes[w.sim.unit(me).unwrap().weapon_class].name;
            Some((s, held.clone()))
        };
        let Some((hit, rocket)) = one(true, &rocketeer, rocket.as_deref(), true) else {
            eprintln!("{class}: no clear line at its weak spot");
            continue;
        };
        eprintln!(
            "{class} <- {rocket} on the weak spot: drop {}, event {}, mark {:?}",
            hit.drop, hit.critical, hit.mark
        );
        assert!(hit.drop > 0.0, "{class}: the rocket did damage");
        assert!(hit.critical, "{class}: the critical-hit event");
        // White on an empty vehicle of no side, red on one its pad gives
        // the other side (the AT-TE, a command walker, keeps its team).
        assert!(
            [ui::hud::target_color::NONE, ui::hud::target_color::ENEMY]
                .map(Some)
                .contains(&hit.mark),
            "{class}: the mark {:?}",
            hit.mark
        );
        lit.push(class.clone());

        if let Some((s, _)) = one(false, &rocketeer, Some(&rocket), true) {
            eprintln!("{class} <- {rocket} on the hull: drop {}", s.drop);
            assert!(s.drop > 0.0, "{class}: the hull rocket did damage");
            assert!(!s.critical, "{class}: the hull: no critical hit");
            assert_eq!(s.mark, None, "{class}: the hull: no mark");
        }
        let (s, rifle) = one(true, &rifleman, rifle.as_deref(), true).expect("the same line");
        eprintln!(
            "{class} <- {rifle} on the weak spot: drop {}, event {}",
            s.drop, s.critical
        );
        assert_eq!(
            s.critical,
            s.drop > 0.0,
            "{class}: the rifle's x4 that took health is a critical hit"
        );
        assert_eq!(s.mark, None, "{class}: but a rifle lights no mark");
        let (s, _) = one(true, &rocketeer, Some(&rocket), false).expect("the same line");
        eprintln!(
            "{class} <- a bot's {rocket} on the weak spot: drop {}, event {}",
            s.drop, s.critical
        );
        assert!(s.drop > 0.0, "{class}: the bot's rocket did damage");
        assert!(!s.critical, "{class}: a bot on Normal: no x4");
        assert_eq!(s.mark, None, "{class}: a bot's rocket: no mark");
    }
    eprintln!("lit: {lit:?}");
    assert!(lit.len() >= 2, "{lit:?} lit the mark");
}

/// Which weapons are rocket launchers (`WeaponClass::rocket_launcher`), on
/// both games' real classes: BF2 by `ScoreForMedalsType` 2 (the four sides'
/// infantry launchers, not the EMP launcher; the award / hero guided ones
/// carry the 2 too, but `weapon_from_db` builds no class for them - their
/// chain reaches `com_weap_null`), BF1 by name (the four sides' infantry launchers); never a
/// rifle, a sniper rifle, a wrist rocket or a mortar.
#[test]
fn the_rocket_launchers_of_both_games() {
    let root = game::assets::baked_root().join("classes");
    if !root.join("bf2_rep.ron").exists() {
        eprintln!("SKIP: no class bake");
        return;
    }
    let sides = ["rep", "cis", "all", "imp"];
    let check = |game: &str, yes: &[&str], no: &[&str]| {
        let db = game::roster::load_classdb(&root, game, &sides).unwrap();
        for (names, want) in [(yes, true), (no, false)] {
            for n in names {
                let w = game::roster::weapon_from_db(&db, n)
                    .unwrap_or_else(|e| panic!("{game} {n}: {e}"));
                assert_eq!(
                    w.rocket_launcher(),
                    want,
                    "{game} {n} ({:?})",
                    w.medals_type
                );
            }
        }
    };
    check(
        "bf2",
        &[
            "rep_weap_inf_rocket_launcher",
            "cis_weap_inf_rocket_launcher",
            "all_weap_inf_rocket_launcher",
            "imp_weap_inf_rocket_launcher",
        ],
        &[
            "rep_weap_inf_emp_launcher",
            "rep_weap_inf_rifle",
            "rep_weap_inf_sniper_rifle",
            "cis_weap_inf_wrist_rocket",
            "imp_weap_inf_mortar_launcher",
        ],
    );
    check(
        "bf1",
        &[
            "rep_weap_inf_rocket_launcher",
            "rep_weap_inf_torpedo_launcher",
            "cis_weap_inf_torpedo_launcher",
            "all_weap_inf_torpedo_launcher",
            "imp_weap_inf_torpedo_launcher",
        ],
        &[
            "rep_weap_inf_rifle",
            "cis_weap_inf_wrist_rocket",
            "imp_weap_inf_mortar_launcher",
            "all_weap_inf_mortar_launcher",
        ],
    );
}

/// BF1's four torpedo launchers take the on-foot lock with their own data:
/// a missile, vehicles only, BF2's 1.0 s LockTime (the shared numbers), and the 100 / 110 m
/// TargetLockMaxDistance / ...Lose defaults BF2's rocket launcher also uses.
/// The jet trooper's rocket (a cannon firing a bullet) and the wrist rocket
/// never lock.
#[test]
fn the_bf1_torpedo_launchers_take_the_foot_lock() {
    let root = game::assets::baked_root().join("classes");
    if !root.join("bf1_rep.ron").exists() {
        eprintln!("SKIP: no class bake");
        return;
    }
    let sides = ["rep", "cis", "all", "imp"];
    let db = game::roster::load_classdb(&root, "bf1", &sides).unwrap();
    let vehicle = 1u32 << core::weapon::HealthType::Vehicle as u32;
    for side in sides {
        let n = format!("{side}_weap_inf_torpedo_launcher");
        let w = game::roster::weapon_from_db(&db, &n).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert_eq!(w.lock_time, 1.0, "{n}");
        assert!(w.ordnance.missile.is_some(), "{n}");
        assert_eq!(w.target_kind, vehicle, "{n}");
        assert_eq!(w.target_lock_max, 100.0, "{n}");
        assert_eq!(w.target_lock_lose, 110.0, "{n}");
    }
    for n in ["rep_weap_inf_rocket_launcher", "cis_weap_inf_wrist_rocket"] {
        let w = game::roster::weapon_from_db(&db, n).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert_eq!(w.lock_time, 0.0, "{n}");
    }
}

// The weak-part lock (`analysis/re-ordnance-kinds.md` 5.1) on real data: a
// human's launcher locks a vehicle's weak part in sight, the rocket steers to
// it through the real trigger, and the hit there is the critical hit.

/// A map's conquest world with its mission's sides, `None` when unbaked.
fn mission_world(map: &str, mission: &str) -> Option<(World, game::mission::MissionDef)> {
    let path = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
        .join(format!("{mission}.ron"));
    let m = game::mission::MissionDef::load(&path).ok()?;
    let f = m.factions();
    let mut w = world(map, [f[0].as_str(), f[1].as_str()])?;
    w.sim.cheats.infinite_ammo = true;
    w.sim.cheats.no_reload = true;
    w.sim.cheats.invincible = true;
    Some((w, m))
}

/// Team `team`'s soldier who spawns with a lock launcher that takes
/// vehicles in his hands, as a human in slot 0.
fn launcher_human(w: &mut World, m: &game::mission::MissionDef, team: u8) -> Option<UnitHandle> {
    let vehicle = 1u32 << core::weapon::HealthType::Vehicle as u32;
    for (class, ..) in &m.teams[usize::from(team)].classes {
        let Ok(roster) = game::roster::ClassRoster::build(&w.db, class, 0, None) else {
            continue;
        };
        let c = roster.register(&mut w.sim, &mut w.names);
        let spec = c.spec(
            team,
            Vec3::new(5000.0, 0.0, 5000.0),
            0.0,
            Controller::Player { slot: 0 },
            0,
            0,
        );
        let wc = &w.sim.weapon_classes[spec.weapon_class];
        if wc.lock_time > 0.0 && wc.ordnance.missile.is_some() && wc.target_kind & vehicle != 0 {
            return w.sim.spawn(&spec);
        }
    }
    None
}

/// Put `u` at `p` on the ground looking at `at`.
fn look(w: &mut World, u: UnitHandle, p: Vec3, at: Vec3) {
    // A stand in a death region ends the walk (the survey stops there).
    let Some(s) = w.sim.unit_mut(u) else {
        return;
    };
    s.soldier.pos = p;
    s.soldier.vel = Vec3::ZERO;
    let d = at - (p + Vec3::Y * 1.7);
    s.soldier.yaw = (-d.x).atan2(-d.z);
    s.soldier.pitch = d.y.atan2(d.x.hypot(d.z));
}

/// What one stand saw and did.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
struct Stand {
    /// The lock took the vehicle.
    locked: bool,
    /// The lock box sat on a weak part on the tick before the rocket left.
    weak: bool,
    /// A rocket left.
    fired: bool,
    /// The rocket left steering to a part.
    part: bool,
    /// The vehicle's drop.
    drop: f32,
    critical: bool,
    mark: bool,
}

/// From `p`: lock `vh` by looking at it, dwell on the lock box to a full
/// lock and fire through the trigger; the vehicle is whole again after.
fn lock_and_fire(w: &mut World, me: UnitHandle, vh: VehicleHandle, p: Vec3) -> Stand {
    use core::sim::{AimTarget, LockTarget};
    let mut out = Stand::default();
    let centre = {
        let v = w.sim.vehicle(vh).unwrap();
        v.pos + v.rot * Vec3::Y * 2.0
    };
    look(w, me, p, centre);
    w.sim.step(&[PadState::default()]);
    let on = |w: &World| {
        w.sim
            .foot_lock_target(0)
            .filter(|(t, _)| *t == LockTarget::Aim(AimTarget::Vehicle(vh)))
            .map(|(_, at)| at)
    };
    let Some(at) = on(w) else {
        return out;
    };
    out.locked = true;
    look(w, me, p, at);
    for _ in 0..150 {
        if w.sim.foot_lock_charge(0) == Some(1.0) {
            break;
        }
        w.sim.step(&[PadState::default()]);
        let Some(at) = on(w) else {
            return out;
        };
        look(w, me, p, at);
    }
    if w.sim.foot_lock_charge(0) != Some(1.0) {
        return out;
    }
    let mut feed =
        game::event_feed::EventFeed::new(game::event_feed::Texts::builtin(), ["rep", "cis"], &[]);
    let before = w.sim.vehicle(vh).unwrap().health.cur;
    let fire = PadState {
        buttons: core::sim::buttons::FIRE,
        ..PadState::default()
    };
    let mut flown = false;
    for k in 0..240 {
        let weak = w.sim.foot_lock_weak(0);
        w.sim
            .step(&[if flown { PadState::default() } else { fire }]);
        look(w, me, p, at);
        let mine = w
            .sim
            .bolts
            .iter()
            .find(|b| b.owner == me)
            .and_then(|b| b.missile);
        if let (false, Some(m)) = (flown, mine) {
            flown = true;
            out.fired = true;
            out.weak = weak;
            out.part = m.part.is_some();
        }
        feed.tick(&w.sim, me);
        out.mark |= feed.hit_critical(w.sim.tick).is_some();
        out.critical |= w.sim.events.iter().any(|e| {
            matches!(e, Event::CriticalHit { shooter, victim, .. }
                if *shooter == me && *victim == Target::Vehicle(vh))
        });
        if (flown && mine.is_none()) || (!flown && k > 60) {
            break;
        }
    }
    let v = w.sim.vehicle_mut(vh).unwrap();
    out.drop = before - v.health.cur;
    v.health.cur = v.health.max;
    out
}

/// One class's stands around its pad vehicle.
#[derive(Default, Debug)]
struct Survey {
    stands: usize,
    locked: usize,
    weak: usize,
    critical: usize,
    /// Rockets sent at a part that never met the vehicle (the ground or a
    /// wall on the way).
    lost: usize,
    /// Shots where the box and the rocket disagreed about the part: the
    /// part came into or left sight on the tick of the shot.
    edge: usize,
    /// Drops of the critical hits and of the plain hits on a centre lock.
    weak_drop: Vec<f32>,
    plain_drop: Vec<f32>,
    marks: usize,
}

/// Walk `dirs` directions at each of `dists` m around every pad vehicle
/// with a weak part, once per class, with the other side's launcher.
fn survey(map: &str, mission: &str, dirs: usize, dists: &[f32]) -> Option<Vec<(String, Survey)>> {
    let (probe, _) = mission_world(map, mission)?;
    let mut cases: Vec<(String, VehicleHandle)> = Vec::new();
    for (vh, v) in probe.sim.alive_vehicles() {
        let c = probe.classes[v.class as usize].clone();
        if !parts(&probe, vh, true).is_empty() && !cases.iter().any(|(k, _)| *k == c) {
            cases.push((c, vh));
        }
    }
    let mut out = Vec::new();
    for (class, vh) in cases {
        let (mut w, m) = mission_world(map, mission)?;
        let side = class.split('_').next().unwrap_or_default();
        let home = usize::from(m.teams[1].faction == side);
        let team = (1 - home) as u8;
        let Some(me) = launcher_human(&mut w, &m, team) else {
            eprintln!("{map} {class}: team {team} fields no lock launcher");
            continue;
        };
        // An empty pad vehicle is of no side: give it its pad's. No one
        // critical hit may destroy it (a one-man AT-ST has 4000).
        let v = w.sim.vehicle_mut(vh).unwrap();
        v.team = Some(home as u8);
        v.health.max = 1.0e6;
        v.health.cur = v.health.max;
        let (centre, floor) = {
            let v = w.sim.vehicle(vh).unwrap();
            (v.pos, v.pos.y)
        };
        let mut s = Survey::default();
        for &dist in dists {
            for k in 0..dirs {
                let a = k as f32 * std::f32::consts::TAU / dirs as f32;
                let mut p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * dist;
                p.y = w.sim.spawn_ground(p);
                if (p.y - floor).abs() > 4.0 {
                    continue;
                }
                if w.sim.unit(me).is_none() {
                    continue;
                }
                s.stands += 1;
                let r = lock_and_fire(&mut w, me, vh, p);
                s.locked += usize::from(r.locked);
                if !r.fired {
                    continue;
                }
                s.edge += usize::from(r.weak != r.part);
                if r.part {
                    s.weak += 1;
                    if r.critical {
                        s.critical += 1;
                        s.weak_drop.push(r.drop);
                        s.marks += usize::from(r.mark);
                    } else {
                        assert_eq!(
                            r.drop, 0.0,
                            "{map} {class}: a part rocket's hit is critical"
                        );
                        s.lost += 1;
                    }
                } else if r.drop > 0.0 && !r.critical {
                    s.plain_drop.push(r.drop);
                }
            }
        }
        eprintln!(
            "{map} {class}: {} stands, {} locked, {} rockets to a weak part, {} critical \
             ({} marks), {} lost, {} edge; critical drops {:?}, plain drops {:?}",
            s.stands,
            s.locked,
            s.weak,
            s.critical,
            s.marks,
            s.lost,
            s.edge,
            span(&s.weak_drop),
            span(&s.plain_drop),
        );
        out.push((class, s));
    }
    Some(out)
}

/// The least and the greatest value.
fn span(v: &[f32]) -> Option<(f32, f32)> {
    let lo = v.iter().copied().reduce(f32::min)?;
    Some((lo, v.iter().copied().fold(lo, f32::max)))
}

/// On both games' Geonosis, each pad vehicle with a weak part: from the
/// stands where the part is in sight the lock box sits on it, the rocket
/// leaves steering to it, and its hits there are critical (the event and
/// the reticle mark) at about four times the plain hit of a centre lock.
#[test]
fn a_locked_rocket_finds_the_weak_spot() {
    let mut ran = 0;
    for (map, mission, want) in [
        (
            "bf2_geo1",
            "bf2_geo1_con",
            &["rep_walk_atte", "cis_tread_hailfire"][..],
        ),
        (
            "bf1_geo1",
            "bf1_geo1_con",
            &["cis_tread_hailfire", "cis_walk_spider"][..],
        ),
    ] {
        let Some(all) = survey(map, mission, 16, &[18.0]) else {
            continue;
        };
        ran += 1;
        for class in want {
            let (_, s) = all
                .iter()
                .find(|(c, _)| c == class)
                .unwrap_or_else(|| panic!("{map}: no {class} surveyed"));
            assert!(s.weak > 0, "{map} {class}: the part is in sight somewhere");
            assert_eq!(
                s.critical + s.lost,
                s.weak,
                "{map} {class}: a part rocket that meets the vehicle is critical"
            );
            assert!(s.critical > 0, "{map} {class}: none arrived");
            assert_eq!(s.marks, s.critical, "{map} {class}: each lights the mark");
            assert_eq!(
                s.edge, 0,
                "{map} {class}: the box showed where the rocket went"
            );
            // BF2's rocket grows from 500 to 1500 over its first half
            // second, so the spans are compared, not one pair of shots
            // (the exact x4 is `core`'s `sim::weak_lock` test).
            if let (Some(weak), Some(plain)) = (span(&s.weak_drop), span(&s.plain_drop)) {
                assert!(weak.0 > 2.5 * plain.1, "{map} {class}: {weak:?} {plain:?}");
            }
        }
    }
    eprintln!("{ran} of 2 maps checked");
}

/// The survey behind the note's table (`analysis/re-ordnance-kinds.md`
/// 5.1): every class with a weak part, 24 directions at 15 m and 30 m.
/// Run by name with `--ignored --nocapture`.
#[test]
#[ignore = "a survey that prints; run by name"]
fn weak_part_lock_survey() {
    let mut seen: Vec<String> = Vec::new();
    for (map, mission) in [
        ("bf2_geo1", "bf2_geo1_con"),
        ("bf2_hot1", "bf2_hot1_con"),
        ("bf2_nab2", "bf2_nab2_con_cw"),
        ("bf2_kas2", "bf2_kas2_con_cw"),
        ("bf2_kas2", "bf2_kas2_con"),
        ("bf2_fel1", "bf2_fel1_con_cw"),
        ("bf2_myg1", "bf2_myg1_con_cw"),
        ("bf2_uta1", "bf2_uta1_con_cw"),
        ("bf2_end1", "bf2_end1_con"),
        ("bf1_geo1", "bf1_geo1_con"),
        ("bf1_hot1", "bf1_hot1_con"),
        ("bf1_nab1", "bf1_nab1_con_cw"),
        ("bf1_kas1", "bf1_kas1_con_cw"),
        ("bf1_kas1", "bf1_kas1_con"),
        ("bf1_kas2", "bf1_kas2_con_cw"),
        ("bf1_rhn1", "bf1_rhn1_con"),
        ("bf1_rhn2", "bf1_rhn2_con_cw"),
        ("bf1_yav1", "bf1_yav1_con"),
        ("bf1_end1", "bf1_end1_con"),
    ] {
        for (class, _) in survey(map, mission, 24, &[15.0, 30.0]).unwrap_or_default() {
            let key = format!("{} {class}", &map[..3]);
            if !seen.contains(&key) {
                seen.push(key);
            }
        }
    }
    eprintln!("surveyed: {seen:?}");
}

/// Every BF1 class in the port's weak-spot tables reaches its hull with a
/// x4 part a round can strike, one per named prim or added part; the
/// hailfire's authored x0 is gone, the Republic transport keeps its own.
/// BF2's classes take their rows from their own data alone.
#[test]
fn bf1_vehicles_have_their_weak_spots() {
    use game::assets::{BF1_ADDED_WEAK_SPOTS, BF1_WEAK_SPOTS, WEAK_SPOT_FACTOR};
    let root = game::assets::baked_root();
    if !root.join("classes/bf1_rep.ron").exists() || !root.join("side_bf1").exists() {
        eprintln!("SKIP: no BF1 class or side bake");
        return;
    }
    let sides = ["all", "imp", "rep", "cis"];
    let db = game::roster::load_classdb(&root.join("classes"), "bf1", &sides).unwrap();
    let files = game::vehicle_pads::side_vehicle_files(&root.join("bf1_geo1"), "bf1");
    // A class's hull as the pads build it, weak parts counted.
    let weak = |class: &str| {
        let (dir, vm) = files
            .iter()
            .find(|(_, vm)| vm.vehicles.iter().any(|v| v.class.name == class))
            .unwrap_or_else(|| panic!("{class}: in no side bake"));
        let k = &vm
            .vehicles
            .iter()
            .find(|v| v.class.name == class)
            .unwrap()
            .class;
        let g = k.geometry.as_deref().unwrap();
        let e = vm
            .collision
            .iter()
            .find(|e| e.geometry.eq_ignore_ascii_case(g))
            .unwrap();
        let cg = game::assets::load_collision_geom(dir, e).unwrap();
        let mut parts =
            game::assets::hull_parts(&cg, &k.collision, &game::assets::hit_locations(&db, class));
        parts.extend(game::assets::added_weak_parts(&db, class));
        assert!(
            parts.iter().all(|p| p.hit_mult == 1.0
                || (p.hit_mult == WEAK_SPOT_FACTOR && p.layers.contains(Layers::ORDNANCE))),
            "{class}: a factor that is neither 1 nor a strikable x4"
        );
        parts.iter().filter(|p| p.hit_mult > 1.0).count()
    };
    for (class, prims) in BF1_WEAK_SPOTS {
        assert_eq!(weak(class), prims.len(), "{class}");
    }
    for w in BF1_ADDED_WEAK_SPOTS {
        assert_eq!(weak(w.class), 1, "{}", w.class);
    }
    // No factor for the rest: a bike, an animal, a flyer.
    for class in [
        "imp_hover_speederbike",
        "all_walk_tauntaun",
        "all_fly_xwing",
    ] {
        assert_eq!(weak(class), 0, "{class}");
    }
    assert_eq!(
        game::assets::hit_locations(&db, "rep_fly_vtrans"),
        [("p_crithit".to_owned(), 0.0)]
    );
    assert_eq!(
        game::assets::hit_locations(&db, "cis_tread_hailfire"),
        [("p_crithit".to_owned(), WEAK_SPOT_FACTOR)]
    );
    // BF2: its own lines, nothing added.
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &sides).unwrap();
    let mut authored = 0;
    for w in BF1_ADDED_WEAK_SPOTS {
        assert!(game::assets::added_weak_parts(&db, w.class).is_empty());
    }
    for class in [
        "all_hover_combatspeeder",
        "all_hover_bos_combatspeeder",
        "cis_hover_aat",
        "cis_tread_hailfire",
        "cis_tread_snailtank",
        "cis_walk_spider",
        "cis_walk_spiderkas",
        "imp_hover_fightertank",
        "imp_walk_atat",
        "imp_walk_atst",
        "imp_walk_atst_jungle",
        "imp_walk_atst_snow",
        "rep_hover_fightertank",
        "rep_walk_atte",
        "rep_walk_oneman_atst",
    ] {
        let rows = game::assets::hit_locations(&db, class);
        assert!(!rows.is_empty(), "{class}");
        assert!(rows.iter().all(|(_, m)| *m == 4.0), "{class}: {rows:?}");
        authored += rows.len();
    }
    assert_eq!(authored, 17, "15 classes, the two spiders with two each");
}
