use super::*;
use crate::collision::{Body, Shape};
use crate::vehicle::tow::TowFrames;
use glam::Mat4;

struct Leg;

impl TowFrames for Leg {
    fn primitives(&self, vehicle: &Vehicle) -> Vec<TowPrimitive> {
        vec![TowPrimitive {
            frame: Mat4::from_translation(vehicle.pos),
            shape: Shape::Sphere { radius: 1.0 },
            offset: Vec3::X,
            leg: Some(0),
        }]
    }

    fn fallen_bodies(&self, vehicle: &Vehicle) -> Vec<Body> {
        let pos = vehicle.pos + Vec3::Y * vehicle.walker.clip_time * 4.0;
        vec![Body::with_layers(
            Mat4::from_translation(pos),
            Shape::Sphere { radius: 1.0 },
            Layers::SOLDIER | Layers::ORDNANCE,
        )]
    }
}

#[test]
fn the_leg_ring_accepts_forward_wraps_and_rejects_reversal() {
    for leg in 0..4 {
        let next = (leg + 1) % 4;
        let opposite = (leg + 2) % 4;
        let previous = (leg + 3) % 4;
        assert_eq!(wrap_order(leg, leg, Some(previous)), 0);
        assert_eq!(wrap_order(next, leg, Some(previous)), 1);
        assert_eq!(wrap_order(previous, leg, Some(previous)), -1);
        assert_eq!(wrap_order(opposite, leg, Some(previous)), -1);
        assert_eq!(wrap_order(opposite, leg, None), 1);
        assert_eq!(wrap_order(previous, leg, Some(opposite)), 1);
    }
}

