//! Sparse fitted displacements of fingerprinted donor vertices, plus removal of
//! donor faces that draw where neither the canonical collision nor the PS2 art is.
//!
//! The table holds, per donor model fingerprint, only (primitive, vertex, delta)
//! moves and (primitive, face) removals; no mesh, UV, colour or texture is
//! stored. Moves take existing donor vertices toward canonical collision points
//! or the PS2 drawing. Per-recipe comments count those target origins, including
//! the PS2-art fallback. The offline fitter scores both directions against PS2's
//! own residual; composed-scene dense checks reject candidates that increase
//! positive excess by more than the fit's 3 mm allowance. This is measured
//! at-rest coverage, not a continuous or all-pose certificate.
//!
//! Collision, placements, model headers and every PS2 bake are untouched: a PS2
//! input never matches a donor fingerprint. Moves run after the earlier
//! hand-written repairs and model recovery, whose final outputs the fitter
//! measured. Primitive N is the Nth segment that has triangles, which is
//! the GLB primitive order `crate::glb` writes.

use anyhow::{ensure, Result};
use glam::Vec3;
use mungefs::model::Model;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(crate) struct Recipe {
    pub name: &'static str,
    /// SHA-256 of the donor's raw `modl` chunk payload.
    pub input: &'static str,
    /// (vertices, faces) of each drawn primitive, as fitted.
    pub shapes: &'static [(u32, u32)],
    /// (primitive, vertex, displacement in model space).
    pub moves: &'static [(u8, u16, [f32; 3])],
    /// (primitive, face index before any removal).
    pub removals: &'static [(u8, u16)],
}

include!("render_offsets_table.rs");

pub(crate) struct Offsets(HashMap<String, &'static Recipe>);

impl Offsets {
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn of(original: Option<&[u8]>) -> Result<Self> {
        Self::select(original, RECIPES)
    }

    pub(crate) fn select(original: Option<&[u8]>, recipes: &'static [Recipe]) -> Result<Self> {
        let mut selected = HashMap::new();
        let Some(bytes) = original else {
            return Ok(Self(selected));
        };
        for chunk in mungefs::content_chunks(mungefs::open(bytes)?)? {
            if chunk.tag != *b"modl" {
                continue;
            }
            let Some(name) = chunk.child_string(b"NAME") else {
                continue;
            };
            if !recipes.iter().any(|r| r.name.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let hash = format!("{:x}", Sha256::digest(chunk.payload));
            if let Some(recipe) = recipes
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(&name) && r.input == hash)
            {
                selected.entry(name.to_ascii_lowercase()).or_insert(recipe);
            } else {
                anyhow::bail!("{name}: unrecognized donor vertex-offset fingerprint {hash}");
            }
        }
        Ok(Self(selected))
    }

    pub(crate) fn apply(&self, model: &mut Model) -> Result<bool> {
        let Some(recipe) = self.0.get(&model.name.to_ascii_lowercase()) else {
            return Ok(false);
        };
        ensure!(
            !model.bone_space,
            "{}: offsets need a posed model",
            model.name
        );
        let drawn: Vec<usize> = model
            .segments
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.triangles.is_empty())
            .map(|(i, _)| i)
            .collect();
        ensure!(
            drawn.len() == recipe.shapes.len(),
            "{}: donor primitive count changed",
            model.name
        );
        for (&si, &(vertices, faces)) in drawn.iter().zip(recipe.shapes) {
            let s = &model.segments[si];
            ensure!(
                s.positions.len() == vertices as usize
                    && s.normals.len() == s.positions.len()
                    && s.triangles.len() == faces as usize
                    && s.triangles
                        .iter()
                        .flatten()
                        .all(|&i| (i as usize) < s.positions.len()),
                "{}: donor primitive shape changed",
                model.name
            );
        }
        if recipe.moves.is_empty() && recipe.removals.is_empty() {
            return Ok(false);
        }
        // Edit a clone so a failed guard leaves the model untouched.
        let mut result = model.clone();
        let mut moved: Vec<Vec<bool>> = result
            .segments
            .iter()
            .map(|s| vec![false; s.positions.len()])
            .collect();
        for &(prim, vertex, delta) in recipe.moves {
            let si = *drawn
                .get(prim as usize)
                .ok_or_else(|| anyhow::anyhow!("{}: bad offset primitive", model.name))?;
            let segment = &mut result.segments[si];
            let v = vertex as usize;
            ensure!(
                v < segment.positions.len(),
                "{}: bad offset vertex",
                model.name
            );
            let p = (Vec3::from(segment.positions[v]) + Vec3::from(delta)).to_array();
            ensure!(
                p.iter().all(|x| x.is_finite()),
                "{}: nonfinite offset",
                model.name
            );
            segment.positions[v] = p;
            moved[si][v] = true;
        }
        let mut removals: Vec<(usize, usize)> = Vec::with_capacity(recipe.removals.len());
        for &(prim, face) in recipe.removals {
            let si = *drawn
                .get(prim as usize)
                .ok_or_else(|| anyhow::anyhow!("{}: bad removal primitive", model.name))?;
            ensure!(
                (face as usize) < result.segments[si].triangles.len(),
                "{}: bad removal face",
                model.name
            );
            removals.push((si, face as usize));
        }
        removals.sort_unstable();
        removals.dedup();
        for &(si, face) in removals.iter().rev() {
            let corners = result.segments[si].triangles.remove(face);
            for i in corners {
                moved[si][i as usize] = true;
            }
        }
        for (segment, moved) in result.segments.iter_mut().zip(&moved) {
            if moved.iter().any(|&m| m) {
                segment.index_count = (segment.triangles.len() * 3) as u32;
                update_normals(segment, moved);
            }
        }
        eprintln!(
            "render offsets: {} from {} ({} moves, {} removals)",
            model.name,
            recipe.input,
            recipe.moves.len(),
            recipe.removals.len()
        );
        *model = result;
        Ok(true)
    }
}

