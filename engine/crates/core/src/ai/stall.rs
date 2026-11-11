//! Card RCYC (the maintainer, 2026-10-01): the LAST RESORT for a bot that is on the
//! level and going nowhere — "it should be available in case a bot gets
//! somehow stuck somewhere (which should never happen because we close all
//! those gaps where it could get stuck)". [`watch`] is the whole rule; the
//! tests of what is and is not recycled live here.
//!
//! Stock conquest has no exit once the losing side is clamped at alive =
//! tickets (re-round-stalls.md C7): one bot that cannot move and cannot be
//! reached holds the round open for ever. Card BNDS gives up a bot that is
//! OFF the level ([`super::LostState`]); this gives up one that is on it. The
//! rule is ours (PROVISIONAL): the PS2 sets a stuck flag and what its agents
//! do with it is unread.
//!
//! The test is NET progress, not the route's stuck window — a wall slide or
//! a door that pushes a crowd back and forth resets that window every time,
//! and it is not asked of one leg (a stuck bot's leg flips between two
//! hubs). A bot PACING a wall moves metres every second, so in the endgame,
//! where it holds the round open, progress is NEW ground only; mid-round it
//! is [`RECYCLE_ROAM`] from where the count began, which a bot lapping a
//! dead end never gets (card OBSR).
//!
//! Card RLOOP (the maintainer, 2026-10-11: "they just stand there ... for a few
//! minutes?"): two minutes is too long for a bot that does not move at all.
//! One whose walk FAILS [`PIN_FAILS`] progress windows without ever being
//! [`PIN_ROAM`] from where the first failed is given up after about 22 s:
//! [`send_off`] walks it round what stopped it along the floor (card RETW's
//! search) when it can, and recycles it when it cannot.
//! The count is of failed windows, so a crowd pushing through a door and a
//! bot going round an obstacle (whose windows do not fail) are not in it.
//!
//! Card UNSN: a give-up here (`NavTick::stalled`) is first offered to
//! [`super::unseen`], which moves the bot on where no human sees it; only
//! when that finds no safe spot does the recycle happen.

use super::state::{AiState, LostState, NavTick, StallState};
use super::{
    PIN_FAILS, PIN_ROAM, PIN_WALKED, RECYCLE_AFTER, RECYCLE_NET, RECYCLE_ROAM, RETURN_PLANS,
};
use crate::sim::Sim;
use glam::Vec3;

