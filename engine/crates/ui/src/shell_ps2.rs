//! The BF2 PS2 shell's front screens (card BSHLA, `analysis/re-bf2-shell.md`
//! §2-§4, §9): Press START, the Main Menu, Singleplayer and Options, each
//! the PS2's vertical-button stack over the background movie, painted by the
//! HUD pass (`crate::hud_draw`) in the shell's 512 × 448 screen.
//!
//! - **Title** (`ifs_start`): `logo` at the top, the `shell_main` movie and
//!   "Press START Button To Begin" pulsing; only Start advances (§2 row 3;
//!   the keyboard's Enter is the port's Start). The legal / logo screens and
//!   the profile flow are skipped (§9.1, §9.3).
//! - **Main Menu** (`ifs_main`): logo + stack 10 units lower than the
//!   others: Singleplayer, **Multiplayer in its slot but disabled** (the
//!   PS2's own dimmed style: label grey 110, focus skips it, §3.3 - the port
//!   has no online play), Splitscreen disabled (until card SSCR), Options,
//!   then `[ours]` **Quit Game**, which exits. The PC's `ifs_main` ends in a
//!   quit row ("Exit To Windows") that the PS2 hides (`re-bf2-menus.md`,
//!   ifs_main list order); the port shows it with its own label.
//! - **Singleplayer** (`ifs_sp`, left-justified, `shell_sub_left`): Rise of
//!   the Empire and Galactic Conquest disabled (no port mode yet), Instant
//!   Action live; Training and Space Overview hidden (§9.5).
//!
//! The stack's geometry is the frames' [M] rule (§3.2), not the Lua's
//! left-justify formula (which does not reproduce them, §3.2 [H]): labels
//! 1.087 × the font's advances across, the bar 1.087 × the widest label +
//! [`BAR_PAD`], centred stacks about x [`CENTRE_X`], left-justified ones from
//! x [`LEFT_X`]; rows `font height + 6` ([`crate::spawn_draw::ROW_H`]) every
//! `+ 4` ([`crate::spawn_draw::ROW_PITCH`]); the stack's middle
//! [`CENTRE_LIFT`] above its container's y (0.6 of the screen, + 10 on the
//! main menu). Focus moves wrap last <-> first and skip disabled rows (§3.2,
//! §3.3).
//!
//! How it sounds and moves (`analysis/re-bf2-menus.md` §4.1, §5;
//! [`crate::shell_feel`]): every stack move clicks (the stacks wrap), Cross
//! on a row plays accept, Back plays the exit click of the screen it returns
//! to, Start on the title accept. The focused row's bar and label pulse
//! together ([`Pulse::BUTTON`], 0.734 s, one phase kept across moves); the
//! others sit at alpha 0.6 with a grey 150 bar. Every page entry (forward
//! or back) replays the flashy entrance: the title at once, row k
//! [`ROW_REVEAL`] × k s later. Singleplayer is always entered on its first
//! live row (the PS2's Training, menuref "Singleplayer").
//!
//! `[ours]` Hor+: the movie, the logo and centred stacks keep the 4:3 box at
//! the window's centre (the movie pillarboxed, never stretched: §9.9), a
//! left-justified stack keeps its offset from the box's left edge, the help
//! lines their offsets from the window's edges.

use crate::hud_draw::{run_width, Ctx};
use crate::hud_tree::HAlign;
use crate::input_icons::Prompt;
use crate::menu::MenuInput;
use crate::shell_feel::{flashy, Cues, MenuSound, Pulse};
use crate::spawn_draw::{
    BUTTON_FONT, LABEL_STRETCH, ROW_H, ROW_PITCH, SELECTED_TEXT, TITLE_TEXT, UNSELECTED_TEXT,
};
use manifest::hud::HudFont;

