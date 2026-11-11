//! Card TPOS: the position table's and the Tactician's tests (out of
//! line from the implementation in `tactic.rs`).

use super::*;
use crate::ai::FightState;
use crate::sim::PadState;
use crate::unit::{Controller, UnitHandle, UnitSpec};

/// The L graph's sim (`ai::tests::l_sim`) with a crate 1 m high across
/// the corridor 3 m south of hub 0 - LOW cover for hub 0's centre and
/// its east and west points toward the south - the table built, and an
/// enemy (idle, kept alive) at `enemy`. The bot stands at `bot`.
fn crate_scene(bot: Vec3, enemy: Vec3) -> (Sim, UnitHandle, UnitHandle) {
    use crate::collision::{Body, Shape};
    let (mut sim, h) = super::super::tests::l_sim();
    sim.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 0.5, -3.0)),
        Shape::Box {
            half: Vec3::new(1.5, 0.5, 0.3),
        },
    ));
    sim.build_positions();
    let e = sim.spawn(&UnitSpec::trooper(1, enemy)).expect("enemy");
    let u = sim.unit_mut(h).expect("bot");
    u.soldier.pos = bot;
    u.soldier.yaw = crate::ai::yaw_toward(enemy - bot);
    (sim, h, e)
}

fn run(sim: &mut Sim, e: UnitHandle, ticks: u32, mut each: impl FnMut(&Sim)) {
    for _ in 0..ticks {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        each(sim);
    }
}

/// Two hubs 12 m apart on flat ground, A (0, 0) with the crate 3 m
/// south of it and B (0, 12) with the post (owned by the enemy, so the
/// bot's objective); the bot at B, an enemy 25 m south of A.
fn two_hub_scene() -> (Sim, UnitHandle, UnitHandle) {
    use crate::collision::{Body, Shape};
    use crate::nav::{Arc, Hub, NavGraph, RouteTable};
    let (mut sim, h) = super::super::tests::l_sim();
    let hub = |z: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(0.0, 0.0, z),
        radius: 2.0,
        arcs,
    };
    let n = 2usize;
    let rows = vec![Vec::new(); crate::nav::TYPE_COUNT * n];
    sim.nav = Some(
        NavGraph::new(
            vec![hub(0.0, vec![0]), hub(12.0, vec![0])],
            vec![Arc {
                a: 0,
                b: 1,
                type_mask: 1,
                flags: 0,
            }],
            RouteTable::from_rows(n, crate::nav::TYPE_COUNT, rows),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph"),
    );
    let post = Vec3::new(0.0, 0.0, 12.0);
    sim.cps[0].def.pos = post;
    sim.cps[0].def.capture = crate::region::Region::sphere(post, 3.0);
    sim.cps[0].nav_hub = Some(1);
    sim.resolve_nav_floors();
    sim.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 0.5, -3.0)),
        Shape::Box {
            half: Vec3::new(1.5, 0.5, 0.3),
        },
    ));
    sim.build_positions();
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let e = sim.spawn(&UnitSpec::trooper(1, enemy)).expect("enemy");
    let u = sim.unit_mut(h).expect("bot");
    u.soldier.pos = post;
    u.soldier.yaw = crate::ai::yaw_toward(enemy - post);
    (sim, h, e)
}

/// A bot fighting a target across a crate takes a position with cover
/// from it - the crate's LOW cover at hub A, 12 m off, not one of the
/// open points of hub B it stands in - reserved for it, with a line to
/// the target from its firing point; it walks there and stands in its
/// cycle.
#[test]
fn a_bot_takes_the_covered_position_across_a_crate() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = two_hub_scene();
    assert!(sim.positions.positions.len() >= 18, "two hubs' points");
    assert!(
        sim.positions.positions.iter().any(|q| q.low != 0),
        "the crate is cover"
    );
    run(&mut sim, e, 240, |_| {});
    let u = sim.unit(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER, "at its position");
    let i = u.ai.tpos.held.expect("a position");
    let q = sim.positions.positions[i as usize];
    let d = Vec2::new(enemy.x - q.pos.x, enemy.z - q.pos.z);
    assert!(
        u.ai.tpos.covered && q.low & sector_pair(d) != 0,
        "LOW cover from the target"
    );
    assert!(u.ai.tpos.until > sim.tick, "reserved");
    assert!(
        nav::xz_dist(u.soldier.pos, q.pos) < 1.0,
        "it stands there: {} at {} for {}",
        u.soldier.pos,
        q.pos,
        sim.tick
    );
    let eye = firing_eye(q.pos, enemy, 0);
    let chest = sim.unit(e).expect("enemy").muzzle();
    assert!(line_open(&sim, eye, chest), "a line to the target");
    // The cycle: hidden (crouched) and firing (standing) in turn.
    let mut phases = std::collections::BTreeSet::new();
    run(&mut sim, e, 300, |s| {
        let u = s.unit(h).expect("bot");
        if u.ai.tpos.held == Some(i) {
            phases.insert(u.ai.tpos.firing);
        }
    });
    assert_eq!(phases.len(), 2, "it hides and fires in turn");
}

/// Two bots fighting from one hub never hold positions within
/// [`SPACING`] of each other - and both get one.
#[test]
fn two_bots_never_hold_positions_within_two_metres() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    let h2 = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(-0.3, 0.0, 0.5))
        })
        .expect("second bot");
    let (mut both, mut close) = (0u32, Vec::new());
    run(&mut sim, e, 300, |s| {
        let at = |h: UnitHandle| {
            let u = s.unit(h)?;
            u.ai.tpos
                .held
                .map(|i| s.positions.positions[i as usize].pos)
        };
        if let (Some(a), Some(b)) = (at(h), at(h2)) {
            both += 1;
            if nav::xz_dist(a, b) < SPACING {
                close.push(s.tick);
            }
        }
    });
    assert!(both > 100, "both held positions: {both} ticks");
    assert!(close.is_empty(), "within {SPACING} m at ticks {close:?}");
}

/// Teammates hold positions a grenade's blast apart ([`SPREAD`]) where
/// the place allows: hub 0 grown to 8 m (its ring points 5.6 m out), two
/// bots starting side by side.
#[test]
fn teammates_hold_positions_a_blast_apart_where_the_place_allows() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    if let Some(g) = sim.nav.as_mut() {
        g.hubs[0].radius = 8.0;
    }
    sim.build_positions();
    let h2 = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(-0.3, 0.0, 0.5))
        })
        .expect("second bot");
    let mut apart = Vec::new();
    run(&mut sim, e, 120, |s| {
        let at = |h: UnitHandle| {
            let u = s.unit(h)?;
            u.ai.tpos
                .held
                .map(|i| s.positions.positions[i as usize].pos)
        };
        if let (Some(a), Some(b)) = (at(h), at(h2)) {
            apart.push(nav::xz_dist(a, b));
        }
    });
    assert!(
        apart.len() > 60,
        "both held positions: {} ticks",
        apart.len()
    );
    let near = apart.iter().filter(|d| **d < SPREAD).count();
    assert_eq!(near, 0, "closer than {SPREAD} m on {near} ticks");
}

/// The endgame (a side down to its last reinforcements): no positions,
/// the bot's own fight - and a position held is left.
#[test]
fn nobody_takes_a_position_in_the_endgame() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::ZERO, enemy);
    run(&mut sim, e, 60, |_| {});
    assert!(
        sim.unit(h).and_then(|u| u.ai.tpos.held).is_some(),
        "a position first"
    );
    sim.battle.teams[1].reinforcements = crate::ai::HOLD_ENDGAME as f32;
    run(&mut sim, e, 60, |s| {
        if s.tick > 75 {
            let u = s.unit(h).expect("bot");
            assert!(
                u.ai.tpos.held.is_none() && !u.ai.fight.tactician(),
                "tick {}",
                s.tick
            );
        }
    });
}

