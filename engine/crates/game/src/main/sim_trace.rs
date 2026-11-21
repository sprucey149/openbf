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
    /// Critical-system points each team scored (credited enemy kills only),
    /// so a round's points split into systems and kills.
    system_points: [i32; 2],
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
            system_points: [0; 2],
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
                    let system = sim.space_assault.as_ref().and_then(|space| {
                        space
                            .critical
                            .iter()
                            .find(|system| system.station == *station)
                    });
                    if let Some(system) = system {
                        self.critical_lost += 1;
                        let owner = sim.stations.get(usize::from(*station)).and_then(|s| s.team);
                        if let Some(team) = killer.and_then(|h| sim.unit(h)).map(|u| u.team) {
                            if owner != Some(team) && system.points > 0 {
                                if let Some(p) = self.system_points.get_mut(usize::from(team)) {
                                    *p += system.points;
                                }
                            }
                        }
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
    let mut hunt_targets = None;
    let mut hunt_alive = None;
    let mut hunt_units = None;
    let mut hunt_skill = None;
    let mut step_timing = false;
    let mut hunt_timeline = false;
    let mut score_limit = None;
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
            "--hunt-targets" => {
                let values = it.next().context("--hunt-targets <team0>:<team1>")?;
                let (a, b) = values.split_once(':').context("expected a:b")?;
                let pair = [a.parse::<u32>()?, b.parse::<u32>()?];
                anyhow::ensure!(pair.into_iter().all(|n| n > 0), "targets must be positive");
                hunt_targets = Some(pair);
            }
            "--hunt-alive" => {
                let values = it.next().context("--hunt-alive <team0>:<team1>")?;
                let (a, b) = values.split_once(':').context("expected a:b")?;
                let pair = [a.parse::<u32>()?, b.parse::<u32>()?];
                anyhow::ensure!(
                    pair.into_iter().all(|n| n > 0),
                    "alive caps must be positive"
                );
                hunt_alive = Some(pair);
            }
            "--hunt-units" => {
                let values = it.next().context("--hunt-units <team0>:<team1>")?;
                let (a, b) = values.split_once(':').context("expected a:b")?;
                let pair = [a.parse::<u32>()?, b.parse::<u32>()?];
                anyhow::ensure!(pair.into_iter().all(|n| n > 0), "units must be positive");
                hunt_units = Some(pair);
            }
            "--hunt-skill" => {
                let values = it.next().context("--hunt-skill <team0 %>:<team1 %>")?;
                let (a, b) = values.split_once(':').context("expected a:b")?;
                let pair = [a.parse::<u32>()?, b.parse::<u32>()?];
                let floor = core::sim::deathmatch::HUNT_SKILL_FLOOR;
                anyhow::ensure!(
                    pair.into_iter().all(|n| (floor..=100).contains(&n)),
                    "Hunt skill percents must be {floor}..=100"
                );
                hunt_skill = Some(pair);
            }
            "--step-timing" => step_timing = true,
            "--hunt-timeline" => hunt_timeline = true,
            "--score-limit" => {
                let n: u32 = it.next().context("--score-limit <points>")?.parse()?;
                anyhow::ensure!(n > 0, "score limit must be positive");
                score_limit = Some(n);
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
            || (hunt_targets.is_none()
                && hunt_alive.is_none()
                && hunt_units.is_none()
                && hunt_skill.is_none()
                && !hunt_timeline),
        "Hunt targets, alive counts, units, skill and timeline require --mode hunt"
    );
    if let Some(m) = mission.as_mut().filter(|m| m.is_hunt()) {
        if let Some(pair) = hunt_targets {
            m.hunt_points_to_win = Some([pair; 4]);
        }
        if let Some(pair) = hunt_alive {
            m.hunt_alive_caps = Some([pair; 4]);
        }
        if let Some(pair) = hunt_units {
            m.hunt_units = Some(pair);
        }
        if let Some(pair) = hunt_skill {
            m.hunt_skill_pct = Some([pair; 4]);
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
        let (mut scene, load) = game::assets::load_battle_scene(&map, &mode)?;
        let killed = mission
            .as_ref()
            .map_or(0, |m| scene.kill_objects(m.scene_kills()));
        if killed != 0 {
            println!("mission: {killed} objects killed from the scene (KillObject)");
        }
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
            xl: false,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: score_limit.unwrap_or(core::sim::DEFAULT_SCORE_LIMIT),
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
            netplay_rows: false,
            opening_humans: [0; 2],
            score_limit: score_limit.unwrap_or(if mode == "hunt" {
                0
            } else {
                core::sim::deathmatch::ASSAULT_SCORE_LIMIT
            }),
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
    // already owns slot 0; each additional slot is its own player slot
    // (`UnitSpec::player_in_slot`: the slot's controller and its
    // `Sim::players` entry agree), a live soldier on team 0 at the same start
    // so the log's pads drive it from the first tick.
    let slots = log.as_ref().map_or(1, InputLog::slot_count);
    let start = sim.units.first().map(|u| u.soldier.pos).unwrap_or_default();
    for slot in 1..slots {
        let spec = core::unit::UnitSpec::player_in_slot(slot as u8, 0, start);
        let h = sim.spawn(&spec);
        if h.is_none() || sim.players.get(slot).copied().flatten() != h {
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
    if mode == "hunt" {
        let counts = [0u8, 1].map(|team| {
            let points = sim.spawn_points(team);
            let mut distinct: Vec<[u32; 3]> = points
                .iter()
                .map(|(p, _)| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
                .collect();
            distinct.sort_unstable();
            distinct.dedup();
            (sim.spawn_posts(team).len(), distinct.len())
        });
        println!(
            "hunt spawns: team 0 {} posts {} nodes, team 1 {} posts {} nodes; alive {:?}, units {:?}, skill {:?} %",
            counts[0].0,
            counts[0].1,
            counts[1].0,
            counts[1].1,
            sim.team_combatants,
            sim.deathmatch.as_ref().map(|d| d.members),
            [sim.hunt_skill_pct(0), sim.hunt_skill_pct(1)],
        );
    }
    let mut step_times: Vec<u32> = Vec::new();
    let mut alive_sum = [0u64; 2];
    let mut tally = game::blackbox::session::Tally::default();
    let mut hunt_end = None;
    // --hunt-timeline: the tick of every Hunt point, per team. Targets never
    // steer the round before it ends, so one long round answers any pair.
    let mut timeline: [Vec<u64>; 2] = Default::default();
    let mut seen = [0u32; 2];
    for _ in 0..ticks {
        let pads = match &mut frames {
            Some(frames) => frames.next().context("input exhausted")?,
            None => &idle,
        };
        if step_timing {
            let start = std::time::Instant::now();
            sim.step(pads);
            step_times.push(start.elapsed().as_micros().min(u128::from(u32::MAX)) as u32);
            for (_, u) in sim.alive_units() {
                if let Some(n) = alive_sum.get_mut(usize::from(u.team)) {
                    *n += 1;
                }
            }
        } else {
            sim.step(pads);
        }
        if let Some(i) = proxy.filter(|_| hunt_end.is_none()) {
            tally.tick(&sim, Some((i, "proxy")));
        }
        if hunt_timeline && hunt_end.is_none() {
            if let Some((points, _)) = sim.team_points() {
                for team in 0..2 {
                    for _ in seen[team]..points[team] {
                        timeline[team].push(sim.tick);
                    }
                }
                seen = points;
            }
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
        if let Some(report) = &report {
            println!("space systems: points {:?}", report.system_points);
        }
    }
    if mode == "hunt" {
        if let Some((points, limits)) = sim.team_points() {
            println!(
                "hunt: points {points:?} of {limits:?}, outcome {:?}, end tick {hunt_end:?}",
                sim.outcome,
            );
        }
        if hunt_timeline {
            for (team, ticks) in timeline.iter().enumerate() {
                let list: Vec<String> = ticks.iter().map(u64::to_string).collect();
                println!("hunt timeline {team}: {}", list.join(","));
            }
        }
    }
    // Issue #171 parity screening: a 1-flag or Hero Assault round's result.
    if let Some(c) = &sim.ctf {
        println!(
            "ctf: points {:?} of {}, outcome {:?}, tick {}",
            c.team_points, c.score_limit, sim.outcome, sim.tick
        );
    }
    if mode != "hunt" {
        if let Some((points, limits)) = sim.team_points() {
            let limit = limits[0];
            println!(
                "deathmatch: points {points:?} of {limit}, outcome {:?}, tick {}",
                sim.outcome, sim.tick
            );
        }
    }
    if let Some(team) = proxy_team {
        println!(
            "hunt proxy: team {team}, kills {}, deaths {}, shots {}, hits {}",
            tally.out.kills, tally.out.deaths, tally.out.shots, tally.out.hits
        );
    }
    // --step-timing: wall time of each Sim::step (the tick alone, not the
    // trace writer), and the mean soldiers alive per side.
    if !step_times.is_empty() {
        let n = step_times.len();
        let mean = step_times.iter().map(|&t| u64::from(t)).sum::<u64>() as f64 / n as f64;
        let mut sorted = step_times.clone();
        sorted.sort_unstable();
        let at = |q: f64| sorted[((n as f64 - 1.0) * q).round() as usize];
        println!(
            "step timing: {n} ticks, mean {mean:.0} us, p50 {} us, p95 {} us, p99 {} us, max {} us; alive mean {:.1}:{:.1}",
            at(0.5),
            at(0.95),
            at(0.99),
            sorted[n - 1],
            alive_sum[0] as f64 / n as f64,
            alive_sum[1] as f64 / n as f64,
        );
    }
    println!("trace: {ticks} ticks -> {}", out.display());
    Ok(())
}
