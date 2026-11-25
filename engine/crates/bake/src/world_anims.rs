//! `bake world-anims <LVL> <map dir>`: every `wrld`'s animations and groups
//! (`mungefs::world_anim`) into `<map dir>/world_anims.ron`
//! (`manifest::world_anim::FILE`), base world and mode layers alike. `bake
//! map` writes the same file; this subcommand adds it to a bake made before
//! the file existed without baking the whole map again.

use anyhow::{anyhow, Context, Result};
use manifest::world_anim::{WorldAnim, WorldAnimGroup, WorldAnimKey, WorldAnims};
use std::path::Path;

fn key(k: &mungefs::world_anim::Key) -> WorldAnimKey {
    WorldAnimKey {
        time: k.time,
        value: k.value,
        interp: k.interp,
        tangent_out: k.tangent_out,
        tangent_in: k.tangent_in,
    }
}

/// The world animations of the LVL at `lvl_path`.
pub fn read(lvl_path: &str) -> Result<WorldAnims> {
    let buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_owned())?;
    read_bytes(&buf).with_context(|| lvl_path.to_owned())
}

pub(crate) fn read_bytes(buf: &[u8]) -> Result<WorldAnims> {
    let inner = mungefs::open(buf).map_err(|e| anyhow!("{e}"))?;
    let mut out = WorldAnims::default();
    for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
        if &c.tag != b"wrld" {
            continue;
        }
        let world = c.child_string(b"NAME").unwrap_or_default();
        let parsed = mungefs::world_anim::parse(&c);
        out.anims
            .extend(parsed.anims.into_iter().map(|a| WorldAnim {
                world: world.clone(),
                name: a.name,
                duration: a.duration,
                looping: a.looping,
                local: a.local,
                pos: a.pos.iter().map(key).collect(),
                rot: a.rot.iter().map(key).collect(),
            }));
        out.groups
            .extend(parsed.groups.into_iter().map(|g| WorldAnimGroup {
                world: world.clone(),
                name: g.name,
                play: g.play,
                restart_on_state: g.restart_on_state,
                no_hierarchy: g.no_hierarchy,
                entries: g.entries,
            }));
    }
    Ok(out)
}

/// Write `outdir`'s world animations file, or remove a stale one when the
/// LVL has none. Returns how many groups it holds.
pub fn bake(lvl_path: &str, outdir: &str) -> Result<usize> {
    let anims = read(lvl_path)?;
    write(Path::new(outdir), &anims)
}

pub(crate) fn write(out: &Path, anims: &WorldAnims) -> Result<usize> {
    let path = out.join(manifest::world_anim::FILE);
    if anims.groups.is_empty() && anims.anims.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path).with_context(|| path.display().to_string())?;
        }
        return Ok(0);
    }
    crate::write_ron(&path, anims)?;
    Ok(anims.groups.len())
}
