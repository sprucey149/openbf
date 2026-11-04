//! WAIT4 also exercises natural boarding, empty/full pickup and expiry under
//! both source rules; the BF1 AT-ST scene is a port-only control.

//! Card WAI's checks: the stick law against 145 ticks of PS2 RAM, then a bot
//! at the pilot seat on flat ground — a BF2 AT-ST class (the strafe branch),
//! a BF1-disc class (no flags, no e08 / e0c) and a BF2 spider class (the
//! plain branch against §4.5 C's negative factor).

mod ps2_rows;

use super::*;
use crate::cp::CpDef;
use crate::heightfield::Heightfield;
use crate::region::Region;
use crate::unit::{Controller, UnitSpec};
use crate::vehicle::walker::tests::atst;
use crate::vehicle::walker::{Clip, ANIM_COUNT};
use crate::vehicle::{SeatKind, SeatSetup, VehicleHandle, VehicleKind, VehicleSpec};
use crate::weapon::HealthType;

/// Acceptance 1: every PS2 row's sticks, to 2e-3.
#[test]
fn the_sticks_are_the_ps2s_on_145_ticks() {
    let mut worst = 0.0f32;
    let mut branches = [0u32; 4];
    for (i, r) in ps2_rows::ROWS.iter().enumerate() {
        let k = Class {
            max_speed: r[1],
            max_turn_speed: r[2],
            strafe: r[3] != 0.0,
        };
        let m = Body {
            pos: Vec3::new(r[4], r[5], r[6]),
            up: Vec3::new(r[7], r[8], r[9]),
            fwd: Vec3::new(r[10], r[11], r[12]),
        };
        let goal = (r[13] != 0.0).then(|| Vec3::new(r[14], r[15], r[16]));
        let (p2, p3) = sticks(r[0], &k, &m, goal, r[17] != 0.0, None);
        let e = (p2 - r[18]).abs().max((p3 - r[19]).abs());
        assert!(
            e < 2e-3,
            "row {i}: ({p2}, {p3}) vs the PS2's {:?}",
            &r[18..]
        );
        worst = worst.max(e);
        // stop / turn in place / walk / strafe: every branch is in the rows.
        let ahead = goal.is_some_and(|g| (g - m.pos).normalize_or_zero().dot(m.fwd) > COS_STRAFE);
        let b = if k.strafe && ahead && p2 > 0.0 {
            3
        } else if p2 > 0.0 {
            2
        } else if p3 != 0.0 {
            1
        } else {
            0
        };
        branches[b] += 1;
    }
    assert!(branches.iter().all(|&n| n > 0), "{branches:?}");
    eprintln!(
        "WAI rows: {} of {} within 2e-3 (worst {worst:.2e}); stop / turn / walk / strafe {branches:?}",
        ps2_rows::ROWS.len(),
        ps2_rows::ROWS.len()
    );
}

/// The law by hand: a goal 45° to the port's right (fwd × up) of a plain
/// class turns in place with the stick at +1... the body-turn stick's sign
/// is "toward −X", and −X is the port's right.
#[test]
fn a_plain_class_turns_in_place_then_walks() {
    let k = Class {
        max_speed: 5.0,
        max_turn_speed: 1.5,
        strafe: false,
    };
    let m = Body {
        pos: Vec3::ZERO,
        up: Vec3::Y,
        fwd: Vec3::Z,
    };
    let right = m.fwd.cross(m.up);
    let dt = crate::TICK_DT;
    let at = |deg: f32| {
        let (s, c) = deg.to_radians().sin_cos();
        Some((m.fwd * c + right * s) * 150.0)
    };
    assert_eq!(sticks(dt, &k, &m, at(45.0), true, None), (0.0, 1.0));
    assert_eq!(sticks(dt, &k, &m, at(-45.0), true, None), (0.0, -1.0));
    // Inside 5°: the throttle is cos(error); the stick is the deadbeat turn.
    let (p2, p3) = sticks(dt, &k, &m, at(1.0), true, None);
    assert!((p2 - 1f32.to_radians().cos()).abs() < 1e-6, "{p2}");
    assert!(
        (p3 - 1f32.to_radians().sin() / (1.5 * dt)).abs() < 1e-5,
        "{p3}"
    );
    // The strafe branch inside 30°: negated, linear to 0.4·c, then saturated.
    let s = Class { strafe: true, ..k };
    let (p2, p3) = sticks(dt, &s, &m, at(10.0), true, None);
    assert!(
        p2 == 1.0 && (p3 + 10f32.to_radians().tan() / 0.4).abs() < 1e-5,
        "{p2} {p3}"
    );
    let (p2, p3) = sticks(dt, &s, &m, at(25.0), true, None);
    assert!(
        p3 == -1.0 && (p2 - 0.4 / 25f32.to_radians().tan()).abs() < 1e-5,
        "{p2} {p3}"
    );
    // Outside 30° a strafe class takes the plain branch.
    assert_eq!(sticks(dt, &s, &m, at(45.0), true, None), (0.0, 1.0));
    // Stopped: no order, or inside 4 m with no route left — and then the
    // stick turns the body to the target.
    assert_eq!(sticks(dt, &k, &m, None, true, None), (0.0, 0.0));
    let near = Some(Vec3::new(0.0, 0.0, 3.0));
    assert_eq!(sticks(dt, &k, &m, near, false, None), (0.0, 0.0));
    assert_eq!(
        sticks(dt, &k, &m, near, false, Some(right * 20.0)),
        (0.0, 1.0)
    );
    assert_eq!(sticks(dt, &k, &m, near, true, None).0, 1.0);
}

