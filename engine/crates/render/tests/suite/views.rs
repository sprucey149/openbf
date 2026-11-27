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

/// The present's wait for the window's image is its first view's: the later views of a split
/// present take the held image and leave `acquire_ms` alone (the quality governor reads it), and
/// a one-view present writes its own.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn acquire_ms_is_the_first_views_wait() {
    const WAIT_MS: f64 = 20.0;
    let mut r = headless();
    let cell = N / 2;
    let cells = [[0, 0], [cell, 0], [0, cell], [cell, cell]];
    r.acquire_delay = Some(std::time::Duration::from_secs_f64(WAIT_MS / 1000.0));
    for (i, at) in cells.iter().enumerate() {
        r.render_view(
            slot([at[0], at[1], cell, cell], i == 0, i == 3),
            &Camera::default(),
            &[],
            &[],
            [0.0, 0.0, 0.0, 1.0],
            None,
        )
        .expect("view");
        let ms = r.acquire_ms();
        assert!(
            (WAIT_MS..WAIT_MS + 15.0).contains(&ms),
            "after view {i}: {ms} ms"
        );
    }
    // One view is its present's first: it writes its own wait, once.
    r.acquire_delay = None;
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    assert!(r.acquire_ms() < WAIT_MS / 2.0, "{} ms", r.acquire_ms());
    r.acquire_delay = Some(std::time::Duration::from_secs_f64(WAIT_MS / 1000.0));
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    let ms = r.acquire_ms();
    assert!((WAIT_MS..WAIT_MS + 15.0).contains(&ms), "one view: {ms} ms");
}

/// A split view's bloom (composited into a second world target rather than over a copy of the
/// frame, `bloom.rs`) leaves the same pixels as one view of the cell's size, lamp softening
/// included; and the next view's cell is untouched by it.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_split_views_bloom_matches_one_view_of_its_size() {
    use render::{Bloom, Renderer};
    const S: u32 = 128;
    let glow = |r: &mut Renderer| {
        let tex = common::upload_tex(r, 1, 1, &[255, 100, 10, 255], "orange glow");
        let quad = view_quad(r, -3.0);
        let mut d = DrawCmd::textured(quad, tex, Mat4::from_scale(Vec3::splat(0.1)));
        d.model = Mat4::from_translation(Vec3::new(0.0, 0.0, -2.7)) * d.model;
        d.light = LightPath::Glow;
        // A lamp: the composite also softens the frame around it.
        d.color[3] = 2.0;
        r.set_bloom(Some(Bloom {
            passes: 5,
            weight: 1.2,
            threshold: 0.5,
        }));
        d
    };
    let mut one = Renderer::new_headless(S, S).unwrap();
    let d = glow(&mut one);
    one.render(&Camera::default(), &[d], [0.0, 0.0, 0.0, 1.0])
        .unwrap();
    let (want, _, _) = one.capture_rgba().unwrap();
    assert!(
        want.iter().step_by(4).filter(|&&r| r > 0).count() > 100,
        "the glow shows"
    );

    let mut split = Renderer::new_headless(2 * S, S).unwrap();
    let d = [glow(&mut split)];
    for i in 0..2u32 {
        split
            .render_view(
                slot([i * S, 0, S, S], i == 0, i == 1),
                &Camera::default(),
                if i == 0 { &d } else { &[] },
                &[],
                [0.0, 0.0, 0.0, 1.0],
                None,
            )
            .unwrap();
    }
    let (got, w, _) = split.capture_rgba().unwrap();
    for y in 0..S {
        for x in 0..S {
            let at = ((y * S + x) * 4) as usize;
            assert_eq!(
                pixel(&got, w, x, y)[..3],
                want[at..at + 3],
                "left cell ({x},{y})"
            );
            assert_eq!(
                pixel(&got, w, S + x, y)[..3],
                [0, 0, 0],
                "right cell ({x},{y})"
            );
        }
    }
}
