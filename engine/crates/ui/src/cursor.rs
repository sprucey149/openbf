//! The shell's pointer activity. The window owns the native cursor image.

/// The shell pointer: where it is, and whether it moved or clicked since the
/// last frame. Hover moves menu focus only on fresh pointer activity, so a
/// resting cursor cannot undo keyboard or controller navigation.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Pointer {
    pos: Option<[f32; 2]>,
    moved: bool,
    click: bool,
    wheel: f32,
}

impl Pointer {
    /// The cursor is at `pos` (window pixels); a new spot counts as motion.
    pub fn moved_to(&mut self, pos: [f32; 2]) {
        if self.pos != Some(pos) {
            self.pos = Some(pos);
            self.moved = true;
        }
    }

    /// A left click for the next frame.
    pub fn click(&mut self) {
        self.click = true;
    }

    /// Mouse wheel vertical delta accumulated for the next frame.
    pub fn wheel(&mut self, dy: f32) {
        self.wheel += dy;
    }

    /// The accumulated wheel delta since the last take.
    pub fn take_wheel(&mut self) -> f32 {
        std::mem::take(&mut self.wheel)
    }

    /// The current cursor position, if any (even if resting).
    pub fn pos(&self) -> Option<[f32; 2]> {
        self.pos
    }

    /// The pointer left the window or lost focus.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// This frame's pointer: its position and whether it clicked, only when
    /// it moved or clicked since the last call.
    pub fn take_frame(&mut self) -> Option<([f32; 2], bool)> {
        let fresh = self.moved || self.click;
        let click = std::mem::take(&mut self.click);
        self.moved = false;
        self.pos.filter(|_| fresh).map(|p| (p, click))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuInput;
    use crate::shell_ps2::{Page, ShellMenus};

    /// One shell frame as the windowed game runs it: a fresh pointer over a
    /// row focuses it (the cursor here rests over row 0), then the step.
    fn frame(menus: &mut ShellMenus, pointer: &mut Pointer, mut input: MenuInput) {
        if let Some((_, click)) = pointer.take_frame() {
            menus.focus_row(0);
            input.confirm |= click;
        }
        menus.step(input, false, 0.1);
    }

    #[test]
    fn a_resting_pointer_does_not_undo_keyboard_navigation() {
        let mut menus = ShellMenus::new(&|_, s| s.to_owned());
        menus.step(MenuInput::default(), true, 1.0);
        let mut pointer = Pointer::default();
        pointer.moved_to([100.0, 200.0]);
        frame(&mut menus, &mut pointer, MenuInput::default());
        assert_eq!(menus.stack().unwrap().focus, 0, "hover focuses row 0");
        let down = MenuInput {
            down: true,
            ..Default::default()
        };
        frame(&mut menus, &mut pointer, down);
        assert_eq!(menus.stack().unwrap().focus, 3, "keyboard moves to row 3");
        // The cursor stays over row 0: idle frames, and a repeat of the same spot.
        frame(&mut menus, &mut pointer, MenuInput::default());
        pointer.moved_to([100.0, 200.0]);
        frame(&mut menus, &mut pointer, MenuInput::default());
        assert_eq!(
            menus.stack().unwrap().focus,
            3,
            "idle pointer reset the focus"
        );
        // Confirm opens the row the keyboard chose.
        let confirm = MenuInput {
            confirm: true,
            ..Default::default()
        };
        frame(&mut menus, &mut pointer, confirm);
        assert_eq!(menus.page, Page::Options);
    }

    #[test]
    fn pointer_motion_and_clicks_are_fresh_once() {
        let mut p = Pointer::default();
        assert_eq!(p.take_frame(), None);
        p.click();
        assert_eq!(p.take_frame(), None, "a click with no position");
        p.moved_to([1.0, 2.0]);
        assert_eq!(p.take_frame(), Some(([1.0, 2.0], false)));
        assert_eq!(p.take_frame(), None);
        p.click();
        assert_eq!(p.take_frame(), Some(([1.0, 2.0], true)));
        assert_eq!(p.take_frame(), None);
        p.clear();
        p.moved_to([1.0, 2.0]);
        assert_eq!(
            p.take_frame(),
            Some(([1.0, 2.0], false)),
            "back after a clear"
        );
    }

    #[test]
    fn mouse_wheel_accumulates_and_clears_without_forcing_hover() {
        let mut p = Pointer::default();
        p.moved_to([50.0, 60.0]);
        assert_eq!(p.take_frame(), Some(([50.0, 60.0], false)));
        // Cursor is now resting at [50.0, 60.0].
        assert_eq!(p.pos(), Some([50.0, 60.0]));
        assert_eq!(p.take_frame(), None, "resting cursor has no motion");
        p.wheel(-1.0);
        p.wheel(-2.0);
        assert_eq!(p.take_wheel(), -3.0);
        assert_eq!(p.take_wheel(), 0.0);
        assert_eq!(
            p.take_frame(),
            None,
            "wheel delta does not trigger resting hover"
        );
        p.wheel(1.0);
        p.clear();
        assert_eq!(p.take_wheel(), 0.0);
        assert_eq!(p.pos(), None);
    }
}