/// A position where the bot is hit twice is left for another.
#[test]
fn a_position_hit_twice_is_left() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::ZERO, enemy);
    run(&mut sim, e, 60, |_| {});
    let first = sim
        .unit(h)
        .and_then(|u| u.ai.tpos.held)
        .expect("a position");
    assert_eq!(sim.unit(h).expect("bot").ai.fight.mode, FightState::COVER);
    // (Not the 5-12 s wait for another: only the hits.)
    sim.unit_mut(h).expect("bot").ai.tpos.next_at = u64::MAX;
    // Two hits, a burst apart (the second inside the cooldown would be
    // the same burst).
    for _ in 0..2 {
        let t = sim.tick;
        sim.unit_mut(h).expect("bot").last_hit = Some((e, t - 1));
        run(&mut sim, e, (REACT_GAP) as u32, |_| {});
    }
    run(&mut sim, e, 30, |_| {});
    let u = sim.unit(h).expect("bot");
    assert_ne!(u.ai.tpos.held, Some(first), "it left the position");
    assert_eq!(
        u.ai.tpos.prev,
        Some(first),
        "...which stays reserved a while"
    );
}
const REACT_GAP: u64 = crate::ai::REACT_COOLDOWN + 1;

/// No position passes (every one is beyond the weapon's band from a
/// target off to the east): the bot fights by its own style - the PS2's
/// band move - and asks again after [`RETRY`].
#[test]
fn no_winner_is_the_bots_own_fight() {
    let enemy = Vec3::new(47.0, 0.0, 0.0);
    let (mut sim, h, e) = crate_scene(Vec3::ZERO, enemy);
    let mut asked = false;
    run(&mut sim, e, 90, |s| {
        let u = s.unit(h).expect("bot");
        asked |= u.ai.tpos.retry_at > 0;
        assert!(u.ai.tpos.held.is_none(), "no position");
    });
    let u = sim.unit(h).expect("bot");
    assert!(asked, "the Tactician was asked");
    assert_eq!(u.ai.behaviour, crate::ai::Behaviour::Fight);
    assert!(
        matches!(
            u.ai.fight.mode,
            FightState::STAND | FightState::WALK | FightState::PAUSE
        ),
        "its own style: mode {}",
        u.ai.fight.mode
    );
}

/// Another position, when the 5-12 s wait sends the bot on, is at least
/// [`MOVE_MIN`] from the one it leaves - or it stays (the maintainer's Kamino
/// scene: hops of 0.6-1.5 m between neighbouring points).
#[test]
fn a_new_position_is_a_move_not_a_twitch() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::ZERO, enemy);
    let (mut held, mut moves, mut asked) = (None::<u32>, Vec::new(), 0u32);
    for _ in 0..12 {
        // The wait is over at once, every time.
        if let Some(u) = sim.unit_mut(h) {
            if u.ai.fight.mode == FightState::COVER {
                u.ai.tpos.next_at = 0;
                asked += 1;
            }
        }
        run(&mut sim, e, 30, |s| {
            let u = s.unit(h).expect("bot");
            if let (Some(a), Some(b)) = (held, u.ai.tpos.held) {
                if a != b {
                    let (p, q) = (
                        s.positions.positions[a as usize],
                        s.positions.positions[b as usize],
                    );
                    moves.push(nav::xz_dist(p.pos, q.pos));
                }
            }
            held = u.ai.tpos.held.or(held);
        });
    }
    assert!(asked >= 6, "the wait ran out at a position {asked} times");
    assert!(moves.iter().all(|d| *d >= MOVE_MIN), "moves of {moves:?} m");
}

/// A walk to its position that ends short of it - here its deadline, the
/// bot 1.3 m off - is arrived where the bot stands when within
/// [`NEAR_ENOUGH`]: the position kept, its cycle begun there.
#[test]
fn a_walk_cut_short_near_its_position_is_there() {
    let (mut sim, h, e) = two_hub_scene();
    let mut i = None;
    run(&mut sim, e, 120, |s| {
        let u = s.unit(h).expect("bot");
        if i.is_none() && u.ai.fight.mode == FightState::TAKE {
            i = u.ai.tpos.held;
        }
    });
    let i = i.expect("it walked to a position");
    let q = sim.positions.positions[i as usize].pos;
    let t = sim.tick;
    let u = sim.unit_mut(h).expect("bot");
    assert_eq!(u.ai.tpos.held, Some(i), "still on its way");
    u.ai.fight.mode = FightState::TAKE;
    u.soldier.pos = q + Vec3::new(1.3, 0.0, 0.0);
    u.soldier.vel = Vec3::ZERO;
    u.ai.tpos.phase_until = t - 1;
    u.ai.next_think = t;
    let mut seen = None;
    run(&mut sim, e, 30, |s| {
        let u = s.unit(h).expect("bot");
        if seen.is_none() && u.ai.fight.mode != FightState::TAKE {
            seen = Some((u.ai.fight.mode, u.ai.tpos.held, u.ai.tpos.stand));
        }
    });
    let (mode, held, stand) = seen.expect("the walk ended");
    assert_eq!((mode, held), (FightState::COVER, Some(i)), "there");
    let d = nav::xz_dist(stand, q);
    assert!(
        d > ARRIVE_NEAR && d <= NEAR_ENOUGH,
        "where it stood: {d} m off"
    );
}

/// A walk past its deadline far from its position fails: the position
/// is left, and this bot never asks for it again.
#[test]
fn a_walk_past_its_deadline_far_off_is_left_for_good() {
    let (mut sim, h, e) = two_hub_scene();
    let mut i = None;
    run(&mut sim, e, 120, |s| {
        let u = s.unit(h).expect("bot");
        if i.is_none() && u.ai.fight.mode == FightState::TAKE {
            i = u.ai.tpos.held;
        }
    });
    let i = i.expect("it walked to a position");
    let q = sim.positions.positions[i as usize].pos;
    let t = sim.tick;
    let u = sim.unit_mut(h).expect("bot");
    u.ai.fight.mode = FightState::TAKE;
    u.soldier.pos = q + Vec3::new(0.0, 0.0, 5.0);
    u.ai.tpos.phase_until = t - 1;
    u.ai.next_think = t;
    let mut again = Vec::new();
    let mut left = false;
    run(&mut sim, e, 300, |s| {
        let u = s.unit(h).expect("bot");
        left |= u.ai.tpos.held != Some(i);
        if left && u.ai.tpos.held == Some(i) {
            again.push(s.tick);
        }
    });
    assert!(left, "the position was left");
    assert!(again.is_empty(), "taken again at ticks {again:?}");
    assert!(sim.unit(h).expect("bot").ai.tpos.bad.contains(&Some(i)));
}

/// A position whose straight way from the bot crosses a soldier barrier
/// is not chosen: the crate's covered points at hub A lie behind a
/// barrier across the corridor, so the bot at B holds none of them
/// (one behind a barrier was walked at and failed again and again,
/// The maintainer's Kamino scene).
#[test]
fn a_position_behind_a_barrier_is_not_chosen() {
    let (mut sim, h, e) = two_hub_scene();
    if let Some(g) = sim.nav.as_mut() {
        g.barriers.push(nav::Barrier {
            centre: Vec2::new(0.0, 6.0),
            axes: [Vec2::X, Vec2::Y],
            half: Vec2::new(6.0, 0.3),
            mask: 1 << nav::SOLDIER,
            enabled: true,
        });
    }
    let mut behind = Vec::new();
    run(&mut sim, e, 240, |s| {
        let u = s.unit(h).expect("bot");
        if let Some(i) =
            u.ai.tpos
                .held
                .filter(|i| s.positions.positions[*i as usize].pos.z < 6.0)
        {
            behind.push((s.tick, i));
        }
    });
    // (Without the barrier it takes the crate's cover:
    // `a_bot_takes_the_covered_position_across_a_crate`.)
    assert!(behind.is_empty(), "held behind the barrier: {behind:?}");
}

