//! GPU referees for particle display-byte blending and cutouts.
use glam::{Mat4, Vec3};
use render::{
    Camera, DrawCmd, DrawLayer, LightPath, ParticleDraw, ParticleShape, Renderer, SkinnedDrawCmd,
};

use crate::common::{capture_pixel, headless, skinned_quad, upload_tex, view_quad, N};

fn linear(display: u8) -> f64 {
    let c = f64::from(display) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn shot(texel: [u8; 4], color: [u8; 4], additive: bool, clear: u8) -> [u8; 4] {
    shot_shape(
        ParticleShape::Sprite {
            pos: Vec3::new(0.0, 0.0, -3.0),
            half: 20.0,
            angle: 0.0,
            uv: [0.0, 0.0, 1.0, 1.0],
        },
        texel,
        color,
        additive,
        clear,
    )
}

fn shot_shape(
    shape: ParticleShape,
    texel: [u8; 4],
    color: [u8; 4],
    additive: bool,
    clear: u8,
) -> [u8; 4] {
    let mut r = headless();
    let tex = upload_tex(&mut r, 1, 1, &texel, "particle");
    r.set_particles(vec![ParticleDraw {
        shape,
        key: Vec3::new(0.0, 0.0, -3.0),
        color,
        texture: Some(tex),
        additive,
    }]);
    let c = linear(clear);
    r.render(&Camera::default(), &[], [c, c, c, 1.0])
        .expect("render");
    capture_pixel(&mut r, N / 2, N / 2)
}

fn close(px: [u8; 4], want: u8, what: &str) {
    for ch in &px[..3] {
        assert!(ch.abs_diff(want) <= 2, "{what}: {px:?}, want {want}");
    }
}

fn grass_fixture(texture: render::TextureHandle) -> (render::GrassClass, render::GrassPatch) {
    use render::{GrassClass, GrassPatch, GrassSprite};
    let sprite = GrassSprite {
        pos: Vec3::new(0.0, -2.0, -3.0),
        width: 4.0,
        height: 4.0,
        up_scale: 1.0,
        lean: 0.0,
        phase: 0.0,
        index: 0,
        tall: None,
        uv: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
        color: [1.0; 4],
    };
    let class = GrassClass {
        texture: Some(texture),
        sprites: vec![sprite],
        tall_lean: 0.0,
        still: true,
        bound: 5.0,
    };
    let patch = GrassPatch {
        class: 0,
        matrix: Mat4::IDENTITY,
        color: [1.0; 4],
        distance: 100.0,
    };
    (class, patch)
}

#[test]
#[ignore = "requires a GPU adapter"]
fn grass_modulates_and_blends_display_bytes_over_dark_ground() {
    let mut r = headless();
    r.lighting = render::Lighting {
        sun_color: [0.0; 3],
        ambient_sky: [1.0; 3],
        ambient_ground: [1.0; 3],
        ..Default::default()
    };
    let texture = upload_tex(&mut r, 1, 1, &[255, 255, 255, 128], "grass display blend");
    let (class, mut patch) = grass_fixture(texture);
    patch.color = [0.8, 0.6, 0.5, 1.0];
    let clear = linear(60);
    for count in [1, 2] {
        r.set_grass(vec![class.clone()], vec![patch; count]);
        r.render(&Camera::default(), &[], [clear, clear, clear, 1.0])
            .unwrap();
        let px = capture_pixel(&mut r, N / 2, N / 2);
        for (channel, tint) in [0.8_f32, 0.6, 0.5].into_iter().enumerate() {
            let mut expected = 60.0;
            for _ in 0..count {
                expected = tint * 255.0 * (128.0 / 255.0) + expected * (127.0 / 255.0);
            }
            assert!(
                px[channel].abs_diff(expected.round() as u8) <= 2,
                "{px:?}, expected {expected}"
            );
        }
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn grass_uses_the_patch_up_normal_and_global_display_light_sum() {
    let mut r = headless();
    let texture = upload_tex(&mut r, 1, 1, &[255; 4], "grass lighting");
    let (class, mut patch) = grass_fixture(texture);
    r.lighting = render::Lighting {
        sun_dir: Vec3::NEG_Y,
        sun_color: [render::srgb_decode(80.0 / 255.0), 0.0, 0.0],
        sun_static: true,
        light2: Some(render::DirLight {
            dir: Vec3::NEG_Y,
            color: [0.0, render::srgb_decode(100.0 / 255.0), 0.0],
            is_static: true,
        }),
        ambient_sky: [0.0, 0.0, render::srgb_decode(150.0 / 255.0)],
        ambient_ground: [0.0, 0.0, render::srgb_decode(50.0 / 255.0)],
        ..Default::default()
    };
    for scale in [1.0, 0.5] {
        patch.matrix = Mat4::from_scale(Vec3::new(1.0, scale, 1.0));
        r.set_grass(vec![class.clone()], vec![patch]);
        r.render(&Camera::default(), &[], [0.0; 4]).unwrap();
        let px = capture_pixel(&mut r, N / 2, N / 2);
        let ny = scale * 127.0 / 128.0;
        for (channel, expected) in [80.0 * ny, 100.0 * ny, 100.0 + 50.0 * ny]
            .into_iter()
            .enumerate()
        {
            assert!(
                px[channel].abs_diff(expected.round() as u8) <= 2,
                "{scale}: {px:?}"
            );
        }
    }
    r.lighting.sun_color = [0.0; 3];
    r.lighting.light2 = None;
    r.lighting.ambient_sky = [0.0; 3];
    r.lighting.ambient_ground = [0.0; 3];
    r.render(&Camera::default(), &[], [0.0; 4]).unwrap();
    close(capture_pixel(&mut r, N / 2, N / 2), 0, "unlit map");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn grass_filters_texture_display_bytes_before_decoding() {
    let mut r = headless();
    r.lighting = render::Lighting {
        sun_color: [0.0; 3],
        ambient_sky: [1.0; 3],
        ambient_ground: [1.0; 3],
        ..Default::default()
    };
    let data = [0, 0, 0, 255, 128, 128, 128, 255];
    let mips = [&data[..]];
    let desc = crate::common::tex_desc(2, 1, &mips, "grass display filtering");
    let texture = r
        .add_texture(&render::TextureDesc {
            sampler: render::SamplerMode::Linear,
            ..desc
        })
        .unwrap();
    let (mut class, patch) = grass_fixture(texture);
    class.sprites[0].uv = [[0.5; 2]; 4];
    r.set_grass(vec![class], vec![patch]);
    r.render(&Camera::default(), &[], [0.0; 4]).unwrap();
    close(capture_pixel(&mut r, N / 2, N / 2), 64, "display midpoint");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn terrain_layers_blend_one_and_two_layers_in_display_bytes() {
    let mut r = headless();
    let texture = upload_tex(&mut r, 1, 1, &[0, 0, 0, 128], "terrain layer blend");
    let mesh = view_quad(&mut r, -3.0);
    let layer = DrawCmd::terrain_layer(mesh, texture, Mat4::IDENTITY);
    let c = linear(200);
    for (state, count) in [
        ("base", 1),
        ("base", 2),
        ("discard:on", 1),
        ("discard:on", 2),
    ] {
        r.frame_ab = render::FrameAb::parse(state).unwrap();
        let draws = vec![layer; count];
        r.render(&Camera::default(), &draws, [c, c, c, 1.0])
            .unwrap();
        let mut expected = 200.0_f32;
        for _ in 0..count {
            expected *= 127.0 / 255.0;
        }
        close(
            capture_pixel(&mut r, N / 2, N / 2),
            expected.round() as u8,
            "terrain source-over",
        );
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn blur_softens_scene_edges_in_place_through_alpha_and_respects_depth() {
    let (width, height) = (512, 448);
    let mut r = Renderer::new_headless(width, height).expect("headless renderer");
    let stripes = upload_tex(
        &mut r,
        2,
        1,
        &[64, 64, 64, 255, 220, 220, 220, 255],
        "background stripes",
    );
    let mask = upload_tex(&mut r, 1, 1, &[255, 0, 255, 128], "half-alpha mask");
    let clear_mask = upload_tex(&mut r, 1, 1, &[255, 0, 255, 0], "clear mask");
    let mesh = view_quad(&mut r, -4.0);
    let scene = [DrawCmd::textured(mesh, stripes, Mat4::IDENTITY)];
    let character = [SkinnedDrawCmd {
        mesh: skinned_quad(&mut r, -4.0),
        texture: Some(stripes),
        color: [1.0; 4],
        model: Mat4::IDENTITY,
        palette: vec![Mat4::IDENTITY].into(),
        layer: DrawLayer::Opaque,
        lit: None,
        light: LightPath::Character,
    }];
    let wall = [DrawCmd::textured(
        mesh,
        stripes,
        Mat4::from_translation(Vec3::Z),
    )];
    let camera = Camera::default();
    let clear = [0.0, 0.0, 0.0, 1.0];
    r.render(&camera, &scene, clear).expect("render");
    let x = width / 2 - 1;
    let y = height / 2;
    let baseline = capture_pixel(&mut r, x, y);
    let outside = capture_pixel(&mut r, x + 12, y);
    let mut sample = |z, texture, x, draws: &[DrawCmd], skinned: &[SkinnedDrawCmd]| {
        let pos = Vec3::new(0.0, 0.0, z);
        r.set_particles(vec![ParticleDraw {
            shape: ParticleShape::Blur {
                pos,
                half: 4.0,
                angle: 0.0,
            },
            key: pos,
            color: [0, 255, 0, 255],
            texture: Some(texture),
            additive: false,
        }]);
        r.render_frame(&camera, draws, skinned, clear, None)
            .expect("render");
        capture_pixel(&mut r, x, y)
    };
    let softened = sample(-3.0, mask, x, &scene, &[]);
    for ch in 0..3 {
        let low = u16::from(baseline[ch]);
        let high = u16::from(outside[ch]);
        let expected = low + (high - low) * 3 / 8;
        assert!(
            u16::from(softened[ch]).abs_diff(expected) <= 5,
            "{softened:?}"
        );
    }
    assert_eq!(
        sample(-3.0, clear_mask, x, &scene, &[]),
        baseline,
        "transparent mask"
    );
    assert!(
        sample(-3.0, mask, x + 12, &scene, &[])[0].abs_diff(outside[0]) <= 4,
        "the edge must not move into the surrounding pixels"
    );
    assert_eq!(
        sample(-5.0, mask, x, &scene, &[]),
        baseline,
        "opaque geometry occludes blur"
    );
    let character_base = sample(-5.0, clear_mask, x, &[], &character);
    let character_heat = sample(-5.0, mask, x, &[], &character);
    assert!(
        character_heat[0] > character_base[0] + 40,
        "heat softens the visible soldier"
    );
    assert_eq!(
        sample(-5.0, mask, x, &wall, &character),
        sample(-5.0, clear_mask, x, &wall, &character),
        "a wall hides the soldier and blocks the heat"
    );
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_normal_particle_blends_in_display_bytes() {
    close(
        shot([0, 0, 0, 255], [255, 255, 255, 0], false, 200),
        200,
        "alpha 0 byte",
    );
    close(
        shot([0, 0, 0, 255], [255, 255, 255, 255], false, 200),
        0,
        "opaque black",
    );
    close(
        shot([0, 0, 0, 255], [255, 255, 255, 128], false, 200),
        100,
        "half alpha: 200 · (1 − 0.5)",
    );
    close(
        shot([0, 0, 0, 128], [255, 255, 255, 255], false, 200),
        100,
        "half texel alpha",
    );
    close(
        shot([100, 100, 100, 255], [255, 255, 255, 255], false, 200),
        100,
        "opaque texel at full colour",
    );
    close(
        shot([200, 200, 200, 255], [128, 128, 128, 255], false, 0),
        100,
        "colour 128 halves the texel",
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn dust_keeps_its_unscaled_hdr_color_and_authored_alpha() {
    let quad = |gain| ParticleShape::AtmosphereQuad {
        pos: Vec3::new(0.0, 0.0, -3.0),
        a: Vec3::X * 20.0,
        b: Vec3::Y * 4.0,
        gain,
    };
    close(
        shot_shape(quad(1.0), [150, 150, 150, 128], [255; 4], false, 100),
        125,
        "ordinary source-over",
    );
    close(
        shot_shape(quad(2.0), [150, 150, 150, 128], [255; 4], false, 100),
        200,
        "unscaled dust in the half-scale HDR frame",
    );
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn an_additive_particle_adds_in_display_bytes() {
    close(
        shot([255, 255, 255, 255], [255, 255, 255, 128], true, 60),
        188,
        "60 + 255 · 0.5",
    );
    close(
        shot([100, 100, 100, 255], [255, 255, 255, 255], true, 200),
        255,
        "clamped",
    );
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_falling_leaf_drops_texels_at_or_below_a_quarter_alpha() {
    let leaf = || ParticleShape::CutoutQuad {
        pos: Vec3::new(0.0, 0.0, -3.0),
        a: Vec3::X * 20.0,
        b: Vec3::Y * 20.0,
    };
    let quad = ParticleShape::Quad {
        pos: Vec3::new(0.0, 0.0, -3.0),
        a: Vec3::X * 20.0,
        b: Vec3::Y * 20.0,
    };
    close(
        shot_shape(leaf(), [0, 0, 0, 51], [255; 4], false, 200),
        200,
        "leaf alpha 0.2 discarded",
    );
    close(
        shot_shape(quad, [0, 0, 0, 51], [255; 4], false, 200),
        160,
        "an ordinary quad blends the same texel",
    );
    close(
        shot_shape(leaf(), [0, 0, 0, 128], [255; 4], false, 200),
        100,
        "leaf alpha 0.5 blends",
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn forest_mesh_uses_the_leaf_cutout_without_changing_normal_particles() {
    let mesh = || ParticleShape::CutoutMesh {
        tris: [0, 1, 2, 2, 1, 3]
            .map(|i| {
                (
                    [
                        Vec3::new(-20.0, 20.0, -3.0),
                        Vec3::new(20.0, 20.0, -3.0),
                        Vec3::new(-20.0, -20.0, -3.0),
                        Vec3::new(20.0, -20.0, -3.0),
                    ][i],
                    [0.5, 0.5],
                )
            })
            .to_vec(),
    };
    close(
        shot_shape(mesh(), [0, 0, 0, 51], [255; 4], false, 200),
        200,
        "bird edge discarded",
    );
    close(
        shot_shape(mesh(), [0, 0, 0, 128], [255; 4], false, 200),
        100,
        "bird edge blends",
    );
}
