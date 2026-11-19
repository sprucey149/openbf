//! Headless CPU witness for the actual Host page and installed menu face.
use super::*;
use ui::egui::{self, Color32};

#[test]
#[ignore = "requires HOST_LAYOUT_ASSETS with the three baked menu fonts; CPU only"]
fn host_layout_native_page_geometry() {
    let assets = std::path::PathBuf::from(
        std::env::var_os("HOST_LAYOUT_ASSETS").expect("HOST_LAYOUT_ASSETS"),
    );
    let fonts = game::hud_feed::menu_face(&assets);
    assert_eq!(fonts.len(), 3, "all native menu faces must be installed");
    let addresses = reach::host_addresses(
        &[
            (std::net::Ipv4Addr::new(192, 168, 255, 254), false),
            (std::net::Ipv4Addr::new(100, 127, 255, 254), true),
        ],
        Some(65535),
    );
    let lines: Vec<_> = addresses
        .iter()
        .map(|s| (s.clone(), Color32::LIGHT_BLUE))
        .collect();
    for size in [
        [1920.0, 1080.0],
        [1280.0, 720.0],
        [800.0, 600.0],
        [640.0, 480.0],
    ] {
        let ctx = egui::Context::default();
        ui::typography::face::install(&ctx, &fonts);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size.into());
        let safe = screen.shrink(size[1] * 0.05);
        let mut page = MpPage::host();
        for (expanded, error) in [(false, false), (true, false), (true, true)] {
            let mut lines = lines.clone();
            let message = "Could not open UDP port 65535: Address already in use";
            if error {
                lines.insert(0, (message.into(), Color32::LIGHT_RED));
            }
            let MpPage::Host(h) = &mut page else {
                unreachable!()
            };
            h.cheats_allowed = expanded;
            h.message = error.then(|| message.to_owned());
            h.port = "65535".into();
            let count = h.rows().len();
            for cursor in 0..count {
                let MpPage::Host(h) = &mut page else {
                    unreachable!()
                };
                h.cursor = cursor;
                let label = h.rows()[cursor].label.trim().to_owned();
                // The production dispatcher must paint only its two endpoints.
                for frame in 0..4 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(screen),
                            ..Default::default()
                        },
                        |ctx| {
                            draw_mp(ctx, &mut page);
                        },
                    );
                    if frame == 3 {
                        let text: Vec<_> = output
                            .shapes
                            .iter()
                            .filter_map(|c| match &c.shape {
                                egui::Shape::Text(t) => Some(t.galley.text()),
                                _ => None,
                            })
                            .collect();
                        assert!(text.contains(&"LAN: Host Game"));
                        assert_eq!(
                            text.iter()
                                .filter(|s| s.starts_with("LAN Address:"))
                                .count(),
                            1
                        );
                        assert_eq!(
                            text.iter()
                                .filter(|s| s.starts_with("VPN Address:"))
                                .count(),
                            1
                        );
                        assert!(!text.iter().any(|s| s.contains("firewall")
                            || s.contains("Up / Down")
                            || s.contains("VPN app")));
                    }
                }
                // Use deliberately long synthetic endpoints for the same draw.
                let MpPage::Host(h) = &mut page else {
                    unreachable!()
                };
                let rows = h.rows();
                let mut output = egui::FullOutput::default();
                for _ in 0..4 {
                    output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(screen),
                            ..Default::default()
                        },
                        |ctx| {
                            ui::lobby::draw_host_page(
                                ctx,
                                &rows,
                                cursor,
                                h.text_mut(),
                                false,
                                &lines,
                            );
                        },
                    );
                }
                for want in ["LAN: Host Game", &addresses[0], &addresses[1], &label] {
                    let (clip, text) = output
                        .shapes
                        .iter()
                        .find_map(|c| match &c.shape {
                            egui::Shape::Text(t) if t.galley.text().trim() == want => {
                                Some((c.clip_rect, t))
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("{size:?} missing {want}"));
                    let bounds = text.visual_bounding_rect();
                    assert!(
                        safe.contains_rect(bounds),
                        "{size:?} {want}: {bounds:?}, safe {safe:?}"
                    );
                    assert!(
                        clip.contains_rect(bounds),
                        "{size:?} {want}: {bounds:?}, clip {clip:?}"
                    );
                    assert_eq!(text.galley.rows.len(), 1, "{size:?} wrapped {want}");
                }
                for shape in &output.shapes {
                    if let egui::Shape::Text(t) = &shape.shape {
                        let bounds = t.visual_bounding_rect();
                        if shape.clip_rect.intersects(bounds) {
                            assert!(
                                shape.clip_rect.contains_rect(bounds),
                                "{size:?} partial row {:?}: {bounds:?}, clip {:?}",
                                t.galley.text(),
                                shape.clip_rect
                            );
                        }
                    }
                }
                if error {
                    let (clip, bounds) = output
                        .shapes
                        .iter()
                        .find_map(|c| match &c.shape {
                            egui::Shape::Text(t) if t.galley.text() == message => {
                                Some((c.clip_rect, t.visual_bounding_rect()))
                            }
                            _ => None,
                        })
                        .expect("actionable port error is painted");
                    assert!(safe.contains_rect(bounds) && clip.contains_rect(bounds));
                }
            }
        }
    }
}
