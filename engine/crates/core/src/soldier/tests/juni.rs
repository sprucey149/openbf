//! JUNI merged jump referee. Original PS2 laws remain in re-stance-roll.md
//! §4.3a–c; BF2 numeric witnesses also run in the parent test module.
use super::*;

fn classes() -> [SoldierConstants; 2] {
    [
        trial_class(GroundRules::Bf1),
        SoldierConstants {
            max_speed: 9.2,
            max_strafe_speed: 3.8,
            acceleration: 48.0,
            jump_vy: 10.0,
            jump_forward_factor: 1.6,
            jump_strafe_factor: 0.65,
            ..SoldierConstants::default()
        },
    ]
}

fn close(got: Vec3, want: Vec3, context: &str) {
    assert!(
        got.distance(want) < 0.0002,
        "{context}: {got:?} vs {want:?}"
    );
}

// Independently evaluate the documented BF2 law in f64, outside the runtime
// helper. Flight rows compare velocity AND position before floor resolution.
fn air_row(v: Vec3, yaw: f32, input: (i16, i16), acceleration: f32, first: bool) -> Vec3 {
    let [x, y, z] = v.to_array().map(f64::from);
    let drag = f64::from(fmath::exp(
        (-0.012 * (x * x + y * y + z * z).sqrt() / 30.0) as f32,
    ));
    let (sy, cy) = fmath::sin_cos(yaw);
    let (sy, cy) = (f64::from(sy), f64::from(cy));
    let f = f64::from(input.0) / 32767.0 * 0.03 * f64::from(acceleration) / 30.0;
    let s = f64::from(input.1) / 32767.0 * 0.03 * f64::from(acceleration) / 30.0;
    Vec3::new(
        (x * drag - sy * f + cy * s) as f32,
        (y * drag - if first { 0.0 } else { 18.0 / 30.0 }) as f32,
        (z * drag - cy * f - sy * s) as f32,
    )
}

#[test]
fn juni_takeoff_axes_classes_yaw_and_inherited_vertical() {
    for c in classes() {
        for yaw in [0.0, 0.7, -2.2] {
            let (sy, cy) = fmath::sin_cos(yaw);
            let f = Vec3::new(-sy, 0.0, -cy);
            let side = Vec3::new(cy, 0.0, -sy);
            for (vf, vs) in [(0.0, 0.0), (6.0, 0.0), (-4.0, 0.0), (0.0, 3.0), (6.0, -3.0)] {
                for vy in [-2.0, 0.0, 3.5] {
                    // Exercise real rules selection through step separately below;
                    // take_off itself has one rule and no map argument.
                    let mut s = Soldier::new(Vec3::ZERO);
                    s.yaw = yaw;
                    s.vel = f * vf + side * vs + Vec3::Y * vy;
                    s.take_off(&c);
                    let want = f
                        * (vf
                            * if vf > 0.0 {
                                c.jump_forward_factor
                            } else {
                                c.jump_strafe_factor
                            })
                        + side * (vs * c.jump_strafe_factor)
                        + Vec3::Y * (vy + c.jump_vy);
                    close(s.vel, want, &format!("takeoff {yaw} {vf} {vs} {vy}"));
                    assert_eq!(s.stance, Stance::Jump);
                }
            }
        }
    }
}

