//! The shell's battle setup and the LAN round built from the same function
//! (issue #168, LAN §4.3): the Instant Action launch data (map, mode, the
//! playlist row's armies, Session Options, seed, tier) as a [`BattleSetup`],
//! and a [`RoundSetup`] as the same [`BattleSetup`], so a LAN round and a
//! single-player round of the same settings are built alike.
//!
//! [`build_round`] is every peer's round build: `make_walk` from the setup,
//! the shell's spawn screens for slot 0, then the round's humans
//! (`game::netplay::install_humans`).

use super::BattleSetup;
use anyhow::Result;
use game::netplay::RoundSetup;
use game::walk::WalkSim;
use std::path::Path;

/// The shell's battle for map `name` in `mode` with the playlist row's
/// `row_armies` (`App::start_battle`): the mission picked for the pair, the
/// player's army on its team there, and the Session Options' population,
/// tickets, timers, score limits and hero rules. The setup's `teams` hold
/// the armies as the battle orders them.
#[allow(clippy::too_many_arguments)]
pub(super) fn shell_setup(
    missions_dir: &Path,
    assets_dir: &Path,
    name: &str,
    mode: &str,
    row_armies: &[String; 2],
    session: &ui::shell::SessionOptions,
    seed: u32,
    difficulty: core::skill::Difficulty,
) -> Result<BattleSetup> {
    let armies = [row_armies[0].as_str(), row_armies[1].as_str()];
    let xl = mode == ui::shell::Mode::Xl.subst();
    let mission_mode = if xl { "con" } else { mode };
    let pick = game::mission::pick(missions_dir, name, mission_mode, armies);
    if xl {
        let code = name.strip_prefix("bf2_").unwrap_or(name);
        anyhow::ensure!(
            pick.as_ref().is_some_and(|p| {
                p.own
                    && p.mission.source_game == "bf2"
                    && ui::shell::xl_eras(code).iter().any(|era| {
                        let pair = era.armies();
                        armies == pair || armies == [pair[1], pair[0]]
                    })
            }),
            "XL requires a supported Battlefront II map and era with a Conquest mission"
        );
    }
    let player_team = pick.as_ref().map_or(0, |p| p.player_team);
    let mut teams = armies.map(str::to_string);
    if player_team == 1 {
        teams.swap(0, 1);
    }
    let mission_file = pick.as_ref().and_then(|p| super::file_name(&p.path));
    match &pick {
        Some(p) => println!(
            "mission: {} for {} against {} ({}), the player on team {}",
            p.path.display(),
            armies[0],
            armies[1],
            if p.own {
                "its own pair"
            } else {
                "a cross-era pair"
            },
            player_team + 1
        ),
        None => println!("mission: none for {name} {mode}"),
    }
    let mut mission = pick.map(|p| p.mission);
    anyhow::ensure!(
        !matches!(mode, "eli" | "hunt") || mission.is_some(),
        "Hero Assault and Hunt require a mission"
    );
    anyhow::ensure!(
        mission.as_ref().is_none_or(|m| m.mode == mission_mode),
        "mission mode differs from {mission_mode}"
    );
    let assault = mission.as_ref().is_some_and(|m| m.is_hero_assault());
    let hunt = mode == "hunt";
    if let Some(m) = mission.as_mut().filter(|_| assault) {
        m.ai_alive_cap = session.ass_numbots.max(0) as u32;
    }
    let space = mode == "ass";
    let setup = BattleSetup {
        seed,
        // The shell round takes the PROFILE's tier (no CLI flag here);
        // Session Options' AI Difficulty writes it.
        difficulty,
        spawn: (0.0, 0.0),
        spawn_set: false,
        targets: false,
        xl,
        ai: None,
        // Explicit CLI cap is separate from the percentage setting.
        ai_cap: space.then_some(session.ass_numbots.max(0) as u32),
        // Map-relative team size, including the human.
        team_size_pct: (!assault && !space && !hunt).then_some(session.con_numbots as u32),
        // Calibrated map tickets, scaled by the shell percentage.
        tickets: 250,
        tickets_set: false,
        ticket_pct: if assault || space || hunt || xl {
            100
        } else {
            session.con_mult.max(10) as u32
        },
        time_limit: if hunt {
            session.hun_timer.max(1) as u32
        } else if assault || space || xl {
            0
        } else {
            session.con_timer.max(0) as u32
        },
        space_score_limit: session.ass_score.max(1) as u32,
        cheats: core::sim::Cheats::default(),
        teams: Some(teams),
        classes_dir: assets_dir.join("classes"),
        mission,
        vehicle: None,
        mission_file,
        player_team,
        heroes: game::heroes::rules(session),
        dlc_heroes: session.dlc_heroes,
        netplay_rows: false,
        opening_humans: [0; 2],
        score_limit: if hunt {
            session.hun_score.max(1) as u32
        } else {
            session.ass_score.max(0) as u32
        },
    };
    Ok(setup)
}

