use super::*;
use crate::cp::CpDef;
use crate::heightfield::Heightfield;
use crate::region::Region;
use crate::unit::{Controller, UnitSpec};
use crate::vehicle::hover::tests::skiff;
use crate::vehicle::{SeatSetup, VehicleKind, VehicleSpec};
use crate::weapon::HealthType;

/// Flat ground, an enemy post 300 m down −Z, a bot at the origin and a
/// skiff-like hover (pilot + one gunner) 12 m away, facing −Z.
fn world() -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    s.hover_params = vec![Some(skiff())];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true; 2],
        exit_drop: 10.0,
        ai_size: 8,
        guns: Vec::new(),
    })];
    let post = Vec3::new(0.0, 0.0, -300.0);
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
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(12.0, 2.0, 0.0),
        yaw: std::f32::consts::PI,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    (s, b, v)
}

fn bail_world(
    kind: VehicleKind,
    draw: u8,
    health: f32,
    gunner_first: bool,
) -> (Sim, UnitHandle, UnitHandle, VehicleHandle) {
    let (mut s, first, v) = world();
    let second = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let (pilot, gunner) = if gunner_first {
        (second, first)
    } else {
        (first, second)
    };
    s.vehicle_mut(v).unwrap().kind = kind;
    if kind == VehicleKind::Walker {
        s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
    }
    assert!(s.enter_vehicle(pilot, v, 0));
    assert!(s.enter_vehicle(gunner, v, 1));
    s.unit_mut(pilot)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .bail_draw = Some(draw);
    // A gunner's passing draw must not override a failing pilot draw.
    s.unit_mut(gunner)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .bail_draw = Some(0);
    for h in [pilot, gunner] {
        let ai = &mut s.unit_mut(h).unwrap().ai;
        ai.target_cp = Some(0);
        ai.next_think = 0;
    }
    s.vehicle_mut(v).unwrap().health.cur = health;
    (s, pilot, gunner, v)
}

#[test]
fn vbail_threshold_draw_and_all_ai_seats_on_next_think() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for gunner_first in [false, true] {
            for (draw, health, leaves) in [
                (0, 190.0, true),
                (100, 190.0, true),
                (83, 190.0, false),
                // Low two bits, not one: 2 and 6 fail, 4 passes.
                (2, 190.0, false),
                (6, 190.0, false),
                (4, 190.0, true),
                (0, 200.0, false),
                (0, 210.0, false),
            ] {
                let (mut s, p, g, v) = bail_world(kind, draw, health, gunner_first);
                s.step(&[]);
                assert_eq!(
                    s.unit(p).unwrap().mount.is_none(),
                    leaves,
                    "pilot {kind:?} draw={draw} health={health}"
                );
                assert_eq!(
                    s.unit(g).unwrap().mount.is_none(),
                    leaves,
                    "gunner {kind:?} draw={draw} health={health}"
                );
                assert_eq!(s.nav_stats.ai_bail_drivers, u64::from(leaves));
                assert_eq!(s.nav_stats.ai_bail_occupants, 2 * u64::from(leaves));
                assert_eq!(s.nav_stats.ai_bail_exposed, u64::from(health < 200.0));
                if leaves {
                    for h in [p, g] {
                        let u = s.unit(h).unwrap();
                        assert!(u.ai.ride.unwrap().bail_draw.is_none());
                        assert_eq!(u.ai.ride.unwrap().block_until, crate::ai::BOARD_BLOCK);
                        assert!(u.soldier.pos.y.abs() < 0.1, "flat-ground exit");
                    }
                    assert!(s.vehicle(v).unwrap().is_empty());
                }
            }
        }
    }
}

