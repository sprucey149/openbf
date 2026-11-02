//! Opt-in real-content benchmark. No timing or observation feeds the sim.
//! `OPENBF_BENCH_DETAIL=out.json game --bench ...` writes raw tick/frame rows.
//! Zero frames measures only the sim, without creating a graphics device.

use super::*;
use serde_json::json;
use std::time::Instant;

// CLOCK_THREAD_CPUTIME_ID excludes scheduler waits. Other platforms retain wall
// samples and report no CPU samples, rather than relabeling wall time as CPU.
#[cfg(target_os = "linux")]
fn cpu_ms() -> Option<f64> {
    #[repr(C)]
    struct Timespec {
        sec: std::ffi::c_long,
        ns: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: std::ffi::c_int, time: *mut Timespec) -> std::ffi::c_int;
    }
    let mut t = Timespec { sec: 0, ns: 0 };
    // SAFETY: a writable ABI-compatible timespec; Linux clock id 3 is thread CPU.
    (unsafe { clock_gettime(3, &mut t) } == 0).then_some(t.sec as f64 * 1000.0 + t.ns as f64 / 1e6)
}

#[cfg(not(target_os = "linux"))]
fn cpu_ms() -> Option<f64> {
    None
}

#[inline(never)]
fn measured_tick(walk: &mut WalkSim, pad: &[core::sim::PadState]) {
    walk.step_tick(pad);
}

/// Listed ticks take this frame, so `valgrind --tool=callgrind
/// --toggle-collect='*targeted_tick*'` profiles one slow tick of a repeatable
/// run: same work as `measured_tick`, only a different symbol.
#[inline(never)]
fn targeted_tick(walk: &mut WalkSim, pad: &[core::sim::PadState]) {
    // Differs from `measured_tick`, or LLVM folds the two into one symbol.
    std::hint::black_box(0u32);
    walk.step_tick(pad);
}

/// `OPENBF_BENCH_TARGET_TICKS=995,2216`: one-based ticks for [`targeted_tick`].
fn target_ticks() -> Vec<u32> {
    std::env::var("OPENBF_BENCH_TARGET_TICKS")
        .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
        .unwrap_or_default()
}

/// The slowest ticks of a run, with what they cost.
#[derive(Default)]
struct SlowTicks(Vec<(u32, f64, game::frame_profile::TickCosts)>);

impl SlowTicks {
    const KEPT: usize = 5;

    fn keep(&mut self, tick: u32, ms: f64, cost: &game::frame_profile::TickCosts) {
        if self.0.len() == Self::KEPT && self.0.last().is_some_and(|t| t.1 >= ms) {
            return;
        }
        self.0.push((tick, ms, cost.clone()));
        self.0.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.0.truncate(Self::KEPT);
    }

