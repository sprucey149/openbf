//! Headless GPU referee for PC Battlefront II bump materials
//! (`render::BumpDesc`, `analysis/re-pc-bump.md`).
//!
//! The PC's `bump` pixel shader lights the texel by `sat(ambient + sun ·
//! sat(N_b · L))` with N_b the normal map, so its light stops at 1 where the
//! PS2's GS sum runs to 2 (§2), and N_b, not the vertex normal, takes the sun.
//! The spec pass adds `lobe · c0 · c2 · gloss` from the PC cube's +Y face, in
//! gamma over the base, and nothing without a face (§3).
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh` (serialised).

use glam::{Mat4, Vec3};
use render::{BumpDesc, Camera, DrawCmd, Lighting};

use crate::common;
use common::{capture_pixel, headless, tex_desc, view_quad, N};

const T: u8 = 160;
const A: u8 = 100;
const S: u8 = 200;

fn lin(b: u8) -> f32 {
    let d = f32::from(b) / 255.0;
    if d <= 0.040_45 {
        d / 12.92
    } else {
        ((d + 0.055) / 1.055).powf(2.4)
    }
}

/// Ambient 100 and a sun of 200 head on to the camera-facing quad.
fn lit() -> render::Renderer {
    let mut r = headless();
    r.lighting = Lighting {
        sun_dir: Vec3::new(0.0, 0.0, -1.0),
        sun_color: [lin(S); 3],
        ambient_sky: [lin(A); 3],
        ambient_ground: [lin(A); 3],
        ..Lighting::default()
    };
    r
}

fn grey_with_normal(r: &mut render::Renderer, normal: [u8; 4]) -> render::TextureHandle {
    let texel = [T, T, T, 255];
    let base: [&[u8]; 1] = [&texel];
    let level: &[u8] = &normal;
    let mut desc = tex_desc(1, 1, &base, "bump base");
    let mips = [level];
    desc.bump = Some(BumpDesc {
        width: 1,
        height: 1,
        mips: &mips,
    });
    r.add_texture(&desc).expect("texture upload")
}

fn shot(r: &mut render::Renderer, draws: &[DrawCmd]) -> [u8; 4] {
    r.render(&Camera::default(), draws, [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    capture_pixel(r, N / 2, N / 2)
}

fn near(px: [u8; 4], want: f32, what: &str) {
    for c in 0..3 {
        assert!(
            (f32::from(px[c]) - want).abs() <= 2.0,
            "{what}: got {px:?}, want {want}"
        );
    }
}

/// The PS2 path sums ambient and sun to 300/255 and multiplies past the
/// texel (188); a flat PC normal map clamps the same sum at 1 (160). A normal
/// map tipped along the quad's tangent takes no sun: the ambient alone (63).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_pc_bump_draw_clamps_its_light_at_one_and_takes_the_sun_by_the_map() {
    let mut r = lit();
    let quad = view_quad(&mut r, -3.0);
    let plain = common::upload_tex(&mut r, 1, 1, &[T, T, T, 255], "plain");
    let ps2 = f32::from(T) * (f32::from(A) + f32::from(S)) / 255.0;
    near(
        shot(&mut r, &[DrawCmd::textured(quad, plain, Mat4::IDENTITY)]),
        ps2,
        "PS2 sum",
    );
    let flat = grey_with_normal(&mut r, [128, 128, 255, 255]);
    near(
        shot(&mut r, &[DrawCmd::textured(quad, flat, Mat4::IDENTITY)]),
        f32::from(T),
        "flat PC normal",
    );
    let tipped = grey_with_normal(&mut r, [255, 128, 128, 255]);
    near(
        shot(&mut r, &[DrawCmd::textured(quad, tipped, Mat4::IDENTITY)]),
        f32::from(T) * f32::from(A) / 255.0,
        "normal along the tangent",
    );
}

/// The spec pass of a bump draw adds nothing without the cube; with a face
/// of 64 everywhere at power 10 (c1 = (1, 0, 0, 0)) it adds 64/255 × the
/// sun's 200/255 × the colour 1 in gamma, weighted by the normal map's alpha.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_pc_bump_spec_pass_adds_the_cube_lobe_by_the_gloss() {
    let mut r = lit();
    let table = [128u8, 128, 128, 255].repeat(4);
    r.set_specular_table(Some((2, 2, &table))).expect("table");
    let quad = view_quad(&mut r, -3.0);
    let tex = grey_with_normal(&mut r, [128, 128, 255, 255]);
    let draws = |t| {
        [
            DrawCmd::textured(quad, t, Mat4::IDENTITY),
            DrawCmd::specular(quad, t, [1.0, 1.0, 1.0], 10, Mat4::IDENTITY),
        ]
    };
    near(shot(&mut r, &draws(tex)), f32::from(T), "no cube");

    let face = [64u8, 64, 64, 64].repeat(4);
    r.set_pc_spec_cube(Some((2, 2, &face))).expect("cube");
    let add = 64.0 / 255.0 * f32::from(S) / 255.0;
    near(
        shot(&mut r, &draws(tex)),
        (f32::from(T) / 255.0 + add) * 255.0,
        "full gloss",
    );
    let half = grey_with_normal(&mut r, [128, 128, 255, 128]);
    near(
        shot(&mut r, &draws(half)),
        (f32::from(T) / 255.0 + add * 128.0 / 255.0) * 255.0,
        "half gloss",
    );
}