/// A clip set whose walk covers `speed` m/s (the AT-ST's, rescaled).
fn rescaled(p: &mut WalkerParams, speed: f32) {
    let k = speed / 6.8064;
    for c in p.clips.iter_mut().flatten() {
        for z in &mut c.root_z {
            *z *= k;
        }
    }
    assert_eq!(p.clips.len(), ANIM_COUNT);
    let walk: &Clip = p.clips[7].as_ref().unwrap();
    assert!((walk.speed() - speed).abs() < 1e-3);
}

/// BF2's cis_walk_spider as the PS2 holds it (re-walker-ai §10 / §12's RAM
/// and `game::vehicle_class::walker`'s census): MaxSpeed 3, MaxTurnSpeed
/// 0.6, turn speeds 0.3 / 0.2, e08 / e0c = tan 87° / 88°, no bit 0, and a
/// walk clip of 3.075 m/s.
fn spider() -> WalkerParams {
    let mut p = WalkerParams {
        max_speed: 3.0,
        boost_speed: 0.0,
        stopped_turn_speed: 0.3,
        forward_turn_speed: 0.2,
        max_turn_speed: 0.6,
        flags: flags::AIM_WINDOW | flags::CAMERA_TURN | flags::REVERSE_STEER,
        ..atst()
    };
    rescaled(&mut p, 3.075);
    p
}

/// A walker as a BF1 disc authors it: the ctor's flags and no e08 / e0c.
fn bf1_class() -> WalkerParams {
    WalkerParams {
        e08: 0.0,
        e0c: 0.0,
        e10: 5.0,
        flags: flags::CTOR,
        ..atst()
    }
}

/// Flat ground, one walker of class `p` at the origin facing +Z with a bot
/// at its pilot seat, and the bot's post 150 m away, `deg` to the port's
/// right of the walker's forward.
fn world(p: WalkerParams, deg: f32) -> (Sim, UnitHandle, VehicleHandle, Vec3) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    s.walker_params = vec![Some(p)];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 8,
        guns: Vec::new(),
    })];
    let right = Vec3::Z.cross(Vec3::Y);
    let (sn, cs) = deg.to_radians().sin_cos();
    let post = (Vec3::Z * cs + right * sn) * 150.0;
    s.add_cp(
        CpDef {
            name: "far".into(),
            pos: post,
            capture: Region::sphere(post, 8.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    let b = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -6.0))
        })
        .unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 8000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    (s, b, v, post)
}

/// The heading error to `post`, degrees, + = the post is to the right.
fn error(s: &Sim, v: VehicleHandle, post: Vec3) -> f32 {
    let v = s.vehicle(v).unwrap();
    let f = v.rot * Vec3::Z;
    let d = (post - v.pos).normalize();
    let right = f.cross(Vec3::Y);
    d.dot(right).atan2(d.dot(f)).to_degrees()
}

/// What a run shows: per tick the error, the throttle and the XZ speed, from
/// the tick the bot is aboard.
struct Run {
    err: Vec<f32>,
    thr: Vec<f32>,
    strafe: Vec<f32>,
    speed: Vec<f32>,
    walk_clip: Vec<bool>,
}

