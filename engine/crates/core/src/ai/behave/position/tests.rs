//! Behaviour regressions spanning the priority chain and positioning.

use super::super::{think, Tick};
use super::{react_ok, ring_point, roll_clear, style_of};
use crate::ai::draw::{unit_unit, SALT_DODGE};
use crate::ai::state::{AiState, Behaviour, FightState, LostState};
pub(super) use crate::ai::tests::STYLE;
use crate::ai::{
    CIRCLE_STOP, DODGE_COOLDOWN, HOLD_STRAFE, LOST_RECYCLE, SIDESTEP_TICKS, THINK_PERIOD,
};
use crate::nav;
use crate::sim::Sim;
use crate::unit::UnitHandle;
use glam::Vec3;

use crate::ai::tests::{all_circle, all_stand, l_sim, Style};

/// Card FGHT (it was card HOLD's "at a hub"): a bot that sees an enemy
/// inside its weapon's reach takes a position and fights from it — it
/// stops walking its route — and once the enemy is gone and the position
/// has been quiet for `HOLD_QUIET` it walks on.
#[test]
fn a_bot_fights_from_a_position_while_it_sees_an_enemy_and_then_advances() {
    // A stander: it keeps its place.
    let _style = all_stand();
    use crate::sim::PadState;
    use crate::unit::UnitSpec;
    let (mut sim, h) = l_sim();
    // A dummy 75 m up the first leg: out of sight at the start (49 m),
    // in sight after some 26 m of it.
    let e = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -75.0)))
        .expect("enemy");
    let mut held_at = None;
    for _ in 0..600 {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        let u = sim.unit(h).expect("alive");
        if u.ai.hold.is_some_and(|x| sim.tick < x.until) {
            held_at = Some(u.soldier.pos);
            break;
        }
    }
    let p0 = held_at.expect("it took a position");
    let seen_from = nav::xz_dist(p0, Vec3::new(0.0, 0.0, -75.0));
    assert!(
        (40.0..50.0).contains(&seen_from),
        "on sight, inside the weapon's reach: {seen_from} m from the enemy"
    );
    // Eight seconds on it is still there.
    for _ in 0..240 {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
    }
    let u = sim.unit(h).expect("alive");
    assert!(
        u.ai.hold.is_some_and(|x| sim.tick < x.until),
        "still on its position while the enemy stands there"
    );
    assert_eq!(u.ai.behaviour, Behaviour::Fight);
    let held = u.soldier.pos;
    assert!(
        nav::xz_dist(held, p0) < crate::ai::HOLD_SHUFFLE + HOLD_STRAFE + 1.0,
        "it has not walked on: {held} from {p0}"
    );
    // The enemy goes; the position goes quiet; the bot advances to the post.
    sim.units[e.index as usize].alive = false;
    for _ in 0..900 {
        sim.step(&[PadState::default()]);
    }
    let u = sim.unit(h).expect("alive");
    assert!(
        nav::xz_dist(u.soldier.pos, Vec3::new(40.0, 0.0, -40.0)) < 8.0,
        "it went on to the post: {}",
        u.soldier.pos
    );
}

/// Flat ground, no graph: `n` AI troopers of team 0 a metre and a half
/// apart at the origin, facing an enemy dummy `d` metres up -Z.
fn fight_scene(n: usize, d: f32) -> (Sim, Vec<UnitHandle>, UnitHandle) {
    use crate::heightfield::Heightfield;
    use crate::unit::{Controller, UnitSpec};
    let mut sim = Sim::empty(11);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let bots = (0..n)
        .map(|i| {
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(1.5 * i as f32, 0.0, 0.0))
            })
            .expect("bot")
        })
        .collect();
    let e = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -d)))
        .expect("enemy");
    (sim, bots, e)
}

/// Card REACT: a stander in a fight, HIT once at tick `at` (Normal's
/// sight; the tier is `d`). What it did about it: 0 stayed, 1 strafed
/// (its goal 2.5 m aside, its spot kept), 2 rolled (its spot the roll's
/// 10 m aside), 3 moved (on its way to a ring point, or a spot 3.5-7 m
/// off) — and the tick its next reaction may come.
fn hit_once(d: crate::skill::Difficulty, at: u64) -> (usize, u64) {
    use crate::sim::PadState;
    // The same scene twice: without the hit, and with it.
    let run = |hit: bool| {
        let (mut sim, bots, e) = fight_scene(1, 30.0);
        sim.difficulty = d;
        while sim.tick < at {
            sim.step(&[PadState::default()]);
            sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        }
        let pos = sim.unit(bots[0]).expect("alive").soldier.pos;
        if hit {
            sim.unit_mut(bots[0]).expect("alive").last_hit = Some((e, at - 1));
        }
        sim.step(&[PadState::default()]);
        (sim.unit(bots[0]).expect("alive").ai, pos)
    };
    let ((calm, _), (st, pos)) = (run(false), run(true));
    assert_eq!(calm.fight.mode, FightState::STAND, "it stands to shoot");
    let (h0, h) = (calm.hold.expect("a position"), st.hold.expect("still"));
    let (spot, goal) = (nav::xz_dist(h.spot, pos), nav::xz_dist(h.goal, pos));
    let what = match () {
        _ if st.fight.mode == FightState::MOVE => 3,
        _ if h.roll_at != 0 => {
            assert_eq!(h.roll_at, at + 6, "the roll's lead");
            assert!((spot - 10.0).abs() < 0.01 && spot == goal, "{spot} {goal}");
            2
        }
        _ if h.spot != h0.spot => {
            assert!(
                (3.49..=7.01).contains(&spot) && spot == goal,
                "{spot} {goal}"
            );
            3
        }
        _ if h.goal != h0.goal => {
            assert!((goal - 2.5).abs() < 0.01, "a strafe of {goal} m");
            1
        }
        _ => {
            // (Card HYST: the hit wakes the bot, whose early think saw
            // its enemy again.)
            let h = crate::ai::HoldState {
                last_contact: h0.last_contact,
                ..h
            };
            assert_eq!((h, st.fight), (h0, calm.fight), "an unnamed reaction");
            0
        }
    };
    (what, st.dodge_until)
}

