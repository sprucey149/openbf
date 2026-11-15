use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, LightPath, MeshData, Renderer, ShadowMesh};

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
        r.set_shadow_triangles(&[]);
        r.render(&camera, &[draw], [0.; 4]).unwrap();
        let (base, _, _) = r.capture_rgba().unwrap();
        r.set_shadow_triangles(&triangles);
        r.render(&camera, &[draw], [0.; 4]).unwrap();
        let (shadow, _, _) = r.capture_rgba().unwrap();
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
        r.set_shadow_triangles(&twice);
        r.render(&camera, &[draw], [0.; 4]).unwrap();
        assert_eq!(r.capture_rgba().unwrap().0, shadow);
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
        r.set_shadow_triangles(&[]);
        r.render(&camera, &draws, [0.; 4]).unwrap();
        let (base, _, _) = r.capture_rgba().unwrap();
        r.set_shadow_triangles(&triangles);
        r.render(&camera, &draws, [0.; 4]).unwrap();
        let (shadow, _, _) = r.capture_rgba().unwrap();
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
