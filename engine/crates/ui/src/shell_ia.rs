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
//! The mission browser widens its side panels with the window while keeping
//! the centre controls at the same scale. The options pages use the shell frame.

use crate::hud_draw::{color, Ctx, Quad};
use crate::hud_tree::HAlign;
use crate::input_icons::Prompt;
use crate::shell::{
    InstantAction, ListRow, Page, Piece, State, COMMANDS, ERA_ROWS, LIST_ROWS, OPTION_ROWS,
};
use crate::shell_ps2::{BAR_PAD, CENTRE_LIFT, CENTRE_X, CONTAINER_Y};
use crate::spawn_draw::{item_pieces, screen_at, Anchor, PIECE_ALPHA, TITLE_PIECES};
use crate::typography::{self, tint, Tint, BODY, ITEM, LIST, ROW_H, ROW_PITCH, TITLE};
use glam::Mat4;
use manifest::hud::HudFont;

/// The backdrop: `iface_instant_action`'s top 448 rows (§1.1, re-bf2-shell
/// §3.4).
pub const BACKDROP: &str = "iface_instant_action";
/// The safe area's top: `ScreenRelativeY 0` (§1.2 [M]).
const SAFE_TOP: f32 = 22.4;
/// The two list windows (f34 pc 97-190): centre y 175.5, 183 tall. Across,
/// the script's centre x 256 ∓ 146 / + 142 and width 172.8 put the borders
/// at 23.6..196.4 and 311.6..484.4. Inset the left window from the
/// artwork's centre trim and widen both windows at the outside edges.
const LEFT_WINDOW: [f32; 4] = [
    19.2,
    SAFE_TOP + 175.5 - 91.5,
    188.0,
    SAFE_TOP + 175.5 + 91.5,
];
const RIGHT_WINDOW: [f32; 4] = [312.8, LEFT_WINDOW[1], 489.6, LEFT_WINDOW[3]];
/// The info box (f34 pc 191-240): y 309, 76 tall; across the script's x -2
/// and safe width 460.8 give 23.6..484.4, `[M]` the frames 19.2..489.6.
const INFO_BOX: [f32; 4] = [
    19.2,
    SAFE_TOP + 309.0 - 38.0,
    489.6,
    SAFE_TOP + 309.0 + 38.0,
];
/// The info text starts four units below its panel.
const INFO_TEXT_Y: f32 = INFO_BOX[1] + 4.0;
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
const ARROW: [f32; 3] = [-32.0, 0.0, 16.0];
/// The window title bar: 28 tall, 4 above the window (buttonwindow f0);
/// `[M]` as wide as the window (the frames' bar spans the window, not the
/// script's width - 26).
const TITLE_H: f32 = 28.0;
const TITLE_GAP: f32 = 4.0;
/// `border_3_pieces`: 16-unit corners (buttonwindow main.f0) over 16 of the
/// sheet's 64 texels.
const BORDER: f32 = 16.0;
/// The screen of the backdrop's centre device on the PS2 art, in shell
/// units: where the chrome frame's brightness falls halfway to the dark
/// screen inside it ([`device_screen`] on the PS2 `iface_instant_action`).
/// The middle slice of the backdrop is drawn texel for unit, so this is
/// where the screen is drawn. The baked Xbox backdrop's screen differs (its
/// left edge is 5 units further right): the bake registers the Xbox sheet to
/// this frame by row landmarks but crops and stretches it uniformly across
/// (`bake::shell::register_xbox_backdrop`), so part of the shift is that
/// registration, not the Xbox art alone. Each bake's own backdrop is
/// therefore measured.
pub const DEVICE_SCREEN: [f32; 4] = [197.2, 145.5, 302.1, 251.4];
/// The command bars' margins inside the device screen: left, top, right,
/// bottom; and the gap between two bars. `[M]` `ia_planet_list.png`: the
/// bars' outlines run from 2.2 px inside the frame's left edge to 3.7 px
/// inside its right (1.28 px per unit across), the stack from 5.2 px below
/// the top edge to 6.3 px above the bottom one (1.071 px per unit down),
/// 3.5 px between bars.
const STACK_INSET: [f32; 4] = [1.7, 4.9, 2.9, 5.9];
const STACK_GAP: f32 = 3.3;

/// The four command bars inside the device screen `device`.
fn command_bars(device: [f32; 4]) -> [[f32; 4]; 4] {
    let [x0, y0, x1, y1] = [
        device[0] + STACK_INSET[0],
        device[1] + STACK_INSET[1],
        device[2] - STACK_INSET[2],
        device[3] - STACK_INSET[3],
    ];
    let n = COMMANDS.len() as f32;
    let row = (y1 - y0 - STACK_GAP * (n - 1.0)) / n;
    std::array::from_fn(|k| {
        let top = y0 + (row + STACK_GAP) * k as f32;
        [x0, top, x1, top + row]
    })
}

