use super::*;
use crate::{
    heightfield::Heightfield,
    soldier::SoldierConstants,
    unit::{Controller, UnitSpec},
};

/// The disc's jet numbers; the game flies every jet trooper on
/// `game::roster::JET_TROOPER_FUEL_*`.
fn class(hover: bool) -> SoldierConstants {
    SoldierConstants {
        jet_jump: if hover { 5.0 } else { 21.0 },
        jet_acceleration: if hover { 30.0 } else { 10.0 },
        jet_hover: hover,
        jet_fuel_initial: if hover { 0.25 } else { 0.5 },
        jet_fuel_cost: if hover { 0.16 } else { 0.0 },
        jet_fuel_recharge: if hover { 0.12 } else { 0.075 },
        jet_fuel_min_border: if hover { 0.24 } else { 0.49 },
        jet_control: [1.5, 1.25, 1.25],
        ..Default::default()
    }
}
fn sim(hover: bool) -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let h = sim
        .spawn(&UnitSpec {
            consts: class(hover),
            jet: true,
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    sim.spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -500.0)))
        .unwrap();
    sim.nav = crate::nav::NavGraph::new(
        [Vec3::ZERO, Vec3::new(0.0, 0.0, -500.0)]
            .into_iter()
            .map(|pos| crate::nav::Hub {
                pos,
                radius: 5.0,
                arcs: vec![0],
            })
            .collect(),
        vec![crate::nav::Arc {
            a: 0,
            b: 1,
            type_mask: 1,
            flags: 0,
        }],
        Default::default(),
        vec![],
        vec![],
    );
    (sim, h)
}
#[test]
fn a_tactical_hop_from_the_marked_deck_needs_supported_floor() {
    let (mut sim, _) = sim(false);
    sim.nav_floors = vec![0.0; 2];
    let spot = Vec3::new(20.0, 0.0, 0.0);
    let over_drop = spot.with_y(4.0);
    assert!(hop_floor_clear(&sim, Vec3::Y * 4.0, over_drop));

    sim.nav
        .as_mut()
        .unwrap()
        .hop_support_areas
        .push(crate::collision::Aabb {
            min: Vec3::splat(-1.0),
            max: Vec3::splat(5.0),
        });
    assert!(hop_floor_clear(&sim, Vec3::ZERO, spot));
    assert!(!hop_floor_clear(&sim, Vec3::Y * 4.0, over_drop));
    assert!(hop_floor_clear(&sim, Vec3::Y * 6.0, over_drop));

    sim.heightfield = None;
    assert!(!hop_floor_clear(&sim, Vec3::ZERO, spot));
}

#[test]
fn a_marked_hop_landing_needs_room_to_coast() {
    use crate::collision::{Aabb, Body, Layers, Shape};

    let (mut sim, _) = sim(false);
    sim.nav_floors = vec![0.0; 2];
    let spot = Vec3::X * 20.0;
    sim.collision.push_body(Body::with_layers(
        glam::Mat4::from_translation(Vec3::new(26.0, 1.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.5, 1.0, 2.0),
        },
        Layers::ALL,
    ));
    assert!(hop_floor_clear(&sim, Vec3::ZERO, spot));

    let area = Aabb {
        min: spot - Vec3::ONE,
        max: spot + Vec3::ONE,
    };
    sim.nav
        .as_mut()
        .unwrap()
        .hop_landing_areas
        .push((area, 12.0));
    assert!(!hop_floor_clear(&sim, Vec3::ZERO, spot));
    assert!(hop_floor_clear(&sim, 2.0 * spot, spot));
    assert!(hop_floor_clear(&sim, Vec3::Z * 3.0, spot + Vec3::Z * 3.0));
}

