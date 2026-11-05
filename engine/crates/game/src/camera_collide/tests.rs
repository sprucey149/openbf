use super::*;

fn mesh(positions: Vec<Vec3>, indices: Vec<u32>) -> Arc<TriMesh> {
    let mut local = Aabb::empty();
    for p in &positions {
        local.add(*p);
    }
    let chunks = triangle_chunks(&positions, &indices);
    Arc::new(TriMesh {
        positions,
        indices,
        local,
        chunks,
    })
}

fn one(mesh: Arc<TriMesh>, m: Mat4, moves: bool) -> CameraBlockers {
    CameraBlockers {
        blockers: vec![Blocker::new(mesh, m, 7, moves)],
    }
}

/// A 10x10 m wall slab in the z = [-2.5, -2.0] plane, at identity.
fn slab() -> Arc<TriMesh> {
    let quads: [[Vec3; 4]; 2] = [
        [
            Vec3::new(-5.0, -5.0, -2.0),
            Vec3::new(5.0, -5.0, -2.0),
            Vec3::new(5.0, 5.0, -2.0),
            Vec3::new(-5.0, 5.0, -2.0),
        ],
        [
            Vec3::new(-5.0, -5.0, -2.5),
            Vec3::new(5.0, -5.0, -2.5),
            Vec3::new(5.0, 5.0, -2.5),
            Vec3::new(-5.0, 5.0, -2.5),
        ],
    ];
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for q in quads {
        let b = positions.len() as u32;
        positions.extend_from_slice(&q);
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    mesh(positions, indices)
}

fn wall() -> CameraBlockers {
    one(slab(), Mat4::IDENTITY, false)
}

/// A 0.1 m square post from y -5 to 5 at (x, z), four sides.
fn post(x: f32, z: f32) -> Arc<TriMesh> {
    let h = 0.05;
    let c = [(-h, -h), (h, -h), (h, h), (-h, h)];
    let mut positions = Vec::new();
    for (dx, dz) in c {
        positions.push(Vec3::new(x + dx, -5.0, z + dz));
        positions.push(Vec3::new(x + dx, 5.0, z + dz));
    }
    let mut indices = Vec::new();
    for i in 0..4u32 {
        let (a, b) = (2 * i, 2 * ((i + 1) % 4));
        indices.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
    }
    mesh(positions, indices)
}

/// Both games' snowspeeder pilots author `TrackOffset` 0 0 0 (`TrackCenter`
/// 0 4 -6 BF2, 0 3 -8 BF1). The track does not start at the centre, so that
/// has a length to shorten: it runs from the model's pull point, and a bay's
/// wall behind the craft brings the camera in.
#[test]
fn a_pilot_seat_with_no_track_offset_still_pulls_in() {
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    let mut w = crate::walk::WalkSim::new(1, Vec3::ZERO, None);
    let h = w.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let mut seat = crate::vehicle_class::SeatSpec::new(SeatKind::Pilot);
    seat.track_center = Some([0.0, 4.0, -6.0]);
    seat.track_offset = Some([0.0, 0.0, 0.0]);
    w.vehicle_seats = vec![vec![seat]];
    w.vehicle_tops = vec![Some(2.0)];
    let player = w.player;
    assert!(w.sim.seat_unit(player, h, 0));
    let open = w.camera().position;
    assert!((open - Vec3::new(0.0, 4.0, -6.0)).length() < 1e-3, "{open}");
    w.camera_blockers = wall();
    let pulled = w.camera().position;
    assert!(
        pulled.z > -2.0 && pulled.z < 0.0 && pulled.y > 1.8 && pulled.y < 4.0,
        "the slab at z -2 stands behind the craft: {pulled}"
    );
}

#[test]
fn camera_stops_short_of_a_wall() {
    let got = wall().pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0));
    assert!(
        (got.z - (-2.0 + CAMERA_RADIUS)).abs() < 1e-3,
        "stopped at {got:?}"
    );
}

#[test]
fn clear_line_keeps_the_full_track_distance() {
    assert_eq!(
        wall().pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, 4.0)),
        Vec3::new(0.0, 0.0, 4.0)
    );
}

