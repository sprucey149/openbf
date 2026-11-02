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
        let mut out = self.ctx.run(raw, ui);
        // Entry can precede the first motion by a frame. egui restores its
        // arrow when that motion finally gives it a pointer position.
        let pointer_present = self.ctx.input(|i| i.pointer.has_pointer());
        self.cursor_dirty |= pointer_present != self.cursor_pointer_present;
        self.cursor_pointer_present = pointer_present;
        if self.window_cursor.is_some() {
            // Keep egui's fallback an arrow; widget icons do not own this cursor.
            out.platform_output.cursor_icon = egui::CursorIcon::Default;
        }
        state.handle_platform_output(window, out.platform_output);
        // egui can restore its arrow on entry or focus changes. Apply the
        // game's cursor last, before the event loop flushes to the compositor.
        if self.cursor_dirty {
            if let Some(cursor) = &self.window_cursor {
                window.set_cursor(cursor.clone());
                window.set_cursor_visible(self.cursor_visible);
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
