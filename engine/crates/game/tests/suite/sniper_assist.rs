//! Sniper aim assist (`[ours]`, the maintainer 2026-10-28: "give bf1 and bf2 snipers
//! aim assist with controller. Mouse is still no aim assist for anything").
//! Every disc sniper ships `LockOnRange 0`; the class loader gives a sniper
//! rifle a magnet (`core::weapon::WeaponClass::give_sniper_assist`).
//!
//! Real classes from the bake; each case skips without them.

use core::sim::{buttons, AimTarget, PadState, Sim};
use core::unit::{UnitHandle, UnitSpec};
use core::weapon::WeaponClass;
use glam::Vec3;

const FOUR: [&str; 4] = ["all", "imp", "rep", "cis"];

fn db(game: &str) -> Option<game::classdb::ClassDb> {
    let dir = game::assets::baked_root().join("classes");
    game::roster::load_classdb(&dir, game, &FOUR).ok()
}

/// Every weapon class of `game`'s four sides that loads.
fn weapons(db: &game::classdb::ClassDb) -> Vec<WeaponClass> {
    let mut names: Vec<_> = db
        .names()
        .filter(|n| db.get_kind("wpnc", n).is_some())
        .map(str::to_owned)
        .collect();
    names.sort();
    names
        .iter()
        .filter_map(|n| game::roster::weapon_from_db(db, n).ok())
        .collect()
}

/// The sniper rifles on both discs, and nothing else (BF2's `com_` bases
/// name no ordnance and are no weapon of their own).
#[test]
fn every_sniper_rifle_and_only_those_gets_a_magnet() {
    let (Some(bf1), Some(bf2)) = (db("bf1"), db("bf2")) else {
        eprintln!("SKIP: no baked classes");
        return;
    };
    for (game, db, want) in [
        (
            "bf1",
            &bf1,
            &[
                "all_weap_inf_sniperrifle",
                "cis_weap_inf_sniperrifle",
                "imp_weap_inf_sniperrifle",
                "rep_weap_inf_sniperrifle",
            ][..],
        ),
        (
            "bf2",
            &bf2,
            &[
                "all_weap_award_sniper_rifle",
                "all_weap_inf_sniper_rifle",
                "cis_weap_award_sniper_rifle",
                "cis_weap_inf_sniper_rifle",
                "imp_weap_award_sniper_rifle",
                "imp_weap_inf_sniper_rifle",
                "rep_weap_award_sniper_rifle",
                "rep_weap_inf_sniper_rifle",
            ][..],
        ),
    ] {
        let all = weapons(db);
        let snipers: Vec<_> = all.iter().filter(|w| w.is_sniper_rifle()).collect();
        let names: Vec<_> = snipers.iter().map(|w| w.name.as_str()).collect();
        println!("{game}: {names:?}");
        assert_eq!(names, want, "{game} sniper rifles");
        for w in snipers {
            assert_eq!(w.lock_on_range, 128.0, "{} range", w.name);
            assert_eq!(w.lock_on_angle, core::weapon::DEG, "{} angle", w.name);
            assert_eq!(w.target_team, 1, "{} enemies only", w.name);
            assert_eq!(w.target_kind & 0x1f, 0b1111);
            assert!(w.extreme_range >= w.lock_on_range, "{} reach", w.name);
        }
    }
}