/// Card REACT (the maintainer, 2026-10-07): a bot that stands to shoot answers a
/// hit by its tier's row — it stays, strafes, rolls or moves in the
/// row's shares (400 hits a tier, each within 7 points of the row, as
/// numbers: the row's own constants would follow a broken table), most
/// at Easy and Normal stay and fewest at Elite, and every hit arms the 1.5 s
/// cooldown, a stay too.
#[test]
fn a_hit_stander_stays_strafes_rolls_or_moves_by_its_tiers_row() {
    use crate::skill::Difficulty as D;
    let _style = all_stand();
    let rows = [
        (D::Easy, [0.35, 0.30, 0.15, 0.20]),
        (D::Normal, [0.35, 0.30, 0.15, 0.20]),
        (D::Hard, [0.20, 0.30, 0.20, 0.30]),
        (D::Elite, [0.10, 0.30, 0.25, 0.35]),
    ];
    let mut stays = Vec::new();
    for (d, row) in rows {
        let mut n = [0u32; 4];
        for k in 0..400u64 {
            let at = 90 + k;
            let (what, until) = hit_once(d, at);
            n[what] += 1;
            assert_eq!(until, at + 45, "{d:?}: the cooldown after a {what}");
        }
        for (i, want) in row.iter().enumerate() {
            let got = n[i] as f32 / 400.0;
            assert!(
                (got - want).abs() < 0.07,
                "{d:?}: reaction {i} in {got} of the hits, the row says {want} ({n:?})"
            );
        }
        stays.push(n[0]);
    }
    // Easy and Normal share a row; from Normal up fewer stay.
    assert!(
        stays[1..].windows(2).all(|w| w[0] > w[1]),
        "stays {stays:?}"
    );
}

/// Card REACT: a burst is ONE draw — a second hit inside the cooldown
/// changes nothing; and a stander under steady fire for 20 s (a hit
/// every third of a second, Normal) does not stand still for long
/// (the maintainer: "it just stays standing there"), keeps its fight, and when it
/// was sent to a new position stands to shoot again once it is there.
#[test]
fn steady_fire_moves_a_stander_about_and_it_fights_on() {
    use crate::sim::PadState;
    let _style = all_stand();
    let (mut sim, bots, e) = fight_scene(1, 30.0);
    let b = bots[0];
    for _ in 0..90 {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
    }
    let (mut last, mut path) = (sim.unit(b).expect("alive").soldier.pos, 0.0f32);
    let (mut still, mut longest, mut walked, mut again) = (0u32, 0u32, 0u32, 0u32);
    for k in 0..600u64 {
        let before = sim.unit(b).expect("alive").ai;
        // (Inside the cooldown every tick: the answer must not change.)
        let cool = sim.tick < before.dodge_until;
        if k % 10 == 0 || cool {
            sim.unit_mut(b).expect("alive").last_hit = Some((e, sim.tick - 1));
        }
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        sim.unit_mut(b).expect("alive").health.cur = 300.0;
        let u = sim.unit(b).expect("alive");
        if cool {
            assert_eq!(u.ai.dodge_until, before.dodge_until, "re-armed");
            again += u32::from(
                u.ai.hold.map(|h| h.spot) != before.hold.map(|h| h.spot)
                    && u.ai.fight.mode == before.fight.mode
                    && u.ai.fight.mode == FightState::STAND,
            );
        }
        walked += u32::from(u.ai.fight.mode == FightState::MOVE);
        assert_eq!(u.ai.behaviour, Behaviour::Fight, "tick {}", sim.tick);
        let step = nav::xz_dist(u.soldier.pos, last);
        (last, path) = (u.soldier.pos, path + step);
        // (Standing: under 0.5 m/s.)
        still = if step < 0.5 / 30.0 { still + 1 } else { 0 };
        longest = longest.max(still);
    }
    assert_eq!(again, 0, "a stander's spot moved inside the cooldown");
    // (Card TPOS: 40 m with the shuffle; without it 39.3.)
    assert!(path > 30.0, "{path} m in 20 s of fire");
    assert!(longest < 200, "{longest} ticks standing still under fire");
    assert!(walked > 0, "it never walked to a new position");
    // The fire stops: it ends its walk and stands to shoot.
    for _ in 0..450 {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
    }
    let u = sim.unit(b).expect("alive");
    assert_eq!(u.ai.fight.mode, FightState::STAND);
    assert!(nav::xz_dist(u.soldier.pos, u.ai.hold.expect("a position").goal) < 1.0);
}

/// Card REACT: the roll CARRIES the bot — pressed once it is under way
/// (a soldier standing still rolls on the spot), it is in the roll a
/// third of a second after the hit and some 10 m aside when it stands
/// again, on its new spot.
#[test]
fn a_hit_bots_roll_carries_it_aside() {
    use crate::sim::PadState;
    use crate::soldier::Stance;
    let _style = all_stand();
    let d = crate::skill::Difficulty::Normal;
    let at = (90..200)
        .find(|at| hit_once(d, *at).0 == 2)
        .expect("a roll");
    let (mut sim, bots, e) = fight_scene(1, 30.0);
    sim.difficulty = d;
    while sim.tick < at {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
    }
    let from = sim.unit(bots[0]).expect("alive").soldier.pos;
    sim.unit_mut(bots[0]).expect("alive").last_hit = Some((e, at - 1));
    let mut rolling = 0;
    for k in 0..60 {
        sim.step(&[PadState::default()]);
        let u = sim.unit(bots[0]).expect("alive");
        rolling += u32::from(u.soldier.stance == Stance::Roll);
        if k == 10 {
            assert_eq!(u.soldier.stance, Stance::Roll, "not rolling yet");
        }
    }
    let u = sim.unit(bots[0]).expect("alive");
    let gone = nav::xz_dist(u.soldier.pos, from);
    assert!(rolling > 20, "{rolling} ticks in the roll");
    assert!(gone > 8.0 && gone < 12.0, "{gone} m aside after the roll");
    assert!(nav::xz_dist(u.soldier.pos, u.ai.hold.expect("a position").spot) < 2.5);
}

/// Card REACT: no step, roll or move of a bot on a position goes into
/// or through a mission death region.
#[test]
fn a_reaction_never_goes_through_a_death_region() {
    use crate::region::Region;
    let (mut sim, bots, _) = fight_scene(1, 30.0);
    let p = Vec3::new(2.5, 0.0, 0.0);
    let ok = |sim: &Sim, p: Vec3| react_ok(sim, sim.unit(bots[0]).expect("alive"), p);
    assert!(ok(&sim, p) && ok(&sim, p * 4.0));
    sim.death_regions = vec![Region::sphere(Vec3::new(5.0, 0.0, 0.0), 1.0)];
    assert!(ok(&sim, p), "short of it");
    assert!(!ok(&sim, p * 2.0), "into it");
    assert!(!ok(&sim, p * 4.0), "through it");
}

