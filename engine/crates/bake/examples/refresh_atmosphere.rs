use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let source = args.get(1).ok_or_else(|| anyhow!("source LVL"))?;
    let target = Path::new(args.get(2).ok_or_else(|| anyhow!("existing baked map"))?);
    let scratch = Path::new(
        args.get(3)
            .ok_or_else(|| anyhow!("new scratch directory"))?,
    );
    if scratch.exists() {
        bail!("scratch directory must be new: {}", scratch.display());
    }
    let path = target.join("manifest.ron");
    let original = std::fs::read(&path)?;
    let mut manifest: manifest::Manifest = ron::de::from_bytes(&original)?;
    let fresh = bake::map::bake_map(
        source,
        scratch.join("fresh").to_str().context("scratch path")?,
        None,
    )?;
    if let Some(sky) = &fresh.sky {
        for geometry in sky
            .models
            .iter()
            .map(|m| &m.geometry)
            .chain(sky.objects.iter().map(|o| &o.geometry))
        {
            if !manifest
                .models
                .iter()
                .any(|m| m.eq_ignore_ascii_case(geometry))
            {
                bail!("new sky geometry {geometry}: run a complete map bake");
            }
        }
    }
    std::fs::write(scratch.join("previous_manifest.ron"), original)?;
    let mut textures = std::collections::BTreeSet::new();
    if let Some(sky) = &fresh.sky {
        for model in &sky.models {
            let glb = gltf::Gltf::open(
                scratch
                    .join("fresh/models")
                    .join(format!("{}.glb", model.geometry.to_ascii_lowercase())),
            )?;
            for material in glb.materials() {
                if let Some(name) = material.name().map(str::trim) {
                    if fresh.textures.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                        textures.insert(name.to_ascii_lowercase());
                    }
                }
            }
        }
    }
    if let Some(name) = fresh.fog_cloud.as_ref().and_then(|c| c.texture.as_ref()) {
        textures.insert(name.to_ascii_lowercase());
    }
    for name in textures {
        let filename = format!("{}.ktx2", name.to_ascii_lowercase());
        let previous = target.join("textures").join(&filename);
        if previous.exists() {
            std::fs::copy(&previous, scratch.join(format!("previous_{filename}")))?;
        }
        std::fs::copy(scratch.join("fresh/textures").join(&filename), previous)?;
        if !manifest
            .textures
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&name))
        {
            manifest.textures.push(name.clone());
        }
    }
    manifest.sky = fresh.sky;
    manifest.fog_cloud = fresh.fog_cloud;
    bake::write_ron(&path, &manifest)?;
    println!(
        "refreshed {}; backup {}",
        target.display(),
        scratch.display()
    );
    Ok(())
}
