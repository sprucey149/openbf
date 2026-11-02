use super::SWEEP_CONTACT_DIST;
use crate::collision::shape::{Body, Capsule, Layers, Shape};
use crate::collision::tests::{floor_quad, mesh_body, mesh_body_at, ramp, world_from_triangles};
use crate::collision::world::CollisionWorld;
use crate::collision::{vector, WALKABLE_MIN_NY};
use glam::Vec3;
use glam::{Mat4, Quat};

/// Contact collection appends in body order, with the exact per-body
/// answers preserved across the broadphase and both mover entry points.
#[test]
fn contact_collection_preserves_per_body_bits_and_existing_output() {
    let pose =
        Mat4::from_rotation_translation(Quat::from_rotation_y(0.37), Vec3::new(4.0, 0.0, -3.0));
    let mut tris = wall_quad(0.0);
    tris.extend(wall_quad(-6.0));
    let centre = pose.transform_point3(Vec3::new(0.2, 0.0, 0.0));
    let bodies = vec![
        Body::new(
            Mat4::from_translation(centre + Vec3::new(0.3, 0.0, 0.0)),
            Shape::Sphere { radius: 0.5 },
        ),
        mesh_body_at(&tris, pose),
        Body::new(
            Mat4::from_translation(centre + Vec3::new(0.0, 0.0, 0.3)),
            Shape::Box {
                half: Vec3::splat(0.4),
            },
        ),
    ];
    let separate: Vec<_> = bodies
        .iter()
        .map(|body| {
            CollisionWorld::new(
                vec![Body::new(body.to_world, body.shape.clone())],
                Vec::new(),
            )
        })
        .collect();
    let world = CollisionWorld::new(bodies, Vec::new());
    let bits = |hits: Vec<(f32, Vec3)>| -> Vec<[u32; 4]> {
        hits.into_iter()
            .map(|(d, n)| [d.to_bits(), n.x.to_bits(), n.y.to_bits(), n.z.to_bits()])
            .collect()
    };
    for shape in [
        Shape::Sphere { radius: 0.6 },
        Shape::Box {
            half: Vec3::splat(0.6),
        },
        Shape::Cylinder {
            radius: 0.6,
            half_height: 0.8,
        },
    ] {
        let query = |world: &CollisionWorld, out: &mut Vec<(f32, Vec3)>| {
            if let Shape::Sphere { radius } = shape {
                world.sphere_contacts(centre, radius, Layers::SOLDIER, out);
            } else {
                world.shape_contacts(&shape, Mat4::from_translation(centre), Layers::SOLDIER, out);
            }
        };
        let prefix = (17.0, Vec3::new(-0.0, 2.0, -3.0));
        let mut expected = vec![prefix];
        for body in &separate {
            let before = expected.len();
            query(body, &mut expected);
            assert!(
                expected.len() > before,
                "fixture body must contribute a contact"
            );
        }
        let mut actual = vec![prefix];
        query(&world, &mut actual);
        assert_eq!(bits(actual), bits(expected));
    }
}

/// Card OBST2: [`CollisionWorld::wall_sweep_small`] asks only the bodies
/// no wider than its limit — a wall across the way is passed over for
/// the crate behind it, which the whole sweep never reaches — and with
/// the limit open it is the whole sweep.
#[test]
fn the_small_sweep_asks_only_small_bodies() {
    let wall = Body::new(
        Mat4::from_translation(Vec3::new(0.0, 1.0, -2.0)),
        Shape::Box {
            half: Vec3::new(5.0, 1.0, 0.25),
        },
    );
    let small = Body::new(
        Mat4::from_translation(Vec3::new(0.0, 1.0, -4.0)),
        Shape::Box {
            half: Vec3::new(0.6, 1.0, 0.6),
        },
    );
    let world = CollisionWorld::new(vec![wall, small], Vec::new());
    let cap = Capsule {
        base: Vec3::ZERO,
        height: 1.9,
        radius: 0.4,
    }
    .stepped();
    let (t, n, body) = world
        .wall_sweep_body(&cap, Vec3::NEG_Z, 5.0)
        .expect("the wall");
    assert!((t - 1.35).abs() < 1e-2 && n.z > 0.99, "the wall: {t} {n}");
    assert!(body.aabb.max.x > 4.0);
    let (t, n, body) = world
        .wall_sweep_small(&cap, Vec3::NEG_Z, 5.0, 1.5)
        .expect("the crate");
    assert!((t - 3.0).abs() < 1e-2 && n.z > 0.99, "the crate: {t} {n}");
    assert!(body.aabb.max.x < 1.0);
    // Half its width is the measure, along X and along Z.
    assert!(world
        .wall_sweep_small(&cap, Vec3::NEG_Z, 5.0, 0.59)
        .is_none());
    assert!(world
        .wall_sweep_small(&cap, Vec3::NEG_Z, 5.0, 0.61)
        .is_some());
    let whole = world.wall_sweep_small(&cap, Vec3::NEG_Z, 5.0, f32::INFINITY);
    assert!(whole.is_some_and(|(t, _, _)| (t - 1.35).abs() < 1e-2));
}

