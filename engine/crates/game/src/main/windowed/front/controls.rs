//! The Controls pages' key / button capture, extracted from the front facade
//! (`analysis/re-bf2-menus.md` §4.1). Pure binding-editor state: no shell
//! navigation or drawing lives here.

use ui::menu::{ControlContext, SettingsPage};
use ui::shell_feel::MenuSound;

pub(in super::super) struct Controls {
    pub(in super::super) context: ControlContext,
    pub(in super::super) layout: Layout,
    pub(in super::super) cursor: usize,
    pub(in super::super) capture: Option<Capture>,
    pub(in super::super) original: Vec<Binding>,
    pub(in super::super) message: Option<String>,
    /// Issue #194 (B10): the Players-screen seat whose profile the page edits; 0 is the
    /// host's (`App.profile`), the shell's and single player's always.
    pub(in super::super) seat: usize,
}

pub(in super::super) enum Layout {
    Keyboard(Vec<Option<game::kbm::Trigger>>),
    Gamepad(Vec<Option<gilrs::Button>>),
}

impl Layout {
    pub(in super::super) fn complete(&self) -> Vec<Binding> {
        match self {
            Self::Keyboard(items) => items
                .iter()
                .flatten()
                .copied()
                .map(Binding::Keyboard)
                .collect(),
            Self::Gamepad(items) => items
                .iter()
                .flatten()
                .copied()
                .map(Binding::Gamepad)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Binding {
    Keyboard(game::kbm::Trigger),
    Gamepad(gilrs::Button),
}

pub(in super::super) struct Capture {
    pub(in super::super) row: usize,
    pub(in super::super) ready: bool,
    pub(in super::super) controller: Option<gilrs::GamepadId>,
    pub(in super::super) candidate: Option<(Binding, std::time::Instant)>,
}

pub(in super::super) const CONTROL_COMMAND_ROWS: usize = 2;
pub(in super::super) const CANCEL_HOLD: std::time::Duration = std::time::Duration::from_millis(600);

impl Controls {
    pub(in super::super) fn new(
        device: SettingsPage,
        context: ControlContext,
        settings: &game::profile::Settings,
    ) -> Self {
        use game::control_layout;
        let layout = if device == SettingsPage::KeyboardMouse {
            let saved = settings.keyboard_layouts.get(context);
            Layout::Keyboard(if control_layout::keyboard(context, saved).is_some() {
                saved.iter().copied().map(Some).collect()
            } else {
                control_layout::keyboard_defaults(context, &settings.keyboard_bindings)
            })
        } else {
            let saved = settings.gamepad_layouts.get(context);
            Layout::Gamepad(if control_layout::gamepad(context, saved).is_some() {
                saved.iter().copied().map(Some).collect()
            } else {
                control_layout::gamepad_defaults(context, &settings.controller_bindings)
            })
        };
        // Issue #172: a layout from before F11 switched fullscreen shows its
        // F11 action unassigned, and the page asks for a key before leaving.
        let mut layout = layout;
        let mut message = None;
        if let Layout::Keyboard(items) = &mut layout {
            let actions = control_layout::actions(context, true);
            for (i, t) in items.iter_mut().enumerate() {
                if *t == Some(game::kbm::FULLSCREEN_KEY) {
                    *t = None;
                    message = Some(format!(
                        "{}; {} is unassigned",
                        game::kbm::f11_note(game::kbm::MAC),
                        actions[i].name
                    ));
                }
            }
        }
        let original = layout.complete();
        Self {
            context,
            layout,
            original,
            cursor: 0,
            capture: None,
            message,
            seat: 0,
        }
    }

    /// The page edits seat `seat`'s profile (a split player's Game Menu, issue #194 B10).
    pub(in super::super) fn for_seat(self, seat: usize) -> Self {
        Self { seat, ..self }
    }

    pub(in super::super) fn keyboard(&self) -> bool {
        matches!(self.layout, Layout::Keyboard(_))
    }

    pub(in super::super) fn len(&self) -> usize {
        match &self.layout {
            Layout::Keyboard(items) => items.len(),
            Layout::Gamepad(items) => items.len(),
        }
    }

    pub(in super::super) fn missing(&self) -> Option<usize> {
        match &self.layout {
            Layout::Keyboard(items) => items.iter().position(Option::is_none),
            Layout::Gamepad(items) => items.iter().position(Option::is_none),
        }
    }

    pub(in super::super) fn cancel(&mut self, reason: &str) -> MenuSound {
        self.capture = None;
        self.message = Some(reason.into());
        MenuSound::Cancel
    }

    pub(in super::super) fn assign(&mut self, bind: Binding) -> MenuSound {
        let Some(capture) = self.capture.as_mut() else {
            return MenuSound::Error;
        };
        let row = capture.row;
        let actions = game::control_layout::actions(self.context, self.keyboard());
        fn assign<T: Copy + Eq>(items: &mut [Option<T>], row: usize, trigger: T) -> Option<usize> {
            let conflict = items
                .iter()
                .position(|t| *t == Some(trigger))
                .filter(|i| *i != row);
            if let Some(i) = conflict {
                items[i] = None;
            }
            items[row] = Some(trigger);
            conflict
        }
        let conflict = match (&mut self.layout, bind) {
            (Layout::Keyboard(items), Binding::Keyboard(t)) => {
                // Issue #172: F11, and Command on macOS, never hold an action.
                if let Some(why) = game::kbm::reserved_key(t, game::kbm::MAC) {
                    self.message = Some(why);
                    return MenuSound::Error;
                }
                if !game::control_layout::accepts_trigger(actions[row], t) {
                    self.message = Some(
                        "Mouse wheel cannot hold movement; choose a key or mouse button".into(),
                    );
                    return MenuSound::Error;
                }
                assign(items, row, t)
            }
            (Layout::Gamepad(items), Binding::Gamepad(b)) => assign(items, row, b),
            _ => return MenuSound::Error,
        };
        self.capture = None;
        self.message = Some(match conflict {
            Some(i) => format!("{} is unassigned", actions[i].name),
            None => "Binding assigned".into(),
        });
        MenuSound::Accept
    }
}

pub(in super::super) const PAD_BUTTONS: [gilrs::Button; 17] = [
    gilrs::Button::South,
    gilrs::Button::East,
    gilrs::Button::West,
    gilrs::Button::North,
    gilrs::Button::LeftTrigger,
    gilrs::Button::LeftTrigger2,
    gilrs::Button::RightTrigger,
    gilrs::Button::RightTrigger2,
    gilrs::Button::Select,
    gilrs::Button::Start,
    gilrs::Button::Mode,
    gilrs::Button::LeftThumb,
    gilrs::Button::RightThumb,
    gilrs::Button::DPadUp,
    gilrs::Button::DPadDown,
    gilrs::Button::DPadLeft,
    gilrs::Button::DPadRight,
];