/// `gDefaultButtonScreenRelativeY` 0.6 of the 448-line screen (§3.1 [V]).
pub const CONTAINER_Y: f32 = 0.6 * 448.0;
/// The main menu's container sits 10 units lower (§3.2 [V]).
pub const MAIN_Y_ADD: f32 = 10.0;
/// The stack's middle stands this far above its container's y [M]: title
/// tops at 206 / 178 / 143.5 px of 480 on the main menu / Singleplayer /
/// Options frames (4, 5, 7 rows) - one offset fits all three to 0.2 units.
pub const CENTRE_LIFT: f32 = 8.7;
/// A centred stack's middle across [M] (main menu bar 143.75..501.25 px of
/// 640 = 115..401 units).
pub const CENTRE_X: f32 = 258.0;
/// A left-justified stack's bar left edge [M] (43 px on both the
/// Singleplayer and the Options frame: the safe area's 25.6 + 8.8).
pub const LEFT_X: f32 = 34.4;
/// The bar's outer width beyond 1.087 × its widest label's advances [M]
/// (§3.2: main menu, Singleplayer and Options agree within 0.9 units).
pub const BAR_PAD: f32 = 134.2;
/// A dimmed button's label grey (`ShowHideVerticalButtons` f7 pc 266-289
/// [V]: 110 instead of 255).
pub const DIMMED_GREY: f32 = 110.0;
/// An unfocused row's bar tint (`FUN_00381858` B2 380175-380178: 150 grey
/// unless the text style is `selected`, re-bf2-menus §5 [V]).
pub const UNFOCUSED_BAR: f32 = 150.0;
/// Seconds between one row's flashy start and the next's (`button` main.f7
/// pc 188, re-bf2-menus §5 [V]; menuref §9.4.5 [M]: ~0.1 s on the PS2).
pub const ROW_REVEAL: f32 = 0.1;
/// The flashy sweep's square (unbaked: drawn untextured until it is, §8).
pub const CURSOR_SHEET: &str = "cursor";
/// The logo: 512 × 256, centred, 38 units above the top of the safe area
/// (22.4, §3.5): §3.4 [V] says "above the top edge", and the frames put it
/// from the safe area's [M] (ink rows 36-147 px of 480 on
/// `title_pressstart.png`; from the screen's edge they land 24 px higher).
pub const LOGO: [f32; 4] = [0.0, 22.4 - 38.0, 512.0, 22.4 - 38.0 + 256.0];
/// The prompt's run top [M] (`title_pressstart.png`: its ink rows 427-441
/// px, a row's ink starts 4.5 units below its run's top), centred.
pub const PROMPT_TOP: f32 = 394.0;
/// The prompt's alpha swings between these at [`PROMPT_RATE`] a second
/// (`ifs_start` main.f2 pc 45-64 [V]).
pub const PROMPT_ALPHA: [f32; 2] = [0.4, 1.0];
pub const PROMPT_RATE: f32 = 0.5;
/// The texture name the movie's current frame is drawn from.
pub const MOVIE_SHEET: &str = "#shell_movie";

/// The movie properties (`manifest::shell`).
pub const MOVIE_MAIN: &str = "shell_main";
pub const MOVIE_SUB: &str = "shell_sub_left";
/// The music every front screen plays (`ifs_start` main.f2 sets
/// `shell_soundtrack`, §2 [V]).
pub const MUSIC: &str = "shell_soundtrack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Title,
    Main,
    Singleplayer,
    Options,
}

/// Every button the port shows; the label is the disc's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowId {
    Singleplayer,
    Multiplayer,
    Splitscreen,
    Options,
    QuitGame,
    RiseOfTheEmpire,
    GalacticConquest,
    InstantAction,
    GameOptions,
    AudioOptions,
    GraphicsOptions,
    Controls,
}

impl RowId {
    /// The disc's text key and its English fallback (`re-bf2-shell` §2,
    /// `bshl/keys_resolved.tsv`).
    pub fn key(self) -> (&'static str, &'static str) {
        match self {
            RowId::Singleplayer => ("ifs.main.sp", "Singleplayer"),
            RowId::Multiplayer => ("ifs.main.mp", "Multiplayer"),
            RowId::Splitscreen => ("ifs.main.split", "Splitscreen"),
            RowId::Options => ("ifs.main.options", "Options"),
            // Our own key: the disc's `common.quit2windows` reads "Exit To Windows".
            RowId::QuitGame => ("ifs.main.quitgame", "Quit Game"),
            RowId::RiseOfTheEmpire => ("ifs.sp.riseempire", "Rise of the Empire"),
            RowId::GalacticConquest => ("ifs.sp.meta", "Galactic Conquest"),
            RowId::InstantAction => ("ifs.sp.ia", "Instant Action"),
            RowId::GameOptions => ("ifs.GameOpt.title", "Game Options"),
            RowId::AudioOptions => ("ifs.SoundOpt.title", "Audio Options"),
            RowId::GraphicsOptions => ("openbf.graphicsoptions", "Graphics Options"),
            RowId::Controls => ("ifs.controls.Vehicle_Unit", "Controls"),
        }
    }
}

/// The other texts the screens show: (key, English).
pub const TEXT_PROMPT: (&str, &str) = ("ifs.start.press_start", "Press START Button To Begin");
pub const TEXT_MAIN: (&str, &str) = ("ifs.main.title", "Main Menu");
pub const TEXT_BACK: (&str, &str) = ("common.back", "Back");
pub const TEXT_SELECT: (&str, &str) = ("common.select", "Select");

#[derive(Debug, Clone, PartialEq)]
pub struct StackRow {
    pub id: RowId,
    pub label: String,
    /// `false` = the PS2's `bDimmed`: drawn grey, never focused.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub title: String,
    pub rows: Vec<StackRow>,
    pub focus: usize,
    pub left: bool,
    /// The container's extra y (the main menu's 10).
    pub y_add: f32,
}

impl Stack {
    /// The next enabled row from `focus` going `step` (+1 down, -1 up),
    /// wrapping (`FollowButtonLink`, §3.3); `focus` when none other is.
    pub fn follow(&self, step: isize) -> usize {
        let n = self.rows.len() as isize;
        let mut k = self.focus as isize;
        for _ in 0..n {
            k = (k + step).rem_euclid(n);
            if self.rows[k as usize].enabled {
                return k as usize;
            }
        }
        self.focus
    }

    /// The first enabled row (`TagOfFirst` skips dimmed ones).
    pub fn first(&self) -> usize {
        self.rows.iter().position(|r| r.enabled).unwrap_or(0)
    }
}

