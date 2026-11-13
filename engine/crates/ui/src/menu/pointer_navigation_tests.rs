use super::*;

#[test]
fn tab_focus_then_enter_does_not_dispatch_a_second_menu_action() {
    let ctx = egui::Context::default();
    let mut model = MenuModel::default();
    model.cursor = rows(&model)
        .iter()
        .position(|r| *r == Row::InvertY)
        .unwrap();
    let frame = |events| {
        let mut actions = Vec::new();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 720.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| actions = draw_menu(ctx, &model),
        );
        actions
    };
    frame(vec![]);
    frame(vec![]);
    let mut focused = false;
    // Enough presses to pass through every row's widgets, the arrows too.
    for _ in 0..48 {
        frame(vec![key(egui::Key::Tab, true)]);
        frame(vec![key(egui::Key::Tab, false)]);
        focused |= ctx.memory(|m| m.focused().is_some());
        let overlay = frame(vec![key(egui::Key::Enter, true)]);
        assert!(
            overlay.is_empty(),
            "egui must not duplicate native navigation: {overlay:?}"
        );
        frame(vec![key(egui::Key::Enter, false)]);
        assert_eq!(
            navigate(
                &model,
                MenuInput {
                    confirm: true,
                    ..Default::default()
                }
            )
            .1,
            vec![MenuAction::ToggleInvertY]
        );
    }
    assert!(focused, "exercise real egui Tab focus");
}

fn key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Default::default(),
    }
}

#[test]
fn entire_row_accepts_pointer_including_binding_value_and_padding() {
    let ctx = egui::Context::default();
    let mut response = None;
    let mut frame = |events| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 720.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    response = Some(row_label(
                        ui,
                        Some(1.0),
                        "Invert mouse Y".into(),
                        Some("OFF".into()),
                    ));
                });
            },
        );
        response.take().unwrap()
    };
    let first = frame(vec![]);
    // Far-right row padding/value, not just the action's text, accepts clicks.
    let pos = first.rect.right_center() - egui::vec2(4.0, 0.0);
    frame(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    ]);
    let r = frame(vec![egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    assert!(r.clicked_by(egui::PointerButton::Primary));
}

/// One frame of the menu at 1280 x 900 with `events`; its actions.
fn menu_frame(ctx: &egui::Context, m: &MenuModel, events: Vec<egui::Event>) -> Vec<MenuAction> {
    let mut actions = Vec::new();
    let _ = ctx.run(
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
    actions
}

/// A primary click at `pos` over three frames; the actions of each.
fn click(ctx: &egui::Context, m: &MenuModel, pos: egui::Pos2) -> Vec<MenuAction> {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let mut out = menu_frame(ctx, m, vec![egui::Event::PointerMoved(pos), button(true)]);
    out.extend(menu_frame(ctx, m, vec![button(false)]));
    out.extend(menu_frame(ctx, m, vec![]));
    out
}

/// Issue #142: every value row steps with the mouse, in a round and on the
/// shell's settings page: ▶ as Right does, ◀ as Left.
#[test]
fn every_value_row_steps_by_pointer() {
    let base = MenuModel {
        assist_intensity: 1.0,
        team_size_pct: 100,
        reinforcements_pct: 100,
        fov_scale: 1.0,
        music_volume: 0.5,
        mouse_sensitivity: 1.0,
        pad_look_sensitivity: 1.0,
        rumble_strength: 0.67,
        input_icons_label: "Auto",
        respawn_label: Some("Respawn".into()),
        ..Default::default()
    };
    let shell = MenuModel {
        in_shell: Some("Game Options".into()),
        respawn_label: None,
        ..base.clone()
    };
    let value_rows = [
        Row::Assist,
        Row::AiCap,
        Row::Reinforcements,
        Row::Fov,
        Row::Music,
        Row::MouseSensitivity,
        Row::PadLookSensitivity,
        Row::FlightControls,
        Row::Rumble,
        Row::CareerAwards,
        Row::InputIcons,
    ];
    for m in [base, shell] {
        let rows = rows(&m);
        for row in value_rows {
            let i = rows.iter().position(|r| *r == row).unwrap();
            for right in [true, false] {
                let ctx = egui::Context::default();
                for _ in 0..3 {
                    menu_frame(&ctx, &m, vec![]);
                }
                let arrow = ctx
                    .read_response(step_id(i, right))
                    .unwrap_or_else(|| panic!("{row:?} has its arrows"));
                let want = step(&m, i, right);
                assert!(!want.is_empty(), "{row:?}");
                assert_eq!(
                    click(&ctx, &m, arrow.rect.center()),
                    want,
                    "{row:?} right {right} in shell {:?}",
                    m.in_shell
                );
            }
        }
    }
}

/// A greyed row has no arrows, and a click anywhere on it does nothing.
#[test]
fn a_greyed_row_takes_no_click() {
    let m = MenuModel {
        team_size_pct: 100,
        team_size_off: true,
        respawn_label: Some("Respawn".into()),
        ..Default::default()
    };
    let i = rows(&m).iter().position(|r| *r == Row::AiCap).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        menu_frame(&ctx, &m, vec![]);
    }
    assert!(ctx.read_response(step_id(i, true)).is_none());
    assert!(ctx.read_response(step_id(i, false)).is_none());
    // The row sits between its neighbours' arrows: click across its width.
    let above = ctx.read_response(step_id(i - 1, true)).unwrap().rect;
    let below = ctx.read_response(step_id(i + 1, true)).unwrap().rect;
    let y = (above.center().y + below.center().y) / 2.0;
    for x in [
        above.center().x,
        above.center().x - 160.0,
        above.center().x - 330.0,
    ] {
        assert_eq!(click(&ctx, &m, egui::pos2(x, y)), vec![], "at {x}");
    }
}

/// Every text drawn in one frame of the menu with `events`.
fn frame_texts(ctx: &egui::Context, m: &MenuModel, events: Vec<egui::Event>) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            draw_menu(ctx, m);
        },
    );
    let mut out = Vec::new();
    for c in &output.shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// Issue #142: a mouse player reads a greyed row's reason by pointing at it,
