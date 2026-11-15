//! `[ours]` How menu text looks, defined once.
//!
//! Every menu, overlay and screen takes its text style from the roles below:
//! the face, the size, the stretch across, the letter spacing, the case
//! rule, the line pitch and the colours. No other module names a font, a
//! size or a stretch for menu text, and no menu shrinks text to fit: a
//! layout makes room for its text, and the rare string that still does not
//! fit ends in an ellipsis at the same size ([`ellipsize`]). A guard test
//! (`typography::tests::menu_modules_take_text_styles_from_typography`)
//! fails when a menu module hard-codes one.
//!
//! The one face is the shell's "BF2 Light" (`menu_gamefont_large / small /
//! tiny`, three metric sets drawn from the same distance pages); a bake
//! without it falls back to the disc's `gamefont_*`, which share its
//! advances ([`crate::hud_draw::HudArt::menu_font`]). The native pass draws
//! it from the distance pages; egui draws the same outlines, traced from the
//! same pages into a font at load time ([`face`]), at the same size for the
//! window's height ([`egui_font`]).
//!
//! The roles keep the PS2 shell's look (`analysis/re-bf2-shell.md` §3.1,
//! §3.2): the stack's title and rows are `gamefont_large` 1.087 × its
//! advances across, the help lines `gamefont_small` 1.11 ×, the dense lists
//! `gamefont_tiny` 1.10 × (its lower case is drawn as capitals). The BF2 HUD
//! tree's own texts are the HUD, not menus, and keep the layouts' fonts.

pub mod face;

use crate::hud_draw::{color, Run, Screen};
use crate::hud_tree::HAlign;
use manifest::hud::HudFont;
use std::borrow::Cow;

/// One of the menu face's three metric sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Large,
    Small,
    Tiny,
}

impl Face {
    pub const ALL: [Face; 3] = [Face::Large, Face::Small, Face::Tiny];

    /// The name the art is looked up by ([`crate::hud_draw::HudArt::menu_font`]
    /// resolves it to the menu face when the bake has one).
    pub const fn role(self) -> &'static str {
        match self {
            Face::Large => "gamefont_large",
            Face::Small => "gamefont_small",
            Face::Tiny => "gamefont_tiny",
        }
    }

    /// The egui font family the same metric set is installed under.
    pub const fn family(self) -> &'static str {
        match self {
            Face::Large => "menu_large",
            Face::Small => "menu_small",
            Face::Tiny => "menu_tiny",
        }
    }
}

/// How a role writes its text's letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// As the text is written: the disc's texts and ours are Title Case
    /// (`tests::our_labels_are_title_case` checks ours).
    AsWritten,
    /// All capitals (the tiny metric set draws lower case as capitals; the
    /// role says so instead of leaving it to the glyphs).
    Upper,
}

impl Case {
    pub fn apply(self, text: &str) -> Cow<'_, str> {
        match self {
            Case::AsWritten => Cow::Borrowed(text),
            Case::Upper if text.chars().any(char::is_lowercase) => Cow::Owned(text.to_uppercase()),
            Case::Upper => Cow::Borrowed(text),
        }
    }
}

/// A text role. Sizes are shell units (the 512 × 448 frame; a unit is
/// 1/448 of the window's height down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub face: Face,
    /// The line's height: the glyphs are scaled so the metric set's line
    /// height is this many units.
    pub size: f32,
    /// Across, × the advances (on top of the frame's own 4:3 shape).
    pub stretch: f32,
    /// Units added after every glyph.
    pub tracking: f32,
    pub case: Case,
    /// From one line's top to the next's, in a paragraph or a list.
    pub line: f32,
}

impl TextStyle {
    /// Glyph scale across and down for `font` (its own line height).
    pub fn scale(&self, font: &HudFont) -> [f32; 2] {
        let ky = self.size / f32::from(font.line_height.max(1));
        [self.stretch * ky, ky]
    }
}

