//! Which input icons the control prompts draw, and the art the repo ships for
//! them: the setting saved in the profile, the device Auto
//! follows, and the icons themselves. `ui::input_icons` is the lookup the
//! screens call; this module feeds it and answers its `icon_*` sheet names.
//!
//! The icons are SVG text, compiled in and rasterized the first time a prompt
//! shows one; `HudView` keeps the texture from then on, so a menu frame pays
//! for a table lookup only. Sources and licences:
//! `assets/input_icons/README.md`.

use serde::{Deserialize, Serialize};
use ui::input_icons::{IconSet, Icons, PadButton};
use winit::event::{ElementState, WindowEvent};

/// The Input icons setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconChoice {
    /// Follow the device in use.
    #[default]
    Auto,
    KeyboardMouse,
    Ds2,
    Ds4,
    Ds5,
    XboxOriginal,
    XboxOne,
}

impl IconChoice {
    pub const ALL: [IconChoice; 7] = [
        IconChoice::Auto,
        IconChoice::KeyboardMouse,
        IconChoice::Ds2,
        IconChoice::Ds4,
        IconChoice::Ds5,
        IconChoice::XboxOriginal,
        IconChoice::XboxOne,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IconChoice::Auto => "Auto",
            IconChoice::KeyboardMouse => "Keyboard and Mouse",
            IconChoice::Ds2 => "PS2 (DS2)",
            IconChoice::Ds4 => "DS4",
            IconChoice::Ds5 => "DS5",
            IconChoice::XboxOriginal => "Original Xbox",
            IconChoice::XboxOne => "Xbox One",
        }
    }

    /// The next choice in `dir`'s direction, wrapping.
    pub fn cycled(self, dir: i32) -> IconChoice {
        let n = Self::ALL.len() as i32;
        let at = Self::ALL.iter().position(|c| *c == self).unwrap_or(0) as i32;
        Self::ALL[(at + dir).rem_euclid(n) as usize]
    }
}

/// The device the player last used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    KeyboardMouse,
    Pad(input::PadFamily),
}

/// The set `choice` draws while `device` is in use.
pub fn set_for(choice: IconChoice, device: Device) -> IconSet {
    match choice {
        IconChoice::Auto => match device {
            Device::KeyboardMouse => IconSet::KeyboardMouse,
            Device::Pad(input::PadFamily::Ds2) => IconSet::Ds2,
            Device::Pad(input::PadFamily::Ds4) => IconSet::Ds4,
            Device::Pad(input::PadFamily::Ds5) => IconSet::Ds5,
            Device::Pad(input::PadFamily::XboxOriginal) => IconSet::XboxOriginal,
            Device::Pad(input::PadFamily::XboxOne) => IconSet::XboxOne,
            Device::Pad(input::PadFamily::Generic) => IconSet::GenericPad,
        },
        IconChoice::KeyboardMouse => IconSet::KeyboardMouse,
        IconChoice::Ds2 => IconSet::Ds2,
        IconChoice::Ds4 => IconSet::Ds4,
        IconChoice::Ds5 => IconSet::Ds5,
        IconChoice::XboxOriginal => IconSet::XboxOriginal,
        IconChoice::XboxOne => IconSet::XboxOne,
    }
}

/// A gilrs button's place on the pad; `None` for those no binding can show.
pub fn pad_button(b: gilrs::Button) -> Option<PadButton> {
    use gilrs::Button as B;
    Some(match b {
        B::South => PadButton::South,
        B::East => PadButton::East,
        B::West => PadButton::West,
        B::North => PadButton::North,
        B::LeftTrigger => PadButton::L1,
        B::RightTrigger => PadButton::R1,
        B::LeftTrigger2 => PadButton::L2,
        B::RightTrigger2 => PadButton::R2,
        B::LeftThumb => PadButton::L3,
        B::RightThumb => PadButton::R3,
        B::Select => PadButton::Select,
        B::Start => PadButton::Start,
        B::DPadUp => PadButton::DPadUp,
        B::DPadDown => PadButton::DPadDown,
        B::DPadLeft => PadButton::DPadLeft,
        B::DPadRight => PadButton::DPadRight,
        _ => return None,
    })
}