#[test]
fn bots_out_of_a_fight_do_not_hop_and_replay_identically() {
    for hover in [false, true] {
        let run = || {
            let (mut sim, h) = sim(hover);
            let mut starts = Vec::new();
            let mut prev = false;
            for _ in 0..1800 {
                sim.step(&[]);
                let s = &sim.unit(h).unwrap().soldier;
                let active = s.stance.jetting();
                if active && !prev {
                    starts.push((sim.tick, sim.unit(h).unwrap().ai.target.is_some()));
                }
                prev = active;
            }
            assert!(starts.iter().all(|s| s.1), "hover {hover}: {starts:?}");
            (sim.hash(), starts)
        };
        assert_eq!(run(), run());
    }
}
#[test]
fn scheduler_draws_only_ps2_second_intervals() {
    let h = UnitHandle { index: 7, gen: 3 };
    for tick in 0..1000 {
        for (lo, hi, salt) in [(6, 12, HOP_SALT), (3, 8, HINT_SALT)] {
            let delay = after(h, tick, lo, hi, salt) - tick;
            assert!((u64::from(lo) * 30..=u64::from(hi) * 30).contains(&delay));
            assert_eq!(delay % 30, 0);
        }
    }
}
#[test]
fn jetless_ai_keeps_no_jet_state() {
    let (mut sim, h) = sim(true);
    sim.units[h.index as usize].consts.jet_fuel_recharge = 0.0;
    for _ in 0..400 {
        sim.step(&[]);
    }
    assert_eq!(sim.unit(h).unwrap().ai.jet, JetState::default());
    assert!(!sim.unit(h).unwrap().soldier.stance.jetting());
}
#[test]
fn every_flight_field_and_scheduler_is_hashed() {
    let base = JetState {
        next_hop: 180,
        next_hint: 90,
        flight: Some(Flight {
            goal: Vec3::ONE,
            kick: Vec3::Y,
            height: Some(5.0),
            route: false,
            hop: false,
            since: 1,
            launched: false,
            stop: false,
        }),
    };
    let bytes = |st: JetState| {
        let mut b = Vec::new();
        st.hash_into(&mut |v| b.extend_from_slice(v));
        b
    };
    for f in [
        |s: &mut JetState| s.next_hop += 1,
        |s: &mut JetState| s.next_hint += 1,
        |s: &mut JetState| s.flight = None,
        |s: &mut JetState| s.flight.as_mut().unwrap().goal.x += 1.0,
        |s: &mut JetState| s.flight.as_mut().unwrap().kick.y += 1.0,
        |s: &mut JetState| s.flight.as_mut().unwrap().height = None,
        |s: &mut JetState| s.flight.as_mut().unwrap().height = Some(6.0),
        |s: &mut JetState| s.flight.as_mut().unwrap().since += 1,
        |s: &mut JetState| s.flight.as_mut().unwrap().launched = true,
        |s: &mut JetState| s.flight.as_mut().unwrap().stop = true,
        |s: &mut JetState| s.flight.as_mut().unwrap().route = true,
        |s: &mut JetState| s.flight.as_mut().unwrap().hop = true,
    ] {
        let mut altered = base;
        f(&mut altered);
        assert_ne!(bytes(base), bytes(altered));
    }
}