/// Card FGHT (`analysis/re-soldier-ai.md` 4.5): a bot whose style is to
/// STAND keeps its place while it fights; one whose style is to CIRCLE
/// walks to points of a ring round its target and pauses on them — and
/// never ends up farther from the target than it began (OURS: the ring is
/// no wider than its own distance).
#[test]
fn a_stander_keeps_its_place_and_a_circler_walks_a_ring_round_its_target() {
    use crate::sim::PadState;
    let run = |style: Style| {
        let (mut sim, bots, e) = fight_scene(1, 30.0);
        let (mut path, mut last, mut far, mut modes) = (0.0f32, Vec3::ZERO, 0.0f32, [0u32; 4]);
        let (mut fights, mut strayed) = (0u32, 0.0f32);
        for _ in 0..600 {
            sim.step(&[PadState::default()]);
            sim.unit_mut(e).expect("enemy").health.cur = 300.0;
            let u = sim.unit(bots[0]).expect("alive");
            path += nav::xz_dist(u.soldier.pos, last);
            last = u.soldier.pos;
            far = far.max(nav::xz_dist(last, Vec3::new(0.0, 0.0, -30.0)));
            strayed = strayed.max(nav::xz_dist(last, Vec3::ZERO));
            modes[usize::from(u.ai.fight.mode)] += 1;
            fights += u32::from(u.ai.behaviour == Behaviour::Fight);
        }
        drop(style);
        assert!(fights > 590, "it fights throughout: {fights} of 600 ticks");
        (path, far, modes, strayed)
    };
    // (Its steps on the position are card HOLD's shuffle and strafe.)
    let (_, _, modes, strayed) = run(all_stand());
    assert!(
        strayed <= crate::ai::HOLD_SHUFFLE + HOLD_STRAFE + 0.5,
        "a stander keeps its place: {strayed} m from it"
    );
    assert!(modes[usize::from(FightState::STAND)] > 590, "{modes:?}");
    let (path, far, modes, _) = run(all_circle());
    assert!(path > 20.0, "a circler walks: {path} m in 20 s");
    assert!(
        far < 31.5,
        "never farther from its target than it began: {far} m"
    );
    assert!(
        modes[usize::from(FightState::WALK)] > 100 && modes[usize::from(FightState::PAUSE)] > 50,
        "it walks and it pauses: {modes:?}"
    );
}

/// Card FGHT, OURS (the maintainer, 2026-10-05: "everyone stops in their tracks
/// which puts them all right next to each other"): bots that stand to
/// shoot do not stand side by side — the first keeps its place, every
/// next one takes a spot [`FIGHT_SPACING`] clear of the others'.
#[test]
fn standers_do_not_stop_beside_each_other() {
    use crate::sim::PadState;
    let _style = all_stand();
    // The enemy steps into sight a second in, so each bot sees it on its
    // own think (a fight starts on a think; the thinks are a tick apart).
    let (mut sim, bots, e) = fight_scene(4, 300.0);
    // A second after that every position is taken, and no shuffle has run.
    for t in 0..60 {
        sim.step(&[PadState::default()]);
        if t == 30 {
            sim.unit_mut(e).expect("enemy").soldier.pos = Vec3::new(0.0, 0.0, -30.0);
        }
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        for b in &bots {
            sim.unit_mut(*b).expect("bot").health.cur = 300.0;
        }
    }
    let spots: Vec<Vec3> = bots
        .iter()
        .map(|b| {
            let u = sim.unit(*b).expect("alive");
            assert_eq!(u.ai.fight.mode, FightState::STAND);
            u.ai.hold.expect("a position").spot
        })
        .collect();
    assert_eq!(spots[0], Vec3::ZERO, "the first keeps its place");
    for (i, a) in spots.iter().enumerate() {
        for b in &spots[i + 1..] {
            // (4 m is `FIGHT_SPACING`, as a number: the rule's own
            // constant would follow a broken rule.)
            assert!(
                nav::xz_dist(*a, *b) >= 3.99,
                "two standers' spots {:.1} m apart: {spots:?}",
                nav::xz_dist(*a, *b)
            );
        }
    }
}

/// Card FGHT: a ring point lies in the PS2's band round the weapon's
/// optimal range (`FUN_002d9228`) from a far target, and no farther from
/// a near target than the bot itself is.
#[test]
fn a_ring_point_is_on_the_weapons_ring_and_no_wider_than_the_bots_distance() {
    for (d, lo, hi) in [(45.0f32, 24.0f32, 36.0f32), (10.0, 0.0, 10.0)] {
        let (mut sim, bots, _) = fight_scene(1, d);
        for tick in [1u64, 40, 400, 4000, 7, 77, 777, 7777] {
            sim.tick = tick;
            let class = sim.unit(bots[0]).expect("alive").weapon_class;
            // The class defaults: a ring of 24 to 36 m.
            sim.weapon_classes[class].ai_range = [0.0, 30.0, 60.0];
            let u = sim.unit(bots[0]).expect("alive");
            let wc = &sim.weapon_classes[u.weapon_class];
            let tk = Tick {
                me: bots[0],
                u,
                eye: u.muzzle(),
                wc,
                tier: sim.difficulty.tier(),
                skill: sim.difficulty.skill_at(u.skill),
            };
            let target = Vec3::new(0.0, 0.0, -d);
            let p = ring_point(&sim, &tk, target).expect("open ground: a point");
            let r = nav::xz_dist(p, target);
            // (The half-way try is only for a point that cannot be had.)
            assert!(
                (lo - 0.01..=hi + 0.01).contains(&r),
                "target at {d} m: a ring point {r} m from it, band {lo}..{hi}"
            );
        }
    }
}