/// The button `map` puts `bit` on, as a pad place.
fn bound(map: &input::Mapping, bit: u32) -> Option<PadButton> {
    map.table
        .iter()
        .find(|(_, bits)| bits & bit != 0)
        .and_then(|(b, _)| pad_button(*b))
}

/// What the screens draw: the set and the spawn prompts as `map` binds them.
pub fn icons(set: IconSet, map: &input::Mapping) -> Icons {
    use core::sim::buttons;
    let fallback = Icons::of(set);
    Icons {
        set,
        spawn_accept: bound(map, buttons::SPAWN_ACCEPT).unwrap_or(fallback.spawn_accept),
        spawn_back: bound(map, buttons::SPAWN_BACK).unwrap_or(fallback.spawn_back),
        class_next: bound(map, buttons::SPAWN_CLASS_NEXT).unwrap_or(fallback.class_next),
        class_prev: bound(map, buttons::SPAWN_CLASS_PREV).unwrap_or(fallback.class_prev),
    }
}

/// Whether a key event is the player pressing that key now: not a repeat, and
/// not the press winit makes up for a key already down when the window gains
/// focus, which Alt-Tabbing back onto a pad game would otherwise read as use.
fn is_key_use(state: ElementState, repeat: bool, synthetic: bool) -> bool {
    state == ElementState::Pressed && !repeat && !synthetic
}

/// A press that says the player is on the keyboard or the mouse. Moving the
/// pointer does not count: a nudged mouse must not change a pad player's
/// prompts.
pub fn is_keyboard_or_mouse_use(event: &WindowEvent) -> bool {
    match event {
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => is_key_use(event.state, event.repeat, *is_synthetic),
        WindowEvent::MouseInput { state, .. } => *state == ElementState::Pressed,
        WindowEvent::MouseWheel { .. } => true,
        _ => false,
    }
}

/// The device Auto follows: the last one used. Before any input it is the
/// connected pad, else the keyboard. A pad that leaves hands back to another
/// connected pad, else the keyboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceTracker {
    device: Option<Device>,
    /// [`input::ActivePad::deliberate`] when last read.
    seen: u64,
    /// Pads connected when last read.
    pads: usize,
}

impl DeviceTracker {
    pub fn keyboard_or_mouse(&mut self) {
        self.device = Some(Device::KeyboardMouse);
    }

    pub fn device(&self) -> Device {
        self.device.unwrap_or(Device::KeyboardMouse)
    }

    /// The set `choice` draws now.
    pub fn set(&self, choice: IconChoice) -> IconSet {
        set_for(choice, self.device())
    }

    /// Catch up with the pads: once a frame, a counter and a count compared;
    /// the pad is asked for its family only when one of them moved.
    pub fn sync(
        &mut self,
        gilrs: Option<&gilrs::Gilrs>,
        active: &input::ActivePad<gilrs::GamepadId>,
    ) {
        let Some(g) = gilrs else {
            return;
        };
        self.step(active.deliberate, g.gamepads().count(), || {
            input::active_gamepad(g, active).map(|(_, pad)| input::pad_family(&pad))
        });
    }

    /// [`DeviceTracker::sync`] without the hardware.
    fn step(
        &mut self,
        deliberate: u64,
        pads: usize,
        family: impl FnOnce() -> Option<input::PadFamily>,
    ) {
        let pressed = deliberate != self.seen;
        let plugged = pads != self.pads;
        self.seen = deliberate;
        self.pads = pads;
        let keyboard = self.device == Some(Device::KeyboardMouse);
        // Plugging a pad in is not using it; only a press takes the prompts
        // off a keyboard the player is on.
        if pressed || (self.device.is_none() || (plugged && !keyboard)) {
            let pad = if pressed || pads > 0 { family() } else { None };
            self.device = Some(pad.map_or(Device::KeyboardMouse, Device::Pad));
        }
    }
}

macro_rules! kenney {
    ($file:literal) => {
        include_str!(concat!("../assets/input_icons/kenney/", $file, ".svg"))
    };
}

macro_rules! drawn {
    ($file:literal) => {
        include_str!(concat!("../assets/input_icons/drawn/", $file, ".svg"))
    };
}

