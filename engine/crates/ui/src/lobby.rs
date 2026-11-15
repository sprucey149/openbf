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
                    ..PageRow::plain("Join by Address", Some(self.address.clone()))
                },
                JoinRow::Test => PageRow::plain("Test Connection", None),
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

/// The focused row's bar: the native stack's white bar, faint.
const FOCUS_BG: crate::typography::Tint = ([255.0; 3], 0.14);

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
