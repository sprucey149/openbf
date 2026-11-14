//! Issue #168: the static gameplay digest (`Sim::static_digest`).

use super::*;
use crate::collision::{trimesh, Body, CollisionWorld, Shape};
use crate::heightfield::Heightfield;
use crate::sim::STATIC_DIGEST_GROUPS;
use glam::Mat4;

/// A map-sized fixture: a `n` x `n` heightfield, `meshes` shared meshes of
/// `tris` triangles placed `bodies` times, and `weapons` weapon classes.
fn fixture(n: u32, meshes: usize, tris: usize, bodies: usize, weapons: usize) -> Sim {
    let mut s = Sim::new(42);
    s.heightfield = Some(Heightfield {
        size: n,
        unit: 2.0,
        offset_x: n as f32,
        offset_z: n as f32,
        heights: (0..n * n).map(|i| (i % 97) as f32 * 0.25).collect(),
    });
    let shapes: Vec<_> = (0..meshes)
        .map(|m| {
            let mut pos = Vec::new();
            let mut idx = Vec::new();
            for t in 0..tris {
                let base = pos.len() as u32;
                let x = t as f32 + m as f32 * 0.5;
                pos.extend([[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 1.0, 1.0]]);
                idx.extend([base, base + 1, base + 2]);
            }
            trimesh(&pos, &idx).expect("a mesh")
        })
        .collect();
    let placed = (0..bodies)
        .map(|b| {
            let at = Mat4::from_translation(Vec3::new(b as f32 * 7.0, 0.0, 3.0));
            Body::new(at, Shape::Mesh(shapes[b % meshes].clone()))
        })
        .collect();
    s.collision = CollisionWorld::new(placed, Vec::new());
    for k in 0..weapons {
        let mut w = crate::weapon::WeaponClass::bf2_rifle();
        w.name = format!("rifle{k}");
        s.add_weapon_class(w);
    }
    s
}

fn unit_class(max_health: f32) -> crate::unit::UnitClass {
    crate::unit::UnitClass {
        consts: crate::soldier::SoldierConstants::default(),
        max_health,
        health_type: crate::weapon::HealthType::Person,
        weapon_class: 0,
        total_clips: 6,
        weapon_slot: 0,
        slots: std::sync::Arc::new([]),
        jet: false,
        capture_posts: true,
        drops: crate::sim::DropTable::default(),
        chunk: None,
        min: 1,
        max: None,
        unlock_points: 0,
        droideka: None,
    }
}

/// Two builds of the same fixture agree; a class value, a terrain sample, a
/// mesh vertex or a session setting each move the digest and only their
/// own group.
#[test]
fn the_static_digest_is_stable_and_names_the_group_that_differs() {
    let a = fixture(16, 2, 8, 5, 3);
    let b = fixture(16, 2, 8, 5, 3);
    assert_eq!(a.static_digest(), b.static_digest());
    let groups = a.static_digest_groups();
    assert!(groups.iter().map(|g| g.0).eq(STATIC_DIGEST_GROUPS));
    assert_eq!(groups, b.static_digest_groups());
    // Taking it changes nothing, and the run hash does not see it.
    assert_eq!(a.hash(), b.hash());

    let differs = |s: &Sim| -> Vec<&'static str> {
        s.static_digest_groups()
            .into_iter()
            .zip(&groups)
            .filter(|(x, y)| x.1 != y.1)
            .map(|(x, _)| x.0)
            .collect()
    };
    let mut c = fixture(16, 2, 8, 5, 3);
    c.weapon_classes[1].reload_time += 0.5;
    assert_ne!(c.static_digest(), a.static_digest());
    assert_eq!(differs(&c), ["weapons"]);
    assert_eq!(c.hash(), a.hash(), "a class value is not in the run hash");

    let mut c = fixture(16, 2, 8, 5, 3);
    c.classes[0].push(unit_class(250.0));
    assert_eq!(differs(&c), ["classes"]);

    let mut c = fixture(16, 2, 8, 5, 3);
    c.heightfield.as_mut().unwrap().heights[37] += 0.01;
    assert_eq!(differs(&c), ["terrain"]);

    let mut c = fixture(16, 2, 9, 5, 3);
    assert_eq!(differs(&c), ["collision"], "one more triangle per mesh");
    c.collision = fixture(16, 2, 8, 6, 3).collision;
    assert_eq!(differs(&c), ["collision"], "one more body");

    let mut c = fixture(16, 2, 8, 5, 3);
    c.time_limit = Some(600);
    assert_eq!(differs(&c), ["mission"]);

    let mut c = fixture(16, 2, 8, 5, 3);
    c.team_handicap_on = !c.team_handicap_on;
    assert_eq!(differs(&c), ["session"]);

    let mut c = fixture(16, 2, 8, 5, 3);
    c.water_level = Some(-3.0);
    assert_eq!(differs(&c), ["water"]);
}

/// A map-sized fixture (a 512 x 512 terrain, 40 meshes of 2000 triangles
/// placed 2000 times and 120 weapon classes) digests repeatably. Its cost
/// (4.4 ms, dev profile) is recorded in the #168 card; core reads no clock.
#[test]
fn the_static_digest_of_a_map_sized_fixture_repeats() {
    let s = fixture(512, 40, 2000, 2000, 120);
    assert_eq!(s.static_digest(), s.static_digest());
}

/// Copies of a map that simulate alike agree (issue #168, LAN §7): sound,
/// effect and texture names and the drawn damage effects are not digested;
/// a sim-read value next to them still is, and so is every terrain cell, far
/// past the boundary too (a soldier may walk on past it).
#[test]
fn the_static_digest_leaves_out_what_the_sim_never_reads() {
    let base = |far: f32, near: f32| {
        let mut s = fixture(16, 2, 8, 5, 3);
        let mut heights = vec![1.0; 32 * 32];
        heights[0] = far; // (-1024, -1024): 900 m past the square
        heights[16 * 32 + 16] = near; // (0, 0): inside it
        s.heightfield = Some(Heightfield {
            size: 32,
            unit: 64.0,
            offset_x: 1024.0,
            offset_z: 1024.0,
            heights,
        });
        s.boundary.add_shape(&[
            [-100.0, -100.0],
            [-100.0, 100.0],
            [100.0, 100.0],
            [100.0, -100.0],
        ]);
        s
    };
    let a = base(1.0, 1.0);
    let groups = a.static_digest_groups();
    let differs = |s: &Sim| -> Vec<&'static str> {
        s.static_digest_groups()
            .into_iter()
            .zip(&groups)
            .filter(|(x, y)| x.1 != y.1)
            .map(|(x, _)| x.0)
            .collect()
    };
    let mut c = base(1.0, 1.0);
    c.weapon_classes[1].fire_sound = Some("another_fire".into());
    c.weapon_classes[1].reload_sound = None;
    c.vehicle_damage_effects = vec![Vec::new()];
    assert!(differs(&c).is_empty());
    c.weapon_classes[1].reload_time += 0.5;
    assert_eq!(differs(&c), ["weapons"]);

    assert_eq!(
        differs(&base(9.0, 1.0)),
        ["terrain"],
        "terrain 900 m past the line"
    );
    assert_eq!(differs(&base(1.0, 9.0)), ["terrain"]);
}
