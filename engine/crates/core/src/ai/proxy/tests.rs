use super::*;
use crate::unit::{Controller, UnitHandle, UnitSpec};
use glam::Vec3;

fn bot(sim: &mut Sim, team: u8, at: Vec3) -> UnitHandle {
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(team, at)
    })
    .expect("bot")
}

fn field(d: Difficulty) -> (Sim, UnitHandle, UnitHandle) {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.units.clear();
    sim.players.clear();
    sim.difficulty = d;
    let a = bot(&mut sim, 0, Vec3::ZERO);
    let b = bot(&mut sim, 1, Vec3::new(0.0, 0.0, -15.0));
    (sim, a, b)
}

/// An unset flag leaves the sim's hash as it was; setting it moves it.
#[test]
fn the_flag_is_hashed_only_when_set() {
    let (mut sim, a, _) = field(Difficulty::Normal);
    let before = sim.hash();
    assert!(sim.set_proxy(a, None));
    assert_eq!(sim.hash(), before, "None is no byte");
    assert!(sim.set_proxy(a, Some(Strength::Good.profile())));
    assert_ne!(sim.hash(), before);
}

/// A proxy is a human to the human-only rules: the TOKN tokens (the enemy
/// bot without one is barred), the DTEAM count, `is_human`; and a bot to the
/// cheats (`is_player` stays false).
#[test]
fn a_proxy_is_human_to_the_human_rules() {
    let (mut sim, a, b) = field(Difficulty::Normal);
    assert_eq!(humans_on(&sim, 0), 0);
    sim.unit_mut(b).unwrap().ai.target = Some(a.into());
    assert!(!crate::ai::tokn::bars(&sim, b, a), "a bot is not governed");
    sim.set_proxy(a, Some(Strength::Good.profile()));
    let u = sim.unit(a).unwrap();
    assert!(u.is_human() && !u.is_player);
    assert_eq!((humans_on(&sim, 0), humans_on(&sim, 1)), (1, 0));
    assert!(crate::ai::tokn::bars(&sim, b, a), "governed, no token yet");
    crate::ai::tokn::deal(&mut sim);
    assert!(
        crate::ai::tokn::holds(&sim, b, a),
        "the lone shooter holds one"
    );
}

/// The proxy's profile is the same on every tier: its skill has no Elite
/// cap, its reaction and view are its own.
#[test]
fn the_profile_ignores_the_tier() {
    let p = Strength::Good.profile();
    for d in Difficulty::ALL {
        let (mut sim, a, _) = field(d);
        sim.set_proxy(a, Some(p));
        let u = sim.unit(a).unwrap();
        assert_eq!(sim.skill_of(u), Skill::from_level(p.skill), "{d:?}");
        let t = sim.unit_tier(u);
        assert_eq!(t.reaction_ticks, p.reaction_ticks);
        assert_eq!(t.react, p.react);
        assert!((t.view_mult - p.view_mult * sim.mission_ai.view_mult).abs() < 1e-6);
    }
    let (sim, a, _) = field(Difficulty::Elite);
    assert_eq!(sim.skill_of(sim.unit(a).unwrap()).err_cap, 0.9, "a bot's");
}

/// A proxy that dies comes back a proxy.
#[test]
fn the_flag_survives_a_respawn() {
    let (mut sim, a, _) = field(Difficulty::Normal);
    sim.set_proxy(a, Some(Strength::Average.profile()));
    let (capture_time, neutralize_time) = crate::cp::CpDef::class_times(false);
    sim.add_cp(
        crate::cp::CpDef {
            name: "home".into(),
            pos: Vec3::ZERO,
            capture: crate::region::Region::sphere(Vec3::ZERO, 8.0),
            control: None,
            value_bleed: 10,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![(Vec3::new(2.0, 0.0, 2.0), 0.0)],
            capture_time,
            neutralize_time,
        },
        Some(0),
    );
    sim.kill_environment(a, crate::sim::DeathCause::Fall);
    let i = a.index as usize;
    sim.respawn_slot(i);
    assert!(sim.units[i].alive && sim.units[i].gen > a.gen, "respawned");
    assert_eq!(sim.units[i].proxy_human, Some(Strength::Average.profile()));
}