/// Card STFAL: the hull probe's pass sees the vehicles' hulls and
/// nothing of the map's own — a wall of the map pushes the full pass and
/// not the hull-only one, and a parked hull's low part pushes both.
#[test]
fn the_hull_only_pass_sees_hulls_and_not_the_map() {
    let wall = Body::new(
        Mat4::from_translation(Vec3::new(2.5, 0.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.5, 5.0, 5.0),
        },
    );
    let mut world = CollisionWorld::new(vec![wall], Vec::new());
    let cap = Capsule {
        base: Vec3::new(2.3, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    assert!(world.push_out_soldier(&cap).x < -0.5, "the map's wall");
    assert_eq!(world.push_out_soldier_hulls(&cap), Vec3::ZERO);
    // A low box on a hull at -3 m: its top (0.9 m) is over the step and
    // under the lifted capsule's round bottom — an AT-ST foot.
    let owner = crate::vehicle::VehicleHandle { index: 0, gen: 0 };
    let part = crate::collision::HullPart::new(
        Mat4::IDENTITY,
        Shape::Box {
            half: Vec3::new(0.5, 0.45, 2.0),
        },
        Layers::ALL,
    );
    world.set_hull(
        owner,
        &[part],
        Mat4::from_translation(Vec3::new(-3.0, 0.45, 0.0)),
    );
    let at_foot = Capsule {
        base: Vec3::new(-2.3, 0.0, 0.0),
        ..cap
    };
    let push = world.push_out_soldier_hulls(&at_foot);
    assert!(push.x > 0.1, "the hull's low box pushes: {push}");
    assert_eq!(
        push,
        world.push_out_soldier(&at_foot),
        "as the full pass does"
    );
}

#[test]
fn push_out_is_horizontal_and_order_independent_for_one_wall() {
    // Slab spanning x in (2, 3).
    let wall = Body::new(
        Mat4::from_translation(Vec3::new(2.5, 0.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.5, 5.0, 5.0),
        },
    );
    let world = CollisionWorld::new(vec![wall], Vec::new());
    let cap = Capsule {
        base: Vec3::new(2.3, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    let push = world.push_out(&cap);
    assert_eq!(push.y, 0.0, "the push is horizontal: {push}");
    assert!((push.x - -(0.3 + cap.radius)).abs() < 1e-3, "{push}");
    assert!(push.z.abs() < 1e-3, "{push}");
    // A floor is the ground query's job, not the wall pass's.
    let floors = world_from_triangles(floor_quad(0.0, 20.0));
    assert_eq!(floors.push_out(&cap), Vec3::ZERO);
}

/// THE POINT OF THE SWEEP: a THIN wall 0.4 m ahead of a 1.0 m step.
///
/// The Phase A.1 probe translated the capsule the whole 1.0 m and asked
/// for a contact THERE, so a wall thinner than the gap it jumped — this
/// one is 0.1 m thick and the capsule clears it by 0.2 m — was never
/// touched, and the unit ran at it at full speed for ever while the wall
/// pass pushed it back (PLAN "PHASE A.2 DESIGN"). The sweep sees it.
#[test]
fn a_thin_wall_inside_the_step_is_swept_and_the_point_probe_missed_it() {
    // Wall: a 0.1 m thick slab centred at x = 1.8 (x in [1.75, 1.85]).
    // Capsule r 0.4 at x = 1.0, so its front face is at x = 1.4 and its
    // front face after a 1.0 m step is at x = 2.4 — clear past the wall,
    // which is why the point probe found nothing.
    let world = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(1.8, 0.0, 0.0)),
            Shape::Box {
                half: Vec3::new(0.05, 5.0, 5.0),
            },
        )],
        Vec::new(),
    );
    let cap = Capsule {
        base: Vec3::new(1.0, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    // The A.1 probe's own test: a contact at the translated pose. The
    // capsule at x = 2.0 spans x in [1.6, 2.4] — which DOES overlap this
    // slab, so make the miss explicit by translating past it instead.
    let past = Capsule {
        base: Vec3::new(3.0, 0.0, 0.0),
        ..cap
    };
    let (shape, iso) = past.to_parry();
    let point_probe = world.bodies()[0].shape.with_parry(&mut |sh| {
        parry3d::query::contact(&iso, &shape, &world.bodies()[0].iso, sh, 0.0)
            .ok()
            .flatten()
            .is_some_and(|c| c.dist < 0.0)
    });
    assert!(
        !point_probe,
        "a point probe 2 m on has walked THROUGH the wall"
    );

    let (t, n) = world
        .wall_sweep(&cap, Vec3::X, 1.0)
        .expect("the sweep sees the thin wall");
    assert!(
        (t - 0.35).abs() < 1e-3,
        "hit 0.35 m along (1.75 - 1.4): {t}"
    );
    // The normal points BACK AT THE MOVER.
    assert!((n - Vec3::NEG_X).length() < 1e-3, "points back: {n}");
    assert_eq!(n.y, 0.0, "the normal is XZ");
    assert!((n.length() - 1.0).abs() < 1e-5, "unit length: {n}");
    assert!(n.dot(Vec3::X) < 0.0, "opposes the direction of travel");

    // Clear, and nothing moved: a shorter step stops short of the wall,
    // and walking away is clear at any distance.
    assert_eq!(world.wall_sweep(&cap, Vec3::X, 0.3), None, "stops short");
    assert_eq!(world.wall_sweep(&cap, Vec3::NEG_X, 5.0), None);
    assert_eq!(cap.base, Vec3::new(1.0, 0.0, 0.0));
    // `dir_xz.y` is ignored: the sweep is horizontal.
    assert!(world
        .wall_sweep(&cap, Vec3::new(1.0, 9.0, 0.0), 1.0)
        .is_some());
    // Zero/degenerate inputs are "clear", never a panic.
    assert_eq!(world.wall_sweep(&cap, Vec3::X, 0.0), None);
    assert_eq!(world.wall_sweep(&cap, Vec3::Y, 1.0), None);
}

/// A WALKABLE floor is never a wall — the same `WALKABLE_MIN_NY` filter
/// the push applies — so a unit may sweep along the ground it stands on.
#[test]
fn the_sweep_skips_a_walkable_floor() {
    let cap = Capsule {
        base: Vec3::new(1.0, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    let floors = world_from_triangles(floor_quad(0.0, 20.0));
    assert_eq!(floors.wall_sweep(&cap, Vec3::X, 1.0), None);
    assert_eq!(floors.push_out(&cap), Vec3::ZERO);
    // A 30 deg ramp is walkable too (|n.y| = cos 30 = 0.87): a unit
    // standing on it may sweep UP it without reading it as a wall.
    let ramps = world_from_triangles(ramp(30.0, 0.0));
    let on_ramp = Capsule {
        base: Vec3::new(1.0, 30f32.to_radians().tan(), 0.0),
        ..cap
    };
    assert_eq!(ramps.wall_sweep(&on_ramp, Vec3::X, 1.0), None);
    // A vertical face is not: the same mesh path, opposite verdict.
    let wall = world_from_triangles(vec![
        [
            Vec3::new(1.6, -5.0, -5.0),
            Vec3::new(1.6, -5.0, 5.0),
            Vec3::new(1.6, 5.0, -5.0),
        ],
        [
            Vec3::new(1.6, 5.0, -5.0),
            Vec3::new(1.6, -5.0, 5.0),
            Vec3::new(1.6, 5.0, 5.0),
        ],
    ]);
    let (t, n) = wall.wall_sweep(&cap, Vec3::X, 1.0).expect("a wall");
    assert!((t - 0.2).abs() < 1e-3, "1.6 - 1.4 = 0.2: {t}");
    assert!(n.x < -0.99, "points back at the mover: {n}");
}

/// A capsule the wall pass left EXACTLY TOUCHING a wall may still walk
/// ALONG it and AWAY from it — and only walking INTO it is blocked.
///
/// This is the regression that cost three maps when the sweep first
/// landed. A shape cast from a touching pose returns a hit at (float) zero
/// distance for every direction that does not strictly separate — 6.2e-5 m
/// along +Z for the pose below — so the steering's slide ALONG the wall
/// came back blocked, its whole fan came back blocked, and the unit backed
/// off. `SWEEP_CONTACT_DIST` hands that case to the point test instead.
#[test]
fn a_capsule_already_touching_a_wall_may_still_walk_along_it() {
    // A DEEP block whose near face is at x = 1.5, so a step into it is
    // unambiguously into that face (a 1 m step through a 1 m slab comes
    // out the far side, and the point test rightly says so).
    let world = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(3.5, 0.0, 0.0)),
            Shape::Box {
                half: Vec3::new(2.0, 5.0, 5.0),
            },
        )],
        Vec::new(),
    );
    // Wall face at x = 1.5; the capsule's front face is exactly on it.
    let touching = Capsule {
        base: Vec3::new(1.1, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    for d in [Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
        assert_eq!(world.wall_sweep(&touching, d, 1.0), None, "dir {d}");
    }
    // Walking INTO it is still blocked, at contact distance, with the
    // wall's own normal — this is what the point test is there to keep.
    let (t, n) = world
        .wall_sweep(&touching, Vec3::X, 1.0)
        .expect("into the wall");
    assert!(t < SWEEP_CONTACT_DIST, "a contact, not a wall ahead: {t}");
    assert!(n.x < -0.99, "the wall's normal: {n}");
    // Overlapping it — what a push-out rounding error leaves — answers the
    // same way, so a unit inside geometry can always walk back out.
    let inside = Capsule {
        base: Vec3::new(1.3, 0.0, 0.0),
        ..touching
    };
    assert_eq!(world.wall_sweep(&inside, Vec3::NEG_X, 1.0), None, "out");
    assert!(
        world.wall_sweep(&inside, Vec3::X, 1.0).is_some(),
        "deeper in"
    );
    // A wall the sweep is CLOSING on from a clear pose is still reported:
    // this only silences contacts, it does not blind the probe.
    let clear = Capsule {
        base: Vec3::new(0.5, 0.0, 0.0),
        ..touching
    };
    let (t, n) = world.wall_sweep(&clear, Vec3::X, 1.0).expect("ahead");
    assert!((t - 0.6).abs() < 1e-3, "1.5 - 0.9: {t}");
    assert!(n.x < -0.99, "{n}");
    // ... and a wall it is moving ALONG is not, at any distance.
    assert_eq!(world.wall_sweep(&clear, Vec3::Z, 5.0), None);
}

/// Two walls in the sweep: the NEAREST decides, and an exact tie goes to
/// the FIRST body — `bodies` order, strict `<` on the time of impact, the
/// rule every other query in this module follows.
#[test]
fn the_sweep_takes_the_nearest_wall_and_the_first_body_on_a_tie() {
    let cap = Capsule {
        base: Vec3::ZERO,
        height: 1.9,
        radius: 0.4,
    };
    // Far wall listed FIRST, near wall second: the near one still wins.
    let world = CollisionWorld::new(
        vec![
            Body::new(
                Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)),
                Shape::Box {
                    half: Vec3::new(0.1, 5.0, 5.0),
                },
            ),
            Body::new(
                Mat4::from_translation(Vec3::new(1.6, 0.0, 0.0)),
                Shape::Box {
                    half: Vec3::new(0.1, 5.0, 5.0),
                },
            ),
        ],
        Vec::new(),
    );
    let (t, _) = world.wall_sweep(&cap, Vec3::X, 5.0).expect("blocked");
    assert!((t - 1.1).abs() < 1e-3, "the nearer wall (1.5 - 0.4): {t}");

    // An exact tie: identical slabs give identical numeric answers, so
    // also observe the identity of the FIRST body that answered.
    let slab = |m: Mat4| {
        Body::new(
            m,
            Shape::Box {
                half: Vec3::new(0.1, 5.0, 0.1),
            },
        )
    };
    let at = Mat4::from_translation(Vec3::new(1.6, 0.0, 0.0));
    let tied = CollisionWorld::new(vec![slab(at), slab(at)], Vec::new());
    let (t0, n0) = tied.wall_sweep(&cap, Vec3::X, 5.0).expect("blocked");
    let (_, _, body) = tied.wall_sweep_body(&cap, Vec3::X, 5.0).expect("blocked");
    assert!(std::ptr::eq(body, &tied.bodies()[0]));
    let one = CollisionWorld::new(vec![slab(at)], Vec::new());
    assert_eq!(Some((t0, n0)), one.wall_sweep(&cap, Vec3::X, 5.0));
}

