//! Pause menu state and actions. Ground and space battles use the
//! BF2-style Game Menu, with the same settings pages in the shell and in
//! battle. The native presentation is in [`crate::pause_draw`]. Egui draws
//! the fallback when the baked menu art is unavailable.
//!
//! Navigation: D-pad up/down moves the cursor, Cross activates/toggles,
//! left/right adjusts sliders, Circle or Start resumes; on a keyboard the
//! arrows, Enter and Esc do the same. The pointer clicks rows, and a
//! value row's ◀ and ▶ step it as Left and Right do ([`step`]).

use egui::{Align2, Color32, RichText, Vec2};

#[derive(Debug, Clone, Default)]
pub struct ClassPickerModel {
    pub tabs: Vec<String>,
    pub tab: usize,
    pub choices: Vec<(usize, String)>,
}

/// A page of the ground Game Menu (`ifs_pausemenu` and the stack it opens,
/// analysis/re-bf2-menus.md "ifs_pausemenu").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PausePage {
    #[default]
    Main,
    Options,
    Settings(SettingsPage),
    /// `[ours]` The session cheat registry (PLAN §Cheats).
    Cheats,
}

mod settings;
pub use settings::{
    draw_fps_counter, rumble_level, ControlContext, FpsCounter, SettingsPage, RUMBLE_LEVELS,
};

/// A Yes/No popup over the menu; it opens on No (both games' pause menus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Popup {
    Respawn,
    Quit,
    Restart,
}

/// The Game Menu's words: the disc's text where the game has a table, else
/// these English defaults (the BF2 disc's own English).
#[derive(Debug, Clone)]
pub struct PauseTexts {
    pub title: String,
    pub resume: String,
    pub options: String,
    pub cheats: String,
    pub restart: String,
    pub quit: String,
    pub respawn_prompt: String,
    pub quit_prompt: String,
    pub restart_prompt: String,
}

