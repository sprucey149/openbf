use super::*;
use crate::collision::{Body, HullPart, Layers, Shape};
use crate::sim::{buttons, PadState, Station, StationClass};
use crate::unit::{SlotSpec, UnitHandle, UnitSpec};
use crate::weapon::{DamageDesc, OrdnanceClass, Repair};
use glam::Mat4;

fn cutter(bf2: bool) -> WeaponClass {
    let mut health = [0.0; 8];
    health[3] = if bf2 { 200.0 } else { 60.0 };
    health[4] = if bf2 { 100.0 } else { 60.0 };
    health[5] = if bf2 { 50.0 } else { 20.0 };
    health[6] = if bf2 { 50.0 } else { 30.0 };
    WeaponClass {
        name: "fusioncutter".into(),
        repair: Some(Repair { health }),
        target_team: 6,
        lock_on_range: if bf2 { 4.0 } else { 3.0 },
        lock_on_angle: if bf2 { 90.0 } else { 30.0 } * crate::weapon::DEG,
        shot_delay: [0.5; 2],
        heat_per_shot: 0.06,
        ..WeaponClass::default()
    }
}

fn fixture(bf2: bool, secondary: bool) -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(5);
    sim.heightfield = Some(super::super::tests::flat_ground());
    sim.ground_rules = if bf2 {
        crate::soldier::GroundRules::Bf2
    } else {
        crate::soldier::GroundRules::Bf1
    };
    sim.weapon_classes.push(cutter(bf2));
    let mut spec = UnitSpec::player(0, Vec3::new(0.0, 0.0, 2.5));
    if secondary {
        spec.slots = vec![SlotSpec {
            slot: 3,
            channel: 1,
            weapon_class: 1,
            clips: 0,
        }]
        .into();
    } else {
        spec.weapon_class = 1;
    }
    let owner = sim.spawn(&spec).unwrap();
    let mut class = StationClass::soldier("medical", 0.5, 15.0, 0);
    class.sphere = Some([0.0, 1.0, 0.0, 1.2]);
    class.wreck_sphere = Some([0.0, 0.2, 0.0, 0.6]);
    sim.add_station_class(class);
    let mut s = Station::new(0, Vec3::ZERO, 5.0);
    s.instance = Some(7);
    s.sphere_centre = Vec3::Y;
    s.sphere_radius = 1.2;
    sim.stations.push(s);
    let mut body = Body::new(
        Mat4::from_translation(Vec3::Y),
        Shape::Cylinder {
            radius: 0.4,
            half_height: 1.0,
        },
    );
    body.instance = Some(7);
    sim.collision.push_body(body);
    sim.collision.bind_station(7, 0);
    sim.station_wreck_collision.push(vec![HullPart::new(
        Mat4::from_translation(Vec3::Y * 0.2),
        Shape::Box {
            half: Vec3::new(0.4, 0.2, 0.4),
        },
        Layers::ALL,
    )]);
    (sim, owner)
}

fn destroy(sim: &mut Sim) {
    let damage: DamageDesc = OrdnanceClass {
        max_damage: 500.0,
        ..Default::default()
    }
    .damage_desc(0.0, false);
    sim.damage_station(0, &damage, 1.0, None, None);
    assert!(sim.stations[0].destroyed);
}

fn fire(secondary: bool) -> PadState {
    PadState {
        buttons: if secondary {
            buttons::SECONDARY_FIRE
        } else {
            buttons::FIRE
        },
        ..Default::default()
    }
}

