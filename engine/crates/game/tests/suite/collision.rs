//! Tier-1: the runtime side of walkable model collision — how a `Scene` turns
//! its baked `coll`/`prim` data into the sim's `core::collision::CollisionWorld`.
//! (The manifest's own round trip, collision sections included, is
//! `manifest`'s test — the bake and the runtime share that one definition.)

use game::assets::{CollisionGeom, CpuMesh, Manifest, ManifestPrim, Scene};
use std::collections::HashMap;

/// Two classes on ONE geometry, two instances of it. The geometry has a mesh,
/// a `p_vehicle_3` prim and a `p_collision` prim; both classes carry BF1
/// KAS2 `kas2_bldg_hut_round`'s real property, `VehicleCollisionOnly =
/// p_vehicle_3`, so that prim is out of the SOLDIER subset and the other two
/// bodies are in it (game::collision_layers).
/// A unit quad at y = 0 — enough triangles for a real TriMesh.
fn mesh_quad() -> CpuMesh {
    CpuMesh {
        positions: vec![
            [-1.0, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
        ],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        uvs: vec![[0.0, 0.0]; 4],
        colors: vec![[255; 4]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        submeshes: vec![],
        spec: Vec::new(),
        light: Vec::new(),
        scroll: Vec::new(),
        sphere: None,
        vis_top: None,
    }
}

/// A 1x2x1 box prim, 2 m above its instance's origin.
fn prim_box(name: &str) -> ManifestPrim {
    ManifestPrim {
        name: name.to_owned(),
        parent: "root".to_owned(),
        kind: "box".to_owned(),
        dims: [1.0, 2.0, 1.0],
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0, 2.0, 0.0],
        mask: 0,
        mask_own: false,
    }
}

fn scene() -> Scene {
    let manifest: Manifest = ron::from_str(
        r#"(
            map: "t",
            source_game: "bf1",
            terrain_name: None,
            sky_name: None,
            terrain_glb: None,
            models: [],
            textures: [],
            classes: [
                (name: "hut_a", base: "prop", geometry: Some("Geo_Hut"),
                 collision: [("vehicle_only", "p_vehicle_3")]),
                (name: "hut_b", base: "prop", geometry: Some("geo_hut"),
                 collision: [("vehicle_only", "p_vehicle_3")]),
            ],
            geometries: [],
            regions: [],
            instances: [
                (class: "hut_a", name: "i0",
                 rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,0,0), props: []),
                (class: "HUT_B", name: "i1",
                 rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (100,0,0), props: []),
            ],
            terrain_cutters: [(sphere: (0,0,0,0), planes: [(0,1,0,-1)])],
        )"#,
    )
    .expect("manifest parses");

    let mut collision = HashMap::new();
    collision.insert(
        "geo_hut".to_owned(),
        CollisionGeom {
            mesh: Some(mesh_quad()),
            prims: vec![prim_box("p_vehicle_3"), prim_box("p_collision")],
        },
    );
    let cutters = manifest.terrain_cutters.clone();
    Scene {
        dir: std::path::PathBuf::new(),
        manifest,
        meshes: HashMap::new(),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: HashMap::new(),
        collision,
        cutters,
    }
}

#[test]
fn collision_world_follows_instance_order_and_uses_the_soldier_subset() {
    let s = scene();
    let w = s.collision_world();
    // Two instances × (one mesh + one prim). `VehicleCollisionOnly` takes
    // `p_vehicle_3` out of the soldier subset.
    assert_eq!(w.bodies().len(), 2 * 2, "mesh + one prim per instance");
    assert!(
        matches!(w.bodies()[0].shape, core::collision::Shape::Mesh(_)),
        "bodies[0] is instance 0's mesh"
    );
    assert!(matches!(
        w.bodies()[1].shape,
        core::collision::Shape::Box { .. }
    ));
    // Instance order: instance 0 sits at the origin, instance 1 at x = 100.
    // Each body knows its instance (card TURR: a turret's camera ray passes
    // its own building's).
    let of: Vec<Option<u32>> = w.bodies().iter().map(|b| b.instance).collect();
    assert_eq!(of, [Some(0), Some(0), Some(1), Some(1)]);
    assert_eq!(w.bodies()[0].to_world.w_axis.truncate(), glam::Vec3::ZERO);
    assert_eq!(
        w.bodies()[2].to_world.w_axis.truncate(),
        glam::Vec3::new(100.0, 0.0, 0.0)
    );
    // Instance 1's prim carries the instance transform AND the prim's own.
    assert_eq!(
        w.bodies()[3].to_world.w_axis.truncate(),
        glam::Vec3::new(100.0, 2.0, 0.0)
    );
    // The mesh is built ONCE and shared: both bodies point at the same TriMesh.
    match (&w.bodies()[0].shape, &w.bodies()[2].shape) {
        (core::collision::Shape::Mesh(a), core::collision::Shape::Mesh(b)) => {
            assert!(std::sync::Arc::ptr_eq(a, b), "one TriMesh per geometry")
        }
        _ => panic!("both are meshes"),
    }
    // Cutters come straight off the manifest.
    assert_eq!(w.cutters.len(), 1);
    assert!(w.is_cut(glam::Vec3::new(0.0, 0.5, 0.0)));
    assert!(!w.is_cut(glam::Vec3::new(0.0, 2.0, 0.0)));
}

