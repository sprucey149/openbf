//! Startup profile selection and the active save binding.

use super::{App, FrontShell};
use game::profile::{valid_name_char, Store, NAME_LIMIT};
use ui::{
    menu::MenuInput,
    shell_feel::MenuSound,
    shell_ps2::{
        profiles::{Page, Picker, Target},
        Page as ShellPage,
    },
};

#[derive(Default)]
pub(super) struct NameInput {
    pub(super) text: String,
    pub(super) backspaces: usize,
}

fn picker(store: &Store) -> Picker {
    match store.list() {
        Ok(list) => Picker::new(list.names, list.last),
        Err(error) => {
            eprintln!("profiles: {error:#}");
            let mut picker = Picker::new(Vec::new(), None);
            picker.message = Some("Could not read profiles".into());
            picker
        }
    }
}

#[derive(Default)]
pub(super) struct FrameInput {
    pub menu: MenuInput,
    pub start: bool,
    pub edges: u32,
    pub dt: f32,
    pub pointer: Option<([f32; 2], bool)>,
    pub wheel: f32,
}

/// What the one name keyboard asks its screen to do after a frame. The
/// mechanics (typed letters, backspace, the symbols page, focus and pointer
/// clicks) and the common key / symbols / backspace targets are applied in
/// [`step_name_keyboard`]; the two screens keep their own prompts and
/// accept / cancel behaviour.
pub(super) enum NameKeyAction {
    /// Keyboard stays up; nothing else to do.
    None,
    /// The symbols page was toggled: return from the frame's step.
    Symbols,
    /// Back was pressed: close the keyboard the screen's way.
    Cancel,
    /// A picker target the screen must apply (its accept or its profile action).
    Target(Target),
}

/// The screen-specific bits of the shared keyboard. Profile creation pages
/// and scrolls its name list with L1/R1 and the wheel, lets Square delete a
/// listed profile, and only types on the Name page. The save-name keyboard is
/// a bare name page with none of those.
pub(super) struct NameKeys<'a> {
    pub frame: &'a FrameInput,
    pub name_limit: usize,
    pub renderer: Option<&'a render::Renderer>,
    pub list: bool,
}

/// One frame of the profile picker's name keyboard, shared by profile
/// creation and the campaign save-name screen. Both put a `Picker` on `front`
/// (`front.picker` / `front.save_name`); this steps the typed glyphs fed by
/// [`App::profile_keyboard`], the backspaces and symbols key, the focus and
/// pointer clicks, and handles the common key / symbols / backspace targets.
/// The caller applies the returned accept or cancel with its own cues.
pub(super) fn step_name_keyboard(
    menus: &mut ui::shell_ps2::ShellMenus,
    name_input: &mut NameInput,
    pick: &mut Picker,
    keys: NameKeys<'_>,
) -> NameKeyAction {
    let NameKeys {
        frame,
        name_limit,
        renderer,
        list,
    } = keys;
    let editing = matches!(pick.page, Page::Name { .. });
    menus.tick(frame.dt);
    pick.advance(frame.dt);
    let typed = std::mem::take(name_input);
    if !list || editing {
        if typed.backspaces != 0 || frame.edges & game::kbm::MENU_SQUARE != 0 {
            for _ in 0..typed.backspaces.max(1) {
                pick.backspace();
            }
            menus.cues.play(MenuSound::Backspace);
        }
        if !typed.text.is_empty() {
            pick.type_name(&typed.text, name_limit);
            menus.cues.play(MenuSound::Key);
        }
        if frame.edges & game::kbm::MENU_EAST != 0 {
            pick.toggle_symbols();
            menus.cues.play(MenuSound::Key);
            return NameKeyAction::Symbols;
        }
    }
    if frame.menu.back {
        return NameKeyAction::Cancel;
    }
    let old = pick.focus;
    let mut x = isize::from(frame.menu.right) - isize::from(frame.menu.left);
    if !list || editing {
        x += isize::from(frame.edges & game::kbm::MENU_R1 != 0)
            - isize::from(frame.edges & game::kbm::MENU_L1 != 0);
    }
    let y = isize::from(frame.menu.down) - isize::from(frame.menu.up);
    if x != 0 || y != 0 {
        pick.move_focus(x, y);
    }
    // L1 / R1 page the profile list; the save keyboard only moves focus.
    if list {
        if frame.edges & game::kbm::MENU_L1 != 0 {
            pick.page_list(-1);
        } else if frame.edges & game::kbm::MENU_R1 != 0 {
            pick.page_list(1);
        }
        if frame.wheel != 0.0 {
            pick.wheel(frame.wheel);
        }
    }
    let mut action = if frame.start && (!list || editing) {
        Some(Target::Accept)
    } else if frame.menu.confirm {
        Some(pick.focus)
    } else if list && matches!(pick.page, Page::List) && frame.edges & game::kbm::MENU_SQUARE != 0 {
        Some(Target::Delete)
    } else {
        None
    };
    if let (Some((point, click)), Some(renderer)) = (frame.pointer, renderer) {
        let (w, h) = renderer.size();
        if let Some(target) = pick.target(point, [w as f32, h as f32]) {
            if !matches!(
                target,
                Target::Delete | Target::Previous | Target::Next | Target::Symbols
            ) {
                pick.point_focus(target);
            }
            if click {
                action = Some(target);
            }
        }
    }
    if pick.focus != old {
        menus.cues.play(MenuSound::Select);
    }
    match action {
        Some(Target::Key(i)) => {
            if let Some(c) = pick.keys().as_bytes().get(i) {
                let mut key = [0; 1];
                pick.type_name(char::from(*c).encode_utf8(&mut key), name_limit);
            }
            menus.cues.play(MenuSound::Key);
            NameKeyAction::None
        }
        Some(Target::Symbols) => {
            pick.toggle_symbols();
            menus.cues.play(MenuSound::Key);
            NameKeyAction::Symbols
        }
        Some(Target::Backspace) => {
            pick.backspace();
            menus.cues.play(MenuSound::Backspace);
            NameKeyAction::None
        }
        Some(Target::Cancel) => NameKeyAction::Cancel,
        Some(other) => NameKeyAction::Target(other),
        None => NameKeyAction::None,
    }
}