#[test]
fn both_games_repair_and_rebuild_through_their_weapon_channels_at_disc_rates() {
    for (bf2, secondary) in [(false, true), (true, false)] {
        let (mut sim, owner) = fixture(bf2, secondary);
        sim.stations[0].health.cur = 10.0;
        sim.step(&[fire(secondary)]);
        assert_eq!(sim.stations[0].health.cur, if bf2 { 80.0 } else { 70.0 });
        assert!(sim.bolts.is_empty() && sim.stickies.is_empty());
        for _ in 0..16 {
            sim.step(&[PadState::default()]);
        }
        destroy(&mut sim);
        sim.unit_mut(owner).unwrap().health.cur = 100.0;
        let mut shots = Vec::new();
        let mut health_before = 0.0;
        for tick in 0..100 {
            sim.step(&[fire(secondary)]);
            if sim.stations[0].health.cur != health_before {
                let delta = sim.stations[0].health.cur - health_before;
                assert_eq!(
                    delta,
                    if bf2 {
                        50.0_f32.min(80.0 - health_before)
                    } else {
                        20.0
                    }
                );
                health_before = sim.stations[0].health.cur;
                shots.push(tick);
            }
            if !sim.stations[0].destroyed {
                break;
            }
            assert_eq!(sim.unit(owner).unwrap().health.cur, 100.0);
            assert!(!sim.stations[0].serving);
        }
        assert_eq!(shots.len(), if bf2 { 2 } else { 4 });
        assert!(shots.windows(2).all(|w| w[1] - w[0] == 15));
        assert_eq!(sim.stations[0].health.cur, 80.0);
        assert!(!sim.stations[0].destroyed);
        assert_eq!(sim.collision.station(7), Some(0));
        assert_eq!(sim.stations[0].sphere_centre, Vec3::Y);
        assert_eq!(sim.stations[0].sphere_radius, 1.2);
        let active: Vec<_> = sim
            .collision
            .bodies()
            .iter()
            .filter(|b| b.instance == Some(7) && b.layers != Layers::NONE)
            .collect();
        assert_eq!(active.len(), 1);
        assert!(matches!(active[0].shape, Shape::Cylinder { .. }));
        for _ in 0..16 {
            sim.step(&[PadState::default()]);
        }
        assert!(sim.unit(owner).unwrap().health.cur > 100.0);
        destroy(&mut sim);
        sim.repair_station(0, 80.0);
        assert_eq!(sim.collision.station(7), Some(0));
        assert_eq!(sim.station_bodies[0].len(), 1);
        assert_eq!(sim.collision.bodies().len(), 1);
    }
}

#[test]
fn repair_acquisition_obeys_range_cone_team_and_world_occlusion() {
    for case in 0..5 {
        let (mut sim, owner) = fixture(true, false);
        sim.stations[0].health.cur = 1.0;
        match case {
            0 => sim.unit_mut(owner).unwrap().soldier.pos.z = 10.0,
            1 => sim.unit_mut(owner).unwrap().soldier.yaw = std::f32::consts::PI,
            2 => sim.stations[0].team = Some(1),
            3 => sim.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 1.0, 1.2)),
                Shape::Box {
                    half: Vec3::new(2.0, 2.0, 0.1),
                },
            )),
            _ => sim.unit_mut(owner).unwrap().soldier.energy = 0.0,
        }
        sim.step(&[fire(false)]);
        assert_eq!(sim.stations[0].health.cur, 1.0, "case {case}");
        assert!(!sim.events.iter().any(|e| matches!(e, Event::Repair { .. })));
    }
}

#[test]
fn partial_rebuilds_stay_wrecks_and_identical_inputs_keep_identical_hashes() {
    let run = || {
        let (mut sim, _) = fixture(true, false);
        destroy(&mut sim);
        let dead = sim.hash();
        sim.repair_station(0, 30.0);
        assert!(sim.stations[0].destroyed);
        assert_ne!(dead, sim.hash());
        sim.repair_station(0, f32::NAN);
        assert_eq!(sim.stations[0].health.cur, 30.0);
        let mut hashes = Vec::new();
        for _ in 0..100 {
            sim.step(&[fire(false)]);
            hashes.push(sim.hash());
        }
        assert!(!sim.stations[0].destroyed);
        hashes
    };
    assert_eq!(run(), run());
}

