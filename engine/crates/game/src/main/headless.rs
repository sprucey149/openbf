//! The offscreen modes: [`run_bench`] (`--bench`: load, tick and frame cost
//! on one adapter) and [`run_headless`] (`--headless`: one captured frame, a
//! `--walk-replay` with its PNG series, `motion.csv` and events CSV — the sim
//! gates run this — a `--capture-orbit` flythrough, or the `--scene-report`
//! census). `--aim-replay` runs the scripted acquisition referee without a device.

use super::battle::{
    hud_model, load_and_upload, load_hud_view, make_walk, spawn_model, squad_icons, walk_draws,
};
use super::{BattleSetup, ViewerArgs};
use anyhow::{anyhow, Context, Result};
use core::sim::InputLog;
use game::viewer::{camera_from, upload_scene, upload_unit_cube, SceneGpu};
use game::walk::WalkSim;
use render::{Camera, DrawCmd, Renderer};

#[path = "headless/detailed_bench.rs"]
mod detailed_bench;

/// Keep feed statistics current even without a HUD consumer. Presentation
/// output has no reader in that mode, so discard it after each sim tick.
fn tick_event_feed(
    feed: &mut game::event_feed::EventFeed,
    sim: &core::sim::Sim,
    player: core::unit::UnitHandle,
    hud: bool,
) {
    feed.tick(sim, player);
    if !hud {
        drop(feed.take());
        drop(feed.take_ambush());
    }
}

/// Headless frame with the HUD painted on top (captures / goldens).
#[allow(clippy::too_many_arguments)]
fn render_headless_hud(
    r: &mut Renderer,
    overlay: &mut ui::overlay::EguiOverlay,
    hud_view: &mut Option<game::hud_feed::HudView>,
    dt: f32,
    camera: &Camera,
    draws: &[DrawCmd],
    skinned: &[render::SkinnedDrawCmd],
    model: &ui::hud::HudModel,
    spawn: Option<&ui::spawn::SpawnModel>,
    clear: [f64; 4],
) -> Result<()> {
    let (w, h) = r.size();
    // BF2's own HUD first, the egui lines and the spawn screen over it.
    let mut hud_view = hud_view.as_mut();
    if let Some(hv) = hud_view.as_mut() {
        hv.update(model, dt, [w, h]);
    }
    let paint = overlay.begin_headless([w, h], 1.0, |ctx| {
        // The spawn screen owns the death message when it is up.
        let hud = match spawn {
            Some(_) => ui::hud::HudModel {
                dead: false,
                ..model.clone()
            },
            None => model.clone(),
        };
        ui::hud::draw_hud(ctx, &hud);
        if let Some(s) = spawn {
            // The PS2's screen is the HUD pass's; without one the panel
            // stands in.
            match hud_view.is_some() {
                true => ui::spawn::draw_spawn(ctx, s),
                false => ui::spawn::draw_spawn(
                    ctx,
                    &ui::spawn::SpawnModel {
                        page: None,
                        ..s.clone()
                    },
                ),
            }
        }
    });
    r.render_frame(
        camera,
        draws,
        skinned,
        clear,
        Some(&mut |dev, q, enc, view, size| {
            if let Some(hv) = hud_view.as_mut() {
                hv.paint(dev, q, enc, view, size);
            }
            overlay.paint(&paint, dev, q, enc, view, size)
        }),
    )
    .map_err(|e| anyhow!("render: {e}"))
}

/// Resources shared by series captures and the always-rendered final replay frame.
struct ReplayFrame {
    cube: render::MeshHandle,
    models: [bool; 2],
    quad: render::MeshHandle,
    sprite: Option<render::TextureHandle>,
    icons: (Option<render::TextureHandle>, Option<render::TextureHandle>),
    vis: game::soldier_vis::SoldierVisuals,
    vehicle_vis: game::viewer::VehicleVisuals,
    deka_vis: game::viewer::DroidekaVisuals,
    fx: game::fx::Fx,
    bolts: game::bolt_vis::BoltVis,
    weather: Option<game::weather::Weather>,
    holos: game::holo::Holos,
    overlay: Option<ui::overlay::EguiOverlay>,
    hud_view: Option<game::hud_feed::HudView>,
    feed: game::event_feed::EventFeed,
}

impl ReplayFrame {
    fn render(
        &mut self,
        r: &mut Renderer,
        gpu: &mut SceneGpu,
        walk: &WalkSim,
        v: &ViewerArgs,
        dt: f32,
    ) -> Result<()> {
        let cam = if v.cam_set {
            camera_from(v.cam)
        } else {
            walk.camera()
        };
        // Replay presentation uses elapsed sim time, independent of capture cadence.
        r.time = (walk.sim.tick as f32 * core::TICK_DT) % render::TIME_WRAP;
        r.water_time = walk.sim.tick as f64 / f64::from(core::TICK_HZ);
        gpu.animate_water_live(r, cam.position)?;
        game::forest::submit_actors(r, &walk.sim);
        gpu.pose_doors(&walk.sim.doors);
        gpu.pose_turrets(&walk.sim, &walk.vehicle_seats);
        gpu.pose_stations(&walk.sim);
        // A set viewer camera is not the chase camera: it sees through
        // nothing.
        let see = if v.cam_set {
            game::camera_collide::SeeThrough::default()
        } else {
            walk.camera_see_through()
        };
        gpu.see_through(&see.props);
        let mut draws = gpu.view_draws(&walk.sim, walk.player);
        walk_draws(self.cube, walk, &mut draws, self.models);
        draws.extend(self.vehicle_vis.draws(r, &walk.sim, &see.vehicles));
        draws.extend(self.deka_vis.draws(r, &walk.sim));
        let mut skinned = self.vis.draws(walk);
        draws.extend(self.vis.weapon_draws(walk));
        draws.extend(self.vis.cloth_draws(r, walk));
        if !v.cam_set {
            let (arms, gun) = self.vis.first_person_draws(walk, &cam);
            skinned.extend(arms);
            draws.extend(gun);
        }
        draws.extend(self.fx.draws(self.quad, self.sprite, &cam));
        self.holos.advance(walk);
        game::point_light::submit(r, gpu, walk, &mut self.bolts, cam.position, &self.holos);
        let mut points = self.vis.fire_points(walk);
        self.deka_vis.fire_points(&walk.sim, &mut points);
        let mut particles = self.vis.flash_draws_at(walk, &points);
        particles.extend(self.vis.saber_draws(walk));
        particles.extend(self.deka_vis.bubbles(&walk.sim, &cam, r.lighting.sun_dir));
        particles.extend(self.holos.draws(&gpu.holo, walk, &cam));
        if let Some(w) = self.weather.as_mut() {
            particles.extend(w.frame_draws(r, &cam, glam::Vec3::ZERO));
        }
        self.fx.submit_with(r, &cam, particles);
        draws.extend(game::squad_vis::squad_draws(
            &walk.sim,
            0,
            self.quad,
            self.icons.0,
            self.icons.1,
            &cam,
        ));
        match &mut self.overlay {
            Some(o) => {
                let model = hud_model(walk, &mut self.feed);
                let spawn = spawn_model(walk, &self.feed);
                render_headless_hud(
                    r,
                    o,
                    &mut self.hud_view,
                    dt,
                    &cam,
                    &draws,
                    &skinned,
                    &model,
                    spawn.as_ref(),
                    gpu.clear,
                )?
            }
            None => r
                .render_frame(&cam, &draws, &skinned, gpu.clear, None)
                .map_err(|e| anyhow!("render: {e}"))?,
        }
        Ok(())
    }
}

