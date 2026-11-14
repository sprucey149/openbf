//! The windowed game's LAN session (issue #168, `analysis/plan/LAN_MULTIPLAYER.md`
//! §4-§9): `game --lan-host` / `--lan-join` straight into a networked
//! Instant Action round, and everything the battle loop does differently
//! with a [`NetplaySession`].
//!
//!   --lan-host [--port P] [--map KEY] [--mode M] [--teams a,b]
//!              [--players N] [--cheats none|all|id,id..] [--name NAME]
//!   --lan-join ADDR[:PORT] [--name NAME]
//!   both: [--assets DIR] [--missions DIR]
//!
//! Rules:
//! - The host starts its round at once (or when `--players` humans are
//!   connected); a guest that connects later joins it mid-round (catch-up
//!   from the round log, then `AddHuman`). Every round is built from the
//!   host's `RoundSetup` (`round_setup::build_round`) on every peer.
//! - Each frame the battle steps through [`LocalView::frame`] (never the
//!   single-player accumulator). Pausing never stops the sim: the menu
//!   overlays, the local pad is neutral while it is open, and bundles keep
//!   stepping.
//! - Pause-menu actions that change the sim become host commands or guest
//!   requests ([`NetplaySession::pause_action`]); Respawn is the next
//!   input's RESPAWN bit. Guests see only the cheats the host allows; the
//!   host's Cheats page applies to a target it cycles: everyone, itself or
//!   one guest. Morse words go the same way; `slow` is the host's alone,
//!   `hud` and the no-op display words stay local.
//! - The net meter (F7, Options → Network meter) and the black box read
//!   [`NetMeter`]; neither touches the sim.
//! - Joins, leaves, refusals and desyncs are HUD lines; a session that ends
//!   (the host left, kicked, fell behind, rejected, timed out) shows a
//!   centred banner for [`ENDED_SECS`] and returns to the shell.
//! - Between rounds every peer waits in the lobby (`lobby.rs`): a round's
//!   end, a copy that plays differently (§4.3: the host benches that guest
//!   and tells it which group differs; a mid-round joiner's build is
//!   checked too), a map a mid-round joiner lacks, a round a
//!   guest could not build or a second desync within 5 minutes (§4.7). A
//!   guest that leaves a round on its own tells the host, which benches it
//!   so its barrier and seat do not wait.
//!   The host ends a decided round with `EndRound` and keeps the round's
//!   ready check ([`NetplaySession::round_decided`]).
//! - A desync rejoin rebuilds the round in play: the abandoned copy is not
//!   recorded into the career (the rebuilt one is, at its outcome).

use super::{App, Screen};
use anyhow::{bail, Context, Result};
use core::sim::PadState;
use game::netplay::{
    cheat_bits, cheat_index, GameCommand, GameNotice, GameRequest, HumanSeat, LeftWhy, Link,
    LinkEvent, LobbyPhase, LocalView, MeterPeer, Mode, NetMeter, Notice, Peer, PlayerProfile,
    RoundSetup, ALL_CHEATS, ALL_SLOTS,
};
use game::walk::WalkSim;
use net::{
    ClientConfig, ClientCore, HostCommand, HostConfig, HostCore, LeaveReason, NetSession,
    UdpTransport,
};
use std::path::PathBuf;
use std::time::Instant;

/// How long the "session ended" banner shows before the shell.
pub(super) const ENDED_SECS: f32 = 4.0;
/// How long a centred notice shows.
const BANNER_SECS: f32 = 4.0;
/// A joiner's catch-up steps per frame in the window.
const CATCH_UP_PER_FRAME: usize = 90;

/// A session's network side.
pub(super) enum NetLink {
    Host(NetSession<HostCore>),
    Client(NetSession<ClientCore>),
}

macro_rules! link {
    ($s:expr, $x:ident => $e:expr) => {
        match $s {
            NetLink::Host($x) => $e,
            NetLink::Client($x) => $e,
        }
    };
}

impl Link for NetLink {
    fn is_host(&self) -> bool {
        matches!(self, NetLink::Host(_))
    }
    fn poll(&mut self) -> Option<LinkEvent> {
        link!(self, x => x.poll())
    }
    fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        link!(self, x => x.submit_input(pad, view_tick))
    }
    fn pacing(&self) -> (u8, u32) {
        link!(self, x => x.pacing())
    }
    fn slack_us(&self) -> Option<i64> {
        link!(self, x => x.slack_us())
    }
    fn jitter_ticks(&self) -> f64 {
        link!(self, x => x.jitter_ticks())
    }
    fn report_hash(&mut self, tick: u32, hash: u64) {
        link!(self, x => x.report_hash(tick, hash))
    }
    fn loaded(&mut self, round_id: u32, static_digest: u64, hash0: u64) {
        link!(self, x => x.loaded(round_id, static_digest, hash0))
    }
    fn caught_up(&mut self, tick: u32) {
        link!(self, x => x.caught_up(tick))
    }
    fn request(&mut self, data: Vec<u8>) {
        link!(self, x => x.request(data))
    }
    fn request_resync(&mut self) {
        link!(self, x => x.request_resync())
    }
    fn desync_hashes(&mut self, tick: u32, first_tick: u32, width: u8, values: Vec<u64>) {
        link!(self, x => x.desync_hashes(tick, first_tick, width, values))
    }
    fn host(&mut self, cmd: HostCommand) {
        link!(self, x => Link::host(x, cmd))
    }
}

/// `--lan-host` / `--lan-join`'s flags (module header).
#[derive(Clone, Debug)]
pub(super) struct LanArgs {
    pub host: bool,
    pub addr: Option<String>,
    pub port: u16,
    pub map: String,
    pub mode: String,
    pub teams: Option<[String; 2]>,
    pub players: usize,
    pub cheats: u32,
    pub name: Option<String>,
    /// Hidden testing switch (`--no-lag-comp`): lag compensation off.
    pub no_lag_comp: bool,
    pub assets: PathBuf,
    pub missions: PathBuf,
}

