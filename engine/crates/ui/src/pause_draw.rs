//! BF2's in-battle Game Menu, using the shell's art, fonts and button animation.

use crate::hud_draw::{color, run_width, Ctx, HudArt, HudFrame, HudVertex, Run, Screen};
use crate::hud_tree::{HAlign, Hud, FRAME_H};
use crate::input_icons::Prompt;
use crate::menu::{self, MenuInput, MenuModel, PausePage, Row};
use crate::shell_feel::{flashy, Pulse};
use crate::shell_ps2::{FlashyRow, CENTRE_X, DIMMED_GREY, ROW_REVEAL, UNFOCUSED_BAR};
use crate::spawn_draw::{
    item_pieces, screen_at, Anchor, BUTTON_FONT, HELP_TEXT, LABEL_STRETCH, PIECE_ALPHA, ROW_H,
    ROW_PITCH, SELECTED_TEXT, SMALL_FONT, SMALL_STRETCH, TITLE_PIECES, TITLE_TEXT, UNSELECTED_TEXT,
};
use glam::{Mat4, Vec3};

// Measured from the settled original single-player pause screen at 4:3
// (analysis/re-bf2-menus.md, ifs_pausemenu). Compact pages are port options.
const STACK_W: f32 = 320.0;
const STACK_Y: f32 = 221.0;
const COMPACT_H: f32 = 20.0;
const COMPACT_PITCH: f32 = 23.0;
const VALUE_W: f32 = 136.0;
const ARROW_W: f32 = 23.0;
const COMPACT_TEXT_INSET: f32 = 4.0;

type Rect = [f32; 4];
type Tint = ([f32; 3], f32);

/// UI time runs while the battle is paused.
pub struct Animation {
    age: f32,
    pulse: Pulse,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            age: 0.0,
            pulse: Pulse::BUTTON,
        }
    }
}

impl Animation {
    pub fn advance(&mut self, dt: f32) {
        self.age = (self.age + dt).min(2.0);
        self.pulse.advance(dt);
    }
}

/// A pointer action on the Game Menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Row(usize),
    Step { row: usize, right: bool },
    ClassTab(usize),
}

struct Layout {
    rows: Vec<Row>,
    first: usize,
    end: usize,
    main: bool,
    title: Rect,
    height: f32,
    pitch: f32,
    tabs: usize,
    tab_rows: usize,
    prompt_room: f32,
}

impl Layout {
    fn new(m: &MenuModel) -> Self {
        let rows = menu::rows(m);
        let tabs = m.class_choices.as_ref().map_or(0, |p| p.tabs.len());
        let tab_rows = tabs.div_ceil(3);
        let main =
            m.page == Some(PausePage::Main) && m.popup.is_none() && m.class_choices.is_none();
        let (height, pitch) = if main {
            (ROW_H, ROW_PITCH)
        } else {
            (COMPACT_H, COMPACT_PITCH)
        };
        let prompt_room = if m.popup.is_some() { 72.0 } else { 0.0 };
        let shown = rows.len().min(10_usize.saturating_sub(tab_rows).max(1));
        let first = menu::first_row(m.cursor.min(rows.len() - 1), shown, rows.len());
        let span = ROW_PITCH + (tab_rows + shown - 1) as f32 * pitch + prompt_room + height;
        let top = STACK_Y - span * 0.5;
        Self {
            rows,
            first,
            end: first + shown,
            main,
            title: [
                CENTRE_X - STACK_W * 0.5,
                top,
                CENTRE_X + STACK_W * 0.5,
                top + ROW_H,
            ],
            height,
            pitch,
            tabs,
            tab_rows,
            prompt_room,
        }
    }

    fn row(&self, i: usize) -> Rect {
        let [left, top, right, _] = self.title;
        let y = top
            + ROW_PITCH
            + (self.tab_rows + i - self.first) as f32 * self.pitch
            + self.prompt_room;
        [left, y, right, y + self.height]
    }

    fn tab(&self, i: usize) -> Rect {
        let w = STACK_W / 3.0;
        let x = self.title[0] + (i % 3) as f32 * w;
        let y = self.title[1] + ROW_PITCH + (i / 3) as f32 * self.pitch;
        [x + 1.0, y, x + w - 1.0, y + self.height]
    }
}

