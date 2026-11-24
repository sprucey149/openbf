//! PC Battlefront II planar reflection and refraction for [`crate::LightPath::WaterNormal`].
//!
//! When a frame submits at least one `WaterNormal` surface draw:
//! 1. Static `Opaque` and `Mask` geometry above the water plane (plus sky shells) is
//!    rendered into a half-resolution reflection target using `view_proj * mirror_y(water_y)`
//!    with a fragment clip at `water_clip_y` (valid directly because `PipelineDesc` renders
//!    with `cull_mode: None`).
//! 2. Immediately before the main `DrawLayer::Water` pass, the already-rendered opaque scene
//!    is copied to `scene_tex` for refracting and tinting underwater surfaces by `RefractionColor`.
//! 3. `water_normal.wgsl` perturbs both render-target lookups with the animated water normal
//!    map, adds sun specular, and blends refraction and reflection via the authored Fresnel band.

use crate::bindings;
use crate::device::create_depth_texture;
use crate::frame::draw_mesh;
use crate::frustum::Frustum;
use crate::mesh::Instance;
use crate::pipelines::PipelineDesc;
use crate::{Camera, DrawCmd, DrawLayer, Globals, LightPath, Renderer, SkyShell};

/// Non-sRGB `@group(3)` targets plus cached per-size render/copy views.
#[derive(Clone)]
pub(crate) struct WaterTargets {
    pub size: [u32; 2],
    pub scene_tex: wgpu::Texture,
    reflect_tex: wgpu::Texture,
    pub reflect_view: wgpu::TextureView,
    pub reflect_depth: wgpu::TextureView,
    pub bg: wgpu::BindGroup,
}

pub(crate) struct WaterReflectState {
    pub pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    pub reflect_globals_buf: wgpu::Buffer,
    pub reflect_globals_bg: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    targets: Vec<WaterTargets>,
}

/// Prepared planar reflection draw list and mirrored GPU instances for one frame.
pub(crate) struct ReflectSetup {
    pub water_y: f32,
    pub sky_base: usize,
    pub world_base: usize,
    pub order: Vec<usize>,
    pub globals: Globals,
}

impl WaterReflectState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        globals_bgl: &wgpu::BindGroupLayout,
        texture_bgl: &wgpu::BindGroupLayout,
        palette_bgl: &wgpu::BindGroupLayout,
        points_buf: &wgpu::Buffer,
        _format: wgpu::TextureFormat,
        display_target: wgpu::TextureFormat,
        use_display_fs: bool,
    ) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water normal bgl"),
            entries: &[
                bindings::texture_layout(3, true),
                bindings::texture_layout(4, true),
                bindings::sampler_layout(5),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water normal pipeline layout"),
            bind_group_layouts: &[globals_bgl, texture_bgl, palette_bgl, &bgl],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("water normal shader"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("shader.wgsl"),
                        include_str!("water_normal.wgsl")
                    )
                    .into(),
                ),
            },
        );
        let pipeline = PipelineDesc {
            label: "map water normal pipeline",
            layout: &layout,
            vs: "vs_main",
            buffers: &[crate::Vertex::layout(), Instance::rigid_layout()],
            fs: if use_display_fs {
                "fs_water_normal_display"
            } else {
                "fs_water_normal"
            },
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: true,
            depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
        }
        .build(device, &shader, display_target);

        let reflect_globals_buf = crate::resources::buffer(
            device,
            "water reflect globals",
            std::mem::size_of::<Globals>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let reflect_globals_bg = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("water reflect globals bg"),
                layout: globals_bgl,
                entries: &[
                    bindings::entire_buffer(0, &reflect_globals_buf),
                    bindings::entire_buffer(1, points_buf),
                ],
            },
        );
        let sampler = crate::made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: Some("water normal rt sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            },
        );
        Self {
            pipeline,
            bgl,
            reflect_globals_buf,
            reflect_globals_bg,
            sampler,
            targets: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.targets.clear();
    }

    pub fn prepare_targets(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
    ) -> WaterTargets {
        if let Some(t) = self.targets.iter().find(|t| t.size == size) {
            return t.clone();
        }
        if self.targets.len() >= 4 {
            self.targets.remove(0);
        }
        let non_srgb = format.remove_srgb_suffix();
        let scene_tex = crate::resources::texture_2d(
            device,
            "water scene copy",
            size,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            &[non_srgb],
        );
        let scene_view = scene_tex.create_view(&wgpu::TextureViewDescriptor {
            format: Some(non_srgb),
            ..Default::default()
        });

        let rw = (size[0] / 2).max(1);
        let rh = (size[1] / 2).max(1);
        let reflect_tex = crate::resources::texture_2d(
            device,
            "water reflection",
            [rw, rh],
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            &[non_srgb],
        );
        let reflect_view = reflect_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let reflect_sample_view = reflect_tex.create_view(&wgpu::TextureViewDescriptor {
            format: Some(non_srgb),
            ..Default::default()
        });
        let reflect_depth = create_depth_texture(device, rw, rh);
        let bg = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("water normal bg"),
                layout: &self.bgl,
                entries: &[
                    bindings::texture_view(3, &scene_view),
                    bindings::texture_view(4, &reflect_sample_view),
                    bindings::sampler(5, &self.sampler),
                ],
            },
        );
        let target = WaterTargets {
            size,
            scene_tex,
            reflect_tex,
            reflect_view,
            reflect_depth,
            bg,
        };
        let _ = &target.reflect_tex;
        self.targets.push(target.clone());
        target
    }
}

