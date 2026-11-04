use glam::Vec3;

#[test]
#[ignore]
fn bf1_geonosis_atmosphere_moves_and_draws_with_the_shared_systems() {
    let map = game::assets::baked_root().join("bf1_geo1");
    assert!(
        map.join("manifest.ron").exists(),
        "BF1 Geonosis bake required"
    );
    let scene = game::assets::load_battle_scene(&map, "con").unwrap().0;
    let mut renderer = render::Renderer::new_headless(320, 240).unwrap();
    let gpu = game::viewer::upload_scene(&mut renderer, &scene).unwrap();
    let camera = render::Camera {
        position: Vec3::new(95.32, -58.73, 164.56),
        yaw: 0.15,
        pitch: 0.238,
        far: 8000.0,
        ..Default::default()
    };
    let draw = |renderer: &mut render::Renderer| {
        renderer
            .render_frame(
                &camera,
                &gpu.draws,
                &[],
                game::viewer::clear_color(&scene),
                None,
            )
            .unwrap();
    };
    renderer.atmosphere_time = Some(0.0);
    renderer.advance_sky();
    let flights = |renderer: &render::Renderer| {
        renderer
            .sky()
            .iter()
            .filter_map(|shell| match shell.placement {
                render::Placement::Flight(flight) => Some(flight.model()),
                render::Placement::Object { .. } => panic!("static sky object"),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let initial = flights(&renderer);
    assert!(!initial.is_empty());
    for tick in 1..=180 {
        renderer.atmosphere_time = Some(f64::from(tick) / 30.0);
        renderer.advance_sky();
        draw(&mut renderer);
    }
    let moved = flights(&renderer);
    assert_eq!(initial.len(), moved.len());
    assert!(initial
        .iter()
        .zip(&moved)
        .all(|(a, b)| a.w_axis.distance(b.w_axis) > 10.0));
    renderer.advance_sky();
    assert_eq!(flights(&renderer), moved);
    let capture = |renderer: &mut render::Renderer| {
        draw(renderer);
        renderer.capture_rgba().unwrap().0
    };
    let with_atmosphere = capture(&mut renderer);
    renderer.set_dust(Vec::new());
    let without_dust = capture(&mut renderer);
    let changed_pixels = |a: &[u8], b: &[u8]| {
        a.as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count()
    };
    let dust_pixels = changed_pixels(&with_atmosphere, &without_dust);
    assert!(dust_pixels > 0, "placed dust must draw");
    renderer.set_fog_cloud(None);
    let without_cloud = capture(&mut renderer);
    let cloud_pixels = changed_pixels(&without_dust, &without_cloud);
    assert!(cloud_pixels > 0, "authored fog cloud must draw");
    println!("BF1 Geonosis: {} moving ship parts; dust {dust_pixels} pixels; fog cloud {cloud_pixels} pixels", initial.len());
}
