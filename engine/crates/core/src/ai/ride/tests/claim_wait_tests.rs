//! CWAIT: actual-step long approaches and direct freshness/identity controls.
use super::*;
use crate::ai::state::BoardMaintenance;

/// A boarder slow enough that a walk from the edge of the claim range
/// outlasts the old eight-second cutoff, whichever way it faces (playtest
/// C3 turned walking bots to face their walk, at full forward speed).
fn slow_walker() -> crate::soldier::SoldierConstants {
    crate::soldier::SoldierConstants {
        max_speed: 6.0,
        max_strafe_speed: 6.0,
        ..Default::default()
    }
}

#[test]
fn cwait_natural_long_approaches_and_departure_controls() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for distance in [None, Some(70.0), Some(74.0)] {
                for phase in [0, 4] {
                    let (mut s, pilot, vh) = pickup_world(kind);
                    s.ground_rules = rules;
                    arm_gunner_seat(&mut s);
                    s.unit_mut(pilot).unwrap().ai.next_think = phase;
                    for _ in 0..20 {
                        s.step(&[]);
                        if s.unit(pilot).unwrap().mount.is_some() {
                            break;
                        }
                    }
                    assert_eq!(s.unit(pilot).unwrap().mount.unwrap().vehicle, vh);
                    let start = s.vehicle(vh).unwrap().pos;
                    let began = s.tick;
                    let other = distance.map(|d| {
                        s.spawn(&UnitSpec {
                            controller: Controller::Ai,
                            consts: slow_walker(),
                            ..UnitSpec::trooper(0, start + Vec3::new(d, -start.y, 0.0))
                        })
                        .unwrap()
                    });
                    let mut claim = None;
                    let mut acquired = None;
                    let mut entry = None;
                    let mut departure = None;
                    let mut completion = None;
                    let mut stamps = 0;
                    let mut last_stamp = None;
                    let mut held_travel = 0.0f32;
                    for _ in 0..600 {
                        s.step(&[]);
                        if let Some(h) = other {
                            let u = s.unit(h).unwrap();
                            if u.mount.is_some_and(|m| m.vehicle == vh) {
                                entry.get_or_insert(s.tick);
                                assert!(u.ai.ride.unwrap().maintenance.is_none());
                            } else if entry.is_none() {
                                let r = u.ai.ride.unwrap();
                                assert_eq!(r.board, Some(vh), "natural claim must persist");
                                acquired.get_or_insert(s.tick);
                                let since = *claim.get_or_insert(r.since);
                                assert_eq!(r.since, since, "maintenance cannot reset give-up");
                                let c = r.maintenance.unwrap();
                                assert_eq!(c.owner, h);
                                if last_stamp != Some(c.tick) {
                                    stamps += 1;
                                    last_stamp = Some(c.tick);
                                }
                                held_travel = held_travel
                                    .max(nav::xz_dist(start, s.vehicle(vh).unwrap().pos));
                                assert_eq!(
                                    locomotion(&s, pilot),
                                    0.0,
                                    "driver must wait before routing"
                                );
                            }
                        }
                        if s.unit(pilot)
                            .unwrap()
                            .ai
                            .ride
                            .unwrap()
                            .pickup
                            .unwrap()
                            .complete
                        {
                            completion.get_or_insert(s.tick);
                        }
                        if locomotion(&s, pilot) > 0.0 {
                            departure.get_or_insert(s.tick);
                        }
                        if departure.is_some() && (other.is_none() || entry.is_some()) {
                            break;
                        }
                    }
                    if other.is_some() {
                        let entered = entry.expect("long approach must board");
                        assert!(
                            entered > claim.unwrap() + 240,
                            "exercise old eight-second cutoff"
                        );
                        assert!(stamps > 24);
                        assert!(held_travel < 0.1);
                        assert!(
                            completion.unwrap() > entered
                                && completion.unwrap() <= entered + crate::ai::THINK_PERIOD
                        );
                        assert!(
                            departure.unwrap() > entered
                                && departure.unwrap() <= entered + crate::ai::THINK_PERIOD
                        );
                    } else {
                        assert_eq!(
                            departure,
                            Some(began + if rules == GroundRules::Bf1 { 121 } else { 122 })
                        );
                    }
                    eprintln!("CWAIT {rules:?}/{kind:?} distance={distance:?} phase={phase} began={began} acquired={acquired:?} since={claim:?} stamp={last_stamp:?} thinks={stamps} entry={entry:?} complete={completion:?} depart={departure:?} held_m={held_travel:.6}");
                }
            }
        }
    }
}

