use super::*;
const BUTTON_FONT: &str = "gamefont_large";
use crate::typography::tint::SELECTED as SELECTED_TEXT;

#[test]
fn shell_draw_and_pointer_share_menu_metrics_at_all_capture_sizes() {
    use crate::hud_draw::HudArt;
    use crate::hud_tree::Hud;
    use manifest::hud::{HudFont, HudGlyph, HudLayout};
    let legacy = HudFont {
        name: BUTTON_FONT.into(),
        line_height: 22,
        pages: vec!["legacy".into()],
        glyphs: (32..127)
            .map(|code| HudGlyph {
                code,
                page: 0,
                advance: 4,
                x0: 0,
                x1: 4,
                y0: 3,
                y1: 18,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect(),
    };
    let mut menu = legacy.clone();
    menu.name = "menu_gamefont_large".into();
    menu.pages = vec!["menu".into()];
    for g in &mut menu.glyphs {
        g.advance = 12;
        g.x1 = 12;
    }
    let mut art = HudArt::default();
    art.fonts.insert(legacy.name.clone(), legacy);
    art.fonts.insert(menu.name.clone(), menu);
    let mut menus = ShellMenus::new(&|_, fallback| fallback.into());
    menus.page = Page::Main;
    menus.on_page = 5.0;
    let empty = HudLayout {
        name: "empty".into(),
        nodes: Vec::new(),
    };
    let mut hud = Hud::new(&empty, &empty);
    hud.shell = Some((menus.clone(), false));
    let layout = stack_layout(art.menu_font(BUTTON_FONT).unwrap(), menus.stack().unwrap());
    for size in [
        [1920.0, 1080.0],
        [2560.0, 1440.0],
        [3840.0, 2160.0],
        [640.0, 480.0],
    ] {
        let frame = crate::hud_draw::draw(&hud, &art, size);
        assert!(frame.batches.iter().any(|b| b.texture == "menu"));
        assert!(!frame.batches.iter().any(|b| b.texture == "legacy"));
        let [x0, y0, x1, y1] = layout.rows[0];
        let screen = crate::spawn_draw::screen_at(size, crate::spawn_draw::Anchor::Centre);
        let point = screen.map(glam::vec3(x1 - 1.0, (y0 + y1) * 0.5, 0.0));
        assert_eq!(
            row_at(art.menu_font(BUTTON_FONT).unwrap(), &menus, point, size),
            Some(0)
        );
        // The edge of this wider row is outside the old, narrow font's bar.
        assert!(x1 > x0);
        assert_eq!(row_at(&art.fonts[BUTTON_FONT], &menus, point, size), None);
    }
    hud.shell = None;
    let ctx = Ctx {
        hud: &hud,
        art: &art,
        out: Default::default(),
        clip: None,
    };
    let face = ctx.face(crate::typography::ITEM.face).unwrap();
    assert_eq!(face.name, "menu_gamefont_large");
}

fn menus() -> ShellMenus {
    ShellMenus::new(&|_, e| e.to_owned())
}

fn press(f: impl Fn(&mut MenuInput)) -> MenuInput {
    let mut i = MenuInput::default();
    f(&mut i);
    i
}

/// Acceptance 2 of card BSHLA, updated for issue #168: Multiplayer sits
/// in its PS2 slot and is live; the dimmed Splitscreen is skipped both
/// ways (§3.3).
#[test]
fn main_menu_skips_the_disabled_splitscreen() {
    let mut m = menus();
    assert_eq!(m.step(MenuInput::default(), true, 0.1), None);
    assert_eq!(m.page, Page::Main);
    let ids: Vec<_> = m.main.rows.iter().map(|r| r.id).collect();
    assert_eq!(
        ids,
        [
            RowId::Singleplayer,
            RowId::Multiplayer,
            RowId::Splitscreen,
            RowId::Options,
            RowId::QuitGame
        ]
    );
    assert!(m.main.rows[1].enabled, "Multiplayer is live (issue #168)");
    assert!(!m.main.rows[2].enabled, "Splitscreen is disabled");
    assert_eq!(m.main.focus, 0);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.main.rows[m.main.focus].id, RowId::Multiplayer);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(
        m.main.rows[m.main.focus].id,
        RowId::Options,
        "Down skips Splitscreen"
    );
    m.step(press(|i| i.up = true), false, 0.1);
    assert_eq!(m.main.rows[m.main.focus].id, RowId::Multiplayer, "and Up");
    m.step(press(|i| i.up = true), false, 0.1);
    // Wrapping: Up from Singleplayer lands on Quit Game, Down from Quit
    // Game on Singleplayer.
    m.step(press(|i| i.up = true), false, 0.1);
    assert_eq!(m.main.rows[m.main.focus].id, RowId::QuitGame);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.main.rows[m.main.focus].id, RowId::Singleplayer);
    // A confirm never lands on a disabled row.
    m.main.focus = 2;
    assert_eq!(m.step(press(|i| i.confirm = true), false, 0.1), None);
    assert_eq!(m.page, Page::Main);
}