/// All button pieces this menu needs for the baked-art path.
pub fn button_sheets() -> impl Iterator<Item = &'static str> {
    TITLE_PIECES
        .into_iter()
        .chain(item_pieces(false))
        .chain(item_pieces(true))
}

fn menu_screen(size: [f32; 2]) -> Screen {
    // Keep the stack and prompts in the viewport on tall windows too.
    let h = size[1].min(size[0] * 0.75);
    let mut screen = screen_at([size[0], h], Anchor::Centre);
    screen.origin[1] = (size[1] - h) * 0.5;
    screen
}

fn arrows([_, y, right, bottom]: Rect) -> [Rect; 2] {
    [right - VALUE_W, right - ARROW_W].map(|x| [x, y, x + ARROW_W, bottom])
}

fn scroll_arrow(ctx: &mut Ctx<'_>, s: &Screen, [_, top, right, bottom]: Rect, down: bool) {
    let center = Vec3::new(right + 12.0, (top + bottom) * 0.5, 0.0);
    let direction = if down { -1.0 } else { 1.0 };
    let points = [
        (0.0, -6.0),
        (5.0, 0.0),
        (1.5, 0.0),
        (1.5, 6.0),
        (-1.5, 6.0),
        (-1.5, 0.0),
        (-5.0, 0.0),
    ];
    let tint = color(HELP_TEXT.0, HELP_TEXT.1);
    let base = ctx.out.verts.len() as u32;
    ctx.out.verts.extend(points.map(|(x, y)| {
        let [x, y] = s.map(center + Vec3::new(x, direction * y, 0.0));
        HudVertex {
            pos: [x, y, 0.0],
            uv: [0.5, 0.5],
            color: tint,
        }
    }));
    let first = ctx.out.indices.len() as u32;
    ctx.out
        .indices
        .extend([0, 1, 6, 2, 3, 4, 2, 4, 5].map(|i| base + i));
    ctx.out.batch("white", false, false, first);
}

fn contains([x0, y0, x1, y1]: Rect, [x, y]: [f32; 2]) -> bool {
    (x0..=x1).contains(&x) && (y0..=y1).contains(&y)
}

fn reveal(age: f32, i: usize) -> f32 {
    age - ROW_REVEAL * (i + 1) as f32
}

pub fn target(m: &MenuModel, anim: &Animation, point: [f32; 2], size: [f32; 2]) -> Option<Target> {
    let layout = Layout::new(m);
    let s = menu_screen(size);
    let p = [
        (point[0] - s.origin[0]) / s.scale[0],
        (point[1] - s.origin[1]) / s.scale[1],
    ];
    for i in 0..layout.tabs {
        if contains(layout.tab(i), p) {
            return Some(Target::ClassTab(i));
        }
    }
    for i in layout.first..layout.end {
        let row = layout.rows[i];
        let mut rect = layout.row(i);
        if layout.main {
            let Some(wipe) = flashy(reveal(anim.age, i)).bar else {
                continue;
            };
            let caps = 2.0 * ROW_H;
            rect[2] = rect[0] + caps + (STACK_W - caps) * wipe;
        }
        if !contains(rect, p) {
            continue;
        }
        if menu::is_value_row(row) && menu::row_locked(m, row).is_none() {
            let [left, right] = arrows(rect);
            if contains(left, p) || contains(right, p) {
                return Some(Target::Step {
                    row: i,
                    right: contains(right, p),
                });
            }
        }
        return Some(Target::Row(i));
    }
    None
}

/// Keyboard and pad use `menu::navigate` directly. Pointer actions use the
/// same navigation after focusing the hit row.
pub fn click(m: &mut MenuModel, target: Target) -> (usize, Vec<menu::MenuAction>) {
    let mut input = MenuInput::default();
    match target {
        Target::Row(row) => {
            m.cursor = row;
            input.confirm = true;
        }
        Target::Step { row, right } => {
            m.cursor = row;
            input.left = !right;
            input.right = right;
        }
        Target::ClassTab(tab) => return (0, vec![menu::MenuAction::SetClassTab(tab)]),
    }
    menu::navigate(m, input)
}

pub fn scroll(m: &mut MenuModel, wheel: f32) {
    let layout = Layout::new(m);
    if layout.end - layout.first < layout.rows.len() {
        let steps = wheel.abs().ceil() as usize;
        m.cursor = if wheel > 0.0 {
            m.cursor.saturating_sub(steps)
        } else {
            m.cursor.saturating_add(steps).min(layout.rows.len() - 1)
        };
    }
}

