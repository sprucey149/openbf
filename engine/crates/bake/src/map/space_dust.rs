use anyhow::{anyhow, Result};
use mungefs::{config::parse_tree, Chunk};
use std::path::Path;

pub(super) fn write(chunks: &[Chunk<'_>], out: &Path) -> Result<()> {
    let path = out.join("space_dust.ron");
    for chunk in chunks.iter().filter(|c| &c.tag == b"fx__") {
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
                return Ok(());
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
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
