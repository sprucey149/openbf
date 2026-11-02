use super::*;
use crate::cp::CpDef;
use crate::heightfield::Heightfield;
use crate::nav::{Arc, Hub, NavGraph, RouteTable};
use crate::region::Region;
use crate::squad::CommandKind;
use crate::unit::{Controller, UnitSpec};

/// Three hubs in a line (0 - 1 - 2), soldier arcs, no authored route table
/// (the BFS answers every hop) — enough for a hop-distance field with a
/// near post and a far one.
fn line_graph() -> NavGraph {
    let hub = |x: f32| Hub {
        pos: Vec3::new(x, 0.0, 0.0),
        radius: 2.0,
        arcs: Vec::new(),
    };
    let mut hubs = vec![hub(0.0), hub(40.0), hub(80.0)];
    hubs[0].arcs = vec![0];
    hubs[1].arcs = vec![0, 1];
    hubs[2].arcs = vec![1];
    let arcs = vec![
        Arc {
            a: 0,
            b: 1,
            type_mask: 1,
            flags: 0,
        },
        Arc {
            a: 1,
            b: 2,
            type_mask: 1,
            flags: 0,
        },
    ];
    NavGraph::new(hubs, arcs, RouteTable::empty(3), Vec::new(), Vec::new()).expect("graph")
}

fn post(name: &str, x: f32, value: i32) -> CpDef {
    CpDef {
        name: name.into(),
        pos: Vec3::new(x, 0.0, 0.0),
        capture: Region::sphere(Vec3::new(x, 0.0, 0.0), 6.0),
        control: None,
        value_bleed: 100,
        value_atk: [value, value],
        value_def: [value, value],
        spawn_nodes: vec![],
        capture_time: 10.0,
        neutralize_time: 12.0,
    }
}

/// A flat world with the line graph, `owners` posts at the three hubs and
/// four AI of team 0 standing at hub 1.
fn world(owners: &[Option<u8>]) -> Sim {
    let mut sim = Sim::empty(7);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    sim.nav = Some(line_graph());
    for (i, o) in owners.iter().enumerate() {
        sim.add_cp(post(&format!("cp{i}"), i as f32 * 40.0, 10), *o);
    }
    for k in 0..4 {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(40.0, 0.0, k as f32 * 2.0))
        })
        .expect("spawned");
    }
    // One enemy, far enough away (300 m) that no post is CONTESTED, so
    // the tests choose when to contest one — and near enough to hub 2 that
    // HUNT always has somewhere to go.
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(1, Vec3::new(80.0, 0.0, 300.0))
    })
    .expect("spawned");
    sim
}

/// Twelve more AI of team 0, 100 m and more from every post: a team of
/// sixteen, whose share of defenders a post is four (card DEFS).
fn crowd(sim: &mut Sim) {
    for k in 0..12 {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 100.0 + 2.0 * k as f32))
        })
        .expect("spawned");
    }
}

fn assignments(sim: &Sim, team: u8) -> Vec<Option<Objective>> {
    sim.commander[usize::from(team)].assignment.clone()
}

/// The squad-order world (brief D5): `values.len()` ENEMY posts at
/// `x = 40 i` (so every one of them is an Attack objective and Hunt is
/// never offered), `bots` AI of team 0 standing at hub 1, and the ISSUER —
/// a player of team 0 in slot 0 — standing with them at x = 40, which puts
/// every bot inside [`crate::squad::ORDER_REACH`]. Bots are indices
/// `0..bots`, the player is `bots`, the enemy `bots + 1`.
fn squad_world(values: &[i32], bots: usize) -> Sim {
    let mut sim = Sim::empty(7);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    sim.nav = Some(line_graph());
    for (i, v) in values.iter().enumerate() {
        sim.add_cp(post(&format!("cp{i}"), i as f32 * 40.0, *v), Some(1));
    }
    for k in 0..bots {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(40.0, 0.0, k as f32 * 2.0))
        })
        .expect("bot");
    }
    sim.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(40.0, 0.0, 0.0)))
        .expect("issuer");
    // One enemy 300 m off, as in `world`: it contests nothing, and Hunt
    // has somewhere to go if it is ever offered.
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(1, Vec3::new(80.0, 0.0, 300.0))
    })
    .expect("enemy");
    sim
}

/// A hint at `x` of `kind` whose allowed-stance mask is `mask` (what the
/// loader has already read out of the authored `PrimaryStance` — see
/// [`stance_mask`]).
fn hint(name: &str, kind: u8, x: f32, mask: u8) -> crate::nav::Hint {
    crate::nav::Hint {
        name: name.into(),
        kind,
        pos: Vec3::new(x, 0.0, 0.0),
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        radius: None,
        mode: None,
        stance_mask: mask,
        secondary_stance_mask: MASK_STAND,
        peek_sides: 0,
        command_post: None,
    }
}

/// The RE's ANSWER TABLE for `PrimaryStance`, both games, verbatim
/// (`engine/artifacts/lead_tmp/s15/re/primary_stance.md`, lead-verified
/// 2026-09-08 evening; `analysis/RE_LOG.md` 2026-09-08 evening). The field
/// is a BITMASK: 3 is stand-or-crouch and NOT prone, BF2 masks `& 3` so it
/// never prones at all, and BF1's 4-7 are the only values that can.
#[test]
fn the_primary_stance_mask_is_the_res_answer_table() {
    // v -> (BF2 mask, BF1 mask), one row per row of the RE's table.
    let table: [(Option<u8>, u8, u8); 8] = [
        (Some(1), MASK_STAND, MASK_STAND),
        (Some(2), MASK_CROUCH, MASK_CROUCH),
        // NOT prone on either game — the old 1/2/3 enum reading's error.
        (Some(3), MASK_STAND | MASK_CROUCH, MASK_STAND | MASK_CROUCH),
        // BF2: `4 & 3 == 0` -> not stored -> the ctor default, stand.
        (Some(4), MASK_STAND, MASK_PRONE),
        (Some(5), MASK_STAND, MASK_STAND | MASK_PRONE),
        (Some(6), MASK_CROUCH, MASK_CROUCH | MASK_PRONE),
        (
            Some(7),
            MASK_STAND | MASK_CROUCH,
            MASK_STAND | MASK_CROUCH | MASK_PRONE,
        ),
        // Absent: both ctors default to stand, so a hint always wants one.
        (None, MASK_STAND, MASK_STAND),
    ];
    for (v, bf2, bf1) in table {
        assert_eq!(stance_mask(v, "bf2"), bf2, "BF2 PrimaryStance {v:?}");
        assert_eq!(stance_mask(v, "bf1"), bf1, "BF1 PrimaryStance {v:?}");
        // The name comes from the manifest and its case is not ours.
        assert_eq!(stance_mask(v, "BF1"), bf1, "case-insensitive");
    }
    // BF2 can never be handed a prone bit; every mask is non-zero.
    for v in 0..=255u8 {
        assert_eq!(stance_mask(Some(v), "bf2") & MASK_PRONE, 0, "BF2 {v} prone");
        assert_ne!(stance_mask(Some(v), "bf2"), 0);
        assert_ne!(stance_mask(Some(v), "bf1"), 0);
        // PORT GUARD (no disc authors it): a value with no low bit set at
        // all would spin the PS2 picker, and answers stand here.
        if v & 7 == 0 {
            assert_eq!(stance_mask(Some(v), "bf1"), MASK_STAND, "guard {v}");
        }
    }
    // The peek sides are the authored bits 3 and 4, and nothing else.
    assert_eq!(peek_sides(None), 0);
    assert_eq!(peek_sides(Some(3)), 0);
    assert_eq!(peek_sides(Some(8 | 3)), 1);
    assert_eq!(peek_sides(Some(16 | 2)), 2);
    assert_eq!(peek_sides(Some(24 | 1)), 3);
    assert_eq!(peek_sides(Some(0xE0)), 0, "bits above 4 are not sides");
}

