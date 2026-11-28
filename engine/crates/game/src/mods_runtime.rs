//! Runtime discovery, auto-conversion, validation, priority ordering, and
//! VFS mounting for filesystem-based legacy Battlefront II content mods.
//!
//! Strictly avoids linking LVL/Lua parsers (`mungefs`) into `game`: pre-converted
//! mods are loaded directly from `openbf_data/mod_cache/`, and missing/stale
//! enabled mods are prepared via the external `bake mod-convert` tool.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};

use anyhow::{bail, Context, Result};
use extensions::mods::catalog::StockCatalogs;
use extensions::mods::{
    compute_mod_fingerprint_cached, content_mod_multiplayer_digest, list_mod_folders,
    mod_cache_path, resolve_content_mods_isolated, DiscoveredMod, FileHashCache, ModCacheMeta,
    ModCacheStatus, ModCompatibility, ModFolderEntry, ModMapSummary, ResourceOverrideConflict,
    TargetSelections,
};
use extensions::profile::{mods_dir, profile_path, ExtensionProfile};
use manifest::hud::HudLocl;
use pak::{clear_overlays, set_overlays, OverlayLayer, OverlayRule};
use ui::shell::{Era, ModMapEntry, Mode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountedMod {
    pub mod_id: String,
    pub namespace: String,
    pub folder_name: String,
    pub display_name: String,
    pub legacy_addon_id: Option<String>,
    pub cache_dir: PathBuf,
    pub source_fingerprint: String,
    pub maps: Vec<ModMapSummary>,
    pub declared_overrides: Vec<String>,
    /// [`ModCacheMeta::replacement_key`]: which stock files this mod replaces
    /// and how, part of the online compatibility digest.
    pub replacement_key: String,
    /// The mod changes gameplay through a stock replacement.
    pub replaces_gameplay: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct MountedModsReport {
    pub disabled: bool,
    pub active: Vec<MountedMod>,
    pub mod_maps: Vec<ModMapEntry>,
    pub custom_modes: BTreeMap<String, extensions::mods::ModModeMeta>,
    pub shell_icon_dirs: Vec<PathBuf>,
    /// Each mod's own places for weapon models in the HUD's weapon rows, in
    /// load order.
    pub hud_icon_places: Vec<manifest::hud::HudLayout>,
    pub active_fingerprints: Vec<(String, String)>,
    pub override_digest: String,
    pub conflicts: Vec<ResourceOverrideConflict>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

static RUNTIME_STATE: RwLock<MountedModsReport> = RwLock::new(MountedModsReport {
    disabled: false,
    active: Vec::new(),
    mod_maps: Vec::new(),
    custom_modes: BTreeMap::new(),
    shell_icon_dirs: Vec::new(),
    hud_icon_places: Vec::new(),
    active_fingerprints: Vec::new(),
    override_digest: String::new(),
    conflicts: Vec::new(),
    warnings: Vec::new(),
    errors: Vec::new(),
});

/// Snapshot of active mod maps for Instant Action catalog injection.
pub fn active_mod_maps() -> Vec<ModMapEntry> {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .mod_maps
        .clone()
}

/// Snapshot of active mod-declared custom mode metadata (`addme.script` `change.mode_*`).
pub fn active_custom_modes() -> BTreeMap<String, extensions::mods::ModModeMeta> {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .custom_modes
        .clone()
}

/// Directories containing baked custom shell/mode icon KTX2s from active mods (priority order).
pub fn active_shell_icon_dirs() -> Vec<PathBuf> {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .shell_icon_dirs
        .clone()
}

/// The active mods' places for weapon models in the HUD's weapon rows.
pub fn active_hud_icon_places() -> Vec<manifest::hud::HudLayout> {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .hud_icon_places
        .clone()
}

fn catalog_entries(maps: &[ModMapSummary], folder: &str, source: &str) -> Vec<ModMapEntry> {
    let mut entries: Vec<ModMapEntry> = Vec::new();
    let mut groups = BTreeMap::new();
    for map in maps {
        let mut modes: BTreeMap<Era, Vec<Mode>> = BTreeMap::new();
        let cw = parse_modes_list(&map.modes_cw);
        if !cw.is_empty() {
            modes.insert(Era::Cw, cw);
        }
        let gcw = parse_modes_list(&map.modes_gcw);
        if !gcw.is_empty() {
            modes.insert(Era::Gcw, gcw);
        }
        for (era_code, mode_list) in &map.modes_custom_eras {
            if let Some(era) = Era::from_code(era_code) {
                let parsed = parse_modes_list(mode_list);
                if !parsed.is_empty() {
                    let slot = modes.entry(era).or_default();
                    for mode in parsed {
                        if !slot.contains(&mode) {
                            slot.push(mode);
                        }
                    }
                }
            }
        }
        let mut era_overrides: BTreeMap<Era, ui::shell::EraOverride> = BTreeMap::new();
        for (era_code, ov) in &map.era_overrides {
            if let Some(era) = Era::from_code(era_code) {
                era_overrides.insert(
                    era,
                    ui::shell::EraOverride {
                        name: ov.name.clone(),
                        icon: ov.icon.clone(),
                    },
                );
            }
        }
        // Every world a mod lists on a stock map's row shares one entry.
        let group = match (&map.stock_row, &map.variant_title) {
            (Some(code), _) => Some(("", code.as_str())),
            (None, Some(title)) => Some((map.source_lvl.as_str(), title.as_str())),
            (None, None) => None,
        };
        if let Some(group) = group {
            if let Some(&index) = groups.get(&group) {
                let entry: &mut ModMapEntry = &mut entries[index];
                entry.variant_keys.push(map.map_key.clone());
                for (era, variant) in modes {
                    let all = entry.modes.entry(era).or_default();
                    for mode in variant {
                        if !all.contains(&mode) {
                            all.push(mode);
                        }
                    }
                }
                for (era, ov) in era_overrides {
                    entry.era_overrides.entry(era).or_insert(ov);
                }
                continue;
            }
            groups.insert(group, entries.len());
        }
        entries.push(ModMapEntry {
            key: map.map_key.clone(),
            stock_row: map.stock_row.clone(),
            legacy_code: map.legacy_code.clone(),
            name: map
                .variant_title
                .as_ref()
                .unwrap_or(&map.display_name)
                .clone(),
            disambiguator: folder.to_owned(),
            source: source.to_owned(),
            description: map.description.clone(),
            modes,
            era_overrides,
            variant_keys: vec![map.map_key.clone()],
        });
    }
    entries
}

/// Resolve a catalog row to one concrete world before mission or asset loading.
pub fn select_world_variant(
    key: &str,
    mode: &str,
    armies: &[String; 2],
    era: Option<&str>,
    seed: u32,
    multiplayer: bool,
) -> String {
    let state = RUNTIME_STATE.read().unwrap_or_else(|e| e.into_inner());
    let maps = || state.active.iter().flat_map(|m| &m.maps);
    let era = era.unwrap_or_else(|| {
        if armies.iter().any(|a| a == "rep" || a == "cis") {
            "cw"
        } else {
            "gcw"
        }
    });
    let choose = |row| choose_world_variant(row, maps(), mode, era, seed, multiplayer);
    if let Some(row) = state.mod_maps.iter().find(|row| row.key == key) {
        if row.variant_keys.len() > 1 {
            return choose(row).unwrap_or(key).to_owned();
        }
        return key.to_owned();
    }
    // A stock map's row: the first mod in load order that lists this mode and
    // era on it supplies the round's map. Otherwise the stock one plays.
    let code = key.strip_prefix("bf2_").unwrap_or(key);
    state
        .mod_maps
        .iter()
        .filter(|row| row.stock_row.as_deref() == Some(code))
        .find_map(choose)
        .unwrap_or(key)
        .to_owned()
}

/// The mission file a round of `map` plays, when the addon script behind
/// `picked` set up more than one roster: a multiplayer round takes the
/// script's multiplayer run, a single-player one rolls by the runs' weights.
/// `None` keeps `picked`.
pub fn select_roster(map: &str, picked: &str, seed: u32, multiplayer: bool) -> Option<String> {
    let state = RUNTIME_STATE.read().unwrap_or_else(|e| e.into_inner());
    let summary = state
        .active
        .iter()
        .flat_map(|m| &m.maps)
        .find(|m| m.map_key == map)?;
    choose_roster(&summary.missions, picked, seed, multiplayer)
        .filter(|file| *file != picked)
        .map(str::to_owned)
}

fn choose_roster<'a>(
    missions: &'a [extensions::mods::ModMissionSummary],
    picked: &str,
    seed: u32,
    multiplayer: bool,
) -> Option<&'a str> {
    let script = &missions
        .iter()
        .find(|m| m.mission_ron == picked)?
        .script_name;
    let runs = || {
        missions
            .iter()
            .filter(move |m| m.supported && m.script_name == *script)
    };
    if multiplayer {
        return runs()
            .find(|m| m.multiplayer_variant)
            .map(|m| m.mission_ron.as_str());
    }
    let candidates: Vec<&str> = runs()
        .flat_map(|m| {
            std::iter::repeat_n(m.mission_ron.as_str(), usize::from(m.single_player_weight))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    // OURS: its own stream, so the troops do not follow the world variant's draw.
    let draw = core::rng::SimRng::new(seed ^ 0x7200_5EED).next_rand() as usize;
    Some(candidates[draw % candidates.len()])
}

fn choose_world_variant<'a>(
    row: &'a ModMapEntry,
    maps: impl Iterator<Item = &'a ModMapSummary>,
    mode: &str,
    era: &str,
    seed: u32,
    multiplayer: bool,
) -> Option<&'a str> {
    let mut candidates = Vec::new();
    for map in maps.filter(|m| row.variant_keys.contains(&m.map_key)) {
        for mission in map
            .missions
            .iter()
            .filter(|m| m.supported && m.mode == mode && m.era == era)
        {
            if multiplayer && mission.multiplayer_variant {
                return Some(&map.map_key);
            }
            candidates.extend(std::iter::repeat_n(
                map.map_key.as_str(),
                usize::from(mission.single_player_weight),
            ));
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let choice = core::rng::SimRng::new(seed).next_rand() as usize % candidates.len();
    Some(candidates[choice])
}

/// Snapshot of currently mounted content mods in priority order (index 0 = highest).
pub fn active_mods() -> Vec<MountedMod> {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .active
        .clone()
}

/// Active `(mod_id, source_fingerprint)` pairs in priority order.
pub fn active_mod_fingerprints() -> Vec<(String, String)> {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .active_fingerprints
        .clone()
}

/// Canonical digest over active mod IDs, fingerprints, and priority order.
pub fn override_digest() -> String {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .override_digest
        .clone()
}

/// Latest mount report (including conflicts/warnings/errors).
pub fn current_report() -> MountedModsReport {
    RUNTIME_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Clear all active runtime mod overlays, localization tables, and state.
pub fn clear_runtime_mods() {
    clear_overlays();
    crate::event_feed::clear_mod_locls();
    let mut state = RUNTIME_STATE.write().unwrap_or_else(|e| e.into_inner());
    *state = MountedModsReport::default();
}

/// Locate `bake` for automatic cache preparation when a mod is launched directly
/// without having opened the launcher first.
pub fn locate_bake_bin(data_dir: &Path) -> Option<PathBuf> {
    if let Some(env_bin) = std::env::var_os("OPENBF_BAKE_BIN") {
        let p = PathBuf::from(env_bin);
        if p.is_file() {
            return Some(p);
        }
    }
    let name = format!("bake{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&name));
            candidates.push(data_dir.join("tools").join(&name));
            candidates.push(dir.join("openbf_data").join("tools").join(&name));
            for anc in dir.ancestors().take(5) {
                let target = anc.join("engine").join("target");
                candidates.push(target.join("release").join(&name));
                candidates.push(target.join("debug").join(&name));
                let local_target = anc.join("target");
                candidates.push(local_target.join("release").join(&name));
                candidates.push(local_target.join("debug").join(&name));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(
            cwd.join("engine")
                .join("target")
                .join("release")
                .join(&name),
        );
        candidates.push(cwd.join("engine").join("target").join("debug").join(&name));
        candidates.push(cwd.join("openbf_data").join("tools").join(&name));
    }
    candidates.push(data_dir.join("tools").join(&name));
    extensions::mods::pick_bake_bin(candidates)
}

/// Where `bake mod-convert` reads the stock bake from: the asset directory
/// when it exists on disk (a checkout), else this executable, whose archive
/// (inside it or `openbf.pak` beside it) carries the baked assets.
fn stock_assets_arg(assets_dir: &Path) -> Option<PathBuf> {
    if assets_dir.is_dir() {
        return Some(assets_dir.to_path_buf());
    }
    std::env::current_exe().ok()
}

/// The digest of the stock catalogs this build carries
/// ([`StockCatalogs::digest_with`]).
fn stock_catalog_digest(assets_dir: &Path) -> String {
    StockCatalogs::digest_with(&|rel| pak::read(assets_dir.join(rel)).ok())
}

/// Convert or refresh one mod folder via external `bake mod-convert`. The
/// conversion reads the stock bake from `assets_dir` (or this executable's
/// archive) and the player's stock copies and target choices from
/// `data_dir`.
pub fn convert_mod_with_bake(
    bake_bin: &Path,
    mod_dir: &Path,
    cache_dir: &Path,
    mod_id: &str,
    namespace: &str,
    data_dir: &Path,
    assets_dir: &Path,
) -> Result<()> {
    let mut cmd = Command::new(bake_bin);
    cmd.arg("mod-convert")
        .arg(mod_dir)
        .arg(cache_dir)
        .arg("--mod-id")
        .arg(mod_id)
        .arg("--namespace")
        .arg(namespace)
        .arg("--data-dir")
        .arg(data_dir);
    if let Some(stock) = stock_assets_arg(assets_dir) {
        cmd.arg("--stock-assets").arg(stock);
    }
    let out = cmd
        .output()
        .with_context(|| format!("running {}", bake_bin.display()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        bail!(
            "mod conversion failed for `{}` (exit {:?}): {}{}",
            mod_dir.display(),
            out.status.code(),
            stderr.trim(),
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                ""
            }
        );
    }
    Ok(())
}

fn parse_modes_list(strs: &[String]) -> Vec<Mode> {
    let mut out = Vec::new();
    for s in strs {
        if let Some(m) = Mode::from_subst(s) {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    out
}

/// What a cache is checked against besides the mod's own files.
struct CacheInputs {
    hashes: FileHashCache,
    selections: TargetSelections,
    catalog_digest: String,
    assets_dir: PathBuf,
}

fn load_cached_or_convert(
    entry: &ModFolderEntry,
    data_dir: &Path,
    inputs: &CacheInputs,
    report: &mut MountedModsReport,
) -> Option<(ModCacheMeta, PathBuf)> {
    if let Some(err) = &entry.symlink_error {
        report
            .errors
            .push(format!("Mod `{}` rejected: {err}", entry.folder_name));
        return None;
    }
    let choices = inputs.selections.for_mod(&entry.mod_id);
    let fp = match compute_mod_fingerprint_cached(&entry.path, &inputs.hashes) {
        Ok(f) => f.digest,
        Err(e) => {
            report.errors.push(format!(
                "Failed to fingerprint mod `{}`: {e}",
                entry.folder_name
            ));
            return None;
        }
    };
    let cache_dir = mod_cache_path(data_dir, &entry.namespace);
    let existing = ModCacheMeta::load(&cache_dir);
    let status = match &existing {
        Some(meta) if meta.mod_id == entry.mod_id => meta.validate(
            &cache_dir,
            &fp,
            &meta.expected_replacement_key(&inputs.catalog_digest, &choices),
        ),
        Some(_) => ModCacheStatus::Stale {
            reason: "mod identity changed".into(),
        },
        None => ModCacheStatus::NeedsConversion,
    };

    if matches!(status, ModCacheStatus::Ready) {
        if let Some(meta) = existing {
            return Some((meta, cache_dir));
        }
    }

    let Some(bake_bin) = locate_bake_bin(data_dir) else {
        report.errors.push(format!(
            "Mod `{}` needs conversion ({}) but `bake` executable was not found.",
            entry.folder_name,
            status.label()
        ));
        return None;
    };
    set_converting(Some(entry.folder_name.clone()));
    println!(
        "mods: preparing `{}` ({}) -> {}",
        entry.folder_name,
        entry.mod_id,
        cache_dir.display()
    );
    if let Err(e) = convert_mod_with_bake(
        &bake_bin,
        &entry.path,
        &cache_dir,
        &entry.mod_id,
        &entry.namespace,
        data_dir,
        &inputs.assets_dir,
    ) {
        report.errors.push(format!(
            "Mod `{}` preparation failed: {e}",
            entry.folder_name
        ));
        return None;
    }

    let Some(meta) = ModCacheMeta::load(&cache_dir) else {
        report.errors.push(format!(
            "Mod `{}` converted without producing cache metadata.",
            entry.folder_name
        ));
        return None;
    };
    match meta.validate(
        &cache_dir,
        &fp,
        &meta.expected_replacement_key(&inputs.catalog_digest, &choices),
    ) {
        ModCacheStatus::Ready => Some((meta, cache_dir)),
        other => {
            report.errors.push(format!(
                "Mod `{}` cache validation failed after conversion ({}).",
                entry.folder_name,
                other.label()
            ));
            None
        }
    }
}

fn discovered_from_cache(entry: &ModFolderEntry, meta: &ModCacheMeta) -> DiscoveredMod {
    DiscoveredMod {
        id: entry.mod_id.clone(),
        namespace: entry.namespace.clone(),
        folder_name: entry.folder_name.clone(),
        root_dir: entry.path.clone(),
        content_root: Some(entry.path.clone()),
        legacy_addon_id: meta.legacy_addon_id.clone(),
        display_name: meta.display_name.clone(),
        version: meta.version.clone(),
        author: meta.author.clone(),
        description: meta.description.clone(),
        mod_type: meta.mod_type,
        compatibility: if meta.warnings.is_empty() {
            ModCompatibility::Supported
        } else {
            ModCompatibility::PartialWithWarnings
        },
        source_fingerprint: meta.source_fingerprint.clone(),
        maps: meta.maps.clone(),
        dependencies: meta.dependencies.clone(),
        conflicts: meta.conflicts.clone(),
        declared_overrides: meta.declared_overrides.clone(),
        custom_sides: Vec::new(),
        warnings: meta.warnings.clone(),
        errors: Vec::new(),
        unused_lvls: Vec::new(),
        resource_ops: meta.resource_ops.clone(),
        capabilities: Vec::new(),
        replacement_key: meta.replacement_key.clone(),
    }
}

/// The layer of a converted map's own units and classes
/// (`levels/<map>/`, `bake mod-convert`), served only while that map is
/// loaded; `None` for a map with no SIDE LVLs of its own.
fn level_layer(mod_id: &str, cache_dir: &Path, map_key: &str) -> Option<OverlayLayer> {
    let root = cache_dir.join("levels").join(map_key);
    let assets = root.join("assets");
    let mut entries: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
    // The stock companions the addon's version of a model does not have.
    let removed: Vec<String> = std::fs::read_to_string(root.join("removed.ron"))
        .ok()
        .and_then(|t| ron::from_str(&t).ok())
        .unwrap_or_default();
    for rel in removed {
        entries.insert(rel.to_ascii_lowercase(), None);
    }
    let mut pending = vec![assets.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            // The listing already says which entries are folders; asking
            // each path again costs a disk query per file, and a large mod
            // has tens of thousands. (A cache holds no links.)
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                pending.push(path);
            } else if let Ok(rel) = path.strip_prefix(&assets) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                entries.insert(rel.to_ascii_lowercase(), Some(path));
            }
        }
    }
    (!entries.is_empty()).then(|| OverlayLayer {
        id: mod_id.to_owned(),
        mount_name: "assets_baked".into(),
        rule: OverlayRule::Overrides {
            entries,
            scope: Some(format!("{}{}", pak::LEVEL_SCOPE, map_key.to_ascii_lowercase())),
        },
    })
}

/// What [`mount_enabled_mods`] is doing, for the title screen to show.
#[derive(Clone, Default)]
pub struct MountStatus {
    /// The folder of the mod being converted, which takes minutes.
    pub converting: Option<String>,
}

static MOUNT_STATUS: RwLock<MountStatus> = RwLock::new(MountStatus { converting: None });

pub fn mount_status() -> MountStatus {
    MOUNT_STATUS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn set_converting(mod_folder: Option<String>) {
    MOUNT_STATUS
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .converting = mod_folder;
}

/// Scan `openbf_data/mods/`, ensure caches for enabled mods from `extensions.ron`,
/// validate dependencies and load order, and mount VFS & localization overlays.
pub fn mount_enabled_mods(
    data_dir: &Path,
    assets_dir: &Path,
    missions_dir: &Path,
    disable_mods: bool,
) -> MountedModsReport {
    clear_overlays();
    crate::event_feed::clear_mod_locls();

    let _ = pak::mount(assets_dir, "assets_baked");
    let _ = pak::mount(missions_dir, "missions");

    if disable_mods {
        let rep = MountedModsReport {
            disabled: true,
            ..Default::default()
        };
        *RUNTIME_STATE.write().unwrap_or_else(|e| e.into_inner()) = rep.clone();
        return rep;
    }

    let mut report = MountedModsReport::default();
    let profile = ExtensionProfile::load(&profile_path(data_dir));
    let mdir = mods_dir(data_dir);
    let folders = match list_mod_folders(&mdir) {
        Ok(list) => list,
        Err(e) => {
            report
                .errors
                .push(format!("Could not scan {}: {e}", mdir.display()));
            *RUNTIME_STATE.write().unwrap_or_else(|e| e.into_inner()) = report.clone();
            return report;
        }
    };

    let mut by_id: BTreeMap<String, ModFolderEntry> = BTreeMap::new();
    let mut visible_ids = Vec::with_capacity(folders.len());
    for entry in folders {
        if profile.is_enabled(&entry.mod_id) {
            visible_ids.push(entry.mod_id.clone());
            by_id.insert(entry.mod_id.clone(), entry);
        }
    }
    if visible_ids.is_empty() {
        *RUNTIME_STATE.write().unwrap_or_else(|e| e.into_inner()) = report.clone();
        return report;
    }

    let ordered_enabled = profile.ordered_visible(&visible_ids);
    let inputs = CacheInputs {
        hashes: FileHashCache::load(data_dir),
        selections: TargetSelections::load(data_dir),
        catalog_digest: stock_catalog_digest(assets_dir),
        assets_dir: assets_dir.to_path_buf(),
    };
    let mut cache_by_id: BTreeMap<String, (ModFolderEntry, ModCacheMeta, PathBuf)> =
        BTreeMap::new();
    let mut discovered_list = Vec::new();

    for &mod_id in &ordered_enabled {
        let Some(entry) = by_id.get(mod_id) else {
            continue;
        };
        set_converting(None);
        if let Some((meta, cache_dir)) =
            load_cached_or_convert(entry, data_dir, &inputs, &mut report)
        {
            discovered_list.push(discovered_from_cache(entry, &meta));
            cache_by_id.insert(mod_id.to_owned(), (entry.clone(), meta, cache_dir));
        }
    }

    let (plan, resolution_errors) = resolve_content_mods_isolated(&discovered_list, &profile);
    if !resolution_errors.is_empty() {
        for err in &resolution_errors {
            let msg = err.to_string();
            eprintln!("mods: {msg}");
            report.errors.push(msg);
        }
        eprintln!(
            "mods: isolated {} incompatible/conflicting mod error(s); unrelated valid mods remain active. Fix or disable conflicting mods in openbf-launcher -> Mods tab, or launch with `--no-mods`.",
            resolution_errors.len()
        );
    }

    report.conflicts = plan.conflicts.clone();
    report.warnings = plan.warnings.clone();
    for conflict in &report.conflicts {
        eprintln!(
            "mods: override conflict on `{}` — `{}` wins over {}",
            conflict.resource_path,
            conflict.winner_name,
            conflict.overridden_names.join(", ")
        );
    }
    for err in &report.errors {
        eprintln!("mods: {err}");
    }

    set_converting(None);
    let mut overlays: Vec<OverlayLayer> = Vec::new();
    // Each addon map's own units stand over everything else on that map.
    let mut level_layers: Vec<OverlayLayer> = Vec::new();
    let mut locls: Vec<Arc<HudLocl>> = Vec::new();

    for mod_id in &plan.active_order {
        let Some((entry, meta, cache_dir)) = cache_by_id.get(mod_id) else {
            continue;
        };
        let assets_cache = cache_dir.join("assets");
        for map in &meta.maps {
            let map_dir = assets_cache.join(&map.map_key);
            if map_dir.is_dir() {
                overlays.push(OverlayLayer {
                    id: mod_id.clone(),
                    mount_name: "assets_baked".into(),
                    rule: OverlayRule::NamespaceDir {
                        prefix: map.map_key.clone(),
                        disk_dir: map_dir,
                    },
                });
            }
        }
        for map in &meta.maps {
            level_layers.extend(level_layer(mod_id, cache_dir, &map.map_key));
        }
        report.mod_maps.extend(catalog_entries(
            &meta.maps,
            &entry.folder_name,
            &meta.display_name,
        ));

        let missions_cache = cache_dir.join("missions");
        if missions_cache.is_dir() {
            overlays.push(OverlayLayer {
                id: mod_id.clone(),
                mount_name: "missions".into(),
                rule: OverlayRule::NamespacePrefix {
                    prefix: format!("{}__", meta.namespace),
                    disk_dir: missions_cache,
                },
            });
        }

        if !meta.declared_overrides.is_empty() {
            let override_assets = cache_dir.join("overrides").join("assets");
            if override_assets.is_dir() {
                let mut entries: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
                for p in &meta.declared_overrides {
                    let norm = p.replace('\\', "/").trim_start_matches('/').to_owned();
                    let lower = norm.to_ascii_lowercase();
                    let disk_path = override_assets.join(&norm);
                    if disk_path.is_file() {
                        entries.insert(lower, Some(disk_path));
                    }
                }
                if !entries.is_empty() {
                    overlays.push(OverlayLayer {
                        id: mod_id.clone(),
                        mount_name: "assets_baked".into(),
                        rule: OverlayRule::Overrides {
                            entries,
                            scope: None,
                        },
                    });
                }
            }
        }

        // Stock replacements: one layer per game, so a BF2 side replacement
        // never reaches a BF1 battle (`pak::set_overlay_scope`). Each entry
        // maps its canonical lowercase key to its exact on-disk path (or
        // `None` when explicitly removed), avoiding case-sensitivity bugs.
        let override_assets = cache_dir.join("overrides").join("assets");
        let mut by_game: BTreeMap<&str, BTreeMap<String, Option<PathBuf>>> = BTreeMap::new();
        for out in &meta.replacement_outputs {
            let norm = out.path.replace('\\', "/").trim_start_matches('/').to_owned();
            let lower = norm.to_ascii_lowercase();
            let target = if out.removed {
                None
            } else {
                Some(override_assets.join(&norm))
            };
            by_game
                .entry(out.game.key())
                .or_default()
                .insert(lower, target);
        }
        for (game, entries) in by_game {
            overlays.push(OverlayLayer {
                id: mod_id.clone(),
                mount_name: "assets_baked".into(),
                rule: OverlayRule::Overrides {
                    entries,
                    scope: Some(game.to_owned()),
                },
            });
        }

        if meta.has_shell_icons {
            let icon_dir = cache_dir
                .join("assets")
                .join(format!("{}__shell_icons", meta.namespace));
            if icon_dir.is_dir() {
                report.shell_icon_dirs.push(icon_dir);
            }
        }
        for (mode_key, mode_meta) in &meta.custom_modes {
            let entry_meta = report
                .custom_modes
                .entry(mode_key.to_ascii_lowercase())
                .or_default();
            if entry_meta.name.is_none() {
                entry_meta.name = mode_meta.name.clone();
            }
            if entry_meta.description.is_none() {
                entry_meta.description = mode_meta.description.clone();
            }
            if entry_meta.icon.is_none() {
                entry_meta.icon = mode_meta.icon.clone();
            }
        }

        let places = cache_dir.join("hud").join("weapon_icons.ron");
        if let Ok(text) = std::fs::read_to_string(&places) {
            match ron::from_str(&text) {
                Ok(places) => report.hud_icon_places.push(places),
                Err(e) => report
                    .warnings
                    .push(format!("{}: {e}", places.display())),
            }
        }

        if meta.has_locl {
            let locl_path = cache_dir.join("locl").join("english.ron");
            if let Ok(text) = std::fs::read_to_string(&locl_path) {
                if let Ok(tbl) = ron::from_str::<HudLocl>(&text) {
                    locls.push(Arc::new(tbl));
                }
            }
        }

        report
            .active_fingerprints
            .push((mod_id.clone(), meta.source_fingerprint.clone()));
        report.active.push(MountedMod {
            mod_id: mod_id.clone(),
            namespace: meta.namespace.clone(),
            folder_name: entry.folder_name.clone(),
            display_name: meta.display_name.clone(),
            legacy_addon_id: meta.legacy_addon_id.clone(),
            cache_dir: cache_dir.clone(),
            source_fingerprint: meta.source_fingerprint.clone(),
            maps: meta.maps.clone(),
            declared_overrides: meta.declared_overrides.clone(),
            replacement_key: meta.replacement_key.clone(),
            replaces_gameplay: meta
                .resource_ops
                .iter()
                .any(|o| o.is_active() && o.changes_gameplay()),
            warnings: meta.warnings.clone(),
        });
    }
    inputs.hashes.save();

    // Online play compares global gameplay-affecting overrides: a mod that
    // overrides stock files or changes gameplay through a replacement must
    // match on every machine. Additional unused addon maps are intersected
    // per-map via `ContentManifest::maps` so having an extra map installed
    // does not block stock play; purely cosmetic replacements stay out of
    // this digest (and `Sim::static_digest` remains the final authority at
    // round load).
    let digest_fps: Vec<String> = report
        .active
        .iter()
        .zip(report.active_fingerprints.iter())
        .map(|(m, (_, fp))| {
            if m.replaces_gameplay {
                format!("{fp}:{}", m.replacement_key)
            } else {
                fp.clone()
            }
        })
        .collect();
    let override_pairs: Vec<(&str, &str)> = report
        .active
        .iter()
        .zip(report.active_fingerprints.iter())
        .zip(digest_fps.iter())
        .filter(|((m, _), _)| !m.declared_overrides.is_empty() || m.replaces_gameplay)
        .map(|((_, (id, _)), fp)| (id.as_str(), fp.as_str()))
        .collect();
    let digest = content_mod_multiplayer_digest(&override_pairs, !report.active.is_empty());
    report.override_digest = if digest == 0 {
        String::new()
    } else {
        format!("{digest:016x}")
    };

    level_layers.append(&mut overlays);
    set_overlays(level_layers);
    crate::event_feed::set_mod_locls(locls);

    if !report.active.is_empty() {
        println!(
            "mods: mounted {} content mod(s): {}",
            report.active.len(),
            report
                .active
                .iter()
                .enumerate()
                .map(|(i, m)| format!("#{} {} ({})", i + 1, m.display_name, m.mod_id))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    *RUNTIME_STATE.write().unwrap_or_else(|e| e.into_inner()) = report.clone();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use extensions::mods::{
        compute_mod_fingerprint, ModContentType, ModMissionSummary, ReplacementOutput,
        MOD_CACHE_SCHEMA_VERSION, MOD_CONVERTER_ID,
    };
    use extensions::profile::ExtensionProfile;
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    #[test]
    fn a_picked_mission_is_drawn_among_its_scripts_rosters() {
        let row = |file: &str, script: &str, weight: u8, multiplayer: bool| ModMissionSummary {
            script_name: script.into(),
            mode: "con".into(),
            era: "gcw".into(),
            mission_ron: file.into(),
            single_player_weight: weight,
            multiplayer_variant: multiplayer,
            supported: true,
            unsupported_reason: None,
        };
        let rows = [
            row("a.ron", "abrg_con", 1, false),
            row("b.ron", "abrg_con", 2, false),
            row("online.ron", "abrg_con", 0, true),
            row("other.ron", "abrg_ctf", 1, true),
        ];
        assert_eq!(choose_roster(&rows, "a.ron", 7, true), Some("online.ron"));
        let drawn: BTreeSet<&str> = (0..64)
            .filter_map(|seed| choose_roster(&rows, "a.ron", seed, false))
            .collect();
        assert_eq!(drawn, BTreeSet::from(["a.ron", "b.ron"]));
        assert_eq!(
            choose_roster(&rows, "a.ron", 11, false),
            choose_roster(&rows, "b.ron", 11, false),
            "one seed, one draw, whichever row was picked"
        );
        assert_eq!(choose_roster(&rows, "missing.ron", 7, false), None);
    }

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn make_cached_mod(
        data_dir: &Path,
        folder: &str,
        id: &str,
        ns: &str,
        map_key: &str,
        override_text: &str,
        locl_cp1: &str,
    ) {
        let mod_dir = mods_dir(data_dir).join(folder);
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("marker.txt"), folder).unwrap();
        let fp = compute_mod_fingerprint(&mod_dir).unwrap();

        let cache = mod_cache_path(data_dir, ns);
        let map_dir = cache.join("assets").join(map_key);
        std::fs::create_dir_all(&map_dir).unwrap();
        std::fs::write(
            map_dir.join("manifest.ron"),
            format!("(map: \"{map_key}\", source_game: \"bf2\", terrain_name: None, sky_name: None, terrain_glb: None, models: [], textures: [], classes: [], geometries: [], regions: [], instances: [], collision: [], layers: [])"),
        )
        .unwrap();

        let missions_dir = cache.join("missions");
        std::fs::create_dir_all(&missions_dir).unwrap();
        let mission_ron = format!("{map_key}_con.ron");
        std::fs::write(
            missions_dir.join(&mission_ron),
            format!("(map: \"{map_key}\", mode: \"con\", source_game: \"bf2\", teams: [(faction: \"all\", units: 16, tickets: 150), (faction: \"imp\", units: 16, tickets: 150)], ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25)"),
        )
        .unwrap();

        let ov_path = cache
            .join("overrides")
            .join("assets")
            .join("side_bf2/classes/all.ron");
        std::fs::create_dir_all(ov_path.parent().unwrap()).unwrap();
        std::fs::write(&ov_path, override_text).unwrap();

        let locl_path = cache.join("locl").join("english.ron");
        std::fs::create_dir_all(locl_path.parent().unwrap()).unwrap();
        let tbl = HudLocl {
            language: "english".into(),
            entries: vec![(manifest::hud::locl_key("level.abr.cp1"), locl_cp1.into())],
        };
        std::fs::write(&locl_path, ron::to_string(&tbl).unwrap()).unwrap();

        let meta = ModCacheMeta {
            schema_version: MOD_CACHE_SCHEMA_VERSION,
            converter: MOD_CONVERTER_ID.into(),
            mod_id: id.into(),
            namespace: ns.into(),
            source_fingerprint: fp.digest,
            legacy_addon_id: Some("ABR".into()),
            display_name: format!("Mod {folder}"),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::LegacyHybridAddon,
            maps: vec![ModMapSummary {
                map_key: map_key.into(),
                legacy_code: "abr".into(),
                display_name: format!("Map {folder}"),
                description: String::new(),
                source_lvl: "ABR/ABR.lvl".into(),
                variant_title: None,
                stock_row: None,
                modes_cw: Vec::new(),
                modes_gcw: vec!["con".into()],
                modes_custom_eras: BTreeMap::new(),
                missions: vec![ModMissionSummary {
                    script_name: format!("{ns}_con"),
                    mode: "con".into(),
                    era: "gcw".into(),
                    mission_ron,
                    single_player_weight: 1,
                    multiplayer_variant: true,
                    supported: true,
                    unsupported_reason: None,
                }],
                sound_planets: Vec::new(),
                era_overrides: BTreeMap::new(),
            }],
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: vec!["side_bf2/classes/all.ron".into()],
            has_locl: true,
            warnings: Vec::new(),
            resource_ops: Vec::new(),
            replacement_key: String::new(),
            replacement_outputs: Vec::new(),
            unsupported: Vec::new(),
            catalog_digest: StockCatalogs::digest_with(&|_| None),
            target_choices: BTreeMap::new(),
            custom_modes: BTreeMap::new(),
            has_shell_icons: false,
        };
        meta.save_atomic(&cache).unwrap();
    }

    #[test]
    #[ignore = "requires the prepared KTH cache in OPENBF_KTH_TEST_DATA"]
    fn kothlis_catalog_uses_one_row_and_selects_authored_weather() {
        let data = PathBuf::from(std::env::var_os("OPENBF_KTH_TEST_DATA").unwrap());
        let cache = data.join("mod_cache/mod_kth");
        let meta = ModCacheMeta::load(&cache).unwrap();
        let rows = catalog_entries(&meta.maps, "KTH", "KTH");
        assert_eq!(rows.len(), 1);
        use ui::shell::{IaCommand, IaInput, IaTexts, InstantAction, ModePick, PointerTarget};
        let missions = cache.join("missions");
        let maps = ui::shell::catalog_with_mods(
            &|_, _| None,
            &[],
            &|key, mode| crate::mission::pairs(&missions, key, mode.subst()),
            &rows,
        );
        let mut shell = InstantAction::new(
            maps,
            IaTexts::new(&|_, english| english.to_owned()),
            Default::default(),
        );
        let confirm = IaInput {
            menu: ui::menu::MenuInput {
                confirm: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let kothlis = shell
            .map_rows()
            .iter()
            .position(|r| r.text.contains("Kothlis"))
            .unwrap();
        assert!(shell.focus_pointer(PointerTarget::Map(kothlis)));
        shell.step(confirm, 0.0);
        let hunt = shell
            .mode_picks()
            .iter()
            .position(|m| *m == ModePick::One(Mode::Hunt))
            .unwrap();
        assert!(shell.focus_pointer(PointerTarget::Mode(hunt)));
        shell.step(confirm, 0.0);
        let clone_wars = shell
            .side_rows()
            .iter()
            .position(|row| row.text == "Clone Wars")
            .unwrap();
        assert!(shell.focus_pointer(PointerTarget::Era(clone_wars)));
        shell.step(confirm, 0.0);
        let Some(IaCommand::Launch(rows_to_launch)) = shell.step(confirm, 0.0) else {
            panic!("Clone Wars Hunt did not launch");
        };
        assert_eq!(rows_to_launch.len(), 1);
        assert_eq!(rows_to_launch[0].armies, ["rep", "cis"]);

        let row = &rows[0];
        assert_eq!(row.variant_keys.len(), 5);
        for (mode, multiplayer_weather) in [
            ("con", ["dawn", "rain"]),
            ("ctf", ["night", "dusk"]),
            ("hunt", ["dusk", "night"]),
        ] {
            for (era, weather) in ["cw", "gcw"].into_iter().zip(multiplayer_weather) {
                let choose = |seed, multiplayer| {
                    choose_world_variant(row, meta.maps.iter(), mode, era, seed, multiplayer)
                        .unwrap()
                };
                let seen = (42..142)
                    .map(|seed| choose(seed, false))
                    .collect::<BTreeSet<_>>();
                assert_eq!(seen.len(), 5, "{mode}/{era}");
                assert_eq!(
                    choose(42, true),
                    format!("mod_kth__kth_{weather}"),
                    "{mode}/{era}"
                );
            }
        }
        for (weather, effect, pulse) in [
            ("dusk", "parcan_light_blue_static", 0.0),
            ("night", "parcan_light_blue", 2.0),
        ] {
            let path = cache.join(format!("assets/mod_kth__kth_{weather}/effects.ron"));
            let fx: manifest::FxSet =
                ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let beam = fx.beams.iter().find(|beam| beam.name == effect).unwrap();
            assert!(beam.entity_light);
            assert_eq!(beam.color, [0, 185, 240, 255]);
            assert_eq!(beam.length, 9.0);
            assert_eq!(beam.pulse_period, pulse);
        }

        let dusk = cache.join("assets/mod_kth__kth_dusk/effects.ron");
        let fx: manifest::FxSet = ron::from_str(&std::fs::read_to_string(dusk).unwrap()).unwrap();
        let corona = fx
            .beams
            .iter()
            .find(|beam| beam.name == "kth_main_discolight")
            .unwrap();
        assert!(corona.corona_flare);
        assert_eq!(corona.flare_intensity, 0.7);
        let pick = |armies| {
            crate::mission::pick(&cache.join("missions"), "mod_kth__kth_dusk", "hunt", armies)
                .unwrap()
        };
        let cw = pick(["rep", "cis"]);
        let gcw = pick(["imp", "all"]);
        assert!(cw
            .mission
            .class_load_order
            .iter()
            .any(|c| c.request == "rep_inf_ep3_sniper"));
        assert!(gcw
            .mission
            .class_load_order
            .iter()
            .any(|c| c.request == "imp_inf_sniper"));
    }

    #[test]
    fn load_order_governs_overrides_and_disabling_clears_overlays() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("openbf-mods-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let assets = root.join("assets_baked");
        let missions = root.join("missions");
        let data = root.join("openbf_data");
        std::fs::create_dir_all(assets.join("side_bf2/classes")).unwrap();
        std::fs::create_dir_all(&missions).unwrap();
        std::fs::write(assets.join("side_bf2/classes/all.ron"), "stock").unwrap();

        make_cached_mod(
            &data,
            "ABR_A",
            "content.abr_a",
            "mod_abr_a",
            "mod_abr_a__abr",
            "alpha_override",
            "Alpha CP",
        );
        make_cached_mod(
            &data,
            "ABR_B",
            "content.abr_b",
            "mod_abr_b",
            "mod_abr_b__abr",
            "beta_override",
            "Beta CP",
        );

        let mut profile = ExtensionProfile::default();
        profile.set_enabled("content.abr_a", true);
        profile.set_enabled("content.abr_b", true);
        profile.order = vec!["content.abr_a".into(), "content.abr_b".into()];
        profile.save(&profile_path(&data)).unwrap();

        let rep1 = mount_enabled_mods(&data, &assets, &missions, false);
        assert!(rep1.errors.is_empty(), "{:?}", rep1.errors);
        assert_eq!(rep1.conflicts.len(), 1);
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/classes/all.ron")).unwrap(),
            "alpha_override"
        );
        assert!(pak::exists(assets.join("mod_abr_a__abr/manifest.ron")));
        assert!(pak::exists(assets.join("mod_abr_b__abr/manifest.ron")));
        assert_eq!(
            crate::event_feed::Texts::load(&assets, "bf2").post_name("level.abr.cp1"),
            "Alpha CP"
        );

        // Swap priority: B above A -> B wins overrides and localization while both maps coexist.
        profile.order = vec!["content.abr_b".into(), "content.abr_a".into()];
        profile.save(&profile_path(&data)).unwrap();
        let rep2 = mount_enabled_mods(&data, &assets, &missions, false);
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/classes/all.ron")).unwrap(),
            "beta_override"
        );
        assert_ne!(rep1.override_digest, rep2.override_digest);
        assert_eq!(
            crate::event_feed::Texts::load(&assets, "bf2").post_name("level.abr.cp1"),
            "Beta CP"
        );

        // Disable B -> A remains active and wins cleanly.
        profile.set_enabled("content.abr_b", false);
        profile.save(&profile_path(&data)).unwrap();
        mount_enabled_mods(&data, &assets, &missions, false);
        assert!(!pak::exists(assets.join("mod_abr_b__abr/manifest.ron")));
        assert!(pak::exists(assets.join("mod_abr_a__abr/manifest.ron")));
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/classes/all.ron")).unwrap(),
            "alpha_override"
        );

        // --no-mods restores stock cleanly.
        let disabled = mount_enabled_mods(&data, &assets, &missions, true);
        assert!(disabled.disabled);
        assert!(active_mod_maps().is_empty());
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/classes/all.ron")).unwrap(),
            "stock"
        );

        clear_runtime_mods();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bf2_scoped_replacements_leave_bf1_untouched_and_hide_removed_files() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("openbf-mods-scope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let assets = root.join("assets_baked");
        let missions = root.join("missions");
        let data = root.join("openbf_data");
        std::fs::create_dir_all(assets.join("side_bf2/soldiers")).unwrap();
        std::fs::create_dir_all(&missions).unwrap();
        std::fs::write(assets.join("side_bf2/soldiers/imp.glb"), "stock_glb").unwrap();
        std::fs::write(assets.join("side_bf2/soldiers/imp.cloth.ron"), "stock_cloth").unwrap();

        let mod_dir = mods_dir(&data).join("SideMod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("imp.lvl"), "mod").unwrap();
        let fp = compute_mod_fingerprint(&mod_dir).unwrap();
        let cache = mod_cache_path(&data, "mod_sidemod");
        let ov = cache.join("overrides/assets/side_bf2/soldiers");
        std::fs::create_dir_all(&ov).unwrap();
        std::fs::write(ov.join("imp.glb"), "mod_glb").unwrap();
        let meta = ModCacheMeta {
            schema_version: MOD_CACHE_SCHEMA_VERSION,
            converter: MOD_CONVERTER_ID.into(),
            mod_id: "content.sidemod".into(),
            namespace: "mod_sidemod".into(),
            source_fingerprint: fp.digest,
            legacy_addon_id: None,
            display_name: "SideMod".into(),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::LegacySideOverhaul,
            maps: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            has_locl: false,
            warnings: Vec::new(),
            resource_ops: Vec::new(),
            replacement_key: String::new(),
            replacement_outputs: vec![
                ReplacementOutput {
                    path: "side_bf2/soldiers/imp.glb".into(),
                    game: extensions::mods::catalog::SourceGame::Bf2,
                    removed: false,
                },
                ReplacementOutput {
                    path: "side_bf2/soldiers/imp.cloth.ron".into(),
                    game: extensions::mods::catalog::SourceGame::Bf2,
                    removed: true,
                },
            ],
            unsupported: Vec::new(),
            catalog_digest: stock_catalog_digest(&assets),
            target_choices: BTreeMap::new(),
            custom_modes: BTreeMap::new(),
            has_shell_icons: false,
        };
        meta.save_atomic(&cache).unwrap();

        let mut profile = ExtensionProfile::default();
        profile.set_enabled("content.sidemod", true);
        profile.order = vec!["content.sidemod".into()];
        profile.save(&profile_path(&data)).unwrap();

        let rep = mount_enabled_mods(&data, &assets, &missions, false);
        assert!(rep.errors.is_empty(), "{:?}", rep.errors);

        // BF1 scope must remain untouched by BF2-only replacements.
        pak::set_overlay_scope(Some("bf1"));
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/soldiers/imp.glb")).unwrap(),
            "stock_glb"
        );
        assert!(pak::is_file(assets.join("side_bf2/soldiers/imp.cloth.ron")));

        // BF2 scope receives the replacement and hides the removed companion.
        pak::set_overlay_scope(Some("bf2"));
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/soldiers/imp.glb")).unwrap(),
            "mod_glb"
        );
        assert!(!pak::is_file(assets.join("side_bf2/soldiers/imp.cloth.ron")));

        // Disabling restores stock immediately.
        mount_enabled_mods(&data, &assets, &missions, true);
        assert_eq!(
            pak::read_to_string(assets.join("side_bf2/soldiers/imp.glb")).unwrap(),
            "stock_glb"
        );
        assert!(pak::is_file(assets.join("side_bf2/soldiers/imp.cloth.ron")));

        pak::set_overlay_scope(None);
        clear_runtime_mods();
        let _ = std::fs::remove_dir_all(&root);
    }
}
