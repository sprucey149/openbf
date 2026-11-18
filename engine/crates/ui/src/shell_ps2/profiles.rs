//! The BF2 profile picker, name keyboard, difficulty and delete confirmation.

use crate::{
    hud_draw::{Ctx, Screen},
    hud_tree::{HAlign, FRAME_H, FRAME_W},
    input_icons::{pad_sheet, IconSet, PadButton, Prompt},
    scroll::{ScrollView, MARGIN},
    spawn_draw::{item_pieces, HELP_Y, ROW_H, ROW_PITCH, SAFE_X, TITLE_PIECES},
    typography::{tint, BODY, ITEM, TEXT_INSET, TITLE},
};

const SHOWN: usize = 5;
/// The list window "Pick a Profile" draws its names in: five rows of the
/// font's height + 6 (28) and 30 more, 170 tall (`ifs_login` main.f44;
/// `[M]` `profile_pick.png`: the window's border 106..288 px, 99..269
/// units).
const LIST_WINDOW: [f32; 4] = [104.0, 98.0, 408.0, 268.0];
/// The names inside it: the list 260 wide in the 295-wide window, so
/// 17.5 / 295 of the window in from either side, and from half of the 30
/// below its top, one row of 28 after another.
const LIST_IN: f32 = 18.0;
const LIST_PAD: f32 = 15.0;
const LIST_ROW: f32 = 28.0;
/// The name keyboard's box, and the typed name's field inside it, clear of
/// the box's border line as the frame's field is (`profile_name.png`).
const NAME_BOX: [f32; 4] = [24.0, 134.0, 488.0, 341.0];
/// OURS: 12 in from the box's sides and 6 below its top, 22 tall. `[M]`
/// `profile_name.png`: the frame's field outline runs 40..600 px across and
/// 93..143 px down inside the box's border at 27..612 px and 82 px, so
/// about 10 units in from each side and the top, 47 units tall.
/// measure: fit the field to the frame's insets and height with the box.
const NAME_FIELD: [f32; 4] = [36.0, 140.0, 476.0, 162.0];
/// "Create" just below the window, inset as the frame's bar is (`[M]`
/// `profile_pick.png`: 296..326 px down, from 137 px across).
const CREATE: [f32; 4] = [110.0, 276.0, 402.0, 304.0];
const COLUMNS: usize = 13;
const KEYS: [&str; 2] = [
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
    "0123456789 !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~",
];
pub const DIFFICULTIES: [&str; 4] = ["Easy", "Normal", "Hard", "Elite"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Profile(usize),
    Create,
    Delete,
    Key(usize),
    Cancel,
    Backspace,
    Accept,
    Difficulty(usize),
    No,
    Yes,
    Previous,
    Next,
    Symbols,
}

#[derive(Debug, Default)]
pub enum Page {
    #[default]
    List,
    Name {
        text: String,
        symbols: bool,
        suggested: bool,
    },
    Difficulty(String),
    Delete(u64),
}

#[derive(Debug)]
pub struct Picker {
    pub names: Vec<(u64, String)>,
    pub page: Page,
    pub focus: Target,
    pub message: Option<String>,
    /// The name keyboard's title when it names something else (a Rise of
    /// the Empire save: `ifs.vkeyboard.title_rote`, `ifs_campaign_load`
    /// main.f32 pc 166-170).
    pub name_title: Option<String>,
    /// The name list's view ([`crate::scroll`]).
    view: ScrollView,
}

impl Picker {
    pub fn new(names: Vec<(u64, String)>, last: Option<u64>) -> Self {
        let mut picker = Self {
            names,
            page: Page::List,
            focus: Target::Create,
            message: None,
            name_title: None,
            view: ScrollView::default(),
        };
        if let Some(index) = picker
            .names
            .iter()
            .position(|(id, _)| Some(*id) == last)
            .or_else(|| (!picker.names.is_empty()).then_some(0))
        {
            picker.set_focus(Target::Profile(index));
            picker.view.snap();
        }
        picker
    }

    /// Focus `target` from the keys or the pad: the list scrolls as little
    /// as keeps it a row inside the window.
    pub fn set_focus(&mut self, target: Target) {
        self.focus_with(target, MARGIN);
    }

    /// Focus `target` under the pointer: the list stays where it is (a row
    /// the window cuts comes fully in).
    pub fn point_focus(&mut self, target: Target) {
        self.focus_with(target, 0);
    }