fn request() -> MoveRequest<'static> {
    MoveRequest {
        kind: super::super::state::WalkAsk::LEG,
        goal: Some(Vec3::new(0.0, 0.0, -50.0)),
        arrive: None,
        post: None,
        defending: false,
        in_range: false,
        ret: None,
        hold: None,
        flee: false,
        boarding: false,
        serving: false,
        excused: false,
        acts: crate::ai::brain::intent::Acts::default(),
    }
}
fn engage(sim: &mut Sim, st: &mut AiState, distance: f32) {
    let enemy = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -distance)))
        .unwrap();
    st.target = Some(enemy.into());
    st.contacts[0] = Some(perception::Contact {
        handle: enemy.into(),
        first_seen: 0,
        last_seen: 0,
        last_pos: Vec3::new(0.0, 0.0, -distance),
        seen_now: true,
        heard: false,
        told: false,
    });
}
#[test]
fn a_fight_hop_flies_to_a_clear_spot_twenty_to_fifty_seven_metres_away() {
    for hover in [false, true] {
        let mut spots = Vec::new();
        for tick in 1..40 {
            let (mut sim, h) = sim(hover);
            let mut st = AiState::default();
            engage(&mut sim, &mut st, 10.0);
            sim.tick = tick;
            st.jet.next_hop = tick;
            ask(&sim, h, &mut st, &request());
            let f = st.jet.flight.expect("a hop");
            let d = nav::xz_dist(f.goal, Vec3::ZERO);
            assert!((20.0..=40.0 * std::f32::consts::SQRT_2).contains(&d), "{d}");
            assert!(f.hop && f.height.is_none() && f.kick == Vec3::ZERO);
            assert!(st.jet.next_hop >= tick + 6 * 30);
            spots.push(f.goal);
        }
        spots.dedup();
        assert!(spots.len() > 30);
        let (sim, h) = sim(hover);
        let mut st = AiState::default();
        st.jet.next_hop = 1;
        ask(&sim, h, &mut st, &request());
        assert!(st.jet.flight.is_none(), "no hop out of a fight");
    }
}
fn ask(sim: &Sim, h: UnitHandle, st: &mut AiState, mv: &MoveRequest) -> PadState {
    let u = sim.unit(h).unwrap();
    let tk = Tick {
        rules: crate::ai::UnitRules::for_unit(sim, u),
        me: h,
        u,
        eye: u.muzzle(),
        wc: &sim.weapon_classes[u.weapon_class],
        tier: sim.unit_tier(u),
        skill: sim.skill_of(u),
    };
    let mut pad = PadState::default();
    run(sim, &tk, st, mv, &mut pad);
    pad
}
#[test]
fn hover_attack_needs_twenty_metres_and_fuel_for_the_trip() {
    for (distance, fuel, want) in [(19.9, 1.0, false), (20.0, 1.0, true), (40.0, 0.3, false)] {
        let (mut sim, h) = sim(true);
        sim.units[h.index as usize].soldier.fuel = fuel;
        let mut st = AiState::default();
        let enemy = sim
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -distance)))
            .unwrap();
        st.target = Some(enemy.into());
        st.contacts[0] = Some(perception::Contact {
            handle: enemy.into(),
            first_seen: 0,
            last_seen: 0,
            last_pos: Vec3::new(0.0, 0.0, -distance),
            seen_now: true,
            heard: false,
            told: false,
        });
        ask(&sim, h, &mut st, &request());
        assert_eq!(st.jet.flight.is_some(), want, "{distance}m {fuel} fuel");
        if want {
            assert_eq!(st.jet.flight.unwrap().height, Some(10.0));
        }
    }
}
#[test]
fn a_marked_launch_area_blocks_the_hover_attack() {
    let (mut sim, h) = sim(true);
    sim.units[h.index as usize].soldier.fuel = 1.0;
    let mut state = AiState::default();
    engage(&mut sim, &mut state, 24.0);
    let mut unmarked = state;
    ask(&sim, h, &mut unmarked, &request());
    assert!(unmarked.jet.flight.is_some());

    sim.nav
        .as_mut()
        .unwrap()
        .attack_takeoff_keepout
        .push(crate::collision::Aabb {
            min: Vec3::splat(-1.0),
            max: Vec3::splat(1.0),
        });
    ask(&sim, h, &mut state, &request());
    assert!(state.jet.flight.is_none());
}
#[test]
fn a_pending_flight_jumps_first_then_uses_the_bot_kick_without_a_second_tap() {
    use crate::soldier::{AirEnv, GroundRules};
    let (sim, h) = sim(true);
    let mut st = AiState::default();
    st.jet.next_hop = 1;
    let mut sim = sim;
    engage(&mut sim, &mut st, 10.0);
    sim.tick = 1;
    let pad = ask(&sim, h, &mut st, &request());
    assert!(pad.pressed(buttons::JUMP));
    let u = &mut sim.units[h.index as usize];
    let env = AirEnv {
        human: false,
        bot_jet: Some(BotJet {
            kick: Vec3::Y * 2.0,
            height: None,
            stop: false,
        }),
        ..Default::default()
    };
    u.soldier.fuel = 1.0;
    u.soldier
        .step_looking(crate::TICK_DT, &pad, &u.consts, GroundRules::Bf2, 1.0, env);
    assert_eq!(u.soldier.stance, Stance::Jump);
    u.soldier.grounded = false;
    let mut control = u.soldier.clone();
    control.step_looking(
        crate::TICK_DT,
        &PadState::default(),
        &u.consts,
        GroundRules::Bf2,
        1.0,
        AirEnv {
            human: false,
            ..Default::default()
        },
    );
    u.soldier.step_looking(
        crate::TICK_DT,
        &PadState::default(),
        &u.consts,
        GroundRules::Bf2,
        1.0,
        env,
    );
    assert_eq!(u.soldier.stance, Stance::Hover);
    assert!((u.soldier.vel.y - control.vel.y - 2.0 * u.consts.jet_jump).abs() < 1e-5);
    assert!((u.soldier.fuel - control.fuel + u.consts.jet_fuel_initial).abs() < 1e-5);
}

