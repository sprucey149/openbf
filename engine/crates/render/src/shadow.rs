//! BF1 authored vehicle volumes: capped z-fail at every distance (a deliberate
//! improvement over the PS2's uncapped far z-pass), then one display-space
//! multiply of 1 - 48/128. Overlaps darken only once. See re-vehicle-shadow §5.
use glam::{Mat4, Vec3};
use std::collections::HashMap;

/// Prepared once at asset load; frame work is linear in the authored mesh.
pub struct ShadowMesh {
    positions: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    normals: Vec<Vec3>,
    edges: Vec<(usize, usize, usize, usize)>,
    points: Vec<Vec3>,
    lit: Vec<bool>,
}
impl ShadowMesh {
    pub fn new(positions: &[[f32; 3]], faces: &[Vec<u16>]) -> Result<Self, String> {
        let positions: Vec<_> = positions.iter().copied().map(Vec3::from_array).collect();
        if positions.len() < 4 || faces.is_empty() || positions.iter().any(|p| !p.is_finite()) {
            return Err("empty or non-finite shadow mesh".into());
        }
        let mut pending = HashMap::new();
        let mut edges = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (f, face) in faces.iter().enumerate() {
            if face.len() < 3 || face.iter().any(|&v| v as usize >= positions.len()) {
                return Err("invalid shadow face".into());
            }
            for i in 0..face.len() {
                let a = face[i] as usize;
                let b = face[(i + 1) % face.len()] as usize;
                if a == b || !seen.insert((a, b)) {
                    return Err("duplicate shadow edge".into());
                }
                if let Some(other) = pending.remove(&(b, a)) {
                    edges.push((a, b, f, other));
                } else {
                    pending.insert((a, b), f);
                }
            }
        }
        if !pending.is_empty() {
            return Err("open shadow mesh".into());
        }
        // B1 0x4acb20 keeps the first consecutive cross with squared length
        // in (1e-5, 1e6), or the LAST cross if none qualifies. Tiny authored
        // gear faces exercise that fallback; rejecting them loses the X-wing.
        let normals = faces
            .iter()
            .map(|face| {
                let mut normal = Vec3::ZERO;
                for triple in face.windows(3) {
                    let a = positions[triple[0] as usize];
                    let b = positions[triple[1] as usize];
                    let c = positions[triple[2] as usize];
                    normal = (b - a).cross(c - b);
                    let size = normal.length_squared();
                    if size > 1e-5 && size < 1e6 {
                        break;
                    }
                }
                normal
            })
            .collect();
        Ok(Self {
            normals,
            points: vec![Vec3::ZERO; positions.len()],
            lit: vec![false; faces.len()],
            positions,
            faces: faces
                .iter()
                .map(|f| f.iter().map(|&v| v as usize).collect())
                .collect(),
            edges,
        })
    }
    /// Append outward-facing triangles, already in world space. Caps are
    /// retained even when the view is inside or farther than the PS2's 30 m.
    pub fn append_volume(&mut self, transform: Mat4, extrusion: Vec3, out: &mut Vec<[f32; 3]>) {
        if extrusion.length_squared() < 1e-8 {
            return;
        }
        for (p, &original) in self.points.iter_mut().zip(&self.positions) {
            *p = transform.transform_point3(original);
        }
        let points = &self.points;
        let local_light = transform.inverse().transform_vector3(extrusion);
        for (lit, normal) in self.lit.iter_mut().zip(&self.normals) {
            *lit = normal.dot(local_light) < 0.0;
        }
        let lit = &self.lit;
        let mut tri =
            |a: Vec3, b: Vec3, c: Vec3| out.extend([a.to_array(), b.to_array(), c.to_array()]);
        for (f, face) in self.faces.iter().enumerate() {
            let shift = if lit[f] { Vec3::ZERO } else { extrusion };
            for i in 1..face.len() - 1 {
                tri(
                    points[face[0]] + shift,
                    points[face[i]] + shift,
                    points[face[i + 1]] + shift,
                );
            }
        }
        for &(a, b, f, g) in &self.edges {
            if lit[f] == lit[g] {
                continue;
            }
            let (a, b) = if lit[f] {
                (points[a], points[b])
            } else {
                (points[b], points[a])
            };
            tri(b, a, a + extrusion);
            tri(b, a + extrusion, b + extrusion);
        }
    }
}