fn claimed() -> (Sim, UnitHandle, VehicleHandle, UnitHandle) {
    let (mut s, pilot, vh) = world();
    arm_gunner_seat(&mut s);
    assert!(s.enter_vehicle(pilot, vh, 0));
    let h = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(60.0, 0.0, 0.0))
        })
        .unwrap();
    s.tick = 300;
    s.unit_mut(pilot)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .pickup
        .as_mut()
        .unwrap()
        .started = Some(1);
    s.unit_mut(h).unwrap().ai.ride = Some(RideState {
        board: Some(vh),
        since: 2,
        maintenance: Some(BoardMaintenance {
            owner: h,
            tick: 299,
        }),
        ..Default::default()
    });
    s.unit_mut(h).unwrap().ai.target_cp = Some(0);
    (s, pilot, vh, h)
}

#[test]
fn cwait_freshness_boundary_and_giveup_are_independent() {
    let (mut s, pilot, vh, h) = claimed();
    let m = s.unit(pilot).unwrap().mount.unwrap();
    let mut st = s.unit(pilot).unwrap().ai;
    assert!(
        pickup_wait(&s, m, &mut st, true),
        "recent maintenance keeps old claim live"
    );
    s.tick = 388;
    assert!(
        pickup_wait(&s, m, &mut st, true),
        "live until, excluding, stamp+90"
    );
    s.tick = 389;
    assert!(
        !pickup_wait(&s, m, &mut st, true),
        "exact three-second expiry"
    );
    assert!(st.ride.unwrap().pickup.unwrap().complete);
    s.unit_mut(h)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .maintenance
        .as_mut()
        .unwrap()
        .tick = 389;
    assert!(
        !pickup_wait(&s, m, &mut st, true),
        "completion is permanent"
    );
    // Direct board-agent continuation: refresh never changes since or priority.
    let mut board = s.unit(h).unwrap().ai;
    let u = s.unit(h).unwrap().clone();
    let mut nt = NavTick::default();
    for tick in (390..902).step_by(10) {
        s.tick = tick;
        assert!(board_goal(&s, h, &u, &mut board, &mut nt).is_some());
        maintain(&s, h, &mut board);
        assert_eq!(board.ride.unwrap().since, 2);
    }
    s.tick = 902;
    assert!(board_goal(&s, h, &u, &mut board, &mut nt).is_none());
    assert_eq!(nt.claim, NavTick::CLAIM_GIVEUP);
    assert!(board.ride.unwrap().maintenance.is_none());
    assert_eq!(board.ride.unwrap().block_until, 902 + BOARD_BLOCK);
    s.unit_mut(h)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .maintenance
        .as_mut()
        .unwrap()
        .tick = 901;
    let mut waiting = s.unit(pilot).unwrap().ai;
    assert!(
        !pickup_wait(&s, m, &mut waiting, true),
        "recent maintenance cannot outlive original give-up"
    );
    // An abandoned record also loses the on-foot claim at the exact boundary.
    let mut board = s.unit(h).unwrap().ai;
    s.unit_mut(h)
        .unwrap()
        .ai
        .ride
        .as_mut()
        .unwrap()
        .maintenance
        .as_mut()
        .unwrap()
        .tick = 389;
    s.tick = 479;
    assert!(board_goal(&s, h, &u, &mut board, &mut nt).is_none());
    assert_eq!(nt.claim, NavTick::CLAIM_LOST);
    assert!(board.ride.unwrap().board.is_none());
    assert!(board.ride.unwrap().maintenance.is_none());
    assert_eq!(vh, m.vehicle);
}

