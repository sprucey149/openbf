//! The spawn screens as the PS2's BF2 draws them (card SSELB slice 2,
//! `analysis/re-spawn-flow.md` §2-§4, `analysis/re-bf2-shell.md` §3): laid
//! out in the shell's 512 × 448 screen space and painted by the HUD pass
//! over the backdrop, with the BF2 bitmap fonts and the shell's own pieces.
//!
//! - **Side select**: the vertical button stack - the title bar
//!   (`bf2_buttons_topleft / title_center / topright`), a row per team and
//!   Auto Assign (`midleft / items_center / midright`, the last one
//!   `botleft / items_center / botright`) - and the help line "Select" with
//!   the Cross icon at the bottom right (`bAcceptIsSelect`, no Back line).
//! - **Class select**: the panel of `FUN_00396430` - a title tab with the
//!   class name over a `border_3_pieces` box of lines (Unit Count, a blank
//!   line, Equipment: and one line per weapon) - and the help lines "Back"
//!   (Triangle) left, "Select Character" (Cross) right.
//! - **Spawn map** (slice 5): the Map element's Spawn view
//!   (`crate::hud_map`, the whole map with the picked post's ring), the HUD
//!   tree's `player1mapkey_spawnonly` legend (the post icon "Command Post",
//!   the D-pad "Change Command Post", "{btna} Select Command Post") and the
//!   mission's objective at `player1objectives`, top right; no Back line.
//!   The legend and the objective are laid out here from the layout's own
//!   numbers, in the HUD tree's mapping: the port's tree has no static
//!   texts (`Text` keys) nor an `objectivelist` element.
//!
//! Every screen first dims the backdrop to [`crate::spawn::BACKDROP_DIM`] of
//! its displayed brightness: the HUD pass blends display values, so the
//! veil's alpha is exactly 1 - 0.51 (the egui veil had to take it through
//! the sRGB curve).
//!
//! Geometry [M] from `lead_tmp/ssel/t1_hot1` (BF2 Hoth, PCSX2 640 × 480 =
//! the 512 × 448 screen at 1.25 × 1.0714 px per unit); the palette and the
//! label stretch are the shell's (`re-bf2-shell.md` §3.1, §3.2). `[ours]`
//! Hor+: the stack and the panel keep their place relative to the centre,
//! a left help line to the left edge, a right one to the right edge.

use crate::hud_draw::{color, Ctx, Frame, Panel, Quad, Screen};
use crate::hud_tree::{HAlign, FRAME_H, FRAME_W};
use crate::input_icons::Prompt;
use glam::{Mat4, Vec3};

pub(crate) const TITLE_PIECES: [&str; 3] = [
    "bf2_buttons_topleft",
    "bf2_buttons_title_center",
    "bf2_buttons_topright",
];

pub(crate) fn item_pieces(last: bool) -> [&'static str; 3] {
    if last {
        [
            "bf2_buttons_botleft",
            "bf2_buttons_items_center",
            "bf2_buttons_botright",
        ]
    } else {
        [
            "bf2_buttons_midleft",
            "bf2_buttons_items_center",
            "bf2_buttons_midright",
        ]
    }
}

use crate::typography::{tint, BODY, HELP, ITEM, LIST, TEXT_INSET, TITLE};
pub use crate::typography::{ROW_H, ROW_PITCH};

/// The side select's stack [M] (`s0_team1.png`): the bar's outer edges
/// across, the title bar's top, a row's height and pitch (the font's 22 +
/// gButtonHeightPad 6, + gButtonGutter 4: `re-bf2-shell.md` §3.2).
pub const SIDE_STACK_X: [f32; 2] = [257.6, 488.8];
pub const SIDE_STACK_TOP: f32 = 188.5;
/// How opaque the button pieces are drawn: the bar over the dimmed
/// backdrop reads (66, 72, 73) where the sheet is (113, 127, 138) at 0.82
/// over about 26 ([M] `s0_team1.png`, a fit: PROVISIONAL).
pub const PIECE_ALPHA: f32 = 0.6;

/// The class panel [M] (`s4_class.png`): the title tab's edges and the
/// box's, the text's left edge and its first line's top (lines are
/// [`BODY`]'s pitch).
pub const CLASS_TAB: [f32; 4] = [234.4, 98.0, 488.8, 122.0];
pub const CLASS_BOX: [f32; 4] = [232.8, 127.0, 491.2, 351.0];
pub const CLASS_TEXT_X: f32 = 242.0;
pub const CLASS_TEXT_TOP: f32 = 133.0;
/// `border_3_pieces`' corners: texels of the 64 × 64 sheet and units on
/// screen (`[ours]` read off the sheet: the rounded border fits in 16).
const BORDER_TEXELS: f32 = 16.0;
const BORDER_UNITS: f32 = 12.0;

/// The help lines' centre line: 15 units above the safe area's bottom
/// (`re-bf2-shell.md` §3.4; safe area 25.6 .. 486.4 × 22.4 .. 425.6,
/// §3.5), and the pad icon's box (a 32 × 32 sheet; square on the 4:3
/// display: 9.6 × 11.2 units, its ring measured 11 px across) [M].
pub const HELP_Y: f32 = 425.6 - 15.0;
pub const SAFE_X: [f32; 2] = [25.6, 486.4];
pub const ICON: [f32; 2] = [9.6, 11.2];
/// From an icon's edge to its label's run [M] (the ink of "Back" and
/// "Select Character" sits 12-14 units from the icon).
pub const ICON_GAP: f32 = 12.4;
/// The help label hangs this much below the icon's centre line [M]: its
/// ink is 2-3 px lower than the line's middle on both frames.
pub const HELP_DROP: f32 = 2.5;

