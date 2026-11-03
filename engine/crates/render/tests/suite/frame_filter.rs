//! Headless GPU referee for the DomeInfo frame filter (`render::FrameFilter`,
//! card BFLT, `analysis/re-glow.md` §3 `[O]`): the pass's output against the
//! model the PS2 was measured with, applied on the CPU to the same frame
//! rendered without it — `out = min(255, v + I/128 · max(0, box(v) − T))`
//! per channel in display bytes, the box `box_side(height)` pixels wide
//! starting one pixel up-left for a side of 2.
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh` (serialised).

use glam::Mat4;
use render::{box_side, Camera, DrawCmd, FrameFilter, Renderer};

use crate::common;
use common::{upload_tex, view_quad};

/// Render the noise sheet off and on at `w`×`h` and return the worst
/// level the pass is off the n × n model, and how many channels it lifted.
fn pass_against_model(w: u32, h: u32) -> (f32, usize) {
    let mut r = Renderer::new_headless(w, h).expect("headless renderer");
    // A 16×16 noise sheet over the whole view: every level from dark to
    // bright, edges everywhere, so the box and the threshold both matter.
    let mut seed = 12345u32;
    let texels: Vec<u8> = (0..16 * 16 * 4)
        .map(|i| {
            if i % 4 == 3 {
                return 255;
            }
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 16) as u8
        })
        .collect();
    let tex = upload_tex(&mut r, 16, 16, &texels, "noise");
    let quad = view_quad(&mut r, -3.0);
    let draws = [DrawCmd::textured(quad, tex, Mat4::IDENTITY)];
    let cam = Camera::default();

    r.render(&cam, &draws, [0.0, 0.0, 0.0, 1.0])
        .expect("render");
    let (off, w, h) = r.capture_rgba().expect("capture");
    let f = FrameFilter {
        threshold: 50.0,
        intensity: 50.0,
    };
    r.set_frame_filter(Some(f));
    r.render(&cam, &draws, [0.0, 0.0, 0.0, 1.0])
        .expect("render");
    let (on, _, _) = r.capture_rgba().expect("capture");

    let n = box_side(h) as i32;
    let at = |x: i32, y: i32, c: usize| {
        let x = x.clamp(0, w as i32 - 1) as u32;
        let y = y.clamp(0, h as i32 - 1) as u32;
        f32::from(off[((y * w + x) * 4) as usize + c])
    };
    let (mut worst, mut lifted) = (0.0f32, 0usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for c in 0..3 {
                let mut sum = 0.0;
                for dy in 0..n {
                    for dx in 0..n {
                        sum += at(x - n / 2 + dx, y - n / 2 + dy, c);
                    }
                }
                let low = sum / (n * n) as f32;
                let v = at(x, y, c);
                let want = (v + f.intensity / 128.0 * (low - f.threshold).max(0.0)).min(255.0);
                let got = f32::from(on[((y as u32 * w + x as u32) * 4) as usize + c]);
                worst = worst.max((got - want).abs());
                lifted += usize::from(got > v);
            }
        }
    }

    // Off again: the frame is back to the unfiltered bytes exactly.
    r.set_frame_filter(None);
    r.render(&cam, &draws, [0.0, 0.0, 0.0, 1.0])
        .expect("render");
    assert_eq!(r.capture_rgba().expect("capture").0, off);
    (worst, lifted)
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_pass_matches_the_ps2_model() {
    // 448 lines: one PS2 half-scale pixel is a 2×2 box, as on the PS2.
    assert_eq!(box_side(448), 2);
    let (worst, lifted) = pass_against_model(96, 448);
    assert!(worst <= 2.0, "the pass is {worst} levels off the model");
    assert!(
        lifted > (96 * 448) as usize,
        "the filter lifted only {lifted} channels"
    );
}

/// Card ISPF: the box is two 1-D passes (rows, then columns); at 720, 1080
/// and 1920 lines (3 × 3, 5 × 5, 9 × 9: an odd side ends on a single load)
/// it is the n × n box's mean to the rounding of the 8-bit result - no more
/// than one level off the model anywhere (measured 0.55-0.67 on both GPUs).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_two_pass_box_is_the_square_box() {
    for (w, h, n) in [(96, 720, 3), (64, 1080, 5), (48, 1920, 9)] {
        assert_eq!(box_side(h), n);
        let (worst, lifted) = pass_against_model(w, h);
        assert!(
            worst <= 1.0,
            "{h} lines: the pass is {worst} levels off the n x n box"
        );
        assert!(
            lifted > (w * h) as usize,
            "{h} lines: the filter lifted only {lifted} channels"
        );
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn bloom_uses_preclip_marked_light_and_scene_depth() {
    use glam::Vec3;
    use render::{Bloom, LightPath, ParticleDraw, ParticleShape};
    let mut r = Renderer::new_headless(128, 128).unwrap();
    let tex = upload_tex(&mut r, 1, 1, &[255, 100, 10, 255], "orange glow");
    let white = upload_tex(&mut r, 1, 1, &[255; 4], "plain white");
    let quad = view_quad(&mut r, -3.0);
    let mut d = DrawCmd::textured(quad, tex, Mat4::from_scale(Vec3::splat(0.1)));
    d.model = Mat4::from_translation(Vec3::new(0.0, 0.0, -2.7)) * d.model;
    d.light = LightPath::Glow;
    let settings = Bloom {
        passes: 5,
        weight: 1.2,
        threshold: 0.5,
    };
    let cam = Camera::default();
    let mut capture = |draws: &[DrawCmd], bloom| {
        r.set_bloom(bloom);
        r.render(&cam, draws, [0.0, 0.0, 0.0, 1.0]).unwrap();
        r.capture_rgba().unwrap().0
    };
    let off = capture(&[d], None);
    let on = capture(&[d], Some(settings));
    let halo = on
        .chunks_exact(4)
        .zip(off.chunks_exact(4))
        .filter(|(a, b)| b[0] == 0 && a[0] > 1)
        .count();
    assert!(halo > 100, "preclip orange halo has only {halo} pixels");
    d.color[3] = 2.0;
    assert_eq!(
        capture(&[d], None),
        off,
        "bloom gain leaves the base draw alone"
    );
    let boosted = capture(&[d], Some(settings));
    assert!(boosted
        .chunks_exact(4)
        .zip(on.chunks_exact(4))
        .zip(off.chunks_exact(4))
        .any(|((a, b), base)| base[0] == 0 && a[0] > b[0].saturating_add(8)));
    d.color[3] = 1.0;
    d.light = LightPath::Unlit;
    d.texture = Some(white);
    assert_eq!(
        capture(&[d], Some(settings)),
        capture(&[d], None),
        "plain white must not bloom"
    );
    d.light = LightPath::Glow;
    d.texture = Some(tex);
    let dark = upload_tex(&mut r, 1, 1, &[0, 0, 0, 255], "dark occluder");
    let mut blocker = DrawCmd::textured(view_quad(&mut r, -1.0), dark, Mat4::IDENTITY);
    blocker.light = LightPath::Unlit;
    r.set_bloom(None);
    r.render(&cam, &[d, blocker], [0.0; 4]).unwrap();
    let blocked = r.capture_rgba().unwrap().0;
    r.set_bloom(Some(settings));
    r.render(&cam, &[d, blocker], [0.0; 4]).unwrap();
    assert_eq!(
        r.capture_rgba().unwrap().0,
        blocked,
        "opaque wall must hide emissive source"
    );
    let particle = ParticleDraw {
        shape: ParticleShape::Sprite {
            pos: Vec3::new(0.0, 0.0, -3.0),
            half: 0.4,
            angle: 0.0,
            uv: [0.0, 0.0, 1.0, 1.0],
        },
        key: Vec3::new(0.0, 0.0, -3.0),
        color: [255; 4],
        texture: Some(white),
        additive: true,
    };
    r.set_particles(vec![particle.clone()]);
    r.set_bloom(None);
    r.render(&cam, &[], [0.0; 4]).unwrap();
    let single = r.capture_rgba().unwrap().0;
    r.set_bloom(Some(Bloom {
        threshold: 0.6,
        ..settings
    }));
    r.render(&cam, &[], [0.0; 4]).unwrap();
    assert_eq!(
        r.capture_rgba().unwrap().0,
        single,
        "one half-scale additive source must stay below threshold"
    );
    r.set_particles(vec![particle.clone(), particle]);
    r.set_bloom(None);
    r.render(&cam, &[], [0.0; 4]).unwrap();
    let off = r.capture_rgba().unwrap().0;
    r.set_bloom(Some(Bloom {
        threshold: 0.6,
        ..settings
    }));
    r.render(&cam, &[], [0.0; 4]).unwrap();
    let on = r.capture_rgba().unwrap().0;
    assert!(
        on.chunks_exact(4)
            .zip(off.chunks_exact(4))
            .any(|(a, b)| b[0] == 0 && a[0] > 1),
        "overlapping additive light must cross threshold"
    );
    r.set_bloom(None);
    r.render(&cam, &[], [0.0; 4]).unwrap();
    assert_eq!(
        r.capture_rgba().unwrap().0,
        off,
        "disabled state must restore exact frame"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn airborne_leaf_draw_and_scene_clear() {
    use glam::Vec3;
    use render::{Camera, LeafEmitter, LeafObject};
    let mut r = Renderer::new_headless(128, 128).unwrap();
    let texture = upload_tex(&mut r, 1, 1, &[50, 170, 20, 255], "leaf control");
    r.set_leaves(
        Vec::new(),
        vec![LeafObject {
            class: 0,
            matrix: Mat4::from_translation(Vec3::new(0.0, 0.0, -2.0)),
            color: [1.0; 4],
            distance: 100.0,
        }],
        None,
    );
    r.set_leaf_fall(
        vec![LeafEmitter {
            count: 1,
            birds: 0,
            radius: 0.0,
            height: 0.0,
            scale: 1.0,
            distance: 100.0,
        }],
        Some(texture),
        Some(Box::new(|_, _| -10.0)),
    );
    let camera = Camera {
        position: Vec3::new(0.0, 1.0, 0.0),
        pitch: -0.3,
        ..Default::default()
    };
    r.render(&camera, &[], [0.0; 4]).unwrap();
    let rgba = r.capture_rgba().unwrap().0;
    assert!(
        rgba.chunks_exact(4).any(|p| p[1] > 10),
        "airborne pool never reached the scene draw list"
    );
    r.clear_scene();
    r.render(&camera, &[], [0.0; 4]).unwrap();
    assert!(
        r.capture_rgba()
            .unwrap()
            .0
            .chunks_exact(4)
            .all(|p| p[..3] == [0; 3]),
        "leaf survived scene clear"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn bloom_and_surface_keep_the_same_material_clock() {
    use render::{Bloom, LightPath};
    let mut r = Renderer::new_headless(128, 128).unwrap();
    let pixels: Vec<u8> = (0..16)
        .flat_map(|x| {
            if x % 4 == 0 {
                [255, 100, 10, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect();
    let mut desc = common::tex_desc(16, 1, &[], "moving glow");
    let mips = [pixels.as_slice()];
    desc.mips = &mips;
    desc.scroll = [5.0 / 255.0, 0.0];
    let texture = r.add_texture(&desc).unwrap();
    let mesh = view_quad(&mut r, -3.0);
    let mut draw = DrawCmd::textured(mesh, texture, Mat4::IDENTITY);
    draw.light = LightPath::Glow;
    r.set_bloom(Some(Bloom {
        passes: 3,
        weight: 1.0,
        threshold: 0.5,
    }));
    let mut at = |seconds: f64| {
        r.time = (seconds % f64::from(render::TIME_WRAP)) as f32;
        r.water_time = seconds;
        r.render(&Camera::default(), &[draw], [0.0; 4]).unwrap();
        r.capture_rgba().unwrap().0
    };
    let a = at(215.0);
    assert_ne!(a, at(216.0), "scroll control is inert");
    assert_eq!(a, at(2000.0), "bloom detached when the water clock wrapped");
    assert_eq!(at(0.0), at(255.0), "material-period mismatch");
}
