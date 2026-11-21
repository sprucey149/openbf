//! `[ours]` The LAN multiplayer pages of the shell (issue #168,
//! `analysis/plan/LAN_MULTIPLAYER.md` §3, §4.1, §5): Host Game's setup page,
//! Join Game's browser and the lobby's roster panel. The models step on the
//! shell's [`MenuInput`] and ask for the shell's clicks; the game owns the
//! sessions, the profile and the drawing order.
//!
//! Rules:
//! - Host setup: max players 2..=16 (default 8), the UDP port (a text box),
//!   the cheat policy (Off, or Allowed with one row per cheat) and lag
//!   compensation on or off, and the award policy (LAN §5b: Awards On, the
//!   single player's default, lets each player's own earned awards apply;
//!   Off, nobody's); Start Hosting checks the port (1..=65535).
//! - Join: "Games on this network" (the hosts the LAN browser heard that
//!   are not recent games; one on a different version is listed dimmed
//!   with why), then "Recent games" (this computer's, newest first: the
//!   host's answer when it is hosting, else "not hosting now", always
//!   pressable; Square, Space or a right-click forgets one), then a
//!   Join-by-address text box (IP or host name, optional `:port`), Test
//!   Connection and Back. The cursor starts on the first joinable game,
//!   else in the address box, and until the player presses something it
//!   moves to the first joinable game that appears; after that the focus
//!   follows its row by address when the lists change under it.
//! - Text rows take the keyboard while focused: the shell's Back key does
//!   not leave the page while one types (the caller drops the menu keys
//!   while [`PageDraw::text_focus`] is set; Escape gives the focus back).
//!   Arriving on a text row (a step, or Join Game's first frame) focuses
//!   its box once ([`HostSetup::focus_text`]); a click in the box does too.
//! - Steps wrap last <-> first and skip dimmed rows, as the shell's stacks
//!   do; a move clicks select, a press accept, a refused press error.

use crate::menu::MenuInput;
use crate::shell_feel::MenuSound;
use crate::typography::{
    egui_color, egui_font, egui_text, egui_units, tint, BODY, ITEM, LIST, TITLE,
};
use egui::{Align2, Color32, Vec2};

/// Max players' range and default (`LAN_MULTIPLAYER.md` §8: up to 16).
pub const MAX_PLAYERS_RANGE: (u8, u8) = (2, 16);
pub const DEFAULT_MAX_PLAYERS: u8 = 8;

/// A row as drawn: label, value, whether it can be focused, whether its
/// value is a text box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRow {
    pub label: String,
    pub value: Option<String>,
    pub enabled: bool,
    pub text: bool,
    /// A section heading (never focused): drawn small, without a bar.
    pub heading: bool,
}

impl PageRow {
    fn plain(label: impl Into<String>, value: Option<String>) -> Self {
        Self {
            label: label.into(),
            value,
            enabled: true,
            text: false,
            heading: false,
        }
    }
}

/// The next enabled row from `at` going `step`, wrapping; `at` when none.
fn follow(rows: &[PageRow], at: usize, step: isize) -> usize {
    let n = rows.len() as isize;
    let mut k = at as isize;
    for _ in 0..n {
        k = (k + step).rem_euclid(n);
        if rows[k as usize].enabled {
            return k as usize;
        }
    }
    at
}

fn on_off(on: bool) -> Option<String> {
    Some(crate::menu::on_off(on).into())
}

/// Host Game's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostRow {
    MaxPlayers,
    Port,
    CheatPolicy,
    /// One cheat of the allowed list (an index into [`HostSetup::cheats`]).
    Cheat(usize),
    Awards,
    Start,
    Back,
}

/// What Host Game asks of the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostCommand {
    Start {
        max_players: u8,
        port: u16,
        /// The allowed cheats, bit `i` = [`HostSetup::cheats`]`[i]`; 0 = Off.
        cheat_policy: u32,
        lag_comp: bool,
        /// Earned awards apply (each player's own); false: nobody's.
        awards: bool,
    },
    Back,
}

/// Host Game's setup page (module rules).
#[derive(Clone, Debug, PartialEq)]
pub struct HostSetup {
    pub max_players: u8,
    pub port: String,
    /// The policy is Allowed (else Off: guests may not ask for any).
    pub cheats_allowed: bool,
    /// Every cheat (id, label) in the game's cheat-bit order, and whether
    /// guests may ask for it.
    pub cheats: Vec<(&'static str, &'static str, bool)>,
    /// Lag compensation: always on, not offered on the page (the
    /// maintainer, November 13, 2026); `--lan-host --no-lag-comp` turns it
    /// off for testing.
    pub lag_comp: bool,
    /// The award policy (module rules).
    pub awards: bool,
    pub cursor: usize,
    /// A refusal to show (a bad port, a port in use).
    pub message: Option<String>,
    /// The cursor just arrived on a text row: its box takes the keyboard.
    pub focus_text: bool,
    clicks: Vec<MenuSound>,
}