#[test]
fn cwait_current_eligibility_and_entity_identities() {
    for case in 0..12 {
        let (mut s, pilot, vh, h) = claimed();
        let m = s.unit(pilot).unwrap().mount.unwrap();
        let mut st = s.unit(pilot).unwrap().ai;
        match case {
            0 => s.unit_mut(h).unwrap().alive = false,
            1 => {
                s.unit_mut(h).unwrap().team = 1;
                s.vehicle_mut(vh).unwrap().team = Some(1);
            }
            2 => s.unit_mut(h).unwrap().controller = Controller::Player { slot: 0 },
            3 => {
                s.unit_mut(h).unwrap().mount = Some(Mount {
                    vehicle: vh,
                    seat: 1,
                })
            }
            4 => {
                s.unit_mut(h)
                    .unwrap()
                    .ai
                    .ride
                    .as_mut()
                    .unwrap()
                    .board
                    .as_mut()
                    .unwrap()
                    .gen += 1
            }
            5 => {
                s.unit_mut(h)
                    .unwrap()
                    .ai
                    .ride
                    .as_mut()
                    .unwrap()
                    .maintenance
                    .as_mut()
                    .unwrap()
                    .owner
                    .gen += 1
            }
            6 => {
                s.unit_mut(h)
                    .unwrap()
                    .ai
                    .ride
                    .as_mut()
                    .unwrap()
                    .maintenance
                    .as_mut()
                    .unwrap()
                    .owner
                    .index += 1
            }
            7 => s.unit_mut(h).unwrap().ai.ride.as_mut().unwrap().maintenance = None,
            8 => s.unit_mut(h).unwrap().ai.ride = None,
            9 => s.seat_setup[0].as_mut().unwrap().boardable[1] = false,
            10 => {
                s.unit_mut(h)
                    .unwrap()
                    .ai
                    .ride
                    .as_mut()
                    .unwrap()
                    .maintenance
                    .as_mut()
                    .unwrap()
                    .tick = 301
            }
            11 => {
                assert!(s.seat_unit(h, vh, 1));
            }
            _ => unreachable!(),
        }
        assert!(!pickup_wait(&s, m, &mut st, true), "invalid record {case}");
    }
    // Multiple records: an invalid one cannot hide a live claimant.
    let (mut s, pilot, vh, h) = claimed();
    let duplicate = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(65.0, 0.0, 0.0))
        })
        .unwrap();
    s.unit_mut(duplicate).unwrap().ai.ride = s.unit(h).unwrap().ai.ride;
    let m = s.unit(pilot).unwrap().mount.unwrap();
    let mut st = s.unit(pilot).unwrap().ai;
    assert!(pickup_wait(&s, m, &mut st, true));
    let u = s.unit(duplicate).unwrap().clone();
    let mut board = u.ai;
    assert!(board_goal(&s, duplicate, &u, &mut board, &mut NavTick::default()).is_none());
    assert!(s.unit(h).unwrap().ai.ride.unwrap().board == Some(vh));
}

#[test]
fn cwait_injected_unmaintained_schedule_expires_in_real_steps() {
    // Deliberately suppress scheduled thinks: a real-step stale-record control,
    // not a naturally reachable scheduling phase.
    let run = |delay| {
        let (mut s, pilot, vh) = pickup_world(VehicleKind::Hover);
        arm_gunner_seat(&mut s);
        s.step(&[]);
        let start = s.vehicle(vh).unwrap().pos;
        let h = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, start + Vec3::new(70.0, -start.y, 0.0))
            })
            .unwrap();
        for _ in 0..40 {
            s.step(&[]);
        }
        if delay {
            s.unit_mut(h).unwrap().ai.next_think = 200;
        }
        for _ in 0..100 {
            s.step(&[]);
        }
        (s, pilot, h)
    };
    let (a, pilot, h) = run(false);
    let (b, _, _) = run(true);
    assert!(a.unit(h).unwrap().ai.ride.unwrap().maintenance.is_some());
    assert!(b.unit(h).unwrap().ai.ride.unwrap().board.is_none());
    assert_eq!(locomotion(&a, pilot), 0.0);
    assert!(locomotion(&b, pilot) > 0.0);
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn pickup_completed_wait_does_not_restart_for_later_claim() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, b, v) = pickup_world(kind);
        assert!(s.enter_vehicle(b, v, 0));
        for _ in 0..140 {
            s.step(&[]);
        }
        assert!(locomotion(&s, b) > 0.0);
        let claimant = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(500.0, 0.0, 0.0))
            })
            .unwrap();
        s.unit_mut(claimant).unwrap().ai.ride = Some(RideState {
            board: Some(v),
            since: s.tick,
            ..Default::default()
        });
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
        s.step(&[]);
        assert!(
            locomotion(&s, b) > 0.0,
            "a later claim cannot restart a completed wait"
        );
        s.unit_mut(claimant).unwrap().ai.ride = None;
        s.step(&[]);
        assert!(locomotion(&s, b) > 0.0);
    }
}

