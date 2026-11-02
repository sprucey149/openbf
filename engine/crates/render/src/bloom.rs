//! Selective BF2 HDR bloom (`analysis/re-glow.md` §2 [D]). Gather unclipped
//! self-lit surfaces and additive sprites in half-scale display units against
//! the scene depth; threshold before downsampling, then the authored cascade.
//! [ours] Only emissive sources bloom. Ordinary lit whites are excluded.
//! [ours] The 4x downsample averages all 16 texels of a block (the PS2 takes one
//! bilinear sample of the central 2x2, `re-glow.md` §2.2).
//! Native quarter-size buffers replace the fixed PS2 128×112 buffers.
use crate::{bindings, pipelines::PipelineDesc, Renderer};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bloom {
    pub passes: u32,
    pub weight: f32,
    pub threshold: f32,
}
impl Bloom {
    fn checked(self) -> Option<Self> {
        (self.passes > 0
            && self.passes <= 16
            && self.weight.is_finite()
            && self.weight > 0.0
            && self.weight <= 2.0
            && self.threshold.is_finite()
            && self.threshold >= 0.0)
            .then_some(self)
    }
    fn tap(self) -> f32 {
        ((self.weight * 127.0).floor() * 0.2).floor() / 128.0
    }
}

pub(crate) struct Gather {
    pub sky: [wgpu::RenderPipeline; 2],
    pub world: wgpu::RenderPipeline,
    pub mask: wgpu::RenderPipeline,
    pub blend: wgpu::RenderPipeline,
    pub particle: wgpu::RenderPipeline,
}
pub(crate) fn gather(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    desc: PipelineDesc<'_>,
    fs: &str,
) -> wgpu::RenderPipeline {
    PipelineDesc {
        fs,
        depth_write: false,
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        ..desc
    }
    .build(device, shader, FORMAT)
}

struct Dest {
    raw: wgpu::TextureView,
    ping: [wgpu::TextureView; 2],
    copy: wgpu::Texture,
    extract_bg: wgpu::BindGroup,
    blur_bg: Vec<wgpu::BindGroup>,
    composite_bg: wgpu::BindGroup,
    size: [u32; 2],
    settings: Bloom,
}

