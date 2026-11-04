use crate::{bindings, Camera, Renderer};
use glam::Vec3;

mod geometry;

#[derive(Debug, Clone, PartialEq)]
pub struct SunFlare {
    pub display_gain: f32,
    pub distance: f32,
    pub angle: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub flare_out_size: f32,
    pub num_flare_outs: u32,
    pub initial_alpha: f32,
    pub halo_rings: [[f32; 5]; 3],
    pub spike_color: [f32; 4],
    pub spike_size: f32,
}

impl SunFlare {
    pub fn direction(&self) -> Vec3 {
        let [a, b] = self.angle.map(f32::to_radians);
        Vec3::new(a.sin() * b.cos(), -a.cos(), a.sin() * b.sin())
    }
}

pub(crate) struct State {
    config: SunFlare,
    pipeline: wgpu::RenderPipeline,
    gather: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    vertices: wgpu::Buffer,
}

impl State {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, config: SunFlare) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sun flare layout"),
            entries: &[
                bindings::uniform_layout(0, wgpu::ShaderStages::VERTEX_FRAGMENT, false, None),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sun flare pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("sun flare shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("sun_flare.wgsl").into()),
            },
        );
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let desc = crate::pipelines::PipelineDesc {
            label: "sun flare pipeline",
            layout: &pipeline_layout,
            vs: "vs_main",
            fs: "fs_main",
            buffers: &[geometry::Vertex::layout()],
            blend: Some(wgpu::BlendState {
                color: add,
                alpha: add,
            }),
            depth_write: false,
            depth_compare: None,
        };
        let pipeline = desc.build(device, &shader, format.remove_srgb_suffix());
        let gather = crate::pipelines::PipelineDesc {
            fs: "fs_bloom",
            ..desc
        }
        .build(device, &shader, wgpu::TextureFormat::Rgba16Float);
        let params = crate::resources::buffer(
            device,
            "sun flare params",
            16 * 10,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let vertices = crate::resources::buffer(
            device,
            "sun flare triangles",
            geometry::BUFFER_BYTES,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        Self {
            config,
            pipeline,
            gather,
            layout,
            params,
            vertices,
        }
    }
}

impl Renderer {
    pub fn set_sun_flare(&mut self, config: Option<SunFlare>) {
        self.sun_flare = config.map(|c| State::new(&self.device, self.format, c));
    }

    pub(crate) fn sun_flare_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &Camera,
        width: u32,
        height: u32,
        gather: bool,
    ) -> u32 {
        let Some(state) = &self.sun_flare else {
            return 0;
        };
        let config = &state.config;
        let dir = config.direction();
        let facing = dir.dot(camera.forward());
        if facing <= 0.0 {
            return 0;
        }
        let clip = camera.view_proj(width as f32 / height as f32)
            * (camera.position + dir * (camera.far * 0.5)).extend(1.0);
        let uv = [clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5];
        let focal = height as f32 / (2.0 * (camera.fov_y * 0.5).tan());
        let angular_scale = focal / (facing * 120.0);
        let mut params = [[0.0f32; 4]; 10];
        params[0] = [
            uv[0] * width as f32,
            uv[1] * height as f32,
            angular_scale,
            config.size,
        ];
        params[1] = geometry::gs(config.color);
        for (i, ring) in config.halo_rings.iter().enumerate() {
            params[2 + i] = geometry::gs([ring[1], ring[2], ring[3], ring[4]]);
        }
        params[5] = [
            config.halo_rings[0][0],
            config.halo_rings[1][0],
            config.halo_rings[2][0],
            config.spike_size,
        ];
        params[6] = geometry::gs(config.spike_color);
        params[7] = [
            config.flare_out_size,
            config.num_flare_outs as f32,
            config.initial_alpha,
            config.color[..3].iter().copied().fold(0.0, f32::max),
        ];
        params[8] = [width as f32, height as f32, config.display_gain, 0.0];
        let cos_half = (1.0 + ((camera.fov_y * 0.5).tan() * (0.637_07 / 0.488_378)).powi(2))
            .sqrt()
            .recip();
        params[9] = [
            focal,
            facing,
            config.distance,
            ((facing - cos_half) / (1.0 - cos_half)).max(0.0),
        ];
        let vertices = geometry::build(config, &params);
        if vertices.is_empty() {
            return 0;
        }
        self.queue
            .write_buffer(&state.vertices, 0, bytemuck::cast_slice(&vertices));
        self.queue
            .write_buffer(&state.params, 0, bytemuck::cast_slice(&params));
        let depth = self
            .depth_view
            .texture()
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("sun flare source depth"),
                aspect: wgpu::TextureAspect::DepthOnly,
                ..Default::default()
            });
        let group = crate::made::bind_group(
            &self.device,
            &wgpu::BindGroupDescriptor {
                label: Some("sun flare group"),
                layout: &state.layout,
                entries: &[
                    bindings::entire_buffer(0, &state.params),
                    bindings::texture_view(1, &depth),
                ],
            },
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sun flare"),
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
        pass.set_pipeline(if gather {
            &state.gather
        } else {
            &state.pipeline
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_vertex_buffer(0, state.vertices.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
        1
    }
}