/// A Cover hint hides by its arrow alone, the PS2's rule
/// (re-soldier-ai 4.6): toward a target within 60 degrees of it, not
/// beyond; any other point only by what is shut round it.
#[test]
fn a_cover_hint_hides_by_its_arrow() {
    let mut q = Position {
        pos: Vec3::ZERO,
        source: Source::Cover,
        facing: Vec2::X,
        stance: 0,
        stance2: 0,
        hub: 0,
        low: 0,
        corner: 0,
        shut: 0,
        tall: 0,
    };
    assert!(q.hides_toward(Vec2::new(10.0, 5.0)), "27 degrees off");
    assert!(!q.hides_toward(Vec2::new(10.0, 20.0)), "63 degrees off");
    assert!(!q.hides_toward(Vec2::new(-10.0, 0.0)), "behind it");
    q.source = Source::Edge;
    assert!(!q.hides_toward(Vec2::X), "an edge point has no arrow");
    q.shut = sector_bit(Vec2::X);
    assert!(q.hides_toward(Vec2::X), "...but its shut sector");
}

/// The positions of hub A (the crate's) with cover toward the south.
fn covered_at_a(sim: &Sim) -> Vec<u32> {
    let d = Vec2::new(0.0, -1.0);
    (0..sim.positions.positions.len() as u32)
        .filter(|&i| {
            let q = &sim.positions.positions[i as usize];
            q.pos.z < 2.0 && q.shut & sector_pair(d) != 0
        })
        .collect()
}

/// A covered position is a candidate however many open ones lie nearer:
/// with 40 more open points round hub B, the bot there still takes the
/// crate's cover at A, 12 m off (the 32 nearest of every kind held 5.0
/// of Mygeeto's 42.6 covered candidates a query).
#[test]
fn a_covered_position_is_a_candidate_past_the_nearest_open_ones() {
    let (mut sim, h, e) = two_hub_scene();
    let mut all = sim.positions.positions.clone();
    let stats = sim.positions.stats;
    for k in 0..40 {
        let a = k as f32 / 40.0 * std::f32::consts::TAU;
        let r = 2.0 + (k % 5) as f32;
        let mut q = all[all.len() - 1];
        q.pos = Vec3::new(r * a.cos(), 0.0, 12.0 + r * a.sin());
        q.source = Source::Hub;
        (q.low, q.corner, q.shut, q.tall) = (0, 0, 0, 0);
        all.push(q);
    }
    sim.positions = PositionTable::index(all, stats);
    let mut covered = false;
    run(&mut sim, e, 240, |s| {
        let u = s.unit(h).expect("bot");
        covered |= u.ai.tpos.held.is_some() && u.ai.tpos.covered;
    });
    assert!(covered, "it took a covered position");
}

/// A covered position farther than [`COVER_REACH`] is not walked to: a
/// bot 17-18 m from the crate's points takes none of them.
#[test]
fn a_covered_position_beyond_reach_is_not_taken() {
    let (mut sim, h, e) = two_hub_scene();
    sim.unit_mut(e).expect("enemy").soldier.pos = Vec3::new(0.0, 0.0, -15.0);
    let cov = covered_at_a(&sim);
    assert!(!cov.is_empty());
    sim.unit_mut(h).expect("bot").soldier.pos = Vec3::new(0.0, 0.0, 17.0);
    let (mut far_picks, mut bad, mut was) = (0, Vec::new(), None);
    run(&mut sim, e, 240, |s| {
        let u = s.unit(h).expect("bot");
        let fresh = u.ai.tpos.held.is_some() && u.ai.tpos.held != was;
        was = u.ai.tpos.held;
        if !fresh {
            return;
        }
        let near = cov
            .iter()
            .map(|&i| nav::xz_dist(s.positions.positions[i as usize].pos, u.soldier.pos))
            .fold(f32::MAX, f32::min);
        if near > COVER_REACH {
            far_picks += 1;
            if u.ai.tpos.covered {
                bad.push(s.tick);
            }
        }
    });
    assert!(far_picks > 0, "it chose from beyond the reach");
    assert!(bad.is_empty(), "covered picks out of reach at {bad:?}");
}

/// The target gone round the cover leaves the position at its re-score,
/// every [`RESCORE`] (the PS2's 5 s), not at the next think.
#[test]
fn a_position_is_rescored_every_five_seconds() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    run(&mut sim, e, 400, |_| {});
    let u = sim.unit_mut(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER, "at its position");
    assert!(u.ai.tpos.covered);
    let i = u.ai.tpos.held.expect("a position");
    u.ai.tpos.next_at = u64::MAX / 2;
    let r0 = u.ai.tpos.rescore_at;
    let t0 = sim.tick;
    assert!(r0 > t0 + 30, "the re-score is ahead: {r0} at {t0}");
    sim.unit_mut(e).expect("enemy").soldier.pos = Vec3::new(0.0, 0.0, 25.0);
    let mut left = None;
    // The re-score needs the target SEEN, behind the bot now: its memory
    // runs out ~1.3 s past the re-score and it turns to find it (2 rad), so
    // it leaves ~2.4 s past the re-score. (It left at +1.4 s while the old
    // stance cycle laid it prone by mistake before the test's turn, 2026-10-28.)
    run(&mut sim, e, (r0 - t0) as u32 + 90, |s| {
        let u = s.unit(h).expect("bot");
        if left.is_none() && u.ai.tpos.held != Some(i) {
            left = Some(s.tick);
        }
    });
    let left = left.expect("left once round");
    assert!(left >= r0, "left at {left}, before its re-score at {r0}");
}

/// A bot at an open position takes a covered one when it comes free,
/// within the 2 s of its next ask, not at the 5-12 s timer: the crate's
/// points at A are reserved by an idle teammate, the bot holds an open
/// point at B; the reservation goes.
#[test]
fn an_open_position_gives_way_to_cover_when_it_comes_free() {
    let (mut sim, h, e) = two_hub_scene();
    let cov = covered_at_a(&sim);
    let centre = *cov
        .iter()
        .min_by(|a, b| {
            let p = |i: u32| sim.positions.positions[i as usize].pos.length();
            p(**a).total_cmp(&p(**b))
        })
        .expect("a covered point");
    let m = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(30.0, 0.0, 30.0)))
        .expect("mate");
    let hold = |s: &mut Sim, on: bool| {
        let t = s.tick;
        let mu = s.unit_mut(m).expect("mate");
        mu.ai.tpos.held = on.then_some(centre);
        mu.ai.tpos.until = if on { t + 1000 } else { 0 };
    };
    for _ in 0..240 {
        hold(&mut sim, true);
        run(&mut sim, e, 1, |_| {});
    }
    let u = sim.unit_mut(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER, "at a position");
    assert!(!u.ai.tpos.covered, "an open one");
    u.ai.tpos.next_at = u64::MAX / 2;
    hold(&mut sim, false);
    let mut covered = false;
    run(&mut sim, e, 90, |s| {
        let u = s.unit(h).expect("bot");
        covered |= u.ai.tpos.held.is_some() && u.ai.tpos.covered;
    });
    assert!(covered, "it moved to the cover within 3 s");
}

