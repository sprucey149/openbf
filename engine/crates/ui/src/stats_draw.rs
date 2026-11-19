//! The stats screens (issue #50, [`crate::stats`]) as BF2's PS2 draws them,
//! painted by the HUD pass in the shell's 512 × 448 units over black.
//!
//! Every place is `[M]`: measured on the native 640 × 480 PCSX2 frames of
//! `engine/artifacts/lead_tmp/statsref/` (1.25 × 1.0714 px per unit; Team
//! Stats `kam1_cw/20_teamstats.png` and `dag1_gcw/20_teamstats.png`,
//! Personal Stats `kam1_cw/308_after_K.png`, Career Stats
//! `dag1_gcw/301_after_K.png`), because the scripts' layout numbers
//! (`ifs_teamstats.luac` and its two siblings) are not read. The pieces are
//! the shell's (`analysis/re-bf2-shell.md` §3): a title tab of the
//! `bf2_buttons_top*` bar over a `border_3_pieces` box, the list cursor
//! `listbox_cursor`, the scroll wedges `listbox_topbot_more`, the help lines.
//! Texts are the menu roles (`crate::typography`): the titles [`TITLE`] (the
//! PS2 draws them in `gamefont_medium`), the rest [`LIST`] (`gamefont_tiny`,
//! capitals), at the roles' own size and stretch; a text too long for its
//! column ends in an ellipsis, except Career Stats rank and tier names,
//! which keep every glyph by fitting horizontally. Colours are the
//! glyphs' brightest pixels on the frames.
//!
//! `[ours]` Hor+: the pages keep the 4:3 box at the window's centre, the help
//! lines their edges, as the other shell pages.

use crate::hud_draw::{Ctx, Quad, Screen};
use crate::hud_tree::HAlign;
use crate::input_icons::{IconSet, Prompt};
use crate::spawn_draw::{screen_at, Anchor, HELP_DROP, ICON, ICON_GAP, SAFE_X, TITLE_PIECES};
use crate::stats::{Page, StatsScreens, SHOWN};
use crate::typography::{self, tint, Tint, HELP, LIST, TITLE};
use glam::Mat4;

/// Colours: the title tab's text at full alpha, grey names and numbers,
/// yellow points, the player's row white, the cursor row's name blue, the
/// award winners yellow (`crate::typography::tint`).
const GREY: Tint = tint::STATS_GREY;
const YELLOW: Tint = tint::STATS_YELLOW;
const WHITE: Tint = tint::STATS_WHITE;
const BLUE: Tint = tint::STATS_BLUE;
const WINNER: Tint = tint::STATS_WINNER;
/// `[ours]` A human's name on Team Stats is bold (issue #168): the run drawn
/// again [`BOLD_DX`] units right.
const BOLD_DX: f32 = typography::BOLD_DX;

/// Every panel's left and right border (Team, Personal and Career: 26 ..
/// 616 px).
const PANEL_X: [f32; 2] = [20.8, 492.8];
/// The first title tab (y 24.5 .. 56.5 px) and its title's centre across
/// (the ink centred at 318 px).
const TAB: [f32; 2] = [22.9, 52.7];
const TITLE_CX: f32 = 255.5;
/// A title's run stands centred in its tab's height.
fn title_top(tab: [f32; 2], line_h: f32) -> f32 {
    tab[0] + (tab[1] - tab[0] - line_h) * 0.5
}

/// Team Stats: the lists' box (57 .. 291 px), the awards' tab (narrower:
/// 28.8 .. 613.7 × 293.5 .. 324.5 px) and box (325 .. 422 px).
const TEAM_BOX: [f32; 4] = [PANEL_X[0], 53.2, PANEL_X[1], 271.6];
const AWARDS_TAB: [f32; 4] = [23.0, 273.9, 491.0, 302.9];
const AWARDS_BOX: [f32; 4] = [PANEL_X[0], 303.3, PANEL_X[1], 393.9];
/// The right list is the left one 294 px further right.
const LIST_DX: f32 = 235.2;
/// The header: the side's emblem centred at 95 px, the four column icons at
/// 201.5 / 236.5 / 272 / 307.5 px (points, kills, deaths, posts), all 16.8
/// units square (21 × 18 px) about y 75.5 px.
const EMBLEM_X: f32 = 76.0;
const COLUMN_X: [f32; 4] = [161.2, 189.2, 217.6, 246.0];
const HEADER_Y: f32 = 70.47;
const HEADER_ICON: f32 = 16.8;
const COLUMN_ICONS: [&str; 4] = ["points", "stats_kills", "stats_deaths", "stats_cpp"];
/// The scroll wedges: two mirrored halves of `listbox_topbot_more` about x
/// 173.5 px, the up one in the header (ink 159 .. 188 × 75 .. 83 px), the
/// down one on the box's bottom border (157 .. 190 × 288 .. 293 px).
const WEDGE_X: f32 = 138.8;
const UP_WEDGE: [f32; 3] = [27.2, 63.5, 78.4];
const DOWN_WEDGE: [f32; 3] = [30.7, 268.8, 278.8];
/// The rows: names from 36 px, squeezed across to fit 140 px (the frames'
/// long names: "THRAK FARELLE" 134 px where its stretch gives 149, "BEN
/// PULASTRA" 145 at its own; the script's `NameWidth` was not read), the
/// first run's top (ink 90 px less the font's 4 texels) and the pitch
/// (22.625 px).
const NAME_X: f32 = 28.8;
const NAME_W: f32 = 112.0;
const ROW_TOP: f32 = 80.0;
const ROW_PITCH: f32 = 21.117;
/// The cursor's outline about a row: 28.5 .. 316.5 px across (the right
/// list's 294 px on), 6.5 px above the ink to 13.5 below its top.
const CURSOR_X: [f32; 2] = [22.8, 253.2];
const CURSOR_Y: [f32; 2] = [-2.07, 16.6];
/// The awards: the first label's run top (ink 340 px), the pitch (20.67
/// px), the labels' x (35 / 330 px), at the page's stretch ("SURVIVALIST"
/// 127 px over 89 texels, "TRAITOR" 81 over 57) squeezed across to fit
/// about 127 px (the long ones end 122 .. 131 px wide: "TANK BUSTER",
/// "KILLING SPREE", "PUBLIC ENEMY", "BANTHA FODDER"); the winners
/// right-aligned at 310.5 / 604.5 px, squeezed to fit 130 px (the frames'
/// longest winners all end 124 .. 130 px wide).
const AWARD_TOP: f32 = 313.3;
const AWARD_PITCH: f32 = 19.29;
const AWARD_X: [f32; 2] = [28.0, 264.0];
const AWARD_W: f32 = 101.6;
const WINNER_RIGHT: [f32; 2] = [248.4, 483.6];
const WINNER_W: f32 = 104.0;

