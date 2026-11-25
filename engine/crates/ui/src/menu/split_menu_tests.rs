//! Issue #194 (B7): the Splitscreen options page (`SPLITSCREEN.md` 18.1 item 4).
use super::*;

fn split(seats: &[usize]) -> SplitMenu {
    let mut seated = [false; 4];
    for &s in seats {
        seated[s] = true;
    }
    SplitMenu {
        top_and_bottom: false,
        seated,
        cells: [0, 1, 2, 3],
        name_tags: true,
        hud_size: 0,
    }
}

fn page(page: PausePage, split: Option<SplitMenu>) -> MenuModel {
    MenuModel {
        page: Some(page),
        split,
        ..Default::default()
    }
}

fn at(m: &MenuModel, row: Row) -> MenuModel {
    MenuModel {
        cursor: rows(m).iter().position(|r| *r == row).unwrap(),
        ..m.clone()
    }
}

fn input(f: impl FnOnce(&mut MenuInput)) -> MenuInput {
    let mut i = MenuInput::default();
    f(&mut i);
    i
}

const SPLIT_PAGE: PausePage = PausePage::Settings(SettingsPage::Splitscreen);

#[test]
fn the_splitscreen_row_shows_only_in_a_split_match() {
    let alone = page(PausePage::Options, None);
    assert!(!rows(&alone).contains(&Row::Settings(SettingsPage::Splitscreen)));
    assert_eq!(rows(&alone).len(), 5);
    let two = page(PausePage::Options, Some(split(&[0, 1])));
    let r = rows(&two);
    assert_eq!(r.len(), 6);
    // Last, so every other row keeps its place.
    assert_eq!(r[..5], rows(&alone)[..]);
    assert_eq!(r[5], Row::Settings(SettingsPage::Splitscreen));
    assert_eq!(press(&two, r[5]), Some(MenuAction::OpenPage(SPLIT_PAGE)));
    // The page's rows: one screen row per seated seat, by seat number.
    let three = page(SPLIT_PAGE, Some(split(&[0, 2, 3])));
    assert_eq!(
        rows(&three),
        [
            Row::PageBack,
            Row::SplitTwoPlayers,
            Row::SplitScreen(0),
            Row::SplitScreen(2),
            Row::SplitScreen(3),
            Row::SplitNameTags,
            Row::SplitHudSize,
        ]
    );
    assert_eq!(menu_title(&three), "Splitscreen Options");
    // Without split the page is only its Back row.
    assert_eq!(rows(&page(SPLIT_PAGE, None)), [Row::PageBack]);
}

#[test]
fn back_from_the_splitscreen_page_returns_to_its_options_row() {
    assert_eq!(
        SPLIT_PAGE.parent(),
        Some((PausePage::Options, Row::Settings(SettingsPage::Splitscreen)))
    );
    let m = page(SPLIT_PAGE, Some(split(&[0, 1])));
    assert_eq!(
        navigate(&at(&m, Row::SplitNameTags), input(|i| i.back = true)).1,
        [MenuAction::ClosePage]
    );
    assert_eq!(
        navigate(&at(&m, Row::PageBack), input(|i| i.confirm = true)).1,
        [MenuAction::ClosePage]
    );
}

#[test]
fn the_two_player_row_is_locked_with_three_or_four() {
    let right = input(|i| i.right = true);
    let cross = input(|i| i.confirm = true);
    let two = page(SPLIT_PAGE, Some(split(&[0, 1])));
    assert_eq!(row_locked(&two, Row::SplitTwoPlayers), None);
    assert_eq!(
        navigate(&at(&two, Row::SplitTwoPlayers), right).1,
        [MenuAction::SetSplitTwoPlayer(true)]
    );
    assert_eq!(
        navigate(&at(&two, Row::SplitTwoPlayers), cross).1,
        [MenuAction::SetSplitTwoPlayer(true)]
    );
    for seats in [&[0, 1, 2][..], &[0, 1, 2, 3], &[0, 1, 3]] {
        let m = page(SPLIT_PAGE, Some(split(seats)));
        assert!(row_locked(&m, Row::SplitTwoPlayers).is_some());
        assert_eq!(
            navigate(&at(&m, Row::SplitTwoPlayers), right).1,
            [] as [MenuAction; 0]
        );
        assert_eq!(
            navigate(&at(&m, Row::SplitTwoPlayers), cross).1,
            [MenuAction::Denied]
        );
    }
}