/// A walk to a near position that comes no [`GAIN`] nearer in
/// [`GAIN_TICKS`] is cut short there and then, long before its deadline
/// (bots slid along a pod's flank for the whole deadline).
#[test]
fn a_walk_that_gains_nothing_for_a_second_is_cut() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.0, 0.0, 6.0), enemy);
    let mut i = None;
    run(&mut sim, e, 120, |s| {
        let u = s.unit(h).expect("bot");
        if i.is_none() && u.ai.fight.mode == FightState::TAKE {
            i = u.ai.tpos.held;
        }
    });
    let i = i.expect("it walked to a position");
    let q = sim.positions.positions[i as usize].pos;
    // Pinned 4 m off it, its deadline far ahead.
    let at = q + Vec3::new(0.0, 0.0, 4.0);
    let t0 = sim.tick;
    let u = sim.unit_mut(h).expect("bot");
    u.ai.fight.mode = FightState::TAKE;
    u.ai.tpos.held = Some(i);
    u.ai.tpos.phase_until = t0 + 3000;
    u.ai.tpos.best = 4.0;
    u.ai.tpos.best_at = t0;
    let stalled0 = sim.nav_stats.tpos_ev[EV_WALK_STALLED.trailing_zeros() as usize];
    let mut ended = None;
    for _ in 0..90 {
        if let Some(u) = sim.unit_mut(h) {
            u.soldier.pos = at;
            u.soldier.vel = Vec3::ZERO;
        }
        run(&mut sim, e, 1, |s| {
            let u = s.unit(h).expect("bot");
            if ended.is_none() && (u.ai.fight.mode != FightState::TAKE || u.ai.tpos.held != Some(i))
            {
                ended = Some(s.tick);
            }
        });
    }
    let ended = ended.expect("the walk ended");
    assert!(ended <= t0 + GAIN_TICKS + 20, "ended at {ended}, from {t0}");
    assert_eq!(
        sim.nav_stats.tpos_ev[EV_WALK_STALLED.trailing_zeros() as usize] - stalled0,
        1,
        "by the no-progress cut"
    );
}

/// The drop test: the way across a 1 m deep trench between two platforms
/// drops; along one platform it does not.
#[test]
fn a_way_over_a_trench_drops() {
    use crate::collision::{Body, Shape};
    let (mut sim, _) = super::super::tests::l_sim();
    for x in [-4.0, 4.0] {
        sim.collision.push_body(Body::new(
            glam::Mat4::from_translation(Vec3::new(x, 0.5, 0.0)),
            Shape::Box {
                half: Vec3::new(3.0, 0.5, 3.0),
            },
        ));
    }
    let (a, a2, b) = (
        Vec3::new(-5.0, 1.0, 0.0),
        Vec3::new(-2.0, 1.0, 2.0),
        Vec3::new(5.0, 1.0, 0.0),
    );
    assert!(no_drop(&sim, a, a2), "along a platform");
    assert!(!no_drop(&sim, a, b), "over the trench");
}

/// A Cover hint inside a soldier barrier's 0.3 m margin is moved out of
/// it (by at most [`HINT_PUSH`]); one deep inside is not (Polis Massa
/// dropped 172 of its 210 hints at build).
#[test]
fn a_hint_in_a_barriers_margin_is_pushed_out() {
    let (mut sim, _) = super::super::tests::l_sim();
    if let Some(g) = sim.nav.as_mut() {
        g.barriers.push(nav::Barrier {
            centre: Vec2::ZERO,
            axes: [Vec2::X, Vec2::Y],
            half: Vec2::new(2.0, 1.0),
            mask: 1 << nav::SOLDIER,
            enabled: true,
        });
    }
    let g = sim.nav.as_ref().expect("graph");
    let b = Builder {
        sim: &sim,
        g,
        rays: 0,
    };
    let q = b.out_of_barriers(Vec2::new(0.0, 1.15)).expect("pushed out");
    assert!(q.y >= 1.0 + IN_BARRIER && (q.y - 1.15) <= HINT_PUSH, "{q}");
    assert!((q.x).abs() < 1e-4, "along the nearer face: {q}");
    assert!(
        b.out_of_barriers(Vec2::new(0.0, 0.2)).is_none(),
        "deep inside"
    );
}

/// Issue #70: a hold spot's dry test is the port's own rule and reads the layer
/// height, so ground at its 0.2 m limit is not refused for the bob's phase.
#[test]
fn the_water_bob_does_not_refuse_a_hold_spot() {
    let (mut sim, _) = super::super::tests::l_sim();
    sim.water_level = Some(0.18);
    sim.water_oscillates = true;
    sim.tick = 25; // quarter period: +6 cm, so the bobbed plane is 0.24
    let g = sim.nav.clone().expect("graph");
    let at = Vec2::new(g.hubs[0].pos.x, g.hubs[0].pos.z);
    let y = g.hubs[0].pos.y;
    let mut b = Builder {
        sim: &sim,
        g: &g,
        rays: 0,
    };
    let f = b.stand(at.x, at.y, y).expect("dry at the layer height");
    assert!(sim.water_under(Vec3::new(at.x, f, at.y)).unwrap() - 0.2 > f);
    sim.water_level = Some(0.25);
    let mut b = Builder {
        sim: &sim,
        g: &g,
        rays: 0,
    };
    assert!(b.stand(at.x, at.y, y).is_none(), "wet at the layer height");
}

/// The mover's "arrived" counts only near the position held: a bot whose
/// walk ended 4 m short of it (the outcome of its walk to the one before)
/// is still on its way; at it, it is there.
#[test]
fn a_walk_ended_short_of_the_position_held_is_not_its_arrival() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    run(&mut sim, e, 400, |_| {});
    let i = sim.unit(h).expect("bot").ai.tpos.held.expect("a position");
    let q = sim.positions.positions[i as usize].pos;
    for (off, there) in [(4.0, false), (0.3, true)] {
        let u = sim.unit_mut(h).expect("bot");
        u.soldier.pos = q + Vec3::new(0.0, 0.0, off);
        u.ai.fight.mode = FightState::TAKE;
        let u = sim.unit(h).expect("bot");
        let mut st = u.ai;
        let mut hs = st.hold.unwrap_or_default();
        let tk = crate::ai::brain::Tick {
            rules: crate::ai::UnitRules::for_unit(&sim, u),
            me: h,
            u,
            eye: u.muzzle(),
            wc: &sim.weapon_classes[u.weapon_class],
            tier: sim.difficulty.tier(),
            skill: sim.difficulty.skill_at(u.skill),
        };
        assert_eq!(
            walk_arrived(&sim, &tk, &mut st, &mut hs),
            there,
            "{off} m off"
        );
        let mode = if there {
            FightState::COVER
        } else {
            FightState::TAKE
        };
        assert_eq!(st.fight.mode, mode, "{off} m off");
    }
}

/// The 5-12 s timer at a covered position takes another only with cover
/// (the PS2's "another qualifying hint"): with an open point 5 m off and the
/// crate's other points within [`MOVE_MIN`], the bot keeps its cover.
#[test]
fn the_timer_from_cover_takes_only_cover() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    let mut all = sim.positions.positions.clone();
    let stats = sim.positions.stats;
    let mut q = all[all.len() - 1];
    q.pos = Vec3::new(5.0, 0.0, 0.0);
    q.source = Source::Hub;
    (q.low, q.corner, q.shut, q.tall) = (0, 0, 0, 0);
    all.push(q);
    sim.positions = PositionTable::index(all, stats);
    run(&mut sim, e, 400, |_| {});
    let now = sim.tick;
    let u = sim.unit_mut(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER, "at its position");
    assert!(u.ai.tpos.covered, "with cover");
    let i = u.ai.tpos.held.expect("a position");
    u.ai.tpos.next_at = now;
    u.ai.tpos.hits = 0;
    let mut kept = true;
    run(&mut sim, e, 30, |s| {
        let u = s.unit(h).expect("bot");
        kept &= u.ai.tpos.held == Some(i) && u.ai.tpos.covered;
    });
    assert!(kept, "it kept its cover");
    let u = sim.unit(h).expect("bot");
    assert!(
        u.ai.tpos.next_at > sim.tick,
        "the timer was asked and drawn again"
    );
}

