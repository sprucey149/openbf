//! `[ours]` The Players screen (issue #194, `analysis/plan/SPLITSCREEN.md` sections 3 and 18.1
//! item 1): who plays on this machine, entered from the main menu's Splitscreen row. The window
//! is cut into the 2x2 grid from the start, one quarter per seat (P1 top left, P2 top right, P3
//! bottom left, P4 bottom right, the battle's own four-view order), each a whole shell screen of
//! its own fitted to its quarter: nothing moves as players join (issue #194 B22). Each region
//! holds its seat's join prompt, profile picker or Ready card, as the PS2's split profile screen
//! has a picker per player. The "P1".."P4" tags name the quarter's seat position, the number the
//! Splitscreen options fall back to; the profile's name is on the card under it.
//!
//! The ui crate draws what the game tells it ([`PlayersView`]); the seats and their pickers
//! are the game's (`game::local_seats::Party`), which hears every pad on its own.

use super::profiles::{self, Picker, Target};
use super::ShellMenus;
use crate::hud_draw::{Ctx, Screen};
use crate::input_icons::{pad_sheet, IconSet, Icons, PadButton, Prompt};
use crate::spawn_draw::{item_pieces, TITLE_PIECES};
use crate::typography::{tint, BODY, ITEM, LIST, TITLE};

/// One seat's region.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum PlayerCell {
    /// Nobody here yet: the join prompt.
    #[default]
    Open,
    /// Seated and choosing a profile: the seat's own picker.
    Picking {
        picker: Picker,
        /// What they hold: "Keyboard and mouse", "Xbox controller", ...
        device: String,
        /// The prompts' set: the seat's own device.
        icons: IconSet,
        /// Their pad was unplugged: the seat and its picker wait for it.
        lost: bool,
    },
    /// Seated on a profile: the Ready card.
    Seated {
        /// The profile's name.
        name: String,
        device: String,
        icons: IconSet,
        /// The profile's career, one line.
        career: String,
        lost: bool,
        /// B opens the seat's picker again (its help says "Change profile", not "Back").
        rechoose: bool,
    },
}

/// What the Players screen shows, filled by the game each frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayersView {
    pub cells: [PlayerCell; 4],
    /// Nobody plays on the keyboard and mouse: the open regions say Enter joins.
    pub keyboard_free: bool,
    /// The buttons the open regions' join line shows (issue #194 B22): the set of every
    /// connected pad nobody holds when they are all one family; `None` (mixed, or none) names
    /// the buttons in words ([`TEXT_JOIN`]).
    pub join_icons: Option<IconSet>,
    /// The party refuses Continue though everyone seated is Ready (two seats on one profile,
    /// `game::local_seats::Party::can_start`): the Continue bars and their help stay dim.
    pub blocked: bool,
    /// Seat `i` is Ready on a profile another seat is Ready on too: its card says so
    /// ([`TEXT_TWICE`]) in place of "Ready".
    pub twice: [bool; 4],
    /// Seat `i` is lost and a held Select (or Tab) empties it: its notice says how.
    pub removable: [bool; 4],
    /// The Select glyph the removable notices show: the seated pads' set when they are all one
    /// family; `None` names the buttons in words ([`TEXT_REMOVE_WORDS`]).
    pub remove_icons: Option<IconSet>,
}

impl PlayersView {
    /// Seats with somebody on them, choosing or Ready.
    pub fn seated(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| !matches!(c, PlayerCell::Open))
            .count()
    }

    /// Somebody still chooses a profile.
    pub fn picking(&self) -> bool {
        self.cells
            .iter()
            .any(|c| matches!(c, PlayerCell::Picking { .. }))
    }

    /// Two or more seated, every one of them Ready, and the party does not refuse: Continue is
    /// live.
    pub fn can_start(&self) -> bool {
        self.seated() >= 2 && !self.picking() && !self.blocked
    }
}