#[test]
fn a_full_ground_cable_releases_then_damages_on_separate_updates() {
    use crate::soldier::GroundRules;
    use crate::unit::UnitSpec;
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    use glam::Quat;
    use std::sync::Arc;

    struct Ring;
    impl TowFrames for Ring {
        fn primitives(&self, vehicle: &Vehicle) -> Vec<TowPrimitive> {
            [
                (Vec3::ZERO, 0, Vec3::X),
                (Vec3::X * 5.0, 1, Vec3::NEG_X),
                (Vec3::Z * 5.0, 3, Vec3::X),
            ]
            .into_iter()
            .map(|(center, leg, offset)| TowPrimitive {
                frame: Mat4::from_translation(vehicle.pos + center),
                shape: Shape::Sphere { radius: 1.0 },
                offset,
                leg: Some(leg),
            })
            .collect()
        }
    }

    for (rules, space, source_lost) in [
        (GroundRules::Bf1, false, false),
        (GroundRules::Bf2, false, false),
        (GroundRules::Bf2, false, true),
        (GroundRules::Bf2, true, false),
    ] {
        let mut sim = Sim::empty(42);
        sim.ground_rules = rules;
        if space {
            sim.space_assault = Some(crate::sim::SpaceAssault::new(10));
        }
        sim.tow_frames = Some(Arc::new(Ring));
        sim.weapon_classes[0].ordnance.scales = [1.0; 7];
        sim.seat_setup = vec![Some(SeatSetup {
            radius: 1.0,
            boardable: vec![true],
            body_active: vec![],
            exit_drop: 10.0,
            ai_size: 1,
            guns: vec![Some(gun::GunSetup {
                weapons: vec![],
                fire_point: Vec3::ZERO,
                pivot: Vec3::ZERO,
                eye: Vec3::ZERO,
                aimer: gun::Aimer::default(),
                pilot: true,
                turn: None,
                framed: false,
                aimer_bind: Quat::IDENTITY,
                gunner: None,
            })],
        })];
        let mut spawn = |kind, team, pos, yaw, seats| {
            sim.spawn_vehicle(&VehicleSpec {
                kind,
                class: 0,
                home_team: Some(team),
                neutral_when_empty: false,
                spawn_pad: None,
                pos,
                yaw,
                max_health: 150.0,
                health_type: HealthType::Vehicle,
                seats,
            })
        };
        let source = spawn(
            VehicleKind::Flyer,
            0,
            Vec3::X * 10.0,
            std::f32::consts::FRAC_PI_2,
            vec![SeatKind::Pilot],
        );
        let target = spawn(VehicleKind::Walker, 1, Vec3::ZERO, 0.0, vec![]);
        let owner = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        assert!(sim.seat_unit(owner, source, 0));
        let mount = sim.unit(owner).unwrap().mount.unwrap();
        sim.arm(source.index as usize);
        let (muzzle, aim) = sim.tow_muzzle(mount).unwrap();
        assert_eq!(muzzle, Vec3::X * 10.0);
        assert!(aim.abs_diff_eq(Vec3::NEG_X, 1.0e-6));
        let shooter = Shooter {
            owner,
            team: 0,
            weapon_class: 0,
            is_player: false,
        };
        let class = TowClass {
            initial_length: 60.0,
            length: 60.0,
        };
        let fire = |sim: &mut Sim| {
            sim.fire_tow(mount, shooter, class, Vec3::ZERO, Vec3::Z, 250.0, 1.0);
        };
        fire(&mut sim);
        // The live sweep inserts the final contact from the 12-record state.
        let anchor = |prim| Anchor {
            prim,
            at: Vec3::X,
            tangent: Vec3::ZERO,
        };
        let mut anchors = vec![anchor(2); WRAPS - 1];
        anchors[WRAPS - 2] = anchor(0);
        sim.tow_cables[0].flight = Flight::Latched { target, anchors };
        sim.tow_cables[0].points = vec![Vec3::Z];

        let rng = sim.melee_lcg.state();
        sim.fly_tow_cables(crate::TICK_DT);
        assert_eq!(sim.vehicle(target).unwrap().health.cur, 150.0);
        assert_eq!(sim.melee_lcg.state(), rng);
        assert_eq!(sim.tow_cables[0].tripped(), space);
        if space {
            assert_eq!(sim.tow_cables[0].points, [Vec3::Z]);
        } else {
            let Flight::Latched { anchors, .. } = &sim.tow_cables[0].flight else {
                panic!("the full ground cable should still be latched");
            };
            assert_eq!(anchors.len(), WRAPS);
            assert_eq!(anchors.last().unwrap().prim, 1);
            assert!(sim.tow_cables[0].points.is_empty());
            fire(&mut sim);
            assert_eq!(
                sim.tow_cables.len(),
                1,
                "the last record keeps the live link"
            );
            let live_hash = sim.hash();
            if source_lost {
                sim.remove_vehicle(source);
            }
            sim.fly_tow_cables(crate::TICK_DT);
            assert_eq!(sim.vehicle(target).unwrap().health.cur, 150.0);
            assert_eq!(sim.melee_lcg.state(), rng);
            if source_lost {
                assert!(sim.tow_cables.is_empty());
                continue;
            }
            assert!(sim.tow_cables[0].tripped());
            assert_eq!(sim.tow_cables[0].points.len(), WRAPS);
            assert_ne!(sim.hash(), live_hash);
        }
        fire(&mut sim);
        assert_eq!(sim.tow_cables.len(), 2, "release frees the live link");
        sim.fly_tow_cables(crate::TICK_DT);
        assert!(sim.vehicle(target).is_none());
        let mut oracle = crate::lcg::GameLcg::new(rng);
        oracle.unit();
        assert_eq!(sim.melee_lcg.state(), oracle.state());
    }
}