#[test]
fn a_direction_falls_in_its_own_sector() {
    assert_eq!(sector_bit(Vec2::new(1.0, 0.0)), 1);
    assert_eq!(sector_bit(Vec2::new(0.0, 1.0)), 1 << 4);
    assert_eq!(sector_bit(Vec2::new(-1.0, 0.0)), 1 << 8);
    assert_eq!(sector_bit(Vec2::new(0.0, -1.0)), 1 << 12);
    // Just under half a sector off still rounds to the sector.
    let a = (0.49f32 / 16.0) * std::f32::consts::TAU;
    assert_eq!(sector_bit(Vec2::new(a.cos(), a.sin())), 1);
    assert_eq!(sector_bit(Vec2::ZERO), 0);
}

// ── Card TPOSR (AUDIT TPOS): the Tactician on a target out of sight, and
// the rules no test refereed (the audit's 15 surviving mutations).

/// The crate scene's bot at its covered position, then a wall 3 m high
/// between it and its target: the target out of its sight, its record kept
/// fresh (as hearing keeps it). The position held, `i`.
fn hidden_scene() -> (Sim, UnitHandle, UnitHandle, u32) {
    use crate::collision::{Body, Shape};
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    run(&mut sim, e, 400, |_| {});
    let u = sim.unit(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER, "at its position");
    assert!(u.ai.tpos.covered, "with cover");
    let i = u.ai.tpos.held.expect("a position");
    sim.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 1.5, -22.0)),
        Shape::Box {
            half: Vec3::new(4.0, 1.5, 0.3),
        },
    ));
    (sim, h, e, i)
}

/// One tick of the hidden scene: the bot's record of the target stamped
/// now, as a sound would (`perception::hear`).
fn heard(sim: &mut Sim, h: UnitHandle, e: UnitHandle, ticks: u32, mut each: impl FnMut(&Sim)) {
    for _ in 0..ticks {
        let t = sim.tick;
        if let Some(u) = sim.unit_mut(h) {
            for c in u.ai.contacts.iter_mut().flatten() {
                if c.handle == e {
                    c.last_seen = t;
                }
            }
        }
        run(sim, e, 1, &mut each);
    }
}

/// A bot hidden at its position whose target is out of its sight runs its
/// position as one in sight (the PS2's cover state machine runs every
/// update): 20 s on, the position is still its own and reserved every tick,
/// and its hide / fire swaps come within a think of their time (AUDIT TPOS:
/// a hidden spell ran 94 s, 20 % of held time went unreserved).
#[test]
fn a_hidden_bot_keeps_its_reservation_and_its_cycle() {
    let (mut sim, h, e, i) = hidden_scene();
    sim.unit_mut(h).expect("bot").ai.tpos.next_at = u64::MAX / 2;
    let (mut unseen, mut late, mut lapsed, mut lost) = (0u32, Vec::new(), Vec::new(), Vec::new());
    let mut swaps = 0u32;
    let mut was = None;
    heard(&mut sim, h, e, 600, |s| {
        let u = s.unit(h).expect("bot");
        let c = crate::perception::contact_of(&u.ai, e).expect("remembered");
        unseen += u32::from(!c.seen_now);
        if u.ai.tpos.held != Some(i) || u.ai.fight.mode != FightState::COVER {
            lost.push(s.tick);
            return;
        }
        if u.ai.tpos.until <= s.tick {
            lapsed.push(s.tick);
        }
        if s.tick > u.ai.tpos.phase_until + crate::ai::THINK_PERIOD {
            late.push(s.tick);
        }
        swaps += u32::from(was.is_some_and(|f| f != u.ai.tpos.firing));
        was = Some(u.ai.tpos.firing);
    });
    assert!(unseen >= 570, "out of sight: {unseen} of 600 ticks");
    assert!(lost.is_empty(), "the position left at {lost:?}");
    assert!(lapsed.is_empty(), "unreserved at {lapsed:?}");
    assert!(late.is_empty(), "swaps over a think late at {late:?}");
    assert!(swaps >= 4, "{swaps} swaps in 20 s");
}

/// ...and its leave tests run: hit twice out of sight of its target, it
/// leaves the position at the next think and does not ask for it again;
/// its 5-12 s timer is asked within a think of running out.
#[test]
fn a_hidden_bot_runs_its_leave_tests() {
    let (mut sim, h, e, i) = hidden_scene();
    sim.unit_mut(h).expect("bot").ai.tpos.next_at = u64::MAX / 2;
    heard(&mut sim, h, e, 60, |_| {});
    let (mut second, mut left) = (None, None);
    let mut watch = |s: &Sim| {
        let u = s.unit(h).expect("bot");
        if second.is_none() && u.ai.tpos.hits >= 2 {
            second = Some(s.tick);
        }
        if left.is_none() && u.ai.tpos.held != Some(i) {
            left = Some(s.tick);
        }
        assert_ne!(
            u.ai.tpos.held.filter(|_| left.is_some()),
            Some(i),
            "taken again"
        );
    };
    for _ in 0..2 {
        let t = sim.tick;
        sim.unit_mut(h).expect("bot").last_hit = Some((e, t - 1));
        heard(&mut sim, h, e, REACT_GAP as u32, &mut watch);
    }
    heard(&mut sim, h, e, 60, &mut watch);
    let u = sim.unit(h).expect("bot");
    assert!(
        !crate::perception::contact_of(&u.ai, e)
            .expect("remembered")
            .seen_now,
        "still out of sight"
    );
    let second = second.expect("hit twice");
    let left = left.expect("it left the position hit twice");
    assert!(
        left <= second + crate::ai::THINK_PERIOD,
        "left at {left}, hit twice by {second}"
    );
    assert!(u.ai.tpos.bad.contains(&Some(i)), "not asked for again");

    // The timer, in a fresh scene.
    let (mut sim, h, e, _) = hidden_scene();
    heard(&mut sim, h, e, 30, |_| {});
    let t0 = sim.tick;
    sim.unit_mut(h).expect("bot").ai.tpos.next_at = t0;
    heard(&mut sim, h, e, crate::ai::THINK_PERIOD as u32 + 1, |_| {});
    let u = sim.unit(h).expect("bot");
    assert!(
        u.ai.tpos.next_at > sim.tick || !u.ai.fight.tactician() || u.ai.tpos.at > t0,
        "the timer was not asked: next_at {} at {}",
        u.ai.tpos.next_at,
        sim.tick
    );
}

/// Flat ground (`l_sim`), the bot at `bot` facing an enemy at `enemy`, and
/// the position table replaced by `table`.
fn flat_scene(bot: Vec3, enemy: Vec3, table: Vec<Position>) -> (Sim, UnitHandle, UnitHandle) {
    let (mut sim, h) = super::super::tests::l_sim();
    let e = sim.spawn(&UnitSpec::trooper(1, enemy)).expect("enemy");
    let u = sim.unit_mut(h).expect("bot");
    u.soldier.pos = bot;
    u.soldier.yaw = crate::ai::yaw_toward(enemy - bot);
    let stats = sim.positions.stats;
    sim.positions = PositionTable::index(table, stats);
    (sim, h, e)
}

/// A position at `p` with LOW cover (shut at crouch height, open over it)
/// toward `toward`.
fn low_toward(p: Vec3, toward: Vec3) -> Position {
    let bits = sector_pair(xz(toward - p));
    Position {
        pos: p,
        source: Source::Edge,
        facing: Vec2::ZERO,
        stance: 0,
        stance2: 0,
        hub: u16::MAX,
        low: bits,
        corner: 0,
        shut: bits,
        tall: 0,
    }
}