impl LanArgs {
    pub fn parse(args: &[String], host: bool) -> Result<Self> {
        let mut a = LanArgs {
            host,
            addr: None,
            port: net::DEFAULT_PORT,
            map: "bf2_kas2".into(),
            mode: "con".into(),
            teams: None,
            players: 1,
            cheats: 0,
            name: None,
            no_lag_comp: false,
            assets: "assets_baked".into(),
            missions: "missions".into(),
        };
        let mut it = args.iter();
        if !host {
            a.addr = Some(it.next().context("--lan-join ADDR[:PORT]")?.clone());
        }
        while let Some(f) = it.next() {
            let mut val = || it.next().with_context(|| format!("{f} <value>"));
            match f.as_str() {
                "--port" if host => a.port = val()?.parse()?,
                "--map" if host => a.map = val()?.clone(),
                "--mode" if host => a.mode = val()?.clone(),
                "--teams" if host => {
                    let v = val()?;
                    let (x, y) = v.split_once(',').context("--teams a,b")?;
                    a.teams = Some([x.trim().into(), y.trim().into()]);
                }
                "--players" if host => a.players = val()?.parse()?,
                "--cheats" if host => {
                    a.cheats = match val()?.as_str() {
                        "none" => 0,
                        "all" => ALL_CHEATS,
                        list => list.split(',').try_fold(0u32, |b, id| {
                            cheat_index(id.trim())
                                .map(|i| b | 1 << i)
                                .with_context(|| format!("unknown cheat {id}"))
                        })?,
                    }
                }
                "--name" => a.name = Some(val()?.clone()),
                "--no-lag-comp" if host => a.no_lag_comp = true,
                "--assets" => a.assets = val()?.into(),
                "--missions" => a.missions = val()?.into(),
                other => bail!("unknown flag: {other}"),
            }
        }
        anyhow::ensure!((1..=32).contains(&a.players), "--players wants 1..=32");
        Ok(a)
    }
}

/// `game --lan-host` / `--lan-join`: the windowed game in a LAN session.
pub(in super::super) fn run_lan(args: &[String], host: bool) -> Result<()> {
    let a = LanArgs::parse(args, host)?;
    let (event_loop, mut app) = super::shell_app(a.assets.clone(), a.missions.clone())?;
    let mut profile = PlayerProfile::from_profile(&app.profile);
    if let Some(n) = &a.name {
        profile.name = n.chars().take(net::msg::MAX_NAME_CHARS).collect();
    }
    if profile.name.is_empty() {
        profile.name = if host { "Host" } else { "Guest" }.into();
    }
    let session = NetplaySession::open(a, profile)?;
    println!("lan: {}", session.status);
    app.netplay = Some(session);
    app.screen = Screen::NetWait;
    super::run_app(event_loop, &mut app)
}

/// Whether a `LoadRound` (`round_id`, `joining`) rebuilds the round in play
/// (a desync's or a reconnect's rejoin), given the last one's id in `last`,
/// which it becomes.
fn note_load(last: &mut Option<u32>, round_id: u32, joining: bool) -> bool {
    let rejoin = joining && *last == Some(round_id);
    *last = Some(round_id);
    rejoin
}

/// A pause-menu cheat row's meaning on the host's Cheats page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CheatRow {
    /// The first row: cycles who the rows apply to.
    Target,
    /// The host's award policy for everyone (LAN §5b).
    Awards,
    Cheat(u8),
}

/// The windowed game's LAN session (module header).
pub(super) struct NetplaySession {
    pub peer: Peer,
    pub link: NetLink,
    pub view: LocalView,
    pub args: LanArgs,
    pub profile: PlayerProfile,
    /// The round the loading screen is building.
    pub loading: Option<RoundSetup>,
    /// The host started its round.
    started: bool,
    pub meter: NetMeter,
    /// A centred notice and when it was raised.
    pub banner: Option<(String, Instant)>,
    /// The session is over: why, and since when.
    pub ended: Option<(String, Instant)>,
    /// The pause menu's Respawn: the next input carries RESPAWN.
    pub respawn: bool,
    /// The host's Cheats page target ([`ALL_SLOTS`] or a slot).
    cheat_target: u8,
    /// The host's Cheats page rows as drawn.
    cheat_rows: Vec<CheatRow>,
    last_hash: Option<(u32, Instant)>,
    rate: (Instant, u64, u64),
    rates: (f32, f32),
    /// The waiting screen's line.
    pub status: String,
    /// The lobby between rounds (`lobby.rs`).
    pub lobby: super::lobby::Lobby,
    /// The round id of the last `LoadRound` (a joining one with the same
    /// id is a rejoin of the round in play).
    round_in_play: Option<u32>,
}

impl NetplaySession {
    /// Opens the session: binds and hosts, or resolves and joins.
    pub fn open(args: LanArgs, profile: PlayerProfile) -> Result<Self> {
        if args.host {
            let lag_comp = !args.no_lag_comp;
            return Self::host(args, profile, net::host::DEFAULT_MAX_PLAYERS, lag_comp);
        }
        let addr = args.addr.clone().unwrap_or_default();
        let host = net::transport::resolve(&addr, net::DEFAULT_PORT)
            .with_context(|| format!("resolve {addr}"))?;
        Self::join(args, profile, host)
    }

    /// Hosts on `args.port`: at most `max_players` humans, lag
    /// compensation on or off.
    pub fn host(
        args: LanArgs,
        profile: PlayerProfile,
        max_players: u8,
        lag_comp: bool,
    ) -> Result<Self> {
        let build = game::netplay::build_id();
        let transport = UdpTransport::host(args.port)
            .with_context(|| format!("UDP port {} is in use or not allowed", args.port))?;
        let mut config = HostConfig::new(build.clone(), profile.name.clone());
        config.profile = profile.encode();
        config.max_players = max_players;
        if !lag_comp {
            config.lag_comp_cap = 0;
        }
        let status = format!(
            "Hosting {} {} on UDP {} (build {build})",
            args.map, args.mode, args.port
        );
        let content = game::netplay::ContentManifest::scan(&args.assets);
        let link = NetLink::Host(NetSession::host(config, transport));
        let mut n = Self::with(args, profile, link, Peer::host(), status);
        n.lobby.content = content;
        n.lobby.max_players = max_players;
        n.lobby.lag_comp = lag_comp;
        Ok(n)
    }

