//! The end of the round's stats screens as BF2's PS2 shows them (issue #50,
//! `analysis/re-round-end.md` §3.2, captures `engine/artifacts/lead_tmp/statsref/`):
//! the model and its buttons; [`crate::stats_draw`] lays the pages out.
//!
//! - **Team Stats** (`ifs_teamstats`): team 1's list left, team 2's right,
//!   whichever side the player is on; each row a soldier's name, points,
//!   kills, deaths and posts captured, sorted points down, then kills down,
//!   then deaths up (statsref README, four lists); 9 rows shown per list.
//!   The cursor starts on the player's row. Up / Down move it a row, L1 / R1
//!   to the first / last shown row (a page further when already there),
//!   Left / Right to the other list on the same shown row (wrapping);
//!   X opens the local player's Personal Stats; Square ("Done") quits; Triangle does
//!   nothing (`ifs_teamstats` main.f11-f13 [V]). The awards panel sits
//!   under the lists.
//! - **Personal Stats** of the local player: X opens Career Stats, Triangle
//!   goes back, Square quits. Up / Down move the highlight.
//! - **Career Stats**: the profile's numbers and the nine medals; Up / Down /
//!   Left / Right move the medal cursor, whose text shows below; Triangle
//!   goes back; X ("Done" on its help line) quits; Square quits too
//!   (`[ours]`: the career script's Square was not read). `[ours]` (card
//!   UNLK3): Down past the medals' last line turns the box to the unlockable
//!   classes' career kills, the same rows four a column; Up past their first
//!   line turns it back.
//!
//! Every text arrives already in the player's language (the game reads
//! BF2's table); numbers arrive as numbers and are printed here.

/// Rows a team list shows at once (statsref: 9 per list).
pub const SHOWN: usize = 9;
/// Medals per column of the career page (5 left, 4 right: statsref
/// `dag1_gcw/301_after_K.png`).
pub const MEDALS_LEFT: usize = 5;
/// The unlockable classes per column of their page (`[ours]`: 4 left, 4
/// right).
pub const UNLOCKS_LEFT: usize = 4;

/// One soldier's row on Team Stats, with his Personal Stats page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatRow {
    pub name: String,
    pub points: i32,
    pub kills: i32,
    pub deaths: i32,
    /// Command posts captured (the `stats_cpp` column).
    pub posts: i32,
    /// The viewer's own row.
    pub me: bool,
    /// The Personal Stats lines: label and value; a blank pair keeps its
    /// line's pitch.
    pub personal: Vec<(String, String)>,
    /// Localized medal names and the counts earned in this round.
    pub medals: Vec<(String, u32)>,
}

/// One team's list: the side's emblem sheet and its rows, sorted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamList {
    /// `all_icon`, `imp_icon`, `rep_icon` or `cis_icon`; empty draws none.
    pub icon: String,
    pub rows: Vec<StatRow>,
}

/// One line of the awards panel: the award's name and its winner's text
/// ("UNIT 538 (1)"); an empty winner is drawn blank.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Award {
    pub label: String,
    pub winner: String,
}

/// One career medal: its name, count, level ("Green") and the line that
/// tells how it is earned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Medal {
    pub name: String,
    pub count: u32,
    pub level: String,
    pub info: String,
}

/// The career page's numbers: the six labelled values (player points,
/// rank, kills, kill/death ratio, deaths, points per life) and the medals.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Career {
    /// Label and value, in the page's reading order: left column then right
    /// column per line.
    pub values: Vec<(String, String)>,
    pub medals: Vec<Medal>,
    /// `[ours]` (card UNLK3): the unlockable classes as medal rows: name,
    /// career kills, the stage (the level's place) and the line below.
    pub unlocks: Vec<Medal>,
}

/// The rows the career box shows: the medals or the unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CareerView {
    /// The first row's index in the medal cursor's run (medals, then
    /// unlocks), the row count and the left column's.
    pub start: usize,
    pub len: usize,
    pub left: usize,
    /// The unlocks' page.
    pub unlocks: bool,
}

impl Career {
    /// The medal cursor's row: a medal, else an unlock.
    pub fn row(&self, k: usize) -> Option<&Medal> {
        match k.checked_sub(self.medals.len()) {
            None => self.medals.get(k),
            Some(u) => self.unlocks.get(u),
        }
    }

