use super::*;
use crate::sim::tests::{fire, flat_ground};
use crate::unit::{Controller, UnitSpec};
use glam::Vec3;

fn demo_log() -> InputLog {
    InputLog {
        seed: 0xC0FF_EE00,
        slots: 1,
        ticks: (0..300)
            .map(|i| PadState {
                lx: ((i * 37) % 65536 - 32768) as i16,
                ly: ((i * 91) % 65536 - 32768) as i16,
                buttons: if i % 7 < 3 { buttons::FIRE } else { 0 },
                ..Default::default()
            })
            .collect(),
    }
}

/// The pre-P9 logs (`game/tests/data/replays/`) carry no `slots`
/// field: they load as one slot and replay to exactly the hash the same
/// log with `slots: 1` gives.
#[test]
fn single_pad_log_without_slots_field_replays_identically() {
    #[derive(Serialize)]
    struct OldLog<'a> {
        seed: u32,
        ticks: &'a [PadState],
    }
    let log = demo_log();
    assert_eq!(log.slots, 1);
    let text = ron::to_string(&OldLog {
        seed: log.seed,
        ticks: &log.ticks,
    })
    .unwrap();
    assert!(!text.contains("slots"), "serialised without a slots field");
    let back: InputLog = ron::from_str(&text).unwrap();
    assert_eq!(back.slots, 1, "a missing slots field reads as one slot");
    assert_eq!(back.tick_count(), log.tick_count());
    assert_eq!(Sim::run_log(&back), Sim::run_log(&log));
}

/// Two occupied slots, one pad each: slot 0 walks forward (yaw 0 faces
/// −Z), slot 1 walks backward, and neither sees the other's stick.
#[test]
fn two_slots_drive_two_units_independently() {
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    let a = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let b = s
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
        .unwrap();
    assert_eq!(s.players, vec![Some(a), Some(b)], "one unit per slot");
    let fwd = PadState {
        ly: 32767,
        ..Default::default()
    };
    let back = PadState {
        ly: -32767,
        ..Default::default()
    };
    for _ in 0..60 {
        s.step(&[fwd, back]);
    }
    let (pa, pb) = (
        s.unit(a).unwrap().soldier.pos,
        s.unit(b).unwrap().soldier.pos,
    );
    assert!(pa.z < -5.0, "slot 0 walked forward: {pa}");
    assert!(pb.z > 2.0, "slot 1 walked backward: {pb}");
    // Swapping the pads swaps who goes which way.
    let mut s2 = Sim::empty(4);
    s2.heightfield = Some(flat_ground());
    let a2 = s2
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let b2 = s2
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
        .unwrap();
    for _ in 0..60 {
        s2.step(&[back, fwd]);
    }
    assert_eq!(s2.unit(a2).unwrap().soldier.pos.z, pb.z);
    assert_eq!(s2.unit(b2).unwrap().soldier.pos.z, pa.z);
}

/// A slot the pad slice does not reach idles — documented behaviour, not
/// an error (a splitscreen player who has not joined; a net peer whose
/// packet has not arrived).
#[test]
fn missing_pad_idles_the_slot() {
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    let a = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let b = s
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
        .unwrap();
    let parked = s.unit(b).unwrap().soldier.pos;
    let fwd = PadState {
        ly: 32767,
        ..Default::default()
    };
    // One pad, two slots: slot 1 gets PadState::default().
    for _ in 0..60 {
        s.step(&[fwd]);
    }
    assert!(s.unit(a).unwrap().soldier.pos.z < -5.0, "slot 0 walked");
    assert_eq!(s.unit(b).unwrap().soldier.pos, parked, "slot 1 never moved");
    // An EMPTY slice idles every slot: the sim settles and then holds.
    for _ in 0..60 {
        s.step(&[]);
    }
    let held = s.unit(a).unwrap().soldier.pos;
    for _ in 0..30 {
        s.step(&[]);
    }
    assert_eq!(s.unit(a).unwrap().soldier.pos, held, "no pads: idle");
}

#[test]
fn different_seed_different_hash() {
    let mut a = demo_log();
    let h1 = Sim::run_log(&a);
    a.seed ^= 1;
    assert_ne!(h1, Sim::run_log(&a));
}

#[test]
fn input_log_ron_round_trip() {
    let log = demo_log();
    let s = ron::to_string(&log).unwrap();
    let back: InputLog = ron::from_str(&s).unwrap();
    assert_eq!(Sim::run_log(&log), Sim::run_log(&back));
}