/// The targets: the maintainer's table, each tier's band below the one
/// before it, and the verdict reads the band.
#[test]
fn the_targets_are_the_maintainers_and_fall_by_tier() {
    assert_eq!(target(Difficulty::Easy), (0.90, 1.0));
    assert_eq!(target(Difficulty::Normal), (0.77, 0.90));
    assert_eq!(target(Difficulty::Hard), (0.45, 0.60));
    assert_eq!(target(Difficulty::Elite), (0.25, 0.40));
    for w in TARGETS.windows(2) {
        assert!(w[1].1 < w[0].1 && w[1].2 <= w[0].2);
    }
    assert!(passes(Difficulty::Normal, 28, 32));
    assert!(!passes(Difficulty::Normal, 24, 32));
    assert!(!passes(Difficulty::Normal, 30, 32));
    assert!(!passes(Difficulty::Normal, 32, 32));
    assert!(passes(Difficulty::Easy, 32, 32));
    assert!(!passes(Difficulty::Elite, 0, 0));
    for (d, lo, hi) in [
        (Difficulty::Easy, 90, 100),
        (Difficulty::Normal, 77, 90),
        (Difficulty::Hard, 45, 60),
        (Difficulty::Elite, 25, 40),
    ] {
        assert!(passes(d, lo, 100) && passes(d, hi, 100), "{d:?}");
        assert!(!passes(d, lo - 1, 100) && !passes(d, hi + 1, 100), "{d:?}");
    }

    assert_eq!(Strength::parse("AVG"), Some(Strength::Average));
    assert_eq!(Strength::parse("good").map(Strength::label), Some("good"));
}

/// A still shooter on team 0 and a standing target `d` m off on team 1, in
/// an empty BF1 round with a rifle whose AI range ends at 24 m; `hand`
/// makes the shooter a proxy with a hand. Returns the shooter's shots over
/// `ticks`.
fn hand_shots(hand: bool, d: f32, ticks: u32) -> usize {
    let _style = crate::ai::tests::all_stand();
    use crate::sim::Event;
    let mut s = Sim::empty(5);
    s.ground_rules = crate::soldier::GroundRules::Bf1;
    s.cheats.infinite_ammo = true;
    s.weapon_classes[0] = crate::weapon::WeaponClass {
        ai_range: [0.0, 12.0, 24.0],
        ..crate::weapon::WeaponClass::bf1_rifle()
    };
    let a = bot(&mut s, 0, Vec3::ZERO);
    let t = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -d)))
        .unwrap();
    if hand {
        let p = Profile {
            hand: Some(hand::Hand::DEFAULT),
            ..Strength::Average.profile()
        };
        s.set_proxy(a, Some(p));
    }
    let mut shots = 0;
    for _ in 0..ticks {
        s.step(&[crate::sim::PadState::default()]);
        s.unit_mut(t).unwrap().health.cur = 300.0;
        shots += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == a))
            .count();
    }
    shots
}

/// The hand fires past the bot's AI range (out to the round's reach), and
/// close in it holds the trigger where a bot fires bursts and pauses.
#[test]
fn a_hand_fires_past_the_ai_range_and_without_pauses() {
    assert_eq!(hand_shots(false, 40.0, 300), 0, "a bot: beyond its 24 m");
    assert!(hand_shots(true, 40.0, 300) > 10, "the hand: 40 m");
    let (bot, hand) = (hand_shots(false, 8.0, 600), hand_shots(true, 8.0, 600));
    assert!(hand > bot * 3 / 2, "held trigger: bot {bot}, hand {hand}");
}

/// A hand proxy's weapons rest at a player's spread floor, now and after a
/// respawn; a proxy without a hand keeps a bot's.
#[test]
fn a_hand_carries_a_players_weapon() {
    let (mut sim, a, b) = field(Difficulty::Normal);
    sim.set_proxy(b, Some(Strength::Average.profile()));
    assert!(!sim.unit(b).unwrap().weapon.human);
    let p = Profile {
        hand: Some(hand::Hand::DEFAULT),
        ..Strength::Good.profile()
    };
    sim.set_proxy(a, Some(p));
    let u = sim.unit(a).unwrap();
    assert!(u.weapon.human && u.held.iter().all(|w| w.state.human));
}

