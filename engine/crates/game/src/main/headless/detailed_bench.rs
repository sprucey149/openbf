//! Opt-in real-content benchmark. No timing or observation feeds the sim.
//! `OPENBF_BENCH_DETAIL=out.json game --bench ...` writes raw tick/frame rows.
//! Zero frames measures only the sim, without creating a graphics device.

use super::*;
use serde_json::json;
use std::time::Instant;

#[path = "sensors.rs"]
mod sensors;

// CLOCK_THREAD_CPUTIME_ID excludes scheduler waits. Other platforms retain wall
// samples and report no CPU samples, rather than relabeling wall time as CPU.
use game::frame_profile::thread_cpu_ms as cpu_ms;

/// This thread's user-mode instructions, the one count another job on the
/// machine does not move. On a hybrid CPU it only runs on the cores of the PMU
/// the kernel chose, so pin the run to the performance cores.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
struct InstrCounter(Option<std::ffi::c_int>);

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl InstrCounter {
    fn new() -> Self {
        unsafe extern "C" {
            fn syscall(n: std::ffi::c_long, ...) -> std::ffi::c_long;
        }
        // perf_event_attr, ABI size 120: type 0 (hardware), size, config 1
        // (instructions), then flags at word 5: exclude_kernel | exclude_hv.
        let mut attr = [0u64; 15];
        attr[0] = 120 << 32;
        attr[1] = 1;
        attr[5] = (1 << 5) | (1 << 6);
        // SAFETY: perf_event_open (298 on x86-64) reads that many bytes from
        // `attr`; pid 0, cpu -1 is this thread on any CPU, no group, no flags.
        let fd = unsafe { syscall(298, attr.as_ptr(), 0i32, -1i32, -1i32, 0u64) };
        Self((fd >= 0).then_some(fd as std::ffi::c_int))
    }

    fn read(&self) -> Option<u64> {
        unsafe extern "C" {
            fn read(fd: std::ffi::c_int, buf: *mut std::ffi::c_void, count: usize) -> isize;
        }
        let mut value = 0u64;
        // SAFETY: a counter fd with no read_format writes one u64.
        (unsafe { read(self.0?, (&mut value as *mut u64).cast(), 8) } == 8).then_some(value)
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
struct InstrCounter;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
impl InstrCounter {
    fn new() -> Self {
        Self
    }

    fn read(&self) -> Option<u64> {
        None
    }
}

/// One moment's counters. `since` gives a stretch's thread CPU ms, instructions
/// and allocations, each `null` where this build or platform has none.
struct Mark {
    cpu: Option<f64>,
    instr: Option<u64>,
    alloc: Option<(u64, u64)>,
}

impl Mark {
    fn now(c: &InstrCounter) -> Self {
        Self {
            cpu: cpu_ms(),
            instr: c.read(),
            alloc: game::alloc_count::so_far(),
        }
    }

    fn since(&self, c: &InstrCounter) -> serde_json::Value {
        let (cpu, instr, alloc) = (cpu_ms(), c.read(), game::alloc_count::so_far());
        let alloc = self.alloc.zip(alloc);
        json!({
            "cpu_ms": self.cpu.zip(cpu).map(|(a, b)| b - a),
            "instr": self.instr.zip(instr).map(|(a, b)| b - a),
            "allocs": alloc.map(|(a, b)| b.0 - a.0),
            "alloc_bytes": alloc.map(|(a, b)| b.1 - a.1),
        })
    }
}

/// Peak and current resident kilobytes of this process, Linux only.
fn memory_kb() -> serde_json::Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.trim().trim_end_matches(" kB").parse::<u64>().ok())
    };
    json!({"peak": field("VmHWM:"), "now": field("VmRSS:")})
}

#[inline(never)]
/// `OPENBF_BENCH_PLAYER=script`: one human who plays by a fixed script instead of standing
/// still, so a run exercises the player path (spawn screens, walking, turning, firing, jumping,
/// reloading, the weapon switch, the action key, respawn and the spawn screens after a death)
/// and its hashes can pin that path before it is refactored (issue #194, phase 0). Pure in the
/// tick number: the same ticks give the same pads on every machine.
fn scripted_pad(k: u32) -> core::sim::PadState {
    use core::sim::buttons;
    let tap = |period: u32, at: u32, bit: u32| if k % period == at { bit } else { 0 };
    let mut b = 0;
    if k % 40 < 15 {
        b |= buttons::FIRE;
    }
    b |= tap(97, 0, buttons::JUMP) | tap(150, 0, buttons::ACTION) | tap(200, 100, buttons::RELOAD);
    b |= tap(250, 10, buttons::SWITCH_WEAPON) | tap(90, 0, buttons::RESPAWN);
    // Spawn screens: accept on every other early tick, then now and then (after a death).
    if k < 8 && k % 2 == 0 || k % 75 == 0 {
        b |= buttons::SPAWN_ACCEPT;
    }
    // Walk forward; turn in a slow sweep that reverses so the player does not walk off in a line.
    let turn = if (k / 120) % 2 == 0 { 2500 } else { -2500 };
    core::sim::PadState {
        ly: 20000,
        rx: if k % 3 == 0 { turn } else { 0 },
        buttons: b,
        ..Default::default()
    }
}