/// A LAN round's [`BattleSetup`]: [`shell_setup`] with the host's values.
pub(super) fn battle_setup(
    round: &RoundSetup,
    missions_dir: &Path,
    assets_dir: &Path,
) -> Result<BattleSetup> {
    anyhow::ensure!(
        round.mode != ui::shell::Mode::Xl.subst(),
        "XL is available in single-player Instant Action"
    );
    let session = round
        .options
        .to_session(ui::shell::SessionOptions::default());
    let mut setup = shell_setup(
        missions_dir,
        assets_dir,
        &round.map,
        &round.mode,
        &round.armies,
        &session,
        round.seed,
        round.difficulty,
    )?;
    // Issue #171: the LAN marker from the opening wave on, and the
    // opening wave counts every round-start human, not only slot 0.
    setup.netplay_rows = true;
    setup.opening_humans = game::netplay::joining_humans(round, setup.player_team);
    Ok(setup)
}

/// Every peer's round build: the battle, the shell's spawn screens for
/// slot 0, the round's humans, and `local_slot` as the viewer.
pub(super) fn build_round(
    scene: &game::assets::Scene,
    round: &RoundSetup,
    missions_dir: &Path,
    assets_dir: &Path,
    local_slot: u8,
) -> Result<WalkSim> {
    round.env.check_local()?;
    let setup = battle_setup(round, missions_dir, assets_dir)?;
    let mut walk = super::battle::make_walk(scene, &setup)?;
    super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
    walk.local_slot = local_slot;
    game::netplay::install_humans(&mut walk, round)?;
    Ok(walk)
}

