//! Draw donor terrain on the canonical height surface, retaining its textures.
//!
//! The simulation grid is retained as an unindexed prefix in the base GLB.
//! Drawing uses a separate grid and the canonical quad diagonal. Cutter holes
//! are rebuilt by subtracting their public plane volumes from each triangle,
//! rather than stretching an incompatible donor's pre-clipped patch mesh.
//! The distant sheet uses the same surface with its own donor texture/UVs.

use super::*;
use anyhow::{bail, Result};
use mungefs::terrain::Cutter;
use std::collections::HashMap;

#[derive(Clone)]
struct Face {
    vertices: [u32; 3],
    /// Original grid triangle, for interpolating donor layer weights.
    grid: [u32; 3],
    patch: usize,
}

pub struct Surface {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colours: Vec<[u8; 3]>,
    faces: Vec<Face>,
}

fn distance(plane: &[f32; 4], p: [f64; 3]) -> f64 {
    plane[0] as f64 * p[0] + plane[1] as f64 * p[1] + plane[2] as f64 * p[2] + plane[3] as f64
}

fn split(poly: &[[f64; 3]], plane: &[f32; 4]) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    // A face on a cutter boundary is outside its open interior. Returning it
    // only once also avoids coplanar duplicates after later cutter planes.
    if poly.iter().all(|&p| distance(plane, p) >= 0.0) {
        return (Vec::new(), poly.to_vec());
    }
    if poly.iter().all(|&p| distance(plane, p) <= 0.0) {
        return (poly.to_vec(), Vec::new());
    }
    let (mut inside, mut outside) = (Vec::new(), Vec::new());
    for (a, b) in poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
    {
        let (da, db) = (distance(plane, *a), distance(plane, *b));
        if da <= 0.0 {
            inside.push(*a);
        }
        if da >= 0.0 {
            outside.push(*a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            let f = da / (da - db);
            let p = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f);
            inside.push(p);
            outside.push(p);
        }
    }
    (inside, outside)
}

fn reaches(cutter: &Cutter, tri: &[[f64; 3]; 3]) -> bool {
    if cutter.planes.is_empty() {
        return false;
    }
    let [radius, cx, cy, cz] = cutter.sphere;
    if radius <= 0.0 {
        return true;
    }
    let centres = [cx as f64, cy as f64, cz as f64];
    let square: f64 = (0..3)
        .map(|i| {
            let lo = tri.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
            let hi = tri.iter().map(|p| p[i]).fold(f64::NEG_INFINITY, f64::max);
            (lo - centres[i]).max(0.0).max(centres[i] - hi).powi(2)
        })
        .sum();
    square <= (radius as f64).powi(2)
}

/// Subtract a convex cutter. Each plane peels off a surviving outside polygon;
/// only the part inside every plane is discarded. A boundary has zero area.
fn subtract(poly: Vec<[f64; 3]>, cutter: &Cutter) -> Vec<Vec<[f64; 3]>> {
    let mut remaining = poly;
    let mut kept = Vec::new();
    for plane in &cutter.planes {
        if remaining.len() < 3 {
            break;
        }
        let (inside, outside) = split(&remaining, plane);
        if outside.len() >= 3 {
            kept.push(outside);
        }
        remaining = inside;
    }
    kept
}

fn barycentric(p: [f32; 3], tri: [[f32; 3]; 3]) -> [f32; 3] {
    let [a, b, c] = tri;
    let d =
        (b[2] - c[2]) as f64 * (a[0] - c[0]) as f64 + (c[0] - b[0]) as f64 * (a[2] - c[2]) as f64;
    let aweight = ((b[2] - c[2]) as f64 * (p[0] - c[0]) as f64
        + (c[0] - b[0]) as f64 * (p[2] - c[2]) as f64)
        / d;
    let bweight = ((c[2] - a[2]) as f64 * (p[0] - c[0]) as f64
        + (a[0] - c[0]) as f64 * (p[2] - c[2]) as f64)
        / d;
    [
        aweight as f32,
        bweight as f32,
        (1.0 - aweight - bweight) as f32,
    ]
}