/// The large metric set as the shell's stacks draw it (§3.2 [M]).
const LARGE: TextStyle = TextStyle {
    face: Face::Large,
    size: 22.0,
    stretch: 1.087,
    tracking: 0.0,
    case: Case::AsWritten,
    line: 22.0,
};
/// The small metric set: the help lines' and the class panel's ink
/// (`spawn_draw`'s measurement, 1.0-1.04 × a 1.087 run).
const SMALL: TextStyle = TextStyle {
    face: Face::Small,
    size: 18.0,
    stretch: 1.11,
    tracking: 0.0,
    case: Case::AsWritten,
    line: 18.0,
};
/// The tiny metric set (Hoth's spawn map: 1.38-1.41 display px per texel).
const TINY: TextStyle = TextStyle {
    face: Face::Tiny,
    size: 15.0,
    stretch: 1.10,
    tracking: 0.0,
    case: Case::Upper,
    line: 15.0,
};

/// A page's title bar.
pub const TITLE: TextStyle = LARGE;
/// A row of a stack or a settings page: its name, its value and its `<` `>`.
pub const ITEM: TextStyle = LARGE;
/// The help lines at the bottom ("Back", "Select").
pub const HELP: TextStyle = SMALL;
/// Paragraphs: a popup's question, a row's help, the class panel, an
/// option's help box.
pub const BODY: TextStyle = SMALL;
/// Dense lists: Instant Action's maps, modes, eras and playlist, its
/// commands and info line, the spawn map's legend and objective, the stats
/// pages' rows.
pub const LIST: TextStyle = TINY;

/// `[ours]` Small readouts over the battle that must not cover it: the FPS
/// counter and the LAN net meter. The small metric set at a third of the
/// help lines' size (about 13 px at 1080p).
/// measure: the net meter's widest line against the battle's width at
/// 1280x720 in a LAN playtest.
pub const READOUT: TextStyle = TextStyle {
    size: 6.0,
    line: 7.0,
    ..SMALL
};

/// Every role, for the tests and the egui fonts.
pub const ROLES: [(&str, TextStyle); 6] = [
    ("TITLE", TITLE),
    ("ITEM", ITEM),
    ("HELP", HELP),
    ("BODY", BODY),
    ("LIST", LIST),
    ("READOUT", READOUT),
];

/// A colour role: display RGB 0..255 and alpha.
pub type Tint = ([f32; 3], f32);

/// The palette (`re-bf2-shell.md` §3.1 [V]; the stats pages' colours are
/// the glyphs' brightest pixels on the PS2 frames).
pub mod tint {
    use super::Tint;
    /// gTitleTextColor / alpha.
    pub const TITLE: Tint = ([246.0, 235.0, 20.0], 0.8);
    /// gSelectedTextColor / alpha.
    pub const SELECTED: Tint = ([138.0, 160.0, 244.0], 0.6);
    /// gUnselectedTextColor / alpha.
    pub const UNSELECTED: Tint = ([240.0, 240.0, 240.0], 0.6);
    /// gHelptextTextColor / alpha.
    pub const HELP: Tint = ([240.0, 240.0, 240.0], 0.8);
    /// A dimmed row (`ShowHideVerticalButtons` f7: grey 110).
    pub const DIMMED: Tint = ([DIMMED_GREY; 3], 0.6);
    /// A stack's note under its rows.
    pub const NOTE: Tint = ([DIMMED_GREY + 60.0; 3], 0.6);
    /// Plain white body text (the class panel, an option's help box).
    pub const BODY: Tint = ([255.0; 3], 1.0);
    /// The spawn map's objective (peak (222, 222, 74) on the frames).
    pub const OBJECTIVE: Tint = ([222.0, 222.0, 74.0], 1.0);
    /// Stats pages: the title at full alpha, grey names and numbers,
    /// yellow points, the player's row white, the cursor row blue, the
    /// award winners yellow.
    pub const STATS_TITLE: Tint = ([246.0, 235.0, 20.0], 1.0);
    pub const STATS_GREY: Tint = ([126.0; 3], 1.0);
    pub const STATS_YELLOW: Tint = ([253.0, 253.0, 0.0], 1.0);
    pub const STATS_WHITE: Tint = ([250.0; 3], 1.0);
    pub const STATS_BLUE: Tint = ([129.0, 150.0, 229.0], 1.0);
    pub const STATS_WINNER: Tint = ([250.0, 250.0, 0.0], 1.0);
    /// A warning line (an egui overlay's error).
    pub const WARNING: Tint = ([255.0, 140.0, 90.0], 1.0);
    /// The scoreboard's rows: the player's own, another human's, a bot's.
    pub const ROW_ME: Tint = ([255.0, 230.0, 120.0], 1.0);
    pub const ROW_HUMAN: Tint = ([255.0; 3], 1.0);
    pub const ROW_BOT: Tint = ([185.0; 3], 1.0);
    /// A loading screen's caption (light blue).
    pub const CAPTION: Tint = ([120.0, 190.0, 255.0], 1.0);
    /// The two teams' accents (the HUD's team colours,
    /// [`crate::hud::TEAM_COLORS`] is built from these).
    pub const TEAMS: [Tint; 2] = [([242.0, 100.0, 40.0], 1.0), ([80.0, 220.0, 90.0], 1.0)];
    /// The grey of a dimmed label.
    pub const DIMMED_GREY: f32 = 110.0;
}