/// A player at the origin facing −Z (the rifle: LockOnRange 64,
/// ExtremeRange 120, LockOnAngle 1°) and the given soldiers, spawned in
/// order after it.
fn range(others: &[(u8, Vec3)]) -> (Sim, UnitHandle, Vec<UnitHandle>) {
    let mut s = Sim::empty(5);
    s.heightfield = Some(flat_ground());
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    // A level aim: the spawn pitch's ray is in the floor 33 m out.
    s.units[p.index as usize].soldier.pitch = 0.0;
    let o = others
        .iter()
        .map(|&(team, pos)| s.spawn(&UnitSpec::trooper(team, pos)).unwrap())
        .collect();
    s.sync_slot_tables();
    (s, p, o)
}

/// The PS2 filters the magnet by team (`re-aim-assist.md` §3.3): a
/// friend square on the crosshair is the reticle's pick (rel 1), never
/// the magnet; an enemy there is both.
#[test]
fn a_friend_on_the_crosshair_is_picked_but_never_the_magnet() {
    let (mut s, p, o) = range(&[(0, Vec3::new(0.0, 0.0, -20.0))]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], None);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), 1)));
    let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), -1)));
}

/// It is a cone, not a ray: an enemy 1 m off the axis at 20 m is inside
/// r + t·slope (1.42 + 0.35) and becomes the magnet.
#[test]
fn an_enemy_just_off_the_ray_is_the_magnet() {
    let (mut s, p, o) = range(&[(1, Vec3::new(1.0, 0.0, -20.0))]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
    // Well outside the cone: nothing.
    let (mut s, p, _) = range(&[(1, Vec3::new(3.0, 0.0, -20.0))]);
    s.acquire_magnet(0, p, false);
    assert_eq!((s.magnet[0], s.reticle_pick[0]), (None, None));
}

/// Past LockOnRange + r but inside ExtremeRange the enemy is picked
/// (the reticle turns red) but is not the magnet.
#[test]
fn past_lock_on_range_is_picked_but_not_the_magnet() {
    let r = (crate::unit::AIM_BOX_MAX - crate::unit::AIM_BOX_MIN).length() * 0.5;
    let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -(64.0 + r + 1.0)))]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], None);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), -1)));
    // Past ExtremeRange + r: not even picked.
    let (mut s, p, _) = range(&[(1, Vec3::new(0.0, 0.0, -(120.0 + r + 1.0)))]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], None);
}

/// OURS (sniper aim assist): the disc's sniper (LockOnRange 0) takes no
/// magnet; with the loader's override it takes an enemy at 50 m and at
/// 120 m, scoped and not, and none past its 128 m reach.
/// measure: controller sniper-assist acquisition and pull at staged distances before replacing
/// the deliberate shared sniper-assist reach.
#[test]
fn a_sniper_rifle_takes_a_magnet_out_to_its_max_range() {
    let r = (crate::unit::AIM_BOX_MAX - crate::unit::AIM_BOX_MIN).length() * 0.5;
    let disc = crate::weapon::tests::disc_sniper(0.0);
    let mut ours = disc.clone();
    ours.give_sniper_assist();
    for (class, dist, want) in [
        (&disc, 50.0, false),
        (&ours, 50.0, true),
        (&ours, 120.0, true),
        (&ours, 128.0 + r + 1.0, false),
    ] {
        for scoped in [false, true] {
            let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -dist))]);
            let w = s.unit(p).unwrap().weapon_class;
            s.weapon_classes[w] = class.clone();
            if scoped {
                s.units[p.index as usize].zoom = Some(crate::unit::Zoom {
                    step: 1,
                    value: 8.0,
                    divisor: 8.0,
                    scope: true,
                });
                assert!(s.unit(p).unwrap().scoped());
            }
            s.acquire_magnet(0, p, false);
            assert_eq!(
                s.magnet[0],
                want.then_some(AimTarget::Unit(o[0])),
                "range {} at {dist} m scoped {scoped}",
                class.lock_on_range
            );
        }
    }
}

