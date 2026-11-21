//! BF2's in-battle Game Menu, using the shell's art, fonts and button animation.

use crate::hud_draw::{color, Ctx, HudArt, HudFrame, HudVertex, Screen};
use crate::hud_tree::{HAlign, Hud, FRAME_H, FRAME_W};
use crate::input_icons::Prompt;
use crate::menu::{self, MenuInput, MenuModel, PausePage, Row};
use crate::scroll::{self, ScrollView, MARGIN};
use crate::shell_feel::{flashy, Pulse};
use crate::shell_ps2::{FlashyRow, BAR_PAD, CENTRE_X, ROW_REVEAL, UNFOCUSED_BAR};
use crate::spawn_draw::{item_pieces, screen_at, Anchor, PIECE_ALPHA, TITLE_PIECES};
use crate::typography::{self, tint, Tint, BODY, ITEM, ROW_H, ROW_PITCH, TEXT_INSET, TITLE};
use glam::{Mat4, Vec3};
use manifest::hud::HudFont;
use std::cell::Cell;

// Measured from the settled original single-player pause screen at 4:3
// (analysis/re-bf2-menus.md, ifs_pausemenu). Settings share the same stack.
/// The stack's least width, of the viewport; `[ours]` it grows to fit its
/// text (the shell's rule: [`BAR_PAD`] beyond the widest label) up to the
/// safe area.
const STACK_VIEWPORT_WIDTH: f32 = 0.72;
const STACK_Y: f32 = 221.0;
const ARROW_W: f32 = ROW_H * 0.72;
/// The least share of the bar the value and its arrows take.
const VALUE_WIDTH_RATIO: f32 = 0.33;
const PROMPT_ROOM: f32 = 72.0;
const FOOTER_TOP: f32 = FRAME_H - 51.0;
const DETAIL_ROOM: f32 = 56.0;
/// `[ours]` A long page's title bar may rise to here (the outcome line's
/// room under the safe area's top, 22.4) so more rows show at once.
const STACK_TOP_MIN: f32 = 22.4 + 24.0;
/// The safe area's margin across (`spawn_draw::SAFE_X`).
const SAFE_MARGIN: f32 = 25.6;

type Rect = [f32; 4];

/// UI time runs while the battle is paused. Also holds the list's view
/// ([`crate::scroll`]): drawing and hit tests read it, input moves it.
pub struct Animation {
    age: f32,
    pulse: Pulse,
    view: Cell<View>,
}

/// The list's view, which list it belongs to, the selection it last kept
/// in view, whether a pointer picked that selection, and the widths the
/// last drawing measured (pointer tests have no art to measure with).
#[derive(Debug, Clone, Copy, Default)]
struct View {
    scroll: ScrollView,
    seen: Option<(u64, usize)>,
    picked: bool,
    widths: Option<(u64, Widths)>,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            age: 0.0,
            pulse: Pulse::BUTTON,
            view: Cell::new(View::default()),
        }
    }
}

impl Animation {
    pub fn advance(&mut self, dt: f32) {
        self.age = (self.age + dt).min(2.0);
        self.pulse.advance(dt);
        let v = self.view.get_mut();
        v.scroll.advance(dt);
    }

    /// The list's first shown row, fractional (tests and captures).
    pub fn scroll_offset(&self) -> f32 {
        self.view.get().scroll.offset()
    }
}

/// A pointer action on the Game Menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Row(usize),
    Step { row: usize, right: bool },
    ClassTab(usize),
}

/// The bar's width and the share at its right the value and its arrows
/// take (0: the page has no values).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Widths {
    bar: f32,
    value: f32,
}

/// Which list `m` shows: a new one starts its view afresh. The page, not
/// its row count: a toggle that shows or hides rows on the same page keeps
/// the view (clamped to the new length).
fn list_id(m: &MenuModel) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    menu::list_key(m).hash(&mut h);
    menu::menu_title(m).hash(&mut h);
    m.class_choices.as_ref().map(|p| p.tab).hash(&mut h);
    h.finish()
}

/// The safe area across, in the menu screen's units.
fn safe_x(s: &Screen, size: [f32; 2]) -> [f32; 2] {
    let left = -s.origin[0] / s.scale[0];
    let right = (size[0] - s.origin[0]) / s.scale[0];
    [left + SAFE_MARGIN, right - SAFE_MARGIN]
}