#[test]
fn vbail_waits_for_pilot_think_and_does_not_evict_humans() {
    let (mut s, p, g, v) = bail_world(VehicleKind::Hover, 0, 190.0, true);
    s.unit_mut(p).unwrap().ai.next_think = 10;
    s.step(&[]);
    assert!(s.unit(p).unwrap().mount.is_some());
    assert!(s.unit(g).unwrap().mount.is_some());
    assert_eq!(s.nav_stats.ai_bail_exposed, 1);
    s.unit_mut(g).unwrap().controller = Controller::Player { slot: 0 };
    s.unit_mut(g).unwrap().is_player = true;
    s.unit_mut(p).unwrap().ai.next_think = s.tick;
    s.step(&[PadState::default()]);
    assert!(s.unit(p).unwrap().mount.is_none());
    assert_eq!(s.unit(g).unwrap().mount.unwrap().vehicle, v);
    assert_eq!(s.nav_stats.ai_bail_exposed, 1);
    assert_eq!(s.nav_stats.ai_bail_occupants, 1);
    // Human pilot does not authorize the gunner's passing draw.
    let (mut s, p, g, v) = bail_world(VehicleKind::Hover, 0, 190.0, false);
    s.unit_mut(p).unwrap().controller = Controller::Player { slot: 0 };
    s.unit_mut(p).unwrap().is_player = true;
    assert!(!vehicle_bail_allowed(&s, v));
    s.step(&[PadState::default()]);
    assert!(s.unit(g).unwrap().mount.is_some());
    assert_eq!(s.nav_stats.ai_bail_drivers, 0);
}

#[test]
fn vbail_draw_uses_entropy_once_per_successful_boarding_and_is_hashed() {
    let (mut s, p, v) = world();
    let mut rng = s.rng.clone();
    let expected = (rng.next_rand() % 101) as u8;
    assert!(s.enter_vehicle(p, v, 0));
    assert_eq!(s.rng.state(), rng.state());
    assert_eq!(
        s.unit(p).unwrap().ai.ride.unwrap().bail_draw,
        Some(expected)
    );
    assert!(!s.enter_vehicle(p, v, 1));
    assert_eq!(s.rng.state(), rng.state());
    let hash = s.hash();
    s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some((expected + 1) % 101);
    assert_ne!(s.hash(), hash);
    s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some(expected);
    s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_seen = true;
    assert_ne!(s.hash(), hash);
    let draw = s.unit(p).unwrap().ai.ride.unwrap().bail_draw;
    let mut st = s.unit(p).unwrap().ai;
    settle(&s, &mut st);
    assert_eq!(st.ride.unwrap().bail_draw, draw);
    s.exit_vehicle(p);
    assert!(s.unit(p).unwrap().ai.ride.unwrap().bail_draw.is_none());
    assert!(s.enter_vehicle(p, v, 0));
    let expected = (rng.next_rand() % 101) as u8;
    assert_eq!(s.rng.state(), rng.state());
    assert_eq!(
        s.unit(p).unwrap().ai.ride.unwrap().bail_draw,
        Some(expected)
    );
}

#[test]
fn vbail_autonomous_boarding_keeps_draw_and_non_ground_entries_consume_none() {
    let (mut s, p, v) = world();
    s.unit_mut(p).unwrap().soldier.pos = s.vehicle(v).unwrap().pos;
    s.unit_mut(p).unwrap().ai.ride = Some(RideState {
        board: Some(v),
        use_req: true,
        ..RideState::default()
    });
    s.ai_use_pass();
    assert!(s.unit(p).unwrap().ai.ride.unwrap().bail_draw.is_some());
    for kind in [VehicleKind::Flyer, VehicleKind::Turret] {
        let (mut s, p, v) = world();
        s.vehicle_mut(v).unwrap().kind = kind;
        let rng = s.rng.state();
        assert!(s.enter_vehicle(p, v, 0));
        assert_eq!(s.rng.state(), rng);
        assert!(s.unit(p).unwrap().ai.ride.is_none());
    }
    let (mut s, p, v) = world();
    s.unit_mut(p).unwrap().controller = Controller::Player { slot: 0 };
    let rng = s.rng.state();
    assert!(s.enter_vehicle(p, v, 0));
    assert_eq!(s.rng.state(), rng);
}

#[test]
fn vbail_seeded_draw_population_and_exact_passing_values() {
    assert_eq!((0..=100).filter(|n| n & 3 == 0).count(), 26);
    let mut total = 0;
    for seed in 1..=8 {
        let (mut s, p, v) = world();
        s.rng = crate::rng::SimRng::new(seed);
        let mut passing = 0;
        for _ in 0..101 {
            assert!(s.enter_vehicle(p, v, 0));
            passing += usize::from(s.unit(p).unwrap().ai.ride.unwrap().bail_draw.unwrap() & 3 == 0);
            s.exit_vehicle(p);
        }
        total += passing;
        eprintln!("VBAIL synthetic seed {seed}: {passing}/101 passing draws");
    }
    let share = total as f32 / 808.0;
    eprintln!(
        "VBAIL synthetic aggregate {total}/808 = {:.2}% (not a map sweep)",
        100.0 * share
    );
    assert!((0.15..=0.37).contains(&share));
}

