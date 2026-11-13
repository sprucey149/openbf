//! Owned-data witnesses for #50, #114 and #128. No window or live input.
use super::*;
use game::{hud_feed::HudView, walk::WalkSim};
use ui::hud::HudModel;

fn capture(
    assets: &std::path::Path,
    out: &std::path::Path,
    name: &str,
    size: [u32; 2],
    model: &HudModel,
) -> HudView {
    let mut renderer = render::Renderer::new_headless(size[0], size[1]).unwrap();
    let mut view = HudView::for_renderer(assets, &renderer).unwrap();
    view.update(model, 0.0, size);
    view.update(model, 10.0, size);
    renderer
        .render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.02, 0.02, 0.02, 1.0],
            Some(&mut |dev, queue, enc, target, size| view.paint(dev, queue, enc, target, size)),
        )
        .unwrap();
    assert_all_sheets_bound(&view);
    let rgba = renderer.capture_rgba().unwrap().0;
    std::fs::write(
        out.join(format!("{name}_{}x{}.png", size[0], size[1])),
        render::encode_png(&rgba, size[0], size[1]).unwrap(),
    )
    .unwrap();
    view
}

fn mounted(assets: &std::path::Path, map: &str, class: &str) -> WalkSim {
    let dir = assets.join(map);
    let mut scene = game::assets::load_scene(&dir).unwrap();
    scene.apply_mode_layer("con");
    let mut sim = core::sim::Sim::empty(42);
    let build = game::vehicle_pads::install(
        &mut sim,
        &scene.manifest,
        &dir,
        &assets.join("classes"),
        &["rep".into(), "cis".into()],
    )
    .unwrap();
    let ci = build.classes.iter().position(|c| c == class).unwrap();
    let spec = build
        .pads
        .iter()
        .flat_map(|p| p.classes.iter().flatten())
        .find(|c| c.spec.class as usize == ci)
        .unwrap()
        .spec
        .clone();
    let team = game::walk::TeamTemplate::default_trooper();
    let mut walk = WalkSim::with_teams(sim, None, glam::Vec3::ZERO, [team.clone(), team], None);
    walk.add_posts(&game::layout::MapLayout::from_manifest(&scene.manifest));
    walk.vehicle_seats = build.seats;
    walk.vehicle_weapon_hud = build.weapon_hud;
    walk.vehicle_labels = build.labels;
    let h = walk.sim.spawn_vehicle(&spec);
    assert!(walk.sim.seat_unit(walk.player, h, 0));
    let v = walk.sim.vehicle_mut(h).unwrap();
    v.health.cur = v.health.max * 0.6;
    walk
}

fn model(walk: &WalkSim) -> HudModel {
    let me = &walk.sim.units[walk.player.index as usize];
    let [weapon, weapon2] = game::hud_feed::weapon_rows(walk);
    HudModel {
        team_names: ["rep".into(), "cis".into()],
        tickets: [147, 152],
        hp: me.health.cur,
        hp_max: me.health.max,
        energy: 100.0,
        energy_max: 100.0,
        weapon,
        weapon2,
        gauge: game::hud_feed::weapon_gauge(walk),
        reticle: game::hud_feed::armed_seat(walk).then_some(0),
        reticle_y: walk.reticle_y(),
        seat_health: me
            .mount
            .and_then(|m| walk.sim.vehicle(m.vehicle))
            .map(|v| (v.health.cur, v.health.max)),
        dead: !me.alive,
        ..Default::default()
    }
}

