//! The multiplayer pages and the lobby (issue #168,
//! `analysis/plan/LAN_MULTIPLAYER.md` §3, §4.1-§4.3, §4.8; FEATURE_SET
//! "Host (P1) sessions", "Host-only session authority", "Post-match
//! results"):
//!
//! - **Host Game** (Main → Multiplayer → LAN: Host Game): max players, the
//!   port, the guests' cheat policy and the award policy
//!   ([`ui::lobby::HostSetup`]; lag compensation is always on); Start Hosting opens the session and the
//!   lobby.
//! - **Join Game**: the LAN browser (`Find` to the limited broadcast, every
//!   adapter's directed broadcast and, by unicast, every recent game's
//!   address, which is what finds a game over Tailscale or WireGuard), this
//!   computer's recent games (`game::netplay::recent`: each with its host's
//!   answer, or "not hosting now"; Square / Space / a right-click forgets
//!   one), Join by address (resolved off the frame loop) and Test
//!   Connection. A recent game is written when the host accepts the player.
//! - **The lobby** ([`Screen::Lobby`]): the host's own Instant Action
//!   screen with a roster panel (name, ping, addon, team, ready; Kick). Its
//!   map list and the addon heroes option are the intersection of every
//!   connected player's content ([`game::netplay::lobby::restrict`]); LAUNCH
//!   plays the playlist as networked rounds. Guests draw the same screen
//!   read-only from the host's [`LobbyState`] ("The host is choosing…") and
//!   can only leave; a round's load takes them to the loading screen.
//! - **Round end**: every peer sees its own team board and personal and
//!   career pages, then waits here ("Waiting for other players"); a guest
//!   sends `Ready`. The host continues the playlist once it and every
//!   connected guest are ready, or 60 s after the round was decided, and
//!   returns everyone to its setup screen after the last row.
//!
//! Rules:
//! - The lobby never touches a sim. The host's state goes out whole when
//!   it changes (the net core throttles it to 10 Hz), maps by key.
//! - A text box has the keyboard while focused: the menu keys wait.
//! - A session that ends shows its banner, then the Multiplayer page
//!   (Join Game for a guest, with why).

use super::front::FrontShell;
use super::lan::{LanArgs, NetLink, NetplaySession};
use super::{App, Camera, Screen};
use anyhow::Result;
use game::netplay::lobby::{self as lb, HostInfo, LobbyPhase, LobbyPlayer, LobbyState};
use game::netplay::{reach, ContentManifest, Link, PlayerProfile};
use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::mpsc;
use std::time::Instant;
use ui::shell::{InstantAction, Launch, MapEntry, SessionOptions};
use ui::shell_feel::MenuSound;

/// How long a lobby line stays in the roster panel.
const LINE_SECS: f32 = 12.0;

/// The lobby's state on either side (module header).
pub(super) struct Lobby {
    /// The setup screen: the host's own, or the guest's read-only copy.
    pub ia: Option<InstantAction>,
    /// This install's whole catalog (the restriction's and the mirror's
    /// source).
    full: Vec<MapEntry>,
    /// This install's content.
    pub content: ContentManifest,
    /// The intersection last applied (the host).
    common: Option<ContentManifest>,
    /// The host's latest state (a guest), and whether it is drawn yet.
    state: Option<LobbyState>,
    state_new: bool,
    missing_said: bool,
    /// What the host last sent.
    sent: Vec<u8>,
    info_sent: Vec<u8>,
    pub phase: LobbyPhase,
    /// The host's playlist still to play, its options and rows played.
    playlist: VecDeque<Launch>,
    session: Option<SessionOptions>,
    played: u32,
    pub ready: BTreeSet<u8>,
    pub results_at: Option<Instant>,
    lines: VecDeque<(String, Instant)>,
    /// The static digest groups of the round this peer last built.
    pub groups: Vec<(&'static str, u64)>,
    /// The round in play (key, label), the host's.
    pub round_map: Option<(String, String)>,
    /// The round in play's mode ("Conquest"), the host's discovery info.
    pub round_mode: String,
    /// This hosting session's random number (`HostInfo::session`).
    pub host_session: u64,
    /// The round a guest sent its `Ready` for.
    ready_sent: Option<u32>,
    pub max_players: u8,
    pub lag_comp: bool,
    /// The host's award policy (LAN §5b): the next round's, and the round
    /// in play's since the pause menu last switched it.
    pub awards: bool,
    /// The pointer was over a panel of the lobby's own last frame.
    over_panel: bool,
    /// The roster panel's pad and key state (issue #168): open or closed,
    /// and the row the pad is on while the list holds the input.
    pub roster_nav: ui::lobby::RosterNav,
}

impl Default for Lobby {
    fn default() -> Self {
        Self {
            ia: None,
            full: Vec::new(),
            content: ContentManifest::default(),
            common: None,
            state: None,
            state_new: false,
            missing_said: false,
            sent: Vec::new(),
            info_sent: Vec::new(),
            phase: LobbyPhase::Choosing,
            playlist: VecDeque::new(),
            session: None,
            played: 0,
            ready: BTreeSet::new(),
            results_at: None,
            lines: VecDeque::new(),
            groups: Vec::new(),
            round_map: None,
            round_mode: String::new(),
            host_session: net::fresh_u64(),
            ready_sent: None,
            max_players: ui::lobby::DEFAULT_MAX_PLAYERS,
            lag_comp: true,
            awards: true,
            over_panel: false,
            roster_nav: Default::default(),
        }
    }
}

impl Lobby {
    /// A lobby line (the roster panel and the console).
    pub fn say(&mut self, line: String) {
        println!("lan lobby: {line}");
        self.lines.push_back((line, Instant::now()));
        while self.lines.len() > 6 {
            self.lines.pop_front();
        }
    }

    /// The host's lobby state arrived (a guest).
    pub fn receive(&mut self, bytes: &[u8]) {
        if let Some(s) = LobbyState::decode(bytes) {
            if let Some(m) = s.message.clone() {
                if self.state.as_ref().and_then(|o| o.message.as_ref()) != Some(&m) {
                    self.say(m);
                }
            }
            self.state = Some(s);
            self.state_new = true;
        }
    }

    pub(super) fn lines(&mut self) -> Vec<String> {
        self.lines
            .retain(|(_, at)| at.elapsed().as_secs_f32() < LINE_SECS);
        self.lines.iter().map(|(l, _)| l.clone()).collect()
    }
}

/// Multiplayer's Host Game or Join Game page.
pub(super) enum MpPage {
    Host(Box<ui::lobby::HostSetup>),
    Join(Box<JoinScreen>),
}

/// Join Game: the page, the LAN browser's socket and an address lookup.
pub(super) struct JoinScreen {
    page: ui::lobby::JoinPage,
    browser: Option<(net::LanBrowser, net::UdpTransport)>,
    clock: net::RealClock,
    resolving: Option<(String, mpsc::Receiver<std::io::Result<SocketAddr>>)>,
    /// The latest "Test connection" (LAN §5b).
    test: Option<TestRun>,
    /// The broadcast targets and each recent game's IPv4 address (by
    /// index of the page's recent games), as last looked up.
    broadcasts: Vec<SocketAddr>,
    recent_addrs: Vec<Option<SocketAddr>>,
    /// The lookup in flight: the addresses asked, and the answer.
    lookup: Option<(Vec<String>, mpsc::Receiver<TargetLookup>)>,
    looked_up: Option<Instant>,
}

/// How often Join Game looks its targets up again (a VPN coming up, an
/// adapter added, a name that now resolves).
const LOOKUP_EVERY_SECS: f32 = 30.0;

/// One lookup of the browser's targets: the broadcasts, then each recent
/// game's IPv4 address.
type TargetLookup = (Vec<SocketAddr>, Vec<Option<SocketAddr>>);

/// A recent game's address as the browser can ask it: IPv4 only (the
/// browser's socket is IPv4); a name that does not resolve is `None`.
fn resolve_v4(addr: &str) -> Option<SocketAddr> {
    match net::transport::resolve(addr, net::DEFAULT_PORT) {
        Ok(a @ SocketAddr::V4(_)) => Some(a),
        _ => None,
    }
}

/// The browser's targets: the broadcasts, then the recent games'
/// addresses, once each.
pub(super) fn join_targets(
    broadcasts: &[SocketAddr],
    recent: &[Option<SocketAddr>],
) -> Vec<SocketAddr> {
    let mut v: Vec<SocketAddr> = Vec::new();
    for a in broadcasts.iter().chain(recent.iter().flatten()) {
        if !v.contains(a) {
            v.push(*a);
        }
    }
    v
}

/// A guest's entry in the recent games: the host's name from the row the
/// player pressed (until the roster names him), whether the host accepted
/// (the entry is written), and the mode of the round last loaded.
#[derive(Clone, Debug, Default)]
pub(super) struct RecentNote {
    pub name_hint: String,
    pub recorded: bool,
    pub mode: String,
}

/// A "Test connection" of a typed address: its lookup, then the test.
pub(super) enum TestRun {
    Resolving(String, mpsc::Receiver<std::io::Result<SocketAddr>>),
    Running(String, Box<reach::ConnectionTest>, net::UdpTransport),
    /// Decided, or failed before asking: the line to keep showing.
    Said(String),
}

impl TestRun {
    fn start(text: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let lookup = text.clone();
        // DNS can take seconds: off the frame loop.
        std::thread::spawn(move || {
            let _ = tx.send(net::transport::resolve(&lookup, net::DEFAULT_PORT));
        });
        TestRun::Resolving(text, rx)
    }

