//! What a baked GLB's material tells the runtime: the ALPHA RULE v2 verdict
//! (glTF `alphaMode`, which the WORLD path routes on) and the RAW PS2 MTRL
//! word beside it (`extras.flags`, which only the SKY path reads).
//!
//! The two are independent by construction — the verdict is the port's census
//! heuristic and the flags are the disc's own bits — and Mustafar is the
//! worked example of them disagreeing: `mus1_sky_volcano`'s segment is MTRL
//! 0x00010003 (bit 0x02 → alpha-TESTED on the PS2,
//! `analysis/re-sky-dome-placement.md` §5 [V]) while ALPHA RULE v2 judges the
//! same segment OPAQUE. A round trip through `bake::glb::model_to_glb` and
//! `game::assets::load_glb` has to carry BOTH.

use bake::alpha::{AlphaMode, AlphaStats, Rule, SegmentAlpha};
use mungefs::model::{Aabb, Model, RenderType, Segment};

/// A one-triangle segment naming `texture`, with the raw MTRL word `flags`.
fn seg(texture: &str, flags: u32) -> Segment {
    Segment {
        vertex_count: 3,
        index_count: 3,
        bf2_material: true,
        pc_geometry: false,
        material_flags: flags,
        render_type: RenderType::Bf2(0),
        spec_power: 0,
        spec_color: 0,
        uv_anim: [0, 0],
        textures: vec![(0, texture.into())],
        bone: None,
        group: None,
        bmap: Vec::new(),
        bind: Vec::new(),
        wght: Vec::new(),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        colors: None,
        triangles: vec![[0, 1, 2]],
    }
}

