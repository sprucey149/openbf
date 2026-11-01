//! Game binary. Capture/replay spine flags (PLAN.md §Determinism) exist from
//! the start; the map viewer, walk mode, the shell, the bench, the orbit
//! capture and the scene report were added phase by phase.
//!
//!   --aim-replay <out-dir>      scripted target/cone/friend/range/wall referee (no GPU)
//!   --sim-hash <ticks>            run N ticks (idle input), print state hash
//!   --sim-trace <out> <ticks> [--input <log.ron>] [--seed S] [--map <baked-dir>]
//!     --trace-full               include quantized tuples (64 MiB cap)
//!   --replay <log.ron>            run an input log, print final hash
//!   (no args) | --shell           BF2-style instant-action shell (windowed)
//!     --assets <dir>              baked maps root (default assets_baked)
//!     --missions <dir>            mission RONs (default missions)
//!   --map <baked-dir>             open the map viewer (windowed)
//!     --walk                      soldier walk mode instead of fly-cam
//!     --targets                   spawn enemy dummy troopers (walk mode)
//!     --seed <n>                  sim seed (spread RNG; default 42)
//!     --difficulty <tier>         easy|normal|hard|elite (default: the profile's, Normal)
//!     --ai <n>                    conquest: CPs from the layout + n AI per team
//!     --ai-cap <n>                override the mission's alive-AI cap (the
//!                                 only way to run above the PS2 unit cap)
//!     --mode <name>               world layer to merge (BF2 maps; default con)
//!     --teams <a,b>               factions (all|imp|rep|cis); default by source game;
//!                                 the mission's own pair the other way round
//!                                 seats the player on its second team
//!     --mission <file.ron>        mission data (factions, tickets, unit counts, AI cap)
//!     --classes <dir>             class RONs (default <map>/../classes)
//!     --tickets <n>               reinforcements per team (default 250)
//!     --time-limit <minutes>      conquest time limit; at expiry more
//!                                 tickets wins, a tie is a draw (default Off)
//!     --events <file.csv>         with --walk-replay: dump sim events
//!     --cheat <list>              dev cheats: invincible,ammo,clip,run,all (comma list)
//!     --spawn x,z                 explicit walk spawn (otherwise mission/post start)
//!     --headless                  no window; render offscreen
//!     --capture-frame <out.png>   headless-render one frame to PNG
//!     --hud                       headless: paint the HUD onto captures (walk mode)
//!     --walk-replay <log.ron>     headless walk: step an input log
//!     --ride-script <out.ron>     headless walk: script a ride (walk to the
//!                                 nearest boardable vehicle, enter, drive
//!                                 200 m, exit — game::ride), write its log,
//!                                 then replay it as --walk-replay does
//!     SQUAD_TRACE (env)           with --walk-replay: every 100 ticks, the
//!                                 units under a squad order, their distance
//!                                 from the player and the order (brief D14)
//!     --capture-series <n>,<dir>  with --walk-replay: PNG every n ticks +
//!                                 motion.csv into <dir>
//!     --capture-orbit <n>,<dir>   headless: n PNGs of a full orbit of the map
//!                                 (scripts/flythrough.sh drives this)
//!     --scene-report              headless: the map's framing plus every draw
//!                                 that resolves no texture; prints and exits
//!     --cam x,y,z,yaw,pitch       fly camera pose (degrees for yaw/pitch)
//!     --size WxH                  headless target size (default 1280x720)
//!     --no-cull                   submit every draw (frustum culling off)
//!     --cam-overview              frame the whole map from its baked bounds
//!     --bench                     measure load/tick/frame cost and exit
//!                                 (OPENBF_ADAPTER below picks the GPU)
//!     --bench-frames <n>          frames to time (default 240)
//!     --bench-ticks <n>           sim ticks to time (default 900)
//!     --bench-csv <file>          append the bench row to a CSV
//!     --bench-player-cam          bench from the player's eye, not the overview
//!   --write-pack <out> [--pak <file>] [--split-at <bytes>] <name>=<dir>...
//!                                 copy this executable to <out> with each
//!                                 <dir> appended as the archive's <name>/;
//!                                 at <bytes> (default about 3.9 GiB) the data
//!                                 goes to <file> (default openbf.pak beside
//!                                 <out>) instead (card PACK; scripts/pack.py)
//!
//! Environment — every variable is optional; unset means the default:
//!   OPENBF_TEXTURE_TIER=1|2       1 = the PS2 disc's own pixels, 2 = the PC
//!                                 tier where one was baked (default 2; the
//!                                 golden tests pin 1 themselves)
//!   OPENBF_FAR_CULL=1             clip the world at the map's scene ranges
//!                                 (default off — PLAN Log 2026-09-17, queue L)
//!   OPENBF_LOWRES=1               draw the `LOWR` far-LOD terrain sheet; it
//!                                 needs OPENBF_FAR_CULL=1 (default off)
//!   OPENBF_DEV=0                  turn the hidden dev mode OFF. It is ON by
//!                                 default while the game is unfinished
//!                                 (game::dev::DEFAULT_ON): the in-world squad
//!                                 marker and Hold cross, and the playtest
//!                                 black box (game::blackbox): F9, quitting,
//!                                 a panic and the stuck alarm write the last
//!                                 60 s to artifacts/playtest/last.json
//!   OPENBF_BLACKBOX=0             no black box in dev mode
//!   OPENBF_MUSIC_VOLUME=<0..1>    the music volume for this run, over the
//!                                 profile's (1 = the disc record's own
//!                                 gain, 0 = no music; card MUSV)
//!   OPENBF_BLACKBOX_DIR=<dir>     its folder (default artifacts/playtest)
//!   OPENBF_BLACKBOX_PANIC=<tick>  panic at that sim tick (the check that a
//!                                 panic writes a snapshot)
//!   OPENBF_SQUAD_MARKER=1|0       that marker alone: 1 shows it without dev
//!                                 mode, 0 hides it in dev mode
//!   OPENBF_DOORS_SOLID=1          BF2 doors never open: shut walls, as baked
//!                                 (default: they open for a soldier — DR2)
//!   OPENBF_PLNP_MISSION=0         skip the mission's start-of-round `plnp`
//!                                 route blocks (default on — queue DR3B)
//!   OPENBF_WATER_TIME=<seconds>   pin the water's clock (captures)
//!   OPENBF_WATER_RIPPLE=1         the stand-in flat-water ripple (default off)
//!   OPENBF_WATER_SPECKLE=0        no speckle layer on the water (default on)
//!   OPENBF_NO_FILTER=1            no DomeInfo frame filter (default: drawn)
//!   OPENBF_SPEC=0                 no specular second pass (default on)
//!   OPENBF_SPEC_MIRROR=0          the consistent specular light instead of
//!                                 the disc's mirrored one (default on)
//!   OPENBF_TERRAIN_LIGHTS=0       no BF2 terrain light pass (default on)
//!   OPENBF_CAMERA_CHUNKS=0        diagnostic: test the chase camera against
//!                                 every triangle of a blocker, no batch bounds
//!                                 (default on; same contacts, slower)
//!   OPENBF_BENCH_DETAIL=<out.json>  with --bench: raw per-tick CPU and
//!                                 per-frame rows to that file instead of the
//!                                 summary (--bench-frames 0 = sim only)
//!   OPENBF_BENCH_TOTAL_PER_SIDE=<any>  with OPENBF_BENCH_DETAIL: --ai <n>
//!                                 counts the human, n soldiers a side in all
//!   OPENBF_BENCH_FOREST_EYE=<any>  with OPENBF_BENCH_DETAIL: look at the
//!                                 first falling-leaf emitter, not the overview
//!   OPENBF_TERRAIN_LIGHT_GROUPS=0  with OPENBF_BENCH_DETAIL, diagnostic: the
//!                                 terrain light pass searches every rectangle,
//!                                 no group bounds (same pixels, slower)
//!   OPENBF_PROFILE=<file>         the profile file (default
//!                                 openbf_data/profile.ron beside the game)
//!   OPENBF_ADAPTER=igpu|dgpu|<name>  the GPU (default: high performance)
//!   OPENBF_GPU_TIMING=1           GPU ms per section of the frame (timestamp
//!                                 queries; `--bench` prints them, the
//!                                 frame profile writes them; card ISPD)
//!   OPENBF_FRAME_PROFILE=<csv>    windowed: a row of CPU and GPU ms per
//!                                 battle frame (game::frame_profile)
//!   OPENBF_NO_VSYNC=1             windowed: present without vsync
//!   OPENBF_WINDOW_SIZE=WxH        windowed: the first window size in
//!                                 physical pixels (default 1280x720 logical)
//!   OPENBF_BF1_WALKER_PACE=1      BF1 walkers on their own gait (vehicle_class::walker)
//!   OPENBF_FLYER_BOTS=1           bots board and fly flyers on the ground maps
//!                                 (default off: it changes those maps' hashes)
//!   OPENBF_AUTOPILOT=1            dev mode, windowed: a scripted pad drives the
//!                                 player (card HITCH; default off)
//!   OPENBF_QUIT_AT=<seconds>      dev mode, windowed: quit when the battle
//!                                 clock reaches it (default never)
//!   OPENBF_FRAME_RECORD=0         no frame-time record in dev mode (default on;
//!                                 game::frame_profile)
//!   OPENBF_LOD=0|ps2              far-scene LOWD switch: 0 = off, ps2 = the
//!                                 PS2's distance (default classic-plus; card LOD)
//!   OPENBF_NO_BLOOM=<any>         no bloom pass (default: the map's HDR bloom)
//!   OPENBF_RENDER_BLEND=0         windowed: draw ticks, no blend between them
//!                                 (default on; game::render_blend)
//!   OPENBF_MATCHLOG=0             no match log in dev mode (default on;
//!                                 game::matchlog)
//!   OPENBF_SHELL_SOUNDS=1         log each shell click and take to stderr
//!                                 (default off)
//!   MUSIC_LOG=<team>              headless: log the music manager's changes
//!                                 for that team; OPENBF_SOUND_ROOT=<dir> reads
//!                                 a scratch bake's `<map>/sounds`
//!   SQUAD_TRACE=<any>             with --walk-replay: the squad trace above
//!                                 (set at all = on, even "0")