    /// One frame: the lookup's answer, the test's datagrams.
    fn pump(&mut self, now: net::Micros, build: &str) {
        let next = match self {
            TestRun::Resolving(text, rx) => match rx.try_recv() {
                Ok(Ok(addr)) => match net::UdpTransport::client() {
                    Ok(t) => Some(TestRun::Running(
                        std::mem::take(text),
                        Box::new(reach::ConnectionTest::new(addr)),
                        t,
                    )),
                    Err(e) => Some(TestRun::Said(format!("Cannot open a UDP port: {e}"))),
                },
                Ok(Err(e)) => Some(TestRun::Said(format!("Cannot find {text}: {e}"))),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(TestRun::Said(format!("Cannot find {text}: lookup failed")))
                }
            },
            TestRun::Running(text, test, t) => {
                let state = test.pump(now, t, build);
                (*state != reach::TestState::Testing)
                    .then(|| TestRun::Said(reach::test_text(text, state)))
            }
            TestRun::Said(_) => None,
        };
        if let Some(n) = next {
            *self = n;
        }
    }

    /// The Join page's line.
    fn line(&self) -> String {
        match self {
            TestRun::Resolving(text, _) => format!("Looking up {text}\u{2026}"),
            TestRun::Running(text, test, _) => reach::test_text(text, test.state()),
            TestRun::Said(line) => line.clone(),
        }
    }
}

/// The Host page's endpoints (LAN §5b, #181), looked
/// up again every few seconds (two UDP connects, nothing sent).
fn host_addresses(port: Option<u16>) -> Vec<String> {
    use std::sync::Mutex;
    type Found = Option<(Instant, Vec<(std::net::Ipv4Addr, bool)>)>;
    static ADDRS: Mutex<Found> = Mutex::new(None);
    let mut cache = ADDRS.lock().unwrap_or_else(|e| e.into_inner());
    if cache
        .as_ref()
        .is_none_or(|(at, _)| at.elapsed().as_secs_f32() > 5.0)
    {
        *cache = Some((Instant::now(), reach::local_ipv4s()));
    }
    let addrs = cache.as_ref().map(|c| c.1.clone()).unwrap_or_default();
    reach::host_addresses(&addrs, port)
}

/// The broadcast targets: the limited broadcast, and each IPv4 adapter's
/// directed broadcast (`reach::directed_broadcasts`), on the default port.
pub(super) fn browse_targets() -> Vec<SocketAddr> {
    let mut v = net::LanBrowser::default_targets();
    for b in reach::directed_broadcasts(&reach::ipv4_interfaces()) {
        v.push(SocketAddr::from((b, net::DEFAULT_PORT)));
    }
    v
}

impl MpPage {
    pub fn host() -> Self {
        let cheats: Vec<(&'static str, &'static str)> = core::sim::Cheats::default()
            .entries()
            .iter()
            .map(|e| (e.0, e.1))
            .collect();
        MpPage::Host(Box::new(ui::lobby::HostSetup::new(
            &cheats,
            net::DEFAULT_PORT,
        )))
    }

    /// Join Game with this computer's recent games. The browser starts on
    /// the limited broadcast; the adapters' broadcasts and the recent
    /// games' addresses join it when their lookup (off the frame loop)
    /// answers.
    pub fn join(recent: Vec<ui::lobby::RecentRow>) -> Self {
        let browser = net::UdpTransport::client()
            .map_err(|e| eprintln!("lan browser: {e}"))
            .ok()
            .map(|t| (net::LanBrowser::new(Vec::new()), t));
        let n = recent.len();
        MpPage::Join(Box::new(JoinScreen {
            page: ui::lobby::JoinPage::new(recent),
            browser,
            clock: net::RealClock::new(),
            resolving: None,
            test: None,
            broadcasts: net::LanBrowser::default_targets(),
            recent_addrs: vec![None; n],
            lookup: None,
            looked_up: None,
        }))
    }
}

impl JoinScreen {
    /// Starts a lookup of the targets when one is due; takes its answer.
    fn pump_targets(&mut self) {
        let addrs: Vec<String> = self.page.recent.iter().map(|r| r.addr.clone()).collect();
        if let Some((asked, rx)) = &self.lookup {
            match rx.try_recv() {
                Ok((broadcasts, resolved)) => {
                    // A forget while it ran: ask again.
                    if *asked == addrs {
                        self.broadcasts = broadcasts;
                        self.recent_addrs = resolved;
                    } else {
                        self.looked_up = None;
                    }
                    self.lookup = None;
                    self.retarget();
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => self.lookup = None,
            }
        }
        let due = self
            .looked_up
            .is_none_or(|t| t.elapsed().as_secs_f32() >= LOOKUP_EVERY_SECS);
        if self.lookup.is_none() && due {
            self.looked_up = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            let asked = addrs.clone();
            // DNS (a MagicDNS name) can take seconds: off the frame loop.
            std::thread::spawn(move || {
                let resolved = asked.iter().map(|a| resolve_v4(a)).collect();
                let _ = tx.send((browse_targets(), resolved));
            });
            self.lookup = Some((addrs, rx));
        }
    }

    fn retarget(&mut self) {
        if let Some((b, _)) = self.browser.as_mut() {
            b.set_targets(join_targets(&self.broadcasts, &self.recent_addrs));
        }
    }

    /// Recent game `i` was forgotten: its row and address go.
    fn forgot(&mut self, i: usize, rows: Vec<ui::lobby::RecentRow>) {
        if i < self.recent_addrs.len() {
            self.recent_addrs.remove(i);
        }
        self.recent_addrs.resize(rows.len(), None);
        self.page.set_recent(rows);
        self.retarget();
    }

    /// The hosts heard, split between the recent games and the rest.
    fn hosts_heard(&mut self, build: &str) {
        let Some((b, _)) = self.browser.as_ref() else {
            return;
        };
        let heard = b
            .hosts()
            .into_iter()
            .map(|h| {
                let info = HostInfo::of_found(&h);
                game::netplay::recent::Heard {
                    addr: h.addr,
                    session: info.session,
                    row: ui::lobby::FoundRow {
                        addr: h.addr.to_string(),
                        refusal: info.refusal(build),
                        name: if info.name.is_empty() {
                            h.addr.ip().to_string()
                        } else {
                            info.name
                        },
                        map: info.map,
                        mode: info.mode,
                        players: format!("{}/{}", info.players, info.max_players),
                        ping_ms: h.rtt.as_millis_f64().round() as u32,
                    },
                }
            })
            .collect();
        let (hosts, live) = game::netplay::recent::merge(heard, &self.recent_addrs);
        self.page.set_hosts(hosts, live);
    }

    /// The host name a Connect of `text` is pressing, if a row shows one.
    fn name_for(&self, text: &str) -> String {
        let p = &self.page;
        p.hosts
            .iter()
            .chain(p.recent.iter().filter_map(|r| r.live.as_ref()))
            .find(|h| h.addr == text)
            .map(|h| h.name.clone())
            .or_else(|| {
                p.recent
                    .iter()
                    .find(|r| r.addr == text)
                    .map(|r| r.live.as_ref().map_or(r.name.clone(), |h| h.name.clone()))
            })
            .unwrap_or_default()
    }
}

/// Draws a Multiplayer page.
pub(super) fn draw_mp(ctx: &ui::egui::Context, page: &mut MpPage) -> ui::lobby::PageDraw {
    use ui::egui::Color32;
    let help = (
        "Up / Down choose, Left / Right change, Enter selects, Escape goes back".to_owned(),
        Color32::GRAY,
    );
    match page {
        MpPage::Host(h) => {
            let rows = h.rows();
            let mut lines = Vec::new();
            if let Some(m) = &h.message {
                lines.push((m.clone(), Color32::from_rgb(255, 160, 120)));
            }
            for l in host_addresses(h.parsed_port()) {
                lines.push((l, Color32::from_rgb(170, 210, 255)));
            }
            if cfg!(target_os = "macos") {
                lines.push((
                    "macOS: allow incoming connections when asked; on macOS 15 also allow \
                     Local Network for openbf (System Settings \u{2192} Privacy & Security)"
                        .to_owned(),
                    Color32::GRAY,
                ));
            }
            let cursor = h.cursor;
            let focus = std::mem::take(&mut h.focus_text);
            ui::lobby::draw_host_page(ctx, &rows, cursor, h.text_mut(), focus, &lines)
        }
        MpPage::Join(j) => {
            let rows = j.page.rows();
            let mut lines = Vec::new();
            let live = j.page.recent.iter().any(|r| r.live.is_some());
            if j.page.hosts.is_empty() && !live {
                lines.push((
                    "Looking for games on this network\u{2026}".to_owned(),
                    Color32::LIGHT_GRAY,
                ));
            }
            lines.push((
                "Over Tailscale or WireGuard, games you joined before appear under Recent \
                 games; a new one needs its address once."
                    .to_owned(),
                Color32::GRAY,
            ));
            if !j.page.recent.is_empty() {
                lines.push((
                    "Square / Space / right-click forgets the selected recent game".to_owned(),
                    Color32::GRAY,
                ));
            }
            if let Some(m) = &j.page.status {
                lines.push((m.clone(), Color32::from_rgb(255, 210, 120)));
            }
            if let Some(t) = &j.test {
                lines.push((t.line(), Color32::from_rgb(170, 210, 255)));
            }
            lines.push(help);
            let cursor = j.page.cursor;
            let focus = std::mem::take(&mut j.page.focus_text);
            let text = j.page.text_mut();
            ui::lobby::draw_page(ctx, "LAN: Join Game", &rows, cursor, text, focus, &lines)
        }
    }
}

/// What a Multiplayer page asks of the App.
pub(super) enum MpDone {
    Host(ui::lobby::HostCommand),
    Join(ui::lobby::JoinCommand),
    /// An address lookup finished.
    Resolved(String, std::io::Result<SocketAddr>),
}

impl App {
    /// The shell frame's Multiplayer page: the browser's replies, a lookup
    /// that finished, the presses.
    pub(super) fn step_mp(
        &mut self,
        front: &mut FrontShell,
        input: ui::menu::MenuInput,
        forget: bool,
        _dt: f32,
    ) -> Option<MpDone> {
        let build = game::netplay::build_id();
        let (done, clicks) = match front.mp.as_mut()? {
            MpPage::Host(h) => {
                let d = h.step(input).map(MpDone::Host);
                (d, h.take_clicks())
            }
            MpPage::Join(j) => {
                j.pump_targets();
                if let Some((b, t)) = j.browser.as_mut() {
                    use net::Clock;
                    b.pump(j.clock.now(), t);
                }
                j.hosts_heard(&build);
                if let Some(t) = j.test.as_mut() {
                    use net::Clock;
                    t.pump(j.clock.now(), &build);
                }
                let resolved = j
                    .resolving
                    .as_ref()
                    .and_then(|(text, rx)| match rx.try_recv() {
                        Ok(r) => Some(MpDone::Resolved(text.clone(), r)),
                        Err(mpsc::TryRecvError::Empty) => None,
                        Err(mpsc::TryRecvError::Disconnected) => Some(MpDone::Resolved(
                            text.clone(),
                            Err(std::io::Error::other("lookup failed")),
                        )),
                    });
                if resolved.is_some() {
                    j.resolving = None;
                }
                let d = resolved.or_else(|| j.page.step(input, forget).map(MpDone::Join));
                (d, j.page.take_clicks())
            }
        };
        for c in clicks {
            front.menus.cues.play(c);
        }
        done
    }

