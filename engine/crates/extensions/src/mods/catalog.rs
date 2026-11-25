//! Stock resource catalog: what every original-game LVL contains, so a legacy
//! replacement LVL can be matched to the file it replaces and compared with it
//! resource by resource.
//!
//! The catalog is metadata generated from the player's own copy by
//! `bake stock-catalog` and packaged with the baked assets under
//! `assets_baked/catalog/<game>_<platform>.ron`. It records, per LVL, the
//! logical path below `_LVL_<platform>` (lowercase, `/`-separated), the size and
//! SHA-256 of the file, the detected game and platform, the `lvl_` sublevel
//! hashes, every named resource chunk with a 64-bit FNV-1a digest of its
//! payload, the per-key digests of every class's properties, and the baked
//! outputs the stock bake wrote from that LVL. No pixels, meshes or text are
//! stored.
//!
//! The same [`LvlFacts`] shape describes a mod's LVL ([`super::lvl_scan`]), so
//! the replacement resolver ([`super::replace`]) compares like with like.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Bump when [`StockCatalog`] or [`LvlFacts`] change shape.
pub const CATALOG_SCHEMA_VERSION: u32 = 1;
/// Bump when `bake stock-catalog` would write different facts for the same input.
pub const CATALOG_GENERATOR_VERSION: u32 = 1;
/// The catalog directory below the baked asset root.
pub const CATALOG_DIR: &str = "catalog";
/// The stock bake's own record of which platform supplied each game's gameplay
/// data and which stock LVLs its animation bakes read.
pub const STOCK_BAKE_FILE: &str = "stock_bake.ron";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourceGame {
    Bf1,
    Bf2,
}

impl SourceGame {
    pub fn key(self) -> &'static str {
        match self {
            Self::Bf1 => "bf1",
            Self::Bf2 => "bf2",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "bf1" => Some(Self::Bf1),
            "bf2" => Some(Self::Bf2),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourcePlatform {
    Pc,
    Ps2,
    Xbox,
}

impl SourcePlatform {
    pub fn key(self) -> &'static str {
        match self {
            Self::Pc => "pc",
            Self::Ps2 => "ps2",
            Self::Xbox => "xbox",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "pc" => Some(Self::Pc),
            "ps2" => Some(Self::Ps2),
            "xbox" => Some(Self::Xbox),
            _ => None,
        }
    }

    /// The data directory name the original game reads its LVLs from.
    pub fn lvl_dir(self) -> &'static str {
        match self {
            Self::Pc => "_lvl_pc",
            Self::Ps2 => "_lvl_ps2",
            Self::Xbox => "_lvl_xbox",
        }
    }
}

/// One named resource chunk inside an LVL (descending `lvl_` sublevels).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LvlResource {
    /// The chunk tag (`modl`, `tex_`, `entc`, ...).
    pub tag: String,
    /// The resource's own name, lowercased (`NAME`, a class's `TYPE`, a
    /// skeleton's `INFO` name).
    pub name: String,
    /// 64-bit FNV-1a over the chunk payload.
    pub digest: u64,
    /// The payload length.
    #[serde(default)]
    pub size: u32,
    /// Classes only (`entc`, `ordc`, `wpnc`, `expc`): FNV-1a 64 over the
    /// ordered values of each property key, keyed by the key's FNV-1a-32 hash.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<(u32, u64)>,
    /// Classes only: the parent class name (`BASE`), lowercased.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

impl LvlResource {
    pub fn is_class(&self) -> bool {
        matches!(self.tag.as_str(), "entc" | "ordc" | "wpnc" | "expc")
    }

    pub fn key(&self) -> (&str, &str) {
        (self.tag.as_str(), self.name.as_str())
    }
}

