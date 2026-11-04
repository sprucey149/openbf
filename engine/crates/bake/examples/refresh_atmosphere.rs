use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;

fn install(fresh: &Path, installed: &Path, scratch: &Path, filename: &str) -> Result<()> {
    if installed.exists() {
        std::fs::copy(installed, scratch.join(format!("previous_{filename}")))?;
    }
    std::fs::copy(fresh, installed)?;
    Ok(())
}

fn install_water(
    target: &Path,
    scratch: &Path,
    manifest: &mut manifest::Manifest,
    water: Option<manifest::ManifestWater>,
) -> Result<()> {
    if let Some(water) = &water {
        let filename = &water.glb;
        install(
            &scratch.join("fresh").join(filename),
            &target.join(filename),
            scratch,
            filename,
        )?;
    }
    manifest.water = water;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(
        args.len() <= 5 && args.get(4).is_none_or(|arg| arg == "--water-only"),
        "expected source LVL, baked map, new scratch directory and optional --water-only"
    );
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
    if args.get(4).is_some_and(|arg| arg == "--water-only") {
        std::fs::write(scratch.join("previous_manifest.ron"), original)?;
        install_water(target, scratch, &mut manifest, fresh.water)?;
        bake::write_ron(&path, &manifest)?;
        println!(
            "refreshed water {}; backup {}",
            target.display(),
            scratch.display()
        );
        return Ok(());
    }
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
    let defs: Vec<manifest::ClassDef> =
        ron::from_str(&std::fs::read_to_string(scratch.join("fresh/classes.ron"))?)?;
    for def in defs
        .iter()
        .filter(|d| d.base.eq_ignore_ascii_case("dusteffect"))
    {
        for (_, value) in def
            .props
            .iter()
            .filter(|(h, _)| *h == mungefs::fnv1a_lowercase("Texture"))
        {
            textures.insert(value.to_ascii_lowercase());
        }
    }
    for name in textures {
        let filename = format!("{name}.ktx2");
        let source = scratch.join("fresh/textures").join(&filename);
        if !source.exists() {
            let common = target
                .parent()
                .context("baked map parent")?
                .join(format!("common_{}", fresh.source_game))
                .join("textures")
                .join(&filename);
            anyhow::ensure!(
                common.exists(),
                "atmosphere texture {name} is absent from the map and {}",
                common.display()
            );
            continue;
        }
        install(
            &source,
            &target.join("textures").join(&filename),
            scratch,
            &filename,
        )?;
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
    manifest.sun_flare = fresh.sun_flare;
    install_water(target, scratch, &mut manifest, fresh.water)?;
    bake::write_ron(&path, &manifest)?;
    println!(
        "refreshed {}; backup {}",
        target.display(),
        scratch.display()
    );
    Ok(())
}
