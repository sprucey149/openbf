use super::*;
use core::unit::SpawnPost;
use core::vehicle::walker::WalkerParams;
use core::vehicle::{SeatKind, VehicleHandle, VehicleKind, VehicleSpec};

fn fixture() -> (WalkSim, MapStatic, VehicleHandle, VehicleHandle) {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let own = walk.sim.unit(walk.player).unwrap().team;
    walk.sim.walker_params = vec![
        Some(WalkerParams {
            spawn_points: vec![(Vec3::new(8.0, 0.0, 0.0), 0.0)],
            ..WalkerParams::default()
        }),
        None,
    ];
    let mut spawn = |team, class, pos| {
        walk.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Walker,
            class,
            home_team: Some(team),
            neutral_when_empty: false,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 60000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        })
    };
    let friend = spawn(own, 0, Vec3::new(500.0, 0.0, 200.0));
    let enemy = spawn(1 - own, 0, Vec3::new(-500.0, 0.0, 200.0));
    spawn(1 - own, 1, Vec3::new(-500.0, 0.0, -200.0));
    let icon = ClassIcon {
        texture: Some("atat_icon".into()),
        scale: 3.5,
        view: VIEW_DEFAULT,
    };
    let map = MapStatic {
        bounds: Some(([0.0, 0.0], 1000.0)),
        soldiers: vec![icon.clone(); 2],
        vehicles: vec![icon; 2],
        ..Default::default()
    };
    (walk, map, friend, enemy)
}

#[test]
fn distant_command_walkers_are_posts_on_both_teams_without_duplicate_unit_icons() {
    let (mut walk, map, friend, enemy) = fixture();
    let model = map.model(&walk);
    assert_eq!(model.posts.len(), 2);
    assert!(
        model.units.is_empty(),
        "ordinary undetected vehicle stays hidden"
    );
    assert!(model.posts.iter().any(|p| p.color == FRIEND));
    assert!(model.posts.iter().any(|p| p.color == ENEMY));
    for post in &model.posts {
        assert_eq!(
            (&*post.texture, post.scale, post.number),
            ("atat_icon", 3.5, None)
        );
        assert!(post.heading.is_some());
        assert!(model.place(post.pos, true).is_some(), "post clamps to rim");
        assert!(model.place(post.pos, false).is_none(), "beyond unit range");
    }
    assert!(walk.sim.seat_unit(walk.player, friend, 0));
    assert_eq!(
        map.model(&walk).posts.len(),
        2,
        "own mount still has its post"
    );
    walk.sim.vehicles[enemy.index as usize].health.cur = 0.0;
    assert_eq!(
        map.model(&walk).posts.len(),
        1,
        "dying walkers lose their post"
    );
}

#[test]
fn spawn_ring_tracks_the_friendly_walker_and_enemy_posts_are_not_spawn_choices() {
    let (mut walk, map, friend, enemy) = fixture();
    let team = walk.sim.unit(walk.player).unwrap().team;
    let choices = walk.sim.player_spawn_posts(team);
    assert!(choices.iter().any(|(p, _)| *p == SpawnPost::Walker(friend)));
    assert!(!choices.iter().any(|(p, _)| *p == SpawnPost::Walker(enemy)));
    walk.sim.spawn_post_choice[0] = Some(SpawnPost::Walker(friend));
    let selected = map.spawn_model(&walk, 0);
    let icon = &selected.posts[selected.select.unwrap()];
    assert_eq!(icon.pos, [500.0, 200.0]);
    assert_eq!(icon.color, FRIEND);
    assert_eq!(
        selected.posts.len(),
        2,
        "spawn view uses the same post list"
    );
    walk.sim.vehicles[friend.index as usize].pos.x += 30.0;
    let moved = map.spawn_model(&walk, 0);
    assert_eq!(moved.posts[moved.select.unwrap()].pos, [530.0, 200.0]);
    walk.sim.vehicles[friend.index as usize].alive = false;
    let dead = map.spawn_model(&walk, 0);
    assert!(dead.select.is_none());
    assert_eq!(dead.posts.len(), 1);
}