/// The screen's texts: (key, English). Our own keys.
pub const TEXT_TITLE: (&str, &str) = ("ifs.main.split", "Splitscreen");
/// The open region's join line without glyphs: the two buttons that seat a pad, by name.
pub const TEXT_JOIN: (&str, &str) = ("openbf.split.join", "Press Cross/A or Start to join");
pub const TEXT_READY: (&str, &str) = ("openbf.split.ready", "Ready");
pub const TEXT_LOST: (&str, &str) = ("openbf.split.lost", "Controller disconnected");
/// A lost seat's way back without glyphs (an unseated pad's South or Start reclaims it).
pub const TEXT_RECLAIM: (&str, &str) = (
    "openbf.split.reclaim",
    "Press Cross/A or Start to come back",
);
/// After the glyphs of a lost seat's pad: what they do.
pub const TEXT_COME_BACK: (&str, &str) = ("openbf.split.come_back", "Come back");
pub const TEXT_KEYBOARD: (&str, &str) = ("openbf.split.keyboard", "Keyboard and mouse");
/// The small line under an open region's join prompt: the keyboard joins as a pad does.
pub const TEXT_JOIN_KEYBOARD: (&str, &str) =
    ("openbf.split.join_keyboard", "Enter: keyboard and mouse");
/// Under it, the free keyboard's way out: Esc backs the whole screen out, every seat emptied.
pub const TEXT_LEAVE_KEYBOARD: (&str, &str) = ("openbf.split.leave_keyboard", "Esc: main menu");
pub const TEXT_CONTINUE: (&str, &str) = ("openbf.split.continue", "Continue");
pub const TEXT_JOIN_HELP: (&str, &str) = ("openbf.split.join_help", "Join");
/// The Ready card's West prompt: the seat's picker again.
pub const TEXT_CHANGE: (&str, &str) = ("openbf.split.change", "Change profile");
/// A Ready card whose profile another seat is Ready on too: one of them changes profile.
pub const TEXT_TWICE: (&str, &str) = ("openbf.split.twice", "Same profile twice");
/// Under [`TEXT_TWICE`]: what unblocks Continue.
pub const TEXT_TWICE_HELP: (&str, &str) = ("openbf.split.twice_help", "One player changes profile");
/// After the Select glyph on a lost seat's notice: hold it on any seated pad to empty the seat.
pub const TEXT_REMOVE: (&str, &str) = ("openbf.split.remove", "Hold to remove");
/// The same without glyphs.
pub const TEXT_REMOVE_WORDS: (&str, &str) =
    ("openbf.split.remove_words", "Hold Select or Tab to remove");
/// Under a Ready card while another seat still chooses.
pub const TEXT_WAITING: (&str, &str) = ("openbf.split.waiting", "Waiting for every player");

/// A split battle cell's lost-controller notice (issue #194; drawn by
/// [`crate::hud_draw::draw_lost_cell`]): what happened, whose cell it is, and the line the
/// player acts on, after `glyph` (a button sheet) when it names one.
#[derive(Debug, Clone, PartialEq)]
pub struct LostCell {
    pub notice: String,
    pub name: String,
    pub action: String,
    pub glyph: Option<&'static str>,
}

/// The lost cell's fill: near black, as the away cell's world clear.
const LOST_FILL: [f32; 4] = [10.0 / 255.0, 10.0 / 255.0, 14.0 / 255.0, 1.0];

/// The texts, looked up once.
#[derive(Debug, Clone, PartialEq)]
pub struct Texts {
    pub title: String,
    pub join: String,
    pub ready: String,
    pub lost: String,
    pub reclaim: String,
    pub keyboard: String,
    pub join_keyboard: String,
    pub leave_keyboard: String,
    pub continue_: String,
    pub join_help: String,
    pub change: String,
    pub waiting: String,
    pub come_back: String,
    pub twice: String,
    pub twice_help: String,
    pub remove: String,
    pub remove_words: String,
}

impl Texts {
    pub fn new(text: &dyn Fn(&str, &str) -> String) -> Texts {
        let t = |(k, e): (&str, &str)| text(k, e);
        Texts {
            title: t(TEXT_TITLE),
            join: t(TEXT_JOIN),
            ready: t(TEXT_READY),
            lost: t(TEXT_LOST),
            reclaim: t(TEXT_RECLAIM),
            keyboard: t(TEXT_KEYBOARD),
            join_keyboard: t(TEXT_JOIN_KEYBOARD),
            leave_keyboard: t(TEXT_LEAVE_KEYBOARD),
            continue_: t(TEXT_CONTINUE),
            join_help: t(TEXT_JOIN_HELP),
            change: t(TEXT_CHANGE),
            waiting: t(TEXT_WAITING),
            come_back: t(TEXT_COME_BACK),
            twice: t(TEXT_TWICE),
            twice_help: t(TEXT_TWICE_HELP),
            remove: t(TEXT_REMOVE),
            remove_words: t(TEXT_REMOVE_WORDS),
        }
    }
}