/// `--bench-seats`: who drives each local player's pad, indexed by their slot. `ai`: the bots'
/// own brain thinks for the seat's soldier on the sim as it stands (`core::ai::think`, its state
/// kept here, never in the sim), and on the spawn screens the seat accepts a page every second.
/// `script`: [`scripted_pad`], each seat phased. `idle`: nobody touches a pad. Every pad is a
/// function of the sim and the tick, so a run's hashes do not depend on its frames or threads.
struct SeatDriver {
    mode: &'static str,
    brains: Vec<Option<(core::unit::UnitHandle, core::ai::AiState)>>,
    /// Per seat: metres walked by its soldiers, lives begun, last position.
    walked: Vec<f32>,
    lives: Vec<u32>,
    at: Vec<Option<glam::Vec3>>,
}

impl SeatDriver {
    fn new(mode: &'static str, seats: usize) -> Self {
        Self {
            mode,
            brains: vec![None; seats],
            walked: vec![0.0; seats],
            lives: vec![0; seats],
            at: vec![None; seats],
        }
    }

    fn pads(&mut self, walk: &WalkSim) -> Vec<core::sim::PadState> {
        use core::sim::{buttons, PadState};
        let k = walk.sim.tick as u32;
        let mut pads = vec![PadState::default(); self.brains.len().max(1)];
        for i in 0..walk.local_count().min(self.brains.len()) {
            let slot = usize::from(walk.local_slot_of(i));
            let h = walk.local_unit(i);
            let live = walk.sim.unit(h).filter(|u| u.alive);
            match (live, self.at[i]) {
                (Some(u), Some(was)) => self.walked[i] += (u.soldier.pos - was).length(),
                (Some(_), None) => self.lives[i] += 1,
                _ => {}
            }
            self.at[i] = live.map(|u| u.soldier.pos);
            let pad = match (self.mode, live) {
                ("ai", Some(u)) => {
                    let brain = match &mut self.brains[i] {
                        Some((had, st)) if *had == h => st,
                        b => &mut b.insert((h, u.ai)).1,
                    };
                    core::ai::think(&walk.sim, h, brain).0
                }
                ("ai", None) => {
                    self.brains[i] = None;
                    // One spawn page a second, seats a third of a second apart.
                    let phase = (k + 10 * i as u32) % 30;
                    PadState {
                        buttons: match phase {
                            0 => buttons::SPAWN_ACCEPT,
                            15 => buttons::RESPAWN,
                            _ => 0,
                        },
                        ..Default::default()
                    }
                }
                ("script", _) => scripted_pad(k.wrapping_add(37 * i as u32)),
                _ => PadState::default(),
            };
            if pads.len() <= slot {
                pads.resize(slot + 1, PadState::default());
            }
            pads[slot] = pad;
        }
        pads
    }