#[test]
fn jump_hints_request_only_non_hover_packs() {
    for hover in [false, true] {
        let (mut sim, h) = sim(hover);
        let hint = nav::Hint {
            name: "jet".into(),
            kind: 4,
            pos: Vec3::ZERO,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            radius: Some(2.0),
            mode: None,
            stance_mask: 1,
            secondary_stance_mask: 1,
            peek_sides: 0,
            command_post: None,
        };
        sim.nav.as_mut().unwrap().hints.push(hint);
        sim.tick = 90;
        sim.units[h.index as usize].soldier.fuel = 1.0;
        let mut st = AiState {
            jet: JetState {
                next_hop: 1000,
                next_hint: 90,
                flight: None,
            },
            ..Default::default()
        };
        ask(&sim, h, &mut st, &request());
        assert_eq!(st.jet.flight.is_some(), !hover);
        if !hover {
            assert!((90 + 3 * 30..=90 + 8 * 30).contains(&st.jet.next_hint));
        }
    }
}

#[test]
fn hover_height_steering_uses_the_ps2_gain_and_cap() {
    use crate::soldier::{AirEnv, GroundRules};
    let c = class(true);
    for height in [-100.0, 9.0, 10.0, 11.0, 100.0] {
        let mut actual = crate::soldier::Soldier::new(Vec3::Y * 10.0);
        actual.stance = Stance::Hover;
        actual.grounded = false;
        actual.fuel = 1.0;
        let mut control = actual.clone();
        // Independent reference: ask the existing human hover mover to use
        // exactly the PS2 bot's body-angle correction, leaving its gun aim alone.
        control.pitch =
            (0.35_f32 * (height - 10.0)).clamp(-30.0 * crate::TICK_DT, 30.0 * crate::TICK_DT);
        let pad = PadState {
            ly: 32767,
            ..Default::default()
        };
        control.step_looking(
            crate::TICK_DT,
            &pad,
            &c,
            GroundRules::Bf2,
            1.0,
            AirEnv::default(),
        );
        actual.step_looking(
            crate::TICK_DT,
            &pad,
            &c,
            GroundRules::Bf2,
            1.0,
            AirEnv {
                human: false,
                bot_jet: Some(BotJet {
                    kick: Vec3::ZERO,
                    height: Some(height),
                    stop: false,
                }),
                ..Default::default()
            },
        );
        assert!(actual.vel.distance(control.vel) < 1e-6, "height {height}");
        assert_eq!(actual.pitch, 0.0, "body steering must not redirect the gun");
    }
}

