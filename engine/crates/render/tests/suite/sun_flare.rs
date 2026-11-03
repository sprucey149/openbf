use crate::common::{capture_pixel, headless, upload_tex, view_quad};
use glam::Mat4;
use render::{Camera, DrawCmd, SunFlare};

fn flare() -> SunFlare {
    SunFlare {
        distance: 720.0,
        display_gain: 1.0,
        angle: [90.0, -90.0],
        color: [255.0; 4],
        size: 5.0,
        flare_out_size: 20.0,
        num_flare_outs: 40,
        initial_alpha: 70.0,
        halo_rings: [
            [0.0, 255.0, 255.0, 255.0, 255.0],
            [10.0, 255.0, 200.0, 0.0, 255.0],
            [30.0, 255.0, 127.0, 0.0, 0.0],
        ],
        spike_color: [230.0, 230.0, 0.0, 128.0],
        spike_size: 20.0,
    }
}

#[test]
#[ignore = "GPU referee: scripts/gpu_tests.sh"]
fn sun_and_halo_are_hidden_by_world_depth_and_clear_with_the_scene() {
    let mut r = headless();
    let cam = Camera::default();
    r.set_sun_flare(Some(flare()));
    r.render(&cam, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert!(capture_pixel(&mut r, 32, 32)[0] > 240);
    assert!(capture_pixel(&mut r, 38, 32)[0] > 10);
    let texture = upload_tex(&mut r, 1, 1, &[0, 0, 0, 255], "roof");
    let mesh = view_quad(&mut r, -3.0);
    r.render(
        &cam,
        &[DrawCmd::textured(mesh, texture, Mat4::IDENTITY)],
        [0.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    assert_eq!(capture_pixel(&mut r, 32, 32), [0, 0, 0, 255]);
    assert_eq!(capture_pixel(&mut r, 38, 32), [0, 0, 0, 255]);
    let narrow = Mat4::from_scale(glam::Vec3::new(0.04, 1.0, 1.0));
    r.render(
        &cam,
        &[DrawCmd::textured(mesh, texture, narrow)],
        [0.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    assert!(capture_pixel(&mut r, 38, 32)[0] > 10);
    assert_eq!(capture_pixel(&mut r, 10, 32), [0, 0, 0, 255]);
    r.clear_scene();
    r.render(&cam, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert_eq!(capture_pixel(&mut r, 32, 32), [0, 0, 0, 255]);
}

#[test]
#[ignore = "GPU referee: scripts/gpu_tests.sh"]
fn alignment_glare_lifts_bright_foreground_and_a_visible_sun_edge_keeps_its_flare() {
    let mut r = headless();
    let cam = Camera::default();
    let gray = upload_tex(&mut r, 1, 1, &[180, 180, 180, 255], "snow");
    let mesh = view_quad(&mut r, -3.0);
    let ground = DrawCmd::textured(
        mesh,
        gray,
        Mat4::from_translation(glam::Vec3::new(0.0, -4.8, 0.0)),
    );
    r.render(&cam, &[ground], [0.0, 0.0, 0.0, 1.0]).unwrap();
    let before = capture_pixel(&mut r, 32, 52);
    r.set_sun_flare(Some(flare()));
    r.render(&cam, &[ground], [0.0, 0.0, 0.0, 1.0]).unwrap();
    let after = capture_pixel(&mut r, 32, 52);
    assert!(
        u16::from(after[0]) > u16::from(before[0]) + 20,
        "{before:?} -> {after:?}"
    );
    let edge = Camera { yaw: 0.63, ..cam };
    r.render(&edge, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert!(capture_pixel(&mut r, 62, 32)[0] > 100);
    let away = Camera {
        yaw: std::f32::consts::PI,
        ..cam
    };
    r.render(&away, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert_eq!(capture_pixel(&mut r, 32, 32), [0, 0, 0, 255]);
}
