//! Headless GPU referee for PC BF2's underwater tint (`render::UnderwaterTint`,
//! `analysis/re-water.md` §9.8): `UnderwaterColor` alpha-blended over exactly
//! the pixels whose near-plane point is at or under the water's level.
//!
//! Ignored by default — needs a real adapter. Run through `scripts/gpu_tests.sh`.

use render::{Camera, UnderwaterTint};

use crate::common::{headless, pixel, N};

fn frame(level: f32) -> Vec<u8> {
    let mut r = headless();
    r.set_underwater(Some(UnderwaterTint {
        level,
        color: [61.0, 124.0, 144.0, 128.0],
    }));
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    r.capture_rgba().expect("capture").0
}

#[test]
#[ignore = "requires a GPU adapter"]
fn the_tint_covers_the_view_under_the_level_only() {
    let rgba = frame(0.0);
    let half = 128.0 / 255.0;
    let want = [61.0 * half, 124.0 * half, 144.0 * half];
    for x in [0, N / 2, N - 1] {
        assert_eq!(pixel(&rgba, N, x, N / 2 - 2), [0, 0, 0, 255], "x {x}");
        let under = pixel(&rgba, N, x, N / 2 + 1);
        for (got, want) in under[..3].iter().zip(want) {
            assert!(
                (f32::from(*got) - want).abs() <= 2.0,
                "x {x}: {under:?} against {want:?}"
            );
        }
        assert_eq!(pixel(&rgba, N, x, N - 1), under, "x {x}");
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn a_level_under_the_whole_near_plane_draws_nothing() {
    let rgba = frame(-1.0);
    assert!(rgba.chunks(4).all(|p| p == [0, 0, 0, 255]));
}

#[test]
#[ignore = "requires a GPU adapter"]
fn a_level_over_the_whole_near_plane_tints_every_pixel() {
    let rgba = frame(1.0);
    let first = pixel(&rgba, N, 0, 0);
    assert_ne!(first, [0, 0, 0, 255]);
    assert!(rgba.chunks(4).all(|p| p == first));
}
