//! The routing tests: the corridor, the corners, the level test, the hub a
//! unit routes from, a post's point, and a hub reached on its level (card
//! SPL11 moved them out of `route.rs`, unchanged).

use super::*;
use crate::ai::tests::l_sim;
use crate::ai::think;
use crate::nav::{Arc, Hub, RouteTable};
use crate::sim::PadState;

/// Card PARR, the contester: a bot on its last leg with no target walks
/// at an enemy soldier standing in the post's capture region — not at
/// one outside it, and not while it has a target of its own.
#[test]
fn a_last_leg_walks_at_an_unseen_enemy_in_the_capture_region() {
    use crate::unit::UnitSpec;
    let (mut sim, h) = l_sim();
    sim.unit_mut(h).expect("bot").soldier.pos = Vec3::new(40.0, 0.0, -30.0);
    let leg = |sim: &Sim, target: Option<UnitHandle>| {
        let mut st = AiState {
            target_cp: Some(0),
            target: target.map(Into::into),
            ..AiState::default()
        };
        let mut nt = NavTick::default();
        st.nav.route = plan_leg(sim, &st, h, 2, 2, None, &mut nt);
        aim_last_leg(sim, h, &mut st, &mut nt);
        st.nav.route.expect("a last leg").waypoint
    };
    let own = leg(&sim, None);
    assert!(sim.cps[0].def.capture.contains(own) && own != sim.cps[0].def.pos);
    let inside = Vec3::new(43.0, 0.0, -42.0);
    let e = sim.spawn(&UnitSpec::trooper(1, inside)).expect("enemy");
    assert_eq!(leg(&sim, None), inside, "at the contester");
    assert_eq!(
        leg(&sim, Some(e)),
        own,
        "a bot with a target keeps its point"
    );
    sim.unit_mut(e).expect("enemy").soldier.pos = Vec3::new(50.0, 0.0, -40.0);
    assert_eq!(
        leg(&sim, None),
        own,
        "an enemy outside the region is not one"
    );
}

/// Card STLVL (TAT2's rooftop hubs 25 / 26, BF1 BES2's hub 97): a
/// soldier on the ground UNDER a roof whose hub contains him routes from
/// the nearest hub on the ground; on the roof he routes from the roof's.
#[test]
fn a_soldier_under_a_roof_routes_from_a_hub_on_his_own_floor() {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::unit::{Controller, UnitSpec};
    use glam::Mat4;
    let slab = |x: f32, y: f32, half: f32| {
        Body::new(
            Mat4::from_translation(Vec3::new(x, y - 0.25, 0.0)),
            Shape::Box {
                half: Vec3::new(half, 0.25, half),
            },
        )
    };
    let mut sim = Sim::empty(5);
    sim.collision =
        CollisionWorld::new(vec![slab(0.0, 0.0, 60.0), slab(0.0, 5.0, 5.0)], Vec::new());
    let hub = |x: f32| Hub {
        pos: Vec3::new(x, 0.0, 0.0),
        radius: 3.0,
        arcs: vec![0],
    };
    sim.nav = Some(
        NavGraph::new(
            vec![hub(0.0), hub(14.0)],
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
    // The post on the ground in hub 1, a spawn node on the roof in hub 0.
    let at = Vec3::new(14.0, 0.0, 1.0);
    sim.add_cp(
        crate::cp::CpDef {
            name: "cp1".into(),
            pos: at,
            capture: crate::region::Region::sphere(at, 6.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![(Vec3::new(0.5, 5.0, 0.5), 0.0)],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    assert!((sim.nav_floor(0).expect("roof hub") - 5.0).abs() < 0.05);
    assert!(sim.nav_floor(1).expect("ground hub").abs() < 0.05);
    let mut put = |y: f32| {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(1.0, y, 1.0))
        })
        .expect("spawned")
    };
    let (under, on) = (put(0.0), put(5.0));
    let g = sim.nav.as_ref().expect("graph");
    assert_eq!(
        g.unit_hub_on(Vec3::new(1.0, 0.0, 1.0), nav::Mover::SOLDIER, &|_| true),
        Some(0),
        "a start is located in XZ: under the roof is in the roof's hub"
    );
    assert_eq!(start_hub(&sim, g, under), Some(1), "under the roof");
    assert_eq!(start_hub(&sim, g, on), Some(0), "on the roof");
}

/// Card DAGS (BF2 Dagobah, post cp4's spawn node on the ground over its
/// cave): a bot spawned on a deck, inside the disc of a hub of the route
/// UNDER the deck, plans its first leg from the deck's own hub.
#[test]
fn a_bot_spawned_on_a_deck_over_a_lower_route_takes_the_decks_own_hub() {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::cp::CpDef;
    use crate::region::Region;
    use crate::unit::{Controller, UnitSpec};
    use glam::Mat4;
    let slab = |c: Vec3, half: Vec3| Body::new(Mat4::from_translation(c), Shape::Box { half });
    let mut sim = Sim::empty(5);
    // The ground (top y 0) and a deck (top y 5) over x -40..5.
    sim.collision = CollisionWorld::new(
        vec![
            slab(Vec3::new(0.0, -0.25, 0.0), Vec3::new(60.0, 0.25, 60.0)),
            slab(Vec3::new(-17.5, 4.75, 0.0), Vec3::new(22.5, 0.25, 5.0)),
        ],
        Vec::new(),
    );
    // Hubs 0 and 1 are the lower route (0 is under the deck's end), 2 and 3
    // the deck's; every hub is authored at y 0. Arc 1 (0-2) joins the two.
    let hub = |x: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(x, 0.0, 0.0),
        radius: 3.0,
        arcs,
    };
    let arc = |a: u16, b: u16| Arc {
        a,
        b,
        type_mask: 1,
        flags: 0,
    };
    sim.nav = Some(
        NavGraph::new(
            vec![
                hub(0.0, vec![0, 1]),
                hub(14.0, vec![0]),
                hub(-15.0, vec![1, 2]),
                hub(-30.0, vec![2]),
            ],
            vec![arc(0, 1), arc(0, 2), arc(2, 3)],
            RouteTable::empty(4),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph"),
    );
    let post = |name: &str, at: Vec3, nodes: Vec<(Vec3, f32)>| CpDef {
        name: name.into(),
        pos: at,
        capture: Region::sphere(at, 6.0),
        control: None,
        value_bleed: 100,
        value_atk: [100, 100],
        value_def: [100, 100],
        spawn_nodes: nodes,
        capture_time: 10.0,
        neutralize_time: 12.0,
    };
    // The bot's own post is under the deck, with a spawn node ON the deck;
    // the post to take is at the deck's far end.
    sim.add_cp(
        post(
            "low",
            Vec3::new(0.0, 0.0, 1.0),
            vec![(Vec3::new(-15.5, 5.0, 0.5), 0.0)],
        ),
        Some(0),
    );
    sim.add_cp(
        post("deck", Vec3::new(-30.0, 5.0, 1.0), Vec::new()),
        Some(1),
    );
    assert!(sim.nav_floor(0).expect("lower hub").abs() < 0.05);
    assert!((sim.nav_floor(2).expect("deck hub") - 5.0).abs() < 0.05);
    assert!((sim.nav_floor(3).expect("deck hub") - 5.0).abs() < 0.05);
    let at = Vec3::new(1.0, 5.0, 1.0);
    let me = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, at)
        })
        .expect("spawned");
    let g = sim.nav.as_ref().expect("graph");
    assert_eq!(
        g.unit_hub_on(at, nav::Mover::SOLDIER, &|_| true),
        Some(0),
        "located in XZ, the bot is in the lower route's hub"
    );
    let mut st = AiState::default();
    retarget_phase_a(&sim, me, &mut st, None, &mut NavTick::default());
    let r = st.nav.route.expect("a leg");
    assert_eq!(st.target_cp, Some(1), "the deck's post");
    assert_eq!(
        (r.via_arc, r.next_hub),
        (Some(2), 3),
        "the first leg leaves the deck's own hub (2) along the deck"
    );
}

