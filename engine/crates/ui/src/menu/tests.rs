use super::*;

fn model() -> MenuModel {
    MenuModel {
        class_picker_available: false,
        class_choices: None,
        cheats: vec![
            ("invincible", "Invincibility", false),
            ("ammo", "Infinite ammo", true),
        ],
        assist_intensity: 1.0,
        team_size_pct: 100,
        reinforcements_pct: 100,
        population_note: Default::default(),
        team_size_off: false,
        fov_scale: 1.0,
        fov_deg: 37.7,
        music_volume: 1.0,
        mouse_sensitivity: 1.0,
        pad_look_sensitivity: 1.0,
        invert_y: false,
        low_power_resolution: false,
        input_icons_label: "Auto",
        icons: Default::default(),
        first_person: false,
        cursor: 0,
        page: None,
        popup: None,
        restart_available: false,
        texts: Default::default(),
        back_steps_out: false,
        respawn_label: Some("Respawn".into()),
        outcome_line: None,
        in_shell: None,
    }
}

/// Both menus carry the row. Cross steps on, as a paused pad has no
/// Left / Right for rows; Left steps back.
#[test]
fn the_input_icons_row_steps_its_choices() {
    let mut m = model();
    let at = |m: &MenuModel| rows(m).iter().position(|r| *r == Row::InputIcons);
    m.cursor = at(&m).unwrap();
    let press = |m: &MenuModel, input| navigate(m, input).1;
    let confirm = MenuInput {
        confirm: true,
        ..Default::default()
    };
    assert_eq!(press(&m, confirm), vec![MenuAction::CycleInputIcons(1)]);
    let left = MenuInput {
        left: true,
        ..Default::default()
    };
    assert_eq!(press(&m, left), vec![MenuAction::CycleInputIcons(-1)]);
    let right = MenuInput {
        right: true,
        ..Default::default()
    };
    assert_eq!(press(&m, right), vec![MenuAction::CycleInputIcons(1)]);
    m.in_shell = Some("Game Options".into());
    assert!(at(&m).is_some());
}

/// The text a settled frame of the menu shows: (what it cuts, what it shows
/// whole, the shown text's bounds).
fn settled_text(m: &MenuModel, size: [f32; 2]) -> (Vec<String>, Vec<String>, egui::Rect) {
    let ctx = egui::Context::default();
    let mut first = m.clone();
    first.cursor = 0;
    let mut out = None;
    // The cursor moves there as a player's does.
    for frame in 0..8 {
        let shown = if frame < 3 { &first } else { m };
        out = Some(menu_frame(&ctx, shown, size, Vec::new()));
    }
    text_of(out.unwrap())
}

/// One frame of the menu; `events` are what the player did in it.
fn menu_frame(
    ctx: &egui::Context,
    m: &MenuModel,
    size: [f32; 2],
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            draw_menu(ctx, m);
        },
    )
}

/// A frame's text: (what it cuts, what it shows whole, the shown text's bounds).
fn text_of(out: egui::FullOutput) -> (Vec<String>, Vec<String>, egui::Rect) {
    let (mut cut, mut whole) = (Vec::new(), Vec::new());
    let mut bounds = egui::Rect::NOTHING;
    for c in out.shapes {
        if let egui::Shape::Text(t) = &c.shape {
            let rect = t.visual_bounding_rect();
            if !rect.intersects(c.clip_rect) {
                continue;
            }
            bounds = bounds.union(rect.intersect(c.clip_rect));
            let list = if c.clip_rect.contains_rect(rect.shrink(1.0)) {
                &mut whole
            } else {
                &mut cut
            };
            list.push(t.galley.text().to_owned());
        }
    }
    (cut, whole, bounds)
}

