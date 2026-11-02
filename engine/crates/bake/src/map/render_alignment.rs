//! Fingerprinted instructions for aligning donor artwork after its bind pose.
//! Only drawing changes: model headers, placements and collision remain intact.
//! The table contains transforms and a face construction from donor vertices,
//! never a replacement mesh. PS2 bakes do not enter this path.

use anyhow::{anyhow, ensure, Result};
use glam::{Mat3, Vec3};
use mungefs::model::Model;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

struct Recipe {
    name: &'static str,
    input: &'static str,
    columns: [[f32; 3]; 3],
    translation: [f32; 3],
    bottom_cap: bool,
}

const IDENTITY: [[f32; 3]; 3] = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];

// Measured against PS2 drawing and the same canonical collision. Rhen Var's
// donor column has four wall quads starting at y=8 and a top at y=32. Stretch
// the existing walls to y=0, preserving the top, then close their lower ring.
// A placement translation cannot preserve both endpoints; the whole-model
// stretch suffices, so no group/vertex deformation or stand-in is necessary.
const RECIPES: &[Recipe] = &[
    Recipe {
        name: "rhn_bldg_lego_2",
        input: "1f250e787c5579ed80f60d879bf613991cca5156f9c59370f8666e5c936eefe7",
        columns: [[1., 0., 0.], [0., 4. / 3., 0.], [0., 0., 1.]],
        translation: [0., -32. / 3., 0.],
        bottom_cap: true,
    },
    Recipe {
        name: "rhn_bldg_lego_2",
        input: "37984d72f41818a5566b5b3853cb210b2d72326153241f22aa1130c9beba5f18",
        columns: [[1., 0., 0.], [0., 4. / 3., 0.], [0., 0., 1.]],
        translation: [0., -32. / 3., 0.],
        bottom_cap: true,
    },
    Recipe {
        name: "geo_sky_spire",
        input: "1a548ee03e9dbe489df47b7d71ed152d18b978ff3bb88ef696b6bc55bfea7fbb",
        columns: IDENTITY,
        translation: [0., 0.40095532, 0.],
        bottom_cap: false,
    },
];

pub(super) struct Alignment(HashMap<String, &'static Recipe>);

impl Alignment {
    pub(super) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = HashMap::new();
        if let Some(bytes) = original {
            for chunk in mungefs::content_chunks(mungefs::open(bytes)?)? {
                if &chunk.tag != b"modl" {
                    continue;
                }
                let Some(name) = chunk.child_string(b"NAME") else {
                    continue;
                };
                let candidates: Vec<_> = RECIPES
                    .iter()
                    .filter(|row| row.name.eq_ignore_ascii_case(&name))
                    .collect();
                if candidates.is_empty() {
                    continue;
                }
                let hash = format!("{:x}", Sha256::digest(chunk.payload));
                if let Some(row) = candidates.into_iter().find(|row| row.input == hash) {
                    selected.entry(name.to_ascii_lowercase()).or_insert(row);
                }
            }
        }
        Ok(Self(selected))
    }

    pub(super) fn shadow(&self, name: &str, out: &Path) -> Result<()> {
        let Some(recipe) = self.0.get(&name.to_ascii_lowercase()) else {
            return Ok(());
        };
        let path = out.join("shadows").join(format!("{name}.ron"));
        let mut pieces: Vec<manifest::shadow::ShadowPiece> =
            ron::from_str(&std::fs::read_to_string(&path)?)?;
        let matrix = Mat3::from_cols_array_2d(&recipe.columns);
        for piece in &mut pieces {
            for p in &mut piece.positions {
                *p = (matrix * Vec3::from(*p) + Vec3::from(recipe.translation)).to_array();
            }
        }
        crate::write_ron(&path, &pieces)
    }

    pub(super) fn apply(&self, model: &mut Model) -> Result<()> {
        let Some(recipe) = self.0.get(&model.name.to_ascii_lowercase()) else {
            return Ok(());
        };
        ensure!(!model.bone_space, "{} needs a posed model", model.name);
        if recipe.bottom_cap {
            ensure!(
                model.segments.len() == 2
                    && matches!(model.segments[0].positions.len(), 16 | 64)
                    && model.segments[1].positions.len() == 4,
                "{}: changed donor column segmentation",
                model.name
            );
        }
        let matrix = Mat3::from_cols_array_2d(&recipe.columns);
        let normal = matrix.inverse().transpose();
        for segment in &mut model.segments {
            for position in &mut segment.positions {
                *position =
                    (matrix * Vec3::from(*position) + Vec3::from(recipe.translation)).to_array();
            }
            for n in &mut segment.normals {
                *n = (normal * Vec3::from(*n)).normalize_or_zero().to_array();
            }
        }
        if recipe.bottom_cap {
            cap_column(model)?;
        }
        eprintln!("render alignment: {} from {}", model.name, recipe.input);
        Ok(())
    }
}