/// STK2: a hunter already at its quarry's hub walks at the QUARRY, and
/// keeps following it as it moves — before, the last leg's waypoint was the
/// hub, and BF1 YAV1's last hunter stood on it for 35,400 ticks.
#[test]
fn a_hunter_on_its_quarrys_hub_walks_at_the_quarry() {
    let (sim, h) = l_sim();
    let mut st = sim.unit(h).expect("alive").ai;
    let mut nt = NavTick::default();
    for enemy in [Vec3::new(5.0, 0.0, -3.0), Vec3::new(-6.0, 0.0, 4.0)] {
        retarget_hunt(&sim, h, &mut st, Some(enemy), &mut nt);
        let r = st.nav.route.expect("a route");
        assert_eq!(
            (r.dest_hub, r.is_last()),
            (0, true),
            "the quarry's hub is the hunter's own"
        );
        assert_eq!(
            r.waypoint, enemy,
            "the last leg aims at the enemy, not hub 0"
        );
    }
}

fn wander_goals(sim: &mut Sim, h: UnitHandle, seconds: u32) -> Vec<(Vec3, u16)> {
    let mut seen: Vec<(Vec3, u16)> = Vec::new();
    for _ in 0..seconds * crate::TICK_HZ {
        sim.step(&[]);
        if let Some(goal) = sim.unit(h).and_then(|u| u.ai.wander) {
            if seen.last() != Some(&goal) {
                seen.push(goal);
            }
        }
    }
    seen
}

#[test]
fn a_foot_bot_in_a_space_round_walks_from_hub_to_hub_and_decides_again_on_arrival() {
    let (mut sim, h) = l_sim();
    sim.space_assault = Some(crate::sim::SpaceAssault::new(10));
    let goals = wander_goals(&mut sim, h, 120);
    assert!(goals.len() >= 2, "{goals:?}");
    let hubs: std::collections::BTreeSet<u16> = goals.iter().map(|g| g.1).collect();
    assert!(hubs.len() >= 2, "{goals:?}");
}

#[test]
fn a_foot_bot_on_a_ground_map_takes_no_space_walk() {
    let (mut sim, h) = l_sim();
    assert!(wander_goals(&mut sim, h, 30).is_empty());
}

#[test]
fn the_follower_walks_the_corridor_instead_of_the_beeline() {
    let (mut sim, h) = l_sim();
    assert_eq!(sim.cps[0].nav_hub, Some(2), "post resolved to its hub");
    let mut legs = Vec::new();
    let mut max_x_before_corner = 0.0f32;
    for _ in 0..1200 {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        if let Some(r) = u.ai.nav.route {
            if legs.last() != Some(&r.next_hub) {
                legs.push(r.next_hub);
            }
        }
        // Before the unit is level with hub 1 it must not have cut the
        // corner: a beeline to the post would run x and z together.
        if u.soldier.pos.z > -35.0 {
            max_x_before_corner = max_x_before_corner.max(u.soldier.pos.x);
        }
    }
    let u = sim.unit(h).expect("alive");
    assert!(
        sim.cps[0].def.capture.contains(u.soldier.pos),
        "reached the post: {:?} after legs {legs:?}",
        u.soldier.pos
    );
    assert_eq!(legs, vec![1, 2], "hub 1 then the post");
    assert!(
        max_x_before_corner < 5.0,
        "cut the corner: x {max_x_before_corner}"
    );
    assert_eq!(sim.nav_stats.bfs_fallbacks, 0, "the table answered");
    assert_eq!(sim.nav_stats.straight_fallbacks, 0);
}

/// With the arc masked out for SOLDIER the table entry still points at it,
/// but nothing is reachable: the unit falls back to walking straight.
#[test]
fn an_unreachable_post_falls_back_to_the_straight_line() {
    let (mut sim, h) = l_sim();
    if let Some(g) = sim.nav.as_mut() {
        let hubs = g.hubs.clone();
        let mut arcs = g.arcs.clone();
        arcs[1].type_mask = 2; // HOVER only: hub 2 is cut off for soldiers
        let routes = g.routes.clone();
        *g = NavGraph::new(hubs, arcs, routes, Vec::new(), Vec::new()).expect("graph");
    }
    for _ in 0..30 {
        sim.step(&[PadState::default()]);
    }
    let u = sim.unit(h).expect("alive");
    assert_eq!(u.ai.target_cp, Some(0));
    assert!(u.ai.nav.route.is_none(), "no route: {:?}", u.ai.nav.route);
}

/// A barrier across the first leg puts CORNER waypoints on the route, and
/// the unit walks them: the corner is what the steering aims at, and the
/// leg advances past it on arrival.
#[test]
fn a_barrier_across_the_leg_puts_corners_on_the_route() {
    let (mut sim, h) = l_sim();
    // A 16 x 2 m wall across the first corridor at z = -20, its ends at
    // x = -8 and x = +8 — a detour has to go round one of them.
    if let Some(g) = sim.nav.as_mut() {
        g.barriers = vec![nav::Barrier {
            centre: Vec2::new(0.0, -20.0),
            axes: [Vec2::X, Vec2::Y],
            half: Vec2::new(8.0, 1.0),
            mask: 1,
            enabled: true,
        }];
    }
    let mut st = AiState::default();
    think(&sim, h, &mut st);
    let r = st.nav.route.expect("a route");
    assert!(r.detour_len > 0, "the leg was detoured: {r:?}");
    let c = r.corner().expect("a corner to walk to");
    assert!(c.x.abs() > 8.0, "round an end of the wall: {c}");
    // The corner is what it aims at, and it gets there and moves on.
    let mut reached = false;
    for _ in 0..600 {
        sim.step(&[PadState::default()]);
        if sim
            .unit(h)
            .and_then(|u| u.ai.nav.route)
            .is_some_and(|r| r.detour_at > 0 || r.detour_len == 0)
        {
            reached = true;
            break;
        }
    }
    assert!(reached, "never got past the corner");
    assert!(sim.nav_stats.detoured_legs > 0);
}