/// One tick of the watch for a bot at `pos`.
///
/// `arrived` — it is where it wants to be (on its post or defence spot, or
/// it has nowhere to go): the watch closes. `walking` — it wants to walk
/// this tick and nothing excuses it (not on a held position, not fighting an
/// enemy it sees, not running from a grenade, not under a squad order): the
/// tick counts. Any other tick the watch waits.
///
/// Progress starts the count again. Mid-round it is [`RECYCLE_ROAM`] from
/// where the count started (`at`). In the `endgame` — a side down to its
/// last reinforcements, where one bot going nowhere holds the round open —
/// it is NEW GROUND only: [`RECYCLE_NET`] outside the box of everything
/// covered since the watch opened, so a bot pacing a wall is caught. A bot
/// that has counted [`RECYCLE_AFTER`] ticks without progress is given up
/// (`NavTick::stalled`; `Sim::step`'s write-back kills it as
/// [`crate::sim::DeathCause::Stalled`] — with its ticket, like a lost bot's,
/// so a 1 v 1 stall ends at once — and it respawns at an owned post).
///
/// Card RLOOP: `failed` — the walk's progress windows that failed this tick
/// (1 for a leg's, 2 for a lost bot's return walk, whose test fails every
/// other window). They are counted while the bot stays within [`PIN_ROAM`]
/// of where the first one happened, through every new plan, and at
/// [`PIN_FAILS`] a bot off the level is given up as lost
/// (`NavTick::recycle`); one on it is handed to [`send_off`] (the return
/// value: true on that tick).
pub(super) fn watch(
    st: &mut AiState,
    pos: Vec3,
    arrived: bool,
    walking: bool,
    endgame: bool,
    failed: u8,
    nt: &mut NavTick,
) -> bool {
    if arrived {
        st.stall = None;
        return false;
    }
    // Card RLOOP: the failures count at ONE place — away from it, on any
    // tick, they start again.
    if let Some(s) = st.stall.as_mut() {
        if s.fails > 0 && (pos - s.pin).length_squared() >= PIN_ROAM * PIN_ROAM {
            s.fails = 0;
        }
    }
    if !walking {
        return false;
    }
    let mut s = st.stall.unwrap_or(StallState {
        at: pos,
        lo: pos,
        hi: pos,
        ticks: 0,
        pin: pos,
        fails: 0,
        walked: None,
    });
    let mut pinned = false;
    if failed > 0 {
        if s.fails == 0 {
            s.pin = pos;
        }
        s.fails = s.fails.saturating_add(failed);
        if s.fails >= PIN_FAILS {
            // A bot off the level is given up as lost, as by its own clock.
            if st.lost.is_some_and(|l| l.confirmed && !l.pinned) {
                nt.recycle = 1;
            } else {
                pinned = true;
            }
        }
    }
    let new_ground = (s.lo - pos).max(pos - s.hi).max_element() >= RECYCLE_NET;
    if new_ground {
        s.lo = s.lo.min(pos);
        s.hi = s.hi.max(pos);
    }
    let moved = (pos - s.at).length_squared() >= RECYCLE_ROAM * RECYCLE_ROAM;
    if new_ground || (moved && !endgame) {
        s.at = pos;
        s.ticks = 0;
    } else {
        s.ticks += 1;
        if s.ticks >= RECYCLE_AFTER {
            nt.stalled = 1;
        }
    }
    st.stall = Some(s);
    pinned
}

