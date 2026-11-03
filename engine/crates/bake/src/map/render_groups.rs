//! Return displaced vertex groups of fingerprinted donor models to their place.
//!
//! These donor models keep PS2's segments, vertex counts and triangle lists,
//! but one contiguous group of vertices sits rigidly moved: drawn metres from
//! the unchanged PS2 collision. A rigid map of that group (measured once
//! against PS2 drawing, residual below a millimetre) puts it back. The table
//! holds index ranges, a rotation and a translation; no mesh. Every other
//! vertex, the UVs, colours, headers and collision are unchanged, and the
//! segment/vertex counts are guarded so a different asset revision is refused.
//! Pure PS2 never selects a recipe.
//!
//! A donor shadow volume was authored around the displaced art (PS2 casts no
//! shadow from these models), so its vertices that sit on a moved group move
//! with it: otherwise the window's shadow stays 1.1 m in front of the window.

use anyhow::{ensure, Result};
use glam::{Mat3, Vec3};
use mungefs::model::Model;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// A shadow vertex whose nearest drawn vertex is in a moved group, and no
/// farther from it than this, follows that group. Measured on the PC
/// cathedral window: its shadow vertices sit 0.048-0.067 m from the drawn
/// vertices they outline; the Hoth chamber's coarse hull is 2.6 m or more away.
const SHADOW_REACH_M: f32 = 0.25;

struct Recipe {
    name: &'static str,
    input: &'static str,
    /// Vertex count of every segment, in order.
    vertices: &'static [usize],
    /// (segment, first vertex, last vertex), inclusive.
    groups: &'static [(usize, usize, usize)],
    columns: [[f32; 3]; 3],
    translation: [f32; 3],
}

// Measured with scripts/render_alignment_sweep.py and the claude-align
// group fits: max vertex residual after the map, against PS2 drawing.
const RECIPES: &[Recipe] = &[
    // BF1 Hoth: a rock group of the small natural chamber is turned by about
    // 18.5 degrees and dropped 3.42 m in both donors. Residual 0.44 mm Xbox.
    Recipe {
        name: "hoth_bldg_small_chamber_natural",
        input: "df3f878602565079e5d08232cf4ebf5611cb3e6f1b810cb1b19c7e8ae346c071",
        vertices: &[432, 24, 88, 263],
        groups: &[(0, 0, 143), (1, 0, 7)],
        columns: [
            [0.948_293, -0.000_112, -0.317_14],
            [0.000_067, 1.000_056, -0.000_104],
            [0.317_268, 0.000_013, 0.948_392],
        ],
        translation: [3.129_2, 3.422_348, 12.553_521],
    },
    // The same group in the PC BF1 donor. Residual 0.26 mm.
    Recipe {
        name: "hoth_bldg_small_chamber_natural",
        input: "97144a270cb4b3688e3bc9a4ae56864c9fc9a268c0a3d8726669a4c5fddc3ea2",
        vertices: &[432, 24, 88, 263],
        groups: &[(0, 0, 143), (1, 0, 7)],
        columns: [
            [0.948_449, -0.000_066, -0.317_263],
            [0.000_006, 1.000_013, 0.],
            [0.317_322, -0.000_042, 0.948_271],
        ],
        translation: [3.122_604, 3.420_142, 12.558_4],
    },
    // BF1 Rhen Var Citadel, PC: the cathedral window's frame and tracery
    // stand 1.104 m proud of the wall they belong to. Residual 0.11 mm.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_window",
        input: "abfdba0087089ba956fe20b204fbb7dda97bca9a75be4b9ff664e21674729716",
        vertices: &[46, 108, 12],
        groups: &[(1, 28, 107), (2, 4, 11)],
        columns: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        translation: [0., 0., -1.104_101],
    },
];