/// Card OBST2: a corner walk longer than the leg's array is CONTINUED from
/// its last stored corner. A walled lane with five baffles across it takes
/// more corners than [`DETOUR_MAX`]; at the last one the rest is searched
/// for - the leg does not go straight at the waypoint, through the baffles
/// left (BF2 Mustafar's leg 30 -> 3 round a pit: 23 recycles in 8 rounds
/// without this).
#[test]
fn a_corner_walk_over_the_legs_array_is_continued() {
    let (mut sim, h) = l_sim();
    let wall = |cx: f32, cz: f32, hx: f32, hz: f32| nav::Barrier {
        centre: Vec2::new(cx, cz),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(hx, hz),
        mask: 1,
        enabled: true,
    };
    // The lane's sides (x = -11 and 11, z 30 .. -34) and five baffles, by
    // turns from the left and from the right, each leaving 6 m at its end.
    let mut lane = vec![wall(-11.0, -2.0, 0.5, 32.0), wall(11.0, -2.0, 0.5, 32.0)];
    for k in 0..5 {
        let side = if k % 2 == 0 { -3.0 } else { 3.0 };
        lane.push(wall(side, -6.0 - 6.0 * k as f32, 7.5, 0.5));
    }
    if let Some(g) = sim.nav.as_mut() {
        g.barriers = lane;
    }
    let mut st = AiState::default();
    let mut nt = NavTick::default();
    let r = plan_leg(&sim, &st, h, 0, 2, None, &mut nt).expect("a leg");
    let g = sim.nav.as_ref().expect("graph");
    let wp = Vec2::new(r.waypoint.x, r.waypoint.z);
    let whole = soldier_way(g, Vec2::ZERO, wp).expect("round the ring");
    assert!(whole.len() > DETOUR_MAX, "a long way round: {whole:?}");
    assert_eq!(usize::from(r.detour_len), DETOUR_MAX, "cut to the array");
    // On the last stored corner: the walk goes on, clear of the ring.
    let last = r.detour[DETOUR_MAX - 1];
    assert!(last.y > -40.0, "short of the arrival plane: {last}");
    st.nav.route = Some(Route {
        detour_at: (DETOUR_MAX - 1) as u8,
        ..r
    });
    let at = Vec3::new(last.x, 0.0, last.y);
    advance_route(&sim, h, &mut st, at, 0.0, &mut nt);
    let r = st.nav.route.expect("the leg");
    let next = r.corner().expect("a corner after the array's last");
    assert_eq!(
        g.leg_crosses_barrier(last, next, crate::unit::SOLDIER_RADIUS, nav::SOLDIER),
        None
    );
    assert_eq!(next, whole[DETOUR_MAX], "the same way round");
    // A walk that fitted the array ends at its last corner as before.
    let mut short = Route {
        detour_len: 2,
        detour_at: 1,
        ..r
    };
    short.detour[1] = last;
    st.nav.route = Some(short);
    advance_route(&sim, h, &mut st, at, 0.0, &mut nt);
    let r = st.nav.route.expect("the leg");
    assert_eq!((r.detour_len, r.detour_at), (2, 2), "no new search");
}

/// A two-storey scene with NO terrain: a lower slab (x in [-60, 0], top
/// y = 0) and an upper one (x in [0, 60], top y = 10). The graph offers
/// the unit exactly one authored way to its post — an arc to a hub whose
/// XZ is floored only on the UPPER storey — and the follower must reject
/// that leg and take the other arc instead.
///
/// Note which way round the test has to be. `ground_under` looks DOWN, so
/// the level test can only see a hub whose floor is ABOVE the unit: a unit
/// on the LOWER floor cannot find the upper storey's hub, and a unit on
/// the upper floor DOES find the lower storey's (a floor it can fall to,
/// which the design does not reject). The second half of the test asserts
/// exactly that asymmetry so it cannot drift unnoticed.
fn two_floor_sim(unit_at: Vec3) -> (Sim, UnitHandle) {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::cp::CpDef;
    use crate::region::Region;
    use crate::unit::{Controller, UnitSpec};
    use glam::Mat4;
    let slab = |c: Vec3, half: Vec3| Body::new(Mat4::from_translation(c), Shape::Box { half });
    let mut sim = Sim::empty(7);
    // No heightfield at all: off the slabs there is nothing, which is the
    // NO_GROUND the level test is looking for.
    sim.collision = CollisionWorld::new(
        vec![
            slab(Vec3::new(-30.0, -1.0, -40.0), Vec3::new(30.0, 1.0, 60.0)),
            slab(Vec3::new(30.0, 9.0, -40.0), Vec3::new(30.0, 1.0, 60.0)),
        ],
        Vec::new(),
    );
    // 0 = start (lower), 1 = the UPPER-only hub, 2 = a lower hub, 3 = the
    // post's hub. Every hub is authored at y 0, like BF2 Polis Massa's.
    let hub = |x: f32, z: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(x, 0.0, z),
        radius: 3.0,
        arcs,
    };
    let hubs = vec![
        hub(-20.0, 0.0, vec![0, 1]),
        hub(30.0, -40.0, vec![0, 2]),
        hub(-40.0, -40.0, vec![1, 3]),
        hub(-40.0, -80.0, vec![2, 3]),
    ];
    let a = |a: u16, b: u16| Arc {
        a,
        b,
        type_mask: 1,
        flags: 0,
    };
    let arcs = vec![a(0, 1), a(0, 2), a(1, 3), a(2, 3)];
    // The table's ONLY answer from hub 0 to hub 3 is slot 0 = arc 0 = the
    // upper-storey hub, with the whole weight.
    let n = 4usize;
    let mut rows = vec![Vec::new(); crate::nav::TYPE_COUNT * n];
    let mut r0 = vec![0u8; n];
    r0[3] = 31 << 3; // slot 0
    rows[nav::SOLDIER * n] = r0;
    sim.nav = Some(
        NavGraph::new(
            hubs,
            arcs,
            RouteTable::from_rows(n, crate::nav::TYPE_COUNT, rows),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph"),
    );
    sim.add_cp(
        CpDef {
            name: "cp1".into(),
            pos: Vec3::new(-40.0, 0.0, -80.0),
            capture: Region::sphere(Vec3::new(-40.0, 0.0, -80.0), 6.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    let h = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, unit_at)
        })
        .expect("spawned");
    (sim, h)
}

