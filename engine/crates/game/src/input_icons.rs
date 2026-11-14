//! Which button icons the control prompts draw, and the art the repo ships for
//! them: the setting saved in the profile, the device Auto
//! follows, and the icons themselves. `ui::input_icons` is the lookup the
//! screens call; this module feeds it and answers its `icon_*` sheet names.
//!
//! The icons are SVG text, compiled in and rasterized the first time a prompt
//! shows one at a size; `HudView` keeps the texture from then on, so a menu
//! frame pays for a table lookup only. Sources and licences:
//! `assets/input_icons/README.md`.

use core::fmath::FloatExt;
use serde::{Deserialize, Serialize};
use ui::input_icons::{IconSet, Icons, PadButton};
use winit::event::{ElementState, WindowEvent};

/// The Button prompts setting.
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
            IconChoice::KeyboardMouse => "Keyboard and mouse",
            IconChoice::Ds2 => "PlayStation 2",
            IconChoice::Ds4 => "PlayStation 4",
            IconChoice::Ds5 => "PlayStation 5",
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

        "icon_kb_enter" => kenney!("keyboard_enter"),
        "icon_kb_backspace" => kenney!("keyboard_backspace"),
        "icon_kb_space" => kenney!("keyboard_space"),
        "icon_kb_arrows" => kenney!("keyboard_arrows_all"),
        "icon_kb_arrows_lr" => kenney!("keyboard_arrows_horizontal"),
        _ => return None,
    })
}

/// The sizes icons are rasterized at. The HUD's sampler has no mips, so a sheet
/// far larger than its quad aliases and one smaller goes soft: [`icon_px`] picks
/// the next size up, and the short list bounds what a dragged window can upload.
const ICON_SIZES: [u32; 8] = [24, 32, 48, 64, 96, 128, 192, 256];
/// The share of the sheet an icon's ink fills: the disc's own icons fill
/// theirs, Kenney's stand inside a margin.
const ICON_FILL: f32 = 0.96;
/// What shows through an icon's enclosed cut-outs. Kenney's letters and symbols
/// are holes in the button, which over a grey backdrop read as grey on colour.
const HOLE: [u8; 3] = [16, 18, 22];
/// The side an icon's holes are found at: a thin stroke at a small size has no
/// fully opaque texel to stop the search.
const HOLE_SEARCH_PX: u32 = 192;

/// The size to rasterize an icon drawn `drawn` window pixels high.
pub fn icon_px(drawn: f32) -> u32 {
    ICON_SIZES
        .into_iter()
        .find(|px| *px as f32 >= drawn)
        .unwrap_or(ICON_SIZES[ICON_SIZES.len() - 1])
}

/// Whether `sheet` is one of the icons [`rasterize`] can draw.
pub fn has_art(sheet: &str) -> bool {
    svg(sheet).is_some()
}

/// Back an image's enclosed transparent texels with [`HOLE`], in place.
/// `premul` is premultiplied RGBA, `side` texels square. Everything the image's
/// edge reaches through less than full alpha, the outer soft rim among it, stays.
fn fill_holes(premul: &mut [u8], side: usize) {
    let mut outside = vec![false; side * side];
    let mut todo: Vec<usize> = (0..side)
        .flat_map(|i| [i, side * (side - 1) + i, side * i, side * i + side - 1])
        .collect();
    while let Some(at) = todo.pop() {
        if outside[at] || premul[at * 4 + 3] == 255 {
            continue;
        }
        outside[at] = true;
        let (x, y) = (at % side, at / side);
        if x > 0 {
            todo.push(at - 1);
        }
        if x + 1 < side {
            todo.push(at + 1);
        }
        if y > 0 {
            todo.push(at - side);
        }
        if y + 1 < side {
            todo.push(at + side);
        }
    }
    for (p, out) in premul.chunks_exact_mut(4).zip(outside) {
        if !out && p[3] < 255 {
            let rest = 255 - u32::from(p[3]);
            for (c, hole) in p.iter_mut().zip(HOLE) {
                *c += (u32::from(hole) * rest / 255) as u8;
            }
            p[3] = 255;
        }
    }
}

