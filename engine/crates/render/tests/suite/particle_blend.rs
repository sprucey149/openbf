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