// Split by queue SPL2 (2026-09-23, a pure move). This file keeps the CLI:
// `main`, the flag parsers, and `ViewerArgs` and `BattleSetup`, which every
// mode reads. The modes live in `main/`: `battle` (the battle every mode
// builds and shows), `headless` (`--bench` and every `--headless` run) and
// `windowed` (the shell and the windowed battle). A binary's root module owns
// `src/`, where the library's modules live, so each part is loaded by
// `#[path]`.
#[path = "main/battle.rs"]
mod battle;
#[path = "main/headless.rs"]
mod headless;
#[path = "main/sim_trace.rs"]
mod sim_trace;
#[path = "main/windowed.rs"]
mod windowed;

use anyhow::{anyhow, bail, Context, Result};
use core::sim::{InputLog, PadState, Sim};
use game::viewer::GOLDEN_CAM;
use headless::{run_bench, run_headless};
use std::path::PathBuf;
use windowed::{run_shell, run_windowed};

#[derive(Clone)]
struct ViewerArgs {
    map: PathBuf,
    headless: bool,
    capture_frame: Option<PathBuf>,
    walk: bool,
    targets: bool,
    seed: u32,
    /// `--difficulty <easy|normal|hard|elite>`: the round's tier. `None` =
    /// take the profile's (PLAN "PHASE C DESIGN" decision 4).
    difficulty: Option<core::skill::Difficulty>,
    cheats: core::sim::Cheats,
    spawn: (f32, f32),
    spawn_set: bool,
    ai: Option<u32>,
    ai_cap: Option<u32>,
    tickets: u32,
    tickets_set: bool,
    /// `--time-limit <minutes>`: the conquest time limit; 0 = Off (default).
    time_limit: u32,
    events: Option<PathBuf>,
    cam_set: bool,
    cam_overview: bool,
    hud: bool,
    mode: String,
    teams: Option<[String; 2]>,
    classes: Option<PathBuf>,
    mission: Option<PathBuf>,
    walk_replay: Option<PathBuf>,
    ride_script: Option<PathBuf>,
    capture_series: Option<(u64, PathBuf)>,
    /// `--capture-orbit <frames>,<dir>`: headless, one PNG per frame of a
    /// full circle around the map, framed by the same `overview_camera`
    /// logic the gallery uses. The Tier-4 flythrough path.
    capture_orbit: Option<(u32, PathBuf)>,
    cam: (f32, f32, f32, f32, f32),
    size: (u32, u32),
    /// `--bench`: measure load/tick/frame cost instead of playing. See
    /// game::bench; `OPENBF_ADAPTER` selects which GPU is measured.
    bench: bool,
    bench_frames: u32,
    bench_ticks: u32,
    bench_csv: Option<PathBuf>,
    /// `--no-cull`: submit every draw, so culling's effect can be measured on
    /// the same scene rather than argued about.
    no_cull: bool,
    /// `--scene-report`: headless census of what a map actually draws —
    /// framing, orbit occlusion, and every draw that resolves NO texture and
    /// falls back to a flat colour. Prints and exits; renders nothing.
    scene_report: bool,
    /// `--bench-player-cam`: measure from the player's eye instead of the
    /// auto-framed overview. The overview sees the WHOLE map, so it is the
    /// one camera where frustum culling can never help — measuring culling
    /// from it would be measuring nothing.
    bench_player_cam: bool,
}

