//! Presentation companions must reach world upload without becoming camera or sim inputs.
use game::assets::{self, presentation};
use game::camera_collide::CameraBlockers;
use glam::Vec3;
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("openbf-presentation-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_glb(dense: bool) -> Vec<u8> {
    let positions: [[f32; 3]; 5] = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0.5, 0.5, 0.],
    ];
    let indices: &[u32] = if dense {
        &[0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4]
    } else {
        &[0, 1, 2, 0, 2, 3]
    };
    let mut bin: Vec<u8> = positions
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    bin.extend(indices.iter().flat_map(|v| v.to_le_bytes()));
    let normals_at = bin.len();
    let normals = [[0.0_f32, 0.0, 1.0]; 5];
    bin.extend(normals.iter().flatten().flat_map(|v| v.to_le_bytes()));
    let uv_at = bin.len();
    bin.extend(
        positions
            .iter()
            .flat_map(|p| [p[0], p[1]])
            .flat_map(|v| v.to_le_bytes()),
    );
    let doc = serde_json::json!({
        "asset":{"version":"2.0"}, "buffers":[{"byteLength":bin.len()}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":60},
                       {"buffer":0,"byteOffset":60,"byteLength":indices.len()*4},
                       {"buffer":0,"byteOffset":normals_at,"byteLength":uv_at-normals_at},
                       {"buffer":0,"byteOffset":uv_at,"byteLength":bin.len()-uv_at}],
        "accessors":[{"bufferView":0,"count":5,"componentType":5126,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
                     {"bufferView":1,"count":indices.len(),"componentType":5125,"type":"SCALAR"},
                     {"bufferView":2,"count":5,"componentType":5126,"type":"VEC3"},
                     {"bufferView":3,"count":5,"componentType":5126,"type":"VEC2"}],
        "materials":[{"name":"wall","alphaMode":"OPAQUE"}],
        "meshes":[{"name":"end_prop_rocks","extras":{"sphere":[0.5,0.5,0,1]},
                    "primitives":[{"attributes":{"POSITION":0,"NORMAL":2,"TEXCOORD_0":3},"indices":1,"material":0,"mode":4}]}],
        "nodes":[{"mesh":0}],"scenes":[{"nodes":[0]}],"scene":0
    });
    bake::glb::pack_glb(&serde_json::to_vec(&doc).unwrap(), &bin)
}

fn write_pack(assets: &Path, mapping: &Path, out: &Path, installed: bool) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_game"));
    cmd.arg("--write-pack").arg(out);
    if installed {
        cmd.arg("--replacements").arg(mapping);
    }
    cmd.arg(format!("assets_baked={}", assets.display()))
        .output()
        .unwrap()
}