/// The whole card on one plane: the bot claims the hover, walks to it,
/// takes the pilot seat, drives it to the post and gets out there.
#[test]
fn a_bot_boards_drives_to_its_post_and_gets_out() {
    let (mut s, b, v) = world();
    let mut boarded_at = None;
    let mut out_at = None;
    for _ in 0..(90 * crate::TICK_HZ) {
        s.step(&[]);
        let seated = s.unit(b).unwrap().mount.is_some();
        if seated && boarded_at.is_none() {
            boarded_at = Some(s.tick);
        }
        if !seated && boarded_at.is_some() {
            out_at = Some(s.tick);
            break;
        }
    }
    let boarded = boarded_at.expect("the bot never boarded");
    assert!(
        boarded < 10 * u64::from(crate::TICK_HZ),
        "boarded at {boarded}"
    );
    let out = out_at.expect("the bot never got out");
    let u = s.unit(b).unwrap();
    let post = Vec3::new(0.0, 0.0, -300.0);
    let arrive = skiff().forward_speed * crate::ai::HOVER_ARRIVE_FACTOR;
    let d = nav::xz_dist(s.vehicle(v).unwrap().pos, post);
    assert!(
        d <= arrive + 1.0,
        "got out {d} m from the post (tick {out})"
    );
    assert!(nav::xz_dist(u.soldier.pos, post) < arrive + 12.0);
    assert_eq!(s.nav_stats.ai_boards, 1);
    assert_eq!(s.nav_stats.ai_dismounts, 1);
    assert!(s.nav_stats.ai_drive_ticks > 0);
    // It does not hop straight back in.
    for _ in 0..crate::TICK_HZ * 5 {
        s.step(&[]);
    }
    assert!(s.unit(b).unwrap().mount.is_none());
}

/// Card VMESH: a driver getting out with its stuck window spent marks
/// its vehicle where it stands, an ordinary dismount does not; no bot claims a marked
/// vehicle until it has moved [`STUCK_MOVED`] (a walker wedged on BF1
/// Endor's cp2 boulder was boarded by pair after pair).
#[test]
fn a_vehicle_given_up_as_stuck_is_not_claimed_until_it_moves() {
    let (mut s, b, v) = world();
    let out = |s: &mut Sim, stuck: bool| {
        assert!(s.enter_vehicle(b, v, 0));
        let u = &mut s.units[b.index as usize];
        let r = u.ai.ride.unwrap_or_default();
        u.ai.ride = Some(RideState { use_req: true, ..r });
        // The driver's stuck window, as its vehicle's drive left it.
        u.ai.install_route_for_fixture(Some(crate::ai::Route {
            stuck_run: if stuck {
                crate::ai::DRIVE_STUCK_BAIL
            } else {
                0
            },
            ..Default::default()
        }));
        s.ai_use_pass();
        assert!(s.unit(b).unwrap().mount.is_none());
        s.units[b.index as usize].ai.ride = None;
    };
    out(&mut s, false);
    assert_eq!(s.vehicle(v).unwrap().stuck_at, None, "a plain dismount");
    out(&mut s, true);
    let at = s.vehicle(v).unwrap().pos;
    assert_eq!(
        s.vehicle(v).unwrap().stuck_at,
        Some(at),
        "given up as stuck"
    );
    // The claim is made in a think: a fresh bot 12 m off makes it at
    // once (the moved case below), and never at the marked vehicle.
    for _ in 0..(3 * crate::TICK_HZ) {
        s.step(&[]);
        let u = s.unit(b).unwrap();
        assert!(
            u.mount.is_none() && u.ai.ride.and_then(|r| r.board).is_none(),
            "claimed a stuck vehicle"
        );
    }
    // Moved off the spot, it is a vehicle again.
    let pos = s.vehicle(v).unwrap().pos + Vec3::new(0.0, 0.0, 1.5 * STUCK_MOVED);
    s.vehicles[v.index as usize].pos = pos;
    s.sync_hull(v.index as usize);
    s.units[b.index as usize].soldier.pos = Vec3::ZERO;
    let boarded = (0..(10 * crate::TICK_HZ)).any(|_| {
        s.step(&[]);
        s.unit(b).unwrap().mount.is_some()
    });
    assert!(boarded, "never boarded once it had moved");
}

