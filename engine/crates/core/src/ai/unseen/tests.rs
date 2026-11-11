use super::*;
use crate::ai::tests::l_sim;
use crate::ai::why::Why;
use crate::collision::{Body, CollisionWorld, Shape};
use crate::sim::{DeathCause, Event, PadState};
use crate::unit::UnitSpec;
use glam::Mat4;

/// A pen round the origin (the bot's spawn on the L map, hub 0): four walls
/// a metre high, 2 m out - too high to step over, low enough to see over.
fn penned() -> (Sim, UnitHandle) {
    let (mut sim, h) = l_sim();
    let wall = |x: f32, z: f32, hx: f32, hz: f32| {
        Body::new(
            Mat4::from_translation(Vec3::new(x, 0.5, z)),
            Shape::Box {
                half: Vec3::new(hx, 0.5, hz),
            },
        )
    };
    sim.collision = CollisionWorld::new(
        vec![
            wall(2.5, 0.0, 0.5, 3.0),
            wall(-2.5, 0.0, 0.5, 3.0),
            wall(0.0, 2.5, 3.0, 0.5),
            wall(0.0, -2.5, 3.0, 0.5),
        ],
        Vec::new(),
    );
    (sim, h)
}

/// A human of the bot's own team at `at`, looking along yaw `yaw` (0: -Z).
fn human(sim: &mut Sim, at: Vec3, yaw: f32) -> UnitHandle {
    let p = sim.spawn(&UnitSpec::player(0, at)).expect("human");
    let u = sim.unit_mut(p).expect("alive");
    u.soldier.yaw = yaw;
    u.soldier.pitch = 0.0;
    p
}

/// Step until the bot is moved or recycled (at most past the stall watch's
/// bound): the move's spot, or `None` for a recycle.
fn outcome(sim: &mut Sim, h: UnitHandle) -> Option<Vec3> {
    let bound = u64::from(crate::ai::RECYCLE_AFTER) + 300;
    for _ in 0..bound {
        sim.step(&[PadState::default()]);
        for e in &sim.events {
            match *e {
                Event::Relocated { unit, to, .. } if unit == h => return Some(to),
                Event::EnvDeath {
                    unit,
                    cause: DeathCause::Stalled,
                } if unit == h => return None,
                _ => {}
            }
        }
        if let Some(u) = sim.unit_mut(h) {
            // (No time limit on its own life: only the watch ends it.)
            u.health.cur = u.health.cur.max(1.0);
        }
    }
    panic!("neither moved nor recycled in {bound} ticks");
}

/// No human: the penned bot is moved to hub 1, the far end of the link it
/// failed on, alive (no ticket spent); the census
/// counts the move and its log says why; outside the pen it walks on to its
/// post.
#[test]
fn a_stalled_bot_no_human_sees_is_moved_past_its_failing_link() {
    let (mut sim, h) = penned();
    let to = outcome(&mut sim, h).expect("moved, not recycled");
    assert!(
        nav::xz_dist(to, Vec3::new(0.0, 0.0, -40.0)) < 0.01,
        "hub 1: {to}"
    );
    let u = sim.unit(h).expect("the same life");
    assert!(nav::xz_dist(u.soldier.pos, to) < 0.5, "{}", u.soldier.pos);
    assert!(
        !sim.events
            .iter()
            .any(|e| matches!(e, Event::EnvDeath { .. })),
        "no death, so no ticket"
    );
    assert_eq!(u.ai.route().map(|r| r.next_hub), None, "a new walk");
    assert!(u.ai.stall.is_none());
    let row = u.ai.why.rows().filter(|r| r.failed).last().expect("a row");
    assert_eq!(row.why, Why::Relocated);
    let k = Why::Relocated.fail_index().expect("a fail code");
    assert_eq!(sim.nav_stats.fails[k], 1);
    let r = Why::Recycled.fail_index().expect("a fail code");
    assert_eq!(sim.nav_stats.fails[r], 0);
    // It walks on from there to the post (40, 0, -40).
    for _ in 0..900 {
        sim.step(&[PadState::default()]);
    }
    let u = sim.unit(h).expect("alive");
    assert!(
        nav::xz_dist(u.soldier.pos, Vec3::new(40.0, 0.0, -40.0)) < 8.0,
        "at its post: {}",
        u.soldier.pos
    );
}

/// A human 10 m away with a clear line over the pen's wall sees the bot
/// whichever way he looks: the recycle stays (with its ticket).
#[test]
fn a_stalled_bot_a_human_near_by_sees_is_recycled() {
    let (mut sim, h) = penned();
    // Looking away (+Z), the bot behind him.
    human(&mut sim, Vec3::new(0.0, 0.0, 10.0), std::f32::consts::PI);
    assert_eq!(outcome(&mut sim, h), None);
    assert!(sim.unit(h).is_none(), "the life is over");
    let k = Why::Recycled.fail_index().expect("a fail code");
    assert_eq!(sim.nav_stats.fails[k], 1);
}

/// A human far off (30 m) looking away from the bot and the spot sees
/// neither; one looking at the pen does.
#[test]
fn far_off_only_what_he_looks_at_is_seen() {
    let (mut sim, h) = penned();
    // At (30, 0, 0) looking +X: yaw -90 deg.
    human(
        &mut sim,
        Vec3::new(30.0, 0.0, 0.0),
        -std::f32::consts::FRAC_PI_2,
    );
    assert!(outcome(&mut sim, h).is_some(), "behind him, beyond 15 m");
    let (mut sim, h) = penned();
    // The same place looking -X, at the pen.
    human(
        &mut sim,
        Vec3::new(30.0, 0.0, 0.0),
        std::f32::consts::FRAC_PI_2,
    );
    assert_eq!(outcome(&mut sim, h), None, "in his view");
}

/// A human near hub 1 (10 m past it, looking on) sees that spot but not the
/// bot: it is moved on to the next hub of its route, hub 2.
#[test]
fn a_spot_a_human_sees_is_passed_for_the_next_hub() {
    let (mut sim, h) = penned();
    human(&mut sim, Vec3::new(0.0, 0.0, -50.0), 0.0);
    let to = outcome(&mut sim, h).expect("moved");
    assert!(
        nav::xz_dist(to, Vec3::new(40.0, 0.0, -40.0)) < 3.5,
        "hub 2: {to}"
    );
}

/// The sight test: near by any way he looks, far only in his view, never
/// through a wall.
#[test]
fn the_sight_test_is_line_and_view() {
    let (mut sim, _) = l_sim();
    sim.units.clear();
    human(&mut sim, Vec3::ZERO, 0.0);
    // Ahead (-Z) at 40 m, behind at 10 m: seen; behind at 20 m: not.
    assert!(seen_by_human(&sim, Vec3::new(0.0, 0.0, -40.0)));
    assert!(seen_by_human(&sim, Vec3::new(0.0, 0.0, 10.0)));
    assert!(!seen_by_human(&sim, Vec3::new(0.0, 0.0, 20.0)));
    // A wall 5 m high between him and the point ahead.
    sim.collision = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(0.0, 2.5, -20.0)),
            Shape::Box {
                half: Vec3::new(10.0, 2.5, 0.5),
            },
        )],
        Vec::new(),
    );
    assert!(!seen_by_human(&sim, Vec3::new(0.0, 0.0, -40.0)));
    // No human, nobody sees.
    let (sim, _) = l_sim();
    assert!(!seen_by_human(&sim, Vec3::ZERO));
}
