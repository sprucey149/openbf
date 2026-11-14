//! SHASH headless trace CLI. Without a map it runs the fixture. With a baked
//! map it builds a Normal battle and requests 16 AI per team. Without a mission, the
//! map supplies its default factions and both teams start with 250 tickets.
//! `--mode` selects Conquest or an authored world layer. `--mission` uses the
//! mission's authored faction order and tickets. `--state-hashes` prints the
//! complete Sim::hash after every tick. No renderer or device.
//! InputLog frame 0 produces record tick 1. A log must have 1..=16 complete
//! slots and at least the requested frames; additional frames are ignored.

use super::BattleSetup;
use anyhow::{bail, Context, Result};
use core::sim::{trace::TraceWriter, InputLog, PadState, Sim};
use std::io::BufWriter;
use std::path::PathBuf;

struct SpaceReport {
    out: std::fs::File,
    flyers_lost: [u32; 3],
    stations_lost: u32,
    critical_lost: u32,
    critical_by_flyers: u32,
    won_at: Option<u64>,
}

impl SpaceReport {
    fn open(path: PathBuf) -> Result<Self> {
        use std::io::Write;
        let mut out = std::fs::File::create(&path).with_context(|| path.display().to_string())?;
        writeln!(
            out,
            "seconds,points0,points1,units0,units1,flying0,flying1,flyers_shot,flyers_crashed,flyers_unseen,stations_lost,system_hits,outcome,critical_lost,critical_by_flyers"
        )?;
        Ok(Self {
            out,
            flyers_lost: [0; 3],
            stations_lost: 0,
            critical_lost: 0,
            critical_by_flyers: 0,
            won_at: None,
        })
    }