/// A driver routes as its vehicle's AISizeType (MEDIUM → table 3) and
/// arrives at ForwardSpeed × 0.5.
#[test]
fn a_seated_bot_routes_as_its_vehicle() {
    let (mut s, b, v) = world();
    assert_eq!(crate::ai::mover(&s, s.unit(b).unwrap()).ty, nav::SOLDIER);
    assert!(s.enter_vehicle(b, v, 0));
    assert_eq!(crate::ai::mover(&s, s.unit(b).unwrap()).ty, 3);
}

/// Two bots, one vehicle: one claim, one driver.
#[test]
fn one_vehicle_takes_one_driver() {
    let (mut s, b, v) = world();
    let c = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(2.0, 0.0, 0.0))
        })
        .unwrap();
    for _ in 0..(10 * crate::TICK_HZ) {
        s.step(&[]);
    }
    let seated: Vec<bool> = [b, c]
        .iter()
        .map(|&h| s.unit(h).unwrap().mount.is_some())
        .collect();
    assert_eq!(seated.iter().filter(|&&x| x).count(), 1, "{seated:?}");
    assert!(s.vehicle(v).unwrap().seats[1].occupant.is_none());
}

/// Card VTAI: two bots and a hover with an armed gunner seat — one
/// drives, and the other claims the gunner seat and boards it while the
/// pilot holds its throttle.
/// [`world`]'s gunner seat gets a gun.
fn arm_gunner_seat(s: &mut Sim) {
    s.seat_setup[0].as_mut().unwrap().guns = vec![
        None,
        Some(crate::vehicle::gun::GunSetup {
            weapons: vec![crate::vehicle::gun::GunWeapon {
                class: 0,
                infinite: true,
                reserve_clips: 0,
                fire_points: Vec::new(),
            }],
            fire_point: Vec3::new(0.0, 0.9, 4.2),
            pivot: Vec3::new(0.0, 0.8, 3.3),
            eye: Vec3::new(0.0, 1.0, 2.0),
            aimer: crate::vehicle::gun::Aimer::default(),
            pilot: false,
            turn: None,
            framed: false,
            aimer_bind: glam::Quat::IDENTITY,
            gunner: None,
        }),
    ];
}

#[test]
fn a_second_bot_takes_the_gunner_seat() {
    let (mut s, b, v) = world();
    arm_gunner_seat(&mut s);
    let c = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(-8.0, 0.0, 0.0))
        })
        .unwrap();
    let mut both = false;
    for _ in 0..(20 * crate::TICK_HZ) {
        s.step(&[]);
        let seated = [b, c].map(|h| s.unit(h).unwrap().mount.is_some());
        if seated == [true, true] {
            both = true;
            break;
        }
    }
    assert!(both, "both bots aboard");
    assert_eq!(s.nav_stats.ai_gunners, 1);
    let seats = &s.vehicle(v).unwrap().seats;
    assert!(seats.iter().all(|x| x.occupant.is_some()));
    // The gunner stays aboard while the pilot drives.
    for _ in 0..(5 * crate::TICK_HZ) {
        s.step(&[]);
    }
    assert!(s.vehicle(v).unwrap().seats[1].occupant.is_some());
}