#[test]
fn detached_wraps_damage_per_occupied_seat_then_play_the_fall_clip() {
    use crate::unit::UnitSpec;
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    use std::sync::Arc;

    let mut s = Sim::empty(42);
    s.tow_frames = Some(Arc::new(Leg));
    s.weapon_classes[0].ordnance.max_damage = 50.0;
    s.weapon_classes[0].ordnance.scales = [0.25; 7];
    s.friendly_fire_scale = 0.0;
    let mut params = walker::WalkerParams {
        clips: vec![None; walker::ANIM_COUNT],
        ..Default::default()
    };
    params.clips[usize::from(walker::ANIM_DEATH)] = Some(walker::Clip {
        duration: 2.0,
        looping: false,
        root_z: vec![],
    });
    s.walker_params = vec![None, Some(params)];
    let source = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    let target = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Walker,
        class: 1,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::Z * 10.0,
        yaw: 0.0,
        max_health: 150.0,
        health_type: HealthType::Vehicle,
        seats: vec![],
    });
    for seat in 0..2 {
        let owner = s
            .spawn(&UnitSpec::trooper(0, Vec3::X * seat as f32))
            .unwrap();
        s.vehicle_mut(source).unwrap().seats[seat].occupant = Some(owner);
    }
    let shooter = Shooter {
        owner: s.vehicle(source).unwrap().seats[1].occupant.unwrap(),
        team: 0,
        weapon_class: 0,
        is_player: false,
    };
    let mut cable = TowCable {
        mount: Mount {
            vehicle: source,
            seat: 1,
        },
        shooter,
        class: TowClass {
            initial_length: 60.0,
            length: 60.0,
        },
        flight: Flight::Detached {
            target,
            anchors: vec![
                Anchor {
                    prim: 0,
                    at: Vec3::X,
                    tangent: Vec3::ZERO,
                };
                WRAPS
            ],
        },
        points: Vec::new(),
    };
    // BF1 sends one packet with its original team. First verify it
    // honors friendly fire, then verify its ordinary hostile damage.
    s.ground_rules = crate::soldier::GroundRules::Bf1;
    assert!(s.step_full_tow(&mut cable, crate::TICK_DT));
    assert_eq!(s.vehicle(target).unwrap().health.cur, 150.0);
    s.vehicle_mut(target).unwrap().team = Some(1);
    s.vehicle_mut(target).unwrap().health.cur = 120.0;
    assert!(s.step_full_tow(&mut cable, crate::TICK_DT));
    assert_eq!(s.vehicle(target).unwrap().health.cur, 82.5);
    s.vehicle_mut(target).unwrap().health.cur = 120.0;
    s.vehicle_mut(target).unwrap().team = Some(0);
    s.ground_rules = crate::soldier::GroundRules::Bf2;
    let rng = s.melee_lcg.state();
    assert!(s.step_full_tow(&mut cable, crate::TICK_DT));
    assert_eq!(
        s.vehicle(target).unwrap().health.cur,
        45.0,
        "BF2 sends two packets based on max health 150, not remaining 120 or weapon damage 50"
    );
    assert_eq!(cable.points.len(), WRAPS);
    let mut oracle = crate::lcg::GameLcg::new(rng);
    let _ = oracle.unit();
    assert_eq!(s.melee_lcg.state(), oracle.state());
    assert!(!s.step_full_tow(&mut cable, crate::TICK_DT));
    assert!(s.vehicle(target).is_none());
    assert_eq!(s.tow_falls.len(), 1);
    let pose = s.tow_falls[0].pose(s.tick + u64::from(crate::TICK_HZ));
    assert_eq!(pose.walker.anim, walker::ANIM_DEATH);
    assert_eq!(pose.walker.clip_time, 1.0);
    let ray = |s: &Sim, y, filter| {
        s.collision
            .ray_with(Vec3::Y * y, Vec3::Z, 15.0, Layers::ORDNANCE, filter)
    };
    let falling_hit = ray(&s, 0.0, HullFilter::Static).unwrap();
    assert_eq!(falling_hit.t, 9.0);
    assert!(falling_hit.owner.is_none());
    assert!(ray(&s, 0.0, HullFilter::Hulls).is_none());

    s.tick += u64::from(crate::TICK_HZ);
    s.sync_tow_falls();
    assert!(ray(&s, 0.0, HullFilter::Static).is_none());
    assert!(ray(&s, 4.0, HullFilter::Static).is_some());
    s.tick += u64::from(crate::TICK_HZ);
    s.sync_tow_falls();
    assert!(s.tow_falls.is_empty());
    assert!(ray(&s, 4.0, HullFilter::Static).is_none());
}

#[test]
fn the_live_span_uses_the_gun_tangent_in_its_range() {
    let at = Vec3::new(0.0, 0.0, 10.0);
    let points = span(at, Vec3::ZERO, Vec3::ZERO, Vec3::X);
    assert_eq!(points[0], at);
    assert_eq!(points[CHORDS], Vec3::ZERO);
    let length = points.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
    assert!(length > 10.0 && length < 12.0, "{length}");
}