/// The stack's widths for `labels`: as wide as its widest name and value
/// need at the role's own size, at least the original's share of the
/// viewport, at most the safe area.
fn measure(
    font: &HudFont,
    title: &str,
    labels: &[(String, Option<String>)],
    s: &Screen,
    size: [f32; 2],
) -> Widths {
    let w = |t: &str| typography::width(font, &ITEM, t);
    let name = labels.iter().map(|(n, _)| w(n)).fold(0.0, f32::max);
    let value = labels
        .iter()
        .filter_map(|(_, v)| v.as_deref().map(w))
        .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))));
    let value_room = value.map_or(0.0, |v| v + 2.0 * TEXT_INSET + 2.0 * ARROW_W);
    let name_room = name + 2.0 * TEXT_INSET + ROW_H * 0.5;
    let need = (typography::width(font, &TITLE, title) + BAR_PAD).max(if value.is_some() {
        // The names' column keeps at least its share when the values are
        // narrow ([`VALUE_WIDTH_RATIO`]).
        (name_room + value_room).max(name_room / (1.0 - VALUE_WIDTH_RATIO))
    } else {
        name + BAR_PAD
    });
    let [l, r] = safe_x(s, size);
    let least = (size[0] / s.scale[0] * STACK_VIEWPORT_WIDTH).min(FRAME_W);
    let bar = need.max(least).min(r - l);
    Widths {
        bar,
        value: if value.is_some() {
            value_room.max(bar * VALUE_WIDTH_RATIO)
        } else {
            0.0
        },
    }
}

struct Layout {
    rows: Vec<Row>,
    /// The list's view and how many rows its window holds.
    view: ScrollView,
    shown: usize,
    main: bool,
    title: Rect,
    widths: Widths,
    tabs: usize,
    /// The window's top, where row `offset` starts.
    rows_top: f32,
}

impl Layout {
    /// The page's geometry with `widths` (from the last drawing when the
    /// caller has no art), following the selection into view.
    fn new(
        m: &MenuModel,
        anim: &Animation,
        widths: Option<Widths>,
        s: &Screen,
        size: [f32; 2],
    ) -> Self {
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
        let bottom = FOOTER_TOP - detail_room;
        let head = (1 + tab_rows) as f32 * ROW_PITCH + prompt_room;
        let gap = ROW_PITCH - ROW_H;
        let capacity = ((bottom - STACK_TOP_MIN - head + gap) / ROW_PITCH)
            .floor()
            .max(1.0) as usize;
        let shown = rows.len().min(capacity).max(1);
        let span = head + shown as f32 * ROW_PITCH - gap;
        let top = (STACK_Y - span * 0.5).min(bottom - span).max(STACK_TOP_MIN);
        let id = list_id(m);
        let mut v = anim.view.get();
        let widths = widths
            .or_else(|| v.widths.filter(|(k, _)| *k == id).map(|(_, w)| w))
            .unwrap_or_else(|| {
                let bar = (size[0] / s.scale[0] * STACK_VIEWPORT_WIDTH).min(FRAME_W);
                Widths {
                    bar,
                    value: bar * VALUE_WIDTH_RATIO,
                }
            });
        v.widths = Some((id, widths));
        // Keep the selection in view: a new list starts with it shown, a
        // key-moved one keeps a margin, a pointer-picked one only comes
        // fully in.
        let cursor = m.cursor.min(rows.len().saturating_sub(1));
        match v.seen {
            Some((k, _)) if k != id => {
                v.scroll = ScrollView::default();
                v.scroll.reveal(cursor, rows.len(), shown, MARGIN);
                v.scroll.snap();
            }
            None => {
                v.scroll = ScrollView::default();
                v.scroll.reveal(cursor, rows.len(), shown, MARGIN);
                v.scroll.snap();
            }
            Some((_, c)) if c != cursor => {
                let margin = if v.picked { 0 } else { MARGIN };
                v.scroll.reveal(cursor, rows.len(), shown, margin);
            }
            _ => v.scroll.clamp(rows.len(), shown),
        }
        v.seen = Some((id, cursor));
        v.picked = false;
        anim.view.set(v);
        let width = widths.bar;
        Self {
            rows,
            view: v.scroll,
            shown,
            main,
            title: [
                CENTRE_X - width * 0.5,
                top,
                CENTRE_X + width * 0.5,
                top + ROW_H,
            ],
            widths,
            tabs,
            rows_top: top + head,
        }
    }

    fn row(&self, i: usize) -> Rect {
        let [left, _, right, _] = self.title;
        let y = self.rows_top + (i as f32 - self.view.offset()) * ROW_PITCH;
        [left, y, right, y + ROW_H]
    }

    /// The list's window: rows are cut at it.
    fn window(&self) -> Rect {
        let [left, _, right, _] = self.title;
        let bottom = self.rows_top + self.shown as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
        [left - 1.0, self.rows_top, right + 1.0, bottom]
    }

    fn visible(&self) -> std::ops::Range<usize> {
        self.view.visible(self.rows.len(), self.shown)
    }

