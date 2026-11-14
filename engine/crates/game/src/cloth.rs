//! Cosmetic BF2 cloth, from CLTH and EntityClothClass. Integrator, list
//! order and collision projection follow B2 FUN_001dcdd0..001ddfc8
//! (`analysis/re-cloth.md` §3–4). No state enters the gameplay sim or RNG.

use anyhow::{bail, Context, Result};
use core::fmath::FloatExt;
use glam::{Mat4, Vec3};
use manifest::cloth::Cloth;

pub fn matrix(rows: &[[f32; 3]; 4]) -> Mat4 {
    Mat4::from_cols(
        Vec3::from(rows[0]).extend(0.0),
        Vec3::from(rows[1]).extend(0.0),
        Vec3::from(rows[2]).extend(0.0),
        Vec3::from(rows[3]).extend(1.0),
    )
}

pub struct Definition {
    pub raw: Cloth,
    parent: usize,
    frame: Mat4,
    pins: Vec<(usize, Vec3)>,
    constraints: [Vec<(usize, usize, f32)>; 3],
    shapes: Vec<(usize, u32, Vec3, Mat4)>,
}

impl Definition {
    pub fn new(raw: Cloth, skel: &crate::skin::Skeleton) -> Result<Self> {
        let bone = |name: &str| {
            skel.index_of(name)
                .with_context(|| format!("cloth {}: bone {name} absent", raw.name))
        };
        if raw.pinned > raw.positions.len()
            || raw.uvs.len() != raw.positions.len()
            || (!raw.pin_bones.is_empty() && raw.pin_bones.len() != raw.pinned)
            || raw.positions.iter().any(|p| !Vec3::from(*p).is_finite())
            || raw.uvs.iter().flatten().any(|x| !x.is_finite())
            || raw.frame.iter().flatten().any(|x| !x.is_finite())
            || raw.shapes.iter().any(|s| {
                s.frame
                    .iter()
                    .flatten()
                    .chain(&s.params)
                    .any(|x| !x.is_finite())
            })
            || raw
                .wind_direction
                .iter()
                .chain([
                    &raw.wind_speed,
                    &raw.dampening,
                    &raw.drag,
                    &raw.max_acceleration,
                ])
                .any(|x| !x.is_finite())
            || !raw.mass.is_finite()
            || raw.mass <= 0.0
        {
            bail!("cloth {}: invalid vertex/pin/physical data", raw.name);
        }
        let parent = bone(&raw.parent)?;
        let frame = matrix(&raw.frame);
        if frame.determinant().abs() <= f32::EPSILON
            || raw
                .shapes
                .iter()
                .any(|s| matrix(&s.frame).determinant().abs() <= f32::EPSILON)
        {
            bail!("cloth {}: singular frame", raw.name);
        }
        let bind_frame = skel.global_bind[parent] * frame;
        let pins = (0..raw.pinned)
            .map(|i| {
                let b = if raw.pin_bones.is_empty() {
                    parent
                } else {
                    bone(&raw.pin_bones[i])?
                };
                let p = (skel.global_bind[b].inverse() * bind_frame)
                    .transform_point3(Vec3::from(raw.positions[i]));
                Ok((b, p))
            })
            .collect::<Result<_>>()?;
        let mut constraints: [Vec<(usize, usize, f32)>; 3] = Default::default();
        for (k, list) in raw.constraints.iter().enumerate() {
            for &[a, b] in list {
                let (a, b) = (a as usize, b as usize);
                if a >= raw.positions.len() || b >= raw.positions.len() {
                    bail!("cloth {}: constraint index", raw.name);
                }
                let rest = Vec3::from(raw.positions[a]).distance(Vec3::from(raw.positions[b]));
                constraints[k].push((a, b, rest));
            }
        }
        if raw
            .triangles
            .iter()
            .flatten()
            .any(|&i| i as usize >= raw.positions.len())
        {
            bail!("cloth {}: triangle index", raw.name);
        }
        let shapes = raw
            .shapes
            .iter()
            .map(|s| {
                if s.kind > 2 {
                    bail!("cloth {}: collision kind {}", raw.name, s.kind);
                }
                Ok((
                    bone(&s.bone)?,
                    s.kind,
                    Vec3::from(s.params),
                    matrix(&s.frame),
                ))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            raw,
            parent,
            frame,
            pins,
            constraints,
            shapes,
        })
    }

