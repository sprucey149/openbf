use core::sim::{Event, Sim};
use core::sticky::Sticky;
use core::unit::UnitHandle;
use game::fx::Fx;
use glam::Vec3;

fn names(fx: &Fx) -> Vec<String> {
    fx.live()
        .iter()
        .filter(|e| !e.is_dead())
        .map(|e| e.name().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_bf1_thermal_detonator_glows_red_in_flight_and_splashes_in_water() {
    let root = game::assets::baked_root();
    let map = root.join("common_bf1");
    if !map.join("effects.ron").exists() || !root.join("classes").exists() {
        eprintln!("SKIP: common_bf1 or the classes are not baked");
        return;
    }
    let db = game::roster::load_classdb(&root.join("classes"), "bf1", &["all", "imp"]).unwrap();
    let w = game::roster::thrown_weapon_from_db(&db, "imp_weap_inf_thermaldetonator")
        .unwrap()
        .unwrap();
    let trail = w.ordnance.impact.as_ref().and_then(|l| l.trail.clone());
    assert_eq!(trail.as_deref(), Some("com_sfx_weap_grenade_red"));
    let class = w.ordnance.sticky.clone().unwrap();
    let look = class.explosion.as_ref().unwrap().look.clone();
    assert_eq!(look.water_effect.as_deref(), Some("watersplash_lg"));
    assert!(look.radius_outer > 0.0, "{look:?}");
    let land = look.effects.clone();
    assert!(!land.is_empty());

    let mut sim = Sim::empty(1);
    sim.water_level = Some(0.0);
    let life = w.ordnance.life_span;
    let wc = sim.add_weapon_class(w);
    sim.stickies.push(Sticky::new(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 4.0, -10.0),
        &class,
        life,
        UnitHandle { index: 0, gen: 0 },
        0,
        wc,
    ));
    let mut fx = Fx::load(&root, &map, "bf1", &["all", "imp"]);
    let mut renderer = render::Renderer::new_headless(384, 384).ok();
    if renderer.is_none() {
        eprintln!("SKIP (GPU part): no GPU adapter");
    }
    let mut blast = None;
    let mut rendered = false;
    for tick in 0..(30.0 * (life + 4.0)) as usize {
        sim.step(&[]);
        fx.ingest_sim(&sim);
        fx.update(core::TICK_DT);
        if let Some(g) = sim.stickies.first() {
            let e = fx
                .live()
                .iter()
                .find(|e| e.name() == trail.as_deref() && !e.is_dead())
                .unwrap_or_else(|| panic!("tick {tick}: no trail on the grenade"));
            assert_eq!(e.frame.pos, g.pos, "tick {tick}: the trail rides the round");
            if tick == 6 {
                if let Some(r) = renderer.as_mut() {
                    let camera = render::Camera {
                        position: g.pos + Vec3::new(0.0, 0.5, 4.0),
                        pitch: -0.1,
                        ..Default::default()
                    };
                    fx.submit(r, &camera);
                    r.render(&camera, &[], [0.01, 0.01, 0.01, 1.0]).unwrap();
                    let (rgba, w, h) = r.capture_rgba().unwrap();
                    let red = rgba
                        .chunks_exact(4)
                        .filter(|p| {
                            i16::from(p[0]) - i16::from(p[1]) > 15
                                && i16::from(p[0]) - i16::from(p[2]) > 15
                        })
                        .count();
                    assert!(
                        red > 20,
                        "only {red} red pixels of the glow reached the frame"
                    );
                    if let Some(dir) = std::env::var_os("OPENBF_WATER_CAPTURE") {
                        let dir = std::path::PathBuf::from(dir);
                        std::fs::create_dir_all(&dir).unwrap();
                        std::fs::write(
                            dir.join("bf1-thermal-glow.png"),
                            render::encode_png(&rgba, w, h).unwrap(),
                        )
                        .unwrap();
                    }
                    rendered = true;
                }
            }
        }
        if let Some(pos) = sim.events.iter().find_map(|e| match e {
            Event::StickyBlast { pos, .. } => Some(*pos),
            _ => None,
        }) {
            blast = Some(pos);
            break;
        }
    }
    let pos = blast.expect("the fuse runs out in the water");
    assert!(pos.y <= 0.3, "{pos}");
    let live = names(&fx);
    assert!(live.iter().any(|n| n == "watersplash_lg"), "{live:?}");
    assert!(!live.iter().any(|n| land.contains(n)), "{live:?}");
    let splash = fx
        .live()
        .iter()
        .find(|e| e.name() == Some("watersplash_lg"))
        .unwrap();
    assert_eq!(splash.frame.pos, Vec3::new(pos.x, 0.02, pos.z));
    assert!(fx
        .live()
        .iter()
        .filter(|e| e.name() == trail.as_deref())
        .all(|e| e.is_stopped()));
    assert!(rendered || renderer.is_none());
}

#[test]
fn kas1_carries_its_wmap_mask_and_a_ripple_reaches_the_frame() {
    let root = game::assets::baked_root();
    let dir = root.join("bf1_kas1");
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.ron")) else {
        eprintln!("SKIP: bf1_kas1 is not baked");
        return;
    };
    let m: manifest::Manifest = ron::from_str(&text).unwrap();
    let water = m.water.as_ref().unwrap();
    let wet = water.gameplay_mask().expect("a bake with a water mask");
    let mask =
        core::water::WetMask::new(wet.grid, wet.cell_size, wet.half_extent, &wet.cells()).unwrap();
    assert_eq!(
        (wet.grid, wet.cell_size, mask.wet_cells()),
        (64, 32.0, 4014)
    );
    let at = Vec3::new(210.79, water.height, -38.07);
    assert!(mask.is_wet(at.x, at.z));
    let mut fx = Fx::load(&root, &dir, "bf1", &["all", "imp"]);
    fx.set_water(&m);
    let look = core::impact::ExplosionLook {
        radius_outer: 6.0,
        ..Default::default()
    };
    assert!(!fx.explode(&look, at - Vec3::Y * 0.2, Some(water.height)));
    let r = fx.ripples()[0];
    assert_eq!((r.life, r.size, r.at), (3.4, 0.4, at));
    fx.update(0.5);
    let Ok(mut renderer) = render::Renderer::new_headless(256, 256) else {
        eprintln!("SKIP (GPU part): no GPU adapter");
        return;
    };
    let camera = render::Camera {
        position: at + Vec3::new(0.0, 4.0, 0.0),
        pitch: -1.55,
        ..Default::default()
    };
    let mut lit = |fx: &mut Fx| {
        fx.submit(&mut renderer, &camera);
        renderer.render(&camera, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
        let (rgba, w, h) = renderer.capture_rgba().unwrap();
        if let Some(d) = std::env::var_os("OPENBF_WATER_CAPTURE") {
            let d = std::path::PathBuf::from(d);
            std::fs::create_dir_all(&d).unwrap();
            let png = render::encode_png(&rgba, w, h).unwrap();
            std::fs::write(
                d.join(format!("kas1-ripple-{}.png", fx.ripples().len())),
                png,
            )
            .unwrap();
        }
        rgba.chunks_exact(4)
            .filter(|p| p[..3].iter().any(|&c| c > 12))
            .count()
    };
    let ring = lit(&mut fx);
    assert_eq!(fx.ripples().len(), 1);
    fx.clear();
    let bare = lit(&mut fx);
    assert_eq!(bare, 0);
    assert!(
        ring > 200,
        "only {ring} pixels of the ripple reached the frame"
    );
}

#[test]
#[ignore = "headless GPU; run through scripts/gpu_tests.sh --game water_fx::dagobah"]
fn dagobah_wading_uses_the_authored_effects_and_reaches_the_frame() {
    use game::walk::{ClassVisual, WalkSim};
    let root = game::assets::baked_root();
    let dir = root.join("bf2_dag1");
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: bf2_dag1 is not baked");
        return;
    }
    let mut scene = game::assets::load_scene(&dir).unwrap();
    scene.apply_mode_layer("con");
    let ground = scene.heightfield().unwrap();
    let level = scene.manifest.water.as_ref().unwrap().height;
    let mut walk = WalkSim::new(1, Vec3::ZERO, scene.heightfield());
    scene.install_water(&mut walk.sim);
    walk.sim.units.truncate(1);
    let mut site = None;
    for x in (-96..96).step_by(2) {
        for z in (-96..96).step_by(2) {
            let at = Vec3::new(x as f32, ground.height_at(x as f32, z as f32), z as f32);
            if (0..7).all(|j| {
                let p = at + Vec3::Z * j as f32;
                let depth = level - ground.height_at(p.x, p.z);
                walk.sim.still_water_under(p).is_some() && (0.4..1.3).contains(&depth)
            }) {
                site = Some(at);
                break;
            }
        }
        if site.is_some() {
            break;
        }
    }
    let at = site.expect("a continuous wading site on Dagobah");
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["rep", "cis"]).unwrap();
    let class = game::roster::ClassRoster::build(&db, "rep_inf_ep3_rifleman", 0, None).unwrap();
    let team = walk.sim.units[0].team as usize;
    walk.teams[team].faction = "rep".into();
    walk.teams[1 - team].faction = "cis".into();
    walk.teams[team].classes = vec![ClassVisual::from_roster(&class)];
    let u = &mut walk.sim.units[0];
    u.consts = class.consts;
    u.alive = true;
    u.soldier.pos = at;
    u.soldier.grounded = true;
    u.soldier.vel = Vec3::Z * 7.0;
    u.soldier.yaw = std::f32::consts::PI;
    let mut fx = Fx::for_walk(&root, &dir, "bf2", &walk);
    fx.set_water(&scene.manifest);
    for key in [
        "WaterSplashEffect",
        "FootWaterSplashEffect",
        "WakeWaterSplashEffect",
    ] {
        let name = db.prop(&class.class, key).expect(key);
        assert!(fx.library().unwrap().get(name).is_some(), "{key}: {name}");
        eprintln!("{key}={name}");
    }
    let mut renderer = render::Renderer::new_headless(640, 448).unwrap();
    let gpu = game::viewer::upload_scene(&mut renderer, &scene).unwrap();
    let mut vis = game::soldier_vis::SoldierVisuals::for_walk(&mut renderer, &root, &scene, &walk);
    let mut feet_seen = false;
    for tick in 1..=18 {
        walk.sim.tick = tick;
        let p = at + Vec3::Z * (tick as f32 * core::TICK_DT * 7.0);
        walk.sim.units[0].soldier.pos = Vec3::new(p.x, ground.height_at(p.x, p.z), p.z);
        vis.update(&walk, core::TICK_DT);
        feet_seen |= vis.water_feet(&walk)[0].is_some();
        fx.ingest_posed(&walk, Some(&vis));
        fx.update(core::TICK_DT);
    }
    assert!(
        feet_seen,
        "the soldier's posed foot bones reach water contact"
    );
    assert!(fx.ripples().len() >= 4);
    assert!(fx
        .ripples()
        .iter()
        .all(|r| r.velocity == Some(Vec3::Z * 7.0)));
    assert!(names(&fx).iter().any(|n| n.contains("watersplash")));
    let pos = walk.sim.units[0].soldier.pos;
    let camera = render::Camera {
        position: pos + Vec3::new(0.0, 3.2, 4.0),
        pitch: -0.65,
        ..Default::default()
    };
    let skin = vis.draws(&walk);
    fx.submit(&mut renderer, &camera);
    renderer
        .render_frame(&camera, &gpu.draws, &skin, gpu.clear, None)
        .unwrap();
    let (wet, w, h) = renderer.capture_rgba().unwrap();
    let count = fx.ripples().len();
    fx.clear();
    fx.submit(&mut renderer, &camera);
    renderer
        .render_frame(&camera, &gpu.draws, &skin, gpu.clear, None)
        .unwrap();
    let (dry, _, _) = renderer.capture_rgba().unwrap();
    let changed = wet
        .chunks_exact(4)
        .zip(dry.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > 3))
        .count();
    eprintln!("Dagobah at {pos:?}: {count} ripples, {changed} changed pixels");
    if let Some(out) = std::env::var_os("OPENBF_WATER_CAPTURE") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for (name, rgba) in [("dagobah-wading.png", wet), ("dagobah-no-contact.png", dry)] {
            std::fs::write(out.join(name), render::encode_png(&rgba, w, h).unwrap()).unwrap();
        }
    }
    assert!(
        changed > 40,
        "water contact must be visible in the real map"
    );
}

