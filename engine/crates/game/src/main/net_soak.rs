//! `game --net-soak` (issue #168, LAN §9): several lockstep peers in one
//! process on a real baked map, in virtual time over `net::LinkSim`, with a
//! scripted round of joins, a cut link, a reconnect and cheat traffic. Every
//! peer's `Sim::hash` is checked at every tick it steps; the run prints a
//! JSON summary and exits non-zero on any failed check.
//!
//! Flags (defaults in brackets):
//!   --map KEY [bf1_tat2]  --mode M [con]  --teams a,b [by game]
//!   --assets DIR [assets_baked]  --missions DIR [missions]
//!   --ticks N [3000]  --seed S [1]  --peers N [4: host + 3]
//!   --latency MS [40]  --jitter MS [10]  --loss F [0.03]
//!   --join-at T [600]  --cheat-at T [900]  --request-at T [1000]
//!   --deny-at T [1100]  --cut-at T [1200]  --rejoin-at T [1700]
//!   --budget N [300]: a joiner's steps per frame while catching up
//!   --out FILE: also write the summary there
//!   --predict: every peer also runs the local prediction each frame (as
//!     the window does) and the summary reports its cost and depth
//!   --peer-assets A,B,..  --peer-missions M,N,..: each peer loads its own
//!     copy of the game (node 0 = the host takes the first entry, guest i
//!     the i-th; guests past the list cycle through entries 1..), e.g. a
//!     PS2 host with PC and Xbox guests; unset, every peer uses --assets
//!     and --missions
//!   --report-dir DIR [the black box's desync/]: where desync reports go
//!   --perturb-at T: the instrument check; at host tick T guest 1's sim is
//!     nudged (a unit moved 1 cm), so the run must report mismatches and a
//!     desync (and that guest rejoins); exits non-zero like any failure
//! Events are at host ticks; a time past --ticks is skipped. The last
//! guest joins mid-round, guest 1 asks for cheats, guest 2 is cut and
//! reconnects (with 2 or fewer guests the roles fold onto the ones there).

use anyhow::{bail, Context, Result};
use core::fmath::FloatExt;
use game::netplay::virtual_net::{Builder, VirtualNet};
use game::netplay::{cheat_index, GameCommand, GameRequest, HumanSeat, PlayerProfile, RoundSetup};
use net::{LinkParams, Micros};
use std::path::PathBuf;
use std::rc::Rc;

/// The soak's knobs.
#[derive(Clone, Debug)]
pub(super) struct SoakArgs {
    pub map: String,
    pub mode: String,
    pub teams: Option<[String; 2]>,
    pub assets: PathBuf,
    pub missions: PathBuf,
    pub ticks: u32,
    pub seed: u64,
    pub peers: usize,
    pub latency_ms: f64,
    pub jitter_ms: f64,
    pub loss: f64,
    pub join_at: u32,
    pub cheat_at: u32,
    pub request_at: u32,
    pub deny_at: u32,
    pub cut_at: u32,
    pub rejoin_at: u32,
    pub budget: usize,
    pub out: Option<PathBuf>,
    pub perturb_at: Option<u32>,
    pub predict: bool,
    pub peer_assets: Vec<PathBuf>,
    pub peer_missions: Vec<PathBuf>,
    pub report_dir: Option<PathBuf>,
}

impl Default for SoakArgs {
    fn default() -> Self {
        Self {
            map: "bf1_tat2".into(),
            mode: "con".into(),
            teams: None,
            assets: "assets_baked".into(),
            missions: "missions".into(),
            ticks: 3000,
            seed: 1,
            peers: 4,
            latency_ms: 40.0,
            jitter_ms: 10.0,
            loss: 0.03,
            join_at: 600,
            cheat_at: 900,
            request_at: 1000,
            deny_at: 1100,
            cut_at: 1200,
            rejoin_at: 1700,
            budget: 300,
            out: None,
            perturb_at: None,
            predict: false,
            peer_assets: Vec::new(),
            peer_missions: Vec::new(),
            report_dir: None,
        }
    }
}