/// Playtest A5: the main menu's bottom row is Quit Game, by pad (the
/// wrap) or by mouse, and it quits.
#[test]
fn main_menu_quit_game_is_the_last_row_and_quits() {
    use crate::shell_feel::MenuSound as S;
    use manifest::hud::{HudFont, HudGlyph};
    let mut m = menus();
    m.step(MenuInput::default(), true, 0.1);
    m.cues.take();
    m.step(press(|i| i.up = true), false, 0.2);
    let row = &m.main.rows[m.main.focus];
    assert_eq!((row.id, row.label.as_str()), (RowId::QuitGame, "Quit Game"));
    assert_eq!(m.main.focus, m.main.rows.len() - 1, "the bottom row");
    m.cues.take();
    assert_eq!(
        m.step(press(|i| i.confirm = true), false, 0.2),
        Some(ShellCommand::Quit)
    );
    assert_eq!(m.cues.take(), [S::Accept]);
    // The mouse: the row's bar is a hit, and a click on it quits.
    let font = HudFont {
        name: BUTTON_FONT.into(),
        line_height: 22,
        pages: vec!["page0".into()],
        glyphs: (32u8..127)
            .map(|code| HudGlyph {
                code: u16::from(code),
                page: 0,
                advance: 10,
                x0: 0,
                x1: 10,
                y0: 2,
                y1: 14,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect(),
    };
    let mut m = menus();
    m.page = Page::Main;
    m.on_page = 2.0;
    let [x0, y0, x1, y1] = stack_layout(&font, m.stack().unwrap()).rows[4];
    let p = [
        ((x0 + x1) * 0.5) * 1.25,
        ((y0 + y1) * 0.5) * (480.0 / 448.0),
    ];
    assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), Some(4));
    assert!(m.focus_row(4));
    assert_eq!(
        m.step(press(|i| i.confirm = true), false, 0.2),
        Some(ShellCommand::Quit)
    );
}

#[test]
fn mouse_hit_uses_the_visible_row_layout_and_skips_disabled_rows() {
    use manifest::hud::{HudFont, HudGlyph};
    let font = HudFont {
        name: BUTTON_FONT.into(),
        line_height: 22,
        pages: vec!["page0".into()],
        glyphs: (32u8..127)
            .map(|code| HudGlyph {
                code: u16::from(code),
                page: 0,
                advance: 10,
                x0: 0,
                x1: 10,
                y0: 2,
                y1: 14,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect(),
    };
    let mut m = menus();
    m.page = Page::Main;
    m.on_page = 2.0;
    let stack = m.stack().unwrap();
    let layout = stack_layout(&font, stack);
    let [x0, y0, x1, y1] = layout.rows[3];
    let p = [
        ((x0 + x1) * 0.5) * 1.25,
        ((y0 + y1) * 0.5) * (480.0 / 448.0),
    ];
    assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), Some(3));
    let [x0, y0, x1, y1] = layout.rows[2];
    let p = [
        ((x0 + x1) * 0.5) * 1.25,
        ((y0 + y1) * 0.5) * (480.0 / 448.0),
    ];
    assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), None);
}

