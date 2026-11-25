//! Stand the HUD's centre reticle in the world, at the point it marks: each
//! eye sees it at that point's own depth.

use crate::{bindings, Camera, FrameTarget, ViewFov};
use glam::{Vec2, Vec3, Vec4};

/// The part of the painted HUD frame the quad shows, a square about the
/// middle, as half its side over the frame's height. The reticle group and
/// its rings sit well inside it.
const HALF: f32 = 0.2;

/// The reticle is not drawn this near the viewer, in metres, where the two
/// eyes cannot join it and it hides what is in front of them.
const HIDDEN_AT: f32 = 3.0;

/// OURS: metres at which the reticle reaches full strength, so it fades in
/// past [`HIDDEN_AT`] without popping. measure: sweep the aim across near and
/// far surfaces in a headset.
const SHOWN_AT: f32 = 4.5;

/// OURS: the metres at which the reticle shows at the size the HUD paints it.
/// measure: aim at near, middle and far targets in a headset.
const FULL_SIZE_AT: f32 = 5.0;

/// OURS: the size to the eye falls as distance to this power, so depth reads
/// from the size alone. It falls slower than a solid object's, which would
/// shrink the ammo ring past reading. measure: as [`FULL_SIZE_AT`].
const SIZE_FALLOFF: f32 = 0.5;

/// OURS: the smallest share of the painted size, reached 80 m out.
/// measure: read the ammo ring on a far target in a headset.
const SMALLEST: f32 = 0.25;

fn apparent_size(distance: f32) -> f32 {
    (FULL_SIZE_AT / distance)
        .powf(SIZE_FALLOFF)
        .clamp(SMALLEST, 1.0)
}

/// A reticle quad standing at one world point, facing the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldReticle {
    centre: Vec3,
    right: Vec3,
    up: Vec3,
    opacity: f32,
}

