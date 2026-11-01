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

#[cfg(test)]
mod tests {
    use super::*;

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
