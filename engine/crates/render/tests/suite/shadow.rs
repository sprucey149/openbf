use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, LightPath, MeshData, Renderer, ShadowMesh};

#[test]
#[ignore = "requires a GPU adapter"]
fn sun_shadows_project_rigid_and_posed_meshes() {
    use render::{DrawLayer, SkinnedDrawCmd, SkinnedMeshData};

    let size = 256;
    let mut r = Renderer::new_headless(size, size).unwrap();
    let floor = [
        [-12., 0., -12.],
        [-12., 0., 12.],
        [12., 0., 12.],
        [12., 0., -12.],
    ];
    let panel = [
        [-0.7, 1., -0.7],
        [-0.7, 1., 0.7],
        [0.7, 1., 0.7],
        [0.7, 1., -0.7],
    ];
    let indices = [0, 1, 2, 0, 2, 3];
    let upload = |r: &mut Renderer, positions| {
        r.add_mesh(&MeshData {
            positions,
            normals: &[[0., 1., 0.]; 4],
            uvs: &[[0.; 2]; 4],
            colors: None,
            indices: &indices,
        })
        .unwrap()
    };
    let ground = upload(&mut r, &floor);
    let rigid = upload(&mut r, &panel);
    let skinned = r
        .add_skinned_mesh(&SkinnedMeshData {
            positions: &panel,
            normals: &[[0., 1., 0.]; 4],
            uvs: &[[0.; 2]; 4],
            colors: None,
            indices: &indices,
            joints: &[[0; 4]; 4],
            weights: &[[1., 0., 0., 0.]; 4],
        })
        .unwrap();
    r.lighting.sun_dir = Vec3::new(0.45, -1., 0.25);
    let eye = Vec3::new(0., 6., 7.);
    let camera = Camera {
        position: eye,
        pitch: -0.68,
        far: 40.,
        ..Default::default()
    };
    let world = [
        DrawCmd::flat(ground, [0.65, 0.65, 0.65, 1.], Mat4::IDENTITY).with_light(LightPath::Unlit),
        DrawCmd::flat(
            rigid,
            [0.9, 0.2, 0.2, 1.],
            Mat4::from_translation(Vec3::new(-2.5, 0., 0.)),
        )
        .with_light(LightPath::Unlit),
    ];
    let pose = |x| SkinnedDrawCmd {
        mesh: skinned,
        texture: None,
        color: [0.2, 0.9, 0.2, 1.],
        model: Mat4::IDENTITY,
        palette: [Mat4::from_translation(Vec3::new(x, 0., 0.))].into(),
        layer: DrawLayer::Opaque,
        lit: None,
        light: LightPath::Unlit,
    };
    let render = |r: &mut Renderer, world: &[DrawCmd], shadows, posed| {
        r.set_sun_shadows(shadows);
        r.render_frame(&camera, world, &[pose(posed)], [0.; 4], None)
            .unwrap();
        r.capture_rgba().unwrap().0
    };
    let mut centres = Vec::new();
    for (scale, x) in [(1.0, 0.25), (0.75, 2.75)] {
        r.set_world_scale(scale);
        let base = render(&mut r, &world, false, x);
        let shaded = render(&mut r, &world, true, x);
        let mut left = 0;
        let mut right = Vec::new();
        let mut interior = 0;
        for (i, (b, s)) in base.chunks_exact(4).zip(shaded.chunks_exact(4)).enumerate() {
            if b[0] > 150 && b[0] == b[1] && b[1] == b[2] && s[0] + 20 < b[0] {
                if i as u32 % size < size / 2 {
                    left += 1;
                } else {
                    right.push(i as u32 % size);
                    if scale == 1.0 {
                        assert!(
                            (s[0] as f32 - 0.68 * b[0] as f32).abs() <= 1.5,
                            "the posed shadow should have a screen-pixel edge"
                        );
                    }
                }
                interior += usize::from((s[0] as f32 - 0.68 * b[0] as f32).abs() <= 1.5);
            }
        }
        if scale == 1.0 {
            assert!(
                interior * 100 > (left + right.len()) * 90,
                "the silhouette interior must keep a flat shade: {interior}"
            );
            assert!(
                interior < left + right.len(),
                "edges should have partial coverage"
            );
        }
        assert!(
            left > 100 && right.len() > 100,
            "{scale}: {left} / {}",
            right.len()
        );
        let mean_x = right.iter().sum::<u32>() / right.len() as u32;
        centres.push(mean_x);
        println!(
            "{scale}: {left} rigid and {} posed shadow pixels, posed centre {mean_x}",
            right.len()
        );
    }
    assert!(
        centres[1] > centres[0] + 25,
        "posed shadow centres: {centres:?}"
    );

    let mut masked = world;
    masked[1].layer = DrawLayer::Mask;
    assert_eq!(
        render(&mut r, &world, true, 2.75),
        render(&mut r, &masked, true, 2.75),
        "an opaque mask casts the same outline as the panel"
    );
    masked[1].color[3] = 0.0;
    assert_eq!(
        render(&mut r, &world[..1], true, 2.75),
        render(&mut r, &masked, true, 2.75),
        "a transparent mask leaves neither geometry nor shadow"
    );

    // The overhang blocks the light falling on the posed panel. The panel
    // should keep its character lighting while still casting onto the floor.
    r.set_world_scale(1.0);
    let overhang = DrawCmd::flat(
        rigid,
        [0.9, 0.2, 0.2, 1.],
        Mat4::from_translation(Vec3::new(2.30, 1., -0.25)),
    )
    .with_light(LightPath::Unlit);
    let cloth_mesh = r
        .add_character_mesh(
            &MeshData {
                positions: &panel,
                normals: &[[0., 1., 0.]; 4],
                uvs: &[[0.; 2]; 4],
                colors: None,
                indices: &indices,
            },
            true,
        )
        .unwrap();
    let cloth = DrawCmd::flat(
        cloth_mesh,
        [0.1, 0.2, 1., 1.],
        Mat4::from_translation(Vec3::new(-2.25, 0., 0.)),
    )
    .with_light(LightPath::Character);
    let left_overhang = DrawCmd {
        model: Mat4::from_translation(Vec3::new(-2.70, 1., -0.25)),
        ..overhang
    };
    let world = [world[0], overhang, left_overhang, cloth];
    let base = render(&mut r, &world, false, 2.75);
    let shaded = render(&mut r, &world, true, 2.75);
    let overlap = base
        .chunks_exact(4)
        .zip(shaded.chunks_exact(4))
        .filter(|(b, s)| b[0] > 150 && b[0] == b[1] && b[1] == b[2] && s[0] + 20 < b[0])
        .map(|(b, s)| {
            assert!(
                s[0] as f32 >= b[0] as f32 * 0.68 - 1.5,
                "a world shadow and a character volume should darken just once"
            );
        })
        .count();
    assert!(overlap > 200, "both overhangs should cast onto the scenery");
    let visible_body = base
        .chunks_exact(4)
        .zip(shaded.chunks_exact(4))
        .filter(|(b, _)| b[1] > 200 && b[0] < 150)
        .map(|(b, s)| {
            assert_eq!(b, s, "the projection should leave the posed panel alone");
        })
        .count();
    assert!(visible_body > 100, "panel should be visible");
    let visible_cloth = base
        .chunks_exact(4)
        .zip(shaded.chunks_exact(4))
        .filter(|(b, _)| i16::from(b[2]) - i16::from(b[0]) > 25)
        .map(|(b, s)| {
            assert_eq!(
                b, s,
                "the projection should leave rigid character parts alone"
            );
        })
        .count();
    assert!(visible_cloth > 100, "cloth should be visible");
}

