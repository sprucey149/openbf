//! Issue #72: a seeded, headless Mustafar crossing round on the mission scene.
//!
//! Conquest never asks for the two jet-only leaf hubs (re-jetpack.md §16).
//! Test-only objective posts send four friendly jet troopers over both links,
//! starting at opposite ends, then back immediately after landing, with no fuel
//! reset. All nine damage volumes and the mission's death-region list stay active.
//! Only strategic destinations are scripted: thinking, route selection, random
//! hub points, hops, flight, collision and hazards run through WalkSim::step_tick.
//! This is a traversal round, not a conquest victory or a PS2 bot-goal comparison.

use core::{
    commander::Objective,
    cp::CpDef,
    region::Region,
    sim::{soldier_blast_sphere, Event, Sim},
    unit::{Controller, UnitHandle},
};
use game::{
    assets::Scene,
    mission::MissionDef,
    roster::{load_classdb, ClassRoster},
    walk::{TeamTemplate, WalkSim},
};
use glam::Vec3;

// Regression limits, not gameplay tuning: 30 s per trip includes launch/landing;
// a moving bot must gain 1 m in every 10 s window, even if it wall-slides.
const TRIP_TICKS: u64 = 900;
const PROGRESS_TICKS: u64 = 300;
const ARRIVE: f32 = 3.0;
const LINKS: [(u16, u16); 2] = [(26, 25), (27, 24)];

#[derive(Debug)]
struct Crossing {
    bot: UnitHandle,
    hubs: [u16; 2],
    goals: [Vec3; 2],
    posts: [usize; 2],
    arc: u16,
    trips: usize,
    since: u64,
    window_since: u64,
    window_distance: f32,
    previous: Vec3,
    jet_ticks: [usize; 2],
    routed: [bool; 2],
    arrivals: [u64; 2],
}

#[derive(Debug, PartialEq)]
struct Receipt {
    // Every tick, not just the final state: scheduling and flight must replay.
    hashes: Vec<u64>,
    arrivals: [[u64; 2]; 4],
    jet_ticks: [[usize; 2]; 4],
}