pub(crate) struct ShadowState {
    vertices: Vec<[f32; 3]>,
    buffer: wgpu::Buffer,
    capacity: usize,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    volume: wgpu::RenderPipeline,
    shade: wgpu::RenderPipeline,
    display_format: wgpu::TextureFormat,
}
impl ShadowState {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        display: Option<wgpu::TextureFormat>,
    ) -> Self {
        let display_format = display.unwrap_or(format);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vehicle shadows"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow camera"),
            entries: &[crate::bindings::uniform_layout(
                0,
                wgpu::ShaderStages::VERTEX,
                false,
                None,
            )],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let uniform = crate::resources::buffer(
            device,
            "shadow camera",
            64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow camera"),
            layout: &bgl,
            entries: &[crate::bindings::entire_buffer(0, &uniform)],
        });
        let face = |op| wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: op,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let make = |shade: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if shade {
                    "shadow darkening"
                } else {
                    "shadow z-fail"
                }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(if shade { "vs_full" } else { "vs_volume" }),
                    buffers: if shade {
                        &[]
                    } else {
                        &[wgpu::VertexBufferLayout {
                            array_stride: 12,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            }],
                        }]
                    },
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::device::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: if shade {
                        wgpu::CompareFunction::Always
                    } else {
                        wgpu::CompareFunction::LessEqual
                    },
                    stencil: if shade {
                        wgpu::StencilState {
                            front: wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::NotEqual,
                                ..Default::default()
                            },
                            back: wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::NotEqual,
                                ..Default::default()
                            },
                            read_mask: 255,
                            write_mask: 0,
                        }
                    } else {
                        wgpu::StencilState {
                            front: face(wgpu::StencilOperation::DecrementWrap),
                            back: face(wgpu::StencilOperation::IncrementWrap),
                            read_mask: 255,
                            write_mask: 255,
                        }
                    },
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if !shade {
                        "fs_volume"
                    } else if display_format.is_srgb() {
                        "fs_linear_fallback"
                    } else {
                        "fs_black"
                    }),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: if shade { display_format } else { format },
                        blend: shade.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: if shade {
                            wgpu::ColorWrites::COLOR
                        } else {
                            wgpu::ColorWrites::empty()
                        },
                    })],
                    compilation_options: Default::default(),
                }),
                multiview: None,
                cache: None,
            })
        };
        Self {
            vertices: Vec::new(),
            buffer: crate::resources::buffer(
                device,
                "shadow triangles",
                12,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            capacity: 1,
            uniform,
            bind,
            volume: make(false),
            shade: make(true),
            display_format,
        }
    }
    pub fn clear(&mut self) {
        self.vertices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, camera: Mat4) {
        if self.vertices.is_empty() {
            return;
        }
        if self.vertices.len() > self.capacity {
            self.capacity = self.vertices.len().next_power_of_two();
            self.buffer = crate::resources::buffer(
                device,
                "shadow triangles",
                (self.capacity * 12) as u64,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&camera.to_cols_array()),
        );
    }
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        view: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> u32 {
        if self.vertices.is_empty() {
            return 0;
        }
        let display = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.display_format),
            ..Default::default()
        });
        for shade in [false, true] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vehicle shadows"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if shade { &display } else { view },
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
                        load: if shade {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(0)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_stencil_reference(0);
            if shade {
                pass.set_pipeline(&self.shade);
                pass.draw(0..3, 0..1);
            } else {
                pass.set_pipeline(&self.volume);
                pass.set_vertex_buffer(0, self.buffer.slice(..));
                pass.draw(0..self.vertices.len() as u32, 0..1);
            }
        }
        2
    }
}
impl crate::Renderer {
    /// Submit BF1 shadow triangles for the next frame only; an empty slice disables the pass.
    pub fn set_shadow_triangles(&mut self, triangles: &[[f32; 3]]) {
        self.shadows.vertices.clear();
        self.shadows.vertices.extend_from_slice(triangles);
    }
}
