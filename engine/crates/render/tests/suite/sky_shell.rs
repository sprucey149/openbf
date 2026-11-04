//! Headless GPU test for the sky pass.
//!
//! Each map's `sky_ DomeInfo` names dome models that the renderer draws around
//! the camera before anything else, unlit and with no depth at all
//! (`Renderer::set_sky`). Three properties have to hold or the sky either
//! disappears behind geometry or eats it:
//!  1. a shell paints where nothing else draws, even though its mesh sits at a
//!     depth that would normally be behind the far plane's worth of geometry;
//!  2. ordinary geometry draws OVER it regardless of distance, because the sky
//!     writes no depth and tests none;
//!  3. the sky is not lit and NEVER fogged — the dome art already is the sky,
//!     and the sky draw flag 0x20 excludes every sky draw from the fog pass
//!     in both ELFs (`analysis/re-fog.md` §2.4 [V]).
//!
//! Ignored by default — needs a real adapter.
//!   scripts/gpu_tests.sh --render

use glam::Mat4;
use render::{Camera, DrawCmd, Fog, Lighting, MeshData, Renderer, SkyShell};

use crate::common;
use common::{capture_pixel, headless, tex_desc, upload_tex, view_quad, N};

/// A quad at `z` spanning the view, with a flat texture.
fn quad(r: &mut Renderer, rgba: [u8; 4], z: f32) -> (render::MeshHandle, render::TextureHandle) {
    let tex = upload_tex(r, 1, 1, &rgba, "flat");
    (view_quad(r, z), tex)
}

fn centre(r: &mut Renderer) -> [u8; 4] {
    capture_pixel(r, N / 2, N / 2)
}

