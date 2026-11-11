//! The unit grid against the plain loop over the units.

use super::*;
use crate::unit::{Controller, UnitSpec};

fn bot(s: &mut Sim, team: u8, p: Vec3) {
    s.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(team, p)
    })
    .expect("a slot");
}

/// xorshift32: the scenes' own draws.
struct Draw(u32);

impl Draw {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// A sim with `n` units, a few of them dead, in a crowd, along a line and
/// scattered over a map-sized square (negative cells and bucket collisions
/// included).
fn scene(d: &mut Draw, n: usize) -> Sim {
    let mut s = Sim::empty(1);
    for _ in 0..n {
        bot(&mut s, (d.next() & 1) as u8, Vec3::ZERO);
    }
    let crowd = Vec3::new(d.range(-300.0, 300.0), 0.0, d.range(-300.0, 300.0));
    for u in s.units.iter_mut() {
        u.soldier.pos = match d.next() % 4 {
            0 => crowd + Vec3::new(d.range(-6.0, 6.0), d.range(-2.0, 2.0), d.range(-6.0, 6.0)),
            1 => crowd + Vec3::new(d.range(-100.0, 100.0), 0.0, CELL * (d.next() % 3) as f32),
            _ => Vec3::new(
                d.range(-900.0, 900.0),
                d.range(-50.0, 50.0),
                d.range(-900.0, 900.0),
            ),
        };
        u.alive = !d.next().is_multiple_of(7);
    }
    s
}

fn plain(s: &Sim, p: Vec3, r: f32) -> Vec<u32> {
    let mut out = Vec::new();
    s.units_within(p, r, &mut out);
    out
}

/// Card TEAM's acceptance: the grid's "within r of p" equals the plain loop
/// on 1,000 random scenes, in the same (unit-index) order.
#[test]
fn the_grid_answers_as_the_plain_loop_on_a_thousand_scenes() {
    let mut d = Draw(0x1234_5678);
    let mut grid = UnitGrid::default();
    let (mut hits, mut cell_walks) = (0usize, 0usize);
    for _ in 0..1000 {
        let n = 1 + (d.next() % 200) as usize;
        let s = scene(&mut d, n);
        assert!(!s.unit_grid.live(), "a hand-made sim asks the units");
        grid.build(&s.units);
        for _ in 0..8 {
            let p = match d.next() % 3 {
                0 => s.units[(d.next() as usize) % s.units.len()].soldier.pos,
                _ => Vec3::new(d.range(-950.0, 950.0), 0.0, d.range(-950.0, 950.0)),
            } + Vec3::new(d.range(-3.0, 3.0), 0.0, d.range(-3.0, 3.0));
            let r = match d.next() % 7 {
                0 => 0.0,
                5 => f32::INFINITY,
                6 => f32::NAN,
                1 => d.range(0.0, 4.0),
                2 => d.range(4.0, 40.0),
                3 => d.range(40.0, 200.0),
                _ => d.range(0.0, 2000.0),
            };
            let mut got = Vec::new();
            grid.within(p, r, &mut got);
            assert_eq!(got, plain(&s, p, r), "scene of {n}, p {p}, r {r}");
            hits += got.len();
            let alive = s.units.iter().filter(|u| u.alive).count();
            let span = 2.0 * r / CELL + 2.0;
            cell_walks += usize::from(span * span < alive.min(BUCKETS) as f32);
        }
    }
    // Both answers were exercised: units found, and the cell walk taken
    // (not only the wide queries' walk over the units).
    assert!(hits > 10_000, "{hits} units found");
    assert!(cell_walks > 2_000, "{cell_walks} queries walked cells");
}

/// Inside the think window the sim answers from the grid; a unit moved
/// after the window closes is answered where it is.
#[test]
fn the_sim_asks_the_grid_only_while_it_is_live() {
    let mut d = Draw(99);
    let mut s = scene(&mut d, 40);
    let p = s.units[3].soldier.pos;
    s.unit_grid.build(&s.units);
    assert!(s.unit_grid.live());
    let before = plain(&s, p, 5.0);
    assert!(before.contains(&3));
    s.unit_grid.clear();
    s.units[3].soldier.pos += Vec3::new(50.0, 0.0, 0.0);
    assert!(!plain(&s, p, 5.0).contains(&3), "the units' own feet");
}

/// A stepped sim keeps the grid closed between ticks.
#[test]
fn a_step_leaves_the_grid_closed() {
    let mut s = Sim::empty(1);
    bot(&mut s, 0, Vec3::ZERO);
    bot(&mut s, 1, Vec3::new(10.0, 0.0, 0.0));
    s.step(&[]);
    assert!(!s.unit_grid.live());
}

/// Card TEAM: the follow gap. Bot `a` walks +X at the origin; one other
/// unit at `p` walking `v`, of team `team`; `a_index_first` = the bot has
/// the lower index.
fn ease_with(p: Vec3, v: Vec3, team: u8, a_index_first: bool) -> f32 {
    let mut s = Sim::empty(1);
    let (ia, ib) = if a_index_first { (0, 1) } else { (1, 0) };
    for i in 0..2 {
        if i == ia {
            bot(&mut s, 0, Vec3::ZERO);
        } else {
            bot(&mut s, team, p);
        }
    }
    s.units[ia].soldier.pos = Vec3::ZERO;
    s.units[ib].soldier.pos = p;
    s.units[ib].soldier.vel = v;
    s.units[ia].soldier.vel = Vec3::new(6.0, 0.0, 0.0);
    let me = s.handle(ia);
    follow_ease(&s, me, Vec3::X)
}

#[test]
fn a_bot_eases_off_behind_a_mate_walking_ahead_the_same_way() {
    let run = Vec3::new(6.0, 0.0, 0.0);
    assert_eq!(
        ease_with(Vec3::new(1.5, 0.0, 0.0), run, 0, true),
        FOLLOW_STICK,
        "1.5 m ahead"
    );
    assert_eq!(
        ease_with(Vec3::new(1.5, 0.0, 1.0), run, 0, true),
        FOLLOW_STICK,
        "34 deg off"
    );
    assert_eq!(
        ease_with(Vec3::new(2.1, 0.0, 0.0), run, 0, true),
        1.0,
        "2.1 m ahead"
    );
    assert_eq!(
        ease_with(Vec3::new(1.0, 0.0, 1.0), run, 0, true),
        1.0,
        "45 deg off"
    );
    assert_eq!(
        ease_with(Vec3::new(-1.5, 0.0, 0.0), run, 0, true),
        1.0,
        "behind"
    );
    assert_eq!(
        ease_with(Vec3::new(1.5, 0.0, 0.0), run, 1, true),
        1.0,
        "an enemy"
    );
    assert_eq!(
        ease_with(Vec3::new(1.5, 0.0, 0.0), Vec3::ZERO, 0, true),
        1.0,
        "a mate standing"
    );
    assert_eq!(
        ease_with(Vec3::new(1.5, 0.0, 0.0), -run, 0, true),
        1.0,
        "coming the other way"
    );
    assert_eq!(
        ease_with(Vec3::new(1.5, 0.0, 0.0), Vec3::new(0.0, 0.0, 6.0), 0, true),
        1.0,
        "crossing"
    );
    assert_eq!(
        ease_with(Vec3::new(1.5, 2.5, 0.0), run, 0, true),
        1.0,
        "another storey"
    );
    // Abreast: the lower index leads, the higher eases off.
    assert_eq!(
        ease_with(Vec3::new(0.2, 0.0, 1.0), run, 0, false),
        FOLLOW_STICK,
        "abreast, the mate's index lower"
    );
    assert_eq!(
        ease_with(Vec3::new(0.2, 0.0, 1.0), run, 0, true),
        1.0,
        "abreast, the bot's index lower"
    );
}

/// Card TEAM: L graph's post (team 1's, a 6 m sphere at (40, 0, -40)) with
/// five AI soldiers of team 0 standing in it at tier `d` (the first at its
/// edge), a crate's soldier barrier 4 m west of the middle (its eight edge
/// points beside the hub's nine within 1.4 m: room for more than two
/// guards), the table built, and an enemy 100 m up -Z (out of sight: the
/// approach).
fn cover_scene(d: crate::skill::Difficulty) -> (Sim, Vec<usize>) {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.difficulty = d;
    if let Some(g) = sim.nav.as_mut() {
        g.barriers.push(crate::nav::Barrier {
            centre: glam::Vec2::new(36.0, -40.0),
            axes: [glam::Vec2::X, glam::Vec2::Y],
            half: glam::Vec2::new(0.5, 0.5),
            mask: 1 << crate::nav::SOLDIER,
            enabled: true,
        });
    }
    sim.build_positions();
    let post = Vec3::new(40.0, 0.0, -40.0);
    let bots = (0..5)
        .map(|k| {
            let a = k as f32 * 1.3;
            let r = if k == 0 { 5.0 } else { 2.5 };
            let i = sim.units.len();
            bot(&mut sim, 0, post + Vec3::new(a.cos(), 0.0, a.sin()) * r);
            i
        })
        .collect();
    sim.spawn(&UnitSpec::trooper(1, Vec3::new(40.0, 0.0, -140.0)))
        .expect("enemy");
    (sim, bots)
}

fn covering(sim: &Sim) -> Vec<usize> {
    (0..sim.units.len())
        .filter(|&i| sim.units[i].ai.role.post == Some(0))
        .collect()
}

/// Card TEAM (bot design 4.3, section 7): while one soldier captures a post,
/// two more of the team standing in it - no more - take the Cover role at
/// Hard and Elite, and none at Easy or Normal. The capturer is the lowest
/// index; the others go in unit order. Each guard position lies in reach of
/// the post, clear of the capturer and of the other, and faces the approach.
#[test]
fn two_at_most_cover_a_capture_and_only_at_hard_and_elite() {
    use crate::skill::Difficulty;
    for (d, n) in [
        (Difficulty::Easy, 0),
        (Difficulty::Normal, 0),
        (Difficulty::Hard, 2),
        (Difficulty::Elite, 2),
    ] {
        let (mut sim, bots) = cover_scene(d);
        assert!(!sim.positions.is_empty(), "the table");
        sim.tick = 0;
        board(&mut sim);
        let got = covering(&sim);
        assert_eq!(got.len(), n, "{d:?}: {got:?}");
        if n == 0 {
            continue;
        }
        assert_eq!(
            got,
            bots[1..3].to_vec(),
            "{d:?}: the next two in unit order"
        );
        let post = Vec3::new(40.0, 0.0, -40.0);
        let feet = sim.units[bots[0]].soldier.pos;
        let spots: Vec<Vec3> = got
            .iter()
            .map(|&i| sim.positions.positions[sim.units[i].ai.role.at as usize].pos)
            .collect();
        for (&i, p) in got.iter().zip(&spots) {
            let r = &sim.units[i].ai.role;
            assert!(crate::nav::xz_dist(*p, post) <= 6.0 + COVER_REACH, "{p}");
            assert!(
                crate::nav::xz_dist(*p, feet) >= COVER_CLEAR,
                "{p} by the capturer"
            );
            assert!(
                r.look.z < p.z - 10.0,
                "faces the approach up -Z: {}",
                r.look
            );
            assert_eq!(r.since, 0);
        }
        assert!(crate::nav::xz_dist(spots[0], spots[1]) >= crate::ai::tactic::SPACING);
        // The board runs once a second per team: team 0 on tick 0 mod 30.
        sim.tick = 1;
        sim.units[bots[3]].ai.role = Default::default();
        board(&mut sim);
        assert_eq!(covering(&sim).len(), 2, "nothing on team 1's tick");
    }
}

/// Card TEAM: in a running battle at Hard the board never has more than
/// [`COVER_CAP`] covering the post, a coverer walks to its guard position
/// and guards there (`Behaviour::Guard`), and the roles end once the post
/// is the team's.
#[test]
fn cover_keeps_its_capacity_and_ends_with_the_capture() {
    use crate::ai::Behaviour;
    use crate::sim::PadState;
    let (mut sim, _) = cover_scene(crate::skill::Difficulty::Hard);
    let (mut most, mut guarded, mut captured_at) = (0usize, false, None);
    for _ in 0..900 {
        sim.step(&[PadState::default()]);
        let got = covering(&sim);
        most = most.max(got.len());
        assert!(got.len() <= COVER_CAP, "tick {}: {got:?}", sim.tick);
        for &i in &got {
            let u = &sim.units[i];
            let at = sim.positions.positions[u.ai.role.at as usize].pos;
            guarded |= u.ai.behaviour == Behaviour::Guard
                && crate::nav::xz_dist(u.soldier.pos, at) <= crate::ai::HOLD_ARRIVE + 0.5;
        }
        if captured_at.is_none() && sim.cps[0].owner == Some(0) {
            captured_at = Some(sim.tick);
        }
        if captured_at.is_some_and(|t| sim.tick > t + crate::TICK_HZ as u64) {
            assert!(got.is_empty(), "tick {}: the post is taken", sim.tick);
        }
    }
    assert_eq!(most, COVER_CAP, "the role was given");
    assert!(guarded, "a coverer stood guard at its position");
    assert!(captured_at.is_some(), "the post was taken");
}

/// Card FLNK: the diamond (`nav::flank::tests::diamond`: posts on hubs 0
/// and 3, the authored way over hub 1, the flank over hub 2) on flat
/// ground, team 0 owning the post at hub 0 and team 1 the one at hub 3, and
/// `n` AI soldiers of team 0 at hub 0 attacking post 1 by the plan, each on
/// its first leg (the authored hop to hub 1); the pair's share is `share`.
fn flank_scene(n: usize, share: f32, d: crate::skill::Difficulty) -> Sim {
    use crate::cp::CpDef;
    use crate::region::Region;
    let mut sim = Sim::empty(7);
    sim.difficulty = d;
    sim.heightfield = Some(crate::heightfield::Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    sim.nav = Some(crate::nav::flank::tests::diamond(-8.0));
    for (x, owner) in [(0.0, 0), (40.0, 1)] {
        let at = Vec3::new(x, 0.0, 0.0);
        sim.add_cp(
            CpDef {
                name: format!("cp{owner}"),
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
            Some(owner),
        );
    }
    let k = sim.flank.find(0, 3).expect("the diamond's flank");
    sim.flank.routes[k].share = share;
    for j in 0..n {
        bot(&mut sim, 0, Vec3::new(0.3 * j as f32 - 1.0, 0.0, 0.5));
    }
    sim.commander[0].assignment = vec![Some(crate::commander::Objective::Attack(1)); n];
    for i in 0..n {
        let h = sim.handle(i);
        let mut st = sim.units[i].ai;
        st.flank = crate::ai::FlankState {
            from: 0,
            to: 3,
            ..Default::default()
        };
        crate::ai::route::plan_to(&sim, &mut st, h, 0, 3, &mut Default::default());
        assert_eq!(st.route().map(|r| r.next_hub), Some(1), "the authored hop");
        sim.units[i].ai = st;
    }
    sim.tick = 0;
    sim
}

fn flanking(sim: &Sim) -> Vec<usize> {
    (0..sim.units.len())
        .filter(|&i| sim.units[i].ai.flank.route.is_some())
        .collect()
}

/// Card FLNK (bot design 4.3, 6.2): of a group of 10 attackers with a flank
/// route and a share of 0.2, exactly 2 take it - the first two in unit order,
/// re-planned onto the route's first hop - and a second pass gives no more;
/// a group of 2 takes none; Easy, the lowest tier, flanks too.
#[test]
fn two_of_ten_attackers_flank_at_a_share_of_a_fifth_and_none_of_two() {
    use crate::skill::Difficulty;
    let mut sim = flank_scene(10, 0.2, Difficulty::Normal);
    board(&mut sim);
    assert_eq!(flanking(&sim), vec![0, 1]);
    assert_eq!(sim.nav_stats.flank_given, 2);
    for i in 0..10 {
        let r = sim.units[i].ai.route().expect("a leg");
        let (next, src) = if i < 2 {
            (2, crate::ai::RouteSource::Flank)
        } else {
            (1, crate::ai::RouteSource::Table)
        };
        assert_eq!((r.next_hub, r.source), (next, src), "unit {i}");
    }
    sim.tick = 30;
    board(&mut sim);
    assert_eq!(flanking(&sim), vec![0, 1], "the next second gives no more");
    for (n, share, want) in [
        (2, 0.2, 0),
        (2, 0.3, 0),
        (3, 0.2, 1),
        (5, 0.1, 1),
        (10, 0.3, 3),
    ] {
        let mut sim = flank_scene(n, share, Difficulty::Normal);
        board(&mut sim);
        assert_eq!(flanking(&sim).len(), want, "{n} at {share}");
    }
    let mut sim = flank_scene(10, 0.2, Difficulty::Easy);
    board(&mut sim);
    assert_eq!(flanking(&sim), vec![0, 1], "Normal");
}

/// Card FLNK: a flanker walks the route's hubs (0 -> 2 -> 3) in a running
/// sim, and the role ends with the last leg into the post.
#[test]
fn a_flanker_walks_the_other_side_and_the_role_ends_at_the_post() {
    use crate::sim::PadState;
    let mut sim = flank_scene(5, 0.2, crate::skill::Difficulty::Normal);
    board(&mut sim);
    assert_eq!(flanking(&sim), vec![0]);
    let (mut via2, mut via1, mut ended) = (false, false, false);
    for _ in 0..1800 {
        sim.step(&[PadState::default()]);
        let u = &sim.units[0];
        let p = u.soldier.pos;
        via2 |= crate::nav::xz_dist(p, Vec3::new(20.0, 0.0, -8.0)) < 3.0;
        via1 |= crate::nav::xz_dist(p, Vec3::new(20.0, 0.0, 5.0)) < 3.0;
        if u.ai.route().is_some_and(|r| r.is_last()) && u.ai.flank.route.is_none() {
            ended = true;
            break;
        }
    }
    assert!(via2 && !via1, "over hub 2, not hub 1: {via2} {via1}");
    assert!(ended, "the role ends on the last leg");
}
