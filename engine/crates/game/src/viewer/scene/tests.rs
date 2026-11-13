use super::*;

/// An ocean step that turns 2 water draws into 130 moves every later draw
/// by 128: the door parts follow, a speckle frame inside the old range
/// (the still plane's) is dropped, and an index before the range stays.
#[test]
fn door_and_speckle_indices_follow_an_ocean_splice() {
    let mut doors = vec![DoorDraw {
        instance: 7,
        model: Mat4::IDENTITY,
        parts: vec![(40..42, Vec::new()), (50..52, Vec::new())],
    }];
    let mut speckle = Some(SpeckleAnim {
        first: vec![3, 11, 60],
        frames: Vec::new(),
        fps: 30.0,
        normal: false,
    });
    let inst = manifest::ManifestInstance {
        name: "t".into(),
        class: "t".into(),
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0; 3],
        props: Vec::new(),
    };
    let rig = manifest::ManifestTurretRig {
        nodes: Vec::new(),
        parts: Vec::new(),
    };
    let mut turrets = vec![super::super::turret_pose::TurretDraw::new(
        9,
        &inst,
        Mat4::IDENTITY,
        &rig,
        vec![(4..6, 0), (60..62, 1)],
        60..64,
    )];
    reindex_after_splice(
        &mut doors,
        &mut turrets,
        &mut [],
        &mut [],
        &mut speckle,
        &(10..12),
        130,
    );
    assert_eq!(turrets[0].parts[0].0, 4..6);
    assert_eq!(turrets[0].parts[1].0, 188..190);
    assert_eq!(turrets[0].whole, 188..192);
    assert_eq!(doors[0].parts[0].0, 168..170);
    assert_eq!(doors[0].parts[1].0, 178..180);
    assert_eq!(speckle.as_ref().unwrap().first, vec![3, 188]);
    // A shrink moves them back; every speckle frame gone drops the layer.
    reindex_after_splice(
        &mut doors,
        &mut turrets,
        &mut [],
        &mut [],
        &mut speckle,
        &(10..140),
        2,
    );
    assert_eq!(doors[0].parts[0].0, 40..42);
    speckle.as_mut().unwrap().first = vec![11];
    reindex_after_splice(
        &mut doors,
        &mut turrets,
        &mut [],
        &mut [],
        &mut speckle,
        &(10..12),
        2,
    );
    assert!(speckle.is_none());
}

/// Only BF1's authored `TerrainEnable(0)` switches the terrain draw off.
#[test]
fn only_bf1_terrain_enable_zero_hides_the_terrain() {
    let sky = |e: Option<f32>| manifest::ManifestSky {
        glow_masks_preserved: false,
        texture: None,
        models: Vec::new(),
        ambient: [1.0; 3],
        low_res_terrain: None,
        low_res_max_distance: None,
        objects: Vec::new(),
        terrain_enable: e,
        reflection_color: None,
        stars: None,
    };
    assert!(!terrain_drawn("bf1", Some(&sky(Some(0.0)))));
    assert!(terrain_drawn("bf2", Some(&sky(Some(0.0)))));
    assert!(terrain_drawn("bf1", Some(&sky(Some(1.0)))));
    assert!(terrain_drawn("bf1", Some(&sky(None))));
    assert!(terrain_drawn("bf1", None));
}

/// Card KSKY: Kamino's `SkyDomeDarkenFactor(0.4)` rests dome shell 0 at
/// 255·(1 − 0.4) = 153 — the byte PS2 RAM holds — while shell 1
/// (`kam1_city`) stays white; a map without the effect darkens nothing.
#[test]
fn lightning_darkens_dome_shell_0_only() {
    let l = manifest::ManifestLightning {
        sky_dome_darken_factor: 0.4,
        sunlight_fade_factor: 0.1,
        color: [255.0; 3],
        brightness_min: 1.0,
        fade_time: 0.2,
        time_between_flashes: [3.0, 5.0],
        time_between_sub_flashes: [0.01, 0.5],
        num_sub_flashes: [2.0, 5.0],
        horizon_angle: [30.0, 60.0],
        sound_crack: None,
        sound_sub_crack: None,
    };
    assert_eq!(l.dome_byte(0.0), 153);
    // A flash at full brightness is white: the hook the weather card animates.
    assert_eq!(l.dome_byte(1.0), 255);
    assert_eq!(l.dome_tint(1.0), 1.0);
    // 153/255 through the sRGB curve.
    let k = ((0.6f32 + 0.055) / 1.055).powf(2.4);
    assert!((l.dome_tint(0.0) - k).abs() < 1e-6, "{}", l.dome_tint(0.0));
    let white = [1.0; 4];
    assert_eq!(sky_shell_tint(Some(&l), 0, white), [k, k, k, 1.0]);
    assert_eq!(sky_shell_tint(Some(&l), 1, white), white);
    assert_eq!(sky_shell_tint(None, 0, white), white);
}