/// One CSV line per sim event (tick,kind,a,b,value).
fn append_events(csv: &mut String, tick: usize, events: &[core::sim::Event]) {
    use core::sim::Event;
    for e in events {
        let line = match e {
            Event::ForceHit {
                owner,
                weapon_class,
                blocked,
                ..
            } => format!(
                "{tick},force_hit,{},{weapon_class},{}\n",
                owner.index,
                u8::from(*blocked)
            ),
            Event::WeaponBlast { pos, weapon_class } => format!(
                "{tick},weapon_blast,{weapon_class},,{:.2}:{:.2}:{:.2}\n",
                pos.x, pos.y, pos.z
            ),
            Event::ComboSound {
                unit,
                weapon_class,
                sound_hash,
            } => format!(
                "{tick},combo_sound,{},{weapon_class},{sound_hash:08x}\n",
                unit.index
            ),
            Event::SaberContact {
                pos,
                weapon_class,
                deflect,
            } => format!(
                "{tick},saber_contact,{weapon_class},{},{:.2}:{:.2}:{:.2}\n",
                u8::from(*deflect),
                pos.x,
                pos.y,
                pos.z
            ),
            Event::SaberThrowHit { pos, weapon_class } => format!(
                "{tick},saber_throw_hit,{weapon_class},,{:.2}:{:.2}:{:.2}\n",
                pos.x, pos.y, pos.z
            ),
            Event::HeroOffered { unit, class } => {
                format!("{tick},hero_offer,{},{class},\n", unit.index)
            }
            Event::MineBlast {
                pos,
                weapon_class,
                cause,
            } => format!(
                "{tick},mine_blast,{weapon_class},{},{:.2}:{:.2}:{:.2}\n",
                *cause as u8, pos.x, pos.y, pos.z
            ),
            Event::MineTick { pos, weapon_class } => {
                format!(
                    "{tick},mine_tick,{weapon_class},,{:.2}:{:.2}:{:.2}\n",
                    pos.x, pos.y, pos.z
                )
            }
            Event::GuidedGone {
                pos,
                weapon_class,
                end,
            } => format!(
                "{tick},guided_gone,{weapon_class},{end:?},{:.2}:{:.2}:{:.2}\n",
                pos.x, pos.y, pos.z
            ),
            Event::DroidCharge { pos, weapon_class } => format!(
                "{tick},droid_charge,{weapon_class},,{:.2}:{:.2}:{:.2}\n",
                pos.x, pos.y, pos.z
            ),
            Event::DroidGone {
                pos,
                weapon_class,
                end,
            } => format!(
                "{tick},droid_gone,{weapon_class},{end:?},{:.2}:{:.2}:{:.2}\n",
                pos.x, pos.y, pos.z
            ),
            Event::Cloak {
                unit,
                weapon_class,
                pos,
            } => format!(
                "{tick},cloak,{},{weapon_class},{:.2}:{:.2}:{:.2}\n",
                unit.index, pos.x, pos.y, pos.z
            ),
            Event::TurretGone {
                pos,
                weapon_class,
                end,
            } => format!(
                "{tick},turret_gone,{weapon_class},{end:?},{:.2}:{:.2}:{:.2}\n",
                pos.x, pos.y, pos.z
            ),
            Event::TurretHit {
                shooter,
                turret,
                damage,
                health_left,
                ..
            } => format!(
                "{tick},turret_hit,{},{turret},{damage:.2},{health_left:.2}\n",
                shooter.index
            ),
            Event::Ability {
                unit,
                weapon_class,
                pos,
                ..
            } => format!(
                "{tick},ability,{},{weapon_class},{:.2}:{:.2}:{:.2}\n",
                unit.index, pos.x, pos.y, pos.z
            ),
            Event::Repair {
                unit,
                amount,
                target,
                ..
            } => {
                let target = match target {
                    core::projectile::Target::Unit(h) => format!("u{}g{}", h.index, h.gen),
                    core::projectile::Target::Vehicle(h) => format!("v{}g{}", h.index, h.gen),
                    core::projectile::Target::Station(i) => format!("s{i}"),
                    core::projectile::Target::Mine(i) => format!("m{i}"),
                    core::projectile::Target::Droid(i) => format!("d{i}"),
                    core::projectile::Target::Turret(i) => format!("t{i}"),
                    core::projectile::Target::Guided(i) => format!("g{i}"),
                    core::projectile::Target::Building(i) => format!("b{i}"),
                    core::projectile::Target::Shield(h) => format!("z{}g{}", h.index, h.gen),
                };
                format!("{tick},repair,{},{target},{amount}\n", unit.index)
            }
            Event::Shot {
                unit, weapon_class, ..
            } => format!("{tick},shot,{},{weapon_class},\n", unit.index),
            // An emitter's arc; `game::fx` draws it and the `hit` lines
            // beside it carry the damage.
            Event::Emitter { .. } => continue,
            Event::Hit {
                shooter,
                victim,
                damage,
                age,
                ..
            } => format!(
                "{tick},hit,{},{},{damage:.1},{age:.3}\n",
                shooter.index, victim.index
            ),
            Event::Kill { shooter, victim } => {
                format!("{tick},kill,{},{},\n", shooter.index, victim.index)
            }
            Event::BoltExpired { struck, .. } => format!("{tick},expire,,,{}\n", *struck as u8),
            // Render-only, and the expire / hit line already records it.
            Event::Impact { .. } => continue,
            // HUD-only (the reticle mark); the hit line records the hit.
            Event::CriticalHit { .. } => continue,
            Event::Despawn { unit } => format!("{tick},despawn,{},,\n", unit.index),
            Event::Respawn { unit } => format!("{tick},respawn,{},,\n", unit.index),
            Event::Reload { unit, .. } => format!("{tick},reload,{},,\n", unit.index),
            Event::Throw { unit, .. } => format!("{tick},throw,{},,\n", unit.index),
            Event::WeaponSelect { unit, weapon_class } => {
                format!("{tick},weapon_select,{},{weapon_class},\n", unit.index)
            }
            Event::FireEmpty { unit } => format!("{tick},fire_empty,{},,\n", unit.index),
            Event::StickyContact { water, .. } => {
                format!("{tick},grenade_contact,,,{}\n", *water as u8)
            }
            Event::StickyBlast { .. } => format!("{tick},grenade_blast,,,\n"),
            // Which slot ordered what, how many bots it reached and which
            // took it — so a scripted input log can PROVE it did what it says.
            Event::Command { slot, kind, units } => {
                format!("{tick},command,{slot},{kind},{units}\n")
            }
            Event::OrderTaken { unit, slot, .. } => {
                format!("{tick},order_taken,{},{slot},\n", unit.index)
            }
            Event::Call { unit, call } => format!("{tick},call,{},{call:?},\n", unit.index),
            Event::Captured { cp, team } => format!("{tick},captured,{cp},{team},\n"),
            Event::Neutralized { cp, by } => format!("{tick},neutralized,{cp},{by},\n"),
            Event::Saved { cp, team } => format!("{tick},saved,{cp},{team},\n"),
            Event::TeamDefeated { team } => format!("{tick},defeated,{team},,\n"),
            Event::TimeUp { winner } => format!("{tick},time_up,{winner},,\n"),
            Event::EnvDeath { unit, cause } => {
                let what = match cause {
                    core::sim::DeathCause::Suicide => "suicide",
                    core::sim::DeathCause::HeroExhausted => "hero_exhausted",
                    core::sim::DeathCause::DeathRegion => "region",
                    core::sim::DeathCause::Fall => "fall",
                    core::sim::DeathCause::VehicleDestroyed => "vehicle",
                    core::sim::DeathCause::Blast => "blast",
                    core::sim::DeathCause::KillFloor => "floor",
                    core::sim::DeathCause::UnderMap => "under",
                    core::sim::DeathCause::Lost => "lost",
                    core::sim::DeathCause::Stalled => "stalled",
                    core::sim::DeathCause::Water => "water",
                };
                format!("{tick},envdeath,{},,{what}\n", unit.index)
            }
            Event::Relocated { unit, to, .. } => format!(
                "{tick},relocated,{},,{:.2}:{:.2}:{:.2}\n",
                unit.index, to.x, to.y, to.z
            ),
            Event::VehicleHit {
                shooter,
                vehicle,
                damage,
                ..
            } => format!(
                "{tick},vehicle_hit,{},{},{damage:.1}\n",
                shooter.map_or(String::new(), |s| s.index.to_string()),
                vehicle.index
            ),
            Event::StationDestroyed { station, class, .. } => {
                format!("{tick},station_destroyed,{station},{class},\n")
            }
            Event::VehicleDestroyed {
                vehicle, killer, ..
            } => format!(
                "{tick},vehicle_destroyed,{},{},\n",
                killer.map_or(String::new(), |s| s.index.to_string()),
                vehicle.index
            ),
            // `a` the droid, `b` its team, the value the chunks' scale.
            Event::DroidBurst {
                unit, team, scale, ..
            } => format!("{tick},droid_burst,{},{team},{scale:.3}\n", unit.index),
            Event::Droideka { unit, cue, .. } => {
                format!("{tick},droideka,{},,{cue:?}\n", unit.index)
            }
            Event::DoorOpened { door } => format!("{tick},door_opened,{door},,\n"),
            Event::DoorClosed { door } => format!("{tick},door_closed,{door},,\n"),
            // `a` is the item class; a taken item's `b` is the taker and its
            // value what it gave (1 health, 2 ammo, 4 energy).
            Event::ItemDropped { item, .. } => format!("{tick},item_dropped,{item},,\n"),
            Event::ItemLanded { item, .. } => format!("{tick},item_landed,{item},,\n"),
            Event::ItemTaken {
                unit,
                item,
                health,
                ammo,
                energy,
                ..
            } => format!(
                "{tick},item_taken,{item},{},{}\n",
                unit.index,
                u8::from(*health) | u8::from(*ammo) << 1 | u8::from(*energy) << 2
            ),
            // `a` is the station; `b` the unit served (a vehicle as `v<n>`)
            // and its value what it gave (1 health, 2 ammo).
            Event::StationServed {
                station,
                unit,
                vehicle,
                health,
                ammo,
                ..
            } => format!(
                "{tick},station_served,{station},{},{}
",
                unit.map(|u| u.index.to_string())
                    .or_else(|| vehicle.map(|v| format!("v{}", v.index)))
                    .unwrap_or_default(),
                u8::from(*health) | u8::from(*ammo) << 1
            ),
        };
        csv.push_str(&line);
    }
}