/// A LAN host's self-check build (`game::netplay::selfcheck`), on a worker
/// thread: the round's map loaded from `assets_dir` again (the window's own
/// copy stays where it is), [`build_round`], and the event feed the window
/// makes for it (`round_prep`). A dropped check stops it between those
/// stages.
pub(super) fn rebuilder(
    assets_dir: std::path::PathBuf,
    missions_dir: std::path::PathBuf,
) -> game::netplay::Rebuild {
    use game::netplay::selfcheck::Stop;
    game::netplay::Rebuild::Thread(std::sync::Arc::new(
        move |round: &RoundSetup, local, stop: &Stop| {
            let dir = assets_dir.join(&round.map);
            let (scene, load) = game::assets::load_battle_scene(&dir, &round.mode)?;
            if let Some((path, Err(error))) = load.common {
                anyhow::bail!("common assets at {} unreadable: {error:#}", path.display());
            }
            stop.check()?;
            let walk = build_round(&scene, round, &missions_dir, &assets_dir, local)?;
            stop.check()?;
            let mut feed = game::event_feed::EventFeed::for_walk(
                &assets_dir,
                &scene.manifest.source_game,
                &walk,
            );
            feed.set_opening_medals(&walk);
            Ok((walk, Some(feed)))
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::netplay::{HumanSeat, PlayerProfile, RoundOptions};

    #[test]
    fn xl_launches_all_supported_eras_from_instant_action() {
        use ui::shell::{MapPick, Mode, ModePick, SidePick};

        let missions = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
        let assets = game::assets::baked_root();
        if !missions.is_dir() || !assets.join("bf2_geo1/manifest.ron").is_file() {
            eprintln!("SKIP: no owned Battlefront II bake and missions");
            return;
        }
        let difficulty = core::skill::Difficulty::Normal;
        let mode = Mode::Xl;
        let mut ia = game::shell_ia::instant_action(&assets, missions, difficulty);
        assert_eq!(ia.texts.mode(mode).0, "XL");
        ia.add(MapPick::All, ModePick::One(mode), SidePick::AllEras);
        let rows = ia.launch();
        let mut pairs: Vec<_> = rows
            .iter()
            .map(|row| (row.key.as_str(), row.armies[0].as_str()))
            .collect();
        pairs.sort_unstable();
        assert_eq!(
            pairs,
            [
                ("bf2_geo1", "rep"),
                ("bf2_hot1", "all"),
                ("bf2_kas2", "all"),
                ("bf2_kas2", "rep"),
            ]
        );
        let mut lobby = game::shell_ia::lobby_instant_action(&assets, missions, difficulty);
        lobby.add(MapPick::All, ModePick::One(mode), SidePick::AllEras);
        assert!(lobby.launch().is_empty());

        for row in rows {
            let (scene, load) =
                game::assets::load_battle_scene(&assets.join(&row.key), mode.subst()).unwrap();
            assert!(load.layer.is_some(), "{} needs its Conquest layer", row.key);
            for (team_pct, ticket_pct, size) in [(100, 100, 48), (200, 50, 96)] {
                let session = ui::shell::SessionOptions {
                    con_numbots: team_pct,
                    con_mult: ticket_pct,
                    con_timer: 1,
                    ..ia.options
                };
                let setup = shell_setup(
                    missions,
                    &assets,
                    &row.key,
                    mode.subst(),
                    &row.armies,
                    &session,
                    42,
                    difficulty,
                )
                .unwrap();
                let mission = setup.mission.as_ref().unwrap();
                let mut walk = super::super::battle::make_walk(&scene, &setup).unwrap();
                assert_eq!(walk.sim.team_combatants, Some([size; 2]), "{}", row.key);
                assert_eq!(
                    walk.sim.battle.teams.map(|team| team.alive()),
                    [size; 2],
                    "{}",
                    row.key
                );
                assert!(walk.sim.is_xl());
                assert!(walk.sim.battle.unlimited_reinforcements);
                assert_eq!(walk.sim.time_limit, None);
                assert!(!walk.sim.player_spawn_posts(0).is_empty());
                assert!(!walk.sim.player_spawn_posts(1).is_empty());
                let owners: Vec<_> = walk.sim.cps.iter().map(|post| post.owner).collect();
                let player = walk.player.index as usize;
                let enemy_post = walk
                    .sim
                    .cps
                    .iter()
                    .position(|post| {
                        post.capture_authored && post.owner == Some(1 - setup.player_team)
                    })
                    .unwrap();
                let original_pos = walk.sim.units[player].soldier.pos;
                walk.sim.units[player].soldier.pos = walk.sim.cps[enemy_post].def.capture.center;
                assert!(!walk.sim.counts_in_cp(player, enemy_post));
                let mut feed = game::event_feed::EventFeed::for_walk(&assets, "bf2", &walk);
                let hud = super::super::battle::hud_model(&walk, &mut feed, 16.0 / 9.0);
                assert_eq!(hud.team_points, Some(([0, 0], 350)));
                assert!(hud.capture.is_none() && hud.countdown.is_none());
                walk.sim.units[player].soldier.pos = original_pos;
                let start = std::time::Instant::now();
                for _ in 0..120 {
                    walk.step_tick(&[core::sim::PadState::default()]);
                }
                assert_eq!(walk.sim.tick, 120);
                assert!(walk.sim.cps.iter().map(|post| post.owner).eq(owners));
                assert_eq!(
                    walk.sim.battle.teams.map(|team| team.bleed_per_sec),
                    [0.0; 2]
                );
                eprintln!(
                    "{} {}: {size} per team, 120 ticks in {:?}",
                    row.key,
                    mission.era.as_deref().unwrap(),
                    start.elapsed()
                );
            }
        }
    }

    fn summary(b: &BattleSetup) -> String {
        format!(
            "{} {:?} {:?} {:?} {:?} {} {} {} {} {:?} {:?} {} {:?} {} {} {:?} {}",
            b.seed,
            b.difficulty,
            b.ai,
            b.ai_cap,
            b.team_size_pct,
            b.tickets,
            b.ticket_pct,
            b.time_limit,
            b.space_score_limit,
            b.teams,
            b.mission_file,
            b.player_team,
            b.heroes,
            b.dlc_heroes,
            b.score_limit,
            b.classes_dir,
            b.mission.as_ref().map_or(0, |m| m.ai_alive_cap),
        )
    }

    /// A round set up from the shell's launch data, sent as bytes and
    /// decoded, builds the same `BattleSetup` the shell builds (every mode,
    /// a non-default session, both army orders).
    #[test]
    fn round_setup_builds_the_shells_battle_setup() {
        let missions = Path::new("../../missions");
        let assets = Path::new("../../assets_baked");
        let session = ui::shell::SessionOptions {
            con_numbots: 140,
            con_mult: 60,
            con_timer: 12,
            hero_unlock: 5,
            hero_unlock_val: 33,
            hero_assign: 7,
            hero_respawn: -1,
            ass_numbots: 20,
            ass_score: 150,
            hun_timer: 7,
            hun_score: 40,
            ..ui::shell::SessionOptions::default()
        };
        let mut host = game::profile::Profile::default();
        host.career.rounds = 17;
        host.settings.difficulty = core::skill::Difficulty::Hard;
        for (map, mode, armies) in [
            ("bf2_kas2", "con", ["rep", "cis"]),
            ("bf2_kas2", "con", ["cis", "rep"]),
            ("bf1_tat2", "con", ["imp", "all"]),
            ("bf2_cor1", "eli", ["rep", "cis"]),
            ("bf2_mus1", "hunt", ["rep", "cis"]),
        ] {
            let armies = armies.map(str::to_owned);
            let mut round = RoundSetup::instant_action(map, mode, armies.clone(), &session, &host);
            round.humans.push(HumanSeat {
                slot: 0,
                name: "Host".into(),
                team: None,
                cheats: 0,
                profile: PlayerProfile::default(),
            });
            let back = RoundSetup::decode(&round.encode()).unwrap();
            assert_eq!(back, round);
            assert_eq!(back.options, RoundOptions::from_session(&session));
            let shell = shell_setup(
                missions,
                assets,
                map,
                mode,
                &armies,
                &session,
                42 + 17,
                core::skill::Difficulty::Hard,
            );
            let lan = battle_setup(&back, missions, assets);
            match (shell, lan) {
                (Ok(a), Ok(b)) => {
                    assert_eq!(a.teams, b.teams, "{map} {mode}");
                    assert_eq!(summary(&a), summary(&b), "{map} {mode}");
                    if missions.is_dir() && mode == "con" {
                        assert!(a.mission_file.is_some(), "{map}: no mission picked");
                    }
                }
                (Err(a), Err(b)) => assert_eq!(format!("{a:#}"), format!("{b:#}")),
                (a, b) => panic!("{map} {mode}: {:?} vs {:?}", a.is_ok(), b.is_ok()),
            }
        }
    }

    /// Issue #171 through the real LAN build (`build_round`): every opening
    /// bot spawns at the row its team plays once every round-start human is
    /// seated, by the single-player head-count rule (OURS #171 step 3, which
    /// replaced option C's host row for both teams: an Elite 2v1's pair's
    /// opening bots at 18, not 20 and not the host alone's 17); at Normal
    /// the DTEAM rule counts every human, not only slot 0, the host: a 1v1
    /// (the guest on the auto team) has no surplus, and a 2v1 with the host
    /// alone gives the surplus to the other side.
    #[test]
    fn a_lan_2v1_round_takes_its_rows_before_the_opening_wave() {
        use core::skill::Difficulty as D;
        let missions = Path::new("../../missions");
        let assets = game::assets::baked_root();
        let dir = assets.join("bf1_nab2");
        if !dir.join("manifest.ron").exists() || !missions.is_dir() {
            eprintln!("SKIP: no real bake or missions");
            return;
        }
        let (scene, _) = game::assets::load_battle_scene(&dir, "con").unwrap();
        // (tier, the joiners: on the host's team / the other team / auto).
        let cases = [
            (D::Hard, [1, 1, 0]),
            (D::Elite, [1, 1, 0]),
            (D::Normal, [1, 1, 0]),
            (D::Normal, [0, 0, 1]),
            (D::Normal, [0, 2, 0]),
        ];
        for (d, [with_host, against, auto]) in cases {
            let mut host = game::profile::Profile::default();
            host.settings.difficulty = d;
            let mut round = RoundSetup::instant_action(
                "bf1_nab2",
                "con",
                ["all".into(), "imp".into()],
                &ui::shell::SessionOptions::default(),
                &host,
            );
            let seat = |slot: u8, team: Option<u8>| HumanSeat {
                slot,
                name: format!("P{slot}"),
                team,
                cheats: 0,
                profile: PlayerProfile::default(),
            };
            round.humans.push(seat(0, None));
            let ht = battle_setup(&round, missions, &assets).unwrap().player_team;
            let mut slot = 1;
            for (n, team) in [(with_host, Some(ht)), (against, Some(1 - ht)), (auto, None)] {
                for _ in 0..n {
                    round.humans.push(seat(slot, team));
                    slot += 1;
                }
            }
            let case = format!("{d:?} {with_host}/{against}/{auto}");
            let walk = build_round(&scene, &round, missions, &assets, 0).unwrap();
            let sim = &walk.sim;
            let humans = sim.humans_per_team();
            // An auto seat joins team 1 only while it has fewer humans.
            let mut want_humans = [0u8; 2];
            want_humans[usize::from(ht)] = 1 + with_host;
            want_humans[usize::from(1 - ht)] = against;
            for _ in 0..auto {
                want_humans[usize::from(want_humans[1] < want_humans[0])] += 1;
            }
            assert_eq!(humans, want_humans, "{case}");
            assert!(sim.netplay_rows, "{case}");
            let want = core::skill::team_skill(d, humans);
            assert_eq!(sim.team_rows(), want, "{case}");
            let mut seen = [0; 2];
            for u in sim
                .units
                .iter()
                .filter(|u| u.alive && u.team < 2 && u.controller == core::unit::Controller::Ai)
            {
                let t = usize::from(u.team);
                seen[t] += 1;
                // Under the old wiring the opening wave saw the host alone
                // (and, before that, no LAN flag): its bots took the
                // surplus row the seated humans no longer give.
                assert_eq!(u.skill, want[t].level, "{case} team {t}");
            }
            assert!(seen[0] > 0 && seen[1] > 0, "{case}: opening bots {seen:?}");
        }
    }
}
