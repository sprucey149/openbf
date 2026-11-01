//! The port's view from a PCSX2 save state's camera pose, so a reference frame and a port frame stand in the
//! same place (card PARITY; was the scratch `pxc_pose.rs` of the PXC captures). `scripts/parity.py` and
//! `engine/artifacts/pcsx2/port.py` call it:
//!
//!   OPENBF_TEXTURE_TIER=1 pose_render <map_dir> <out.png=x,y,z,yaw_deg,pitch_deg[,fov_y_deg[,w,h]]>...
//!
//! yaw/pitch are `render::Camera`'s (0 = looking down -Z, + turns left; + pitch looks up) — exactly what
//! `engine/artifacts/pcsx2/pose.py` prints. fov_y defaults to the port's 70; the size to 640x480 (the
//! PS2 frame's 4:3). No soldier, no HUD: the scene as `upload_scene` builds it (conquest layer), its water stepped
//! to the water clock's 0 s from each pose's eye as the game loop steps it (`SceneGpu::animate_water`) — so Kamino
//! shows the FFT sea the game draws, not the flat plane of an unstepped scene (card KSEA).
//! `PXC_DROP_LAYERS=Opaque,Blend,…` drops draws by layer; `PXC_SKIPPED=1` prints the classes without a model;
//! `PXC_LIGHTS=1` prints the local lights the scene is drawn with (card BPRL: the posts of the conquest mission);
//! `POSE_DOORS_OPEN=<0..1>` draws every BF2 door at that `open` (card STDOR: a PS2 frame taken inside a trigger).
//! `POSE_TURRET=x,y,z,look_yaw,look_pitch,aim_yaw,aim_pitch` (degrees, the port's seat angles: yaw turns the model's
//! +z toward +x, pitch + is up) draws the building turret nearest x,y,z turned so (card VTURD: a PS2 frame's own aim).
//! The map loads as a battle loads it (`assets::load_battle_scene`: the con layer, then the game's COMMON bake);
//! `POSE_COMMON=0` leaves the COMMON bake out (before card SKIPC every still did, and drew no post console or droid).
//! `POSE_GUNNER=<soldier geometry>` (with `POSE_TURRET`, card VTURG) seats a soldier of that body in the turret nearest
//! x,y,z, its gun at the same angles, and draws him as the game does (`soldier_vis`); an 8th `POSE_TURRET` value is the
//! turn's rate over its speed (the nine-pose's lean on a full-circle seat). `POSE_FACTIONS=all,imp` (default) names
//! the sides whose classes the turrets are built from. `POSE_ZOOM=1` (with `POSE_GUNNER`, card VTURZ) zooms the seat at
//! its weapon's `ZoomMin` and draws every pose from the port's own zoomed camera (`WalkSim::camera`, the eye point
//! along the look, the eased zoom's FOV) and the gunner's own view (`SceneGpu::view_draws`: his building left out);
//! the pose's place and angles are then ignored and the camera is printed; `POSE_ZOOM=pose` keeps the pose's camera
//! (a fit of the PS2's zoomed projection).
//! `POSE_RIDE=<vehicle class>,<soldier geometry>,<the vehicle's 16 matrix words as the PS2's RAM holds them>[,throttle,
//! strafe,turn]` (card VRIDE) spawns that class at the matrix (rows X, Y, Z, origin), seats a soldier of that body as its
//! pilot, holds the inputs for 3 s (the nine-pose settles) and draws the vehicle and its rider as a battle does.
//! The map's precipitation is drawn at its setup state (card WTHR; `PXC_WEATHER=0` leaves it out, `WEATHER_VECTOR`
//! / `WEATHER_DT` / `WEATHER_ONLY` are the card's frame test, see below).
use anyhow::{anyhow, Result};
use game::assets::load_scene;
use game::viewer::upload_scene;
use glam::Vec3;
use render::{Camera, Renderer};
use std::path::Path;

