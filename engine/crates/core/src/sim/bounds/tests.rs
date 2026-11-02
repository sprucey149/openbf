use super::*;
use crate::collision::{Body, CollisionWorld, Shape};
use crate::nav::{Arc, Hub, NavGraph, RouteTable};
use crate::sim::tests::sphere_cp;
use crate::sim::DeathCause;
use crate::unit::UnitSpec;
use glam::Mat4;

/// Two decks 40 m apart joined by a bridge, tops at y 10 and y 16 (the
/// bridge is a ramp), every hub AUTHORED at y 12 — and a hidden floor at
/// y 2 under all of it. One post on the low deck.
fn decks() -> Sim {
    decks_with(Vec::new())
}

/// [`decks`] with more bodies in its world.
fn decks_with(mut extra: Vec<Body>) -> Sim {
    let slab = |x: f32, y: f32, hx: f32, hz: f32| {
        Body::new(
            Mat4::from_translation(Vec3::new(x, y - 0.25, 0.0)),
            Shape::Box {
                half: Vec3::new(hx, 0.25, hz),
            },
        )
    };
    // The ramp: a slab tilted about Z, rising 6 m over 20 m.
    let rise = (6.0f32 / 20.0).atan();
    let ramp = Body::new(
        Mat4::from_translation(Vec3::new(20.0, 13.0 - 0.25, 0.0))
            * Mat4::from_rotation_z(rise)
            * Mat4::IDENTITY,
        Shape::Box {
            half: Vec3::new(10.6, 0.25, 2.0),
        },
    );
    let mut s = Sim::empty(3);
    let mut bodies = vec![
        slab(0.0, 10.0, 10.0, 10.0),
        slab(40.0, 16.0, 10.0, 10.0),
        ramp,
        slab(20.0, 2.0, 40.0, 20.0),
    ];
    bodies.append(&mut extra);
    s.collision = CollisionWorld::new(bodies, Vec::new());
    let hub = |x: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(x, 12.0, 0.0),
        radius: 6.0,
        arcs,
    };
    s.nav = Some(
        NavGraph::new(
            vec![hub(0.0, vec![0]), hub(40.0, vec![0])],
            vec![Arc {
                a: 0,
                b: 1,
                type_mask: 1,
                flags: 0,
            }],
            RouteTable::empty(2),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph"),
    );
    s.add_cp(
        sphere_cp("cp1", Vec3::new(1.0, 10.0, 1.0), 6.0, false),
        Some(0),
    );
    s
}