#[test]
fn a_waypoint_with_no_floor_on_this_level_is_rejected_for_the_other_arc() {
    let (mut sim, h) = two_floor_sim(Vec3::new(-20.0, 0.0, 0.0));
    assert_eq!(sim.cps[0].nav_hub, Some(3), "post resolved to its hub");
    // The hub the TABLE wants (1) has no floor on the lower storey; the
    // alternative (2) does.
    {
        let g = sim.nav.as_ref().expect("graph");
        assert!(!hub_has_floor(&sim, g, 1, 0.0), "hub 1 is upstairs only");
        assert!(hub_has_floor(&sim, g, 2, 0.0));
        // The chooser really does offer the bad one first.
        assert_eq!(
            g.next_hop(0, 3, nav::Mover::SOLDIER, None, None, 0),
            Some((0, 1))
        );
    }
    let mut seen = Vec::new();
    for _ in 0..1500 {
        sim.step(&[PadState::default()]);
        if let Some(r) = sim.unit(h).expect("alive").ai.nav.route {
            if seen.last() != Some(&r.next_hub) {
                seen.push(r.next_hub);
            }
        }
    }
    assert!(
        !seen.contains(&1),
        "never routed through the upstairs hub: {seen:?}"
    );
    assert!(sim.nav_stats.level_rejected > 0, "the leg was rejected");
    assert_eq!(
        sim.nav_stats.level_unrouted, 0,
        "and an alternative was always found"
    );
    assert_eq!(sim.nav_stats.straight_fallbacks, 0);
    let u = sim.unit(h).expect("alive");
    assert!(
        sim.cps[0].def.capture.contains(u.soldier.pos),
        "walked the alternative to the post: {:?} via {seen:?}",
        u.soldier.pos
    );

    // The asymmetry: from the UPPER storey the same hub 1 has a floor —
    // the lower slab, 10 m down — so nothing is rejected. `ground_under`
    // looks down, and a floor below is somewhere a unit can fall to.
    let (up, _) = two_floor_sim(Vec3::new(20.0, 10.0, -40.0));
    let g = up.nav.as_ref().expect("graph");
    assert!(hub_has_floor(&up, g, 1, 10.0), "its own storey");
    assert!(
        hub_has_floor(&up, g, 2, 10.0),
        "and the lower storey's hub is a floor it can fall to"
    );
    assert_eq!(
        hub_waypoint(&up, g, 2, 10.0).y,
        0.0,
        "resolved 10 m below the unit, and accepted"
    );
    assert_eq!(hub_waypoint(&up, g, 1, 10.0).y, 10.0, "its own storey");
}

/// Card BNDS: a hub is reached on ITS level. Under it — at its XZ, on a
/// floor far below its resolved one — is not arrival.
#[test]
fn a_hub_is_not_reached_from_under_it() {
    use crate::ai::tests::rim_sim;
    let (sim, _) = rim_sim(false);
    let g = sim.nav.as_ref().expect("graph");
    let r = crate::ai::Route {
        dest_hub: 1,
        next_hub: 1,
        via_arc: Some(0),
        waypoint: Vec3::new(6.0, 10.0, 0.0),
        ..Default::default()
    };
    assert!(hub_reached(&sim, g, &r, Vec3::new(6.0, 10.0, 0.5), 0.0));
    assert!(!hub_reached(&sim, g, &r, Vec3::new(6.0, 2.0, 0.5), 0.0));
}

/// Card HUBP's scene: the L graph with hub 1 (at 0, -40) grown to 8 m, and
/// the legs 200 soldiers standing at hub 0 plan toward the post at hub 2.
fn hub_point_legs(barrier: Option<nav::Barrier>) -> (Sim, Vec<Route>) {
    use crate::unit::{Controller, UnitSpec};
    let (mut sim, first) = l_sim();
    if let Some(g) = sim.nav.as_mut() {
        g.hubs[1].radius = 8.0;
        g.barriers.extend(barrier);
    }
    let mut units = vec![first];
    for _ in 1..200 {
        let spec = UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        };
        units.push(sim.spawn(&spec).expect("spawned"));
    }
    let st = AiState {
        target_cp: Some(0),
        ..AiState::default()
    };
    let legs = units
        .iter()
        .map(|&h| plan_leg(&sim, &st, h, 0, 2, None, &mut NavTick::default()).expect("a leg"))
        .collect();
    (sim, legs)
}

/// Card HUBP: every unit walks at its OWN point of the next hub — the
/// PS2's waypoint, a point of the hub's sphere — not at the hub's centre:
/// over 200 units the XZ offsets span the hub's radius, none leaves it, no
/// two are the same, and they lie on every side of the centre. The leg's
/// line starts where the unit stood.
#[test]
fn each_unit_draws_its_own_point_of_the_next_hub() {
    let (_, legs) = hub_point_legs(None);
    let c = Vec2::new(0.0, -40.0);
    let offs: Vec<Vec2> = legs
        .iter()
        .map(|r| Vec2::new(r.waypoint.x, r.waypoint.z) - c)
        .collect();
    let max = offs.iter().map(|o| o.length()).fold(0.0, f32::max);
    assert!(max > 0.8 * 8.0 && max <= 8.0 + 1e-3, "max offset {max}");
    let mean = offs.iter().map(|o| o.length()).sum::<f32>() / offs.len() as f32;
    assert!(
        mean > 0.5 * 8.0,
        "a sphere's XZ is dense at the rim: {mean}"
    );
    for (i, a) in offs.iter().enumerate() {
        assert!(offs[..i].iter().all(|b| b != a), "unit {i} shares a point");
    }
    for side in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
        let n = offs.iter().filter(|o| o.dot(side) > 2.0).count();
        assert!(n > 20, "{n} of 200 on side {side}");
    }
    assert!(legs
        .iter()
        .all(|r| r.next_hub == 1 && r.waypoint.y == 0.0 && r.from == Vec2::ZERO));
}

/// Card HUBP: a point that lies in a barrier is drawn again (the PS2's
/// re-draw) — with a barrier over the east half of the hub no leg ends in
/// it, where without it a third and more do.
#[test]
fn a_hub_point_in_a_barrier_is_drawn_again() {
    let east = nav::Barrier {
        centre: Vec2::new(5.0, -40.0),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(4.0, 10.0),
        mask: 1,
        enabled: true,
    };
    let inside = |legs: &[Route]| {
        legs.iter()
            .filter(|r| east.contains(Vec2::new(r.waypoint.x, r.waypoint.z)))
            .count()
    };
    let (_, open) = hub_point_legs(None);
    assert!(inside(&open) > 60, "{} of 200 unbarred", inside(&open));
    let (_, barred) = hub_point_legs(Some(east));
    assert_eq!(inside(&barred), 0);
    let off_centre = barred
        .iter()
        .filter(|r| nav::xz_dist(r.waypoint, Vec3::new(0.0, 0.0, -40.0)) > 1.0)
        .count();
    assert!(off_centre > 190, "the re-draw is a draw: {off_centre}");
    // A draw in a barrier costs none of the port's own tries: with all but
    // the hub's north cap barred (an eighth of the draws), every unit still
    // finds its point there.
    let south = nav::Barrier {
        centre: Vec2::new(0.0, -44.0),
        half: Vec2::new(10.0, 10.0),
        ..east
    };
    let (_, capped) = hub_point_legs(Some(south));
    let north = capped.iter().filter(|r| r.waypoint.z > -34.0).count();
    assert!(north >= 195, "{north} of 200 in the cap");
}