#[test]
fn a_sniper_aims_at_the_visible_soldier_even_after_switching_from_a_slow_gun() {
    let (mut sim, player, targets) = range(&[(1, Vec3::new(0.0, 0.0, -60.0))]);
    let enemy = targets[0];
    let velocity = Vec3::X * 8.0;
    sim.unit_mut(enemy).unwrap().soldier.vel = velocity;
    let magnet = AimTarget::Unit(enemy);
    sim.assist_lead[0] = Some((
        magnet,
        Lead {
            vel: velocity,
            offset: Vec3::X * 4.0,
        },
    ));
    let weapon = sim.unit(player).unwrap().weapon_class;
    let player_unit = sim.unit_mut(player).unwrap();
    player_unit.zoom = Some(crate::unit::Zoom {
        step: 1,
        value: 8.0,
        divisor: 8.0,
        scope: true,
    });
    let beam = crate::weapon::BeamClass {
        range: 500.0,
        life: 0.15,
        pierce: true,
        impact: None,
    };
    for (speed, beam) in [(2000.0, None), (150.0, Some(beam))] {
        let mut class = crate::weapon::tests::disc_sniper(0.0);
        class.ordnance.velocity = speed;
        class.ordnance.beam = beam;
        sim.weapon_classes[weapon] = class;
        let (_, _, lead) = sim
            .assisted(0, player, magnet, &PadState::default(), crate::TICK_DT)
            .unwrap();
        let (target, lead) = lead.unwrap();
        assert_eq!(target, magnet);
        assert_eq!(lead.offset, velocity * -crate::unit::HUMAN_SHOT_LAG);
    }
}

/// The held magnet's cone widens by π/4 while FIRE is held: an enemy
/// that steps 3 m off the axis at 20 m is kept while firing and dropped
/// when not.
#[test]
fn the_sticky_magnet_holds_a_quarter_turn_wider_while_firing() {
    for (firing, kept) in [(true, true), (false, false)] {
        let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
        s.unit_mut(o[0]).unwrap().soldier.pos.x = 3.0;
        s.acquire_magnet(0, p, firing);
        assert_eq!(s.magnet[0].is_some(), kept, "firing {firing}");
    }
}

/// Two enemies side by side, the second scoring a little better than
/// the held first: still, or the stick pushed away from the second, the
/// first is kept; the stick pushed toward the second (left) or a mouse
/// gives it to the second. A far nearer enemy on the aim's line does not
/// take a held magnet either.
#[test]
fn the_magnet_holds_its_target_unless_the_player_steers_to_another() {
    let a = (1, Vec3::new(0.3, 0.0, -20.0));
    let b = (1, Vec3::new(-0.6, 0.0, -20.0));
    let stick = |x: f32| Steer::Stick(glam::Vec2::new(x, 0.0));
    for (steer, want) in [
        (stick(0.0), 0),
        (stick(0.1), 0),
        (stick(0.6), 0),
        (stick(-0.6), 1),
        (Steer::Mouse, 1),
    ] {
        let (mut s, p, o) = range(&[a, b]);
        s.magnet[0] = Some(AimTarget::Unit(o[0]));
        // Aim a little toward the second.
        s.units[p.index as usize].soldier.yaw = 0.01;
        s.acquire_magnet_steering(0, p, false, steer);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[want])), "{steer:?}");
    }
    // On the aim's line (the eye 1.3 m over a body's middle): the held
    // one at 20 m, the newcomer at 6 m.
    let (mut s, p, o) = range(&[
        (1, Vec3::new(0.3, 0.0, -20.0)),
        (1, Vec3::new(0.0, 0.0, -6.0)),
    ]);
    for &h in &o {
        s.unit_mut(h).unwrap().soldier.pos.y = 1.3;
    }
    s.magnet[0] = Some(AimTarget::Unit(o[0]));
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])), "a nearer enemy");
}

/// The maintainer's report (2026-10-29): aiming at one bot 20 m out,
/// another walks across the aim's line in front of it or behind it. The
/// player holds the stick still, or nudges it up or down (fine aim, not
/// toward the passer). The magnet stays on the first all the way; on
/// the old rule a nudge gave it to the passer 3 m in front, and a still
/// stick to one 11 m in front.
#[test]
fn a_bot_walking_past_the_held_one_does_not_take_the_magnet() {
    let up = Steer::Stick(glam::Vec2::new(0.0, 0.3));
    let still = Steer::Stick(glam::Vec2::ZERO);
    for (z, steer) in [(-17.0, up), (-23.0, up), (-9.0, still), (-17.0, still)] {
        let (mut s, p, o) = range(&[
            (1, Vec3::new(0.0, 0.0, -20.0)),
            (1, Vec3::new(-4.0, 0.0, z)),
        ]);
        // Their middles on the aim's line (the eye 1.3 m over a body's).
        for &h in &o {
            s.unit_mut(h).unwrap().soldier.pos.y = 1.3;
        }
        s.acquire_magnet(0, p, true);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
        for _ in 0..60 {
            s.unit_mut(o[1]).unwrap().soldier.pos.x += 8.0 / 60.0;
            s.acquire_magnet_steering(0, p, true, steer);
            assert_eq!(
                s.magnet[0],
                Some(AimTarget::Unit(o[0])),
                "passer at z {z}, x {:.2}, {steer:?}",
                s.unit(o[1]).unwrap().soldier.pos.x
            );
        }
    }
}