/// What a scan found in one LVL file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LvlFacts {
    pub size: u64,
    pub sha256: String,
    /// An empty container (`ucfb` with a zero payload): the original game
    /// ships these as placeholders and mods use them to blank a file.
    #[serde(default)]
    pub stub: bool,
    #[serde(default)]
    pub game: Option<SourceGame>,
    #[serde(default)]
    pub platform: Option<SourcePlatform>,
    #[serde(default)]
    pub sublevels: Vec<u32>,
    #[serde(default)]
    pub resources: Vec<LvlResource>,
    /// Set when the file is not a readable `ucfb` container.
    #[serde(default)]
    pub error: Option<String>,
}

impl LvlFacts {
    /// Every distinct `(tag, name)` key with the set of payload digests seen
    /// under it (a sublevel may repeat a shared texture).
    pub fn digests_by_key(&self) -> BTreeMap<(String, String), BTreeSet<u64>> {
        let mut out: BTreeMap<(String, String), BTreeSet<u64>> = BTreeMap::new();
        for r in &self.resources {
            out.entry((r.tag.clone(), r.name.clone()))
                .or_default()
                .insert(r.digest);
        }
        out
    }

    pub fn keys(&self) -> BTreeSet<(String, String)> {
        self.resources
            .iter()
            .map(|r| (r.tag.clone(), r.name.clone()))
            .collect()
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.resources.iter().any(|r| r.tag == tag)
    }
}

/// One stock LVL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogLvl {
    /// Logical path below `_LVL_<platform>`, lowercase with `/`.
    pub path: String,
    pub facts: LvlFacts,
    /// Baked asset paths (relative to `assets_baked/`) the stock bake wrote
    /// from this LVL and that exist in the packaged assets.
    #[serde(default)]
    pub outputs: Vec<String>,
}

/// A stock LVL the stock bake read as a dependency of a whole group, e.g. the
/// animation banks every soldier bake takes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockDependency {
    pub role: String,
    pub game: SourceGame,
    pub platform: SourcePlatform,
    pub path: String,
    pub sha256: String,
}

/// How the packaged stock assets were baked: the platform each game's
/// gameplay data came from and the stock LVLs whole bake groups depend on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockBakeInfo {
    pub schema_version: u32,
    #[serde(default)]
    pub gameplay: BTreeMap<SourceGame, SourcePlatform>,
    #[serde(default)]
    pub dependencies: Vec<StockDependency>,
    /// Per game: the class property hashes its PC gameplay correction table
    /// keeps from the PC copy, because they only change presentation.
    #[serde(default)]
    pub presentation: BTreeMap<SourceGame, Vec<u32>>,
}

impl StockBakeInfo {
    pub fn presentation_of(&self, game: SourceGame) -> BTreeSet<u32> {
        self.presentation
            .get(&game)
            .map(|v| v.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn dependency(&self, role: &str) -> Option<&StockDependency> {
        self.dependencies.iter().find(|d| d.role == role)
    }
}

/// Every LVL of one game's data tree on one platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockCatalog {
    pub schema_version: u32,
    pub generator_version: u32,
    pub game: SourceGame,
    pub platform: SourcePlatform,
    pub lvls: Vec<CatalogLvl>,
}

impl StockCatalog {
    pub fn file_name(game: SourceGame, platform: SourcePlatform) -> String {
        format!("{}_{}.ron", game.key(), platform.key())
    }

    pub fn by_path(&self, logical: &str) -> Option<&CatalogLvl> {
        let lower = logical.to_ascii_lowercase();
        self.lvls.iter().find(|l| l.path == lower)
    }
}

/// The set of catalogs available to this installation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StockCatalogs {
    pub catalogs: Vec<StockCatalog>,
    pub bake: StockBakeInfo,
    /// SHA-256 over the catalog files as read: part of every replacement
    /// cache key, so a new stock bake re-resolves and reconverts.
    pub digest: String,
}

impl StockCatalogs {
    pub fn is_empty(&self) -> bool {
        self.catalogs.is_empty()
    }

    pub fn get(&self, game: SourceGame, platform: SourcePlatform) -> Option<&StockCatalog> {
        self.catalogs
            .iter()
            .find(|c| c.game == game && c.platform == platform)
    }