/// The fight modes of bot 0 over 20 s ([none, stand, walk, pause] ticks)
/// with its target `d` metres off and its weapon's AI ranges `ai`
/// (`scoped`: a single-shot zoom weapon, the AI's sniper table).
fn modes_at(d: f32, ai: [f32; 3], scoped: bool) -> [u32; 4] {
    use crate::sim::PadState;
    let (mut sim, bots, e) = fight_scene(1, d);
    let class = sim.unit(bots[0]).expect("alive").weapon_class;
    let c = &mut sim.weapon_classes[class];
    c.ai_range = ai;
    if scoped {
        (c.zoom_max, c.trigger_single, c.yaw_spread) = (8.0, true, 0.0);
    }
    assert_eq!(c.ai_distance_table(), scoped);
    let mut modes = [0u32; 4];
    for _ in 0..600 {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        sim.unit_mut(bots[0]).expect("bot").health.cur = 300.0;
        modes[usize::from(sim.unit(bots[0]).expect("alive").ai.fight.mode)] += 1;
    }
    modes
}

/// Card FGHTA (`analysis/re-soldier-ai.md` 4.4): a bot whose style is to
/// stand stands only while its target is inside the middle of its
/// weapon's band — for [0, 24, 64] between 12 and 44 m; nearer or farther
/// it takes ring points as a circler does. The style is forced, the band
/// is the rule's own.
#[test]
fn a_stander_circles_outside_the_middle_of_its_weapons_band() {
    let _style = crate::ai::tests::all_standers();
    let stand = usize::from(FightState::STAND);
    // (A standing soldier is seen from 49 m: the far case is at 47.)
    for (d, stands) in [(10.0f32, false), (30.0, true), (47.0, false)] {
        let modes = modes_at(d, [0.0, 24.0, 64.0], false);
        if stands {
            assert!(modes[stand] >= 590, "at {d} m it stands: {modes:?}");
        } else if d < 12.0 {
            // (Its ring is no wider than its own distance: never in.)
            assert_eq!(modes[stand], 0, "at {d} m it circles: {modes:?}");
        } else {
            // It walks in to a ring point, and stands once in the band.
            let walk = modes[usize::from(FightState::WALK)];
            assert!(walk >= 10, "at {d} m it walks to a ring point: {modes:?}");
        }
    }
}

/// Card FGHTA: a scoped rifle stands wherever its target is in range —
/// the PS2's scout rule (`FUN_002abc38`), by the bot's own draw. It was a
/// circler inside (MinRange + OptimalRange) / 2 = 36 m: on its spot for 0
/// of 600 ticks at 12, 20 and 30 m. An unscoped weapon of the same ranges
/// still follows the band; out of range the scoped one circles (OURS).
#[test]
fn a_scoped_rifle_stands_wherever_its_target_is_in_range() {
    let stand = usize::from(FightState::STAND);
    for d in [12.0f32, 20.0, 30.0, 45.0] {
        let modes = modes_at(d, [8.0, 64.0, 128.0], true);
        assert!(modes[stand] >= 590, "scoped, at {d} m: {modes:?}");
    }
    let modes = modes_at(5.0, [8.0, 64.0, 128.0], true);
    assert_eq!(modes[stand], 0, "scoped, inside MinRange: {modes:?}");
    let _style = crate::ai::tests::all_standers();
    let modes = modes_at(20.0, [8.0, 64.0, 128.0], false);
    assert_eq!(modes[stand], 0, "unscoped, under the band: {modes:?}");
}

/// Card FGHTA: the style's draw — half the bots stand at every tier, a
/// third of them would crouch, and a scoped weapon always stands. Over 200
/// unit indices; the numbers are the PS2's weights
/// (`analysis/re-soldier-ai.md` 4.4), not the rule's constants.
#[test]
fn half_the_bots_stand_and_a_third_crouch() {
    let share = |k: usize| {
        let n = (0..200u32)
            .filter(|&index| {
                let s = style_of(UnitHandle { index, gen: 1 }, false);
                [s.0, s.1][k]
            })
            .count();
        n as f32 / 200.0
    };
    let s = share(0);
    assert!((0.4..=0.6).contains(&s), "{s} stand");
    let c = share(1);
    assert!((0.233..=0.433).contains(&c), "{c} crouch");
    assert!((0..200u32).all(|index| style_of(UnitHandle { index, gen: 3 }, true).0));
}

/// Card FGHTA: a stander whose draw says so crouches on its spot, and
/// one whose draw does not stands upright.
#[test]
fn a_stander_that_draws_the_crouch_crouches() {
    use crate::sim::PadState;
    for crouches in [false, true] {
        // The first bot index whose draw is a stander of this kind.
        let n = (0..200u32)
            .find(|&index| style_of(UnitHandle { index, gen: 1 }, false) == (true, crouches))
            .expect("one in 200") as usize
            + 1;
        let (mut sim, bots, e) = fight_scene(n, 30.0);
        let me = *bots.last().expect("a bot");
        assert_eq!((me.index as usize, me.gen), (n - 1, 1));
        for _ in 0..90 {
            sim.step(&[PadState::default()]);
            sim.unit_mut(e).expect("enemy").health.cur = 300.0;
            for b in &bots {
                sim.unit_mut(*b).expect("bot").health.cur = 300.0;
            }
        }
        let u = sim.unit(me).expect("alive");
        assert_eq!(u.ai.fight.mode, FightState::STAND);
        let mask = u.ai.hold.expect("a position").stance_mask;
        assert_eq!(
            mask == crate::commander::MASK_CROUCH,
            crouches,
            "bot {n}: mask {mask}"
        );
    }
}