    /// A Multiplayer page's click or Enter in a text box: focus the row and
    /// press it.
    pub(super) fn after_mp_draw(
        &mut self,
        front: &mut FrontShell,
        d: ui::lobby::PageDraw,
    ) -> Option<MpDone> {
        if let (Some(i), Some(MpPage::Join(j))) = (d.secondary, front.mp.as_mut()) {
            // A right-click on a recent game forgets it.
            if matches!(j.page.row_ids().get(i), Some(ui::lobby::JoinRow::Recent(_))) {
                j.page.cursor = i;
                let forget = j.page.step(Default::default(), true).map(MpDone::Join);
                for c in j.page.take_clicks() {
                    front.menus.cues.play(c);
                }
                return forget;
            }
        }
        if d.clicked.is_none() && !d.submitted {
            return None;
        }
        let press = ui::menu::MenuInput {
            confirm: true,
            ..Default::default()
        };
        let (done, clicks) = match front.mp.as_mut()? {
            MpPage::Host(h) => {
                if let Some(i) = d.clicked {
                    h.cursor = i;
                    h.focus_text = h.row_ids().get(i) == Some(&ui::lobby::HostRow::Port);
                }
                (h.step(press).map(MpDone::Host), h.take_clicks())
            }
            MpPage::Join(j) => {
                if let Some(i) = d.clicked {
                    j.page.cursor = i;
                }
                (
                    j.page.step(press, false).map(MpDone::Join),
                    j.page.take_clicks(),
                )
            }
        };
        for c in clicks {
            front.menus.cues.play(c);
        }
        done
    }