    pub fn lvl(&self, target: &TargetRef) -> Option<&CatalogLvl> {
        self.get(target.game, target.platform)?.by_path(&target.path)
    }

    fn files() -> Vec<String> {
        let mut out = Vec::new();
        for game in [SourceGame::Bf1, SourceGame::Bf2] {
            for platform in [SourcePlatform::Pc, SourcePlatform::Ps2, SourcePlatform::Xbox] {
                out.push(format!("{CATALOG_DIR}/{}", StockCatalog::file_name(game, platform)));
            }
        }
        out.push(format!("{CATALOG_DIR}/{STOCK_BAKE_FILE}"));
        out
    }

    /// The [`Self::digest`] of the catalogs `read` serves, without parsing
    /// them: the game checks a cache against its own archive this way.
    pub fn digest_with(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"openbf-stock-catalogs-v2\0");
        for rel in Self::files() {
            if let Some(bytes) = read(&rel) {
                hasher.update(rel.as_bytes());
                hasher.update([0]);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
            }
        }
        let mut hex = String::with_capacity(64);
        for b in hasher.finalize() {
            use std::fmt::Write;
            let _ = write!(hex, "{b:02x}");
        }
        hex
    }

    /// Read every catalog through `read`, which maps a path relative to the
    /// baked asset root to its bytes (a directory or the packed archive).
    /// A catalog of another schema is skipped, as if absent.
    pub fn load_with(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"openbf-stock-catalogs-v2\0");
        let mut raw_files: Vec<(String, Vec<u8>)> = Vec::new();
        for rel in Self::files() {
            if let Some(bytes) = read(&rel) {
                hasher.update(rel.as_bytes());
                hasher.update([0]);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
                raw_files.push((rel, bytes));
            }
        }
        let mut digest = String::with_capacity(64);
        for b in hasher.finalize() {
            use std::fmt::Write;
            let _ = write!(digest, "{b:02x}");
        }

        let bake_rel = format!("{CATALOG_DIR}/{STOCK_BAKE_FILE}");
        let mut bake = StockBakeInfo::default();
        let mut cat_inputs: Vec<Vec<u8>> = Vec::new();
        for (rel, bytes) in raw_files {
            if rel == bake_rel {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    bake = ron::from_str::<StockBakeInfo>(text).unwrap_or_default();
                }
            } else {
                cat_inputs.push(bytes);
            }
        }

        let catalogs: Vec<StockCatalog> = if cat_inputs.len() <= 1 {
            cat_inputs
                .into_iter()
                .filter_map(|bytes| {
                    let text = std::str::from_utf8(&bytes).ok()?;
                    let cat = ron::from_str::<StockCatalog>(text).ok()?;
                    (cat.schema_version == CATALOG_SCHEMA_VERSION).then_some(cat)
                })
                .collect()
        } else {
            std::thread::scope(|s| {
                let handles: Vec<_> = cat_inputs
                    .iter()
                    .map(|bytes| {
                        s.spawn(move || {
                            let text = std::str::from_utf8(bytes).ok()?;
                            let cat = ron::from_str::<StockCatalog>(text).ok()?;
                            (cat.schema_version == CATALOG_SCHEMA_VERSION).then_some(cat)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .filter_map(|h| h.join().ok().flatten())
                    .collect()
            })
        };

        Self {
            catalogs,
            bake,
            digest,
        }
    }

    /// Read catalogs from a baked asset directory on disk.
    pub fn load_dir(assets_dir: &Path) -> Self {
        let root = assets_dir.to_path_buf();
        Self::load_with(&move |rel: &str| std::fs::read(root.join(rel)).ok())
    }
}

/// A stock replacement destination: one LVL of one game on one platform.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TargetRef {
    pub game: SourceGame,
    pub platform: SourcePlatform,
    pub path: String,
}

impl TargetRef {
    /// `bf2 pc:side/imp.lvl`, the form selections and manifests use.
    pub fn label(&self) -> String {
        format!("{} {}:{}", self.game.key(), self.platform.key(), self.path)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let (head, path) = s.trim().split_once(':')?;
        let mut parts = head.split_whitespace();
        let game = SourceGame::parse(parts.next()?)?;
        let platform = parts
            .next()
            .map_or(Some(SourcePlatform::Pc), SourcePlatform::parse)?;
        let path = normalize_logical(path)?;
        Some(Self {
            game,
            platform,
            path,
        })
    }
}

/// Lowercase, `/`-separated, no empty, `.` or `..` segments.
pub fn normalize_logical(raw: &str) -> Option<String> {
    let parts: Vec<String> = raw
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .map(str::to_ascii_lowercase)
        .collect();
    if parts.is_empty() || parts.iter().any(|p| p == ".." || p.contains(':')) {
        return None;
    }
    Some(parts.join("/"))
}

/// `openbf_data/stock_sources.ron`: the stock game copies the player picked
/// in the launcher, for a mod conversion that must read stock LVLs (a
/// re-baked soldier borrows its sibling sides and animation banks). The
/// launcher rewrites it whenever its source paths change; `bake mod-convert`
/// reads it from `--data-dir`, so the game can start a conversion too.
pub const STOCK_SOURCES_FILE: &str = "stock_sources.ron";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockSourceEntry {
    pub game: SourceGame,
    pub platform: SourcePlatform,
    /// A PC install folder, an extracted disc tree, or a disc image.
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockSourceList {
    #[serde(default)]
    pub sources: Vec<StockSourceEntry>,
}

impl StockSourceList {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(STOCK_SOURCES_FILE)
    }

    pub fn load(data_dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(data_dir))
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write the list when it differs from what is on disk.
    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        if Self::load(data_dir) == *self && Self::path(data_dir).is_file() {
            return Ok(());
        }
        std::fs::create_dir_all(data_dir)?;
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(std::io::Error::other)?;
        let path = Self::path(data_dir);
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_labels_round_trip_and_normalize_case() {
        let t = TargetRef::parse("bf2 pc:FPM\\Imp\\AllXWing.lvl").unwrap();
        assert_eq!(t.path, "fpm/imp/allxwing.lvl");
        assert_eq!(TargetRef::parse(&t.label()), Some(t));
        assert!(TargetRef::parse("bf2 pc:../x.lvl").is_none());
        assert_eq!(
            TargetRef::parse("bf1:end/end1.lvl").unwrap().platform,
            SourcePlatform::Pc
        );
    }

    #[test]
    fn catalogs_load_from_a_directory_and_digest_their_bytes() {
        let dir = std::env::temp_dir().join(format!("openbf-cat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(CATALOG_DIR)).unwrap();
        let cat = StockCatalog {
            schema_version: CATALOG_SCHEMA_VERSION,
            generator_version: CATALOG_GENERATOR_VERSION,
            game: SourceGame::Bf2,
            platform: SourcePlatform::Pc,
            lvls: vec![CatalogLvl {
                path: "end/end1.lvl".into(),
                facts: LvlFacts {
                    size: 16,
                    sha256: "ab".into(),
                    ..Default::default()
                },
                outputs: vec!["bf2_end1/manifest.ron".into()],
            }],
        };
        std::fs::write(
            dir.join(CATALOG_DIR).join("bf2_pc.ron"),
            ron::to_string(&cat).unwrap(),
        )
        .unwrap();
        let a = StockCatalogs::load_dir(&dir);
        assert_eq!(a.catalogs.len(), 1);
        assert!(a
            .lvl(&TargetRef::parse("bf2 pc:END/End1.lvl").unwrap())
            .is_some());
        let mut cat2 = cat;
        cat2.lvls[0].facts.size = 17;
        std::fs::write(
            dir.join(CATALOG_DIR).join("bf2_pc.ron"),
            ron::to_string(&cat2).unwrap(),
        )
        .unwrap();
        assert_ne!(StockCatalogs::load_dir(&dir).digest, a.digest);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