impl HostSetup {
    /// The page with its defaults: `cheats` all allowed once the policy is
    /// switched to Allowed.
    pub fn new(cheats: &[(&'static str, &'static str)], default_port: u16) -> Self {
        Self {
            max_players: DEFAULT_MAX_PLAYERS,
            port: default_port.to_string(),
            cheats_allowed: false,
            cheats: cheats.iter().map(|(id, l)| (*id, *l, true)).collect(),
            lag_comp: true,
            awards: true,
            cursor: 0,
            message: None,
            focus_text: false,
            clicks: Vec::new(),
        }
    }

    pub fn row_ids(&self) -> Vec<HostRow> {
        let mut v = vec![HostRow::MaxPlayers, HostRow::Port, HostRow::CheatPolicy];
        if self.cheats_allowed {
            v.extend((0..self.cheats.len()).map(HostRow::Cheat));
        }
        v.extend([HostRow::Awards, HostRow::Start, HostRow::Back]);
        v
    }

    pub fn rows(&self) -> Vec<PageRow> {
        self.row_ids()
            .into_iter()
            .map(|r| match r {
                HostRow::MaxPlayers => {
                    PageRow::plain("Max Players", Some(self.max_players.to_string()))
                }
                HostRow::Port => PageRow {
                    text: true,
                    ..PageRow::plain("UDP Port", Some(self.port.clone()))
                },
                HostRow::CheatPolicy => PageRow::plain(
                    "Cheats for Guests",
                    Some(
                        if self.cheats_allowed {
                            "Allowed List"
                        } else {
                            "Off"
                        }
                        .into(),
                    ),
                ),
                HostRow::Cheat(i) => {
                    let (id, label, on) = self.cheats[i];
                    let label = crate::menu::cheat_text(id).map_or(label, |(l, _)| l);
                    PageRow::plain(format!("   {label}"), on_off(on))
                }
                HostRow::Awards => PageRow::plain("Awards", on_off(self.awards)),
                HostRow::Start => PageRow::plain("Start Hosting", None),
                HostRow::Back => PageRow::plain("Back", None),
            })
            .collect()
    }

    /// The allowed-cheat bits (0 when the policy is Off).
    pub fn cheat_policy(&self) -> u32 {
        if !self.cheats_allowed {
            return 0;
        }
        self.cheats
            .iter()
            .enumerate()
            .filter(|(_, c)| c.2)
            .fold(0, |b, (i, _)| b | 1 << i)
    }

    /// The port typed, when it is one.
    pub fn parsed_port(&self) -> Option<u16> {
        self.port.trim().parse::<u16>().ok().filter(|p| *p != 0)
    }

    /// The clicks asked for since the last call.
    pub fn take_clicks(&mut self) -> Vec<MenuSound> {
        std::mem::take(&mut self.clicks)
    }

    /// One frame's presses.
    pub fn step(&mut self, input: MenuInput) -> Option<HostCommand> {
        if input.back {
            self.clicks.push(MenuSound::Exit);
            return Some(HostCommand::Back);
        }
        let rows = self.rows();
        if input.up || input.down {
            self.cursor = follow(&rows, self.cursor, if input.down { 1 } else { -1 });
            self.focus_text = rows[self.cursor].text;
            self.clicks.push(MenuSound::Select);
        }
        let ids = self.row_ids();
        self.cursor = self.cursor.min(ids.len() - 1);
        let row = ids[self.cursor];
        let side = if input.right {
            1
        } else if input.left {
            -1
        } else {
            0
        };
        if side != 0 {
            let (lo, hi) = MAX_PLAYERS_RANGE;
            match row {
                HostRow::MaxPlayers => {
                    self.max_players =
                        (self.max_players as i32 + side).clamp(lo as i32, hi as i32) as u8
                }
                HostRow::CheatPolicy => self.cheats_allowed = !self.cheats_allowed,
                HostRow::Cheat(i) => self.cheats[i].2 = !self.cheats[i].2,
                HostRow::Awards => self.awards = !self.awards,
                _ => return None,
            }
            self.clicks.push(MenuSound::Select);
            return None;
        }
        if !input.confirm {
            return None;
        }
        match row {
            HostRow::CheatPolicy => self.cheats_allowed = !self.cheats_allowed,
            HostRow::Cheat(i) => self.cheats[i].2 = !self.cheats[i].2,
            HostRow::Awards => self.awards = !self.awards,
            HostRow::MaxPlayers | HostRow::Port => return None,
            HostRow::Start => {
                let Some(port) = self.parsed_port() else {
                    self.message = Some("The port must be a number from 1 to 65535".into());
                    self.clicks.push(MenuSound::Error);
                    return None;
                };
                self.clicks.push(MenuSound::Accept);
                return Some(HostCommand::Start {
                    max_players: self.max_players,
                    port,
                    cheat_policy: self.cheat_policy(),
                    lag_comp: self.lag_comp,
                    awards: self.awards,
                });
            }
            HostRow::Back => {
                self.clicks.push(MenuSound::Exit);
                return Some(HostCommand::Back);
            }
        }
        self.clicks.push(MenuSound::Accept);
        None
    }

    /// The text box under the cursor, if the cursor is on one.
    pub fn text_mut(&mut self) -> Option<&mut String> {
        (self.row_ids().get(self.cursor) == Some(&HostRow::Port)).then_some(&mut self.port)
    }
}

