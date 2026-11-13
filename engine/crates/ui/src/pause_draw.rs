//! BF2's in-battle Game Menu, using the shell's art, fonts and button animation.

use crate::hud_draw::{color, run_width, Ctx, HudArt, HudFrame, HudVertex, Run, Screen};
use crate::hud_tree::{HAlign, Hud, FRAME_H, FRAME_W};
use crate::input_icons::Prompt;
use crate::menu::{self, MenuInput, MenuModel, PausePage, Row};
use crate::shell_feel::{flashy, Pulse};
use crate::shell_ps2::{FlashyRow, CENTRE_X, DIMMED_GREY, ROW_REVEAL, UNFOCUSED_BAR};
use crate::spawn_draw::{
    item_pieces, screen_at, Anchor, BUTTON_FONT, HELP_TEXT, LABEL_STRETCH, PIECE_ALPHA, ROW_H,
    ROW_PITCH, SELECTED_TEXT, SMALL_FONT, SMALL_STRETCH, TITLE_PIECES, TITLE_TEXT, UNSELECTED_TEXT,
};
use glam::{Mat4, Vec3};
use manifest::hud::HudFont;

// Measured from the settled original single-player pause screen at 4:3
// (analysis/re-bf2-menus.md, ifs_pausemenu). Settings share the same stack.
const STACK_VIEWPORT_WIDTH: f32 = 0.72;
const STACK_Y: f32 = 221.0;
const TEXT_INSET: f32 = 5.0;
const ARROW_W: f32 = ROW_H * 0.72;
const VALUE_WIDTH_RATIO: f32 = 0.33;
const PROMPT_ROOM: f32 = 72.0;
const FOOTER_TOP: f32 = FRAME_H - 51.0;
const DETAIL_ROOM: f32 = 56.0;

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
    tabs: usize,
    tab_rows: usize,
    prompt_room: f32,
}

impl Layout {
    fn new(m: &MenuModel, width: f32) -> Self {
        let rows = menu::rows(m);
        let tabs = m.class_choices.as_ref().map_or(0, |p| p.tabs.len());
        let tab_rows = tabs.div_ceil(3);
        let main =
            m.page == Some(PausePage::Main) && m.popup.is_none() && m.class_choices.is_none();
        let prompt_room = if m.popup.is_some() { PROMPT_ROOM } else { 0.0 };
        // Reserve the same space below every settings stack for messages, even
        // when the selected row has no message. Moving focus never shifts it.
        let detail_room = if main || m.popup.is_some() {
            0.0
        } else {
            DETAIL_ROOM
        };
        let half_space = STACK_Y.min(FOOTER_TOP - STACK_Y - detail_room);
        let capacity = ((half_space * 2.0 - prompt_room - ROW_H) / ROW_PITCH) as usize;
        let shown = rows.len().min(capacity.saturating_sub(tab_rows).max(1));
        let first = menu::first_row(
            m.cursor.min(rows.len().saturating_sub(1)),
            shown,
            rows.len(),
        );
        let span = (tab_rows + shown) as f32 * ROW_PITCH + prompt_room + ROW_H;
        let top = STACK_Y - span * 0.5;
        Self {
            rows,
            first,
            end: first + shown,
            main,
            title: [
                CENTRE_X - width * 0.5,
                top,
                CENTRE_X + width * 0.5,
                top + ROW_H,
            ],
            tabs,
            tab_rows,
            prompt_room,
        }
    }

    fn row(&self, i: usize) -> Rect {
        let [left, top, right, _] = self.title;
        let y = top + (1 + self.tab_rows + i - self.first) as f32 * ROW_PITCH + self.prompt_room;
        [left, y, right, y + ROW_H]
    }

