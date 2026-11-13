//! The ground Game Menu (playtest A6, issue #128): both discs' row order,
use super::*;

fn ground() -> MenuModel {
    MenuModel {
        page: Some(PausePage::Main),
        class_picker_available: true,
        cheats: vec![
            ("invincible", "Invincibility", false),
            ("ammo", "Infinite ammo", true),
        ],
        respawn_label: Some("Respawn".into()),
        restart_available: true,
        assist_intensity: 1.0,
        team_size_pct: 100,
        reinforcements_pct: 100,
        fov_scale: 1.0,
        music_volume: 0.5,
        mouse_sensitivity: 1.0,
        pad_look_sensitivity: 1.0,
        input_icons_label: "Auto",
        ..Default::default()
    }
}

fn input(f: impl FnOnce(&mut MenuInput)) -> MenuInput {
    let mut i = MenuInput::default();
    f(&mut i);
    i
}

fn at(m: &MenuModel, row: Row) -> MenuModel {
    MenuModel {
        cursor: rows(m).iter().position(|r| *r == row).unwrap(),
        ..m.clone()
    }
}

/// Both games' order (resume, opts, suicide, restart, quit); openbf's Cheats
/// and Change class follow Options. Rows the battle cannot offer drop out.
#[test]
fn the_game_menu_lists_the_original_rows_in_order() {
    let m = ground();
    assert_eq!(
        rows(&m),
        vec![
            Row::Resume,
            Row::Options,
            Row::ClassPicker,
            Row::Respawn,
            Row::Restart,
            Row::Quit
        ]
    );
    let bare = MenuModel {
        cheats: Vec::new(),
        class_picker_available: false,
        respawn_label: None,
        restart_available: false,
        ..m
    };
    assert_eq!(rows(&bare), vec![Row::Resume, Row::Options, Row::Quit]);
}

/// The shell's settings use the flat list. A flat Quit leaves at once.
#[test]
fn the_flat_menu_remains_available() {
    let space = MenuModel {
        page: None,
        ..ground()
    };
    let settings = [
        Row::Assist,
        Row::AiCap,
        Row::Reinforcements,
        Row::Fov,
        Row::Music,
        Row::MouseSensitivity,
        Row::PadLookSensitivity,
        Row::FlightControls,
        Row::Rumble,
        Row::InvertY,
        Row::LowPowerResolution,
        Row::InputIcons,
        Row::Viewpoint,
        Row::CareerAwards,
    ];
    let mut want = vec![Row::Resume, Row::ClassPicker, Row::Cheat(0), Row::Cheat(1)];
    want.extend(settings);
    want.extend([Row::Respawn, Row::Quit]);
    assert_eq!(rows(&space), want);
    let quit = at(&space, Row::Quit);
    assert_eq!(
        navigate(&quit, input(|i| i.confirm = true)).1,
        vec![MenuAction::Quit]
    );
    let shell = MenuModel {
        page: None,
        in_shell: Some("Game Options".into()),
        cheats: Vec::new(),
        ..ground()
    };
    let mut want = vec![Row::Resume];
    want.extend(settings);
    assert_eq!(rows(&shell), want);
}