fn run(p: WalkerParams, deg: f32, secs: u32) -> Run {
    let (mut s, b, v, post) = world(p, deg);
    // The bot walks up and boards by its own claim.
    for _ in 0..(10 * crate::TICK_HZ) {
        s.step(&[]);
        if s.unit(b).unwrap().mount.is_some() {
            break;
        }
    }
    assert!(s.unit(b).unwrap().mount.is_some(), "the bot never boarded");
    assert_eq!(s.nav_stats.ai_boards, 1);
    let mut r = Run {
        err: Vec::new(),
        thr: Vec::new(),
        strafe: Vec::new(),
        speed: Vec::new(),
        walk_clip: Vec::new(),
    };
    for _ in 0..(secs * crate::TICK_HZ) {
        let from = s.vehicle(v).unwrap().pos;
        s.step(&[]);
        let w = s.vehicle(v).unwrap();
        assert!(s.unit(b).unwrap().mount.is_some(), "the bot got out");
        r.err.push(error(&s, v, post));
        r.thr.push(w.input.throttle);
        r.strafe.push(w.input.strafe);
        r.speed.push(nav::xz_dist(from, w.pos) / crate::TICK_DT);
        r.walk_clip.push(matches!(w.walker.anim, 7 | 8));
    }
    r
}

fn mean(x: &[f32]) -> f32 {
    x.iter().sum::<f32>() / x.len().max(1) as f32
}

