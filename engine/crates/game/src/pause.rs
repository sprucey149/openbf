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

/// The original single-player menu shows Respawn until the round ends.
pub fn respawn_label(walk: &WalkSim, feed: &crate::event_feed::EventFeed) -> Option<String> {
    walk.sim
        .outcome
        .is_none()
        .then(|| feed.screen_text(RESPAWN_LABEL_KEY, "Respawn"))
}

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
    pub animation: ui::pause_draw::Animation,
    /// The Yes/No popup open, if any.
    pub confirm: Option<Popup>,
    pub class_picker: bool,
    pub class_tab: usize,
    pub page: PausePage,
}

impl PauseMenu {
    /// Closed: the next pause opens on the Game Menu's first row.
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.animation = Default::default();
        self.confirm = None;
        self.class_picker = false;
        self.page = PausePage::Main;
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
/// Resume) waits for its release before play sees it. A mission popup's frames
/// mark the same, for the press that closes it.
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
                menu.page = page;
                menu.cursor = 0;
            }
        }
        MenuAction::ClosePage => {
            if let Some((page, row)) = menu.page.parent() {
                // Issue #194: looked up with the split rows on, so Back from the Splitscreen
                // page lands on its own Options row (it is last: no other row moves).
                let model = ui::menu::MenuModel {
                    page: Some(page),
                    split: Some(Default::default()),
                    ..Default::default()
                };
                menu.page = page;
                menu.cursor = ui::menu::rows(&model)
                    .iter()
                    .position(|r| *r == row)
                    .unwrap_or(0);
            }
        }
        MenuAction::RequestRespawn => {
            if *paused && walk.sim.outcome.is_none() {
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
                if walk.sim.unit(walk.player).is_some() {
                    walk.request_respawn();
                }
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
        MenuAction::SetAssist(v) => {
            walk.sim.assist_intensity = crate::profile::clamp_assist_intensity(v)
        }
        MenuAction::SetAiCap(c) => walk.set_team_size_percent(c),
        MenuAction::SetReinforcements(c) => walk.reinforcement_pct = c.clamp(10, 500),
        MenuAction::SetMusic(v) => {
            if let Some(m) = walk.music.as_mut() {
                m.volume = v.clamp(crate::music::VOLUME_RANGE.0, crate::music::VOLUME_RANGE.1);
            }
        }
        MenuAction::SetFov(k) => walk.fov_scale = crate::walk::clamp_fov_scale(k),
        MenuAction::ResetGraphics => walk.fov_scale = crate::profile::Settings::default().fov_scale,
        // The pad sampler takes the profile's value; the aim assist reads
        // the push before it. Space battles keep 1.0.
        MenuAction::SetPadLookSensitivity(v) => {
            walk.sim.pad_look_sensitivity = walk.pad_look_for_battle(v)
        }
        MenuAction::SetCareerAwards(on) => walk.sim.set_career_award_bonuses(on),
        // Quit: --map launches exit, shell launches return to the shell;
        // Restart reloads the playlist row. Both are the caller's (App state).
        MenuAction::Quit
        | MenuAction::Restart
        | MenuAction::Denied
        | MenuAction::SetDifficulty(_)
        | MenuAction::SetEffectsVolume(_)
        | MenuAction::SetDialogueVolume(_)
        | MenuAction::SetFpsCounter(_)
        | MenuAction::SetNetworkMeter(_)
        | MenuAction::EditPlayerName
        | MenuAction::EditBindings(..)
        | MenuAction::SetRumble(_)
        | MenuAction::ToggleRumbleOnFire
        | MenuAction::SetMouseSensitivity(_)
        | MenuAction::ToggleFlightControls
        | MenuAction::ToggleInvertY
        | MenuAction::ToggleTerrainLod
        | MenuAction::ToggleHardShadows
        | MenuAction::ToggleSceneSoftness
        | MenuAction::ToggleLowPowerResolution
        | MenuAction::SetAutoQuality(_)
        | MenuAction::SetWindowMode(_)
        | MenuAction::CycleInputIcons(_)
        | MenuAction::ToggleViewpoint
        | MenuAction::SetSplitTwoPlayer(_)
        | MenuAction::MoveSplitScreen { .. }
        | MenuAction::SetSplitNameTags(_)
        | MenuAction::SetSplitHudSize(_) => {}
    }
}