/// `--bench`: load the map, run the sim, render frames, print the numbers.
///
/// Deliberately measures the SAME code paths the game uses — `load_and_upload`,
/// `WalkSim::step_tick`, `Renderer::render_frame` with the real draw list — so
/// an improvement here is an improvement in the game rather than in a harness.
/// The sim is driven by a fixed idle pad: this measures the machine, and a
/// pad-script would make runs incomparable between maps.
pub(super) fn run_bench(v: &ViewerArgs) -> Result<()> {
    if let Some(path) = std::env::var_os("OPENBF_BENCH_DETAIL") {
        return detailed_bench::run(v, std::path::Path::new(&path));
    }
    use std::time::Instant;

    let mut r = Renderer::new_headless(v.size.0, v.size.1).map_err(|e| anyhow!("renderer: {e}"))?;
    r.cull = !v.no_cull;
    let adapter = {
        let i = r.adapter_info();
        format!("{} ({:?}, {:?})", i.name, i.device_type, i.backend)
    };

    // Split load from upload: they have different fixes (parsing vs GPU
    // residency), so one number for both would hide which is the problem.
    let t = Instant::now();
    let (scene, load) = game::assets::load_battle_scene(std::path::Path::new(&v.map), &v.mode)?;
    // A common set that exists but will not load must say so: benching the
    // map without its shared props would read as a speed-up.
    if let Some((cd, Err(e))) = &load.common {
        eprintln!("common assets at {} unreadable: {e:#}", cd.display());
    }
    let scene_load = t.elapsed();

    let t = Instant::now();
    let mut gpu = upload_scene(&mut r, &scene)?;
    let gpu_upload = t.elapsed();

    let stats = game::bench::SceneStats::of(&scene, gpu.draws.len());

    // Sim: a real conquest round on this map (use --ai to populate it), driven
    // by an idle pad.
    let mut walk = make_walk(&scene, &BattleSetup::from_args(v, v.seed)?);
    let assets_dir = std::path::Path::new(&v.map)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let mut vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &assets_dir, &scene, &walk);
    let models = [vis.has_model(0), vis.has_model(1)];
    let mut vehicle_vis =
        game::viewer::VehicleVisuals::load(&mut r, &scene, &assets_dir, &walk.vehicle_geometry);
    let mut deka_vis = game::viewer::DroidekaVisuals::load(&mut r, &scene, &assets_dir, &walk);
    let cube = upload_unit_cube(&mut r)?;
    let mut tick = game::bench::Samples::default();
    let pad = [core::sim::PadState::default()];
    gpu.step_stations(&walk.sim);
    for _ in 0..v.bench_ticks {
        let t = Instant::now();
        walk.step_tick(&pad);
        gpu.step_stations(&walk.sim);
        vis.update(&walk, core::TICK_DT);
        tick.push(t.elapsed());
    }

    // Frames: the world draw list PLUS the battle's soldiers, because a frame
    // without them is not a frame the game ever renders — and the skinned path
    // (a 96-joint palette per draw) is the part most likely to cost.
    let mut frame = game::bench::Samples::default();
    let mut gpu_sections = game::bench::Sections::default();
    let mut battle_draws = 0usize;
    let mut skinned_draws = 0usize;
    if v.bench_frames > 0 {
        let cam = bench_camera(v, &scene, &walk);
        // The water moves (queue OC): step it once so the draw list has the
        // ocean's patches, then every frame below.
        gpu.animate_water(&mut r, game::water_anim::clock(0.0), cam.position)?;
        gpu.pose_doors(&walk.sim.doors);
        gpu.pose_turrets(&walk.sim, &walk.vehicle_seats);
        gpu.pose_stations(&walk.sim);
        let mut draws = gpu.draws.clone();
        walk_draws(cube, &walk, &mut draws, models);
        draws.extend(vehicle_vis.draws(&mut r, &walk.sim, &[]));
        draws.extend(deka_vis.draws(&mut r, &walk.sim));
        let skinned = vis.draws(&walk);
        draws.extend(vis.cloth_draws(&mut r, &walk));
        battle_draws = draws.len();
        skinned_draws = skinned.len();
        // One warm-up frame: the first submit pays for pipeline warm-up and
        // first-touch of every buffer, and folding that into the mean would
        // quietly flatter a longer run.
        r.render_frame(&cam, &draws, &skinned, gpu.clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        r.wait_idle().map_err(|e| anyhow!("wait: {e}"))?;
        for k in 0..v.bench_frames {
            let t = Instant::now();
            let clock = game::water_anim::clock(k as f64 / 60.0);
            gpu.animate_water(&mut r, clock, cam.position)?;
            // A fixed eye keeps the same cells, so only the water's own
            // draws change: copy them over rather than re-cloning the list.
            if let Some(w) = gpu.water_motion.as_ref().map(|m| m.draw_range()) {
                draws[w.clone()].copy_from_slice(&gpu.draws[w]);
            }
            r.render_frame(&cam, &draws, &skinned, gpu.clear, None)
                .map_err(|e| anyhow!("render: {e}"))?;
            // render_frame returns once the queue is SUBMITTED, so without a
            // wait this would time command encoding and nothing else. Wait on
            // the device rather than reading pixels back: a 1280x720 readback
            // is ~3.7 MB of copy the real game never performs, and folding it
            // into the frame time measures the harness instead of the render.
            r.wait_idle().map_err(|e| anyhow!("wait: {e}"))?;
            frame.push(t.elapsed());
            // `OPENBF_GPU_TIMING=1`: the newest frame the timer has read back.
            gpu_sections.add(r.gpu_sections());
        }
    }

    let mut stats = stats;
    stats.battle_draws = battle_draws;
    stats.skinned_draws = skinned_draws;
    let units_alive = walk.sim.units.iter().filter(|u| u.alive).count();
    let report = game::bench::Report {
        // The DIRECTORY name, not manifest.map: both discs ship worlds called
        // hoth/end1/yavin1, so the world name is not a map identity.
        map: v
            .map
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| scene.manifest.map.clone()),
        adapter,
        units_alive,
        frame_stats: r.frame_stats(),
        size: v.size,
        scene_load,
        gpu_upload,
        stats,
        tick,
        frame,
        gpu: gpu_sections,
    };
    report.print();
    if let Some(p) = &v.bench_csv {
        let fresh = !p.exists();
        let mut out = if fresh {
            String::from(game::bench::Report::CSV_HEADER)
        } else {
            std::fs::read_to_string(p)?
        };
        out.push_str(&report.csv_row());
        std::fs::write(p, out)?;
    }
    Ok(())
}

