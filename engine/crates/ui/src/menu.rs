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
    /// The Input icons row's value and the footer's names.
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
    /// The Input icons row, a step on (+1) or back (-1) through its choices.
    CycleInputIcons(i32),
    ToggleViewpoint,
    Quit,
}

pub const MOUSE_SENSITIVITY_RANGE: (f32, f32) = (0.1, 5.0);

#[cfg(test)]
mod class_picker_tests {
    use super::*;

    #[test]
    fn class_picker_navigation_and_back_stay_inside_pause() {
        let model = MenuModel {
            class_picker_available: true,
            cursor: 1,
            ..Default::default()
        };
        let enter = MenuInput {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(navigate(&model, enter).1, vec![MenuAction::OpenClassPicker]);
        let picker = MenuModel {
            class_choices: Some(ClassPickerModel {
                tabs: vec!["BF1".into(), "BF2".into()],
                tab: 0,
                choices: vec![(3, "Trooper".into()), (9, "Hero".into())],
            }),
            cursor: 2,
            ..model
        };
        assert_eq!(navigate(&picker, enter).1, vec![MenuAction::SelectClass(9)]);
        assert_eq!(
            navigate(
                &picker,
                MenuInput {
                    left: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::SetClassTab(1)]
        );
        assert_eq!(
            navigate(
                &picker,
                MenuInput {
                    right: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::SetClassTab(1)]
        );
        let back = MenuInput {
            back: true,
            ..Default::default()
        };
        assert_eq!(
            navigate(&picker, back).1,
            vec![MenuAction::CloseClassPicker]
        );
        assert_eq!(
            navigate(
                &picker,
                MenuInput {
                    down: true,
                    ..Default::default()
                }
            )
            .0,
            0
        );
        assert!(!rows(&MenuModel::default()).contains(&Row::ClassPicker));
    }
}

/// Draw editable bindings and capture/conflict feedback. Navigation is owned by the caller.
pub fn draw_controls(
    ctx: &egui::Context,
    labels: &[(String, String)],
    cursor: usize,
    capture: bool,
    message: Option<&str>,
) -> Option<usize> {
    let mut selected = None;
    let cursor_id = egui::Id::new("controls_last_cursor");
    let changed = ctx.data_mut(|data| {
        let changed = data.get_temp::<usize>(cursor_id) != Some(cursor);
        data.insert_temp(cursor_id, cursor);
        changed
    });
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
                    egui::ScrollArea::vertical()
                        .max_height(500.0)
                        .show(ui, |ui| {
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
                                if changed && i == cursor {
                                    ui.scroll_to_cursor(Some(egui::Align::Center));
                                }
                            }
                        });
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
/// battle a Reinforcements change applies from the next round.
pub fn row_help(m: &MenuModel, row: Row) -> Option<String> {
    match row {
        Row::AiCap => Some(m.population_note[0].clone()),
        Row::Reinforcements if m.in_shell.is_none() => Some(format!(
            "{}\nChanges apply from the next round.",
            m.population_note[1]
        )),
        Row::Reinforcements => Some(m.population_note[1].clone()),
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
        // Cross steps on too: a paused frame's Left / Right are the Morse
        // keys (`game::pause::rows_input`), so a pad needs another way round.
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

/// Draw the menu; pointer clicks produce actions too.
pub fn draw_menu(ctx: &egui::Context, m: &MenuModel) -> Vec<MenuAction> {
    let mut actions = Vec::new();
    let cursor_changed = ctx.data_mut(|data| {
        let id = egui::Id::new("pause_last_cursor");
        let state = (
            m.class_choices.as_ref().map(|p| p.tab),
            m.respawn_confirm,
            m.cursor,
        );
        let changed = data.get_temp::<(Option<usize>, bool, usize)>(id) != Some(state);
        data.insert_temp(id, state);
        changed
    });
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
                    egui::ScrollArea::vertical()
                        .id_salt(("pause_rows", m.class_choices.as_ref().map(|p| p.tab)))
                        .max_height((ctx.content_rect().height() - 220.0).max(100.0))
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
                                        "Input icons  ◀ ▶".into(),
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
                                if cursor_changed && i == m.cursor {
                                    clicked.scroll_to_me(Some(egui::Align::Center));
                                }
                            }
                        });
                    // Room is kept for the help so the centered menu does not
                    // jump as the cursor reaches those rows.
                    if rows.contains(&Row::Reinforcements) {
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
                            m.icons.menu_footer()
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
mod tests {
    use super::*;

    fn model() -> MenuModel {
        MenuModel {
            class_picker_available: false,
            class_choices: None,
            cheats: vec![
                ("invincible", "Invincibility", false),
                ("ammo", "Infinite ammo", true),
            ],
            assist_intensity: 1.0,
            team_size_pct: 100,
            reinforcements_pct: 100,
            population_note: Default::default(),
            team_size_off: false,
            fov_scale: 1.0,
            fov_deg: 37.7,
            music_volume: 1.0,
            mouse_sensitivity: 1.0,
            invert_y: false,
            low_power_resolution: false,
            input_icons_label: "Auto",
            icons: Default::default(),
            first_person: false,
            cursor: 0,
            respawn_confirm: false,
            respawn_label: Some("Respawn".into()),
            outcome_line: None,
            in_shell: None,
        }
    }

    /// Both menus carry the row. Cross steps on, as a paused pad has no
    /// Left / Right for rows; Left steps back.
    #[test]
    fn the_input_icons_row_steps_its_choices() {
        let mut m = model();
        let at = |m: &MenuModel| rows(m).iter().position(|r| *r == Row::InputIcons);
        m.cursor = at(&m).unwrap();
        let press = |m: &MenuModel, input| navigate(m, input).1;
        let confirm = MenuInput {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(press(&m, confirm), vec![MenuAction::CycleInputIcons(1)]);
        let left = MenuInput {
            left: true,
            ..Default::default()
        };
        assert_eq!(press(&m, left), vec![MenuAction::CycleInputIcons(-1)]);
        let right = MenuInput {
            right: true,
            ..Default::default()
        };
        assert_eq!(press(&m, right), vec![MenuAction::CycleInputIcons(1)]);
        m.in_shell = Some("Game Options".into());
        assert!(at(&m).is_some());
    }

    #[test]
    fn team_size_row_only_where_it_acts_and_rows_explain_averages() {
        let mut m = model();
        m.population_note = ["A\nL".into(), "R\nL".into()];
        m.reinforcements_pct = 150;
        assert!(rows(&m).contains(&Row::AiCap));
        assert_eq!(row_help(&m, Row::AiCap).as_deref(), Some("A\nL"));
        assert_eq!(
            row_help(&m, Row::Reinforcements).as_deref(),
            Some("R\nL\nChanges apply from the next round.")
        );
        assert_eq!(row_help(&m, Row::Fov), None);
        assert_eq!(
            population_row(&m, Row::AiCap),
            Some(("Team size  ◀ ▶", "100%".into()))
        );
        assert_eq!(
            population_row(&m, Row::Reinforcements),
            Some(("Reinforcements  ◀ ▶", "150%".into()))
        );
        m.team_size_off = true;
        assert!(!rows(&m).contains(&Row::AiCap));
        assert!(rows(&m).contains(&Row::Reinforcements));
        // The shell's settings page sets the next battles: both rows.
        m.in_shell = Some("Game Options".into());
        assert!(rows(&m).contains(&Row::AiCap));
        assert_eq!(row_help(&m, Row::Reinforcements).as_deref(), Some("R\nL"));
    }

    #[test]
    fn the_mouse_rows_step_the_sensitivity_and_toggle_invert() {
        let mut m = model();
        let at = |m: &MenuModel, r: Row| rows(m).iter().position(|x| *x == r).unwrap();
        m.cursor = at(&m, Row::MouseSensitivity);
        let go = |m: &MenuModel, input: MenuInput| navigate(m, input).1;
        let right = MenuInput {
            right: true,
            ..Default::default()
        };
        let left = MenuInput {
            left: true,
            ..Default::default()
        };
        assert_eq!(go(&m, right), vec![MenuAction::SetMouseSensitivity(1.1)]);
        m.mouse_sensitivity = 0.1;
        assert_eq!(go(&m, left), vec![MenuAction::SetMouseSensitivity(0.1)]);
        m.mouse_sensitivity = 5.0;
        assert_eq!(go(&m, right), vec![MenuAction::SetMouseSensitivity(5.0)]);
        m.cursor = at(&m, Row::InvertY);
        let confirm = MenuInput {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(go(&m, confirm), vec![MenuAction::ToggleInvertY]);
        m.cursor = at(&m, Row::LowPowerResolution);
        assert_eq!(go(&m, confirm), vec![MenuAction::ToggleLowPowerResolution]);
        assert_eq!(rows(&m).last(), Some(&Row::Quit));
    }

    #[test]
    fn respawn_confirmation_navigation_and_shell_exclusion() {
        let mut m = model();
        m.cursor = rows(&m).iter().position(|r| *r == Row::Respawn).unwrap();
        assert_eq!(
            navigate(
                &m,
                MenuInput {
                    confirm: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::RequestRespawn]
        );
        m.respawn_confirm = true;
        m.cursor = 0;
        assert_eq!(rows(&m), vec![Row::No, Row::Yes]);
        assert_eq!(
            navigate(
                &m,
                MenuInput {
                    confirm: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::CancelRespawn]
        );
        assert_eq!(
            navigate(
                &m,
                MenuInput {
                    down: true,
                    confirm: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::Respawn]
        );
        assert_eq!(
            navigate(
                &m,
                MenuInput {
                    right: true,
                    confirm: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::Respawn]
        );
        m.cursor = 1;
        assert_eq!(
            navigate(
                &m,
                MenuInput {
                    back: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::CancelRespawn]
        );
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw_menu(ctx, &m);
        });
        m.respawn_confirm = false;
        m.in_shell = Some("Options".into());
        assert!(!rows(&m).contains(&Row::Respawn));
        m.in_shell = None;
        m.respawn_label = None;
        assert!(!rows(&m).contains(&Row::Respawn));
    }

    #[test]
    fn navigation_wraps_toggles_and_adjusts() {
        let mut m = model();
        // Down twice → second cheat; confirm toggles it.
        let (c, a) = navigate(
            &m,
            MenuInput {
                down: true,
                ..Default::default()
            },
        );
        m.cursor = c;
        let (c, a2) = navigate(
            &m,
            MenuInput {
                down: true,
                confirm: true,
                ..Default::default()
            },
        );
        assert!(a.is_empty());
        assert_eq!((c, a2), (2, vec![MenuAction::ToggleCheat(1)]));
        // Up from the top wraps to Quit.
        m.cursor = 0;
        let (c, _) = navigate(
            &m,
            MenuInput {
                up: true,
                ..Default::default()
            },
        );
        assert_eq!(rows(&m)[c], Row::Quit);
        // Slider rows adjust with left/right and clamp.
        m.cursor = 3; // Assist
        let (_, a) = navigate(
            &m,
            MenuInput {
                right: true,
                ..Default::default()
            },
        );
        assert_eq!(a, vec![MenuAction::SetAssist(1.1)]);
        m.cursor = 4; // AiCap
        m.team_size_pct = 10;
        let (_, a) = navigate(
            &m,
            MenuInput {
                left: true,
                ..Default::default()
            },
        );
        assert_eq!(a, vec![MenuAction::SetAiCap(10)]);
        m.cursor = 6; // Fov
        assert_eq!(rows(&m)[6], Row::Fov);
        let (_, a) = navigate(
            &m,
            MenuInput {
                right: true,
                ..Default::default()
            },
        );
        assert_eq!(a, vec![MenuAction::SetFov(1.05)]);
        // The music volume: tenths, held in 0..1.
        m.cursor = 7;
        assert_eq!(rows(&m)[7], Row::Music);
        let step = |m: &MenuModel, right: bool| {
            let input = MenuInput {
                left: !right,
                right,
                ..Default::default()
            };
            navigate(m, input).1
        };
        assert_eq!(step(&m, true), vec![MenuAction::SetMusic(1.0)]);
        assert_eq!(step(&m, false), vec![MenuAction::SetMusic(0.9)]);
        m.music_volume = 0.1;
        assert_eq!(step(&m, false), vec![MenuAction::SetMusic(0.0)]);
        m.music_volume = 0.0;
        assert_eq!(step(&m, false), vec![MenuAction::SetMusic(0.0)]);
        // Back always resumes.
        let (_, a) = navigate(
            &m,
            MenuInput {
                back: true,
                ..Default::default()
            },
        );
        assert_eq!(a, vec![MenuAction::Resume]);
        // Draws headless.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw_menu(ctx, &m);
        });
    }
}

#[cfg(test)]
mod morse_tests {
    use super::*;

    fn edge(up: bool, down: bool, left: bool, right: bool) -> MenuInput {
        MenuInput {
            up,
            down,
            left,
            right,
            confirm: false,
            back: false,
        }
    }

    /// Type a word the PS2 way: symbols, Left after each letter, Right at the end.
    fn type_word(m: &mut MorseCheats, word: &str) -> Option<&'static str> {
        let mut hit = None;
        for ch in word.bytes() {
            let code = MORSE.iter().find(|(c, _)| *c == ch).unwrap().1;
            for sym in code.bytes() {
                assert!(m
                    .feed(edge(sym == b'.', sym == b'-', false, false))
                    .is_none());
            }
            assert!(m.feed(edge(false, false, true, false)).is_none());
        }
        hit = hit.or(m.feed(edge(false, false, false, true)));
        hit
    }

    #[test]
    fn all_seven_words_recognised() {
        let mut m = MorseCheats::new();
        for (w, h) in CHEAT_WORDS {
            assert_eq!(fnv1a_lower(w.as_bytes()), h, "{w}");
            assert_eq!(type_word(&mut m, w), Some(w));
            assert_eq!(m.pending(), (String::new(), String::new()));
        }
    }

    /// `[ours]` clip, run, all: recognised, and no PS2 hash collides.
    #[test]
    fn our_words_recognised_without_collisions() {
        let mut m = MorseCheats::new();
        for w in OUR_WORDS {
            let h = fnv1a_lower(w.as_bytes());
            assert!(CHEAT_WORDS.iter().all(|(_, ps2)| *ps2 != h), "{w}");
            assert_eq!(type_word(&mut m, w), Some(w));
        }
    }

    #[test]
    fn unknown_word_resets_silently_then_next_word_works() {
        let mut m = MorseCheats::new();
        assert_eq!(type_word(&mut m, "sso"), None);
        assert_eq!(type_word(&mut m, "sos"), Some("sos"));
    }

    #[test]
    fn unrecognised_letter_is_dropped_but_word_continues() {
        let mut m = MorseCheats::new();
        // "..-.-" is no Morse letter: dropped, the word buffer is untouched.
        for sym in "..-.-".bytes() {
            m.feed(edge(sym == b'.', sym == b'-', false, false));
        }
        m.feed(edge(false, false, true, false));
        assert_eq!(m.pending().1, "");
        assert_eq!(type_word(&mut m, "at"), Some("at"));
    }

    #[test]
    fn letter_buffer_wraps_at_six_symbols() {
        // Six dots wrap mlen to 0; a seventh symbol overwrites slot 0 and the
        // commit sees a one-symbol string: '-' -> 't'.
        let mut m = MorseCheats::new();
        for _ in 0..6 {
            m.feed(edge(true, false, false, false));
        }
        assert_eq!(m.pending().0, "");
        m.feed(edge(false, true, false, false));
        m.feed(edge(false, false, true, false));
        assert_eq!(m.pending().1, "t");
    }

    #[test]
    fn priority_up_over_down_and_state_persists_across_idle_frames() {
        let mut m = MorseCheats::new();
        m.feed(edge(true, true, false, false));
        assert_eq!(m.pending().0, ".");
        for _ in 0..1000 {
            m.feed(edge(false, false, false, false));
        }
        assert_eq!(m.pending().0, ".");
        m.feed(edge(true, false, false, false));
        m.feed(edge(true, false, false, false));
        m.feed(edge(false, false, true, false));
        assert_eq!(m.pending(), (String::new(), "s".into()));
    }
}

#[cfg(test)]
mod pointer_navigation_tests {
    use super::*;

    #[test]
    fn tab_focus_then_enter_does_not_dispatch_a_second_menu_action() {
        let ctx = egui::Context::default();
        let mut model = MenuModel::default();
        model.cursor = rows(&model)
            .iter()
            .position(|r| *r == Row::InvertY)
            .unwrap();
        let frame = |events| {
            let mut actions = Vec::new();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 720.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| actions = draw_menu(ctx, &model),
            );
            actions
        };
        frame(vec![]);
        frame(vec![]);
        let mut focused = false;
        for _ in 0..16 {
            frame(vec![key(egui::Key::Tab, true)]);
            frame(vec![key(egui::Key::Tab, false)]);
            focused |= ctx.memory(|m| m.focused().is_some());
            let overlay = frame(vec![key(egui::Key::Enter, true)]);
            assert!(
                overlay.is_empty(),
                "egui must not duplicate native navigation: {overlay:?}"
            );
            frame(vec![key(egui::Key::Enter, false)]);
            assert_eq!(
                navigate(
                    &model,
                    MenuInput {
                        confirm: true,
                        ..Default::default()
                    }
                )
                .1,
                vec![MenuAction::ToggleInvertY]
            );
        }
        assert!(focused, "exercise real egui Tab focus");
    }

    fn key(key: egui::Key, pressed: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn entire_row_accepts_pointer_including_binding_value_and_padding() {
        let ctx = egui::Context::default();
        let mut response = None;
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 720.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        response = Some(row_label(
                            ui,
                            Some(1.0),
                            "Invert mouse Y".into(),
                            Some("OFF".into()),
                        ));
                    });
                },
            );
            response.take().unwrap()
        };
        let first = frame(vec![]);
        // Far-right row padding/value, not just the action's text, accepts clicks.
        let pos = first.rect.right_center() - egui::vec2(4.0, 0.0);
        frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ]);
        let r = frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert!(r.clicked_by(egui::PointerButton::Primary));
    }
}