fn post(sim: &mut Sim, name: &str, at: Vec3, owner: Option<u8>) {
    let (capture_time, neutralize_time) = crate::cp::CpDef::class_times(false);
    sim.add_cp(
        crate::cp::CpDef {
            name: name.into(),
            pos: at,
            capture: crate::region::Region::sphere(at, 8.0),
            control: None,
            value_bleed: 10,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![(at + Vec3::new(2.0, 0.0, 2.0), 0.0)],
            capture_time,
            neutralize_time,
        },
        owner,
    );
}

/// A hunting proxy attacks the nearest post its team does not hold (its
/// own nearer post is not one), then hunts when it holds them all; a Normal
/// bot and a proxy that does not hunt keep the commander's plan.
#[test]
fn a_hunting_proxy_goes_to_the_nearest_post_it_does_not_hold() {
    use crate::commander::Objective;
    let (mut sim, a, b) = field(Difficulty::Normal);
    // (The fixture's own posts are team 0's: out of the running.)
    for c in &mut sim.cps {
        c.owner = Some(0);
    }
    let n = sim.cps.len() as u32;
    post(&mut sim, "own", Vec3::new(5.0, 0.0, 0.0), Some(0));
    post(&mut sim, "far", Vec3::new(80.0, 0.0, 0.0), Some(1));
    post(&mut sim, "near", Vec3::new(0.0, 0.0, 40.0), None);
    assert_eq!(front(&sim, sim.unit(a).unwrap()), None, "a bot");
    sim.set_proxy(b, Some(Strength::Average.profile()));
    assert_eq!(front(&sim, sim.unit(b).unwrap()), None, "no hunts");
    let p = Profile {
        hunts: true,
        ..Strength::Good.profile()
    };
    sim.set_proxy(a, Some(p));
    assert_eq!(
        front(&sim, sim.unit(a).unwrap()),
        Some(Objective::Attack(n + 2))
    );
    sim.cps[n as usize + 2].owner = Some(0);
    assert_eq!(
        front(&sim, sim.unit(a).unwrap()),
        Some(Objective::Attack(n + 1))
    );
    sim.cps[n as usize + 1].owner = Some(0);
    assert_eq!(front(&sim, sim.unit(a).unwrap()), Some(Objective::Hunt));
}

/// A mending proxy under its share goes to the nearest standing health
/// droid in reach, stays there until nearly whole, and never goes to one
/// out of reach or destroyed; a bot whose tier has no `mend` never mends.
#[test]
fn a_hurt_proxy_goes_to_a_health_droid_and_stays_until_whole() {
    let (mut sim, a, _) = field(Difficulty::Normal);
    let c = sim.add_station_class(crate::sim::StationClass::soldier("health", 0.5, 20.0, 0));
    let at = Vec3::new(30.0, 0.0, 0.0);
    sim.stations.push(crate::sim::Station::new(c, at, 2.0));
    sim.stations
        .push(crate::sim::Station::new(c, Vec3::new(200.0, 0.0, 0.0), 2.0));
    let hurt = |sim: &mut Sim, share: f32| {
        let u = sim.unit_mut(a).unwrap();
        u.health.cur = u.health.max * share;
    };
    hurt(&mut sim, 0.2);
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "a bot");
    let p = Profile {
        mend: 0.35,
        ..Strength::Good.profile()
    };
    sim.set_proxy(a, Some(p));
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), Some(at));
    hurt(&mut sim, 0.6);
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "not low, not there");
    sim.unit_mut(a).unwrap().soldier.pos = at;
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), Some(at), "there: stays");
    hurt(&mut sim, 1.0);
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "whole");
    hurt(&mut sim, 0.2);
    sim.stations[0].destroyed = true;
    sim.unit_mut(a).unwrap().soldier.pos = Vec3::ZERO;
    assert_eq!(
        mend(&sim, sim.unit(a).unwrap()),
        None,
        "the far one is out of reach"
    );
}