/// Personal Stats: its box (57 .. 247 px), the Medals tab (250.5 .. 282.5
/// px) and box (283 .. 413 px); lines from 45 px, values right-aligned at
/// 592.5 px, the first run's top (ink 66 px) and the pitch (17.22 px).
const PERSONAL_BOX: [f32; 4] = [PANEL_X[0], 53.2, PANEL_X[1], 230.5];
const MEDALS_TAB: [f32; 2] = [233.8, 263.7];
const MEDALS_BOX: [f32; 4] = [PANEL_X[0], 264.1, PANEL_X[1], 385.5];
const LINE_X: [f32; 2] = [36.0, 474.0];
const LINE_TOP: f32 = 57.6;
const LINE_PITCH: f32 = 16.07;
// BF2 ifs_personalstats main.f4/f6: fill two columns across each row,
// showing only earned medals, with each count in parentheses.
const ROUND_MEDAL_X: [f32; 2] = [36.0, 256.8];
const ROUND_MEDAL_TOP: f32 = 270.2;
const ROUND_MEDAL_PITCH: f32 = 14.0;
const ROUND_MEDAL_W: f32 = 216.0;

/// Career Stats: three boxes sharing borders (57 .. 165, 161 .. 331, 327 ..
/// 422 px); the values' lines at line 0, 2 and 3 of a 23.5 px pitch from
/// ink 73 px, labels from 36 / 330 px, values right-aligned at 309.5 /
/// 604.5 px and squeezed to 73 px ("PRIVATE" 73 px over 56 texels, "0.00"
/// at the page's stretch); "Career Medals" from 39 px (ink 187 px); the
/// medals' lines from ink 212 px, the same pitch, names from 38 / 332 px,
/// counts centred at 208 / 502 px, levels right-aligned at 310 / 604 px
/// and squeezed to 61 px ("GREEN" 61 px over 44 texels); the medal cursor
/// 31 .. 314.5 px, 0.75 px above the run's top to 17 below; the medal's
/// text from 39 px (ink 344 px), 16 px a line, broken to 452 units.
const CAREER_BOXES: [[f32; 4]; 3] = [
    [PANEL_X[0], 53.2, PANEL_X[1], 154.9],
    [PANEL_X[0], 150.3, PANEL_X[1], 309.0],
    [PANEL_X[0], 305.2, PANEL_X[1], 393.9],
];
const CAREER_LINE_TOP: f32 = 64.13;
const CAREER_PITCH: f32 = 21.93;
const CAREER_LINES: [usize; 3] = [0, 2, 3];
const CAREER_LABEL_X: [f32; 2] = [28.8, 264.0];
const CAREER_VALUE_RIGHT: [f32; 2] = [247.6, 483.6];
const CAREER_VALUE_W: f32 = 58.4;
const MEDALS_HEAD: (f32, f32) = (31.2, 170.5);
const MEDAL_TOP: f32 = 193.9;
const MEDAL_NAME_X: [f32; 2] = [30.4, 265.6];
const MEDAL_COUNT_X: [f32; 2] = [166.4, 401.6];
const MEDAL_LEVEL_RIGHT: [f32; 2] = [248.0, 483.2];
const MEDAL_CURSOR_X: [f32; 2] = [24.8, 251.6];
/// `[ours]` A row's name squeezed to fit before the count (the medals'
/// names all fit; the unlockable classes' longest do not).
const MEDAL_NAME_W: f32 = 104.0;
/// `[ours]` The career box's wedges when the unlocks' page is there: the
/// Team Stats list's, at the box's centre, the down one on the box's bottom
/// border, the up one beside the heading.
const CAREER_WEDGE_X: f32 = 256.8;
const CAREER_UP_WEDGE: [f32; 3] = [27.2, 160.6, 175.5];
const CAREER_DOWN_WEDGE: [f32; 3] = [30.7, 306.2, 316.2];
const MEDAL_CURSOR_Y: [f32; 2] = [-0.7, 17.0];
const INFO_AT: (f32, f32) = (31.2, 317.1);
const INFO_PITCH: f32 = 14.93;
const INFO_W: f32 = 452.0;

/// The help lines' Square: BF2's own `btnmisc` is not baked (COMMON.LVL)
/// and BF1's is another picture (a blue ring round a filled square), so
/// `[ours]` its outline is drawn as the frames show it: pink (209, 129, 195),
/// 9 px across (35 .. 43 px), 2 px thick.
const SQUARE: [f32; 2] = [7.2, 8.4];
const SQUARE_LINE: [f32; 2] = [1.6, 1.87];
const SQUARE_PINK: [f32; 3] = [209.0, 129.0, 195.0];

const NONE: usize = usize::MAX;

