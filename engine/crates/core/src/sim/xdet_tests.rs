//! XDET, the cross-platform determinism battery (#168,
//! `analysis/plan/LAN_MULTIPLAYER.md` §7). Ignored by default; run it with
//! `--ignored --nocapture xdet` on every target and diff the output
//! (`scripts/xdet.sh` does both). Every line is `scenario tick hash`: the
//! same build on Linux, Windows and aarch64 must print the same lines.
//!
//! The scenes reuse the fixtures of the existing tests: the pinned
//! `--sim-hash` scene (`Sim::new(42)` idle, pinned at ticks 100 and 600), the
//! pinned all-proxy skirmish, the jet trooper, the L graph's bots, the TAT1
//! skiff seats world, the flyer bots, the duel, and a walk through static and
//! hull collision.

use crate::collision::{Body, CollisionWorld, HullPart, Layers, Shape};
use crate::sim::tests::{duel, flat_ground};
use crate::sim::{buttons, PadState, Sim};
use crate::skill::Difficulty;
use crate::unit::{Controller, UnitSpec};
use glam::{Mat4, Quat, Vec3};

/// Steps `sim` `ticks` times with `pads(step)`, printing the hash at step 0,
/// every 30 steps and at step 100 (the `--sim-hash 100` pin).
fn run(name: &str, mut sim: Sim, ticks: u32, mut pads: impl FnMut(u32) -> Vec<PadState>) {
    println!("{name} 0 {:016x}", sim.hash());
    for t in 1..=ticks {
        let p = pads(t);
        sim.step(&p);
        if t % 30 == 0 || t == 100 {
            println!("{name} {t} {:016x}", sim.hash());
        }
    }
}

/// A player's scripted pad: walk, turn, strafe-sprint, jump, fire, crouch,
/// mouse turn and reload, 2 s each, all from integer arithmetic.
fn scripted(t: u32) -> PadState {
    let edge = |every: u32| t.is_multiple_of(every);
    let mut p = PadState::default();
    match (t / 60) % 8 {
        0 => p.ly = i16::MAX,
        1 => {
            p.ly = i16::MAX;
            p.rx = 16000;
        }
        2 => {
            p.lx = -20000;
            p.ly = 9000;
            p.buttons |= buttons::SPRINT;
        }
        3 => {
            p.ly = i16::MAX;
            if edge(15) {
                p.buttons |= buttons::JUMP;
            }
        }
        4 => {
            p.buttons |= buttons::FIRE;
            p.ry = ((t % 40) as i16 - 20) * 600;
            p.rx = -7000;
        }
        5 => {
            if t.is_multiple_of(60) {
                p.buttons |= buttons::ROLL_STANCE;
            }
            p.ly = 20000;
        }
        6 => {
            p.ly = i16::MAX;
            p.mx = 37;
            p.my = -3;
        }
        _ => {
            if t % 60 == 5 {
                p.buttons |= buttons::RELOAD;
            }
        }
    }
    p
}

fn bot(sim: &mut Sim, team: u8, at: Vec3) {
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(team, at)
    })
    .expect("bot");
}

/// Static boxes, a tilted slab and a ramp mesh on flat ground, and a parked
/// box-hulled hover: the player walks a scripted loop through them.
fn collision_scene() -> Sim {
    let mut s = Sim::empty(11);
    s.heightfield = Some(flat_ground());
    let ramp = crate::collision::trimesh(
        &[
            [-3.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 2.5, -8.0],
            [-3.0, 2.5, -8.0],
        ],
        &[0, 1, 2, 0, 2, 3],
    )
    .expect("ramp");
    s.collision = CollisionWorld::new(
        vec![
            Body::new(
                Mat4::from_translation(Vec3::new(0.0, 1.0, -12.0)),
                Shape::Box {
                    half: Vec3::new(4.0, 1.0, 0.5),
                },
            ),
            Body::new(
                Mat4::from_rotation_translation(
                    Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.2),
                    Vec3::new(9.0, 0.4, -6.0),
                ),
                Shape::Box {
                    half: Vec3::new(3.0, 0.4, 3.0),
                },
            ),
            Body::new(
                Mat4::from_translation(Vec3::new(-8.0, 0.0, -4.0)),
                Shape::Mesh(ramp),
            ),
        ],
        Vec::new(),
    );
    s.hull_parts = vec![Some(
        vec![HullPart::new(
            Mat4::from_translation(Vec3::Y),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 3.0),
            },
            Layers::ALL,
        )]
        .into(),
    )];
    s.spawn(&UnitSpec::player(0, Vec3::ZERO)).expect("player");
    s.spawn_vehicle(&crate::vehicle::VehicleSpec {
        kind: crate::vehicle::VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(4.0, 0.0, 6.0),
        yaw: 0.3,
        max_health: 900.0,
        health_type: crate::weapon::HealthType::Vehicle,
        seats: vec![crate::vehicle::SeatKind::Pilot],
    });
    s
}

