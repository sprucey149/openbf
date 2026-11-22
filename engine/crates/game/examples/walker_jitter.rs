//! A human walker pilot's camera frame by frame at a display rate, as the
//! window draws it (ticks stepped by the accumulator, the pose blended
//! between the last two ticks, the look preview drawn in), while the walker
//! idles then walks straight ahead:
//!
//!   walker_jitter [mission] [class] [hz] [--mouse] [--ticks] [--quiet]
//!     [--mx=<deg/s>] [--rx=<-1..1>] [--lx=<-1..1>]
//!
//! (defaults `bf2_end1_con imp_walk_atst_jungle 60`). `--mouse` drives with the
//! mouse-aim flag held (keyboard + mouse), `--ticks` draws ticks only (no
//! blend). Prints per frame the camera's yaw and its step, the camera's
//! sideways step (along its own right), and the drawn walker's highest node
//! in the camera's frame; then a summary over the walk: the RMS of the yaw's
//! and the sideways position's second difference (a smooth sway is small,
//! a frame-to-frame jitter large) and how often the yaw step flips sign.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim
#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::Result;
use core::sim::{buttons, PadState};
use glam::{Mat4, Vec3};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags: Vec<&str> = args
        .iter()
        .filter(|a| a.starts_with("--"))
        .map(String::as_str)
        .collect();
    let mut pos = args.iter().filter(|a| !a.starts_with("--"));
    let mission = pos.next().cloned().unwrap_or_else(|| "bf2_end1_con".into());
    let class = pos
        .next()
        .cloned()
        .unwrap_or_else(|| "imp_walk_atst_jungle".into());
    let hz: f32 = pos.next().map_or(Ok(60.0), |s| s.parse())?;
    let mouse = flags.contains(&"--mouse");
    let quiet = flags.contains(&"--quiet");
    // `--mx=<deg/s>` turns the mouse steadily (implies `--mouse`),
    // `--rx=<-1..1>` / `--lx=<-1..1>` hold the aim / steer stick.
    let num = |name: &str| {
        flags
            .iter()
            .find_map(|f| f.strip_prefix(name))
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let (mx_rate, rx, lx) = (num("--mx="), num("--rx="), num("--lx="));
    let mouse = mouse || mx_rate != 0.0;
    let mut pending = 0.0f64;
    let root = game::assets::baked_root();
    let (mut walk, _scene, h) = walker_battle::board_vehicle(&root, &mission, &class, 0)?;
    let slot = h.index as usize;
    for _ in 0..30 {
        walk.step_tick(&[PadState::default()]);
    }
    walk.blend.on = !flags.contains(&"--ticks");
    let class_ix = walk.sim.vehicles[slot].class as usize;
    let rig = walk.walker_rigs.get(class_ix).cloned().flatten();
    let scale = walk
        .sim
        .walker_params
        .get(class_ix)
        .and_then(Option::as_ref)
        .map_or(1.0, |p| p.root_scale);
    // The drawn model's highest node at rest: the cockpit / deck.
    let head = rig.as_ref().and_then(|r| {
        (0..r.skeleton.len()).max_by(|&a, &b| {
            let y = |k: usize| r.skeleton.global_bind[k].w_axis.y;
            y(a).total_cmp(&y(b))
        })
    });
    let dt = 1.0 / hz;
    let idle = (hz * 1.0) as usize;
    let frames = (hz * 7.0) as usize;
    let mut rows: Vec<(f32, Vec3, Vec3, Vec3)> = Vec::new();
    for k in 0..frames {
        let walking = k >= idle;
        let stick = |f: f32| (f * f32::from(i16::MAX)) as i16;
        if walking {
            pending += f64::from(mx_rate.to_radians() * dt);
        }
        let mut pad = PadState {
            ly: if walking { i16::MAX } else { 0 },
            rx: if walking { stick(rx) } else { 0 },
            lx: if walking { stick(lx) } else { 0 },
            mx: PadState::quantize_look(pending),
            buttons: if mouse { buttons::MOUSE_AIM } else { 0 },
            ..Default::default()
        };
        let ticks = walk.advance_with(dt, &[pad], |_| {});
        if ticks > 0 {
            pending -= f64::from(pad.look().0);
            pad.mx = PadState::quantize_look(pending);
        }
        game::render_blend::blend_in_looking(&mut walk, &pad);
        let cam = walk.camera();
        let v = &walk.sim.vehicles[slot];
        let drawn = core::vehicle::walker::legs_rot(v);
        let yaw_of = |q: glam::Quat| {
            let f = q * Vec3::Z;
            core::fmath::atan2(f.x, f.z).to_degrees()
        };
        let head_c = match (&rig, head) {
            (Some(r), Some(n)) => {
                // Posed: the entity matrix is the body's, bone 0 the legs'.
                let m = Mat4::from_rotation_translation(v.rot, v.pos) * r.nodes(v, scale)[n];
                cam.view().transform_point3(m.w_axis.truncate())
            }
            _ => Vec3::ZERO,
        };
        let row = (cam.yaw.to_degrees(), cam.position, cam.right(), head_c);
        if !quiet {
            let (dyaw, side) = rows
                .last()
                .map_or((0.0, 0.0), |p| (wrap(row.0 - p.0), (row.1 - p.1).dot(p.2)));
            println!(
                "{k:4} t{ticks} f{:.2} cam_yaw {:9.4} dyaw {:8.4} side {:8.4} body {:9.4} legs {:9.4} aim {:8.4} h {:8.4} anim {} {:.3} head_c {:7.3} {:7.3}",
                game::render_blend::fraction(&walk),
                row.0,
                dyaw,
                side,
                yaw_of(v.rot),
                yaw_of(drawn),
                v.walker.aim_yaw.to_degrees(),
                v.walker.leg_angle().to_degrees(),
                v.walker.anim,
                v.walker.clip_time,
                head_c.x,
                head_c.y,
            );
        }
        if walking {
            rows.push(row);
        }
        walk.blend_out();
    }
    // Skip the first second of the walk (the start-up step).
    let w = &rows[(hz as usize).min(rows.len())..];
    let yaw: Vec<f32> = w.iter().map(|r| r.0).collect();
    let side: Vec<f32> = w
        .windows(2)
        .map(|p| (p[1].1 - p[0].1).dot(p[0].2))
        .collect();
    let headx: Vec<f32> = w.iter().map(|r| r.3.x).collect();
    let d1: Vec<f32> = yaw.windows(2).map(|p| wrap(p[1] - p[0])).collect();
    let rms2 = |d: &[f32]| {
        let s: Vec<f32> = d.windows(2).map(|p| p[1] - p[0]).collect();
        (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
    };
    // Frames the drawn pose did not move while walking (in the camera's
    // frame, which rides the body): a held pose.
    let held = w.windows(2).filter(|p| p[1].3 == p[0].3).count();
    let flips = d1
        .windows(2)
        .filter(|p| p[0] * p[1] < 0.0 && p[0].abs().max(p[1].abs()) > 1e-4)
        .count();
    let range = |x: &[f32]| {
        let lo = x.iter().copied().fold(f32::MAX, f32::min);
        let hi = x.iter().copied().fold(f32::MIN, f32::max);
        hi - lo
    };
    println!(
        "summary {mission} {class} {hz} Hz {flags:?} blend={}: frames {} yaw turned {:.3} deg, yaw 2nd-diff RMS {:.4} deg, yaw-step sign flips {} ({:.0}%), side step 1st-diff RMS {:.4} m, head x range {:.3} m, head x 2nd-diff RMS {:.4} m, head 2nd-diff max {:.4} m, held frames {held}",
        walk.blend.on,
        w.len(),
        d1.iter().sum::<f32>(),
        rms2(&d1),
        flips,
        100.0 * flips as f32 / d1.len().max(1) as f32,
        {
            let s: Vec<f32> = side.windows(2).map(|p| p[1] - p[0]).collect();
            (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
        },
        range(&headx),
        {
            let d: Vec<f32> = headx.windows(2).map(|p| p[1] - p[0]).collect();
            rms2(&d)
        },
        {
            let d: Vec<f32> = headx.windows(2).map(|p| p[1] - p[0]).collect();
            d.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0, f32::max)
        },
    );
    Ok(())
}

fn wrap(deg: f32) -> f32 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}