/// Card FGHTA (`analysis/re-soldier-ai.md` 4.5): a circler's pause lasts
/// 1.9 s (57 ticks; the next ring point is drawn on the first think after
/// it), and on its way it stops for one at the PS2's rate — 1 in 12 per
/// 0.4 s think, 0.21 a second: about 1 in 14 of the port's thinks, not 1
/// in 6 and not never.
#[test]
fn a_circler_pauses_57_ticks_and_stops_on_its_way_at_the_ps2s_rate() {
    use crate::sim::PadState;
    let _style = all_circle();
    let rate = CIRCLE_STOP * crate::TICK_HZ as f32 / THINK_PERIOD as f32;
    assert!((0.203..=0.214).contains(&rate), "{rate} stops a second");
    let (mut walk_ticks, mut stops, mut pauses) = (0u32, 0u32, Vec::new());
    for n in 1..=6usize {
        // A wide ring (32 to 45 m round a target 45 m off): long walks.
        let (mut sim, bots, e) = fight_scene(n, 45.0);
        let me = *bots.last().expect("a bot");
        let class = sim.unit(me).expect("alive").weapon_class;
        sim.weapon_classes[class].ai_range = [0.0, 40.0, 80.0];
        let (mut run, mut was, mut goal) = (0u64, 0u8, Vec3::ZERO);
        for _ in 0..1800 {
            sim.step(&[PadState::default()]);
            sim.unit_mut(e).expect("enemy").health.cur = 300.0;
            for b in &bots {
                sim.unit_mut(*b).expect("bot").health.cur = 300.0;
            }
            let u = sim.unit(me).expect("alive");
            let mode = u.ai.fight.mode;
            walk_ticks += u32::from(mode == FightState::WALK);
            if mode == FightState::PAUSE {
                run += 1;
                // A pause taken short of the ring point: the chance's.
                stops +=
                    u32::from(was == FightState::WALK && nav::xz_dist(u.soldier.pos, goal) > 4.0);
            } else if run > 0 {
                pauses.push(run);
                run = 0;
            }
            was = mode;
            goal = u.ai.hold.map_or(goal, |h| h.goal);
        }
    }
    assert!(pauses.len() > 20, "pauses: {pauses:?}");
    // (A pause after which no ring point can be had is taken again.)
    let single = pauses.iter().filter(|p| **p < 57 + THINK_PERIOD).count();
    assert!(
        pauses.iter().all(|p| *p >= 57) && 2 * single > pauses.len(),
        "a pause is 57 ticks and ends on the next think: {pauses:?}"
    );
    // 1 in 14.4 thinks; 1 in 6 is 0.167.
    let per_think = stops as f32 * THINK_PERIOD as f32 / walk_ticks as f32;
    assert!(
        (0.04..=0.105).contains(&per_think),
        "{stops} stops in {walk_ticks} walking ticks: {per_think} a think"
    );
}

/// Card FGHTA: a seen target is fought to the weapon's MaxRange + 10 m
/// (`analysis/re-soldier-ai.md` 4.2) — at + 5 m it is, at + 15 m not.
#[test]
fn a_target_is_fought_to_ten_metres_past_the_weapons_range() {
    let _style = all_stand();
    let modes = modes_at(35.0, [0.0, 15.0, 30.0], false);
    assert!(
        modes[usize::from(FightState::STAND)] >= 590,
        "at MaxRange + 5: {modes:?}"
    );
    let modes = modes_at(45.0, [0.0, 15.0, 30.0], false);
    assert_eq!(modes[0], 600, "at MaxRange + 15: {modes:?}");
}

/// Card TEAM: a circler whose target is past the trigger's reach and
/// that finds no ring point (water over the ground refuses every one)
/// walks at its target until it is in reach and fires; it no longer
/// pauses where it stands for good (BF1 Kashyyyk at Hard, seed 2: four
/// last soldiers 57 m apart, the round never ended).
#[test]
fn a_circler_with_no_ring_point_walks_at_a_target_out_of_reach() {
    use crate::sim::{Event, PadState};
    let _style = all_circle();
    let (mut sim, bots, e) = fight_scene(1, 0.0);
    let reach = {
        let u = sim.unit(bots[0]).expect("bot");
        sim.weapon_classes[u.weapon_class].ai_range[2]
    };
    let start = reach + 5.0;
    sim.unit_mut(e).expect("enemy").soldier.pos = Vec3::new(0.0, 0.0, -start);
    sim.water_level = Some(1.0);
    // (Hard's sight, 61 m standing, sees it - Normal's 49 m does not.)
    sim.difficulty = crate::skill::Difficulty::Hard;
    let (mut near, mut shots) = (start, 0u32);
    for _ in 0..450 {
        sim.step(&[PadState::default()]);
        sim.unit_mut(e).expect("enemy").health.cur = 300.0;
        let u = sim.unit(bots[0]).expect("alive");
        near = near.min(nav::xz_dist(u.soldier.pos, Vec3::new(0.0, 0.0, -start)));
        shots += sim
            .events
            .iter()
            .filter(|v| matches!(v, Event::Shot { unit, .. } if *unit == bots[0]))
            .count() as u32;
    }
    assert!(
        near < reach,
        "it walked into reach: {near} m (reach {reach})"
    );
    assert!(shots > 0, "and fired");
}

/// Issue #70: the 6 cm water bob is what bolts and soldiers meet. A bot's
/// ring-point rule is the port's own and reads the layer height, so ground
/// sitting right at its 0.2 m limit is not taken and refused with the phase.
#[test]
fn the_water_bob_does_not_move_a_bots_ring_points() {
    let points = |oscillates: bool| {
        let (mut sim, bots, _) = fight_scene(1, 0.0);
        let h = bots[0];
        let class = sim.unit(h).expect("bot").weapon_class;
        sim.weapon_classes[class].ai_range = [0.0, 30.0, 60.0];
        sim.water_level = Some(0.2);
        sim.water_oscillates = oscillates;
        (1..=64u64)
            .map(|tick| {
                sim.tick = 17 * tick;
                let u = sim.unit(h).expect("alive");
                let tk = Tick {
                    me: h,
                    u,
                    eye: u.muzzle(),
                    wc: &sim.weapon_classes[u.weapon_class],
                    tier: sim.difficulty.tier(),
                    skill: sim.difficulty.skill_at(u.skill),
                };
                ring_point(&sim, &tk, Vec3::new(0.0, 0.0, -45.0))
            })
            .collect::<Vec<Option<Vec3>>>()
    };
    let still = points(false);
    assert!(
        still.iter().flatten().count() >= 8,
        "ring points at the limit: {still:?}"
    );
    assert_eq!(points(true), still);
}