/// The shell and battle use the same category and device rows.
#[test]
fn the_options_page_is_shared_with_the_shell() {
    let options = MenuModel {
        page: Some(PausePage::Options),
        ..ground()
    };
    assert_eq!(
        rows(&options),
        [
            Row::PageBack,
            Row::Settings(SettingsPage::Game),
            Row::Settings(SettingsPage::Audio),
            Row::Settings(SettingsPage::Graphics),
            Row::Settings(SettingsPage::Controls),
        ]
    );
    let page = |page| MenuModel {
        page: Some(page),
        ..ground()
    };
    let game = page(PausePage::Settings(SettingsPage::Game));
    let mut shell = game.clone();
    shell.cheats.clear();
    shell.in_shell = Some(String::new());
    assert_eq!(rows(&game), rows(&shell));
    assert_eq!(
        row_locked(&shell, Row::Cheats),
        Some("Cheats are available while a battle is paused.")
    );
    let controls = page(PausePage::Settings(SettingsPage::Controls));
    assert_eq!(
        rows(&controls),
        [
            Row::PageBack,
            Row::Settings(SettingsPage::KeyboardMouse),
            Row::Settings(SettingsPage::Gamepad)
        ]
    );
    let pad = page(PausePage::Settings(SettingsPage::Gamepad));
    assert!(rows(&pad).contains(&Row::Assist));
    assert_eq!(
        press(&pad, Row::RumbleOnFire),
        Some(MenuAction::ToggleRumbleOnFire)
    );
    assert!(!rows(&pad).contains(&Row::MouseSensitivity));
    let keyboard = page(PausePage::Settings(SettingsPage::KeyboardMouse));
    assert!(rows(&keyboard).contains(&Row::MouseSensitivity));
    assert!(!rows(&keyboard).contains(&Row::Rumble));
    let cheats = page(PausePage::Cheats);
    assert_eq!(rows(&cheats), [Row::PageBack, Row::Cheat(0), Row::Cheat(1)]);
    let cross = input(|i| i.confirm = true);
    assert_eq!(navigate(&options, cross).1, [MenuAction::ClosePage]);
    assert_eq!(
        navigate(&at(&game, Row::Cheats), cross).1,
        [MenuAction::OpenPage(PausePage::Cheats)]
    );
    assert_eq!(
        navigate(&at(&cheats, Row::Cheat(1)), cross).1,
        [MenuAction::ToggleCheat(1)]
    );
}

/// Quit, Restart and Respawn each ask first; the popup opens on No, and only
/// Yes acts.
#[test]
fn quit_and_restart_ask_first_with_no_focused() {
    let cross = input(|i| i.confirm = true);
    let down_cross = input(|i| {
        i.down = true;
        i.confirm = true
    });
    for (row, ask, popup, yes) in [
        (
            Row::Quit,
            MenuAction::RequestQuit,
            Popup::Quit,
            MenuAction::Quit,
        ),
        (
            Row::Restart,
            MenuAction::RequestRestart,
            Popup::Restart,
            MenuAction::Restart,
        ),
        (
            Row::Respawn,
            MenuAction::RequestRespawn,
            Popup::Respawn,
            MenuAction::Respawn,
        ),
    ] {
        assert_eq!(navigate(&at(&ground(), row), cross).1, vec![ask]);
        let asked = MenuModel {
            popup: Some(popup),
            cursor: 0,
            ..ground()
        };
        assert_eq!(rows(&asked), vec![Row::No, Row::Yes]);
        assert_eq!(navigate(&asked, cross).1, vec![MenuAction::CancelPopup]);
        assert_eq!(navigate(&asked, down_cross).1, vec![yes]);
        // A popup's question is drawn above its rows.
        assert!(popup_prompt(&asked).is_some_and(|p| p.ends_with('?')));
    }
}

/// Back from a settings page goes up one level; the main menu and its popups retain Resume.
#[test]
fn back_returns_from_the_settings_page() {
    let back = input(|i| i.back = true);
    for page in [PausePage::Main, PausePage::Options, PausePage::Cheats] {
        let m = MenuModel {
            page: Some(page),
            ..ground()
        };
        for cursor in 0..rows(&m).len() {
            let m = MenuModel {
                cursor,
                ..m.clone()
            };
            let action = if page == PausePage::Main {
                MenuAction::Resume
            } else {
                MenuAction::ClosePage
            };
            assert_eq!(navigate(&m, back).1, [action], "{page:?}");
        }
    }
    for popup in [Popup::Respawn, Popup::Quit, Popup::Restart] {
        let m = MenuModel {
            popup: Some(popup),
            ..ground()
        };
        assert_eq!(navigate(&m, back).1, vec![MenuAction::Resume], "{popup:?}");
    }
}

