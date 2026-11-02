//! Issue #72: original class tables, authored Mustafar links, and BF2 HUD.
use core::{sim::Sim, unit::Controller};
use game::roster::{load_classdb, ClassRoster};
use glam::Vec3;

#[test]
fn jet_route_flags_follow_recharge_not_the_jedi_jump() {
    let root = game::assets::baked_root();
    for (game, names) in [
        (
            "bf1",
            [
                "rep_inf_jet_trooper",
                "imp_inf_dark_trooper",
                "rep_inf_clone_trooper",
            ],
        ),
        (
            "bf2",
            [
                "rep_inf_ep3_jettrooper",
                "imp_inf_dark_trooper",
                "all_hero_luke_jedi",
            ],
        ),
    ] {
        let db = load_classdb(&root.join("classes"), game, &["rep", "imp", "all"]).unwrap();
        for (i, name) in names.iter().enumerate() {
            let r = ClassRoster::build(&db, name, 1, None).unwrap();
            assert_eq!(r.jet, i < 2, "{game} {name}");
            assert_eq!(r.jet, r.consts.has_jet());
        }
    }
}

#[test]
fn real_bf2_hud_draws_fuel_then_disables_it() {
    use ui::{
        hud::{feed, HudModel},
        hud_tree::{Hud, Kind},
    };
    let root = game::assets::baked_root();
    let read = |name| {
        ron::from_str::<manifest::hud::HudLayout>(
            &std::fs::read_to_string(root.join("common_bf2/hud").join(name)).unwrap(),
        )
        .unwrap()
    };
    let empty = manifest::hud::HudLayout {
        name: "empty".into(),
        nodes: vec![],
    };
    let mut hud = Hud::new(&read("1playerhud.ron"), &empty);
    let group = hud.find("player1jetpackbar").unwrap();
    let fill = hud.find("player1jetpack_fill").unwrap();
    let mut m = HudModel::default();
    feed(&mut hud, &m, &|_| false);
    m.jet = Some((0.6, 0.24));
    feed(&mut hud, &m, &|_| false);
    hud.advance(0.2);
    assert!(hud.alpha(group) > 0.99);
    assert!(matches!(hud.elems[fill].kind, Kind::BarBitmap));
    m.jet = None;
    feed(&mut hud, &m, &|_| false);
    hud.advance(4.0);
    assert_eq!(hud.alpha(group), 0.0);
    m.jet = Some((0.6, 0.24));
    feed(&mut hud, &m, &|_| false);
    hud.advance(0.2);
    assert!(hud.alpha(group) > 0.99, "same-fuel class restores the bar");
    m.dead = true;
    feed(&mut hud, &m, &|_| false);
    hud.advance(4.0);
    assert_eq!(hud.alpha(group), 0.0);
}

#[test]
fn original_jet_classes_hop_in_both_game_rule_sets() {
    let root = game::assets::baked_root();
    for game in ["bf1", "bf2"] {
        let db = load_classdb(&root.join("classes"), game, &["rep"]).unwrap();
        let name = if game == "bf1" {
            "rep_inf_jet_trooper"
        } else {
            "rep_inf_ep3_jettrooper"
        };
        let roster = ClassRoster::build(&db, name, 1, None).unwrap();
        let run = || {
            let mut sim = Sim::empty(42);
            sim.ground_rules = core::soldier::GroundRules::for_game(game);
            sim.heightfield = Some(core::heightfield::Heightfield {
                size: 2,
                unit: 4000.0,
                offset_x: 2000.0,
                offset_z: 2000.0,
                heights: vec![0.0; 4],
            });
            let class = roster.register(&mut sim, &mut Vec::new());
            let bot = sim
                .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Ai, 0, 0))
                .unwrap();
            sim.spawn(&core::unit::UnitSpec::trooper(
                1,
                Vec3::new(0.0, 0.0, -500.0),
            ))
            .unwrap();
            sim.nav = core::nav::NavGraph::new(
                [Vec3::ZERO, Vec3::new(0.0, 0.0, -500.0)]
                    .into_iter()
                    .map(|pos| core::nav::Hub {
                        pos,
                        radius: 5.0,
                        arcs: vec![0],
                    })
                    .collect(),
                vec![core::nav::Arc {
                    a: 0,
                    b: 1,
                    type_mask: 1,
                    flags: 0,
                }],
                Default::default(),
                vec![],
                vec![],
            );
            let mut air = 0;
            for _ in 0..600 {
                sim.step(&[]);
                air += usize::from(sim.unit(bot).unwrap().soldier.stance.jetting());
            }
            assert!(air > 20, "{game}: {air} jet updates");
            (sim.hash(), air)
        };
        assert_eq!(run(), run());
    }
}

