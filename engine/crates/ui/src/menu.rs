//! Pause menu (BF2-style, controller-navigated): resume, confirmed Respawn, the session cheat
//! registry (PLAN §Cheats), the settings (aim-assist intensity slider,
//! unit-count/alive-AI cap slider, the FOV option, the music volume), quit. Pure egui over a
//! [`MenuModel`]; the game applies the returned [`MenuAction`]s.
//!
//! Navigation: D-pad up/down moves the cursor, Cross activates/toggles,
//! left/right adjusts sliders, Circle or Start resumes; on a keyboard the
//! arrows, Enter and Esc do the same, and the pointer clicks rows (egui).

use egui::{Align2, Color32, RichText, Vec2};

#[derive(Debug, Clone, Default)]
pub struct ClassPickerModel {
    pub tabs: Vec<String>,
    pub tab: usize,
    pub choices: Vec<(usize, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct MenuModel {
    pub class_picker_available: bool,
    pub class_choices: Option<ClassPickerModel>,
    /// Cheat registry rows: (id, label, enabled).
    pub cheats: Vec<(&'static str, &'static str, bool)>,
    pub assist_intensity: f32,
    /// Team size percentage (the historical action name is SetAiCap).
    pub team_size_pct: u32,
    pub reinforcements_pct: u32,
    /// The Team size / Reinforcements help (all-map averages), in that order.
    pub population_note: [String; 2],
    /// A battle without map team sizes (not a conquest from a mission): the
    /// Team size row would do nothing there, so it is not shown.
    pub team_size_off: bool,
    /// The FOV option (a multiplier on the PS2 camera's angle) and the
    /// vertical FOV it gives, in degrees, for the label.
    pub fov_scale: f32,
    pub fov_deg: f32,
    /// The music volume, 0..1 (1 = the disc record's own gain, 0 = off).
    pub music_volume: f32,
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    pub low_power_resolution: bool,
    /// The Button prompts row's value and the footer's names.
    pub input_icons_label: &'static str,
    pub icons: crate::input_icons::Icons,
    pub first_person: bool,
    pub cursor: usize,
    /// Pending pause-menu Respawn confirmation (re-spawn-flow §7).
    pub respawn_confirm: bool,
    /// None outside a live battle; caller may supply a localized label.
    pub respawn_label: Option<String>,
    /// Round result line, if the round is over.
    pub outcome_line: Option<String>,
    /// The shell's settings page (card BSHLA: Options → Game / Audio
    /// Options): titled by `title`, its first row "Back", no Quit row.
    pub in_shell: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction {
    OpenClassPicker,
    CloseClassPicker,
    SelectClass(usize),
    SetClassTab(usize),
    Resume,
    RequestRespawn,
    CancelRespawn,
    Respawn,
    ToggleCheat(usize),
    SetAssist(f32),
    /// Map-relative team size percentage, 10..=[`crate::population::MAX_TEAM_PCT`].
    SetAiCap(u32),
    SetReinforcements(u32),
    SetFov(f32),
    SetMusic(f32),
    SetMouseSensitivity(f32),
    ToggleInvertY,
    ToggleLowPowerResolution,
    /// The Button prompts row, a step on (+1) or back (-1) through its choices.
    CycleInputIcons(i32),
    ToggleViewpoint,
    Quit,
}

pub const MOUSE_SENSITIVITY_RANGE: (f32, f32) = (0.1, 5.0);

#[cfg(test)]
mod class_picker_tests;

/// Draw editable bindings and capture/conflict feedback. Navigation is owned by the caller.
pub fn draw_controls(
    ctx: &egui::Context,
    labels: &[(String, String)],
    cursor: usize,
    capture: bool,
    message: Option<&str>,
) -> Option<usize> {
    let mut selected = None;
    egui::Area::new(egui::Id::new("controls_menu"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(220))
                .inner_margin(egui::Margin::same(18))
                .show(ui, |ui| {
                    ui.heading("CONTROLS");
                    ui.label(if capture {
                        "Press a control · Esc / Start cancels"
                    } else {
                        "Select a row to change it · Back returns"
                    });
                    if let Some(message) = message {
                        ui.label(message);
                    }
                    let room = ctx.content_rect().height() - PANEL_MARGINS - ui.min_rect().height();
                    let gap = ui.spacing().item_spacing.y;
                    let list =
                        RowList::for_frame(ctx, (None, false), cursor, labels.len(), room, gap);
                    let shown = list.area(None, gap).show(ui, |ui| {
                        for (i, (name, value)) in labels.iter().enumerate() {
                            if row_label(
                                ui,
                                // The list cursor box is solid (menuref §9.1.9).
                                (i == cursor).then_some(1.0),
                                name.clone(),
                                Some(value.clone()),
                            )
                            .clicked_by(egui::PointerButton::Primary)
                            {
                                selected = Some(i);
                            }
                        }
                    });
                    list.remember(ctx, &shown, gap);
                });
        });
    selected
}

/// Controller edges for one frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub confirm: bool,
    pub back: bool,
}

/// Rows in cursor order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Row {
    ClassPicker,
    ClassBack,
    ClassChoice(usize),
    Resume,
    Respawn,
    No,
    Yes,
    Cheat(usize),
    Assist,
    AiCap,
    Reinforcements,
    Viewpoint,
    Fov,
    Music,
    MouseSensitivity,
    InvertY,
    LowPowerResolution,
    InputIcons,
    Quit,
}

