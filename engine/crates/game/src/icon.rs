//! The application icon, drawn at pack time from the player's own bake: a
//! three-quarter close-up of the clone trooper's helmet on a transparent
//! square. `scripts/pack.py` runs `game --render-icon <out.png>` after the
//! bake, and `scripts/make_icons.py` turns that PNG into every platform's
//! container (`docs/branding.md`). No icon artwork is kept in the repository.
//!
//! The body is the first one baked of [`TROOPERS`]: BF2's clone trooper, then
//! BF1's, so a BF1-only bake still gets one. Every source the bake reads (PC,
//! Xbox, PS2) writes the same `side_<game>/soldiers/<model>.glb` layout.
//!
//! No pipeline writes alpha (`render::encode_png_rgb`'s note), so the
//! background is found by drawing the same frame twice, once over black and
//! once over white: in linear light a pixel of coverage `a` and colour `c`
//! reads `a·c` over black and `a·c + (1 − a)` over white ([`unblend`]).

use crate::assets::load_skinned_glb;
use crate::viewer::{look_at, upload_skinned};
use anyhow::{anyhow, Context, Result};
use glam::{Mat4, Vec3};
use render::{Renderer, SkinnedDrawCmd};
use std::path::{Path, PathBuf};

// The build script's icon step, compiled here so its tests run with the crate's.
#[cfg(test)]
#[path = "../build_icon.rs"]
#[allow(dead_code)]
mod build_icon;

/// The bodies tried, in order, under the bake root: BF2's Republic clone
/// trooper (`rep_inf_ep3_rifleman`'s geometry), BF1's clone trooper, and
/// BF2's Clone Wars trooper.
pub const TROOPERS: [&str; 3] = [
    "side_bf2/soldiers/rep_inf_ep3trooper.glb",
    "side_bf1/soldiers/rep_inf_trooper.glb",
    "side_bf2/soldiers/rep_inf_trooper.glb",
];

/// The default edge of the square, pixels.
pub const ICON_SIZE: u32 = 1024;

/// Supersampling factor: the frame is drawn this many times larger and box
/// filtered down, for smooth edges on the helmet's outline.
const SUPERSAMPLE: u32 = 2;

/// The share of the frame's edge the helmet's outline spans, on its longer
/// side, leaving a margin all round.
const FILL: f32 = 0.86;

/// The first baked trooper body under `assets`.
pub fn find_trooper(assets: &Path) -> Option<PathBuf> {
    TROOPERS
        .iter()
        .map(|rel| assets.join(rel))
        .find(|p| pak::is_file(p))
}

/// The camera's vertical field of view, degrees.
const ICON_FOV_Y_DEG: f32 = 22.0;

/// The head bone: the vertices skinned to it are the helmet, which the frame
/// centres on and the only part drawn.
const HEAD_BONES: [&str; 2] = ["bone_head", "bone_neck"];

