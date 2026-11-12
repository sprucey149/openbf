//! Bake a loading-screen LVL (LOAD/<MAP>.LVL; a BF2 one holds a pool of stills
//! instead, `LoadStyle::Pool`). BF1's is the classic zoom sequence
//! galactic map → locale → landsat → level (textures suffixed _0.._3) plus
//! the planet globe texture — to KTX2 + a RON sequence the game plays
//! between the shell and the battle (PLAN §Presentation: BF1 loading screens
//! + beeps; 4:3 art presents PILLARBOXED).

use anyhow::{anyhow, bail, Context, Result};
use manifest::{LoadingLayout, LoadingNoise, LoadingTarget};
use mungefs::{config::ConfigNode, Chunk};
use std::path::Path;

/// The RON's struct and its style enum live in the `manifest` crate: ONE
/// definition, deriving `Serialize` for this side and `Deserialize` for
/// `game::assets::load_loadscreen`, which reads the file back and cannot
/// depend on this crate. Before this they were hand-kept twins.
pub use manifest::{LoadScreen, LoadStyle};

/// Writes `<outdir>/load/<name>.ktx2` + `<outdir>/load/load.ron`.
pub fn bake_loadscreen(lvl_path: &str, outdir: &str) -> Result<LoadScreen> {
    let buf = read_lvl(lvl_path)?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let out = Path::new(outdir).join("load");
    std::fs::create_dir_all(&out)?;
    let mut named: Vec<(u32, String)> = Vec::new();
    let mut extras = Vec::new();
    // BF1 keeps the textures at the top level; BF2 wraps each in `lvl_`.
    let content = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    for c in content.into_iter() {
        if &c.tag != b"tex_" {
            continue;
        }
        let t = mungefs::texture::Texture::parse(&c)
            .map_err(|e| anyhow!("tex_ 0x{:X}: {e}", c.offset))?;
        write_texture(&out, &t)?;
        // Sequence order = trailing _<n>; unsuffixed textures are extras.
        match t
            .name
            .rsplit_once('_')
            .and_then(|(_, n)| n.parse::<u32>().ok())
        {
            Some(n) => named.push((n, t.name.clone())),
            None => extras.push(t.name.clone()),
        }
    }
    named.sort();
    let sequence: Vec<String> = named.into_iter().map(|(_, n)| n).collect();
    // BF1 zoom steps are `load_*`; anything else numbered is a BF2 still pool.
    // A lone unnumbered `loadscreen` (BF2 COMMON.LVL) is the pool itself.
    let style = if sequence.iter().any(|n| n.starts_with("load_")) {
        LoadStyle::Sequence
    } else {
        LoadStyle::Pool
    };
    let (sequence, extras) = if sequence.is_empty() && extras.len() == 1 {
        (extras, Vec::new())
    } else {
        (sequence, extras)
    };
    let ls = LoadScreen {
        sequence,
        extras,
        style,
    };
    crate::write_ron(&out.join("load.ron"), &ls)?;
    Ok(ls)
}

fn read_lvl(lvl_path: &str) -> Result<Vec<u8>> {
    let mut buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?;
    if buf.len() >= 8 && &buf[0..4] == b"ucfb" {
        let total =
            8usize.saturating_add(u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize);
        if total > buf.len() && total - buf.len() <= 65536 {
            buf.resize(total, 0);
        }
    }
    Ok(buf)
}

fn write_texture(dir: &Path, t: &mungefs::texture::Texture) -> Result<()> {
    let mips: Vec<Vec<u8>> = (0..t.mips.len()).map(|m| t.rgba8(m)).collect();
    let bytes = crate::ktx2w::write_rgba8(t.info.width, t.info.height, &mips);
    std::fs::write(dir.join(format!("{}.ktx2", t.name)), bytes)?;
    Ok(())
}