/// Card HUBP, ours: a drawn point is one the hub's own floor leads to. On
/// the rim deck (x and z within ±10, nothing beyond its east edge) with
/// hub 1 at x 6 grown to 9 m, no point is over the open air east of the
/// deck, behind a wall across the deck at x 0, or off the deck's height.
#[test]
fn a_hub_point_stands_on_the_hubs_own_floor() {
    use crate::ai::tests::rim_sim_with;
    use crate::collision::{Body, Shape};
    let wall = Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 11.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.25, 1.0, 10.0),
        },
    );
    let (mut sim, h) = rim_sim_with(false, vec![wall]);
    if let Some(g) = sim.nav.as_mut() {
        g.hubs[1].radius = 9.0;
    }
    let st = AiState::default();
    let (mut off, mut xs) = (0, Vec::new());
    for tick in 0..300 {
        sim.tick = tick;
        let r = plan_leg(&sim, &st, h, 0, 1, None, &mut NavTick::default()).expect("a leg");
        let p = r.waypoint;
        assert!(
            p.x <= 10.0 && p.x >= 0.9,
            "over the edge or at the wall: {p}"
        );
        assert!((p.y - 10.0).abs() < 0.1 && p.z.abs() <= 9.0, "{p}");
        off += usize::from(nav::xz_dist(p, Vec3::new(6.0, 0.0, 0.0)) > 1.0);
        xs.push(p.x);
    }
    assert!(off > 250, "{off} of 300 off the centre");
    assert!(xs.iter().any(|&x| x > 8.0) && xs.iter().any(|&x| x < 3.0));
}

/// Card HUBP: a stuck unit's re-plan walks the arc's line, centre to centre.
#[test]
fn a_stuck_units_leg_is_the_arcs_own_line() {
    let (sim, legs) = hub_point_legs(None);
    let g = sim.nav.as_ref().expect("graph");
    let mut r = legs[7];
    assert_ne!(r.waypoint, Vec3::new(0.0, 0.0, -40.0));
    centre_leg(&sim, g, &mut r, Vec3::new(3.0, 0.0, -5.0));
    assert_eq!(r.waypoint, Vec3::new(0.0, 0.0, -40.0));
    assert_eq!(r.from, Vec2::ZERO);
    assert_eq!(off_arc_line(&r, Vec3::new(3.0, 0.0, -5.0)), Some(3.0));
}

/// Card HUBP: a leg planned on ARRIVAL runs from the waypoint just reached
/// to the unit's point of the next hub (the PS2's leg); a leg planned
/// anywhere else starts at its hub's centre, the arc's own end.
#[test]
fn a_leg_planned_on_arrival_starts_at_the_waypoint_reached() {
    let (mut sim, h) = l_sim();
    if let Some(g) = sim.nav.as_mut() {
        g.hubs[1].radius = 8.0;
        g.hubs[2].radius = 8.0;
    }
    sim.cps[0].def.pos = Vec3::new(80.0, 0.0, -40.0);
    sim.step(&[PadState::default()]);
    let first = sim.unit(h).and_then(|u| u.ai.nav.route).expect("a route");
    assert_eq!((first.next_hub, first.from), (1, Vec2::ZERO));
    let reached = Vec2::new(first.waypoint.x, first.waypoint.z);
    assert!(
        (reached - Vec2::new(0.0, -40.0)).length() > 1.0,
        "{reached}"
    );
    for _ in 0..900 {
        sim.step(&[PadState::default()]);
        let r = sim.unit(h).and_then(|u| u.ai.nav.route).expect("a route");
        if r.next_hub == 2 {
            assert_eq!(r.from, reached);
            return;
        }
    }
    panic!("never reached hub 1");
}

/// Card HUBP, ours: a point is one the floor walked straight from the unit
/// reaches. On the rim deck a unit standing on it takes a point of hub 1 on
/// every leg; one with no floor under its feet (12 m under the deck, over
/// nothing) has none to walk and gets the hub's centre.
#[test]
fn a_hub_point_is_one_the_units_floor_reaches() {
    use crate::ai::tests::rim_sim;
    let (mut sim, h) = rim_sim(false);
    if let Some(g) = sim.nav.as_mut() {
        g.hubs[1].radius = 3.0;
    }
    let st = AiState::default();
    let centre = Vec3::new(6.0, 10.0, 0.0);
    let off = |sim: &Sim| {
        (0..100)
            .filter(|_| {
                let r = plan_leg(sim, &st, h, 0, 1, None, &mut NavTick::default()).expect("leg");
                nav::xz_dist(r.waypoint, centre) > 0.5
            })
            .count()
    };
    assert_eq!(off(&sim), 100, "from the deck every leg takes a point");
    // 12 m under the deck, over nothing: no floor leads anywhere.
    sim.unit_mut(h).expect("bot").soldier.pos = Vec3::new(0.0, -2.0, 0.0);
    assert_eq!(off(&sim), 0, "no floor from the feet: the centre");
}

/// Card MOVR2 — the PS2's leg rule: the point walked at is the unit's
/// projection on the leg's line advanced by its speed + [`LEG_LEAD`],
/// clamped to the leg's end; a corner and a leg with no arc are walked
/// straight at.
#[test]
fn a_leg_is_aimed_at_the_projection_advanced_by_the_speed() {
    let r = Route {
        next_hub: 1,
        dest_hub: 2,
        via_arc: Some(0),
        from: Vec2::ZERO,
        waypoint: Vec3::new(0.0, 2.0, -40.0),
        ..Route::default()
    };
    let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-4;
    // Beside the line at a run: 6 m/s + 0.2 m ahead of the projection, at
    // the unit's own height.
    let p = leg_point(&r, Vec3::new(3.0, 0.5, -10.0), 6.0);
    assert!(near(p, Vec3::new(0.0, 0.5, -16.2)), "{p}");
    // From a stand: back to the line, 0.2 m on.
    let p = leg_point(&r, Vec3::new(3.0, 0.5, -10.0), 0.0);
    assert!(near(p, Vec3::new(0.0, 0.5, -10.2)), "{p}");
    // On the line the point is still the projection's, not the far hub's.
    let p = leg_point(&r, Vec3::new(0.0, 0.0, -10.0), 6.0);
    assert!(near(p, Vec3::new(0.0, 0.0, -16.2)), "{p}");
    // Behind the leg's start the projection is the start.
    let p = leg_point(&r, Vec3::new(2.0, 0.0, 5.0), 6.0);
    assert!(near(p, Vec3::new(0.0, 0.0, -6.2)), "{p}");
    // Clamped to the leg's end: the waypoint itself.
    assert_eq!(leg_point(&r, Vec3::new(1.0, 0.0, -34.0), 6.0), r.waypoint);
    // A detour corner is walked straight at...
    let mut c = r;
    c.detour[0] = Vec2::new(4.0, -12.0);
    c.detour_len = 1;
    let p = leg_point(&c, Vec3::new(3.0, 0.5, -10.0), 6.0);
    assert!(near(p, Vec3::new(4.0, 0.5, -12.0)), "{p}");
    // ...and so is a leg with no arc (the last leg).
    let last = Route { via_arc: None, ..r };
    assert_eq!(
        leg_point(&last, Vec3::new(3.0, 0.5, -10.0), 6.0),
        r.waypoint
    );
}