/// A proxy with a class comes back as that class at every respawn, as a
/// player picks his; one without keeps the bots' draw.
#[test]
fn a_proxy_respawns_as_its_class() {
    let class = || crate::unit::UnitClass {
        consts: crate::soldier::SoldierConstants::default(),
        max_health: 300.0,
        health_type: crate::weapon::HealthType::Person,
        weapon_class: 0,
        total_clips: 6,
        weapon_slot: 0,
        slots: std::sync::Arc::new([]),
        jet: false,
        capture_posts: true,
        drops: crate::sim::DropTable::default(),
        chunk: None,
        min: 0,
        max: None,
        unlock_points: 0,
        droideka: None,
    };
    let (mut sim, a, _) = field(Difficulty::Normal);
    sim.classes[0] = vec![class(), class(), class()];
    post(&mut sim, "home", Vec3::ZERO, Some(0));
    let lives = |sim: &mut Sim| -> Vec<u8> {
        (0..12)
            .map(|_| {
                sim.kill_environment(sim.handle(a.index as usize), crate::sim::DeathCause::Fall);
                sim.respawn_slot(a.index as usize);
                sim.units[a.index as usize].spec.class
            })
            .collect()
    };
    let drawn = lives(&mut sim);
    assert!(drawn.iter().any(|&c| c != 2), "the draw: {drawn:?}");
    let p = Profile {
        class: Some(2),
        ..Strength::Good.profile()
    };
    sim.set_proxy(sim.handle(a.index as usize), Some(p));
    assert_eq!(lives(&mut sim), vec![2; 12]);
}

/// A picking proxy takes the enemy aiming at it over a nearer one, a hurt
/// one over a slightly nearer whole one, and only enemies it sees now; a
/// proxy that does not pick, and a Normal bot, get nothing from it.
#[test]
fn a_picking_proxy_takes_the_threat_then_the_hurt_then_the_nearest() {
    use crate::perception::{Contact, Foe};
    let (mut sim, a, near) = field(Difficulty::Normal);
    let far = bot(&mut sim, 1, Vec3::new(0.0, 0.0, -22.0));
    let seen = |h: UnitHandle, now: bool| {
        Some(Contact {
            handle: Foe::Unit(h),
            first_seen: 0,
            last_seen: 0,
            last_pos: Vec3::ZERO,
            seen_now: now,
            heard: false,
            told: false,
        })
    };
    let mut st = crate::ai::state::AiState::default();
    st.contacts[0] = seen(near, true);
    st.contacts[1] = seen(far, true);
    assert_eq!(pick(&sim, a, &st), None, "a bot");
    sim.set_proxy(a, Some(Strength::Average.profile()));
    assert_eq!(pick(&sim, a, &st), None, "no picks");
    let p = Profile {
        picks: true,
        ..Strength::Good.profile()
    };
    sim.set_proxy(a, Some(p));
    assert_eq!(pick(&sim, a, &st), Some(near), "the nearest");
    sim.unit_mut(far).unwrap().ai.target = Some(a.into());
    assert_eq!(pick(&sim, a, &st), Some(far), "the threat (22 - 15 < 15)");
    sim.unit_mut(far).unwrap().ai.target = None;
    sim.unit_mut(far).unwrap().health.cur *= 0.5;
    assert_eq!(pick(&sim, a, &st), Some(far), "hurt (22 - 10 < 15)");
    st.contacts[1] = seen(far, false);
    assert_eq!(pick(&sim, a, &st), Some(near), "unseen is not picked");
}