/// Authored nav + collision, with the map's lava and steam and the mission's fly
/// ceiling: a jet bot crosses each jet-only link both ways and lives. Reverse trips
/// finish by walking after their fuel expires. These two far hubs are leaves, so
/// only a bot sent to one flies the link: no seeded conquest round has done it yet.
#[test]
fn mustafar_jet_connections_arrive_alive_through_the_mission_hazards() {
    use core::ai::{AiState, Route};
    let root = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&root.join("bf2_mus1"), "con").unwrap();
    let graph = game::layout::nav_graph(&scene.manifest, &scene.dir).unwrap();
    let db = load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
    let roster = ClassRoster::build(&db, "rep_inf_ep3_jettrooper", 1, None).unwrap();
    let mission = game::mission::MissionDef::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../missions/bf2_mus1_con_cw.ron"),
    )
    .unwrap();
    let mut failures = Vec::new();
    // Connection names are authored identifiers, not vector indices.
    for (connection, a, b) in [(58, 26, 25), (65, 27, 24)] {
        let (arc_index, arc) = graph
            .arcs
            .iter()
            .copied()
            .enumerate()
            .find(|(_, arc)| arc.a == a && arc.b == b)
            .unwrap();
        assert_ne!(arc.flags & core::nav::Arc::JET_JUMP, 0);
        for (from, to) in [(arc.a, arc.b), (arc.b, arc.a)] {
            let mut sim = Sim::empty(42);
            sim.ground_rules = core::soldier::GroundRules::Bf2;
            sim.heightfield = scene.heightfield();
            scene.install_collision(&mut sim);
            scene.install_damage_regions(&mut sim);
            assert!(!sim.damage_regions.is_empty(), "the map's lava");
            sim.fly_ceiling = mission.max_player_fly_height.unwrap_or(sim.fly_ceiling);
            let mut start = graph.hubs[from as usize].pos;
            let mut goal = graph.hubs[to as usize].pos;
            // The hubs' own storey where authored; the far hubs say y=0.
            let floor_at = |sim: &Sim, p: Vec3, top: f32| {
                let probe = if p.y > 0.0 { p.y + 1.0 } else { top };
                sim.ground_under(Vec3::new(p.x, probe, p.z), false)
            };
            let top = start.y.max(goal.y) + 20.0;
            start.y = floor_at(&sim, start, top);
            goal.y = floor_at(&sim, goal, top);
            sim.nav = Some(graph.clone());
            let class = roster.register(&mut sim, &mut Vec::new());
            let bot = sim
                .spawn(&class.spec(0, start, 0.0, Controller::Ai, 0, 0))
                .unwrap();
            let u = &mut sim.units[bot.index as usize];
            u.soldier.fuel = 1.0;
            u.ai = AiState {
                next_think: u64::MAX,
                route: Some(Route {
                    dest_hub: to,
                    next_hub: to,
                    via_arc: Some(arc_index as u16),
                    waypoint: graph.hubs[to as usize].pos,
                    from: glam::Vec2::new(start.x, start.z),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let mut jet_ticks = 0;
            let mut closest = f32::MAX;
            for _ in 0..600 {
                sim.step(&[]);
                let Some(u) = sim.unit(bot) else {
                    break;
                };
                jet_ticks += usize::from(u.soldier.stance.jetting());
                closest = closest.min(core::nav::xz_dist(u.soldier.pos, goal));
                if closest < 3.0 {
                    break;
                }
            }
            let Some(u) = sim.unit(bot) else {
                failures.push(format!("arc {arc_index} {from}->{to}: died over the lava"));
                continue;
            };
            eprintln!("MUS1 Connection{connection} arc {arc_index} {from}->{to}: from {start:?} to {goal:?}, jets {jet_ticks}, nearest {closest}, end {:?}, fuel {}",u.soldier.pos,u.soldier.fuel);
            assert!(jet_ticks > 0, "arc {arc_index} {from}->{to}: never jetted");
            if closest >= 3.0 {
                failures.push(format!(
                    "arc {arc_index} {from}->{to}: stopped {closest}m short"
                ));
            }
            assert!(u.soldier.fuel < 1.0);
            assert!(
                u.soldier.pos.y >= goal.y - 0.01,
                "arrived below the destination floor"
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// The map's own damage regions at the spots measured live on the PS2
/// (`analysis/re-jetpack.md` §16): steam takes 1.667 an update and the lava pit
/// and the sea take everything at once. A soldier beside them takes nothing.
#[test]
fn mustafars_regions_hurt_as_on_the_ps2() {
    use core::unit::UnitSpec;
    let root = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&root.join("bf2_mus1"), "con").unwrap();
    let mut sim = Sim::empty(1);
    scene.install_damage_regions(&mut sim);
    assert_eq!(sim.damage_regions.len(), 9);
    let mut spec = UnitSpec::trooper(0, Vec3::ZERO);
    spec.max_health = 400.0;
    // Feet where the PS2's first hit came, 0.43 m over the box at most.
    for (spot, want) in [
        (Vec3::new(-54.24, 75.9, 111.41), Some(1.0 / 30.0 * 50.0)),
        (Vec3::new(-58.78, 67.9, 152.53), None),
        (Vec3::new(100.0, 20.0, 80.0), None),
        (Vec3::new(-54.24, 76.2, 111.41), Some(0.0)),
    ] {
        let mut sim = Sim::empty(1);
        sim.heightfield = scene.heightfield();
        scene.install_collision(&mut sim);
        scene.install_damage_regions(&mut sim);
        let h = sim.spawn(&spec).unwrap();
        sim.unit_mut(h).unwrap().soldier.pos = spot;
        sim.step(&[]);
        match want {
            Some(lost) => {
                let hp = sim.unit(h).expect("alive").health.cur;
                assert!(
                    (400.0 - hp - lost).abs() < 0.01,
                    "{spot}: lost {}",
                    400.0 - hp
                );
            }
            None => assert!(sim.unit(h).is_none(), "{spot}: instant death"),
        }
    }
}