/// What a step asks the game to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellCommand {
    /// Singleplayer → Instant Action: today's IA setup (until card BSHLB).
    InstantAction,
    Settings(crate::menu::SettingsPage),
    /// Back on the title screen, or the main menu's Quit Game row (`[ours]`:
    /// the PS2 has no way out).
    Quit,
}

/// The shell's screens and where each one's focus is.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellMenus {
    pub page: Page,
    pub main: Stack,
    pub singleplayer: Stack,
    pub options: Stack,
    pub prompt: String,
    pub back: String,
    pub select: String,
    /// Seconds since the page was entered (the reveal) and since the shell
    /// started (the prompt's pulse).
    pub on_page: f32,
    pub clock: f32,
    /// A page of the port's own is up over the screen (the Instant Action
    /// setup, the settings): only the movie draws.
    pub covered: bool,
    /// The clicks asked for, for the game to play.
    pub cues: Cues,
    /// The focused button's pulse (one phase, shared by every screen; a
    /// list's cursor box does not pulse, menuref §9.1.9).
    pub pulse: Pulse,
}

impl ShellMenus {
    /// The four screens, their texts looked up by `text(key, English)`.
    pub fn new(text: &dyn Fn(&str, &str) -> String) -> ShellMenus {
        let row = |id: RowId, enabled: bool| {
            let (k, e) = id.key();
            StackRow {
                id,
                label: text(k, e),
                enabled,
            }
        };
        let stack = |title: String, rows: Vec<StackRow>, left: bool, y_add: f32| {
            let mut s = Stack {
                title,
                rows,
                focus: 0,
                left,
                y_add,
            };
            s.focus = s.first();
            s
        };
        let (k, e) = RowId::Singleplayer.key();
        let sp_title = text(k, e);
        let (k, e) = RowId::Options.key();
        let opt_title = text(k, e);
        ShellMenus {
            page: Page::Title,
            main: stack(
                text(TEXT_MAIN.0, TEXT_MAIN.1),
                vec![
                    row(RowId::Singleplayer, true),
                    row(RowId::Multiplayer, false),
                    row(RowId::Splitscreen, false),
                    row(RowId::Options, true),
                    row(RowId::QuitGame, true),
                ],
                false,
                MAIN_Y_ADD,
            ),
            singleplayer: stack(
                sp_title,
                vec![
                    row(RowId::RiseOfTheEmpire, false),
                    row(RowId::GalacticConquest, false),
                    row(RowId::InstantAction, true),
                ],
                true,
                0.0,
            ),
            options: stack(
                opt_title,
                vec![
                    row(RowId::GameOptions, true),
                    row(RowId::AudioOptions, true),
                    row(RowId::GraphicsOptions, true),
                    row(RowId::Controls, true),
                ],
                true,
                0.0,
            ),
            prompt: text(TEXT_PROMPT.0, TEXT_PROMPT.1),
            back: text(TEXT_BACK.0, TEXT_BACK.1),
            select: text(TEXT_SELECT.0, TEXT_SELECT.1),
            on_page: 0.0,
            clock: 0.0,
            covered: false,
            cues: Cues::default(),
            pulse: Pulse::BUTTON,
        }
    }

    pub fn stack(&self) -> Option<&Stack> {
        match self.page {
            Page::Title => None,
            Page::Main => Some(&self.main),
            Page::Singleplayer => Some(&self.singleplayer),
            Page::Options => Some(&self.options),
        }
    }

    fn stack_mut(&mut self) -> Option<&mut Stack> {
        match self.page {
            Page::Title => None,
            Page::Main => Some(&mut self.main),
            Page::Singleplayer => Some(&mut self.singleplayer),
            Page::Options => Some(&mut self.options),
        }
    }

    /// Move focus to an enabled row; disabled and missing rows are ignored.
    pub fn focus_row(&mut self, row: usize) -> bool {
        let Some(stack) = self.stack_mut() else {
            return false;
        };
        if !stack.rows.get(row).is_some_and(|r| r.enabled) {
            return false;
        }
        stack.focus = row;
        true
    }

    fn go(&mut self, page: Page) {
        self.page = page;
        self.on_page = 0.0;
        // Back on Singleplayer the focus is its first row (Training on the
        // PS2) whatever was left (menuref "Singleplayer", `ia_enter_back`).
        if page == Page::Singleplayer {
            self.singleplayer.focus = self.singleplayer.first();
        }
    }