fn colors(selected: bool, locked: bool, pulse: f32) -> (Tint, Tint) {
    let grey = ([UNFOCUSED_BAR; 3], UNSELECTED_TEXT.1);
    if locked {
        (grey, ([DIMMED_GREY; 3], UNSELECTED_TEXT.1))
    } else if selected {
        (([255.0; 3], pulse), (SELECTED_TEXT.0, pulse))
    } else {
        (grey, UNSELECTED_TEXT)
    }
}

fn text(ctx: &mut Ctx<'_>, s: &Screen, rect: Rect, label: &str, tint: Tint) {
    row_text(ctx, s, rect, label, tint, false);
}

fn row_text(ctx: &mut Ctx<'_>, s: &Screen, rect: Rect, label: &str, tint: Tint, compact: bool) {
    let (role, stretch, inset) = if compact {
        (BUTTON_FONT, LABEL_STRETCH, COMPACT_TEXT_INSET)
    } else {
        (SMALL_FONT, SMALL_STRETCH, 0.0)
    };
    let Some(font) = ctx.font(role) else {
        return;
    };
    let [x0, y0, x1, y1] = rect;
    let h = f32::from(font.line_height);
    let w = run_width(font, label, stretch, false);
    let ky = ((y1 - y0 - inset) / h.max(1.0)).clamp(0.0, 1.0);
    let kx = ((x1 - x0 - 4.0) / w.max(1.0)).clamp(0.0, ky);
    // Fit compact labels proportionally so a long name keeps the shell
    // face's stroke proportions.
    let ky = if compact { kx } else { ky };
    ctx.run(
        usize::MAX,
        &Mat4::IDENTITY,
        s,
        font,
        &Run {
            text: label,
            at: ((x0 + x1) * 0.5, (y0 + y1 - h * ky) * 0.5),
            h_align: HAlign::Center,
            scale: [kx * stretch, ky],
            color: color(tint.0, tint.1),
            additive: false,
            shadow: false,
        },
    );
}

