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
    for _ in 0..16 {
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