/// A sheet's SVG. The disc's own PS2 sheets are here too: a bake that lacks
/// them draws PlayStation symbols, not nothing. The original Xbox's come from
/// the player's disc when baked (`bake::xbox_icons`); these are what shows
/// otherwise: its face and White / Black buttons drawn for this project, the
/// rest shared with the Xbox One.
fn svg(sheet: &str) -> Option<&'static str> {
    Some(match sheet {
        "icon_ps_cross" | "btna" => kenney!("playstation_button_color_cross"),
        "icon_ps_circle" => kenney!("playstation_button_color_circle"),
        "icon_ps_square" => kenney!("playstation_button_color_square"),
        "icon_ps_triangle" | "btnb" => kenney!("playstation_button_color_triangle"),
        "icon_ps_l1" => kenney!("playstation_trigger_l1"),
        "icon_ps_r1" => kenney!("playstation_trigger_r1"),
        "icon_ps_l2" => kenney!("playstation_trigger_l2"),
        "icon_ps_r2" => kenney!("playstation_trigger_r2"),
        "icon_ps_l3" => kenney!("playstation_stick_l_press"),
        "icon_ps_r3" => kenney!("playstation_stick_r_press"),
        "icon_ps_dpad" | "btn_directional_pad" => kenney!("playstation_dpad_all"),
        "icon_ps_dpad_lr" | "btn_directional_pad_lr" => kenney!("playstation_dpad_horizontal"),
        "icon_ps_dpad_up" => kenney!("playstation_dpad_up"),
        "icon_ps_dpad_down" => kenney!("playstation_dpad_down"),
        "icon_ps_dpad_left" => kenney!("playstation_dpad_left"),
        "icon_ps_dpad_right" => kenney!("playstation_dpad_right"),
        "icon_ps2_select" => kenney!("playstation3_button_select"),
        "icon_ps2_start" => kenney!("playstation3_button_start"),
        "icon_ds4_share" => kenney!("playstation4_button_share"),
        "icon_ds4_options" => kenney!("playstation4_button_options"),
        "icon_ds5_create" => kenney!("playstation5_button_create"),
        "icon_ds5_options" => kenney!("playstation5_button_options"),

        "icon_xbone_a" => kenney!("xbox_button_color_a"),
        "icon_xbone_b" => kenney!("xbox_button_color_b"),
        "icon_xbone_x" => kenney!("xbox_button_color_x"),
        "icon_xbone_y" => kenney!("xbox_button_color_y"),
        "icon_xbone_lb" => kenney!("xbox_lb"),
        "icon_xbone_rb" => kenney!("xbox_rb"),
        "icon_xbone_lt" | "icon_duke_l" => kenney!("xbox_lt"),
        "icon_xbone_rt" | "icon_duke_r" => kenney!("xbox_rt"),
        "icon_xbone_l3" | "icon_duke_l3" => kenney!("xbox_stick_l_press"),
        "icon_xbone_r3" | "icon_duke_r3" => kenney!("xbox_stick_r_press"),
        "icon_xbone_view" => kenney!("xbox_button_view"),
        "icon_xbone_menu" => kenney!("xbox_button_menu"),
        "icon_duke_back" => kenney!("xbox_button_back"),
        "icon_duke_start" => kenney!("xbox_button_start"),
        "icon_xbone_dpad" | "icon_duke_dpad" => kenney!("xbox_dpad_all"),
        "icon_xbone_dpad_lr" | "icon_duke_dpad_lr" => kenney!("xbox_dpad_horizontal"),
        "icon_xbone_dpad_up" | "icon_duke_dpad_up" => kenney!("xbox_dpad_up"),
        "icon_xbone_dpad_down" | "icon_duke_dpad_down" => kenney!("xbox_dpad_down"),
        "icon_xbone_dpad_left" | "icon_duke_dpad_left" => kenney!("xbox_dpad_left"),
        "icon_xbone_dpad_right" | "icon_duke_dpad_right" => kenney!("xbox_dpad_right"),

        "icon_duke_a" => drawn!("duke_a"),
        "icon_duke_b" => drawn!("duke_b"),
        "icon_duke_x" => drawn!("duke_x"),
        "icon_duke_y" => drawn!("duke_y"),
        "icon_duke_white" => drawn!("duke_white"),
        "icon_duke_black" => drawn!("duke_black"),

        "icon_pad_south" => drawn!("pad_south"),
        "icon_pad_east" => drawn!("pad_east"),
        "icon_pad_west" => drawn!("pad_west"),
        "icon_pad_north" => drawn!("pad_north"),

        "icon_kb_enter" => kenney!("keyboard_enter"),
        "icon_kb_backspace" => kenney!("keyboard_backspace"),
        "icon_kb_space" => kenney!("keyboard_space"),
        "icon_kb_arrows" => kenney!("keyboard_arrows_all"),
        "icon_kb_arrows_lr" => kenney!("keyboard_arrows_horizontal"),
        _ => return None,
    })
}

