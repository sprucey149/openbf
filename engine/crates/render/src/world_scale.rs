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
    /// The split views' second colour target per size, sharing the first's depth
    /// ([`WorldScale::bloom_out`]).
    outs: Vec<WorldTarget>,
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
            outs: Vec::new(),
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
        let target = self.make_target(device, format, size);
        self.targets.push(target.clone());
        target
    }

    /// [ours] A split view's bloom composite target: a second colour target of `world`'s size
    /// that shares its depth. The composite reads `world` and writes this one, so the frame is
    /// not copied first (`Renderer::bloom_pass`); the passes after the bloom draw here and the
    /// upscale reads it.
    pub fn bloom_out(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        world: &WorldTarget,
    ) -> WorldTarget {
        if let Some(t) = self.outs.iter().find(|t| t.size == world.size) {
            return t.clone();
        }
        let target = self.target_with_depth(device, format, world.size, world.depth.clone());
        self.outs.push(target.clone());
        target
    }

    pub(super) fn make_target(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
    ) -> WorldTarget {
        let depth = crate::device::create_depth_texture(device, size[0], size[1]);
        self.target_with_depth(device, format, size, depth)
    }

    fn target_with_depth(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        depth: wgpu::TextureView,
    ) -> WorldTarget {
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
        WorldTarget {
            size,
            texture,
            view,
            depth,
            bind_group,
        }
    }

    /// Forget the targets: the window changed, and the sizes with it.
    pub fn clear(&mut self) {
        self.targets.clear();
        self.outs.clear();
    }

    /// Draw `target` over the whole of `frame` (the tests' blit; a frame draws through
    /// [`WorldScale::upscale_into`]).
    #[cfg(test)]
    pub fn upscale(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &WorldTarget,
        frame: &wgpu::TextureView,
    ) {
        self.upscale_into(encoder, target, frame, None, true);
    }

    /// Draw `target` over `rect` (x, y, width, height in frame pixels) of `frame`, or all of it.
    /// `clear` blacks the frame first; the later views of one present keep what the earlier ones
    /// drew (issue #194).
    pub fn upscale_into(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &WorldTarget,
        frame: &wgpu::TextureView,
        rect: Option<[u32; 4]>,
        clear: bool,
    ) {
        let viewport = rect.map(|[x, y, w, h]| [x as f32, y as f32, w as f32, h as f32]);
        self.draw(encoder, target, frame, viewport, rect, clear);
    }

    /// Draw `target` into the `viewport` (x, y, width, height, fractional frame pixels) of a
    /// cleared `frame`: the desktop mirror's letterbox.
    pub(super) fn blit(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &WorldTarget,
        frame: &wgpu::TextureView,
        viewport: Option<[f32; 4]>,
    ) {
        self.draw(encoder, target, frame, viewport, None, true);
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &WorldTarget,
        frame: &wgpu::TextureView,
        viewport: Option<[f32; 4]>,
        scissor: Option<[u32; 4]>,
        clear: bool,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world upscale"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        if let Some([x, y, w, h]) = scissor {
            pass.set_scissor_rect(x, y, w, h);
        }
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
