//! Round trip: bake::skin_glb writer -> game::assets::load_skinned_glb ->
//! game::skin palette. A two-bone bar with one clip that swings the root.

use bake::skin_glb::{skinned_to_glb, Bone, Clip, ClipTrack, SkinnedModel, SkinnedSegment};
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn skinned_glb_round_trip() {
    let model = SkinnedModel {
        name: "test_bar".into(),
        bones: vec![
            Bone {
                name: "bone_root".into(),
                parent: None,
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            Bone {
                name: "bone_tip".into(),
                parent: Some(0),
                translation: [0.0, 1.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        segments: vec![SkinnedSegment {
            positions: vec![
                [-0.3, 0.0, 0.0],
                [0.3, 0.0, 0.0],
                [-0.3, 1.0, 0.0],
                [0.3, 1.0, 0.0],
                [0.0, 1.5, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 5],
            uvs: vec![[0.0, 0.0]; 5],
            joints: vec![
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [0, 1, 0, 0],
                [0, 1, 0, 0],
                [1, 0, 0, 0],
            ],
            weights: vec![
                [1.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.5, 0.5, 0.0, 0.0],
                [0.5, 0.5, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
            ],
            triangles: vec![[0, 1, 3], [0, 3, 2], [2, 3, 4]],
            texture: "imp_inf_stormtrooper".into(),
            blend: false,
            spec: None,
            cloth: false,
        }],
    };
    let swing = Quat::from_rotation_z(FRAC_PI_2).to_array();
    let clips = vec![Clip {
        name: "human_rifle_test".into(),
        fps: 30.0,
        frames: 3,
        tracks: vec![ClipTrack {
            bone: 0,
            rotations: vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0], swing],
            translations: vec![[0.0; 3]; 3],
        }],
    }];
    let glb = skinned_to_glb(&model, &clips).expect("write glb");

    let dir = std::env::temp_dir().join(format!("openbf_skinned_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_bar.glb");
    std::fs::write(&path, &glb).unwrap();
    let asset = game::assets::load_skinned_glb(&path).expect("load glb");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(asset.name, "test_bar");
    let skel = &asset.skeleton;
    assert_eq!(skel.names, vec!["bone_root", "bone_tip"]);
    assert_eq!(skel.parents, vec![None, Some(0)]);
    assert!(close(
        skel.global_bind[1].transform_point3(Vec3::ZERO),
        Vec3::Y
    ));

    let m = &asset.mesh;
    assert_eq!(m.positions.len(), 5);
    assert_eq!(m.joints[2], [0, 1, 0, 0]);
    assert_eq!(m.weights[2], [0.5, 0.5, 0.0, 0.0]);
    assert_eq!(m.indices, vec![0, 1, 3, 0, 3, 2, 2, 3, 4]);
    assert_eq!(
        m.submeshes,
        vec![(
            0,
            9,
            "imp_inf_stormtrooper".to_string(),
            render::DrawLayer::Opaque
        )]
    );

    assert_eq!(asset.clips.len(), 1);
    let clip = &asset.clips[0];
    assert_eq!(
        (clip.name.as_str(), clip.frames, clip.fps),
        ("human_rifle_test", 3, 30.0)
    );
    assert!(clip.tracks[0].is_some() && clip.tracks[1].is_none());
    // Frame 2 swings the root 90° about z: the tip-bound vertex (0,1.5,0)
    // lands at (-1.5,0,0); at frame 1 it is still at rest.
    let pose = clip.sample(skel, 2.0, false);
    let pal = skel.palette(&pose.locals());
    assert!(close(
        pal[1].transform_point3(Vec3::new(0.0, 1.5, 0.0)),
        Vec3::new(-1.5, 0.0, 0.0)
    ));
    let rest = skel.palette(&clip.sample(skel, 1.0, false).locals());
    assert!(close(
        rest[1].transform_point3(Vec3::new(0.0, 1.5, 0.0)),
        Vec3::new(0.0, 1.5, 0.0)
    ));

    // End to end on the GPU: upload through the viewer helper and render the
    // swung pose 5 m in front of the default camera. The tip vertex (0,1.5,0)
    // ends at (-1.5,0,0) so the pixel there is lit; its rest position is bare.
    let mut r = render::Renderer::new_headless(96, 96).expect("headless renderer");
    let uploaded = game::viewer::upload_skinned(&mut r, &asset.mesh).expect("upload");
    assert_eq!(uploaded.len(), 1);
    assert_eq!(uploaded[0].1, Some("imp_inf_stormtrooper"));
    let cam = render::Camera::default();
    let model = glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0));
    let draw = render::SkinnedDrawCmd {
        mesh: uploaded[0].0,
        texture: None,
        color: [0.0, 1.0, 0.0, 1.0],
        model,
        palette: pal.clone().into(),
        layer: uploaded[0].2,
        light: render::LightPath::Character,
        lit: None,
    };
    r.render_frame(&cam, &[], &[draw], [1.0, 0.0, 0.0, 1.0], None)
        .expect("frame");
    let (rgba, w, _) = r.capture_rgba().expect("capture");
    let px = |p: Vec3| {
        let clip = cam.view_proj(1.0) * model.transform_point3(p).extend(1.0);
        let ndc = clip.truncate() / clip.w;
        let x = ((ndc.x * 0.5 + 0.5) * 96.0).round().clamp(0.0, 95.0) as u32;
        let y = ((1.0 - (ndc.y * 0.5 + 0.5)) * 96.0)
            .round()
            .clamp(0.0, 95.0) as u32;
        let i = ((y * w + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    };
    // Just inside the swung tip triangle (its apex is at (-1.5,0,0)).
    let swung = px(Vec3::new(-1.2, 0.05, 0.0));
    assert!(swung[1] > 100 && swung[0] < 60, "swung tip lit: {swung:?}");
    let rest_apex = px(Vec3::new(0.0, 1.4, 0.0));
    assert_eq!(rest_apex, [255, 0, 0, 255], "rest apex vacated");
}

/// A one-triangle segment on bone 0 that names `texture`.
fn tri(texture: &str, blend: bool) -> SkinnedSegment {
    SkinnedSegment {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        joints: vec![[0, 0, 0, 0]; 3],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
        triangles: vec![[0, 1, 2]],
        texture: texture.into(),
        blend,
        spec: None,
        cloth: false,
    }
}

/// Queue SKA: the pass is per SEGMENT, not per texture. `all_inf_wookie`'s
/// three segments name one sheet and the disc composites only its fur
/// (`bake::soldier::segment_blends`), so a model shaped like it must load as
/// Opaque / Blend / Opaque — and only the blended material may state an
/// `alphaMode` at all. The other two keep glTF's OPAQUE default, which is
/// what leaves every other soldier GLB byte-identical to its earlier bake.
#[test]
fn only_the_blended_segment_states_an_alpha_mode_and_it_loads_as_the_blend_pass() {
    let model = SkinnedModel {
        name: "test_fur".into(),
        bones: vec![Bone {
            name: "bone_root".into(),
            parent: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }],
        segments: vec![tri("fur", false), tri("fur", true), tri("fur", false)],
    };
    let glb = skinned_to_glb(&model, &[]).expect("write glb");

    // The GLB's JSON chunk: a 12-byte header, then (length, "JSON", bytes).
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    assert_eq!(&glb[16..20], b"JSON");
    let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + len]).expect("glTF JSON");
    let modes: Vec<Option<&str>> = doc["materials"]
        .as_array()
        .expect("materials")
        .iter()
        .map(|m| m.get("alphaMode").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(modes, [None, Some("BLEND"), None]);

    let dir = std::env::temp_dir().join(format!("openbf_skinned_blend_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_fur.glb");
    std::fs::write(&path, &glb).unwrap();
    let asset = game::assets::load_skinned_glb(&path).expect("load glb");
    let _ = std::fs::remove_dir_all(&dir);
    let layers: Vec<render::DrawLayer> = asset.mesh.submeshes.iter().map(|s| s.3).collect();
    assert_eq!(
        layers,
        [
            render::DrawLayer::Opaque,
            render::DrawLayer::Blend,
            render::DrawLayer::Opaque
        ]
    );
}

/// A soldier GLB whose material asks for MASK fails to LOAD: the loader is
/// where the caller can skip one model (`game::soldier_vis`), while
/// `render_frame` would refuse the whole frame and end the windowed game. No
/// bake writes one — `bake::soldier::segment_blends` refuses the cutout bit —
/// so the fixture relabels the one BLEND material in place.
#[test]
fn a_skinned_material_asking_for_mask_fails_to_load() {
    let model = SkinnedModel {
        name: "test_mask".into(),
        bones: vec![Bone {
            name: "bone_root".into(),
            parent: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }],
        segments: vec![tri("fur", true)],
    };
    let mut glb = skinned_to_glb(&model, &[]).expect("write glb");
    // `"MASK" ` is as long as `"BLEND"` (JSON allows the space), so the
    // chunk lengths in the GLB header stay true.
    let at = glb
        .windows(7)
        .position(|w| w == b"\"BLEND\"")
        .expect("the blended material");
    glb[at..at + 7].copy_from_slice(b"\"MASK\" ");

    let dir = std::env::temp_dir().join(format!("openbf_skinned_mask_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_mask.glb");
    std::fs::write(&path, &glb).unwrap();
    let loaded = game::assets::load_skinned_glb(&path);
    let _ = std::fs::remove_dir_all(&dir);
    let err = loaded.err().expect("a MASK skinned material must not load");
    assert!(format!("{err:#}").contains("MASK"), "{err:#}");
}

/// Queue SPEC2: a side model's segment carries its specular second pass as
/// the rigid bake does — `extras.spec_pass` on its material, `rgb: null` for
/// BF1's "the map's sun" — and the loader reads it back per submesh. A
/// segment with no pass writes no extras at all, so every soldier without a
/// specular segment keeps its GLB byte-identical.
#[test]
fn a_skinned_segment_carries_its_specular_pass_through_the_glb() {
    use bake::spec::SpecPass;
    let own = SpecPass {
        power: 4,
        rgb: Some([1.0, 0.5, 0.25]),
    };
    let sun = SpecPass {
        power: 50,
        rgb: None,
    };
    let mut segments = vec![tri("a", false), tri("b", false), tri("c", false)];
    segments[0].spec = Some(own);
    segments[2].spec = Some(sun);
    let model = SkinnedModel {
        name: "test_spec".into(),
        bones: vec![Bone {
            name: "bone_root".into(),
            parent: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }],
        segments,
    };
    let glb = skinned_to_glb(&model, &[]).expect("write glb");
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + len]).expect("glTF JSON");
    let extras: Vec<Option<&serde_json::Value>> = doc["materials"]
        .as_array()
        .expect("materials")
        .iter()
        .map(|m| m.get("extras"))
        .collect();
    assert!(extras[1].is_none(), "no pass, no extras: {:?}", extras[1]);
    assert_eq!(
        extras[2].unwrap()["spec_pass"]["rgb"],
        serde_json::Value::Null
    );

    let dir = std::env::temp_dir().join(format!("openbf_skinned_spec_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_spec.glb");
    std::fs::write(&path, &glb).unwrap();
    let asset = game::assets::load_skinned_glb(&path).expect("load glb");
    let _ = std::fs::remove_dir_all(&dir);
    let got: Vec<_> = (0..3).map(|k| asset.mesh.spec_pass(k)).collect();
    let want = |p: SpecPass| game::assets::SpecPass {
        power: p.power,
        rgb: p.rgb,
    };
    assert_eq!(got, [Some(want(own)), None, Some(want(sun))]);
    // BF1's sun-coloured pass resolves against the map's sun: its BYTES raw
    // over 128, from the manifest's linear colour (card B1DK). Kamino's sun
    // (65,85,105) is (0.508,0.664,0.820).
    let lin = |b: f32| ((b / 255.0 + 0.055) / 1.055_f32).powf(2.4);
    let s = got[2]
        .unwrap()
        .rgb_under([lin(65.0), lin(85.0), lin(105.0)]);
    for (got, byte) in s.iter().zip([65.0_f32, 85.0, 105.0]) {
        assert!((got - byte / 128.0).abs() < 1e-4, "{s:?}");
    }
    assert_eq!(got[2].unwrap().rgb_under([1.0, 1.0, 0.0])[2], 0.0);
}