/// The stance CODE (what `AiState::want_stance` carries) round-trips, and
/// the 0 sentinel and the stances no hint can author stay outside it.
#[test]
fn the_stance_code_is_the_mask_bit_index_plus_one() {
    assert_eq!(stance_of_code(1), Some(Stance::Stand));
    assert_eq!(stance_of_code(2), Some(Stance::Crouch));
    assert_eq!(stance_of_code(3), Some(Stance::Prone));
    for c in [0u8, 4, 5, 255] {
        assert_eq!(stance_of_code(c), None, "code {c} means nothing");
        assert_eq!(stance_bit(c), 0);
    }
    for c in [1u8, 2, 3] {
        assert_eq!(stance_code(stance_of_code(c).expect("mapped")), c);
    }
    // A code's bit is the mask bit the loader would have set for it.
    assert_eq!(stance_bit(1), MASK_STAND);
    assert_eq!(stance_bit(2), MASK_CROUCH);
    assert_eq!(stance_bit(3), MASK_PRONE);
    // The code of a `Stance` no hint can author must be 0 — the sentinel
    // `AiState::want_stance` reads as "no wish".
    for s in [Stance::Sprint, Stance::Jump, Stance::Roll, Stance::Fall] {
        assert_eq!(stance_code(s), 0);
    }
}

/// D13(a): a Snipe hint is only offered to a unit whose weapon passes the
/// AI distance table; a Cover hint is offered to everyone. D13(b): the
/// spot comes back with the hint's allowed-stance MASK.
#[test]
fn a_snipe_hint_is_for_scoped_units_only_and_the_spot_carries_its_stance() {
    let mut sim = world(&[Some(0), None, None]);
    // cp0 is at x = 0. A Snipe hint 3 m from it (prone: a BF1 hint
    // authoring 4) and a Cover hint 9 m from it (crouch): the Snipe one is
    // nearer to both the post and the unit, so it wins whenever it is on
    // offer at all.
    sim.nav.as_mut().expect("graph").hints = vec![
        hint("snipe", HINT_SNIPE, 3.0, MASK_PRONE),
        hint("cover", HINT_COVER, 9.0, MASK_CROUCH),
    ];
    let from = Vec3::ZERO;
    assert_eq!(
        defend_spot(&sim, 0, from, true),
        Some((Vec3::new(3.0, 0.0, 0.0), MASK_PRONE)),
        "a scoped unit takes the nearer Snipe hint and its stance"
    );
    assert_eq!(
        defend_spot(&sim, 0, from, false),
        Some((Vec3::new(9.0, 0.0, 0.0), MASK_CROUCH)),
        "an unscoped unit does not see the Snipe hint at all"
    );
    // A hint outside HINT_RADIUS of the POST is not a spot, however near
    // the unit stands to it.
    sim.nav.as_mut().expect("graph").hints = vec![hint("far", HINT_COVER, 26.0, MASK_STAND)];
    assert_eq!(defend_spot(&sim, 0, Vec3::new(26.0, 0.0, 0.0), true), None);
    // A mask with two bits comes back whole — the DRAW is the caller's
    // (`ai::stance_hold`), not the spot's.
    sim.nav.as_mut().expect("graph").hints =
        vec![hint("odd", HINT_COVER, 5.0, MASK_STAND | MASK_CROUCH)];
    assert_eq!(
        defend_spot(&sim, 0, from, true),
        Some((Vec3::new(5.0, 0.0, 0.0), MASK_STAND | MASK_CROUCH))
    );
    // `hint_stance` is the same rule centred on the UNIT: 40 m from cp0 is
    // outside the post's radius, but the unit is standing on the hint.
    sim.nav.as_mut().expect("graph").hints = vec![hint("here", HINT_COVER, 40.0, MASK_PRONE)];
    assert_eq!(defend_spot(&sim, 0, Vec3::new(40.0, 0.0, 0.0), true), None);
    assert_eq!(
        hint_stance(&sim, Vec3::new(40.0, 0.0, 0.0), true),
        MASK_PRONE
    );
    // No hint in range is the one "no wish" answer.
    assert_eq!(hint_stance(&sim, Vec3::new(80.0, 0.0, 0.0), true), 0);
}

/// The plan is a pure function of sim state: two identical sims stepped
/// the same way produce the same assignment, and the same hash.
#[test]
fn the_plan_is_deterministic_and_replay_exact() {
    let run = || {
        let mut sim = world(&[Some(1), None, Some(1)]);
        for _ in 0..90 {
            sim.step(&[crate::sim::PadState::default()]);
        }
        (assignments(&sim, 0), sim.hash())
    };
    let (a, ha) = run();
    let (b, hb) = run();
    assert_eq!(a, b);
    assert_eq!(ha, hb);
    assert!(a.iter().any(|o| o.is_some()), "somebody got an objective");
    // The plan really is part of the hash: perturbing it changes it.
    let mut sim = world(&[Some(1), None, Some(1)]);
    sim.step(&[crate::sim::PadState::default()]);
    let before = sim.hash();
    sim.commander[0].assignment[1] = Some(Objective::Hunt);
    assert_ne!(before, sim.hash(), "the plan is in the sim hash");
}

/// D5: a Follow or Hold bot is the PLAYER'S — it leaves the commander's
/// pool the moment the order lands, stays out of every plan while it
/// stands, and comes back at the first plan after `clear_all`.
#[test]
fn a_followed_or_holding_bot_is_out_of_the_plan_until_it_is_cleared() {
    for follow in [true, false] {
        let mut sim = squad_world(&[10, 10, 10], 4);
        plan(&mut sim, 0);
        assert!(
            assignments(&sim, 0)[..4].iter().all(|o| o.is_some()),
            "every bot starts in the pool"
        );
        let h = sim.players[0].expect("the issuer");
        let kind = if follow {
            CommandKind::Follow(h)
        } else {
            CommandKind::Hold(Vec3::ZERO)
        };
        // A FOLLOW is taken by two (card SQACK), a HOLD by everyone.
        let n = if follow { crate::squad::FOLLOW_CAP } else { 4 };
        assert_eq!(usize::from(sim.issue_command(0, kind)), n);
        plan(&mut sim, 0);
        assert!(
            assignments(&sim, 0)[..n].iter().all(|o| o.is_none()),
            "out of the pool while the order stands ({kind:?})"
        );
        assert!(
            sim.units[..n].iter().all(|u| u.ai.squad.is_none()),
            "and out of the squad bookkeeping"
        );
        assert_eq!(usize::from(sim.clear_all(0)), n);
        plan(&mut sim, 0);
        assert!(
            assignments(&sim, 0)[..4].iter().all(|o| o.is_some()),
            "back in the pool after clear_all ({kind:?})"
        );
    }
}