#[test]
fn presentation_writer_keeps_stock_and_rejects_unsupported_inputs() {
    let temp = Temp::new();
    let assets = temp.0.join("assets");
    let map = assets.join("bf1_end1");
    std::fs::create_dir_all(map.join("models")).unwrap();
    let manifest = r#"(map:"end1",source_game:"bf1",terrain_name:None,sky_name:None,terrain_glb:None,
        models:["end_prop_rocks"],textures:[],classes:[(name:"rock",base:"prop",geometry:Some("rock"))],
        geometries:[(name:"rock",model:"end_prop_rocks",lods:[])],regions:[],
        instances:[(class:"rock",name:"one",rotation:((1,0,0),(0,1,0),(0,0,1)),translation:(0,0,0),team:None,props:[])])"#;
    std::fs::write(map.join("manifest.ron"), manifest).unwrap();
    let stock = map.join("models/end_prop_rocks.glb");
    std::fs::write(&stock, fixture_glb(false)).unwrap();
    let payload = temp.0.join("payload.glb");
    std::fs::write(&payload, fixture_glb(true)).unwrap();
    let target = "assets_baked/bf1_end1/presentation/models/end_prop_rocks.glb";
    let mapping = temp.0.join("mapping.json");
    let rows = |name: &str| serde_json::json!([[name, payload]]);
    std::fs::write(&mapping, rows(target).to_string()).unwrap();
    let mut parent_entries = pak::collect(&assets, "assets_baked").unwrap();
    let error = pak::replace_sources(&mut parent_entries, &mapping).unwrap_err();
    assert!(error
        .to_string()
        .contains("replacement archive entry does not exist"));
    let out = temp.0.join("openbf");
    let result = write_pack(&assets, &mapping, &out, true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let archive = pak::Archive::open(&out).unwrap().unwrap();
    assert_eq!(archive.len(), 3);
    assert_eq!(
        archive
            .read_entry("assets_baked/bf1_end1/models/end_prop_rocks.glb")
            .unwrap(),
        fixture_glb(false)
    );
    assert_eq!(archive.read_entry(target).unwrap(), fixture_glb(true));
    let scene = assets::load_scene(&map).unwrap();
    assert!(presentation::load_model(&scene, "end_prop_rocks")
        .unwrap()
        .is_none());
    let companion = map.join("presentation/models/end_prop_rocks.glb");
    std::fs::create_dir_all(companion.parent().unwrap()).unwrap();
    std::fs::copy(&payload, &companion).unwrap();
    assert_eq!(
        presentation::load_model(&scene, "end_prop_rocks")
            .unwrap()
            .unwrap()
            .indices
            .len(),
        12
    );
    assert_eq!(scene.meshes["end_prop_rocks"].indices.len(), 6);
    std::fs::remove_file(&companion).unwrap();
    let before = std::fs::read(&out).unwrap();
    for bad in [
        rows("assets_baked/bf1_end1/presentation/models/unknown.glb"),
        serde_json::json!([[target, payload], [target, payload]]),
    ] {
        std::fs::write(&mapping, bad.to_string()).unwrap();
        assert!(!write_pack(&assets, &mapping, &out, true).status.success());
        assert_eq!(std::fs::read(&out).unwrap(), before);
    }
    std::fs::write(&mapping, rows(target).to_string()).unwrap();
    for bad in [
        manifest.replace("base:\"prop\"", "base:\"door\""),
        manifest.replace("class:\"rock\"", "class:\"unknown\""),
        manifest
            .replace("source_game:\"bf1\"", "source_game:\"bf2\"")
            .replace(
                "geometry:Some(\"rock\"))],",
                "geometry:Some(\"rock\")),(name:\"alias\",base:\"prop\",build_zone:Some(\"rock\"))],",
            )
            .replace(
                "props:[])])",
                "props:[]),(class:\"alias\",name:\"two\",rotation:((1,0,0),(0,1,0),(0,0,1)),translation:(0,0,0),team:None,props:[])])",
            ),
    ] {
        std::fs::write(map.join("manifest.ron"), bad).unwrap();
        assert!(!write_pack(&assets, &mapping, &out, true).status.success());
        assert_eq!(std::fs::read(&out).unwrap(), before);
    }
    std::fs::write(map.join("manifest.ron"), manifest).unwrap();
    // A morph target is refused before general import, even on an allowlisted path.
    let raw = fixture_glb(true);
    let len = u32::from_le_bytes(raw[12..16].try_into().unwrap()) as usize;
    let mut doc: serde_json::Value = serde_json::from_slice(&raw[20..20 + len]).unwrap();
    let mut missing_uv = doc.clone();
    missing_uv["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .remove("TEXCOORD_0");
    std::fs::write(
        &payload,
        bake::glb::pack_glb(&serde_json::to_vec(&missing_uv).unwrap(), &raw[28 + len..]),
    )
    .unwrap();
    let result = write_pack(&assets, &mapping, &out, true);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("normals and texture coordinates"));
    assert_eq!(std::fs::read(&out).unwrap(), before);

    doc["meshes"][0]["primitives"][0]["targets"] = serde_json::json!([{"POSITION":0}]);
    std::fs::write(
        &payload,
        bake::glb::pack_glb(&serde_json::to_vec(&doc).unwrap(), &raw[28 + len..]),
    )
    .unwrap();
    assert!(!write_pack(&assets, &mapping, &out, true).status.success());
    assert_eq!(std::fs::read(&out).unwrap(), before);
    assert!(write_pack(&assets, &mapping, &out, false).status.success());
    assert_eq!(pak::Archive::open(&out).unwrap().unwrap().len(), 2);
}

fn owned_cases() -> Vec<serde_json::Value> {
    serde_json::from_str(
        &std::env::var("OPENBF_PRESENTATION_CASES").expect("explicit owned fixture list"),
    )
    .unwrap()
}