/// Draw the icon: `size`×`size` straight-alpha RGBA8 (sRGB-encoded colour).
pub fn render_icon(assets: &Path, size: u32) -> Result<Vec<u8>> {
    let glb = find_trooper(assets).ok_or_else(|| {
        anyhow!(
            "no clone trooper is baked under {} (looked for {})",
            assets.display(),
            TROOPERS.join(", ")
        )
    })?;
    let side = glb
        .parent()
        .and_then(Path::parent)
        .context("the trooper's side folder")?;
    let game = side
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("side_"))
        .unwrap_or("bf2");
    let asset = load_skinned_glb(&glb).with_context(|| glb.display().to_string())?;

    let big = size * SUPERSAMPLE;
    let mut r = Renderer::new_headless(big, big).map_err(|e| anyhow!("renderer: {e}"))?;
    r.lighting = studio_light();
    crate::viewer::install_spec_table(&mut r, side.parent().unwrap_or(assets), game)?;

    // The bind pose: the head upright (the standing idle bows it).
    let pose = crate::skin::Pose::bind(&asset.skeleton);
    let palette = asset.skeleton.palette(&pose.locals());
    let head = HEAD_BONES
        .iter()
        .find_map(|b| asset.skeleton.index_of(b))
        .ok_or_else(|| anyhow!("{} has no head bone", glb.display()))?;
    let (centre, radius) = helmet_bounds(&asset.mesh, &palette, head)
        .ok_or_else(|| anyhow!("{} has no vertex on its head bone", glb.display()))?;
    // The helmet alone: every other bone folds to a point inside it, so the
    // shoulders and chest draw nothing and the neck tucks into the helmet.
    let fold = Mat4::from_cols(
        glam::Vec4::ZERO,
        glam::Vec4::ZERO,
        glam::Vec4::ZERO,
        centre.extend(1.0),
    );
    let palette: Vec<Mat4> = palette
        .iter()
        .enumerate()
        .map(|(j, m)| if j == head { *m } else { fold })
        .collect();

    let mut draws: Vec<SkinnedDrawCmd> = Vec::new();
    for (k, (mesh, mat, layer)) in upload_skinned(&mut r, &asset.mesh)?.into_iter().enumerate() {
        let texture = match mat {
            Some(m) => crate::viewer::upload_texture_file(
                &mut r,
                &side.join("textures"),
                &m.to_ascii_lowercase(),
            )?,
            None => None,
        };
        let spec = crate::soldier_vis::part_spec(&r, asset.mesh.spec_pass(k));
        let base = SkinnedDrawCmd {
            mesh,
            texture,
            color: [1.0; 4],
            model: Mat4::IDENTITY,
            palette: palette.clone().into(),
            layer,
            light: render::LightPath::Character,
            lit: None,
        };
        draws.extend(crate::soldier_vis::with_spec(base, spec));
    }

    let mut shoot = |dist: f32| -> Result<(Vec<u8>, u32)> {
        let cam = icon_camera(centre, dist);
        let mut frames = Vec::new();
        for clear in [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]] {
            r.render_frame(&cam, &[], &draws, clear, None)
                .map_err(|e| anyhow!("render: {e}"))?;
            let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
            if (w, h) != (big, big) {
                return Err(anyhow!("capture is {w}x{h}, not {big}x{big}"));
            }
            frames.push(rgba);
        }
        let rgba = unblend(&frames[0], &frames[1]);
        let ([x0, y0], [x1, y1]) = alpha_box(&rgba, big)
            .ok_or_else(|| anyhow!("the helmet did not draw: every pixel is background"))?;
        Ok((rgba, (x1 - x0).max(y1 - y0) + 1))
    };
    // A first look from the helmet's sphere, then the distance that makes the
    // outline span FILL of the frame (a narrow lens: span ~ 1 / distance).
    let first = radius / (ICON_FOV_Y_DEG.to_radians() * 0.5).sin();
    let (_, span) = shoot(first)?;
    let dist = first * span as f32 / (FILL * big as f32);
    let (rgba, _) = shoot(dist)?;
    Ok(centre_on_alpha(&downsample(&rgba, big, SUPERSAMPLE), size))
}

/// The box around a square image's covered pixels: `([x0, y0], [x1, y1])`,
/// inclusive, or `None` when nothing is covered.
pub fn alpha_box(rgba: &[u8], size: u32) -> Option<([u32; 2], [u32; 2])> {
    let mut lo = [u32::MAX; 2];
    let mut hi = [0u32; 2];
    let mut any = false;
    for (k, px) in rgba.chunks_exact(4).enumerate() {
        if px[3] > 0 {
            let (x, y) = (k as u32 % size, k as u32 / size);
            lo = [lo[0].min(x), lo[1].min(y)];
            hi = [hi[0].max(x), hi[1].max(y)];
            any = true;
        }
    }
    any.then_some((lo, hi))
}