#[test]
fn a_miss_to_the_side_does_not_pull_in() {
    assert_eq!(
        wall().pull_in(Vec3::new(20.0, 0.0, 0.0), Vec3::new(20.0, 0.0, -4.0)),
        Vec3::new(20.0, 0.0, -4.0)
    );
}

/// The reason the triangle pass exists: a concave model's AABB covers
/// open ground, and a box-only test collapsed the camera there. With the
/// triangle pass, a segment through the gap keeps its full track.
#[test]
fn concave_model_does_not_block_through_its_opening() {
    let mut m = mesh(
        vec![
            Vec3::new(4.0, -5.0, -2.0),
            Vec3::new(5.0, -5.0, -2.0),
            Vec3::new(5.0, 5.0, -2.0),
        ],
        vec![0, 1, 2],
    );
    // Keep the slab's whole box: the AABB spans the origin's track.
    Arc::get_mut(&mut m).unwrap().local = Aabb {
        min: Vec3::new(-5.0, -5.0, -2.5),
        max: Vec3::new(5.0, 5.0, -2.0),
    };
    assert_eq!(
        one(m, Mat4::IDENTITY, false).pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0)),
        Vec3::new(0.0, 0.0, -4.0),
        "the AABB is hit but no triangle is"
    );
}

/// The droid bug: something right behind the player used to leave the
/// camera at a 25 % floor of its track, inside it. Now the camera comes
/// all the way in (the body is hidden there, `BODY_HIDE_DIST`).
#[test]
fn a_blocker_right_behind_the_target_takes_the_whole_track() {
    // The player 0.2 m in front of the slab's near face, the camera
    // wanted 4 m behind him through it.
    let got = wall().pull_in(Vec3::new(0.0, 0.0, -1.8), Vec3::new(0.0, 0.0, -5.8));
    assert_eq!(got, Vec3::new(0.0, 0.0, -1.8));
}

/// Touching a wall at the start blocks only a camera that moves into
/// it: a soldier with his back to a wall can swing the camera along it.
#[test]
fn a_start_against_a_wall_blocks_only_toward_it() {
    let start = Vec3::new(0.0, 0.0, -1.8);
    let along = Vec3::new(3.0, 0.0, -1.8);
    assert_eq!(wall().pull_in(start, along), along);
    let away = Vec3::new(0.0, 0.0, 2.0);
    assert_eq!(wall().pull_in(start, away), away);
}

/// A thin post 0.2 m beside the track: a ray along the centre misses it,
/// the swept sphere stops at its radius from it.
#[test]
fn a_post_beside_the_track_stops_the_sphere() {
    let b = Blocker::new(post(0.2, -3.0), Mat4::IDENTITY, 7, false);
    let dir = Vec3::NEG_Z;
    let t = b
        .sweep(b.inv, Vec3::ZERO, dir, 6.0, CAMERA_RADIUS)
        .expect("the sphere meets the post");
    let got = dir * t;
    assert!(got.z > -3.0, "{got:?}");
    let clear = post_clearance(&post(0.2, -3.0), got);
    assert!((clear - CAMERA_RADIUS).abs() < 1e-3, "{clear} {got:?}");
}

/// The playtest's ask (2026-10-28): a small prop between the camera and
/// the player never pulls the camera in; it is drawn see-through, and
/// hidden outright with the camera inside it. A prop beside the sight
/// line, or past the player, stays whole.
#[test]
fn a_small_prop_is_see_through_and_never_pulls() {
    let b = one(post(0.2, -3.0), Mat4::IDENTITY, false);
    assert_eq!(b.is_prop(7), Some(true));
    assert_eq!(wall().is_prop(7), Some(false));
    let (player, cam) = (Vec3::ZERO, Vec3::new(0.0, 0.0, -6.0));
    let none = |_| None;
    assert_eq!(
        b.fraction(player, cam, None, none, &[]),
        1.0,
        "a prop never pulls"
    );
    // The gallery's framing still stops at it.
    assert!(b.pull_in(player, cam).z > -3.0);
    assert_eq!(
        b.see_through(cam, &[player], None, none, &[]),
        vec![(7, SEE_THROUGH_OPACITY)]
    );
    // The camera inside the post's box: hidden.
    let inside = Vec3::new(0.2, 1.0, -3.0);
    assert_eq!(
        b.see_through(inside, &[player], None, none, &[]),
        vec![(7, 0.0)]
    );
    // Beside the sight line: whole.
    let aside = Vec3::new(3.0, 0.0, -6.0);
    assert!(b
        .see_through(aside, &[Vec3::new(3.0, 0.0, 0.0)], None, none, &[])
        .is_empty());
    // Past the player (0.2 m beyond his sight point): whole.
    let lean = one(post(0.0, -0.2), Mat4::IDENTITY, false);
    assert!(lean
        .see_through(Vec3::new(0.0, 0.0, 6.0), &[player], None, none, &[])
        .is_empty());
    // A skipped instance (a gunner's own building) is left alone.
    assert!(b.see_through(cam, &[player], Some(7), none, &[]).is_empty());
}