/// The non-snipers keep the disc's values: the rifle, the bowcaster, the
/// pistols with a scope and a scoped vehicle gun.
#[test]
fn non_sniper_weapons_keep_the_discs_magnet() {
    let (Some(bf1), Some(bf2)) = (db("bf1"), db("bf2")) else {
        eprintln!("SKIP: no baked classes");
        return;
    };
    for (db, weapon, range, angle) in [
        (&bf1, "all_weap_inf_rifle", None, None),
        (&bf1, "imp_weap_hover_fightertank_laser", Some(0.0), None),
        (&bf2, "rep_weap_inf_rifle", None, None),
        (&bf2, "all_weap_inf_bowcaster", Some(64.0), Some(3.0)),
        (&bf2, "rep_weap_award_pistol", Some(40.0), Some(1.0)),
        (&bf2, "all_weap_hero_targetpistol", Some(64.0), Some(1.0)),
        (&bf2, "rep_weap_hover_fightertank_laser", None, None),
    ] {
        let w = game::roster::weapon_from_db(db, weapon).expect(weapon);
        assert!(!w.is_sniper_rifle(), "{weapon}");
        let mut again = w.clone();
        again.give_sniper_assist();
        assert_eq!(
            (again.lock_on_range, again.lock_on_angle, again.target_kind),
            (w.lock_on_range, w.lock_on_angle, w.target_kind),
            "{weapon}"
        );
        if let Some(r) = range {
            assert_eq!(w.lock_on_range, r, "{weapon} LockOnRange");
        }
        if let Some(a) = angle {
            assert!((w.lock_on_angle - a * core::weapon::DEG).abs() < 1e-6);
        }
    }
}

/// A player with `weapon` at the origin facing −Z and an idle enemy soldier
/// `dist` m out, the aim on the enemy's middle, scoped or not.
fn range(weapon: &WeaponClass, game: &str, dist: f32, scoped: bool) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(7);
    s.ground_rules = core::soldier::GroundRules::for_game(game);
    let class = s.add_weapon_class(weapon.clone());
    let p = s
        .spawn(&UnitSpec {
            weapon_class: class,
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    let e = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -dist)))
        .unwrap();
    if scoped {
        for b in [buttons::ZOOM, 0] {
            s.step(&[PadState {
                buttons: b,
                ..Default::default()
            }]);
        }
        assert!(s.unit(p).unwrap().scoped(), "{} scoped", weapon.name);
    }
    let aim = |s: &mut Sim, at: Vec3| {
        let eye = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views()).0;
        let d = at - eye;
        let u = s.unit_mut(p).unwrap();
        u.soldier.yaw = (-d.x).atan2(-d.z);
        u.soldier.pitch = d.y.atan2((d.x * d.x + d.z * d.z).sqrt());
    };
    let ctr = s.unit(e).unwrap().aim_bounds().0;
    aim(&mut s, ctr);
    // The aim ray's origin moves with the pitch: aim again from there.
    aim(&mut s, ctr);
    (s, e)
}

fn sniper(game: &str, weapon: &str) -> Option<WeaponClass> {
    let db = db(game)?;
    game::roster::weapon_from_db(&db, weapon).ok()
}

/// A BF1 and a BF2 sniper rifle on a controller take an enemy soldier at
/// 50 m and at 120 m as the magnet, scoped and not.
#[test]
fn a_sniper_on_a_controller_takes_a_magnet_at_50_and_120_m() {
    for (game, weapon) in [
        ("bf1", "all_weap_inf_sniperrifle"),
        ("bf1", "imp_weap_inf_sniperrifle"),
        ("bf2", "rep_weap_inf_sniper_rifle"),
        ("bf2", "cis_weap_award_sniper_rifle"),
    ] {
        let Some(w) = sniper(game, weapon) else {
            eprintln!("SKIP: no baked classes");
            return;
        };
        for dist in [50.0, 120.0] {
            for scoped in [false, true] {
                let (mut s, e) = range(&w, game, dist, scoped);
                s.step(&[PadState::default()]);
                assert_eq!(
                    s.magnet_at(0),
                    Some(AimTarget::Unit(e)),
                    "{weapon} at {dist} m, scoped {scoped}"
                );
            }
        }
        // Past the rifle's reach (128 m + the target's radius): none.
        let (mut s, _) = range(&w, game, 140.0, false);
        s.step(&[PadState::default()]);
        assert_eq!(s.magnet_at(0), None, "{weapon} at 140 m");
    }
}