/// Shift a square straight-alpha image so the box around its covered pixels
/// sits in the middle: the helmet's sphere centres the camera, but the
/// helmet's outline is not symmetric about it.
pub fn centre_on_alpha(rgba: &[u8], size: u32) -> Vec<u8> {
    let Some((lo, hi)) = alpha_box(rgba, size) else {
        return rgba.to_vec();
    };
    let n = i64::from(size);
    let dx = (n - 1 - i64::from(hi[0]) - i64::from(lo[0])) / 2;
    let dy = (n - 1 - i64::from(hi[1]) - i64::from(lo[1])) / 2;
    let mut out = vec![0u8; rgba.len()];
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x - dx, y - dy);
            if (0..n).contains(&sx) && (0..n).contains(&sy) {
                let (d, s) = (((y * n + x) * 4) as usize, ((sy * n + sx) * 4) as usize);
                out[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
            }
        }
    }
    out
}

/// The posed helmet: the centre of the box around every vertex skinned
/// mostly to the head bone, and the radius of the sphere through its corners.
fn helmet_bounds(
    mesh: &crate::assets::CpuSkinnedMesh,
    palette: &[Mat4],
    head: usize,
) -> Option<(Vec3, f32)> {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for (k, p) in mesh.positions.iter().enumerate() {
        let (joints, weights) = (mesh.joints.get(k)?, mesh.weights.get(k)?);
        let on_head: f32 = (0..4)
            .filter(|&i| usize::from(joints[i]) == head)
            .map(|i| weights[i])
            .sum();
        if on_head < 0.5 {
            continue;
        }
        let mut v = Vec3::ZERO;
        for i in 0..4 {
            if let Some(m) = palette.get(usize::from(joints[i])) {
                v += m.transform_point3(Vec3::from(*p)) * weights[i];
            }
        }
        lo = lo.min(v);
        hi = hi.max(v);
    }
    (lo.x <= hi.x).then(|| ((lo + hi) * 0.5, (hi - lo).length() * 0.5))
}

/// Models face +Z. The eye sits in front and to the model's left (+X),
/// a little above the visor, `dist` metres from the helmet's centre, and looks
/// at it. A narrow lens keeps the helmet from bulging.
fn icon_camera(target: Vec3, dist: f32) -> render::Camera {
    let azimuth = 35f32.to_radians();
    let elevation = 8f32.to_radians();
    let fov_y = ICON_FOV_Y_DEG.to_radians();
    let dir = Vec3::new(
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
        azimuth.cos() * elevation.cos(),
    );
    let mut cam = look_at(target + dir * dist, target);
    cam.fov_y = fov_y;
    cam.near = 0.05;
    cam.far = 20.0;
    cam
}

/// A key light from above the camera's left, a cool fill from behind on the
/// other side to lift the helmet's edge, and a soft ambient.
fn studio_light() -> render::Lighting {
    render::Lighting {
        // Directions the light travels.
        sun_dir: Vec3::new(-0.35, -0.6, -0.72).normalize(),
        sun_color: [1.1, 1.06, 1.0],
        light2: Some(render::DirLight {
            dir: Vec3::new(0.8, -0.2, 0.55).normalize(),
            color: [0.35, 0.4, 0.5],
            is_static: false,
        }),
        ambient_sky: [0.34, 0.35, 0.39],
        ambient_ground: [0.16, 0.16, 0.18],
        ..render::Lighting::default()
    }
}

fn to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// One straight-alpha RGBA8 frame from the same frame drawn over black and
/// over white (both sRGB-encoded RGBA8). Linear light: over black a pixel is
/// `a·c`, over white `a·c + (1 − a)`, so `1 − a` is their difference.
pub fn unblend(black: &[u8], white: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(black.len());
    for (b, w) in black.chunks_exact(4).zip(white.chunks_exact(4)) {
        let bl = [to_linear(b[0]), to_linear(b[1]), to_linear(b[2])];
        let wl = [to_linear(w[0]), to_linear(w[1]), to_linear(w[2])];
        let gap = (0..3).map(|i| wl[i] - bl[i]).sum::<f32>() / 3.0;
        let a = (1.0 - gap).clamp(0.0, 1.0);
        if a < 1.0 / 255.0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        for c in bl {
            out.push(to_srgb(c / a));
        }
        out.push((a * 255.0).round() as u8);
    }
    out
}

