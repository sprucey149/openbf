//! Background mod discovery and versioned cache conversion worker for the launcher.
//!
//! Keeps potentially slow filesystem scanning, fingerprinting, and `bake mod-convert`
//! subprocesses off the egui thread while waking the UI cleanly as snapshots change.

use extensions::mods::catalog::StockCatalogs;
use extensions::mods::replace::{mod_capabilities, Capability};
use extensions::mods::{
    list_mod_folders, mod_cache_path, DiscoveredMod, FileHashCache, ModCacheMeta, ModCacheStatus,
    ModCompatibility, ModContentType, ModFolderEntry, TargetChoice, TargetSelections,
    MOD_CACHE_SCHEMA_VERSION, MOD_CONVERTER_ID,
};
use mungefs::modscan::{FactsCache, ScanContext};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Inactivity watchdog for `bake mod-convert`: if the converter produces zero
/// stdout/stderr activity for this long, terminate it cleanly as stalled.
const CONVERT_STALL_TIMEOUT: Duration = Duration::from_secs(180);

/// Uncached mods up to this total size are deep-inspected inline during initial
/// discovery (~few ms); larger uncached mods appear in the list immediately via
/// metadata-only discovery and are deep-inspected on demand when enabled or
/// expanded so copying multi-gigabyte mods into `mods/` never stalls the UI.
const INLINE_DEEP_INSPECT_MAX_BYTES: u64 = 8 * 1024 * 1024;
const MOD_INDEX_CACHE_FILE: &str = "cache/mod_index.ron";
const MOD_INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct DiscoveredModState {
    pub discovered: DiscoveredMod,
    /// `true` once full LVL inspection / target resolution has completed (or
    /// been loaded from cache); `false` for uncached large mods listed via
    /// metadata-only discovery until enabled or expanded.
    pub inspected: bool,
    /// `true` while background deep inspection is actively running for this mod.
    pub inspecting: bool,
    pub cache_status: ModCacheStatus,
    pub cache_warnings: Vec<String>,
    pub cache_unsupported: Vec<String>,
    pub progress_pct: u8,
    pub progress_msg: String,
    pub convert_started_at: Option<Instant>,
}

#[derive(Clone, Debug, Default)]
pub struct ModsSnapshot {
    pub scanning: bool,
    pub generation: u64,
    pub mods: BTreeMap<String, DiscoveredModState>,
    pub folder_ids: Vec<String>,
    pub inspecting_id: Option<String>,
    pub converting_id: Option<String>,
    pub scan_error: Option<String>,
    failed_fingerprints: BTreeMap<String, (String, String)>,
}

/// Where a scan and a conversion read from: `openbf_data`, and the stock
/// bake (the built game, whose archive carries `assets_baked/`, or a
/// checkout's `engine/assets_baked`) that holds the stock resource catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModsEnv {
    pub data_dir: PathBuf,
    pub stock: Option<PathBuf>,
}

enum WorkerCmd {
    Scan {
        env: ModsEnv,
    },
    InspectMods {
        env: ModsEnv,
        mod_ids: Vec<String>,
    },
    ConvertQueue {
        bake_bin: Option<PathBuf>,
        env: ModsEnv,
        mod_ids: Vec<String>,
        force_retry: bool,
    },
}

pub struct ModsWorker {
    tx: Sender<WorkerCmd>,
    state: Arc<Mutex<ModsSnapshot>>,
    active_child: Arc<Mutex<Option<Child>>>,
    cancel_convert: Arc<AtomicBool>,
}

impl Drop for ModsWorker {
    fn drop(&mut self) {
        self.cancel_conversion();
    }
}

impl ModsWorker {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, rx) = mpsc::channel::<WorkerCmd>();
        let state = Arc::new(Mutex::new(ModsSnapshot::default()));
        let active_child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let cancel_convert = Arc::new(AtomicBool::new(false));
        let state_bg = Arc::clone(&state);
        let child_bg = Arc::clone(&active_child);
        let cancel_bg = Arc::clone(&cancel_convert);
        std::thread::Builder::new()
            .name("launcher-mods-worker".into())
            .spawn(move || worker_loop(rx, state_bg, child_bg, cancel_bg, wake))
            .expect("spawn mods worker thread");
        Self {
            tx,
            state,
            active_child,
            cancel_convert,
        }
    }

    /// Cancel the currently running `bake mod-convert` subprocess (if any).
    pub fn cancel_conversion(&self) {
        self.cancel_convert.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.active_child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
        }
    }

    pub fn snapshot(&self) -> ModsSnapshot {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn trigger_scan(&self, env: ModsEnv) {
        if let Ok(mut s) = self.state.lock() {
            s.scanning = true;
        }
        let _ = self.tx.send(WorkerCmd::Scan { env });
    }

    pub fn queue_inspect(&self, env: ModsEnv, mod_ids: Vec<String>) {
        if mod_ids.is_empty() {
            return;
        }
        if let Ok(mut s) = self.state.lock() {
            for id in &mod_ids {
                if let Some(m) = s.mods.get_mut(id) {
                    if !m.inspected {
                        m.inspecting = true;
                    }
                }
            }
        }
        let _ = self.tx.send(WorkerCmd::InspectMods { env, mod_ids });
    }

    pub fn queue_conversions(
        &self,
        bake_bin: Option<PathBuf>,
        env: ModsEnv,
        ordered_enabled_ids: Vec<String>,
    ) {
        if ordered_enabled_ids.is_empty() {
            return;
        }
        let _ = self.tx.send(WorkerCmd::ConvertQueue {
            bake_bin,
            env,
            mod_ids: ordered_enabled_ids,
            force_retry: false,
        });
    }

    pub fn retry_conversion(&self, bake_bin: Option<PathBuf>, env: ModsEnv, mod_id: String) {
        if let Ok(mut s) = self.state.lock() {
            s.failed_fingerprints.remove(&mod_id);
        }
        let _ = self.tx.send(WorkerCmd::ConvertQueue {
            bake_bin,
            env,
            mod_ids: vec![mod_id],
            force_retry: true,
        });
    }
}