/// A host the LAN browser heard, as Join Game lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundRow {
    /// `ip:port`, what Connect joins.
    pub addr: String,
    pub name: String,
    /// The map in play (its label), empty in the lobby.
    pub map: String,
    /// The mode in play ("Conquest"), empty in the lobby.
    pub mode: String,
    pub players: String,
    pub ping_ms: u32,
    /// `None` = joinable, else why not (a different version, full).
    pub refusal: Option<String>,
}

impl FoundRow {
    /// "Ann — Kashyyyk: Docks, Conquest" (with why it cannot be joined).
    fn label(&self) -> String {
        let place = match (self.map.is_empty(), self.mode.is_empty()) {
            (true, _) => "in the lobby".to_owned(),
            (false, true) => self.map.clone(),
            (false, false) => format!("{}, {}", self.map, self.mode),
        };
        let mut label = format!("{} \u{2014} {place}", self.name);
        if let Some(why) = &self.refusal {
            label = format!("{label} ({why})");
        }
        label
    }

    fn value(&self) -> Option<String> {
        Some(format!("{}  {} ms", self.players, self.ping_ms))
    }
}

/// A game this computer joined before, as Join Game lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentRow {
    /// The address as it was joined: what Connect joins.
    pub addr: String,
    /// The host's player name when last joined ("" unknown).
    pub name: String,
    /// When it was last joined ("2 days ago", "" unknown).
    pub joined: String,
    /// The host's answer to this computer's question, while it is hosting.
    pub live: Option<FoundRow>,
}

impl RecentRow {
    pub fn new(
        addr: impl Into<String>,
        name: impl Into<String>,
        joined: impl Into<String>,
    ) -> Self {
        Self {
            addr: addr.into(),
            name: name.into(),
            joined: joined.into(),
            live: None,
        }
    }

    fn row(&self) -> PageRow {
        match &self.live {
            Some(h) => PageRow::plain(h.label(), h.value()),
            None => {
                let who = if self.name.is_empty() {
                    &self.addr
                } else {
                    &self.name
                };
                let label = if self.joined.is_empty() {
                    format!("{who} (not hosting now)")
                } else {
                    format!("{who} (not hosting now, joined {})", self.joined)
                };
                let value = (!self.name.is_empty()).then(|| self.addr.clone());
                PageRow::plain(label, value)
            }
        }
    }
}

/// Join Game's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinRow {
    /// "Games on this network" / "Recent games": a heading, never focused.
    Heading(JoinSection),
    Host(usize),
    Recent(usize),
    Address,
    /// "Test connection" for the typed address (LAN §5b).
    Test,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinSection {
    Found,
    Recent,
}

/// What Join Game asks of the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinCommand {
    /// Join this address (a found host's `ip:port`, the typed text or a
    /// recent game's address).
    Connect(String),
    /// Test the typed address: ask it the discovery question.
    Test(String),
    /// Forget this recent game (its index in [`JoinPage::recent`]).
    Forget(usize),
    Back,
}

/// A row by what it shows, so the focus follows it when the lists change.
#[derive(Clone, Debug, PartialEq, Eq)]
enum JoinKey {
    Host(String),
    Recent(String),
    Other(JoinRow),
}

/// Join Game's page (module rules).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct JoinPage {
    /// The games found on the network that are not recent games.
    pub hosts: Vec<FoundRow>,
    pub address: String,
    pub recent: Vec<RecentRow>,
    pub cursor: usize,
    /// The connection's state or why the last one ended.
    pub status: Option<String>,
    /// The cursor just arrived on the address box (see [`HostSetup::focus_text`]).
    pub focus_text: bool,
    /// The player has pressed something: the cursor no longer jumps to the
    /// first game that appears.
    touched: bool,
    clicks: Vec<MenuSound>,
}

impl JoinPage {
    /// The page with this computer's recent games; the cursor starts on
    /// the first game, else in the address box.
    pub fn new(recent: Vec<RecentRow>) -> Self {
        let mut p = Self {
            recent,
            ..Default::default()
        };
        p.home();
        p
    }

    /// The first joinable game, else the address box (with the keyboard).
    fn home(&mut self) {
        let ids = self.row_ids();
        let joinable = |r: &JoinRow| match r {
            JoinRow::Host(i) => self.hosts[*i].refusal.is_none(),
            JoinRow::Recent(i) => self.recent[*i]
                .live
                .as_ref()
                .is_none_or(|h| h.refusal.is_none()),
            _ => false,
        };
        let live_recent = ids.iter().position(|r| {
            matches!(r, JoinRow::Recent(i) if self.recent[*i].live.is_some()) && joinable(r)
        });
        let first_host = ids
            .iter()
            .position(|r| matches!(r, JoinRow::Host(_)) && joinable(r));
        self.cursor = live_recent
            .or(first_host)
            .or_else(|| {
                ids.iter()
                    .position(|r| matches!(r, JoinRow::Recent(_)) && joinable(r))
            })
            .or_else(|| ids.iter().position(|r| *r == JoinRow::Address))
            .unwrap_or(0);
        self.focus_text = ids[self.cursor] == JoinRow::Address;
    }

