//! Rejoin donor-only render pieces removed by canonical world replacement.
//!
//! Xbox Jabba splits the torture room and its pipes into independently placed
//! props. Their original placements coincide, while the PS2 world names only
//! the unsuffixed models. Keep the canonical simulation placements/collision;
//! append the already-posed donor segments to those two render models only.
//! The source LVL has passed gameplay fingerprint validation before this call.
//! Reject missing dependencies, duplicate/changed placements and changed class
//! bindings rather than silently emitting only part of a room.

use anyhow::{anyhow, bail, ensure, Result};
use mungefs::model::Model;
use mungefs::world::{Transform, World};
use std::collections::{HashMap, HashSet};

pub(super) struct Reassembled {
    pub model: Model,
    pub lighting: Vec<crate::glb::LightPath>,
}

struct Recipe {
    target: &'static str,
    parts: &'static [&'static str],
}

const RECIPES: &[Recipe] = &[
    Recipe {
        target: "tat3_bldg_torture_main",
        parts: &["tat3_bldg_torture_main_2", "tat3_bldg_torture_main_3"],
    },
    Recipe {
        target: "tat3_bldg_torture_pipes",
        parts: &["tat3_bldg_torture_pipes_1"],
    },
];

/// Called only for non-reference bytes returned by `read_with_original`.
/// `models` have already been put through the original donor bind pose.
pub(super) fn models(
    original: &[u8],
    corrected_worlds: &[&World],
    models: &HashMap<String, Model>,
) -> Result<Vec<Reassembled>> {
    let chunks = mungefs::content_chunks(mungefs::open(original).map_err(|e| anyhow!(e))?)
        .map_err(|e| anyhow!(e))?;
    let mut resources = HashSet::new();
    let mut bindings = HashMap::new();
    let mut lods = HashMap::new();
    let mut worlds = Vec::new();
    for chunk in chunks {
        match &chunk.tag {
            b"modl" => {
                if let Some(name) = chunk.child_string(b"NAME") {
                    resources.insert(name.to_ascii_lowercase());
                }
            }
            b"entc" => {
                let class = mungefs::entity::EntityClass::parse(&chunk).map_err(|e| anyhow!(e))?;
                bindings.insert(
                    class.name.to_ascii_lowercase(),
                    (
                        class.base.to_ascii_lowercase(),
                        class.geometry_name().map(str::to_ascii_lowercase),
                    ),
                );
            }
            b"gmod" => {
                let geometry = mungefs::entity::GeomModel::parse(&chunk).map_err(|e| anyhow!(e))?;
                lods.insert(
                    geometry.name.to_ascii_lowercase(),
                    geometry.best_model().map(str::to_ascii_lowercase),
                );
            }
            b"wrld" => {
                worlds.push(World::parse(&chunk).map_err(|e| anyhow!(e))?);
            }
            _ => {}
        }
    }
    if !worlds
        .iter()
        .any(|world| world.name.eq_ignore_ascii_case("tat3"))
    {
        return Ok(Vec::new());
    }
    let originals: Vec<_> = worlds.iter().collect();
    let mut out = Vec::new();
    for recipe in RECIPES {
        if !recipe.parts.iter().any(|part| resources.contains(*part)) {
            continue;
        }
        for name in std::iter::once(recipe.target).chain(recipe.parts.iter().copied()) {
            ensure!(
                resources.contains(name),
                "render reassembly: missing source model {name}"
            );
            ensure!(
                bindings.get(name) == Some(&("prop".to_owned(), Some(name.to_owned())))
                    && lods.get(name) == Some(&Some(name.to_owned())),
                "render reassembly: changed static class/model binding for {name}"
            );
        }
        out.push(join(recipe, &originals, corrected_worlds, models)?);
    }
    Ok(out)
}

fn unique_placement(worlds: &[&World], class: &str) -> Result<Transform> {
    let matches: Vec<_> = worlds
        .iter()
        .flat_map(|world| &world.instances)
        .filter(|instance| instance.class.eq_ignore_ascii_case(class))
        .collect();
    ensure!(
        matches.len() == 1,
        "render reassembly: {class} needs exactly one placement, got {}",
        matches.len()
    );
    Ok(matches[0].transform)
}