    /// What a Multiplayer page asked for (after the shell frame).
    pub(super) fn mp_done(&mut self, done: MpDone) {
        use ui::lobby::{HostCommand, JoinCommand};
        match done {
            MpDone::Host(HostCommand::Back) | MpDone::Join(JoinCommand::Back) => {
                if let Some(f) = self.front.as_mut() {
                    f.mp = None;
                    f.text_focus = false;
                }
            }
            MpDone::Host(HostCommand::Start {
                max_players,
                port,
                cheat_policy,
                lag_comp,
                awards,
            }) => {
                let opened = self.host_from_shell(max_players, port, cheat_policy, lag_comp);
                if let Some(n) = self.netplay.as_mut().filter(|_| opened.is_ok()) {
                    n.lobby.awards = awards;
                }
                match opened {
                    Ok(()) => self.enter_lobby(),
                    Err(e) => {
                        if let Some(MpPage::Host(h)) =
                            self.front.as_mut().and_then(|f| f.mp.as_mut())
                        {
                            h.message = Some(format!("Cannot host: {e:#}"));
                        }
                    }
                }
            }
            MpDone::Join(JoinCommand::Forget(i)) => {
                let recent = self.recent_games();
                if recent.forget(i) {
                    recent.save();
                }
                let rows = recent.rows(game::netplay::recent::now_unix());
                if let Some(MpPage::Join(j)) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
                    j.forgot(i, rows);
                }
            }
            MpDone::Join(JoinCommand::Connect(text)) => {
                if let Some(MpPage::Join(j)) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
                    self.join_name_hint = j.name_for(&text);
                    let (tx, rx) = mpsc::channel();
                    let lookup = text.clone();
                    // DNS can take seconds: off the frame loop.
                    std::thread::spawn(move || {
                        let _ = tx.send(net::transport::resolve(&lookup, net::DEFAULT_PORT));
                    });
                    j.page.status = Some(format!("Looking up {text}\u{2026}"));
                    j.resolving = Some((text, rx));
                }
            }
            MpDone::Join(JoinCommand::Test(text)) => {
                if let Some(MpPage::Join(j)) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
                    j.test = Some(TestRun::start(text));
                }
            }
            MpDone::Resolved(text, Err(e)) => {
                if let Some(MpPage::Join(j)) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
                    j.page.status = Some(format!("Cannot find {text}: {e}"));
                }
            }
            MpDone::Resolved(text, Ok(addr)) => match self.join_from_shell(&text, addr) {
                Ok(()) => self.enter_lobby(),
                Err(e) => {
                    if let Some(MpPage::Join(j)) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
                        j.page.status = Some(format!("Cannot join {text}: {e:#}"));
                    }
                }
            },
        }
    }

    fn lan_args(&self, host: bool) -> LanArgs {
        let mut a = LanArgs::parse(&[], true).expect("defaults parse");
        a.host = host;
        a.assets = self.assets_dir.clone();
        a.missions = self.missions_dir.clone();
        a
    }

    fn lan_profile(&self) -> PlayerProfile {
        let mut p = PlayerProfile::from_profile(&self.profile);
        p.name = lb::player_name(&self.profile.name);
        p
    }

    /// The Players screen's further seats as this machine's LAN players
    /// (issue #194 B19), when the Multiplayer page was opened from the
    /// Splitscreen mode list (`App.split_party`): each seated seat 1-3 with
    /// its Ready (or lent) profile, seat order. Empty otherwise.
    fn lan_locals(&self) -> Vec<super::lan::LanLocal> {
        use game::local_seats::SeatProfile;
        let (Some(owners), Some(front)) = (self.split_party.as_ref(), self.front.as_ref()) else {
            return Vec::new();
        };
        (1..input::owners::MAX_LOCAL_PLAYERS)
            .filter(|&seat| owners.is_seated(seat))
            .filter_map(|seat| {
                let p = match front.party.profile(seat) {
                    SeatProfile::Ready(p) => Some(p),
                    SeatProfile::Lent(_) => self.split_profiles[seat].as_ref(),
                    _ => None,
                }?;
                let mut profile = PlayerProfile::from_profile(p);
                profile.name = lb::player_name(&p.name);
                Some(super::lan::LanLocal {
                    seat: seat as u8,
                    name: profile.name.clone(),
                    profile,
                    slot: None,
                })
            })
            .collect()
    }

    /// Host Game's Start Hosting: the session is open, the lobby next.
    fn host_from_shell(
        &mut self,
        max_players: u8,
        port: u16,
        cheat_policy: u32,
        lag_comp: bool,
    ) -> Result<()> {
        let mut args = self.lan_args(true);
        args.port = port;
        args.cheats = cheat_policy;
        let mut n = NetplaySession::host(args, self.lan_profile(), max_players, lag_comp)?;
        n.peer.cheat_policy = cheat_policy;
        n.add_host_locals(self.lan_locals());
        println!("lan: {}", n.status);
        self.netplay = Some(n);
        Ok(())
    }

    /// Join Game's Connect: the session is open (the lobby says when the
    /// host answers); the address joins the recent games once the host
    /// accepts ([`App::note_recent_game`]).
    fn join_from_shell(&mut self, text: &str, addr: SocketAddr) -> Result<()> {
        let mut args = self.lan_args(false);
        args.addr = Some(text.to_owned());
        let mut n = NetplaySession::join_locals(args, self.lan_profile(), addr, self.lan_locals())?;
        n.status = format!("Connecting to {text}\u{2026}");
        n.recent.name_hint = std::mem::take(&mut self.join_name_hint);
        println!("lan: {}", n.status);
        self.netplay = Some(n);
        Ok(())
    }

    /// This computer's recent games, read (and seeded once from the active
    /// profile's old list) on first use; kept nowhere in tests.
    pub(super) fn recent_games(&mut self) -> &mut game::netplay::recent::RecentGames {
        let legacy = &self.profile.settings.recent_hosts;
        self.recent_games.get_or_insert_with(|| {
            if cfg!(test) {
                game::netplay::recent::RecentGames::in_memory()
            } else {
                let path = game::profile::data_dir().join(game::netplay::recent::FILE_NAME);
                game::netplay::recent::RecentGames::load_or_import(&path, legacy)
            }
        })
    }

    /// Join Game's rows of the recent games.
    pub(super) fn recent_rows(&mut self) -> Vec<ui::lobby::RecentRow> {
        self.recent_games().rows(game::netplay::recent::now_unix())
    }

    /// A guest's recent game (module header): written when the host
    /// accepts (`welcomed`), then kept up to date with the host's name and
    /// the round last played; the file is written only on a change.
    pub(super) fn note_recent_game(&mut self, welcomed: bool) {
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        if n.is_host() || !(welcomed || n.recent.recorded) {
            return;
        }
        let Some(addr) = n.args.addr.clone() else {
            return;
        };
        n.recent.recorded = true;
        let name = n
            .names()
            .get(&0)
            .cloned()
            .unwrap_or_else(|| n.recent.name_hint.clone());
        let map = match n.lobby.state.as_ref().map(|s| &s.phase) {
            Some(LobbyPhase::InRound(label)) => label.clone(),
            _ => String::new(),
        };
        let mode = if map.is_empty() {
            String::new()
        } else {
            n.recent.mode.clone()
        };
        let recent = self.recent_games();
        let mut changed = false;
        if welcomed {
            recent.remember(&addr, &name, game::netplay::recent::now_unix());
            changed = true;
        }
        changed |= recent.update(&addr, &name, &map, &mode);
        if changed {
            recent.save();
        }
    }

    /// The shell hands over to the lobby screen.
    fn enter_lobby(&mut self) {
        if let Some(f) = self.front.as_mut() {
            f.mp = None;
            f.text_focus = false;
        }
        if let Some(n) = self.netplay.as_mut() {
            n.lobby.roster_nav.release();
        }
        self.screen = Screen::Lobby;
        self.sync_grab();
    }

    /// Out of the round in play (or its loading) into the lobby, the
    /// session kept; `line` says why.
    pub(super) fn net_to_lobby(&mut self, line: Option<String>) {
        if self.loading.is_some() {
            self.finish_loading();
        }
        pak::set_overlay_scope(None);
        if self.walk.is_some() {
            let n = self.netplay.take();
            self.leave_battle();
            self.netplay = n;
        }
        if let Some(n) = self.netplay.as_mut() {
            n.peer.leave_round();
            n.loading = None;
            n.view.reset();
            // The player list held the pad when the round began; the lobby
            // comes back with its input free.
            n.lobby.roster_nav.release();
            if let Some(l) = line {
                n.lobby.say(l);
            }
        }
        self.screen = Screen::Lobby;
        self.sync_grab();
    }

    /// The round's end screens were left (LAN §4.8): to the lobby, ready.
    pub(super) fn net_round_finished(&mut self) {
        self.net_to_lobby(None);
        if let Some(n) = self.netplay.as_mut() {
            if n.is_host() {
                if !matches!(n.lobby.phase, LobbyPhase::Results { .. }) {
                    n.round_decided();
                }
                // The host's machine (all its own players) is ready.
                n.lobby.ready.insert(n.peer.machine_of(0));
            }
        }
    }

    /// The session is over: back to the Multiplayer page, the guest's on
    /// Join Game with why.
    fn leave_lobby(&mut self, why: Option<String>) {
        let host = self.netplay.as_ref().is_some_and(|n| n.is_host());
        if let Some(n) = self.netplay.take() {
            n.shutdown();
        }
        self.init_shell();
        let recent = self.recent_rows();
        let front = self
            .front
            .get_or_insert_with(|| FrontShell::new(&self.assets_dir));
        front.menus.page = ui::shell_ps2::Page::Multiplayer;
        front.menus.on_page = 0.0;
        front.text_focus = false;
        front.mp = Some(if host {
            let mut p = MpPage::host();
            if let MpPage::Host(h) = &mut p {
                h.message = why;
            }
            p
        } else {
            let mut p = MpPage::join(recent);
            if let MpPage::Join(j) = &mut p {
                j.page.status = why;
            }
            p
        });
    }

    /// The lobby screen's frame (module header).
    pub(super) fn redraw_lobby(&mut self, dt: f32) -> Result<()> {
        self.net_idle();
        if !matches!(self.screen, Screen::Lobby) {
            return Ok(());
        }
        let Some(n) = self.netplay.as_mut() else {
            self.init_shell();
            return Ok(());
        };
        if n.ended_done() {
            let why = n.ended.as_ref().map(|(t, _)| t.clone());
            self.leave_lobby(why);
            return Ok(());
        }
        let mut front = self
            .front
            .take()
            .unwrap_or_else(|| FrontShell::new(&self.assets_dir));
        if front.view.is_none() {
            if let Some(r) = &self.renderer {
                front.view = game::hud_feed::HudView::for_renderer(&self.assets_dir, r)
                    .map_err(|e| eprintln!("lobby: no BF2 HUD bake ({e:#})"))
                    .ok();
            }
        }
        let mut edges = self.menu_edges();
        edges |= front.repeat.step(self.prev_menu_buttons & 15, dt);
        let mut input = ui::menu::MenuInput {
            up: edges & 1 != 0,
            down: edges & 2 != 0,
            left: edges & 4 != 0,
            right: edges & 8 != 0,
            confirm: edges & 16 != 0,
            back: edges & 32 != 0,
        };
        // The roster panel first (issue #168): Square / X / Space moves the
        // pad into it; while it holds the input the screen under it sees
        // none, and Back closes the list instead of leaving the lobby.
        if let Some(n) = self.netplay.as_mut() {
            let host = n.is_host();
            let rows = roster_rows(n);
            let toggle = edges & game::kbm::MENU_SQUARE != 0;
            let was = n.lobby.roster_nav;
            let kick = n.lobby.roster_nav.input(toggle, input, &rows, host);
            if toggle || was.active() {
                if n.lobby.roster_nav != was {
                    front.menus.cues.play(MenuSound::Select);
                }
                input = ui::menu::MenuInput::default();
                edges = 0;
            }
            if let Some(slot) = kick {
                front.menus.cues.play(MenuSound::Accept);
                lobby_kick(n, slot);
            }
        }
        front.menus.tick(dt);
        let wheel = self.shell_pointer.take_wheel();
        let mut pointer = self.shell_pointer.take_frame();
        let n = self.netplay.as_mut().expect("session");
        if n.lobby.over_panel {
            pointer = pointer.map(|(p, _)| (p, false));
        }
        let click = pointer.is_some_and(|(_, c)| c);
        // The setup screen, built at the lobby's first frame.
        self.ensure_lobby_ia();
        let n = self.netplay.as_mut().expect("session");
        let host = n.is_host();
        let mut leave = false;
        let mut launch_next = false;
        if host {
            self.host_lobby_restrict();
            let n = self.netplay.as_mut().expect("session");
            let choosing = n.lobby.phase == LobbyPhase::Choosing;
            let mut ia = n.lobby.ia.take().expect("setup screen");
            let command = if choosing {
                self.step_instant(
                    front.view.as_ref(),
                    &mut ia,
                    input,
                    edges,
                    wheel,
                    pointer,
                    click,
                    dt,
                )
            } else {
                None
            };
            for s in ia.take_clicks() {
                front.menus.cues.play(s);
            }
            let n = self.netplay.as_mut().expect("session");
            let mut prefs = None;
            match command {
                Some(ui::shell::IaCommand::Back) => leave = true,
                Some(ui::shell::IaCommand::Launch(rows)) if !rows.is_empty() => {
                    n.lobby.playlist = rows.into();
                    n.lobby.session = Some(ia.options);
                    n.lobby.played = 0;
                    launch_next = true;
                }
                Some(ui::shell::IaCommand::Prefs) => prefs = Some(ia.options),
                _ => {}
            }
            n.lobby.ia = Some(ia);
            if let Some(options) = prefs {
                self.keep_session_prefs(&options);
            }
            let n = self.netplay.as_mut().expect("session");
            match n.lobby.phase.clone() {
                LobbyPhase::Results { .. } => {
                    // Ready is a machine's (B19): one answer for all its players.
                    let ready = &n.lobby.ready;
                    let all = n.peer.guests().values().all(|g| ready.contains(&g.machine));
                    let waited = n
                        .lobby
                        .results_at
                        .is_some_and(|t| t.elapsed().as_secs_f32() >= lb::READY_WAIT_SECS);
                    n.lobby.phase = LobbyPhase::Results {
                        ready: ready.iter().copied().collect(),
                    };
                    if ready.contains(&0) && (all || waited) {
                        launch_next = true;
                    }
                }
                // The host is here, so no round is in play (a load failed).
                LobbyPhase::InRound(_) => n.lobby.phase = LobbyPhase::Choosing,
                LobbyPhase::Choosing => {}
            }
            if input.back && !choosing {
                leave = true;
            }
        } else {
            // A guest: the host's screen, read-only; only Back (leave) and
            // the player list (taken above).
            let lobby = &mut n.lobby;
            if lobby.state_new {
                lobby.state_new = false;
                if let (Some(s), Some(ia)) = (lobby.state.as_ref(), lobby.ia.as_mut()) {
                    let missing = s.apply_to(ia, &lobby.full);
                    if !missing.is_empty() && !lobby.missing_said {
                        lobby.missing_said = true;
                        let line = format!(
                            "Not installed here: {} (the host lists only maps every player has)",
                            missing.join(", ")
                        );
                        lobby.say(line);
                    }
                }
            }
            // Waiting at the results (or benched): this guest is ready.
            let results = matches!(
                lobby.state.as_ref().map(|s| &s.phase),
                Some(LobbyPhase::Results { .. })
            );
            let round = n.peer.round_id();
            if results && n.lobby.ready_sent != Some(round) {
                n.lobby.ready_sent = Some(round);
                if let NetLink::Client(s) = &n.link {
                    s.send(net::ClientCommand::Ready {
                        round_id: round,
                        profile: n.current_profile(&self.profile).encode(),
                    });
                }
                n.ready_locals(round);
            }
            if input.back {
                front.menus.cues.play(MenuSound::Exit);
                leave = true;
            }
        }
        if leave {
            self.front = Some(front);
            self.leave_lobby(None);
            return Ok(());
        }
        if launch_next {
            self.front = Some(front);
            self.host_next_round();
            if !matches!(self.screen, Screen::Lobby) {
                return Ok(());
            }
            front = self.front.take().expect("front");
        }
        self.host_publish();
        self.draw_lobby(&mut front, dt)?;
        front.play_cues(self.audio.as_mut(), dt);
        self.step_shell_music(&mut front, dt);
        self.front = Some(front);
        Ok(())
    }

    /// The setup screen the lobby shows (the host's, or a guest's copy the
    /// host's state is drawn on), built once a session.
    fn ensure_lobby_ia(&mut self) {
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        if n.lobby.ia.is_some() {
            return;
        }
        let mut ia = game::shell_ia::lobby_instant_action(
            &self.assets_dir,
            &self.missions_dir,
            self.profile.settings.difficulty,
        );
        ia.options.num_bots = self.profile.settings.team_size_pct as i32;
        ia.options.con_numbots = self.profile.settings.team_size_pct as i32;
        ia.options.con_mult = self.profile.settings.reinforcement_pct as i32;
        n.lobby.full = ia.maps.clone();
        n.lobby.ia = Some(ia);
    }

    /// The host's list kept to what every connected player can play.
    fn host_lobby_restrict(&mut self) {
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        let common = common_content(n);
        if n.lobby.common.as_ref() == Some(&common) {
            return;
        }
        let addon = n.lobby.content.addon_heroes;
        let lobby = &mut n.lobby;
        let lines = match lobby.ia.as_mut() {
            Some(ia) => lb::restrict(ia, &lobby.full, &common, addon),
            None => Vec::new(),
        };
        lobby.common = Some(common);
        for l in lines {
            lobby.say(l);
        }
    }

    /// The host's next playlist row (LAN §4.8), or back to its setup
    /// screen after the last.
    fn host_next_round(&mut self) {
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        // The roster may have changed since Launch: rows (and the addon
        // heroes) not every connected player has now are dropped here.
        let common = common_content(n);
        let mut session = n.lobby.session.unwrap_or_default();
        let mut lines = Vec::new();
        let next = lb::next_playable_row(
            &mut n.lobby.playlist,
            &mut session,
            &common,
            n.lobby.content.addon_heroes,
            &mut lines,
        );
        if n.lobby.session.is_some() {
            n.lobby.session = Some(session);
        }
        for l in lines {
            n.lobby.say(l);
        }
        let Some(row) = next else {
            n.lobby.phase = LobbyPhase::Choosing;
            n.lobby.ready.clear();
            n.lobby.results_at = None;
            if let Some(ia) = n.lobby.ia.as_mut() {
                ia.enter();
            }
            n.lobby
                .say("The playlist is over: choose the next one".into());
            return;
        };
        let pos = n.lobby.played;
        n.lobby.played += 1;
        let setup = n.host_row_setup(&row, &session, &self.profile, pos);
        n.lobby.phase = LobbyPhase::InRound(row.label.clone());
        n.lobby.round_map = Some((row.key.clone(), row.label.clone()));
        n.lobby.round_mode = row.mode.english().to_owned();
        n.lobby.ready.clear();
        n.lobby.results_at = None;
        self.session = session;
        self.host_publish();
        let n = self.netplay.as_mut().expect("session");
        match n.host_start(&setup) {
            Ok(setup) => self.net_load(setup),
            Err(e) => n.lobby.say(format!("Cannot start {}: {e:#}", row.label)),
        }
    }

    /// The host's lobby state and discovery blob, sent when they change.
    pub(super) fn host_publish(&mut self) {
        let Some(n) = self.netplay.as_mut().filter(|n| n.is_host()) else {
            return;
        };
        // A command-line host starts its first round before any lobby
        // frame: its setup screen is built here, so the roster (names over
        // heads, the scoreboard, pings) reaches the guests in that round too.
        if n.lobby.ia.is_none() {
            self.ensure_lobby_ia();
            self.host_lobby_restrict();
        }
        let Some(n) = self.netplay.as_mut() else {
            return;
        };
        let Some(ia) = n.lobby.ia.as_ref() else {
            return;
        };
        let mut s = LobbyState::from_ia(ia);
        s.phase = match &n.lobby.phase {
            LobbyPhase::Results { .. } => LobbyPhase::Results {
                ready: n.lobby.ready.iter().copied().collect(),
            },
            p => p.clone(),
        };
        s.cheat_policy = n.args.cheats;
        s.lag_comp = n.lobby.lag_comp;
        s.awards = n.lobby.awards;
        s.max_players = n.lobby.max_players;
        s.roster = host_roster(n);
        s.message = n.lobby.lines.back().map(|(l, _)| l.clone());
        let bytes = s.encode();
        if bytes != n.lobby.sent {
            n.lobby.sent = bytes.clone();
            n.link.host(net::HostCommand::SetLobby(bytes));
        }
        let info = HostInfo {
            name: n.profile.name.clone(),
            map: n
                .lobby
                .round_map
                .as_ref()
                .map(|(_, l)| l.clone())
                .unwrap_or_default(),
            mode: if n.lobby.round_map.is_some() {
                n.lobby.round_mode.clone()
            } else {
                String::new()
            },
            players: (n.peer.local_count() + n.peer.guests().len()) as u8,
            max_players: n.lobby.max_players,
            protocol: net::PROTOCOL,
            build: game::netplay::build_id(),
            session: n.lobby.host_session,
        }
        .encode();
        if info != n.lobby.info_sent {
            n.lobby.info_sent = info.clone();
            n.link.host(net::HostCommand::SetInfo(info));
        }
    }

    /// Draws the lobby: the setup screen (or the waiting line), the roster
    /// and the status.
    fn draw_lobby(&mut self, front: &mut FrontShell, dt: f32) -> Result<()> {
        let prompts = ui::lobby::RosterPrompts::of(&self.current_icons());
        let Some(n) = self.netplay.as_mut() else {
            return Ok(());
        };
        let host = n.is_host();
        let roster = roster_rows(n);
        let phase = if host {
            Some(n.lobby.phase.clone())
        } else {
            n.lobby.state.as_ref().map(|s| s.phase.clone())
        };
        let status = match (&phase, host) {
            (None, _) => Some(n.status.clone()),
            (Some(LobbyPhase::Choosing), false) => Some("The host is choosing\u{2026}".to_owned()),
            (Some(LobbyPhase::Choosing), true) => None,
            (Some(LobbyPhase::InRound(map)), _) => Some(format!(
                "The host is playing {map}; you join the next round"
            )),
            (Some(LobbyPhase::Results { ready }), _) => {
                // Counted by machine: one Ready answers for its players (B19).
                let machines: BTreeSet<u8> = roster.iter().map(|r| r.machine).collect();
                Some(format!(
                    "Waiting for other players ({}/{} ready)",
                    ready.iter().filter(|m| machines.contains(m)).count(),
                    machines.len()
                ))
            }
        };
        let lines = n.lobby.lines();
        let banner = n.banner_text().map(str::to_owned);
        let awards = match n.lobby.state.as_ref().filter(|_| !host) {
            Some(s) => s.awards,
            None => n.lobby.awards,
        };
        let title = format!(
            "Players {}/{} \u{b7} Awards {}",
            roster.len(),
            n.lobby
                .state
                .as_ref()
                .map_or(n.lobby.max_players, |s| s.max_players.max(1)),
            if awards { "On" } else { "Off" }
        );
        let ia = n.lobby.ia.clone().filter(|_| phase.is_some());
        // The movie behind every page but the mission list (§1.1).
        let movie_name = front.menus.movie();
        let mut has_movie = false;
        if let Some(m) = front.movie.as_mut() {
            m.play(movie_name);
            if let Some((rgba, w, h)) = m.frame(dt) {
                if let Some(v) = front.view.as_mut() {
                    v.set_movie_frame(rgba, w, h);
                }
            }
            has_movie = m.shown;
        }
        let mut kicked = None;
        let mut over_panel = false;
        let mut nav = n.lobby.roster_nav;
        if let (Some(r), Some(overlay), Some(window)) =
            (&mut self.renderer, &mut self.overlay, &self.window)
        {
            let (w, h) = r.size();
            if let Some(v) = front.view.as_mut() {
                match ia.as_ref() {
                    Some(ia) => {
                        let shown = has_movie && ia.page != ui::shell::Page::Missions;
                        v.update_instant(&front.menus, ia, shown, [w, h]);
                    }
                    None => {
                        front.menus.covered = true;
                        v.update_shell(&front.menus, has_movie, [w, h]);
                    }
                }
            }
            let paint = overlay.begin(window, |ctx| {
                if let Some(t) = &status {
                    ui::lobby::draw_status(ctx, t);
                }
                if phase.is_some() {
                    kicked = ui::lobby::draw_roster(
                        ctx, &title, &roster, host, &lines, &mut nav, &prompts,
                    );
                }
                if let Some(t) = &banner {
                    super::lan::draw_banner(ctx, t);
                }
                over_panel = ctx.is_pointer_over_area();
            });
            let view = &mut front.view;
            r.render_with_overlay(
                &Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, target, size| {
                    if let Some(v) = view.as_mut() {
                        v.paint(dev, q, enc, target, size);
                    }
                    overlay.paint(&paint, dev, q, enc, target, size)
                }),
            )?;
        }
        if let Some(n) = self.netplay.as_mut() {
            n.lobby.over_panel = over_panel;
            n.lobby.roster_nav = nav;
            if let Some(slot) = kicked {
                lobby_kick(n, slot);
            }
        }
        Ok(())
    }
}