impl App {
    pub(super) fn step_profiles(&mut self, front: &mut FrontShell, frame: FrameInput) -> bool {
        let Some(store) = self.profile_store.as_ref() else {
            return false;
        };
        // Issue #194: profiles P2-P4 hold are greyed here and refused (`title_refuses`).
        let held = front.party.guest_ids();
        if let Some(pick) = front.picker.as_mut() {
            pick.taken = held;
        }
        let Some(pick) = front.picker.as_mut() else {
            let cue = match (front.menus.page, frame.start, frame.menu.back) {
                (ShellPage::Title, true, _) => MenuSound::Accept,
                (ShellPage::Main, _, true) => MenuSound::Exit,
                _ => return false,
            };
            front.picker = Some(picker(store));
            front.menus.tick(frame.dt);
            front.cue(cue);
            return true;
        };

        match step_name_keyboard(
            &mut front.menus,
            &mut front.name_input,
            pick,
            NameKeys {
                frame: &frame,
                name_limit: NAME_LIMIT,
                renderer: self.renderer.as_ref(),
                list: true,
            },
        ) {
            NameKeyAction::None => {}
            NameKeyAction::Symbols => return true,
            NameKeyAction::Cancel => {
                if matches!(front.picker.as_ref().map(|p| &p.page), Some(Page::List)) {
                    front.picker = None;
                    front.menus.profile_cancelled();
                } else if front
                    .picker
                    .as_ref()
                    .is_some_and(|p| matches!(p.page, Page::Delete(_)))
                {
                    self.profile_action(front, Target::No);
                } else {
                    self.profile_action(front, Target::Cancel);
                }
                front.cue(MenuSound::Exit);
                return true;
            }
            NameKeyAction::Target(action) => self.profile_action(front, action),
        }
        true
    }

