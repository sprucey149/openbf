//! The Instant Action screen and Session Options drawn by the HUD pass in
//! the shell's 512 × 448 units (`analysis/re-bf2-menus.md` §1.2, §2.1-2.2;
//! the model is `crate::shell`).
//!
//! Positions come from the scripts' numbers (§1.2's table, safe area
//! 25.6, 22.4 + 460.8 × 403.2; `ScreenRelativeY 0` = the safe area's top),
//! except where a constant says `[M]`: measured on the PCSX2 frames
//! (`bshl/ref_frames/`, 640 × 480 = 1.25 × 1.0714 px per unit) because the
//! code route is not read (the command stack's origin, §9.1 item 8) or the
//! frame disagrees with it.
//!
//! `[ours]` Hor+ as the other shell pages: everything keeps the 4:3 box at
//! the window's centre, the help lines their edges.

use crate::hud_draw::{color, run_width, Ctx, Quad};
use crate::hud_tree::HAlign;
use crate::input_icons::Prompt;
use crate::shell::{
    InstantAction, ListRow, Page, Piece, State, COMMANDS, ERA_ROWS, LIST_ROWS, OPTION_ROWS,
};
use crate::shell_ps2::{BAR_PAD, CENTRE_LIFT, CENTRE_X, CONTAINER_Y, DIMMED_GREY};
use crate::spawn_draw::{
    item_pieces, screen_at, Anchor, BUTTON_FONT, LABEL_STRETCH, MAP_FONT, MAP_STRETCH, PIECE_ALPHA,
    ROW_H, ROW_PITCH, SELECTED_TEXT, SMALL_FONT, SMALL_STRETCH, TITLE_PIECES, TITLE_TEXT,
    UNSELECTED_TEXT,
};
use glam::Mat4;
use manifest::hud::HudFont;

/// The backdrop: `iface_instant_action`'s top 448 rows (§1.1, re-bf2-shell
/// §3.4).
pub const BACKDROP: &str = "iface_instant_action";
/// The lists' font on the PS2 (f34 pc 2-12).
pub const LIST_FONT: &str = MAP_FONT;
/// The safe area's top: `ScreenRelativeY 0` (§1.2 [M]).
const SAFE_TOP: f32 = 22.4;
/// The two list windows (f34 pc 97-190): centre y 175.5, 183 tall. Across,
/// the script's centre x 256 ∓ 146 / + 142 and width 172.8 put the borders
/// at 23.6..196.4 and 311.6..484.4; `[M]` the frames' outer borders are at
/// 19.2..196.0 and 312.8..489.6 (`ia_missionselect.png` row 230 px), drawn
/// so.
pub const LEFT_WINDOW: [f32; 4] = [
    19.2,
    SAFE_TOP + 175.5 - 91.5,
    196.0,
    SAFE_TOP + 175.5 + 91.5,
];
pub const RIGHT_WINDOW: [f32; 4] = [312.8, LEFT_WINDOW[1], 489.6, LEFT_WINDOW[3]];
/// The info box (f34 pc 191-240): y 309, 76 tall; across the script's x -2
/// and safe width 460.8 give 23.6..484.4, `[M]` the frames 19.2..489.6.
pub const INFO_BOX: [f32; 4] = [
    19.2,
    SAFE_TOP + 309.0 - 38.0,
    489.6,
    SAFE_TOP + 309.0 + 38.0,
];
/// Its text: x -220.4 (10 in from the box's left), y -34 from its centre,
/// 440.8 wide; `[M]` 10 in from the frames' box (ink from 29.6).
pub const INFO_TEXT: (f32, f32, f32) = (INFO_BOX[0] + 10.0, SAFE_TOP + 309.0 - 34.0, 440.8);
/// The list rows: 17 tall (Era 38) (f34 pc 28-63). The script makes them
/// 167.8 wide in a window of rows + 30; `[M]` the frames' focus box runs
/// from 4.8 inside the window's left border to 7.6 inside its right (164.4
/// wide), and the first row's top sits 16.6 below the window's top (Era:
/// 6.6; `ia_planet_list.png`, `ia_era.png`).
pub const ROW: f32 = 17.0;
pub const ERA_ROW: f32 = 38.0;
const LIST_IN: [f32; 2] = [4.8, 7.6];
const LIST_PAD: f32 = 16.6;
const ERA_PAD: f32 = 6.6;
/// A row's text: 4 in from the list's left (f0); `[M]` 2 from the box
/// (the frames' ink starts at 26.4).
const TEXT_X: f32 = 2.0;
/// Mode / era icons: 0.75 × 17 and 0.375 × 38 square, at the row's right
/// edge `[ours]` 5 in (the text's width - icon - 10 leaves 5 a side); the
/// playlist's era icon 8 left of the mode icon (f1, f2, f3).
const MODE_ICON: f32 = 12.75;
const ERA_ICON: f32 = 14.25;
const ICON_IN: f32 = 5.0;
const ICON_GAP: f32 = 8.0;
/// The focus box: the script's list width × row + 10 (listmanager f0 pc
/// 137-154); `[M]` the frames' box is 0.87 × the row tall, centred on it
/// (14.5 of 17, 32.7 of 38, 34.6 of an option row's 40).
const CURSOR_H: f32 = 0.87;
/// The scroll arrows: two mirrored `listbox_topbot_more` halves, x -32..2
/// and 32..-1 from the list's centre, 16 tall (listmanager f0); `[M]` the
/// down arrow's ink sits right under the last row (`ia_planet_list.png`).
const ARROW: [f32; 3] = [-32.0, 2.0, 16.0];
/// The window title bar: 28 tall, 4 above the window (buttonwindow f0);
/// `[M]` as wide as the window (the frames' bar spans the window, not the
/// script's width - 26).
const TITLE_H: f32 = 28.0;
const TITLE_GAP: f32 = 4.0;
/// `border_3_pieces`: 16-unit corners (buttonwindow main.f0) over 16 of the
/// sheet's 64 texels.
const BORDER: f32 = 16.0;
/// The command stack `[M]` (`ia_playlist_launch.png`; its route from `yTop
/// -130` is not read, §9.1 item 8): the bars' edges across, the first bar's
/// top, rows 21 tall (`gamefont_tiny` 15 + 6) every 25.
pub const STACK_X: [f32; 2] = [202.4, 306.4];
pub const STACK_TOP: f32 = 150.3;
const STACK_ROW: f32 = 21.0;
const STACK_PITCH: f32 = 25.0;

