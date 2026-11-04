use anyhow::{anyhow, Result};
use glam::Vec3;
use std::path::{Path, PathBuf};

fn resolve_default_path(candidates: &[&str]) -> Result<PathBuf> {
    for c in candidates {
        let p = Path::new(c);
        if p.exists() {
            return Ok(p.to_path_buf());
        }
    }
    Err(anyhow!(
        "Could not find any candidate path from {:?}",
        candidates
    ))
}

fn draw(
    renderer: &mut render::Renderer,
    fx: &mut game::fx::Fx,
    weather: &mut Option<game::weather::Weather>,
    draws: &[render::DrawCmd],
    clear: [f64; 4],
    camera: &render::Camera,
) -> Result<()> {
    let snow = weather
        .as_mut()
        .map_or_else(Vec::new, |w| w.frame_draws(renderer, camera, Vec3::ZERO));
    fx.submit_with(renderer, camera, snow);
    renderer.render_frame(camera, draws, &[], clear, None)?;
    Ok(())
}

fn capture(renderer: &mut render::Renderer, path: &Path) -> Result<Vec<u8>> {
    let (rgba, width, height) = renderer.capture_rgba()?;
    std::fs::write(path, render::encode_png_rgb(&rgba, width, height)?)?;
    Ok(rgba)
}