/// Reflection matrix across the horizontal plane `y = water_y`.
pub(crate) fn mirror_y_matrix(water_y: f32) -> glam::Mat4 {
    glam::Mat4::from_cols_array(&[
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        -1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        2.0 * water_y,
        0.0,
        1.0,
    ])
}

/// View-projection matrix for the planar reflection across `y = water_y`.
pub(crate) fn reflect_view_proj(view_proj: glam::Mat4, water_y: f32) -> glam::Mat4 {
    view_proj * mirror_y_matrix(water_y)
}

/// Height of the first kept [`LightPath::WaterNormal`] surface in this frame, if any.
pub(crate) fn water_normal_height(
    meshes: &[crate::mesh::GpuMesh],
    draws: &[DrawCmd],
    keep: &[bool],
) -> Option<f32> {
    draws
        .iter()
        .enumerate()
        .find(|&(i, d)| keep[i] && d.layer == DrawLayer::Water && d.light == LightPath::WaterNormal)
        .and_then(|(_, d)| {
            let m = meshes.get(d.mesh.0 as usize)?;
            let (lo, hi) = crate::frustum::transformed_aabb(&d.model, m.bounds.0, m.bounds.1);
            Some((lo.y + hi.y) * 0.5)
        })
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_water_reflect_setup(
        &self,
        water_y: f32,
        camera: &Camera,
        view_proj: glam::Mat4,
        base_globals: &Globals,
        sky: &[SkyShell],
        sky_base_colors: &[Instance],
        atmosphere_time: f64,
        draws: &[DrawCmd],
        allow_draw: impl Fn(&DrawCmd) -> bool,
        instances: &mut Vec<Instance>,
    ) -> ReflectSetup {
        let reflect_vp = reflect_view_proj(view_proj, water_y);
        let reflect_frustum = Frustum::from_view_proj(&reflect_vp);
        let reflect_eye = glam::Vec3::new(
            camera.position.x,
            2.0 * water_y - camera.position.y,
            camera.position.z,
        );

        let mut order: Vec<usize> = draws
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                matches!(d.layer, DrawLayer::Opaque | DrawLayer::Mask)
                    && d.light != LightPath::Iface
                    && allow_draw(d)
            })
            .filter(|(_, d)| {
                let Some(m) = self.meshes.get(d.mesh.0 as usize) else {
                    return false;
                };
                let (lo, hi) = crate::frustum::transformed_aabb(&d.model, m.bounds.0, m.bounds.1);
                hi.y >= water_y - 0.1 && (!self.cull || reflect_frustum.intersects_aabb(lo, hi))
            })
            .map(|(i, _)| i)
            .collect();
        order.sort_by_key(|&i| crate::frame::draw_key(&draws[i]));

        let sky_base = instances.len();
        for (idx, s) in sky.iter().enumerate() {
            let mut inst = sky_base_colors[idx];
            inst.model = Self::sky_model(s, reflect_eye, atmosphere_time, self.water_time)
                .to_cols_array_2d();
            instances.push(inst);
        }

        let world_base = instances.len();
        let lighting = self.lightning.map_or(self.lighting, |l| l.0);
        for &i in &order {
            let d = &draws[i];
            let (cull, cull_near) = crate::draw::instance_cull(d);
            let region = d.region_lit.and_then(|sphere| {
                crate::lighting::character_region_light(
                    &self.character_light_regions,
                    &lighting,
                    sphere,
                )
            });
            instances.push(Instance {
                model: d.model.to_cols_array_2d(),
                color: d.color,
                cull,
                cull_near,
                light: d.light as u32 | crate::affine::layer_bit(d.layer),
                fade: d.fade.map_or(crate::mesh::NO_FADE, |f| {
                    [f.begin, f.end, f.near, f.opacity]
                }),
                lights: [0; 4],
                region_top: region.map_or([0.0; 4], |r| [r[0][0], r[0][1], r[0][2], r[0][3] + 1.0]),
                region_bottom: region.map_or([0.0; 4], |r| r[1]),
            });
        }

        let globals = Globals {
            view_proj: reflect_vp.to_cols_array_2d(),
            camera_pos: reflect_eye.to_array(),
            water_clip_y: water_y - 0.05,
            water_clip_on: 1.0,
            ..*base_globals
        };
        ReflectSetup {
            water_y,
            sky_base,
            world_base,
            order,
            globals,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_water_reflection(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &WaterTargets,
        setup: &ReflectSetup,
        sky_order: &[usize],
        sky_solid_count: usize,
        draws: &[DrawCmd],
        clear: [f64; 4],
        camera_y: f32,
        skip_sky: bool,
    ) -> u32 {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("water reflection pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.reflect_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear[0],
                        g: clear[1],
                        b: clear[2],
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.reflect_depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if camera_y <= setup.water_y {
            return 0;
        }
        let mut draw_calls = 0u32;
        pass.set_bind_group(0, &self.water_reflect.reflect_globals_bg, &[]);
        pass.set_vertex_buffer(1, self.instance_buf.slice(..));

        if !skip_sky && !self.sky.is_empty() {
            let mut bound = None;
            let solid = &sky_order[..sky_solid_count];
            let blended = &sky_order[sky_solid_count..];
            for &slot in solid.iter().chain(blended.iter()) {
                let shell = &self.sky[slot];
                let Some(mesh) = self.meshes.get(shell.mesh.0 as usize) else {
                    continue;
                };
                let idx = (
                    usize::from(shell.depth),
                    usize::from(shell.lit),
                    Self::sky_mode(shell).index(),
                );
                if bound != Some(idx) {
                    pass.set_pipeline(&self.sky_pipelines[idx.0][idx.1][idx.2]);
                    bound = Some(idx);
                }
                pass.set_bind_group(1, self.tex_bg(shell.texture), &[]);
                let inst = (setup.sky_base + slot) as u32;
                draw_mesh(&mut pass, mesh, inst..inst + 1);
                draw_calls += 1;
            }
        }

        let fade = self.lighting.far_fade.is_some();
        let mut run_start = 0;
        let mut bound_layer: Option<DrawLayer> = None;
        while run_start < setup.order.len() {
            let first = &draws[setup.order[run_start]];
            let key = crate::frame::draw_key(first);
            let mut run_end = run_start + 1;
            while run_end < setup.order.len()
                && crate::frame::draw_key(&draws[setup.order[run_end]]) == key
            {
                run_end += 1;
            }
            if bound_layer != Some(first.layer) {
                pass.set_pipeline(match (first.layer, fade) {
                    (DrawLayer::Mask, false) => &self.mask_pipeline,
                    (DrawLayer::Mask, true) => &self.fade_mask_pipeline,
                    (_, false) => &self.pipeline,
                    (_, true) => &self.fade_pipeline,
                });
                bound_layer = Some(first.layer);
            }
            pass.set_bind_group(1, self.tex_bg(first.texture), &[]);
            draw_mesh(
                &mut pass,
                &self.meshes[first.mesh.0 as usize],
                (setup.world_base + run_start) as u32..(setup.world_base + run_end) as u32,
            );
            draw_calls += 1;
            run_start = run_end;
        }
        draw_calls
    }
}
