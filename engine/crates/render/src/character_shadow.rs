//! Screen-resolution shadow volumes from the same posed geometry as the
//! character draw. Closing each triangle's extrusion also handles open
//! render meshes. Matching side walls cancel before upload.

use crate::frustum::{self, Frustum};
use crate::mesh::GpuMesh;
use crate::{DrawCmd, DrawLayer, LightPath, Renderer, SkinnedDrawCmd, Vertex};
use glam::{Mat4, Vec3};
use std::collections::HashMap;

// OURS: bound work to nearby characters and keep the near cap clear of the
// receiver at ground contact. measure: BF1 PC caster cutoff and contact.
const CASTER_RANGE: f32 = 28.0;
const LENGTH: f32 = 40.0;
const CONTACT_BIAS: f32 = 0.003;
// High-poly modded characters and weapons can exceed 10k-20k triangles per mesh.
// Clustering above this threshold keeps a tight silhouette proxy while keeping
// CPU extrusion and stencil fillrate bounded on large battles.
const MAX_VOLUME_TRIS: usize = 256;

pub(crate) struct VolumeMesh {
    pub positions: Vec<[f32; 3]>,
    source_vertices: Vec<u32>,
    skin: Vec<([u8; 4], [f32; 4])>,
    faces: Vec<[u32; 3]>,
    interior_edges: Vec<InteriorEdge>,
    other_edges: Vec<EdgeRun>,
    other_uses: Vec<EdgeUse>,
    closed: bool,
}

#[derive(Clone, Copy)]
struct InteriorEdge {
    a: u32,
    b: u32,
    f0: u32,
    f1: u32,
}

#[derive(Clone, Copy)]
struct EdgeRun {
    a: u32,
    b: u32,
    start: u32,
    len: u32,
}

#[derive(Clone, Copy)]
struct EdgeUse {
    winding: i8,
    face: u32,
}

#[inline]
fn canon_bits(v: f32) -> u32 {
    if v == 0.0 {
        0
    } else {
        v.to_bits()
    }
}

type SkinStreams<'a> = (&'a [[u8; 4]], &'a [[f32; 4]]);
type WeldedGeometry = (
    Vec<[f32; 3]>,
    Vec<u32>,
    Vec<([u8; 4], [f32; 4])>,
    Vec<[u32; 3]>,
);