/// `sheet` at `px` square as straight-alpha RGBA8 with its size; `None` when it
/// is not one of the icons, or its SVG fails to parse.
pub fn rasterize(sheet: &str, px: u32) -> Option<(Vec<u8>, u32, u32)> {
    let tree = resvg::usvg::Tree::from_str(svg(sheet)?, &resvg::usvg::Options::default()).ok()?;
    let scale = HOLE_SEARCH_PX.div_ceil(px).max(1);
    let big = px * scale;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(big, big)?;
    let ink = tree.root().abs_stroke_bounding_box();
    let side = big as f32;
    let k = side * ICON_FILL / ink.width().fmax(ink.height());
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
    fill_holes(pixmap.data_mut(), big as usize);
    // Down to `px`, a box over premultiplied texels, then straight alpha.
    let (scale, px_us, data) = (scale as usize, px as usize, pixmap.data());
    let mut rgba = Vec::with_capacity(px_us * px_us * 4);
    for y in 0..px_us {
        for x in 0..px_us {
            let mut sum = [0u32; 4];
            for row in y * scale..(y + 1) * scale {
                let from = (row * px_us * scale + x * scale) * 4;
                for p in data[from..from + scale * 4].chunks_exact(4) {
                    for (s, c) in sum.iter_mut().zip(p) {
                        *s += u32::from(*c);
                    }
                }
            }
            let n = (scale * scale) as u32;
            let alpha = sum[3] / n;
            for c in &sum[..3] {
                let c = (c / n * 255).checked_div(alpha).unwrap_or(0);
                rgba.push(c.min(255) as u8);
            }
            rgba.push(alpha as u8);
        }
    }
    Some((rgba, px, px))
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
            let (rgba, w, h) = rasterize(sheet, 48).unwrap_or_else(|| panic!("{sheet}: no art"));
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

    /// A letter cut out of a button reads against any backdrop: its hole is
    /// backed, and what surrounds the button stays clear, at every size.
    #[test]
    fn an_icons_cut_outs_are_backed_and_its_outside_stays_clear() {
        for px in ICON_SIZES {
            let (rgba, ..) = rasterize("icon_xbone_a", px).unwrap();
            let (side, texel) = (px as usize, |x: usize, y: usize| (y * px as usize + x) * 4);
            assert_eq!(rgba[texel(0, 0) + 3], 0, "{px}: the corner");
            let (middle, top) = (side / 2, side / 2 - side / 8);
            let c = &rgba[texel(middle, top)..texel(middle, top) + 4];
            assert_eq!(c[3], 255, "{px}: the letter has no hole left");
            assert!(c[1] < 80, "{px}: the letter is dark: {c:?}");
            let rim = &rgba[texel(middle, side / 16)..texel(middle, side / 16) + 4];
            assert!(rim[1] > 120 && rim[3] == 255, "{px}: the button: {rim:?}");
        }
    }

    /// The next size up, so a quad never magnifies; a huge one takes the most.
    #[test]
    fn an_icon_is_rasterized_at_the_next_size_up() {
        assert_eq!(icon_px(10.0), 24);
        assert_eq!(icon_px(18.0), 24);
        assert_eq!(icon_px(27.0), 32);
        assert_eq!(icon_px(54.0), 64);
        assert_eq!(icon_px(5000.0), 256);
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
        for c in IconChoice::ALL {
            assert!(!c.label().contains("DS"), "{c:?} names a pad plainly");
        }
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
        // A GameSir G7 Pro and a pad nothing names: the Xbox One's letters.
        let family = |name, hw, kind| input::family_of(name, hw, kind);
        for f in [
            family(
                "GameSir G7 Pro",
                Some((0x3537, 0x1022)),
                input::PadKind::Sdl,
            ),
            family("Twin USB Joystick", None, input::PadKind::Raw),
        ] {
            assert_eq!(auto(pad(f)), IconSet::XboxOne);
        }
        // And Sony's own pads stay PlayStation.
        let sony = family(
            "Wireless Controller",
            Some((0x054c, 0x0ce6)),
            input::PadKind::Sdl,
        );
        assert_eq!(auto(pad(sony)), IconSet::Ds5);
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