pub fn rows(m: &MenuModel) -> Vec<Row> {
    if let Some(choices) = &m.class_choices {
        return std::iter::once(Row::ClassBack)
            .chain(choices.choices.iter().map(|(id, _)| Row::ClassChoice(*id)))
            .collect();
    }
    if m.respawn_confirm {
        return vec![Row::No, Row::Yes];
    }
    let mut v = vec![Row::Resume];
    if m.class_picker_available && m.in_shell.is_none() {
        v.push(Row::ClassPicker);
    }
    v.extend((0..m.cheats.len()).map(Row::Cheat));
    v.push(Row::Assist);
    if !m.team_size_off || m.in_shell.is_some() {
        v.push(Row::AiCap);
    }
    v.push(Row::Reinforcements);
    v.push(Row::Fov);
    v.push(Row::Music);
    v.push(Row::MouseSensitivity);
    v.push(Row::InvertY);
    v.push(Row::LowPowerResolution);
    v.push(Row::InputIcons);
    v.push(Row::Viewpoint);
    if m.in_shell.is_none() {
        if m.respawn_label.is_some() {
            v.push(Row::Respawn);
        }
        v.push(Row::Quit);
    }
    v
}

/// The Team size / Reinforcements rows as drawn: label and value, nothing
/// after the percentage.
pub fn population_row(m: &MenuModel, row: Row) -> Option<(&'static str, String)> {
    match row {
        Row::AiCap => Some(("Team size  ◀ ▶", format!("{}%", m.team_size_pct))),
        Row::Reinforcements => Some(("Reinforcements  ◀ ▶", format!("{}%", m.reinforcements_pct))),
        _ => None,
    }
}

/// The help under the rows for the focused row: Team size and
/// Reinforcements say what an average match looks like at their values. In a
/// battle a Reinforcements change applies from the next round. Button prompts
/// says what it is for.
pub fn row_help(m: &MenuModel, row: Row) -> Option<String> {
    match row {
        Row::AiCap => Some(m.population_note[0].clone()),
        Row::Reinforcements if m.in_shell.is_none() => Some(format!(
            "{}\nChanges apply from the next round.",
            m.population_note[1]
        )),
        Row::Reinforcements => Some(m.population_note[1].clone()),
        Row::InputIcons => Some(
            "Which button icons the game shows. Auto follows the controller you are using."
                .to_owned(),
        ),
        _ => None,
    }
}