fn capture_named(renderer: &mut render::Renderer, out: &Path, name: &str) -> Result<Vec<u8>> {
    let path = out.join(name);
    let rgba = capture(renderer, &path)?;
    println!("Wrote {}", path.display());
    Ok(rgba)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let map = if let Some(arg) = args.get(1) {
        PathBuf::from(arg)
    } else {
        resolve_default_path(&["engine/assets_baked/bf2_geo1", "assets_baked/bf2_geo1"])?
    };

    let out = if let Some(arg) = args.get(2) {
        PathBuf::from(arg)
    } else {
        resolve_default_path(&["engine/artifacts/playtest", "artifacts/playtest"])?
    };

    let pose: Vec<f32> = if let Some(arg) = args.get(3) {
        arg.split(',').map(str::parse).collect::<Result<_, _>>()?
    } else {
        vec![-68.0, -62.0, 268.0, 0.0, 20.0]
    };
    anyhow::ensure!(pose.len() == 5, "pose must be x,y,z,yaw,pitch");
    let hz = std::env::var("CAPTURE_HZ")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1);
    anyhow::ensure!(
        hz > 0 && hz <= 30 && 30 % hz == 0,
        "capture rate must divide 30"
    );
    let seconds = std::env::var("CAPTURE_SECONDS")
        .ok()
        .map(|v| v.parse::<u32>())
        .transpose()?
        .unwrap_or(20);
    anyhow::ensure!(
        seconds > 0 && seconds <= 300,
        "capture seconds must be 1..300"
    );

    let camera = render::Camera {
        position: Vec3::new(pose[0], pose[1], pose[2]),
        yaw: pose[3].to_radians(),
        pitch: pose[4].to_radians(),
        fov_y: std::env::var("CAPTURE_FOV")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(52.05)
            .to_radians(),
        far: 8000.0,
        ..Default::default()
    };
    let mut scene = game::assets::load_battle_scene(&map, "con")?.0;
    if std::env::var_os("CAPTURE_MEAN_WATER").is_some() {
        if let Some(water) = &mut scene.manifest.water {
            water.vertex_diffuse = false;
        }
        if let Some(mesh) = &mut scene.water {
            mesh.colors.fill([255; 4]);
        }
    }
    if let Some(path) = args.get(4) {
        let added: manifest::Manifest = ron::from_str(&std::fs::read_to_string(path)?)?;
        scene.manifest.sky = added.sky;
        scene.manifest.fog_cloud = added.fog_cloud;
        if let Some(name) = scene
            .manifest
            .fog_cloud
            .as_ref()
            .and_then(|f| f.texture.as_ref())
        {
            let dir = Path::new(path).parent().unwrap().join("textures");
            let texture = game::assets::load_texture(&dir, name)?;
            let alpha = texture
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .fold((255u8, 0u8), |(a, b), v| (a.min(v), b.max(v)));
            println!("cloud texture {name} alpha {alpha:?}");
            scene.textures.insert(name.to_ascii_lowercase(), texture);
        }
    }
    std::fs::create_dir_all(&out)?;
    let mut renderer = render::Renderer::new_headless(856, 642)?;
    let mut gpu = game::viewer::upload_scene(&mut renderer, &scene)?;
    let clear = game::viewer::clear_color(&scene);
    if std::env::var_os("CAPTURE_NO_CLOUD").is_some() {
        renderer.set_fog_cloud(None);
    }
    if std::env::var_os("CAPTURE_NO_DUST").is_some() {
        renderer.set_dust(Vec::new());
    }
    if std::env::var_os("CAPTURE_NO_SUN").is_some() {
        renderer.set_sun_flare(None);
    }
    let mut fx = game::fx::Fx::load(
        map.parent().unwrap(),
        &map,
        &scene.manifest.source_game,
        &["rep", "cis"],
    );
    fx.set_scene_effects(&scene);
    let mut weather = game::weather::Weather::load(
        &scene.manifest,
        map.parent().unwrap(),
        &map,
        game::weather::STILL_SEED,
    );

    for tick in 0..=seconds * 30 {
        let time = f64::from(tick) / 30.0;
        renderer.time = time as f32;
        renderer.water_time = time;
        renderer.atmosphere_time = Some(time);
        renderer.advance_sky();
        gpu.animate_water(&mut renderer, time, camera.position)?;
        if tick > 0 {
            fx.update(1.0 / 30.0);
            if let Some(w) = &mut weather {
                w.update(1.0 / 30.0);
            }
        }
        draw(
            &mut renderer,
            &mut fx,
            &mut weather,
            &gpu.draws,
            clear,
            &camera,
        )?;
        if tick % (30 / hz) == 0 {
            capture(
                &mut renderer,
                &out.join(format!("{:03}.png", tick / (30 / hz))),
            )?;
            if tick % 30 == 0 {
                println!("t={time:.0}: {} live world effects", fx.len());
            }
        }
    }

    let has_ship = renderer.sky().iter().any(|s| {
        matches!(
            s.placement,
            render::Placement::Flight(_) | render::Placement::Object { .. }
        )
    });
    let mut ships = Vec::new();
    for shell in renderer.sky() {
        if let render::Placement::Flight(f) = shell.placement {
            if !ships.contains(&f) {
                ships.push(f);
            }
        }
    }
    let selected = std::env::var("CAPTURE_SHIP")
        .ok()
        .map(|v| v.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let selected = ships.get(selected).copied();
    if std::env::var_os("CAPTURE_ISOLATE_SHIP").is_some() {
        let shells = renderer
            .sky()
            .iter()
            .filter(|s| match s.placement {
                render::Placement::Flight(f) => Some(f) == selected,
                _ => true,
            })
            .copied()
            .collect();
        renderer.set_sky(shells);
    }
    let material_lighting: Vec<_> = renderer.sky().iter().map(|s| s.lit).collect();
    let ship_camera = if let Some(flight_pos) = selected.map(|f| f.model().w_axis.truncate()) {
        let dir = (flight_pos - camera.position).normalize();
        let position = std::env::var("CAPTURE_SHIP_DISTANCE")
            .ok()
            .map(|v| v.parse::<f32>())
            .transpose()?
            .map_or(camera.position, |distance| flight_pos - dir * distance);
        let pitch = dir.y.asin();
        let yaw = (-dir.x).atan2(-dir.z);
        render::Camera {
            position,
            yaw,
            pitch,
            fov_y: 35.0f32.to_radians(),
            far: 8000.0,
            ..Default::default()
        }
    } else {
        camera
    };

    println!("Capturing ship_unshaded.png vs ship_shaded.png...");
    for shell in renderer.sky_mut() {
        shell.lit = false;
    }
    draw(
        &mut renderer,
        &mut fx,
        &mut weather,
        &gpu.draws,
        clear,
        &ship_camera,
    )?;
    let rgba_unshaded = capture_named(&mut renderer, &out, "ship_unshaded.png")?;

    for (shell, lit) in renderer.sky_mut().iter_mut().zip(material_lighting) {
        shell.lit = lit;
    }
    draw(
        &mut renderer,
        &mut fx,
        &mut weather,
        &gpu.draws,
        clear,
        &ship_camera,
    )?;
    let rgba_shaded = capture_named(&mut renderer, &out, "ship_shaded.png")?;

    let pixel_diff = rgba_unshaded
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rgba_shaded.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count();
    println!("Pixels changed between unshaded and shaded: {pixel_diff}");
    if has_ship && std::env::var_os("CAPTURE_REQUIRE_SHIP_LIGHTING").is_some() {
        assert!(
            pixel_diff > 0,
            "shaded ship must visually differ from unshaded ship"
        );
    } else if !has_ship {
        assert_eq!(pixel_diff, 0, "no ship should change lighting");
    }

    println!("Capturing paused_frame_0.png vs paused_frame_1.png...");
    let frozen_time = renderer.atmosphere_time.unwrap_or(5.0);
    renderer.atmosphere_time = Some(frozen_time);
    draw(
        &mut renderer,
        &mut fx,
        &mut weather,
        &gpu.draws,
        clear,
        &ship_camera,
    )?;
    let rgba_paused_0 = capture_named(&mut renderer, &out, "paused_frame_0.png")?;

    for _ in 0..30 {
        renderer.advance_sky();
        draw(
            &mut renderer,
            &mut fx,
            &mut weather,
            &gpu.draws,
            clear,
            &ship_camera,
        )?;
    }
    let rgba_paused_1 = capture_named(&mut renderer, &out, "paused_frame_1.png")?;

    anyhow::ensure!(
        rgba_paused_0 == rgba_paused_1,
        "paused frame 0 and paused frame 1 must be byte-identical across 30 redraws"
    );
    println!("Verified: paused frames are identical across 30 redraws!");

    println!(
        "All visual comparison captures successfully completed in {}",
        out.display()
    );
    Ok(())
}
