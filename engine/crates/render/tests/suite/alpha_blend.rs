//! Headless GPU tests for the three alpha passes.
//!
//! ALPHA RULE v2 makes the alpha mode a property of the DRAW, not of the
//! texture: `bake::alpha` writes OPAQUE / MASK / BLEND on each GLB primitive's
//! material and the runtime picks the pipeline from it. The three differ only
//! here:
//!   * Opaque — no alpha test, depth written. An OPAQUE material's alpha is
//!     not a mask (grime, specular, a scroll ramp).
//!   * Mask — the shader's MASK test (`MASK_CUTOFF`, the disc's alpha ≥ 193),
//!     depth written, same sort and instancing.
//!   * Blend — src-alpha compositing, depth writes off, sorted far to near.
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh`.

use glam::Mat4;
use render::{Camera, DrawCmd, DrawLayer};

use crate::common;
use common::{capture_pixel, headless, upload_tex, view_quad, N};

/// One quad at `z`, filling the view, textured with `texels` (a `1 x n`
/// column, top row first). `layer` picks the pass. Returns the frame pixel at
/// `(x, y)`.
fn render_at(layers: &[(&[[u8; 4]], f32, DrawLayer)], x: u32, y: u32) -> [u8; 4] {
    let mut r = headless();
    let mut draws = Vec::new();
    for &(texels, z, layer) in layers {
        let flat: Vec<u8> = texels.iter().flatten().copied().collect();
        let tex = upload_tex(&mut r, 1, texels.len() as u32, &flat, "layer");
        let mesh = view_quad(&mut r, z);
        draws.push(match layer {
            DrawLayer::Mask => DrawCmd::textured_masked(mesh, tex, Mat4::IDENTITY),
            DrawLayer::Blend => DrawCmd::textured_blended(mesh, tex, Mat4::IDENTITY),
            _ => DrawCmd::textured(mesh, tex, Mat4::IDENTITY),
        });
    }
    r.render(&Camera::default(), &draws, [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    capture_pixel(&mut r, x, y)
}

/// As [`render_at`], sampling the centre of the frame.
fn render_layers(layers: &[(&[[u8; 4]], f32, DrawLayer)]) -> [u8; 4] {
    render_at(layers, N / 2, N / 2)
}

/// A uniform 1x1 texture.
fn flat(rgba: [u8; 4]) -> [[u8; 4]; 1] {
    [rgba]
}

/// The whole point: a texture whose every texel is below the alpha test draws
/// NOTHING when alpha-tested, and something when composited.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn sub_test_alpha_is_invisible_tested_and_visible_blended() {
    // Alpha 64 (0.25) — under the MASK test, like the shipped water/glass.
    let ghost = flat([255, 255, 255, 64]);
    let tested = render_layers(&[(&ghost, -5.0, DrawLayer::Mask)]);
    assert_eq!(
        tested,
        [0, 0, 0, 255],
        "MASK: every fragment must be discarded, leaving the clear color"
    );

    let blended = render_layers(&[(&ghost, -5.0, DrawLayer::Blend)]);
    assert!(
        blended[0] > 20,
        "BLEND: the surface must appear over the clear color, got {blended:?}"
    );
    assert!(
        blended[0] < 200,
        "BLEND: at alpha 0.25 it must not be opaque, got {blended:?}"
    );

    // …and the same texture on an OPAQUE draw is SOLID. This is ALPHA RULE
    // v2's whole reason to exist: `kam_crate`'s alpha is grime, not a mask,
    // and reading it as one shredded the Kamino crates.
    let opaque = render_layers(&[(&ghost, -5.0, DrawLayer::Opaque)]);
    assert!(
        opaque[0] > 200,
        "OPAQUE: the alpha must be ignored entirely, got {opaque:?}"
    );
}

/// The MASK pipeline cuts a texture in half at its alpha test and the OPAQUE
/// pipeline draws the same texture whole — the two pipelines differ ONLY in
/// the discard, and the mode is chosen per draw.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn mask_discards_the_low_half_the_opaque_pipeline_keeps() {
    // A 1x2 column: top texel opaque, bottom texel fully transparent. The
    // quad's UVs put v=0 at the top of the frame.
    let checker = [[255u8, 255, 255, 255], [255, 255, 255, 0]];
    let masked_top = render_at(&[(&checker, -5.0, DrawLayer::Mask)], 32, 16);
    let masked_bottom = render_at(&[(&checker, -5.0, DrawLayer::Mask)], 32, 48);
    assert!(
        masked_top[0] > 200,
        "MASK: the alpha-255 half must draw, got {masked_top:?}"
    );
    assert_eq!(
        masked_bottom,
        [0, 0, 0, 255],
        "MASK: the alpha-0 half must be discarded, leaving the clear color"
    );

    let opaque_top = render_at(&[(&checker, -5.0, DrawLayer::Opaque)], 32, 16);
    let opaque_bottom = render_at(&[(&checker, -5.0, DrawLayer::Opaque)], 32, 48);
    assert!(
        opaque_top[0] > 200,
        "OPAQUE: the alpha-255 half must draw, got {opaque_top:?}"
    );
    assert!(
        opaque_bottom[0] > 200,
        "OPAQUE: the alpha-0 half must draw SOLID too, got {opaque_bottom:?}"
    );
}

/// A masked draw writes depth like an opaque one, so it occludes what is
/// behind it wherever it is not discarded — and does not where it is.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_masked_draw_writes_depth_where_it_keeps_and_not_where_it_cuts() {
    let checker = [[0u8, 255, 0, 255], [0, 255, 0, 0]];
    let red = flat([255, 0, 0, 255]);
    let scene: [(&[[u8; 4]], f32, DrawLayer); 2] = [
        (&checker, -4.0, DrawLayer::Mask),
        (&red, -6.0, DrawLayer::Opaque),
    ];
    let top = render_at(&scene, 32, 16);
    let bottom = render_at(&scene, 32, 48);
    assert!(
        top[1] > 100 && top[0] < 60,
        "the kept half must hide the red behind it, got {top:?}"
    );
    assert!(
        bottom[0] > 100,
        "the discarded half must let the red through, got {bottom:?}"
    );
}

/// A blended surface must not OCCLUDE what is behind it: the blend pipeline
/// keeps depth testing but turns depth writes off, and the pass runs after
/// every opaque draw regardless of submission order.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn blended_surface_composites_over_opaque_behind_it() {
    let opaque_red = flat([255, 0, 0, 255]);
    let ghost_blue = flat([0, 0, 255, 64]);

    // Submit the near transparent quad FIRST and the far opaque one second —
    // if the pass order were submission order, the red would be lost.
    let out = render_layers(&[
        (&ghost_blue, -4.0, DrawLayer::Blend),
        (&opaque_red, -6.0, DrawLayer::Opaque),
    ]);
    assert!(
        out[0] > 60,
        "the opaque surface behind must still show through, got {out:?}"
    );
    assert!(
        out[2] > 15,
        "the transparent surface in front must tint it, got {out:?}"
    );

    // Control: the same two quads with the near one opaque hides the red.
    let opaque_blue = flat([0, 0, 255, 255]);
    let hidden = render_layers(&[
        (&opaque_blue, -4.0, DrawLayer::Opaque),
        (&opaque_red, -6.0, DrawLayer::Opaque),
    ]);
    assert!(
        hidden[0] < 30,
        "an OPAQUE near surface must hide the far one, got {hidden:?}"
    );
}