fn join(
    recipe: &Recipe,
    original_worlds: &[&World],
    corrected_worlds: &[&World],
    models: &HashMap<String, Model>,
) -> Result<Reassembled> {
    let original = unique_placement(original_worlds, recipe.target)?;
    let canonical = unique_placement(corrected_worlds, recipe.target)?;
    ensure!(
        original == canonical,
        "render reassembly: changed canonical placement for {}",
        recipe.target
    );
    let base = models
        .get(recipe.target)
        .ok_or_else(|| anyhow!("render reassembly: missing posed model {}", recipe.target))?;
    ensure!(
        !base.bone_space,
        "render reassembly: {} is not posed",
        recipe.target
    );
    let mut combined = base.clone();
    let mut lighting: Vec<_> = base
        .segments
        .iter()
        .map(|segment| crate::glb::light_path(base, segment))
        .collect();
    for part in recipe.parts {
        ensure!(
            unique_placement(original_worlds, part)? == original,
            "render reassembly: changed relative placement for {part}"
        );
        if corrected_worlds
            .iter()
            .flat_map(|world| &world.instances)
            .any(|instance| instance.class.eq_ignore_ascii_case(part))
        {
            bail!("render reassembly: {part} is still placed; joining would duplicate it");
        }
        let model = models
            .get(*part)
            .ok_or_else(|| anyhow!("render reassembly: missing posed model {part}"))?;
        ensure!(!model.bone_space, "render reassembly: {part} is not posed");
        ensure!(
            !model.segments.is_empty(),
            "render reassembly: {part} has no render segments"
        );
        lighting.extend(
            model
                .segments
                .iter()
                .map(|segment| crate::glb::light_path(model, segment)),
        );
        combined.segments.extend(model.segments.iter().cloned());
    }
    // Preserve the canonical model header, including its sphere. Render
    // vertices/materials come from the donor; simulation numbers do not move.
    // Model::face_count is an opaque INFO word (see segdump); GLB counts and
    // accessor bounds come from the actual segment streams, not this header.
    Ok(Reassembled {
        model: combined,
        lighting,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType, Segment};
    use mungefs::world::Instance;

    fn transform(x: f32) -> Transform {
        Transform {
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [x, 0.0, 0.0],
        }
    }

    fn world(classes: &[&str]) -> World {
        World {
            name: "tat3".into(),
            terrain_name: None,
            sky_name: None,
            instances: classes
                .iter()
                .map(|name| Instance {
                    class: (*name).into(),
                    name: String::new(),
                    transform: transform(3.0),
                    props: Vec::new(),
                })
                .collect(),
            regions: Vec::new(),
            hints: Vec::new(),
            barriers: Vec::new(),
        }
    }

    fn model(name: &str, x: f32) -> Model {
        Model {
            name: name.into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            vis_box: Aabb {
                min: [0.0; 3],
                max: [2.0; 3],
            },
            face_count: 7,
            sphere: Some([1.0, 2.0, 3.0, 4.0]),
            segments: vec![Segment {
                vertex_count: 3,
                index_count: 3,
                render_type: RenderType::Bf2(0),
                material_flags: 4,
                bf2_material: true,
                pc_geometry: false,
                spec_power: 0,
                spec_color: 0,
                uv_anim: [0; 2],
                textures: vec![(0, "donor_texture".into())],
                bone: None,
                group: None,
                bmap: Vec::new(),
                bind: Vec::new(),
                wght: Vec::new(),
                positions: vec![[x, 0.0, 0.0], [x, 1.0, 0.0], [x, 0.0, 1.0]],
                normals: vec![[1.0, 0.0, 0.0]; 3],
                uvs: vec![[0.0, 0.0]; 3],
                colors: Some(vec![[23, 45, 67, 255]; 3]),
                triangles: vec![[0, 1, 2]],
            }],
        }
    }

    fn inputs() -> (World, World, HashMap<String, Model>) {
        let recipe = &RECIPES[0];
        let names: Vec<_> = std::iter::once(recipe.target)
            .chain(recipe.parts.iter().copied())
            .collect();
        let original = world(&names);
        let corrected = world(&[recipe.target]);
        let models = names
            .iter()
            .enumerate()
            .map(|(i, name)| ((*name).into(), model(name, i as f32)))
            .collect();
        (original, corrected, models)
    }

    fn resource(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = Vec::from(*tag);
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out
    }

    fn text(tag: &[u8; 4], name: &str) -> Vec<u8> {
        let mut body = name.as_bytes().to_vec();
        body.push(0);
        resource(tag, &body)
    }

    // Metadata discovery is independent of the already-decoded/posed Model
    // cache, so these fixtures need only NAME on their modl resources.
    fn metadata(original: &World, changed_binding: bool, omit_model: Option<&str>) -> Vec<u8> {
        let mut body = Vec::new();
        for instance in &original.instances {
            let name = &instance.class;
            if omit_model != Some(name.as_str()) {
                body.extend(resource(b"modl", &text(b"NAME", name)));
            }
            let mut class = text(b"TYPE", name);
            class.extend(text(b"BASE", "prop"));
            let mut prop = mungefs::entity::PROP_GEOMETRYNAME.to_le_bytes().to_vec();
            prop.extend(if changed_binding && name == RECIPES[0].parts[0] {
                b"different"
            } else {
                name.as_bytes()
            });
            prop.push(0);
            class.extend(resource(b"PROP", &prop));
            body.extend(resource(b"entc", &class));
            let mut geometry = text(b"NAME", name);
            let mut lod = name.as_bytes().to_vec();
            lod.push(0);
            lod.extend(1u32.to_le_bytes());
            geometry.extend(resource(b"LOD0", &lod));
            body.extend(resource(b"gmod", &geometry));
        }
        let mut world = text(b"NAME", &original.name);
        for instance in &original.instances {
            let mut info = text(b"TYPE", &instance.class);
            info.extend(text(b"NAME", &instance.name));
            let mut transform = Vec::new();
            for value in instance
                .transform
                .rotation
                .iter()
                .flatten()
                .chain(&instance.transform.translation)
            {
                transform.extend(value.to_le_bytes());
            }
            info.extend(resource(b"XFRM", &transform));
            world.extend(resource(b"inst", &resource(b"INFO", &info)));
        }
        body.extend(resource(b"wrld", &world));
        resource(b"ucfb", &body)
    }

    #[test]
    fn dispatcher_preserves_each_source_models_evaluated_lighting() {
        let (original, corrected, mut posed) = inputs();
        posed.get_mut(RECIPES[0].target).unwrap().segments[0].material_flags = 0x201;
        for part in RECIPES[0].parts {
            posed.get_mut(*part).unwrap().segments[0].material_flags = 1;
        }
        let out = super::models(&metadata(&original, false, None), &[&corrected], &posed).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].lighting,
            [
                crate::glb::LightPath::PrelitAdd,
                crate::glb::LightPath::Lit,
                crate::glb::LightPath::Lit
            ]
        );
        assert_eq!(
            crate::glb::light_path(&out[0].model, &out[0].model.segments[1]),
            crate::glb::LightPath::Prelit
        );
    }

    #[test]
    fn dispatcher_rejects_changed_bindings_and_absent_source_dependencies() {
        let (original, corrected, posed) = inputs();
        assert!(super::models(&metadata(&original, true, None), &[&corrected], &posed).is_err());
        assert!(super::models(
            &metadata(&original, false, Some(RECIPES[0].parts[1])),
            &[&corrected],
            &posed
        )
        .is_err());
    }

    #[test]
    fn reference_partition_without_split_models_is_unchanged() {
        let (_, corrected, posed) = inputs();
        assert!(
            super::models(&metadata(&corrected, false, None), &[&corrected], &posed)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn rejoins_only_render_segments_and_preserves_canonical_header() {
        let (original, corrected, models) = inputs();
        let rebuilt = join(&RECIPES[0], &[&original], &[&corrected], &models).unwrap();
        assert_eq!(rebuilt.lighting.len(), 3);
        let joined = rebuilt.model;
        assert_eq!(joined.segments.len(), 3);
        assert_eq!(joined.segments[2].positions[0], [2.0, 0.0, 0.0]);
        assert_eq!(joined.segments[2].colors, Some(vec![[23, 45, 67, 255]; 3]));
        assert_eq!(joined.sphere, models[RECIPES[0].target].sphere);
        assert_eq!(joined.vis_box.max, [2.0; 3]);
        assert_eq!(joined.face_count, 7);
        assert_eq!(corrected.instances.len(), 1);
    }

    #[test]
    fn missing_dependency_fails_instead_of_writing_a_partial_room() {
        let (original, corrected, mut models) = inputs();
        models.remove(RECIPES[0].parts[1]);
        assert!(join(&RECIPES[0], &[&original], &[&corrected], &models)
            .err()
            .unwrap()
            .to_string()
            .contains("missing posed model"));
    }

    #[test]
    fn moved_or_duplicate_source_piece_is_rejected() {
        let (mut original, corrected, models) = inputs();
        original.instances[1].transform = transform(4.0);
        assert!(join(&RECIPES[0], &[&original], &[&corrected], &models).is_err());
        original.instances[1].transform = transform(3.0);
        let duplicate = world(&[RECIPES[0].parts[0]]);
        assert!(join(
            &RECIPES[0],
            &[&original, &duplicate],
            &[&corrected],
            &models
        )
        .is_err());
    }

    #[test]
    fn canonically_placed_piece_is_not_drawn_twice() {
        let (original, mut corrected, models) = inputs();
        corrected
            .instances
            .push(world(&[RECIPES[0].parts[0]]).instances.remove(0));
        assert!(join(&RECIPES[0], &[&original], &[&corrected], &models)
            .err()
            .unwrap()
            .to_string()
            .contains("duplicate"));
    }
}