fn worker_loop(
    rx: mpsc::Receiver<WorkerCmd>,
    state: Arc<Mutex<ModsSnapshot>>,
    active_child: Arc<Mutex<Option<Child>>>,
    cancel_convert: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let mut cache = WorkerCache::default();
    while let Ok(first) = rx.recv() {
        let mut cmds = vec![first];
        while let Ok(more) = rx.try_recv() {
            cmds.push(more);
        }

        // Coalesce back-to-back scans so focus churn does not repeat work.
        let mut last_scan: Option<ModsEnv> = None;
        let mut inspects: Vec<(ModsEnv, Vec<String>)> = Vec::new();
        let mut converts: Vec<(Option<PathBuf>, ModsEnv, Vec<String>, bool)> = Vec::new();
        for cmd in cmds {
            match cmd {
                WorkerCmd::Scan { env } => last_scan = Some(env),
                WorkerCmd::InspectMods { env, mod_ids } => inspects.push((env, mod_ids)),
                WorkerCmd::ConvertQueue {
                    bake_bin,
                    env,
                    mod_ids,
                    force_retry,
                } => converts.push((bake_bin, env, mod_ids, force_retry)),
            }
        }

        if let Some(env) = last_scan {
            cleanup_orphaned_staging(&env.data_dir);
            run_scan(&env, &mut cache, &state);
            wake();
        }
        for (env, mod_ids) in inspects {
            run_inspect_mods(&env, &mod_ids, &mut cache, &state, &wake);
        }
        for (bake_bin, env, mod_ids, force_retry) in converts {
            run_convert_queue(
                bake_bin.as_deref(),
                &env,
                &mod_ids,
                force_retry,
                &state,
                &active_child,
                &cancel_convert,
                &wake,
            );
        }
    }
}

