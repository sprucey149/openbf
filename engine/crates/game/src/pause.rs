//! Pause-menu battle dispatch, shared by the windowed caller and BUGX scenes.
//! Respawn confirmation uses the existing death flow (analysis/re-spawn-flow §7/§6).
//! The ground Game Menu's rows and popups follow both discs' `ifs_pausemenu`
//! (analysis/re-bf2-menus.md "ifs_pausemenu").
use crate::walk::WalkSim;
use core::sim::{buttons, HeldEat, PadState};
use ui::menu::{PausePage, Popup};

/// The Respawn row's text-table key: both discs' `ifs_pausemenu.luac` read
/// `game.pause.suicide` ("Respawn"; its popup is `game.pause.suicide_prompt`).
pub const RESPAWN_LABEL_KEY: &str = "game.pause.suicide";
/// The Game Menu's other text-table keys (BF2 `ifs_pausemenu.luac`). The
/// Quit popup is the host's text, which the PS2 shows in single player
/// (re-bf2-menus.md "ifs_pausemenu").
pub const TITLE_KEY: &str = "game.pause.title";
pub const RESUME_KEY: &str = "common.resume";
pub const OPTIONS_KEY: &str = "ifs.main.options";
pub const RESTART_KEY: &str = "common.restart";
pub const QUIT_KEY: &str = "common.quit";
pub const SUICIDE_PROMPT_KEY: &str = "game.pause.suicide_prompt";
pub const QUIT_PROMPT_KEY: &str = "ifs.pause.warn_host_quit";
pub const RESTART_PROMPT_KEY: &str = "ifs.pause.warn_restart";

/// The Game Menu's words from the feed's BF2 table, English without one.
pub fn texts(feed: &crate::event_feed::EventFeed) -> ui::menu::PauseTexts {
    let d = ui::menu::PauseTexts::default();
    ui::menu::PauseTexts {
        title: feed.screen_text(TITLE_KEY, &d.title),
        resume: feed.screen_text(RESUME_KEY, &d.resume),
        options: feed.screen_text(OPTIONS_KEY, &d.options),
        restart: feed.screen_text(RESTART_KEY, &d.restart),
        quit: feed.screen_text(QUIT_KEY, &d.quit),
        respawn_prompt: feed.screen_text(SUICIDE_PROMPT_KEY, &d.respawn_prompt),
        quit_prompt: feed.screen_text(QUIT_PROMPT_KEY, &d.quit_prompt),
        restart_prompt: feed.screen_text(RESTART_PROMPT_KEY, &d.restart_prompt),
        cheats: d.cheats,
    }
}

#[derive(Default)]
pub struct PauseMenu {
    pub cursor: usize,
    /// The Yes/No popup open, if any.
    pub confirm: Option<Popup>,
    pub class_picker: bool,
    pub class_tab: usize,
    /// The Game Menu page (ground battles; space keeps the flat list).
    pub page: PausePage,
    /// The Game Menu row that opened the page shown, for its Back.
    pub opener: usize,
}

impl PauseMenu {
    /// Closed: the next pause opens on the Game Menu's first row.
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.confirm = None;
        self.class_picker = false;
        self.page = PausePage::Main;
        self.opener = 0;
    }

    pub fn class_picker_model(&self, walk: &WalkSim) -> Option<ui::menu::ClassPickerModel> {
        if !self.class_picker {
            return None;
        }
        walk.class_picker.as_ref().map(|p| p.model(self.class_tab))
    }
}

/// Pad bits that are a standing state, not a press: a resume never eats them
/// (a mouse player's MOUSE_AIM is set every frame).
const RESUME_KEEPS: u32 = buttons::MOUSE_AIM | buttons::FIRST_PERSON | buttons::RESPAWN;

/// Playtest A3: a paused frame marks every button for eating, so whatever is
/// still down when play resumes (the Resume confirm, a Back, the click on
/// Resume) waits for its release before play sees it.
pub fn hold_for_resume(gate: &mut HeldEat) {
    gate.eat(!RESUME_KEEPS);
}

