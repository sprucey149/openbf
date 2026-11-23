//! Apply a reviewed PS2 gameplay correction set before decoding a donor LVL.
//!
//! The original input is fingerprinted before any correction. PS2 content is
//! returned verbatim and never consults the correction tables. All consumers of
//! donor classes, worlds and collision use the same corrected byte stream;
//! terrain heights are applied after integer-height decoding by `terrain`.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

mod program;
mod rebuild;
mod terrain;
#[cfg(test)]
mod tests;
mod water;

pub use terrain::{apply_render_heights, apply_terrain};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash)]
pub(super) struct Locator {
    #[serde(default)]
    scope: Vec<u32>,
    name: String,
    #[serde(default)]
    ordinal: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Class {
    #[serde(flatten)]
    at: Locator,
    kind: String,
    base: String,
    properties: Vec<(u32, PropertyValue)>,
    #[serde(default)]
    preserve_hashes: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RemovedClass {
    #[serde(flatten)]
    at: Locator,
    kind: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct WorldRecord {
    kind: String,
    name: String,
    #[serde(default)]
    ordinal: usize,
    #[serde(default)]
    class: String,
    #[serde(default)]
    preserve_name_from: Option<usize>,
    #[serde(default)]
    region_setting: Option<String>,
    transform: [f32; 12],
    #[serde(default)]
    size: Option<[f32; 3]>,
    #[serde(default)]
    flags: u32,
    #[serde(default)]
    properties: Vec<(u32, PropertyValue)>,
    #[serde(default)]
    preserve_hashes: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct World {
    #[serde(flatten)]
    at: Locator,
    terrain_name: Option<String>,
    sky_name: Option<String>,
    records: Vec<WorldRecord>,
    #[serde(default)]
    patch_records: bool,
    #[serde(default)]
    record_order: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Primitive {
    name: String,
    parent: String,
    mask: u8,
    mask_own: u8,
    kind: u32,
    dimensions: [f32; 3],
    transform: [f32; 12],
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Primitives {
    #[serde(flatten)]
    at: Locator,
    records: Vec<Primitive>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Cutter {
    sphere: [f32; 4],
    planes: Vec<[f32; 4]>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Terrain {
    #[serde(flatten)]
    at: Locator,
    grid_size: u32,
    grid_unit: f32,
    height_scale: f32,
    #[serde(default)]
    height_samples: Vec<[f32; 3]>,
    #[serde(default)]
    height_fills: Vec<[f32; 5]>,
    #[serde(default)]
    render_height_fills: Vec<[f32; 5]>,
    #[serde(default)]
    render_grid_size: Option<u32>,
    #[serde(default)]
    boundaries: Vec<Vec<[f32; 2]>>,
    cutters: Option<Vec<Cutter>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum Argument {
    Number(f32),
    Name(String),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum PropertyValue {
    Text(String),
    Tokens(Vec<Argument>),
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ConfigNode {
    hash: u32,
    arguments: Vec<Argument>,
    children: Vec<ConfigNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Sky {
    #[serde(flatten)]
    at: Locator,
    nodes: Vec<ConfigNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ModelBounds {
    #[serde(flatten)]
    at: Locator,
    sphere: Option<[f32; 4]>,
    vis_box: [[f32; 3]; 2],
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Collision {
    #[serde(flatten)]
    at: Locator,
    program: usize,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Resource {
    #[serde(flatten)]
    at: Locator,
    kind: String,
    program: usize,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct File {
    file: String,
    sha256: String,
    #[serde(default)]
    classes: Vec<Class>,
    #[serde(default)]
    remove_classes: Vec<RemovedClass>,
    #[serde(default)]
    world: Vec<World>,
    #[serde(default)]
    primitives: Vec<Primitives>,
    #[serde(default)]
    remove_primitives: Vec<Locator>,
    #[serde(default)]
    terrain: Vec<Terrain>,
    #[serde(default)]
    remove_water: Vec<Locator>,
    #[serde(default)]
    water_mask: Option<water::WaterMaskCorrection>,
    #[serde(default)]
    sky: Vec<Sky>,
    #[serde(default)]
    models: Vec<ModelBounds>,
    #[serde(default)]
    collision: Vec<Collision>,
    #[serde(default)]
    remove_collision: Vec<Locator>,
    #[serde(default)]
    resources: Vec<Resource>,
    #[serde(default)]
    remove_resources: Vec<RemovedClass>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Table {
    schema: u32,
    reference: String,
    game: String,
    platform: String,
    presentation_hashes: Vec<u32>,
    files: Vec<File>,
    #[serde(skip)]
    programs: Option<program::Programs>,
}

fn tables() -> Result<&'static [Table]> {
    static TABLES: OnceLock<Result<Vec<Table>, String>> = OnceLock::new();
    let result = TABLES.get_or_init(|| {
        [
            (
                include_str!("../../manifest/data/gameplay/corrections-v1/bf1-pc.json"),
                include_str!("../../manifest/data/gameplay/corrections-v1/bf1-pc-programs.json"),
            ),
            (
                include_str!("../../manifest/data/gameplay/corrections-v1/bf1-xbox.json"),
                include_str!("../../manifest/data/gameplay/corrections-v1/bf1-xbox-programs.json"),
            ),
            (
                include_str!("../../manifest/data/gameplay/corrections-v1/bf2-pc.json"),
                include_str!("../../manifest/data/gameplay/corrections-v1/bf2-pc-programs.json"),
            ),
            (
                include_str!("../../manifest/data/gameplay/corrections-v1/bf2-pc-gog.json"),
                include_str!(
                    "../../manifest/data/gameplay/corrections-v1/bf2-pc-gog-programs.json"
                ),
            ),
            (
                include_str!("../../manifest/data/gameplay/corrections-v1/bf2-xbox.json"),
                include_str!("../../manifest/data/gameplay/corrections-v1/bf2-xbox-programs.json"),
            ),
        ]
        .into_iter()
        .map(|(text, programs)| {
            let mut table: Table = serde_json::from_str(text).map_err(|e| e.to_string())?;
            let programs: program::Programs =
                serde_json::from_str(programs).map_err(|e| e.to_string())?;
            if table.schema != 2
                || table.reference != "ps2-v1"
                || !matches!(table.game.as_str(), "bf1" | "bf2")
                || !matches!(table.platform.as_str(), "pc" | "xbox")
                || !programs.matches(&table)
            {
                return Err("unsupported gameplay correction revision".to_string());
            }
            table.programs = Some(programs);
            Ok(table)
        })
        .collect()
    });
    result
        .as_ref()
        .map(Vec::as_slice)
        .map_err(|e| anyhow!("gameplay corrections: {e}"))
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn platform(bytes: &[u8]) -> Result<Option<&'static str>> {
    let inner = mungefs::open(bytes).map_err(|e| anyhow!("{e}"))?;
    let mut found = None;
    for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
        if &c.tag != b"tex_" {
            continue;
        }
        let next = if c.child(b"FMT_").is_some() {
            "pc"
        } else if c.child(b"INFO").is_some_and(|i| i.payload.len() == 20)
            && c.child(b"BODY").is_some()
        {
            "xbox"
        } else {
            "ps2"
        };
        if found.is_some_and(|old| old != next) {
            bail!("mixed gameplay source texture encodings");
        }
        found = Some(next);
    }
    Ok(found)
}

fn source_root(path: &Path) -> Option<PathBuf> {
    path.ancestors().skip(1).find_map(|dir| {
        let core = crate::join_ci(dir, "CORE.LVL");
        let mission = crate::join_ci(dir, "MISSION.LVL");
        (core.is_file() && mission.is_file()).then(|| dir.to_owned())
    })
}

fn source_platform(path: &Path, bytes: &[u8]) -> Result<Option<&'static str>> {
    if let Some(p) = platform(bytes)? {
        return Ok(Some(p));
    }
    source_root(path)
        .map(|root| {
            let core = crate::join_ci(&root, "CORE.LVL");
            let bytes = std::fs::read(&core).with_context(|| core.display().to_string())?;
            platform(&bytes)
        })
        .transpose()
        .map(Option::flatten)
}

pub(crate) fn source_game(path: &Path) -> Result<Option<&'static str>> {
    let Some(root) = source_root(path) else {
        return Ok(None);
    };
    let mission = crate::join_ci(&root, "MISSION.LVL");
    let bytes = std::fs::read(&mission).with_context(|| mission.display().to_string())?;
    let inner = mungefs::open(&bytes).map_err(|e| anyhow!("{e}"))?;
    for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
        if &c.tag == b"scr_" {
            if let Some(body) = c.child(b"BODY") {
                if body.payload.starts_with(b"\x1bLua") {
                    return match body.payload.get(4) {
                        Some(0x40) => Ok(Some("bf1")),
                        Some(0x50) => Ok(Some("bf2")),
                        _ => bail!("unknown gameplay mission Lua generation"),
                    };
                }
            }
        }
    }
    bail!("cannot identify gameplay source game from mission contents")
}

/// The optional hero pack is an addon, not a game source: it answers to its own
/// checks in `crate::dlc`, and no correction table can name a pack in any format.
/// `dlc::bake` names its one LVL here so the reference disc's reads stay gated.
pub(crate) fn exempt_addon(path: &Path) {
    let mut exempt = ADDONS.lock().unwrap_or_else(|e| e.into_inner());
    if !exempt.iter().any(|p| p == path) {
        exempt.push(path.to_owned());
    }
}

static ADDONS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn is_addon(path: &Path) -> bool {
    ADDONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|p| p == path)
}

pub(super) fn correction(
    path: &Path,
    bytes: &[u8],
) -> Result<Option<(&'static Table, &'static File)>> {
    if is_addon(path) {
        return Ok(None);
    }
    let Some(platform) = source_platform(path, bytes)? else {
        return Ok(None);
    };
    if platform == "ps2" {
        return Ok(None);
    }
    let game = source_game(path)?;
    let hash = digest(bytes);
    let matches: Vec<_> = tables()?
        .iter()
        .filter(|t| t.platform == platform && game.is_none_or(|g| t.game == g))
        .flat_map(|t| {
            t.files
                .iter()
                .filter(|f| f.sha256 == hash)
                .map(move |f| (t, f))
        })
        .collect();
    let matches = same_bytes_by_path(path, matches);
    if matches.is_empty() {
        bail!("{}: unknown {platform} gameplay source fingerprint {hash}; no corrections were applied. Supply a supported unmodified source or regenerate and review its PS2 corrections", path.display());
    }
    // A matching map from a modified install is not a matching source revision,
    // and multiple official releases (e.g. Steam vs GOG v1.1) can share identical
    // map/mission bytes while differing in core.lvl. Filter candidates against
    // the owning install's core.lvl and mission.lvl fingerprints.
    if let Some(root) = source_root(path) {
        let core_path = crate::join_ci(&root, "core.lvl");
        let mission_path = crate::join_ci(&root, "mission.lvl");
        let core_hash = digest(
            &std::fs::read(&core_path).with_context(|| core_path.display().to_string())?,
        );
        let mission_hash = digest(
            &std::fs::read(&mission_path).with_context(|| mission_path.display().to_string())?,
        );
        let mut root_matched = Vec::new();
        for &(table, file) in &matches {
            let exp_core = table
                .files
                .iter()
                .find(|f| f.file.eq_ignore_ascii_case("core.lvl"))
                .context("correction table lacks a source root fingerprint")?;
            let exp_mission = table
                .files
                .iter()
                .find(|f| f.file.eq_ignore_ascii_case("mission.lvl"))
                .context("correction table lacks a source root fingerprint")?;
            if core_hash == exp_core.sha256 && mission_hash == exp_mission.sha256 {
                root_matched.push((table, file));
            }
        }
        return match root_matched.as_slice() {
            [one] => Ok(Some(*one)),
            [] => {
                let bad = if matches.iter().any(|(t, _)| {
                    t.files
                        .iter()
                        .any(|f| f.file.eq_ignore_ascii_case("core.lvl") && f.sha256 == core_hash)
                }) {
                    mission_path
                } else {
                    core_path
                };
                bail!("{}: unknown {platform} gameplay source root fingerprint; no corrections were applied", bad.display());
            }
            _ => bail!("{}: ambiguous gameplay source fingerprint {hash}", path.display()),
        };
    }
    match matches.as_slice() {
        [one] => Ok(Some(*one)),
        _ => bail!("{}: ambiguous gameplay source fingerprint {hash}", path.display()),
    }
}

/// A disc can hold one file's bytes under two names (BF1 Xbox's `common/eng` and
/// `common/uk_`), and each name has its own row: the input's own path picks one.
fn same_bytes_by_path<'a>(
    path: &Path,
    matches: Vec<(&'a Table, &'a File)>,
) -> Vec<(&'a Table, &'a File)> {
    if matches.len() < 2 {
        return matches;
    }
    let lower = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let named: Vec<_> = matches
        .iter()
        .copied()
        .filter(|(_, f)| {
            lower
                .strip_suffix(&f.file.to_ascii_lowercase())
                .is_some_and(|before| before.ends_with('/'))
        })
        .collect();
    if named.is_empty() {
        matches
    } else {
        named
    }
}

/// Read one source LVL. Every donor gameplay reader must use this entry point.
pub fn read(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    Ok(read_with_original(path)?.0)
}

/// The unmodified donor is exposed only alongside its corrected gameplay
/// stream, after the same file and source-root fingerprint checks. Render
/// vertices' BMAP indices belong to that donor's skeleton ordering.
pub(crate) fn read_with_original(path: impl AsRef<Path>) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    let level = read_level(path)?;
    Ok((level.gameplay, level.original))
}

pub(crate) struct Level {
    pub gameplay: Vec<u8>,
    pub original: Option<Vec<u8>>,
    pub water_mask: Option<&'static water::WaterMaskCorrection>,
}

pub(crate) fn read_level(path: impl AsRef<Path>) -> Result<Level> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
    let Some((table, file)) = correction(path, &bytes)? else {
        return Ok(Level {
            gameplay: bytes,
            original: None,
            water_mask: None,
        });
    };
    let corrected = rebuild::apply(&bytes, table, file).with_context(|| {
        format!(
            "{}: PS2 gameplay corrections for {}",
            path.display(),
            file.file
        )
    })?;
    Ok(Level {
        gameplay: corrected,
        original: Some(bytes),
        water_mask: file.water_mask.as_ref(),
    })
}