/// A 45 deg ramp quad on the plane `y = x − x0`, over `x in [lo, hi]` and
/// `z in [−5, 5]`. Its face normal is `(∓0.707, ±0.707, 0)`: `|n.y|` is
/// well above [`WALKABLE_MIN_NY`], so every triangle of it is a FLOOR.
fn ramp_quad(x0: f32, lo: f32, hi: f32) -> Vec<[Vec3; 3]> {
    let p = |x: f32, z: f32| Vec3::new(x, x - x0, z);
    vec![
        [p(lo, -5.0), p(hi, -5.0), p(hi, 5.0)],
        [p(lo, -5.0), p(hi, 5.0), p(lo, 5.0)],
    ]
}

/// A vertical wall quad in the plane `x = at`, `y in [−1, 3]`,
/// `z in [−5, 5]`. Face normal `(±1, 0, 0)`: a WALL.
fn wall_quad(at: f32) -> Vec<[Vec3; 3]> {
    let q = |y: f32, z: f32| Vec3::new(at, y, z);
    vec![
        [q(-1.0, -5.0), q(-1.0, 5.0), q(3.0, -5.0)],
        [q(3.0, -5.0), q(-1.0, 5.0), q(3.0, 5.0)],
    ]
}

/// Phase A.3: a wall standing behind a ramp OF THE SAME MESH is seen.
///
/// One body, two surfaces: a 45 deg ramp rising in front of the capsule
/// (hit first, at 0.33 m, with a FLOOR normal) and a vertical wall 0.8 m
/// ahead of the capsule's front face. Before A.3 the whole-body cast's
/// floor hit skipped the whole BODY and the sweep answered `None` while
/// the wall pass pushed the unit back every tick — the BF1 NAB2 stall
/// (PLAN Log, "Phase A.2 landed"). The per-triangle pass finds the wall.
#[test]
fn the_sweep_finds_a_wall_behind_a_ramp_of_the_same_mesh() {
    let cap = Capsule {
        base: Vec3::ZERO,
        height: 1.9,
        radius: 0.4,
    };
    // Ramp `y = x − 0.5` over x in [0.5, 1.2]; wall at x = 1.2, which is
    // 0.8 m from the capsule's front face at x = 0.4.
    let ramp = ramp_quad(0.5, 0.5, 1.2);
    let wall = wall_quad(1.2);

    // The ramp ALONE is still no wall: the floor filter is intact.
    let floors = world_from_triangles(ramp.clone());
    assert_eq!(floors.wall_sweep(&cap, Vec3::X, 1.0), None, "a ramp");

    let both: Vec<[Vec3; 3]> = ramp.iter().chain(wall.iter()).copied().collect();
    let one_body = world_from_triangles(both);
    let (t, n) = one_body
        .wall_sweep(&cap, Vec3::X, 1.0)
        .expect("the wall behind the ramp");
    assert!((t - 0.8).abs() < 1e-3, "1.2 − 0.4 = 0.8: {t}");
    assert!(n.x < -0.99, "points back at the mover: {n}");
    assert_eq!(n.y, 0.0, "the normal is XZ");

    // The same geometry as TWO bodies — the ramp body contributes nothing
    // and the wall body answers through the ordinary whole-body path — is
    // the same answer, so the per-triangle path agrees with the per-body
    // one.
    let two = CollisionWorld::new(vec![mesh_body(&ramp), mesh_body(&wall)], Vec::new());
    assert_eq!(
        two.wall_sweep(&cap, Vec3::X, 1.0),
        Some((t, n)),
        "one body vs two"
    );
}