/// Apply controller input to the cursor/values; returns actions to perform
/// and the new cursor.
pub fn navigate(m: &MenuModel, input: MenuInput) -> (usize, Vec<MenuAction>) {
    let rows = rows(m);
    let n = rows.len();
    let mut cursor = m.cursor.min(n - 1);
    let mut actions = Vec::new();
    if input.back {
        actions.push(if m.class_choices.is_some() {
            MenuAction::CloseClassPicker
        } else if m.respawn_confirm {
            MenuAction::CancelRespawn
        } else {
            MenuAction::Resume
        });
        return (cursor, actions);
    }
    if let Some(picker) = &m.class_choices {
        let n = picker.tabs.len();
        if n > 0 && (input.left || input.right) {
            let tab = if input.right {
                (picker.tab + 1) % n
            } else {
                (picker.tab + n - 1) % n
            };
            return (0, vec![MenuAction::SetClassTab(tab)]);
        }
    }
    if input.up {
        cursor = (cursor + n - 1) % n;
    }
    if input.down {
        cursor = (cursor + 1) % n;
    }
    if m.respawn_confirm && (input.left || input.right) {
        cursor = 1 - cursor;
    }
    match rows[cursor] {
        Row::ClassPicker if input.confirm => actions.push(MenuAction::OpenClassPicker),
        Row::ClassBack if input.confirm => actions.push(MenuAction::CloseClassPicker),
        Row::ClassChoice(i) if input.confirm => actions.push(MenuAction::SelectClass(i)),
        Row::Respawn if input.confirm => actions.push(MenuAction::RequestRespawn),
        Row::No if input.confirm => actions.push(MenuAction::CancelRespawn),
        Row::Yes if input.confirm => actions.push(MenuAction::Respawn),
        Row::Resume if input.confirm => actions.push(MenuAction::Resume),
        Row::Cheat(i) if input.confirm => actions.push(MenuAction::ToggleCheat(i)),
        Row::Assist if input.left || input.right => {
            let d = if input.right { 0.1 } else { -0.1 };
            actions.push(MenuAction::SetAssist(
                ((m.assist_intensity + d) * 10.0).round() / 10.0,
            ));
        }
        Row::AiCap if input.left || input.right => {
            let pct = if input.right {
                m.team_size_pct
                    .saturating_add(10)
                    .min(crate::population::MAX_TEAM_PCT)
            } else {
                m.team_size_pct.saturating_sub(10).max(10)
            };
            actions.push(MenuAction::SetAiCap(pct));
        }
        Row::Reinforcements if input.left || input.right => {
            let pct = if input.right {
                m.reinforcements_pct.saturating_add(10).min(500)
            } else {
                m.reinforcements_pct.saturating_sub(10).max(10)
            };
            actions.push(MenuAction::SetReinforcements(pct));
        }
        Row::Fov if input.left || input.right => {
            let d = if input.right { 0.05 } else { -0.05 };
            actions.push(MenuAction::SetFov(
                ((m.fov_scale + d) * 20.0).round() / 20.0,
            ));
        }
        Row::Music if input.left || input.right => {
            let d = if input.right { 0.1 } else { -0.1 };
            actions.push(MenuAction::SetMusic(
                ((m.music_volume + d) * 10.0).round().clamp(0.0, 10.0) / 10.0,
            ));
        }
        Row::MouseSensitivity if input.left || input.right => {
            let d = if input.right { 0.1 } else { -0.1 };
            let (lo, hi) = MOUSE_SENSITIVITY_RANGE;
            actions.push(MenuAction::SetMouseSensitivity(
                (((m.mouse_sensitivity + d) * 10.0).round() / 10.0).clamp(lo, hi),
            ));
        }
        Row::InvertY if input.confirm || input.left || input.right => {
            actions.push(MenuAction::ToggleInvertY)
        }
        Row::LowPowerResolution if input.confirm || input.left || input.right => {
            actions.push(MenuAction::ToggleLowPowerResolution)
        }
        // Cross steps on too, beside the left stick and A / D: a paused D-pad's
        // Left / Right are the Morse keys (`game::pause::rows_input`).
        Row::InputIcons if input.confirm || input.left || input.right => {
            actions.push(MenuAction::CycleInputIcons(if input.left { -1 } else { 1 }))
        }
        Row::Viewpoint if input.confirm || input.left || input.right => {
            actions.push(MenuAction::ToggleViewpoint)
        }
        Row::Quit if input.confirm => actions.push(MenuAction::Quit),
        _ => {}
    }
    (cursor, actions)
}

