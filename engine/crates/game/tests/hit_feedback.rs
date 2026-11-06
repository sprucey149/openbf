//! Issue #104: real baked soldier hit overlays in two offscreen map scenes.
//! Run via scripts/gpu_tests.sh --hit-feedback. Outputs remain local assets.
use game::animator::{body_vel, AnimInput, Animator};
use glam::{Mat4, Vec3};
use std::path::PathBuf;

#[test]
#[ignore = "requires fresh side bakes, maps and headless GPU"]
fn hit_feedback_captures_all_four_sides_on_bf1_and_bf2_maps() {
    let sides = PathBuf::from(std::env::var_os("OPENBF_HIT_SIDES").expect("fresh side bakes"));
    let out = PathBuf::from(std::env::var_os("OPENBF_HIT_CAPTURES").expect("owned capture output"));
    std::fs::create_dir_all(&out).unwrap();
    let root = game::assets::baked_root();
    game::assets::set_texture_tier(game::assets::TextureTier::Ps2);
    for game in ["bf1", "bf2"] {
        let (scene, _) =
            game::assets::load_battle_scene(&root.join(format!("{game}_hot1")), "con").unwrap();
        let mut renderer = render::Renderer::new_headless(960, 720).unwrap();
        let gpu = game::viewer::upload_scene(&mut renderer, &scene).unwrap();
        let (camera, stand) = game::gallery::ground_view(&scene);
        let side = sides.join(format!("side_{game}"));
        let asset = game::assets::load_skinned_glb(&side.join("soldiers/imp_inf_stormtrooper.glb"))
            .unwrap();
        let rig =
            game::soldier_vis::SoldierVisuals::load_rig(&side, "imp_inf_stormtrooper", &asset)
                .unwrap();
        let parts: Vec<_> = game::viewer::upload_skinned(&mut renderer, &asset.mesh)
            .unwrap()
            .into_iter()
            .map(|(mesh, texture, layer)| {
                let texture = texture.map(|t| {
                    game::viewer::upload_texture_file(
                        &mut renderer,
                        &side.join("textures"),
                        &t.to_ascii_lowercase(),
                    )
                    .unwrap()
                    .expect("soldier texture")
                });
                (mesh, texture, layer)
            })
            .collect();
        let mut walk = game::walk::WalkSim::new(42, stand, None);
        let unit = walk.player;
        let yaw = camera.yaw;
        walk.sim.units[unit.index as usize].soldier.yaw = yaw;
        let input = AnimInput {
            stance: core::soldier::Stance::Stand,
            vel: body_vel(Vec3::ZERO, yaw),
            yaw,
            mouse_aim: false,
            max_speed: 7.0,
            root_scale: 1.0,
            dead: false,
            hero_dead: false,
            hard_lock: false,
            set: rig.weapon_set("human", "rifle"),
            fired: false,
            throw: None,
            reloading: false,
            reload_time: 1.0,
            alert: false,
            aim: (0.0, 0.0),
        };
        let model = Mat4::from_translation(stand) * Mat4::from_rotation_y(yaw);
        let mut base = Animator::new();
        base.step(&rig, &asset.clips, &asset.skeleton, &input, 0.0);
        for (name, dir) in [
            ("front", Vec3::Z),
            ("left", Vec3::X),
            ("back", Vec3::NEG_Z),
            ("right", Vec3::NEG_X),
        ] {
            let mut animator = base.clone();
            let mut plain = base.clone();
            plain.step(&rig, &asset.clips, &asset.skeleton, &input, 1.0 / 60.0);
            walk.sim.events = vec![core::sim::Event::Hit {
                direct: true,
                shooter: unit,
                victim: unit,
                damage: 0.0,
                health_left: 100.0,
                age: 0.0,
                dir: glam::Quat::from_rotation_y(yaw) * dir,
            }];
            let hash = walk.sim.hash();
            animator.note_hit(&walk.sim, 0);
            // Second rendered update, inside the 0.125-second fade.
            animator.step(&rig, &asset.clips, &asset.skeleton, &input, 1.0 / 60.0);
            animator.step(&rig, &asset.clips, &asset.skeleton, &input, 0.0);
            assert_eq!(walk.sim.hash(), hash);
            let hit = animator.pose(&asset.skeleton);
            assert_ne!(
                hit.0,
                plain.pose(&asset.skeleton).0,
                "{game} {name}: actual hit clip must resolve"
            );
            for (suffix, pose) in [("idle", plain.pose(&asset.skeleton)), ("hit", hit)] {
                let palette = asset.skeleton.palette(&pose.locals());
                let draws: Vec<_> = parts
                    .iter()
                    .map(|(mesh, texture, layer)| render::SkinnedDrawCmd {
                        mesh: *mesh,
                        texture: *texture,
                        layer: *layer,
                        model,
                        color: [1.0; 4],
                        palette: palette.clone().into(),
                        light: render::LightPath::Character,
                        lit: None,
                    })
                    .collect();
                renderer
                    .render_frame(&camera, &gpu.draws, &draws, gpu.clear, None)
                    .unwrap();
                let (rgba, w, h) = renderer.capture_rgba().unwrap();
                std::fs::write(
                    out.join(format!("{game}_{name}_{suffix}.png")),
                    render::encode_png(&rgba, w, h).unwrap(),
                )
                .unwrap();
            }
        }
        // FPM has no hit-pose column: a direct contact must leave its arms,
        // weapon attachment and camera transform byte-identical at frozen time.
        let mut teams = [
            game::walk::TeamTemplate::default_trooper(),
            game::walk::TeamTemplate::default_trooper(),
        ];
        let class = &mut teams[0].classes[0];
        class.geometry = Some("imp_inf_stormtrooper".into());
        class.first_person = Some("imp_1st_stormtrooper".into());
        class.first_person_weapon = game::first_person::FpWeapon {
            geometry: Some("imp_1st_weap_inf_rifle".into()),
            row: 0,
        };
        let mut visuals =
            game::soldier_vis::SoldierVisuals::load(&mut renderer, &sides, game, &teams);
        walk.sim.units[unit.index as usize].first_person = true;
        walk.sim.events.clear();
        for _ in 0..40 {
            visuals.update(&walk, 1.0 / 30.0);
        }
        let (before, before_guns) = visuals.first_person_draws(&walk, &camera);
        assert!(!before.is_empty(), "{game}: actual FPM arms required");
        assert!(!before_guns.is_empty(), "{game}: actual FPM gun required");
        walk.sim.tick += 1;
        walk.sim.events = vec![core::sim::Event::Hit {
            direct: true,
            shooter: unit,
            victim: unit,
            damage: 30.0,
            health_left: 100.0,
            age: 0.0,
            dir: Vec3::Z,
        }];
        visuals.update(&walk, 0.0);
        let (after, after_guns) = visuals.first_person_draws(&walk, &camera);
        for (a, b) in before.iter().zip(&after) {
            assert_eq!(a.palette, b.palette);
            assert_eq!(a.model, b.model);
        }
        for (a, b) in before_guns.iter().zip(&after_guns) {
            assert_eq!(a.model, b.model);
        }
        for (suffix, arms, guns) in [
            ("idle", &before, &before_guns),
            ("hit", &after, &after_guns),
        ] {
            let mut draws = gpu.draws.clone();
            draws.extend(guns.iter().cloned());
            renderer
                .render_frame(&camera, &draws, arms, gpu.clear, None)
                .unwrap();
            let (rgba, w, h) = renderer.capture_rgba().unwrap();
            std::fs::write(
                out.join(format!("{game}_first_person_{suffix}.png")),
                render::encode_png(&rgba, w, h).unwrap(),
            )
            .unwrap();
        }
    }
}
