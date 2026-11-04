//! Headless GPU referee for the world shader's lighting paths
//! (`render::LightPath`, queue LITEB): the PS2's VU1 programs for a static
//! model (`analysis/re-model-lighting.md` §2/§3, `[O]`), each pinned to its
//! formula on one quad whose vertex colour stands in for the disc's COL0.
//!
//! The three light terms are put in three different channels — the sun in
//! red, `Light2` in green, the hemisphere ambient in blue — so one pixel says
//! which terms a path took.
//!
//! Ignored by default — needs a real adapter. Run them through
//! `scripts/gpu_tests.sh` (serialised).

use glam::{Mat4, Vec3};
use render::{light_sum, Camera, DirLight, DrawCmd, LightPath, Lighting, MeshData};

use crate::common;
use common::{capture_pixel, headless, upload_tex, N};

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn terrain_light_groups_match_the_linear_shader_exactly() {
    use render::{LitPatch, TerrainLight, TerrainLights};
    let mut r = headless();
    let quad = common::view_quad(&mut r, -3.0);
    let tex = upload_tex(&mut r, 1, 1, &[40, 40, 40, 255], "terrain");
    let draws = [DrawCmd::textured(quad, tex, Mat4::IDENTITY).with_light(LightPath::Terrain)];
    r.set_omniproj(Some((1, 1, &[255; 4]))).unwrap();
    r.render(&Camera::default(), &draws, [0.0; 4]).unwrap();
    let base = r.capture_rgba().unwrap().0;
    let mut patches: Vec<_> = (0..128)
        .map(|i| LitPatch {
            min: [100.0 + i as f32, -4.0],
            max: [101.0 + i as f32, -2.0],
            lights: [0; 3],
            count: 1,
        })
        .collect();
    // The first visible rectangle lies across a group boundary. Another
    // overlaps it with a different light: reordering would change the pixels.
    patches[7] = LitPatch {
        min: [-0.5, -4.0],
        max: [0.5, -2.0],
        lights: [0; 3],
        count: 1,
    };
    patches[8] = LitPatch {
        min: [-2.0, -4.0],
        max: [2.0, -2.0],
        lights: [1; 3],
        count: 1,
    };
    patches[16].lights[0] = 99; // packing rejects an invalid light index
    let lights = vec![
        TerrainLight {
            pos: Vec3::new(0.0, 1.0, -3.0),
            rgb: [0.5, 0.0, 0.0],
        },
        TerrainLight {
            pos: Vec3::new(0.0, 1.0, -3.0),
            rgb: [0.0, 0.5, 0.0],
        },
    ];
    for count in [0, 1, 8, 9, 17, 127, 128] {
        r.set_terrain_lights(TerrainLights {
            lights: lights.clone(),
            patches: patches[..count].to_vec(),
        });
        r.set_terrain_light_grouping(false);
        r.render(&Camera::default(), &draws, [0.0; 4]).unwrap();
        let reference = r.capture_rgba().unwrap().0;
        r.set_terrain_light_grouping(true);
        r.render(&Camera::default(), &draws, [0.0; 4]).unwrap();
        let grouped = r.capture_rgba().unwrap().0;
        assert_eq!(grouped, reference, "{count} input rectangles");
        if count >= 8 {
            assert_ne!(reference, base, "the light must be visible");
        }
    }
}

/// The vertex colour every quad below carries: 128/255 linear (a Unorm8 is
/// not sRGB-decoded), the COL0 the bake writes.
const C: u8 = 128;