    /// The box's two pages (the unlocks' only when there are any).
    pub fn views(&self) -> Vec<CareerView> {
        let n = self.medals.len();
        let mut v = vec![CareerView {
            start: 0,
            len: n,
            left: MEDALS_LEFT.min(n),
            unlocks: false,
        }];
        if !self.unlocks.is_empty() {
            v.push(CareerView {
                start: n,
                len: self.unlocks.len(),
                left: UNLOCKS_LEFT.min(self.unlocks.len()),
                unlocks: true,
            });
        }
        v
    }

    /// The page row `k` is on.
    pub fn view_of(&self, k: usize) -> CareerView {
        let views = self.views();
        views
            .iter()
            .copied()
            .rfind(|v| k >= v.start)
            .unwrap_or(views[0])
    }
}

/// The pages' fixed texts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatsTexts {
    pub team_title: String,
    pub awards_title: String,
    pub personal_title: String,
    pub medals_title: String,
    pub career_title: String,
    pub career_medals: String,
    /// `[ours]`: the unlocks' page heading.
    pub career_unlocks: String,
    pub done: String,
    pub select: String,
    pub back: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Team,
    Personal,
    Career,
}

/// This frame's presses (edges), the PS2's pad names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pad {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub l1: bool,
    pub r1: bool,
    pub cross: bool,
    pub triangle: bool,
    pub square: bool,
}

impl Pad {
    pub fn any(&self) -> bool {
        self.up
            || self.down
            || self.left
            || self.right
            || self.l1
            || self.r1
            || self.cross
            || self.triangle
            || self.square
    }
}

/// What a press asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Leave the stats (`ScriptCB_QuitFromStats`): the next playlist map,
    /// else the shell.
    Quit,
}

/// The stats screens: their data and where the cursors stand.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatsScreens {
    pub texts: StatsTexts,
    /// Team 1 (left) and team 2 (right).
    pub lists: [TeamList; 2],
    /// The awards panel's two columns of four.
    pub awards: [Vec<Award>; 2],
    pub career: Career,
    pub page: Page,
    /// The list the cursor is in (0 left) and its row.
    pub list: usize,
    pub row: usize,
    /// Per list, its first shown row.
    pub first: [usize; 2],
    /// The focused stat on the Personal Stats page.
    pub personal_row: usize,
    /// The career page's medal cursor (`career.medals` index).
    pub medal: usize,
}

/// BF2's order (statsref README, deduced from four lists): points down,
/// then kills down, then deaths up; equal rows keep their order.
pub fn sort_rows(rows: &mut [StatRow]) {
    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.kills.cmp(&a.kills))
            .then(a.deaths.cmp(&b.deaths))
    });
}

impl StatsScreens {
    /// Team Stats with the cursor on the viewer's row (`SetCursorToPlayer`),
    /// its list scrolled just far enough to show it. The rows are sorted
    /// here.
    pub fn new(
        texts: StatsTexts,
        mut lists: [TeamList; 2],
        awards: [Vec<Award>; 2],
        career: Career,
    ) -> Self {
        for l in &mut lists {
            sort_rows(&mut l.rows);
        }
        let (list, row) = (0..2)
            .find_map(|l| lists[l].rows.iter().position(|r| r.me).map(|i| (l, i)))
            .unwrap_or((0, 0));
        let mut first = [0; 2];
        first[list] = (row + 1).saturating_sub(SHOWN);
        Self {
            texts,
            lists,
            awards,
            career,
            page: Page::Team,
            list,
            row,
            first,
            personal_row: 0,
            medal: 0,
        }
    }

    /// The row the cursor is on, if its list has any.
    pub fn selected(&self) -> Option<&StatRow> {
        self.lists[self.list].rows.get(self.row)
    }

    pub fn player_row(&self) -> Option<&StatRow> {
        self.lists
            .iter()
            .flat_map(|list| &list.rows)
            .find(|row| row.me)
    }

    pub fn selection(&self) -> (Page, usize, usize) {
        match self.page {
            Page::Team => (self.page, self.list, self.row),
            Page::Personal => (self.page, 0, self.personal_row),
            Page::Career => (self.page, 0, self.medal),
        }
    }

    /// One frame's presses. `Some(Quit)` leaves the stats.
    pub fn input(&mut self, p: Pad) -> Option<Command> {
        match self.page {
            Page::Team => {
                if p.square {
                    return Some(Command::Quit);
                }
                if p.cross && self.player_row().is_some() {
                    self.page = Page::Personal;
                    self.personal_row = 0;
                } else if p.up {
                    self.step(-1);
                } else if p.down {
                    self.step(1);
                } else if p.l1 {
                    self.page_up();
                } else if p.r1 {
                    self.page_down();
                } else if p.left || p.right {
                    self.flip();
                }
            }
            Page::Personal => {
                if p.square {
                    return Some(Command::Quit);
                }
                if p.triangle {
                    self.page = Page::Team;
                } else if p.cross {
                    self.page = Page::Career;
                    self.medal = 0;
                } else if p.up || p.down {
                    self.move_personal_row(p.up);
                }
            }
            Page::Career => {
                if p.square || p.cross {
                    return Some(Command::Quit);
                }
                if p.triangle {
                    self.page = Page::Personal;
                } else {
                    self.move_medal(p);
                }
            }
        }
        None
    }

