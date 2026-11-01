//! Keyboard + mouse to `PadState`: the bindings table, held keys and latched
//! taps, mouse look in `LOOK_UNIT`s, the merge with the controller, pointer lock
//! and menu keys. OURS (public pull request #1); rules in `AGENTS.md`
//! (Keyboard + mouse).

use core::sim::{buttons, PadState, LOOK_UNIT};
use winit::event::{KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window};

pub const RAD_PER_COUNT: f64 = 0.0006;
pub const SENSITIVITY_RANGE: (f32, f32) = ui::menu::MOUSE_SENSITIVITY_RANGE;
pub const KEY_ESCAPE: u32 = 128;
pub const KEY_ENTER: u32 = 256;
/// `[ours]` Page Up / Page Down are the shell's L1 / R1 (the Instant Action
/// lists' paging, Session Options' x10 steps).
pub const KEY_PAGE_UP: u32 = 512;
pub const KEY_PAGE_DOWN: u32 = 1024;
/// The menu edge bits of L1 and R1.
pub const MENU_L1: u32 = 128;
pub const MENU_R1: u32 = 256;
/// `[ours]` Space is the pad's Square on the stats pages ("Done", issue
/// #50), and its menu edge bit.
pub const KEY_SQUARE: u32 = 2048;
pub const MENU_SQUARE: u32 = 512;

pub fn menu_key(code: KeyCode) -> u32 {
    match code {
        KeyCode::ArrowUp => 1,
        KeyCode::ArrowDown => 2,
        KeyCode::ArrowLeft => 4,
        KeyCode::ArrowRight => 8,
        KeyCode::Enter | KeyCode::NumpadEnter => 16 | KEY_ENTER,
        KeyCode::Backspace => 32,
        KeyCode::Escape => KEY_ESCAPE,
        KeyCode::PageUp => KEY_PAGE_UP,
        KeyCode::PageDown => KEY_PAGE_DOWN,
        KeyCode::Space => KEY_SQUARE,
        _ => 0,
    }
}