    /// Joins the host at `host`.
    pub fn join(args: LanArgs, profile: PlayerProfile, host: std::net::SocketAddr) -> Result<Self> {
        let build = game::netplay::build_id();
        let transport = UdpTransport::client().context("bind a UDP port")?;
        // One token per game process (LAN §4.5): a rejoin reclaims the slot
        // and name the host keeps for it.
        let token = game::netplay::lobby::session_token();
        let mut config = ClientConfig::new(build.clone(), profile.name.clone(), token);
        config.profile = profile.encode();
        let content = game::netplay::ContentManifest::scan(&args.assets);
        config.content = content.encode();
        let status = format!("Connecting to {host}\u{2026}");
        let link = NetLink::Client(NetSession::join(config, host, transport));
        let mut n = Self::with(args, profile, link, Peer::client(), status);
        n.lobby.content = content;
        Ok(n)
    }

    fn with(
        args: LanArgs,
        profile: PlayerProfile,
        link: NetLink,
        peer: Peer,
        status: String,
    ) -> Self {
        let mut peer = peer;
        peer.cheat_policy = args.cheats;
        // A joiner's catch-up keeps the window drawing (LAN §4.5): a few
        // frames a second while it steps the round log.
        peer.catch_up_budget = CATCH_UP_PER_FRAME;
        Self {
            peer,
            link,
            view: LocalView::new(),
            args,
            profile,
            loading: None,
            started: false,
            meter: NetMeter::default(),
            banner: None,
            ended: None,
            respawn: false,
            cheat_target: ALL_SLOTS,
            cheat_rows: Vec::new(),
            last_hash: None,
            rate: (Instant::now(), 0, 0),
            rates: (0.0, 0.0),
            status,
            lobby: Default::default(),
            round_in_play: None,
        }
    }

    pub fn is_host(&self) -> bool {
        self.peer.is_host()
    }

    /// Says goodbye (the network thread delivers the `Leave`).
    pub fn shutdown(self) {
        match self.link {
            NetLink::Host(s) => s.shutdown(),
            NetLink::Client(s) => s.shutdown(),
        }
    }

    /// The host's round setup now: Instant Action of the flags' map with
    /// the shell's session options, every connected human seated.
    pub fn host_setup(
        &self,
        source_game: &str,
        session: &ui::shell::SessionOptions,
        profile: &game::profile::Profile,
    ) -> RoundSetup {
        let armies = self.args.teams.clone().unwrap_or_else(|| {
            if source_game.eq_ignore_ascii_case("bf1") {
                ["all".into(), "imp".into()]
            } else {
                ["rep".into(), "cis".into()]
            }
        });
        let setup =
            RoundSetup::instant_action(&self.args.map, &self.args.mode, armies, session, profile);
        self.seat_humans(setup)
    }

    /// The host's round for a playlist row (the lobby's Launch): the row's
    /// map, mode and armies with the launched session options.
    pub fn host_row_setup(
        &self,
        row: &ui::shell::Launch,
        session: &ui::shell::SessionOptions,
        profile: &game::profile::Profile,
        pos: u32,
    ) -> RoundSetup {
        let mut setup = RoundSetup::instant_action(
            &row.key,
            row.mode.subst(),
            row.armies.clone(),
            session,
            profile,
        );
        setup.difficulty = game::shell_ia::port_difficulty(session.difficulty);
        setup.playlist_pos = pos;
        self.seat_humans(setup)
    }

    /// Every connected human seated, the host's cheat policy.
    fn seat_humans(&self, mut setup: RoundSetup) -> RoundSetup {
        setup.cheat_policy = self.args.cheats;
        setup.awards = self.lobby.awards;
        setup.humans.push(HumanSeat {
            slot: 0,
            name: self.profile.name.clone(),
            team: None,
            cheats: 0,
            profile: self.profile.clone(),
        });
        for (slot, g) in self.peer.guests() {
            setup.humans.push(HumanSeat {
                slot: *slot,
                name: g.name.clone(),
                team: None,
                cheats: 0,
                profile: g.profile.clone(),
            });
        }
        setup
    }

    /// The loading screen built the round.
    pub fn round_built(&mut self, walk: &mut WalkSim) {
        self.view.reset();
        self.lobby.groups = walk.sim.static_digest_groups();
        self.peer.round_built(&mut self.link, walk);
    }

    /// The round in play was decided (its outcome stepped): the host ends
    /// the round's bundles and opens the ready check (LAN §4.8).
    pub fn round_decided(&mut self) {
        if !self.is_host() || matches!(self.lobby.phase, LobbyPhase::Results { .. }) {
            return;
        }
        self.link.host(HostCommand::EndRound);
        self.lobby.phase = LobbyPhase::Results { ready: Vec::new() };
        self.lobby.ready.clear();
        self.lobby.results_at = Some(Instant::now());
        self.lobby.round_map = None;
    }

    /// A guest's notice to this session (`GameNotice` bytes): back to the
    /// lobby with its line, if it is one.
    fn game_notice(&self, bytes: &[u8]) -> Option<String> {
        match GameNotice::decode(bytes)? {
            GameNotice::DigestMismatch { map, groups } => Some(
                game::netplay::lobby::digest_mismatch_text(&map, &groups, &self.lobby.groups),
            ),
            GameNotice::BackToLobby(text) => Some(text),
        }
    }

    /// The host sends `slot` to the lobby with `notice` (LAN §4.3).
    fn bench(&mut self, slot: u8, notice: GameNotice) {
        self.link.host(HostCommand::Notice {
            slot: Some(slot),
            data: notice.encode(),
        });
        self.link.host(HostCommand::Bench(slot));
    }