/// Bot `h`'s Tactician asked once, against its seen enemy `e`.
fn ask(sim: &Sim, h: UnitHandle, e: UnitHandle, leaving: Option<(u32, bool)>) -> Option<Pick> {
    let u = sim.unit(h).expect("bot");
    let tk = crate::ai::brain::Tick {
        rules: crate::ai::UnitRules::for_unit(sim, u),
        me: h,
        u,
        eye: u.muzzle(),
        wc: &sim.weapon_classes[u.weapon_class],
        tier: sim.difficulty.tier(),
        skill: sim.difficulty.skill_at(u.skill),
    };
    let mut nt = NavTick::default();
    let foe = Foe::seen(sim.unit(e).expect("enemy"));
    choose(sim, &tk, &u.ai, &foe, leaving, false, None, &mut nt)
}

/// A position just LEFT stays reserved [`FREE_AFTER`] (the PS2's hint
/// rule): a teammate that left the crate's covered point a moment ago keeps
/// the bot off it and off every point within [`SPACING`] of it; once the 2 s
/// are over the bot takes the cover.
#[test]
fn a_position_just_left_stays_reserved() {
    let (mut sim, h, e) = two_hub_scene();
    let cov = covered_at_a(&sim);
    let centre = *cov
        .iter()
        .min_by(|a, b| {
            let p = |i: u32| sim.positions.positions[i as usize].pos.length();
            p(**a).total_cmp(&p(**b))
        })
        .expect("a covered point");
    let c = sim.positions.positions[centre as usize].pos;
    let m = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(30.0, 0.0, 30.0)))
        .expect("mate");
    // Every tick the mate holds the centre and leaves it (`release`).
    let leave = |s: &mut Sim| {
        let t = s.tick;
        let mu = s.unit_mut(m).expect("mate");
        mu.ai.tpos.held = Some(centre);
        release(&mut mu.ai, t);
    };
    let mut near = Vec::new();
    for _ in 0..240 {
        leave(&mut sim);
        run(&mut sim, e, 1, |s| {
            let u = s.unit(h).expect("bot");
            if let Some(q) =
                u.ai.tpos
                    .held
                    .map(|i| s.positions.positions[i as usize].pos)
            {
                if nav::xz_dist(q, c) < SPACING {
                    near.push(s.tick);
                }
            }
        });
    }
    assert!(
        near.is_empty(),
        "held within {SPACING} m of a left position at {near:?}"
    );
    let u = sim.unit_mut(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER, "at an open position");
    u.ai.tpos.next_at = u64::MAX / 2;
    // The mate's last leave is over its 2 s at the bot's next ask.
    let mut covered = false;
    run(&mut sim, e, (FREE_AFTER + 2 * RETRY) as u32, |s| {
        let u = s.unit(h).expect("bot");
        covered |= u.ai.tpos.held.is_some() && u.ai.tpos.covered;
    });
    assert!(covered, "it took the cover once the 2 s were over");
}

/// A Cover hint covers from a target its arrow points at only from the
/// bot's side of that target (the PS2's `(H - T).(T - me) < 0`,
/// re-soldier-ai 4.6): of two hints pointing at the target, the one on the
/// far side is no cover.
#[test]
fn a_cover_hint_covers_only_on_the_bots_side_of_the_target() {
    let (enemy, bot) = (Vec3::ZERO, Vec3::new(9.0, 0.0, 0.0));
    let hint = |p: Vec3| Position {
        pos: p,
        source: Source::Cover,
        facing: xz(enemy - p).normalize(),
        stance: 0,
        stance2: 0,
        hub: u16::MAX,
        low: 0,
        corner: 0,
        shut: 0,
        tall: 0,
    };
    // Both 7 m from the target (2 m nearer than the bot: card HYST's
    // closing rule passes them). Near: 6.3 m from the bot, on its side; far:
    // 14.8 m, beyond the target.
    let table = vec![
        hint(Vec3::new(5.0, 0.0, 4.9)),
        hint(Vec3::new(-5.0, 0.0, 4.9)),
    ];
    let (sim, h, e) = flat_scene(bot, enemy, table);
    let p = ask(&sim, h, e, None).expect("a position");
    let at = sim.positions.positions[p.idx as usize].pos;
    assert_eq!(
        (at, p.covered),
        (Vec3::new(5.0, 0.0, 4.9), true),
        "the near side's hint, as cover"
    );
}

/// The tier's "move" share of hits leaves the position on ONE hit (card
/// REACT's row): with the share at 1 every hit does, at 0 none does.
#[test]
fn the_tiers_move_share_leaves_on_one_hit() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    run(&mut sim, e, 400, |_| {});
    for (share, hits) in [(1.0, 2), (0.0, 1)] {
        let u = sim.unit(h).expect("bot");
        assert_eq!(u.ai.fight.mode, FightState::COVER);
        let mut tier = sim.difficulty.tier();
        tier.react[2] = share;
        let tk = crate::ai::brain::Tick {
            rules: crate::ai::UnitRules::for_unit(&sim, u),
            me: h,
            u,
            eye: u.muzzle(),
            wc: &sim.weapon_classes[u.weapon_class],
            tier,
            skill: sim.difficulty.skill_at(u.skill),
        };
        let mut st = u.ai;
        st.tpos.hits = 0;
        hit(&sim, &tk, &mut st);
        assert_eq!(st.tpos.hits, hits, "share {share}");
    }
}

/// A CORNER position (shut at both heights toward the target) leans out to
/// whichever side has the line: a pillar in front of it and to its right,
/// the left step's.
#[test]
fn a_corner_leans_to_the_side_with_the_line() {
    use crate::collision::{Body, Shape};
    let (enemy, at) = (Vec3::new(0.0, 0.0, -15.0), Vec3::ZERO);
    let mut q = low_toward(at, enemy);
    (q.low, q.corner, q.tall) = (0, q.shut, q.shut);
    let (mut sim, h, e) = flat_scene(Vec3::new(0.0, 0.0, 3.0), enemy, vec![q]);
    // (The lean side +1 is +x here: `firing_eye`.)
    sim.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.3, 1.5, -1.5)),
        Shape::Box {
            half: Vec3::new(1.0, 1.5, 0.3),
        },
    ));
    assert!(!line_open(
        &sim,
        firing_eye(at, enemy, 1),
        sim.unit(e).expect("e").muzzle()
    ));
    let p = ask(&sim, h, e, None).expect("a position");
    assert_eq!((p.covered, p.lean), (true, -1));
}

/// A position in a teammate's line of fire (within [`LINE`] of it) is not
/// chosen; out of it, it is.
#[test]
fn a_position_in_a_friends_line_of_fire_is_not_chosen() {
    let (enemy, p) = (Vec3::new(0.0, 0.0, -20.0), Vec3::new(0.0, 0.0, -6.0));
    let (mut sim, h, e) = flat_scene(Vec3::new(4.0, 0.0, -4.0), enemy, vec![low_toward(p, enemy)]);
    assert!(ask(&sim, h, e, None).is_some(), "no friend: taken");
    let m = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 4.0)))
        .expect("mate");
    let t = sim.tick;
    let mu = sim.unit_mut(m).expect("mate");
    mu.ai.target = Some(e.into());
    mu.ai.contacts[0] = Some(crate::perception::Contact {
        handle: e.into(),
        first_seen: t,
        last_seen: t,
        last_pos: enemy,
        seen_now: true,
        heard: false,
        told: false,
    });
    assert!(ask(&sim, h, e, None).is_none(), "in the mate's line");
}