    pub fn frame(&self, globals: &[Mat4]) -> Mat4 {
        globals[self.parent] * self.frame
    }

    pub fn pin_positions(&self, globals: &[Mat4]) -> Vec<Vec3> {
        let inv = self.frame(globals).inverse();
        self.pins
            .iter()
            .map(|&(b, p)| (inv * globals[b]).transform_point3(p))
            .collect()
    }
}

pub struct State {
    pub positions: Vec<Vec3>,
    previous: Vec<Vec3>,
    normals: Vec<Vec3>,
    origin: Vec3,
    velocity: Vec3,
    seed: u32,
}

impl State {
    pub fn new(def: &Definition, model: Mat4, globals: &[Mat4], seed: u32) -> Self {
        let positions: Vec<Vec3> = def.raw.positions.iter().copied().map(Vec3::from).collect();
        Self {
            previous: positions.clone(),
            normals: normals(&positions, &def.raw.triangles),
            positions,
            origin: (model * def.frame(globals)).transform_point3(Vec3::ZERO),
            velocity: Vec3::ZERO,
            seed,
        }
    }

    /// The PS2 caps dt at .039 and frame velocity at 10 m/s. Acceleration's
    /// clamp, drag, mass and wind are the authored class values/defaults.
    pub fn step(&mut self, def: &Definition, model: Mat4, globals: &[Mat4], dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.fmin(0.039);
        let world = model * def.frame(globals);
        let origin = world.transform_point3(Vec3::ZERO);
        let velocity = ((origin - self.origin) / dt).clamp_length_max(10.0);
        let accel =
            ((velocity - self.velocity) / dt).clamp_length_max(def.raw.max_acceleration.fmax(0.0));
        self.origin = origin;
        self.velocity = velocity;
        // Presentation scales (the spawn line-up) do not scale gravity.
        let inverse = glam::Mat3::from_cols(
            world.x_axis.truncate().normalize(),
            world.y_axis.truncate().normalize(),
            world.z_axis.truncate().normalize(),
        )
        .transpose();
        let gravity = inverse * (Vec3::new(0.0, -9.8, 0.0) - def.raw.dampening * accel);
        let wind = inverse * Vec3::from(def.raw.wind_direction);
        let frame_velocity = inverse * velocity;
        // [ours] Dedicated cosmetic stream; the PS2 advances its shared LCG
        // twice per cloth update. Its float construction and force stay exact.
        self.seed = self.seed.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
        let hi = self.seed >> 16;
        self.seed = self.seed.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
        let random = f32::from_bits(((hi | (self.seed & 0xffff0000)) >> 9) | 0x3f800000) - 1.0;
        for i in def.raw.pinned..self.positions.len() {
            let p = self.positions[i];
            let local_velocity = (p - self.previous[i]) / dt + frame_velocity;
            let n = self.normals[i];
            let force =
                def.raw.drag * local_velocity + n * (random * def.raw.wind_speed * n.dot(wind));
            self.positions[i] =
                p + (p - self.previous[i]) + (gravity + force / def.raw.mass) * dt * dt;
            self.previous[i] = p;
        }
        // Pinned positions follow the actual posed bones, in this frame.
        for (i, p) in def.pin_positions(globals).into_iter().enumerate() {
            self.positions[i] = p;
        }
        for k in (0..3).rev().filter(|&k| def.raw.enabled[k]) {
            for &(a, b, rest) in &def.constraints[k] {
                constrain(&mut self.positions, def.raw.pinned, a, b, rest);
            }
        }
        let inv_frame = def.frame(globals).inverse();
        for &(bone, kind, params, local) in &def.shapes {
            let shape = inv_frame * globals[bone] * local;
            for p in &mut self.positions[def.raw.pinned..] {
                collide(p, kind, params, shape);
            }
        }
        self.normals = normals(&self.positions, &def.raw.triangles);
    }

