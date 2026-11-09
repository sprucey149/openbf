//! Real-disc class → uploaded mesh → weapon_draws → captured pixel referee.
use core::collision::HullFilter;
use core::projectile::{Bolt, Shooter};
use game::{classdb::ClassDb, soldier_vis::SoldierVisuals, walk::WalkSim};
use glam::{Mat4, Quat, Vec3};

fn round(walk: &WalkSim, class: usize) -> Bolt {
    let o = &walk.sim.weapon_classes[class].ordnance;
    let mut b = Bolt::fire(
        Vec3::ZERO,
        Vec3::Z,
        o.velocity,
        o.life_span,
        Shooter {
            owner: walk.player,
            team: 0,
            weapon_class: class,
            is_player: true,
        },
        None,
        None,
        &walk.sim.collision,
        HullFilter::Static,
    );
    if let Some(c) = &o.missile {
        b.as_missile(o, c, None, &mut core::lcg::GameLcg::new(42));
    }
    b
}

fn guided(walk: &WalkSim, class: usize) -> core::guided::GuidedRocket {
    core::guided::GuidedRocket {
        id: 7,
        owner: walk.player,
        team: 0,
        weapon_class: class,
        seat: None,
        pos: Vec3::ZERO,
        vel: Vec3::Z * 30.0,
        rot: Quat::IDENTITY,
        pitch_rate: 0.0,
        turn_rate: 0.0,
        roll_input: 0.0,
        bank: 0.0,
        health: 1.0,
        energy: 200.0,
        age: 1.0,
        boosted: false,
        prev_buttons: 0,
        pending_end: None,
        blind: 0.0,
    }
}