    pub fn row_ids(&self) -> Vec<JoinRow> {
        let mut v = Vec::new();
        if !self.hosts.is_empty() {
            v.push(JoinRow::Heading(JoinSection::Found));
            v.extend((0..self.hosts.len()).map(JoinRow::Host));
        }
        if !self.recent.is_empty() {
            v.push(JoinRow::Heading(JoinSection::Recent));
            v.extend((0..self.recent.len()).map(JoinRow::Recent));
        }
        v.extend([JoinRow::Address, JoinRow::Test, JoinRow::Back]);
        v
    }

    pub fn rows(&self) -> Vec<PageRow> {
        self.row_ids()
            .into_iter()
            .map(|r| match r {
                JoinRow::Heading(s) => PageRow {
                    enabled: false,
                    heading: true,
                    ..PageRow::plain(
                        match s {
                            JoinSection::Found => "Games on this network",
                            JoinSection::Recent => "Recent games",
                        },
                        None,
                    )
                },
                JoinRow::Host(i) => {
                    let h = &self.hosts[i];
                    PageRow {
                        enabled: h.refusal.is_none(),
                        ..PageRow::plain(h.label(), h.value())
                    }
                }
                // Always enabled: a host not answering may still be up
                // (a firewall, a slow tunnel), and Enter tries it.
                JoinRow::Recent(i) => self.recent[i].row(),
                JoinRow::Address => PageRow {
                    text: true,
                    ..PageRow::plain("Join by Address", Some(self.address.clone()))
                },
                JoinRow::Test => PageRow::plain("Test Connection", None),
                JoinRow::Back => PageRow::plain("Back", None),
            })
            .collect()
    }

    fn key(&self, r: JoinRow) -> JoinKey {
        match r {
            JoinRow::Host(i) => JoinKey::Host(self.hosts[i].addr.clone()),
            JoinRow::Recent(i) => JoinKey::Recent(self.recent[i].addr.clone()),
            r => JoinKey::Other(r),
        }
    }

    /// Replaces the found games and the recent games' answers (`live`, by
    /// index, the rest of each recent row kept); the focus follows its row.
    /// Before the player presses anything it goes to the first joinable
    /// game.
    pub fn set_hosts(&mut self, hosts: Vec<FoundRow>, live: Vec<Option<FoundRow>>) {
        let same_live = self
            .recent
            .iter()
            .enumerate()
            .all(|(i, r)| r.live == live.get(i).cloned().flatten());
        if hosts == self.hosts && same_live {
            return;
        }
        let ids = self.row_ids();
        let at = ids.get(self.cursor).map(|r| self.key(*r));
        let old_pos = self.cursor;
        self.hosts = hosts;
        for (i, r) in self.recent.iter_mut().enumerate() {
            r.live = live.get(i).cloned().flatten();
        }
        self.settle(at, old_pos);
    }

    /// Replaces the recent games (after a Forget); the focus follows its
    /// row, or the next one.
    pub fn set_recent(&mut self, recent: Vec<RecentRow>) {
        let ids = self.row_ids();
        let at = ids.get(self.cursor).map(|r| self.key(*r));
        let old_pos = self.cursor;
        self.recent = recent;
        self.settle(at, old_pos);
    }

    fn settle(&mut self, at: Option<JoinKey>, old_pos: usize) {
        if !self.touched && self.address.trim().is_empty() {
            self.home();
            return;
        }
        let ids = self.row_ids();
        self.cursor = at
            .and_then(|k| ids.iter().position(|r| self.key(*r) == k))
            .unwrap_or(old_pos.min(ids.len() - 1));
        let rows = self.rows();
        if !rows[self.cursor].enabled {
            self.cursor = follow(&rows, self.cursor, 1);
        }
    }

    pub fn take_clicks(&mut self) -> Vec<MenuSound> {
        std::mem::take(&mut self.clicks)
    }

    /// One frame's presses; `forget` is Square (Space): it forgets
    /// the recent game under the cursor.
    pub fn step(&mut self, input: MenuInput, forget: bool) -> Option<JoinCommand> {
        if input.back {
            self.clicks.push(MenuSound::Exit);
            return Some(JoinCommand::Back);
        }
        if input.up || input.down || input.confirm || forget {
            self.touched = true;
        }
        let rows = self.rows();
        if input.up || input.down {
            self.cursor = follow(&rows, self.cursor, if input.down { 1 } else { -1 });
            self.focus_text = rows[self.cursor].text;
            self.clicks.push(MenuSound::Select);
        }
        let ids = self.row_ids();
        self.cursor = self.cursor.min(ids.len() - 1);
        if forget {
            if let JoinRow::Recent(i) = ids[self.cursor] {
                self.clicks.push(MenuSound::Accept);
                return Some(JoinCommand::Forget(i));
            }
        }
        if !input.confirm {
            return None;
        }
        let addr = match ids[self.cursor] {
            JoinRow::Host(i) if self.hosts[i].refusal.is_none() => self.hosts[i].addr.clone(),
            JoinRow::Host(_) | JoinRow::Heading(_) => String::new(),
            JoinRow::Address | JoinRow::Test => self.address.trim().to_owned(),
            JoinRow::Recent(i) => self.recent[i].addr.clone(),
            JoinRow::Back => {
                self.clicks.push(MenuSound::Exit);
                return Some(JoinCommand::Back);
            }
        };
        if addr.is_empty() {
            self.status = Some("Type the host's address (IP or name, optional :port)".into());
            self.clicks.push(MenuSound::Error);
            return None;
        }
        self.clicks.push(MenuSound::Accept);
        if ids[self.cursor] == JoinRow::Test {
            return Some(JoinCommand::Test(addr));
        }
        Some(JoinCommand::Connect(addr))
    }