/// The disc's sniper, `LockOnRange 0`, takes no magnet: what the override
/// changes.
#[test]
fn the_discs_sniper_without_the_override_takes_none() {
    let Some(mut w) = sniper("bf1", "all_weap_inf_sniperrifle") else {
        eprintln!("SKIP: no baked classes");
        return;
    };
    w.lock_on_range = 0.0;
    let (mut s, _) = range(&w, "bf1", 50.0, false);
    s.step(&[PadState::default()]);
    assert_eq!(s.magnet_at(0), None);
}

/// Mouse aim is never pulled, with a sniper as with every weapon; on a
/// controller the same shot, held on an enemy 0.6 m off the reticle, is.
#[test]
fn a_mouse_aimed_sniper_is_never_pulled() {
    for (game, weapon) in [
        ("bf1", "all_weap_inf_sniperrifle"),
        ("bf2", "rep_weap_inf_sniper_rifle"),
    ] {
        let Some(w) = sniper(game, weapon) else {
            eprintln!("SKIP: no baked classes");
            return;
        };
        for scoped in [false, true] {
            let run = |mouse: bool| {
                let (mut s, e) = range(&w, game, 50.0, scoped);
                s.unit_mut(e).unwrap().soldier.pos.x = 0.6;
                let yaw0 = s.units[0].soldier.yaw;
                let mut pad = PadState {
                    buttons: buttons::FIRE,
                    ..Default::default()
                };
                if mouse {
                    pad.buttons |= buttons::MOUSE_AIM;
                }
                // A held trigger: the first shot, then the pull.
                for _ in 0..10 {
                    s.step(&[pad]);
                }
                s.units[0].soldier.yaw - yaw0
            };
            let (pad, mouse) = (run(false), run(true));
            assert!(pad.abs() > 1e-4, "{weapon} scoped {scoped}: pad {pad}");
            assert_eq!(mouse, 0.0, "{weapon} scoped {scoped}: mouse");
        }
    }
}

/// The sniper's assist is under the same speed cap as every weapon's
/// (`core::aim_assist::speed_cap`, [ours] 2026-10-29: 3 x BF1's 0.675
/// rad/s): scoped at both zoom steps, a held trigger on an enemy crossing
/// at three times the tracking's capped rate, the stick at rest. No tick
/// turns the aim further than the tracking and the pull at their caps,
/// divided by the zoom (the PS2's one-unit clamp turns up to twice that).
#[test]
fn a_scoped_snipers_assist_turns_no_faster_than_the_cap() {
    use core::aim_assist::GAIN;
    for (game, weapon) in [
        ("bf1", "all_weap_inf_sniperrifle"),
        ("bf2", "rep_weap_inf_sniper_rifle"),
    ] {
        let Some(w) = sniper(game, weapon) else {
            eprintln!("SKIP: no baked classes");
            return;
        };
        for presses in [1, 2] {
            let dist = 30.0;
            let (mut s, e) = range(&w, game, dist, true);
            if presses == 2 {
                for b in [buttons::ZOOM, 0] {
                    s.step(&[PadState {
                        buttons: b,
                        ..Default::default()
                    }]);
                }
            }
            // The disc classes' MaxTurnSpeed (the spec's default is 2.0,
            // under 3 x BF1's, where the cap is the PS2's one unit).
            s.units[0].consts.max_turn_speed = 4.0;
            let me = &s.units[0];
            // Standing: the turn factor is 1.
            assert_eq!(me.soldier.stance, core::soldier::Stance::Stand);
            let z = me.zoom.expect("scoped");
            let r = me.consts.max_turn_speed;
            // rad/s the tracking turns at its cap, zoomed.
            let cap = (3.0 * 0.675 / r).min(1.0) * r / z.divisor;
            let vel = Vec3::X * (3.0 * cap * dist);
            let u = s.unit_mut(e).unwrap();
            u.controller = core::unit::Controller::Pacer { vel, period: 0 };
            u.soldier.vel = vel;
            u.health.cur = 1.0e9;
            let fire = PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            };
            let mut largest = 0.0f32;
            for _ in 0..30 {
                let held = s.magnet_at(0).is_some();
                let yaw0 = s.units[0].soldier.yaw;
                s.step(&[fire]);
                s.unit_mut(e).unwrap().health.cur = 1.0e9;
                if held {
                    largest = largest.max((s.units[0].soldier.yaw - yaw0).abs());
                }
            }
            // Both terms at their caps: the tracking's and GAIN of it.
            let bound = (1.0 + GAIN) * cap * core::TICK_DT;
            println!(
                "{weapon} zoom x{} (divisor {}): largest assist turn {largest:.5} rad of {bound:.5}",
                z.value, z.divisor
            );
            assert!(
                largest <= bound * 1.001,
                "{weapon} x{}: {largest} > {bound}",
                z.value
            );
            assert!(
                largest >= 0.9 * cap * core::TICK_DT,
                "{weapon} x{}: the assist acts",
                z.value
            );
        }
    }
}