/// Measure the device's screen on a copy's `iface_instant_action` (RGBA8,
/// `width` x `height`, its top 448 / 512 drawn over the shell's 448 units):
/// along three rows and three columns through the device, the inner edge of
/// the chrome frame is where its brightness falls halfway to the dark
/// screen. `None` when the art has no such frame where the PS2 and Xbox
/// arts have it (the PC's flat backdrop), so the caller keeps
/// [`DEVICE_SCREEN`].
pub fn device_screen(rgba: &[u8], width: u32, height: u32) -> Option<[f32; 4]> {
    let (w, h) = (width as usize, height as usize);
    if w < 64 || h < 64 || rgba.len() < w * h * 4 {
        return None;
    }
    let (per_x, per_y) = (w as f32 / 512.0, h as f32 / 512.0);
    let lum = |x: usize, y: usize| {
        let i = 4 * (y.min(h - 1) * w + x.min(w - 1));
        (f32::from(rgba[i]) + f32::from(rgba[i + 1]) + f32::from(rgba[i + 2])) / 3.0
    };
    // One scan from `from` toward `to` (texels): the brightest texel is the
    // chrome, the darkest within 10 units after it the screen.
    let edge = |sample: &dyn Fn(usize) -> f32, from: usize, to: usize, per: f32| {
        let step = |k: usize| if to >= from { from + k } else { from - k };
        let vals: Vec<f32> = (0..=from.abs_diff(to)).map(|k| sample(step(k))).collect();
        let peak = (0..vals.len()).max_by(|&a, &b| vals[a].total_cmp(&vals[b]))?;
        let reach = (10.0 * per).ceil() as usize;
        let trough = vals[peak..(peak + reach + 1).min(vals.len())]
            .iter()
            .copied()
            .fold(f32::MAX, f32::min);
        if vals[peak] - trough < 30.0 {
            return None;
        }
        let mid = (vals[peak] + trough) * 0.5;
        let j = (peak..vals.len() - 1).find(|&j| vals[j] > mid && vals[j + 1] <= mid)?;
        let f = (vals[j] - mid) / (vals[j] - vals[j + 1]);
        let at = step(j) as f32 + 0.5 + if to >= from { f } else { -f };
        Some(at / per)
    };
    let tx = |u: f32| (u * per_x) as usize;
    let ty = |v: f32| (v * per_y) as usize;
    let rows = [180.0, 200.0, 220.0];
    let cols = [240.0, 250.0, 260.0];
    let side = |lines: &[f32], across: bool, from: f32, to: f32| -> Option<f32> {
        let mut found = Vec::new();
        for &line in lines {
            let e = if across {
                let y = ty(line);
                edge(&|x| lum(x, y), tx(from), tx(to), per_x)
            } else {
                let x = tx(line);
                edge(&|y| lum(x, y), ty(from), ty(to), per_y)
            };
            found.push(e?);
        }
        let (lo, hi) = found
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        // A frame, not a scratch: the three scans agree.
        (hi - lo <= 2.0).then(|| found.iter().sum::<f32>() / found.len() as f32)
    };
    let screen = [
        side(&rows, true, 185.0, 215.0)?,
        side(&cols, false, 130.0, 160.0)?,
        side(&rows, true, 315.0, 285.0)?,
        side(&cols, false, 265.0, 235.0)?,
    ];
    // Inside the backdrop's middle slice, which is drawn texel for unit.
    (screen[0] >= 192.0 && screen[2] <= 308.0 && screen[2] - screen[0] >= 80.0).then_some(screen)
}

/// Keep the middle of the original artwork registered with the command
/// buttons. Each outside slice widens by the same amount as its list.
struct MissionLayout {
    screen: crate::hud_draw::Screen,
    extra: f32,
    height: f32,
}

impl MissionLayout {
    fn new(size: [f32; 2]) -> Self {
        let height = size[1].min(size[0] * 0.75);
        let mut screen = screen_at([size[0], height], Anchor::Centre);
        screen.origin[1] = (size[1] - height) * 0.5;
        let extra = (screen.origin[0] / screen.scale[0]).max(0.0);
        Self {
            screen,
            extra,
            height,
        }
    }

    fn point(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            (x - self.screen.origin[0]) / self.screen.scale[0],
            (y - self.screen.origin[1]) / self.screen.scale[1],
        ]
    }

    fn left(&self) -> [f32; 4] {
        let [left, top, right, bottom] = LEFT_WINDOW;
        [left - self.extra, top, right, bottom]
    }

    fn right(&self) -> [f32; 4] {
        let [left, top, right, bottom] = RIGHT_WINDOW;
        [left, top, right + self.extra, bottom]
    }

    fn info(&self) -> [f32; 4] {
        let [left, top, right, bottom] = INFO_BOX;
        [left - self.extra, top, right + self.extra, bottom]
    }
}

/// Which list is under the pointer, including the frame used for scrolling.
pub(crate) fn list_under_pointer(point: [f32; 2], size: [f32; 2]) -> Option<bool> {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return None;
    }
    let layout = MissionLayout::new(size);
    let [x, y] = layout.point(point);
    [layout.left(), layout.right()]
        .into_iter()
        .zip([false, true])
        .find_map(|([left, top, right, bottom], playlist)| {
            ((left..=right).contains(&x) && (top..=bottom).contains(&y)).then_some(playlist)
        })
}

