//! `game --net-host` and `game --net-join ADDR` (issue #168, LAN §9): the
//! lockstep soak across real machines, headless. Real UDP through
//! `net::NetSession`, the real clock, a headless `WalkSim` per process and
//! scripted pads; a status line every 2 s (ping, input delay, hash checks)
//! and a JSON summary at the end. Run one host and any guests on machines
//! that reach each other (a LAN or a VPN) to validate before playing.
//!
//!   --net-host [--port P] [--map KEY] [--mode M] [--teams a,b]
//!              [--players N] [--wait S] [--seconds S] [--name NAME]
//!   --net-join ADDR[:PORT] [--seconds S] [--name NAME]
//!   both: [--assets DIR] [--missions DIR] [--locals N] [--away S]
//!
//! The host waits until `--players` humans (every machine's locals and
//! itself included, default 2) are connected or `--wait` seconds (default
//! 60) pass, starts the round and plays `--seconds` (default 60) of it, or
//! until the round's outcome (then it ends the round for everyone); a guest
//! that connects later joins mid-round. A guest plays `--seconds` from the
//! moment its round starts playing, or until the host ends; it gives up
//! `GIVE_UP` seconds after that budget if its round never starts.
//!
//! `--locals N` (1 to 4, default 1; issue #194): this machine plays N
//! players, each on its own scripted pad, as a splitscreen machine does (a
//! guest's `Hello` asks a slot for each, all or none; the host adds its own
//! with `HostCommand::AddLocal`). `--away S`: S seconds into the round the
//! machine's last local loses its pad for `AWAY_FOR` seconds, then gets it
//! back (`Peer::set_local_away`: the round seats it again as it left).
//!
//! The last line is the verdict ([`verdict`]): `PASS` with the number of
//! hash checks, or exit status 1 when nothing was compared (no other player,
//! no hash checks, a refused or lost connection) or a desync was reported.

use anyhow::{bail, Context, Result};
use game::netplay::{HumanSeat, Link, Mode, Notice, Peer, PeerStats, RoundSetup};
use net::{ClientConfig, ClientCore, HostConfig, HostCore, NetSession, UdpTransport};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_micros(16_667);
const STATUS_EVERY: Duration = Duration::from_secs(2);
/// A guest whose round has not started this long after its `--seconds`
/// stops waiting.
const GIVE_UP: f64 = 120.0;
/// How long an `--away` local's pad stays gone.
const AWAY_FOR: f64 = 10.0;
/// The host lingers this long after its round ended, so the guests step and
/// hash-check its last ticks before its link closes.
const LINGER: Duration = Duration::from_secs(3);

/// The soak's verdict: a pass needs another player, at least one hash
/// check, equal static digests and no desync. `trouble` says why the session
/// ended early, if it did; `others` is how many other players took part.
fn verdict(trouble: Option<&str>, others: usize, stats: &PeerStats) -> Result<String> {
    let (hash_reports, desyncs) = (stats.hash_reports, stats.desyncs);
    if let Some(t) = trouble {
        bail!("FAIL: the session ended early ({t}); nothing was confirmed");
    }
    if others == 0 {
        bail!("FAIL: no other player connected; nothing was compared");
    }
    if stats.digest_mismatches > 0 {
        bail!("FAIL: a copy of the map plays differently (static digests differ)");
    }
    if hash_reports == 0 {
        bail!("FAIL: no hash checks ran; nothing was compared");
    }
    if desyncs > 0 {
        bail!("FAIL: {desyncs} desyncs in {hash_reports} hash checks");
    }
    Ok(format!("PASS: {hash_reports} hash checks, 0 desyncs"))
}

struct Args {
    port: u16,
    map: String,
    mode: String,
    teams: Option<[String; 2]>,
    players: usize,
    wait: f64,
    seconds: f64,
    name: Option<String>,
    assets: PathBuf,
    missions: PathBuf,
    addr: Option<String>,
    locals: usize,
    away: Option<f64>,
}