/// A region's seat tag ("P1"), in the region's own shell units (512 x 448, centred).
pub const TAG_BAR: [f32; 4] = [104.0, 36.0, 408.0, 36.0 + crate::spawn_draw::ROW_H];
/// The Ready card and the open region's box.
pub const CARD: [f32; 4] = [96.0, 92.0, 416.0, 316.0];
/// The Continue button on a Ready card.
pub const CONTINUE_BAR: [f32; 4] = [96.0, 336.0, 416.0, 336.0 + crate::spawn_draw::ROW_H];

/// Each seat's region in the window, `[x0, y0, x1, y1]` pixels, by seat number: always the
/// 2x2 grid (issue #194 B22), so no region moves or resizes as seats fill. At 16:9 a quarter's
/// 4:3 shell screen is the same size a half's was (both are the region's height high).
pub fn regions(size: [f32; 2]) -> Vec<(usize, [f32; 4])> {
    let [w, h] = size;
    let (hw, hh) = (w * 0.5, h * 0.5);
    (0..4)
        .map(|i| {
            let (x, y) = ((i % 2) as f32 * hw, (i / 2) as f32 * hh);
            (i, [x, y, x + hw, y + hh])
        })
        .collect()
}

/// What a click on the Players screen lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayersHit {
    Continue,
    /// A target of seat `.0`'s picker (the game lets only the keyboard seat's picker hear it).
    Picker(usize, Target),
}

/// Hit-test a window-space pointer on the Players screen.
pub fn hit(menus: &ShellMenus, point: [f32; 2], size: [f32; 2]) -> Option<PlayersHit> {
    if size[0] <= 0.0 || size[1] <= 0.0 || menus.page != super::Page::Players || menus.covered {
        return None;
    }
    let view = &menus.players;
    let inside =
        |r: [f32; 4]| point[0] >= r[0] && point[0] < r[2] && point[1] >= r[1] && point[1] < r[3];
    let (seat, rect) = regions(size).into_iter().find(|(_, r)| inside(*r))?;
    match &view.cells[seat] {
        PlayerCell::Picking { picker, .. } => picker
            .target_in(point, rect)
            .map(|t| PlayersHit::Picker(seat, t)),
        PlayerCell::Seated { .. } => {
            let s = profiles::screen_in(rect);
            let x = (point[0] - s.origin[0]) / s.scale[0];
            let y = (point[1] - s.origin[1]) / s.scale[1];
            let [x0, y0, x1, y1] = CONTINUE_BAR;
            (view.can_start() && x >= x0 && x <= x1 && y >= y0 && y <= y1)
                .then_some(PlayersHit::Continue)
        }
        PlayerCell::Open => None,
    }
}