/// and the controller look help shows the same way; the cursor row's help
/// comes back once the pointer leaves.
#[test]
fn a_hovered_row_shows_its_help() {
    let m = MenuModel {
        team_size_pct: 100,
        team_size_off: true,
        pad_look_sensitivity: 1.0,
        respawn_label: Some("Respawn".into()),
        ..Default::default()
    };
    let rows = rows(&m);
    let locked = rows.iter().position(|r| *r == Row::AiCap).unwrap();
    let pad = rows
        .iter()
        .position(|r| *r == Row::PadLookSensitivity)
        .unwrap();
    let why = row_locked(&m, Row::AiCap).unwrap();
    let pad_help = row_help(&m, Row::PadLookSensitivity).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        menu_frame(&ctx, &m, vec![]);
    }
    let shows = |texts: &[String], s: &str| texts.iter().any(|t| t == s);
    // The cursor is on Resume: no help yet.
    let idle = frame_texts(&ctx, &m, vec![]);
    assert!(!shows(&idle, why) && !shows(&idle, &pad_help));
    // The greyed row: between its neighbours' arrows, over its label.
    let above = ctx.read_response(step_id(locked - 1, true)).unwrap().rect;
    let below = ctx.read_response(step_id(locked + 1, true)).unwrap().rect;
    let at = egui::pos2(
        above.center().x - 330.0,
        (above.center().y + below.center().y) / 2.0,
    );
    frame_texts(&ctx, &m, vec![egui::Event::PointerMoved(at)]);
    assert!(
        shows(&frame_texts(&ctx, &m, vec![]), why),
        "greyed row reason"
    );
    // The controller look row's arrow.
    let arrow = ctx.read_response(step_id(pad, true)).unwrap().rect.center();
    frame_texts(&ctx, &m, vec![egui::Event::PointerMoved(arrow)]);
    assert!(
        shows(&frame_texts(&ctx, &m, vec![]), &pad_help),
        "pad look help"
    );
    // Off the menu: the cursor row's (none) again.
    frame_texts(
        &ctx,
        &m,
        vec![egui::Event::PointerMoved(egui::pos2(5.0, 5.0))],
    );
    let gone = frame_texts(&ctx, &m, vec![]);
    assert!(!shows(&gone, why) && !shows(&gone, &pad_help));
}
