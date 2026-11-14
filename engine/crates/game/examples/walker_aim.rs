#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::{anyhow, Context, Result};
use glam::Vec3;
use render::{Camera, Renderer};
use std::path::PathBuf;
use walker_battle::battle;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(
        args.next()
            .context("walker_aim <out_dir> [class] [yaw_degrees] [pitch_degrees]")?,
    );
    let class = args.next().unwrap_or_else(|| "rep_walk_atte".into());
    let yaw = args.next().map_or(Ok(60.0f32), |s| s.parse())?.to_radians();
    let pitch = args.next().map_or(Ok(0.0f32), |s| s.parse())?.to_radians();
    std::fs::create_dir_all(&out)?;
    let root = game::assets::baked_root();
    let (mut walk, scene) = battle(&root)?;
    let index = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&class));
    let index = match index {
        Some(i) => i,
        None => {
            let classes = root.join("classes");
            let mut db = game::classdb::ClassDb::default();
            for path in [classes.join("bf2_imp.ron"), classes.join("bf2_rep.ron")] {
                db.load(&path)?;
            }
            let geometry = db
                .prop(&class, "GeometryName")
                .context("geometry")?
                .to_string();
            walk.vehicle_geometry.push(Some(geometry));
            walk.vehicle_classes.push(class.clone());
            walk.vehicle_classes.len() - 1
        }
    };
    let mut db = game::classdb::ClassDb::default();
    for faction in ["imp", "rep", "cis"] {
        db.load(&root.join(format!("classes/bf2_{faction}.ron")))?;
    }
    let seats = game::vehicle_class::seats(&db, &class);
    while walk.vehicle_seats.len() <= index {
        walk.vehicle_seats.push(Vec::new());
    }
    while walk.vehicle_riders.len() <= index {
        walk.vehicle_riders.push(Vec::new());
    }
    walk.vehicle_seats[index] = seats.clone();
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == index)
        .unwrap_or_else(|| {
            walk.sim
                .vehicles
                .iter()
                .position(|v| v.alive && v.kind == core::vehicle::VehicleKind::Walker)
                .unwrap()
        });
    let v = &mut walk.sim.vehicles[slot];
    v.class = index as u32;
    v.kind = core::vehicle::VehicleKind::Walker;
    v.guns = seats
        .iter()
        .map(|_| {
            Some(core::vehicle::gun::GunState {
                target: None,
                look: (0.0, 0.0),
                aim: (0.0, 0.0),
                rate: (0.0, 0.0),
                rel: (0.0, 0.0),
                weapons: Vec::new(),
                zoom: None,
            })
        })
        .collect();
    let pos = v.pos;
    if class == "rep_walk_atte" {
        let h = core::vehicle::VehicleHandle {
            index: slot as u32,
            gen: v.gen,
        };
        let player = walk.player;
        anyhow::ensure!(walk.sim.seat_unit(player, h, 1), "cannot board gunner");
    }
    for (i, other) in walk.sim.vehicles.iter_mut().enumerate() {
        other.alive = i == slot;
    }
    let mut r = Renderer::new_headless(960, 720).map_err(|e| anyhow!("renderer: {e}"))?;
    let gpu = game::viewer::upload_scene(&mut r, &scene)?;
    let mut vehicles =
        game::viewer::VehicleVisuals::load(&mut r, &scene, &root, &walk.vehicle_geometry);
    let mut soldiers = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
    let scale: f32 = std::env::var("WALKER_CAPTURE_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let eye = pos + Vec3::new(-22.0, 16.0, 30.0) * scale;
    let look = (pos + Vec3::Y * 5.0 - eye).normalize();
    let cam = Camera {
        roll: 0.0,
        position: eye,
        yaw: (-look.x).atan2(-look.z),
        pitch: look.y.asin(),
        fov_y: 50.0f32.to_radians(),
        near: 0.1,
        far: 2000.0,
    };
    for aimed in [false, true] {
        let v = &mut walk.sim.vehicles[slot];
        for gun in v.guns.iter_mut().flatten() {
            gun.look = if aimed { (yaw, pitch) } else { (0.0, 0.0) };
            gun.aim = gun.look;
        }
        if std::env::var_os("WALKER_WALK_ONLY").is_some() {
            v.guns.clear();
        }
        soldiers.update(&walk, core::TICK_DT);
        let skinned = soldiers.draws(&walk);
        println!(
            "aimed={aimed}, player_mount={:?}, riders={:?}, skinned={}",
            walk.sim.units[walk.player.index as usize].mount,
            soldiers.drawn(&walk),
            skinned.len()
        );
        let mut draws = gpu.view_draws(&walk.sim, walk.player);
        draws.extend(vehicles.draws(&mut r, &walk.sim, &[]));
        r.render_frame(&cam, &draws, &skinned, gpu.clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        std::fs::write(
            out.join(format!(
                "{class}_{}.png",
                if aimed { "aim" } else { "rest" }
            )),
            render::encode_png(&rgba, w, h)?,
        )?;
    }
    Ok(())
}
