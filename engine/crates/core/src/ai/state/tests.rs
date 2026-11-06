use super::*;
use crate::ai::DETOUR_MAX;

#[test]
fn the_route_is_part_of_the_ai_hash() {
    let hash = |st: &AiState| {
        let mut h: u64 = 0;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h = (h ^ x as u64).wrapping_mul(0x1_0000_01B3);
            }
        };
        st.hash_into(&mut eat);
        h
    };
    let mut a = AiState::default();
    let base = hash(&a);
    a.route = Some(Route::default());
    assert_ne!(base, hash(&a), "having a route is hashed");
    for mutate in [
        (|r: &mut Route| r.dest_hub = 3) as fn(&mut Route),
        |r: &mut Route| r.next_hub = 4,
        |r: &mut Route| r.via_arc = Some(1),
        |r: &mut Route| r.waypoint = Vec3::new(1.0, 2.0, 3.0),
        |r: &mut Route| r.ref_dist = 9.0,
        |r: &mut Route| r.from = Vec2::new(5.0, 0.0),
        |r: &mut Route| r.from = Vec2::new(0.0, 5.0),
        |r: &mut Route| r.stuck_ticks = 5,
        |r: &mut Route| r.stuck_run = 6,
        |r: &mut Route| r.sidestep = -7,
        |r: &mut Route| r.source = RouteSource::Bfs,
        |r: &mut Route| r.detour[0] = Vec2::new(1.0, 2.0),
        |r: &mut Route| r.detour[DETOUR_MAX - 1] = Vec2::new(3.0, 4.0),
        |r: &mut Route| r.detour_len = 2,
        |r: &mut Route| r.detour_at = 1,
    ] {
        let mut b = AiState {
            route: Some(Route::default()),
            ..AiState::default()
        };
        let before = hash(&b);
        mutate(b.route.as_mut().unwrap());
        assert_ne!(before, hash(&b), "every Route field is hashed");
    }
    // ... and the two AiState fields Phase A.1 added.
    for mutate in [
        (|st: &mut AiState| st.steer = SteerPick::Slide) as fn(&mut AiState),
        |st: &mut AiState| st.steer = SteerPick::Fan(-3),
        |st: &mut AiState| st.slide = -1,
        // ... and the two Phase B added.
        |st: &mut AiState| st.squad = Some(0),
        |st: &mut AiState| st.unreachable_dwell = 1,
        // ... and D13's.
        |st: &mut AiState| st.want_stance = 3,
        |st: &mut AiState| st.wander = Some((Vec3::X, 1)),
        // ... and every field Phase C added (D9's aim tracker, D10's
        // tracked-position error, D11's fire cycle). One assertion PER
        // FIELD and per float COMPONENT: a field left out of `hash_into`
        // makes two sims that have already diverged compare equal, which
        // is the one failure the replay gates cannot see (the replays have
        // no AI, so nothing else pins these eleven fields).
        |st: &mut AiState| st.aim_dir.x = 1.0,
        |st: &mut AiState| st.aim_dir.y = 1.0,
        |st: &mut AiState| st.aim_dir.z = 1.0,
        |st: &mut AiState| st.aim_vel.x = 1.0,
        |st: &mut AiState| st.aim_vel.y = 1.0,
        |st: &mut AiState| st.aim_vel.z = 1.0,
        // `err` starts at the no-target floor, not at 0.
        |st: &mut AiState| st.err = ERR_NO_TARGET + 1.0,
        |st: &mut AiState| st.err_pos.x = 1.0,
        |st: &mut AiState| st.err_pos.y = 1.0,
        |st: &mut AiState| st.err_pos.z = 1.0,
        |st: &mut AiState| st.dispersion = Some(0.125),
        |st: &mut AiState| st.burst = 1,
        |st: &mut AiState| st.pause_until = 1,
        |st: &mut AiState| st.charge_until = 1,
        // ... and card STTAP's trigger state.
        |st: &mut AiState| st.tap_gate = false,
        |st: &mut AiState| st.tap_until = 1,
        |st: &mut AiState| st.snipe_until = 1,
        |st: &mut AiState| st.tap_byte = 1,
        // ... and card GRAI's weapon choice.
        |st: &mut AiState| st.arm.pick = Some(0),
        |st: &mut AiState| st.arm.in_range = true,
        |st: &mut AiState| st.arm.vehicle = true,
        |st: &mut AiState| st.arm.spare = true,
        |st: &mut AiState| st.arm.group_until = 1,
        |st: &mut AiState| st.arm.group_next = 1,
        |st: &mut AiState| st.arm.claim = Some((UnitHandle { index: 1, gen: 0 }, 1)),
        |st: &mut AiState| st.dodge_until = 1,
        |st: &mut AiState| st.strafe_until = 1,
        |st: &mut AiState| st.strafe_side = -1,
        |st: &mut AiState| st.fight.mode = FightState::STAND,
        |st: &mut AiState| st.fight.pause_until = 1,
        // ... and card TEAM's role.
        |st: &mut AiState| st.role.post = Some(0),
        |st: &mut AiState| st.role.since = 1,
        |st: &mut AiState| st.role.at = 1,
        |st: &mut AiState| st.role.look.x = 1.0,
        |st: &mut AiState| st.role.look.y = 1.0,
        |st: &mut AiState| st.role.look.z = 1.0,
    ] {
        let mut b = AiState::default();
        let before = hash(&b);
        mutate(&mut b);
        assert_ne!(before, hash(&b), "every AiState field is hashed");
    }
    // Cards BNDS, HOLD and RCYC: the optional states eat nothing when
    // `None` and every field when `Some`.
    let with = |f: fn(&mut AiState)| {
        let mut b = AiState {
            lost: Some(LostState::default()),
            hold: Some(HoldState::default()),
            stall: Some(StallState::default()),
            ..AiState::default()
        };
        f(&mut b);
        hash(&b)
    };
    let both = with(|_| {});
    assert_ne!(both, hash(&AiState::default()), "having them is hashed");
    for mutate in [
        (|st: &mut AiState| st.lost.as_mut().unwrap().since = 1) as fn(&mut AiState),
        |st: &mut AiState| st.lost.as_mut().unwrap().confirmed = true,
        |st: &mut AiState| st.lost.as_mut().unwrap().tries = 1,
        |st: &mut AiState| st.lost.as_mut().unwrap().jump_after = 1,
        |st: &mut AiState| st.lost.as_mut().unwrap().window = 1,
        |st: &mut AiState| st.lost.as_mut().unwrap().ref_dist = 1.0,
        |st: &mut AiState| st.lost.as_mut().unwrap().shut = true,
        |st: &mut AiState| st.lost.as_mut().unwrap().pinned = true,
        |st: &mut AiState| st.lost.as_mut().unwrap().plans = 1,
        |st: &mut AiState| {
            let l = st.lost.as_mut().unwrap();
            l.plans = 1;
            l.path.len = 1;
            l.path.pts[0].z = 1.0;
        },
        |st: &mut AiState| st.hold.as_mut().unwrap().spot.x = 1.0,
        |st: &mut AiState| st.hold.as_mut().unwrap().goal.z = 1.0,
        |st: &mut AiState| st.hold.as_mut().unwrap().until = 1,
        |st: &mut AiState| st.hold.as_mut().unwrap().skip = 1,
        |st: &mut AiState| st.hold.as_mut().unwrap().shuffle_at = 1,
        |st: &mut AiState| st.hold.as_mut().unwrap().roll_at = 1,
        |st: &mut AiState| st.hold.as_mut().unwrap().strafe_side = 1,
        |st: &mut AiState| st.hold.as_mut().unwrap().stance_mask = 1,
        |st: &mut AiState| st.hold.as_mut().unwrap().look.y = 1.0,
        |st: &mut AiState| st.hold.as_mut().unwrap().last_contact = 1,
        |st: &mut AiState| st.stall.as_mut().unwrap().at.x = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().at.y = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().at.z = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().lo.x = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().lo.y = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().lo.z = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().hi.x = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().hi.y = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().hi.z = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().ticks = 1,
        |st: &mut AiState| st.stall.as_mut().unwrap().pin.x = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().pin.y = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().pin.z = 1.0,
        |st: &mut AiState| st.stall.as_mut().unwrap().fails = 1,
        |st: &mut AiState| st.stall.as_mut().unwrap().walked = Some(Vec3::ZERO),
        |st: &mut AiState| st.stall = None,
    ] {
        assert_ne!(
            both,
            with(mutate),
            "every LostState / HoldState / StallState field is hashed"
        );
    }
    // D10: the dispersion is hashed by VALUE, and `Some(0.0)` — a unit
    // whose distance table answers "no spread at all" — is not `None`.
    let di = |d: Option<f32>| {
        hash(&AiState {
            dispersion: d,
            ..AiState::default()
        })
    };
    assert_ne!(di(None), di(Some(0.0)));
    assert_ne!(di(Some(0.0)), di(Some(0.125)));
    // D11 tick stamps in distinct hash positions must not collide.
    let stamp: [fn(&mut AiState); 4] = [
        |st: &mut AiState| st.pause_until = 7,
        |st: &mut AiState| st.charge_until = 7,
        |st: &mut AiState| st.dodge_until = 7,
        |st: &mut AiState| st.strafe_until = 7,
    ];
    let stamped: Vec<u64> = stamp
        .iter()
        .map(|m| {
            let mut b = AiState::default();
            m(&mut b);
            hash(&b)
        })
        .collect();
    for i in 0..stamped.len() {
        for j in i + 1..stamped.len() {
            assert_ne!(
                stamped[i], stamped[j],
                "tick stamps {i} and {j} hash to the same word"
            );
        }
    }
    // D6: every `CmdState` field one at a time, and — the property the
    // replay gates rest on — an AiState with NO order eats NOTHING for it.
    // The no-order stream is a strict PREFIX of the ordered one, shorter by
    // exactly the 23 bytes `CmdState::hash_into` writes, which is only true
    // because all later optional extensions are absent in this fixture.
    let bytes = |st: &AiState| {
        let mut out: Vec<u8> = Vec::new();
        st.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let ordered = bytes(&AiState {
        cmd: Some(CmdState::default()),
        ..AiState::default()
    });
    let bare = bytes(&AiState::default());
    assert_eq!(ordered.len(), bare.len() + 1 + 12 + 2 + 8);
    assert_eq!(ordered[..bare.len()], bare[..], "the order is hashed LAST");
    assert_eq!(ordered[bare.len()], 1, "and behind a presence tag");
    let with = |c: CmdState| {
        hash(&AiState {
            cmd: Some(c),
            ..AiState::default()
        })
    };
    assert_ne!(hash(&AiState::default()), with(CmdState::default()));
    for mutate in [
        (|c: &mut CmdState| c.goal.x = 1.0) as fn(&mut CmdState),
        |c: &mut CmdState| c.goal.y = 1.0,
        |c: &mut CmdState| c.goal.z = 1.0,
        |c: &mut CmdState| c.far_ticks = 1,
        |c: &mut CmdState| c.roll = 1,
    ] {
        let mut c = CmdState::default();
        let before = with(c);
        mutate(&mut c);
        assert_ne!(before, with(c), "every CmdState field is hashed");
    }
    // Same argument for the three float TRIPLES: `aim_dir`, `aim_vel` and
    // `err_pos` are eaten one after another, so a duplicated component
    // would make two of them interchangeable.
    let triple: [fn(&mut AiState); 3] = [
        |st: &mut AiState| st.aim_dir = Vec3::new(1.0, 2.0, 3.0),
        |st: &mut AiState| st.aim_vel = Vec3::new(1.0, 2.0, 3.0),
        |st: &mut AiState| st.err_pos = Vec3::new(1.0, 2.0, 3.0),
    ];
    let tripled: Vec<u64> = triple
        .iter()
        .map(|m| {
            let mut b = AiState::default();
            m(&mut b);
            hash(&b)
        })
        .collect();
    for i in 0..tripled.len() {
        for j in i + 1..tripled.len() {
            assert_ne!(
                tripled[i], tripled[j],
                "float triples {i} and {j} hash to the same word"
            );
        }
    }
    // The fan's step is part of the hash, not just the tag.
    let f = |k| {
        hash(&AiState {
            steer: SteerPick::Fan(k),
            ..AiState::default()
        })
    };
    assert_ne!(f(1), f(2));
    assert_ne!(f(1), f(-1));
    // The squad id is hashed by VALUE, and `Some(0)` is not `None`.
    let sq = |k: Option<u32>| {
        hash(&AiState {
            squad: k,
            ..AiState::default()
        })
    };
    assert_ne!(sq(None), sq(Some(0)));
    assert_ne!(sq(Some(0)), sq(Some(1)));
}
#[test]
fn every_pickup_field_changes_the_hash() {
    let mut r = RideState::default();
    r.begin_pickup(
        crate::vehicle::VehicleHandle { index: 2, gen: 3 },
        10,
        crate::vehicle::VehicleKind::Walker,
        Default::default(),
    );
    let bytes = |r: &RideState| {
        let mut out = vec![];
        r.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let base = bytes(&r);
    for mutate in [
        (|w: &mut PickupWait| w.vehicle.index += 1) as fn(&mut PickupWait),
        |w| w.vehicle.gen += 1,
        |w| w.seated += 1,
        |w| w.started = Some(12),
        |w| w.released = Some(12),
        |w| w.complete = true,
        |w| w.departed = true,
    ] {
        let mut other = r;
        mutate(other.pickup.as_mut().unwrap());
        assert_ne!(base, bytes(&other));
    }
    let mut a = r;
    a.pickup.as_mut().unwrap().started = Some(12);
    let mut b = a;
    b.pickup.as_mut().unwrap().started = Some(13);
    assert_ne!(bytes(&a), bytes(&b));
    a.pickup.as_mut().unwrap().released = Some(12);
    b = a;
    b.pickup.as_mut().unwrap().released = Some(13);
    assert_ne!(bytes(&a), bytes(&b));
    b.pickup = None;
    assert_ne!(base, bytes(&b));
}
#[test]
fn cwait_each_maintenance_field_is_hashed_only_with_a_claim() {
    let bytes = |r: &RideState| {
        let mut out = vec![];
        r.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let mut r = RideState {
        board: Some(crate::vehicle::VehicleHandle { index: 2, gen: 3 }),
        maintenance: Some(BoardMaintenance {
            owner: UnitHandle { index: 4, gen: 5 },
            tick: 6,
        }),
        ..Default::default()
    };
    let baseline = bytes(&r);
    for change in 0..4 {
        let mut other = r;
        match change {
            0 => other.maintenance.as_mut().unwrap().owner.index += 1,
            1 => other.maintenance.as_mut().unwrap().owner.gen += 1,
            2 => other.maintenance.as_mut().unwrap().tick += 1,
            _ => other.maintenance = None,
        }
        assert_ne!(baseline, bytes(&other), "omitted field {change}");
    }
    r.board = None;
    let absent = bytes(&r);
    r.maintenance = None;
    assert_eq!(absent, bytes(&r), "inactive maintenance adds no bytes");
    assert_eq!(bytes(&RideState::default()), bytes(&r));
}

#[test]
fn walker_progress_clock_and_each_xz_component_are_hashed() {
    let bytes = |r: &RideState| {
        let mut out = vec![];
        r.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let base = RideState::default();
    let a = RideState {
        walker_progress: Some((Vec2::ZERO, 0)),
        ..base
    };
    assert_ne!(bytes(&base), bytes(&a));
    for value in [(Vec2::X, 0), (Vec2::Y, 0), (Vec2::ZERO, 1)] {
        assert_ne!(
            bytes(&a),
            bytes(&RideState {
                walker_progress: Some(value),
                ..a
            })
        );
    }
}

#[test]
fn endgame_command_departure_changes_the_hashed_future() {
    let bytes = |r: &RideState| {
        let mut out = vec![];
        r.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let ordinary = RideState::default();
    let departed = RideState {
        command_dismounted: true,
        ..ordinary
    };
    assert_ne!(bytes(&ordinary), bytes(&departed));
}