/// One row; `focus` is the focused row's cursor box alpha (0..1).
fn row_label(
    ui: &mut egui::Ui,
    focus: Option<f32>,
    text: String,
    value: Option<String>,
) -> egui::Response {
    let selected = focus.is_some();
    let bg = match focus {
        Some(a) => Color32::from_rgba_unmultiplied(76, 115, 178, (200.0 * a.clamp(0.0, 1.0)) as u8),
        None => Color32::TRANSPARENT,
    };
    let mut label = None;
    let row = egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .corner_radius(3.0)
        .show(ui, |ui| {
            ui.set_min_width(360.0);
            ui.horizontal(|ui| {
                label = Some(
                    ui.add(
                        egui::Label::new(RichText::new(text).size(18.0).color(if selected {
                            Color32::WHITE
                        } else {
                            Color32::LIGHT_GRAY
                        }))
                        .sense(egui::Sense::click()),
                    ),
                );
                if let Some(v) = value {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(v).size(18.0).color(Color32::WHITE).strong());
                    });
                }
            });
        });
    label
        .unwrap()
        .union(row.response.interact(egui::Sense::click()))
}

/// The height the panel's frame and the window's edge take from the row list.
const PANEL_MARGINS: f32 = 60.0;
/// The space under the rows for the footer line.
const FOOTER_ROOM: f32 = 30.0;
/// The space under the rows kept for the help: see [`row_help`].
const HELP_ROOM: f32 = 70.0;

/// The pause menu's scrolling rows, in whole rows: a list too long for the
/// window shows only rows that fit, so none is ever cut at its edge. The cursor
/// sits as near the middle as the list allows, and a mouse wheel's or the bar's
/// scroll settles on a row when it stops.
struct RowList {
    /// A row's height and the gap under it, measured from the last frame.
    pitch: f32,
    rows: usize,
    shown: usize,
    /// Set when the list moves this frame.
    offset: Option<f32>,
}

impl RowList {
    /// A first guess at [`Self::pitch`], for the first frame: 18-point text,
    /// a row's margins and the gap.
    const FIRST_PITCH: f32 = 36.0;
    /// How long a scroll must rest (seconds) before the list settles on a row:
    /// a slow trackpad's small steps add up instead of each being undone.
    const SETTLE_AFTER: f64 = 0.25;

    fn pitch_id() -> egui::Id {
        egui::Id::new("pause_rows_pitch")
    }

    fn state_id() -> egui::Id {
        egui::Id::new("pause_rows_state")
    }

    fn offset_id() -> egui::Id {
        egui::Id::new("pause_rows_offset")
    }

    fn scrolled_id() -> egui::Id {
        egui::Id::new("pause_rows_scrolled")
    }