/// Card MOUTH — the PS2's segment rule (BF2 `FUN_002f1470` ->
/// `FUN_002f0e78`, `analysis/re-soldier-ai.md` 2): with a detour corner
/// behind it a unit is aimed along the line from THAT corner to the
/// waypoint, not back at the leg's own line from [`Route::from`]. The scene
/// is BF2 Kamino's walkway mouth: the unit's point of the hub 4 m beside the
/// walkway's line, one corner at the mouth, the walkway along x = 0.
#[test]
fn past_a_corner_the_leg_is_aimed_along_the_corner_to_the_waypoint() {
    let mut r = Route {
        next_hub: 1,
        dest_hub: 2,
        via_arc: Some(0),
        from: Vec2::new(4.0, 0.0),
        waypoint: Vec3::new(0.0, 2.0, -40.0),
        detour_len: 1,
        detour_at: 1,
        ..Route::default()
    };
    r.detour[0] = Vec2::new(0.0, -2.0);
    let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-4;
    // Just past the corner at a run: 6.2 m on along the walkway. (On the
    // leg's own line the point is (3.1, -9.0): beside the walkway.)
    let p = leg_point(&r, Vec3::new(0.3, 0.5, -2.5), 6.0);
    assert!(near(p, Vec3::new(0.0, 0.5, -8.7)), "{p}");
    // Short of the corner still (it counts as passed within a metre): the
    // projection is the corner.
    let p = leg_point(&r, Vec3::new(0.6, 0.5, -1.5), 0.0);
    assert!(near(p, Vec3::new(0.0, 0.5, -2.2)), "{p}");
    // Clamped to the waypoint, as on any leg.
    assert_eq!(leg_point(&r, Vec3::new(0.2, 0.0, -35.0), 6.0), r.waypoint);
    // Two corners behind: the line starts at the LAST one.
    let mut two = r;
    two.detour[1] = Vec2::new(-1.0, -10.0);
    two.detour_len = 2;
    two.detour_at = 2;
    let p = leg_point(&two, Vec3::new(-1.0, 0.0, -10.0), 0.0);
    let dir = (Vec2::new(0.0, -40.0) - two.detour[1]).normalize();
    let want = two.detour[1] + dir * LEG_LEAD;
    assert!(near(p, Vec3::new(want.x, 0.0, want.y)), "{p}");
    // A corner still ahead is no start: with one of the two passed the unit
    // walks straight at the second.
    two.detour_at = 1;
    let p = leg_point(&two, Vec3::new(0.0, 0.0, -3.0), 6.0);
    assert!(near(p, Vec3::new(-1.0, 0.0, -10.0)), "{p}");
    // A leg whose corners were dropped (the stuck window's re-plan) is on
    // its own line again.
    let dropped = Route {
        detour_len: 0,
        detour_at: 0,
        ..r
    };
    let p = leg_point(&dropped, Vec3::new(4.0, 0.0, 0.0), 0.0);
    let dir = (Vec2::new(0.0, -40.0) - r.from).normalize();
    let want = r.from + dir * LEG_LEAD;
    assert!(near(p, Vec3::new(want.x, 0.0, want.y)), "{p}");
}

/// Card MOUTH, through `think`: a bot that has walked round a barrier's end
/// goes on along the line from that corner to its waypoint. Before the card
/// its aim went back to the line from the leg's start, which the barrier
/// lies across.
#[test]
fn a_bot_past_a_barriers_corner_walks_on_from_the_corner() {
    let (mut sim, h) = l_sim();
    if let Some(g) = sim.nav.as_mut() {
        g.barriers = vec![nav::Barrier {
            centre: Vec2::new(0.0, -20.0),
            axes: [Vec2::X, Vec2::Y],
            half: Vec2::new(8.0, 1.0),
            mask: 1,
            enabled: true,
        }];
    }
    let off = |a: Vec2, b: Vec2, p: Vec3| {
        let d = (b - a).normalize();
        let rel = Vec2::new(p.x, p.z) - a;
        (rel - d * rel.dot(d)).length()
    };
    let (mut on_corner_line, mut off_leg_line) = (0, 0);
    for _ in 0..900 {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        let (Some(r), true) = (u.ai.nav.route, u.ai.walk.kind == crate::ai::WalkAsk::LEG) else {
            continue;
        };
        if r.via_arc.is_none() || r.detour_at == 0 || r.corner().is_some() || u.ai.slide != 0 {
            continue;
        }
        let goal = u.ai.walk.goal;
        let wp = Vec2::new(r.waypoint.x, r.waypoint.z);
        if nav::xz_dist(goal, r.waypoint) < 1e-3 {
            continue;
        }
        let last = r.detour[usize::from(r.detour_at) - 1];
        assert!(
            off(last, wp, goal) < 1e-2,
            "aimed at {goal}, off the line {last} -> {wp}"
        );
        on_corner_line += 1;
        if off(r.from, wp, goal) > 0.5 {
            off_leg_line += 1;
        }
    }
    assert!(
        on_corner_line >= 20 && off_leg_line >= 20,
        "{on_corner_line} ticks past the corner, {off_leg_line} of them off the leg's own line"
    );
}

/// Card MOUTH: a hub hop opens a new progress window. A bot that runs a
/// free corridor never fails one, whatever the window's tick count is when
/// it reaches the hub — carried over the hop, a window closed a tick after
/// it on 0.2 m of the new leg: a re-plan and a side-step out of nothing.
#[test]
fn a_hop_opens_a_new_progress_window() {
    for phase in 0..crate::ai::STUCK_WINDOW {
        let (mut sim, h) = l_sim();
        sim.step(&[PadState::default()]);
        // The window's phase: this one closes `phase` ticks early (and
        // passes, its reference being out of reach), every later one 60
        // ticks after the last.
        let r = sim.unit_mut(h).expect("bot").ai.nav.route.as_mut();
        let r = r.expect("a route");
        (r.stuck_ticks, r.ref_dist) = (phase, f32::MAX);
        let (mut hops, mut last) = (0, None);
        for _ in 0..400 {
            sim.step(&[PadState::default()]);
            let Some(r) = sim.unit(h).expect("alive").ai.nav.route else {
                continue;
            };
            assert_eq!(
                (r.stuck_run, r.sidestep),
                (0, 0),
                "a failed window on a free corridor, {phase} ticks into it at the start"
            );
            if last.is_some_and(|l| l != r.next_hub) {
                hops += 1;
            }
            last = Some(r.next_hub);
        }
        assert!(hops >= 1, "the bot never hopped a hub");
    }
}

