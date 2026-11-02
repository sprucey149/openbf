//! Headless GPU tests of the world drawn smaller than the window
//! (`render::world_scale`) and of the frame's draw order and discard choice.
//! Ignored by default — needs a real adapter (`scripts/gpu_tests.sh`).

use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, LightPath, MeshData, ShadowMesh};

use crate::common;
use common::{headless, view_quad, N};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLACK: [f64; 4] = [0.0, 0.0, 0.0, 1.0];

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_scaled_world_still_fills_the_window() {
    let mut r = headless();
    let quad = view_quad(&mut r, -5.0);
    let d = DrawCmd::flat(quad, RED, Mat4::IDENTITY).with_light(LightPath::Unlit);
    for scale in [1.0, 0.75, 0.5, 0.25] {
        r.set_world_scale(scale);
        r.render(&Camera::default(), &[d], BLACK).expect("frame");
        let (buf, w, h) = r.capture_rgba().expect("capture");
        assert_eq!((w, h), (N, N), "the frame stays the window's size");
        for p in buf.as_chunks::<4>().0 {
            assert!(p[0] > 200 && p[1] < 30, "scale {scale}: {p:?}");
        }
    }
}

/// The overlay paints the real frame after the upscale, at the window's size.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_overlay_draws_at_the_windows_size_over_a_scaled_world() {
    let mut r = headless();
    r.set_world_scale(0.5);
    let quad = view_quad(&mut r, -5.0);
    let d = DrawCmd::flat(quad, RED, Mat4::IDENTITY).with_light(LightPath::Unlit);
    let mut seen = [0, 0];
    let mut overlay = |_: &wgpu::Device,
                       _: &wgpu::Queue,
                       encoder: &mut wgpu::CommandEncoder,
                       view: &wgpu::TextureView,
                       size: [u32; 2]| {
        seen = size;
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    };
    r.render_with_overlay(&Camera::default(), &[d], BLACK, Some(&mut overlay))
        .expect("frame");
    assert_eq!(seen, [N, N]);
    let (buf, _, _) = r.capture_rgba().expect("capture");
    for p in buf.as_chunks::<4>().0 {
        assert!(p[2] > 200 && p[0] < 30, "{p:?}");
    }
}

/// Whatever the order the draws take, the nearer surface shows.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_nearest_surface_shows_whichever_mesh_comes_first() {
    let mut r = headless();
    let far = view_quad(&mut r, -40.0);
    let near = view_quad(&mut r, -5.0);
    let near_low = view_quad(&mut r, -5.0);
    let far_high = view_quad(&mut r, -40.0);
    for (front, back) in [(near, far), (near_low, far_high)] {
        let draws = [
            DrawCmd::flat(back, [0.0, 1.0, 0.0, 1.0], Mat4::IDENTITY).with_light(LightPath::Unlit),
            DrawCmd::flat(front, RED, Mat4::IDENTITY).with_light(LightPath::Unlit),
        ];
        r.render(&Camera::default(), &draws, BLACK).expect("frame");
        let (buf, _, _) = r.capture_rgba().expect("capture");
        for p in buf.as_chunks::<4>().0 {
            assert!(p[0] > 200 && p[1] < 30, "{p:?}");
        }
    }
}

/// A draw whose far cull reaches into it still loses the pixels past the cull,
/// and one wholly short of it draws whole: the frame picks the pipeline that can
/// discard only for the first, and the picture is the shader's rule either way.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_cull_that_reaches_a_draw_still_discards_and_one_that_does_not_draws_it_whole() {
    let mut r = headless();
    let quad = view_quad(&mut r, -5.0);
    let plain = DrawCmd::flat(quad, RED, Mat4::IDENTITY).with_light(LightPath::Unlit);
    let reds = |r: &mut render::Renderer, d: DrawCmd| {
        r.render(&Camera::default(), &[d], BLACK).expect("frame");
        let (buf, _, _) = r.capture_rgba().expect("capture");
        buf.as_chunks::<4>().0.iter().filter(|p| p[0] > 128).count() as u32
    };
    assert_eq!(reds(&mut r, plain.with_cull(Some(100.0))), N * N);
    // The cull falls inside the quad's box, 5 m at its centre and more at its
    // corners, and every fragment reads the corners' distance: the draw is
    // not dropped on the CPU, and only the shader's discard can remove it.
    assert_eq!(reds(&mut r, plain.with_cull(Some(5.5))), 0);
}

/// A vehicle's shadow runs in the world's own target, on the world's own depth,
/// so it falls in the same place whatever size that target is.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_vehicle_shadow_falls_the_same_on_a_scaled_world() {
    let mut r = headless();
    let plane = r
        .add_mesh(&MeshData {
            positions: &[
                [-100., 0., -100.],
                [100., 0., -100.],
                [100., 0., 100.],
                [-100., 0., 100.],
            ],
            normals: &[[0., 1., 0.]; 4],
            uvs: &[[0.; 2]; 4],
            colors: None,
            indices: &[0, 2, 1, 0, 3, 2],
        })
        .unwrap();
    let ground =
        DrawCmd::flat(plane, [0.65, 0.65, 0.65, 1.], Mat4::IDENTITY).with_light(LightPath::Unlit);
    let mut cube = ShadowMesh::new(
        &[
            [-1., 1., -1.],
            [1., 1., -1.],
            [1., 3., -1.],
            [-1., 3., -1.],
            [-1., 1., 1.],
            [1., 1., 1.],
            [1., 3., 1.],
            [-1., 3., 1.],
        ],
        &[
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![3, 7, 6, 2],
            vec![0, 4, 7, 3],
            vec![1, 2, 6, 5],
        ],
    )
    .unwrap();
    let mut volume = Vec::new();
    cube.append_volume(Mat4::IDENTITY, Vec3::new(0., -500., 0.), &mut volume);
    let camera = Camera {
        position: Vec3::new(0., 8., 8.),
        yaw: 0.,
        pitch: -0.75,
        far: 40.,
        ..Default::default()
    };
    let mut frame = |scale: f32| {
        r.set_world_scale(scale);
        r.set_shadow_triangles(&volume);
        r.render(&camera, &[ground], BLACK).expect("frame");
        let (lit, _, _) = r.capture_rgba().expect("capture");
        r.render(&camera, &[ground], BLACK).expect("frame");
        let (bare, _, _) = r.capture_rgba().expect("capture");
        lit.as_chunks::<4>()
            .0
            .iter()
            .zip(bare.as_chunks::<4>().0)
            .filter(|(a, b)| i32::from(a[0]) + 16 < i32::from(b[0]))
            .count() as f32
    };
    let (whole, half) = (frame(1.0), frame(0.5));
    assert!(whole > 0.0, "the shadow shows at full size");
    assert!(
        (half - whole).abs() / whole < 0.15,
        "{whole} shadowed pixels at full size, {half} at half"
    );
}