/// Phase A.3, the TOUCHING branch: the capsule is exactly on the wall and
/// penetrating the ramp of the same mesh, and the ramp is the DEEPER
/// end-pose contact — so the body branch read a floor normal and skipped
/// the body. The per-triangle pass asks the same point-test question of
/// each non-floor triangle and the wall answers.
#[test]
fn a_capsule_touching_a_wall_over_a_ramp_of_the_same_mesh_is_blocked() {
    let cap = Capsule {
        base: Vec3::new(0.8, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    // The wall is exactly on the capsule's front face (0.8 + 0.4).
    // The ramp is placed so the capsule's bottom cap already penetrates it
    // by 0.15 m, which grows to 0.36 m over the 0.3 m step while the
    // wall's penetration only reaches 0.30 m: the ramp is the deeper
    // end-pose contact, which is what the body branch reads.
    let ramp = ramp_quad(0.4 + 0.25 * std::f32::consts::SQRT_2, -2.0, 1.2);
    let wall = wall_quad(1.2);
    let both: Vec<[Vec3; 3]> = ramp.iter().chain(wall.iter()).copied().collect();
    let world = world_from_triangles(both);

    // Pin the construction: the DEEPEST end-pose contact really is a floor.
    let mut end = cap;
    end.base += Vec3::X * 0.3;
    let (end_shape, end_iso) = end.to_parry();
    let body = &world.bodies()[0];
    let c = body
        .shape
        .with_parry(&mut |s| {
            parry3d::query::contact(&end_iso, &end_shape, &body.iso, s, 0.0)
                .ok()
                .flatten()
        })
        .expect("an end-pose contact");
    assert!(
        c.dist < 0.0,
        "penetrating at the end of the step: {}",
        c.dist
    );
    assert!(
        c.normal2.y.abs() >= WALKABLE_MIN_NY,
        "the deepest end-pose contact is the RAMP: {:?}",
        c.normal2
    );

    let (t, n) = world
        .wall_sweep(&cap, Vec3::X, 0.3)
        .expect("blocked by the wall it is touching");
    assert!(t < SWEEP_CONTACT_DIST, "a contact, not a wall ahead: {t}");
    assert!(n.x < -0.99, "the wall's normal: {n}");
}

/// The `coll` MESH twin of the fixture below: a wall QUAD at x = 1.5
/// spanning y 0..6 and z −5..5, as two triangles. A `coll` wall is a
/// zero-thickness SHEET, which is why a step of `ai::PROBE_DIST` from
/// inside it comes out the far side with no contact at all.
fn wall_sheet() -> Vec<[Vec3; 3]> {
    let p = |y: f32, z: f32| Vec3::new(1.5, y, z);
    vec![
        [p(0.0, -5.0), p(0.0, 5.0), p(6.0, 5.0)],
        [p(0.0, -5.0), p(6.0, 5.0), p(6.0, -5.0)],
    ]
}

/// Phase A.4: a body the capsule is ALREADY INSIDE still blocks the way
/// FURTHER IN — the cast cannot say which way, and the end-pose point test
/// answers about the wrong escape direction.
///
/// The fixture is BF2 MYG1's stall, reduced: a 118 deg rotated box, the
/// probe capsule 0.1 … 1.0 m inside one side face with its feet at y 2.9.
/// The capsule's feet are 1.1 m below the box's TOP, so at the END of a
/// 1 m step INTO the face the cheapest way out of the box is UPWARD: the
/// deepest end-pose contact's normal is (0, 1, 0), a FLOOR, and the A.2
/// rule skipped the whole body — `wall_sweep` answered `None` in every
/// direction while `push_out` reported the penetration exactly. Unit 6 sat
/// there for 53,400 ticks at `steer fwd` with `push 0.300`.
///
/// The raw-parry assertions come FIRST and are the DETECTOR for a future
/// parry upgrade, as in `mesh_contact_is_not_mirrored`.
#[test]
fn a_penetrated_rotated_box_still_blocks_the_way_in() {
    let to_world = Mat4::from_rotation_y(118f32.to_radians());
    let nx = to_world.transform_vector3(Vec3::X).normalize();
    let nz = to_world.transform_vector3(Vec3::Z).normalize();
    let world = CollisionWorld::new(
        vec![Body::new(
            to_world,
            Shape::Box {
                half: Vec3::new(4.0, 4.0, 9.8),
            },
        )],
        Vec::new(),
    );
    // The capsule `depth` metres inside the +X face, feet at y 2.9.
    let at = |depth: f32| Capsule {
        base: nx * (4.0 + 0.5 - depth) + Vec3::Y * 2.9,
        height: 1.9,
        radius: 0.5,
    };
    // The AI's own step (`ai::PROBE_DIST`), which is what makes the
    // end-pose escape vertical: 1.0 m in on top of 0.1 m already inside is
    // more than the 1.1 m to the box's top face.
    const STEP: f32 = 1.0;

    // RAW parry, at depth 0.3, with the arguments `wall_sweep` passes: a
    // cast from a PENETRATING pose reports t = 0 in EVERY direction — out
    // of the box included — always with the same, direction-independent
    // normal. That is why the cast cannot judge this body.
    const FIXED: &str = "parry's shape cast now answers a direction from inside a body — \
                         re-read brief_a4_sweep.md before trusting it";
    let cap = at(0.3);
    let (shape, iso) = cap.to_parry();
    let body = &world.bodies()[0];
    let raw = |d: Vec3| {
        body.shape.with_parry(&mut |s| {
            parry3d::query::cast_shapes(
                &iso,
                &vector(d),
                &shape,
                &body.iso,
                &vector(Vec3::ZERO),
                s,
                parry3d::query::ShapeCastOptions {
                    max_time_of_impact: STEP,
                    target_distance: 0.0,
                    stop_at_penetration: true,
                    compute_impact_geometry_on_penetration: true,
                },
            )
            .ok()
            .flatten()
        })
    };
    let into = raw(-nx).expect("a cast from inside reports a hit");
    for d in [nx, nz, -nz] {
        let h = raw(d).unwrap_or_else(|| panic!("{FIXED}: no hit along {d}"));
        assert_eq!(h.time_of_impact, 0.0, "{FIXED}: along {d}");
        assert_eq!(
            h.normal1, into.normal1,
            "{FIXED}: the cast's normal now depends on the direction ({d})"
        );
    }
    assert_eq!(into.time_of_impact, 0.0, "{FIXED}");

    // OURS: the penetration test answers what the cast cannot.
    for depth in [0.1f32, 0.3, 0.5, 1.0] {
        let cap = at(depth);
        let push = world.push_out(&cap);
        assert!(
            (push.length() - depth).abs() < 1e-3 && push.normalize().dot(nx) > 0.99,
            "the fixture: {depth} m inside the +X face, push {push:?}"
        );
        let (t, n) = world
            .wall_sweep(&cap, -nx, STEP)
            .unwrap_or_else(|| panic!("blocked going deeper in, depth {depth}"));
        assert!(t < SWEEP_CONTACT_DIST, "at contact, not ahead: {t}");
        assert!(n.dot(nx) > 0.99, "pushed back out of the face: {n:?}");
        // Along the face and back OUT of it are both open — a step may
        // keep or reduce a penetration, only never deepen one.
        for d in [nz, -nz, nx] {
            assert_eq!(world.wall_sweep(&cap, d, STEP), None, "depth {depth}, {d}");
        }
    }
}

/// Phase A.4, the MESH half: the same, for a `coll` wall — a zero-
/// thickness sheet. A capsule already inside one and stepping
/// `ai::PROBE_DIST` further in ends CLEAN THROUGH it, so the end-pose
/// point test found no contact at all and answered `None` INTO the wall,
/// while stepping ALONG it — where the end pose is still inside — answered
/// BLOCKED. The wrong way round in both directions.
#[test]
fn a_penetrated_mesh_wall_still_blocks_the_way_in() {
    const STEP: f32 = 1.0;
    for deg in [0.0f32, 118.0] {
        let to_world = Mat4::from_rotation_y(deg.to_radians());
        let world = CollisionWorld::new(vec![mesh_body_at(&wall_sheet(), to_world)], Vec::new());
        let ax = to_world.transform_vector3(Vec3::X).normalize();
        let az = to_world.transform_vector3(Vec3::Z).normalize();
        // `depth` past the sheet, the capsule's axis still on the near
        // side of it (which is the side `push_out` pushes back to).
        for depth in [0.1f32, 0.3, 0.45] {
            let cap = Capsule {
                base: ax * (1.0 + depth),
                height: 1.9,
                radius: 0.5,
            };
            let push = world.push_out(&cap);
            assert!(
                (push.length() - depth).abs() < 1e-3 && push.normalize().dot(ax) < -0.99,
                "the fixture: {deg} deg, {depth} m inside, push {push:?}"
            );
            let (t, n) = world
                .wall_sweep(&cap, ax, STEP)
                .unwrap_or_else(|| panic!("blocked going deeper in, {deg} deg, {depth}"));
            assert!(t < SWEEP_CONTACT_DIST, "at contact, not ahead: {t}");
            assert!(n.dot(ax) < -0.99, "pushed back out of the sheet: {n:?}");
            for d in [az, -az, -ax] {
                assert_eq!(world.wall_sweep(&cap, d, STEP), None, "{depth} m, {d}");
            }
        }
    }
}

/// Phase A.4, the rule in one sentence: a step may KEEP a penetration
/// (walking along the wall) or REDUCE one (walking out of it), and may
/// never DEEPEN one. The unit-test box, the capsule 0.2 m inside its
/// x = 1.5 face.
#[test]
fn a_step_may_not_deepen_a_penetration_but_may_keep_it() {
    let world = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(3.5, 0.0, 0.0)),
            Shape::Box {
                half: Vec3::new(2.0, 5.0, 5.0),
            },
        )],
        Vec::new(),
    );
    let inside = Capsule {
        base: Vec3::new(1.3, 0.0, 0.0),
        height: 1.9,
        radius: 0.4,
    };
    assert!(
        (world.push_out(&inside) - Vec3::new(-0.2, 0.0, 0.0)).length() < 1e-3,
        "the fixture: 0.2 m inside the x = 1.5 face"
    );
    // ALONG the wall: kept, not deepened — open.
    assert_eq!(world.wall_sweep(&inside, Vec3::Z, 1.0), None, "along");
    assert_eq!(world.wall_sweep(&inside, Vec3::NEG_Z, 1.0), None, "along");
    // OUT: reduced — open.
    assert_eq!(world.wall_sweep(&inside, Vec3::NEG_X, 1.0), None, "out");
    // IN: deepened — blocked, at contact distance, with the wall's normal.
    let (t, n) = world
        .wall_sweep(&inside, Vec3::X, 1.0)
        .expect("deeper in is blocked");
    assert!(t < SWEEP_CONTACT_DIST, "at contact, not ahead: {t}");
    assert!(n.x < -0.99, "the wall's normal: {n}");
}