/// Queue HDL: a hint on a ledge above the post is not a defend spot, even
/// when it is the nearer one — the defender would walk its straight last
/// leg into the wall under it (POL1's `hintnode153`, 8.5 m above cp6con).
/// The same-floor hint is taken instead; with none on the floor this
/// optional hint query returns `None`.
#[test]
fn a_hint_above_the_posts_level_is_not_a_defend_spot() {
    use crate::collision::{Body, CollisionWorld, Shape};
    use glam::Mat4;
    let mut sim = world(&[Some(0), None, None]);
    // A ledge 3 m from cp0 (x = 0), its top at y 8.
    sim.collision = CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(3.0, 7.5, 0.0)),
            Shape::Box {
                half: Vec3::new(2.0, 0.5, 2.0),
            },
        )],
        Vec::new(),
    );
    let mut up = hint("up", HINT_COVER, 3.0, MASK_CROUCH);
    up.pos.y = 8.0;
    sim.nav.as_mut().expect("graph").hints = vec![up, hint("floor", HINT_COVER, 9.0, MASK_STAND)];
    let from = Vec3::ZERO;
    assert_eq!(
        defend_spot(&sim, 0, from, false),
        Some((Vec3::new(9.0, 0.0, 0.0), MASK_STAND)),
        "the nearer hint is 8 m up: the same-floor one is the spot"
    );
    // A hint just off the floor (its own floor is still the post's) is on
    // the level.
    sim.nav.as_mut().expect("graph").hints[1].pos.y = 0.5;
    sim.nav.as_mut().expect("graph").hints[1].pos.x = 2.0;
    sim.nav.as_mut().expect("graph").hints[1].pos.z = 5.0;
    assert_eq!(
        defend_spot(&sim, 0, from, false),
        Some((Vec3::new(2.0, 0.5, 5.0), MASK_STAND))
    );
    // Only the ledge hint: the optional hint query returns no spot.
    sim.nav.as_mut().expect("graph").hints.truncate(1);
    assert_eq!(defend_spot(&sim, 0, from, false), None);
}

/// Card STLVL: a hint UNDER the post's level with nothing over it is a
/// spot only when a bot can drop to it — BF2 DEA1's cp2 has one 6 m
/// under its gallery, past the AI's ledge rule.
#[test]
fn a_hint_a_drop_too_deep_under_the_post_is_not_a_defend_spot() {
    let sim = world(&[Some(0), None, None]);
    let on_ground = Vec3::new(9.0, 0.0, 0.0);
    assert!(hint_on_level(&sim, 0.0, on_ground), "the post's own floor");
    assert!(hint_on_level(&sim, 3.0, on_ground), "a 3 m drop");
    assert!(
        !hint_on_level(&sim, crate::ai::LEDGE_DROP + 2.0, on_ground),
        "a 6 m drop"
    );
}

/// D8: [`hint_spot`] is [`defend_spot`]'s body over any centre and radius —
/// a squad `Hold` asks it about an ANCHOR at
/// 10 m, where a defender asks about a post at
/// [`HINT_RADIUS`], and the radius is the only difference.
#[test]
fn the_hint_spot_is_the_defend_spot_over_any_centre_and_radius() {
    let mut sim = world(&[Some(1), None, Some(1)]);
    let g = sim.nav.as_mut().expect("graph");
    // Cover at x = 49 and x = 51, an anchor at x = 40: 9 m and 11 m off.
    g.hints = vec![
        hint("near", HINT_COVER, 49.0, MASK_CROUCH),
        hint("far", HINT_COVER, 51.0, MASK_STAND),
    ];
    let anchor = Vec3::new(40.0, 0.0, 0.0);
    let hold = 10.0;
    assert_eq!(
        hint_spot(&sim, anchor, hold, anchor, false),
        Some((Vec3::new(49.0, 0.0, 0.0), MASK_CROUCH)),
        "the one inside 10 m"
    );
    // Only the far one is authored: nothing is in range of the anchor, and
    // the SAME query at the post radius finds it.
    sim.nav.as_mut().expect("graph").hints.remove(0);
    assert_eq!(
        hint_spot(&sim, anchor, hold, anchor, false),
        None,
        "11 m off"
    );
    assert_eq!(
        hint_spot(&sim, anchor, HINT_RADIUS, anchor, false),
        Some((Vec3::new(51.0, 0.0, 0.0), MASK_STAND)),
        "in range of a post"
    );
    // And Defend still goes through it: the post at x = 40 is cp1.
    assert_eq!(
        defend_spot(&sim, 1, anchor, false),
        hint_spot(&sim, sim.cps[1].def.pos, HINT_RADIUS, anchor, false),
        "defend_spot IS hint_spot at the post"
    );
}

fn dp(owner: Option<u8>, team: u8, atk: i32, def: i32) -> DrawPost {
    DrawPost {
        friendly: owner == Some(team),
        neutral: owner.is_none(),
        atk,
        def,
        open: true,
    }
}

/// `n` draws of `posts` with the plan's own per-unit number, as the share
/// (%) of each post.
fn shares(rules: GoalRules, team: u8, posts: &[DrawPost], n: u32) -> Vec<f32> {
    let mut hits = vec![0u32; posts.len()];
    for k in 0..n {
        // 32 unit slots over consecutive plans, as a round draws them.
        let r = roll(k % 32, u64::from(k / 32) * PLAN_PERIOD + 1, 1 + k / 640);
        assert!((0.0..1.0).contains(&r), "{r}");
        let c = draw_post(rules, team, posts, r).expect("a post");
        hits[c as usize] += 1;
    }
    hits.iter().map(|&h| 100.0 * h as f32 / n as f32).collect()
}

/// The draw on Polis Massa's start (`analysis/re-conquest-goal.md` section
/// 8's test vectors): port team 0 (the PS2's team 1) owns cp6con 20,
/// cp4con 10 and cp1con 10, team 1 cp5con 20 and cp3con 10, cp2con (20)
/// is neutral. Team 1's candidates weigh 20 : 20 : 10 : 10 (PS2: 21 / 27
/// / 17 / 14 of 79 draws), team 0's 20 : 20 : 10 (13 / 17 / 8 of 38), and
/// an own post is never drawn while a candidate exists.
#[test]
fn the_draw_is_by_value_over_the_posts_that_are_not_the_teams() {
    // cp1con, cp2con, cp3con, cp4con, cp5con, cp6con.
    let owners = [Some(0), None, Some(1), Some(0), Some(1), Some(0)];
    let values = [10, 20, 10, 10, 20, 20];
    let posts = |team: u8| -> Vec<DrawPost> {
        owners
            .iter()
            .zip(values)
            .map(|(o, v)| dp(*o, team, v, v))
            .collect()
    };
    let close = |got: &[f32], want: &[f32]| {
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() <= 3.0, "{got:?} against {want:?}");
        }
    };
    let third = 100.0 / 3.0;
    let s = shares(GoalRules::Bf2, 1, &posts(1), 10_000);
    close(&s, &[third / 2.0, third, 0.0, third / 2.0, 0.0, third]);
    assert_eq!((s[2], s[4]), (0.0, 0.0), "never an own post: {s:?}");
    let s = shares(GoalRules::Bf2, 0, &posts(0), 10_000);
    close(&s, &[0.0, 40.0, 20.0, 0.0, 40.0, 0.0]);
    assert_eq!((s[0], s[3], s[5]), (0.0, 0.0, 0.0), "{s:?}");
}