/// Card RLOOP: what becomes of a bot on the level that [`watch`] found
/// pinned. Card RETW's floor search (`Sim::floor_walk_to`) looks for a walk
/// from its feet to its leg's waypoint, round what the steering could not
/// get past; with one, the bot walks it as a [`LostState::pinned`] spell
/// and its count starts again. With none — no leg, no walk on its floor
/// (BF2 Utapau's pit, 21 m under hub 60), or pinned again within
/// [`PIN_WALKED`] of where its last walk began (`StallState::walked`), or
/// its walk got nowhere — it is recycled (`NavTick::stalled`). Ours,
/// PROVISIONAL, like the count.
pub(super) fn send_off(sim: &Sim, st: &mut AiState, pos: Vec3, nt: &mut NavTick) {
    let Some(mut s) = st.stall else {
        return;
    };
    let again = st.lost.is_some_and(|l| l.pinned)
        || s.walked
            .is_some_and(|w| (pos - w).length_squared() < PIN_WALKED * PIN_WALKED);
    let walk = st
        .route()
        .filter(|_| !again)
        .and_then(|r| sim.floor_walk_to(pos, r.waypoint));
    match walk {
        Some(path) if path.len > 0 => {
            st.lost = Some(LostState {
                since: sim.tick,
                confirmed: true,
                pinned: true,
                path,
                plans: RETURN_PLANS,
                ..LostState::default()
            });
            s.fails = 0;
            s.walked = Some(pos);
            st.stall = Some(s);
            nt.failed(super::why::Why::Pinned);
        }
        _ => nt.stalled = 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::l_sim;
    use crate::ai::HoldState;
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::cp::CpDef;
    use crate::nav;
    use crate::region::Region;
    use crate::sim::{DeathCause, Event, PadState, Sim};
    use crate::unit::{UnitHandle, UnitSpec};
    use glam::Mat4;

    /// A pen round the origin: four walls a metre high, 2 m from the centre
    /// — too high to step over, low enough to see and be seen over.
    fn pen(sim: &mut Sim) {
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
    }

    /// The L map with the bot's own post behind it, outside any pen.
    fn home_sim() -> (Sim, UnitHandle, usize) {
        let (mut sim, h) = l_sim();
        let home = Vec3::new(0.0, 0.0, 60.0);
        let cp = sim.add_cp(
            CpDef {
                name: "home".into(),
                pos: home,
                capture: Region::sphere(home, 6.0),
                control: None,
                value_bleed: 100,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: vec![(home, 0.0)],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(0),
        );
        (sim, h, cp)
    }

    /// Card UNSN: a human of the bot's team 10 m south of the origin - over
    /// the pen's wall he sees the penned bot, so its recycle stays (no
    /// unseen move).
    fn watched(sim: &mut Sim) {
        sim.spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, 10.0)))
            .expect("human");
    }

    /// Step `ticks`, calling `each` after every one; the ticks a bot died as
    /// stalled.
    fn stalls(sim: &mut Sim, ticks: u64, mut each: impl FnMut(&mut Sim)) -> Vec<u64> {
        let mut at = Vec::new();
        for _ in 0..ticks {
            sim.step(&[PadState::default()]);
            if sim.events.iter().any(|e| {
                matches!(
                    e,
                    Event::EnvDeath {
                        cause: DeathCause::Stalled,
                        ..
                    }
                )
            }) {
                at.push(sim.tick);
            }
            each(sim);
        }
        at
    }

    /// A bot penned behind a wall with its objective outside is recycled
    /// once, after `RECYCLE_AFTER`, and respawns at a post its team owns —
    /// from where it walks on and is not recycled again.
    #[test]
    fn a_penned_bot_is_recycled_once_and_respawns_at_an_owned_post() {
        let (mut sim, h, cp) = home_sim();
        pen(&mut sim);
        watched(&mut sim);
        let tickets = sim.battle.teams[0].tickets();
        let after = u64::from(RECYCLE_AFTER);
        assert!(stalls(&mut sim, after - 1, |_| {}).is_empty(), "not before");
        let mut left = 150;
        while stalls(&mut sim, 1, |_| {}).is_empty() {
            left -= 1;
            assert!(left > 0, "recycled at RECYCLE_AFTER");
        }
        assert!(sim.unit(h).is_none(), "the penned life is over");
        assert_eq!(
            sim.battle.teams[0].tickets(),
            tickets - 1,
            "with its ticket"
        );
        // The next wave puts it on its own post.
        let mut back = None;
        for _ in 0..600 {
            sim.step(&[PadState::default()]);
            if sim
                .events
                .iter()
                .any(|e| matches!(e, Event::Respawn { .. }))
            {
                back = Some(sim.handle(h.index as usize));
                break;
            }
        }
        let u = sim.unit(back.expect("respawned")).expect("alive");
        assert!(
            nav::xz_dist(u.soldier.pos, sim.cps[cp].def.pos) < 5.0,
            "at its team's post: {}",
            u.soldier.pos
        );
        assert!(
            u.ai.stall.is_none_or(|s| s.ticks <= 1),
            "a new life, a new watch: {:?}",
            u.ai.stall
        );
        // Outside the pen it walks to the enemy post like any bot.
        let again = stalls(&mut sim, after + 300, |_| {});
        assert!(again.is_empty(), "not again: {again:?}");
    }

    /// A YARD the bot cannot leave but can walk the length of: 13 m by 4 m
    /// inside its walls, the objective outside.
    fn long_pen(sim: &mut Sim) {
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
                wall(7.5, 0.0, 0.5, 3.0),
                wall(-7.5, 0.0, 0.5, 3.0),
                wall(0.0, 2.5, 8.0, 0.5),
                wall(0.0, -2.5, 8.0, 0.5),
            ],
            Vec::new(),
        );
    }

    /// Card OBSR: a bot in a yard it cannot leave but can walk the length
    /// of goes round its walls for good — it is never within a few metres of
    /// one place for long (card RCYC's 5 m: not recycled in six minutes) —
    /// and is given up all the same: it is never [`RECYCLE_ROAM`] from where
    /// its count began.
    #[test]
    fn a_bot_lapping_a_yard_it_cannot_leave_is_recycled() {
        let (mut sim, h, _) = home_sim();
        long_pen(&mut sim);
        watched(&mut sim);
        let after = u64::from(RECYCLE_AFTER);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let early = stalls(&mut sim, after - 1, |s| {
            if let Some(u) = s.unit(h) {
                lo = lo.min(u.soldier.pos);
                hi = hi.max(u.soldier.pos);
            }
        });
        assert!(early.is_empty(), "not before: {early:?}");
        assert!(
            hi.x - lo.x > 2.0 * RECYCLE_NET,
            "it walks the yard: {lo} .. {hi}"
        );
        let at = stalls(&mut sim, 600, |_| {});
        assert_eq!(at.len(), 1, "given up once: {at:?}");
        assert!(sim.unit(h).is_none(), "the lapping life is over");
    }

    /// The same pen with an enemy in sight over its wall: the bot is in a
    /// firefight, and a firefight is not a stall.
    #[test]
    fn a_bot_fighting_an_enemy_it_sees_is_not_recycled() {
        let (mut sim, h, _) = home_sim();
        pen(&mut sim);
        let e = sim
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -32.0)))
            .expect("enemy");
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 600, |s| {
            s.unit_mut(e).expect("enemy").health.cur = 300.0;
        });
        assert!(at.is_empty(), "{at:?}");
        let u = sim.unit(h).expect("alive");
        assert!(
            u.soldier.pos.length() < 3.0,
            "still penned: {}",
            u.soldier.pos
        );
        assert!(
            u.ai.stall.is_none_or(|s| s.ticks < RECYCLE_AFTER / 4),
            "the watch waited: {:?}",
            u.ai.stall
        );
    }

    /// Card VMESH: a bot standing in the capture region of the post it is
    /// sent to - capturing, where the stall watch closes - but shut inside
    /// a closed prop is given up as lost within `SHUT_RECYCLE` and a think;
    /// the same bot in the open, in a shell open below (a hollow trunk:
    /// not watertight) or in a closed shell with a planning hub inside (a
    /// room the bots walk) is not. (BF1 Endor's cp2 boulder lies in the
    /// post's region: one bot stood inside it for 806 s.)
    #[test]
    fn a_bot_shut_inside_a_closed_body_is_given_up_while_capturing() {
        // cp1's hub is at (40, 0, -40), its region 6 m round it.
        let run = |spot: Vec3, body: Option<(Vec3, bool)>| {
            let (mut sim, h) = l_sim();
            sim.units[h.index as usize].soldier.pos = spot;
            if let Some((centre, open_bottom)) = body {
                sim.collision = CollisionWorld::new(
                    vec![crate::collision::box_mesh(
                        centre,
                        Vec3::new(2.5, 3.0, 2.5),
                        open_bottom,
                    )],
                    Vec::new(),
                );
            }
            let mut lost_at = None;
            let mut capturing = 0;
            for _ in 0..(crate::ai::SHUT_RECYCLE + 3 * crate::ai::THINK_PERIOD) {
                sim.step(&[PadState::default()]);
                if let Some(u) = sim.unit(h).filter(|u| u.alive) {
                    if sim.cps[0].def.capture.contains(u.soldier.pos) && u.ai.target_cp == Some(0) {
                        capturing += 1;
                    }
                }
                let died = sim.events.iter().any(|e| {
                    matches!(e, Event::EnvDeath { unit, cause: DeathCause::Lost } if *unit == h)
                });
                if died && lost_at.is_none() {
                    lost_at = Some(sim.tick);
                }
            }
            assert!(capturing > 0, "it was at its post");
            lost_at
        };
        let spot = Vec3::new(43.5, 0.0, -40.0);
        let up = Vec3::Y * 2.0;
        let t = run(spot, Some((spot + up, false))).expect("never given up");
        assert!(
            t >= crate::ai::SHUT_RECYCLE,
            "given up at {t}, before the bound"
        );
        assert_eq!(run(spot, None), None, "in the open");
        assert_eq!(run(spot, Some((spot + up, true))), None, "open below");
        let hub = Vec3::new(41.0, 0.0, -40.0);
        assert_eq!(
            run(hub, Some((Vec3::new(40.0, 2.0, -40.0), false))),
            None,
            "a hub inside"
        );
    }

    /// A bot holding a position (card HOLD) stays put by choice.
    #[test]
    fn a_holder_is_not_recycled() {
        // Card FGHT: a shooter that keeps its place.
        let _style = crate::ai::tests::all_stand();
        let (mut sim, h) = l_sim();
        let e = sim
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -32.0)))
            .expect("enemy");
        sim.unit_mut(h).expect("alive").ai.hold = Some(HoldState {
            until: u64::MAX,
            shuffle_at: 30,
            ..HoldState::default()
        });
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 600, |s| {
            s.unit_mut(e).expect("enemy").health.cur = 300.0;
        });
        assert!(at.is_empty(), "{at:?}");
        let u = sim.unit(h).expect("alive");
        assert!(u.ai.hold.is_some(), "still on its position");
        assert!(u.soldier.pos.length() < 10.0, "{}", u.soldier.pos);
    }

    /// A defender that has reached its spot is where it wants to be.
    #[test]
    fn a_defender_on_its_spot_is_not_recycled() {
        let (mut sim, h) = l_sim();
        sim.cps[0].owner = Some(0);
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 1200, |_| {});
        assert!(at.is_empty(), "{at:?}");
        let u = sim.unit(h).expect("alive");
        assert!(
            nav::xz_dist(u.soldier.pos, Vec3::new(40.0, 0.0, -40.0)) < 8.0,
            "on its post: {}",
            u.soldier.pos
        );
        assert_eq!(u.ai.stall, None, "arrived: no watch");
    }

    /// The L map's post raised onto a slab 3 m over the ground, hollow
    /// underneath: BF1 Geonosis's post platforms.
    fn slab_sim() -> (Sim, UnitHandle) {
        let (mut sim, h) = l_sim();
        let top = Vec3::new(40.0, 3.0, -40.0);
        sim.cps[0].def.pos = top;
        sim.cps[0].def.capture = Region::sphere(top, 6.0);
        sim.collision = CollisionWorld::new(
            vec![Body::new(
                Mat4::from_translation(top - Vec3::Y * 0.1),
                Shape::Box {
                    half: Vec3::new(4.0, 0.1, 4.0),
                },
            )],
            Vec::new(),
        );
        (sim, h)
    }

    /// Card STEND: a bot under its post's floor — inside the capture volume,
    /// on the centre in XZ — has not arrived, so the watch runs and gives it
    /// up (BF1 GEO1 seed 42: a droid left under a post's platform by its
    /// walker held the round open at 9 v 2). Card PARR: its point is on the
    /// post's floor or beside the platform, never under it; kept under it
    /// (its walker's shell, in the map this stands for) it is given up.
    #[test]
    fn a_bot_under_its_posts_floor_has_not_arrived() {
        let (mut sim, h) = slab_sim();
        let under = Vec3::new(40.0, 0.0, -40.0);
        sim.unit_mut(h).expect("alive").soldier.pos = under;
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 150, |s| {
            if let Some(u) = s.unit_mut(h) {
                u.soldier.pos = under;
            }
        });
        assert_eq!(at.len(), 1, "given up once: {at:?}");
    }

    /// Card PARR: ...and free to walk, it comes out from under the platform
    /// and stands beside it, inside the capture region.
    #[test]
    fn a_bot_under_its_posts_floor_walks_out_beside_it() {
        let (mut sim, h) = slab_sim();
        sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(40.0, 0.0, -40.0);
        // (Card DEFS: the post stays under threat, so its one defender is
        // not sent on after 20 quiet seconds - there is no other post here.)
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 150, |s| {
            s.commander[0].threat = vec![s.tick];
        });
        assert!(at.is_empty(), "{at:?}");
        let u = sim.unit(h).expect("alive");
        let d = (u.soldier.pos - Vec3::new(40.0, 0.0, -40.0)).abs();
        assert!(d.x.max(d.z) > 4.0, "not under it: {}", u.soldier.pos);
        assert!(sim.cps[0].def.capture.contains(u.soldier.pos));
        assert_eq!(u.ai.stall, None, "arrived: no watch");
    }

    /// The mirror: the post on the ground and the bot on a roof over it
    /// (TAT1's cp2: the last rebel 5 m over the post, 16 v 1).
    #[test]
    fn a_bot_on_a_roof_over_its_post_has_not_arrived() {
        let (mut sim, h) = slab_sim();
        let ground = Vec3::new(40.0, 0.0, -40.0);
        sim.cps[0].def.pos = ground;
        sim.cps[0].def.capture = Region::sphere(ground, 6.0);
        sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(40.0, 3.0, -40.0);
        // (Card HUBP: its first leg is to its own point of the post's hub, a
        // walk across the roof the watch counts as progress — 400 ticks in
        // hand, not 150.)
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 400, |s| {
            // Kept on the roof: it has no way down in the map this stands for.
            if let Some(u) = s.unit_mut(h) {
                u.soldier.pos.y = u.soldier.pos.y.max(3.0);
            }
        });
        assert_eq!(at.len(), 1, "given up once: {at:?}");
    }

    /// ...and one standing on that floor has.
    #[test]
    fn a_bot_on_its_posts_floor_has_arrived() {
        let (mut sim, h) = slab_sim();
        sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(40.0, 3.0, -40.0);
        let at = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 150, |_| {});
        assert!(at.is_empty(), "{at:?}");
        let u = sim.unit(h).expect("alive");
        let d = (u.soldier.pos - Vec3::new(40.0, 3.0, -40.0)).abs();
        assert!(
            d.x.max(d.z) < 4.0 && d.y < 0.05,
            "on the slab: {}",
            u.soldier.pos
        );
        assert_eq!(u.ai.stall, None, "arrived: no watch");
    }

    /// The rule by itself: waiting ticks do not count, `RECYCLE_ROAM` from
    /// where the count began starts it again mid-round, only new ground
    /// does in the endgame, an arrival closes the watch.
    #[test]
    fn the_watch_counts_walking_ticks_and_progress_starts_the_count_again() {
        let mut st = AiState::default();
        let mut nt = NavTick::default();
        let p = Vec3::new(1.0, 0.0, 1.0);
        for _ in 0..RECYCLE_AFTER - 1 {
            watch(&mut st, p, false, true, false, 0, &mut nt);
            watch(&mut st, p, false, false, false, 0, &mut nt); // a waiting tick
        }
        assert_eq!(nt.stalled, 0);
        assert_eq!(st.stall.map(|s| s.ticks), Some(RECYCLE_AFTER - 1));
        // Just inside RECYCLE_ROAM is not progress; RECYCLE_ROAM is.
        let far = p + Vec3::X * RECYCLE_ROAM;
        watch(
            &mut st,
            far - Vec3::X * 0.1,
            false,
            false,
            false,
            0,
            &mut nt,
        );
        assert_eq!(st.stall.map(|s| s.ticks), Some(RECYCLE_AFTER - 1));
        watch(&mut st, far, false, true, false, 0, &mut nt);
        assert_eq!(nt.stalled, 0);
        assert_eq!(
            st.stall,
            Some(StallState {
                at: far,
                lo: p,
                hi: far,
                ticks: 0,
                pin: p,
                fails: 0,
                walked: None,
            })
        );
        // Pacing between two points: mid-round each crossing is progress...
        let pace = |st: &mut AiState, nt: &mut NavTick, endgame: bool| {
            for k in 0..RECYCLE_AFTER {
                let at = if k % 2 == 0 { p } else { far };
                watch(st, at, false, true, endgame, 0, nt);
            }
        };
        pace(&mut st, &mut nt, false);
        assert_eq!(nt.stalled, 0, "a pacer is left alone mid-round");
        // ...and in the endgame it is ground already covered.
        pace(&mut st, &mut nt, true);
        assert_eq!(nt.stalled, 1, "and given up in the endgame");
        // New ground is progress in the endgame too.
        let mut nt = NavTick::default();
        watch(&mut st, far + Vec3::Z * 30.0, false, true, true, 0, &mut nt);
        assert_eq!((nt.stalled, st.stall.map(|s| s.ticks)), (0, Some(0)));
        watch(&mut st, p, true, true, true, 0, &mut nt);
        assert_eq!(st.stall, None, "an arrival closes the watch");
    }

    /// Card RLOOP: the walk's failed windows count at one place — through
    /// waiting ticks, not on them; [`PIN_ROAM`] away they start again; at
    /// [`PIN_FAILS`] the bot is given up, as lost when it is off the level.
    #[test]
    fn failed_windows_at_one_place_give_the_bot_up() {
        let mut st = AiState::default();
        let mut nt = NavTick::default();
        let p = Vec3::new(1.0, 0.0, 1.0);
        let near = p + Vec3::X * (PIN_ROAM - 0.1);
        for k in 0..PIN_FAILS - 1 {
            // (Standing a little way off is the same place.)
            let at = if k % 2 == 0 { p } else { near };
            watch(&mut st, at, false, true, false, 1, &mut nt);
            watch(&mut st, at, false, true, false, 0, &mut nt);
            // A failure on a tick it is excused for is not counted.
            watch(&mut st, at, false, false, false, 1, &mut nt);
        }
        assert_eq!(st.stall.map(|s| (s.pin, s.fails)), Some((p, PIN_FAILS - 1)));
        assert_eq!((nt.stalled, nt.recycle), (0, 0), "not before the bound");
        // Away from the place the count starts again, at the next failure's.
        let far = p + Vec3::X * PIN_ROAM;
        watch(&mut st, far, false, false, false, 0, &mut nt);
        assert_eq!(st.stall.map(|s| s.fails), Some(0));
        watch(&mut st, far, false, true, false, 1, &mut nt);
        assert_eq!(st.stall.map(|s| (s.pin, s.fails)), Some((far, 1)));
        assert_eq!((nt.stalled, nt.recycle), (0, 0));
        // A lost bot's failed return window counts two, and it dies as lost.
        let mut lost = st;
        lost.lost = Some(crate::ai::LostState {
            confirmed: true,
            ..Default::default()
        });
        let mut ln = NavTick::default();
        for _ in 0..PIN_FAILS / 2 {
            watch(&mut lost, far, false, true, false, 2, &mut ln);
        }
        assert_eq!((ln.stalled, ln.recycle), (0, 1), "given up as lost");
        // On the level: handed to `send_off` at the bound and not one
        // failure before; the watch itself gives nobody up.
        for _ in 1..PIN_FAILS - 1 {
            assert!(!watch(&mut st, far, false, true, false, 1, &mut nt));
        }
        assert!(watch(&mut st, far, false, true, false, 1, &mut nt));
        assert_eq!((nt.stalled, nt.recycle), (0, 0));
        // A bot on a pinned walk is on the level: not given up as lost.
        let mut walk = st;
        walk.lost = Some(crate::ai::LostState {
            confirmed: true,
            pinned: true,
            ..Default::default()
        });
        let mut wn = NavTick::default();
        assert!(watch(&mut walk, far, false, true, false, 2, &mut wn));
        assert_eq!((wn.stalled, wn.recycle), (0, 0));
        // An arrival closes this count with the watch.
        watch(&mut st, far, true, true, false, 0, &mut nt);
        assert_eq!(st.stall, None);
    }

    /// A cell the bot cannot take two steps in: 1.6 m inside its walls,
    /// `h` m high.
    fn cell(sim: &mut Sim, h: f32) {
        let wall = |x: f32, z: f32, hx: f32, hz: f32| {
            Body::new(
                Mat4::from_translation(Vec3::new(x, h / 2.0, z)),
                Shape::Box {
                    half: Vec3::new(hx, h / 2.0, hz),
                },
            )
        };
        sim.collision = CollisionWorld::new(
            vec![
                wall(1.3, 0.0, 0.5, 1.8),
                wall(-1.3, 0.0, 0.5, 1.8),
                wall(0.0, 1.3, 1.8, 0.5),
                wall(0.0, -1.3, 1.8, 0.5),
            ],
            Vec::new(),
        );
    }

    /// Card RLOOP: a bot shut in where its steering cannot move it, its
    /// objective outside, is handed one plan after another (BF2 Dagobah,
    /// main 8ad30f4: two bots at one barrier for 150 s each). Run it to
    /// [`PIN_FAILS`] failed windows; the plans it was handed and how often
    /// the Recover ladder's count began again.
    fn pin(sim: &mut Sim, h: UnitHandle) -> (Vec<(u16, Option<u16>)>, u32) {
        let bound = u64::from(PIN_FAILS) * u64::from(crate::ai::STUCK_WINDOW);
        // (Every window of a shut-in bot may fail, and none before the
        // first has run.)
        let (mut plans, mut resets, mut last) = (Vec::new(), 0, 0);
        let early = stalls(sim, bound - 1, |s| {
            if let Some(u) = s.unit(h) {
                if let Some(r) = u.ai.route() {
                    if !plans.contains(&(r.next_hub, r.via_arc)) {
                        plans.push((r.next_hub, r.via_arc));
                    }
                }
                resets += u32::from(u.ai.mv.recover < last);
                last = u.ai.mv.recover;
                assert!(u.ai.lost.is_none(), "no walk before the bound");
            }
        });
        assert!(
            early.is_empty(),
            "not before {PIN_FAILS} windows: {early:?}"
        );
        assert!(plans.len() >= 2, "two plans in turn: {plans:?}");
        (plans, resets)
    }

    /// ...with no floor walk out (walls 3 m high) it is given up within a
    /// few windows of the bound, not after the two minutes of
    /// [`RECYCLE_AFTER`]; it respawns at its post and walks on.
    #[test]
    fn a_bot_shut_in_one_place_is_given_up_within_the_bound() {
        let (mut sim, h, _) = home_sim();
        cell(&mut sim, 3.0);
        let (plans, resets) = pin(&mut sim, h);
        let mut left = 6 * u64::from(crate::ai::STUCK_WINDOW);
        while stalls(&mut sim, 1, |_| {}).is_empty() {
            left -= 1;
            assert!(
                left > 0,
                "given up within {} windows (plans {plans:?}, the ladder's count began again {resets} times)",
                PIN_FAILS + 6
            );
        }
        assert!(sim.unit(h).is_none(), "the shut-in life is over");
        assert!(
            sim.tick < u64::from(RECYCLE_AFTER) / 3,
            "long before the last resort"
        );
        // The next life starts at its post, outside, and is not given up.
        let again = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 300, |_| {});
        assert!(again.is_empty(), "not again: {again:?}");
    }

    /// ...with one (walls a metre high: card RETW's floor walk steps over,
    /// as a lost bot's jump takes it) it is sent on that walk at the bound,
    /// gets out, reaches its waypoint, walks on and is never given up.
    #[test]
    fn a_bot_pinned_with_a_floor_walk_out_walks_it() {
        let (mut sim, h, _) = home_sim();
        cell(&mut sim, 1.0);
        pin(&mut sim, h);
        let mut sent = None;
        let given = stalls(&mut sim, 3 * u64::from(crate::ai::STUCK_WINDOW), |s| {
            if let Some(l) = s.unit(h).and_then(|u| u.ai.lost) {
                sent = sent.or(Some((s.tick, l)));
            }
        });
        assert!(given.is_empty(), "not given up: {given:?}");
        let (at, l) = sent.expect("sent on a floor walk within the bound");
        assert!(l.pinned && l.confirmed && l.path.len > 0, "{l:?}");
        assert!(at < 12 * u64::from(crate::ai::STUCK_WINDOW));
        let after = stalls(&mut sim, u64::from(RECYCLE_AFTER) + 300, |_| {});
        let u = sim.unit(h).expect("the same life");
        assert!(after.is_empty(), "never given up: {after:?}");
        assert!(u.ai.lost.is_none(), "the walk is over");
        let p = u.soldier.pos;
        assert!(p.x.abs() > 2.5 || p.z.abs() > 2.5, "out of the cell: {p}");
    }
}