    /// A human-readable leave reason.
    fn left_text(reason: LeaveReason) -> &'static str {
        match reason {
            LeaveReason::Left => "left",
            LeaveReason::Timeout => "timed out",
            LeaveReason::Kicked => "was kicked",
            LeaveReason::Replaced => "reconnected",
            LeaveReason::FellBehind => "could not keep up and was dropped",
        }
    }

    fn name_of(&self, slot: u8) -> String {
        self.peer
            .guests()
            .get(&slot)
            .map_or_else(|| format!("Player {slot}"), |g| g.name.clone())
    }

    /// One frame of the session's events: the rounds to build, the lines
    /// to show, the end.
    pub fn poll(&mut self) -> Vec<NetAction> {
        let mut out = Vec::new();
        for n in self.peer.poll(&mut self.link) {
            match n {
                Notice::Welcome { slot } => {
                    self.status = format!("Connected as player {slot}; waiting for the round");
                }
                Notice::Load {
                    setup,
                    joining,
                    round_id,
                } => {
                    self.status = format!(
                        "Loading {} {}{}",
                        setup.map,
                        setup.mode,
                        if joining { " (joining)" } else { "" }
                    );
                    let rejoin = note_load(&mut self.round_in_play, round_id, joining);
                    out.push(NetAction::Load { setup, rejoin });
                }
                Notice::Rejected { reason, detail } => {
                    let why = game::netplay::lobby::reject_text(
                        reason,
                        &detail,
                        &game::netplay::build_id(),
                    );
                    out.push(NetAction::Ended(why));
                }
                Notice::Disconnected(r) => out.push(NetAction::Ended(
                    match r {
                        net::client::DisconnectReason::HostLeft => "The host left the game",
                        net::client::DisconnectReason::Timeout => "Lost the connection to the host",
                        net::client::DisconnectReason::Left => "You left the battle",
                    }
                    .into(),
                )),
                Notice::BadSetup(e) => out.push(NetAction::Ended(format!(
                    "Cannot read the host's battle: {e}"
                ))),
                Notice::PlayerJoined {
                    slot,
                    name,
                    reconnect,
                } => {
                    out.push(NetAction::Line(format!(
                        "{name} {}",
                        if reconnect { "reconnected" } else { "joined" }
                    )));
                    // A mid-round joiner without the map waits in the lobby
                    // (one with it is checked by its build's digests).
                    let lacks = |key: &str| {
                        self.peer
                            .guests()
                            .get(&slot)
                            .and_then(|g| g.content.as_ref())
                            .is_some_and(|c| !c.maps.contains(key))
                    };
                    if let Some((key, label)) = self.lobby.round_map.clone() {
                        if lacks(&key) {
                            self.bench(
                                slot,
                                GameNotice::BackToLobby(format!(
                                    "You do not have {label}; you join the next round"
                                )),
                            );
                            self.lobby.say(format!("{name} waits for the next round"));
                        }
                    }
                }
                Notice::DesyncReport(path) => out.push(NetAction::Line(format!(
                    "Desync report saved: {}",
                    path.display()
                ))),
                Notice::DesyncReportFailed(e) => {
                    out.push(NetAction::Line(format!("Desync report not saved: {e}")))
                }
                Notice::GuestCaughtUp { slot, .. } => out.push(NetAction::Line(format!(
                    "{} entered the battle",
                    self.name_of(slot)
                ))),
                Notice::PlayerLeft { slot, reason } => out.push(NetAction::Line(format!(
                    "{} {}",
                    self.name_of(slot),
                    Self::left_text(reason)
                ))),
                Notice::DigestMismatch { slot, start, .. } => {
                    let label = self
                        .lobby
                        .round_map
                        .as_ref()
                        .map_or_else(|| "the map".to_owned(), |(_, l)| l.clone());
                    let groups = self
                        .lobby
                        .groups
                        .iter()
                        .map(|(n, d)| ((*n).to_owned(), *d))
                        .collect();
                    let notice = if start {
                        // Equal static digests, a different battle at tick 0.
                        GameNotice::BackToLobby(format!(
                            "Your copy of {label} starts the battle differently from the host's (the opening state)"
                        ))
                    } else {
                        GameNotice::DigestMismatch {
                            map: label.clone(),
                            groups,
                        }
                    };
                    self.bench(slot, notice);
                    let line = format!(
                        "{}'s copy of {label} plays differently from the host's; they wait in the lobby",
                        self.name_of(slot)
                    );
                    self.lobby.say(line.clone());
                    out.push(NetAction::Line(line));
                }
                Notice::PeerReady { slot } => {
                    self.lobby.ready.insert(slot);
                }
                Notice::Lobby(bytes) => self.lobby.receive(&bytes),
                Notice::Message(bytes) => {
                    if let Some(text) = self.game_notice(&bytes) {
                        out.push(NetAction::ToLobby(text));
                    }
                }
                Notice::Desync { slot, tick } => {
                    let text = if self.is_host() {
                        format!(
                            "{} fell out of sync at tick {tick}; rejoining",
                            self.name_of(slot)
                        )
                    } else {
                        format!("Out of sync with the host at tick {tick}; rejoining")
                    };
                    self.banner = Some((text.clone(), Instant::now()));
                    out.push(NetAction::Line(text));
                }
                Notice::DesyncAgain { tick } => out.push(NetAction::ToLobby(format!(
                    "Out of sync with the host again at tick {tick}; you join the next round"
                ))),
                Notice::LeftRound { slot, why, groups } => {
                    if !self.is_host() {
                        continue;
                    }
                    let who = self.name_of(slot);
                    let line = match why {
                        LeftWhy::LoadFailed => {
                            self.link.host(HostCommand::Bench(slot));
                            format!("{who} could not build the round; they wait for the next")
                        }
                        LeftWhy::Desynced => {
                            let parted = if groups.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", groups.join(", "))
                            };
                            self.bench(
                                slot,
                                GameNotice::BackToLobby(format!(
                                    "Your game parted from the host's twice in 5 minutes{parted}"
                                )),
                            );
                            format!(
                                "{who} fell out of sync twice in 5 minutes{parted}; they wait in the lobby"
                            )
                        }
                    };
                    self.lobby.say(line.clone());
                    out.push(NetAction::Line(line));
                }
                Notice::Refused { slot, why } => out.push(NetAction::Line(format!(
                    "{}'s request was refused ({why:?})",
                    self.name_of(slot)
                ))),
                Notice::Chat { slot, text } => {
                    out.push(NetAction::Line(format!("{}: {text}", self.name_of(slot))))
                }
                Notice::Started => {}
            }
        }
        out
    }

    /// The host's round is due: at once, or once `--players` are here.
    pub fn host_due(&self) -> bool {
        self.is_host() && !self.started && 1 + self.peer.guests().len() >= self.args.players
    }

    /// The host starts its round: every guest loads it; returns the setup
    /// to build here.
    pub fn host_start(&mut self, setup: &RoundSetup) -> Result<RoundSetup> {
        self.started = true;
        let bytes = self.peer.host_load(&mut self.link, setup);
        RoundSetup::decode(&bytes)
    }

    /// The pad this frame hands the session: neutral while paused (the sim
    /// keeps stepping), RESPAWN once after the menu's Respawn.
    pub fn frame_pad(&mut self, pad: PadState, paused: bool) -> PadState {
        let mut pad = if paused { PadState::default() } else { pad };
        if std::mem::take(&mut self.respawn) {
            pad.buttons |= core::sim::buttons::RESPAWN;
        }
        pad
    }

    /// The cheat rows the pause menu shows (module rules): a guest's
    /// allowed cheats for its own slot; the host's target row then every
    /// cheat for that target.
    pub fn cheat_entries(&mut self, walk: &WalkSim) -> Vec<(&'static str, &'static str, bool)> {
        let all = core::sim::Cheats::default().entries();
        let bits_of = |slot: u8| cheat_bits(&walk.sim.cheats_for_slot(usize::from(slot)));
        let mut rows = Vec::new();
        let mut out = Vec::new();
        if self.is_host() {
            let humans: Vec<u8> = walk.netplay.profiles.keys().copied().collect();
            if self.cheat_target != ALL_SLOTS && !humans.contains(&self.cheat_target) {
                self.cheat_target = ALL_SLOTS;
            }
            rows.push(CheatRow::Target);
            out.push((
                "lan_target",
                target_label(self.cheat_target, &self.name_of(self.cheat_target)),
                false,
            ));
            rows.push(CheatRow::Awards);
            out.push(("lan_awards", "Awards (everyone)", !walk.netplay.awards_off));
            let on = |i: usize| match self.cheat_target {
                ALL_SLOTS => !humans.is_empty() && humans.iter().all(|s| bits_of(*s) & 1 << i != 0),
                s => bits_of(s) & 1 << i != 0,
            };
            for (i, e) in all.iter().enumerate() {
                rows.push(CheatRow::Cheat(i as u8));
                out.push((e.0, e.1, on(i)));
            }
        } else {
            let mine = bits_of(self.peer.local_slot);
            let policy = walk.netplay.cheat_policy;
            for (i, e) in all.iter().enumerate() {
                // Allowed ones, and any still on (switching off is always allowed).
                if policy & 1 << i != 0 || mine & 1 << i != 0 {
                    rows.push(CheatRow::Cheat(i as u8));
                    out.push((e.0, e.1, mine & 1 << i != 0));
                }
            }
        }
        self.cheat_rows = rows;
        out
    }

    /// Switch cheat `id` for the host's target or the guest's own slot.
    fn toggle_cheat(&mut self, walk: &WalkSim, id: u8) {
        let bit = 1u32 << id;
        if self.is_host() {
            let slot = self.cheat_target;
            let on = match slot {
                ALL_SLOTS => !walk
                    .netplay
                    .profiles
                    .keys()
                    .all(|s| cheat_bits(&walk.sim.cheats_for_slot(usize::from(*s))) & bit != 0),
                s => cheat_bits(&walk.sim.cheats_for_slot(usize::from(s))) & bit == 0,
            };
            self.peer
                .host_command(&mut self.link, &GameCommand::SetSlotCheat { slot, id, on });
        } else {
            let on =
                cheat_bits(&walk.sim.cheats_for_slot(usize::from(self.peer.local_slot))) & bit == 0;
            self.peer
                .request(&mut self.link, &GameRequest::CheatToggle { id, on });
        }
    }

    /// The host's Cheats page target, cycled: everyone, then each human.
    fn next_target(&mut self, walk: &WalkSim) {
        let humans: Vec<u8> = walk.netplay.profiles.keys().copied().collect();
        self.cheat_target = match self.cheat_target {
            ALL_SLOTS => humans.first().copied().unwrap_or(ALL_SLOTS),
            t => humans.iter().copied().find(|s| *s > t).unwrap_or(ALL_SLOTS),
        };
    }

    /// A pause-menu action in a LAN round (module rules). True when it was
    /// handled here; the single-player path runs otherwise.
    #[allow(clippy::too_many_arguments)]
    pub fn pause_action(
        &mut self,
        walk: &WalkSim,
        a: ui::menu::MenuAction,
        paused: &mut bool,
        menu: &mut game::pause::PauseMenu,
        profile: &mut game::profile::Profile,
        session: &mut ui::shell::SessionOptions,
    ) -> bool {
        use ui::menu::MenuAction;
        let host = self.is_host();
        let save = |profile: &mut game::profile::Profile| {
            if let Err(e) = profile.save() {
                eprintln!("profile autosave failed: {e:#}");
            }
        };
        match a {
            MenuAction::ToggleCheat(i) => match self.cheat_rows.get(i).copied() {
                Some(CheatRow::Target) => self.next_target(walk),
                Some(CheatRow::Awards) => {
                    let on = walk.netplay.awards_off;
                    self.lobby.awards = on;
                    self.peer
                        .host_command(&mut self.link, &GameCommand::SetAwardPolicy { on });
                }
                Some(CheatRow::Cheat(id)) => self.toggle_cheat(walk, id),
                None => {}
            },
            MenuAction::SetAiCap(c) => {
                if host {
                    let pct = c.clamp(10, ui::population::MAX_TEAM_PCT);
                    self.peer
                        .host_command(&mut self.link, &GameCommand::SetTeamSize { pct });
                    profile.settings.team_size_pct = pct;
                    session.con_numbots = pct as i32;
                    session.num_bots = session.con_numbots;
                    save(profile);
                }
            }
            MenuAction::SetReinforcements(c) => {
                if host {
                    let pct = c.clamp(10, 500);
                    self.peer
                        .host_command(&mut self.link, &GameCommand::SetReinforcements { pct });
                    profile.settings.reinforcement_pct = pct;
                    session.con_mult = pct as i32;
                    save(profile);
                }
            }
            MenuAction::SetCareerAwards(on) => {
                // Each player's own; the host's goes on its slot's tick.
                if host {
                    self.peer.host_command(
                        &mut self.link,
                        &GameCommand::SetCareerAwards { slot: 0, on },
                    );
                }
                profile.settings.career_awards = on;
                save(profile);
            }
            // Sim inputs fixed by the round's setup: kept for the next one.
            MenuAction::SetAssist(v) => {
                profile.settings.assist_intensity = game::profile::clamp_assist_intensity(v);
                save(profile);
            }
            MenuAction::SetPadLookSensitivity(v) => {
                profile.settings.pad_look_sensitivity = game::profile::clamp_pad_look(v);
                save(profile);
            }
            MenuAction::Respawn => {
                if *paused
                    && menu.confirm == Some(ui::menu::Popup::Respawn)
                    && walk.sim.outcome.is_none()
                {
                    self.respawn = walk.sim.unit(walk.player).is_some();
                    *paused = false;
                }
                menu.confirm = None;
                menu.cursor = 0;
            }
            // The class picker writes the sim: not in a LAN round.
            MenuAction::OpenClassPicker | MenuAction::SelectClass(_) => {}
            MenuAction::Restart | MenuAction::RequestRestart => {}
            _ => return false,
        }
        true
    }

    /// A Morse word typed on the Cheats page (module rules); the HUD line.
    pub fn morse_word(
        &mut self,
        word: &str,
        walk: &mut WalkSim,
        cheats: &mut game::cheats::Session,
    ) -> String {
        let host = self.is_host();
        let local = self.peer.local_slot;
        let mine = cheat_bits(&walk.sim.cheats_for_slot(usize::from(local)));
        let mut want = None;
        if let Some(id) = core::sim::Cheats::id_for_word(word) {
            if let Some(i) = cheat_index(id) {
                want = Some(mine ^ 1 << i);
            }
        } else if word == "cheat" {
            let mut c = walk.sim.cheats_for_slot(usize::from(local));
            c.toggle_five();
            want = Some(cheat_bits(&c));
        }
        if let Some(bits) = want {
            let changed = bits ^ mine;
            let policy = walk.netplay.cheat_policy;
            let mut refused = false;
            for id in 0..game::netplay::CHEAT_COUNT {
                if changed & 1 << id == 0 {
                    continue;
                }
                let on = bits & 1 << id != 0;
                if host {
                    self.peer.host_command(
                        &mut self.link,
                        &GameCommand::SetSlotCheat {
                            slot: local,
                            id,
                            on,
                        },
                    );
                } else if on && policy & 1 << id == 0 {
                    refused = true;
                } else {
                    self.peer
                        .request(&mut self.link, &GameRequest::CheatToggle { id, on });
                }
            }
            return if refused {
                format!("cheat {word}: not allowed by the host")
            } else {
                format!(
                    "cheat {word}: sent to the {}",
                    if host { "battle" } else { "host" }
                )
            };
        }
        // Display and sound words never touch the sim; while they are
        // soft-locked (`game::cheats::WORDS_LOCKED`) they do nothing at all.
        cheats.apply_word(word, &mut walk.sim)
    }

    /// The meter's figures for this frame.
    pub fn update_meter(&mut self, walk: Option<&WalkSim>, report_ticks: u32) {
        let now = Instant::now();
        let p = &self.peer;
        let pending = p.pending_inputs().len() as u32;
        let ps = &self.view.predictor.stats;
        let mut m = NetMeter {
            host: p.is_host(),
            slot: p.local_slot,
            delay_ticks: pending,
            delay_ms: pending as f32 * 1000.0 / 30.0,
            dilation: p.pacer().dilation() as f32,
            buffered: p.queued() as u32,
            buffer_target: p.playback().target() as f32,
            buffer_level: p.playback().level() as f32,
            desyncs: p.stats.desyncs,
            predict_mode: ps.mode,
            predict_depth: ps.depth as u32,
            predict_ms: ps.cost_ms as f32,
            corrections: ps.corrections,
            tick: p.next_tick(),
            mode: format!("{:?}", p.mode()),
            ..NetMeter::default()
        };
        let reported =
            p.next_tick().saturating_sub(1) / game::netplay::HASH_EVERY * game::netplay::HASH_EVERY;
        if report_ticks > 0
            && p.stats.hash_reports > 0
            && self.last_hash.is_none_or(|(t, _)| t != reported)
        {
            self.last_hash = Some((reported, now));
        }
        if let Some((t, at)) = self.last_hash {
            m.hash_tick = Some(t);
            m.hash_age_s = at.elapsed().as_secs_f32();
        }
        let (bytes_in, bytes_out) = match &self.link {
            NetLink::Host(s) => {
                let st = s.stats();
                m.queue_depth = st.local_depth;
                m.starved = st.local_starved;
                m.slack_ms = st.local_slack_us.map(|v| v as f32 / 1e3);
                m.peers = st
                    .peers
                    .iter()
                    .map(|g| MeterPeer {
                        slot: g.slot,
                        name: g.name.clone(),
                        ping_ms: g.rtt_ms as f32,
                        depth: g.queue_depth,
                        starved: g.starved,
                        trouble: g.trouble,
                    })
                    .collect();
                st.peers.iter().fold((0, 0), |(i, o), g| {
                    (i + g.link.bytes_recv, o + g.link.bytes_sent)
                })
            }
            NetLink::Client(s) => {
                let st = s.stats();
                m.ping_ms = st.rtt_ms as f32;
                m.ping_var_ms = st.rttvar_ms as f32;
                m.queue_depth = st.queue_depth;
                m.starved = st.starved;
                m.slack_ms = st.slack_us.map(|v| v as f32 / 1e3);
                m.jitter_ticks = st.jitter_ticks as f32;
                let sent = st.link.packets_sent.max(1);
                m.loss_pct = st.link.packets_lost as f32 * 100.0 / sent as f32;
                m.resends = st.link.resends;
                (st.link.bytes_recv, st.link.bytes_sent)
            }
        };
        let secs = self.rate.0.elapsed().as_secs_f32();
        if secs >= 1.0 {
            self.rates = (
                bytes_in.saturating_sub(self.rate.1) as f32 / secs,
                bytes_out.saturating_sub(self.rate.2) as f32 / secs,
            );
            self.rate = (now, bytes_in, bytes_out);
        }
        (m.bytes_in_s, m.bytes_out_s) = self.rates;
        if let Some(w) = walk {
            m.tick = w.sim.tick as u32;
        }
        self.meter = m;
    }

    /// Catching up with a running round: the hook's sounds and effects
    /// are skipped for the replayed ticks.
    pub fn catching_up(&self) -> bool {
        self.peer.mode() == Mode::CatchingUp
    }

    /// The centred notice still showing.
    pub fn banner_text(&self) -> Option<&str> {
        if let Some((t, _)) = &self.ended {
            return Some(t);
        }
        self.banner
            .as_ref()
            .filter(|(_, at)| at.elapsed().as_secs_f32() < BANNER_SECS)
            .map(|(t, _)| t.as_str())
    }

    /// The session ended and its banner has shown long enough.
    pub fn ended_done(&self) -> bool {
        self.ended
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed().as_secs_f32() >= ENDED_SECS)
    }
}