fn parse(args: &[String], join: bool) -> Result<Args> {
    let mut a = Args {
        port: net::DEFAULT_PORT,
        map: "bf1_tat2".into(),
        mode: "con".into(),
        teams: None,
        players: 2,
        wait: 60.0,
        seconds: 60.0,
        name: None,
        assets: "assets_baked".into(),
        missions: "missions".into(),
        addr: None,
        locals: 1,
        away: None,
    };
    let mut it = args.iter();
    if join {
        a.addr = Some(it.next().context("--net-join ADDR[:PORT]")?.clone());
    }
    while let Some(f) = it.next() {
        let mut val = || it.next().with_context(|| format!("{f} <value>"));
        match f.as_str() {
            "--port" if !join => a.port = val()?.parse()?,
            "--map" if !join => a.map = val()?.clone(),
            "--mode" if !join => a.mode = val()?.clone(),
            "--teams" if !join => {
                let v = val()?;
                let (x, y) = v.split_once(',').context("--teams a,b")?;
                a.teams = Some([x.trim().into(), y.trim().into()]);
            }
            "--players" if !join => a.players = val()?.parse()?,
            "--wait" if !join => a.wait = val()?.parse()?,
            "--seconds" => a.seconds = val()?.parse()?,
            "--name" => a.name = Some(val()?.clone()),
            "--assets" => a.assets = val()?.into(),
            "--missions" => a.missions = val()?.into(),
            "--locals" => {
                a.locals = val()?.parse()?;
                anyhow::ensure!(
                    (1..=net::MAX_LOCALS).contains(&a.locals),
                    "--locals 1 to {}",
                    net::MAX_LOCALS
                );
            }
            "--away" => a.away = Some(val()?.parse()?),
            other => bail!("unknown flag: {other}"),
        }
    }
    Ok(a)
}

/// Scenes by map key, loaded once each.
#[derive(Default)]
struct Scenes(BTreeMap<String, game::assets::Scene>);

impl Scenes {
    fn get(
        &mut self,
        assets: &std::path::Path,
        map: &str,
        mode: &str,
    ) -> Result<&game::assets::Scene> {
        if !self.0.contains_key(map) {
            let dir = assets.join(map);
            let (scene, load) = game::assets::load_battle_scene(&dir, mode)
                .with_context(|| format!("load {}", dir.display()))?;
            if let Some((path, Err(error))) = load.common {
                bail!("common assets at {} unreadable: {error:#}", path.display());
            }
            self.0.insert(map.to_owned(), scene);
        }
        Ok(&self.0[map])
    }
}

/// One round's walk and feed, and its dev-only session stats
/// (`session-stats` feature): a flagged peer records the round while it
/// hash-checks against unflagged ones.
struct Round {
    walk: game::walk::WalkSim,
    feed: game::event_feed::EventFeed,
    stats: Option<game::session_stats::SessionStats>,
}

impl Round {
    /// Write the session stats, if any (`quit` before an outcome).
    fn finish(&mut self) {
        if let Some(s) = self.stats.as_mut() {
            let reason = if self.walk.sim.outcome.is_some() {
                "outcome"
            } else {
                "quit"
            };
            if let Some(p) = s.write(&self.walk, self.feed.round(), Some(&self.feed), reason) {
                println!("session stats: wrote {}", p.display());
            }
        }
    }
}

fn build(scenes: &mut Scenes, a: &Args, setup: &RoundSetup, local: u8) -> Result<Round> {
    let started = Instant::now();
    let scene = scenes.get(&a.assets, &setup.map, &setup.mode)?;
    let walk = super::round_setup::build_round(scene, setup, &a.missions, &a.assets, local)?;
    let feed = game::event_feed::EventFeed::for_walk(&a.assets, &scene.manifest.source_game, &walk);
    let stats = game::session_stats::ENABLED.then(|| {
        let info = game::session_stats::RoundInfo {
            map: setup.map.clone(),
            mode: setup.mode.clone(),
            source_game: scene.manifest.source_game.clone(),
            seed: setup.seed,
            role: if local == 0 { "host" } else { "guest" }.into(),
            local_slot: local,
            ..Default::default()
        };
        game::session_stats::SessionStats::start(info, &walk, game::session_stats::dir())
    });
    println!(
        "round built: {} {} in {:.1} s, static digest {:016x}",
        setup.map,
        setup.mode,
        started.elapsed().as_secs_f64(),
        walk.sim.static_digest()
    );
    Ok(Round { walk, feed, stats })
}

