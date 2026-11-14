//! Queue item OC's measuring tool: the water moving, measured without the
//! game binary, through the same `SceneGpu::animate_water` the game loop calls
//! every frame (`main/windowed.rs`; wired in when OC landed, `b8ebe59`).
//!
//!   cargo run --release -p game --example water_frames -- <map_dir> <out_dir> [frames]
//!
//! 1. The map's golden VANTAGE (`gallery::vantage_camera` at `MAP_PANEL`, the
//!    conquest layer merged as `gallery::map_sheet` merges it): `still.png`
//!    is the scene exactly as `upload_scene` builds it, which is what the
//!    golden draws; `t0.00.png`, `t0.50.png`, `t1.00.png` are the water
//!    stepped to those times.
//! 2. `nowater.png`: the still frame with the water draws taken out. A pixel
//!    that differs from `still.png` is a WATER pixel. For each pair of frames
//!    it prints the mean abs difference over the water pixels, and how many
//!    pixels OUTSIDE them differ at all (with their bounding box).
//! 3. Frame time from the overview camera `--bench` uses (no soldiers — this
//!    loads no battle): `frames` frames of the still scene, then `frames`
//!    frames stepping the water every frame (FFT + upload + render + wait),
//!    alternated over five rounds; and the step alone.
//! 4. The ocean's amplitude — the disc's own, unnormalised (`re-water.md`
//!    §3a, queue OCB); this prints what the port draws.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use game::gallery::{vantage_camera, MAP_PANEL};
use game::viewer::{overview_camera, upload_scene};
use game::water_anim::{Ocean, Spectrum, OCEAN_SEED};
use render::{Camera, DrawCmd, Renderer};

fn shoot(r: &mut Renderer, cam: &Camera, draws: &[DrawCmd], clear: [f64; 4]) -> Result<Vec<u8>> {
    r.render(cam, draws, clear)
        .map_err(|e| anyhow!("render: {e}"))?;
    let (rgba, _, _) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
    Ok(rgba)
}

fn save(out: &Path, name: &str, rgba: &[u8], (w, h): (u32, u32)) -> Result<()> {
    let png = render::encode_png(rgba, w, h).map_err(|e| anyhow!("png: {e}"))?;
    std::fs::write(out.join(name), png).with_context(|| name.to_string())
}

/// Mean abs difference over the water pixels (rgb), and the count and
/// bounding box of the differing pixels outside them.
fn compare(label: &str, a: &[u8], b: &[u8], water: &[bool], w: u32) {
    let (mut sum, mut n_water) = (0u64, 0u64);
    let (mut outside, mut bbox) = (0u64, (u32::MAX, u32::MAX, 0u32, 0u32));
    for (i, (pa, pb)) in a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .enumerate()
    {
        let d: u64 = (0..3).map(|c| u64::from(pa[c].abs_diff(pb[c]))).sum();
        if water[i] {
            sum += d;
            n_water += 1;
        } else if d > 0 {
            outside += 1;
            let (x, y) = (i as u32 % w, i as u32 / w);
            bbox = (bbox.0.min(x), bbox.1.min(y), bbox.2.max(x), bbox.3.max(y));
        }
    }
    let mean = sum as f64 / (3 * n_water.max(1)) as f64;
    let bb = if outside > 0 {
        format!("x {}..{} y {}..{}", bbox.0, bbox.2, bbox.1, bbox.3)
    } else {
        "-".into()
    };
    println!("{label}: water mean abs {mean:.3} over {n_water} px; outside-water changed {outside} px, bbox {bb}");
}