#[test]
#[ignore = "requires both disc bakes and headless GPU; scripts/gpu_tests.sh --game rocket_visible"]
fn rocket_classes_reach_the_mesh_and_the_pixels() {
    let root = std::env::var_os("OPENBF_ROCKET_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root);
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires both disc bakes");
        return;
    }
    let mut renderer = render::Renderer::new_headless(960, 540).expect("headless GPU");
    let mut total = 0;
    for (game, map) in [("bf1", "bf1_hot1"), ("bf2", "bf2_kas2")] {
        let mut db = ClassDb::default();
        let mut files: Vec<_> = std::fs::read_dir(root.join("classes"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(game))
            .collect();
        files.sort();
        for p in files {
            db.load(&p).unwrap();
        }
        let mut names: Vec<_> = db
            .names()
            .filter(|n| {
                db.get(n).is_some_and(|d| d.kind == "wpnc")
                    && db.prop(n, "OrdnanceName").is_some_and(|o| {
                        db.engine_base(o) == Some("missile")
                            || db.prop(o, "GeometryName") == Some("com_weap_missile")
                            || (["rocket", "missile", "torpedo", "recoilless"]
                                .iter()
                                .any(|part| n.contains(*part))
                                && matches!(db.engine_base(o), Some("bullet" | "bolt" | "shell")))
                    })
            })
            .map(str::to_owned)
            .collect();
        names.sort();
        assert!(
            names.len() >= 15,
            "{game}: unexpectedly small census: {names:?}"
        );
        let expected_no_model: &[&str] = if game == "bf1" {
            &[
                "all_weap_fly_kwing_miss",
                "all_weap_fly_xwing_ptorpedo",
                "cis_weap_inf_wrist_rocket",
            ]
        } else {
            &[
                "all_weap_fly_awing_missile",
                "cis_weap_fly_droidgunship_missile",
                "cis_weap_fly_trifighter_missile",
                "cis_weap_inf_wrist_rocket",
                "cis_weap_inf_wrist_rocket_fett",
                "imp_weap_fly_tieinter_missile",
                "imp_weap_fly_trooptrans_ptorpedo",
                "imp_weap_inf_wrist_rocket",
                "rep_weap_inf_emp_launcher",
                "tur_weap_recoilless_kas",
            ]
        };
        for name in expected_no_model {
            assert!(
                names.iter().any(|n| n == name),
                "{game} {name}: census omitted faithful trail-only round"
            );
            let ord = db.prop(name, "OrdnanceName").unwrap();
            assert_eq!(
                db.prop(ord, "GeometryName"),
                None,
                "{game} {name}: disc model changed"
            );
        }
        let mut walk = WalkSim::new(42, Vec3::new(0.0, -100.0, 0.0), None);
        let classes: Vec<_> = names
            .iter()
            .map(|n| {
                walk.sim
                    .add_weapon_class(game::roster::weapon_from_db(&db, n).unwrap())
            })
            .collect();
        let scene = game::assets::load_scene(&root.join(map)).unwrap();
        let vis = SoldierVisuals::for_walk(&mut renderer, &root, &scene, &walk);
        for (name, class) in names.iter().zip(classes) {
            let ord = db.prop(name, "OrdnanceName").unwrap();
            let geometry = db.prop(ord, "GeometryName");
            assert_eq!(
                walk.sim.weapon_classes[class].ordnance.geometry.as_deref(),
                geometry,
                "{game} {name}"
            );
            walk.sim.guided.clear();
            walk.sim.bolts.clear();
            let at = if db.engine_base(ord) == Some("flyer") {
                walk.sim.guided.push(guided(&walk, class));
                vec![(
                    class,
                    Mat4::from_rotation_translation(
                        walk.sim.guided[0].drawn_rotation(),
                        Vec3::ZERO,
                    ),
                )]
            } else {
                walk.sim.bolts = vec![round(&walk, class)];
                game::bolt_vis::missiles_in_flight(&walk)
            };
            let draws = vis.weapon_draws(&walk);
            walk.sim.unit_mut(walk.player).unwrap().first_person = true;
            assert_eq!(
                vis.weapon_draws(&walk).len(),
                draws.len(),
                "first person hides ordnance"
            );
            walk.sim.unit_mut(walk.player).unwrap().first_person = false;
            if let Some(geometry) = geometry {
                let model = vis.ordnance_model(class).expect("registered model");
                assert!(
                    !model.parts.is_empty(),
                    "{game} {name}: {geometry} not uploaded"
                );
                let matrix = at.iter().find(|a| a.0 == class).unwrap().1;
                for part in &model.parts {
                    assert!(
                        draws.iter().any(|d| d.mesh == part.0 && d.model == matrix),
                        "{game} {name}: missing draw"
                    );
                }
                // The same mesh must survive the own-shot and enemy-shot paths.
                if let Some(b) = walk.sim.bolts.first_mut() {
                    b.owner.index += 1000;
                    b.team = 1;
                } else {
                    walk.sim.guided[0].owner.index += 1000;
                    walk.sim.guided[0].team = 1;
                }
                assert_eq!(vis.weapon_draws(&walk).len(), draws.len());
            } else {
                assert!(vis.ordnance_model(class).is_none());
                assert!(at.is_empty());
            }
            println!(
                "ROCKET {game} {name} -> {ord} base={} model={} drawn={}",
                db.engine_base(ord).unwrap(),
                geometry.unwrap_or("NONE (disc)"),
                geometry.is_some()
            );
            total += 1;
            if ![
                "rep_weap_inf_rocket_launcher",
                "imp_weap_inf_torpedo_launcher",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            walk.sim.guided.clear();
            walk.sim.bolts = vec![round(&walk, class)];
            let mut fx =
                game::fx::Fx::load(&root, &root.join(map), game, &["all", "imp", "rep", "cis"]);
            // An actual moving trail, ending at the origin; don't stack a second effect at the head.
            for tick in 0..18 {
                walk.sim.tick = tick;
                walk.sim.bolts[0].pos =
                    Vec3::Z * (tick as f32 - 17.0) * walk.sim.bolts[0].vel.length() * core::TICK_DT;
                walk.sim.bolts[0].age = tick as f32 * core::TICK_DT;
                fx.ingest_sim(&walk.sim);
                fx.update(core::TICK_DT);
            }
            assert!(fx.live().iter().any(|e| !e.is_dead()), "real trail missing");
            let model = vis.ordnance_model(class).unwrap();
            let draws: Vec<_> = vis
                .weapon_draws(&walk)
                .into_iter()
                .filter(|d| model.parts.iter().any(|p| p.0 == d.mesh))
                .collect();
            if game == "bf2" {
                let camera =
                    game::walk::look_camera(Vec3::Z * 90.0, Vec3::NEG_Z, 55_f32.to_radians());
                let mut bright = Vec::new();
                for zoom in [1.0, 2.0] {
                    fx.submit_zoomed(&mut renderer, &camera, zoom, Vec::new());
                    renderer.render(&camera, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
                    let (rgba, _, _) = renderer.capture_rgba().unwrap();
                    bright.push(
                        rgba.chunks_exact(4)
                            .filter(|p| p[2] > 150 && i16::from(p[2]) - i16::from(p[0]) > 40)
                            .count(),
                    );
                }
                println!("ZOOM bright blue pixels at 90m: {bright:?}");
                assert!(
                    bright[1] > bright[0],
                    "scoped far fade never reached the renderer"
                );
            }
            for (view, eye) in [("side", Vec3::X * 8.0), ("head", Vec3::Z * 15.0)] {
                let camera = game::walk::look_camera(eye, -eye.normalize(), 55_f32.to_radians());
                let (lo, hi) = model.bounds.expect("uploaded model bounds");
                let projection = camera.view_proj(960.0 / 540.0) * draws[0].model;
                let mut min = glam::Vec2::splat(f32::INFINITY);
                let mut max = glam::Vec2::splat(f32::NEG_INFINITY);
                for x in [lo.x, hi.x] {
                    for y in [lo.y, hi.y] {
                        for z in [lo.z, hi.z] {
                            let p = projection.project_point3(Vec3::new(x, y, z));
                            let pixel = glam::Vec2::new((p.x + 1.0) * 480.0, (1.0 - p.y) * 270.0);
                            min = min.min(pixel);
                            max = max.max(pixel);
                        }
                    }
                }
                let mut frames = Vec::new();
                for mode in [
                    "bare",
                    "model",
                    "combined",
                    "trail",
                    "map-bare",
                    "map-model",
                    "map-combined",
                    "map-trail",
                ] {
                    if mode.starts_with("map-") {
                        game::viewer::apply_lighting(&mut renderer, &scene);
                    } else {
                        renderer.lighting = render::Lighting::default();
                        renderer.set_frame_filter(None);
                        renderer.set_bloom(None);
                    }
                    if ["combined", "trail", "map-combined", "map-trail"].contains(&mode) {
                        fx.submit_walk(&mut renderer, &camera, &walk, Vec::new());
                    } else {
                        renderer.set_particles(Vec::new());
                    }
                    let visible =
                        if ["model", "combined", "map-model", "map-combined"].contains(&mode) {
                            draws.as_slice()
                        } else {
                            &[]
                        };
                    renderer
                        .render(&camera, visible, [0.08, 0.12, 0.18, 1.0])
                        .unwrap();
                    let (rgba, w, h) = renderer.capture_rgba().unwrap();
                    if let Some(dir) = std::env::var_os("OPENBF_ROCKET_CAPTURE") {
                        let dir = std::path::PathBuf::from(dir);
                        std::fs::create_dir_all(&dir).unwrap();
                        std::fs::write(
                            dir.join(format!("{game}-{name}-{view}-{mode}.png")),
                            render::encode_png(&rgba, w, h).unwrap(),
                        )
                        .unwrap();
                    }
                    frames.push(rgba);
                }
                let changed = |a: usize, b: usize| {
                    frames[a]
                        .chunks_exact(4)
                        .zip(frames[b].chunks_exact(4))
                        .enumerate()
                        .filter(|(i, (a, b))| {
                            let x = (i % 960) as f32;
                            let y = (i / 960) as f32;
                            a != b
                                && x >= min.x.floor()
                                && x <= max.x.ceil()
                                && y >= min.y.floor()
                                && y <= max.y.ceil()
                        })
                        .count()
                };
                let body = changed(0, 1);
                let with_trail = changed(2, 3);
                println!(
                    "PIXELS {game} {name} {view}: body={body} combined-minus-trail={with_trail}"
                );
                assert!(body > 5, "model has no pixels");
                assert!(with_trail > 5, "trail hides the model");
                let map_body = changed(4, 5);
                let map_trail = changed(6, 7);
                println!("MAPPIXELS {game} {name} {view}: body={map_body} combined-minus-trail={map_trail}");
                assert!(
                    map_body > 5 && map_trail > 5,
                    "map lighting/filter/bloom hides the model"
                );
            }
            // Exercise the real display loop too: emit at the interpolated
            // position, then draw the model and particles from that same pose.
            game::viewer::apply_lighting(&mut renderer, &scene);
            walk.sim.tick = 0;
            walk.blend.reset();
            walk.blend.on = true;
            walk.sim.bolts = vec![round(&walk, class)];
            let mut moving =
                game::fx::Fx::load(&root, &root.join(map), game, &["all", "imp", "rep", "cis"]);
            for frame in 0..25 {
                walk.advance_with(1.0 / 60.0, &[], |w| moving.ingest_sim(&w.sim));
                moving.advance_frame(None, &mut walk, &core::sim::PadState::default(), 1.0 / 60.0);
                if frame < 23 {
                    continue;
                }
                let hash = walk.sim.hash();
                game::render_blend::blend_in_looking(&mut walk, &core::sim::PadState::default());
                let pos = walk.sim.bolts[0].pos;
                for (view, offset) in [("side", Vec3::X * 8.0), ("head", Vec3::Z * 15.0)] {
                    let camera = game::walk::look_camera(
                        pos + offset,
                        -offset.normalize(),
                        55_f32.to_radians(),
                    );
                    moving.submit_walk(&mut renderer, &camera, &walk, Vec::new());
                    let shown: Vec<_> = vis
                        .weapon_draws(&walk)
                        .into_iter()
                        .filter(|d| model.parts.iter().any(|p| p.0 == d.mesh))
                        .collect();
                    assert!(!shown.is_empty());
                    renderer
                        .render(&camera, &shown, [0.08, 0.12, 0.18, 1.0])
                        .unwrap();
                    if let Some(dir) = std::env::var_os("OPENBF_ROCKET_CAPTURE") {
                        let (rgba, w, h) = renderer.capture_rgba().unwrap();
                        std::fs::write(
                            std::path::PathBuf::from(dir)
                                .join(format!("{game}-{name}-{view}-flight-{frame}.png")),
                            render::encode_png(&rgba, w, h).unwrap(),
                        )
                        .unwrap();
                    }
                }
                walk.blend_out();
                assert_eq!(walk.sim.hash(), hash);
            }
            walk.blend.reset();
            walk.blend.on = false;
            renderer.lighting = render::Lighting::default();
            renderer.set_frame_filter(None);
            renderer.set_bloom(None);
        }
    }
    println!("ROCKET census checked {total} real weapon classes");
}
