//! Keyboard + mouse to `PadState`: the bindings table, held keys and latched
//! taps, mouse look in `LOOK_UNIT`s, the merge with the controller, pointer lock
//! and menu keys. OURS (public pull request #1); rules in `analysis/plan/FEATURE_SET.md`, Platform & meta
//! (Keyboard + mouse).

use core::fmath::FloatExt;
use core::sim::{buttons, PadState, LOOK_UNIT};
use winit::event::{KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window};

pub const RAD_PER_COUNT: f64 = 0.0006;
/// OURS (the maintainer, 2026-11-14, after the first LAN playtest): the
/// on-foot factor on mouse look. Mouse sensitivity 1.0, the stored and shown
/// default, turns a soldier on foot as 2.8 did before (the playtester's own
/// setting). Vehicles, turrets and starfighters keep factor 1.0, so their
/// look at the default is what it always was
/// (`control_layout::look_scale`).
/// measure: maintainer and playtester feel at 100 % on foot, on Windows, Linux
/// and macOS mice. replace: a per-device calibration if raw counts differ.
pub const ON_FOOT_LOOK_SCALE: f64 = 2.8;
pub const FLYER_COUNTS_PER_SECOND_AT_FULL: f64 = 2048.0;
/// OURS: on macOS a Control press waits this long before the battle sees it,
/// so Control+Command+F (the system's fullscreen key) never rolls the soldier
/// when Control goes down first (the maintainer, 2026-11-14: one fullscreen key
/// per OS, live in battle). A shorter Control tap still rolls, at its release.
/// measure: the gap between Control and Command in a two-finger chord on a
/// Mac keyboard, and whether 0.1 s of roll delay is felt.
pub const MAC_CHORD_WAIT: std::time::Duration = std::time::Duration::from_millis(100);
const FLYER_HOLD_AFTER: f64 = 0.1;
const FLYER_HOLD_FROM: f64 = 1.0;
const FLYER_RATE_TOP: f64 = 1.1;
const FLYER_STICK_CANCEL: f32 = 0.4;
const FLYER_READ_SECONDS: f64 = 1.0 / core::TICK_HZ as f64;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HeldRate {
    rate: f64,
    moving_for: f64,
}