/// `m` as the HUD pass lays it out in a 640 x 480 window, with a font
/// of 10-unit glyphs.
fn frame(m: ShellMenus) -> crate::hud_draw::HudFrame {
    use crate::hud_draw::HudArt;
    use crate::hud_tree::Hud;
    use manifest::hud::{HudFont, HudGlyph, HudLayout};
    let font = HudFont {
        name: BUTTON_FONT.into(),
        line_height: 22,
        pages: vec!["page0".into()],
        glyphs: (32u8..127)
            .map(|code| HudGlyph {
                code: u16::from(code),
                page: 0,
                advance: 10,
                x0: 0,
                x1: 10,
                y0: 2,
                y1: 14,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect(),
    };
    let mut art = HudArt::default();
    art.fonts.insert(BUTTON_FONT.into(), font);
    let empty = HudLayout {
        name: "t".into(),
        nodes: Vec::new(),
    };
    let mut hud = Hud::new(&empty, &empty);
    hud.shell = Some((m, false));
    crate::hud_draw::draw(&hud, &art, [640.0, 480.0])
}

/// The main menu as the HUD pass lays it out: the dimmed rows' labels
/// are drawn grey 110 (§3.3), every glyph of both of them.
#[test]
fn the_disabled_rows_are_drawn_dimmed() {
    let mut m = menus();
    m.step(MenuInput::default(), true, 5.0);
    m.step(MenuInput::default(), false, 5.0);
    let f = frame(m);
    let grey = DIMMED_GREY / 255.0;
    let dimmed = f
        .verts
        .iter()
        .filter(|v| v.color[..3].iter().all(|c| (c - grey).abs() < 1e-4))
        .count();
    let glyphs = "Splitscreen".len();
    assert_eq!(dimmed, 4 * glyphs, "Splitscreen dimmed");
    let sheets: Vec<&str> = f.batches.iter().map(|b| b.texture.as_str()).collect();
    for p in [
        "logo",
        "bf2_buttons_topleft",
        "bf2_buttons_botright",
        "btna",
        "btnb",
    ] {
        assert!(sheets.contains(&p), "{p}: {sheets:?}");
    }
}

#[test]
fn the_flow_reaches_instant_action_and_back() {
    let mut m = menus();
    let confirm = press(|i| i.confirm = true);
    // Only Start leaves the title: Cross does nothing there.
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::Title);
    m.step(MenuInput::default(), true, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::Singleplayer);
    assert_eq!(m.movie(), MOVIE_SUB);
    // Rise of the Empire is live; down skips disabled Galactic Conquest to Instant Action.
    assert_eq!(
        m.singleplayer.rows[m.singleplayer.focus].id,
        RowId::RiseOfTheEmpire
    );
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(
        m.singleplayer.rows[m.singleplayer.focus].id,
        RowId::InstantAction
    );
    assert_eq!(
        m.step(confirm, false, 0.1),
        Some(ShellCommand::InstantAction)
    );
    m.step(press(|i| i.back = true), false, 0.1);
    assert_eq!(m.page, Page::Main);
    // Down: Multiplayer (issue #168), then Options past Splitscreen.
    m.step(press(|i| i.down = true), false, 0.1);
    m.step(press(|i| i.down = true), false, 0.1);
    m.step(confirm, false, 0.1);
    assert_eq!(m.page, Page::Options);
    assert_eq!(
        m.step(confirm, false, 0.1),
        Some(ShellCommand::Settings(crate::menu::SettingsPage::Game))
    );
    m.step(press(|i| i.back = true), false, 0.1);
    m.step(press(|i| i.back = true), false, 0.1);
    assert_eq!(m.page, Page::Title);
    assert_eq!(m.movie(), MOVIE_MAIN);
    assert_eq!(
        m.step(press(|i| i.back = true), false, 0.1),
        Some(ShellCommand::Quit)
    );
}

#[test]
fn controls_are_reachable_from_options() {
    let mut m = menus();
    m.page = Page::Options;
    m.options.focus = m
        .options
        .rows
        .iter()
        .position(|row| row.id == RowId::Controls)
        .unwrap();
    assert!(m.options.rows[m.options.focus].enabled);
    assert_eq!(
        m.step(press(|i| i.confirm = true), false, 0.1),
        Some(ShellCommand::Settings(crate::menu::SettingsPage::Controls))
    );
}

#[test]
fn the_prompt_pulses_between_its_alphas() {
    let mut m = menus();
    let mut seen = (f32::MAX, f32::MIN);
    for _ in 0..100 {
        m.step(MenuInput::default(), false, 0.05);
        let a = m.prompt_alpha();
        seen = (seen.0.min(a), seen.1.max(a));
    }
    assert!(
        (seen.0 - 0.4).abs() < 0.03 && (seen.1 - 1.0).abs() < 0.03,
        "{seen:?}"
    );
}

/// re-bf2-menus §4.1: Start on the title accepts; a stack move clicks
/// even through the wrap; Cross accepts; Back plays the exit click.
#[test]
fn the_stacks_ask_for_the_ps2_clicks() {
    use crate::shell_feel::MenuSound as S;
    let mut m = menus();
    m.step(press(|i| i.confirm = true), false, 0.2);
    assert!(m.cues.take().is_empty(), "Cross does nothing on the title");
    m.step(MenuInput::default(), true, 0.2);
    assert_eq!(m.cues.take(), [S::Accept]);
    // Multiplayer, Options, Quit Game, Singleplayer (the wrap),
    // Multiplayer, Options.
    for _ in 0..6 {
        m.step(press(|i| i.down = true), false, 0.2);
    }
    assert_eq!(m.cues.take(), [S::Select; 6], "every press, the wrap too");
    m.step(press(|i| i.confirm = true), false, 0.2);
    assert_eq!(m.page, Page::Options);
    assert_eq!(m.cues.take(), [S::Accept]);
    m.step(press(|i| i.back = true), false, 0.2);
    assert_eq!(m.cues.take(), [S::Exit]);
    // Presses faster than 0.1 s apart: one click.
    m.step(press(|i| i.down = true), false, 0.02);
    m.step(press(|i| i.down = true), false, 0.02);
    assert_eq!(m.cues.take(), [S::Select]);
    // Back from a page of the port's own: the stack's re-Enter.
    m.on_page = 3.0;
    m.uncover();
    assert_eq!((m.cues.take(), m.on_page), (vec![S::Exit], 0.0));
}

/// re-bf2-menus §5: the pulse's phase runs on through a focus move, and
/// the focused row's label and bar share it; the others sit at 0.6 with
/// a grey 150 bar. It steps at 30 Hz (menuref §9.4.1).
#[test]
fn the_focused_row_pulses_and_keeps_its_phase_across_moves() {
    let mut m = menus();
    m.step(MenuInput::default(), true, 0.0);
    m.step(MenuInput::default(), false, 0.15);
    let before = m.pulse.alpha();
    // 0.15 s = 4 whole updates of 2/30 from 0.5.
    assert!((before - (0.5 + 4.0 * 2.0 / 30.0)).abs() < 1e-4, "{before}");
    m.step(press(|i| i.down = true), false, 0.05);
    assert!(
        (m.pulse.alpha() - 0.9).abs() < 1e-4,
        "not reset by the move"
    );
    m.on_page = 5.0;
    let a = m.pulse.alpha();
    let f = frame(m);
    let [r, g, b] = SELECTED_TEXT.0.map(|c| c / 255.0);
    let focused = |v: &&crate::hud_draw::HudVertex| {
        (v.color[0] - r).abs() < 1e-4
            && (v.color[1] - g).abs() < 1e-4
            && (v.color[2] - b).abs() < 1e-4
    };
    let label: Vec<_> = f.verts.iter().filter(focused).collect();
    // Down from Singleplayer: Multiplayer (issue #168).
    assert_eq!(label.len(), 4 * "Multiplayer".len());
    assert!(label.iter().all(|v| (v.color[3] - a).abs() < 1e-4));
    let white_bar = f
        .verts
        .iter()
        .filter(|v| v.color == [1.0, 1.0, 1.0, a])
        .count();
    assert_eq!(white_bar, 3 * 4, "the focused bar's three pieces");
    let grey = UNFOCUSED_BAR / 255.0;
    let grey_bars = f
        .verts
        .iter()
        .filter(|v| v.color == [grey, grey, grey, 0.6])
        .count();
    assert_eq!(grey_bars, 4 * 3 * 4, "the other four rows' bars");
}

/// re-bf2-menus §5, menuref §9.4.5: on entering a page nothing of row 1
/// shows until its 0.1 s delay, then its bar wipes out before its text
/// appears.
#[test]
fn a_page_entry_replays_the_flashy_entrance() {
    let mut m = menus();
    m.step(MenuInput::default(), true, 0.0);
    let bar_right = |m: &ShellMenus| {
        let f = frame(m.clone());
        f.verts
            .iter()
            .filter(|v| v.color[..3] == [1.0, 1.0, 1.0] && v.color[3] == m.pulse.alpha())
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max)
    };
    m.on_page = 0.05;
    assert_eq!(bar_right(&m), f32::MIN, "row 1 not started");
    m.on_page = 0.1 + 0.05;
    let half = bar_right(&m);
    m.on_page = 0.1 + 0.1;
    let full = bar_right(&m);
    assert!(half > f32::MIN && half < full, "{half} {full}");
    let text = |m: &ShellMenus| {
        let [r, g, b] = SELECTED_TEXT.0.map(|c| c / 255.0);
        frame(m.clone())
            .verts
            .iter()
            .filter(|v| v.color[..3] == [r, g, b])
            .count()
    };
    m.on_page = 0.1 + 0.15;
    assert_eq!(text(&m), 0, "the square sweeps before the text");
    m.on_page = 0.1 + 0.3;
    assert_eq!(text(&m), 4 * "Singleplayer".len());
}

