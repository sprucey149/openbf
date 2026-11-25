//! The windowed shell's front screens (card BSHLA, `ui::shell_ps2`): the
//! PS2's Press START, Main Menu, Singleplayer and Options over the shell
//! movie, with the shell music; Instant Action and its Session Options
//! (`ui::shell`, `ui::shell_ia`, `analysis/re-bf2-menus.md` ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§1-ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§2) and the
//! settings page (`ui::menu` in its shell form). A child of `windowed` so it
//! reaches the `App` state.
//!
//! The menus' clicks (`analysis/re-bf2-menus.md` ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§4.1) are asked for by
//! `ui::shell_ps2` and, for the pages of the port's own, here; all of them go
//! through `ShellMenus::cues` (the select click's 0.1 s limit) and play from
//! the CORE.LVL bank (`SoundLib::load_shell`).

use super::{App, Camera};
use core::fmath::FloatExt;
use ui::menu::PausePage;
// The extracted `front/tests.rs` reaches these through `use super::*`.
#[cfg(test)]
use ui::menu::{ControlContext, SettingsPage};
use ui::shell_feel::MenuSound;

#[path = "front/campaign.rs"]
mod campaign;
#[path = "front/campaign_saves.rs"]
mod campaign_saves;
#[path = "front/controls.rs"]
mod controls;
#[path = "front/credits.rs"]
mod credits;
#[path = "front/gal.rs"]
mod gal;
#[path = "front/players.rs"]
mod players;
#[path = "front/profiles.rs"]
mod profiles;

// The binding editor is built in `windowed` (the pause / shell command path)
// and its state travels with `App`, so keep the facade's names visible there.
pub(super) use controls::{
    Binding, Capture, Controls, Layout, CANCEL_HOLD, CONTROL_COMMAND_ROWS, PAD_BUTTONS,
};

