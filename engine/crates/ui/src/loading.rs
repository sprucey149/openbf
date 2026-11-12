//! BF1's 4:3 loading display. The scan and zoom share one wall-clock sample
//! with the audio. The shell may keep the final image up while loading.

use egui::{
    pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, TextureHandle, TextureId, Vec2,
};
use std::time::Duration;

const FULL: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));

/// BF1 PS2 FUN_002c6468 advances when at least 40 ms have elapsed. At 60 Hz
/// the capture advances about every third refresh. Sample the same recurrence
/// at 50 ms and interpolate between samples on modern displays.
const SCAN_SAMPLE: f32 = 0.05;
const TRACK_SOUND_DELTA: f32 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Vertical,
    Horizontal,
    Zoom,
}

pub struct Choreography {
    tracking: f32,
    zoom: f32,
    targets: Vec<Rect>,
}

pub struct Frame {
    pub step: usize,
    pub cue: Option<Cue>,
    target: Rect,
    selection: Rect,
    trails: [Rect; 3],
    blend: Option<f32>,
    final_transition: bool,
}

impl Choreography {
    pub fn new(tracking: f32, zoom: f32, targets: Vec<Rect>) -> Option<Self> {
        if !(0.1..=15.0).contains(&tracking)
            || !(0.1..=15.0).contains(&zoom)
            || targets.is_empty()
            || targets
                .iter()
                .any(|r| !r.is_finite() || !r.is_positive() || !FULL.contains_rect(*r))
        {
            return None;
        }
        Some(Self {
            tracking,
            zoom,
            targets,
        })
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f32((self.tracking + self.zoom) * self.targets.len() as f32)
    }

    pub fn sample(&self, elapsed: Duration) -> Frame {
        let cycle = self.tracking + self.zoom;
        let elapsed = elapsed.as_secs_f32();
        let step = (elapsed / cycle).floor() as usize;
        let Some(&target) = self.targets.get(step) else {
            return Frame {
                step: self.targets.len(),
                cue: None,
                target: FULL,
                selection: FULL,
                trails: [FULL; 3],
                blend: None,
                final_transition: false,
            };
        };
        let at = elapsed - step as f32 * cycle;
        let (selection, blend, cue) = self.selection(target, at);
        let trails = std::array::from_fn(|i| {
            self.selection(target, (at - (i + 1) as f32 * SCAN_SAMPLE).max(0.0))
                .0
        });
        Frame {
            step,
            cue,
            target,
            selection,
            trails,
            blend,
            final_transition: step + 1 == self.targets.len(),
        }
    }

    /// BF1 PS2 FUN_002c69c0 settles Y first, then X. The loop is audible only
    /// while the leading edge moves by more than 0.001 of the whole image.
    fn selection(&self, target: Rect, at: f32) -> (Rect, Option<f32>, Option<Cue>) {
        if at >= self.tracking {
            let zoom_time = at - self.tracking;
            let progress = (zoom_time / self.zoom).clamp(0.0, 1.0);
            // The window expands linearly. FUN_002c69c0 fades the previous
            // alpha by the elapsed zoom fraction on every update.
            let (blend, _) = sweep(zoom_time, self.zoom);
            return (
                interpolate(target, FULL, progress),
                Some(blend),
                Some(Cue::Zoom),
            );
        }
        let half = self.tracking * 0.5;
        let (x, y, moving) = if at < half {
            let (y, moving) = sweep(at, half);
            (
                0.0,
                y,
                (moving * target.min.y > TRACK_SOUND_DELTA).then_some(Cue::Vertical),
            )
        } else {
            let (x, moving) = sweep(at - half, half);
            (
                x,
                1.0,
                (moving * target.min.x > TRACK_SOUND_DELTA).then_some(Cue::Horizontal),
            )
        };
        let edge = |start: f32, end: f32, t: f32| start + (end - start) * t;
        (
            Rect::from_min_max(
                pos2(target.min.x * x, target.min.y * y),
                pos2(edge(1.0, target.max.x, x), edge(1.0, target.max.y, y)),
            ),
            None,
            moving,
        )
    }
}

/// The original eases from the previous position using the elapsed fraction
/// of the sweep at each update.
fn sweep(at: f32, duration: f32) -> (f32, f32) {
    let sample = at / SCAN_SAMPLE;
    let whole = sample.floor() as u32;
    let mut position = 0.0;
    for i in 1..=whole {
        position += (1.0 - position) * (i as f32 * SCAN_SAMPLE / duration).min(1.0);
    }
    let delta = (1.0 - position) * ((whole + 1) as f32 * SCAN_SAMPLE / duration).min(1.0);
    (position + delta * sample.fract(), delta)
}

fn interpolate(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min.lerp(b.min, t), a.max.lerp(b.max, t))
}

