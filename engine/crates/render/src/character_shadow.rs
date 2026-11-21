//! Screen-resolution shadow volumes from the same posed geometry as the
//! character draw. Closing each triangle's extrusion also handles open
//! render meshes. Matching side walls cancel before upload.

use crate::mesh::GpuMesh;
use crate::{DrawCmd, DrawLayer, LightPath, Renderer, SkinnedDrawCmd};
use glam::{Mat4, Vec3};

// OURS: bound work to nearby characters and keep the near cap clear of the
// receiver at ground contact. measure: BF1 PC caster cutoff and contact.
const CASTER_RANGE: f32 = 28.0;
const LENGTH: f32 = 40.0;
const CONTACT_BIAS: f32 = 0.003;

pub(crate) struct VolumeMesh {
    pub positions: Vec<[f32; 3]>,
    skin: Vec<([u8; 4], [f32; 4])>,
    faces: Vec<[u32; 3]>,
    edges: Vec<EdgeUse>,
}

struct EdgeUse {
    edge: [u32; 2],
    winding: i8,
    face: usize,
}

impl VolumeMesh {
    pub fn new(positions: &[[f32; 3]], indices: &[u32]) -> Self {
        let mut edges = Vec::with_capacity(indices.len());
        let faces = indices
            .chunks_exact(3)
            .enumerate()
            .map(|(face, tri)| {
                let [a, b, c] = [tri[0], tri[1], tri[2]];
                for (from, to) in [(a, b), (b, c), (c, a)] {
                    edges.push(EdgeUse {
                        edge: [from.min(to), from.max(to)],
                        winding: if from < to { 1 } else { -1 },
                        face,
                    });
                }
                [a, b, c]
            })
            .collect();
        edges.sort_unstable_by_key(|usage| usage.edge);
        Self {
            positions: positions.to_vec(),
            skin: Vec::new(),
            faces,
            edges,
        }
    }

    pub fn skinned(data: &crate::SkinnedMeshData<'_>) -> Self {
        let mut mesh = Self::new(data.positions, data.indices);
        mesh.skin = data
            .joints
            .iter()
            .copied()
            .zip(data.weights.iter().copied())
            .collect();
        mesh
    }
}

#[derive(Default)]
struct Scratch {
    points: Vec<Vec3>,
    facing: Vec<i8>,
}

impl Scratch {
    fn append(
        &mut self,
        mesh: &VolumeMesh,
        model: Mat4,
        palette: &[Mat4],
        sun: Vec3,
        out: &mut Vec<[f32; 3]>,
    ) {
        let bias = sun * CONTACT_BIAS;
        self.points.clear();
        if palette.is_empty() {
            self.points.extend(
                mesh.positions
                    .iter()
                    .map(|&p| model.transform_point3(p.into()) + bias),
            );
        } else {
            self.points
                .extend(
                    mesh.positions
                        .iter()
                        .zip(&mesh.skin)
                        .map(|(&p, &(joints, weights))| {
                            let p = Vec3::from(p);
                            let posed = joints
                                .into_iter()
                                .zip(weights)
                                .filter(|&(_, weight)| weight != 0.0)
                                .fold(Vec3::ZERO, |sum, (joint, weight)| {
                                    sum + palette[joint as usize].transform_point3(p) * weight
                                });
                            model.transform_point3(posed) + bias
                        }),
                );
        }
        let extrusion = sun * LENGTH;
        let mut tri =
            |a: Vec3, b: Vec3, c: Vec3| out.extend([a.to_array(), b.to_array(), c.to_array()]);
        self.facing.clear();
        for &[a, b, c] in &mesh.faces {
            let [a, mut b, mut c] = [a, b, c].map(|v| self.points[v as usize]);
            let area = (b - a).cross(c - a).dot(sun);
            if !area.is_finite() || area == 0.0 {
                self.facing.push(0);
                continue;
            }
            let facing = if area < 0.0 { 1 } else { -1 };
            self.facing.push(facing);
            if facing < 0 {
                std::mem::swap(&mut b, &mut c);
            }
            tri(a, b, c);
            tri(a + extrusion, c + extrusion, b + extrusion);
        }
        for uses in mesh.edges.chunk_by(|a, b| a.edge == b.edge) {
            let winding: i32 = uses
                .iter()
                .map(|usage| i32::from(usage.winding * self.facing[usage.face]))
                .sum();
            let [mut a, mut b] = uses[0].edge.map(|v| self.points[v as usize]);
            if winding < 0 {
                std::mem::swap(&mut a, &mut b);
            }
            for _ in 0..winding.unsigned_abs() {
                tri(b, a, a + extrusion);
                tri(b, a + extrusion, b + extrusion);
            }
        }
    }
}

pub(crate) fn rigid_caster<'a>(draw: &DrawCmd, meshes: &'a [GpuMesh]) -> Option<&'a VolumeMesh> {
    if draw.layer == DrawLayer::Opaque && draw.light == LightPath::Character {
        meshes[draw.mesh.0 as usize].shadow.as_ref()
    } else {
        None
    }
}

impl Renderer {
    pub(crate) fn prepare_character_shadows(
        &mut self,
        eye: Vec3,
        sun: Vec3,
        camera: Mat4,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
    ) {
        let nearby =
            |model: Mat4| eye.distance_squared(model.w_axis.truncate()) < CASTER_RANGE.powi(2);
        let mut scratch = Scratch::default();
        let volumes = self.shadows.vertices_mut();
        volumes.clear();
        for d in draws {
            if let Some(mesh) = rigid_caster(d, &self.meshes).filter(|_| nearby(d.model)) {
                scratch.append(mesh, d.model, &[], sun, volumes);
            }
        }
        if !self.frame_ab.skips("soldiers") {
            for d in skinned {
                if d.layer == DrawLayer::Opaque && d.light != LightPath::Iface && nearby(d.model) {
                    let mesh = self.skinned_meshes[d.mesh.0 as usize]
                        .shadow
                        .as_ref()
                        .unwrap();
                    scratch.append(mesh, d.model, &d.palette, sun, volumes);
                }
            }
        }
        self.shadows.prepare(&self.device, &self.queue, camera);
    }
}