/// Card UTALK, BF1's need-gunner call (re-squad-orders.md 1, 5): an AI
/// ground pilot calls independently of the vehicle's gun seats
/// with chance 0.3, and every friendly AI soldier on foot within 30 m
/// (3-D) answers "yes" - not one at 35 m, not an enemy, not a human, not
/// the caller. BF2 remains silent; boarding admission is independent.
#[test]
fn a_seated_ai_calls_for_a_gunner_and_the_ai_near_it_answer() {
    use crate::sim::{Call, Event};
    let (mut s, b, v) = world();
    s.ground_rules = crate::soldier::GroundRules::Bf1;
    let ai = |team: u8, at: Vec3| UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(team, at)
    };
    let near = s.spawn(&ai(0, Vec3::new(12.0, 20.0, 20.0))).unwrap();
    let far = s.spawn(&ai(0, Vec3::new(12.0, 0.0, 35.0))).unwrap();
    let enemy = s.spawn(&ai(1, Vec3::new(12.0, 0.0, 5.0))).unwrap();
    let human = s
        .spawn(&UnitSpec::player(0, Vec3::new(12.0, 0.0, -5.0)))
        .unwrap();
    assert!(s.enter_vehicle(b, v, 0));
    // Over 1000 ticks of the call alone: (calls, answers by unit).
    let tally = |s: &mut Sim| {
        let (mut calls, mut yes) = (0, Vec::new());
        for t in 0..1000 {
            s.tick = t;
            s.events.clear();
            s.need_gunner_call(b, v);
            for e in &s.events {
                match *e {
                    Event::Call {
                        unit,
                        call: Call::NeedGunner,
                    } => {
                        assert_eq!(unit, b);
                        calls += 1;
                    }
                    Event::Call {
                        unit,
                        call: Call::Yes,
                    } => yes.push(unit),
                    _ => {}
                }
            }
        }
        (calls, yes)
    };
    let (calls, yes) = tally(&mut s);
    assert!((250..=350).contains(&calls), "unarmed ground call: {calls}");
    assert_eq!(yes.len(), calls);
    assert!(yes.iter().all(|&u| u == near));
    arm_gunner_seat(&mut s);
    let (calls, yes) = tally(&mut s);
    assert!((250..=350).contains(&calls), "chance 0.3: {calls} of 1000");
    assert_eq!(yes.len(), calls, "one answer per call");
    assert!(
        yes.iter().all(|&u| u == near),
        "only the one in reach: {:?}",
        [far, enemy, human]
    );
    s.ground_rules = crate::soldier::GroundRules::Bf2;
    assert_eq!(tally(&mut s).0, 0, "BF2");
    s.ground_rules = crate::soldier::GroundRules::Bf1;
    assert!(s.enter_vehicle(near, v, 1));
    assert_eq!(tally(&mut s).0, calls, "full ground vehicle still calls");
}
fn pickup_probe(rules: GroundRules, full: bool) {
    let (mut s, b, v) = world();
    s.ground_rules = rules;
    let mut boarded = None;
    for _ in 0..300 {
        s.step(&[]);
        if s.unit(b).unwrap().mount.is_some() {
            boarded = Some(s.tick);
            break;
        }
    }
    let boarded = boarded.expect("natural boarding failed");
    if full {
        let rider = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        assert!(s.enter_vehicle(rider, v, 1));
    }
    assert_eq!(s.vehicle(v).unwrap().seats[1].occupant.is_some(), full);
    let start = s.vehicle(v).unwrap().pos;
    let mut max_throttle: f32 = 0.0;
    for _ in 0..60 {
        s.step(&[]);
        assert!(s.unit(b).unwrap().mount.is_some(), "pilot left");
        if let Some(h) = s.unit(b).unwrap().ai.ride.and_then(|r| r.hover) {
            max_throttle = max_throttle.max(h.sticks[0].abs()).max(h.sticks[1].abs());
        }
    }
    let delta = nav::xz_dist(start, s.vehicle(v).unwrap().pos);
    eprintln!("WAIT {rules:?} full={full} boarded={boarded} tick={} delta={delta:.6} max_stick={max_throttle:.6}",s.tick);
    if full {
        assert!(
            delta > 1.0 && max_throttle > 0.0,
            "full vehicle must depart"
        );
    } else {
        assert_eq!(
            max_throttle, 0.0,
            "new driver must wait while empty seat and within four seconds"
        );
        assert!(delta < 0.1, "waiting vehicle must stay put");
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
fn pickup_bf1_empty() {
    pickup_probe(GroundRules::Bf1, false);
}
#[test]
fn pickup_bf2_empty() {
    pickup_probe(GroundRules::Bf2, false);
}
#[test]
fn pickup_bf1_full_control() {
    pickup_probe(GroundRules::Bf1, true);
}
#[test]
fn pickup_bf2_full_control() {
    pickup_probe(GroundRules::Bf2, true);
}
fn pickup_world(kind: VehicleKind) -> (Sim, UnitHandle, VehicleHandle) {
    let (mut s, b, v) = world();
    if kind == VehicleKind::Walker {
        s.vehicle_mut(v).unwrap().kind = kind;
        s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
    }
    (s, b, v)
}

fn locomotion(s: &Sim, b: UnitHandle) -> f32 {
    let r = s.unit(b).unwrap().ai.ride.unwrap();
    r.sticks.map_or_else(
        || {
            r.hover
                .map_or(0.0, |h| h.sticks[0].abs().max(h.sticks[1].abs()))
        },
        |x| x[0].abs().max(x[1].abs()),
    )
}

#[test]
fn pickup_lifecycle_success_refusal_handover_switch_and_handle_reuse() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, b, v) = pickup_world(kind);
        assert!(!s.enter_vehicle(b, v, 9));
        assert!(s.unit(b).unwrap().ai.ride.is_none());
        s.tick = 500; // Distinguish a new tenure from an expired zero clock.
        assert!(s.enter_vehicle(b, v, 0));
        let w = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
        assert_eq!(
            (w.vehicle, w.seated, w.started, w.complete),
            (v, 500, None, false)
        );
        assert!(!s.enter_vehicle(b, v, 1));
        assert_eq!(s.unit(b).unwrap().ai.ride.unwrap().pickup, Some(w));
        s.step(&[]);
        assert_eq!(locomotion(&s, b), 0.0);
        let old = s.unit(b).unwrap().ai.ride.unwrap().pickup;
        s.exit_vehicle(b);
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
        assert!(s.enter_vehicle(b, v, 0));
        assert_ne!(s.unit(b).unwrap().ai.ride.unwrap().pickup, old);
        s.step(&[]);
        assert_eq!(locomotion(&s, b), 0.0);
        // A successful seat switch uses unseat/seat, just like switch_pass.
        s.unseat_unit(b);
        assert!(s.seat_unit(b, v, 1));
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
        s.unseat_unit(b);
        assert!(s.seat_unit(b, v, 0));
        assert_eq!(
            s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().seated,
            s.tick
        );
        // A human takeover ends the AI tenure; the next AI pilot starts fresh.
        let human = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        assert!(s.enter_vehicle(human, v, 0));
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
        assert!(s.unit(human).unwrap().ai.ride.is_none());
        s.exit_vehicle(human);
        assert!(s.enter_vehicle(b, v, 0));
        s.remove_vehicle(v);
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
        let replacement = s.spawn_vehicle(&VehicleSpec {
            kind,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        });
        assert_eq!(v.index, replacement.index);
        assert_ne!(v.gen, replacement.gen);
        assert!(s.enter_vehicle(b, replacement, 0));
        s.step(&[]);
        assert_eq!(
            s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().vehicle,
            replacement
        );
        assert_eq!(locomotion(&s, b), 0.0);
    }
}