/// Any list length in any window: no row is shown cut, the cursor's row is
/// shown whole, and the panel and its help stay on the window.
#[test]
fn the_pause_rows_show_whole_at_any_window() {
    const CHEATS: [(&str, &str, bool); 12] = [("c", "A cheat", false); 12];
    let mut m = model();
    m.population_note = [
        "Average match: 28 a side\nSmallest map: 12 a side".to_owned(),
        "Average match: 200 reinforcements\nSmallest map: 120".to_owned(),
    ];
    for cheats in [0, 2, 6, 12] {
        m.cheats = CHEATS[..cheats].to_vec();
        for picker in [false, true] {
            m.class_picker_available = picker;
            for h in [
                400.0, 480.0, 540.0, 600.0, 720.0, 768.0, 805.0, 900.0, 1080.0,
            ] {
                for (cursor, row) in rows(&m).into_iter().enumerate() {
                    m.cursor = cursor;
                    let (cut, whole, bounds) = settled_text(&m, [1280.0, h]);
                    let at = format!("{cheats} cheats, {h} high, on {row:?}");
                    assert!(cut.is_empty(), "{at}: {cut:?} cut");
                    assert!(
                        bounds.top() >= 0.0 && bounds.bottom() <= h,
                        "{at}: {bounds:?}"
                    );
                    let label = match row {
                        Row::Respawn => Some("Respawn"),
                        Row::InputIcons => Some("Button prompts"),
                        Row::Quit => Some("Quit to desktop"),
                        _ => None,
                    };
                    if let Some(label) = label {
                        assert!(whole.iter().any(|t| t == label), "{at}: {whole:?}");
                    }
                }
            }
        }
    }
}

/// A mouse wheel's notch moves the list and a slow trackpad's small steps
/// add up; either way it comes to rest on a whole row.
#[test]
fn a_scrolled_list_moves_and_settles_on_a_row() {
    let wheel = |unit, delta| egui::Event::MouseWheel {
        unit,
        delta: egui::vec2(0.0, delta),
        modifiers: egui::Modifiers::NONE,
    };
    let notch = wheel(egui::MouseWheelUnit::Line, -1.0);
    let step = wheel(egui::MouseWheelUnit::Point, -2.0);
    let size = [1280.0, 480.0];
    let m = model();
    for (name, event, every, frames) in [("wheel", notch, 1000, 1), ("trackpad", step, 3, 120)] {
        let ctx = egui::Context::default();
        for _ in 0..4 {
            menu_frame(&ctx, &m, size, Vec::new());
        }
        let over = egui::Event::PointerMoved(egui::pos2(640.0, 240.0));
        menu_frame(&ctx, &m, size, vec![over]);
        let before = text_of(menu_frame(&ctx, &m, size, Vec::new())).1;
        for frame in 0..frames {
            let events = if frame % every == 0 {
                vec![event.clone()]
            } else {
                Vec::new()
            };
            menu_frame(&ctx, &m, size, events);
        }
        for _ in 0..120 {
            menu_frame(&ctx, &m, size, Vec::new());
        }
        let (cut, whole, _) = text_of(menu_frame(&ctx, &m, size, Vec::new()));
        assert_ne!(before, whole, "{name}: the list did not move");
        assert!(cut.is_empty(), "{name}: {cut:?} cut");
    }
}

/// A tall window shows the whole list, with nothing left to scroll.
#[test]
fn a_tall_window_shows_every_pause_row() {
    let mut m = model();
    m.cursor = rows(&m).iter().position(|r| *r == Row::InputIcons).unwrap();
    for h in [805.0, 1080.0] {
        let (_, whole, _) = settled_text(&m, [1920.0, h]);
        for label in ["Resume", "Respawn", "Quit to desktop"] {
            assert!(whole.iter().any(|t| t == label), "{h}: {label}");
        }
    }
}

/// The row says what it is for, in both menus.
#[test]
fn the_button_prompts_row_explains_itself() {
    let mut m = model();
    let want = "Which button icons the game shows. Auto follows the controller you are using.";
    assert_eq!(row_help(&m, Row::InputIcons).as_deref(), Some(want));
    m.in_shell = Some("Game Options".into());
    assert_eq!(row_help(&m, Row::InputIcons).as_deref(), Some(want));
}

#[test]
fn team_size_row_explains_averages_and_greys_where_it_cannot_act() {
    let mut m = model();
    m.population_note = ["A\nL".into(), "R\nL".into()];
    m.reinforcements_pct = 150;
    assert!(rows(&m).contains(&Row::AiCap));
    assert_eq!(row_help(&m, Row::AiCap).as_deref(), Some("A\nL"));
    assert_eq!(
        row_help(&m, Row::Reinforcements).as_deref(),
        Some("R\nL\nChanges apply from the next round.")
    );
    assert_eq!(row_help(&m, Row::Fov), None);
    assert_eq!(
        population_row(&m, Row::AiCap),
        Some(("Team size", "100%".into()))
    );
    assert_eq!(
        population_row(&m, Row::Reinforcements),
        Some(("Reinforcements", "150%".into()))
    );
    assert_eq!(row_locked(&m, Row::AiCap), None);
    // Over, the round keeps its teams: the value is the next round's.
    m.outcome_line = Some("Round over".into());
    assert_eq!(
        row_help(&m, Row::AiCap).as_deref(),
        Some("A\nL\nChanges apply from the next round.")
    );
    m.outcome_line = None;
    m.team_size_off = true;
    assert!(rows(&m).contains(&Row::AiCap));
    assert!(row_locked(&m, Row::AiCap).is_some());
    assert_eq!(
        row_help(&m, Row::AiCap).as_deref(),
        row_locked(&m, Row::AiCap)
    );
    assert!(rows(&m).contains(&Row::Reinforcements));
    assert_eq!(row_locked(&m, Row::Reinforcements), None);
    // The shell's settings page sets the next battles: both rows act.
    m.in_shell = Some("Game Options".into());
    assert!(rows(&m).contains(&Row::AiCap));
    assert_eq!(row_locked(&m, Row::AiCap), None);
    assert_eq!(row_help(&m, Row::Reinforcements).as_deref(), Some("R\nL"));
}

