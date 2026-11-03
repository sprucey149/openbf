use super::SunFlare;
use glam::Vec2;
use std::f32::consts::{PI, TAU};

const MAX_COPIES: usize = 256;
pub(super) const BUFFER_BYTES: u64 =
    ((MAX_COPIES * 60 + 4096) * std::mem::size_of::<Vertex>()) as u64;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
    source: [f32; 2],
    kind: f32,
}
impl Vertex {
    pub(super) fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTR: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x4, 2 => Float32x2, 3 => Float32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTR,
        }
    }
}
fn gs(c: [f32; 4]) -> [f32; 4] {
    c.map(|v| ((v + 1.0) * 0.5).floor() / 128.0)
}
struct Mesh {
    vertices: Vec<Vertex>,
    origin: Vec2,
    source_radius: f32,
}
impl Mesh {
    fn ring(
        &mut self,
        center: Vec2,
        radii: [f32; 3],
        sides: usize,
        colors: [[f32; 4]; 3],
        kind: f32,
    ) {
        if radii[2] <= 0.0 {
            return;
        }
        for band in 0..2 {
            if radii[band] == radii[band + 1] {
                continue;
            }
            for i in 0..sides {
                let at = |j: usize, edge: usize| {
                    let angle = j as f32 * TAU / sides as f32;
                    let offset = Vec2::new(angle.cos(), angle.sin()) * radii[edge];
                    Vertex {
                        position: (center + offset).to_array(),
                        color: colors[edge],
                        source: (self.origin + offset * (self.source_radius / radii[2])).to_array(),
                        kind,
                    }
                };
                let a = at(i, band);
                let b = at(i + 1, band);
                let c = at(i, band + 1);
                let d = at(i + 1, band + 1);
                if radii[band] == 0.0 {
                    self.vertices.extend([a, c, d]);
                } else {
                    self.vertices.extend([a, b, c, c, b, d]);
                }
            }
        }
    }
    fn spike(&mut self, angle: f32, radius: f32, stretch: Vec2, color: [f32; 4]) {
        let center = Vertex {
            position: self.origin.to_array(),
            color,
            source: self.origin.to_array(),
            kind: 1.0,
        };
        for i in 0..3 {
            let at = |j: usize| {
                let theta = angle + j as f32 * TAU / 3.0;
                let offset = Vec2::new(theta.cos(), -theta.sin()) * radius * stretch;
                Vertex {
                    position: (self.origin + offset).to_array(),
                    color: [0.0; 4],
                    ..center
                }
            };
            self.vertices.extend([center, at(i), at(i + 1)]);
        }
    }
}
pub(super) fn build(config: &SunFlare, p: &[[f32; 4]; 10]) -> Vec<Vertex> {
    let origin = Vec2::new(p[0][0], p[0][1]);
    let scale = p[0][2];
    let source_radius = config.size * scale;
    let mut m = Mesh {
        vertices: Vec::with_capacity(4096),
        origin,
        source_radius,
    };
    m.ring(
        origin,
        [p[5][0] * scale, p[5][1] * scale, p[5][2] * scale],
        30,
        [p[2], p[3], p[4]],
        0.0,
    );
    m.ring(origin, [0.0, 0.0, source_radius], 30, [p[1]; 3], 0.0);
    let viewport = Vec2::new(p[8][0], p[8][1]);
    if origin.x + source_radius < 0.0
        || origin.y + source_radius < 0.0
        || origin.x - source_radius > viewport.x
        || origin.y - source_radius > viewport.y
    {
        return m.vertices;
    }
    let copies = (config.num_flare_outs as usize).min(MAX_COPIES);
    for i in 0..copies {
        let tail = 1.0 - i as f32 / copies as f32;
        let alpha_byte = (config.initial_alpha * tail.powi(3)).round();
        if alpha_byte < 3.0 {
            break;
        }
        let size =
            config.size + (i + 1) as f32 * (config.flare_out_size - config.size) / copies as f32;
        m.ring(
            origin,
            [0.0, 0.0, size * scale],
            20,
            [gs([255.0, 255.0, 255.0, alpha_byte]); 3],
            2.0,
        );
    }
    let center = viewport * 0.5;
    let axis = origin - center;
    let ndc = axis / center;
    let fade = (1.0 - ndc.length_squared() * 0.5).max(0.0);
    let alpha_byte = (fade * p[7][3]).round();
    let [focal, facing, distance, alignment] = p[9];
    let spike_distance = config.spike_size * distance * 0.0111;
    let phase = spike_distance * 1.047_187_1 + (-axis.x * PI + axis.y * TAU) / focal;
    let radius = (fade + 1.0) * p[7][3] * spike_distance * 0.0025 * focal / (distance * facing);
    m.spike(phase, radius, Vec2::new(1.0, 2.0), p[6]);
    m.spike(1.047_187_1 - phase, radius, Vec2::new(2.5, 1.0), p[6]);
    let glare = 35.0 * 0.0111 * alignment * focal / facing;
    m.ring(
        origin,
        [0.0, glare, glare * 3.0],
        30,
        [
            gs([255.0, 255.0, 255.0, 0.0]),
            gs([255.0, 255.0, 255.0, (72.0 * alignment).round()]),
            gs([255.0, 255.0, 255.0, 0.0]),
        ],
        1.0,
    );
    // 0x472500's original polygon centers, radii, RGB and alpha-presence flags.
    let ghosts = [
        (
            -0.3,
            6,
            [0.0, 0.015, 0.02],
            [
                [27.0, 54.0, 13.0, 1.0],
                [13.0, 27.0, 13.0, 1.0],
                [13.0, 27.0, 27.0, 0.0],
            ],
        ),
        (
            -0.34,
            5,
            [0.0, 0.005, 0.01],
            [
                [108.0, 54.0, 54.0, 1.0],
                [54.0, 54.0, 54.0, 1.0],
                [54.0, 54.0, 108.0, 0.0],
            ],
        ),
        (
            -0.5,
            20,
            [0.0; 3],
            [
                [27.0, 13.0, 54.0, 0.0],
                [20.0, 27.0, 10.0, 1.0],
                [54.0, 13.0, 0.0, 0.0],
            ],
        ),
        (
            -0.775,
            6,
            [0.0, 0.01, 0.02],
            [
                [13.0, 20.0, 13.0, 1.0],
                [13.0, 20.0, 13.0, 1.0],
                [13.0, 20.0, 13.0, 0.0],
            ],
        ),
        (
            -0.825,
            8,
            [0.0, 0.02, 0.04],
            [
                [20.0, 34.0, 20.0, 1.0],
                [20.0, 34.0, 20.0, 1.0],
                [20.0, 34.0, 20.0, 0.0],
            ],
        ),
        (
            -0.895,
            8,
            [0.0, 0.01825, 0.0375],
            [
                [20.0, 34.0, 20.0, 1.0],
                [20.0, 34.0, 20.0, 1.0],
                [27.0, 40.0, 27.0, 0.0],
            ],
        ),
        (
            -0.95,
            7,
            [0.0, 0.015, 0.03],
            [
                [13.0, 20.0, 13.0, 1.0],
                [13.0, 20.0, 13.0, 1.0],
                [13.0, 20.0, 13.0, 0.0],
            ],
        ),
        (
            0.3,
            5,
            [0.0, 0.004, 0.008],
            [
                [136.0, 68.0, 27.0, 1.0],
                [108.0, 68.0, 13.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        ),
        (
            0.2,
            6,
            [0.0, 0.025, 0.04],
            [
                [68.0, 0.0, 0.0, 1.0],
                [54.0, 13.0, 0.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        ),
        (
            0.125,
            6,
            [0.0, 0.015, 0.025],
            [
                [61.0, 54.0, 0.0, 1.0],
                [27.0, 54.0, 0.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        ),
        (
            1.7,
            6,
            [0.0, 0.015, 0.02],
            [
                [0.0, 13.0, 81.0, 1.0],
                [0.0, 0.0, 68.0, 1.0],
                [0.0, 0.0, 68.0, 0.0],
            ],
        ),
        (
            1.5299999,
            5,
            [0.0, 0.0145, 0.0175],
            [
                [34.0, 0.0, 81.0, 1.0],
                [0.0, 0.0, 68.0, 1.0],
                [0.0, 0.0, 68.0, 0.0],
            ],
        ),
        (
            1.62,
            7,
            [0.0, 0.02, 0.0225],
            [
                [20.0, 27.0, 68.0, 1.0],
                [13.0, 20.0, 54.0, 1.0],
                [0.0, 0.0, 0.0, 0.0],
            ],
        ),
    ];
    for (i, (along, sides, mut radii, mut colors)) in ghosts.into_iter().enumerate() {
        if i == 2 {
            radii = [fade * 0.06, (fade + 1.0) * 0.0825, (fade + 1.0) * 0.1];
        }
        for color in &mut colors {
            color[3] *= alpha_byte;
            *color = gs(*color);
        }
        m.ring(
            center + axis * along,
            radii.map(|r| r * focal),
            sides,
            colors,
            1.0,
        );
    }
    assert!(m.vertices.len() * std::mem::size_of::<Vertex>() <= BUFFER_BYTES as usize);
    m.vertices
}