/// Past 20 the proxy's skill runs on the same curves: tighter spread and
/// error, a faster tracker, floors at zero; at 20 and below it is the
/// table's own, and a level past the maximum is the maximum.
#[test]
fn the_skill_runs_past_twenty_on_the_same_curves() {
    for l in 1..=20 {
        assert_eq!(skill_over(l), Skill::from_level(l));
    }
    let (k20, k25) = (skill_over(20), skill_over(25));
    assert!(k25.spread_mult < k20.spread_mult && k25.spread_mult > 0.0);
    assert!(k25.err_floor < k20.err_floor && k25.err_cap < k20.err_cap);
    assert!(k25.aim_accel > k20.aim_accel);
    assert_eq!(k25.dispersion_mult, 0.0);
    assert!((k25.spread_mult - (3.0 - 2.0 * 24.0 / 19.0)).abs() < 1e-5);
    assert_eq!(skill_over(40), k25);
    assert_eq!(k25.burst_t, k20.burst_t);
}

/// The good strength plays by every player's rule the fit kept; the
/// average one by none of them (a bot's gunner and plan).
#[test]
fn the_good_proxy_carries_the_fitted_features() {
    let g = Strength::Good.profile();
    assert!(g.hand.is_some() && g.headshots && g.hunts && g.picks);
    assert_eq!((g.class, g.mend), (Some(0), 0.35));
    let a = Strength::Average.profile();
    assert!(a.hand.is_none() && !a.headshots && !a.hunts && !a.picks);
    assert_eq!((a.class, a.mend), (None, 0.0));
}

// [ours] Tier features use each team's row, including split-tier rounds.
#[test]
fn tier_bots_attack_the_nearest_unheld_post() {
    use crate::commander::Objective;
    let (mut sim, a, _) = field(Difficulty::Easy);
    for c in &mut sim.cps {
        c.owner = Some(0);
    }
    let n = sim.cps.len() as u32;
    post(&mut sim, "near", Vec3::new(0.0, 0.0, 30.0), None);
    post(&mut sim, "far", Vec3::new(0.0, 0.0, 60.0), Some(1));
    sim.team_difficulty[0] = Some(Difficulty::Elite);
    assert_eq!(
        front(&sim, sim.unit(a).unwrap()),
        Some(Objective::Attack(n))
    );
    sim.cps[n as usize].owner = Some(0);
    assert_eq!(
        front(&sim, sim.unit(a).unwrap()),
        Some(Objective::Attack(n + 1))
    );
    sim.cps[n as usize + 1].owner = Some(0);
    assert_eq!(front(&sim, sim.unit(a).unwrap()), Some(Objective::Hunt));
    sim.difficulty = Difficulty::Elite;
    for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
        sim.team_difficulty[0] = Some(d);
        assert_eq!(front(&sim, sim.unit(a).unwrap()), None, "{d:?}");
    }
}

#[test]
fn tier_hurt_bots_seek_a_working_health_droid() {
    let (mut sim, a, _) = field(Difficulty::Easy);
    let c = sim.add_station_class(crate::sim::StationClass::soldier("health", 0.5, 20.0, 0));
    let at = Vec3::new(30.0, 0.0, 0.0);
    sim.stations.push(crate::sim::Station::new(c, at, 2.0));
    for d in [Difficulty::Hard, Difficulty::Elite] {
        sim.team_difficulty[0] = Some(d);
        let u = sim.unit_mut(a).unwrap();
        u.soldier.pos = Vec3::ZERO;
        u.health.cur = u.health.max * 0.34;
        assert_eq!(mend(&sim, sim.unit(a).unwrap()), Some(at), "{d:?}");
        let u = sim.unit_mut(a).unwrap();
        u.health.cur = u.health.max * 0.35;
        assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "threshold");
        sim.unit_mut(a).unwrap().soldier.pos = at;
        assert_eq!(mend(&sim, sim.unit(a).unwrap()), Some(at), "stay to heal");
        let u = sim.unit_mut(a).unwrap();
        u.health.cur = u.health.max * 0.95;
        assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "healed");
    }
    sim.difficulty = Difficulty::Elite;
    for d in [Difficulty::Easy, Difficulty::Normal] {
        sim.team_difficulty[0] = Some(d);
        let u = sim.unit_mut(a).unwrap();
        u.health.cur = u.health.max * 0.1;
        assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "{d:?}");
    }
    sim.team_difficulty[0] = Some(Difficulty::Hard);
    sim.stations[0].destroyed = true;
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "destroyed");
    sim.stations[0].destroyed = false;
    sim.unit_mut(a).unwrap().soldier.pos = at + Vec3::X * 81.0;
    assert_eq!(mend(&sim, sim.unit(a).unwrap()), None, "too far");
}