/// Issue #142: a battle without map team sizes shows Team size greyed with
/// its reason; no key, stick or click changes it.
#[test]
fn a_battle_without_team_sizes_greys_the_team_size_row() {
    let mut m = model();
    m.team_size_off = true;
    let at = rows(&m).iter().position(|r| *r == Row::AiCap).unwrap();
    m.cursor = at;
    for right in [false, true] {
        let input = MenuInput {
            left: !right,
            right,
            ..Default::default()
        };
        assert!(navigate(&m, input).1.is_empty());
        assert!(step(&m, at, right).is_empty());
    }
    // The cursor still lands on it and moves on.
    let down = MenuInput {
        down: true,
        ..Default::default()
    };
    assert_eq!(navigate(&m, down).0, at + 1);
    let (_, whole, _) = settled_text(&m, [1280.0, 900.0]);
    assert!(whole.iter().any(|t| t == "Team size"), "{whole:?}");
    let reason = row_locked(&m, Row::AiCap).unwrap();
    assert!(whole.iter().any(|t| t == reason), "{whole:?}");
}

/// The Game Menu's stack is the original's width (260 PS2 safe-area units at
/// the shell's safe-area scale, re-bf2-menus.md "ifs_pausemenu"), and rows
/// sit with the pause menu's ySpacing 5 gap: a row's drawn box spans the
/// stack, and consecutive rows' boxes are `row height + 5` apart.
#[test]
fn the_game_menu_draws_the_stack_at_its_width() {
    let m = MenuModel {
        page: Some(PausePage::Main),
        respawn_label: Some("Respawn".into()),
        restart_available: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    // Settle the list's measured pitch over a few frames.
    let mut out = None;
    for _ in 0..8 {
        out = Some(menu_frame(&ctx, &m, [1280.0, 900.0], Vec::new()));
    }
    // Row boxes: the filled frames under the focused row and the panel's
    // row lines; measure from the text rows' bounds per frame.
    let out = out.unwrap();
    let mut tops = Vec::new();
    for c in &out.shapes {
        if let egui::Shape::Text(t) = &c.shape {
            let r = t.visual_bounding_rect();
            if c.clip_rect.contains_rect(r.shrink(1.0)) {
                tops.push((r.min.x, r.min.y));
            }
        }
    }
    // The row labels all start at the same left edge inside the stack, and
    // the stack is at least PAUSE_STACK_WIDTH wide: a value row's arrows sit
    // at its right end, so the widest drawn row spans it.
    let left = tops.iter().map(|(x, _)| *x).fold(f32::MAX, f32::min);
    let right = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.visual_bounding_rect().max.x),
            _ => None,
        })
        .fold(f32::MIN, f32::max);
    assert!(
        right - left >= PAUSE_STACK_WIDTH - 1.0,
        "width {} < stack",
        right - left
    );
    // Two consecutive row labels sit one pitch apart: text height + the gap.
    tops.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let first = tops[0].1;
    let second = tops.iter().find(|(_, y)| *y > first + 1.0).map(|(_, y)| *y);
    if let Some(second) = second {
        let text_h = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.visual_bounding_rect().height()),
                _ => None,
            })
            .next()
            .unwrap();
        // The row's frame adds its margins; the gap between rows is 5, so
        // pitch >= text height + gap. Rows are uniform, so every pair matches.
        let pitch = second - first;
        assert!(
            pitch >= text_h + PAUSE_ROW_GAP,
            "pitch {pitch} < text {text_h} + gap"
        );
    }
}