    fn tab(&self, i: usize) -> Rect {
        let w = (self.title[2] - self.title[0]) / 3.0;
        let x = self.title[0] + (i % 3) as f32 * w;
        let y = self.title[1] + (1 + i / 3) as f32 * ROW_PITCH;
        [x + 1.0, y, x + w - 1.0, y + ROW_H]
    }

    fn arrows(&self, [_, y, right, bottom]: Rect) -> [Rect; 2] {
        [right - self.widths.value, right - ARROW_W].map(|x| [x, y, x + ARROW_W, bottom])
    }

    fn columns(&self, rect: Rect, has_value: bool) -> (Rect, Rect) {
        let [left, top, _, bottom] = rect;
        let [decrease, increase] = self.arrows(rect);
        let name = if has_value {
            [left + ROW_H * 0.5, top, decrease[0], bottom]
        } else {
            rect
        };
        (name, [decrease[2], top, increase[0], bottom])
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
    let h = typography::menu_height(size);
    let mut screen = screen_at([size[0], h], Anchor::Centre);
    screen.origin[1] = (size[1] - h) * 0.5;
    screen
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
    let tint = color(tint::HELP.0, tint::HELP.1);
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
    let layout = Layout::new(m, anim, None, &s, size);
    let p = [
        (point[0] - s.origin[0]) / s.scale[0],
        (point[1] - s.origin[1]) / s.scale[1],
    ];
    for i in 0..layout.tabs {
        if contains(layout.tab(i), p) {
            return Some(Target::ClassTab(i));
        }
    }
    if !contains(layout.window(), p) {
        return None;
    }
    for i in layout.visible() {
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
            let [left, right] = layout.arrows(rect);
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
/// same navigation after selecting the hit row, which leaves the list's view
/// where it is ([`crate::scroll`]).
pub fn click(
    m: &mut MenuModel,
    anim: &Animation,
    target: Target,
) -> (usize, Vec<menu::MenuAction>) {
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
    let mut v = anim.view.get();
    v.picked = true;
    anim.view.set(v);
    menu::navigate(m, input)
}

/// A pointer pick the caller activates itself (the binding list sets its
/// own cursor): like [`click`], the list's view stays where it is.
pub fn pick(anim: &Animation) {
    let mut v = anim.view.get();
    v.picked = true;
    anim.view.set(v);
}

/// Whether row `row` lies wholly inside the list's window as `m` is drawn
/// now (tests and captures).
pub fn row_in_view(m: &MenuModel, anim: &Animation, row: usize, size: [f32; 2]) -> bool {
    let s = menu_screen(size);
    let layout = Layout::new(m, anim, None, &s, size);
    let [_, top, _, bottom] = layout.row(row);
    let [_, w_top, _, w_bottom] = layout.window();
    top >= w_top - 1.0e-3 && bottom <= w_bottom + 1.0e-3
}

/// The wheel or a trackpad: `wheel` lines (> 0 up) scroll the list's view;
/// the selection stays where it is.
pub fn scroll(m: &MenuModel, anim: &Animation, wheel: f32, size: [f32; 2]) {
    let s = menu_screen(size);
    let layout = Layout::new(m, anim, None, &s, size);
    let mut v = anim.view.get();
    v.scroll.wheel(wheel, layout.rows.len(), layout.shown);
    anim.view.set(v);
}

fn colors(selected: bool, locked: bool, pulse: f32) -> (Tint, Tint) {
    let grey = ([UNFOCUSED_BAR; 3], tint::UNSELECTED.1);
    if locked {
        (grey, tint::DIMMED)
    } else if selected {
        (([255.0; 3], pulse), (tint::SELECTED.0, pulse))
    } else {
        (grey, tint::UNSELECTED)
    }
}

/// A paragraph in [`BODY`], broken at words to `rect`'s width and centred
/// in it; every word kept.
fn paragraph(ctx: &mut Ctx<'_>, s: &Screen, [x0, y0, x1, y1]: Rect, label: &str) {
    let lines = ctx.wrapped(&BODY, label, x1 - x0 - 2.0 * TEXT_INSET);
    let n = lines.len().max(1) as f32;
    let top = ((y0 + y1 - BODY.line * n) * 0.5).max(y0);
    for (i, line) in lines.iter().enumerate() {
        let at = ((x0 + x1) * 0.5, top + i as f32 * BODY.line);
        ctx.text(s, &BODY, line, at, HAlign::Center, tint::HELP);
    }
}

/// The translucent Game Menu stack and its subpages over the frozen scene.
pub fn draw(hud: &Hud, art: &HudArt, m: &MenuModel, anim: &Animation, size: [f32; 2]) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    draw_stack(&mut ctx, m, anim, size);
    ctx.out
}

pub(crate) fn draw_stack(ctx: &mut Ctx<'_>, m: &MenuModel, anim: &Animation, size: [f32; 2]) {
    ctx.spawn_veil(size);
    let s = menu_screen(size);
    let Some(font) = ctx.face(ITEM.face) else {
        return;
    };
    // Every row of the page at the role's own size: the bar makes room for
    // the widest, so no row is ever drawn smaller.
    let labels: Vec<_> = menu::rows(m)
        .iter()
        .map(|row| menu::row_text(m, *row))
        .collect();
    let title = menu::menu_title(m);
    let widths = measure(font, title, &labels, &s, size);
    let layout = Layout::new(m, anim, Some(widths), &s, size);
    let title_age = if layout.main { anim.age } else { 2.0 };
    let title = FlashyRow {
        rect: layout.title,
        pieces: TITLE_PIECES,
        bar: ([255.0; 3], PIECE_ALPHA),
        text: title,
        style: tint::TITLE,
        font: &crate::typography::TITLE,
    };
    ctx.flashy_row(&s, &title, title_age);

    let pulse = anim.pulse.alpha();
    if let Some(picker) = &m.class_choices {
        for (i, label) in picker.tabs.iter().enumerate() {
            let rect = layout.tab(i);
            let (bar, tint) = colors(i == picker.tab, false, pulse);
            ctx.bar_tinted(&s, rect, item_pieces(false), bar);
            ctx.text_in(&s, &ITEM, label, rect, tint);
        }
    }

    if let Some(prompt) = menu::popup_prompt(m) {
        let [left, _, right, bottom] = layout.title;
        let next = layout.rows_top;
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
        let rect = [left, top - 3.0 - BODY.size - 2.0, right, top - 3.0];
        ctx.text_in(&s, &BODY, outcome, rect, tint::HELP);
    }

    // The rows, at their (eased) place, cut at the list's window.
    let window = layout.window();
    let [_, wy0] = s.map(Vec3::new(window[0], window[1], 0.0));
    let [_, wy1] = s.map(Vec3::new(window[2], window[3], 0.0));
    ctx.clip = Some([f32::MIN, wy0, f32::MAX, wy1]);
    for i in layout.visible() {
        let (name, value) = &labels[i];
        let row = layout.rows[i];
        let rect = layout.row(i);
        let pieces = item_pieces(i + 1 == layout.rows.len());
        let locked = menu::row_locked(m, row).is_some();
        let (bar, tint) = colors(i == m.cursor, locked, pulse);
        if layout.main {
            ctx.flashy_row(
                &s,
                &FlashyRow {
                    rect,
                    pieces,
                    bar,
                    text: name,
                    style: tint,
                    font: &ITEM,
                },
                reveal(anim.age, i),
            );
            continue;
        }
        ctx.bar_tinted(&s, rect, pieces, bar);
        let (name_box, value_box) = layout.columns(rect, value.is_some());
        ctx.text_in(&s, &ITEM, name, name_box, tint);
        if let Some(value) = value {
            ctx.text_in(&s, &ITEM, value, value_box, tint);
            if menu::is_value_row(row) && !locked {
                for (arrow, [x0, y0, x1, y1]) in ["<", ">"].into_iter().zip(layout.arrows(rect)) {
                    let at = ((x0 + x1) * 0.5, (y0 + y1 - ITEM.size) * 0.5);
                    ctx.text(&s, &ITEM, arrow, at, HAlign::Center, tint);
                }
            }
        }
    }
    ctx.clip = None;

    let last = layout.rows_top + (layout.shown as f32 - 1.0) * ROW_PITCH;
    let [left, _, right, _] = layout.title;
    if layout.view.offset() > 0.01 {
        scroll_arrow(
            ctx,
            &s,
            [left, layout.rows_top, right, layout.rows_top + ROW_H],
            false,
        );
    }
    if layout.view.offset() < scroll::max_offset(layout.rows.len(), layout.shown) - 0.01 {
        scroll_arrow(ctx, &s, [left, last, right, last + ROW_H], true);
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
    if let Some(detail) = detail.filter(|_| !layout.rows.is_empty()) {
        let bottom = window[3];
        let [l, r] = safe_x(&s, size);
        paragraph(
            ctx,
            &s,
            [
                (left - 64.0).max(l),
                bottom + 4.0,
                (right + 64.0).min(r),
                FOOTER_TOP,
            ],
            &detail,
        );
    }

    let help_h = size[1].min(size[0] * 0.75);
    // On a tall window the prompts stay at the real safe-area edges, using
    // the centered stack's scale.
    ctx.help_bottom([size[0], help_h], false, Prompt::Back, "Back", size[1]);
    ctx.help_bottom([size[0], help_h], true, Prompt::Accept, "Select", size[1]);
}