/// An ASYMMETRIC mesh body for the parry-mirror regression tests: two
/// parallel wall quads, at local x = 5 and x = -6, on an instance
/// translated to (100, 0, 0). The real walls are at world x = 105 and
/// x = 94; the POINT MIRROR of the x = 105 wall through the instance's
/// origin lands at world x = 95, where there is nothing — and it is inside
/// the body's world AABB, so the broad phase does not hide it. This is
/// the shape of a real building instance (BF1 BES1's walkways), and it is
/// why the symmetric `floor_quad`/`wall_quad`-at-the-origin worlds the
/// other tests use never caught the bug.
fn mirror_body() -> Body {
    let mut tris = wall_quad(5.0);
    tris.extend(wall_quad(-6.0));
    mesh_body_at(&tris, Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)))
}

fn mirror_world() -> CollisionWorld {
    CollisionWorld::new(vec![mirror_body()], Vec::new())
}

fn raw_parry_contact(body: &Body, cap: &Capsule) -> Option<parry3d::query::Contact> {
    let Shape::Mesh(mesh) = &body.shape else {
        unreachable!("mesh body")
    };
    let (shape, iso) = cap.to_parry();
    parry3d::query::contact(&iso, &shape, &body.iso, mesh.as_ref(), 0.0)
        .ok()
        .flatten()
}