/// A stack row's bar height and the pitch between rows (the large set's 22
/// + gButtonHeightPad 6, + gButtonGutter 4: `re-bf2-shell.md` §3.2).
pub const ROW_H: f32 = 28.0;
pub const ROW_PITCH: f32 = 32.0;
/// The room kept between a column's edge and its text.
pub const TEXT_INSET: f32 = 5.0;
/// `[ours]` The menu face has one weight: a bold run (a human's name on
/// Team Stats, issue #168) is the same glyphs drawn again this many units
/// right, which thickens every stem by the same share at any size.
pub const BOLD_DX: f32 = 0.8;
/// What a string too long for its room ends with.
pub const ELLIPSIS: &str = "...";

fn glyph(font: &HudFont, c: char) -> Option<&manifest::hud::HudGlyph> {
    font.glyphs.iter().find(|g| u32::from(g.code) == c as u32)
}

/// How wide `text` is laid out in `style` (shell units), its case applied.
pub fn width(font: &HudFont, style: &TextStyle, text: &str) -> f32 {
    let text = style.case.apply(text);
    let [kx, _] = style.scale(font);
    text.chars()
        .filter_map(|c| glyph(font, c))
        .map(|g| f32::from(g.advance) * kx + style.tracking)
        .sum()
}

/// `text` as it fits in `room` units at the role's own size: whole, or cut
/// at a character and ended with [`ELLIPSIS`].
pub fn ellipsize<'a>(font: &HudFont, style: &TextStyle, text: &'a str, room: f32) -> Cow<'a, str> {
    // A hair of slack: a column sized to its widest text must take it whole.
    if width(font, style, text) <= room + 0.01 {
        return Cow::Borrowed(text);
    }
    let dots = width(font, style, ELLIPSIS);
    let mut cut = text.trim_end().to_owned();
    while !cut.is_empty() && width(font, style, &cut) + dots > room {
        cut.pop();
        while cut.ends_with(' ') {
            cut.pop();
        }
    }
    Cow::Owned(format!("{cut}{ELLIPSIS}"))
}

