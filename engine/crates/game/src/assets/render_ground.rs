//! Corrected ground used only to anchor drawn foliage, grass and leaves.
//! The simulation still reads the untouched leading grid in Scene::heightfield.

use super::{CpuMesh, Scene};
use core::heightfield::Heightfield;

/// Decode and validate the explicit render grid, never infer an appended grid
/// from cutter vertices. All coordinates and heights must be finite and every
/// row/column must be uniformly spaced in the stated frame.
pub(super) fn from_mesh(mesh: &CpuMesh) -> Option<Heightfield> {
    let (offset, size) = mesh.render_grid?;
    if size < 2 || size > u32::MAX as usize {
        return None;
    }
    let count = size.checked_mul(size)?;
    let end = offset.checked_add(count)?;
    let positions = mesh.positions.get(offset..end)?;
    let first = positions[0];
    let unit = positions[1][0] - first[0];
    if !(unit.is_finite() && unit > 0.0) {
        return None;
    }
    for (index, point) in positions.iter().enumerate() {
        if !point.iter().all(|value| value.is_finite())
            || point[0] != first[0] + (index % size) as f32 * unit
            || point[2] != first[2] + (index / size) as f32 * unit
        {
            return None;
        }
    }
    Some(Heightfield {
        size: size as u32,
        unit,
        offset_x: -first[0],
        offset_z: -first[2],
        heights: positions.iter().map(|point| point[1]).collect(),
    })
}

impl Scene {
    /// Full corrected drawn terrain for presentation attachments. PS2 and older
    /// bakes retain exactly their previous height query. Never install this
    /// field into the simulation: its interpolation frame can be larger.
    pub fn render_heightfield(&self) -> Option<Heightfield> {
        match self.terrain.as_ref()?.render_grid {
            Some(_) => from_mesh(self.terrain.as_ref()?),
            None => self.heightfield(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh() -> CpuMesh {
        let mut positions = vec![
            [0.0, 99.0, 0.0],
            [1.0, 99.0, 0.0],
            [0.0, 99.0, 1.0],
            [1.0, 99.0, 1.0],
        ];
        positions.extend([
            [0.0, 2.0, 0.0],
            [1.0, 3.0, 0.0],
            [0.0, 4.0, 1.0],
            [1.0, 5.0, 1.0],
        ]);
        // A cutter vertex is not part of the render height query.
        positions.push([99.0, -50.0, 99.0]);
        CpuMesh {
            positions,
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            submeshes: Vec::new(),
            spec: Vec::new(),
            light: Vec::new(),
            scroll: Vec::new(),
            sphere: None,
            vis_top: None,
            render_grid: Some((4, 2)),
        }
    }

    #[test]
    fn attachments_follow_only_the_explicit_render_grid() {
        let mesh = mesh();
        let ground = from_mesh(&mesh).unwrap();
        assert_eq!(ground.heights, [2.0, 3.0, 4.0, 5.0]);
        assert_eq!(ground.height_at(0.5, 0.5), 3.5);
        assert!(mesh.positions[..4].iter().all(|p| p[1] == 99.0));
        let mut scene = Scene {
            dir: std::path::PathBuf::new(),
            manifest: ron::from_str("(map:\"t\",source_game:\"bf1\",models:[],textures:[],classes:[],geometries:[],regions:[],instances:[],layers:[])").unwrap(),
            meshes: std::collections::HashMap::new(), terrain: Some(mesh), water: None,
            lowres: None, terrain_layers: None, textures: std::collections::HashMap::new(),
            collision: std::collections::HashMap::new(), cutters: Vec::new(),
        };
        assert_eq!(scene.heightfield().unwrap().height_at(0.5, 0.5), 99.0);
        assert_eq!(scene.render_heightfield().unwrap().height_at(0.5, 0.5), 3.5);
        scene.terrain.as_mut().unwrap().render_grid = None;
        assert_eq!(
            scene.render_heightfield().unwrap().height_at(0.5, 0.5),
            99.0
        );
    }

    #[test]
    fn rejects_partial_overflowed_and_malformed_grids() {
        let mut mesh = mesh();
        for grid in [(4, 0), (4, 3), (usize::MAX, 2), (4, usize::MAX)] {
            mesh.render_grid = Some(grid);
            assert!(from_mesh(&mesh).is_none());
        }
        mesh.render_grid = Some((4, 2));
        mesh.positions[7][2] = 1.1;
        assert!(from_mesh(&mesh).is_none());
        mesh.positions[7][2] = 1.0;
        mesh.positions[7][1] = f32::NAN;
        assert!(from_mesh(&mesh).is_none());
    }
}