/// The NAME rule is gone: with NO collision property the class keeps every
/// body, `p_vehicle_3` included — which is what the PS2 does
/// (analysis/re-collision-layers.md §0.3) and the opposite of what the
/// session-7 rule did.
#[test]
fn a_class_without_collision_props_keeps_the_vehicle_prim() {
    let mut s = scene();
    for c in &mut s.manifest.classes {
        c.collision.clear();
    }
    let w = s.collision_world();
    assert_eq!(w.bodies().len(), 2 * 3, "mesh + BOTH prims per instance");
}

/// D3/D4: the world is the UNION of the two subsets, each body carries the
/// mask of the subsets it is in, and the SOLDIER-masked subsequence is
/// byte-for-byte the list a soldier-only build would produce — the property
/// that makes the soldier world bit-identical across this change.
///
/// Two classes on two geometries, both of which the discs really author:
/// class A is BF2 common's `com_inv_col_16` shape (`OrdnanceCollision = none`
/// — soldier-only, bolt-transparent) and class B is `nab2_bldg_b_19`'s
/// (`SoldierCollision = collisionmesh` — the mesh in both, the prim ordnance-
/// only). analysis/re-collision-layers.md §2, §8.
#[test]
fn the_world_is_the_union_with_masks_and_the_soldier_subsequence_is_unchanged() {
    use core::collision::{Layers, Shape};
    let mut s = scene();
    // Geometry B: the same body shape as `geo_hut`, under its own name.
    let b_geom = CollisionGeom {
        mesh: Some(mesh_quad()),
        prims: vec![prim_box("p_cube")],
    };
    s.collision.insert("geo_b".to_owned(), b_geom);
    s.manifest.classes = ron::from_str(
        r#"[
            (name: "a", base: "prop", geometry: Some("geo_hut"),
             collision: [("ordnance", "none")]),
            (name: "b", base: "prop", geometry: Some("geo_b"),
             collision: [("soldier", "collisionmesh")]),
        ]"#,
    )
    .expect("classes parse");
    s.manifest.instances = ron::from_str(
        r#"[
            (class: "a", name: "i0",
             rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,0,0), props: []),
            (class: "b", name: "i1",
             rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (100,0,0), props: []),
        ]"#,
    )
    .expect("instances parse");
    // Geometry `geo_hut` has TWO prims; class "a" names none of them, so all
    // three of its bodies are soldier-only.
    let w = s.collision_world();
    let got: Vec<(Layers, bool)> = w
        .bodies()
        .iter()
        .map(|b| (b.layers, matches!(b.shape, Shape::Mesh(_))))
        .collect();
    assert_eq!(
        got,
        vec![
            (Layers::SOLDIER, true),   // A.mesh
            (Layers::SOLDIER, false),  // A.p_vehicle_3
            (Layers::SOLDIER, false),  // A.p_collision
            (Layers::ALL, true),       // B.mesh  (named by SoldierCollision)
            (Layers::ORDNANCE, false), // B.p_cube (ordnance keeps the default)
        ],
        "the union, in instance order, with the prim order inside an instance"
    );

    // The SOLDIER subsequence == a build that only ever knew Layer::Soldier:
    // same bodies, same order, same placement and shape kind.
    let soldier: Vec<(glam::Vec3, bool)> = w
        .bodies()
        .iter()
        .filter(|b| b.layers.contains(Layers::SOLDIER))
        .map(|b| {
            (
                b.to_world.w_axis.truncate(),
                matches!(b.shape, Shape::Mesh(_)),
            )
        })
        .collect();
    assert_eq!(
        soldier,
        vec![
            (glam::Vec3::ZERO, true),
            (glam::Vec3::new(0.0, 2.0, 0.0), false),
            (glam::Vec3::new(0.0, 2.0, 0.0), false),
            (glam::Vec3::new(100.0, 0.0, 0.0), true),
        ]
    );
    // ... and nothing outside the soldier subset answers a soldier query: the
    // ordnance-only box sits at (100, 2, 0) and the ray THROUGH it is a hit
    // for the bolt and a miss for the soldier.
    let from = glam::Vec3::new(100.0, 2.0, 20.0);
    assert!(w
        .ray(from, glam::Vec3::NEG_Z, 100.0, Layers::ORDNANCE)
        .is_some());
    assert!(w
        .ray(from, glam::Vec3::NEG_Z, 100.0, Layers::SOLDIER)
        .is_none());
}

