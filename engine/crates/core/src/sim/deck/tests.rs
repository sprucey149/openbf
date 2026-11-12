use super::*;
use crate::collision::{HullPart, Layers, Shape};
use crate::sim::input::{buttons, PadState};
use crate::unit::UnitSpec;
use crate::vehicle::{SeatKind, VehicleSpec};
use crate::weapon::HealthType;
use crate::TICK_DT;

fn deck(rules: GroundRules) -> (Sim, usize, usize) {
    let mut s = Sim::empty(5);
    s.ground_rules = rules;
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    s.hull_parts = vec![Some(
        vec![HullPart::new(
            Mat4::from_translation(Vec3::Y),
            Shape::Box {
                half: Vec3::new(4.0, 1.0, 6.0),
            },
            Layers::ALL,
        )]
        .into(),
    )];
    s.sync_hulls();
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(0.0, 2.0, 0.0)))
        .unwrap();
    s.sync_slot_tables();
    (s, p.index as usize, v.index as usize)
}

fn fly(s: &mut Sim, v: usize, vel: Vec3) {
    let f = &mut s.vehicles[v];
    f.vel = vel;
    f.pos += vel * TICK_DT;
}

fn idle(s: &mut Sim) {
    s.step(&[PadState::default()]);
}

#[test]
fn a_rising_then_cruising_flyer_carries_the_soldier_on_its_back() {
    let (mut s, p, v) = deck(GroundRules::Bf2);
    idle(&mut s);
    assert!(s.units[p].deck.is_some(), "his floor is the flyer");
    for _ in 0..30 {
        fly(&mut s, v, Vec3::new(0.0, 3.0, 0.0));
        idle(&mut s);
    }
    for _ in 0..30 {
        fly(&mut s, v, Vec3::new(0.0, 0.0, -20.0));
        idle(&mut s);
    }
    let (feet, hull) = (s.units[p].soldier.pos, s.vehicles[v].pos);
    assert!((feet.y - (hull.y + 2.0)).abs() < 0.05, "{feet} on {hull}");
    assert!((feet.z - hull.z).abs() < 0.05, "{feet} on {hull}");
    assert_eq!(s.units[p].soldier.stance, Stance::Stand);
    assert!(s.units[p].deck.is_some());
}

#[test]
fn a_flyer_that_loses_its_speed_throws_its_rider_on_along_its_heading() {
    let (mut s, p, v) = deck(GroundRules::Bf2);
    idle(&mut s);
    for _ in 0..10 {
        fly(&mut s, v, Vec3::new(0.0, 0.0, -20.0));
        idle(&mut s);
    }
    fly(&mut s, v, Vec3::new(0.0, 0.0, -5.0));
    idle(&mut s);
    let sol = &s.units[p].soldier;
    assert_eq!(sol.stance, Stance::Thrown);
    assert!(s.units[p].deck.is_none());
    assert!(sol.vel.z < -15.0 && sol.vel.y > 3.0, "{}", sol.vel);
}

#[test]
fn a_flyer_tipped_past_the_limit_tumbles_its_rider() {
    let (mut s, p, v) = deck(GroundRules::Bf2);
    idle(&mut s);
    s.vehicles[v].rot = glam::Quat::from_rotation_z(1.2);
    idle(&mut s);
    assert_eq!(s.units[p].soldier.stance, Stance::Tumble);
    assert!(s.units[p].deck.is_none());
}

#[test]
fn a_jump_from_a_moving_deck_keeps_the_flyers_velocity() {
    let (mut s, p, v) = deck(GroundRules::Bf2);
    idle(&mut s);
    for _ in 0..5 {
        fly(&mut s, v, Vec3::new(0.0, 0.0, -20.0));
        idle(&mut s);
    }
    fly(&mut s, v, Vec3::new(0.0, 0.0, -20.0));
    s.step(&[PadState {
        buttons: buttons::JUMP,
        ..PadState::default()
    }]);
    let sol = &s.units[p].soldier;
    assert_eq!(sol.stance, Stance::Jump);
    assert!(sol.platform_vel.is_none());
    assert!(sol.vel.z < -15.0, "{}", sol.vel);
}

