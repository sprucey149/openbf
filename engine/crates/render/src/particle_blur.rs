use crate::{bindings, grass::GrassVertex, made, pipelines::PipelineDesc};

pub(crate) fn character_stencil(pass_op: wgpu::StencilOperation) -> wgpu::StencilState {
    let write = pass_op == wgpu::StencilOperation::Replace;
    let face = wgpu::StencilFaceState {
        compare: if write {
            wgpu::CompareFunction::Always
        } else {
            wgpu::CompareFunction::Equal
        },
        pass_op,
        ..Default::default()
    };
    wgpu::StencilState {
        front: face,
        back: face,
        read_mask: 1,
        write_mask: u32::from(write),
    }
}

pub(crate) struct ParticleBlur {
    pipeline: wgpu::RenderPipeline,
    over_character: wgpu::RenderPipeline,
    downsample: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    scene: Option<BlurScene>,
}

struct BlurScene {
    copy: wgpu::Texture,
    reduced: wgpu::TextureView,
    copy_group: wgpu::BindGroup,
    reduced_group: wgpu::BindGroup,
}

impl ParticleBlur {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals: &wgpu::BindGroupLayout,
        mask: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle blur scene layout"),
            entries: &[
                bindings::texture_layout(0, true),
                bindings::sampler_layout(1),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle blur pipeline layout"),
            bind_group_layouts: &[globals, mask, &layout],
            push_constant_ranges: &[],
        });
        let shader = made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("particle blur"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("shader.wgsl"),
                        include_str!("particle_blur.wgsl")
                    )
                    .into(),
                ),
            },
        );
        let composite = PipelineDesc {
            label: "particle blur",
            layout: &pipeline_layout,
            vs: "vs_particle_blur",
            buffers: &[GrassVertex::layout()],
            fs: "fs_particle_blur",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
        };
        let display_format = format.remove_srgb_suffix();
        let pipeline = composite.build(device, &shader, display_format);
        let over_character = PipelineDesc {
            label: "particle blur over character",
            depth_compare: Some(wgpu::CompareFunction::Greater),
            ..composite
        }
        .build_with_stencil(
            device,
            &shader,
            display_format,
            &[],
            wgpu::ColorWrites::COLOR,
            character_stencil(wgpu::StencilOperation::Keep),
        );
        let downsample_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle blur downsample layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let downsample_shader = made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("particle blur downsample"),
                source: wgpu::ShaderSource::Wgsl(include_str!("world_scale.wgsl").into()),
            },
        );
        let downsample = PipelineDesc {
            label: "particle blur downsample",
            layout: &downsample_layout,
            vs: "vs_upscale",
            buffers: &[],
            fs: "fs_upscale",
            blend: None,
            depth_write: false,
            depth_compare: None,
        }
        .build(device, &downsample_shader, format.remove_srgb_suffix());
        let sampler = made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: Some("particle blur scene sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            },
        );
        Self {
            pipeline,
            over_character,
            downsample,
            layout,
            sampler,
            scene: None,
        }
    }

    pub fn prepare(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat, size: [u32; 2]) {
        if self
            .scene
            .as_ref()
            .is_some_and(|scene| [scene.copy.width(), scene.copy.height()] == size)
        {
            return;
        }
        let display_format = format.remove_srgb_suffix();
        let copy = crate::resources::texture_2d(
            device,
            "particle blur scene copy",
            size,
            format,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            &[display_format],
        );
        let view = copy.create_view(&wgpu::TextureViewDescriptor {
            format: Some(display_format),
            ..Default::default()
        });
        let [width, height] = size;
        let reduced_size = [
            ((96.0 * width as f32 / height as f32).round() as u32).clamp(1, width),
            112.min(height),
        ];
        let reduced = crate::resources::texture_2d(
            device,
            "particle blur reduced scene",
            reduced_size,
            display_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            &[],
        )
        .create_view(&Default::default());
        let group = |view| {
            made::bind_group(
                device,
                &wgpu::BindGroupDescriptor {
                    label: Some("particle blur scene"),
                    layout: &self.layout,
                    entries: &[
                        bindings::texture_view(0, view),
                        bindings::sampler(1, &self.sampler),
                    ],
                },
            )
        };
        self.scene = Some(BlurScene {
            copy,
            copy_group: group(&view),
            reduced_group: group(&reduced),
            reduced,
        });
    }

    pub fn pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> wgpu::RenderPass<'e> {
        let scene = self.scene.as_ref().expect("prepared particle blur");
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            scene.copy.as_image_copy(),
            scene.copy.size(),
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("particle blur downsample pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scene.reduced,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.downsample);
            pass.set_bind_group(0, &scene.copy_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particle blur pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Discard,
                }),
            }),
            ..Default::default()
        })
    }

    pub fn bind_composite(&self, pass: &mut wgpu::RenderPass<'_>, over_character: bool) {
        pass.set_pipeline(if over_character {
            &self.over_character
        } else {
            &self.pipeline
        });
        let scene = self.scene.as_ref().expect("prepared particle blur");
        pass.set_bind_group(2, &scene.reduced_group, &[]);
    }
}