pub(crate) struct BloomState {
    pub settings: Option<Bloom>,
    pub gather: Gather,
    extract: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    dest: Option<Dest>,
    copyable: bool,
}
impl BloomState {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        copyable: bool,
        gather: Gather,
    ) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom layout"),
            entries: &[
                bindings::texture_layout(0, true),
                bindings::sampler_layout(1),
                bindings::uniform_layout(2, wgpu::ShaderStages::FRAGMENT, false, None),
                bindings::texture_layout(3, true),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("bloom"),
                source: wgpu::ShaderSource::Wgsl(include_str!("bloom.wgsl").into()),
            },
        );
        let pipeline = |fs, format| {
            PipelineDesc {
                label: fs,
                layout: &layout,
                vs: "vs_bloom",
                buffers: &[],
                fs,
                blend: None,
                depth_write: false,
                depth_compare: None,
            }
            .build(device, &shader, format)
        };
        Self {
            settings: None,
            gather,
            extract: pipeline("fs_extract", FORMAT),
            blur: pipeline("fs_blur", FORMAT),
            composite: pipeline("fs_composite", format),
            bgl,
            sampler: crate::made::sampler(
                device,
                &wgpu::SamplerDescriptor {
                    label: Some("bloom linear"),
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                },
            ),
            dest: None,
            copyable,
        }
    }
}
impl Renderer {
    pub fn set_bloom(&mut self, settings: Option<Bloom>) {
        self.bloom.settings = settings.and_then(Bloom::checked);
    }
    pub(crate) fn prepare_bloom(&mut self, w: u32, h: u32) -> bool {
        let b = &mut self.bloom;
        let Some(settings) = b.settings.filter(|_| b.copyable) else {
            return false;
        };
        if matches!(&b.dest, Some(d) if d.size == [w,h] && d.settings == settings) {
            return true;
        }
        let texture = |label, size, format, usage, views: &[wgpu::TextureFormat]| {
            crate::resources::texture_2d(&self.device, label, size, format, usage, views)
        };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let raw =
            texture("bloom light", [w, h], FORMAT, usage, &[]).create_view(&Default::default());
        let small = [w.div_ceil(4), h.div_ceil(4)];
        let ping = [0, 1].map(|_| {
            texture("bloom ping", small, FORMAT, usage, &[]).create_view(&Default::default())
        });
        let display = self.format.remove_srgb_suffix();
        let copy = texture(
            "bloom frame",
            [w, h],
            self.format,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            &[display],
        );
        let view = copy.create_view(&wgpu::TextureViewDescriptor {
            format: Some(display),
            ..Default::default()
        });
        let group = |source: &wgpu::TextureView, offset: f32| {
            use wgpu::util::DeviceExt;
            let data = [settings.threshold, settings.tap(), offset, 0.0];
            let params = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("bloom params"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            crate::made::bind_group(
                &self.device,
                &wgpu::BindGroupDescriptor {
                    label: Some("bloom inputs"),
                    layout: &b.bgl,
                    entries: &[
                        bindings::texture_view(0, source),
                        bindings::sampler(1, &b.sampler),
                        bindings::entire_buffer(2, &params),
                        bindings::texture_view(3, &view),
                    ],
                },
            )
        };
        let extract_bg = group(&raw, 0.0);
        let blur_bg = (0..settings.passes * 2)
            .map(|n| {
                group(
                    &ping[(n % 2) as usize],
                    (n + 1) as f32 * 0.5 * h as f32 / 448.0,
                )
            })
            .collect();
        let composite_bg = group(&ping[0], 0.0);
        b.dest = Some(Dest {
            raw,
            ping,
            copy,
            extract_bg,
            blur_bg,
            composite_bg,
            size: [w, h],
            settings,
        });
        true
    }
    pub(crate) fn bloom_gather_view(&self) -> &wgpu::TextureView {
        &self.bloom.dest.as_ref().expect("prepared bloom").raw
    }
    pub(crate) fn bloom_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::Texture,
        view: &wgpu::TextureView,
    ) -> u32 {
        let b = &self.bloom;
        let d = b.dest.as_ref().expect("prepared bloom");
        encoder.copy_texture_to_texture(
            src.as_image_copy(),
            d.copy.as_image_copy(),
            wgpu::Extent3d {
                width: d.size[0],
                height: d.size[1],
                depth_or_array_layers: 1,
            },
        );
        let mut fullscreen =
            |target: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline, bg: &wgpu::BindGroup| {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bloom"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
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
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bg, &[]);
                pass.draw(0..3, 0..1);
            };
        fullscreen(&d.ping[0], &b.extract, &d.extract_bg);
        for (n, bg) in d.blur_bg.iter().enumerate() {
            fullscreen(&d.ping[(n + 1) % 2], &b.blur, bg);
        }
        fullscreen(view, &b.composite, &d.composite_bg);
        2 + d.blur_bg.len() as u32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_taps_and_invalid_settings() {
        for (weight, expected) in [(1.2, 30.0), (1.1, 27.0), (1.05, 26.0), (1.0, 25.0)] {
            let b = Bloom {
                passes: 5,
                weight,
                threshold: 0.5,
            };
            assert_eq!(b.tap(), expected / 128.0);
            assert!(b.checked().is_some());
        }
        for passes in [0, 17, u32::MAX] {
            assert!(Bloom {
                passes,
                weight: 1.2,
                threshold: 0.5
            }
            .checked()
            .is_none());
        }
        assert!(Bloom {
            passes: 5,
            weight: f32::NAN,
            threshold: 0.5
        }
        .checked()
        .is_none());
    }
}