/// The held magnet dies: the enemy beside it takes the magnet.
#[test]
fn a_held_magnet_that_dies_hands_over() {
    let (mut s, p, o) = range(&[
        (1, Vec3::new(0.3, 0.0, -20.0)),
        (1, Vec3::new(-0.8, 0.0, -20.0)),
    ]);
    s.magnet[0] = Some(AimTarget::Unit(o[0]));
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
    s.unit_killed(o[0], None);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[1])));
}

/// Out of sight for up to `MAGNET_LOS_GRACE` (7 ticks) the held magnet
/// is kept; longer, it is let go.
#[test]
fn a_held_magnet_out_of_sight_is_kept_briefly() {
    use crate::collision::{Body, CollisionWorld, Shape};
    let wall = || {
        Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        )
    };
    for (hidden, kept) in [(7, true), (8, false)] {
        let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
        s.collision.push_body(wall());
        for _ in 0..hidden {
            s.acquire_magnet(0, p, false);
        }
        assert_eq!(s.magnet[0].is_some(), kept, "{hidden} ticks hidden");
        // Back in sight: held again only if it was kept.
        s.collision = CollisionWorld::new(Vec::new(), Vec::new());
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet_hidden[0], 0);
    }
}

/// The flicker in play: two enemies 1.5 m apart 20 m out walking across
/// the view at 4 m/s, the player holding FIRE and not touching the aim
/// stick for 4 s, the pull on. On the PS2's rule the magnet changes
/// hands 17 times; it is held.
#[test]
fn two_enemies_side_by_side_do_not_trade_the_magnet() {
    let (mut s, _, o) = range(&[
        (1, Vec3::new(0.5, 0.0, -20.0)),
        (1, Vec3::new(-1.0, 0.0, -20.0)),
    ]);
    for &h in &o {
        s.unit_mut(h).unwrap().health.cur = 1.0e9;
        s.unit_mut(h).unwrap().health.max = 1.0e9;
    }
    // The pull on (the SP Auto Aim option): it is what turns the aim.
    s.assist_armed = true;
    let pad = PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    };
    let (mut last, mut changes) = (None, 0);
    for _ in 0..120 {
        // Walking side by side (placed: a bot's own walk would turn).
        for &h in &o {
            let u = s.unit_mut(h).unwrap();
            u.soldier.pos.x += 4.0 / 30.0;
            u.soldier.pos.z = -20.0;
        }
        s.step(&[pad]);
        let m = s.magnet[0];
        if m.is_some() && last.is_some() && m != last {
            changes += 1;
        }
        last = m.or(last);
    }
    assert!(last.is_some(), "a magnet");
    assert!(changes <= 1, "the magnet changed hands {changes} times");
}

/// The tournament is order-dependent (§3.3): with nothing held, a
/// better-scoring friend found FIRST takes the pick and the enemy behind
/// cannot beat it — no magnet; found after a held enemy, the friend is
/// skipped. The port's order is the unit table's.
#[test]
fn a_friend_found_first_shadows_the_enemy() {
    let friend = (0, Vec3::new(0.0, 0.0, -10.0));
    let enemy = (1, Vec3::new(0.5, 0.0, -20.0));
    let (mut s, p, o) = range(&[friend, enemy]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], None);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), 1)));
    let (mut s, p, o) = range(&[enemy, friend]);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
}

/// A vehicle 20 m down the crosshair, its blast sphere 2 m about a
/// point 1 m up; `kind` Hover or a building Turret (no pilot seat).
fn put_vehicle(s: &mut Sim, kind: crate::vehicle::VehicleKind) -> VehicleHandle {
    use crate::vehicle::{BlastSetup, SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let turret = kind == VehicleKind::Turret;
    let v = s.spawn_vehicle(&VehicleSpec {
        kind,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 0.0, -20.0),
        yaw: 0.0,
        max_health: 500.0,
        health_type: if turret {
            HealthType::Building
        } else {
            HealthType::Vehicle
        },
        seats: vec![if turret {
            SeatKind::Gunner(0)
        } else {
            SeatKind::Pilot
        }],
    });
    s.blast_setup = vec![Some(BlastSetup {
        explosion: None,
        sphere_centre: Vec3::new(0.0, 1.0, 0.0),
        sphere_radius: 2.0,
    })];
    v
}