#[test]
fn a_mending_bot_leaves_the_last_hub_and_reaches_the_health_droid() {
    for difficulty in [Difficulty::Hard, Difficulty::Elite] {
        let (mut sim, bot) = crate::ai::tests::l_sim();
        sim.difficulty = difficulty;
        let start = Vec3::new(-0.5, 0.0, 0.0);
        let unit = sim.unit_mut(bot).unwrap();
        unit.soldier.pos = start;
        unit.soldier.grounded = true;
        let mut ai = unit.ai;
        crate::ai::think(&sim, bot, &mut ai);
        assert_eq!(ai.target_cp, Some(0), "control: a post order is active");
        assert_eq!(
            ai.route().unwrap().dest_hub,
            2,
            "control: the post is elsewhere"
        );

        // The droid shares the unit's current hub, above the bot's feet. The
        // bot must enter its sphere even when within a metre of it in XZ.
        let health =
            sim.add_station_class(crate::sim::StationClass::soldier("health", 0.5, 20.0, 0));
        let droid = Vec3::new(1.5, 1.3, 0.0);
        sim.stations
            .push(crate::sim::Station::new(health, droid, 1.5));
        let unit = sim.unit_mut(bot).unwrap();
        unit.health.cur = unit.health.max * 0.2;
        let before = unit.health.cur;
        unit.ai = ai;
        assert!(!sim.stations[0].reaches(start));
        assert!(!sim.stations[0].reaches(Vec3::ZERO));
        assert!(!sim.stations[0].reaches(Vec3::new(0.6, 0.0, 0.0)));

        sim.tick = ai.next_think;
        crate::ai::think(&sim, bot, &mut ai);
        let route = ai.route().expect("a route to the station's hub");
        assert!(route.is_last(), "control: the bot is on the station's hub");
        assert_eq!(route.dest_hub, 0);
        assert_eq!(
            ai.walk.goal, droid,
            "{difficulty:?}: walk beyond the hub centre"
        );

        sim.stations[0].destroyed = true;
        sim.tick = ai.next_think;
        crate::ai::think(&sim, bot, &mut ai);
        assert_eq!(ai.target_cp, Some(0), "keep the post assignment");
        assert_eq!(ai.route().unwrap().dest_hub, 2, "resume the attack route");
        sim.stations[0].destroyed = false;

        for _ in 0..180 {
            sim.step(&[]);
            let unit = sim.unit(bot).expect("the same bot life");
            if unit.health.cur > before {
                break;
            }
        }
        let unit = sim.unit(bot).expect("the same bot life");
        assert!(
            unit.health.cur > before,
            "{difficulty:?}: the droid must heal the bot"
        );
    }
}

fn add_disconnected_health_droid(sim: &mut Sim) -> Vec3 {
    use crate::nav::{Arc, Hub, NavGraph, SOLDIER};

    let droid = Vec3::new(20.0, 0.0, 0.0);
    let graph = sim.nav.as_mut().unwrap();
    let mut hubs = std::mem::take(&mut graph.hubs);
    let mut arcs = std::mem::take(&mut graph.arcs);
    hubs.extend([droid, droid + Vec3::X * 10.0].map(|pos| Hub {
        pos,
        radius: 2.0,
        arcs: vec![2],
    }));
    arcs.push(Arc {
        a: 3,
        b: 4,
        type_mask: 1 << SOLDIER,
        flags: 0,
    });
    *graph = NavGraph::new(hubs, arcs, Default::default(), vec![], vec![]).unwrap();
    assert!(!graph.reachable(0, 3, SOLDIER));
    assert!(!graph.reachable(2, 3, SOLDIER));

    let class = sim.add_station_class(crate::sim::StationClass::soldier("health", 0.5, 20.0, 0));
    sim.stations
        .push(crate::sim::Station::new(class, droid, 1.5));
    droid
}