/// [ours] C5 (playtest 2026-11-05: "aim assist should follow moving
/// targets a little"): a scoped sniper on a controller, the sticks at rest
/// and no press, turns with an enemy crossing at 3 m/s 50 m out by about
/// `SNIPER_FOLLOW` of the enemy's own turn, and holds him as the magnet.
/// Unscoped, and the rifle (whose magnet follows only after a press),
/// turn not at all; a mouse never.
#[test]
fn a_scoped_sniper_follows_a_crossing_enemy_a_little_without_firing() {
    let turn = |w: &WeaponClass, game: &str, dist: f32, scoped: bool, pad: PadState| {
        let (mut s, e) = range(w, game, dist, scoped);
        let vel = Vec3::X * 3.0;
        let u = s.unit_mut(e).unwrap();
        u.controller = core::unit::Controller::Pacer { vel, period: 0 };
        u.soldier.vel = vel;
        u.health.cur = 1.0e9;
        let bearing = |s: &Sim| {
            let p = s.unit(e).unwrap().soldier.pos;
            (-p.x).atan2(-p.z)
        };
        let (yaw0, b0) = (s.units[0].soldier.yaw, bearing(&s));
        for i in 0..10 {
            s.step(&[pad]);
            assert_eq!(
                s.magnet_at(0),
                Some(AimTarget::Unit(e)),
                "{} tick {i}",
                w.name
            );
        }
        (s.units[0].soldier.yaw - yaw0) / (bearing(&s) - b0)
    };
    let mouse = PadState {
        buttons: buttons::MOUSE_AIM,
        ..Default::default()
    };
    for (game, weapon) in [
        ("bf1", "all_weap_inf_sniperrifle"),
        ("bf2", "rep_weap_inf_sniper_rifle"),
    ] {
        let Some(w) = sniper(game, weapon) else {
            eprintln!("SKIP: no baked classes");
            return;
        };
        let k = turn(&w, game, 50.0, true, PadState::default());
        println!("{weapon}: scoped, the aim turned {k:.3} of the enemy's turn");
        assert!((0.3..0.7).contains(&k), "{weapon} scoped: {k}");
        assert_eq!(turn(&w, game, 50.0, false, PadState::default()), 0.0);
        let (mut s, _) = range(&w, game, 50.0, true);
        let yaw0 = s.units[0].soldier.yaw;
        s.step(&[mouse]);
        assert_eq!(s.units[0].soldier.yaw, yaw0, "{weapon}: mouse");
    }
    let Some(rifle) = sniper("bf1", "all_weap_inf_rifle") else {
        eprintln!("SKIP: no baked classes");
        return;
    };
    assert_eq!(turn(&rifle, "bf1", 30.0, false, PadState::default()), 0.0);
}
