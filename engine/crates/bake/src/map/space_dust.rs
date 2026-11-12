use anyhow::{anyhow, Result};
use mungefs::{config::parse_tree, Chunk};
use std::path::Path;

pub(super) fn write(chunks: &[Chunk<'_>], out: &Path) -> Result<()> {
    let path = out.join("space_dust.ron");
    'effects: for chunk in chunks.iter().filter(|c| &c.tag == b"fx__") {
        for node in parse_tree(chunk).map_err(|e| anyhow!("{e}"))? {
            if node.hash != 0x6e6e8d54
                || !node
                    .string
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case("SpaceDust"))
            {
                continue;
            }
            let enabled = node.find(0xaf8bb8ce).and_then(|n| n.f32(0)).unwrap_or(0.0) != 0.0;
            if !enabled {
                break 'effects;
            }
            let texture = node
                .find(0x3c6468f4)
                .and_then(|n| n.string.clone())
                .unwrap_or_default();
            let values: Vec<(u32, f32)> = node
                .children
                .iter()
                .filter_map(|n| n.f32(0).map(|v| (n.hash, v)))
                .collect();
            std::fs::write(path, ron::ser::to_string(&(texture, values))?)?;
            return Ok(());
        }
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn build_chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        bytes.resize(bytes.len().next_multiple_of(4), 0);
        bytes
    }

    #[test]
    fn disabled_space_dust_clears_the_previous_bake() {
        let root = std::env::temp_dir().join(format!("openbf-space-dust-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("space_dust.ron");
        let mut effect = Vec::new();
        effect.extend(0x6e6e8d54u32.to_le_bytes());
        effect.push(1);
        effect.extend(4u32.to_le_bytes());
        effect.extend(10u32.to_le_bytes());
        effect.extend(b"SpaceDust\0");
        let mut payload = build_chunk(b"DATA", &effect);
        let mut enabled = Vec::new();
        enabled.extend(0xaf8bb8ceu32.to_le_bytes());
        enabled.push(1);
        enabled.extend(1.0f32.to_le_bytes());
        payload.extend(build_chunk(b"SCOP", &build_chunk(b"DATA", &enabled)));

        let bake = |payload| {
            write(
                &[Chunk {
                    tag: *b"fx__",
                    payload,
                    offset: 0,
                }],
                &root,
            )
        };
        bake(&payload).unwrap();
        assert!(path.is_file(), "enabled effect is baked");

        enabled[5..].copy_from_slice(&0.0f32.to_le_bytes());
        let mut payload = build_chunk(b"DATA", &effect);
        payload.extend(build_chunk(b"SCOP", &build_chunk(b"DATA", &enabled)));
        bake(&payload).unwrap();
        let stale = path.exists();
        std::fs::remove_dir_all(root).unwrap();
        assert!(!stale, "disabled effect must remove the previous bake");
    }
}