fn median(x: &[f32]) -> f32 {
    let mut v = x.to_vec();
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

/// Acceptance 2, first half: a BF2 AT-ST class (bit 0, e08 / e0c = tan 87°
/// / 88°) and a BF1-disc class turn TOWARD a goal 45° to either side, the
/// error falls under 1° and the ground speed settles at MaxSpeed's 5.0.
#[test]
fn an_atst_and_a_bf1_walker_turn_toward_the_goal_and_walk_at_5_m_s() {
    for (name, p) in [("BF2 AT-ST", atst()), ("BF1-disc class", bf1_class())] {
        for side in [45.0f32, -45.0] {
            let r = run(p.clone(), side, 20);
            // Toward the goal's side from the first second on.
            let e1 = r.err[crate::TICK_HZ as usize];
            assert!(
                e1.signum() == side.signum() && e1.abs() < 44.0 - 20.0,
                "{name} {side}: error {e1} after 1 s"
            );
            assert!(
                r.err.iter().all(|e| e.abs() < 45.5),
                "{name} {side}: turned away"
            );
            let tail = r.err.len() - 5 * crate::TICK_HZ as usize;
            let worst = r.err[tail..].iter().fold(0.0f32, |m, e| m.max(e.abs()));
            let speed = mean(&r.speed[tail..]);
            eprintln!(
                "WAI flat {name} goal {side:+}: error after 1 s {e1:+.2}, last 5 s max |error| {worst:.3} deg, speed {speed:.3} m/s"
            );
            assert!(worst < 1.0, "{name} {side}: error {worst}");
            assert!((speed - 5.0).abs() <= 0.1, "{name} {side}: {speed} m/s");
        }
    }
}

/// Acceptance 2, second half: a BF2 spider class (no bit 0) crabs — the
/// error settles in 4-6.5° (PS2 median 5.31°), the throttle toggles between
/// 0 and ≥ 0.996 with the stick pinned at ±1, and the root runs 3.3-3.9 m/s
/// in the walk clip (PS2 3.63).
#[test]
fn a_spider_crabs_five_degrees_off_its_goal() {
    for side in [45.0f32, -45.0] {
        let r = run(spider(), side, 40);
        let from = 20 * crate::TICK_HZ as usize;
        let errs: Vec<f32> = r.err[from..].iter().map(|e| e.abs()).collect();
        let med = median(&errs);
        let (lo, hi) = errs
            .iter()
            .fold((f32::MAX, 0.0f32), |(l, h), &e| (l.min(e), h.max(e)));
        let thr = &r.thr[from..];
        let stops = thr.iter().filter(|&&t| t == 0.0).count();
        let walks = thr.iter().filter(|&&t| t >= 0.996).count();
        let walking: Vec<f32> = r.speed[from..]
            .iter()
            .zip(&r.walk_clip[from..])
            .filter(|(_, &w)| w)
            .map(|(&s, _)| s)
            .collect();
        let speed = mean(&walking);
        eprintln!(
            "WAI flat BF2 spider goal {side:+}: |error| median {med:.2} (min {lo:.2}, max {hi:.2}) deg; throttle 0 on {stops}, >= 0.996 on {walks} of {} ticks; root speed in the walk clip {speed:.2} m/s",
            thr.len()
        );
        assert!((4.0..=6.5).contains(&med), "{side}: median {med}");
        assert!(r.err[from..].iter().all(|e| e.signum() == side.signum()));
        assert_eq!(stops + walks, thr.len(), "{side}: a throttle in between");
        assert!(stops > 0 && walks > 0, "{side}: {stops} / {walks}");
        assert!(r.strafe[from..].iter().all(|s| s.abs() == 1.0));
        assert!((3.3..=3.9).contains(&speed), "{side}: {speed} m/s");
    }
}

/// Card WALKB1 (re-walker §13.9): a BF1-map spider is BF2's with e0c = 0 —
/// its bot holds the goal under 2° while it walks at one 30° off, where
/// BF2's own e0c (tan 88°) leaves it crabbing about 5° off.
#[test]
fn a_bf1_map_spider_converges_on_its_goal() {
    for side in [30.0f32, -30.0] {
        let med = |p: WalkerParams| {
            let r = run(p, side, 40);
            let from = 20 * crate::TICK_HZ as usize;
            let errs: Vec<f32> = r.err[from..]
                .iter()
                .zip(&r.walk_clip[from..])
                .filter(|(_, &w)| w)
                .map(|(e, _)| e.abs())
                .collect();
            eprintln!("  walked {} of {} ticks", errs.len(), r.err.len() - from);
            assert!(errs.len() > 100, "{side}: walked {} ticks", errs.len());
            median(&errs)
        };
        let bf1 = med(WalkerParams {
            e0c: 0.0,
            ..spider()
        });
        let bf2 = med(spider());
        eprintln!("WALKB1 flat spider goal {side:+}: |error| median walking, e0c 0: {bf1:.2} deg; e0c tan 88: {bf2:.2} deg");
        assert!(bf1 < 2.0, "{side}: e0c 0 holds {bf1} deg");
        assert!((4.0..=6.5).contains(&bf2), "{side}: BF2's holds {bf2} deg");
    }
}

/// The whole ride: the bot drives the walker to its post and gets out there.
#[test]
fn a_bot_drives_a_walker_to_its_post_and_gets_out() {
    let (mut s, b, v, post) = world(atst(), 45.0);
    let mut boarded = false;
    for _ in 0..(60 * crate::TICK_HZ) {
        s.step(&[]);
        let seated = s.unit(b).unwrap().mount.is_some();
        boarded |= seated;
        if boarded && !seated {
            break;
        }
    }
    assert!(boarded && s.unit(b).unwrap().mount.is_none());
    let d = nav::xz_dist(s.vehicle(v).unwrap().pos, post);
    assert!(d <= 8.0 + 1.0, "got out {d} m from the post");
    assert_eq!(s.nav_stats.ai_dismounts, 1);
    assert_eq!(s.nav_stats.ai_walker_rides, 1);
    assert!(
        s.nav_stats.ai_walker_cm > 13_000,
        "{}",
        s.nav_stats.ai_walker_cm
    );
}

/// Card WAI2: a command walker (a class with spawn points: the AT-AT, the
/// AT-TE) is boarded however near the bot's post is, and its pilot stays in
/// at the post — the PS2's never gets out (re-walker-ai §13).
#[test]
fn a_command_walkers_pilot_stays_in_at_its_post() {
    let mut p = atst();
    p.spawn_points = vec![(Vec3::ZERO, 0.0)];
    let (mut s, b, v, post) = world(p, 45.0);
    // Ordinary round population: last-crew escape is covered separately.
    for i in 0..COMMAND_ENDGAME {
        s.spawn(&UnitSpec::trooper(0, Vec3::new(500.0 + i as f32, 0.0, 0.0)))
            .unwrap();
    }
    for _ in 0..(60 * crate::TICK_HZ) {
        s.step(&[]);
    }
    assert!(s.unit(b).unwrap().mount.is_some(), "the pilot got out");
    assert_eq!(s.nav_stats.ai_dismounts, 0);
    let d = nav::xz_dist(s.vehicle(v).unwrap().pos, post);
    assert!(d <= 8.0 + 1.0, "it stands {d} m from the post");
}

fn pickup_walker_world(p: WalkerParams, deg: f32) -> (Sim, UnitHandle, VehicleHandle, Vec3) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    s.walker_params = vec![Some(p)];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true; 2],
        exit_drop: 10.0,
        ai_size: 8,
        guns: Vec::new(),
    })];
    let right = Vec3::Z.cross(Vec3::Y);
    let (sn, cs) = deg.to_radians().sin_cos();
    let post = (Vec3::Z * cs + right * sn) * 150.0;
    s.add_cp(
        CpDef {
            name: "far".into(),
            pos: post,
            capture: Region::sphere(post, 8.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    let b = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -6.0))
        })
        .unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 8000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    (s, b, v, post)
}