    fn tab(&self, i: usize) -> Rect {
        let w = (self.title[2] - self.title[0]) / 3.0;
        let x = self.title[0] + (i % 3) as f32 * w;
        let y = self.title[1] + (1 + i / 3) as f32 * ROW_PITCH;
        [x + 1.0, y, x + w - 1.0, y + ROW_H]
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

fn stack_width(s: &Screen, size: [f32; 2]) -> f32 {
    (size[0] / s.scale[0] * STACK_VIEWPORT_WIDTH).min(FRAME_W)
}

fn arrows([left, y, right, bottom]: Rect) -> [Rect; 2] {
    [right - (right - left) * VALUE_WIDTH_RATIO, right - ARROW_W]
        .map(|x| [x, y, x + ARROW_W, bottom])
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
    let s = menu_screen(size);
    let layout = Layout::new(m, stack_width(&s, size));
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
            rect[2] = rect[0] + caps + (rect[2] - rect[0] - caps) * wipe;
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
    let layout = Layout::new(m, FRAME_W);
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
    let Some(font) = ctx.font(SMALL_FONT) else {
        return;
    };
    let scale = fit(font, rect, label, SMALL_STRETCH);
    label_at(
        ctx,
        s,
        rect,
        label,
        (SMALL_FONT, SMALL_STRETCH),
        scale,
        tint,
    );
}

fn fit(font: &HudFont, [left, top, right, bottom]: Rect, label: &str, stretch: f32) -> f32 {
    let width = run_width(font, label, stretch, false).max(1.0);
    let height = f32::from(font.line_height).max(1.0);
    ((right - left - 2.0 * TEXT_INSET) / width)
        .min((bottom - top) / height)
        .clamp(0.0, 1.0)
}

fn columns(rect: Rect, has_value: bool) -> (Rect, Rect) {
    let [left, top, _, bottom] = rect;
    let [decrease, increase] = arrows(rect);
    let name = if has_value {
        [left, top, decrease[0], bottom]
    } else {
        rect
    };
    (name, [decrease[2], top, increase[0], bottom])
}

fn label_at(
    ctx: &mut Ctx<'_>,
    s: &Screen,
    [x0, y0, x1, y1]: Rect,
    label: &str,
    (role, stretch): (&str, f32),
    scale: f32,
    tint: Tint,
) {
    let Some(font) = ctx.font(role) else { return };
    let h = f32::from(font.line_height);
    ctx.run(
        usize::MAX,
        &Mat4::IDENTITY,
        s,
        font,
        &Run {
            text: label,
            at: ((x0 + x1) * 0.5, (y0 + y1 - h * scale) * 0.5),
            h_align: HAlign::Center,
            scale: [stretch * scale, scale],
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
    let layout = Layout::new(m, stack_width(&s, size));
    let Some(font) = ctx.font(BUTTON_FONT) else {
        return;
    };
    let lh = f32::from(font.line_height);
    // Measure the entire page so rows share one type size and scrolling
    // never changes it. The stock size fits unless a label needs more space.
    let labels: Vec<_> = layout
        .rows
        .iter()
        .map(|row| menu::row_text(m, *row))
        .collect();
    let text_scale = labels.iter().fold(1.0_f32, |scale, (name, value)| {
        let (name_box, value_box) = columns(layout.title, value.is_some());
        let scale = scale.min(fit(font, name_box, name, LABEL_STRETCH));
        value.as_ref().map_or(scale, |value| {
            scale.min(fit(font, value_box, value, LABEL_STRETCH))
        })
    });
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

    for (i, (name, value)) in labels
        .iter()
        .enumerate()
        .take(layout.end)
        .skip(layout.first)
    {
        let row = layout.rows[i];
        let rect = layout.row(i);
        let pieces = item_pieces(i + 1 == layout.end);
        let locked = menu::row_locked(m, row).is_some();
        let (bar, mut tint) = colors(i == m.cursor, locked, pulse);
        if layout.main {
            let age = reveal(anim.age, i);
            let at_stock_size = text_scale == 1.0;
            ctx.flashy_row(
                &s,
                &FlashyRow {
                    rect,
                    pieces,
                    bar,
                    text: if at_stock_size { name } else { "" },
                    style: tint,
                },
                age,
                lh,
            );
            if at_stock_size {
                continue;
            }
            tint.1 *= flashy(age).text_a;
        } else {
            ctx.bar_tinted(&s, rect, pieces, bar);
        }
        let face = (BUTTON_FONT, LABEL_STRETCH);
        let (name_box, value_box) = columns(rect, value.is_some());
        label_at(ctx, &s, name_box, name, face, text_scale, tint);
        if let Some(value) = value {
            label_at(ctx, &s, value_box, value, face, text_scale, tint);
            if menu::is_value_row(row) && !locked {
                for (arrow, rect) in ["<", ">"].into_iter().zip(arrows(rect)) {
                    label_at(ctx, &s, rect, arrow, face, text_scale, tint);
                }
            }
        }
    }

    if layout.first > 0 {
        scroll_arrow(ctx, &s, layout.row(layout.first), false);
    }
    if layout.end < layout.rows.len() {
        scroll_arrow(ctx, &s, layout.row(layout.end - 1), true);
    }
    let detail = layout
        .rows
        .get(m.cursor)
        .and_then(|&row| menu::row_help(m, row))
        .or_else(|| {
            m.class_choices
                .as_ref()
                .filter(|p| p.choices.is_empty())
                .map(|_| "No supported characters in this tab.".into())
        });
    if let Some(detail) = detail.filter(|_| layout.end > 0) {
        let bottom = layout.row(layout.end - 1)[3];
        let [left, _, right, _] = layout.title;
        paragraph(
            ctx,
            &s,
            [left - 64.0, bottom + 4.0, right + 64.0, FOOTER_TOP],
            &detail,
        );
    }

    let help_h = size[1].min(size[0] * 0.75);
    // On a tall window the prompts stay at the real safe-area edges, using
    // the centered stack's scale.
    ctx.help_bottom([size[0], help_h], false, Prompt::Back, "Back", size[1]);
    ctx.help_bottom([size[0], help_h], true, Prompt::Accept, "Select", size[1]);
}
