use super::*;

#[test]
fn visor_keeps_its_edges_inside_both_canted_eye_frusta() {
    let eyes = [-1.0, 1.0].map(|side| VisorEye {
        offset: Vec3::X * side * 0.032,
        orientation: Quat::from_rotation_y(-side * 0.12),
        fov: ViewFov {
            left: -1.1,
            right: 1.1,
            down: -1.3,
            up: 1.3,
        },
    });
    let visor = Visor::new(eyes);
    assert!((visor.half.x / visor.half.y - 16.0 / 9.0).abs() < 1e-6);
    assert!(visor.half.x > 0.55, "the HUD reaches the sides of the view");
    for x in [-1.0, 0.0, 1.0] {
        for y in [-1.0, 0.0, 1.0] {
            let p = shell_point(visor.half * Vec2::new(x, y));
            assert!(eyes.iter().all(|eye| eye.sees(p)), "point {x}, {y}");
        }
    }
}