/// `player1mapkey` + its `player1mapkey_spawnonly` child's Position, in
/// viewport fractions of the HUD tree (the layout [V]).
pub const LEGEND_AT: [f32; 2] = [0.142071, 0.682009 + 0.069934];
/// The legend's parts relative to it (the layout [V]): the post icon
/// (`hud_flag_icon`, rect 0.031117 × 0.046218, friend blue), the D-pad
/// (`btn_directional_pad`, 0.03984 × 0.053168 at Scale 0.81) and the three
/// texts (alpha 0.59 / 0.61 / 0.59).
pub const LEGEND_ICON: [f32; 4] = [0.00973, 0.000002, 0.031117, 0.046218];
pub const LEGEND_PAD: [f32; 4] = [0.010315, 0.051341, 0.03984 * 0.81, 0.053168 * 0.81];
/// The texts' ink stands this much lower than the layout's place puts it
/// [M] (2 display px); the Cross a `{btna}` draws is this wide, in its
/// text's alpha (the stroke's blue peaks at 153, 165 here), centred on the
/// line, and its text starts a line height plus the gap
/// after the tag's place [M] (22 and 32 display px).
pub const LEGEND_DROP: f32 = 1.8;
pub const INLINE_ICON: f32 = 17.5;
pub const INLINE_ICON_GAP: f32 = 5.5;
pub const LEGEND_TEXT: [[f32; 3]; 3] = [
    [0.052833, 0.006737, 0.59],
    [0.052729, 0.054129, 0.61],
    [0.014572, 0.103588, 0.59],
];
/// The post icon's colour (the layout's `color`: the Map's friend blue).
pub const LEGEND_ICON_RGB: [f32; 3] = [1.0, 86.0, 213.0];
/// `player1objectives`' Position and its Rect's width (the layout [V]);
/// the ink starts (11.4, 15) display px inside it [M], in the frame's
/// yellow [M] (peak (222, 222, 74) over the dimmed sky), its lines the
/// font's line height apart (17 px in the frame).
pub const OBJECTIVE_AT: [f32; 2] = [0.533743, 0.055098];
pub const OBJECTIVE_WIDTH: f32 = 0.399926;
pub const OBJECTIVE_INSET: [f32; 2] = [9.0, 9.0];

/// What one spawn screen shows; every text already in the player's
/// language (the game reads the disc's table).
#[derive(Debug, Clone, PartialEq)]
pub enum SpawnPage {
    /// "Select a team": the title, the rows (team 1, team 2, Auto Assign)
    /// and the highlighted one; the help line's "Select".
    Side {
        title: String,
        rows: Vec<String>,
        selected: usize,
        select: String,
    },
    /// The spawn map: the mission's objective line and the legend's three
    /// texts (the map itself is [`crate::hud::HudModel::map`]'s Spawn
    /// view).
    Map {
        objective: String,
        legend: [String; 3],
    },
    /// The class panel: the class's name, its lines (blank ones keep
    /// their pitch) and the help lines "Back" and "Select Character";
    /// the team, picked visible row and raw class IDs stand the 3-D
    /// line-up (`game::spawn_preview`); `refused` has bit `j` set for each
    /// visible row `j` the game refuses (drawn black there).
    Class {
        title: String,
        lines: Vec<String>,
        back: String,
        accept: String,
        team: u8,
        class: usize,
        classes: Vec<u8>,
        refused: u32,
    },
}

impl SpawnPage {
    /// Whether the HUD tree still draws under this page: none does (the
    /// PS2's frames show no HUD under any of the three; the map page draws
    /// the Map element's Spawn view and the legend itself).
    pub fn keeps_tree(&self) -> bool {
        false
    }
}

/// The shell's screen units → window pixels: the 4:3 box of the window's
/// height, moved right by `shift` window pixels.
fn shell_screen(size: [f32; 2], shift: f32) -> Screen {
    let box_w = 4.0 / 3.0 * size[1];
    Screen {
        origin: [(size[0] - box_w) * 0.5 + shift, 0.0],
        scale: [box_w / FRAME_W, size[1] / FRAME_H],
    }
}

/// Anchors (`[ours]` Hor+).
pub(crate) enum Anchor {
    Centre,
    Left,
    Right,
}

pub(crate) fn screen_at(size: [f32; 2], a: Anchor) -> Screen {
    let extra = size[0] - 4.0 / 3.0 * size[1];
    shell_screen(
        size,
        match a {
            Anchor::Centre => 0.0,
            Anchor::Left => -extra * 0.5,
            Anchor::Right => extra * 0.5,
        },
    )
}

const NONE: usize = usize::MAX;