fn main() -> Result<()> {
    // A packed build (card PACK) serves the default data directories from its
    // own file when they are not directories here; a dev checkout reads them.
    pak::mount(std::path::Path::new("assets_baked"), "assets_baked");
    pak::mount(std::path::Path::new("missions"), "missions");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    let a = it.next().cloned().unwrap_or_else(|| "--shell".to_string());
    match a.as_str() {
        "--shell" => {
            let mut assets = PathBuf::from("assets_baked");
            let mut missions = PathBuf::from("missions");
            while let Some(f) = it.next() {
                match f.as_str() {
                    "--assets" => assets = PathBuf::from(it.next().context("--assets <dir>")?),
                    "--missions" => {
                        missions = PathBuf::from(it.next().context("--missions <dir>")?)
                    }
                    other => bail!("unknown shell flag: {other}"),
                }
            }
            run_shell(assets, missions)
        }
        "--write-pack" => {
            // Card PACK (`scripts/pack.py` runs it): copy this executable to
            // <out> with each <name>=<dir> appended as the archive's <name>/.
            let usage = "--write-pack <out> [--pak <file>] [--split-at <bytes>] <name>=<dir>...";
            let out = PathBuf::from(it.next().context(usage)?);
            let mut pak_file = out.with_file_name(pak::PAK_NAME);
            let mut split_at = pak::SINGLE_FILE_LIMIT;
            let mut entries = Vec::new();
            while let Some(spec) = it.next() {
                match spec.as_str() {
                    "--pak" => pak_file = PathBuf::from(it.next().context(usage)?),
                    "--split-at" => {
                        split_at = it.next().context(usage)?.parse().context("--split-at")?
                    }
                    _ => {
                        let (name, dir) = spec.split_once('=').context(usage)?;
                        entries.extend(
                            pak::collect(std::path::Path::new(dir), name)
                                .with_context(|| dir.to_string())?,
                        );
                    }
                }
            }
            let exe = std::env::current_exe()?;
            let layout = pak::write_pack_at(&exe, &out, &pak_file, &entries, split_at)
                .with_context(|| out.display().to_string())?;
            match layout {
                pak::PackLayout::SingleFile => {
                    println!("packed {} files into {}", entries.len(), out.display())
                }
                pak::PackLayout::Beside => println!(
                    "packed {} files into {} beside {}",
                    entries.len(),
                    pak_file.display(),
                    out.display()
                ),
            }
            Ok(())
        }
        "--aim-replay" => headless::run_aim_replay(&args[1..]),
        "--sim-trace" => sim_trace::run(&args[1..]),
        "--sim-hash" => {
            let n: u64 = it
                .next()
                .context("--sim-hash <ticks>")?
                .parse()
                .context("tick count")?;
            let mut sim = Sim::new(42);
            let idle = [PadState::default()];
            for _ in 0..n {
                sim.step(&idle);
            }
            println!("{:016x}", sim.hash());
            Ok(())
        }
        "--replay" => {
            let path = it.next().context("--replay <log.ron>")?;
            let log: InputLog =
                ron::from_str(&std::fs::read_to_string(path)?).context("parse input log")?;
            println!("{:016x}", Sim::run_log(&log));
            Ok(())
        }
        "--map" => {
            let mut v = ViewerArgs {
                map: PathBuf::from(it.next().context("--map <baked-dir>")?),
                headless: false,
                capture_frame: None,
                walk: false,
                targets: false,
                seed: 42,
                difficulty: None,
                cheats: core::sim::Cheats::default(),
                spawn: (-140.0, 230.0),
                spawn_set: false,
                bench: false,
                bench_frames: 240,
                bench_ticks: 900,
                bench_csv: None,
                no_cull: false,
                scene_report: false,
                bench_player_cam: false,
                ai: None,
                ai_cap: None,
                tickets: 250,
                tickets_set: false,
                time_limit: 0,
                events: None,
                cam_set: false,
                cam_overview: false,
                hud: false,
                mode: "con".to_string(),
                teams: None,
                classes: None,
                mission: None,
                walk_replay: None,
                ride_script: None,
                capture_series: None,
                capture_orbit: None,
                cam: GOLDEN_CAM,
                size: (1280, 720),
            };
            while let Some(f) = it.next() {
                match f.as_str() {
                    "--headless" => v.headless = true,
                    "--walk" => v.walk = true,
                    "--targets" => v.targets = true,
                    "--hud" => v.hud = true,
                    "--seed" => v.seed = it.next().context("--seed <n>")?.parse()?,
                    "--difficulty" => {
                        let a = it.next().context("--difficulty <easy|normal|hard|elite>")?;
                        v.difficulty = Some(
                            core::skill::Difficulty::parse(a)
                                .with_context(|| format!("--difficulty: unknown tier {a:?}"))?,
                        );
                    }
                    "--ai" => v.ai = Some(it.next().context("--ai <n>")?.parse()?),
                    "--ai-cap" => {
                        let n: u32 = it.next().context("--ai-cap <n>")?.parse()?;
                        if n == 0 {
                            bail!("--ai-cap 0: a round with no live AI is not a workload");
                        }
                        v.ai_cap = Some(n);
                    }
                    "--tickets" => {
                        v.tickets = it.next().context("--tickets <n>")?.parse()?;
                        if v.tickets == 0 {
                            bail!(
                                "--tickets 0: a round with no reinforcements cannot spawn anyone"
                            );
                        }
                        v.tickets_set = true;
                    }
                    "--time-limit" => {
                        v.time_limit = it.next().context("--time-limit <minutes>")?.parse()?
                    }
                    "--mode" => v.mode = it.next().context("--mode <name>")?.to_string(),
                    "--teams" => {
                        let arg = it.next().context("--teams a,b")?;
                        let (a, b) = arg.split_once(',').context("--teams a,b")?;
                        v.teams = Some([a.trim().to_string(), b.trim().to_string()]);
                    }
                    "--classes" => {
                        v.classes = Some(PathBuf::from(it.next().context("--classes <dir>")?))
                    }
                    "--mission" => {
                        v.mission = Some(PathBuf::from(it.next().context("--mission <file>")?))
                    }
                    "--events" => {
                        v.events = Some(PathBuf::from(it.next().context("--events <file>")?))
                    }
                    "--cheat" => v.cheats = parse_cheats(it.next().context("--cheat <list>")?)?,
                    "--capture-frame" => {
                        v.capture_frame = Some(PathBuf::from(
                            it.next().context("--capture-frame <out.png>")?,
                        ));
                    }
                    "--walk-replay" => {
                        v.walk_replay =
                            Some(PathBuf::from(it.next().context("--walk-replay <log.ron>")?));
                    }
                    "--ride-script" => {
                        v.ride_script =
                            Some(PathBuf::from(it.next().context("--ride-script <out.ron>")?));
                    }
                    "--capture-series" => {
                        let arg = it.next().context("--capture-series <n>,<dir>")?;
                        let (n, dir) = arg.split_once(',').context("--capture-series <n>,<dir>")?;
                        v.capture_series =
                            Some((n.parse().context("series interval")?, dir.into()));
                    }
                    "--spawn" => {
                        let arg = it.next().context("--spawn x,z")?;
                        let (x, z) = arg.split_once(',').context("--spawn x,z")?;
                        v.spawn = (x.trim().parse()?, z.trim().parse()?);
                        v.spawn_set = true;
                    }
                    "--cam" => {
                        v.cam = parse_cam(it.next().context("--cam x,y,z,yaw,pitch")?)?;
                        v.cam_set = true;
                    }
                    // Frame the whole map from its baked bounds (map bring-up).
                    "--cam-overview" => v.cam_overview = true,
                    "--capture-orbit" => {
                        let a = it.next().context("--capture-orbit <frames>,<dir>")?;
                        let (n, dir) = a
                            .split_once(',')
                            .context("--capture-orbit <frames>,<dir>")?;
                        v.capture_orbit =
                            Some((n.parse().context("orbit frame count")?, PathBuf::from(dir)));
                        v.headless = true;
                    }
                    "--scene-report" => {
                        v.scene_report = true;
                        v.headless = true;
                    }
                    "--bench" => v.bench = true,
                    "--no-cull" => v.no_cull = true,
                    "--bench-player-cam" => v.bench_player_cam = true,
                    "--bench-frames" => {
                        v.bench_frames = it.next().context("--bench-frames <n>")?.parse()?
                    }
                    "--bench-ticks" => {
                        v.bench_ticks = it.next().context("--bench-ticks <n>")?.parse()?
                    }
                    "--bench-csv" => {
                        v.bench_csv = Some(PathBuf::from(it.next().context("--bench-csv <f>")?))
                    }
                    "--size" => v.size = parse_size(it.next().context("--size WxH")?)?,
                    other => bail!("unknown viewer flag: {other}"),
                }
            }
            if v.capture_frame.is_some() && !v.headless {
                bail!("--capture-frame requires --headless");
            }
            if v.bench {
                run_bench(&v)
            } else if v.headless {
                run_headless(&v)
            } else {
                run_windowed(v)
            }
        }
        other => bail!("unknown flag: {other}"),
    }
}

