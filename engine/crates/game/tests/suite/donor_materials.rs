//! Owned-input regression and headless captures for issue #99.
use game::{assets, viewer};
use glam::Vec3;
use std::path::PathBuf;

#[test]
#[ignore = "owned before/after bakes and GPU; scripts/gpu_tests.sh --game donor_materials"]
fn donor_materials_resolve_and_normal_water_reaches_pixels() {
    let root = PathBuf::from(std::env::var_os("OPENBF_DONOR_BAKES").expect("OPENBF_DONOR_BAKES"));
    let capture =
        PathBuf::from(std::env::var_os("OPENBF_DONOR_CAPTURE").expect("OPENBF_DONOR_CAPTURE"));
    std::fs::create_dir_all(&capture).unwrap();
    let maps = [
        "bf1_kas1", "bf1_kas2", "bf1_nab2", "bf1_yav1", "bf2_dag1", "bf2_dea1", "bf2_kas2",
        "bf2_nab2", "bf2_yav1",
    ];
    let mut checked = 0;
    for profile in ["xbox", "pc1"] {
        for map in maps {
            if profile == "pc1" && map.starts_with("bf1") {
                continue;
            }
            let mut renderer = render::Renderer::new_headless(384, 216).expect("headless GPU");
            let mut images = Vec::new();
            for phase in ["before", "after"] {
                let dir = root.join(format!("{profile}-{phase}")).join(map);
                let scene = assets::load_scene(&dir).unwrap();
                let water = scene.manifest.water.as_ref().expect("water");
                assert!(water.texture.is_none());
                let n = water.config.as_ref().and_then(|c| c.normal_maps.as_ref());
                if phase == "after" {
                    let n = n.expect("normal material survives parse/bake/load");
                    assert_eq!((n.textures.len(), n.fps), (16, 8.0));
                    assert!(n
                        .textures
                        .iter()
                        .all(|name| scene.textures.contains_key(name)));
                } else {
                    assert!(n.is_none());
                }
                viewer::apply_lighting(&mut renderer, &scene);
                let mut gpu = viewer::upload_scene(&mut renderer, &scene).unwrap();
                let missing: usize = gpu
                    .untextured
                    .iter()
                    .filter(|u| u.0 == "<water>")
                    .map(|u| u.2)
                    .sum();
                assert_eq!(missing, usize::from(phase == "before"));
                let mesh = scene.water.as_ref().unwrap();
                let q = &mesh.indices[..3];
                let centre = q
                    .iter()
                    .map(|&i| Vec3::from(mesh.positions[i as usize]))
                    .sum::<Vec3>()
                    / 3.0;
                // Two eyes on real water geometry, matched across revisions.
                for (view, offset) in [
                    ("near", Vec3::new(0.0, 3.0, 5.0)),
                    ("grazing", Vec3::new(0.0, 1.0, 12.0)),
                ] {
                    let camera = game::walk::look_camera(
                        centre + offset,
                        -offset.normalize(),
                        55_f32.to_radians(),
                    );
                    let mut times = Vec::new();
                    for time in [0.0, 0.5] {
                        renderer.time = time;
                        gpu.animate_water(&mut renderer, time as f64, camera.position)
                            .unwrap();
                        let draws: Vec<_> = gpu
                            .draws
                            .iter()
                            .filter(|d| d.layer == render::DrawLayer::Water || d.texture.is_none())
                            .copied()
                            .filter(|d| {
                                d.light == render::LightPath::WaterNormal
                                    || d.color == [0.16, 0.28, 0.34, 1.0]
                            })
                            .collect();
                        assert!(!draws.is_empty());
                        renderer
                            .render(&camera, &draws, [0.06, 0.07, 0.08, 1.0])
                            .unwrap();
                        let (rgba, w, h) = renderer.capture_rgba().unwrap();
                        std::fs::write(
                            capture.join(format!("{profile}-{map}-{phase}-{view}-{time}.png")),
                            render::encode_png(&rgba, w, h).unwrap(),
                        )
                        .unwrap();
                        times.push(rgba);
                    }
                    if phase == "after" {
                        let changed = times[0]
                            .chunks_exact(4)
                            .zip(times[1].chunks_exact(4))
                            .filter(|(a, b)| a != b)
                            .count();
                        assert!(
                            changed > 10,
                            "{profile}/{map}/{view}: no visible normal animation ({changed})"
                        );
                        println!("NORMAL_PIXELS {profile} {map} {view} {changed}");
                    }
                    images.push(times.remove(0));
                }
                if phase == "after" && profile == "xbox" && map == "bf2_dag1" {
                    let mut incomplete = assets::load_scene(&dir).unwrap();
                    let last = incomplete
                        .manifest
                        .water
                        .as_ref()
                        .unwrap()
                        .config
                        .as_ref()
                        .unwrap()
                        .normal_maps
                        .as_ref()
                        .unwrap()
                        .textures[15]
                        .clone();
                    incomplete.textures.remove(&last);
                    let fallback = viewer::upload_scene(&mut renderer, &incomplete).unwrap();
                    assert!(!fallback
                        .draws
                        .iter()
                        .any(|d| d.light == render::LightPath::WaterNormal));
                    assert_eq!(
                        fallback
                            .untextured
                            .iter()
                            .filter(|u| u.0 == "<water>")
                            .map(|u| u.2)
                            .sum::<usize>(),
                        1
                    );
                }
                println!("WATER {profile} {map} {phase} missing={missing}");
            }
            for view in 0..2 {
                assert_ne!(
                    images[view],
                    images[view + 2],
                    "normal path did not change pixels"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 14);
    for profile in ["ps2", "xbox", "pc1"] {
        let dir = root.join(format!("{profile}-after")).join("bf2_spa3");
        let mut scene = assets::load_scene(&dir).unwrap();
        scene.apply_mode_layer("ass");
        assert!(scene.textures.contains_key("dea1_prop_barrel_crate"));
        let mut renderer = render::Renderer::new_headless(64, 64).unwrap();
        let good = viewer::upload_scene(&mut renderer, &scene).unwrap();
        assert!(!good
            .untextured
            .iter()
            .any(|u| u.1 == "dea1_prop_barrel_crate"));
        scene.textures.remove("dea1_prop_barrel_crate");
        let bad = viewer::upload_scene(&mut renderer, &scene).unwrap();
        assert_eq!(
            bad.untextured
                .iter()
                .filter(|u| u.1 == "dea1_prop_barrel_crate")
                .map(|u| u.2)
                .sum::<usize>(),
            1
        );
    }
}

#[test]
#[ignore = "owned before/after map bakes; OPENBF_DONOR_BAKES"]
fn donor_bakes_preserve_gameplay_fields() {
    let root = PathBuf::from(std::env::var_os("OPENBF_DONOR_BAKES").expect("OPENBF_DONOR_BAKES"));
    for profile in ["ps2", "xbox", "pc1"] {
        let before = root.join(format!("{profile}-before"));
        let after = root.join(format!("{profile}-after"));
        let mut count = 0;
        for entry in std::fs::read_dir(&after).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path().join("manifest.ron");
            if !path.is_file() {
                continue;
            }
            let load = |p: &std::path::Path| {
                let m: manifest::Manifest =
                    ron::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
                let mut value = serde_json::to_value(m).unwrap();
                for field in ["textures", "texture_tiers", "texture_tier2", "transparent"] {
                    value.as_object_mut().unwrap().remove(field);
                }
                if let Some(config) = value["water"]["config"].as_object_mut() {
                    config.remove("normal_maps");
                }
                value
            };
            assert_eq!(
                load(&before.join(entry.file_name()).join("manifest.ron")),
                load(&path),
                "non-material manifest change: {profile}/{:?}",
                entry.file_name()
            );
            let recovered = root
                .join(format!("{profile}-recovered"))
                .join(entry.file_name())
                .join("manifest.ron");
            if recovered.is_file() {
                assert_eq!(
                    load(&path),
                    load(&recovered),
                    "supplied-copy gameplay drift"
                );
                println!(
                    "RECOVERED_GAMEPLAY_FIELDS {profile}/{:?}",
                    entry.file_name()
                );
            }
            count += 1;
        }
        assert_eq!(count, 40, "all ground and space maps must be checked");
        println!("GAMEPLAY_FIELDS {profile} {count}");
    }
}

#[test]
#[ignore = "owned donor bakes and GPU; scripts/gpu_tests.sh --game donor_slot_census"]
fn donor_slot_census() {
    let root = PathBuf::from(std::env::var_os("OPENBF_DONOR_BAKES").expect("OPENBF_DONOR_BAKES"));
    let mut plain_total = 0;
    let mut recovered_total = 0;
    for profile in ["ps2", "xbox", "pc1"] {
        let mut count = 0;
        for entry in std::fs::read_dir(root.join(format!("{profile}-after"))).unwrap() {
            let entry = entry.unwrap();
            if !entry.path().join("manifest.ron").is_file() {
                continue;
            }
            let map = entry.file_name().to_str().unwrap().to_owned();
            let mut renderer = render::Renderer::new_headless(64, 64).expect("headless GPU");
            let mut scene = assets::load_scene(&entry.path()).unwrap();
            scene.apply_mode_layer(if map.starts_with("bf2_spa") {
                "ass"
            } else {
                "con"
            });
            let common = root
                .join(format!("{profile}-after"))
                .join(format!("common_{}", scene.manifest.source_game));
            scene.merge_common(assets::load_common(&common).expect("owned COMMON control"));
            let gpu = viewer::upload_scene(&mut renderer, &scene).unwrap();
            let missing: usize = gpu.untextured.iter().map(|x| x.2).sum();
            if missing > 0 {
                println!("SLOTS {profile}/{map} {missing} {:?}", gpu.untextured);
            }
            plain_total += missing;
            let recovered = root.join(format!("{profile}-recovered")).join(&map);
            if recovered.join("manifest.ron").is_file() {
                let mut scene = assets::load_scene(&recovered).unwrap();
                scene.apply_mode_layer("con");
                scene.merge_common(assets::load_common(&common).expect("owned COMMON control"));
                let gpu = viewer::upload_scene(&mut renderer, &scene).unwrap();
                let missing: usize = gpu.untextured.iter().map(|x| x.2).sum();
                assert_eq!(
                    missing, 0,
                    "recovered {profile}/{map}: {:?}",
                    gpu.untextured
                );
                println!("RECOVERED {profile}/{map} 0");
                recovered_total += missing;
            } else {
                recovered_total += missing;
            }
            count += 1;
        }
        assert_eq!(count, 40);
    }
    assert_eq!(
        plain_total, 13,
        "12 console glow draws + one snowdrift require another copy"
    );
    assert_eq!(recovered_total, 0);
}