fn inside(dst: Rect, uv: Rect) -> Rect {
    let point = |p: Pos2| dst.min + p.to_vec2() * dst.size();
    Rect::from_min_max(point(uv.min), point(uv.max))
}

/// Sprites baked from the common BF1 loading LVL.
pub struct Sprites {
    pub disk: TextureHandle,
    pub led: TextureHandle,
    pub edges: [TextureHandle; 3],
    pub scanline: TextureHandle,
    pub noise: Noise,
    pub label: Option<Label>,
}

pub struct Noise {
    pub frames: Vec<TextureHandle>,
    pub frame_rate: f32,
    pub uv_start: [f32; 2],
}

pub struct Label {
    pub font: manifest::hud::HudFont,
    pub pages: Vec<TextureHandle>,
}

pub fn draw_choreography(
    ctx: &egui::Context,
    images: [Option<TextureId>; 2],
    frame: &Frame,
    sprites: &Sprites,
    elapsed: Duration,
    prompt: bool,
) {
    let screen = ctx.content_rect();
    let dst = pillarbox(screen);
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::BLACK);
    let clipped = painter.with_clip_rect(dst);
    if let Some(image) = images[0] {
        clipped.image(image, dst, FULL, Color32::WHITE);
        if let (Some(blend), Some(next)) = (frame.blend, images[1]) {
            let view = inside(dst, frame.selection);
            // BF1 PS2 FUN_002c7780 draws the magnified crop beneath the incoming view.
            clipped.image(image, view, frame.target, Color32::WHITE);
            clipped.image(next, view, FULL, alpha(blend));
        }
    }
    if images[1].is_some() {
        let fade = if frame.final_transition {
            frame.blend.map_or(1.0, |t| 1.0 - t)
        } else {
            1.0
        };
        // FUN_002c7780 darkens with the scanline alpha and applies the
        // changing noise tile in a separate gray layer.
        let scan_uv = Rect::from_min_max(
            Pos2::ZERO,
            pos2(1.0, 480.0 / sprites.scanline.size()[1] as f32),
        );
        clipped.image(
            sprites.scanline.id(),
            dst,
            scan_uv,
            Color32::from_black_alpha(alpha(fade).a()),
        );
        let noise = &sprites.noise;
        if !noise.frames.is_empty() {
            let index = (elapsed.as_secs_f32() * noise.frame_rate) as usize % noise.frames.len();
            let uv = Rect::from_min_max(pos2(noise.uv_start[0], noise.uv_start[1]), FULL.max);
            clipped.image(
                noise.frames[index].id(),
                dst,
                uv,
                Color32::from_rgba_unmultiplied(80, 80, 80, alpha(fade).a()),
            );
        }
        let edges = sprites.edges.each_ref().map(TextureHandle::id);
        for trail in frame.trails.into_iter().rev() {
            draw_edges(&clipped, dst, trail, edges, alpha(0.15 * fade));
        }
        draw_edges(&clipped, dst, frame.selection, edges, alpha(fade));
    }
    draw_disk(&clipped, dst, sprites, elapsed.as_secs_f32());
    draw_skip(&painter, screen, prompt);
}

fn alpha(opacity: f32) -> Color32 {
    Color32::from_white_alpha((opacity * 255.0).round() as u8)
}

fn draw_edges(
    painter: &egui::Painter,
    dst: Rect,
    selection: Rect,
    edges: [TextureId; 3],
    tint: Color32,
) {
    let r = inside(dst, selection);
    let size = vec2(dst.width() / 640.0, dst.height() / 480.0) * 32.0;
    let half = size * 0.5;
    for y in [r.min.y, r.max.y] {
        let mut left = dst.min.x;
        for x in [r.min.x, r.max.x] {
            if left < x - half.x {
                let segment =
                    Rect::from_min_max(pos2(left, y - half.y), pos2(x - half.x, y + half.y));
                painter.image(edges[0], segment, FULL, tint);
            }
            painter.image(
                edges[2],
                Rect::from_center_size(pos2(x, y), size),
                FULL,
                tint,
            );
            left = x + half.x;
        }
        if left < dst.max.x {
            let segment = Rect::from_min_max(pos2(left, y - half.y), pos2(dst.max.x, y + half.y));
            painter.image(edges[0], segment, FULL, tint);
        }
    }
    for x in [r.min.x, r.max.x] {
        let mut top = dst.min.y;
        for bottom in [r.min.y - half.y, r.max.y - half.y, dst.max.y] {
            if top < bottom {
                let segment = Rect::from_min_max(pos2(x - half.x, top), pos2(x + half.x, bottom));
                painter.image(edges[1], segment, FULL, tint);
            }
            top = top.max(bottom + size.y);
        }
    }
}