/// menuref "Singleplayer": coming back to Singleplayer (from Instant
/// Action, or from the Main Menu) focuses its first row, not the one
/// left.
#[test]
fn singleplayer_is_always_entered_on_its_first_row() {
    let mut m = menus();
    // As if Training were shown: the first row live.
    m.singleplayer.rows[0].enabled = true;
    m.step(MenuInput::default(), true, 0.1);
    m.step(press(|i| i.confirm = true), false, 0.1);
    assert_eq!(m.page, Page::Singleplayer);
    assert_eq!(m.singleplayer.focus, 0);
    m.singleplayer.focus = 2;
    // Instant Action opens over it and closes: back on the first row.
    m.uncover();
    assert_eq!(m.singleplayer.focus, 0);
    m.singleplayer.focus = 2;
    m.step(press(|i| i.back = true), false, 0.1);
    m.step(press(|i| i.confirm = true), false, 0.1);
    assert_eq!(m.page, Page::Singleplayer);
    assert_eq!(m.singleplayer.focus, 0);
}

/// Issue #168: Main → Multiplayer offers LAN Host Game and Join Game;
/// Online is shown dimmed with its note, never focused; Back returns to
/// the main menu.
#[test]
fn multiplayer_offers_lan_host_and_join_and_a_dimmed_online_row() {
    let mut m = menus();
    let confirm = press(|i| i.confirm = true);
    m.step(MenuInput::default(), true, 0.1);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::Multiplayer);
    assert_eq!(m.movie(), MOVIE_SUB);
    let rows: Vec<_> = m
        .multiplayer
        .rows
        .iter()
        .map(|r| (r.id, r.label.as_str(), r.enabled))
        .collect();
    assert_eq!(
        rows,
        [
            (RowId::LanHost, "LAN: Host Game", true),
            (RowId::LanJoin, "LAN: Join Game", true),
            (RowId::Online, "Online", false),
        ]
    );
    assert_eq!(
        m.multiplayer.note.as_deref(),
        Some("Online play needs a dedicated server \u{2014} not available yet")
    );
    assert_eq!(m.step(confirm, false, 0.1), Some(ShellCommand::HostGame));
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), Some(ShellCommand::JoinGame));
    // Down from Join wraps past the dimmed Online row to Host.
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.multiplayer.rows[m.multiplayer.focus].id, RowId::LanHost);
    assert!(!m.focus_row(2), "Online cannot take the pointer's focus");
    m.step(press(|i| i.back = true), false, 0.1);
    assert_eq!(m.page, Page::Main);
}

