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
    /// The clicks asked for since the last frame took them, with the select
    /// click's 0.1 s rate limit on the pause menu's own clock (the shell's
    /// `ShellMenus::cues` is the same kit).
    pub cues: ui::shell_feel::Cues,
}

impl PauseMenu {
    /// Closed: the next pause opens on the Game Menu's first row.
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.confirm = None;
        self.class_picker = false;
        self.page = PausePage::Main;
        self.opener = 0;
        self.cues = Default::default();
    }

    /// Ask for a menu click (rate limited on the pause menu's clock).
    pub fn cue(&mut self, s: ui::shell_feel::MenuSound) {
        self.cues.play(s);
    }

    /// The clicks asked for since the last frame, oldest first; `dt` seconds
    /// advances the select click's rate limit *after* the take, so a cue queued
    /// this frame counts from when it starts (the PS2 refuses a start inside
    /// its playinterval).
    pub fn take_cues(&mut self, dt: f32) -> Vec<ui::shell_feel::MenuSound> {
        self.cues.take_then_tick(dt)
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

/// The frame's row edges from `kbm::menu_edges`' bits and `dt`: the D-pad /
/// arrows as [`MenuInput`], with the value-step bits from the left stick and
/// the A / D keys, and L1 / R1 (the shell's paging arms) held through
/// [`ui::shell_feel::HoldRepeat`] so a page can repeat. Returns the row input
/// and the page direction: +[`ui::menu::PAGE_STEP`] on R1,
/// -[`ui::menu::PAGE_STEP`] on L1, 0 with neither. `prev_menu_buttons` is the
/// last frame's held bits, for the stick latch; `dpad_steps`: the popup and
/// the picker keep their D-pad.
pub fn row_edges(
    edges: u32,
    dt: f32,
    prev_menu_buttons: u32,
    dpad_steps: bool,
    repeat: &mut ui::shell_feel::HoldRepeat,
) -> (ui::menu::MenuInput, i32) {
    const ADJUST: u32 = crate::kbm::MENU_ADJUST_LEFT | crate::kbm::MENU_ADJUST_RIGHT;
    // One repeat step per frame on every held arm (the stick's A / D steps and
    // the L1 / R1 pages), so a held arm's delay runs from its press: two calls
    // with arguments that differ reset the timer, and the page never repeats.
    const ARMS: u32 = ADJUST | crate::kbm::MENU_L1 | crate::kbm::MENU_R1;
    let arms = edges | repeat.step(prev_menu_buttons & ARMS, dt);
    let adjust = arms & ADJUST;
    // A held page repeats beside the stick's steps.
    const PAGES: u32 = crate::kbm::MENU_L1 | crate::kbm::MENU_R1;
    let pages = arms & PAGES;
    let dir = if pages & crate::kbm::MENU_R1 != 0 {
        ui::menu::PAGE_STEP
    } else if pages & crate::kbm::MENU_L1 != 0 {
        -ui::menu::PAGE_STEP
    } else {
        0
    };
    (
        rows_input(
            ui::menu::MenuInput {
                up: edges & 1 != 0,
                down: edges & 2 != 0,
                left: edges & 4 != 0 && edges & ADJUST == 0,
                right: edges & 8 != 0 && edges & ADJUST == 0,
                confirm: edges & 16 != 0,
                back: edges & 32 != 0,
            },
            adjust & crate::kbm::MENU_ADJUST_LEFT != 0,
            adjust & crate::kbm::MENU_ADJUST_RIGHT != 0,
            dpad_steps,
        ),
        dir,
    )
}

/// The page step on the focused row: L1 / R1 (a `dir` of ±[`ui::menu::PAGE_STEP`])
/// apply a value row's page, as the shell's Session Options' x10 steps. A
/// locked row takes no page; rows that take no step stay as their Left / Right
/// presses left them (`navigate` already ran).
pub fn apply_page(model: &ui::menu::MenuModel, dir: i32, actions: &mut Vec<ui::menu::MenuAction>) {
    if dir == 0 {
        return;
    }
    let row = ui::menu::rows(model)[model.cursor];
    if ui::menu::row_locked(model, row).is_some() {
        return;
    }
    if let Some(a) = ui::menu::page_step_action(model, row, dir) {
        actions.push(a);
    }
}

/// A frame's page step on the model: the new model (the page applied to the
/// cursor `navigate` left) and the actions it asks for. The windowed loop
/// applies them through `apply_pause_action`, which reads walk values.
pub fn page_after(
    model: &ui::menu::MenuModel,
    dir: i32,
    cursor: usize,
) -> (ui::menu::MenuModel, Vec<ui::menu::MenuAction>) {
    let mut m = model.clone();
    m.cursor = cursor;
    let mut actions = Vec::new();
    apply_page(&m, dir, &mut actions);
    for a in &actions {
        // The actions carry their own clamped values.
        match *a {
            ui::menu::MenuAction::SetAssist(v) => m.assist_intensity = v,
            ui::menu::MenuAction::SetAiCap(v) => m.team_size_pct = v,
            ui::menu::MenuAction::SetReinforcements(v) => m.reinforcements_pct = v,
            ui::menu::MenuAction::SetFov(v) => m.fov_scale = v,
            ui::menu::MenuAction::SetMusic(v) => m.music_volume = v,
            ui::menu::MenuAction::SetMouseSensitivity(v) => m.mouse_sensitivity = v,
            ui::menu::MenuAction::SetPadLookSensitivity(v) => m.pad_look_sensitivity = v,
            _ => {}
        }
    }
    (m, actions)
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

/// The full click for a frame of the rows: `pause_sound`'s, with the page
/// step's sound and the refusals'. A page that moved a row accepts, as its
/// Left / Right press does; the music's page stays quiet (its audition is
/// the voice, not a click). A confirm on a locked row - one shown greyed with
/// its reason - plays the error click (menuref §4.1: a refused action), as
/// does Cross on a row that takes nothing (the PS2's pause menu has no such
/// row; `[ours]`'s plain-text rows), and a page step on a row that takes no
/// step (a popup's No / Yes, a button row).
pub fn frame_sound(
    model: &ui::menu::MenuModel,
    input: ui::menu::MenuInput,
    dir: i32,
    to: usize,
    actions: &[ui::menu::MenuAction],
) -> Option<ui::shell_feel::MenuSound> {
    use ui::menu::MenuAction;
    use ui::shell_feel::MenuSound;
    let rows = ui::menu::rows(model);
    let row = rows[model.cursor.min(rows.len().saturating_sub(1))];
    if dir != 0 {
        // A page on a greyed row, or with no value row under the cursor (a
        // popup's No / Yes), is refused.
        if ui::menu::row_locked(model, row).is_some() {
            return Some(MenuSound::Error);
        }
        // A page that moved a value row accepts; the music's step plays nothing
        // (its audition is the voice, not a click).
        let moved = ui::menu::page_step_action(model, row, dir)
            .is_some_and(|a| !matches!(a, MenuAction::SetMusic(_)));
        if input.confirm && actions.is_empty() && ui::menu::row_locked(model, row).is_none() {
            return Some(MenuSound::Error);
        }
        if moved {
            // A page step is a press: accept as Game Options rows do.
            return Some(MenuSound::Accept);
        }
        if ui::menu::page_step_action(model, row, dir).is_none() {
            return Some(MenuSound::Error);
        }
    } else if input.confirm && actions.is_empty() && ui::menu::row_locked(model, row).is_none() {
        return Some(MenuSound::Error);
    }
    pause_sound(input, model.cursor, to, actions)
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

    /// Phase 2's cue set: L1 / R1 page a value row by ten steps and accept as
    /// its press does; the music's page is quiet (its audition is the voice);
    /// a page on a greyed row plays the error click, as a refused action; a
    /// confirm on a row that takes nothing is refused too. A held page repeats
    /// after the shell's delay, every 5 updates.
    #[test]
    fn l1_r1_page_a_row_and_sound_as_its_press() {
        let at = |row: ui::menu::Row| {
            ui::menu::rows(&ui::menu::MenuModel::default())
                .iter()
                .position(|r| *r == row)
                .unwrap()
        };
        let cross = MenuInput {
            confirm: true,
            ..Default::default()
        };
        // A page moves the row and accepts.
        let fov = ui::menu::MenuModel {
            cursor: at(ui::menu::Row::Fov),
            fov_scale: 1.0,
            ..Default::default()
        };
        let (after, acts) = page_after(&fov, ui::menu::PAGE_STEP, fov.cursor);
        assert_eq!(acts, vec![MenuAction::SetFov(1.5)]);
        assert_eq!(after.fov_scale, 1.5);
        assert_eq!(
            frame_sound(&fov, MenuInput::default(), 10, fov.cursor, &[]),
            Some(MenuSound::Accept)
        );
        // The music's page moves the voice and stays quiet (menuref §4.1).
        let music = ui::menu::MenuModel {
            cursor: at(ui::menu::Row::Music),
            music_volume: 0.5,
            ..Default::default()
        };
        let (_, acts) = page_after(&music, -ui::menu::PAGE_STEP, music.cursor);
        assert_eq!(acts, vec![MenuAction::SetMusic(0.0)]);
        assert_eq!(
            frame_sound(&music, MenuInput::default(), -10, music.cursor, &[]),
            None
        );
        // At the top, a page up clamps to 1.0 and stays quiet.
        let music = ui::menu::MenuModel {
            cursor: at(ui::menu::Row::Music),
            music_volume: 1.0,
            ..Default::default()
        };
        let (_, acts) = page_after(&music, ui::menu::PAGE_STEP, music.cursor);
        assert_eq!(acts, vec![MenuAction::SetMusic(1.0)]);
        assert_eq!(
            frame_sound(&music, MenuInput::default(), 10, music.cursor, &[]),
            None
        );
        // A page on a greyed row is refused: the error click (menuref §4.1).
        let locked = ui::menu::MenuModel {
            cursor: at(ui::menu::Row::AiCap),
            team_size_off: true,
            ..Default::default()
        };
        let (_, acts) = page_after(&locked, ui::menu::PAGE_STEP, locked.cursor);
        assert!(acts.is_empty());
        assert_eq!(
            frame_sound(&locked, MenuInput::default(), 10, locked.cursor, &[]),
            Some(MenuSound::Error)
        );
        // A confirm on a row that takes nothing is refused too.
        let flat = ui::menu::MenuModel {
            cursor: at(ui::menu::Row::Quit),
            ..Default::default()
        };
        assert_eq!(
            frame_sound(&flat, cross, 0, flat.cursor, &[]),
            Some(MenuSound::Error)
        );
        // A page step with no row under it (a popup's No/Yes rows) takes the
        // refusal's error click.
        let popup = ui::menu::MenuModel {
            popup: Some(Popup::Respawn),
            cursor: 0,
            ..Default::default()
        };
        assert_eq!(
            frame_sound(&popup, MenuInput::default(), 10, 0, &[]),
            Some(MenuSound::Error)
        );
    }

    /// A held L1 / R1 presses once on its edge, then repeats after the shell's
    /// delay every 5 updates; a press is not a Morse key, so the rows' Left /
    /// Right keep the adjust bits out.
    #[test]
    fn a_held_page_repeats_after_the_delay() {
        let mut repeat = ui::shell_feel::HoldRepeat::default();
        let r1 = crate::kbm::MENU_R1;
        // The edge frame: one press, no repeat yet.
        let (input, dir) = row_edges(r1, 1.0 / 60.0, r1, false, &mut repeat);
        assert_eq!(dir, ui::menu::PAGE_STEP);
        assert!(!input.right && !input.left);
        // Held: the first repeat at 0.62 s, then every 5 updates (the shell's
        // steps run at 30 Hz). The press count is measured after the delay.
        let mut presses = 0;
        let mut t = 0.0;
        while t < 1.0 {
            t += 1.0 / 30.0;
            let (_, dir) = row_edges(0, 1.0 / 30.0, r1, false, &mut repeat);
            presses += (dir != 0) as usize;
        }
        // Past the delay: repeats land every 5 updates (REPEAT_EVERY), so a
        // held second yields 3 presses at 0.62 / 0.79 / 0.96 s, never a flood.
        assert!((3..=4).contains(&presses), "presses: {presses}");
        // L1 pages back.
        let mut repeat = ui::shell_feel::HoldRepeat::default();
        let (_, dir) = row_edges(
            crate::kbm::MENU_L1,
            1.0 / 60.0,
            crate::kbm::MENU_L1,
            false,
            &mut repeat,
        );
        assert_eq!(dir, -ui::menu::PAGE_STEP);
    }

    /// The menu's own click queue rate-limits a select to one per 0.1 s (the
    /// PS2's playinterval), and takes the clicks oldest first.
    #[test]
    fn the_select_click_is_rate_limited() {
        let mut menu = PauseMenu::default();
        menu.cue(MenuSound::Select);
        assert_eq!(menu.take_cues(1.0 / 60.0), vec![MenuSound::Select]);
        // Inside the interval: dropped, as the PS2 refuses a start.
        menu.cue(MenuSound::Select);
        assert!(menu.take_cues(1.0 / 60.0).is_empty());
        // Advance the clock past the first select's ok time (clock is now 3/60
        // = 0.05, select_ok = 1/60 + 0.1 = 0.117): a take of 0.1 s moves the
        // clock to 0.15 > select_ok, so the next select is allowed. An accept
        // is never rate limited, so Cancel rides along.
        menu.take_cues(0.1);
        menu.cue(MenuSound::Select);
        menu.cue(MenuSound::Cancel);
        let got = menu.take_cues(0.1);
        assert_eq!(got, vec![MenuSound::Select, MenuSound::Cancel]);
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
