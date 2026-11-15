use super::*;
use crate::ai::tokn::{deal, holds};
use crate::collision::{Body, Shape};
use crate::perception::Contact;
use crate::sim::PadState;
use crate::unit::{Controller, UnitSpec};
use crate::weapon::WeaponClass;

/// A flat field at tier `d`: a human of team 0 at the origin and enemy bots
/// (team 1, facing him) at `dists` metres north of him, no target yet. The
/// human's spawn grace is over.
fn field(d: Difficulty, dists: &[f32]) -> (Sim, UnitHandle, Vec<UnitHandle>) {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.units.clear();
    sim.difficulty = d;
    let human = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).expect("human");
    let bots = dists.iter().map(|&z| bot(&mut sim, z, 0)).collect();
    past_grace(&mut sim);
    (sim, human, bots)
}

/// An enemy bot `z` m north of the origin, facing it, with weapon `class`.
fn bot(sim: &mut Sim, z: f32, class: usize) -> UnitHandle {
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        weapon_class: class,
        yaw: std::f32::consts::PI,
        ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -z))
    })
    .expect("bot")
}

fn past_grace(sim: &mut Sim) {
    upkeep(sim);
    sim.tick += GRACE;
}

/// `bot` holds a record of `h`: seen at its last think, or heard / told.
fn record(sim: &mut Sim, bot: UnitHandle, h: UnitHandle, seen: bool, told: bool) {
    let (tick, at) = (sim.tick, sim.unit(h).expect("alive").capsule().center());
    sim.unit_mut(bot).expect("alive").ai.contacts[0] = Some(Contact {
        handle: h.into(),
        first_seen: tick,
        last_seen: tick,
        last_pos: at,
        seen_now: seen,
        heard: !seen,
        told,
    });
}

fn sees(sim: &mut Sim, bot: UnitHandle, h: UnitHandle) {
    record(sim, bot, h, true, false);
}

fn holders(sim: &Sim) -> Vec<UnitHandle> {
    let mut v: Vec<UnitHandle> = sim.tokens.iter().map(|t| t.bot).collect();
    v.sort_by_key(|h| h.index);
    v
}

/// Only Hard and Elite focus: at Easy and Normal a bot that sees the human
/// but shoots at nobody takes no token; at Hard the nearest five of eight
/// that see him take them, at Elite seven.
#[test]
fn only_hard_and_elite_focus_up_to_the_tiers_tokens() {
    let dists = [8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0, 36.0];
    for (d, n) in [
        (Difficulty::Easy, 0),
        (Difficulty::Normal, 0),
        (Difficulty::Hard, 5),
        (Difficulty::Elite, 7),
    ] {
        assert_eq!(focuses(d), n > 0, "{d:?}");
        let (mut sim, human, bots) = field(d, &dists);
        for &b in &bots {
            sees(&mut sim, b, human);
        }
        deal(&mut sim);
        assert_eq!(holders(&sim), bots[..n].to_vec(), "{d:?}");
        assert!(sim.tokens.iter().all(|t| t.human == human));
    }
}

/// A shot heard or a friend's word makes a record, but only the bot's own
/// eyes let it focus.
#[test]
fn a_heard_or_told_human_is_not_focused_until_seen() {
    let (mut sim, human, bots) = field(Difficulty::Hard, &[10.0, 20.0]);
    record(&mut sim, bots[0], human, false, false);
    record(&mut sim, bots[1], human, false, true);
    deal(&mut sim);
    assert!(sim.tokens.is_empty());
    sees(&mut sim, bots[0], human);
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![bots[0]]);
}

/// No focus in his first three seconds after a spawn.
#[test]
fn no_focus_in_the_spawn_grace() {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.units.clear();
    sim.difficulty = Difficulty::Hard;
    let human = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).expect("human");
    let b = bot(&mut sim, 10.0, 0);
    sees(&mut sim, b, human);
    deal(&mut sim);
    assert!(sim.tokens.is_empty(), "just spawned");
    sim.tick += GRACE - 1;
    deal(&mut sim);
    assert!(sim.tokens.is_empty(), "still in grace");
    sim.tick += 1;
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![b]);
    assert_eq!(sim.focus_born.len(), 1);
}