pub(super) struct Groups(HashMap<String, &'static Recipe>);

impl Groups {
    pub(super) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = HashMap::new();
        if let Some(original) = original {
            for chunk in mungefs::content_chunks(mungefs::open(original)?)? {
                if chunk.tag != *b"modl" {
                    continue;
                }
                let Some(name) = chunk.child_string(b"NAME") else {
                    continue;
                };
                let hash = format!("{:x}", Sha256::digest(chunk.payload));
                if let Some(recipe) = RECIPES
                    .iter()
                    .find(|r| r.name.eq_ignore_ascii_case(&name) && r.input == hash)
                {
                    selected.insert(name.to_ascii_lowercase(), recipe);
                }
            }
        }
        Ok(Self(selected))
    }

    pub(super) fn apply(&self, model: &mut Model) -> Result<()> {
        let Some(recipe) = self.0.get(&model.name.to_ascii_lowercase()) else {
            return Ok(());
        };
        restore(model, recipe)?;
        eprintln!("render groups: {} from {}", model.name, recipe.input);
        Ok(())
    }

    /// Move the shadow vertices that outline a moved group, from the model as
    /// stored (before `apply`). Call right after the shadow file is written.
    pub(super) fn shadow(&self, model: &Model, out: &Path) -> Result<()> {
        let Some(recipe) = self.0.get(&model.name.to_ascii_lowercase()) else {
            return Ok(());
        };
        let path = out.join("shadows").join(format!("{}.ron", model.name));
        let mut pieces: Vec<manifest::shadow::ShadowPiece> =
            ron::from_str(&std::fs::read_to_string(&path)?)?;
        let moved = follow(
            model,
            recipe,
            pieces.iter_mut().flat_map(|p| p.positions.iter_mut()),
        )?;
        crate::write_ron(&path, &pieces)?;
        eprintln!("render groups: {} shadow, {moved} vertices", model.name);
        Ok(())
    }
}

fn check(model: &Model, recipe: &Recipe) -> Result<()> {
    ensure!(
        !model.bone_space,
        "render groups: {} needs a posed model",
        model.name
    );
    ensure!(
        model.segments.len() == recipe.vertices.len()
            && model
                .segments
                .iter()
                .zip(recipe.vertices)
                .all(|(s, &n)| s.positions.len() == n && s.normals.len() == n),
        "render groups: {}: changed donor segmentation",
        model.name
    );
    Ok(())
}

fn moved(recipe: &Recipe, segment: usize, vertex: usize) -> bool {
    recipe
        .groups
        .iter()
        .any(|&(s, first, last)| s == segment && (first..=last).contains(&vertex))
}

/// Apply the recipe's map to every shadow vertex whose nearest stored vertex
/// is moved and within `SHADOW_REACH_M`; returns how many moved.
fn follow<'a>(
    model: &Model,
    recipe: &Recipe,
    shadow: impl Iterator<Item = &'a mut [f32; 3]>,
) -> Result<usize> {
    check(model, recipe)?;
    let matrix = Mat3::from_cols_array_2d(&recipe.columns);
    let translation = Vec3::from(recipe.translation);
    let mut count = 0;
    for p in shadow {
        let here = Vec3::from(*p);
        let nearest = model
            .segments
            .iter()
            .enumerate()
            .flat_map(|(s, seg)| {
                seg.positions
                    .iter()
                    .enumerate()
                    .map(move |(i, q)| (here.distance(Vec3::from(*q)), s, i))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, s, i)) = nearest {
            if d <= SHADOW_REACH_M && moved(recipe, s, i) {
                *p = (matrix * here + translation).to_array();
                count += 1;
            }
        }
    }
    Ok(count)
}