/// Card HTGT (`re-target-hud.md` §2): a vehicle on the crosshair is the
/// reticle's pick, team 0 while empty (relation 0) and an enemy once
/// the other team holds it; it is the magnet only for a weapon whose
/// TargetVehicle bit is set and that may take its relation.
#[test]
fn a_vehicle_on_the_crosshair_is_picked_and_magnets_by_the_weapons_bits() {
    use crate::vehicle::VehicleKind;
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, VehicleKind::Hover);
    let w = s.unit(p).unwrap().weapon_class;
    s.weapon_classes[w].target_team = 1;
    s.weapon_classes[w].target_kind = 0x1f;
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
    assert_eq!(
        s.magnet[0], None,
        "an enemy-only weapon: team 0 is no magnet"
    );
    s.vehicles[v.index as usize].team = Some(1);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), -1)));
    assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)));
    // BF1's rifles: TargetVehicle 0 - picked, never the magnet.
    s.weapon_classes[w].target_kind = 0x07;
    s.magnet[0] = None;
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), -1)));
    assert_eq!(s.magnet[0], None);
    // Its own team's: a friend.
    s.vehicles[v.index as usize].team = Some(0);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 1)));
}

/// A soldier in front of a tank scores lower and takes the pick; a
/// destroyed vehicle is no candidate.
#[test]
fn a_soldier_before_a_vehicle_wins_and_a_wreck_is_no_target() {
    use crate::vehicle::VehicleKind;
    let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -10.0))]);
    let v = put_vehicle(&mut s, VehicleKind::Hover);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), -1)));
    s.units[o[0].index as usize].alive = false;
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
    s.vehicles[v.index as usize].alive = false;
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], None);
}

/// A building turret is picked through its own building's bodies (its
/// centre sits inside them); another building in the way still blocks.
#[test]
fn a_turret_is_picked_through_its_own_building() {
    use crate::collision::{Body, Shape};
    use crate::vehicle::VehicleKind;
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, VehicleKind::Turret);
    // Its building's bounds win over a blast sphere of radius 0.
    s.blast_setup[0].as_mut().unwrap().sphere_radius = 0.0;
    s.vehicles[v.index as usize].aim_sphere = Some((Vec3::new(0.0, 1.0, 0.0), 3.0));
    s.vehicles[v.index as usize].instance = Some(7);
    let mut shell = Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 1.0, -20.0)),
        Shape::Box {
            half: Vec3::splat(3.0),
        },
    );
    shell.instance = Some(7);
    s.collision.push_body(shell);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
    s.vehicles[v.index as usize].instance = Some(8);
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], None, "another building's shell blocks");
}

/// The shooter's own vehicle is never its pick.
#[test]
fn the_shooters_own_vehicle_is_not_picked() {
    use crate::vehicle::VehicleKind;
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, VehicleKind::Hover);
    assert!(s.seat_unit(p, v, 0));
    s.acquire_magnet(0, p, false);
    assert_eq!(s.reticle_pick[0], None);
}

/// Acceptance 3 of card HTGT: the readout reads the picked vehicle's
/// health every tick, so shots into its hull move it while the pick
/// holds - no hit event in between.
#[test]
fn shots_into_a_picked_vehicle_drop_its_health_while_it_stays_picked() {
    use crate::collision::{HullPart, Layers, Shape};
    use crate::vehicle::VehicleKind;
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, VehicleKind::Hover);
    s.vehicles[v.index as usize].team = Some(1);
    s.hull_parts = vec![Some(
        vec![HullPart::new(
            glam::Mat4::from_translation(Vec3::Y),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 3.0),
            },
            Layers::ALL,
        )]
        .into(),
    )];
    s.sync_hulls();
    // The test rifle's bolt does nothing to a vehicle (scale 0, as
    // BF1's): give it a launcher's full vehicle scale.
    let w = s.unit(p).unwrap().weapon_class;
    s.weapon_classes[w].ordnance.scales[crate::weapon::HealthType::Vehicle as usize] = 1.0;
    let hull = &mut s.vehicles[v.index as usize].health;
    (hull.max, hull.cur) = (5000.0, 5000.0);
    let max = hull.max;
    let mut seen = Vec::new();
    for _ in 0..90 {
        s.step(&[fire()]);
        assert_eq!(
            s.reticle_pick[0].map(|(t, _)| t),
            Some(AimTarget::Vehicle(v))
        );
        seen.push(s.vehicles[v.index as usize].health.cur);
    }
    let last = *seen.last().unwrap();
    assert!(last < max, "hull {last} of {max}");
    assert!(seen.windows(2).all(|w| w[1] <= w[0]), "never rises");
    eprintln!("hull after 90 ticks of fire: {last} of {max}");
}