/// A baked terrain mesh: `n x n` world-space grid vertices row-major, two
/// triangles per cell, one material — what `bake::terrain_glb` emits.
fn grid_mesh(n: usize) -> CpuMesh {
    let unit = 8.0f32;
    let half = n as f32 / 2.0 * unit;
    let mut positions = Vec::new();
    for z in 0..n {
        for x in 0..n {
            positions.push([x as f32 * unit - half, 0.0, z as f32 * unit - (half - unit)]);
        }
    }
    let mut indices = Vec::new();
    for z in 0..n - 1 {
        for x in 0..n - 1 {
            let a = (z * n + x) as u32;
            let (b, c, d) = (a + 1, a + n as u32, a + n as u32 + 1);
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let v = positions.len();
    CpuMesh {
        positions,
        normals: vec![[0.0, 1.0, 0.0]; v],
        uvs: vec![[0.0, 0.0]; v],
        colors: vec![[255; 4]; v],
        indices,
        submeshes: vec![(0, 0, "hoth_main_1".into(), render::DrawLayer::Opaque, 0)],
        spec: Vec::new(),
        light: Vec::new(),
        scroll: Vec::new(),
        sphere: None,
        vis_top: None,
        render_grid: None,
        contact_surface: false,
    }
}

/// The chunking the grid walk did before the bake could ship replacement
/// meshes — kept here as the reference the new dealer must reproduce.
fn grid_walk(n: usize) -> Vec<Vec<u32>> {
    let per_row = n.div_ceil(TERRAIN_CHUNK_CELLS).max(1);
    let mut out = Vec::new();
    for cz in 0..per_row {
        for cx in 0..per_row {
            let (x0, z0) = (cx * TERRAIN_CHUNK_CELLS, cz * TERRAIN_CHUNK_CELLS);
            let (x1, z1) = (
                (x0 + TERRAIN_CHUNK_CELLS).min(n - 1),
                (z0 + TERRAIN_CHUNK_CELLS).min(n - 1),
            );
            if x0 >= x1 || z0 >= z1 {
                continue;
            }
            let mut idx = Vec::new();
            for z in z0..z1 {
                for x in x0..x1 {
                    let a = (z * n + x) as u32;
                    let (b, c, d) = (a + 1, a + n as u32, a + n as u32 + 1);
                    idx.extend_from_slice(&[a, c, b, b, c, d]);
                }
            }
            out.push(idx);
        }
    }
    out
}

/// Terrain chunking is now driven by the BAKED index buffer, and on a
/// plain grid that reproduces the old grid walk chunk for chunk, index for
/// index — so no golden moves on the 19 terrains that ship no replacement
/// mesh.
#[test]
fn terrain_chunks_reproduce_the_grid_walk() {
    for n in [64usize, 128] {
        let cpu = grid_mesh(n);
        let got: Vec<Vec<u32>> = terrain_chunk_indices(&cpu)
            .expect("a grid mesh chunks")
            .into_iter()
            .filter(|c| !c.is_empty())
            .collect();
        assert_eq!(got, grid_walk(n), "n = {n}");
    }
}

/// A cutter-clipped patch: the bake drops that patch's quads and appends
/// the shipped replacement vertices after the grid
/// (analysis/re-terrain-cutters.md §0.3). The dropped quads must stay
/// dropped — the defect was drawing them anyway — and the replacement
/// triangle must be dealt into the chunk its world position is in.
#[test]
fn a_replaced_patch_loses_its_quads_and_keeps_the_shipped_triangle() {
    let n = 64usize;
    let mut cpu = grid_mesh(n);
    // The patch is cells 40..48 in x and z: chunk (1, 1) of the 2x2. Its
    // INTERIOR vertices are referenced by its quads and by nothing else,
    // so they are the ones that must disappear.
    let dropped: Vec<u32> = (41..48)
        .flat_map(|z: usize| (41..48).map(move |x| (z * 64 + x) as u32))
        .collect();
    let mut idx = Vec::new();
    for z in 0..n - 1 {
        for x in 0..n - 1 {
            if (40..48).contains(&x) && (40..48).contains(&z) {
                continue; // the patch the munge replaced
            }
            let a = (z * n + x) as u32;
            let (b, c, d) = (a + 1, a + n as u32, a + n as u32 + 1);
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let base = cpu.positions.len() as u32;
    for p in [
        [6.0f32, -3.0, 90.0],
        [20.0, -3.0, 90.0],
        [13.0, -3.0, 104.0],
    ] {
        cpu.positions.push(p);
        cpu.normals.push([0.0, 1.0, 0.0]);
        cpu.uvs.push([0.0, 0.0]);
        cpu.colors.push([255; 4]);
    }
    idx.extend_from_slice(&[base, base + 1, base + 2]);
    cpu.indices = idx;

    let chunks = terrain_chunk_indices(&cpu).expect("chunks");
    let all: Vec<u32> = chunks.concat();
    assert_eq!(all.len(), cpu.indices.len(), "every triangle is dealt once");
    assert!(
        !dropped.iter().any(|v| all.contains(v)),
        "a dropped quad came back"
    );
    // The three appended vertices sit at world (6..20, 90..104) = cells
    // (40..46, 42..46), so their triangle belongs to chunk (1, 1).
    assert!(
        chunks[3].ends_with(&[base, base + 1, base + 2]),
        "chunk (1,1)"
    );
}

/// The whole untextured policy, as three arms. See [`route_untextured`].
#[test]
fn untextured_routing_skips_the_disc_s_own_gaps_and_greys_only_our_misses() {
    // The disc named no texture at all: skipped, not painted.
    assert_eq!(route_untextured(None), Untextured::DiscUntextured);
    // An empty material name is the same thing — `upload_submeshes` maps
    // it to `None`, but the bake writes it as "" and either can arrive.
    assert_eq!(route_untextured(Some("")), Untextured::DiscUntextured);

    // A texture that is on neither disc: skipped, and counted apart.
    for name in DANGLING_ON_DISC {
        assert_eq!(route_untextured(Some(name)), Untextured::Dangling, "{name}");
    }
    // Matched case-insensitively — the discs spell these `yav_prop_ivy`,
    // `Cor1_bldg_flowers` and `noIcon`.
    assert_eq!(
        route_untextured(Some("Cor1_bldg_flowers")),
        Untextured::Dangling
    );
    assert_eq!(route_untextured(Some("noIcon")), Untextured::Dangling);
    assert_eq!(
        route_untextured(Some("tat_propCZ1_parts_head")),
        Untextured::Dangling
    );

    // A far model's missing reduced copy is looked up as the full sheet.
    assert_eq!(
        lowrez_source("com_item_healthrecharge_lowrez"),
        Some("com_item_healthrecharge")
    );
    assert_eq!(lowrez_source("com_item_healthrecharge"), None);

    // Anything else that fails to resolve is OUR bug: grey and counted.
    assert_eq!(
        route_untextured(Some("tat1_bldg_wall")),
        Untextured::LookupFailed
    );
    // Near-misses on the dangling list must NOT be swallowed by it.
    assert_eq!(
        route_untextured(Some("yav_prop_ivy2")),
        Untextured::LookupFailed
    );
    assert_eq!(route_untextured(Some("icon")), Untextured::LookupFailed);
}

/// The V address-mode rule over the shapes `artifacts/sky_wrap/dome_uv.txt`
/// actually holds — the four overshooting shells that made the horizon
/// stripe, the two base domes that genuinely tile in V, and the ordinary
/// [0, 1] shell.
#[test]
fn the_v_wrap_rule_over_the_shipped_dome_segments() {
    use render::WrapV::{ClampToEdge, Repeat};
    // Overshoot past v = 1: clamp, or the last rows wrap to the zenith.
    assert_eq!(sky_wrap_v(0.0020, 1.0010), ClampToEdge); // hoth_sky_dome
    assert_eq!(sky_wrap_v(0.0020, 1.0767), ClampToEdge); // mus1_sky_dome prim1
    assert_eq!(sky_wrap_v(0.0308, 1.1304), ClampToEdge); // BF1 NAB1 sky_water
                                                         // Genuinely tiling base domes: leave them repeating.
    assert_eq!(sky_wrap_v(-1.7080, 2.7080), Repeat); // BF2 POL1 pol1_skydome
    assert_eq!(sky_wrap_v(-0.5029, 1.5015), Repeat); // BF2 TAN1 tan1_sky_dome
                                                     // The ordinary case — and the one the bilinear footprint still
                                                     // wraps without the clamp.
    assert_eq!(sky_wrap_v(0.0, 1.0), ClampToEdge);
    // The tolerance is symmetric and exclusive at its own edges.
    assert_eq!(sky_wrap_v(-0.25, 1.25), ClampToEdge);
    assert_eq!(sky_wrap_v(-0.2501, 1.0), Repeat);
    assert_eq!(sky_wrap_v(0.0, 1.2501), Repeat);
}

/// The ocean foam's UV scale is the base repeat over the foam's (both KAM1s
/// 40 / 32 = 1.25), 1 on a bake without the foam field.
#[test]
fn foam_uv_scale_is_base_over_foam_repeat() {
    let c = manifest::ManifestWaterConfig {
        metres_per_repeat: Some([40.0, 40.0]),
        foam_metres_per_repeat: Some([32.0, 32.0]),
        ..Default::default()
    };
    assert_eq!(super::foam_uv_scale(Some(&c)), [1.25, 1.25]);
    let old = manifest::ManifestWaterConfig {
        foam_metres_per_repeat: None,
        ..c.clone()
    };
    assert_eq!(super::foam_uv_scale(Some(&old)), [1.0, 1.0]);
    assert_eq!(super::foam_uv_scale(None), [1.0, 1.0]);
}

#[test]
fn material_scroll_bindings_do_not_change_the_static_texture() {
    let key = "lava".to_owned();
    assert_eq!(scrolling_key(key.clone(), [0.0; 2]), key);
    assert_eq!(scrolling_key(key.clone(), [-0.0, 0.0]), key);
    let a = scrolling_key(key.clone(), [-1.0, 0.0]);
    assert_ne!(a, key);
    assert_eq!(a, scrolling_key(key.clone(), [-1.0, 0.0]));
    assert_ne!(a, scrolling_key(key.clone(), [0.0, -1.0]));
    assert_ne!(a, scrolling_key(key.clone(), [-0.5, 0.0]));
    assert_ne!(
        a,
        scrolling_key(format!("{CLAMP_V_KEY_PREFIX}{key}"), [-1.0, 0.0])
    );
}

/// Real Mustafar material through the world uploader; no shared bake writes.
#[test]
#[ignore = "requires GPU, real Mustafar bake and OPENBF_SCROLL_MODELS overlay"]
fn mustafar_world_material_moves_and_static_control_does_not() {
    let root = crate::assets::baked_root();
    let mut scene = crate::assets::load_scene(&root.join("bf2_mus1")).unwrap();
    let overlay = std::env::var_os("OPENBF_SCROLL_MODELS").expect("selected-model bake directory");
    let name = "mus1_bldg_world_2";
    let mesh =
        crate::assets::load_glb(&std::path::PathBuf::from(overlay).join(format!("{name}.glb")))
            .unwrap();
    let k = (0..mesh.submeshes.len())
        .find(|&k| mesh.uv_scroll(k) != [0.0; 2])
        .unwrap();
    assert_eq!(mesh.uv_scroll(k), [-5.0 / 255.0, 0.0]);
    let inst = scene
        .manifest
        .instances
        .iter()
        .find(|i| scene.resolve_model(&i.class) == Some(name))
        .unwrap()
        .clone();
    let matrix = super::super::camera::instance_matrix(inst.rotation, inst.translation);
    let (start, count, ..) = mesh.submeshes[k];
    let vertices = &mesh.indices[start as usize..(start + count) as usize];
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut normal = Vec3::ZERO;
    for &i in vertices {
        let p = matrix.transform_point3(Vec3::from(mesh.positions[i as usize]));
        lo = lo.min(p);
        hi = hi.max(p);
        normal += matrix.transform_vector3(Vec3::from(mesh.normals[i as usize]));
    }
    let centre = (lo + hi) * 0.5;
    let normal = normal.try_normalize().unwrap_or(Vec3::Y);
    let eye = centre + normal * (hi - lo).length().max(10.0);
    let dir = (centre - eye).normalize();
    let cam = render::Camera {
        position: eye,
        yaw: (-dir.x).atan2(-dir.z),
        pitch: dir.y.asin(),
        ..Default::default()
    };
    scene.meshes.insert(name.into(), mesh);
    scene.manifest.instances = vec![inst];
    scene.manifest.sky = None;
    scene.terrain = None;
    scene.water = None;
    scene.lowres = None;
    scene.terrain_layers = None;
    let mut r = Renderer::new_headless(640, 480).unwrap();
    let gpu = upload_scene(&mut r, &scene).unwrap();
    let frame = |r: &mut Renderer, gpu: &SceneGpu, t: f64| {
        r.water_time = t;
        r.render_frame(&cam, &gpu.draws, &[], gpu.clear, None)
            .unwrap();
        r.capture_rgba().unwrap().0
    };
    let a = frame(&mut r, &gpu, 0.0);
    let b = frame(&mut r, &gpu, 2.0);
    let repeated = frame(&mut r, &gpu, 255.0);
    assert_eq!(a, repeated, "whole material period must repeat");
    let changed = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 100, "only {changed} pixels moved");
    if let Some(out) = std::env::var_os("OPENBF_SCROLL_PICTURES") {
        let out = std::path::PathBuf::from(out);
        for (label, rgba) in [("world-scroll-t0", &a), ("world-scroll-t2", &b)] {
            std::fs::write(
                out.join(format!("{label}.png")),
                render::encode_png_rgb(rgba, 640, 480).unwrap(),
            )
            .unwrap();
        }
    }
    scene.meshes.get_mut(name).unwrap().scroll.fill([0.0; 2]);
    let mut r = Renderer::new_headless(640, 480).unwrap();
    let gpu = upload_scene(&mut r, &scene).unwrap();
    assert_eq!(
        frame(&mut r, &gpu, 0.0),
        frame(&mut r, &gpu, 2.0),
        "static material moved"
    );
    eprintln!("Mustafar lava: {changed} pixels changed; static control unchanged");
}

#[test]
#[ignore = "requires a GPU adapter and the owned BF2 Kashyyyk bake"]
fn render_options_switch_live_without_reuploading_the_scene() {
    let root = crate::assets::baked_root();
    let scene = crate::assets::load_scene(&root.join("bf2_kas2")).unwrap();
    let mut r = Renderer::new_headless(640, 480).unwrap();
    let mut gpu = upload_scene(&mut r, &scene).unwrap();
    assert!(gpu.terrain_lod.is_some());
    assert!(gpu.softness.is_some());
    let count = gpu.draws.len();
    let camera = render::Camera {
        position: Vec3::new(-29.95, 3.23, -46.41),
        yaw: -0.287f32.to_radians(),
        pitch: 3.727f32.to_radians(),
        ..Default::default()
    };
    let frame = |r: &mut Renderer, gpu: &SceneGpu| {
        r.render_frame(&camera, &gpu.draws, &[], gpu.clear, None)
            .unwrap();
        r.capture_rgba().unwrap().0
    };
    let mut frames = Vec::new();
    for (terrain, softness) in [(false, false), (true, false), (true, true), (false, false)] {
        gpu.apply_render_options(&mut r, terrain, softness);
        assert_eq!(gpu.draws.len(), count);
        frames.push(frame(&mut r, &gpu));
    }
    assert_ne!(frames[0], frames[1]);
    assert_ne!(frames[1], frames[2]);
    assert_eq!(frames[0], frames[3]);
    if let Some(out) = std::env::var_os("OPENBF_RENDER_OPTIONS_PICTURES") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for (label, rgba) in ["both-off", "terrain-on", "both-on", "restored"]
            .into_iter()
            .zip(&frames)
        {
            std::fs::write(
                out.join(format!("{label}.png")),
                render::encode_png(rgba, 640, 480).unwrap(),
            )
            .unwrap();
        }
        let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
        for (w, h) in [(640, 480)] {
            let mut model = ui::menu::MenuModel {
                page: Some(ui::menu::PausePage::Settings(
                    ui::menu::SettingsPage::Graphics,
                )),
                fov_scale: 1.0,
                fov_deg: 52.0,
                assist_intensity: 1.0,
                team_size_pct: 100,
                reinforcements_pct: 100,
                music_volume: 1.0,
                mouse_sensitivity: 1.0,
                pad_look_sensitivity: 1.0,
                ..Default::default()
            };
            model.cursor = ui::menu::rows(&model)
                .iter()
                .position(|r| *r == ui::menu::Row::SceneSoftness)
                .unwrap();
            let paint = overlay.begin_headless([w, h], 1.0, |ctx| {
                ui::menu::draw_menu(ctx, &model);
            });
            r.render_frame(
                &camera,
                &gpu.draws,
                &[],
                gpu.clear,
                Some(&mut |dev, q, enc, target, size| {
                    overlay.paint(&paint, dev, q, enc, target, size)
                }),
            )
            .unwrap();
            let rgba = r.capture_rgba().unwrap().0;
            std::fs::write(
                out.join(format!("options-{w}x{h}.png")),
                render::encode_png(&rgba, w, h).unwrap(),
            )
            .unwrap();
        }
    }
}