#[test]
fn ps2_tow_previous_leg_is_only_supplied_above_two_attachments() {
    use crate::collision::Shape;
    use glam::Mat4;
    let part = |pos, leg| TowPrimitive {
        frame: Mat4::from_translation(pos),
        shape: Shape::Sphere { radius: 1.0 },
        offset: Vec3::X,
        leg: Some(leg),
    };
    let parts = [
        part(Vec3::ZERO, 0),
        part(Vec3::X * 5.0, 2),
        part(Vec3::Z * 5.0, 3),
    ];
    let anchor = |prim| Anchor {
        prim,
        at: Vec3::X,
        tangent: Vec3::ZERO,
    };
    for count in [1, 2, 3] {
        let mut anchors = vec![anchor(2); count];
        anchors[count - 1] = anchor(0);
        let mut points = Vec::new();
        let active = advance_latch(
            &parts,
            &mut anchors,
            (Vec3::X * 10.0, Vec3::NEG_X),
            TowClass {
                initial_length: 60.0,
                length: 60.0,
            },
            0.0,
            &mut points,
            false,
        );
        assert_eq!(active, count <= 2, "attachment count {count}");
        assert_eq!(anchors.len(), count + usize::from(count <= 2));
        if count <= 2 {
            assert_eq!(anchors.last().unwrap().prim, 1);
        }
    }
}

#[test]
fn ps2_tow_previous_leg_count_includes_the_initial_latch_and_each_insert() {
    use crate::collision::Shape;
    use glam::Mat4;
    let parts: Vec<_> = [0.0, 5.0]
        .into_iter()
        .enumerate()
        .map(|(leg, x)| TowPrimitive {
            frame: Mat4::from_translation(Vec3::X * x),
            shape: Shape::Sphere { radius: 1.0 },
            offset: Vec3::X,
            leg: Some(leg as u8),
        })
        .collect();
    let mut anchors = vec![Anchor {
        prim: 0,
        at: Vec3::X,
        tangent: Vec3::ZERO,
    }];
    let mut points = Vec::new();
    let class = TowClass {
        initial_length: 60.0,
        length: 60.0,
    };
    assert!(advance_latch(
        &parts,
        &mut anchors,
        (Vec3::X * 10.0, Vec3::NEG_X),
        class,
        0.0,
        &mut points,
        false,
    ));
    assert_eq!(
        anchors.iter().map(|a| a.prim).collect::<Vec<_>>(),
        vec![0, 1]
    );
    // With two existing records, reversal is still admitted. The inserted
    // third record is counted on the next update, when reversal is rejected.
    assert!(advance_latch(
        &parts,
        &mut anchors,
        (Vec3::NEG_X * 5.0, Vec3::X),
        class,
        0.0,
        &mut points,
        false,
    ));
    assert_eq!(
        anchors.iter().map(|a| a.prim).collect::<Vec<_>>(),
        vec![0, 1, 0]
    );
    assert!(!advance_latch(
        &parts,
        &mut anchors,
        (Vec3::X * 10.0, Vec3::NEG_X),
        class,
        0.0,
        &mut points,
        false,
    ));
    assert_eq!(anchors.len(), 3);
}