#[test]
fn pickup_wait_preserves_progress_rng_events_and_bailout_priority() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, b, v) = pickup_world(kind);
        assert!(s.enter_vehicle(b, v, 0));
        let mut st = s.unit(b).unwrap().ai;
        st.install_route_for_fixture(Some(super::super::state::Route {
            waypoint: Vec3::new(0.0, 0.0, -300.0),
            ref_dist: 300.0,
            stuck_ticks: 59,
            stuck_run: 100,
            sidestep: 7,
            ..Default::default()
        }));
        let route = st.route();
        let follower = s.vehicle(v).unwrap().hover.ai;
        let rng = s.rng.state();
        let events = s.events.len();
        let m = s.unit(b).unwrap().mount.unwrap();
        for tick in 0..120 {
            s.tick = tick;
            let (_, nt) = drive(&s, b, m, &mut st, NavTick::default(), tick % 10 == 0);
            assert_eq!(st.route(), route, "wait cannot spend route progress");
            assert_eq!(nt.stuck_run, 0);
            assert!(!st.ride.unwrap().use_req);
            if let Some(h) = st.ride.unwrap().hover {
                assert_eq!(h.ai, follower);
            }
        }
        assert_eq!(s.rng.state(), rng);
        assert_eq!(s.events.len(), events, "wait emits no call/reply");
        s.unit_mut(b).unwrap().ai = st;
        s.unit_mut(b).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some(0);
        s.unit_mut(b).unwrap().ai.next_think = 0;
        s.vehicle_mut(v).unwrap().health.cur = 100.0;
        s.step(&[]);
        assert!(s.unit(b).unwrap().mount.is_none(), "bailout preempts wait");
    }
}