fn draw_disk(painter: &egui::Painter, dst: Rect, sprites: &Sprites, time: f32) {
    // FUN_002c5b88 anchors the two 256 px sprites at (0.75, 0.7); the
    // inner copy is 72% size and turns in the opposite direction.
    let center = dst.min + dst.size() * vec2(0.75, 0.7);
    let scale = dst.width() / 640.0;
    for (size, rate) in [(256.0, 0.2), (256.0 * 0.72, -0.2)] {
        let mut mesh = egui::Mesh::with_texture(sprites.disk.id());
        mesh.add_rect_with_uv(
            Rect::from_center_size(center, Vec2::splat(size * scale)),
            FULL,
            Color32::WHITE,
        );
        mesh.rotate(egui::emath::Rot2::from_angle(time * rate), center);
        painter.add(egui::Shape::mesh(mesh));
    }
    let led = (time * 12.0) as usize % 30;
    for i in 0..30 {
        let angle = i as f32 * std::f32::consts::TAU / 30.0;
        let direction = vec2(angle.cos(), angle.sin());
        let age = (led + 30 - i) % 30;
        let light = (255.0 * 0.95_f32.powi(age as i32)) as u8;
        let at = center + direction * (55.0 * scale);
        let mut mesh = egui::Mesh::with_texture(sprites.led.id());
        mesh.add_rect_with_uv(
            Rect::from_center_size(at, sprites.led.size_vec2() * scale),
            FULL,
            Color32::from_gray(light),
        );
        mesh.rotate(egui::emath::Rot2::from_angle(angle), at);
        painter.add(egui::Shape::mesh(mesh));
    }
    draw_label(painter, center, scale, sprites.label.as_ref());
}

fn draw_label(painter: &egui::Painter, center: Pos2, scale: f32, label: Option<&Label>) {
    let color = Color32::from_rgb(200, 200, 255);
    let rotation = egui::emath::Rot2::from_angle(-0.15);
    if let Some(label) = label {
        let glyphs = "LOADING"
            .bytes()
            .filter_map(|c| label.font.glyphs.iter().find(|g| g.code == u16::from(c)));
        let width = crate::hud_draw::run_width(&label.font, "LOADING", scale, false);
        let top = center.y - f32::from(label.font.line_height) * scale * 0.5;
        let mut pen = center.x - width * 0.5;
        for g in glyphs {
            if let Some(page) = label.pages.get(usize::from(g.page)) {
                let [u0, u1, v0, v1] = g.uv;
                let uv = Rect::from_min_max(pos2(u0, v0), pos2(u1, v1));
                let rect = Rect::from_min_max(
                    pos2(pen + f32::from(g.x0) * scale, top + f32::from(g.y0) * scale),
                    pos2(pen + f32::from(g.x1) * scale, top + f32::from(g.y1) * scale),
                );
                let mut mesh = egui::Mesh::with_texture(page.id());
                mesh.add_rect_with_uv(rect, uv, color);
                mesh.rotate(rotation, center);
                painter.add(egui::Shape::mesh(mesh));
            }
            pen += f32::from(g.advance) * scale;
        }
        return;
    }
    let galley =
        painter.layout_no_wrap("LOADING".into(), FontId::proportional(21.0 * scale), color);
    let offset = rotation * (galley.size() * 0.5);
    painter.add(egui::epaint::TextShape::new(center - offset, galley, color).with_angle(-0.15));
}

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
    draw_skip(&painter, screen, prompt);
}

fn draw_skip(painter: &egui::Painter, screen: Rect, prompt: bool) {
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
    fn bf1_tracks_each_axis_pauses_and_zooms_into_the_authored_window() {
        let target = Rect::from_min_max(pos2(0.2, 0.6), pos2(0.25, 0.65));
        let motion = Choreography::new(3.5, 1.0, vec![target; 3]).unwrap();
        let sample = |ms| motion.sample(Duration::from_millis(ms));
        assert_eq!(motion.duration(), Duration::from_millis(13_500));

        let vertical = sample(300);
        assert_eq!(vertical.cue, Some(Cue::Vertical));
        assert_eq!(vertical.selection.min.x, 0.0);
        assert!(vertical.selection.min.y > 0.0 && vertical.selection.min.y < target.min.y);
        assert_eq!(sample(1500).cue, None);

        let horizontal = sample(2000);
        assert_eq!(horizontal.cue, Some(Cue::Horizontal));
        assert_eq!(horizontal.selection.min.y, target.min.y);
        assert!(horizontal.selection.min.x > 0.0 && horizontal.selection.min.x < target.min.x);
        assert_eq!(sample(3300).cue, None);

        let zoom = sample(4000);
        assert_eq!(zoom.cue, Some(Cue::Zoom));
        assert!(zoom.blend.unwrap() > 0.9);
        assert_eq!(zoom.selection, interpolate(target, FULL, 0.5));
        assert_eq!(sample(4500).step, 1);
        assert_eq!(sample(9000).step, 2);
        assert_eq!(sample(13_500).step, 3);
        assert_eq!(sample(13_500).cue, None);
    }

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