    /// `room` is the height free for the list; at least one row shows.
    fn for_frame(
        ctx: &egui::Context,
        page: (Option<usize>, bool),
        cursor: usize,
        rows: usize,
        room: f32,
        gap: f32,
    ) -> Self {
        let pitch = ctx
            .data(|d| d.get_temp::<f32>(Self::pitch_id()))
            .unwrap_or(Self::FIRST_PITCH);
        let shown = (((room + gap) / pitch).floor() as usize).clamp(1, rows.max(1));
        let state = (page.0, page.1, cursor, shown);
        let moved = ctx.data_mut(|d| {
            let changed = d.get_temp(Self::state_id()) != Some(state);
            d.insert_temp(Self::state_id(), state);
            changed
        });
        let offset = if moved {
            Some(first_row(cursor, shown, rows) as f32 * pitch)
        } else {
            let (busy, now) = ctx.input(|i| {
                (
                    i.smooth_scroll_delta != Vec2::ZERO || i.pointer.any_down(),
                    i.time,
                )
            });
            if busy {
                ctx.data_mut(|d| d.insert_temp(Self::scrolled_id(), now));
            }
            let rested = ctx
                .data(|d| d.get_temp::<f64>(Self::scrolled_id()))
                .is_none_or(|at| now - at >= Self::SETTLE_AFTER);
            ctx.data(|d| d.get_temp::<f32>(Self::offset_id()))
                .map(|at| (at / pitch).round() * pitch)
                .filter(|_| !busy && rested)
        };
        Self {
            pitch,
            rows,
            shown,
            offset,
        }
    }

    fn area(&self, tab: Option<usize>, gap: f32) -> egui::ScrollArea {
        // At least the whole rows too: a panel's first size is egui's default
        // area (400 points), and a list asked only for "at most" stays inside it.
        let height = self.shown as f32 * self.pitch - gap;
        let area = egui::ScrollArea::vertical()
            .id_salt(("pause_rows", tab))
            .max_height(height)
            .min_scrolled_height(height);
        match self.offset {
            Some(at) => area.vertical_scroll_offset(at),
            None => area,
        }
    }

    /// What the next frame needs from this one's list.
    fn remember<R>(
        &self,
        ctx: &egui::Context,
        out: &egui::scroll_area::ScrollAreaOutput<R>,
        gap: f32,
    ) {
        ctx.data_mut(|d| {
            d.insert_temp(Self::offset_id(), out.state.offset.y);
            if self.rows > 0 {
                d.insert_temp(
                    Self::pitch_id(),
                    (out.content_size.y + gap) / self.rows as f32,
                );
            }
        });
    }
}

/// The first row of a list of `rows` showing `shown`, with `cursor` as near the
/// middle as the ends allow.
fn first_row(cursor: usize, shown: usize, rows: usize) -> usize {
    cursor
        .saturating_sub(shown / 2)
        .min(rows.saturating_sub(shown))
}