#[test]
fn the_screen_rows_step_their_own_seat_and_show_its_place() {
    let left = input(|i| i.left = true);
    let right = input(|i| i.right = true);
    let cross = input(|i| i.confirm = true);
    let two = page(SPLIT_PAGE, Some(split(&[0, 2])));
    for (inp, r) in [(left, false), (right, true), (cross, true)] {
        assert_eq!(
            navigate(&at(&two, Row::SplitScreen(2)), inp).1,
            [MenuAction::MoveSplitScreen { seat: 2, right: r }]
        );
    }
    // Two seated: the side each has of the two, whatever the absent seats hold.
    let s = SplitMenu {
        cells: [3, 0, 1, 2],
        ..split(&[0, 2])
    };
    assert_eq!((s.screen_label(0), s.screen_label(2)), ("Right", "Left"));
    let tb = SplitMenu {
        top_and_bottom: true,
        ..s
    };
    assert_eq!((tb.screen_label(0), tb.screen_label(2)), ("Bottom", "Top"));
    // Three or four: the grid place.
    let s = SplitMenu {
        cells: [3, 1, 2, 0],
        ..split(&[0, 1, 2])
    };
    assert_eq!(s.screen_label(0), "Bottom Right");
    assert_eq!(s.screen_label(1), "Top Right");
    let m = page(SPLIT_PAGE, Some(s));
    assert_eq!(
        row_text(&m, Row::SplitScreen(0)),
        ("P1".to_string(), Some("Bottom Right".to_string()))
    );
}

#[test]
fn name_tags_and_hud_size_step() {
    let left = input(|i| i.left = true);
    let right = input(|i| i.right = true);
    let m = page(SPLIT_PAGE, Some(split(&[0, 1])));
    assert_eq!(
        navigate(&at(&m, Row::SplitNameTags), right).1,
        [MenuAction::SetSplitNameTags(false)]
    );
    assert_eq!(
        navigate(&at(&m, Row::SplitHudSize), right).1,
        [MenuAction::SetSplitHudSize(1)]
    );
    assert_eq!(
        navigate(&at(&m, Row::SplitHudSize), left).1,
        [MenuAction::SetSplitHudSize(2)]
    );
    assert_eq!(
        row_text(&m, Row::SplitHudSize),
        ("HUD Size".to_string(), Some("Full".to_string()))
    );
    assert_eq!(
        row_text(&m, Row::SplitNameTags).1.as_deref(),
        Some(on_off(true))
    );
    for row in [
        Row::SplitTwoPlayers,
        Row::SplitScreen(0),
        Row::SplitNameTags,
        Row::SplitHudSize,
    ] {
        assert!(is_value_row(row));
    }
}

/// Issue #194 (B10): every settings row on the Game Menu's pages has a scope, every action a
/// row makes (a press or either step) has that same scope, and only with a non-P1 opener do
/// the host rows say they are shared (the player's rows never do).
#[test]
fn every_settings_row_and_its_actions_share_one_scope() {
    let pages = [
        PausePage::Settings(SettingsPage::Game),
        PausePage::Settings(SettingsPage::Audio),
        PausePage::Settings(SettingsPage::Graphics),
        PausePage::Settings(SettingsPage::KeyboardMouse),
        PausePage::Settings(SettingsPage::Gamepad),
        PausePage::Settings(SettingsPage::Splitscreen),
    ];
    let (mut players, mut hosts) = (0, 0);
    for p in pages {
        let mut m = page(p, Some(split(&[0, 1])));
        m.cheats = vec![("c", "C", false)];
        m.base_fov_deg = 60.0;
        m.fov_scale = 1.0;
        m.music_volume = 0.5;
        m.mouse_sensitivity = 1.0;
        m.pad_look_sensitivity = 1.0;
        m.assist_intensity = 0.5;
        m.team_size_pct = 100;
        m.reinforcements_pct = 100;
        m.difficulty = 2;
        for row in rows(&m) {
            if row == Row::PageBack {
                continue;
            }
            let scope = row_scope(row).unwrap_or_else(|| panic!("{row:?} has no scope"));
            match scope {
                RowScope::Player => players += 1,
                RowScope::Host => hosts += 1,
            }
            let at_row = at(&m, row);
            for input in [
                MenuInput {
                    confirm: true,
                    ..Default::default()
                },
                MenuInput {
                    left: true,
                    ..Default::default()
                },
                MenuInput {
                    right: true,
                    ..Default::default()
                },
            ] {
                for a in navigate(&at_row, input).1 {
                    if matches!(a, MenuAction::OpenPage(_) | MenuAction::Denied) {
                        continue;
                    }
                    assert_eq!(action_scope(a), Some(scope), "{row:?} makes {a:?}");
                }
            }
            let shared = MenuModel {
                shared_rows: true,
                ..m.clone()
            };
            let line = match row {
                Row::ResetGraphics => RESET_SHARED_HELP,
                _ => SHARED_HELP,
            };
            let says = |m: &MenuModel| row_help(m, row).is_some_and(|h| h.contains(line));
            assert!(!says(&m), "{row:?} with P1 as opener");
            assert_eq!(says(&shared), scope == RowScope::Host, "{row:?}");
        }
    }
    assert!(
        players >= 12 && hosts >= 16,
        "{players} player rows, {hosts} host rows"
    );
    assert_eq!(row_scope(Row::Resume), None);
    assert_eq!(action_scope(MenuAction::Respawn), None);
}

