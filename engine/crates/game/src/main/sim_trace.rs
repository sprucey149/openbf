//! SHASH headless trace CLI: fixture by default, or the existing battle
//! builder on a baked conquest map (16 AI/team, the default difficulty (Normal), 250
//! tickets, default factions, no mission overrides). No renderer or device.
//! InputLog frame 0 produces record tick 1. A log must have 1..=16 complete
//! slots and at least the requested frames; additional frames are ignored.

use super::BattleSetup;
use anyhow::{bail, Context, Result};
use core::sim::{trace::TraceWriter, InputLog, PadState, Sim};
use std::io::BufWriter;
use std::path::PathBuf;

pub(super) fn run(args: &[String]) -> Result<()> {
    let mut it = args.iter();
    let out = PathBuf::from(it.next().context("--sim-trace <out> <ticks>")?);
    let ticks: usize = it
        .next()
        .context("--sim-trace <out> <ticks>")?
        .parse()
        .context("tick count")?;
    let (mut input, mut seed, mut map, mut full) = (None, None, None, false);
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
            "--trace-full" => full = true,
            other => bail!("unknown sim-trace flag: {other}"),
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
    let (mut sim, name) = if let Some(map) = map {
        let (scene, load) = game::assets::load_battle_scene(&map, "con")?;
        if let Some((path, Err(error))) = load.common {
            bail!("common assets at {} unreadable: {error:#}", path.display());
        }
        let setup = BattleSetup {
            seed,
            difficulty: Default::default(),
            spawn: (-140.0, 230.0),
            spawn_set: false,
            targets: false,
            ai: Some(16),
            ai_cap: None,
            team_size_pct: None,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            cheats: Default::default(),
            teams: None,
            classes_dir: map
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("classes"),
            mission: None,
            mission_file: None,
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
        };
        let walk = super::battle::make_walk(&scene, &setup);
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
    for _ in 0..ticks {
        let pads = match &mut frames {
            Some(frames) => frames.next().context("input exhausted")?,
            None => &idle,
        };
        sim.step(pads);
        writer.write_tick(&sim)?;
    }
    writer.finish()?;
    println!("trace: {ticks} ticks -> {}", out.display());
    Ok(())
}
