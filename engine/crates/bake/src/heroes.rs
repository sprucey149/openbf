//! BF2 hero assets, discovered from the disc's SIDE class references.
//! The side bake used to enumerate infantry alone. This pass includes every
//! hero soldier, held/offhand model, HUD model and blade texture, including
//! campaign variants. Gameplay remains in the class database and combo bake.

use crate::classes::BakedClass;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroAssets {
    pub class: String,
    pub geometry: String,
    pub side: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HeroBake {
    pub heroes: Vec<HeroAssets>,
    pub soldiers: Vec<String>,
    pub weapons: Vec<String>,
    pub textures: Vec<String>,
}

fn chain<'a>(classes: &'a [BakedClass], name: &str) -> Vec<&'a BakedClass> {
    let mut result = Vec::new();
    let mut current = name;
    while let Some(c) = classes
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(current))
    {
        if result.iter().any(|p: &&BakedClass| p.name == c.name) {
            break;
        }
        result.push(c);
        current = &c.base;
    }
    result
}

fn scalar<'a>(classes: &'a [BakedClass], name: &str, key: &str) -> Option<&'a str> {
    let hash = mungefs::fnv1a_lowercase(key);
    chain(classes, name).into_iter().find_map(|c| {
        c.props
            .iter()
            .rev()
            .find(|p| p.0 == hash)
            .map(|p| p.1.as_str())
    })
}

fn is_hero(classes: &[BakedClass], c: &BakedClass) -> bool {
    c.kind == "entc"
        && matches!(
            c.name.split('_').next(),
            Some("all" | "imp" | "rep" | "cis")
        )
        && c.name.split('_').nth(1) == Some("hero")
        && chain(classes, &c.name)
            .last()
            .is_some_and(|c| c.base.eq_ignore_ascii_case("soldier"))
}