/// The BF2 PS2 shell's front screens (card BSHLA, `ui::shell_ps2`): Press
/// START, the Main Menu, Singleplayer and Options over the background
/// movie, with the shell music; Instant Action opens the PS2's screen
/// (`ui::shell`) and Game / Audio Options the settings page (`ui::menu` in
/// its shell form).
pub(super) struct FrontShell {
    pub(super) menus: ui::shell_ps2::ShellMenus,
    picker: Option<ui::shell_ps2::profiles::Picker>,
    /// The Rise of the Empire save's name keyboard.
    save_name: Option<ui::shell_ps2::profiles::Picker>,
    name_input: profiles::NameInput,
    pub(super) movie: Option<game::shell_movie::ShellMovie>,
    /// Built at the first frame with a renderer.
    pub(super) view: Option<game::hud_feed::HudView>,
    /// The Instant Action screen: built at its first Enter, kept (with its
    /// playlist and session options) for the process, as the PS2 keeps
    /// `gPickedMapList` (ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§1.3).
    ia: Option<ui::shell::InstantAction>,
    /// It is up.
    ia_open: bool,
    /// The settings page is up, at this cursor.
    settings: Option<(PausePage, usize)>,
    settings_animation: ui::pause_draw::Animation,
    sounds: Option<game::sounds::SoundLib>,
    pub(super) music: Option<audio::MusicVoice>,
    /// Synced audio track for an active cutscene in `cutscenes`.
    pub(super) movie_audio: Option<(String, audio::Shot)>,
    /// Queued full-screen campaign cutscenes (`crawl1`/`crawl2`, `*tran1`, `*mon01`, `*mon02`).
    pub(super) cutscenes: std::collections::VecDeque<String>,
    /// Mission index in `ui::campaign::RISE_OF_THE_EMPIRE` waiting on `cutscenes`.
    pub(super) pending_campaign: Option<usize>,
    credits: Option<credits::Crawl>,
    /// Lazy 3D spinning planet backdrop (`bf2_gal1`) for the campaign battle screen.
    gal: Option<gal::GalPlanetView>,
    gal_load_failed: bool,
    /// `metagame_menu_music` from `bf2_gal1/sounds`, played over the planet screen.
    gal_music: Option<(f32, Vec<audio::StreamTake>)>,
    /// Whether `self.music` currently plays `gal_music` instead of `shell_soundtrack`.
    active_gal_music: bool,
    /// `shell_soundtrack`'s gain (x its bus) and takes, and which one plays
    /// when (re-bf2-menus ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§4.4).
    music_takes: Option<(f32, Vec<audio::StreamTake>)>,
    playlist: ui::shell_feel::ShellPlaylist,
    /// The random take picker's state (the shell is not the sim: seeded
    /// from the clock, menuref ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§9.3.9).
    music_rng: u64,
    /// A held D-pad arm's repeat (menuref: 0.62 s, then every 0.167 s).
    pub(super) repeat: ui::shell_feel::HoldRepeat,
    /// The music level was raised from off: the take of the moment again.
    music_replay: bool,
    /// Issue #128: when the music's clock last moved. The menus' frames can
    /// stall (a hidden window draws no frames, and each frame's step is
    /// clamped) while the take plays on in real time.
    music_clock: Option<std::time::Instant>,
    /// A page of the port's own covered the screen last frame.
    was_covered: bool,
    /// Issue #168: Multiplayer's Host Game or Join Game page is up.
    pub(super) mp: Option<super::lobby::MpPage>,
    /// A text box had the keyboard last frame: the menu keys wait.
    pub(super) text_focus: bool,
    /// Game Options' Player name box is open: the text and its refusal.
    name_edit: Option<(String, Option<String>)>,
    /// Issue #194: the Players screen's seats, kept for the session (the shell lives across
    /// battles), and each connected pad's held buttons last frame.
    pub(super) party: game::local_seats::Party<gilrs::GamepadId>,
    party_held: Vec<players::PadHeld>,
    /// The keyboard seat's held arrows repeat on the Players screen.
    party_key_repeat: ui::shell_feel::HoldRepeat,
    /// The keyboard's Tab held toward emptying a lost seat (`PartyPress::remove`).
    party_key_remove: game::local_seats::RemoveHold,
}

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
        let gal_music = game::sounds::SoundLib::load(&assets_dir.join("bf2_gal1"))
            .and_then(|sl| sl.stream_takes("metagame_menu_music"));
        FrontShell {
            menus: ui::shell_ps2::ShellMenus::new(&text),
            picker: None,
            save_name: None,
            name_input: Default::default(),
            movie,
            view: None,
            ia: None,
            ia_open: false,
            settings: None,
            settings_animation: Default::default(),
            sounds,
            music: None,
            movie_audio: None,
            cutscenes: std::collections::VecDeque::new(),
            pending_campaign: None,
            credits: None,
            gal: None,
            gal_load_failed: false,
            gal_music,
            active_gal_music: false,
            music_takes,
            playlist: Default::default(),
            music_rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x9e37_79b9, |d| d.as_nanos() as u64)
                | 1,
            repeat: Default::default(),
            music_replay: false,
            music_clock: None,
            was_covered: false,
            mp: None,
            text_focus: false,
            name_edit: None,
            party: Default::default(),
            party_held: Vec::new(),
            party_key_repeat: Default::default(),
            party_key_remove: Default::default(),
        }
    }

    /// The seats as a split match left them.
    pub(super) fn set_party(&mut self, owners: input::owners::PadOwners<gilrs::GamepadId>) {
        self.party.set_owners(owners);
    }

    /// Each split round moves P2-P4's Ready profiles into the battle (`App.split_profiles`, by
    /// seat number), never a copy (`SPLITSCREEN.md` 18.1 item 1).
    pub(super) fn lend_seat_profiles(&mut self, to: &mut [Option<game::profile::Profile>; 4]) {
        for (seat, slot) in to.iter_mut().enumerate().skip(1) {
            if let Some(p) = self.party.lend(seat) {
                *slot = Some(p);
            }
        }
    }

    /// The battle is over: the seats' profiles come back to their seats as it left them.
    pub(super) fn give_back_seat_profiles(
        &mut self,
        from: &mut [Option<game::profile::Profile>; 4],
    ) {
        for (seat, slot) in from.iter_mut().enumerate() {
            if let Some(p) = slot.take() {
                self.party.give_back(seat, p);
            }
        }
    }

    /// The shell is left for a battle: its music stops, and starts with a
    /// new random take when the shell comes back.
    pub(super) fn stop_music(&mut self) {
        self.music = None;
        self.movie_audio = None;
        self.active_gal_music = false;
        self.playlist = Default::default();
        self.music_clock = None;
    }

    pub(super) fn clear_gal(&mut self) {
        self.gal = None;
        self.gal_load_failed = false;
    }

    /// Tests: the shell music's takes in place of the bank's.
    #[cfg(test)]
    pub(super) fn set_music_takes(&mut self, takes: Vec<audio::StreamTake>) {
        self.music_takes = Some((1.0, takes));
    }

    /// Tests: the take playing now (`None` in a gap or before the first).
    #[cfg(test)]
    pub(super) fn music_playing(&self) -> Option<usize> {
        self.playlist.playing(&self.music_secs())
    }

    /// The takes' lengths in list order (none without the bank).
    fn music_secs(&self) -> Vec<f32> {
        self.music_takes
            .as_ref()
            .map_or_else(Vec::new, |(_, takes)| {
                takes.iter().map(audio::StreamTake::secs).collect()
            })
    }

    /// The music's schedule moved on by one menu frame: `dt` the frame's
    /// step, `wall` the real seconds since the last one (the take keeps
    /// playing in real time, so the next is due by the later of the two).
    /// Returns the take to start now: the playlist's next, or the take of
    /// the moment again after the level was raised from off.
    fn music_due(&mut self, dt: f32, wall: f32) -> Option<usize> {
        let secs = self.music_secs();
        let mut rng = self.music_rng;
        let due = {
            let mut pick = |n: usize| (xorshift(&mut rng) % n.max(1) as u64) as usize;
            self.playlist.step(dt.fmax(wall), &secs, &mut pick)
        };
        self.music_rng = rng;
        // A replay asked for waits until there is a take to play (none
        // loaded yet, or a gap between takes).
        let start = due.or_else(|| self.playlist.playing(&secs).filter(|_| self.music_replay));
        if start.is_some() {
            self.music_replay = false;
        }
        start
    }

    /// Ask for a menu click (rate limited, `ShellMenus::cues`).
    fn cue(&mut self, s: MenuSound) {
        self.menus.cues.play(s);
    }

    /// Play this frame's cues on the shell's clock.
    pub(super) fn play_cues(&mut self, audio: Option<&mut audio::Audio>, dt: f32) {
        let cues = self.menus.cues.take();
        let Some(sl) = self.sounds.as_mut() else {
            return;
        };
        sl.advance_menu_clock(dt);
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

/// A settings page frame's click (re-bf2-menus ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§4.1 [V]): a cursor move
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
        Row::InvertY
        | Row::InputIcons
        | Row::FlightControls
        | Row::CareerAwards
        | Row::HardShadows
        | Row::WindowMode
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

/// Which seats the Players screen's party holds.
fn party_seated<Id: Copy + PartialEq>(owners: &input::owners::PadOwners<Id>) -> [bool; 4] {
    std::array::from_fn(|i| owners.is_seated(i))
}

impl App {
    /// The Players screen's seats' names for the Splitscreen page's rows (issue #194 B22,
    /// `MenuModel::split_names`): P1 the current profile, a further seat its Ready or lent
    /// profile, through the single-player name filter; `None` for an open, picking or unnamed
    /// seat.
    fn party_seat_names(
        &self,
        party: &game::local_seats::Party<gilrs::GamepadId>,
    ) -> [Option<String>; 4] {
        use game::local_seats::SeatProfile;
        std::array::from_fn(|seat| {
            let name = match party.profile(seat) {
                SeatProfile::Host => Some(self.profile.name.as_str()),
                SeatProfile::Ready(p) => Some(p.name.as_str()),
                SeatProfile::Lent(_) => self.split_profiles[seat].as_ref().map(|p| p.name.as_str()),
                SeatProfile::Open | SeatProfile::Picking(_) => None,
            };
            name.and_then(|n| super::single_player_name(false, n))
        })
    }

    pub(super) fn queue_battle_movies(&mut self) {
        if !self.in_battle() || self.netplay.is_some() {
            return;
        }
        let clips = self
            .walk
            .as_mut()
            .and_then(|(walk, _)| walk.campaign.as_mut())
            .map_or_else(Vec::new, |campaign| campaign.take_movies());
        if !clips.is_empty() {
            self.front
                .get_or_insert_with(|| FrontShell::new(&self.assets_dir))
                .cutscenes
                .extend(clips);
        }
    }

    pub(super) fn battle_movie_pending(&self) -> bool {
        self.in_battle() && self.front.as_ref().is_some_and(|f| !f.cutscenes.is_empty())
    }

    pub(super) fn redraw_battle_movie(&mut self, dt: f32) -> anyhow::Result<()> {
        let edges = self.menu_edges();
        self.kbm.drop_taps();
        self.kbm.clear_look();
        game::pause::hold_for_resume(&mut self.resume_gate);
        let Some(front) = self.front.as_mut() else {
            return Ok(());
        };
        if (edges & (16 | 32 | 64 | game::kbm::MENU_NORTH)) != 0
            && front.movie.as_ref().is_some_and(|m| m.shown)
        {
            front.skip_cutscene();
        }
        let (frame, shown) = front.step_cutscene_movie(
            dt,
            self.audio.as_mut(),
            self.profile.settings.dialogue_volume,
        );
        if let (Some(r), Some(hud)) = (self.renderer.as_mut(), self.hud_view.as_mut()) {
            if let Some((rgba, w, h)) = frame {
                hud.set_movie_frame(rgba, w, h);
            }
            let (w, h) = r.size();
            hud.update_cutscene([w, h], shown);
            r.render_with_overlay(
                &Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, target, size| hud.paint(dev, q, enc, target, size)),
            )?;
        }
        Ok(())
    }
    /// Session Options were left (`push_prefs`): AI Difficulty is the
    /// profile's tier, saved.
    pub(super) fn keep_session_prefs(&mut self, options: &ui::shell::SessionOptions) {
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
                // Issue #194 (B10): a split player's page writes their own seat's profile.
                let seat = controls.seat;
                let profile = match seat {
                    0 => Some(&mut self.profile),
                    _ => self.split_profiles.get_mut(seat).and_then(Option::as_mut),
                };
                let Some(profile) = profile else {
                    return Some(MenuSound::Exit);
                };
                let settings = &mut profile.settings;
                match controls.layout {
                    Layout::Keyboard(items) => settings
                        .keyboard_layouts
                        .set(controls.context, items.into_iter().flatten().collect()),
                    Layout::Gamepad(items) => settings
                        .gamepad_layouts
                        .set(controls.context, items.into_iter().flatten().collect()),
                }
                if let Err(e) = profile.save() {
                    eprintln!("profile autosave failed: {e:#}");
                }
                if seat != 0 {
                    // Their pad table is rebuilt from the new layouts on its next sample.
                    if let Some(split) = self.split.as_mut() {
                        let n = split.inputs.len();
                        if let Some(k) =
                            (0..n).find(|&k| super::split::profile_seat(&split.seats, k) == seat)
                        {
                            split.inputs[k].relayout();
                        }
                    }
                }
                self.refresh_context_bindings();
                self.kbm.release_keys();
                self.sprint_latch = input::SprintLatch::default();
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
        // Split: the pad of the player whose profile the page edits captures, even when the
        // menu has passed to someone else (issue #194 B10, `App::controls_pad`).
        let owner = self.controls_pad();
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
            let pad = self.gilrs.as_ref().and_then(|g| match owner {
                Some(Some(id)) => g.connected_gamepad(id).map(|p| (id, p)),
                // Split: the player whose profile this is has no pad; nobody else's captures.
                Some(None) => None,
                None => input::active_gamepad(g, &self.active_pad),
            });
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
        // Split: the editing seat's own icon choice on their own pad (issue #194 B10).
        let set = self
            .editor_icon_set(controls.seat)
            .unwrap_or_else(|| self.icon_device.set(self.profile.settings.input_icons));
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

    /// The shell music (`ifs_start` sets `shell_soundtrack`, ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§2): a random
    /// take each time with the shell's gap between them, at its gain x the
    /// `shellmusic` bus x the profile's level (re-bf2-menus ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§4.4, menuref
    /// ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§9.3.9). Every menu screen's frame runs it, the LAN lobby's too
    /// (issue #128), so the takes follow one another for as long as the
    /// menus are up.
    pub(super) fn step_shell_music(&mut self, front: &mut FrontShell, dt: f32) {
        let now = std::time::Instant::now();
        let wall = front
            .music_clock
            .replace(now)
            .map_or(0.0, |t| now.duration_since(t).as_secs_f32());
        let due = front.music_due(dt, wall);
        if let (Some(k), Some((gain, takes)), Some(audio)) =
            (due, &front.music_takes, self.audio.as_mut())
        {
            let level = self.profile.settings.music_volume;
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

    /// Shell frame: the PS2 front screens (card BSHLA), the Instant Action
    /// screen or the settings page; navigate, maybe launch, draw.
    pub(super) fn redraw_shell(&mut self, dt: f32) {
        let mut edges = self.menu_edges();
        let mut front = self
            .front
            .take()
            .unwrap_or_else(|| FrontShell::new(&self.assets_dir));
        let (party_pads, party_presses) = self.poll_party_pads(&mut front, dt);
        if front.menus.page == ui::shell_ps2::Page::Credits {
            // The UI page owns navigation (any skip leaves for Singleplayer
            // with its own Exit cue); `Crawl` stays the game-side renderer it
            // invokes. With no crawl bake, leave the page rather than draw a
            // blank shell.
            if front.credits.is_none() {
                front.menus.open_page(ui::shell_ps2::Page::Singleplayer);
            } else {
                // ifs_credits plays no music of its own: the shell's
                // `shell_soundtrack` (ifs_start) carries on under the crawl.
                if front.active_gal_music {
                    front.stop_music();
                }
                self.step_shell_music(&mut front, dt);
                let _ = self.shell_pointer.take_wheel();
                let click = self.shell_pointer.take_frame().is_some_and(|(_, c)| c);
                // OURS: a click also skips, as Cross / Triangle / Start do.
                // replace: none, maintainer decision 2026-11-20.
                front.menus.step(
                    ui::menu::MenuInput {
                        confirm: edges & 16 != 0 || click,
                        back: edges & 32 != 0,
                        ..Default::default()
                    },
                    edges & 64 != 0,
                    dt,
                );
                let left = front.menus.page != ui::shell_ps2::Page::Credits;
                let done = left || front.credits.as_mut().is_none_or(|c| c.advance(dt));
                let mut failed = false;
                if !done {
                    if let (Some(r), Some(overlay), Some(window), Some(crawl)) = (
                        &mut self.renderer,
                        &mut self.overlay,
                        &self.window,
                        front.credits.as_mut(),
                    ) {
                        let paint = overlay.begin(window, |ctx| crawl.draw(ctx));
                        if let Err(error) = r.render_with_overlay(
                            &Camera::default(),
                            &[],
                            [0.0, 0.0, 0.0, 1.0],
                            Some(&mut |dev, q, enc, target, size| {
                                overlay.paint(&paint, dev, q, enc, target, size);
                            }),
                        ) {
                            eprintln!("render: {error:#}");
                            failed = true;
                        }
                    }
                }
                if failed {
                    self.request_close(super::CloseReason::Error);
                }
                if done {
                    // OURS: the menus start their own take afresh, not the
                    // one the crawl played over. replace: none, maintainer
                    // decision 2026-11-20.
                    front.credits = None;
                    front.stop_music();
                }
                self.front = Some(front);
                return;
            }
        }
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
        // A text box has the keyboard (issue #168): its Backspace is not
        // Back; Up and Down still move between the rows.
        if front.name_edit.is_some() {
            input = ui::menu::MenuInput::default();
            start = false;
        } else if front.text_focus {
            input = ui::menu::MenuInput {
                up: input.up,
                down: input.down,
                ..Default::default()
            };
            start = false;
        }
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
        let click = pointer.is_some_and(|(_, c)| c) && front.name_edit.is_none();
        if let (Some((point, _)), Some((w, h)), Some(view)) = (
            pointer,
            self.renderer.as_ref().map(|r| r.size()),
            front.view.as_ref(),
        ) {
            if !front.ia_open
                && front.settings.is_none()
                && self.controls.is_none()
                && front.mp.is_none()
                && front.picker.is_none()
                && front.save_name.is_none()
                && front.cutscenes.is_empty()
            {
                if let Some(row) = view.shell_row_at(&front.menus, point, [w as f32, h as f32]) {
                    front.menus.focus_row(row);
                    input.confirm |= click;
                } else if click {
                    if front.menus.page == ui::shell_ps2::Page::CampaignBattle {
                        input.confirm = true;
                    } else {
                        start |= view.shell_prompt_at(&front.menus, point, [w as f32, h as f32]);
                    }
                }
            }
        }
        if let (Some((point, true)), Some(renderer)) = (pointer, self.renderer.as_ref()) {
            let picker = front.picker.is_some();
            let main = front.menus.page == ui::shell_ps2::Page::Main
                && !front.ia_open
                && front.settings.is_none()
                && self.controls.is_none();
            if self.profile_store.is_some() && (picker || main) {
                let (w, h) = renderer.size();
                input.back |= ui::shell_ps2::profiles::back_at(point, [w as f32, h as f32], picker);
            }
        }
        let mut launch = None;
        // Both branches of the movie step below assign it.
        let launch_campaign;
        let mut quit = false;
        let mut settings_model = None;
        let mut controls_model = None;
        let stack_frame = !front.ia_open
            && self.controls.is_none()
            && front.settings.is_none()
            && front.mp.is_none()
            && front.cutscenes.is_empty();
        let mut mp_done = None;
        if !stack_frame {
            front.menus.tick(dt);
        }
        if front.mp.is_some() {
            // Square (Space) forgets a recent game; not while typing.
            let forget = edges & game::kbm::MENU_SQUARE != 0 && !front.text_focus;
            mp_done = self.step_mp(&mut front, input, forget, dt);
        } else if let Some(ia) = front.ia.as_mut().filter(|_| front.ia_open) {
            let view = front.view.as_ref();
            let command = self.step_instant(view, ia, input, edges, wheel, pointer, click, dt);
            match command {
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
            let native = front.view.as_ref().is_some_and(|v| v.can_draw_pause());
            let size = self.renderer.as_ref().map(|r| {
                let (w, h) = r.size();
                [w as f32, h as f32]
            });
            let click = pointer.and_then(|(point, down)| down.then_some(point));
            if let Some(row) = game::pause::binding_list_frame(
                controls_model.as_ref().filter(|_| !input.back && native),
                &mut front.settings_animation,
                wheel,
                click,
                size,
                dt,
            ) {
                if let Some(sound) = self.activate_control_row(row) {
                    front.cue(sound);
                }
                controls_model = self.controls_model(icons);
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
                network_meter: settings.network_meter,
                player_name: self.profile.name.clone(),
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
                hard_shadows: settings.hard_shadows,
                scene_softness: settings.scene_softness,
                auto_quality: settings.auto_quality,
                window_mode: self.shown_window_mode(),
                exclusive_fullscreen: self.display.exclusive,
                input_icons_label: settings.input_icons.label(),
                icons,
                first_person: settings.viewpoint.is_first(),
                cursor,
                page: Some(page),
                in_shell: Some(String::new()),
                // Issue #194: the Splitscreen page (the Splitscreen mode list's row), for the
                // Players screen's seats.
                split: (page == PausePage::Settings(ui::menu::SettingsPage::Splitscreen)).then(
                    || {
                        super::menu_actions::split_menu(
                            &settings.splitscreen,
                            party_seated(front.party.owners()),
                        )
                    },
                ),
                split_names: self.party_seat_names(&front.party),
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
                if let Some((w, h)) = self.renderer.as_ref().map(|r| r.size()) {
                    ui::pause_draw::scroll(
                        &model,
                        &front.settings_animation,
                        wheel,
                        [w as f32, h as f32],
                    );
                }
            }
            let (cursor, acts) = if let Some(hit) = pointer_hit {
                let result = ui::pause_draw::click(&mut model, &front.settings_animation, hit);
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
                        hard_shadows: self.profile.settings.hard_shadows,
                        auto_quality: self.profile.settings.auto_quality,
                        split: model.split.map(|_| {
                            super::menu_actions::split_menu(
                                &self.profile.settings.splitscreen,
                                party_seated(front.party.owners()),
                            )
                        }),
                        split_names: self.party_seat_names(&front.party),
                        ..model
                    });
        } else if !front.cutscenes.is_empty() {
            if input.confirm || input.back || start || click {
                front.skip_cutscene();
            }
        } else if front.save_name.is_some() {
            self.step_save_name(
                &mut front,
                profiles::FrameInput {
                    menu: input,
                    start,
                    edges,
                    dt,
                    pointer,
                    wheel,
                },
            );
        } else if front.menus.page == ui::shell_ps2::Page::Players && front.picker.is_none() {
            self.step_players(&mut front, party_pads, party_presses, pointer, wheel, dt);
        } else if !self.step_profiles(
            &mut front,
            profiles::FrameInput {
                menu: input,
                start,
                edges,
                dt,
                pointer,
                wheel,
            },
        ) {
            self.sync_campaign_menu_state(&mut front, edges);
            let cmd = front.menus.step(input, start, dt);
            self.save_campaign_menu_state(&mut front);
            match cmd {
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
                Some(
                    cmd @ (ui::shell_ps2::ShellCommand::StartCampaignNewGame
                    | ui::shell_ps2::ShellCommand::EditCampaignSaveName
                    | ui::shell_ps2::ShellCommand::WriteCampaignSave
                    | ui::shell_ps2::ShellCommand::LoadCampaignSave
                    | ui::shell_ps2::ShellCommand::DeleteCampaignSave
                    | ui::shell_ps2::ShellCommand::LaunchCampaignMission { .. }),
                ) => self.campaign_command(&mut front, cmd),
                Some(ui::shell_ps2::ShellCommand::Settings(page)) => {
                    front.settings = Some((PausePage::Settings(page), 0));
                }
                // Issue #194 B19: opened from the Splitscreen mode list, the
                // session takes the Players screen's seats along.
                Some(ui::shell_ps2::ShellCommand::HostGame) => {
                    self.split_party = (front.menus.multiplayer_split && front.party.can_start())
                        .then(|| front.party.owners().clone());
                    front.mp = Some(super::lobby::MpPage::host());
                }
                Some(ui::shell_ps2::ShellCommand::JoinGame) => {
                    self.split_party = (front.menus.multiplayer_split && front.party.can_start())
                        .then(|| front.party.owners().clone());
                    front.mp = Some(super::lobby::MpPage::join(self.recent_rows()));
                }
                Some(ui::shell_ps2::ShellCommand::Quit) => quit = true,
                None => {}
            }
        }
        let mut has_movie = false;
        if !front.cutscenes.is_empty() {
            let (shown, launch) = self.step_shell_cutscene(
                &mut front,
                dt,
                false,
                self.profile.settings.dialogue_volume,
            );
            has_movie = shown;
            launch_campaign = launch;
        } else {
            front.movie_audio = None;
            launch_campaign = front.pending_campaign.take();
            let launching = launch.is_some() || launch_campaign.is_some();
            let on_planet_music = !launching
                && front.menus.over_planet()
                && !front.ia_open
                && front.settings.is_none()
                && self.controls.is_none()
                && front.mp.is_none()
                && front.picker.is_none()
                && front.gal_music.is_some();
            if on_planet_music {
                if !front.active_gal_music {
                    front.stop_music();
                    front.active_gal_music = true;
                }
                if let (Some((gain, takes)), Some(audio)) = (&front.gal_music, self.audio.as_mut())
                {
                    let level = self.profile.settings.music_volume;
                    if level <= 0.0 {
                        front.music = None;
                    } else if front.music.is_none() || front.music_replay {
                        front.music_replay = false;
                        front.music = audio
                            .play_stream(
                                takes[0].clone(),
                                game::sounds::level_db(*gain * level),
                                true,
                            )
                            .map_err(|e| eprintln!("campaign music: {e:#}"))
                            .ok();
                    }
                }
            } else {
                if front.active_gal_music {
                    front.stop_music();
                }
                if !launching {
                    self.step_shell_music(&mut front, dt);
                }
            }
            let movie_name = if launching { "" } else { front.menus.movie() };
            if movie_name.is_empty() {
                if let Some(m) = front.movie.as_mut() {
                    m.stop();
                }
                has_movie = false;
            } else if let Some(m) = front.movie.as_mut() {
                // A copy without the page's movie (the PC has no planet
                // transitions, the Xbox no geo1tran1) shows the sub-menus'
                // loop rather than a black backdrop.
                let movie_name = if m.has(movie_name) {
                    movie_name
                } else {
                    ui::shell_ps2::MOVIE_SUB
                };
                m.play(movie_name);
                if let Some((rgba, w, h)) = m.frame(dt) {
                    if let Some(v) = front.view.as_mut() {
                        v.set_movie_frame(rgba, w, h);
                    }
                }
                has_movie = m.shown;
            }
        }
        let launching = launch.is_some() || launch_campaign.is_some();
        front.menus.covered = front.ia_open
            || front.settings.is_some()
            || self.controls.is_some()
            || front.mp.is_some()
            || front.picker.is_some()
            || front.save_name.is_some()
            || !front.cutscenes.is_empty()
            || launching;
        // The IA screen has its own backdrop and no movie (ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§1.1); Session
        // Options keep `shell_sub_left`.
        let ia = front
            .ia
            .as_ref()
            .filter(|_| front.ia_open && front.settings.is_none() && self.controls.is_none());
        let movie_shown = has_movie && ia.is_none_or(|ia| ia.page != ui::shell::Page::Missions);
        let icons = self.current_icons();
        let mut control_clicked = None;
        let mut setting_clicked = Vec::new();
        let mut pending_mp = None;
        let mut pending_name = None;
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
                        if let Some(picker) = front.picker.as_ref().or(front.save_name.as_ref()) {
                            v.update_shell_profiles(&front.menus, has_movie, picker, [w, h]);
                        } else if let Some(model) = controls_model
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
            let notice = &mut self.load_notice;
            let mp = &mut front.mp;
            let name_edit = &mut front.name_edit;
            let mut mp_draw = None;
            let mut name_done = None;
            let paint = overlay.begin(window, |ctx| {
                if let Some(page) = mp.as_mut() {
                    mp_draw = Some(super::lobby::draw_mp(ctx, page));
                }
                if let Some((name, error)) = name_edit.as_mut() {
                    name_done = ui::lobby::draw_name_box(ctx, name, error.as_deref());
                }
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
                notice.draw(ctx, std::time::Instant::now(), game::session_log::path());
            });
            front.text_focus = mp_draw.is_some_and(|d| d.text_focus);
            if let Some(d) = mp_draw {
                pending_mp = Some(d);
            }
            if let Some(keep) = name_done {
                pending_name = Some(keep);
            }
            let (cam, planet_draws) =
                campaign::planet_for_frame(&mut front, &self.assets_dir, r, dt);
            let view = &mut front.view;
            if let Err(e) = r.render_with_overlay(
                &cam,
                &planet_draws,
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, target, size| {
                    if let Some(v) = view.as_mut() {
                        v.paint(dev, q, enc, target, size);
                    }
                    overlay.paint(&paint, dev, q, enc, target, size)
                }),
            ) {
                eprintln!("render: {e}");
                self.request_close(super::CloseReason::Error);
            }
        }
        if let Some(sound) = control_clicked.and_then(|row| self.activate_control_row(row)) {
            front.cue(sound);
        }
        if let Some(d) = pending_mp {
            mp_done = mp_done.or_else(|| self.after_mp_draw(&mut front, d));
        }
        if let Some(keep) = pending_name {
            self.close_name_box(&mut front, keep);
        }
        for action in setting_clicked {
            if let Some(s) = shell_click_sound(action) {
                front.cue(s);
            }
            self.apply_shell_setting(&mut front, action);
        }
        // A page of the port's own closed: the screen under it is entered
        // again (its exit click, its entrance).
        let covered = front.ia_open
            || front.settings.is_some()
            || self.controls.is_some()
            || front.mp.is_some();
        if front.was_covered && !covered {
            front.menus.uncover();
        }
        front.was_covered = covered;
        if let Some(sounds) = front.sounds.as_mut() {
            let s = &self.profile.settings;
            sounds.set_user_volumes(s.effects_volume, s.dialogue_volume);
        }
        front.play_cues(self.audio.as_mut(), dt);
        // Issue #194: an Instant Action launched over the Splitscreen mode list plays with the
        // Players screen's seats, in their order; any other launch plays alone.
        let split_launch = (front.menus.page == ui::shell_ps2::Page::SplitModes
            && front.party.can_start())
        .then(|| front.party.owners().clone());
        self.front = Some(front);
        if quit {
            self.request_close(super::CloseReason::Quit);
        }
        if let Some((rows, options)) = launch {
            self.split_party = split_launch;
            self.start_playlist(rows, options);
        }
        if let Some(index) = launch_campaign {
            self.start_campaign(index);
        }
        if let Some(done) = mp_done {
            self.mp_done(done);
        }
    }

    /// One frame of an Instant Action screen's presses: the pad and keys,
    /// the wheel and the pointer (the shell's and the LAN lobby's).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn step_instant(
        &self,
        view: Option<&game::hud_feed::HudView>,
        ia: &mut ui::shell::InstantAction,
        input: ui::menu::MenuInput,
        edges: u32,
        wheel: f32,
        pointer: Option<([f32; 2], bool)>,
        click: bool,
        dt: f32,
    ) -> Option<ui::shell::IaCommand> {
        let mut ia_input = ui::shell::IaInput {
            menu: input,
            l1: edges & game::kbm::MENU_L1 != 0,
            r1: edges & game::kbm::MENU_R1 != 0,
        };
        if wheel != 0.0 {
            // The wheel's own amount, a trackpad's fractions too: the list's
            // view eases there (`ui::scroll`).
            let lines = -wheel;
            let point = self.shell_pointer.pos();
            let size = self.renderer.as_ref().map(|r| r.size());
            let require_hover = self
                .display_ext
                .as_ref()
                .is_some_and(|ext| ext.has_menu_pointer());
            ia.scroll_pointer(
                point,
                size.map(|(w, h)| [w as f32, h as f32]),
                lines,
                require_hover,
            );
        }
        let pointer = if wheel != 0.0 && !click {
            None
        } else {
            pointer
        };
        if let (Some((point, _)), Some((w, h))) =
            (pointer, self.renderer.as_ref().map(|r| r.size()))
        {
            if let Some(target) =
                view.and_then(|view| view.instant_target(ia, point, [w as f32, h as f32]))
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
                                ia.scroll_left(delta as f32);
                            }
                            ui::shell::PointerTarget::ScrollRight(delta) => {
                                ia.scroll_right(delta as f32);
                            }
                            ui::shell::PointerTarget::ScrollOption(delta) => {
                                ia.scroll_option(delta as f32);
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
        ia.step(ia_input, dt)
    }

    /// Game Options' Player name box closed: `keep` saves a valid name
    /// (`net::msg::valid_name`), else the box stays with why.
    fn close_name_box(&mut self, front: &mut FrontShell, keep: bool) {
        let Some((name, _)) = front.name_edit.take() else {
            return;
        };
        if !keep {
            front.cue(MenuSound::Exit);
            return;
        }
        let name = name.trim().to_owned();
        if !net::msg::valid_name(&name) {
            front.name_edit = Some((
                name,
                Some("A name is 1 to 24 characters, not only spaces".into()),
            ));
            front.cue(MenuSound::Error);
            return;
        }
        front.cue(MenuSound::Accept);
        self.profile.name = name;
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
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
            MenuAction::EditPlayerName => {
                front.name_edit =
                    Some((game::netplay::lobby::player_name(&self.profile.name), None));
                return;
            }
            MenuAction::SetDifficulty(v) => {
                s.difficulty = game::shell_ia::port_difficulty(v);
                if let Some(ia) = front.ia.as_mut() {
                    ia.options.difficulty = v;
                }
            }
            MenuAction::SetEffectsVolume(v) => s.effects_volume = v.clamp(0.0, 1.0),
            MenuAction::SetDialogueVolume(v) => s.dialogue_volume = v.clamp(0.0, 1.0),
            MenuAction::SetFpsCounter(v) => s.fps_counter = v as u8,
            MenuAction::SetNetworkMeter(on) => s.network_meter = on,
            MenuAction::SetAutoQuality(on) => s.auto_quality = on,
            MenuAction::SetWindowMode(mode) => {
                self.set_window_mode(mode);
                return;
            }
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
                let gain = if front.active_gal_music {
                    front.gal_music.as_ref()
                } else {
                    front.music_takes.as_ref()
                }
                .map_or(1.0, |(g, _)| *g);
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
            MenuAction::ToggleHardShadows => s.hard_shadows = !s.hard_shadows,
            MenuAction::ToggleSceneSoftness => s.scene_softness = !s.scene_softness,
            MenuAction::ToggleInvertY => s.invert_y = !s.invert_y,
            MenuAction::ToggleLowPowerResolution => {
                s.low_power_resolution = !s.low_power_resolution
            }
            MenuAction::CycleInputIcons(dir) => s.input_icons = s.input_icons.cycled(dir),
            MenuAction::ToggleViewpoint => s.viewpoint = s.viewpoint.toggled(),
            // Issue #194: the Splitscreen page, from the Splitscreen mode list.
            MenuAction::SetSplitTwoPlayer(_)
            | MenuAction::MoveSplitScreen { .. }
            | MenuAction::SetSplitNameTags(_)
            | MenuAction::SetSplitHudSize(_) => {
                super::menu_actions::apply_split_setting(
                    &mut s.splitscreen,
                    a,
                    party_seated(front.party.owners()),
                );
            }
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
        self.refresh_context_bindings();
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
    }
}

#[cfg(test)]
#[path = "front/tests.rs"]
mod tests;
#[cfg(test)]
pub(super) use tests::test_take;