/// Card FGHTA: with a nav graph a ring point is taken INSIDE a hub — the
/// one nearest the drawn point, within 10 m of it (`RING_SNAP`) — and
/// never in a hub that lies more than 2 m farther from the target than
/// the bot stands (`RING_BACK`). The numbers are written out: the rule's
/// constants would follow a broken rule.
#[test]
fn a_ring_point_on_a_graph_lies_in_a_hub_and_never_farther_back() {
    use crate::sim::PadState;
    // The L graph: hubs (0, 0), (0, -40) and (40, -40), 2 m wide.
    let hubs = [
        Vec3::ZERO,
        Vec3::new(0.0, 0.0, -40.0),
        Vec3::new(40.0, 0.0, -40.0),
    ];
    let points = |at: Vec3, target: Vec3| {
        let (mut sim, h) = l_sim();
        sim.step(&[PadState::default()]);
        sim.unit_mut(h).expect("bot").soldier.pos = at;
        let class = sim.unit(h).expect("alive").weapon_class;
        sim.weapon_classes[class].ai_range = [0.0, 30.0, 60.0];
        (1..=64u64)
            .filter_map(|tick| {
                sim.tick = 17 * tick;
                let u = sim.unit(h).expect("alive");
                let tk = Tick {
                    me: h,
                    u,
                    eye: u.muzzle(),
                    wc: &sim.weapon_classes[u.weapon_class],
                    tier: sim.difficulty.tier(),
                    skill: sim.difficulty.skill_at(u.skill),
                };
                ring_point(&sim, &tk, target)
            })
            .collect::<Vec<Vec3>>()
    };
    // From hub 0, a target 44.7 m off between hubs 1 and 2 (20 m from
    // each): its 24-36 m ring passes within 10 m of both.
    let got = points(Vec3::ZERO, Vec3::new(20.0, 0.0, -40.0));
    assert!(got.len() >= 8, "ring points on the graph: {}", got.len());
    for p in &got {
        let near = hubs
            .iter()
            .map(|h| nav::xz_dist(*p, *h))
            .fold(f32::MAX, f32::min);
        assert!(
            near <= 2.01,
            "a ring point {near} m from the nearest hub: {p}"
        );
    }
    // On the first leg 20 m from hub 0, a target 8 m off toward it: the
    // ring is 8 m wide, the only hub within 10 m of it is hub 0, 12 m
    // from the target — every point of it is over 2 m farther back.
    let got = points(Vec3::new(0.0, 0.0, -20.0), Vec3::new(0.0, 0.0, -12.0));
    assert!(got.is_empty(), "a circle does not back away: {got:?}");
}

/// Card HOLD: a grenade on the ground sends an Elite bot inside its
/// warning radius clear of it; a bot outside the radius does not stir.
#[test]
fn a_bot_runs_clear_of_a_grenade_on_the_ground() {
    use crate::sim::{PadState, HAZARD_RADIUS};
    use crate::skill::Difficulty;
    let run = |at: Vec3| {
        let (mut sim, h) = l_sim();
        sim.difficulty = Difficulty::Elite;
        // No objective to walk to: the only thing that moves it is the grenade.
        sim.cps.clear();
        sim.nav = None;
        sim.unit_mut(h).expect("alive").skill = Difficulty::Elite.base_skill();
        sim.add_hazard(at, 150);
        for _ in 0..120 {
            sim.step(&[PadState::default()]);
        }
        nav::xz_dist(sim.unit(h).expect("alive").soldier.pos, at)
    };
    let near = run(Vec3::new(2.0, 0.0, 1.0));
    assert!(near > HAZARD_RADIUS, "ran clear: {near:.1} m from it");
    let far = run(Vec3::new(12.0, 0.0, 0.0));
    assert!(
        (far - 12.0).abs() < 0.5,
        "outside the radius it stays: {far:.1}"
    );
}

/// Card HOLD: the dodge only rolls where there is floor to roll over.
#[test]
fn a_roll_needs_clear_floor() {
    use crate::collision::{Body, CollisionWorld, Shape};
    let (mut sim, h) = l_sim();
    let u = sim.unit(h).expect("alive");
    assert!(
        roll_clear(&sim, u, 1) && roll_clear(&sim, u, -1),
        "open ground"
    );
    // A wall 4 m to the right (+X of a unit facing -Z).
    sim.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::from_translation(Vec3::new(4.0, 0.0, 0.0)),
            Shape::Box {
                half: Vec3::new(0.5, 6.0, 30.0),
            },
        )],
        Vec::new(),
    );
    let u = sim.unit(h).expect("alive");
    assert!(!roll_clear(&sim, u, 1), "a wall in the way");
    assert!(roll_clear(&sim, u, -1), "the other side is still open");
}

/// Card BNDS: a bot on a hull, 4 m under its deck past the rim barrier,
/// is OFF the level; two thinks confirm it, it walks back at the nearest
/// hub and climbs to the deck, and the state is gone once it is on it.
#[test]
fn a_bot_off_the_level_walks_back_onto_it() {
    use crate::ai::tests::rim_sim;
    use crate::sim::{Level, PadState};
    let (mut sim, h) = rim_sim(true);
    let y = sim.ground_under(Vec3::new(-18.0, 12.0, 0.0), false);
    sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-18.0, y, 0.0);
    assert!(sim.level_at(Vec3::new(-18.0, y, 0.0)).is_off());
    let mut was_lost = false;
    for _ in 0..600 {
        sim.step(&[PadState::default()]);
        let u = sim
            .unit(h)
            .expect("alive: no recycle on a hull it can leave");
        was_lost |= u.ai.lost.is_some_and(|l| l.confirmed);
    }
    let u = sim.unit(h).expect("alive");
    assert!(was_lost, "the return walk was on");
    assert_eq!(sim.level_at(u.soldier.pos), Level::On, "{}", u.soldier.pos);
    assert!(u.soldier.pos.y > 9.5, "back on the deck: {}", u.soldier.pos);
    assert_eq!(u.ai.lost, None);
}

/// Card BOWL: the same on the FIRST metres of the hull — 3 m past the
/// rail and 1.5 m under the deck, where the level test used to say ON
/// and a bot was never lost. Two thinks confirm it and it walks back.
#[test]
fn a_bot_just_past_the_rail_is_lost_and_walks_back() {
    use crate::ai::tests::rim_sim;
    use crate::sim::{Level, PadState};
    let (mut sim, h) = rim_sim(true);
    let y = sim.ground_under(Vec3::new(-13.0, 12.0, 0.0), false);
    assert!(y < 9.0 && y > 8.0, "on the hull: {y}");
    sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-13.0, y, 0.0);
    assert!(sim.level_at(Vec3::new(-13.0, y, 0.0)).is_off());
    // Until it is back on the level (what it does at the rim afterwards
    // is not this test's — card JARC; the walk along the rim after the
    // return steps off it again under the old rules too, and where the
    // 300th tick fell in that cycle was luck).
    let mut lost_at = None;
    for t in 0..300 {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        if lost_at.is_none() && u.ai.lost.is_some_and(|l| l.confirmed) {
            lost_at = Some(t);
        }
        if lost_at.is_some() && u.ai.lost.is_none() {
            break;
        }
    }
    assert!(
        lost_at.is_some_and(|t| t <= 2 * THINK_PERIOD),
        "lost within two thinks: {lost_at:?}"
    );
    let u = sim.unit(h).expect("alive");
    assert_eq!(sim.level_at(u.soldier.pos), Level::On, "{}", u.soldier.pos);
    assert!(u.soldier.pos.y > 9.2, "back at the rim: {}", u.soldier.pos);
    assert_eq!(u.ai.lost, None);
}

