//! Share the OpenXR-selected Vulkan device and queue with wgpu.

use anyhow::{anyhow, bail, Context, Result};
use ash::{vk, vk::Handle};
use openxr as xr;
use std::sync::{Arc, Mutex};
use wgpu::hal::vulkan as hal;

pub(super) fn window_renderer(
    xr: &xr::Instance,
    system: xr::SystemId,
    window: Arc<winit::window::Window>,
) -> Result<render::Renderer> {
    let requirements = xr.graphics_requirements::<xr::Vulkan>(system)?;
    // Match wgpu-hal 27's supported Vulkan instance API and intersect it with
    // the loader and runtime versions. OpenXR versions use a different encoding.
    let entry = unsafe { ash::Entry::load() }.context("load Vulkan")?;
    let available =
        unsafe { entry.try_enumerate_instance_version() }?.unwrap_or(vk::API_VERSION_1_0);
    let vk_version =
        |v: xr::Version| vk::make_api_version(0, u32::from(v.major()), u32::from(v.minor()), 0);
    let api = available
        .min(vk::API_VERSION_1_3)
        .min(vk_version(requirements.max_api_version_supported));
    if api < vk_version(requirements.min_api_version_supported) {
        bail!("the OpenXR runtime and wgpu have no compatible Vulkan API version");
    }
    // Both bindings use the Vulkan system ABI and pointer-sized dispatchable
    // handles. OpenXR forwards this loader function to the selected runtime.
    let get_proc = unsafe {
        std::mem::transmute::<vk::PFN_vkGetInstanceProcAddr, xr::sys::platform::VkGetInstanceProcAddr>(
            entry.static_fn().get_instance_proc_addr,
        )
    };
    let flags = wgpu::InstanceFlags::empty();
    let extensions = hal::Instance::desired_extensions(&entry, api, flags)?;
    let names: Vec<_> = extensions.iter().map(|name| name.as_ptr()).collect();
    let app = vk::ApplicationInfo::default()
        .application_name(c"openbf")
        .api_version(api);
    let info = vk::InstanceCreateInfo::default()
        .application_info(&app)
        .enabled_extension_names(&names);
    // OpenXR adds any runtime-specific extensions to wgpu's list before
    // creating the instance. HAL owns the resulting handle.
    let raw = unsafe { xr.create_vulkan_instance(system, get_proc, (&raw const info).cast()) }
        .context("OpenXR Vulkan instance")?
        .map_err(vk::Result::from_raw)?;
    let raw_instance =
        unsafe { ash::Instance::load(entry.static_fn(), vk::Instance::from_raw(raw as u64)) };
    let instance = unsafe {
        hal::Instance::from_raw(
            entry,
            raw_instance,
            api,
            0,
            None,
            extensions,
            flags,
            Default::default(),
            false,
            None,
        )
    }?;
    // The runtime chooses the physical device, including on hybrid GPUs.
    let physical = unsafe { xr.vulkan_graphics_device(system, raw) }
        .context("OpenXR Vulkan physical device")?;
    let exposed = instance
        .expose_adapter(vk::PhysicalDevice::from_raw(physical as u64))
        .context("the OpenXR GPU does not support wgpu")?;
    let instance = unsafe { wgpu::Instance::from_hal::<hal::Api>(instance) };
    let surface = instance
        .create_surface(window.clone())
        .context("create the VR mirror surface")?;
    let adapter = unsafe { instance.create_adapter_from_hal(exposed) };
    if !adapter.is_surface_supported(&surface) {
        bail!("the OpenXR GPU cannot present to the desktop mirror");
    }
    let desc = wgpu::DeviceDescriptor {
        label: Some("openbf OpenXR device"),
        memory_hints: wgpu::MemoryHints::Performance,
        ..Default::default()
    };
    let gpu = unsafe { adapter.as_hal::<hal::Api>() }.context("expected a Vulkan adapter")?;
    let raw_instance = gpu.shared_instance().raw_instance();
    let physical = gpu.raw_physical_device();
    let families = unsafe { raw_instance.get_physical_device_queue_family_properties(physical) };
    let surface_hal =
        unsafe { surface.as_hal::<hal::Api>() }.context("expected a Vulkan surface")?;
    let wsi = ash::khr::surface::Instance::new(gpu.shared_instance().entry(), raw_instance);
    let family = families
        .iter()
        .enumerate()
        .filter(|(_, p)| p.queue_count != 0 && p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .find_map(|(i, _)| {
            let i = i as u32;
            // The surface and physical device belong to the same instance.
            unsafe {
                wsi.get_physical_device_surface_support(physical, i, surface_hal.raw_handle())
            }
            .unwrap_or(false)
            .then_some(i)
        })
        .context("the OpenXR GPU has no graphics queue for the desktop mirror")?;
    drop(surface_hal);
    let extensions = gpu.required_device_extensions(desc.required_features);
    let names: Vec<_> = extensions.iter().map(|name| name.as_ptr()).collect();
    let mut features = gpu.physical_device_features(&extensions, desc.required_features);
    let queues = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family)
        .queue_priorities(&[1.0])];
    let info = features.add_to_device_create(
        vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_extension_names(&names),
    );
    // The runtime adds its required extensions to the HAL features and
    // creates the device on the queue family used for rendering and desktop WSI.
    let raw_device = unsafe {
        xr.create_vulkan_device(
            system,
            get_proc,
            physical.as_raw() as _,
            (&raw const info).cast(),
        )
    }
    .context("OpenXR Vulkan device")?
    .map_err(vk::Result::from_raw)?;
    let raw_device = unsafe {
        ash::Device::load(
            raw_instance.fp_v1_0(),
            vk::Device::from_raw(raw_device as u64),
        )
    };
    let open_device = unsafe {
        gpu.device_from_raw(
            raw_device,
            None,
            &extensions,
            desc.required_features,
            &desc.memory_hints,
            family,
            0,
        )
    }?;
    drop(gpu);
    let (device, queue) = unsafe { adapter.create_device_from_hal(open_device, &desc) }?;
    Ok(render::Renderer::new_windowed_with_device(
        window, surface, adapter, device, queue,
    )?)
}

