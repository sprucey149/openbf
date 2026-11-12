//! BF1 loading screen: one 4:3 image at a time, PILLARBOXED (PLAN
//! §Presentation — 4:3-authored content is never stretched), with a caption.

use egui::{Align2, Color32, Rect, RichText, TextureId, Vec2};

/// Largest 4:3 rect centered in `screen`.
pub fn pillarbox(screen: Rect) -> Rect {
    let w = screen.width();
    let h = screen.height();
    let (bw, bh) = if w / h > 4.0 / 3.0 {
        (h * 4.0 / 3.0, h)
    } else {
        (w, w * 3.0 / 4.0)
    };
    Rect::from_center_size(screen.center(), Vec2::new(bw, bh))
}

/// Paint the current step's image pillarboxed plus the caption.
pub fn draw_loading(
    ctx: &egui::Context,
    image: Option<TextureId>,
    caption: &str,
    step: usize,
    steps: usize,
) {
    draw_loading_with_skip(ctx, image, caption, step, steps, false);
}

/// The prompt is controlled by the loading input clock, never UI animation.
pub fn draw_loading_with_skip(
    ctx: &egui::Context,
    image: Option<TextureId>,
    caption: &str,
    step: usize,
    steps: usize,
    prompt: bool,
) {
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::BLACK);
    let dst = pillarbox(screen);
    if let Some(id) = image {
        painter.image(
            id,
            dst,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    egui::Area::new(egui::Id::new("loading_caption"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -24.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("{caption}   {}/{}", step + 1, steps.max(1)))
                    .size(18.0)
                    .color(Color32::from_rgb(120, 190, 255)),
            );
        });
    if prompt {
        // Painter text has no Area fade-in/out: the deadline is exactly 3 s.
        painter.text(
            screen.right_bottom() - Vec2::new(24.0, 24.0),
            Align2::RIGHT_BOTTOM,
            "Push any button to skip",
            egui::FontId::proportional(18.0),
            Color32::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pillarbox_keeps_four_by_three_centered() {
        let wide = pillarbox(Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(1920.0, 1080.0),
        ));
        assert!((wide.width() / wide.height() - 4.0 / 3.0).abs() < 1e-4);
        assert!((wide.height() - 1080.0).abs() < 1e-3);
        assert!((wide.center().x - 960.0).abs() < 1e-3);
        let tall = pillarbox(Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(800.0, 1000.0),
        ));
        assert!((tall.width() - 800.0).abs() < 1e-3);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw_loading(ctx, None, "TATOOINE", 0, 4)
        });
    }
}
