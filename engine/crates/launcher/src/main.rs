//! Native `openbf-launcher` binary entry point (winit + wgpu + egui).
//!
//! Opens a frameless `740×450` desktop window using the workspace's existing
//! `winit`/`wgpu`/`egui` stack. A `UserEvent::Wake` proxy lets the background
//! `pack.py` thread trigger repaints on new progress lines with zero polling.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use egui_wgpu::ScreenDescriptor;
use launcher::paths::LauncherPaths;
use launcher::ui::{HostActions, LauncherApp, WINDOW_HEIGHT, WINDOW_WIDTH};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    Wake,
}

struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuState {
    fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance
            .create_surface(Arc::clone(&window))
            .context("creating wgpu surface")?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .context("finding a compatible GPU adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("openbf-launcher"),
            required_features: wgpu::Features::empty(),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .context("requesting wgpu device")?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // egui blends text in sRGB space; prefer a non-sRGB surface format
        // (Bgra8Unorm / Rgba8Unorm) so dark text on light backgrounds stays crisp.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        LauncherApp::configure_style(&egui_ctx);
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer =
            egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            egui_ctx,
            egui_state,
            egui_renderer,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, app: &mut LauncherApp) -> (HostActions, Duration) {
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let mut actions = HostActions::default();
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            actions = app.ui(ctx);
        });
        let repaint_after = full_output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return (actions, Duration::ZERO);
            }
            Err(_) => return (actions, repaint_after),
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("openbf-launcher"),
            });
        self.egui_renderer
            .update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen);
        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("openbf-launcher-egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.egui_renderer
                .render(&mut rpass.forget_lifetime(), &tris, &screen);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        (actions, repaint_after)
    }
}

struct Host {
    gpu: Option<GpuState>,
    app: LauncherApp,
    next_repaint: Option<Instant>,
    launch_on_exit: bool,
}

impl ApplicationHandler<UserEvent> for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("openbf — launcher")
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64))
            .with_resizable(false)
            .with_decorations(false);
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create launcher window"),
        );
        window.request_redraw();
        self.gpu = Some(GpuState::new(window).expect("failed to initialize launcher renderer"));
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: UserEvent) {
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        if let Some(deadline) = self.next_repaint {
            let now = Instant::now();
            if now >= deadline {
                self.next_repaint = None;
                gpu.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(gpu) = &mut self.gpu else {
            return;
        };
        let response = gpu.egui_state.on_window_event(&gpu.window, &event);
        if response.repaint {
            gpu.window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => {
                self.app.cancel_build_if_running();
                self.gpu = None;
                event_loop.exit();
            }
            WindowEvent::Focused(true) => {
                self.app.refresh_fs_state();
                gpu.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                gpu.resize(size.width, size.height);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let (actions, repaint_after) = gpu.render(&mut self.app);
                if actions.drag_window {
                    let _ = gpu.window.drag_window();
                }
                if actions.minimize {
                    gpu.window.set_minimized(true);
                }
                if actions.launch_and_close {
                    self.app.cancel_build_if_running();
                    // Hide and destroy the launcher window + wgpu surface before
                    // spawning the game (which runs after the event loop exits) so
                    // the window vanishes immediately even if OS security scanning
                    // delays CreateProcess on a large single-file openbf binary.
                    gpu.window.set_visible(false);
                    self.gpu = None;
                    self.launch_on_exit = true;
                    event_loop.exit();
                    return;
                }
                if actions.close {
                    self.app.cancel_build_if_running();
                    gpu.window.set_visible(false);
                    self.gpu = None;
                    event_loop.exit();
                    return;
                }
                self.next_repaint = if repaint_after.is_zero() {
                    gpu.window.request_redraw();
                    None
                } else if repaint_after < Duration::from_secs(3600) {
                    Instant::now().checked_add(repaint_after)
                } else {
                    None
                };
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first().map(String::as_str) {
        match first {
            "--audio-restore-worker" => {
                let kind = args
                    .get(1)
                    .map(String::as_str)
                    .context("--audio-restore-worker <dialogue|effects|music>")?;
                return launcher::pipeline::audio_restore::run_worker(kind);
            }
            "--headless-setup" => {
                return run_headless_setup(&args[1..]);
            }
            _ => {}
        }
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .context("creating event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let wake_proxy = event_loop.create_proxy();
    let paths = LauncherPaths::discover();
    let app = LauncherApp::new(paths, move || {
        let _ = wake_proxy.send_event(UserEvent::Wake);
    });
    let mut host = Host {
        gpu: None,
        app,
        next_repaint: None,
        launch_on_exit: false,
    };
    event_loop.run_app(&mut host).context("running launcher")?;
    if host.launch_on_exit {
        let _ = launcher::runner::launch_game(&host.app.paths, &host.app.config);
    }
    Ok(())
}

struct StdoutSink;
impl launcher::pipeline::ProgressSink for StdoutSink {
    fn on_step(&self, index: u8, total: u8, label: &str) {
        println!("[step {index}/{total}] {label}");
    }
    fn on_line(&self, line: &str) {
        println!("{line}");
    }
}

fn run_headless_setup(args: &[String]) -> Result<()> {
    use launcher::config::{LauncherConfig, Platform};
    use std::path::PathBuf;
    use std::sync::{atomic::AtomicBool, Mutex};

    let mut cfg = LauncherConfig::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut val = || {
            it.next()
                .map(PathBuf::from)
                .with_context(|| format!("missing path after {flag}"))
        };
        match flag.as_str() {
            "--bf1-ps2" => {
                cfg.bf1.platform = Platform::Ps2;
                cfg.bf1.path = Some(val()?);
            }
            "--bf1-xbox" => {
                cfg.bf1.platform = Platform::Xbox;
                cfg.bf1.path = Some(val()?);
            }
            "--bf1-pc" => {
                cfg.bf1.platform = Platform::Pc;
                cfg.bf1.path = Some(val()?);
            }
            "--bf2-ps2" => {
                cfg.bf2.platform = Platform::Ps2;
                cfg.bf2.path = Some(val()?);
            }
            "--bf2-xbox" => {
                cfg.bf2.platform = Platform::Xbox;
                cfg.bf2.path = Some(val()?);
            }
            "--bf2-pc" => {
                cfg.bf2.platform = Platform::Pc;
                cfg.bf2.path = Some(val()?);
            }
            "--dlc" => {
                cfg.dlc_dir = Some(val()?);
            }
            other => anyhow::bail!("unknown --headless-setup flag: {other}"),
        }
    }
    let paths = LauncherPaths::discover();
    launcher::pipeline::run_setup(
        &paths,
        &cfg,
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(None)),
        Arc::new(StdoutSink),
    )
}