/// Area-weighted normals for vertices on faces that touch a moved vertex; other
/// normals, and every UV/colour stream, keep their donor values.
fn update_normals(segment: &mut mungefs::model::Segment, moved: &[bool]) {
    let mut affected = vec![false; moved.len()];
    for triangle in &segment.triangles {
        if triangle.iter().any(|&i| moved[i as usize]) {
            for &i in triangle {
                affected[i as usize] = true;
            }
        }
    }
    let mut normals = vec![Vec3::ZERO; moved.len()];
    for triangle in &segment.triangles {
        let [a, b, c] = triangle.map(|i| Vec3::from(segment.positions[i as usize]));
        let normal = (b - a).cross(c - a);
        for &i in triangle {
            if affected[i as usize] {
                normals[i as usize] += normal;
            }
        }
    }
    for (i, normal) in normals.into_iter().enumerate() {
        if affected[i] && normal.length_squared() > 0. {
            segment.normals[i] = normal.normalize().to_array();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType, Segment};

    static TEST: [Recipe; 1] = [Recipe {
        name: "test_wall",
        input: "unused",
        shapes: &[(4, 2)],
        moves: &[(0, 1, [0., 0.5, 0.]), (0, 2, [0., 0.5, 0.])],
        removals: &[(0, 1)],
    }];

    fn wall(empty_first: bool) -> Model {
        let quad = Segment {
            vertex_count: 4,
            index_count: 6,
            bf2_material: false,
            pc_geometry: false,
            material_flags: 1,
            render_type: RenderType::Bf1("Normal".into()),
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0; 2],
            textures: vec![(0, "donor".into())],
            bone: None,
            group: None,
            bmap: vec![],
            bind: vec![],
            wght: vec![],
            positions: vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            normals: vec![[0., 0., 1.]; 4],
            uvs: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
            colors: Some(vec![[1, 2, 3, 4]; 4]),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut segments = vec![quad];
        if empty_first {
            let mut empty = segments[0].clone();
            empty.triangles.clear();
            segments.insert(0, empty);
        }
        Model {
            name: "test_wall".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [0.; 3],
                max: [1.; 3],
            },
            vis_box: Aabb {
                min: [0.; 3],
                max: [1.; 3],
            },
            sphere: Some([0.5, 0.5, 0., 1.]),
            face_count: 2,
            segments,
        }
    }

    fn table() -> Offsets {
        Offsets(HashMap::from([("test_wall".to_string(), &TEST[0])]))
    }

    #[test]
    fn ps2_and_unknown_inputs_select_nothing() {
        assert!(Offsets::of(None).unwrap().0.is_empty());
        assert!(Offsets::select(Some(b"ucfb\0\0\0\0"), &TEST)
            .unwrap()
            .0
            .is_empty());
    }

    #[test]
    fn recognized_name_requires_its_exact_donor_fingerprint() {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = tag.to_vec();
            bytes.extend((payload.len() as u32).to_le_bytes());
            bytes.extend(payload);
            bytes.resize(bytes.len().next_multiple_of(4), 0);
            bytes
        }
        let payload = chunk(b"NAME", b"test_wall\0");
        let bytes = chunk(b"ucfb", &chunk(b"modl", &payload));
        assert!(Offsets::select(Some(&bytes), &TEST).is_err());
        let input = Box::leak(format!("{:x}", Sha256::digest(&payload)).into_boxed_str());
        let recipes = Box::leak(Box::new([Recipe { input, ..TEST[0] }]));
        assert_eq!(Offsets::select(Some(&bytes), recipes).unwrap().0.len(), 1);
        // The same named model with an extra source record must refuse even if
        // its eventual vertex/face counts could still match the recipe.
        let mut changed = payload;
        changed.extend(chunk(b"TEST", &[1, 2, 3, 4]));
        let bytes = chunk(b"ucfb", &chunk(b"modl", &changed));
        assert!(Offsets::select(Some(&bytes), recipes).is_err());
    }

    #[test]
    fn moves_and_removals_keep_attributes_and_headers() {
        let mut m = wall(true);
        let old = m.clone();
        assert!(table().apply(&mut m).unwrap());
        let s = &m.segments[1];
        assert_eq!(s.positions[1], [1., 0.5, 0.]);
        assert_eq!(s.positions[2], [1., 1.5, 0.]);
        assert_eq!(s.triangles, vec![[0, 1, 2]]);
        assert_eq!(s.index_count, 3);
        assert_eq!(s.uvs, old.segments[1].uvs);
        assert_eq!(s.colors, old.segments[1].colors);
        assert_eq!(m.segments[0].positions, old.segments[0].positions);
        assert_eq!(
            (m.vertex_box.max, m.sphere, m.face_count),
            (old.vertex_box.max, old.sphere, old.face_count)
        );
        assert!((Vec3::from(s.normals[1]) - Vec3::Z).length() < 1e-6);
    }

    #[test]
    fn a_changed_shape_fails_without_editing() {
        let mut m = wall(false);
        m.segments[0].positions.push([2., 2., 2.]);
        m.segments[0].normals.push([0., 0., 1.]);
        let old = m.clone();
        assert!(table().apply(&mut m).is_err());
        assert_eq!(m.segments[0].positions, old.segments[0].positions);
        assert_eq!(m.segments[0].triangles, old.segments[0].triangles);
    }

    #[test]
    fn other_models_are_left_alone() {
        let mut m = wall(false);
        m.name = "another".into();
        let old = m.clone();
        assert!(!table().apply(&mut m).unwrap());
        assert_eq!(m.segments[0].positions, old.segments[0].positions);
    }

    #[test]
    fn a_recognized_unchanged_donor_does_not_request_a_rewrite() {
        let recipe: &'static Recipe = Box::leak(Box::new(Recipe {
            moves: &[],
            removals: &[],
            ..TEST[0]
        }));
        let offsets = Offsets(HashMap::from([("test_wall".into(), recipe)]));
        let mut model = wall(false);
        assert!(!offsets.apply(&mut model).unwrap());
        assert_eq!(
            model.segments[0].positions,
            wall(false).segments[0].positions
        );
    }

    #[test]
    fn an_invalid_removal_cannot_leave_an_earlier_move_applied() {
        let recipe: &'static Recipe = Box::leak(Box::new(Recipe {
            removals: &[(0, 9)],
            ..TEST[0]
        }));
        let offsets = Offsets(HashMap::from([("test_wall".into(), recipe)]));
        let mut model = wall(false);
        let original = model.clone();
        assert!(offsets.apply(&mut model).is_err());
        assert_eq!(model.segments[0].positions, original.segments[0].positions);
        assert_eq!(model.segments[0].triangles, original.segments[0].triangles);
        assert_eq!(model.segments[0].normals, original.segments[0].normals);
    }

    #[test]
    fn every_table_row_is_in_range_and_finite() {
        for r in RECIPES {
            assert_eq!(r.input.len(), 64, "{}", r.name);
            for &(p, v, d) in r.moves {
                let (vertices, _) = r.shapes[p as usize];
                assert!(
                    (v as u32) < vertices && d.iter().all(|x| x.is_finite()),
                    "{}",
                    r.name
                );
            }
            for &(p, f) in r.removals {
                assert!((f as u32) < r.shapes[p as usize].1, "{}", r.name);
            }
        }
    }
}
