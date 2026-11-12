//! Live command-walker segment placement and malformed-sphere controls.
use super::*;

const WALKER_CAST_FROM: Vec3 = Vec3::new(0.0, 8.0, 0.0);
const WALKER_CAST_NODE: Vec3 = Vec3::new(8.0, 2.0, 0.0);
const WALKER_CAST_WALL: Vec3 = Vec3::new(3.75, 5.1875, 0.0);
const WALKER_CAST_YAW: f32 = 0.375;

fn walker_cast_fixture(building: bool, sphere: bool) -> (Sim, crate::vehicle::VehicleHandle) {
    use crate::collision::{Body, HullFilter, HullPart, Layers, Shape};
    use crate::vehicle::VehicleHandle;
    use glam::Mat4;

    let mut s = command_walker_sim(Vec3::ZERO, 0.0);
    s.step(&[]);
    let (walker, _) = s.command_walkers(0).next().expect("live command walker");
    let v = s.vehicle_mut(walker).unwrap();
    v.pos = Vec3::ZERO;
    v.rot = glam::Quat::IDENTITY;
    s.walker_params[0].as_mut().unwrap().spawn_points = vec![(WALKER_CAST_NODE, WALKER_CAST_YAW)];
    s.walker_params[0].as_mut().unwrap().spawn_sphere_centre = sphere.then_some(WALKER_CAST_FROM);

    // Two available floors at the endpoint. The old soldier-floor query
    // seats the node on the box at y=1 instead of leaving it at y=2.
    s.collision
        .push_body(wall(Vec3::new(8.0, 0.5, 0.0), Vec3::new(0.75, 0.5, 0.75)));
    assert_eq!(s.spawn_ground(WALKER_CAST_NODE), 1.0);

    // Each decoy would be a strictly earlier segment hit under the wrong
    // category or layer. The port's synthetic hulls carry distinct handles.
    let shape = Shape::Box {
        half: Vec3::new(0.25, 0.75, 0.75),
    };
    s.collision.push_body(Body::with_layers(
        Mat4::from_translation(Vec3::new(1.0, 7.25, 0.0)),
        shape.clone(),
        Layers::SOLDIER,
    ));
    let part = HullPart::new(Mat4::IDENTITY, shape.clone(), Layers::ORDNANCE);
    let other = VehicleHandle {
        index: walker.index + 1,
        gen: walker.gen,
    };
    s.collision.set_hull(
        walker,
        std::slice::from_ref(&part),
        Mat4::from_translation(Vec3::new(2.0, 6.5, 0.0)),
    );
    s.collision.set_hull(
        other,
        &[part],
        Mat4::from_translation(Vec3::new(3.0, 5.75, 0.0)),
    );
    if building {
        s.collision.push_body(Body::with_layers(
            Mat4::from_translation(Vec3::new(4.0, 5.0, 0.0)),
            shape,
            Layers::ORDNANCE,
        ));
    }

    let ray = |layer, hulls| {
        s.collision.ray_with(
            WALKER_CAST_FROM,
            (WALKER_CAST_NODE - WALKER_CAST_FROM) / 10.0,
            10.0,
            layer,
            hulls,
        )
    };
    let soldier_decoy = ray(Layers::SOLDIER, HullFilter::Static).unwrap();
    assert!((soldier_decoy.t - 0.9375).abs() < 1e-5);
    assert_eq!(
        ray(Layers::ORDNANCE, HullFilter::All).unwrap().owner,
        Some(walker)
    );
    assert_eq!(
        ray(Layers::ORDNANCE, HullFilter::except(Some(walker)))
            .unwrap()
            .owner,
        Some(other)
    );
    let world_hit = ray(Layers::ORDNANCE, HullFilter::Static);
    if building {
        assert!((world_hit.unwrap().t - 4.6875).abs() < 1e-5);
    } else {
        assert!(world_hit.is_none());
    }
    assert!(crate::collision::ray_terrain_ps2(
        s.heightfield.as_ref().unwrap(),
        &s.collision.cutters,
        WALKER_CAST_FROM,
        WALKER_CAST_NODE,
    )
    .is_none());
    (s, walker)
}

fn respawn_at_walker_fixture(s: &mut Sim, spec: UnitSpec, selected: Option<SpawnPost>) -> Vec3 {
    let h = s.spawn(&spec).expect("initial unit");
    let i = h.index as usize;
    if let Some(slot) = spec.player_slot {
        s.sync_slot_tables();
        s.spawn_post_choice[usize::from(slot)] = selected;
    } else {
        s.units[i].spawn_post = selected;
    }
    s.units[i].alive = false;
    let mut expected_rng = s.lcg.clone();
    expected_rng.index(1);
    s.respawn_slot(i);
    let next = &s.units[i];
    assert!(next.alive);
    assert_eq!(next.gen, h.gen + 1);
    assert_eq!(s.lcg.state(), expected_rng.state());
    if selected != Some(SpawnPost::Post(0)) {
        assert_eq!(next.soldier.yaw, WALKER_CAST_YAW);
    }
    next.soldier.pos
}

#[test]
fn command_walker_bot_respawn_keeps_the_oblique_building_hit() {
    let (mut s, walker) = walker_cast_fixture(true, true);
    let mut ai = UnitSpec::trooper(0, Vec3::new(-40.0, 0.0, 0.0));
    ai.controller = Controller::Ai;
    let actual = respawn_at_walker_fixture(&mut s, ai, Some(SpawnPost::Walker(walker)));
    assert!(actual.distance(WALKER_CAST_WALL) < 1e-5, "{actual:?}");
}