/// Beyond the reach no focus; a scoped rifle reaches farther.
#[test]
fn the_reach_is_the_weapons_and_a_scope_reaches_farther() {
    let at = |z: f32, scoped: bool| {
        let (mut sim, human, _) = field(Difficulty::Hard, &[]);
        let class = if scoped {
            sim.add_weapon_class(WeaponClass {
                zoom_max: 8.0,
                trigger_single: true,
                yaw_spread: 0.0,
                ..WeaponClass::bf1_rifle()
            })
        } else {
            0
        };
        let b = bot(&mut sim, z, class);
        sees(&mut sim, b, human);
        deal(&mut sim);
        holds(&sim, b, human)
    };
    assert!(at(FOOT_RANGE - 2.0, false));
    assert!(!at(FOOT_RANGE + 2.0, false), "beyond the reach on foot");
    assert!(at(FOOT_RANGE + 2.0, true), "a scope reaches past it");
    assert!(at(SCOPED_RANGE - 2.0, true));
    assert!(!at(SCOPED_RANGE + 2.0, true));
}

/// A human in a vehicle is focused by a bot whose weapons hurt it, within
/// the vehicle reach, never by a rifleman; a bot with only heavy weapons
/// does not focus a human on foot.
#[test]
fn the_weapons_decide_vehicle_or_soldier() {
    use crate::ai::arsenal::tests::{heavy_scene, launcher, manned, rifle};
    use crate::vehicle::VehicleKind;
    let vs_tank = |first: WeaponClass, second: WeaponClass, z: f32| {
        let (mut sim, b, _) = heavy_scene(first, second);
        sim.difficulty = Difficulty::Hard;
        let (_, rider) = manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -z));
        sim.units[rider.index as usize].is_player = true;
        past_grace(&mut sim);
        sees(&mut sim, b, rider);
        deal(&mut sim);
        holds(&sim, b, rider)
    };
    assert!(vs_tank(launcher(), rifle(), 60.0), "a launcher at 60 m");
    assert!(!vs_tank(launcher(), rifle(), VEHICLE_RANGE + 5.0));
    assert!(!vs_tank(rifle(), rifle(), 30.0), "a rifle never");
    let vs_soldier = |first: WeaponClass, second: WeaponClass| {
        let (mut sim, b, _) = heavy_scene(first, second);
        sim.difficulty = Difficulty::Hard;
        let h = sim
            .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, -20.0)))
            .expect("human");
        past_grace(&mut sim);
        sees(&mut sim, b, h);
        deal(&mut sim);
        holds(&sim, b, h)
    };
    assert!(vs_soldier(rifle(), launcher()));
    assert!(!vs_soldier(launcher(), launcher()), "only heavy weapons");
}

/// A bot capturing a post, or defending one under threat, keeps its job.
#[test]
fn a_capturer_and_a_threatened_posts_defender_keep_their_job() {
    let post = Vec3::new(40.0, 0.0, -40.0);
    let try_it = |objective: Objective, owner: Option<u8>, threat: bool| {
        let (mut sim, _) = crate::ai::tests::l_sim();
        sim.units.clear();
        sim.difficulty = Difficulty::Hard;
        sim.cps[0].owner = owner;
        let human = sim
            .spawn(&UnitSpec::player(0, post + Vec3::new(0.0, 0.0, 20.0)))
            .expect("human");
        let b = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(1, post)
            })
            .expect("bot");
        past_grace(&mut sim);
        sim.tick += 100 * DEFEND_QUIET_FALL;
        let plan = &mut sim.commander[1];
        plan.assignment = vec![None; b.index as usize + 1];
        plan.assignment[b.index as usize] = Some(objective);
        plan.threat = vec![if threat { sim.tick } else { 0 }];
        sees(&mut sim, b, human);
        deal(&mut sim);
        holds(&sim, b, human)
    };
    assert!(!try_it(Objective::Attack(0), Some(0), false), "capturing");
    assert!(!try_it(Objective::Attack(0), None, false), "capturing");
    assert!(try_it(Objective::Defend(0), Some(1), false), "a quiet post");
    assert!(!try_it(Objective::Defend(0), Some(1), true), "threatened");
}