fn verify_graphics(out: &std::path::Path) {
    use ui::menu::{MenuAction, MenuModel, PausePage, Row, SettingsPage};
    let base = game::walk::chase_fov_y().to_degrees();
    let cap = game::walk::clamp_fov_scale(2.0);
    for in_shell in [None, Some(String::new())] {
        let mut menu = MenuModel {
            page: Some(PausePage::Settings(SettingsPage::Graphics)),
            in_shell,
            base_fov_deg: base,
            fov_scale: cap,
            ..Default::default()
        };
        let row = ui::menu::rows(&menu)
            .iter()
            .position(|r| *r == Row::Fov)
            .unwrap();
        assert_eq!(ui::menu::step(&menu, row, true), vec![MenuAction::Denied]);
        let reverse = ui::menu::step(&menu, row, false);
        assert!(matches!(reverse.as_slice(), [MenuAction::SetFov(v)] if *v < cap));
        menu.fov_scale = ui::menu::FOV_SCALE_RANGE.0;
        assert_eq!(ui::menu::step(&menu, row, false), vec![MenuAction::Denied]);
        assert!(
            matches!(ui::menu::step(&menu, row, true).as_slice(), [MenuAction::SetFov(v)] if *v > menu.fov_scale)
        );
        assert_eq!(
            ui::menu::press(&menu, Row::ResetGraphics),
            Some(MenuAction::ResetGraphics)
        );
    }
    let path = out.join("old-profile.ron");
    for scale in [0.0, 2.0, f32::NAN] {
        let mut profile = game::profile::Profile::default();
        profile.settings.fov_scale = scale;
        std::fs::write(&path, ron::to_string(&profile).unwrap()).unwrap();
        let loaded = game::profile::Profile::load_from(&path);
        assert_eq!(
            loaded.settings.fov_scale,
            game::walk::clamp_fov_scale(scale)
        );
    }
    let mut settings = game::profile::Settings {
        fov_scale: cap,
        low_power_resolution: true,
        fps_counter: 1,
        viewpoint: game::profile::Settings::default().viewpoint.toggled(),
        music_volume: 0.33,
        ..Default::default()
    };
    let mut expected = settings.clone();
    let defaults = game::profile::Settings::default();
    expected.fov_scale = defaults.fov_scale;
    expected.viewpoint = defaults.viewpoint;
    expected.fps_counter = defaults.fps_counter;
    expected.low_power_resolution = defaults.low_power_resolution;
    settings.reset_graphics();
    assert_eq!(
        settings, expected,
        "reset changes only Graphics preferences"
    );
    let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
    let mut paused = true;
    let mut pause = game::pause::PauseMenu::default();
    game::pause::apply_menu_action(&mut walk, MenuAction::SetFov(2.0), &mut paused, &mut pause);
    assert_eq!(walk.fov_scale, cap);
    assert!((walk.camera().fov_y.to_degrees() - 100.0).abs() < 0.001);
    game::pause::apply_menu_action(
        &mut walk,
        MenuAction::ResetGraphics,
        &mut paused,
        &mut pause,
    );
    assert_eq!(walk.fov_scale, defaults.fov_scale);
    println!("Graphics: shell/pause cap, denied actions, reverse, 3 legacy profile loads and scoped reset verified");
}