fn parse_cam(s: &str) -> Result<(f32, f32, f32, f32, f32)> {
    let p: Vec<f32> = s
        .split(',')
        .map(|t| t.trim().parse::<f32>())
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow!("--cam: {e}"))?;
    if p.len() != 5 {
        bail!(
            "--cam wants x,y,z,yaw,pitch (degrees), got {} values",
            p.len()
        );
    }
    Ok((p[0], p[1], p[2], p[3], p[4]))
}

fn parse_cheats(s: &str) -> Result<core::sim::Cheats> {
    let mut c = core::sim::Cheats::default();
    for item in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match item {
            "invincible" | "god" => c.invincible = true,
            "ammo" | "infinite_ammo" => c.infinite_ammo = true,
            "clip" | "no_reload" => c.no_reload = true,
            "run" | "stamina" => c.infinite_stamina = true,
            "all" | "classes" => c.all_classes = true,
            "awards" => c.all_awards = true,
            other => bail!("unknown cheat: {other} (invincible, ammo, clip, run, all, awards)"),
        }
    }
    Ok(c)
}

fn parse_size(s: &str) -> Result<(u32, u32)> {
    let (w, h) = s.split_once(['x', 'X']).context("--size WxH")?;
    Ok((w.parse()?, h.parse()?))
}