/// Camera for a bench run: the map's own overview, so every map is framed the
/// same way and the frame time reflects how much of the map is on screen.
fn bench_camera(v: &ViewerArgs, scene: &game::assets::Scene, walk: &WalkSim) -> render::Camera {
    if v.cam_set {
        camera_from(v.cam)
    } else if v.bench_player_cam {
        walk.camera()
    } else {
        game::viewer::overview_camera(scene)
    }
}

pub(super) fn run_headless(v: &ViewerArgs) -> Result<()> {
    let mut r = Renderer::new_headless(v.size.0, v.size.1).map_err(|e| anyhow!("renderer: {e}"))?;
    let (mut gpu, scene) = load_and_upload(&mut r, &v.map, &v.mode)?;

    if v.scene_report {
        return scene_report(&scene, &gpu, &v.map);
    }

    let ride_log = match &v.ride_script {
        Some(out) => Some(ride_script(&scene, v, out)?),
        None => None,
    };
    if let Some(log_path) = ride_log.as_ref().or(v.walk_replay.as_ref()) {
        let log: InputLog =
            ron::from_str(&std::fs::read_to_string(log_path)?).context("parse walk log")?;
        let cube = upload_unit_cube(&mut r)?;
        let replay_setup = BattleSetup::from_args(v, log.seed)?;
        if let Some(mission) = replay_setup.mission.as_ref() {
            game::forest::configure(&mut r, &scene, Some(mission))?;
        }
        let mut walk = make_walk(&scene, &replay_setup);
        let assets_dir = std::path::Path::new(&v.map)
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &assets_dir, &scene, &walk);
        let models = [vis.has_model(0), vis.has_model(1)];
        let vehicle_vis =
            game::viewer::VehicleVisuals::load(&mut r, &scene, &assets_dir, &walk.vehicle_geometry);
        let deka_vis = game::viewer::DroidekaVisuals::load(&mut r, &scene, &assets_dir, &walk);
        let quad = game::viewer::upload_billboard_quad(&mut r)?;
        let sprite = game::viewer::upload_fx_sprite(&mut r).ok();
        let icons = squad_icons(&mut r, &assets_dir);
        let mut fx =
            game::fx::Fx::for_walk(&assets_dir, &scene.dir, &scene.manifest.source_game, &walk);
        fx.set_water(&scene.manifest);
        fx.set_scene_effects(&scene);
        let bolts = game::bolt_vis::BoltVis::new(&assets_dir, &scene.manifest.source_game);
        // The map's precipitation (card WTHR), on the sim's tick here.
        let weather = game::weather::Weather::load(
            &scene.manifest,
            &assets_dir,
            &scene.dir,
            game::weather::STILL_SEED,
        );
        let holos = game::holo::Holos::default();
        let overlay = v
            .hud
            .then(|| ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT));
        let hud_view = v.hud.then(|| load_hud_view(&assets_dir, &r)).flatten();
        // Sim tick of the last HUD frame: the fades run on the sim's clock.
        let mut hud_tick: Option<u64> = None;
        // The event lines and statistics, as the window keeps them.
        let feed =
            game::event_feed::EventFeed::for_walk(&assets_dir, &scene.manifest.source_game, &walk);
        let mut csv = String::from(
            "tick,x,y,z,speed,stance,energy,yaw,magnet,shots,hits,kills,health,bolts,enemies,tickets0,tickets1,alive0,alive1,cps0,cps1,outcome\n",
        );
        let mut events_csv = String::from("tick,kind,a,b,value,extra\n");
        let trace_squad = std::env::var_os("SQUAD_TRACE").is_some();
        let mut hits = 0u32;
        // Dev referee: SOUND_LOG=<csv> lists every sound the ticks start
        // (`game::event_sounds::SoundLog`).
        let sound_log_path = std::env::var_os("SOUND_LOG");
        let mut sound_log = sound_log_path.as_ref().map(|_| {
            let mut sl = game::event_sounds::SoundLog::new(&scene.dir, &walk);
            let classes = BattleSetup::from_args(v, v.seed).map(|b| b.classes_dir);
            sl.ambience = game::ambience::Ambience::load(
                &scene.manifest,
                &scene.dir,
                &classes.unwrap_or_else(|_| assets_dir.join("classes")),
            );
            sl
        });
        let mut frame = ReplayFrame {
            cube,
            models,
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
            feed,
        };
        gpu.step_stations(&walk.sim);
        for (tick, pads) in log.frames().enumerate() {
            game::heroes::scores(&mut walk.sim, &frame.feed);
            walk.step_tick(pads);
            gpu.step_stations(&walk.sim);
            frame.fx.ingest_posed(&walk, Some(&frame.vis));
            let effect_sounds = frame.fx.take_sounds();
            if let Some(sl) = sound_log.as_mut() {
                sl.tick(tick, &walk, &effect_sounds);
                sl.voice(tick, &mut walk);
            }
            frame.vis.update(&walk, core::TICK_DT);
            let mut fire_points = frame.vis.fire_points(&walk);
            frame.deka_vis.fire_points(&walk.sim, &mut fire_points);
            frame.fx.set_repair_origins(&fire_points);
            frame.bolts.set_fire_points(fire_points);
            frame.fx.set_jet_frames(frame.vis.jet_frames(&walk));
            frame.fx.set_burn_frames(frame.vis.burn_frames(&walk));
            frame.fx.update(core::TICK_DT);
            if let Some(w) = frame.weather.as_mut() {
                w.update(core::TICK_DT);
            }
            frame.bolts.update(&walk, core::TICK_DT);
            tick_event_feed(&mut frame.feed, &walk.sim, walk.player, v.hud);
            if v.events.is_some() {
                append_events(&mut events_csv, tick, &walk.sim.events);
            }
            hits += walk
                .sim
                .events
                .iter()
                .filter(|e| matches!(e, core::sim::Event::Hit { .. }))
                .count() as u32;
            // `SQUAD_TRACE=1`: every 100 ticks, who is under an order, how far
            // they are from the player and what they were told — the Tier-4
            // capture's own referee (brief D14), because a screenshot cannot
            // say whether the six that took the order are the six that stayed.
            if trace_squad && tick % 100 == 0 {
                let p = walk.sim.units[walk.player.index as usize].soldier.pos;
                let mut who: Vec<String> = Vec::new();
                for (h, u) in walk.sim.alive_units() {
                    if let Some(c) = u.command {
                        who.push(format!(
                            "#{}@{}:{:.0}m:{}",
                            h.index,
                            c.issuer,
                            core::nav::xz_dist(u.soldier.pos, p),
                            match c.kind {
                                core::squad::CommandKind::Follow(_) => "F",
                                core::squad::CommandKind::Hold(_) => "H",
                            }
                        ));
                    }
                }
                println!(
                    "SQUAD t={tick} commanded={} releases={} [{}]",
                    who.len(),
                    walk.sim.nav_stats.releases,
                    who.join(" ")
                );
            }
            let s = walk.soldier();
            let me = &walk.sim.units[walk.player.index as usize];
            let speed = glam::Vec2::new(s.vel.x, s.vel.z).length();
            csv.push_str(&format!(
                "{tick},{:.3},{:.3},{:.3},{:.3},{:?},{:.1},{:.4},{},{},{},{},{:.0},{},{}",
                s.pos.x,
                s.pos.y,
                s.pos.z,
                speed,
                s.stance,
                s.energy,
                s.yaw,
                walk.magnet().map_or(-1, |h| i64::from(h.index)),
                me.weapon.shots_fired,
                hits,
                me.kills,
                me.health.cur,
                walk.sim.bolts.len(),
                walk.target_draws().len(),
            ));
            let b = &walk.sim.battle;
            let cps = |t: u8| walk.sim.cps.iter().filter(|c| c.owner == Some(t)).count();
            csv.push_str(&format!(
                ",{},{},{},{},{},{},{}\n",
                b.teams[0].tickets(),
                b.teams[1].tickets(),
                b.teams[0].alive(),
                b.teams[1].alive(),
                cps(0),
                cps(1),
                walk.sim.outcome.map_or(-1, i64::from)
            ));
            if let Some((every, dir)) = &v.capture_series {
                if (tick as u64).is_multiple_of(*every) {
                    std::fs::create_dir_all(dir)?;
                    let dt = hud_tick.map_or(0.0, |t| {
                        walk.sim.tick.saturating_sub(t) as f32 * core::TICK_DT
                    });
                    frame.render(&mut r, &mut gpu, &walk, v, dt)?;
                    if frame.overlay.is_some() {
                        hud_tick = Some(walk.sim.tick);
                    }
                    let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
                    let png = render::encode_png(&rgba, w, h).map_err(|e| anyhow!("png: {e}"))?;
                    std::fs::write(dir.join(format!("walk_{tick:05}.png")), png)?;
                }
            }
        }
        if let (Some(path), Some(sl)) = (&sound_log_path, &sound_log) {
            std::fs::write(path, sl.csv())?;
        }
        if let Some(path) = &v.events {
            std::fs::write(path, &events_csv).with_context(|| path.display().to_string())?;
        }
        println!(
            "sim hash after {} ticks: {:016x}{}",
            log.ticks.len(),
            walk.sim.hash(),
            match walk.sim.outcome {
                Some(core::sim::OUTCOME_DRAW) => " (round over: time up, a draw)".to_string(),
                Some(w) => format!(" (round over: team {w} wins)"),
                None => String::new(),
            }
        );
        // What the player ENDED the run as: the class its unit was built
        // from, the class its next respawn would take, and the weapon it is
        // holding — the three numbers a spawn-select replay has to show.
        {
            let me = &walk.sim.units[walk.player.index as usize];
            let team = &walk.teams[usize::from(me.team.min(1))];
            let slot = me.spec.player_slot.map_or(0, usize::from);
            let choice = walk.sim.spawn_choice.get(slot).copied().unwrap_or(0);
            println!(
                "player: {} as {} (class {}), weapon_class {} {}; spawn_choice {} = {}",
                if me.alive { "alive" } else { "DEAD" },
                team.class(me.spec.class).name,
                me.spec.class,
                me.weapon_class,
                walk.sim.weapon_classes[me.weapon_class].name,
                choice,
                team.class(choice).name,
            );
        }
        if let Some((_, dir)) = &v.capture_series {
            std::fs::write(dir.join("motion.csv"), &csv)?;
            println!(
                "walk replay: {} ticks -> {}",
                log.ticks.len(),
                dir.display()
            );
        }
        // Leave the final frame rendered, even if the series just captured this tick.
        let dt = hud_tick.map_or(0.0, |t| {
            walk.sim.tick.saturating_sub(t) as f32 * core::TICK_DT
        });
        frame.render(&mut r, &mut gpu, &walk, v, dt)?;
    } else if let Some((frames, dir)) = &v.capture_orbit {
        return capture_orbit(&mut r, &scene, &mut gpu, *frames, dir);
    } else {
        let cam = if v.cam_overview {
            game::viewer::overview_camera(&scene)
        } else {
            camera_from(v.cam)
        };
        // The water at the renderer's clock (0 here) unless
        // OPENBF_WATER_TIME pins it (queue OC).
        gpu.animate_water_live(&mut r, cam.position)?;
        // The map's precipitation at its still state (card WTHR).
        let map = std::path::Path::new(&v.map);
        let assets = map.parent().unwrap_or(std::path::Path::new("."));
        game::weather::Weather::submit_still(&mut r, &scene.manifest, assets, map, &cam);
        r.add_particles(game::holo::still_draws(&gpu.holo, &scene, &cam));
        r.render(&cam, &gpu.draws, gpu.clear)
            .map_err(|e| anyhow!("render: {e}"))?;
    }

    if let Some(out) = &v.capture_frame {
        let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        let png = render::encode_png(&rgba, w, h).map_err(|e| anyhow!("png: {e}"))?;
        std::fs::write(out, png).with_context(|| out.display().to_string())?;
        println!("captured {}x{} -> {}", w, h, out.display());
    }
    Ok(())
}