#[test]
fn an_unroutable_health_droid_keeps_the_command_post_fallback() {
    let (mut sim, bot) = crate::ai::tests::l_sim();
    sim.difficulty = Difficulty::Hard;
    let mut ai = sim.unit(bot).unwrap().ai;
    crate::ai::think(&sim, bot, &mut ai);
    assert_eq!(ai.target_cp, Some(0));
    assert_eq!(ai.route().unwrap().dest_hub, 2);

    let droid = add_disconnected_health_droid(&mut sim);
    let unit = sim.unit_mut(bot).unwrap();
    unit.health.cur = unit.health.max * 0.2;
    assert_eq!(mend(&sim, sim.unit(bot).unwrap()), Some(droid));

    sim.tick = ai.next_think;
    crate::ai::think(&sim, bot, &mut ai);
    assert!(
        ai.route().is_none(),
        "the station is on a disconnected graph"
    );
    assert_eq!(ai.walk.goal, sim.cps[0].def.pos, "keep the post fallback");
}

#[test]
fn a_newly_hurt_bot_keeps_its_unrelated_last_leg_between_thinks() {
    let (mut sim, bot) = crate::ai::tests::l_sim();
    sim.difficulty = Difficulty::Hard;
    let at = sim.cps[0].def.pos + Vec3::X;
    let unit = sim.unit_mut(bot).unwrap();
    unit.soldier.pos = at;
    let mut ai = unit.ai;
    crate::ai::think(&sim, bot, &mut ai);
    let route = ai.route().expect("a route to the post");
    assert!(route.is_last());
    assert_eq!(route.dest_hub, 2);
    let droid = add_disconnected_health_droid(&mut sim);

    sim.tick = 1;
    assert!(sim.tick < ai.next_think);
    let mut healthy = ai;
    crate::ai::think(&sim, bot, &mut healthy);
    let unit = sim.unit_mut(bot).unwrap();
    unit.health.cur = unit.health.max * 0.2;
    assert_eq!(mend(&sim, sim.unit(bot).unwrap()), Some(droid));
    crate::ai::think(&sim, bot, &mut ai);

    assert_eq!(ai.last_think, healthy.last_think);
    assert_eq!(ai.route().unwrap().dest_hub, 2);
    assert_eq!(ai.walk, healthy.walk, "keep the old leg until a fresh plan");
}

#[test]
fn tier_elite_bots_prioritize_visible_threats() {
    use crate::perception::{Contact, Foe};
    let (mut sim, a, near) = field(Difficulty::Easy);
    let far = bot(&mut sim, 1, Vec3::new(0.0, 0.0, -22.0));
    let mut st = crate::ai::state::AiState::default();
    for (i, h) in [near, far].into_iter().enumerate() {
        st.contacts[i] = Some(Contact {
            handle: Foe::Unit(h),
            first_seen: 0,
            last_seen: 0,
            last_pos: Vec3::ZERO,
            seen_now: true,
            heard: false,
            told: false,
        });
    }
    sim.team_difficulty[0] = Some(Difficulty::Elite);
    assert_eq!(pick(&sim, a, &st), Some(near));
    sim.unit_mut(far).unwrap().ai.target = Some(a.into());
    assert_eq!(pick(&sim, a, &st), Some(far), "shoot the threat");
    st.contacts[1].as_mut().unwrap().seen_now = false;
    assert_eq!(pick(&sim, a, &st), Some(near), "only seen enemies");
    sim.difficulty = Difficulty::Elite;
    for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
        sim.team_difficulty[0] = Some(d);
        assert_eq!(pick(&sim, a, &st), None, "{d:?}");
    }
    let u = sim.unit(a).unwrap();
    assert!(u.proxy_human.is_none() && !u.weapon.human && u.skill <= 20);
}