/// A door class for the fixture: `hut_a` becomes `door`-based, with no
/// collision property (every body in every subset, as the disc's doors), a
/// 4 m trigger at model (0, 0, −1.1) and a 31-frame clip that lifts both
/// prims 3 m, linearly (the mesh has no bone: it stays).
fn door_scene() -> Scene {
    let mut s = scene();
    let c = &mut s.manifest.classes[0];
    c.base = "door".to_owned();
    c.collision.clear();
    c.door = Some(game::assets::ManifestDoor {
        triggers: vec![game::assets::ManifestDoorTrigger {
            hardpoint: "hp_door".to_owned(),
            radius: 4.0,
            position: [0.0, 0.0, -1.1],
        }],
        frames: 31,
        prims: ["p_vehicle_3", "P_COLLISION"]
            .iter()
            .map(|n| game::assets::ManifestDoorPrim {
                name: (*n).to_owned(),
                motion: (0..31)
                    .map(|f| ([0.0, 0.0, 0.0, 1.0], [0.0, 3.0 * f as f32 / 30.0, 0.0]))
                    .collect(),
            })
            .collect(),
        parts: Vec::new(),
        open_sound: Some("door_open".to_owned()),
        close_sound: None,
    });
    s
}

/// DR2 (`analysis/re-bf2-doors.md` §2, §3): a door is built SHUT and in
/// every subset its class names; each prim its clip moves becomes a door
/// body — by name, case-insensitively — posed per frame as
/// `instance · D(f) · XFRM`, with the trigger in world space; nothing else
/// about the world changes.
#[test]
fn a_door_is_built_shut_with_its_moving_prim_and_its_trigger() {
    use core::collision::Layers;
    let s = door_scene();
    let (w, doors) = s.collision_and_doors_with(true);
    // Instance 0 (the door): mesh, p_vehicle_3, p_collision — all in every
    // subset; instance 1 (hut_b) as before.
    let got: Vec<Layers> = w.bodies().iter().map(|b| b.layers).collect();
    assert_eq!(
        got,
        vec![
            Layers::ALL,
            Layers::ALL,
            Layers::ALL,
            Layers::ALL,
            Layers::ALL
        ]
    );
    assert_eq!(doors.len(), 1);
    let d = &doors[0];
    assert_eq!(d.open, 0.0);
    assert_eq!(d.triggers[0].center, glam::Vec3::new(0.0, 0.0, -1.1));
    assert_eq!(d.triggers[0].radius, 4.0);
    // DRV: the door knows its instance, where it stands and what it sounds.
    assert_eq!(d.instance, 0);
    assert_eq!(d.origin, s.manifest.instances[0].translation.into());
    assert_eq!(d.open_sound.as_deref(), Some("door_open"));
    assert_eq!(d.close_sound, None);
    let ids: Vec<usize> = d.bodies.iter().map(|b| b.body).collect();
    assert_eq!(ids, vec![1, 2], "mesh 0, then p_vehicle_3 and p_collision");
    let shut = d.bodies[1].frames[0].1;
    let open = d.bodies[1].frames[30].1;
    assert!((shut - glam::Vec3::new(0.0, 2.0, 0.0)).length() < 1e-6);
    assert!((open - glam::Vec3::new(0.0, 5.0, 0.0)).length() < 1e-5);
    // No door entity: the same bodies, bit for bit.
    let (plain, none) = s.collision_and_doors_with(false);
    assert!(none.is_empty());
    let masks = |w: &core::collision::CollisionWorld| {
        w.bodies()
            .iter()
            .map(|b| (b.layers, b.to_world))
            .collect::<Vec<_>>()
    };
    assert_eq!(masks(&w), masks(&plain));
}

/// DR2: the door's box leaves the doorway when the door opens — a soldier
/// ray that stopped on it goes through — and blocks again when it shuts.
#[test]
fn an_open_door_lets_a_soldier_through_and_a_shut_one_does_not() {
    use core::collision::Layers;
    let s = door_scene();
    let (mut w, mut doors) = s.collision_and_doors_with(true);
    let d = &mut doors[0];
    // Both boxes span y 0..4 at (0, 2, 0); the mesh quad lies flat at y 0.
    // A soldier ray at chest height (y 2) toward the doorway:
    let from = glam::Vec3::new(0.0, 2.0, 20.0);
    let blocked = |w: &core::collision::CollisionWorld| {
        w.ray(from, glam::Vec3::NEG_Z, 100.0, Layers::SOLDIER)
            .is_some()
    };
    assert!(blocked(&w), "shut");
    let step = |d: &mut core::door::Door, w: &mut core::collision::CollisionWorld, on: bool| {
        for _ in 0..30 {
            d.advance(on, 1.0 / 30.0);
            for k in 0..d.bodies.len() {
                w.move_body(d.bodies[k].body, d.pose(k));
            }
        }
    };
    step(d, &mut w, true);
    assert_eq!(d.open, 1.0);
    assert!(!blocked(&w), "open: the boxes rode the clip up 3 m");
    step(d, &mut w, false);
    assert_eq!(d.open, 0.0);
    assert!(blocked(&w), "shut again");
}

#[test]
fn resolve_geometry_is_case_insensitive() {
    let s = scene();
    assert_eq!(s.resolve_geometry("hut_a"), Some("Geo_Hut"));
    assert_eq!(s.resolve_geometry("HUT_A"), Some("Geo_Hut"));
    assert_eq!(s.resolve_geometry("Hut_B"), Some("geo_hut"));
    assert_eq!(s.resolve_geometry("nope"), None);
}