/// parry 0.22.1's composite `contact` tests the capsule at a MIRRORED pose
/// (module header), so the wall pass was pushing units out of a phantom
/// copy of every building. [`mesh_contact`] asks the triangles directly.
///
/// The raw-parry assertions come FIRST and are the DETECTOR for a future
/// parry upgrade: they assert that upstream is still wrong, and say what
/// to do when it is not.
#[test]
fn mesh_contact_is_not_mirrored() {
    let body = mirror_body();
    // 0.2 m INTO the real wall at world x = 105.
    let real = Capsule {
        base: Vec3::new(104.7, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    // Its point mirror through the instance origin: empty air, 1.3 m from
    // the nearest real surface (the x = 94 wall).
    let phantom = Capsule {
        base: Vec3::new(95.3, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    const FIXED: &str =
        "parry fixed contact_composite_shape_shape — drop `mesh_contact`'s workaround";

    let raw_real = raw_parry_contact(&body, &real);
    assert!(
        !raw_real.is_some_and(|c| c.dist < 0.0),
        "{FIXED}: raw parry now sees the real wall the capsule is 0.2 m inside ({raw_real:?})"
    );
    let raw_phantom = raw_parry_contact(&body, &phantom);
    assert!(
        raw_phantom.is_some_and(|c| c.dist < 0.0),
        "{FIXED}: raw parry no longer reports a penetration in the empty air at the \
         mirrored pose ({raw_phantom:?})"
    );

    // OURS: the real wall pushes, the phantom does not.
    let world = mirror_world();
    let push = world.push_out(&real);
    assert!(
        push.x < -0.1 && push.y == 0.0,
        "pushed back out of the real wall, got {push:?}"
    );
    assert_eq!(
        world.push_out(&phantom),
        Vec3::ZERO,
        "nothing to push out of at the mirrored pose"
    );

    // A ROTATED instance: the mirror is no longer a reflection in x, and
    // the wall's world normal is not one of the mesh's own axes.
    let to_world = Mat4::from_rotation_translation(
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        Vec3::new(-20.0, 0.0, 40.0),
    );
    let mut tris = wall_quad(5.0);
    tris.extend(wall_quad(-6.0));
    let rotated = mesh_body_at(&tris, to_world);
    // The wall's world plane and outward normal, from the instance matrix.
    let n = to_world
        .transform_vector3(Vec3::new(-1.0, 0.0, 0.0))
        .normalize();
    let on_wall = to_world.transform_point3(Vec3::new(5.0, 0.0, 0.0));
    // 0.2 m into it, from the side the wall faces.
    let cap = Capsule {
        base: on_wall + n * 0.3,
        height: 1.9,
        radius: 0.5,
    };
    let raw_rot = raw_parry_contact(&rotated, &cap);
    assert!(
        !raw_rot.is_some_and(|c| c.dist < 0.0),
        "{FIXED}: raw parry now sees the rotated instance's real wall ({raw_rot:?})"
    );
    let rot_world = CollisionWorld::new(vec![rotated], Vec::new());
    let push = rot_world.push_out(&cap);
    assert!(
        push.length() > 0.1 && push.normalize().dot(n) > 0.99,
        "pushed along the rotated wall's world normal {n:?}, got {push:?}"
    );
}

/// The same bug through [`CollisionWorld::wall_sweep`]'s end-pose contact
/// (A.2's "touching branch", now the not-penetrating half of
/// [`penetration_block`] — the test is renamed with it, its assertions
/// unchanged): a unit standing against a wall asked parry's composite
/// `contact` at the end pose, got the phantom's answer, and reported the
/// way ahead CLEAR while [`CollisionWorld::push_out`] shoved it back — the
/// stall signature. The A.3 per-triangle cast pass is not involved.
#[test]
fn the_wall_sweeps_penetration_test_is_not_mirrored() {
    let world = mirror_world();
    // EXACTLY touching the real wall at world x = 105, walking into it.
    let touching = Capsule {
        base: Vec3::new(104.5, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    let (t, n) = world
        .wall_sweep(&touching, Vec3::X, 0.3)
        .expect("the real wall blocks");
    assert!(t < SWEEP_CONTACT_DIST, "the touching branch, got t = {t}");
    assert!(n.x < -0.99, "pushed back off the wall, got {n:?}");

    // The mirror of that pose: nothing ahead within the step.
    let mirrored = Capsule {
        base: Vec3::new(95.5, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    assert_eq!(world.wall_sweep(&mirrored, Vec3::X, 0.3), None);
}

/// A KAS2 City Hall lobe (card JWAL, `analysis/re-soldier-walls.md` §2.2):
/// a solid cylinder r 9.25 whose top is at y 17.007, at the origin.
fn lobe() -> CollisionWorld {
    CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(0.0, 15.007, 0.0)),
            Shape::Cylinder {
                radius: 9.25,
                half_height: 2.0,
            },
        )],
        Vec::new(),
    )
}

/// The exact pose the port's soldier entered the lobe at (the walk-jump
/// reproduction's tick 42, feet 16.0667, 0.2323 m outside the rim). The
/// stepped capsule's lower sphere is 0.16 above the top, so its rim
/// contact is floor-like (n.y 0.5655) and the BARE pass skips it. The
/// soldier's pass asks the full capsule instead and pushes him out.
#[test]
fn a_ledges_rim_under_a_jump_is_a_wall_for_the_soldier() {
    let world = lobe();
    let full = Capsule {
        base: Vec3::new(9.4823, 16.0667, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    assert_eq!(
        world.push_out(&full.stepped()),
        Vec3::ZERO,
        "the bare pass skips the rim (the leak's mechanism)"
    );
    let push = world.push_out_soldier(&full);
    assert!(push.x > 0.25 && push.z.abs() < 1e-4, "pushed out: {push}");
    assert_eq!(push.y, 0.0);

    // One tick later in the leak's residue: 0.31 m inside the rim at feet
    // 16.39, where the full capsule's shortest way out is UP (0.61) and
    // the ground query cannot lift him. The ledge escape pushes him out
    // radially instead.
    let inside = Capsule {
        base: Vec3::new(8.942, 16.3929, 0.0),
        ..full
    };
    let push = world.push_out_soldier(&inside);
    assert!(
        (push.x - (9.25 + 0.5 - 8.942)).abs() < 1e-3,
        "radially out: {push}"
    );

    // Within the step it is a step: feet 0.5 below the top, the ground
    // query stands him on it, and the wall pass leaves him be.
    let step = Capsule {
        base: Vec3::new(9.0, 16.507, 0.0),
        ..full
    };
    assert_eq!(world.push_out_soldier(&step), Vec3::ZERO);
    // Standing on the top: nothing to push.
    let on_top = Capsule {
        base: Vec3::new(5.0, 17.007, 0.0),
        ..full
    };
    assert_eq!(world.push_out_soldier(&on_top), Vec3::ZERO);
}

/// The AI's probe agrees: a crate 0.75 m tall (above the step) blocks the
/// stepped capsule's sweep, where its top-edge graze used to be skipped
/// as a floor.
#[test]
fn the_probe_sees_a_crate_above_the_step() {
    let world = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(2.0, 0.375, 0.0)),
            Shape::Box {
                half: Vec3::new(0.5, 0.375, 2.0),
            },
        )],
        Vec::new(),
    );
    let cap = Capsule {
        base: Vec3::new(0.0, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    let (t, n) = world
        .wall_sweep(&cap.stepped(), Vec3::X, 2.0)
        .expect("the crate blocks the probe");
    assert!(t < 2.0 && n.x < -0.5, "t {t} n {n}");
    // A kerb under the step (0.4 m) is still open.
    let kerb = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(2.0, 0.2, 0.0)),
            Shape::Box {
                half: Vec3::new(0.5, 0.2, 2.0),
            },
        )],
        Vec::new(),
    );
    assert_eq!(kerb.wall_sweep(&cap.stepped(), Vec3::X, 2.0), None);
}