/// The host's Cheats target row label (leaked once per distinct label: a
/// handful per session).
fn target_label(slot: u8, name: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static LABELS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let text = if slot == ALL_SLOTS {
        "Cheats for: everyone".to_owned()
    } else if slot == 0 {
        "Cheats for: you (host)".to_owned()
    } else {
        format!("Cheats for: {name} (slot {slot})")
    };
    let mut map = LABELS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.entry(text.clone())
        .or_insert_with(|| Box::leak(text.into_boxed_str()))
}

/// What a frame's session events ask of the App.
pub(super) enum NetAction {
    /// Build this round (a guest's `LoadRound`); `rejoin`: the round in
    /// play again (a desync's or a reconnect's catch-up).
    Load {
        setup: Box<RoundSetup>,
        rejoin: bool,
    },
    /// A HUD line.
    Line(String),
    /// The session is over.
    Ended(String),
    /// Back to the lobby with this line (benched by the host).
    ToLobby(String),
}

/// Draws the net meter panel (top right, under the FPS counter's corner);
/// `quality`: automatic quality's note (LAN §8b.2), its last line.
pub(super) fn draw_meter(ctx: &ui::egui::Context, m: &NetMeter, quality: Option<&str>) {
    use ui::egui::{Align2, Area, Color32, Frame, Id, Label, Margin, RichText, TextWrapMode};
    Area::new(Id::new("net_meter"))
        .interactable(false)
        .anchor(Align2::RIGHT_TOP, ui::egui::vec2(-8.0, 8.0))
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(180))
                .inner_margin(Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    let quality = quality.map(|q| format!("quality: {q}"));
                    for line in m.lines().into_iter().chain(quality) {
                        ui.add(
                            Label::new(RichText::new(line).monospace().size(12.0))
                                .wrap_mode(TextWrapMode::Extend),
                        );
                    }
                });
        });
}