    fn tick(&mut self, sim: &Sim) -> Result<()> {
        use core::sim::Event;
        use std::io::Write;
        for e in &sim.events {
            match e {
                Event::VehicleDestroyed { class, killer, .. } => {
                    let flyer = sim
                        .flyer_params
                        .get(*class as usize)
                        .is_some_and(Option::is_some);
                    if flyer {
                        self.flyers_lost[usize::from(killer.is_none())] += 1;
                    }
                }
                Event::StationDestroyed {
                    station, killer, ..
                } => {
                    self.stations_lost += 1;
                    if sim.space_assault.as_ref().is_some_and(|space| {
                        space
                            .critical
                            .iter()
                            .any(|system| system.station == *station)
                    }) {
                        self.critical_lost += 1;
                        if killer
                            .and_then(|h| sim.unit(h))
                            .and_then(|u| u.mount)
                            .and_then(|m| sim.vehicle(m.vehicle))
                            .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer)
                        {
                            self.critical_by_flyers += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        if self.won_at.is_none() && sim.outcome.is_some() {
            self.won_at = Some(sim.tick);
            println!(
                "space assault: outcome {:?} at {} s, points {:?}",
                sim.outcome,
                sim.tick / u64::from(core::TICK_HZ),
                sim.space_team_points().map(|p| p.0)
            );
        }
        if !sim.tick.is_multiple_of(u64::from(core::TICK_HZ) * 60) {
            return Ok(());
        }
        let Some(((points, _), balance)) = sim.space_team_points().zip(sim.space_balance()) else {
            return Ok(());
        };
        let mut units = [0u32; 2];
        for (_, u) in sim.alive_units() {
            if let Some(n) = units.get_mut(usize::from(u.team)) {
                *n += 1;
            }
        }
        let mut flying = [0u32; 2];
        for (_, v) in sim.alive_vehicles() {
            let is_flying = v.kind == core::vehicle::VehicleKind::Flyer
                && v.flyer.phase == core::vehicle::flyer::FlightPhase::Flying;
            if let (true, Some(n)) = (
                is_flying,
                v.team.and_then(|t| flying.get_mut(usize::from(t))),
            ) {
                *n += 1;
            }
        }
        let unseen = balance.fighter_kills;
        writeln!(
            self.out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{:?},{},{}",
            sim.tick / u64::from(core::TICK_HZ),
            points[0],
            points[1],
            units[0],
            units[1],
            flying[0],
            flying[1],
            self.flyers_lost[0].saturating_sub(unseen),
            self.flyers_lost[1],
            unseen,
            self.stations_lost,
            balance.system_hits,
            sim.outcome,
            self.critical_lost,
            self.critical_by_flyers,
        )?;
        Ok(())
    }
}

pub(super) fn run(args: &[String]) -> Result<()> {
    let mut it = args.iter();
    let out = PathBuf::from(it.next().context("--sim-trace <out> <ticks>")?);
    let ticks: usize = it
        .next()
        .context("--sim-trace <out> <ticks>")?
        .parse()
        .context("tick count")?;
    let (mut input, mut seed, mut map, mut full) = (None, None, None, false);
    let (mut mode, mut mission_file) = ("con".to_owned(), None);
    let mut state_hashes = false;
    let mut space_report = None;
    let mut no_human = false;
    let mut difficulty = core::skill::Difficulty::default();
    let mut proxy_team = None;
    let mut hunt_counts = None;
    let mut hunt_weights = None;
    let mut hunt_health = None;
    let mut stop_at_outcome = false;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--input" => input = Some(PathBuf::from(it.next().context("--input <inputlog.ron>")?)),
            "--seed" => {
                seed = Some(
                    it.next()
                        .context("--seed <u32>")?
                        .parse::<u32>()
                        .context("seed")?,
                )
            }
            "--map" => map = Some(PathBuf::from(it.next().context("--map <baked-dir>")?)),
            "--mode" => mode = it.next().context("--mode <name>")?.to_owned(),
            "--mission" => {
                mission_file = Some(PathBuf::from(it.next().context("--mission <file.ron>")?));
            }
            "--state-hashes" => state_hashes = true,
            "--no-human" => no_human = true,
            "--stop-at-outcome" => stop_at_outcome = true,
            "--difficulty" => {
                let name = it.next().context("--difficulty <tier>")?;
                difficulty = core::skill::Difficulty::parse(name).context("unknown difficulty")?;
            }
            "--hunt-proxy" => {
                let team: u8 = it.next().context("--hunt-proxy <0|1>")?.parse()?;
                anyhow::ensure!(team < 2, "Hunt proxy team must be 0 or 1");
                proxy_team = Some(team);
            }
            "--hunt-counts" | "--hunt-weights" | "--hunt-health" => {
                let values = it.next().context("Hunt override requires a:b")?;
                let (a, b) = values.split_once(':').context("expected a:b")?;
                let (a, b) = (a.parse::<u16>()?, b.parse::<u16>()?);
                anyhow::ensure!(a > 0 && b > 0, "Hunt override must be positive");
                match flag.as_str() {
                    "--hunt-counts" => hunt_counts = Some([u8::try_from(a)?, u8::try_from(b)?]),
                    "--hunt-weights" => hunt_weights = Some([u8::try_from(a)?, u8::try_from(b)?]),
                    _ => hunt_health = Some([a, b]),
                }
            }
            "--space-report" => {
                space_report = Some(PathBuf::from(
                    it.next().context("--space-report <file.csv>")?,
                ));
            }
            "--trace-full" => full = true,
            other => bail!("unknown sim-trace flag: {other}"),
        }
    }
    if map.is_none() && (mode != "con" || mission_file.is_some()) {
        bail!("--mode and --mission require --map");
    }
    if let Some(path) = &mission_file {
        if path == &out
            || std::fs::canonicalize(path)
                .ok()
                .zip(std::fs::canonicalize(&out).ok())
                .is_some_and(|(input, output)| input == output)
        {
            bail!("trace output aliases the mission file");
        }
    }
    let mut mission = mission_file
        .as_ref()
        .map(|path| game::mission::MissionDef::load(path))
        .transpose()?;
    if let Some(mission) = &mission {
        if mission.mode != mode {
            bail!("mission mode {} differs from --mode {mode}", mission.mode);
        }
    }
    let log: Option<InputLog> = input
        .as_ref()
        .map(|p| -> Result<_> {
            ron::from_str(&std::fs::read_to_string(p).with_context(|| p.display().to_string())?)
                .context("parse input log")
        })
        .transpose()?;
    if let Some(log) = &log {
        if !(1..=16).contains(&log.slots) || !log.ticks.len().is_multiple_of(usize::from(log.slots))
        {
            bail!("input log needs 1..=16 slots and complete frames");
        }
        if log.tick_count() < ticks {
            bail!(
                "input log has {} frames, requested {ticks}",
                log.tick_count()
            );
        }
        if input.as_ref() == Some(&out) {
            bail!("trace output must differ from the input log");
        }
        // Catch existing symlinks/relative aliases before opening the output.
        if let (Ok(a), Ok(b)) = (
            std::fs::canonicalize(input.as_ref().unwrap()),
            std::fs::canonicalize(&out),
        ) {
            if a == b {
                bail!("trace output aliases the input log");
            }
        }
    }
    let seed = seed.or_else(|| log.as_ref().map(|l| l.seed)).unwrap_or(42);
    anyhow::ensure!(
        proxy_team.is_none() || (mode == "hunt" && no_human),
        "Hunt proxy requires --mode hunt --no-human"
    );
    anyhow::ensure!(
        mode == "hunt"
            || (hunt_counts.is_none() && hunt_weights.is_none() && hunt_health.is_none()),
        "Hunt overrides require --mode hunt"
    );
    if let Some(m) = mission.as_mut().filter(|m| m.is_hunt()) {
        if let Some(counts) = hunt_counts {
            m.hunt_combatants = Some([counts.map(u32::from); 4]);
            m.ai_alive_cap = u32::from(counts[0].max(counts[1]));
        }
        if let Some(weights) = hunt_weights {
            m.hunt_points_per_kill = Some(weights);
            m.hunt_kill_values = None;
        }
        if let Some(health) = hunt_health {
            m.hunt_health_percent = Some([health; 4]);
        }
    }
    let (mut sim, name) = if let Some(map) = map {
        // Normal battles may use a generic design fallback. A non-Conquest
        // parity trace must prove the requested mode actually exists first.
        if mode != "con" {
            let manifest: game::assets::Manifest =
                ron::from_str(&pak::read_to_string(map.join("manifest.ron"))?)?;
            if !(mode == "hunt" && manifest.source_game == "bf1")
                && game::assets::authored_mode_layer_index(&manifest, &mode).is_none()
            {
                bail!("map has no authored world layer for mode {mode}");
            }
        }
        let (scene, load) = game::assets::load_battle_scene(&map, &mode)?;
        if let Some((path, Err(error))) = load.common {
            bail!("common assets at {} unreadable: {error:#}", path.display());
        }
        if let Some(mission) = &mission {
            if (mode == "hunt") != mission.is_hunt() {
                bail!("Hunt mode requires its matching Hunt mission");
            }
            if !map
                .file_name()
                .is_some_and(|key| mission.map.eq_ignore_ascii_case(&key.to_string_lossy()))
                || !mission
                    .source_game
                    .eq_ignore_ascii_case(&scene.manifest.source_game)
            {
                bail!("mission does not describe this map and game");
            }
        }
        let setup = BattleSetup {
            seed,
            difficulty,
            spawn: (-140.0, 230.0),
            spawn_set: false,
            targets: false,
            ai: (mode != "hunt").then_some(16),
            ai_cap: None,
            team_size_pct: None,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: mission.as_ref().map(|m| m.factions()),
            classes_dir: map
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("classes"),
            mission,
            vehicle: None,
            mission_file: mission_file.as_ref().map(|p| p.display().to_string()),
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
            score_limit: if mode == "hunt" {
                50
            } else {
                core::sim::deathmatch::ASSAULT_SCORE_LIMIT
            },
        };
        let walk = super::battle::make_walk(&scene, &setup)?;
        (
            walk.sim,
            format!("{}:{}", scene.manifest.source_game, scene.manifest.map),
        )
    } else {
        (Sim::new(seed), "fixture".to_owned())
    };
    // Provision each recorded local player slot on the same setup. Map mode
    // already owns slot 0; additional slots use the same team/start position.
    let slots = log.as_ref().map_or(1, InputLog::slot_count);
    let start = sim.units.first().map(|u| u.soldier.pos).unwrap_or_default();
    for slot in 1..slots {
        let mut spec = core::unit::UnitSpec::player(0, start);
        spec.controller = core::unit::Controller::Player { slot: slot as u8 };
        if sim.spawn(&spec).is_none() {
            bail!("cannot provision input slot {slot}");
        }
    }
    let file = std::fs::File::create(&out).with_context(|| out.display().to_string())?;
    let mut writer = TraceWriter::new(BufWriter::new(file), &name, seed, full)?;
    let idle = [PadState::default()];
    let mut frames = log.as_ref().map(InputLog::frames);
    let mut report = space_report.map(SpaceReport::open).transpose()?;
    if no_human && mode == "hunt" {
        for slot in 0..sim.players.len() {
            if let Some((pos, yaw)) = sim.vacate_player_slot(slot) {
                sim.spawn_ai(0, pos, yaw);
            }
        }
    } else if no_human {
        sim.players.iter_mut().for_each(|slot| *slot = None);
    }
    let proxy = if let Some(team) = proxy_team {
        let handle = sim
            .alive_units()
            .find(|(_, u)| u.team == team)
            .map(|(h, _)| h)
            .context("Hunt proxy team has no live bot")?;
        sim.set_proxy(handle, Some(core::ai::proxy::Strength::Good.profile()));
        Some(handle.index as usize)
    } else {
        None
    };
    let mut tally = game::blackbox::session::Tally::default();
    let mut hunt_end = None;
    for _ in 0..ticks {
        let pads = match &mut frames {
            Some(frames) => frames.next().context("input exhausted")?,
            None => &idle,
        };
        sim.step(pads);
        if let Some(i) = proxy.filter(|_| hunt_end.is_none()) {
            tally.tick(&sim, Some((i, "proxy")));
        }
        if mode == "hunt" && sim.outcome.is_some() && hunt_end.is_none() {
            hunt_end = Some(sim.tick);
        }
        writer.write_tick(&sim)?;
        if let Some(report) = &mut report {
            report.tick(&sim)?;
        }
        if state_hashes {
            println!("state {} {:016x}", sim.tick, sim.hash());
        }
        if stop_at_outcome && sim.outcome.is_some() {
            break;
        }
    }
    writer.finish()?;
    if let Some((points, limit)) = sim.space_team_points() {
        println!(
            "space assault: points {points:?} of {limit}, outcome {:?}, tick {}",
            sim.outcome, sim.tick
        );
    }
    if mode == "hunt" {
        if let Some((points, limit)) = sim.team_points() {
            println!(
                "hunt: points {points:?} of {limit}, outcome {:?}, end tick {hunt_end:?}",
                sim.outcome,
            );
        }
    }
    if let Some(team) = proxy_team {
        println!(
            "hunt proxy: team {team}, kills {}, deaths {}, shots {}, hits {}",
            tally.out.kills, tally.out.deaths, tally.out.shots, tally.out.hits
        );
    }
    println!("trace: {ticks} ticks -> {}", out.display());
    Ok(())
}