impl Ctx<'_> {
    /// The Players screen over the shell movie: every seat's region.
    pub(crate) fn players_page(&mut self, m: &ShellMenus, size: [f32; 2]) {
        for (seat, rect) in regions(size) {
            self.player_region(m, seat, rect);
        }
    }

    fn player_region(&mut self, m: &ShellMenus, seat: usize, rect: [f32; 4]) {
        let t = &m.players_texts;
        let view = &m.players;
        let s = profiles::screen_in(rect);
        let tag = format!("P{}", seat + 1);
        // A lost seat that a held Select empties: the glyphs (or words) its notice shows.
        let remove = |i: usize| view.removable[i].then_some(view.remove_icons);
        match &view.cells[seat] {
            PlayerCell::Open => {
                self.bordered(&s, CARD);
                self.text_in(&s, &TITLE, &tag, line(CARD, 14.0, TITLE.size), tint::DIMMED);
                let pulse = m.prompt_alpha();
                let (rgb, _) = tint::UNSELECTED;
                let at = line(CARD, 100.0, BODY.size);
                match view.join_icons {
                    // The waiting pads' own Accept and Start, then "Join", on the same line.
                    Some(set) => self.glyph_line(
                        &s,
                        &BODY,
                        &[
                            Icons::of(set).sheet(Prompt::Accept),
                            pad_sheet(set, PadButton::Start),
                        ],
                        &t.join_help,
                        at,
                        (rgb, pulse),
                    ),
                    None => self.text_in(&s, &BODY, &t.join, at, (rgb, pulse)),
                }
                if view.keyboard_free {
                    let below = line(CARD, 100.0 + BODY.size + 8.0, LIST.size);
                    self.text_in(&s, &LIST, &t.join_keyboard, below, tint::NOTE);
                    let under = line(CARD, 100.0 + BODY.size + 16.0 + LIST.size, LIST.size);
                    self.text_in(&s, &LIST, &t.leave_keyboard, under, tint::NOTE);
                }
            }
            PlayerCell::Picking {
                picker,
                icons,
                lost,
                ..
            } => {
                profiles::draw_in(self, picker, rect, Icons::of(*icons));
                self.text_in(
                    &s,
                    &TITLE,
                    &tag,
                    [24.0, 36.0, 96.0, 36.0 + TITLE.size],
                    tint::TITLE,
                );
                if *lost {
                    self.lost_notice(&s, t, *icons, remove(seat));
                }
            }
            PlayerCell::Seated {
                name,
                device,
                icons,
                career,
                lost,
                rechoose,
            } => {
                let icons = Icons::of(*icons);
                self.bar(&s, TAG_BAR, TITLE_PIECES);
                self.text_in(&s, &TITLE, &tag, TAG_BAR, tint::TITLE);
                self.bordered(&s, CARD);
                self.text_in(&s, &TITLE, name, line(CARD, 24.0, TITLE.size), tint::BODY);
                self.text_in(
                    &s,
                    &LIST,
                    device,
                    line(CARD, 70.0, LIST.size),
                    tint::UNSELECTED,
                );
                self.text_in(&s, &LIST, career, line(CARD, 94.0, LIST.size), tint::NOTE);
                if *lost {
                    self.lost_notice(&s, t, icons.set, remove(seat));
                } else if view.twice[seat] {
                    self.text_in(
                        &s,
                        &BODY,
                        &t.twice,
                        line(CARD, 150.0, BODY.size),
                        tint::WARNING,
                    );
                    let at = line(CARD, 150.0 + BODY.size + 8.0, LIST.size);
                    self.text_in(&s, &LIST, &t.twice_help, at, tint::NOTE);
                } else {
                    self.text_in(
                        &s,
                        &BODY,
                        &t.ready,
                        line(CARD, 150.0, BODY.size),
                        tint::SELECTED,
                    );
                    if view.picking() {
                        let at = line(CARD, 150.0 + BODY.size + 8.0, LIST.size);
                        self.text_in(&s, &LIST, &t.waiting, at, tint::NOTE);
                    }
                }
                let live = view.can_start();
                let alpha = m.pulse.alpha();
                let (bar, style) = if live {
                    (([255.0; 3], alpha), (tint::SELECTED.0, alpha))
                } else {
                    (
                        ([super::UNFOCUSED_BAR; 3], tint::UNSELECTED.1),
                        tint::DIMMED,
                    )
                };
                self.bar_tinted(&s, CONTINUE_BAR, item_pieces(true), bar);
                self.text_in(&s, &ITEM, &t.continue_, CONTINUE_BAR, style);
                let back = if *rechoose { &t.change } else { &m.back };
                self.help_icon_on(&s, false, profiles::seat_back_icon(icons), back);
                if live {
                    self.help_icon_on(&s, true, icons.sheet(Prompt::Accept), &t.continue_);
                } else if icons.set != IconSet::KeyboardMouse && !*rechoose {
                    let west = pad_sheet(icons.set, PadButton::West);
                    self.help_icon_on(&s, true, west, &t.change);
                }
            }
        }
    }

    /// A seat whose pad is gone: the notice over its region; nothing else there moves.
    /// The seat's pad was `set` (issue #194 B22): its Accept and Start glyphs say how to come
    /// back; the keyboard's set (no pad to come back with) says it in words. `remove`: the seat
    /// can be emptied by a held Select on a seated pad (or Tab), shown with that set's Select
    /// glyph (`Some(Some(set))`) or in words (`Some(None)`), under "Come back".
    fn lost_notice(
        &mut self,
        s: &Screen,
        t: &Texts,
        set: IconSet,
        remove: Option<Option<IconSet>>,
    ) {
        // Inside the card, over its Ready line (the name and device stay readable).
        let bar = [112.0, 214.0, 400.0, 308.0];
        self.bar_tinted(s, bar, item_pieces(false), ([0.0; 3], 0.85));
        self.text_in(s, &BODY, &t.lost, line(bar, 12.0, BODY.size), tint::WARNING);
        let below = line(bar, 66.0, LIST.size);
        match remove {
            Some(Some(by)) => self.glyph_line(
                s,
                &LIST,
                &[pad_sheet(by, PadButton::Select)],
                &t.remove,
                below,
                tint::NOTE,
            ),
            Some(None) => self.text_in(s, &LIST, &t.remove_words, below, tint::NOTE),
            None => {}
        }
        let at = line(bar, 40.0, LIST.size);
        match set {
            IconSet::KeyboardMouse => self.text_in(s, &LIST, &t.reclaim, at, tint::NOTE),
            set => self.glyph_line(
                s,
                &LIST,
                &[
                    Icons::of(set).sheet(Prompt::Accept),
                    pad_sheet(set, PadButton::Start),
                ],
                &t.come_back,
                at,
                tint::NOTE,
            ),
        }
    }

    /// A split battle cell whose player's controller is gone (issue #194): the cell filled dark
    /// and `notice`'s lines in its middle, laid out in the cell's own shell screen (`size`, the
    /// cell's pixels, its top-left at 0, 0) as the Players screen's lost notice is. The line the
    /// player acts on is the largest and brightest, with its button's glyph when it names one.
    pub(crate) fn lost_cell(&mut self, notice: &LostCell, size: [f32; 2]) {
        self.fill(
            usize::MAX,
            &Screen {
                origin: [0.0, 0.0],
                scale: [1.0, 1.0],
            },
            [0.0, 0.0, size[0], size[1]],
            LOST_FILL,
        );
        let s = profiles::screen_in([0.0, 0.0, size[0], size[1]]);
        let across = [24.0, 0.0, 488.0, 0.0];
        let gap = 10.0;
        let total = BODY.size + TITLE.size + ITEM.size + 2.0 * gap;
        let top = (crate::hud_tree::FRAME_H - total) * 0.5;
        self.text_in(
            &s,
            &BODY,
            &notice.notice,
            line(across, top, BODY.size),
            tint::WARNING,
        );
        let name_at = top + BODY.size + gap;
        self.text_in(
            &s,
            &TITLE,
            &notice.name,
            line(across, name_at, TITLE.size),
            tint::BODY,
        );
        let at = line(across, name_at + TITLE.size + gap, ITEM.size);
        match notice.glyph {
            Some(sheet) => self.glyph_line(&s, &ITEM, &[sheet], &notice.action, at, tint::BODY),
            None => self.text_in(&s, &ITEM, &notice.action, at, tint::BODY),
        }
    }

    /// `sheets` (button glyphs, sized to the role's text) then `text` in `style`, centred
    /// together on the line `rect`, so the line's place is the same as a plain text line's there.
    fn glyph_line(
        &mut self,
        s: &Screen,
        style: &crate::typography::TextStyle,
        sheets: &[&str],
        text: &str,
        [x0, y0, x1, y1]: [f32; 4],
        tint: crate::typography::Tint,
    ) {
        use crate::spawn_draw::{ICON, ICON_GAP};
        // The glyph as tall as the text, in the help line's icon proportions.
        let k = (style.size / ICON[1]).max(1.0);
        let [iw, ih] = ICON.map(|v| v * k);
        let gap = ICON_GAP * 0.5;
        let width = sheets.len() as f32 * (iw + gap) + self.text_width(style, text);
        let mut x = (x0 + x1 - width) * 0.5;
        let mid = (y0 + y1) * 0.5;
        for sheet in sheets {
            let top = mid - ih * 0.5;
            self.sheet(
                s,
                [x, top, x + iw, top + ih],
                [0.0, 0.0, 1.0, 1.0],
                sheet,
                tint.1,
            );
            x += iw + gap;
        }
        let at = (x, mid - style.size * 0.5);
        self.text(s, style, text, at, crate::hud_tree::HAlign::Left, tint);
    }
}

/// A line `h` tall, `k` below the top of `rect`, across its width.
fn line([x0, y0, x1, _]: [f32; 4], k: f32, h: f32) -> [f32; 4] {
    [x0, y0 + k, x1, y0 + k + h]
}