    fn focus_with(&mut self, target: Target, margin: usize) {
        self.focus = target;
        if let Target::Profile(i) = target {
            self.view.reveal(i, self.names.len(), SHOWN, margin);
        }
    }

    /// The wheel: `lines` (> 0 up) scroll the names, not the focus.
    pub fn wheel(&mut self, lines: f32) {
        if matches!(self.page, Page::List) {
            self.view.wheel(lines, self.names.len(), SHOWN);
        }
    }

    /// Ease the list's view over `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        self.view.advance(dt);
    }

    /// The first name at least partly shown.
    fn first(&self) -> usize {
        self.view.first()
    }

    pub fn selected(&self) -> Option<(u64, &str)> {
        let i = match self.focus {
            Target::Profile(i) => i,
            _ => return None,
        };
        self.names.get(i).map(|(id, name)| (*id, name.as_str()))
    }

    pub fn edit_name(&mut self, suggested: String) {
        self.page = Page::Name {
            text: suggested,
            symbols: false,
            suggested: true,
        };
        self.focus = Target::Accept;
        self.message = None;
    }

    pub fn resume_name(&mut self, name: String) {
        self.edit_name(name);
        if let Page::Name { suggested, .. } = &mut self.page {
            *suggested = false;
        }
    }

    pub fn type_name(&mut self, text: &str, limit: usize) {
        if let Page::Name {
            text: name,
            suggested,
            ..
        } = &mut self.page
        {
            if *suggested {
                name.clear();
                *suggested = false;
            }
            let available = limit.saturating_sub(name.chars().count());
            name.extend(text.chars().take(available));
            self.message = None;
        }
    }

    pub fn backspace(&mut self) {
        if let Page::Name {
            text, suggested, ..
        } = &mut self.page
        {
            *suggested = false;
            text.pop();
            self.message = None;
        }
    }

    pub fn toggle_symbols(&mut self) {
        if let Page::Name { symbols, .. } = &mut self.page {
            *symbols = !*symbols;
            if let Target::Key(i) = self.focus {
                self.focus = Target::Key(i.min(self.keys().len() - 1));
            }
        }
    }

    pub fn keys(&self) -> &'static str {
        KEYS[usize::from(matches!(self.page, Page::Name { symbols: true, .. }))]
    }

    pub fn page_list(&mut self, direction: isize) {
        if !matches!(self.page, Page::List) || self.names.is_empty() {
            return;
        }
        let current = match self.focus {
            Target::Profile(i) => i,
            _ => self.first(),
        };
        let next = current
            .saturating_add_signed(direction * SHOWN as isize)
            .min(self.names.len() - 1);
        self.set_focus(Target::Profile(next));
    }

    pub fn move_focus(&mut self, x: isize, y: isize) {
        let target = match self.page {
            Page::List => {
                let count = self.names.len();
                let i = match self.focus {
                    Target::Profile(i) => i,
                    _ => count,
                };
                if y == 0 {
                    return;
                }
                let i = wrap(i, y, count + 1);
                if i == count {
                    Target::Create
                } else {
                    Target::Profile(i)
                }
            }
            Page::Name { .. } => {
                let keys = self.keys().len();
                let rows = keys.div_ceil(COLUMNS);
                let (row, col) = match self.focus {
                    Target::Key(i) => (i / COLUMNS, i % COLUMNS),
                    Target::Cancel => (rows, 0),
                    Target::Backspace => (rows, 1),
                    _ => (rows, 2),
                };
                let next = wrap(row, y, rows + 1);
                let col = if y != 0 && row == rows {
                    col * 5 + 1
                } else if y != 0 && next == rows {
                    col / 5
                } else {
                    col
                };
                if next == rows {
                    [Target::Cancel, Target::Backspace, Target::Accept][wrap(col, x, 3)]
                } else {
                    let width = COLUMNS.min(keys - next * COLUMNS);
                    Target::Key(next * COLUMNS + wrap(col.min(width - 1), x, width))
                }
            }
            Page::Difficulty(_) => {
                let i = if let Target::Difficulty(i) = self.focus {
                    i
                } else {
                    1
                };
                Target::Difficulty(wrap(i, x + y, DIFFICULTIES.len()))
            }
            Page::Delete(_) => {
                if (x + y) == 0 {
                    return;
                }
                if self.focus == Target::No {
                    Target::Yes
                } else {
                    Target::No
                }
            }
        };
        self.set_focus(target);
    }

    fn layout(&self) -> Vec<([f32; 4], Target, String)> {
        let mut rows = Vec::new();
        let mut push = |rect, target, label| rows.push((rect, target, label));
        let row = |i: usize| {
            let y = 100.0 + i as f32 * ROW_PITCH;
            [104.0, y, 408.0, y + ROW_H]
        };
        match &self.page {
            Page::List => {
                let [left, top, right, _] = LIST_WINDOW;
                let offset = self.view.offset();
                let list_row = |i: f32| {
                    let y = top + LIST_PAD + i * LIST_ROW;
                    [left + LIST_IN, y, right - LIST_IN, y + LIST_ROW]
                };
                for k in self.view.visible(self.names.len(), SHOWN) {
                    push(
                        list_row(k as f32 - offset),
                        Target::Profile(k),
                        self.names[k].1.clone(),
                    );
                }
                let arrow = |[_, y0, _, y1]: [f32; 4]| [415.0, y0, 449.0, y1];
                if offset > 0.01 {
                    push(arrow(list_row(0.0)), Target::Previous, "^".into());
                }
                if offset + (SHOWN as f32) < self.names.len() as f32 - 0.01 {
                    push(
                        arrow(list_row((SHOWN - 1) as f32)),
                        Target::Next,
                        "v".into(),
                    );
                }
                push(CREATE, Target::Create, "Create".into());
                if self.selected().is_some() {
                    push(
                        [369.0, 366.0, 497.0, 394.0],
                        Target::Delete,
                        "Delete".into(),
                    );
                }
            }
            Page::Name { .. } => {
                for (i, key) in self.keys().chars().enumerate() {
                    let x = 35.0 + (i % COLUMNS) as f32 * 34.0;
                    let y = 164.0 + (i / COLUMNS) as f32 * 34.0;
                    let label = if key == ' ' {
                        "_".into()
                    } else {
                        key.to_string()
                    };
                    push([x, y, x + 32.0, y + 28.0], Target::Key(i), label);
                }
                let symbols = if matches!(self.page, Page::Name { symbols: true, .. }) {
                    "Letters"
                } else {
                    "Symbols"
                };
                push(
                    [366.0, 366.0, 497.0, 394.0],
                    Target::Symbols,
                    symbols.into(),
                );
                for (i, (target, label)) in [
                    (Target::Cancel, "Cancel"),
                    (Target::Backspace, "Backspace"),
                    (Target::Accept, "Accept"),
                ]
                .into_iter()
                .enumerate()
                {
                    let x = 34.0 + i as f32 * 150.0;
                    push([x, 306.0, x + 144.0, 334.0], target, label.into());
                }
            }
            Page::Difficulty(_) => {
                for (i, label) in DIFFICULTIES.iter().enumerate() {
                    let [x0, y0, x1, y1] = row(i);
                    push(
                        [x0, y0 + 62.0, x1, y1 + 62.0],
                        Target::Difficulty(i),
                        (*label).into(),
                    );
                }
            }
            Page::Delete(_) => {
                push(row(4), Target::No, "No".into());
                push(row(5), Target::Yes, "Yes".into());
            }
        }
        rows
    }

    pub fn target(&self, point: [f32; 2], size: [f32; 2]) -> Option<Target> {
        if size.into_iter().any(|v| v <= 0.0) {
            return None;
        }
        let s = screen(size);
        let [x, y] = std::array::from_fn(|i| (point[i] - s.origin[i]) / s.scale[i]);
        self.layout()
            .into_iter()
            .find_map(|([x0, y0, x1, y1], target, _)| {
                // A name counts only inside the list's window.
                let window = LIST_WINDOW[1] + LIST_PAD;
                let shown = !matches!(target, Target::Profile(_))
                    || (window..=window + SHOWN as f32 * LIST_ROW).contains(&y);
                (shown && x >= x0 && x <= x1 && y >= y0 && y <= y1).then_some(target)
            })
    }
}

