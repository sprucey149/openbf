//! Optional static draw geometry. Stock model files and `Scene::meshes` remain
//! the inputs to cameras, bounds, joints and gameplay. Only world upload calls
//! `load_model`; admitting another model requires a consumer/placement audit.

use super::{CpuMesh, Manifest, Scene};
use anyhow::{ensure, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One registry shared with pack validation, containing audited stock paths.
pub fn stock_path(target: &str) -> Option<&'static str> {
    static PATHS: OnceLock<Vec<String>> = OnceLock::new();
    let (map, model) = target.split_once("/presentation/models/")?;
    let stock = format!("{map}/models/{model}");
    PATHS
        .get_or_init(|| serde_json::from_str(include_str!("presentation_models.json")).unwrap())
        .iter()
        .find(|s| **s == stock)
        .map(String::as_str)
}

fn placement(manifest: &Manifest, model: &str) -> Result<()> {
    ensure!(
        manifest.models.iter().any(|m| m == model),
        "stock model is not in map manifest"
    );
    let resolves = |class: &manifest::ManifestClass| {
        class.geometry.as_ref().is_some_and(|g| {
            manifest
                .geometries
                .iter()
                .find(|x| x.name.eq_ignore_ascii_case(g))
                .map_or(g.as_str(), |x| x.model.as_str())
                .eq_ignore_ascii_case(model)
        })
    };
    for class in manifest.classes.iter().filter(|c| resolves(c)) {
        ensure!(
            (class.base.eq_ignore_ascii_case("prop")
                || class.base.eq_ignore_ascii_case("building"))
                && class.door.is_none()
                && class.turret_rig.is_none()
                && class.hardpoints.is_empty()
                && class.attach.is_empty()
                && class.build_zone.is_none(),
            "presentation model has a moving or unsupported class: {}",
            class.name
        );
    }
    let mut placed = false;
    for instance in manifest
        .instances
        .iter()
        .chain(manifest.layers.iter().flat_map(|layer| &layer.instances))
    {
        let Some(class) = super::resolve_manifest_class(manifest, &instance.class)
            .filter(|class| resolves(class))
        else {
            continue;
        };
        ensure!(
            instance.class.eq_ignore_ascii_case(&class.name),
            "presentation model used by a build-zone alias: {}",
            instance.class
        );
        placed = true;
    }
    ensure!(placed, "presentation model has no stock map placement");
    Ok(())
}

fn rigid_document(path: &Path) -> Result<serde_json::Value> {
    let raw = pak::read(path)?;
    let gltf = gltf::Gltf::from_slice(&raw)?;
    let doc = &gltf.document;
    ensure!(
        gltf.blob.is_some()
            && doc.buffers().count() == 1
            && doc
                .buffers()
                .all(|b| matches!(b.source(), gltf::buffer::Source::Bin))
            && doc.skins().next().is_none()
            && doc.animations().next().is_none()
            && doc.images().next().is_none()
            && doc.textures().next().is_none()
            && doc.meshes().count() == 1,
        "presentation GLB must be self-contained and rigid"
    );
    for mesh in doc.meshes() {
        ensure!(
            mesh.weights().is_none(),
            "presentation morph weights are unsupported"
        );
        for p in mesh.primitives() {
            ensure!(
                p.get(&gltf::Semantic::Normals).is_some()
                    && p.get(&gltf::Semantic::TexCoords(0)).is_some(),
                "presentation GLB requires normals and texture coordinates"
            );
            ensure!(
                p.mode() == gltf::mesh::Mode::Triangles
                    && p.indices().is_some()
                    && p.morph_targets().next().is_none()
                    && p.material().alpha_mode() == gltf::material::AlphaMode::Opaque
                    && p.attributes().all(|(s, _)| matches!(
                        s,
                        gltf::Semantic::Positions
                            | gltf::Semantic::Normals
                            | gltf::Semantic::TexCoords(0)
                            | gltf::Semantic::Colors(0)
                    )),
                "presentation GLB requires opaque rigid triangles"
            );
        }
    }
    let value = serde_json::to_value(doc.as_json())?;
    ensure!(
        ["extensions", "extensionsUsed", "extensionsRequired"]
            .iter()
            .all(|k| value
                .get(k)
                .is_none_or(|v| v.as_object().is_some_and(|v| v.is_empty())
                    || v.as_array().is_some_and(|v| v.is_empty()))),
        "presentation GLB extensions are unsupported"
    );
    Ok(value)
}