#[test]
fn the_wrap_stores_the_chord_fraction_and_half_the_world_curve_derivative() {
    use crate::collision::Shape;
    use glam::{Mat4, Quat};

    let muzzle = Vec3::new(10.0, 6.0, 0.0);
    let aim = Vec3::NEG_X;
    let incoming = Vec3::new(3.0, 4.0, 2.0);
    let points = span(Vec3::ZERO, incoming, muzzle, aim);
    let contact = points[4].lerp(points[5], 0.25);
    let half = Vec3::new(0.35, 0.35, 0.125);
    let parts = [
        TowPrimitive {
            frame: Mat4::from_translation(Vec3::NEG_X * 5.0),
            shape: Shape::Sphere { radius: 1.0 },
            offset: Vec3::ZERO,
            leg: Some(0),
        },
        TowPrimitive {
            frame: Mat4::from_translation(Vec3::NEG_X),
            shape: Shape::Sphere { radius: 1.0 },
            offset: Vec3::X,
            leg: Some(1),
        },
        TowPrimitive {
            frame: Mat4::from_rotation_translation(
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                contact + Vec3::X * half.z,
            ),
            shape: Shape::Box { half },
            offset: Vec3::ZERO,
            leg: Some(2),
        },
    ];
    let prior = [
        Anchor {
            prim: 0,
            at: Vec3::ZERO,
            tangent: Vec3::ZERO,
        },
        Anchor {
            prim: 1,
            at: Vec3::X,
            tangent: incoming,
        },
    ];
    for space in [false, true] {
        let mut anchors = prior.to_vec();
        assert!(advance_latch(
            &parts,
            &mut anchors,
            (muzzle, aim),
            TowClass {
                initial_length: 60.0,
                length: 60.0,
            },
            0.0,
            &mut Vec::new(),
            space,
        ));
        assert_eq!(anchors.len(), 3);
        let wrap = anchors[2];
        assert_eq!(wrap.prim, 2);
        let expected_position = if space {
            contact
        } else {
            points[4] + (points[5] - points[4]).normalize() * 0.25
        };
        let actual_position = parts[2].frame.transform_point3(wrap.at);
        assert!(
            actual_position.distance(expected_position) < 1e-5,
            "{actual_position:?}"
        );
        // p(t) = (-2, -8, 2)t³ + (9, 10, -4)t² + (3, 4, 2)t. The hit is at t = 0.425.
        let expected = Vec3::new(4.783125, 4.0825, -0.158125);
        assert!(wrap.tangent.distance(expected) < 1e-5, "{wrap:?}");
    }
}

#[test]
fn the_live_tangent_retains_its_world_direction_while_the_contact_moves() {
    use crate::collision::Shape;
    use glam::{Mat4, Quat};

    let start = Vec3::new(0.0, 2.0, 1.0);
    let tangent = Vec3::new(3.0, 6.0, -2.0);
    let older = Vec3::new(-2.0, 2.0, -3.0);
    let parts = [
        TowPrimitive {
            frame: Mat4::from_translation(Vec3::NEG_X * 5.0),
            shape: Shape::Sphere { radius: 1.0 },
            offset: Vec3::ZERO,
            leg: Some(0),
        },
        TowPrimitive {
            frame: Mat4::from_translation(older),
            shape: Shape::Sphere { radius: 0.1 },
            offset: Vec3::X * 0.5,
            leg: Some(1),
        },
        TowPrimitive {
            frame: Mat4::from_rotation_translation(Quat::from_rotation_x(1.0), start),
            shape: Shape::Sphere { radius: 0.1 },
            offset: Vec3::X * 0.25,
            leg: Some(2),
        },
    ];
    let mut anchors = vec![
        Anchor {
            prim: 0,
            at: Vec3::ZERO,
            tangent: Vec3::ZERO,
        },
        Anchor {
            prim: 1,
            at: Vec3::Y * 0.75,
            tangent: Vec3::new(0.0, 3.0, 4.0),
        },
        Anchor {
            prim: 2,
            at: Vec3::ZERO,
            tangent,
        },
    ];
    let muzzle = Vec3::new(10.0, 4.0, 0.0);
    let aim = Vec3::NEG_X;
    let mut points = Vec::new();
    assert!(advance_latch(
        &parts,
        &mut anchors,
        (muzzle, aim),
        TowClass {
            initial_length: 60.0,
            length: 10.43,
        },
        crate::TICK_DT,
        &mut points,
        false,
    ));
    assert_eq!(anchors.len(), 3);
    assert_eq!(anchors[0].tangent, Vec3::ZERO);
    assert!(anchors[1].tangent.distance(Vec3::new(0.0, 2.4, 3.2)) < 1e-6);
    assert_eq!(anchors[2].tangent, tangent);
    assert_eq!(points[1], older + Vec3::new(0.5, 0.75, 0.0));
    let new_start = start + Vec3::X * 0.25;
    assert_eq!(points[2], new_start);
    let expected = new_start * 0.972 + tangent * 0.081 + muzzle * 0.028 + aim * 0.135;
    assert!(points[3].distance(expected) < 1e-5, "{:?}", points[3]);
}
