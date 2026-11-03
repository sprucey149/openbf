//! Keep the player's far texture bound when a public sky correction renames it.
//! The original bytes are supplied only after gameplay's source fingerprints
//! pass. No PS2 stream enters this path, and no existing texture is replaced.

use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;

type Alias = (String, String);

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 200
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

pub(super) fn far_alias(
    original: Option<&[u8]>,
    requested: Option<&str>,
    available: &[&str],
) -> Result<Option<Alias>> {
    let (Some(original), Some(requested)) = (original, requested) else {
        return Ok(None);
    };
    if available.iter().any(|n| n.eq_ignore_ascii_case(requested)) {
        return Ok(None);
    }
    let chunks = mungefs::content_chunks(mungefs::open(original).map_err(|e| anyhow!("{e}"))?)
        .map_err(|e| anyhow!("{e}"))?;
    let mut native = None::<String>;
    for chunk in chunks.iter().filter(|c| &c.tag == b"sky_") {
        let sky = mungefs::sky::Sky::parse(chunk).map_err(|e| anyhow!("{e}"))?;
        let name = sky
            .dome
            .and_then(|d| d.low_res_terrain)
            .and_then(|l| l.texture);
        if let Some(name) = name {
            if native
                .as_ref()
                .is_some_and(|previous| !previous.eq_ignore_ascii_case(&name))
            {
                bail!("ambiguous native far texture binding");
            }
            native = Some(name);
        }
    }
    let Some(native) = native else {
        return Ok(None);
    };
    let Some(available) = available.iter().find(|n| n.eq_ignore_ascii_case(&native)) else {
        return Ok(None); // The alignment gate reports a genuinely absent binding.
    };
    if !safe_name(requested) || !safe_name(available) {
        bail!("unsafe far texture alias name");
    }
    Ok(Some((
        requested.to_ascii_lowercase(),
        (*available).to_owned(),
    )))
}