fn run(scene: &Scene, mission: &MissionDef, roster: &ClassRoster, seed: u32) -> Receipt {
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = layout.apply_mission(mission);
    let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    mission.apply_arc_blocks(nav.as_mut());
    mission.apply_barriers(nav.as_mut());
    let mut sim = Sim::empty(seed);
    sim.battle = core::battle::Battle::new(mission.tickets());
    mission.apply_death_drains(&mut sim.battle);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    sim.omni_radius = core::sim::omni_radius_for("bf2");
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);
    assert_eq!(sim.damage_regions.len(), 9, "lava and steam must be active");
    assert_eq!(applied.death_regions.len(), 0, "MUS1 authors none");
    sim.death_regions = applied.death_regions;
    sim.fly_ceiling = applied.fly_ceiling;
    assert_eq!(sim.fly_ceiling, 90.0);
    let class = roster.register(&mut sim, &mut Vec::new());
    let teams = mission.factions().map(|faction| TeamTemplate {
        faction,
        ..TeamTemplate::default_trooper()
    });
    let mut w = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, teams, nav);
    scene.install_collision(&mut w.sim);
    w.add_posts(&layout);
    mission.apply_door_locks(&mut w.sim.doors, &scene.manifest);
    mission.apply_ai(&mut w.sim);
    w.opening_wave([0; 2], true);
    assert_eq!(w.sim.cps.len(), 6, "retain the mission's command posts");

    let graph = w.sim.nav.as_ref().unwrap().clone();
    let mut crossings = Vec::new();
    for (a, b) in LINKS {
        let (arc, link) = graph
            .arcs
            .iter()
            .enumerate()
            .find(|(_, arc)| (arc.a, arc.b) == (a, b))
            .expect("authored jet link");
        assert_ne!(link.flags & core::nav::Arc::JET_JUMP, 0);
        let top = graph.hubs[a as usize]
            .pos
            .y
            .max(graph.hubs[b as usize].pos.y)
            + 20.0;
        let goals = [a, b].map(|hub| {
            let p = graph.hubs[hub as usize].pos;
            let probe = if p.y > 0.0 { p.y + 1.0 } else { top };
            Vec3::new(
                p.x,
                w.sim.ground_under(Vec3::new(p.x, probe, p.z), false),
                p.z,
            )
        });
        let posts = [0, 1].map(|end| {
            let at = w.sim.add_cp_unresolved(
                CpDef {
                    name: format!("jet-test-hub-{}", [a, b][end]),
                    pos: goals[end],
                    capture: Region::sphere(goals[end], 1.0),
                    control: None,
                    value_bleed: 0,
                    value_atk: [0; 2],
                    value_def: [0; 2],
                    spawn_nodes: vec![],
                    capture_time: f32::MAX,
                    neutralize_time: f32::MAX,
                },
                None,
            );
            // Resolve to the authored identifier even for its y=0 leaf hub.
            w.sim.cps[at].nav_hub = Some([a, b][end]);
            at
        });
        for end in [0, 1] {
            let bot = w
                .sim
                .spawn(&class.spec(0, goals[end], 0.0, Controller::Ai, 0, 0))
                .unwrap();
            // Stage charged troopers; no fuel/health/position writes after this.
            w.sim.unit_mut(bot).unwrap().soldier.fuel = 1.0;
            crossings.push(Crossing {
                bot,
                hubs: [[a, b][end], [a, b][1 - end]],
                goals: [goals[end], goals[1 - end]],
                posts: [posts[end], posts[1 - end]],
                arc: arc as u16,
                trips: 0,
                since: 0,
                window_since: 0,
                window_distance: goals[0].distance(goals[1]),
                previous: goals[end],
                jet_ticks: [0; 2],
                routed: [false; 2],
                arrivals: [0; 2],
            });
        }
    }

    let mut hashes = Vec::new();
    for _ in 0..2400 {
        // Keep the ordinary commander cadence, substituting only the test
        // troopers' objectives before the regular tactical brain reads them.
        for team in 0..2 {
            if w.sim.tick >= w.sim.commander[team].next_plan_tick {
                core::commander::plan(&mut w.sim, team as u8);
            }
        }
        for c in &crossings {
            let dest = if c.trips == 0 { 1 } else { 0 };
            w.sim.commander[0].assignment[c.bot.index as usize] =
                Some(Objective::Attack(c.posts[dest] as u32));
        }
        w.step_tick(&[]);
        for event in &w.sim.events {
            if let Event::EnvDeath { unit, cause } = event {
                assert!(
                    crossings.iter().all(|c| c.bot != *unit),
                    "seed {seed} tick {}: {unit:?} {cause:?}; crossing {:?}; soldier {:?}",
                    w.sim.tick,
                    crossings.iter().find(|c| c.bot == *unit),
                    w.sim.units[unit.index as usize].soldier
                );
            }
        }
        for c in &mut crossings {
            let u = w.sim.unit(c.bot).unwrap_or_else(|| {
                panic!("seed {seed} tick {}: crossing bot died: {c:?}", w.sim.tick)
            });
            let pos = u.soldier.pos;
            let (lift, radius) = soldier_blast_sphere(u.soldier.stance);
            assert!(
                !w.sim
                    .damage_regions
                    .iter()
                    .any(|r| r.region.touches_sphere(pos + Vec3::Y * lift, radius)),
                "seed {seed} tick {}: damage-region contact: {c:?}, pos {pos:?}",
                w.sim.tick
            );
            assert!(!w.sim.death_regions.iter().any(|r| r.contains(pos)));
            assert_eq!(
                u.health.cur, u.health.max,
                "no damage or healing masks a bad crossing"
            );
            assert!(
                pos.distance(c.previous) < 2.0,
                "seed {seed}: relocated instead of crossing: {c:?}"
            );
            c.previous = pos;
            if c.trips == 2 {
                continue;
            }
            let trip = c.trips;
            let goal = c.goals[1 - trip];
            let distance = core::nav::xz_dist(pos, goal);
            c.jet_ticks[trip] += usize::from(u.soldier.stance.jetting());
            c.routed[trip] |=
                u.ai.route
                    .is_some_and(|r| r.via_arc == Some(c.arc) && r.next_hub == c.hubs[1 - trip])
                    && u.ai.jet.flight.is_some_and(|f| f.route && f.launched);
            assert!(
                w.sim.tick - c.since < TRIP_TICKS,
                "seed {seed}: trip timed out: {c:?}, at {pos:?}, ai {:?}",
                u.ai
            );
            if distance < ARRIVE && (pos.y - goal.y).abs() < 0.1 && u.soldier.grounded {
                assert!(c.routed[trip], "must take the authored jet route: {c:?}");
                assert!(c.jet_ticks[trip] > 0, "must fly on every trip: {c:?}");
                c.arrivals[trip] = w.sim.tick;
                c.trips += 1;
                c.since = w.sim.tick;
                c.window_since = w.sim.tick;
                c.window_distance = core::nav::xz_dist(pos, c.goals[0]);
            } else if w.sim.tick - c.window_since >= PROGRESS_TICKS {
                assert!(
                    c.window_distance - distance > 1.0,
                    "seed {seed}: no net progress: {c:?}, at {pos:?}, ai {:?}",
                    u.ai
                );
                c.window_since = w.sim.tick;
                c.window_distance = distance;
            }
        }
        hashes.push(w.sim.hash());
        if crossings.iter().all(|c| c.trips == 2) {
            let arrivals = std::array::from_fn(|i| crossings[i].arrivals);
            let jet_ticks = std::array::from_fn(|i| crossings[i].jet_ticks);
            eprintln!("MUS1 seed {seed}: 8/8 landed crossings, 0 hazard contacts/deaths; arrivals {arrivals:?}, jet ticks {jet_ticks:?}, tick {}, hash {:016x}", w.sim.tick, hashes.last().unwrap());
            return Receipt {
                hashes,
                arrivals,
                jet_ticks,
            };
        }
    }
    panic!("seed {seed}: unfinished round: {crossings:?}");
}

#[test]
fn mustafar_seeded_jet_round_crosses_both_links_out_and_back() {
    let root = game::assets::baked_root();
    let (scene, loaded) = game::assets::load_battle_scene(&root.join("bf2_mus1"), "con").unwrap();
    assert!(
        loaded.common.is_some_and(|(_, result)| result.is_ok()),
        "include the battle's shared collision"
    );
    assert!(loaded.layer.is_some(), "include the conquest layer");
    let mission = MissionDef::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../missions/bf2_mus1_con_cw.ron"),
    )
    .unwrap();
    let db = load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
    let roster = ClassRoster::build(&db, "rep_inf_ep3_jettrooper", 1, None).unwrap();
    let mut previous = None;
    for seed in [1, 42] {
        let receipt = run(&scene, &mission, &roster, seed);
        assert_eq!(
            receipt,
            run(&scene, &mission, &roster, seed),
            "seed {seed}: every tick must replay"
        );
        if let Some(previous) = previous {
            assert_ne!(receipt.hashes, previous, "the round must use its seed");
        }
        previous = Some(receipt.hashes);
    }
}