/// MUS1's two sky models in one file: the volcano's 0x00010003 (alpha-tested
/// on the disc, OPAQUE by ALPHA RULE v2) and one of the dome's 0x00010010
/// (opaque both ways), plus a BLEND verdict on a 0x04 word so the two channels
/// are seen disagreeing and agreeing in the same round trip.
#[test]
fn load_glb_carries_both_the_alpha_mode_and_the_raw_mtrl_flags() {
    let m = Model {
        name: "mus1_sky".into(),
        bone_space: false,
        vertex_box: Aabb {
            min: [0.0; 3],
            max: [1.0; 3],
        },
        vis_box: Aabb {
            min: [0.0; 3],
            max: [1.0; 3],
        },
        face_count: 3,
        sphere: None,
        segments: vec![
            seg("mus1_sky_volcano", 0x0001_0003),
            seg("mus1_sky_dome", 0x0001_0010),
            seg("mus1_sky_cloud", 0x0001_0005),
        ],
    };
    let stats = AlphaStats {
        mid: 0.55,
        a0: 0.12,
        amax: 255,
    };
    let alphas = vec![
        SegmentAlpha {
            texture: "mus1_sky_volcano".into(),
            // The disagreement: the census says OPAQUE, the disc's 0x02 says
            // alpha-tested. Both have to survive the file.
            mode: AlphaMode::Opaque,
            rule: Rule::Mask,
            flags: 0x0001_0003,
            rtyp: RenderType::Bf2(0),
            stats: Some(stats),
        },
        SegmentAlpha {
            texture: "mus1_sky_dome".into(),
            mode: AlphaMode::Opaque,
            rule: Rule::Mask,
            flags: 0x0001_0010,
            rtyp: RenderType::Bf2(0),
            stats: Some(stats),
        },
        SegmentAlpha {
            texture: "mus1_sky_cloud".into(),
            mode: AlphaMode::Blend,
            rule: Rule::Modulation,
            flags: 0x0001_0005,
            rtyp: RenderType::Bf2(3),
            stats: Some(stats),
        },
    ];

    // `vertex_light` on, as a map bake writes it, so the lighting path makes
    // the same round trip (queue LITEB).
    let glb = bake::glb::model_to_glb(&m, &alphas, true).expect("write glb");
    let dir = std::env::temp_dir().join(format!("openbf_glb_material_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mus1_sky.glb");
    std::fs::write(&path, &glb).unwrap();
    let cpu = game::assets::load_glb(&path).expect("load glb");
    let _ = std::fs::remove_dir_all(&dir);

    let got: Vec<(&str, render::DrawLayer, u32)> = cpu
        .submeshes
        .iter()
        .map(|(_, _, mat, layer, flags)| (mat.as_str(), *layer, *flags))
        .collect();
    assert_eq!(
        got,
        vec![
            ("mus1_sky_volcano", render::DrawLayer::Opaque, 0x0001_0003),
            ("mus1_sky_dome", render::DrawLayer::Opaque, 0x0001_0010),
            ("mus1_sky_cloud", render::DrawLayer::Blend, 0x0001_0005),
        ]
    );

    // The lighting path rides the same material: MTRL bit 0x01 lit, clear
    // unlit (`bake::glb::light_path`).
    assert_eq!(
        (0..3).map(|i| cpu.light_path(i)).collect::<Vec<_>>(),
        vec![
            render::LightPath::Lit,
            render::LightPath::Unlit,
            render::LightPath::Lit
        ]
    );

    // And what the sky pass makes of those words — the ELF's rule, which is
    // NOT the alpha mode beside it (RE §5).
    let modes: Vec<render::SkyMode> = got
        .iter()
        .map(|&(_, _, f)| render::SkyMode::from_flags(f))
        .collect();
    assert_eq!(
        modes,
        vec![
            // 0x02 — the volcano. OPAQUE to the world path, and BLEND to the
            // sky path, which under the sky draw flag composites every
            // alpha-carrying material (RE §5, LEAD CORRECTION 2, session 12).
            render::SkyMode::Blend,
            render::SkyMode::Opaque, // neither bit — the dome
            render::SkyMode::Blend,  // 0x04
        ]
    );
}

/// A GLB with no `extras` at all — anything baked before ALPHA RULE v2 — reads
/// as flags 0, which `SkyMode::from_flags` calls OPAQUE: the same verdict such
/// a bake meant, and the same one `layer_of` already gives its `alphaMode`.
#[test]
fn a_glb_without_extras_reads_as_flags_zero() {
    let positions = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let glb = bake::glb::triangles_to_glb("floor", &positions, &[[0u32, 1, 2]]).expect("write glb");
    let dir = std::env::temp_dir().join(format!("openbf_glb_noextras_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("floor.glb");
    std::fs::write(&path, &glb).unwrap();
    let cpu = game::assets::load_glb(&path).expect("load glb");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(cpu.submeshes.len(), 1);
    assert_eq!(cpu.submeshes[0].3, render::DrawLayer::Opaque);
    assert_eq!(cpu.submeshes[0].4, 0);
    assert_eq!(render::SkyMode::from_flags(0), render::SkyMode::Opaque);
}

#[test]
fn felucia_shell_and_glowing_interior_keep_separate_draw_paths() {
    let mut shell = seg("fel1_bldg_bladder1", 0x85);
    shell.colors = Some(vec![[127; 4]; 3]);
    let mut inner = shell.clone();
    inner.material_flags = 0x1_0098;
    let model = Model {
        name: "felucia_shell_regression".into(),
        bone_space: false,
        vertex_box: Aabb {
            min: [0.0; 3],
            max: [1.0; 3],
        },
        vis_box: Aabb {
            min: [0.0; 3],
            max: [1.0; 3],
        },
        face_count: 2,
        sphere: None,
        segments: vec![inner, shell],
    };
    let stats = AlphaStats {
        mid: 0.24868774,
        a0: 0.017425537,
        amax: 255,
    };
    let alphas: Vec<_> = model
        .segments
        .iter()
        .map(|segment| {
            let (mode, rule) =
                bake::alpha::verdict(segment.material_flags, &segment.render_type, Some(&stats));
            SegmentAlpha {
                texture: "fel1_bldg_bladder1".into(),
                mode,
                rule,
                flags: segment.material_flags,
                rtyp: segment.render_type.clone(),
                stats: Some(stats),
            }
        })
        .collect();
    let bytes = bake::glb::world_model_to_glb(&model, &alphas, true).unwrap();
    let dir = std::env::temp_dir().join(format!("openbf_felucia_shell_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plant.glb");
    std::fs::write(&path, bytes).unwrap();
    let mesh = game::assets::load_glb(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(mesh.submeshes[0].3, render::DrawLayer::Opaque);
    assert_eq!(mesh.light_path(0), render::LightPath::Glow);
    assert_eq!(mesh.submeshes[1].3, render::DrawLayer::Blend);
    assert_eq!(mesh.light_path(1), render::LightPath::Lit);
}