/// Hit-test the visible row geometry used by the Instant Action renderer.
pub fn pointer_target(
    ia: &InstantAction,
    point: [f32; 2],
    size: [f32; 2],
    font: &HudFont,
) -> Option<crate::shell::PointerTarget> {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return None;
    }
    let box_w = 4.0 / 3.0 * size[1];
    let x = (point[0] - (size[0] - box_w) * 0.5) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    match ia.page {
        Page::Missions => {
            if (STACK_X[0]..=STACK_X[1]).contains(&x) {
                for (k, &command) in COMMANDS.iter().enumerate() {
                    let y0 = STACK_TOP + STACK_PITCH * k as f32;
                    if (y0..=y0 + STACK_ROW).contains(&y) && ia.command_live(command) {
                        return Some(crate::shell::PointerTarget::Command(k));
                    }
                }
            }
            let play_rows = ia.play_rows();
            let rx0 = RIGHT_WINDOW[0] + LIST_IN[0];
            let rx1 = RIGHT_WINDOW[2] - LIST_IN[1];
            let rcx = (rx0 + rx1) * 0.5;
            let ry0 = RIGHT_WINDOW[1] + LIST_PAD;
            let rlast = ry0 + ROW * LIST_ROWS.min(play_rows.len()) as f32;
            if (rcx - 32.0..=rcx + 32.0).contains(&x) {
                if ia.play_list.top > 0 && (ry0 - 16.0..=ry0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollRight(-1));
                }
                if ia.play_list.top + LIST_ROWS < play_rows.len()
                    && (rlast..=rlast + 16.0).contains(&y)
                {
                    return Some(crate::shell::PointerTarget::ScrollRight(1));
                }
            }
            if ia.state == State::Playlist {
                let row = ((y - ry0) / ROW).floor() as isize + ia.play_list.top as isize;
                if x >= rx0 && x <= rx1 && y >= ry0 && row >= 0 {
                    let k = row as usize;
                    if k < play_rows.len() && k < ia.play_list.top + LIST_ROWS {
                        return Some(crate::shell::PointerTarget::Playlist(k));
                    }
                }
            }

            let (rows, cur, row_h, pad, target) = match ia.state {
                State::Mode => (
                    ia.mode_rows(),
                    ia.mode_list,
                    ROW,
                    LIST_PAD,
                    crate::shell::PointerTarget::Mode(0),
                ),
                State::Era => (
                    ia.side_rows(),
                    ia.era_list,
                    ERA_ROW,
                    ERA_PAD,
                    crate::shell::PointerTarget::Era(0),
                ),
                _ => (
                    ia.map_rows(),
                    ia.map_list,
                    ROW,
                    LIST_PAD,
                    crate::shell::PointerTarget::Map(0),
                ),
            };
            let shown = if ia.state == State::Era {
                ERA_ROWS
            } else {
                LIST_ROWS
            };
            let lx0 = LEFT_WINDOW[0] + LIST_IN[0];
            let lx1 = LEFT_WINDOW[2] - LIST_IN[1];
            let lcx = (lx0 + lx1) * 0.5;
            let ly0 = LEFT_WINDOW[1] + pad;
            let llast = ly0 + row_h * shown.min(rows.len()) as f32;
            if (lcx - 32.0..=lcx + 32.0).contains(&x) {
                if cur.top > 0 && (ly0 - 16.0..=ly0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollLeft(-1));
                }
                if cur.top + shown < rows.len() && (llast..=llast + 16.0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollLeft(1));
                }
            }
            if !matches!(ia.state, State::Command | State::Playlist) {
                let row = ((y - ly0) / row_h).floor() as isize + cur.top as isize;
                if x >= lx0 && x <= lx1 && y >= ly0 && row >= 0 {
                    let k = row as usize;
                    if k < rows.len() && k < cur.top + shown && rows[k].enabled {
                        return Some(match target {
                            crate::shell::PointerTarget::Map(_) => {
                                crate::shell::PointerTarget::Map(k)
                            }
                            crate::shell::PointerTarget::Mode(_) => {
                                crate::shell::PointerTarget::Mode(k)
                            }
                            _ => crate::shell::PointerTarget::Era(k),
                        });
                    }
                }
            }
            None
        }
        Page::Overview(focus) => {
            let labels: Vec<_> = ia
                .overview()
                .into_iter()
                .map(|(g, _)| ia.texts.group(g).0)
                .collect();
            let widest = std::iter::once(ia.texts.session_options.as_str())
                .chain(labels.iter().copied())
                .map(|label| run_width(font, label, LABEL_STRETCH, false))
                .fold(0.0, f32::max);
            let w = widest + BAR_PAD;
            let x0 = CENTRE_X - w * 0.5;
            let bars = labels.len() + 1;
            let height = bars as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
            let top = CONTAINER_Y - CENTRE_LIFT - height * 0.5;
            for (k, (_, live)) in ia.overview().iter().enumerate() {
                let y0 = top + ROW_PITCH * (k + 1) as f32;
                if *live && x >= x0 && x <= x0 + w && y >= y0 && y <= y0 + ROW_H {
                    return Some(crate::shell::PointerTarget::Overview(k));
                }
            }
            let _ = focus;
            None
        }
        Page::Group(g, cur) => {
            let cx = (OPTIONS_WINDOW[0] + OPTIONS_WINDOW[2]) * 0.5;
            let x0 = cx - OPTION_LIST_W * 0.5;
            let x1 = cx + OPTION_LIST_W * 0.5;
            let y0 = OPTIONS_WINDOW[1] + OPTION_PAD;
            let rows = ia.options.rows(g);
            let last = y0 + OPTION_ROW * OPTION_ROWS as f32;
            if (cx - 32.0..=cx + 32.0).contains(&x) {
                if cur.top > 0 && (y0 - 16.0..=y0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollOption(-1));
                }
                if cur.top + OPTION_ROWS < rows.len() && (last..=last + 16.0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollOption(1));
                }
            }
            let row = ((y - y0) / OPTION_ROW).floor() as isize + cur.top as isize;
            if x >= x0 && x <= x1 && y >= y0 && row >= cur.top as isize {
                let k = row as usize;
                if k < rows.len() && k < cur.top + OPTION_ROWS {
                    let centre = (x0 + x1) * 0.5;
                    return Some(crate::shell::PointerTarget::Option(k, x >= centre));
                }
            }
            None
        }
    }
}
/// The bar tint: white focused, grey 150 otherwise (`analysis/re-bf2-menus.md`
/// §5).
const BAR_GREY: f32 = 150.0;

