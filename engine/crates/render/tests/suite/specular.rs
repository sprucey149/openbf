//! Headless GPU referee for the SPECULAR second pass
//! (`render::DrawLayer::Spec`, queue SPEC).
//!
//! The PS2 draws it right after its base, `Cs·Ad + Cd` with `Cs = min(1,
//! T·rgb)` — the table texel times a flat colour — and `Ad` the base pass's
//! own alpha, in the GS's gamma units (`analysis/re-specular-pass.md` §2, §6,
//! `[O]`). These referees pin that add on a known texel, colour and mask over
//! a known base; that it keeps its base's depth and loses to a nearer draw;
//! that no table means no pass; and that the shader's tex-gen is
//! `render::spec_texcoord`, the disc's mirrored light and the consistent one.
//! The skinned twin (queue SPEC2) adds the same way and takes its tex-gen
//! from the SKINNED vertex and normal.
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh` (serialised).

use glam::{Mat4, Vec3};
use render::{
    spec_texcoord, Camera, DrawCmd, DrawLayer, Lighting, SkinnedDrawCmd, SkinnedMeshHandle,
};

use crate::common;
use common::{capture_pixel, headless, skinned_quad, upload_tex, view_quad, N};

/// A renderer that shades a texel back as itself (flat white ambient, no
/// sun term), with the sun TRAVELLING along `sun_dir` for the tex-gen.
fn unlit(sun_dir: Vec3) -> render::Renderer {
    let mut r = headless();
    r.lighting = Lighting {
        sun_dir,
        sun_color: [0.0; 3],
        ambient_sky: [1.0; 3],
        ambient_ground: [1.0; 3],
        ..Lighting::default()
    };
    r
}

/// A 2×2 table of one grey value: the pass samples `grey` wherever it looks.
fn flat_table(r: &mut render::Renderer, grey: u8) {
    let t = [grey, grey, grey, 255].repeat(4);
    r.set_specular_table(Some((2, 2, &t))).expect("table");
}

/// The centre pixel of a frame drawing `draws`.
fn shot(r: &mut render::Renderer, draws: &[DrawCmd]) -> [u8; 4] {
    r.render(&Camera::default(), draws, [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    capture_pixel(r, N / 2, N / 2)
}

fn close(px: [u8; 4], want: [u8; 3], what: &str) {
    for c in 0..3 {
        assert!(
            (i32::from(px[c]) - i32::from(want[c])).abs() <= 1,
            "{what}: got {px:?}, want {want:?}"
        );
    }
}

/// Base grey 100 under a table texel of 128 and a colour (1, 0.5, 0): the GS
/// adds 128, 64 and 0 in its own units, so the pixel reads 228, 164, 100 —
/// the add happens in gamma, not in the linear target. A base alpha of half
/// halves it (`Ad`), and alpha 0 leaves the base alone.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_pass_adds_texel_times_colour_times_base_alpha_in_gamma() {
    let mut r = unlit(Vec3::NEG_Y);
    flat_table(&mut r, 128);
    let quad = view_quad(&mut r, -3.0);
    for (alpha, want) in [
        (255u8, [228u8, 164, 100]),
        // Disc alpha 128 → Ad ≈ 0.502: +64 and +32.
        (128, [164, 132, 100]),
        (0, [100, 100, 100]),
    ] {
        let base = upload_tex(&mut r, 1, 1, &[100, 100, 100, alpha], "base");
        let px = shot(
            &mut r,
            &[
                DrawCmd::textured(quad, base, Mat4::IDENTITY),
                DrawCmd::specular(quad, base, [1.0, 0.5, 0.0], 20, Mat4::IDENTITY),
            ],
        );
        close(px, want, &format!("base alpha {alpha}"));
    }

    // The colour saturates per channel before the mask: T·rgb capped at 1,
    // then the sum capped at 255 (the frame's own clamp).
    let base = upload_tex(&mut r, 1, 1, &[200, 100, 0, 255], "base");
    let px = shot(
        &mut r,
        &[
            DrawCmd::textured(quad, base, Mat4::IDENTITY),
            DrawCmd::specular(quad, base, [2.0, 2.0, 2.0], 20, Mat4::IDENTITY),
        ],
    );
    close(px, [255, 255, 255], "saturated");
    let px = shot(
        &mut r,
        &[
            DrawCmd::textured(quad, base, Mat4::IDENTITY),
            DrawCmd::specular(quad, base, [0.5, 0.5, 0.5], 20, Mat4::IDENTITY),
        ],
    );
    close(px, [255, 164, 64], "half colour: +64 each, red capped");
}

/// Card FALPH: a pass sheet's alpha is the pass's gloss WEIGHT (the SBD
/// droid sheet's mean is 34/255), never coverage — so the frame it lands in
/// stays opaque. The base, its add and a blended quad over them, on a clear
/// that ASKS for alpha 0, leave every pixel at alpha 255; the colour is the
/// same weighted add as above (`Ad` still the sheet's own alpha).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_pass_sheet_draw_leaves_the_frame_opaque() {
    let mut r = unlit(Vec3::NEG_Y);
    flat_table(&mut r, 128);
    let quad = view_quad(&mut r, -3.0);
    let near = view_quad(&mut r, -2.0);
    let sheet = upload_tex(&mut r, 1, 1, &[100, 100, 100, 34], "pass sheet");
    let glass = upload_tex(&mut r, 1, 1, &[0, 0, 200, 64], "glass");
    let draws = [
        DrawCmd::textured(quad, sheet, Mat4::IDENTITY),
        DrawCmd::specular(quad, sheet, [1.0, 1.0, 1.0], 20, Mat4::IDENTITY),
    ];
    r.render(&Camera::default(), &draws, [0.0, 0.0, 0.0, 0.0])
        .expect("frame");
    let (rgba, _, _) = r.capture_rgba().expect("capture");
    assert!(
        rgba.as_chunks::<4>().0.iter().all(|px| px[3] == 255),
        "base + add: every pixel must be opaque"
    );
    // +128 · 34/255 ≈ +17 in gamma units.
    close(
        capture_pixel(&mut r, N / 2, N / 2),
        [117, 117, 117],
        "weighted add",
    );

    let mut with_glass = draws.to_vec();
    with_glass.push(DrawCmd::textured_blended(near, glass, Mat4::IDENTITY));
    r.render(&Camera::default(), &with_glass, [0.0, 0.0, 0.0, 0.0])
        .expect("frame");
    let (rgba, _, _) = r.capture_rgba().expect("capture");
    assert!(
        rgba.as_chunks::<4>().0.iter().all(|px| px[3] == 255),
        "a blended draw over it must not write its alpha either"
    );
}

/// The pass keeps its base's depth (`LessEqual`, no write): it lands on its
/// own base, loses to a nearer opaque draw submitted in the same frame, and a
/// renderer with no table draws no pass at all.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_pass_keeps_its_base_depth_and_needs_a_table() {
    let mut r = unlit(Vec3::NEG_Y);
    flat_table(&mut r, 128);
    let base = upload_tex(&mut r, 1, 1, &[100, 100, 100, 255], "base");
    let front_tex = upload_tex(&mut r, 1, 1, &[0, 0, 50, 255], "front");
    let quad = view_quad(&mut r, -3.0);
    let front = view_quad(&mut r, -2.0);
    let spec = DrawCmd::specular(quad, base, [1.0, 1.0, 1.0], 20, Mat4::IDENTITY);

    let px = shot(
        &mut r,
        &[
            DrawCmd::textured(quad, base, Mat4::IDENTITY),
            DrawCmd::textured(front, front_tex, Mat4::IDENTITY),
            spec,
        ],
    );
    close(px, [0, 0, 50], "a nearer draw covers the highlight");

    r.set_specular_table(None).expect("no table");
    let px = shot(
        &mut r,
        &[DrawCmd::textured(quad, base, Mat4::IDENTITY), spec],
    );
    close(px, [100, 100, 100], "no table, no pass");
}

/// The shader's tex-gen is `spec_texcoord`, PER VERTEX as the VU1 computes
/// it (`analysis/re-specular-pass.md` §3) and interpolated across the
/// triangle: a table that is a ramp in t reads t back. The centre pixel sits
/// on the quad's shared diagonal, so its t is the mean of corners 0 and 2's;
/// the disc's mirrored light and the consistent one land on different rows
/// (§4).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_texgen_is_spec_texcoord_mirrored_and_consistent() {
    let sun = Vec3::new(1.0, 0.0, -1.0).normalize();
    let mut r = unlit(sun);
    // 1 wide, 64 tall: row j holds 4j, so bilinear T(t) = 256·t − 2 inside
    // the ramp.
    let ramp: Vec<u8> = (0..64u8).flat_map(|j| [4 * j, 4 * j, 4 * j, 255]).collect();
    r.set_specular_table(Some((1, 64, &ramp))).expect("table");
    let black = upload_tex(&mut r, 1, 1, &[0, 0, 0, 255], "base");
    let z = -3.0;
    let quad = view_quad(&mut r, z);

    for mirror in [true, false] {
        r.spec_mirror = mirror;
        let corner = |x: f32, y: f32| {
            spec_texcoord(
                20.0,
                Vec3::Z,
                Vec3::ZERO,
                Vec3::new(x, y, z),
                Camera::default().right(),
                sun,
                mirror,
            )[1]
        };
        let t = 0.5 * (corner(-4.0, -4.0) + corner(4.0, 4.0));
        let want = (256.0 * t - 2.0).round() as u8;
        let px = shot(
            &mut r,
            &[
                DrawCmd::textured(quad, black, Mat4::IDENTITY),
                DrawCmd::specular(quad, black, [1.0; 3], 20, Mat4::IDENTITY),
            ],
        );
        // The centre pixel sits half a pixel off the quad's centre, so allow
        // the ramp's slope across it.
        assert!(
            (i32::from(px[0]) - i32::from(want)).abs() <= 3,
            "mirror {mirror}: t {t}, want {want}, got {px:?}"
        );
    }
}

/// A skinned opaque base at `palette`, with the model matrix left identity.
fn skinned_base(
    mesh: SkinnedMeshHandle,
    texture: render::TextureHandle,
    palette: Mat4,
) -> SkinnedDrawCmd {
    SkinnedDrawCmd {
        mesh,
        texture: Some(texture),
        color: [1.0; 4],
        model: Mat4::IDENTITY,
        palette: vec![palette].into(),
        layer: DrawLayer::Opaque,
        light: render::LightPath::Character,
        lit: None,
    }
}

/// The full frame drawing `draws` and `skinned`.
fn frame(r: &mut render::Renderer, draws: &[DrawCmd], skinned: &[SkinnedDrawCmd]) -> Vec<u8> {
    r.render_frame(
        &Camera::default(),
        draws,
        skinned,
        [0.0, 0.0, 0.0, 1.0],
        None,
    )
    .expect("frame");
    r.capture_rgba().expect("capture").0
}

/// Queue SPEC2: a side model's pass is the world's, skinned. Over a skinned
/// base it adds exactly what the rigid pass adds (grey 100 + texel 128 ×
/// (1, 0.5, 0) → 228, 164, 100, in gamma), it keeps its base's depth, and
/// without a table it is skipped like the world's.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_skinned_pass_adds_as_the_rigid_one_and_needs_a_table() {
    let mut r = unlit(Vec3::NEG_Y);
    flat_table(&mut r, 128);
    let base_tex = upload_tex(&mut r, 1, 1, &[100, 100, 100, 255], "base");
    let front_tex = upload_tex(&mut r, 1, 1, &[0, 0, 50, 255], "front");
    let quad = skinned_quad(&mut r, -3.0);
    let front = view_quad(&mut r, -2.0);
    let base = skinned_base(quad, base_tex, Mat4::IDENTITY);
    let spec = base.specular([1.0, 0.5, 0.0], 20);
    assert_eq!(spec.layer, DrawLayer::Spec);
    let centre = |rgba: &[u8]| common::pixel(rgba, N, N / 2, N / 2);

    let px = centre(&frame(&mut r, &[], &[base.clone(), spec.clone()]));
    close(px, [228, 164, 100], "skinned pass over a skinned base");

    let px = centre(&frame(
        &mut r,
        &[DrawCmd::textured(front, front_tex, Mat4::IDENTITY)],
        &[base.clone(), spec.clone()],
    ));
    close(px, [0, 0, 50], "a nearer draw covers the skinned highlight");

    r.set_specular_table(None).expect("no table");
    let px = centre(&frame(&mut r, &[], &[base, spec]));
    close(px, [100, 100, 100], "no table, no skinned pass");
}

/// Queue DSHN: a droid's base sheet keeps its disc alpha, a dark gloss mask
/// (`analysis/re-spec-gloss.md` §1/§3), and the skinned pass is weighted by
/// it like the rigid one (`Ad`): alpha 64 (≈ 0.25) adds a quarter — texel 128
/// × (1, 0.5, 0) → +32, +16, 0 over grey 100.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_skinned_pass_is_weighted_by_its_base_sheets_gloss_alpha() {
    let mut r = unlit(Vec3::NEG_Y);
    flat_table(&mut r, 128);
    let quad = skinned_quad(&mut r, -3.0);
    let centre = |rgba: &[u8]| common::pixel(rgba, N, N / 2, N / 2);
    for (alpha, want) in [(64u8, [132u8, 116, 100]), (0, [100, 100, 100])] {
        let tex = upload_tex(&mut r, 1, 1, &[100, 100, 100, alpha], "gloss");
        let base = skinned_base(quad, tex, Mat4::IDENTITY);
        let spec = base.specular([1.0, 0.5, 0.0], 20);
        let px = centre(&frame(&mut r, &[], &[base, spec]));
        close(px, want, &format!("skinned base alpha {alpha}"));
    }
}

/// The skinned pass's tex-gen reads the SKINNED position and normal: a quad
/// posed by its palette draws the same frame, pixel for pixel (±1), as the
/// rigid quad placed by the same matrix as its model — in both light modes,
/// under a table that is a ramp in t, so every pixel is its own t. The pose
/// tilts the quad's normal, so a pass that skipped the palette would read a
/// different t (the test checks that the pose moves the picture at all).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_skinned_pass_takes_the_skinned_position_and_normal() {
    let sun = Vec3::new(1.0, 0.0, -1.0).normalize();
    let mut r = unlit(sun);
    let ramp: Vec<u8> = (0..64u8).flat_map(|j| [4 * j, 4 * j, 4 * j, 255]).collect();
    r.set_specular_table(Some((1, 64, &ramp))).expect("table");
    let black = upload_tex(&mut r, 1, 1, &[0, 0, 0, 255], "base");
    let rigid = view_quad(&mut r, 0.0);
    let skinned = skinned_quad(&mut r, 0.0);
    let pose = Mat4::from_translation(Vec3::new(0.3, 0.2, -3.0))
        * Mat4::from_rotation_x(0.35)
        * Mat4::from_rotation_y(0.25);
    let unposed = Mat4::from_translation(Vec3::new(0.0, 0.0, -3.0));

    for mirror in [true, false] {
        r.spec_mirror = mirror;
        let rigid_frame = |r: &mut render::Renderer, m: Mat4| {
            frame(
                r,
                &[
                    DrawCmd::textured(rigid, black, m),
                    DrawCmd::specular(rigid, black, [1.0; 3], 20, m),
                ],
                &[],
            )
        };
        let want = rigid_frame(&mut r, pose);
        let flat = rigid_frame(&mut r, unposed);
        let base = skinned_base(skinned, black, pose);
        let got = frame(&mut r, &[], &[base.clone(), base.specular([1.0; 3], 20)]);

        let worst = want
            .iter()
            .zip(&got)
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .max()
            .unwrap();
        assert!(
            worst <= 1,
            "mirror {mirror}: skinned vs rigid, worst {worst}"
        );
        let moved = want
            .iter()
            .zip(&flat)
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .max()
            .unwrap();
        assert!(
            moved > 8,
            "mirror {mirror}: the pose moved the picture by {moved} only"
        );
        let centre = common::pixel(&got, N, N / 2, N / 2);
        assert!(
            centre[0] > 0,
            "mirror {mirror}: the pass drew nothing: {centre:?}"
        );
    }
}