impl Default for PauseTexts {
    fn default() -> Self {
        Self {
            title: "Game Menu".into(),
            resume: "Resume Game".into(),
            options: "Options".into(),
            cheats: "Cheats".into(),
            restart: "Restart Mission".into(),
            quit: "Quit".into(),
            respawn_prompt: "Do you really wish to kill your current player?".into(),
            quit_prompt:
                "Leaving the game now will end this session. Are you sure you want to leave?".into(),
            restart_prompt: "Do you really want to restart this battle?".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingRows {
    pub title: &'static str,
    pub labels: Vec<(String, String)>,
    pub capture: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MenuModel {
    pub class_picker_available: bool,
    pub bindings: Option<BindingRows>,
    pub class_choices: Option<ClassPickerModel>,
    /// Cheat registry rows: (id, label, enabled).
    pub cheats: Vec<(&'static str, &'static str, bool)>,
    pub assist_intensity: f32,
    /// Instant Action's 1-based Easy / Normal / Hard / Elite selector.
    pub difficulty: i32,
    pub effects_volume: f32,
    pub dialogue_volume: f32,
    pub fps_counter: FpsCounter,

    /// Team size percentage (the historical action name is SetAiCap).
    pub team_size_pct: u32,
    pub reinforcements_pct: u32,
    /// The Team size / Reinforcements help (all-map averages), in that order.
    pub population_note: [String; 2],
    /// A battle without map team sizes (not a conquest from a mission): the
    /// Team size row would do nothing there, so it is shown greyed with the
    /// reason ([`row_locked`]).
    pub team_size_off: bool,
    /// The FOV option (a multiplier on the PS2 camera's angle) and the
    /// vertical FOV it gives, in degrees, for the label.
    pub fov_scale: f32,
    pub fov_deg: f32,
    /// The music volume, 0..1 (1 = the disc record's own gain, 0 = off).
    pub music_volume: f32,
    pub rumble_strength: f32,
    pub rumble_on_fire: bool,
    pub career_awards_disabled: bool,
    pub mouse_sensitivity: f32,
    /// The right stick's look sensitivity ([`PAD_LOOK_SENSITIVITY_RANGE`]).
    pub pad_look_sensitivity: f32,
    pub rc_flight_controls: bool,
    pub invert_y: bool,
    pub low_power_resolution: bool,
    pub terrain_lod: bool,
    pub scene_softness: bool,
    /// The Button prompts row's value and the footer's names.
    pub input_icons_label: &'static str,
    pub icons: crate::input_icons::Icons,
    pub first_person: bool,
    pub cursor: usize,
    /// The Game Menu page shown. The shell's settings use a flat list.
    pub page: Option<PausePage>,
    /// The Yes/No popup open over the menu (Respawn: re-spawn-flow §7).
    pub popup: Option<Popup>,
    /// Restart Mission is offered (a battle launched from the shell).
    pub restart_available: bool,
    pub texts: PauseTexts,
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
    /// No on a popup.
    CancelPopup,
    Respawn,
    OpenPage(PausePage),
    /// A sub-page's Back row: to the Game Menu, on the row that opened it.
    ClosePage,
    /// The Game Menu's Quit and Restart rows: they ask first.
    RequestQuit,
    RequestRestart,
    /// Restart confirmed: the same battle again from its loading screen.
    Restart,
    ToggleCheat(usize),
    SetAssist(f32),
    SetDifficulty(i32),
    SetEffectsVolume(f32),
    SetDialogueVolume(f32),
    SetFpsCounter(FpsCounter),
    EditBindings(SettingsPage, ControlContext),
    /// Map-relative team size percentage, 10..=[`crate::population::MAX_TEAM_PCT`].
    SetAiCap(u32),
    SetReinforcements(u32),
    SetFov(f32),
    SetMusic(f32),
    SetRumble(f32),
    ToggleRumbleOnFire,
    SetCareerAwards(bool),
    SetMouseSensitivity(f32),
    SetPadLookSensitivity(f32),
    ToggleFlightControls,
    ToggleInvertY,
    ToggleLowPowerResolution,
    ToggleTerrainLod,
    ToggleSceneSoftness,
    /// The Button prompts row, a step on (+1) or back (-1) through its choices.
    CycleInputIcons(i32),
    ToggleViewpoint,
    Quit,
}

pub const MOUSE_SENSITIVITY_RANGE: (f32, f32) = (0.1, 5.0);

/// OURS: the controller look sensitivity, a factor on the right stick before
/// the sim reads it. 1.0 is the stick as it was (`core::sim::LOOK_SCALE`); from
/// 1.25 up a full push already turns at the class's top rate, so higher values
/// only reach it earlier in the stick's travel. measure: the maintainer's pad
/// feel check at both ends, BF2 rifleman on foot and a hover tank's turret.
pub const PAD_LOOK_SENSITIVITY_RANGE: (f32, f32) = (0.5, 1.5);

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
                        "Press and release a control · Hold to cancel"
                    } else {
                        "Select a row to change it · Back returns"
                    });
                    if let Some(message) = message {
                        ui.label(message);
                    }
                    let room = ctx.content_rect().height() - PANEL_MARGINS - ui.min_rect().height();
                    let gap = ui.spacing().item_spacing.y;
                    let list = RowList::for_frame(ctx, (None, 0), cursor, labels.len(), room, gap);
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
    Binding(usize),
    ClassBack,
    ClassChoice(usize),
    Resume,
    Respawn,
    No,
    Yes,
    Cheat(usize),
    Settings(SettingsPage),
    EditBindings(ControlContext),
    Difficulty,
    EffectsVolume,
    DialogueVolume,
    GraphicsQuality,
    FpsCounter,
    Assist,
    AiCap,
    Reinforcements,
    Viewpoint,
    Fov,
    Music,
    Rumble,
    RumbleOnFire,
    CareerAwards,
    MouseSensitivity,
    PadLookSensitivity,
    FlightControls,
    InvertY,
    LowPowerResolution,
    TerrainLod,
    SceneSoftness,
    InputIcons,
    Quit,
    /// The Game Menu's Options and Cheats rows.
    Options,
    Cheats,
    Restart,
    /// A sub-page's first row: back to the Game Menu.
    PageBack,
}

/// The settings rows, in order: the shell's settings page and the Game
/// Menu's Options page alike.
pub fn settings_rows() -> [Row; 14] {
    [
        Row::Assist,
        Row::AiCap,
        Row::Reinforcements,
        Row::Fov,
        Row::Music,
        Row::MouseSensitivity,
        Row::PadLookSensitivity,
        Row::FlightControls,
        Row::Rumble,
        Row::InvertY,
        Row::LowPowerResolution,
        Row::InputIcons,
        Row::Viewpoint,
        Row::CareerAwards,
    ]
}

pub fn rows(m: &MenuModel) -> Vec<Row> {
    if let Some(bindings) = &m.bindings {
        return (0..bindings.labels.len()).map(Row::Binding).collect();
    }
    if let Some(choices) = &m.class_choices {
        return std::iter::once(Row::ClassBack)
            .chain(choices.choices.iter().map(|(id, _)| Row::ClassChoice(*id)))
            .collect();
    }
    if m.popup.is_some() {
        return vec![Row::No, Row::Yes];
    }
    let cheats = (0..m.cheats.len()).map(Row::Cheat);
    match m.page {
        // Both games' row order (resume, opts, suicide, restart, quit);
        // `[ours]` Cheats and Change class follow Options.
        Some(PausePage::Main) => {
            let mut v = vec![Row::Resume, Row::Options];
            if m.class_picker_available {
                v.push(Row::ClassPicker);
            }
            if m.respawn_label.is_some() {
                v.push(Row::Respawn);
            }
            if m.restart_available {
                v.push(Row::Restart);
            }
            v.push(Row::Quit);
            v
        }
        Some(PausePage::Options) => std::iter::once(Row::PageBack)
            .chain(SettingsPage::CATEGORIES.map(Row::Settings))
            .collect(),
        Some(PausePage::Settings(page)) => std::iter::once(Row::PageBack)
            .chain(page.rows().iter().copied())
            .collect(),
        Some(PausePage::Cheats) => std::iter::once(Row::PageBack).chain(cheats).collect(),
        None => {
            let mut v = vec![Row::Resume];
            if m.class_picker_available && m.in_shell.is_none() {
                v.push(Row::ClassPicker);
            }
            v.extend(cheats);
            v.extend(settings_rows());
            if m.in_shell.is_none() {
                if m.respawn_label.is_some() {
                    v.push(Row::Respawn);
                }
                v.push(Row::Quit);
            }
            v
        }
    }
}

/// What Cross (or a click) on a button row does; `None` on a setting row.
pub fn press(m: &MenuModel, row: Row) -> Option<MenuAction> {
    Some(match row {
        Row::ClassPicker => MenuAction::OpenClassPicker,
        Row::ClassBack => MenuAction::CloseClassPicker,
        Row::ClassChoice(i) => MenuAction::SelectClass(i),
        Row::Respawn => MenuAction::RequestRespawn,
        Row::No => MenuAction::CancelPopup,
        Row::Yes => match m.popup {
            Some(Popup::Quit) => MenuAction::Quit,
            Some(Popup::Restart) => MenuAction::Restart,
            _ => MenuAction::Respawn,
        },
        Row::Resume => MenuAction::Resume,
        Row::Cheat(i) => MenuAction::ToggleCheat(i),
        Row::Options => MenuAction::OpenPage(PausePage::Options),
        Row::Cheats if row_locked(m, row).is_none() => MenuAction::OpenPage(PausePage::Cheats),
        Row::Settings(page) => MenuAction::OpenPage(PausePage::Settings(page)),
        Row::EditBindings(context) => match m.page {
            Some(PausePage::Settings(page)) => MenuAction::EditBindings(page, context),
            _ => return None,
        },
        Row::PageBack => MenuAction::ClosePage,
        Row::Restart => MenuAction::RequestRestart,
        Row::FlightControls => MenuAction::ToggleFlightControls,
        Row::FpsCounter => MenuAction::SetFpsCounter(m.fps_counter.toggled()),
        Row::CareerAwards => MenuAction::SetCareerAwards(m.career_awards_disabled),
        Row::Rumble => {
            let next = (rumble_level(m.rumble_strength) + 1) % RUMBLE_LEVELS.len();
            MenuAction::SetRumble(RUMBLE_LEVELS[next].0)
        }
        Row::RumbleOnFire => MenuAction::ToggleRumbleOnFire,
        // The flat list (space battles) quits at once, as it always has.
        Row::Quit if m.page.is_some() => MenuAction::RequestQuit,
        Row::Quit => MenuAction::Quit,
        _ => return None,
    })
}

/// The Team size / Reinforcements rows as drawn: label and value, nothing
/// after the percentage.
pub fn population_row(m: &MenuModel, row: Row) -> Option<(&'static str, String)> {
    match row {
        Row::AiCap => Some(("Team size", format!("{}%", m.team_size_pct))),
        Row::Reinforcements => Some(("Reinforcements", format!("{}%", m.reinforcements_pct))),
        _ => None,
    }
}

/// The help under the rows for the hovered or focused row: Team size and
/// Reinforcements say what an average match looks like at their values. In a
/// battle a Reinforcements change applies from the next round. Button prompts
/// says what it is for.
pub fn row_help(m: &MenuModel, row: Row) -> Option<String> {
    if let Some(why) = row_locked(m, row) {
        return Some(why.to_owned());
    }
    match row {
        // Over, the round keeps its teams; the value is the next round's.
        Row::AiCap if m.in_shell.is_none() && m.outcome_line.is_some() => Some(format!(
            "{}\nChanges apply from the next round.",
            m.population_note[0]
        )),
        Row::AiCap => Some(m.population_note[0].clone()),
        Row::Reinforcements if m.in_shell.is_none() => Some(format!(
            "{}\nChanges apply from the next round.",
            m.population_note[1]
        )),
        Row::Reinforcements => Some(m.population_note[1].clone()),
        Row::Binding(_) => m.bindings.as_ref().and_then(|b| {
            if b.capture {
                Some("Press and release a control to assign it. Hold a control to cancel.".into())
            } else {
                b.message.clone()
            }
        }),
        Row::Difficulty if m.in_shell.is_none() => Some("Changes apply from the next round.".into()),
        Row::Rumble => Some(
            "Vibrates when you take a hit, your vehicle takes damage, or your ship collides."
                .into(),
        ),
        Row::CareerAwards => Some(
            "Use earned weapons and stat bonuses. Medals and career stats still count when off."
                .to_owned(),
        ),
        Row::InputIcons => Some(
            "Which button icons the game shows. Auto follows the controller you are using."
                .to_owned(),
        ),
        Row::PadLookSensitivity => Some(
            "How fast the controller's right stick turns the view. From 1.3 up it reaches \
             full turning speed before the stick's end. Flying a ship and space battles are not affected."
                .to_owned(),
        ),
        Row::FlightControls => Some(
            if m.rc_flight_controls {
                "Left stick controls yaw and throttle. Right stick controls roll and pitch."
            } else {
                "Left stick controls yaw and pitch. Right stick controls roll and throttle."
            }
            .to_owned(),
        ),
        _ => None,
    }
}

/// Why a row cannot change here, if it cannot: such a row is drawn greyed
/// with this reason as its help, and takes no step.
pub fn row_locked(m: &MenuModel, row: Row) -> Option<&'static str> {
    match row {
        Row::GraphicsQuality => Some("Normal is the current graphics preset."),
        Row::Cheats if m.cheats.is_empty() => {
            Some("Cheats are available while a battle is paused.")
        }
        Row::AiCap if m.team_size_off && m.in_shell.is_none() => {
            Some("Team size is fixed in this battle: it has no map team sizes to scale.")
        }
        _ => None,
    }
}