/// Issue #142: the controller look row steps by tenths in its range, in a
/// round and in the shell, and says what it does.
#[test]
fn the_pad_look_row_steps_and_clamps() {
    let mut m = model();
    for shell in [None, Some("Game Options".to_owned())] {
        m.in_shell = shell;
        let at = rows(&m)
            .iter()
            .position(|r| *r == Row::PadLookSensitivity)
            .expect("the row is in both menus");
        assert_eq!(
            rows(&m)[at - 1],
            Row::MouseSensitivity,
            "beside the mouse's"
        );
        m.pad_look_sensitivity = 1.0;
        assert_eq!(
            step(&m, at, true),
            vec![MenuAction::SetPadLookSensitivity(1.1)]
        );
        assert_eq!(
            step(&m, at, false),
            vec![MenuAction::SetPadLookSensitivity(0.9)]
        );
        m.pad_look_sensitivity = PAD_LOOK_SENSITIVITY_RANGE.0;
        assert_eq!(
            step(&m, at, false),
            vec![MenuAction::SetPadLookSensitivity(0.5)]
        );
        m.pad_look_sensitivity = PAD_LOOK_SENSITIVITY_RANGE.1;
        assert_eq!(
            step(&m, at, true),
            vec![MenuAction::SetPadLookSensitivity(1.5)]
        );
        assert!(row_help(&m, Row::PadLookSensitivity).is_some());
    }
}

#[test]
fn the_mouse_rows_step_the_sensitivity_and_toggle_invert() {
    let mut m = model();
    let at = |m: &MenuModel, r: Row| rows(m).iter().position(|x| *x == r).unwrap();
    m.cursor = at(&m, Row::MouseSensitivity);
    let go = |m: &MenuModel, input: MenuInput| navigate(m, input).1;
    let right = MenuInput {
        right: true,
        ..Default::default()
    };
    let left = MenuInput {
        left: true,
        ..Default::default()
    };
    assert_eq!(go(&m, right), vec![MenuAction::SetMouseSensitivity(1.1)]);
    m.mouse_sensitivity = 0.1;
    assert_eq!(go(&m, left), vec![MenuAction::SetMouseSensitivity(0.1)]);
    m.mouse_sensitivity = 5.0;
    assert_eq!(go(&m, right), vec![MenuAction::SetMouseSensitivity(5.0)]);
    m.cursor = at(&m, Row::InvertY);
    let confirm = MenuInput {
        confirm: true,
        ..Default::default()
    };
    assert_eq!(go(&m, confirm), vec![MenuAction::ToggleInvertY]);
    m.cursor = at(&m, Row::LowPowerResolution);
    assert_eq!(go(&m, confirm), vec![MenuAction::ToggleLowPowerResolution]);
    assert_eq!(rows(&m).last(), Some(&Row::Quit));
}