/// Headless census of a map: how the gallery/flythrough frames it, whether an
/// orbit can see its centre at all, and every draw that resolved NO texture.
///
/// The untextured list comes out of `upload_scene` itself rather than a
/// separate walk of the manifest, so it cannot drift from what renders.
fn scene_report(scene: &game::assets::Scene, gpu: &SceneGpu, map: &std::path::Path) -> Result<()> {
    let name = map
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let o = game::viewer::overview_bounds(scene);
    match o {
        Some(o) => println!(
            "{name}\tcentre {:.0},{:.0},{:.0}\tradius {:.0}\tdraws {}\tinstances {}",
            o.center.x,
            o.center.y,
            o.center.z,
            o.radius,
            gpu.draws.len(),
            scene.manifest.instances.len()
        ),
        None => println!("{name}\tno framing"),
    }
    let total: usize = gpu.untextured.iter().map(|u| u.2).sum();
    // "untextured draws" is now OUR lookup failures ONLY — draws that ask for
    // a texture we should have found and did not, still painted flat grey so
    // they stay visible. The three lines below are the disc's own gaps, which
    // are SKIPPED rather than drawn (see `viewer::Untextured`), so they are
    // counted apart and are not a bug in this loader.
    println!("  untextured draws: {total} of {}", gpu.draws.len());
    for (what, mat, n) in gpu.untextured.iter().take(20) {
        println!("    {n:5}  {what}  material={mat}");
    }
    if gpu.untextured.len() > 20 {
        println!("    ... {} more kinds", gpu.untextured.len() - 20);
    }
    println!("  untextured on the disc: {}", gpu.disc_untextured);
    println!("  dangling on the disc: {}", gpu.dangling);
    println!(
        "  terrain chunks dropped (texture in another LVL): {}",
        gpu.terrain_dropped
    );
    Ok(())
}