/// The host removes guest `machine` (its first slot) from the game, every
/// player on it (the roster's Kick, by pointer or pad; B19). The host's own
/// players are never kicked.
fn lobby_kick(n: &mut NetplaySession, machine: u8) {
    if !n.is_host() || n.peer.host_local(machine) {
        return;
    }
    let rows = roster_rows(n);
    let mut names = ui::lobby::machine_names(&rows, machine);
    if names.is_empty() {
        names = format!("Player {}", u32::from(machine) + 1);
    }
    // The network core kicks the slot's whole connection.
    n.link.host(net::HostCommand::Kick {
        slot: machine,
        ban: false,
    });
    let were = if rows.iter().filter(|r| r.machine == machine).count() > 1 {
        "were"
    } else {
        "was"
    };
    n.lobby.say(format!("{names} {were} removed from the game"));
}

/// The roster panel's rows: the host's own roster, or the one the host
/// last sent, with each player's ready mark.
fn roster_rows(n: &NetplaySession) -> Vec<ui::lobby::RosterRow> {
    let host = n.is_host();
    let players = if host {
        host_roster(n)
    } else {
        n.lobby
            .state
            .as_ref()
            .map(|s| s.roster.clone())
            .unwrap_or_default()
    };
    let rows = players
        .into_iter()
        .map(|p| {
            // Ready is the machine's (B19).
            let ready = match &n.lobby.phase {
                LobbyPhase::Results { ready } if host => ready.contains(&p.machine),
                _ => match n.lobby.state.as_ref().map(|s| &s.phase) {
                    Some(LobbyPhase::Results { ready }) => ready.contains(&p.machine),
                    _ => false,
                },
            };
            ui::lobby::RosterRow {
                slot: p.slot,
                machine: p.machine,
                name: p.name,
                ping_ms: p.ping_ms,
                addon: p.addon,
                team: p
                    .team
                    .map_or_else(|| "Auto".into(), |t| format!("{}", t + 1)),
                ready,
            }
        })
        .collect();
    ui::lobby::group_rows(rows)
}