/// The rows a step changes (Left / Right, ◀ / ▶); Button prompts too.
pub(crate) fn is_value_row(row: Row) -> bool {
    matches!(
        row,
        Row::Assist
            | Row::AiCap
            | Row::Reinforcements
            | Row::Fov
            | Row::Music
            | Row::Difficulty
            | Row::EffectsVolume
            | Row::DialogueVolume
            | Row::FpsCounter
            | Row::Rumble
            | Row::MouseSensitivity
            | Row::PadLookSensitivity
            | Row::FlightControls
            | Row::CareerAwards
            | Row::InputIcons
    )
}

/// A step on row `index` (◀ when `right` is false, ▶ when true): the actions
/// Left or Right give with the cursor there.
pub fn step(m: &MenuModel, index: usize, right: bool) -> Vec<MenuAction> {
    let at = MenuModel {
        cursor: index,
        ..m.clone()
    };
    navigate(
        &at,
        MenuInput {
            left: !right,
            right,
            ..Default::default()
        },
    )
    .1
}

/// The egui id of row `index`'s ◀ (`right` false) or ▶ widget.
pub fn step_id(index: usize, right: bool) -> egui::Id {
    egui::Id::new(("menu_step", index, right))
}

/// Apply controller input to the cursor/values; returns actions to perform
/// and the new cursor.
pub fn navigate(m: &MenuModel, input: MenuInput) -> (usize, Vec<MenuAction>) {
    let rows = rows(m);
    let n = rows.len();
    let mut cursor = m.cursor.min(n - 1);
    let mut actions = Vec::new();
    // Back resumes from anywhere, the class picker and the Respawn popup
    // included (playtest A3).
    if input.back {
        let back = if matches!(
            m.page,
            Some(PausePage::Options | PausePage::Settings(_) | PausePage::Cheats)
        ) && m.popup.is_none()
        {
            MenuAction::ClosePage
        } else {
            MenuAction::Resume
        };
        return (cursor, vec![back]);
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
    if m.popup.is_some() && (input.left || input.right) {
        cursor = 1 - cursor;
    }
    if input.confirm {
        if let Some(a) = press(m, rows[cursor]) {
            return (cursor, vec![a]);
        }
    }
    match rows[cursor] {
        row if row_locked(m, row).is_some() => {}
        Row::Assist if input.left || input.right => {
            let d = if input.right { 0.1 } else { -0.1 };
            actions.push(MenuAction::SetAssist(
                ((m.assist_intensity + d) * 10.0).round().clamp(0.0, 10.0) / 10.0,
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
        Row::Difficulty if input.left || input.right => {
            let d = if input.right { 1 } else { -1 };
            actions.push(MenuAction::SetDifficulty((m.difficulty + d).clamp(1, 4)));
        }
        Row::EffectsVolume | Row::DialogueVolume if input.left || input.right => {
            let effects = rows[cursor] == Row::EffectsVolume;
            let level = if effects {
                m.effects_volume
            } else {
                m.dialogue_volume
            };
            let d = if input.right { 1.0 } else { -1.0 };
            let level = ((level * 10.0).round() + d).clamp(0.0, 10.0) / 10.0;
            actions.push(if effects {
                MenuAction::SetEffectsVolume(level)
            } else {
                MenuAction::SetDialogueVolume(level)
            });
        }
        Row::FpsCounter if input.left || input.right => {
            actions.push(MenuAction::SetFpsCounter(m.fps_counter.toggled()));
        }
        Row::Rumble if input.left || input.right => {
            let i = rumble_level(m.rumble_strength);
            let next = if input.right {
                (i + 1).min(RUMBLE_LEVELS.len() - 1)
            } else {
                i.saturating_sub(1)
            };
            if next != i {
                actions.push(MenuAction::SetRumble(RUMBLE_LEVELS[next].0));
            }
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
        Row::PadLookSensitivity if input.left || input.right => {
            let d = if input.right { 0.1 } else { -0.1 };
            let (lo, hi) = PAD_LOOK_SENSITIVITY_RANGE;
            actions.push(MenuAction::SetPadLookSensitivity(
                (((m.pad_look_sensitivity + d) * 10.0).round() / 10.0).clamp(lo, hi),
            ));
        }
        Row::FlightControls if input.left || input.right => {
            actions.push(MenuAction::ToggleFlightControls)
        }
        Row::CareerAwards if input.left || input.right => {
            actions.push(MenuAction::SetCareerAwards(input.right));
        }
        Row::InvertY if input.confirm || input.left || input.right => {
            actions.push(MenuAction::ToggleInvertY)
        }
        Row::TerrainLod if input.confirm || input.left || input.right => {
            actions.push(MenuAction::ToggleTerrainLod)
        }
        Row::SceneSoftness if input.confirm || input.left || input.right => {
            actions.push(MenuAction::ToggleSceneSoftness)
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
    menu_row(ui, focus, text, value, RowKind::Plain).0
}

/// What a row draws beside its label and value.
#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    Plain,
    /// A value row: ◀ and ▶ around the value, with ids from [`step_id`] for
    /// this row index.
    Steps(usize),
    /// A row that cannot change here ([`row_locked`]): greyed, no arrows.
    Locked,
}

/// The value's width between a value row's arrows, so the arrows line up.
const STEP_VALUE_WIDTH: f32 = 112.0;
/// A pause-menu row's least width: the longest label, the arrows and the value.
const MENU_ROW_WIDTH: f32 = 460.0;

/// The height of a row's 18-point text: a value row's arrows and value keep
/// to it, so every row has the same pitch ([`RowList`]).
fn text_height(ui: &egui::Ui) -> f32 {
    ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(18.0)))
}

/// One ◀ or ▶ of a value row: a click steps the row.
fn step_arrow(ui: &mut egui::Ui, id: egui::Id, glyph: &str) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(24.0, text_height(ui)), egui::Sense::hover());
    let r = ui.interact(rect, id, egui::Sense::click());
    let color = if r.hovered() {
        Color32::from_rgb(255, 214, 102)
    } else {
        Color32::WHITE
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(18.0),
        color,
    );
    r
}

/// A row, on a value row the arrow clicked this frame (`Some(true)` ▶), and
/// whether the pointer is over the row.
fn menu_row(
    ui: &mut egui::Ui,
    focus: Option<f32>,
    text: String,
    value: Option<String>,
    kind: RowKind,
) -> (egui::Response, Option<bool>, bool) {
    let selected = focus.is_some();
    let bg = match focus {
        Some(a) => Color32::from_rgba_unmultiplied(76, 115, 178, (200.0 * a.clamp(0.0, 1.0)) as u8),
        None => Color32::TRANSPARENT,
    };
    let (text_color, value_color) = match (kind, selected) {
        (RowKind::Locked, _) => (Color32::GRAY, Color32::GRAY),
        (_, true) => (Color32::WHITE, Color32::WHITE),
        (_, false) => (Color32::LIGHT_GRAY, Color32::WHITE),
    };
    let mut label = None;
    let mut stepped = None;
    let row = egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .corner_radius(3.0)
        .show(ui, |ui| {
            ui.set_min_width(if kind == RowKind::Plain {
                360.0
            } else {
                MENU_ROW_WIDTH
            });
            ui.horizontal(|ui| {
                label = Some(
                    ui.add(
                        egui::Label::new(RichText::new(text).size(18.0).color(text_color))
                            .sense(egui::Sense::click()),
                    ),
                );
                if let Some(v) = value {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let v = RichText::new(v).size(18.0).color(value_color).strong();
                        let RowKind::Steps(index) = kind else {
                            ui.label(v);
                            return;
                        };
                        if step_arrow(ui, step_id(index, true), "▶")
                            .clicked_by(egui::PointerButton::Primary)
                        {
                            stepped = Some(true);
                        }
                        ui.add_sized([STEP_VALUE_WIDTH, text_height(ui)], egui::Label::new(v));
                        if step_arrow(ui, step_id(index, false), "◀")
                            .clicked_by(egui::PointerButton::Primary)
                        {
                            stepped = Some(false);
                        }
                    });
                }
            });
        });
    let label = label.unwrap();
    let hovered = row.response.contains_pointer();
    // The whole row takes a click, but over a value row's arrows it would
    // take theirs: there only the label does.
    let response = match kind {
        RowKind::Plain => label.union(row.response.interact(egui::Sense::click())),
        _ => label,
    };
    (response, stepped, hovered)
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
        page: (Option<usize>, u8),
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
pub(crate) fn first_row(cursor: usize, shown: usize, rows: usize) -> usize {
    cursor
        .saturating_sub(shown / 2)
        .min(rows.saturating_sub(shown))
}

/// A row's label and value as drawn.
pub(crate) fn row_text(m: &MenuModel, row: Row) -> (String, Option<String>) {
    let volume = |level: f32| Some(format!("{:.0}%", level * 100.0));
    let on_off = |on: bool| Some(if on { "ON" } else { "off" }.to_owned());
    match row {
        Row::Binding(i) => {
            let (name, value) = &m.bindings.as_ref().unwrap().labels[i];
            (name.clone(), (!value.is_empty()).then(|| value.clone()))
        }
        Row::ClassPicker => ("Change class / hero (testing)".into(), None),
        Row::ClassBack => ("Back".into(), None),
        Row::ClassChoice(k) => (
            m.class_choices
                .as_ref()
                .and_then(|p| p.choices.iter().find(|(id, _)| *id == k))
                .map_or_else(String::new, |c| c.1.clone()),
            None,
        ),
        Row::Respawn => (
            m.respawn_label.clone().unwrap_or_else(|| "Respawn".into()),
            None,
        ),
        Row::No => ("No".into(), None),
        Row::Yes => ("Yes".into(), None),
        Row::Resume => (
            if m.in_shell.is_some() {
                "Back".into()
            } else if m.page.is_some() {
                m.texts.resume.clone()
            } else {
                "Resume".into()
            },
            None,
        ),
        // The Cheats page is titled: its rows need no prefix.
        Row::Cheat(k) => {
            let (_, label, on) = m.cheats[k];
            match m.page {
                Some(_) => (label.into(), on_off(on)),
                None => (format!("Cheat: {label}"), on_off(on)),
            }
        }
        Row::Options => (m.texts.options.clone(), None),
        Row::Settings(page) => (page.title().into(), None),
        Row::EditBindings(context) => (context.title().into(), None),
        Row::Difficulty => (
            "Difficulty".into(),
            Some(
                ["Easy", "Normal", "Hard", "Elite"][(m.difficulty.clamp(1, 4) - 1) as usize].into(),
            ),
        ),
        Row::EffectsVolume => ("Sound Effects Volume".into(), volume(m.effects_volume)),
        Row::DialogueVolume => ("Dialogue Volume".into(), volume(m.dialogue_volume)),
        Row::GraphicsQuality => ("Graphics quality".into(), Some("Normal".into())),
        Row::FpsCounter => ("FPS counter".into(), Some(m.fps_counter.label().into())),
        Row::Cheats => (m.texts.cheats.clone(), None),
        Row::Restart => (m.texts.restart.clone(), None),
        Row::PageBack => ("Back".into(), None),
        Row::Assist => (
            "Aim assist intensity".into(),
            Some(format!("{:.1}", m.assist_intensity)),
        ),
        Row::AiCap | Row::Reinforcements => {
            let (label, value) = population_row(m, row).unwrap();
            (label.into(), Some(value))
        }
        Row::InputIcons => (
            "Button prompts".into(),
            Some(m.input_icons_label.to_owned()),
        ),
        Row::Viewpoint => (
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
        Row::Fov => ("Field of view".into(), Some(format!("{:.0}°", m.fov_deg))),
        Row::Music => (
            "Music volume".into(),
            Some(if m.music_volume <= 0.0 {
                "off".into()
            } else {
                format!("{:.0}%", m.music_volume * 100.0)
            }),
        ),
        Row::CareerAwards => (
            "Career award bonuses".into(),
            Some(
                if m.career_awards_disabled {
                    "Off"
                } else {
                    "On"
                }
                .into(),
            ),
        ),
        Row::Rumble => (
            "Controller rumble".into(),
            Some(RUMBLE_LEVELS[rumble_level(m.rumble_strength)].1.into()),
        ),
        Row::RumbleOnFire => (
            "Rumble on fire".into(),
            Some(if m.rumble_on_fire { "On" } else { "Off" }.into()),
        ),
        Row::MouseSensitivity => (
            "Mouse sensitivity".into(),
            Some(format!("{:.1}", m.mouse_sensitivity)),
        ),
        Row::PadLookSensitivity => (
            "Controller look sensitivity".into(),
            Some(format!("{:.1}", m.pad_look_sensitivity)),
        ),
        Row::FlightControls => (
            "Flight controls".into(),
            Some(
                if m.rc_flight_controls {
                    "RC Mode 2"
                } else {
                    "Classic"
                }
                .into(),
            ),
        ),
        Row::TerrainLod => ("Distant terrain".into(), on_off(m.terrain_lod)),
        Row::SceneSoftness => ("Scene softness".into(), on_off(m.scene_softness)),
        Row::InvertY => ("Invert mouse Y".into(), on_off(m.invert_y)),
        Row::LowPowerResolution => (
            "Low power resolution".into(),
            on_off(m.low_power_resolution),
        ),
        Row::Quit if m.page.is_some() => (m.texts.quit.clone(), None),
        Row::Quit => ("Quit to desktop".into(), None),
    }
}

/// The panel's title: the Game Menu's page, the class picker's, the shell
/// page's own, or the flat list's.
pub(crate) fn menu_title(m: &MenuModel) -> &str {
    if m.class_choices.is_some() {
        return "CHANGE CLASS / HERO";
    }
    if let Some(bindings) = &m.bindings {
        return bindings.title;
    }
    match m.page {
        Some(PausePage::Main) => &m.texts.title,
        Some(PausePage::Options) => &m.texts.options,
        Some(PausePage::Cheats) => &m.texts.cheats,
        Some(PausePage::Settings(page)) => page.title(),
        None => m.in_shell.as_deref().unwrap_or("PAUSED"),
    }
}

/// The open popup's question.
pub(crate) fn popup_prompt(m: &MenuModel) -> Option<&str> {
    Some(match m.popup? {
        Popup::Respawn => &m.texts.respawn_prompt,
        Popup::Quit => &m.texts.quit_prompt,
        Popup::Restart => &m.texts.restart_prompt,
    })
}

/// Which list the rows are (a page or a popup), so a new one starts at its
/// cursor's place.
fn list_key(m: &MenuModel) -> u8 {
    match (m.popup, m.page) {
        (Some(p), _) => 1 + p as u8,
        (None, None | Some(PausePage::Main)) => 0,
        (None, Some(PausePage::Options)) => 4,
        (None, Some(PausePage::Cheats)) => 5,
        (None, Some(PausePage::Settings(page))) => 6 + page as u8,
    }
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
                            RichText::new(menu_title(m))
                                .size(28.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        if let Some(o) = &m.outcome_line {
                            ui.label(RichText::new(o).size(16.0).color(Color32::LIGHT_GRAY));
                        }
                    });
                    ui.add_space(8.0);
                    if let Some(prompt) = popup_prompt(m) {
                        // The flat list (space battles) keeps its own line.
                        match m.page {
                            Some(_) => ui.label(RichText::new(prompt).size(16.0).color(Color32::WHITE)),
                            None if matches!(m.popup, Some(Popup::Respawn)) => {
                                ui.label("Do you really wish to kill your current player?")
                            }
                            None => ui.label(prompt),
                        };
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
                    let has_help = rows.iter().any(|r| {
                        matches!(r, Row::Reinforcements | Row::InputIcons | Row::PadLookSensitivity | Row::FlightControls | Row::Rumble)
                            || row_locked(m, *r).is_some()
                    });
                    let mut hovered_help = None;
                    let room = ctx.content_rect().height()
                        - PANEL_MARGINS
                        - ui.min_rect().height()
                        - FOOTER_ROOM
                        - if has_help { HELP_ROOM } else { 0.0 };
                    let gap = ui.spacing().item_spacing.y;
                    let list = RowList::for_frame(
                        ctx,
                        (m.class_choices.as_ref().map(|p| p.tab), list_key(m)),
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
                                let (text, value) = row_text(m, *row);
                                let kind = if row_locked(m, *row).is_some() {
                                    RowKind::Locked
                                } else if is_value_row(*row) {
                                    RowKind::Steps(i)
                                } else {
                                    RowKind::Plain
                                };
                                let (clicked, stepped, hovered) = menu_row(ui, sel, text, value, kind);
                                if hovered {
                                    hovered_help = row_help(m, *row);
                                }
                                if let Some(right) = stepped {
                                    actions.extend(step(m, i, right));
                                } else if clicked.clicked_by(egui::PointerButton::Primary) {
                                    if let Some(a) = press(m, *row) {
                                        actions.push(a);
                                    }
                                    match row {
                                        Row::TerrainLod => actions.push(MenuAction::ToggleTerrainLod),
                                        Row::SceneSoftness => actions.push(MenuAction::ToggleSceneSoftness),
                                        Row::InvertY => actions.push(MenuAction::ToggleInvertY),
                                        Row::LowPowerResolution => {
                                            actions.push(MenuAction::ToggleLowPowerResolution)
                                        }
                                        Row::InputIcons => {
                                            actions.push(MenuAction::CycleInputIcons(1))
                                        }
                                        Row::Viewpoint => actions.push(MenuAction::ToggleViewpoint),
                                        _ => {}
                                    }
                                }
                            }
                        });
                    list.remember(ctx, &shown, gap);
                    // Room is kept for the help so the centered menu does not
                    // jump as the cursor reaches those rows. The row under the
                    // pointer gives its help first, so a mouse player reads a
                    // greyed row's reason too.
                    if has_help {
                        ui.add_space(6.0);
                        ui.scope(|ui| {
                            ui.set_min_height(60.0);
                            let help = hovered_help
                                .or_else(|| rows.get(m.cursor).and_then(|r| row_help(m, *r)));
                            if let Some(help) = help {
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
mod tests;

#[cfg(test)]
mod morse_tests;

#[cfg(test)]
mod pointer_navigation_tests;

#[cfg(test)]
mod game_menu_tests;