impl SoakArgs {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut a = Self::default();
        let mut it = args.iter();
        while let Some(f) = it.next() {
            let mut val = || it.next().with_context(|| format!("{f} <value>"));
            match f.as_str() {
                "--map" => a.map = val()?.clone(),
                "--mode" => a.mode = val()?.clone(),
                "--teams" => {
                    let v = val()?;
                    let (x, y) = v.split_once(',').context("--teams a,b")?;
                    a.teams = Some([x.trim().into(), y.trim().into()]);
                }
                "--assets" => a.assets = val()?.into(),
                "--missions" => a.missions = val()?.into(),
                "--ticks" => a.ticks = val()?.parse()?,
                "--seed" => a.seed = val()?.parse()?,
                "--peers" => a.peers = val()?.parse()?,
                "--latency" => a.latency_ms = val()?.parse()?,
                "--jitter" => a.jitter_ms = val()?.parse()?,
                "--loss" => a.loss = val()?.parse()?,
                "--join-at" => a.join_at = val()?.parse()?,
                "--cheat-at" => a.cheat_at = val()?.parse()?,
                "--request-at" => a.request_at = val()?.parse()?,
                "--deny-at" => a.deny_at = val()?.parse()?,
                "--cut-at" => a.cut_at = val()?.parse()?,
                "--rejoin-at" => a.rejoin_at = val()?.parse()?,
                "--budget" => a.budget = val()?.parse()?,
                "--out" => a.out = Some(val()?.into()),
                "--perturb-at" => a.perturb_at = Some(val()?.parse()?),
                "--predict" => a.predict = true,
                "--peer-assets" => a.peer_assets = val()?.split(',').map(PathBuf::from).collect(),
                "--peer-missions" => {
                    a.peer_missions = val()?.split(',').map(PathBuf::from).collect()
                }
                "--report-dir" => a.report_dir = Some(val()?.into()),
                other => bail!("unknown --net-soak flag: {other}"),
            }
        }
        anyhow::ensure!((2..=16).contains(&a.peers), "--peers wants 2..=16");
        anyhow::ensure!(
            a.peer_missions.is_empty() || a.peer_missions.len() == a.peer_assets.len(),
            "--peer-missions wants one entry per --peer-assets entry"
        );
        if let Some(first) = a.peer_assets.first() {
            a.assets = first.clone();
        }
        if let Some(first) = a.peer_missions.first() {
            a.missions = first.clone();
        }
        Ok(a)
    }
}

/// The default armies of a map's game: the Galactic Civil War on BF1, the
/// Clone Wars on BF2.
pub(super) fn default_armies(source_game: &str) -> [String; 2] {
    if source_game.eq_ignore_ascii_case("bf1") {
        ["all".into(), "imp".into()]
    } else {
        ["rep".into(), "cis".into()]
    }
}

/// A scripted player's profile: a few medals and unlock kills varied by
/// slot, so per-slot profile inputs differ between humans.
pub(super) fn scripted_profile(slot: u8) -> PlayerProfile {
    let k = u32::from(slot);
    let mut p = PlayerProfile {
        name: format!("Peer{slot}"),
        ..PlayerProfile::default()
    };
    p.medals[0] = k;
    p.unlock_kills = std::array::from_fn(|i| (i as u32 * 13 + k * 31) % 120);
    p.assist.intensity = 0.5 + 0.1 * (k % 5) as f32;
    p
}

/// The builder every peer of a run uses: the map loaded once, each round
/// built from its setup through `round_setup::build_round`.
pub(super) fn builder(
    scene: Rc<game::assets::Scene>,
    assets: PathBuf,
    missions: PathBuf,
) -> Builder {
    Box::new(move |setup: &RoundSetup, local: u8, _node: usize| {
        let walk = super::round_setup::build_round(&scene, setup, &missions, &assets, local)?;
        let feed =
            game::event_feed::EventFeed::for_walk(&assets, &scene.manifest.source_game, &walk);
        Ok((walk, Some(feed)))
    })
}

/// The copy of the game node `node` loads: the host the first entry, guest
/// i the i-th, later guests cycling through the guests' entries.
pub(super) fn copy_of(node: usize, copies: usize) -> usize {
    if node < copies || copies <= 1 {
        node.min(copies.saturating_sub(1))
    } else {
        1 + (node - 1) % (copies - 1)
    }
}