/// Sim setup shared by the windowed and headless walk/battle paths.
#[derive(Clone)]
struct BattleSetup {
    seed: u32,
    /// The round's difficulty tier (D2). `--difficulty` wins over the
    /// profile; `None` at neither is `Difficulty::default()` (Normal).
    difficulty: core::skill::Difficulty,
    spawn: (f32, f32),
    spawn_set: bool,
    targets: bool,
    ai: Option<u32>,
    /// Explicit CLI alive-AI limit for stress runs. Interactive battles use
    /// the map-relative combatant percentage; other CLI launches use the mission.
    ai_cap: Option<u32>,
    team_size_pct: Option<u32>,
    tickets: u32,
    /// Explicit --tickets wins over the mission's (and ignores
    /// `ticket_pct`).
    tickets_set: bool,
    /// The shell's reinforcement % (`con_mult`, 10..500, default 100):
    /// scales each team's calibrated map count (mission count on the legacy CLI),
    /// or `tickets` without a mission
    /// (queue TLO, `core::sim::scale_reinforcements`).
    ticket_pct: u32,
    /// The conquest time limit in minutes; 0 = Off (queue TLO).
    time_limit: u32,
    cheats: core::sim::Cheats,
    teams: Option<[String; 2]>,
    classes_dir: PathBuf,
    mission: Option<game::mission::MissionDef>,
    /// The mission's file name, for the black box header (card ERAM).
    mission_file: Option<String>,
    /// The sim team the player's army sits on: 1 when the shell picked a
    /// mission's own pair the other way round (`game::mission::pick`).
    player_team: u8,
    /// The session's hero rules (`game::heroes::rules`); `None` = Heroes
    /// Off, no hero installed. The CLI takes the retail defaults.
    heroes: Option<core::sim::heroes::HeroRules>,
}