fn paragraph(ctx: &mut Ctx<'_>, s: &Screen, rect: Rect, label: &str) {
    let Some(font) = ctx.font(SMALL_FONT) else {
        return;
    };
    let [x0, y0, x1, y1] = rect;
    let h = f32::from(font.line_height).max(1.0);
    let mut lines = Vec::new();
    for part in label.lines() {
        let mut line = String::new();
        for word in part.split_whitespace() {
            let next = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && run_width(font, &next, SMALL_STRETCH, false) > x1 - x0 - 4.0 {
                lines.push(line);
                line = word.to_owned();
            } else {
                line = next;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    // Keep all the words, even if a translation needs more room.
    let n = lines.len().max(1) as f32;
    let pitch = h.min((y1 - y0) / n);
    let top = (y0 + y1 - pitch * n) * 0.5;
    for (i, line) in lines.iter().enumerate() {
        let y = top + i as f32 * pitch;
        text(ctx, s, [x0, y, x1, y + pitch], line, HELP_TEXT);
    }
}

/// The translucent Game Menu stack and its subpages over the frozen scene.
pub fn draw(hud: &Hud, art: &HudArt, m: &MenuModel, anim: &Animation, size: [f32; 2]) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        menu_fonts: true,
    };
    draw_stack(&mut ctx, m, anim, size);
    ctx.out
}

pub(crate) fn draw_stack(ctx: &mut Ctx<'_>, m: &MenuModel, anim: &Animation, size: [f32; 2]) {
    ctx.spawn_veil(size);
    let s = menu_screen(size);
    let layout = Layout::new(m);
    let Some(font) = ctx.font(BUTTON_FONT) else {
        return;
    };
    let lh = f32::from(font.line_height);
    let title = menu::menu_title(m);
    let title_age = if layout.main { anim.age } else { 2.0 };
    let title = FlashyRow {
        rect: layout.title,
        pieces: TITLE_PIECES,
        bar: ([255.0; 3], PIECE_ALPHA),
        text: title,
        style: TITLE_TEXT,
    };
    ctx.flashy_row(&s, &title, title_age, lh);

    let pulse = anim.pulse.alpha();
    if let Some(picker) = &m.class_choices {
        for (i, label) in picker.tabs.iter().enumerate() {
            let rect = layout.tab(i);
            let (bar, tint) = colors(i == picker.tab, false, pulse);
            ctx.bar_tinted(&s, rect, item_pieces(false), bar);
            text(ctx, &s, rect, label, tint);
        }
    }

    if let Some(prompt) = menu::popup_prompt(m) {
        let [left, _, right, bottom] = layout.title;
        let next = layout.row(layout.first)[1];
        ctx.quad(
            usize::MAX,
            &Mat4::IDENTITY,
            &s,
            &crate::hud_draw::Quad {
                rect: [left, bottom, right, next - 2.0],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [0.0, 0.0, 0.0, 0.92],
                texture: "white",
                additive: false,
                shadow: false,
                boxed: false,
            },
        );
        paragraph(
            ctx,
            &s,
            [left + 10.0, bottom + 4.0, right - 10.0, next - 4.0],
            prompt,
        );
    }
    if let Some(outcome) = &m.outcome_line {
        let [left, top, right, _] = layout.title;
        text(
            ctx,
            &s,
            [left, top - 23.0, right, top - 3.0],
            outcome,
            HELP_TEXT,
        );
    }

    for i in layout.first..layout.end {
        let row = layout.rows[i];
        let rect = layout.row(i);
        let pieces = item_pieces(i + 1 == layout.end);
        let (name, value) = menu::row_text(m, row);
        let locked = menu::row_locked(m, row).is_some();
        let (bar, tint) = colors(i == m.cursor, locked, pulse);
        if layout.main {
            let age = reveal(anim.age, i);
            if run_width(font, &name, LABEL_STRETCH, false) <= STACK_W - 12.0 {
                ctx.flashy_row(
                    &s,
                    &FlashyRow {
                        rect,
                        pieces,
                        bar,
                        text: &name,
                        style: tint,
                    },
                    age,
                    lh,
                );
            } else {
                // Long localized labels and the test picker use the smaller face.
                ctx.flashy_row(
                    &s,
                    &FlashyRow {
                        rect,
                        pieces,
                        bar,
                        text: "",
                        style: tint,
                    },
                    age,
                    lh,
                );
                let phase = flashy(age);
                if phase.text_a > 0.0 {
                    text(ctx, &s, rect, &name, (tint.0, tint.1 * phase.text_a));
                }
            }
            continue;
        }
        let [left, top, right, bottom] = rect;
        ctx.bar_tinted(&s, rect, pieces, bar);
        let compact = m.bindings.is_some() || matches!(m.page, Some(PausePage::Settings(_)));
        if let Some(value) = value {
            let value_x = right - VALUE_W;
            row_text(
                ctx,
                &s,
                [left + 8.0, top, value_x - 3.0, bottom],
                &name,
                tint,
                compact,
            );
            row_text(
                ctx,
                &s,
                [value_x + ARROW_W, top, right - ARROW_W, bottom],
                &value,
                tint,
                compact,
            );
            if menu::is_value_row(row) && !locked {
                for (arrow, rect) in ["<", ">"].into_iter().zip(arrows(rect)) {
                    row_text(ctx, &s, rect, arrow, tint, compact);
                }
            }
        } else {
            row_text(ctx, &s, rect, &name, tint, compact);
        }
    }

    if layout.first > 0 {
        scroll_arrow(ctx, &s, layout.row(layout.first), false);
    }
    if layout.end < layout.rows.len() {
        scroll_arrow(ctx, &s, layout.row(layout.end - 1), true);
    }
    let row = layout.rows[m.cursor.min(layout.rows.len() - 1)];
    let detail = menu::row_help(m, row).or_else(|| {
        m.class_choices
            .as_ref()
            .filter(|p| p.choices.is_empty())
            .map(|_| "No supported characters in this tab.".into())
    });
    if let Some(detail) = detail {
        let bottom = layout.row(layout.end - 1)[3];
        let [left, _, right, _] = layout.title;
        paragraph(
            ctx,
            &s,
            [left - 64.0, bottom + 4.0, right + 64.0, FRAME_H - 51.0],
            &detail,
        );
    }

    let help_h = size[1].min(size[0] * 0.75);
    // On a tall window the prompts stay at the real safe-area edges, using
    // the centered stack's scale.
    ctx.help_bottom([size[0], help_h], false, Prompt::Back, "Back", size[1]);
    ctx.help_bottom([size[0], help_h], true, Prompt::Accept, "Select", size[1]);
}