fn build_geometry(
    positions: &[[f32; 3]],
    skin: Option<SkinStreams<'_>>,
    indices: &[u32],
) -> WeldedGeometry {
    // First pass: weld bit-identical coincident positions (UV/normal seams)
    // while keeping exact original vertex coordinates.
    let mut exact_map: HashMap<[u32; 3], u32> = HashMap::with_capacity(positions.len());
    let mut remap: Vec<u32> = Vec::with_capacity(positions.len());
    let mut out_pos: Vec<[f32; 3]> = Vec::new();
    let mut out_src: Vec<u32> = Vec::new();
    let mut out_skin: Vec<([u8; 4], [f32; 4])> = Vec::new();

    for (i, &p) in positions.iter().enumerate() {
        let key = [canon_bits(p[0]), canon_bits(p[1]), canon_bits(p[2])];
        let id = *exact_map.entry(key).or_insert_with(|| {
            let id = out_pos.len() as u32;
            out_pos.push(p);
            out_src.push(i as u32);
            if let Some((joints, weights)) = skin {
                out_skin.push((joints[i], weights[i]));
            }
            id
        });
        remap.push(id);
    }

    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(indices.len() / 3);
    for tri in indices.chunks_exact(3) {
        let a = remap[tri[0] as usize];
        let b = remap[tri[1] as usize];
        let c = remap[tri[2] as usize];
        if a != b && b != c && c != a {
            faces.push([a, b, c]);
        }
    }

    if faces.len() <= MAX_VOLUME_TRIS || out_pos.is_empty() {
        return (out_pos, out_src, out_skin, faces);
    }

    // Spatial vertex clustering for high-poly meshes (e.g. HD modded models).
    // Step down grid resolution until the proxy fits within MAX_VOLUME_TRIS.
    let mut lo = Vec3::from(out_pos[0]);
    let mut hi = lo;
    for &p in &out_pos[1..] {
        let v = Vec3::from(p);
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let extent = (hi - lo).max_element().max(1e-4);

    let mut best = (out_pos, out_src, out_skin, faces);
    for res in [24u32, 18, 14, 10, 8, 6] {
        let scale = res as f32 / extent;
        let mut cell_map: HashMap<[i32; 3], u32> = HashMap::new();
        let mut c_remap: Vec<u32> = Vec::with_capacity(positions.len());
        let mut c_pos: Vec<[f32; 3]> = Vec::new();
        let mut c_src: Vec<u32> = Vec::new();
        let mut c_skin: Vec<([u8; 4], [f32; 4])> = Vec::new();

        for (i, &p) in positions.iter().enumerate() {
            let rel = (Vec3::from(p) - lo) * scale;
            let key = [
                rel.x.floor() as i32,
                rel.y.floor() as i32,
                rel.z.floor() as i32,
            ];
            let id = *cell_map.entry(key).or_insert_with(|| {
                let id = c_pos.len() as u32;
                c_pos.push(p);
                c_src.push(i as u32);
                if let Some((joints, weights)) = skin {
                    c_skin.push((joints[i], weights[i]));
                }
                id
            });
            c_remap.push(id);
        }

        let mut seen_tris: HashMap<[u32; 3], ()> = HashMap::new();
        let mut c_faces: Vec<[u32; 3]> = Vec::new();
        for tri in indices.chunks_exact(3) {
            let a = c_remap[tri[0] as usize];
            let b = c_remap[tri[1] as usize];
            let c = c_remap[tri[2] as usize];
            if a == b || b == c || c == a {
                continue;
            }
            let mut sorted = [a, b, c];
            sorted.sort_unstable();
            if seen_tris.insert(sorted, ()).is_none() {
                c_faces.push([a, b, c]);
            }
        }
        best = (c_pos, c_src, c_skin, c_faces);
        if best.3.len() <= MAX_VOLUME_TRIS {
            break;
        }
    }
    best
}

impl VolumeMesh {
    fn from_parts(
        positions: Vec<[f32; 3]>,
        source_vertices: Vec<u32>,
        skin: Vec<([u8; 4], [f32; 4])>,
        faces: Vec<[u32; 3]>,
    ) -> Self {
        let mut raw_edges: Vec<([u32; 2], i8, u32)> = Vec::with_capacity(faces.len() * 3);
        for (face, &[a, b, c]) in faces.iter().enumerate() {
            let face = face as u32;
            for (from, to) in [(a, b), (b, c), (c, a)] {
                let (e, w) = if from < to {
                    ([from, to], 1i8)
                } else {
                    ([to, from], -1i8)
                };
                raw_edges.push((e, w, face));
            }
        }
        raw_edges.sort_unstable_by_key(|&(edge, _, _)| edge);

        let mut interior_edges = Vec::new();
        let mut other_edges = Vec::new();
        let mut other_uses = Vec::new();
        for group in raw_edges.chunk_by(|a, b| a.0 == b.0) {
            let [a, b] = group[0].0;
            if group.len() == 2 && group[0].1 == -group[1].1 {
                let (f0, f1) = if group[0].1 > 0 {
                    (group[0].2, group[1].2)
                } else {
                    (group[1].2, group[0].2)
                };
                interior_edges.push(InteriorEdge { a, b, f0, f1 });
            } else {
                let start = other_uses.len() as u32;
                for &(_, winding, face) in group {
                    other_uses.push(EdgeUse { winding, face });
                }
                other_edges.push(EdgeRun {
                    a,
                    b,
                    start,
                    len: group.len() as u32,
                });
            }
        }
        let closed = !faces.is_empty() && other_edges.is_empty();
        Self {
            positions,
            source_vertices,
            skin,
            faces,
            interior_edges,
            other_edges,
            other_uses,
            closed,
        }
    }

    pub fn new(positions: &[[f32; 3]], indices: &[u32]) -> Self {
        let (positions, source_vertices, skin, faces) = build_geometry(positions, None, indices);
        Self::from_parts(positions, source_vertices, skin, faces)
    }

    pub fn skinned(data: &crate::SkinnedMeshData<'_>) -> Self {
        let (positions, source_vertices, skin, faces) = build_geometry(
            data.positions,
            Some((data.joints, data.weights)),
            data.indices,
        );
        Self::from_parts(positions, source_vertices, skin, faces)
    }

    pub fn update_positions(&mut self, vertices: &[Vertex]) {
        for (dst, &src) in self.positions.iter_mut().zip(&self.source_vertices) {
            if let Some(v) = vertices.get(src as usize) {
                *dst = v.position;
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Scratch {
    points: Vec<Vec3>,
    facing: Vec<i8>,
    world_palette: Vec<Mat4>,
}

impl Scratch {
    fn append(
        &mut self,
        mesh: &VolumeMesh,
        model: Mat4,
        world_palette: &[Mat4],
        sun: Vec3,
        frustum: &Frustum,
        out: &mut Vec<[f32; 3]>,
    ) {
        if mesh.faces.is_empty() {
            return;
        }
        let bias = sun * CONTACT_BIAS;
        let extrusion = sun * LENGTH;
        self.points.clear();
        self.points.reserve(mesh.positions.len());

        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);

        if world_palette.is_empty() {
            for &p in &mesh.positions {
                let wp = model.transform_point3(Vec3::from(p)) + bias;
                lo = lo.min(wp);
                hi = hi.max(wp);
                self.points.push(wp);
            }
        } else {
            for (&p, &(joints, weights)) in mesh.positions.iter().zip(&mesh.skin) {
                let p = Vec3::from(p);
                let mut posed = world_palette[joints[0] as usize].transform_point3(p) * weights[0];
                if weights[1] != 0.0 {
                    posed += world_palette[joints[1] as usize].transform_point3(p) * weights[1];
                    if weights[2] != 0.0 {
                        posed += world_palette[joints[2] as usize].transform_point3(p) * weights[2];
                        if weights[3] != 0.0 {
                            posed +=
                                world_palette[joints[3] as usize].transform_point3(p) * weights[3];
                        }
                    }
                }
                let wp = posed + bias;
                lo = lo.min(wp);
                hi = hi.max(wp);
                self.points.push(wp);
            }
        }

        let swept_lo = lo.min(lo + extrusion);
        let swept_hi = hi.max(hi + extrusion);
        if !frustum.intersects_aabb(swept_lo, swept_hi) {
            return;
        }

        self.facing.clear();
        self.facing.reserve(mesh.faces.len());
        let mut tri =
            |a: Vec3, b: Vec3, c: Vec3| out.extend([a.to_array(), b.to_array(), c.to_array()]);

        if mesh.closed {
            // Closed manifold: a single cap per face (lit near cap, unlit far
            // cap) and a single quad per silhouette edge form one closed volume
            // with half the caps and half the side quads of per-face prisms.
            for &[ia, ib, ic] in &mesh.faces {
                let a = self.points[ia as usize];
                let b = self.points[ib as usize];
                let c = self.points[ic as usize];
                let lit = (b - a).cross(c - a).dot(sun) < 0.0;
                self.facing.push(if lit { 1 } else { -1 });
                if lit {
                    tri(a, b, c);
                } else {
                    tri(a + extrusion, b + extrusion, c + extrusion);
                }
            }
            for e in &mesh.interior_edges {
                let f0 = self.facing[e.f0 as usize];
                let f1 = self.facing[e.f1 as usize];
                if f0 == f1 {
                    continue;
                }
                let (a, b) = if f0 > 0 {
                    (self.points[e.a as usize], self.points[e.b as usize])
                } else {
                    (self.points[e.b as usize], self.points[e.a as usize])
                };
                tri(b, a, a + extrusion);
                tri(b, a + extrusion, b + extrusion);
            }
            return;
        }

        for &[ia, ib, ic] in &mesh.faces {
            let a = self.points[ia as usize];
            let mut b = self.points[ib as usize];
            let mut c = self.points[ic as usize];
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
        for e in &mesh.interior_edges {
            let f0 = self.facing[e.f0 as usize];
            let f1 = self.facing[e.f1 as usize];
            if f0 == f1 {
                continue;
            }
            let winding = i32::from(f0) - i32::from(f1);
            let (a, b) = if winding > 0 {
                (self.points[e.a as usize], self.points[e.b as usize])
            } else {
                (self.points[e.b as usize], self.points[e.a as usize])
            };
            for _ in 0..winding.unsigned_abs() {
                tri(b, a, a + extrusion);
                tri(b, a + extrusion, b + extrusion);
            }
        }
        for run in &mesh.other_edges {
            let uses = &mesh.other_uses[run.start as usize..(run.start + run.len) as usize];
            let winding: i32 = uses
                .iter()
                .map(|usage| i32::from(usage.winding * self.facing[usage.face as usize]))
                .sum();
            if winding == 0 {
                continue;
            }
            let (a, b) = if winding > 0 {
                (self.points[run.a as usize], self.points[run.b as usize])
            } else {
                (self.points[run.b as usize], self.points[run.a as usize])
            };
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
        frustum: &Frustum,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
    ) {
        let range_sq = CASTER_RANGE * CASTER_RANGE;
        let extrusion = sun * LENGTH;
        let nearby = |model: Mat4| eye.distance_squared(model.w_axis.truncate()) < range_sq;
        let (volumes, scratch) = self.shadows.vertices_and_scratch_mut();
        volumes.clear();
        for d in draws {
            if let Some(mesh) = rigid_caster(d, &self.meshes).filter(|_| nearby(d.model)) {
                let gpu = &self.meshes[d.mesh.0 as usize];
                let (lo, hi) = frustum::transformed_aabb(&d.model, gpu.bounds.0, gpu.bounds.1);
                let swept_lo = lo.min(lo + extrusion);
                let swept_hi = hi.max(hi + extrusion);
                if !frustum.intersects_aabb(swept_lo, swept_hi) {
                    continue;
                }
                scratch.append(mesh, d.model, &[], sun, frustum, volumes);
            }
        }
        if !self.frame_ab.skips("soldiers") {
            let mut cached_palette: Option<(Mat4, *const Mat4, usize)> = None;
            for d in skinned {
                if d.layer == DrawLayer::Opaque && d.light != LightPath::Iface && nearby(d.model) {
                    let gpu = &self.skinned_meshes[d.mesh.0 as usize];
                    let target = match gpu.shadow_source {
                        crate::mesh::ShadowSource::Own => gpu,
                        crate::mesh::ShadowSource::Proxy(p) => {
                            let proxy = &self.skinned_meshes[p.0 as usize];
                            if proxy.max_joint as usize >= d.palette.len() {
                                continue;
                            }
                            proxy
                        }
                        crate::mesh::ShadowSource::None => continue,
                    };
                    let Some(mesh) = target.shadow.as_ref() else {
                        continue;
                    };
                    let key = (d.model, d.palette.as_ptr(), d.palette.len());
                    if cached_palette != Some(key) {
                        scratch.world_palette.clear();
                        scratch.world_palette.reserve(d.palette.len());
                        for &j in d.palette.iter() {
                            scratch.world_palette.push(d.model * j);
                        }
                        cached_palette = Some(key);
                    }
                    // Take world_palette temporarily so `scratch.append` can borrow `scratch`.
                    let wp = std::mem::take(&mut scratch.world_palette);
                    scratch.append(mesh, d.model, &wp, sun, frustum, volumes);
                    scratch.world_palette = wp;
                }
            }
        }
        self.shadows.prepare(&self.device, &self.queue, camera);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_mesh_welds_uv_seams_and_bounds_high_poly_meshes() {
        // Closed cube with duplicated seam vertices (24 vertices, 12 triangles).
        let base = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let mut pos = Vec::new();
        let mut idx = Vec::new();
        let quads = [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [3, 7, 6, 2],
            [0, 4, 7, 3],
            [1, 2, 6, 5],
        ];
        for q in quads {
            let o = pos.len() as u32;
            for v in q {
                pos.push(base[v]);
            }
            idx.extend([o, o + 1, o + 2, o, o + 2, o + 3]);
        }
        let cube = VolumeMesh::new(&pos, &idx);
        assert_eq!(cube.positions.len(), 8);
        assert_eq!(cube.faces.len(), 12);
        assert!(cube.closed, "welded cube must be detected as closed");

        // High-poly 60x60 grid (7,200 tris) simplifies under MAX_VOLUME_TRIS.
        let n = 60usize;
        let mut gpos = Vec::new();
        let mut gidx = Vec::new();
        for y in 0..=n {
            for x in 0..=n {
                gpos.push([x as f32 / n as f32, 0.0, y as f32 / n as f32]);
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = (y * (n + 1) + x) as u32;
                let b = a + 1;
                let c = a + (n + 1) as u32;
                let d = c + 1;
                gidx.extend([a, b, d, a, d, c]);
            }
        }
        let hi = VolumeMesh::new(&gpos, &gidx);
        assert!(
            hi.faces.len() <= MAX_VOLUME_TRIS,
            "got {} faces, expected <= {MAX_VOLUME_TRIS}",
            hi.faces.len()
        );
        assert!(hi.faces.len() > 20, "proxy must keep non-trivial geometry");
    }
}
