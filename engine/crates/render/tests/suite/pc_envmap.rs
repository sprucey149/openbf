//! Headless GPU referee for PC Battlefront II environment-mapped materials
//! (`render::EnvDesc`, `analysis/re-pc-envmap.md`).
//!
//! The PC's pass adds `cube(R) · c0` in gamma over the base, R the world
//! reflection, where the diffuse alpha passes an alpha test of reference
//! 0x7f.
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh` (serialised).

use glam::Mat4;
use render::{Camera, DrawCmd, EnvDesc};

use crate::common;
use common::{capture_pixel, headless, tex_desc, view_quad, N};

const T: u8 = 120;
const FACE: u8 = 100;

fn grey_with_cube(r: &mut render::Renderer, alpha: u8) -> render::TextureHandle {
    let texel = [T, T, T, alpha];
    let base: [&[u8]; 1] = [&texel];
    // One texel a face, D3D order: only +Z (the fifth) holds `FACE`.
    let mut faces = [10u8, 10, 10, 255].repeat(6);
    faces[16..19].fill(FACE);
    let mips: [&[u8]; 1] = [&faces];
    let mut desc = tex_desc(1, 1, &base, "env base");
    desc.env = Some(EnvDesc {
        size: 1,
        mips: &mips,
    });
    r.add_texture(&desc).expect("texture upload")
}

fn shot(r: &mut render::Renderer, draws: &[DrawCmd]) -> [u8; 4] {
    r.render(&Camera::default(), draws, [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    capture_pixel(r, N / 2, N / 2)
}

/// The camera looks down -Z at a quad facing it, so R is +Z: the pass adds
/// the +Z face times the colour (half) over the base, and nothing under the
/// alpha reference.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_pc_env_pass_adds_the_reflected_face_above_the_alpha_reference() {
    let mut r = headless();
    let table = [128u8, 128, 128, 255].repeat(4);
    r.set_specular_table(Some((2, 2, &table))).expect("table");
    let quad = view_quad(&mut r, -3.0);
    for (alpha, add) in [(255, f32::from(FACE) * 0.5), (128, f32::from(FACE) * 0.5), (127, 0.0)] {
        let tex = grey_with_cube(&mut r, alpha);
        let base = DrawCmd::textured(quad, tex, Mat4::IDENTITY);
        let without = shot(&mut r, &[base]);
        let with = shot(
            &mut r,
            &[
                base,
                DrawCmd::specular(quad, tex, [0.5, 0.5, 0.5], 10, Mat4::IDENTITY),
            ],
        );
        for c in 0..3 {
            let want = (f32::from(without[c]) + add).min(255.0);
            assert!(
                (f32::from(with[c]) - want).abs() <= 2.0,
                "alpha {alpha}: base {without:?}, got {with:?}, want {want}"
            );
        }
    }
}