/// BF2 weighs a hostile post by ATK and a neutral one by the mean of ATK
/// and DEF, and drops a value of 0 and a post the unit cannot reach.
#[test]
fn the_bf2_draw_weighs_by_owner_and_drops_zero_and_unreachable_posts() {
    let mut posts = vec![
        dp(Some(1), 0, 30, 0),
        dp(None, 0, 30, 10),
        dp(Some(1), 0, 0, 50),
        dp(Some(1), 0, 40, 0),
    ];
    posts[3].open = false;
    let s = shares(GoalRules::Bf2, 0, &posts, 10_000);
    for (g, w) in s.iter().zip([60.0, 40.0, 0.0, 0.0]) {
        assert!((g - w).abs() <= 3.0, "{s:?}");
    }
    assert_eq!((s[2], s[3]), (0.0, 0.0), "{s:?}");
}

/// BF1 (`FUN_002617f8`): the PS2's team 1 (port team 0) weighs every
/// candidate by `Value_ATK`, its team 2 by `Value_DEF`, whoever owns the
/// post — a neutral post is no mean.
#[test]
fn the_bf1_draw_weighs_team_1_by_atk_and_team_2_by_def() {
    // (ATK, DEF): 30 / 10 and 10 / 30, one hostile and one neutral.
    let posts = |team: u8| vec![dp(Some(1 - team), team, 30, 10), dp(None, team, 10, 30)];
    let s = shares(GoalRules::Bf1, 0, &posts(0), 10_000);
    assert!((s[0] - 75.0).abs() <= 3.0, "team 0 by ATK: {s:?}");
    let s = shares(GoalRules::Bf1, 1, &posts(1), 10_000);
    assert!((s[0] - 25.0).abs() <= 3.0, "team 1 by DEF: {s:?}");
    // BF2 on the same posts: 30 against the mean 20.
    let s = shares(GoalRules::Bf2, 1, &posts(1), 10_000);
    assert!((s[0] - 60.0).abs() <= 3.0, "{s:?}");
}

/// With no candidate BF2 draws among its own posts by `Value_DEF`, then
/// uniformly (never on a one-post map); BF1 goes straight to uniform.
#[test]
fn with_nothing_to_attack_the_draw_falls_back() {
    let own = vec![dp(Some(0), 0, 10, 30), dp(Some(0), 0, 10, 10)];
    let s = shares(GoalRules::Bf2, 0, &own, 10_000);
    assert!((s[0] - 75.0).abs() <= 3.0, "own posts by DEF: {s:?}");
    let s = shares(GoalRules::Bf1, 0, &own, 10_000);
    assert!((s[0] - 50.0).abs() <= 3.0, "BF1 uniform: {s:?}");
    let zero = vec![dp(Some(0), 0, 10, 0), dp(Some(0), 0, 10, 0)];
    let s = shares(GoalRules::Bf2, 0, &zero, 10_000);
    assert!((s[0] - 50.0).abs() <= 3.0, "then uniform: {s:?}");
    assert_eq!(draw_post(GoalRules::Bf2, 0, &zero[..1], 0.5), None);
    assert_eq!(draw_post(GoalRules::Bf1, 0, &[], 0.5), None);
}

/// Plan team 0 `n` times, a second apart, without stepping the sim.
fn plans(sim: &mut Sim, n: u32) {
    for _ in 0..n {
        sim.tick += PLAN_PERIOD;
        plan(sim, 0);
    }
}

/// A unit keeps the post it drew: sixty plans with enemies about, posts
/// to choose from and nothing done change no target.
#[test]
fn a_target_is_kept_from_plan_to_plan() {
    let mut sim = world(&[Some(1), Some(1), Some(1)]);
    // Off every post, so nobody is "at" its target.
    for (k, u) in sim.units.iter_mut().enumerate() {
        u.soldier.pos = Vec3::new(20.0, 0.0, 30.0 + k as f32);
    }
    plan(&mut sim, 0);
    let first = assignments(&sim, 0);
    let changes = sim.nav_stats.assignment_changes;
    assert_eq!(changes, 4, "one draw per unit");
    // An enemy walks onto one of the targets: nothing in the plan looks.
    let n = sim.units.len() - 1;
    sim.units[n].soldier.pos = Vec3::new(0.0, 0.0, 1.0);
    plans(&mut sim, 60);
    assert_eq!(assignments(&sim, 0), first);
    assert_eq!(sim.nav_stats.assignment_changes, changes);
}

/// A respawned slot holds no target: the new life draws.
#[test]
fn a_new_life_draws_again() {
    let mut sim = world(&[Some(1), Some(1), Some(1)]);
    let held = vec![Some(Objective::Attack(0)); sim.units.len()];
    let gens: Vec<u32> = sim.units.iter().map(|u| u.gen).collect();
    sim.commander[0].assignment = held.clone();
    sim.commander[0].gens = gens.clone();
    plans(&mut sim, 1);
    assert_eq!(assignments(&sim, 0)[..4], held[..4], "the same lives keep");
    sim.commander[0].gens = gens.iter().map(|g| g + 1).collect();
    plans(&mut sim, 1);
    assert_ne!(assignments(&sim, 0)[..4], held[..4], "new lives draw");
    assert_eq!(sim.nav_stats.assignment_changes, 4);
}

/// BF2: a captured target is held (`Defend`) by whoever targeted it. A
/// unit inside its capture region leaves 5 s after arriving; one with a
/// fight on its hands, or outside the region, stays; and a post worth 0
/// to defend sends its holders on at once.
#[test]
fn bf2_holds_a_captured_target_until_five_seconds_at_the_post() {
    let fixture = |def: i32| {
        let mut sim = world(&[Some(1), Some(1), Some(1)]);
        crowd(&mut sim);
        sim.cps[1].def.value_def = [def, def];
        let mut held = vec![Some(Objective::Attack(1)); 5];
        held.resize(sim.units.len(), Some(Objective::Attack(0)));
        sim.commander[0].assignment = held;
        // Units 0-2 stand on cp1 (x = 40); unit 3 is on its way.
        sim.units[3].soldier.pos = Vec3::new(20.0, 0.0, 30.0);
        sim.cps[1].owner = Some(0);
        sim
    };
    let mut sim = fixture(10);
    // Unit 2 has an enemy to fight.
    let enemy = sim.handle(4);
    sim.units[2].ai.target = Some(enemy.into());
    plans(&mut sim, 4);
    assert_eq!(
        assignments(&sim, 0)[..4],
        [Some(Objective::Defend(1)); 4],
        "held, and relabelled"
    );
    assert_eq!(
        sim.nav_stats.assignment_changes, 0,
        "a relabel is no change"
    );
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    assert!(
        a[..2]
            .iter()
            .all(|o| matches!(o, Some(Objective::Attack(0 | 2)))),
        "5 s at the post: on to an enemy post: {a:?}"
    );
    assert_eq!(a[2..4], [Some(Objective::Defend(1)); 2], "{a:?}");
    // The fight over, unit 2's five seconds start.
    sim.units[2].ai.target = None;
    plans(&mut sim, 4);
    assert_eq!(assignments(&sim, 0)[2], Some(Objective::Defend(1)));
    plans(&mut sim, 1);
    assert!(matches!(
        assignments(&sim, 0)[2],
        Some(Objective::Attack(0 | 2))
    ));

    // Value_DEF 0: every holder draws at the next plan, wherever it is.
    let mut sim = fixture(0);
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    assert!(
        a[..4]
            .iter()
            .all(|o| matches!(o, Some(Objective::Attack(0 | 2)))),
        "{a:?}"
    );
}