fn log_notices(notices: &[Notice]) {
    for n in notices {
        match n {
            Notice::Load { .. } => {}
            other => println!("net: {other:?}"),
        }
    }
}

/// Steps one frame of a built round with every local player's scripted pad
/// (local 0 first).
fn play<L: Link>(peer: &mut Peer, link: &mut L, round: &mut Option<Round>, dt: f64) {
    if let Some(r) = round.as_mut() {
        let tick = r.walk.sim.tick;
        let pads: Vec<_> = (0..peer.local_count())
            .map(|k| {
                let slot = peer.local_slot_of(k).unwrap_or(peer.local_slot);
                game::netplay::virtual_net::scripted_pad(slot, tick)
            })
            .collect();
        let stats = &mut r.stats;
        peer.frame_locals(
            link,
            dt,
            &pads,
            &mut r.walk,
            Some(&mut r.feed),
            |walk, _, _| {
                if let Some(s) = stats.as_mut() {
                    s.tick(walk);
                }
            },
        );
    }
}

/// `--away`'s phase: 0 before, 1 while the last local's pad is gone, 2 once
/// it is back. `played` is the seconds since the round started.
fn step_away<L: Link>(peer: &mut Peer, link: &mut L, a: &Args, played: f64, phase: &mut u8) {
    let Some(at) = a.away else {
        return;
    };
    let last = peer.local_count().saturating_sub(1);
    let slot = peer.local_slot_of(last);
    if *phase == 0 && played >= at {
        peer.set_local_away(link, last, true);
        *phase = 1;
        println!("local {last} (slot {slot:?}): pad away");
    } else if *phase == 1 && played >= at + AWAY_FOR {
        peer.set_local_away(link, last, false);
        *phase = 2;
        println!("local {last} (slot {slot:?}): pad back");
    }
}

/// Every local player's slot and whether the round seats it now.
fn seated(peer: &Peer, round: &Option<Round>) -> Vec<(u8, bool)> {
    (0..peer.local_count())
        .filter_map(|k| peer.local_slot_of(k))
        .map(|s| {
            let on = round.as_ref().is_some_and(|r| {
                r.walk
                    .sim
                    .players
                    .get(usize::from(s))
                    .is_some_and(Option::is_some)
            });
            (s, on)
        })
        .collect()
}