#[test]
fn respawn_confirmation_navigation_and_shell_exclusion() {
    let mut m = model();
    m.cursor = rows(&m).iter().position(|r| *r == Row::Respawn).unwrap();
    assert_eq!(
        navigate(
            &m,
            MenuInput {
                confirm: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::RequestRespawn]
    );
    m.popup = Some(Popup::Respawn);
    m.cursor = 0;
    assert_eq!(rows(&m), vec![Row::No, Row::Yes]);
    assert_eq!(
        navigate(
            &m,
            MenuInput {
                confirm: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::CancelPopup]
    );
    assert_eq!(
        navigate(
            &m,
            MenuInput {
                down: true,
                confirm: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::Respawn]
    );
    assert_eq!(
        navigate(
            &m,
            MenuInput {
                right: true,
                confirm: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::Respawn]
    );
    m.cursor = 1;
    // Back from the popup resumes (playtest A3); No is its cancel.
    assert_eq!(
        navigate(
            &m,
            MenuInput {
                back: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::Resume]
    );
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        draw_menu(ctx, &m);
    });
    m.popup = None;
    m.in_shell = Some("Options".into());
    assert!(!rows(&m).contains(&Row::Respawn));
    m.in_shell = None;
    m.respawn_label = None;
    assert!(!rows(&m).contains(&Row::Respawn));
}

#[test]
fn navigation_wraps_toggles_and_adjusts() {
    let mut m = model();
    // Down twice → second cheat; confirm toggles it.
    let (c, a) = navigate(
        &m,
        MenuInput {
            down: true,
            ..Default::default()
        },
    );
    m.cursor = c;
    let (c, a2) = navigate(
        &m,
        MenuInput {
            down: true,
            confirm: true,
            ..Default::default()
        },
    );
    assert!(a.is_empty());
    assert_eq!((c, a2), (2, vec![MenuAction::ToggleCheat(1)]));
    // Up from the top wraps to Quit.
    m.cursor = 0;
    let (c, _) = navigate(
        &m,
        MenuInput {
            up: true,
            ..Default::default()
        },
    );
    assert_eq!(rows(&m)[c], Row::Quit);
    // Slider rows adjust with left/right and clamp.
    m.cursor = 3; // Assist
    let (_, a) = navigate(
        &m,
        MenuInput {
            right: true,
            ..Default::default()
        },
    );
    assert_eq!(a, vec![MenuAction::SetAssist(1.1)]);
    m.cursor = 4; // AiCap
    m.team_size_pct = 10;
    let (_, a) = navigate(
        &m,
        MenuInput {
            left: true,
            ..Default::default()
        },
    );
    assert_eq!(a, vec![MenuAction::SetAiCap(10)]);
    m.cursor = 6; // Fov
    assert_eq!(rows(&m)[6], Row::Fov);
    let (_, a) = navigate(
        &m,
        MenuInput {
            right: true,
            ..Default::default()
        },
    );
    assert_eq!(a, vec![MenuAction::SetFov(1.05)]);
    // The music volume: tenths, held in 0..1.
    m.cursor = 7;
    assert_eq!(rows(&m)[7], Row::Music);
    let step = |m: &MenuModel, right: bool| {
        let input = MenuInput {
            left: !right,
            right,
            ..Default::default()
        };
        navigate(m, input).1
    };
    assert_eq!(step(&m, true), vec![MenuAction::SetMusic(1.0)]);
    assert_eq!(step(&m, false), vec![MenuAction::SetMusic(0.9)]);
    m.music_volume = 0.1;
    assert_eq!(step(&m, false), vec![MenuAction::SetMusic(0.0)]);
    m.music_volume = 0.0;
    assert_eq!(step(&m, false), vec![MenuAction::SetMusic(0.0)]);
    // Back always resumes.
    let (_, a) = navigate(
        &m,
        MenuInput {
            back: true,
            ..Default::default()
        },
    );
    assert_eq!(a, vec![MenuAction::Resume]);
    // Draws headless.
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        draw_menu(ctx, &m);
    });
}

/// Playtest A3: Back (B / Circle, Backspace) resumes from every row.
#[test]
fn back_resumes_from_every_pause_row() {
    let mut m = model();
    let back = MenuInput {
        back: true,
        ..Default::default()
    };
    for cursor in 0..rows(&m).len() {
        m.cursor = cursor;
        assert_eq!(
            navigate(&m, back).1,
            vec![MenuAction::Resume],
            "row {cursor}"
        );
    }
    // A space battle keeps Back as the popup's cancel; its rows still resume.
    m.back_steps_out = true;
    m.cursor = 0;
    assert_eq!(navigate(&m, back).1, vec![MenuAction::Resume]);
    m.popup = Some(Popup::Respawn);
    assert_eq!(navigate(&m, back).1, vec![MenuAction::CancelPopup]);
}

/// A value row's arrows sit clear of its label and inside the panel, on the
/// longest label too.
#[test]
fn the_value_arrows_clear_their_labels() {
    let mut m = model();
    m.fov_deg = 120.0;
    m.fov_scale = 1.25;
    // Each value its own text.
    (m.mouse_sensitivity, m.pad_look_sensitivity) = (2.5, 0.7);
    m.reinforcements_pct = 150;
    let ctx = egui::Context::default();
    let mut out = None;
    for _ in 0..4 {
        out = Some(menu_frame(&ctx, &m, [1280.0, 1080.0], Vec::new()));
    }
    let out = out.unwrap();
    let text_rect = |want: &str| {
        out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == want => Some(t.visual_bounding_rect()),
            _ => None,
        })
    };
    for (i, row) in rows(&m).into_iter().enumerate() {
        if !is_value_row(row) {
            continue;
        }
        let (label, value) = row_text(&m, row);
        let label = text_rect(&label).unwrap_or_else(|| panic!("{row:?} label"));
        let value = text_rect(&value.unwrap()).unwrap_or_else(|| panic!("{row:?} value"));
        let left = ctx.read_response(step_id(i, false)).unwrap().rect;
        let right = ctx.read_response(step_id(i, true)).unwrap().rect;
        assert!(label.right() < left.left(), "{row:?}: {label:?} {left:?}");
        assert!(left.right() <= value.left(), "{row:?}: {left:?} {value:?}");
        assert!(
            value.right() <= right.left(),
            "{row:?}: {value:?} {right:?}"
        );
    }
}
