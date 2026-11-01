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
//! the CORE.LVL bank (`SoundLib::load_shell`). `OPENBF_SHELL_SOUNDS=1` logs
//! each start to stderr (a run without a sound device logs them too).

use super::{App, Camera};
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
    settings: Option<usize>,
    controls: Option<Controls>,
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
    /// `OPENBF_SHELL_SOUNDS=1`: log every click and take started.
    log_sounds: bool,
}

struct Controls {
    cursor: usize,
    capture: Option<usize>,
    ready: bool,
    controller: Option<gilrs::GamepadId>,
    message: Option<String>,
}

const CONTROL_COMMAND_ROWS: usize = 2;

impl Controls {
    /// Start always cancels, including keyboard rows and simultaneous button presses.
    /// A held accept button must be released before it can become a new binding.
    fn capture_button(
        &mut self,
        mut down: impl FnMut(gilrs::Button) -> bool,
    ) -> Option<gilrs::Button> {
        if down(gilrs::Button::Start) {
            self.capture = None;
            self.message = Some("Binding change cancelled".into());
            return None;
        }
        let pressed = PAD_BUTTONS.iter().copied().find(|b| down(*b));
        if !self.ready {
            self.ready = pressed.is_none();
            None
        } else {
            pressed
        }
    }
}