    /// The clocks only: a frame of `dt` while a page of the port's own
    /// covers the screen (its pulses and click limit keep running).
    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        self.on_page += dt;
        self.cues.tick(dt);
        self.pulse.advance(dt);
    }

    /// The page over the screen closed: this screen is entered again, so it
    /// plays its exit click and its entrance (template Enter main.f10 pc 48,
    /// re-bf2-menus §4.1 [V]).
    pub fn uncover(&mut self) {
        self.go(self.page);
        self.cues.play(MenuSound::Exit);
    }

    /// One frame: `dt` seconds, the menu edges and Start's.
    pub fn step(&mut self, input: MenuInput, start: bool, dt: f32) -> Option<ShellCommand> {
        self.tick(dt);
        if self.page == Page::Title {
            if start {
                // `ifs_start` main.f8 pc 23 (re-bf2-menus §4.1 [V]).
                self.cues.play(MenuSound::Accept);
                self.go(Page::Main);
            } else if input.back {
                return Some(ShellCommand::Quit);
            }
            return None;
        }
        if input.back {
            let to = if self.page == Page::Main {
                Page::Title
            } else {
                Page::Main
            };
            // The screen underneath plays its exit click on re-Enter.
            self.cues.play(MenuSound::Exit);
            self.go(to);
            return None;
        }
        let s = self.stack_mut()?;
        // `FollowButtonLink` clicks whenever it finds a row; links wrap.
        let mut moved = false;
        if input.up {
            s.focus = s.follow(-1);
            moved = true;
        }
        if input.down {
            s.focus = s.follow(1);
            moved = true;
        }
        let row = s.rows.get(s.focus).filter(|r| r.enabled).map(|r| r.id);
        if moved && row.is_some() {
            self.cues.play(MenuSound::Select);
        }
        if !input.confirm {
            return None;
        }
        let row = row?;
        self.cues.play(MenuSound::Accept);
        match row {
            RowId::Singleplayer => self.go(Page::Singleplayer),
            RowId::Options => self.go(Page::Options),
            RowId::InstantAction => return Some(ShellCommand::InstantAction),
            RowId::GameOptions => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Game))
            }
            RowId::AudioOptions => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Audio))
            }
            RowId::GraphicsOptions => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Graphics))
            }
            RowId::Controls => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Controls))
            }
            RowId::QuitGame => return Some(ShellCommand::Quit),
            _ => {}
        }
        None
    }

    /// The movie property the page plays (§4).
    pub fn movie(&self) -> &'static str {
        match self.page {
            Page::Title | Page::Main => MOVIE_MAIN,
            Page::Singleplayer | Page::Options => MOVIE_SUB,
        }
    }

    /// The prompt's alpha now: a triangle between [`PROMPT_ALPHA`] at
    /// [`PROMPT_RATE`] a second, starting at the top.
    pub fn prompt_alpha(&self) -> f32 {
        let [lo, hi] = PROMPT_ALPHA;
        let span = hi - lo;
        let t = (self.clock * PROMPT_RATE).rem_euclid(2.0 * span);
        if t < span {
            hi - t
        } else {
            lo + (t - span)
        }
    }
}

/// A stack's boxes in shell units: the title bar and one per row.
#[derive(Debug, Clone, PartialEq)]
pub struct StackLayout {
    pub title: [f32; 4],
    pub rows: Vec<[f32; 4]>,
}

/// Lay a stack out (the [M] rule of the module doc).
pub fn stack_layout(font: &HudFont, stack: &Stack) -> StackLayout {
    let widest = std::iter::once(stack.title.as_str())
        .chain(stack.rows.iter().map(|r| r.label.as_str()))
        .map(|t| run_width(font, t, LABEL_STRETCH, false))
        .fold(0.0, f32::max);
    let w = widest + BAR_PAD;
    let x0 = if stack.left {
        LEFT_X
    } else {
        CENTRE_X - w * 0.5
    };
    let bars = stack.rows.len() + 1;
    let height = bars as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
    let top = CONTAINER_Y + stack.y_add - CENTRE_LIFT - height * 0.5;
    let bar = |k: usize| {
        let y = top + ROW_PITCH * k as f32;
        [x0, y, x0 + w, y + ROW_H]
    };
    StackLayout {
        title: bar(0),
        rows: (1..bars).map(bar).collect(),
    }
}

/// Find an enabled, revealed shell row beneath a window-space pointer.
pub fn row_at(
    font: &HudFont,
    menus: &ShellMenus,
    point: [f32; 2],
    size: [f32; 2],
) -> Option<usize> {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return None;
    }
    let stack = menus.stack()?;
    let box_w = 4.0 / 3.0 * size[1];
    let origin_x = if stack.left {
        0.0
    } else {
        (size[0] - box_w) * 0.5
    };
    let x = (point[0] - origin_x) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    stack
        .rows
        .iter()
        .zip(stack_layout(font, stack).rows)
        .enumerate()
        .find_map(|(i, (row, [x0, y0, x1, y1]))| {
            (row.enabled
                && menus.on_page >= ROW_REVEAL * (i + 1) as f32
                && x >= x0
                && x <= x1
                && y >= y0
                && y <= y1)
                .then_some(i)
        })
}

/// Hit-test the title page's visible Start prompt.
pub fn prompt_at(font: &HudFont, menus: &ShellMenus, point: [f32; 2], size: [f32; 2]) -> bool {
    if size[0] <= 0.0 || size[1] <= 0.0 || menus.page != Page::Title {
        return false;
    }
    let box_w = 4.0 / 3.0 * size[1];
    let x = (point[0] - (size[0] - box_w) * 0.5) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    let width = run_width(font, &menus.prompt, LABEL_STRETCH, false);
    let x0 = 256.0 - width * 0.5;
    x >= x0 - 6.0
        && x <= x0 + width + 6.0
        && y >= PROMPT_TOP - 4.0
        && y <= PROMPT_TOP + f32::from(font.line_height) + 4.0
}