/// A builder whose peers load different copies of the game (`--peer-assets`):
/// each copy's map scene is loaded once, on the first peer that needs it.
fn mixed_builder(
    map: String,
    mode: String,
    first: Rc<game::assets::Scene>,
    copies: Vec<(PathBuf, PathBuf)>,
) -> Builder {
    let mut scenes: Vec<Option<Rc<game::assets::Scene>>> = vec![None; copies.len()];
    scenes[0] = Some(first);
    Box::new(move |setup: &RoundSetup, local: u8, node: usize| {
        let c = copy_of(node, copies.len());
        let (assets, missions) = &copies[c];
        if scenes[c].is_none() {
            let dir = assets.join(&map);
            let (scene, load) = game::assets::load_battle_scene(&dir, &mode)
                .with_context(|| format!("load {}", dir.display()))?;
            if let Some((path, Err(error))) = load.common {
                bail!("common assets at {} unreadable: {error:#}", path.display());
            }
            scenes[c] = Some(Rc::new(scene));
        }
        let scene = scenes[c].as_ref().expect("loaded");
        let walk = super::round_setup::build_round(scene, setup, missions, assets, local)?;
        let feed =
            game::event_feed::EventFeed::for_walk(assets, &scene.manifest.source_game, &walk);
        Ok((walk, Some(feed)))
    })
}