#[test]
#[ignore = "set OPENBF_PRESENTATION_CASES to owned assets/mapping/out objects"]
fn presentation_owned_camera_and_archive() {
    for case in owned_cases() {
        let path = |key| PathBuf::from(case[key].as_str().unwrap());
        let (root, mapping, out) = (path("assets"), path("mapping"), path("out"));
        let rows: Vec<(String, PathBuf)> =
            serde_json::from_slice(&std::fs::read(&mapping).unwrap()).unwrap();
        let stock_entries = pak::collect(&root, "assets_baked").unwrap();
        let mut installed_bytes = None;
        for installed in [false, true, true, false] {
            let result = write_pack(&root, &mapping, &out, installed);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let archive = pak::Archive::open(&out).unwrap().unwrap();
            assert_eq!(
                archive.len(),
                stock_entries.len() + if installed { rows.len() } else { 0 }
            );
            for (name, file) in &stock_entries {
                assert_eq!(
                    archive.read_entry(name).unwrap(),
                    std::fs::read(file).unwrap(),
                    "{name}"
                );
            }
            if installed {
                let bytes = std::fs::read(&out).unwrap();
                if let Some(previous) = &installed_bytes {
                    assert_eq!(&bytes, previous);
                }
                installed_bytes = Some(bytes);
                for (name, payload) in &rows {
                    assert_eq!(
                        archive.read_entry(name).unwrap(),
                        std::fs::read(payload).unwrap()
                    );
                }
            }
        }
        for (target, payload) in rows {
            let relative = target.strip_prefix("assets_baked/").unwrap();
            let stock = presentation::stock_path(relative).unwrap();
            let (map, _) = stock.split_once('/').unwrap();
            let dir = root.join(map);
            let (before, _) = assets::load_battle_scene(&dir, "con").unwrap();
            let model = Path::new(stock).file_stem().unwrap().to_str().unwrap();
            let blockers = CameraBlockers::from_scene(&before);
            let companion = root.join(relative);
            assert!(!companion.exists(), "fixture must start stock");
            std::fs::create_dir_all(companion.parent().unwrap()).unwrap();
            std::fs::copy(&payload, &companion).unwrap();
            let (after, _) = assets::load_battle_scene(&dir, "con").unwrap();
            let visual = presentation::load_model(&after, model).unwrap().unwrap();
            assert_ne!(visual.indices.len(), after.meshes[model].indices.len());
            assert_eq!(before.manifest, after.manifest);
            assert_eq!(
                before.meshes[model].positions,
                after.meshes[model].positions
            );
            assert_eq!(before.meshes[model].indices, after.meshes[model].indices);
            let packed = CameraBlockers::from_scene(&after);
            let mut rays = 0;
            let mut hits = 0;
            let mut poses = 0;
            let mut a = game::walk::WalkSim::new(7, Vec3::ZERO, before.heightfield());
            before.install_collision(&mut a.sim);
            let mut b = game::walk::WalkSim::new(7, Vec3::ZERO, after.heightfield());
            after.install_collision(&mut b.sim);
            for inst in before
                .manifest
                .instances
                .iter()
                .filter(|i| before.resolve_model(&i.class) == Some(model))
            {
                let matrix = game::viewer::instance_matrix(inst.rotation, inst.translation);
                let cpu = &before.meshes[model];
                for tri in cpu.indices.chunks_exact(3) {
                    let p: Vec<_> = tri
                        .iter()
                        .map(|&i| matrix.transform_point3(Vec3::from(cpu.positions[i as usize])))
                        .collect();
                    let Some(n) = (p[1] - p[0]).cross(p[2] - p[0]).try_normalize() else {
                        continue;
                    };
                    let centre = (p[0] + p[1] + p[2]) / 3.;
                    for sign in [-1., 1.] {
                        let from = centre + n * sign * 2.;
                        let to = centre - n * sign * 2.;
                        let a = blockers.pull_in(from, to);
                        assert_eq!(a, packed.pull_in(from, to));
                        assert_eq!(
                            blockers.fraction(from, to, None, |_| None, &[]),
                            packed.fraction(from, to, None, |_| None, &[])
                        );
                        assert_eq!(
                            blockers.see_through(from, &[to], None, |_| None, &[]),
                            packed.see_through(from, &[to], None, |_| None, &[])
                        );
                        hits += usize::from(a.distance(to) > 0.001);
                        rays += 1;
                    }
                }
                // Real chase camera, with the normal stock blocker route and the empty fallback.
                for fallback in [false, true] {
                    a.camera_blockers = if fallback {
                        CameraBlockers::default()
                    } else {
                        CameraBlockers::from_scene(&before)
                    };
                    b.camera_blockers = if fallback {
                        CameraBlockers::default()
                    } else {
                        CameraBlockers::from_scene(&after)
                    };
                    for yaw in [0., 1.2, 2.4, 3.6, 4.8] {
                        for w in [&mut a, &mut b] {
                            let u = w.sim.unit_mut(w.player).unwrap();
                            u.soldier.pos = Vec3::from(inst.translation);
                            u.soldier.yaw = yaw;
                        }
                        assert_eq!(a.camera().position, b.camera().position);
                        assert_eq!(a.camera().yaw, b.camera().yaw);
                        assert_eq!(a.camera().pitch, b.camera().pitch);
                        poses += 1;
                    }
                }
            }
            std::fs::remove_file(&companion).unwrap();
            assert!(rays > 0 && hits > 0 && poses > 0);
            eprintln!(
                "{} {map}/{model}: {rays} camera rays ({hits} hits), {poses} chase poses match",
                case["name"]
            );
        }
    }
}