/// A position by a grenade on the ground (inside its warning radius) is
/// not chosen.
#[test]
fn a_position_by_a_grenade_is_not_chosen() {
    let (enemy, p) = (Vec3::new(0.0, 0.0, -20.0), Vec3::new(0.0, 0.0, -6.0));
    let (mut sim, h, e) = flat_scene(Vec3::new(4.0, 0.0, -4.0), enemy, vec![low_toward(p, enemy)]);
    assert!(ask(&sim, h, e, None).is_some(), "no grenade: taken");
    sim.hazards.push(crate::sim::Hazard {
        pos: p + Vec3::new(1.0, 0.0, 0.0),
        radius: 3.0,
        since: 0,
        until: u64::MAX,
        owner: None,
        warning_id: None,
    });
    assert!(ask(&sim, h, e, None).is_none(), "by the grenade");
}

/// The weapon's band is grown OUTWARD by [`BAND_GROW`] at both edges: a
/// position 2 m inside its inner edge is still a candidate.
#[test]
fn the_band_is_grown_at_its_inner_edge() {
    let enemy = Vec3::new(0.0, 0.0, -20.0);
    let (sim, h, e) = flat_scene(Vec3::new(3.0, 0.0, -10.0), enemy, Vec::new());
    let wc = &sim.weapon_classes[sim.unit(h).expect("bot").weapon_class];
    let [lo, opt, _] = wc.ai_range;
    let edge = opt.min(lo + 3.0);
    // (Never on the target's own feet: a body stands within 1.5 m.)
    let d = (edge - 2.0).max(CROWD + 0.5);
    assert!(d < edge && d > edge - BAND_GROW, "edge {edge}, d {d}");
    let p = enemy + Vec3::new(0.0, 0.0, d);
    // (The bot 3 m beside it: no walk at the target, card HYST.)
    let (sim, h, e2) = flat_scene(
        p + Vec3::new(3.0, 0.0, 0.0),
        enemy,
        vec![low_toward(p, enemy)],
    );
    let _ = e;
    assert!(
        ask(&sim, h, e2, None).is_some(),
        "{d} m from the target, edge {edge}"
    );
}

/// The confirming ray: a position whose firing point has no line to the
/// target (a wall in front, whatever its masks say) is not chosen.
#[test]
fn a_position_with_no_line_to_the_target_is_not_chosen() {
    use crate::collision::{Body, Shape};
    let (enemy, p) = (Vec3::new(0.0, 0.0, -20.0), Vec3::new(0.0, 0.0, -6.0));
    let (mut sim, h, e) = flat_scene(Vec3::new(4.0, 0.0, -4.0), enemy, vec![low_toward(p, enemy)]);
    assert!(ask(&sim, h, e, None).is_some(), "a line: taken");
    sim.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 2.0, -9.0)),
        Shape::Box {
            half: Vec3::new(2.0, 2.0, 0.3),
        },
    ));
    assert!(ask(&sim, h, e, None).is_none(), "a wall in the line");
}

/// Another position, when one is left (not with the target round its
/// cover), lies [`MOVE_MIN`] from it: a covered one 2 m off is not taken.
#[test]
fn a_position_two_metres_from_the_one_left_is_not_taken() {
    let enemy = Vec3::new(0.0, 0.0, -20.0);
    let (left, next) = (Vec3::new(0.0, 0.0, -2.0), Vec3::new(2.0, 0.0, -2.0));
    let table = vec![low_toward(left, enemy), low_toward(next, enemy)];
    let (sim, h, e) = flat_scene(Vec3::new(2.0, 0.0, 0.0), enemy, table);
    let idx = |p: Vec3| {
        (0..sim.positions.positions.len() as u32)
            .find(|&i| sim.positions.positions[i as usize].pos == p)
            .expect("in the table")
    };
    let (l, n) = (idx(left), idx(next));
    assert!(ask(&sim, h, e, None).is_some(), "asked afresh: one of them");
    assert_eq!(ask(&sim, h, e, Some((l, false))), None, "leaving the first");
    assert_eq!(
        ask(&sim, h, e, Some((l, true))).map(|p| p.idx),
        Some(n),
        "the target round its cover: any other"
    );
}

/// B12's counter: fighting at its position with cover from its target -
/// within [`NEAR_ENOUGH`] of the position, the target on its shut side.
#[test]
fn covered_from_target_counts_only_at_the_position_and_its_cover() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    run(&mut sim, e, 400, |_| {});
    let u = sim.unit(h).expect("bot");
    let q = sim.positions.positions[u.ai.tpos.held.expect("a position") as usize].pos;
    for (off, toward, yes) in [
        (1.0, enemy, true),
        (3.0, enemy, false),
        (1.0, Vec3::new(0.0, 0.0, 25.0), false),
    ] {
        let u = sim.unit_mut(h).expect("bot");
        u.soldier.pos = q + Vec3::new(off, 0.0, 0.0);
        for c in u.ai.contacts.iter_mut().flatten() {
            c.last_pos = toward;
        }
        let u = sim.unit(h).expect("bot");
        assert_eq!(u.ai.fight.mode, FightState::COVER);
        assert_eq!(
            covered_from_target(&sim, u),
            yes,
            "{off} m off, target at {toward}"
        );
    }
}

/// A position more than [`CLIMB`] above the bot is walked onto only when
/// its floor is its hub's: one 2 m up with no hub is not chosen, one 1 m up
/// is.
#[test]
fn a_position_two_metres_up_off_its_hub_is_not_chosen() {
    let enemy = Vec3::new(0.0, 0.0, -20.0);
    for (dy, yes) in [(2.0, false), (1.0, true)] {
        let p = Vec3::new(0.0, dy, -6.0);
        let (sim, h, e) = flat_scene(Vec3::new(4.0, 0.0, -4.0), enemy, vec![low_toward(p, enemy)]);
        assert_eq!(ask(&sim, h, e, None).is_some(), yes, "{dy} m up");
    }
}

/// A walk to a position is given its straight length at [`TAKE_SPEED`] and
/// [`TAKE_SLACK`] more: 7.3 s for the crate's 13 m (a pinned bot's walk
/// ends by the mover's own 2 s give-up first, so the deadline is read where
/// it is drawn).
#[test]
fn a_walk_to_a_position_is_given_its_length_at_3_m_a_second_and_3_s() {
    let (mut sim, h, e) = two_hub_scene();
    let mut took = None;
    run(&mut sim, e, 120, |s| {
        let u = s.unit(h).expect("bot");
        if took.is_none() && u.ai.fight.mode == FightState::TAKE {
            took = Some((u.ai.tpos.at, u.ai.tpos.best, u.ai.tpos.phase_until));
        }
    });
    let (t0, travel, deadline) = took.expect("it took a position to walk to");
    assert!(travel > 10.0, "{travel} m");
    // Hard-coded: 3 m/s and 3 s at 30 ticks a second.
    let due = t0 + ((travel / 3.0 + 3.0) * 30.0) as u64;
    assert!(
        deadline.abs_diff(due) <= 1,
        "deadline {deadline}, due {due} ({travel} m)"
    );
}