#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn capture_playtest_hud() {
    let (assets, out) = paths();
    verify_graphics(&out);
    let mut receipts = Vec::new();
    let mut chainguns = Vec::new();
    let mut bespin = game::assets::load_scene(&assets.join("bf1_bes1")).unwrap();
    bespin.apply_mode_layer("con");
    for size in [[640, 480], [1280, 720]] {
        for victory in [true, false] {
            let model = HudModel {
                countdown: Some((victory, 17.0)),
                team_names: ["rep".into(), "cis".into()],
                tickets: [147, 152],
                hp: 300.0,
                hp_max: 300.0,
                map: Some(game::minimap::still(&bespin, glam::Vec3::ZERO, 0.0)),
                ..Default::default()
            };
            let view = capture(
                &assets,
                &out,
                if victory { "victory" } else { "defeat" },
                size,
                &model,
            );
            for name in ["victorytext", "victorytimer", "defeattext", "defeattimer"] {
                let bounds = view.bounds(name);
                println!("{size:?} {victory} {name}: {bounds:?}");
                receipts.push(serde_json::json!({"size": size, "victory": victory, "element": name, "bounds": bounds}));
            }
        }
        for map in ["bf1_geo1", "bf2_geo1"] {
            let mut walk = mounted(&assets, map, "cis_tread_hailfire");
            walk.sim.spawn_screens = true;
            let mut scene = game::assets::load_scene(&assets.join(map)).unwrap();
            scene.apply_mode_layer("con");
            let mut live = model(&walk);
            live.map = Some(game::minimap::still(&scene, glam::Vec3::ZERO, 0.0));
            let view = capture(&assets, &out, map, size, &live);
            for (row, weapon) in [live.weapon.as_ref(), live.weapon2.as_ref()]
                .into_iter()
                .enumerate()
            {
                let weapon = weapon.expect("armed Hailfire row");
                let mesh = view.weapon_mesh(row as u8 + 1, weapon);
                if mesh.1.as_deref() == Some("hud_vehicle_chaingun") {
                    let bounds = |element| {
                        view.bounds(&format!("player1weapon{}{element}", row + 1))
                            .expect("visible weapon row element")
                    };
                    chainguns.push((
                        map,
                        size,
                        bounds("mesh"),
                        bounds("backdrop"),
                        bounds("ammoInfinite"),
                    ));
                }
                println!("{map} row{}: {weapon:?}, resolved={mesh:?}", row + 1);
                receipts.push(serde_json::json!({"map":map,"row": row+1,"key":weapon.key,"class":weapon.class,"icon":weapon.icon,"resolved":mesh}));
            }
            assert!(view.bounds("player1vehiclehealth_fill").is_some());
            assert!(view.bounds("player1reticule_bullseye").is_some());
            let h = walk.sim.unit(walk.player).unwrap().mount.unwrap().vehicle;
            walk.sim.destroy_vehicle(h, None);
            let mut dead = model(&walk);
            dead.map = live.map.clone();
            assert!(dead.dead);
            assert!(dead.weapon.is_none() && dead.weapon2.is_none() && dead.reticle.is_none());
            capture(&assets, &out, &format!("{map}_dead"), size, &dead);
            for _ in 0..4 * core::TICK_HZ + 2 {
                walk.step_tick(&[core::sim::PadState::default()]);
            }
            assert_eq!(walk.sim.spawn_stage[0], core::sim::SpawnStage::Map);
            let feed = game::event_feed::EventFeed::for_walk(&assets, &map[..3], &walk);
            let page = game::spawn_page::page(&walk, &feed, 0).expect("native respawn map");
            assert!(matches!(page, ui::spawn_draw::SpawnPage::Map { .. }));
            let mut respawn = model(&walk);
            respawn.spawn = Some(page);
            respawn.map = Some(game::minimap::spawn_still(&scene, None));
            capture(&assets, &out, &format!("{map}_respawn"), size, &respawn);
        }
    }
    assert_eq!(chainguns.len(), 4, "one chaingun per map and frame size");
    for (map, size, item, backdrop, ammo) in chainguns {
        assert!(
            item[0] >= ammo[2].max(backdrop[0])
                && item[1] >= backdrop[1]
                && item[2] <= backdrop[2]
                && item[3] <= backdrop[3],
            "{map} {size:?}: chaingun {item:?} must fit on the backdrop {backdrop:?} to the right of its ammo indicator {ammo:?}"
        );
    }
    std::fs::write(
        out.join("playtest-hud.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
    for name in ["victorytext", "victorytimer", "defeattext", "defeattimer"] {
        let bounds = |size| {
            receipts
                .iter()
                .find(|r| r["element"] == name && r["size"][0] == size && !r["bounds"].is_null())
                .unwrap()["bounds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        let narrow = bounds(640);
        let wide = bounds(1280);
        for x in [0, 2] {
            let expected = 1280.0 - (640.0 - narrow[x]) * 1.5;
            assert!(
                (wide[x] - expected).abs() < 0.02,
                "{name} stays at its right-edge offset: {} != {expected}",
                wide[x]
            );
        }
    }
    for bf1 in receipts.iter().filter(|r| r["map"] == "bf1_geo1") {
        let class = bf1["class"].as_str().unwrap();
        let canonical = game::normalize::vehicles::twin_row(class)
            .and_then(|t| t.twin)
            .unwrap_or(class);
        let bf2 = receipts
            .iter()
            .find(|r| r["map"] == "bf2_geo1" && r["class"] == canonical)
            .unwrap();
        assert_eq!(bf1["resolved"], bf2["resolved"], "{class} uses BF2 HUD art");
    }
}
