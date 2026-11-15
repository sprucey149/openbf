//! The DomeInfo frame filter (card BFLT, `analysis/re-glow.md` §3 `[O]`):
//! both games lift everything brighter than a threshold over the finished
//! 3D frame, before the HUD — the whole-frame brightening every BF1 map
//! (and BF2 TAT2) shows on the PS2.
//!
//! What the PS2 does (B1 `FUN_0049fe60`, B2 `FUN_00585890`): nothing while
//! `Intensity` < 1; else the frame is boxed 2:1 to 256×224, `Threshold` is
//! subtracted (GS ALPHA 0x62, clamped at 0), two sub-pixel blur steps, and
//! `Intensity`/128 of the result is added back over the frame (ALPHA 0x68)
//! — per channel, in the GS's display bytes. Measured on the PS2 (§3.3):
//! `out = min(255, v + I/128 · up2(max(0, box2(v) − T)))` predicts the
//! filtered frame within mean |d| 1.6–3.9.
//!
//! The port runs it at native resolution: the low-pass is a box of
//! `height/224` pixels centred on each pixel ([`box_side`]), averaged in two
//! 1-D passes (card ISPF): the row means into a 10-bit target, then the
//! column means - n linear samples a pixel, two texels each, not n² loads;
//! the same mean to the 8-bit result's rounding. Like the terrain light pass
//! (`terrain_light.rs`) it reads a COPY of the frame - in display units,
//! through a view without the sRGB suffix - and writes the colour over the
//! target (`FRAME_WRITES`: the alpha plane stays as the scene left it). A
//! window whose surface cannot be copied draws no filter
//! (said once on stderr).

use crate::bindings;
use crate::Renderer;

/// A map's filter as authored (`manifest::ManifestFilter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameFilter {
    /// `Threshold`, a display byte 0..255.
    pub threshold: f32,
    /// `Intensity`; the gain is `intensity / 128`.
    pub intensity: f32,
}

/// The widest box the pass averages (a 2688-line target).
const MAX_BOX: u32 = 12;

/// The low-pass box's side in output pixels for a `height`-line target:
/// one PS2 half-scale pixel (448 display lines / 224) at this resolution.
pub fn box_side(height: u32) -> u32 {
    ((height as f32 / 224.0).round() as u32).clamp(1, MAX_BOX)
}

/// The shader's `FilterParams`.
fn params(f: &FrameFilter, height: u32) -> [f32; 4] {
    [
        f.threshold / 255.0,
        f.intensity / 128.0,
        box_side(height) as f32,
        0.0,
    ]
}

/// The format of the row pass's target: display units, row means of
/// 8-bit bytes held within 1/8 of a byte by 10 bits a channel - half the
/// bytes of a half-float target, which the iGPU's bandwidth shows.
const ROWS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgb10a2Unorm;

/// The frame's copy and the row pass's target for one target size.
struct FilterDest {
    copy: wgpu::Texture,
    rows_view: wgpu::TextureView,
    /// Pass 1 reads the copy's display bytes; pass 2 the rows and the copy.
    rows_bg: wgpu::BindGroup,
    bg: wgpu::BindGroup,
    w: u32,
    h: u32,
}

/// The pass's per-renderer state.
pub(crate) struct FilterState {
    pub filter: Option<FrameFilter>,
    softness: f32,
    rows_pipeline: wgpu::RenderPipeline,
    pipeline: wgpu::RenderPipeline,
    rows_bgl: wgpu::BindGroupLayout,
    bgl: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,
    /// Two texels a sample (`box_sum` in the shader).
    sampler: wgpu::Sampler,
    dest: Option<FilterDest>,
    copyable: bool,
    warned: bool,
}

impl FilterState {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat, copyable: bool) -> Self {
        let params_entry = bindings::uniform_layout(1, wgpu::ShaderStages::FRAGMENT, false, None);
        let sampler_entry = bindings::sampler_layout(4);
        let rows_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame filter rows bgl"),
            entries: &[
                params_entry,
                bindings::texture_layout(2, true),
                sampler_entry,
            ],
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame filter bgl"),
            entries: &[
                bindings::texture_layout(0, true),
                params_entry,
                bindings::texture_layout(2, true),
                bindings::texture_layout(3, true),
                sampler_entry,
            ],
        });
        let sampler = crate::made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: Some("frame filter pair sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            },
        );
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("frame filter shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("filter.wgsl").into()),
            },
        );
        let build =
            |label: &str, bgl: &wgpu::BindGroupLayout, fs: &str, format: wgpu::TextureFormat| {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(label),
                    bind_group_layouts: &[bgl],
                    push_constant_ranges: &[],
                });
                crate::pipelines::PipelineDesc {
                    label,
                    layout: &layout,
                    vs: "vs_filter",
                    buffers: &[],
                    fs,
                    blend: None,
                    depth_write: false,
                    depth_compare: None,
                }
                .build(device, &shader, format)
            };
        let rows_pipeline = build(
            "frame filter rows pipeline",
            &rows_bgl,
            "fs_filter_rows",
            ROWS_FORMAT,
        );
        let pipeline = build("frame filter pipeline", &bgl, "fs_filter", format);
        let params_buf = crate::resources::buffer(
            device,
            "frame filter params",
            16,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        Self {
            filter: None,
            softness: 0.0,
            rows_pipeline,
            pipeline,
            rows_bgl,
            bgl,
            params_buf,
            sampler,
            dest: None,
            copyable,
            warned: false,
        }
    }
}

impl Renderer {
    /// The map's frame filter, or `None` for no pass (card BFLT).
    pub fn set_frame_filter(&mut self, filter: Option<FrameFilter>) {
        self.filter.filter = filter;
    }

