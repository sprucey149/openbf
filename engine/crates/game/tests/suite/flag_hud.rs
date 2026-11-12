use ui::hud_flag::FlagStatus;

#[test]
fn flag_status_uses_original_hud_art_through_lifecycle() {
    let root = game::assets::baked_root().join("common_bf2/hud");
    let read = |name: &str| -> manifest::hud::HudLayout {
        ron::from_str(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap()
    };
    let mut hud = ui::hud_tree::Hud::new(&read("1playerhud.ron"), &read("hudtransforms.ron"));
    let mut model = ui::hud::HudModel::default();
    let art = ui::hud_draw::HudArt::default();
    let draw = |hud: &mut ui::hud_tree::Hud, model: &ui::hud::HudModel| {
        ui::hud::feed(hud, model, &|_| false);
        hud.advance(1.0);
        ui::hud_draw::draw(hud, &art, [1280.0, 720.0])
            .batches
            .into_iter()
            .map(|b| b.texture)
            .collect::<Vec<_>>()
    };
    model.flag_status = Some(FlagStatus::Carried {
        friendly: false,
        player_first_person: false,
    });
    let enemy = draw(&mut hud, &model);
    assert!(enemy.iter().any(|s| s == "flag_carried"));
    assert!(enemy.iter().any(|s| s == "flag_team_carried"));
    model.flag_status = Some(FlagStatus::Dropped);
    let dropped = draw(&mut hud, &model);
    assert!(dropped.iter().any(|s| s == "flag_dropped"));
    assert!(!dropped.iter().any(|s| s == "flag_carried"));
    model.flag_status = None;
    let returned = draw(&mut hud, &model);
    assert!(!returned.iter().any(|s| s == "flag_dropped"));
    model.flag_status = Some(FlagStatus::Carried {
        friendly: true,
        player_first_person: false,
    });
    let friendly = draw(&mut hud, &model);
    assert!(friendly.iter().any(|s| s == "flag_carried"));
    assert!(friendly.iter().any(|s| s == "flag_team_carried"));
    model.flag_status = Some(FlagStatus::Carried {
        friendly: true,
        player_first_person: true,
    });
    let own = draw(&mut hud, &model);
    assert!(own.iter().any(|s| s == "hud_player_captured_flag"));
}

#[test]
#[ignore]
fn rendered_flag_hud_carrier_and_drop() {
    let root = game::assets::baked_root();
    let mut renderer = render::Renderer::new_headless(1280, 720).unwrap();
    let mut hud = game::hud_feed::HudView::load(
        &root,
        render::HEADLESS_FORMAT,
        renderer.display_view_format(),
    )
    .unwrap();
    let mut model = ui::hud::HudModel::default();
    let out = root.parent().unwrap().join("artifacts/lead_tmp/issue-144");
    std::fs::create_dir_all(&out).unwrap();
    for (label, status, color) in [
        (
            "enemy-carrier",
            Some(FlagStatus::Carried {
                friendly: false,
                player_first_person: false,
            }),
            [175.0, 0.0, 0.0],
        ),
        (
            "friendly-carrier",
            Some(FlagStatus::Carried {
                friendly: true,
                player_first_person: false,
            }),
            [0.0, 0.0, 255.0],
        ),
        ("dropped", Some(FlagStatus::Dropped), [0.0, 0.0, 255.0]),
    ] {
        model.flag_status = status;
        model.hints = if label == "dropped" {
            vec![]
        } else {
            vec![ui::hud_tree::HintMarker {
                ndc: [0.0, 0.0],
                distance: 50.0,
                friendly: label == "friendly-carrier",
                visible: true,
                on_screen: true,
                flag: false,
                carrier: true,
                flyer_view: true,
                own_carrier: true,
                objective: false,
                pulse: None,
                tint: Some(color),
            }]
        };
        hud.update(&model, 1.0, [1280, 720]);
        renderer
            .render_frame(
                &render::Camera::default(),
                &[],
                &[],
                [0.015, 0.015, 0.015, 1.0],
                Some(&mut |dev, queue, enc, view, size| hud.paint(dev, queue, enc, view, size)),
            )
            .unwrap();
        let (rgba, w, h) = renderer.capture_rgba().unwrap();
        std::fs::write(
            out.join(format!("flag-hud-{label}.png")),
            render::encode_png(&rgba, w, h).unwrap(),
        )
        .unwrap();
        assert!(
            rgba.chunks_exact(4).any(|p| p[0] > 80 || p[2] > 80),
            "{label}"
        );
    }
}

#[test]
fn marker_styles_cover_occlusion_offscreen_and_flyer_scale() {
    let root = game::assets::baked_root().join("common_bf2/hud");
    let read = |name: &str| -> manifest::hud::HudLayout {
        ron::from_str(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap()
    };
    let mut hud = ui::hud_tree::Hud::new(&read("1playerhud.ron"), &read("hudtransforms.ron"));
    let art = ui::hud_draw::HudArt::default();
    let mut marker = ui::hud_tree::HintMarker {
        ndc: [0.0, 0.0],
        distance: 50.0,
        friendly: true,
        visible: true,
        on_screen: true,
        flag: false,
        carrier: true,
        flyer_view: true,
        own_carrier: true,
        objective: false,
        pulse: None,
        tint: Some([0.0, 0.0, 255.0]),
    };
    let frame = |hud: &mut ui::hud_tree::Hud, marker| {
        hud.hint_markers = vec![marker];
        ui::hud_draw::draw(hud, &art, [1280.0, 720.0])
    };
    let own = frame(&mut hud, marker);
    let batch = own
        .batches
        .iter()
        .find(|b| b.texture == "hud_target_flag_onscreen")
        .unwrap();
    let width = |frame: &ui::hud_draw::HudFrame, batch: &ui::hud_draw::HudBatch| {
        let points = &frame.indices[batch.first as usize..(batch.first + batch.count) as usize];
        let (lo, hi) = points
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &i| {
                let x = frame.verts[i as usize].pos[0];
                (lo.min(x), hi.max(x))
            });
        hi - lo
    };
    let own_width = width(&own, batch);
    marker.own_carrier = false;
    let ally = frame(&mut hud, marker);
    let batch = ally
        .batches
        .iter()
        .find(|b| b.texture == "hud_target_flag_onscreen")
        .unwrap();
    assert!((own_width / width(&ally, batch) - 1.6).abs() < 0.001);
    marker.carrier = false;
    marker.objective = true;
    marker.flyer_view = false;
    marker.tint = Some([223.0, 223.0, 32.0]);
    let clear = frame(&mut hud, marker);
    assert!(clear
        .batches
        .iter()
        .any(|b| b.texture == "hud_objective_icon"));
    marker.visible = false;
    let hidden = frame(&mut hud, marker);
    assert!(hidden
        .batches
        .iter()
        .any(|b| b.texture == "hud_objective_icon_hidden"));
    marker.on_screen = false;
    let mut other = marker;
    other.carrier = true;
    other.objective = false;
    other.tint = Some([0.0, 0.0, 255.0]);
    hud.hint_markers = vec![marker, other];
    let offscreen = ui::hud_draw::draw(&hud, &art, [1280.0, 720.0]);
    assert_eq!(
        offscreen
            .batches
            .iter()
            .filter(|b| b.texture == "hud_objective_icon_offscreen")
            .map(|b| b.count)
            .sum::<u32>(),
        12
    );
}

#[test]
#[ignore]
fn rendered_flag_marker_occlusion_and_offscreen() {
    let root = game::assets::baked_root();
    let mut renderer = render::Renderer::new_headless(1280, 720).unwrap();
    let mut hud = game::hud_feed::HudView::load(
        &root,
        render::HEADLESS_FORMAT,
        renderer.display_view_format(),
    )
    .unwrap();
    let mut model = ui::hud::HudModel::default();
    let mut target = ui::hud_tree::HintMarker {
        ndc: [0.0, 0.0],
        distance: 50.0,
        friendly: true,
        visible: true,
        on_screen: true,
        flag: true,
        carrier: false,
        flyer_view: true,
        own_carrier: false,
        objective: true,
        pulse: None,
        tint: Some([223.0, 223.0, 32.0]),
    };
    let out = root.parent().unwrap().join("artifacts/lead_tmp/issue-144");
    std::fs::create_dir_all(&out).unwrap();
    for label in ["clear", "occluded", "offscreen"] {
        target.visible = label != "occluded";
        target.on_screen = label != "offscreen";
        target.ndc = if target.on_screen {
            [0.0, 0.0]
        } else {
            [0.9, 0.2]
        };
        model.hints = vec![target];
        if label == "offscreen" {
            let mut carrier = target;
            carrier.ndc = [0.9, -0.2];
            carrier.carrier = true;
            carrier.flag = false;
            carrier.objective = false;
            carrier.tint = Some([0.0, 0.0, 255.0]);
            model.hints.push(carrier);
        }
        hud.update(&model, 1.0, [1280, 720]);
        renderer
            .render_frame(
                &render::Camera::default(),
                &[],
                &[],
                [0.015, 0.015, 0.015, 1.0],
                Some(&mut |dev, queue, enc, view, size| hud.paint(dev, queue, enc, view, size)),
            )
            .unwrap();
        let (rgba, w, h) = renderer.capture_rgba().unwrap();
        std::fs::write(
            out.join(format!("flag-marker-{label}.png")),
            render::encode_png(&rgba, w, h).unwrap(),
        )
        .unwrap();
    }
}
