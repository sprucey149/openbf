//! Playtest C3: a bot walking somewhere faces its walk unless it can fight
//! what it looks at (`think`'s aim).

use super::*;
use crate::ai::arsenal::tests::{heavy_scene, manned, rifle};
use crate::cp::CpDef;
use crate::region::Region;
use crate::sim::Event;
use crate::unit::UnitSpec;
use crate::vehicle::VehicleKind;
use crate::TICK_HZ;

/// A rifleman (a rifle in both slots) at the origin with the L graph and an
/// enemy post at hub 2: he walks -Z to hub 1, then +X.
fn walk_scene() -> (Sim, UnitHandle) {
    let (mut sim, bot, _) = heavy_scene(rifle(), rifle());
    sim.nav = Some(crate::ai::tests::l_graph());
    let post = Vec3::new(40.0, 0.0, -40.0);
    sim.add_cp(
        CpDef {
            name: "cp1".into(),
            pos: post,
            capture: Region::sphere(post, 6.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    (sim, bot)
}

/// An enemy soldier that stands where he is and does not die.
fn dummy(sim: &mut Sim, pos: Vec3) -> UnitHandle {
    let e = sim.spawn(&UnitSpec::trooper(1, pos)).expect("enemy");
    sim.units[e.index as usize].health.max = 1e9;
    sim.units[e.index as usize].health.cur = 1e9;
    e
}

/// What `bot` did over `secs` seconds: the worst angle (degrees) between
/// its facing and its walk while it moved faster than 1 m/s (after a
/// second to turn), and the shots it fired.
fn walk_for(sim: &mut Sim, bot: UnitHandle, secs: u32) -> (f32, u32) {
    let (mut worst, mut shots, mut moving) = (0.0f32, 0u32, 0u32);
    for i in 0..secs * TICK_HZ {
        sim.step(&[]);
        shots += sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == bot))
            .count() as u32;
        let u = sim.unit(bot).expect("alive");
        let v = Vec2::new(u.soldier.vel.x, u.soldier.vel.z);
        if i >= TICK_HZ && v.length() > 1.0 {
            moving += 1;
            let fwd = Vec2::new(-u.soldier.yaw.sin(), -u.soldier.yaw.cos());
            let cos = fwd.dot(v.normalize()).clamp(-1.0, 1.0);
            worst = worst.max(cos.acos().to_degrees());
        }
    }
    assert!(moving > TICK_HZ, "the bot walked ({moving} ticks)");
    (worst, shots)
}

/// A walker no weapon of the rifleman's hurts, 29 m off his walk: he keeps
/// it as his target but walks on facing his walk, and fires nothing. The
/// control: its pilot gets out, and the rifleman turns to him and fires.
#[test]
fn a_walking_bot_faces_its_walk_past_a_vehicle_it_cannot_hurt() {
    let _stand = crate::ai::tests::all_stand();
    let (mut sim, bot) = walk_scene();
    let (_, rider) = manned(&mut sim, VehicleKind::Walker, Vec3::new(15.0, 1.5, -25.0));
    sim.step(&[]);
    let u = sim.unit(bot).expect("alive");
    assert_eq!(u.ai.target, Some(rider.into()), "the walker is seen");
    assert!(u.ai.arm.spare, "and nothing he holds hurts it");
    let (worst, shots) = walk_for(&mut sim, bot, 3);
    assert!(worst < 25.0, "faces the walk: worst {worst:.0} deg");
    assert_eq!(shots, 0);

    // The control: the pilot on foot is fought.
    let (mut sim, bot) = walk_scene();
    let (_, rider) = manned(&mut sim, VehicleKind::Walker, Vec3::new(15.0, 1.5, -25.0));
    sim.unseat_unit(rider);
    let mut shots = 0;
    for _ in 0..4 * TICK_HZ {
        sim.step(&[]);
        shots += sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == bot))
            .count();
    }
    assert!(shots >= 3, "the pilot on foot is shot at: {shots}");
}