impl Ctx<'_> {
    pub(crate) fn sheet(
        &mut self,
        s: &Screen,
        rect: [f32; 4],
        uv: [f32; 4],
        texture: &str,
        alpha: f32,
    ) {
        self.sheet_tinted(s, rect, uv, texture, ([255.0; 3], alpha));
    }

    /// [`Ctx::sheet`] tinted `rgb` (0..255).
    pub(crate) fn sheet_tinted(
        &mut self,
        s: &Screen,
        rect: [f32; 4],
        uv: [f32; 4],
        texture: &str,
        (rgb, alpha): ([f32; 3], f32),
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

    /// A bar of three pieces: two square ends of the row's height, the
    /// centre stretched between them.
    pub(crate) fn bar(&mut self, s: &Screen, rect: [f32; 4], pieces: [&str; 3]) {
        self.bar_tinted(s, rect, pieces, ([255.0; 3], PIECE_ALPHA));
    }

    /// [`Ctx::bar`] tinted `rgb` at `alpha`.
    pub(crate) fn bar_tinted(
        &mut self,
        s: &Screen,
        [x0, y0, x1, y1]: [f32; 4],
        pieces: [&str; 3],
        tint: ([f32; 3], f32),
    ) {
        let h = y1 - y0;
        let full = [0.0, 0.0, 1.0, 1.0];
        self.sheet_tinted(s, [x0, y0, x0 + h, y1], full, pieces[0], tint);
        self.sheet_tinted(s, [x0 + h, y0, x1 - h, y1], full, pieces[1], tint);
        self.sheet_tinted(s, [x1 - h, y0, x1, y1], full, pieces[2], tint);
    }

    /// `border_3_pieces` as nine parts: fixed corners, stretched edges.
    pub(crate) fn bordered(&mut self, s: &Screen, [x0, y0, x1, y1]: [f32; 4]) {
        let (c, t) = (BORDER_UNITS, BORDER_TEXELS / 64.0);
        let xs = [x0, x0 + c, x1 - c, x1];
        let ys = [y0, y0 + c, y1 - c, y1];
        let us = [0.0, t, 1.0 - t, 1.0];
        for j in 0..3 {
            for i in 0..3 {
                self.sheet(
                    s,
                    [xs[i], ys[j], xs[i + 1], ys[j + 1]],
                    [us[i], us[j], us[i + 1], us[j + 1]],
                    "border_3_pieces",
                    1.0,
                );
            }
        }
    }

    /// A help line: the pad icon at the safe area's edge, its label beside
    /// it on the inside.
    pub(crate) fn help(&mut self, size: [f32; 2], right: bool, prompt: Prompt, text: &str) {
        self.help_bottom(size, right, prompt, text, size[1]);
    }

    /// The same footer at the bottom of a window taller than its UI frame.
    pub(crate) fn help_bottom(
        &mut self,
        size: [f32; 2],
        right: bool,
        prompt: Prompt,
        text: &str,
        bottom: f32,
    ) {
        let icon = self.art.icons.sheet(prompt);
        let a = if right { Anchor::Right } else { Anchor::Left };
        let mut s = screen_at(size, a);
        s.origin[1] += bottom - size[1];
        self.help_icon_on(&s, right, icon, text);
    }

    /// A help line on the shell screen `s` itself (a Players screen region's box, issue #194):
    /// `icon` at its safe area's edge, `text` beside it on the inside.
    pub(crate) fn help_icon_on(&mut self, s: &Screen, right: bool, icon: &str, text: &str) {
        let s = *s;
        let ix = if right {
            SAFE_X[1] - ICON[0]
        } else {
            SAFE_X[0]
        };
        let top = HELP_Y - ICON[1] * 0.5;
        self.sheet(
            &s,
            [ix, top, ix + ICON[0], top + ICON[1]],
            [0.0, 0.0, 1.0, 1.0],
            icon,
            1.0,
        );
        let (x, align) = if right {
            (ix - ICON_GAP, HAlign::Right)
        } else {
            (ix + ICON[0] + ICON_GAP, HAlign::Left)
        };
        let at = (x, HELP_Y - HELP.size * 0.5 + HELP_DROP);
        self.text(&s, &HELP, text, at, align, tint::HELP);
    }

    /// The veil that leaves [`crate::spawn::BACKDROP_DIM`] of the
    /// backdrop's displayed brightness, over the whole window.
    pub(crate) fn spawn_veil(&mut self, size: [f32; 2]) {
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
                color: [0.0, 0.0, 0.0, 1.0 - crate::spawn::BACKDROP_DIM],
                texture: "white",
                additive: false,
                shadow: false,
                boxed: false,
            },
        );
    }

    /// A help line in the panel `p`, at its real bottom.
    fn panel_help(&mut self, p: &Panel, right: bool, prompt: Prompt, text: &str) {
        self.help_bottom(p.size, right, prompt, text, p.bottom);
    }

    /// The page in the centred panel `p` ([`crate::hud_draw::Frame::panel`]).
    pub(crate) fn spawn_page(&mut self, page: &SpawnPage, p: &Panel) {
        match page {
            SpawnPage::Side {
                title,
                rows,
                selected,
                select,
            } => {
                let s = p.screen(Anchor::Centre);
                let [x0, x1] = SIDE_STACK_X;
                let lh = TITLE.size;
                let cx = (x0 + x1) * 0.5;
                let row = |k: usize| {
                    let y = SIDE_STACK_TOP + ROW_PITCH * k as f32;
                    [x0, y, x1, y + ROW_H]
                };
                let r = row(0);
                self.bar(&s, r, TITLE_PIECES);
                let mid = r[1] + (ROW_H - lh) * 0.5;
                self.text(&s, &TITLE, title, (cx, mid), HAlign::Center, tint::TITLE);
                for (k, text) in rows.iter().enumerate() {
                    let r = row(k + 1);
                    self.bar(&s, r, item_pieces(k + 1 == rows.len()));
                    let style = if k == *selected {
                        tint::SELECTED
                    } else {
                        tint::UNSELECTED
                    };
                    let mid = r[1] + (ROW_H - ITEM.size) * 0.5;
                    self.text(&s, &ITEM, text, (cx, mid), HAlign::Center, style);
                }
                self.panel_help(p, true, Prompt::SpawnAccept, select);
            }
            SpawnPage::Map { objective, legend } => {
                self.map_spawn(p);
                self.map_legend(p, legend);
                self.objective(p, objective);
            }
            SpawnPage::Class {
                title,
                lines,
                back,
                accept,
                ..
            } => {
                let s = p.screen(Anchor::Centre);
                self.bar(&s, CLASS_TAB, TITLE_PIECES);
                self.text_in(&s, &TITLE, title, CLASS_TAB, tint::TITLE);
                self.bordered(&s, CLASS_BOX);
                let room = CLASS_BOX[2] - TEXT_INSET - CLASS_TEXT_X;
                for (k, line) in lines.iter().enumerate() {
                    let at = (CLASS_TEXT_X, CLASS_TEXT_TOP + BODY.line * k as f32);
                    let line = self.ellipsized(&BODY, line, room);
                    self.text(&s, &BODY, &line, at, HAlign::Left, tint::BODY);
                }
                self.panel_help(p, false, Prompt::SpawnBack, back);
                self.panel_help(p, true, Prompt::SpawnAccept, accept);
            }
        }
    }
}