/// Card AIRC: a rail across the hull, its top 0.9 m over the hull under
/// it, shuts the lost bot's way back with a low wall. It jumps from where
/// it stands with the stick held at the wall — the air steers it over —
/// and never backs off for a run-up.
#[test]
fn a_lost_bot_jumps_a_low_wall_from_where_it_stands() {
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        check_lost_bot_jump(rules);
    }
}

fn check_lost_bot_jump(rules: crate::soldier::GroundRules) {
    use crate::ai::tests::rim_sim_with;
    use crate::collision::{Body, Shape};
    use crate::sim::{Level, PadState};
    let rail = Body::new(
        glam::Mat4::from_translation(Vec3::new(-12.4, 9.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.1, 0.5, 10.0),
        },
    );
    let (mut sim, h) = rim_sim_with(true, vec![rail]);
    sim.ground_rules = rules;
    // At rest against the rail's outer face (x -12.5, his radius 0.5).
    let y = sim.ground_under(Vec3::new(-13.05, 9.5, 0.0), false);
    assert!(y < 9.0 && y > 8.0, "on the hull: {y}");
    sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-13.05, y, 0.0);
    // Until it is back on the level (what it does on the deck afterwards
    // is not this test's: card JARC).
    let (mut reached, mut backed, mut jumped) = (f32::MIN, 0.0f32, false);
    for _ in 0..300 {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        let x = u.soldier.pos.x;
        jumped |= u.soldier.stance == crate::soldier::Stance::Jump;
        if x < -12.5 {
            reached = reached.max(x);
            backed = backed.max(reached - x);
        }
        if jumped && u.ai.lost.is_none() {
            break;
        }
    }
    let u = sim.unit(h).expect("alive");
    assert!(jumped, "it jumped");
    assert!(backed < 0.6, "no run-up: backed off {backed} m");
    assert_eq!(u.ai.lost, None);
    assert_eq!(sim.level_at(u.soldier.pos), Level::On, "{}", u.soldier.pos);
    // (Card RETW: inside the rail it may land on the hull's first
    // metre, under the rim, which is on the level — the return walk
    // aims at its floor walk's point, no longer at the hub's centre.)
    assert!(
        u.soldier.pos.x > -12.3 && u.soldier.pos.y > 9.2,
        "over the rail, back on the level: {}",
        u.soldier.pos
    );
}

/// Card RETW: a ground slab under a deck (top y 5, x and z within ±8)
/// whose hub 0 is pinned by a post on it, and hub 1 on the ground 20 m
/// east, pinned by a second post; a wall under the deck (x 3, z -20..4)
/// and a barrier at x 12.5 that keeps hub 1's walk from vouching for
/// the ground west of x 2.5. A bot under the deck there is OFF the level
/// (the deck is 5 m over it), and its way to hub 1 is round the wall's
/// end.
fn under_deck_sim(wall: bool, pad: bool) -> (Sim, UnitHandle) {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::cp::CpDef;
    use crate::nav::{Arc, Hub, NavGraph, RouteTable};
    use crate::region::Region;
    use crate::unit::{Controller, UnitSpec};
    use glam::Mat4;
    let slab = |c: Vec3, half: Vec3| Body::new(Mat4::from_translation(c), Shape::Box { half });
    let mut bodies = vec![
        slab(Vec3::new(0.0, -0.25, 0.0), Vec3::new(40.0, 0.25, 40.0)),
        slab(Vec3::new(0.0, 4.75, 0.0), Vec3::new(8.0, 0.25, 8.0)),
    ];
    if wall {
        bodies.push(slab(Vec3::new(3.0, 1.5, -8.0), Vec3::new(0.25, 1.5, 12.0)));
    }
    if pad {
        bodies.push(slab(Vec3::new(0.0, 0.75, 0.0), Vec3::new(1.5, 0.75, 1.5)));
    }
    let mut sim = Sim::empty(7);
    sim.collision = CollisionWorld::new(bodies, Vec::new());
    let hub = |x: f32, arcs: Vec<u16>| Hub {
        pos: Vec3::new(x, 2.0, 0.0),
        radius: 4.0,
        arcs,
    };
    let mut g = NavGraph::new(
        vec![hub(0.0, vec![0]), hub(20.0, vec![0])],
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
    .expect("graph");
    g.barriers.push(nav::Barrier {
        centre: glam::Vec2::new(12.5, 0.0),
        axes: [glam::Vec2::X, glam::Vec2::Y],
        half: glam::Vec2::new(0.5, 30.0),
        mask: 1,
        enabled: true,
    });
    sim.nav = Some(g);
    for (name, at) in [
        ("cp1", Vec3::new(0.0, 5.0, 0.0)),
        ("cp2", Vec3::new(20.0, 0.0, 0.0)),
    ] {
        sim.add_cp(
            CpDef {
                name: name.into(),
                pos: at,
                capture: Region::sphere(at, 3.0),
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
    }
    let h = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .expect("spawned");
    sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(0.0, if pad { 1.5 } else { 0.0 }, 0.0);
    (sim, h)
}

/// [`under_deck_sim`]'s bot walked until it is back on the level (at
/// most 300 ticks): the tick and where.
fn walk_back(sim: &mut Sim, h: UnitHandle) -> Option<(u32, Vec3)> {
    use crate::sim::PadState;
    for t in 0..300u32 {
        sim.step(&[PadState::default()]);
        let u = sim.unit(h).expect("alive");
        if u.ai.lost.is_none() && t > 2 * THINK_PERIOD as u32 {
            return Some((t, u.soldier.pos));
        }
    }
    None
}

/// Card RETW: a bot under a deck walks back to the level along the
/// FLOOR — round the wall's end to the hub on its own floor, inside the
/// first return window — where the straight line at the nearest hub
/// stood it under the deck it cannot reach.
#[test]
fn a_bot_under_a_deck_walks_round_a_corner_to_a_level_hub() {
    use crate::sim::Level;
    let (mut sim, h) = under_deck_sim(true, false);
    let feet = Vec3::new(0.0, 0.0, 0.0);
    assert_eq!(sim.level_at(feet), Level::Off(5.0), "under the deck");
    assert_eq!(
        sim.return_point(feet, 0).map(|p| p.x),
        Some(0.0),
        "the nearest hub is the deck's, overhead"
    );
    let path = sim.return_path(feet).expect("a floor walk");
    let pts = &path.pts[..usize::from(path.len)];
    assert!(
        pts.iter().any(|p| p.z > 4.0) && pts.iter().all(|p| p.y.abs() < 0.1),
        "round the wall's end, on the ground: {pts:?}"
    );
    let (t, at) = walk_back(&mut sim, h).expect("back on the level within 10 s");
    assert_eq!(sim.level_at(at), Level::On);
    assert!(
        at.z > 4.0 && at.x > 2.5 && at.y.abs() < 0.1,
        "round the wall's end, on the ground: {at} at tick {t}"
    );
    // From a pad 1.5 m up (BF2 Kamino's pod pad): the walk steps down
    // its kerb, which a lost bot's steering refuses anywhere else.
    let (mut sim, h) = under_deck_sim(true, true);
    assert!(
        sim.level_at(Vec3::new(0.0, 1.5, 0.0)).is_off(),
        "on the pad"
    );
    let (t, at) = walk_back(&mut sim, h).expect("down the kerb and back within 10 s");
    assert!(
        at.z > 4.0 && at.x > 2.5 && at.y.abs() < 0.1,
        "down the kerb, round the wall's end: {at} at tick {t}"
    );
    // The control: with no wall the same walk is a straight line.
    let (open, _) = under_deck_sim(false, false);
    let p = open.return_path(feet).expect("a floor walk");
    assert_eq!(
        p.len,
        1,
        "one straight walk: {:?}",
        &p.pts[..usize::from(p.len)]
    );
}

/// Card BNDS: a bot off the level for `LOST_RECYCLE` is given up — it
/// dies as lost, with its ticket, and respawns like any other death.
#[test]
fn a_bot_with_no_way_back_is_recycled() {
    use crate::ai::tests::rim_sim;
    use crate::sim::{DeathCause, Event, PadState};
    let (mut sim, h) = rim_sim(true);
    let y = sim.ground_under(Vec3::new(-18.0, 12.0, 0.0), false);
    let u = sim.unit_mut(h).expect("alive");
    u.soldier.pos = Vec3::new(-18.0, y, 0.0);
    // Off the level since long ago, as if every return had failed.
    u.ai.lost = Some(LostState {
        confirmed: true,
        ..LostState::default()
    });
    u.ai.next_think = u64::MAX;
    sim.tick = LOST_RECYCLE + 1;
    sim.step(&[PadState::default()]);
    assert!(sim.unit(h).is_none(), "given up");
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::EnvDeath {
            cause: DeathCause::Lost,
            ..
        }
    )));
}