fn cap_column(model: &mut Model) -> Result<()> {
    let segment = &mut model.segments[0];
    let mut ring: Vec<usize> = Vec::new();
    for (i, p) in segment.positions.iter().enumerate() {
        if p[1].abs() < 0.00001 && !ring.iter().any(|&j| segment.positions[j] == *p) {
            ring.push(i);
        }
    }
    ensure!(ring.len() == 4, "{}: changed lower ring", model.name);
    // Counterclockwise in X/Z points down in this Y-up coordinate system.
    ring.sort_by(|&a, &b| {
        let a = segment.positions[a];
        let b = segment.positions[b];
        a[2].atan2(a[0]).total_cmp(&b[2].atan2(b[0]))
    });
    let base = u32::try_from(segment.positions.len()).map_err(|e| anyhow!(e))?;
    for &i in &ring {
        segment.positions.push(segment.positions[i]);
        segment.normals.push([0., -1., 0.]);
        // Reuse this donor wall's UVs and colours, including its own texture.
        segment.uvs.push(segment.uvs[i]);
        if let Some(colors) = &mut segment.colors {
            colors.push(colors[i]);
        }
    }
    segment
        .triangles
        .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
    segment.vertex_count = segment.positions.len() as u32;
    segment.index_count = (segment.triangles.len() * 3) as u32;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType, Segment};

    fn column() -> Model {
        let mut wall = Segment {
            vertex_count: 16,
            index_count: 24,
            bf2_material: false,
            pc_geometry: false,
            material_flags: 1,
            render_type: RenderType::Bf1("Normal".into()),
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0; 2],
            textures: vec![(0, "donor_brick".into())],
            bone: Some("root".into()),
            group: None,
            bmap: vec![],
            bind: vec![],
            wght: vec![],
            positions: vec![],
            normals: vec![[1., 0., 0.]; 16],
            uvs: vec![[0., 1.]; 16],
            colors: Some(vec![[34, 56, 78, 128]; 16]),
            triangles: vec![],
        };
        for (i, (x, z)) in [(-2., -2.), (2., -2.), (2., 2.), (-2., 2.)]
            .into_iter()
            .enumerate()
        {
            let (u, v) = [(2., -2.), (2., 2.), (-2., 2.), (-2., -2.)][i];
            wall.positions
                .extend([[x, 8., z], [u, 8., v], [u, 32., v], [x, 32., z]]);
            let b = (i * 4) as u32;
            wall.triangles
                .extend([[b, b + 1, b + 2], [b, b + 2, b + 3]]);
        }
        let mut top = wall.clone();
        top.positions = vec![
            [-2., 32., -2.],
            [2., 32., -2.],
            [2., 32., 2.],
            [-2., 32., 2.],
        ];
        top.normals.truncate(4);
        top.uvs.truncate(4);
        top.colors.as_mut().unwrap().truncate(4);
        top.triangles = vec![[0, 2, 1], [0, 3, 2]];
        top.vertex_count = 4;
        top.index_count = 6;
        Model {
            name: "rhn_bldg_lego_2".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [-2., 8., -2.],
                max: [2., 32., 2.],
            },
            vis_box: Aabb {
                min: [-2., 0., -2.],
                max: [2., 32., 2.],
            },
            face_count: 23,
            sphere: Some([1., 2., 3., 40.]),
            segments: vec![wall, top],
        }
    }

    #[test]
    fn reference_input_never_selects_a_recipe() {
        assert!(Alignment::of(None).unwrap().0.is_empty());
    }

    #[test]
    fn column_repair_closes_the_bottom_and_keeps_materials_and_gameplay_header() {
        let mut model = column();
        let header = (
            model.vertex_box.min,
            model.vis_box.max,
            model.sphere,
            model.face_count,
        );
        let uv = model.segments[0].uvs.clone();
        let top = model.segments[1].positions.clone();
        let alignment = Alignment(HashMap::from([(model.name.clone(), &RECIPES[0])]));
        alignment.apply(&mut model).unwrap();
        assert_eq!(model.segments[1].positions, top);
        let wall = &model.segments[0];
        assert_eq!(&wall.uvs[..16], uv);
        assert_eq!(wall.textures[0].1, "donor_brick");
        assert_eq!(wall.triangles.len(), 10);
        for triangle in &wall.triangles[8..] {
            let [a, b, c] = triangle.map(|i| Vec3::from(wall.positions[i as usize]));
            assert!(a.y.abs() < 0.00001 && b.y.abs() < 0.00001 && c.y.abs() < 0.00001);
            assert!((b - a).cross(c - a).y < 0.);
        }
        assert_eq!(
            (
                model.vertex_box.min,
                model.vis_box.max,
                model.sphere,
                model.face_count
            ),
            header
        );
        model.segments[0].positions.pop();
        assert!(alignment.apply(&mut model).is_err());
    }
}