#[test]
fn a_short_elevated_jet_link_keeps_jetting_until_above_its_platform() {
    let (mut sim, h) = sim(true);
    let goal = Vec3::Y * 5.0;
    sim.units[h.index as usize].soldier.stance = Stance::Hover;
    sim.units[h.index as usize].soldier.grounded = false;
    let mut st = AiState {
        jet: JetState {
            flight: Some(Flight {
                goal,
                kick: Vec3::Y,
                height: Some(5.0),
                route: true,
                hop: false,
                since: 0,
                launched: true,
                stop: false,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    ask(&sim, h, &mut st, &request());
    assert!(
        !st.jet.command().unwrap().stop,
        "same horizontal point is still under the deck"
    );
    sim.units[h.index as usize].soldier.pos.y = 5.0;
    ask(&sim, h, &mut st, &request());
    assert!(st.jet.command().unwrap().stop);
}

#[test]
fn sideways_hover_travel_climbs_without_turning_the_weapon() {
    use crate::soldier::{AirEnv, GroundRules};
    let c = class(true);
    let mut s = crate::soldier::Soldier::new(Vec3::Y * 10.0);
    s.stance = Stance::Hover;
    s.grounded = false;
    s.fuel = 1.0;
    s.step_looking(
        crate::TICK_DT,
        &PadState {
            lx: 32767,
            ..Default::default()
        },
        &c,
        GroundRules::Bf2,
        1.0,
        AirEnv {
            human: false,
            bot_jet: Some(BotJet {
                kick: Vec3::ZERO,
                height: Some(20.0),
                stop: false,
            }),
            ..Default::default()
        },
    );
    assert!(s.vel.y > 0.5 && s.vel.x > 0.0);
    assert_eq!(s.yaw, 0.0);
    assert_eq!(s.pitch, 0.0);
}

#[test]
fn losing_the_only_goal_stops_and_discards_the_flight() {
    let (mut sim, h) = sim(true);
    sim.nav = None;
    for unit in &mut sim.units {
        unit.alive = false;
    }
    let u = &mut sim.units[h.index as usize];
    u.alive = true;
    u.soldier.stance = Stance::Hover;
    u.soldier.grounded = false;
    u.soldier.pos.y = 10.0;
    u.soldier.fuel = 1.0;
    u.ai.next_think = u64::MAX;
    u.ai.jet.flight = Some(Flight {
        goal: Vec3::new(20.0, 0.0, 0.0),
        kick: Vec3::Y,
        height: Some(10.0),
        route: false,
        hop: false,
        since: 0,
        launched: true,
        stop: false,
    });
    sim.step(&[]);
    assert_eq!(sim.unit(h).unwrap().soldier.stance, Stance::Fall);
    sim.step(&[]);
    assert_eq!(sim.unit(h).unwrap().ai.jet.flight, None);
}

#[test]
fn review_pr120_soldier_roof_blocks_hop_like_ordnance_roof() {
    use crate::collision::{Body, Layers, Shape};
    let outcomes = [Layers::ALL, Layers::SOLDIER].map(|layers| {
        let (mut s, h) = sim(true);
        s.collision.push_body(Body::with_layers(
            glam::Mat4::from_translation(Vec3::Y * 5.0),
            Shape::Box {
                half: Vec3::new(100.0, 0.5, 100.0),
            },
            layers,
        ));
        s.tick = 1;
        let mut st = AiState::default();
        engage(&mut s, &mut st, 10.0);
        st.jet.next_hop = 1;
        ask(&s, h, &mut st, &request());
        println!("review roof layers={layers:?} flight={:?}", st.jet.flight);
        st.jet.flight.is_some()
    });
    assert_eq!(
        outcomes,
        [false, false],
        "a solid soldier roof must reject all candidate sky rays"
    );
}

#[test]
fn review_pr120_barrier_and_no_target_controls() {
    let mut outcomes = Vec::new();
    for mode in 0..4 {
        let (mut s, h) = sim(true);
        s.tick = 1;
        let mut st = AiState::default();
        if mode != 0 {
            engage(&mut s, &mut st, 10.0);
        }
        if mode >= 2 {
            s.nav.as_mut().unwrap().barriers.push(crate::nav::Barrier {
                centre: glam::Vec2::ZERO,
                axes: [glam::Vec2::X, glam::Vec2::Y],
                half: glam::Vec2::splat(100.0),
                mask: 1,
                enabled: mode == 2,
            });
        }
        st.jet.next_hop = 1;
        ask(&s, h, &mut st, &request());
        outcomes.push(st.jet.flight.is_some());
    }
    println!("review no-target/open/enabled-barrier/disabled-barrier: {outcomes:?}");
    assert_eq!(outcomes, [false, true, false, true]);
}

#[test]
fn tactical_hop_sky_preserves_sight_and_sees_current_movers() {
    use crate::collision::{Body, HullPart, Layers, Shape};
    use glam::Mat4;
    let (mut s, h) = sim(true);
    let spot = Vec3::X * 25.0;
    let roof = |layers| {
        Body::with_layers(
            Mat4::from_translation(spot + Vec3::Y * 5.0),
            Shape::Box {
                half: Vec3::splat(1.0),
            },
            layers,
        )
    };
    assert!(hop_sky_clear(&s, h, spot));
    s.collision.push_body(roof(Layers::SOLDIER));
    assert!(!hop_sky_clear(&s, h, spot));
    assert!(perception::los(&s, spot, spot + Vec3::Y * HOP_SKY));
    let (mut s, h) = sim(true);
    let owner = crate::vehicle::VehicleHandle { index: 7, gen: 3 };
    let parts = [HullPart::new(
        Mat4::IDENTITY,
        roof(Layers::SOLDIER).shape,
        Layers::SOLDIER,
    )];
    s.collision
        .set_hull(owner, &parts, Mat4::from_translation(spot + Vec3::Y * 5.0));
    assert!(!hop_sky_clear(&s, h, spot), "a moving hull blocks");
    s.collision
        .set_hull(owner, &parts, Mat4::from_translation(spot + Vec3::X * 10.0));
    assert!(hop_sky_clear(&s, h, spot), "the old hull pose is gone");
    s.collision.push_body(Body::with_layers(
        Mat4::from_translation(spot + Vec3::Y * 42.0),
        Shape::Box {
            half: Vec3::splat(1.0),
        },
        Layers::ALL,
    ));
    assert!(hop_sky_clear(&s, h, spot), "beyond the 40 m ray");
    let actor = s
        .spawn(&UnitSpec::trooper(0, spot + Vec3::Y * 4.0))
        .unwrap();
    assert!(
        !hop_sky_clear(&s, h, spot),
        "another soldier blocks regardless of team"
    );
    assert!(
        hop_sky_clear(&s, actor, spot),
        "the query excludes its owner"
    );
    s.kill_environment(actor, crate::sim::DeathCause::DeathRegion);
    assert!(hop_sky_clear(&s, h, spot), "dead soldiers do not block");
}

#[test]
fn kamino_hops_require_clear_barrier_lines_only_when_blind_jumps_are_off() {
    for blind in [false, true] {
        for enabled in [false, true] {
            let (mut s, h) = sim(true);
            s.mission_ai.allow_blind_jet_jumps = blind;
            s.tick = 1;
            // A small closed ring surrounds the start, not the candidate
            // destinations. Every candidate passes the existing point test.
            for axis in [glam::Vec2::X, glam::Vec2::Y] {
                for side in [-1.0, 1.0] {
                    s.nav.as_mut().unwrap().barriers.push(crate::nav::Barrier {
                        centre: axis * side * 5.0,
                        axes: [glam::Vec2::X, glam::Vec2::Y],
                        half: if axis.x != 0.0 {
                            glam::Vec2::new(0.1, 6.0)
                        } else {
                            glam::Vec2::new(6.0, 0.1)
                        },
                        mask: 1,
                        enabled,
                    });
                }
            }
            let mut st = AiState::default();
            engage(&mut s, &mut st, 10.0);
            st.jet.next_hop = 1;
            ask(&s, h, &mut st, &request());
            assert_eq!(
                st.jet.flight.is_some(),
                blind || !enabled,
                "blind={blind}, enabled={enabled}"
            );
        }
    }
    let (mut s, _) = sim(true);
    let hash = s.hash();
    s.mission_ai.allow_blind_jet_jumps = false;
    assert_ne!(
        hash,
        s.hash(),
        "mission policy affects deterministic continuation"
    );
}

#[test]
fn abandoned_hover_attack_drops_to_the_deck_after_wall_contact() {
    use crate::soldier::{AirEnv, GroundRules};

    let (mut sim, h) = sim(true);
    let mut state = AiState {
        jet: JetState {
            flight: Some(Flight {
                goal: Vec3::NEG_Z * 30.0,
                kick: Vec3::Y,
                height: Some(10.0),
                route: false,
                hop: false,
                since: 0,
                launched: true,
                stop: false,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    sim.tick = 90;
    let u = &mut sim.units[h.index as usize];
    u.soldier.stance = Stance::Hover;
    u.soldier.grounded = false;
    u.soldier.pos.y = 0.03;
    u.soldier.fuel = 0.5;
    u.soldier.time_in_stance = 2.0;
    // The wall pass removed the previous step's entire forward movement.
    u.wall_push = Vec3::Z * 0.02;

    let mut linked = state;
    linked.jet.flight.as_mut().unwrap().route = true;
    ask(&sim, h, &mut linked, &request());
    assert!(!linked.jet.command().unwrap().stop);

    let mut tracking = state;
    engage(&mut sim, &mut tracking, 30.0);
    ask(&sim, h, &mut tracking, &request());
    assert!(!tracking.jet.command().unwrap().stop);

    let mut pending = state;
    pending.jet.flight.as_mut().unwrap().launched = false;
    sim.units[h.index as usize].soldier.stance = Stance::Jump;
    ask(&sim, h, &mut pending, &request());
    assert!(pending.jet.command().is_none());
    sim.units[h.index as usize].soldier.stance = Stance::Hover;

    let pad = ask(&sim, h, &mut state, &request());
    assert!(
        state.jet.command().unwrap().stop,
        "still flying into the wall"
    );
    let u = &mut sim.units[h.index as usize];
    u.soldier.step_looking(
        crate::TICK_DT,
        &pad,
        &u.consts,
        GroundRules::Bf2,
        1.0,
        AirEnv {
            human: false,
            bot_jet: state.jet.command(),
            ..Default::default()
        },
    );
    assert_eq!(u.soldier.stance, Stance::Fall);
}