impl Ctx<'_> {
    /// The shell's page for a `size` window: the movie (its frame is the
    /// [`MOVIE_SHEET`] texture; `movie` false draws none), then the logo
    /// or the stack and the help lines.
    pub(crate) fn shell_page(&mut self, m: &ShellMenus, movie: bool, size: [f32; 2]) {
        use crate::spawn_draw::{screen_at, Anchor};
        let centre = screen_at(size, Anchor::Centre);
        let full = [0.0, 0.0, 1.0, 1.0];
        if movie {
            self.sheet(&centre, [0.0, 0.0, 512.0, 448.0], full, MOVIE_SHEET, 1.0);
        }
        if m.covered {
            return;
        }
        if matches!(m.page, Page::Title | Page::Main) {
            self.sheet(&centre, LOGO, full, "logo", 1.0);
        }
        let Some(stack) = m.stack() else {
            let (rgb, _) = UNSELECTED_TEXT;
            let at = (256.0, PROMPT_TOP);
            let style = (rgb, m.prompt_alpha());
            self.label(&centre, BUTTON_FONT, &m.prompt, at, HAlign::Center, style);
            return;
        };
        let Some(font) = self.font(BUTTON_FONT) else {
            return;
        };
        let lay = stack_layout(font, stack);
        let lh = f32::from(font.line_height);
        let s = if stack.left {
            screen_at(size, Anchor::Left)
        } else {
            centre
        };
        let title = FlashyRow {
            rect: lay.title,
            pieces: crate::spawn_draw::TITLE_PIECES,
            bar: ([255.0; 3], crate::spawn_draw::PIECE_ALPHA),
            text: &stack.title,
            style: TITLE_TEXT,
        };
        self.flashy_row(&s, &title, m.on_page, lh);
        let pulse = m.pulse.alpha();
        for (k, (row, r)) in stack.rows.iter().zip(&lay.rows).enumerate() {
            let grey = ([UNFOCUSED_BAR; 3], UNSELECTED_TEXT.1);
            let (bar, style) = if !row.enabled {
                (grey, ([DIMMED_GREY; 3], UNSELECTED_TEXT.1))
            } else if k == stack.focus {
                (([255.0; 3], pulse), (SELECTED_TEXT.0, pulse))
            } else {
                (grey, UNSELECTED_TEXT)
            };
            let row = FlashyRow {
                rect: *r,
                pieces: crate::spawn_draw::item_pieces(k + 1 == stack.rows.len()),
                bar,
                text: &row.label,
                style,
            };
            let t = m.on_page - ROW_REVEAL * (k + 1) as f32;
            self.flashy_row(&s, &row, t, lh);
        }
        self.help(size, false, Prompt::Back, &m.back);
        self.help(size, true, Prompt::Accept, &m.select);
    }

    /// One stack row (or the title) `t` seconds into its flashy entrance
    /// (re-bf2-menus §5 [V]): the bar's middle wipes out, a `cursor` square
    /// sweeps across the text box, the text pops up about its centre line
    /// and fades in.
    pub(crate) fn flashy_row(
        &mut self,
        s: &crate::hud_draw::Screen,
        row: &FlashyRow<'_>,
        t: f32,
        lh: f32,
    ) {
        let f = flashy(t);
        let [x0, y0, x1, y1] = row.rect;
        let Some(out) = f.bar else {
            return;
        };
        let caps = 2.0 * (y1 - y0);
        let right = x0 + caps + (x1 - x0 - caps).max(0.0) * out;
        self.bar_tinted(s, [x0, y0, right, y1], row.pieces, row.bar);
        let (cx, top) = ((x0 + x1) * 0.5, y0 + (ROW_H - lh) * 0.5);
        if let Some(across) = f.square {
            let w = self
                .font(BUTTON_FONT)
                .map_or(0.0, |font| run_width(font, row.text, LABEL_STRETCH, false));
            let x = cx - w * 0.5 + w * across;
            let rect = [x, top, x + lh, top + lh];
            let full = [0.0, 0.0, 1.0, 1.0];
            self.sheet(s, rect, full, CURSOR_SHEET, row.style.1);
        }
        if f.text_h <= 0.0 {
            return;
        }
        let at = (cx, top + lh * (1.0 - f.text_h) * 0.5);
        let style = (row.style.0, row.style.1 * f.text_a);
        self.label_scaled(
            s,
            BUTTON_FONT,
            row.text,
            at,
            HAlign::Center,
            style,
            f.text_h,
        );
    }
}

