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
    text: String,
    backspaces: usize,
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

impl App {
    pub(super) fn step_profiles(&mut self, front: &mut FrontShell, frame: FrameInput) -> bool {
        let FrameInput {
            menu,
            start,
            edges,
            dt,
            pointer,
            wheel,
        } = frame;
        let Some(store) = self.profile_store.as_ref() else {
            return false;
        };
        let Some(pick) = front.picker.as_mut() else {
            let cue = match (front.menus.page, start, menu.back) {
                (ShellPage::Title, true, _) => MenuSound::Accept,
                (ShellPage::Main, _, true) => MenuSound::Exit,
                _ => return false,
            };
            front.picker = Some(picker(store));
            front.menus.tick(dt);
            front.cue(cue);
            return true;
        };

        front.menus.tick(dt);
        let editing = matches!(pick.page, Page::Name { .. });
        let name_input = std::mem::take(&mut front.name_input);
        if editing {
            if name_input.backspaces != 0 || edges & game::kbm::MENU_SQUARE != 0 {
                for _ in 0..name_input.backspaces.max(1) {
                    pick.backspace();
                }
                front.menus.cues.play(MenuSound::Backspace);
            }
            if !name_input.text.is_empty() {
                pick.type_name(&name_input.text, NAME_LIMIT);
                front.menus.cues.play(MenuSound::Key);
            }
            if edges & game::kbm::MENU_EAST != 0 {
                pick.toggle_symbols();
                front.menus.cues.play(MenuSound::Key);
                return true;
            }
        }
        if menu.back {
            match pick.page {
                Page::List => {
                    front.picker = None;
                    front.menus.profile_cancelled();
                }
                Page::Delete(_) => self.profile_action(front, Target::No),
                _ => self.profile_action(front, Target::Cancel),
            }
            front.cue(MenuSound::Exit);
            return true;
        }

        let old = pick.focus;
        let mut x = isize::from(menu.right) - isize::from(menu.left);
        if editing {
            x += isize::from(edges & game::kbm::MENU_R1 != 0)
                - isize::from(edges & game::kbm::MENU_L1 != 0);
        }
        let y = isize::from(menu.down) - isize::from(menu.up);
        if x != 0 || y != 0 {
            pick.move_focus(x, y);
        }
        if edges & game::kbm::MENU_L1 != 0 || wheel > 0.0 {
            pick.page_list(-1);
        } else if edges & game::kbm::MENU_R1 != 0 || wheel < 0.0 {
            pick.page_list(1);
        }
        let mut action = if editing && start {
            Some(Target::Accept)
        } else if menu.confirm {
            Some(pick.focus)
        } else if matches!(pick.page, Page::List) && edges & game::kbm::MENU_SQUARE != 0 {
            Some(Target::Delete)
        } else {
            None
        };
        if let (Some((point, click)), Some(renderer)) = (pointer, self.renderer.as_ref()) {
            let (w, h) = renderer.size();
            if let Some(target) = pick.target(point, [w as f32, h as f32]) {
                if !matches!(
                    target,
                    Target::Delete | Target::Previous | Target::Next | Target::Symbols
                ) {
                    pick.set_focus(target);
                }
                if click {
                    action = Some(target);
                }
            }
        }
        if pick.focus != old {
            front.cue(MenuSound::Select);
        }
        if let Some(action) = action {
            self.profile_action(front, action);
        }
        true
    }

    fn profile_action(&mut self, front: &mut FrontShell, action: Target) {
        let pick = front.picker.as_mut().unwrap();
        let store = self.profile_store.as_ref().unwrap();
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
                self.profile = profile;
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
                self.session = Default::default();
                front.ia = None;
                if let (Some(voice), Some((gain, _))) = (&mut front.music, &front.music_takes) {
                    voice.set_volume(
                        game::sounds::level_db(*gain * self.profile.settings.music_volume),
                        0.1,
                    );
                } else {
                    front.music_replay = true;
                }
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

    pub(in super::super) fn profile_keyboard(&mut self, event: &winit::event::WindowEvent) -> bool {
        use winit::{
            event::WindowEvent,
            keyboard::{Key, NamedKey},
        };
        let Some(front) = self.front.as_mut() else {
            return false;
        };
        if !front
            .picker
            .as_ref()
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
}