/// Bake the four BF2 sides' hero closure to a side output directory.
/// `disc` is the extracted `DATA/_LVL_PS2` directory, not its parent.
pub fn bake_heroes(disc: &Path, out: &Path) -> Result<HeroBake> {
    let ingame = crate::join_ci(disc, "INGAME.LVL");
    let side_dir = crate::join_ci(disc, "SIDE");
    let sides: Vec<_> = ["ALL", "IMP", "REP", "CIS"]
        .map(|s| (s, crate::join_ci(&side_dir, &format!("{s}.LVL"))))
        .into_iter()
        .collect();
    let mut inputs = Vec::new();
    for path in sides.iter().map(|s| &s.1).chain([&ingame]) {
        let data = std::fs::read(path).with_context(|| path.display().to_string())?;
        inputs.push((path, data));
    }
    let mut classes = Vec::new();
    let mut models = BTreeSet::new();
    let mut texture_sources: BTreeMap<String, usize> = BTreeMap::new();
    for (index, (_, data)) in inputs.iter().enumerate() {
        classes.extend(crate::classes::read_classes(
            data,
            &crate::classes::CLASS_TAGS,
            crate::classes::MalformedClass::Strict,
        )?);
        let inner = mungefs::open(data).map_err(|e| anyhow!("{e}"))?;
        for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
            if &c.tag == b"modl" {
                if let Some(name) = c.child_string(b"NAME") {
                    models.insert(name.to_ascii_lowercase());
                }
            } else if &c.tag == b"tex_" {
                if let Some(name) = c.child_string(b"NAME") {
                    texture_sources
                        .entry(name.to_ascii_lowercase())
                        .or_insert(index);
                }
            }
        }
    }
    let mut result = HeroBake::default();
    let mut closure = BTreeSet::new();
    let mut needed_models = BTreeSet::new();
    let mut needed_textures = BTreeSet::new();
    let h_name = mungefs::fnv1a_lowercase("WeaponName");
    let numbered: Vec<_> = (1..=4)
        .map(|n| mungefs::fnv1a_lowercase(&format!("WeaponName{n}")))
        .collect();
    for c in &classes {
        if !is_hero(&classes, c) {
            continue;
        }
        let geometry = scalar(&classes, &c.name, "GeometryName")
            .ok_or_else(|| anyhow!("hero {} has no GeometryName", c.name))?
            .to_ascii_lowercase();
        if result.heroes.iter().any(|h| h.class == c.name) {
            continue;
        }
        let side = c.name.split('_').next().unwrap().to_ascii_uppercase();
        result.heroes.push(HeroAssets {
            class: c.name.clone(),
            geometry: geometry.clone(),
            side: side.to_ascii_lowercase(),
        });
        if !result.soldiers.contains(&geometry) {
            let source = sides.iter().find(|s| s.0 == side).unwrap();
            let baked = crate::soldier::bake_soldier_anims(
                &source.1.to_string_lossy(),
                &geometry,
                Some(&ingame.to_string_lossy()),
                &out.to_string_lossy(),
                None,
            )
            .with_context(|| format!("hero {} ({geometry})", c.name))?;
            crate::cloth::bake_companion(&source.1, &geometry, out)?;
            println!(
                "hero {}: {} bones, {} clips, cloths {:?}",
                c.name, baked.bones, baked.clips, baked.cloths
            );
            result.soldiers.push(geometry);
        }
        // Include every referenced ancestor slot as well: an overridden slot's
        // asset is harmless and this preserves all campaign class variants.
        for c in chain(&classes, &c.name) {
            for (h, value) in &c.props {
                if *h == h_name || numbered.contains(h) {
                    closure.insert(value.to_ascii_lowercase());
                }
            }
        }
    }
    if result.heroes.is_empty() {
        bail!("no BF2 hero soldiers in {}", disc.display());
    }
    // Follow class-valued properties (weapon -> ordnance -> explosion) and
    // their parents before collecting named assets. Keep all repeated edges.
    let mut pending: Vec<_> = closure.iter().cloned().collect();
    let mut visited = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        for c in chain(&classes, &name) {
            for (_, value) in &c.props {
                if classes.iter().any(|c| c.name.eq_ignore_ascii_case(value)) {
                    pending.push(value.to_ascii_lowercase());
                }
            }
            for key in [
                "GeometryName",
                "OffhandGeometryName",
                "DroppedItemGeometryName",
                "HUDTag",
                "IconTexture",
                "LightSaberTexture",
            ] {
                let hash = mungefs::fnv1a_lowercase(key);
                for (_, value) in c.props.iter().filter(|p| p.0 == hash) {
                    let value = value.trim().to_ascii_lowercase();
                    if models.contains(&value) {
                        needed_models.insert(value);
                    } else if texture_sources.contains_key(&value) {
                        needed_textures.insert(value);
                    } else if matches!(
                        key,
                        "GeometryName" | "OffhandGeometryName" | "LightSaberTexture"
                    ) && !value.is_empty()
                        && value != "none"
                    {
                        bail!(
                            "{}: {key} asset {value} is not in the BF2 sides/INGAME",
                            c.name
                        );
                    }
                }
            }
        }
    }
    let sources = inputs
        .iter()
        .map(|p| p.0.to_string_lossy())
        .collect::<Vec<_>>()
        .join(",");
    for model in needed_models {
        crate::soldier::bake_weapon(&sources, &model, &out.to_string_lossy(), None)?;
        result.weapons.push(model);
    }
    let flags: HashMap<_, _> = needed_textures
        .iter()
        .map(|t| (t.clone(), crate::detail::ALPHA_IS_MEANINGFUL))
        .collect();
    for (_, data) in &inputs {
        let inner = mungefs::open(data).map_err(|e| anyhow!("{e}"))?;
        for tex in crate::soldier::bake_textures_flagged(
            inner,
            &needed_textures.iter().cloned().collect::<Vec<_>>(),
            &flags,
            out,
            None,
        )? {
            if !result.textures.contains(&tex) {
                result.textures.push(tex);
            }
        }
    }
    result.heroes.sort_by(|a, b| a.class.cmp(&b.class));
    result.soldiers.sort();
    result.textures.sort();
    crate::write_ron(&out.join("heroes.ron"), &result)?;
    Ok(result)
}