/// The Game Menu's title and labels come from its texts.
#[test]
fn controller_rumble_steps_through_four_levels() {
    let mut model = MenuModel {
        page: Some(PausePage::Settings(SettingsPage::Gamepad)),
        ..Default::default()
    };
    let row = rows(&model).iter().position(|r| *r == Row::Rumble).unwrap();
    assert!(step(&model, row, false).is_empty());
    for (strength, _) in RUMBLE_LEVELS.into_iter().skip(1) {
        assert_eq!(step(&model, row, true), [MenuAction::SetRumble(strength)]);
        model.rumble_strength = strength;
    }
    assert!(step(&model, row, true).is_empty());
    assert_eq!(press(&model, Row::Rumble), Some(MenuAction::SetRumble(0.0)));
}

#[test]
fn the_game_menu_is_titled_by_its_page() {
    let m = ground();
    assert_eq!(menu_title(&m), "Game Menu");
    assert_eq!(row_text(&m, Row::Resume).0, "Resume Game");
    assert_eq!(row_text(&m, Row::Quit).0, "Quit");
    let options = MenuModel {
        page: Some(PausePage::Options),
        ..ground()
    };
    assert_eq!(menu_title(&options), "Options");
    let flat = MenuModel { page: None, ..m };
    assert_eq!(menu_title(&flat), "PAUSED");
    assert_eq!(row_text(&flat, Row::Quit).0, "Quit to desktop");
}

/// One frame at 1280 x 900; its actions and its shapes.
fn frame(
    ctx: &egui::Context,
    m: &MenuModel,
    events: Vec<egui::Event>,
) -> (Vec<MenuAction>, Vec<egui::epaint::ClippedShape>) {
    let mut actions = Vec::new();
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| actions = draw_menu(ctx, m),
    );
    (actions, out.shapes)
}

/// A primary click on the row labelled `label`; the actions it gives.
fn click_label(m: &MenuModel, label: &str) -> Vec<MenuAction> {
    let ctx = egui::Context::default();
    let mut shapes = Vec::new();
    for _ in 0..3 {
        shapes = frame(&ctx, m, vec![]).1;
    }
    let pos = shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{label} drawn"));
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let mut out = frame(&ctx, m, vec![egui::Event::PointerMoved(pos), button(true)]).0;
    out.extend(frame(&ctx, m, vec![button(false)]).0);
    out.extend(frame(&ctx, m, vec![]).0);
    out
}

/// The mouse reaches every Game Menu row as Cross does.
#[test]
fn the_game_menu_rows_take_the_mouse() {
    let m = ground();
    assert_eq!(
        click_label(&m, "Options"),
        vec![MenuAction::OpenPage(PausePage::Options)]
    );
    assert_eq!(click_label(&m, "Quit"), vec![MenuAction::RequestQuit]);
    assert_eq!(
        click_label(&m, "Restart Mission"),
        vec![MenuAction::RequestRestart]
    );
    let options = MenuModel {
        page: Some(PausePage::Options),
        ..ground()
    };
    assert_eq!(click_label(&options, "Back"), vec![MenuAction::ClosePage]);
    let asked = MenuModel {
        popup: Some(Popup::Quit),
        ..ground()
    };
    assert_eq!(click_label(&asked, "Yes"), vec![MenuAction::Quit]);
    assert_eq!(click_label(&asked, "No"), vec![MenuAction::CancelPopup]);
}

#[test]
fn career_awards_can_be_set_with_confirm_or_the_value_arrows() {
    let options = MenuModel {
        page: Some(PausePage::Settings(SettingsPage::Game)),
        ..ground()
    };
    let on = at(&options, Row::CareerAwards);
    let off = MenuModel {
        career_awards_disabled: true,
        ..on.clone()
    };
    assert_eq!(
        press(&on, Row::CareerAwards),
        Some(MenuAction::SetCareerAwards(false))
    );
    assert_eq!(
        navigate(&off, input(|i| i.confirm = true)).1,
        vec![MenuAction::SetCareerAwards(true)]
    );
    assert_eq!(
        step(&on, on.cursor, false),
        vec![MenuAction::SetCareerAwards(false)]
    );
    assert_eq!(
        step(&off, off.cursor, true),
        vec![MenuAction::SetCareerAwards(true)]
    );
}
