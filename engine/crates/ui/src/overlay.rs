//! egui ↔ winit ↔ wgpu glue for the game's windowed loop, plus a windowless
//! path for headless captures and goldens. Adapted from the stock
//! egui-wgpu/egui-winit integration pattern (and an earlier wgpu project's
//! `render_tests.rs` renderer setup, which uses the same `RendererOptions`
//! constructor); painted through `render::Renderer::render_with_overlay`.

use egui_wgpu::ScreenDescriptor;
use winit::window::Window;

pub struct EguiOverlay {
    ctx: egui::Context,
    /// None for the windowless (headless capture / golden) path.
    state: Option<egui_winit::State>,
    renderer: egui_wgpu::Renderer,
    window_cursor: Option<winit::window::Cursor>,
    cursor_visible: bool,
    cursor_dirty: bool,
    cursor_pointer_present: bool,
    /// Issue #182: the menu cursor drawn by the game while the window is
    /// fullscreen ([`cursor_plan`]).
    software_cursor: Option<SoftwareCursor>,
    /// Whether the last applied plan drew the cursor in software.
    software_shown: bool,
    /// Whether the last applied plan showed the system cursor.
    os_shown: bool,
    /// Issue #182: the window fills its monitor (a fullscreen the system
    /// chose, which `Window::fullscreen` may not report), read on resize.
    covers_monitor: bool,
    /// Issue #182: frames left in which the cursor is applied again.
    reassert: CursorReassert,
}

/// Issue #182: how long the cursor is applied again after a display,
/// focus or size change. Every platform finishes a fullscreen change on its
/// own schedule (a Windows mode switch, a macOS Space animation, an X11 or
/// Wayland configure), and each one may rebuild the window's cursor state
/// after the game's last call, so the game's choice is sent every frame for
/// about two seconds rather than once. [ours, measure: playtest on each OS,
/// replace: drop to the event alone if no OS shows a lost cursor]
pub const CURSOR_REASSERT_FRAMES: u32 = 120;

/// Issue #182: the frames in which the overlay sends the cursor again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorReassert {
    frames: u32,
}

impl CursorReassert {
    /// A display, focus or size change: send the cursor for the next frames.
    pub fn arm(&mut self) {
        self.frames = CURSOR_REASSERT_FRAMES;
    }

    /// One frame: true while the cursor should be sent again.
    pub fn tick(&mut self) -> bool {
        let on = self.frames > 0;
        self.frames = self.frames.saturating_sub(1);
        on
    }
}

/// Issue #182: window events after which the system may have reset the
/// cursor's image, visibility or grab.
pub fn resets_cursor(event: &winit::event::WindowEvent) -> bool {
    use winit::event::WindowEvent;
    matches!(
        event,
        WindowEvent::Resized(_)
            | WindowEvent::Moved(_)
            | WindowEvent::Focused(_)
            | WindowEvent::Occluded(_)
            | WindowEvent::ScaleFactorChanged { .. }
    )
}

/// Issue #182: the window covers its whole monitor, so it is fullscreen to
/// the player whatever the window reports.
pub fn covers_monitor(inner: [u32; 2], monitor: Option<[u32; 2]>) -> bool {
    monitor.is_some_and(|m| m[0] > 0 && m[1] > 0 && inner[0] >= m[0] && inner[1] >= m[1])
}

/// Issue #182: the menu cursor as an image the overlay draws itself.
struct SoftwareCursor {
    rgba: Vec<u8>,
    side: u32,
    /// The click point in the image's pixels.
    hotspot: [f32; 2],
    texture: Option<egui::TextureHandle>,
}

/// How a frame shows the menu cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPlan {
    /// The system cursor is shown over the window.
    pub os: bool,
    /// The overlay draws the cursor at the pointer.
    pub software: bool,
}

/// Issue #182: players on Windows saw no menu cursor in fullscreen, though
/// the game had asked for it (the window's own cursor is not guaranteed over
/// a fullscreen surface). Fullscreen menus therefore draw the cursor in the
/// overlay and hide the system one, so exactly one cursor shows; a window
/// keeps the system cursor (no frame of lag). A hidden cursor (mouse look,
/// a controller's battle) is hidden either way. [ours]
pub fn cursor_plan(visible: bool, has_software: bool, fullscreen: bool) -> CursorPlan {
    let software = visible && has_software && fullscreen;
    CursorPlan {
        os: visible && !software,
        software,
    }
}