/// A millimetre-scale art change can toggle an entire small prop's
/// visibility. The alignment gate cannot borrow that prop's resting art
/// as permanent coverage for a hole in a different, nonfading building.
#[test]
fn a_small_geometry_shift_can_hide_only_one_sources_model() {
    let quad = |size| {
        mesh(
            vec![
                Vec3::ZERO,
                Vec3::new(size, 0., 0.),
                Vec3::new(size, 0., size),
                Vec3::new(0., 0., size),
            ],
            vec![0, 1, 2, 0, 2, 3],
        )
    };
    let reference = one(quad(2.), Mat4::IDENTITY, false);
    let source = one(
        quad(2.),
        Mat4::from_translation(Vec3::new(0., 0.005, 0.)),
        false,
    );
    let big = one(quad(6.), Mat4::IDENTITY, false);
    let eye = Vec3::new(1., CAMERA_RADIUS + 0.0025, 1.);
    let sight = Vec3::new(1., 2., 1.);
    assert!(reference
        .see_through(eye, &[sight], None, |_| None, &[])
        .is_empty());
    assert_eq!(
        source.see_through(eye, &[sight], None, |_| None, &[]),
        vec![(7, 0.)]
    );
    assert!(big
        .see_through(eye, &[sight], None, |_| None, &[])
        .is_empty());
}

fn post_clearance(m: &TriMesh, p: Vec3) -> f32 {
    m.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.positions[i as usize]);
            closest_on_triangle(p, a, b, c).distance(p)
        })
        .fold(f32::MAX, f32::min)
}

#[test]
fn rotated_instance_transforms_the_segment() {
    // Same slab, yawed 90 deg: it now blocks along -x instead of -z.
    let b = one(
        slab(),
        Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
        false,
    );
    let got = b.pull_in(Vec3::ZERO, Vec3::new(-4.0, 0.0, 0.0));
    assert!((got.x - (-2.0 + CAMERA_RADIUS)).abs() < 1e-3, "{got:?}");
    assert_eq!(
        b.pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0)),
        Vec3::new(0.0, 0.0, -4.0),
        "no longer blocks the old direction"
    );
}

/// A droid is cast where it is drawn: its live pose, not its placed one.
#[test]
fn a_turning_blocker_is_cast_at_its_live_pose() {
    let b = one(slab(), Mat4::IDENTITY, true);
    let turned = Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let live = |i: u32| (i == 7).then_some(turned);
    let to_x = Vec3::new(-4.0, 0.0, 0.0);
    let f = b.fraction(Vec3::ZERO, to_x, None, live, &[]);
    assert!((f * 4.0 - (2.0 - CAMERA_RADIUS)).abs() < 1e-3, "{f}");
    let to_z = Vec3::new(0.0, 0.0, -4.0);
    assert_eq!(b.fraction(Vec3::ZERO, to_z, None, live, &[]), 1.0);
    // No live pose: the placed one.
    assert!(b.fraction(Vec3::ZERO, to_z, None, |_| None, &[]) < 1.0);
}

#[test]
fn a_skipped_instance_does_not_block() {
    let to = Vec3::new(0.0, 0.0, -4.0);
    assert_eq!(wall().fraction(Vec3::ZERO, to, Some(7), |_| None, &[]), 1.0);
    assert!(wall().fraction(Vec3::ZERO, to, Some(8), |_| None, &[]) < 1.0);
}