/// One full circle around the map, one PNG per frame — the Tier-4
/// flythrough. Framing comes from `overview_camera`, so a map's orbit is the
/// same shot the golden gallery frames, swung around it: no hand-authored
/// poses to keep in step with the bake.
///
/// The renderer's clock advances at 30 fps; water geometry and speckles step
/// at that clock (or OPENBF_WATER_TIME). Everything else is static — this
/// renders the SCENE, not a battle.
fn capture_orbit(
    r: &mut Renderer,
    scene: &game::assets::Scene,
    gpu: &mut SceneGpu,
    frames: u32,
    dir: &std::path::Path,
) -> Result<()> {
    use game::viewer::{look_at, orbit_eye, overview_bounds};
    std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    let Some(o) = overview_bounds(scene) else {
        anyhow::bail!("no instances or terrain to orbit");
    };
    for k in 0..frames {
        let t = k as f32 / frames as f32;
        let a = std::f32::consts::TAU * t;
        // A gentle rise and fall over the circle, so it reads as a fly-round
        // rather than a turntable.
        // (`fmath` only because this file also runs the sim, which the libm
        // referee scans; the orbit itself is presentation.)
        let lift = 0.34 + 0.12 * core::fmath::sin_cos(a).0;
        let cam = look_at(orbit_eye(o, a, lift), o.center);
        r.time = (k as f32 / 30.0) % render::TIME_WRAP;
        r.water_time = k as f64 / 30.0;
        gpu.animate_water_live(r, cam.position)?;
        r.render(&cam, &gpu.draws, gpu.clear)
            .map_err(|e| anyhow!("render: {e}"))?;
        let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        let png = render::encode_png(&rgba, w, h).map_err(|e| anyhow!("png: {e}"))?;
        std::fs::write(dir.join(format!("{k:04}.png")), png)?;
    }
    println!(
        "captured {frames} orbit frames -> {} (centre {:.0},{:.0},{:.0} radius {:.0})",
        dir.display(),
        o.center.x,
        o.center.y,
        o.center.z,
        o.radius
    );
    Ok(())
}

