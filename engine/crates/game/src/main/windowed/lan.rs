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
//! - A round start (not a join or a rejoin) is one moment everywhere: each
//!   peer plays at least its loading presentation, builds and reports
//!   `Loaded`, and holds its last image until the host's `Start` names
//!   tick 0 ([`NetplaySession::start_at`]); then every screen goes to the
//!   battle together. Only the host skips the presentation (the
//!   single-player two presses), and its skip ends every peer's
//!   (`Peer::skip_loading`); the build and the start are unchanged. A
//!   guest's presses do nothing. A peer still building then (the
//!   barrier's timeout) enters when built, behind its spawn screen. Joins
//!   enter as soon as they are built and catch up.
//! - Between rounds every peer waits in the lobby (`lobby.rs`): a round's
//!   end, a copy that plays differently (§4.3: the host benches that guest
//!   and tells it which group differs; a mid-round joiner's build is
//!   checked too), a map a mid-round joiner lacks, a round a
//!   guest could not build or a guest's second desync in one round (within
//!   5 minutes, §4.7). A
//!   guest that leaves a round on its own tells the host, which benches it
//!   so its barrier and seat do not wait.
//!   The host ends a decided round with `EndRound` and keeps the round's
//!   ready check ([`NetplaySession::round_decided`]).
//! - A desync rejoin rebuilds the round in play: the abandoned copy is not
//!   recorded into the career (the rebuilt one is, at its outcome).
//! - The host checks its own game first on a desync (`netplay::selfcheck`,
//!   a worker thread with its own copy of the map,
//!   `round_setup::rebuilder`): a host game that parted is corrected in
//!   place and the guests play on; a second in one round ends it.

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
    fn submit_inputs(&mut self, pads: &[(u8, PadState)], view_tick: u32) {
        link!(self, x => x.submit_inputs(pads, view_tick))
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
    fn set_local_away(&mut self, local: u8, slot: u8, away: bool) {
        link!(self, x => Link::set_local_away(x, local, slot, away))
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
pub(in super::super) fn run_lan(args: &[String], host: bool, no_mods: bool) -> Result<()> {
    let a = LanArgs::parse(args, host)?;
    let _ = game::mods_runtime::mount_enabled_mods(
        &extensions::profile::discover_data_dir(),
        &a.assets,
        &a.missions,
        no_mods,
    );
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
    super::run_app(event_loop, &mut app)?;
    super::end_after_close(&app)
}

/// Whether a `LoadRound` (`round_id`, `joining`) rebuilds the round in play
/// (a desync's or a reconnect's rejoin), given the last one's id in `last`,
/// which it becomes.
fn note_load(last: &mut Option<u32>, round_id: u32, joining: bool) -> bool {
    let rejoin = joining && *last == Some(round_id);
    *last = Some(round_id);
    rejoin
}

/// A diagnostic line for the session log only (`netplay::diag`).
fn log(line: &str) {
    println!("lan: {line}");
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

/// A further player of this machine in a LAN session (splitscreen, issue
/// #194 B19): a Players-screen seat (1-3) with its profile. The host's get
/// their slot and unique name from `HostCommand::AddLocal`; a guest's go
/// in its `Hello` and get their slots with `Welcome`.
#[derive(Clone, Debug)]
pub(super) struct LanLocal {
    pub seat: u8,
    /// The name asked for, then the unique one the host gave.
    pub name: String,
    pub profile: PlayerProfile,
    /// Its slot once the host gave one.
    pub slot: Option<u8>,
}

/// The windowed game's LAN session (module header).
pub(super) struct NetplaySession {
    pub peer: Peer,
    pub link: NetLink,
    pub view: LocalView,
    pub args: LanArgs,
    pub profile: PlayerProfile,
    /// This machine's further players, seat order (splitscreen B19).
    pub locals: Vec<LanLocal>,
    /// The round the loading screen is building.
    pub loading: Option<RoundSetup>,
    /// The round's start (`Notice::Started`): when it was heard and how
    /// long after that tick 0 begins. Every peer's loading screen ends then.
    pub start: Option<(Instant, f64)>,
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
    /// Every human's session name this peer has heard, by slot: the
    /// rounds' seats, the joins and the roster (issue #168). Kept after a
    /// player leaves, for his lines and the stats pages. Presentation only.
    names: std::collections::BTreeMap<u8, String>,
    /// A guest's entry in this computer's recent games (`lobby.rs`).
    pub recent: super::lobby::RecentNote,
    /// The session's diagnostic log lines (`netplay::diag`) and its clock.
    diag: game::netplay::diag::Diag,
    began: Instant,
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
        let mut peer = Peer::host();
        // Its self-check rebuilds rounds as every peer does (module rules).
        peer.rebuild = Some(super::super::round_setup::rebuilder(
            args.assets.clone(),
            args.missions.clone(),
        ));
        let mut n = Self::with(args, profile, link, peer, status);
        n.lobby.content = content;
        n.lobby.max_players = max_players;
        n.lobby.lag_comp = lag_comp;
        Ok(n)
    }

    /// Joins the host at `host`.
    pub fn join(args: LanArgs, profile: PlayerProfile, host: std::net::SocketAddr) -> Result<Self> {
        Self::join_locals(args, profile, host, Vec::new())
    }

    /// Joins the host at `host` with this machine's further players
    /// `locals` (splitscreen B19): its `Hello` asks a slot for each, all or
    /// none.
    pub fn join_locals(
        args: LanArgs,
        profile: PlayerProfile,
        host: std::net::SocketAddr,
        locals: Vec<LanLocal>,
    ) -> Result<Self> {
        let build = game::netplay::build_id();
        let transport = UdpTransport::client().context("bind a UDP port")?;
        // One token per game process (LAN §4.5): a rejoin reclaims the slot
        // and name the host keeps for it.
        let token = game::netplay::lobby::session_token();
        let mut config = ClientConfig::new(build.clone(), profile.name.clone(), token);
        config.profile = profile.encode();
        let content = game::netplay::ContentManifest::scan(&args.assets);
        config.content = content.encode();
        config.locals = locals
            .iter()
            .map(|l| net::msg::LocalHello {
                seat: l.seat,
                name: l.name.clone(),
                profile: l.profile.encode(),
            })
            .collect();
        let status = format!("Connecting to {host}\u{2026}");
        let link = NetLink::Client(NetSession::join(config, host, transport));
        let mut n = Self::with(args, profile, link, Peer::client(), status);
        n.lobby.content = content;
        n.locals = locals;
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
            locals: Vec::new(),
            loading: None,
            start: None,
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
            names: Default::default(),
            recent: Default::default(),
            diag: Default::default(),
            began: Instant::now(),
        }
    }

    /// The host seats this machine's further players `locals`
    /// (splitscreen B19), between frames: each asks the network core for a
    /// slot ([`Notice::LocalAdded`] answers). Called as the session opens,
    /// before any round's inputs.
    pub fn add_host_locals(&mut self, locals: Vec<LanLocal>) {
        if !self.is_host() {
            return;
        }
        for l in &locals {
            self.link.host(HostCommand::AddLocal {
                seat: l.seat,
                name: l.name.clone(),
                profile: l.profile.encode(),
            });
        }
        self.locals.extend(locals);
    }

    /// When this peer's loading screen ends for the round's start (module
    /// rules): `Start` heard plus its wait. None before `Start`.
    pub fn start_at(&self) -> Option<Instant> {
        // The wait is a `u32` of microseconds over 1e6: never negative.
        self.start
            .map(|(heard, wait)| heard + std::time::Duration::from_secs_f64(wait))
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
        self.seat_humans(setup, profile)
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
        self.seat_humans(setup, profile)
    }

    pub(super) fn current_profile(&self, profile: &game::profile::Profile) -> PlayerProfile {
        let mut current = PlayerProfile::from_profile(profile);
        current.name.clone_from(&self.profile.name);
        current
    }

    /// Each further local's session profile from its seat's lent profile as the round left it
    /// (issue #194 B20): its medals, class unlocks, assist and awards row carry into the next
    /// round (the host seats its own from `locals`, a guest sends them in [`Self::ready_locals`]).
    /// The session name stays.
    pub(super) fn refresh_local_profiles(
        &mut self,
        seat_profiles: &[Option<game::profile::Profile>; 4],
    ) {
        for l in &mut self.locals {
            if let Some(p) = seat_profiles
                .get(usize::from(l.seat))
                .and_then(Option::as_ref)
                .filter(|_| l.seat != 0)
            {
                let mut fresh = PlayerProfile::from_profile(p);
                fresh.name.clone_from(&l.profile.name);
                for (count, had) in fresh.medals.iter_mut().zip(l.profile.medals) {
                    *count = (*count).max(had);
                }
                l.profile = fresh;
            }
        }
    }

    /// A guest's Ready for each of its further locals, with its own profile (issue #194 B20,
    /// `ClientCommand::ReadyLocal`): the host seats each of this machine's players on its own
    /// medals and unlocks next round.
    pub(super) fn ready_locals(&self, round_id: u32) {
        let NetLink::Client(s) = &self.link else {
            return;
        };
        for i in 1..net::MAX_LOCALS {
            let Some(slot) = self.peer.local_slot_of(i) else {
                break;
            };
            if let Some(l) = self.locals.iter().find(|l| l.slot == Some(slot)) {
                s.send(net::ClientCommand::ReadyLocal {
                    local: i as u8,
                    round_id,
                    profile: l.profile.encode(),
                });
            }
        }
    }

    /// Every connected human seated, the host's cheat policy.
    fn seat_humans(&self, mut setup: RoundSetup, profile: &game::profile::Profile) -> RoundSetup {
        setup.map = game::mods_runtime::select_world_variant(
            &setup.map,
            &setup.mode,
            &setup.armies,
            setup.seed,
            true,
        );
        setup.cheat_policy = self.args.cheats;
        setup.awards = self.lobby.awards;
        setup.humans.push(HumanSeat {
            slot: 0,
            name: self.profile.name.clone(),
            team: None,
            cheats: 0,
            profile: self.current_profile(profile),
        });
        // The host machine's further players (B19), each with its seat's profile.
        for l in &self.locals {
            if let Some(slot) = l.slot.filter(|s| self.peer.host_local(*s)) {
                setup.humans.push(HumanSeat {
                    slot,
                    name: l.name.clone(),
                    team: None,
                    cheats: 0,
                    profile: l.profile.clone(),
                });
            }
        }
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
        if !self.is_host() {
            let t = self.now_s();
            if let Some(line) = self.diag.built(t) {
                log(&line);
            }
        }
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
            .map(|g| g.name.clone())
            .or_else(|| self.names.get(&slot).cloned())
            .unwrap_or_else(|| format!("Player {}", u32::from(slot) + 1))
    }

    /// Every human's session name this peer has heard, by slot (the kill
    /// feed's and the stats pages' names, issue #168).
    pub fn names(&self) -> &std::collections::BTreeMap<u8, String> {
        &self.names
    }

    /// Learns the names of the roster as it stands (the host's own, or
    /// the one the host last sent).
    fn learn_names(&mut self) {
        for p in self.roster() {
            // This machine's further players (B19) take the unique name the
            // host gave them, profile too (a guest's are only told it here).
            if let Some(l) = self.locals.iter_mut().find(|l| l.slot == Some(p.slot)) {
                if l.name != p.name {
                    l.name.clone_from(&p.name);
                    l.profile.name.clone_from(&p.name);
                }
            }
            if self.names.get(&p.slot) != Some(&p.name) {
                self.names.insert(p.slot, p.name);
            }
        }
    }

    /// The session time for a diagnostic line (`netplay::diag`).
    fn now_s(&self) -> f64 {
        self.began.elapsed().as_secs_f64()
    }

    /// One frame of the session's events: the rounds to build, the lines
    /// to show, the end.
    pub fn poll(&mut self) -> Vec<NetAction> {
        let mut out = Vec::new();
        // Before this frame's notices: a guest who leaves now keeps the
        // name the roster gave him.
        self.learn_names();
        for n in self.peer.poll(&mut self.link) {
            match n {
                Notice::Welcome { slot } => {
                    // A guest's further players' slots, in its `Hello`'s order (B19).
                    let extra: Vec<u8> = self.peer.extra_locals().iter().map(|q| q.slot).collect();
                    for (l, s) in self.locals.iter_mut().zip(extra) {
                        l.slot = Some(s);
                    }
                    self.status = format!("Connected as player {slot}; waiting for the round");
                    out.push(NetAction::Welcomed);
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
                    self.start = None;
                    if let Some(m) = ui::shell::MODES.iter().find(|m| m.subst() == setup.mode) {
                        self.recent.mode = m.english().to_owned();
                    }
                    let rejoin = note_load(&mut self.round_in_play, round_id, joining);
                    let what = format!("{} {}", setup.map, setup.mode);
                    let t = self.now_s();
                    log(&self.diag.load_began(t, round_id, &what, joining, []));
                    for h in &setup.humans {
                        self.names.insert(h.slot, h.name.clone());
                    }
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
                    self.names.insert(slot, name.clone());
                    let target = self
                        .peer
                        .guests()
                        .get(&slot)
                        .and_then(|g| g.content.as_ref())
                        .map(|c| c.target.clone())
                        .unwrap_or_default();
                    let t = self.now_s();
                    log(&self.diag.joined(t, slot, &name, &target, reconnect));
                    out.push(NetAction::Line(format!(
                        "{name} {}",
                        if reconnect { "reconnected" } else { "joined" }
                    )));
                    let guest_content = self
                        .peer
                        .guests()
                        .get(&slot)
                        .and_then(|g| g.content.as_ref());
                    if let Some(reason) =
                        game::netplay::lobby::guest_mod_mismatch(&self.lobby.content, guest_content)
                    {
                        let line = format!("{name}: {reason}");
                        self.lobby.say(line.clone());
                        out.push(NetAction::Line(line));
                        if self.lobby.round_map.is_some() {
                            self.bench(slot, GameNotice::BackToLobby(reason));
                        }
                    } else if let Some((key, label)) = self.lobby.round_map.clone() {
                        // A mid-round joiner without the map waits in the lobby
                        // (one with it is checked by its build's digests).
                        if guest_content.is_some_and(|c| !c.maps.contains(&key)) {
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
                Notice::PlayerLeft { slot, reason } => {
                    let (t, name) = (self.now_s(), self.name_of(slot));
                    log(&self.diag.left(t, slot, &name, reason));
                    out.push(NetAction::Line(format!(
                        "{name} {}",
                        Self::left_text(reason)
                    )));
                }
                Notice::GuestTrouble { slot, on } => {
                    let (t, name) = (self.now_s(), self.name_of(slot));
                    if let Some(line) = self.diag.guest_trouble(t, slot, &name, on) {
                        log(&line);
                    }
                }
                Notice::HostTrouble(on) => {
                    let t = self.now_s();
                    if let Some(line) = self.diag.host_trouble(t, on) {
                        log(&line);
                    }
                }
                Notice::GuestLoaded { slot } => {
                    let (t, name) = (self.now_s(), self.name_of(slot));
                    if let Some(line) = self.diag.guest_loaded(t, slot, &name) {
                        log(&line);
                    }
                }
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
                // Ready is a machine's (B19): its local 0 speaks for it.
                Notice::PeerReady { slot } => {
                    self.lobby.ready.insert(self.peer.machine_of(slot));
                }
                Notice::LocalAdded { seat, added } => match added {
                    Ok((slot, name)) => {
                        if let Some(l) = self.locals.iter_mut().find(|l| l.seat == seat) {
                            l.slot = Some(slot);
                            l.name.clone_from(&name);
                            l.profile.name.clone_from(&name);
                        }
                        self.names.insert(slot, name.clone());
                        let t = self.now_s();
                        log(&self.diag.joined(t, slot, &name, "", false));
                        out.push(NetAction::LocalsChanged);
                    }
                    Err(reason) => {
                        let name = self
                            .locals
                            .iter()
                            .find(|l| l.seat == seat)
                            .map_or_else(|| format!("Player {}", seat + 1), |l| l.name.clone());
                        self.locals.retain(|l| l.seat != seat);
                        self.lobby
                            .say(game::netplay::lobby::local_refusal_text(&name, reason));
                    }
                },
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
                Notice::HostDiverged { tick, .. } => {
                    let line = if self.is_host() {
                        format!("Your game fell out of sync at tick {tick} and was corrected")
                    } else {
                        "The host's game was corrected".to_owned()
                    };
                    self.banner = Some((line.clone(), Instant::now()));
                    out.push(NetAction::Line(line));
                }
                Notice::RoundAbandoned { detail, .. } => {
                    if !detail.is_empty() {
                        println!("lan: round ended: {detail}");
                    }
                    let line = if self.is_host() {
                        "Your game could not be kept in sync; the round ends".to_owned()
                    } else {
                        "The host's game could not be kept in sync; the round ends".to_owned()
                    };
                    self.banner = Some((line.clone(), Instant::now()));
                    // Every peer stepped `EndRound`: the round's results,
                    // as when it is decided (the host's ready check).
                    out.push(NetAction::RoundOver(line));
                }
                Notice::SelfCheckFailed { tick, detail } => {
                    println!("lan: the self-check at tick {tick} gave no verdict: {detail}");
                }
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
                                    "Your game parted from the host's twice within 5 minutes{parted}"
                                )),
                            );
                            format!(
                                "{who} fell out of sync twice within 5 minutes{parted}; they wait in the lobby"
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
                Notice::Started { begin_in_s } => {
                    self.start = Some((Instant::now(), begin_in_s));
                    let t = self.now_s();
                    let names = self.names.clone();
                    let name_of = |s: u8| {
                        names
                            .get(&s)
                            .cloned()
                            .unwrap_or_else(|| format!("Player {}", u32::from(s) + 1))
                    };
                    if let Some(line) = self.diag.started(t, name_of) {
                        log(&line);
                    }
                }
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
        self.start = None;
        let bytes = self.peer.host_load(&mut self.link, setup);
        let setup = RoundSetup::decode(&bytes)?;
        let local = self.peer.local_slot;
        let guests = setup.humans.iter().map(|h| h.slot).filter(|&s| s != local);
        let what = format!("{} {}", setup.map, setup.mode);
        let t = self.now_s();
        log(&self
            .diag
            .load_began(t, self.peer.round_id(), &what, false, guests));
        Ok(setup)
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

    /// The cheat rows the pause menu of the local player on `slot` shows
    /// (module rules): the host's first player gets the target row then
    /// every cheat for that target; anyone else their own slot's cheats
    /// (cheats are per slot in LAN), a guest's only those the host allows
    /// (issue #194: a further player's page is theirs, not the first
    /// player's).
    pub fn cheat_entries(
        &mut self,
        walk: &WalkSim,
        slot: u8,
    ) -> Vec<(&'static str, &'static str, bool)> {
        let all = core::sim::Cheats::default().entries();
        let bits_of = |slot: u8| cheat_bits(&walk.sim.cheats_for_slot(usize::from(slot)));
        let mut rows = Vec::new();
        let mut out = Vec::new();
        if self.is_host() && slot == self.peer.local_slot {
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
            let mine = bits_of(slot);
            // The host machine sets any cheat; a guest asks within the policy.
            let policy = if self.is_host() {
                u32::MAX
            } else {
                walk.netplay.cheat_policy
            };
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

    /// Switch cheat `id` as the local player on `opener_slot`: for the
    /// host's target when that is the host's first player, else for
    /// `opener_slot` itself.
    fn toggle_cheat(&mut self, walk: &WalkSim, id: u8, opener_slot: u8) {
        let bit = 1u32 << id;
        if self.is_host() {
            let slot = if opener_slot == self.peer.local_slot {
                self.cheat_target
            } else {
                opener_slot
            };
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
            let slot = opener_slot;
            let on = cheat_bits(&walk.sim.cheats_for_slot(usize::from(slot))) & bit == 0;
            self.peer
                .request(&mut self.link, &GameRequest::CheatToggle { slot, id, on });
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
    /// handled here; the single-player path runs otherwise. `opener_slot` is
    /// the slot of the local player the action acts as, and `profile` their
    /// own profile (`None`: a seat with no profile, nothing of theirs is
    /// saved): a further player's per-player row is theirs (issue #194 B20).
    #[allow(clippy::too_many_arguments)]
    pub fn pause_action(
        &mut self,
        walk: &WalkSim,
        a: ui::menu::MenuAction,
        opener_slot: u8,
        paused: &mut bool,
        menu: &mut game::pause::PauseMenu,
        mut profile: Option<&mut game::profile::Profile>,
        session: &mut ui::shell::SessionOptions,
    ) -> bool {
        use ui::menu::MenuAction;
        let host = self.is_host();
        let mut save = |edit: &dyn Fn(&mut game::profile::Profile)| {
            if let Some(profile) = profile.as_deref_mut() {
                edit(profile);
                if let Err(e) = profile.save() {
                    eprintln!("profile autosave failed: {e:#}");
                }
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
                Some(CheatRow::Cheat(id)) => self.toggle_cheat(walk, id, opener_slot),
                None => {}
            },
            MenuAction::SetAiCap(c) => {
                if host {
                    let pct = c.clamp(10, ui::population::MAX_TEAM_PCT);
                    self.peer
                        .host_command(&mut self.link, &GameCommand::SetTeamSize { pct });
                    save(&|p| p.settings.team_size_pct = pct);
                    session.con_numbots = pct as i32;
                    session.num_bots = session.con_numbots;
                }
            }
            MenuAction::SetReinforcements(c) => {
                if host {
                    let pct = c.clamp(10, 500);
                    self.peer
                        .host_command(&mut self.link, &GameCommand::SetReinforcements { pct });
                    save(&|p| p.settings.reinforcement_pct = pct);
                    session.con_mult = pct as i32;
                }
            }
            MenuAction::SetCareerAwards(on) => {
                // Each player's own; the host's goes on its slot's tick.
                if host {
                    self.peer.host_command(
                        &mut self.link,
                        &GameCommand::SetCareerAwards {
                            slot: opener_slot,
                            on,
                        },
                    );
                }
                save(&|p| p.settings.career_awards = on);
            }
            // Sim inputs fixed by the round's setup: kept for the next one.
            MenuAction::SetAssist(v) => {
                save(&|p| {
                    p.settings.assist_intensity = game::profile::clamp_assist_intensity(v);
                });
            }
            MenuAction::SetPadLookSensitivity(v) => {
                save(&|p| p.settings.pad_look_sensitivity = game::profile::clamp_pad_look(v));
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

    /// [`Self::pause_action`] as local player `opener` of this machine (issue #194 B20): a
    /// further player's per-player rows write their own seat's lent profile and their own slot,
    /// never the first player's. A further player's Respawn is left to the walk
    /// (`local_seats::lan_pads`): `false`.
    #[allow(clippy::too_many_arguments)]
    pub fn opener_pause_action(
        &mut self,
        walk: &WalkSim,
        a: ui::menu::MenuAction,
        opener: usize,
        paused: &mut bool,
        menu: &mut game::pause::PauseMenu,
        host: &mut game::profile::Profile,
        seat_profiles: &mut [Option<game::profile::Profile>; 4],
        seats: Option<&game::local_seats::Seats<gilrs::GamepadId>>,
        session: &mut ui::shell::SessionOptions,
    ) -> bool {
        if opener > 0 && a == ui::menu::MenuAction::Respawn {
            return false;
        }
        let slot = walk.local_slot_of(opener);
        let profile = super::menu_actions::opener_profile(host, seat_profiles, seats, opener);
        self.pause_action(walk, a, slot, paused, menu, profile, session)
    }

    /// A Morse word typed on the Cheats page of the local player on `local`
    /// (module rules): it changes that player's own slot (issue #194); the
    /// HUD line.
    pub fn morse_word(
        &mut self,
        word: &str,
        local: u8,
        walk: &mut WalkSim,
        cheats: &mut game::cheats::Session,
    ) -> String {
        let host = self.is_host();
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
                    self.peer.request(
                        &mut self.link,
                        &GameRequest::CheatToggle {
                            slot: local,
                            id,
                            on,
                        },
                    );
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
    /// The host accepted this guest (a recent game from now on).
    Welcomed,
    /// Back to the lobby with this line (benched by the host).
    ToLobby(String),
    /// The round ended without being decided (the host abandoned it): to
    /// the lobby's results with this line, as after a decided round.
    RoundOver(String),
    /// This machine's further players changed (splitscreen B19): a round
    /// in play makes its walk's locals the peer's.
    LocalsChanged,
}

/// Draws the net meter panel (top right, under the FPS counter's corner);
/// `quality`: automatic quality's note (LAN §8b.2), its last line.
pub(super) fn draw_meter(ctx: &ui::egui::Context, m: &NetMeter, quality: Option<&str>) {
    use ui::egui::{Align2, Area, Color32, Frame, Id, Label, Margin, TextWrapMode};
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
                            Label::new(ui::typography::egui_text(
                                ctx,
                                &ui::typography::READOUT,
                                ui::typography::tint::HELP,
                                &line,
                            ))
                            .wrap_mode(TextWrapMode::Extend),
                        );
                    }
                });
        });
}

/// Draws a centred banner (a desync, the session's end).
pub(super) fn draw_banner(ctx: &ui::egui::Context, text: &str) {
    use ui::egui::{Align2, Area, Color32, Frame, Id, Margin};
    Area::new(Id::new("net_banner"))
        .interactable(false)
        .anchor(Align2::CENTER_CENTER, ui::egui::vec2(0.0, -80.0))
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(Margin::symmetric(18, 10))
                .show(ui, |ui| {
                    ui.label(ui::typography::egui_text(
                        ctx,
                        &ui::typography::ITEM,
                        ui::typography::tint::TITLE,
                        text,
                    ));
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
                NetAction::RoundOver(text) => {
                    println!("lan: {text}");
                    self.net_round_finished();
                    if let Some(n) = self.netplay.as_mut() {
                        n.lobby.say(text);
                    }
                }
                NetAction::Welcomed => self.note_recent_game(true),
                NetAction::LocalsChanged => {
                    if let (Some(n), Some((walk, _))) = (self.netplay.as_ref(), self.walk.as_mut())
                    {
                        if walk.local_slot_of(0) == n.peer.local_slot {
                            n.peer.sync_walk_locals(walk);
                        }
                    }
                    self.relay_lan_split();
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
        self.note_recent_game(false);
        // The guests' lobby follows the host in a battle too (its roster,
        // the decided round's ready check).
        self.host_publish();
        self.net_names_to_feed();
    }

    /// A running LAN round's local line-up changed (issue #194 B20): the split views are laid
    /// out again for the walk's locals, each player who stays keeping their seat, pad, input
    /// and feed; a player who left takes nobody else's cell. With no split left for several
    /// locals (no Players-screen party), the further ones are marked away so bots hold their
    /// slots rather than leaving soldiers standing on neutral pads.
    pub(super) fn relay_lan_split(&mut self) {
        let Some((walk, _)) = self.walk.as_mut() else {
            return;
        };
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        let lan_seats = super::split::lan_seats_of(&n.locals, self.split_party.as_ref(), walk);
        let same = self.split.as_ref().is_some_and(|s| {
            s.inputs.len() == lan_seats.len()
                && (0..lan_seats.len()).all(|k| s.seats.seat_number(k) == lan_seats[k])
        });
        if same {
            return;
        }
        let old = self.split.take();
        let party = match (old.as_ref(), self.split_party.as_ref()) {
            (Some(o), Some(p)) => Some(o.party_back(p)),
            (None, Some(p)) => Some(p.clone()),
            (Some(o), None) => Some(o.seats.party_owners()),
            (None, None) => None,
        };
        let viewpoint = self.profile.settings.viewpoint;
        let fresh = party
            .as_ref()
            .and_then(|p| super::split::seat_lan(walk, &lan_seats, viewpoint, p));
        // Each further player's feed by seat: the ones who stay keep theirs.
        let old_seats: Vec<usize> = old.as_ref().map_or_else(Vec::new, |o| {
            (0..o.inputs.len())
                .map(|k| o.seats.seat_number(k))
                .collect()
        });
        let mut feeds: Vec<Option<game::event_feed::EventFeed>> =
            std::mem::take(&mut self.split_feeds)
                .into_iter()
                .map(Some)
                .collect();
        self.split_feeds = (1..walk.local_count())
            .map(|k| {
                let seat = lan_seats.get(k).copied();
                old_seats
                    .iter()
                    .position(|s| Some(*s) == seat)
                    .filter(|&j| j > 0)
                    .and_then(|j| feeds.get_mut(j - 1).and_then(Option::take))
                    .unwrap_or_else(|| {
                        let mut f = game::event_feed::EventFeed::for_walk(
                            &self.assets_dir,
                            &self.split_source_game,
                            walk,
                        );
                        walk.with_local(k, |w| f.set_opening_medals(w));
                        f
                    })
            })
            .collect();
        match fresh {
            Some(mut f) => {
                // A newcomer's own view and FOV, as `install_seat_profiles` gives them at the
                // round's start (the ones who stay keep theirs below).
                for k in 1..f.inputs.len() {
                    let seat = f.seats.seat_number(k);
                    if let Some(p) = self.split_profiles.get(seat).and_then(Option::as_ref) {
                        f.inputs[k].set_viewpoint(p.settings.viewpoint);
                        let fov = game::walk::clamp_fov_scale(p.settings.fov_scale);
                        walk.with_local(k, |w| w.fov_scale = fov);
                    }
                }
                if let Some(o) = old {
                    f.carry_from(o);
                }
                println!("split: this machine's LAN players changed; views laid out again");
                self.split = Some(f);
            }
            None => {
                if walk.local_count() > 1 {
                    eprintln!(
                        "split: this machine's LAN players changed with no split; \
                         the further players are away"
                    );
                    for k in 1..walk.local_count() {
                        n.peer.set_local_away(&mut n.link, k, true);
                    }
                }
            }
        }
        self.name_tags
            .resize_with(walk.local_count().max(1), Default::default);
        self.feed_names();
    }

    /// The kill feed and the stats pages name every human by his session
    /// name, on every peer (issue #168). Presentation only: the sim never
    /// sees a name.
    pub(super) fn net_names_to_feed(&mut self) {
        if let Some(n) = self.netplay.as_ref() {
            let names = n.names().iter().map(|(slot, name)| (*slot, name.as_str()));
            self.names.set(names);
        }
        self.feed_names();
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
            n.lobby.roster_nav.release();
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
            return;
        }
        // Issue #194 B20: the further players play the round on their seats' profiles (their
        // view, pads and the round's record), as an offline split round lends them.
        self.lend_split_profiles();
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
                        use ui::typography::{egui_text, tint, BODY, HELP, TITLE};
                        ui.label(egui_text(ctx, &TITLE, tint::TITLE, &status));
                        if !guests.is_empty() {
                            let line = format!("Players here: {}", guests.join(", "));
                            ui.label(egui_text(ctx, &BODY, tint::UNSELECTED, &line));
                        }
                        ui.add_space(12.0);
                        ui.label(egui_text(ctx, &HELP, tint::HELP, "Back leaves."));
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

    fn host_on_free_port() -> NetplaySession {
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .and_then(|s| s.local_addr())
            .map(|a| a.port())
            .unwrap();
        let args = LanArgs::parse(&["--port".to_owned(), port.to_string()], true).unwrap();
        let mut profile = PlayerProfile::decode_or_default(&[]);
        profile.name = "Host".into();
        NetplaySession::host(args, profile, 4, true).unwrap()
    }

    /// Issue #194: the host's first player's Cheats page picks a target; a further player of the
    /// host's machine gets their own slot's cheats, with no target row, and a toggle there acts
    /// on their own row list.
    #[test]
    fn lan_a_further_players_cheats_page_is_their_own_slot() {
        let mut n = host_on_free_port();
        let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
        walk.adopt_local(2);
        let first = n.cheat_entries(&walk, walk.local_slot_of(0));
        assert_eq!(first[0].0, "lan_target");
        assert_eq!(n.cheat_rows[0], CheatRow::Target);
        let further = n.cheat_entries(&walk, 2);
        let all = core::sim::Cheats::default().entries().len();
        assert_eq!(further.len(), all, "every cheat, for their own slot");
        assert!(further
            .iter()
            .all(|e| e.0 != "lan_target" && e.0 != "lan_awards"));
        assert!(n.cheat_rows.iter().all(|r| matches!(r, CheatRow::Cheat(_))));
    }

    /// Issue #194 B20 review: a further player's Game Menu rows in a LAN round write their own
    /// seat's profile, never the first player's; their Respawn is left to their own view.
    #[test]
    fn lan_a_further_players_game_menu_rows_are_their_own() {
        use ui::menu::MenuAction;
        let mut n = host_on_free_port();
        let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
        walk.adopt_local(2);
        let mut host = game::profile::Profile::default();
        host.settings.pad_look_sensitivity = 1.0;
        host.settings.career_awards = true;
        let mut seat_profiles: [Option<game::profile::Profile>; 4] = Default::default();
        seat_profiles[1] = Some(host.clone());
        let seats = game::local_seats::Seats::<gilrs::GamepadId>::new(2, true);
        let (mut paused, mut menu) = (true, game::pause::PauseMenu::default());
        let mut session = ui::shell::SessionOptions::default();
        let mut act = |n: &mut NetplaySession,
                       host: &mut game::profile::Profile,
                       seat_profiles: &mut [Option<game::profile::Profile>; 4],
                       a: MenuAction,
                       opener: usize| {
            n.opener_pause_action(
                &walk,
                a,
                opener,
                &mut paused,
                &mut menu,
                host,
                seat_profiles,
                Some(&seats),
                &mut session,
            )
        };
        assert!(act(
            &mut n,
            &mut host,
            &mut seat_profiles,
            MenuAction::SetPadLookSensitivity(1.7),
            1
        ));
        assert!(act(
            &mut n,
            &mut host,
            &mut seat_profiles,
            MenuAction::SetCareerAwards(false),
            1
        ));
        assert!(act(
            &mut n,
            &mut host,
            &mut seat_profiles,
            MenuAction::SetAssist(0.25),
            1
        ));
        let p2 = seat_profiles[1].as_ref().unwrap();
        assert_eq!(
            p2.settings.pad_look_sensitivity,
            game::profile::clamp_pad_look(1.7)
        );
        assert!(!p2.settings.career_awards);
        assert_eq!(
            p2.settings.assist_intensity,
            game::profile::clamp_assist_intensity(0.25)
        );
        assert_eq!(
            host.settings.pad_look_sensitivity, 1.0,
            "P1's own is untouched"
        );
        assert!(host.settings.career_awards);
        // P1's own row is still P1's.
        assert!(act(
            &mut n,
            &mut host,
            &mut seat_profiles,
            MenuAction::SetPadLookSensitivity(1.3),
            0
        ));
        assert_eq!(
            host.settings.pad_look_sensitivity,
            game::profile::clamp_pad_look(1.3)
        );
        // A further player's Respawn goes to their own view, not the session's.
        assert!(!act(
            &mut n,
            &mut host,
            &mut seat_profiles,
            MenuAction::Respawn,
            1
        ));
        // The next round seats the further player on their profile as it is now.
        n.locals.push(LanLocal {
            seat: 1,
            name: "Han".into(),
            profile: PlayerProfile {
                name: "Han".into(),
                ..PlayerProfile::default()
            },
            slot: Some(2),
        });
        n.refresh_local_profiles(&seat_profiles);
        assert_eq!(n.locals[0].profile.name, "Han");
        assert!(!n.locals[0].profile.award_bonuses);
    }

    /// Second review of issue #168: a round the host abandons (its game
    /// parted twice) takes everyone from the battle to the lobby's results,
    /// as a decided round does: the host's ready check runs, the guest is
    /// ready by itself, and the playlist's end brings everyone back to the
    /// setup screen. Nobody is left frozen in the battle.
    #[test]
    fn lan_an_abandoned_round_takes_everyone_to_the_results() {
        use super::super::lobby::tests::{lobby_pair, run};
        let (mut host, mut guest) = lobby_pair();
        // Both in a battle when the host gives the round up.
        host.screen = Screen::Battle;
        guest.screen = Screen::Battle;
        {
            let n = host.netplay.as_mut().unwrap();
            n.peer.abandon_round(&mut n.link, "parted twice".into());
        }
        let start = Instant::now();
        while start.elapsed().as_secs_f32() < 5.0
            && !(matches!(host.screen, Screen::Lobby) && matches!(guest.screen, Screen::Lobby))
        {
            for a in [&mut *host, &mut *guest] {
                if matches!(a.screen, Screen::Battle) {
                    a.net_idle();
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        assert!(
            matches!(host.screen, Screen::Lobby),
            "the host stayed in the battle"
        );
        assert!(
            matches!(guest.screen, Screen::Lobby),
            "the guest stayed in the battle"
        );
        let h = host.netplay.as_mut().unwrap();
        assert!(matches!(h.lobby.phase, LobbyPhase::Results { .. }));
        assert!(h.lobby.ready.contains(&0), "the host left its results");
        for a in [&mut *host, &mut *guest] {
            let lines = a.netplay.as_mut().unwrap().lobby.lines();
            assert!(
                lines
                    .iter()
                    .any(|l| l.contains("could not be kept in sync")),
                "{lines:?}"
            );
        }
        // The guest is ready at the results; the ready check completes.
        run(&mut host, &mut guest, |h, _| {
            h.netplay.as_ref().unwrap().lobby.phase == LobbyPhase::Choosing
        });
        for mut a in [host, guest] {
            if let Some(n) = a.netplay.take() {
                n.shutdown();
            }
        }
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