#[test]
fn selected_command_walker_player_respawn_keeps_the_oblique_building_hit() {
    let (mut s, walker) = walker_cast_fixture(true, true);
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    let actual = respawn_at_walker_fixture(&mut s, player, Some(SpawnPost::Walker(walker)));
    assert!(actual.distance(WALKER_CAST_WALL) < 1e-5, "{actual:?}");
}

#[test]
fn unselected_player_command_walker_respawn_keeps_the_oblique_building_hit() {
    let (mut s, _) = walker_cast_fixture(true, true);
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    let actual = respawn_at_walker_fixture(&mut s, player, None);
    assert!(actual.distance(WALKER_CAST_WALL) < 1e-5, "{actual:?}");
}

#[test]
fn command_walker_respawn_without_world_hit_keeps_the_authored_endpoint() {
    let (mut s, _) = walker_cast_fixture(false, true);
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    assert_eq!(
        respawn_at_walker_fixture(&mut s, player, None),
        WALKER_CAST_NODE
    );
}

#[test]
fn command_walker_with_zero_length_cast_keeps_the_authored_endpoint() {
    let (mut s, walker) = walker_cast_fixture(false, true);
    s.walker_params[0].as_mut().unwrap().spawn_sphere_centre = Some(WALKER_CAST_NODE);
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    assert_eq!(
        respawn_at_walker_fixture(&mut s, player, Some(SpawnPost::Walker(walker))),
        WALKER_CAST_NODE,
    );
}

#[test]
fn command_walker_respawn_keeps_the_earlier_terrain_hit_over_a_building() {
    use crate::collision::{Body, HullFilter, Layers, Shape};
    use glam::Mat4;

    let (mut s, walker) = walker_cast_fixture(false, true);
    let node = Vec3::new(8.0, -2.0, 0.0);
    s.walker_params[0].as_mut().unwrap().spawn_points = vec![(node, WALKER_CAST_YAW)];
    s.collision.push_body(Body::with_layers(
        Mat4::from_translation(Vec3::new(7.25, -1.125, 0.0)),
        Shape::Box {
            half: Vec3::new(0.25, 1.0, 0.75),
        },
        Layers::ORDNANCE,
    ));
    let delta = node - WALKER_CAST_FROM;
    let length = delta.length();
    let terrain = crate::collision::ray_terrain_ps2(
        s.heightfield.as_ref().unwrap(),
        &s.collision.cutters,
        WALKER_CAST_FROM,
        node,
    )
    .expect("oblique terrain hit");
    let building = s
        .collision
        .ray_with(
            WALKER_CAST_FROM,
            delta / length,
            length,
            Layers::ORDNANCE,
            HullFilter::Static,
        )
        .expect("building beyond the terrain");
    assert!((terrain - 0.8).abs() < 0.005);
    assert!((building.t / length - 0.875).abs() < 1e-5);
    let expected = WALKER_CAST_FROM + delta * terrain;
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    let actual = respawn_at_walker_fixture(&mut s, player, Some(SpawnPost::Walker(walker)));
    assert!(actual.distance(expected) < 1e-5, "{actual:?}");
}

#[test]
fn command_walker_with_invalid_sphere_keeps_the_existing_floor() {
    let centres = [
        Vec3::new(f32::NAN, 8.0, 0.0),
        Vec3::new(f32::INFINITY, 8.0, 0.0),
        Vec3::new(1.0e20, 8.0, 0.0),
        Vec3::new(f32::MAX, 8.0, 0.0),
    ];
    let placements = centres.map(|centre| {
        let (mut s, walker) = walker_cast_fixture(false, true);
        s.walker_params[0].as_mut().unwrap().spawn_sphere_centre = Some(centre);
        // Keep the pre-guard failure observable without sending a NaN or
        // an enormous segment to the recursive terrain query.
        s.heightfield = None;
        s.collision = crate::collision::CollisionWorld::new(Vec::new(), Vec::new());
        s.spawn_seats.push((WALKER_CAST_NODE, 1.0));
        let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
        respawn_at_walker_fixture(&mut s, player, Some(SpawnPost::Walker(walker)))
    });
    assert_eq!(placements, [Vec3::new(8.0, 1.0, 0.0); 4]);
}

#[test]
fn command_walker_with_map_scale_exceeding_cast_keeps_the_existing_floor() {
    let (mut s, walker) = walker_cast_fixture(false, true);
    // A vertical segment takes the direct terrain branch on the pre-guard
    // source. Its length is finite but greatly exceeds the terrain span.
    s.walker_params[0].as_mut().unwrap().spawn_sphere_centre = Some(Vec3::new(8.0, 1.0e12, 0.0));
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    assert_eq!(
        respawn_at_walker_fixture(&mut s, player, Some(SpawnPost::Walker(walker))),
        Vec3::new(8.0, 1.0, 0.0),
    );
}

#[test]
fn command_walker_without_sphere_and_fixed_post_keep_the_existing_floor() {
    let (mut missing_sphere, _) = walker_cast_fixture(true, false);
    let player = UnitSpec::player(0, Vec3::new(-40.0, 0.0, 0.0));
    let floor = Vec3::new(8.0, 1.0, 0.0);
    assert_eq!(
        respawn_at_walker_fixture(&mut missing_sphere, player.clone(), None),
        floor
    );
    let (mut fixed_post, _) = walker_cast_fixture(true, true);
    node_post(&mut fixed_post, WALKER_CAST_NODE);
    assert_eq!(
        respawn_at_walker_fixture(&mut fixed_post, player, Some(SpawnPost::Post(0))),
        floor
    );
}
