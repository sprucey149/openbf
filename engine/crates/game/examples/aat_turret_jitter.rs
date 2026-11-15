//! A hover's drawn turret and aimer nodes frame by frame at a display rate,
//! as the window draws them, while its seat turns and stops:
//!
//!   aat_turret_jitter [mission] [class] [seat] [hz] [stick|mouse]
//!
//! Prints, per frame, the tick count, the blend fraction and each moving
//! node's world yaw and pitch with their step from the last frame. A smooth
//! turn steps every node by even amounts; a jitter shows as a node whose
//! steps bunch up on the frames that step a tick.
#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::{Context, Result};
use core::sim::PadState;
use glam::{Mat4, Vec3};

fn yaw_pitch(m: &Mat4) -> (f32, f32) {
    let f = m.z_axis.truncate().normalize_or_zero();
    (
        f.x.atan2(f.z).to_degrees(),
        f.y.clamp(-1.0, 1.0).asin().to_degrees(),
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mission = args.next().unwrap_or_else(|| "bf1_rhn1_con_cw".into());
    let map = mission.splitn(3, '_').take(2).collect::<Vec<_>>().join("_");
    let class = args.next().unwrap_or_else(|| "cis_hover_aat".into());
    let seat: u8 = args.next().map_or(Ok(0), |s| s.parse())?;
    let hz: f32 = args.next().map_or(Ok(144.0), |s| s.parse())?;
    let mode = args.next().unwrap_or_default();
    let mouse = mode.starts_with("mouse");
    let pitch = mode.ends_with("pitch");
    let ragged = mode.contains("ragged");
    let spin = mode.contains("spin");
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
    for (i, sp) in walk.vehicle_seats[index].iter().enumerate() {
        println!(
            "seat {i}: {:?} aimers {:?} aim_tension {:?} move_tension {:?}",
            sp.kind,
            sp.aimers
                .iter()
                .map(|a| a.node.as_str())
                .collect::<Vec<_>>(),
            sp.aim_tension,
            sp.move_tension
        );
    }
    anyhow::ensure!(walk.sim.seat_unit(player, h, seat), "cannot board");
    if std::env::var_os("AAT_BOT_PILOT").is_some() && seat != 0 {
        let free = |walk: &game::walk::WalkSim| {
            (0..walk.sim.units.len()).find(|&i| {
                let u = &walk.sim.units[i];
                u.alive && u.mount.is_none() && u.controller == core::unit::Controller::Ai
            })
        };
        for _ in 0..900 {
            if free(&walk).is_some() {
                break;
            }
            walk.step_tick(&[PadState::default()]);
        }
        let bot = free(&walk).context("no free bot")?;
        let bh = core::unit::UnitHandle {
            index: bot as u32,
            gen: walk.sim.units[bot].gen,
        };
        anyhow::ensure!(walk.sim.seat_unit(bh, h, 0), "bot cannot board");
        println!("bot {bot} drives");
    }
    for _ in 0..30 {
        walk.step_tick(&[PadState::default()]);
    }
    let rig = game::viewer::hover_rigs(&scene.manifest, &root, &walk.vehicle_geometry)
        .get(index)
        .cloned()
        .flatten()
        .context("no hover rig")?;
    let bind = rig.hover_nodes(&walk.sim.vehicles[slot], None);
    for (y, pp) in [(0.0f32, 0.0f32), (1.0, 0.0), (-1.0, 1.0)] {
        let n = rig.hover_nodes(
            &walk.sim.vehicles[slot],
            Some(game::viewer::turret_pose::HoverNine { p: pp, y }.pose()),
        );
        let moved: Vec<&str> = (0..n.len())
            .filter(|&k| !n[k].abs_diff_eq(bind[k], 1e-4))
            .map(|k| rig.skeleton.names[k].as_str())
            .collect();
        println!("nine y{y} p{pp}: moves {moved:?}");
    }
    walk.blend.on = true;
    let dt = 1.0 / hz;
    let frames = (hz * 3.0) as usize;
    let mut pending = 0i32;
    let mut last: Option<Vec<(f32, f32)>> = None;
    let mut moving: Vec<usize> = Vec::new();
    let mut last_cam: Option<(f32, Vec3)> = None;
    for k in 0..frames {
        let turning = k < frames / 2;
        let fdt = if ragged {
            dt * [0.6f32, 1.4, 1.0, 0.8, 1.2][k % 5]
        } else {
            dt
        };
        let per_frame = if turning {
            (fdt * 86.4f32.to_radians() / core::sim::LOOK_UNIT) as i32
        } else {
            0
        };
        let pad = if mouse {
            pending += per_frame;
            if pitch {
                PadState {
                    my: pending,
                    buttons: core::sim::buttons::MOUSE_AIM,
                    ..Default::default()
                }
            } else {
                PadState {
                    mx: pending,
                    buttons: core::sim::buttons::MOUSE_AIM,
                    ..Default::default()
                }
            }
        } else if pitch {
            PadState {
                ry: if turning { 32767 } else { 0 },
                ..Default::default()
            }
        } else {
            PadState {
                rx: if turning { 32767 } else { 0 },
                ..Default::default()
            }
        };
        let ticks = walk.advance_with(fdt, &[pad], |w| {
            if spin {
                let v = &mut w.sim.vehicles[slot];
                v.rot = glam::Quat::from_rotation_y(0.5f32.to_radians()) * v.rot;
                v.pos += v.rot * Vec3::Z * 0.2;
            }
        });
        if ticks > 0 && mouse {
            pending = 0;
        }
        let shown = if mouse {
            if pitch {
                PadState {
                    my: pending,
                    buttons: core::sim::buttons::MOUSE_AIM,
                    ..Default::default()
                }
            } else {
                PadState {
                    mx: pending,
                    buttons: core::sim::buttons::MOUSE_AIM,
                    ..Default::default()
                }
            }
        } else {
            pad
        };
        game::render_blend::blend_in_looking(&mut walk, &shown);
        let v = &walk.sim.vehicles[slot];
        let model = Mat4::from_rotation_translation(v.rot, v.pos);
        let nodes = rig.hover_nodes(v, None);
        if moving.is_empty() && k > 4 {
            moving = (0..nodes.len())
                .filter(|&n| !nodes[n].abs_diff_eq(bind[n], 1e-4))
                .collect();
        }
        let now: Vec<(f32, f32)> = moving
            .iter()
            .map(|&n| yaw_pitch(&(model * nodes[n])))
            .collect();
        let g = v.guns.get(seat as usize).and_then(Option::as_ref);
        let cam = walk.camera();
        let cy = cam.yaw.to_degrees();
        let direct = game::walk::vehicle_camera(
            v,
            walk.vehicle_seats[index].get(seat as usize),
            g.map(|g| g.look),
            cam.fov_y,
            Vec3::ZERO,
        )
        .1;
        let rot_yaw = {
            let f = v.rot * Vec3::Z;
            f.x.atan2(f.z).to_degrees()
        };
        let d_cam = last_cam.map_or((0.0, Vec3::ZERO), |(y, p): (f32, Vec3)| {
            (cy - y, cam.position - p)
        });
        last_cam = Some((cy, cam.position));
        let mut line = format!(
            "{k:4} t{ticks} f{:.2} direct {:8.3} hull {:8.3} cam {:8.3} dc{:6.3} dpos {:6.3},{:6.3},{:6.3} look {:7.3} aim {:7.3} tgt {}",
            game::render_blend::fraction(&walk),
            direct.yaw.to_degrees(),
            rot_yaw,
            cy,
            d_cam.0,
            d_cam.1.x,
            d_cam.1.y,
            d_cam.1.z,
            g.map_or(0.0, |g| g.look.0.to_degrees()),
            g.map_or(0.0, |g| g.aim.0.to_degrees()),
            g.and_then(|g| g.target).map_or("-".into(), |t: Vec3| format!("{:.1},{:.1},{:.1}", t.x, t.y, t.z)),
        );
        if let Some(prev) = &last {
            for (i, &n) in moving.iter().enumerate() {
                if i < prev.len() {
                    line += &format!(
                        " | {} {:7.2} d{:6.3} p{:6.2} dp{:6.3}",
                        rig.skeleton.names[n],
                        now[i].0,
                        now[i].0 - prev[i].0,
                        now[i].1,
                        now[i].1 - prev[i].1
                    );
                }
            }
        }
        println!("{line}");
        last = Some(now);
        walk.blend_out();
    }
    Ok(())
}
