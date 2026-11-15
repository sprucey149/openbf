//! Consecutive headless frames of a seat's view, one tick apart, to catch a
//! vehicle part that flickers while the camera barely moves:
//!
//!   aat_flicker <out_dir> [mission] [class] [seat] [frames]
//!
//! Writes `<out_dir>/fNN.png` at 1920x1080. `AAT_FLICKER_STILL` holds the
//! sim still between frames; `AAT_FLICKER_NO_SHADOW` drops the BF1 vehicle
//! shadow volumes. Playtest 2026-11-14: the BF1 AAT's side guns flickered
//! dark from the driver's seat on Rhen Var Harbor; with the shadows off the
//! frames were steady (`render::shadow`).
#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::{anyhow, Context, Result};
use core::sim::PadState;
use render::Renderer;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(
        args.next()
            .context("aat_flicker <out_dir> [mission] [class] [seat] [frames]")?,
    );
    let mission = args.next().unwrap_or_else(|| "bf1_rhn1_con_cw".into());
    let map = mission.splitn(3, '_').take(2).collect::<Vec<_>>().join("_");
    let class = args.next().unwrap_or_else(|| "cis_hover_aat".into());
    let seat: u8 = args.next().map_or(Ok(0), |s| s.parse())?;
    let frames: usize = args.next().map_or(Ok(8), |s| s.parse())?;
    std::fs::create_dir_all(&out)?;
    let root = game::assets::baked_root();
    let (mut walk, scene) = walker_battle::battle_on(&root, &map, &mission)?;
    let index = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&class))
        .context("class")?;
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == index)
        .context("no live vehicle")?;
    let h = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    let player = walk.player;
    anyhow::ensure!(walk.sim.seat_unit(player, h, seat), "cannot board");
    for _ in 0..60 {
        walk.step_tick(&[PadState::default()]);
    }
    let mut r = Renderer::new_headless(1920, 1080).map_err(|e| anyhow!("renderer: {e}"))?;
    let gpu = game::viewer::upload_scene(&mut r, &scene)?;
    let mut vehicles =
        game::viewer::VehicleVisuals::load(&mut r, &scene, &root, &walk.vehicle_geometry);
    let skinned = Vec::new();
    for k in 0..frames {
        if k > 0 && std::env::var_os("AAT_FLICKER_STILL").is_none() {
            walk.step_tick(&[PadState::default()]);
        }
        let cam = walk.camera();
        let mut draws = gpu.view_draws(&walk.sim, walk.player);
        draws.extend(vehicles.draws(&mut r, &walk.sim, &[]));
        if std::env::var_os("AAT_FLICKER_NO_SHADOW").is_some() {
            r.set_shadow_triangles(&[]);
        }
        r.render_frame(&cam, &draws, &skinned, gpu.clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        let (rgba, w, hgt) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        std::fs::write(
            out.join(format!("f{k:02}.png")),
            render::encode_png(&rgba, w, hgt)?,
        )?;
        let v = &walk.sim.vehicles[slot];
        println!(
            "{k}: cam {:.4?} yaw {:.4} pos {:.4?}",
            cam.position, cam.yaw, v.pos
        );
    }
    Ok(())
}