    pub fn text_mut(&mut self) -> Option<&mut String> {
        (self.row_ids().get(self.cursor) == Some(&JoinRow::Address)).then_some(&mut self.address)
    }
}

/// What a page's drawing reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageDraw {
    /// A text box has the keyboard: the caller drops the menu keys.
    pub text_focus: bool,
    /// A row was clicked: focus it and press it.
    pub clicked: Option<usize>,
    /// Enter was pressed in a text box: press its row.
    pub submitted: bool,
    /// A row was right-clicked (Join Game: forget that recent game).
    pub secondary: Option<usize>,
}

/// The focused row's bar: the native stack's white bar, faint.
const FOCUS_BG: crate::typography::Tint = ([255.0; 3], 0.14);

/// Host-only compact form (#181). Keep the heading and endpoint/error
/// readouts outside the scrolling settings, within a TV's 5% safe inset.
/// The shared row reveal rule lets keys/pad reach every expanded cheat;
/// an unchanged cursor leaves wheel scrolling alone.
pub fn draw_host_page(
    ctx: &egui::Context,
    rows: &[PageRow],
    cursor: usize,
    text: Option<&mut String>,
    focus: bool,
    lines: &[(String, Color32)],
) -> PageDraw {
    let mut out = PageDraw::default();
    let mut text = text;
    let screen = ctx.content_rect();
    let safe = screen.shrink(screen.height() * 0.05);
    let inset = egui_units(ctx, 10.0);
    let width = egui_units(ctx, 480.0).min(safe.width() - 2.0 * inset);
    let gap = egui_units(ctx, 4.0);
    let pitch = egui_units(ctx, 28.0);
    egui::Area::new(egui::Id::new("lan_host_page"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .constrain_to(safe)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(inset)
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = gap;
                    // Labels must leave row clicks to the form, rather than
                    // taking them for egui's text selection.
                    ui.style_mut().interaction.selectable_labels = false;
                    ui.visuals_mut().clip_rect_margin = 0.0;
                    let title = ui.vertical_centered(|ui| {
                        ui.label(egui_text(ctx, &TITLE, tint::TITLE, "LAN: Host Game"));
                    });
                    let footer: Vec<_> = lines
                        .iter()
                        .map(|(line, color)| {
                            ui.painter()
                                .layout(line.clone(), egui_font(ctx, &BODY), *color, width)
                        })
                        .collect();
                    let footer_height: f32 = footer.iter().map(|g| g.size().y + gap).sum();
                    let room = safe.height()
                        - 2.0 * inset
                        - title.response.rect.height()
                        - gap
                        - footer_height;
                    let shown = ((room + gap) / pitch).floor().max(1.0) as usize;
                    let height = shown.min(rows.len()) as f32 * pitch - gap;
                    let state_id = egui::Id::new("lan_host_rows_state");
                    let offset_id = egui::Id::new("lan_host_rows_offset");
                    let state = (cursor, rows.len(), shown);
                    let before = ctx.data_mut(|d| {
                        let before = d.get_temp::<(usize, usize, usize)>(state_id);
                        d.insert_temp(state_id, state);
                        before
                    });
                    let mut area = egui::ScrollArea::vertical()
                        .id_salt("lan_host_rows")
                        .max_height(height)
                        .min_scrolled_height(height)
                        .auto_shrink([false, true]);
                    if before != Some(state) {
                        let offset = ctx.data(|d| d.get_temp::<f32>(offset_id)).unwrap_or(0.0);
                        let mut view = crate::scroll::ScrollView::at(offset / pitch);
                        let pointer =
                            ctx.input(|i| i.pointer.any_down() || i.pointer.any_released());
                        view.reveal(
                            cursor,
                            rows.len(),
                            shown,
                            if pointer { 0 } else { crate::scroll::MARGIN },
                        );
                        area = area.vertical_scroll_offset(view.target() * pitch);
                    }
                    let list = area.show(ui, |ui| {
                        for (i, row) in rows.iter().enumerate() {
                            let focused = i == cursor;
                            let fg = if !row.enabled {
                                tint::DIMMED
                            } else if focused {
                                tint::SELECTED
                            } else {
                                tint::UNSELECTED
                            };
                            let (rect, hit) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), pitch - gap),
                                egui::Sense::click(),
                            );
                            if focused {
                                ui.painter().rect_filled(rect, 3.0, egui_color(FOCUS_BG));
                            }
                            let rect = rect.shrink2(Vec2::new(gap, 0.0));
                            let split = rect.left() + rect.width() * 0.63;
                            let label_rect = egui::Rect::from_min_max(
                                rect.min,
                                egui::pos2(split - gap, rect.bottom()),
                            );
                            let mut label_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .id_salt(("host_label", i))
                                    .max_rect(label_rect)
                                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            );
                            label_ui.label(egui_text(ctx, &BODY, fg, &row.label));
                            let value_rect =
                                egui::Rect::from_min_max(egui::pos2(split, rect.top()), rect.max);
                            let mut value_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .id_salt(("host_value", i))
                                    .max_rect(value_rect)
                                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
                            );
                            if let (true, Some(t)) = (row.text && focused, text.as_deref_mut()) {
                                let r = value_ui.add(
                                    egui::TextEdit::singleline(t)
                                        .font(egui_font(ctx, &BODY))
                                        .margin(Vec2::new(gap, 0.0))
                                        .desired_width(value_rect.width()),
                                );
                                if focus {
                                    r.request_focus();
                                }
                                if r.lost_focus()
                                    && value_ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    out.submitted = true;
                                }
                                out.text_focus |= r.has_focus();
                            } else if let Some(v) = &row.value {
                                value_ui.label(egui_text(ctx, &BODY, fg, v));
                            }
                            if row.enabled
                                && !(row.text && focused)
                                && hit.clicked_by(egui::PointerButton::Primary)
                            {
                                out.clicked = Some(i);
                            }
                        }
                    });
                    ctx.data_mut(|d| d.insert_temp(offset_id, list.state.offset.y));
                    for galley in footer {
                        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                        ui.painter().galley(rect.min, galley, Color32::WHITE);
                    }
                });
        });
    out
}