/// The live pad after a resume: the buttons held through it are dropped until
/// released; sticks and mouse look pass.
pub fn resume_pad(gate: &mut HeldEat, pad: PadState) -> PadState {
    PadState {
        buttons: gate.filter(pad.buttons),
        ..pad
    }
}

/// Apply a pause-menu action to the battle/session.
pub fn apply_menu_action(
    walk: &mut WalkSim,
    a: ui::menu::MenuAction,
    paused: &mut bool,
    menu: &mut PauseMenu,
) {
    use ui::menu::MenuAction;
    match a {
        MenuAction::SetClassTab(tab) => {
            if *paused && menu.class_picker && tab < crate::class_picker::TABS.len() {
                menu.class_tab = tab;
                menu.cursor = 0;
            }
        }
        MenuAction::OpenClassPicker => {
            if *paused && crate::class_picker::ClassPicker::available(walk) {
                if let Some(picker) = walk.class_picker.as_mut() {
                    picker.load();
                    menu.class_picker = true;
                    menu.confirm = None;
                    menu.cursor = 0;
                }
            }
        }
        MenuAction::CloseClassPicker => {
            menu.class_picker = false;
            menu.cursor = 0;
        }
        MenuAction::SelectClass(i) => {
            if *paused && menu.class_picker {
                if let Some(mut picker) = walk.class_picker.take() {
                    if picker.select(walk, i) {
                        menu.class_picker = false;
                        menu.cursor = 0;
                        *paused = false;
                    }
                    walk.class_picker = Some(picker);
                }
            }
        }
        MenuAction::Resume => {
            menu.reset();
            *paused = false;
        }
        MenuAction::OpenPage(page) => {
            if *paused {
                menu.opener = menu.cursor;
                menu.page = page;
                menu.cursor = 0;
            }
        }
        MenuAction::ClosePage => {
            menu.page = PausePage::Main;
            menu.cursor = menu.opener;
        }
        MenuAction::RequestRespawn => {
            if *paused && walk.sim.outcome.is_none() && walk.sim.unit(walk.player).is_some() {
                menu.confirm = Some(Popup::Respawn);
                menu.cursor = 0; // No is the safe initial selection.
            }
        }
        MenuAction::RequestQuit => {
            if *paused {
                menu.confirm = Some(Popup::Quit);
                menu.cursor = 0;
            }
        }
        MenuAction::RequestRestart => {
            if *paused {
                menu.confirm = Some(Popup::Restart);
                menu.cursor = 0;
            }
        }
        MenuAction::CancelPopup => {
            menu.confirm = None;
            menu.cursor = 0;
        }
        MenuAction::Respawn => {
            if *paused && menu.confirm == Some(Popup::Respawn) && walk.sim.outcome.is_none() {
                walk.request_respawn();
                *paused = false;
            }
            menu.confirm = None;
            menu.cursor = 0;
        }
        MenuAction::ToggleCheat(i) => {
            let id = walk.sim.cheats.entries()[i].0;
            walk.sim.cheats.toggle(id);
            println!("cheat {id}: {}", walk.sim.cheats.entries()[i].2);
        }
        MenuAction::SetAssist(v) => walk.sim.assist_intensity = v.clamp(0.0, 1.5),
        MenuAction::SetAiCap(c) => walk.set_team_size_percent(c),
        MenuAction::SetReinforcements(c) => walk.reinforcement_pct = c.clamp(10, 500),
        MenuAction::SetMusic(v) => {
            if let Some(m) = walk.music.as_mut() {
                m.volume = v.clamp(crate::music::VOLUME_RANGE.0, crate::music::VOLUME_RANGE.1);
            }
        }
        MenuAction::SetFov(k) => {
            walk.fov_scale = k.clamp(
                crate::walk::FOV_SCALE_RANGE.0,
                crate::walk::FOV_SCALE_RANGE.1,
            )
        }
        // The pad sampler takes the profile's value; the aim assist reads
        // the push before it. Space battles keep 1.0.
        MenuAction::SetPadLookSensitivity(v) => {
            walk.sim.pad_look_sensitivity = walk.pad_look_for_battle(v)
        }
        // Quit: --map launches exit, shell launches return to the shell;
        // Restart reloads the playlist row. Both are the caller's (App state).
        MenuAction::Quit
        | MenuAction::Restart
        | MenuAction::SetMouseSensitivity(_)
        | MenuAction::ToggleInvertY
        | MenuAction::ToggleLowPowerResolution
        | MenuAction::CycleInputIcons(_)
        | MenuAction::ToggleViewpoint => {}
    }
}