/// D11's dodge-on-hit (RE §9.4: a movement re-plan, not a roll). A unit
/// with no route strafes; the coin is the skill's `lerp(0, 0.2)`; the
/// cooldown is 2 s.
#[test]
fn a_hit_unit_dodges_on_the_skill_coin_and_holds_the_cooldown() {
    use crate::sim::Sim;
    use crate::unit::{Controller, UnitSpec};
    let build = |skill: u8| {
        let mut s = Sim::empty(5);
        let h = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let other = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -8.0)))
            .unwrap();
        s.unit_mut(h).unwrap().skill = skill;
        (s, h, other)
    };
    // A tick whose coin passes at skill 20 (chance 0.2) and one whose
    // coin does not — the same unit, so only the draw differs.
    let (s0, h, _) = build(20);
    let hit = |t: u64| unit_unit(h.index, t, h.gen, SALT_DODGE);
    let yes = (1..500).find(|&t| hit(t) < 0.2).expect("a passing tick");
    let no = (1..500).find(|&t| hit(t) >= 0.2).expect("a failing tick");
    drop(s0);

    let fire = |skill: u8, tick: u64, st: &mut AiState| {
        let (mut s, h, other) = build(skill);
        s.tick = tick;
        s.unit_mut(h).unwrap().last_hit = Some((other, tick - 1));
        let (pad, _) = think(&s, h, st);
        (pad, s.tick)
    };

    // Skill 20 on a passing tick: it strafes, and the pad carries it.
    let mut st = AiState::default();
    let (pad, tick) = fire(20, yes, &mut st);
    assert_ne!(st.strafe_side, 0, "no dodge at tick {yes}");
    assert_eq!(st.strafe_until, tick + SIDESTEP_TICKS as u64);
    assert_eq!(st.dodge_until, tick + DODGE_COOLDOWN);
    assert_ne!(pad.lx, 0, "the strafe reaches the pad: {pad:?}");

    // The same unit on a failing tick: nothing.
    let mut st = AiState::default();
    let (pad, _) = fire(20, no, &mut st);
    assert_eq!(st.strafe_side, 0, "dodged on a failing coin at {no}");
    assert_eq!(pad.lx, 0);

    // Skill 1: `dodge_chance` is 0, so no tick can pass.
    let mut st = AiState::default();
    let (_, _) = fire(1, yes, &mut st);
    assert_eq!(st.strafe_side, 0, "skill 1 must never dodge");
    assert_eq!(st.dodge_until, 0);

    // The cooldown: a hit inside the 2 s window is ignored even on a
    // passing coin.
    let mut st = AiState::default();
    let (_, t0) = fire(20, yes, &mut st);
    let armed = st.dodge_until;
    let later = (yes + 1..armed).find(|&t| hit(t) < 0.2);
    if let Some(t) = later {
        let (mut s, h, other) = build(20);
        s.tick = t;
        s.unit_mut(h).unwrap().last_hit = Some((other, t - 1));
        st.strafe_side = 0;
        st.strafe_until = 0;
        think(&s, h, &mut st);
        assert_eq!(st.strafe_side, 0, "dodged at {t}, inside the cooldown");
        assert_eq!(st.dodge_until, armed, "the cooldown was re-armed");
    }
    assert_eq!(armed, t0 + DODGE_COOLDOWN);
}