#[test]
fn juni_every_air_row_and_complete_flight() {
    for rules in BOTH {
        for c in classes() {
            for yaw in [0.0, 0.7, -2.2] {
                for input in [(0, 0), (FWD, 0), (-FWD, 0), (0, FWD), (FWD, -FWD)] {
                    for inherited in [Vec3::ZERO, Vec3::new(3.0, -1.0, -6.0)] {
                        let mut s = Soldier::new(Vec3::ZERO);
                        s.yaw = yaw;
                        s.vel = inherited;
                        s.take_off(&c);
                        // The actual takeoff/contact leaves the first-air marker.
                        s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
                        let (mut want_v, mut want_p) = (s.vel, s.pos);
                        let pad = PadState {
                            ly: input.0,
                            lx: input.1,
                            ..Default::default()
                        };
                        let mut landed = false;
                        for row in 0..90 {
                            want_v = air_row(want_v, yaw, input, c.acceleration, row == 0);
                            want_p += want_v * DT;
                            s.step(DT, &pad, &c, rules);
                            let context =
                                format!("air {rules:?} yaw {yaw} input {input:?} row {row}");
                            close(s.vel, want_v, &context);
                            close(s.pos, want_p, &context);
                            let landing = want_p.y <= 0.02 && want_v.y < 0.1;
                            s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
                            assert_eq!(s.grounded, landing, "{context}");
                            if landing {
                                assert_eq!(s.pos.y, 0.0);
                                go(&mut s, &PadState::default(), &c, rules, true);
                                assert_eq!(s.stance, Stance::Stand);
                                landed = true;
                                break;
                            }
                        }
                        assert!(landed, "complete flight {rules:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn juni_fall_uses_bf2_and_thrown_uses_bf1_on_every_map() {
    for rules in BOTH {
        for stance in [Stance::Fall, Stance::Thrown] {
            for input in [(0, 0), (FWD, 0), (-FWD, 0), (0, FWD)] {
                let c = classes()[0];
                let mut s = Soldier::new(Vec3::new(0.0, 10.0, 0.0));
                s.stance = stance;
                s.grounded = false;
                s.vel = Vec3::new(0.4, 2.0, -3.0);
                let mut want = s.vel;
                let pad = PadState {
                    ly: input.0,
                    lx: input.1,
                    ..Default::default()
                };
                for row in 0..8 {
                    if stance == Stance::Thrown {
                        // KNOCK_RULES: BF1's on every map: one-sided approach, 70*1.2*0.2/30,
                        // no drag, MaxSpeed forward/back, MaxStrafeSpeed side.
                        let raise = |cur: f32, stick: i16, cap: f32| {
                            let target = f32::from(stick) / 32767.0 * 0.2 * cap;
                            let a = c.acceleration * 1.2 * 0.2 / 30.0;
                            if stick > 0 && cur < target {
                                (cur + a).min(target)
                            } else if stick < 0 && cur > target {
                                (cur - a).max(target)
                            } else {
                                cur
                            }
                        };
                        want.x = raise(want.x, input.1, c.max_strafe_speed);
                        want.z = -raise(-want.z, input.0, c.max_speed);
                        want.y -= 13.5 / 30.0;
                    } else {
                        want = air_row(want, 0.0, input, c.acceleration, false);
                    }
                    s.step(DT, &pad, &c, rules);
                    close(
                        s.vel,
                        want,
                        &format!("original thrown/fall {rules:?} {stance:?} row {row}"),
                    );
                    assert_eq!(s.stance, stance);
                    if stance == Stance::Thrown {
                        assert!(s.no_fire());
                    }
                }
            }
        }
    }
}

#[test]
fn juni_late_press_young_old_stand_and_sprint() {
    for rules in BOTH {
        for sprint in [false, true] {
            for age in if sprint {
                vec![20, 40]
            } else {
                vec![0, 6, 20, 40, 160]
            } {
                for off in 0..=12 {
                    let (s, _) = press_off_the_floor(rules, sprint, age, off);
                    assert_eq!(
                        s.stance == Stance::Jump,
                        off <= 9,
                        "late {rules:?} sprint {sprint} age {age} off {off}"
                    );
                }
            }
        }
    }
}

#[test]
fn juni_landing_lock_edges_held_press_and_energy() {
    for rules in BOTH {
        let c = classes()[0];
        // Pin both sides of the crouch's own strict 0.5 s gate; a held
        // refused press never becomes a new edge when that gate opens.
        for press_tick in [14, 15, 16] {
            let mut crouched = Soldier::new(Vec3::ZERO);
            crouched.stance = Stance::Crouch;
            for _ in 1..press_tick {
                go(&mut crouched, &PadState::default(), &c, rules, true);
            }
            go(&mut crouched, &JUMP, &c, rules, true);
            let want = if press_tick == 16 {
                Stance::Stand
            } else {
                Stance::Crouch
            };
            assert_eq!(
                crouched.stance, want,
                "crouch gate {rules:?} tick {press_tick}"
            );
            for _ in 0..20 {
                go(&mut crouched, &JUMP, &c, rules, true);
            }
            assert_eq!(crouched.stance, want, "holding creates no new edge");
            if want == Stance::Crouch {
                go(&mut crouched, &PadState::default(), &c, rules, true);
                go(&mut crouched, &JUMP, &c, rules, true);
                assert_eq!(crouched.stance, Stance::Stand);
            }
            go(&mut crouched, &PadState::default(), &c, rules, true);
            go(&mut crouched, &JUMP, &c, rules, true);
            assert_eq!(crouched.stance, Stance::Jump, "fresh press after rise");
        }
        let mut landed = Soldier::new(Vec3::ZERO);
        go(&mut landed, &JUMP, &c, rules, true);
        for _ in 0..90 {
            go(&mut landed, &PadState::default(), &c, rules, true);
            if landed.grounded && landed.stance == Stance::Stand {
                break;
            }
        }
        assert!(landed.grounded);
        assert_eq!(c.soft_lock, 0.3, "unchanged configured soft lock");
        assert_eq!(landed.lock_timer, 0.3, "landing arms the full soft lock");
        for wait in 0..=12 {
            let mut s = landed.clone();
            for _ in 0..wait {
                go(&mut s, &PadState::default(), &c, rules, true);
            }
            let open = wait >= 9; // nine release ticks, then the press tick
            let residual = (0.3 - wait as f32 / 30.0).max(0.0);
            assert!(
                (s.lock_timer - residual).abs() < 1e-7,
                "lock countdown {rules:?} wait {wait}"
            );
            go(&mut s, &JUMP, &c, rules, true);
            assert_eq!(
                s.stance == Stance::Jump,
                open,
                "landing lock {rules:?} wait {wait}"
            );
        }
        let mut held = Soldier::new(Vec3::ZERO);
        let mut jumps = 0;
        for _ in 0..120 {
            let prev = held.stance;
            go(&mut held, &JUMP, &c, rules, true);
            jumps += usize::from(prev != Stance::Jump && held.stance == Stance::Jump);
        }
        assert_eq!(jumps, 1, "held {rules:?}");
        for depleted in [false, true] {
            let mut s = Soldier::new(Vec3::ZERO);
            s.energy = 4.0;
            s.energy_depleted = depleted;
            go(&mut s, &JUMP, &c, rules, true);
            assert_eq!(s.stance == Stance::Jump, !depleted, "energy {rules:?}");
            if !depleted {
                assert!(s.energy < 0.0 && s.energy_depleted);
            }
        }
        for stance in [
            Stance::Prone,
            Stance::Roll,
            Stance::Thrown,
            Stance::Tumble,
            Stance::GetUp,
            Stance::Dead,
        ] {
            let mut s = Soldier::new(Vec3::ZERO);
            s.stance = stance;
            go(&mut s, &JUMP, &c, rules, true);
            assert_ne!(s.stance, Stance::Jump, "excluded {rules:?} {stance:?}");
        }
    }
}

#[test]
fn juni_sim_step_selects_merged_takeoff_and_first_air() {
    for rules in BOTH {
        for c in classes() {
            for yaw in [0.0, 0.7, -2.2] {
                for input in [(0, 0), (FWD, 0), (-FWD, 0), (0, FWD), (FWD, -FWD)] {
                    let mut sim = crate::sim::Sim::empty(3);
                    sim.ground_rules = rules;
                    sim.heightfield = Some(crate::heightfield::Heightfield {
                        size: 2,
                        unit: 100.0,
                        offset_x: 50.0,
                        offset_z: 50.0,
                        heights: vec![0.0; 4],
                    });
                    let h = sim
                        .spawn(&crate::unit::UnitSpec::player(0, Vec3::ZERO))
                        .unwrap();
                    let u = sim.unit_mut(h).unwrap();
                    u.consts = c;
                    u.soldier.yaw = yaw;
                    let (sy, cy) = fmath::sin_cos(yaw);
                    let f = Vec3::new(-sy, 0.0, -cy);
                    let side = Vec3::new(cy, 0.0, -sy);
                    let vf = f32::from(input.0) / 32767.0
                        * (c.acceleration * DT).min(if input.0 > 0 {
                            c.max_speed
                        } else {
                            c.max_strafe_speed
                        });
                    let vs = f32::from(input.1) / 32767.0
                        * (c.acceleration * DT).min(c.max_strafe_speed);
                    let want = f
                        * (vf
                            * if vf > 0.0 {
                                c.jump_forward_factor
                            } else {
                                c.jump_strafe_factor
                            })
                        + side * (vs * c.jump_strafe_factor)
                        + Vec3::Y * c.jump_vy;
                    sim.step(&[PadState {
                        ly: input.0,
                        lx: input.1,
                        ..JUMP
                    }]);
                    let s = &sim.unit(h).unwrap().soldier;
                    assert_eq!(s.stance, Stance::Jump);
                    close(
                        s.vel,
                        want,
                        &format!("sim takeoff {rules:?} yaw {yaw} input {input:?}"),
                    );
                    sim.step(&[PadState {
                        ly: input.0,
                        lx: input.1,
                        ..Default::default()
                    }]);
                    close(
                        sim.unit(h).unwrap().soldier.vel,
                        air_row(want, yaw, input, c.acceleration, true),
                        "sim first air",
                    );
                }
            }
        }
    }
}

#[test]
fn juni_thrown_grounded_first_update_and_landing_locks() {
    for rules in BOTH {
        let c = classes()[0];
        let mut s = Soldier::new(Vec3::ZERO);
        s.stance = Stance::Thrown;
        s.vel = Vec3::new(0.4, 2.0, -3.0);
        let pad = PadState {
            ly: -FWD,
            ..Default::default()
        };
        // KNOCK_RULES: BF1's throw on every map.
        let first = Vec3::new(0.4, 2.0, -0.2); // opposite stick: 3 - 70*1.2/30
        go(&mut s, &pad, &c, rules, true);
        close(s.vel, first, "thrown first update skips gravity");
        assert!(!s.grounded && s.no_fire());
        let second = Vec3::new(0.4, 1.55, 0.36); // opposite: another 70*1.2*0.2/30
        go(&mut s, &pad, &c, rules, true);
        close(
            s.vel,
            second,
            "thrown second update uses original gravity/control",
        );
        for speed in [2.0, 4.0, 7.0] {
            for draw in [0.1, 0.9] {
                let mut s = Soldier::new(Vec3::new(0.0, 0.01, 0.0));
                s.grounded = false;
                s.stance = Stance::Thrown;
                s.time_in_stance = 0.4;
                s.knock_draw = draw;
                s.vel = Vec3::new(0.0, -1.0, -speed);
                go(&mut s, &PadState::default(), &c, rules, true);
                assert!(s.grounded && s.no_fire());
                assert_eq!(
                    s.stance,
                    Stance::Thrown,
                    "contact precedes stance transition"
                );
                go(&mut s, &PadState::default(), &c, rules, true);
                let want = if speed > 6.0 || speed > 3.0 && draw > 0.3 {
                    Stance::Tumble
                } else {
                    Stance::GetUp
                };
                assert_eq!(
                    s.stance, want,
                    "landing {rules:?} speed {speed} draw {draw}"
                );
                for _ in 0..150 {
                    assert!(s.no_fire());
                    go(&mut s, &PadState::default(), &c, rules, true);
                    if s.stance == Stance::Stand {
                        break;
                    }
                }
                assert_eq!(s.stance, Stance::Stand);
                // KNOCK_RULES: one lock everywhere.
                assert_eq!(
                    s.lock_timer,
                    crate::soldier::GETUP_LOCK,
                    "get-up lock {rules:?}"
                );
                let at = s.pos;
                go(&mut s, &PadState { ly: FWD, ..JUMP }, &c, rules, true);
                assert!(s.no_fire());
                assert_eq!(s.stance, Stance::Stand);
                close(s.pos, at, "get-up lock freezes movement");
            }
        }
    }
}