fn capture_shadowed(
    r: &mut Renderer,
    camera: &Camera,
    draws: &[DrawCmd],
    triangles: &[[f32; 3]],
) -> Vec<u8> {
    r.set_shadow_triangles(triangles);
    r.render(camera, draws, [0.; 4]).unwrap();
    r.capture_rgba().unwrap().0
}

fn cube() -> ShadowMesh {
    ShadowMesh::new(
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
    .unwrap()
}

#[test]
#[ignore = "requires a GPU adapter"]
fn capped_box_shadows_match_projected_plane_inside_and_outside() {
    let n = 256;
    let mut r = Renderer::new_headless(n, n).unwrap();
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
    let draw =
        DrawCmd::flat(plane, [0.65, 0.65, 0.65, 1.], Mat4::IDENTITY).with_light(LightPath::Unlit);
    let mut triangles = Vec::new();
    cube().append_volume(Mat4::IDENTITY, Vec3::new(125., -500., 0.), &mut triangles);
    for (name, camera) in [
        (
            "outside",
            Camera {
                position: Vec3::new(5., 6., 8.),
                yaw: 5.0_f32.atan2(8.),
                pitch: -0.57,
                far: 40.,
                ..Default::default()
            },
        ),
        (
            "inside",
            Camera {
                position: Vec3::new(0., 0.5, 0.),
                pitch: -0.5,
                far: 40.,
                ..Default::default()
            },
        ),
    ] {
        let base = capture_shadowed(&mut r, &camera, &[draw], &[]);
        let shadow = capture_shadowed(&mut r, &camera, &[draw], &triangles);
        let inv = camera.view_proj(1.).inverse();
        let mut expected = 0;
        let mut actual = 0;
        let mut wrong = 0;
        for y in 0..n {
            for x in 0..n {
                let ndc = Vec3::new(
                    2. * (x as f32 + 0.5) / n as f32 - 1.,
                    1. - 2. * (y as f32 + 0.5) / n as f32,
                    0.5,
                );
                let ray = (inv.project_point3(ndc) - camera.position).normalize();
                let t = -camera.position.y / ray.y;
                let p = camera.position + ray * t;
                let inside = t > 0. && (-0.75..1.75).contains(&p.x) && (-1.0..1.0).contains(&p.z);
                let i = ((y * n + x) * 4) as usize;
                let dark = shadow[i] + 10 < base[i];
                expected += usize::from(inside);
                actual += usize::from(dark);
                wrong += usize::from(inside != dark);
                if dark {
                    assert!(
                        (shadow[i] as f32 - base[i] as f32 * 0.625).abs() <= 1.5,
                        "{name}: display multiply {} -> {}",
                        base[i],
                        shadow[i]
                    );
                }
            }
        }
        println!("{name}: expected {expected}, actual {actual}, mismatches {wrong}");
        assert!(expected > 100, "{name}: test must exercise ground shadow");
        assert!(
            wrong as f32 / (expected as f32) < 0.02,
            "{name}: expected {expected}, actual {actual}, mismatches {wrong}"
        );
        // Overlapping volumes shade once; no stale frame after clearing.
        let mut twice = triangles.clone();
        twice.extend_from_slice(&triangles);
        assert_eq!(capture_shadowed(&mut r, &camera, &[draw], &twice), shadow);
        // Submission is per frame: entering a shell/preview leaves no old shadow.
        r.render(&camera, &[draw], [0.; 4]).unwrap();
        assert_eq!(r.capture_rgba().unwrap().0, base);
    }
}

/// Playtest 2026-11-14, BF1 Rhen Var Harbor: the AAT's side guns flickered
/// dark as the camera bobbed. The volume's near cap lay exactly on the
/// model's own faces, so which of the two won the depth test changed every
/// frame. The PS2 caps with the faces turned away from the sun and sends the
/// volume on a 16-bit grid (`ShadowMesh::append_volume`); here the box is
/// drawn exactly where its shadow mesh is, as a vehicle's is, and seen from
/// eyes a few centimetres apart.
#[test]
#[ignore = "requires a GPU adapter"]
fn a_box_does_not_flicker_under_its_own_shadow() {
    let n = 256;
    let mut r = Renderer::new_headless(n, n).unwrap();
    let quad = |r: &mut Renderer, positions: &[[f32; 3]; 4], normal: [f32; 3]| {
        r.add_mesh(&MeshData {
            positions,
            normals: &[normal; 4],
            uvs: &[[0.; 2]; 4],
            colors: None,
            indices: &[0, 1, 2, 0, 2, 3],
        })
        .unwrap()
    };
    let top = quad(
        &mut r,
        &[[-1., 3., -1.], [-1., 3., 1.], [1., 3., 1.], [1., 3., -1.]],
        [0., 1., 0.],
    );
    let side = quad(
        &mut r,
        &[[1., 1., -1.], [1., 3., -1.], [1., 3., 1.], [1., 1., 1.]],
        [1., 0., 0.],
    );
    let draws = [
        DrawCmd::flat(top, [0.9, 0.2, 0.2, 1.], Mat4::IDENTITY).with_light(LightPath::Unlit),
        DrawCmd::flat(side, [0.2, 0.9, 0.2, 1.], Mat4::IDENTITY).with_light(LightPath::Unlit),
    ];
    let mut triangles = Vec::new();
    cube().append_volume(Mat4::IDENTITY, Vec3::new(125., -500., 0.), &mut triangles);
    let mut side_shaded = Vec::new();
    for k in 0..8 {
        let nudge = 0.013 * k as f32;
        let position = Vec3::new(6. + nudge, 7. - nudge, 5. + 0.5 * nudge);
        let camera = Camera {
            position,
            yaw: position.x.atan2(position.z),
            pitch: -0.55,
            far: 40.,
            ..Default::default()
        };
        let base = capture_shadowed(&mut r, &camera, &draws, &[]);
        let shadow = capture_shadowed(&mut r, &camera, &draws, &triangles);
        let (mut tops, mut top_dark, mut sides, mut side_dark) = (0, 0, 0, 0);
        for (b, s) in base.chunks(4).zip(shadow.chunks(4)) {
            let dark = s[0] + 10 < b[0] || s[1] + 10 < b[1];
            if b[0] > 200 && b[1] < 150 {
                tops += 1;
                top_dark += usize::from(dark);
            } else if b[1] > 200 && b[0] < 150 {
                sides += 1;
                side_dark += usize::from(dark);
            }
        }
        assert!(
            tops > 500 && sides > 500,
            "eye {k}: {tops} top, {sides} side pixels"
        );
        assert_eq!(
            top_dark, 0,
            "eye {k}: the sunlit top darkened in {top_dark} of {tops} pixels"
        );
        let shaded = side_dark as f32 / sides as f32;
        assert!(
            !(0.05..=0.95).contains(&shaded),
            "eye {k}: the side away from the sun is speckled, {side_dark} of {sides} pixels dark"
        );
        side_shaded.push(shaded > 0.5);
    }
    assert!(
        side_shaded.windows(2).all(|w| w[0] == w[1]),
        "the side away from the sun flips between eyes: {side_shaded:?}"
    );
}