/// The assist pulls toward a vehicle magnet as toward a soldier.
#[test]
fn aim_assist_pulls_toward_a_vehicle_target() {
    use crate::vehicle::VehicleKind;
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, VehicleKind::Hover);
    s.vehicles[v.index as usize].pos.x = 0.8;
    s.vehicles[v.index as usize].team = Some(1);
    let w = s.unit(p).unwrap().weapon_class;
    s.weapon_classes[w].target_kind = 0x1f;
    for _ in 0..30 {
        s.step(&[fire()]);
    }
    assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)));
    assert!(
        s.unit(p).unwrap().soldier.yaw < -0.005,
        "yaw {}",
        s.unit(p).unwrap().soldier.yaw
    );
}

#[test]
fn aim_assist_leaves_a_mouse_aimed_pad_alone() {
    use crate::vehicle::VehicleKind;
    let run = |mouse: bool| {
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, VehicleKind::Hover);
        s.vehicles[v.index as usize].pos.x = 0.8;
        s.vehicles[v.index as usize].team = Some(1);
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].target_kind = 0x1f;
        let mut pad = fire();
        if mouse {
            pad.buttons |= buttons::MOUSE_AIM;
        }
        for _ in 0..30 {
            s.step(&[pad]);
        }
        assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)));
        s.unit(p).unwrap().soldier.yaw
    };
    assert!(run(false) < -0.005);
    assert_eq!(run(true), 0.0);
}

/// HTGTW: an off-pivot centre turns with the turret, rather than orbiting
/// the unturned location. The radius is too small to cover both rays.
#[test]
fn htgtw_turned_turret_uses_the_rotated_sphere() {
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, crate::vehicle::VehicleKind::Turret);
    let turret = &mut s.vehicles[v.index as usize];
    turret.pos = Vec3::new(4.0, 0.0, -16.0);
    turret.rot = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    turret.aim_sphere = Some((Vec3::new(4.0, 1.0, 0.0), 0.25));
    let candidate = s.aim_candidate(AimTarget::Vehicle(v)).unwrap();
    // +X becomes -Z: (4,0,-16) + (0,1,-4).
    assert!(candidate.ctr.distance(Vec3::new(4.0, 1.0, -20.0)) < 1e-5);
    let eye = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views()).0;
    let aim = |s: &mut Sim, point: Vec3| {
        let d = point - eye;
        let player = &mut s.units[p.index as usize];
        player.soldier.yaw = crate::fmath::atan2(-d.x, -d.z);
        player.soldier.pitch = crate::fmath::atan2(d.y, (d.x * d.x + d.z * d.z).sqrt());
        s.acquire_magnet(0, p, false);
    };
    aim(&mut s, Vec3::new(4.0, 1.0, -20.0));
    assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
    aim(&mut s, Vec3::new(8.0, 1.0, -16.0));
    assert_eq!(s.reticle_pick[0], None);
}

#[test]
fn htgtw_vehicle_only_weapon_excludes_building_turrets() {
    use crate::vehicle::VehicleKind;
    for (kind, vehicle_magnet) in [(VehicleKind::Hover, true), (VehicleKind::Turret, false)] {
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, kind);
        s.vehicles[v.index as usize].team = Some(1);
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].target_kind = 1 << 3;
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), -1)));
        assert_eq!(
            s.magnet[0],
            vehicle_magnet.then_some(AimTarget::Vehicle(v)),
            "{kind:?}"
        );
        // Invert the weapon's bits, retaining the same geometry and target.
        s.magnet[0] = None;
        s.weapon_classes[w].target_kind = 1 << 4;
        s.acquire_magnet(0, p, false);
        assert_eq!(
            s.magnet[0],
            (!vehicle_magnet).then_some(AimTarget::Vehicle(v)),
            "{kind:?}"
        );
    }
}

#[test]
fn htgtw_crossing_vehicle_assist_leads_in_its_travel_direction() {
    let (mut s, p, _) = range(&[]);
    let v = put_vehicle(&mut s, crate::vehicle::VehicleKind::Hover);
    // Centre exactly on the player's ray: stationary fire has no yaw
    // pull. With no movement stick, tracking cannot supply the lead.
    let (eye, fwd) = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views());
    s.vehicles[v.index as usize].aim_sphere = Some((Vec3::ZERO, 0.5));
    s.vehicles[v.index as usize].pos = eye + fwd * 20.0;
    let w = s.unit(p).unwrap().weapon_class;
    s.weapon_classes[w].ordnance.velocity = 40.0;
    let mut outputs = Vec::new();
    for speed in [0.0, 4.0, -4.0] {
        s.vehicles[v.index as usize].vel = Vec3::X * speed;
        s.assist[0] = Default::default();
        s.assist_lead[0] = None;
        let pad = s.apply_assist(0, p, AimTarget::Vehicle(v), &fire(), 1.0 / 30.0);
        outputs.push(pad.rx);
    }
    assert_eq!(outputs[0], 0);
    assert!(outputs[1] > 100, "rightward lead: {outputs:?}");
    assert!(outputs[2] < -100, "leftward lead: {outputs:?}");
    assert_eq!(outputs[1], -outputs[2]);
}