#[test]
#[ignore = "GPU: set OPENBF_PRESENTATION_CASES; run through scripts/gpu_tests.sh --presentation-meshes"]
fn presentation_owned_world_upload() {
    for case in owned_cases() {
        let root = PathBuf::from(case["assets"].as_str().unwrap());
        let rows: Vec<(String, PathBuf)> =
            serde_json::from_slice(&std::fs::read(case["mapping"].as_str().unwrap()).unwrap())
                .unwrap();
        for (target, payload) in rows {
            let relative = target.strip_prefix("assets_baked/").unwrap();
            let stock = presentation::stock_path(relative).unwrap();
            let (map, _) = stock.split_once('/').unwrap();
            let model = Path::new(stock).file_stem().unwrap().to_str().unwrap();
            let (mut scene, _) = assets::load_battle_scene(&root.join(map), "con").unwrap();
            let inst = scene
                .manifest
                .instances
                .iter()
                .find(|i| scene.resolve_model(&i.class) == Some(model))
                .unwrap()
                .clone();
            let matrix = game::viewer::instance_matrix(inst.rotation, inst.translation);
            let sphere = game::point_light::model_sphere(&scene.meshes[model], matrix).unwrap();
            // Isolate one real stock placement so another object cannot mask a missing draw binding.
            scene.manifest.instances = vec![inst];
            scene.manifest.sky = None;
            scene.manifest.foliage = None;
            scene.terrain = None;
            scene.water = None;
            scene.lowres = None;
            scene.terrain_layers = None;
            let companion = root.join(relative);
            assert!(!companion.exists());
            let mut renderer = render::Renderer::new_headless(640, 480).unwrap();
            let mut frames = Vec::new();
            for installed in [false, true, false] {
                if installed {
                    std::fs::create_dir_all(companion.parent().unwrap()).unwrap();
                    std::fs::copy(&payload, &companion).unwrap();
                } else if companion.exists() {
                    std::fs::remove_file(&companion).unwrap();
                }
                renderer.clear_scene();
                let gpu = game::viewer::upload_scene(&mut renderer, &scene).unwrap();
                assert!(!gpu.draws.is_empty());
                let mut views = Vec::new();
                for direction in [Vec3::new(1., 0.4, 1.), Vec3::new(-1., 0.4, -1.)] {
                    let eye = sphere.centre + direction.normalize() * sphere.radius * 2.5;
                    let camera = game::viewer::look_at(eye, sphere.centre);
                    renderer
                        .render_frame(&camera, &gpu.draws, &[], gpu.clear, None)
                        .unwrap();
                    views.extend(renderer.capture_rgba().unwrap().0);
                }
                frames.push(views);
            }
            assert_eq!(frames[0], frames[2], "removal must restore stock pixels");
            let changed = frames[0]
                .chunks_exact(4)
                .zip(frames[1].chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                changed > 0,
                "{map}/{model}: presentation payload never reached the drawn triangles"
            );
            eprintln!(
                "{} {map}/{model}: {changed} changed pixels across two headless views; stock restored",
                case["name"]
            );
        }
    }
}