    fn len(&self) -> usize {
        self.lists[self.list].rows.len()
    }

    fn move_personal_row(&mut self, up: bool) {
        let Some(row) = self.player_row() else { return };
        let valid = |i: &usize| !row.personal[*i].0.is_empty();
        let next = if up {
            (0..self.personal_row).rev().find(valid)
        } else {
            (self.personal_row + 1..row.personal.len()).find(valid)
        };
        if let Some(next) = next {
            self.personal_row = next;
        }
    }

    /// Keep the cursor's row shown.
    fn scroll_to_row(&mut self) {
        let f = &mut self.first[self.list];
        if self.row < *f {
            *f = self.row;
        } else if self.row >= *f + SHOWN {
            *f = self.row + 1 - SHOWN;
        }
    }

    /// Up / Down: a row, stopping at the ends `[H]` (the ends were not
    /// pressed against on the captures).
    fn step(&mut self, by: i32) {
        let n = self.len();
        if n == 0 {
            return;
        }
        self.row = (self.row as i32 + by).clamp(0, n as i32 - 1) as usize;
        self.scroll_to_row();
    }

    /// L1: the first shown row; there already, a page up.
    fn page_up(&mut self) {
        let f = self.first[self.list];
        if self.row > f {
            self.row = f;
        } else {
            self.row = self.row.saturating_sub(SHOWN);
            self.first[self.list] = f.saturating_sub(SHOWN);
            self.scroll_to_row();
        }
    }

    /// R1: the last shown row (statsref: R1 jumped to row 9); there already,
    /// a page down.
    fn page_down(&mut self) {
        let n = self.len();
        if n == 0 {
            return;
        }
        let f = self.first[self.list];
        let last = (f + SHOWN - 1).min(n - 1);
        if self.row < last {
            self.row = last;
        } else {
            self.row = (self.row + SHOWN).min(n - 1);
            self.first[self.list] = (f + SHOWN).min(n.saturating_sub(SHOWN));
            self.scroll_to_row();
        }
    }

    /// Left / Right: the other list, on the same shown row (its last when
    /// shorter); an empty list is not entered.
    fn flip(&mut self) {
        let other = 1 - self.list;
        let n = self.lists[other].rows.len();
        if n == 0 {
            return;
        }
        let shown_row = self.row - self.first[self.list];
        self.list = other;
        self.row = (self.first[other] + shown_row).min(n - 1);
        self.scroll_to_row();
    }

    /// The career page's medal cursor: Up / Down within a column, Left /
    /// Right to the other column on the same line (its last when shorter).
    /// `[ours]`: Down off a column's end goes on to the unlocks' page, the
    /// same column's first line; Up off its top comes back to the medals'
    /// last line of that column.
    fn move_medal(&mut self, p: Pad) {
        let views = self.career.views();
        let Some(page) = views
            .iter()
            .rposition(|v| self.medal >= v.start)
            .filter(|&i| views[i].len > 0)
        else {
            return;
        };
        let v = views[page];
        let k = self.medal - v.start;
        let (col, line) = if k < v.left { (0, k) } else { (1, k - v.left) };
        let len = |v: CareerView, c: usize| if c == 0 { v.left } else { v.len - v.left };
        let at = |v: CareerView, c: usize, line: usize| {
            let c = if len(v, c) == 0 { 0 } else { c };
            let line = line.min(len(v, c).saturating_sub(1));
            v.start + if c == 0 { line } else { v.left + line }
        };
        self.medal = if p.up {
            match (line, page.checked_sub(1)) {
                (0, Some(prev)) => at(views[prev], col, usize::MAX),
                _ => at(v, col, line.saturating_sub(1)),
            }
        } else if p.down {
            match views.get(page + 1) {
                Some(&next) if line + 1 >= len(v, col) => at(next, col, 0),
                _ => at(v, col, line + 1),
            }
        } else if (p.left || p.right) && len(v, 1 - col) > 0 {
            at(v, 1 - col, line)
        } else {
            self.medal
        };
    }
}

