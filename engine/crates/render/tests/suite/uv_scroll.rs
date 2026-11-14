//! Headless GPU test for the scrolling-UV material.
//!
//! A map's water surface scrolls its texture at the velocity the LVL's
//! `fx__ Effect("Water")` authors (`Velocity / INFO5`, `mungefs::fx`, queue W1
//! 2026-09-09; the pre-`fx__` rule read `WATR -> LAYR`'s dead copy, which both
//! ELFs drop). The scroll is a per-material
//! `[du/dt, dv/dt]` multiplied by [`render::Renderer::time`] in the fragment
//! shader, so two properties have to hold:
//!  1. a scrolling material actually MOVES as the clock advances;
//!  2. a material with zero scroll is byte-identical at any clock value —
//!     which is what says this change cannot move the committed goldens.
//!
//! Ignored by default — needs a real adapter.
//!   scripts/gpu_tests.sh --render

use glam::Mat4;
use render::{Camera, DrawCmd, Renderer};

use crate::common;
use common::{headless, tex_desc, view_quad};

/// A 2x1 texture — black then white — on a quad filling the view. Scrolling
/// it in U slides the boundary across the frame.
fn striped_quad(r: &mut Renderer, scroll: [f32; 2]) -> (render::MeshHandle, render::TextureHandle) {
    let texels = [0u8, 0, 0, 255, 255, 255, 255, 255];
    let mips: [&[u8]; 1] = [&texels];
    let mut desc = tex_desc(2, 1, &mips, "stripes");
    desc.scroll = scroll;
    let tex = r.add_texture(&desc).expect("texture");
    (view_quad(r, -2.0), tex)
}

fn frame(r: &mut Renderer, mesh: render::MeshHandle, tex: render::TextureHandle) -> Vec<u8> {
    r.render(
        &Camera::default(),
        &[DrawCmd::textured(mesh, tex, Mat4::IDENTITY)],
        [0.0, 1.0, 0.0, 1.0],
    )
    .expect("frame");
    r.capture_rgba().expect("capture").0
}

#[test]
#[ignore = "requires a GPU adapter"]
fn a_scrolling_material_moves_with_the_clock() {
    let mut r = headless();
    let (mesh, tex) = striped_quad(&mut r, [0.5, 0.0]);

    r.time = 0.0;
    r.water_time = 0.0;
    let t0 = frame(&mut r, mesh, tex);
    // Half a texture width later the stripes have swapped places.
    r.time = 1.0;
    r.water_time = 1.0;
    let t1 = frame(&mut r, mesh, tex);
    assert_ne!(t0, t1, "a scrolling material did not move");

    // A whole texture width brings it exactly back.
    r.time = 2.0;
    r.water_time = 2.0;
    let t2 = frame(&mut r, mesh, tex);
    assert_eq!(t0, t2, "one full period did not return to the start");
}

/// The property the goldens depend on: every texture but a water surface has
/// `scroll: [0.0, 0.0]`, and `uv + 0.0 * time` is exactly `uv`, so advancing
/// the clock cannot change a single pixel of anything else.
#[test]
#[ignore = "requires a GPU adapter"]
fn a_still_material_is_identical_at_any_time() {
    let mut r = headless();
    let (mesh, tex) = striped_quad(&mut r, [0.0, 0.0]);

    r.time = 0.0;
    r.water_time = 0.0;
    let t0 = frame(&mut r, mesh, tex);
    for t in [0.5, 17.0, render::TIME_WRAP] {
        r.time = t;
        r.water_time = f64::from(t);
        assert_eq!(
            t0,
            frame(&mut r, mesh, tex),
            "still material moved at t={t}"
        );
    }
}

/// A tread's offset (`game::viewer::treads`): a texture instance shifted by
/// half a width shows the base's stripes swapped, a whole width brings it
/// back, and the base handle it was made from does not move.
#[test]
#[ignore = "requires a GPU adapter"]
fn a_texture_instance_offsets_alone() {
    let mut r = headless();
    let (mesh, base) = striped_quad(&mut r, [0.0, 0.0]);
    let tread = r.add_texture_instance(base).expect("instance");
    let still = frame(&mut r, mesh, base);
    assert_eq!(still, frame(&mut r, mesh, tread), "a fresh instance is the base");

    r.set_texture_uv_offset(tread, [0.5, 0.0]);
    let half = frame(&mut r, mesh, tread);
    assert_ne!(still, half, "the offset did not move the instance");
    assert_eq!(still, frame(&mut r, mesh, base), "the offset moved the base");

    r.set_texture_uv_offset(tread, [1.0, 0.0]);
    assert_eq!(still, frame(&mut r, mesh, tread), "a whole width did not return");
}