/// The cutter's click: a press its energy cannot pay, on either channel,
/// once, with or without a target; a press it can pay is silent.
#[test]
fn a_press_the_cutters_energy_cannot_pay_clicks_once() {
    let clicks = |sim: &Sim| {
        sim.events
            .iter()
            .filter(|e| matches!(e, Event::FireEmpty { .. }))
            .count()
    };
    for (bf2, secondary) in [(false, false), (false, true), (true, false), (true, true)] {
        let (mut sim, owner) = fixture(bf2, secondary);
        sim.step(&[fire(secondary)]);
        assert_eq!(clicks(&sim), 0, "paid press, bf2 {bf2}, ch1 {secondary}");
        sim.step(&[PadState::default()]);

        sim.unit_mut(owner).unwrap().soldier.energy = 0.0;
        let mut n = 0;
        for _ in 0..3 {
            sim.step(&[fire(secondary)]);
            n += clicks(&sim);
        }
        assert_eq!(n, 1, "unpaid press, bf2 {bf2}, ch1 {secondary}");

        sim.step(&[PadState::default()]);
        sim.unit_mut(owner).unwrap().is_player = false;
        sim.unit_mut(owner).unwrap().soldier.energy = 0.0;
        sim.step(&[fire(secondary)]);
        assert_eq!(clicks(&sim), 0, "a bot does not click");

        // The stamina cheat pays every pulse, so nothing is refused.
        let (mut sim, owner) = fixture(bf2, secondary);
        sim.cheats.infinite_stamina = true;
        sim.unit_mut(owner).unwrap().soldier.energy = 0.0;
        sim.step(&[fire(secondary)]);
        assert_eq!(clicks(&sim), 0, "cheat, bf2 {bf2}, ch1 {secondary}");
    }
}

#[test]
fn repair_pulses_spend_energy_against_an_idle_control() {
    for (bf2, secondary) in [(false, true), (true, false)] {
        let (mut firing, owner) = fixture(bf2, secondary);
        let (mut idle, control) = fixture(bf2, secondary);
        for (sim, who) in [(&mut firing, owner), (&mut idle, control)] {
            sim.stations[0].health.max = 1000.0;
            sim.stations[0].health.cur = 1.0;
            let u = sim.unit_mut(who).unwrap();
            u.soldier.energy = u.consts.energy_bar * 0.5;
        }
        let initial = idle.unit(control).unwrap().soldier.energy;
        // The fixture authors HeatPerShot=0.06. Do not read the runtime cost
        // back as the expected result: the idle twin isolates regeneration.
        let cost = 0.06 * idle.unit(control).unwrap().consts.energy_bar;
        let mut pulses = 0;
        for tick in 0..17 {
            firing.step(&[fire(secondary)]);
            idle.step(&[PadState::default()]);
            let count = firing
                .events
                .iter()
                .filter(|e| {
                    matches!(e,
                        Event::Repair { unit, target: Target::Station(0), .. } if *unit == owner
                    )
                })
                .count();
            assert_eq!(count, usize::from(tick == 0 || tick == 15), "tick {tick}");
            pulses += count;
            let actual = idle.unit(control).unwrap().soldier.energy
                - firing.unit(owner).unwrap().soldier.energy;
            assert!(
                (actual - cost * pulses as f32).abs() < 0.0001,
                "bf2={bf2} tick={tick}: spent {actual}, expected {}",
                cost * pulses as f32
            );
            assert!(!idle
                .events
                .iter()
                .any(|e| matches!(e, Event::Repair { .. })));
        }
        assert_eq!(pulses, 2);
        assert!(idle.unit(control).unwrap().soldier.energy > initial);
    }
}

/// A two-seat hover 2.5 m ahead of the cutter's owner (moved clear of the
/// fixture's droid), with `riders` of `team` aboard.
fn vehicle_scene(
    bf2: bool,
    secondary: bool,
    team: u8,
    riders: usize,
) -> (Sim, UnitHandle, VehicleHandle, Vec<UnitHandle>) {
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    let (mut sim, owner) = fixture(bf2, secondary);
    sim.unit_mut(owner).unwrap().soldier.pos = Vec3::new(20.0, 0.0, 2.5);
    sim.seat_setup = vec![Some(SeatSetup {
        radius: 1.5,
        body_active: vec![],
        boardable: vec![true; 2],
        exit_drop: 10.0,
        ai_size: 1,
        guns: Vec::new(),
    })];
    sim.slice_time = vec![2.0];
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(20.0, 0.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: crate::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.aim_sphere = Some((Vec3::Y, 1.0));
        v.health.cur = 400.0;
    }
    // Idle players as riders: a bot with no class data would get out.
    let riders = (0..riders)
        .map(|k| {
            let h = sim
                .spawn(&UnitSpec::player_in_slot(
                    k as u8 + 1,
                    team,
                    Vec3::new(30.0, 0.0, 0.0),
                ))
                .unwrap();
            assert!(sim.seat_unit(h, vh, k as u8));
            h
        })
        .collect();
    (sim, owner, vh, riders)
}