/// Draws the other players' names over their heads (LAN §5b,
/// `game::name_tags`): the HUD's friend or enemy colour, faded with
/// distance, under the HUD's own panels.
pub(super) fn draw_name_tags(
    ctx: &ui::egui::Context,
    tags: &[game::name_tags::NameTag],
    view_proj: glam::Mat4,
) {
    use ui::egui::{Align2, Color32, FontId, Id, LayerId, Order};
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Background, Id::new("name_tags")));
    let [enemy, _, friend] = ui::hud::RETICLE_COLORS;
    for t in tags {
        let Some([x, y]) =
            game::name_tags::to_screen(view_proj, t.at, [screen.width(), screen.height()])
        else {
            continue;
        };
        let rgb = if t.friend { friend } else { enemy };
        let a = (t.alpha * 255.0).round() as u8;
        let pos = screen.min + ui::egui::vec2(x, y);
        let font = FontId::proportional(15.0);
        painter.text(
            pos + ui::egui::vec2(1.0, 1.0),
            Align2::CENTER_BOTTOM,
            &t.name,
            font.clone(),
            Color32::from_black_alpha(a / 2 + a / 4),
        );
        painter.text(
            pos,
            Align2::CENTER_BOTTOM,
            &t.name,
            font,
            Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], a),
        );
    }
}