impl WorldReticle {
    /// The reticle at `centre` as `viewer` sees it, `radians_per_height` the
    /// angle the HUD frame's height covers on the visor. It looks smaller the
    /// farther the point is. `None` where it is hidden.
    pub fn facing(centre: Vec3, viewer: &Camera, radians_per_height: f32) -> Option<Self> {
        let toward = viewer.position - centre;
        let distance = toward.length();
        let t = ((distance - HIDDEN_AT) / (SHOWN_AT - HIDDEN_AT)).clamp(0.0, 1.0);
        let opacity = t * t * (3.0 - 2.0 * t);
        if !distance.is_finite() || opacity <= 0.0 {
            return None;
        }
        let normal = toward / distance;
        let up = viewer.right().cross(viewer.forward());
        let right = up.cross(normal).try_normalize()?;
        let side = distance * (HALF * radians_per_height * apparent_size(distance)).tan();
        Some(Self {
            centre,
            right: right * side,
            up: normal.cross(right) * side,
            opacity,
        })
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Top left, top right, bottom right, bottom left.
    fn corners(&self) -> [Vec3; 4] {
        [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
            .map(|(x, y)| self.centre + self.right * x + self.up * y)
    }
}

pub struct ReticlePass {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params: [wgpu::Buffer; 2],
    sampler: wgpu::Sampler,
    display: wgpu::TextureFormat,
}

impl ReticlePass {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world reticle bindings"),
            entries: &[
                bindings::uniform_layout(0, wgpu::ShaderStages::VERTEX_FRAGMENT, false, None),
                bindings::texture_layout(1, true),
                bindings::sampler_layout(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world reticle pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = crate::made::shader_module(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("world reticle shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("reticle.wgsl").into()),
            },
        );
        // Premultiplied display values, as the visor composites the HUD.
        let display = format.remove_srgb_suffix();
        let pipeline = crate::pipelines::PipelineDesc {
            label: "world reticle",
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
                "world reticle eye parameters",
                80,
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

    /// Draw `reticle`'s HUD frame `source` over eye `eye`'s `target`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        renderer: &crate::Renderer,
        encoder: &mut wgpu::CommandEncoder,
        target: &FrameTarget,
        source: &FrameTarget,
        reticle: &WorldReticle,
        (camera, fov): &(Camera, ViewFov),
        eye: usize,
    ) {
        let [width, height] = source.size();
        let uv_half = Vec2::new(HALF * height as f32 / width.max(1) as f32, HALF);
        let view_projection = fov.view_projection(camera);
        let mut params = [Vec4::ZERO; 5];
        for (clip, corner) in params.iter_mut().zip(reticle.corners()) {
            *clip = view_projection * corner.extend(1.0);
        }
        params[4] = Vec4::new(uv_half.x, uv_half.y, reticle.opacity, 0.0);
        let params = params.map(|v| v.to_array());
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
        let source = display_view(source);
        let target = display_view(target);
        renderer
            .queue()
            .write_buffer(&self.params[eye], 0, bytemuck::bytes_of(&params));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world reticle eye"),
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
            label: Some("draw the reticle in the world"),
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
        pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PER_HEIGHT: f32 = 0.8;

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn the_pass_paints_the_frame_over_the_middle_of_an_eye() {
        let mut renderer = crate::Renderer::new_headless(256, 192).unwrap();
        let [source, eye] = [(); 2].map(|()| renderer.frame_target([256, 192]).unwrap());
        let pass = ReticlePass::new(renderer.device(), renderer.surface_format());
        let camera = Camera::default();
        let fov = ViewFov {
            left: -1.0,
            right: 1.0,
            down: -0.75,
            up: 0.75,
        };
        let reticle = WorldReticle::facing(Vec3::NEG_Z * 20.0, &camera, PER_HEIGHT).unwrap();
        let mut encoder = renderer
            .device()
            .create_command_encoder(&Default::default());
        for (target, color) in [(&source, wgpu::Color::RED), (&eye, wgpu::Color::BLACK)] {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view(),
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        pass.draw(
            &renderer,
            &mut encoder,
            &eye,
            &source,
            &reticle,
            &(camera, fov),
            0,
        );
        renderer.queue().submit([encoder.finish()]);
        let blit = wgpu::util::TextureBlitter::new(renderer.device(), renderer.surface_format());
        renderer
            .render_with_overlay(
                &camera,
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |device, _, encoder, output, _| {
                    blit.copy(device, encoder, eye.view(), output)
                }),
            )
            .unwrap();
        let (rgba, width, height) = renderer.capture_rgba().unwrap();
        let red = |x: u32, y: u32| rgba[((y * width + x) * 4) as usize];
        assert!(red(width / 2, height / 2) > 200);
        assert_eq!(red(4, 4), 0);
        assert_eq!(red(width - 5, height / 2), 0);
    }

    #[test]
    fn the_reticle_hides_near_the_viewer_and_fades_in_past_three_metres() {
        let viewer = Camera::default();
        let at = |d: f32| WorldReticle::facing(Vec3::NEG_Z * d, &viewer, PER_HEIGHT);
        assert_eq!(at(0.5), None);
        assert_eq!(at(3.0), None);
        let part = at(3.75).unwrap().opacity();
        assert!(part > 0.0 && part < 1.0);
        assert_eq!(at(4.5).unwrap().opacity(), 1.0);
        assert_eq!(at(512.0).unwrap().opacity(), 1.0);
    }

    #[test]
    fn the_reticle_centres_on_its_point_and_looks_smaller_farther_away() {
        let viewer = Camera {
            position: Vec3::new(2.0, 3.0, 4.0),
            yaw: 0.6,
            pitch: -0.3,
            roll: 0.2,
            ..Default::default()
        };
        let fov = ViewFov {
            left: -1.1,
            right: 0.9,
            down: -1.0,
            up: 1.0,
        };
        let view_projection = fov.view_projection(&viewer);
        let screen = |p: Vec3| {
            let clip = view_projection * p.extend(1.0);
            clip.truncate().truncate() / clip.w
        };
        let sizes = [5.0, 20.0, 80.0, 400.0].map(|distance| {
            let point = viewer.position + viewer.forward() * distance;
            let reticle = WorldReticle::facing(point, &viewer, PER_HEIGHT).unwrap();
            let corners = reticle.corners().map(screen);
            let middle = corners.iter().sum::<Vec2>() / 4.0;
            assert!(middle.distance(screen(point)) < 1e-3);
            assert!(corners[0].x < corners[1].x && corners[0].y > corners[3].y);
            corners[0].distance(corners[1])
        });
        assert!(sizes[0] > sizes[1] * 1.9, "{sizes:?}");
        assert!(sizes[1] > sizes[2] * 1.9, "{sizes:?}");
        assert!((sizes[2] - sizes[3]).abs() < 1e-3, "{sizes:?}");
    }
}
