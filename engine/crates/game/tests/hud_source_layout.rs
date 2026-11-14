//! Stock source-layout parity, independent of edition-specific mesh/font art.
//! Opt in with four real HUD_LAYOUT_ASSETS roots and owned HUD_LAYOUT_OUT.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)
use std::{collections::BTreeMap, path::Path};
use ui::hud_tree::{Elem, Hud, Kind};
#[path = "support/hud_layout.rs"]
mod support;

fn read<T: serde::de::DeserializeOwned>(p: &Path) -> T {
    ron::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}
fn load(root: &Path) -> Hud {
    let mut hud = Hud::new(
        &read(&root.join("common_bf2/hud/1playerhud.ron")),
        &read(&root.join("common_bf2/hud/hudtransforms.ron")),
    );
    ui::hud::place_countdown_labels(&mut hud);
    hud
}
fn key(hud: &Hud, e: &Elem) -> String {
    if e.is_segment {
        format!("{}/{}", hud.elems[e.parent.unwrap()].name, e.name)
    } else {
        e.name.clone()
    }
}
fn geometry(e: &Elem, y: Option<f32>) -> String {
    let mut position = e.pos;
    if let Some(y) = y {
        position.y = y;
    }
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
        e.kind,
        position,
        e.rot,
        e.scale,
        e.rect,
        e.h_align,
        e.v_align,
        e.angle_start,
        e.angle_end,
        e.segments,
        e.circular
    )
}

fn message_pitch(root: &Path, hud: &Hud) -> f32 {
    let elem = &hud.elems[hud.find("player1systemMessages").unwrap()];
    let font: manifest::hud::HudFont =
        read(&root.join(format!("common_bf2/fonts/{}.ron", elem.font)));
    let line = f32::from(font.line_height) * elem.text_scale[1];
    elem.rect.h / (elem.rect.h / line).floor().max(1.0)
}

fn cap_frames(root: &Path) -> BTreeMap<String, [f32; 4]> {
    let mut hud = load(root);
    let index: manifest::hud::HudIndex = read(&root.join("common_bf2/hud/index.ron"));
    let mut art = ui::hud_draw::HudArt::default();
    for name in index.fonts {
        let font: manifest::hud::HudFont = read(&root.join(format!("common_bf2/fonts/{name}.ron")));
        art.fonts.insert(name, font);
    }
    // The measuring glyph and forced visibility isolate geometry from text,
    // localization and event timing. Actual gameplay visibility is tested below.
    for elem in &mut hud.elems {
        elem.alpha = 1.0;
        elem.fader = Default::default();
        elem.fader.show();
        if elem.kind == Kind::Text {
            elem.text = "0".into();
        }
        if let Some(lines) = elem.lines.as_deref_mut() {
            let mut fader = ui::hud_tree::Fader::default();
            fader.show();
            lines.shown = vec![ui::hud_tree::Line {
                text: "0".into(),
                color: [255.0; 3],
                y: 0.0,
                fader,
                held: 0.0,
            }];
        }
    }
    let frame = ui::hud_draw::draw(&hud, &art, [640.0, 480.0]);
    let mut result = BTreeMap::new();
    for (i, bounds) in frame.boxes {
        let elem = &hud.elems[i];
        if matches!(elem.kind, Kind::Text | Kind::MultiLineText) {
            result.insert(elem.name.clone(), bounds);
        }
    }
    result
}

