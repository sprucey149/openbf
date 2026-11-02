//! The GPU side of a `Renderer`: the two constructors (`new_windowed`,
//! `new_headless`) — the adapter and device (`request_device`, which honours
//! `OPENBF_ADAPTER`) and the window surface or the headless texture
//! ([`HEADLESS_FORMAT`]; the surface composites `Opaque` when it can,
//! `pick_alpha_mode`) — the depth target, the meshes and textures uploaded
//! to the device, and the size, resize and idle queries. Both constructors
//! finish in `finish_init` (`pipelines.rs`).

use crate::mesh::GpuMesh;
use crate::texture::GpuTexture;
use crate::{
    MeshData, MeshHandle, RenderError, Renderer, SkinnedMeshData, SkinnedMeshHandle, Target,
    TextureDesc, TextureHandle,
};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::ThreadId;

/// Format used for the headless render target (and thus PNG captures).
pub const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Whose turn it is to use Vulkan in this process, and how many renderers that
/// thread holds.
static GPU_TURN: (Mutex<Option<(ThreadId, u32)>>, Condvar) = (Mutex::new(None), Condvar::new());

/// A renderer's turn at the GPU, from before its Vulkan instance is created to
/// after its device is dropped.
///
/// The Vulkan loader walks its lists of instances and devices with no lock, so
/// one thread making or dropping a renderer crashes another thread that is
/// naming an object or listing extensions (`loader_get_icd_and_device` and
/// `terminator_EnumerateInstanceExtensionProperties`). A device making objects
/// is enough to be hit, so taking turns only to create one would not help: the
/// turn lasts as long as the renderer. The test harness runs tests on parallel
/// threads, so this is what keeps a test binary from needing `--test-threads=1`.
///
/// The thread holding a turn may make more renderers. Another thread waits, so
/// a renderer must not wait on another thread that is making one.
pub(crate) struct GpuTurn(());

impl GpuTurn {
    pub(crate) fn take() -> Self {
        let (holder, free) = &GPU_TURN;
        let me = std::thread::current().id();
        let mut held = holder.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match &mut *held {
                None => {
                    *held = Some((me, 1));
                    return Self(());
                }
                Some((owner, count)) if *owner == me => {
                    *count += 1;
                    return Self(());
                }
                Some(_) => held = free.wait(held).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }
}

impl Drop for GpuTurn {
    fn drop(&mut self) {
        let (holder, free) = &GPU_TURN;
        let mut held = holder.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, count)) = &mut *held {
            *count -= 1;
            if *count == 0 {
                *held = None;
                free.notify_all();
            }
        }
    }
}