/// Draws a page: `title`, `rows` with `cursor` focused, the text box of
/// the focused text row bound to `text` (given the keyboard when `focus`),
/// then `lines` (status, help).
pub fn draw_page(
    ctx: &egui::Context,
    title: &str,
    rows: &[PageRow],
    cursor: usize,
    text: Option<&mut String>,
    focus: bool,
    lines: &[(String, Color32)],
) -> PageDraw {
    let mut out = PageDraw::default();
    let mut text = text;
    egui::Area::new(egui::Id::new("lan_page"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(egui::Margin::same(18))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    let width = egui_units(ctx, 420.0);
                    ui.set_min_width(width);
                    ui.vertical_centered(|ui| {
                        ui.label(egui_text(ctx, &TITLE, tint::TITLE, title));
                    });
                    ui.add_space(8.0);
                    for (i, row) in rows.iter().enumerate() {
                        if row.heading {
                            ui.add_space(4.0);
                            ui.label(egui_text(ctx, &BODY, tint::HELP, &row.label));
                            continue;
                        }
                        let focused = i == cursor;
                        let fg = if !row.enabled {
                            tint::DIMMED
                        } else if focused {
                            tint::SELECTED
                        } else {
                            tint::UNSELECTED
                        };
                        let frame = egui::Frame::new()
                            .fill(if focused {
                                egui_color(FOCUS_BG)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .corner_radius(3.0)
                            .show(ui, |ui| {
                                ui.set_min_width(width - 20.0);
                                ui.horizontal(|ui| {
                                    ui.label(egui_text(ctx, &ITEM, fg, &row.label));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| match (row.text && focused, text.as_deref_mut()) {
                                            (true, Some(t)) => {
                                                let r = ui.add(
                                                    egui::TextEdit::singleline(t)
                                                        .font(egui_font(ctx, &ITEM))
                                                        .desired_width(egui_units(ctx, 180.0)),
                                                );
                                                if focus {
                                                    r.request_focus();
                                                }
                                                if r.lost_focus()
                                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                                {
                                                    out.submitted = true;
                                                }
                                                out.text_focus |= r.has_focus();
                                            }
                                            _ => {
                                                if let Some(v) = &row.value {
                                                    ui.label(egui_text(ctx, &ITEM, fg, v));
                                                }
                                            }
                                        },
                                    );
                                });
                            });
                        // A text row's box takes its own clicks.
                        if !row.text {
                            let hit = frame.response.interact(egui::Sense::click());
                            if row.enabled && hit.clicked_by(egui::PointerButton::Primary) {
                                out.clicked = Some(i);
                            }
                            if row.enabled && hit.clicked_by(egui::PointerButton::Secondary) {
                                out.secondary = Some(i);
                            }
                        }
                    }
                    ui.add_space(8.0);
                    for (line, color) in lines {
                        ui.label(egui_text(ctx, &BODY, tint::HELP, line).color(*color));
                    }
                });
        });
    out
}

/// One player in the lobby's roster panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterRow {
    pub slot: u8,
    pub name: String,
    /// `None` for the host itself.
    pub ping_ms: Option<u32>,
    pub addon: bool,
    pub team: String,
    pub ready: bool,
}