/// Card MOVR2: `think` walks an arc leg at [`leg_point`] with the soldier's
/// own speed — from a stand beside the line the first step is back to the
/// line, and at a run the point leads by the speed.
#[test]
fn a_bot_beside_its_leg_walks_at_the_leg_point_by_its_own_speed() {
    let (mut sim, h) = l_sim();
    sim.cps[0].def.pos = Vec3::new(80.0, 0.0, -40.0);
    sim.unit_mut(h).expect("bot").soldier.pos = Vec3::new(4.0, 0.0, -8.0);
    let (mut slow, mut fast) = (0, 0);
    for _ in 0..60 {
        let before = &sim.unit(h).expect("alive").soldier;
        let (pos, vel) = (before.pos, before.vel);
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        let (Some(r), true) = (u.ai.nav.route, u.ai.walk.kind == crate::ai::WalkAsk::LEG) else {
            continue;
        };
        if r.via_arc.is_none() || u.ai.slide != 0 {
            continue;
        }
        let speed = Vec2::new(vel.x, vel.z).length();
        let want = leg_point(&r, pos, speed);
        assert!(
            (u.ai.walk.goal - want).length() < 1e-3,
            "walks at {} for {want} at {speed} m/s",
            u.ai.walk.goal
        );
        assert_ne!(want, r.waypoint, "a point on the leg, not its end");
        if speed < 0.5 {
            slow += 1;
        } else if speed > 3.0 {
            fast += 1;
        }
    }
    assert!(
        slow >= 1 && fast >= 10,
        "{slow} from a stand, {fast} at a run"
    );
}

