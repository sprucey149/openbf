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
/// tickets, timers, score limits and hero rules. Returns the setup and the
/// teams as the battle orders them.
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
) -> Result<(BattleSetup, [String; 2])> {
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
        teams: Some(teams.clone()),
        classes_dir: assets_dir.join("classes"),
        mission,
        vehicle: None,
        mission_file,
        player_team,
        heroes: game::heroes::rules(session),
        dlc_heroes: session.dlc_heroes,
        score_limit: if hunt {
            session.hun_score.max(1) as u32
        } else {
            session.ass_score.max(0) as u32
        },
    };
    Ok((setup, teams))
}

/// A LAN round's [`BattleSetup`]: [`shell_setup`] with the host's values.
pub(super) fn battle_setup(
    round: &RoundSetup,
    missions_dir: &Path,
    assets_dir: &Path,
) -> Result<(BattleSetup, [String; 2])> {
    let session = round
        .options
        .to_session(ui::shell::SessionOptions::default());
    shell_setup(
        missions_dir,
        assets_dir,
        &round.map,
        &round.mode,
        &round.armies,
        &session,
        round.seed,
        round.difficulty,
    )
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
    let (setup, _) = battle_setup(round, missions_dir, assets_dir)?;
    let mut walk = super::battle::make_walk(scene, &setup)?;
    super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
    walk.local_slot = local_slot;
    game::netplay::install_humans(&mut walk, round)?;
    Ok(walk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::netplay::{HumanSeat, PlayerProfile, RoundOptions};

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
                (Ok((a, ta)), Ok((b, tb))) => {
                    assert_eq!(ta, tb, "{map} {mode}");
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
}
