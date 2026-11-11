//! Card ARCM: the back-and-forth detector, the bot's closed arcs, the
//! team's count and the route draw round them.

use super::*;
use crate::ai::tests::l_sim;
use crate::ai::{plan_leg, NavTick, Route, RouteSource};
use crate::nav::{self, Arc, Hub, NavGraph, RouteTable};
use glam::Vec3;

/// A walk at a waypoint 10 m off along +x whose distance runs `ds` (one a
/// tick): the ticks the detector fired on.
fn fires(ds: &[f32]) -> Vec<u64> {
    let mut o = Osc::default();
    let target = Vec3::new(10.0, 0.0, 0.0);
    let mut out = Vec::new();
    for (t, &d) in ds.iter().enumerate() {
        let pos = Vec3::new(10.0 - d, 0.0, 0.0);
        if o.step(target, pos, t as u64 + 1) {
            out.push(t as u64 + 1);
        }
    }
    out
}

/// `n` ticks from distance `a` to `b`, straight.
fn run(a: f32, b: f32, n: usize) -> Vec<f32> {
    (1..=n).map(|i| a + (b - a) * i as f32 / n as f32).collect()
}

#[test]
fn three_turns_inside_five_seconds_are_a_back_and_forth() {
    // 5 -> 3.5 -> 5 -> 3.5 -> 5: closing, opening, closing, opening -
    // three turns of 1.5 m in 2 s.
    let mut ds = vec![5.0];
    for _ in 0..2 {
        ds.extend(run(5.0, 3.5, 15));
        ds.extend(run(3.5, 5.0, 15));
    }
    assert_eq!(fires(&ds).len(), 1, "{:?}", fires(&ds));
    // A sway under the amplitude is no turn.
    let mut ds = vec![5.0];
    for _ in 0..6 {
        ds.extend(run(5.0, 4.2, 10));
        ds.extend(run(4.2, 5.0, 10));
    }
    assert!(fires(&ds).is_empty());
}

#[test]
fn turns_spread_over_more_than_five_seconds_or_with_new_ground_are_not() {
    // The same three turns, 3 s apart each.
    let mut ds = vec![5.0];
    for _ in 0..2 {
        ds.extend(run(5.0, 3.5, 90));
        ds.extend(run(3.5, 5.0, 90));
    }
    assert!(fires(&ds).is_empty(), "{:?}", fires(&ds));
    // Fast turns, but each approach gets over a metre nearer than ever:
    // the walk makes ground (a bot weaving on its way in).
    let mut ds = vec![15.0];
    let mut d = 15.0;
    for _ in 0..4 {
        ds.extend(run(d, d - 3.0, 10));
        ds.extend(run(d - 3.0, d - 1.5, 10));
        d -= 1.5;
    }
    assert!(fires(&ds).is_empty(), "{:?}", fires(&ds));
}

#[test]
fn a_closed_arc_opens_after_a_minute_and_the_oldest_slot_makes_room() {
    let mut a = ArcmState::default();
    assert!(!a.any_closed(0));
    a.close(7, 100);
    assert!(a.is_closed(7, 100) && a.is_closed(7, 100 + ARC_CLOSED - 1));
    assert!(!a.is_closed(7, 100 + ARC_CLOSED) && !a.any_closed(100 + ARC_CLOSED));
    // Closing it again renews its own slot.
    a.close(7, 200);
    assert_eq!(a.closed.iter().filter(|c| c.0 == 7).count(), 1);
    for (i, arc) in [8, 9, 10].into_iter().enumerate() {
        a.close(arc, 300 + i as u64);
    }
    // Four arcs in three slots: the one closed first (7, at 200) went.
    assert!(!a.is_closed(7, 310));
    assert!([8, 9, 10].iter().all(|&x| a.is_closed(x, 310)));
}

#[test]
fn the_team_shuns_an_arc_closed_by_three_bots_in_turn() {
    let (mut sim, _) = l_sim();
    assert!(!sim.note_arc_closed(0, 1, 5));
    assert!(!sim.note_arc_closed(0, 1, 5), "the same bot again");
    assert!(!sim.arc_shunned(0, 1));
    assert!(!sim.note_arc_closed(0, 1, 6));
    assert!(sim.note_arc_closed(0, 1, 7), "the third bot");
    assert!(sim.arc_shunned(0, 1) && sim.any_shunned(0));
    assert!(
        !sim.arc_shunned(1, 1) && !sim.any_shunned(1),
        "the other team's"
    );
    assert!(!sim.arc_shunned(0, 0));
}