/// Draw the menu; pointer clicks produce actions too.
pub fn draw_menu(ctx: &egui::Context, m: &MenuModel) -> Vec<MenuAction> {
    let mut actions = Vec::new();
    egui::Area::new(egui::Id::new("pause_menu"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(200))
                .inner_margin(egui::Margin::same(18))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(if m.class_choices.is_some() { "CHANGE CLASS / HERO" } else { m.in_shell.as_deref().unwrap_or("PAUSED") })
                                .size(28.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        if let Some(o) = &m.outcome_line {
                            ui.label(RichText::new(o).size(16.0).color(Color32::LIGHT_GRAY));
                        }
                    });
                    ui.add_space(8.0);
                    if m.respawn_confirm {
                        ui.label("Do you really wish to kill your current player?");
                    }
                    if m.class_choices.is_some() {
                        ui.label("Testing: switch in place with full health and equipment. Team stays the same.");
                        ui.label("Your next normal spawn returns to the battle roster.");
                    }
                    if let Some(picker) = &m.class_choices {
                        ui.set_max_width(760.0);
                        ui.horizontal_wrapped(|ui| {
                            for (tab, label) in picker.tabs.iter().enumerate() {
                                if ui.selectable_label(tab == picker.tab, label).clicked_by(egui::PointerButton::Primary) {
                                    actions.push(MenuAction::SetClassTab(tab));
                                }
                            }
                        });
                        if picker.choices.is_empty() { ui.label("No supported characters are available in this tab."); }
                    }
                    let rows = rows(m);
                    let has_help = rows.contains(&Row::Reinforcements) || rows.contains(&Row::InputIcons);
                    let room = ctx.content_rect().height()
                        - PANEL_MARGINS
                        - ui.min_rect().height()
                        - FOOTER_ROOM
                        - if has_help { HELP_ROOM } else { 0.0 };
                    let gap = ui.spacing().item_spacing.y;
                    let list = RowList::for_frame(
                        ctx,
                        (m.class_choices.as_ref().map(|p| p.tab), m.respawn_confirm),
                        m.cursor,
                        rows.len(),
                        room,
                        gap,
                    );
                    let shown = list
                        .area(m.class_choices.as_ref().map(|p| p.tab), gap)
                        .show(ui, |ui| {
                            for (i, row) in rows.iter().enumerate() {
                                let sel = (i == m.cursor).then_some(1.0);
                                let clicked = match row {
                                    Row::ClassPicker => row_label(ui, sel, "Change class / hero (testing)".into(), None),
                                    Row::ClassBack => row_label(ui, sel, "Back".into(), None),
                                    Row::ClassChoice(k) => row_label(ui, sel, m.class_choices.as_ref().unwrap().choices.iter().find(|(id, _)| id == k).unwrap().1.clone(), None),
                                    Row::Respawn => row_label(
                                        ui,
                                        sel,
                                        m.respawn_label.clone().unwrap_or_else(|| "Respawn".into()),
                                        None,
                                    ),
                                    Row::No => row_label(ui, sel, "No".into(), None),
                                    Row::Yes => row_label(ui, sel, "Yes".into(), None),
                                    Row::Resume => {
                                        let back = if m.in_shell.is_some() {
                                            "Back"
                                        } else {
                                            "Resume"
                                        };
                                        row_label(ui, sel, back.into(), None)
                                    }
                                    Row::Cheat(k) => {
                                        let (_, label, on) = m.cheats[*k];
                                        row_label(
                                            ui,
                                            sel,
                                            format!("Cheat: {label}"),
                                            Some(if on { "ON".into() } else { "off".into() }),
                                        )
                                    }
                                    Row::Assist => row_label(
                                        ui,
                                        sel,
                                        "Aim assist intensity  ◀ ▶".into(),
                                        Some(format!("{:.1}", m.assist_intensity)),
                                    ),
                                    Row::AiCap | Row::Reinforcements => {
                                        let (label, value) = population_row(m, *row).unwrap();
                                        row_label(ui, sel, label.into(), Some(value))
                                    }
                                    Row::InputIcons => row_label(
                                        ui,
                                        sel,
                                        "Button prompts  ◀ ▶".into(),
                                        Some(m.input_icons_label.to_owned()),
                                    ),
                                    Row::Viewpoint => row_label(
                                        ui,
                                        sel,
                                        "Viewpoint".into(),
                                        Some(
                                            if m.first_person {
                                                "1st Person"
                                            } else {
                                                "3rd Person"
                                            }
                                            .into(),
                                        ),
                                    ),
                                    Row::Fov => row_label(
                                        ui,
                                        sel,
                                        "Field of view  ◀ ▶".into(),
                                        Some(format!("{:.2}× ({:.0}°)", m.fov_scale, m.fov_deg)),
                                    ),
                                    Row::Music => row_label(
                                        ui,
                                        sel,
                                        "Music volume  ◀ ▶".into(),
                                        Some(if m.music_volume <= 0.0 {
                                            "off".into()
                                        } else {
                                            format!("{:.0}%", m.music_volume * 100.0)
                                        }),
                                    ),
                                    Row::MouseSensitivity => row_label(
                                        ui,
                                        sel,
                                        "Mouse sensitivity  ◀ ▶".into(),
                                        Some(format!("{:.1}", m.mouse_sensitivity)),
                                    ),
                                    Row::InvertY => row_label(
                                        ui,
                                        sel,
                                        "Invert mouse Y".into(),
                                        Some(if m.invert_y { "ON".into() } else { "off".into() }),
                                    ),
                                    Row::LowPowerResolution => row_label(
                                        ui,
                                        sel,
                                        "Low power resolution".into(),
                                        Some(if m.low_power_resolution {
                                            "ON".into()
                                        } else {
                                            "off".into()
                                        }),
                                    ),
                                    Row::Quit => row_label(ui, sel, "Quit to desktop".into(), None),
                                };
                                if clicked.clicked_by(egui::PointerButton::Primary) {
                                    match row {
                                        Row::ClassPicker => actions.push(MenuAction::OpenClassPicker),
                                        Row::ClassBack => actions.push(MenuAction::CloseClassPicker),
                                        Row::ClassChoice(k) => actions.push(MenuAction::SelectClass(*k)),
                                        Row::Respawn => actions.push(MenuAction::RequestRespawn),
                                        Row::No => actions.push(MenuAction::CancelRespawn),
                                        Row::Yes => actions.push(MenuAction::Respawn),
                                        Row::Resume => actions.push(MenuAction::Resume),
                                        Row::Cheat(k) => actions.push(MenuAction::ToggleCheat(*k)),
                                        Row::InvertY => actions.push(MenuAction::ToggleInvertY),
                                        Row::LowPowerResolution => {
                                            actions.push(MenuAction::ToggleLowPowerResolution)
                                        }
                                        Row::InputIcons => {
                                            actions.push(MenuAction::CycleInputIcons(1))
                                        }
                                        Row::Viewpoint => actions.push(MenuAction::ToggleViewpoint),
                                        Row::Quit => actions.push(MenuAction::Quit),
                                        _ => {}
                                    }
                                }
                            }
                        });
                    list.remember(ctx, &shown, gap);
                    // Room is kept for the help so the centered menu does not
                    // jump as the cursor reaches those rows.
                    if has_help {
                        ui.add_space(6.0);
                        ui.scope(|ui| {
                            ui.set_min_height(60.0);
                            if let Some(help) = rows.get(m.cursor).and_then(|r| row_help(m, *r)) {
                                ui.label(RichText::new(help).size(15.0).color(Color32::LIGHT_GRAY));
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(if m.class_choices.is_some() {
                            "Up/Down choose · Left/Right tabs · Enter select · Esc back".to_owned()
                        } else {
                            m.icons.menu_footer(m.in_shell.is_none())
                        })
                            .size(12.0)
                            .color(Color32::GRAY),
                    );
                });
        });
    actions
}