impl Renderer {
    /// Windowed renderer. The surface takes its own `Arc` on the window, so the
    /// caller keeps ownership of theirs for event handling.
    pub fn new_windowed(window: Arc<winit::window::Window>) -> Result<Self, RenderError> {
        let turn = GpuTurn::take();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let (adapter, device, queue) = request_device(&instance, Some(&surface))?;

        // sRGB surface format selection: an earlier wgpu project's context.rs `from_parts`.
        let caps = surface.get_capabilities(&adapter);
        // An adapter that cannot present to this window reports no formats —
        // reachable when `OPENBF_ADAPTER` picks it, since that path skips the
        // compatible-surface request — so that is an error, not an index panic.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| {
                RenderError::Surface(format!(
                    "adapter {:?} cannot present to this window",
                    adapter.get_info().name
                ))
            })?;
        let alpha_mode = pick_alpha_mode(&caps.alpha_modes);
        println!(
            "surface alpha modes {:?} -> {alpha_mode:?}",
            caps.alpha_modes
        );
        let size = window.inner_size();
        // COPY_SRC where the surface allows it: BF2's terrain light pass
        // reads a copy of the frame (`terrain_light.rs`).
        let usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | (caps.usages & wgpu::TextureUsages::COPY_SRC);
        // The HUD pass blends display values, as the PS2 does, through a
        // non-sRGB view of the frame (`Renderer::display_view_format`) —
        // where the backend lets a surface carry one.
        let plain = format.remove_srgb_suffix();
        let view_formats = if plain != format
            && adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS)
        {
            vec![plain]
        } else {
            vec![]
        };
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: present_mode(),
            alpha_mode,
            view_formats,
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let info = adapter.get_info();
        Self::finish_init(
            device,
            queue,
            Target::Window { surface, config },
            format,
            info,
            turn,
        )
    }

    /// Headless renderer: offscreen `Rgba8UnormSrgb` target, no window or display
    /// needed (adapter requested with `compatible_surface: None`).
    pub fn new_headless(width: u32, height: u32) -> Result<Self, RenderError> {
        assert!(width > 0 && height > 0, "headless target must be non-empty");
        let turn = GpuTurn::take();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let (adapter, device, queue) = request_device(&instance, None)?;
        let info = adapter.get_info();

        let texture = crate::resources::texture_2d(
            &device,
            "headless color",
            [width, height],
            HEADLESS_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            // For the HUD pass (`Renderer::display_view_format`).
            &[HEADLESS_FORMAT.remove_srgb_suffix()],
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self::finish_init(
            device,
            queue,
            Target::Headless {
                texture,
                view,
                width,
                height,
            },
            HEADLESS_FORMAT,
            info,
            turn,
        )
    }

    /// The adapter this renderer is running on.
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    // ── Resources ────────────────────────────────────────────────────────────

    pub fn add_mesh(&mut self, data: &MeshData<'_>) -> Result<MeshHandle, RenderError> {
        let mesh = GpuMesh::upload(&self.device, data)?;
        self.meshes.push(mesh);
        Ok(MeshHandle(self.meshes.len() as u32 - 1))
    }

    pub fn add_skinned_mesh(
        &mut self,
        data: &SkinnedMeshData<'_>,
    ) -> Result<SkinnedMeshHandle, RenderError> {
        let mesh = GpuMesh::upload_skinned(&self.device, data)?;
        self.skinned_meshes.push(mesh);
        Ok(SkinnedMeshHandle(self.skinned_meshes.len() as u32 - 1))
    }

    pub fn add_texture(&mut self, desc: &TextureDesc<'_>) -> Result<TextureHandle, RenderError> {
        let tex = GpuTexture::upload(&self.device, &self.queue, &self.texture_bgl, desc)?;
        self.textures.push(tex);
        Ok(TextureHandle(self.textures.len() as u32 - 1))
    }

    // ── Misc ─────────────────────────────────────────────────────────────────

    /// Current target size in pixels.
    pub fn size(&self) -> (u32, u32) {
        match &self.target {
            Target::Window { config, .. } => (config.width, config.height),
            Target::Headless { width, height, .. } => (*width, *height),
        }
    }

    /// Windowed only: reconfigure surface + depth on window resize (no-op headless).
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Target::Window { surface, config } = &mut self.target {
            if width == 0 || height == 0 {
                return; // minimized; keep old config
            }
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
            self.depth_view = create_depth_texture(&self.device, width, height);
            self.world_scale.clear();
        }
    }

    /// The frame's format without its sRGB encoding, when a view of the
    /// frame may be made in it: a pass drawing through such a view writes
    /// and blends DISPLAY values (the HUD pass, `crate::hud`). `None` where
    /// the window surface cannot carry the second view format.
    pub fn display_view_format(&self) -> Option<wgpu::TextureFormat> {
        match &self.target {
            Target::Window { config, .. } => config.view_formats.first().copied(),
            Target::Headless { .. } => Some(HEADLESS_FORMAT.remove_srgb_suffix()),
        }
    }

    /// Escape hatch for the egui overlay, which builds its own renderer.
    pub fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    /// Block until the GPU has finished everything submitted so far.
    ///
    /// `render_frame` returns as soon as the queue is submitted, so timing it
    /// alone measures command encoding. `--bench` needs the GPU's own cost and
    /// should not pay for a pixel readback to get it.
    pub fn wait_idle(&self) -> Result<(), RenderError> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map(|_| ())
            .map_err(|e| RenderError::Device(e.to_string()))
    }
}

/// Adapter + device request; `surface: None` is the headless path (works with no
/// display). Adapted from an earlier wgpu project's context.rs `WgpuContext::from_parts`; the BC
/// texture-compression opt-in was dropped — bake output is RGBA8 for now.
///
/// `OPENBF_ADAPTER` overrides the default HighPerformance preference so the same
/// binary can be measured on either GPU: `igpu` / `dgpu` pick by device type,
/// anything else is matched as a case-insensitive substring of the adapter name.
/// It is an env var rather than a flag because every entry point that builds a
/// Renderer (the game, the gallery, the golden tests) needs it and none of them
/// should grow a parameter for a measurement knob. A software adapter is never
/// selected implicitly — measuring llvmpipe by accident is worse than failing.
fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), RenderError> {
    let adapter = match std::env::var("OPENBF_ADAPTER") {
        Ok(want) if !want.is_empty() => pick_adapter(instance, &want)?,
        _ => pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            // Prefer the discrete NVIDIA adapter over the Intel iGPU.
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
        }))
        .map_err(|e| RenderError::Adapter(e.to_string()))?,
    };

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("swbf render device"),
        // Timestamp queries only for `OPENBF_GPU_TIMING=1` (`gpu_timer.rs`).
        required_features: crate::gpu_timer::wanted_features(&adapter),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .map_err(|e| RenderError::Device(e.to_string()))?;
    Ok((adapter, device, queue))
}