#[test]
fn off_the_flyers_floor_for_half_a_second_he_is_let_go() {
    let (mut s, p, v) = deck(GroundRules::Bf2);
    idle(&mut s);
    let _ = v;
    s.units[p].soldier.pos.y += 0.5;
    let mut ticks = 0;
    while s.units[p].deck.is_some() && ticks < 60 {
        idle(&mut s);
        ticks += 1;
    }
    let t = ticks as f32 * TICK_DT;
    assert!((0.5..0.6).contains(&t), "let go after {t} s");
    let y = s.units[p].soldier.pos.y;
    let one_tick = crate::soldier::GRAVITY * TICK_DT * TICK_DT;
    assert!(
        (y - (2.5 - one_tick)).abs() < 1e-3,
        "no gravity while carried: {y}"
    );
}

#[test]
fn bf1_has_no_deck() {
    let (mut s, p, v) = deck(GroundRules::Bf1);
    idle(&mut s);
    assert!(s.units[p].deck.is_none());
    for _ in 0..10 {
        fly(&mut s, v, Vec3::new(0.0, 0.0, -20.0));
        idle(&mut s);
    }
    assert!(s.units[p].soldier.pos.z > -1.0);
}

#[test]
fn a_hangar_shield_holds_the_rider_while_the_flyer_passes_it() {
    use crate::collision::{trimesh, Body, CollisionWorld};
    let (mut s, p, v) = deck(GroundRules::Bf2);
    let z = -4.0;
    let mut shield = Body::new(
        Mat4::IDENTITY,
        Shape::Mesh(
            trimesh(
                &[
                    [-50.0, -5.0, z],
                    [50.0, -5.0, z],
                    [50.0, 20.0, z],
                    [-50.0, 20.0, z],
                ],
                &[0, 1, 2, 0, 2, 3],
            )
            .unwrap(),
        ),
    );
    shield.layers = Layers::SOLDIER;
    s.collision = CollisionWorld::new(vec![shield], Vec::new());
    s.sync_hulls();
    idle(&mut s);
    assert!(s.units[p].deck.is_some(), "his floor is the flyer");
    for _ in 0..30 {
        fly(&mut s, v, Vec3::new(0.0, 0.0, -20.0));
        idle(&mut s);
    }
    let sol = &s.units[p].soldier;
    assert!(sol.pos.z > z, "held on the hangar side: {}", sol.pos);
    assert!(s.units[p].deck.is_none(), "let go once off its floor");
    assert!(s.vehicles[v].pos.z < -15.0);
}

#[test]
fn the_flyers_own_fin_does_not_shove_a_held_rider_through_the_shield() {
    use crate::collision::{trimesh, Body, CollisionWorld};
    let (mut s, p, v) = deck(GroundRules::Bf2);
    s.hull_parts = vec![Some(
        vec![
            HullPart::new(
                Mat4::from_translation(Vec3::Y),
                Shape::Box {
                    half: Vec3::new(4.0, 1.0, 6.0),
                },
                Layers::ALL,
            ),
            HullPart::new(
                Mat4::from_translation(Vec3::new(0.0, 3.0, 4.0)),
                Shape::Box {
                    half: Vec3::new(1.5, 1.0, 0.5),
                },
                Layers::ALL,
            ),
        ]
        .into(),
    )];
    let z = -4.0;
    let mut shield = Body::new(
        Mat4::IDENTITY,
        Shape::Mesh(
            trimesh(
                &[
                    [-50.0, -5.0, z],
                    [50.0, -5.0, z],
                    [50.0, 20.0, z],
                    [-50.0, 20.0, z],
                ],
                &[0, 1, 2, 0, 2, 3],
            )
            .unwrap(),
        ),
    );
    shield.layers = Layers::SOLDIER;
    s.collision = CollisionWorld::new(vec![shield], Vec::new());
    s.sync_hulls();
    idle(&mut s);
    assert!(s.units[p].deck.is_some(), "his floor is the flyer");
    let mut furthest = f32::MAX;
    for _ in 0..30 {
        fly(&mut s, v, Vec3::new(0.0, 0.0, -10.0));
        idle(&mut s);
        furthest = furthest.min(s.units[p].soldier.pos.z);
    }
    assert!(furthest > z, "held on the hangar side: {furthest}");
    assert!(s.vehicles[v].pos.z < -9.0);
}