/// sRGB encode of a linear value, as the target stores it.
fn srgb(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let s = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

fn close(got: [u8; 4], want: [f32; 3], what: &str) {
    for i in 0..3 {
        let w = srgb(want[i]);
        assert!(
            got[i].abs_diff(w) <= 1,
            "{what}: channel {i} got {got:?}, want {:?} (linear {want:?})",
            want.map(srgb)
        );
    }
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn each_path_takes_the_vu1_terms_it_should() {
    let mut r = headless();
    // Every light hits the camera-facing quad (normal +z) head on; the
    // hemisphere at n.y = 0 is the mean of sky and ground.
    r.lighting = Lighting {
        sun_dir: Vec3::new(0.0, 0.0, -1.0),
        sun_color: [0.25, 0.0, 0.0],
        sun_static: true,
        light2: Some(DirLight {
            dir: Vec3::new(0.0, 0.0, -1.0),
            color: [0.0, 0.25, 0.0],
            is_static: false,
        }),
        ambient_sky: [0.0, 0.0, 0.5],
        ambient_ground: [0.0, 0.0, 0.0],
        ..Lighting::default()
    };
    let white = upload_tex(&mut r, 1, 1, &[255, 255, 255, 255], "white");
    let quad = r
        .add_mesh(&MeshData {
            positions: &[
                [-4.0, -4.0, -3.0],
                [4.0, -4.0, -3.0],
                [4.0, 4.0, -3.0],
                [-4.0, 4.0, -3.0],
            ],
            normals: &[[0.0, 0.0, 1.0]; 4],
            uvs: &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            colors: Some(&[[C, C, C, 255]; 4]),
            indices: &[0, 1, 2, 0, 2, 3],
        })
        .expect("quad");
    let mut shot = |light: LightPath| {
        let d = DrawCmd::textured(quad, white, Mat4::IDENTITY).with_light(light);
        r.render(&Camera::default(), &[d], [0.0, 0.0, 0.0, 1.0])
            .expect("frame");
        capture_pixel(&mut r, N / 2, N / 2)
    };
    let c = f32::from(C) / 255.0;

    // Lit (0x1bb/0x1e6 then 0x521): c × (ambient + sun + Light2) — every
    // light, the Static sun included. The hemisphere blends its two colours
    // in GS units (card LSUM): half the sky's display value.
    close(
        shot(LightPath::Lit),
        [c * 0.25, c * 0.25, light_sum(c, &[(0.5, 0.5)], 0.0)],
        "lit",
    );
    // Prelit (§3): no ambient, the Static sun skipped, Light2 kept, × c.
    close(shot(LightPath::Prelit), [0.0, c * 0.25, 0.0], "prelit");
    // Prelit, COL0 ADDED (0x537): c + the non-Static lights, in GS units.
    let add = light_sum(1.0, &[(c, 1.0), (0.25, 1.0)], 0.0);
    close(shot(LightPath::PrelitAdd), [c, add, c], "prelit_add");
    // Unlit (0x2a3 → 0x2da): c alone, no light at all.
    close(shot(LightPath::Unlit), [c; 3], "unlit");
    let vertex_lit = shot(LightPath::VertexLit);
    close(vertex_lit, [c; 3], "vertex_lit");
    // Unlit, COL0 added to the unit material (0x2a3 → 0x2bf): past white.
    close(shot(LightPath::UnlitAdd), [1.0; 3], "unlit_add");
    // Terrain (0x2a3 → 0x2da on the heightfield): the byte is the GS's, so
    // 128 is unity and no light is taken.
    close(shot(LightPath::Terrain), [1.0; 3], "terrain");
    let dynamic = render::PointLight {
        pos: Vec3::ZERO,
        color: [0.25, 0.0, 0.0],
        lum: 0.25,
        range: 100.0,
        is_static: false,
        spot: false,
        cone: None,
    };
    r.set_point_lights(vec![
        dynamic,
        render::PointLight {
            color: [0.0, 1.0, 0.0],
            is_static: true,
            ..dynamic
        },
    ]);
    let draw = DrawCmd::textured(quad, white, Mat4::IDENTITY)
        .with_light(LightPath::VertexLit)
        .with_lit(Some(render::LitSphere {
            centre: Vec3::new(0.0, 0.0, -3.0),
            radius: 6.0,
        }));
    r.render(&Camera::default(), &[draw], [0.0, 0.0, 0.0, 1.0])
        .unwrap();
    let lit = capture_pixel(&mut r, N / 2, N / 2);
    assert!(
        lit[0] > vertex_lit[0],
        "dynamic point light is added: {lit:?}"
    );
    assert_eq!(
        &lit[1..3],
        &vertex_lit[1..3],
        "static light is already baked"
    );
}

/// The light sum is made in GS (display) units (card LSUM,
/// `analysis/re-kamino-decks.md` §2): one lit texel T under an ambient A and
/// a head-on sun S reads T × (A + S) on the screen — Kamino's blue channel,
/// ambient 100, sun 105 — not the sum of the linearised colours, which the
/// port made before and which is 41 levels darker here (87 against 128).
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn the_lit_sum_is_made_in_display_units() {
    const T: u8 = 160;
    const A: u8 = 100;
    const S: u8 = 105;
    let lin = |b: u8| {
        let d = f32::from(b) / 255.0;
        if d <= 0.040_45 {
            d / 12.92
        } else {
            ((d + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut r = headless();
    r.lighting = Lighting {
        sun_dir: Vec3::new(0.0, 0.0, -1.0),
        sun_color: [lin(S); 3],
        ambient_sky: [lin(A); 3],
        ambient_ground: [lin(A); 3],
        ..Lighting::default()
    };
    assert!(r.lighting.light2.is_none() && !r.lighting.sun_static);
    let tex = upload_tex(&mut r, 1, 1, &[T, T, T, 255], "grey");
    let quad = common::view_quad(&mut r, -3.0);
    r.render(
        &Camera::default(),
        &[DrawCmd::textured(quad, tex, Mat4::IDENTITY)],
        [0.0, 0.0, 0.0, 1.0],
    )
    .expect("frame");
    let got = capture_pixel(&mut r, N / 2, N / 2);
    // The shader's own arithmetic, by its CPU twin.
    let twin = light_sum(lin(T), &[(lin(A), 1.0), (lin(S), 1.0)], 0.0);
    close(got, [twin; 3], "lit, ambient + sun");
    // The PS2's product in display units: 160 × 205 / 255 = 128.6 (the
    // exact sRGB curve's toe leaves the port within 3 levels of it).
    let ps2 = f32::from(T) * (f32::from(A) + f32::from(S)) / 255.0;
    // The linear sum's value: 160 × encode(lin(100) + lin(105)).
    let linear = srgb(lin(T) * (lin(A) + lin(S)));
    for ch in got.iter().take(3) {
        assert!(
            (f32::from(*ch) - ps2).abs() <= 3.0,
            "got {got:?}, the PS2's product {ps2}"
        );
        assert!(
            ch.abs_diff(linear) >= 15,
            "got {got:?} is the linear sum's {linear}"
        );
    }
}

/// BF2's self-lit paths (card GLOW, `analysis/re-glow.md` §1): `Glow`
/// multiplies the unlit colour by 1 + As/128 in DISPLAY units, with As from
/// the texel's alpha and the vertex alpha (A = 64 → At = 32 → As = 63 at
/// Av = 255); `Unscaled` doubles it; both clamp at white and take no light.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn glow_and_unscaled_gain_in_display_units() {
    let mut r = headless();
    r.lighting = Lighting {
        sun_dir: Vec3::new(0.0, 0.0, -1.0),
        sun_color: [0.5, 0.5, 0.5],
        ..Lighting::default()
    };
    let tex = upload_tex(&mut r, 1, 1, &[255, 255, 255, 64], "glow");
    const V: u8 = 40;
    let quad = r
        .add_mesh(&MeshData {
            positions: &[
                [-4.0, -4.0, -3.0],
                [4.0, -4.0, -3.0],
                [4.0, 4.0, -3.0],
                [-4.0, 4.0, -3.0],
            ],
            normals: &[[0.0, 0.0, 1.0]; 4],
            uvs: &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            colors: Some(&[[V, V, V, 255]; 4]),
            indices: &[0, 1, 2, 0, 2, 3],
        })
        .expect("quad");
    let shot = |r: &mut render::Renderer, light: LightPath| {
        // A red instance tint: `Glow` drops it, `Unscaled` keeps it.
        let mut d = DrawCmd::textured(quad, tex, Mat4::IDENTITY).with_light(light);
        d.color = [1.0, 0.5, 0.5, 1.0];
        r.render(&Camera::default(), &[d], [0.0, 0.0, 0.0, 1.0])
            .expect("frame");
        capture_pixel(r, N / 2, N / 2)
    };
    let v = f32::from(V) / 255.0;
    let disp = f32::from(srgb(v)) / 255.0;
    let lin = |d: f32| {
        let d = d.min(1.0);
        if d <= 0.040_45 {
            d / 12.92
        } else {
            ((d + 0.055) / 1.055).powf(2.4)
        }
    };
    let g = lin(disp * (1.0 + 63.0 / 128.0));
    close(shot(&mut r, LightPath::Glow), [g; 3], "glow");
    let u = |t: f32| lin(f32::from(srgb(v * t)) / 255.0 * 2.0);
    close(
        shot(&mut r, LightPath::Unscaled),
        [u(1.0), u(0.5), u(0.5)],
        "unscaled",
    );
    // A map's tune (`Lighting::glow_gain`) scales both gains in display units.
    r.lighting.glow_gain = 0.8;
    let g = lin(disp * (1.0 + 63.0 / 128.0) * 0.8);
    close(shot(&mut r, LightPath::Glow), [g; 3], "glow x0.8");
    let u = |t: f32| lin(f32::from(srgb(v * t)) / 255.0 * 2.0 * 0.8);
    close(
        shot(&mut r, LightPath::Unscaled),
        [u(1.0), u(0.5), u(0.5)],
        "unscaled x0.8",
    );
}

fn gouraud_fixture(camera: &Camera) -> ([[f32; 3]; 3], [[f32; 3]; 3], [[u8; 4]; 3], Vec<Mat4>) {
    let tan = (camera.fov_y * 0.5).tan();
    let positions = [[-0.8, -0.8, 2.0], [0.8, -0.8, 5.0], [0.0, 0.8, 9.0]]
        .map(|[x, y, depth]| [x * depth * tan, y * depth * tan, -depth]);
    let rotations = [2.2142974, -0.6435011, 0.9272952].map(Mat4::from_rotation_y);
    let normals = rotations.map(|rotation| rotation.transform_vector3(Vec3::Z).to_array());
    let palette = positions
        .iter()
        .zip(rotations)
        .map(|(position, rotation)| {
            let p = Vec3::from(*position);
            Mat4::from_translation(p) * rotation * Mat4::from_translation(-p)
        })
        .collect();
    (
        positions,
        normals,
        [[64, 64, 64, 255], [128, 128, 128, 255], [255; 4]],
        palette,
    )
}

fn gouraud_lighting() -> Lighting {
    Lighting {
        sun_dir: Vec3::NEG_Z,
        sun_color: [render::srgb_decode(0.8); 3],
        ambient_sky: [render::srgb_decode(0.1); 3],
        ambient_ground: [render::srgb_decode(0.1); 3],
        ..Lighting::default()
    }
}

fn screen_weights(x: u32, y: u32) -> [f32; 3] {
    let px = (x as f32 + 0.5) * 2.0 / N as f32 - 1.0;
    let py = 1.0 - (y as f32 + 0.5) * 2.0 / N as f32;
    let top = (py + 0.8) / 1.6;
    let right = (px + 0.8 - 0.8 * top) / 1.6;
    [1.0 - top - right, right, top]
}

#[test]
#[ignore = "requires a GPU adapter"]
fn directional_material_lighting_is_gouraud_after_posing_and_in_display_units() {
    use render::{DrawLayer, SkinnedDrawCmd, SkinnedMeshData};
    let mut r = headless();
    let camera = Camera::default();
    r.lighting = gouraud_lighting();
    let (positions, normals, colors, palette) = gouraud_fixture(&camera);
    let uvs = [[0.0; 2]; 3];
    let indices = [0, 1, 2];
    let texture = upload_tex(&mut r, 1, 1, &[255; 4], "gouraud white");
    let rigid = r
        .add_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            uvs: &uvs,
            colors: Some(&colors),
            indices: &indices,
        })
        .unwrap();
    let skin = r
        .add_skinned_mesh(&SkinnedMeshData {
            positions: &positions,
            normals: &[[0.0, 0.0, 1.0]; 3],
            uvs: &uvs,
            colors: Some(&colors),
            indices: &indices,
            joints: &[[0, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0]],
            weights: &[[1.0, 0.0, 0.0, 0.0]; 3],
        })
        .unwrap();
    for path in [LightPath::Lit, LightPath::Prelit, LightPath::Character] {
        for skinned in [false, true] {
            let rigid_draws = if skinned {
                vec![]
            } else {
                vec![DrawCmd::textured(rigid, texture, Mat4::IDENTITY).with_light(path)]
            };
            let skin_draws = if skinned {
                vec![SkinnedDrawCmd {
                    mesh: skin,
                    texture: Some(texture),
                    color: [1.0; 4],
                    model: Mat4::IDENTITY,
                    palette: palette.clone().into(),
                    layer: DrawLayer::Opaque,
                    light: path,
                    lit: None,
                }]
            } else {
                vec![]
            };
            r.render_frame(&camera, &rigid_draws, &skin_draws, [0.0; 4], None)
                .unwrap();
            let (rgba, width, _) = r.capture_rgba().unwrap();
            for (x, y) in [(24, 40), (32, 32), (38, 42)] {
                let weights = screen_weights(x, y);
                let expected: f32 = (0..3)
                    .map(|i| {
                        let ambient = if path == LightPath::Prelit { 0.0 } else { 0.1 };
                        let lit = light_sum(
                            colors[i][0] as f32 / 255.0,
                            &[
                                (render::srgb_decode(0.8), normals[i][2].max(0.0)),
                                (render::srgb_decode(ambient), 1.0),
                            ],
                            0.0,
                        );
                        weights[i] * render::srgb_encode(lit)
                    })
                    .sum();
                let pixel = common::pixel(&rgba, width, x, y);
                let expected = (expected * 255.0).round() as u8;
                assert!(
                    pixel[0].abs_diff(expected) <= 2,
                    "{path:?} skinned={skinned}, ({x},{y}) got {pixel:?}, expected {expected}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn specular_uses_the_same_gouraud_base_for_rigid_and_region_lit_skinned_draws() {
    use render::{
        CharacterLightRegion, CharacterLightVolume, DrawLayer, LitSphere, SkinnedDrawCmd,
        SkinnedMeshData,
    };
    let mut r = headless();
    let camera = Camera::default();
    r.lighting = gouraud_lighting();
    let (positions, normals, colors, palette) = gouraud_fixture(&camera);
    let uvs = [[0.0; 2]; 3];
    let indices = [0, 1, 2];
    let texture = upload_tex(&mut r, 1, 1, &[100, 100, 100, 255], "gouraud spec base");
    let rigid = r
        .add_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            uvs: &uvs,
            colors: Some(&colors),
            indices: &indices,
        })
        .unwrap();
    let skin = r
        .add_skinned_mesh(&SkinnedMeshData {
            positions: &positions,
            normals: &[[0.0, 0.0, 1.0]; 3],
            uvs: &uvs,
            colors: Some(&colors),
            indices: &indices,
            joints: &[[0, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0]],
            weights: &[[1.0, 0.0, 0.0, 0.0]; 3],
        })
        .unwrap();
    r.set_character_light_regions(vec![CharacterLightRegion {
        centre: Vec3::new(0.0, 0.0, -5.0),
        axes: glam::Mat3::IDENTITY,
        volume: CharacterLightVolume::Sphere(100.0),
        top: [0.1; 3],
        bottom: [0.2; 3],
        directional: [0.4, 0.6],
    }]);
    r.set_specular_table(Some((1, 1, &[255; 4]))).unwrap();
    let add = [0.05, 0.10, 0.15];
    for skinned in [false, true] {
        let rigid_base = DrawCmd::textured(rigid, texture, Mat4::IDENTITY);
        let skin_base = SkinnedDrawCmd {
            mesh: skin,
            texture: Some(texture),
            color: [1.0; 4],
            model: Mat4::IDENTITY,
            palette: palette.clone().into(),
            layer: DrawLayer::Opaque,
            light: LightPath::Character,
            lit: Some(LitSphere {
                centre: Vec3::new(0.0, 0.0, -5.0),
                radius: 1.0,
            }),
        };
        let mut captures = Vec::new();
        for specular in [false, true] {
            let mut rigid_draws = Vec::new();
            let mut skin_draws = Vec::new();
            if skinned {
                skin_draws.push(skin_base.clone());
                if specular {
                    skin_draws.push(skin_base.specular(add, 50));
                }
            } else {
                rigid_draws.push(rigid_base);
                if specular {
                    rigid_draws.push(DrawCmd::specular(rigid, texture, add, 50, Mat4::IDENTITY));
                }
            }
            r.render_frame(&camera, &rigid_draws, &skin_draws, [0.0; 4], None)
                .unwrap();
            captures.push(r.capture_rgba().unwrap().0);
        }
        for (x, y) in [(24, 40), (32, 32), (38, 42)] {
            let before = common::pixel(&captures[0], N, x, y);
            let after = common::pixel(&captures[1], N, x, y);
            for channel in 0..3 {
                let expected = (f32::from(before[channel]) + add[channel] * 255.0).round() as u8;
                assert!(after[channel].abs_diff(expected) <= 2,
                    "skinned={skinned}, ({x},{y}) before={before:?}, after={after:?}, expected {expected}");
            }
        }
    }
}
