//! Playtest 2026-11-14, BF1 Rhen Var Harbor, the AAT's gunner seat with the
//! mouse at 74 fps: the turret "doesn't move smoothly like with the
//! controller" (black box: seat 1, mouse aim, 74 fps).
//!
//! A flyer or hover gunner's camera is a chase state stepped once per tick
//! (`WalkSim::next_cam_gunner`) and drawn blended between ticks. A stick's
//! look moves by ticks, so the blend is exact; a mouse's moves with the
//! frames, and a tick takes in all the motion of the frames before it (two
//! or three at 74 fps), so the blended camera turned 0.96 deg a frame
//! through one tick and 1.44 through the next under a turret turning an even
//! 1.17 (`examples/aat_turret_jitter.rs`). The camera now turns with the
//! frame's previewed look on the aim frame, as the drawn turret does
//! (`render_blend::Blend::preview`).

use super::seat_camera::{map, missions_of};
use core::sim::{buttons, PadState};

/// Each frame's camera yaw turn (deg) while the mouse turns the AAT gunner's
/// look at an even rate, `hz` frames a second.
fn camera_turns(hz: f32) -> Option<Vec<f32>> {
    let root = game::assets::baked_root();
    let dir = root.join("bf1_rhn1");
    let mission = missions_of("bf1_rhn1")
        .into_iter()
        .find(|m| m.factions().iter().any(|f| f == "cis"))?;
    let mut m = map(&dir, &mission.factions())?;
    let class = m.build.classes.iter().position(|c| c == "cis_hover_aat")?;
    for _ in 0..30 {
        m.walk.step_tick(&[PadState::default()]);
    }
    let h = m
        .walk
        .sim
        .alive_vehicles()
        .find(|(_, v)| v.class as usize == class)
        .map(|(h, _)| h)?;
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 1), "the AAT's gunner seat");
    for _ in 0..30 {
        m.walk.step_tick(&[PadState::default()]);
    }
    m.walk.blend.on = true;
    let dt = 1.0 / hz;
    let per_frame = (dt * 86.4f32.to_radians() / core::sim::LOOK_UNIT) as i32;
    let mouse = |mx: i32| PadState {
        mx,
        buttons: buttons::MOUSE_AIM,
        ..Default::default()
    };
    let mut pending = 0;
    let mut last = None;
    let mut turns = Vec::new();
    for k in 0..(hz as usize) {
        pending += per_frame;
        if m.walk.advance_with(dt, &[mouse(pending)], |_| {}) > 0 {
            pending = 0;
        }
        game::render_blend::blend_in_looking(&mut m.walk, &mouse(pending));
        let yaw = m.walk.camera().yaw.to_degrees();
        m.walk.blend_out();
        if let Some(y) = last.filter(|_| k > hz as usize / 4) {
            turns.push(y - yaw);
        }
        last = Some(yaw);
    }
    Some(turns)
}

#[test]
fn the_aat_gunners_camera_turns_evenly_with_the_mouse() {
    for hz in [74.0, 144.0] {
        let Some(turns) = camera_turns(hz) else {
            eprintln!("SKIP: bf1_rhn1 is not baked");
            return;
        };
        let mean = turns.iter().sum::<f32>() / turns.len() as f32;
        let worst = turns
            .iter()
            .map(|t| (t / mean - 1.0).abs())
            .fold(0.0, f32::max);
        println!(
            "{hz} Hz: {mean:.3} deg a frame, worst off by {:.1}%",
            worst * 100.0
        );
        assert!(
            worst < 0.03,
            "{hz} Hz: the camera's turn strays {:.1}% from its mean {mean:.3} deg a frame: {turns:?}",
            worst * 100.0
        );
    }
}