/// The pixel size icons are rasterized at. A prompt shows one 20-50 px high,
/// and the HUD's sampler has no mips: this keeps the minification near 2.
const ICON_PX: u32 = 48;
/// The share of the sheet an icon's ink fills: the disc's own icons fill
/// theirs, Kenney's stand inside a margin.
const ICON_FILL: f32 = 0.96;

/// `sheet` as straight-alpha RGBA8 with its size; `None` when it is not one of
/// the icons, or its SVG fails to parse.
pub fn rasterize(sheet: &str) -> Option<(Vec<u8>, u32, u32)> {
    let tree = resvg::usvg::Tree::from_str(svg(sheet)?, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(ICON_PX, ICON_PX)?;
    let ink = tree.root().abs_stroke_bounding_box();
    let side = ICON_PX as f32;
    let k = side * ICON_FILL / ink.width().max(ink.height());
    let at = |start: f32, len: f32| (side - len * k) * 0.5 - start * k;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_row(
            k,
            0.0,
            0.0,
            k,
            at(ink.x(), ink.width()),
            at(ink.y(), ink.height()),
        ),
        &mut pixmap.as_mut(),
    );
    Some((pixmap.take_demultiplied(), ICON_PX, ICON_PX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use input::PadFamily;
    use ui::input_icons::{pad_sheet, Prompt, ALL_SETS};

    /// Every sheet the lookup can answer has art with ink in it, so no set
    /// can draw an untextured quad even with nothing baked.
    #[test]
    fn every_set_has_art_for_every_prompt() {
        let mut sheets: Vec<&str> = Vec::new();
        for set in ALL_SETS {
            sheets.extend(Prompt::ALL.map(|p| Icons::of(set).sheet(p)));
            sheets.extend(PadButton::ALL.map(|b| pad_sheet(set, b)));
        }
        sheets.sort_unstable();
        sheets.dedup();
        let started = std::time::Instant::now();
        for sheet in &sheets {
            let (rgba, w, h) = rasterize(sheet).unwrap_or_else(|| panic!("{sheet}: no art"));
            assert_eq!(rgba.len(), (w * h * 4) as usize, "{sheet}");
            let ink = rgba.chunks_exact(4).filter(|p| p[3] > 128).count();
            assert!(ink > 100, "{sheet}: {ink} opaque texels");
        }
        eprintln!(
            "{} icons rasterized in {:?}",
            sheets.len(),
            started.elapsed()
        );
    }

    /// The setting's rows come round in order, both ways.
    #[test]
    fn the_choices_cycle_and_wrap() {
        let mut c = IconChoice::Auto;
        for want in IconChoice::ALL.iter().skip(1) {
            c = c.cycled(1);
            assert_eq!(c, *want);
        }
        assert_eq!(c.cycled(1), IconChoice::Auto);
        assert_eq!(IconChoice::Auto.cycled(-1), IconChoice::XboxOne);
    }

    /// Auto draws the device in use; a fixed choice ignores it.
    #[test]
    fn auto_follows_the_device_and_a_choice_holds() {
        let pad = |f| Device::Pad(f);
        let auto = |d| set_for(IconChoice::Auto, d);
        assert_eq!(auto(Device::KeyboardMouse), IconSet::KeyboardMouse);
        assert_eq!(auto(pad(PadFamily::Ds2)), IconSet::Ds2);
        assert_eq!(auto(pad(PadFamily::Ds4)), IconSet::Ds4);
        assert_eq!(auto(pad(PadFamily::Ds5)), IconSet::Ds5);
        assert_eq!(auto(pad(PadFamily::XboxOriginal)), IconSet::XboxOriginal);
        assert_eq!(auto(pad(PadFamily::XboxOne)), IconSet::XboxOne);
        assert_eq!(auto(pad(PadFamily::Generic)), IconSet::GenericPad);
        assert_eq!(
            set_for(IconChoice::Ds5, Device::KeyboardMouse),
            IconSet::Ds5
        );
        assert_eq!(
            set_for(IconChoice::KeyboardMouse, pad(PadFamily::Ds4)),
            IconSet::KeyboardMouse
        );
    }

    /// The spawn prompts follow a remap, the default stays the PS2's.
    #[test]
    fn the_spawn_prompts_read_the_bindings() {
        let mut map = input::Mapping::default();
        assert_eq!(icons(IconSet::Ds2, &map), Icons::of(IconSet::Ds2));
        let accept = map
            .table
            .iter()
            .position(|(_, bits)| bits & core::sim::buttons::SPAWN_ACCEPT != 0)
            .unwrap();
        map.remap(accept, gilrs::Button::West);
        let got = icons(IconSet::XboxOne, &map);
        assert_eq!(got.spawn_accept, PadButton::West);
        assert_eq!(got.sheet(Prompt::SpawnAccept), "icon_xbone_x");
        let next = map
            .table
            .iter()
            .position(|(_, bits)| bits & core::sim::buttons::SPAWN_CLASS_NEXT != 0)
            .unwrap();
        map.remap(next, gilrs::Button::LeftTrigger);
        assert_eq!(icons(IconSet::XboxOne, &map).class_keys(), "LB / View");
    }

    /// Only a real press or the wheel says keyboard and mouse: not a held
    /// key's repeat or the press made up on focus, a release or the pointer.
    #[test]
    fn only_real_presses_count_as_the_keyboard_or_mouse() {
        use winit::event::{DeviceId, MouseButton, MouseScrollDelta, TouchPhase};
        assert!(is_key_use(ElementState::Pressed, false, false));
        assert!(!is_key_use(ElementState::Pressed, true, false));
        assert!(!is_key_use(ElementState::Pressed, false, true));
        assert!(!is_key_use(ElementState::Released, false, false));

        let device_id = DeviceId::dummy();
        let click = |state| WindowEvent::MouseInput {
            device_id,
            state,
            button: MouseButton::Left,
        };
        assert!(is_keyboard_or_mouse_use(&click(ElementState::Pressed)));
        assert!(!is_keyboard_or_mouse_use(&click(ElementState::Released)));
        assert!(is_keyboard_or_mouse_use(&WindowEvent::MouseWheel {
            device_id,
            delta: MouseScrollDelta::LineDelta(0.0, 1.0),
            phase: TouchPhase::Moved,
        }));
        assert!(!is_keyboard_or_mouse_use(&WindowEvent::CursorMoved {
            device_id,
            position: (1.0, 1.0).into(),
        }));
    }

    /// The last device used wins, and a pad needs a press to take over from
    /// the keyboard.
    #[test]
    fn the_tracker_follows_the_last_device_used() {
        let ds5 = || Some(PadFamily::Ds5);
        let none = || None;
        // Nothing connected: the keyboard.
        let mut t = DeviceTracker::default();
        t.step(0, 0, none);
        assert_eq!(t.device(), Device::KeyboardMouse);

        // A pad already connected at start, untouched: that pad.
        let mut t = DeviceTracker::default();
        t.step(0, 1, ds5);
        assert_eq!(t.device(), Device::Pad(PadFamily::Ds5));
        // A key: the keyboard; the idle pad does not take it back.
        t.keyboard_or_mouse();
        t.step(0, 1, ds5);
        assert_eq!(t.device(), Device::KeyboardMouse);
        // Another pad plugged in is not used yet.
        t.step(0, 2, ds5);
        assert_eq!(t.device(), Device::KeyboardMouse);
        // A press takes it.
        t.step(1, 2, ds5);
        assert_eq!(t.device(), Device::Pad(PadFamily::Ds5));
        // Steady state does not ask the pad again.
        t.step(1, 2, || panic!("asked the pad with nothing moved"));
        // It leaves and none is left: the keyboard.
        t.step(1, 0, none);
        assert_eq!(t.device(), Device::KeyboardMouse);
    }
}