fn stats_ms(v: &[f64]) -> serde_json::Value {
    if v.is_empty() {
        return serde_json::json!({ "count": 0 });
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let pct = |p: f64| s[((s.len() - 1) as f64 * p).round() as usize];
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    serde_json::json!({
        "count": s.len(),
        "mean": (mean * 10.0).round() / 10.0,
        "p50": pct(0.5),
        "p95": pct(0.95),
        "max": s[s.len() - 1],
    })
}

pub(super) fn run(args: &[String]) -> Result<()> {
    let mut a = SoakArgs::parse(args)?;
    let perturbed = a.perturb_at;
    let wall = std::time::Instant::now();
    let dir = a.assets.join(&a.map);
    let (scene, load) = game::assets::load_battle_scene(&dir, &a.mode)
        .with_context(|| format!("load {}", dir.display()))?;
    if let Some((path, Err(error))) = load.common {
        bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let scene = Rc::new(scene);
    let load_ms = wall.elapsed().as_secs_f64() * 1e3;
    let armies = a
        .teams
        .clone()
        .unwrap_or_else(|| default_armies(&scene.manifest.source_game));
    let link = LinkParams {
        latency: Micros((a.latency_ms * 1000.0) as u64),
        jitter: Micros((a.jitter_ms * 1000.0) as u64),
        loss: a.loss,
        duplicate: 0.0,
        reorder: 0.0,
    };
    let build = game::netplay::build_id();
    let mut vnet = VirtualNet::new(
        a.seed,
        link,
        &build,
        scripted_profile(0).encode(),
        if a.peer_assets.len() > 1 {
            let copies = a
                .peer_assets
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let m = a.peer_missions.get(i).unwrap_or(&a.missions);
                    (p.clone(), m.clone())
                })
                .collect();
            mixed_builder(a.map.clone(), a.mode.clone(), scene.clone(), copies)
        } else {
            builder(scene.clone(), a.assets.clone(), a.missions.clone())
        },
    );
    vnet.predict = a.predict;
    let guests = a.peers - 1;
    let late = guests; // node index of the mid-round joiner
    let starters = if guests >= 2 { guests - 1 } else { guests };
    for i in 1..=starters {
        vnet.join(
            &format!("Peer{i}"),
            100 + i as u64,
            scripted_profile(i as u8).encode(),
            16.7 + i as f64 * 1.3,
        );
    }
    anyhow::ensure!(
        vnet.run_until(20_000, |v| v.nodes[1..]
            .iter()
            .all(|n| n.peer.local_slot != 0)),
        "guests were not welcomed"
    );
    let mut setup = RoundSetup::instant_action(
        &a.map,
        &a.mode,
        armies.clone(),
        &ui::shell::SessionOptions::default(),
        &game::profile::Profile::default(),
    );
    let stamina = u32::from(cheat_index("stamina").expect("stamina"));
    let clip = u32::from(cheat_index("clip").expect("clip"));
    setup.cheat_policy = 1 << stamina | 1 << clip;
    for slot in 0..=starters as u8 {
        setup.humans.push(HumanSeat {
            slot,
            name: format!("Peer{slot}"),
            team: None,
            cheats: 0,
            profile: scripted_profile(slot),
        });
    }
    vnet.host_load(&setup);
    let report_dir = a
        .report_dir
        .clone()
        .unwrap_or_else(game::netplay::desync::default_dir);
    for n in &mut vnet.nodes {
        n.peer.catch_up_budget = a.budget;
        n.peer.report_dir = Some(report_dir.clone());
    }
    let invincible = u32::from(cheat_index("invincible").expect("invincible"));
    let ammo = u32::from(cheat_index("ammo").expect("ammo"));
    let classes = cheat_index("classes").expect("classes");
    let asker = 1usize;
    let cut = if guests >= 2 { 2usize } else { 1 };
    let mut done = [false; 6];
    let mut joiner_pos: Option<(glam::Vec3, f32)> = None;
    let mut joiner_alive_ticks = 0u32;
    // Slot 1's and the asker's cheats once both requests had time to land.
    let mut cheat_sample: Option<(u32, u32)> = None;
    // The guest tick the perturbation's nudge first shows in.
    let mut nudged_tick: Option<u32> = None;
    let target = a.ticks;
    let limit_ms = u64::from(target) * 34 + 120_000;
    let mut ms = 0u64;
    while vnet.host_tick() < target && ms < limit_ms {
        let t = vnet.host_tick();
        if !done[0] && t >= a.join_at && guests >= 2 {
            done[0] = true;
            vnet.join("Late", 999, scripted_profile(late as u8).encode(), 15.0);
            for n in &mut vnet.nodes {
                n.peer.catch_up_budget = a.budget;
                n.peer.report_dir = Some(report_dir.clone());
            }
        }
        if !done[1] && t >= a.cheat_at {
            done[1] = true;
            vnet.host_command(&GameCommand::SetSlotCheats {
                slot: 1,
                bits: 1 << invincible | 1 << ammo,
            });
        }
        if !done[2] && t >= a.request_at {
            done[2] = true;
            vnet.request(
                asker,
                &GameRequest::CheatToggle {
                    id: stamina as u8,
                    on: true,
                },
            );
        }
        if !done[3] && t >= a.deny_at {
            done[3] = true;
            vnet.request(
                asker,
                &GameRequest::CheatToggle {
                    id: classes,
                    on: true,
                },
            );
        }
        if !done[4] && t >= a.cut_at {
            done[4] = true;
            vnet.cut(cut, true);
        }
        if a.perturb_at.is_some_and(|p| t >= p) {
            if let Some(w) = vnet.nodes[1].walk.as_mut() {
                if let Some(u) = w.sim.units.iter_mut().find(|u| u.alive) {
                    u.soldier.pos.x += 0.01;
                    a.perturb_at = None;
                    nudged_tick = Some(vnet.nodes[1].peer.next_tick());
                }
            }
        }
        if !done[5] && t >= a.rejoin_at {
            done[5] = true;
            vnet.reconnect(cut, scripted_profile(cut as u8).encode());
            vnet.nodes[cut].peer.report_dir = Some(report_dir.clone());
        }
        vnet.step();
        ms += 1;
        if ms.is_multiple_of(33) {
            if let Some(w) = vnet.nodes[0].walk.as_ref() {
                if cheat_sample.is_none() && vnet.host_tick() >= a.deny_at.max(a.request_at) + 30 {
                    let slot = usize::from(vnet.nodes[asker].peer.local_slot);
                    cheat_sample = Some((
                        game::netplay::cheat_bits(&w.sim.cheats_for_slot(1)),
                        game::netplay::cheat_bits(&w.sim.cheats_for_slot(slot)),
                    ));
                }
                let slot = vnet.nodes.get(late).map(|n| n.peer.local_slot);
                if let Some(u) = slot
                    .filter(|_| done[0])
                    .and_then(|s| w.sim.players.get(usize::from(s)).copied().flatten())
                    .and_then(|h| w.sim.unit(h))
                {
                    joiner_alive_ticks += 1;
                    let p = u.soldier.pos;
                    joiner_pos = Some(match joiner_pos {
                        None => (p, 0.0),
                        Some((first, d)) => (first, d.fmax(first.distance(p))),
                    });
                }
            }
            if vnet.nodes[0].peer.mode() == game::netplay::Mode::Ended
                || vnet.nodes[0]
                    .walk
                    .as_ref()
                    .is_some_and(|w| w.sim.outcome.is_some())
            {
                break;
            }
        }
    }
    // Let the guests step what the host closed.
    let last = vnet.host_tick();
    vnet.run_until(3_000, |v| {
        v.nodes[1..]
            .iter()
            .filter(|n| n.walk.is_some() && n.peer.mode() == game::netplay::Mode::Playing)
            .all(|n| n.peer.next_tick() >= last)
    });
    a.perturb_at = perturbed;
    // Desync rows' cost (`netplay::desync::row_of`, every stepped tick of a
    // lockstep round): 300 rows of the host's final state.
    let row_us = vnet.nodes[0].walk.as_ref().map(|w| {
        let t = std::time::Instant::now();
        let mut x = 0u64;
        for _ in 0..300 {
            x ^= std::hint::black_box(game::netplay::desync::row_of(&w.sim))[1];
        }
        std::hint::black_box(x);
        t.elapsed().as_secs_f64() * 1e6 / 300.0
    });
    summarize(
        &a,
        &vnet,
        &scene.manifest.source_game,
        load_ms,
        wall,
        joiner_pos,
        joiner_alive_ticks,
        late,
        cut,
        asker,
        cheat_sample,
        (row_us, nudged_tick),
    )
}