/// The objective, broken at words to its list's width.
fn objective_lines(font: &manifest::hud::HudFont, text: &str) -> Vec<String> {
    let width = OBJECTIVE_WIDTH * FRAME_W - OBJECTIVE_INSET[0];
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    crate::typography::wrap(font, &LIST, &flat, width)
}

impl Ctx<'_> {
    /// The spawn map's legend: the post icon, the D-pad and the three
    /// lines; a text's leading `{btna}` is the accept icon, a line high.
    fn map_legend(&mut self, p: &Panel, texts: &[String; 3]) {
        let s = p.tree(LEGEND_AT[0]);
        let at = |x: f32, y: f32| ((LEGEND_AT[0] + x) * FRAME_W, (LEGEND_AT[1] + y) * FRAME_H);
        let full = [0.0, 0.0, 1.0, 1.0];
        let rect = |[x, y, w, h]: [f32; 4]| {
            let (x0, y0) = at(x, y);
            [x0, y0, x0 + w * FRAME_W, y0 + h * FRAME_H]
        };
        self.quad(
            NONE,
            &Mat4::IDENTITY,
            &s,
            &Quad {
                rect: rect(LEGEND_ICON),
                uv: full,
                color: color(LEGEND_ICON_RGB, 1.0),
                texture: "hud_flag_icon",
                additive: false,
                shadow: false,
                boxed: false,
            },
        );
        let pad = self.art.icons.sheet(Prompt::DPad);
        self.sheet(&s, rect(LEGEND_PAD), full, pad, 1.0);
        let lh = LIST.size;
        for (text, [x, y, alpha]) in texts.iter().zip(LEGEND_TEXT) {
            let (mut x0, mut y0) = at(x, y);
            y0 += LEGEND_DROP;
            let mut text = text.trim();
            if let Some(rest) = text.strip_prefix("{btna}") {
                let (cx, cy, h) = (x0 + lh * 0.5, y0 + lh * 0.5, INLINE_ICON * 0.5);
                let icon = self.art.icons.sheet(Prompt::SpawnAccept);
                self.sheet(&s, [cx - h, cy - h, cx + h, cy + h], full, icon, alpha);
                x0 += lh + INLINE_ICON_GAP;
                text = rest;
            }
            self.text(&s, &LIST, text, (x0, y0), HAlign::Left, ([255.0; 3], alpha));
        }
    }

    /// The mission's objective, word-wrapped to its list's width.
    fn objective(&mut self, p: &Panel, text: &str) {
        let Some(font) = self.face(LIST.face) else {
            return;
        };
        let s = p.tree(OBJECTIVE_AT[0]);
        let lines = objective_lines(font, text);
        let pitch = LIST.line;
        let x = OBJECTIVE_AT[0] * FRAME_W + OBJECTIVE_INSET[0];
        let y = OBJECTIVE_AT[1] * FRAME_H + OBJECTIVE_INSET[1];
        for (k, line) in lines.iter().enumerate() {
            let at = (x, y + pitch * k as f32);
            self.text(&s, &LIST, line, at, HAlign::Left, tint::OBJECTIVE);
        }
    }
}

/// Where a shell-space point lands in a `size` window under the centre
/// anchor (tests and the golden's checks).
pub fn window_point(p: [f32; 2], size: [f32; 2]) -> [f32; 2] {
    window_point_in(p, &Frame::window(size))
}

/// [`window_point`] in the screen `f` (its centred panel).
pub fn window_point_in(p: [f32; 2], f: &Frame) -> [f32; 2] {
    f.panel()
        .screen(Anchor::Centre)
        .map(Vec3::new(p[0], p[1], 0.0))
}

/// Hit-test the "Select a team" screen rows in a `size` window.
/// Returns `Some(row_idx)` where 0 = Team 1, 1 = Team 2, 2 = Auto Assign.
pub fn side_row_at(point: [f32; 2], size: [f32; 2], row_count: usize) -> Option<usize> {
    side_row_at_in(point, &Frame::window(size), row_count)
}