/// An enemy behind a wall fails the LOS ray and is not picked.
#[test]
fn an_enemy_behind_a_wall_is_not_picked() {
    use crate::collision::{Body, Shape};
    let (mut s, p, _) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
    s.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    s.acquire_magnet(0, p, false);
    assert_eq!((s.magnet[0], s.reticle_pick[0]), (None, None));
}

/// FUN_00241b98's two rejections and its score.
#[test]
fn cone_score_matches_the_ps2_formula() {
    let (pos, dir) = (Vec3::ZERO, Vec3::NEG_Z);
    assert_eq!(
        cone_score(1.0, 0.0, 10.0, pos, dir, Vec3::new(0.0, 0.0, 1.0)),
        -1.0
    );
    assert_eq!(
        cone_score(1.0, 0.0, 10.0, pos, dir, Vec3::new(0.0, 0.0, -11.5)),
        -1.0
    );
    assert_eq!(
        cone_score(1.0, 0.0, 10.0, pos, dir, Vec3::new(1.5, 0.0, -5.0)),
        -1.0
    );
    assert_eq!(
        cone_score(1.0, 0.1, 10.0, pos, dir, Vec3::new(1.5, 0.0, -5.0)),
        5.0 + 16.0 * 2.25
    );
}

/// Aim assist in the world: neutral stick + fire, target slightly right
/// → yaw pulled negative (positive yaw = left).
#[test]
fn aim_assist_pulls_toward_the_unit_target() {
    let mut s = Sim::empty(11);
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    s.spawn(&UnitSpec::trooper(1, Vec3::new(0.4, 0.0, -20.0)))
        .unwrap();
    for _ in 0..30 {
        s.step(&[fire()]);
    }
    assert!(s.magnet[0].is_some(), "magnet acquired in the cone");
    assert!(
        s.unit(p).unwrap().soldier.yaw < -0.005,
        "yaw {}",
        s.unit(p).unwrap().soldier.yaw
    );
}

/// What [`strafe`] measured.
struct Strafe {
    hits: u32,
    shots: u32,
    /// The player's yaw after each tick.
    yaws: Vec<f32>,
    /// Ticks the reticle's pick (the HUD's colour and readout) was not
    /// the runner.
    unpicked: u32,
    /// The runner's speed.
    run: f32,
}

/// A runner `dist` m ahead crossing at his class's run speed and turning
/// about every `period` ticks, around the player's line; the player's
/// rifle firing a round of `speed` and `life`. The player holds his
/// sticks still, aimed at the runner's centre at the start, and taps
/// FIRE every 10 ticks for `ticks`, the magnet already acquired (a press
/// with no magnet arms nothing). `lead`: the assist's lead on or off
/// (off = the PS2's assist).
fn strafe(lead: bool, dist: f32, speed: f32, life: f32, period: u32, ticks: u32) -> Strafe {
    let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -dist))]);
    s.assist_lead_on = lead;
    let w = s.unit(p).unwrap().weapon_class;
    s.weapon_classes[w].ordnance.velocity = speed;
    s.weapon_classes[w].ordnance.life_span = life;
    let e = o[0];
    let run = s.unit(e).unwrap().consts.max_speed;
    let u = &mut s.units[e.index as usize];
    u.soldier.pos.x = -run * period as f32 * crate::TICK_DT * 0.5;
    u.controller = Controller::Pacer {
        vel: Vec3::X * run,
        period,
    };
    u.health.cur = 1.0e9;
    let (eye, _) = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views());
    let c = s.unit(e).unwrap().capsule().center() - eye;
    let me = &mut s.units[p.index as usize].soldier;
    me.yaw = crate::fmath::atan2(-c.x, -c.z);
    me.pitch = crate::fmath::atan2(c.y, (c.x * c.x + c.z * c.z).sqrt());
    s.acquire_magnet(0, p, false);
    let mut out = Strafe {
        hits: 0,
        shots: 0,
        yaws: Vec::new(),
        unpicked: 0,
        run,
    };
    for k in 0..ticks {
        let pad = if k % 10 == 0 {
            fire()
        } else {
            PadState::default()
        };
        s.step(&[pad]);
        for ev in &s.events {
            match *ev {
                crate::sim::Event::Shot { unit, .. } if unit == p => out.shots += 1,
                crate::sim::Event::Hit { victim, .. } if victim == e => out.hits += 1,
                _ => {}
            }
        }
        s.units[e.index as usize].health.cur = 1.0e9;
        out.yaws.push(s.unit(p).unwrap().soldier.yaw);
        if s.reticle_pick[0].map(|(t, _)| t) != Some(AimTarget::Unit(e)) {
            out.unpicked += 1;
        }
    }
    out
}