impl Surface {
    pub fn build(t: &Terrain, bf2: bool, heights: &[f32]) -> Result<Self> {
        let n = t.info.grid_size as usize;
        if heights.len() != n * n || heights.iter().any(|v| !v.is_finite()) {
            bail!("invalid render height grid");
        }
        let g = ground(t, bf2);
        let mut out = Self {
            positions: g.positions[..n * n].to_vec(),
            normals: Vec::with_capacity(n * n),
            colours: g
                .colours
                .map_or_else(|| vec![[128; 3]; n * n], |c| c[..n * n].to_vec()),
            faces: Vec::with_capacity((n - 1) * (n - 1) * 2),
        };
        let h = |x: usize, z: usize| heights[z * n + x];
        for z in 0..n {
            for x in 0..n {
                out.positions[z * n + x][1] = h(x, z);
                let dx = (h((x + 1).min(n - 1), z) - h(x.saturating_sub(1), z))
                    / (2.0 * t.info.grid_unit);
                let dz = (h(x, (z + 1).min(n - 1)) - h(x, z.saturating_sub(1)))
                    / (2.0 * t.info.grid_unit);
                let inv = 1.0 / (dx * dx + 1.0 + dz * dz).sqrt();
                out.normals.push([-dx * inv, inv, -dz * inv]);
            }
        }
        let (per_row, cells) = t
            .patch_grid()
            .ok_or_else(|| anyhow::anyhow!("render terrain patch grid missing"))?;
        for z in 0..n - 1 {
            for x in 0..n - 1 {
                let a = (z * n + x) as u32;
                let patch = x / cells + (z / cells) * per_row;
                for grid in quad_triangles(z as u32, [a, a + 1, a + n as u32, a + n as u32 + 1]) {
                    let tri = grid.map(|v| out.positions[v as usize].map(f64::from));
                    let cutters: Vec<_> = t.cutters.iter().filter(|c| reaches(c, &tri)).collect();
                    if cutters.is_empty() {
                        out.faces.push(Face {
                            vertices: grid,
                            grid,
                            patch,
                        });
                        continue;
                    }
                    let mut polys = vec![tri.to_vec()];
                    for cutter in cutters {
                        polys = polys
                            .into_iter()
                            .flat_map(|p| subtract(p, cutter))
                            .collect();
                    }
                    for poly in polys {
                        let base = out.positions.len() as u32;
                        for p in &poly {
                            let p = p.map(|v| v as f32);
                            let weights = barycentric(p, grid.map(|v| out.positions[v as usize]));
                            let colour = std::array::from_fn(|i| {
                                (0..3)
                                    .map(|j| weights[j] * out.colours[grid[j] as usize][i] as f32)
                                    .sum::<f32>()
                                    .round()
                                    .clamp(0.0, 255.0) as u8
                            });
                            let normal: [f32; 3] = std::array::from_fn(|i| {
                                (0..3)
                                    .map(|j| weights[j] * out.normals[grid[j] as usize][i])
                                    .sum()
                            });
                            out.positions.push(p);
                            out.normals.push(normal);
                            out.colours.push(colour);
                        }
                        for i in 1..poly.len() - 1 {
                            // Clipping on an existing edge can repeat a corner.
                            // Zero projected area has no drawn surface.
                            let a = poly[0];
                            let b = poly[i];
                            let c = poly[i + 1];
                            if (b[0] - a[0]) * (c[2] - a[2]) == (b[2] - a[2]) * (c[0] - a[0]) {
                                continue;
                            }
                            out.faces.push(Face {
                                vertices: [base, base + i as u32, base + i as u32 + 1],
                                grid,
                                patch,
                            });
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn positions_indices(&self) -> (Vec<[f32; 3]>, Vec<u32>) {
        (
            self.positions.clone(),
            self.faces.iter().flat_map(|f| f.vertices).collect(),
        )
    }

    pub fn base_glb(&self, t: &Terrain) -> Result<Vec<u8>> {
        let n = t.info.grid_size;
        let query_count = n * n;
        let mut positions = Vec::with_capacity(query_count as usize + self.positions.len());
        for z in 0..n {
            for x in 0..n {
                let (wx, wz) = t.world_xz(x, z);
                positions.push([wx, t.world_y(x, z), wz]);
            }
        }
        positions.extend_from_slice(&self.positions);
        let mut normals = self.normals[..query_count as usize].to_vec();
        normals.extend_from_slice(&self.normals);
        let mut colours = self.colours[..query_count as usize].to_vec();
        colours.extend_from_slice(&self.colours);
        let uv: Vec<_> = positions
            .iter()
            .flat_map(|p| layer_uv(t, 0, false, grid_coords(t, *p)))
            .collect();
        let indices: Vec<_> = self
            .faces
            .iter()
            .flat_map(|f| f.vertices.map(|v| v + query_count))
            .collect();
        let mut bin = Bin::default();
        let (min, max) = min_max3(&positions);
        let pos = bin.f32s(&flat3(&positions), "VEC3", json!({"min":min,"max":max}));
        let nrm = bin.f32s(&flat3(&normals), "VEC3", json!({}));
        let uv = bin.f32s(&uv, "VEC2", json!({}));
        let rgba: Vec<_> = colours
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect();
        let col = bin.accessor(
            &rgba,
            5121,
            positions.len(),
            "VEC4",
            json!({"normalized":true}),
        );
        let idx = bin.u32s(&indices, json!({}));
        crate::gltf_bin::rigid_glb(
            bin,
            json!({"name":t.name,"extras":{"render_grid_offset":query_count,"render_grid_size":n},"primitives":[{
            "attributes":{"POSITION":pos,"NORMAL":nrm,"TEXCOORD_0":uv,"COLOR_0":col},
            "indices":idx,"material":0,"mode":4}]}),
            &t.name,
            Some(json!([{"name":t.layer_textures.first().cloned().unwrap_or_default()}])),
        )
    }

    pub fn layers_glb(&self, t: &Terrain, bf2: bool) -> Result<Option<Vec<u8>>> {
        let layers = t.layer_textures.len().min(16);
        if layers < 2 || t.patch_alpha.len() != t.patch_layer_masks.len() {
            return Ok(None);
        }
        let n = t.info.grid_size as usize;
        let (per_row, cells) = t
            .patch_grid()
            .ok_or_else(|| anyhow::anyhow!("missing layer patch grid"))?;
        if cells != t.patch_edge {
            return Ok(None);
        }
        let mut bin = Bin::default();
        let mut materials = Vec::new();
        let mut primitives = Vec::new();
        for k in 1..layers {
            let mut positions = Vec::new();
            let mut normals = Vec::new();
            let mut colours = Vec::new();
            let mut indices = Vec::new();
            let mut slots = HashMap::new();
            for face in &self.faces {
                let mask = t.patch_layer_masks[face.patch];
                let painted: Vec<_> = (0..16).filter(|b| mask >> b & 1 != 0).collect();
                let Some(rank) = painted.iter().position(|&layer| layer == k) else {
                    continue;
                };
                let whole = bf2 && rank + 1 == painted.len();
                let x0 = (face.patch % per_row) * cells;
                let z0 = (face.patch / per_row) * cells;
                let edge = cells + 1;
                let source_alpha = t
                    .patch_alpha
                    .get(face.patch)
                    .and_then(|a| a.get(rank * edge * edge..(rank + 1) * edge * edge));
                // A donor cutter patch may only store per-mesh alphas. The
                // base texture remains opaque; sample its own layer weights
                // in world X/Z wherever that donor mesh supplies them.
                let nibs = face.grid.map(|v| {
                    let (x, z) = (v as usize % n, v as usize / n);
                    source_alpha.map_or_else(
                        || patch_alpha(t, face.patch, rank, self.positions[v as usize]),
                        |a| Some(a[(z - z0) * edge + x - x0]),
                    )
                });
                let [Some(a), Some(b), Some(c)] = nibs else {
                    continue; // Never invent a painted layer without donor weights.
                };
                let nibs = [a, b, c];
                if (!whole && nibs.contains(&ABSENT))
                    || nibs.iter().all(|&a| LAYER_ALPHA[a as usize] == 0)
                {
                    continue;
                }
                let corner_alpha = nibs.map(|a| LAYER_ALPHA[a as usize] as f32 * 255.0 / 128.0);
                for v in face.vertices {
                    let p = self.positions[v as usize];
                    let weights = barycentric(p, face.grid.map(|i| self.positions[i as usize]));
                    let alpha = (0..3)
                        .map(|i| weights[i] * corner_alpha[i])
                        .sum::<f32>()
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    let next = positions.len() as u32;
                    let index = *slots.entry((v, alpha)).or_insert(next);
                    if index == next {
                        positions.push(p);
                        normals.push(self.normals[v as usize]);
                        let c = self.colours[v as usize];
                        colours.extend([c[0], c[1], c[2], alpha]);
                    }
                    indices.push(index);
                }
            }
            if indices.is_empty() {
                continue;
            }
            let uv: Vec<_> = positions
                .iter()
                .flat_map(|p| layer_uv(t, k, false, grid_coords(t, *p)))
                .collect();
            let (min, max) = min_max3(&positions);
            let pos = bin.f32s(&flat3(&positions), "VEC3", json!({"min":min,"max":max}));
            let nrm = bin.f32s(&flat3(&normals), "VEC3", json!({}));
            let uv = bin.f32s(&uv, "VEC2", json!({}));
            let col = bin.accessor(
                &colours,
                5121,
                positions.len(),
                "VEC4",
                json!({"normalized":true}),
            );
            let idx = bin.u32s(&indices, json!({}));
            primitives.push(
                json!({"attributes":{"POSITION":pos,"NORMAL":nrm,"TEXCOORD_0":uv,"COLOR_0":col},
                "indices":idx,"material":materials.len(),"mode":4}),
            );
            materials.push(json!({"name":t.layer_textures[k],"alphaMode":"BLEND"}));
        }
        if primitives.is_empty() {
            return Ok(None);
        }
        crate::gltf_bin::rigid_glb(
            bin,
            json!({"name":format!("{}_layers",t.name),"primitives":primitives}),
            &format!("{}_layers", t.name),
            Some(json!(materials)),
        )
        .map(Some)
    }
}

fn patch_alpha(t: &Terrain, patch: usize, rank: usize, p: [f32; 3]) -> Option<u8> {
    let pm = t.patch_meshes.get(patch).and_then(|p| p.as_ref())?;
    let count = pm.positions.len();
    let alpha = pm.alpha.get(rank * count..(rank + 1) * count)?;
    let positions = t.patch_mesh_world(patch);
    let mut best = (f32::INFINITY, 0xe);
    for tri in mungefs::model::unstitch_strips(&pm.strips) {
        if tri.iter().any(|&v| v as usize >= positions.len()) {
            continue;
        }
        let weights = barycentric(p, tri.map(|v| positions[v as usize]));
        if weights.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let outside: f32 = weights.iter().map(|v| (-v).max(0.0)).sum();
        if outside < best.0 {
            // Nearest donor alpha on a reshaped cut boundary; quantize back
            // to the donor nibble vocabulary before interpolating the face.
            let sum: f32 = weights.iter().map(|v| v.max(0.0)).sum();
            let value = (0..3)
                .map(|i| {
                    weights[i].max(0.0) * LAYER_ALPHA[alpha[tri[i] as usize] as usize] as f32 / sum
                })
                .sum::<f32>();
            let nib = (0..15)
                .min_by(|&a, &b| {
                    (LAYER_ALPHA[a] as f32 - value)
                        .abs()
                        .total_cmp(&(LAYER_ALPHA[b] as f32 - value).abs())
                })
                .unwrap();
            best = (outside, nib as u8);
        }
        if outside == 0.0 {
            break;
        }
    }
    best.0.is_finite().then_some(best.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::terrain::TerrainInfo;

    fn fixture() -> Terrain {
        Terrain {
            name: "ground".into(),
            info: TerrainInfo {
                grid_unit: 1.0,
                height_scale: 0.5,
                height_floor: -20.0,
                height_ceiling: 20.0,
                grid_size: 16,
            },
            raw: vec![0; 256],
            lumi: None,
            colr: None,
            layer_textures: vec!["donor_sand".into(), "donor_stone".into()],
            layer_scale: vec![8.0, 8.0],
            layer_axis: vec![0, 0],
            layer_rotation: vec![0.0, 0.0],
            patch_layer_masks: vec![3; 4],
            patch_meshes: vec![None; 4],
            patch_edge: 8,
            patch_alpha: vec![vec![0xe; 162]; 4],
            cutters: Vec::new(),
            lowres: None,
        }
    }

    fn box_cutter() -> Cutter {
        Cutter {
            sphere: [10.0, 0.0, 0.0, 0.0],
            planes: vec![
                [1.0, 0.0, 0.0, -0.6],
                [-1.0, 0.0, 0.0, -0.2],
                [0.0, 0.0, 1.0, -0.6],
                [0.0, 0.0, -1.0, -0.2],
                [0.0, 1.0, 0.0, -1.0],
                [0.0, -1.0, 0.0, -1.0],
            ],
        }
    }

    fn area(s: &Surface) -> f64 {
        s.faces
            .iter()
            .map(|f| {
                let [a, b, c] = f.vertices.map(|v| s.positions[v as usize].map(f64::from));
                ((b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn donor_drawing_retains_the_complete_original_query_grid() {
        let t = fixture();
        let heights = vec![3.0; 256];
        let s = Surface::build(&t, false, &heights).unwrap();
        let bytes = s.base_glb(&t).unwrap();
        let (doc, buffers, _) = gltf::import_slice(&bytes).unwrap();
        let primitive = doc.meshes().next().unwrap().primitives().next().unwrap();
        let reader = primitive.reader(|b| buffers.get(b.index()).map(|v| v.0.as_slice()));
        let positions: Vec<_> = reader.read_positions().unwrap().collect();
        assert!(positions[..256].iter().all(|p| p[1] == 0.0));
        assert!(positions[256..].iter().all(|p| p[1] == 3.0));
        assert!(reader.read_indices().unwrap().into_u32().all(|v| v >= 256));
        assert_eq!(t.raw, vec![0; 256]);
    }

    #[test]
    fn cutter_rebuild_removes_only_its_open_interior_and_retains_layers() {
        let mut t = fixture();
        t.cutters.push(box_cutter());
        let s = Surface::build(&t, false, &vec![0.0; 256]).unwrap();
        assert!((area(&s) - (225.0 - 0.64)).abs() < 0.00001);
        for face in &s.faces {
            let tri = face.vertices.map(|v| s.positions[v as usize]);
            let centre: [f64; 3] =
                std::array::from_fn(|i| tri.iter().map(|v| v[i] as f64).sum::<f64>() / 3.0);
            assert!(t.cutters[0]
                .planes
                .iter()
                .any(|p| distance(p, centre) >= -0.000001));
        }
        let bytes = s.layers_glb(&t, false).unwrap().unwrap();
        let (doc, _, _) = gltf::import_slice(&bytes).unwrap();
        assert_eq!(doc.materials().next().unwrap().name(), Some("donor_stone"));
    }

    #[test]
    fn absent_donor_layer_weights_never_become_opaque_paint() {
        let mut t = fixture();
        t.patch_alpha = vec![Vec::new(); 4];
        let s = Surface::build(&t, false, &vec![0.0; 256]).unwrap();
        assert!(s.layers_glb(&t, false).unwrap().is_none());
        t.patch_alpha.clear();
        assert!(s.layers_glb(&t, false).unwrap().is_none());
    }

    #[test]
    fn coplanar_cutter_boundaries_do_not_duplicate_faces() {
        let mut t = fixture();
        let mut cutter = box_cutter();
        cutter.planes[4][3] = 0.0;
        t.cutters.push(cutter);
        let s = Surface::build(&t, false, &vec![0.0; 256]).unwrap();
        assert_eq!(area(&s), 225.0);
    }

    #[test]
    fn both_alternating_rows_use_the_canonical_diagonal() {
        let t = fixture();
        let mut heights = vec![0.0; 256];
        heights[17] = 2.0;
        heights[33] = 4.0;
        let s = Surface::build(&t, false, &heights).unwrap();
        assert_eq!(s.faces[0].grid, [0, 16, 17]);
        assert_eq!(s.faces[1].grid, [0, 17, 1]);
        assert_eq!(s.faces[30].grid, [16, 32, 17]);
        assert_eq!(s.faces[31].grid, [17, 32, 33]);
        assert_eq!(s.positions[17][1], 2.0);
        assert_eq!(s.positions[33][1], 4.0);
    }
}