#[path = "support/pose_ride_spec.rs"]
mod pose_ride_spec;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let map = Path::new(
        args.get(1)
            .ok_or_else(|| anyhow!("usage: pose_render <map_dir> <out.png=pose>..."))?,
    );
    // The map as a battle loads it (the con layer and the game's COMMON bake: the post console, the recharge droids
    // and the gun turrets live there, card SKIPC); `POSE_COMMON=0` draws the map's own bake alone.
    let scene = if std::env::var("POSE_COMMON").as_deref() == Ok("0") {
        let mut s = load_scene(map)?;
        s.apply_mode_layer("con");
        s
    } else {
        let (s, load) = game::assets::load_battle_scene(map, "con")?;
        if let Some((cd, Err(e))) = &load.common {
            return Err(anyhow!(
                "common assets at {} unreadable: {e:#}",
                cd.display()
            ));
        }
        s
    };
    if std::env::var_os("PXC_LIGHTS").is_some() {
        for l in game::point_light::still_lights(&scene) {
            let c = l.color.map(|v| (v * 128.0).round() as i32);
            println!(
                "light {:.2} {:.2} {:.2} r {:.1} gs {} {} {} static {}",
                l.pos.x, l.pos.y, l.pos.z, l.range, c[0], c[1], c[2], l.is_static
            );
        }
    }
    // The map's weather (card WTHR) at a fixed state: the setup's particles, no time run. `PXC_WEATHER=0`
    // leaves it out; `WEATHER_VECTOR=<json>` injects a PS2 state's particles and wind (`predict.py --export`),
    // `WEATHER_DT=<s>` runs them that long (it may be negative: the PS2 frame lags its RAM), `WEATHER_ONLY=1`
    // draws the weather alone over black - the frame test of the card.
    let assets = map.parent().unwrap_or(Path::new("."));
    let mut weather = match std::env::var("PXC_WEATHER").as_deref() {
        Ok("0") => None,
        _ => game::weather::Weather::load(&scene.manifest, assets, map, game::weather::STILL_SEED),
    };
    if let (Some(w), Ok(path)) = (weather.as_mut(), std::env::var("WEATHER_VECTOR")) {
        let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let v3 = |v: &serde_json::Value| {
            Vec3::new(
                v[0].as_f64().unwrap_or(0.0) as f32,
                v[1].as_f64().unwrap_or(0.0) as f32,
                v[2].as_f64().unwrap_or(0.0) as f32,
            )
        };
        let parts = j["particles"]
            .as_array()
            .ok_or_else(|| anyhow!("{path}: no particles"))?;
        w.particles = parts
            .iter()
            .map(|q| game::weather::Particle {
                pos: v3(&q["pos"]),
                offset: v3(&q["vel"]),
                alpha: q["alpha"].as_f64().unwrap_or(1.0) as f32,
                spin: 0.0,
                phase: 0.5,
            })
            .collect();
        w.wind = game::weather::Wind::new(v3(&j["wind"]), 0.0, 0.0);
        println!("weather: {} particles from {path}", w.particles.len());
    }
    if let (Some(w), Some(dt)) = (
        weather.as_mut(),
        std::env::var("WEATHER_DT")
            .ok()
            .and_then(|v| v.parse::<f32>().ok()),
    ) {
        w.update(dt);
    }
    let weather_only = std::env::var_os("WEATHER_ONLY").is_some();
    type Cur = (
        u32,
        u32,
        Renderer,
        game::viewer::SceneGpu,
        Vec<render::SkinnedDrawCmd>,
    );
    let mut cur: Option<Cur> = None;
    // `POSE_ZOOM`'s gunner: the port's zoomed camera and his sim.
    let mut zoom_view: Option<ZoomView> = None;
    for spec in &args[2..] {
        let (out, c) = spec
            .split_once('=')
            .ok_or_else(|| anyhow!("want out.png=x,y,z,yaw,pitch[,fov[,w,h]]"))?;
        let c = c
            .split(',')
            .map(|v| v.trim().parse::<f32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("{spec}: {e}"))?;
        if c.len() < 5 {
            return Err(anyhow!("{spec}: want x,y,z,yaw,pitch[,fov[,w,h]]"));
        }
        let (w, h) = if c.len() >= 8 {
            (c[6] as u32, c[7] as u32)
        } else {
            (640, 480)
        };
        if cur.as_ref().map(|k| (k.0, k.1)) != Some((w, h)) {
            // The renderer is made once per size, the scene uploaded into it.
            let mut r = Renderer::new_headless(w, h).map_err(|e| anyhow!("renderer: {e}"))?;
            let mut gpu = upload_scene(&mut r, &scene)?;
            let mut skinned = Vec::new();
            if let Some(open) = std::env::var("POSE_DOORS_OPEN")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
            {
                let (_, mut doors) = scene.collision_and_doors();
                for d in &mut doors {
                    d.open = open;
                }
                gpu.pose_doors(&doors);
            }
            if let Some(t) = std::env::var("POSE_TURRET").ok().and_then(|v| {
                let f: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (f.len() == 7 || f.len() == 8).then_some(f)
            }) {
                // The map's own classes, then every shared class file of its game.
                let mut db = game::classdb::ClassDb::default();
                db.load_map(map)?;
                let game_key = scene.manifest.source_game.to_ascii_lowercase();
                let mut files: Vec<_> = std::fs::read_dir(assets.join("classes"))?
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with(&game_key))
                    })
                    .collect();
                files.sort();
                for f in files {
                    db.load(&f)?;
                }
                let section = |class: &str| {
                    let name = db.resolve(class)?.to_owned();
                    game::vehicle_class::seats(&db, &name)
                        .into_iter()
                        .find(|s| s.turret_node.is_some())
                };
                let rad = |d: f32| d.to_radians();
                gpu.pose_turrets_at(
                    Some(Vec3::new(t[0], t[1], t[2])),
                    section,
                    (rad(t[3]), rad(t[4])),
                    (rad(t[5]), rad(t[6])),
                );
                if let Ok(body) = std::env::var("POSE_GUNNER") {
                    let look = (rad(t[3]), rad(t[4]));
                    let aim = (rad(t[5]), rad(t[6]));
                    let (s, z) = gunner(
                        &mut r,
                        &scene,
                        map,
                        assets,
                        &body,
                        Vec3::new(t[0], t[1], t[2]),
                        look,
                        aim,
                        t.get(7).copied().unwrap_or(0.0),
                    )?;
                    skinned = s;
                    zoom_view = z;
                }
            }
            if let Ok(ride) = std::env::var("POSE_RIDE") {
                let (vehicle, rider) = ride_draws(&mut r, &scene, map, assets, &ride)?;
                gpu.draws.extend(vehicle);
                skinned = rider;
            }
            if std::env::var_os("PXC_SKIPPED").is_some() {
                for (class, n) in &gpu.skipped {
                    println!("skipped {n:4} {class}");
                }
            }
            cur = Some((w, h, r, gpu, skinned));
        }
        let Some((_, _, r, gpu, skinned)) = cur.as_mut() else {
            unreachable!("set above");
        };
        let own = std::env::var("POSE_ZOOM").as_deref() == Ok("1");
        let cam = match &zoom_view {
            Some((cam, _)) if own => *cam,
            _ => Camera {
                position: Vec3::new(c[0], c[1], c[2]),
                yaw: c[3].to_radians(),
                pitch: c[4].to_radians(),
                fov_y: c.get(5).copied().unwrap_or(70.0).to_radians(),
                ..Camera::default()
            },
        };
        gpu.animate_water(r, game::water_anim::clock(0.0), cam.position)?;
        // Dropped per frame, from a copy: the water's draws are re-picked for every eye.
        let mut draws = match &zoom_view {
            Some((_, walk)) => gpu.view_draws(&walk.sim, walk.player),
            None => gpu.draws.clone(),
        };
        if let Ok(drop) = std::env::var("PXC_DROP_LAYERS") {
            let n = draws.len();
            draws.retain(|d| !drop.split(',').any(|l| l == format!("{:?}", d.layer)));
            println!("dropped {} of {n} draws ({drop})", n - draws.len());
        }
        let mut clear = gpu.clear;
        let mut particles = if let Some(w) = weather.as_mut() {
            let wd = w.frame_draws(r, &cam, Vec3::ZERO);
            println!("weather: {} draws", wd.len());
            wd
        } else {
            Vec::new()
        };
        // The posts' holograms (card CPHOL); `PXC_HOLO=0` leaves them out.
        if std::env::var("PXC_HOLO").as_deref() != Ok("0") {
            particles.extend(game::holo::still_draws(&gpu.holo, &scene, &cam));
        }
        if weather_only {
            draws.clear();
            clear = [0.0, 0.0, 0.0, 1.0];
            r.set_sky(Vec::new());
            r.set_grass(Vec::new(), Vec::new());
            r.set_frame_filter(None);
        }
        if let Some(time) = std::env::var("POSE_EFFECT_TIME")
            .ok()
            .and_then(|t| t.parse::<f32>().ok())
            .filter(|t| t.is_finite() && *t >= 0.0)
        {
            let mut fx = game::fx::Fx::load(
                map.parent().unwrap_or(Path::new(".")),
                map,
                &scene.manifest.source_game,
                &[],
            );
            fx.set_scene_effects(&scene);
            for _ in 0..(time * 30.0).ceil() as u32 {
                fx.update(1.0 / 30.0);
            }
            fx.submit_with(r, &cam, particles);
        } else {
            r.set_particles(particles);
        }
        r.render_frame(&cam, &draws, skinned, clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        let png = render::encode_png_rgb(&rgba, w, h).map_err(|e| anyhow!("png: {e}"))?;
        std::fs::write(out, png)?;
        println!("{out}");
    }
    Ok(())
}

