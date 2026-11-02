//! The planning graph's tests: locate (the PS2's and the unit locate), the
//! chooser, the BFS, jet arcs, the route table's bytes, barriers and the
//! corner walk (card LOC3 moved them out of `nav.rs`, unchanged but for the
//! locate cases).

use super::*;

fn hub(x: f32, z: f32, r: f32, arcs: &[u16]) -> Hub {
    Hub {
        pos: Vec3::new(x, 0.0, z),
        radius: r,
        arcs: arcs.to_vec(),
    }
}

fn arc(a: u16, b: u16) -> Arc {
    Arc {
        a,
        b,
        type_mask: 1,
        flags: 0,
    }
}

/// 0 --0-- 1 --1-- 2, plus a long way round 0 --2-- 3 --3-- 2.
fn graph() -> NavGraph {
    let hubs = vec![
        hub(0.0, 0.0, 2.0, &[0, 2]),
        hub(10.0, 0.0, 2.0, &[0, 1]),
        hub(20.0, 0.0, 2.0, &[1, 3]),
        hub(10.0, 40.0, 2.0, &[2, 3]),
    ];
    let arcs = vec![arc(0, 1), arc(1, 2), arc(0, 3), arc(3, 2)];
    NavGraph::new(hubs, arcs, RouteTable::empty(4), Vec::new(), Vec::new()).expect("graph")
}