    fn print(&self) {
        for (tick, ms, cost) in &self.0 {
            println!("slow tick {tick}: {ms:.1} ms, {}", cost.line());
        }
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

pub(super) fn run(v: &ViewerArgs, output: &std::path::Path) -> Result<()> {
    let t = Instant::now();
    let (scene, load) = game::assets::load_battle_scene(&v.map, &v.mode)?;
    if let Some((path, Err(e))) = &load.common {
        return Err(anyhow!("common assets {}: {e:#}", path.display()));
    }
    let load_ms = ms(t);
    let setup = BattleSetup::from_args(v, v.seed)?;
    let t = Instant::now();
    let total = std::env::var_os("OPENBF_BENCH_TOTAL_PER_SIDE").is_some();
    let counts = if total {
        let n =
            v.ai.filter(|&n| n > 0)
                .context("total population requires --ai > 0")?;
        let mut counts = [n; 2];
        counts[usize::from(setup.player_team.min(1))] -= 1;
        Some(counts)
    } else {
        None
    };
    let mut walk = super::super::battle::make_walk_with_counts(&scene, &setup, counts);
    let setup_ms = ms(t);
    let initial_alive = alive(&walk);
    if total {
        anyhow::ensure!(
            initial_alive == [v.ai.unwrap() as usize; 2],
            "opening population mismatch: {initial_alive:?}"
        );
    }
    let initial_hash = format!("{:016x}", walk.sim.hash());
    let pad = [core::sim::PadState::default()];
    let mut ticks = Vec::with_capacity(v.bench_ticks as usize);
    let mut slowest = SlowTicks::default();
    let mut hashes = Vec::new();
    let mut cameras = Vec::new();
    let targets = target_ticks();
    for k in 0..v.bench_ticks {
        let t = Instant::now();
        let cpu = cpu_ms();
        if targets.contains(&(k + 1)) {
            targeted_tick(&mut walk, &pad);
        } else {
            measured_tick(&mut walk, &pad);
        }
        let cpu = cpu.zip(cpu_ms()).map(|(a, b)| b - a);
        let wall = ms(t);
        let mut cost = game::frame_profile::TickCosts::default();
        cost.add(&walk.sim.tick_cost, |u| {
            game::frame_profile::unit_name(&walk, u)
        });
        slowest.keep(k + 1, wall, &cost);
        ticks.push(json!({
            "tick": k + 1, "wall_ms": wall, "cpu_ms": cpu, "cost": serde_json::to_value(&cost)?,
        }));
        if (k + 1) % 30 == 0 || k + 1 == v.bench_ticks {
            hashes.push(json!([k + 1, format!("{:016x}", walk.sim.hash())]));
            let cam = walk.camera();
            cameras.push(json!([
                k + 1,
                cam.position.to_array().map(f32::to_bits),
                cam.yaw.to_bits(),
                cam.pitch.to_bits(),
                cam.fov_y.to_bits()
            ]));
        }
    }
    let sim_hash = format!("{:016x}", walk.sim.hash());
    let mut report = json!({
        "schema": 2, "map": v.map, "seed": v.seed, "size": [v.size.0, v.size.1],
        "ai": v.ai, "initial_alive": initial_alive, "alive_after_sim": alive(&walk),
        "total_per_side": total,
        "native_alive": walk.sim.units.iter().filter(|u| u.alive && u.team >= 2).count(),
        "vehicles_alive": walk.sim.vehicles.iter().filter(|v| v.alive).count(),
        "initial_hash": initial_hash, "sim_hash": sim_hash, "tick_hashes": hashes,
        // Navigation caches are derived inputs and omitted from Sim::hash.
        // Keep their float bits too so setup optimizations can prove parity.
        "nav_floors": walk.sim.nav_floors.iter().map(|y| y.to_bits()).collect::<Vec<_>>(),
        "nav_unwalkable": walk.sim.nav_unwalkable,
        "nav_trapped": walk.sim.nav_trapped,
        "kill_floor": walk.sim.kill_floor.map(f32::to_bits),
        "camera_samples": cameras,
        "load_ms": load_ms, "setup_ms": setup_ms, "ticks": ticks,
        "leaf_groups": scene.manifest.leaf.len(),
        "falling_leaves": scene.manifest.leaf.iter().map(|l| u32::from(l.falling_count)).sum::<u32>(),
    });
    if v.bench_frames > 0 {
        render(v, &scene, &setup, &mut walk, &mut report)?;
    }
    std::fs::write(output, serde_json::to_vec(&report)?)?;
    slowest.print();
    println!("detailed bench {}: sim hash {sim_hash}", output.display());
    Ok(())
}

fn alive(walk: &WalkSim) -> [usize; 2] {
    [0, 1].map(|team| {
        walk.sim
            .units
            .iter()
            .filter(|u| u.alive && u.team == team)
            .count()
    })
}

fn render(
    v: &ViewerArgs,
    scene: &game::assets::Scene,
    setup: &BattleSetup,
    walk: &mut WalkSim,
    report: &mut serde_json::Value,
) -> Result<()> {
    render::made::enable(true);
    let t = Instant::now();
    let mut r = Renderer::new_headless(v.size.0, v.size.1).map_err(|e| anyhow!("{e}"))?;
    r.cull = !v.no_cull;
    if std::env::var("OPENBF_TERRAIN_LIGHT_GROUPS").as_deref() == Ok("0") {
        r.set_terrain_light_grouping(false);
    }
    report["device_ms"] = json!(ms(t));
    report["adapter"] = json!(format!("{:?}", r.adapter_info()));
    let t = Instant::now();
    let mut gpu = upload_scene(&mut r, scene)?;
    game::forest::configure(&mut r, scene, setup.mission.as_ref())?;
    let assets = v.map.parent().unwrap_or_else(|| std::path::Path::new("."));
    let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, assets, scene, walk);
    let mut frame = ReplayFrame {
        cube: upload_unit_cube(&mut r)?,
        models: [vis.has_model(0), vis.has_model(1)],
        vis,
        vehicle_vis: game::viewer::VehicleVisuals::load(
            &mut r,
            scene,
            assets,
            &walk.vehicle_geometry,
        ),
        deka_vis: game::viewer::DroidekaVisuals::load(&mut r, scene, assets, walk),
        quad: game::viewer::upload_billboard_quad(&mut r)?,
        sprite: game::viewer::upload_fx_sprite(&mut r).ok(),
        icons: squad_icons(&mut r, assets),
        fx: game::fx::Fx::for_walk(assets, &scene.dir, &scene.manifest.source_game, walk),
        bolts: game::bolt_vis::BoltVis::new(assets, &scene.manifest.source_game),
        weather: game::weather::Weather::load(
            &scene.manifest,
            assets,
            &scene.dir,
            game::weather::STILL_SEED,
        ),
        holos: game::holo::Holos::default(),
        overlay: v
            .hud
            .then(|| ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT)),
        hud_view: v.hud.then(|| load_hud_view(assets, &r)).flatten(),
        feed: game::event_feed::EventFeed::for_walk(assets, &scene.manifest.source_game, walk),
    };
    frame.fx.set_water(&scene.manifest);
    frame.fx.set_scene_effects(scene);
    // Match the windowed battle's HITCH loading phase. Otherwise the harness
    // reports first-shot uploads that production already preloads.
    frame.fx.preload(&mut r);
    frame.vehicle_vis.preload(&mut r, &walk.sim);
    frame.bolts.preload(&mut r, walk);
    gpu.step_stations(&walk.sim);
    frame.vis.update(walk, core::TICK_DT);
    report["upload_ms"] = json!(ms(t));
    report["load_objects"] = made();
    report["load_objects_dropped"] = json!(render::made::dropped());
    let mut frames = Vec::with_capacity(v.bench_frames as usize);
    // Use a fixed overview or explicitly supplied eye; keep the production
    // replay renderer, including its effects, forest, lights and HUD.
    let mut view = v.clone();
    if !v.cam_set && !v.bench_player_cam {
        let mut cam = super::bench_camera(v, scene, walk);
        if std::env::var_os("OPENBF_BENCH_FOREST_EYE").is_some() {
            let ground = scene.heightfield();
            let objects = game::viewer::leaf::objects(
                &scene.manifest,
                &game::viewer::foliage::records(scene),
                ground.as_ref(),
            );
            let object = objects
                .iter()
                .find(|o| scene.manifest.leaf[o.class as usize].falling_count > 0)
                .context("forest benchmark requires a baked falling-leaf emitter")?;
            let at = object.matrix.w_axis.truncate();
            let y = ground.as_ref().map_or(at.y, |g| g.height_at(at.x, at.z));
            cam.position = glam::Vec3::new(at.x + 4.0, y + 2.0, at.z + 7.0);
            cam.yaw = 20f32.to_radians();
            cam.pitch = -12f32.to_radians();
        }
        view.cam = (
            cam.position.x,
            cam.position.y,
            cam.position.z,
            cam.yaw.to_degrees(),
            cam.pitch.to_degrees(),
        );
        view.cam_set = true;
    }
    report["camera"] = json!(view.cam);
    if let Some(states) = pricing_states()? {
        return price_states(
            v, &mut r, &mut gpu, walk, &mut frame, &view, &states, report,
        );
    }
    for k in 0..=v.bench_frames {
        let t = Instant::now();
        if k % 2 == 0 {
            step_world(v, &mut gpu, walk, &mut frame);
        }
        let update_ms = ms(t);
        let render_start = Instant::now();
        frame.render(&mut r, &mut gpu, walk, &view, 1.0 / 60.0)?;
        r.wait_idle().map_err(|e| anyhow!("{e}"))?;
        let render_ms = ms(render_start);
        let wall = ms(t);
        let stats = r.frame_stats();
        frames.push(json!({"frame": k, "wall_ms": wall, "render_ms": render_ms,
            "update_ms": update_ms, "draw_calls": stats.draw_calls,
            "culled": stats.culled, "submitted": stats.submitted, "objects": made(),
            "gpu_ms": r.gpu_sections(), "alive": alive(walk)}));
    }
    report["frames"] = json!(frames);
    report["final_hash"] = json!(format!("{:016x}", walk.sim.hash()));
    if let Some(path) = &v.capture_frame {
        let (rgba, w, h) = r.capture_rgba()?;
        image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8)?;
    }
    render::made::enable(false);
    Ok(())
}