/// The Online note is drawn under the Multiplayer stack once revealed.
#[test]
fn the_online_note_is_drawn_on_the_multiplayer_page() {
    let mut m = menus();
    m.page = Page::Multiplayer;
    m.step(MenuInput::default(), false, 5.0);
    let f = frame(m);
    let note = ([DIMMED_GREY + 60.0; 3].map(|c| c / 255.0), ());
    let lit = f
        .verts
        .iter()
        .filter(|v| {
            v.color[..3]
                .iter()
                .zip(note.0)
                .all(|(c, n)| (c - n).abs() < 1e-4)
        })
        .count();
    assert!(lit >= 4 * "Online".len(), "note glyphs drawn: {lit}");
}

#[test]
fn rise_of_the_empire_page_lists_unlocked_missions_toggles_all_and_launches() {
    let mut m = menus();
    let confirm = press(|i| i.confirm = true);
    m.step(MenuInput::default(), true, 0.1);
    m.step(confirm, false, 0.1);
    assert_eq!(m.page, Page::Singleplayer);

    // First visit prompts for Geonosis training; Skip unlocks Turn 1 and opens New Game / Load.
    m.step(confirm, false, 0.1);
    assert_eq!(m.page, Page::AskTraining);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert!(m.training_prompted());
    assert_eq!(m.campaign_unlocked, 2);
    assert_eq!(m.page, Page::RiseNewLoad);

    // New Game emits StartCampaignNewGame targeting Turn 1 (Mygeeto).
    assert_eq!(
        m.step(confirm, false, 0.1),
        Some(ShellCommand::StartCampaignNewGame)
    );
    assert_eq!(m.campaign_selected, 1);

    // Load opens the save list without the load question (`SkipPromptSave`); Back returns.
    m.open_page(Page::RiseNewLoad);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::CampaignSaves);
    assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::List);
    m.step(press(|i| i.back = true), false, 0.1);
    assert_eq!(m.page, Page::RiseNewLoad);

    // Open CampaignBattle directly (as LoadCampaign / crawl completion does), check 3D planet movie == ""
    m.select_campaign_mission(1);
    m.open_page(Page::CampaignBattle);
    assert_eq!(m.page, Page::CampaignBattle);
    assert_eq!(m.movie(), "");

    // Esc / Back / Start from CampaignBattle opens Game Menu (CampaignMenu) over the planet.
    assert_eq!(m.step(press(|i| i.back = true), false, 0.1), None);
    assert_eq!(m.page, Page::CampaignMenu);
    assert_eq!(m.movie(), "");

    // Row 0 = Resume Game -> back to CampaignBattle.
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::CampaignBattle);

    // Re-open Game Menu -> Options -> Back returns to Game Menu (not Main).
    m.step(MenuInput::default(), true, 0.1);
    assert_eq!(m.page, Page::CampaignMenu);
    m.step(press(|i| i.down = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::Options);
    assert_eq!(m.step(press(|i| i.back = true), false, 0.1), None);
    assert_eq!(m.page, Page::CampaignMenu);

    // Save Game opens the save list; Load Game asks the load question first.
    m.focus_row(2);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::CampaignSaves);
    assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::List);
    m.step(press(|i| i.back = true), false, 0.1);
    assert_eq!(m.page, Page::CampaignMenu);
    m.focus_row(3);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::ConfirmLoad);
    m.step(press(|i| i.up = true), false, 0.1);
    m.step(confirm, false, 0.1);
    assert_eq!(m.page, Page::CampaignMenu);

    // Quit opens Yes/No confirmation defaulting to No; Back cancels, Yes returns to Singleplayer.
    m.focus_row(4);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert!(m.campaign_quit_confirm);
    assert_eq!(m.campaign_quit_popup.focus, 1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert!(!m.campaign_quit_confirm);
    assert_eq!(m.page, Page::CampaignMenu);

    // Yes asks the save question (`QuitRequested`), then leaves.
    m.focus_row(4);
    m.step(confirm, false, 0.1);
    m.step(press(|i| i.up = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::CampaignSaves);
    assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::Prompt);
    m.step(press(|i| i.up = true), false, 0.1);
    assert_eq!(m.step(confirm, false, 0.1), None);
    assert_eq!(m.page, Page::Singleplayer);

    // Attack from CampaignBattle launches the selected mission.
    m.set_campaign_unlocked(18);
    m.select_campaign_mission(17);
    m.open_page(Page::CampaignBattle);
    assert_eq!(
        m.step(confirm, false, 0.1),
        Some(ShellCommand::LaunchCampaignMission { index: 17 })
    );

    // Once prompted, Training appears as Singleplayer's first row and launches mission 0 directly.
    m.open_page(Page::Singleplayer);
    assert_eq!(m.singleplayer.rows[0].id, RowId::Training);
    assert_eq!(
        m.step(confirm, false, 0.1),
        Some(ShellCommand::LaunchCampaignMission { index: 0 })
    );
}