/// A row as [`Ctx::flashy_row`] draws it.
pub(crate) struct FlashyRow<'a> {
    pub(crate) rect: [f32; 4],
    pub(crate) pieces: [&'static str; 3],
    /// The bar's tint and alpha.
    pub(crate) bar: ([f32; 3], f32),
    pub(crate) text: &'a str,
    pub(crate) style: ([f32; 3], f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_draw_and_pointer_share_menu_metrics_at_all_capture_sizes() {
        use crate::hud_draw::HudArt;
        use crate::hud_tree::Hud;
        use manifest::hud::{HudFont, HudGlyph, HudLayout};
        let legacy = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["legacy".into()],
            glyphs: (32..127)
                .map(|code| HudGlyph {
                    code,
                    page: 0,
                    advance: 4,
                    x0: 0,
                    x1: 4,
                    y0: 3,
                    y1: 18,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut menu = legacy.clone();
        menu.name = "menu_gamefont_large".into();
        menu.pages = vec!["menu".into()];
        for g in &mut menu.glyphs {
            g.advance = 12;
            g.x1 = 12;
        }
        let mut art = HudArt::default();
        art.fonts.insert(legacy.name.clone(), legacy);
        art.fonts.insert(menu.name.clone(), menu);
        let mut menus = ShellMenus::new(&|_, fallback| fallback.into());
        menus.page = Page::Main;
        menus.on_page = 5.0;
        let empty = HudLayout {
            name: "empty".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.shell = Some((menus.clone(), false));
        let layout = stack_layout(art.menu_font(BUTTON_FONT).unwrap(), menus.stack().unwrap());
        for size in [
            [1920.0, 1080.0],
            [2560.0, 1440.0],
            [3840.0, 2160.0],
            [640.0, 480.0],
        ] {
            let frame = crate::hud_draw::draw(&hud, &art, size);
            assert!(frame.batches.iter().any(|b| b.texture == "menu"));
            assert!(!frame.batches.iter().any(|b| b.texture == "legacy"));
            let [x0, y0, x1, y1] = layout.rows[0];
            let screen = crate::spawn_draw::screen_at(size, crate::spawn_draw::Anchor::Centre);
            let point = screen.map(glam::vec3(x1 - 1.0, (y0 + y1) * 0.5, 0.0));
            assert_eq!(
                row_at(art.menu_font(BUTTON_FONT).unwrap(), &menus, point, size),
                Some(0)
            );
            // The edge of this wider row is outside the old, narrow font's bar.
            assert!(x1 > x0);
            assert_eq!(row_at(&art.fonts[BUTTON_FONT], &menus, point, size), None);
        }
        hud.shell = None;
        let ctx = Ctx {
            hud: &hud,
            art: &art,
            out: Default::default(),
            menu_fonts: true,
        };
        assert_eq!(ctx.font(BUTTON_FONT).unwrap().name, "menu_gamefont_large");
    }

    fn menus() -> ShellMenus {
        ShellMenus::new(&|_, e| e.to_owned())
    }

    fn press(f: impl Fn(&mut MenuInput)) -> MenuInput {
        let mut i = MenuInput::default();
        f(&mut i);
        i
    }

    /// Acceptance 2 of card BSHLA: Multiplayer sits in its PS2 slot,
    /// disabled, and focus skips it both ways (§3.3: the PS2 skips its
    /// dimmed Splitscreen the same way).
    #[test]
    fn main_menu_skips_the_disabled_multiplayer() {
        let mut m = menus();
        assert_eq!(m.step(MenuInput::default(), true, 0.1), None);
        assert_eq!(m.page, Page::Main);
        let ids: Vec<_> = m.main.rows.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            [
                RowId::Singleplayer,
                RowId::Multiplayer,
                RowId::Splitscreen,
                RowId::Options,
                RowId::QuitGame
            ]
        );
        let mp = &m.main.rows[1];
        assert!(!mp.enabled, "Multiplayer is present and disabled");
        assert_eq!(m.main.focus, 0);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(
            m.main.rows[m.main.focus].id,
            RowId::Options,
            "Down skips it"
        );
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::Singleplayer, "and Up");
        // Wrapping: Up from Singleplayer lands on Quit Game, Down from Quit
        // Game on Singleplayer.
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::QuitGame);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::Singleplayer);
        // A confirm never lands on a disabled row.
        m.main.focus = 1;
        assert_eq!(m.step(press(|i| i.confirm = true), false, 0.1), None);
        assert_eq!(m.page, Page::Main);
    }

    /// Playtest A5: the main menu's bottom row is Quit Game, by pad (the
    /// wrap) or by mouse, and it quits.
    #[test]
    fn main_menu_quit_game_is_the_last_row_and_quits() {
        use crate::shell_feel::MenuSound as S;
        use manifest::hud::{HudFont, HudGlyph};
        let mut m = menus();
        m.step(MenuInput::default(), true, 0.1);
        m.cues.take();
        m.step(press(|i| i.up = true), false, 0.2);
        let row = &m.main.rows[m.main.focus];
        assert_eq!((row.id, row.label.as_str()), (RowId::QuitGame, "Quit Game"));
        assert_eq!(m.main.focus, m.main.rows.len() - 1, "the bottom row");
        m.cues.take();
        assert_eq!(
            m.step(press(|i| i.confirm = true), false, 0.2),
            Some(ShellCommand::Quit)
        );
        assert_eq!(m.cues.take(), [S::Accept]);
        // The mouse: the row's bar is a hit, and a click on it quits.
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: 10,
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut m = menus();
        m.page = Page::Main;
        m.on_page = 2.0;
        let [x0, y0, x1, y1] = stack_layout(&font, m.stack().unwrap()).rows[4];
        let p = [
            ((x0 + x1) * 0.5) * 1.25,
            ((y0 + y1) * 0.5) * (480.0 / 448.0),
        ];
        assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), Some(4));
        assert!(m.focus_row(4));
        assert_eq!(
            m.step(press(|i| i.confirm = true), false, 0.2),
            Some(ShellCommand::Quit)
        );
    }

    #[test]
    fn mouse_hit_uses_the_visible_row_layout_and_skips_disabled_rows() {
        use manifest::hud::{HudFont, HudGlyph};
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: 10,
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut m = menus();
        m.page = Page::Main;
        m.on_page = 2.0;
        let stack = m.stack().unwrap();
        let layout = stack_layout(&font, stack);
        let [x0, y0, x1, y1] = layout.rows[3];
        let p = [
            ((x0 + x1) * 0.5) * 1.25,
            ((y0 + y1) * 0.5) * (480.0 / 448.0),
        ];
        assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), Some(3));
        let [x0, y0, x1, y1] = layout.rows[1];
        let p = [
            ((x0 + x1) * 0.5) * 1.25,
            ((y0 + y1) * 0.5) * (480.0 / 448.0),
        ];
        assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), None);
    }

    /// `m` as the HUD pass lays it out in a 640 x 480 window, with a font
    /// of 10-unit glyphs.
    fn frame(m: ShellMenus) -> crate::hud_draw::HudFrame {
        use crate::hud_draw::HudArt;
        use crate::hud_tree::Hud;
        use manifest::hud::{HudFont, HudGlyph, HudLayout};
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: 10,
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut art = HudArt::default();
        art.fonts.insert(BUTTON_FONT.into(), font);
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.shell = Some((m, false));
        crate::hud_draw::draw(&hud, &art, [640.0, 480.0])
    }

    /// The main menu as the HUD pass lays it out: the dimmed rows' labels
    /// are drawn grey 110 (§3.3), every glyph of both of them.
    #[test]
    fn the_disabled_rows_are_drawn_dimmed() {
        let mut m = menus();
        m.step(MenuInput::default(), true, 5.0);
        m.step(MenuInput::default(), false, 5.0);
        let f = frame(m);
        let grey = DIMMED_GREY / 255.0;
        let dimmed = f
            .verts
            .iter()
            .filter(|v| v.color[..3].iter().all(|c| (c - grey).abs() < 1e-4))
            .count();
        let glyphs = "MultiplayerSplitscreen".len();
        assert_eq!(dimmed, 4 * glyphs, "Multiplayer and Splitscreen dimmed");
        let sheets: Vec<&str> = f.batches.iter().map(|b| b.texture.as_str()).collect();
        for p in [
            "logo",
            "bf2_buttons_topleft",
            "bf2_buttons_botright",
            "btna",
            "btnb",
        ] {
            assert!(sheets.contains(&p), "{p}: {sheets:?}");
        }
    }

    #[test]
    fn the_flow_reaches_instant_action_and_back() {
        let mut m = menus();
        let confirm = press(|i| i.confirm = true);
        // Only Start leaves the title: Cross does nothing there.
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Title);
        m.step(MenuInput::default(), true, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Singleplayer);
        assert_eq!(m.movie(), MOVIE_SUB);
        // The first focus skips the two disabled campaigns.
        assert_eq!(
            m.singleplayer.rows[m.singleplayer.focus].id,
            RowId::InstantAction
        );
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::InstantAction)
        );
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::Main);
        m.step(press(|i| i.down = true), false, 0.1);
        m.step(confirm, false, 0.1);
        assert_eq!(m.page, Page::Options);
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::Settings(crate::menu::SettingsPage::Game))
        );
        m.step(press(|i| i.back = true), false, 0.1);
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::Title);
        assert_eq!(m.movie(), MOVIE_MAIN);
        assert_eq!(
            m.step(press(|i| i.back = true), false, 0.1),
            Some(ShellCommand::Quit)
        );
    }

    #[test]
    fn controls_are_reachable_from_options() {
        let mut m = menus();
        m.page = Page::Options;
        m.options.focus = m
            .options
            .rows
            .iter()
            .position(|row| row.id == RowId::Controls)
            .unwrap();
        assert!(m.options.rows[m.options.focus].enabled);
        assert_eq!(
            m.step(press(|i| i.confirm = true), false, 0.1),
            Some(ShellCommand::Settings(crate::menu::SettingsPage::Controls))
        );
    }

    #[test]
    fn the_prompt_pulses_between_its_alphas() {
        let mut m = menus();
        let mut seen = (f32::MAX, f32::MIN);
        for _ in 0..100 {
            m.step(MenuInput::default(), false, 0.05);
            let a = m.prompt_alpha();
            seen = (seen.0.min(a), seen.1.max(a));
        }
        assert!(
            (seen.0 - 0.4).abs() < 0.03 && (seen.1 - 1.0).abs() < 0.03,
            "{seen:?}"
        );
    }

    /// re-bf2-menus §4.1: Start on the title accepts; a stack move clicks
    /// even through the wrap; Cross accepts; Back plays the exit click.
    #[test]
    fn the_stacks_ask_for_the_ps2_clicks() {
        use crate::shell_feel::MenuSound as S;
        let mut m = menus();
        m.step(press(|i| i.confirm = true), false, 0.2);
        assert!(m.cues.take().is_empty(), "Cross does nothing on the title");
        m.step(MenuInput::default(), true, 0.2);
        assert_eq!(m.cues.take(), [S::Accept]);
        // Options, Quit Game, Singleplayer (the wrap), Options.
        for _ in 0..4 {
            m.step(press(|i| i.down = true), false, 0.2);
        }
        assert_eq!(m.cues.take(), [S::Select; 4], "every press, the wrap too");
        m.step(press(|i| i.confirm = true), false, 0.2);
        assert_eq!(m.page, Page::Options);
        assert_eq!(m.cues.take(), [S::Accept]);
        m.step(press(|i| i.back = true), false, 0.2);
        assert_eq!(m.cues.take(), [S::Exit]);
        // Presses faster than 0.1 s apart: one click.
        m.step(press(|i| i.down = true), false, 0.02);
        m.step(press(|i| i.down = true), false, 0.02);
        assert_eq!(m.cues.take(), [S::Select]);
        // Back from a page of the port's own: the stack's re-Enter.
        m.on_page = 3.0;
        m.uncover();
        assert_eq!((m.cues.take(), m.on_page), (vec![S::Exit], 0.0));
    }

    /// re-bf2-menus §5: the pulse's phase runs on through a focus move, and
    /// the focused row's label and bar share it; the others sit at 0.6 with
    /// a grey 150 bar. It steps at 30 Hz (menuref §9.4.1).
    #[test]
    fn the_focused_row_pulses_and_keeps_its_phase_across_moves() {
        let mut m = menus();
        m.step(MenuInput::default(), true, 0.0);
        m.step(MenuInput::default(), false, 0.15);
        let before = m.pulse.alpha();
        // 0.15 s = 4 whole updates of 2/30 from 0.5.
        assert!((before - (0.5 + 4.0 * 2.0 / 30.0)).abs() < 1e-4, "{before}");
        m.step(press(|i| i.down = true), false, 0.05);
        assert!(
            (m.pulse.alpha() - 0.9).abs() < 1e-4,
            "not reset by the move"
        );
        m.on_page = 5.0;
        let a = m.pulse.alpha();
        let f = frame(m);
        let [r, g, b] = SELECTED_TEXT.0.map(|c| c / 255.0);
        let focused = |v: &&crate::hud_draw::HudVertex| {
            (v.color[0] - r).abs() < 1e-4
                && (v.color[1] - g).abs() < 1e-4
                && (v.color[2] - b).abs() < 1e-4
        };
        let label: Vec<_> = f.verts.iter().filter(focused).collect();
        assert_eq!(label.len(), 4 * "Options".len());
        assert!(label.iter().all(|v| (v.color[3] - a).abs() < 1e-4));
        let white_bar = f
            .verts
            .iter()
            .filter(|v| v.color == [1.0, 1.0, 1.0, a])
            .count();
        assert_eq!(white_bar, 3 * 4, "the focused bar's three pieces");
        let grey = UNFOCUSED_BAR / 255.0;
        let grey_bars = f
            .verts
            .iter()
            .filter(|v| v.color == [grey, grey, grey, 0.6])
            .count();
        assert_eq!(grey_bars, 4 * 3 * 4, "the other four rows' bars");
    }

    /// re-bf2-menus §5, menuref §9.4.5: on entering a page nothing of row 1
    /// shows until its 0.1 s delay, then its bar wipes out before its text
    /// appears.
    #[test]
    fn a_page_entry_replays_the_flashy_entrance() {
        let mut m = menus();
        m.step(MenuInput::default(), true, 0.0);
        let bar_right = |m: &ShellMenus| {
            let f = frame(m.clone());
            f.verts
                .iter()
                .filter(|v| v.color[..3] == [1.0, 1.0, 1.0] && v.color[3] == m.pulse.alpha())
                .map(|v| v.pos[0])
                .fold(f32::MIN, f32::max)
        };
        m.on_page = 0.05;
        assert_eq!(bar_right(&m), f32::MIN, "row 1 not started");
        m.on_page = 0.1 + 0.05;
        let half = bar_right(&m);
        m.on_page = 0.1 + 0.1;
        let full = bar_right(&m);
        assert!(half > f32::MIN && half < full, "{half} {full}");
        let text = |m: &ShellMenus| {
            let [r, g, b] = SELECTED_TEXT.0.map(|c| c / 255.0);
            frame(m.clone())
                .verts
                .iter()
                .filter(|v| v.color[..3] == [r, g, b])
                .count()
        };
        m.on_page = 0.1 + 0.15;
        assert_eq!(text(&m), 0, "the square sweeps before the text");
        m.on_page = 0.1 + 0.3;
        assert_eq!(text(&m), 4 * "Singleplayer".len());
    }

    /// menuref "Singleplayer": coming back to Singleplayer (from Instant
    /// Action, or from the Main Menu) focuses its first row, not the one
    /// left.
    #[test]
    fn singleplayer_is_always_entered_on_its_first_row() {
        let mut m = menus();
        // As if Training were shown: the first row live.
        m.singleplayer.rows[0].enabled = true;
        m.step(MenuInput::default(), true, 0.1);
        m.step(press(|i| i.confirm = true), false, 0.1);
        assert_eq!(m.page, Page::Singleplayer);
        assert_eq!(m.singleplayer.focus, 0);
        m.singleplayer.focus = 2;
        // Instant Action opens over it and closes: back on the first row.
        m.uncover();
        assert_eq!(m.singleplayer.focus, 0);
        m.singleplayer.focus = 2;
        m.step(press(|i| i.back = true), false, 0.1);
        m.step(press(|i| i.confirm = true), false, 0.1);
        assert_eq!(m.page, Page::Singleplayer);
        assert_eq!(m.singleplayer.focus, 0);
    }
}