/// [`side_row_at`] in the screen `f`, `point` in its pixels.
pub fn side_row_at_in(point: [f32; 2], f: &Frame, row_count: usize) -> Option<usize> {
    let s = f.panel().screen(Anchor::Centre);
    let sx = (point[0] - s.origin[0]) / s.scale[0];
    let sy = (point[1] - s.origin[1]) / s.scale[1];
    let [x0, x1] = SIDE_STACK_X;
    if sx < x0 || sx > x1 {
        return None;
    }
    for k in 0..row_count {
        let y0 = SIDE_STACK_TOP + ROW_PITCH * (k + 1) as f32;
        let y1 = y0 + ROW_H;
        if sy >= y0 && sy <= y1 {
            return Some(k);
        }
    }
    None
}

/// Issue #128: the help line a pointer can press - the right one accepts
/// ("Select", "Select Character"), the left one backs ("Back").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Help {
    Accept,
    Back,
}

/// `[ours]` The help line's pointer box past its icon and label, in the
/// shell's units: a little to each side, and down and up from the line's
/// middle. measure: live clicks on the help lines.
pub const HELP_PAD: [f32; 2] = [4.0, 12.0];

/// The window point of the middle of a help line's icon (left or `right`)
/// in a `size` window ([`Ctx::help`]'s layout).
pub fn help_icon_point(size: [f32; 2], right: bool) -> [f32; 2] {
    help_icon_point_in(&Frame::window(size), right)
}

/// [`help_icon_point`] in the screen `f` (the footer at its real bottom).
pub fn help_icon_point_in(f: &Frame, right: bool) -> [f32; 2] {
    let (a, ix) = if right {
        (Anchor::Right, SAFE_X[1] - ICON[0])
    } else {
        (Anchor::Left, SAFE_X[0])
    };
    help_screen(&f.panel(), a).map(Vec3::new(ix + ICON[0] * 0.5, HELP_Y, 0.0))
}

/// The screen a help line under anchor `a` is laid out in ([`Ctx::help_bottom`]).
fn help_screen(p: &Panel, a: Anchor) -> Screen {
    let mut s = screen_at(p.size, a);
    s.origin[1] += p.bottom - p.size[1];
    s
}

/// Which of `page`'s help lines holds the window point `point` of a `size`
/// window: its icon and label as [`Ctx::help`] lays them out, `font` the
/// label's (`None`: a label of eight units a letter), padded by
/// [`HELP_PAD`]. The spawn map has no help line of its own.
pub fn help_at(
    font: Option<&manifest::hud::HudFont>,
    page: &SpawnPage,
    point: [f32; 2],
    size: [f32; 2],
) -> Option<Help> {
    help_at_in(font, page, point, &Frame::window(size))
}

/// [`help_at`] in the screen `f`, `point` in its pixels.
pub fn help_at_in(
    font: Option<&manifest::hud::HudFont>,
    page: &SpawnPage,
    point: [f32; 2],
    f: &Frame,
) -> Option<Help> {
    let lines: Vec<(bool, &str, Help)> = match page {
        SpawnPage::Side { select, .. } => vec![(true, select.as_str(), Help::Accept)],
        SpawnPage::Class { back, accept, .. } => vec![
            (false, back.as_str(), Help::Back),
            (true, accept.as_str(), Help::Accept),
        ],
        SpawnPage::Map { .. } => Vec::new(),
    };
    lines
        .into_iter()
        .find_map(|(right, text, help)| help_line_at(font, right, text, point, f).then_some(help))
}