/// The 5 s test runs once per stay: a unit that arrived more than 5 s
/// before the capture stays on as the post's defender (card DEFS: while
/// the post is threatened - here by an enemy 25 m off).
#[test]
fn a_capturer_that_passed_its_test_stays() {
    let mut sim = world(&[Some(1), Some(1), Some(1)]);
    enemy_near_cp1(&mut sim, 25.0);
    sim.commander[0].assignment = vec![Some(Objective::Attack(1)); 5];
    plans(&mut sim, 8);
    assert_eq!(assignments(&sim, 0)[0], Some(Objective::Attack(1)));
    assert_eq!(sim.commander[0].at_post[0], AT_POST_TESTED);
    sim.cps[1].owner = Some(0);
    plans(&mut sim, 30);
    assert_eq!(assignments(&sim, 0)[0], Some(Objective::Defend(1)));
    // Away from the post and back: a new stay, a new test.
    sim.units[0].soldier.pos = Vec3::new(20.0, 0.0, 30.0);
    plans(&mut sim, 1);
    sim.units[0].soldier.pos = Vec3::new(40.0, 0.0, 0.0);
    plans(&mut sim, 5);
    assert!(matches!(
        assignments(&sim, 0)[0],
        Some(Objective::Attack(0 | 2))
    ));
}

/// BF1 (`FUN_00261f88`): when a target becomes the team's, every unit
/// holding it draws at once — nobody stays as a defender.
#[test]
fn bf1_redraws_every_holder_when_the_target_is_taken() {
    let mut sim = world(&[Some(1), Some(1), Some(1)]);
    sim.ground_rules = crate::soldier::GroundRules::Bf1;
    sim.commander[0].assignment = vec![Some(Objective::Attack(1)); 5];
    sim.units[3].soldier.pos = Vec3::new(20.0, 0.0, 30.0);
    plans(&mut sim, 3);
    assert_eq!(assignments(&sim, 0)[..4], [Some(Objective::Attack(1)); 4]);
    sim.cps[1].owner = Some(0);
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    assert!(
        a[..4]
            .iter()
            .all(|o| matches!(o, Some(Objective::Attack(0 | 2)))),
        "{a:?}"
    );
}

/// BF1's retake call (`FUN_00262090`): a post the team lost to neutral
/// takes the team's AI within 60 m — five of them at most.
#[test]
fn bf1_sends_up_to_five_nearby_units_to_a_post_lost_to_neutral() {
    let mut sim = world(&[Some(1), Some(0), Some(1)]);
    sim.ground_rules = crate::soldier::GroundRules::Bf1;
    // Three more of team 0 beside the first three; unit 3 is 100 m off.
    for k in 0..3 {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(42.0, 0.0, k as f32 * 2.0))
        })
        .expect("spawned");
    }
    sim.units[3].soldier.pos = Vec3::new(40.0, 0.0, 100.0);
    sim.commander[0].assignment = vec![Some(Objective::Attack(0)); 8];
    plans(&mut sim, 1);
    assert!(assignments(&sim, 0)
        .iter()
        .all(|o| *o != Some(Objective::Attack(1))));
    sim.cps[1].owner = None;
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    let sent: Vec<usize> = (0..a.len())
        .filter(|&i| a[i] == Some(Objective::Attack(1)))
        .collect();
    assert_eq!(sent, vec![0, 1, 2, 5, 6], "five, in index order: {a:?}");
    // The call is the event's: the next plan sends nobody new.
    plans(&mut sim, 1);
    assert_eq!(assignments(&sim, 0), a);
}

/// BF1's at-post helper (`FUN_00255fd0`): 5 s inside an own target's
/// region the unit draws again WITHOUT that post. Here the only other
/// post is worth 0, so the draw is the uniform fallback over the open
/// posts: with the exclusion every unit in the region leaves, without
/// it some stay.
#[test]
fn bf1_excludes_the_post_it_stood_five_seconds_on() {
    let mut sim = world(&[Some(1), Some(0)]);
    // Card DEFS: a team of sixteen and a threatened post, so the cap
    // (four) takes none of the four away.
    crowd(&mut sim);
    enemy_near_cp1(&mut sim, 25.0);
    sim.ground_rules = crate::soldier::GroundRules::Bf1;
    sim.cps[0].def.value_atk = [0, 0];
    sim.commander[0].assignment = vec![Some(Objective::Defend(1)); 5];
    plans(&mut sim, 4);
    assert_eq!(assignments(&sim, 0)[..4], [Some(Objective::Defend(1)); 4]);
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    assert_eq!(a[..3], [Some(Objective::Attack(0)); 3], "{a:?}");
    assert_eq!(a[3], Some(Objective::Defend(1)), "on the region's rim");
}

/// A passenger takes its AI driver's target (BF2 `FUN_002e80b0`), and
/// keeps its own when the pilot seat is empty.
#[test]
fn a_passenger_takes_its_drivers_target() {
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let mut sim = world(&[Some(1), Some(1), Some(1)]);
    for (k, u) in sim.units.iter_mut().enumerate() {
        u.soldier.pos = Vec3::new(20.0, 0.0, 30.0 + k as f32);
    }
    let v = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(20.0, 1.0, 30.0),
        yaw: 0.0,
        max_health: 900.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Passenger(0)],
    });
    let (driver, rider) = (sim.handle(0), sim.handle(1));
    assert!(sim.seat_unit(rider, v, 1));
    let mut held = vec![Some(Objective::Attack(0)); 5];
    held[1] = Some(Objective::Attack(2));
    sim.commander[0].assignment = held.clone();
    plans(&mut sim, 1);
    assert_eq!(assignments(&sim, 0)[..4], held[..4], "no driver: kept");
    assert!(sim.seat_unit(driver, v, 0));
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    assert_eq!(a[..4], [Some(Objective::Attack(0)); 4], "{a:?}");
}

/// Two graphs that do not join: hubs 0 - 1 and 2 - 3, a post at 0, 1 and
/// 2 and the four AI of team 0 at hub 1.
fn split_world(owners: [Option<u8>; 3]) -> Sim {
    let mut sim = world(&[]);
    let hub = |x: f32, arc: u16| Hub {
        pos: Vec3::new(x, 0.0, 0.0),
        radius: 2.0,
        arcs: vec![arc],
    };
    let arc = |a: u16, b: u16| Arc {
        a,
        b,
        type_mask: 1,
        flags: 0,
    };
    sim.nav = NavGraph::new(
        vec![hub(0.0, 0), hub(40.0, 0), hub(80.0, 1), hub(120.0, 1)],
        vec![arc(0, 1), arc(2, 3)],
        RouteTable::empty(4),
        Vec::new(),
        Vec::new(),
    );
    for (i, o) in owners.iter().enumerate() {
        sim.add_cp(post(&format!("cp{i}"), i as f32 * 40.0, 10), *o);
    }
    for (k, u) in sim.units.iter_mut().enumerate() {
        u.soldier.pos = Vec3::new(40.0, 0.0, 10.0 + k as f32);
    }
    sim
}