/// Box filter by `factor`, weighting colour by coverage (premultiplied in
/// linear light), so a transparent neighbour does not darken an edge.
pub fn downsample(rgba: &[u8], width: u32, factor: u32) -> Vec<u8> {
    let (w, f) = (width as usize, factor as usize);
    let h = rgba.len() / 4 / w;
    let (ow, oh) = (w / f, h / f);
    let mut out = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        for ox in 0..ow {
            let mut acc = [0f32; 4];
            for dy in 0..f {
                for dx in 0..f {
                    let i = ((oy * f + dy) * w + ox * f + dx) * 4;
                    let a = f32::from(rgba[i + 3]) / 255.0;
                    for c in 0..3 {
                        acc[c] += to_linear(rgba[i + c]) * a;
                    }
                    acc[3] += a;
                }
            }
            let n = (f * f) as f32;
            let a = acc[3] / n;
            if a <= 0.0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            for c in &acc[..3] {
                out.push(to_srgb(c / acc[3]));
            }
            out.push((a * 255.0).round() as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_pixel_is_transparent_and_a_solid_one_keeps_its_colour() {
        let black = [0, 0, 0, 255, 200, 100, 50, 255];
        let white = [255, 255, 255, 255, 200, 100, 50, 255];
        let out = unblend(&black, &white);
        assert_eq!(&out[..4], &[0, 0, 0, 0]);
        assert_eq!(&out[4..], &[200, 100, 50, 255]);
    }

    #[test]
    fn a_half_covered_pixel_comes_back_half_transparent() {
        // Mid grey at half coverage, composed in linear light.
        let c = to_linear(128);
        let black = [to_srgb(0.5 * c); 3];
        let white = [to_srgb(0.5 * c + 0.5); 3];
        let out = unblend(
            &[black[0], black[1], black[2], 255],
            &[white[0], white[1], white[2], 255],
        );
        assert!((i32::from(out[3]) - 128).abs() <= 2, "alpha {}", out[3]);
        assert!((i32::from(out[0]) - 128).abs() <= 3, "colour {}", out[0]);
    }

    #[test]
    fn downsampling_ignores_the_colour_of_transparent_pixels() {
        // Two opaque red, two transparent "white" pixels: half-covered red.
        let rgba = [
            255, 0, 0, 255, 255, 255, 255, 0, //
            255, 0, 0, 255, 255, 255, 255, 0,
        ];
        let out = downsample(&rgba, 2, 2);
        assert_eq!(out, vec![255, 0, 0, 128]);
    }

    #[test]
    fn a_covered_corner_is_moved_to_the_middle() {
        // 4x4, one covered pixel at (0, 0): the box's centre goes to (1, 1).
        let mut rgba = vec![0u8; 4 * 4 * 4];
        rgba[..4].copy_from_slice(&[9, 9, 9, 255]);
        let out = centre_on_alpha(&rgba, 4);
        assert_eq!(alpha_box(&out, 4), Some(([1, 1], [1, 1])));
        assert_eq!(&out[(4 + 1) * 4..(4 + 1) * 4 + 4], &[9, 9, 9, 255]);
        assert_eq!(alpha_box(&[0u8; 16], 2), None);
    }

    #[test]
    fn the_trooper_search_prefers_bf2_then_bf1() {
        assert!(TROOPERS[0].starts_with("side_bf2/"));
        assert!(TROOPERS[1].starts_with("side_bf1/"));
        let none = std::env::temp_dir().join("openbf-icon-no-bake");
        assert_eq!(find_trooper(&none), None);
    }
}
