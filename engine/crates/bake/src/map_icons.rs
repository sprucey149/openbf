//! Side-LVL minimap sheets. Class MapTexture is a HUD binding independent
//! of the vehicle's drawn model (analysis/re-hud-map.md §2.1, §3).
//!
//! Stock BF1 names `speeder_icon` on the console `rep_fly_vtrans` class and
//! stock BF2 names `protoshuttle_icon` on `imp_fly_trooptrans`. Those names
//! have no corresponding sheet in the checked SIDE or common LVLs, so they
//! produce no output here.

use anyhow::{Context, Result};
use mungefs::{entity::EntityClass, texture::Texture};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Write the side-owned textures named by MapTexture to the shared HUD
/// texture directory for this game. Earlier LVLs and chunks win duplicates.
/// Names supplied by common or map LVLs resolve in their usual directories.
pub fn bake(side_lvls: &[PathBuf], out: &Path) -> Result<Vec<String>> {
    let inputs = side_lvls
        .iter()
        .map(crate::gameplay::read)
        .collect::<Result<Vec<_>>>()?;
    let chunks = inputs
        .iter()
        .zip(side_lvls)
        .map(|(bytes, path)| {
            mungefs::open(bytes)
                .and_then(mungefs::content_chunks)
                .with_context(|| path.display().to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    let map_texture = mungefs::fnv1a_lowercase("MapTexture");
    let mut wanted = BTreeSet::new();
    for chunk in chunks.iter().flatten() {
        if !crate::classes::CLASS_TAGS.contains(&&chunk.tag) {
            continue;
        }
        for prop in EntityClass::parse(chunk)?.props {
            let name = prop.value.as_str();
            if prop.key_hash == map_texture
                && !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            {
                wanted.insert(name.to_ascii_lowercase());
            }
        }
    }

    let textures = out.join("textures");
    std::fs::create_dir_all(&textures)?;
    let mut baked = Vec::new();
    for chunk in chunks.iter().flatten().filter(|c| &c.tag == b"tex_") {
        let Some(name) = chunk.child_str(b"NAME").map(str::to_ascii_lowercase) else {
            continue;
        };
        if !wanted.remove(&name) {
            continue;
        }
        let texture = Texture::parse(chunk).with_context(|| format!("MapTexture {name}"))?;
        crate::detail::write_ktx2(&textures, &format!("{name}.ktx2"), &texture)?;
        baked.push(name);
    }
    baked.sort();
    Ok(baked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out.resize(out.len().next_multiple_of(4), 0);
        out
    }

    fn texture(name: &[u8], alpha: u8) -> Vec<u8> {
        let info = [1, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 1, 0];
        let rgba = [0, alpha, 30, 20, 10]; // RLE literal, stored in ABGR order.
        chunk(
            b"tex_",
            &[
                chunk(b"NAME", name),
                chunk(b"INFO", &info),
                chunk(b"BODY", &rgba),
            ]
            .concat(),
        )
    }

    #[test]
    fn bakes_class_references_across_sides_with_authored_alpha() {
        let root = std::env::temp_dir().join(format!("openbf-map-icons-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let first = root.join("first.lvl");
        let second = root.join("second.lvl");
        let map_property = |name: &[u8]| {
            let mut prop = mungefs::fnv1a_lowercase("MapTexture")
                .to_le_bytes()
                .to_vec();
            prop.extend_from_slice(name);
            chunk(b"PROP", &prop)
        };
        let class = chunk(
            b"entc",
            &[
                chunk(b"TYPE", b"test_vehicle\0"),
                map_property(b"Flight-Icon\0"),
                map_property(b"common_icon\0"),
                map_property(b"../escape\0"),
            ]
            .concat(),
        );
        std::fs::write(
            &first,
            chunk(
                b"ucfb",
                &[
                    texture(b"flight-icon\0", 64),
                    texture(b"unreferenced\0", 255),
                    texture(b"../escape\0", 128),
                ]
                .concat(),
            ),
        )
        .unwrap();
        std::fs::write(
            &second,
            chunk(b"ucfb", &[class, texture(b"FLIGHT-ICON\0", 255)].concat()),
        )
        .unwrap();

        let out = root.join("side");
        let baked = bake(&[first, second], &out).unwrap();
        assert_eq!(baked, ["flight-icon"]);
        let dir = out.join("textures");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert!(!out.join("escape.ktx2").exists());
        let ktx = ktx2::Reader::new(std::fs::read(dir.join("flight-icon.ktx2")).unwrap()).unwrap();
        assert_eq!(ktx.levels().next().unwrap().data, &[10, 20, 30, 64]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
