use super::*;
use crate::collision::{trimesh, Body, Shape};
use crate::soldier::Stance;
use glam::Mat4;

/// A running jump up (or down) a ramp clears it in both games: the take-off
/// ADDS the class's jump velocity to the vertical velocity it has. BF2's PS2
/// does (B2 `FUN_00246ed0`, `add.s` at 0x247034); BF1's stores it
/// (`FUN_00208b78`, `swc1` at 0x208bb4) and a sprinting BF1 jump up a 15.5 deg
/// ramp cleared only 0.46 m, so BF1 maps take BF2's rule (a named deviation,
/// FEEL_NOTES 2026-10-25).
#[test]
fn running_jumps_clear_ramps_at_every_phase() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for deg in [-40.0, -20.0, 0.0, 10.0, 15.5, 20.0, 30.0, 40.0] {
            for sprint in [false, true] {
                for jump_tick in 30..60 {
                    for max_speed in [6.5, 9.0] {
                        check_ramp_jump(rules, deg, sprint, jump_tick, max_speed);
                    }
                }
            }
        }
    }
}

fn check_ramp_jump(rules: GroundRules, deg: f32, sprint: bool, jump_tick: u32, max_speed: f32) {
    let context = format!("{rules:?} {deg} deg speed {max_speed} sprint {sprint} tick {jump_tick}");
    let (sn, cs) = crate::fmath::sin_cos(deg.to_radians());
    let grade = sn / cs;
    let mut s = Sim::empty(3);
    s.ground_rules = rules;
    let positions = [
        [-100.0, 50.0 - 100.0 * grade, 100.0],
        [100.0, 50.0 - 100.0 * grade, 100.0],
        [100.0, 50.0 + 100.0 * grade, -100.0],
        [-100.0, 50.0 + 100.0 * grade, -100.0],
    ];
    s.collision = CollisionWorld::new(
        vec![Body::new(
            Mat4::IDENTITY,
            Shape::Mesh(trimesh(&positions, &[0, 1, 2, 0, 2, 3]).unwrap()),
        )],
        Vec::new(),
    );
    let h = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(0.0, 50.0, 0.0)))
        .unwrap();
    s.unit_mut(h).unwrap().consts.max_speed = max_speed;
    for _ in 0..2 {
        s.step(&[PadState::default()]);
    }
    let mut pad = PadState {
        ly: i16::MAX,
        buttons: if sprint { buttons::SPRINT } else { 0 },
        ..PadState::default()
    };
    for _ in 0..jump_tick {
        s.step(&[pad]);
    }
    let before = s.unit(h).unwrap().soldier.clone();
    assert!(before.grounded, "{context}: run-up {before:?}");
    pad.buttons |= buttons::JUMP;
    s.step(&[pad]);
    let after = &s.unit(h).unwrap().soldier;
    assert_eq!(
        after.stance,
        Stance::Jump,
        "{context}: before {before:?}, after {after:?}"
    );
    pad.buttons &= !buttons::JUMP;
    for _ in 0..5 {
        s.step(&[pad]);
    }
    let after = &s.unit(h).unwrap().soldier;
    let clearance = after.pos.y - (50.0 - after.pos.z * grade);
    assert!(
        clearance > 0.5,
        "{context}: clearance {clearance}, before {before:?}, after {after:?}"
    );
}

/// The walkable face overtakes an ascending foot in one step. The same
/// slope over a lower route is clear when the foot starts below its plane.
#[test]
fn an_uphill_jump_stays_on_its_side_of_a_stacked_ramp() {
    for (under, start_y) in [(false, 0.55), (true, 0.25)] {
        let positions = [
            [-2.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
            [2.0, 1.0, -1.0],
            [-2.0, 1.0, -1.0],
        ];
        let world = CollisionWorld::new(
            vec![Body::new(
                Mat4::IDENTITY,
                Shape::Mesh(trimesh(&positions, &[0, 1, 2, 0, 2, 3]).unwrap()),
            )],
            Vec::new(),
        );
        let from = Capsule {
            base: Vec3::new(0.0, start_y, 0.0),
            height: 1.9,
            radius: 0.5,
        };
        let to = Capsule {
            base: from.base + Vec3::new(0.0, 0.025, -0.4),
            ..from
        };
        let mut soldier = crate::soldier::Soldier::new(to.base);
        soldier.stance = Stance::Jump;
        soldier.grounded = false;
        soldier.vel = (to.base - from.base) / TICK_DT;
        let original_vy = soldier.vel.y;
        resolve_soldier_walls(&world, &mut soldier, from, to, true);
        if under {
            assert_eq!(soldier.pos.y, to.base.y);
            assert_eq!(soldier.vel.y, original_vy);
        } else {
            assert!(soldier.pos.y >= 0.5 - 0.5 * soldier.pos.z, "{soldier:?}");
            assert!(soldier.vel.y > original_vy, "{soldier:?}");
        }
    }
}
