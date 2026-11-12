use glam::Vec3;

#[test]
#[ignore]
fn live_space_flag_models_replace_home_on_all_maps() {
    let root = game::assets::baked_root();
    let mut renderer = render::Renderer::new_headless(640, 480).unwrap();
    renderer.cull = false;
    for name in [
        "bf2_spa1", "bf2_spa3", "bf2_spa9", "bf2_spa6", "bf2_spa7", "bf2_spa8",
    ] {
        let (scene, _) = game::assets::load_battle_scene(&root.join(name), "1flag").unwrap();
        let index = scene
            .manifest
            .instances
            .iter()
            .position(|i| i.class.eq_ignore_ascii_case("com_icon_spaceflag"))
            .unwrap();
        let home = Vec3::from(scene.manifest.instances[index].translation);
        let zones: Vec<_> = scene
            .manifest
            .instances
            .iter()
            .enumerate()
            .filter(|(_, i)| i.class.eq_ignore_ascii_case("com_prop_ctfdrop_space"))
            .map(|(i, instance)| (i as u32, Vec3::from(instance.translation)))
            .collect();
        assert_eq!(zones.len(), 2, "{name}");
        let mut gpu = game::viewer::upload_scene(&mut renderer, &scene).unwrap();
        let mut visuals = game::viewer::VehicleVisuals::load(&mut renderer, &scene, &root, &[]);
        let mut sim = core::sim::Sim::empty(292);
        let player = sim.spawn(&core::unit::UnitSpec::trooper(0, home)).unwrap();
        sim.ctf = Some(core::sim::CtfState::new_space_one_flag(
            5, home, zones[0].1, zones[1].1,
        ));
        gpu.pose_ctf(&sim, player);
        let color = |gpu: &game::viewer::SceneGpu, index| {
            let range = &gpu
                .instance_draws
                .iter()
                .find(|(i, _)| *i == index)
                .unwrap()
                .1;
            gpu.draws[range.start].color
        };
        let blue = color(&gpu, zones[0].0);
        let red = color(&gpu, zones[1].0);
        assert!(blue[2] > blue[0] && red[0] > red[2], "{name}");
        let opponent = sim.spawn(&core::unit::UnitSpec::trooper(1, home)).unwrap();
        gpu.pose_ctf(&sim, opponent);
        assert_eq!(color(&gpu, zones[0].0), red, "{name}");
        assert_eq!(color(&gpu, zones[1].0), blue, "{name}");
        gpu.pose_ctf(&sim, player);
        let ranges: Vec<_> = gpu
            .instance_draws
            .iter()
            .filter(|(i, _)| *i == index as u32)
            .collect();
        assert!(!ranges.is_empty(), "{name}");
        for (_, range) in ranges {
            assert!(
                gpu.draws[range.clone()]
                    .iter()
                    .all(|d| d.fade.is_some_and(|f| f.opacity == 0.0)),
                "{name}"
            );
        }
        let at_home = visuals.draws(&mut renderer, &sim, &[]);
        assert!(!at_home.is_empty(), "{name} home flag model missing");
        assert!(at_home
            .iter()
            .all(|d| d.model.w_axis.truncate().distance(home) < 0.01));
        let moved = home + Vec3::new(80.0, 20.0, 30.0);
        let flag = &mut sim.ctf.as_mut().unwrap().flags[0];
        flag.status = core::sim::FlagStatus::Dropped {
            timer: 10.0,
            pickup_cooldown: 0.0,
        };
        flag.current_pos = moved;
        let dropped = visuals.draws(&mut renderer, &sim, &[]);
        assert!(!dropped.is_empty(), "{name} dropped flag model missing");
        assert!(dropped
            .iter()
            .all(|d| d.model.w_axis.truncate().distance(moved) < 0.01));
        sim.ctf.as_mut().unwrap().flags[0].status =
            core::sim::FlagStatus::Carried { carrier: player };
        let carried = visuals.draws(&mut renderer, &sim, &[]);
        assert!(!carried.is_empty(), "{name} carried flag model missing");
        assert!(carried
            .iter()
            .all(|d| d.model.w_axis.truncate().distance(moved) < 0.01));
        if name == "bf2_spa8" {
            let out = root.parent().unwrap().join("artifacts/lead_tmp/issue-144");
            std::fs::create_dir_all(&out).unwrap();
            for (label, draws, pos) in [
                ("home", at_home, home),
                ("dropped", dropped, moved),
                ("carried", carried, moved),
            ] {
                let offset = if label == "carried" {
                    Vec3::new(1.5, 0.5, 3.0)
                } else {
                    Vec3::new(20.0, 10.0, 35.0)
                };
                let target = if label == "carried" {
                    pos + Vec3::new(0.0, -0.75, -1.88)
                } else {
                    pos
                };
                let camera = game::viewer::look_at(target + offset, target);
                renderer
                    .render(&camera, &[], [0.01, 0.01, 0.01, 1.0])
                    .unwrap();
                let (background, _, _) = renderer.capture_rgba().unwrap();
                renderer
                    .render(&camera, &draws, [0.01, 0.01, 0.01, 1.0])
                    .unwrap();
                let (rgba, w, h) = renderer.capture_rgba().unwrap();
                let changed = rgba
                    .chunks_exact(4)
                    .zip(background.chunks_exact(4))
                    .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > 8))
                    .count();
                assert!(
                    changed > 50,
                    "{label} model did not visibly render: {changed} pixels"
                );
                std::fs::write(
                    out.join(format!("port-isolated-{label}.png")),
                    render::encode_png(&rgba, w, h).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