/// The window's present mode: vsync, unless `OPENBF_NO_VSYNC=1` (a
/// measurement knob, card ISPD: with vsync a frame's time is the display's,
/// whatever the frame cost).
fn present_mode() -> wgpu::PresentMode {
    if std::env::var("OPENBF_NO_VSYNC").as_deref() == Ok("1") {
        wgpu::PresentMode::AutoNoVsync
    } else {
        wgpu::PresentMode::AutoVsync
    }
}

/// Select an adapter by `OPENBF_ADAPTER` (see `request_device`). Errors list
/// what was available, because a silent fallback to the wrong GPU would make
/// every number printed afterwards a lie.
fn pick_adapter(instance: &wgpu::Instance, want: &str) -> Result<wgpu::Adapter, RenderError> {
    use wgpu::DeviceType;
    let all = instance.enumerate_adapters(wgpu::Backends::PRIMARY);
    let want_lc = want.to_ascii_lowercase();
    let hit = all.iter().find(|a| {
        let info = a.get_info();
        match want_lc.as_str() {
            "igpu" => info.device_type == DeviceType::IntegratedGpu,
            "dgpu" => info.device_type == DeviceType::DiscreteGpu,
            s => info.name.to_ascii_lowercase().contains(s),
        }
    });
    match hit {
        Some(a) => Ok(a.clone()),
        None => {
            let have: Vec<String> = all
                .iter()
                .map(|a| {
                    let i = a.get_info();
                    format!("{} ({:?}, {:?})", i.name, i.device_type, i.backend)
                })
                .collect();
            Err(RenderError::Adapter(format!(
                "OPENBF_ADAPTER={want} matched no adapter; available: {}",
                have.join("; ")
            )))
        }
    }
}

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

/// Depth24PlusStencil8 depth target, matching the pipeline's depth-stencil state.
/// From an earlier wgpu project's renderer/mod.rs `create_depth_texture` (binding usage dropped).
pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = crate::resources::texture_2d(
        device,
        "depth",
        [width, height],
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        &[],
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// The window surface's composite mode: `Opaque` whenever the surface lists
/// it, so the desktop never shows through the frame (card FALPH). The frame's
/// alpha is 1.0 anyway (`FRAME_WRITES`); this is the second lock, for a
/// compositor that would otherwise take a listed `PreMultiplied` first.
/// Without `Opaque`, the surface's first mode (or `Auto`) as before.
fn pick_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode::{Auto, Opaque};
    if modes.contains(&Opaque) {
        Opaque
    } else {
        modes.first().copied().unwrap_or(Auto)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_one_thread_holds_a_gpu_turn_and_that_thread_may_take_another() {
        use super::GpuTurn;
        use std::sync::atomic::{AtomicU32, Ordering};
        static HOLDERS: AtomicU32 = AtomicU32::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    for _ in 0..50 {
                        let first = GpuTurn::take();
                        assert_eq!(HOLDERS.fetch_add(1, Ordering::SeqCst), 0);
                        // Its own second turn does not wait on itself.
                        let second = GpuTurn::take();
                        drop(first);
                        std::thread::yield_now();
                        assert_eq!(HOLDERS.fetch_sub(1, Ordering::SeqCst), 1);
                        drop(second);
                    }
                });
            }
        });
    }

    use super::*;
    use wgpu::CompositeAlphaMode::{Auto, Inherit, Opaque, PostMultiplied, PreMultiplied};

    #[test]
    fn the_surface_is_opaque_whenever_it_can_be() {
        assert_eq!(pick_alpha_mode(&[PreMultiplied, Opaque]), Opaque);
        assert_eq!(pick_alpha_mode(&[PostMultiplied, Inherit, Opaque]), Opaque);
        assert_eq!(pick_alpha_mode(&[Inherit, PreMultiplied]), Inherit);
        assert_eq!(pick_alpha_mode(&[]), Auto);
    }
}