/// Remove orphaned `*.staging.*` directories left behind by killed or crashed
/// conversions when no conversion is currently active.
fn cleanup_orphaned_staging(data_dir: &Path) {
    let cache_root = extensions::profile::mod_cache_dir(data_dir);
    let Ok(rd) = std::fs::read_dir(&cache_root) else {
        return;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains(".staging.") && entry.path().is_dir() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StockStamp {
    path: PathBuf,
    size_or_count: u64,
    mtime_ns: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ModIndexEntry {
    #[serde(default)]
    converter: String,
    tree_stamp: String,
    catalog_digest: String,
    target_choices: BTreeMap<String, TargetChoice>,
    discovered: DiscoveredMod,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ModIndexFile {
    schema_version: u32,
    entries: BTreeMap<String, ModIndexEntry>,
}

#[derive(Default)]
struct ModIndexCache {
    path: Option<PathBuf>,
    entries: BTreeMap<String, ModIndexEntry>,
    dirty: bool,
}

impl ModIndexCache {
    fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(MOD_INDEX_CACHE_FILE);
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| ron::from_str::<ModIndexFile>(&t).ok())
            .filter(|f| f.schema_version == MOD_INDEX_SCHEMA_VERSION)
            .map(|f| f.entries)
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries,
            dirty: false,
        }
    }

    fn get(
        &self,
        mod_id: &str,
        tree_stamp: &str,
        catalog_digest: &str,
        choices: &BTreeMap<String, TargetChoice>,
    ) -> Option<DiscoveredMod> {
        let e = self.entries.get(mod_id)?;
        if e.converter == MOD_CONVERTER_ID
            && e.tree_stamp == tree_stamp
            && e.catalog_digest == catalog_digest
            && &e.target_choices == choices
        {
            Some(e.discovered.clone())
        } else {
            None
        }
    }

    fn insert(
        &mut self,
        mod_id: String,
        tree_stamp: String,
        catalog_digest: String,
        target_choices: BTreeMap<String, TargetChoice>,
        discovered: DiscoveredMod,
    ) {
        self.entries.insert(
            mod_id,
            ModIndexEntry {
                converter: MOD_CONVERTER_ID.into(),
                tree_stamp,
                catalog_digest,
                target_choices,
                discovered,
            },
        );
        self.dirty = true;
    }

    fn save(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(path) = &self.path else { return };
        self.dirty = false;
        let file = ModIndexFile {
            schema_version: MOD_INDEX_SCHEMA_VERSION,
            entries: self.entries.clone(),
        };
        if let Err(err) = extensions::profile::write_ron_atomic(path, &file, "mod index cache") {
            eprintln!("launcher: failed to save {}: {err:#}", path.display());
        }
    }
}

/// Fast metadata-only directory tree stamp (zero file content reads).
/// Enforces root-boundary containment, skips escaping symlinks, prevents
/// directory cycles, and bounds file count.
/// Returns `(stamp_hex, total_file_bytes, max_mtime_ns)`.
fn dir_tree_stamp(mod_root: &Path) -> std::io::Result<(String, u64, u128)> {
    const MAX_STAMP_FILES: usize = 10_000;
    let root_canon = std::fs::canonicalize(mod_root)?;
    let mut visited_dirs: BTreeSet<PathBuf> = BTreeSet::from([root_canon.clone()]);
    let mut rows: Vec<(String, u64, u128)> = Vec::new();
    let mut stack = vec![(mod_root.to_path_buf(), 0usize)];
    let mut total_bytes = 0u64;
    let mut max_mtime = 0u128;
    while let Some((dir, depth)) = stack.pop() {
        if depth > 16 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let Ok(smeta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if smeta.file_type().is_symlink() {
                match std::fs::canonicalize(&p) {
                    Ok(target) if target.starts_with(&root_canon) => {}
                    _ => continue,
                }
            }
            let Ok(meta) = std::fs::metadata(&p) else {
                continue;
            };
            if meta.is_dir() {
                if let Ok(canon) = std::fs::canonicalize(&p) {
                    if canon.starts_with(&root_canon) && visited_dirs.insert(canon) {
                        stack.push((p, depth + 1));
                    }
                }
            } else if meta.is_file() {
                if rows.len() >= MAX_STAMP_FILES {
                    break;
                }
                let rel = p
                    .strip_prefix(mod_root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                let len = meta.len();
                let mt = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                total_bytes = total_bytes.saturating_add(len);
                max_mtime = max_mtime.max(mt);
                rows.push((rel, len, mt));
            }
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x8422_2325_cbf2_9ce4;
    let mix = |h: &mut u64, bytes: &[u8]| {
        for &b in bytes {
            *h ^= u64::from(b);
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for (rel, len, mt) in &rows {
        mix(&mut h1, rel.as_bytes());
        mix(&mut h1, &[0]);
        mix(&mut h1, &len.to_le_bytes());
        mix(&mut h1, &mt.to_le_bytes());
        mix(&mut h2, &mt.to_le_bytes());
        mix(&mut h2, &len.to_le_bytes());
        mix(&mut h2, rel.as_bytes());
        mix(&mut h2, &[0xff]);
    }
    Ok((
        format!("{h1:016x}{h2:016x}:{}:{total_bytes}", rows.len()),
        total_bytes,
        max_mtime,
    ))
}

#[derive(Default)]
struct WorkerCache {
    stock_stamp: Option<StockStamp>,
    catalog_digest: String,
    catalogs: Option<StockCatalogs>,
    data_dir: Option<PathBuf>,
    hashes: Option<FileHashCache>,
    facts: Option<FactsCache>,
    index: Option<ModIndexCache>,
}

fn file_mtime_ns(p: &Path) -> Option<(u64, u128)> {
    let m = std::fs::metadata(p).ok()?;
    let mt = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some((m.len(), mt))
}

fn compute_stock_stamp(stock: Option<&Path>) -> Option<StockStamp> {
    let stock = stock?;
    if stock.is_dir() {
        let cat_dir = stock.join(extensions::mods::catalog::CATALOG_DIR);
        let mut sum_len = 0u64;
        let mut max_mt = 0u128;
        if let Ok(rd) = std::fs::read_dir(&cat_dir) {
            for e in rd.flatten() {
                if let Some((len, mt)) = file_mtime_ns(&e.path()) {
                    sum_len = sum_len.wrapping_add(len);
                    max_mt = max_mt.max(mt);
                }
            }
        }
        Some(StockStamp {
            path: stock.to_path_buf(),
            size_or_count: sum_len,
            mtime_ns: max_mt,
        })
    } else {
        // For split builds, the catalog bytes live inside `openbf.pak` beside `openbf.exe`.
        let pak_path = stock.with_file_name(pak::PAK_NAME);
        let target = if pak_path.is_file() {
            pak_path.as_path()
        } else {
            stock
        };
        let (len, mt) = file_mtime_ns(target)?;
        Some(StockStamp {
            path: stock.to_path_buf(),
            size_or_count: len,
            mtime_ns: mt,
        })
    }
}

fn catalog_digest_for(stock: Option<&Path>) -> String {
    let Some(stock) = stock else {
        return StockCatalogs::digest_with(&|_| None);
    };
    if stock.is_dir() {
        return StockCatalogs::digest_with(&|rel| std::fs::read(stock.join(rel)).ok());
    }
    match pak::find_archive(stock) {
        Some(archive) => StockCatalogs::digest_with(&|rel| {
            archive.read_entry(&format!("assets_baked/{rel}")).ok()
        }),
        None => StockCatalogs::digest_with(&|_| None),
    }
}

impl WorkerCache {
    fn ensure_data_dir(&mut self, data_dir: &Path) {
        if self.data_dir.as_deref() != Some(data_dir) {
            self.data_dir = Some(data_dir.to_path_buf());
            self.hashes = Some(FileHashCache::load(data_dir));
            self.index = Some(ModIndexCache::load(data_dir));
            self.facts = None;
        }
    }

    fn catalog_digest(&mut self, stock: Option<&Path>) -> &str {
        let stamp = compute_stock_stamp(stock);
        if self.stock_stamp != stamp || self.catalog_digest.is_empty() {
            self.stock_stamp = stamp;
            self.catalogs = None;
            self.catalog_digest = catalog_digest_for(stock);
        }
        &self.catalog_digest
    }

    fn catalogs(&mut self, stock: Option<&Path>) -> &StockCatalogs {
        let stamp = compute_stock_stamp(stock);
        if self.stock_stamp != stamp {
            self.stock_stamp = stamp;
            self.catalogs = None;
        }
        if self.catalogs.is_none() {
            let loaded = load_catalogs(stock);
            self.catalog_digest = loaded.digest.clone();
            self.catalogs = Some(loaded);
        }
        self.catalogs.as_ref().unwrap()
    }

    /// What a full inspection reads, loading the stock catalogs and the LVL
    /// facts cache on first use.
    fn deep_context<'a>(
        &'a mut self,
        data_dir: &Path,
        stock: Option<&Path>,
        selections: &'a TargetSelections,
    ) -> ScanContext<'a> {
        let _ = self.catalogs(stock);
        if self.facts.is_none() {
            self.facts = Some(FactsCache::load(data_dir));
        }
        ScanContext {
            catalogs: self.catalogs.as_ref().unwrap(),
            selections,
            hashes: self.hashes.as_ref().unwrap(),
            facts: self.facts.as_ref().unwrap(),
        }
    }

    /// Record `info` in the mod index; a folder that could not be stamped
    /// is not recorded.
    fn remember(
        &mut self,
        tree_stamp: String,
        catalog_digest: &str,
        choices: BTreeMap<String, TargetChoice>,
        info: &DiscoveredMod,
    ) {
        if tree_stamp.is_empty() {
            return;
        }
        self.index.as_mut().unwrap().insert(
            info.id.clone(),
            tree_stamp,
            catalog_digest.to_owned(),
            choices,
            info.clone(),
        );
    }
}

/// The warnings and unsupported notes a ready cache recorded; empty for any
/// other status.
fn ready_cache_notes(status: &ModCacheStatus, cache_dir: &Path) -> (Vec<String>, Vec<String>) {
    if !matches!(status, ModCacheStatus::Ready) {
        return Default::default();
    }
    ModCacheMeta::load(cache_dir).map_or_else(Default::default, |m| (m.warnings, m.unsupported))
}

/// Fast path: when a mod already has a validated `cache_meta.ron` whose
/// metadata timestamp is at least as new as every file in `mod_root` (so no
/// source file was edited after `bake mod-convert` built the cache), reuse its
/// resolved metadata directly without hashing multi-gigabyte `.lvl` files or
/// parsing the stock catalogs.
fn try_discovered_from_cache_fast(
    entry: &ModFolderEntry,
    data_dir: &Path,
    selections: &TargetSelections,
    catalog_digest: &str,
    max_source_mtime_ns: u128,
) -> Option<DiscoveredMod> {
    if entry.symlink_error.is_some() {
        return None;
    }
    let cache_dir = mod_cache_path(data_dir, &entry.namespace);
    let meta_path = ModCacheMeta::path_in(&cache_dir);
    let (_, meta_mtime) = file_mtime_ns(&meta_path)?;
    if meta_mtime == 0 || max_source_mtime_ns > meta_mtime {
        return None;
    }
    let meta = ModCacheMeta::load(&cache_dir)?;
    if meta.mod_id != entry.mod_id
        || meta.schema_version != MOD_CACHE_SCHEMA_VERSION
        || meta.converter != MOD_CONVERTER_ID
    {
        return None;
    }
    let choices = selections.for_mod(&entry.mod_id);
    let expected_key = meta.expected_replacement_key(catalog_digest, &choices);
    if !matches!(
        meta.validate(&cache_dir, &meta.source_fingerprint, &expected_key),
        ModCacheStatus::Ready
    ) {
        return None;
    }
    let mut caps: BTreeSet<Capability> = mod_capabilities(&meta.resource_ops).into_iter().collect();
    if !meta.maps.is_empty() {
        caps.insert(Capability::AddonMissions);
    }
    Some(DiscoveredMod {
        id: entry.mod_id.clone(),
        namespace: entry.namespace.clone(),
        folder_name: entry.folder_name.clone(),
        root_dir: entry.path.clone(),
        content_root: Some(entry.path.clone()),
        legacy_addon_id: meta.legacy_addon_id,
        display_name: meta.display_name,
        version: meta.version,
        author: meta.author,
        description: meta.description,
        mod_type: meta.mod_type,
        compatibility: if meta.warnings.is_empty() {
            ModCompatibility::Supported
        } else {
            ModCompatibility::PartialWithWarnings
        },
        source_fingerprint: meta.source_fingerprint,
        maps: meta.maps,
        dependencies: meta.dependencies,
        conflicts: meta.conflicts,
        declared_overrides: meta.declared_overrides,
        custom_sides: Vec::new(),
        warnings: meta.warnings,
        errors: Vec::new(),
        unused_lvls: Vec::new(),
        resource_ops: meta.resource_ops,
        capabilities: caps.into_iter().collect(),
        replacement_key: meta.replacement_key,
    })
}

fn inspect_folder_entry(entry: &ModFolderEntry, ctx: &ScanContext<'_>) -> DiscoveredMod {
    if let Some(err) = &entry.symlink_error {
        return DiscoveredMod {
            id: entry.mod_id.clone(),
            namespace: entry.namespace.clone(),
            folder_name: entry.folder_name.clone(),
            root_dir: entry.path.clone(),
            content_root: None,
            legacy_addon_id: None,
            display_name: entry.folder_name.clone(),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::Invalid,
            compatibility: ModCompatibility::Invalid(err.clone()),
            source_fingerprint: String::new(),
            maps: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            custom_sides: Vec::new(),
            warnings: Vec::new(),
            errors: vec![err.clone()],
            unused_lvls: Vec::new(),
            resource_ops: Vec::new(),
            capabilities: Vec::new(),
            replacement_key: String::new(),
        };
    }

    mungefs::modscan::inspect_mod_folder(&entry.path, &entry.mod_id, &entry.namespace, ctx).0
}

/// The stock resource catalogs of the stock bake at `stock`: a baked asset
/// directory, or a game executable or `openbf.pak` carrying the archive.
/// Empty when there is none, which leaves every replacement unresolved.
pub fn load_catalogs(stock: Option<&Path>) -> StockCatalogs {
    let Some(stock) = stock else {
        return StockCatalogs::default();
    };
    if stock.is_dir() {
        return StockCatalogs::load_dir(stock);
    }
    match pak::find_archive(stock) {
        Some(archive) => {
            StockCatalogs::load_with(&|rel| archive.read_entry(&format!("assets_baked/{rel}")).ok())
        }
        None => StockCatalogs::default(),
    }
}

#[cfg(test)]
pub(crate) fn scan_mods_sync(
    env: &ModsEnv,
) -> std::io::Result<(BTreeMap<String, DiscoveredMod>, Vec<String>)> {
    let mut cache = WorkerCache::default();
    let (map, ids) = scan_mods_with_cache(env, &mut cache, true)?;
    Ok((
        map.into_iter().map(|(k, (m, _))| (k, m)).collect(),
        ids,
    ))
}

fn scan_mods_with_cache(
    env: &ModsEnv,
    cache: &mut WorkerCache,
    force_deep: bool,
) -> std::io::Result<(BTreeMap<String, (DiscoveredMod, bool)>, Vec<String>)> {
    let data_dir = &env.data_dir;
    let mdir = extensions::profile::mods_dir(data_dir);
    let entries = list_mod_folders(&mdir)?;
    let selections = TargetSelections::load(data_dir);
    cache.ensure_data_dir(data_dir);
    let catalog_digest = cache.catalog_digest(env.stock.as_deref()).to_owned();

    let mut mods: BTreeMap<String, (DiscoveredMod, bool)> = BTreeMap::new();
    let mut ids = Vec::with_capacity(entries.len());
    let mut deep_misses: Vec<(&ModFolderEntry, String)> = Vec::new();
    for entry in &entries {
        ids.push(entry.mod_id.clone());
        if entry.symlink_error.is_some() {
            let dummy = StockCatalogs::default();
            let hashes = FileHashCache::default();
            let facts = FactsCache::default();
            let ctx = ScanContext {
                catalogs: &dummy,
                selections: &selections,
                hashes: &hashes,
                facts: &facts,
            };
            mods.insert(entry.mod_id.clone(), (inspect_folder_entry(entry, &ctx), true));
            continue;
        }
        let choices = selections.for_mod(&entry.mod_id);
        let (tree_stamp, total_bytes, max_mtime) =
            dir_tree_stamp(&entry.path).unwrap_or_else(|_| (String::new(), 0, 0));

        if !tree_stamp.is_empty() {
            if let Some(info) = cache.index.as_ref().unwrap().get(
                &entry.mod_id,
                &tree_stamp,
                &catalog_digest,
                &choices,
            ) {
                mods.insert(info.id.clone(), (info, true));
                continue;
            }
        }
        if let Some(info) = try_discovered_from_cache_fast(
            entry,
            data_dir,
            &selections,
            &catalog_digest,
            max_mtime,
        ) {
            cache.remember(tree_stamp, &catalog_digest, choices, &info);
            mods.insert(info.id.clone(), (info, true));
            continue;
        }

        if force_deep || total_bytes <= INLINE_DEEP_INSPECT_MAX_BYTES {
            deep_misses.push((entry, tree_stamp));
        } else {
            // Large uncached mod: list immediately from directory metadata and
            // defer full LVL scanning until the user enables or expands it.
            let (quick, _) = mungefs::modscan::quick_inspect_mod_folder(
                &entry.path,
                &entry.mod_id,
                &entry.namespace,
            );
            mods.insert(quick.id.clone(), (quick, false));
        }
    }

    if !deep_misses.is_empty() {
        let newly_inspected: Vec<(DiscoveredMod, String)> = {
            let ctx = cache.deep_context(data_dir, env.stock.as_deref(), &selections);
            deep_misses
                .into_iter()
                .map(|(entry, tree_stamp)| (inspect_folder_entry(entry, &ctx), tree_stamp))
                .collect()
        };
        for (info, tree_stamp) in newly_inspected {
            let choices = selections.for_mod(&info.id);
            cache.remember(tree_stamp, &catalog_digest, choices, &info);
            mods.insert(info.id.clone(), (info, true));
        }
        cache.facts.as_ref().unwrap().save();
        cache.hashes.as_ref().unwrap().save();
    }
    cache.index.as_mut().unwrap().save();
    Ok((mods, ids))
}

fn cache_status_for(
    info: &DiscoveredMod,
    data_dir: &Path,
    failed_fingerprints: &BTreeMap<String, (String, String)>,
) -> ModCacheStatus {
    if !info.compatibility.is_playable() {
        return ModCacheStatus::NotApplicable;
    }
    if let Some((fp, err)) = failed_fingerprints.get(&info.id) {
        if fp == &info.source_fingerprint {
            return ModCacheStatus::Failed(err.clone());
        }
    }
    let cache_dir = mod_cache_path(data_dir, &info.namespace);
    match ModCacheMeta::load(&cache_dir) {
        Some(meta) if meta.mod_id == info.id => {
            meta.validate(&cache_dir, &info.source_fingerprint, &info.replacement_key)
        }
        Some(_) => ModCacheStatus::Stale {
            reason: "mod identity changed".into(),
        },
        None => ModCacheStatus::NeedsConversion,
    }
}

fn run_scan(env: &ModsEnv, cache: &mut WorkerCache, state: &Arc<Mutex<ModsSnapshot>>) {
    let data_dir = &env.data_dir;
    let res = scan_mods_with_cache(env, cache, false);
    let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
    snap.scanning = false;
    snap.generation = snap.generation.saturating_add(1);
    match res {
        Ok((discovered_map, folder_ids)) => {
            snap.scan_error = None;
            snap.folder_ids = folder_ids;
            let mut next_mods = BTreeMap::new();
            for (id, (info, inspected)) in discovered_map {
                let prev_converting = snap.converting_id.as_deref() == Some(id.as_str());
                let prev_inspecting = snap.inspecting_id.as_deref() == Some(id.as_str());
                let cache_status = if prev_converting {
                    ModCacheStatus::Converting
                } else if !inspected {
                    ModCacheStatus::NeedsConversion
                } else {
                    cache_status_for(&info, data_dir, &snap.failed_fingerprints)
                };
                let (progress_pct, progress_msg, convert_started_at) = snap
                    .mods
                    .get(&id)
                    .filter(|_| prev_converting)
                    .map_or((0, String::new(), None), |m| {
                        (m.progress_pct, m.progress_msg.clone(), m.convert_started_at)
                    });
                let (cache_warnings, cache_unsupported) = if inspected {
                    ready_cache_notes(&cache_status, &mod_cache_path(data_dir, &info.namespace))
                } else {
                    Default::default()
                };
                next_mods.insert(
                    id,
                    DiscoveredModState {
                        discovered: info,
                        inspected,
                        inspecting: !inspected && prev_inspecting,
                        cache_status,
                        cache_warnings,
                        cache_unsupported,
                        progress_pct,
                        progress_msg,
                        convert_started_at,
                    },
                );
            }
            snap.mods = next_mods;
        }
        Err(e) => {
            snap.scan_error = Some(e.to_string());
        }
    }
}

fn run_inspect_mods(
    env: &ModsEnv,
    mod_ids: &[String],
    cache: &mut WorkerCache,
    state: &Arc<Mutex<ModsSnapshot>>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    let data_dir = &env.data_dir;
    let mdir = extensions::profile::mods_dir(data_dir);
    let Ok(entries) = list_mod_folders(&mdir) else {
        return;
    };
    let by_id: BTreeMap<String, ModFolderEntry> = entries
        .into_iter()
        .map(|e| (e.mod_id.clone(), e))
        .collect();
    let selections = TargetSelections::load(data_dir);
    cache.ensure_data_dir(data_dir);
    let catalog_digest = cache.catalog_digest(env.stock.as_deref()).to_owned();

    let mut did_deep = false;
    for mod_id in mod_ids {
        let already = {
            let snap = state.lock().unwrap_or_else(|e| e.into_inner());
            snap.mods.get(mod_id).is_none_or(|m| m.inspected)
        };
        if already {
            continue;
        }
        let Some(entry) = by_id.get(mod_id) else {
            continue;
        };
        {
            let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
            snap.inspecting_id = Some(mod_id.clone());
            if let Some(m) = snap.mods.get_mut(mod_id) {
                m.inspecting = true;
            }
        }
        wake();

        let (tree_stamp, _, _) =
            dir_tree_stamp(&entry.path).unwrap_or_else(|_| (String::new(), 0, 0));
        let choices = selections.for_mod(mod_id);
        let info = if let Some(cached) = (!tree_stamp.is_empty())
            .then(|| {
                cache
                    .index
                    .as_ref()
                    .unwrap()
                    .get(mod_id, &tree_stamp, &catalog_digest, &choices)
            })
            .flatten()
        {
            cached
        } else {
            did_deep = true;
            let info = {
                let ctx = cache.deep_context(data_dir, env.stock.as_deref(), &selections);
                inspect_folder_entry(entry, &ctx)
            };
            cache.remember(tree_stamp, &catalog_digest, choices, &info);
            info
        };

        let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
        snap.inspecting_id = None;
        snap.generation = snap.generation.saturating_add(1);
        let cache_status = if snap.converting_id.as_deref() == Some(mod_id.as_str()) {
            ModCacheStatus::Converting
        } else {
            cache_status_for(&info, data_dir, &snap.failed_fingerprints)
        };
        let (cache_warnings, cache_unsupported) =
            ready_cache_notes(&cache_status, &mod_cache_path(data_dir, &info.namespace));
        if let Some(m) = snap.mods.get_mut(mod_id) {
            m.discovered = info;
            m.inspected = true;
            m.inspecting = false;
            m.cache_status = cache_status;
            m.cache_warnings = cache_warnings;
            m.cache_unsupported = cache_unsupported;
        }
        drop(snap);
        wake();
    }
    if did_deep {
        if let Some(facts) = cache.facts.as_ref() {
            facts.save();
        }
        if let Some(hashes) = cache.hashes.as_ref() {
            hashes.save();
        }
        if let Some(index) = cache.index.as_mut() {
            index.save();
        }
    }
}

fn run_convert_queue(
    bake_bin: Option<&Path>,
    env: &ModsEnv,
    mod_ids: &[String],
    force_retry: bool,
    state: &Arc<Mutex<ModsSnapshot>>,
    active_child: &Arc<Mutex<Option<Child>>>,
    cancel_convert: &Arc<AtomicBool>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    let data_dir = env.data_dir.as_path();
    for mod_id in mod_ids {
        let task = {
            let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
            let ModsSnapshot {
                mods,
                converting_id,
                generation,
                failed_fingerprints,
                ..
            } = &mut *snap;
            if force_retry {
                failed_fingerprints.remove(mod_id);
            }
            let Some(item) = mods.get_mut(mod_id) else {
                continue;
            };
            if !item.discovered.compatibility.is_playable() {
                continue;
            }
            let status = cache_status_for(&item.discovered, data_dir, failed_fingerprints);
            let needs = matches!(
                status,
                ModCacheStatus::NeedsConversion | ModCacheStatus::Stale { .. }
            );
            if !needs {
                item.cache_status = status;
                continue;
            }
            let Some(bake) = bake_bin else {
                let err = "`bake` tool executable not found".to_owned();
                item.cache_status = ModCacheStatus::Failed(err.clone());
                let fp = item.discovered.source_fingerprint.clone();
                failed_fingerprints.insert(mod_id.clone(), (fp, err));
                *generation = generation.saturating_add(1);
                drop(snap);
                wake();
                continue;
            };
            item.cache_status = ModCacheStatus::Converting;
            item.progress_pct = 0;
            item.progress_msg = "Starting conversion...".into();
            item.convert_started_at = Some(Instant::now());
            let mod_dir = item.discovered.root_dir.clone();
            let ns = item.discovered.namespace.clone();
            let fp = item.discovered.source_fingerprint.clone();
            let key = item.discovered.replacement_key.clone();
            let cache_dir = mod_cache_path(data_dir, &ns);
            cancel_convert.store(false, Ordering::SeqCst);
            *converting_id = Some(mod_id.clone());
            *generation = generation.saturating_add(1);
            Some((bake.to_path_buf(), mod_dir, cache_dir, ns, fp, key))
        };

        let Some((bake, mod_dir, cache_dir, ns, fp, key)) = task else {
            continue;
        };
        wake();

        let result = run_bake_convert(
            &bake,
            env,
            &mod_dir,
            &cache_dir,
            mod_id,
            &ns,
            active_child,
            cancel_convert,
            |pct, msg| {
                let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(item) = snap.mods.get_mut(mod_id) {
                    item.progress_pct = pct;
                    item.progress_msg = msg.to_owned();
                }
                wake();
            },
        );

        let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
        snap.converting_id = None;
        snap.generation = snap.generation.saturating_add(1);
        match result {
            Ok(()) => {
                let status = converted_status(&cache_dir, &fp, &key, &bake);
                let (cache_warnings, cache_unsupported) = ready_cache_notes(&status, &cache_dir);
                if let ModCacheStatus::Failed(err) = &status {
                    snap.failed_fingerprints
                        .insert(mod_id.clone(), (fp.clone(), err.clone()));
                } else {
                    snap.failed_fingerprints.remove(mod_id);
                }
                if let Some(item) = snap.mods.get_mut(mod_id) {
                    item.progress_pct = 100;
                    item.progress_msg = "Ready".into();
                    item.convert_started_at = None;
                    item.cache_status = status;
                    item.cache_warnings = cache_warnings;
                    item.cache_unsupported = cache_unsupported;
                }
            }
            Err(err) => {
                let err_str = err.to_string();
                snap.failed_fingerprints
                    .insert(mod_id.clone(), (fp, err_str.clone()));
                if let Some(item) = snap.mods.get_mut(mod_id) {
                    item.convert_started_at = None;
                    item.cache_status = ModCacheStatus::Failed(err_str);
                }
            }
        }
        drop(snap);
        wake();
    }
}

/// The cache a finished conversion left behind. A cache that still reads as
/// stale is a failure, never queued again: a `bake` older than this launcher
/// writes an older converter version every time, and requeueing it loops.
fn converted_status(
    cache_dir: &Path,
    fingerprint: &str,
    replacement_key: &str,
    bake: &Path,
) -> ModCacheStatus {
    match ModCacheMeta::load(cache_dir)
        .map(|meta| meta.validate(cache_dir, fingerprint, replacement_key))
    {
        None => ModCacheStatus::Failed("missing cache metadata".into()),
        Some(ModCacheStatus::Stale { reason } | ModCacheStatus::Failed(reason)) => {
            let written = ModCacheMeta::load(cache_dir).map(|m| m.converter);
            if written.is_some_and(|v| v != MOD_CONVERTER_ID) {
                ModCacheStatus::Failed(format!(
                    "conversion finished but the cache is still stale ({reason}); `{}` may be older than the launcher",
                    bake.display()
                ))
            } else {
                ModCacheStatus::Failed(format!(
                    "conversion finished but the cache is still stale ({reason})"
                ))
            }
        }
        Some(ModCacheStatus::NeedsConversion) => {
            ModCacheStatus::Failed("conversion finished but wrote no cache".into())
        }
        Some(status) => status,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_bake_convert(
    bake: &Path,
    env: &ModsEnv,
    mod_dir: &Path,
    cache_dir: &Path,
    mod_id: &str,
    namespace: &str,
    active_child: &Arc<Mutex<Option<Child>>>,
    cancel_convert: &Arc<AtomicBool>,
    mut on_progress: impl FnMut(u8, &str),
) -> anyhow::Result<()> {
    let mut cmd = Command::new(bake);
    cmd.arg("mod-convert")
        .arg(mod_dir)
        .arg(cache_dir)
        .arg("--mod-id")
        .arg(mod_id)
        .arg("--namespace")
        .arg(namespace)
        .arg("--data-dir")
        .arg(&env.data_dir);
    if let Some(stock) = &env.stock {
        cmd.arg("--stock-assets").arg(stock);
    }
    if std::env::var_os("OPENBF_FFMPEG").is_none() {
        let bundled = env
            .data_dir
            .join("tools")
            .join(format!("ffmpeg{}", crate::paths::EXE_SUFFIX));
        if bundled.is_file() {
            cmd.env("OPENBF_FFMPEG", bundled);
        }
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("launching `{}`: {e}", bake.display()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Ok(mut slot) = active_child.lock() {
        *slot = Some(child);
    }

    let last_activity = Arc::new(Mutex::new(Instant::now()));
    let finished = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));

    let err_activity = Arc::clone(&last_activity);
    let stderr_handle = std::thread::spawn(move || {
        stderr
            .map(|s| {
                let mut lines = Vec::new();
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    if let Ok(mut t) = err_activity.lock() {
                        *t = Instant::now();
                    }
                    lines.push(line);
                }
                lines.join("\n")
            })
            .unwrap_or_default()
    });

    // Inactivity watchdog: if `bake` produces zero stdout/stderr output for
    // `CONVERT_STALL_TIMEOUT`, terminate it so the UI never hangs forever.
    let wd_activity = Arc::clone(&last_activity);
    let wd_finished = Arc::clone(&finished);
    let wd_timed_out = Arc::clone(&timed_out);
    let wd_child = Arc::clone(active_child);
    let wd_cancel = Arc::clone(cancel_convert);
    let watchdog = std::thread::spawn(move || {
        while !wd_finished.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(2));
            if wd_finished.load(Ordering::SeqCst) {
                break;
            }
            let stalled = wd_activity
                .lock()
                .map(|t| t.elapsed() >= CONVERT_STALL_TIMEOUT)
                .unwrap_or(false);
            if wd_cancel.load(Ordering::SeqCst) || stalled {
                if stalled && !wd_cancel.load(Ordering::SeqCst) {
                    wd_timed_out.store(true, Ordering::SeqCst);
                }
                if let Ok(mut slot) = wd_child.lock() {
                    if let Some(c) = slot.as_mut() {
                        let _ = c.kill();
                    }
                }
                break;
            }
        }
    });

    if let Some(out) = stdout {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Ok(mut t) = last_activity.lock() {
                *t = Instant::now();
            }
            if let Some(rest) = line.strip_prefix("PROGRESS ") {
                if let Some((pct_str, msg)) = rest.split_once(' ') {
                    if let Ok(pct) = pct_str.parse::<u8>() {
                        on_progress(pct, msg.trim());
                    }
                }
            }
        }
    }

    finished.store(true, Ordering::SeqCst);
    let child_opt = active_child.lock().ok().and_then(|mut s| s.take());
    let status = match child_opt {
        Some(mut c) => c.wait()?,
        None => anyhow::bail!("conversion process handle lost"),
    };
    let _ = watchdog.join();
    let err_txt = stderr_handle.join().unwrap_or_default();

    if cancel_convert.load(Ordering::SeqCst) {
        cleanup_orphaned_staging(&env.data_dir);
        anyhow::bail!("Cancelled by user (click Retry to convert again)");
    }
    if timed_out.load(Ordering::SeqCst) {
        cleanup_orphaned_staging(&env.data_dir);
        anyhow::bail!(
            "Timed out after {} minutes with no progress (stalled)",
            CONVERT_STALL_TIMEOUT.as_secs() / 60
        );
    }
    if !status.success() {
        anyhow::bail!(
            "exit {:?}{}",
            status.code(),
            if err_txt.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", err_txt.trim())
            }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_duplicate_legacy_ids_and_detects_added_and_removed_folders() {
        let root = std::env::temp_dir().join(format!("openbf-worker-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let env = ModsEnv {
            data_dir: root.clone(),
            stock: None,
        };
        let mods = extensions::profile::mods_dir(&root);
        std::fs::create_dir_all(mods.join("BadMod")).unwrap();
        std::fs::write(mods.join("BadMod/mod.dll"), b"MZ").unwrap();

        let (map1, ids1) = scan_mods_sync(&env).unwrap();
        assert_eq!(ids1, ["content.badmod"]);
        assert_eq!(map1["content.badmod"].mod_type, ModContentType::Unsupported);

        // Two folders sharing legacy layout id get independent stable mod IDs/namespaces.
        for folder in ["ABR_One", "ABR_Two"] {
            let side = mods.join(folder).join("data/_LVL_PC/SIDE");
            std::fs::create_dir_all(&side).unwrap();
            std::fs::write(side.join("all.lvl"), b"ucfb_stub").unwrap();
        }
        let (map2, ids2) = scan_mods_sync(&env).unwrap();
        assert_eq!(ids2.len(), 3);
        assert_ne!(
            map2["content.abr_one"].namespace,
            map2["content.abr_two"].namespace
        );

        std::fs::remove_dir_all(mods.join("ABR_One")).unwrap();
        let (_map3, ids3) = scan_mods_sync(&env).unwrap();
        assert_eq!(ids3.len(), 2);
        assert!(!ids3.contains(&"content.abr_one".to_owned()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "requires the user's installed addon and isolated conversion cache"]
    fn discovers_kothlis_and_abr_with_only_standalone_missions() {
        let data = PathBuf::from(
            std::env::var_os("OPENBF_KTH_TEST_DATA").expect("isolated addon test data"),
        );
        let (mods, ids) = scan_mods_sync(&ModsEnv {
            data_dir: data.clone(),
            stock: None,
        })
        .unwrap();
        assert!(ids.contains(&"content.kth".to_owned()));
        assert!(ids.contains(&"content.abr".to_owned()));
        let kth = &mods["content.kth"];
        assert_eq!(kth.maps.len(), 5);
        for map in &kth.maps {
            assert_eq!(map.missions.len(), 6, "{}", map.map_key);
            assert!(map.missions.iter().all(|mission| mission.supported));
            assert_eq!(map.modes_cw, ["con", "ctf", "hunt"]);
            assert_eq!(map.modes_gcw, ["con", "ctf", "hunt"]);
        }
        assert_eq!(
            cache_status_for(kth, &data, &BTreeMap::new()),
            ModCacheStatus::Ready
        );
        assert_eq!(mods["content.abr"].maps[0].missions.len(), 4);
    }

    #[test]
    fn a_conversion_that_leaves_a_stale_cache_fails_instead_of_requeueing() {
        let cache =
            std::env::temp_dir().join(format!("openbf-worker-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();
        let bake = Path::new("bake.exe");
        assert!(matches!(
            converted_status(&cache, "fp", "", bake),
            ModCacheStatus::Failed(_)
        ));

        let mut meta = ModCacheMeta {
            schema_version: extensions::mods::MOD_CACHE_SCHEMA_VERSION,
            converter: "an older converter".into(),
            mod_id: "content.abr".into(),
            namespace: "mod_abr".into(),
            source_fingerprint: "fp".into(),
            legacy_addon_id: None,
            display_name: "ABR".into(),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::LegacyMapAddon,
            maps: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            has_locl: false,
            warnings: Vec::new(),
            resource_ops: Vec::new(),
            replacement_key: String::new(),
            replacement_outputs: Vec::new(),
            unsupported: Vec::new(),
            catalog_digest: String::new(),
            target_choices: BTreeMap::new(),
            custom_modes: BTreeMap::new(),
            has_shell_icons: false,
        };
        meta.save_atomic(&cache).unwrap();
        let status = converted_status(&cache, "fp", "", bake);
        assert!(
            matches!(&status, ModCacheStatus::Failed(e) if e.contains("bake.exe")),
            "{status:?}"
        );
        assert!(!matches!(
            status,
            ModCacheStatus::NeedsConversion | ModCacheStatus::Stale { .. }
        ));

        meta.converter = extensions::mods::MOD_CONVERTER_ID.into();
        meta.save_atomic(&cache).unwrap();
        assert_eq!(converted_status(&cache, "fp", "", bake), ModCacheStatus::Ready);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn fast_scan_lists_large_uncached_mods_without_reading_lvl_contents() {
        let root = std::env::temp_dir().join(format!(
            "openbf-worker-fast-list-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mod_dir = extensions::profile::mods_dir(&root)
            .join("HugeSidePack")
            .join("side");
        std::fs::create_dir_all(&mod_dir).unwrap();
        // Sparse 16 MiB file (> INLINE_DEEP_INSPECT_MAX_BYTES) with invalid ucfb header:
        // if fast listing ever tried to read/scan it, it would mark it inspected or error.
        let f = std::fs::File::create(mod_dir.join("all.lvl")).unwrap();
        f.set_len(16 * 1024 * 1024).unwrap();
        drop(f);

        let env = ModsEnv {
            data_dir: root.clone(),
            stock: None,
        };
        let mut cache = WorkerCache::default();
        let (mods, ids) = scan_mods_with_cache(&env, &mut cache, false).unwrap();
        assert_eq!(ids, ["content.hugesidepack"]);
        let (info, inspected) = &mods["content.hugesidepack"];
        assert!(!inspected, "large uncached mod should defer deep LVL scanning");
        assert_eq!(info.mod_type, ModContentType::LegacySideOverhaul);
        assert!(info.compatibility.is_playable());
        assert!(
            cache.catalogs.is_none(),
            "fast listing must not parse stock catalogs when no mod needs deep inspection"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
