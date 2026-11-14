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
//! - Join: the hosts the LAN browser heard (a host on another build is
//!   listed dimmed with why), a Join-by-address text box (IP or host name,
//!   optional `:port`) and the recent hosts. Focus follows a host by its
//!   address when the list changes under it.
//! - Text rows take the keyboard while focused: the shell's Back key does
//!   not leave the page while one types (the caller drops the menu keys
//!   while [`PageDraw::text_focus`] is set; Escape gives the focus back).
//!   Arriving on a text row (a step, or Join Game's first frame) focuses
//!   its box once ([`HostSetup::focus_text`]); a click in the box does too.
//! - Steps wrap last <-> first and skip dimmed rows, as the shell's stacks
//!   do; a move clicks select, a press accept, a refused press error.

use crate::menu::MenuInput;
use crate::shell_feel::MenuSound;
use egui::{Align2, Color32, RichText, Vec2};

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
}

impl PageRow {
    fn plain(label: impl Into<String>, value: Option<String>) -> Self {
        Self {
            label: label.into(),
            value,
            enabled: true,
            text: false,
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
    Some(if on { "On" } else { "Off" }.into())
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
        v.extend([
            HostRow::Awards,
            HostRow::Start,
            HostRow::Back,
        ]);
        v
    }

    pub fn rows(&self) -> Vec<PageRow> {
        self.row_ids()
            .into_iter()
            .map(|r| match r {
                HostRow::MaxPlayers => {
                    PageRow::plain("Max players", Some(self.max_players.to_string()))
                }
                HostRow::Port => PageRow {
                    text: true,
                    ..PageRow::plain("UDP port", Some(self.port.clone()))
                },
                HostRow::CheatPolicy => PageRow::plain(
                    "Cheats for guests",
                    Some(
                        if self.cheats_allowed {
                            "Allowed list"
                        } else {
                            "Off"
                        }
                        .into(),
                    ),
                ),
                HostRow::Cheat(i) => {
                    let (_, label, on) = self.cheats[i];
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
    pub map: String,
    pub players: String,
    pub ping_ms: u32,
    /// `None` = joinable, else why not (another build, full).
    pub refusal: Option<String>,
}

/// Join Game's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinRow {
    Host(usize),
    Address,
    /// "Test connection" for the typed address (LAN §5b).
    Test,
    Recent(usize),
    Back,
}

/// What Join Game asks of the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinCommand {
    /// Join this address (a found host's `ip:port`, the typed text or a
    /// recent host).
    Connect(String),
    /// Test the typed address: ask it the discovery question.
    Test(String),
    Back,
}

/// Join Game's page (module rules).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct JoinPage {
    pub hosts: Vec<FoundRow>,
    pub address: String,
    pub recent: Vec<String>,
    pub cursor: usize,
    /// The connection's state or why the last one ended.
    pub status: Option<String>,
    /// The cursor just arrived on the address box (see [`HostSetup::focus_text`]).
    pub focus_text: bool,
    clicks: Vec<MenuSound>,
}

impl JoinPage {
    pub fn new(recent: Vec<String>) -> Self {
        let mut p = Self {
            recent,
            ..Default::default()
        };
        p.cursor = p
            .row_ids()
            .iter()
            .position(|r| *r == JoinRow::Address)
            .unwrap_or(0);
        p.focus_text = true;
        p
    }

    pub fn row_ids(&self) -> Vec<JoinRow> {
        (0..self.hosts.len())
            .map(JoinRow::Host)
            .chain([JoinRow::Address, JoinRow::Test])
            .chain((0..self.recent.len()).map(JoinRow::Recent))
            .chain([JoinRow::Back])
            .collect()
    }

    pub fn rows(&self) -> Vec<PageRow> {
        self.row_ids()
            .into_iter()
            .map(|r| match r {
                JoinRow::Host(i) => {
                    let h = &self.hosts[i];
                    let mut label = format!("{} \u{2014} {}", h.name, h.map);
                    if let Some(why) = &h.refusal {
                        label = format!("{label} ({why})");
                    }
                    PageRow {
                        enabled: h.refusal.is_none(),
                        ..PageRow::plain(label, Some(format!("{}  {} ms", h.players, h.ping_ms)))
                    }
                }
                JoinRow::Address => PageRow {
                    text: true,
                    ..PageRow::plain("Join by address", Some(self.address.clone()))
                },
                JoinRow::Test => PageRow::plain("Test connection", None),
                JoinRow::Recent(i) => PageRow::plain(format!("Recent: {}", self.recent[i]), None),
                JoinRow::Back => PageRow::plain("Back", None),
            })
            .collect()
    }

    /// Replaces the found hosts; the focus stays on the same host (by
    /// address) or the same row below the list.
    pub fn set_hosts(&mut self, hosts: Vec<FoundRow>) {
        if hosts == self.hosts {
            return;
        }
        let ids = self.row_ids();
        let at = ids.get(self.cursor).copied();
        let old = self.hosts.len();
        let keep = match at {
            Some(JoinRow::Host(i)) => self.hosts.get(i).map(|h| h.addr.clone()),
            _ => None,
        };
        let new = hosts.len();
        self.hosts = hosts;
        self.cursor = match (at, keep) {
            (Some(JoinRow::Host(_)), Some(addr)) => self
                .hosts
                .iter()
                .position(|h| h.addr == addr)
                .unwrap_or(new),
            (Some(JoinRow::Host(_)), None) => new,
            _ => (self.cursor + new).saturating_sub(old),
        };
        let rows = self.rows();
        self.cursor = self.cursor.min(rows.len() - 1);
        if !rows[self.cursor].enabled {
            self.cursor = follow(&rows, self.cursor, 1);
        }
    }

    pub fn take_clicks(&mut self) -> Vec<MenuSound> {
        std::mem::take(&mut self.clicks)
    }

    /// One frame's presses.
    pub fn step(&mut self, input: MenuInput) -> Option<JoinCommand> {
        if input.back {
            self.clicks.push(MenuSound::Exit);
            return Some(JoinCommand::Back);
        }
        let rows = self.rows();
        if input.up || input.down {
            self.cursor = follow(&rows, self.cursor, if input.down { 1 } else { -1 });
            self.focus_text = rows[self.cursor].text;
            self.clicks.push(MenuSound::Select);
        }
        let ids = self.row_ids();
        self.cursor = self.cursor.min(ids.len() - 1);
        if !input.confirm {
            return None;
        }
        let addr = match ids[self.cursor] {
            JoinRow::Host(i) if self.hosts[i].refusal.is_none() => self.hosts[i].addr.clone(),
            JoinRow::Host(_) => String::new(),
            JoinRow::Address | JoinRow::Test => self.address.trim().to_owned(),
            JoinRow::Recent(i) => self.recent[i].clone(),
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
}

const FOCUS_BG: Color32 = Color32::from_rgb(76, 115, 178);

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
                    ui.set_min_width(520.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(title)
                                .size(28.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                    });
                    ui.add_space(8.0);
                    for (i, row) in rows.iter().enumerate() {
                        let focused = i == cursor;
                        let fg = if !row.enabled {
                            Color32::GRAY
                        } else if focused {
                            Color32::WHITE
                        } else {
                            Color32::LIGHT_GRAY
                        };
                        let frame = egui::Frame::new()
                            .fill(if focused {
                                FOCUS_BG
                            } else {
                                Color32::TRANSPARENT
                            })
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .corner_radius(3.0)
                            .show(ui, |ui| {
                                ui.set_min_width(500.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&row.label).size(18.0).color(fg));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| match (row.text && focused, text.as_deref_mut()) {
                                            (true, Some(t)) => {
                                                let r = ui.add(
                                                    egui::TextEdit::singleline(t)
                                                        .font(egui::FontId::proportional(18.0))
                                                        .desired_width(240.0),
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
                                                    ui.label(
                                                        RichText::new(v)
                                                            .size(18.0)
                                                            .color(fg)
                                                            .strong(),
                                                    );
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
                        }
                    }
                    ui.add_space(8.0);
                    for (line, color) in lines {
                        ui.label(RichText::new(line).size(16.0).color(*color));
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

/// Draws the lobby's roster panel (top right); with `kick`, every guest's
/// row has a Kick button. Returns the slot whose Kick was clicked.
pub fn draw_roster(
    ctx: &egui::Context,
    title: &str,
    rows: &[RosterRow],
    kick: bool,
    lines: &[String],
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
                    ui.label(
                        RichText::new(title)
                            .size(18.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );
                    egui::Grid::new("lan_roster_grid")
                        .striped(true)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            for h in ["Player", "Ping", "Addon", "Team", ""] {
                                ui.label(RichText::new(h).size(13.0).color(Color32::GRAY));
                            }
                            ui.end_row();
                            for r in rows {
                                let name = if r.ready {
                                    format!("{} \u{2713}", r.name)
                                } else {
                                    r.name.clone()
                                };
                                ui.label(RichText::new(name).size(15.0).color(Color32::WHITE));
                                ui.label(
                                    r.ping_ms
                                        .map_or_else(|| "host".into(), |p| format!("{p} ms")),
                                );
                                ui.label(if r.addon { "yes" } else { "no" });
                                ui.label(&r.team);
                                if kick && r.slot != 0 {
                                    if ui.button("Kick").clicked() {
                                        kicked = Some(r.slot);
                                    }
                                } else {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        });
                    for l in lines {
                        ui.label(
                            RichText::new(l)
                                .size(14.0)
                                .color(Color32::from_rgb(255, 210, 120)),
                        );
                    }
                });
        });
    kicked
}

/// Draws a line across the top of the screen (the guest's "The host is
/// choosing…", "Waiting for other players").
pub fn draw_status(ctx: &egui::Context, text: &str) {
    egui::Area::new(egui::Id::new("lan_status"))
        .interactable(false)
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 14.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(egui::Margin::symmetric(16, 8))
                .corner_radius(5.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .size(20.0)
                            .color(Color32::from_rgb(255, 210, 120)),
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
                    ui.label(
                        RichText::new("Player name")
                            .size(22.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );
                    let r = ui.add(
                        egui::TextEdit::singleline(name)
                            .font(egui::FontId::proportional(18.0))
                            .char_limit(crate::lobby::NAME_CHARS)
                            .desired_width(300.0),
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
                        ui.label(
                            RichText::new(e)
                                .size(15.0)
                                .color(Color32::from_rgb(255, 140, 120)),
                        );
                    }
                    ui.label(
                        RichText::new(
                            "Enter keeps it, Escape cancels. Other players see this name.",
                        )
                        .size(14.0)
                        .color(Color32::LIGHT_GRAY),
                    );
                });
        });
    out
}

/// The longest player name (the net crate's limit, `net::msg::MAX_NAME_CHARS`).
pub const NAME_CHARS: usize = 24;

#[cfg(test)]
mod tests;
