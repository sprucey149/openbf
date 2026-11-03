use super::*;

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
fn the_thirteenth_anchor_releases_the_live_link() {
    use crate::collision::Shape;
    use glam::Mat4;
    let part = |pos, leg, offset| TowPrimitive {
        frame: Mat4::from_translation(pos),
        shape: Shape::Sphere { radius: 1.0 },
        offset,
        leg: Some(leg),
    };
    let parts = [
        part(Vec3::ZERO, 0, Vec3::X),
        part(Vec3::X * 5.0, 1, Vec3::NEG_X),
        part(Vec3::Z * 5.0, 3, Vec3::X),
    ];
    let anchor = |prim| Anchor {
        prim,
        at: Vec3::X,
        tangent: Vec3::ZERO,
    };
    for count in [11, 12] {
        let mut anchors = vec![anchor(2); count];
        anchors[count - 1] = anchor(0);
        let mut points = Vec::new();
        let active = advance_latch(
            &parts,
            &mut anchors,
            Vec3::X * 10.0,
            Vec3::NEG_X,
            TowClass {
                initial_length: 60.0,
                length: 60.0,
            },
            crate::TICK_DT,
            &mut points,
        );
        assert_eq!(anchors.len(), count + 1);
        assert_eq!(anchors.last().unwrap().prim, 1);
        assert_eq!(active, count == 11);
    }
}

#[test]
fn detached_wraps_damage_per_occupied_seat_then_play_the_fall_clip() {
    use crate::collision::{Body, Shape};
    use crate::unit::UnitSpec;
    use crate::vehicle::tow::TowFrames;
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    use glam::Mat4;
    use std::sync::Arc;

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
    let mut s = Sim::empty(42);
    s.tow_frames = Some(Arc::new(Leg));
    s.weapon_classes[0].ordnance.max_damage = 50.0;
    s.weapon_classes[0].ordnance.scales = [1.0; 7];
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
    let mut anchors = vec![
        Anchor {
            prim: 0,
            at: Vec3::X,
            tangent: Vec3::ZERO
        };
        WRAPS
    ];
    let mut points = Vec::new();
    let shooter = Shooter {
        owner: s.vehicle(source).unwrap().seats[1].occupant.unwrap(),
        team: 0,
        weapon_class: 0,
        is_player: false,
    };
    // BF1 sends one packet with its original team. First verify it
    // honors friendly fire, then verify its ordinary hostile damage.
    s.ground_rules = crate::soldier::GroundRules::Bf1;
    assert!(s.step_tripped_tow(
        target,
        &mut anchors,
        source,
        shooter,
        crate::TICK_DT,
        &mut points
    ));
    assert_eq!(s.vehicle(target).unwrap().health.cur, 150.0);
    s.vehicle_mut(target).unwrap().team = Some(1);
    assert!(s.step_tripped_tow(
        target,
        &mut anchors,
        source,
        shooter,
        crate::TICK_DT,
        &mut points
    ));
    assert_eq!(s.vehicle(target).unwrap().health.cur, 100.0);
    s.vehicle_mut(target).unwrap().health.cur = 150.0;
    s.vehicle_mut(target).unwrap().team = Some(0);
    s.ground_rules = crate::soldier::GroundRules::Bf2;
    let rng = s.melee_lcg.state();
    assert!(s.step_tripped_tow(
        target,
        &mut anchors,
        source,
        shooter,
        crate::TICK_DT,
        &mut points
    ));
    assert_eq!(s.vehicle(target).unwrap().health.cur, 50.0);
    assert_eq!(points.len(), WRAPS);
    let mut oracle = crate::lcg::GameLcg::new(rng);
    let _ = oracle.unit();
    assert_eq!(s.melee_lcg.state(), oracle.state());
    assert!(!s.step_tripped_tow(
        target,
        &mut anchors,
        source,
        shooter,
        crate::TICK_DT,
        &mut points
    ));
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