/// Issue #194 (`SPLITSCREEN.md` 18.1 item 6, B10): [`apply_menu_action`] for the Game Menu
/// opened by local player `opener`, called with that player swapped in
/// (`WalkSim::with_local(opener)`). A player after the first sets their own slot's aim assist,
/// pad look and award bonuses ([`apply_slot_setting`]), never the session's (which are the
/// first player's); everything else is [`apply_menu_action`]. `opener` 0 is exactly
/// [`apply_menu_action`].
pub fn apply_opener_action(
    walk: &mut WalkSim,
    a: ui::menu::MenuAction,
    paused: &mut bool,
    menu: &mut PauseMenu,
    opener: usize,
) {
    if opener > 0 && apply_slot_setting(walk, a) {
        return;
    }
    apply_menu_action(walk, a, paused, menu);
}

/// A per-player setting from the Game Menu on the swapped-in player's own slot
/// (`walk.local_slot`): aim assist and pad look through `Sim::set_slot_assist`, the award
/// bonuses through `Sim::set_slot_career_award_bonuses`, and the slot's installed profile
/// (`walk.netplay.profiles`) kept in step. True when `a` was one of them. The FOV needs nothing
/// here: `WalkSim::fov_scale` is swapped per player (`LocalView`).
pub fn apply_slot_setting(walk: &mut WalkSim, a: ui::menu::MenuAction) -> bool {
    use ui::menu::MenuAction;
    let slot = walk.local_slot;
    let s = usize::from(slot);
    match a {
        MenuAction::SetAssist(v) => {
            let mut p = walk.sim.assist_profile(s);
            p.intensity = crate::profile::clamp_assist_intensity(v);
            walk.sim.set_slot_assist(s, Some(p));
            if let Some(installed) = walk.netplay.profiles.get_mut(&slot) {
                installed.assist.intensity = p.intensity;
            }
        }
        MenuAction::SetPadLookSensitivity(v) => {
            let mut p = walk.sim.assist_profile(s);
            p.pad_look_sensitivity = walk.pad_look_for_battle(v);
            walk.sim.set_slot_assist(s, Some(p));
            if let Some(installed) = walk.netplay.profiles.get_mut(&slot) {
                installed.assist.pad_look_sensitivity = crate::profile::clamp_pad_look(v);
            }
        }
        MenuAction::SetCareerAwards(on) => {
            let awards_off = walk.netplay.awards_off;
            walk.sim.set_slot_career_award_bonuses(s, on && !awards_off);
            if let Some(installed) = walk.netplay.profiles.get_mut(&slot) {
                installed.award_bonuses = on;
            }
        }
        _ => return false,
    }
    true
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

/// One frame of a binding list's pointer and view (the Controls pages, in
/// battle and in the shell): the wheel scrolls the view, a click picks the
/// row under it (returned for the caller to activate) without moving the
/// view, and the view eases on by `dt`. Call it every frame the list is
/// open; `model` is `None` when no native menu is drawn (the view still
/// eases), `size` the window's.
pub fn binding_list_frame(
    model: Option<&ui::menu::MenuModel>,
    animation: &mut ui::pause_draw::Animation,
    wheel: f32,
    click: Option<[f32; 2]>,
    size: Option<[f32; 2]>,
    dt: f32,
) -> Option<usize> {
    let mut picked = None;
    if let (Some(model), Some(size)) = (model, size) {
        if wheel != 0.0 {
            ui::pause_draw::scroll(model, animation, wheel, size);
        }
        if let Some(point) = click {
            if let Some(ui::pause_draw::Target::Row(row)) =
                ui::pause_draw::target(model, animation, point, size)
            {
                ui::pause_draw::pick(animation);
                picked = Some(row);
            }
        }
    }
    animation.advance(dt);
    picked
}

/// Resolve one pause input. A pointer action or an ordinary menu edge wins
/// when it lands in the same frame as a shoulder step.
pub fn navigate(
    model: &mut ui::menu::MenuModel,
    input: ui::menu::MenuInput,
    target: Option<ui::pause_draw::Target>,
    page_right: Option<bool>,
    animation: &ui::pause_draw::Animation,
) -> (usize, Vec<ui::menu::MenuAction>) {
    if let Some(hit) = target.filter(|_| !input.back) {
        return ui::pause_draw::click(model, animation, hit);
    }
    if let Some(right) = page_right {
        if !input.back && !input.confirm && !input.left && !input.right && !input.up && !input.down
        {
            return (model.cursor, ui::menu::page_step(model, right));
        }
    }
    ui::menu::navigate(model, input)
}

/// The pause menu's click for one frame (menuref "In battle" and its sound
/// table): Back resumes with the exit click (from a popup too, playtest A3);
/// a sub-page's Back row exit (the PS2's sub-page Triangle); Cross on No
/// cancel, on any other row accept; a move select; a value step accept (as
/// Game Options). Music adjustments are silent. Start's accept on opening
/// is the caller's.
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
    if actions.contains(&MenuAction::Denied) {
        return Some(MenuSound::Error);
    }
    if !actions.is_empty() {
        return actions
            .iter()
            .any(|a| !matches!(a, MenuAction::SetMusic(_)))
            .then_some(MenuSound::Accept);
    }
    (from != to).then_some(MenuSound::Select)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui::shell_feel::MenuSound;
    use ui::{
        menu::{rows, MenuAction, MenuInput, MenuModel, Row, SettingsPage},
        pause_draw::Target,
    };

    #[test]
    fn respawn_stays_in_the_active_round_menu_while_the_player_is_dead() {
        let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
        let feed = crate::event_feed::EventFeed::new(
            crate::event_feed::Texts::builtin(),
            ["rep", "cis"],
            &[],
        );
        let (mut paused, mut menu) = (true, PauseMenu::default());
        assert!(walk.sim.unit(walk.player).is_some());
        apply_menu_action(
            &mut walk,
            MenuAction::RequestRespawn,
            &mut paused,
            &mut menu,
        );
        assert_eq!(menu.confirm, Some(Popup::Respawn));
        apply_menu_action(&mut walk, MenuAction::Respawn, &mut paused, &mut menu);
        walk.step_tick(&[]);
        assert!(walk.sim.unit(walk.player).is_none());
        assert_eq!(respawn_label(&walk, &feed).as_deref(), Some("Respawn"));
        let death_tick = walk.sim.units[walk.player.index as usize].respawn_at;
        paused = true;
        apply_menu_action(
            &mut walk,
            MenuAction::RequestRespawn,
            &mut paused,
            &mut menu,
        );
        assert_eq!(menu.confirm, Some(Popup::Respawn));
        apply_menu_action(&mut walk, MenuAction::Respawn, &mut paused, &mut menu);
        assert!(!paused);
        walk.step_tick(&[]);
        assert_eq!(
            walk.sim.units[walk.player.index as usize].respawn_at,
            death_tick
        );
        walk.sim.outcome = Some(0);
        assert_eq!(respawn_label(&walk, &feed), None);
    }

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

    /// Issue #194 (B7): Back from the Splitscreen page lands on its own Options row (the
    /// last), and Back from the other pages still on theirs.
    #[test]
    fn back_from_the_splitscreen_page_returns_to_its_options_row() {
        let mut walk = crate::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        let (mut paused, mut menu) = (true, PauseMenu::default());
        let split = PausePage::Settings(SettingsPage::Splitscreen);
        apply_menu_action(
            &mut walk,
            MenuAction::OpenPage(split),
            &mut paused,
            &mut menu,
        );
        menu.cursor = 3;
        apply_menu_action(&mut walk, MenuAction::ClosePage, &mut paused, &mut menu);
        assert_eq!((menu.page, menu.cursor), (PausePage::Options, 5));
        let options = MenuModel {
            page: Some(PausePage::Options),
            split: Some(Default::default()),
            ..Default::default()
        };
        assert_eq!(
            rows(&options)[menu.cursor],
            Row::Settings(SettingsPage::Splitscreen)
        );
        for (page, at) in [(SettingsPage::Game, 1), (SettingsPage::Controls, 4)] {
            menu.page = PausePage::Settings(page);
            apply_menu_action(&mut walk, MenuAction::ClosePage, &mut paused, &mut menu);
            assert_eq!((menu.page, menu.cursor), (PausePage::Options, at));
        }
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

    fn at(model: &mut MenuModel, row: Row) -> usize {
        model.cursor = rows(model).iter().position(|r| *r == row).unwrap();
        model.cursor
    }

    #[test]
    fn pause_shoulder_steps_share_bounds_and_yield_to_back_and_clicks() {
        let mut m = MenuModel {
            page: Some(PausePage::Settings(SettingsPage::KeyboardMouse)),
            mouse_sensitivity: 1.0,
            ..Default::default()
        };
        let row = at(&mut m, Row::MouseSensitivity);
        let idle = MenuInput::default();
        // A shoulder takes ten 5 % steps (100 % -> 150 % / 50 %) and stops
        // at the slider's bounds (5 % to 200 %, `ui::menu::MOUSE_SENSITIVITY_RANGE`).
        assert_eq!(
            navigate(&mut m, idle, None, Some(true), &Default::default()).1,
            [MenuAction::SetMouseSensitivity(1.5)]
        );
        assert_eq!(
            navigate(&mut m, idle, None, Some(false), &Default::default()).1,
            [MenuAction::SetMouseSensitivity(0.5)]
        );
        m.mouse_sensitivity = 1.8;
        assert_eq!(
            navigate(&mut m, idle, None, Some(true), &Default::default()).1,
            [MenuAction::SetMouseSensitivity(2.0)]
        );
        m.mouse_sensitivity = 0.3;
        assert_eq!(
            navigate(&mut m, idle, None, Some(false), &Default::default()).1,
            [MenuAction::SetMouseSensitivity(0.05)]
        );
        m.mouse_sensitivity = 1.0;
        let click = Target::Step { row, right: true };
        assert_eq!(
            navigate(&mut m, idle, Some(click), Some(true), &Default::default()).1,
            [MenuAction::SetMouseSensitivity(1.05)]
        );
        let back = MenuInput { back: true, ..idle };
        assert_eq!(
            navigate(&mut m, back, Some(click), Some(false), &Default::default()).1,
            [MenuAction::ClosePage]
        );

        m.page = Some(PausePage::Settings(SettingsPage::Graphics));
        m.base_fov_deg = 60.0;
        m.fov_scale = 100.0 / 60.0;
        let fov = at(&mut m, Row::Fov);
        let (_, actions) = navigate(&mut m, idle, None, Some(true), &Default::default());
        assert_eq!(actions, [MenuAction::Denied]);
        assert_eq!(
            pause_sound(idle, fov, fov, &actions),
            Some(MenuSound::Error)
        );
        assert_eq!(
            navigate(&mut m, idle, None, Some(false), &Default::default()).1,
            [MenuAction::SetFov(1.2)]
        );
        at(&mut m, Row::ResetGraphics);
        assert!(
            navigate(&mut m, idle, None, Some(true), &Default::default())
                .1
                .is_empty()
        );

        m.page = Some(PausePage::Settings(SettingsPage::Gamepad));
        m.assist_intensity = 0.5;
        at(&mut m, Row::Assist);
        assert_eq!(
            navigate(&mut m, idle, None, Some(true), &Default::default()).1,
            [MenuAction::SetAssist(1.0)]
        );
        assert_eq!(
            navigate(&mut m, idle, None, Some(false), &Default::default()).1,
            [MenuAction::SetAssist(0.0)]
        );
    }

    #[test]
    fn pause_cues_match_pointer_and_pad_actions() {
        let idle = MenuInput::default();
        let confirm = MenuInput {
            confirm: true,
            ..idle
        };
        let right = MenuInput {
            right: true,
            ..idle
        };
        for (mut m, row, input, cue) in [
            (
                MenuModel {
                    page: Some(PausePage::Main),
                    ..Default::default()
                },
                Row::Options,
                confirm,
                Some(MenuSound::Accept),
            ),
            (
                MenuModel {
                    popup: Some(Popup::Quit),
                    ..Default::default()
                },
                Row::No,
                confirm,
                Some(MenuSound::Cancel),
            ),
            (
                MenuModel {
                    popup: Some(Popup::Quit),
                    ..Default::default()
                },
                Row::Yes,
                confirm,
                Some(MenuSound::Accept),
            ),
            (
                MenuModel {
                    page: Some(PausePage::Options),
                    ..Default::default()
                },
                Row::PageBack,
                confirm,
                Some(MenuSound::Exit),
            ),
            (
                MenuModel {
                    page: Some(PausePage::Settings(SettingsPage::Audio)),
                    music_volume: 0.5,
                    ..Default::default()
                },
                Row::Music,
                right,
                None,
            ),
            (
                MenuModel {
                    page: Some(PausePage::Settings(SettingsPage::Graphics)),
                    ..Default::default()
                },
                Row::GraphicsQuality,
                confirm,
                Some(MenuSound::Error),
            ),
        ] {
            let from = at(&mut m, row);
            let target = if input.right {
                Target::Step {
                    row: from,
                    right: true,
                }
            } else {
                Target::Row(from)
            };
            let (to, pad) = navigate(&mut m, input, None, None, &Default::default());
            let (hit, mouse) = navigate(&mut m, idle, Some(target), None, &Default::default());
            assert_eq!((hit, &mouse), (to, &pad), "{row:?}");
            assert_eq!(pause_sound(input, from, to, &pad), cue, "{row:?}");
            assert_eq!(pause_sound(idle, from, hit, &mouse), cue, "{row:?}");
        }
    }

    fn binding_list(actions: usize) -> MenuModel {
        let mut labels = vec![
            ("Back".to_string(), String::new()),
            ("Restore defaults".to_string(), String::new()),
        ];
        labels.extend((0..actions).map(|i| (format!("Action {i}"), "Key".to_string())));
        MenuModel {
            bindings: Some(ui::menu::BindingRows {
                title: "Infantry Controls",
                labels,
                capture: false,
                message: None,
            }),
            ..Default::default()
        }
    }

    const SIZE: [f32; 2] = [1920.0, 1080.0];

    /// `n` frames of the battle loop: the list's frame, then its drawing
    /// (which lays the list out, keeping the selection in view).
    fn frames(m: &MenuModel, anim: &mut ui::pause_draw::Animation, n: usize) {
        for _ in 0..n {
            binding_list_frame(Some(m), anim, 0.0, None, Some(SIZE), 1.0 / 60.0);
            let _ = ui::pause_draw::row_in_view(m, anim, m.cursor, SIZE);
        }
    }

    /// Review blocker: in battle the binding list's view never eased, so
    /// keys walked the selection out of the window and the wheel did nothing.
    #[test]
    fn the_binding_list_keeps_its_selection_in_view_and_follows_the_wheel() {
        use ui::pause_draw::{row_in_view, Animation};
        let mut m = binding_list(14);
        let mut anim = Animation::default();
        let len = rows(&m).len();
        for cursor in 0..len {
            m.cursor = cursor;
            frames(&m, &mut anim, 30);
            assert!(
                row_in_view(&m, &anim, cursor, SIZE),
                "row {cursor} out of the window at offset {}",
                anim.scroll_offset()
            );
        }
        let bottom = anim.scroll_offset();
        assert!(bottom > 0.0, "a long list scrolled: {bottom}");
        binding_list_frame(Some(&m), &mut anim, 3.0, None, Some(SIZE), 1.0 / 60.0);
        frames(&m, &mut anim, 30);
        assert!(
            (anim.scroll_offset() - (bottom - 3.0).max(0.0)).abs() < 1.0e-3,
            "the wheel moved the view: {} from {bottom}",
            anim.scroll_offset()
        );
        assert_eq!(m.cursor, len - 1, "the wheel never moves the selection");
    }

    /// Review: a click on the last wholly shown row selected it and then
    /// scrolled the list a row to keep a margin below it.
    #[test]
    fn a_click_on_the_last_shown_binding_row_leaves_the_list_where_it_is() {
        use ui::pause_draw::{row_in_view, target, Animation};
        let m = binding_list(14);
        let mut anim = Animation::default();
        frames(&m, &mut anim, 5);
        let len = rows(&m).len();
        let last = (0..len)
            .rev()
            .find(|&r| row_in_view(&m, &anim, r, SIZE))
            .expect("a shown row");
        assert!(last + 1 < len, "the list is longer than its window");
        let point = (0..SIZE[1] as usize)
            .map(|y| [SIZE[0] * 0.5, y as f32])
            .find(|&p| target(&m, &anim, p, SIZE) == Some(Target::Row(last)))
            .expect("a point on the row");
        let before = anim.scroll_offset();
        let picked = binding_list_frame(
            Some(&m),
            &mut anim,
            0.0,
            Some(point),
            Some(SIZE),
            1.0 / 60.0,
        );
        assert_eq!(picked, Some(last));
        let mut m = m;
        m.cursor = last; // the caller activates it
        frames(&m, &mut anim, 30);
        assert_eq!(anim.scroll_offset(), before, "a click never moves the list");
    }

    /// Review: a page whose row count changes (a toggle that hides rows)
    /// counted as a new list and jumped back to its selection.
    #[test]
    fn a_row_count_change_on_the_same_page_keeps_the_view() {
        use ui::pause_draw::Animation;
        let m = binding_list(14);
        let mut anim = Animation::default();
        frames(&m, &mut anim, 1);
        binding_list_frame(Some(&m), &mut anim, -4.0, None, Some(SIZE), 1.0 / 60.0);
        frames(&m, &mut anim, 30);
        let before = anim.scroll_offset();
        assert!(before > 0.0, "scrolled: {before}");
        let shorter = binding_list(13);
        frames(&shorter, &mut anim, 30);
        let after = anim.scroll_offset();
        assert!(
            after > 0.0 && (before - after) <= 1.0 + 1.0e-3,
            "kept, clamped at most a row: {before} -> {after}"
        );
    }

    /// Issue #194 (B10): P2's assist from their own Game Menu sets slot 1's profile and leaves
    /// the session's `assist_intensity` (the first player's); opener 0 is today's action.
    #[test]
    fn p2s_assist_is_their_slots_and_p1s_the_sessions() {
        let mut walk = crate::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let (mut paused, mut menu) = (true, PauseMenu::default());
        let session = walk.sim.assist_intensity;
        walk.with_local(1, |w| {
            apply_opener_action(w, MenuAction::SetAssist(0.25), &mut paused, &mut menu, 1)
        });
        assert_eq!(walk.sim.assist_intensity, session);
        assert_eq!(walk.sim.assist_profile(1).intensity, 0.25);
        assert_eq!(walk.sim.assist_profile(0).intensity, session);
        apply_opener_action(
            &mut walk,
            MenuAction::SetAssist(0.75),
            &mut paused,
            &mut menu,
            0,
        );
        assert_eq!(walk.sim.assist_intensity, 0.75);
        assert_eq!(walk.sim.assist_profile(1).intensity, 0.25);
        // Anything else is the plain action, the opener's own player swapped in.
        walk.with_local(1, |w| {
            apply_opener_action(w, MenuAction::SetFov(1.2), &mut paused, &mut menu, 1)
        });
        assert_eq!(walk.with_local(1, |w| w.fov_scale), 1.2);
        assert_ne!(walk.fov_scale, 1.2);
    }
}