pub(super) fn create_session(
    xr: &xr::Instance,
    system: xr::SystemId,
    device: &wgpu::Device,
) -> Result<(
    xr::Session<xr::Vulkan>,
    xr::FrameWaiter,
    xr::FrameStream<xr::Vulkan>,
)> {
    let gpu = unsafe { device.as_hal::<hal::Api>() }.context("expected a Vulkan device")?;
    // The application keeps this device alive until all XR resources are gone.
    Ok(unsafe {
        xr.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: gpu.shared_instance().raw_instance().handle().as_raw() as _,
                physical_device: gpu.raw_physical_device().as_raw() as _,
                device: gpu.raw_device().handle().as_raw() as _,
                queue_family_index: gpu.queue_family_index(),
                queue_index: gpu.queue_index(),
            },
        )
    }?)
}

#[derive(Clone, Copy)]
pub(super) struct Format(pub wgpu::TextureFormat, pub vk::Format);

impl Format {
    pub fn choose(session: &xr::Session<xr::Vulkan>) -> Result<Self> {
        let formats = session.enumerate_swapchain_formats()?;
        [
            Self(
                wgpu::TextureFormat::Rgba8UnormSrgb,
                vk::Format::R8G8B8A8_SRGB,
            ),
            Self(
                wgpu::TextureFormat::Bgra8UnormSrgb,
                vk::Format::B8G8R8A8_SRGB,
            ),
        ]
        .into_iter()
        .find(|format| formats.contains(&(format.1.as_raw() as u32)))
        .with_context(|| format!("OpenXR offers no supported sRGB color format: {formats:?}"))
    }
}

type Swapchain = xr::Swapchain<xr::Vulkan>;

pub(super) struct Surface {
    pub images: Vec<wgpu::TextureView>,
    pub chain: Arc<Mutex<Swapchain>>,
    pub size: [u32; 2],
}

impl Surface {
    pub fn new(
        session: &xr::Session<xr::Vulkan>,
        device: &wgpu::Device,
        size: [u32; 2],
        format: Format,
    ) -> Result<Self> {
        let [width, height] = size;
        let limit = device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > limit || height > limit {
            bail!("OpenXR target {width}x{height} exceeds device limits");
        }
        let chain = session.create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT,
            format: format.1.as_raw() as u32,
            sample_count: 1,
            width,
            height,
            face_count: 1,
            array_size: 1,
            mip_count: 1,
        })?;
        let handles = chain.enumerate_images()?;
        if handles.is_empty() {
            bail!("OpenXR swapchain has no color images");
        }
        let chain = Arc::new(Mutex::new(chain));
        let desc = wgpu::TextureDescriptor {
            label: Some("OpenXR color image"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.0,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        let gpu = unsafe { device.as_hal::<hal::Api>() }.context("expected a Vulkan device")?;
        let imported = handles
            .into_iter()
            .map(|image| {
                let keep_alive = chain.clone();
                // These render targets are owned by OpenXR, on this device. Keep
                // the swapchain alive until wgpu's last deferred image use is gone.
                unsafe {
                    gpu.texture_from_raw(
                        vk::Image::from_raw(image),
                        &wgpu::hal::TextureDescriptor {
                            label: desc.label,
                            size: desc.size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: desc.dimension,
                            format: desc.format,
                            usage: wgpu::TextureUses::COLOR_TARGET,
                            memory_flags: wgpu::hal::MemoryFlags::empty(),
                            view_formats: Vec::new(),
                        },
                        Some(Box::new(move || drop(keep_alive))),
                    )
                }
            })
            .collect::<Vec<_>>();
        drop(gpu);
        let images = imported
            .into_iter()
            .map(|texture| {
                // The first pass clears all pixels after the OpenXR wait. Every
                // use leaves the image in COLOR_ATTACHMENT_OPTIMAL for release.
                unsafe { device.create_texture_from_hal::<hal::Api>(texture, &desc) }
                    .create_view(&Default::default())
            })
            .collect();
        Ok(Self {
            images,
            chain,
            size,
        })
    }

    pub fn draw(&self, paint: impl FnOnce(&wgpu::TextureView)) -> Result<()> {
        let mut chain = self.chain.lock().unwrap();
        let index = chain.acquire_image()? as usize;
        chain.wait_image(xr::Duration::INFINITE)?;
        let view = self
            .images
            .get(index)
            .ok_or_else(|| anyhow!("invalid OpenXR image index"));
        if let Ok(view) = &view {
            paint(view);
        }
        chain.release_image()?;
        view.map(|_| ())
    }

    pub fn rect(&self) -> xr::Rect2Di {
        xr::Rect2Di {
            offset: xr::Offset2Di { x: 0, y: 0 },
            extent: xr::Extent2Di {
                width: self.size[0] as i32,
                height: self.size[1] as i32,
            },
        }
    }
}

pub(super) fn clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear OpenXR color image"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
}