/// `OPENBF_FRAMEAB=base,order:mesh,no:models`: price these states. The first
/// is what the others are priced against.
fn pricing_states() -> Result<Option<Vec<(String, render::FrameAb)>>> {
    let Ok(list) = std::env::var("OPENBF_FRAMEAB") else {
        return Ok(None);
    };
    let states = list
        .split(',')
        .map(|name| {
            render::FrameAb::parse(name.trim())
                .map(|ab| (name.trim().to_string(), ab))
                .map_err(|e| anyhow!("OPENBF_FRAMEAB: {e}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(states))
}

/// Per-frame interleaved pricing (`render::frame_ab`): each round is one world
/// step drawn once per state, in an order that rotates so no state always
/// follows the same one. The states differ only in the renderer, and the sim
/// steps as it does without them: `final_hash` must not move.
/// `--capture-frame x.png` writes the last round's frames as `x.<state>.png`.
#[allow(clippy::too_many_arguments)]
fn price_states(
    v: &ViewerArgs,
    r: &mut Renderer,
    gpu: &mut SceneGpu,
    walk: &mut WalkSim,
    frame: &mut ReplayFrame,
    view: &ViewerArgs,
    states: &[(String, render::FrameAb)],
    report: &mut serde_json::Value,
) -> Result<()> {
    anyhow::ensure!(
        r.gpu_timing_on(),
        "OPENBF_FRAMEAB prices GPU time: run with OPENBF_GPU_TIMING=1 on an adapter with timestamps"
    );
    let names: Vec<String> = states.iter().map(|s| s.0.clone()).collect();
    let mut rounds = Vec::with_capacity(v.bench_frames as usize);
    for k in 0..=v.bench_frames {
        if k % 2 == 0 {
            step_world(v, gpu, walk, frame);
        }
        let mut round = vec![game::bench::PricedFrame::default(); states.len()];
        for at in 0..states.len() {
            let s = (at + k as usize) % states.len();
            r.frame_ab = states[s].1;
            let cpu = cpu_ms();
            frame.render(r, gpu, walk, view, 1.0 / 60.0)?;
            let cpu = cpu.zip(cpu_ms()).map(|(a, b)| b - a);
            r.wait_idle().map_err(|e| anyhow!("{e}"))?;
            r.collect_gpu_timing();
            round[s] = game::bench::PricedFrame {
                sections: r
                    .gpu_sections()
                    .iter()
                    .map(|(l, ms)| (l.to_string(), *ms))
                    .collect(),
                cpu_ms: cpu,
                draw_calls: r.frame_stats().draw_calls,
            };
            if let (Some(path), true) = (&v.capture_frame, k == v.bench_frames) {
                let (rgba, w, h) = r.capture_rgba()?;
                let path = path.with_extension(format!("{}.png", states[s].0.replace(':', "_")));
                image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8)?;
            }
        }
        rounds.push(round);
    }
    r.frame_ab = render::FrameAb::default();
    let prices = game::bench::price(&names, &rounds);
    game::bench::print_prices(&prices);
    report["prices"] = json!(prices
        .iter()
        .map(|p| json!({
            "state": p.state, "rounds": p.rounds, "gpu_ms": p.gpu_ms, "delta_ms": p.delta_ms,
            "delta_q1": p.delta_q1, "delta_q3": p.delta_q3, "cpu_delta_ms": p.cpu_delta_ms,
            "draw_calls": p.draw_calls, "sections": p.sections,
        }))
        .collect::<Vec<_>>());
    report["final_hash"] = json!(format!("{:016x}", walk.sim.hash()));
    render::made::enable(false);
    Ok(())
}

/// One sim tick and everything the frames read from it, as a battle frame does.
fn step_world(v: &ViewerArgs, gpu: &mut SceneGpu, walk: &mut WalkSim, frame: &mut ReplayFrame) {
    measured_tick(walk, &[core::sim::PadState::default()]);
    gpu.step_stations(&walk.sim);
    frame.fx.ingest_posed(walk, Some(&frame.vis));
    drop(frame.fx.take_sounds());
    frame.vis.update(walk, core::TICK_DT);
    let mut points = frame.vis.fire_points(walk);
    frame.deka_vis.fire_points(&walk.sim, &mut points);
    frame.fx.set_repair_origins(&points);
    frame.bolts.set_fire_points(points);
    frame.fx.set_jet_frames(frame.vis.jet_frames(walk));
    frame.fx.update(core::TICK_DT);
    if let Some(w) = frame.weather.as_mut() {
        w.update(core::TICK_DT);
    }
    frame.bolts.update(walk, core::TICK_DT);
    tick_event_feed(&mut frame.feed, &walk.sim, walk.player, v.hud);
}

fn made() -> serde_json::Value {
    json!(render::made::take()
        .into_iter()
        .map(|m| json!({
            "kind": m.kind, "label": m.label, "bytes": m.bytes, "ms": m.ms,
        }))
        .collect::<Vec<_>>())
}
