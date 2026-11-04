//! The HUD pass (card BHUD2): window-pixel triangles over the finished frame —
//! textured, vertex-coloured, alpha or additive, in the order they are given.
//! A batch flagged `depth` (a HUD mesh) is depth-tested against itself in a
//! depth buffer of the pass's own; everything else ignores depth, so order
//! alone decides what covers what. The layout that produces the triangles is
//! `ui::hud_draw`; this file only puts them on the target.
//!
//! [`HudPass`] is its own object rather than a `Renderer` field because it
//! paints from the overlay hook (`OverlayFn`), like the egui overlay does.
//!
//! Colours are DISPLAY values and blend as display values, which is what the
//! PS2's frame buffer does: black at alpha 0.6 leaves 40 % of the pixel's
//! byte, not 40 % of its light. The pass gets there by drawing through a
//! non-sRGB view of the frame (`Renderer::display_view_format`); where the
//! window surface has none it falls back to the frame's own sRGB view and
//! linearises each fragment, so colours are right and only the blend is the
//! renderer's linear one.

use crate::bindings;
use bytemuck::{Pod, Zeroable};

/// Window pixels (x right, y down), depth 0 near … 1 far; display (sRGB
/// encoded) RGB with straight alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// A texture uploaded with [`HudPass::add_texture`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HudTexture(u32);

/// A run of indexed triangles sharing one texture and state.
#[derive(Debug, Clone, Copy)]
pub struct HudBatch {
    /// `None` draws the vertex colour alone.
    pub texture: Option<HudTexture>,
    pub additive: bool,
    pub depth: bool,
    pub first: u32,
    pub count: u32,
}