#[test]
fn proxy_features_do_not_inherit_the_hosts_bot_columns() {
    for d in Difficulty::ALL {
        let (mut sim, a, _) = field(d);
        for p in [Strength::Average.profile(), Strength::Good.profile()] {
            sim.set_proxy(a, Some(p));
            let t = sim.unit_tier(sim.unit(a).unwrap());
            assert_eq!(
                (t.picks, t.mend, t.hunts),
                (p.picks, p.mend, p.hunts),
                "{d:?}"
            );
        }
    }
}

/// The proxy freeze (bot rework stage 0): every read site of a proxy's tier
/// gives the frozen value, on every tier. Where a site takes the live value
/// as an input (the Cover role, the scope share) the test passes a live
/// value unlike the frozen one, so a site gone back to the live row fails
/// here. The others read `Difficulty::tier` directly, which equals the
/// frozen rows today; for them the all-proxy pin below is the real guard.
#[test]
fn the_proxy_reads_the_frozen_rows() {
    let p = Strength::Good.profile();
    for d in Difficulty::ALL {
        let (mut sim, a, b) = field(d);
        sim.set_proxy(a, Some(p));
        let u = sim.unit(a).unwrap();
        // (The proxy's team takes its DTEAM row: one human more.)
        let row_d = sim.difficulty_of(u.team);
        let frozen = frozen_tier(row_d);
        let mut row = frozen;
        row.view_mult *= sim.mission_ai.view_mult;
        assert_eq!(
            sim.unit_tier(u),
            p.over(row, sim.mission_ai.view_mult),
            "{d:?}"
        );
        assert_eq!(sim.aim_err_cap(u), frozen_err_cap(row_d, u.skill), "{d:?}");
        // The Cover role: the proxy's frozen row whatever the live row says;
        // a bot follows the live row.
        let bot_u = sim.unit(b).unwrap();
        for live in [false, true] {
            assert_eq!(
                crate::ai::team::covers(u, row_d, live),
                frozen.cover,
                "{d:?}"
            );
            assert_eq!(crate::ai::team::covers(bot_u, row_d, live), live, "{d:?}");
        }
        // The scope share: Hard's frozen share, whatever tier is passed.
        let mut odd = frozen;
        odd.scope_sight = 0.25;
        assert_eq!(
            crate::perception::scope_share(u, &odd),
            FROZEN_TIERS[2].scope_sight,
            "{d:?}"
        );
        assert_eq!(crate::perception::scope_share(bot_u, &odd), 0.25);
        // The focus rule.
        assert_eq!(
            crate::ai::dfocus::focuses_as(&sim, u),
            frozen_focuses(row_d),
            "{d:?}"
        );
    }
}

/// Two Good proxies a side, 20 m apart on the flat L, stepped 30 s at Easy
/// and at Elite: a pin on everything the proxy's brain runs. It moves with
/// any change to the code a proxy runs (`ai::legacy_think`), the physics
/// and the weapons included; such a move is declared and explained.
/// 2026-11-13 (#168): glam's `scalar-math` (one math path on every CPU,
/// LAN_MULTIPLAYER.md §7) moved Easy 67abfedaa0631084 -> 7efc85cee4eb5f2a and
/// Elite e3f2d3c9099e44c4 -> 8350aa51d0876ce8.
#[test]
fn an_all_proxy_skirmish_keeps_its_hash() {
    let run = |d: Difficulty| {
        let (mut sim, _) = crate::ai::tests::l_sim();
        sim.units.clear();
        sim.players.clear();
        sim.difficulty = d;
        let p = Strength::Good.profile();
        for (team, x, z) in [
            (0, 0.0, 0.0),
            (0, 3.0, 0.0),
            (1, 0.0, -20.0),
            (1, 3.0, -20.0),
        ] {
            let h = bot(&mut sim, team, Vec3::new(x, 0.0, z));
            assert!(sim.set_proxy(h, Some(p)));
        }
        for _ in 0..900 {
            sim.step(&[]);
        }
        sim.hash()
    };
    let got = [run(Difficulty::Easy), run(Difficulty::Elite)];
    assert_eq!(
        got.map(|h| format!("{h:016x}")),
        ["7efc85cee4eb5f2a", "8350aa51d0876ce8"],
        "the all-proxy skirmish moved"
    );
}
