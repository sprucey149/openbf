//! Terrain draw: the heightfield split into square chunks with their bounds,
//! and the layer uploads. Cutter patches use the bake's pre-clipped meshes
//! (`analysis/re-terrain-cutters.md` §0.3).

use super::*;

/// Cells along one edge of a terrain draw chunk. See
/// [`upload_terrain_chunks`]; 32 is exactly 4x4 of the disc's own 8-cell
/// `PCHS` patches, and gives an 8x8 grid of 64 chunks on a 256-cell map.
/// PROVISIONAL as a tuning choice — it is the granularity/draw-count
/// trade-off, nothing decoded.
pub(super) const TERRAIN_CHUNK_CELLS: usize = 32;

/// Split the terrain heightfield into square draw chunks.
///
/// As one mesh it was 130k triangles with a map-wide AABB, so frustum culling
/// could never touch it and it was the measured residual on every big map
/// (PLAN B4). Chunking gives each piece its own bounds.
///
/// **This happens at UPLOAD, not in the bake, and that is deliberate.**
/// `Scene::heightfield` rebuilds the sim's ground-height query out of
/// `terrain.positions`, assuming they are a row-major `n x n` grid in the
/// order the bake emits them. Chunking the GLB silently breaks that: the
/// vertex count stops being `n²` (chunk seams duplicate a row), and on BF1
/// Hoth 69,169 happens to be a perfect square, so the guard in `heightfield`
/// passes and the sim gets a garbage ground plane — soldiers and the camera
/// end up floored against nonsense, and `--sim-hash` moves. Splitting here
/// leaves the baked data, the manifest and every sim path untouched by
/// construction.
///
/// Falls back to the unsplit mesh whenever the terrain is not the grid this
/// assumes (which is the same contract `Scene::heightfield` checks).
pub(super) fn upload_terrain_chunks<'a>(
    r: &mut Renderer,
    cpu: &'a CpuMesh,
) -> Result<Vec<SubmeshPart<'a>>> {
    let Some(chunks) = terrain_chunk_indices(cpu) else {
        return upload_submeshes(r, cpu);
    };
    let mat = cpu.submeshes[0].2.as_str();
    let mat = (!mat.is_empty()).then_some(mat);
    // Every chunk is one sheet split for culling, so they all share the one
    // submesh's alpha mode.
    let layer = cpu.submeshes[0].3;
    // ...and its raw MTRL word, which no terrain draw reads (the sky pass is
    // the only consumer) but which travels with the tuple.
    let flags = cpu.submeshes[0].4;
    let mut out = Vec::new();
    for indices in chunks.iter().filter(|c| !c.is_empty()) {
        // Compacting is what gives the chunk its own vertex range and
        // therefore its own AABB — the whole point of the split.
        let sub = compact_submesh(cpu.mesh_data(indices));
        let mesh = r
            .add_mesh(&sub.mesh_data())
            .map_err(|e| anyhow!("terrain chunk upload: {e}"))?;
        out.push((mesh, mat, layer, flags));
    }
    if out.is_empty() {
        return upload_submeshes(r, cpu);
    }
    Ok(out)
}