/// A riser just over the step (BF2 KAM1's 0.62 m console front, the stall
/// card JWAL's first build hit): the stepped capsule's round bottom passes
/// over its top edge without touching it, so no contact asks the ledge
/// rule, yet the full capsule is 0.27 m into the face. The soldier's pass
/// asks the full capsule anyway and pushes him back out; left alone, the
/// next tick puts his axis past the face and the push goes to the far side.
#[test]
fn a_riser_just_over_the_step_blocks_before_the_axis_crosses_it() {
    let p = |x: f32, y: f32| Vec3::new(x, y, -1.0);
    let world = CollisionWorld::new(
        vec![mesh_body(&[
            [p(-5.0, 0.0), p(5.0, 0.0), p(5.0, 0.62)],
            [p(-5.0, 0.0), p(5.0, 0.62), p(-5.0, 0.62)],
        ])],
        Vec::new(),
    );
    let full = Capsule {
        base: Vec3::new(0.0, 0.0, -0.77),
        height: 1.9,
        radius: 0.5,
    };
    assert_eq!(
        world.push_out(&full.stepped()),
        Vec3::ZERO,
        "the stepped capsule is clear"
    );
    let push = world.push_out_soldier(&full);
    assert!(
        (push.z - 0.27).abs() < 1e-3 && push.x.abs() < 1e-4,
        "pushed back out: {push}"
    );
    // A riser within the step is still climbed, not pushed.
    let low = CollisionWorld::new(
        vec![mesh_body(&[
            [p(-5.0, 0.0), p(5.0, 0.0), p(5.0, 0.55)],
            [p(-5.0, 0.0), p(5.0, 0.55), p(-5.0, 0.55)],
        ])],
        Vec::new(),
    );
    assert_eq!(low.push_out_soldier(&full), Vec3::ZERO);
}

/// Card WPSH, M2 (`analysis/re-soldier-walls.md` §4): a BF2 sprint-jump
/// (MaxSpeed 9 × 1.5 × 1.3 = 17.55 m/s, 0.585 m a tick) at a
/// zero-thickness mesh sheet, from every phase of its last step before
/// contact, on an axis-aligned and a rotated instance. The pass at the
/// END pose alone lets the axis past the sheet, and the deepest contact
/// then pushes to the far side; along the move it never crosses. A walk
/// (6.5 m/s) is one sub-step, as before, and stops touching the sheet.
#[test]
fn a_bf2_sprint_jump_cannot_tunnel_a_mesh_sheet() {
    for deg in [0.0f32, 118.0] {
        let to_world = Mat4::from_rotation_y(deg.to_radians());
        let world = CollisionWorld::new(vec![mesh_body_at(&wall_sheet(), to_world)], Vec::new());
        let ax = to_world.transform_vector3(Vec3::X).normalize();
        let at = |x: f32| Capsule {
            base: ax * x + Vec3::Y * 0.3,
            height: 1.9,
            radius: 0.5,
        };
        let mut old_crossed = 0;
        for speed in [17.55f32, 6.5] {
            let step = speed / 30.0;
            for k in 0..8 {
                // The last pose before contact: `phase` short of touching.
                let phase = step * k as f32 / 8.0;
                let mut x = 1.0 - phase;
                for tick in 0..6 {
                    let from = at(x);
                    let to = at(x + step);
                    if tick == 1 && world.push_out_soldier(&to).dot(ax) > 0.0 {
                        old_crossed += 1; // the end-pose pass pushes to the far side
                    }
                    x = (to.base + world.push_out_soldier_from(&from, &to)).dot(ax);
                    assert!(
                        x <= 1.0 + 1e-3,
                        "{deg} deg, {speed} m/s, phase {phase}, tick {tick}: axis at {x}"
                    );
                }
                assert!((x - 1.0).abs() < 1e-3, "{speed} m/s ends touching: {x}");
            }
        }
        assert!(
            old_crossed > 0,
            "the fixture: the end-pose pass tunnels, {deg} deg"
        );
    }
}

