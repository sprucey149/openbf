//! Regression tests for the shell front screens, extracted from the facade.
//! Logical test names and selectors are unchanged.

use super::*;

/// Tests: a mono take of `secs` seconds that no test opens (28 frames
/// from each 16-byte block at 28 Hz: a block a second).
pub(in super::super) fn test_take(secs: u32) -> audio::StreamTake {
    audio::StreamTake {
        path: std::path::PathBuf::new(),
        offset: 0,
        size: 16 * secs as usize,
        format: 0,
        channels: 1,
        substreams: 1,
        interleave: 0,
        frequency: 28,
        block_size: 16 * 1024,
    }
}

/// A settings change made in a vehicle (the real `apply_shell_setting`
/// path, not `Kbm::follow` alone) keeps the vehicle's table and look
/// factor, and takes the new sensitivity.
#[test]
fn a_settings_change_in_a_vehicle_keeps_the_vehicle_table() {
    use game::kbm::Bindings;
    use std::path::PathBuf;
    let dir = std::env::temp_dir().join(format!("openbf-vehicle-setting-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).unwrap();
    let store = game::profile::Store::new(dir.clone());
    let mut app = App::new(
        PathBuf::new(),
        PathBuf::new(),
        super::super::tests::setup(None),
        None,
        None,
        None,
        Default::default(),
    );
    app.profile = store
        .create("Tester", core::skill::Difficulty::Normal)
        .unwrap();
    app.profile_store = Some(store);
    app.binding_context = Some(ControlContext::Vehicle);
    app.refresh_context_bindings();
    assert_eq!(app.kbm.bindings, Bindings::pc_default_v0());
    let mut front = FrontShell::new(&dir);
    app.apply_shell_setting(&mut front, ui::menu::MenuAction::SetMouseSensitivity(0.5));
    assert_eq!(app.kbm.bindings, Bindings::pc_default_v0());
    assert_eq!(app.kbm.look_scale, 1.0);
    assert_eq!(app.kbm.sensitivity, 0.5);
    app.binding_context = Some(ControlContext::Infantry);
    app.refresh_context_bindings();
    assert_eq!(app.kbm.bindings, Bindings::pc_default());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Issue #128: the shell music never ends while the menus are up. Over
/// twelve minutes of shell frames a take always follows the last after
/// the shell's gap, also while the frames stall (a hidden window: each
/// frame's step clamped to 0.1 s while two real seconds pass), since the
/// take itself plays on in real time.
#[test]
fn the_shell_music_moves_on_take_after_take_for_twelve_minutes() {
    let mut front = FrontShell::new(std::path::Path::new("/nonexistent-openbf-assets"));
    front.set_music_takes(vec![test_take(95), test_take(140), test_take(60)]);
    let secs = front.music_secs();
    assert_eq!(secs, [95.0, 140.0, 60.0]);
    // (real seconds, the silence so far, the longest, takes started)
    let mut run = (0.0f32, 0.0f32, 0.0f32, 0u32);
    let frames = |front: &mut FrontShell, run: &mut (f32, f32, f32, u32), dt, wall, until| {
        while run.0 < until {
            if front.music_due(dt, wall).is_some() {
                run.3 += 1;
            }
            run.0 += wall;
            if front.music_playing().is_some() {
                run.1 = 0.0;
            } else {
                run.1 += wall;
                run.2 = run.2.fmax(run.1);
            }
        }
    };
    frames(&mut front, &mut run, 1.0 / 30.0, 1.0 / 30.0, 360.0);
    let steady = run.3;
    frames(&mut front, &mut run, 0.1, 2.0, 720.0);
    let (starts, longest) = (run.3, run.2);
    assert!(steady >= 3, "{steady} takes in the first six minutes");
    assert!(
        starts - steady >= 2,
        "{} takes while the frames stalled",
        starts - steady
    );
    assert!(
        longest <= ui::shell_feel::MUSIC_GAP + 2.0,
        "{longest} s without music"
    );
}

/// Issue #128: music raised from off asks for the take of the moment
/// again; with no takes loaded the ask waits for one instead of being
/// spent on nothing.
#[test]
fn a_music_replay_waits_for_a_take() {
    let mut front = FrontShell::new(std::path::Path::new("/nonexistent-openbf-assets"));
    front.music_takes = None;
    front.music_replay = true;
    assert_eq!(front.music_due(0.1, 0.1), None);
    assert!(front.music_replay, "nothing to play: the replay waits");
    front.set_music_takes(vec![test_take(95)]);
    assert_eq!(front.music_due(0.1, 0.1), Some(0), "the first take");
    assert!(!front.music_replay, "the take started");
    front.music_replay = true;
    assert_eq!(
        front.music_due(0.1, 0.1),
        Some(0),
        "the take of the moment again"
    );
    assert!(!front.music_replay);
    assert_eq!(front.music_due(0.1, 0.1), None, "once");
}

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

/// re-bf2-menus â”¬Âº4.1: a settings move clicks; a value step plays accept
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

/// Issue #172: F11 is the fullscreen key; the editor keeps the row as it was.
#[test]
fn the_binding_editor_refuses_the_fullscreen_key() {
    let mut c = controls();
    let f11 = game::kbm::Trigger::Key(winit::keyboard::KeyCode::F11);
    assert_eq!(c.assign(Binding::Keyboard(f11)), MenuSound::Error);
    assert!(c.capture.is_some(), "still waiting for a key");
    assert_eq!(
        c.message.as_deref(),
        Some(
            format!(
                "{}; choose another key",
                game::kbm::f11_note(game::kbm::MAC)
            )
            .as_str()
        )
    );
    assert_eq!(c.missing(), None);
}

/// Issue #172: a saved layout that bound F11 opens with that action
/// unassigned and says so; the page asks for a key before it saves.
#[test]
fn a_saved_f11_binding_opens_unassigned() {
    let mut settings = game::profile::Settings::default();
    let mut items: Vec<_> = game::control_layout::keyboard_defaults(
        ControlContext::Infantry,
        &settings.keyboard_bindings,
    )
    .into_iter()
    .map(Option::unwrap)
    .collect();
    let row = items.len() - 1;
    items[row] = game::kbm::FULLSCREEN_KEY;
    assert!(game::control_layout::keyboard(ControlContext::Infantry, &items).is_some());
    settings
        .keyboard_layouts
        .set(ControlContext::Infantry, items);
    let c = Controls::new(
        SettingsPage::KeyboardMouse,
        ControlContext::Infantry,
        &settings,
    );
    assert_eq!(c.missing(), Some(row));
    let name = game::control_layout::actions(ControlContext::Infantry, true)[row].name;
    assert_eq!(
        c.message.as_deref(),
        Some(
            format!(
                "{}; {name} is unassigned",
                game::kbm::f11_note(game::kbm::MAC)
            )
            .as_str()
        )
    );
    assert_eq!(settings.free_fullscreen_key(), [ControlContext::Infantry]);
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
        netplay_rows: false,
        opening_humans: [0; 2],
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