#[test]
#[ignore = "requires a GPU adapter"]
fn hdr_cloud_keeps_its_full_brightness_without_changing_ordinary_particles() {
    let mut r = headless();
    let tex = upload_tex(&mut r, 1, 1, &[135, 97, 66, 255], "brownmist sample");
    let pos = glam::Vec3::new(0.0, 0.0, -2.0);
    let draw = |gain| render::ParticleDraw {
        shape: render::ParticleShape::Cloud {
            pos,
            half: 20.0,
            angle: 0.0,
            gain,
        },
        key: pos,
        color: [168, 172, 180, 128],
        texture: Some(tex),
        additive: false,
    };
    let clear = [0.0, 0.0, 0.0, 1.0];
    r.set_particles(vec![draw(1.0)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let ordinary = centre(&mut r);
    r.set_particles(vec![draw(2.0)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let hdr = centre(&mut r);
    for (actual, expected) in hdr[..3].iter().zip([89i16, 65, 47]) {
        assert!(
            (i16::from(*actual) - expected).abs() <= 2,
            "HDR cloud: {hdr:?}"
        );
    }
    for i in 0..3 {
        assert!((i16::from(hdr[i]) - i16::from(ordinary[i]) * 2).abs() <= 2);
    }
    let mut particle = draw(2.0);
    particle.shape = render::ParticleShape::Sprite {
        pos,
        half: 20.0,
        angle: 0.0,
        uv: [0.0, 0.0, 1.0, 1.0],
    };
    r.set_particles(vec![particle]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    assert_eq!(centre(&mut r), ordinary);
}

/// The sky paints the background, and an opaque draw in front of it wins —
/// even though the sky quad is nearer the camera than the geometry, because
/// the sky pass tests no depth and writes none.
#[test]
#[ignore = "requires a GPU adapter"]
fn sky_paints_the_background_and_geometry_draws_over_it() {
    let mut r = headless();
    // Sky quad at z = -1 (very close), geometry at z = -20 (far behind it).
    let (sky_mesh, sky_tex) = quad(&mut r, [255, 0, 0, 255], -1.0);
    let (geo_mesh, geo_tex) = quad(&mut r, [0, 0, 255, 255], -20.0);
    r.set_sky(vec![SkyShell {
        mesh: sky_mesh,
        texture: Some(sky_tex),
        color: [1.0; 4],
        scale: 1.0,
        placement: render::Placement::Object {
            offset: glam::Vec3::ZERO,
        },
        yaw: 0.0,
        mode: render::SkyMode::Blend,
        lit: false,
        depth: false,
        emission: None,
    }]);

    // Sky alone: the frame is the shell, not the clear colour.
    r.render(&Camera::default(), &[], [0.0, 1.0, 0.0, 1.0])
        .expect("frame");
    let sky_only = centre(&mut r);
    assert!(
        sky_only[0] > 200 && sky_only[1] < 60,
        "sky shell did not paint the background: {sky_only:?}"
    );

    // With geometry BEHIND the shell in z, the geometry still wins.
    r.render(
        &Camera::default(),
        &[DrawCmd::textured(geo_mesh, geo_tex, Mat4::IDENTITY)],
        [0.0, 1.0, 0.0, 1.0],
    )
    .expect("frame");
    let with_geo = centre(&mut r);
    assert!(
        with_geo[2] > 150 && with_geo[0] < 60,
        "geometry did not draw over the sky shell: {with_geo:?}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn sky_particles_paint_before_world_and_do_not_write_depth() {
    let mut r = headless();
    let texture = upload_tex(&mut r, 1, 1, &[255, 0, 0, 255], "sky effect");
    r.set_sky_particles(vec![render::ParticleDraw {
        shape: render::ParticleShape::Sprite {
            pos: glam::Vec3::new(0.0, 0.0, -2.0),
            half: 20.0,
            angle: 0.0,
            uv: [0.0, 0.0, 1.0, 1.0],
        },
        key: glam::Vec3::new(0.0, 0.0, -2.0),
        color: [255; 4],
        texture: Some(texture),
        additive: false,
    }]);
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .unwrap();
    assert!(centre(&mut r)[0] > 200);
    let (mesh, texture) = quad(&mut r, [0, 0, 255, 255], -20.0);
    r.render(
        &Camera::default(),
        &[DrawCmd::textured(mesh, texture, Mat4::IDENTITY)],
        [0.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    let pixel = centre(&mut r);
    assert!(pixel[2] > 150 && pixel[0] < 60, "{pixel:?}");
    r.clear_scene();
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .unwrap();
    assert_eq!(centre(&mut r)[0], 0);
}

/// `clear_scene` drops the sky: the next frame is the clear colour, as the
/// shell after a battle must be (it drew the map's sky behind its movie).
#[test]
#[ignore = "requires a GPU adapter"]
fn clear_scene_drops_the_sky() {
    let mut r = headless();
    let (mesh, tex) = quad(&mut r, [255, 0, 0, 255], -1.0);
    r.set_sky(vec![SkyShell {
        mesh,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement: render::Placement::Object {
            offset: glam::Vec3::ZERO,
        },
        yaw: 0.0,
        mode: render::SkyMode::Blend,
        lit: false,
        depth: false,
        emission: None,
    }]);
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    assert!(centre(&mut r)[0] > 200, "the sky did not paint");
    r.clear_scene();
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    assert_eq!(
        centre(&mut r),
        [0, 0, 0, 255],
        "the sky outlived clear_scene"
    );
}

/// The sky is unlit and unfogged: darkening the sun and ambient to nothing
/// and switching on a fog that would swallow anything at that distance leaves
/// the shell exactly as bright.
#[test]
#[ignore = "requires a GPU adapter"]
fn sky_ignores_lighting_and_fog() {
    let mut r = headless();
    let (mesh, tex) = quad(&mut r, [200, 120, 60, 255], -1.0);
    r.set_sky(vec![SkyShell {
        mesh,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement: render::Placement::Object {
            offset: glam::Vec3::ZERO,
        },
        yaw: 0.0,
        mode: render::SkyMode::Blend,
        lit: false,
        depth: false,
        emission: None,
    }]);
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    let lit = centre(&mut r);

    r.lighting = Lighting {
        sun_color: [0.0; 3],
        ambient_sky: [0.0; 3],
        ambient_ground: [0.0; 3],
        fog: Some(Fog {
            color: [0.0, 1.0, 0.0],
            near: 0.0,
            far: 0.5,
        }),
        ..Lighting::default()
    };
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    let dark = centre(&mut r);
    assert_eq!(lit, dark, "the sky shell reacted to lighting or fog");
    assert!(
        lit[0] > 150,
        "sky shell should keep its own colour: {lit:?}"
    );
}

/// **No sky shell takes the fog — not even a landscape one.** Until
/// 2026-09-03 a PARALLAX `DomeModel` (an authored `Offset` or a
/// `MovementScale` other than 1.0 — Kamino's city, Hoth's mountains,
/// Tatooine's rock spires) went through a second, FOGGED sky pipeline, on the
/// argument that such a shell is landscape at a distance and that leaving it
/// sharp behind fogged terrain left a seam at the horizon. The ELF says
/// otherwise: the sky tick draws every shell with the draw flag 0x20 set and
/// both fog gates exclude it (BF2 `(flags & 0x20020) == 0x20000` at 723621,
/// BF1 `(flags & 0x20) == 0` at 594590 — `analysis/re-fog.md` §2.4 [V]).
/// There is one sky fragment entry point and it never calls `apply_fog`.
///
/// This referee pins that for BOTH placements and both modes, under a fog
/// dense enough to swallow anything: a `Placement::Dome` with the offset and
/// movement scale that used to classify as parallax, and a
/// `Placement::Object` (a `SkyObject` capital ship), which used to be fogged
/// whenever the map had fog at all.
#[test]
#[ignore = "requires a GPU adapter"]
fn no_shell_takes_the_fog_whatever_its_placement() {
    let mut r = headless();
    let (mesh, tex) = quad(&mut r, [200, 0, 0, 255], -1.0);
    let shell = |placement, mode| SkyShell {
        mesh,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement,
        yaw: 0.0,
        mode,
        lit: false,
        depth: false,
        emission: None,
    };
    // The shapes that used to be routed through the fogged pipeline.
    let placements = [
        (
            "base dome",
            render::Placement::Dome {
                offset_y: 0.0,
                movement_scale: 1.0,
            },
        ),
        (
            "parallax shell (kam1_city: offset 80, movement 0.995)",
            render::Placement::Dome {
                offset_y: 80.0,
                movement_scale: 0.995,
            },
        ),
        (
            "sky object (a capital ship at distance)",
            render::Placement::Object {
                offset: glam::Vec3::new(0.0, 0.0, 0.0),
            },
        ),
    ];
    for (what, placement) in placements {
        for mode in [render::SkyMode::Opaque, render::SkyMode::Blend] {
            r.set_sky(vec![shell(placement, mode)]);
            r.lighting = Lighting::default();
            r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
                .expect("frame");
            let plain = centre(&mut r);
            // Green fog that reaches full strength within a metre: anything
            // that consulted `apply_fog` would come back green.
            r.lighting = Lighting {
                fog: Some(Fog {
                    color: [0.0, 1.0, 0.0],
                    near: 0.0,
                    far: 0.5,
                }),
                ..Lighting::default()
            };
            r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
                .expect("frame");
            assert_eq!(plain, centre(&mut r), "{what} ({mode:?}) took the fog");
            assert!(plain[0] > 150, "{what} lost its own colour: {plain:?}");
        }
    }
}

/// A 2x1 nearest-sampled texture across the same view-spanning quad: the LEFT
/// half of the frame is `left`, the RIGHT half is `right`. Used to put an
/// opaque texel and a part-transparent one in one draw.
fn split_quad(
    r: &mut Renderer,
    left: [u8; 4],
    right: [u8; 4],
) -> (render::MeshHandle, render::TextureHandle) {
    let mut texels = Vec::new();
    texels.extend_from_slice(&left);
    texels.extend_from_slice(&right);
    let tex = upload_tex(r, 2, 1, &texels, "split");
    (view_quad(r, -1.0), tex)
}

/// The two sky modes over ONE fixture, which is what the ELF's per-material
/// GS state buys (`analysis/re-sky-dome-placement.md` §5 [V], `SkyMode`, LEAD
/// CORRECTION 2 of session 12).
///
/// The shell's texture is opaque red on the left and 39 %-alpha blue on the
/// right. The right half is the whole test:
///  * OPAQUE (a word with neither 0x02 nor 0x04 — `mus1_sky_dome`'s
///    0x00010001 / 0x00010010): the shell PAINTS, so the frame takes the
///    texture's RGB and the alpha is ignored. This is the mode the port did
///    not have, and the reason Mustafar's volcano skirt was a hole;
///  * BLEND (0x04, and 0x02 too — under the sky draw flag the ELF sends both
///    down the translucent path, `mus1_sky_volcano`'s 0x00010003 and
///    `myg1_sky_mountains_01`'s 0x00000003): composited, so the frame is part
///    texture and part clear.
///
/// There is no third, alpha-TESTED sky mode: the 0x5000b AREF word is on the
/// non-sky path only.
#[test]
#[ignore = "requires a GPU adapter"]
fn a_sky_shell_paints_or_composites_by_its_material() {
    let mut r = headless();
    let (mesh, tex) = split_quad(&mut r, [255, 0, 0, 255], [0, 0, 255, 100]);
    let shell = |mode| SkyShell {
        mesh,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement: render::Placement::Object {
            offset: glam::Vec3::ZERO,
        },
        yaw: 0.0,
        mode,
        lit: false,
        depth: false,
        emission: None,
    };
    // Green clear, so anything the shell does not paint reads as green.
    let clear = [0.0, 1.0, 0.0, 1.0];
    let (lx, rx, y) = (N / 4, N * 3 / 4, N / 2);

    r.set_sky(vec![shell(render::SkyMode::Opaque)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let opaque_l = capture_pixel(&mut r, lx, y);
    let opaque_r = capture_pixel(&mut r, rx, y);
    assert!(
        opaque_r[2] > 200 && opaque_r[1] < 40,
        "an OPAQUE shell must paint its texture's RGB through a low alpha: {opaque_r:?}"
    );

    r.set_sky(vec![shell(render::SkyMode::Blend)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let blend_l = capture_pixel(&mut r, lx, y);
    let blend_r = capture_pixel(&mut r, rx, y);
    assert!(
        blend_r[1] > 40 && blend_r[2] > 40,
        "a BLEND shell must composite over the clear colour: {blend_r:?}"
    );
    // A 39 %-alpha texel composited keeps LESS of the texture and MORE of the
    // green clear than the same texel painted opaque — and, unlike the sky
    // MASK mode this replaces, it is not cut away.
    assert!(
        blend_r[2] < opaque_r[2] && blend_r[1] > opaque_r[1],
        "BLEND did not land between OPAQUE and the clear: {opaque_r:?} / {blend_r:?}"
    );

    // The OPAQUE half of the texture is the control: both modes agree there,
    // so the only thing the mode changed is what happens to alpha.
    assert_eq!(opaque_l, blend_l, "modes disagreed on an alpha-255 texel");
    assert!(
        opaque_l[0] > 200,
        "the opaque half did not paint: {opaque_l:?}"
    );
}

/// The V address mode, on one fixture: a 4x4 texture whose TOP row is red and
/// whose other three rows are blue, on a quad whose v runs 0.0 at the top of
/// the frame to ~1.13 at the bottom, crossing v = 1.0 at 87 % of the frame's
/// height.
///
/// The bottom band of that quad samples v > 1. With [`WrapV::Repeat`] — what
/// the port did to every texture before 2026-09-03 — it wraps round to the
/// texture's top row and comes back RED: that is the hard stripe across
/// Hoth's and Kamino's horizons, whose base domes end at v = 1.0010 and
/// v = 0.9976 (`engine/artifacts/sky_wrap/dome_uv.txt`). With
/// [`WrapV::ClampToEdge`] it holds the last row and stays BLUE.
///
/// `game::viewer::sky_wrap_v` is what decides which a shell segment gets; this
/// pins that the flag reaches the sampler at all.
#[test]
#[ignore = "requires a GPU adapter"]
fn the_v_wrap_mode_decides_whether_the_bottom_ring_wraps_to_the_top_row() {
    // 4x4: row 0 red, rows 1..3 blue. NEAREST, so the reading is the texel
    // the wrap lands on and not a blend of two.
    let mut texels = vec![0u8; 4 * 4 * 4];
    for (i, px) in texels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        px.copy_from_slice(if i < 4 {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }

    let mips: [&[u8]; 1] = [&texels];
    let shell_of = |r: &mut Renderer, wrap| {
        let mut desc = tex_desc(4, 4, &mips, "v wrap");
        desc.wrap_v = wrap;
        let tex = r.add_texture(&desc).expect("texture");
        // A view-spanning quad, v = 0 at its top edge and 1.2 at its bottom.
        // The quad is +-0.8 at z = -1 and `Camera::default()`'s 70 deg fov_y
        // shows +-tan(35 deg) = +-0.700 there, so the FRAME sees v = 0.062 at
        // its top row and 1.125 at its bottom, crossing 1.0 at 87 % of its
        // height. (A quad whose 1.02 sat on an edge far outside the frustum
        // would never sample past 1.0 on screen at all — measured, first
        // attempt.)
        let mesh = r
            .add_mesh(&MeshData {
                positions: &[
                    [-0.8, -0.8, -1.0],
                    [0.8, -0.8, -1.0],
                    [0.8, 0.8, -1.0],
                    [-0.8, 0.8, -1.0],
                ],
                normals: &[[0.0, 0.0, 1.0]; 4],
                uvs: &[[0.0, 1.2], [1.0, 1.2], [1.0, 0.0], [0.0, 0.0]],
                colors: None,
                indices: &[0, 1, 2, 0, 2, 3],
            })
            .expect("mesh");
        SkyShell {
            mesh,
            texture: Some(tex),
            color: [1.0; 4],
            scale: 1.0,
            placement: render::Placement::Object {
                offset: glam::Vec3::ZERO,
            },
            yaw: 0.0,
            mode: render::SkyMode::Opaque,
            lit: false,
            depth: false,
            emission: None,
        }
    };

    let mut r = headless();
    // Two rows from the bottom of the frame: v there is ~1.12, past the edge.
    let (x, y) = (N / 2, N - 2);

    let repeat = shell_of(&mut r, render::WrapV::Repeat);
    r.set_sky(vec![repeat]);
    r.render(&Camera::default(), &[], [0.0, 1.0, 0.0, 1.0])
        .expect("frame");
    let wrapped = capture_pixel(&mut r, x, y);
    assert!(
        wrapped[0] > 200 && wrapped[2] < 60,
        "REPEAT should wrap v > 1 back to the texture's top row: {wrapped:?}"
    );

    let clamped = shell_of(&mut r, render::WrapV::ClampToEdge);
    r.set_sky(vec![clamped]);
    r.render(&Camera::default(), &[], [0.0, 1.0, 0.0, 1.0])
        .expect("frame");
    let held = capture_pixel(&mut r, x, y);
    assert!(
        held[2] > 200 && held[0] < 60,
        "ClampToEdge should hold the last row instead of wrapping: {held:?}"
    );
}

/// **A `Reflect` shell's mirrored copy lands below the horizon**
/// (`analysis/re-sky-reflect.md` §2, §5 — BF1 Kashyyyk). A band that sits
/// wholly ABOVE the eye (y 0.2..1.0 at z = −2) is drawn once as a
/// `Placement::Dome` and once as a `Placement::Mirror`: the dome paints
/// only above the centre row and the mirror only below it
/// (y → −0.85·y + 0.1, i.e. −0.07..−0.75 at t = 0).
#[test]
#[ignore = "requires a GPU adapter"]
fn a_mirrored_shell_paints_below_the_horizon() {
    let mut r = headless();
    let band = r
        .add_mesh(&MeshData {
            positions: &[
                [-4.0, 0.2, -2.0],
                [4.0, 0.2, -2.0],
                [4.0, 1.0, -2.0],
                [-4.0, 1.0, -2.0],
            ],
            normals: &[[0.0, 0.0, 1.0]; 4],
            uvs: &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            colors: None,
            indices: &[0, 1, 2, 0, 2, 3],
        })
        .expect("mesh upload");
    let tex = upload_tex(&mut r, 1, 1, &[255, 0, 0, 255], "red");
    let shell = |placement| SkyShell {
        mesh: band,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement,
        yaw: 0.0,
        mode: render::SkyMode::Opaque,
        lit: false,
        depth: false,
        emission: None,
    };
    let (dome, mirror) = (
        render::Placement::Dome {
            offset_y: 0.0,
            movement_scale: 1.0,
        },
        render::Placement::Mirror {
            offset_y: 0.0,
            movement_scale: 1.0,
        },
    );
    // Rows 16 and 48 look 0.35 above / below the axis: y = ±0.7 at z = −2.
    let rows = |r: &mut Renderer| {
        let (buf, w, _) = r.capture_rgba().expect("capture");
        (
            common::pixel(&buf, w, N / 2, 16),
            common::pixel(&buf, w, N / 2, 48),
        )
    };
    let red = |p: [u8; 4]| p[0] > 200 && p[1] < 40 && p[2] < 40;
    let clear = [0.0, 0.0, 1.0, 1.0];

    r.set_sky(vec![shell(dome)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let (above, below) = rows(&mut r);
    assert!(red(above) && !red(below), "dome: {above:?} {below:?}");

    r.set_sky(vec![shell(mirror)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let (above, below) = rows(&mut r);
    assert!(!red(above) && red(below), "mirror: {above:?} {below:?}");

    // Both, in the PS2's order (mirror first): the sky above AND below.
    r.set_sky(vec![shell(mirror), shell(dome)]);
    r.render(&Camera::default(), &[], clear).expect("frame");
    let (above, below) = rows(&mut r);
    assert!(red(above) && red(below), "both: {above:?} {below:?}");
}

/// A lit sky shell (`lit: true`, used for `SkyObject` capital ships) responds
/// to sun and ambient lighting while keeping the sky pass's unfogged property
/// and depth-free behavior.
#[test]
#[ignore = "requires a GPU adapter"]
fn sky_lit_shell_takes_sun_lighting_and_ignores_fog() {
    let mut r = headless();
    let (mesh, tex) = quad(&mut r, [255, 255, 255, 255], -1.0);
    r.set_sky(vec![SkyShell {
        mesh,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement: render::Placement::Object {
            offset: glam::Vec3::ZERO,
        },
        yaw: 0.0,
        mode: render::SkyMode::Blend,
        lit: true,
        depth: false,
        emission: None,
    }]);

    // Sun facing the quad directly with color (1.0, 0.5, 0.25), zero ambient.
    // Dense green fog that would turn anything fogged pure green.
    r.lighting = Lighting {
        sun_color: [1.0, 0.5, 0.25],
        sun_dir: glam::Vec3::new(0.0, 0.0, -1.0),
        ambient_sky: [0.0; 3],
        ambient_ground: [0.0; 3],
        fog: Some(Fog {
            color: [0.0, 1.0, 0.0],
            near: 0.1,
            far: 0.5,
        }),
        ..Lighting::default()
    };

    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    let lit = centre(&mut r);

    // Should receive sun color (R > G > B in sRGB ~ 255, 188, 137), NOT fogged green.
    assert!(
        lit[0] > 240 && lit[1] > 170 && lit[1] < 200 && lit[2] > 120 && lit[2] < 150,
        "lit sky shell did not take sun lighting: {lit:?}"
    );

    // If sun is shut off, it becomes dark (unlike unlit shells).
    r.lighting.sun_color = [0.0; 3];
    r.render(&Camera::default(), &[], [0.0, 0.0, 0.0, 1.0])
        .expect("frame");
    let unlit = centre(&mut r);
    assert!(
        unlit[0] < 10 && unlit[1] < 10 && unlit[2] < 10,
        "lit sky shell remained bright with sun turned off: {unlit:?}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn sky_glow_uses_alpha_as_gain_and_bloom_stays_behind_world_and_clouds() {
    let mut r = headless();
    let mesh = view_quad(&mut r, -1.0);
    let tex = upload_tex(
        &mut r,
        2,
        1,
        &[180, 120, 60, 0, 180, 120, 60, 255],
        "sky glow mask",
    );
    let shell = SkyShell {
        mesh,
        texture: Some(tex),
        color: [1.0; 4],
        scale: 1.0,
        placement: render::Placement::Object {
            offset: glam::Vec3::ZERO,
        },
        yaw: 0.0,
        mode: render::SkyMode::Opaque,
        lit: false,
        depth: false,
        emission: Some(render::LightPath::Glow),
    };
    r.lighting.fog = Some(Fog {
        color: [0.0, 1.0, 0.0],
        near: 0.01,
        far: 0.02,
    });
    let clear = [0.0, 0.0, 0.0, 1.0];
    r.set_sky(vec![shell]);
    r.render(&Camera::default(), &[], clear).unwrap();
    let no_gain = capture_pixel(&mut r, N / 4, N / 2);
    let gain = capture_pixel(&mut r, 3 * N / 4, N / 2);
    for (actual, expected) in no_gain[..3].iter().zip([180i16, 120, 60]) {
        assert!((i16::from(*actual) - expected).abs() <= 1, "{no_gain:?}");
    }
    assert!(gain[0] > 250 && gain[1] > 250 && gain[2] > 170, "{gain:?}");
    let bloom = Some(render::Bloom {
        passes: 5,
        weight: 1.2,
        threshold: 0.5,
    });
    r.set_bloom(bloom);
    r.render(&Camera::default(), &[], clear).unwrap();
    let halo = capture_pixel(&mut r, N / 2 - 1, N / 2);
    assert!(
        halo[0] > no_gain[0] && halo[2] > no_gain[2],
        "missing sky halo: {halo:?}"
    );
    let (cover_mesh, cover_tex) = quad(&mut r, [16, 32, 48, 255], -1.0);
    let draws = [DrawCmd::textured(
        cover_mesh,
        cover_tex,
        Mat4::from_scale(glam::Vec3::new(100.0, 100.0, 20.0)),
    )];
    r.render(&Camera::default(), &draws, clear).unwrap();
    let covered = r.capture_rgba().unwrap().0;
    r.set_bloom(None);
    r.render(&Camera::default(), &draws, clear).unwrap();
    assert!(
        covered == r.capture_rgba().unwrap().0,
        "sky bloom leaked through world geometry"
    );
    let cover = SkyShell {
        mesh: cover_mesh,
        texture: Some(cover_tex),
        emission: None,
        ..shell
    };
    r.set_sky(vec![shell, cover]);
    r.render(&Camera::default(), &[], clear).unwrap();
    let covered = r.capture_rgba().unwrap().0;
    r.set_bloom(bloom);
    r.render(&Camera::default(), &[], clear).unwrap();
    assert!(
        covered == r.capture_rgba().unwrap().0,
        "sky bloom leaked through an opaque sky layer"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn sky_objects_depth_test_their_own_hull_and_the_world() {
    for lit in [false, true] {
        let mut r = headless();
        r.lighting.sun_color = [0.0; 3];
        r.lighting.ambient_sky = [1.0; 3];
        r.lighting.ambient_ground = [1.0; 3];
        let near = view_quad(&mut r, -1.0);
        let far = view_quad(&mut r, -1.5);
        let red = upload_tex(&mut r, 1, 1, &[255, 0, 0, 255], "near hull");
        let blue = upload_tex(&mut r, 1, 1, &[0, 0, 255, 255], "rear hull");
        let shell = SkyShell {
            mesh: near,
            texture: Some(red),
            color: [1.0; 4],
            scale: 1.0,
            placement: render::Placement::Object {
                offset: glam::Vec3::ZERO,
            },
            yaw: 0.0,
            mode: render::SkyMode::Opaque,
            lit,
            depth: true,
            emission: None,
        };
        r.set_sky(vec![
            shell,
            SkyShell {
                mesh: far,
                texture: Some(blue),
                ..shell
            },
        ]);
        let clear = [0.0, 0.0, 0.0, 1.0];
        r.render(&Camera::default(), &[], clear).unwrap();
        assert_eq!(
            centre(&mut r),
            [255, 0, 0, 255],
            "rear hull painted over front hull"
        );
        let (behind, green) = quad(&mut r, [0, 255, 0, 255], -2.0);
        r.set_sky_particles(vec![render::ParticleDraw {
            shape: render::ParticleShape::Sprite {
                pos: glam::Vec3::new(0.0, 0.0, -0.5),
                half: 20.0,
                angle: 0.0,
                uv: [0.0, 0.0, 1.0, 1.0],
            },
            key: glam::Vec3::new(0.0, 0.0, -0.5),
            color: [255; 4],
            texture: Some(green),
            additive: false,
        }]);
        r.render(&Camera::default(), &[], clear).unwrap();
        assert_eq!(
            centre(&mut r),
            [255, 0, 0, 255],
            "dome effects painted over a ship hull"
        );
        r.set_sky_particles(Vec::new());
        r.render(
            &Camera::default(),
            &[DrawCmd::textured(behind, green, Mat4::IDENTITY)],
            clear,
        )
        .unwrap();
        assert_eq!(
            centre(&mut r),
            [255, 0, 0, 255],
            "distant world painted through hull"
        );
        let front = view_quad(&mut r, -0.5);
        r.render(
            &Camera::default(),
            &[DrawCmd::textured(front, green, Mat4::IDENTITY)],
            clear,
        )
        .unwrap();
        assert_eq!(
            centre(&mut r),
            [0, 255, 0, 255],
            "ship covered nearby terrain"
        );
        let glass = upload_tex(&mut r, 1, 1, &[255, 0, 0, 128], "front glass");
        r.set_sky(vec![
            SkyShell {
                texture: Some(glass),
                mode: render::SkyMode::Blend,
                ..shell
            },
            SkyShell {
                mesh: far,
                texture: Some(blue),
                ..shell
            },
        ]);
        r.render(&Camera::default(), &[], clear).unwrap();
        let pixel = centre(&mut r);
        assert!(
            pixel[0] > 150 && pixel[2] > 150,
            "opaque hull covered transparent foreground: {pixel:?}"
        );
        r.set_sky(vec![SkyShell {
            texture: Some(glass),
            mode: render::SkyMode::Blend,
            ..shell
        }]);
        r.render(
            &Camera::default(),
            &[DrawCmd::textured(far, blue, Mat4::IDENTITY)],
            clear,
        )
        .unwrap();
        let pixel = centre(&mut r);
        assert!(
            pixel[0] > 150 && pixel[2] > 150,
            "distant world covered transparent foreground: {pixel:?}"
        );
    }
}