    fn profile_action(&mut self, front: &mut FrontShell, action: Target) {
        let pick = front.picker.as_mut().unwrap();
        let store = self.profile_store.as_ref().unwrap();
        if game::local_seats::title_refuses(pick, action) {
            pick.message = Some(game::local_seats::TAKEN.into());
            front.cue(MenuSound::Error);
            return;
        }
        let result = match action {
            Target::Profile(i) => store.select(pick.names[i].0).map(Some),
            Target::Create => {
                let suggestion = (1..)
                    .map(|i| format!("Player {i}"))
                    .find(|name| !pick.names.iter().any(|(_, n)| n.eq_ignore_ascii_case(name)))
                    .unwrap();
                pick.edit_name(suggestion);
                Ok(None)
            }
            Target::Delete => {
                let Some((id, _)) = pick.selected() else {
                    return;
                };
                pick.page = Page::Delete(id);
                pick.set_focus(Target::No);
                Ok(None)
            }
            Target::Key(i) => {
                if let Some(c) = pick.keys().as_bytes().get(i) {
                    let mut key = [0; 1];
                    pick.type_name(char::from(*c).encode_utf8(&mut key), NAME_LIMIT);
                }
                front.cue(MenuSound::Key);
                return;
            }
            Target::Symbols => {
                pick.toggle_symbols();
                front.cue(MenuSound::Key);
                return;
            }
            Target::Backspace => {
                pick.backspace();
                front.cue(MenuSound::Backspace);
                return;
            }
            Target::Accept => {
                let Page::Name { text, .. } = &pick.page else {
                    return;
                };
                let name = text.trim();
                let message = if name.is_empty() {
                    Some("Enter at least one character")
                } else if pick
                    .names
                    .iter()
                    .any(|(_, n)| n.trim().eq_ignore_ascii_case(name))
                {
                    Some("That profile name already exists")
                } else {
                    None
                };
                if let Some(message) = message {
                    pick.message = Some(message.into());
                    front.cue(MenuSound::Error);
                    return;
                }
                pick.page = Page::Difficulty(name.to_owned());
                pick.set_focus(Target::Difficulty(1));
                Ok(None)
            }
            Target::Difficulty(i) => {
                let Page::Difficulty(name) = &pick.page else {
                    return;
                };
                store
                    .create(name, core::skill::Difficulty::ALL[i])
                    .map(Some)
            }
            Target::Yes => {
                let Page::Delete(id) = pick.page else { return };
                if let Err(error) = store.delete(id) {
                    Err(error)
                } else {
                    *pick = picker(store);
                    front.cue(MenuSound::Accept);
                    return;
                }
            }
            Target::No => {
                let Page::Delete(id) = pick.page else { return };
                pick.page = Page::List;
                pick.set_focus(
                    pick.names
                        .iter()
                        .position(|(key, _)| *key == id)
                        .map_or(Target::Create, Target::Profile),
                );
                Ok(None)
            }
            Target::Cancel => {
                if let Page::Difficulty(name) = &pick.page {
                    pick.resume_name(name.clone());
                } else {
                    pick.page = Page::List;
                    pick.set_focus(Target::Create);
                }
                Ok(None)
            }
            Target::Previous | Target::Next => {
                pick.page_list(if action == Target::Previous { -1 } else { 1 });
                front.cue(MenuSound::Select);
                return;
            }
        };
        match result {
            Ok(Some(profile)) => {
                self.apply_host_profile(front, profile);
                self.session = Default::default();
                front.ia = None;
                front.menus.profile_selected();
                front.picker = None;
                front.cue(MenuSound::Accept);
            }
            Ok(None) => {
                pick.message = None;
                front.cue(MenuSound::Accept);
            }
            Err(error) => {
                eprintln!("profiles: {error:#}");
                pick.message = Some(error.to_string());
                front.cue(MenuSound::Error);
            }
        }
    }

    /// `profile` becomes the current profile (seat 0's, the host's): its window mode, keys,
    /// bindings, pad warning, music level and campaign unlocks. The title picker also resets
    /// the session and goes to the main menu; the Players screen's seat 0 picker calls only
    /// this, so the other seats and the session options stay as they were (issue #194).
    pub(super) fn apply_host_profile(
        &mut self,
        front: &mut FrontShell,
        profile: game::profile::Profile,
    ) {
        self.profile = profile;
        // The save list's current save was the old profile's.
        front.menus.campaign_saves.current = None;
        super::super::display::hold_window_mode(&mut self.display.held, &mut self.profile);
        self.apply_window_mode();
        self.kbm.follow(&self.profile.settings);
        self.kbm.release_keys();
        self.refresh_context_bindings();
        if let Some(g) = &self.gilrs {
            self.pad_warning = input::log_pads_with(
                g,
                &self.profile.settings.controller_bindings,
                &self.active_pad,
            );
        }
        if let (Some(voice), Some((gain, _))) = (&mut front.music, &front.music_takes) {
            voice.set_volume(
                game::sounds::level_db(*gain * self.profile.settings.music_volume),
                0.1,
            );
        } else {
            front.music_replay = true;
        }
        front
            .menus
            .set_campaign_unlocked(self.profile.campaign.unlocked);
    }