/// Hit-test the visible row geometry used by the Instant Action renderer.
pub fn pointer_target(
    ia: &InstantAction,
    point: [f32; 2],
    size: [f32; 2],
    font: &HudFont,
    device: [f32; 4],
) -> Option<crate::shell::PointerTarget> {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return None;
    }
    let box_w = 4.0 / 3.0 * size[1];
    let x = (point[0] - (size[0] - box_w) * 0.5) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    match ia.page {
        Page::Missions => {
            let layout = MissionLayout::new(size);
            let [x, y] = layout.point(point);
            let left = layout.left();
            let right = layout.right();
            for (k, (&command, [x0, y0, x1, y1])) in
                COMMANDS.iter().zip(command_bars(device)).enumerate()
            {
                if (x0..=x1).contains(&x) && (y0..=y1).contains(&y) && ia.command_live(command) {
                    return Some(crate::shell::PointerTarget::Command(k));
                }
            }
            let play_rows = ia.play_rows();
            let rx0 = right[0] + LIST_IN[0];
            let rx1 = right[2] - LIST_IN[1];
            let rcx = (rx0 + rx1) * 0.5;
            let ry0 = right[1] + LIST_PAD;
            let rlast = ry0 + ROW * LIST_ROWS.min(play_rows.len()) as f32;
            let (above, below) = ia.play_list.more(play_rows.len(), LIST_ROWS);
            if (rcx - 32.0..=rcx + 32.0).contains(&x) {
                if above && (ry0 - 16.0..=ry0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollRight(-1));
                }
                if below && (rlast..=rlast + 16.0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollRight(1));
                }
            }
            if ia.state == State::Playlist && x >= rx0 && x <= rx1 {
                if let Some(k) = ia
                    .play_list
                    .row_at(y - ry0, ROW, LIST_ROWS, play_rows.len())
                {
                    return Some(crate::shell::PointerTarget::Playlist(k));
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
            let lx0 = left[0] + LIST_IN[0];
            let lx1 = left[2] - LIST_IN[1];
            let lcx = (lx0 + lx1) * 0.5;
            let ly0 = left[1] + pad;
            let llast = ly0 + row_h * shown.min(rows.len()) as f32;
            let (above, below) = cur.more(rows.len(), shown);
            if (lcx - 32.0..=lcx + 32.0).contains(&x) {
                if above && (ly0 - 16.0..=ly0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollLeft(-1));
                }
                if below && (llast..=llast + 16.0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollLeft(1));
                }
            }
            if !matches!(ia.state, State::Command | State::Playlist) && x >= lx0 && x <= lx1 {
                if let Some(k) = cur.row_at(y - ly0, row_h, shown, rows.len()) {
                    if rows[k].enabled {
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
                .map(|label| typography::width(font, &ITEM, label))
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
            let (above, below) = cur.more(rows.len(), OPTION_ROWS);
            if (cx - 32.0..=cx + 32.0).contains(&x) {
                if above && (y0 - 16.0..=y0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollOption(-1));
                }
                if below && (last..=last + 16.0).contains(&y) {
                    return Some(crate::shell::PointerTarget::ScrollOption(1));
                }
            }
            if let Some(k) = cur
                .row_at(y - y0, OPTION_ROW, OPTION_ROWS, rows.len())
                .filter(|_| x >= x0 && x <= x1)
            {
                if rows.get(k).is_some_and(|o| ia.options.option_enabled(*o)) {
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
/// `[ours]` drawn as every paragraph is ([`BODY`], white).
const OPTION_INFO_TEXT: (f32, f32) = (10.0, 65.0 / 4.0 - 10.0);

/// The "list full" popup `[ours]` geometry (`Popup_Ok`'s is not read).
const POPUP: [f32; 4] = [136.0, 184.0, 376.0, 264.0];

const NONE: usize = usize::MAX;

type Style = Tint;

/// A list row's colour: the row at the list's index blue, the rest white,
/// `[ours]` dimmed grey 110 (§7).
fn row_style(row: &ListRow, at: bool) -> Style {
    if !row.enabled {
        tint::DIMMED
    } else if at {
        tint::SELECTED
    } else {
        tint::UNSELECTED
    }
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
        self.text_in(s, &TITLE, title, bar, tint::TITLE);
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

    /// `listbox_cursor` around a row: its line sits at texels 3..61 × 4..54
    /// of the sheet, 2 texels thick; drawn with thin corners `[ours]`.
    pub(crate) fn cursor_box(&mut self, s: &crate::hud_draw::Screen, r: [f32; 4]) {
        let uv = [3.0 / 64.0, 4.0 / 64.0, 61.0 / 64.0, 54.0 / 64.0];
        self.nine(s, r, uv, (4.0, 6.0 / 64.0), "listbox_cursor");
    }

    /// One list in a window: its rows from the view's `offset` (cut at the
    /// window), the cursor box when the list has the focus, the scroll
    /// arrows when it scrolls. Every row is [`LIST`] at its own size; a row
    /// too long for the window ends in an ellipsis.
    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        s: &crate::hud_draw::Screen,
        win: [f32; 4],
        rows: &[ListRow],
        cur: crate::shell::ListCursor,
        shown: usize,
        row_h: f32,
        focused: bool,
    ) {
        let x0 = win[0] + LIST_IN[0];
        let x1 = win[2] - LIST_IN[1];
        let cx = (x0 + x1) * 0.5;
        let era = row_h > ROW;
        let y0 = win[1] + if era { ERA_PAD } else { LIST_PAD };
        let icon = if era { ERA_ICON } else { MODE_ICON };
        let at = cur.at;
        let offset = cur.offset();
        let [_, cy0] = s.map(glam::Vec3::new(0.0, y0, 0.0));
        let [_, cy1] = s.map(glam::Vec3::new(0.0, y0 + row_h * shown as f32, 0.0));
        self.clip = Some([f32::MIN, cy0, f32::MAX, cy1]);
        for k in cur.view.visible(rows.len(), shown) {
            let row = &rows[k];
            let y = y0 + row_h * (k as f32 - offset);
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
            let lines: Vec<String> = if era {
                self.wrapped(&LIST, &row.text, width)
                    .iter()
                    .map(|l| self.ellipsized(&LIST, l, width))
                    .collect()
            } else {
                vec![self.ellipsized(&LIST, &row.text, width)]
            };
            let ty = y + (row_h - LIST.line * lines.len() as f32) * 0.5;
            for (j, line) in lines.iter().enumerate() {
                let at = (x0 + TEXT_X, ty + LIST.line * j as f32);
                self.text(s, &LIST, line, at, HAlign::Left, style);
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
        self.clip = None;
        let last = y0 + row_h * shown.min(rows.len()) as f32;
        let (above, below) = cur.more(rows.len(), shown);
        self.more_arrows(s, cx, [y0, last], above, below);
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
        let layout = MissionLayout::new(size);
        let s = layout.screen;
        let t = &ia.texts;
        let help_size = [size[0], layout.height];
        let slice = [0.0, 192.0, 308.0, 512.0];
        let drawn = [-layout.extra, slice[1], slice[2], 512.0 + layout.extra];
        for k in 0..3 {
            self.tinted(
                &s,
                [drawn[k], 0.0, drawn[k + 1], 448.0],
                [slice[k] / 512.0, 0.0, slice[k + 1] / 512.0, 0.875],
                BACKDROP,
                ([255.0; 3], 1.0),
            );
        }
        if ia.full {
            self.window_box(&s, POPUP, None);
            let lines = self.wrapped(&BODY, &ia.texts.list_full, POPUP[2] - POPUP[0] - 20.0);
            let y = (POPUP[1] + POPUP[3]) * 0.5 - BODY.line * lines.len() as f32 * 0.5;
            for (j, l) in lines.iter().enumerate() {
                let at = (256.0, y + BODY.line * j as f32);
                self.text(&s, &BODY, l, at, HAlign::Center, tint::UNSELECTED);
            }
            self.help_bottom(help_size, true, Prompt::Accept, &t.select, size[1]);
            return;
        }
        // The left window: Planet, or Mode / Era in their states (f12).
        let (title, rows, cur, shown, row_h) = match ia.state {
            State::Mode => (&t.mode, ia.mode_rows(), ia.mode_list, LIST_ROWS, ROW),
            State::Era => (&t.era, ia.side_rows(), ia.era_list, ERA_ROWS, ERA_ROW),
            _ => (&t.planet, ia.map_rows(), ia.map_list, LIST_ROWS, ROW),
        };
        let left_focus = matches!(ia.state, State::Map | State::Mode | State::Era);
        let left = layout.left();
        let right = layout.right();
        self.window_box(&s, left, Some(title));
        self.list(&s, left, &rows, cur, shown, row_h, left_focus);
        self.window_box(&s, right, Some(&t.playlist));
        let play = ia.play_rows();
        let pc = ia.play_list;
        let focus = ia.state == State::Playlist;
        self.list(&s, right, &play, pc, LIST_ROWS, ROW, focus);
        // The command stack: square notched bars, `gamefont_tiny` labels
        // (main pc 87); the kept focus blue and pulsing even while a list
        // has it (f11; menuref §9.1.9: ADD pulses beside the solid list
        // cursor).
        let bars = command_bars(self.art.instant_device.unwrap_or(DEVICE_SCREEN));
        for (k, (c, r)) in COMMANDS.iter().zip(bars).enumerate() {
            if k == ia.command {
                self.bar_tinted(&s, r, item_pieces(false), ([255.0; 3], pulse));
            } else {
                self.bar_grey(&s, r, item_pieces(false), BAR_GREY);
            }
            let style = if k == ia.command {
                (tint::SELECTED.0, pulse)
            } else if ia.command_live(*c) {
                tint::UNSELECTED
            } else {
                tint::DIMMED
            };
            self.text_in(&s, &LIST, t.command(*c).0, r, style);
        }
        // The info box (f9).
        let info = layout.info();
        self.window_box(&s, info, None);
        let (x, y, w) = (info[0] + 10.0, INFO_TEXT_Y, info[2] - info[0] - 20.0);
        for (j, line) in self.wrapped(&LIST, &ia.info(), w).iter().enumerate() {
            let at = (x, y + LIST.line * j as f32);
            self.text(&s, &LIST, line, at, HAlign::Left, tint::SELECTED);
        }
        self.help_bottom(help_size, false, Prompt::Back, &t.back, size[1]);
        self.help_bottom(help_size, true, Prompt::Accept, &t.select, size[1]);
    }

    /// "Session Options": the PS2 stack rule of `crate::shell_ps2`
    /// (`stack_layout`'s [M] widths), centred; dimmed groups grey (§2.1).
    fn overview(&mut self, ia: &InstantAction, focus: usize, pulse: f32, size: [f32; 2]) {
        let s = screen_at(size, Anchor::Centre);
        let t = &ia.texts;
        let Some(font) = self.face(ITEM.face) else {
            return;
        };
        let rows: Vec<(&str, bool)> = ia
            .overview()
            .into_iter()
            .map(|(g, live)| (t.group(g).0, live))
            .collect();
        let widest = std::iter::once(t.session_options.as_str())
            .chain(rows.iter().map(|(l, _)| *l))
            .map(|l| typography::width(font, &ITEM, l))
            .fold(0.0, f32::max);
        let w = widest + BAR_PAD;
        let x0 = CENTRE_X - w * 0.5;
        let bars = rows.len() + 1;
        let height = bars as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
        let top = CONTAINER_Y - CENTRE_LIFT - height * 0.5;
        let bar = |k: usize| {
            let y = top + ROW_PITCH * k as f32;
            [x0, y, x0 + w, y + ROW_H]
        };
        self.bar_grey(&s, bar(0), TITLE_PIECES, 255.0);
        self.text_in(&s, &TITLE, &t.session_options, bar(0), tint::TITLE);
        for (k, (label, live)) in rows.iter().enumerate() {
            let r = bar(k + 1);
            let pieces = item_pieces(k + 1 == rows.len());
            if k == focus {
                self.bar_tinted(&s, r, pieces, ([255.0; 3], pulse));
            } else {
                self.bar_grey(&s, r, pieces, BAR_GREY);
            }
            let style = if !live {
                tint::DIMMED
            } else if k == focus {
                (tint::SELECTED.0, pulse)
            } else {
                tint::UNSELECTED
            };
            self.text_in(&s, &ITEM, label, r, style);
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
        let rows = ia.options.rows(g);
        let offset = cur.offset();
        let [_, cy0] = s.map(glam::Vec3::new(0.0, y0, 0.0));
        let [_, cy1] = s.map(glam::Vec3::new(
            0.0,
            y0 + OPTION_ROW * OPTION_ROWS as f32,
            0.0,
        ));
        self.clip = Some([f32::MIN, cy0, f32::MAX, cy1]);
        for k in cur.view.visible(rows.len(), OPTION_ROWS) {
            let o = &rows[k];
            let y = y0 + OPTION_ROW * (k as f32 - offset);
            let live = ia.options.option_enabled(*o);
            if live && k == cur.at {
                let m = OPTION_ROW * (1.0 - CURSOR_H) * 0.5;
                self.cursor_box(&s, [x0, y + m, x1, y + OPTION_ROW - m]);
            }
            let style = if !live {
                tint::DIMMED
            } else if k == cur.at {
                tint::SELECTED
            } else {
                tint::UNSELECTED
            };
            // `[ours]` the rows are a settings row's role ([`ITEM`]), as on
            // every other options page.
            let ty = y + (OPTION_ROW - ITEM.size) * 0.5;
            let mut x = x0 + OPTION_TEXT_X;
            for p in t.option(*o, &ia.options) {
                match p {
                    Piece::Text(text) => {
                        self.text(&s, &ITEM, &text, (x, ty), HAlign::Left, style);
                        x += self.text_width(&ITEM, &text);
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
        self.clip = None;
        // [ours] Only the Hero page, with the DLC row, holds more than fit.
        let last = y0 + OPTION_ROW * OPTION_ROWS as f32;
        let (above, below) = cur.more(rows.len(), OPTION_ROWS);
        self.more_arrows(&s, cx, [y0, last], above, below);
        let info = OPTION_INFO;
        let grey = ([128.0; 3], 0.5);
        self.tinted(&s, info, [0.0, 0.0, 1.0, 1.0], "white_rect", grey);
        if let Some(o) = rows.get(cur.at) {
            let (dx, dy) = OPTION_INFO_TEXT;
            let w = info[2] - info[0] - 2.0 * dx;
            let help = ia
                .options
                .population_help(*o)
                .unwrap_or_else(|| t.help(*o).to_owned());
            for (j, line) in self.wrapped(&BODY, &help, w).iter().enumerate() {
                let y = info[1] + dy.max(4.0) + BODY.line * j as f32;
                let at = (info[0] + dx, y);
                self.text(&s, &BODY, line, at, HAlign::Left, tint::BODY);
            }
        }
        self.help(size, false, Prompt::Back, &t.back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BUTTON_FONT: &str = "gamefont_large";
    const SMALL_FONT: &str = "gamefont_small";
    const LIST_FONT: &str = "gamefont_tiny";
    use crate::typography::tint::DIMMED_GREY;
    use crate::typography::wrap as role_wrap;

    /// `[ours]` the lines the box holds: its third line's glyphs (18 tall) end
    /// at 6.25 + 2 × 19.6 + 18 = 63.45 of its 65. The draw does not clip, so a
    /// fourth line would spill below it; every help line is held to three.
    const OPTION_INFO_LINES: usize = 3;

    /// `gamefont_small`'s advances for ' '..='~' (`common_bf2/fonts/
    /// gamefont_small.ron`), so the check runs without the bake.
    const SMALL_ADVANCES: [u8; 95] = [
        4, 5, 10, 15, 11, 14, 12, 4, 4, 5, 11, 11, 4, 11, 4, 8, 11, 7, 11, 11, 12, 11, 11, 11, 11,
        11, 5, 5, 11, 11, 11, 9, 13, 11, 11, 11, 11, 10, 10, 11, 11, 5, 12, 11, 9, 13, 12, 11, 11,
        11, 11, 11, 11, 12, 11, 16, 13, 11, 11, 8, 8, 8, 8, 11, 8, 12, 11, 11, 11, 12, 6, 12, 12,
        7, 6, 11, 5, 15, 11, 11, 12, 12, 10, 11, 7, 12, 11, 16, 13, 11, 11, 11, 3, 11, 12,
    ];

    fn small_font() -> HudFont {
        use manifest::hud::HudGlyph;
        HudFont {
            name: SMALL_FONT.into(),
            line_height: 18,
            pages: vec!["page0".into()],
            glyphs: (32u16..127)
                .zip(SMALL_ADVANCES)
                .map(|(code, advance)| HudGlyph {
                    code,
                    page: 0,
                    advance,
                    x0: 0,
                    x1: advance,
                    y0: 0,
                    y1: 18,
                    uv: [0.0; 4],
                })
                .collect(),
        }
    }

    /// The fixture is the bake's font, and the optional menu face's (which
    /// keeps its advances), where the bake is present.
    #[test]
    fn small_font_fixture_matches_the_bake() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets_baked/common_bf2/fonts"
        );
        for name in ["gamefont_small", "menu_gamefont_small"] {
            let path = format!("{dir}/{name}.ron");
            match std::fs::read_to_string(&path) {
                Ok(text) => small_font_matches(&text),
                Err(_) => eprintln!("skip: no {path}"),
            }
        }
    }

    fn small_font_matches(text: &str) {
        // The ui crate reads no RON: each glyph's `code:` then its `advance:`.
        let field = |at: &str, key: &str| -> u16 {
            let rest = at[at.find(key).expect(key) + key.len()..].trim_start();
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            rest[..end].parse().expect(key)
        };
        let baked: Vec<(u16, u16)> = text
            .match_indices("code:")
            .map(|(i, _)| (field(&text[i..], "code:"), field(&text[i..], "advance:")))
            .collect();
        let fixture = small_font();
        for g in &fixture.glyphs {
            let advance = baked.iter().find(|b| b.0 == g.code).map(|b| b.1);
            assert_eq!(
                advance,
                Some(u16::from(g.advance)),
                "{:?}",
                char::from_u32(g.code.into())
            );
        }
        assert_eq!(field(text, "line_height:"), u16::from(fixture.line_height));
    }

    /// Issue #172: every Session Options help line, the localized rows'
    /// English and openbf's own (Team size, Reinforcements, Assault AI
    /// Units) at every value that changes its width, wraps to the lines
    /// the box holds at the box's width.
    #[test]
    fn every_shell_help_line_fits_the_help_box() {
        use crate::population::{Defaults, Entry};
        use crate::shell::{IaTexts, SessionOptions, GROUPS};
        let font = small_font();
        let width = OPTION_INFO[2] - OPTION_INFO[0] - 2.0 * OPTION_INFO_TEXT.0;
        let texts = IaTexts::new(&|_, e| e.to_owned());
        static TABLE: [Entry; 2] = [
            (
                "wide",
                false,
                Defaults {
                    combatants: [32, 32],
                    tickets: [999, 999],
                },
            ),
            (
                "small",
                true,
                Defaults {
                    combatants: [8, 8],
                    tickets: [50, 50],
                },
            ),
        ];
        let mut checked = 0;
        for dlc in [false, true] {
            for pct in [10, 100, 200, 500] {
                for ass in [0, 9, 10, 12, 16] {
                    let o = SessionOptions {
                        population_table: &TABLE,
                        num_bots: pct.min(200),
                        con_numbots: pct.min(200),
                        con_mult: pct,
                        ass_numbots: ass,
                        dlc_heroes_installed: dlc,
                        dlc_heroes: dlc,
                        ..Default::default()
                    };
                    for g in GROUPS {
                        for row in o.rows(g) {
                            let help = o
                                .population_help(row)
                                .unwrap_or_else(|| texts.help(row).to_owned());
                            assert!(!help.is_empty(), "{row:?}");
                            let lines = role_wrap(&font, &crate::typography::BODY, &help, width);
                            assert!(
                                lines.len() <= OPTION_INFO_LINES,
                                "{row:?}: {} lines at {width}: {lines:#?}",
                                lines.len()
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 0);
    }

    /// The command stack sits inside each copy's device screen with the
    /// original's margins, the same in shell units at every window size.
    #[test]
    fn the_command_bars_fit_the_device_screen_of_each_copy() {
        use crate::hud_draw::HudArt;
        use crate::hud_tree::Hud;
        use crate::shell::{IaTexts, InstantAction, SessionOptions};
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
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        // The PS2 art's screen and the Xbox bake's (measured on the baked
        // `iface_instant_action` of each copy, the Xbox one after the bake's
        // registration), and no art measured.
        let ps2 = [197.2, 145.5, 302.1, 251.4];
        let xbox = [202.5, 145.4, 302.9, 252.2];
        let pieces = item_pieces(false);
        for (device, expect) in [(Some(ps2), ps2), (Some(xbox), xbox), (None, ps2)] {
            let mut art = HudArt::default();
            for f in [LIST_FONT, SMALL_FONT, BUTTON_FONT] {
                art.fonts.insert(f.into(), font(f));
            }
            art.instant_device = device;
            for size in [
                [640.0, 480.0],
                [1024.0, 768.0],
                [600.0, 900.0],
                [1280.0, 720.0],
                [1920.0, 1094.0],
                [1920.0, 1200.0],
                [3440.0, 1440.0],
                [3839.0, 1960.0],
            ] {
                let mut hud = Hud::new(&empty, &empty);
                let mut menus = crate::shell_ps2::ShellMenus::new(&|_, e| e.to_owned());
                menus.covered = true;
                hud.shell = Some((menus, false));
                hud.instant = Some(Box::new(InstantAction::new(
                    Vec::new(),
                    IaTexts::new(&|_, e| e.to_owned()),
                    SessionOptions::default(),
                )));
                let f = crate::hud_draw::draw(&hud, &art, size);
                let [mut x0, mut y0, mut x1, mut y1] = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for b in f
                    .batches
                    .iter()
                    .filter(|b| pieces.contains(&b.texture.as_str()))
                {
                    for i in &f.indices[b.first as usize..(b.first + b.count) as usize] {
                        let [x, y, _] = f.verts[*i as usize].pos;
                        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                    }
                }
                let s = MissionLayout::new(size).screen;
                let at = |x: f32, y: f32| s.map(glam::Vec3::new(x, y, 0.0));
                let [dx0, dy0] = at(expect[0], expect[1]);
                let [dx1, dy1] = at(expect[2], expect[3]);
                let gaps = [x0 - dx0, y0 - dy0, dx1 - x1, dy1 - y1];
                let want = [
                    STACK_INSET[0] * s.scale[0],
                    STACK_INSET[1] * s.scale[1],
                    STACK_INSET[2] * s.scale[0],
                    STACK_INSET[3] * s.scale[1],
                ];
                for (side, (g, w)) in ["left", "top", "right", "bottom"]
                    .iter()
                    .zip(gaps.iter().zip(want))
                {
                    assert!(
                        (g - w).abs() <= 1.0,
                        "{device:?} at {size:?}: the {side} gap is {g:.2} px, want {w:.2}"
                    );
                }
            }
        }
    }

    /// A device drawn as the arts draw it: a grey backdrop, a bright
    /// chrome band whose brightness falls off over three texels into a dark
    /// screen. The measured screen is the band's inner edge at either
    /// texture size; a flat backdrop (the PC's) measures nothing.
    #[test]
    fn the_device_screen_is_measured_on_the_art() {
        let paint = |side: u32, screen: [f32; 4]| {
            let per = side as f32 / 512.0;
            let mut rgba = vec![0u8; (side * side * 4) as usize];
            for y in 0..side {
                for x in 0..side {
                    let (u, v) = ((x as f32 + 0.5) / per, (y as f32 + 0.5) / per);
                    // Distance inside the screen's edge, in units.
                    let d = (u - screen[0])
                        .min(screen[2] - u)
                        .min(v - screen[1])
                        .min(screen[3] - v);
                    let l = if d > 1.5 {
                        15.0
                    } else if d > -1.5 {
                        // 160 at -1.5 down to 15 at 1.5: halfway at the edge.
                        87.5 - d / 1.5 * 72.5
                    } else if d > -6.0 {
                        160.0
                    } else {
                        50.0
                    };
                    let i = 4 * (y * side + x) as usize;
                    rgba[i..i + 4].copy_from_slice(&[l as u8, l as u8, l as u8, 255]);
                }
            }
            rgba
        };
        for side in [512, 1024] {
            for screen in [DEVICE_SCREEN, [202.5, 145.4, 302.9, 252.2]] {
                let got = device_screen(&paint(side, screen), side, side).unwrap();
                for k in 0..4 {
                    assert!(
                        (got[k] - screen[k]).abs() < 0.6,
                        "{side}: {got:?} {screen:?}"
                    );
                }
            }
        }
        let flat = vec![40u8; 512 * 512 * 4];
        assert_eq!(device_screen(&flat, 512, 512), None);
        assert_eq!(device_screen(&flat[..100], 512, 512), None);
    }

    /// The pointer finds a command on the bars as drawn for the copy's art.
    #[test]
    fn mouse_hit_finds_commands_on_the_fitted_bars() {
        use crate::shell::{IaTexts, SessionOptions};
        use manifest::hud::HudFont;
        let mut ia = InstantAction::new(
            Vec::new(),
            IaTexts::new(&|_, e| e.to_owned()),
            SessionOptions::default(),
        );
        ia.state = State::Command;
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 15,
            pages: Vec::new(),
            glyphs: Vec::new(),
        };
        let xbox = [202.5, 145.4, 302.9, 252.2];
        let size = [1920.0, 1094.0];
        let s = MissionLayout::new(size).screen;
        let options = COMMANDS.len() - 1;
        let [x0, y0, x1, y1] = command_bars(xbox)[options];
        let at = |x, y| s.map(glam::Vec3::new(x, y, 0.0));
        let hit = |p| pointer_target(&ia, p, size, &font, xbox);
        let want = Some(crate::shell::PointerTarget::Command(options));
        assert_eq!(hit(at(x0 + 0.5, (y0 + y1) * 0.5)), want);
        assert_eq!(hit(at(x1 - 0.5, (y0 + y1) * 0.5)), want);
        assert_eq!(hit(at(x0 - 1.0, (y0 + y1) * 0.5)), None);
    }

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
        assert_eq!(ia.map_list.top(), 0);
        let hit_up = pointer_target(
            &ia,
            to_screen(cx, y0 - 8.0),
            [640.0, 480.0],
            &font,
            DEVICE_SCREEN,
        );
        assert_eq!(hit_up, None, "no up arrow when at top");

        let hit_down = pointer_target(
            &ia,
            to_screen(cx, last + 8.0),
            [640.0, 480.0],
            &font,
            DEVICE_SCREEN,
        );
        assert_eq!(
            hit_down,
            Some(crate::shell::PointerTarget::ScrollLeft(1)),
            "down arrow is hit"
        );

        // Scroll down: now up arrow is present
        ia.scroll_left(1.0);
        assert_eq!(ia.map_list.top(), 1);
        let hit_up_scrolled = pointer_target(
            &ia,
            to_screen(cx, y0 - 8.0),
            [640.0, 480.0],
            &font,
            DEVICE_SCREEN,
        );
        assert_eq!(
            hit_up_scrolled,
            Some(crate::shell::PointerTarget::ScrollLeft(-1)),
            "up arrow is hit when scrolled"
        );

        // Editing the playlist: the map rows do not take the hover or click.
        ia.state = State::Playlist;
        let row = pointer_target(
            &ia,
            to_screen(cx, y0 + ROW * 0.5),
            [640.0, 480.0],
            &font,
            DEVICE_SCREEN,
        );
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
        let hit = pointer_target(&ia, point, [640.0, 480.0], &font, DEVICE_SCREEN);
        assert_eq!(hit, Some(crate::shell::PointerTarget::Map(0)));
        assert!(ia.focus_pointer(hit.unwrap()));
        assert_eq!(ia.map_list.at, 0);
    }

    #[test]
    fn mission_windows_fit_and_keep_the_centre_controls_registered() {
        // §1.2: the Planet window's body 106.4..289.4 (the frame 109..287).
        assert!((LEFT_WINDOW[1] - 106.4).abs() < 0.01);
        assert!((LEFT_WINDOW[3] - 289.4).abs() < 0.01);
        assert_eq!(RIGHT_WINDOW[1], LEFT_WINDOW[1]);
        // Nine rows of 17 + 30 fill the window.
        assert!(LIST_PAD + LIST_ROWS as f32 * ROW < 183.0);
        assert!((INFO_BOX[1] - (22.4 + 309.0 - 38.0)).abs() < 0.01);
        for size in [
            [640.0, 480.0],
            [3840.0, 2160.0],
            [3440.0, 1440.0],
            [600.0, 900.0],
        ] {
            let layout = MissionLayout::new(size);
            let left = layout.left();
            let right = layout.right();
            let px = |x, y| layout.screen.map(glam::Vec3::new(x, y, 0.0));
            assert!(px(left[0], left[1])[0] >= 0.0);
            assert!(px(right[2], right[3])[0] <= size[0]);
            assert_eq!(
                list_under_pointer(px(left[0] + 2.0, left[1] + 2.0), size),
                Some(false)
            );
            assert_eq!(
                list_under_pointer(px(right[2] - 2.0, right[1] + 2.0), size),
                Some(true)
            );
            let bar = command_bars(DEVICE_SCREEN)[0];
            assert_eq!(list_under_pointer(px(bar[0] + 1.0, bar[1]), size), None);
        }
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
        let [r, g, b] = crate::typography::tint::SELECTED.0.map(|c| c / 255.0);
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