pub(super) fn install(
    alias: &Alias,
    dir: &Path,
    textures: &mut Vec<String>,
    tiers: &mut Vec<(String, String)>,
    tier2: &mut Vec<(String, String)>,
    transparent: &mut Vec<String>,
) -> Result<()> {
    let (target, native) = alias;
    if !safe_name(target) || !safe_name(native) || target.eq_ignore_ascii_case(native) {
        bail!("invalid far texture alias");
    }
    if textures.iter().any(|n| n.eq_ignore_ascii_case(target)) {
        bail!("refusing to replace an existing far texture");
    }
    if !textures.iter().any(|n| n.eq_ignore_ascii_case(native)) {
        bail!("native far texture was not emitted");
    }
    for suffix in ["ktx2", "tier2.ktx2", "dtl.ktx2"] {
        let source = dir.join(format!("{native}.{suffix}"));
        if suffix != "ktx2" && !source.is_file() {
            crate::remove_optional_file(&dir.join(format!("{target}.{suffix}")))?;
            continue;
        }
        std::fs::copy(&source, dir.join(format!("{target}.{suffix}")))
            .with_context(|| format!("far texture alias from {}", source.display()))?;
    }
    let tier = tiers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(native))
        .map(|(_, source)| source.clone());
    if let Some(source) = tier {
        tiers.push((target.clone(), source));
    }
    if dir.join(format!("{target}.tier2.ktx2")).is_file() {
        tier2.push((target.clone(), format!("{target}.tier2.ktx2")));
    }
    if transparent.iter().any(|n| n.eq_ignore_ascii_case(native)) {
        transparent.push(target.clone());
    }
    textures.push(target.clone());
    eprintln!("render texture alias: {target} <- {native}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend((payload.len() as u32).to_le_bytes());
        out.extend(payload);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out
    }

    fn node(name: &str, text: Option<&str>, children: &[u8]) -> Vec<u8> {
        let mut data = mungefs::fnv1a_lowercase(name).to_le_bytes().to_vec();
        data.push(u8::from(text.is_some()));
        if let Some(text) = text {
            data.extend(4u32.to_le_bytes());
            data.extend((text.len() as u32 + 1).to_le_bytes());
            data.extend(text.as_bytes());
            data.push(0);
        } else {
            data.extend(0u32.to_le_bytes());
        }
        let mut out = chunk(b"DATA", &data);
        if !children.is_empty() {
            out.extend(chunk(b"SCOP", children));
        }
        out
    }

    fn sky(texture: &str) -> Vec<u8> {
        chunk(
            b"sky_",
            &node(
                "DomeInfo",
                None,
                &node("LowResTerrain", None, &node("Texture", Some(texture), &[])),
            ),
        )
    }

    #[test]
    fn aliases_only_missing_corrected_bindings_from_the_native_sky() {
        let raw = chunk(b"ucfb", &sky("kas1"));
        let alias = far_alias(Some(&raw), Some("kashyyyk1"), &["Kas1"]).unwrap();
        assert_eq!(alias, Some(("kashyyyk1".into(), "Kas1".into())));
        assert!(far_alias(None, Some("kashyyyk1"), &["kas1"])
            .unwrap()
            .is_none());
        assert!(
            far_alias(Some(&raw), Some("kashyyyk1"), &["kashyyyk1", "kas1"])
                .unwrap()
                .is_none()
        );
        assert!(far_alias(Some(&raw), Some("kashyyyk1"), &[])
            .unwrap()
            .is_none());
        let ambiguous = chunk(b"ucfb", &[sky("kas1"), sky("other")].concat());
        assert!(far_alias(Some(&ambiguous), Some("kashyyyk1"), &["kas1", "other"]).is_err());
    }

    #[test]
    fn installs_both_tiers_and_detail_without_replacing_native_art() {
        let temp = std::env::temp_dir().join(format!("openbf-far-alias-{}", std::process::id()));
        std::fs::create_dir(&temp).unwrap();
        let dir = temp.as_path();
        for suffix in ["ktx2", "tier2.ktx2", "dtl.ktx2"] {
            std::fs::write(dir.join(format!("kas1.{suffix}")), suffix).unwrap();
        }
        let mut textures = vec!["kas1".into()];
        let mut tiers = vec![("kas1".into(), "pc".into())];
        let mut tier2 = vec![("kas1".into(), "kas1.tier2.ktx2".into())];
        let mut transparent = vec!["kas1".into()];
        let alias = ("kashyyyk1".into(), "kas1".into());
        install(
            &alias,
            dir,
            &mut textures,
            &mut tiers,
            &mut tier2,
            &mut transparent,
        )
        .unwrap();
        for suffix in ["ktx2", "tier2.ktx2", "dtl.ktx2"] {
            assert_eq!(
                std::fs::read(dir.join(format!("kashyyyk1.{suffix}"))).unwrap(),
                suffix.as_bytes()
            );
            assert_eq!(
                std::fs::read(dir.join(format!("kas1.{suffix}"))).unwrap(),
                suffix.as_bytes()
            );
        }
        assert_eq!(textures, ["kas1", "kashyyyk1"]);
        assert!(install(
            &alias,
            dir,
            &mut textures,
            &mut tiers,
            &mut tier2,
            &mut transparent
        )
        .is_err());
        // A later bake without the optional donor must not keep a stale alias
        // override: runtime selects tier 2 by file existence.
        for suffix in ["tier2.ktx2", "dtl.ktx2"] {
            std::fs::remove_file(dir.join(format!("kas1.{suffix}"))).unwrap();
        }
        let mut textures = vec!["kas1".into()];
        let mut tiers = Vec::new();
        let mut tier2 = Vec::new();
        let mut transparent = Vec::new();
        install(
            &alias,
            dir,
            &mut textures,
            &mut tiers,
            &mut tier2,
            &mut transparent,
        )
        .unwrap();
        assert!(!dir.join("kashyyyk1.tier2.ktx2").exists());
        assert!(!dir.join("kashyyyk1.dtl.ktx2").exists());
        assert!(tier2.is_empty());
        std::fs::remove_dir_all(temp).unwrap();
    }
}