    /// Deformed vertices in model space, matching the ordinary body draw.
    pub fn vertices(&self, def: &Definition, globals: &[Mat4]) -> Vec<render::Vertex> {
        let frame = def.frame(globals);
        self.positions
            .iter()
            .zip(&self.normals)
            .zip(&def.raw.uvs)
            .map(|((&p, &n), &uv)| render::Vertex {
                position: frame.transform_point3(p).to_array(),
                normal: frame.transform_vector3(n).normalize_or_zero().to_array(),
                uv,
                color: [255; 4],
            })
            .collect()
    }
}

fn constrain(p: &mut [Vec3], pinned: usize, a: usize, b: usize, rest: f32) {
    let delta = p[b] - p[a];
    let length = delta.length();
    // [ours] Degenerate data must remain finite rather than divide by zero.
    if length <= f32::EPSILON {
        return;
    }
    let correction = delta * ((length - rest) / length);
    if a < pinned {
        p[b] -= correction;
    } else if b < pinned {
        p[a] += correction;
    } else {
        p[a] += correction * 0.5;
        p[b] -= correction * 0.5;
    }
}

fn collide(p: &mut Vec3, kind: u32, params: Vec3, shape: Mat4) {
    let mut q = shape.inverse().transform_point3(*p);
    let before = q;
    match kind {
        0 => {
            let length = q.length();
            if length < params.x && length > f32::EPSILON {
                q *= params.x / length;
            }
        }
        1 => {
            // The PS2 cylinder's axis is row 1; half length is twice p1.
            // Its axial branch adds positive axial depth even on -Y.
            let radial = Vec3::new(q.x, 0.0, q.z);
            let r = radial.length();
            let dr = params.x - r;
            let dy = 2.0 * params.y - q.y.abs();
            if dr > 0.0 && dy > 0.0 {
                if dr < dy {
                    if r > f32::EPSILON {
                        q += radial * (dr / r);
                    }
                } else {
                    q.y += dy;
                }
            }
        }
        2 => {
            let depth = params - q.abs();
            if depth.min_element() > 0.0 {
                let axis = if depth.x < depth.y {
                    if depth.x < depth.z {
                        0
                    } else {
                        2
                    }
                } else if depth.y < depth.z {
                    1
                } else {
                    2
                };
                q[axis] += if q[axis] < 0.0 {
                    -depth[axis]
                } else {
                    depth[axis]
                };
            }
        }
        _ => {}
    }
    if q != before {
        *p = shape.transform_point3(q);
    }
}

pub fn normals(p: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; p.len()];
    for &[a, b, c] in triangles {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        let n = (p[b] - p[a]).cross(p[c] - p[a]);
        for i in [a, b, c] {
            out[i] += n;
        }
    }
    out.iter_mut().for_each(|n| *n = n.normalize_or_zero());
    out
}

pub(crate) fn upload(
    r: &mut render::Renderer,
    existing: Option<render::MeshHandle>,
    vertices: &[render::Vertex],
    indices: &[u32],
) -> std::result::Result<render::MeshHandle, render::RenderError> {
    if let Some(mesh) = existing {
        r.update_vertices(mesh, vertices)?;
        return Ok(mesh);
    }
    let positions: Vec<_> = vertices.iter().map(|v| v.position).collect();
    let normals: Vec<_> = vertices.iter().map(|v| v.normal).collect();
    let uvs: Vec<_> = vertices.iter().map(|v| v.uv).collect();
    r.add_dynamic_mesh(&render::MeshData {
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        colors: None,
        indices,
    })
}

#[cfg(test)]
mod tests;