/// Session Options' group page (`ifs_instant_options` main.f22): the window
/// 0.85 × the safe width (391.7), 5 rows of 40 + 30, centred at y 145;
/// `[M]` its borders across are at 56.0..457.6 on `options_conquest.png`
/// (the script's 60.2..451.9), drawn so.
pub const OPTIONS_WINDOW: [f32; 4] = [
    56.0,
    SAFE_TOP + 145.0 - 115.0,
    457.6,
    SAFE_TOP + 145.0 + 115.0,
];
const OPTION_ROW: f32 = 40.0;
/// Rows' width: the window - 40 + 30 (PS2), text 10 in; `[M]` the focus box
/// 64.0..447.6 (`options_conquest.png`).
const OPTION_LIST_W: f32 = 383.6;
const OPTION_TEXT_X: f32 = 10.0;
/// The first option row's top below the window's (`[M]` 15.8: the box's top
/// at 70.9).
const OPTION_PAD: f32 = 15.8;
/// `{OptionL}` / `{OptionR}`: `optionl` / `optionr` drawn this square
/// `[M]` (their ink 8 px wide on the frames).
const ARROW_ICON: f32 = 14.0;
/// The grey help box: `white_rect` (128, 128, 128) a 0.5, 65 tall, from the
/// safe area's left + 4 to its width - 8 at y 267.5 (main.f22 pc 61-106);
/// `[M]` its top is the container's y (291 on the frames).
pub const OPTION_INFO: [f32; 4] = [
    25.6 + 4.0,
    SAFE_TOP + 267.5,
    25.6 + 460.8 - 4.0,
    SAFE_TOP + 267.5 + 65.0,
];
/// The help text: 10 in, 65 / 4 - 10 down; the script names no font (nil).
/// `[M]` the PS2 draws it in the rows' face and size (`gamefont_small`,
/// mixed case, white), lines 19.6 units apart (menuref §9.1.4, "Session
/// Options": 21 px).
const OPTION_INFO_TEXT: (f32, f32) = (10.0, 65.0 / 4.0 - 10.0);
const OPTION_INFO_PITCH: f32 = 19.6;
const OPTION_INFO_STYLE: ([f32; 3], f32) = ([255.0; 3], 1.0);

/// The "list full" popup `[ours]` geometry (`Popup_Ok`'s is not read).
const POPUP: [f32; 4] = [136.0, 184.0, 376.0, 264.0];

const NONE: usize = usize::MAX;

/// A font's stretch across (the shell's labels, `crate::spawn_draw`).
fn stretch(font: &str) -> f32 {
    match font {
        BUTTON_FONT => LABEL_STRETCH,
        MAP_FONT => MAP_STRETCH,
        _ => SMALL_STRETCH,
    }
}

type Style = ([f32; 3], f32);

/// A list row's colour: the row at the list's index blue, the rest white,
/// `[ours]` dimmed grey 110 (§7).
fn row_style(row: &ListRow, at: bool) -> Style {
    if !row.enabled {
        ([DIMMED_GREY; 3], UNSELECTED_TEXT.1)
    } else if at {
        SELECTED_TEXT
    } else {
        UNSELECTED_TEXT
    }
}