/// A path's last component as text.
fn file_name(p: &std::path::Path) -> Option<String> {
    p.file_name().map(|n| n.to_string_lossy().into_owned())
}

impl BattleSetup {
    fn from_args(v: &ViewerArgs, seed: u32) -> Result<Self> {
        let mission = v
            .mission
            .as_deref()
            .map(game::mission::MissionDef::load)
            .transpose()?;
        // `--teams` naming the mission's own pair the other way round seats
        // the first army on its own team, as the shell does (card ERAM).
        let own_seat = match (&mission, &v.teams) {
            (Some(m), Some(t)) => {
                game::mission::pick_index(&[m.factions()], [t[0].as_str(), t[1].as_str()])
                    .is_some_and(|(_, seat, _)| seat == 1)
            }
            _ => false,
        };
        Ok(Self {
            seed,
            difficulty: v.difficulty.unwrap_or_default(),
            spawn: v.spawn,
            spawn_set: v.spawn_set,
            targets: v.targets,
            ai: v.ai,
            ai_cap: v.ai_cap,
            team_size_pct: None,
            tickets: v.tickets,
            tickets_set: v.tickets_set,
            ticket_pct: 100,
            time_limit: v.time_limit,
            cheats: v.cheats,
            teams: v
                .teams
                .clone()
                .filter(|_| !own_seat)
                .or_else(|| mission.as_ref().map(|m| m.factions())),
            mission,
            mission_file: v.mission.as_deref().and_then(file_name),
            player_team: u8::from(own_seat),
            heroes: Some(core::sim::heroes::HeroRules::default()),
            classes_dir: v.classes.clone().unwrap_or_else(|| {
                v.map
                    .parent()
                    .map(|p| p.join("classes"))
                    .unwrap_or_else(|| PathBuf::from("classes"))
            }),
        })
    }
}