/// Card HJAM: a hover's driver has reached a detour corner at its own node
/// radius (ForwardSpeed x 0.5, the `min_arrive` its driver hands in), a
/// soldier — and a soldier handed the same number — at [`CORNER_ARRIVE`].
/// (At the soldier's metre a BF1 speeder bike turned round a corner it had
/// passed 3 m wide for ten seconds: 43 % of BF1 Endor's bike-driving time.)
#[test]
fn a_hovers_driver_reaches_a_corner_at_its_node_radius() {
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let (mut sim, h) = l_sim();
    if let Some(g) = sim.nav.as_mut() {
        g.barriers = vec![nav::Barrier {
            centre: Vec2::new(0.0, -20.0),
            axes: [Vec2::X, Vec2::Y],
            half: Vec2::new(8.0, 1.0),
            mask: 1,
            enabled: true,
        }];
    }
    let mut st = AiState::default();
    think(&sim, h, &mut st);
    let route = st.nav.route.expect("a route");
    let c = route.corner().expect("a corner to walk to");
    // `d` metres short of the corner, on the way to it from the start.
    let at = |d: f32| {
        let p = c - c.normalize() * d;
        Vec3::new(p.x, 0.0, p.y)
    };
    let passed = |sim: &Sim, d: f32, min_arrive: f32| {
        let mut st = AiState {
            nav: crate::ai::route::NavState::for_fixture(Some(route)),
            ..AiState::default()
        };
        let mut nt = NavTick::default();
        advance_route(sim, h, &mut st, at(d), min_arrive, &mut nt);
        st.nav.route.and_then(|r| r.corner()) != Some(c)
    };
    assert!(!passed(&sim, 8.0, 12.0), "on foot: the soldier's metre");
    assert!(passed(&sim, 0.9, 12.0), "on foot, inside the metre");
    let seat = |sim: &mut Sim, kind| {
        sim.exit_vehicle(h);
        let v = sim.spawn_vehicle(&VehicleSpec {
            kind,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: at(8.0) + Vec3::Y * 2.0,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        assert!(sim.enter_vehicle(h, v, 0));
    };
    seat(&mut sim, VehicleKind::Hover);
    assert!(passed(&sim, 8.0, 12.0), "a hover's driver: its node radius");
    assert!(passed(&sim, 11.9, 12.0));
    assert!(!passed(&sim, 12.5, 12.0), "and no further out");
    assert!(passed(&sim, 0.9, 0.5), "never under the soldier's metre");
    seat(&mut sim, VehicleKind::Walker);
    assert!(!passed(&sim, 8.0, 12.0), "a walker's driver: the metre");
}

/// A jet class's boarding destination may be joined only by a jet arc.
/// Both the follower's outer guard and its empty-table BFS must allow it.
#[test]
fn a_jet_only_boarding_route_uses_the_units_mover() {
    let (mut sim, me) = l_sim();
    let g = sim.nav.take().expect("graph");
    let mut arcs = g.arcs;
    for arc in &mut arcs {
        arc.flags = Arc::JET_JUMP;
    }
    sim.nav =
        Some(NavGraph::new(g.hubs, arcs, RouteTable::empty(3), vec![], vec![]).expect("jet graph"));
    let at = Vec3::new(40.0, 0.0, -40.0);
    let mut st = AiState::default();
    let mut nt = NavTick::default();
    sim.unit_mut(me).expect("bot").spec.jet = true;
    retarget_board(&sim, me, &mut st, at, &mut nt);
    let r = st.nav.route.expect("jet can reach the boarding hub");
    assert_eq!((r.dest_hub, r.next_hub, r.via_arc), (2, 1, Some(0)));
    assert_eq!(r.source, RouteSource::Bfs);
    sim.unit_mut(me).expect("bot").spec.jet = false;
    retarget_board(&sim, me, &mut st, at, &mut nt);
    assert_eq!(st.nav.route, None, "jetless unit cannot cross the same arc");
}

/// Issue #34: a flight of stairs 2.0 m wide between its rail barriers (BF2
/// Coruscant's to the War Room). A soldier's corner walk goes up it - at the
/// wide margin it is shut or walked round the outside - and keeps the rail a
/// waypoint on the flight stands near.
#[test]
fn a_soldier_takes_the_stairs_between_their_rails() {
    let bar = |cx: f32, cz: f32, hx: f32, hz: f32| nav::Barrier {
        centre: Vec2::new(cx, cz),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(hx, hz),
        mask: 1,
        enabled: true,
    };
    // The rails (inner faces at x -1.0 and 1.0, z -10 .. -18).
    let rails = [bar(-1.1, -14.0, 0.1, 4.0), bar(1.1, -14.0, 0.1, 4.0)];
    let up_the_flight = |g: &NavGraph, a: Vec2, b: Vec2, way: &[Vec2]| {
        let mut at = a;
        for &c in way.iter().chain(std::iter::once(&b)) {
            assert_eq!(
                g.leg_crosses_barrier(at, c, crate::unit::SOLDIER_RADIUS, nav::SOLDIER),
                None,
                "{at} -> {c} crosses a rail: {way:?}"
            );
            at = c;
        }
        for c in way {
            assert!(
                c.x.abs() < 1.0 || c.y > -10.0,
                "a corner beside the flight: {way:?}"
            );
        }
    };
    let mut g = crate::ai::tests::l_graph();
    // The mouth: a wall across it on both sides, so the flight is the way.
    g.barriers = rails.to_vec();
    g.barriers.push(bar(16.2, -10.0, 15.0, 0.1));
    g.barriers.push(bar(-16.2, -10.0, 15.0, 0.1));
    let (a, b) = (Vec2::new(1.6, -4.0), Vec2::new(0.0, -24.0));
    assert!(
        g.detour(a, b, DETOUR_RADIUS, nav::SOLDIER)
            .is_none_or(|w| w.iter().any(|c| c.x.abs() > 1.0)),
        "the wide margin finds the flight"
    );
    let way = soldier_way(&g, a, b).expect("a way up");
    assert!(!way.is_empty(), "the straight leg grazes the rail's end");
    up_the_flight(&g, a, b, &way);
    // Open floor beside the flight; the waypoint on it 0.7 m from the right
    // rail, which the wide margin ignores: its way crosses that rail.
    g.barriers = rails.to_vec();
    let (a, b) = (Vec2::new(3.0, -4.0), Vec2::new(0.3, -14.0));
    let wide = g
        .detour(a, b, DETOUR_RADIUS, nav::SOLDIER)
        .expect("a wide way");
    let mut at = a;
    let crossed = wide.iter().chain(std::iter::once(&b)).any(|&c| {
        let x = g.leg_crosses_barrier(at, c, crate::unit::SOLDIER_RADIUS, nav::SOLDIER);
        at = c;
        x.is_some()
    });
    assert!(crossed, "the wide way keeps the rail: {wide:?}");
    let way = soldier_way(&g, a, b).expect("a way up");
    up_the_flight(&g, a, b, &way);
}

/// BF2's arc flag 2 (RE_LOG "BF2 arc flags"): the leg starts with a jump. One jump for
/// the arc, however long it takes to walk, and none on a plain arc.
#[test]
fn a_jump_arc_starts_with_one_jump_and_a_plain_arc_has_none() {
    for (flags, want) in [(0, 0), (Arc::JUMP, 1)] {
        let (mut sim, me) = l_sim();
        let g = sim.nav.take().expect("graph");
        let mut arcs = g.arcs;
        for arc in &mut arcs {
            arc.flags = flags;
        }
        sim.nav =
            Some(NavGraph::new(g.hubs, arcs, RouteTable::empty(3), vec![], vec![]).expect("graph"));
        let (mut jumps, mut was, mut leg) = (0, false, None);
        for _ in 0..300 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(me).expect("alive");
            let via = u.ai.nav.route.and_then(|r| r.via_arc);
            if via.is_some() && leg.is_none() {
                leg = via;
            }
            if via != leg {
                break;
            }
            let now = u.soldier.stance == crate::soldier::Stance::Jump;
            jumps += i32::from(now && !was);
            was = now;
        }
        assert!(leg.is_some(), "the bot walked an arc");
        assert_eq!(jumps, want, "flags {flags}");
    }
}

/// Card WAI: [`crate::ai::RIDE_GAIN`] is judged once, when a walker is
/// claimed (`ride::routes_to`). A ride under way to the hub nearest the post
/// keeps its leg however little gain is left (the gain falls to 0 as the
/// walker reaches that hub), and a fresh plan from near the drop hub still
/// gets a leg to it: the pilot is not put out 40 m short, and a command
/// walker is not left standing.
#[test]
fn a_walker_ride_to_a_drop_hub_is_not_judged_again_on_the_way() {
    use crate::cp::CpDef;
    use crate::heightfield::Heightfield;
    use crate::nav::NavGraph;
    use crate::region::Region;
    use crate::unit::{Controller, UnitSpec};
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let mut s = Sim::empty(3);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
    // ai_size 8: the walker routes as type 3.
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 8,
        guns: Vec::new(),
    })];
    let post = Vec3::new(0.0, 0.0, 150.0);
    s.add_cp(
        CpDef {
            name: "far".into(),
            pos: post,
            capture: Region::sphere(post, 8.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    // Hub 1 is the drop hub: the walker's type reaches it, the last arc to
    // the post's hub 2 is soldiers' only.
    let hub = |z: f32, radius: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(0.0, 0.0, z),
        radius,
        arcs,
    };
    s.nav = NavGraph::new(
        vec![
            hub(0.0, 10.0, vec![0]),
            hub(100.0, 4.0, vec![0, 1]),
            hub(150.0, 4.0, vec![1]),
        ],
        vec![
            Arc {
                a: 0,
                b: 1,
                type_mask: 1 | 1 << 3,
                flags: 0,
            },
            Arc {
                a: 1,
                b: 2,
                type_mask: 1,
                flags: 0,
            },
        ],
        RouteTable::empty(3),
        vec![],
        vec![],
    );
    s.cps[0].nav_hub = Some(2);
    let b = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 8000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    assert!(s.seat_unit(b, v, 0));
    let place = |s: &mut Sim, z: f32| {
        let p = Vec3::new(0.0, 0.0, z);
        s.vehicle_mut(v).unwrap().pos = p;
        s.unit_mut(b).unwrap().soldier.pos = p;
    };

    // Mid-ride, 30 m of gain left: the leg to the drop hub is kept.
    place(&mut s, 70.0);
    let leg = Route {
        dest_hub: 1,
        next_hub: 1,
        waypoint: Vec3::new(0.0, 0.0, 100.0),
        ..Default::default()
    };
    let mut st = AiState {
        target_cp: Some(0),
        nav: crate::ai::route::NavState::for_fixture(Some(leg)),
        ..Default::default()
    };
    let mut nt = NavTick::default();
    retarget_to_post(&s, b, &mut st, 0, false, &mut nt);
    assert_eq!(
        st.nav.route.map(|r| r.dest_hub),
        Some(1),
        "leg dropped mid-ride"
    );
    assert_eq!(nt.unroutable, 0);

    // No route yet, 25 m of gain: a leg to the drop hub is planned.
    place(&mut s, 75.0);
    let mut st = AiState::default();
    let mut nt = NavTick::default();
    retarget_to_post(&s, b, &mut st, 0, false, &mut nt);
    assert_eq!(st.nav.route.map(|r| r.dest_hub), Some(1), "no leg planned");
    assert_eq!(nt.unroutable, 0);
}
