//! Bake a loading-screen LVL (LOAD/<MAP>.LVL; a BF2 one holds a pool of stills
//! instead, `LoadStyle::Pool`). BF1's is the classic zoom sequence
//! galactic map → locale → landsat → level (textures suffixed _0.._3) plus
//! the planet globe texture — to KTX2 + a RON sequence the game plays
//! between the shell and the battle (PLAN §Presentation: BF1 loading screens
//! + beeps; 4:3 art presents PILLARBOXED).

use anyhow::{anyhow, Context, Result};
use std::path::Path;

/// The RON's struct and its style enum live in the `manifest` crate: ONE
/// definition, deriving `Serialize` for this side and `Deserialize` for
/// `game::assets::load_loadscreen`, which reads the file back and cannot
/// depend on this crate. Before this they were hand-kept twins.
pub use manifest::{LoadScreen, LoadStyle};

/// Writes `<outdir>/load/<name>.ktx2` + `<outdir>/load/load.ron`.
pub fn bake_loadscreen(lvl_path: &str, outdir: &str) -> Result<LoadScreen> {
    let mut buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?;
    if buf.len() >= 8 && &buf[0..4] == b"ucfb" {
        let total =
            8usize.saturating_add(u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize);
        if total > buf.len() && total - buf.len() <= 65536 {
            buf.resize(total, 0);
        }
    }
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
        let mips: Vec<Vec<u8>> = (0..t.mips.len()).map(|m| t.rgba8(m)).collect();
        let bytes = crate::ktx2w::write_rgba8(t.info.width, t.info.height, &mips);
        std::fs::write(out.join(format!("{}.ktx2", t.name)), bytes)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The writer and the reader (`game::assets::load_loadscreen`) are ONE
    /// struct now (queue item 13), so every `load.ron` the bake shipped must
    /// survive a parse and a re-serialize UNCHANGED, byte for byte, through
    /// `crate::write_ron`'s config. Data-gated on `engine/assets_baked`.
    #[test]
    fn every_shipped_load_ron_re_serializes_byte_identically() {
        let root = Path::new(crate::BAKED_ROOT);
        if !root.exists() {
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
