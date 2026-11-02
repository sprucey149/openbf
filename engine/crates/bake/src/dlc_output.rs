//! Private DLC staging and texture aliases. Installing these heroes must never
//! replace retail texture bytes, even when the source reuses retail names.

use crate::classes::BakedClass;
use crate::heroes::HeroBake;
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct Staging(PathBuf);

impl Staging {
    pub(super) fn new() -> Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        for _ in 0..128 {
            let path = std::env::temp_dir().join(format!(
                "openbf_dlc_stage_{}_{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("create private DLC staging"),
            }
        }
        bail!("could not allocate a private DLC staging directory")
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    if !root.exists() {
        return Ok(result);
    }
    ensure!(!root.is_symlink(), "DLC staging must not contain symlinks");
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        ensure!(!path.is_symlink(), "DLC staging must not contain symlinks");
        if path.is_dir() {
            result.extend(files(&path)?);
        } else {
            ensure!(path.is_file(), "DLC staging contains a non-file");
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}

fn directory(path: &Path) -> Result<()> {
    ensure!(
        path.ancestors().all(|p| !p.is_symlink()),
        "DLC destination must not contain symlink ancestors: {}",
        path.display()
    );
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn alias(value: &mut String, names: &BTreeMap<String, String>) {
    if let Some(name) = names.get(&value.trim().to_ascii_lowercase()) {
        *value = name.clone();
    }
}

fn glb_aliases(bytes: &[u8], names: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    ensure!(
        bytes.len() >= 28 && &bytes[..4] == b"glTF",
        "invalid staged GLB header"
    );
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    ensure!(
        word(4) == 2 && word(8) == bytes.len(),
        "invalid staged GLB length/version"
    );
    ensure!(&bytes[16..20] == b"JSON", "staged GLB has no JSON chunk");
    let json_end = 20usize.checked_add(word(12)).context("GLB JSON overflow")?;
    ensure!(json_end <= bytes.len() - 8, "staged GLB JSON exceeds file");
    ensure!(
        &bytes[json_end + 4..json_end + 8] == b"BIN\0",
        "staged GLB has no BIN chunk"
    );
    let bin_start = json_end + 8;
    ensure!(
        word(json_end) == bytes.len() - bin_start,
        "staged GLB BIN length disagrees"
    );
    let mut doc: Value = serde_json::from_slice(&bytes[20..json_end])?;
    if let Some(materials) = doc.get_mut("materials").and_then(Value::as_array_mut) {
        for material in materials {
            if let Some(Value::String(name)) = material.get_mut("name") {
                alias(name, names);
            }
        }
    }
    // The BIN payload (geometry, joints, weights and animation) stays exact.
    Ok(crate::glb::pack_glb(
        &serde_json::to_vec(&doc)?,
        &bytes[bin_start..],
    ))
}

/// Replace a folder only these heroes own with what the bake staged. Nothing
/// staged (a source with no effects) leaves nothing behind.
pub(super) fn replace_tree(staged: &Path, destination: &Path) -> Result<()> {
    ensure!(
        !destination.is_symlink(),
        "DLC destination is a symlink: {}",
        destination.display()
    );
    if destination.exists() {
        std::fs::remove_dir_all(destination)?;
    }
    for path in files(staged)? {
        let target = destination.join(path.strip_prefix(staged)?);
        directory(target.parent().context("DLC file has no parent")?)?;
        std::fs::copy(&path, &target)?;
    }
    Ok(())
}

/// Install only the side files emitted by the selective bake. All source
/// texture names become `dlch_<name>`; `_dtl` and `.tier2` companions follow
/// their base automatically. Class references and generated descriptors agree
/// with those aliases, while gameplay fields and model names stay untouched.
pub(super) fn install(
    staged: &Path,
    destination: &Path,
    classes: &mut [BakedClass],
    result: &mut HeroBake,
) -> Result<()> {
    let mut names = BTreeMap::new();
    let textures = files(&staged.join("textures"))?;
    for path in &textures {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("non-UTF8 DLC texture name")?;
        let base = name
            .strip_suffix(".ktx2")
            .context("unexpected staged texture format")?;
        let base = base.strip_suffix(".tier2").unwrap_or(base);
        names.insert(base.to_owned(), format!("dlch_{base}"));
    }
    let texture_keys: Vec<_> = [
        "LightSaberTexture",
        "LightSaberGlowTexture",
        "IconTexture",
        "HUDTexture",
        "OverrideTexture",
    ]
    .into_iter()
    .flat_map(|key| {
        std::iter::once(key.to_owned()).chain((1..=4).map(move |n| format!("{key}{n}")))
    })
    .map(|key| mungefs::fnv1a_lowercase(&key))
    .collect();
    for class in classes {
        for (key, value) in &mut class.props {
            if texture_keys.contains(key)
                && !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "none" | "null" | "notexture"
                )
            {
                alias(value, &names);
            }
        }
    }
    for name in &mut result.textures {
        alias(name, &names);
    }
    result.textures.sort();
    result.textures.dedup();
    for subdir in ["soldiers", "weapons", "textures"] {
        for path in files(&staged.join(subdir))? {
            let relative = path.strip_prefix(staged)?;
            let target = if subdir == "textures" {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("non-UTF8 DLC texture name")?;
                destination.join("textures").join(format!("dlch_{name}"))
            } else {
                destination.join(relative)
            };
            directory(target.parent().context("DLC file has no parent")?)?;
            ensure!(
                !target.is_symlink(),
                "DLC destination file is a symlink: {}",
                target.display()
            );
            let bytes = std::fs::read(&path)?;
            let bytes = if path.extension().is_some_and(|e| e == "glb") {
                glb_aliases(&bytes, &names).with_context(|| path.display().to_string())?
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".skins.ron"))
            {
                let mut skins: manifest::SoldierSkins = ron::de::from_bytes(&bytes)?;
                for name in skins
                    .groups
                    .values_mut()
                    .flatten()
                    .chain(&mut skins.overrides)
                {
                    alias(name, &names);
                }
                ron::ser::to_string_pretty(&skins, ron::ser::PrettyConfig::default())?.into_bytes()
            } else {
                bytes
            };
            std::fs::write(&target, bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aliases_preserve_retail_bytes_glb_bin_and_nontexture_references() {
        let stage = Staging::new().unwrap();
        let dest = Staging::new().unwrap();
        for root in [stage.path(), dest.path()] {
            std::fs::create_dir_all(root.join("textures")).unwrap();
            std::fs::create_dir_all(root.join("soldiers")).unwrap();
        }
        std::fs::write(
            dest.path().join("textures/redlightsabre.ktx2"),
            b"retail pixels",
        )
        .unwrap();
        for name in [
            "redlightsabre",
            "redlightsabre.tier2",
            "redlightsabre_dtl",
            "body",
            "notexture",
        ] {
            std::fs::write(
                stage.path().join(format!("textures/{name}.ktx2")),
                b"DLC pixels",
            )
            .unwrap();
        }
        let bin = [1, 2, 3, 4, 128, 64, 0, 0];
        let doc = json!({"materials":[{"name":"redlightsabre"},{"name":"retail-only"}], "nodes":[{"name":"redlightsabre"}]});
        let glb = crate::glb::pack_glb(&serde_json::to_vec(&doc).unwrap(), &bin);
        std::fs::write(stage.path().join("soldiers/hero.glb"), glb).unwrap();
        let skins = manifest::SoldierSkins {
            groups: BTreeMap::from([("override_texture".into(), vec!["redlightsabre".into()])]),
            overrides: vec!["body".into()],
        };
        crate::write_ron(&stage.path().join("soldiers/hero.skins.ron"), &skins).unwrap();
        std::fs::write(
            stage.path().join("soldiers/hero.motion.ron"),
            b"motion stays exact",
        )
        .unwrap();
        let mut classes = vec![BakedClass {
            name: "hero".into(),
            kind: "entc".into(),
            base: "soldier".into(),
            props: [
                ("LightSaberTexture", "redlightsabre"),
                ("GeometryName", "redlightsabre"),
                ("IconTexture", "body"),
                ("LightSaberGlowTexture", "notexture"),
            ]
            .into_iter()
            .map(|(k, v)| (mungefs::fnv1a_lowercase(k), v.into()))
            .collect(),
        }];
        let mut result = HeroBake {
            textures: vec!["redlightsabre".into(), "body".into()],
            ..Default::default()
        };
        install(stage.path(), dest.path(), &mut classes, &mut result).unwrap();
        assert_eq!(
            std::fs::read(dest.path().join("textures/redlightsabre.ktx2")).unwrap(),
            b"retail pixels"
        );
        for name in [
            "redlightsabre",
            "redlightsabre.tier2",
            "redlightsabre_dtl",
            "body",
            "notexture",
        ] {
            assert_eq!(
                std::fs::read(dest.path().join(format!("textures/dlch_{name}.ktx2"))).unwrap(),
                b"DLC pixels"
            );
        }
        let bytes = std::fs::read(dest.path().join("soldiers/hero.glb")).unwrap();
        let json_end = 20 + u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let rewritten: Value = serde_json::from_slice(&bytes[20..json_end]).unwrap();
        assert_eq!(rewritten["materials"][0]["name"], "dlch_redlightsabre");
        assert_eq!(rewritten["materials"][1]["name"], "retail-only");
        assert_eq!(rewritten["nodes"][0]["name"], "redlightsabre");
        assert_eq!(&bytes[json_end + 8..], &bin);
        assert_eq!(
            classes[0]
                .props
                .iter()
                .map(|p| p.1.as_str())
                .collect::<Vec<_>>(),
            [
                "dlch_redlightsabre",
                "redlightsabre",
                "dlch_body",
                "notexture"
            ]
        );
        let skins: manifest::SoldierSkins = ron::de::from_bytes(
            &std::fs::read(dest.path().join("soldiers/hero.skins.ron")).unwrap(),
        )
        .unwrap();
        assert_eq!(skins.groups["override_texture"], ["dlch_redlightsabre"]);
        assert_eq!(skins.overrides, ["dlch_body"]);
        assert_eq!(result.textures, ["dlch_body", "dlch_redlightsabre"]);
        assert_eq!(
            std::fs::read(dest.path().join("soldiers/hero.motion.ron")).unwrap(),
            b"motion stays exact"
        );
    }

    #[test]
    fn malformed_glb_is_rejected_without_touching_binary_payload() {
        let names = BTreeMap::new();
        let good = crate::glb::pack_glb(b"{}", &[1, 2, 3, 4]);
        for end in [0, 4, 12, 20, 24, good.len() - 1] {
            assert!(glb_aliases(&good[..end], &names).is_err());
        }
        let mut bad = good.clone();
        bad[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(glb_aliases(&bad, &names).is_err());
        let mut bad = good;
        let json_end = 20 + u32::from_le_bytes(bad[12..16].try_into().unwrap()) as usize;
        bad[json_end..json_end + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(glb_aliases(&bad, &names).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_destination_directories_and_files() {
        use std::os::unix::fs::symlink;
        let stage = Staging::new().unwrap();
        let dest = Staging::new().unwrap();
        let protected = Staging::new().unwrap();
        std::fs::create_dir(stage.path().join("textures")).unwrap();
        std::fs::write(stage.path().join("textures/shared.ktx2"), b"DLC").unwrap();
        let mut result = HeroBake::default();
        symlink(protected.path(), dest.path().join("textures")).unwrap();
        assert!(install(stage.path(), dest.path(), &mut [], &mut result).is_err());
        assert_eq!(std::fs::read_dir(protected.path()).unwrap().count(), 0);
        std::fs::remove_file(dest.path().join("textures")).unwrap();
        std::fs::create_dir(dest.path().join("textures")).unwrap();
        let target = protected.path().join("keep");
        std::fs::write(&target, b"retail").unwrap();
        symlink(&target, dest.path().join("textures/dlch_shared.ktx2")).unwrap();
        assert!(install(stage.path(), dest.path(), &mut [], &mut result).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"retail");
    }
}