/// `text` broken into lines no wider than `width` at the font's stretch:
/// the disc's line ends first, then words.
fn wrap(font: &manifest::hud::HudFont, text: &str, width: f32) -> Vec<String> {
    let kx = stretch(&font.name);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let para = para.trim_end_matches('\r').trim_end();
        let mut line = String::new();
        for word in para.split(' ').filter(|w| !w.is_empty()) {
            let next = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && run_width(font, &next, kx, false) > width {
                out.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = next;
            }
        }
        out.push(line);
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

impl Ctx<'_> {
    fn tinted(
        &mut self,
        s: &crate::hud_draw::Screen,
        rect: [f32; 4],
        uv: [f32; 4],
        texture: &str,
        (rgb, alpha): Style,
    ) {
        self.quad(
            NONE,
            &Mat4::IDENTITY,
            s,
            &Quad {
                rect,
                uv,
                color: color(rgb, alpha),
                texture,
                additive: false,
                shadow: false,
                boxed: false,
            },
        );
    }

    /// A nine-part box, untinted: `corner` units over `t` of the sheet's uv
    /// at each corner, the edges stretched.
    pub(crate) fn nine(
        &mut self,
        s: &crate::hud_draw::Screen,
        [x0, y0, x1, y1]: [f32; 4],
        uv: [f32; 4],
        (corner, t): (f32, f32),
        texture: &str,
    ) {
        let style = ([255.0; 3], 1.0);
        let xs = [x0, x0 + corner, x1 - corner, x1];
        let ys = [y0, y0 + corner, y1 - corner, y1];
        let us = [uv[0], uv[0] + t, uv[2] - t, uv[2]];
        let vs = [uv[1], uv[1] + t, uv[3] - t, uv[3]];
        for j in 0..3 {
            for i in 0..3 {
                self.tinted(
                    s,
                    [xs[i], ys[j], xs[i + 1], ys[j + 1]],
                    [us[i], vs[j], us[i + 1], vs[j + 1]],
                    texture,
                    style,
                );
            }
        }
    }

    /// `NewButtonWindow`: the `border_3_pieces` box and, with a title, its
    /// bar above (`ifelem_buttonwindow`).
    fn window_box(&mut self, s: &crate::hud_draw::Screen, r: [f32; 4], title: Option<&str>) {
        let full = [0.0, 0.0, 1.0, 1.0];
        self.nine(s, r, full, (BORDER, BORDER / 64.0), "border_3_pieces");
        let Some(title) = title else {
            return;
        };
        let bar = [r[0], r[1] - TITLE_GAP - TITLE_H, r[2], r[1] - TITLE_GAP];
        self.bar_grey(s, bar, TITLE_PIECES, 255.0);
        let lh = self.font_h(BUTTON_FONT);
        let at = ((bar[0] + bar[2]) * 0.5, bar[1] + (TITLE_H - lh) * 0.5);
        self.label(s, BUTTON_FONT, title, at, HAlign::Center, TITLE_TEXT);
    }

    fn bar_grey(
        &mut self,
        s: &crate::hud_draw::Screen,
        [x0, y0, x1, y1]: [f32; 4],
        pieces: [&str; 3],
        grey: f32,
    ) {
        let h = y1 - y0;
        let full = [0.0, 0.0, 1.0, 1.0];
        let st = ([grey; 3], PIECE_ALPHA);
        self.tinted(s, [x0, y0, x0 + h, y1], full, pieces[0], st);
        self.tinted(s, [x0 + h, y0, x1 - h, y1], full, pieces[1], st);
        self.tinted(s, [x1 - h, y0, x1, y1], full, pieces[2], st);
    }

    /// A list row's text, left-aligned; `[ours]` squeezed across to fit
    /// `width` (the merged list's "<name> (bfN)" rows outgrow BF2's own
    /// names: "BESPIN: CLOUD CITY (bf1)").
    fn label_fit(
        &mut self,
        s: &crate::hud_draw::Screen,
        text: &str,
        at: (f32, f32),
        width: f32,
        (rgb, alpha): Style,
    ) {
        let Some(font) = self.art.fonts.get(LIST_FONT) else {
            return;
        };
        let kx = stretch(LIST_FONT);
        let w = run_width(font, text, kx, false);
        let k = if w > width { kx * width / w } else { kx };
        self.run(
            NONE,
            &Mat4::IDENTITY,
            s,
            font,
            &crate::hud_draw::Run {
                text,
                at,
                h_align: HAlign::Left,
                scale: [k, 1.0],
                color: color(rgb, alpha),
                additive: false,
                shadow: false,
            },
        );
    }

    fn font_h(&self, font: &str) -> f32 {
        self.art
            .fonts
            .get(font)
            .map_or(15.0, |f| f32::from(f.line_height))
    }

    /// `listbox_cursor` around a row: its line sits at texels 3..61 × 4..54
    /// of the sheet, 2 texels thick; drawn with thin corners `[ours]`.
    pub(crate) fn cursor_box(&mut self, s: &crate::hud_draw::Screen, r: [f32; 4]) {
        let uv = [3.0 / 64.0, 4.0 / 64.0, 61.0 / 64.0, 54.0 / 64.0];
        self.nine(s, r, uv, (4.0, 6.0 / 64.0), "listbox_cursor");
    }

    /// One list in a window: its rows from `top`, the cursor box when the
    /// list has the focus, the scroll arrows when it scrolls.
    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        s: &crate::hud_draw::Screen,
        win: [f32; 4],
        rows: &[ListRow],
        at: usize,
        top: usize,
        shown: usize,
        row_h: f32,
        focused: bool,
    ) {
        let x0 = win[0] + LIST_IN[0];
        let x1 = win[2] - LIST_IN[1];
        let cx = (x0 + x1) * 0.5;
        let era = row_h > ROW;
        let y0 = win[1] + if era { ERA_PAD } else { LIST_PAD };
        let lh = self.font_h(LIST_FONT);
        let icon = if era { ERA_ICON } else { MODE_ICON };
        for (k, row) in rows.iter().enumerate().skip(top).take(shown) {
            let y = y0 + row_h * (k - top) as f32;
            let style = row_style(row, k == at);
            if focused && k == at {
                let m = row_h * (1.0 - CURSOR_H) * 0.5;
                self.cursor_box(s, [x0, y + m, x1, y + row_h - m]);
            }
            let icons = match (row.icon, row.icon2) {
                (Some(_), Some(_)) => ICON_IN + 2.0 * icon + ICON_GAP,
                (Some(_), None) => ICON_IN + icon,
                _ => 0.0,
            };
            let width = x1 - x0 - TEXT_X * 2.0 - icons;
            let lines = match self.art.fonts.get(LIST_FONT) {
                Some(f) if era => wrap(f, &row.text, width),
                _ => vec![row.text.clone()],
            };
            let ty = y + (row_h - lh * lines.len() as f32) * 0.5;
            for (j, line) in lines.iter().enumerate() {
                let at_xy = (x0 + TEXT_X, ty + lh * j as f32);
                self.label_fit(s, line, at_xy, width, style);
            }
            let iy = y + (row_h - icon) * 0.5;
            if let Some(tex) = row.icon {
                let ix = x1 - ICON_IN - icon;
                // Grey 128 under the row's colour (f2 pc 57; the frames'
                // icons are half the text's brightness [M]).
                let half = (style.0.map(|c| c * 128.0 / 255.0), style.1);
                self.tinted(
                    s,
                    [ix, iy, ix + icon, iy + icon],
                    [0.0, 0.0, 1.0, 1.0],
                    tex,
                    half,
                );
                if let Some(tex2) = row.icon2 {
                    // The playlist's era icon: grey 128 (f3 pc 64-89).
                    let ex = ix - ICON_GAP - icon;
                    let grey = ([128.0; 3], style.1);
                    self.tinted(
                        s,
                        [ex, iy, ex + icon, iy + icon],
                        [0.0, 0.0, 1.0, 1.0],
                        tex2,
                        grey,
                    );
                }
            }
        }
        let last = y0 + row_h * shown.min(rows.len()) as f32;
        self.more_arrows(s, cx, [y0, last], top > 0, top + shown < rows.len());
    }

    /// A list's scroll arrows: above the rows from `[top, bottom]` when rows are
    /// hidden up there, below them when rows are hidden down there.
    fn more_arrows(
        &mut self,
        s: &crate::hud_draw::Screen,
        cx: f32,
        [top, bottom]: [f32; 2],
        above: bool,
        below: bool,
    ) {
        let [a0, a1, ah] = ARROW;
        let style = ([255.0; 3], 1.0);
        if above {
            let r = [top - ah, top];
            self.tinted(
                s,
                [cx + a0, r[0], cx + a1, r[1]],
                [0.0, 0.0, 1.0, 1.0],
                "listbox_topbot_more",
                style,
            );
            self.tinted(
                s,
                [cx - a1, r[0], cx - a0, r[1]],
                [1.0, 0.0, 0.0, 1.0],
                "listbox_topbot_more",
                style,
            );
        }
        if below {
            let r = [bottom, bottom + ah];
            self.tinted(
                s,
                [cx + a0, r[0], cx + a1, r[1]],
                [0.0, 1.0, 1.0, 0.0],
                "listbox_topbot_more",
                style,
            );
            self.tinted(
                s,
                [cx - a1, r[0], cx - a0, r[1]],
                [1.0, 1.0, 0.0, 0.0],
                "listbox_topbot_more",
                style,
            );
        }
    }

    /// The Instant Action screen or its Session Options page over the
    /// shell's (the template's help lines included); `pulse` is the focused
    /// button's alpha (`crate::shell_feel::Pulse::BUTTON`).
    pub(crate) fn instant_page(&mut self, ia: &InstantAction, pulse: f32, size: [f32; 2]) {
        match ia.page {
            Page::Missions => self.missions(ia, pulse, size),
            Page::Overview(focus) => self.overview(ia, focus, pulse, size),
            Page::Group(..) => self.group(ia, size),
        }
    }

    fn missions(&mut self, ia: &InstantAction, pulse: f32, size: [f32; 2]) {
        let s = screen_at(size, Anchor::Centre);
        let t = &ia.texts;
        self.tinted(
            &s,
            [0.0, 0.0, 512.0, 448.0],
            [0.0, 0.0, 1.0, 0.875],
            BACKDROP,
            ([255.0; 3], 1.0),
        );
        if ia.full {
            self.window_box(&s, POPUP, None);
            if let Some(f) = self.art.fonts.get(SMALL_FONT) {
                let lh = f32::from(f.line_height);
                let lines = wrap(f, &ia.texts.list_full, POPUP[2] - POPUP[0] - 20.0);
                let y = (POPUP[1] + POPUP[3]) * 0.5 - lh * lines.len() as f32 * 0.5;
                for (j, l) in lines.iter().enumerate() {
                    let at = (256.0, y + lh * j as f32);
                    self.label(&s, SMALL_FONT, l, at, HAlign::Center, UNSELECTED_TEXT);
                }
            }
            self.help(size, true, Prompt::Accept, &t.select);
            return;
        }
        // The left window: Planet, or Mode / Era in their states (f12).
        let (title, rows, cur, shown, row_h) = match ia.state {
            State::Mode => (&t.mode, ia.mode_rows(), ia.mode_list, LIST_ROWS, ROW),
            State::Era => (&t.era, ia.side_rows(), ia.era_list, ERA_ROWS, ERA_ROW),
            _ => (&t.planet, ia.map_rows(), ia.map_list, LIST_ROWS, ROW),
        };
        let left_focus = matches!(ia.state, State::Map | State::Mode | State::Era);
        self.window_box(&s, LEFT_WINDOW, Some(title));
        self.list(
            &s,
            LEFT_WINDOW,
            &rows,
            cur.at,
            cur.top,
            shown,
            row_h,
            left_focus,
        );
        self.window_box(&s, RIGHT_WINDOW, Some(&t.playlist));
        let play = ia.play_rows();
        let pc = ia.play_list;
        let focus = ia.state == State::Playlist;
        self.list(
            &s,
            RIGHT_WINDOW,
            &play,
            pc.at,
            pc.top,
            LIST_ROWS,
            ROW,
            focus,
        );
        // The command stack: square notched bars, `gamefont_tiny` labels
        // (main pc 87); the kept focus blue and pulsing even while a list
        // has it (f11; menuref §9.1.9: ADD pulses beside the solid list
        // cursor).
        let lh = self.font_h(LIST_FONT);
        for (k, c) in COMMANDS.iter().enumerate() {
            let y = STACK_TOP + STACK_PITCH * k as f32;
            let r = [STACK_X[0], y, STACK_X[1], y + STACK_ROW];
            if k == ia.command {
                self.bar_tinted(&s, r, item_pieces(false), ([255.0; 3], pulse));
            } else {
                self.bar_grey(&s, r, item_pieces(false), BAR_GREY);
            }
            let style = if k == ia.command {
                (SELECTED_TEXT.0, pulse)
            } else if ia.command_live(*c) {
                UNSELECTED_TEXT
            } else {
                ([DIMMED_GREY; 3], UNSELECTED_TEXT.1)
            };
            let at = ((r[0] + r[2]) * 0.5, y + (STACK_ROW - lh) * 0.5);
            self.label(&s, LIST_FONT, t.command(*c).0, at, HAlign::Center, style);
        }
        // The info box (f9).
        self.window_box(&s, INFO_BOX, None);
        if let Some(f) = self.art.fonts.get(LIST_FONT) {
            let (x, y, w) = INFO_TEXT;
            let lh = f32::from(f.line_height);
            for (j, line) in wrap(f, &ia.info(), w).iter().enumerate() {
                let at = (x, y + lh * j as f32);
                self.label(&s, LIST_FONT, line, at, HAlign::Left, SELECTED_TEXT);
            }
        }
        self.help(size, false, Prompt::Back, &t.back);
        self.help(size, true, Prompt::Accept, &t.select);
    }

    /// "Session Options": the PS2 stack rule of `crate::shell_ps2`
    /// (`stack_layout`'s [M] widths), centred; dimmed groups grey (§2.1).
    fn overview(&mut self, ia: &InstantAction, focus: usize, pulse: f32, size: [f32; 2]) {
        let s = screen_at(size, Anchor::Centre);
        let t = &ia.texts;
        let Some(font) = self.art.fonts.get(BUTTON_FONT) else {
            return;
        };
        let rows: Vec<(&str, bool)> = ia
            .overview()
            .into_iter()
            .map(|(g, live)| (t.group(g).0, live))
            .collect();
        let widest = std::iter::once(t.session_options.as_str())
            .chain(rows.iter().map(|(l, _)| *l))
            .map(|l| run_width(font, l, LABEL_STRETCH, false))
            .fold(0.0, f32::max);
        let lh = f32::from(font.line_height);
        let w = widest + BAR_PAD;
        let x0 = CENTRE_X - w * 0.5;
        let bars = rows.len() + 1;
        let height = bars as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
        let top = CONTAINER_Y - CENTRE_LIFT - height * 0.5;
        let bar = |k: usize| {
            let y = top + ROW_PITCH * k as f32;
            [x0, y, x0 + w, y + ROW_H]
        };
        let mid = |r: [f32; 4]| ((r[0] + r[2]) * 0.5, r[1] + (ROW_H - lh) * 0.5);
        self.bar_grey(&s, bar(0), TITLE_PIECES, 255.0);
        let title = &t.session_options;
        self.label(
            &s,
            BUTTON_FONT,
            title,
            mid(bar(0)),
            HAlign::Center,
            TITLE_TEXT,
        );
        for (k, (label, live)) in rows.iter().enumerate() {
            let r = bar(k + 1);
            let pieces = item_pieces(k + 1 == rows.len());
            if k == focus {
                self.bar_tinted(&s, r, pieces, ([255.0; 3], pulse));
            } else {
                self.bar_grey(&s, r, pieces, BAR_GREY);
            }
            let style = if !live {
                ([DIMMED_GREY; 3], UNSELECTED_TEXT.1)
            } else if k == focus {
                (SELECTED_TEXT.0, pulse)
            } else {
                UNSELECTED_TEXT
            };
            self.label(&s, BUTTON_FONT, label, mid(r), HAlign::Center, style);
        }
        self.help(size, false, Prompt::Back, &t.back);
        self.help(size, true, Prompt::Accept, &t.select);
    }

    /// A group's page: the window of option rows "Label: < value >", the
    /// focus box, the grey help box; Back only (§2.2).
    fn group(&mut self, ia: &InstantAction, size: [f32; 2]) {
        let Page::Group(g, cur) = ia.page else {
            return;
        };
        let s = screen_at(size, Anchor::Centre);
        let t = &ia.texts;
        let win = OPTIONS_WINDOW;
        self.window_box(&s, win, Some(t.group(g).1));
        let cx = (win[0] + win[2]) * 0.5;
        let x0 = cx - OPTION_LIST_W * 0.5;
        let x1 = cx + OPTION_LIST_W * 0.5;
        let y0 = win[1] + OPTION_PAD;
        let lh = self.font_h(SMALL_FONT);
        let kx = stretch(SMALL_FONT);
        let rows = ia.options.rows(g);
        for (k, o) in rows.iter().enumerate().skip(cur.top).take(OPTION_ROWS) {
            let y = y0 + OPTION_ROW * (k - cur.top) as f32;
            if k == cur.at {
                let m = OPTION_ROW * (1.0 - CURSOR_H) * 0.5;
                self.cursor_box(&s, [x0, y + m, x1, y + OPTION_ROW - m]);
            }
            // Every row is white; the focused one blue (main.f1, the frames).
            let style = if k == cur.at {
                SELECTED_TEXT
            } else {
                UNSELECTED_TEXT
            };
            let ty = y + (OPTION_ROW - lh) * 0.5;
            let mut x = x0 + OPTION_TEXT_X;
            for p in t.option(*o, &ia.options) {
                match p {
                    Piece::Text(text) => {
                        self.label(&s, SMALL_FONT, &text, (x, ty), HAlign::Left, style);
                        if let Some(f) = self.art.fonts.get(SMALL_FONT) {
                            x += run_width(f, &text, kx, false);
                        }
                    }
                    Piece::Left | Piece::Right => {
                        let tex = if p == Piece::Left {
                            "optionl"
                        } else {
                            "optionr"
                        };
                        let iy = y + (OPTION_ROW - ARROW_ICON) * 0.5;
                        let r = [x, iy, x + ARROW_ICON, iy + ARROW_ICON];
                        self.tinted(&s, r, [0.0, 0.0, 1.0, 1.0], tex, ([255.0; 3], 1.0));
                        x += ARROW_ICON;
                    }
                }
            }
        }
        // [ours] Only the Hero page, with the DLC row, holds more than fit.
        let last = y0 + OPTION_ROW * OPTION_ROWS as f32;
        let (above, below) = (cur.top > 0, cur.top + OPTION_ROWS < rows.len());
        self.more_arrows(&s, cx, [y0, last], above, below);
        let info = OPTION_INFO;
        let grey = ([128.0; 3], 0.5);
        self.tinted(&s, info, [0.0, 0.0, 1.0, 1.0], "white_rect", grey);
        if let (Some(o), Some(f)) = (rows.get(cur.at), self.art.fonts.get(SMALL_FONT)) {
            let (dx, dy) = OPTION_INFO_TEXT;
            let w = info[2] - info[0] - 2.0 * dx;
            let help = ia
                .options
                .population_help(*o)
                .unwrap_or_else(|| t.help(*o).to_owned());
            for (j, line) in wrap(f, &help, w).iter().enumerate() {
                let y = info[1] + dy.max(4.0) + OPTION_INFO_PITCH * j as f32;
                self.label(
                    &s,
                    SMALL_FONT,
                    line,
                    (info[0] + dx, y),
                    HAlign::Left,
                    OPTION_INFO_STYLE,
                );
            }
        }
        self.help(size, false, Prompt::Back, &t.back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_hit_detects_scroll_arrows() {
        use crate::shell::{Game, IaTexts, MapEntry, Mode, SessionOptions};
        use manifest::hud::{HudFont, HudGlyph};
        let maps: Vec<_> = (0..20)
            .map(|i| MapEntry {
                key: format!("test{i}"),
                game: Game::Bf2,
                code: "t",
                label: format!("Test {i}"),
                description: String::new(),
                modes: [vec![Mode::Con], Vec::new()],
                baked: true,
                missions: vec![(Mode::Con, ["rep".into(), "cis".into()])],
            })
            .collect();
        let mut ia = InstantAction::new(
            maps,
            IaTexts::new(&|_, e| e.to_owned()),
            SessionOptions::default(),
        );
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 15,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 8,
                    x0: 0,
                    x1: 8,
                    y0: 2,
                    y1: 12,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let cx = (LEFT_WINDOW[0] + LIST_IN[0] + LEFT_WINDOW[2] - LIST_IN[1]) * 0.5;
        let y0 = LEFT_WINDOW[1] + LIST_PAD;
        let last = y0 + ROW * LIST_ROWS as f32;
        let to_screen = |mx: f32, my: f32| [mx * (640.0 / 512.0), my * (480.0 / 448.0)];

        // At top: no up arrow, but down arrow is present
        assert_eq!(ia.map_list.top, 0);
        let hit_up = pointer_target(&ia, to_screen(cx, y0 - 8.0), [640.0, 480.0], &font);
        assert_eq!(hit_up, None, "no up arrow when at top");

        let hit_down = pointer_target(&ia, to_screen(cx, last + 8.0), [640.0, 480.0], &font);
        assert_eq!(
            hit_down,
            Some(crate::shell::PointerTarget::ScrollLeft(1)),
            "down arrow is hit"
        );

        // Scroll down: now up arrow is present
        ia.scroll_left(1);
        assert_eq!(ia.map_list.top, 1);
        let hit_up_scrolled = pointer_target(&ia, to_screen(cx, y0 - 8.0), [640.0, 480.0], &font);
        assert_eq!(
            hit_up_scrolled,
            Some(crate::shell::PointerTarget::ScrollLeft(-1)),
            "up arrow is hit when scrolled"
        );

        // Editing the playlist: the map rows do not take the hover or click.
        ia.state = State::Playlist;
        let row = pointer_target(&ia, to_screen(cx, y0 + ROW * 0.5), [640.0, 480.0], &font);
        assert_eq!(
            row, None,
            "the left rows are inert while the playlist has focus"
        );
    }

    #[test]
    fn mouse_hit_selects_the_visible_instant_action_row() {
        use crate::shell::{Game, IaTexts, MapEntry, Mode, SessionOptions};
        let map = MapEntry {
            key: "test".into(),
            game: Game::Bf2,
            code: "t",
            label: "Test map".into(),
            description: String::new(),
            modes: [vec![Mode::Con], Vec::new()],
            baked: true,
            missions: vec![(Mode::Con, ["rep".into(), "cis".into()])],
        };
        let mut ia = InstantAction::new(
            vec![map],
            IaTexts::new(&|_, e| e.to_owned()),
            SessionOptions::default(),
        );
        ia.state = State::Map;
        let x = (LEFT_WINDOW[0] + LIST_IN[0] + LEFT_WINDOW[2] - LIST_IN[1]) * 0.5;
        let y = LEFT_WINDOW[1] + LIST_PAD + ROW * 0.5;
        let point = [x * 1.25, y * (480.0 / 448.0)];
        use manifest::hud::{HudFont, HudGlyph};
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 15,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 8,
                    x0: 0,
                    x1: 8,
                    y0: 2,
                    y1: 12,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let hit = pointer_target(&ia, point, [640.0, 480.0], &font);
        assert_eq!(hit, Some(crate::shell::PointerTarget::Map(0)));
        assert!(ia.focus_pointer(hit.unwrap()));
        assert_eq!(ia.map_list.at, 0);
    }

    #[test]
    fn the_windows_sit_where_the_script_puts_them() {
        // §1.2: the Planet window's body 106.4..289.4 (the frame 109..287).
        assert!((LEFT_WINDOW[1] - 106.4).abs() < 0.01);
        assert!((LEFT_WINDOW[3] - 289.4).abs() < 0.01);
        assert_eq!(RIGHT_WINDOW[1], LEFT_WINDOW[1]);
        // Nine rows of 17 + 30 fill the window.
        assert!(LIST_PAD + LIST_ROWS as f32 * ROW < 183.0);
        assert!((INFO_BOX[1] - (22.4 + 309.0 - 38.0)).abs() < 0.01);
    }

    /// The screen as the HUD pass lays it out: the backdrop, the windows,
    /// the focus box, the help icons; a dimmed map row drawn grey 110.
    #[test]
    fn the_screen_draws_its_pieces_and_dims_rows() {
        use crate::hud_draw::HudArt;
        use crate::hud_tree::Hud;
        use crate::shell::{IaTexts, InstantAction, MapEntry, Mode, SessionOptions, State};
        use manifest::hud::{HudFont, HudGlyph, HudLayout};
        let font = |name: &str| HudFont {
            name: name.into(),
            line_height: 15,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 8,
                    x0: 0,
                    x1: 8,
                    y0: 2,
                    y1: 12,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut art = HudArt::default();
        for f in [LIST_FONT, SMALL_FONT, BUTTON_FONT] {
            art.fonts.insert(f.into(), font(f));
        }
        let entry = |label: &str, baked: bool| MapEntry {
            key: label.into(),
            game: crate::shell::Game::Bf2,
            code: "x",
            label: label.into(),
            description: String::new(),
            modes: [vec![Mode::Con], Vec::new()],
            baked,
            missions: vec![(Mode::Con, ["rep".into(), "cis".into()])],
        };
        let maps = vec![entry("AAA", true), entry("ZZZ", false)];
        let mut ia = InstantAction::new(
            maps,
            IaTexts::new(&|_, e| e.to_owned()),
            SessionOptions::default(),
        );
        ia.state = State::Map;
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        let mut menus = crate::shell_ps2::ShellMenus::new(&|_, e| e.to_owned());
        menus.covered = true;
        hud.shell = Some((menus, false));
        hud.instant = Some(Box::new(ia));
        let f = crate::hud_draw::draw(&hud, &art, [640.0, 480.0]);
        let sheets: Vec<&str> = f.batches.iter().map(|b| b.texture.as_str()).collect();
        for p in [
            BACKDROP,
            "border_3_pieces",
            "listbox_cursor",
            "bf2_buttons_midleft",
            "btna",
            "btnb",
        ] {
            assert!(sheets.contains(&p), "{p}: {sheets:?}");
        }
        let grey = DIMMED_GREY / 255.0;
        let dimmed = f
            .verts
            .iter()
            .filter(|v| v.color[..3].iter().all(|c| (c - grey).abs() < 1e-4))
            .count();
        // "ZZZ" (unbaked), and all four commands while a list has the
        // focus, ADD excepted (it keeps its focus colour).
        let glyphs = "ZZZ".len() + "LaunchRemoveOptions".len();
        assert_eq!(dimmed, 4 * glyphs);
        // menuref §9.1.9: the list cursor box is solid while ADD pulses.
        let alphas = |tex: &str| -> Vec<f32> {
            f.batches
                .iter()
                .filter(|b| b.texture == tex)
                .flat_map(|b| &f.indices[b.first as usize..(b.first + b.count) as usize])
                .map(|i| f.verts[*i as usize].color[3])
                .collect()
        };
        let cursor = alphas("listbox_cursor");
        assert!(
            !cursor.is_empty() && cursor.iter().all(|a| *a == 1.0),
            "{cursor:?}"
        );
        let pulse = crate::shell_feel::Pulse::BUTTON.alpha();
        let [r, g, b] = crate::spawn_draw::SELECTED_TEXT.0.map(|c| c / 255.0);
        let add: Vec<f32> = f
            .verts
            .iter()
            .filter(|v| (v.color[0] - r).abs() < 1e-4 && (v.color[2] - b).abs() < 1e-4)
            .filter(|v| (v.color[1] - g).abs() < 1e-4)
            .map(|v| v.color[3])
            .collect();
        assert!(add.contains(&pulse), "the ADD label at the pulse's alpha");
    }
}