const SHADER: &str = r#"
// `linearise` is 1 on the fallback path (module doc).
struct Screen { size: vec2<f32>, linearise: f32, pad: f32 };
@group(0) @binding(0) var<uniform> screen: Screen;
@group(1) @binding(0) var sheet: texture_2d<f32>;
@group(1) @binding(1) var sheet_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_hud(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.clip = vec4<f32>(
        pos.x / screen.size.x * 2.0 - 1.0,
        1.0 - pos.y / screen.size.y * 2.0,
        pos.z,
        1.0,
    );
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_hud(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(sheet, sheet_sampler, in.uv) * in.color;
    if (screen.linearise < 0.5) {
        return c;
    }
    let lo = c.rgb / 12.92;
    let hi = pow((c.rgb + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return vec4<f32>(select(hi, lo, c.rgb <= vec3<f32>(0.04045)), c.a);
}

@fragment
fn fs_distance(in: VsOut) -> @location(0) vec4<f32> {
    let distance = textureSample(sheet, sheet_sampler, in.uv).a - 0.5;
    // Derivatives are in native output pixels, independent of layout scale.
    let width = max(fwidth(distance), 1.0 / 255.0);
    let alpha = clamp(distance / width + 0.5, 0.0, 1.0) * in.color.a;
    var rgb = in.color.rgb;
    if (screen.linearise > 0.5) {
        let lo = rgb / 12.92;
        let hi = pow((rgb + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
        rgb = select(hi, lo, rgb <= vec3<f32>(0.04045));
    }
    return vec4<f32>(rgb, alpha);
}
"#;

pub struct HudPass {
    /// Indexed `[depth][additive]`.
    pipelines: [[wgpu::RenderPipeline; 2]; 2],
    distance_pipelines: [wgpu::RenderPipeline; 2],
    distance_textures: std::collections::HashSet<HudTexture>,
    screen_buf: wgpu::Buffer,
    screen_bg: wgpu::BindGroup,
    texture_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    repeat_sampler: wgpu::Sampler,
    textures: Vec<wgpu::BindGroup>,
    /// The textures rewritten every frame ([`HudPass::add_stream_texture`]):
    /// the handle, the texture and its size.
    streams: Vec<(HudTexture, wgpu::Texture, [u32; 2])>,
    white: wgpu::BindGroup,
    vertices: Option<(wgpu::Buffer, u64)>,
    indices: Option<(wgpu::Buffer, u64)>,
    depth: Option<(wgpu::TextureView, [u32; 2])>,
    /// The non-sRGB view format to draw through, or `None` on the fallback.
    display: Option<wgpu::TextureFormat>,
}

fn texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    levels: &[&[u8]],
    width: u32,
    height: u32,
) -> Option<wgpu::BindGroup> {
    let view = crate::texture::upload_levels(
        device,
        queue,
        width,
        height,
        levels,
        // The sheet's bytes as they are: display values (module doc).
        false,
        Some("hud sheet"),
    )
    .ok()?;
    Some(crate::made::bind_group(
        device,
        &wgpu::BindGroupDescriptor {
            label: Some("hud sheet bg"),
            layout,
            entries: &[
                bindings::texture_view(0, &view),
                bindings::sampler(1, sampler),
            ],
        },
    ))
}

impl HudPass {
    /// A pass that draws into frames of `format`; `display` is
    /// `Renderer::display_view_format` for those frames.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        display: Option<wgpu::TextureFormat>,
    ) -> Self {
        let format = display.unwrap_or(format);
        let screen_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud screen bgl"),
            entries: &[bindings::uniform_layout(
                0,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                false,
                None,
            )],
        });
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud sheet bgl"),
            entries: &[
                bindings::texture_layout(0, true),
                bindings::sampler_layout(1),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud layout"),
            bind_group_layouts: &[&screen_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("hud shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            },
        );
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4];
        let pipeline = |depth: bool, additive: bool, distance: bool| {
            let blend = wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: if additive {
                        wgpu::BlendFactor::One
                    } else {
                        wgpu::BlendFactor::OneMinusSrcAlpha
                    },
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            };
            // HUD meshes are mirrored and turned freely: no culling.
            crate::pipelines::PipelineDesc {
                label: "hud pipeline",
                layout: &layout,
                vs: "vs_hud",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HudVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
                fs: if distance { "fs_distance" } else { "fs_hud" },
                blend: Some(blend),
                depth_write: depth,
                depth_compare: Some(if depth {
                    wgpu::CompareFunction::LessEqual
                } else {
                    wgpu::CompareFunction::Always
                }),
            }
            .build(device, &shader, format)
        };
        let pipelines = [
            [pipeline(false, false, false), pipeline(false, true, false)],
            [pipeline(true, false, false), pipeline(true, true, false)],
        ];
        let distance_pipelines = [pipeline(false, false, true), pipeline(false, true, true)];
        let screen_buf = crate::resources::buffer(
            device,
            "hud screen",
            16,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let screen_bg = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("hud screen bg"),
                layout: &screen_bgl,
                entries: &[bindings::entire_buffer(0, &screen_buf)],
            },
        );
        // Most sheets have one level. Reconstructed UI sheets retain the
        // source levels so their new detail is filtered as they shrink.
        let sampler = crate::made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: Some("hud sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            },
        );
        let repeat_sampler = crate::made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: Some("hud repeat sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            },
        );
        let white = texture_bind_group(device, queue, &texture_bgl, &sampler, &[&[255; 4]], 1, 1)
            .expect("a 1x1 texture uploads");
        Self {
            pipelines,
            distance_pipelines,
            distance_textures: Default::default(),
            screen_buf,
            screen_bg,
            texture_bgl,
            sampler,
            repeat_sampler,
            textures: Vec::new(),
            streams: Vec::new(),
            white,
            vertices: None,
            indices: None,
            depth: None,
            display,
        }
    }

    /// Upload the RGBA8 levels of a sheet; `None` when they do not fit the size.
    pub fn add_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        levels: &[&[u8]],
        width: u32,
        height: u32,
        repeat: bool,
    ) -> Option<HudTexture> {
        let bg = texture_bind_group(
            device,
            queue,
            &self.texture_bgl,
            if repeat {
                &self.repeat_sampler
            } else {
                &self.sampler
            },
            levels,
            width,
            height,
        )?;
        self.textures.push(bg);
        Some(HudTexture(self.textures.len() as u32 - 1))
    }

    /// A baked signed-distance glyph page. Coverage is reconstructed at the
    /// target resolution; colour, weight and shadows remain the layout's.
    pub fn add_distance_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Option<HudTexture> {
        let texture = self.add_texture(device, queue, &[rgba], width, height, false)?;
        self.distance_textures.insert(texture);
        Some(texture)
    }

    /// A `width` x `height` RGBA8 texture of display values whose texels
    /// [`HudPass::write_stream_texture`] replaces (the shell's movie, card
    /// BSHLA); black until written.
    pub fn add_stream_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Option<HudTexture> {
        let texture = crate::resources::texture_2d(
            device,
            "hud stream",
            [width, height],
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            &[],
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("hud stream bg"),
                layout: &self.texture_bgl,
                entries: &[
                    bindings::texture_view(0, &view),
                    bindings::sampler(1, &self.sampler),
                ],
            },
        );
        self.textures.push(bg);
        let t = HudTexture(self.textures.len() as u32 - 1);
        self.streams.push((t, texture, [width, height]));
        self.write_stream_texture(queue, t, &vec![0; (width * height * 4) as usize]);
        Some(t)
    }

    /// Replace a stream texture's texels; ignored when `rgba` is not its
    /// size or `t` is not a stream texture.
    pub fn write_stream_texture(&self, queue: &wgpu::Queue, t: HudTexture, rgba: &[u8]) {
        let Some((_, texture, [w, h])) = self.streams.iter().find(|(s, ..)| *s == t) else {
            return;
        };
        if rgba.len() != (*w * *h * 4) as usize {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(*w * 4),
                rows_per_image: Some(*h),
            },
            wgpu::Extent3d {
                width: *w,
                height: *h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Draw `batches` over `view` (call from the renderer's overlay hook).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        vertices: &[HudVertex],
        indices: &[u32],
        batches: &[HudBatch],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size: [u32; 2],
    ) {
        if batches.is_empty() || vertices.is_empty() || size[0] == 0 || size[1] == 0 {
            return;
        }
        queue.write_buffer(
            &self.screen_buf,
            0,
            bytemuck::cast_slice(&[
                size[0] as f32,
                size[1] as f32,
                if self.display.is_some() { 0.0 } else { 1.0 },
                0.0,
            ]),
        );
        let grow = |slot: &mut Option<(wgpu::Buffer, u64)>, bytes: &[u8], usage| {
            let need = bytes.len() as u64;
            if !matches!(slot, Some((_, cap)) if *cap >= need) {
                let cap = need.next_power_of_two().max(4096);
                *slot = Some((
                    crate::resources::buffer(
                        device,
                        "hud geometry",
                        cap,
                        usage | wgpu::BufferUsages::COPY_DST,
                    ),
                    cap,
                ));
            }
            if let Some((buf, _)) = slot {
                queue.write_buffer(buf, 0, bytes);
            }
        };
        grow(
            &mut self.vertices,
            bytemuck::cast_slice(vertices),
            wgpu::BufferUsages::VERTEX,
        );
        grow(
            &mut self.indices,
            bytemuck::cast_slice(indices),
            wgpu::BufferUsages::INDEX,
        );
        if !matches!(&self.depth, Some((_, s)) if *s == size) {
            self.depth = Some((
                crate::device::create_depth_texture(device, size[0], size[1]),
                size,
            ));
        }
        let (Some((vbuf, _)), Some((ibuf, _)), Some((depth, _))) =
            (&self.vertices, &self.indices, &self.depth)
        else {
            return;
        };
        let display_view = self.display.map(|format| {
            view.texture().create_view(&wgpu::TextureViewDescriptor {
                format: Some(format),
                ..Default::default()
            })
        });
        let view = display_view.as_ref().unwrap_or(view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hud pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.screen_bg, &[]);
        pass.set_vertex_buffer(0, vbuf.slice(..));
        pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
        let total = indices.len() as u32;
        for b in batches {
            let end = b.first.saturating_add(b.count);
            if b.count == 0 || end > total {
                continue;
            }
            let bg = b
                .texture
                .and_then(|t| self.textures.get(t.0 as usize))
                .unwrap_or(&self.white);
            let distance = b
                .texture
                .is_some_and(|t| self.distance_textures.contains(&t));
            pass.set_pipeline(if distance {
                &self.distance_pipelines[usize::from(b.additive)]
            } else {
                &self.pipelines[usize::from(b.depth)][usize::from(b.additive)]
            });
            pass.set_bind_group(1, bg, &[]);
            pass.draw_indexed(b.first..end, 0, 0..1);
        }
    }
}