/// The mission's end movie before the stats (`FUN_00381110` →
/// `FUN_00564398`), after the banner's fade to black: black over the window
/// and, once a frame is decoded (`movie`), the clip in the shell's 512 × 448
/// box as the shell's own movies ([`crate::shell_ps2::MOVIE_SHEET`]).
pub fn draw_end_movie(
    hud: &crate::hud_tree::Hud,
    art: &crate::hud_draw::HudArt,
    movie: bool,
    size: [f32; 2],
) -> crate::hud_draw::HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: crate::hud_draw::HudFrame::default(),
        clip: None,
    };
    ctx.quad(
        usize::MAX,
        &Mat4::IDENTITY,
        &Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        },
        &Quad {
            rect: [0.0, 0.0, size[0], size[1]],
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [0.0, 0.0, 0.0, 1.0],
            texture: "white",
            additive: false,
            shadow: false,
            boxed: false,
        },
    );
    if movie {
        ctx.sheet(
            &screen_at(size, Anchor::Centre),
            [0.0, 0.0, 512.0, 448.0],
            [0.0, 0.0, 1.0, 1.0],
            crate::shell_ps2::MOVIE_SHEET,
            1.0,
        );
    }
    ctx.out
}

/// A help line's pad icon.
#[derive(Clone, Copy)]
enum Icon {
    Cross,
    Triangle,
    Square,
}