#[test]
#[ignore = "XDET battery: run with --ignored --nocapture xdet on each target"]
fn xdet_battery() {
    // The pinned `--sim-hash` scene: 100 -> 5573f2b603ed95f4,
    // 600 -> 5c5458c8927f122b.
    run("sim_hash_idle", Sim::new(42), 600, |_| {
        vec![PadState::default()]
    });

    let mut s = Sim::new(42);
    s.heightfield = Some(flat_ground());
    run("player_scripted", s, 960, |t| vec![scripted(t)]);

    for hover in [false, true] {
        let (s, _) = crate::ai::jet::tests::sim(hover);
        run(
            if hover { "jet_hover_bot" } else { "jet_bot" },
            s,
            900,
            |_| vec![],
        );
    }

    // The pinned all-proxy skirmish (`an_all_proxy_skirmish_keeps_its_hash`):
    // 900 -> 67abfedaa0631084 (Easy), e3f2d3c9099e44c4 (Elite).
    for (name, d) in [
        ("proxy_skirmish_easy", Difficulty::Easy),
        ("proxy_skirmish_elite", Difficulty::Elite),
    ] {
        let (mut s, _) = crate::ai::tests::l_sim();
        s.units.clear();
        s.players.clear();
        s.difficulty = d;
        let p = crate::ai::proxy::Strength::Good.profile();
        for (team, x, z) in [
            (0, 0.0, 0.0),
            (0, 3.0, 0.0),
            (1, 0.0, -20.0),
            (1, 3.0, -20.0),
        ] {
            s.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(team, Vec3::new(x, 0.0, z))
            })
            .map(|h| assert!(s.set_proxy(h, Some(p))))
            .expect("proxy");
        }
        run(name, s, 900, |_| vec![]);
    }

    // Bots of both sides on the L graph, its post at hub 2.
    let (mut s, _) = crate::ai::tests::l_sim();
    for (team, x, z) in [
        (0, 2.0, 1.0),
        (0, -2.0, 2.0),
        (1, 40.0, -44.0),
        (1, 36.0, -40.0),
        (1, 44.0, -38.0),
    ] {
        bot(&mut s, team, Vec3::new(x, 0.0, z));
    }
    run("l_bots", s, 900, |_| vec![]);

    // The TAT1 skiff seats world: settle, board, drive and turn, get out
    // and walk.
    let (s, _, _) = crate::sim::seats::tests::world(Vec3::new(0.0, 2.0, -6.0), None, true);
    run("skiff_drive", s, 900, |t| {
        let mut p = PadState::default();
        if t == 61 || t == 721 {
            p.buttons = buttons::ACTION;
        } else if (63..720).contains(&t) {
            p.ly = if (t / 90) % 3 == 2 { -12000 } else { i16::MAX };
            p.rx = match (t / 45) % 4 {
                0 => 0,
                1 => i16::MAX,
                2 => -20000,
                _ => 8000,
            };
            if t % 50 == 0 {
                p.buttons = buttons::FIRE;
            }
        } else if t > 722 {
            p.ly = i16::MAX;
            p.rx = 6000;
        }
        vec![p]
    });

    for (name, bf2) in [("flyer_bots_bf1", false), ("flyer_bots_bf2", true)] {
        let (s, _, _) = crate::ai::flyer_drive::tests::world(bf2, true);
        run(name, s, 900, |_| vec![]);
    }

    // The duel: the player fires bolts at the trooper 20 m ahead, sweeping.
    let (s, _, _) = duel();
    run("duel_fire", s, 600, |t| {
        vec![PadState {
            buttons: if t % 4 < 2 { buttons::FIRE } else { 0 },
            rx: ((t % 60) as i16 - 30) * 200,
            ry: ((t % 22) as i16 - 11) * 150,
            ..PadState::default()
        }]
    });

    run("collision_walk", collision_scene(), 900, |t| {
        let mut p = scripted(t);
        p.lx = p.lx.saturating_add(((t % 70) as i16 - 35) * 300);
        vec![p]
    });
}