/// The reach drop: a post the unit has no route to is not offered.
#[test]
fn a_post_with_no_route_to_it_is_not_drawn() {
    let mut sim = split_world([Some(1), Some(0), Some(1)]);
    // Eight first draws per unit, each with another random number.
    for _ in 0..8 {
        sim.commander[0].assignment.clear();
        plans(&mut sim, 1);
        let a = assignments(&sim, 0);
        assert_eq!(a[..4], [Some(Objective::Attack(0)); 4], "{a:?}");
    }
}

/// No route to the target it holds: the unit draws again at the next
/// plan (BF2's draw tests the route; a PORT GUARD on a BF1 map).
#[test]
fn a_target_with_no_route_to_it_is_redrawn() {
    let mut sim = split_world([Some(1), Some(0), Some(1)]);
    sim.commander[0].assignment = vec![Some(Objective::Attack(2)); 5];
    plans(&mut sim, 1);
    let a = assignments(&sim, 0);
    assert_eq!(a[..4], [Some(Objective::Attack(0)); 4], "{a:?}");
    assert_eq!(sim.nav_stats.assignment_changes, 4);
}

/// In the endgame nobody holds a post: a holder draws an enemy post, or
/// hunts when there is none it may take.
#[test]
fn in_the_endgame_nobody_holds_a_post() {
    let mut sim = world(&[Some(0), Some(0), Some(1)]);
    crowd(&mut sim);
    sim.cps[2].def.value_atk = [0, 0];
    sim.commander[0].assignment = vec![Some(Objective::Defend(1)); 5];
    plans(&mut sim, 3);
    assert_eq!(assignments(&sim, 0)[..4], [Some(Objective::Defend(1)); 4]);
    sim.battle = crate::battle::Battle::new([crate::ai::HOLD_ENDGAME, 100]);
    plans(&mut sim, 1);
    assert_eq!(assignments(&sim, 0)[..4], [Some(Objective::Hunt); 4]);
    sim.cps[2].def.value_atk = [10, 10];
    plans(&mut sim, 1);
    assert_eq!(assignments(&sim, 0)[..4], [Some(Objective::Attack(2)); 4]);
}

/// With nothing left to take the team HUNTS (the port's endgame), and
/// nobody hunts while a post is still takeable.
#[test]
fn hunt_when_there_is_nothing_to_attack() {
    let mut sim = world(&[Some(0), Some(0), Some(0)]);
    plan(&mut sim, 0);
    let a = assignments(&sim, 0);
    assert_eq!(a[..4], [Some(Objective::Hunt); 4], "{a:?}");
    let mut sim = world(&[Some(1), Some(0), Some(0)]);
    plan(&mut sim, 0);
    let b = assignments(&sim, 0);
    assert_eq!(b[..4], [Some(Objective::Attack(0)); 4], "{b:?}");
}

/// Units on the same objective standing together are a squad; one on its
/// own is not.
#[test]
fn units_together_on_one_objective_form_a_squad() {
    let mut sim = world(&[Some(1), None, Some(1)]);
    // Unit 3 is sent far away from the other three.
    sim.units[3].soldier.pos = Vec3::new(40.0, 0.0, 300.0);
    plan(&mut sim, 0);
    let a = assignments(&sim, 0);
    let squads: Vec<Option<u32>> = sim.units.iter().map(|u| u.ai.squad).collect();
    for i in 0..3 {
        if a[i] == a[(i + 1) % 3] {
            assert_eq!(
                squads[i],
                a[i].map(Objective::key),
                "unit {i} shares its objective with a neighbour"
            );
        }
    }
    assert_eq!(squads[3], None, "the lone unit has no squad");
}

#[test]
fn objective_bytes_and_keys_are_distinct() {
    let all = [
        Objective::Attack(0),
        Objective::Attack(1),
        Objective::Defend(0),
        Objective::Defend(1),
        Objective::Hunt,
    ];
    let mut keys: Vec<u32> = all.iter().map(|o| o.key()).collect();
    let mut bytes: Vec<[u8; 5]> = all.iter().map(|o| o.bytes()).collect();
    keys.sort_unstable();
    keys.dedup();
    bytes.sort_unstable();
    bytes.dedup();
    assert_eq!(keys.len(), all.len());
    assert_eq!(bytes.len(), all.len());
    assert_eq!(Objective::Attack(3).cp(), Some(3));
    assert_eq!(Objective::Hunt.cp(), None);
    assert_eq!(Objective::Defend(2).label(), "Defend cp2");
}

#[test]
fn the_draws_number_is_uniform_and_varies_per_unit_and_plan() {
    let mut bins = [0u32; 10];
    for u in 0..32u32 {
        for p in 0..100u64 {
            let x = roll(u, p * PLAN_PERIOD, 1);
            assert!((0.0..1.0).contains(&x), "{x}");
            bins[(x * 10.0) as usize] += 1;
        }
    }
    assert!(
        bins.iter().all(|&b| (250..=390).contains(&b)),
        "3,200 numbers over ten bins: {bins:?}"
    );
    // Deterministic.
    assert_eq!(roll(3, 90, 1), roll(3, 90, 1));
    assert_ne!(roll(3, 90, 1), roll(3, 120, 1));
    assert_ne!(roll(3, 90, 1), roll(4, 90, 1));
    assert_ne!(roll(3, 90, 1), roll(3, 90, 2));
}

/// Card DEFS's scene: [`world`] with cp1 the enemy's and sixteen AI of
/// team 0 - slots 0-3 are `world`'s (at x = 40, z = 0 / 2 / 4 / 6), slots
/// 5-8 stand 11-14 m from cp1 (outside its 6 m region, so the PS2's 5 s
/// at-post test never runs for them) and slots 9-16 far off at cp0 - the
/// eight near cp1 all targeting it. One plan is made, then cp1 is TAKEN:
/// the next plan sees the capture.
fn defs_world() -> Sim {
    let mut sim = world(&[Some(1), Some(1), Some(1)]);
    for k in 0..12 {
        let at = match k {
            0..=3 => Vec3::new(40.0, 0.0, -11.0 - k as f32),
            _ => Vec3::new(0.0, 0.0, 20.0 + 2.0 * k as f32),
        };
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, at)
        })
        .expect("spawned");
    }
    let mut a = vec![Some(Objective::Attack(0)); sim.units.len()];
    for i in [0, 1, 2, 3, 5, 6, 7, 8] {
        a[i] = Some(Objective::Attack(1));
    }
    sim.commander[0].assignment = a;
    sim.commander[0].gens = sim.units.iter().map(|u| u.gen).collect();
    plans(&mut sim, 1);
    sim.cps[1].owner = Some(0);
    sim
}

fn defenders(sim: &Sim, cp: u32) -> Vec<usize> {
    (0..sim.units.len())
        .filter(|&i| sim.commander[0].of(i as u32) == Some(Objective::Defend(cp)))
        .collect()
}

/// An enemy that stands `d` metres from cp1 and thinks nothing.
fn enemy_near_cp1(sim: &mut Sim, d: f32) {
    sim.spawn(&UnitSpec::trooper(1, Vec3::new(40.0 + d, 0.0, 0.0)))
        .expect("enemy");
}

