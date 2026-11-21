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
use egui::{Align2, Color32};

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

// `[ours]` (maintainer, 2026-11-19): the board fits the window at any
// size and display scale. Its gaps are shell units, so they follow the text;
// the number columns keep their measured width; the names share what is
// left, and a name that does not fit is cut with "..." at the same text size;
// only the rows that fit are shown, always with the viewer's own.
/// The space kept clear of every screen edge (shell units).
const GUTTER: f32 = 8.0;
/// The board's top below the top of the screen (shell units).
const TOP: f32 = 20.0;
/// The frame's inner margin (shell units), across and down.
const MARGIN: [f32; 2] = [6.0, 4.0];
/// Between two columns, and its floor on a narrow window (shell units).
const COLUMN_GAP: f32 = 6.0;
const COLUMN_GAP_MIN: f32 = 2.0;
/// Between the two teams (shell units), and its floor.
const TEAM_GAP: f32 = 12.0;
const TEAM_GAP_MIN: f32 = 4.0;
/// Between two rows (shell units).
const ROW_GAP: f32 = 1.0;
/// The cut name's mark.
const ELLIPSIS: &str = "...";

/// The rows of a team the board shows when `fit` fit: the first ones in
/// Team Stats' order, the viewer's row taking the last place when it would
/// fall below the cut.
pub fn shown_rows(rows: &[ScoreRow], fit: usize) -> Vec<&ScoreRow> {
    let n = rows.len().min(fit).min(MAX_ROWS);
    let mut out: Vec<&ScoreRow> = rows[..n].iter().collect();
    if let Some(me) = rows.iter().position(|r| r.me) {
        if me >= n && n > 0 {
            out[n - 1] = &rows[me];
        }
    }
    out
}