/// `POSE_ZOOM`'s gunner (card VTURZ): the port's zoomed camera and his sim.
type ZoomView = (Camera, game::walk::WalkSim);

/// Card VTURG: a soldier of body `body` seated in the building turret nearest `near`, its gun at `look` / `aim`
/// (`stick` = the turn's rate over its speed), drawn as a battle draws him (`soldier_vis`); with `POSE_ZOOM`, the seat
/// zoomed at its weapon's `ZoomMin` and his [`ZoomView`].
#[allow(clippy::too_many_arguments)]
fn gunner(
    r: &mut Renderer,
    scene: &game::assets::Scene,
    map: &Path,
    assets: &Path,
    body: &str,
    near: Vec3,
    look: (f32, f32),
    aim: (f32, f32),
    stick: f32,
) -> Result<(Vec<render::SkinnedDrawCmd>, Option<ZoomView>)> {
    let factions: Vec<String> = std::env::var("POSE_FACTIONS")
        .unwrap_or_else(|_| "all,imp".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    let f = [
        factions[0].clone(),
        factions.get(1).cloned().unwrap_or_default(),
    ];
    let mut sim = core::sim::Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    scene.install_collision(&mut sim);
    let build =
        game::vehicle_pads::install(&mut sim, &scene.manifest, map, &assets.join("classes"), &f)?;
    let team = body_team(body);
    let mut walk = game::walk::WalkSim::with_teams(sim, None, near, [team.clone(), team], None);
    walk.vehicle_seats = build.seats.clone();
    let (h, v) = walk
        .sim
        .alive_vehicles()
        .filter(|(_, v)| v.kind == core::vehicle::VehicleKind::Turret)
        .min_by(|a, b| a.1.pos.distance(near).total_cmp(&b.1.pos.distance(near)))
        .map(|(h, v)| (h, v.class as usize))
        .ok_or_else(|| anyhow!("no turret"))?;
    let seat = walk.sim.seat_setup[v]
        .as_ref()
        .and_then(|s| s.guns.iter().position(Option::is_some))
        .ok_or_else(|| anyhow!("no gunner seat"))?;
    let rate = walk.sim.seat_setup[v].as_ref().unwrap().guns[seat]
        .as_ref()
        .unwrap()
        .yaw_speed();
    let player = walk.player;
    if !walk.sim.seat_unit(player, h, seat as u8) {
        return Err(anyhow!("the seat would not take him"));
    }
    let mut vis = game::soldier_vis::SoldierVisuals::load(
        r,
        assets,
        &scene.manifest.source_game,
        &walk.teams,
    );
    vis.set_turrets(game::viewer::turret_pose::rigged_turrets(scene));
    for k in 0..3 {
        // The tick before the last one turned by `stick` at the full rate.
        let y = if k == 2 {
            look.0
        } else {
            look.0 - stick * rate * core::TICK_DT
        };
        let veh = walk.sim.vehicle_mut(h).unwrap();
        if veh.guns.len() <= seat {
            veh.guns.resize(seat + 1, None);
        }
        veh.guns[seat] = Some(core::vehicle::gun::GunState {
            look: (y, look.1),
            aim,
            rate: (0.0, 0.0),
            rel: aim,
            weapons: Vec::new(),
            zoom: None,
        });
        walk.sim.tick += 1;
        vis.update(&walk, core::TICK_DT);
    }
    let drawn = vis.drawn(&walk);
    println!(
        "gunner {body}: {} drawn {:?}",
        drawn.len(),
        drawn.first().map(|d| d.1.w_axis.truncate())
    );
    // The gunner is drawn in every view, his own zoomed one too.
    let draws = vis.draws(&walk);
    let zoom = if matches!(std::env::var("POSE_ZOOM").as_deref(), Ok("1" | "pose")) {
        let gun = walk.sim.seat_setup[v].as_ref().unwrap().guns[seat]
            .clone()
            .unwrap();
        let wc = &walk.sim.weapon_classes[gun
            .weapons
            .first()
            .ok_or_else(|| anyhow!("no weapon"))?
            .class];
        if !wc.zooms() {
            return Err(anyhow!("the seat's weapon does not zoom"));
        }
        let value = wc.zoom_min;
        walk.sim.vehicle_mut(h).unwrap().guns[seat]
            .as_mut()
            .unwrap()
            .zoom = Some(value);
        // The camera's zoom eased all the way (a battle's after 0.3 s).
        walk.zoom_s = value;
        walk.zoom_fov_mix = 1.0;
        let cam = walk.camera();
        let f = cam.forward();
        println!(
            "zoom {value}: camera at ({:.3}, {:.3}, {:.3}) along ({:.4}, {:.4}, {:.4}), fov_y {:.2} deg",
            cam.position.x,
            cam.position.y,
            cam.position.z,
            f.x,
            f.y,
            f.z,
            cam.fov_y.to_degrees()
        );
        Some((cam, walk))
    } else {
        None
    };
    Ok((draws, zoom))
}

/// Body-only diagnostic visual: preserve the default team's other fields.
fn body_team(body: &str) -> game::walk::TeamTemplate {
    let mut team = game::walk::TeamTemplate::default_trooper();
    team.classes = vec![game::walk::ClassVisual {
        name: body.into(),
        label: body.into(),
        geometry: Some(body.into()),
        ..Default::default()
    }];
    team
}

/// Card VRIDE: `POSE_RIDE`'s vehicle of class `spec[0]` at the PS2 matrix `spec[2..18]`, a soldier of body `spec[1]` its
/// pilot, the drive's inputs `spec[18..21]` held for 3 s; the vehicle's draws and the rider's, as a battle draws them.
fn ride_draws(
    r: &mut Renderer,
    scene: &game::assets::Scene,
    map: &Path,
    assets: &Path,
    spec: &str,
) -> Result<(Vec<render::DrawCmd>, Vec<render::SkinnedDrawCmd>)> {
    let ride = pose_ride_spec::parse(spec).map_err(|e| anyhow!(e))?;
    let (class, body) = (ride.class, ride.body);
    let f = ride.matrix;
    let factions: Vec<String> = std::env::var("POSE_FACTIONS")
        .unwrap_or_else(|_| "all,imp".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    let fs = [
        factions[0].clone(),
        factions.get(1).cloned().unwrap_or_default(),
    ];
    let mut sim = core::sim::Sim::empty(1);
    scene.install_collision(&mut sim);
    let build =
        game::vehicle_pads::install(&mut sim, &scene.manifest, map, &assets.join("classes"), &fs)?;
    let ci = build
        .classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(class))
        .ok_or_else(|| anyhow!("no {class} among {:?}", build.classes))?;
    let pad = build
        .pads
        .iter()
        .flat_map(|p| p.classes.iter().flatten())
        .find(|c| c.spec.class as usize == ci)
        .ok_or_else(|| anyhow!("no pad of {class}"))?
        .spec
        .clone();
    let axis = |i: usize| Vec3::new(f[4 * i], f[4 * i + 1], f[4 * i + 2]);
    let pos = axis(3);
    let rot = glam::Quat::from_mat3(&glam::Mat3::from_cols(axis(0), axis(1), axis(2))).normalize();
    let team = body_team(body);
    let mut walk = game::walk::WalkSim::with_teams(sim, None, pos, [team.clone(), team], None);
    walk.vehicle_seats = build.seats.clone();
    walk.vehicle_riders = build.riders.clone();
    walk.vehicle_geometry = build.geometry.clone();
    let h = walk
        .sim
        .spawn_vehicle(&core::vehicle::VehicleSpec { pos, ..pad });
    let player = walk.player;
    if !walk.sim.seat_unit(player, h, 0) {
        return Err(anyhow!("the pilot seat would not take him"));
    }
    let input = ride.input;
    let mut vis = game::soldier_vis::SoldierVisuals::load(
        r,
        assets,
        &scene.manifest.source_game,
        &walk.teams,
    );
    vis.set_walker_rigs(game::viewer::walker_rigs(
        &scene.manifest,
        assets,
        &walk.vehicle_geometry,
    ));
    for _ in 0..90 {
        let v = walk.sim.vehicle_mut(h).unwrap();
        (v.pos, v.rot) = (pos, rot);
        (v.input.throttle, v.input.strafe, v.input.turn) = (input[0], input[1], input[2]);
        walk.sim.tick += 1;
        vis.update(&walk, core::TICK_DT);
    }
    let drawn = vis.drawn(&walk);
    println!(
        "ride {class} + {body}: {} drawn {:?}",
        drawn.len(),
        drawn.first().map(|d| d.1.w_axis.truncate())
    );
    let mut vv = game::viewer::VehicleVisuals::load(r, scene, assets, &walk.vehicle_geometry);
    Ok((vv.draws(r, &walk.sim, &[]), vis.draws(&walk)))
}