    pub fn set_softness(&mut self, strength: Option<f32>) {
        self.softness.softness = strength
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
            .clamp(0.0, 128.0)
            / 128.0;
    }

    pub(crate) fn softness_wanted(&mut self) -> bool {
        Self::filter_state_wanted(&mut self.softness)
    }

    /// Whether this frame runs the filter: one set with a gain, and a
    /// target it can copy.
    pub(crate) fn filter_wanted(&mut self) -> bool {
        Self::filter_state_wanted(&mut self.filter)
    }

    fn filter_state_wanted(f: &mut FilterState) -> bool {
        let want = f.softness > 0.0 || f.filter.is_some_and(|p| p.intensity >= 1.0);
        if want && !f.copyable {
            if !f.warned {
                eprintln!("frame filter: the window surface cannot be copied; no filter pass");
                f.warned = true;
            }
            return false;
        }
        want
    }

    /// Upload the parameters and (re)build the copy for a `w`×`h` target.
    pub(crate) fn prepare_filter(&mut self, w: u32, h: u32) {
        Self::prepare_filter_state(
            &self.device,
            &self.queue,
            self.format,
            &mut self.filter,
            w,
            h,
        );
    }

    pub(crate) fn prepare_softness(&mut self, w: u32, h: u32) {
        Self::prepare_filter_state(
            &self.device,
            &self.queue,
            self.format,
            &mut self.softness,
            w,
            h,
        );
    }

    fn prepare_filter_state(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        f: &mut FilterState,
        w: u32,
        h: u32,
    ) {
        let p = f.filter.unwrap_or(FrameFilter {
            threshold: 0.0,
            intensity: 0.0,
        });
        let mut words = params(&p, h);
        words[3] = f.softness;
        queue.write_buffer(&f.params_buf, 0, bytemuck::cast_slice(&words));
        if matches!(&f.dest, Some(d) if (d.w, d.h) == (w, h)) {
            return;
        }
        // The copy carries a view without the sRGB suffix: `textureLoad`
        // through it returns the display bytes as stored.
        let display = format.remove_srgb_suffix();
        let copy = crate::resources::texture_2d(
            device,
            "frame filter source",
            [w, h],
            format,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            &[display],
        );
        let rows = crate::resources::texture_2d(
            device,
            "frame filter rows",
            [w, h],
            ROWS_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            &[],
        );
        let frame_view = copy.create_view(&wgpu::TextureViewDescriptor::default());
        let disp_view = copy.create_view(&wgpu::TextureViewDescriptor {
            format: Some(display),
            ..Default::default()
        });
        let rows_view = rows.create_view(&wgpu::TextureViewDescriptor::default());
        let rows_bg = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("frame filter rows bg"),
                layout: &f.rows_bgl,
                entries: &[
                    bindings::entire_buffer(1, &f.params_buf),
                    bindings::texture_view(2, &disp_view),
                    bindings::sampler(4, &f.sampler),
                ],
            },
        );
        let bg = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("frame filter bg"),
                layout: &f.bgl,
                entries: &[
                    bindings::texture_view(0, &frame_view),
                    bindings::entire_buffer(1, &f.params_buf),
                    bindings::texture_view(2, &disp_view),
                    bindings::texture_view(3, &rows_view),
                    bindings::sampler(4, &f.sampler),
                ],
            },
        );
        f.dest = Some(FilterDest {
            copy,
            rows_view,
            rows_bg,
            bg,
            w,
            h,
        });
    }

    /// Copy `src` (the finished 3D frame) and draw the filter over `view`:
    /// the row means, then the column means with the threshold and gain.
    pub(crate) fn filter_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::Texture,
        view: &wgpu::TextureView,
    ) -> u32 {
        Self::filter_state_pass(&self.filter, encoder, src, view)
    }

    pub(crate) fn softness_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::Texture,
        view: &wgpu::TextureView,
    ) -> u32 {
        Self::filter_state_pass(&self.softness, encoder, src, view)
    }

    fn filter_state_pass(
        f: &FilterState,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::Texture,
        view: &wgpu::TextureView,
    ) -> u32 {
        let Some(d) = &f.dest else {
            return 0;
        };
        encoder.copy_texture_to_texture(
            src.as_image_copy(),
            d.copy.as_image_copy(),
            wgpu::Extent3d {
                width: d.w,
                height: d.h,
                depth_or_array_layers: 1,
            },
        );
        let passes: [(
            &wgpu::TextureView,
            &wgpu::RenderPipeline,
            &wgpu::BindGroup,
            &str,
        ); 2] = [
            (
                &d.rows_view,
                &f.rows_pipeline,
                &d.rows_bg,
                "frame filter rows pass",
            ),
            (view, &f.pipeline, &d.bg, "frame filter pass"),
        ];
        for (target, pipeline, bg, label) in passes {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One PS2 half-scale pixel: 2 at the PS2's 448 lines, 3 at 720,
    /// 5 at 1080, capped.
    #[test]
    fn the_box_is_one_ps2_half_pixel() {
        assert_eq!(box_side(448), 2);
        assert_eq!(box_side(720), 3);
        assert_eq!(box_side(1080), 5);
        assert_eq!(box_side(100), 1);
        assert_eq!(box_side(8000), MAX_BOX);
    }

    /// TAT1's 50/50: threshold 50/255, gain 50/128.
    #[test]
    fn params_are_display_units() {
        let p = params(
            &FrameFilter {
                threshold: 50.0,
                intensity: 50.0,
            },
            720,
        );
        assert_eq!(p, [50.0 / 255.0, 50.0 / 128.0, 3.0, 0.0]);
    }
}
