#[path = "../../examples/support/walker_battle.rs"]
mod walker_battle;
use core::sim::{AimTarget, PadState};
use glam::Vec3;

#[test]
#[ignore = "requires owned BF2 Hoth assets and a headless GPU"]
fn shield_generator_has_its_objective_icon_enemy_reticle_and_named_health_bar() {
    let root = game::assets::baked_root();
    let (mut w, scene) = walker_battle::battle_on(&root, "bf2_hot1", "bf2_hot1_con").unwrap();
    let si = w
        .sim
        .stations
        .iter()
        .position(|s| w.sim.station_classes[s.class as usize].name == "hoth_bldg_shieldgenerator")
        .expect("generator health entity");
    assert_eq!(w.sim.stations[si].health.max, 120000.0);
    assert_eq!(w.sim.stations[si].team, Some(1));
    let centre = w.sim.stations[si].sphere_centre;
    assert!(w.sim.stations[si].sphere_radius > 1.0);
    let pi = w.player.index as usize;
    w.sim.units[pi].soldier.pos = centre + Vec3::X * 100.0;
    let at = w.sim.units[pi].soldier.pos;
    w.sim.units[pi].soldier.pos.y = w.sim.heightfield.as_ref().unwrap().height_at(at.x, at.z) + 0.1;
    w.sim.units[pi].soldier.yaw = std::f32::consts::FRAC_PI_2;
    w.sim.units[pi].soldier.pitch = 0.0;
    w.step_tick(&[PadState::default()]);
    eprintln!(
        "generator centre {:?}, radius {}; player {:?}, ray {:?}, gun {} range {}",
        centre,
        w.sim.stations[si].sphere_radius,
        w.sim.units[pi].soldier.pos,
        w.sim.units[pi].aim_ray(w.sim.ground_rules, w.sim.views()),
        w.sim.weapon_classes[w.sim.units[pi].weapon_class].name,
        w.sim.weapon_classes[w.sim.units[pi].weapon_class].extreme_range
    );
    assert_eq!(
        w.sim.reticle_pick[0],
        Some((AimTarget::Station(si as u16), -1))
    );
    assert_ne!(
        w.sim.magnet[0],
        Some(AimTarget::Station(si as u16)),
        "building readout does not pull infantry aim"
    );
    let feed = game::event_feed::EventFeed::for_walk(&root, "bf2", &w);
    let target = feed.target(&w.sim, 0).expect("generator target readout");
    assert_eq!(target.name, "Shield Generator");
    assert_eq!(target.health_fraction, 1.0);
    assert_eq!(target.relation, -1);
    w.sim.units[pi].team = 1;
    w.step_tick(&[PadState::default()]);
    assert_eq!(
        w.sim.reticle_pick[0],
        Some((AimTarget::Station(si as u16), 1)),
        "Rebels see their own generator as friendly"
    );
    w.sim.units[pi].team = 0;
    w.step_tick(&[PadState::default()]);
    let map = game::minimap::MapStatic::load(
        &scene,
        &root.join("classes"),
        &w,
        &w.vehicle_classes,
        Some("gcw"),
    );
    let model = map.model(&w);
    let icon = model
        .posts
        .iter()
        .find(|p| p.texture == "hud_dest_icon")
        .unwrap();
    assert_eq!(
        (icon.scale, icon.color, icon.number),
        (3.0, ui::hud_map::ENEMY, None)
    );
    assert!(!w
        .sim
        .player_spawn_posts(0)
        .iter()
        .any(|(_, p)| (p.x - icon.pos[0]).abs() < 1.0 && (p.z - icon.pos[1]).abs() < 1.0));
    let mut r = render::Renderer::new_headless(1280, 720).unwrap();
    let mut hud = game::hud_feed::HudView::for_renderer(&root, &r).unwrap();
    let out = root.parent().unwrap().join("artifacts/hoth-object-hud");
    std::fs::create_dir_all(&out).unwrap();
    let hud_model = ui::hud::HudModel {
        map: Some(model),
        target: Some(target),
        reticle: Some(-1),
        reticle_y: game::walk::ps2_reticle_y("bf2"),
        ..Default::default()
    };
    hud.update(&hud_model, 1.0, [1280, 720]);
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.12, 0.12, 0.12, 1.0],
        Some(&mut |dev, q, enc, v, size| hud.paint(dev, q, enc, v, size)),
    )
    .unwrap();
    assert!(hud.drawn_sheets().any(|s| s == "hud_dest_icon"));
    assert!(!hud.unresolved_sheets().any(|s| s == "hud_dest_icon"));
    let (rgba, width, height) = r.capture_rgba().unwrap();
    std::fs::write(
        out.join("generator.png"),
        render::encode_png(&rgba, width, height).unwrap(),
    )
    .unwrap();
    w.sim.stations[si].health.cur = 60000.0;
    assert_eq!(feed.target(&w.sim, 0).unwrap().health_fraction, 0.5);
    w.sim.stations[si].destroyed = true;
    assert!(!map
        .model(&w)
        .posts
        .iter()
        .any(|p| p.texture == "hud_dest_icon"));
    eprintln!("Hoth generator: authored icon, 120000 health, enemy target, 50% bar and dead-marker removal");
}

#[test]
#[ignore = "requires owned BF2 Hoth assets and a headless GPU"]
fn empty_hoth_turrets_have_gray_map_icons_outside_the_detection_cone() {
    let root = game::assets::baked_root();
    let (mut w, scene) = walker_battle::battle_on(&root, "bf2_hot1", "bf2_hot1_con").unwrap();
    let turret = w
        .sim
        .vehicles
        .iter()
        .find(|v| v.kind == core::vehicle::VehicleKind::Turret && v.alive)
        .unwrap()
        .pos;
    let pi = w.player.index as usize;
    w.sim.units[pi].soldier.pos = turret + Vec3::X * 30.0;
    w.sim.units[pi].soldier.yaw = 0.0;
    let map = game::minimap::MapStatic::load(
        &scene,
        &root.join("classes"),
        &w,
        &w.vehicle_classes,
        Some("gcw"),
    );
    let model = map.model(&w);
    let icon = model
        .units
        .iter()
        .find(|p| p.texture == "turret_icon" && p.pos == [turret.x, turret.z])
        .expect("neutral turret to the side of the view");
    assert_eq!(icon.color, [128; 3]);
    let icon_pos = icon.pos;
    let mut r = render::Renderer::new_headless(1280, 720).unwrap();
    let mut hud = game::hud_feed::HudView::for_renderer(&root, &r).unwrap();
    hud.update(
        &ui::hud::HudModel {
            map: Some(model),
            ..Default::default()
        },
        1.0,
        [1280, 720],
    );
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.12, 0.12, 0.12, 1.0],
        Some(&mut |dev, q, enc, v, size| hud.paint(dev, q, enc, v, size)),
    )
    .unwrap();
    assert!(hud.drawn_sheets().any(|s| s == "turret_icon"));
    assert!(!hud.unresolved_sheets().any(|s| s == "turret_icon"));
    let (rgba, width, height) = r.capture_rgba().unwrap();
    let out = root.parent().unwrap().join("artifacts/hoth-object-hud");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join("neutral-turrets.png"),
        render::encode_png(&rgba, width, height).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Hoth neutral turret: gray authored marker at {:?}",
        icon_pos
    );
}