/// PS2 BF2 pause-menu cheat codes: Morse code typed on the D-pad while paused
/// (engine `FUN_0038c1d8`; tables at 0x5ff100/0x5ff220 — lead-verified against
/// the raw ELF 2026-08-30, see `analysis/re-cheat-codes.md`). Up = dot,
/// Down = dash, Left = end of letter, Right = submit word. Rising edges only,
/// one symbol per frame, priority Up > Down > Left > Right. The letter buffer
/// wraps at 6 symbols (`% 6`), the word at 32 chars (`& 0x1f`); nothing times
/// out and nothing resets on resume or level change — a half-typed word
/// survives until the next submit. Unrecognised letters are dropped,
/// unrecognised words reset silently. BF1 has no such system.
#[derive(Debug, Clone)]
pub struct MorseCheats {
    letter: [u8; 8],
    mlen: usize,
    word: [u8; 33],
    wlen: usize,
}

impl Default for MorseCheats {
    fn default() -> Self {
        Self::new()
    }
}

/// ITU Morse for a–z, 0–9 in the engine's table order (0x5ff100).
pub const MORSE: [(u8, &str); 36] = [
    (b'a', ".-"),
    (b'b', "-..."),
    (b'c', "-.-."),
    (b'd', "-.."),
    (b'e', "."),
    (b'f', "..-."),
    (b'g', "--."),
    (b'h', "...."),
    (b'i', ".."),
    (b'j', ".---"),
    (b'k', "-.-"),
    (b'l', ".-.."),
    (b'm', "--"),
    (b'n', "-."),
    (b'o', "---"),
    (b'p', ".--."),
    (b'q', "--.-"),
    (b'r', ".-."),
    (b's', "..."),
    (b't', "-"),
    (b'u', "..-"),
    (b'v', "...-"),
    (b'w', ".--"),
    (b'x', "-..-"),
    (b'y', "-.--"),
    (b'z', "--.."),
    (b'0', "-----"),
    (b'1', ".----"),
    (b'2', "..---"),
    (b'3', "...--"),
    (b'4', "....-"),
    (b'5', "....."),
    (b'6', "-...."),
    (b'7', "--..."),
    (b'8', "---.."),
    (b'9', "----."),
];