fn wrap(i: usize, direction: isize, count: usize) -> usize {
    (i as isize + direction).rem_euclid(count as isize) as usize
}

fn screen(size: [f32; 2]) -> Screen {
    let height = size[1].min(size[0] * 0.75);
    let width = height * 4.0 / 3.0;
    Screen {
        origin: [(size[0] - width) / 2.0, (size[1] - height) / 2.0],
        scale: [width / FRAME_W, height / FRAME_H],
    }
}

/// The left footer uses the same height and safe-area edge as its drawing.
pub fn back_at(point: [f32; 2], size: [f32; 2], picker: bool) -> bool {
    let height = if picker {
        size[1].min(size[0] * 0.75)
    } else {
        size[1]
    };
    let y = (size[1] - height) * 0.5 + HELP_Y * height / FRAME_H;
    let x = SAFE_X[0] * (height * 4.0 / 3.0) / FRAME_W;
    (x..=x * 4.8).contains(&point[0]) && (point[1] - y).abs() <= 14.0 * height / FRAME_H
}

pub(crate) fn draw(ctx: &mut Ctx<'_>, picker: &Picker, size: [f32; 2]) {
    let s = screen(size);
    let (title, left, right, top) = match picker.page {
        Page::List => ("Pick a Profile", 104.0, 408.0, 68.0),
        Page::Name { .. } => (
            picker.name_title.as_deref().unwrap_or("Enter profile name"),
            33.0,
            479.0,
            102.0,
        ),
        Page::Difficulty(_) => ("Select Difficulty", 104.0, 408.0, 122.0),
        Page::Delete(_) => ("Delete Profile", 104.0, 408.0, 122.0),
    };
    let bar = [left, top, right, top + ROW_H];
    ctx.bar(&s, bar, TITLE_PIECES);
    ctx.text_in(&s, &TITLE, title, bar, tint::TITLE);
    if let Page::Name { text, .. } = &picker.page {
        ctx.bordered(&s, NAME_BOX);
        ctx.bar(&s, NAME_FIELD, item_pieces(false));
        ctx.text_in(&s, &ITEM, text, NAME_FIELD, tint::SELECTED);
    }
    if let Page::Delete(id) = picker.page {
        let name = picker
            .names
            .iter()
            .find(|(key, _)| *key == id)
            .map_or("", |(_, name)| name.as_str());
        let line = [104.0, 166.0, 408.0, 166.0 + BODY.size];
        ctx.text_in(
            &s,
            &BODY,
            &format!("Delete {name}?"),
            line,
            tint::UNSELECTED,
        );
    }
    let list = matches!(picker.page, Page::List);
    if list {
        ctx.bordered(&s, LIST_WINDOW);
    }
    // The names scroll inside the window: cut them at its rows.
    let [_, wy0] = s.map(glam::Vec3::new(0.0, LIST_WINDOW[1] + LIST_PAD, 0.0));
    let [_, wy1] = s.map(glam::Vec3::new(
        0.0,
        LIST_WINDOW[1] + LIST_PAD + SHOWN as f32 * LIST_ROW,
        0.0,
    ));
    for (rect, target, text) in picker.layout() {
        let focused = target == picker.focus;
        let name = matches!(target, Target::Profile(_));
        ctx.clip = name.then_some([f32::MIN, wy0, f32::MAX, wy1]);
        let style = if focused {
            tint::SELECTED
        } else {
            tint::UNSELECTED
        };
        if (focused || !name) && !matches!(target, Target::Key(_)) {
            ctx.bar_tinted(
                &s,
                rect,
                item_pieces(matches!(
                    target,
                    Target::Create | Target::Difficulty(3) | Target::Yes
                )),
                ([255.0; 3], if focused { 0.9 } else { 0.65 }),
            );
        } else if focused {
            ctx.bordered(&s, rect);
        }
        let role = if matches!(
            target,
            Target::Key(_)
                | Target::Cancel
                | Target::Backspace
                | Target::Accept
                | Target::Symbols
                | Target::Delete
        ) {
            &BODY
        } else {
            &ITEM
        };
        let icon = match target {
            Target::Delete => Some(ctx.art.icons.sheet(Prompt::Done)),
            Target::Symbols if ctx.art.icons.set != IconSet::KeyboardMouse => {
                Some(pad_sheet(ctx.art.icons.set, PadButton::East))
            }
            _ => None,
        };
        let (x, align) = if let Some(icon) = icon {
            let middle = (rect[1] + rect[3]) * 0.5;
            ctx.sheet(
                &s,
                [rect[0] + 9.0, middle - 5.6, rect[0] + 18.6, middle + 5.6],
                [0.0, 0.0, 1.0, 1.0],
                icon,
                1.0,
            );
            (rect[0] + 25.0, HAlign::Left)
        } else if name {
            (rect[0] + 14.0, HAlign::Left)
        } else {
            ((rect[0] + rect[2]) * 0.5, HAlign::Center)
        };
        // Never smaller: a name wider than its row ends in an ellipsis.
        let margin = if icon.is_some() { 32.0 } else { 20.0 };
        let text = ctx.ellipsized(role, &text, rect[2] - rect[0] - margin + 2.0 * TEXT_INSET);
        let y = (rect[1] + rect[3] - role.size) * 0.5;
        ctx.text(&s, role, &text, (x, y), align, style);
    }
    ctx.clip = None;
    if let Some(message) = &picker.message {
        let line = [24.0, 343.0, 488.0, 343.0 + BODY.size];
        ctx.text_in(&s, &BODY, message, line, tint::TITLE);
    }
    let height = FRAME_H * s.scale[1];
    let bottom = s.origin[1] + height;
    ctx.help_bottom([size[0], height], false, Prompt::Back, "Back", bottom);
    ctx.help_bottom([size[0], height], true, Prompt::Accept, "Select", bottom);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn profile_picker_pages_to_last_used_and_shares_pointer_layout() {
        let mut pick = Picker::new(
            (0..13).map(|i| (i, format!("Player {i}"))).collect(),
            Some(12),
        );
        assert_eq!(pick.selected(), Some((12, "Player 12")));
        assert_eq!(pick.first(), 8);
        pick.page_list(-1);
        assert_eq!(pick.selected(), Some((7, "Player 7")));
        for size in [[640.0, 480.0], [1920.0, 1080.0], [600.0, 900.0]] {
            for ([left, top, right, bottom], target, _) in pick.layout() {
                let point =
                    screen(size).map(Vec3::new((left + right) / 2.0, (top + bottom) / 2.0, 0.0));
                assert_eq!(pick.target(point, size), Some(target));
            }
        }
        pick.set_focus(Target::Profile(0));
        pick.move_focus(0, -1);
        assert_eq!(pick.focus, Target::Create);
        pick.move_focus(0, 1);
        assert_eq!(pick.focus, Target::Profile(0));
    }

    /// The names sit inside the list window, clear of its border line (the
    /// line is drawn 0.5-2.5 units inside the window's edge), and Create
    /// sits below it.
    #[test]
    fn profile_rows_fit_inside_the_list_window() {
        let pick = Picker::new((0..7).map(|i| (i, format!("Player {i}"))).collect(), None);
        let [wx0, wy0, wx1, wy1] = LIST_WINDOW;
        let mut names = 0;
        for ([x0, y0, x1, y1], target, _) in pick.layout() {
            match target {
                Target::Profile(_) => {
                    names += 1;
                    for (side, gap) in [
                        ("left", x0 - wx0),
                        ("top", y0 - wy0),
                        ("right", wx1 - x1),
                        ("bottom", wy1 - y1),
                    ] {
                        assert!(gap >= 4.0, "{target:?}: {side} gap {gap}");
                    }
                }
                Target::Create => assert!(y0 >= wy1 + 4.0, "Create at {y0}"),
                _ => {}
            }
        }
        assert_eq!(names, SHOWN);
        // The name keyboard: the typed name's field and the keys' row of
        // buttons inside the box, the field above the first key row.
        let [bx0, by0, bx1, by1] = NAME_BOX;
        let [fx0, fy0, fx1, fy1] = NAME_FIELD;
        assert!(fx0 - bx0 >= 4.0 && fy0 - by0 >= 4.0 && bx1 - fx1 >= 4.0);
        let mut pick = pick;
        pick.edit_name("Player 1".into());
        for ([x0, y0, x1, y1], target, _) in pick.layout() {
            if matches!(target, Target::Symbols) {
                continue;
            }
            assert!(
                x0 - bx0 >= 4.0 && bx1 - x1 >= 4.0 && by1 - y1 >= 4.0,
                "{target:?}"
            );
            assert!(y0 >= fy1, "{target:?} under the name field");
        }
    }

    #[test]
    fn profile_keyboard_wraps_and_replaces_the_suggestion_only_once() {
        let mut pick = Picker::new(Vec::new(), None);
        pick.edit_name("Player 1".into());
        pick.move_focus(0, 1);
        assert_eq!(pick.focus, Target::Key(11));
        pick.type_name("A", 10);
        pick.toggle_symbols();
        pick.type_name("123456789more", 10);
        pick.backspace();
        assert!(
            matches!(&pick.page, Page::Name { text, symbols: true, .. } if text == "A12345678")
        );
        pick.resume_name("Ann".into());
        pick.type_name("a", 10);
        assert!(matches!(&pick.page, Page::Name { text, .. } if text == "Anna"));
    }
}