/// A square of four hubs, 0 (0,0) 1 (0,-40) 2 (40,-40) 3 (40,0): from 0 the
/// authored table goes to 2 by arc 0 (to 1), and only by it.
fn square() -> NavGraph {
    let hub = |x: f32, z: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(x, 0.0, z),
        radius: 2.0,
        arcs,
    };
    let hubs = vec![
        hub(0.0, 0.0, vec![0, 2]),
        hub(0.0, -40.0, vec![0, 1]),
        hub(40.0, -40.0, vec![1, 3]),
        hub(40.0, 0.0, vec![2, 3]),
    ];
    let arc = |a: u16, b: u16| Arc {
        a,
        b,
        type_mask: 1,
        flags: 0,
    };
    let arcs = vec![arc(0, 1), arc(1, 2), arc(0, 3), arc(3, 2)];
    let n = 4usize;
    let mut rows = vec![Vec::new(); nav::TYPE_COUNT * n];
    for (from, slot) in [(0usize, 0u8), (1, 1), (3, 1)] {
        let mut r = vec![0u8; n];
        r[2] = (31 << 3) | slot;
        rows[nav::SOLDIER * n + from] = r;
    }
    NavGraph::new(
        hubs,
        arcs,
        RouteTable::from_rows(n, nav::TYPE_COUNT, rows),
        Vec::new(),
        Vec::new(),
    )
    .expect("graph")
}

/// The arc of the leg `plan_leg` draws from hub 0 to hub 2.
fn drawn(sim: &crate::sim::Sim, st: &AiState, h: crate::unit::UnitHandle) -> (u16, NavTick) {
    let mut nt = NavTick::default();
    let r = plan_leg(sim, st, h, 0, 2, None, &mut nt).expect("a leg");
    (r.via_arc.expect("an arc"), nt)
}

#[test]
fn a_closed_or_shunned_arc_is_drawn_round_while_another_way_exists() {
    let (mut sim, h) = l_sim();
    sim.nav = Some(square());
    let mut st = AiState::default();
    assert_eq!(drawn(&sim, &st, h).0, 0, "the table's way");
    // Closed to the bot: the other way round the square.
    st.arcm.close(0, sim.tick);
    let (arc, nt) = drawn(&sim, &st, h);
    assert_eq!((arc, nt.shun_round, nt.shun_kept), (2, 1, 0));
    // ...for a minute.
    sim.tick += ARC_CLOSED;
    assert_eq!(drawn(&sim, &st, h).0, 0);
    // Shunned by the bot's team (three bots closed it): the same.
    let team = sim.unit(h).expect("bot").team;
    for u in 10..13 {
        sim.note_arc_closed(team, 0, u);
    }
    assert_eq!(drawn(&sim, &AiState::default(), h).0, 2);
    // The other team still takes the table's way.
    sim.unit_mut(h).expect("bot").team = 1 - team;
    assert_eq!(drawn(&sim, &AiState::default(), h).0, 0);
}

#[test]
fn a_closed_arc_with_no_other_way_is_kept() {
    // The L: arc 0 is the only way from hub 0.
    let (sim, h) = l_sim();
    let mut st = AiState::default();
    st.arcm.close(0, sim.tick);
    let (arc, nt) = drawn(&sim, &st, h);
    assert_eq!((arc, nt.shun_round, nt.shun_kept), (0, 0, 1));
}

/// A bot drawn round a closed arc keeps to the way round at the next hubs:
/// on the square with arc 1 (1-2) closed, a bot re-planning at hub 1 goes
/// back to hub 0, and at hub 0 (a re-plan, no arrival arc) the table's way
/// is arc 0 to hub 1 again - the loop. It goes round by arc 2 instead.
#[test]
fn a_bot_drawn_round_keeps_the_way_round_to_the_same_goal() {
    let (mut sim, h) = l_sim();
    sim.nav = Some(square());
    let mut st = AiState::default();
    st.arcm.close(1, sim.tick);
    let mut nt = NavTick::default();
    let back = plan_leg(&sim, &st, h, 1, 2, None, &mut nt).expect("a leg");
    assert_eq!((back.via_arc, back.source), (Some(0), RouteSource::Shun));
    st.install_route_for_fixture(Some(back));
    let on = plan_leg(&sim, &st, h, 0, 2, None, &mut nt).expect("a leg");
    assert_eq!((on.via_arc, on.source), (Some(2), RouteSource::Shun));
    // Another goal: the table again.
    st.install_route_for_fixture(Some(Route {
        dest_hub: 1,
        ..back
    }));
    let other = plan_leg(&sim, &st, h, 0, 2, None, &mut nt).expect("a leg");
    assert_eq!(other.via_arc, Some(0));
}