/// The engine's 7-entry `{handler, hash}` table (0x5ff220) with the words
/// recovered by hashing: FNV-1a-32 over `byte | 0x20`.
pub const CHEAT_WORDS: [(&str, u32); 7] = [
    ("alias", 0x7d26_8157),
    ("sos", 0xcb36_196c),
    ("ammo", 0x5b9b_0daf),
    ("80s", 0xfe9e_d9d2),
    ("at", 0x5725_1588),
    ("hud", 0xe28d_61c6),
    ("slow", 0x6d95_42b8),
];

/// `[ours]` (card CHEAT2): openbf's extra words, matched the same way:
/// `clip` no reloading, `run` infinite stamina, `all` unlock all classes,
/// `cheat` all five at once.
pub const OUR_WORDS: [&str; 4] = ["clip", "run", "all", "cheat"];

/// FNV-1a-32 with the engine's `| 0x20` fold (`FUN_00539308`).
pub fn fnv1a_lower(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |h, &b| {
        (h ^ u32::from(b | 0x20)).wrapping_mul(0x0100_0193)
    })
}

fn cstr(buf: &[u8]) -> &[u8] {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    &buf[..end]
}

impl MorseCheats {
    pub fn new() -> Self {
        Self {
            letter: [0; 8],
            mlen: 0,
            word: [0; 33],
            wlen: 0,
        }
    }

    /// Feed one frame of D-pad rising edges; returns the recognised word when
    /// Right submits a known one.
    pub fn feed(&mut self, input: MenuInput) -> Option<&'static str> {
        let mut hit = None;
        if input.up {
            self.push(b'.');
        } else if input.down {
            self.push(b'-');
        } else if input.left {
            // FUN_0038c1c0: terminate + reset, then strncmp(code, morse, 6)
            // over the 36 entries; the first match appends its letter.
            self.letter[self.mlen] = 0;
            self.mlen = 0;
            let code = cstr(&self.letter);
            if let Some((ch, _)) = MORSE.iter().find(|(_, m)| m.as_bytes() == code) {
                self.word[self.wlen] = *ch;
                self.wlen += 1;
            }
        } else if input.right {
            self.word[self.wlen] = 0;
            self.wlen = 0;
            let h = fnv1a_lower(cstr(&self.word));
            hit = CHEAT_WORDS
                .iter()
                .find(|(_, wh)| *wh == h)
                .map(|(w, _)| *w)
                .or_else(|| {
                    OUR_WORDS
                        .into_iter()
                        .find(|w| fnv1a_lower(w.as_bytes()) == h)
                });
            self.letter[self.mlen] = 0;
            self.mlen = 0;
        }
        self.wlen &= 0x1f;
        self.mlen %= 6;
        hit
    }

    fn push(&mut self, sym: u8) {
        self.letter[self.mlen] = sym;
        self.mlen += 1;
    }

    /// (letter symbols so far, word so far) — for tests and debug overlays.
    pub fn pending(&self) -> (String, String) {
        let l = String::from_utf8_lossy(&self.letter[..self.mlen]).into_owned();
        let w = String::from_utf8_lossy(&self.word[..self.wlen]).into_owned();
        (l, w)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod morse_tests;

#[cfg(test)]
mod pointer_navigation_tests;
