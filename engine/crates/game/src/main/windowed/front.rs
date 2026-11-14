//! The windowed shell's front screens (card BSHLA, `ui::shell_ps2`): the
//! PS2's Press START, Main Menu, Singleplayer and Options over the shell
//! movie, with the shell music; Instant Action and its Session Options
//! (`ui::shell`, `ui::shell_ia`, `analysis/re-bf2-menus.md` §1-§2) and the
//! settings page (`ui::menu` in its shell form). A child of `windowed` so it
//! reaches the `App` state.
//!
//! The menus' clicks (`analysis/re-bf2-menus.md` §4.1) are asked for by
//! `ui::shell_ps2` and, for the pages of the port's own, here; all of them go
//! through `ShellMenus::cues` (the select click's 0.1 s limit) and play from
//! the CORE.LVL bank (`SoundLib::load_shell`).

use super::{App, Camera};
use ui::menu::{ControlContext, PausePage, SettingsPage};
use ui::shell_feel::MenuSound;

/// The BF2 PS2 shell's front screens (card BSHLA, `ui::shell_ps2`): Press
/// START, the Main Menu, Singleplayer and Options over the background
/// movie, with the shell music; Instant Action opens the PS2's screen
/// (`ui::shell`) and Game / Audio Options the settings page (`ui::menu` in
/// its shell form).
pub(super) struct FrontShell {
    menus: ui::shell_ps2::ShellMenus,
    movie: Option<game::shell_movie::ShellMovie>,
    /// Built at the first frame with a renderer.
    view: Option<game::hud_feed::HudView>,
    /// The Instant Action screen: built at its first Enter, kept (with its
    /// playlist and session options) for the process, as the PS2 keeps
    /// `gPickedMapList` (§1.3).
    ia: Option<ui::shell::InstantAction>,
    /// It is up.
    ia_open: bool,
    /// The settings page is up, at this cursor.
    settings: Option<(PausePage, usize)>,
    settings_animation: ui::pause_draw::Animation,
    sounds: Option<game::sounds::SoundLib>,
    pub(super) music: Option<audio::MusicVoice>,
    /// `shell_soundtrack`'s gain (x its bus) and takes, and which one plays
    /// when (re-bf2-menus §4.4).
    music_takes: Option<(f32, Vec<audio::StreamTake>)>,
    playlist: ui::shell_feel::ShellPlaylist,
    /// The random take picker's state (the shell is not the sim: seeded
    /// from the clock, menuref §9.3.9).
    music_rng: u64,
    /// A held D-pad arm's repeat (menuref: 0.62 s, then every 0.167 s).
    repeat: ui::shell_feel::HoldRepeat,
    /// The music level was raised from off: the take of the moment again.
    music_replay: bool,
    /// A page of the port's own covered the screen last frame.
    was_covered: bool,
}

pub(super) struct Controls {
    context: ControlContext,
    layout: Layout,
    pub(super) cursor: usize,
    capture: Option<Capture>,
    original: Vec<Binding>,
    message: Option<String>,
}

enum Layout {
    Keyboard(Vec<Option<game::kbm::Trigger>>),
    Gamepad(Vec<Option<gilrs::Button>>),
}

