//! Hoth's command walker marker, selection and live spawn path.
#[path = "../../examples/support/walker_battle.rs"]
mod walker_battle;
use core::sim::{buttons, PadState, ScreenPick, SpawnStage};
use core::unit::SpawnPost;

#[test]
#[ignore = "requires local BF2 Hoth assets and a headless GPU"]
fn hoth_atats_show_their_post_icons_and_spawn_an_imperial_at_the_selected_walker() {
    let root = game::assets::baked_root();
    let (mut walk, scene) = walker_battle::battle_on(&root, "bf2_hot1", "bf2_hot1_con").unwrap();
    let own = walk.sim.unit(walk.player).unwrap().team;
    assert_eq!(walk.teams[own as usize].faction, "imp");
    let walkers: Vec<_> = walk.sim.command_walkers(own).map(|(h, _)| h).collect();
    assert_eq!(walkers.len(), 2);
    let map = game::minimap::MapStatic::load(
        &scene,
        &root.join("classes"),
        &walk,
        &walk.vehicle_classes,
        Some("gcw"),
    );
    let normal = map.model(&walk);
    assert_eq!(
        normal
            .posts
            .iter()
            .filter(|p| p.texture == "atat_icon")
            .count(),
        2
    );
    assert!(!normal.units.iter().any(|p| p.texture == "atat_icon"));
    let target = SpawnPost::Walker(walkers[1]);
    assert!(walk.sim.open_spawn_screens(0, false));
    walk.step_tick(&[PadState {
        pick: Some(ScreenPick::post(target)),
        ..Default::default()
    }]);
    assert_eq!(walk.sim.spawn_post_selected(0), Some(target));
    let spawn = map.spawn_model(&walk, 0);
    let selected = &spawn.posts[spawn.select.unwrap()];
    assert_eq!(
        (&*selected.texture, selected.number, selected.color),
        ("atat_icon", None, ui::hud_map::FRIEND)
    );

    let size = [1280, 720];
    let mut renderer = render::Renderer::new_headless(size[0], size[1]).unwrap();
    let mut hud = game::hud_feed::HudView::for_renderer(&root, &renderer).unwrap();
    let out = root.parent().unwrap().join("artifacts/command-walker-map");
    std::fs::create_dir_all(&out).unwrap();
    for (label, model) in [
        (
            "minimap",
            ui::hud::HudModel {
                map: Some(normal),
                ..Default::default()
            },
        ),
        (
            "spawn",
            ui::hud::HudModel {
                map: Some(spawn),
                spawn: Some(ui::spawn_draw::SpawnPage::Map {
                    objective: "Capture all command posts".into(),
                    legend: ["COMMAND POST".into(), "SPAWN POINT".into(), "PLAYER".into()],
                }),
                ..Default::default()
            },
        ),
    ] {
        hud.update(&model, 1.0, size);
        renderer
            .render_with_overlay(
                &render::Camera::default(),
                &[],
                [0.08, 0.08, 0.08, 1.0],
                Some(&mut |dev, q, enc, view, size| hud.paint(dev, q, enc, view, size)),
            )
            .unwrap();
        assert!(
            hud.drawn_sheets().any(|s| s == "atat_icon"),
            "{label}: no AT-AT ink"
        );
        assert!(
            !hud.unresolved_sheets().any(|s| s == "atat_icon"),
            "{label}: missing AT-AT texture"
        );
        let (rgba, width, height) = renderer.capture_rgba().unwrap();
        std::fs::write(
            out.join(format!("{label}.png")),
            render::encode_png(&rgba, width, height).unwrap(),
        )
        .unwrap();
    }

    // Accept the map and a class through the same input the real UI sends.
    for _ in 0..2 {
        walk.step_tick(&[PadState {
            buttons: buttons::SPAWN_ACCEPT,
            ..Default::default()
        }]);
        walk.step_tick(&[PadState::default()]);
    }
    for _ in 0..60 {
        if walk.sim.unit(walk.player).unwrap().alive {
            break;
        }
        walk.step_tick(&[PadState::default()]);
    }
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Accepted);
    let unit = walk.sim.unit(walk.player).unwrap();
    assert!(unit.alive);
    let vehicle = walk.sim.vehicle(walkers[1]).unwrap();
    assert!(
        (unit.soldier.pos - vehicle.pos).length() < 20.0,
        "spawn {:?}, walker {:?}",
        unit.soldier.pos,
        vehicle.pos
    );
    assert_eq!(walk.sim.spawn_post_selected(0), Some(target));
    eprintln!(
        "Hoth: 2 AT-AT posts rendered; selected walker respawn {:?}",
        unit.soldier.pos
    );
}