/// Card MAIS: a team the mission makes less aggressive
/// (`SetTeamAggressiveness` 0.7, BF1 and BF2 Endor's Imperials) keeps
/// more of itself on a threatened post: 0.25 / 0.7 of sixteen is six, so
/// the six nearest of the eight that took it hold it. 1.0 is the old
/// quarter.
#[test]
fn a_less_aggressive_team_keeps_more_defenders() {
    assert_eq!(defend_share(1.0), DEFEND_SHARE);
    assert!((defend_share(0.7) - 0.25 / 0.7).abs() < 1e-6);
    assert_eq!(defend_share(0.1), 1.0, "never over the whole team");
    assert_eq!(defend_cap(16, 0, defend_share(0.7)), 6);
    let mut sim = defs_world();
    sim.units[0].soldier.pos = Vec3::new(40.0, 0.0, 20.0);
    sim.units[1].soldier.pos = Vec3::new(40.0, 0.0, 21.0);
    sim.mission_ai.aggressiveness[0] = 0.7;
    plans(&mut sim, 1);
    assert_eq!(defenders(&sim, 1), [2, 3, 5, 6, 7, 8]);
}

/// Card MAIS (OURS): a post the team's bots may not capture is not drawn
/// to attack; one they hold still is to defend.
#[test]
fn a_post_barred_to_the_teams_bots_is_not_drawn_to_take() {
    let mut sim = defs_world();
    // cp0 and cp2 are the enemy's: bar both to team 0's bots.
    sim.cps[0].ai_capture_ban = 1;
    sim.cps[2].ai_capture_ban = 1;
    sim.units[0].soldier.pos = Vec3::new(40.0, 0.0, 20.0);
    sim.units[1].soldier.pos = Vec3::new(40.0, 0.0, 21.0);
    plans(&mut sim, 1);
    for i in [0, 1, 7, 8] {
        assert!(
            !matches!(sim.commander[0].of(i), Some(Objective::Attack(0 | 2))),
            "unit {i} drew a barred post: {:?}",
            sim.commander[0].of(i)
        );
    }
    assert_eq!(defenders(&sim, 1), [2, 3, 5, 6], "its own post is held");
}

/// Card DEFS, rule 2: of the eight that targeted the post, a quarter of
/// the team's sixteen living units hold it - the four NEAREST - and the
/// other four have another post at once.
#[test]
fn a_taken_post_keeps_a_quarter_of_the_team_and_the_rest_go_on_at_once() {
    let mut sim = defs_world();
    // Slots 0 and 1 stand 20 m off: the nearest four are 2, 3, 5 and 6.
    sim.units[0].soldier.pos = Vec3::new(40.0, 0.0, 20.0);
    sim.units[1].soldier.pos = Vec3::new(40.0, 0.0, 21.0);
    plans(&mut sim, 1);
    assert_eq!(defend_cap(16, 0, DEFEND_SHARE), 4);
    assert_eq!(defenders(&sim, 1), [2, 3, 5, 6], "the four nearest hold it");
    for i in [0, 1, 7, 8] {
        assert!(
            matches!(sim.commander[0].of(i), Some(Objective::Attack(0 | 2))),
            "unit {i} is over the cap: {:?}",
            sim.commander[0].of(i)
        );
    }
    // The share is of the LIVING team, and never under one defender.
    assert_eq!(defend_cap(17, 0, DEFEND_SHARE), 4);
    assert_eq!(defend_cap(22, 0, DEFEND_SHARE), 6);
    assert_eq!(defend_cap(1, 0, DEFEND_SHARE), 1);
    // With eight of the team dead the cap is two.
    let mut sim = defs_world();
    for i in 9..17 {
        sim.units[i].alive = false;
    }
    plans(&mut sim, 1);
    assert_eq!(defenders(&sim, 1), [0, 1]);
    // A DRAW that lands on an own post with its defenders is made again
    // without it: here the enemy's post is worth nothing, so BF2 draws
    // among its own - four take cp1 and the rest go at cp0 after all.
    let mut sim = world(&[Some(1), Some(0)]);
    crowd(&mut sim);
    sim.cps[0].def.value_atk = [0, 0];
    plans(&mut sim, 1);
    assert_eq!(defenders(&sim, 1), [0, 1, 2, 3]);
    for i in 5..17 {
        assert_eq!(sim.commander[0].of(i), Some(Objective::Attack(0)));
    }
}

/// Card DEFS, rule 1: of four defenders two have the post's two Cover
/// hints, one each with its stance, kept from plan to plan, and the other
/// two have none; stepped, the two walk to their hints and crouch, the
/// others to a point of the capture region each, standing.
#[test]
fn every_defender_has_its_own_spot_and_only_a_hint_asks_for_a_crouch() {
    let mut sim = defs_world();
    sim.nav.as_mut().expect("graph").hints = vec![
        hint("a", HINT_COVER, 49.0, MASK_CROUCH),
        hint("b", HINT_COVER, 31.0, MASK_CROUCH),
    ];
    // Out of the region, so nobody's 5 s test runs while they walk.
    for i in 0..4 {
        sim.units[i].soldier.pos = Vec3::new(40.0, 0.0, 6.5 + i as f32);
    }
    plans(&mut sim, 1);
    let ds = defenders(&sim, 1);
    assert_eq!(ds, [0, 1, 2, 3]);
    let spots: Vec<(Vec3, u8)> = (0..sim.units.len())
        .filter_map(|i| sim.commander[0].spot(i as u32))
        .collect();
    assert_eq!(spots.len(), 2, "one defender a hint: {spots:?}");
    assert!(crate::nav::xz_dist(spots[0].0, spots[1].0) >= DEFEND_SPACING);
    assert_eq!(
        spots.iter().map(|s| s.1).collect::<Vec<_>>(),
        [MASK_CROUCH; 2],
        "the hints' own stance"
    );
    // A hint is one defender's: a third asking has none left.
    let taken: Vec<Vec3> = spots.iter().map(|s| s.0).collect();
    assert_eq!(
        pick_spot(&sim, 1, 2, sim.units[2].soldier.pos, &taken),
        None
    );
    assert!(pick_spot(&sim, 1, 2, sim.units[2].soldier.pos, &taken[..1]).is_some());
    // Kept: moved so that each stands nearer the OTHER's hint, each still
    // has its own at the next plan.
    let holders: Vec<usize> = ds
        .iter()
        .copied()
        .filter(|&i| sim.commander[0].spot(i as u32).is_some())
        .collect();
    let before: Vec<_> = holders
        .iter()
        .map(|&i| sim.commander[0].spot(i as u32))
        .collect();
    let home: Vec<Vec3> = holders.iter().map(|&i| sim.units[i].soldier.pos).collect();
    for (k, &i) in holders.iter().enumerate() {
        let other = before[1 - k].expect("a spot").0;
        sim.units[i].soldier.pos = Vec3::new(other.x, 0.0, 5.0);
    }
    plans(&mut sim, 1);
    let after: Vec<_> = holders
        .iter()
        .map(|&i| sim.commander[0].spot(i as u32))
        .collect();
    assert_eq!(after, before, "a defender keeps its spot");
    for (k, &i) in holders.iter().enumerate() {
        sim.units[i].soldier.pos = home[k];
    }
    sim.commander[0].threat[1] = sim.tick;
    // Stepped (4 s, inside the capture's own 5 s): the two on a hint
    // stand on it, kept, and crouch; the two without walk into the
    // capture region and stand; no two of the four are in one place.
    for _ in 0..120 {
        sim.step(&[crate::sim::PadState::default()]);
    }
    assert_eq!(defenders(&sim, 1), ds);
    for &i in &ds {
        let u = &sim.units[i];
        match sim.commander[0].spot(i as u32) {
            Some(s) => {
                assert!(spots.contains(&s), "kept");
                assert!(
                    crate::nav::xz_dist(u.soldier.pos, s.0) < 1.0,
                    "unit {i} at {:?}, its spot {:?}",
                    u.soldier.pos,
                    s.0
                );
                assert_eq!(u.soldier.stance, Stance::Crouch, "unit {i}");
            }
            None => {
                assert!(sim.cps[1].def.capture.contains(u.soldier.pos), "unit {i}");
                assert_eq!(
                    u.soldier.stance,
                    Stance::Stand,
                    "unit {i}: no hint, no crouch"
                );
            }
        }
        for &j in ds.iter().filter(|&&j| j > i) {
            let d = crate::nav::xz_dist(u.soldier.pos, sim.units[j].soldier.pos);
            assert!(d >= 1.5, "units {i} and {j} are {d} m apart");
        }
    }
}