impl Layout {
    fn complete(&self) -> Vec<Binding> {
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
enum Binding {
    Keyboard(game::kbm::Trigger),
    Gamepad(gilrs::Button),
}

struct Capture {
    row: usize,
    ready: bool,
    controller: Option<gilrs::GamepadId>,
    candidate: Option<(Binding, std::time::Instant)>,
}

const CONTROL_COMMAND_ROWS: usize = 2;
const CANCEL_HOLD: std::time::Duration = std::time::Duration::from_millis(600);

impl Controls {
    pub(super) fn new(
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
        let original = layout.complete();
        Self {
            context,
            layout,
            original,
            cursor: 0,
            capture: None,
            message: None,
        }
    }

    fn keyboard(&self) -> bool {
        matches!(self.layout, Layout::Keyboard(_))
    }

    fn len(&self) -> usize {
        match &self.layout {
            Layout::Keyboard(items) => items.len(),
            Layout::Gamepad(items) => items.len(),
        }
    }

    fn missing(&self) -> Option<usize> {
        match &self.layout {
            Layout::Keyboard(items) => items.iter().position(Option::is_none),
            Layout::Gamepad(items) => items.iter().position(Option::is_none),
        }
    }

    fn cancel(&mut self, reason: &str) -> MenuSound {
        self.capture = None;
        self.message = Some(reason.into());
        MenuSound::Cancel
    }

    fn assign(&mut self, bind: Binding) -> MenuSound {
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

const PAD_BUTTONS: [gilrs::Button; 17] = [
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

impl FrontShell {
    pub(super) fn new(assets_dir: &std::path::Path) -> Self {
        let texts = game::event_feed::Texts::load(assets_dir, "bf2");
        let text = |key: &str, english: &str| {
            texts
                .bf2_text(key)
                .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
        };
        let movie = game::shell_movie::ShellMovie::load(assets_dir);
        if movie.is_none() {
            eprintln!("shell: no movie baked (bake common); the menus draw over black");
        }
        let sounds = game::sounds::SoundLib::load_shell(
            &game::shell_movie::shell_dir(assets_dir),
            assets_dir,
        );
        let music_takes = sounds
            .as_ref()
            .and_then(|sl| sl.stream_takes(ui::shell_ps2::MUSIC));
        FrontShell {
            menus: ui::shell_ps2::ShellMenus::new(&text),
            movie,
            view: None,
            ia: None,
            ia_open: false,
            settings: None,
            settings_animation: Default::default(),
            sounds,
            music: None,
            music_takes,
            playlist: Default::default(),
            music_rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x9e37_79b9, |d| d.as_nanos() as u64)
                | 1,
            repeat: Default::default(),
            music_replay: false,
            was_covered: false,
        }
    }

    /// The shell is left for a battle: its music stops, and starts with a
    /// new random take when the shell comes back.
    pub(super) fn stop_music(&mut self) {
        self.music = None;
        self.playlist = Default::default();
    }

    /// Ask for a menu click (rate limited, `ShellMenus::cues`).
    fn cue(&mut self, s: MenuSound) {
        self.menus.cues.play(s);
    }

    /// Play the clicks asked for this frame, and keep the sound library's
    /// clock (its `PlayInterval`s) on the shell's.
    fn play_cues(&mut self, audio: Option<&mut audio::Audio>, dt: f32) {
        let cues = self.menus.cues.take();
        let Some(sl) = self.sounds.as_mut() else {
            return;
        };
        sl.tick(dt, glam::Vec3::ZERO, &mut Vec::new());
        let mut audio = audio;
        for s in cues {
            sl.play_2d(audio.as_deref_mut(), s.name());
        }
    }
}

/// The music's random pick: xorshift64 over `state`.
fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// A settings page frame's click (re-bf2-menus §4.1 [V]): a cursor move
/// clicks; Left / Right on a Game Options row plays accept, even at a clamp
/// (`common_ifs_opt_general` main.f6 / f8 pc 113); the music row plays
/// nothing but the music (`common_ifs_opt_sound`); Cross on a toggle
/// accept. Back is the Options stack's exit click on its re-Enter.
fn setting_sound(
    model: &ui::menu::MenuModel,
    input: ui::menu::MenuInput,
    to: usize,
) -> Option<MenuSound> {
    use ui::menu::Row;
    if input.back {
        return None;
    }
    let rows = ui::menu::rows(model);
    if to != model.cursor.min(rows.len().saturating_sub(1)) {
        return Some(MenuSound::Select);
    }
    match rows.get(to)? {
        Row::Music | Row::Resume => None,
        Row::InvertY | Row::InputIcons | Row::FlightControls | Row::CareerAwards
            if input.confirm || input.left || input.right =>
        {
            Some(MenuSound::Accept)
        }
        _ if input.left || input.right => Some(MenuSound::Accept),
        _ => None,
    }
}

/// A settings page click's sound, as its pad press's ([`setting_sound`]): a
/// toggle or a value step accept, the music's step nothing but the music,
/// Back nothing here.
fn shell_click_sound(action: ui::menu::MenuAction) -> Option<MenuSound> {
    use ui::menu::MenuAction;
    match action {
        MenuAction::Denied => Some(MenuSound::Error),
        MenuAction::SetMusic(_) | MenuAction::Resume | MenuAction::Quit | MenuAction::ClosePage => {
            None
        }
        _ => Some(MenuSound::Accept),
    }
}

impl App {
    /// Session Options were left (`push_prefs`): AI Difficulty is the
    /// profile's tier, saved.
    fn keep_session_prefs(&mut self, options: &ui::shell::SessionOptions) {
        self.profile.settings.difficulty = game::shell_ia::port_difficulty(options.difficulty);
        self.profile.settings.team_size_pct = options.con_numbots as u32;
        self.profile.settings.reinforcement_pct = options.con_mult as u32;
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
    }

    pub(super) fn activate_control_row(&mut self, row: usize) -> Option<MenuSound> {
        let controls = self.controls.as_mut()?;
        if controls.capture.is_some() {
            return None;
        }
        controls.cursor = row;
        match row {
            0 => {
                if let Some(first) = controls.missing() {
                    controls.cursor = CONTROL_COMMAND_ROWS + first;
                    controls.message = Some("Assign every action before leaving this page".into());
                    return Some(MenuSound::Error);
                }
                let controls = self.controls.take()?;
                if controls.original == controls.layout.complete() {
                    return Some(MenuSound::Exit);
                }
                let settings = &mut self.profile.settings;
                match controls.layout {
                    Layout::Keyboard(items) => settings
                        .keyboard_layouts
                        .set(controls.context, items.into_iter().flatten().collect()),
                    Layout::Gamepad(items) => settings
                        .gamepad_layouts
                        .set(controls.context, items.into_iter().flatten().collect()),
                }
                self.refresh_context_bindings();
                self.kbm.release_keys();
                self.sprint_latch = input::SprintLatch::default();
                if let Err(e) = self.profile.save() {
                    eprintln!("profile autosave failed: {e:#}");
                }
                Some(MenuSound::Exit)
            }
            1 => {
                controls.layout = if controls.keyboard() {
                    Layout::Keyboard(game::control_layout::keyboard_defaults(
                        controls.context,
                        &game::kbm::Bindings::default(),
                    ))
                } else {
                    Layout::Gamepad(game::control_layout::gamepad_defaults(
                        controls.context,
                        &input::Mapping::default(),
                    ))
                };
                controls.original.clear();
                controls.message = Some("Default bindings restored".into());
                Some(MenuSound::Accept)
            }
            _ => {
                if row >= CONTROL_COMMAND_ROWS + controls.len() {
                    return None;
                }
                controls.message = None;
                controls.capture = Some(Capture {
                    row: row - CONTROL_COMMAND_ROWS,
                    ready: false,
                    controller: None,
                    candidate: None,
                });
                Some(MenuSound::Accept)
            }
        }
    }

    pub(super) fn reset_control_device(&mut self) {
        if let Some(capture) = self.controls.as_mut().and_then(|c| c.capture.as_mut()) {
            if capture.controller.take().is_some() {
                capture.ready = false;
                capture.candidate = None;
            }
        }
    }

    pub(super) fn step_controls(&mut self, input: ui::menu::MenuInput) -> Option<MenuSound> {
        let controls = self.controls.as_mut()?;
        let keyboard = controls.keyboard();
        if let Some(capture) = controls.capture.as_mut() {
            let now = std::time::Instant::now();
            if capture
                .candidate
                .is_some_and(|(_, start)| now.duration_since(start) >= CANCEL_HOLD)
            {
                return Some(controls.cancel("Binding change cancelled"));
            }
            if keyboard {
                return None;
            }
            let pad = self
                .gilrs
                .as_ref()
                .and_then(|g| input::active_gamepad(g, &self.active_pad));
            let Some((id, pad)) = pad else {
                return Some(controls.cancel("Controller disconnected; binding unchanged"));
            };
            if capture.controller != Some(id) {
                capture.controller = Some(id);
                capture.ready = false;
                capture.candidate = None;
            }
            if input::is_down(&pad, gilrs::Button::Start) {
                return Some(controls.cancel("Binding change cancelled"));
            }
            let pressed = PAD_BUTTONS
                .iter()
                .copied()
                .find(|&b| input::is_down(&pad, b));
            if !capture.ready {
                capture.ready = pressed.is_none();
                return None;
            }
            if let Some((Binding::Gamepad(button), _)) = capture.candidate {
                if !input::is_down(&pad, button) {
                    return Some(controls.assign(Binding::Gamepad(button)));
                }
            } else if let Some(button) = pressed {
                capture.candidate = Some((Binding::Gamepad(button), now));
            }
            return None;
        }

        let count = CONTROL_COMMAND_ROWS + controls.len();
        let from = controls.cursor;
        if input.up {
            controls.cursor = (controls.cursor + count - 1) % count;
        }
        if input.down {
            controls.cursor = (controls.cursor + 1) % count;
        }
        let cursor = controls.cursor;
        if input.back || input.confirm {
            return self.activate_control_row(if input.back { 0 } else { cursor });
        }
        (from != cursor).then_some(MenuSound::Select)
    }

    pub(super) fn controls_model(
        &self,
        icons: ui::input_icons::Icons,
    ) -> Option<ui::menu::MenuModel> {
        let controls = self.controls.as_ref()?;
        let mut labels = vec![
            ("Back".into(), String::new()),
            ("Restore defaults".into(), String::new()),
        ];
        let set = self.icon_device.set(self.profile.settings.input_icons);
        let values = match &controls.layout {
            Layout::Keyboard(items) => items
                .iter()
                .map(|t| t.map(game::control_layout::keyboard_name))
                .collect::<Vec<_>>(),
            Layout::Gamepad(items) => items
                .iter()
                .map(|t| t.map(|b| game::control_layout::gamepad_name(b, set)))
                .collect(),
        };
        labels.extend(
            game::control_layout::actions(controls.context, controls.keyboard())
                .into_iter()
                .zip(values)
                .map(|(a, value)| (a.name.into(), value.unwrap_or_else(|| "Unassigned".into()))),
        );
        Some(ui::menu::MenuModel {
            bindings: Some(ui::menu::BindingRows {
                title: controls.context.title(),
                labels,
                capture: controls.capture.is_some(),
                message: controls.message.clone(),
            }),
            cursor: controls.cursor,
            icons,
            ..Default::default()
        })
    }

    fn play_binding_sound(&mut self, sound: MenuSound) {
        if matches!(self.screen, super::Screen::Shell) {
            if let Some(front) = self.front.as_mut() {
                front.cue(sound);
            }
        } else if let Some(sounds) = self.sounds.as_mut() {
            sounds.play_2d(self.audio.as_mut(), sound.name());
        }
    }

    pub(super) fn capture_keyboard(&mut self, event: &winit::event::WindowEvent) -> bool {
        use game::kbm::Trigger;
        use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};
        let Some(controls) = self.controls.as_mut() else {
            return false;
        };
        let keyboard = controls.keyboard();
        let Some(capture) = controls.capture.as_mut() else {
            return false;
        };
        if matches!(event, WindowEvent::Focused(false)) {
            controls.cancel("Binding change cancelled on focus loss");
            return false;
        }
        let (trigger, state) = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return keyboard;
                };
                if code == KeyCode::Escape && event.state.is_pressed() {
                    let sound = controls.cancel("Binding change cancelled");
                    self.play_binding_sound(sound);
                    return true;
                }
                (Trigger::Key(code), event.state)
            }
            WindowEvent::MouseInput { state, button, .. } => (Trigger::Mouse(*button), *state),
            WindowEvent::MouseWheel { delta, .. } if keyboard => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if y != 0.0 {
                    let trigger = if y > 0.0 {
                        Trigger::WheelUp
                    } else {
                        Trigger::WheelDown
                    };
                    let sound = controls.assign(Binding::Keyboard(trigger));
                    self.play_binding_sound(sound);
                }
                return true;
            }
            _ => return false,
        };
        if !keyboard {
            return false;
        }
        if state == ElementState::Pressed {
            if capture.candidate.is_none()
                && !matches!(event, WindowEvent::KeyboardInput { event, .. } if event.repeat)
            {
                capture.candidate = Some((Binding::Keyboard(trigger), std::time::Instant::now()));
            }
        } else if let Some((Binding::Keyboard(candidate), start)) = capture.candidate {
            if trigger == candidate {
                capture.candidate = None;
                let sound = if start.elapsed() >= CANCEL_HOLD {
                    controls.cancel("Binding change cancelled")
                } else {
                    controls.assign(Binding::Keyboard(trigger))
                };
                self.play_binding_sound(sound);
            }
        }
        true
    }

    /// Shell frame: the PS2 front screens (card BSHLA), the Instant Action
    /// screen or the settings page; navigate, maybe launch, draw.
    pub(super) fn redraw_shell(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        dt: f32,
    ) {
        let mut edges = self.menu_edges();
        let mut front = self
            .front
            .take()
            .unwrap_or_else(|| FrontShell::new(&self.assets_dir));
        // A held arm presses again (menuref: from 0.62 s, every 0.167 s).
        edges |= front.repeat.step(self.prev_menu_buttons & 15, dt);
        let mut input = ui::menu::MenuInput {
            up: edges & 1 != 0,
            down: edges & 2 != 0,
            left: edges & 4 != 0,
            right: edges & 8 != 0,
            confirm: edges & 16 != 0,
            back: edges & 32 != 0,
        };
        let mut start = edges & 64 != 0;
        if front.view.is_none() {
            if let Some(r) = &self.renderer {
                front.view = game::hud_feed::HudView::for_renderer(&self.assets_dir, r)
                    .map_err(|e| {
                        eprintln!("shell: no BF2 HUD bake ({e:#}); the menus are not drawn")
                    })
                    .ok();
            }
        }
        // Hover acts only on fresh pointer motion or a click (`Pointer`).
        let wheel = self.shell_pointer.take_wheel();
        let pointer = self.shell_pointer.take_frame();
        let click = pointer.is_some_and(|(_, c)| c);
        if let (Some((point, _)), Some((w, h)), Some(view)) = (
            pointer,
            self.renderer.as_ref().map(|r| r.size()),
            front.view.as_ref(),
        ) {
            if !front.ia_open && front.settings.is_none() && self.controls.is_none() {
                if let Some(row) = view.shell_row_at(&front.menus, point, [w as f32, h as f32]) {
                    front.menus.focus_row(row);
                    input.confirm |= click;
                } else if click {
                    start |= view.shell_prompt_at(&front.menus, point, [w as f32, h as f32]);
                }
            }
        }
        let mut launch = None;
        let mut quit = false;
        let mut settings_model = None;
        let mut controls_model = None;
        let stack_frame = !front.ia_open && self.controls.is_none() && front.settings.is_none();
        if !stack_frame {
            front.menus.tick(dt);
        }
        if let Some(ia) = front.ia.as_mut().filter(|_| front.ia_open) {
            let mut ia_input = ui::shell::IaInput {
                menu: input,
                l1: edges & game::kbm::MENU_L1 != 0,
                r1: edges & game::kbm::MENU_R1 != 0,
            };
            if wheel != 0.0 {
                let lines = if wheel > 0.0 {
                    -(wheel.round().max(1.0) as isize)
                } else {
                    (-wheel).round().max(1.0) as isize
                };
                let point = self.shell_pointer.pos();
                let size = self.renderer.as_ref().map(|r| r.size());
                ia.scroll_pointer(point, size.map(|(w, h)| [w as f32, h as f32]), lines);
            }
            let pointer = if wheel != 0.0 && !click {
                None
            } else {
                pointer
            };
            if let (Some((point, _)), Some((w, h))) =
                (pointer, self.renderer.as_ref().map(|r| r.size()))
            {
                if let Some(target) = front
                    .view
                    .as_ref()
                    .and_then(|view| view.instant_target(ia, point, [w as f32, h as f32]))
                {
                    if ia.focus_pointer(target) {
                        ia_input.menu.confirm |= click
                            && !matches!(
                                target,
                                ui::shell::PointerTarget::Option(..)
                                    | ui::shell::PointerTarget::ScrollLeft(..)
                                    | ui::shell::PointerTarget::ScrollRight(..)
                                    | ui::shell::PointerTarget::ScrollOption(..)
                            );
                        if click {
                            match target {
                                ui::shell::PointerTarget::ScrollLeft(delta) => {
                                    ia.scroll_left(delta);
                                }
                                ui::shell::PointerTarget::ScrollRight(delta) => {
                                    ia.scroll_right(delta);
                                }
                                ui::shell::PointerTarget::ScrollOption(delta) => {
                                    ia.scroll_option(delta);
                                }
                                ui::shell::PointerTarget::Option(_, right) => {
                                    ia_input.menu.left |= !right;
                                    ia_input.menu.right |= right;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            match ia.step(ia_input, dt) {
                Some(ui::shell::IaCommand::Back) => front.ia_open = false,
                Some(ui::shell::IaCommand::Launch(rows)) => launch = Some((rows, ia.options)),
                Some(ui::shell::IaCommand::Prefs) => {
                    let options = ia.options;
                    self.keep_session_prefs(&options);
                }
                None => {}
            }
            // The screen's own clicks (menuref sound table); Back to
            // Singleplayer clicks as that stack's re-Enter (`uncover`).
            for s in ia.take_clicks() {
                front.menus.cues.play(s);
            }
        } else if self.controls.is_some() {
            if let Some(sound) = self.step_controls(input) {
                front.cue(sound);
            }
            let icons = self.current_icons();
            controls_model = self.controls_model(icons);
            front.settings_animation.advance(dt);
            if let Some(model) = controls_model.as_mut().filter(|_| !input.back) {
                let native = front.view.as_ref().is_some_and(|v| v.can_draw_pause());
                if native {
                    if wheel != 0.0 {
                        ui::pause_draw::scroll(model, wheel);
                        if let Some(controls) = self.controls.as_mut() {
                            controls.cursor = model.cursor;
                        }
                    }
                    if let Some((point, true)) = pointer {
                        if let Some((w, h)) = self.renderer.as_ref().map(|r| r.size()) {
                            if let Some(ui::pause_draw::Target::Row(row)) = ui::pause_draw::target(
                                model,
                                &front.settings_animation,
                                point,
                                [w as f32, h as f32],
                            ) {
                                if let Some(sound) = self.activate_control_row(row) {
                                    front.cue(sound);
                                }
                                controls_model = self.controls_model(icons);
                            }
                        }
                    }
                }
            }
        } else if let Some((page, cursor)) = front.settings {
            let icons = self.current_icons();
            let settings = &self.profile.settings;
            let mut model = ui::menu::MenuModel {
                assist_intensity: settings.assist_intensity,
                difficulty: game::shell_ia::ia_level(settings.difficulty),
                effects_volume: settings.effects_volume,
                dialogue_volume: settings.dialogue_volume,
                fps_counter: ui::menu::FpsCounter::from_index(settings.fps_counter),
                team_size_pct: settings.team_size_pct,
                reinforcements_pct: settings.reinforcement_pct,
                population_note: game::conquest_defaults::menu_note(
                    settings.team_size_pct,
                    settings.reinforcement_pct,
                ),
                fov_scale: settings.fov_scale,
                base_fov_deg: game::walk::chase_fov_y().to_degrees(),
                music_volume: settings.music_volume,
                rumble_strength: settings.rumble_strength,
                rumble_on_fire: settings.rumble_on_fire,
                career_awards_disabled: !settings.career_awards,
                mouse_sensitivity: settings.mouse_sensitivity,
                pad_look_sensitivity: settings.pad_look_sensitivity,
                rc_flight_controls: settings.flight_controls == input::FlightControls::RcMode2,
                invert_y: settings.invert_y,
                low_power_resolution: settings.low_power_resolution,
                terrain_lod: settings.terrain_lod,
                scene_softness: settings.scene_softness,
                input_icons_label: settings.input_icons.label(),
                icons,
                first_person: settings.viewpoint.is_first(),
                cursor,
                page: Some(page),
                in_shell: Some(String::new()),
                ..Default::default()
            };
            front.settings_animation.advance(dt);
            let native = front.view.as_ref().is_some_and(|v| v.can_draw_pause());
            let pointer_hit = if native && click && !input.back {
                pointer.and_then(|(point, _)| {
                    let (w, h) = self.renderer.as_ref()?.size();
                    ui::pause_draw::target(
                        &model,
                        &front.settings_animation,
                        point,
                        [w as f32, h as f32],
                    )
                })
            } else {
                None
            };
            if wheel != 0.0 && native {
                ui::pause_draw::scroll(&mut model, wheel);
            }
            let (cursor, acts) = if let Some(hit) = pointer_hit {
                let result = ui::pause_draw::click(&mut model, hit);
                if let Some(sound) = result.1.first().copied().and_then(shell_click_sound) {
                    front.cue(sound);
                }
                result
            } else {
                let result = ui::menu::navigate(&model, input);
                if let Some(sound) = result
                    .1
                    .contains(&ui::menu::MenuAction::Denied)
                    .then_some(MenuSound::Error)
                    .or_else(|| setting_sound(&model, input, result.0))
                    .or_else(|| result.1.first().copied().and_then(shell_click_sound))
                {
                    front.cue(sound);
                }
                result
            };
            front.settings = Some((page, cursor));
            for a in acts {
                self.apply_shell_setting(&mut front, a);
            }
            settings_model =
                front
                    .settings
                    .filter(|_| self.controls.is_none())
                    .map(|(page, cursor)| ui::menu::MenuModel {
                        cursor,
                        page: Some(page),
                        difficulty: game::shell_ia::ia_level(self.profile.settings.difficulty),
                        effects_volume: self.profile.settings.effects_volume,
                        dialogue_volume: self.profile.settings.dialogue_volume,
                        fps_counter: ui::menu::FpsCounter::from_index(
                            self.profile.settings.fps_counter,
                        ),
                        rumble_strength: self.profile.settings.rumble_strength,
                        rumble_on_fire: self.profile.settings.rumble_on_fire,
                        career_awards_disabled: !self.profile.settings.career_awards,
                        rc_flight_controls: self.profile.settings.flight_controls
                            == input::FlightControls::RcMode2,
                        fov_scale: self.profile.settings.fov_scale,
                        first_person: self.profile.settings.viewpoint.is_first(),
                        low_power_resolution: self.profile.settings.low_power_resolution,
                        ..model
                    });
        } else {
            match front.menus.step(input, start, dt) {
                Some(ui::shell_ps2::ShellCommand::InstantAction) => {
                    match front.ia.as_mut() {
                        Some(ia) => ia.enter(),
                        None => {
                            front.ia = Some(game::shell_ia::instant_action(
                                &self.assets_dir,
                                &self.missions_dir,
                                self.profile.settings.difficulty,
                            ))
                        }
                    }
                    if let Some(ia) = front.ia.as_mut() {
                        ia.options.num_bots = self.profile.settings.team_size_pct as i32;
                        ia.options.con_numbots = self.profile.settings.team_size_pct as i32;
                        ia.options.con_mult = self.profile.settings.reinforcement_pct as i32;
                    }
                    front.ia_open = true;
                }
                Some(ui::shell_ps2::ShellCommand::Settings(page)) => {
                    front.settings = Some((PausePage::Settings(page), 0));
                }
                Some(ui::shell_ps2::ShellCommand::Quit) => quit = true,
                None => {}
            }
        }
        // The music (`ifs_start` sets `shell_soundtrack`, §2): a random take
        // each time with the shell's gap between them, at its gain x the
        // `shellmusic` bus x the profile's level (re-bf2-menus §4.4, menuref
        // §9.3.9).
        let secs: Vec<f32> = front
            .music_takes
            .as_ref()
            .map_or_else(Vec::new, |(_, takes)| {
                takes.iter().map(audio::StreamTake::secs).collect()
            });
        let mut rng = front.music_rng;
        let due = {
            let mut pick = |n: usize| (xorshift(&mut rng) % n.max(1) as u64) as usize;
            front.playlist.step(dt, &secs, &mut pick)
        };
        front.music_rng = rng;
        if let Some((gain, takes)) = &front.music_takes {
            let level = self.profile.settings.music_volume;
            // A level raised from off: the take of the moment starts again.
            let replay = std::mem::take(&mut front.music_replay);
            let due = due.or_else(|| front.playlist.playing(&secs).filter(|_| replay));
            if let (Some(k), Some(audio)) = (due, self.audio.as_mut()) {
                front.music = None;
                if level > 0.0 {
                    front.music = audio
                        .play_stream(
                            takes[k].clone(),
                            game::sounds::level_db(gain * level),
                            false,
                        )
                        .map_err(|e| eprintln!("shell music: {e:#}"))
                        .ok();
                }
            }
        }
        front.menus.covered = front.ia_open || front.settings.is_some() || self.controls.is_some();
        let movie_name = front.menus.movie();
        let mut has_movie = false;
        if let Some(m) = front.movie.as_mut() {
            m.play(movie_name);
            if let Some((rgba, w, h)) = m.frame(dt) {
                if let Some(v) = front.view.as_mut() {
                    v.set_movie_frame(rgba, w, h);
                }
            }
            has_movie = m.shown;
        }
        // The IA screen has its own backdrop and no movie (§1.1); Session
        // Options keep `shell_sub_left`.
        let ia = front
            .ia
            .as_ref()
            .filter(|_| front.ia_open && front.settings.is_none() && self.controls.is_none());
        let movie_shown = has_movie && ia.is_none_or(|ia| ia.page != ui::shell::Page::Missions);
        let icons = self.current_icons();
        let mut control_clicked = None;
        let mut setting_clicked = Vec::new();
        if let (Some(r), Some(overlay), Some(window)) =
            (&mut self.renderer, &mut self.overlay, &self.window)
        {
            let (w, h) = r.size();
            let native_options = front.view.as_ref().is_some_and(|v| v.can_draw_pause());
            if let Some(v) = front.view.as_mut() {
                v.set_icons(icons);
                match ia {
                    Some(ia) => v.update_instant(&front.menus, ia, movie_shown, [w, h]),
                    None => {
                        if let Some(model) = controls_model
                            .as_ref()
                            .or(settings_model.as_ref())
                            .filter(|_| native_options)
                        {
                            v.update_shell_options(
                                &front.menus,
                                has_movie,
                                model,
                                &front.settings_animation,
                                [w, h],
                            );
                        } else {
                            v.update_shell(&front.menus, has_movie, [w, h]);
                        }
                    }
                }
            }
            let warning = self.pad_warning.as_deref();
            let paint = overlay.begin(window, |ctx| {
                ui::menu::draw_fps_counter(
                    ctx,
                    ui::menu::FpsCounter::from_index(self.profile.settings.fps_counter),
                    self.frame_seconds,
                );
                if let Some(m) = settings_model.as_ref().filter(|_| !native_options) {
                    setting_clicked = ui::menu::draw_menu(ctx, m);
                }
                if let Some(bindings) = controls_model
                    .as_ref()
                    .filter(|_| !native_options)
                    .and_then(|m| m.bindings.as_ref().map(|bindings| (m.cursor, bindings)))
                {
                    let (cursor, bindings) = bindings;
                    control_clicked = ui::menu::draw_controls(
                        ctx,
                        &bindings.labels,
                        cursor,
                        bindings.capture,
                        bindings.message.as_deref(),
                    );
                }
                if let Some(w) = warning {
                    ui::hud::draw_pad_warning(ctx, w);
                }
            });
            let view = &mut front.view;
            if let Err(e) = r.render_with_overlay(
                &Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, target, size| {
                    if let Some(v) = view.as_mut() {
                        v.paint(dev, q, enc, target, size);
                    }
                    overlay.paint(&paint, dev, q, enc, target, size)
                }),
            ) {
                eprintln!("render: {e}");
                event_loop.exit();
            }
        }
        if let Some(sound) = control_clicked.and_then(|row| self.activate_control_row(row)) {
            front.cue(sound);
        }
        for action in setting_clicked {
            if let Some(s) = shell_click_sound(action) {
                front.cue(s);
            }
            self.apply_shell_setting(&mut front, action);
        }
        // A page of the port's own closed: the screen under it is entered
        // again (its exit click, its entrance).
        let covered = front.ia_open || front.settings.is_some() || self.controls.is_some();
        if front.was_covered && !covered {
            front.menus.uncover();
        }
        front.was_covered = covered;
        if let Some(sounds) = front.sounds.as_mut() {
            let s = &self.profile.settings;
            sounds.set_user_volumes(s.effects_volume, s.dialogue_volume);
        }
        front.play_cues(self.audio.as_mut(), dt);
        self.front = Some(front);
        if quit {
            event_loop.exit();
        }
        if let Some((rows, options)) = launch {
            self.start_playlist(rows, options);
        }
    }

    /// One settings page action in the shell: the profile's value, saved;
    /// the music's level follows its slider.
    fn apply_shell_setting(&mut self, front: &mut FrontShell, a: ui::menu::MenuAction) {
        use ui::menu::MenuAction;
        let s = &mut self.profile.settings;
        match a {
            MenuAction::OpenClassPicker
            | MenuAction::SetClassTab(_)
            | MenuAction::CloseClassPicker
            | MenuAction::SelectClass(_) => return,
            MenuAction::RequestRespawn
            | MenuAction::CancelPopup
            | MenuAction::Respawn
            | MenuAction::RequestQuit
            | MenuAction::RequestRestart
            | MenuAction::Restart => return,
            MenuAction::OpenPage(page) => {
                front.settings = Some((page, 0));
                return;
            }
            MenuAction::EditBindings(device, context) => {
                self.controls = Some(Controls::new(device, context, &self.profile.settings));
                return;
            }
            MenuAction::Resume | MenuAction::Quit => {
                front.settings = None;
                return;
            }
            MenuAction::ClosePage => {
                front.settings = front.settings.and_then(|(page, _)| {
                    let (parent, row) = page.parent()?;
                    if parent == PausePage::Options {
                        return None;
                    }
                    let model = ui::menu::MenuModel {
                        page: Some(parent),
                        ..Default::default()
                    };
                    let cursor = ui::menu::rows(&model).iter().position(|r| *r == row)?;
                    Some((parent, cursor))
                });
                return;
            }
            MenuAction::ToggleCheat(_) => return,
            MenuAction::SetDifficulty(v) => {
                s.difficulty = game::shell_ia::port_difficulty(v);
                if let Some(ia) = front.ia.as_mut() {
                    ia.options.difficulty = v;
                }
            }
            MenuAction::SetEffectsVolume(v) => s.effects_volume = v.clamp(0.0, 1.0),
            MenuAction::SetDialogueVolume(v) => s.dialogue_volume = v.clamp(0.0, 1.0),
            MenuAction::SetFpsCounter(v) => s.fps_counter = v as u8,
            MenuAction::SetAssist(v) => {
                s.assist_intensity = game::profile::clamp_assist_intensity(v)
            }
            MenuAction::SetAiCap(c) => s.team_size_pct = c,
            MenuAction::SetReinforcements(c) => s.reinforcement_pct = c,
            MenuAction::SetFov(k) => s.fov_scale = game::walk::clamp_fov_scale(k),
            MenuAction::ResetGraphics => s.reset_graphics(),
            MenuAction::Denied => return,
            MenuAction::SetCareerAwards(on) => s.career_awards = on,
            MenuAction::SetRumble(v) => s.rumble_strength = v.clamp(0.0, 1.0),
            MenuAction::ToggleRumbleOnFire => s.rumble_on_fire = !s.rumble_on_fire,
            MenuAction::SetMusic(v) => {
                s.music_volume = v.clamp(game::music::VOLUME_RANGE.0, game::music::VOLUME_RANGE.1);
                // The take playing follows the level; off stops it.
                let gain = front.music_takes.as_ref().map_or(1.0, |(g, _)| *g);
                match front.music.as_mut() {
                    Some(m) if s.music_volume > 0.0 => {
                        m.set_volume(game::sounds::level_db(gain * s.music_volume), 0.05)
                    }
                    Some(_) => front.music = None,
                    None => front.music_replay = s.music_volume > 0.0,
                }
            }
            MenuAction::SetMouseSensitivity(v) => {
                s.mouse_sensitivity = v.clamp(
                    game::kbm::SENSITIVITY_RANGE.0,
                    game::kbm::SENSITIVITY_RANGE.1,
                )
            }
            MenuAction::SetPadLookSensitivity(v) => {
                s.pad_look_sensitivity = game::profile::clamp_pad_look(v)
            }
            MenuAction::ToggleFlightControls => s.flight_controls = s.flight_controls.toggled(),
            MenuAction::ToggleTerrainLod => s.terrain_lod = !s.terrain_lod,
            MenuAction::ToggleSceneSoftness => s.scene_softness = !s.scene_softness,
            MenuAction::ToggleInvertY => s.invert_y = !s.invert_y,
            MenuAction::ToggleLowPowerResolution => {
                s.low_power_resolution = !s.low_power_resolution
            }
            MenuAction::CycleInputIcons(dir) => s.input_icons = s.input_icons.cycled(dir),
            MenuAction::ToggleViewpoint => s.viewpoint = s.viewpoint.toggled(),
        }
        if matches!(a, MenuAction::SetRumble(_)) {
            game::rumble::preview(
                self.gilrs.as_mut(),
                &self.active_pad,
                &mut self.rumble_effect,
                s.rumble_strength,
            );
        }
        self.kbm.follow(&self.profile.settings);
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls() -> Controls {
        let mut controls = Controls::new(
            SettingsPage::KeyboardMouse,
            ControlContext::Infantry,
            &Default::default(),
        );
        controls.capture = Some(Capture {
            row: 0,
            ready: false,
            controller: None,
            candidate: None,
        });
        controls
    }

    /// re-bf2-menus §4.1: a settings move clicks; a value step plays accept
    /// (even at a clamp); the music row and Back play nothing here.
    #[test]
    fn the_settings_page_asks_for_the_options_clicks() {
        use ui::menu::{MenuInput, MenuModel, Row};
        let model = MenuModel {
            in_shell: Some("Game Options".into()),
            team_size_pct: 10,
            ..Default::default()
        };
        let rows = ui::menu::rows(&model);
        let at = |row: Row| MenuModel {
            cursor: rows.iter().position(|r| *r == row).unwrap(),
            ..model.clone()
        };
        let sound = |m: &MenuModel, input: MenuInput| {
            let (to, _) = ui::menu::navigate(m, input);
            setting_sound(m, input, to)
        };
        let down = MenuInput {
            down: true,
            ..Default::default()
        };
        let left = MenuInput {
            left: true,
            ..Default::default()
        };
        let back = MenuInput {
            back: true,
            ..Default::default()
        };
        assert_eq!(sound(&model, down), Some(MenuSound::Select));
        assert_eq!(
            sound(&at(Row::AiCap), left),
            Some(MenuSound::Accept),
            "at its clamp"
        );
        assert_eq!(sound(&at(Row::Music), left), None);
        assert_eq!(sound(&at(Row::Fov), back), None);
        // A click sounds as its press: a value step accepts, music and Back
        // stay quiet (issue #142).
        use ui::menu::MenuAction;
        assert_eq!(
            shell_click_sound(MenuAction::SetFov(1.05)),
            Some(MenuSound::Accept)
        );
        assert_eq!(
            shell_click_sound(MenuAction::SetPadLookSensitivity(1.1)),
            Some(MenuSound::Accept)
        );
        assert_eq!(
            shell_click_sound(MenuAction::ToggleInvertY),
            Some(MenuSound::Accept)
        );
        assert_eq!(shell_click_sound(MenuAction::SetMusic(0.5)), None);
        assert_eq!(shell_click_sound(MenuAction::Resume), None);
        assert_eq!(sound(&at(Row::Fov), MenuInput::default()), None);
    }

    #[test]
    fn occupied_binding_leaves_the_displaced_action_unassigned() {
        let mut c = controls();
        let key = game::kbm::Trigger::Key(winit::keyboard::KeyCode::KeyS);
        assert_eq!(c.assign(Binding::Keyboard(key)), MenuSound::Accept);
        assert_eq!(c.missing(), Some(1));
        assert_eq!(c.message.as_deref(), Some("Move backward is unassigned"));
    }

    #[test]
    fn rejected_wheel_binding_explains_rejection_and_focus_loss_cancels() {
        let setup = crate::BattleSetup {
            seed: 42,
            difficulty: Default::default(),
            spawn: (0.0, 0.0),
            spawn_set: false,
            targets: false,
            ai: None,
            ai_cap: None,
            team_size_pct: None,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: None,
            classes_dir: Default::default(),
            mission: None,
            vehicle: None,
            mission_file: None,
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
            score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
        };
        let mut app = App::new(
            Default::default(),
            Default::default(),
            setup,
            None,
            None,
            None,
            Default::default(),
        );
        app.controls = Some(controls());
        let before = app.profile.clone();
        assert!(
            app.capture_keyboard(&winit::event::WindowEvent::MouseWheel {
                device_id: winit::event::DeviceId::dummy(),
                delta: winit::event::MouseScrollDelta::LineDelta(0.0, 1.0),
                phase: winit::event::TouchPhase::Moved,
            })
        );
        assert_eq!(app.profile, before);
        let c = app.controls.as_ref().unwrap();
        assert!(c.capture.is_some());
        assert!(c
            .message
            .as_deref()
            .unwrap()
            .contains("wheel cannot hold movement"));
        assert!(!app.capture_keyboard(&winit::event::WindowEvent::Focused(false)));
        assert!(app.controls.as_ref().unwrap().capture.is_none());
        assert_eq!(app.profile, before);
    }
}
