//! Shared Options page tree and the frame-rate overlay.

use super::{PausePage, Row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlContext {
    Infantry,
    Vehicle,
    Starfighter,
}

impl ControlContext {
    pub fn title(self) -> &'static str {
        match self {
            Self::Infantry => "Infantry Controls",
            Self::Vehicle => "Vehicle Controls",
            Self::Starfighter => "Starfighter Controls",
        }
    }
}

use egui::{Align2, Color32};
use ControlContext::{Infantry, Starfighter, Vehicle};

/// The options pages available in the main menu and in a battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsPage {
    Game,
    Audio,
    Graphics,
    Controls,
    KeyboardMouse,
    Gamepad,
}

impl SettingsPage {
    pub const CATEGORIES: [Self; 4] = [Self::Game, Self::Audio, Self::Graphics, Self::Controls];

    pub fn title(self) -> &'static str {
        match self {
            Self::Game => "Game Options",
            Self::Audio => "Audio Options",
            Self::Graphics => "Graphics Options",
            Self::Controls => "Controls",
            Self::KeyboardMouse => "Keyboard / Mouse",
            Self::Gamepad => "Gamepad",
        }
    }

    pub(super) fn rows(self) -> &'static [Row] {
        use Row::*;
        match self {
            Self::Game => &[
                Difficulty,
                AiCap,
                Reinforcements,
                CareerAwards,
                Cheats,
                PlayerName,
            ],
            Self::Audio => &[Music, EffectsVolume, DialogueVolume],
            Self::Graphics => &[
                Viewpoint,
                Fov,
                GraphicsQuality,
                AutoQuality,
                LowPowerResolution,
                FpsCounter,
                NetworkMeter,
                ResetGraphics,
            ],
            Self::Controls => &[Settings(Self::KeyboardMouse), Settings(Self::Gamepad)],
            Self::KeyboardMouse => &[
                MouseSensitivity,
                InvertY,
                EditBindings(Infantry),
                EditBindings(Vehicle),
                EditBindings(Starfighter),
            ],
            Self::Gamepad => &[
                Assist,
                PadLookSensitivity,
                FlightControls,
                Rumble,
                RumbleOnFire,
                InputIcons,
                EditBindings(Infantry),
                EditBindings(Vehicle),
                EditBindings(Starfighter),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FpsCounter {
    #[default]
    Off,
    On,
}

impl FpsCounter {
    pub fn from_index(index: u8) -> Self {
        match index {
            1 | 2 => Self::On,
            _ => Self::Off,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::On => "On",
        }
    }

    pub(super) fn toggled(self) -> Self {
        match self {
            Self::Off => Self::On,
            Self::On => Self::Off,
        }
    }
}

pub const FOV_SCALE_RANGE: (f32, f32) = (0.75, 2.0);
pub const MAX_FOV_DEG: f32 = 100.0;

pub fn clamp_fov_scale(base_degrees: f32, scale: f32) -> f32 {
    let (min, max) = FOV_SCALE_RANGE;
    let max = if base_degrees > 0.0 {
        (MAX_FOV_DEG / base_degrees).clamp(min, max)
    } else {
        max
    };
    if scale.is_finite() {
        scale.clamp(min, max)
    } else {
        1.0_f32.clamp(min, max)
    }
}

/// [ours] Feedback presets, from disabled through the original strength.
pub const RUMBLE_LEVELS: [(f32, &str); 4] =
    [(0.0, "Off"), (0.33, "Low"), (0.67, "Medium"), (1.0, "High")];

pub fn rumble_level(strength: f32) -> usize {
    (strength.clamp(0.0, 1.0) * 3.0).round() as usize
}

/// An optional top-left counter, in display coordinates and outside the sim.
pub fn draw_fps_counter(ctx: &egui::Context, mode: FpsCounter, seconds: f32) {
    draw_fps_counter_note(ctx, mode, seconds, None);
}

/// [`draw_fps_counter`] with a note after the rate, such as the automatic
/// quality's render scale ("60 FPS · res 85%").
pub fn draw_fps_counter_note(
    ctx: &egui::Context,
    mode: FpsCounter,
    seconds: f32,
    note: Option<&str>,
) {
    if mode == FpsCounter::Off || !seconds.is_finite() || seconds <= 0.0 {
        return;
    }
    let rate = format!("{:.0} FPS", seconds.recip());
    let text = egui::RichText::new(match note {
        Some(n) => format!("{rate} · {n}"),
        None => rate,
    })
    .monospace();
    egui::Area::new(egui::Id::new("fps_counter"))
        .interactable(false)
        .anchor(Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(180))
                .inner_margin(egui::Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Extend));
                });
        });
}

impl PausePage {
    /// Back returns to the containing page and restores its selection.
    pub fn parent(self) -> Option<(Self, Row)> {
        let (page, row) = match self {
            Self::Main => return None,
            Self::Options => (Self::Main, Row::Options),
            Self::Settings(SettingsPage::KeyboardMouse | SettingsPage::Gamepad) => {
                let Self::Settings(device) = self else {
                    unreachable!()
                };
                (
                    Self::Settings(SettingsPage::Controls),
                    Row::Settings(device),
                )
            }
            Self::Settings(category) => (Self::Options, Row::Settings(category)),
            Self::Cheats => (Self::Settings(SettingsPage::Game), Row::Cheats),
        };
        Some((page, row))
    }
}