/// A model scaled by its instance: the sphere keeps its world radius.
#[test]
fn a_scaled_instance_keeps_the_world_radius() {
    let b = one(slab(), Mat4::from_scale(Vec3::splat(2.0)), false);
    let got = b.pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -8.0));
    assert!((got.z - (-4.0 + CAMERA_RADIUS)).abs() < 1e-3, "{got:?}");
}

/// B1 `FUN_0019fd70`: in at once, out slowly and only past the step.
#[test]
fn the_pull_eases_out_as_the_ps2_does() {
    assert_eq!(ease_pull(1.0, 0.3, 1.0 / 30.0), 0.3);
    let dt = 1.0 / 30.0;
    assert_eq!(ease_pull(0.3, 0.4, dt), 0.3, "a rise under 0.15 waits");
    assert!((ease_pull(0.3, 0.5, dt) - (0.3 + 0.025)).abs() < 1e-6);
    assert!((ease_pull(0.3, 1.0, dt) - (0.3 + 0.025)).abs() < 1e-6);
    assert_eq!(ease_pull(0.99, 1.0, dt), 1.0, "never past the raw");
    let mut f = 0.0;
    let mut n = 0;
    while f < 1.0 {
        f = ease_pull(f, 1.0, dt);
        n += 1;
    }
    assert!(
        (40..=41).contains(&n),
        "4/3 s back from fully in: {n} ticks"
    );
}

#[test]
fn transformed_box_bounds_all_corners() {
    let local = Aabb {
        min: Vec3::new(-1.0, 0.0, -1.0),
        max: Vec3::new(1.0, 2.0, 1.0),
    };
    let w = Aabb::transformed(local, Mat4::from_rotation_y(std::f32::consts::FRAC_PI_4));
    assert!((w.max.x - 2f32.sqrt()).abs() < 1e-5, "{w:?}");
    assert!((w.max.y - 2.0).abs() < 1e-5, "{w:?}");
}