/// `[ours]` The roster panel's own input (issue #168, the first VPN
/// playtest: the host could neither close the list nor reach its Kick with a
/// controller). The panel shows at the lobby's start; the shell's Square
/// (Xbox X, Space) moves into it, opening it if it was closed. In the list
/// Up / Down move between the players, Accept on a guest asks to kick him
/// and a second Accept kicks; Back cancels that question, else closes the
/// list (hidden), as Square does. While the list holds the input the
/// screen under it gets none. The mouse keeps its Kick buttons and has a
/// Hide / Players button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RosterNav {
    /// The list is closed: only its "Players" tab shows.
    pub hidden: bool,
    /// The row the pad is on while the list holds the input.
    pub focus: Option<usize>,
    /// The guest (by slot) whose kick waits for a second Accept. It holds
    /// only while the pad is on his row: a roster that shifts under the
    /// pad (he left) drops the question rather than hand it to the next.
    pub confirm: Option<u8>,
}

impl RosterNav {
    /// Whether the list takes this frame's menu input (the screen under it
    /// must not see it).
    pub fn active(&self) -> bool {
        self.focus.is_some()
    }

    /// Gives the input back and drops any question (the lobby is entered
    /// again, after a round); an open or closed list stays so.
    pub fn release(&mut self) {
        self.focus = None;
        self.confirm = None;
    }

    /// The focused row while its kick waits for a second Accept.
    pub fn asking<'a>(&self, rows: &'a [RosterRow]) -> Option<&'a RosterRow> {
        let r = rows.get(self.focus?)?;
        (self.confirm == Some(r.slot)).then_some(r)
    }

    /// One frame of the pad and keys: `toggle` is the Square edge. Returns
    /// the slot to kick (only with `kick`, the host's, never the host's own
    /// row). Call it before the screen under the panel steps, and give that
    /// screen no input when it returns with [`RosterNav::active`] or when
    /// `toggle` was pressed.
    pub fn input(
        &mut self,
        toggle: bool,
        m: MenuInput,
        rows: &[RosterRow],
        kick: bool,
    ) -> Option<u8> {
        let Some(at) = self.focus else {
            if toggle && !rows.is_empty() {
                *self = Self {
                    hidden: false,
                    focus: Some(0),
                    confirm: None,
                };
            } else if toggle {
                self.hidden = !self.hidden;
            }
            return None;
        };
        if rows.is_empty() {
            *self = Self::default();
            return None;
        }
        let at = at.min(rows.len() - 1);
        self.focus = Some(at);
        if self.confirm != Some(rows[at].slot) {
            self.confirm = None;
        }
        if toggle || (m.back && self.confirm.is_none()) {
            *self = Self {
                hidden: true,
                focus: None,
                confirm: None,
            };
            return None;
        }
        if m.back {
            self.confirm = None;
            return None;
        }
        let n = rows.len();
        if m.up || m.down {
            let step = if m.up { n - 1 } else { 1 };
            self.focus = Some((at + step) % n);
            self.confirm = None;
            return None;
        }
        if m.confirm && kick && rows[at].slot != 0 {
            if self.confirm.is_some() {
                self.confirm = None;
                return Some(rows[at].slot);
            }
            self.confirm = Some(rows[at].slot);
        }
        None
    }
}

/// The roster panel's prompt words for the set in use: the button that
/// opens the list, Accept and Back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RosterPrompts {
    pub open: String,
    pub accept: String,
    pub back: String,
}

impl RosterPrompts {
    /// The shell's buttons as `icons` names them; the lobby's Back is
    /// Circle / B, or Backspace.
    pub fn of(icons: &crate::input_icons::Icons) -> Self {
        use crate::input_icons::{pad_name, IconSet, PadButton, Prompt};
        Self {
            open: icons.text(Prompt::Done).to_owned(),
            accept: icons.text(Prompt::Accept).to_owned(),
            back: if icons.set == IconSet::KeyboardMouse {
                "Backspace".to_owned()
            } else {
                pad_name(icons.set, PadButton::East).to_owned()
            },
        }
    }

    /// The help line under the list for `nav`, `kick` the host's.
    pub fn line(&self, nav: &RosterNav, rows: &[RosterRow], kick: bool) -> String {
        let Some(at) = nav.focus else {
            return format!("{}: players list", self.open);
        };
        let guest = rows.get(at).filter(|r| kick && r.slot != 0);
        match guest {
            Some(r) if nav.confirm == Some(r.slot) => format!(
                "Kick {}? {}: kick \u{b7} {}: cancel",
                r.name, self.accept, self.back
            ),
            Some(_) => format!(
                "Up / Down: move \u{b7} {}: kick \u{b7} {}: close",
                self.accept, self.back
            ),
            None => format!("Up / Down: move \u{b7} {}: close", self.back),
        }
    }
}