/// Card STLVL (BF2 Mygeeto's cp1): a pier at y 4 over the ground, laid
/// as two slabs with a 0.2 m seam between them; hub 0 at its root with
/// a spawn node on it, hub 1 on the balcony at its end — and the post
/// itself on the ground 12 m beyond, with hub 1 its nearest.
fn pier() -> Sim {
    let slab = |y: f32, z0: f32, z1: f32, hx: f32| {
        Body::new(
            Mat4::from_translation(Vec3::new(0.0, y - 0.25, (z0 + z1) * 0.5)),
            Shape::Box {
                half: Vec3::new(hx, 0.25, (z1 - z0) * 0.5),
            },
        )
    };
    let mut s = Sim::empty(3);
    s.collision = CollisionWorld::new(
        vec![
            slab(0.0, -40.0, 60.0, 40.0),
            slab(4.0, 0.0, 10.0, 2.0),
            slab(4.0, 10.2, 20.0, 2.0),
        ],
        Vec::new(),
    );
    let hub = |z: f32, radius: f32| Hub {
        pos: Vec3::new(0.0, 1.0, z),
        radius,
        arcs: vec![0],
    };
    s.nav = Some(
        NavGraph::new(
            vec![hub(2.2, 3.0), hub(18.0, 4.0)],
            vec![Arc {
                a: 0,
                b: 1,
                type_mask: 1,
                flags: 0,
            }],
            RouteTable::empty(2),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph"),
    );
    let mut cp = sphere_cp("cp1", Vec3::new(0.0, 0.0, 30.0), 14.0, false);
    cp.spawn_nodes = vec![(Vec3::new(0.5, 4.0, 2.0), 0.0)];
    s.add_cp(cp, Some(0));
    s
}

/// The balcony's hub is on the pier: the walk from the root crosses the
/// seam (one ray falls through it to the ground 4 m down, the foot does
/// not), and the post on the ground beyond — outside the hub's disc —
/// starts the walk there without pinning it.
#[test]
fn a_seam_is_not_a_drop_and_a_post_beyond_a_hub_does_not_pin_it() {
    let s = pier();
    // The seam is real: the one ray at it finds the ground.
    assert!(s.ground_point_under(Vec3::new(0.0, 5.2, 10.1), true) < 0.1);
    assert!((s.nav_floor(0).expect("hub 0") - 4.0).abs() < 0.05);
    assert!(
        (s.nav_floor(1).expect("hub 1") - 4.0).abs() < 0.05,
        "the balcony: {:?}",
        s.nav_floor(1)
    );
    assert!(s.arc_walkable(0, 1, 1.0), "balcony to root");
    assert!(s.arc_walkable(0, 0, 1.0), "root to balcony");
    // A soldier on the balcony is on its hub's storey; one on the ground
    // under it is not.
    assert!(s.floor_reaches_hub(Vec3::new(1.0, 4.0, 15.0), 1, 1.5));
    assert!(!s.floor_reaches_hub(Vec3::new(1.0, 0.0, 15.0), 1, 1.5));
}

/// The floors are walked from the post: the low deck's hub at its deck,
/// the far hub up the ramp at ITS deck — neither at the authored 12, and
/// neither on the hidden floor that the authored height's own ray finds
/// first under the far hub.
#[test]
fn hub_floors_are_walked_from_the_posts() {
    let s = decks();
    assert!((s.nav_floor(0).expect("hub 0") - 10.0).abs() < 0.05);
    assert!((s.nav_floor(1).expect("hub 1") - 16.0).abs() < 0.05);
    // The authored height's ray lands under the far deck.
    assert!(s.ground_under(Vec3::new(40.0, 12.0, 0.0), false) < 3.0);
    assert_eq!(s.kill_floor, Some(10.0 - KILL_FLOOR_DROP));
}

/// On a deck, on the ramp and a step down a hull are on the level; the
/// hidden floor under a deck or under the bridge is off it.
#[test]
fn the_level_test_tells_the_deck_from_what_is_under_it() {
    let s = decks();
    assert_eq!(s.level_at(Vec3::new(3.0, 10.0, 2.0)), Level::On);
    assert_eq!(
        s.level_at(Vec3::new(20.0, 13.0, 0.0)),
        Level::On,
        "the ramp"
    );
    assert_eq!(
        s.level_at(Vec3::new(9.0, 8.5, 9.0)),
        Level::On,
        "1.5 m down"
    );
    assert!(
        s.level_at(Vec3::new(3.0, 2.0, 2.0)).is_off(),
        "under a deck"
    );
    assert!(
        s.level_at(Vec3::new(20.0, 2.0, 0.0)).is_off(),
        "under the bridge"
    );
    // Open air all the way from the hubs to it: nothing vouches for it.
    assert_eq!(s.level_at(Vec3::new(-25.0, 2.0, 15.0)), Level::Unknown);
    // With no graph nothing is judged.
    let mut bare = Sim::empty(1);
    bare.resolve_nav_floors();
    assert_eq!(bare.level_at(Vec3::ZERO), Level::Unknown);
}

/// A barrier along a deck's rim stops the walk: a hull that falls away
/// past it is off the level a few metres down, where without the
/// barrier the walk would follow it down and call it on.
#[test]
fn a_barrier_at_the_rim_stops_the_walk() {
    // A hull: a long ramp falling west from the low deck's rim (x -10).
    let fall = (6.0f32 / 12.0).atan();
    let hull = Body::new(
        Mat4::from_translation(Vec3::new(-16.0, 7.0 - 0.25, 0.0)) * Mat4::from_rotation_z(fall),
        Shape::Box {
            half: Vec3::new(6.8, 0.25, 10.0),
        },
    );
    let mut s = decks_with(vec![hull]);
    let down = Vec3::new(
        -18.0,
        s.ground_under(Vec3::new(-18.0, 12.0, 0.0), false),
        0.0,
    );
    assert!(down.y < 7.0 && down.y > 5.0, "on the hull: {down}");
    assert_eq!(
        s.level_at(down),
        Level::On,
        "no barrier: the walk follows the hull"
    );
    let g = s.nav.as_mut().expect("graph");
    g.barriers.push(crate::nav::Barrier {
        centre: Vec2::new(-10.0, 0.0),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(0.5, 30.0),
        mask: 1,
        enabled: true,
    });
    assert!(s.level_at(down).is_off(), "the walk stops at the rim");
    assert_eq!(s.level_at(Vec3::new(-7.0, 10.0, 0.0)), Level::On);
    assert_eq!(
        s.level_at(Vec3::new(-11.5, 9.3, 0.0)),
        Level::On,
        "a step past the rail is still on it"
    );
    // Card BOWL: 3 m out on the hull, 1.5 m under the deck, is OFF — the
    // walk the barrier stopped vouches for a kerb, not for a hull — and
    // the census calls it beyond a rim, by that drop.
    let out = Vec3::new(
        -13.0,
        s.ground_under(Vec3::new(-13.0, 12.0, 0.0), false),
        0.0,
    );
    assert!(out.y < 9.0 && out.y > 8.0, "on the hull: {out}");
    assert!(s.level_at(out).is_off(), "the first metres of a hull");
    let drop = s.rim_drop(out).expect("beyond the rim");
    assert!((drop - (10.0 - out.y)).abs() < 0.05, "{drop}");
    assert_eq!(s.rim_drop(Vec3::new(-7.0, 10.0, 0.0)), None, "on the deck");
    // ...but a hole the map's models CUT into a terrain is a trench, not
    // a hull: there the walk's full allowance stands (BF2 Hoth).
    s.heightfield = Some(crate::heightfield::Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    s.collision.cutters.push(crate::collision::Cutter {
        sphere: [0.0; 4],
        planes: vec![
            glam::Vec4::new(1.0, 0.0, 0.0, 10.5),
            glam::Vec4::new(-1.0, 0.0, 0.0, -16.0),
        ],
    });
    assert_eq!(s.level_at(out), Level::On, "in a terrain cut");
    assert_eq!(s.rim_drop(out), None, "a trench is no rim");
    s.collision.cutters.clear();
    assert!(s.level_at(out).is_off(), "the cut made it a trench");
    // A barrier across the DECK (a doorway's, a wall's): what is level
    // with the walk's stop behind it is still on the level.
    let g = s.nav.as_mut().expect("graph");
    g.barriers.push(crate::nav::Barrier {
        centre: Vec2::new(5.0, 0.0),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(0.5, 30.0),
        mask: 1,
        enabled: true,
    });
    assert_eq!(s.level_at(Vec3::new(8.0, 10.0, 3.0)), Level::On, "a room");
}

/// Card RETW: the straight pull of a floor walk. Past a wall's end the
/// line is clear; through the wall it is not, also from a point the
/// body already touches the wall at; a kerb up and a deck's edge down
/// are walks, a dip of more than `LOST_DOWN` under both ends is not.
#[test]
fn a_floor_line_is_a_walk_a_soldier_can_make() {
    let slab = |c: Vec3, half: Vec3| Body::new(Mat4::from_translation(c), Shape::Box { half });
    let mut sim = Sim::empty(1);
    sim.collision = CollisionWorld::new(
        vec![
            slab(Vec3::new(0.0, -0.25, 0.0), Vec3::new(40.0, 0.25, 40.0)),
            // A wall x 2.75..3.25, z -20..4.
            slab(Vec3::new(3.0, 1.5, -8.0), Vec3::new(0.25, 1.5, 12.0)),
            // A kerb 0.4 m high at x -5..-3; two decks (top 1.75) at
            // x -26..-20 and -18..-12 with the ground between them.
            slab(Vec3::new(-4.0, 0.2, 0.0), Vec3::new(1.0, 0.2, 5.0)),
            slab(Vec3::new(-23.0, 1.5, 0.0), Vec3::new(3.0, 0.25, 5.0)),
            slab(Vec3::new(-15.0, 1.5, 0.0), Vec3::new(3.0, 0.25, 5.0)),
        ],
        Vec::new(),
    );
    let at = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    assert!(
        sim.floor_line(at(0.0, 0.0, 6.0), at(8.0, 0.0, 6.0)),
        "past the wall's end"
    );
    assert!(
        !sim.floor_line(at(0.0, 0.0, 0.0), at(8.0, 0.0, 0.0)),
        "through the wall"
    );
    assert!(
        !sim.floor_line(at(2.4, 0.0, 3.0), at(8.0, 0.0, 3.0)),
        "through the wall from a point touching it"
    );
    assert!(
        sim.floor_line(at(0.0, 0.0, 0.0), at(-4.0, 0.4, 0.0)),
        "up the kerb"
    );
    assert!(
        sim.floor_line(at(-14.0, 1.75, 0.0), at(-8.0, 0.0, 0.0)),
        "down off the deck"
    );
    assert!(
        !sim.floor_line(at(-22.0, 1.75, 0.0), at(-14.0, 1.75, 0.0)),
        "not through the dip between the decks"
    );
}

/// Card LVLF: a trench — a floor the map's models cut into the terrain,
/// 2.3 m under the snow, behind the barrier along its edge — is on the
/// level; a pit 6 m down in the same cut and a floor under UNCUT terrain
/// are not.
#[test]
fn a_trench_cut_into_the_terrain_is_on_the_level() {
    let floor = |x: f32, top: f32| {
        Body::new(
            Mat4::from_translation(Vec3::new(x, top - 0.25, 0.0)),
            Shape::Box {
                half: Vec3::new(5.0, 0.25, 10.0),
            },
        )
    };
    let mut s = Sim::empty(3);
    s.collision = CollisionWorld::new(
        vec![
            floor(25.0, -2.3),
            floor(45.0, -6.0),
            floor(-25.0, -2.3),
            floor(35.0, -2.3),
            floor(35.0, 0.5),
        ],
        Vec::new(),
    );
    s.heightfield = Some(crate::heightfield::Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    // The cut: 19.5 < x < 50.5.
    s.collision.cutters.push(crate::collision::Cutter {
        sphere: [0.0; 4],
        planes: vec![
            glam::Vec4::new(-1.0, 0.0, 0.0, 19.5),
            glam::Vec4::new(1.0, 0.0, 0.0, -50.5),
        ],
    });
    let mut g = NavGraph::new(
        vec![Hub {
            pos: Vec3::new(0.0, 3.0, 0.0),
            radius: 6.0,
            arcs: Vec::new(),
        }],
        Vec::new(),
        RouteTable::empty(1),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    // The barrier along the trench's edge.
    g.barriers.push(crate::nav::Barrier {
        centre: Vec2::new(18.0, 0.0),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(0.5, 30.0),
        mask: 1,
        enabled: true,
    });
    s.nav = Some(g);
    s.add_cp(
        sphere_cp("cp1", Vec3::new(1.0, 0.0, 1.0), 6.0, false),
        Some(0),
    );
    assert!(s.nav_floor(0).expect("hub 0").abs() < 0.05);
    assert_eq!(s.level_at(Vec3::new(10.0, 0.0, 3.0)), Level::On, "snow");
    let trench = Vec3::new(25.0, -2.3, 0.0);
    assert!((s.ground_under(trench, false) + 2.3).abs() < 0.05);
    assert_eq!(s.level_at(trench), Level::On, "the trench's floor");
    assert!(s.level_at(Vec3::new(45.0, -6.0, 0.0)).is_off(), "a pit");
    // ...and neither is the same depth under a roof (BF2 Polis Massa's
    // slab under its rooms): the pit's floor, lifted to 2.3 m, under a
    // deck at the snow's height.
    assert!(
        s.level_at(Vec3::new(35.0, -2.3, 0.0)).is_off(),
        "a roofed floor in the cut"
    );
    // Without the cut the same floor is 2.3 m under the walk's stop.
    s.collision.cutters.clear();
    assert!(s.level_at(trench).is_off(), "the cut made it a trench");
}

/// Card LVLF: a spawn node authored under its floor and OVER another one
/// (BF2 Utapau's, 1 m under the landing and 75 m over the sinkhole)
/// spawns its unit on its own floor, not on the one under the map.
#[test]
fn a_spawn_node_under_its_floor_spawns_on_it() {
    let s = decks();
    let node = Vec3::new(3.0, 8.5, 2.0);
    assert!(s.ground_under(node, false) < 3.0, "the hidden floor");
    assert!((s.spawn_ground(node) - 10.0).abs() < 0.05, "the deck");
    // A node on its floor, and one on a floor nothing calls off, stay.
    assert!((s.spawn_ground(Vec3::new(3.0, 10.2, 2.0)) - 10.0).abs() < 0.05);
    let low = Vec3::new(-18.0, 2.3, 15.0);
    assert_eq!(s.level_at(Vec3::new(-18.0, 2.0, 15.0)), Level::Unknown);
    assert!((s.spawn_ground(low) - 2.0).abs() < 0.05);
}

/// A hard landing under a deck is a landing under the map; the same
/// landing in the open, a soft one under the deck and one on the deck
/// are not.
#[test]
fn a_hard_landing_under_a_floor_is_under_the_map() {
    let s = decks();
    // An 8.5 m fall: the deck he fell from is 8 m over the hidden floor.
    let hard = -17.5;
    assert!(s.lands_under_the_map(Vec3::new(3.0, 2.0, 2.0), hard));
    assert!(
        !s.lands_under_the_map(Vec3::new(3.0, 2.0, 2.0), -6.0),
        "soft"
    );
    assert!(
        !s.lands_under_the_map(Vec3::new(-25.0, 2.0, 15.0), hard),
        "nothing overhead"
    );
    assert!(
        !s.lands_under_the_map(Vec3::new(3.0, 10.0, 2.0), hard),
        "on the deck"
    );
}

/// The kill floor and the landing under the map kill a player through the
/// invincibility cheat, as a death region does; a lethal landing in the
/// open still spares him.
#[test]
fn the_kill_floor_kills_through_the_cheat() {
    let mut s = decks();
    s.cheats.invincible = true;
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(-30.0, 10.0, 0.0)))
        .expect("player");
    // Nothing under him out there: he falls past the kill floor.
    for _ in 0..200 {
        s.step(&[crate::sim::PadState::default()]);
        if s.unit(p).is_none() {
            break;
        }
    }
    assert!(s.unit(p).is_none(), "the kill floor took him");
    assert!(s.events.iter().any(|e| matches!(
        e,
        crate::sim::Event::EnvDeath {
            cause: DeathCause::KillFloor,
            ..
        }
    )));
}

/// A hover tank driven off BF2 Mygeeto's deck fell for good (census `drive`:
/// y -1156 and still going), alive, so its pad never made another. Out there
/// the fall ends at the kill floor.
#[test]
fn a_craft_past_the_kill_floor_is_destroyed() {
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let mut s = decks();
    let floor = s.kill_floor.expect("a kill floor");
    let h = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(-30.0, floor + 2.0, 0.0),
        yaw: 0.0,
        max_health: 500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    assert!(s.vehicle(h).is_some(), "it starts above the floor");
    s.vehicle_mut(h).expect("the craft").pos.y = floor - 1.0;
    s.step(&[]);
    assert!(s.vehicle(h).is_none(), "the kill floor took the craft");
    assert!(s
        .events
        .iter()
        .any(|e| matches!(e, crate::sim::Event::VehicleDestroyed { .. })));
}

/// Start checks belong to one resolution: enclosing a previously open hub
/// makes it a pen, which must not be mistaken for a walled outgoing arc.
#[test]
fn arc_start_checks_are_recomputed_after_collision_changes() {
    let mut s = Sim::empty(3);
    s.collision = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)),
            Shape::Box {
                half: Vec3::new(40.0, 0.25, 40.0),
            },
        )],
        Vec::new(),
    );
    s.nav = NavGraph::new(
        [Vec3::ZERO, Vec3::X * 8.0, Vec3::Z * 8.0]
            .into_iter()
            .enumerate()
            .map(|(i, pos)| Hub {
                pos,
                radius: 0.5,
                arcs: if i == 0 {
                    vec![0, 1]
                } else {
                    vec![(i - 1) as u16]
                },
            })
            .collect(),
        [1, 2]
            .map(|b| Arc {
                a: 0,
                b,
                type_mask: 1,
                flags: 0,
            })
            .to_vec(),
        RouteTable::empty(3),
        Vec::new(),
        Vec::new(),
    );
    s.add_cp(sphere_cp("cp1", Vec3::ZERO, 1.0, false), Some(0));
    assert_eq!(s.nav_unwalkable, [0, 0]);
    assert_eq!(s.nav_floors, [0.0, 0.0, 0.0]);
    assert_eq!(
        s.floor_reach(Vec3::ZERO, super::super::spawn::PEN_CELLS),
        200
    );
    for (centre, half) in [
        (Vec3::new(3.0, 2.0, 0.0), Vec3::new(0.25, 2.0, 3.25)),
        (Vec3::new(-3.0, 2.0, 0.0), Vec3::new(0.25, 2.0, 3.25)),
        (Vec3::new(0.0, 2.0, 3.0), Vec3::new(3.25, 2.0, 0.25)),
        (Vec3::new(0.0, 2.0, -3.0), Vec3::new(3.25, 2.0, 0.25)),
    ] {
        s.collision.push_body(Body::new(
            Mat4::from_translation(centre),
            Shape::Box { half },
        ));
    }
    assert!(s.soldier_clear(Vec3::ZERO));
    assert!(s.floor_reach(Vec3::ZERO, super::super::spawn::PEN_CELLS) < 200);
    s.resolve_nav_floors();
    assert_eq!(s.nav_unwalkable, [0, 0], "a pen is not judged walled");
}

