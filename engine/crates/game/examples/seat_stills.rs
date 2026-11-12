//! Headless stills of a vehicle seat's camera on a real map (the IFT-X on BF2
//! Kashyyyk by default, against the PS2's slot 8 / 9 states in
//! `analysis/re-hover-camera.md`):
//!
//!   seat_stills <out_dir> [mission] [class] [seat]
//!
//! (mission `bf2_kas2_con_cw` by default; the map is its first two words).
//!
//! The player boards seat `seat` of the first live vehicle of `class`, then
//! holds the right stick still, up and down for a second each, then fires
//! level for a third of a second. Frames:
//! `<out_dir>/<class>_s<seat>_{level,up,down,fire}.png`. Each prints the camera's
//! place in the vehicle's frame, its pitch against the vehicle's, the FOV and
//! the seat's look.
#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::{anyhow, Context, Result};
use core::sim::PadState;
use glam::Vec3;
use render::Renderer;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(
        args.next()
            .context("seat_stills <out_dir> [mission] [class] [seat]")?,
    );
    let mission = args.next().unwrap_or_else(|| "bf2_kas2_con_cw".into());
    let map = mission.splitn(3, '_').take(2).collect::<Vec<_>>().join("_");
    let class = args
        .next()
        .unwrap_or_else(|| "rep_hover_fightertank".into());
    let seat: u8 = args.next().map_or(Ok(0), |s| s.parse())?;
    std::fs::create_dir_all(&out)?;
    let root = game::assets::baked_root();
    let (mut walk, scene) = walker_battle::battle_on(&root, &map, &mission)?;
    let index = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&class))
        .with_context(|| format!("{class} not on {map}: {:?}", walk.vehicle_classes))?;
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
    let mut r = Renderer::new_headless(960, 540).map_err(|e| anyhow!("renderer: {e}"))?;
    let gpu = game::viewer::upload_scene(&mut r, &scene)?;
    let mut vehicles =
        game::viewer::VehicleVisuals::load(&mut r, &scene, &root, &walk.vehicle_geometry);
    let mut soldiers = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
    let mut bolts = game::bolt_vis::BoltVis::new(&root, &scene.manifest.source_game);
    for (name, ry, fire) in [
        ("level", 0i16, false),
        ("up", 32767, false),
        ("down", -32767, false),
        ("fire", 0, true),
    ] {
        let pad = PadState {
            ry,
            ..Default::default()
        };
        for _ in 0..30 {
            walk.step_tick(&[pad]);
            soldiers.update(&walk, core::TICK_DT);
        }
        let release = PadState {
            buttons: if fire { core::sim::buttons::FIRE } else { 0 },
            ..Default::default()
        };
        for _ in 0..if fire { 10 } else { 4 } {
            walk.step_tick(&[release]);
            soldiers.update(&walk, core::TICK_DT);
        }
        let shots = bolts.draws(&mut r, &walk);
        println!("{name}: {} bolt draws", shots.len());
        r.set_bolts(shots);
        let cam = walk.camera();
        let v = &walk.sim.vehicles[slot];
        let local = v.rot.inverse() * (cam.position - v.pos);
        let fwd = v.rot * Vec3::NEG_Z;
        let body_pitch = (v.rot * Vec3::Z).y.asin();
        let look = v.guns.get(seat as usize).and_then(Option::as_ref);
        println!(
            "{name}: camera {local:.3?} in the vehicle frame, pitch {:.2} deg ({:.2} against the body), fov_y {:.2} deg, look {:?} aim {:?}, forward {fwd:.2?}",
            cam.pitch.to_degrees(),
            (cam.pitch - body_pitch).to_degrees(),
            cam.fov_y.to_degrees(),
            look.map(|g| (g.look.0.to_degrees(), g.look.1.to_degrees())),
            look.map(|g| (g.aim.0.to_degrees(), g.aim.1.to_degrees())),
        );
        let skinned = soldiers.draws(&walk);
        let mut draws = gpu.view_draws(&walk.sim, walk.player);
        draws.extend(soldiers.weapon_draws(&walk));
        draws.extend(vehicles.draws(&mut r, &walk.sim, &[]));
        r.render_frame(&cam, &draws, &skinned, gpu.clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        let (rgba, w, hgt) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        std::fs::write(
            out.join(format!("{class}_s{seat}_{name}.png")),
            render::encode_png(&rgba, w, hgt)?,
        )?;
    }
    Ok(())
}