/// Draw the software cursor with its click point at `pointer` (points).
fn paint_software_cursor(
    ctx: &egui::Context,
    texture: &egui::TextureHandle,
    side_px: u32,
    hotspot_px: [f32; 2],
    pointer: egui::Pos2,
) {
    let ppp = ctx.pixels_per_point();
    let size = egui::Vec2::splat(side_px as f32 / ppp);
    let min = pointer - egui::vec2(hotspot_px[0], hotspot_px[1]) / ppp;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Debug,
        egui::Id::new("openbf_menu_cursor"),
    ));
    painter.image(
        texture.id(),
        egui::Rect::from_min_size(min, size),
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}

/// One frame's tessellated UI, ready to paint.
pub struct FramePaint {
    prims: Vec<egui::ClippedPrimitive>,
    textures: egui::TexturesDelta,
    pixels_per_point: f32,
}

impl FramePaint {
    /// The texture uploads this frame carries and their bytes (a font
    /// atlas that grew, a new image) - the frame recorder's (card HITCH).
    pub fn texture_uploads(&self) -> (usize, u64) {
        let bytes = self
            .textures
            .set
            .iter()
            .map(|(_, d)| (d.image.width() * d.image.height() * 4) as u64)
            .sum();
        (self.textures.set.len(), bytes)
    }
}

impl EguiOverlay {
    pub fn new(window: &Window, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let renderer =
            egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        Self {
            ctx,
            state: Some(state),
            renderer,
            window_cursor: None,
            cursor_visible: true,
            cursor_dirty: false,
            cursor_pointer_present: false,
            software_cursor: None,
            software_shown: false,
            os_shown: false,
            covers_monitor: false,
            reassert: CursorReassert::default(),
        }
    }

    /// Windowless overlay for headless captures and goldens: no winit input,
    /// the frame size is passed to [`EguiOverlay::begin_headless`].
    pub fn headless(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        // egui fades new Areas in over `animation_time` (0.1 s); a headless
        // capture runs two frames, so without this the HUD lands at ~44 %
        // opacity (measured). Instant for captures and goldens.
        ctx.style_mut(|s| s.animation_time = 0.0);
        Self {
            ctx,
            state: None,
            window_cursor: None,
            cursor_visible: true,
            cursor_dirty: false,
            cursor_pointer_present: false,
            software_cursor: None,
            software_shown: false,
            os_shown: false,
            covers_monitor: false,
            reassert: CursorReassert::default(),
            renderer: egui_wgpu::Renderer::new(
                device,
                format,
                egui_wgpu::RendererOptions::default(),
            ),
        }
    }

    /// The egui context (texture uploads for images).
    pub fn ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// Let the game own the native cursor instead of egui's widget icons.
    pub fn set_window_cursor(&mut self, cursor: winit::window::Cursor) {
        self.window_cursor = Some(cursor);
        self.cursor_dirty = true;
    }

    /// Issue #182: the menu cursor's image (straight RGBA, `side` x `side`
    /// pixels, click point `hotspot` in pixels) for fullscreen menus.
    pub fn set_software_cursor(&mut self, rgba: Vec<u8>, side: u32, hotspot: [f32; 2]) {
        self.software_cursor = Some(SoftwareCursor {
            rgba,
            side,
            hotspot,
            texture: None,
        });
        self.cursor_dirty = true;
    }

    /// Apply the cursor again at the next frame: something else (the pointer
    /// grab) set the system cursor's visibility directly.
    pub fn refresh_cursor(&mut self) {
        self.cursor_dirty = true;
    }

    /// Issue #182: the game asked for a new window mode; keep sending the
    /// cursor while the system finishes the change.
    pub fn display_changed(&mut self) {
        self.cursor_dirty = true;
        self.reassert.arm();
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_dirty |= self.cursor_visible != visible;
        self.cursor_visible = visible;
    }

