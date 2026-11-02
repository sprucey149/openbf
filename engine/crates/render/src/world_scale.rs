//! The world drawn smaller than the window, for the low-power setting: every
//! world pass runs at the scaled size in a target of its own, the result is
//! upscaled into the frame, and the HUD then draws at the window's full size so
//! it stays sharp. At a scale of 1 none of this exists: the world draws straight
//! into the frame, so the setting costs nothing when it is off.
//!
//! The scale comes from the game (`game::dyn_res`). The projection keeps the
//! window's shape whatever the scale.

use crate::pipelines::PipelineDesc;

/// A world target of one size. Handles, so a frame can hold it while it draws.
#[derive(Clone)]
pub(crate) struct WorldTarget {
    pub size: [u32; 2],
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

/// The size the world draws at: the window's times `scale`, at least a pixel.
pub(crate) fn scaled((w, h): (u32, u32), scale: f32) -> (u32, u32) {
    let at = |side: u32| ((side as f32 * scale).round() as u32).clamp(1, side);
    (at(w), at(h))
}

/// The upscale's pipeline and the targets made so far. A controller moves among
/// a few sizes, so each is made once.
pub(crate) struct WorldScale {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: Vec<WorldTarget>,
}

impl WorldScale {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world upscale layout"),
            entries: &[
                crate::bindings::texture_layout(0, true),
                crate::bindings::sampler_layout(1),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world upscale"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("world upscale"),
                source: wgpu::ShaderSource::Wgsl(include_str!("world_scale.wgsl").into()),
            },
        );
        let pipeline = PipelineDesc {
            label: "world upscale",
            layout: &pipeline_layout,
            vs: "vs_upscale",
            buffers: &[],
            fs: "fs_upscale",
            blend: None,
            depth_write: false,
            depth_compare: None,
        }
        .build(device, &shader, format);
        let sampler = crate::made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: Some("world upscale"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            },
        );
        Self {
            pipeline,
            layout,
            sampler,
            targets: Vec::new(),
        }
    }

    /// The target of `size`, made on first use.
    pub fn target(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
    ) -> WorldTarget {
        if let Some(t) = self.targets.iter().find(|t| t.size == size) {
            return t.clone();
        }
        let texture = crate::resources::texture_2d(
            device,
            "world target",
            size,
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            &[format.remove_srgb_suffix()],
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("world upscale"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            },
        );
        let target = WorldTarget {
            size,
            texture,
            view,
            depth: crate::device::create_depth_texture(device, size[0], size[1]),
            bind_group,
        };
        self.targets.push(target.clone());
        target
    }

    /// Forget the targets: the window changed, and the sizes with it.
    pub fn clear(&mut self) {
        self.targets.clear();
    }

    /// Draw `target` over the whole of `frame`.
    pub fn upscale(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &WorldTarget,
        frame: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world upscale"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &target.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scaled_size_rounds_and_never_leaves_the_window() {
        assert_eq!(scaled((1920, 1080), 1.0), (1920, 1080));
        assert_eq!(scaled((1920, 1080), 0.5), (960, 540));
        assert_eq!(scaled((1920, 1080), 0.75), (1440, 810));
        assert_eq!(scaled((3, 1), 0.01), (1, 1));
        assert_eq!(scaled((100, 100), 2.0), (100, 100));
    }
}
