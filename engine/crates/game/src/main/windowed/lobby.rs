//! The multiplayer pages and the lobby (issue #168,
//! `analysis/plan/LAN_MULTIPLAYER.md` §3, §4.1-§4.3, §4.8; FEATURE_SET
//! "Host (P1) sessions", "Host-only session authority", "Post-match
//! results"):
//!
//! - **Host Game** (Main → Multiplayer → LAN: Host Game): max players, the
//!   port, the guests' cheat policy and the award policy
//!   ([`ui::lobby::HostSetup`]; lag compensation is always on); Start Hosting opens the session and the
//!   lobby.
//! - **Join Game**: the LAN browser (`Find` to the limited broadcast and the
//!   primary interface's /24 directed broadcast), Join by address (resolved
//!   off the frame loop) and the recent hosts (the profile's).
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
    /// The round a guest sent its `Ready` for.
    ready_sent: Option<u32>,
    pub max_players: u8,
    pub lag_comp: bool,
    /// The host's award policy (LAN §5b): the next round's, and the round
    /// in play's since the pause menu last switched it.
    pub awards: bool,
    /// The pointer was over a panel of the lobby's own last frame.
    over_panel: bool,
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
            ready_sent: None,
            max_players: ui::lobby::DEFAULT_MAX_PLAYERS,
            lag_comp: true,
            awards: true,
            over_panel: false,
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

    fn lines(&mut self) -> Vec<String> {
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

/// The Host page's help lines (LAN §5b): this machine's addresses, looked
/// up again every few seconds (two UDP connects, nothing sent).
fn host_help(port: u16) -> Vec<String> {
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
    reach::host_help(&addrs, port)
}

/// The broadcast targets: the limited broadcast, and the /24 directed
/// broadcast of the interface the default route leaves by (a UDP connect
/// sends nothing). Without a crate for the interface list the mask is not
/// known; a wrong guess only reaches nobody.
pub(super) fn browse_targets() -> Vec<SocketAddr> {
    let mut v = net::LanBrowser::default_targets();
    let local = std::net::UdpSocket::bind("0.0.0.0:0").and_then(|s| {
        s.connect("192.0.2.1:9")?;
        s.local_addr()
    });
    if let Ok(SocketAddr::V4(a)) = local {
        let o = a.ip().octets();
        if a.ip().is_private() || a.ip().is_link_local() {
            let b = std::net::Ipv4Addr::new(o[0], o[1], o[2], 255);
            v.push(SocketAddr::from((b, net::DEFAULT_PORT)));
        }
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

    pub fn join(recent: Vec<String>) -> Self {
        let browser = net::UdpTransport::client()
            .map_err(|e| eprintln!("lan browser: {e}"))
            .ok()
            .map(|t| (net::LanBrowser::new(browse_targets()), t));
        MpPage::Join(Box::new(JoinScreen {
            page: ui::lobby::JoinPage::new(recent),
            browser,
            clock: net::RealClock::new(),
            resolving: None,
            test: None,
        }))
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
            let port = h.parsed_port().unwrap_or(net::DEFAULT_PORT);
            for l in host_help(port) {
                lines.push((l, Color32::from_rgb(170, 210, 255)));
            }
            lines.push(help);
            let cursor = h.cursor;
            let focus = std::mem::take(&mut h.focus_text);
            ui::lobby::draw_page(
                ctx,
                "LAN: Host Game",
                &rows,
                cursor,
                h.text_mut(),
                focus,
                &lines,
            )
        }
        MpPage::Join(j) => {
            let rows = j.page.rows();
            let mut lines = Vec::new();
            if j.page.hosts.is_empty() {
                lines.push((
                    "Looking for games on this network\u{2026}".to_owned(),
                    Color32::LIGHT_GRAY,
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
        _dt: f32,
    ) -> Option<MpDone> {
        let build = game::netplay::build_id();
        let (done, clicks) = match front.mp.as_mut()? {
            MpPage::Host(h) => {
                let d = h.step(input).map(MpDone::Host);
                (d, h.take_clicks())
            }
            MpPage::Join(j) => {
                if let Some((b, t)) = j.browser.as_mut() {
                    use net::Clock;
                    b.pump(j.clock.now(), t);
                    let rows = b
                        .hosts()
                        .into_iter()
                        .filter_map(|h| {
                            let info = HostInfo::decode(&h.info)?;
                            Some(ui::lobby::FoundRow {
                                addr: h.addr.to_string(),
                                refusal: info.refusal(&build),
                                name: info.name,
                                map: if info.map.is_empty() {
                                    "in the lobby".into()
                                } else {
                                    info.map
                                },
                                players: format!("{}/{}", info.players, info.max_players),
                                ping_ms: h.rtt.as_millis_f64().round() as u32,
                            })
                        })
                        .collect();
                    j.page.set_hosts(rows);
                }
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
                let d = resolved.or_else(|| j.page.step(input).map(MpDone::Join));
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
                }
                (h.step(press).map(MpDone::Host), h.take_clicks())
            }
            MpPage::Join(j) => {
                if let Some(i) = d.clicked {
                    j.page.cursor = i;
                }
                (j.page.step(press).map(MpDone::Join), j.page.take_clicks())
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
            MpDone::Join(JoinCommand::Connect(text)) => {
                if let Some(MpPage::Join(j)) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
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
        println!("lan: {}", n.status);
        self.netplay = Some(n);
        Ok(())
    }

    /// Join Game's Connect: the session is open (the lobby says when the
    /// host answers); the address joins the recent hosts.
    fn join_from_shell(&mut self, text: &str, addr: SocketAddr) -> Result<()> {
        let mut args = self.lan_args(false);
        args.addr = Some(text.to_owned());
        let mut n = NetplaySession::join(args, self.lan_profile(), addr)?;
        n.status = format!("Connecting to {text}\u{2026}");
        lb::remember_host(&mut self.profile.settings.recent_hosts, text);
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
        println!("lan: {}", n.status);
        self.netplay = Some(n);
        Ok(())
    }

    /// The shell hands over to the lobby screen.
    fn enter_lobby(&mut self) {
        if let Some(f) = self.front.as_mut() {
            f.mp = None;
            f.text_focus = false;
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
        if self.walk.is_some() {
            let n = self.netplay.take();
            self.leave_battle();
            self.netplay = n;
        }
        if let Some(n) = self.netplay.as_mut() {
            n.peer.leave_round();
            n.loading = None;
            n.view.reset();
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
                n.lobby.ready.insert(0);
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
        let recent = self.profile.settings.recent_hosts.clone();
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
        let input = ui::menu::MenuInput {
            up: edges & 1 != 0,
            down: edges & 2 != 0,
            left: edges & 4 != 0,
            right: edges & 8 != 0,
            confirm: edges & 16 != 0,
            back: edges & 32 != 0,
        };
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
                    let guests: Vec<u8> = n.peer.guests().keys().copied().collect();
                    let ready = &n.lobby.ready;
                    let all = guests.iter().all(|s| ready.contains(s));
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
            // A guest: the host's screen, read-only; only Back (leave).
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
                    s.send(net::ClientCommand::Ready(round));
                }
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
        let mut ia = game::shell_ia::instant_action(
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
            players: 1 + n.peer.guests().len() as u8,
            max_players: n.lobby.max_players,
            protocol: net::PROTOCOL,
            build: game::netplay::build_id(),
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
        let Some(n) = self.netplay.as_mut() else {
            return Ok(());
        };
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
        let roster: Vec<ui::lobby::RosterRow> = players
            .into_iter()
            .map(|p| {
                let ready = match &n.lobby.phase {
                    LobbyPhase::Results { ready } if host => ready.contains(&p.slot),
                    _ => match n.lobby.state.as_ref().map(|s| &s.phase) {
                        Some(LobbyPhase::Results { ready }) => ready.contains(&p.slot),
                        _ => false,
                    },
                };
                ui::lobby::RosterRow {
                    slot: p.slot,
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
            (Some(LobbyPhase::Results { ready }), _) => Some(format!(
                "Waiting for other players ({}/{} ready)",
                ready.len(),
                roster.len()
            )),
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
                    kicked = ui::lobby::draw_roster(ctx, &title, &roster, host, &lines);
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
            if let Some(slot) = kicked {
                let name = n
                    .peer
                    .guests()
                    .get(&slot)
                    .map_or_else(|| format!("Player {slot}"), |g| g.name.clone());
                n.link.host(net::HostCommand::Kick { slot, ban: false });
                n.lobby.say(format!("{name} was removed from the game"));
            }
        }
        Ok(())
    }
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
    std::iter::once(LobbyPlayer {
        slot: 0,
        name: n.profile.name.clone(),
        ping_ms: None,
        addon: n.lobby.content.addon_heroes,
        team: None,
    })
    .chain(n.peer.guests().iter().map(|(slot, g)| LobbyPlayer {
        slot: *slot,
        name: g.name.clone(),
        ping_ms: Some(pings.get(slot).copied().unwrap_or(0)),
        addon: g.content.as_ref().is_some_and(|c| c.addon_heroes),
        team: None,
    }))
    .collect()
}

#[cfg(test)]
mod tests {
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
    fn run(host: &mut App, guest: &mut App, mut done: impl FnMut(&App, &App) -> bool) {
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
}