/// Issue #194 B22: each screen row is labelled by its seat's player ("Han", "Leia"), the place
/// beside it; a seat with no usable name falls back to its number ("P3"). The native panel ends
/// a name too wide for its row in an ellipsis (`Ctx::text_in`, `typography::ellipsize`).
#[test]
fn split_screen_rows_show_profile_names() {
    let long = "Commander of the Fleet XY".to_string();
    let m = MenuModel {
        split_names: [
            Some("Han".into()),
            Some("Leia".into()),
            None,
            Some(long.clone()),
        ],
        ..page(SPLIT_PAGE, Some(split(&[0, 1, 2, 3])))
    };
    let label = |seat: u8| row_text(&m, Row::SplitScreen(seat));
    assert_eq!(label(0), ("Han".to_string(), Some("Top Left".to_string())));
    assert_eq!(
        label(1),
        ("Leia".to_string(), Some("Top Right".to_string()))
    );
    assert_eq!(label(2).0, "P3");
    assert_eq!(label(3).0, long);
    // The row's ellipsis: the long name cut to a narrow row, at the role's own size.
    let font = manifest::hud::HudFont {
        name: "f".into(),
        line_height: 10,
        pages: vec!["p".into()],
        glyphs: long
            .chars()
            .chain(['.', '\u{2026}'])
            .map(|c| manifest::hud::HudGlyph {
                code: c as u32 as _,
                page: 0,
                advance: 6,
                x0: 0,
                x1: 5,
                y0: 0,
                y1: 9,
                uv: [0.0; 4],
            })
            .collect(),
    };
    let room = crate::typography::width(&font, &crate::typography::ITEM, "Commander") + 0.5;
    let cut = crate::typography::ellipsize(&font, &crate::typography::ITEM, &long, room);
    assert!(cut.ends_with(crate::typography::ELLIPSIS), "{cut}");
    assert!(crate::typography::width(&font, &crate::typography::ITEM, &cut) <= room);
}

/// Issue #194: in a LAN round (`own_cheats`) cheats are per slot, so a further player's Cheats
/// rows say they are their own, never "Shared ... for every player"; offline they stay shared.
#[test]
fn lan_cheat_rows_say_they_are_the_openers_own() {
    let m = MenuModel {
        cheats: vec![("stamina", "Infinite stamina", false)],
        shared_rows: true,
        ..Default::default()
    };
    let lan = MenuModel {
        own_cheats: true,
        ..m.clone()
    };
    for row in [Row::Cheats, Row::Cheat(0)] {
        let help = |m: &MenuModel| row_help(m, row).unwrap_or_default();
        assert!(help(&m).contains(SHARED_HELP), "{row:?} offline");
        assert!(help(&lan).contains(OWN_CHEATS_HELP), "{row:?} in LAN");
        assert!(!help(&lan).contains(SHARED_HELP), "{row:?} in LAN");
    }
    // Other host rows stay shared in LAN.
    assert!(row_help(&lan, Row::Difficulty).is_some_and(|h| h.contains(SHARED_HELP)));
}