/// A told record behind a walking bot (a friend facing back sees an enemy
/// 19 m behind him): the bot does not walk its leg backwards to look at it.
#[test]
fn a_walking_bot_faces_its_walk_past_a_told_record() {
    let (mut sim, bot) = walk_scene();
    let friend = sim
        .spawn(&UnitSpec {
            controller: crate::unit::Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 6.0))
        })
        .expect("friend");
    sim.units[friend.index as usize].soldier.yaw = std::f32::consts::PI;
    let foe = dummy(&mut sim, Vec3::new(0.0, 0.0, 25.0));
    let mut told = false;
    for _ in 0..TICK_HZ {
        sim.step(&[]);
        let st = &sim.unit(bot).expect("alive").ai;
        told |= crate::perception::contact_of(st, foe).is_some_and(|c| c.told)
            && st.target == Some(foe.into());
    }
    assert!(told, "the friend's sighting is the bot's target");
    let (worst, _) = walk_for(&mut sim, bot, 3);
    assert!(worst < 25.0, "faces the walk: worst {worst:.0} deg");
}

/// One think of a fresh bot in `sim`: the pad and the state.
fn one_think(sim: &Sim, bot: UnitHandle) -> (PadState, AiState) {
    let mut st = AiState::default();
    let (pad, _) = super::think(sim, bot, &mut st);
    (pad, st)
}

/// The look stick of the scene's bot with nobody about: its travelling look.
fn travel_rx(space: bool) -> i16 {
    let (mut sim, bot) = walk_scene();
    if space {
        sim.space_assault = Some(crate::sim::SpaceAssault::new(10));
    }
    let (pad, st) = one_think(&sim, bot);
    assert!(st.target.is_none() && st.route.is_some());
    pad.rx
}

/// A soldier seen beyond the reach of every weapon the bot holds (an 8 / 16 m
/// rifle: 26 m) is its target but gets the travelling look; inside the reach
/// (a 64 m rifle) he is aimed at.
#[test]
fn a_walking_bot_faces_its_walk_past_an_enemy_beyond_its_reach() {
    let scene = |far: f32| {
        let (mut sim, bot) = walk_scene();
        for w in &mut sim.weapon_classes {
            w.ai_range = [0.0, far * 0.5, far];
        }
        let e = dummy(&mut sim, Vec3::new(15.0, 0.0, -30.0));
        let (pad, st) = one_think(&sim, bot);
        assert_eq!(st.target, Some(e.into()), "seen");
        (pad, st)
    };
    let (pad, st) = scene(16.0);
    assert_eq!(st.aim_dir, Vec3::ZERO, "the travelling look");
    assert_eq!(pad.rx, travel_rx(false), "no turn off its walk: {pad:?}");
    assert_ne!(pad.ly, 0, "walks on: {pad:?}");
    let (pad, st) = scene(64.0);
    assert_ne!(st.aim_dir, Vec3::ZERO, "aimed at");
    assert_ne!(pad.rx, travel_rx(false), "turns to him: {pad:?}");
}

/// Space rounds keep the aim they had: the same walker is looked at by a
/// bot walking its leg in a Space Assault round.
#[test]
fn a_space_rounds_walking_bot_still_looks_at_the_vehicle() {
    let scene = |space: bool| {
        let (mut sim, bot) = walk_scene();
        if space {
            sim.space_assault = Some(crate::sim::SpaceAssault::new(10));
        }
        let (_, rider) = manned(&mut sim, VehicleKind::Walker, Vec3::new(15.0, 1.5, -25.0));
        let (pad, st) = one_think(&sim, bot);
        assert_eq!(st.target, Some(rider.into()));
        assert!(st.arm.spare);
        assert!(st.route.is_some(), "walking its leg");
        (pad, st)
    };
    let (pad, st) = scene(false);
    assert_eq!(
        (st.aim_dir, pad.rx),
        (Vec3::ZERO, travel_rx(false)),
        "ground"
    );
    let (pad, st) = scene(true);
    assert_ne!(st.aim_dir, Vec3::ZERO, "space: aimed at");
    assert_ne!(pad.rx, travel_rx(true), "space: turns to it: {pad:?}");
}
