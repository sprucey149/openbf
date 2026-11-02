//! The chase camera at BF2 Death Star's doors on the real bake (data-gated:
//! skipped when the bake is absent). The report: "even walking through doors
//! puts the camera in the trooper". `examples/camera_doorway.rs` walks a
//! soldier through them tick by tick; here the track itself is pinned.

use game::camera_collide::CameraBlockers;
use glam::Vec3;

/// Boom from the 1.5 m pull point to BF2's standing camera: 3.2 m back, up.
const BACK: f32 = 3.08;
const UP: f32 = 1.17;

#[test]
fn death_star_blast_doors_pull_the_camera_only_while_shut() {
    let dir = game::assets::baked_root().join("bf2_dea1");
    if !dir.join("manifest.ron").exists() {
        return;
    }
    let (scene, _) = game::assets::load_battle_scene(&dir, "con").expect("map");
    let blockers = CameraBlockers::from_scene(&scene);
    let (_, mut doors) = scene.collision_and_doors();
    let at = doors
        .iter()
        .position(|d| d.origin.distance(Vec3::new(-84.0, 50.0, -108.0)) < 1.0)
        .expect("the blast door");
    // Just past the door going -x, in the doorway: the camera is back toward
    // +x, across the door's plane. The rig must not leave the leaf behind.
    let pull = Vec3::new(-85.0, 51.94, -108.0);
    let camera = pull + Vec3::new(BACK, UP, 0.0);
    let clear = |doors: &[core::door::Door]| blockers.fraction(pull, camera, None, |_| None, doors);
    doors[at].open = 1.0;
    assert_eq!(clear(&doors), 1.0, "an open doorway holds no camera back");
    doors[at].open = 0.0;
    assert!(
        clear(&doors) < 0.5,
        "a shut door still does: {}",
        clear(&doors)
    );
    // No sim door (`OPENBF_DOORS_SOLID`): drawn shut, tested shut.
    assert!(clear(&[]) < 0.5);
    // A wall still pulls with every door open: straight back through the
    // room's wall from the hall behind this door.
    doors.iter_mut().for_each(|d| d.open = 1.0);
    let wall_pull = Vec3::new(-84.0, 51.94, -96.0);
    let far = wall_pull + Vec3::new(0.0, UP, 40.0);
    let f = blockers.fraction(wall_pull, far, None, |_| None, &doors);
    assert!(f < 1.0, "the Death Star has walls: {f}");
}