impl Ctx<'_> {
    /// A panel: its title tab (the shell's title bar pieces, as the class
    /// panel's) with the title centred, and its `border_3_pieces` box.
    fn stats_panel(&mut self, s: &Screen, tab: [f32; 4], title: &str, body: [f32; 4]) {
        self.bar(s, tab, TITLE_PIECES);
        let at = (TITLE_CX, title_top([tab[1], tab[3]], TITLE.size));
        let title = self.ellipsized(&TITLE, title, tab[2] - tab[0] - 36.0);
        self.text(s, &TITLE, &title, at, HAlign::Center, tint::STATS_TITLE);
        self.stats_box(s, body);
    }

    /// `border_3_pieces` as nine parts: 16-unit corners over 16 texels.
    fn stats_box(&mut self, s: &Screen, r: [f32; 4]) {
        self.nine(
            s,
            r,
            [0.0, 0.0, 1.0, 1.0],
            (16.0, 16.0 / 64.0),
            "border_3_pieces",
        );
    }

    /// A help line: the pad icon at the safe area's edge, its label beside
    /// it on the inside (`crate::spawn_draw`'s numbers).
    fn stats_help(&mut self, size: [f32; 2], right: bool, icon: Icon, text: &str) {
        let (a, ix) = if right {
            (Anchor::Right, SAFE_X[1] - ICON[0])
        } else {
            (Anchor::Left, SAFE_X[0])
        };
        let s = screen_at(size, a);
        let y = crate::spawn_draw::HELP_Y;
        let top = y - ICON[1] * 0.5;
        match icon {
            Icon::Square if self.art.icons.set == IconSet::Ds2 => {
                let cx = ix + ICON[0] * 0.5;
                let [w, h] = SQUARE;
                let [x0, y0, x1, y1] = [cx - w * 0.5, y - h * 0.5, cx + w * 0.5, y + h * 0.5];
                let [tx, ty] = SQUARE_LINE;
                let full = [0.0, 0.0, 1.0, 1.0];
                for r in [
                    [x0, y0, x1, y0 + ty],
                    [x0, y1 - ty, x1, y1],
                    [x0, y0, x0 + tx, y1],
                    [x1 - tx, y0, x1, y1],
                ] {
                    self.sheet_tinted(&s, r, full, "white", (SQUARE_PINK, 1.0));
                }
            }
            Icon::Cross | Icon::Triangle | Icon::Square => {
                let sheet = self.art.icons.sheet(match icon {
                    Icon::Cross => Prompt::Accept,
                    Icon::Triangle => Prompt::Back,
                    Icon::Square => Prompt::Done,
                });
                let r = [ix, top, ix + ICON[0], top + ICON[1]];
                self.sheet(&s, r, [0.0, 0.0, 1.0, 1.0], sheet, 1.0);
            }
        }
        let (x, align) = if right {
            (ix - ICON_GAP, HAlign::Right)
        } else {
            (ix + ICON[0] + ICON_GAP, HAlign::Left)
        };
        let at = (x, y - HELP.size * 0.5 + HELP_DROP);
        self.text(&s, &HELP, text, at, align, tint::HELP);
    }

    /// The black the pages stand on (`statsscreens_bg` is black on the
    /// disc), over the whole window.
    fn stats_black(&mut self, size: [f32; 2]) {
        let s = Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        };
        self.quad(
            NONE,
            &Mat4::IDENTITY,
            &s,
            &Quad {
                rect: [0.0, 0.0, size[0], size[1]],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [0.0, 0.0, 0.0, 1.0],
                texture: "white",
                additive: false,
                shadow: false,
                boxed: false,
            },
        );
    }

    /// The stats page up, alone over black.
    pub(crate) fn stats_page(&mut self, st: &StatsScreens, size: [f32; 2]) {
        self.stats_black(size);
        let s = screen_at(size, Anchor::Centre);
        match st.page {
            Page::Team => self.team_stats(&s, st, size),
            Page::Personal => self.personal_stats(&s, st, size),
            Page::Career => self.career_stats(&s, st, size),
        }
    }

    fn team_stats(&mut self, s: &Screen, st: &StatsScreens, size: [f32; 2]) {
        let tab = [PANEL_X[0], TAB[0], PANEL_X[1], TAB[1]];
        self.stats_panel(s, tab, &st.texts.team_title, TEAM_BOX);
        self.stats_panel(s, AWARDS_TAB, &st.texts.awards_title, AWARDS_BOX);
        let full = [0.0, 0.0, 1.0, 1.0];
        for (l, list) in st.lists.iter().enumerate() {
            let dx = LIST_DX * l as f32;
            let icon = |x: f32| {
                let h = HEADER_ICON * 0.5;
                [x - h, HEADER_Y - h, x + h, HEADER_Y + h]
            };
            if !list.icon.is_empty() {
                self.sheet(s, icon(EMBLEM_X + dx), full, &list.icon, 1.0);
            }
            for (x, sheet) in COLUMN_X.iter().zip(COLUMN_ICONS) {
                self.sheet(s, icon(x + dx), full, sheet, 1.0);
            }
            self.wedges(s, WEDGE_X + dx);
            let first = st.first[l];
            for (k, row) in list.rows.iter().enumerate().skip(first).take(SHOWN) {
                let top = ROW_TOP + ROW_PITCH * (k - first) as f32;
                let cursor = st.list == l && st.row == k;
                if cursor {
                    let r = [
                        CURSOR_X[0] + dx,
                        top + CURSOR_Y[0],
                        CURSOR_X[1] + dx,
                        top + CURSOR_Y[1],
                    ];
                    self.cursor_box(s, r);
                }
                let (name, number) = match (cursor, row.me) {
                    (true, true) => (BLUE, WHITE),
                    (true, false) => (BLUE, GREY),
                    (false, true) => (WHITE, WHITE),
                    (false, false) => (GREY, GREY),
                };
                let at = (NAME_X + dx, top);
                let bold = if row.human { BOLD_DX } else { 0.0 };
                let fitted = self.ellipsized(&LIST, &row.name, NAME_W - bold);
                self.text(s, &LIST, &fitted, at, HAlign::Left, name);
                if row.human {
                    let at = (at.0 + bold, at.1);
                    self.text(s, &LIST, &fitted, at, HAlign::Left, name);
                }
                let values = [row.points, row.kills, row.deaths, row.posts];
                for (c, (x, v)) in COLUMN_X.iter().zip(values).enumerate() {
                    let style = if c == 0 { YELLOW } else { number };
                    let at = (x + dx, top);
                    let v = v.to_string();
                    self.text(s, &LIST, &v, at, HAlign::Center, style);
                }
            }
        }
        for (c, column) in st.awards.iter().enumerate() {
            for (k, a) in column.iter().enumerate() {
                let top = AWARD_TOP + AWARD_PITCH * k as f32;
                let at = (AWARD_X[c], top);
                let fitted = self.ellipsized(&LIST, &a.label, AWARD_W);
                self.text(s, &LIST, &fitted, at, HAlign::Left, WHITE);
                if a.winner.is_empty() {
                    continue;
                }
                let fitted = self.ellipsized(&LIST, &a.winner, WINNER_W);
                let at = (WINNER_RIGHT[c], top);
                self.text(s, &LIST, &fitted, at, HAlign::Right, WINNER);
            }
        }
        self.stats_help(size, false, Icon::Square, &st.texts.done);
        self.stats_help(size, true, Icon::Cross, &st.texts.select);
    }

    /// The two scroll wedges of a list centred at `cx`.
    fn wedges(&mut self, s: &Screen, cx: f32) {
        self.wedge(s, cx, UP_WEDGE, false);
        self.wedge(s, cx, DOWN_WEDGE, true);
    }

    /// One wedge: two mirrored halves of `listbox_topbot_more`, `down`
    /// flipped.
    fn wedge(&mut self, s: &Screen, cx: f32, [hw, y0, y1]: [f32; 3], down: bool) {
        let tex = "listbox_topbot_more";
        let (v0, v1) = if down { (1.0, 0.0) } else { (0.0, 1.0) };
        self.sheet(s, [cx - hw, y0, cx, y1], [0.0, v0, 1.0, v1], tex, 1.0);
        self.sheet(s, [cx, y0, cx + hw, y1], [1.0, v0, 0.0, v1], tex, 1.0);
    }

    fn personal_stats(&mut self, s: &Screen, st: &StatsScreens, size: [f32; 2]) {
        let tab = [PANEL_X[0], TAB[0], PANEL_X[1], TAB[1]];
        let title = st
            .player_row()
            .filter(|row| !row.name.is_empty())
            .map_or_else(
                || st.texts.personal_title.clone(),
                |row| format!("{} ({})", st.texts.personal_title, row.name),
            );
        self.stats_panel(s, tab, &title, PERSONAL_BOX);
        let medals_tab = [PANEL_X[0], MEDALS_TAB[0], PANEL_X[1], MEDALS_TAB[1]];
        self.stats_panel(s, medals_tab, &st.texts.medals_title, MEDALS_BOX);
        if let Some(row) = st.player_row() {
            for (k, (label, value)) in row.personal.iter().enumerate() {
                let top = LINE_TOP + LINE_PITCH * k as f32;
                let focused = k == st.personal_row && !label.is_empty();
                if focused {
                    self.cursor_box(
                        s,
                        [
                            LINE_X[0] - 6.0,
                            top - 0.5,
                            LINE_X[1] + 6.0,
                            top + LINE_PITCH,
                        ],
                    );
                }
                let at = (LINE_X[0], top);
                let tint = if focused { BLUE } else { WHITE };
                self.text(s, &LIST, label, at, HAlign::Left, tint);
                let at = (LINE_X[1], top);
                self.text(s, &LIST, value, at, HAlign::Right, WHITE);
            }
            for (i, (name, count)) in row.medals.iter().enumerate() {
                let label = format!("{name} ({count})");
                let at = (
                    ROUND_MEDAL_X[i % 2],
                    ROUND_MEDAL_TOP + ROUND_MEDAL_PITCH * (i / 2) as f32,
                );
                let fitted = self.ellipsized(&LIST, &label, ROUND_MEDAL_W);
                self.text(s, &LIST, &fitted, at, HAlign::Left, WHITE);
            }
        }
        self.stats_help(size, false, Icon::Triangle, &st.texts.back);
        self.stats_help(size, true, Icon::Cross, &st.texts.career_title);
    }

    /// Career names must remain complete. Use the shared role and its
    /// actual advances, changing only horizontal scale when room runs out.
    /// `[ours]` Keep the measured anchors and a shared text inset between
    /// fields; unlike other stats columns these names never use ellipses.
    fn career_fit(&self, text: &str, room: f32) -> typography::TextStyle {
        let width = self.text_width(&LIST, text);
        let fit = if width > 0.0 {
            (room / width).clamp(0.0, 1.0)
        } else {
            1.0
        };
        typography::TextStyle {
            stretch: LIST.stretch * fit,
            tracking: LIST.tracking * fit,
            ..LIST
        }
    }

    fn career_stats(&mut self, s: &Screen, st: &StatsScreens, size: [f32; 2]) {
        let tab = [PANEL_X[0], TAB[0], PANEL_X[1], TAB[1]];
        self.stats_panel(s, tab, &st.texts.career_title, CAREER_BOXES[0]);
        self.stats_box(s, CAREER_BOXES[1]);
        self.stats_box(s, CAREER_BOXES[2]);
        let c = &st.career;
        for (k, (label, value)) in c.values.iter().enumerate() {
            let (line, col) = (CAREER_LINES[(k / 2).min(2)], k % 2);
            let top = CAREER_LINE_TOP + CAREER_PITCH * line as f32;
            let at = (CAREER_LABEL_X[col], top);
            self.text(s, &LIST, label, at, HAlign::Left, WHITE);
            let at = (CAREER_VALUE_RIGHT[col], top);
            if k == 1 {
                // Rank is the only named value; numbers retain their columns.
                let room = at.0
                    - CAREER_LABEL_X[col]
                    - self.text_width(&LIST, label)
                    - typography::TEXT_INSET;
                let style = self.career_fit(value, room);
                self.text(s, &style, value, at, HAlign::Right, YELLOW);
            } else {
                let fitted = self.ellipsized(&LIST, value, CAREER_VALUE_W);
                self.text(s, &LIST, &fitted, at, HAlign::Right, YELLOW);
            }
        }
        let view = c.view_of(st.medal);
        let (head, rows) = if view.unlocks {
            (&st.texts.career_unlocks, &c.unlocks)
        } else {
            (&st.texts.career_medals, &c.medals)
        };
        self.text(s, &LIST, head, MEDALS_HEAD, HAlign::Left, YELLOW);
        if c.views().len() > 1 {
            match view.unlocks {
                true => self.wedge(s, CAREER_WEDGE_X, CAREER_UP_WEDGE, false),
                false => self.wedge(s, CAREER_WEDGE_X, CAREER_DOWN_WEDGE, true),
            }
        }
        let split = view.left;
        for (k, m) in rows.iter().enumerate() {
            let (col, line) = if k < split { (0, k) } else { (1, k - split) };
            let dx = LIST_DX * col as f32;
            let top = MEDAL_TOP + CAREER_PITCH * line as f32;
            let on = view.start + k == st.medal;
            if on {
                let r = [
                    MEDAL_CURSOR_X[0] + dx,
                    top + MEDAL_CURSOR_Y[0],
                    MEDAL_CURSOR_X[1] + dx,
                    top + MEDAL_CURSOR_Y[1],
                ];
                self.cursor_box(s, r);
            }
            let (text, number) = if on { (BLUE, BLUE) } else { (WHITE, YELLOW) };
            let at = (MEDAL_NAME_X[col], top);
            let fitted = self.ellipsized(&LIST, &m.name, MEDAL_NAME_W);
            self.text(s, &LIST, &fitted, at, HAlign::Left, text);
            let at = (MEDAL_COUNT_X[col], top);
            let n = m.count.to_string();
            // Leave room for the name even at the largest stored count.
            let count_room =
                2.0 * (at.0 - MEDAL_NAME_X[col] - MEDAL_NAME_W - typography::TEXT_INSET);
            let count_style = self.career_fit(&n, count_room);
            let count_width = self.text_width(&count_style, &n);
            self.text(s, &count_style, &n, at, HAlign::Center, number);
            let room = MEDAL_LEVEL_RIGHT[col] - (at.0 + count_width * 0.5) - typography::TEXT_INSET;
            let at = (MEDAL_LEVEL_RIGHT[col], top);
            let style = self.career_fit(&m.level, room);
            self.text(s, &style, &m.level, at, HAlign::Right, text);
        }
        if let Some(m) = c.row(st.medal) {
            let flat = m.info.split_whitespace().collect::<Vec<_>>().join(" ");
            let lines = self.wrapped(&LIST, &flat, INFO_W);
            for (k, line) in lines.iter().enumerate() {
                let at = (INFO_AT.0, INFO_AT.1 + INFO_PITCH * k as f32);
                self.text(s, &LIST, line, at, HAlign::Left, WHITE);
            }
        }
        self.stats_help(size, false, Icon::Triangle, &st.texts.back);
        self.stats_help(size, true, Icon::Cross, &st.texts.done);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEXT_FONT: &str = "gamefont_tiny";
    const TITLE_FONT: &str = "gamefont_large";
    const SMALL_FONT: &str = "gamefont_small";
    use crate::hud_draw::{draw, HudArt, HudFrame};
    use crate::hud_tree::Hud;
    use crate::stats::{Award, Career, Medal, StatRow, StatsTexts, TeamList};
    use manifest::hud::{HudFont, HudGlyph, HudLayout};

    fn font(name: &str, line: u8) -> HudFont {
        HudFont {
            name: name.into(),
            line_height: line,
            pages: vec![format!("{name}_page")],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 9,
                    x0: 0,
                    x1: 9,
                    y0: 4,
                    y1: 12,
                    uv: [
                        f32::from(code) / 128.0,
                        f32::from(code + 1) / 128.0,
                        0.0,
                        1.0,
                    ],
                })
                .collect(),
        }
    }

    fn screens() -> StatsScreens {
        let row = |name: &str, points: i32, me: bool| StatRow {
            name: name.into(),
            points,
            kills: points,
            deaths: 1,
            posts: 0,
            me,
            human: me,
            personal: vec![("Kills".into(), points.to_string())],
            medals: Vec::new(),
        };
        let lists = [
            TeamList {
                icon: "cis_icon".into(),
                rows: (0..12)
                    .map(|i| row(&format!("UNIT {i:03}"), i, false))
                    .collect(),
            },
            TeamList {
                icon: "rep_icon".into(),
                rows: vec![row("PLAYER 1", 0, true), row("RHO 557", 3, false)],
            },
        ];
        let award = |l: &str, w: &str| Award {
            label: l.into(),
            winner: w.into(),
        };
        StatsScreens::new(
            StatsTexts {
                team_title: "Team Stats".into(),
                done: "Done".into(),
                ..Default::default()
            },
            lists,
            [
                vec![
                    award("Tank Buster", ""),
                    award("Camper", "RAYCE FARELLE (2:14)"),
                ],
                vec![award("Traitor", "KL 987 (2)")],
            ],
            Career {
                values: vec![("Kills".into(), "3".into())],
                medals: (0..9)
                    .map(|i| Medal {
                        name: format!("M{i}"),
                        info: "SIX KILLS WITH A PISTOL IN ONE LIFE EQUIPS YOU".into(),
                        ..Default::default()
                    })
                    .collect(),
                unlocks: Vec::new(),
            },
        )
    }

    fn frame(st: &StatsScreens) -> HudFrame {
        frame_at(st, [640.0, 480.0])
    }

    fn frame_at(st: &StatsScreens, size: [f32; 2]) -> HudFrame {
        let mut art = HudArt::default();
        art.fonts.insert(TEXT_FONT.into(), font(TEXT_FONT, 15));
        art.fonts.insert(TITLE_FONT.into(), font(TITLE_FONT, 18));
        art.fonts.insert(SMALL_FONT.into(), font(SMALL_FONT, 18));
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.stats = Some(Box::new(st.clone()));
        draw(&hud, &art, size)
    }

    fn sheets(f: &HudFrame) -> Vec<&str> {
        f.batches.iter().map(|b| b.texture.as_str()).collect()
    }

    /// Team Stats over black: both tabs and boxes, the emblems, the four
    /// column icons, the wedges, the cursor, the help icons; 9 rows of the
    /// long list, both of the short one.
    #[test]
    fn team_stats_draws_its_pieces() {
        let st = screens();
        let f = frame(&st);
        let s = sheets(&f);
        for want in [
            "white",
            "bf2_buttons_title_center",
            "border_3_pieces",
            "cis_icon",
            "rep_icon",
            "points",
            "stats_kills",
            "stats_deaths",
            "stats_cpp",
            "listbox_topbot_more",
            "listbox_cursor",
            "btna",
        ] {
            assert!(s.contains(&want), "{want}: {s:?}");
        }
        // The first quad is the black under everything, the whole window.
        assert_eq!(f.verts[0].pos[..2], [0.0, 0.0]);
        assert_eq!(f.verts[2].pos[..2], [640.0, 480.0]);
        assert_eq!(f.verts[0].color, [0.0, 0.0, 0.0, 1.0]);
    }

    /// At 640 × 480 the units are the PS2 frame's: the cursor's outline
    /// stands where `kam1_cw/20_teamstats.png` has it for the right list's
    /// first row (x 323.5 .. 610.5 px), the player's row white, the cursor
    /// row's name blue.
    #[test]
    fn the_cursor_and_the_rows_land_on_the_frames_places() {
        let st = screens();
        assert_eq!((st.list, st.row), (1, 1), "the player sorts second");
        let f = frame(&st);
        let cursor: Vec<_> = f
            .batches
            .iter()
            .filter(|b| b.texture == "listbox_cursor")
            .flat_map(|b| {
                f.indices[b.first as usize..(b.first + b.count) as usize]
                    .iter()
                    .map(|i| f.verts[*i as usize].pos)
            })
            .collect();
        let x0 = cursor.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let x1 = cursor.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        assert!(
            (x0 - 322.5).abs() < 1.5 && (x1 - 610.5).abs() < 1.5,
            "{x0} {x1}"
        );
        let y0 = cursor.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        // Row 2's ink top 113 px, the outline 6.5 px above it.
        assert!((y0 - 106.5).abs() < 1.0, "{y0}");
        let has = |rgb: [f32; 3]| {
            f.verts.iter().any(|v| {
                v.color[..3]
                    .iter()
                    .zip(rgb)
                    .all(|(a, b)| (a - b / 255.0).abs() < 1e-4)
            })
        };
        assert!(has(WHITE.0), "the player's row");
        assert!(has(BLUE.0), "the cursor row's name");
    }

    /// The text font's vertices: position and colour.
    fn text_verts(f: &HudFrame) -> Vec<([f32; 2], [f32; 4])> {
        let page = format!("{TEXT_FONT}_page");
        f.batches
            .iter()
            .filter(|b| b.texture == page)
            .flat_map(|b| {
                f.indices[b.first as usize..(b.first + b.count) as usize]
                    .iter()
                    .map(|i| {
                        let v = &f.verts[*i as usize];
                        ([v.pos[0], v.pos[1]], v.color)
                    })
            })
            .collect()
    }

    /// Issue #168: a human's name on Team Stats is bold, a bot's is not:
    /// the same glyphs once more, `BOLD_DX` units (here 1 px) to the right,
    /// in the row's own colour; nothing else on the page changes.
    #[test]
    fn team_stats_draws_human_names_bold() {
        let mut st = screens();
        let plain = text_verts(&frame(&st));
        // "RHO 557" (a bot until now, grey, not under the cursor) is a
        // second human; "PLAYER 1" (the viewer) was one already.
        let rho = &mut st.lists[1].rows[0];
        assert_eq!(rho.name, "RHO 557");
        rho.human = true;
        let bold = text_verts(&frame(&st));
        assert_eq!(
            bold.len(),
            plain.len() + 7 * 6,
            "7 glyphs, 2 triangles each"
        );
        let dx = BOLD_DX * 640.0 / 512.0;
        let mut extra: Vec<_> = bold.iter().filter(|v| !plain.contains(v)).collect();
        extra.dedup();
        assert!(!extra.is_empty());
        for (p, c) in extra {
            let copy = plain.iter().any(|(q, k)| {
                k == c && (q[0] + dx - p[0]).abs() < 1e-3 && (q[1] - p[1]).abs() < 1e-3
            });
            assert!(
                copy,
                "{p:?} is no copy of a plain glyph {dx} px left in {c:?}"
            );
            assert_eq!(c[..3], GREY.0.map(|v| v / 255.0), "same colour");
        }
        // The viewer's row is bold too: his name (under the cursor, blue)
        // has twice the vertices of the same name drawn plain.
        let blue = |v: &&([f32; 2], [f32; 4])| v.1[..3] == BLUE.0.map(|x| x / 255.0);
        let mut st = screens();
        st.lists[1].rows[1].human = false;
        let viewer_plain = text_verts(&frame(&st)).iter().filter(blue).count();
        let viewer_bold = plain.iter().filter(blue).count();
        assert_eq!(viewer_plain, "PLAYER 1".len() * 6);
        assert_eq!(viewer_bold, 2 * viewer_plain);
    }

    /// Square is the PS2's drawn outline only on the PS2's set; any other
    /// set's Done is its own icon.
    #[test]
    fn done_is_the_sets_own_icon() {
        use crate::input_icons::{IconSet, Icons};
        let mut art = HudArt::default();
        art.fonts.insert(TEXT_FONT.into(), font(TEXT_FONT, 15));
        art.fonts.insert(TITLE_FONT.into(), font(TITLE_FONT, 18));
        art.fonts.insert(SMALL_FONT.into(), font(SMALL_FONT, 18));
        art.icons = Icons::of(IconSet::XboxOne);
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.stats = Some(Box::new(screens()));
        let f = draw(&hud, &art, [640.0, 480.0]);
        let s = sheets(&f);
        assert!(
            s.contains(&"icon_xbone_x") && s.contains(&"icon_xbone_a"),
            "{s:?}"
        );
        assert!(!s.contains(&"btna"), "{s:?}");
        let pink = f.verts.iter().any(|v| {
            v.color[..3]
                .iter()
                .zip(SQUARE_PINK)
                .all(|(a, b)| (a - b / 255.0).abs() < 1e-4)
        });
        assert!(!pink, "the PS2's drawn Square stayed");
    }

    /// Personal Stats and Career Stats retain the player's rows and controls.
    #[test]
    fn personal_and_career_pages_draw() {
        let mut st = screens();
        st.page = Page::Personal;
        let f = frame(&st);
        assert!(sheets(&f).contains(&"btnb") && sheets(&f).contains(&"btna"));
        st.row = 0;
        let f = frame(&st);
        assert!(
            sheets(&f).contains(&"btna"),
            "Career Stats after browsing a bot on the scoreboard"
        );
        st.row = 1;
        st.page = Page::Career;
        let f = frame(&st);
        // Nine quads a box, six indices a quad.
        let quads: u32 = f
            .batches
            .iter()
            .filter(|b| b.texture == "border_3_pieces")
            .map(|b| b.count / 6)
            .sum();
        assert_eq!(quads, 27);
        assert!(sheets(&f).contains(&"listbox_cursor"));
        assert!(!sheets(&f).contains(&"listbox_topbot_more"), "no unlocks");
    }

    /// Decode the glyph quads emitted by the normal HUD draw path. Each
    /// synthetic glyph has its own UV, so an ellipsis cannot pass as a name.
    fn career_glyphs(
        f: &HudFrame,
        s: &Screen,
        col: usize,
        top: f32,
    ) -> Vec<(char, [f32; 4], [f32; 4])> {
        let page = format!("{TEXT_FONT}_page");
        let y = s.origin[1] + s.scale[1] * (top + 4.0);
        let x = s.origin[0] + s.scale[0] * (if col == 0 { 20.0 } else { 260.0 });
        let right = s.origin[0] + s.scale[0] * (if col == 0 { 252.0 } else { 490.0 });
        f.batches
            .iter()
            .filter(|b| b.texture == page)
            .flat_map(|b| f.indices[b.first as usize..(b.first + b.count) as usize].chunks_exact(6))
            .filter_map(|q| {
                let a = &f.verts[q[0] as usize];
                let b = &f.verts[q[2] as usize];
                ((a.pos[1] - y).abs() < 0.01 && a.pos[0] >= x && a.pos[0] < right).then(|| {
                    (
                        (a.uv[0] * 128.0).round() as u8 as char,
                        [a.pos[0], a.pos[1], b.pos[0], b.pos[1]],
                        a.color,
                    )
                })
            })
            .collect()
    }

    #[test]
    fn career_names_rank_keeps_complete_glyphs_and_clear_neighbors() {
        check_career_names(true);
    }

    #[test]
    fn career_names_medals_keep_complete_glyphs_and_clear_neighbors() {
        check_career_names(false);
    }

    fn check_career_names(rank_only: bool) {
        for size in [
            [640.0, 480.0],
            [1280.0, 720.0],
            [1920.0, 1080.0],
            [1280.0, 1024.0],
        ] {
            for (rank, tier) in [
                ("Private", "Legendary"),
                ("General", "Green"),
                ("Sergeant", "Veteran"),
                ("Captain", "Elite"),
                ("Generaloberst der Republik", "Legendenstatus der Republik"),
            ] {
                for count in [0, 64, 999, u32::MAX] {
                    for selected in [0, 5] {
                        let mut st = screens();
                        st.page = Page::Career;
                        st.medal = selected;
                        st.career.values =
                            vec![(String::new(), String::new()), ("Rank".into(), rank.into())];
                        for m in &mut st.career.medals {
                            m.name = "Gunslinger".into();
                            m.count = count;
                            m.level = tier.into();
                            m.info.clear();
                        }
                        let f = frame_at(&st, size);
                        let s = screen_at(size, Anchor::Centre);
                        if rank_only {
                            let glyphs = career_glyphs(&f, &s, 1, CAREER_LINE_TOP);
                            let actual: String = glyphs.iter().map(|g| g.0).collect();
                            assert_eq!(
                                actual,
                                format!("RANK{}", LIST.case.apply(rank)),
                                "rank at {size:?}"
                            );
                            assert!(
                                glyphs[3].1[2] + 4.9 * s.scale[0] <= glyphs[4].1[0],
                                "rank label gap"
                            );
                            assert_eq!(glyphs[4].2[..3], YELLOW.0.map(|v| v / 255.0));
                            if rank == "Private" {
                                assert!(
                                    (glyphs[4].1[2] - glyphs[4].1[0] - 9.9 * s.scale[0]).abs()
                                        < 0.01,
                                    "stock rank keeps its natural width"
                                );
                            }
                            let last = glyphs.last().unwrap().1;
                            assert!(
                                (last[2] - s.origin[0] - CAREER_VALUE_RIGHT[1] * s.scale[0]).abs()
                                    < 0.01
                            );
                            continue;
                        }
                        for (col, right) in MEDAL_LEVEL_RIGHT.iter().enumerate() {
                            let glyphs = career_glyphs(&f, &s, col, MEDAL_TOP);
                            let n = count.to_string();
                            let actual: String = glyphs.iter().map(|g| g.0).collect();
                            assert_eq!(
                                actual,
                                format!("GUNSLINGER{n}{}", LIST.case.apply(tier)),
                                "medal at {size:?} count={count}"
                            );
                            let name_end = glyphs[9].1[2];
                            let count_start = glyphs[10].1[0];
                            let count_end = glyphs[9 + n.len()].1[2];
                            let tier_start = glyphs[10 + n.len()].1[0];
                            assert!(
                                name_end + 4.9 * s.scale[0] <= count_start,
                                "name/count overlap: {name_end} {count_start}"
                            );
                            assert!(
                                count_end + 4.9 * s.scale[0] <= tier_start,
                                "count/tier overlap: {count_end} {tier_start}"
                            );
                            let focused = selected == col * 5;
                            let text_tint = if focused { BLUE } else { WHITE };
                            let number_tint = if focused { BLUE } else { YELLOW };
                            assert_eq!(glyphs[0].2[..3], text_tint.0.map(|v| v / 255.0));
                            assert_eq!(glyphs[10].2[..3], number_tint.0.map(|v| v / 255.0));
                            assert_eq!(glyphs[10 + n.len()].2[..3], text_tint.0.map(|v| v / 255.0));
                            let count_center = (count_start + count_end) * 0.5;
                            assert!(
                                (count_center - s.origin[0] - MEDAL_COUNT_X[col] * s.scale[0])
                                    .abs()
                                    < 0.01
                            );
                            let last = glyphs.last().unwrap().1;
                            assert!((last[2] - s.origin[0] - right * s.scale[0]).abs() < 0.01);
                            println!("{size:?} {rank}/{tier} {count} col={col}: name_end={name_end:.2}, count=[{count_start:.2},{count_end:.2}], tier=[{tier_start:.2},{:.2}]", last[2]);
                        }
                    }
                }
            }
        }
    }

    /// Card UNLK3: with unlocks the box has a down wedge on the medals' page
    /// and an up one on the unlocks' page, whose heading and rows it draws.
    #[test]
    fn the_unlocks_page_draws() {
        let mut st = screens();
        st.page = Page::Career;
        st.texts.career_medals = "MEDALSHEAD".into();
        st.texts.career_unlocks = "UNLOCKSHEAD".into();
        st.career.unlocks = (0..8)
            .map(|i| Medal {
                name: format!("U{i}"),
                ..Default::default()
            })
            .collect();
        let wedges = |st: &StatsScreens| {
            let f = frame(st);
            f.batches
                .iter()
                .filter(|b| b.texture == "listbox_topbot_more")
                .map(|b| b.count / 6)
                .sum::<u32>()
        };
        assert_eq!(wedges(&st), 2, "one wedge, two halves");
        st.medal = 9;
        assert_eq!(wedges(&st), 2);
        let f = frame(&st);
        let cursor_y = f
            .batches
            .iter()
            .filter(|b| b.texture == "listbox_cursor")
            .flat_map(|b| {
                f.indices[b.first as usize..(b.first + b.count) as usize]
                    .iter()
                    .map(|i| f.verts[*i as usize].pos[1])
            })
            .fold(f32::MAX, f32::min);
        // The first row's cursor, as the medals' first (at 640 × 480).
        let want = (MEDAL_TOP + MEDAL_CURSOR_Y[0]) * 480.0 / 448.0;
        assert!((cursor_y - want).abs() < 1.0, "{cursor_y} {want}");
    }
}