pub fn menu_bits(keys: u32, shell: bool) -> u32 {
    let mut bits = keys & 63;
    if keys & KEY_ESCAPE != 0 {
        bits |= if shell { 32 } else { 64 };
    }
    if shell && keys & KEY_ENTER != 0 {
        bits |= 64;
    }
    if keys & KEY_PAGE_UP != 0 {
        bits |= MENU_L1;
    }
    if keys & KEY_PAGE_DOWN != 0 {
        bits |= MENU_R1;
    }
    if keys & KEY_SQUARE != 0 {
        bits |= MENU_SQUARE;
    }
    bits
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Trigger {
    Key(KeyCode),
    Mouse(MouseButton),
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bind {
    pub bits: u32,
    pub walk: (i8, i8),
}

impl Bind {
    const fn bits(bits: u32) -> Bind {
        Bind { bits, walk: (0, 0) }
    }

    const fn walk(x: i8, y: i8, bits: u32) -> Bind {
        Bind { bits, walk: (x, y) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bindings {
    pub table: Vec<(Trigger, Bind)>,
}

impl Bindings {
    /// Saved layouts may change triggers/order, not the supported action set.
    pub fn is_valid(&self) -> bool {
        let mut remaining = Self::pc_default().table;
        if self.table.len() != remaining.len() {
            return false;
        }
        for (i, (trigger, bind)) in self.table.iter().enumerate() {
            if *trigger == Trigger::Key(KeyCode::Escape)
                || self.table[..i].iter().any(|(t, _)| t == trigger)
                || (matches!(trigger, Trigger::WheelUp | Trigger::WheelDown) && bind.walk != (0, 0))
            {
                return false;
            }
            let Some(at) = remaining.iter().position(|(_, action)| action == bind) else {
                return false;
            };
            remaining.swap_remove(at);
        }
        true
    }

    pub fn with_new_actions(mut self) -> Bindings {
        for (trigger, bind) in Self::pc_default().table {
            let known = self.table.iter().any(|(_, b)| *b == bind);
            let free = !self.table.iter().any(|(t, _)| *t == trigger);
            if !known && free {
                self.table.push((trigger, bind));
            }
        }
        self
    }

    pub fn remap(&mut self, index: usize, trigger: Trigger) -> bool {
        if index >= self.table.len() || trigger == Trigger::Key(KeyCode::Escape) {
            return false;
        }
        let (old, bind) = self.table[index];
        let other = self.table.iter().position(|(t, _)| *t == trigger);
        let wheel = |t| matches!(t, Trigger::WheelUp | Trigger::WheelDown);
        if (wheel(trigger) && bind.walk != (0, 0))
            || other.is_some_and(|i| wheel(old) && self.table[i].1.walk != (0, 0))
        {
            return false;
        }
        if let Some(other) = other {
            self.table[other].0 = self.table[index].0;
        }
        self.table[index].0 = trigger;
        true
    }

    pub fn pc_default() -> Bindings {
        use buttons as b;
        use KeyCode as K;
        use Trigger::{Key, Mouse};
        Bindings {
            table: vec![
                (Key(K::KeyW), Bind::walk(0, 1, b::SPAWN_POST_UP)),
                (Key(K::KeyS), Bind::walk(0, -1, b::SPAWN_POST_DOWN)),
                (Key(K::KeyA), Bind::walk(-1, 0, b::SPAWN_POST_LEFT)),
                (Key(K::KeyD), Bind::walk(1, 0, b::SPAWN_POST_RIGHT)),
                (Mouse(MouseButton::Left), Bind::bits(b::FIRE)),
                (Mouse(MouseButton::Right), Bind::bits(b::SECONDARY_FIRE)),
                (Mouse(MouseButton::Middle), Bind::bits(b::ZOOM)),
                (Key(K::KeyZ), Bind::bits(b::ZOOM)),
                (Key(K::Space), Bind::bits(b::JUMP | b::SPAWN_ACCEPT)),
                (Key(K::Enter), Bind::bits(b::SPAWN_ACCEPT)),
                (Key(K::NumpadEnter), Bind::bits(b::SPAWN_ACCEPT)),
                (Key(K::Backspace), Bind::bits(b::SPAWN_BACK)),
                (Key(K::KeyC), Bind::bits(b::ROLL_STANCE)),
                (Key(K::ControlLeft), Bind::bits(b::ROLL_STANCE)),
                (Key(K::ShiftLeft), Bind::bits(b::SPRINT)),
                (Key(K::KeyR), Bind::bits(b::RELOAD)),
                // The pad's Triangle: use, and the class screens' back
                // (issue #28: it opens and closes the class change at a post).
                (Key(K::KeyE), Bind::bits(b::ACTION | b::SPAWN_BACK)),
                (
                    Key(K::KeyQ),
                    Bind::bits(b::SWITCH_WEAPON | b::SPAWN_CLASS_NEXT),
                ),
                (
                    Trigger::WheelDown,
                    Bind::bits(b::SWITCH_WEAPON | b::SPAWN_CLASS_NEXT),
                ),
                (
                    Trigger::WheelUp,
                    Bind::bits(b::SWITCH_WEAPON | b::SPAWN_CLASS_PREV),
                ),
                (Key(K::Tab), Bind::bits(b::SPAWN_CLASS_PREV)),
                (Key(K::KeyG), Bind::bits(b::SWITCH_SECONDARY)),
                (Key(K::KeyV), Bind::bits(b::VIEW_TOGGLE)),
                (
                    Key(K::ArrowUp),
                    Bind::bits(b::SQUAD_MOVE_OUT | b::SPAWN_POST_UP),
                ),
                (
                    Key(K::ArrowDown),
                    Bind::bits(b::SQUAD_HOLD | b::SPAWN_POST_DOWN),
                ),
                (
                    Key(K::ArrowLeft),
                    Bind::bits(b::SQUAD_AT_EASE | b::SPAWN_POST_LEFT | b::HERO_ACCEPT),
                ),
                (
                    Key(K::ArrowRight),
                    Bind::bits(b::SQUAD_FOLLOW | b::SPAWN_POST_RIGHT | b::HERO_DECLINE),
                ),
                (Key(K::F1), Bind::bits(b::SQUAD_FOLLOW | b::HERO_ACCEPT)),
                (Key(K::F2), Bind::bits(b::SQUAD_HOLD | b::HERO_DECLINE)),
                (Key(K::F3), Bind::bits(b::SQUAD_AT_EASE)),
                (Key(K::F4), Bind::bits(b::SQUAD_MOVE_OUT)),
            ],
        }
    }

    fn of(&self, t: Trigger) -> impl Iterator<Item = Bind> + '_ {
        self.table
            .iter()
            .filter(move |(k, _)| *k == t)
            .map(|(_, b)| *b)
    }
}

impl Default for Bindings {
    fn default() -> Self {
        Bindings::pc_default()
    }
}

#[derive(Debug)]
pub struct Kbm {
    pub bindings: Bindings,
    pub sensitivity: f32,
    pub invert_y: bool,
    held: Vec<Trigger>,
    taps: u32,
    pending: (f64, f64),
    mouse_aim: bool,
    want_grab: bool,
    grabbed: bool,
}

impl Default for Kbm {
    fn default() -> Self {
        Kbm {
            bindings: Bindings::default(),
            sensitivity: 1.0,
            invert_y: false,
            held: Vec::new(),
            taps: 0,
            pending: (0.0, 0.0),
            mouse_aim: false,
            want_grab: false,
            grabbed: false,
        }
    }
}

impl Kbm {
    pub fn follow(&mut self, settings: &crate::profile::Settings) {
        self.sensitivity = settings.mouse_sensitivity;
        self.invert_y = settings.invert_y;
        if self.bindings != settings.keyboard_bindings {
            self.release_keys();
            self.bindings = settings.keyboard_bindings.clone();
        }
    }

    pub fn press(&mut self, t: Trigger, down: bool) {
        let at = self.held.iter().position(|h| *h == t);
        match (down, at) {
            (true, None) => {
                self.taps |= self.bindings.of(t).fold(0, |a, b| a | b.bits);
                self.held.push(t);
            }
            (false, Some(i)) => {
                self.held.swap_remove(i);
            }
            _ => {}
        }
    }

    pub fn tap(&mut self, t: Trigger) {
        self.press(t, true);
        self.press(t, false);
    }

    pub fn wheel(&mut self, dy: f32) {
        if dy > 0.0 {
            self.tap(Trigger::WheelUp);
        } else if dy < 0.0 {
            self.tap(Trigger::WheelDown);
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent, battle: bool, allow_grab: bool) {
        if !battle {
            return;
        }
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => self.press(Trigger::Key(*code), state.is_pressed()),
            WindowEvent::MouseInput { state, button, .. } => {
                if self.grabbed || !state.is_pressed() {
                    self.press(Trigger::Mouse(*button), state.is_pressed());
                } else if allow_grab {
                    self.click_in();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => self.wheel(match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32,
            }),
            _ => {}
        }
    }

    pub fn motion(&mut self, dx: f64, dy: f64) {
        if !self.grabbed {
            return;
        }
        let k = RAD_PER_COUNT * f64::from(self.sensitivity);
        let up = if self.invert_y { dy } else { -dy };
        self.pending.0 += dx * k;
        self.pending.1 += up * k;
        if dx != 0.0 || dy != 0.0 {
            self.mouse_aim = true;
        }
    }

    fn walk(&self) -> (f32, f32) {
        let (mut x, mut y) = (0i32, 0i32);
        for t in &self.held {
            for b in self.bindings.of(*t) {
                x += i32::from(b.walk.0);
                y += i32::from(b.walk.1);
            }
        }
        let (x, y) = (x.signum() as f32, y.signum() as f32);
        if x != 0.0 && y != 0.0 {
            let k = std::f32::consts::FRAC_1_SQRT_2;
            (x * k, y * k)
        } else {
            (x, y)
        }
    }

    pub fn look(&self) -> PadState {
        PadState {
            mx: PadState::quantize_look(self.pending.0),
            my: PadState::quantize_look(self.pending.1),
            buttons: if self.mouse_aim {
                buttons::MOUSE_AIM
            } else {
                0
            },
            ..PadState::default()
        }
    }

    pub fn pad(&mut self, pad: PadState) -> PadState {
        if pad.rx != 0 || pad.ry != 0 {
            self.mouse_aim = false;
        }
        let held = self
            .held
            .iter()
            .flat_map(|t| self.bindings.of(*t))
            .fold(0, |a, b| a | b.bits);
        let (kx, ky) = self.walk();
        let axis = |key: f32, stick: i16| {
            if key != 0.0 && key.abs() > PadState::axis(stick).abs() {
                PadState::quantize(key)
            } else {
                stick
            }
        };
        let look = self.look();
        PadState {
            lx: axis(kx, pad.lx),
            ly: axis(ky, pad.ly),
            buttons: pad.buttons | held | self.taps | look.buttons,
            mx: look.mx,
            my: look.my,
            ..pad
        }
    }

    pub fn consumed(&mut self, pad: &PadState, ticks: u32) {
        if ticks == 0 {
            return;
        }
        self.pending.0 -= f64::from(pad.mx) * f64::from(LOOK_UNIT);
        self.pending.1 -= f64::from(pad.my) * f64::from(LOOK_UNIT);
        self.taps = 0;
    }

    pub fn drop_taps(&mut self) {
        self.taps = 0;
    }

    pub fn release_keys(&mut self) {
        self.held.clear();
        self.taps = 0;
    }

    pub fn grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn click_in(&mut self) {
        self.want_grab = true;
    }

    pub fn sync_grab(&mut self, window: &Window, allowed: bool) {
        let was_grabbed = self.grabbed;
        if let Err(e) = self.sync_grab_with(allowed, |mode| window.set_cursor_grab(mode)) {
            eprintln!("mouse: cannot update the pointer grab ({e})");
        }
        if self.grabbed && !was_grabbed {
            // Wayland accepts a position hint only after locking.
            let size = window.inner_size();
            let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                size.width / 2,
                size.height / 2,
            ));
        }
        if was_grabbed != self.grabbed {
            window.set_cursor_visible(!self.grabbed);
        }
    }

    fn sync_grab_with<E>(
        &mut self,
        allowed: bool,
        mut grab: impl FnMut(CursorGrabMode) -> Result<(), E>,
    ) -> Result<(), E> {
        if !allowed {
            self.want_grab = false;
        }
        let want = self.want_grab && allowed;
        if want == self.grabbed {
            return Ok(());
        }
        if want {
            // A failed request stays free until the next click.
            self.want_grab = false;
            grab(CursorGrabMode::Locked).or_else(|_| grab(CursorGrabMode::Confined))?;
            self.want_grab = true;
            self.grabbed = true;
        } else {
            grab(CursorGrabMode::None)?;
            self.clear_grab();
        }
        Ok(())
    }

    fn clear_grab(&mut self) {
        self.pending = (0.0, 0.0);
        self.grabbed = false;
    }

    pub fn release(&mut self, window: &Window) {
        self.want_grab = false;
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
        self.clear_grab();
    }
}

#[cfg(test)]
mod tests;