/// Whether the point `point` of the screen `f` is on one help line (its icon and label, the
/// right-hand one when `right`), padded by [`HELP_PAD`].
pub(crate) fn help_line_at(
    font: Option<&manifest::hud::HudFont>,
    right: bool,
    text: &str,
    point: [f32; 2],
    f: &Frame,
) -> bool {
    let width = font.map_or(8.0 * text.chars().count() as f32, |f| {
        crate::typography::width(f, &HELP, text)
    });
    let (a, x0, x1) = if right {
        let ix = SAFE_X[1] - ICON[0];
        (Anchor::Right, ix - ICON_GAP - width, ix + ICON[0])
    } else {
        let ix = SAFE_X[0];
        (Anchor::Left, ix, ix + ICON[0] + ICON_GAP + width)
    };
    let s = help_screen(&f.panel(), a);
    let sx = (point[0] - s.origin[0]) / s.scale[0];
    let sy = (point[1] - s.origin[1]) / s.scale[1];
    sx >= x0 - HELP_PAD[0] && sx <= x1 + HELP_PAD[0] && (sy - HELP_Y).abs() <= HELP_PAD[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    const BUTTON_FONT: &str = "gamefont_large";
    const SMALL_FONT: &str = "gamefont_small";
    const MAP_FONT: &str = "gamefont_tiny";
    use crate::hud_draw::{HudArt, HudFrame};
    use crate::hud_tree::Hud;
    use crate::typography::tint::{
        SELECTED as SELECTED_TEXT, TITLE as TITLE_TEXT, UNSELECTED as UNSELECTED_TEXT,
    };
    use manifest::hud::{HudFont, HudGlyph, HudLayout};

    /// A font whose every printable glyph is 10 wide, 12 tall.
    fn font(name: &str, line_height: u8) -> HudFont {
        HudFont {
            name: name.into(),
            line_height,
            pages: vec![format!("{name}_tex0")],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: if code == 32 { 0 } else { 10 },
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        }
    }

    fn draw(page: &SpawnPage, size: [f32; 2]) -> HudFrame {
        let mut art = HudArt::default();
        art.fonts.insert(BUTTON_FONT.into(), font(BUTTON_FONT, 22));
        art.fonts.insert(SMALL_FONT.into(), font(SMALL_FONT, 18));
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.spawn = Some(page.clone());
        crate::hud_draw::draw(&hud, &art, size)
    }

    fn side() -> SpawnPage {
        SpawnPage::Side {
            title: "Select a team".into(),
            rows: vec!["Empire".into(), "Rebels".into(), "Auto Assign".into()],
            selected: 1,
            select: "Select".into(),
        }
    }

    /// The texture of every batch, in draw order.
    fn sheets(f: &HudFrame) -> Vec<&str> {
        f.batches.iter().map(|b| b.texture.as_str()).collect()
    }

    /// The class and map pages draw the lookup's icons: a remapped accept
    /// shows the bound button, the map's D-pad and inline icon follow.
    #[test]
    fn the_prompts_follow_the_icon_set() {
        use crate::input_icons::{IconSet, Icons, PadButton};
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut art = HudArt::default();
        art.fonts.insert(SMALL_FONT.into(), font(SMALL_FONT, 18));
        art.fonts.insert(MAP_FONT.into(), font(MAP_FONT, 12));
        art.icons = Icons {
            spawn_accept: PadButton::West,
            ..Icons::of(IconSet::XboxOne)
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.spawn = Some(SpawnPage::Class {
            title: "Trooper".into(),
            lines: Vec::new(),
            back: "Back".into(),
            accept: "Select Character".into(),
            team: 0,
            class: 0,
            classes: vec![0],
            refused: 0,
        });
        let f = crate::hud_draw::draw(&hud, &art, [640.0, 480.0]);
        let s = sheets(&f);
        assert!(
            s.contains(&"icon_xbone_x") && s.contains(&"icon_xbone_y"),
            "{s:?}"
        );
        assert!(!s.contains(&"btna") && !s.contains(&"btnb"), "{s:?}");

        art.icons = Icons::of(IconSet::KeyboardMouse);
        hud.spawn = Some(SpawnPage::Map {
            objective: "Capture".into(),
            legend: [
                "Command Post".into(),
                "Change Command Post".into(),
                "{btna} Select Command Post".into(),
            ],
        });
        let f = crate::hud_draw::draw(&hud, &art, [640.0, 480.0]);
        let s = sheets(&f);
        assert!(
            s.contains(&"icon_kb_arrows") && s.contains(&"icon_kb_enter"),
            "{s:?}"
        );
        assert!(!s.contains(&"btn_directional_pad"), "{s:?}");
    }

    /// The side select: the veil, the title bar's three pieces, a row's
    /// three for each of the three rows (the last with the rounded
    /// bottom), the Cross icon; the highlighted row's label in the
    /// selected colour, the others unselected.
    #[test]
    fn the_side_select_is_the_ps2_stack() {
        let f = draw(&side(), [640.0, 480.0]);
        let s = sheets(&f);
        assert_eq!(s[0], "white", "the veil first");
        for p in [
            "bf2_buttons_topleft",
            "bf2_buttons_title_center",
            "bf2_buttons_midleft",
            "bf2_buttons_botright",
            "btna",
        ] {
            assert!(s.contains(&p), "{p} not drawn: {s:?}");
        }
        assert!(!s.contains(&"btnb"), "the side select has no Back line");
        let mids = s.iter().filter(|t| **t == "bf2_buttons_midleft").count();
        let bots = s.iter().filter(|t| **t == "bf2_buttons_botleft").count();
        assert_eq!((mids, bots), (2, 1));
        // The veil leaves 0.51 of the displayed value: the HUD pass blends
        // display values.
        let veil = f.verts[0].color;
        assert_eq!(veil[..3], [0.0; 3]);
        assert!((1.0 - veil[3] - crate::spawn::BACKDROP_DIM).abs() < 1e-6);
        // Label colours: one row selected (blue), two not (grey), the title
        // yellow.
        let has = |rgb: [f32; 3], a: f32| {
            let c = color(rgb, a);
            f.verts.iter().filter(|v| v.color == c).count()
        };
        assert!(has(SELECTED_TEXT.0, SELECTED_TEXT.1) > 0);
        assert!(has(TITLE_TEXT.0, TITLE_TEXT.1) > 0);
        assert!(has(UNSELECTED_TEXT.0, UNSELECTED_TEXT.1) > has(SELECTED_TEXT.0, SELECTED_TEXT.1));
    }

    /// On a 4:3 window of 640 × 480 the stack lands where the PS2 draws it
    /// (`s0_team1.png`: the bar 322..611 px across, the title's top at 202
    /// px); on a 21:9 window it keeps its place from the centre.
    #[test]
    fn the_stack_sits_at_the_ps2_place() {
        let [x0, y0] = window_point([SIDE_STACK_X[0], SIDE_STACK_TOP], [640.0, 480.0]);
        let [x1, _] = window_point([SIDE_STACK_X[1], 0.0], [640.0, 480.0]);
        assert!(
            (x0 - 322.0).abs() < 1.0 && (x1 - 611.0).abs() < 1.0,
            "{x0} {x1}"
        );
        assert!((y0 - 202.0).abs() < 1.0, "{y0}");
        let wide = window_point([256.0, 224.0], [2520.0, 1080.0]);
        assert!((wide[0] - 1260.0).abs() < 0.01 && (wide[1] - 540.0).abs() < 0.01);
    }

    /// The class panel: the tab, nine border parts, a line per text, both
    /// help icons, Triangle at the left edge and Cross at the right of a
    /// wide window.
    #[test]
    fn the_class_panel_has_its_parts() {
        let page = SpawnPage::Class {
            title: "Snowtrooper".into(),
            lines: vec![
                "Unit Count: 7".into(),
                String::new(),
                "Equipment:".into(),
                "Blaster Rifle".into(),
            ],
            back: "Back".into(),
            accept: "Select Character".into(),
            team: 0,
            class: 0,
            classes: vec![0],
            refused: 0,
        };
        let size = [2520.0, 1080.0];
        let f = draw(&page, size);
        let s = sheets(&f);
        assert!(s.contains(&"bf2_buttons_title_center") && s.contains(&"border_3_pieces"));
        // Every quad is 4 vertices: find the icons' boxes.
        let quad_of = |tex: &str| {
            let b = f.batches.iter().find(|b| b.texture == tex).unwrap();
            let vs: Vec<_> = f.indices[b.first as usize..(b.first + b.count) as usize]
                .iter()
                .map(|i| f.verts[*i as usize].pos)
                .collect();
            let xs = vs.iter().map(|p| p[0]);
            (
                xs.clone().fold(f32::MAX, f32::min),
                xs.fold(f32::MIN, f32::max),
            )
        };
        let (bx0, _) = quad_of("btnb");
        let (_, ax1) = quad_of("btna");
        let unit = 1080.0 / FRAME_H;
        assert!(
            (bx0 - SAFE_X[0] * 4.0 / 3.0 * unit * FRAME_H / FRAME_W).abs() < 0.5,
            "{bx0}"
        );
        assert!(
            (size[0] - ax1 - (FRAME_W - SAFE_X[1]) * 4.0 / 3.0 * 1080.0 / FRAME_W).abs() < 0.5,
            "{ax1}"
        );
    }

    fn map_page() -> SpawnPage {
        SpawnPage::Map {
            objective: "Capture all the Command Posts".into(),
            legend: [
                "Command Post".into(),
                "Change Command Post".into(),
                "{btna} Select Command Post".into(),
            ],
        }
    }

    /// No page keeps the tree under it: the map page draws its own map.
    #[test]
    fn no_page_keeps_the_tree() {
        assert!(!side().keeps_tree());
        assert!(!map_page().keeps_tree());
    }

    /// The spawn map page: the Map element's Spawn view (posts, the picked
    /// one's ring on it, the north pointer), the legend's icons and the
    /// objective broken where the PS2's frame breaks it.
    #[test]
    fn the_map_page_has_its_parts() {
        use crate::hud_map::{MapIcon, MapModel};
        use manifest::hud::{HudArg, HudNode};
        let n = HudArg::Num;
        let p = |name: &str, args: Vec<HudArg>| HudNode {
            name: name.into(),
            args,
            children: Vec::new(),
        };
        let layout = HudLayout {
            name: "t".into(),
            nodes: vec![HudNode {
                name: "Map".into(),
                args: vec![HudArg::Str("player1map".into())],
                children: vec![
                    p("Scale", vec![n(0.9), n(0.9), n(0.9)]),
                    p("alpha", vec![n(0.85)]),
                ],
            }],
        };
        let empty = HudLayout {
            name: "e".into(),
            nodes: Vec::new(),
        };
        let mut art = HudArt::default();
        art.fonts.insert(MAP_FONT.into(), font(MAP_FONT, 15));
        let mut hud = Hud::new(&layout, &empty);
        let post = |x: f32| MapIcon {
            pos: [x, 0.0],
            texture: "hud_flag_icon".into(),
            scale: 2.5,
            color: crate::hud_map::FRIEND,
            ..MapIcon::default()
        };
        hud.map = Some(
            MapModel {
                extent: 400.0,
                posts: vec![post(-100.0), post(150.0)],
                ..MapModel::default()
            }
            .spawn_view(Some(1)),
        );
        hud.spawn = Some(map_page());
        let size = [1280.0, 720.0];
        let f = crate::hud_draw::draw(&hud, &art, size);
        let s = sheets(&f);
        for tex in [
            "hud_flag_icon",
            "hud_icon_flash",
            "hud_map_northpointer",
            "btn_directional_pad",
            "btna",
        ] {
            assert!(s.contains(&tex), "{tex}: {s:?}");
        }
        // The ring stands on the picked post (its Shadow copy, one unit
        // down-right, widens the box).
        let b = f
            .batches
            .iter()
            .find(|b| b.texture == "hud_icon_flash")
            .unwrap();
        let vs: Vec<[f32; 3]> = f.indices[b.first as usize..(b.first + b.count) as usize]
            .iter()
            .map(|i| f.verts[*i as usize].pos)
            .collect();
        let mid = [0, 1].map(|a| {
            let (lo, hi) = vs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                (lo.min(v[a]), hi.max(v[a]))
            });
            (lo + hi) * 0.5
        });
        let want = crate::hud_map::window_point(&hud, [150.0, 0.0], true, size).unwrap();
        assert!(
            (mid[0] - want[0]).abs() < 1.5 && (mid[1] - want[1]).abs() < 1.5,
            "{mid:?} {want:?}"
        );
        // The PS2 frame's two lines; a 10-texel font breaks it there too.
        let lines = objective_lines(&art.fonts[MAP_FONT], "Capture all the Command Posts");
        // Broken as the capitals are wide; drawn in capitals by the role.
        assert_eq!(lines, ["Capture all the", "Command Posts"]);
    }

    /// PR #102's spawn-map click lands where the post is drawn whatever
    /// the layout's Map `Scale`: at 0.6, not the 0.9 it once assumed, the
    /// hit point of the picked post is the centre of its drawn ring.
    #[test]
    fn the_spawn_map_click_follows_the_map_elements_scale() {
        use crate::hud_map::{hit_spawn_post, map_scale, spawn_window_point, MapIcon, MapModel};
        use manifest::hud::{HudArg, HudNode};
        let n = HudArg::Num;
        let layout = HudLayout {
            name: "t".into(),
            nodes: vec![HudNode {
                name: "Map".into(),
                args: vec![HudArg::Str("player1map".into())],
                children: vec![HudNode {
                    name: "Scale".into(),
                    args: vec![n(0.6), n(0.6), n(0.6)],
                    children: Vec::new(),
                }],
            }],
        };
        let empty = HudLayout {
            name: "e".into(),
            nodes: Vec::new(),
        };
        let mut art = HudArt::default();
        art.fonts.insert(MAP_FONT.into(), font(MAP_FONT, 15));
        let mut hud = Hud::new(&layout, &empty);
        let post = |x: f32| MapIcon {
            pos: [x, 0.0],
            texture: "hud_flag_icon".into(),
            scale: 2.5,
            color: crate::hud_map::FRIEND,
            ..MapIcon::default()
        };
        let map = MapModel {
            extent: 400.0,
            posts: vec![post(-100.0), post(150.0)],
            ..MapModel::default()
        }
        .spawn_view(Some(1));
        hud.map = Some(map.clone());
        hud.spawn = Some(map_page());
        let size = [1280.0, 720.0];
        let f = crate::hud_draw::draw(&hud, &art, size);
        let b = f
            .batches
            .iter()
            .find(|b| b.texture == "hud_icon_flash")
            .unwrap();
        let vs: Vec<[f32; 3]> = f.indices[b.first as usize..(b.first + b.count) as usize]
            .iter()
            .map(|i| f.verts[*i as usize].pos)
            .collect();
        let mid = [0, 1].map(|a| {
            let (lo, hi) = vs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                (lo.min(v[a]), hi.max(v[a]))
            });
            (lo + hi) * 0.5
        });
        let scale = map_scale(&hud).unwrap();
        assert_eq!(scale, glam::Vec3::splat(0.6));
        let hit = spawn_window_point(&map, scale, [150.0, 0.0], true, size).unwrap();
        assert!(
            (mid[0] - hit[0]).abs() < 1.5 && (mid[1] - hit[1]).abs() < 1.5,
            "drawn {mid:?}, hit {hit:?}"
        );
        let posts = [
            (0u8, glam::Vec3::new(-100.0, 0.0, 0.0)),
            (1u8, glam::Vec3::new(150.0, 0.0, 0.0)),
        ];
        assert_eq!(
            hit_spawn_post(&map, scale, &posts, mid, size, 28.0),
            Some(1)
        );
        // The old fixed 0.9 puts the click target off the drawn post.
        let old =
            spawn_window_point(&map, glam::Vec3::splat(0.9), [150.0, 0.0], true, size).unwrap();
        assert!((old[0] - mid[0]).abs() > 28.0, "old {old:?}, drawn {mid:?}");
    }

    #[test]
    fn side_row_hit_test_identifies_team_rows_accurately() {
        let size = [640.0, 480.0];
        // Title row is at SIDE_STACK_TOP (k=0 in row layout, but not a team item).
        let title_pt = window_point(
            [
                (SIDE_STACK_X[0] + SIDE_STACK_X[1]) * 0.5,
                SIDE_STACK_TOP + 10.0,
            ],
            size,
        );
        assert_eq!(
            side_row_at(title_pt, size, 3),
            None,
            "title row is not clickable"
        );

        // Row 0 = Team 1 (k=1 in row layout: SIDE_STACK_TOP + ROW_PITCH)
        let team1_pt = window_point(
            [
                (SIDE_STACK_X[0] + SIDE_STACK_X[1]) * 0.5,
                SIDE_STACK_TOP + ROW_PITCH + 10.0,
            ],
            size,
        );
        assert_eq!(side_row_at(team1_pt, size, 3), Some(0), "row 0 hits team 1");

        // Row 1 = Team 2 (k=2 in row layout: SIDE_STACK_TOP + ROW_PITCH * 2)
        let team2_pt = window_point(
            [
                (SIDE_STACK_X[0] + SIDE_STACK_X[1]) * 0.5,
                SIDE_STACK_TOP + ROW_PITCH * 2.0 + 10.0,
            ],
            size,
        );
        assert_eq!(side_row_at(team2_pt, size, 3), Some(1), "row 1 hits team 2");

        // Row 2 = Auto Assign (k=3: SIDE_STACK_TOP + ROW_PITCH * 3)
        let auto_pt = window_point(
            [
                (SIDE_STACK_X[0] + SIDE_STACK_X[1]) * 0.5,
                SIDE_STACK_TOP + ROW_PITCH * 3.0 + 10.0,
            ],
            size,
        );
        assert_eq!(
            side_row_at(auto_pt, size, 3),
            Some(2),
            "row 2 hits auto assign"
        );

        // Outside to left
        let left_pt = window_point(
            [SIDE_STACK_X[0] - 10.0, SIDE_STACK_TOP + ROW_PITCH + 10.0],
            size,
        );
        assert_eq!(side_row_at(left_pt, size, 3), None);
    }
}