fn restore(model: &mut Model, recipe: &Recipe) -> Result<()> {
    check(model, recipe)?;
    let matrix = Mat3::from_cols_array_2d(&recipe.columns);
    let normal = matrix.inverse().transpose();
    let translation = Vec3::from(recipe.translation);
    for &(segment, first, last) in recipe.groups {
        let s = &mut model.segments[segment];
        ensure!(
            first <= last && last < s.positions.len(),
            "render groups: bad range"
        );
        for i in first..=last {
            s.positions[i] = (matrix * Vec3::from(s.positions[i]) + translation).to_array();
            s.normals[i] = (normal * Vec3::from(s.normals[i]))
                .normalize_or_zero()
                .to_array();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType, Segment};

    fn segment(positions: Vec<[f32; 3]>) -> Segment {
        let n = positions.len();
        Segment {
            vertex_count: n as u32,
            index_count: 0,
            bf2_material: false,
            pc_geometry: true,
            material_flags: 0,
            render_type: RenderType::Bf1("Normal".into()),
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0; 2],
            textures: vec![],
            bone: None,
            group: None,
            bmap: vec![],
            bind: vec![],
            wght: vec![],
            positions,
            normals: vec![[0., 0., 1.]; n],
            uvs: vec![[0., 0.]; n],
            colors: None,
            triangles: vec![],
        }
    }

    // A wall piece at z = -5 that stays, and a frame (segment 1, vertices 1-2)
    // standing 1 m proud of it at z = -4, which the recipe moves back by 1 m.
    static FRAME: Recipe = Recipe {
        name: "frame",
        input: "",
        vertices: &[2, 3],
        groups: &[(1, 1, 2)],
        columns: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        translation: [0., 0., -1.],
    };

    fn frame() -> Model {
        Model {
            name: "frame".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [0.; 3],
                max: [0.; 3],
            },
            vis_box: Aabb {
                min: [0.; 3],
                max: [0.; 3],
            },
            face_count: 0,
            sphere: None,
            segments: vec![
                segment(vec![[0., 0., -5.], [1., 0., -5.]]),
                segment(vec![[0., 1., -5.], [0., 9., -4.], [1., 9., -4.]]),
            ],
        }
    }

    #[test]
    fn shadow_outlining_a_moved_group_follows_it_and_the_rest_stays() {
        let model = frame();
        let mut shadow = [
            [0.05, 9., -4.05], // on the frame: follows it to z = -5.05
            [1., 9.04, -4.],   // on the frame
            [0., 0., -5.05],   // on the wall: stays
            [0.04, 1., -5.],   // on segment 1's kept vertex: stays
            [0.5, 9., -2.],    // 2 m from anything drawn: stays
        ];
        let before = shadow;
        assert_eq!(follow(&model, &FRAME, shadow.iter_mut()).unwrap(), 2);
        assert_eq!(shadow[0], [0.05, 9., -5.05]);
        assert_eq!(shadow[1], [1., 9.04, -5.]);
        assert_eq!(shadow[2..], before[2..]);
        // The drawn frame moves the same way, so shadow and art meet again.
        let mut drawn = frame();
        restore(&mut drawn, &FRAME).unwrap();
        assert_eq!(drawn.segments[1].positions[1], [0., 9., -5.]);
        assert_eq!(drawn.segments[1].positions[0], [0., 1., -5.]);
    }

    #[test]
    fn shadow_follow_refuses_a_changed_donor() {
        let mut model = frame();
        model.segments[1].positions.pop();
        model.segments[1].normals.pop();
        let mut shadow = [[0., 9., -4.]];
        assert!(follow(&model, &FRAME, shadow.iter_mut()).is_err());
    }

    #[test]
    fn rotation_columns_are_orthonormal() {
        for recipe in RECIPES {
            let m = Mat3::from_cols_array_2d(&recipe.columns);
            assert!((m.determinant() - 1.).abs() < 1e-3, "{}", recipe.name);
            assert!(
                (m * m.transpose() - Mat3::IDENTITY).abs_diff_eq(Mat3::ZERO, 1e-3),
                "{}",
                recipe.name
            );
            assert!(recipe
                .groups
                .iter()
                .all(|&(s, a, b)| s < recipe.vertices.len() && a <= b && b < recipe.vertices[s]));
        }
    }
}