/// Issue #35: a player's cutter on an enemy vehicle with riders fills its
/// meter by ShotDelay a shot (not its health); past the class's slice time
/// every rider is put out beside it and only the slicer may board.
#[test]
fn a_slice_ejects_every_enemy_and_lets_the_slicer_in() {
    for (bf2, secondary) in [(false, true), (true, false)] {
        let (mut sim, owner, vh, riders) = vehicle_scene(bf2, secondary, 1, 2);
        let mut ejected = None;
        for tick in 0..90u64 {
            sim.step(&[fire(secondary)]);
            let v = sim.vehicle(vh).unwrap();
            assert_eq!(v.health.cur, 400.0, "slicing does not repair");
            if riders.iter().all(|&r| sim.unit(r).unwrap().mount.is_none()) {
                ejected = Some(tick);
                break;
            }
            // 0.5 s a shot: 0.5, 1.0, ... past 2.0 s on the fifth shot.
            let shots = tick / 15 + 1;
            assert_eq!(v.slice, 0.5 * shots as f32, "tick {tick}");
        }
        assert_eq!(ejected, Some(60), "bf2={bf2}");
        let v = sim.vehicle(vh).unwrap();
        assert_eq!(v.slice, 0.0);
        assert_eq!(v.team, None, "an empty vehicle goes neutral");
        assert_eq!(v.slice_hold.map(|(h, _)| h), Some(owner));
        let at = v.pos;
        for &r in &riders {
            let u = sim.unit(r).unwrap();
            assert!(u.alive);
            assert!(u.soldier.pos.distance(at) < 6.0, "beside the vehicle");
            // An ejected rider cannot take it back while the slicer's hold runs.
            assert!(!sim.enter_vehicle(r, vh, 0));
        }
        assert!(sim.enter_vehicle(owner, vh, 0), "the slicer boards");
    }
}

/// Issue #35 [ours]: for SLICE_RESERVE after a slice no bot may take a seat
/// (claim or entry); after it the vehicle is anyone's.
#[test]
fn a_bot_cannot_board_inside_the_slice_hold() {
    let (mut sim, _, vh, riders) = vehicle_scene(true, false, 1, 1);
    for _ in 0..61 {
        sim.step(&[fire(false)]);
    }
    assert!(sim.unit(riders[0]).unwrap().mount.is_none());
    let until = sim.vehicle(vh).unwrap().slice_hold.unwrap().1;
    assert_eq!(until, sim.tick - 1 + 90);
    let bot = sim
        .spawn(&UnitSpec {
            controller: crate::unit::Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(22.0, 0.0, 0.0))
        })
        .unwrap();
    while sim.tick < until {
        assert_eq!(sim.enter_seat(bot, vh), None);
        assert!(!sim.enter_vehicle(bot, vh, 0));
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.enter_seat(bot, vh), Some(0));
    assert!(sim.enter_vehicle(bot, vh, 0));
}

/// Issue #35: a class with no slice time (command walkers and hovers) is
/// never sliced, and a bot's cutter never takes an enemy vehicle.
#[test]
fn command_vehicles_and_bots_never_slice() {
    let (mut sim, owner, vh, riders) = vehicle_scene(true, false, 1, 2);
    let i = owner.index as usize;
    assert_eq!(sim.repair_target(i, 1), Some(Target::Vehicle(vh)));
    sim.units[i].controller = crate::unit::Controller::Ai;
    assert_eq!(sim.repair_target(i, 1), None, "a bot never slices");
    sim.units[i].controller = crate::unit::Controller::Player { slot: 0 };
    sim.slice_time = vec![0.0];
    assert_eq!(sim.repair_target(i, 1), None);
    for _ in 0..200 {
        sim.step(&[fire(false)]);
    }
    let v = sim.vehicle(vh).unwrap();
    assert_eq!((v.slice, v.health.cur), (0.0, 400.0));
    assert!(riders.iter().all(|&r| sim.unit(r).unwrap().mount.is_some()));
}

