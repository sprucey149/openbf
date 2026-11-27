//! Generic legacy Battlefront II addon discovery, mission translation, and
//! versioned cache conversion.

mod addon_sides;
pub mod convert;
pub mod mission;
pub mod replace;
mod world;

use extensions::mods::catalog::{StockCatalogs, StockSourceList};
use extensions::mods::{DiscoveredMod, FileHashCache, TargetSelections};
use mungefs::addon::AddonInspection;
use mungefs::modscan::{FactsCache, ScanContext};
use std::path::{Path, PathBuf};

use crate::stock::{StockAssets, StockSources};

pub use convert::{convert_mod_to_cache, ConvertOptions, ConvertReport};

/// Everything a mod inspection or conversion reads besides the mod folder:
/// the stock bake (assets and catalog), the player's stock game copies, and
/// the player's target choices and scan caches in `openbf_data`.
pub struct ModEnv {
    pub stock: Option<StockAssets>,
    pub catalogs: StockCatalogs,
    pub sources: StockSources,
    pub data_dir: Option<PathBuf>,
    pub selections: TargetSelections,
    pub hashes: FileHashCache,
    pub facts: FactsCache,
}

impl Default for ModEnv {
    fn default() -> Self {
        Self {
            stock: None,
            catalogs: StockCatalogs::default(),
            sources: StockSources::default(),
            data_dir: None,
            selections: TargetSelections::default(),
            hashes: FileHashCache::default(),
            facts: FactsCache::default(),
        }
    }
}

impl ModEnv {
    /// `data_dir` is `openbf_data` (target choices, stock copies, caches);
    /// `stock_assets` the baked asset directory, `openbf.pak`, or a game
    /// executable carrying its archive; `extra_sources` more
    /// `game:platform=path` copies.
    pub fn open(
        data_dir: Option<&Path>,
        stock_assets: Option<&Path>,
        extra_sources: &[String],
    ) -> anyhow::Result<Self> {
        let mut env = Self::default();
        if let Some(path) = stock_assets {
            let stock = StockAssets::open(path)?;
            env.catalogs = stock.catalogs();
            env.stock = Some(stock);
        }
        if let Some(dir) = data_dir {
            env.sources = StockSources::from_list(&StockSourceList::load(dir));
            env.selections = TargetSelections::load(dir);
            env.hashes = FileHashCache::load(dir);
            env.facts = FactsCache::load(dir);
            env.data_dir = Some(dir.to_path_buf());
        }
        for arg in extra_sources {
            env.sources.parse_arg(arg)?;
        }
        Ok(env)
    }

    pub fn scan_context(&self) -> ScanContext<'_> {
        ScanContext {
            catalogs: &self.catalogs,
            selections: &self.selections,
            hashes: &self.hashes,
            facts: &self.facts,
        }
    }

    /// Write the scan caches back to `openbf_data`.
    pub fn save_caches(&self) {
        self.hashes.save();
        self.facts.save();
    }
}

/// Inspect a single directory inside `openbf_data/mods/` into shared
/// `DiscoveredMod` metadata alongside raw [`AddonInspection`]
/// ([`mungefs::modscan::inspect_mod_folder`]).
pub fn inspect_mod_folder(
    mod_root: &Path,
    mod_id: &str,
    namespace: &str,
    env: &ModEnv,
) -> (DiscoveredMod, AddonInspection) {
    let out = mungefs::modscan::inspect_mod_folder(mod_root, mod_id, namespace, &env.scan_context());
    env.save_caches();
    out
}
