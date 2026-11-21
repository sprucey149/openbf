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
    let pick = game::mission::pick(missions_dir, name, mode, armies);
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
        mission.as_ref().is_none_or(|m| m.mode == mode),
        "mission mode differs from {mode}"
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
        // Map-relative team size, including the human.
        ai: None,
        // Explicit CLI cap is separate from the percentage setting.
        ai_cap: space.then_some(session.ass_numbots.max(0) as u32),
        team_size_pct: (!assault && !space && !hunt).then_some(session.con_numbots as u32),
        // Calibrated map tickets, scaled by the shell percentage.
        tickets: 250,
        tickets_set: false,
        ticket_pct: if assault || space || hunt {
            100
        } else {
            session.con_mult.max(10) as u32
        },
        time_limit: if hunt {
            session.hun_timer.max(1) as u32
        } else if assault || space {
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
        !round.mode.eq_ignore_ascii_case("campaign"),
        "campaign is single-player only"
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

/// Singleplayer "Rise of the Empire" campaign mission setup (`mode == "campaign"`):
/// loads the generated campaign RON script directly, puts the player on
/// team 0, and preserves the script's authored unit counts, hero roster, and
/// ticket rules without overwriting them with Instant Action defaults.
pub(super) fn campaign_setup(
    missions_dir: &Path,
    assets_dir: &Path,
    entry: &ui::campaign::CampaignMissionEntry,
    seed: u32,
    difficulty: core::skill::Difficulty,
) -> Result<BattleSetup> {
    let mission = game::mission::MissionDef::load_campaign(missions_dir, entry.script)?;
    anyhow::ensure!(
        mission.teams.len() >= 2,
        "campaign mission {} must define two teams",
        entry.script
    );
    let teams = [
        mission.teams[0].faction.clone(),
        mission.teams[1].faction.clone(),
    ];
    let tickets = mission.teams[0].tickets.max(100);
    let player_team = mission.initial_player_team();
    Ok(BattleSetup {
        seed,
        difficulty,
        spawn: (0.0, 0.0),
        spawn_set: false,
        targets: false,
        ai: None,
        ai_cap: None,
        team_size_pct: None,
        tickets,
        tickets_set: false,
        ticket_pct: 100,
        time_limit: 0,
        space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
        cheats: core::sim::Cheats::default(),
        teams: Some(teams),
        classes_dir: assets_dir.join("classes"),
        mission: Some(mission),
        vehicle: None,
        mission_file: Some(format!("bf2_{}.ron", entry.script)),
        player_team,
        heroes: game::heroes::rules(&ui::shell::SessionOptions::default()),
        dlc_heroes: false,
        netplay_rows: false,
        opening_humans: [0; 2],
        score_limit: 0,
    })
}

/// World sublevel layers requested by a campaign mission script, if any.
pub(super) fn campaign_layers_for(
    missions_dir: &Path,
    map: &str,
    mode: &str,
    script: Option<&str>,
) -> Vec<String> {
    if !mode.eq_ignore_ascii_case("campaign") {
        return Vec::new();
    }
    if let Some(stem) = script {
        if let Ok(m) = game::mission::MissionDef::load_campaign(missions_dir, stem) {
            return m.layers;
        }
    }
    let dir = missions_dir.join("campaign");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    // The campaign directory is generated local data; its `read_dir` order is
    // filesystem-dependent, so sort before picking a mission for a map.
    let mut files: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ron"))
        .collect();
    files.sort();
    for p in files {
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(m) = game::mission::MissionDef::load_campaign(missions_dir, stem) {
            if m.map.eq_ignore_ascii_case(map) {
                return m.layers;
            }
        }
    }
    Vec::new()
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
            let layers = campaign_layers_for(&missions_dir, &round.map, &round.mode, None);
            let (scene, load) =
                game::assets::load_battle_scene_with_layers(&dir, &round.mode, &layers)?;
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
    fn lan_rejects_campaign_rounds() {
        let round = RoundSetup::instant_action(
            "bf2_geo1",
            "Campaign",
            ["rep".into(), "cis".into()],
            &ui::shell::SessionOptions::default(),
            &game::profile::Profile::default(),
        );
        assert!(RoundSetup::decode(&round.encode())
            .unwrap_err()
            .to_string()
            .contains("campaign is single-player only"));
        assert!(battle_setup(&round, Path::new(""), Path::new(""))
            .err()
            .unwrap()
            .to_string()
            .contains("campaign is single-player only"));
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

    #[test]
    fn campaign_setup_loads_rise_of_the_empire_entry() {
        let missions = Path::new("../../missions");
        let assets = Path::new("../../assets_baked");
        if !missions.join("campaign").is_dir() {
            return;
        }
        let entry = &ui::campaign::RISE_OF_THE_EMPIRE[0];
        let setup = campaign_setup(missions, assets, entry, 42, core::skill::Difficulty::Normal)
            .expect("campaign setup for Geonosis");
        assert_eq!(setup.player_team, 0);
        assert_eq!(setup.team_size_pct, None);
        assert_eq!(setup.mission_file.as_deref(), Some("bf2_geo1c_c.ron"));
        let mission = setup.mission.as_ref().expect("mission");
        assert!(mission.campaign.is_some());
        assert_eq!(
            campaign_layers_for(missions, entry.map, "campaign", Some(entry.script)),
            mission.layers
        );
    }

    #[test]
    #[ignore = "requires the owned bf2_myg1 bake and missions; run with --release"]
    fn myg1c_walk_to_the_objective_has_no_long_tick() {
        use core::sim::{buttons, PadState};
        let missions = Path::new("../../missions");
        let assets = Path::new("../../assets_baked");
        let entry = &ui::campaign::RISE_OF_THE_EMPIRE[1];
        let layers = campaign_layers_for(missions, entry.map, "campaign", Some(entry.script));
        let (scene, _) = game::assets::load_battle_scene_with_layers(
            &assets.join(entry.map),
            "campaign",
            &layers,
        )
        .expect("myg1 scene");
        let setup = campaign_setup(missions, assets, entry, 42, core::skill::Difficulty::Normal)
            .expect("myg1c setup");
        let mut walk = super::super::battle::make_walk(&scene, &setup).expect("walk");
        super::super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
        let route = [
            glam::Vec3::new(-123.0, 0.0, 299.0),
            glam::Vec3::new(-80.0, 0.0, 231.0),
            glam::Vec3::new(-64.0, 0.0, 198.0),
            glam::Vec3::new(-62.0, 0.0, 140.0),
        ];
        let mut worst = (0.0f64, 0u64);
        let mut spawned_at = None;
        for tick in 0..(60 * core::TICK_HZ as u64) {
            let alive = walk.sim.unit(walk.player).is_some_and(|u| u.alive);
            let pad = PadState {
                buttons: if alive || tick % 15 > 7 {
                    0
                } else {
                    buttons::SPAWN_ACCEPT
                },
                ly: if alive { PadState::quantize(1.0) } else { 0 },
                ..PadState::default()
            };
            if alive {
                let since = *spawned_at.get_or_insert(tick);
                let d = (tick - since) as f32 * core::TICK_DT * 7.0;
                let mut left = d;
                let mut at = route[route.len() - 1];
                for w in route.windows(2) {
                    let seg = (w[1] - w[0]).length();
                    if left <= seg {
                        at = w[0] + (w[1] - w[0]) * (left / seg);
                        break;
                    }
                    left -= seg;
                }
                let i = walk.player.index as usize;
                walk.sim.units[i].soldier.pos.x = at.x;
                walk.sim.units[i].soldier.pos.z = at.z;
            }
            let t = std::time::Instant::now();
            walk.step_tick(&[pad]);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            if ms > worst.0 {
                worst = (ms, tick);
            }
            if ms > 200.0 {
                let p = walk.sim.unit(walk.player).map(|u| u.soldier.pos);
                eprintln!("tick {tick}: {ms:.0} ms, human at {p:?}");
            }
        }
        eprintln!(
            "spawned at {spawned_at:?}, worst tick {:.1} ms at {}",
            worst.0, worst.1
        );
        assert!(spawned_at.is_some(), "the human never spawned");
        assert!(worst.0 < 500.0, "a {:.0} ms tick at {}", worst.0, worst.1);
    }
}