fn pickup_walker_probe(rules: crate::soldier::GroundRules, full: bool) {
    let (mut s, b, v, _) = pickup_walker_world(atst(), 0.0);
    s.ground_rules = rules;
    let mut boarded = None;
    for _ in 0..300 {
        s.step(&[]);
        if s.unit(b).unwrap().mount.is_some() {
            boarded = Some(s.tick);
            break;
        }
    }
    let boarded = boarded.expect("natural walker boarding failed");
    if full {
        let rider = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        assert!(s.enter_vehicle(rider, v, 1));
    }
    assert_eq!(s.vehicle(v).unwrap().seats[1].occupant.is_some(), full);
    let start = s.vehicle(v).unwrap().pos;
    let mut max_stick: f32 = 0.0;
    for _ in 0..60 {
        s.step(&[]);
        assert!(s.unit(b).unwrap().mount.is_some(), "pilot left");
        if let Some(sticks) = s.unit(b).unwrap().ai.ride.and_then(|r| r.sticks) {
            max_stick = max_stick.max(sticks[0].abs()).max(sticks[1].abs());
        }
    }
    let delta = crate::nav::xz_dist(start, s.vehicle(v).unwrap().pos);
    eprintln!("WAIT WALKER {rules:?} full={full} boarded={boarded} tick={} delta={delta:.6} max_stick={max_stick:.6}",s.tick);
    if full {
        assert!(delta > 1.0 && max_stick > 0.0, "full walker departs");
    } else {
        assert_eq!(
            max_stick, 0.0,
            "new walker driver must wait within four seconds"
        );
        assert!(delta < 0.1);
    }
    if !full {
        let w = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
        assert_eq!(
            w.seated + 1,
            boarded,
            "successful seat clock survives ai_use_pass"
        );
        let expected = if rules == crate::soldier::GroundRules::Bf1 {
            w.seated
        } else {
            w.started.unwrap()
        } + 120;
        let mut departure = None;
        for _ in 0..100 {
            s.step(&[]);
            let r = s.unit(b).unwrap().ai.ride.unwrap();
            let stick = r.sticks.map_or_else(
                || {
                    r.hover
                        .map_or(0.0, |h| h.sticks[0].abs().max(h.sticks[1].abs()))
                },
                |x| x[0].abs().max(x[1].abs()),
            );
            if stick > 0.0 {
                departure = Some(s.tick);
                break;
            }
            assert!(crate::nav::xz_dist(start, s.vehicle(v).unwrap().pos) < 0.1);
            assert!(s.vehicle(v).unwrap().stuck_at.is_none());
        }
        let departure = departure.expect("driver must depart after initial window");
        assert!(
            (expected..=expected + crate::ai::THINK_PERIOD + 2).contains(&departure),
            "departure {departure}, deadline {expected}"
        );
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
        eprintln!("PICKUP {rules:?} boarded={boarded} start={:?} deadline={expected} complete/depart={departure} pos={:?}", w.started, s.vehicle(v).unwrap().pos);
    }
}
#[test]
fn pickup_walker_bf1_empty() {
    pickup_walker_probe(crate::soldier::GroundRules::Bf1, false);
}
#[test]
fn pickup_walker_bf2_empty() {
    pickup_walker_probe(crate::soldier::GroundRules::Bf2, false);
}
#[test]
fn pickup_walker_bf1_full_control() {
    pickup_walker_probe(crate::soldier::GroundRules::Bf1, true);
}
#[test]
fn pickup_walker_bf2_full_control() {
    pickup_walker_probe(crate::soldier::GroundRules::Bf2, true);
}

