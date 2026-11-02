//! Player camera viewpoint preference (first person vs third person).
//!
//! Toggled via options menu or hotkey; stamped onto the sim input pad as
//! `buttons::FIRST_PERSON` (`analysis/re-first-person.md` §3).

use core::sim::{buttons, PadState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Viewpoint {
    #[default]
    Third,
    First,
}

impl Viewpoint {
    pub fn toggled(self) -> Self {
        match self {
            Viewpoint::Third => Viewpoint::First,
            Viewpoint::First => Viewpoint::Third,
        }
    }

    pub fn is_first(self) -> bool {
        matches!(self, Viewpoint::First)
    }

    pub fn stamp(self, pad: PadState) -> PadState {
        let rest = pad.buttons & !(buttons::FIRST_PERSON | buttons::VIEW_TOGGLE);
        let buttons = match self {
            Viewpoint::Third => rest,
            Viewpoint::First => rest | buttons::FIRST_PERSON,
        };
        PadState { buttons, ..pad }
    }
}

#[derive(Debug, Default)]
pub struct ViewToggle {
    held: bool,
}

impl ViewToggle {
    pub fn pressed(&mut self, pad: &PadState) -> bool {
        let down = pad.pressed(buttons::VIEW_TOGGLE);
        let edge = down && !self.held;
        self.held = down;
        edge
    }
}

/// The window's toggle for a run that reads its pads from a log. The window
/// turns `VIEW_TOGGLE` into `FIRST_PERSON` before the sim sees it, so a logged
/// tap did nothing in a replay and the soldier stayed in third person.
///
/// A recorded log never holds the bit, and until a log taps it the pads pass
/// through unchanged, so a log that holds `FIRST_PERSON` itself replays as before,
/// and a tap on such a log turns from the view it is holding. Only `--walk-replay`
/// runs this; `--replay` and `--sim-trace` step a log's bits as they are.
#[derive(Debug, Default)]
pub struct ScriptedView {
    toggle: ViewToggle,
    view: Option<Viewpoint>,
}

impl ScriptedView {
    /// The pad of the player a log drives, as the window would hand it to the sim.
    pub fn apply(&mut self, pad: PadState) -> PadState {
        if self.toggle.pressed(&pad) {
            let held = if pad.pressed(buttons::FIRST_PERSON) {
                Viewpoint::First
            } else {
                Viewpoint::Third
            };
            self.view = Some(self.view.unwrap_or(held).toggled());
        }
        self.view.map_or(pad, |view| view.stamp(pad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logged_tap_changes_the_view_and_an_untapped_log_passes_through() {
        let pad = |bits| PadState {
            buttons: bits,
            ..PadState::default()
        };
        let mut untapped = ScriptedView::default();
        for bits in [0, buttons::FIRST_PERSON, buttons::FIRE, 0] {
            assert_eq!(untapped.apply(pad(bits)).buttons, bits);
        }
        let mut tapped = ScriptedView::default();
        let seen: Vec<u32> = [
            0,
            buttons::VIEW_TOGGLE,
            buttons::VIEW_TOGGLE,
            buttons::FIRE,
            0,
            buttons::VIEW_TOGGLE,
            0,
        ]
        .iter()
        .map(|&bits| tapped.apply(pad(bits)).buttons)
        .collect();
        assert_eq!(
            seen,
            [
                0,
                buttons::FIRST_PERSON,
                buttons::FIRST_PERSON,
                buttons::FIRE | buttons::FIRST_PERSON,
                buttons::FIRST_PERSON,
                0,
                0
            ]
        );
    }

    #[test]
    fn a_tap_on_a_log_holding_first_person_turns_to_third() {
        let pad = |bits| PadState {
            buttons: bits,
            ..PadState::default()
        };
        let mut view = ScriptedView::default();
        let seen: Vec<u32> = [
            buttons::FIRST_PERSON,
            buttons::FIRST_PERSON | buttons::VIEW_TOGGLE,
            buttons::FIRST_PERSON,
            0,
            buttons::VIEW_TOGGLE,
            0,
        ]
        .iter()
        .map(|&bits| view.apply(pad(bits)).buttons)
        .collect();
        assert_eq!(
            seen,
            [
                buttons::FIRST_PERSON,
                0,
                0,
                0,
                buttons::FIRST_PERSON,
                buttons::FIRST_PERSON
            ]
        );
    }

    #[test]
    fn the_viewpoint_rides_the_pad() {
        let pad = PadState {
            buttons: buttons::FIRE | buttons::FIRST_PERSON,
            ..PadState::default()
        };
        assert_eq!(Viewpoint::Third.stamp(pad).buttons, buttons::FIRE);
        assert_eq!(
            Viewpoint::First.stamp(PadState::default()).buttons,
            buttons::FIRST_PERSON
        );
        assert_eq!(Viewpoint::Third.toggled(), Viewpoint::First);
        assert_eq!(Viewpoint::First.toggled(), Viewpoint::Third);
        let toggle = PadState {
            buttons: buttons::VIEW_TOGGLE,
            ..PadState::default()
        };
        assert_eq!(
            Viewpoint::First.stamp(toggle).buttons,
            buttons::FIRST_PERSON
        );
    }

    #[test]
    fn a_held_toggle_flips_the_viewpoint_once() {
        let down = PadState {
            buttons: buttons::VIEW_TOGGLE,
            ..PadState::default()
        };
        let up = PadState::default();
        let mut t = ViewToggle::default();
        let seen: Vec<bool> = [down, down, up, down]
            .iter()
            .map(|p| t.pressed(p))
            .collect();
        assert_eq!(seen, [true, false, false, true]);
    }
}