/// Card WPSH, M3 (§4.1, the cp5 hut): the capsule's axis came at a face
/// from its near side and is now past it (the hut's outer skin, a
/// ceiling, let it in). The triangle's deepest contact points to the FAR
/// side; the end-pose pass takes it and carries the soldier through, the
/// pass along the move never pushes that way.
#[test]
fn no_push_carries_the_axis_through_a_face_it_came_at() {
    let world = CollisionWorld::new(vec![mesh_body(&wall_sheet())], Vec::new());
    let at = |x: f32| Capsule {
        base: Vec3::new(x, 0.3, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    let (from, to) = (at(1.4), at(1.6));
    assert!(
        world.push_out_soldier(&to).x > 0.3,
        "the fixture: the end-pose pass pushes through"
    );
    let push = world.push_out_soldier_from(&from, &to);
    assert!(push.x <= 0.0, "no push to the far side: {push}");
    // A start that already straddles the face says nothing: its contact
    // stands, as the end-pose pass has it.
    let tilted = CollisionWorld::new(
        vec![mesh_body(&[[
            Vec3::new(1.0, 0.0, -5.0),
            Vec3::new(1.0, 0.0, 5.0),
            Vec3::new(2.0, 4.0, 0.0),
        ]])],
        Vec::new(),
    );
    let (from, to) = (at(1.3), at(1.35));
    assert_eq!(
        tilted.push_out_soldier_from(&from, &to),
        tilted.push_out_soldier(&to),
        "straddling at the start"
    );
}

/// Card WTUN: a two-triangle body, an overhang's underside over the
/// soldier's head and the wall under it. The underside's contact (0.2 m)
/// is deeper than the wall's (0.1 m); the pass answered only the deepest,
/// a ceiling, and skipped the whole body, so the soldier walked through
/// the wall (BF1 BES1 body 26, 95 census runs at one site). The wall
/// contact now wins.
#[test]
fn a_wall_under_a_deeper_overhang_of_the_same_mesh_still_pushes() {
    let tris = vec![
        // The wall: the plane x = 1.0, y in [−1, 3], face normal −X.
        [
            Vec3::new(1.0, -1.0, -5.0),
            Vec3::new(1.0, 3.0, 0.0),
            Vec3::new(1.0, -1.0, 5.0),
        ],
        // The underside: the plane y = 1.7 over x in [−3, 1], face down.
        [
            Vec3::new(-3.0, 1.7, -5.0),
            Vec3::new(-3.0, 1.7, 5.0),
            Vec3::new(1.0, 1.7, 0.0),
        ],
    ];
    let world = world_from_triangles(tris);
    let cap = Capsule {
        base: Vec3::new(0.6, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    let push = world.push_out(&cap);
    assert!((push - Vec3::new(-0.1, 0.0, 0.0)).length() < 1e-3, "{push}");
}

/// Card WTUN: a ceiling-like contact pushes by its normal's HORIZONTAL
/// part. Under a box's lower edge (the slab leaning over the soldier, BF2
/// DAG1) the head's contact points 45 deg down and away, and pushes; flat
/// under the box, straight down, it pushes nothing.
#[test]
fn a_ceiling_contact_pushes_only_its_horizontal_part() {
    // x in (0.3, 4.3), y in (1.7, 3.7).
    let world = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(2.3, 2.7, 0.0)),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 5.0),
            },
        )],
        Vec::new(),
    );
    let cap = |x: f32| Capsule {
        base: Vec3::new(x, 0.0, 0.0),
        height: 1.9,
        radius: 0.5,
    };
    // The top sphere's centre (0, 1.4) is 0.424 m from the edge (0.3, 1.7):
    // 0.076 m deep along (−0.707, −0.707), so the first pass pushes
    // 0.054 m along −X and the second (`WALL_ITERATIONS`) a little more,
    // never the whole 0.108 m a renormalised wall push would take.
    let push = world.push_out(&cap(0.0));
    let first = (0.5 - 0.18f32.sqrt()) * std::f32::consts::FRAC_1_SQRT_2;
    assert!(push.y == 0.0 && push.z.abs() < 1e-4, "{push}");
    assert!(push.x < -first + 1e-4 && push.x > -2.0 * first, "{push}");
    // Flat under the box: 0.2 m deep, straight down, no push.
    assert!(world.push_out(&cap(2.0)).length() < 1e-4);
}

/// Remembering a pick's start contacts changes no answer: from inside a dense
/// mesh, on every heading and on the second asking, each sweep returns the same
/// bits as with nothing remembered.
#[test]
fn remembered_start_contacts_answer_the_same_bits() {
    let mut tris = Vec::new();
    for k in 0..24 {
        let (a0, a1) = (k as f32 * 0.2618, (k + 1) as f32 * 0.2618);
        for j in 0..6 {
            let (y0, y1) = (j as f32 * 0.4, (j + 1) as f32 * 0.4);
            let p = |a: f32, y: f32| Vec3::new(0.3 * a.cos(), y, 0.3 * a.sin());
            tris.push([p(a0, y0), p(a1, y0), p(a1, y1)]);
            tris.push([p(a0, y0), p(a1, y1), p(a0, y1)]);
        }
    }
    let world = world_from_triangles(tris);
    let cap = Capsule {
        base: Vec3::new(0.05, 0.1, 0.02),
        height: 1.8,
        radius: 0.4,
    };
    let headings: Vec<Vec3> = (0..16)
        .map(|k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_8;
            Vec3::new(a.cos(), 0.0, a.sin())
        })
        .collect();
    let bits = |r: Option<(f32, Vec3, &crate::collision::Body)>| {
        r.map(|(t, n, _)| [t.to_bits(), n.x.to_bits(), n.y.to_bits(), n.z.to_bits()])
    };
    let plain: Vec<_> = headings
        .iter()
        .map(|d| bits(world.wall_sweep_small(&cap, *d, 1.0, f32::INFINITY)))
        .collect();
    assert!(plain.iter().any(Option::is_some), "a block to compare");
    for _ in 0..2 {
        let remembered = world.remembering_start_contacts(|| {
            let mut out = Vec::new();
            for d in headings.iter().chain(&headings) {
                out.push(bits(world.wall_sweep_small(&cap, *d, 1.0, f32::INFINITY)));
            }
            out
        });
        assert_eq!(remembered[..16], plain[..]);
        assert_eq!(remembered[16..], plain[..]);
    }
}

/// The key of a remembered contact holds the capsule's place and the body's:
/// inside one scope, another pose or the same mesh placed elsewhere gets its own
/// answer. And nothing outlives the scope.
#[test]
fn remembered_start_contacts_do_not_alias_poses_or_bodies() {
    let mut tris = Vec::new();
    for k in 0..24 {
        let (a0, a1) = (k as f32 * 0.2618, (k + 1) as f32 * 0.2618);
        for j in 0..6 {
            let (y0, y1) = (j as f32 * 0.4, (j + 1) as f32 * 0.4);
            let p = |a: f32, y: f32| Vec3::new(0.3 * a.cos(), y, 0.3 * a.sin());
            tris.push([p(a0, y0), p(a1, y0), p(a1, y1)]);
            tris.push([p(a0, y0), p(a1, y1), p(a0, y1)]);
        }
    }
    let placed = |x: f32, z: f32| {
        CollisionWorld::new(
            vec![mesh_body_at(
                &tris,
                Mat4::from_translation(Vec3::new(x, 0.0, z)),
            )],
            Vec::new(),
        )
    };
    let worlds = [placed(0.0, 0.0), placed(0.13, -0.07), placed(-0.11, 0.09)];
    let caps = [(0.05, 0.02), (0.21, -0.04), (-0.09, 0.17)].map(|(x, z)| Capsule {
        base: Vec3::new(x, 0.1, z),
        height: 1.8,
        radius: 0.4,
    });
    let sweeps = |out: &mut Vec<_>| {
        for world in &worlds {
            for cap in &caps {
                for k in 0..16 {
                    let a = k as f32 * std::f32::consts::FRAC_PI_8;
                    let d = Vec3::new(a.cos(), 0.0, a.sin());
                    let hit = world.wall_sweep_small(cap, d, 1.0, f32::INFINITY);
                    out.push(hit.map(|(t, n, _)| {
                        [t.to_bits(), n.x.to_bits(), n.y.to_bits(), n.z.to_bits()]
                    }));
                }
            }
        }
    };
    let held = || super::queries::START_CONTACTS.with_borrow(|m| (m.on, m.deep.len()));
    let mut plain = Vec::new();
    sweeps(&mut plain);
    assert!(plain.iter().flatten().count() > 20, "blocks to compare");
    let mut remembered = Vec::new();
    worlds[0].remembering_start_contacts(|| {
        sweeps(&mut remembered);
        let (on, held) = held();
        assert!(on && held > 0, "the scope remembers");
    });
    assert_eq!(remembered, plain);
    assert_eq!(held(), (false, 0));
}