#[allow(clippy::too_many_arguments)]
fn summarize(
    a: &SoakArgs,
    vnet: &VirtualNet,
    game_name: &str,
    load_ms: f64,
    wall: std::time::Instant,
    joiner: Option<(glam::Vec3, f32)>,
    joiner_alive_samples: u32,
    late: usize,
    cut_node: usize,
    asker_node: usize,
    cheat_sample: Option<(u32, u32)>,
    (row_us, nudged_tick): (Option<f64>, Option<u32>),
) -> Result<()> {
    let cut = usize::from(vnet.nodes[cut_node].peer.local_slot);
    let asker = vnet.nodes[asker_node].peer.local_slot;
    let secs = vnet.now.as_secs_f64().fmax(1e-3);
    let host = &vnet.nodes[0];
    let host_walk = host
        .walk
        .as_ref()
        .context("the host never built its round")?;
    let host_digest = host.builds.first().map(|b| b.0);
    let digests_equal = vnet
        .nodes
        .iter()
        .flat_map(|n| n.builds.iter())
        .all(|b| Some(b.0) == host_digest);
    let desyncs: u32 = vnet.nodes.iter().map(|n| n.peer.stats.desyncs).sum();
    let leave = host.applied.iter().find_map(|(t, ap)| match ap {
        game::netplay::Applied::Left { slot, bot } if usize::from(*slot) == cut => {
            Some((*t, bot.is_some()))
        }
        _ => None,
    });
    let joins: Vec<(u32, u8)> = host
        .applied
        .iter()
        .filter_map(|(t, ap)| match ap {
            game::netplay::Applied::Joined { slot, ok: true } => Some((*t, *slot)),
            _ => None,
        })
        .collect();
    let bits = |ids: &[&str]| -> u32 {
        ids.iter()
            .filter_map(|id| cheat_index(id))
            .fold(0, |b, i| b | 1 << i)
    };
    let (cheats1, cheats_asker) = cheat_sample.unwrap_or_default();
    let (want1, want_asker) = if asker == 1 {
        let b = bits(&["invincible", "ammo", "stamina"]);
        (b, b)
    } else {
        (bits(&["invincible", "ammo"]), bits(&["stamina"]))
    };
    let peers: Vec<serde_json::Value> = vnet
        .nodes
        .iter()
        .map(|n| {
            let cu: Vec<serde_json::Value> = n
                .catch_up
                .iter()
                .map(|(start, end, wall)| {
                    serde_json::json!({
                        "connect_to_caught_up_ms": end.map(|e| e.saturating_sub(n.connected_at).as_millis_f64()),
                        "load_to_caught_up_ms": end.map(|e| e.saturating_sub(*start).as_millis_f64()),
                        "step_wall_ms": (wall * 10.0).round() / 10.0,
                    })
                })
                .collect();
            serde_json::json!({
                "name": n.name,
                "slot": n.peer.local_slot,
                "mode": format!("{:?}", n.peer.mode()),
                "next_tick": n.peer.next_tick(),
                "builds": n.builds.len(),
                "build_wall_ms": n.build_ms.iter().map(|m| m.round()).collect::<Vec<_>>(),
                "build_error": n.build_error,
                "copy": (!a.peer_assets.is_empty()).then(|| {
                    let i = vnet.nodes.iter().position(|m| std::ptr::eq(m, n)).unwrap_or(0);
                    a.peer_assets[copy_of(i, a.peer_assets.len())].display().to_string()
                }),
                "static_digests": n.builds.iter().map(|b| format!("{:016x}", b.0)).collect::<Vec<_>>(),
                "input_latency_ms": stats_ms(&n.latencies_ms),
                "predict_ms": stats_ms(&n.predict_ms),
                "predict_depth": stats_ms(&n.predict_depth.iter().map(|d| f64::from(*d)).collect::<Vec<_>>()),
                "predict_off_frames": n.predict_off,
                "predict_corrections": n.predictor.as_ref().map_or(0, |p| p.stats.corrections),
                "catch_up": cu,
                "bytes_out_per_s": (n.bytes_out as f64 / secs).round(),
                "bytes_in_per_s": (n.bytes_in as f64 / secs).round(),
                "stats": n.peer.stats,
            })
        })
        .collect();
    let refused = host.peer.stats.refused;
    let checks = &vnet.checker;
    let joiner_moved = joiner.map_or(0.0, |j| j.1);
    let mut failures = Vec::new();
    if checks.mismatch_count > 0 {
        failures.push(format!("{} hash mismatches", checks.mismatch_count));
    }
    if desyncs > 0 {
        failures.push(format!("{desyncs} desync events"));
    }
    if !digests_equal {
        failures.push("static digests differ".to_owned());
    }
    if vnet.nodes.iter().any(|n| n.build_error.is_some()) {
        failures.push("a peer failed to build".to_owned());
    }
    if vnet.nodes.iter().any(|n| n.peer.stats.out_of_order > 0) {
        failures.push("bundles out of order".to_owned());
    }
    let ran = |t: u32| t < host_walk.sim.tick as u32;
    if ran(a.cut_at + 400) && leave.is_none() {
        failures.push("the cut guest was never removed".to_owned());
    }
    if cheat_sample.is_some() && cheats1 != want1 {
        failures.push(format!("slot 1 cheats {cheats1:#x}, want {want1:#x}"));
    }
    if cheat_sample.is_some() && cheats_asker != want_asker {
        failures.push(format!(
            "slot {asker} cheats {cheats_asker:#x}, want {want_asker:#x}"
        ));
    }
    if ran(a.deny_at + 30) && refused != 1 {
        failures.push(format!("{refused} requests refused, want 1"));
    }
    if a.peers >= 3 && ran(a.join_at + 900) && joiner_moved < 1.0 {
        failures.push(format!(
            "the joiner did not spawn and move ({joiner_moved:.1} m)"
        ));
    }
    if a.peers >= 3 && ran(a.rejoin_at + 900) {
        let back = joins
            .iter()
            .any(|(t, s)| usize::from(*s) == cut && *t > a.rejoin_at);
        if !back {
            failures.push("the reconnected guest was not added back".to_owned());
        }
    }
    let summary = serde_json::json!({
        "map": a.map,
        "game": game_name,
        "mode": a.mode,
        "build": game::netplay::build_id(),
        "target": game::netplay::target(),
        "ticks": host_walk.sim.tick,
        "outcome": host_walk.sim.outcome,
        "virtual_s": (secs * 10.0).round() / 10.0,
        "wall_s": (wall.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        "map_load_ms": load_ms.round(),
        "perturbed_at": a.perturb_at,
        "link": { "latency_ms": a.latency_ms, "jitter_ms": a.jitter_ms, "loss": a.loss },
        "peers": vnet.nodes.len(),
        "hash_checks": checks.checks,
        "mismatches": checks.mismatch_count,
        "first_mismatches": checks.mismatches,
        "desync_events": desyncs,
        "static_digest": host_digest.map(|d| format!("{d:016x}")),
        "static_digests_equal": digests_equal,
        "joins": joins,
        "leave": leave.map(|(t, bot)| serde_json::json!({"tick": t, "bot_took_over": bot})),
        "joiner": { "node": late, "alive_samples": joiner_alive_samples, "moved_m": (joiner_moved * 10.0).round() / 10.0 },
        "slot1_cheats": format!("{cheats1:#x}"),
        "asker_slot": asker,
        "asker_cheats": format!("{cheats_asker:#x}"),
        "cut_slot": cut,
        "requests_refused": refused,
        "net": { "sent": vnet.sim.sent, "lost": vnet.sim.lost, "delivered": vnet.sim.delivered },
        "desync_reports": vnet.nodes.iter().flat_map(|n| n.peer.desync_reports.iter().map(|p| p.display().to_string())).collect::<Vec<_>>(),
        "perturbed_guest_tick": nudged_tick,
        "desync_row_us": row_us.map(|u| (u * 10.0).round() / 10.0),
        "per_peer": peers,
        "failures": failures,
    });
    let text = serde_json::to_string_pretty(&summary)?;
    println!("{text}");
    if let Some(out) = &a.out {
        std::fs::write(out, &text).with_context(|| out.display().to_string())?;
    }
    if !failures.is_empty() {
        bail!("net soak failed: {}", failures.join("; "));
    }
    Ok(())
}