#[test]
fn cwait_extension_preserves_progress_aim_rng_events_and_bailout() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, pilot, vh, _) = claimed();
        if kind == VehicleKind::Walker {
            s.vehicle_mut(vh).unwrap().kind = kind;
            s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
        }
        let mut gun = s.seat_setup[0].as_ref().unwrap().guns[1].clone().unwrap();
        gun.pilot = true;
        gun.aimer.pitch = [0.2, 0.4];
        s.seat_setup[0].as_mut().unwrap().guns[0] = Some(gun);
        s.vehicle_mut(vh).unwrap().guns.resize_with(2, || None);
        s.vehicle_mut(vh).unwrap().guns[0] = Some(crate::vehicle::gun::GunState {
            look: (0.0, 0.0),
            aim: (0.0, 0.0),
            rate: (0.0, 0.0),
            rel: (0.0, 0.0),
            weapons: vec![],
            zoom: None,
        });
        let mut st = s.unit(pilot).unwrap().ai;
        st.install_route_for_fixture(Some(crate::ai::state::Route {
            waypoint: Vec3::new(0.0, 0.0, -300.0),
            ref_dist: 300.0,
            stuck_ticks: 59,
            stuck_run: 100,
            sidestep: 7,
            ..Default::default()
        }));
        let route = st.route();
        let follower = s.vehicle(vh).unwrap().hover.ai;
        let rng = s.rng.state();
        let events = s.events.len();
        let m = s.unit(pilot).unwrap().mount.unwrap();
        let (pad, nt) = drive(&s, pilot, m, &mut st, NavTick::default(), true);
        assert_eq!(
            st.route(),
            route,
            "claim hold cannot spend progress/stuck clocks"
        );
        assert_eq!(nt.stuck_run, 0);
        assert!(!st.ride.unwrap().use_req);
        if let Some(h) = st.ride.unwrap().hover {
            assert_eq!(h.ai, follower);
        }
        assert_ne!(pad.ry, 0, "waiting pilot still aims its gun");
        assert_eq!(s.rng.state(), rng);
        assert_eq!(s.events.len(), events, "claim hold emits no event");
        s.unit_mut(pilot).unwrap().ai = st;
        s.unit_mut(pilot)
            .unwrap()
            .ai
            .ride
            .as_mut()
            .unwrap()
            .bail_draw = Some(0);
        s.unit_mut(pilot).unwrap().ai.next_think = s.tick;
        s.vehicle_mut(vh).unwrap().health.cur = 100.0;
        s.step(&[]);
        assert!(
            s.unit(pilot).unwrap().mount.is_none(),
            "bailout preempts maintained claim"
        );
    }
}

#[test]
fn cwait_real_claim_seating_release_and_slot_reuse() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, pilot, vh, h) = claimed();
        if kind == VehicleKind::Walker {
            s.vehicle_mut(vh).unwrap().kind = kind;
            s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
        }
        s.unit_mut(h).unwrap().soldier.pos = s.vehicle(vh).unwrap().pos;
        let old_claim = s.unit(h).unwrap().ai.ride.unwrap();
        assert!(s.seat_unit(h, vh, 1));
        let r = s.unit(h).unwrap().ai.ride.unwrap();
        assert!(r.board.is_none() && r.maintenance.is_none());
        s.unseat_unit(h);
        assert!(s.unit(h).unwrap().ai.ride.unwrap().maintenance.is_none());
        // Restore a stale copied row adversarially, then actually recycle its slot.
        s.unit_mut(h).unwrap().ai.ride = Some(old_claim);
        s.kill_environment(h, crate::sim::DeathCause::Fall);
        assert!(
            s.units[h.index as usize]
                .ai
                .ride
                .unwrap()
                .maintenance
                .is_none(),
            "death ends heartbeat immediately"
        );
        let replacement = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(60.0, 0.0, 0.0))
            })
            .unwrap();
        assert_eq!(replacement.index, h.index);
        assert_ne!(replacement.gen, h.gen);
        assert!(s.unit(replacement).unwrap().ai.ride.is_none());
        s.unit_mut(replacement).unwrap().ai.ride = Some(old_claim);
        let mut st = s.unit(pilot).unwrap().ai;
        let m = s.unit(pilot).unwrap().mount.unwrap();
        assert!(
            !pickup_wait(&s, m, &mut st, true),
            "recycled slot cannot inherit maintenance"
        );
        s.remove_vehicle(vh);
        assert!(
            s.unit(replacement)
                .unwrap()
                .ai
                .ride
                .unwrap()
                .maintenance
                .is_none(),
            "target removal ends heartbeat immediately"
        );
        assert!(s.unit(pilot).unwrap().ai.ride.unwrap().pickup.is_none());
        // The existing WAIT4 real-step tests cover vehicle reuse and genuine new tenure.
    }
}