fn action_name(bits: u32, walk: (i8, i8)) -> String {
    use core::sim::buttons as b;
    let mut names = Vec::new();
    match walk {
        (0, 1) => names.push("Move forward"),
        (0, -1) => names.push("Move backward"),
        (-1, 0) => names.push("Move left"),
        (1, 0) => names.push("Move right"),
        _ => {}
    }
    for (bit, name) in [
        (b::FIRE, "Fire"),
        (b::SECONDARY_FIRE, "Secondary fire"),
        (b::ZOOM, "Zoom"),
        (b::JUMP, "Jump"),
        (b::ROLL_STANCE, "Roll / stance"),
        (b::SPRINT, "Sprint"),
        (b::RELOAD, "Reload"),
        (b::ACTION, "Action"),
        (b::SWITCH_WEAPON, "Next weapon"),
        (b::SWITCH_SECONDARY, "Secondary weapon"),
        (b::SQUAD_MOVE_OUT, "Squad move"),
        (b::SQUAD_HOLD, "Squad hold"),
        (b::SQUAD_AT_EASE, "Squad at ease"),
        (b::SQUAD_FOLLOW, "Squad follow"),
        (b::SPAWN_ACCEPT, "Spawn accept"),
        (b::SPAWN_BACK, "Spawn back"),
        (b::SPAWN_CLASS_NEXT, "Next class"),
        (b::SPAWN_CLASS_PREV, "Previous class"),
        (b::SPAWN_POST_UP, "Post up"),
        (b::SPAWN_POST_DOWN, "Post down"),
        (b::SPAWN_POST_LEFT, "Post left"),
        (b::SPAWN_POST_RIGHT, "Post right"),
    ] {
        if bits & bit != 0 {
            names.push(name);
        }
    }
    names.join(" / ")
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
    pub(super) fn reset_control_device(&mut self) {
        if let Some(controls) = self.controls.as_mut() {
            controls.ready = false;
            controls.controller = None;
        }
    }

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
            controls: None,
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
            log_sounds: std::env::var("OPENBF_SHELL_SOUNDS").is_ok_and(|v| v == "1"),
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
            let starts = sl.play_2d(audio.as_deref_mut(), s.name());
            if self.log_sounds {
                let clock = self.menus.clock;
                match starts.first() {
                    Some(st) => eprintln!(
                        "shell sound t={clock:.3} {} gain {:.3} bus {:.2} pitch {:.2} take {}",
                        s.name(),
                        st.gain,
                        st.bus,
                        st.pitch,
                        st.path.display()
                    ),
                    None => eprintln!("shell sound t={clock:.3} {} refused", s.name()),
                }
            }
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
        Row::InvertY if input.confirm || input.left || input.right => Some(MenuSound::Accept),
        _ if input.left || input.right => Some(MenuSound::Accept),
        _ => None,
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

    fn control_labels(&self) -> Vec<(String, String)> {
        let mut rows = vec![
            ("Back".into(), String::new()),
            ("Restore defaults".into(), String::new()),
        ];
        for (trigger, bind) in &self.profile.settings.keyboard_bindings.table {
            rows.push((
                format!("Keyboard / mouse: {}", action_name(bind.bits, bind.walk)),
                format!("{trigger:?}"),
            ));
        }
        for (button, bits) in &self.profile.settings.controller_bindings.table {
            rows.push((
                format!("Controller: {}", action_name(*bits, (0, 0))),
                format!("{button:?}"),
            ));
        }
        rows
    }

    fn save_controls(&mut self) {
        self.kbm.follow(&self.profile.settings);
        self.kbm.release_keys();
        if let Some(g) = self.gilrs.as_ref() {
            self.pad_warning = input::log_pads_with(
                g,
                &self.profile.settings.controller_bindings,
                &self.active_pad,
            );
        }
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
    }

    fn activate_control_row(&mut self, front: &mut FrontShell, row: usize) {
        if let Some(controls) = front.controls.as_mut() {
            controls.cursor = row;
        }
        if row != 0 {
            front.cue(MenuSound::Accept);
        }
        match row {
            0 => front.controls = None,
            1 => {
                self.profile.settings.keyboard_bindings = game::kbm::Bindings::default();
                self.profile.settings.controller_bindings = input::Mapping::default();
                self.sprint_latch = input::SprintLatch::default();
                if let Some(controls) = front.controls.as_mut() {
                    controls.message = Some("Default bindings restored".into());
                }
                self.save_controls();
            }
            _ => {
                if let Some(controls) = front.controls.as_mut() {
                    controls.message = None;
                    controls.capture = Some(row);
                    controls.ready = false;
                    controls.controller = None;
                }
            }
        }
    }

    pub(super) fn capture_keyboard(&mut self, event: &winit::event::WindowEvent) -> bool {
        use game::kbm::Trigger;
        use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};
        let Some(front) = self.front.as_mut() else {
            return false;
        };
        let FrontShell {
            controls, menus, ..
        } = front;
        let Some(controls) = controls.as_mut() else {
            return false;
        };
        let Some(row) = controls.capture else {
            return false;
        };
        let cues = &mut menus.cues;
        if matches!(event, WindowEvent::Focused(false)) {
            controls.capture = None;
            controls.message = Some("Binding change cancelled on focus loss".into());
            return false;
        }
        let count = self.profile.settings.keyboard_bindings.table.len();
        if row < CONTROL_COMMAND_ROWS || row >= CONTROL_COMMAND_ROWS + count {
            if matches!(event, WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && event.physical_key == PhysicalKey::Code(KeyCode::Escape))
            {
                controls.capture = None;
                controls.message = Some("Binding change cancelled".into());
                cues.play(MenuSound::Cancel);
                return true;
            }
            return false;
        }
        let trigger = match event {
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        controls.capture = None;
                        controls.message = Some("Binding change cancelled".into());
                        cues.play(MenuSound::Cancel);
                        return true;
                    }
                    PhysicalKey::Code(code) => Some(Trigger::Key(code)),
                    _ => None,
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => Some(Trigger::Mouse(*button)),
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if y > 0.0 {
                    Some(Trigger::WheelUp)
                } else if y < 0.0 {
                    Some(Trigger::WheelDown)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(trigger) = trigger {
            let index = row - CONTROL_COMMAND_ROWS;
            let bindings = &mut self.profile.settings.keyboard_bindings;
            let swapped = bindings
                .table
                .iter()
                .enumerate()
                .find(|(i, (t, _))| *i != index && *t == trigger)
                .map(|(_, (_, bind))| action_name(bind.bits, bind.walk));
            if !bindings.remap(index, trigger) {
                controls.message =
                    Some("Mouse wheel cannot hold movement; choose a key or mouse button".into());
                cues.play(MenuSound::Error);
                return true;
            }
            cues.play(MenuSound::Accept);
            controls.message = Some(swapped.map_or_else(
                || "Binding saved".into(),
                |name| format!("Binding saved; swapped with {name}"),
            ));
            controls.capture = None;
            self.save_controls();
        }
        matches!(event, WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed)
            || matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    ..
                } | WindowEvent::MouseWheel { .. }
            )
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
        let pointer = self.shell_pointer.take_frame();
        let click = pointer.is_some_and(|(_, c)| c);
        if let (Some((point, _)), Some((w, h)), Some(view)) = (
            pointer,
            self.renderer.as_ref().map(|r| r.size()),
            front.view.as_ref(),
        ) {
            if !front.ia_open && front.settings.is_none() && front.controls.is_none() {
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
        let stack_frame = !front.ia_open && front.controls.is_none() && front.settings.is_none();
        if !stack_frame {
            front.menus.tick(dt);
        }
        if let Some(ia) = front.ia.as_mut().filter(|_| front.ia_open) {
            let mut ia_input = ui::shell::IaInput {
                menu: input,
                l1: edges & game::kbm::MENU_L1 != 0,
                r1: edges & game::kbm::MENU_R1 != 0,
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
                        ia_input.menu.confirm |=
                            click && !matches!(target, ui::shell::PointerTarget::Option(..));
                        if click {
                            if let ui::shell::PointerTarget::Option(_, right) = target {
                                ia_input.menu.left |= !right;
                                ia_input.menu.right |= right;
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
        } else if let Some(controls) = front.controls.as_mut() {
            let count = CONTROL_COMMAND_ROWS
                + self.profile.settings.keyboard_bindings.table.len()
                + self.profile.settings.controller_bindings.table.len();
            let mut activate = None;
            if let Some(row) = controls.capture {
                let keyboard_count = self.profile.settings.keyboard_bindings.table.len();
                let controller_row = row >= CONTROL_COMMAND_ROWS + keyboard_count;
                let pad = self
                    .gilrs
                    .as_ref()
                    .and_then(|g| input::active_gamepad(g, &self.active_pad));
                if let Some((id, pad)) = pad {
                    if controls.controller != Some(id) {
                        controls.controller = Some(id);
                        controls.ready = false;
                    }
                    let was = controls.capture;
                    let button = controls.capture_button(|b| input::is_down(&pad, b));
                    if was.is_some() && controls.capture.is_none() {
                        front.menus.cues.play(MenuSound::Cancel);
                    }
                    if controller_row {
                        if let Some(button) = button {
                            let index = row - CONTROL_COMMAND_ROWS - keyboard_count;
                            let bindings = &mut self.profile.settings.controller_bindings;
                            let swapped = bindings
                                .table
                                .iter()
                                .enumerate()
                                .find(|(i, (b, _))| *i != index && *b == button)
                                .map(|(_, (_, bits))| action_name(*bits, (0, 0)));
                            bindings.remap(index, button);
                            controls.capture = None;
                            front.menus.cues.play(MenuSound::Accept);
                            controls.message = Some(swapped.map_or_else(
                                || "Binding saved".into(),
                                |name| format!("Binding saved; swapped with {name}"),
                            ));
                            self.sprint_latch = input::SprintLatch::default();
                            self.save_controls();
                        }
                    }
                } else if controller_row {
                    controls.capture = None;
                    controls.message = Some("Controller disconnected; binding unchanged".into());
                    front.menus.cues.play(MenuSound::Error);
                }
            } else {
                let from = controls.cursor;
                if input.up {
                    controls.cursor = (controls.cursor + count - 1) % count;
                }
                if input.down {
                    controls.cursor = (controls.cursor + 1) % count;
                }
                front.menus.cues.list_move(from, controls.cursor);
                if input.back {
                    controls.cursor = 0;
                }
                if input.confirm || input.back {
                    activate = Some(controls.cursor);
                }
            }
            if let Some(row) = activate {
                self.activate_control_row(&mut front, row);
            }
            controls_model = front.controls.as_ref().map(|c| {
                (
                    self.control_labels(),
                    c.cursor,
                    c.capture.is_some(),
                    c.message.clone(),
                )
            });
        } else if let Some(cursor) = front.settings {
            let settings = &self.profile.settings;
            let k = settings.fov_scale;
            let model = ui::menu::MenuModel {
                assist_intensity: settings.assist_intensity,
                team_size_pct: settings.team_size_pct,
                reinforcements_pct: settings.reinforcement_pct,
                population_note: game::conquest_defaults::menu_note(
                    settings.team_size_pct,
                    settings.reinforcement_pct,
                ),
                fov_scale: k,
                fov_deg: (game::walk::chase_fov_y() * k).to_degrees(),
                music_volume: settings.music_volume,
                mouse_sensitivity: settings.mouse_sensitivity,
                invert_y: settings.invert_y,
                cursor,
                in_shell: Some(
                    front.menus.options.rows[front.menus.options.focus]
                        .label
                        .clone(),
                ),
                ..Default::default()
            };
            let (cursor, acts) = ui::menu::navigate(&model, input);
            if let Some(s) = setting_sound(&model, input, cursor) {
                front.cue(s);
            }
            front.settings = Some(cursor);
            for a in acts {
                self.apply_shell_setting(&mut front, a);
            }
            settings_model = front
                .settings
                .map(|cursor| ui::menu::MenuModel { cursor, ..model });
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
                Some(ui::shell_ps2::ShellCommand::Settings) => front.settings = Some(0),
                Some(ui::shell_ps2::ShellCommand::Controls) => {
                    front.controls = Some(Controls {
                        cursor: 0,
                        capture: None,
                        ready: false,
                        controller: None,
                        message: None,
                    });
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
            if let (Some(k), true) = (due, front.log_sounds) {
                eprintln!(
                    "shell music t={:.3} take {} of {} ({:.1} s) gain {gain:.3}",
                    front.menus.clock,
                    k + 1,
                    takes.len(),
                    secs[k]
                );
            }
        }
        front.menus.covered = front.ia_open || front.settings.is_some() || front.controls.is_some();
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
            .filter(|_| front.ia_open && front.settings.is_none() && front.controls.is_none());
        let movie_shown = has_movie && ia.is_none_or(|ia| ia.page != ui::shell::Page::Missions);
        let mut control_clicked = None;
        let mut setting_clicked = Vec::new();
        if let (Some(r), Some(overlay), Some(window)) =
            (&mut self.renderer, &mut self.overlay, &self.window)
        {
            let (w, h) = r.size();
            if let Some(v) = front.view.as_mut() {
                match ia {
                    Some(ia) => v.update_instant(&front.menus, ia, movie_shown, [w, h]),
                    None => v.update_shell(&front.menus, has_movie, [w, h]),
                }
            }
            let warning = self.pad_warning.as_deref();
            let paint = overlay.begin(window, |ctx| {
                if let Some(m) = &settings_model {
                    setting_clicked = ui::menu::draw_menu(ctx, m);
                }
                if let Some((labels, cursor, capture, message)) = &controls_model {
                    control_clicked =
                        ui::menu::draw_controls(ctx, labels, *cursor, *capture, message.as_deref());
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
        if let (Some(row), Some(controls)) = (control_clicked, front.controls.as_mut()) {
            if controls.capture.is_none() {
                self.activate_control_row(&mut front, row);
            }
        }
        for action in setting_clicked {
            // A pointer click sounds as its pad press (only toggles click).
            if action == ui::menu::MenuAction::ToggleInvertY {
                front.cue(MenuSound::Accept);
            }
            self.apply_shell_setting(&mut front, action);
        }
        // A page of the port's own closed: the screen under it is entered
        // again (its exit click, its entrance).
        let covered = front.ia_open || front.settings.is_some() || front.controls.is_some();
        if front.was_covered && !covered {
            front.menus.uncover();
        }
        front.was_covered = covered;
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
            MenuAction::RequestRespawn | MenuAction::CancelRespawn | MenuAction::Respawn => return,
            MenuAction::Resume | MenuAction::Quit => {
                front.settings = None;
                return;
            }
            MenuAction::ToggleCheat(_) => return,
            MenuAction::SetAssist(v) => s.assist_intensity = v.clamp(0.0, 1.5),
            MenuAction::SetAiCap(c) => s.team_size_pct = c,
            MenuAction::SetReinforcements(c) => s.reinforcement_pct = c,
            MenuAction::SetFov(k) => {
                s.fov_scale = k.clamp(game::walk::FOV_SCALE_RANGE.0, game::walk::FOV_SCALE_RANGE.1)
            }
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
            MenuAction::ToggleInvertY => s.invert_y = !s.invert_y,
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

    fn controls(row: usize) -> Controls {
        Controls {
            cursor: row,
            capture: Some(row),
            ready: false,
            controller: None,
            message: None,
        }
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
        assert_eq!(sound(&at(Row::Fov), MenuInput::default()), None);
    }

    #[test]
    fn start_cancels_either_device_before_accept_or_release_gate() {
        for row in [
            CONTROL_COMMAND_ROWS,
            CONTROL_COMMAND_ROWS + game::kbm::Bindings::default().table.len(),
        ] {
            for ready in [false, true] {
                let mut c = controls(row);
                c.ready = ready;
                assert_eq!(
                    c.capture_button(|b| matches!(b, gilrs::Button::Start | gilrs::Button::South)),
                    None
                );
                assert_eq!(c.capture, None);
                assert_eq!(c.message.as_deref(), Some("Binding change cancelled"));
            }
        }
    }

    #[test]
    fn held_accept_must_release_before_controller_binding() {
        let mut c = controls(CONTROL_COMMAND_ROWS + game::kbm::Bindings::default().table.len());
        assert_eq!(c.capture_button(|b| b == gilrs::Button::South), None);
        assert!(!c.ready);
        assert_eq!(c.capture_button(|_| false), None);
        assert!(c.ready);
        assert_eq!(
            c.capture_button(|b| b == gilrs::Button::West),
            Some(gilrs::Button::West)
        );
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
            cheats: Default::default(),
            teams: None,
            classes_dir: Default::default(),
            mission: None,
            mission_file: None,
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
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
        let mut front = FrontShell::new(std::path::Path::new("__no_assets_for_capture_test__"));
        front.controls = Some(controls(CONTROL_COMMAND_ROWS));
        app.front = Some(front);
        let before = app.profile.clone();
        assert!(
            app.capture_keyboard(&winit::event::WindowEvent::MouseWheel {
                device_id: winit::event::DeviceId::dummy(),
                delta: winit::event::MouseScrollDelta::LineDelta(0.0, 1.0),
                phase: winit::event::TouchPhase::Moved,
            })
        );
        assert_eq!(app.profile, before);
        let c = app.front.as_ref().unwrap().controls.as_ref().unwrap();
        assert!(c.capture.is_some());
        assert!(c
            .message
            .as_deref()
            .unwrap()
            .contains("wheel cannot hold movement"));
        assert!(!app.capture_keyboard(&winit::event::WindowEvent::Focused(false)));
        assert!(app
            .front
            .as_ref()
            .unwrap()
            .controls
            .as_ref()
            .unwrap()
            .capture
            .is_none());
        assert_eq!(app.profile, before);
    }
}