impl HeldRate {
    fn after(self, input: f64, dt: f64, top: f64) -> Self {
        let held = self.rate.abs() > FLYER_HOLD_FROM;
        let opposed = self.rate * input < 0.0;
        let (rate, moving_for) = if !held {
            (input, 0.0)
        } else if opposed {
            let left = self.rate + input * 2.0;
            if left * self.rate > 0.0 {
                (left, self.moving_for + dt)
            } else {
                (0.0, 0.0)
            }
        } else {
            let moving_for = self.moving_for + dt;
            let hold = moving_for > FLYER_HOLD_AFTER && input.abs() < self.rate.abs();
            (if hold { self.rate } else { input }, moving_for)
        };
        HeldRate {
            rate: rate.clamp(-top, top),
            moving_for,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FlyerMouse {
    axes: [HeldRate; 2],
    counts: (f64, f64),
    last_read: (f64, f64),
    next: Option<([HeldRate; 2], (f64, f64))>,
}
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
/// `[ours]` A and D step a value on the pause menu, whose arrows are the cheat
/// code's Morse keys (`game::pause::rows_input`).
pub const KEY_ADJUST_LEFT: u32 = 4096;
pub const KEY_ADJUST_RIGHT: u32 = 8192;
/// The menu edge bits of a value step: those keys, or the left stick.
pub const MENU_ADJUST_LEFT: u32 = 1024;
pub const MENU_ADJUST_RIGHT: u32 = 2048;
/// North (Triangle / Y) as held, so a Triangle already down when the pause
/// menu opens is no Back edge. Never returned as an edge itself.
pub const MENU_NORTH: u32 = 4096;
/// Circle / B, separately from Back, for the profile keyboard symbols page.
pub const MENU_EAST: u32 = 8192;
pub const MENU_MAP: u32 = 16384;
pub const MENU_DETAILS: u32 = 32768;
pub const MENU_HINTS: u32 = 65536;

/// A frame's menu edges from the held bits. With `north_backs` (the ground
/// pause menu, as BF2's ifs_pausemenu) North's own fresh press is Back (32).
pub fn menu_edges(now: u32, prev: u32, tapped: u32, north_backs: bool) -> u32 {
    let mut edges = (now & !prev) | tapped;
    if north_backs && edges & MENU_NORTH != 0 {
        edges |= 32;
    }
    edges & !MENU_NORTH
}

/// The arms the stick held last frame, for `input::stick_arms`' hysteresis. Left
/// and Right come from the value-step bits, because the bare arm bits are the
/// D-pad's and arrows' too: a held Morse key must not latch a half-tilted stick
/// into a value step.
pub fn stick_latch(prev_menu_buttons: u32) -> u32 {
    let mut held = prev_menu_buttons & 3;
    if prev_menu_buttons & MENU_ADJUST_LEFT != 0 {
        held |= 4;
    }
    if prev_menu_buttons & MENU_ADJUST_RIGHT != 0 {
        held |= 8;
    }
    held
}

pub fn menu_key(code: KeyCode) -> u32 {
    match code {
        KeyCode::ArrowUp => 1,
        KeyCode::Tab => MENU_MAP,
        KeyCode::KeyE => MENU_DETAILS,
        KeyCode::KeyR => MENU_HINTS,
        KeyCode::ArrowDown => 2,
        KeyCode::ArrowLeft => 4,
        KeyCode::ArrowRight => 8,
        KeyCode::Enter | KeyCode::NumpadEnter => 16 | KEY_ENTER,
        KeyCode::Backspace => 32,
        KeyCode::Escape => KEY_ESCAPE,
        KeyCode::PageUp => KEY_PAGE_UP,
        KeyCode::PageDown => KEY_PAGE_DOWN,
        KeyCode::Space => KEY_SQUARE,
        KeyCode::KeyA => KEY_ADJUST_LEFT,
        KeyCode::KeyD => KEY_ADJUST_RIGHT,
        _ => 0,
    }
}

pub fn menu_bits(keys: u32, shell: bool) -> u32 {
    let mut bits = keys & (63 | MENU_MAP | MENU_DETAILS | MENU_HINTS);
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
    if keys & KEY_ADJUST_LEFT != 0 {
        bits |= MENU_ADJUST_LEFT;
    }
    if keys & KEY_ADJUST_RIGHT != 0 {
        bits |= MENU_ADJUST_RIGHT;
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

/// Issue #172: F11 switches fullscreen before any binding sees it.
pub const FULLSCREEN_KEY: Trigger = Trigger::Key(KeyCode::F11);

/// Issue #182: the in-battle scoreboard's key. It is read held, as itself,
/// so a context's saved layout (which binds only that context's actions)
/// never leaves the board without its key ([`step_scoreboard`]). A layout
/// that binds an action to it keeps that action alone ([`Kbm::scoreboard_held`]).
pub const SCOREBOARD_KEY: Trigger = Trigger::Key(KeyCode::Tab);

/// One battle frame of the scoreboard's hold (`ui::scoreboard`'s rules):
/// the pad's Select bit and Switch Weapon, and the scoreboard key held.
pub fn step_scoreboard(
    hold: &mut ui::scoreboard::Hold,
    kbm: &Kbm,
    pad: &PadState,
    dt: f32,
) -> bool {
    hold.step(
        pad.buttons & buttons::SPAWN_CLASS_PREV != 0,
        pad.buttons & buttons::SWITCH_WEAPON != 0,
        kbm.scoreboard_held(),
        dt,
    )
}

/// This build runs on macOS (its fullscreen key is Control+Command+F).
pub const MAC: bool = cfg!(target_os = "macos");

/// Issue #172: why F11 holds no action, worded for the OS (`mac`): F11 is
/// the fullscreen key on Windows and Linux, and stays reserved on macOS,
/// whose key is Control+Command+F, so a profile moved between OSes stays
/// valid.
pub fn f11_note(mac: bool) -> String {
    let key = ui::menu::fullscreen_key_name(mac);
    if mac {
        format!("F11 is reserved ({key} switches fullscreen)")
    } else {
        format!("{key} switches fullscreen")
    }
}

/// Issue #172: a key the Controls page refuses to bind on this OS (`mac`),
/// with the line it shows: F11 everywhere, and Command on macOS, which
/// [`Kbm::key`] keeps for the system's shortcuts.
pub fn reserved_key(t: Trigger, mac: bool) -> Option<String> {
    if t == FULLSCREEN_KEY {
        return Some(format!("{}; choose another key", f11_note(mac)));
    }
    if mac && matches!(t, Trigger::Key(KeyCode::SuperLeft | KeyCode::SuperRight)) {
        return Some("Command is kept for system shortcuts on macOS; choose another key".into());
    }
    None
}

impl Bindings {
    /// Issue #172: a layout from before F11 switched fullscreen moves its
    /// F11 action to a free default key (the action's own first); the
    /// legacy layout has no editor, and must stay complete. Some(trigger)
    /// when it moved.
    pub fn free_fullscreen_key(&mut self) -> Option<Trigger> {
        let at = self.table.iter().position(|(t, _)| *t == FULLSCREEN_KEY)?;
        let bind = self.table[at].1;
        let defaults = Self::pc_default().table;
        let own = defaults.iter().filter(|(_, b)| *b == bind).map(|(t, _)| *t);
        let free = own.chain(defaults.iter().map(|(t, _)| *t)).find(|t| {
            !self.table.iter().any(|(used, _)| used == t)
                && (!matches!(t, Trigger::WheelUp | Trigger::WheelDown) || bind.walk == (0, 0))
        })?;
        self.table[at].0 = free;
        Some(free)
    }

    /// Saved layouts may change triggers/order, not the supported action set:
    /// the stock table's, or the one before settings version 1 (a remap
    /// saved then keeps its shape).
    pub fn is_valid(&self) -> bool {
        [Self::pc_default(), Self::pc_default_v0()]
            .into_iter()
            .any(|stock| self.fits(stock.table))
    }

    fn fits(&self, mut remaining: Vec<(Trigger, Bind)>) -> bool {
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
        let defaults = Self::pc_default().table;
        for (trigger, bind) in &defaults {
            if self.table.iter().any(|(_, b)| b == bind) {
                continue;
            }
            // A player's remap may already use a newly introduced default key.
            // Give the new action a free trigger rather than invalidating the saved layout.
            let free = std::iter::once(*trigger)
                .chain(defaults.iter().map(|(t, _)| *t))
                .find(|t| {
                    !self.table.iter().any(|(used, _)| used == t)
                        && (!matches!(t, Trigger::WheelUp | Trigger::WheelDown)
                            || bind.walk == (0, 0))
                });
            if let Some(trigger) = free {
                self.table.push((trigger, *bind));
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

    /// The stock table (settings version 1, the maintainer's choice after the
    /// first LAN playtest, 2026-11-14: Right mouse zooms, the wheel switches
    /// weapons, Left Ctrl rolls). Several triggers may share an action, and
    /// the first one is what the Controls page shows
    /// (`control_layout::keyboard_defaults`): every shown default works on a
    /// three-button mouse, and the thumb buttons are extras. Vehicles and
    /// starfighters do not play this table: see [`Bindings::vehicle_default`].
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
                // Middle mouse first, so the shown default works on any mouse;
                // the back thumb button too.
                (Mouse(MouseButton::Middle), Bind::bits(b::SECONDARY_FIRE)),
                (Mouse(MouseButton::Back), Bind::bits(b::SECONDARY_FIRE)),
                (Mouse(MouseButton::Right), Bind::bits(b::ZOOM)),
                (Key(K::KeyZ), Bind::bits(b::ZOOM)),
                (Key(K::Space), Bind::bits(b::JUMP | b::SPAWN_ACCEPT)),
                (Key(K::Enter), Bind::bits(b::SPAWN_ACCEPT)),
                (Key(K::NumpadEnter), Bind::bits(b::SPAWN_ACCEPT)),
                (Key(K::Backspace), Bind::bits(b::SPAWN_BACK)),
                (Key(K::ControlLeft), Bind::bits(b::ROLL_STANCE)),
                (Key(K::KeyC), Bind::bits(b::ROLL_STANCE)),
                (Key(K::ShiftLeft), Bind::bits(b::SPRINT)),
                (Key(K::KeyR), Bind::bits(b::RELOAD)),
                // The pad's Triangle: use, and the class screens' back
                // (issue #28: it opens and closes the class change at a post).
                (Key(K::KeyE), Bind::bits(b::ACTION | b::SPAWN_BACK)),
                (
                    Trigger::WheelDown,
                    Bind::bits(b::SWITCH_WEAPON | b::SPAWN_CLASS_NEXT),
                ),
                (
                    Trigger::WheelUp,
                    Bind::bits(b::SWITCH_WEAPON | b::SPAWN_CLASS_PREV),
                ),
                (
                    Key(K::KeyQ),
                    Bind::bits(b::SWITCH_WEAPON | b::SPAWN_CLASS_NEXT),
                ),
                (Key(K::Tab), Bind::bits(b::SPAWN_CLASS_PREV)),
                // G first, as above; the forward thumb button too.
                (Key(K::KeyG), Bind::bits(b::SWITCH_SECONDARY)),
                (Mouse(MouseButton::Forward), Bind::bits(b::SWITCH_SECONDARY)),
                (Key(K::KeyV), Bind::bits(b::VIEW_TOGGLE)),
                (Key(K::AltLeft), Bind::bits(b::FLYER_TRICK)),
                (Key(K::CapsLock), Bind::bits(b::FLYER_REVERSE)),
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

    /// What a vehicle or a starfighter without a layout of its own plays
    /// while the profile keeps the stock table: the stock table from before
    /// settings version 1 (Right mouse secondary fire, Middle mouse and Z
    /// zoom), so the 2026-11-14 infantry defaults leave the vehicles'
    /// controls (their own contributor's area) as they were.
    pub fn vehicle_default() -> Bindings {
        Self::pc_default_v0()
    }

    /// The stock table before settings version 1 (2026-11-14), frozen: what
    /// a profile from before then holds when the player never changed it,
    /// and the table vehicles and starfighters keep ([`Bindings::vehicle_default`]).
    pub fn pc_default_v0() -> Bindings {
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
                (Key(K::AltLeft), Bind::bits(b::FLYER_TRICK)),
                (Key(K::CapsLock), Bind::bits(b::FLYER_REVERSE)),
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
    /// The control context's factor on mouse look ([`ON_FOOT_LOOK_SCALE`] on
    /// foot, 1.0 in a vehicle or starfighter; [`Kbm::swap_context`]). The
    /// default 1.0 is the raw scale.
    pub look_scale: f64,
    pub invert_y: bool,
    held: Vec<Trigger>,
    /// Issue #182: the bits a held trigger gained when the table changed
    /// under it ([`Kbm::swap_context`]), kept out of the pad until it is
    /// let go. A context change never presses a button.
    swap_held: Vec<(Trigger, u32)>,
    taps: u32,
    /// A spawn-screen click's pick, kept like a tap until a tick takes it.
    pick: Option<core::sim::ScreenPick>,
    pending: (f64, f64),
    mouse_aim: bool,
    want_grab: bool,
    grabbed: bool,
    /// Issue #172: `grabbed` may not be the system's after a display mode
    /// change; the next sync sends a lock or a release either way.
    grab_stale: bool,
    flyer: FlyerMouse,
    /// macOS's chord rule ([`Kbm::key`]): Command is the system's modifier.
    mac: MacChords,
}

/// macOS: Command (either Super key) belongs to the system's shortcuts, so a
/// key pressed while it is down never reaches the bindings, and a Control
/// press waits [`MAC_CHORD_WAIT`] for a Command that would make it part of
/// Control+Command+F. Off on other OSes.
#[derive(Debug, Default)]
struct MacChords {
    on: bool,
    /// The Super keys held.
    command: u8,
    /// Keys held since a Command chord took them; they reach the bindings
    /// only after they are released and pressed again.
    swallowed: Vec<Trigger>,
    /// A Control press waiting for a possible Command.
    waiting: Option<(Trigger, std::time::Instant)>,
}

impl Default for Kbm {
    fn default() -> Self {
        Kbm {
            bindings: Bindings::default(),
            sensitivity: 1.0,
            look_scale: 1.0,
            invert_y: false,
            held: Vec::new(),
            swap_held: Vec::new(),
            taps: 0,
            pick: None,
            pending: (0.0, 0.0),
            mouse_aim: false,
            want_grab: false,
            grabbed: false,
            grab_stale: false,
            flyer: FlyerMouse::default(),
            mac: MacChords {
                on: MAC,
                ..MacChords::default()
            },
        }
    }
}

impl Kbm {
    /// The profile's mouse settings. The bindings belong to the control
    /// context ([`Kbm::swap_context`] with `control_layout::context_bindings`),
    /// so a settings change never installs the on-foot table in a vehicle.
    pub fn follow(&mut self, settings: &crate::profile::Settings) {
        self.sensitivity = settings.mouse_sensitivity;
        self.invert_y = settings.invert_y;
    }

    /// Enter a control context: its bindings and look factor. Held keys and
    /// buttons stay held (a trigger held across getting into or out of a
    /// vehicle reads through the new table); only latched taps are dropped.
    ///
    /// Issue #182: a held key keeps only the bits it gave before the change;
    /// the bits the new table adds wait until it is let go and pressed again.
    /// The on-foot layout's use key opens a post's class screen, whose
    /// table (the spawn screens') also makes it back: still held, it must
    /// not back out of the screen it just opened.
    pub fn swap_context(&mut self, bindings: &Bindings, look_scale: f64) {
        self.look_scale = look_scale;
        if &self.bindings != bindings {
            self.taps = 0;
            let swap_held = self
                .held
                .iter()
                .filter_map(|&t| {
                    let gave = self.held_bits(t);
                    let gains = bindings.of(t).fold(0, |a, b| a | b.bits) & !gave;
                    (gains != 0).then_some((t, gains))
                })
                .collect();
            self.swap_held = swap_held;
            self.bindings.clone_from(bindings);
        }
    }

    /// The bits held trigger `t` gives the pad now: its bindings, less what
    /// it gained from a context change while held ([`Kbm::swap_context`]).
    fn held_bits(&self, t: Trigger) -> u32 {
        let waiting = self
            .swap_held
            .iter()
            .find(|(h, _)| *h == t)
            .map_or(0, |(_, bits)| *bits);
        self.bindings.of(t).fold(0, |a, b| a | b.bits) & !waiting
    }

    pub fn set_bindings(&mut self, bindings: &Bindings) {
        if &self.bindings != bindings {
            self.release_keys();
            self.bindings.clone_from(bindings);
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
                self.swap_held.retain(|(h, _)| *h != t);
            }
            _ => {}
        }
    }

    /// A keyboard key in battle. Off macOS it is [`Kbm::press`]; on macOS the
    /// chord rule ([`MacChords`]) runs first.
    pub fn key(&mut self, code: KeyCode, down: bool, now: std::time::Instant) {
        let t = Trigger::Key(code);
        if !self.mac.on {
            self.press(t, down);
            return;
        }
        if matches!(code, KeyCode::SuperLeft | KeyCode::SuperRight) {
            if down {
                self.mac.command = self.mac.command.saturating_add(1);
                // Control was the chord's first key: it never rolls.
                if let Some((waiting, _)) = self.mac.waiting.take() {
                    self.mac.swallowed.push(waiting);
                }
            } else {
                self.mac.command = self.mac.command.saturating_sub(1);
            }
            return;
        }
        if down {
            // A key repeat changes nothing (a held or waiting key stays so).
            if self.mac.swallowed.contains(&t)
                || self.held.contains(&t)
                || self.mac.waiting.is_some_and(|(w, _)| w == t)
            {
                return;
            }
            if self.mac.command > 0 {
                self.mac.swallowed.push(t);
            } else if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) {
                self.chord_due(now);
                // The other Control key is still waiting: no Command joined
                // it, so it goes down now rather than being lost.
                if let Some((other, _)) = self.mac.waiting.take() {
                    self.press(other, true);
                }
                self.mac.waiting = Some((t, now));
            } else {
                self.press(t, true);
            }
            return;
        }
        if let Some(i) = self.mac.swallowed.iter().position(|s| *s == t) {
            self.mac.swallowed.swap_remove(i);
        } else if self.mac.waiting.is_some_and(|(w, _)| w == t) {
            // A quick Control tap: it rolls now, at its release.
            self.mac.waiting = None;
            self.tap(t);
        } else {
            self.press(t, false);
        }
    }

    /// A key let go outside battle input (for example while the Controls
    /// page captures a binding): it goes through [`Kbm::key`], so a
    /// Command, waiting or swallowed key leaves no chord state behind.
    pub fn key_released(&mut self, code: KeyCode, now: std::time::Instant) {
        self.key(code, false, now);
    }

    /// macOS: a waiting Control press that no Command joined in
    /// [`MAC_CHORD_WAIT`] goes down now.
    pub fn chord_due(&mut self, now: std::time::Instant) {
        if let Some((t, at)) = self.mac.waiting {
            if now.saturating_duration_since(at) >= MAC_CHORD_WAIT {
                self.mac.waiting = None;
                self.press(t, true);
            }
        }
    }

    /// Turn the macOS chord rule on or off (it follows the build's OS).
    pub fn set_mac_chords(&mut self, on: bool) {
        self.mac = MacChords {
            on,
            ..MacChords::default()
        };
    }

    pub fn tap(&mut self, t: Trigger) {
        self.press(t, true);
        self.press(t, false);
    }

    pub fn tap_bits(&mut self, bits: u32) {
        self.taps |= bits;
    }

    /// A pointer pick on a spawn screen for the next tick's pad (the last
    /// one wins until a tick takes it).
    pub fn set_pick(&mut self, pick: core::sim::ScreenPick) {
        self.pick = Some(pick);
    }

    /// The spawn-screen pick waiting for the next tick, if any.
    pub fn pending_pick(&self) -> Option<core::sim::ScreenPick> {
        self.pick
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
            } => self.key(*code, state.is_pressed(), std::time::Instant::now()),
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
        let k = RAD_PER_COUNT * f64::from(self.sensitivity) * self.look_scale;
        let up = if self.invert_y { dy } else { -dy };
        self.pending.0 += dx * k;
        self.pending.1 += up * k;
        self.flyer.counts.0 += dx;
        self.flyer.counts.1 += up;
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
        self.flyer = FlyerMouse::default();
        self.merge(pad)
    }

    pub fn flyer_pad(&mut self, pad: PadState) -> PadState {
        self.pending = (0.0, 0.0);
        let cancel = PadState::axis(pad.rx).abs() > FLYER_STICK_CANCEL
            || PadState::axis(pad.ry).abs() > FLYER_STICK_CANCEL;
        if cancel || !self.grabbed {
            self.flyer.axes = [HeldRate::default(); 2];
        }
        let moved = self.flyer.counts != (0.0, 0.0);
        let read = if moved {
            self.flyer.counts
        } else {
            self.flyer.last_read
        };
        let last_read = if moved { self.flyer.counts } else { (0.0, 0.0) };
        let scale = f64::from(self.sensitivity) / FLYER_COUNTS_PER_SECOND_AT_FULL;
        let top = FLYER_RATE_TOP / scale.fmax(f64::MIN_POSITIVE);
        let step = |axis: HeldRate, counts: f64| {
            axis.after(counts / FLYER_READ_SECONDS, FLYER_READ_SECONDS, top)
        };
        let axes = [
            step(self.flyer.axes[0], read.0),
            step(self.flyer.axes[1], read.1),
        ];
        self.flyer.next = Some((axes, last_read));
        let stick = |rate: f64, held: i16| {
            let mine = (rate * scale) as f32;
            if PadState::axis(held).abs() > mine.abs() {
                held
            } else {
                PadState::quantize(mine)
            }
        };
        let (rx, ry) = (stick(axes[0].rate, pad.rx), stick(axes[1].rate, pad.ry));
        let merged = self.merge(pad);
        PadState { rx, ry, ..merged }
    }

    fn merge(&mut self, pad: PadState) -> PadState {
        self.chord_due(std::time::Instant::now());
        if pad.rx != 0 || pad.ry != 0 {
            self.mouse_aim = false;
        }
        let held = self.held.iter().fold(0, |a, t| a | self.held_bits(*t));
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
            pick: self.pick.or(pad.pick),
            ..pad
        }
    }

    pub fn consumed(&mut self, pad: &PadState, ticks: u32) {
        if ticks == 0 {
            return;
        }
        self.pending.0 -= f64::from(pad.mx) * f64::from(LOOK_UNIT);
        self.pending.1 -= f64::from(pad.my) * f64::from(LOOK_UNIT);
        if let Some((axes, last_read)) = self.flyer.next.take() {
            self.flyer.axes = axes;
            self.flyer.last_read = last_read;
            self.flyer.counts = (0.0, 0.0);
        }
        self.taps = 0;
        self.pick = None;
    }

    pub fn drop_taps(&mut self) {
        self.taps = 0;
        self.pick = None;
    }

    /// Discard camera input accumulated while a modal screen owns input.
    pub fn clear_look(&mut self) {
        self.pending = (0.0, 0.0);
        self.flyer = Default::default();
    }

    pub fn release_keys(&mut self) {
        let on = self.mac.on;
        self.set_mac_chords(on);
        self.held.clear();
        self.swap_held.clear();
        self.taps = 0;
        self.pick = None;
    }

    pub fn grabbed(&self) -> bool {
        self.grabbed
    }

    /// The scoreboard key is down ([`SCOREBOARD_KEY`]) and this context's
    /// table leaves it free: unbound, or bound only to the scoreboard's own
    /// pad bit (the stock tables' Select). A layout that puts an action on
    /// it plays that action alone. A key a macOS Command chord took is not.
    pub fn scoreboard_held(&self) -> bool {
        self.held.contains(&SCOREBOARD_KEY)
            && self
                .bindings
                .of(SCOREBOARD_KEY)
                .all(|b| b.walk == (0, 0) && b.bits & !buttons::SPAWN_CLASS_PREV == 0)
    }

    /// A click asked for the pointer and the next sync takes it.
    pub fn wants_grab(&self) -> bool {
        self.want_grab
    }

    pub fn click_in(&mut self) {
        self.want_grab = true;
    }

    /// Returns true when it set the system cursor's visibility, which the
    /// overlay's cursor plan then applies again (issue #182).
    pub fn sync_grab(&mut self, window: &Window, allowed: bool) -> bool {
        let stale = self.grab_stale;
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
        let touched = was_grabbed != self.grabbed || stale;
        if touched {
            window.set_cursor_visible(!self.grabbed);
        }
        touched
    }

    /// [`Kbm::sync_grab`]'s rule with the window's grab call given as
    /// `grab` (the window, or a test standing in for it).
    pub fn sync_grab_with<E>(
        &mut self,
        allowed: bool,
        mut grab: impl FnMut(CursorGrabMode) -> Result<(), E>,
    ) -> Result<(), E> {
        if !allowed {
            self.want_grab = false;
        }
        let want = self.want_grab && allowed;
        if want == self.grabbed && !self.grab_stale {
            return Ok(());
        }
        // One try: a failed request is not repeated every frame.
        self.grab_stale = false;
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

    /// Issue #172: a display mode change may drop the system's grab while
    /// this still holds it, or keep it while this has let go. The next sync
    /// always sends one: a lock if the player had the pointer and play
    /// allows it, else a release (a pause menu never keeps a stale lock).
    pub fn forget_grab(&mut self) {
        self.clear_grab();
        self.grab_stale = true;
    }

    fn clear_grab(&mut self) {
        self.pending = (0.0, 0.0);
        self.flyer = FlyerMouse::default();
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