#[test]
fn cwait_natural_maintenance_clocks_hash_and_change_the_same_future() {
    let stamp_at = |ticks| {
        let (mut s, _, vh) = pickup_world(VehicleKind::Hover);
        arm_gunner_seat(&mut s);
        s.step(&[]);
        let start = s.vehicle(vh).unwrap().pos;
        let h = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                consts: slow_walker(),
                ..UnitSpec::trooper(0, start + Vec3::new(70.0, -start.y, 0.0))
            })
            .unwrap();
        while s.tick < ticks {
            s.step(&[]);
        }
        s.unit(h).unwrap().ai.ride.unwrap().maintenance.unwrap()
    };
    let early = stamp_at(230);
    let late = stamp_at(240);
    assert_eq!(early.owner, late.owner);
    assert_eq!((early.tick, late.tick), (229, 239));
    // As in WAIT4's continuation referee, transplant naturally reached clocks
    // into otherwise identical state. No caller output is injected.
    let continue_with = |stamp| {
        let (mut s, pilot, _, h) = claimed();
        s.tick = 320;
        s.unit_mut(h).unwrap().ai.ride.as_mut().unwrap().maintenance = Some(stamp);
        (s, pilot, h)
    };
    let (mut a, pilot, h) = continue_with(early);
    let (mut b, _, _) = continue_with(late);
    assert_ne!(
        a.hash(),
        b.hash(),
        "active maintenance clock must be hashed"
    );
    a.unit_mut(h).unwrap().ai.ride.as_mut().unwrap().maintenance = Some(late);
    assert_eq!(a.hash(), b.hash(), "null control: no other field differs");
    a.unit_mut(h).unwrap().ai.ride.as_mut().unwrap().maintenance = Some(early);
    a.step(&[]);
    b.step(&[]);
    assert!(
        locomotion(&a, pilot) > 0.0,
        "expired maintenance releases the driver"
    );
    assert_eq!(
        locomotion(&b, pilot),
        0.0,
        "recent maintenance keeps the same driver waiting"
    );
}

#[test]
fn pickup_full_ends_early_and_vacancy_does_not_restart() {
    for kind in [VehicleKind::Hover, VehicleKind::Walker] {
        let (mut s, b, v) = pickup_world(kind);
        assert!(s.enter_vehicle(b, v, 0));
        for _ in 0..30 {
            s.step(&[]);
            assert_eq!(locomotion(&s, b), 0.0);
        }
        let human = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        assert!(s.enter_vehicle(human, v, 1));
        s.step(&[]);
        assert!(locomotion(&s, b) > 0.0, "full {kind:?} departs next update");
        assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
        s.exit_vehicle(human);
        s.step(&[]);
        assert!(locomotion(&s, b) > 0.0, "vacancy cannot restart {kind:?}");
    }
}

#[test]
fn cwait_keeps_turret_claim_policy_outside_ground_maintenance() {
    let (mut s, _, vh, h) = claimed();
    s.vehicle_mut(vh).unwrap().kind = VehicleKind::Turret;
    for _ in 0..2 {
        s.spawn(&UnitSpec {
            controller: Controller::Idle,
            ..UnitSpec::trooper(0, Vec3::new(500.0, 0.0, 0.0))
        })
        .unwrap();
    }
    let u = s.unit(h).unwrap().clone();
    let mut st = u.ai;
    let mut nt = NavTick::default();
    s.tick = 241;
    assert!(
        board_goal(&s, h, &u, &mut st, &mut nt).is_some(),
        "turret does not adopt ground freshness"
    );
    s.tick = 242;
    assert!(
        board_goal(&s, h, &u, &mut st, &mut nt).is_none(),
        "turret retains its legacy abandonment"
    );
    assert_eq!(nt.claim, NavTick::CLAIM_LOST);
}