impl NetplaySession {
    /// The session's players by slot with their pings: the host's own
    /// roster, or the latest a guest heard (the host keeps sending it in a
    /// round).
    pub(super) fn roster(&self) -> Vec<LobbyPlayer> {
        if self.is_host() {
            host_roster(self)
        } else {
            self.lobby
                .state
                .as_ref()
                .map(|s| s.roster.clone())
                .unwrap_or_default()
        }
    }
}

/// What every connected player can play now: the host's content and each
/// guest's that has a readable manifest, intersected.
fn common_content(n: &NetplaySession) -> ContentManifest {
    let guests: Vec<&ContentManifest> = n
        .peer
        .guests()
        .values()
        .filter_map(|g| g.content.as_ref())
        .collect();
    ContentManifest::intersect(std::iter::once(&n.lobby.content).chain(guests))
}

/// The host's roster: itself, then every connected guest.
fn host_roster(n: &NetplaySession) -> Vec<LobbyPlayer> {
    let pings: std::collections::BTreeMap<u8, u32> = match &n.link {
        NetLink::Host(s) => s
            .stats()
            .peers
            .iter()
            .map(|p| (p.slot, p.rtt_ms.round() as u32))
            .collect(),
        NetLink::Client(_) => Default::default(),
    };
    // The host's own players (B19): slot 0, then its further locals with the
    // unique names the network core gave them.
    let own = n.locals.iter().filter_map(|l| {
        let slot = l.slot.filter(|s| n.peer.host_local(*s))?;
        Some(LobbyPlayer {
            slot,
            machine: 0,
            name: l.name.clone(),
            ping_ms: None,
            addon: n.lobby.content.addon_heroes,
            team: None,
        })
    });
    std::iter::once(LobbyPlayer {
        slot: 0,
        machine: 0,
        name: n.profile.name.clone(),
        ping_ms: None,
        addon: n.lobby.content.addon_heroes,
        team: None,
    })
    .chain(own)
    .chain(n.peer.guests().iter().map(|(slot, g)| {
        LobbyPlayer {
            slot: *slot,
            machine: g.machine,
            name: g.name.clone(),
            // A machine's one link: its first slot's figures.
            ping_ms: Some(
                pings
                    .get(slot)
                    .or_else(|| pings.get(&g.machine))
                    .copied()
                    .unwrap_or(0),
            ),
            addon: g.content.as_ref().is_some_and(|c| c.addon_heroes),
            team: None,
        }
    }))
    .collect()
}

