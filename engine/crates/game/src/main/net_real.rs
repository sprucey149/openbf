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
//!   both: [--assets DIR] [--missions DIR]
//!
//! The host waits until `--players` humans (itself included, default 2) are
//! connected or `--wait` seconds (default 60) pass, starts the round and
//! plays `--seconds` (default 60) of it; a guest that connects later joins
//! mid-round. A guest plays `--seconds` from the moment its round starts
//! playing, or until the host ends; it gives up `GIVE_UP` seconds after that
//! budget if its round never starts.
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

/// One round's walk and feed.
struct Round {
    walk: game::walk::WalkSim,
    feed: game::event_feed::EventFeed,
}

fn build(scenes: &mut Scenes, a: &Args, setup: &RoundSetup, local: u8) -> Result<Round> {
    let started = Instant::now();
    let scene = scenes.get(&a.assets, &setup.map, &setup.mode)?;
    let walk = super::round_setup::build_round(scene, setup, &a.missions, &a.assets, local)?;
    let feed = game::event_feed::EventFeed::for_walk(&a.assets, &scene.manifest.source_game, &walk);
    println!(
        "round built: {} {} in {:.1} s, static digest {:016x}",
        setup.map,
        setup.mode,
        started.elapsed().as_secs_f64(),
        walk.sim.static_digest()
    );
    Ok(Round { walk, feed })
}

fn log_notices(notices: &[Notice]) {
    for n in notices {
        match n {
            Notice::Load { .. } => {}
            other => println!("net: {other:?}"),
        }
    }
}

/// Steps one frame of a built round with the scripted pad.
fn play<L: Link>(peer: &mut Peer, link: &mut L, round: &mut Option<Round>, dt: f64) {
    if let Some(r) = round.as_mut() {
        let pad = game::netplay::virtual_net::scripted_pad(peer.local_slot, r.walk.sim.tick);
        peer.frame(link, dt, pad, &mut r.walk, Some(&mut r.feed), |_, _, _| {});
    }
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
    loop {
        let notices = peer.poll(&mut link);
        log_notices(&notices);
        peak_guests = peak_guests.max(peer.guests().len());
        let ready =
            1 + peer.guests().len() >= a.players || opened.elapsed().as_secs_f64() >= a.wait;
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
        let done = started_at.is_some_and(|t| t.elapsed().as_secs_f64() >= a.seconds)
            || peer.mode() == Mode::Ended;
        if done {
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
        "stats": peer.stats,
        "net": link.stats(),
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    let stats = peer.stats.clone();
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
        "build": game::netplay::build_id(),
        "target": game::netplay::target(),
        "ticks": peer.next_tick(),
        "ended": end,
        "stats": peer.stats,
        "net": link.stats(),
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    let stats = peer.stats.clone();
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
