//! The seat rider and the camera frame by frame at a display rate, as the
//! window draws them (ticks stepped by the accumulator, the pose blended
//! between the last two ticks), while the seat turns and stops:
//!
//!   seat_jitter [mission] [class] [seat] [hz]
//!
//! Prints, per frame, the tick fraction, the seat's look yaw, the rider's
//! root and a bone in the camera's frame. A smooth turn moves them by even
//! steps; a jitter shows as steps that jump between frames.
#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::{anyhow, Context, Result};
use core::sim::PadState;
use glam::Vec3;
use render::Renderer;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mission = args.next().unwrap_or_else(|| "bf2_kas2_con_cw".into());
    let map = mission.splitn(3, '_').take(2).collect::<Vec<_>>().join("_");
    let class = args
        .next()
        .unwrap_or_else(|| "rep_hover_fightertank".into());
    let seat: u8 = args.next().map_or(Ok(1), |s| s.parse())?;
    let hz: f32 = args.next().map_or(Ok(144.0), |s| s.parse())?;
    let root = game::assets::baked_root();
    let (mut walk, scene) = walker_battle::battle_on(&root, &map, &mission)?;
    let index = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&class))
        .with_context(|| format!("{class} not on {map}"))?;
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == index)
        .context("no live vehicle of the class")?;
    let h = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    let player = walk.player;
    anyhow::ensure!(walk.sim.seat_unit(player, h, seat), "cannot board");
    for _ in 0..30 {
        walk.step_tick(&[PadState::default()]);
    }
    let mut r = Renderer::new_headless(320, 180).map_err(|e| anyhow!("renderer: {e}"))?;
    let mut soldiers = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
    walk.blend.on = true;
    let dt = 1.0 / hz;
    let frames = (hz * 3.0) as usize;
    let mut last: Option<(Vec3, Vec3, f32)> = None;
    for k in 0..frames {
        let rx = if k < frames / 2 { 32767 } else { 0 };
        let pad = PadState {
            rx,
            ..Default::default()
        };
        let ticks = walk.advance_with(dt, &[pad], |_| {});
        game::render_blend::blend_in_looking(&mut walk, &pad);
        soldiers.update(&walk, dt);
        let cam = walk.camera();
        let view = cam.view();
        let me = walk.sim.unit(player).map(|u| u.soldier.pos);
        let draw = soldiers
            .draws(&walk)
            .into_iter()
            .min_by(|a, b| {
                let d = |m: &glam::Mat4| m.w_axis.truncate().distance(cam.position);
                d(&a.model).total_cmp(&d(&b.model))
            })
            .context("no rider drawn")?;
        let root_c = view.transform_point3(draw.model.w_axis.truncate());
        let bone = draw.palette.len() / 2;
        let bone_c = view.transform_point3((draw.model * draw.palette[bone]).w_axis.truncate());
        let look = walk.sim.vehicles[slot]
            .guns
            .get(seat as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |g| g.look.0);
        let (dr, db, dl) = last.map_or((Vec3::ZERO, Vec3::ZERO, 0.0), |(r0, b0, l0)| {
            (root_c - r0, bone_c - b0, look - l0)
        });
        println!(
            "{k:4} t{ticks} f{:.2} look {:7.4} dlook {:7.4} root {:6.3} {:6.3} {:6.3} droot {:7.4} {:7.4} dbone {:7.4} {:7.4} {me:?}",
            game::render_blend::fraction(&walk),
            look,
            dl,
            root_c.x,
            root_c.y,
            root_c.z,
            dr.x,
            dr.z,
            db.x,
            db.z,
        );
        last = Some((root_c, bone_c, look));
        walk.blend_out();
    }
    Ok(())
}