/// A focuser that loses sight keeps him for the memory, walking to his last
/// seen point when it is near, then lets go.
#[test]
fn a_focuser_keeps_him_for_the_memory_then_lets_go() {
    let (mut sim, human, bots) = field(Difficulty::Hard, &[20.0]);
    let b = bots[0];
    sees(&mut sim, b, human);
    deal(&mut sim);
    let st = sim.unit(b).expect("alive").ai;
    assert_eq!(pick(&sim, b, &st), Some(human));
    sim.unit_mut(b).expect("alive").ai.target = Some(human.into());
    // Out of sight now: a wall between them.
    if let Some(c) = sim.unit_mut(b).expect("alive").ai.contacts[0].as_mut() {
        c.seen_now = false;
    }
    sim.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
        Shape::Box {
            half: Vec3::new(10.0, 5.0, 0.5),
        },
    ));
    sim.tick += MEMORY;
    deal(&mut sim);
    assert!(holds(&sim, b, human), "kept for the memory");
    let st = sim.unit(b).expect("alive").ai;
    let pos = sim.unit(b).expect("alive").soldier.pos;
    assert!(push(&sim, b, &st, pos).is_some(), "20 m: it pushes");
    assert_eq!(push(&sim, b, &st, pos + Vec3::new(0.0, 0.0, -10.0)), None);
    sim.tick += 1;
    deal(&mut sim);
    assert!(!holds(&sim, b, human), "then let go");
}

/// Two humans: each has his own tokens, a bot takes one of them only, and
/// a bot holding both focuses the seen one first, then the nearer.
#[test]
fn several_humans_each_their_own_and_the_best_seen_first() {
    let (mut sim, h0, bots) = field(Difficulty::Hard, &[10.0, 12.0]);
    let h1 = sim
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(4.0, 0.0, 0.0)))
        .expect("human 2");
    past_grace(&mut sim);
    let tick = sim.tick;
    let rec = |sim: &Sim, h: UnitHandle, seen: bool| Contact {
        handle: h.into(),
        first_seen: tick,
        last_seen: tick,
        last_pos: sim.unit(h).expect("alive").capsule().center(),
        seen_now: seen,
        heard: false,
        told: false,
    };
    let (c0, c1) = (rec(&sim, h0, true), rec(&sim, h1, true));
    for &b in &bots {
        let st = &mut sim.unit_mut(b).expect("alive").ai;
        st.contacts[0] = Some(c0);
        st.contacts[1] = Some(c1);
    }
    deal(&mut sim);
    for &b in &bots {
        let n = sim.tokens.iter().filter(|t| t.bot == b).count();
        assert_eq!(n, 1, "one human a focuser");
    }
    // A bot with tokens of both: the nearer of two seen, else the seen one.
    let b = bots[0];
    sim.tokens.clear();
    for h in [h0, h1] {
        sim.tokens.push(crate::ai::tokn::Token {
            human: h,
            bot: b,
            since: tick,
        });
    }
    let mut st = sim.unit(b).expect("alive").ai;
    assert_eq!(pick(&sim, b, &st), Some(h0), "both seen: the nearer");
    st.contacts[0] = Some(rec(&sim, h0, false));
    assert_eq!(pick(&sim, b, &st), Some(h1), "the seen one first");
}

/// The fellows' bearings: a focuser sees where the others on the same human
/// stand, and nothing when it focuses nobody.
#[test]
fn a_focuser_knows_its_fellows_bearings() {
    let (mut sim, human, bots) = field(Difficulty::Hard, &[10.0, 20.0]);
    sim.unit_mut(bots[1]).expect("alive").soldier.pos = Vec3::new(20.0, 0.0, 0.0);
    for &b in &bots {
        sees(&mut sim, b, human);
    }
    deal(&mut sim);
    let f = fellows(&sim, bots[0], human, Vec3::ZERO);
    assert_eq!(f.len(), 1);
    assert!((f[0] - glam::Vec2::X).length() < 1e-4, "{:?}", f[0]);
    let (sim, human, bots) = field(Difficulty::Normal, &[10.0, 20.0]);
    assert!(fellows(&sim, bots[0], human, Vec3::ZERO).is_empty());
}