/// `text` broken into lines no wider than `room`: its own line ends first,
/// then between words. A word wider than the room keeps its own line.
pub fn wrap(font: &HudFont, style: &TextStyle, text: &str, room: f32) -> Vec<String> {
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
            if !line.is_empty() && width(font, style, &next) > room {
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

// ---- the native pass ----------------------------------------------------

impl crate::hud_draw::Ctx<'_> {
    /// One run of `text` in `style` and `tint`, hung at `at` (its left
    /// edge, centre or right edge by `align`, and the line's top).
    pub(crate) fn text(
        &mut self,
        s: &Screen,
        style: &TextStyle,
        text: &str,
        at: (f32, f32),
        align: HAlign,
        tint: Tint,
    ) {
        self.text_popped(s, style, text, at, align, tint, 1.0);
    }

    /// [`Ctx::text`] with the glyphs `pop` × their height down from `at`
    /// (the shell's flashy entrance).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text_popped(
        &mut self,
        s: &Screen,
        style: &TextStyle,
        text: &str,
        at: (f32, f32),
        align: HAlign,
        (rgb, alpha): Tint,
        pop: f32,
    ) {
        let Some(font) = self.face(style.face) else {
            return;
        };
        let text = style.case.apply(text);
        let [kx, ky] = style.scale(font);
        self.run(
            usize::MAX,
            &glam::Mat4::IDENTITY,
            s,
            font,
            &Run {
                text: &text,
                at,
                h_align: align,
                scale: [kx, ky * pop],
                color: color(rgb, alpha),
                additive: false,
                shadow: false,
                tracking: style.tracking,
            },
        );
    }

    /// `text` centred in `rect` both ways, ended in an ellipsis if wider
    /// than the rect less [`TEXT_INSET`] a side.
    pub(crate) fn text_in(
        &mut self,
        s: &Screen,
        style: &TextStyle,
        text: &str,
        [x0, y0, x1, y1]: [f32; 4],
        tint: Tint,
    ) {
        let text = self.ellipsized(style, text, x1 - x0 - 2.0 * TEXT_INSET);
        let at = ((x0 + x1) * 0.5, (y0 + y1 - style.size) * 0.5);
        self.text(s, style, &text, at, HAlign::Center, tint);
    }

    /// How wide `text` is in `style` (0 without the face).
    pub(crate) fn text_width(&self, style: &TextStyle, text: &str) -> f32 {
        self.face(style.face).map_or(0.0, |f| width(f, style, text))
    }

    /// [`ellipsize`] with this art's face.
    pub(crate) fn ellipsized(&self, style: &TextStyle, text: &str, room: f32) -> String {
        match self.face(style.face) {
            Some(f) => ellipsize(f, style, text, room).into_owned(),
            None => text.to_owned(),
        }
    }

    /// [`wrap`] with this art's face.
    pub(crate) fn wrapped(&self, style: &TextStyle, text: &str, room: f32) -> Vec<String> {
        match self.face(style.face) {
            Some(f) => wrap(f, style, text, room),
            None => vec![text.to_owned()],
        }
    }
}

// ---- egui -------------------------------------------------------------

/// The height the menus lay out in for a `size` window: its height, or
/// 4:3 of its width on a window taller than that (the pause menu's
/// screen, [`crate::pause_draw`]). A unit is 1/448 of it on both passes.
pub fn menu_height(size: [f32; 2]) -> f32 {
    size[1].min(size[0] * 0.75)
}

/// Shell units → egui points for the context's window ([`menu_height`]).
pub fn points_per_unit(ctx: &egui::Context) -> f32 {
    let r = ctx.content_rect();
    menu_height([r.width(), r.height()]) / crate::hud_tree::FRAME_H
}

/// `units` shell units in egui points for this window (widths and gaps
/// that should follow the text's size).
pub fn egui_units(ctx: &egui::Context, units: f32) -> f32 {
    units * points_per_unit(ctx)
}

/// The egui font of `style` for this window: the same outlines
/// ([`face::install`]) at the same height as the native pass draws it.
/// Before the face is installed (a bake without it) egui's own face stands
/// in at the same size.
pub fn egui_font(ctx: &egui::Context, style: &TextStyle) -> egui::FontId {
    let size = (style.size * points_per_unit(ctx)).max(1.0);
    let family = egui::FontFamily::Name(style.face.family().into());
    let installed = ctx.fonts(|f| f.definitions().families.contains_key(&family));
    egui::FontId::new(
        size,
        if installed {
            family
        } else {
            egui::FontFamily::Proportional
        },
    )
}

/// A [`Tint`] as an egui colour (straight alpha).
pub fn egui_color((rgb, alpha): Tint) -> egui::Color32 {
    let c = |v: f32| v.clamp(0.0, 255.0).round() as u8;
    egui::Color32::from_rgba_unmultiplied(c(rgb[0]), c(rgb[1]), c(rgb[2]), c(alpha * 255.0))
}

/// `text` in `style` and `tint`, its case applied, for an egui label.
pub fn egui_text(ctx: &egui::Context, style: &TextStyle, tint: Tint, text: &str) -> egui::RichText {
    egui::RichText::new(style.case.apply(text).into_owned())
        .font(egui_font(ctx, style))
        .color(egui_color(tint))
}

/// The gap between egui rows of `style`, in points.
pub fn egui_line_gap(ctx: &egui::Context, style: &TextStyle) -> f32 {
    ((style.line - style.size).max(0.0) + 4.0) * points_per_unit(ctx)
}

#[cfg(test)]
mod tests;
