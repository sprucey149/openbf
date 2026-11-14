//! `game --static-digest` (issue #168, LAN §4.3 and §7): build a round the
//! way every LAN peer builds it (`RoundSetup` -> `round_setup::build_round`)
//! and print its static gameplay digest by group, one JSON line per map and
//! mode. Two copies of the game whose lines differ would be refused by the
//! load barrier; `--dump DIR` writes each group's fields as text so two
//! copies can be diffed field by field.
//!
//!   game --static-digest --assets DIR --missions DIR --map KEY[,KEY..|all]
//!        [--mode M[,M..]|auto] [--teams a,b] [--dump DIR] [--out FILE]
//!
//! `--map all` takes every map folder under `--assets` that has a
//! `manifest.ron`; `--mode auto` (the default) takes every mode the
//! missions dir has a file for (`<map>_<mode>[_cw].ron`), and `con` for a
//! map without one. Nothing is stepped: the digest is a tick-0 measurement.

use anyhow::{bail, Context, Result};
use core::sim::Sim;
use game::netplay::{HumanSeat, RoundSetup};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

struct Args {
    assets: PathBuf,
    missions: PathBuf,
    maps: Vec<String>,
    modes: Option<Vec<String>>,
    teams: Option<[String; 2]>,
    dump: Option<PathBuf>,
    out: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut a = Args {
        assets: "assets_baked".into(),
        missions: "missions".into(),
        maps: Vec::new(),
        modes: None,
        teams: None,
        dump: None,
        out: None,
    };
    let mut it = args.iter();
    while let Some(f) = it.next() {
        let mut val = || it.next().with_context(|| format!("{f} <value>"));
        match f.as_str() {
            "--assets" => a.assets = val()?.into(),
            "--missions" => a.missions = val()?.into(),
            "--map" => a.maps = val()?.split(',').map(str::to_owned).collect(),
            "--mode" => {
                let v = val()?;
                a.modes = (v != "auto").then(|| v.split(',').map(str::to_owned).collect());
            }
            "--teams" => {
                let v = val()?;
                let (x, y) = v.split_once(',').context("--teams a,b")?;
                a.teams = Some([x.trim().into(), y.trim().into()]);
            }
            "--dump" => a.dump = Some(val()?.into()),
            "--out" => a.out = Some(val()?.into()),
            other => bail!("unknown --static-digest flag: {other}"),
        }
    }
    anyhow::ensure!(!a.maps.is_empty(), "--map KEY[,KEY..|all]");
    if a.maps == ["all"] {
        a.maps = std::fs::read_dir(&a.assets)
            .with_context(|| a.assets.display().to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("manifest.ron").is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        a.maps.sort();
    }
    Ok(a)
}

/// The modes `missions` has a file for on `map` (`<map>_<mode>[_cw].ron`).
pub(super) fn modes_of(missions: &Path, map: &str) -> Vec<String> {
    let prefix = format!("{map}_");
    let mut modes: Vec<String> = std::fs::read_dir(missions)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| {
            let rest = n.strip_prefix(&prefix)?.strip_suffix(".ron")?;
            let mode = rest.split('_').next()?;
            Some(mode.to_owned())
        })
        .collect();
    modes.sort();
    modes.dedup();
    if modes.is_empty() {
        modes.push("con".into());
    }
    modes
}

/// The round every peer builds for `map` and `mode`: the shell's Instant
/// Action defaults with the host (slot 0) as the one human.
pub(super) fn round(map: &str, mode: &str, armies: [String; 2]) -> RoundSetup {
    let mut setup = RoundSetup::instant_action(
        map,
        mode,
        armies,
        &ui::shell::SessionOptions::default(),
        &game::profile::Profile::default(),
    );
    setup.humans.push(HumanSeat {
        slot: 0,
        name: "Peer0".into(),
        team: None,
        cheats: 0,
        profile: super::net_soak::scripted_profile(0),
    });
    setup
}

