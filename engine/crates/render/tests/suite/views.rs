//! Headless GPU tests of several views sharing one frame (issue #194, splitscreen):
//! `Renderer::render_view` draws each view from its own camera into its own rectangle of the
//! window, in one present. Ignored by default - needs a real adapter (`scripts/gpu_tests.sh`).

use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, LightPath, ViewSlot};

use crate::common;
use common::{headless, pixel, view_quad, N};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

fn slot(rect: [u32; 4], first: bool, last: bool) -> ViewSlot {
    ViewSlot { rect, first, last }
}

fn is(p: [u8; 4], rgb: [u8; 3]) -> bool {
    let near = |a: u8, b: u8| a.abs_diff(b) <= 12;
    near(p[0], rgb[0]) && near(p[1], rgb[1]) && near(p[2], rgb[2])
}

/// Two side-by-side views in one present: the left sees a red quad ahead, the right is turned the
/// other way and sees only its blue clear colour. Each pixel belongs to exactly one view.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn two_views_each_fill_their_own_half_with_their_own_camera() {
    let mut r = headless();
    let quad = view_quad(&mut r, -5.0);
    let draws = [DrawCmd::flat(quad, RED, Mat4::IDENTITY).with_light(LightPath::Unlit)];
    let ahead = Camera::default();
    let behind = Camera {
        yaw: std::f32::consts::PI,
        ..Camera::default()
    };
    let half = N / 2;
    r.render_view(
        slot([0, 0, half, N], true, false),
        &ahead,
        &draws,
        &[],
        [0.0, 0.0, 0.0, 1.0],
        None,
    )
    .expect("view 1");
    r.render_view(
        slot([half, 0, half, N], false, true),
        &behind,
        &draws,
        &[],
        [0.0, 0.0, 1.0, 1.0],
        None,
    )
    .expect("view 2");
    let (buf, w, h) = r.capture_rgba().expect("capture");
    assert_eq!((w, h), (N, N));
    for y in [0, N / 2, N - 1] {
        for x in 0..half {
            assert!(is(pixel(&buf, w, x, y), [255, 0, 0]), "left ({x},{y}) red");
        }
        for x in half..N {
            assert!(
                is(pixel(&buf, w, x, y), [0, 0, 255]),
                "right ({x},{y}) blue"
            );
        }
    }
}

/// Four views in a 2x2 grid, each its own clear colour and the draw list seen or not by its own
/// camera: no view leaves anything in another's cell, and the later views do not clear the earlier.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn four_views_in_a_grid_do_not_bleed_into_each_other() {
    let mut r = headless();
    let quad = view_quad(&mut r, -5.0);
    let draws = [DrawCmd::flat(quad, RED, Mat4::IDENTITY).with_light(LightPath::Unlit)];
    let cell = N / 2;
    let cells = [[0, 0], [cell, 0], [0, cell], [cell, cell]];
    let clears = [
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0, 1.0],
        [0.0, 1.0, 1.0, 1.0],
    ];
    // Views 0 and 3 look at the quad; 1 and 2 look away from it.
    let looks_at = [true, false, false, true];
    for (i, at) in cells.iter().enumerate() {
        let cam = Camera {
            yaw: if looks_at[i] {
                0.0
            } else {
                std::f32::consts::PI
            },
            position: Vec3::ZERO,
            ..Camera::default()
        };
        r.render_view(
            slot([at[0], at[1], cell, cell], i == 0, i == 3),
            &cam,
            &draws,
            &[],
            clears[i],
            None,
        )
        .expect("view");
    }
    let (buf, w, _) = r.capture_rgba().expect("capture");
    let expect = [[255, 0, 0], [0, 0, 255], [255, 255, 0], [255, 0, 0]];
    for (i, at) in cells.iter().enumerate() {
        for (dx, dy) in [
            (0, 0),
            (cell - 1, 0),
            (0, cell - 1),
            (cell - 1, cell - 1),
            (cell / 2, cell / 2),
        ] {
            let p = pixel(&buf, w, at[0] + dx, at[1] + dy);
            assert!(is(p, expect[i]), "cell {i} at ({dx},{dy}): {p:?}");
        }
    }
}

/// The overlay of a view is told the view's size, not the window's.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_views_overlay_gets_the_views_size() {
    let mut r = headless();
    let mut seen = Vec::new();
    for i in 0..2u32 {
        let mut overlay = |_: &wgpu::Device,
                           _: &wgpu::Queue,
                           _: &mut wgpu::CommandEncoder,
                           _: &wgpu::TextureView,
                           size: [u32; 2]| seen.push(size);
        r.render_view(
            slot([i * N / 2, 0, N / 2, N], i == 0, i == 1),
            &Camera::default(),
            &[],
            &[],
            [0.0, 0.0, 0.0, 1.0],
            Some(&mut overlay),
        )
        .expect("view");
    }
    assert_eq!(seen, vec![[N / 2, N], [N / 2, N]]);
}