fn checked_mesh(stock: &Path, payload: &Path) -> Result<CpuMesh> {
    // Validate before import: the general importer may resolve external resources.
    let original = rigid_document(stock)?;
    let replacement = rigid_document(payload)?;
    for key in ["nodes", "scenes", "scene", "materials"] {
        ensure!(
            original.get(key) == replacement.get(key),
            "presentation GLB changed {key}"
        );
    }
    let a = &original["meshes"][0];
    let b = &replacement["meshes"][0];
    ensure!(
        a.get("name") == b.get("name") && a.get("extras") == b.get("extras"),
        "presentation GLB changed model metadata"
    );
    let bindings = |v: &serde_json::Value| -> Vec<serde_json::Value> {
        v["primitives"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["material"].clone())
            .collect()
    };
    ensure!(
        bindings(a) == bindings(b),
        "presentation GLB changed material bindings"
    );
    let original = super::load_glb(stock)?;
    let replacement = super::load_glb(payload)?;
    ensure!(
        !replacement.positions.is_empty()
            && !replacement.indices.is_empty()
            && replacement.indices.len().is_multiple_of(3)
            && replacement
                .positions
                .iter()
                .flatten()
                .chain(replacement.normals.iter().flatten())
                .chain(replacement.uvs.iter().flatten())
                .all(|v| v.is_finite())
            && replacement
                .indices
                .iter()
                .all(|&i| (i as usize) < replacement.positions.len())
            && replacement.colors.iter().all(|c| c[3] == 255),
        "presentation GLB has invalid or nonopaque vertex data"
    );
    ensure!(
        !original.contact_surface && original.render_grid.is_none(),
        "presentation contact surfaces and render terrain are unsupported"
    );
    let bounds = |mesh: &CpuMesh| {
        let mut bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
        for p in &mesh.positions {
            for (i, &v) in p.iter().enumerate() {
                bounds[0][i] = core::fmath::min(bounds[0][i], v);
                bounds[1][i] = core::fmath::max(bounds[1][i], v);
            }
        }
        bounds
    };
    ensure!(
        bounds(&original)
            .iter()
            .flatten()
            .zip(bounds(&replacement).iter().flatten())
            .all(|(a, b)| a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-6),
        "presentation GLB changed stock bounds"
    );
    Ok(replacement)
}

/// Read at most once per uploaded model. Never insert the result in `Scene::meshes`.
pub fn load_model(scene: &Scene, model: &str) -> Result<Option<CpuMesh>> {
    let Some(map) = scene.dir.file_name().and_then(|s| s.to_str()) else {
        return Ok(None);
    };
    let target = format!("{map}/presentation/models/{model}.glb");
    if stock_path(&target).is_none() {
        return Ok(None);
    }
    let payload = scene
        .dir
        .join("presentation/models")
        .join(format!("{model}.glb"));
    if !pak::is_file(&payload) {
        return Ok(None);
    }
    placement(&scene.manifest, model)?;
    checked_mesh(
        &scene.dir.join("models").join(format!("{model}.glb")),
        &payload,
    )
    .with_context(|| target)
    .map(Some)
}

/// Add placeholders for audited presentation entries before `pak::replace_sources`.
/// The writer still validates the entire mapping before opening any output.
pub fn prepare_pack(entries: &mut Vec<(String, PathBuf)>, mapping: &Path) -> Result<()> {
    let rows: Vec<(String, PathBuf)> = serde_json::from_slice(&std::fs::read(mapping)?)?;
    let mut additions = Vec::new();
    for (target, payload) in rows {
        let Some(target) = target.strip_prefix("assets_baked/") else {
            continue;
        };
        // Unknown presentation entries must reach the existing missing-entry error.
        let Some(stock) = stock_path(target) else {
            continue;
        };
        let archive_target = format!("assets_baked/{target}");
        let archive_stock = format!("assets_baked/{stock}");
        ensure!(
            !entries.iter().any(|(n, _)| n == &archive_target),
            "presentation entry already exists in stock bake: {target}"
        );
        let (_, source) = entries
            .iter()
            .find(|(n, _)| n == &archive_stock)
            .context("presentation target has no stock model")?;
        let map = source
            .parent()
            .and_then(Path::parent)
            .context("stock map directory")?;
        let manifest: Manifest =
            ron::from_str(&std::fs::read_to_string(map.join("manifest.ron"))?)?;
        let model = source
            .file_stem()
            .and_then(|n| n.to_str())
            .context("stock model name")?;
        placement(&manifest, model)?;
        checked_mesh(source, &payload).with_context(|| format!("presentation {target}"))?;
        additions.push((archive_target, source.clone()));
    }
    entries.extend(additions);
    Ok(())
}