/// The ticks the reticle's turn changes sense (a wobble), counting only
/// turns over 0.1 mrad.
fn wobbles(yaws: &[f32]) -> usize {
    let d: Vec<f32> = yaws
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| d.abs() > 1e-4)
        .collect();
    d.windows(2).filter(|w| w[0] * w[1] < 0.0).count()
}

/// The largest turn of the reticle in one tick (rad).
fn largest_step(yaws: &[f32]) -> f32 {
    yaws.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

/// The lead (ours, playtest 2026-10-28): a player holding his sticks
/// still on a runner crossing at run speed, tapping FIRE, hits far more
/// often than under the PS2's assist - a rifle's 80 m/s round and a
/// sniper's 2000 m/s one, at 30 and 60 m. (The disc snipers ship
/// LockOnRange 0; the loader's `WeaponClass::give_sniper_assist` gives
/// them a 128 m, 1° magnet; the 2000 m/s round here is the rifle's
/// magnet with a sniper's speed.) The runner
/// turns about every 5 s.
#[test]
fn the_lead_hits_a_crossing_runner_under_a_still_stick() {
    for (name, speed, life, dist, floor) in [
        ("rifle", 80.0, 1.5, 30.0, 0.6),
        ("rifle", 80.0, 1.5, 60.0, 0.4),
        ("sniper", 2000.0, 0.15, 30.0, 0.9),
        ("sniper", 2000.0, 0.15, 60.0, 0.8),
    ] {
        let ps2 = strafe(false, dist, speed, life, 150, 600);
        let ours = strafe(true, dist, speed, life, 150, 600);
        println!(
            "{name} at {dist} m, runner {} m/s: PS2 assist {}/{}, with the lead {}/{}",
            ours.run, ps2.hits, ps2.shots, ours.hits, ours.shots
        );
        assert_eq!(ps2.shots, ours.shots, "{name} {dist}");
        let share = ours.hits as f32 / ours.shots as f32;
        assert!(share >= floor, "{name} at {dist} m: {share}");
        // Twice the PS2's and ten more, or every shot.
        assert!(
            ours.hits >= (ps2.hits * 2 + 10).min(ours.shots),
            "{name} at {dist} m: {} vs {}",
            ours.hits,
            ps2.hits
        );
        assert_eq!(ours.unpicked, 0, "{name} {dist}: the HUD's pick held");
    }
}

/// No jump when the runner turns about (every 1.5 s here): the reticle
/// never turns further in a tick than the runner's own swing across
/// that tick (his speed both ways) and the lead's easing
/// ([`aim_assist::LEAD_MAX_RATE`]) take it, and it changes the sense
/// of its turn only when he does (no wobble). The HUD's pick (the
/// reticle's colour and the target readout) stays the runner
/// throughout.
#[test]
fn the_reticle_does_not_jump_when_the_runner_turns_about() {
    const PERIOD: u32 = 45;
    const TICKS: u32 = 600;
    for (speed, life, dist) in [(80.0, 1.5, 30.0), (80.0, 1.5, 60.0), (2000.0, 0.15, 30.0)] {
        let ours = strafe(true, dist, speed, life, PERIOD, TICKS);
        let bound = (2.0 * ours.run + aim_assist::LEAD_MAX_RATE) / dist * crate::TICK_DT;
        let step = largest_step(&ours.yaws);
        let turns = (TICKS / PERIOD) as usize;
        println!(
            "{speed} m/s round at {dist} m: largest step {step:.4} rad of {bound:.4}, {} wobbles for {turns} turns",
            wobbles(&ours.yaws)
        );
        assert!(step <= bound, "{speed} at {dist} m: {step} > {bound}");
        assert!(wobbles(&ours.yaws) <= turns + 1, "{speed} at {dist} m");
        assert_eq!(ours.unpicked, 0, "{speed} at {dist} m: the HUD's pick held");
    }
}