/// Bake the common config and only the sprites used by the zoom presentation.
pub fn bake_loading_layout(lvl_path: &str, outdir: &str) -> Result<()> {
    let buf = read_lvl(lvl_path)?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    let layout = parse_layout(
        chunks
            .iter()
            .find(|c| &c.tag == b"load")
            .context("BF1 load config")?,
    )?;
    let dir = Path::new(outdir).join("load");
    std::fs::create_dir_all(&dir)?;
    let mut wanted = std::collections::BTreeSet::from([
        layout.disk.as_str(),
        layout.led.as_str(),
        layout.scanline.as_str(),
        layout.edges[0].as_str(),
        layout.edges[1].as_str(),
        layout.edges[2].as_str(),
    ]);
    wanted.extend(layout.noise.frames.iter().map(String::as_str));
    for chunk in chunks.iter().filter(|c| &c.tag == b"tex_") {
        let texture = mungefs::texture::Texture::parse(chunk)
            .map_err(|e| anyhow!("tex_ 0x{:X}: {e}", chunk.offset))?;
        if wanted.remove(texture.name.as_str()) {
            write_texture(&dir, &texture)?;
        }
    }
    if !wanted.is_empty() {
        bail!("missing BF1 loading sprites: {wanted:?}");
    }
    crate::write_ron(&dir.join("layout.ron"), &layout)
}