#[test]
fn pickup_reachable_different_clocks_hash_and_continue_differently() {
    // Both scenes stay still: only the time of successful entry differs.
    // A player controller holds the late entrant on foot until entry.
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let scene = |late: bool| {
            let (mut s, b, v) = world();
            s.ground_rules = rules;
            if late {
                s.unit_mut(b).unwrap().controller = Controller::Player { slot: 0 };
            } else {
                assert!(s.enter_vehicle(b, v, 0));
            }
            for _ in 0..30 {
                s.step(&[]);
            }
            if late {
                s.unit_mut(b).unwrap().controller = Controller::Ai;
                assert!(s.enter_vehicle(b, v, 0));
            }
            for _ in 0..10 {
                s.step(&[]);
            }
            (
                s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap(),
                s.vehicle(v).unwrap().pos,
            )
        };
        let (early, ep) = scene(false);
        let (late, lp) = scene(true);
        assert_eq!(ep, lp, "identical stationary geometry");
        assert_ne!(early.seated, late.seated);
        // Put each reachable clock into the same stationary continuation.
        // Every other sim field, including RNG, geometry and AI, is equal.
        let continuation = |wait| {
            let (mut s, b, v) = world();
            s.ground_rules = rules;
            assert!(s.enter_vehicle(b, v, 0));
            for _ in 0..40 {
                s.step(&[]);
            }
            s.tick = 125;
            s.unit_mut(b).unwrap().ai.ride.as_mut().unwrap().pickup = Some(wait);
            (s, b, v)
        };
        let (mut a, ab, av) = continuation(early);
        let (mut c, cb, cv) = continuation(late);
        assert_eq!(a.vehicle(av).unwrap().pos, c.vehicle(cv).unwrap().pos);
        assert_ne!(
            a.hash(),
            c.hash(),
            "different future departure must be hashed"
        );
        a.step(&[]);
        c.step(&[]);
        assert!(
            a.unit(ab)
                .unwrap()
                .ai
                .ride
                .unwrap()
                .pickup
                .unwrap()
                .complete
        );
        assert!(
            !c.unit(cb)
                .unwrap()
                .ai
                .ride
                .unwrap()
                .pickup
                .unwrap()
                .complete
        );
        assert!(locomotion(&a, ab) > 0.0);
        assert_eq!(locomotion(&c, cb), 0.0);
    }
}
#[test]
fn pickup_clock_boundary_uses_source_rule_and_first_think() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        for rules in [GroundRules::Bf1, GroundRules::Bf2] {
            let (mut s, b, v) = pickup_world(kind);
            s.ground_rules = rules;
            let mut expected_rng = s.rng.clone();
            expected_rng.next_rand(); // unchanged VBAIL boarding draw
            assert!(s.enter_vehicle(b, v, 0));
            assert_eq!(s.rng.state(), expected_rng.state());
            assert!(!s.enter_vehicle(b, v, 0));
            assert_eq!(s.rng.state(), expected_rng.state());
            s.unit_mut(b).unwrap().ai.next_think = 10;
            for _ in 0..10 {
                s.step(&[]);
            }
            assert!(s
                .unit(b)
                .unwrap()
                .ai
                .ride
                .unwrap()
                .pickup
                .unwrap()
                .started
                .is_none());
            s.step(&[]);
            assert_eq!(
                s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().started,
                Some(10)
            );
            let end = if rules == GroundRules::Bf1 { 120 } else { 130 };
            while s.tick <= end {
                s.step(&[]);
                assert_eq!(locomotion(&s, b), 0.0);
            }
            s.step(&[]);
            assert!(locomotion(&s, b) > 0.0);
            eprintln!(
                "BOUNDARY {kind:?} {rules:?} seated=0 first_think=10 departure={}",
                s.tick
            );
        }
    }
    let (mut s, b, v) = world();
    s.vehicle_mut(v).unwrap().kind = VehicleKind::Turret;
    let rng = s.rng.state();
    assert!(s.enter_vehicle(b, v, 0));
    assert!(s.unit(b).unwrap().ai.ride.is_none());
    assert_eq!(rng, s.rng.state(), "turret has no wait or boarding draw");
}
#[path = "post_setout_tests.rs"]
mod post_setout_tests;
#[path = "seat_call_tests.rs"]
mod seat_call_tests;
#[path = "setout_call_tests.rs"]
mod setout_call_tests;
#[path = "wait_call_tests.rs"]
mod wait_call_tests;

#[path = "call_board_tests.rs"]
mod call_board_tests;
#[path = "claim_wait_tests.rs"]
mod claim_wait_tests;
mod get_out_tests;
#[path = "human_wait_tests.rs"]
mod human_wait_tests;
#[path = "order_scene.rs"]
mod order_scene;
#[path = "order_wait_tests.rs"]
mod order_wait_tests;
#[path = "pile_in_tests.rs"]
mod pile_in_tests;
