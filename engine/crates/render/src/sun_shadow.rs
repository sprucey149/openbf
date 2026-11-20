//! Camera-local directional shadows from the current world and skinned meshes.
//! One bilinear depth comparison antialiases the edge of the silhouette.
//! The opaque scene receives the shadow before soldiers and water draw.

use crate::bindings;
use crate::frame::draw_mesh;
use crate::mesh::Instance;
use crate::{DrawCmd, DrawLayer, Renderer, SkinnedDrawCmd, SkinnedVertex, Vertex};
use glam::{Mat4, Vec3};

// OURS: fit nearby battle geometry with ~1.6 cm texels and fade before the
// edge of coverage. measure: fixed-camera BF1 PC shadow distance and luma.
const HALF_SPAN: f32 = 32.0;
const LIGHT_DEPTH: f32 = 192.0;
const FADE_START: f32 = 24.0;
const FADE_END: f32 = 31.0;
const RECEIVER_BIAS: f32 = 0.055;
// OURS: nearby lit and shadowed floor patches in the supplied PC screenshot
// had median display luma of about 185 and 125. measure: a locked-camera pair.
const SHADOW_TRANSMITTANCE: f32 = 0.68;

pub(crate) fn caster_key(draw: &DrawCmd) -> (bool, u32, Option<u32>) {
    let masked = draw.layer == DrawLayer::Mask;
    (
        masked,
        draw.mesh.0,
        draw.texture.filter(|_| masked).map(|t| t.0),
    )
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowView {
    light_clip: [[f32; 4]; 4],
    world_from_clip: [[f32; 4]; 4],
    eye_and_time: [f32; 4],
    params: [f32; 4],
}

pub(crate) struct SunShadow {
    size: u32,
    depth: wgpu::TextureView,
    uniform: wgpu::Buffer,
    camera: wgpu::BindGroup,
    rigid: wgpu::RenderPipeline,
    mask: wgpu::RenderPipeline,
    skin: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    compare: wgpu::Sampler,
    display_format: wgpu::TextureFormat,
}

impl SunShadow {
    fn new(r: &Renderer) -> Self {
        let device = &r.device;
        let size = 4096.min(device.limits().max_texture_dimension_2d);
        let depth_format = wgpu::TextureFormat::Depth16Unorm;
        let depth = crate::resources::texture_2d(
            device,
            "sun shadow depth",
            [size, size],
            depth_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            &[],
        )
        .create_view(&Default::default());
        let uniform = crate::resources::buffer(
            device,
            "sun shadow view",
            std::mem::size_of::<ShadowView>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sun shadows"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sun_shadow.wgsl").into()),
        });
        let uniform_entry =
            bindings::uniform_layout(0, wgpu::ShaderStages::VERTEX_FRAGMENT, false, None);
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sun shadow camera"),
            entries: &[uniform_entry],
        });
        let camera = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sun shadow camera"),
            layout: &camera_layout,
            entries: &[bindings::entire_buffer(0, &uniform)],
        });
        let caster_layout = |groups: &[&wgpu::BindGroupLayout]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sun shadow caster"),
                bind_group_layouts: groups,
                push_constant_ranges: &[],
            })
        };
        let caster = |layout: &wgpu::PipelineLayout, vs, buffers: &[_], masked: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sun shadow caster"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    buffers,
                    compilation_options: Default::default(),
                },
                fragment: masked.then_some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask"),
                    targets: &[],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 1.5,
                        clamp: 0.0,
                    },
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let rigid_buffers = [Vertex::layout(), Instance::rigid_layout()];
        let rigid = caster(
            &caster_layout(&[&camera_layout]),
            "vs_rigid",
            &rigid_buffers,
            false,
        );
        let mask = caster(
            &caster_layout(&[&camera_layout, &r.texture_bgl]),
            "vs_rigid",
            &rigid_buffers,
            true,
        );
        let skin = caster(
            &caster_layout(&[&camera_layout, &r.texture_bgl, &r.palette_bgl]),
            "vs_skin",
            &[SkinnedVertex::layout(), Instance::layout()],
            false,
        );
        let depth_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sun shadow receiver"),
            entries: &[
                uniform_entry,
                depth_entry(1),
                depth_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let compare = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sun shadow edge coverage"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let display_format = r.water_display_format.unwrap_or(r.format);
        let composite = crate::pipelines::PipelineDesc {
            label: "sun shadow composite",
            layout: &caster_layout(&[&composite_layout]),
            vs: "vs_composite",
            buffers: &[],
            fs: "fs_composite",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            depth_compare: None,
        }
        .build(device, &shader, display_format);
        Self {
            size,
            depth,
            uniform,
            camera,
            rigid,
            mask,
            skin,
            composite,
            composite_layout,
            compare,
            display_format,
        }
    }

    pub fn light_clip(&self, eye: Vec3, direction: Vec3) -> Mat4 {
        let up = if direction.y.abs() > 0.95 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let rotation = Mat4::look_to_rh(Vec3::ZERO, direction, up);
        let texels_per_metre = self.size as f32 / (2.0 * HALF_SPAN);
        let mut anchor = rotation.transform_point3(eye);
        anchor.x = (anchor.x * texels_per_metre).round() / texels_per_metre;
        anchor.y = (anchor.y * texels_per_metre).round() / texels_per_metre;
        let view = Mat4::from_translation(-anchor - Vec3::Z * (LIGHT_DEPTH * 0.75)) * rotation;
        let projection = Mat4::orthographic_rh(
            -HALF_SPAN,
            HALF_SPAN,
            -HALF_SPAN,
            HALF_SPAN,
            0.0,
            LIGHT_DEPTH,
        );
        projection * view
    }

    pub fn prepare(&self, r: &Renderer, light_clip: Mat4, view_proj: Mat4, eye: Vec3, time: f32) {
        let opacity = if self.display_format.is_srgb() {
            1.0 - SHADOW_TRANSMITTANCE.powf(2.2)
        } else {
            1.0 - SHADOW_TRANSMITTANCE
        };
        let view = ShadowView {
            light_clip: light_clip.to_cols_array_2d(),
            world_from_clip: view_proj.inverse().to_cols_array_2d(),
            eye_and_time: eye.extend(time).to_array(),
            params: [FADE_START, FADE_END, RECEIVER_BIAS / LIGHT_DEPTH, opacity],
        };
        r.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&view));
    }

    pub fn draw_casters(
        &self,
        r: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        draws: &[DrawCmd],
        casters: &[usize],
        first_instance: usize,
        skinned: &[SkinnedDrawCmd],
        skin_casters: &[usize],
    ) -> u32 {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sun shadow depth"),
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        let mut draw_calls = 0;
        pass.set_bind_group(0, &self.camera, &[]);
        pass.set_vertex_buffer(1, r.instance_buf.slice(..));
        let key = |i: usize| caster_key(&draws[i]);
        let mut first = 0;
        let mut last_masked = None;
        for run in casters.chunk_by(|a, b| key(*a) == key(*b)) {
            let (masked, mesh, _) = key(run[0]);
            if last_masked != Some(masked) {
                pass.set_pipeline(if masked { &self.mask } else { &self.rigid });
                last_masked = Some(masked);
            }
            if masked {
                pass.set_bind_group(1, r.tex_bg(draws[run[0]].texture), &[]);
            }
            let from = (first_instance + first) as u32;
            first += run.len();
            draw_mesh(
                &mut pass,
                &r.meshes[mesh as usize],
                from..(first_instance + first) as u32,
            );
            draw_calls += 1;
        }
        if !skin_casters.is_empty() {
            pass.set_pipeline(&self.skin);
            pass.set_bind_group(1, r.tex_bg(None), &[]);
            pass.set_vertex_buffer(1, r.skinned_instance_buf.slice(..));
            for &i in skin_casters {
                pass.set_bind_group(2, &r.palette_bg, &[(i * crate::PALETTE_STRIDE) as u32]);
                draw_mesh(
                    &mut pass,
                    &r.skinned_meshes[skinned[i].mesh.0 as usize],
                    i as u32..i as u32 + 1,
                );
                draw_calls += 1;
            }
        }
        draw_calls
    }

    pub fn composite(
        &self,
        r: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Texture,
    ) {
        let scene_depth = r
            .depth_view
            .texture()
            .create_view(&wgpu::TextureViewDescriptor {
                aspect: wgpu::TextureAspect::DepthOnly,
                ..Default::default()
            });
        let group = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sun shadow receiver"),
            layout: &self.composite_layout,
            entries: &[
                bindings::entire_buffer(0, &self.uniform),
                bindings::texture_view(1, &scene_depth),
                bindings::texture_view(2, &self.depth),
                bindings::sampler(3, &self.compare),
            ],
        });
        let display = frame.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.display_format),
            ..Default::default()
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sun shadow composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &display,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.composite);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl Renderer {
    /// Use hard directional shadows in place of the authored vehicle volume pass.
    pub fn set_sun_shadows(&mut self, enabled: bool) {
        if self.sun_shadow.is_some() != enabled {
            self.sun_shadow = enabled.then(|| SunShadow::new(self));
        }
    }

    pub fn sun_shadows_enabled(&self) -> bool {
        self.sun_shadow.is_some() && self.lighting.sun_dir.try_normalize().is_some()
    }
}
