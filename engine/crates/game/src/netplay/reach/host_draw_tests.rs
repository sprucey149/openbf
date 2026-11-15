//! Owned-font, headless GPU Host form witness for issue #181.
use super::*;

#[test]
#[ignore = "requires GPU, HOST_LAYOUT_ASSETS and an owned HOST_LAYOUT_OUT directory"]
fn reach_host_layout_headless_gpu() {
    let assets = std::path::PathBuf::from(
        std::env::var_os("HOST_LAYOUT_ASSETS").expect("HOST_LAYOUT_ASSETS"),
    );
    let out =
        std::path::PathBuf::from(std::env::var_os("HOST_LAYOUT_OUT").expect("HOST_LAYOUT_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let fonts = crate::hud_feed::menu_face(&assets);
    assert_eq!(fonts.len(), 3);
    let cheats: Vec<_> = core::sim::Cheats::default()
        .entries()
        .iter()
        .map(|e| (e.0, e.1))
        .collect();
    let lines: Vec<_> = host_addresses(
        &[
            (Ipv4Addr::new(192, 168, 255, 254), false),
            (Ipv4Addr::new(100, 127, 255, 254), true),
        ],
        Some(65535),
    )
    .into_iter()
    .map(|s| (s, ui::egui::Color32::LIGHT_BLUE))
    .collect();
    for size in [[1920, 1080], [640, 480]] {
        let mut renderer = render::Renderer::new_headless(size[0], size[1]).unwrap();
        let mut overlay =
            ui::overlay::EguiOverlay::headless(renderer.device(), render::HEADLESS_FORMAT);
        ui::typography::face::install(overlay.ctx(), &fonts);
        for (name, expanded, port) in [
            ("compact", false, false),
            ("expanded", true, false),
            ("port", true, true),
        ] {
            let mut h = ui::lobby::HostSetup::new(&cheats, 65535);
            h.cheats_allowed = expanded;
            h.cursor = if port { 1 } else { h.rows().len() - 1 };
            let rows = h.rows();
            let paint = overlay.begin_headless(size, 1.0, |ctx| {
                ui::lobby::draw_host_page(
                    ctx,
                    &rows,
                    h.cursor,
                    if port { Some(&mut h.port) } else { None },
                    port,
                    &lines,
                );
            });
            renderer
                .render_with_overlay(
                    &render::Camera::default(),
                    &[],
                    [0.0, 0.0, 0.0, 1.0],
                    Some(&mut |dev, q, enc, target, size| {
                        overlay.paint(&paint, dev, q, enc, target, size)
                    }),
                )
                .unwrap();
            let (rgba, _, _) = renderer.capture_rgba().unwrap();
            let mut bright = 0;
            let inset = size[1] / 20;
            for (i, px) in rgba.chunks_exact(4).enumerate() {
                if px[..3].iter().any(|c| *c > 40) {
                    bright += 1;
                    let (x, y) = (i as u32 % size[0], i as u32 / size[0]);
                    assert!(
                        x >= inset && x < size[0] - inset && y >= inset && y < size[1] - inset,
                        "{size:?} {name}: visible UI outside safe inset at {x},{y}"
                    );
                }
            }
            assert!(bright > 1000, "the overlay actually painted");
            image::save_buffer(
                out.join(format!("host-{}x{}-{name}.png", size[0], size[1])),
                &rgba,
                size[0],
                size[1],
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}