/// `text` cut to `room` points wide in `font` (with [`ELLIPSIS`]) when it
/// is wider.
fn fit_text(painter: &egui::Painter, text: &str, font: &egui::FontId, room: f32) -> String {
    let width = |t: &str| {
        painter
            .layout_no_wrap(t.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if width(text) <= room {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    for n in (1..chars.len()).rev() {
        let cut: String = chars[..n].iter().collect::<String>().trim_end().to_owned() + ELLIPSIS;
        if width(&cut) <= room {
            return cut;
        }
    }
    if width(ELLIPSIS) <= room {
        ELLIPSIS.to_owned()
    } else {
        String::new()
    }
}

/// Draws the board centred under the top of the screen, inside it.
pub fn draw_scoreboard(ctx: &egui::Context, b: &Scoreboard) {
    use crate::typography::{egui_color, egui_font, egui_units, tint, ITEM, LIST};
    use egui::{pos2, vec2, Id, LayerId, Order, Rect};
    let [enemy, _, friend] = crate::hud::RETICLE_COLORS;
    let screen = ctx.content_rect();
    let u = egui_units(ctx, 1.0);
    let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("scoreboard")));
    let row_font = egui_font(ctx, &LIST);
    let title_font = egui_font(ctx, &ITEM);
    let case = |t: &str| LIST.case.apply(t).into_owned();
    let width = |t: &str, f: &egui::FontId| {
        painter
            .layout_no_wrap(t.to_owned(), f.clone(), Color32::WHITE)
            .size()
            .x
    };
    let row_h = painter
        .layout_no_wrap("Ag".into(), row_font.clone(), Color32::WHITE)
        .size()
        .y;
    let title_h = painter
        .layout_no_wrap("Ag".into(), title_font.clone(), Color32::WHITE)
        .size()
        .y;

    // The number columns: their header, a four-digit value or the widest
    // value shown, whichever is wider.
    let mut heads = vec!["Points", "Kills", "Deaths"];
    if b.ping_column {
        heads.push("Ping");
    }
    let rows = || b.teams.iter().flat_map(|t| t.rows.iter());
    let numbers: Vec<f32> = heads
        .iter()
        .enumerate()
        .map(|(c, h)| {
            let widest = rows()
                .map(|r| match c {
                    0 => width(&r.points.to_string(), &row_font),
                    1 => width(&r.kills.to_string(), &row_font),
                    2 => width(&r.deaths.to_string(), &row_font),
                    _ => width(&case(&ping_text(r.ping)), &row_font),
                })
                .fold(0.0, f32::max);
            width(&case(h), &row_font)
                .max(width("9999", &row_font))
                .max(widest)
        })
        .collect();
    let fixed: f32 = numbers.iter().sum();
    let natural_name = rows()
        .map(|r| width(&case(&r.name), &row_font))
        .fold(width(&case("Name"), &row_font), f32::max);
    let min_name = width(&case("MMM"), &row_font) + width(ELLIPSIS, &row_font);
    let room = (screen.width() - 2.0 * GUTTER * u).max(0.0);
    let cols = heads.len() as f32;
    let board_w = |name: f32, gap: f32, team_gap: f32| {
        2.0 * (name + fixed + cols * gap) + team_gap + 2.0 * MARGIN[0] * u
    };
    // Full gaps first; on a narrow window the gaps close to their floor
    // before a name is cut below `min_name`.
    let (mut gap, mut team_gap) = (COLUMN_GAP * u, TEAM_GAP * u);
    let name_room = |gap: f32, team_gap: f32| {
        (room - 2.0 * MARGIN[0] * u - team_gap) / 2.0 - fixed - cols * gap
    };
    if name_room(gap, team_gap) < min_name.min(natural_name) {
        gap = COLUMN_GAP_MIN * u;
        team_gap = TEAM_GAP_MIN * u;
    }
    let name_w = natural_name.min(name_room(gap, team_gap)).max(0.0);
    let total_w = board_w(name_w, gap, team_gap).min(room.max(0.0));
    let team_w = (total_w - 2.0 * MARGIN[0] * u - team_gap) / 2.0;

    // Rows: as many as fit below the title and the header.
    let line = row_h + ROW_GAP * u;
    let top = screen.top() + TOP * u;
    let below =
        (screen.bottom() - GUTTER * u - top - 2.0 * MARGIN[1] * u - title_h - line).max(0.0);
    let fit = ((below / line).floor() as usize).max(1);
    let shown: [Vec<&ScoreRow>; 2] = [
        shown_rows(&b.teams[0].rows, fit),
        shown_rows(&b.teams[1].rows, fit),
    ];
    let n = shown[0].len().max(shown[1].len());
    let total_h = 2.0 * MARGIN[1] * u + title_h + line * (n as f32 + 1.0);
    let left = (screen.center().x - total_w / 2.0).max(screen.left());
    let frame = Rect::from_min_size(pos2(left, top), vec2(total_w, total_h));
    painter.rect_filled(frame, 0.0, Color32::from_black_alpha(190));

    let clip = frame.intersect(screen);
    let painter = painter.with_clip_rect(clip);
    for (k, team) in b.teams.iter().enumerate() {
        let x0 = frame.left() + MARGIN[0] * u + k as f32 * (team_w + team_gap);
        let mut y = frame.top() + MARGIN[1] * u;
        let tint_team = color(if team.friend { friend } else { enemy });
        let title = fit_text(&painter, &team.title, &title_font, team_w);
        painter.text(
            pos2(x0, y),
            Align2::LEFT_TOP,
            title,
            title_font.clone(),
            tint_team,
        );
        y += title_h;
        // A row: the name left in its column, the numbers right-aligned in
        // theirs.
        let draw_row = |y: f32, name: &str, cells: &[String], c: Color32| {
            let name = fit_text(&painter, &case(name), &row_font, name_w);
            painter.text(pos2(x0, y), Align2::LEFT_TOP, name, row_font.clone(), c);
            let mut x = x0 + name_w;
            for (cell, w) in cells.iter().zip(&numbers) {
                x += gap + w;
                painter.text(
                    pos2(x, y),
                    Align2::RIGHT_TOP,
                    case(cell),
                    row_font.clone(),
                    c,
                );
            }
        };
        let head: Vec<String> = heads.iter().map(|h| h.to_string()).collect();
        draw_row(y, "Name", &head, egui_color(tint::NOTE));
        for r in &shown[k] {
            y += line;
            let c = if r.me {
                tint::ROW_ME
            } else if r.human {
                tint::ROW_HUMAN
            } else {
                tint::ROW_BOT
            };
            let mut cells = vec![
                r.points.to_string(),
                r.kills.to_string(),
                r.deaths.to_string(),
            ];
            if b.ping_column {
                cells.push(ping_text(r.ping));
            }
            draw_row(y, &r.name, &cells, egui_color(c));
        }
    }
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

    /// 32 rows a side with 20-character names, the viewer's row last (the
    /// cut must keep it).
    fn crowded(ping: bool) -> Scoreboard {
        let team = |t: u8| ScoreTeam {
            title: if t == 0 {
                "Galactic Republic"
            } else {
                "Confederacy of Independent Systems"
            }
            .into(),
            rows: (0..32)
                .map(|i| ScoreRow {
                    name: if i == 31 && t == 0 {
                        "the maintainer".into()
                    } else if i % 2 == 0 {
                        format!("WWWWWWWWWWWWWWWWWW{i:02}")
                    } else {
                        format!("Commander_Bacara_{i:03}")
                    },
                    points: 120 - i,
                    kills: 99 - i,
                    deaths: i,
                    ping: if ping && i % 3 == 0 {
                        Ping::Ms(1000 + i as u32)
                    } else {
                        Ping::None
                    },
                    human: i % 3 == 0,
                    me: i == 31 && t == 0,
                })
                .collect(),
            friend: false,
        };
        Scoreboard::new([team(0), team(1)], 0, ping)
    }

    /// `[ours]` (2026-11-19): at every common window size in points
    /// (1280x720 to 4K, and 1920x1080 / 2560x1440 / 4K at 125-150 %
    /// Windows scale, which egui sees as fewer points), with 32 rows a side,
    /// 20-character names and the Ping column on and off: every text lies
    /// inside the window and its clip, no two cells of a line overlap, and
    /// the viewer's row is drawn. Egui's own face stands in for the baked
    /// one (the layout measures whichever face is installed).
    #[test]
    fn scoreboard_fits_every_window_and_keeps_the_viewers_row() {
        let sizes = [
            (640.0, 480.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1280.0, 800.0),
            (1366.0, 768.0),
            (1440.0, 1080.0),
            (1536.0, 864.0),
            (1680.0, 1050.0),
            (1920.0, 1080.0),
            (1920.0, 1200.0),
            (2048.0, 1152.0),
            (2560.0, 1080.0),
            (2560.0, 1440.0),
            (3440.0, 1440.0),
            (3840.0, 2160.0),
        ];
        for (w, h) in sizes {
            for ping in [false, true] {
                let b = crowded(ping);
                let ctx = egui::Context::default();
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                };
                let _ = ctx.run(input.clone(), |ctx| draw_scoreboard(ctx, &b));
                let out = ctx.run(input, |ctx| draw_scoreboard(ctx, &b));
                let mut texts = Vec::new();
                for c in &out.shapes {
                    match &c.shape {
                        egui::Shape::Text(t) => {
                            let r = egui::Rect::from_min_size(t.pos, t.galley.size());
                            let inside = |outer: egui::Rect| {
                                r.min.x >= outer.min.x - 0.5
                                    && r.min.y >= outer.min.y - 0.5
                                    && r.max.x <= outer.max.x + 0.5
                                    && r.max.y <= outer.max.y + 0.5
                            };
                            assert!(
                                inside(screen) && inside(c.clip_rect),
                                "{w}x{h} ping {ping}: {:?} at {r:?}",
                                t.galley.text()
                            );
                            texts.push((r, t.galley.text().to_owned()));
                        }
                        egui::Shape::Rect(f) => {
                            assert!(screen.contains_rect(f.rect), "{w}x{h}: frame {:?}", f.rect)
                        }
                        _ => {}
                    }
                }
                assert!(
                    texts.iter().any(|(_, t)| t == "THE MAINTAINER"),
                    "{w}x{h} ping {ping}: the viewer's row"
                );
                assert!(
                    texts.iter().any(|(_, t)| t == "DEATHS"),
                    "{w}x{h}: the header"
                );
                if ping {
                    assert!(
                        texts.iter().any(|(_, t)| t == "PING"),
                        "{w}x{h}: the ping header"
                    );
                }
                // No two cells of one line overlap.
                texts.sort_by(|a, b| {
                    (a.0.min.y.round() as i32, a.0.min.x as i32)
                        .cmp(&(b.0.min.y.round() as i32, b.0.min.x as i32))
                });
                for pair in texts.windows(2) {
                    let (a, b) = (&pair[0], &pair[1]);
                    if (a.0.min.y - b.0.min.y).abs() < 0.5 {
                        assert!(
                            a.0.max.x <= b.0.min.x + 0.5,
                            "{w}x{h} ping {ping}: {:?} runs into {:?}",
                            a.1,
                            b.1
                        );
                    }
                }
            }
        }
    }

    /// The cut keeps the first rows and puts the viewer's row in the last
    /// place; a board that fits shows every row.
    #[test]
    fn scoreboard_rows_cut_keeps_the_viewer() {
        let b = crowded(false);
        let rows = &b.teams[0].rows;
        let shown = shown_rows(rows, 10);
        assert_eq!(shown.len(), 10);
        assert!(shown[9].me);
        assert_eq!(shown[..9], rows.iter().take(9).collect::<Vec<_>>()[..]);
        assert_eq!(shown_rows(rows, 99).len(), 32);
        assert!(shown_rows(&b.teams[1].rows, 10).iter().all(|r| !r.me));
    }
}
