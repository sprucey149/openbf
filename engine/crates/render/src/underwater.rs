//! PC Battlefront II's underwater tint (`analysis/re-water.md` §9.8 `[O]`).
//!
//! PC `BattlefrontII.exe` 0x6cf320 takes the camera's near-plane rectangle,
//! clips it against the water plane (LAYR height + bob), keeps the part at or
//! under the surface and fills it, as a screen-space fan (0x6eb180), with the
//! map's `UnderwaterColor` alpha-blended over the finished 3D frame. It runs
//! on the near pass of a flat-water map only, and only when one of the four
//! near-plane corners stands over a wet cell (0x6cffc0, the same query as
//! `core::sim::Sim::water_under`); the caller decides that and hands the
//! level here ([`Renderer::set_underwater`]).
//!
//! The port draws one full-screen triangle and keeps each pixel whose
//! near-plane point lies at or under the level: the same coverage as the
//! clipped polygon, for any projection the view uses. It blends in display
//! bytes through the frame's non-sRGB view, as the PC's D3D9 back buffer
//! does; a surface without that view blends the linearised colour in linear
//! light instead.

use crate::{bindings, Renderer};
use glam::Mat4;

/// One frame's tint: the water's height and the authored colour (0..255 RGBA).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnderwaterTint {
    pub level: f32,
    pub color: [f32; 4],
}

pub(crate) struct UnderwaterState {
    pub tint: Option<UnderwaterTint>,
    display: wgpu::RenderPipeline,
    linear: wgpu::RenderPipeline,
    params: wgpu::Buffer,
    group: wgpu::BindGroup,
}

/// The shader's `UnderwaterParams`: the inverse view-projection, the colour
/// in display units, then the level, the target's reciprocal size and whether
/// the colour is linearised for an sRGB target.
fn params(inverse: Mat4, tint: UnderwaterTint, size: [u32; 2], linear: bool) -> [f32; 24] {
    let mut out = [0.0; 24];
    out[..16].copy_from_slice(&inverse.to_cols_array());
    for (o, c) in out[16..20].iter_mut().zip(tint.color) {
        *o = (c / 255.0).clamp(0.0, 1.0);
    }
    out[20] = tint.level;
    out[21] = 1.0 / size[0].max(1) as f32;
    out[22] = 1.0 / size[1].max(1) as f32;
    out[23] = if linear { 1.0 } else { 0.0 };
    out
}

impl UnderwaterState {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("underwater layout"),
            entries: &[bindings::uniform_layout(
                0,
                wgpu::ShaderStages::FRAGMENT,
                false,
                None,
            )],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("underwater pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("underwater shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("underwater.wgsl").into()),
            },
        );
        let desc = crate::pipelines::PipelineDesc {
            label: "underwater pipeline",
            layout: &pipeline_layout,
            vs: "vs_underwater",
            buffers: &[],
            fs: "fs_underwater",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            depth_compare: None,
        };
        let display = desc.build(device, &shader, format.remove_srgb_suffix());
        let linear = desc.build(device, &shader, format);
        let params = crate::resources::buffer(
            device,
            "underwater params",
            (24 * 4) as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let group = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: Some("underwater group"),
                layout: &layout,
                entries: &[bindings::entire_buffer(0, &params)],
            },
        );
        Self {
            tint: None,
            display,
            linear,
            params,
            group,
        }
    }
}

impl Renderer {
    /// This frame's underwater tint, or `None` to draw none. Kept until changed.
    pub fn set_underwater(&mut self, tint: Option<UnderwaterTint>) {
        self.underwater.tint = tint.filter(|t| t.level.is_finite() && t.color[3] > 0.0);
    }

    /// Lay the tint over `view` (the world's colour target, `size` pixels)
    /// for a frame drawn with `view_proj`. `display` says the view is the
    /// frame's non-sRGB twin.
    pub(crate) fn underwater_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        display: bool,
        view_proj: Mat4,
        size: [u32; 2],
    ) -> u32 {
        let Some(tint) = self.underwater.tint else {
            return 0;
        };
        let words = params(view_proj.inverse(), tint, size, !display);
        self.queue
            .write_buffer(&self.underwater.params, 0, bytemuck::cast_slice(&words));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("underwater"),
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
        pass.set_pipeline(if display {
            &self.underwater.display
        } else {
            &self.underwater.linear
        });
        pass.set_bind_group(0, &self.underwater.group, &[]);
        pass.draw(0..3, 0..1);
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_carry_display_colour_level_and_size() {
        let tint = UnderwaterTint {
            level: -1.3,
            color: [61.0, 124.0, 144.0, 128.0],
        };
        let p = params(Mat4::IDENTITY, tint, [1920, 1080], false);
        assert_eq!(&p[..16], &Mat4::IDENTITY.to_cols_array());
        assert_eq!(
            &p[16..20],
            &[61.0 / 255.0, 124.0 / 255.0, 144.0 / 255.0, 128.0 / 255.0]
        );
        assert_eq!(p[20], -1.3);
        assert_eq!(p[21], 1.0 / 1920.0);
        assert_eq!(p[22], 1.0 / 1080.0);
        assert_eq!(p[23], 0.0);
        assert_eq!(params(Mat4::IDENTITY, tint, [1, 1], true)[23], 1.0);
    }
}