/// #86/#108: a drop hub can be far from an Attack post. Endgame pilots
/// must leave it even while their retained order still names that post.
#[test]
fn command_drop_hub_exits_for_tickets_or_last_crew() {
    for post in [false, true] {
        for (alive, tickets, exits) in [(7, 250, false), (7, 10, true), (6, 250, true)] {
            let mut p = atst();
            p.spawn_points = vec![(Vec3::ZERO, 0.0)];
            let (mut s, b, v, _) = world(p.clone(), 0.0);
            s.seat_setup[0].as_mut().unwrap().guns = vec![Some(crate::vehicle::gun::GunSetup {
                weapons: vec![],
                fire_point: Vec3::ZERO,
                pivot: Vec3::ZERO,
                eye: Vec3::ZERO,
                aimer: Default::default(),
                pilot: true,
                turn: None,
                framed: false,
                aimer_bind: glam::Quat::IDENTITY,
                gunner: None,
            })];
            for i in 1..alive {
                s.spawn(&UnitSpec::trooper(0, Vec3::new(100.0 + i as f32, 0.0, 0.0)))
                    .unwrap();
            }
            s.battle.teams[1].reinforcements = tickets as f32;
            assert!(s.enter_vehicle(b, v, 0));
            let mut st = AiState {
                target_cp: post.then_some(0),
                route: Some(super::super::state::Route {
                    waypoint: Vec3::ZERO,
                    ..Default::default()
                }),
                ride: Some(RideState::default()),
                ..Default::default()
            };
            let m = s.unit(b).unwrap().mount.unwrap();
            drive(&s, b, m, &p, &mut st, NavTick::default());
            assert_eq!(
                st.ride.unwrap().use_req,
                exits,
                "post={post}, alive={alive}, tickets={tickets}"
            );
        }
    }
}

#[test]
fn walker_progress_ignores_bob_and_jitter_but_counts_travel() {
    let mut st = AiState::default();
    for tick in 0..DRIVE_STUCK_BAIL {
        let pos = Vec3::new(if tick % 2 == 0 { 0.0 } else { 1.0 }, tick as f32, 0.0);
        walker_progress(&mut st, pos, true);
        // Replanning and its short progress window cannot erase this clock.
        st.route = None;
    }
    assert_eq!(
        st.ride.unwrap().walker_progress.unwrap().1,
        DRIVE_STUCK_BAIL
    );
    for _ in 0..100 {
        walker_progress(&mut st, Vec3::ZERO, false);
    }
    assert_eq!(
        st.ride.unwrap().walker_progress.unwrap().1,
        DRIVE_STUCK_BAIL
    );
    walker_progress(&mut st, Vec3::new(2.0, 500.0, 0.0), true);
    assert_eq!(st.ride.unwrap().walker_progress.unwrap().1, 0);
    for tick in 1..(DRIVE_STUCK_BAIL * 4) {
        walker_progress(&mut st, Vec3::new(2.0 + tick as f32 * 0.1, 0.0, 0.0), true);
        assert!(st.ride.unwrap().walker_progress.unwrap().1 < DRIVE_STUCK_BAIL);
    }
}

#[test]
fn jittering_spider_bails_and_marks_the_abandoned_vehicle() {
    let p = spider();
    let (mut s, b, v, post) = world(p.clone(), 0.0);
    assert!(s.enter_vehicle(b, v, 0));
    let m = s.unit(b).unwrap().mount.unwrap();
    let mut st = AiState {
        target_cp: Some(0),
        route: Some(super::super::state::Route {
            waypoint: post,
            ..Default::default()
        }),
        ride: Some(RideState::default()),
        ..Default::default()
    };
    for tick in 0..=DRIVE_STUCK_BAIL {
        s.vehicle_mut(v).unwrap().pos = Vec3::new(
            if tick % 2 == 0 { 0.0 } else { 1.0 },
            (tick % 3) as f32,
            0.0,
        );
        let r = st.route.as_mut().unwrap();
        r.stuck_run = 0;
        r.ref_dist = 151.0; // A fresh route reference may call each jitter progress.
        drive(&s, b, m, &p, &mut st, NavTick::default());
        assert_eq!(st.ride.unwrap().use_req, tick == DRIVE_STUCK_BAIL);
    }
    assert_eq!(st.route.unwrap().stuck_run, DRIVE_STUCK_BAIL);
    s.units[b.index as usize].ai = st;
    s.ai_use_pass();
    assert!(s.unit(b).unwrap().mount.is_none());
    assert!(s.vehicle(v).unwrap().stuck_at.is_some());
}