/// Four 12 m walls round the origin, 5 m out: no way out.
fn closed_room() -> crate::collision::CollisionWorld {
    use crate::collision::{Body, CollisionWorld, Shape};
    let slab =
        |c: Vec3, half: Vec3| Body::new(glam::Mat4::from_translation(c), Shape::Box { half });
    CollisionWorld::new(
        vec![
            slab(Vec3::new(0.0, 0.0, -5.5), Vec3::new(6.0, 6.0, 0.5)),
            slab(Vec3::new(0.0, 0.0, 5.5), Vec3::new(6.0, 6.0, 0.5)),
            slab(Vec3::new(-5.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 6.0)),
            slab(Vec3::new(5.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 6.0)),
        ],
        Vec::new(),
    )
}

/// A progress window that fails on an arc leg closes nothing the first time;
/// a second failure with no progress between (`MoverState::recover`, which
/// the Recover ladder counts) closes the arc to the bot.
#[test]
fn an_arc_is_closed_to_the_bot_at_its_second_failure_running() {
    use crate::ai::STUCK_WINDOW;
    let (sim, h) = l_sim();
    let mut st = AiState::default();
    let mut nt = NavTick::default();
    crate::ai::route::plan_to(&sim, &mut st, h, 0, 2, &mut nt);
    let pos = sim.unit(h).expect("bot").soldier.pos;
    let window = |st: &mut AiState| {
        let mut nt = NavTick::default();
        for _ in 0..STUCK_WINDOW {
            crate::ai::route::progress(&sim, h, st, pos, true, false, &mut nt);
        }
        nt
    };
    let nt = window(&mut st);
    assert!(nt.fail().is_fail() && nt.closed_arc.is_none(), "{nt:?}");
    assert!(!st.arcm.any_closed(sim.tick));
    st.mv.recover = 1;
    let arc = st.route().and_then(|r| r.via_arc).expect("an arc leg");
    let nt = window(&mut st);
    assert_eq!(nt.closed_arc, Some(arc), "{nt:?}");
    assert!(st.arcm.is_closed(arc, sim.tick));
}

/// End to end: a bot penned in a room fails its walk on arc 0 window after
/// window; the arc is closed to it and the team counts it once.
#[test]
fn a_penned_bot_closes_its_arc_and_the_team_counts_it() {
    use crate::sim::PadState;
    let (mut sim, h) = l_sim();
    sim.collision = closed_room();
    while sim.nav_stats.arcs_closed == 0 && sim.tick < 1800 {
        sim.step(&[PadState::default()]);
    }
    let u = sim.unit(h).expect("alive");
    let rows: Vec<_> = u.ai.why.rows().copied().collect();
    assert!(u.ai.arcm.is_closed(0, sim.tick), "{rows:?}");
    assert!(rows.iter().filter(|r| r.failed).count() >= 2, "{rows:?}");
    assert_eq!(sim.arc_fails[usize::from(u.team)][0].n, 1);
}

/// The detector's verdict closes the progress window at once as a failed
/// one, `Why::Oscillation`, and closes the leg's arc - with no earlier
/// failure.
#[test]
fn a_back_and_forth_fails_the_window_at_once_and_closes_the_arc() {
    use crate::ai::why::Why;
    let (sim, h) = l_sim();
    let mut st = AiState::default();
    let mut nt = NavTick::default();
    crate::ai::route::plan_to(&sim, &mut st, h, 0, 2, &mut nt);
    let arc = st.route().and_then(|r| r.via_arc).expect("an arc leg");
    let pos = sim.unit(h).expect("bot").soldier.pos;
    let mut nt = NavTick::default();
    crate::ai::route::progress(&sim, h, &mut st, pos, true, false, &mut nt);
    assert_eq!(nt.fail(), Why::None, "one tick of a window");
    crate::ai::route::progress(&sim, h, &mut st, pos, true, true, &mut nt);
    assert_eq!(
        (nt.fail(), nt.fail_arc, nt.closed_arc),
        (Why::Oscillation, Some(arc), Some(arc))
    );
    assert!(st.arcm.is_closed(arc, sim.tick));
}