/// Issue #35 (a): a live friendly or empty vehicle gains the disc
/// VehicleHealth once per ShotDelay (BF2 +200, BF1 +60 per 0.5 s); an
/// enemy vehicle and one just out of range gain nothing.
#[test]
fn the_cutter_repairs_friendly_vehicles_at_the_disc_rate() {
    for (bf2, secondary) in [(false, true), (true, false)] {
        let step = if bf2 { 200.0 } else { 60.0 };
        for case in ["friendly", "empty", "enemy", "far"] {
            let (team, riders) = match case {
                "enemy" => (1, 1),
                "empty" => (0, 0),
                _ => (0, 1),
            };
            let (mut sim, _, vh, _) = vehicle_scene(bf2, secondary, team, riders);
            if case == "enemy" {
                // A class that cannot be sliced: the shot does nothing at all.
                sim.slice_time = vec![0.0];
            }
            if case == "far" {
                // Just past LockOnRange + the sphere's radius.
                let reach = if bf2 { 4.0 } else { 3.0 } + 1.0;
                sim.vehicle_mut(vh).unwrap().pos.z = 2.5 - reach - 0.3;
            }
            let mut rises = Vec::new();
            let mut before = 400.0;
            for tick in 0..31 {
                sim.step(&[fire(secondary)]);
                let now = sim.vehicle(vh).unwrap().health.cur;
                if now != before {
                    assert_eq!(now - before, step, "bf2={bf2} {case}");
                    rises.push(tick);
                    before = now;
                }
            }
            let expected: &[u64] = match case {
                "friendly" | "empty" => &[0, 15, 30],
                _ => &[],
            };
            assert_eq!(rises, expected, "bf2={bf2} {case}");
            assert_eq!(sim.vehicle(vh).unwrap().slice, 0.0);
        }
    }
}

#[test]
fn audit_paid_slice_pulses_emit_zero_healing_feedback() {
    for (bf2, secondary) in [(false, true), (true, false)] {
        let (mut sim, owner, vh, _) = vehicle_scene(bf2, secondary, 1, 1);
        let before = sim.unit(owner).unwrap().soldier.energy;
        sim.fire_repair(owner.index as usize, 1, Target::Vehicle(vh));
        assert_eq!(sim.vehicle(vh).unwrap().slice, 0.5);
        assert_eq!(sim.vehicle(vh).unwrap().health.cur, 400.0);
        assert!(sim.unit(owner).unwrap().soldier.energy < before);
        assert!(sim.events.iter().any(|e| matches!(e,
            Event::Repair { unit, target: Target::Vehicle(v), amount, .. }
            if *unit == owner && *v == vh && *amount == 0.0)));
        sim.events.clear();
        sim.unit_mut(owner).unwrap().soldier.energy = 0.0;
        sim.fire_repair(owner.index as usize, 1, Target::Vehicle(vh));
        assert_eq!(sim.vehicle(vh).unwrap().slice, 0.5);
        assert!(sim.events.is_empty(), "unpaid pulse emits no action");
    }
}

/// The acquisition's ray stops the target's radius short of its centre
/// (`re-aim-assist.md` 3.3): what lies inside that radius hides nothing, what
/// lies before it does.
#[test]
fn a_wall_inside_the_targets_radius_does_not_hide_it_and_one_before_it_does() {
    for (z, blocked) in [(0.6, false), (1.6, true)] {
        let (mut sim, _) = fixture(true, false);
        sim.stations[0].health.cur = 1.0;
        sim.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 1.0, z)),
            Shape::Box {
                half: Vec3::new(2.0, 2.0, 0.1),
            },
        ));
        sim.step(&[fire(false)]);
        assert_eq!(sim.stations[0].health.cur == 1.0, blocked, "wall at z {z}");
    }
}

/// A target whose centre is under the terrain, the top of its sphere above:
/// the terrain only blocks when it rises before the sphere.
#[test]
fn a_target_sunk_under_the_terrain_takes_the_cutter_from_above_it() {
    let (mut sim, _) = fixture(true, false);
    sim.stations[0].health.cur = 1.0;
    sim.stations[0].sphere_centre = Vec3::new(0.0, -0.5, 0.0);
    sim.stations[0].sphere_radius = 2.5;
    sim.step(&[fire(false)]);
    assert!(sim.stations[0].health.cur > 1.0, "sunk target taken");
}