    pub(in super::super) fn profile_keyboard(&mut self, event: &winit::event::WindowEvent) -> bool {
        use winit::{
            event::WindowEvent,
            keyboard::{Key, NamedKey},
        };
        let Some(front) = self.front.as_mut() else {
            return false;
        };
        let seat_typing = front.menus.page == ShellPage::Players
            && front.picker.is_none()
            && front.party.keyboard_typing();
        if !seat_typing
            && !front
                .picker
                .as_ref()
                .or(front.save_name.as_ref())
                .is_some_and(|p| matches!(p.page, Page::Name { .. }))
        {
            return false;
        }
        let WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } = event
        else {
            return false;
        };
        if event.logical_key == Key::Named(NamedKey::Backspace) {
            if event.state.is_pressed() {
                front.name_input.backspaces = front.name_input.backspaces.saturating_add(1);
            }
            return true;
        }
        if event.state.is_pressed() {
            let text: String = event
                .text
                .as_deref()
                .unwrap_or("")
                .chars()
                .filter(|&c| valid_name_char(c))
                .collect();
            if !text.is_empty() {
                front.name_input.text.push_str(&text);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::skill::Difficulty;
    use std::path::PathBuf;

    #[test]
    fn profile_picker_routes_start_create_switch_and_delete() {
        let dir =
            std::env::temp_dir().join(format!("openbf-profile-picker-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let store = Store::new(dir.clone());
        let mut app = App::new(
            PathBuf::new(),
            PathBuf::new(),
            super::super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile_store = Some(store.clone());
        let mut front = FrontShell::new(&dir);
        let step = |app: &mut App, front: &mut FrontShell, menu: MenuInput, start, edges| {
            assert!(app.step_profiles(
                front,
                FrameInput {
                    menu,
                    start,
                    edges,
                    ..Default::default()
                }
            ));
        };
        let confirm = MenuInput {
            confirm: true,
            ..Default::default()
        };
        let back = MenuInput {
            back: true,
            ..Default::default()
        };
        let start = MenuInput::default();

        step(&mut app, &mut front, start, true, 0);
        assert_eq!(front.menus.page, ShellPage::Title);
        assert_eq!(front.picker.as_ref().unwrap().focus, Target::Create);
        step(&mut app, &mut front, confirm, false, 0);
        front.name_input.text = "Pilot".into();
        step(&mut app, &mut front, confirm, true, 0);
        assert_eq!(front.picker.as_ref().unwrap().focus, Target::Difficulty(1));
        step(&mut app, &mut front, confirm, false, 0);
        assert_eq!(front.menus.page, ShellPage::Main);
        assert!(front.picker.is_none());
        assert_eq!(app.profile.name, "Pilot");
        app.profile.career.kills = 25;
        app.profile.save().unwrap();

        store.create("Rebel", Difficulty::Easy).unwrap();
        step(&mut app, &mut front, back, false, 0);
        assert_eq!(front.picker.as_ref().unwrap().focus, Target::Profile(1));
        step(&mut app, &mut front, confirm, false, 0);
        assert_eq!(app.profile.name, "Rebel");
        assert_eq!(app.profile.settings.difficulty, Difficulty::Easy);
        assert_eq!(app.profile.career.kills, 0);

        step(&mut app, &mut front, back, false, 0);
        step(&mut app, &mut front, start, false, game::kbm::MENU_SQUARE);
        assert_eq!(front.picker.as_ref().unwrap().focus, Target::No);
        step(&mut app, &mut front, confirm, false, 0);
        assert_eq!(store.list().unwrap().names.len(), 2);
        step(&mut app, &mut front, start, false, game::kbm::MENU_SQUARE);
        step(
            &mut app,
            &mut front,
            MenuInput {
                down: true,
                ..Default::default()
            },
            false,
            0,
        );
        step(&mut app, &mut front, confirm, false, 0);
        assert_eq!(store.list().unwrap().names.len(), 1);
        step(&mut app, &mut front, confirm, false, 0);
        assert_eq!(app.profile.name, "Pilot");
        assert_eq!(app.profile.career.kills, 25);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Issue #194: seat 0's pick on the Players screen changes the current profile and
    /// nothing else: the page, the session options and the Instant Action screen stay.
    #[test]
    fn a_seat_zero_pick_keeps_the_players_screen_and_the_session() {
        let dir = std::env::temp_dir().join(format!("openbf-profile-seat0-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let store = Store::new(dir.clone());
        let mut app = App::new(
            PathBuf::new(),
            PathBuf::new(),
            super::super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile_store = Some(store.clone());
        app.profile = store.create("Ann", Difficulty::Normal).unwrap();
        let mut front = FrontShell::new(&dir);
        front.menus.open_page(ShellPage::Players);
        app.session.num_bots = 17;
        let session = app.session;
        let mut cy = store.create_quiet("Cy", Difficulty::Hard).unwrap();
        cy.settings.music_volume = 0.25;
        app.apply_host_profile(&mut front, cy);
        assert_eq!(app.profile.name, "Cy");
        assert_eq!(app.profile.settings.difficulty, Difficulty::Hard);
        assert_eq!(
            front.menus.page,
            ShellPage::Players,
            "the Players screen stays"
        );
        assert_eq!(app.session, session, "the session options as they were");
        assert!(front.picker.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Issue #194 review: P2-P4's profiles go into every split round that starts loading (the
    /// next round and a Restart Mission included), come back on the shell, and stay on their
    /// seats when nothing loads.
    #[test]
    fn split_profiles_are_lent_every_round_and_back_on_the_shell() {
        use game::local_seats::{PartyEnv, PartyFrame, SeatProfile};
        let dir = std::env::temp_dir().join(format!("openbf-split-lend-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let mut app = App::new(
            PathBuf::new(),
            PathBuf::new(),
            super::super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        let mut front = FrontShell::new(&dir);
        // The keyboard on seat 1 (a pad seat needs a real gilrs id); with no catalog it sits
        // at once on an unsaved "Player 2".
        let mut kb = input::owners::PadOwners::<gilrs::GamepadId>::default();
        kb.join_keyboard();
        let owners = kb.unpacked(&[1]);
        front.set_party(owners.clone());
        let env = PartyEnv {
            store: None,
            host_id: None,
            difficulty: Difficulty::Normal,
        };
        front.party.step(&PartyFrame::default(), &env);
        assert!(matches!(front.party.profile(1), SeatProfile::Ready(p) if p.name == "Player 2"));
        app.front = Some(front);
        app.split_party = Some(owners);
        let lent = |app: &App| app.split_profiles[1].as_ref().map(|p| p.name.clone());
        let on_seat = |app: &App| {
            matches!(
                app.front.as_ref().unwrap().party.profile(1),
                SeatProfile::Ready(_)
            )
        };
        // A launch whose rows all fail to load lends nothing.
        app.start_playlist(Vec::new(), Default::default());
        assert_eq!(lent(&app), None);
        assert!(on_seat(&app));
        for round in 1..=3 {
            // A round starts loading (`next_round`), then leaves the battle (the round's end,
            // or Restart Mission) before the next one loads.
            app.lend_split_profiles();
            assert_eq!(lent(&app).as_deref(), Some("Player 2"), "round {round}");
            assert!(
                !on_seat(&app),
                "round {round}: one live copy, in the battle"
            );
            app.leave_battle();
            assert_eq!(lent(&app), None, "round {round}");
            assert!(on_seat(&app), "round {round}: back on its seat");
        }
        // A load that fails after the lend (the worker's error) returns to the shell too.
        app.lend_split_profiles();
        app.init_shell();
        assert!(on_seat(&app));
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A2 wiring: F11 on the title screen, before any profile is picked,
    /// writes nothing and reaches the profile created next.
    #[test]
    fn a_title_screen_window_mode_reaches_the_profile_created_next() {
        use ui::menu::WindowMode;
        let dir =
            std::env::temp_dir().join(format!("openbf-profile-picker-mode-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let store = Store::new(dir.clone());
        let mut app = App::new(
            PathBuf::new(),
            PathBuf::new(),
            super::super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile_store = Some(store.clone());
        assert!(!app.profile.persists());

        // A first launch is fullscreen; F11 on the title screen leaves it.
        assert_eq!(app.profile.settings.window_mode, WindowMode::Borderless);
        app.set_window_mode(WindowMode::Windowed);
        assert_eq!(app.display.held, Some(WindowMode::Windowed));
        assert!(!dir.join("profile.ron").exists());
        assert!(store.list().unwrap().names.is_empty());

        let mut front = FrontShell::new(&dir);
        let confirm = MenuInput {
            confirm: true,
            ..Default::default()
        };
        let step = |app: &mut App, front: &mut FrontShell, menu: MenuInput, start| {
            assert!(app.step_profiles(
                front,
                FrameInput {
                    menu,
                    start,
                    ..Default::default()
                }
            ));
        };
        step(&mut app, &mut front, MenuInput::default(), true);
        step(&mut app, &mut front, confirm, false);
        front.name_input.text = "Pilot".into();
        step(&mut app, &mut front, confirm, true);
        step(&mut app, &mut front, confirm, false);
        assert_eq!(app.profile.name, "Pilot");
        assert_eq!(app.display.held, None);
        assert_eq!(app.profile.settings.window_mode, WindowMode::Windowed);
        let (id, _) = store.list().unwrap().names[0].clone();
        let saved = store.select(id).unwrap();
        assert_eq!(saved.settings.window_mode, WindowMode::Windowed);
        assert_eq!(saved.settings.last_fullscreen, WindowMode::Borderless);
        assert!(!dir.join("profile.ron").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