#[test]
fn keyboard_enter_sets_confirm_and_start_without_hijacking_campaign_menu() {
    let mut m = menus();
    m.set_campaign_unlocked(2);
    m.select_campaign_mission(1);
    m.open_page(Page::CampaignBattle);

    // On desktop, KeyCode::Enter sets both confirm and start in menu_bits().
    let enter = press(|i| i.confirm = true);
    assert_eq!(
        m.step(enter, true, 0.1),
        Some(ShellCommand::LaunchCampaignMission { index: 1 })
    );
    assert_eq!(m.page, Page::CampaignBattle);

    // Esc / controller Start (back/start without confirm) still opens Game Menu.
    assert_eq!(m.step(press(|i| i.back = true), false, 0.1), None);
    assert_eq!(m.page, Page::CampaignMenu);

    // Inside Game Menu, Enter on Quit opens confirmation -> Up -> Yes -> the save question.
    m.focus_row(4);
    assert_eq!(m.step(enter, true, 0.1), None);
    assert!(m.campaign_quit_confirm);
    assert_eq!(m.page, Page::CampaignMenu);
    m.step(press(|i| i.up = true), false, 0.1);
    assert_eq!(m.step(enter, true, 0.1), None);
    assert_eq!(m.page, Page::CampaignSaves);
}

/// The end-of-campaign crawl is a real page: no stack or shell movie of
/// its own, and any skip input returns to the Singleplayer screen that
/// queued it.
#[test]
fn credits_is_a_stackless_page_that_returns_to_singleplayer() {
    let mut m = menus();
    m.open_page(Page::Credits);
    assert_eq!(m.page, Page::Credits);
    assert!(m.stack().is_none(), "the game draws the crawl, not a stack");
    assert_eq!(m.movie(), "", "no shell movie under the crawl");
    // Idle frames stay on the crawl.
    assert_eq!(m.step(MenuInput::default(), false, 0.1), None);
    assert_eq!(m.page, Page::Credits);
    // Cross, Back/Triangle or Start all leave for Singleplayer.
    m.step(press(|i| i.back = true), false, 0.1);
    assert_eq!(m.page, Page::Singleplayer);
}