pub(super) fn run(args: &[String]) -> Result<()> {
    let a = parse(args)?;
    let mut lines = Vec::new();
    let mut failed = 0;
    for map in &a.maps {
        let modes = a
            .modes
            .clone()
            .unwrap_or_else(|| modes_of(&a.missions, map));
        for mode in &modes {
            let line = match one(&a, map, mode) {
                Ok(v) => v,
                Err(e) => {
                    failed += 1;
                    serde_json::json!({ "map": map, "mode": mode, "error": format!("{e:#}") })
                }
            };
            println!("{line}");
            lines.push(line.to_string());
        }
    }
    if let Some(out) = &a.out {
        std::fs::write(out, lines.join("\n") + "\n").with_context(|| out.display().to_string())?;
    }
    if failed > 0 {
        eprintln!("static digest: {failed} round(s) failed to build");
    }
    Ok(())
}

fn one(a: &Args, map: &str, mode: &str) -> Result<serde_json::Value> {
    let dir = a.assets.join(map);
    let (scene, load) = game::assets::load_battle_scene(&dir, mode)
        .with_context(|| format!("load {}", dir.display()))?;
    if let Some((path, Err(error))) = load.common {
        bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let armies = a
        .teams
        .clone()
        .unwrap_or_else(|| super::net_soak::default_armies(&scene.manifest.source_game));
    let setup = round(map, mode, armies);
    let walk = super::round_setup::build_round(&scene, &setup, &a.missions, &a.assets, 0)?;
    let sim = &walk.sim;
    let groups: serde_json::Map<String, serde_json::Value> = sim
        .static_digest_groups()
        .into_iter()
        .map(|(g, d)| (g.to_owned(), format!("{d:016x}").into()))
        .collect();
    if let Some(root) = &a.dump {
        let d = root.join(format!("{map}_{mode}"));
        std::fs::create_dir_all(&d).with_context(|| d.display().to_string())?;
        for (group, text) in dump(sim) {
            std::fs::write(d.join(format!("{group}.txt")), text)?;
        }
    }
    Ok(serde_json::json!({
        "map": map,
        "mode": mode,
        "game": scene.manifest.source_game,
        "digest": format!("{:016x}", sim.static_digest()),
        "hash0": format!("{:016x}", sim.hash()),
        "groups": groups,
    }))
}

/// Each digest group's fields as `{:#?}` text, one section per field, for
/// diffing two copies (a diagnostic: it follows the groups of
/// `core::sim::state::digest` by hand; meshes and the nav graph, digested
/// by `feed_digest`, are summarized by their own digest of the group).
fn dump(sim: &Sim) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    macro_rules! group {
        ($name:literal, $($field:ident),* $(,)?) => {{
            let mut s = String::new();
            $( let _ = writeln!(s, "== {}\n{:#?}", stringify!($field), &sim.$field); )*
            out.push(($name, s));
        }};
    }
    group!(
        "mission",
        time_limit,
        respawn_delay,
        ai_alive_cap,
        team_combatants,
        friendly_fire_scale,
        mission_ai,
        omni_radius,
        ground_rules,
        extra_humans,
        kill_floor,
        item_pool,
        flyer_bots,
        ground_flyer_map,
    );
    group!(
        "session",
        team_handicap_on,
        cheats,
        slot_cheats,
        assist_intensity,
        pad_look_sensitivity,
        assist_armed,
        assist_lead_on,
        slot_assist,
    );
    group!("weapons", weapon_classes);
    group!("classes", classes, item_classes, station_classes);
    group!(
        "modes",
        test_rosters,
        heroes,
        deathmatch,
        space_assault,
        ctf
    );
    group!(
        "vehicles",
        hover_params,
        walker_params,
        flyer_params,
        flight_limits,
        landing_regions,
        flyer_paths,
        seat_setup,
        blast_setup,
        vehicle_damage_effects,
        slice_time,
        repair_scale,
        weak_spot_factor,
        missile_target_points,
    );
    group!("props", cps, doors, vehicle_pads, stations, station_bodies);
    group!(
        "bounds",
        boundary,
        death_regions,
        damage_regions,
        fly_ceiling
    );
    group!("water", water_level, water_mask, water_oscillates);
    group!(
        "nav",
        flank,
        nav_floors,
        nav_unwalkable,
        nav_trapped,
        spawn_seats
    );
    let mut s = String::new();
    if let Some(h) = &sim.heightfield {
        let _ = writeln!(
            s,
            "size {} unit {} offset {} {}",
            h.size, h.unit, h.offset_x, h.offset_z
        );
        for (i, row) in h.heights.chunks(h.size.max(1) as usize).enumerate() {
            let _ = writeln!(s, "{i}: {row:?}");
        }
    }
    out.push(("terrain", s));
    out
}