    /// Feed a window event; true when egui consumed it (pointer over a panel).
    pub fn on_window_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        use winit::event::WindowEvent;
        self.cursor_dirty |= matches!(
            event,
            WindowEvent::CursorEntered { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::Focused(_)
                | WindowEvent::ScaleFactorChanged { .. }
        );
        if resets_cursor(event) {
            self.reassert.arm();
        }
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            let inner = window.inner_size();
            let monitor = window.current_monitor().map(|m| {
                let size = m.size();
                [size.width, size.height]
            });
            self.covers_monitor = covers_monitor([inner.width, inner.height], monitor);
        }
        match &mut self.state {
            Some(st) => st.on_window_event(window, event).consumed,
            None => false,
        }
    }

    /// Run the UI closure for this frame and tessellate.
    pub fn begin(&mut self, window: &Window, ui: impl FnMut(&egui::Context)) -> FramePaint {
        let Some(state) = self.state.as_mut() else {
            let size = window.inner_size();
            return self.begin_headless(
                [size.width, size.height],
                window.scale_factor() as f32,
                ui,
            );
        };
        let raw = state.take_egui_input(window);
        let plan = cursor_plan(
            self.cursor_visible && self.window_cursor.is_some(),
            self.software_cursor.is_some(),
            self.covers_monitor || window.fullscreen().is_some(),
        );
        self.cursor_dirty |= self.reassert.tick();
        self.cursor_dirty |= plan.software != self.software_shown;
        self.software_shown = plan.software;
        self.cursor_dirty |= plan.os != self.os_shown;
        self.os_shown = plan.os;
        let soft = if plan.software {
            self.software_cursor.as_mut().map(|c| {
                let texture = c
                    .texture
                    .get_or_insert_with(|| {
                        self.ctx.load_texture(
                            "openbf_menu_cursor",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [c.side as usize, c.side as usize],
                                &c.rgba,
                            ),
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .clone();
                (texture, c.side, c.hotspot)
            })
        } else {
            None
        };
        let mut ui = ui;
        let mut out = self.ctx.run(raw, |ctx| {
            ui(ctx);
            if let Some((texture, side, hotspot)) = &soft {
                if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    paint_software_cursor(ctx, texture, *side, *hotspot, pos);
                }
            }
        });
        // Entry can precede the first motion by a frame. egui restores its
        // arrow when that motion finally gives it a pointer position.
        let pointer_present = self.ctx.input(|i| i.pointer.has_pointer());
        self.cursor_dirty |= pointer_present != self.cursor_pointer_present;
        self.cursor_pointer_present = pointer_present;
        if self.window_cursor.is_some() {
            // Keep egui's fallback an arrow; widget icons do not own this cursor.
            // A hidden cursor (pad input, mouse look) is hidden for egui too,
            // so its own pointer-entry update cannot show it again.
            out.platform_output.cursor_icon = if plan.os {
                egui::CursorIcon::Default
            } else {
                egui::CursorIcon::None
            };
        }
        state.handle_platform_output(window, out.platform_output);
        // egui can restore its arrow on entry or focus changes. Apply the
        // game's cursor last, before the event loop flushes to the compositor.
        if self.cursor_dirty {
            if let Some(cursor) = &self.window_cursor {
                window.set_cursor(cursor.clone());
                window.set_cursor_visible(plan.os);
                self.cursor_dirty = false;
            }
        }
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        FramePaint {
            prims,
            textures: out.textures_delta,
            pixels_per_point: out.pixels_per_point,
        }
    }

    /// Windowless frame: runs the UI twice (anchored areas need a warm-up
    /// frame to size themselves) and tessellates the second.
    pub fn begin_headless(
        &mut self,
        size_px: [u32; 2],
        pixels_per_point: f32,
        mut ui: impl FnMut(&egui::Context),
    ) -> FramePaint {
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(
                    size_px[0] as f32 / pixels_per_point,
                    size_px[1] as f32 / pixels_per_point,
                ),
            )),
            ..Default::default()
        };
        // Textures from the warm-up frames must still reach the GPU. More than one:
        // a layout that measures on the frame before (the pause menu's rows) settles
        // over a few, as it does in play within the first frames.
        let mut textures = egui::TexturesDelta::default();
        for _ in 0..3 {
            textures.append(self.ctx.run(input(), &mut ui).textures_delta);
        }
        let out = self.ctx.run(input(), &mut ui);
        textures.append(out.textures_delta);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        FramePaint {
            prims,
            textures,
            pixels_per_point: out.pixels_per_point,
        }
    }

    /// Paint a prepared frame into `view` (call from the renderer's overlay hook).
    pub fn paint(
        &mut self,
        frame: &FramePaint,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size: [u32; 2],
    ) {
        let screen = ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: frame.pixels_per_point,
        };
        for (id, delta) in &frame.textures.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }
        let extra = self
            .renderer
            .update_buffers(device, queue, encoder, &frame.prims, &screen);
        if !extra.is_empty() {
            queue.submit(extra);
        }
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &frame.prims, &screen);
        }
        for id in &frame.textures.free {
            self.renderer.free_texture(id);
        }
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    /// Issue #182: fullscreen menus draw the cursor themselves and hide the
    /// system one; a window keeps the system cursor; hidden stays hidden.
    #[test]
    fn fullscreen_menus_show_exactly_one_cursor() {
        let p = |os, software| CursorPlan { os, software };
        assert_eq!(
            cursor_plan(true, true, true),
            p(false, true),
            "fullscreen menu"
        );
        assert_eq!(
            cursor_plan(true, true, false),
            p(true, false),
            "windowed menu"
        );
        assert_eq!(
            cursor_plan(false, true, true),
            p(false, false),
            "fullscreen play"
        );
        assert_eq!(
            cursor_plan(false, true, false),
            p(false, false),
            "windowed play"
        );
        assert_eq!(
            cursor_plan(true, false, true),
            p(true, false),
            "no image: the system cursor"
        );
    }

    /// Issue #182: after a fullscreen, focus or size change the cursor is
    /// sent again for a fixed run of frames, then left alone; a window that
    /// fills its monitor counts as fullscreen.
    #[test]
    fn display_changes_send_the_cursor_again_for_a_while() {
        use winit::event::WindowEvent;
        let mut r = CursorReassert::default();
        assert!(!r.tick(), "idle: nothing sent");
        r.arm();
        let sent = (0..CURSOR_REASSERT_FRAMES + 10)
            .filter(|_| r.tick())
            .count();
        assert_eq!(sent as u32, CURSOR_REASSERT_FRAMES);
        assert!(!r.tick(), "settled");
        r.arm();
        assert!(r.tick());
        r.arm();
        let again = (0..CURSOR_REASSERT_FRAMES * 2).filter(|_| r.tick()).count();
        assert_eq!(
            again as u32, CURSOR_REASSERT_FRAMES,
            "a new change restarts the run"
        );

        assert!(resets_cursor(&WindowEvent::Focused(true)));
        assert!(resets_cursor(&WindowEvent::Occluded(false)));
        assert!(resets_cursor(&WindowEvent::Resized(
            winit::dpi::PhysicalSize::new(1, 1)
        )));
        assert!(!resets_cursor(&WindowEvent::CursorLeft {
            device_id: winit::event::DeviceId::dummy(),
        }));

        assert!(covers_monitor([1920, 1080], Some([1920, 1080])));
        assert!(
            !covers_monitor([1920, 1040], Some([1920, 1080])),
            "maximized"
        );
        assert!(!covers_monitor([1280, 720], None), "no monitor");
        assert!(!covers_monitor([0, 0], Some([0, 0])));
        // A filled monitor counts as fullscreen for the plan.
        let fs = covers_monitor([2560, 1440], Some([2560, 1440]));
        assert_eq!(
            cursor_plan(true, true, fs),
            CursorPlan {
                os: false,
                software: true
            }
        );
    }

    /// The drawn cursor puts its click point on the pointer, on the top layer,
    /// at its pixel size whatever the display scale.
    #[test]
    fn the_software_cursor_tip_sits_on_the_pointer() {
        for ppp in [1.0f32, 1.25, 2.0] {
            let ctx = egui::Context::default();
            let side = 40;
            let texture = ctx.load_texture(
                "cursor",
                egui::ColorImage::from_rgba_unmultiplied([side, side], &vec![255; side * side * 4]),
                egui::TextureOptions::LINEAR,
            );
            let pointer = egui::pos2(100.0, 50.0);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                events: vec![egui::Event::PointerMoved(pointer)],
                ..Default::default()
            };
            ctx.set_pixels_per_point(ppp);
            let out = ctx.run(input, |ctx| {
                let pos = ctx.input(|i| i.pointer.hover_pos()).expect("pointer");
                paint_software_cursor(ctx, &texture, side as u32, [3.75, 3.75], pos);
            });
            let rects: Vec<egui::Rect> = out
                .shapes
                .iter()
                .filter(|c| c.clip_rect.is_positive())
                .filter_map(|c| match &c.shape {
                    egui::Shape::Mesh(m) if m.texture_id == texture.id() => {
                        Some(egui::Rect::from_points(
                            &m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
                        ))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(rects.len(), 1, "ppp {ppp}: one cursor image");
            let r = rects[0];
            let tip = r.min + egui::vec2(3.75, 3.75) / ppp;
            assert!((tip - pointer).length() < 1e-3, "ppp {ppp}: tip {tip:?}");
            assert!((r.width() - side as f32 / ppp).abs() < 1e-3, "ppp {ppp}");
        }
    }
}
