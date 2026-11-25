//! Project the existing HUD texture onto a head-relative spherical visor.

use crate::{bindings, FrameTarget, ViewFov};
use glam::{Quat, Vec2, Vec3};

// Vergence distance for the HUD shell. Its angular bounds are fitted to the
// headset, independently of the desktop canvas size.
const RADIUS: f32 = 2.0;
const INSET: f32 = 0.045;
const ASPECT: f32 = 16.0 / 9.0;

#[derive(Clone, Copy)]
pub struct VisorEye {
    pub offset: Vec3,
    pub orientation: Quat,
    pub fov: ViewFov,
}

impl VisorEye {
    fn shell_angles(self, uv: Vec2) -> Vec2 {
        let fov = self.fov;
        let ray = self.orientation
            * Vec3::new(
                fov.left + (fov.right - fov.left) * uv.x,
                fov.down + (fov.up - fov.down) * uv.y,
                -1.0,
            )
            .normalize();
        let b = self.offset.dot(ray);
        let distance = (b * b + RADIUS * RADIUS - self.offset.length_squared()).sqrt() - b;
        angles(self.offset + ray * distance)
    }

    fn sees(self, point: Vec3) -> bool {
        let p = self.orientation.conjugate() * (point - self.offset);
        if p.z >= 0.0 {
            return false;
        }
        let fov = self.fov;
        let uv = Vec2::new(
            (p.x / -p.z - fov.left) / (fov.right - fov.left),
            (p.y / -p.z - fov.down) / (fov.up - fov.down),
        );
        uv.cmpge(Vec2::splat(INSET)).all() && uv.cmple(Vec2::splat(1.0 - INSET)).all()
    }
}

#[derive(Clone, Copy)]
pub struct Visor {
    eyes: [VisorEye; 2],
    half: Vec2,
}

impl Visor {
    pub fn new(eyes: [VisorEye; 2]) -> Self {
        // Start with the overlap of the eye frusta at the horizontal and
        // vertical center lines. Fit the widescreen HUD within that overlap,
        // then check the corners and edge midpoints on the curved surface.
        let mut half = Vec2::splat(std::f32::consts::FRAC_PI_2);
        for eye in eyes {
            let left = eye.shell_angles(Vec2::new(0.0, 0.5)).x;
            let right = eye.shell_angles(Vec2::new(1.0, 0.5)).x;
            let down = eye.shell_angles(Vec2::new(0.5, 0.0)).y;
            let up = eye.shell_angles(Vec2::new(0.5, 1.0)).y;
            half = half.min(Vec2::new((-left).min(right), (-down).min(up)));
        }
        let height = half.y.min(half.x / ASPECT).max(0.01);
        let half = Vec2::new(height * ASPECT, height);
        let fits = |s: f32| {
            [-1.0, 0.0, 1.0].into_iter().all(|x| {
                [-1.0, 0.0, 1.0].into_iter().all(|y| {
                    let point = shell_point(half * Vec2::new(x, y) * s);
                    eyes.iter().all(|eye| eye.sees(point))
                })
            })
        };
        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..10 {
            let mid = (low + high) * 0.5;
            if fits(mid) {
                low = mid;
            } else {
                high = mid;
            }
        }
        Self {
            eyes,
            half: half * low.max(0.01),
        }
    }

    /// The angle the HUD frame's height covers on the shell.
    pub fn radians_per_height(&self) -> f32 {
        2.0 * self.half.y
    }
}

fn angles(p: Vec3) -> Vec2 {
    Vec2::new(p.x.atan2(-p.z), p.y.atan2(p.with_y(0.0).length()))
}

fn shell_point(angular: Vec2) -> Vec3 {
    let (sy, cy) = angular.x.sin_cos();
    let (sp, cp) = angular.y.sin_cos();
    Vec3::new(sy * cp, sp, -cy * cp) * RADIUS
}

pub struct VisorPass {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params: [wgpu::Buffer; 2],
    sampler: wgpu::Sampler,
    display: wgpu::TextureFormat,
}

impl VisorPass {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("visor bindings"),
            entries: &[
                bindings::uniform_layout(0, wgpu::ShaderStages::FRAGMENT, false, None),
                bindings::texture_layout(1, true),
                bindings::sampler_layout(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("visor pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("visor shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("visor.wgsl").into()),
            },
        );
        // The game's HUD stores premultiplied display values. Composite in
        // that same encoding to preserve its authored text and transparency.
        let display = format.remove_srgb_suffix();
        let pipeline = crate::pipelines::PipelineDesc {
            label: "visor",
            layout: &pipeline_layout,
            vs: "vs_main",
            fs: "fs_main",
            buffers: &[],
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            depth_write: false,
            depth_compare: None,
        }
        .build(device, &shader, display);
        let params = std::array::from_fn(|_| {
            crate::resources::buffer(
                device,
                "visor eye parameters",
                64,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            )
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            layout,
            pipeline,
            params,
            sampler,
            display,
        }
    }

    pub fn draw(
        &self,
        renderer: &crate::Renderer,
        encoder: &mut wgpu::CommandEncoder,
        target: &FrameTarget,
        hud: &FrameTarget,
        visor: &Visor,
        eye: usize,
    ) {
        let view = visor.eyes[eye];
        let fov = view.fov;
        let params = [
            view.offset.extend(RADIUS).to_array(),
            view.orientation.to_array(),
            [fov.left, fov.right, fov.down, fov.up],
            [visor.half.x, visor.half.y, 0.0, 0.0],
        ];
        let device = renderer.device();
        let display_view = |frame: &FrameTarget| {
            frame
                .view()
                .texture()
                .create_view(&wgpu::TextureViewDescriptor {
                    format: Some(self.display),
                    ..Default::default()
                })
        };
        let source = display_view(hud);
        let target = display_view(target);
        renderer
            .queue()
            .write_buffer(&self.params[eye], 0, bytemuck::bytes_of(&params));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("visor eye"),
            layout: &self.layout,
            entries: &[
                bindings::entire_buffer(0, &self.params[eye]),
                bindings::texture_view(1, &source),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("draw the HUD on the visor"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests;