#[test]
fn chunked_sweeps_match_the_complete_triangle_scan_bit_for_bit() {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for x in -12..=12 {
        for z in -12..=12 {
            let at = positions.len() as u32;
            let (x, z) = (x as f32 * 2.0, z as f32 * 2.0);
            positions.extend([
                Vec3::new(x, 0.0, z),
                Vec3::new(x + 1.8, 0.0, z),
                Vec3::new(x, 0.2, z + 1.8),
            ]);
            indices.extend([at, at + 1, at + 2]);
        }
    }
    let mesh = mesh(positions, indices);
    assert_eq!(mesh.chunks.len(), 40); // includes the final short batch
    let mut hits = 0;
    let mut misses = 0;
    for pose in [
        Mat4::IDENTITY,
        Mat4::from_translation(Vec3::new(170.0, -3.0, -250.0))
            * Mat4::from_rotation_y(0.71)
            * Mat4::from_scale(Vec3::new(0.7, 1.3, 1.8)),
    ] {
        let b = Blocker::new(mesh.clone(), pose, 7, true);
        for x in -10..=10 {
            for y in [-0.35, 0.0, 0.15, 0.35, 1.0, 3.0] {
                let from = pose.transform_point3(Vec3::new(x as f32 * 2.0, y, -8.0));
                for delta in [
                    Vec3::new(0.0, -2.0, 0.0),
                    Vec3::new(0.35, -0.1, 8.0),
                    Vec3::new(-3.0, 0.0, 8.0),
                ] {
                    let end = pose.transform_point3(Vec3::new(x as f32 * 2.0, y, -8.0) + delta);
                    let len = end.distance(from);
                    let dir = (end - from) / len;
                    for radius in [SIGHT_RADIUS, CAMERA_RADIUS] {
                        let want = b.sweep_filtered::<false>(b.inv, from, dir, len, radius);
                        let got = b.sweep(b.inv, from, dir, len, radius);
                        assert_eq!(
                            got.map(f32::to_bits),
                            want.map(f32::to_bits),
                            "{from} {dir} {len} {radius}"
                        );
                        if got.is_some() {
                            hits += 1;
                        } else {
                            misses += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(hits > 100 && misses > 100, "{hits} hits / {misses} misses");
}

/// A door 10 m wide and 8 m tall, in the plane z = -2: submesh 0 is a leaf
/// that rises 8 m over 3 frames, submesh 1 a lintel above it that stays.
fn door_model(leaf_top: f32) -> (crate::assets::CpuMesh, manifest::ManifestDoor) {
    let q = |y0: f32, y1: f32| {
        [
            [-5.0, y0, -2.0],
            [5.0, y0, -2.0],
            [5.0, y1, -2.0],
            [-5.0, y1, -2.0],
        ]
    };
    let mut positions = q(-4.0, leaf_top).to_vec();
    positions.extend(q(leaf_top, leaf_top + 1.0));
    let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
    let cpu = crate::assets::CpuMesh {
        positions,
        normals: Vec::new(),
        uvs: Vec::new(),
        colors: Vec::new(),
        indices,
        submeshes: vec![
            (0, 6, "leaf".into(), render::DrawLayer::Opaque, 0),
            (6, 6, "lintel".into(), render::DrawLayer::Opaque, 0),
        ],
        spec: Vec::new(),
        light: Vec::new(),
        scroll: Vec::new(),
        sphere: None,
        vis_top: None,
        render_grid: None,
        contact_surface: false,
    };
    let rig = manifest::ManifestDoor {
        locked: false,
        triggers: Vec::new(),
        frames: 3,
        prims: Vec::new(),
        open_sound: None,
        close_sound: None,
        locked_sound: None,
        parts: vec![manifest::ManifestDoorPart {
            submesh: 0,
            bone: "leaf".into(),
            motion: vec![
                ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0]),
                ([0.0, 0.0, 0.0, 1.0], [0.0, 4.0, 0.0]),
                ([0.0, 0.0, 0.0, 1.0], [0.0, 8.0, 0.0]),
            ],
        }],
    };
    (cpu, rig)
}

fn door_blockers(place: Mat4) -> CameraBlockers {
    let (cpu, rig) = door_model(4.0);
    let positions = cpu.positions.iter().copied().map(Vec3::from).collect();
    let whole = mesh(positions, cpu.indices.clone());
    let blockers = split_door(&cpu, &whole, &rig)
        .into_iter()
        .map(|(part, frames)| match frames {
            Some(frames) => Blocker::door_part(part, place, 7, frames),
            None => Blocker::new(part, place, 7, false),
        })
        .collect();
    CameraBlockers { blockers }
}

#[test]
fn contact_surface_stays_visible_and_pulls_the_camera_clear() {
    let (mut cpu, _) = door_model(1.0);
    for p in &mut cpu.positions {
        p[0] *= 0.2;
    }
    let positions = cpu.positions.clone();
    let indices = cpu.indices.clone();
    let mut scene = crate::assets::Scene {
        dir: std::path::PathBuf::new(),
        manifest: ron::from_str(r#"(map:"camera",source_game:"bf1",models:["contact"],textures:[],
            classes:[(name:"contact",base:"prop",geometry:Some("contact"))],
            geometries:[(name:"contact",model:"contact",lods:[])],regions:[],layers:[],
            instances:[(class:"contact",name:"contact",rotation:((1.0,0.0,0.0),(0.0,1.0,0.0),(0.0,0.0,1.0)),translation:(0.0,0.0,0.0),props:[])])"#).unwrap(),
        meshes: HashMap::from([("contact".to_owned(), cpu)]),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: HashMap::new(),
        collision: HashMap::new(),
        cutters: Vec::new(),
    };
    let legacy = CameraBlockers::from_scene(&scene);
    let none = |_: u32| None;
    let target = Vec3::ZERO;
    let desired = Vec3::new(0.0, 0.0, -4.0);
    let eye = Vec3::new(0.0, 0.0, -2.01);
    assert_eq!(legacy.fraction(target, desired, None, none, &[]), 1.0);
    assert_eq!(
        legacy.see_through(eye, &[target], None, none, &[]),
        vec![(0, 0.0)]
    );
    scene.meshes.get_mut("contact").unwrap().contact_surface = true;
    let contact = CameraBlockers::from_scene(&scene);
    assert!(contact.fraction(target, desired, None, none, &[]) < 0.5);
    assert!(contact
        .see_through(eye, &[target], None, none, &[])
        .is_empty());
    assert_eq!(
        contact.pull_in(target, desired),
        legacy.pull_in(target, desired)
    );
    assert_eq!(scene.meshes["contact"].positions, positions);
    assert_eq!(scene.meshes["contact"].indices, indices);
}

fn open_at(open: f32) -> [Door; 1] {
    [Door {
        instance: 7,
        open,
        ..Door::default()
    }]
}

/// The report: "even walking through doors puts the camera in the
/// trooper". A shut door pulls the camera, an open one does not, one part
/// open only where its leaf has really gone, and its lintel always.
#[test]
fn a_door_pulls_the_camera_only_where_its_leaf_is_now() {
    let b = door_blockers(Mat4::IDENTITY);
    let (from, to) = (Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0));
    let none = |_: u32| None;
    let stopped = (2.0 - CAMERA_RADIUS) / 4.0;
    for open in [0.0, 0.25] {
        let f = b.fraction(from, to, None, none, &open_at(open));
        assert!((f - stopped).abs() < 1e-3, "{open}: {f}");
    }
    assert_eq!(b.fraction(from, to, None, none, &open_at(1.0)), 1.0);
    // A track at the lintel's height, leaf shut and well below it, stops
    // at the part the clip does not move.
    let up = Vec3::Y * 4.5;
    let f = b.fraction(from + up, to + up, None, none, &open_at(0.0));
    assert!((f - stopped).abs() < 1e-3, "{f}");
    // No sim twin (`OPENBF_DOORS_SOLID`): drawn shut, and tested shut.
    assert!(b.fraction(from, to, None, none, &[]) < 1.0);
    // Another instance's door moves nothing here.
    let other = [Door {
        instance: 8,
        open: 1.0,
        ..Door::default()
    }];
    assert!(b.fraction(from, to, None, none, &other) < 1.0);
    // The diagnostics agree with the rule.
    assert!(b.track_hits(from, to, &open_at(1.0)).is_empty());
    assert_eq!(b.track_hits(from, to, &open_at(0.0)).len(), 1);
}

/// The motion is the model's: the instance's turn carries it. A leaf that
/// has moved into the track's way blocks it (the broad phase covers the
/// whole clip, not just the shut pose).
#[test]
fn a_door_leaf_blocks_where_the_clip_has_put_it() {
    let turn = Mat4::from_translation(Vec3::new(30.0, 0.0, 0.0)) * Mat4::from_rotation_y(1.0);
    let b = door_blockers(turn);
    let none = |_: u32| None;
    let (from, to) = (
        turn.transform_point3(Vec3::new(0.0, 9.0, 0.0)),
        turn.transform_point3(Vec3::new(0.0, 9.0, -4.0)),
    );
    // At 9 m: over the lintel (4..5 m) and the shut leaf (-4..4 m).
    assert_eq!(b.fraction(from, to, None, none, &open_at(0.0)), 1.0);
    // The leaf rose 8 m to 4..12 m: it is there now.
    let f = b.fraction(from, to, None, none, &open_at(1.0));
    assert!((f * 4.0 - (2.0 - CAMERA_RADIUS)).abs() < 1e-3, "{f}");
}

/// A small door is a prop as a whole (parts never decide), and it is
/// listed once however many of its parts are in the way.
#[test]
fn a_door_prop_is_listed_once_at_its_lowest_opacity() {
    let (cpu, rig) = door_model(0.5);
    let positions: Vec<Vec3> = cpu
        .positions
        .iter()
        .map(|p| Vec3::from(*p) * Vec3::new(0.2, 1.0, 1.0))
        .collect();
    let whole = mesh(positions, cpu.indices.clone());
    let blockers: Vec<Blocker> = split_door(&cpu, &whole, &rig)
        .into_iter()
        .map(|(part, frames)| match frames {
            Some(frames) => Blocker::door_part(part, Mat4::IDENTITY, 7, frames),
            None => Blocker::new(part, Mat4::IDENTITY, 7, false),
        })
        .collect();
    assert!(blockers.iter().all(|b| b.prop));
    let b = CameraBlockers { blockers };
    let shut = open_at(0.0);
    let got = b.see_through(
        Vec3::new(0.0, 0.25, -4.0),
        &[Vec3::new(0.0, 0.25, 0.0), Vec3::new(0.0, 1.0, 0.0)],
        None,
        |_| None,
        &shut,
    );
    assert_eq!(got, vec![(7, SEE_THROUGH_OPACITY)]);
}
