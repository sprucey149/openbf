//! Headless GPU referee for the WATER pass (`render::DrawLayer::Water`).
//!
//! The water is the one surface the PS2 draws translucent AND occluding: its
//! near main layer is ALPHA 0x44 over TEST 0x50000, "depth-tested and
//! depth-WRITTEN" (`analysis/re-water.md` §1 [V]). Neither of the other two
//! passes spells that — `Opaque` ignores alpha, `Blend` drops the depth
//! write — so the first three referees here pin the three things that make the pass
//! its own: it composites, it still occludes what is drawn after it, and it
//! is fogged by its own distance rather than the world's.
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh`.

use glam::Mat4;
use render::{Camera, DrawCmd, Fog, Lighting};

use crate::common;
use common::{capture_pixel, headless, tex_desc, upload_tex, view_quad, N};

/// A renderer whose lighting is a flat white ambient: every referee below
/// wants the texel and the tint back, not a lambert term.
fn unlit() -> render::Renderer {
    let mut r = headless();
    r.lighting = Lighting {
        sun_color: [0.0; 3],
        ambient_sky: [1.0; 3],
        ambient_ground: [1.0; 3],
        ..Lighting::default()
    };
    r
}

/// The centre pixel of a frame drawing `draws`.
fn shot(r: &mut render::Renderer, draws: &[DrawCmd]) -> [u8; 4] {
    r.render(&Camera::default(), draws, [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    capture_pixel(r, N / 2, N / 2)
}

#[test]
#[ignore = "requires a GPU adapter"]
fn diffuse_vertex_colours_do_not_tint_the_separate_speckle_layer() {
    let mut r = unlit();
    r.lighting.water.speckle_ambient = [0.5, 0.5, 0.5, 1.0];
    let white = upload_tex(&mut r, 1, 1, &[255; 4], "water");
    let colored = r
        .add_mesh(&render::MeshData {
            positions: &[
                [-4.0, -4.0, -3.0],
                [4.0, -4.0, -3.0],
                [4.0, 4.0, -3.0],
                [-4.0, 4.0, -3.0],
            ],
            normals: &[[0.0, 0.0, 1.0]; 4],
            uvs: &[[0.0; 2]; 4],
            colors: Some(&[[40, 80, 120, 255]; 4]),
            indices: &[0, 1, 2, 0, 2, 3],
        })
        .unwrap();
    let plain = view_quad(&mut r, -3.0);
    let water = shot(
        &mut r,
        &[DrawCmd::water(colored, white, [1.0; 4], Mat4::IDENTITY)],
    );
    for (actual, expected) in water[..3].iter().zip([40i32, 80, 120]) {
        assert!((i32::from(*actual) - expected).abs() <= 2, "{water:?}");
    }
    let tinted = shot(
        &mut r,
        &[DrawCmd::speckle(colored, white, [1.0; 4], Mat4::IDENTITY)],
    );
    let untinted = shot(
        &mut r,
        &[DrawCmd::speckle(plain, white, [1.0; 4], Mat4::IDENTITY)],
    );
    assert_eq!(tinted, untinted);
    assert!(tinted[0] > 100, "{tinted:?}");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn water_modulates_and_composites_display_colours_with_texture_alpha() {
    let mut r = unlit();
    let bed = upload_tex(&mut r, 1, 1, &[120, 100, 80, 255], "bed");
    let surface = upload_tex(&mut r, 1, 1, &[80, 160, 120, 128], "surface");
    let floor = view_quad(&mut r, -5.0);
    let water = view_quad(&mut r, -3.0);
    let px = shot(
        &mut r,
        &[
            DrawCmd::water(floor, bed, [1.0; 4], Mat4::IDENTITY),
            DrawCmd::water(water, surface, [0.5; 4], Mat4::IDENTITY),
        ],
    );
    for (actual, expected) in px[..3].iter().zip([100i32, 95, 75]) {
        assert!((i32::from(*actual) - expected).abs() <= 2, "{px:?}");
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn far_water_colour_grid_changes_with_position_and_only_beyond_the_crossover() {
    let mut r = unlit();
    let white = upload_tex(&mut r, 1, 1, &[255; 4], "water");
    let mesh = view_quad(&mut r, -3.0);
    let draws = [DrawCmd::water(mesh, white, [1.0; 4], Mat4::IDENTITY)];
    let dark = u32::from_le_bytes([30, 60, 90, 255]);
    let light = u32::from_le_bytes([110, 160, 210, 255]);
    r.lighting.water.far_diffuse = render::WaterDiffuse {
        origin: [-1.0, -3.0],
        step: [2.0; 2],
        dims: [2, 2],
        range: [1.0, 2.0],
        colors: [[dark, light, dark, light]; 16],
    };
    for scale in [1.0, 0.5] {
        r.set_world_scale(scale);
        shot(&mut r, &draws);
        let a = capture_pixel(&mut r, N / 4, N / 2);
        let b = capture_pixel(&mut r, 3 * N / 4, N / 2);
        assert!(
            b[0] > a[0] + 30 && b[2] > a[2] + 50,
            "{scale}: {a:?}, {b:?}"
        );
    }
    r.lighting.water.far_diffuse.range = [4.0, 5.0];
    assert_eq!(shot(&mut r, &draws), [255; 4]);
    r.lighting.water.far_diffuse.range = [2.0, 4.0];
    let px = shot(&mut r, &draws);
    for (actual, expected) in px[..3].iter().zip([163i32, 183, 203]) {
        assert!((i32::from(*actual) - expected).abs() <= 5, "{px:?}");
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn water_only_fog_covers_the_composite_even_with_a_transparent_surface() {
    let mut r = unlit();
    r.lighting.water_gain = 255.0 / 128.0;
    r.lighting.water_fog_begin = Some(1.0);
    r.lighting.water_fog = Some(Fog {
        color: [0.0, 0.0, render::srgb_decode(104.0 / 255.0)],
        near: 0.0,
        far: 1.0,
    });
    let green = upload_tex(&mut r, 1, 1, &[0, 255, 0, 255], "bed");
    let clear = upload_tex(&mut r, 1, 1, &[255, 255, 255, 0], "water");
    let bed = view_quad(&mut r, -5.0);
    let mesh = view_quad(&mut r, -3.0);
    let floor = DrawCmd::textured(bed, green, Mat4::IDENTITY);
    assert_eq!(shot(&mut r, &[floor]), [0, 255, 0, 255]);
    let px = shot(
        &mut r,
        &[floor, DrawCmd::water(mesh, clear, [1.0; 4], Mat4::IDENTITY)],
    );
    for (actual, expected) in px[..3].iter().zip([0i32, 7, 101]) {
        assert!((i32::from(*actual) - expected).abs() <= 1, "{px:?}");
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn water_hdr_gain_amplifies_surface_and_speckle_without_changing_opacity_or_the_bed() {
    let mut r = unlit();
    r.lighting.water_gain = 255.0 / 128.0;
    r.lighting.water.speckle_ambient = [0.5; 4];
    let bed = upload_tex(&mut r, 1, 1, &[40, 60, 80, 255], "bed");
    let surface = upload_tex(&mut r, 1, 1, &[80, 120, 160, 128], "surface");
    let highlight = upload_tex(&mut r, 1, 1, &[40, 80, 120, 0], "highlight");
    let floor = view_quad(&mut r, -5.0);
    let water = view_quad(&mut r, -3.0);
    let ground = DrawCmd::textured(floor, bed, Mat4::IDENTITY);
    assert_eq!(shot(&mut r, &[ground]), [40, 60, 80, 255]);
    let px = shot(
        &mut r,
        &[
            ground,
            DrawCmd::water(water, surface, [0.5; 4], Mat4::IDENTITY),
            DrawCmd::speckle(water, highlight, [1.0; 4], Mat4::IDENTITY),
        ],
    );
    for (actual, expected) in px[..3].iter().zip([70i32, 115, 160]) {
        assert!((i32::from(*actual) - expected).abs() <= 2, "{px:?}");
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn far_speckle_uses_the_current_frames_texture_alpha_and_keeps_the_near_crossfade() {
    let mut r = unlit();
    r.lighting.water.speckle_ambient = [0.5; 4];
    let bed = upload_tex(&mut r, 1, 1, &[40, 60, 80, 255], "bed");
    let a = upload_tex(&mut r, 1, 1, &[80, 120, 160, 64], "current");
    let b = upload_tex(&mut r, 1, 1, &[160, 80, 120, 255], "next");
    let clear = upload_tex(&mut r, 1, 1, &[80, 120, 160, 0], "unmasked rgb");
    let floor = view_quad(&mut r, -5.0);
    let water = view_quad(&mut r, -3.0);
    let mut draws = [
        DrawCmd::textured(floor, bed, Mat4::IDENTITY),
        DrawCmd::speckle(water, a, [1.0, 1.0, 1.0, 0.25], Mat4::IDENTITY),
        DrawCmd::speckle(water, b, [0.0, 1.0, 1.0, 0.75], Mat4::IDENTITY),
    ];
    for (range, expected) in [
        ([4.0, 5.0], [75i32, 82, 112]),
        ([2.0, 4.0], [60, 75, 101]),
        ([1.0, 2.0], [45, 68, 90]),
    ] {
        r.lighting.water.far_range = range;
        let px = shot(&mut r, &draws);
        for (actual, expected) in px[..3].iter().zip(expected) {
            assert!(
                (i32::from(*actual) - expected).abs() <= 2,
                "{range:?}: {px:?}"
            );
        }
    }
    draws[1].texture = Some(clear);
    assert_eq!(shot(&mut r, &draws), [40, 60, 80, 255]);
    r.lighting.water.far_range = [0.0; 2];
    let px = shot(&mut r, &draws);
    assert!(px[0] > 70 && px[2] > 105, "near-only: {px:?}");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn far_speckle_rounds_weak_highlights_with_the_ps2_dither_pattern() {
    let mut r = unlit();
    r.lighting.water_gain = 255.0 / 128.0;
    r.lighting.water.speckle_ambient = [0.5; 4];
    let mut desc = tex_desc(1, 1, &[&[20, 20, 20, 255]], "display bytes");
    desc.srgb = false;
    let tex = r.add_texture(&desc).unwrap();
    let mesh = view_quad(&mut r, -3.0);
    let draws = [DrawCmd::speckle(mesh, tex, [1.0; 4], Mat4::IDENTITY)];
    r.lighting.water.far_range = [1.0, 2.0];
    assert_eq!(shot(&mut r, &draws), [0, 0, 0, 255]);
    assert_eq!(capture_pixel(&mut r, N / 2 + 3, N / 2), [16, 16, 16, 255]);
    r.lighting.water.far_range = [4.0, 5.0];
    assert_eq!(shot(&mut r, &draws), [10, 10, 10, 255]);
}

#[test]
#[ignore = "requires a GPU adapter"]
fn speckle_adds_display_colours_and_crossfades_without_reading_texture_alpha() {
    let mut r = unlit();
    r.lighting.water.speckle_ambient = [0.5, 0.5, 0.5, 0.5];
    let bed = upload_tex(&mut r, 1, 1, &[40, 60, 80, 255], "bed");
    let a = upload_tex(&mut r, 1, 1, &[80, 120, 160, 0], "frame a");
    let b = upload_tex(&mut r, 1, 1, &[160, 80, 120, 255], "frame b");
    let floor = view_quad(&mut r, -5.0);
    let water = view_quad(&mut r, -3.0);
    let px = shot(
        &mut r,
        &[
            DrawCmd::water(floor, bed, [1.0; 4], Mat4::IDENTITY),
            DrawCmd::speckle(water, a, [1.0, 1.0, 1.0, 0.25], Mat4::IDENTITY),
            DrawCmd::speckle(water, b, [1.0, 1.0, 1.0, 0.75], Mat4::IDENTITY),
        ],
    );
    for (actual, expected) in px[..3].iter().zip([75i32, 82, 112]) {
        assert!((i32::from(*actual) - expected).abs() <= 2, "{px:?}");
    }
}

/// A water quad at half tint alpha over an opaque red one composites — and
/// the same draw through the opaque pass does not.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_water_composites_over_the_world_it_covers() {
    let mut r = unlit();
    let red = upload_tex(&mut r, 1, 1, &[255, 0, 0, 255], "floor");
    let white = upload_tex(&mut r, 1, 1, &[255, 255, 255, 255], "water");
    let floor = view_quad(&mut r, -5.0);
    let water = view_quad(&mut r, -3.0);

    let px = shot(
        &mut r,
        &[
            DrawCmd::textured(floor, red, Mat4::IDENTITY),
            DrawCmd::water(water, white, [1.0, 1.0, 1.0, 0.5], Mat4::IDENTITY),
        ],
    );
    assert_eq!(px[0], 255, "the red floor is still under it: {px:?}");
    assert!((i32::from(px[1]) - 128).abs() <= 1, "{px:?}");
    assert_eq!(px[1], px[2], "a white water layer is neutral: {px:?}");

    // The same two draws with the water opaque: it PAINTS, and the floor's
    // red is gone. This is what the pass must not do.
    let px = shot(
        &mut r,
        &[
            DrawCmd::textured(floor, red, Mat4::IDENTITY),
            DrawCmd::textured(water, white, Mat4::IDENTITY),
        ],
    );
    assert_eq!(px, [255, 255, 255, 255], "opaque paints: {px:?}");
}

/// The water WRITES depth: a blended draw behind it is hidden, where the same
/// draw in front of it composites. This is why the water is not a `Blend`
/// draw — `Blend` has no depth write and the two would show through each
/// other.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_water_still_occludes_what_is_drawn_after_it() {
    let mut r = unlit();
    let green = upload_tex(&mut r, 1, 1, &[0, 255, 0, 255], "prop");
    let blue = upload_tex(&mut r, 1, 1, &[0, 0, 255, 255], "water");
    let water = view_quad(&mut r, -3.0);
    let behind = view_quad(&mut r, -4.0);
    let front = view_quad(&mut r, -2.0);

    // Opaque water alpha 1: the pixel is the water alone, so the green quad
    // BEHIND it left no trace — the depth test saw the water's write.
    let px = shot(
        &mut r,
        &[
            DrawCmd::water(water, blue, [1.0; 4], Mat4::IDENTITY),
            DrawCmd::textured_blended(behind, green, Mat4::IDENTITY),
        ],
    );
    assert_eq!(
        px[1], 0,
        "a blended draw behind the water is hidden: {px:?}"
    );
    assert_eq!(px[2], 255, "and the water itself is there: {px:?}");

    // The same draw IN FRONT of the water is not hidden.
    let px = shot(
        &mut r,
        &[
            DrawCmd::water(water, blue, [1.0; 4], Mat4::IDENTITY),
            DrawCmd::textured_blended(front, green, Mat4::IDENTITY),
        ],
    );
    assert_eq!(px[1], 255, "in front of the water it draws: {px:?}");
}

/// The water's fog gate is its own: inside `water_fog_begin` it takes none,
/// past it the map's ramp — while an opaque draw at the same distance is
/// fogged either way (`analysis/re-water.md` §1 [V]).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_water_takes_fog_only_past_its_own_distance() {
    let fog = Fog {
        color: [1.0, 0.0, 0.0],
        near: 0.0,
        far: 10.0,
    };
    // The quad sits 5 m out, so the world's ramp is half up at it.
    let z = -5.0;

    // Water fog crossover BEYOND the surface: unfogged.
    let mut r = unlit();
    r.lighting = Lighting {
        fog: Some(fog),
        water_fog_begin: Some(8.0),
        ..r.lighting
    };
    let white = upload_tex(&mut r, 1, 1, &[255, 255, 255, 255], "water");
    let mesh = view_quad(&mut r, z);
    let near = shot(
        &mut r,
        &[DrawCmd::water(mesh, white, [1.0; 4], Mat4::IDENTITY)],
    );
    assert_eq!(near, [255, 255, 255, 255], "near water is never fogged");

    // The same frame with the crossover INSIDE it: the ramp applies, and the
    // pixel is exactly what the opaque pass makes of the same quad.
    let mut r = unlit();
    r.lighting = Lighting {
        fog: Some(fog),
        water_fog_begin: Some(1.0),
        ..r.lighting
    };
    let white = upload_tex(&mut r, 1, 1, &[255, 255, 255, 255], "water");
    let mesh = view_quad(&mut r, z);
    let far = shot(
        &mut r,
        &[DrawCmd::water(mesh, white, [1.0; 4], Mat4::IDENTITY)],
    );
    let opaque = shot(&mut r, &[DrawCmd::textured(mesh, white, Mat4::IDENTITY)]);
    assert!(far[1] < 250, "past the crossover the water fogs: {far:?}");
    assert_eq!(far, opaque, "and it fogs by the SAME ramp: {far:?}");

    // `None` means never, whatever the map's fog says.
    let mut r = unlit();
    r.lighting = Lighting {
        fog: Some(fog),
        water_fog_begin: None,
        ..r.lighting
    };
    let white = upload_tex(&mut r, 1, 1, &[255, 255, 255, 255], "water");
    let mesh = view_quad(&mut r, z);
    let never = shot(
        &mut r,
        &[DrawCmd::water(mesh, white, [1.0; 4], Mat4::IDENTITY)],
    );
    assert_eq!(never, [255, 255, 255, 255], "BF2 fogs no water at all");
}

/// A LIT water draw (`Lighting::water_lit`, BF2's ocean — card KSEAL) is the
/// unlit one scaled, in GS terms, by `sun byte / 128 · n·l · weight`, then by
/// its loss with depth; weight 0 leaves the draw exactly as it was.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_lit_water_draw_takes_the_suns_diffuse_term() {
    // The quad faces the eye 5 m out; the sun shines straight onto it with
    // the byte 64 in every channel (0.05127 linear), half of GS unity.
    let lit = |weight: f32, dim: Option<[f32; 2]>| {
        let mut r = unlit();
        r.lighting = Lighting {
            sun_dir: glam::Vec3::NEG_Z,
            sun_color: [0.05127; 3],
            water_lit: weight,
            water_dim: dim,
            ..r.lighting
        };
        let white = upload_tex(&mut r, 1, 1, &[255, 255, 255, 255], "water");
        let mesh = view_quad(&mut r, -5.0);
        shot(
            &mut r,
            &[DrawCmd::water(mesh, white, [1.0; 4], Mat4::IDENTITY)],
        )
    };
    assert_eq!(lit(0.0, None), [255, 255, 255, 255], "weight 0 is unlit");
    assert_eq!(lit(0.0, Some([2.0, 0.1])), [255, 255, 255, 255]);
    let near = |px: [u8; 4], want: i32| (0..3).all(|c| (i32::from(px[c]) - want).abs() <= 2);
    let full = lit(1.0, None);
    assert!(near(full, 128), "64/128 of white: {full:?}");
    let part = lit(0.5, None);
    assert!(near(part, 64), "and half of that at weight 0.5: {part:?}");
    // 3 m past the dim's begin at 0.1 per metre: 0.7 of the lit colour.
    let far = lit(1.0, Some([2.0, 0.1]));
    assert!(near(far, 89), "0.7 of 128: {far:?}");
}