/// The rows' share of a paused frame's edges: D-pad Left/Right are the
/// Morse letter and word keys (card CHEAT2), so they never step a row;
/// the left stick and the A and D keys do (`step_left`, `step_right`).
/// A Yes/No popup and the class picker keep the D-pad too (`dpad_steps`).
/// Up/Down move the cursor, as on the PS2.
pub fn rows_input(
    input: ui::menu::MenuInput,
    step_left: bool,
    step_right: bool,
    dpad_steps: bool,
) -> ui::menu::MenuInput {
    ui::menu::MenuInput {
        left: step_left || (dpad_steps && input.left),
        right: step_right || (dpad_steps && input.right),
        ..input
    }
}

/// The pause menu's click for one frame (menuref "In battle" and its sound
/// table): Back resumes with the exit click (from a popup too, playtest A3);
/// a sub-page's Back row exit (the PS2's sub-page Triangle); Cross on No
/// cancel, on any other row accept; a move select; a value step accept (as
/// Game Options). Start's accept on opening is the caller's.
pub fn pause_sound(
    input: ui::menu::MenuInput,
    from: usize,
    to: usize,
    actions: &[ui::menu::MenuAction],
) -> Option<ui::shell_feel::MenuSound> {
    use ui::menu::MenuAction;
    use ui::shell_feel::MenuSound;
    if input.back {
        // A space battle's Back that only cancels the popup keeps its cancel.
        return Some(if actions.contains(&MenuAction::CancelPopup) {
            MenuSound::Cancel
        } else {
            MenuSound::Exit
        });
    }
    if actions.contains(&MenuAction::CancelPopup) {
        return Some(MenuSound::Cancel);
    }
    if actions.contains(&MenuAction::ClosePage) {
        return Some(MenuSound::Exit);
    }
    if !actions.is_empty() {
        return Some(MenuSound::Accept);
    }
    (from != to).then_some(MenuSound::Select)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui::menu::{MenuAction, MenuInput};
    use ui::shell_feel::MenuSound;

    /// A paused D-pad types Morse and never steps a value; the stick or A / D
    /// reach every slider, and the popup keeps the D-pad.
    #[test]
    fn a_paused_step_comes_from_the_stick_or_keys_not_the_morse_keys() {
        let dpad = MenuInput {
            left: true,
            ..Default::default()
        };
        assert!(!rows_input(dpad, false, false, false).left);
        assert!(rows_input(dpad, false, false, true).left);
        let stepped = rows_input(MenuInput::default(), false, true, false);
        assert!(stepped.right && !stepped.left);
        let model = ui::menu::MenuModel {
            music_volume: 0.5,
            cursor: ui::menu::rows(&ui::menu::MenuModel::default())
                .iter()
                .position(|r| *r == ui::menu::Row::Music)
                .unwrap(),
            ..Default::default()
        };
        assert_eq!(
            ui::menu::navigate(&model, stepped).1,
            vec![MenuAction::SetMusic(0.6)]
        );
        assert!(
            ui::menu::navigate(&model, rows_input(dpad, false, false, false))
                .1
                .is_empty()
        );
    }

    /// menuref "In battle": move select, Back exit, the popup's No cancel,
    /// Cross accept; nothing when nothing happens.
    #[test]
    fn the_pause_menu_asks_for_the_ps2_clicks() {
        let back = MenuInput {
            back: true,
            ..Default::default()
        };
        let cross = MenuInput {
            confirm: true,
            ..Default::default()
        };
        let down = MenuInput {
            down: true,
            ..Default::default()
        };
        assert_eq!(
            pause_sound(back, 0, 0, &[MenuAction::Resume]),
            Some(MenuSound::Exit)
        );
        assert_eq!(
            pause_sound(cross, 0, 0, &[MenuAction::CancelPopup]),
            Some(MenuSound::Cancel)
        );
        // A space battle's Back that only cancels the popup.
        assert_eq!(
            pause_sound(back, 0, 0, &[MenuAction::CancelPopup]),
            Some(MenuSound::Cancel)
        );
        assert_eq!(
            pause_sound(cross, 2, 2, &[MenuAction::RequestRespawn]),
            Some(MenuSound::Accept)
        );
        assert_eq!(pause_sound(down, 0, 1, &[]), Some(MenuSound::Select));
        assert_eq!(pause_sound(cross, 3, 3, &[]), None);
    }

    /// Card CHEAT2: typing Morse never steps a slider; Up/Down still move.
    /// Playtest A3: the A / Cross that picked Resume does not jump, the B /
    /// Circle that resumed does not roll; each acts again once released, and
    /// a press begun after the resume, the mouse aim and the stick pass.
    #[test]
    fn a_press_held_through_resume_waits_for_its_release() {
        let mut gate = HeldEat::default();
        let pad = |bits| PadState {
            buttons: bits,
            lx: 9,
            ..Default::default()
        };
        hold_for_resume(&mut gate);
        let held = buttons::JUMP | buttons::ROLL_STANCE | buttons::SPAWN_ACCEPT;
        for _ in 0..3 {
            let out = resume_pad(&mut gate, pad(held | buttons::MOUSE_AIM));
            assert_eq!(out.buttons, buttons::MOUSE_AIM);
            assert_eq!(out.lx, 9);
        }
        // FIRE pressed after the resume, JUMP still down.
        let out = resume_pad(&mut gate, pad(buttons::JUMP | buttons::FIRE));
        assert_eq!(out.buttons, buttons::FIRE);
        assert_eq!(resume_pad(&mut gate, pad(0)).buttons, 0);
        assert!(gate.is_clear());
        assert_eq!(
            resume_pad(&mut gate, pad(buttons::JUMP)).buttons,
            buttons::JUMP
        );
    }

    #[test]
    fn left_and_right_never_reach_the_rows() {
        let model = ui::menu::MenuModel {
            cheats: core::sim::Cheats::default().entries().to_vec(),
            ..Default::default()
        };
        let rows = ui::menu::rows(&model);
        let fov = rows.iter().position(|r| *r == ui::menu::Row::Fov).unwrap();
        let at_fov = ui::menu::MenuModel {
            cursor: fov,
            fov_scale: 1.0,
            ..model.clone()
        };
        for (left, right) in [(true, false), (false, true)] {
            let input = MenuInput {
                left,
                right,
                ..Default::default()
            };
            let (cursor, actions) =
                ui::menu::navigate(&at_fov, rows_input(input, false, false, false));
            assert_eq!((cursor, actions.len()), (fov, 0));
        }
        let up = rows_input(
            MenuInput {
                up: true,
                ..Default::default()
            },
            false,
            false,
            false,
        );
        assert_eq!(ui::menu::navigate(&at_fov, up).0, fov - 1);
        // The Respawn popup still flips No/Yes with them.
        let popup = ui::menu::MenuModel {
            popup: Some(Popup::Respawn),
            ..model
        };
        let right = MenuInput {
            right: true,
            ..Default::default()
        };
        assert_eq!(
            ui::menu::navigate(&popup, rows_input(right, false, false, true)).0,
            1
        );
    }

    /// Playtest A6: a sub-page's Back returns to the row that opened it; a
    /// Quit asks first on No; Resume closes everything, so the next pause
    /// opens on the Game Menu's first row.
    #[test]
    fn the_game_menu_pages_and_popups_close_where_they_opened() {
        let mut walk = crate::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        let (mut paused, mut menu) = (true, PauseMenu::default());
        let mut act = |a, paused: &mut bool, menu: &mut PauseMenu| {
            apply_menu_action(&mut walk, a, paused, menu)
        };
        menu.cursor = 1;
        act(
            MenuAction::OpenPage(PausePage::Options),
            &mut paused,
            &mut menu,
        );
        assert_eq!((menu.page, menu.cursor), (PausePage::Options, 0));
        menu.cursor = 5;
        act(MenuAction::ClosePage, &mut paused, &mut menu);
        assert_eq!((menu.page, menu.cursor), (PausePage::Main, 1));
        menu.cursor = 4;
        act(MenuAction::RequestQuit, &mut paused, &mut menu);
        assert_eq!((menu.confirm, menu.cursor), (Some(Popup::Quit), 0));
        act(MenuAction::CancelPopup, &mut paused, &mut menu);
        assert_eq!(menu.confirm, None);
        act(MenuAction::RequestRestart, &mut paused, &mut menu);
        assert_eq!(menu.confirm, Some(Popup::Restart));
        // A Yes on another popup is not a Respawn's.
        act(MenuAction::Respawn, &mut paused, &mut menu);
        assert!(paused && menu.confirm.is_none());
        act(
            MenuAction::OpenPage(PausePage::Cheats),
            &mut paused,
            &mut menu,
        );
        menu.cursor = 2;
        act(MenuAction::Resume, &mut paused, &mut menu);
        assert!(!paused);
        assert_eq!(
            (menu.page, menu.cursor, menu.confirm),
            (PausePage::Main, 0, None)
        );
        // Unpaused, nothing opens.
        act(MenuAction::RequestQuit, &mut paused, &mut menu);
        act(
            MenuAction::OpenPage(PausePage::Options),
            &mut paused,
            &mut menu,
        );
        assert_eq!((menu.page, menu.confirm), (PausePage::Main, None));
    }

    /// A sub-page's Back row exits as the PS2's sub-page Triangle; No cancels.
    #[test]
    fn a_page_back_exits_and_a_popup_no_cancels() {
        let cross = MenuInput {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(
            pause_sound(cross, 0, 0, &[MenuAction::ClosePage]),
            Some(MenuSound::Exit)
        );
        assert_eq!(
            pause_sound(cross, 0, 0, &[MenuAction::CancelPopup]),
            Some(MenuSound::Cancel)
        );
        assert_eq!(
            pause_sound(cross, 1, 1, &[MenuAction::OpenPage(PausePage::Options)]),
            Some(MenuSound::Accept)
        );
        assert_eq!(
            pause_sound(cross, 1, 1, &[MenuAction::RequestQuit]),
            Some(MenuSound::Accept)
        );
    }

    /// Issue #142: the controller look sensitivity reaches a ground battle's
    /// aim assist, clamped; space battles keep 1.0 at any setting.
    #[test]
    fn the_pad_look_setting_stays_out_of_space() {
        let mut walk = crate::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        let (mut paused, mut menu) = (true, PauseMenu::default());
        let set = |walk: &mut crate::walk::WalkSim, v, paused: &mut bool, menu: &mut PauseMenu| {
            apply_menu_action(walk, MenuAction::SetPadLookSensitivity(v), paused, menu);
            walk.sim.pad_look_sensitivity
        };
        assert_eq!(set(&mut walk, 1.3, &mut paused, &mut menu), 1.3);
        assert_eq!(set(&mut walk, 9.0, &mut paused, &mut menu), 1.5);
        walk.sim.space_assault = Some(core::sim::SpaceAssault::new(180));
        assert_eq!(set(&mut walk, 1.3, &mut paused, &mut menu), 1.0);
        assert_eq!(walk.pad_look_for_battle(0.5), 1.0);
    }
}