/// In play, with real perception: a bot with a nearer enemy soldier in front
/// keeps shooting him at Normal, and at Hard turns to the human it sees -
/// but never through a wall, and (issue #171 lever L1) not while the nearer
/// soldier is at half the human's distance or nearer: then it holds the
/// human's token and keeps the near one.
#[test]
fn in_play_hard_turns_to_the_human_but_not_through_a_wall() {
    let run = |d: Difficulty, wall: bool, decoy_z: f32| {
        let (mut sim, human, bots) = field(d, &[]);
        let b = bot(&mut sim, 40.0, 0);
        let decoy = sim
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -decoy_z)))
            .expect("decoy");
        for h in [human, decoy] {
            let u = sim.unit_mut(h).expect("alive");
            u.health.max = 1e9;
            u.health.cur = 1e9;
        }
        if wall {
            sim.collision.push_body(Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -20.0)),
                Shape::Box {
                    half: Vec3::new(10.0, 5.0, 0.5),
                },
            ));
        }
        let _ = bots;
        for _ in 0..60 {
            sim.step(&[PadState::default()]);
        }
        let u = sim.unit(b).expect("alive");
        (u.ai.target == Some(human.into()), holds(&sim, b, human))
    };
    assert_eq!(run(Difficulty::Normal, false, 37.0), (false, false));
    assert_eq!(run(Difficulty::Hard, false, 37.0), (false, true), "3 m: L1");
    assert_eq!(
        run(Difficulty::Hard, false, 15.0),
        (true, true),
        "25 m of 40"
    );
    assert_eq!(run(Difficulty::Hard, true, 37.0), (false, false), "a wall");
}

#[test]
fn a_proxy_gets_the_same_focus_grace_as_a_player() {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.units.clear();
    sim.players.clear();
    sim.difficulty = Difficulty::Hard;
    let human = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    sim.set_proxy(human, Some(crate::ai::proxy::Strength::Good.profile()));
    let b = bot(&mut sim, 10.0, 0);
    sees(&mut sim, b, human);
    deal(&mut sim);
    assert!(sim.tokens.is_empty(), "spawn grace");
    sim.tick += GRACE;
    sees(&mut sim, b, human);
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![b], "proxy can be focused after grace");
    sim.unit_mut(human).unwrap().alive = false;
    upkeep(&mut sim);
    assert!(sim.focus_born.is_empty(), "dead handles expire");
}

/// Issue #171 lever L1: a Hard or Elite focuser that sees an enemy bot at
/// half the human's distance or nearer lets the focus go (the census FAR
/// test); one farther than that, or a proxy focuser (frozen), keeps it.
#[test]
fn a_focuser_yields_to_a_bot_at_half_the_humans_distance() {
    for d in [Difficulty::Hard, Difficulty::Elite] {
        assert!(yields_at(d), "{d:?}");
        for (bot_z, yields) in [(20.0, true), (18.0, true), (10.0, false), (5.0, false)] {
            let (mut sim, human, bots) = field(d, &[30.0]);
            let b = bots[0];
            // An enemy bot of the focuser: team 0, between them.
            let near = sim
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -bot_z))
                })
                .expect("near bot");
            sees(&mut sim, b, human);
            let (tick, at) = (sim.tick, sim.unit(near).expect("alive").capsule().center());
            sim.unit_mut(b).expect("alive").ai.contacts[1] = Some(Contact {
                handle: near.into(),
                first_seen: tick,
                last_seen: tick,
                last_pos: at,
                seen_now: true,
                heard: false,
                told: false,
            });
            deal(&mut sim);
            assert!(holds(&sim, b, human), "{d:?} {bot_z}: holds the token");
            let st = sim.unit(b).expect("alive").ai;
            let want = (!yields).then_some(human);
            assert_eq!(pick(&sim, b, &st), want, "{d:?} bot at {bot_z} m");
            // A proxy focuser keeps the frozen rule (no yield).
            sim.set_proxy(b, Some(crate::ai::proxy::Strength::Good.profile()));
            let st = sim.unit(b).expect("alive").ai;
            if crate::ai::dfocus::focuses_as(&sim, sim.unit(b).expect("alive")) {
                assert_eq!(pick(&sim, b, &st), Some(human), "{d:?} proxy");
            }
        }
    }
    for d in [Difficulty::Easy, Difficulty::Normal] {
        assert!(!yields_at(d), "{d:?}");
    }
}