#[cfg(test)]
#[path = "lobby/host_layout_tests.rs"]
mod host_layout_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Boxed: the test's own frame would hold every by-value copy of the
    /// 100 KB App a debug build makes.
    fn app() -> Box<App> {
        Box::new(App::new(
            Default::default(),
            Default::default(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        ))
    }

    fn free_port() -> u16 {
        std::net::UdpSocket::bind("127.0.0.1:0")
            .and_then(|s| s.local_addr())
            .map(|a| a.port())
            .expect("a free UDP port")
    }

    /// Both lobbies, frame by frame, until `done` holds (5 s at most).
    pub(crate) fn run(host: &mut App, guest: &mut App, mut done: impl FnMut(&App, &App) -> bool) {
        let start = Instant::now();
        while start.elapsed().as_secs_f32() < 5.0 {
            if done(host, guest) {
                return;
            }
            for a in [&mut *host, &mut *guest] {
                if matches!(a.screen, Screen::Lobby) {
                    a.redraw_lobby(1.0 / 60.0).expect("lobby frame");
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        assert!(done(host, guest), "not reached in 5 s");
    }

    /// A host's lobby and a guest in it, over UDP on this machine.
    pub(crate) fn lobby_pair() -> (Box<App>, Box<App>) {
        let port = free_port();
        let mut host = app();
        host.host_from_shell(4, port, 0, true).expect("hosting");
        host.enter_lobby();
        let mut guest = app();
        let mut args = guest.lan_args(false);
        args.addr = Some(format!("127.0.0.1:{port}"));
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let n = NetplaySession::join(args, guest.lan_profile(), addr).expect("joining");
        guest.netplay = Some(n);
        guest.enter_lobby();
        run(&mut host, &mut guest, |_, g| {
            g.netplay
                .as_ref()
                .and_then(|n| n.lobby.state.as_ref())
                .is_some_and(|s| s.roster.len() == 2)
        });
        (host, guest)
    }

    /// The Tailscale fix, over UDP on this machine: a recent game is written
    /// only once the host accepts the guest, under the host's name; the next
    /// Join Game asks that address by unicast (no broadcast is needed to
    /// reach it), shows the host's answer on the recent row, and one press
    /// joins it.
    #[test]
    fn a_recent_game_is_written_on_welcome_and_found_again_by_unicast() {
        let port = free_port();
        let mut host = app();
        host.profile.name = "Ann".into();
        host.host_from_shell(4, port, 0, true).expect("hosting");
        host.enter_lobby();
        let mut guest = app();
        guest.profile.name = "Bo".into();
        // An address nobody hosts on: no welcome, no recent game.
        let dead = free_port();
        let dead_text = format!("127.0.0.1:{dead}");
        guest
            .join_from_shell(&dead_text, SocketAddr::from(([127, 0, 0, 1], dead)))
            .expect("joining");
        for _ in 0..50 {
            guest.net_idle();
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        assert!(
            guest.recent_games().games.is_empty(),
            "not before a welcome"
        );
        guest.netplay.take().unwrap().shutdown();
        let text = format!("127.0.0.1:{port}");
        guest
            .join_from_shell(&text, SocketAddr::from(([127, 0, 0, 1], port)))
            .expect("joining");
        guest.enter_lobby();
        run(&mut host, &mut guest, |_, g| {
            g.recent_games
                .as_ref()
                .and_then(|r| r.games.first())
                .is_some_and(|r| r.addr == text && r.name == "Ann" && r.last_joined_unix > 0)
        });
        assert_eq!(guest.recent_games().games.len(), 1);
        guest.netplay.take().unwrap().shutdown();
        // Join Game again: the recent game answers on its own row.
        let mut front = FrontShell::new(&guest.assets_dir);
        front.mp = Some(MpPage::join(guest.recent_rows()));
        let live = |f: &FrontShell| match f.mp.as_ref() {
            Some(MpPage::Join(j)) => j.page.recent.first().and_then(|r| r.live.clone()),
            _ => None,
        };
        let start = Instant::now();
        while live(&front).is_none() && start.elapsed().as_secs_f32() < 5.0 {
            host.redraw_lobby(1.0 / 60.0).expect("host frame");
            assert!(guest
                .step_mp(&mut front, Default::default(), false, 1.0 / 60.0)
                .is_none());
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        let row = live(&front).expect("the recent game answered within 5 s");
        assert_eq!(row.name, "Ann");
        assert_eq!(row.refusal, None, "same build");
        let Some(MpPage::Join(j)) = front.mp.as_ref() else {
            unreachable!()
        };
        assert!(j.page.hosts.is_empty(), "listed once, on its recent row");
        assert_eq!(
            j.page.row_ids()[j.page.cursor],
            ui::lobby::JoinRow::Recent(0)
        );
        assert!(j.page.rows()[j.page.cursor]
            .label
            .starts_with("Ann \u{2014} in the lobby"));
        let press = ui::menu::MenuInput {
            confirm: true,
            ..Default::default()
        };
        assert!(matches!(
            guest.step_mp(&mut front, press, false, 1.0 / 60.0),
            Some(MpDone::Join(ui::lobby::JoinCommand::Connect(t))) if t == text
        ));
        // Square forgets it, and the next Join Game no longer lists it.
        let forget = guest.step_mp(&mut front, Default::default(), true, 1.0 / 60.0);
        guest.front = Some(front);
        guest.mp_done(forget.expect("forget"));
        assert!(guest.recent_games().games.is_empty());
        if let Some(n) = host.netplay.take() {
            n.shutdown();
        }
    }

    /// Headless, over UDP on this machine: Start Hosting opens the lobby;
    /// a guest joins and draws the host's state (roster, phase); at the
    /// results it is ready by itself, so the host's ready check completes
    /// and the playlist's end returns everyone to the setup screen; Kick
    /// ends the guest's session with plain text.
    #[test]
    fn a_guest_joins_the_lobby_waits_at_the_results_and_can_be_kicked() {
        let port = free_port();
        let mut host = app();
        host.profile.name = "Ann".into();
        host.host_from_shell(4, port, 0b11, true).expect("hosting");
        host.enter_lobby();
        assert!(matches!(host.screen, Screen::Lobby));
        let mut guest = app();
        guest.profile.name = "Bo".into();
        let mut args = guest.lan_args(false);
        args.addr = Some(format!("127.0.0.1:{port}"));
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let n = NetplaySession::join(args, guest.lan_profile(), addr).expect("joining");
        guest.netplay = Some(n);
        guest.enter_lobby();
        let state = |a: &App| a.netplay.as_ref().and_then(|n| n.lobby.state.clone());
        run(&mut host, &mut guest, |_, g| {
            state(g).is_some_and(|s| s.roster.len() == 2)
        });
        let s = state(&guest).unwrap();
        assert_eq!(s.phase, LobbyPhase::Choosing);
        assert_eq!(
            s.roster.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Ann", "Bo"]
        );
        assert_eq!((s.cheat_policy, s.max_players, s.lag_comp), (0b11, 4, true));
        assert!(guest.netplay.as_ref().unwrap().lobby.ia.is_some());
        host.profile.career.medals[0] = 3;
        guest.profile.career.medals[0] = 7;
        // The results' ready check: the host has left its pages.
        {
            let n = host.netplay.as_mut().unwrap();
            n.lobby.phase = LobbyPhase::Results { ready: Vec::new() };
            n.lobby.ready.insert(0);
            n.lobby.results_at = Some(Instant::now());
        }
        run(&mut host, &mut guest, |h, _| {
            h.netplay.as_ref().unwrap().lobby.phase == LobbyPhase::Choosing
        });
        let setup = host
            .netplay
            .as_ref()
            .unwrap()
            .host_setup("bf2", &host.session, &host.profile);
        assert_eq!(setup.humans[0].name, "Ann");
        assert_eq!(setup.humans[0].profile.medals[0], 3);
        assert_eq!(setup.humans[1].name, "Bo");
        assert_eq!(setup.humans[1].profile.medals[0], 7);
        let lines = host.netplay.as_mut().unwrap().lobby.lines();
        assert!(
            lines.iter().any(|l| l.contains("playlist is over")),
            "{lines:?}"
        );
        // Kick: the guest's session ends with why.
        let slot = *host
            .netplay
            .as_ref()
            .unwrap()
            .peer
            .guests()
            .keys()
            .next()
            .unwrap();
        host.netplay
            .as_mut()
            .unwrap()
            .link
            .host(net::HostCommand::Kick { slot, ban: false });
        run(&mut host, &mut guest, |_, g| {
            g.netplay.as_ref().is_some_and(|n| n.ended.is_some())
        });
        let why = guest.netplay.as_ref().unwrap().ended.clone().unwrap().0;
        assert_eq!(why, "The host removed you from the game");
        for mut a in [host, guest] {
            if let Some(n) = a.netplay.take() {
                n.shutdown();
            }
        }
    }

    /// Issue #194 B19, headless over UDP on this machine: the host plays with
    /// a second local player (Rex) and a guest machine joins with two (Bo and
    /// Leia). Every roster groups the rows by machine (the host's own first,
    /// a machine's first player before its further one), the round's seats
    /// carry the host's local with its own profile, one Ready answers for the
    /// guest machine, the host's own player is never kicked, and a kick from
    /// Leia's row removes the whole guest machine.
    #[test]
    fn a_two_player_guest_machine_groups_readies_and_is_kicked_whole() {
        let port = free_port();
        let mut host = app();
        host.profile.name = "Ann".into();
        host.host_from_shell(4, port, 0, true).expect("hosting");
        let local = |base: PlayerProfile, seat: u8, name: &str, medals: u32| {
            let mut profile = base;
            profile.name = name.into();
            profile.medals[0] = medals;
            super::super::lan::LanLocal {
                seat,
                name: name.into(),
                profile,
                slot: None,
            }
        };
        let rex = local(host.lan_profile(), 1, "Rex", 5);
        host.netplay.as_mut().unwrap().add_host_locals(vec![rex]);
        host.enter_lobby();
        let mut guest = app();
        guest.profile.name = "Bo".into();
        let mut args = guest.lan_args(false);
        args.addr = Some(format!("127.0.0.1:{port}"));
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let leia = local(guest.lan_profile(), 2, "Leia", 0);
        let n = NetplaySession::join_locals(args, guest.lan_profile(), addr, vec![leia])
            .expect("joining");
        guest.netplay = Some(n);
        guest.enter_lobby();
        let state = |a: &App| a.netplay.as_ref().and_then(|n| n.lobby.state.clone());
        run(&mut host, &mut guest, |_, g| {
            state(g).is_some_and(|s| s.roster.len() == 4)
        });
        let rows = |a: &App| {
            roster_rows(a.netplay.as_ref().unwrap())
                .into_iter()
                .map(|r| (r.name.clone(), r.machine, r.follows()))
                .collect::<Vec<_>>()
        };
        let bo = guest.netplay.as_ref().unwrap().peer.local_slot;
        let leia_slot = guest.netplay.as_ref().unwrap().locals[0]
            .slot
            .expect("Leia's slot");
        assert_ne!(bo, 0);
        let want = vec![
            ("Ann".to_owned(), 0, false),
            ("Rex".to_owned(), 0, true),
            ("Bo".to_owned(), bo, false),
            ("Leia".to_owned(), bo, true),
        ];
        assert_eq!(rows(&host), want, "the host's roster, grouped");
        assert_eq!(rows(&guest), want, "the guest's copy, grouped the same");
        {
            let n = host.netplay.as_ref().unwrap();
            let rex = n.locals[0].slot.expect("Rex's slot");
            assert!(n.peer.host_local(rex) && !n.peer.host_local(bo));
            assert_eq!(n.peer.machine_of(leia_slot), bo);
            let setup = n.host_setup("bf2", &host.session, &host.profile);
            let seats: Vec<_> = setup
                .humans
                .iter()
                .map(|h| (h.slot, h.name.as_str()))
                .collect();
            assert_eq!(
                seats,
                [(0, "Ann"), (rex, "Rex"), (bo, "Bo"), (leia_slot, "Leia")]
            );
            assert_eq!(setup.humans[1].profile.medals[0], 5, "Rex's own profile");
        }
        // The results' ready check: the guest machine's one Ready answers.
        {
            let n = host.netplay.as_mut().unwrap();
            n.lobby.phase = LobbyPhase::Results { ready: Vec::new() };
            n.lobby.ready.insert(0);
            n.lobby.results_at = Some(Instant::now());
        }
        run(&mut host, &mut guest, |h, _| {
            h.netplay.as_ref().unwrap().lobby.phase == LobbyPhase::Choosing
        });
        // The host's own Rex: no kick.
        let n = host.netplay.as_mut().unwrap();
        let rex = n.locals[0].slot.unwrap();
        lobby_kick(n, rex);
        assert_eq!(roster_rows(n).len(), 4, "the host's own player stays");
        // From Leia's row the pad asks for, then kicks, her machine.
        let table = roster_rows(n);
        let mut nav = ui::lobby::RosterNav::default();
        nav.input(true, Default::default(), &table, true);
        nav.focus = table.iter().position(|r| r.slot == leia_slot);
        let accept = ui::menu::MenuInput {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(nav.input(false, accept, &table, true), None);
        let machine = nav.input(false, accept, &table, true);
        assert_eq!(machine, Some(bo), "the machine, by its first slot");
        lobby_kick(n, bo);
        run(&mut host, &mut guest, |h, g| {
            g.netplay.as_ref().is_some_and(|n| n.ended.is_some())
                && h.netplay.as_ref().unwrap().peer.guests().is_empty()
        });
        assert_eq!(
            rows(&host),
            want[..2].to_vec(),
            "both of the machine's players left"
        );
        let lines = host.netplay.as_mut().unwrap().lobby.lines();
        assert!(
            lines
                .iter()
                .any(|l| l == "Bo and Leia were removed from the game"),
            "{lines:?}"
        );
        for mut a in [host, guest] {
            if let Some(n) = a.netplay.take() {
                n.shutdown();
            }
        }
    }

    /// Issue #168, the first VPN playtest: the host's player list works
    /// without a mouse (Square / X / Space in, Up / Down, Accept twice to
    /// kick, Back out; keys and pad buttons make the same menu bits), the
    /// setup screen under it gets none of those presses, and every peer
    /// knows every human by his session name, kept after he leaves.
    #[test]
    fn the_host_kicks_from_the_player_list_without_a_mouse() {
        let port = free_port();
        let mut host = app();
        host.profile.name = "Ann".into();
        host.host_from_shell(4, port, 0, true).expect("hosting");
        host.enter_lobby();
        let mut guest = app();
        guest.profile.name = "Bo".into();
        let mut args = guest.lan_args(false);
        args.addr = Some(format!("127.0.0.1:{port}"));
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let n = NetplaySession::join(args, guest.lan_profile(), addr).expect("joining");
        guest.netplay = Some(n);
        guest.enter_lobby();
        let names = |a: &App| {
            a.netplay
                .as_ref()
                .map(|n| n.names().values().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        run(&mut host, &mut guest, |h, g| {
            names(h) == ["Ann", "Bo"] && names(g) == ["Ann", "Bo"]
        });
        let nav = |a: &App| a.netplay.as_ref().unwrap().lobby.roster_nav;
        // The setup screen's place, its entrance clock aside.
        let ia = |a: &App| {
            let mut ia = a.netplay.as_ref().unwrap().lobby.ia.clone().unwrap();
            ia.on_page = 0.0;
            ia
        };
        let screen = ia(&host);
        // One frame of the host with `keys` tapped.
        let press = |a: &mut App, keys: u32| {
            a.keys_tapped |= keys;
            a.redraw_lobby(1.0 / 60.0).expect("lobby frame");
        };
        let space = game::kbm::menu_key(winit::keyboard::KeyCode::Space);
        let down = game::kbm::menu_key(winit::keyboard::KeyCode::ArrowDown);
        let enter = game::kbm::menu_key(winit::keyboard::KeyCode::Enter);
        let back = game::kbm::menu_key(winit::keyboard::KeyCode::Backspace);
        press(&mut host, space);
        assert_eq!(nav(&host).focus, Some(0), "in the list, on the host");
        press(&mut host, down);
        assert_eq!(nav(&host).focus, Some(1), "on Bo");
        assert_eq!(ia(&host), screen, "the setup screen saw no press");
        press(&mut host, enter);
        assert_eq!(nav(&host).confirm, Some(1), "asks first, for Bo");
        press(&mut host, back);
        assert!(
            nav(&host).active() && nav(&host).confirm.is_none(),
            "Back cancels"
        );
        press(&mut host, enter);
        press(&mut host, enter);
        assert!(matches!(host.screen, Screen::Lobby), "Back never left");
        run(&mut host, &mut guest, |_, g| {
            g.netplay.as_ref().is_some_and(|n| n.ended.is_some())
        });
        let why = guest.netplay.as_ref().unwrap().ended.clone().unwrap().0;
        assert_eq!(why, "The host removed you from the game");
        run(&mut host, &mut guest, |h, _| {
            h.netplay.as_ref().unwrap().peer.guests().is_empty()
        });
        let host_lines = host.netplay.as_mut().unwrap().lobby.lines();
        assert!(host_lines
            .iter()
            .any(|l| l == "Bo was removed from the game"));
        assert_eq!(names(&host), ["Ann", "Bo"], "Bo keeps his name");
        // Back closes the list and gives the screen its input back.
        press(&mut host, back);
        assert!(!nav(&host).active() && nav(&host).hidden);
        assert!(matches!(host.screen, Screen::Lobby));
        // Back from a round, the list never keeps the pad it held.
        press(&mut host, space);
        assert!(nav(&host).active());
        host.net_to_lobby(None);
        assert!(!nav(&host).active() && nav(&host).confirm.is_none());
        for mut a in [host, guest] {
            if let Some(n) = a.netplay.take() {
                n.shutdown();
            }
        }
    }

    /// Discovery asks the limited broadcast first, on the default port.
    #[test]
    fn the_browser_asks_the_limited_broadcast() {
        let t = browse_targets();
        assert_eq!(
            t[0],
            SocketAddr::from(([255, 255, 255, 255], net::DEFAULT_PORT))
        );
        assert!(t.iter().all(|a| a.port() == net::DEFAULT_PORT));
    }

    /// Join Game asks the broadcasts and, by unicast, every recent game
    /// that resolved (over a tunnel the only way to hear it), once each.
    #[test]
    fn the_browser_asks_the_recent_games_by_unicast() {
        let a = |s: &str| s.parse::<SocketAddr>().unwrap();
        let broadcasts = [a("255.255.255.255:37450"), a("192.168.1.255:37450")];
        let recent = [
            Some(a("100.64.0.9:37450")),
            None,
            Some(a("100.64.0.9:37450")),
            Some(a("10.0.0.4:40000")),
        ];
        assert_eq!(
            join_targets(&broadcasts, &recent),
            [
                a("255.255.255.255:37450"),
                a("192.168.1.255:37450"),
                a("100.64.0.9:37450"),
                a("10.0.0.4:40000"),
            ]
        );
        assert_eq!(resolve_v4("10.0.0.4"), Some(a("10.0.0.4:37450")));
        assert_eq!(
            resolve_v4("[::1]:37450"),
            None,
            "the browser's socket is IPv4"
        );
    }

    /// Issue #128: the LAN lobby is a menu too, and its frames keep the
    /// shell music going: over twelve minutes in the host's lobby a take
    /// follows the last after the shell's gap (the lobby used to leave the
    /// playlist unstepped, so the music ended with its take and never came
    /// back between rounds).
    #[test]
    fn the_lobby_keeps_the_shell_music_going_for_twelve_minutes() {
        let port = free_port();
        let mut host = app();
        host.host_from_shell(4, port, 0b11, true).expect("hosting");
        host.enter_lobby();
        let mut front = FrontShell::new(&host.assets_dir);
        front.set_music_takes(vec![
            super::super::front::test_take(95),
            super::super::front::test_take(140),
        ]);
        host.front = Some(front);
        let dt = 0.5;
        let (mut silent, mut longest, mut starts, mut was) = (0.0f32, 0.0f32, 0, None);
        for _ in 0..(12.0 * 60.0 / dt) as usize {
            host.redraw_lobby(dt).expect("lobby frame");
            assert!(matches!(host.screen, Screen::Lobby));
            let now = host.front.as_ref().expect("front").music_playing();
            if now.is_some() && was.is_none() {
                starts += 1;
            }
            if now.is_some() {
                silent = 0.0;
            } else {
                silent += dt;
                longest = longest.max(silent);
            }
            was = now;
        }
        assert!(starts >= 4, "{starts} takes in twelve minutes");
        assert!(
            longest <= ui::shell_feel::MUSIC_GAP + dt,
            "{longest} s without music"
        );
    }
}