/// Seconds as BF2's stats print a time: "1:24".
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, points: i32, kills: i32, deaths: i32) -> StatRow {
        StatRow {
            name: name.into(),
            points,
            kills,
            deaths,
            ..Default::default()
        }
    }

    fn team(n: usize, me: Option<usize>) -> TeamList {
        TeamList {
            icon: "rep_icon".into(),
            rows: (0..n)
                .map(|i| StatRow {
                    me: me == Some(i),
                    // Descending points keep the build order as the sort.
                    points: 100 - i as i32,
                    ..row(&format!("U{i}"), 0, 0, 0)
                })
                .collect(),
        }
    }

    fn screens(a: usize, b: usize, me: (usize, usize)) -> StatsScreens {
        let mine = |l: usize| (me.0 == l).then_some(me.1);
        StatsScreens::new(
            StatsTexts::default(),
            [team(a, mine(0)), team(b, mine(1))],
            [Vec::new(), Vec::new()],
            Career {
                values: Vec::new(),
                medals: (0..9).map(|_| Medal::default()).collect(),
                unlocks: Vec::new(),
            },
        )
    }

    fn press(f: impl FnOnce(&mut Pad)) -> Pad {
        let mut p = Pad::default();
        f(&mut p);
        p
    }

    /// Points down, then kills down, then deaths up (the README's four
    /// lists): Dagobah's right list in its PS2 order, shuffled and sorted
    /// back.
    #[test]
    fn rows_sort_by_points_then_kills_then_deaths() {
        let want = [
            row("PG 134", 2, 0, 1),
            row("SU 697", 1, 1, 1),
            row("CX 512", 1, 1, 2),
            row("JM 221", 1, 1, 2),
            row("SF 021", 1, -1, 1),
            row("VH 961", 1, -1, 2),
            row("KG 077", 0, 0, 0),
            row("ZP 778", 0, 0, 1),
            row("JE 929", -2, -1, 1),
        ];
        let mut rows = want.to_vec();
        rows.reverse();
        rows.swap(0, 4);
        sort_rows(&mut rows);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        // CX 512 and JM 221 tie: their order stays as given (reversed).
        assert_eq!(
            names,
            [
                "PG 134", "SU 697", "JM 221", "CX 512", "SF 021", "VH 961", "KG 077", "ZP 778",
                "JE 929"
            ]
        );
    }

    /// The cursor starts on the player's row; a row past the first page
    /// scrolls its list just enough.
    #[test]
    fn the_cursor_starts_on_the_players_row() {
        let s = screens(16, 16, (1, 5));
        assert_eq!((s.list, s.row, s.first), (1, 5, [0, 0]));
        assert!(s.selected().unwrap().me);
        let s = screens(16, 16, (0, 12));
        assert_eq!((s.list, s.row, s.first), (0, 12, [4, 0]));
    }

    /// Up / Down a row (stopping at the ends), scrolling the 9 shown rows;
    /// R1 to the last shown row and then a page on; L1 back.
    #[test]
    fn rows_move_page_and_scroll() {
        let mut s = screens(20, 16, (0, 0));
        s.input(press(|p| p.up = true));
        assert_eq!(s.row, 0, "the top stops");
        s.input(press(|p| p.r1 = true));
        assert_eq!((s.row, s.first[0]), (8, 0), "R1: the last shown row");
        s.input(press(|p| p.down = true));
        assert_eq!((s.row, s.first[0]), (9, 1), "a row past it scrolls one");
        s.input(press(|p| p.r1 = true));
        assert_eq!((s.row, s.first[0]), (18, 10), "R1 there: a page on");
        s.input(press(|p| p.r1 = true));
        assert_eq!((s.row, s.first[0]), (19, 11), "the last row");
        s.input(press(|p| p.down = true));
        assert_eq!(s.row, 19, "the bottom stops");
        s.input(press(|p| p.l1 = true));
        assert_eq!((s.row, s.first[0]), (11, 11), "L1: the first shown row");
        s.input(press(|p| p.l1 = true));
        assert_eq!((s.row, s.first[0]), (2, 2), "L1 there: a page up");
    }

    /// Left / Right switch lists on the same shown row, both ways round; a
    /// shorter list takes its last row.
    #[test]
    fn left_and_right_switch_lists_on_the_shown_row() {
        let mut s = screens(16, 4, (0, 6));
        s.input(press(|p| p.right = true));
        assert_eq!((s.list, s.row), (1, 3), "the short list's last row");
        s.input(press(|p| p.up = true));
        s.input(press(|p| p.right = true));
        assert_eq!((s.list, s.row), (0, 2), "Right from the right list wraps");
        s.input(press(|p| p.left = true));
        assert_eq!((s.list, s.row), (1, 2));
        let mut s = screens(16, 0, (0, 3));
        s.input(press(|p| p.left = true));
        assert_eq!((s.list, s.row), (0, 3), "an empty list is not entered");
    }

    /// X: Team -> the player's Personal -> the player's Career;
    /// Triangle back; Triangle does nothing on Team; Square quits from Team
    /// and Personal, X or Square from Career.
    #[test]
    fn buttons_walk_the_pages_and_quit() {
        let mut s = screens(16, 16, (1, 5));
        assert_eq!(s.input(press(|p| p.triangle = true)), None);
        assert_eq!(s.page, Page::Team);
        s.input(press(|p| p.cross = true));
        assert_eq!(s.page, Page::Personal);
        assert!(s.selected().unwrap().me);
        s.input(press(|p| p.cross = true));
        assert_eq!(s.page, Page::Career);
        s.input(press(|p| p.triangle = true));
        s.input(press(|p| p.triangle = true));
        assert_eq!(s.page, Page::Team);
        // Browsing a bot does not change whose personal and career stats open.
        s.input(press(|p| p.down = true));
        let viewed = (s.list, s.row);
        assert!(!s.selected().unwrap().me);
        s.input(press(|p| p.cross = true));
        assert_eq!(s.page, Page::Personal);
        assert!(s.player_row().unwrap().me);
        assert_eq!(s.input(press(|p| p.cross = true)), None);
        assert_eq!(s.page, Page::Career);
        s.input(press(|p| p.triangle = true));
        assert_eq!(s.page, Page::Personal);
        s.input(press(|p| p.triangle = true));
        assert_eq!((s.list, s.row), viewed);
        s.input(press(|p| p.cross = true));
        assert_eq!(s.input(press(|p| p.square = true)), Some(Command::Quit));
        let mut s = screens(3, 3, (0, 0));
        assert_eq!(s.input(press(|p| p.square = true)), Some(Command::Quit));
        s.page = Page::Career;
        assert_eq!(s.input(press(|p| p.cross = true)), Some(Command::Quit));
    }

    /// The medal cursor: five down the left column, four down the right;
    /// Left / Right keep the line (the right column's last when shorter).
    #[test]
    fn the_medal_cursor_moves_in_two_columns() {
        let mut s = screens(3, 3, (0, 0));
        s.page = Page::Career;
        for _ in 0..6 {
            s.input(press(|p| p.down = true));
        }
        assert_eq!(s.medal, 4, "the left column's last");
        s.input(press(|p| p.right = true));
        assert_eq!(s.medal, 8, "the right column's last line");
        s.input(press(|p| p.up = true));
        s.input(press(|p| p.left = true));
        assert_eq!(s.medal, 2);
        s.input(press(|p| p.up = true));
        s.input(press(|p| p.up = true));
        s.input(press(|p| p.up = true));
        assert_eq!(s.medal, 0);
    }

    /// Card UNLK3: Down off a medal column's end turns to the unlocks' page
    /// (four a column), the same column; Up off its top comes back to that
    /// column's last medal; Left / Right stay on the page.
    #[test]
    fn the_medal_cursor_runs_on_to_the_unlocks() {
        let mut s = screens(3, 3, (0, 0));
        s.career.unlocks = (0..8).map(|_| Medal::default()).collect();
        s.page = Page::Career;
        let down = press(|p| p.down = true);
        let up = press(|p| p.up = true);
        for _ in 0..5 {
            s.input(down);
        }
        assert_eq!(s.medal, 9, "the unlocks' first, left column");
        assert!(s.career.view_of(s.medal).unlocks);
        for _ in 0..5 {
            s.input(down);
        }
        assert_eq!(s.medal, 12, "their left column's last");
        s.input(press(|p| p.right = true));
        assert_eq!(s.medal, 16, "the right column, same line");
        for _ in 0..3 {
            s.input(up);
        }
        assert_eq!(s.medal, 13);
        s.input(up);
        assert_eq!(s.medal, 8, "back: the right medal column's last");
        assert!(!s.career.view_of(s.medal).unlocks);
        s.input(down);
        assert_eq!(s.medal, 13);
        assert_eq!(s.career.row(13), Some(&s.career.unlocks[4]));
    }

    #[test]
    fn times_print_as_minutes_and_seconds() {
        assert_eq!(clock(84.6), "1:24");
        assert_eq!(clock(36.0), "0:36");
        assert_eq!(clock(-3.0), "0:00");
    }
}