/// Draws a centred banner (a desync, the session's end).
pub(super) fn draw_banner(ctx: &ui::egui::Context, text: &str) {
    use ui::egui::{Align2, Area, Color32, Frame, Id, Margin, RichText};
    Area::new(Id::new("net_banner"))
        .interactable(false)
        .anchor(Align2::CENTER_CENTER, ui::egui::vec2(0.0, -80.0))
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(Margin::symmetric(18, 10))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .size(22.0)
                            .color(Color32::from_rgb(255, 210, 120)),
                    );
                });
        });
}

impl App {
    /// The session's events outside a battle frame (the waiting and
    /// loading screens) and in one: builds, lines, the end.
    pub(super) fn net_idle(&mut self) {
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        let actions = n.poll();
        for a in actions {
            match a {
                NetAction::Load { setup, rejoin } => {
                    if rejoin {
                        // The same round goes on from its log: the copy
                        // torn down is not a round of the career.
                        self.round_recorded = true;
                    }
                    self.net_load(*setup);
                }
                NetAction::Line(text) => {
                    println!("lan: {text}");
                    let tick = self.walk.as_ref().map_or(0, |w| w.0.sim.tick);
                    self.feed.say(text, ui::hud::message_color::YELLOW, tick);
                }
                NetAction::ToLobby(text) => {
                    println!("lan: {text}");
                    self.net_to_lobby(Some(text));
                }
                NetAction::Ended(text) => {
                    println!("lan: session ended: {text}");
                    if let Some(n) = self.netplay.as_mut() {
                        if n.ended.is_none() {
                            n.ended = Some((text, Instant::now()));
                        }
                    }
                }
            }
        }
        // The guests' lobby follows the host in a battle too (its roster,
        // the decided round's ready check).
        self.host_publish();
    }