#[test]
#[ignore = "requires four chosen-source bakes and owned output"]
fn shared_single_player_geometry_across_four_real_bakes() {
    let roots =
        std::env::split_paths(&std::env::var_os("HUD_LAYOUT_ASSETS").expect("HUD_LAYOUT_ASSETS"))
            .collect::<Vec<_>>();
    assert_eq!(roots.len(), 4, "PS2, PC, Xbox, mixed in that order");
    let output =
        std::path::PathBuf::from(std::env::var_os("HUD_LAYOUT_OUT").expect("owned HUD_LAYOUT_OUT"));
    std::fs::create_dir_all(&output).unwrap();
    let canonical = load(&roots[0]);
    let canonical_pitch = message_pitch(&roots[0], &canonical);
    let canonical_caps = cap_frames(&roots[0]);
    let canonical = canonical
        .elems
        .iter()
        .filter(|e| e.kind != Kind::Other)
        .map(|e| (key(&canonical, e), e))
        .collect::<BTreeMap<_, _>>();
    let mut failures = Vec::new();
    let mut receipts = Vec::new();
    let mut native_reference = std::collections::BTreeMap::new();
    for (profile, root) in ["ps2", "pc", "xbox", "mixed"].into_iter().zip(&roots) {
        let hud = load(root);
        let caps = cap_frames(root);
        let mut cap_count = 0;
        for (name, got) in &caps {
            if let Some(want) = canonical_caps.get(name) {
                cap_count += 1;
                if (got[1] - want[1]).abs() > 0.02 {
                    failures.push(format!(
                        "{profile} {name} measured cap {} != {}",
                        got[1], want[1]
                    ));
                }
            }
        }
        assert!(
            cap_count >= 47,
            "missing shared text measurements: {cap_count}"
        );
        println!("{profile}: {cap_count} native cap origins compared");
        let pitch = message_pitch(root, &hud);
        if pitch != canonical_pitch {
            failures.push(format!(
                "{profile} message pitch {pitch} != {canonical_pitch}"
            ));
        }
        let mut compared = 0;
        let mut meshes = 0;
        for e in hud.elems.iter().filter(|e| e.kind != Kind::Other) {
            let Some(c) = canonical.get(&key(&hud, e)) else {
                continue;
            };
            compared += 1;
            let calibrated = profile == "pc"
                && (matches!(e.kind, Kind::Text | Kind::MultiLineText)
                    || e.name == "player1weapon2backdrop");
            let y = calibrated.then_some(c.pos.y);
            if geometry(e, y) != geometry(c, None) {
                failures.push(format!(
                    "{profile} {}: {} != {}",
                    e.name,
                    geometry(e, y),
                    geometry(c, None)
                ));
            }
            let mut bindings = std::collections::HashSet::new();
            for m in &e.mesh_info {
                if !bindings.insert(&m.mesh) {
                    continue;
                }
                if let Some(c) = c.mesh_info.iter().find(|c| c.mesh == m.mesh) {
                    meshes += 1;
                    if m != c {
                        failures.push(format!("{profile} {} mesh {:?} != {:?}", e.name, m, c));
                    }
                }
            }
        }
        assert!(
            compared > 140 && meshes > 60,
            "inventory unexpectedly empty: {compared}/{meshes}"
        );
        println!("{profile}: {compared} interpreted elements, {meshes} MeshInfo records compared");
        for game in ["bf1", "bf2"] {
            for size in support::SIZES {
                for state in 0..3 {
                    let mut view =
                        game::hud_feed::HudView::load(root, render::HEADLESS_FORMAT, None).unwrap();
                    let model = support::model(root, game, state);
                    support::settle(&mut view, &model, state, size);
                    let bounds = support::ELEMENTS
                        .iter()
                        .map(|n| ((*n).to_owned(), view.bounds(n)))
                        .collect::<BTreeMap<_, _>>();
                    let case = (game, size, state);
                    if profile == "ps2" {
                        native_reference.insert(case, bounds.clone());
                    } else {
                        let reference = &native_reference[&case];
                        let tolerance = 0.02 * size[1] as f32 / 480.0;
                        for name in support::ELEMENTS {
                            if bounds[*name].is_some() != reference[*name].is_some() {
                                failures.push(format!("{profile} {game}/{state}/{size:?} {name} visibility differs from PS2"));
                            }
                        }
                        for name in [
                            "player1health_fill",
                            "player1energy_fill",
                            "player1jetpack_fill",
                            "player1vehiclehealth_fill",
                            "player1weapon1backdrop",
                            "player1weapon2backdrop",
                            "player1statskills.icon",
                            "player1statspoints.icon",
                            "player1targethealth_fill",
                        ] {
                            if let (Some(got), Some(want)) = (bounds[name], reference[name]) {
                                for k in 0..4 {
                                    if (got[k] - want[k]).abs() > tolerance {
                                        failures.push(format!("{profile} {game}/{state}/{size:?} {name}[{k}] {} != {}",got[k],want[k]));
                                    }
                                }
                            }
                        }
                        for name in [
                            "player1weapon1ammo",
                            "player1weapon2ammo",
                            "player1teams_team1reinforcements",
                            "player1teams_team2reinforcements",
                            "player1targetinfo_classNameNEW",
                            "victorytimer",
                            "defeattimer",
                            "player1statskills",
                            "player1statspoints",
                        ] {
                            if let (Some(got), Some(want)) = (bounds[name], reference[name]) {
                                if (got[1] - want[1]).abs() > tolerance {
                                    failures.push(format!(
                                        "{profile} {game}/{state}/{size:?} {name} cap top {} != {}",
                                        got[1], want[1]
                                    ));
                                }
                            }
                        }
                    }
                    if state == 2 {
                        for n in [
                            "player1weapon1ammo",
                            "player1weapon1mesh",
                            "player1health_fill",
                            "player1reticule_bullseye",
                        ] {
                            assert!(view.bounds(n).is_none(), "{profile} disabled {n}");
                        }
                    } else {
                        for n in [
                            "player1health_fill",
                            "player1energy_fill",
                            "player1jetpack_fill",
                            "player1reticule_bullseye",
                        ] {
                            assert!(
                                view.bounds(n).is_some(),
                                "{profile} {game} {state} missing {n}"
                            );
                        }
                        if state == 1 {
                            assert!(
                                view.bounds("player1vehiclehealth_fill").is_some(),
                                "{profile} vehicle health event"
                            );
                            assert!(
                                view.bounds("player1weapon1ammo").is_none(),
                                "{profile} infinite weapon has no count"
                            );
                            assert!(
                                view.bounds("player1weapon1ammoInfinite").is_some(),
                                "{profile} infinite icon"
                            );
                        }
                        let centre = view
                            .map_point(model.map.as_ref().unwrap().eye, true, size)
                            .unwrap();
                        // Projection uses the PS2 Small anchor, including right-edge Hor+.
                        let sx = ui::hud_draw::frame_scale([size[0] as f32, size[1] as f32]);
                        let want = [
                            ui::hud_draw::OVERFILL[0].0 * 4.0 / 3.0 * size[1] as f32
                                + ui::hud_draw::anchor_shift(
                                    0.856385,
                                    [size[0] as f32, size[1] as f32],
                                )
                                + sx[0] * 512.0 * 0.856385,
                            ui::hud_draw::OVERFILL[1].0 * size[1] as f32 + sx[1] * 448.0 * 0.186248,
                        ];
                        for k in 0..2 {
                            if (centre[k] - want[k]).abs() > 0.001 {
                                failures.push(format!(
                                    "{profile} map {size:?}: {centre:?} != {want:?}"
                                ));
                            }
                        }
                        let r = view.bounds("player1reticule_bullseye").unwrap();
                        assert!(((r[0] + r[2]) * 0.5 - size[0] as f32 * 0.5).abs() < 1.0);
                        assert!(
                            ((r[1] + r[3]) * 0.5 - model.reticle_y * size[1] as f32).abs() < 1.0
                        );
                    }
                    receipts.push(serde_json::json!({"profile":profile,"game":game,"size":size,"state":state,"bounds":bounds}));
                }
            }
        }
    }
    std::fs::write(
        output.join("bounds.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
    assert!(
        failures.is_empty(),
        "{} geometry mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