/// BF1 PS2 FUN_002c8aa0 reads the shared settings and map-local zoom boxes.
fn parse_layout(chunk: &Chunk<'_>) -> Result<LoadingLayout> {
    let nodes = mungefs::config::parse_tree(chunk).map_err(|e| anyhow!("{e}"))?;
    let common = nodes
        .iter()
        .find(|n| n.hash == 0x8689_c861)
        .context("shared loading settings")?;
    let timing = common.find(0xb32f_0201).context("loading timing")?;
    let seconds = |i| {
        timing
            .f32(i)
            .filter(|v| (0.1..=15.0).contains(v))
            .context("invalid loading duration")
    };
    let sprite = |node: &ConfigNode, i| {
        node.str_arg(i)
            .filter(|name| !name.is_empty())
            .map(str::to_ascii_lowercase)
            .context("missing loading sprite")
    };
    let edges = common.find(0x6ae7_b95f).context("tracking sprites")?;
    let noise = common.find(0xa363_3da9).context("loading noise")?;
    let prefix = sprite(noise, 0)?;
    let frames = noise
        .f32(1)
        .filter(|n| (1.0..=16.0).contains(n))
        .context("noise frames")? as usize;
    let noise = LoadingNoise {
        frames: (0..frames).map(|i| format!("{prefix}{i}")).collect(),
        frame_rate: noise.f32(2).context("noise frame rate")?,
        uv_start: [noise.f32(3).unwrap_or(1.0), noise.f32(4).unwrap_or(1.0)],
    };
    let mut layout = LoadingLayout {
        tracking_seconds: seconds(0)?,
        zoom_seconds: seconds(1)?,
        disk: sprite(common.find(0x9880_e7ac).context("loading disk")?, 0)?,
        led: sprite(common.find(0x9d5b_100a).context("loading dial lights")?, 0)?,
        edges: [sprite(edges, 0)?, sprite(edges, 1)?, sprite(edges, 2)?],
        scanline: sprite(common.find(0xe3dd_2365).context("loading scanline")?, 0)?,
        noise,
        maps: Default::default(),
    };
    for map in nodes.iter().filter(|n| n.hash == 0x37a3_e893) {
        let Some(key) = map.str_arg(0) else { continue };
        let mut targets = Vec::new();
        for target in map.find_all(0xd7b3_7b83) {
            let index = target.f32(0).context("loading image index")?;
            let rect = [2, 3, 4, 5].map(|i| target.f32(i).unwrap_or(f32::NAN));
            if !index.is_finite() || rect.iter().any(|v| !v.is_finite() || *v < 0.0) {
                bail!("invalid loading target for {key}");
            }
            targets.push((
                index,
                LoadingTarget {
                    image: sprite(target, 1)?,
                    rect,
                },
            ));
        }
        targets.sort_by(|a, b| a.0.total_cmp(&b.0));
        layout.maps.insert(
            key.to_ascii_lowercase(),
            targets.into_iter().map(|(_, t)| t).collect(),
        );
    }
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bf1_common_layout_reads_the_authored_choreography() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/LOAD/LOAD.LVL"
        );
        if !Path::new(path).exists() {
            eprintln!("SKIP: extracted BF1 LOAD/LOAD.LVL not present");
            return;
        }
        let bytes = read_lvl(path).unwrap();
        let chunks = mungefs::content_chunks(mungefs::open(&bytes).unwrap()).unwrap();
        let layout = parse_layout(chunks.iter().find(|c| &c.tag == b"load").unwrap()).unwrap();
        assert_eq!((layout.tracking_seconds, layout.zoom_seconds), (3.5, 1.0));
        assert_eq!(layout.maps.len(), 16);
        assert_eq!(layout.noise.frames.len(), 6);
        for targets in layout.maps.values() {
            assert_eq!(targets.len(), 4);
            assert_eq!(targets[0].image, "load_galacticmap_0");
            assert!(targets[..3]
                .iter()
                .all(|t| t.rect[2] > 0.0 && t.rect[3] > 0.0));
        }
        assert_ne!(layout.maps["bes1"][2].rect, layout.maps["bes2"][2].rect);
    }

    /// The writer and the reader (`game::assets::load_loadscreen`) are ONE
    /// struct now (queue item 13), so every `load.ron` the bake shipped must
    /// survive a parse and a re-serialize UNCHANGED, byte for byte, through
    /// `crate::write_ron`'s config. Data-gated on `engine/assets_baked`.
    #[test]
    fn every_shipped_load_ron_re_serializes_byte_identically() {
        let root = Path::new(crate::BAKED_ROOT);
        if !crate::baked_root_has_data() {
            eprintln!("SKIP: assets_baked not present");
            return;
        }
        let mut checked = 0usize;
        let mut dirs: Vec<_> = std::fs::read_dir(root)
            .expect("assets_baked")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        dirs.sort();
        for d in dirs {
            let f = d.join("load/load.ron");
            if !f.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&f).expect("read");
            let parsed: LoadScreen =
                ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            let again =
                ron::ser::to_string_pretty(&parsed, ron::ser::PrettyConfig::default()).unwrap();
            assert_eq!(again, text, "{} re-serializes differently", f.display());
            checked += 1;
        }
        assert!(checked >= 30, "only {checked} load.ron found");
    }

    /// Data-gated Tier-1: BF1 LOAD/TAT1.LVL → 4-step sequence + planet globe.
    #[test]
    fn tat1_loadscreen_sequence() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/LOAD/TAT1.LVL"
        );
        if !Path::new(p).exists() {
            eprintln!("SKIP: extracted LOAD/TAT1.LVL not present");
            return;
        }
        let out =
            std::env::temp_dir().join(format!("openbf-loadscreen-tat1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let ls = bake_loadscreen(p, out.to_str().unwrap()).expect("bake");
        assert_eq!(
            ls.sequence,
            vec![
                "load_galacticmap_0",
                "load_locale_tatooine_1",
                "load_landsat_tatooine_2",
                "load_level_tatooine1_3"
            ]
        );
        assert_eq!(ls.extras, vec!["load_planet_tattoine"]);
        for n in ls.sequence.iter().chain(ls.extras.iter()) {
            assert!(out.join("load").join(format!("{n}.ktx2")).exists(), "{n}");
        }
        let back: LoadScreen =
            ron::from_str(&std::fs::read_to_string(out.join("load/load.ron")).unwrap()).unwrap();
        assert_eq!(back.sequence.len(), 4);
        let _ = std::fs::remove_dir_all(&out);
    }

    /// Data-gated Tier-1: BF2 LOAD/TAT2.LVL → 6 Mos Eisley stills (pool).
    #[test]
    fn bf2_tat2_loadscreen_pool() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/LOAD/TAT2.LVL"
        );
        if !std::path::Path::new(p).exists() {
            eprintln!("SKIP: BF2 LOAD/TAT2.LVL not present");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_bf2load_{}", std::process::id()));
        let ls = bake_loadscreen(p, dir.to_str().unwrap()).unwrap();
        assert_eq!(ls.style, LoadStyle::Pool);
        assert_eq!(
            ls.sequence,
            ["mos_1", "mos_2", "mos_3", "mos_4", "mos_5", "mos_6"]
        );
        assert!(ls.extras.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Data-gated Tier-1: BF2 LOAD/COMMON.LVL → the single generic `loadscreen`.
    #[test]
    fn bf2_common_loadscreen_pool() {
        let c = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/LOAD/COMMON.LVL"
        );
        if !std::path::Path::new(c).exists() {
            eprintln!("SKIP: BF2 LOAD/COMMON.LVL not present");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("openbf_bf2load_common_{}", std::process::id()));
        let common = bake_loadscreen(c, dir.to_str().unwrap()).unwrap();
        assert_eq!(common.style, LoadStyle::Pool);
        assert_eq!(common.sequence, ["loadscreen"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
