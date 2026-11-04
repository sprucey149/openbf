//! Headless GPU smoke test. Ignored by default — needs a real adapter.
//! Run with `scripts/gpu_tests.sh`.

use glam::Mat4;
use render::{Camera, DrawCmd, MeshData, Renderer};

use crate::common;
use common::{headless, pixel, N};

#[test]
#[ignore = "requires a GPU adapter"]
fn headless_dimensions_respect_the_requested_device_limit() {
    let limit = wgpu::Limits::default().max_texture_dimension_2d;
    let too_large = limit + 1;
    for (width, height) in [(too_large, 1), (1, too_large)] {
        assert!(matches!(
            Renderer::new_headless(width, height),
            Err(render::RenderError::BadTexture(message))
                if message.contains(&format!("{width}x{height}"))
                    && message.contains(&limit.to_string())
        ));
    }
    let renderer = Renderer::new_headless(limit, 1).expect("the limit is a valid size");
    assert_eq!(renderer.size(), (limit, 1));
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn headless_clear_readback_and_triangle() {
    let mut r = headless();

    // 1) Clear-only frame. Linear (1,0,0) is a fixed point of sRGB encoding, so
    //    the readback must be exactly pure red.
    let cam = Camera::default();
    r.render(&cam, &[], [1.0, 0.0, 0.0, 1.0])
        .expect("clear frame");
    let (rgba, w, h) = r.capture_rgba().expect("capture");
    assert_eq!((w, h), (N, N));
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    assert_eq!(
        pixel(&rgba, w, 32, 32),
        [255, 0, 0, 255],
        "center clear color"
    );
    assert_eq!(
        pixel(&rgba, w, 0, 0),
        [255, 0, 0, 255],
        "corner clear color"
    );

    // 2) Full-pipeline frame: flat-green triangle in front of the default camera
    //    (at origin looking down -Z), large enough to cover the center pixel but
    //    not the corners.
    let mesh = r
        .add_mesh(&MeshData {
            positions: &[[-4.0, -4.0, -5.0], [4.0, -4.0, -5.0], [0.0, 4.0, -5.0]],
            normals: &[[0.0, 0.0, 1.0]; 3],
            uvs: &[[0.0, 0.0]; 3],
            colors: None,
            indices: &[0, 1, 2],
        })
        .expect("mesh upload");
    let draw = DrawCmd::flat(mesh, [0.0, 1.0, 0.0, 1.0], Mat4::IDENTITY);
    r.render(&cam, &[draw], [1.0, 0.0, 0.0, 1.0])
        .expect("triangle frame");
    let (rgba, w, _) = r.capture_rgba().expect("capture 2");

    let center = pixel(&rgba, w, 32, 32);
    assert!(
        center[1] > 100,
        "center should be lit green, got {center:?}"
    );
    assert!(
        center[0] < 60,
        "center red channel should be near zero, got {center:?}"
    );
    assert_eq!(
        pixel(&rgba, w, 0, 0),
        [255, 0, 0, 255],
        "corner stays clear color"
    );

    // PNG encoding of a capture must succeed (what --capture-frame will use).
    let png = render::encode_png(&rgba, w, N).expect("png encode");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}
