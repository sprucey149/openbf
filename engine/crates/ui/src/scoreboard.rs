//! `[ours]` The in-battle scoreboard (LAN §5b, issue #168): both teams'
//! soldiers, players and bots, with their points, kills and deaths, and a
//! ping column for the humans of a multiplayer round. It reads the same
//! numbers as the round-end Team Stats page (`game::round_stats`) and is
//! drawn as an overlay that pauses nothing.
//!
//! Rules:
//! - It shows while Select / View (a controller) or Tab (the keyboard) is
//!   held. Select is the pad's `SPAWN_CLASS_PREV` bit in battle: its hold
//!   shows the board after [`SHOW_AFTER`] seconds, so the vehicle seat chord
//!   (Select with Switch Weapon) never flashes it, and a Switch Weapon press
//!   during the hold hides it until the hold ends ([`Hold`]).
//! - Tab is read as the key itself, not through a binding: it shows the
//!   board at once and in every control context, whatever layout the player
//!   saved unless that layout binds an action to Tab, and a held Tab keeps
//!   the board up through a weapon or seat switch (Tab+Q still picks the
//!   previous seat; the keyboard deliberately does not hide the board for
//!   the seat chord; issue #182).
//! - The caller does not draw it on the spawn screens (Select is the
//!   previous class there), under the pause menu or at the round's end.
//! - Rows are sorted as Team Stats sorts them ([`crate::stats::sort_rows`]'s
//!   order), the viewer's team on the left.

use crate::stats::SHOWN;
use egui::{Align2, Color32, RichText};

/// How long the button must be held before the board shows (seconds).
pub const SHOW_AFTER: f32 = 0.15;
/// Rows a team's column shows at most (two Team Stats pages).
pub const MAX_ROWS: usize = SHOWN * 2 + 14;

/// The scoreboard's hold: the button's state frame by frame (module rules).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hold {
    held_for: f32,
    /// Switch Weapon was pressed during this hold: the seat chord.
    chord: bool,
    shown: bool,
}

impl Hold {
    /// One frame: whether the pad's board button (`select`), Switch Weapon
    /// and the keyboard's board key (`key`) are held, and the frame's
    /// seconds. Returns whether the board shows.
    pub fn step(&mut self, select: bool, switch_weapon: bool, key: bool, dt: f32) -> bool {
        let pad = if select {
            self.chord |= switch_weapon;
            self.held_for += dt.max(0.0);
            !self.chord && self.held_for >= SHOW_AFTER
        } else {
            self.chord = false;
            self.held_for = 0.0;
            false
        };
        self.shown = key || pad;
        self.shown
    }

    /// The last step's answer.
    pub fn shown(&self) -> bool {
        self.shown
    }
}

/// The ping cell of a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ping {
    /// A bot, or a round without a network: blank.
    #[default]
    None,
    /// The host's own player.
    Host,
    /// A guest's round trip to the host, in milliseconds.
    Ms(u32),
}

/// One soldier's row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScoreRow {
    pub name: String,
    pub points: i32,
    pub kills: i32,
    pub deaths: i32,
    pub ping: Ping,
    /// A human player (bots are drawn dimmer).
    pub human: bool,
    /// The viewer's own row.
    pub me: bool,
}

/// One team's column.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScoreTeam {
    pub title: String,
    /// The viewer's team (friend colour) or the other (enemy colour).
    pub friend: bool,
    pub rows: Vec<ScoreRow>,
}

/// The board: the viewer's team first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scoreboard {
    pub teams: [ScoreTeam; 2],
    /// A multiplayer round: the Ping column is drawn.
    pub ping_column: bool,
}

/// Team Stats' order: points down, kills down, deaths up; equal rows keep
/// their order.
pub fn sort_rows(rows: &mut [ScoreRow]) {
    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.kills.cmp(&a.kills))
            .then(a.deaths.cmp(&b.deaths))
    });
}

impl Scoreboard {
    /// The board from the two teams' rows (team 0 first) and the viewer's
    /// team: rows sorted, the viewer's team on the left.
    pub fn new(mut teams: [ScoreTeam; 2], viewer_team: u8, ping_column: bool) -> Self {
        for (i, t) in teams.iter_mut().enumerate() {
            sort_rows(&mut t.rows);
            t.friend = i == usize::from(viewer_team.min(1));
        }
        if viewer_team == 1 {
            teams.swap(0, 1);
        }
        Self { teams, ping_column }
    }
}

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// The ping cell's text.
pub fn ping_text(p: Ping) -> String {
    match p {
        Ping::None => String::new(),
        Ping::Host => "host".into(),
        Ping::Ms(ms) => ms.to_string(),
    }
}