/// Draws the lobby's roster panel (top right); with `kick`, every guest's
/// row has a Kick button. `nav` is the pad's place in it (its focused row
/// is marked, a closed list is a "Players" tab) and its Hide / Players
/// button sets it. Returns the slot whose Kick was clicked.
pub fn draw_roster(
    ctx: &egui::Context,
    title: &str,
    rows: &[RosterRow],
    kick: bool,
    lines: &[String],
    nav: &mut RosterNav,
    prompts: &RosterPrompts,
) -> Option<u8> {
    let mut kicked = None;
    egui::Area::new(egui::Id::new("lan_roster"))
        .anchor(Align2::RIGHT_TOP, Vec2::new(-12.0, 12.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(190))
                .inner_margin(egui::Margin::same(10))
                .corner_radius(5.0)
                .show(ui, |ui| {
                    if nav.hidden {
                        let label = format!("Players ({}) \u{b7} {}", rows.len(), prompts.open);
                        if ui
                            .button(egui_text(ctx, &BODY, tint::UNSELECTED, &label))
                            .clicked()
                        {
                            nav.hidden = false;
                        }
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.label(egui_text(ctx, &TITLE, tint::TITLE, title));
                        if ui
                            .button(egui_text(ctx, &BODY, tint::UNSELECTED, "Hide"))
                            .clicked()
                        {
                            *nav = RosterNav {
                                hidden: true,
                                ..RosterNav::default()
                            };
                        }
                    });
                    egui::Grid::new("lan_roster_grid")
                        .striped(true)
                        .spacing([egui_units(ctx, 8.0), egui_units(ctx, 2.0)])
                        .show(ui, |ui| {
                            for h in ["Player", "Ping", "Addon", "Team", ""] {
                                ui.label(egui_text(ctx, &LIST, tint::DIMMED, h));
                            }
                            ui.end_row();
                            for (k, r) in rows.iter().enumerate() {
                                let focused = nav.focus == Some(k);
                                let name = match (focused, r.ready) {
                                    (true, true) => format!("\u{25b6} {} \u{2713}", r.name),
                                    (true, false) => format!("\u{25b6} {}", r.name),
                                    (false, true) => format!("{} \u{2713}", r.name),
                                    (false, false) => r.name.clone(),
                                };
                                let tint = if focused {
                                    tint::SELECTED
                                } else {
                                    tint::UNSELECTED
                                };
                                let cell = |t: &str| egui_text(ctx, &LIST, tint, t);
                                let mut text = cell(&name);
                                if focused {
                                    text = text.background_color(egui_color(FOCUS_BG));
                                }
                                ui.label(text);
                                ui.label(cell(
                                    &r.ping_ms
                                        .map_or_else(|| "host".into(), |p| format!("{p} ms")),
                                ));
                                ui.label(cell(if r.addon { "Yes" } else { "No" }));
                                ui.label(cell(&r.team));
                                if kick && r.slot != 0 {
                                    let armed = focused && nav.confirm == Some(r.slot);
                                    let button =
                                        egui::Button::new(cell("Kick")).selected(armed || focused);
                                    if ui.add(button).clicked() {
                                        kicked = Some(r.slot);
                                    }
                                } else {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        });
                    let help = prompts.line(nav, rows, kick);
                    let color = if nav.asking(rows).is_some() {
                        tint::WARNING
                    } else {
                        tint::HELP
                    };
                    ui.label(egui_text(ctx, &BODY, color, &help));
                    for l in lines {
                        ui.label(egui_text(ctx, &BODY, tint::TITLE, l));
                    }
                });
        });
    kicked
}

/// Draws a status line near the bottom of the screen (the guest's "The
/// host is choosing…", "Waiting for other players").
pub fn draw_status(ctx: &egui::Context, text: &str) {
    // Above the help lines, clear of the roster at the top right.
    egui::Area::new(egui::Id::new("lan_status"))
        .interactable(false)
        .anchor(
            Align2::CENTER_BOTTOM,
            Vec2::new(0.0, -egui_units(ctx, 64.0)),
        )
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(egui::Margin::symmetric(16, 8))
                .corner_radius(5.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(egui_text(ctx, &ITEM, tint::TITLE, text))
                            .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
        });
}

/// The Player name text box over the Game Options page: returns
/// `Some(true)` when Enter accepts it, `Some(false)` when Escape cancels.
pub fn draw_name_box(ctx: &egui::Context, name: &mut String, error: Option<&str>) -> Option<bool> {
    let mut out = None;
    egui::Area::new(egui::Id::new("lan_name_box"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(230))
                .inner_margin(egui::Margin::same(18))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.label(egui_text(ctx, &TITLE, tint::TITLE, "Player Name"));
                    let r = ui.add(
                        egui::TextEdit::singleline(name)
                            .font(egui_font(ctx, &ITEM))
                            .char_limit(crate::lobby::NAME_CHARS)
                            .desired_width(egui_units(ctx, 240.0)),
                    );
                    if !r.has_focus() && !r.lost_focus() {
                        r.request_focus();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        out = Some(false);
                    } else if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        out = Some(true);
                    }
                    if let Some(e) = error {
                        ui.label(egui_text(ctx, &BODY, tint::WARNING, e));
                    }
                    ui.label(egui_text(
                        ctx,
                        &BODY,
                        tint::HELP,
                        "Enter keeps it, Escape cancels. Other players see this name.",
                    ));
                });
        });
    out
}

/// The longest player name (the net crate's limit, `net::msg::MAX_NAME_CHARS`).
pub const NAME_CHARS: usize = 24;

#[cfg(test)]
mod tests;