pub(super) fn host(args: &[String]) -> Result<()> {
    let a = parse(args, false)?;
    let transport =
        UdpTransport::host(a.port).with_context(|| format!("bind UDP port {}", a.port))?;
    let mut profile = super::net_soak::scripted_profile(0);
    profile.name = a.name.clone().unwrap_or_else(|| "Host".into());
    let mut config = HostConfig::new(game::netplay::build_id(), profile.name.clone());
    config.profile = profile.encode();
    let mut link: NetSession<HostCore> = NetSession::host(config, transport);
    let mut peer = Peer::host();
    // The host machine's further players (splitscreen B19), each on its own profile.
    let local_profile = |k: usize| {
        let mut p = super::net_soak::scripted_profile(10 + k as u8);
        p.name = format!("{}_{k}", profile.name);
        p
    };
    for k in 1..a.locals {
        let p = local_profile(k);
        Link::host(
            &mut link,
            net::HostCommand::AddLocal {
                seat: k as u8,
                name: p.name.clone(),
                profile: p.encode(),
            },
        );
    }
    let mut host_locals: BTreeMap<u8, (String, game::netplay::PlayerProfile)> = BTreeMap::new();
    let mut scenes = Scenes::default();
    // Load the map before anyone waits on it.
    let source_game = scenes
        .get(&a.assets, &a.map, &a.mode)?
        .manifest
        .source_game
        .clone();
    println!(
        "hosting {} {} on UDP {} (build {}, {}); waiting for {} players or {} s",
        a.map,
        a.mode,
        a.port,
        game::netplay::build_id(),
        game::netplay::target(),
        a.players,
        a.wait
    );
    let opened = Instant::now();
    let mut round: Option<Round> = None;
    let mut started_at: Option<Instant> = None;
    let mut last = Instant::now();
    let mut status = Instant::now();
    let mut peak_guests = 0;
    let mut away_phase = 0;
    let mut ended_at: Option<Instant> = None;
    loop {
        let notices = peer.poll(&mut link);
        log_notices(&notices);
        for n in &notices {
            if let Notice::LocalAdded {
                seat,
                added: Ok((slot, name)),
            } = n
            {
                let mut p = local_profile(usize::from(*seat));
                p.name.clone_from(name);
                host_locals.insert(*slot, (name.clone(), p));
            }
        }
        peak_guests = peak_guests.max(peer.guests().len());
        let humans = peer.local_count() + peer.guests().len();
        let locals_in = peer.local_count() >= a.locals;
        let ready = locals_in && (humans >= a.players || opened.elapsed().as_secs_f64() >= a.wait);
        if round.is_none() && ready {
            let armies = a
                .teams
                .clone()
                .unwrap_or_else(|| super::net_soak::default_armies(&source_game));
            let mut setup = RoundSetup::instant_action(
                &a.map,
                &a.mode,
                armies,
                &ui::shell::SessionOptions::default(),
                &game::profile::Profile::default(),
            );
            setup.humans.push(HumanSeat {
                slot: 0,
                name: profile.name.clone(),
                team: None,
                cheats: 0,
                profile: profile.clone(),
            });
            for (slot, (name, p)) in &host_locals {
                setup.humans.push(HumanSeat {
                    slot: *slot,
                    name: name.clone(),
                    team: None,
                    cheats: 0,
                    profile: p.clone(),
                });
            }
            for (slot, g) in peer.guests() {
                setup.humans.push(HumanSeat {
                    slot: *slot,
                    name: g.name.clone(),
                    team: None,
                    cheats: 0,
                    profile: g.profile.clone(),
                });
            }
            let bytes = peer.host_load(&mut link, &setup);
            let setup = RoundSetup::decode(&bytes)?;
            let mut r = build(&mut scenes, &a, &setup, 0)?;
            peer.round_built(&mut link, &mut r.walk);
            round = Some(r);
            last = Instant::now();
        }
        if started_at.is_none() && peer.mode() == Mode::Playing {
            started_at = Some(Instant::now());
            println!("round started");
        }
        let dt = last.elapsed().as_secs_f64();
        last = Instant::now();
        play(&mut peer, &mut link, &mut round, dt);
        if let Some(t) = started_at {
            step_away(
                &mut peer,
                &mut link,
                &a,
                t.elapsed().as_secs_f64(),
                &mut away_phase,
            );
        }
        // The round's outcome: the host ends its bundles for everyone (LAN §4.8).
        if ended_at.is_none() && round.as_ref().is_some_and(|r| r.walk.sim.outcome.is_some()) {
            Link::host(&mut link, net::HostCommand::EndRound);
            ended_at = Some(Instant::now());
            println!(
                "round decided at tick {}: outcome {:?}",
                peer.next_tick(),
                round.as_ref().and_then(|r| r.walk.sim.outcome)
            );
        }
        if status.elapsed() >= STATUS_EVERY {
            status = Instant::now();
            let s = link.stats();
            let pings: Vec<String> = s
                .peers
                .iter()
                .map(|p| {
                    format!(
                        "{}:{:.0}ms{}",
                        p.slot,
                        p.rtt_ms,
                        if p.trouble { "!" } else { "" }
                    )
                })
                .collect();
            println!(
                "tick {} | guests [{}] | own queue {} | hash reports {} desyncs {} | dilation {:+.3}",
                peer.next_tick(),
                pings.join(" "),
                s.local_depth,
                peer.stats.hash_reports,
                peer.stats.desyncs,
                peer.pacer().dilation(),
            );
        }
        let timed_out = started_at.is_some_and(|t| t.elapsed().as_secs_f64() >= a.seconds);
        if timed_out && ended_at.is_none() {
            Link::host(&mut link, net::HostCommand::EndRound);
            ended_at = Some(Instant::now());
        }
        if ended_at.is_some_and(|t| t.elapsed() >= LINGER) {
            break;
        }
        std::thread::sleep(FRAME.saturating_sub(last.elapsed()));
    }
    let summary = serde_json::json!({
        "role": "host",
        "build": game::netplay::build_id(),
        "target": game::netplay::target(),
        "ticks": peer.next_tick(),
        "guests": peer.guests().len(),
        "peak_guests": peak_guests,
        "locals": seated(&peer, &round),
        "away": away_phase,
        "outcome": round.as_ref().and_then(|r| r.walk.sim.outcome),
        "stats": peer.stats,
        "net": link.stats(),
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    let stats = peer.stats.clone();
    if let Some(r) = round.as_mut() {
        r.finish();
    }
    link.shutdown();
    println!("{}", verdict(None, peak_guests, &stats)?);
    Ok(())
}

pub(super) fn join(args: &[String]) -> Result<()> {
    let a = parse(args, true)?;
    let addr = a.addr.as_deref().expect("parsed");
    let host = net::transport::resolve(addr, net::DEFAULT_PORT)
        .with_context(|| format!("resolve {addr}"))?;
    let transport = UdpTransport::client().context("bind a UDP port")?;
    let token = game::netplay::lobby::session_token();
    let mut profile = super::net_soak::scripted_profile(1);
    profile.name = a.name.clone().unwrap_or_else(|| "Guest".into());
    let mut config = ClientConfig::new(game::netplay::build_id(), profile.name.clone(), token);
    config.profile = profile.encode();
    config.content = game::netplay::ContentManifest::scan(&a.assets).encode();
    // This machine's further players (splitscreen B19): its `Hello` asks a slot for each.
    config.locals = (1..a.locals)
        .map(|k| net::LocalHello {
            seat: k as u8,
            name: format!("{}_{k}", profile.name),
            profile: super::net_soak::scripted_profile(20 + k as u8).encode(),
        })
        .collect();
    let mut link: NetSession<ClientCore> = NetSession::join(config, host, transport);
    let mut peer = Peer::client();
    let mut scenes = Scenes::default();
    println!(
        "joining {host} (build {}, {})",
        game::netplay::build_id(),
        game::netplay::target()
    );
    let opened = Instant::now();
    let mut round: Option<Round> = None;
    let mut started_at: Option<Instant> = None;
    let mut last = Instant::now();
    let mut status = Instant::now();
    let mut end = None;
    let mut host_left = false;
    let mut away_phase = 0;
    loop {
        let notices = peer.poll(&mut link);
        log_notices(&notices);
        for n in &notices {
            match n {
                Notice::Load { setup, joining, .. } => {
                    println!("loading {} {} (joining: {joining})", setup.map, setup.mode);
                    drop(round.take());
                    let mut r = build(&mut scenes, &a, setup, peer.local_slot)?;
                    peer.round_built(&mut link, &mut r.walk);
                    round = Some(r);
                    last = Instant::now();
                }
                Notice::Disconnected(net::client::DisconnectReason::HostLeft) => {
                    host_left = true;
                    end = Some(format!("{n:?}"));
                }
                Notice::Rejected { .. }
                | Notice::Disconnected(_)
                | Notice::BadSetup(_)
                | Notice::DigestMismatch { .. } => {
                    end = Some(format!("{n:?}"));
                }
                _ => {}
            }
        }
        if started_at.is_none() && peer.mode() == Mode::Playing {
            started_at = Some(Instant::now());
            println!("round started");
        }
        let dt = last.elapsed().as_secs_f64();
        last = Instant::now();
        play(&mut peer, &mut link, &mut round, dt);
        if let Some(t) = started_at {
            step_away(
                &mut peer,
                &mut link,
                &a,
                t.elapsed().as_secs_f64(),
                &mut away_phase,
            );
        }
        if status.elapsed() >= STATUS_EVERY {
            status = Instant::now();
            let s = link.stats();
            println!(
                "tick {} {:?} | ping {:.0} ms ±{:.0} | input delay {} ticks ({:.0} ms) | buffer {} target {:.1} jitter {:.2} | loss {} resends {} | hash reports {} desyncs {}",
                peer.next_tick(),
                peer.mode(),
                s.rtt_ms,
                s.rttvar_ms,
                peer.pending_inputs().len(),
                peer.pending_inputs().len() as f64 * 1000.0 / 30.0,
                peer.queued(),
                peer.playback().target(),
                s.jitter_ticks,
                s.link.packets_lost,
                s.link.resends,
                peer.stats.hash_reports,
                peer.stats.desyncs,
            );
        }
        let played = started_at.is_some_and(|t| t.elapsed().as_secs_f64() >= a.seconds);
        let gave_up = opened.elapsed().as_secs_f64() >= a.seconds + GIVE_UP;
        if end.is_some() || played || gave_up || peer.mode() == Mode::Ended {
            break;
        }
        std::thread::sleep(FRAME.saturating_sub(last.elapsed()));
    }
    let summary = serde_json::json!({
        "role": "guest",
        "slot": peer.local_slot,
        "locals": seated(&peer, &round),
        "away": away_phase,
        "outcome": round.as_ref().and_then(|r| r.walk.sim.outcome),
        "build": game::netplay::build_id(),
        "target": game::netplay::target(),
        "ticks": peer.next_tick(),
        "ended": end,
        "stats": peer.stats,
        "net": link.stats(),
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    let stats = peer.stats.clone();
    if let Some(r) = round.as_mut() {
        r.finish();
    }
    link.shutdown();
    // The host ending the session after the round started is the normal
    // end of its `--seconds`; anything else cut the check short.
    let trouble = if started_at.is_none() {
        Some(end.unwrap_or_else(|| "the round never started".into()))
    } else if host_left {
        None
    } else {
        end
    };
    let others = usize::from(started_at.is_some());
    println!("{}", verdict(trouble.as_deref(), others, &stats)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{verdict, PeerStats};

    /// A run that compared nothing never passes: no other player, no hash
    /// checks, an early end or differing copies each fail, as does a desync.
    #[test]
    fn net_real_verdict_needs_hash_checks_and_another_player() {
        let stats = |hash_reports, desyncs, digest_mismatches| PeerStats {
            hash_reports,
            desyncs,
            digest_mismatches,
            ..PeerStats::default()
        };
        assert_eq!(
            verdict(None, 1, &stats(40, 0, 0)).unwrap(),
            "PASS: 40 hash checks, 0 desyncs"
        );
        let fail = |t: Option<&str>, o, s: PeerStats| verdict(t, o, &s).unwrap_err().to_string();
        assert!(fail(None, 1, stats(0, 0, 0)).contains("no hash checks"));
        assert!(fail(None, 0, stats(40, 0, 0)).contains("no other player"));
        assert!(fail(Some("Disconnected(Timeout)"), 1, stats(40, 0, 0)).contains("ended early"));
        assert!(fail(None, 1, stats(40, 0, 1)).contains("plays differently"));
        assert!(fail(None, 1, stats(40, 2, 0)).contains("2 desyncs in 40 hash checks"));
    }
}