fn stats(v: &mut [f64]) -> (f64, f64) {
    v.sort_by(f64::total_cmp);
    (v.iter().sum::<f64>() / v.len() as f64, v[v.len() / 2])
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: water_frames <map_dir> <out_dir> [frames]";
    let map = PathBuf::from(args.next().context(usage)?);
    let out = PathBuf::from(args.next().context(usage)?);
    let frames: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(240);
    std::fs::create_dir_all(&out)?;

    let mut scene = game::assets::load_scene(&map).with_context(|| map.display().to_string())?;
    scene.apply_mode_layer("con");
    let scene = scene;
    let size = MAP_PANEL;
    let mut r = Renderer::new_headless(size.0, size.1).map_err(|e| anyhow!("renderer: {e}"))?;
    let mut gpu = upload_scene(&mut r, &scene)?;
    let still_draws = gpu.draws.clone();
    let range = gpu
        .water_motion
        .as_ref()
        .map(|m| m.draw_range())
        .context("this map has no water")?;
    let cam = vantage_camera(&scene);

    let still = shoot(&mut r, &cam, &gpu.draws, gpu.clear)?;
    save(&out, "still.png", &still, size)?;
    let dry: Vec<DrawCmd> = gpu
        .draws
        .iter()
        .enumerate()
        .filter(|(i, _)| !range.contains(i))
        .map(|(_, d)| *d)
        .collect();
    let nowater = shoot(&mut r, &cam, &dry, gpu.clear)?;
    save(&out, "nowater.png", &nowater, size)?;
    let water: Vec<bool> = still
        .as_chunks::<4>()
        .0
        .iter()
        .zip(nowater.as_chunks::<4>().0)
        .map(|(a, b)| a[..3] != b[..3])
        .collect();
    println!(
        "{}: water draws {} (still); water pixels {} of {}",
        map.display(),
        range.len(),
        water.iter().filter(|&&w| w).count(),
        water.len()
    );

    let mut shots = Vec::new();
    for t in [0.0f64, 0.5, 1.0] {
        gpu.animate_water(&mut r, t, cam.position)?;
        let img = shoot(&mut r, &cam, &gpu.draws, gpu.clear)?;
        save(&out, &format!("t{t:.2}.png"), &img, size)?;
        shots.push((t, img));
    }
    let motion = gpu.water_motion.as_ref().expect("checked above");
    println!(
        "stepped: ocean {}, water draws now {}",
        motion.is_ocean(),
        motion.draw_range().len()
    );
    compare("still -> t0.00", &still, &shots[0].1, &water, size.0);
    for pair in shots.windows(2) {
        let label = format!("t{:.2} -> t{:.2}", pair[0].0, pair[1].0);
        compare(&label, &pair[0].1, &pair[1].1, &water, size.0);
    }

    if motion.is_ocean() {
        let o = Ocean::new(16, 160.0, &Spectrum::KAM1, OCEAN_SEED);
        for t in [0.0f32, 0.5, 1.0, 10.0, 100.0] {
            let h = o.heights(f64::from(t));
            let (lo, hi) = h
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
            println!(
                "ocean t {t:>5.1}: rms {:.4} m, min {lo:.4} m, max {hi:.4} m",
                o.rms(f64::from(t))
            );
        }
        println!(
            "ocean bound {:.4} m (loose: every mode at its crest at once)",
            o.bound()
        );
    }

    if frames > 0 {
        for (name, view) in [("overview", overview_camera(&scene)), ("vantage", cam)] {
            let frame = |r: &mut Renderer, draws: &[DrawCmd], clear| -> Result<f64> {
                let t = Instant::now();
                r.render(&view, draws, clear)
                    .map_err(|e| anyhow!("render: {e}"))?;
                r.wait_idle().map_err(|e| anyhow!("wait: {e}"))?;
                Ok(t.elapsed().as_secs_f64() * 1e3)
            };
            // Warm both paths once: the first submit pays for pipelines and
            // first-touch of every buffer.
            frame(&mut r, &still_draws, gpu.clear)?;
            let fs = r.frame_stats();
            gpu.animate_water(&mut r, 0.0, view.position)?;
            frame(&mut r, &gpu.draws, gpu.clear)?;
            let fm = r.frame_stats();
            let water = gpu
                .water_motion
                .as_ref()
                .map_or(0, |m| m.draw_range().len());
            println!(
                "{name}: still {} draws submitted / {} culled; moving {} / {} ({water} water draws)",
                fs.submitted, fs.culled, fm.submitted, fm.culled
            );
            let (mut still_ms, mut moving_ms, mut step_ms) = (Vec::new(), Vec::new(), Vec::new());
            let mut clock = 0.0f64;
            for _round in 0..5 {
                for _ in 0..frames {
                    still_ms.push(frame(&mut r, &still_draws, gpu.clear)?);
                }
                for _ in 0..frames {
                    clock += 1.0 / 60.0;
                    let t = Instant::now();
                    gpu.animate_water(&mut r, clock, view.position)?;
                    let s = t.elapsed().as_secs_f64() * 1e3;
                    r.render(&view, &gpu.draws, gpu.clear)
                        .map_err(|e| anyhow!("render: {e}"))?;
                    r.wait_idle().map_err(|e| anyhow!("wait: {e}"))?;
                    moving_ms.push(t.elapsed().as_secs_f64() * 1e3);
                    step_ms.push(s);
                }
            }
            let (sm, sd) = stats(&mut still_ms);
            let (mm, md) = stats(&mut moving_ms);
            let (tm, td) = stats(&mut step_ms);
            println!("{name} frame ms (5 x {frames} each, alternated):");
            println!("  still   mean {sm:.3}  median {sd:.3}");
            println!(
                "  moving  mean {mm:.3}  median {md:.3}   delta mean {:+.1}%  median {:+.1}%",
                100.0 * (mm - sm) / sm,
                100.0 * (md - sd) / sd
            );
            println!("  step alone (FFT + upload + draws) mean {tm:.4}  median {td:.4}");
        }
        println!("adapter {:?}", r.adapter_info().name);
    }
    Ok(())
}