/// Draws the board centred, under the top of the screen.
pub fn draw_scoreboard(ctx: &egui::Context, b: &Scoreboard) {
    use egui::{Area, Frame, Grid, Id, Margin};
    let [enemy, _, friend] = crate::hud::RETICLE_COLORS;
    Area::new(Id::new("scoreboard"))
        .interactable(false)
        .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(190))
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for (k, team) in b.teams.iter().enumerate() {
                            if k == 1 {
                                ui.add_space(28.0);
                            }
                            let tint = color(if team.friend { friend } else { enemy });
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&team.title)
                                        .font(crate::typography::egui_font(
                                            ctx,
                                            &crate::typography::ITEM,
                                        ))
                                        .color(tint),
                                );
                                Grid::new(("scoreboard_team", k))
                                    .num_columns(if b.ping_column { 5 } else { 4 })
                                    .spacing([16.0, 2.0])
                                    .show(ui, |ui| {
                                        let head = |t: &str| {
                                            crate::typography::egui_text(
                                                ctx,
                                                &crate::typography::LIST,
                                                crate::typography::tint::NOTE,
                                                t,
                                            )
                                        };
                                        ui.label(head("Name"));
                                        ui.label(head("Points"));
                                        ui.label(head("Kills"));
                                        ui.label(head("Deaths"));
                                        if b.ping_column {
                                            ui.label(head("Ping"));
                                        }
                                        ui.end_row();
                                        for r in team.rows.iter().take(MAX_ROWS) {
                                            let c = if r.me {
                                                crate::typography::tint::ROW_ME
                                            } else if r.human {
                                                crate::typography::tint::ROW_HUMAN
                                            } else {
                                                crate::typography::tint::ROW_BOT
                                            };
                                            let cell = |t: String| {
                                                crate::typography::egui_text(
                                                    ctx,
                                                    &crate::typography::LIST,
                                                    c,
                                                    &t,
                                                )
                                            };
                                            ui.label(cell(r.name.clone()));
                                            ui.label(cell(r.points.to_string()));
                                            ui.label(cell(r.kills.to_string()));
                                            ui.label(cell(r.deaths.to_string()));
                                            if b.ping_column {
                                                ui.label(cell(ping_text(r.ping)));
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                        }
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn hold(h: &mut Hold, frames: usize, select: bool, switch: bool) -> bool {
        (0..frames).fold(false, |_, _| h.step(select, switch, false, DT))
    }

    /// Held, the board shows after the short delay and hides on release.
    #[test]
    fn scoreboard_hold_shows_after_the_delay() {
        let mut h = Hold::default();
        assert!(!hold(&mut h, 5, true, false), "not at once");
        assert!(hold(&mut h, 10, true, false));
        assert!(h.shown());
        assert!(!h.step(false, false, false, DT), "release hides it");
        assert!(!hold(&mut h, 3, false, false));
    }

    /// The vehicle seat chord (Select with Switch Weapon) never shows it,
    /// pressed at once or during the hold, until the button is let go.
    #[test]
    fn scoreboard_hold_chord_hides_it() {
        let mut h = Hold::default();
        assert!(!h.step(true, true, false, DT), "a key bound to both");
        assert!(!hold(&mut h, 30, true, false), "the chord spoils the hold");
        let mut h = Hold::default();
        assert!(hold(&mut h, 20, true, false));
        assert!(
            !h.step(true, true, false, DT),
            "Switch Weapon mid-hold hides it"
        );
        assert!(!hold(&mut h, 30, true, false));
        h.step(false, false, false, DT);
        assert!(hold(&mut h, 20, true, false), "a new hold shows it again");
    }

    /// Tab (issue #182): shown on its first frame, with or without the pad
    /// bit a stock table also gives it, and a weapon switch while it is held
    /// (the wheel's Switch Weapon and Select bits, or Q's) never hides it.
    #[test]
    fn scoreboard_tab_shows_at_once_and_survives_a_weapon_switch() {
        let mut h = Hold::default();
        assert!(
            h.step(false, false, true, DT),
            "a custom layout: no pad bit"
        );
        let mut h = Hold::default();
        assert!(
            h.step(true, false, true, DT),
            "the stock table's pad bit too"
        );
        assert!(h.step(true, true, true, DT), "wheel up mid-hold");
        assert!(h.step(true, false, true, DT));
        assert!(!h.step(false, false, false, DT), "release hides it");
    }

    fn row(name: &str, points: i32, kills: i32, deaths: i32, ping: Ping) -> ScoreRow {
        ScoreRow {
            name: name.into(),
            points,
            kills,
            deaths,
            human: ping != Ping::None,
            ping,
            me: false,
        }
    }

    /// Team Stats' order, the viewer's team on the left, pings kept.
    #[test]
    fn scoreboard_model_sorts_and_puts_the_viewer_left() {
        let teams = [
            ScoreTeam {
                title: "Republic".into(),
                rows: vec![
                    row("Bot A", 3, 1, 0, Ping::None),
                    row("the maintainer", 5, 2, 1, Ping::Host),
                ],
                ..Default::default()
            },
            ScoreTeam {
                title: "CIS".into(),
                rows: vec![
                    row("playtest_1", 4, 2, 2, Ping::Ms(48)),
                    row("Bot B", 4, 2, 1, Ping::None),
                ],
                ..Default::default()
            },
        ];
        let b = Scoreboard::new(teams, 1, true);
        assert_eq!(b.teams[0].title, "CIS");
        assert!(b.teams[0].friend && !b.teams[1].friend);
        let names: Vec<&str> = b.teams[0].rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["Bot B", "playtest_1"],
            "equal points: fewer deaths first"
        );
        assert_eq!(b.teams[1].rows[0].name, "the maintainer");
        assert_eq!(ping_text(b.teams[0].rows[1].ping), "48");
        assert_eq!(ping_text(b.teams[1].rows[0].ping), "host");
        assert_eq!(ping_text(b.teams[0].rows[0].ping), "");
    }
}