    /// A round to build: the battle in play (if any) goes, the loading
    /// screen builds the new one.
    pub(super) fn net_load(&mut self, setup: RoundSetup) {
        if self.walk.is_some() {
            // Keep the session through the battle's teardown.
            let n = self.netplay.take();
            self.leave_battle();
            self.netplay = n;
        }
        if self.loading.is_some() {
            self.finish_loading();
        }
        let key = setup.map.clone();
        let mode = setup.mode.clone();
        if let Some(n) = self.netplay.as_mut() {
            n.loading = Some(setup);
        }
        if !self.begin_loading_key(&key, mode, None) {
            // The lobby waits for the next round (the host benches a joiner
            // without the map); the command line's session ends.
            let in_lobby = self.netplay.as_ref().is_some_and(|n| n.lobby.ia.is_some());
            if in_lobby {
                self.net_round_failed(format!("The host's map {key} is not installed here"));
                return;
            }
            if let Some(n) = self.netplay.as_mut() {
                n.loading = None;
                n.ended = Some((
                    format!("The host's map {key} is not installed here"),
                    Instant::now(),
                ));
            }
            self.screen = Screen::NetWait;
        }
    }

    /// The round this guest was given cannot be built here: the host is
    /// told (it benches this slot at once instead of holding its load
    /// barrier for it) and the lobby waits for the next round.
    pub(super) fn net_round_failed(&mut self, line: String) {
        if let Some(n) = self.netplay.as_mut() {
            n.peer
                .leave_round_for_lobby(&mut n.link, LeftWhy::LoadFailed);
        }
        self.net_to_lobby(Some(line));
    }

    /// The host's round is due: build its setup and load it.
    fn net_host_start(&mut self) -> Result<()> {
        let Some(n) = self.netplay.as_ref() else {
            return Ok(());
        };
        let source_game = game::assets::discover_maps(&self.assets_dir)
            .into_iter()
            .find(|(k, ..)| *k == n.args.map)
            .map_or_else(|| "bf2".to_owned(), |(_, g, _)| g);
        let setup = n.host_setup(&source_game, &self.session, &self.profile);
        let n = self.netplay.as_mut().expect("session");
        let setup = n.host_start(&setup)?;
        self.net_load(setup);
        Ok(())
    }

    /// The waiting screen (connecting, or the host waiting for players).
    pub(super) fn redraw_net_wait(&mut self) -> Result<()> {
        self.net_idle();
        if let Some(n) = self.netplay.as_ref() {
            if n.host_due() && n.ended.is_none() {
                if let Err(e) = self.net_host_start() {
                    eprintln!("lan: cannot start the round: {e:#}");
                    if let Some(n) = self.netplay.as_mut() {
                        n.ended = Some((format!("Cannot start the round: {e:#}"), Instant::now()));
                    }
                }
            }
        }
        if !matches!(self.screen, Screen::NetWait) {
            return Ok(());
        }
        let back = self.menu_edges() & 32 != 0;
        let (status, ended, done) = match self.netplay.as_ref() {
            Some(n) => (
                n.status.clone(),
                n.banner_text().map(str::to_owned),
                n.ended_done(),
            ),
            None => (String::new(), None, true),
        };
        if done || back {
            if let Some(n) = self.netplay.take() {
                n.shutdown();
            }
            self.init_shell();
            return Ok(());
        }
        if let (Some(r), Some(overlay), Some(window)) =
            (&mut self.renderer, &mut self.overlay, &self.window)
        {
            let guests = self
                .netplay
                .as_ref()
                .map(|n| {
                    n.peer
                        .guests()
                        .values()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let paint = overlay.begin(window, |ctx| {
                ui::egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.heading(status.as_str());
                        if !guests.is_empty() {
                            ui.label(format!("Players here: {}", guests.join(", ")));
                        }
                        ui.add_space(12.0);
                        ui.label("Back leaves.");
                    });
                });
                if let Some(t) = &ended {
                    draw_banner(ctx, t);
                }
            });
            r.render_with_overlay(
                &render::Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, view, size| overlay.paint(&paint, dev, q, enc, view, size)),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Only a joining load of the round already in play is a rejoin (its
    /// torn-down copy is not recorded): not a first mid-round join, not a
    /// new round.
    #[test]
    fn lan_a_rejoin_is_the_round_in_play_loaded_again() {
        let mut last = None;
        assert!(!note_load(&mut last, 3, true), "a mid-round join");
        assert!(note_load(&mut last, 3, true), "a desync rejoin");
        assert!(!note_load(&mut last, 4, false), "the next round");
        assert!(note_load(&mut last, 4, true));
        assert!(!note_load(&mut last, 5, true), "a new round joined late");
    }

    /// The launch flags: the host's map, port, players and cheat policy;
    /// a guest's address; unknown and host-only flags refused for a guest.
    #[test]
    fn lan_args_parse_the_host_and_guest_flags() {
        let h = LanArgs::parse(
            &args(&[
                "--port",
                "40000",
                "--map",
                "bf1_tat2",
                "--players",
                "3",
                "--cheats",
                "ammo,stamina",
                "--name",
                "Ann",
            ]),
            true,
        )
        .unwrap();
        assert_eq!((h.port, h.map.as_str(), h.players), (40000, "bf1_tat2", 3));
        assert_eq!(
            h.cheats,
            1 << cheat_index("ammo").unwrap() | 1 << cheat_index("stamina").unwrap()
        );
        assert_eq!(h.name.as_deref(), Some("Ann"));
        assert_eq!(
            LanArgs::parse(&args(&["--cheats", "all"]), true)
                .unwrap()
                .cheats,
            ALL_CHEATS
        );
        let g = LanArgs::parse(&args(&["10.0.0.2:37450"]), false).unwrap();
        assert_eq!(g.addr.as_deref(), Some("10.0.0.2:37450"));
        assert!(LanArgs::parse(&args(&["10.0.0.2", "--map", "x"]), false).is_err());
        assert!(LanArgs::parse(&args(&[]), false).is_err());
        assert!(LanArgs::parse(&args(&["--cheats", "nope"]), true).is_err());
    }
}