/// Upload the terrain's LAYER meshes ([`crate::assets::Scene::terrain_layers`])
/// split into the base terrain's draw chunks: LAYER-major, so every chunk of
/// layer k gets a lower mesh handle than any chunk of layer k+1 and the
/// `TerrainLayer` pass composites them in ascending LTEX order. A triangle
/// goes to the chunk of its minimum cell, as the base's do. Empty when the
/// base terrain is not the grid this assumes.
pub(super) fn upload_terrain_layer_chunks<'a>(
    r: &mut Renderer,
    base: &CpuMesh,
    layers: &'a CpuMesh,
) -> Result<Vec<(render::MeshHandle, &'a str)>> {
    let Some(n) = crate::assets::terrain_grid_size(&base.positions) else {
        return Ok(Vec::new());
    };
    let p0 = base.positions[0];
    let unit = base.positions[1][0] - p0[0];
    if !(unit.is_finite() && unit > 0.0) {
        return Ok(Vec::new());
    }
    let cell = |w: f32, o: f32| (((w - o) / unit) as isize).clamp(0, n as isize - 1) as usize;
    let per_row = n.div_ceil(TERRAIN_CHUNK_CELLS).max(1);
    let mut out = Vec::new();
    for (start, count, mat, _, _) in &layers.submeshes {
        let idx = &layers.indices[*start as usize..(*start + *count) as usize];
        let mut chunks = vec![Vec::new(); per_row * per_row];
        for tri in idx.as_chunks::<3>().0 {
            if tri.iter().any(|&i| i as usize >= layers.positions.len()) {
                continue;
            }
            let (x, z) = tri.iter().fold((usize::MAX, usize::MAX), |a, &i| {
                let p = layers.positions[i as usize];
                (a.0.min(cell(p[0], p0[0])), a.1.min(cell(p[2], p0[2])))
            });
            let (cx, cz) = (
                (x / TERRAIN_CHUNK_CELLS).min(per_row - 1),
                (z / TERRAIN_CHUNK_CELLS).min(per_row - 1),
            );
            chunks[cz * per_row + cx].extend_from_slice(tri);
        }
        for indices in chunks.iter().filter(|c| !c.is_empty()) {
            let sub = compact_submesh(layers.mesh_data(indices));
            let mesh = r
                .add_mesh(&sub.mesh_data())
                .map_err(|e| anyhow!("terrain layer upload: {e}"))?;
            out.push((mesh, mat.as_str()));
        }
    }
    Ok(out)
}

/// Deal the terrain's BAKED triangles into square chunks, in chunk order
/// (z-major), each keeping the baked index order. `None` when the mesh is not
/// a single-material heightfield mesh.
///
/// It reads the baked index buffer rather than re-deriving the grid's quads,
/// because the bake no longer emits one quad per cell everywhere: a render
/// patch a terrain cutter reaches into ships a PRE-CLIPPED replacement mesh
/// that the bake substitutes for those 64 quads (`bake::terrain_glb`,
/// analysis/re-terrain-cutters.md §0.3). Walking the grid would silently draw
/// the snow back inside Hoth's outpost rooms.
///
/// A triangle belongs to the chunk of its MINIMUM cell, which puts both
/// triangles of a grid quad in the chunk that owns the quad's lower-left
/// corner — the same assignment the grid walk made, so the chunking of a
/// terrain with no replacement mesh is unchanged down to the index order.
pub(super) fn terrain_chunk_indices(cpu: &CpuMesh) -> Option<Vec<Vec<u32>>> {
    if cpu.submeshes.len() != 1 {
        return None;
    }
    let n = crate::assets::terrain_grid_size(&cpu.positions)?;
    let p0 = cpu.positions[0];
    let unit = cpu.positions[1][0] - p0[0];
    if !(unit.is_finite() && unit > 0.0) {
        return None;
    }
    // A grid vertex IS its cell (row-major, `z * n + x`); a replacement
    // vertex is placed by its world position, which lies inside the patch it
    // replaces.
    let cell = |v: u32| -> (usize, usize) {
        let v = v as usize;
        if v < n * n {
            (v % n, v / n)
        } else {
            let p = cpu.positions[v];
            let f = |w: f32, o: f32| (((w - o) / unit) as isize).clamp(0, n as isize - 1) as usize;
            (f(p[0], p0[0]), f(p[2], p0[2]))
        }
    };
    let per_row = n.div_ceil(TERRAIN_CHUNK_CELLS).max(1);
    let mut chunks = vec![Vec::new(); per_row * per_row];
    for tri in cpu.indices.as_chunks::<3>().0 {
        if tri.iter().any(|&i| i as usize >= cpu.positions.len()) {
            return None;
        }
        let cells = tri.iter().map(|&i| cell(i));
        let (x, z) = cells.fold((usize::MAX, usize::MAX), |a, c| {
            (a.0.min(c.0), a.1.min(c.1))
        });
        let (cx, cz) = (
            (x / TERRAIN_CHUNK_CELLS).min(per_row - 1),
            (z / TERRAIN_CHUNK_CELLS).min(per_row - 1),
        );
        chunks[cz * per_row + cx].extend_from_slice(tri);
    }
    Some(chunks)
}