    fn report(&self, walk: &WalkSim) -> serde_json::Value {
        let seats: Vec<serde_json::Value> = (0..walk.local_count().min(self.brains.len()))
            .map(|i| {
                let u = &walk.sim.units[walk.local_unit(i).index as usize];
                json!({"team": u.team, "kills": u.kills, "deaths": u.deaths,
                    "lives": self.lives[i], "walked_m": self.walked[i]})
            })
            .collect();
        json!({"mode": self.mode, "seats": seats})
    }
}

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
    let mut walk = super::super::battle::make_walk_with_counts(&scene, &setup, counts)?;
    // `--bench-views`: the further people on this machine, as the split window seats them but
    // without their spawn screens, so they respawn on their own like the first player here.
    for slot in 1..v.bench_views as u8 {
        walk.add_local_player(slot, None)
            .map_err(|e| anyhow!("bench player {slot} could not be seated: {e:?}"))?;
    }
    // Driven seats play as the window seats them: behind their spawn screens.
    if v.bench_seats != "idle" {
        super::super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
        let side_choice = setup
            .mission
            .as_ref()
            .map_or(true, |m| m.allows_side_select());
        for i in 1..walk.local_count() {
            let slot = usize::from(walk.local_slot_of(i));
            walk.sim.open_spawn_screens(slot, side_choice);
        }
    }
    let mut seats = SeatDriver::new(v.bench_seats, v.bench_views.max(1));
    let setup_ms = ms(t);
    let initial_alive = alive(&walk);
    if total {
        anyhow::ensure!(
            initial_alive == [v.ai.unwrap() as usize; 2],
            "opening population mismatch: {initial_alive:?}"
        );
    }
    let initial_hash = format!("{:016x}", walk.sim.hash());
    let scripted = std::env::var("OPENBF_BENCH_PLAYER").is_ok_and(|s| s == "script");
    // One driver per pad: the gate's one scripted human, or the seats' own.
    anyhow::ensure!(
        !scripted || v.bench_seats == "idle",
        "OPENBF_BENCH_PLAYER=script drives the first pad itself; use it with --bench-seats idle"
    );
    let mut ticks = Vec::with_capacity(v.bench_ticks as usize);
    let mut slowest = SlowTicks::default();
    let mut hashes = Vec::new();
    let mut cameras = Vec::new();
    let targets = target_ticks();
    let counter = InstrCounter::new();
    // The scripted player's life: deaths and (re)spawns seen, so a gate can show it covered them.
    let mut life = (0u32, 0u32);
    let mut was_alive = walk.sim.unit(walk.player).is_some_and(|u| u.alive);
    let memory_after_setup = memory_kb();
    for k in 0..v.bench_ticks {
        let mut pad = seats.pads(&walk);
        let t = Instant::now();
        let mark = Mark::now(&counter);
        if scripted {
            pad[0] = scripted_pad(k);
        }
        let pad = &pad[..];
        if targets.contains(&(k + 1)) {
            targeted_tick(&mut walk, pad);
        } else {
            measured_tick(&mut walk, pad);
        }
        let spent = mark.since(&counter);
        let is_alive = walk.sim.unit(walk.player).is_some_and(|u| u.alive);
        life.0 += u32::from(was_alive && !is_alive);
        life.1 += u32::from(!was_alive && is_alive);
        was_alive = is_alive;
        let wall = ms(t);
        let mut cost = game::frame_profile::TickCosts::default();
        cost.add(&walk.sim.tick_cost, |u| {
            game::frame_profile::unit_name(&walk, u)
        });
        slowest.keep(k + 1, wall, &cost);
        ticks.push(json!({
            "tick": k + 1, "wall_ms": wall, "cpu_ms": spent["cpu_ms"], "instr": spent["instr"],
            "allocs": spent["allocs"], "alloc_bytes": spent["alloc_bytes"],
            "cost": serde_json::to_value(&cost)?,
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
        "schema": 3, "map": v.map, "seed": v.seed, "size": [v.size.0, v.size.1],
        "ai": v.ai, "initial_alive": initial_alive, "alive_after_sim": alive(&walk),
        "total_per_side": total,
        "player_script": scripted, "player_deaths": life.0, "player_spawns": life.1,
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
        "memory_after_setup_kb": memory_after_setup, "memory_after_ticks_kb": memory_kb(),
        "leaf_groups": scene.manifest.leaf.len(),
        "falling_leaves": scene.manifest.leaf.iter().map(|l| u32::from(l.falling_count)).sum::<u32>(),
    });
    if v.bench_frames > 0 || v.bench_pipelined {
        render(v, &scene, &setup, &mut walk, &mut seats, &mut report)?;
    }
    report["seats"] = seats.report(&walk);
    report["memory_end_kb"] = memory_kb();
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
    seats: &mut SeatDriver,
    report: &mut serde_json::Value,
) -> Result<()> {
    render::made::enable(true);
    let t = Instant::now();
    let mut r = Renderer::new_headless(v.size.0, v.size.1).map_err(|e| anyhow!("{e}"))?;
    r.cull = !v.no_cull;
    if std::env::var("OPENBF_HARD_SHADOWS").as_deref() == Ok("1") {
        r.set_sun_shadows(true);
    }
    if let Some(scale) = v.bench_scale {
        r.set_world_scale(scale);
    }
    report["views"] = json!(v.bench_views);
    report["world_scale"] = json!(r.world_scale());
    report["device_ms"] = json!(ms(t));
    report["adapter"] = json!(format!("{:?}", r.adapter_info()));
    let t = Instant::now();
    let mut gpu = upload_scene(&mut r, scene)?;
    game::forest::configure(&mut r, scene, setup.mission.as_ref())?;
    let assets = v.map.parent().unwrap_or_else(|| std::path::Path::new("."));
    let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, assets, scene, walk);
    let mut frame = ReplayFrame {
        cube: upload_unit_cube(&mut r)?,
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
        overlay: v.hud.then(|| {
            let o = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
            ui::typography::face::install(o.ctx(), &game::hud_feed::menu_face(assets));
            o
        }),
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
    report["memory_after_upload_kb"] = memory_kb();
    report["load_objects"] = made();
    report["load_objects_dropped"] = json!(render::made::dropped());
    let mut frames = Vec::with_capacity(v.bench_frames as usize);
    // Use a fixed overview or explicitly supplied eye; keep the production
    // replay renderer, including its effects, forest, lights and HUD.
    let mut view = v.clone();
    if !v.cam_set && !v.bench_player_cam {
        let cam = super::bench_camera(v, scene, walk);
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
    if v.bench_pipelined {
        return pipelined(v, scene, &mut r, &mut gpu, walk, &mut frame, &view, seats, report);
    }
    let counter = InstrCounter::new();
    for k in 0..=v.bench_frames {
        let t = Instant::now();
        let mark = Mark::now(&counter);
        if k % 2 == 0 {
            let pads = seats.pads(walk);
            step_world(v, &mut gpu, walk, &mut frame, &pads, &mut []);
        }
        let update = mark.since(&counter);
        let update_ms = ms(t);
        if v.bench_views > 1 {
            let render_start = Instant::now();
            let views = split_frame(&mut r, &mut gpu, walk, &mut frame, v.bench_views)?;
            let render_ms = ms(render_start);
            let mut gpu_ms: Vec<(String, f64)> = Vec::new();
            for view in &views {
                for (label, t) in view["gpu_ms"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| Some((p.get(0)?.as_str()?.to_owned(), p.get(1)?.as_f64()?)))
                {
                    match gpu_ms.iter_mut().find(|(l, _)| *l == label) {
                        Some((_, sum)) => *sum += t,
                        None => gpu_ms.push((label, t)),
                    }
                }
            }
            // The timer's own sum of the present's views (`Renderer::gpu_sections` under
            // `time_every_view`): the same total as `gpu_ms`, a check on the sum.
            let present_gpu_ms: f64 = r.gpu_sections().iter().map(|s| s.1).sum();
            frames.push(json!({"frame": k, "wall_ms": ms(t), "render_ms": render_ms,
                "update": update, "update_ms": update_ms, "gpu_ms": gpu_ms,
                "present_gpu_ms": present_gpu_ms,
                "views": views, "alive": alive(walk)}));
            continue;
        }
        let render_start = Instant::now();
        let mark = Mark::now(&counter);
        frame.render(&mut r, &mut gpu, walk, &view, 1.0 / 60.0)?;
        // Up to here the CPU built and submitted the frame; the rest is waiting.
        let submit = mark.since(&counter);
        let submit_ms = ms(render_start);
        r.wait_idle().map_err(|e| anyhow!("{e}"))?;
        let render_ms = ms(render_start);
        let wall = ms(t);
        r.collect_gpu_timing();
        let stats = r.frame_stats();
        frames.push(json!({"frame": k, "wall_ms": wall, "render_ms": render_ms,
            "submit_ms": submit_ms, "submit": submit, "update": update,
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

/// `--bench-views`: one frame of `n` views as the split window draws it, each player's world
/// from their own eye through `render_view` into their cell. The bench waits for the GPU after
/// each view (the window does not) so each view's CPU build, submit and GPU sections are its
/// own; under `OPENBF_GPU_TIMING=1` every view is timed (`Renderer::time_every_view`).
fn split_frame(
    r: &mut Renderer,
    gpu: &mut SceneGpu,
    walk: &mut WalkSim,
    frame: &mut ReplayFrame,
    n: usize,
) -> Result<Vec<serde_json::Value>> {
    let (width, height) = r.size();
    let cells = game::split_layout::cells(n, width, height, Default::default());
    r.time_every_view = r.gpu_timing_on();
    let mut rows = Vec::with_capacity(cells.len());
    for (i, cell) in cells.iter().enumerate() {
        let t = Instant::now();
        let built = walk.with_local(i, |w| {
            let cam = w.camera();
            frame
                .build_view(r, gpu, w, &cam, false, (cell[2], cell[3]))
                .map(|(d, s)| (cam, d, s))
        });
        let (cam, draws, skinned) = built?;
        let build_ms = ms(t);
        let slot = render::ViewSlot {
            rect: *cell,
            first: i == 0,
            last: i + 1 == cells.len(),
        };
        r.render_view(slot, &cam, &draws, &skinned, gpu.clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        let submit_ms = ms(t) - build_ms;
        r.wait_idle().map_err(|e| anyhow!("{e}"))?;
        let view_ms = ms(t);
        r.collect_gpu_timing();
        let stats = r.frame_stats();
        rows.push(
            json!({"view": i, "build_ms": build_ms, "submit_ms": submit_ms,
            "view_ms": view_ms, "draws": draws.len(), "skinned": skinned.len(),
            "draw_calls": stats.draw_calls, "culled": stats.culled,
            "submitted": stats.submitted, "gpu_ms": r.gpu_view_sections(),
            "eye": cam.position.to_array()}),
        );
    }
    Ok(rows)
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
            let pads = vec![core::sim::PadState::default(); v.bench_views.max(1)];
            step_world(v, gpu, walk, frame, &pads, &mut []);
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

/// One sim tick from `pads` (by slot) and everything the frames read from it, as a battle
/// frame does; `feeds` are the further views' event feeds (`--bench-pipelined` with `--hud`).
fn step_world(
    v: &ViewerArgs,
    gpu: &mut SceneGpu,
    walk: &mut WalkSim,
    frame: &mut ReplayFrame,
    pads: &[core::sim::PadState],
    feeds: &mut [game::event_feed::EventFeed],
) {
    measured_tick(walk, pads);
    gpu.step_stations(&walk.sim);
    frame.fx.ingest_posed(walk, Some(&frame.vis));
    drop(frame.fx.take_sounds());
    frame.vis.update(walk, core::TICK_DT);
    let mut points = frame
        .vis
        .tick_fire_points(walk, |i| frame.fx.needs_repair_origin(i));
    frame.deka_vis.fire_points(&walk.sim, &mut points);
    frame.fx.set_repair_origins(&points);
    frame.bolts.set_fire_points(points);
    let jets = if walk
        .sim
        .units
        .iter()
        .any(|u| u.alive && u.soldier.stance.jetting())
        || !frame.fx.jets().is_empty()
    {
        frame.vis.jet_frames_where(walk, |i, u| {
            u.soldier.stance.jetting() || frame.fx.has_jet_for(i)
        })
    } else {
        Vec::new()
    };
    frame.fx.set_jet_frames(jets);
    frame.fx.update(core::TICK_DT);
    if let Some(w) = frame.weather.as_mut() {
        w.update(core::TICK_DT);
    }
    frame.bolts.update(walk, core::TICK_DT);
    tick_event_feed(&mut frame.feed, &walk.sim, walk.player, v.hud);
    for (k, feed) in feeds.iter_mut().enumerate() {
        tick_event_feed(feed, &walk.sim, walk.local_unit(k + 1), v.hud);
    }
}

fn made() -> serde_json::Value {
    json!(render::made::take()
        .into_iter()
        .map(|m| json!({
            "kind": m.kind, "label": m.label, "bytes": m.bytes, "ms": m.ms,
        }))
        .collect::<Vec<_>>())
}

/// Percentiles of `xs` (nearest rank), `null` when empty.
fn pct(xs: &[f64], ps: &[f64]) -> serde_json::Value {
    let mut v: Vec<f64> = xs.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return serde_json::Value::Null;
    }
    v.sort_by(f64::total_cmp);
    let mut out = serde_json::Map::new();
    for p in ps {
        let i = ((p / 100.0) * (v.len() - 1) as f64).round() as usize;
        out.insert(format!("p{p}"), json!(v[i.min(v.len() - 1)]));
    }
    out.insert("mean".into(), json!(v.iter().sum::<f64>() / v.len() as f64));
    out.insert("n".into(), json!(v.len()));
    serde_json::Value::Object(out)
}

/// `--bench-pipelined`: frames as the window runs them for `--bench-seconds`. The sim ticks on
/// the wall clock (30 Hz, the walk's accumulator and its cap), so a slow frame runs two ticks as
/// on the night; nothing waits for the GPU after a view, and at most two frames are in flight
/// (a window's frame-latency limit; headless there is no vsync, so the frame time is the
/// pipeline's own and a vsync step is read off it afterwards). With `--bench-views` above 1 the
/// views draw through the split window's own `render_views` (each further player's lists, their
/// spawn-screen soldiers), and with `--hud` each view's HUD as the window paints it
/// (`SplitUi`: the BF2 HUD with its minimap and name tags, and the egui text in each cell).
/// One view draws the single-view frame (`ReplayFrame::render`). Each frame's row: its start,
/// main-thread CPU, ticks, tick wall ms, the UI models', the draw and submit, the wait for the
/// frames in flight, and the newest GPU present (`OPENBF_GPU_TIMING=1`). Clocks and power are
/// sampled alongside ([`sensors`]). The sim hash every 30 ticks pins the run.
#[allow(clippy::too_many_arguments)]
fn pipelined(
    v: &ViewerArgs,
    scene: &game::assets::Scene,
    r: &mut Renderer,
    gpu: &mut SceneGpu,
    walk: &mut WalkSim,
    frame: &mut ReplayFrame,
    view: &ViewerArgs,
    seats: &mut SeatDriver,
    report: &mut serde_json::Value,
) -> Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    let n = v.bench_views.max(1);
    let (width, height) = r.size();
    let cells = game::split_layout::cells(n, width, height, Default::default());
    r.time_every_view = r.gpu_timing_on();
    let assets = v.map.parent().unwrap_or_else(|| std::path::Path::new("."));
    let ui = v.hud && n > 1;
    let mut feeds: Vec<game::event_feed::EventFeed> = (1..n)
        .map(|i| {
            walk.with_local(i, |w| {
                game::event_feed::EventFeed::for_walk(assets, &scene.manifest.source_game, w)
            })
        })
        .collect();
    let mut tags: Vec<game::name_tags::NameTags> = (0..n).map(|_| Default::default()).collect();
    let mut previews: Vec<game::spawn_preview::Preview> = Vec::new();
    let mut preview0 = game::spawn_preview::Preview::default();
    let away = vec![false; n];
    let aspect = width as f32 / height.max(1) as f32;
    let name = |slot: u8| Some(format!("Player {}", slot + 1));
    let done = std::sync::Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let sampler = sensors::Sampler::start(start, std::time::Duration::from_millis(250));
    let length = std::time::Duration::from_secs_f32(v.bench_seconds.max(1.0));
    let mut rows = Vec::new();
    let mut hashes = Vec::new();
    let mut tick_ms = Vec::new();
    let mut accum = 0.0f32;
    let mut last = start;
    let mut k: u64 = 0;
    let first_tick = walk.sim.tick;
    // `--bench-governor`: the window's automatic quality, fed as the window feeds it (the last
    // frame's time, its main-thread CPU and its wait for a frame in flight, the headless
    // stand-in for the acquire), held to 60 a second.
    let mut governor = game::quality_governor::Governor::default();
    let mut governed: Option<(f64, f64)> = None;
    let mut scale_changes = Vec::new();
    let map_label = game::quality_governor::map_label(&scene.dir, &scene.manifest.map, v.xl);
    while start.elapsed() < length {
        k += 1;
        let t0 = Instant::now();
        let c0 = cpu_ms();
        let dt = (t0 - last).as_secs_f32();
        last = t0;
        if v.bench_governor {
            if let Some((cpu, wait)) = governed {
                let q = governor.frame(
                    dt * 1000.0,
                    Some(cpu as f32),
                    Some(wait as f32),
                    1000.0 / 60.0,
                    0,
                );
                for note in governor.take_notes() {
                    println!("{}", note.line(&map_label, n));
                }
                if q.scale != r.world_scale() {
                    r.set_world_scale(q.scale);
                    scale_changes.push(json!([(t0 - start).as_secs_f64(), q.scale]));
                }
            }
        }
        // The walk's accumulator (`WalkSim::advance_while`), its cap included.
        accum = (accum + dt).min(core::MAX_FRAME_ACCUM_SECS);
        let (mut ticks, mut ticks_ms, mut seat_ms) = (0u32, 0.0f64, 0.0f64);
        while accum >= core::TICK_DT {
            let ts = Instant::now();
            let pads = seats.pads(walk);
            seat_ms += ms(ts);
            let tt = Instant::now();
            step_world(v, gpu, walk, frame, &pads, &mut feeds);
            let one = ms(tt);
            tick_ms.push(one);
            ticks_ms += one;
            accum -= core::TICK_DT;
            ticks += 1;
            if walk.sim.tick % 30 == 0 {
                hashes.push(json!([walk.sim.tick, format!("{:016x}", walk.sim.hash())]));
            }
        }
        let tu = Instant::now();
        let cameras: Vec<render::Camera> = (0..n).map(|i| walk.camera_of(i)).collect();
        let mut models: Vec<Option<ui::hud::HudModel>> = Vec::new();
        let mut spawn_up: Vec<bool> = Vec::new();
        let mut spawn0 = None;
        if ui {
            models.push(Some(super::super::battle::hud_model_for_view(
                walk,
                &mut frame.feed,
                aspect,
                None,
            )));
            spawn0 = spawn_model(walk, &frame.feed);
            spawn_up.push(spawn0.is_some());
            for (m, up) in super::super::windowed::split::further_view_models(
                walk, &mut feeds, &away, aspect, &[],
            ) {
                models.push(m);
                spawn_up.push(up);
            }
            for (i, cell) in cells.iter().enumerate() {
                let (Some(Some(model)), Some(t)) = (models.get_mut(i), tags.get_mut(i)) else {
                    continue;
                };
                if spawn_up[i] || model.round_end.is_some() {
                    t.clear();
                    continue;
                }
                model.name_tags = game::name_tags::cell_tags(
                    t,
                    &walk.sim,
                    walk.local_unit(i),
                    &cameras[i],
                    *cell,
                    dt,
                    &name,
                );
            }
        }
        let ui_ms = ms(tu);
        // Two frames in flight: this one's draw waits for the one before last.
        let tw = Instant::now();
        while done.load(Ordering::Acquire) + 2 < k {
            let _ = r.device().poll(wgpu::PollType::Poll);
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        let wait_ms = ms(tw);
        let td = Instant::now();
        let mut build0_ms = 0.0;
        if n == 1 {
            frame.render(r, gpu, walk, view, dt)?;
        } else {
            let tb = Instant::now();
            let cam0 = cameras[0];
            let (mut draws, mut skinned) =
                frame.build_view(r, gpu, walk, &cam0, false, (cells[0][2], cells[0][3]))?;
            if ui {
                // View 0's spawn-screen soldiers, in its UI rectangle (as the window's).
                let page = spawn0.as_ref().and_then(|s| s.page.as_ref());
                let (fov_y, ui_aspect) = game::split_layout::ui_view(cam0.fov_y, cells[0]);
                preview0.set_view_aspect(Some(ui_aspect));
                let deka = &mut frame.deka_vis;
                let sim = &walk.sim;
                let (body, gun) = preview0.draws_live(
                    r,
                    &frame.vis,
                    page,
                    &render::Camera { fov_y, ..cam0 },
                    dt,
                    |r, team, class, model, dt| {
                        let c = sim
                            .classes
                            .get(usize::from(team))?
                            .get(usize::from(class))?
                            .droideka
                            .as_deref()?;
                        Some(deka.preview_draws(r, team, class, c, model, dt))
                    },
                );
                skinned.extend(body);
                draws.extend(gun);
            }
            build0_ms = ms(tb);
            let pages: Vec<Option<ui::spawn_draw::SpawnPage>> = models
                .iter()
                .map(|m| m.as_ref().and_then(|m| m.spawn.clone()))
                .collect();
            let split_ui = game::split_ui::SplitUi {
                models: &models,
                icons: &[],
                dt,
                hidden: false,
                pause: None,
                window: [width, height],
                views: n,
                hud_k: 1.0,
                lost: &[],
            };
            let clear = gpu.clear;
            let ReplayFrame {
                cube,
                quad,
                sprite,
                icons,
                vis,
                vehicle_vis,
                deka_vis,
                fx,
                bolts,
                weather,
                holos,
                overlay,
                hud_view,
                ..
            } = &mut *frame;
            let paint = match (ui, overlay.as_mut()) {
                (true, Some(o)) => Some(o.begin_headless([width, height], 1.0, |ctx| {
                    for (i, cell) in cells.iter().enumerate() {
                        if let Some(Some(m)) = models.get(i) {
                            let t = game::name_tags::cell_text(ctx, *cell, 1.0);
                            ui::hud::draw_hud_in(ctx, m, t.rect, i + 1, t.scale, t.timer_at);
                        }
                    }
                })),
                _ => None,
            };
            let mut parts = super::super::windowed::split::ViewParts {
                walk: &mut *walk,
                cube: *cube,
                gpu: &mut *gpu,
                soldier_vis: Some(vis),
                vehicle_vis,
                deka_vis,
                fx,
                holos,
                weather: weather.as_mut(),
                bolts,
                fx_quad: Some(*quad),
                fx_sprite: *sprite,
                squad_icons: *icons,
                previews: &mut previews,
                pages: &pages,
                preview_dt: dt,
                lan_view: None,
            };
            let mut last_overlay = |dev: &wgpu::Device,
                                    q: &wgpu::Queue,
                                    enc: &mut wgpu::CommandEncoder,
                                    tv: &wgpu::TextureView,
                                    _size: [u32; 2]| {
                if let (Some(o), Some(p)) = (overlay.as_mut(), paint.as_ref()) {
                    o.paint(p, dev, q, enc, tv, [width, height]);
                }
            };
            let mut view_hud = |i: usize,
                                cell: game::split_layout::Cell,
                                dev: &wgpu::Device,
                                q: &wgpu::Queue,
                                enc: &mut wgpu::CommandEncoder,
                                tv: &wgpu::TextureView| {
                if let (true, Some(hv)) = (ui, hud_view.as_mut()) {
                    split_ui.paint(hv, i, cell, dev, q, enc, tv);
                }
            };
            super::super::windowed::split::render_views(
                &mut parts,
                r,
                &cells,
                (&draws, &skinned),
                &cameras,
                &[],
                &away,
                clear,
                Some(&mut last_overlay),
                Some(&mut view_hud),
            )
            .map_err(|e| anyhow!("render: {e}"))?;
        }
        let draw_ms = ms(td);
        let d = done.clone();
        r.queue().on_submitted_work_done(move || {
            d.fetch_max(k, Ordering::AcqRel);
        });
        r.collect_gpu_timing();
        let gpu_ms: f64 = r.gpu_sections().iter().map(|s| s.1).sum();
        let cpu = c0.zip(cpu_ms()).map(|(a, b)| b - a);
        governed = cpu.map(|c| (c, wait_ms));
        rows.push(json!({
            "t_ms": (t0 - start).as_secs_f64() * 1000.0, "cpu_ms": cpu, "ticks": ticks,
            "tick_ms": ticks_ms, "seat_ms": seat_ms, "ui_ms": ui_ms, "build0_ms": build0_ms,
            "draw_ms": draw_ms, "wait_ms": wait_ms, "gpu_ms": gpu_ms,
            "draw_calls": r.frame_stats().draw_calls, "world_scale": r.world_scale(),
        }));
    }
    r.wait_idle().map_err(|e| anyhow!("{e}"))?;
    let sensors = sampler.finish();
    // Frame time: from one frame's start to the next's.
    let starts: Vec<f64> = rows.iter().filter_map(|f| f["t_ms"].as_f64()).collect();
    let frame_ms: Vec<f64> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    let col = |key: &str| -> Vec<f64> { rows.iter().filter_map(|f| f[key].as_f64()).collect() };
    let ps = [50.0, 95.0, 99.0];
    let share = |limit: f64| {
        frame_ms.iter().filter(|&&f| f <= limit).count() as f64 / frame_ms.len().max(1) as f64
    };
    let sensor_col = |key: &str| -> Vec<f64> {
        sensors.iter().filter_map(|s| s[key].as_f64()).collect()
    };
    let metric_col = |key: &str| -> Vec<f64> {
        sensors
            .iter()
            .filter_map(|s| s["metrics"][key].as_f64())
            .collect()
    };
    let summary = json!({
        "seconds": start.elapsed().as_secs_f64(), "frames": rows.len(),
        "ticks": walk.sim.tick - first_tick,
        "frame_ms": pct(&frame_ms, &ps), "cpu_ms": pct(&col("cpu_ms"), &ps),
        "tick_ms": pct(&tick_ms, &ps), "frame_tick_ms": pct(&col("tick_ms"), &ps),
        // The seats' own pads (`ai::think` for each seat): main-thread work inside `cpu_ms` that
        // a window with people at the pads does not do.
        "seat_ms": pct(&col("seat_ms"), &ps),
        "ui_ms": pct(&col("ui_ms"), &ps), "draw_ms": pct(&col("draw_ms"), &ps),
        "wait_ms": pct(&col("wait_ms"), &ps), "gpu_ms": pct(&col("gpu_ms"), &ps),
        "share_within_16_7": share(1000.0 / 60.0), "share_within_33_3": share(1000.0 / 30.0),
        "cpu_mhz": pct(&sensor_col("cpu_mhz_avg"), &ps),
        "main_cpu_mhz": pct(&sensor_col("main_cpu_mhz"), &ps),
        "gpu_sclk_mhz": pct(&sensor_col("gpu_sclk_mhz"), &ps),
        "gpu_power_w": pct(&sensor_col("gpu_power_w"), &ps),
        "socket_w": pct(&metric_col("socket_w"), &ps),
        "tctl_c": pct(&sensor_col("tctl_c"), &ps),
        "gpu_busy_pct": pct(&sensor_col("gpu_busy_pct"), &ps),
    });
    let mut summary = summary;
    if v.bench_governor {
        // Each scale's frames (frame time from its start to the next's), and where it settled.
        let mut per_scale: Vec<(f64, Vec<f64>)> = Vec::new();
        for (row, ms) in rows.iter().zip(&frame_ms) {
            let s = row["world_scale"].as_f64().unwrap_or(1.0);
            match per_scale.iter_mut().find(|p| p.0 == s) {
                Some(p) => p.1.push(*ms),
                None => per_scale.push((s, vec![*ms])),
            }
        }
        summary["governor"] = json!({
            "changes": scale_changes,
            "final_scale": r.world_scale(),
            "per_scale": per_scale.iter().map(|(s, f)| json!({"scale": s, "frame_ms": pct(f, &ps)})).collect::<Vec<_>>(),
        });
    }
    println!("pipelined: {}", serde_json::to_string(&summary)?);
    report["views"] = json!(n);
    report["pipelined"] = summary;
    report["pipelined_frames"] = json!(rows);
    report["pipelined_tick_hashes"] = json!(hashes);
    report["sensors"] = json!(sensors);
    report["think_threads"] = json!(core::sim::think_pool::default_think_threads());
    report["final_hash"] = json!(format!("{:016x}", walk.sim.hash()));
    report["alive_end"] = json!(alive(walk));
    if let Some(path) = &v.capture_frame {
        let (rgba, w, h) = r.capture_rgba()?;
        image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8)?;
    }
    render::made::enable(false);
    Ok(())
}