/// A hub's start check is its own: the first hub's answer is not lent to the next.
#[test]
fn arc_start_checks_belong_to_the_hub_they_leave() {
    let pen = Vec3::X * 8.0;
    let mut bodies = vec![Body::new(
        Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        Shape::Box {
            half: Vec3::new(40.0, 0.25, 40.0),
        },
    )];
    // Hub 1 stands in this pen.
    for (centre, half) in [
        (Vec3::new(3.0, 2.0, 0.0), Vec3::new(0.25, 2.0, 3.25)),
        (Vec3::new(-3.0, 2.0, 0.0), Vec3::new(0.25, 2.0, 3.25)),
        (Vec3::new(0.0, 2.0, 3.0), Vec3::new(3.25, 2.0, 0.25)),
        (Vec3::new(0.0, 2.0, -3.0), Vec3::new(3.25, 2.0, 0.25)),
    ] {
        bodies.push(Body::new(
            Mat4::from_translation(pen + centre),
            Shape::Box { half },
        ));
    }
    let mut s = Sim::empty(3);
    s.collision = CollisionWorld::new(bodies, Vec::new());
    s.nav = NavGraph::new(
        [Vec3::ZERO, pen]
            .into_iter()
            .map(|pos| Hub {
                pos,
                radius: 0.5,
                arcs: vec![0],
            })
            .collect(),
        vec![Arc {
            a: 0,
            b: 1,
            type_mask: 1,
            flags: 0,
        }],
        RouteTable::empty(2),
        Vec::new(),
        Vec::new(),
    );
    s.nav_floors = vec![0.0, 0.0];
    let mut starts = vec![None; 2];
    s.arc_walled(0, 0, &mut starts);
    assert_eq!(starts, [Some(true), None]);
    assert!(!s.arc_walled(0, 1, &mut starts), "a pen is not judged");
    assert_eq!(starts, [Some(true), Some(false)]);
}

/// A map's posts walk the level once, after the last: what stands on them is
/// what a walk after every post leaves, floors, spawn seats and flanks alike.
#[test]
fn walking_the_level_once_after_the_last_post_changes_nothing() {
    let posts = || {
        [
            sphere_cp("cp2", Vec3::new(41.0, 16.0, 1.0), 6.0, false),
            sphere_cp("cp3", Vec3::new(20.0, 13.0, 0.0), 6.0, false),
        ]
    };
    let derived = |s: &Sim| {
        format!(
            "{:?} {:?} {:?} {:?} {:?} {:?}",
            s.nav_floors, s.nav_unwalkable, s.nav_trapped, s.kill_floor, s.spawn_seats, s.flank
        )
    };
    let mut each = decks();
    for p in posts() {
        each.add_cp(p, Some(1));
    }
    let mut once = decks();
    for p in posts() {
        once.add_cp_unresolved(p, Some(1));
    }
    once.resolve_posts();
    assert_eq!(derived(&each), derived(&once));
    once.resolve_posts();
    assert_eq!(derived(&each), derived(&once), "the walk starts over");
}