/// A bot at its Tactician's position does not step out of a friend's line
/// (card FFLN's step is the fallback stander's): with a teammate standing in
/// its line to the target, no step is taken - its goal stays where it stands
/// and the step's own marks on the hold (the commit time, the side) are the
/// take's. (The cycle puts a stepped goal back on the same think; the marks
/// stay with a bot that falls back to its own fight. A bot walking to its
/// position is not a holder: no step either way.)
#[test]
fn a_bot_at_its_position_does_not_step_out_of_a_friends_line() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let (mut sim, h, e) = crate_scene(Vec3::new(0.3, 0.0, 0.3), enemy);
    run(&mut sim, e, 400, |_| {});
    let u = sim.unit(h).expect("bot");
    assert_eq!(u.ai.fight.mode, FightState::COVER);
    let i = u.ai.tpos.held.expect("a position");
    let stand = u.ai.tpos.stand;
    let marks = u.ai.hold.map(|hs| (hs.commit_until, hs.strafe_side));
    sim.unit_mut(h).expect("bot").ai.tpos.next_at = u64::MAX / 2;
    // A friend 6 m down the line, standing.
    let mid = stand + (enemy - stand).normalize() * 6.0;
    let _m = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(mid.x, 0.0, mid.z)))
        .expect("mate");
    let (mut at, mut off) = (0u32, Vec::new());
    run(&mut sim, e, 240, |s| {
        let u = s.unit(h).expect("bot");
        if u.ai.tpos.held != Some(i) {
            return;
        }
        at += 1;
        let hs = u.ai.hold.expect("a hold");
        if nav::xz_dist(hs.goal, stand) > LEAN + 0.1
            || Some((hs.commit_until, hs.strafe_side)) != marks
        {
            off.push(s.tick);
        }
    });
    assert!(at >= 200, "at its position {at} ticks");
    assert!(off.is_empty(), "stepped at {off:?}");
}

/// The hidden scene's target known only from a friend (a told record: no
/// rule of the fight runs on it, card TEAM) from the tick before a think:
/// the position stays reserved every tick it is still held, and a position
/// hit twice is left at the next think, not when the hold runs out.
#[test]
fn a_bot_on_a_told_record_keeps_its_reservation_and_leaves_hit_twice() {
    for hit_twice in [false, true] {
        let (mut sim, h, e, i) = hidden_scene();
        sim.unit_mut(h).expect("bot").ai.tpos.next_at = u64::MAX / 2;
        heard(&mut sim, h, e, 30, |_| {});
        while sim.unit(h).expect("bot").ai.next_think != sim.tick + 1 {
            heard(&mut sim, h, e, 1, |_| {});
        }
        let t0 = sim.tick;
        let (mut lapsed, mut left) = (Vec::new(), None);
        for k in 0..120u64 {
            let t = sim.tick;
            let u = sim.unit_mut(h).expect("bot");
            for c in u.ai.contacts.iter_mut().flatten() {
                if c.handle == e {
                    (c.last_seen, c.told) = (t, true);
                }
            }
            if hit_twice && k == 2 {
                u.ai.tpos.hits = 2;
            }
            run(&mut sim, e, 1, |s| {
                let u = s.unit(h).expect("bot");
                if u.ai.tpos.held == Some(i) && u.ai.fight.tactician() {
                    if u.ai.tpos.until <= s.tick {
                        lapsed.push(s.tick);
                    }
                } else if left.is_none() {
                    left = Some(s.tick);
                }
            });
        }
        let left = left.expect("the hold ran out");
        assert!(
            lapsed.is_empty(),
            "held unreserved at {lapsed:?} (from {t0})"
        );
        if hit_twice {
            assert!(
                left <= t0 + 3 + crate::ai::THINK_PERIOD,
                "hit twice at {}, left at {left}",
                t0 + 2
            );
        } else {
            assert!(
                left > t0 + crate::ai::THINK_PERIOD,
                "kept a while: left at {left}"
            );
        }
    }
}

/// A position at `p` with CORNER cover (shut at both heights) toward `toward`.
fn corner_toward(p: Vec3, toward: Vec3) -> Position {
    let bits = sector_pair(xz(toward - p));
    Position {
        low: 0,
        corner: bits,
        tall: bits,
        ..low_toward(p, toward)
    }
}

/// Bot `h`'s Tactician asked as the 5-12 s timer asks (card HYST): leaving
/// position `held`, which it keeps unless beaten by the margin.
fn ask_keep(
    sim: &Sim,
    h: UnitHandle,
    e: UnitHandle,
    held: u32,
    keep: bool,
) -> (Option<Pick>, NavTick) {
    let u = sim.unit(h).expect("bot");
    let tk = crate::ai::brain::Tick {
        rules: crate::ai::UnitRules::for_unit(sim, u),
        me: h,
        u,
        eye: u.muzzle(),
        wc: &sim.weapon_classes[u.weapon_class],
        tier: sim.difficulty.tier(),
        skill: sim.difficulty.skill_at(u.skill),
    };
    let mut nt = NavTick::default();
    let foe = Foe::seen(sim.unit(e).expect("enemy"));
    let k = keep.then_some(Pick {
        idx: held,
        covered: true,
        lean: 1,
    });
    let p = choose(sim, &tk, &u.ai, &foe, Some((held, false)), true, k, &mut nt);
    (p, nt)
}

/// Card HYST: the timer re-scores the position held beside its candidates.
/// LOW cover 3.5 m away outscores the CORNER the bot stands at (3 x 1.0 +
/// near 1.83 against 3 x 0.8 + near 2.0) but by less than 15 %: kept; asked
/// without the position held, the LOW one wins.
#[test]
fn the_position_held_is_kept_unless_another_beats_it_by_the_margin() {
    let enemy = Vec3::new(0.0, 0.0, -25.0);
    let table = vec![
        corner_toward(Vec3::ZERO, enemy),
        low_toward(Vec3::new(3.5, 0.0, 0.0), enemy),
    ];
    let (sim, h, e) = flat_scene(Vec3::ZERO, enemy, table);
    let (free, nt) = ask_keep(&sim, h, e, 0, false);
    assert_eq!(free.map(|p| p.idx), Some(1), "the LOW cover scores higher");
    assert_eq!(nt.tpos_hyst & HY_KEPT, 0);
    let (kept, nt) = ask_keep(&sim, h, e, 0, true);
    assert_eq!(kept.map(|p| p.idx), Some(0), "not by 15 %: kept");
    assert_eq!(nt.tpos_hyst & HY_KEPT, HY_KEPT);
    // An open position held gives way to the LOW cover (it beats it by far).
    let table = vec![
        low_toward(Vec3::ZERO, Vec3::new(0.0, 0.0, 25.0)),
        low_toward(Vec3::new(3.5, 0.0, 0.0), enemy),
    ];
    let (sim, h, e) = flat_scene(Vec3::ZERO, enemy, table);
    let (p, _) = ask_keep(&sim, h, e, 0, true);
    assert_eq!(p.map(|p| p.idx), Some(1), "cover beats the open spot");
}

/// Card HYST (The Last of Us' rule): from inside the band, cover 10 m
/// straight at a target 20 m off is not taken - the bot would run at the
/// enemy in the open; cover off to the side is, though farther.
#[test]
fn cover_reached_by_running_at_the_enemy_is_not_taken() {
    let enemy = Vec3::new(0.0, 0.0, -20.0);
    let ahead = low_toward(Vec3::new(0.0, 0.0, -10.0), enemy);
    let side = low_toward(Vec3::new(12.0, 0.0, -2.0), enemy);
    let (sim, h, e) = flat_scene(Vec3::ZERO, enemy, vec![ahead, side]);
    assert_eq!(
        ask(&sim, h, e, None).map(|p| p.idx),
        Some(1),
        "the side one"
    );
    // Alone, the one ahead is not taken at all.
    let (sim, h, e) = flat_scene(Vec3::ZERO, enemy, vec![ahead]);
    assert_eq!(ask(&sim, h, e, None).map(|p| p.idx), None);
    // A step forward (under 4 m) is, and 6 m forward (under 40 % of the
    // 20 m) too.
    for z in [-3.0, -6.0] {
        let near = low_toward(Vec3::new(0.0, 0.0, z), enemy);
        let (sim, h, e) = flat_scene(Vec3::ZERO, enemy, vec![near]);
        assert_eq!(ask(&sim, h, e, None).map(|p| p.idx), Some(0), "{z} m");
    }
}