/// Card DEFS, rule 3: five seconds without a threat or a capture and the
/// post keeps ONE defender; an enemy inside the threat radius - a fifth
/// of the distance between the map's two farthest posts - keeps the
/// share, and brings it back when it comes.
#[test]
fn a_quiet_post_keeps_one_defender_and_a_threatened_one_its_share() {
    // Quiet: the 300 m enemy threatens nothing.
    let mut sim = defs_world();
    for i in 0..4 {
        sim.units[i].soldier.pos = Vec3::new(40.0, 0.0, 6.5 + i as f32);
    }
    plans(&mut sim, 5);
    assert_eq!(defenders(&sim, 1), [0, 1, 2, 3], "4 s after the capture");
    plans(&mut sim, 1);
    assert_eq!(defenders(&sim, 1), [0], "5 s: one, the nearest");
    assert_eq!(defend_cap(16, DEFEND_QUIET_FALL - 1, DEFEND_SHARE), 4);
    assert_eq!(
        defend_cap(16, DEFEND_QUIET_FALL, DEFEND_SHARE),
        DEFEND_QUIET_CAP
    );

    // Threatened: an enemy 25 m from the post (the radius here is the
    // floor, 30 m: the posts span 80 m).
    let mut sim = defs_world();
    for i in 0..4 {
        sim.units[i].soldier.pos = Vec3::new(40.0, 0.0, 6.5 + i as f32);
    }
    assert_eq!(threat_radius(&sim), THREAT_RADIUS_MIN);
    enemy_near_cp1(&mut sim, 25.0);
    plans(&mut sim, 40);
    assert_eq!(defenders(&sim, 1), [0, 1, 2, 3], "held while threatened");
    assert_eq!(sim.commander[0].threat[1], sim.tick);

    // The radius is the map's: at 35 m the same enemy is no threat on
    // this map, and is one where the farthest posts are 400 m apart.
    let mut sim = defs_world();
    enemy_near_cp1(&mut sim, 35.0);
    let r = threat_radius(&sim);
    assert!(!post_threatened(&sim, 0, 1, r));
    sim.cps[2].def.pos.x = 400.0;
    let r = threat_radius(&sim);
    assert_eq!(r, THREAT_FRAC * 400.0);
    assert!(post_threatened(&sim, 0, 1, r));
    // A team-mate there with a target is a threat too; one without is not.
    let mut sim = defs_world();
    assert!(!post_threatened(&sim, 0, 1, 30.0));
    let enemy = sim.handle(4);
    sim.units[0].ai.target = Some(enemy.into());
    assert!(post_threatened(&sim, 0, 1, 30.0));
    assert!(!post_threatened(&sim, 0, 0, 30.0), "not cp0, 40 m off");
}

/// Card DEFS, rule 4: twenty seconds quiet and the last defender draws
/// another post too, though it never stood in the capture region (the
/// PS2's 5 s test did not run for it) - and not while the post is
/// threatened.
#[test]
fn the_last_defender_of_a_quiet_post_goes_on_after_twenty_seconds() {
    let fixture = || {
        let mut sim = defs_world();
        for i in 0..4 {
            sim.units[i].soldier.pos = Vec3::new(40.0, 0.0, 6.5 + i as f32);
        }
        sim
    };
    let mut sim = fixture();
    plans(&mut sim, 20);
    assert_eq!(defenders(&sim, 1), [0], "19 s after the capture");
    assert_eq!(sim.commander[0].at_post[0], 0, "never in the region");
    plans(&mut sim, 1);
    assert_eq!(defenders(&sim, 1), [] as [usize; 0], "20 s: nobody");
    assert!(matches!(
        sim.commander[0].of(0),
        Some(Objective::Attack(0 | 2))
    ));
    assert_eq!(
        defend_cap(16, DEFEND_QUIET_LEAVE - 1, DEFEND_SHARE),
        DEFEND_QUIET_CAP
    );
    assert_eq!(defend_cap(16, DEFEND_QUIET_LEAVE, DEFEND_SHARE), 0);
    // Threatened all the while: the four are still there.
    let mut sim = fixture();
    enemy_near_cp1(&mut sim, 25.0);
    plans(&mut sim, 30);
    assert_eq!(defenders(&sim, 1), [0, 1, 2, 3]);
}

/// The hash must separate "no unit there" from "a unit with no objective",
/// and must move for every field.
#[test]
fn the_plan_hash_covers_every_field() {
    let h = |p: &TeamPlan| {
        let mut acc: u64 = 0xCBF2_9CE4_8422_2325;
        p.hash_into(&mut |b: &[u8]| {
            for &x in b {
                acc ^= u64::from(x);
                acc = acc.wrapping_mul(0x1_0000_01B3);
            }
        });
        acc
    };
    let base = TeamPlan {
        assignment: vec![Some(Objective::Attack(1)), None],
        gens: vec![1, 0],
        at_post: vec![2, 0],
        owners: vec![Some(0), None],
        spots: vec![Some((Vec3::new(1.0, 2.0, 3.0), MASK_CROUCH)), None],
        threat: vec![90, 0],
        next_plan_tick: 30,
    };
    let mut tick = base.clone();
    tick.next_plan_tick = 60;
    let mut shorter = base.clone();
    shorter.assignment.pop();
    let mut other = base.clone();
    other.assignment[0] = Some(Objective::Defend(1));
    let mut none = base.clone();
    none.assignment[0] = None;
    let mut swapped = base.clone();
    swapped.assignment.swap(0, 1);
    let mut gen = base.clone();
    gen.gens[0] = 2;
    let mut clock = base.clone();
    clock.at_post[0] = AT_POST_TESTED;
    let mut owner = base.clone();
    owner.owners[1] = Some(1);
    let mut spot = base.clone();
    spot.spots[0] = Some((Vec3::new(1.0, 2.0, 3.5), MASK_CROUCH));
    let mut mask = base.clone();
    mask.spots[0] = Some((Vec3::new(1.0, 2.0, 3.0), 0));
    let mut no_spot = base.clone();
    no_spot.spots[0] = None;
    let mut threat = base.clone();
    threat.threat[1] = 90;
    for v in [
        &tick, &shorter, &other, &none, &swapped, &gen, &clock, &owner, &spot, &mask, &no_spot,
        &threat,
    ] {
        assert_ne!(h(&base), h(v), "{v:?}");
    }
}