/// `--ride-script <out>` (card VSEAT): run `game::ride::script` on a fresh
/// battle, write its log to `out`, print what happened; the caller replays
/// the log as `--walk-replay` does.
fn ride_script(
    scene: &game::assets::Scene,
    v: &ViewerArgs,
    out: &std::path::Path,
) -> Result<std::path::PathBuf> {
    let mut walk = make_walk(scene, &BattleSetup::from_args(v, v.seed)?);
    let ride = game::ride::script(&mut walk, v.seed);
    println!(
        "ride: {} ticks; entered {:?}; drove {:.1} m; exited {:?}; sim hash {:016x}",
        ride.log.ticks.len(),
        ride.entered,
        ride.driven_m,
        ride.exited,
        walk.sim.hash()
    );
    std::fs::write(out, ron::to_string(&ride.log)?)?;
    anyhow::ensure!(ride.exited.is_some(), "the ride did not complete");
    Ok(out.to_path_buf())
}

/// AIMG's scripted, asset-free acquisition replay. Geometry follows the
/// existing AIMF fixtures (analysis/re-aim-assist.md §3.3); only Sim::step
/// acquires the target. No renderer, baked map or private acquisition call.
pub(super) fn run_aim_replay(args: &[String]) -> Result<()> {
    use core::collision::{Body, CollisionWorld, Shape};
    use core::heightfield::Heightfield;
    use core::sim::{buttons, AimTarget, PadState, Sim};
    use core::unit::UnitSpec;
    use glam::{Mat4, Vec3};
    use std::fmt::Write;

    anyhow::ensure!(args.len() == 1, "--aim-replay <output-directory>");
    let out = std::path::Path::new(&args[0]);
    std::fs::create_dir_all(out)?;
    let mut sim = Sim::empty(5);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let player = sim
        .spawn(&UnitSpec::player(0, Vec3::ZERO))
        .context("player")?;
    let enemy = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(1.0, 0.0, -20.0)))
        .context("enemy")?;
    let friend = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(20.0, 0.0, 20.0)))
        .context("friend")?;
    let mut csv = String::from("tick,phase,magnet,reticle,relation,yaw,pitch,enemy_x,enemy_z,friend_x,friend_z,wall,hash\n");
    // The same enemy leaves and re-enters the cone; the held target must
    // release on the next tick. FIRE is released in rejection phases so
    // its deliberately wider sticky cone does not mask the cone boundary.
    for (phase, x, z, friendly, wall, acquired) in [
        ("off_ray", 1.0, -20.0, false, false, true),
        ("outside_cone", 8.0, -20.0, false, false, false),
        ("friend_crosshair", 8.0, -20.0, true, false, false),
        ("past_range", 0.0, -70.0, false, false, false),
        ("wall", 1.0, -20.0, false, true, false),
        ("reacquire", 1.0, -20.0, false, false, true),
    ] {
        sim.units[player.index as usize].soldier.yaw = 0.0;
        sim.units[player.index as usize].soldier.pitch = 0.0;
        sim.units[enemy.index as usize].soldier.pos = Vec3::new(x, 0.0, z);
        let f = if friendly {
            Vec3::new(0.0, 0.0, -10.0)
        } else {
            Vec3::new(20.0, 0.0, 20.0)
        };
        sim.units[friend.index as usize].soldier.pos = f;
        sim.collision = CollisionWorld::default();
        if wall {
            sim.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
                Shape::Box {
                    half: Vec3::new(5.0, 5.0, 0.5),
                },
            ));
        }
        for _ in 0..12 {
            sim.step(&[PadState {
                buttons: if acquired { buttons::FIRE } else { 0 },
                ..Default::default()
            }]);
            let magnet = sim.magnet_at(0);
            anyhow::ensure!(
                magnet == acquired.then_some(AimTarget::Unit(enemy)),
                "{phase} tick {}: magnet {magnet:?}",
                sim.tick
            );
            let pick = sim.reticle_pick[0];
            if friendly {
                anyhow::ensure!(
                    pick == Some((AimTarget::Unit(friend), 1)),
                    "friend not picked: {pick:?}"
                );
            }
            if phase == "past_range" {
                anyhow::ensure!(
                    pick == Some((AimTarget::Unit(enemy), -1)),
                    "distant enemy not picked: {pick:?}"
                );
            }
            if wall {
                anyhow::ensure!(pick.is_none(), "wall did not block reticle: {pick:?}");
            }
            let (pick_index, relation) = match pick {
                Some((AimTarget::Unit(h), relation)) => (i64::from(h.index), relation),
                _ => (-1, 0),
            };
            let p = &sim.units[player.index as usize].soldier;
            writeln!(
                csv,
                "{},{phase},{},{pick_index},{relation},{:.6},{:.6},{x},{z},{},{},{},{:016x}",
                sim.tick,
                if magnet.is_some() {
                    i64::from(enemy.index)
                } else {
                    -1
                },
                p.yaw,
                p.pitch,
                f.x,
                f.z,
                u8::from(wall),
                sim.hash()
            )?;
        }
    }
    std::fs::write(out.join("motion.csv"), csv)?;
    println!("sim hash after {} ticks: {:016x}", sim.tick, sim.hash());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tick_event_feed;
    use core::sim::{Event, PadState, Sim};
    use core::unit::UnitSpec;
    use game::event_feed::EventFeed;
    use glam::Vec3;

    #[test]
    fn replay_feed_without_hud_drains_each_tick_and_preserves_hud_delivery() {
        fn run(hud: bool) -> (u64, i32) {
            let mut sim = Sim::empty(7);
            let player = sim
                .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
                .unwrap();
            let mut feed = EventFeed::default();
            let mut reference = EventFeed::default();
            let mut expected_messages = Vec::new();
            let mut expected_edges = Vec::new();
            let mut delivered = 0;
            // A minute of hits and a human kill line every five seconds:
            // beyond the same-line suppression window, across many captures.
            for tick in 0..1800 {
                sim.step(&[PadState::default()]);
                if tick % 150 == 0 {
                    sim.events.push(Event::Kill {
                        shooter: player,
                        victim: player,
                    });
                }
                sim.events.push(Event::Hit {
                    shooter: player,
                    victim: player,
                    damage: 0.0,
                    age: 0.0,
                    health_left: 100.0,
                    dir: Vec3::X,
                });
                let hash = sim.hash();
                reference.tick(&sim, player);
                expected_messages.extend(reference.take().into_iter().map(|m| (m.text, m.color)));
                expected_edges.extend(reference.take_ambush());
                tick_event_feed(&mut feed, &sim, player, hud);
                assert_eq!(sim.hash(), hash, "feed must not alter the sim");
                if hud && (tick % 17 == 0 || tick == 1799) {
                    let messages: Vec<_> =
                        feed.take().into_iter().map(|m| (m.text, m.color)).collect();
                    delivered += messages.len();
                    assert_eq!(messages, expected_messages);
                    assert_eq!(feed.take_ambush(), expected_edges);
                    expected_messages.clear();
                    expected_edges.clear();
                }
            }
            if hud {
                assert_eq!(delivered, 12);
            } else {
                assert_eq!(expected_messages.len(), 12);
                assert_eq!(expected_edges.len(), 1800);
                assert_eq!(feed.take().len(), 0, "HUD-off retained messages");
                assert_eq!(feed.take_ambush().len(), 0, "HUD-off retained hits");
            }
            let stats = feed.stats(&sim, player);
            assert_eq!(stats.points, reference.stats(&sim, player).points);
            (sim.hash(), stats.points)
        }
        assert_eq!(run(false), run(true));
    }
}