/// Card WAI2: BF2 Hoth's AT-AT stands inside a soldier hub laid over the
/// first hub of its own path — it is located on the hub its type has an
/// arc at, in a disc or as the nearest; a soldier, and a type with no
/// arc anywhere, take the plain answer.
#[test]
fn a_vehicle_type_locates_on_a_hub_of_its_own_graph() {
    let huge = Arc {
        type_mask: 16,
        ..arc(1, 2)
    };
    let g = NavGraph::new(
        vec![
            hub(0.0, 0.0, 6.0, &[0]),
            hub(1.0, 0.0, 8.0, &[1]),
            hub(50.0, 0.0, 2.0, &[1]),
            hub(0.0, 30.0, 2.0, &[0]),
        ],
        vec![arc(0, 3), huge],
        RouteTable::empty(4),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    let at = Vec3::ZERO;
    assert_eq!(g.hub_for(at, Mover::SOLDIER), Some(0));
    assert_eq!(g.hub_for(at, Mover::new(4, false)), Some(1));
    assert_eq!(g.hub_for(at, Mover::new(2, false)), Some(0));
    let out = Vec3::new(0.0, 0.0, 25.0);
    assert_eq!(g.hub_for(out, Mover::SOLDIER), Some(3));
    assert_eq!(g.hub_for(out, Mover::new(4, false)), Some(1));
}

/// Card HUBM: TAT1's 525 m dune-sea hub has no arc. A soldier inside
/// only that disc is on the corridor it stands in, else at the nearer
/// end of the nearest arc it may use — never in the arcless hub, and
/// not at a nearer hub its type has no arc at.
#[test]
fn a_hub_with_no_arc_for_the_mover_is_not_located() {
    let huge = Arc {
        type_mask: 16,
        ..arc(4, 5)
    };
    let g = NavGraph::new(
        vec![
            hub(0.0, 0.0, 500.0, &[]),
            hub(100.0, 0.0, 2.0, &[0]),
            hub(140.0, 0.0, 4.0, &[0]),
            hub(-200.0, 0.0, 500.0, &[]),
            hub(-20.0, 0.0, 1.0, &[1]),
            hub(-20.0, 30.0, 1.0, &[1]),
        ],
        vec![arc(1, 2), huge],
        RouteTable::empty(6),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    // In the corridor of arc 0 (and in both arcless discs).
    match g.locate(Vec3::new(120.0, 0.0, 1.0), Mover::SOLDIER) {
        Some(Locus::Arc(0, t)) => assert!((t - 0.5).abs() < 1e-3, "t {t}"),
        other => panic!("expected the corridor, got {other:?}"),
    }
    // In the open: hub 4 is the nearest hub, but a soldier has no arc
    // there; the nearest soldier arc's nearer end is hub 1.
    let open = Vec3::new(-10.0, 0.0, 0.0);
    assert!(matches!(
        g.locate(open, Mover::SOLDIER),
        Some(Locus::Nearest(0, _))
    ));
    assert_eq!(g.hub_for(open, Mover::SOLDIER), Some(1));
    assert_eq!(g.hub_for(open, Mover::new(4, false)), Some(4));
    // Past the far end: that end.
    assert_eq!(
        g.hub_for(Vec3::new(300.0, 0.0, 9.0), Mover::SOLDIER),
        Some(2)
    );
    // Along both discs, for both types: never hub 0 or 3.
    for x in (-600..500).step_by(7) {
        for ty in [SOLDIER, 4] {
            let h = g.hub_for(Vec3::new(x as f32, 0.0, 13.0), Mover::new(ty, false));
            assert!(!matches!(h, Some(0 | 3) | None), "x {x} type {ty}: {h:?}");
        }
    }
}

#[test]
fn locate_takes_the_smallest_containing_hub() {
    // Two hubs on the same spot: the tight one wins even though the wide
    // one is listed first.
    let g = NavGraph::new(
        vec![hub(0.0, 0.0, 9.0, &[]), hub(1.0, 0.0, 2.0, &[])],
        Vec::new(),
        RouteTable::empty(2),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    assert_eq!(
        g.locate(Vec3::new(1.0, 0.0, 0.0), Mover::SOLDIER),
        Some(Locus::Hub(1))
    );
    // BF1's hub is a SPHERE: 3 m up is outside the tight one and inside
    // the wide one; BF2's is an XZ disc: 300 m up is still inside.
    assert_eq!(
        g.locate(Vec3::new(1.0, 3.0, 0.0), Mover::SOLDIER),
        Some(Locus::Hub(0))
    );
    let mut discs = g.clone();
    discs.hub_discs = true;
    assert_eq!(
        discs.locate(Vec3::new(1.0, 300.0, 0.0), Mover::SOLDIER),
        Some(Locus::Hub(1))
    );
    // Outside both radii and no arcs: the nearest hub (ours).
    assert_eq!(
        g.locate(Vec3::new(40.0, 0.0, 0.0), Mover::SOLDIER),
        Some(Locus::Hub(1))
    );
}

#[test]
fn arc_corridor_half_width_interpolates_the_end_radii() {
    // Hub 0 r = 1 at x 0, hub 1 r = 5 at x 10: the corridor is a wedge, so
    // 3 m off the segment is outside near hub 0 and inside near hub 1.
    let g = NavGraph::new(
        vec![hub(0.0, 0.0, 1.0, &[0]), hub(10.0, 0.0, 5.0, &[0])],
        vec![arc(0, 1)],
        RouteTable::empty(2),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    // 3 m off the axis: outside the 1.4 m wedge at x = 1 ...
    assert_eq!(
        g.locate(Vec3::new(1.0, 0.0, 3.0), Mover::SOLDIER),
        Some(Locus::Nearest(0, 0.1)),
        "wedge is 1.4 m wide at x = 1"
    );
    // ... and 2.5 m off it at x = 4 is inside the 2.6 m wedge there.
    match g.locate(Vec3::new(4.0, 0.0, 2.5), Mover::SOLDIER) {
        Some(Locus::Arc(0, t)) => assert!((t - 0.4).abs() < 1e-3, "t {t}"),
        other => panic!("expected the corridor, got {other:?}"),
    }
    // An arc the type may not use is not a corridor for it.
    let mut g2 = g.clone();
    g2.arcs[0].type_mask = 2;
    assert_eq!(
        g2.locate(Vec3::new(4.0, 0.0, 2.5), Mover::SOLDIER),
        Some(Locus::Hub(0))
    );
    // hub_for takes the nearer end of the arc.
    assert_eq!(g.hub_for(Vec3::new(4.0, 0.0, 2.5), Mover::SOLDIER), Some(0));
    assert_eq!(g.hub_for(Vec3::new(6.0, 0.0, 3.3), Mover::SOLDIER), Some(1));
}

/// `weight << 3 | slot`, one row per alternative.
fn entry(weight: u8, slot: u8) -> u8 {
    (weight << 3) | slot
}

#[test]
fn next_hop_takes_the_first_alternative_and_skips_the_arrival_arc() {
    let mut g = graph();
    // At hub 1 (slots [arc0, arc1]) heading for hub 2: row 0 says slot 1
    // with weight 25, row 1 says slot 0 with weight 6 (they sum to 31).
    let hub_count = 4;
    let mut rows = vec![Vec::new(); TYPE_COUNT * hub_count];
    let mut r0 = vec![0u8; hub_count];
    let mut r1 = vec![0u8; hub_count];
    r0[2] = entry(25, 1);
    r1[2] = entry(6, 0);
    let mut run = r0.clone();
    run.extend_from_slice(&r1);
    rows[SOLDIER * hub_count + 1] = run;
    g.routes = RouteTable::from_rows(hub_count, TYPE_COUNT, rows);

    // r = 0 → the first (highest-weight) alternative: arc 1 to hub 2.
    assert_eq!(
        g.next_hop(1, 2, Mover::SOLDIER, None, None, 0),
        Some((1, 2))
    );
    // r = 30 → past the 25 of row 0, into row 1: arc 0 back to hub 0.
    assert_eq!(
        g.next_hop(1, 2, Mover::SOLDIER, None, None, 30),
        Some((0, 0))
    );
    // Arriving on arc 1 (weight 25) leaves budget 6, so every r lands in
    // row 1 — and the arrival arc is never chosen.
    for r in 0..6 {
        assert_eq!(
            g.next_hop(1, 2, Mover::SOLDIER, Some(1), None, r),
            Some((0, 0)),
            "r {r}"
        );
    }
    // No entries at all: the table has nothing, so the caller must fall
    // back to BFS.
    assert_eq!(g.next_hop(0, 2, Mover::SOLDIER, None, None, 0), None);
}

#[test]
fn next_hop_dead_end_leaves_by_another_arc() {
    let mut g = graph();
    let hub_count = 4;
    let mut rows = vec![Vec::new(); TYPE_COUNT * hub_count];
    // At hub 1, the ONLY listed way to hub 2 is slot 0 (arc 0) with the
    // full weight 31 — arriving on it leaves budget 0.
    let mut r0 = vec![0u8; hub_count];
    r0[2] = entry(31, 0);
    rows[SOLDIER * hub_count + 1] = r0;
    g.routes = RouteTable::from_rows(hub_count, TYPE_COUNT, rows);
    // Arrived on arc 0: budget 0 → some OTHER arc of hub 1 (only arc 1).
    assert_eq!(
        g.next_hop(1, 2, Mover::SOLDIER, Some(0), None, 7),
        Some((1, 2))
    );
    // With no other arc allowed for the type there is nowhere to go.
    g.arcs[1].type_mask = 2;
    assert_eq!(g.next_hop(1, 2, Mover::SOLDIER, Some(0), None, 7), None);
}

#[test]
fn bfs_finds_a_hop_the_table_lacks_and_none_when_disconnected() {
    let g = graph(); // empty table everywhere
    assert_eq!(g.next_hop(0, 2, Mover::SOLDIER, None, None, 0), None);
    // 0 → 1 → 2 is two arcs, 0 → 3 → 2 also two; arc-index order breaks
    // the tie, so the first hop is arc 0.
    assert_eq!(g.route_bfs(0, 2, Mover::SOLDIER, &|_| true), Some((0, 1)));
    assert_eq!(g.route_bfs(2, 0, Mover::SOLDIER, &|_| true), Some((1, 1)));
    assert_eq!(
        g.route_bfs(0, 0, Mover::SOLDIER, &|_| true),
        None,
        "already there"
    );
    // A lone hub reaches nothing.
    let mut hubs = g.hubs.clone();
    hubs.push(hub(500.0, 0.0, 2.0, &[]));
    let g2 =
        NavGraph::new(hubs, g.arcs.clone(), RouteTable::empty(5), vec![], vec![]).expect("graph");
    assert_eq!(g2.route_bfs(0, 4, Mover::SOLDIER, &|_| true), None);
    assert!(!g2.reachable(0, 4, SOLDIER));
    assert!(g2.reachable(0, 2, SOLDIER));
    // HOVER (bit 1) is on no arc here, so nothing is reachable for it.
    assert!(!g2.reachable(0, 2, 1));
    assert_eq!(g2.route_bfs(0, 2, Mover::new(1, false), &|_| true), None);
}

/// A `jet_jump` arc is OFF a jetless soldier's graph everywhere the graph
/// is asked — `locate`, `next_hop`, `route_bfs`, and the connectivity the
/// `reachable` cache answers from — while a plain `jump` arc stays
/// routable (PLAN "PHASE A.2 DESIGN" decision 4).
#[test]
fn a_jet_jump_arc_is_off_a_jetless_soldiers_graph() {
    let mut g = graph();
    // Arc 0 (0 -> 1) becomes the jet leap; arc 1 (1 -> 2) a plain jump.
    g.arcs[0].flags = Arc::JET_JUMP;
    g.arcs[1].flags = Arc::JUMP;
    let g = NavGraph::new(
        g.hubs.clone(),
        g.arcs.clone(),
        RouteTable::empty(4),
        vec![],
        vec![],
    )
    .expect("graph");
    let jet = Mover::new(SOLDIER, true);

    // The census the sweep prints.
    assert_eq!(g.arc_flag_census(SOLDIER), (1, 1), "(jump, jet_jump)");

    // 1. `route_bfs`: without a jet the short way (arc 0) is gone, so the
    //    long way round (arc 2) is the first hop. With one, arc 0 is back.
    assert_eq!(g.route_bfs(0, 2, Mover::SOLDIER, &|_| true), Some((2, 3)));
    assert_eq!(g.route_bfs(0, 2, jet, &|_| true), Some((0, 1)));
    // The plain `jump` arc 1 is on BOTH answers — it is still routable.
    assert_eq!(g.route_bfs(1, 2, Mover::SOLDIER, &|_| true), Some((1, 2)));

    // 2. `next_hop`: the table names slot 0 of hub 0, which IS the jet
    //    arc, so a jetless soldier is left with hub 0's other arc and a
    //    jet trooper takes the one the table asked for.
    let hub_count = 4;
    let mut rows = vec![Vec::new(); TYPE_COUNT * hub_count];
    let mut r0 = vec![0u8; hub_count];
    r0[2] = entry(31, 0); // hub 0 slot 0 = arc 0 = the jet leap
    rows[SOLDIER * hub_count] = r0;
    let mut g2 = g;
    g2.routes = RouteTable::from_rows(hub_count, TYPE_COUNT, rows);
    assert_eq!(g2.next_hop(0, 2, jet, None, None, 0), Some((0, 1)));
    assert_eq!(
        g2.next_hop(0, 2, Mover::SOLDIER, None, None, 0),
        Some((2, 3))
    );

    // 3. `locate`: no unit is IN the corridor of a flagged arc (the
    //    PS2's step 2 skips flags & 7); as the nearest arc it is a jet
    //    trooper's and not a jetless soldier's.
    let p = Vec3::new(5.0, 0.0, 0.0);
    assert!(matches!(g2.locate(p, jet), Some(Locus::Nearest(0, _))));
    assert!(matches!(
        g2.locate(p, Mover::SOLDIER),
        Some(Locus::Nearest(j, _)) if j != 0
    ));

    // 4. Connectivity: cut the long way round and a jetless soldier can no
    //    longer reach hub 2 at all, though the arcs still join them.
    let mut arcs = g2.arcs.clone();
    arcs[2].type_mask = 2;
    arcs[3].type_mask = 2;
    let g3 =
        NavGraph::new(g2.hubs.clone(), arcs, RouteTable::empty(4), vec![], vec![]).expect("graph");
    assert!(!g3.reachable(0, 2, SOLDIER), "only the jet leap is left");
    assert_eq!(g3.route_bfs(0, 2, Mover::SOLDIER, &|_| true), None);
    assert_eq!(g3.route_bfs(0, 2, jet, &|_| true), Some((0, 1)));
    assert!(!g3.reachable_by(0, 2, Mover::SOLDIER));
    assert!(g3.reachable_by(0, 2, jet));
    // The intermediate floor filter still applies; the destination is exempt.
    assert_eq!(g3.route_bfs(0, 2, jet, &|h| h != 1), None);
    assert_eq!(g3.route_bfs(0, 2, jet, &|h| h != 2), Some((0, 1)));
}

/// The two Phase A.2 filters the follower drives `next_hop` and
/// `route_bfs` with when a leg's waypoint turned out to have no floor at
/// the unit's level (decision 3): the arc it just rejected, and the hubs
/// that have no floor.
#[test]
fn a_rejected_arc_and_a_floorless_hub_are_taken_off_the_search() {
    let mut g = graph();
    let hub_count = 4;
    let mut rows = vec![Vec::new(); TYPE_COUNT * hub_count];
    // Hub 0 -> hub 2: slot 0 (arc 0, via hub 1) weight 20, slot 1 (arc 2,
    // via hub 3) weight 11.
    let mut r0 = vec![0u8; hub_count];
    let mut r1 = vec![0u8; hub_count];
    r0[2] = entry(20, 0);
    r1[2] = entry(11, 1);
    let mut run = r0.clone();
    run.extend_from_slice(&r1);
    rows[SOLDIER * hub_count] = run;
    g.routes = RouteTable::from_rows(hub_count, TYPE_COUNT, rows);

    // r = 0 takes the first alternative, arc 0.
    assert_eq!(
        g.next_hop(0, 2, Mover::SOLDIER, None, None, 0),
        Some((0, 1))
    );
    // Re-asked with arc 0 EXCLUDED, the weight of its slot comes out of
    // the budget (31 - 20 = 11) and every r lands on the other one.
    for r in 0..11 {
        assert_eq!(
            g.next_hop(0, 2, Mover::SOLDIER, None, Some(0), r),
            Some((2, 3)),
            "r {r}"
        );
    }
    // Excluding BOTH leaves the budget empty, which is the PS2's dead end:
    // any other arc of hub 0 — and there is none.
    assert_eq!(g.next_hop(0, 2, Mover::SOLDIER, Some(2), Some(0), 0), None);

    // `route_bfs` with a floor predicate: hub 1 has no floor at this
    // unit's level, so the search goes the long way round instead.
    assert_eq!(g.route_bfs(0, 2, Mover::SOLDIER, &|_| true), Some((0, 1)));
    assert_eq!(g.route_bfs(0, 2, Mover::SOLDIER, &|h| h != 1), Some((2, 3)));
    // The DESTINATION is exempt — it is the goal, not a hub to stand on.
    assert_eq!(g.route_bfs(0, 2, Mover::SOLDIER, &|h| h != 2), Some((0, 1)));
    // With every intermediate hub floorless there is no path at all.
    assert_eq!(g.route_bfs(0, 2, Mover::SOLDIER, &|h| h == 2), None);
}

/// The START hub prefers a containing hub whose XZ has a floor at the
/// unit's own level, and only falls back to the plain smallest-radius
/// answer when none of them has one (decision 3).
#[test]
fn the_start_hub_prefers_a_containing_hub_with_a_floor() {
    // Both hubs contain the origin: hub 0 is the bigger (r 6 at x -3),
    // hub 1 the smaller (r 2 at x 1.5), so the plain answer is hub 1.
    let g = NavGraph::new(
        vec![hub(-3.0, 0.0, 6.0, &[]), hub(1.5, 0.0, 2.0, &[])],
        Vec::new(),
        RouteTable::empty(2),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    let p = Vec3::ZERO;
    assert_eq!(g.hub_for(p, Mover::SOLDIER), Some(1), "smallest radius");
    // Hub 1's XZ has no floor on this level: the bigger hub wins instead.
    assert_eq!(g.hub_for_on(p, Mover::SOLDIER, &|h| h != 1), Some(0));
    assert_eq!(
        g.locate_on(p, Mover::SOLDIER, &|h| h != 1),
        Some(Locus::Hub(0))
    );
    // Neither has one: the old answer stands rather than nothing.
    assert_eq!(g.hub_for_on(p, Mover::SOLDIER, &|_| false), Some(1));
    // And the preference never overrides a SMALLER hub that does have a
    // floor: with both passing, the smallest still wins.
    assert_eq!(g.hub_for_on(p, Mover::SOLDIER, &|_| true), Some(1));
}

#[test]
fn route_table_round_trips_the_sidecar_bytes() {
    let hub_count = 2usize;
    let mut bin = Vec::new();
    bin.extend((hub_count as u16).to_le_bytes());
    bin.push(TYPE_COUNT as u8);
    for t in 0..TYPE_COUNT {
        for h in 0..hub_count {
            // One row for SOLDIER hub 0, none anywhere else.
            if t == SOLDIER && h == 0 {
                bin.push(1);
                bin.extend_from_slice(&[0, entry(31, 0)]);
            } else {
                bin.push(0);
            }
        }
    }
    let t = RouteTable::parse(&bin).expect("parses");
    assert_eq!((t.hub_count, t.types), (hub_count, TYPE_COUNT));
    assert_eq!(
        t.entries(SOLDIER, 0, 1).collect::<Vec<_>>(),
        vec![entry(31, 0)]
    );
    assert_eq!(t.entries(SOLDIER, 0, 0).count(), 0, "self entry is 0");
    assert_eq!(t.entries(SOLDIER, 1, 0).count(), 0);
    assert_eq!(t.entries(4, 0, 1).count(), 0);
    // Truncation is an error, not a silent half-table.
    assert!(RouteTable::parse(&bin[..bin.len() - 1]).is_err());
    assert!(RouteTable::parse(&[1, 0]).is_err());
}

#[test]
fn barrier_contains_and_crosses_in_its_own_frame() {
    // A 4 x 1 rectangle turned 45 degrees about y.
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let b = Barrier {
        centre: Vec2::new(0.0, 0.0),
        axes: [Vec2::new(s, s), Vec2::new(-s, s)],
        half: Vec2::new(4.0, 1.0),
        mask: 1,
        enabled: true,
    };
    assert!(b.stops(SOLDIER) && !b.stops(1));
    assert!(b.contains(Vec2::new(2.0, 2.0)), "along the long axis");
    assert!(!b.contains(Vec2::new(-2.0, 2.0)), "across the short axis");
    assert!(b.contains(Vec2::ZERO));
    // A segment straight across the middle crosses; one well outside does
    // not.
    assert!(b.crosses(Vec2::new(-3.0, 3.0), Vec2::new(3.0, -3.0)));
    assert!(!b.crosses(Vec2::new(-9.0, 9.0), Vec2::new(-8.0, 8.0)));
    // Parallel and outside the slab.
    assert!(!b.crosses(Vec2::new(-9.0, 5.0), Vec2::new(9.0, 5.0)));
    // The radius margin grows the rectangle on every side.
    assert!(!b.contains_r(Vec2::new(-2.0, 2.0), 0.5));
    assert!(b.contains_r(Vec2::new(-2.0, 2.0), 2.0));
    assert!(!b.crosses_r(Vec2::new(-9.0, 5.0), Vec2::new(9.0, 5.0), 1.0));
    assert!(b.crosses_r(Vec2::new(-9.0, 5.0), Vec2::new(9.0, 5.0), 3.0));
    // The corners are the PS2's order (+A+B, +A-B, -A-B, -A+B) and the
    // expanded rectangle contains all four.
    let c = b.corners_r(0.0);
    assert!((c[0] - (b.axes[0] * 4.0 + b.axes[1])).length() < 1e-5);
    assert!((c[2] + c[0]).length() < 1e-5, "opposite corners");
    for p in b.corners_r(1.0) {
        assert!(b.contains_r(p, 1.001), "{p} outside the grown rectangle");
        assert!(!b.contains(p), "{p} is not inside the raw one");
    }
}

/// A graph with two hubs and whatever barriers the test needs. The hubs
/// are only there because [`NavGraph::new`] refuses an empty plan; the
/// corner walk never looks at them.
fn barrier_graph(barriers: Vec<Barrier>) -> NavGraph {
    let hub = |x: f32, z: f32| Hub {
        pos: Vec3::new(x, 0.0, z),
        radius: 2.0,
        arcs: Vec::new(),
    };
    NavGraph::new(
        vec![hub(0.0, 0.0), hub(0.0, -40.0)],
        Vec::new(),
        RouteTable::empty(2),
        barriers,
        Vec::new(),
    )
    .expect("graph")
}

/// An axis-aligned rectangle (half extents) that stops SOLDIER.
fn wall(cx: f32, cz: f32, hx: f32, hz: f32) -> Barrier {
    Barrier {
        centre: Vec2::new(cx, cz),
        axes: [Vec2::X, Vec2::Y],
        half: Vec2::new(hx, hz),
        mask: 1,
        enabled: true,
    }
}

/// A leg straight through a rectangle is reported and detoured around one
/// of its corners; the corner path is clear and the leg without the
/// barrier is not detoured at all.
#[test]
fn a_leg_across_a_barrier_walks_its_corner() {
    // A 20 x 2 m wall across the z = 0 line, the leg running -z to +z.
    let g = barrier_graph(vec![wall(0.0, 0.0, 10.0, 1.0)]);
    let (a, b) = (Vec2::new(0.0, -10.0), Vec2::new(0.0, 10.0));
    let r = 1.0;
    assert_eq!(g.leg_crosses_barrier(a, b, r, SOLDIER), Some(0));
    // A different type is not stopped by a SOLDIER-only barrier.
    assert_eq!(g.leg_crosses_barrier(a, b, r, 1), None);
    // ... and neither is anything once the barrier is closed off.
    let mut off = g.clone();
    off.barriers[0].enabled = false;
    assert_eq!(off.leg_crosses_barrier(a, b, r, SOLDIER), None);
    assert_eq!(off.detour(a, b, r, SOLDIER), Some(Vec::new()));

    let path = g.detour(a, b, r, SOLDIER).expect("a way round");
    assert!(!path.is_empty(), "the straight leg is blocked");
    // Every leg of the corner path is clear, and it goes round an END of
    // the wall (|x| past the 10 m half extent), not through it.
    let mut prev = a;
    for &p in &path {
        assert_eq!(g.leg_crosses_barrier(prev, p, r, SOLDIER), None, "{p}");
        prev = p;
    }
    assert_eq!(g.leg_crosses_barrier(prev, b, r, SOLDIER), None);
    assert!(
        path.iter().any(|p| p.x.abs() > 10.0),
        "went round an end: {path:?}"
    );
    // With the wall gone the leg is straight.
    let clear = barrier_graph(Vec::new());
    assert_eq!(clear.detour(a, b, r, SOLDIER), Some(Vec::new()));
}

/// A leg whose START is inside a barrier ignores THAT barrier (twenty of
/// the discs' hubs sit inside one) and still routes round the others.
#[test]
fn a_barrier_containing_an_end_point_is_ignored_for_that_leg() {
    let g = barrier_graph(vec![
        wall(0.0, -10.0, 4.0, 4.0), // the start sits inside this one
        wall(0.0, 0.0, 10.0, 1.0),  // and this one is really in the way
    ]);
    let (a, b) = (Vec2::new(0.0, -10.0), Vec2::new(0.0, 10.0));
    let r = 1.0;
    assert!(g.barriers[0].contains_r(a, r), "the start is inside it");
    assert_eq!(
        g.leg_crosses_barrier(a, b, r, SOLDIER),
        Some(1),
        "barrier 0 is ignored, barrier 1 is the answer"
    );
    let path = g.detour(a, b, r, SOLDIER).expect("a way round");
    assert!(!path.is_empty());
    // The same leg with only the containing barrier is simply clear.
    let only = barrier_graph(vec![wall(0.0, -10.0, 4.0, 4.0)]);
    assert_eq!(only.detour(a, b, r, SOLDIER), Some(Vec::new()));
}

/// Two wall segments with a 6 m gap between them: the detour goes THROUGH
/// the gap, not round the outside of either.
#[test]
fn a_wall_with_a_gap_is_routed_through_the_gap() {
    let g = barrier_graph(vec![
        wall(-23.0, 0.0, 20.0, 1.0), // x in [-43, -3]
        wall(23.0, 0.0, 20.0, 1.0),  // x in [3, 43]
    ]);
    let (a, b) = (Vec2::new(-20.0, -10.0), Vec2::new(-20.0, 10.0));
    let r = 0.5;
    assert_eq!(g.leg_crosses_barrier(a, b, r, SOLDIER), Some(0));
    let path = g.detour(a, b, r, SOLDIER).expect("through the gap");
    let mut prev = a;
    for &p in &path {
        assert_eq!(g.leg_crosses_barrier(prev, p, r, SOLDIER), None, "{p}");
        prev = p;
    }
    assert_eq!(g.leg_crosses_barrier(prev, b, r, SOLDIER), None);
    // The gap is x in (-3, 3); the ends are at -43 and +43.
    assert!(
        path.iter().all(|p| p.x.abs() < 10.0),
        "used the gap, not an outside end: {path:?}"
    );
}

/// A closed box round the goal: there is no chain of corners into it, so
/// the search says so instead of inventing one.
#[test]
fn a_walled_off_goal_has_no_detour() {
    let g = barrier_graph(vec![
        wall(0.0, 6.0, 6.0, 0.5),
        wall(0.0, -6.0, 6.0, 0.5),
        wall(6.0, 0.0, 0.5, 6.0),
        wall(-6.0, 0.0, 0.5, 6.0),
    ]);
    let (a, b) = (Vec2::new(0.0, -30.0), Vec2::ZERO);
    assert!(g.leg_crosses_barrier(a, b, 0.5, SOLDIER).is_some());
    assert_eq!(g.detour(a, b, 0.5, SOLDIER), None, "no way in");
    // The same walls with one side removed DO let a unit in.
    let mut open = g.clone();
    open.barriers.remove(1);
    assert!(open.detour(a, b, 0.5, SOLDIER).is_some(), "the gap is open");
}

/// A rail: a barrier of half length `hl` and half width `hw` centred at
/// `c`, its long side along the direction `ang` (radians from +x).
fn rail(c: Vec2, ang: f32, hl: f32, hw: f32) -> Barrier {
    let along = Vec2::new(ang.cos(), ang.sin());
    Barrier {
        centre: c,
        axes: [along, along.perp()],
        half: Vec2::new(hl, hw),
        mask: 1,
        enabled: true,
    }
}

/// Is every leg of `a -> path.. -> b` clear of every barrier?
fn path_is_clear(g: &NavGraph, a: Vec2, path: &[Vec2], b: Vec2, r: f32) -> bool {
    let mut prev = a;
    for &p in path.iter().chain(std::iter::once(&b)) {
        if g.leg_crosses_barrier(prev, p, r, SOLDIER).is_some() {
            return false;
        }
        prev = p;
    }
    true
}

/// Card OBST2: the barrier a leg crosses is gone round however many others
/// stand in the band. (The corner search used to take the first 24 barriers
/// of the band in index order: with the crossed wall after them it answered
/// "the straight leg is clear" - 22,268 of 68,186 crossing legs on the
/// discs, which hold 30-60 barriers a band at the median.)
#[test]
fn the_barrier_crossed_is_rounded_in_a_crowded_band() {
    // Thirty posts beside the leg, in its band; then the wall across it.
    let mut walls: Vec<Barrier> = (0..30)
        .map(|i| wall(-20.0 + (i % 2) as f32 * 40.0, -14.0 + i as f32, 0.3, 0.3))
        .collect();
    walls.push(wall(0.0, 0.0, 10.0, 1.0));
    let g = barrier_graph(walls);
    let (a, b) = (Vec2::new(0.0, -10.0), Vec2::new(0.0, 10.0));
    let r = 1.0;
    assert_eq!(g.leg_crosses_barrier(a, b, r, SOLDIER), Some(30));
    let path = g.detour(a, b, r, SOLDIER).expect("a way round");
    assert!(!path.is_empty(), "the straight leg is blocked");
    assert!(path_is_clear(&g, a, &path, b, r), "{path:?}");
    assert!(
        path.iter().any(|p| p.x.abs() > 10.0),
        "round an end: {path:?}"
    );
}

/// Card OBST2: a ring of sixteen overlapping rails round the goal (a
/// platform's rim, BF2 Kamino) with one of them left out on the FAR side:
/// the search walks the ring round, rail by rail, to the gap - and with the
/// ring closed there is no way and it says so.
#[test]
fn a_ring_of_rails_is_walked_round_to_its_gap() {
    let ring = |skip: Option<usize>| {
        let n = 16;
        let rails = (0..n)
            .filter(|&i| Some(i) != skip)
            .map(|i| {
                let t = i as f32 * std::f32::consts::TAU / n as f32;
                let out = Vec2::new(t.cos(), t.sin());
                rail(out * 12.0, t + std::f32::consts::FRAC_PI_2, 2.6, 0.4)
            })
            .collect();
        barrier_graph(rails)
    };
    let (a, b) = (Vec2::new(0.0, -30.0), Vec2::ZERO);
    let r = 1.0;
    // Rail 4 is the one at +z, across the ring from the start.
    let g = ring(Some(4));
    let path = g.detour(a, b, r, SOLDIER).expect("round to the gap");
    assert!(path.len() >= 3, "rail by rail: {path:?}");
    assert!(path_is_clear(&g, a, &path, b, r), "{path:?}");
    assert!(
        path.iter().any(|p| p.y > 12.0),
        "in by the far side: {path:?}"
    );
    assert_eq!(ring(None).detour(a, b, r, SOLDIER), None, "a closed ring");
}

/// Both silhouette corners of a crossed barrier are tried: with one end of
/// the wall shut by a second wall, the way is round the other end.
#[test]
fn a_shut_end_is_left_for_the_other_end() {
    let g = barrier_graph(vec![
        wall(0.0, 0.0, 10.0, 1.0),  // across the leg
        wall(12.0, 0.0, 0.5, 30.0), // along it, closing the +x end
    ]);
    let (a, b) = (Vec2::new(0.0, -10.0), Vec2::new(0.0, 10.0));
    let r = 1.0;
    let path = g.detour(a, b, r, SOLDIER).expect("round the open end");
    assert!(path_is_clear(&g, a, &path, b, r), "{path:?}");
    assert!(path.iter().all(|p| p.x < 0.0), "the -x end: {path:?}");
    // The same wall with the -x end shut is rounded by +x.
    let g = barrier_graph(vec![wall(0.0, 0.0, 10.0, 1.0), wall(-12.0, 0.0, 0.5, 30.0)]);
    let path = g.detour(a, b, r, SOLDIER).expect("round the open end");
    assert!(path.iter().all(|p| p.x > 0.0), "the +x end: {path:?}");
}

/// The silhouette corners are the two a viewer sees the rectangle between:
/// facing a long side they are that side's ends, never a far corner.
#[test]
fn the_silhouette_is_the_pair_the_viewer_sees_the_barrier_between() {
    let b = wall(0.0, 0.0, 1.0, 3.0);
    let s = b.silhouette(Vec2::new(-6.0, 1.0), Vec2::new(6.0, 0.0), 0.5, 0.5);
    let (lo, hi) = (Vec2::new(-1.5, -3.5), Vec2::new(-1.5, 3.5));
    assert!(s.contains(&Some(lo)) && s.contains(&Some(hi)), "{s:?}");
    // Past its end, the far side's corner is the one to go round by.
    let s = b.silhouette(Vec2::new(-6.0, 6.0), Vec2::new(6.0, -1.0), 0.5, 0.5);
    assert!(s.contains(&Some(Vec2::new(1.5, 3.5))), "{s:?}");
    assert!(s.contains(&Some(Vec2::new(-1.5, -3.5))), "{s:?}");
    // The pair is the one seen on the TESTED rectangle (grown 0.5), placed
    // on the larger one (grown 1.0): beside the long side within the tested
    // rectangle's length (z 3.3 of 3.5) but past the bare one's (3.0), the
    // near side's two ends - asked of the bare rectangle the far side's.
    let p = Vec2::new(-2.2, 3.3);
    let s = b.silhouette(p, Vec2::new(6.0, -1.0), 0.5, 1.0);
    let (lo, hi) = (Vec2::new(-2.0, -4.0), Vec2::new(-2.0, 4.0));
    assert!(s.contains(&Some(lo)) && s.contains(&Some(hi)), "{s:?}");
    let s = b.silhouette(p, Vec2::new(6.0, -1.0), 0.0, 1.0);
    assert!(s.contains(&Some(Vec2::new(2.0, 4.0))), "{s:?}");
    // A viewer inside the placed rectangle and outside the tested one, at
    // an end of the barrier, looking through it: the far end's corner on
    // the near side - not the placed rectangle's corner behind the viewer
    // (-2, 4), which is what the pair asked of the placed one holds.
    let (p, to) = (Vec2::new(-1.8, 3.8), Vec2::new(1.8, -3.8));
    let s = b.silhouette(p, to, 0.5, 1.0);
    assert!(s.contains(&Some(Vec2::new(-2.0, -4.0))), "{s:?}");
    assert!(s.contains(&Some(Vec2::new(2.0, 4.0))), "{s:?}");
}

/// The answer is a pure function of the graph and the two points — the
/// tie order is the node index, nothing is sorted by float.
#[test]
fn the_detour_is_deterministic() {
    let g = barrier_graph(vec![wall(0.0, 0.0, 10.0, 1.0), wall(0.0, 8.0, 10.0, 1.0)]);
    let (a, b) = (Vec2::new(0.0, -10.0), Vec2::new(0.0, 20.0));
    let first = g.detour(a, b, 1.0, SOLDIER);
    assert!(first.as_ref().is_some_and(|p| !p.is_empty()));
    for _ in 0..8 {
        assert_eq!(g.detour(a, b, 1.0, SOLDIER), first);
    }
}

/// Card LOC3: where a UNIT stands is the port's own XZ reading (a named
/// deviation, `locate_unit_on`) beside the PS2's 3-D `locate` - a hub is a
/// disc whatever the game, a corridor is the first that contains the point
/// in XZ, and off both it is the nearer end of the nearest arc.
#[test]
fn a_unit_is_located_in_xz_and_a_goal_in_3d() {
    // Hub 0 r 2 and hub 1 r 2, 20 m apart, 6 m up; arc 0 between them.
    let mut hubs = vec![hub(0.0, 0.0, 2.0, &[0]), hub(20.0, 0.0, 2.0, &[0])];
    hubs[0].pos.y = 6.0;
    hubs[1].pos.y = 6.0;
    let g = NavGraph::new(
        hubs,
        vec![arc(0, 1)],
        RouteTable::empty(2),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph");
    let s = Mover::SOLDIER;
    // Under hub 0, on the ground: in its disc, 6 m outside its sphere.
    let under = Vec3::new(0.5, 0.0, 0.0);
    assert_eq!(g.locate_unit_on(under, s, &|_| true), Some(Locus::Hub(0)));
    assert!(matches!(g.locate(under, s), Some(Locus::Nearest(0, _))));
    // Under the arc, 13 m along: in its corridor in XZ, under it in 3-D.
    let mid = Vec3::new(13.0, 0.0, 1.0);
    assert!(matches!(
        g.locate_unit_on(mid, s, &|_| true),
        Some(Locus::Arc(0, _))
    ));
    assert!(matches!(g.locate(mid, s), Some(Locus::Nearest(0, _))));
    assert_eq!(g.unit_hub_on(mid, s, &|_| true), Some(1));
    // At the arc's own height both agree.
    let on = Vec3::new(13.0, 6.0, 1.0);
    assert!(matches!(g.locate(on, s), Some(Locus::Arc(0, _))));
}

mod regressions {
    // Synthetic regression coverage for ordering and clipping seams.
    use super::super::*;
    use crate::sim::Sim;

    // Frozen pre-extraction entry query: compare Option and f32 bits, including
    // public signed-zero extents, rather than substituting approximate geometry.
    fn original_entry(br: &Barrier, a: Vec2, b: Vec2) -> Option<f32> {
        if br.contains(a) {
            return None;
        }
        let local = |p: Vec2| {
            let d = p - br.centre;
            Vec2::new(d.dot(br.axes[0]), d.dot(br.axes[1]))
        };
        let (p0, p1) = (local(a), local(b));
        let d = p1 - p0;
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for k in 0..2 {
            let (p, q, h) = (d[k], p0[k], br.half[k]);
            if p.abs() < 1e-9 {
                if q.abs() > h {
                    return None;
                }
                continue;
            }
            let (mut lo, mut hi) = ((-h - q) / p, (h - q) / p);
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            t0 = t0.max(lo);
            t1 = t1.min(hi);
            if t0 > t1 {
                return None;
            }
        }
        Some(t0)
    }

    #[test]
    fn entry_clip_preserves_original_bits_and_inside_policy() {
        let points = [
            Vec2::ZERO,
            Vec2::new(-3.0, 0.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(0.0, -3.0),
            Vec2::new(0.0, 3.0),
            Vec2::new(-1.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-3.0, 1.0),
            Vec2::new(3.0, 1.0),
            Vec2::new(1e-10, -3.0),
            Vec2::new(1e-10, 3.0),
        ];
        for axes in [
            [Vec2::X, Vec2::Y],
            [Vec2::new(0.6, 0.8), Vec2::new(-0.8, 0.6)],
        ] {
            for half in [
                Vec2::ONE,
                Vec2::ZERO,
                Vec2::new(-0.0, 0.0),
                Vec2::new(0.0, -0.0),
            ] {
                let br = Barrier {
                    centre: Vec2::ZERO,
                    axes,
                    half,
                    mask: 1,
                    enabled: true,
                };
                for a in points {
                    for b in points {
                        assert_eq!(
                            br.entry_t(a, b).map(f32::to_bits),
                            original_entry(&br, a, b).map(f32::to_bits),
                            "{axes:?}, {half:?}, {a:?} -> {b:?}"
                        );
                        if br.contains(a) {
                            assert_eq!(br.entry_t(a, b), None);
                        }
                    }
                }
            }
        }
    }

    fn floor_graph() -> NavGraph {
        NavGraph {
            hubs: [0.0, 1.0, -1.0, 2.0, -2.0, f32::INFINITY]
                .into_iter()
                .map(|x| Hub {
                    pos: Vec3::new(x, 0.0, 0.0),
                    radius: 0.0,
                    arcs: vec![],
                })
                .collect(),
            ..NavGraph::default()
        }
    }

    #[test]
    fn floor_gather_preserves_ties_capacity_filters_and_sentinels() {
        let sim = Sim::empty(1);
        let g = floor_graph(); // Explicit graph works even when sim.nav is None.
        let floors = [f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0];
        let floor = |i: usize| floors.get(i).copied().filter(|f| !f.is_nan());
        let near = sim.nearest_floor_hubs::<3>(&g, Vec3::ZERO, floor);
        assert_eq!(near, [(1.0, 1), (1.0, 2), (2.0, 4)]);
        let near = sim.nearest_floor_hubs::<3>(&g, Vec3::ZERO, |i| floor(i).filter(|_| i != 1));
        assert_eq!(near[0], (1.0, 2));
        assert_eq!(near[1], (2.0, 4));
        assert_eq!(near[2], (2.0 + crate::sim::LEVEL_RISE, 3));
        let absent = sim.nearest_floor_hubs::<2>(&g, Vec3::ZERO, |i| floor(i).filter(|_| i >= 5));
        assert_eq!(absent, [(f32::INFINITY, u16::MAX); 2]);
        assert_eq!(
            sim.nearest_floor_hubs::<2>(&NavGraph::default(), Vec3::ZERO, |_| Some(0.0)),
            absent
        );
        assert_eq!(sim.nearest_floor_hubs::<0>(&g, Vec3::ZERO, floor), []);
        // NaN and infinite floor distances never displace the sentinel slots.
        assert_eq!(
            sim.nearest_floor_hubs::<2>(&g, Vec3::ZERO, |_| Some(f32::NAN)),
            absent
        );
        assert_eq!(
            sim.nearest_floor_hubs::<2>(&g, Vec3::ZERO, |_| Some(f32::INFINITY)),
            absent
        );
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn level_at_keeps_direct_indexed_short_floor_behavior() {
        let mut sim = Sim::empty(1);
        sim.nav = Some(floor_graph());
        sim.nav_floors = vec![0.0];
        sim.level_at(Vec3::ZERO);
    }
}

/// The corner search's culling changes no answer: legs across a crowded field
/// of rails, some long and some short, walk exactly the same corners with every
/// barrier tested.
#[test]
fn culling_a_corner_search_changes_no_corner() {
    let mut seed = 0x9e37_79b9u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let mut barriers = Vec::new();
    for _ in 0..300 {
        let c = Vec2::new(next() * 400.0 - 200.0, next() * 400.0 - 200.0);
        barriers.push(rail(c, next() * 3.0, 1.0 + next() * 15.0, 0.5 + next()));
    }
    let g = barrier_graph(barriers);
    let mut detours = 0;
    for k in 0..400 {
        let a = Vec2::new(next() * 400.0 - 200.0, next() * 400.0 - 200.0);
        let reach = if k % 2 == 0 { 30.0 } else { 300.0 };
        let b = a + Vec2::new(next() - 0.5, next() - 0.5) * reach;
        for radius in [0.4, 1.0] {
            let culled = g.detour_culled(a, b, radius, SOLDIER, true);
            let full = g.detour_culled(a, b, radius, SOLDIER, false);
            let bits = |p: &Option<Vec<Vec2>>| {
                p.as_ref().map(|p| {
                    p.iter()
                        .map(|c| [c.x.to_bits(), c.y.to_bits()])
                        .collect::<Vec<_>>()
                })
            };
            assert_eq!(bits(&culled), bits(&full), "{a} -> {b} at {radius}");
            detours += usize::from(culled.as_ref().is_some_and(|p| !p.is_empty()));
        }
    }
    assert!(detours > 50, "corners walked: {detours}");
}

/// The box check in front of `barrier_stop` changes no answer: every barrier's
/// own test, over random legs, gives the same bits.
#[test]
fn barrier_stop_answers_what_every_barrier_says() {
    let mut seed = 0x51ed_270bu32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let mut barriers = Vec::new();
    for _ in 0..200 {
        let c = Vec2::new(next() * 200.0 - 100.0, next() * 200.0 - 100.0);
        barriers.push(rail(
            c,
            next() * 6.3,
            0.2 + next() * 12.0,
            0.2 + next() * 2.0,
        ));
    }
    let g = barrier_graph(barriers);
    let mut stopped = 0;
    for _ in 0..4000 {
        let a = Vec2::new(next() * 220.0 - 110.0, next() * 220.0 - 110.0);
        let b = a + Vec2::new(next() - 0.5, next() - 0.5) * 60.0;
        let every = g
            .barriers
            .iter()
            .filter(|br| br.blocks(SOLDIER))
            .filter_map(|br| br.entry_t(a, b))
            .fold(1.0f32, f32::min);
        let t = g.barrier_stop(a, b, SOLDIER);
        assert_eq!(t.to_bits(), every.to_bits(), "{a} -> {b}");
        stopped += usize::from(t < 1.0);
    }
    assert!(stopped > 100, "legs stopped: {stopped}");
}

/// Bits of a corner search's answer.
fn corner_bits(p: &Option<Vec<Vec2>>) -> Option<Vec<[u32; 2]>> {
    p.as_ref()
        .map(|p| p.iter().map(|c| [c.x.to_bits(), c.y.to_bits()]).collect())
}

/// The cull's band allows for a barrier's reach: a long rail whose CENTRE lies
/// far outside the band still closes the cheaper way round the wall.
#[test]
fn a_rail_centred_outside_the_band_still_turns_the_search() {
    let front = wall(0.5, -5.0, 3.0, 0.25);
    // Along X, x -121 to -1: under the wall's cheaper corner, its centre 61 m
    // off a leg whose band is 25 m.
    let far = wall(-61.0, -7.0, 60.0, 0.25);
    let (a, b) = (Vec2::ZERO, Vec2::new(0.0, -10.0));
    let open = barrier_graph(vec![front])
        .detour(a, b, 0.4, SOLDIER)
        .unwrap();
    assert!(open[0].x < 0.0, "the left is cheaper: {open:?}");
    let g = barrier_graph(vec![front, far]);
    let culled = g.detour_culled(a, b, 0.4, SOLDIER, true);
    assert_eq!(
        corner_bits(&culled),
        corner_bits(&g.detour_culled(a, b, 0.4, SOLDIER, false))
    );
    let culled = culled.unwrap();
    assert!(culled[0].x > 0.0, "the rail closes the left: {culled:?}");
    assert!(path_is_clear(&g, a, &culled, b, 0.4));
}

/// Two barriers a segment meets at the same `t`: the lowest index is the
/// answer, though the cull walks them by the X of their centres.
#[test]
fn of_equal_crossings_the_lowest_index_is_nearest() {
    let g = barrier_graph(vec![
        wall(2.0, -5.0, 3.0, 0.25),
        wall(-2.0, -5.0, 3.0, 0.25),
    ]);
    let (a, b) = (Vec2::ZERO, Vec2::new(0.0, -10.0));
    let mut cand: Vec<DetourCand> = (0..2u16)
        .map(|index| DetourCand {
            centre: g.barriers[index as usize].centre,
            reach: 10.0,
            index,
        })
        .collect();
    cand.sort_unstable_by(|p, q| p.centre.x.total_cmp(&q.centre.x));
    assert_eq!(cand[0].index, 1, "the walk meets the higher index first");
    let mut s = DetourSearch {
        g: &g,
        cand,
        max_reach: 10.0,
        radius: 0.4,
        goal: b,
        perp: Vec2::X,
        band: 25.0,
        nodes: Vec::new(),
        open: Vec::new(),
        visited: Vec::new(),
        tests: 0,
    };
    assert_eq!(s.nearest_crossed(a, b), Some(0));
}

/// A leg that is not finite is not culled: every barrier answers it, near or
/// far, as before there was a cull. `Vec2::min` would otherwise drop a NaN end.
#[test]
fn a_leg_that_is_not_finite_is_answered_by_every_barrier() {
    let g = barrier_graph(
        (0..50)
            .map(|k| rail(Vec2::new(k as f32 * 4.0 - 100.0, 0.0), 0.3, 1.0, 1.0))
            .collect(),
    );
    for (a, b) in [
        (Vec2::NAN, Vec2::ZERO),
        // ...the goal outside any band a barrier could reach.
        (Vec2::NAN, Vec2::new(500.0, 0.0)),
        (Vec2::ZERO, Vec2::NAN),
        (Vec2::new(f32::NAN, 0.0), Vec2::new(5.0, 90.0)),
        (Vec2::new(0.0, -5.0), Vec2::new(f32::NAN, 5.0)),
        (Vec2::splat(f32::INFINITY), Vec2::ZERO),
        (Vec2::ZERO, Vec2::new(0.0, f32::NEG_INFINITY)),
    ] {
        let every = g
            .barriers
            .iter()
            .filter(|br| br.blocks(SOLDIER))
            .filter_map(|br| br.entry_t(a, b))
            .fold(1.0f32, f32::min);
        assert_eq!(
            g.barrier_stop(a, b, SOLDIER).to_bits(),
            every.to_bits(),
            "{a} -> {b}"
        );
        assert_eq!(
            corner_bits(&g.detour_culled(a, b, 0.4, SOLDIER, true)),
            corner_bits(&g.detour_culled(a, b, 0.4, SOLDIER, false)),
            "{a} -> {b}"
        );
    }
}