#[test]
#[ignore = "headless GPU; run through scripts/gpu_tests.sh --game water_fx::dagobah"]
fn dagobah_grass_keeps_the_ps2_texture_and_renders_in_the_scene() {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_dag1");
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: bf2_dag1 is not baked");
        return;
    }
    let mut scene = game::assets::load_scene(&dir).unwrap();
    scene.apply_mode_layer("con");
    let grass = &scene.manifest.foliage.as_ref().unwrap().grass;
    assert_eq!(grass.len(), 1);
    assert_eq!(grass[0].texture, "dag1_grass");
    let camera = render::Camera {
        position: Vec3::new(-9.229933, 6.6619954, -95.428314),
        yaw: 2.254181,
        pitch: -0.372927,
        fov_y: 2.0 * (1.0 / 2.047594_f32).atan(),
        ..Default::default()
    };
    let mut renderer = render::Renderer::new_headless(640, 448).unwrap();
    let gpu = game::viewer::upload_scene(&mut renderer, &scene).unwrap();
    renderer.render(&camera, &gpu.draws, gpu.clear).unwrap();
    let (rgba, w, h) = renderer.capture_rgba().unwrap();
    renderer.set_grass(Vec::new(), Vec::new());
    renderer.render(&camera, &gpu.draws, gpu.clear).unwrap();
    let (bare, _, _) = renderer.capture_rgba().unwrap();
    if let Some(out) = std::env::var_os("OPENBF_WATER_CAPTURE") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            out.join("dagobah-grass.png"),
            render::encode_png(&rgba, w, h).unwrap(),
        )
        .unwrap();
    }
    let changed = rgba
        .chunks_exact(4)
        .zip(bare.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|k| a[k].abs_diff(b[k]) > 3))
        .count();
    eprintln!("Dagobah grass changes {changed} pixels");
    assert!(changed > 200, "grass must reach the frame");
}
