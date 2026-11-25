//! Runtime discovery, auto-conversion, validation, priority ordering, and
//! VFS mounting for filesystem-based legacy Battlefront II content mods.
//!
//! Strictly avoids linking LVL/Lua parsers (`mungefs`) into `game`: pre-converted
//! mods are loaded directly from `openbf_data/mod_cache/`, and missing/stale
//! enabled mods are prepared via the external `bake mod-convert` tool.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};

use anyhow::{bail, Context, Result};
use extensions::mods::catalog::StockCatalogs;
use extensions::mods::{
    compute_mod_fingerprint_cached, content_mod_multiplayer_digest, list_mod_folders,
    mod_cache_path, resolve_content_mods, DiscoveredMod, FileHashCache, ModCacheMeta,
    ModCacheStatus, ModCompatibility, ModFolderEntry, ModMapSummary, ResourceOverrideConflict,
    TargetSelections,
};
use extensions::profile::{mods_dir, profile_path, ExtensionProfile};
use manifest::hud::HudLocl;
use pak::{clear_overlays, set_overlays, OverlayLayer, OverlayRule};
use ui::shell::{ModMapEntry, Mode, MODES};

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

fn catalog_entries(maps: &[ModMapSummary], folder: &str) -> Vec<ModMapEntry> {
    let mut entries: Vec<ModMapEntry> = Vec::new();
    let mut groups = BTreeMap::new();
    for map in maps {
        let modes = [
            parse_modes_list(&map.modes_cw),
            parse_modes_list(&map.modes_gcw),
        ];
        if let Some(title) = &map.variant_title {
            let group = (map.source_lvl.as_str(), title.as_str());
            if let Some(&index) = groups.get(&group) {
                let entry: &mut ModMapEntry = &mut entries[index];
                entry.variant_keys.push(map.map_key.clone());
                for (all, variant) in entry.modes.iter_mut().zip(modes) {
                    for mode in variant {
                        if !all.contains(&mode) {
                            all.push(mode);
                        }
                    }
                }
                continue;
            }
            groups.insert(group, entries.len());
        }
        entries.push(ModMapEntry {
            key: map.map_key.clone(),
            legacy_code: map.legacy_code.clone(),
            name: map
                .variant_title
                .as_ref()
                .unwrap_or(&map.display_name)
                .clone(),
            disambiguator: folder.to_owned(),
            description: map.description.clone(),
            modes,
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
    seed: u32,
    multiplayer: bool,
) -> String {
    let state = RUNTIME_STATE.read().unwrap_or_else(|e| e.into_inner());
    let Some(row) = state
        .mod_maps
        .iter()
        .find(|row| row.key == key && row.variant_keys.len() > 1)
    else {
        return key.to_owned();
    };
    let maps = state.active.iter().flat_map(|m| &m.maps);
    let era = if armies.iter().any(|a| a == "rep" || a == "cis") {
        "cw"
    } else {
        "gcw"
    };
    choose_world_variant(row, maps, mode, era, seed, multiplayer)
        .unwrap_or(key)
        .to_owned()
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
    candidates.push(data_dir.join("tools").join(&name));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&name));
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
    candidates.into_iter().find(|p| p.is_file())
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
        if let Some(m) = MODES
            .into_iter()
            .find(|cand| cand.subst().eq_ignore_ascii_case(s))
        {
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
        if let Some((meta, cache_dir)) =
            load_cached_or_convert(entry, data_dir, &inputs, &mut report)
        {
            discovered_list.push(discovered_from_cache(entry, &meta));
            cache_by_id.insert(mod_id.to_owned(), (entry.clone(), meta, cache_dir));
        }
    }

    let plan = match resolve_content_mods(&discovered_list, &profile) {
        Ok(p) => p,
        Err(errs) => {
            for err in errs {
                let msg = err.to_string();
                eprintln!("mods: {msg}");
                report.errors.push(msg);
            }
            *RUNTIME_STATE.write().unwrap_or_else(|e| e.into_inner()) = report.clone();
            return report;
        }
    };

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

    let mut overlays: Vec<OverlayLayer> = Vec::new();
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
        report
            .mod_maps
            .extend(catalog_entries(&meta.maps, &entry.folder_name));

        // Custom side packages (<namespace>_side_<side>).
        if let Ok(rd) = std::fs::read_dir(&assets_cache) {
            let mut side_dirs: Vec<(String, PathBuf)> = rd
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if !p.is_dir() {
                        return None;
                    }
                    let name = p.file_name()?.to_str()?.to_owned();
                    let side_prefix = format!("{}_side_", meta.namespace);
                    name.starts_with(&side_prefix).then_some((name, p))
                })
                .collect();
            side_dirs.sort_by(|a, b| a.0.cmp(&b.0));
            for (prefix, disk_dir) in side_dirs {
                overlays.push(OverlayLayer {
                    id: mod_id.clone(),
                    mount_name: "assets_baked".into(),
                    rule: OverlayRule::NamespaceDir { prefix, disk_dir },
                });
            }
        }

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
                let allowed_paths: BTreeSet<String> = meta
                    .declared_overrides
                    .iter()
                    .map(|p| {
                        p.replace('\\', "/")
                            .trim_start_matches('/')
                            .to_ascii_lowercase()
                    })
                    .collect();
                overlays.push(OverlayLayer {
                    id: mod_id.clone(),
                    mount_name: "assets_baked".into(),
                    rule: OverlayRule::Overrides {
                        allowed_paths,
                        disk_dir: override_assets,
                        scope: None,
                    },
                });
            }
        }

        // Stock replacements: one layer per game, so a BF2 side replacement
        // never reaches a BF1 battle (`pak::set_overlay_scope`).
        let override_assets = cache_dir.join("overrides").join("assets");
        let mut by_game: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        for out in &meta.replacement_outputs {
            by_game
                .entry(out.game.key())
                .or_default()
                .insert(out.path.replace('\\', "/").to_ascii_lowercase());
        }
        for (game, allowed_paths) in by_game {
            overlays.push(OverlayLayer {
                id: mod_id.clone(),
                mount_name: "assets_baked".into(),
                rule: OverlayRule::Overrides {
                    allowed_paths,
                    disk_dir: override_assets.clone(),
                    scope: Some(game.to_owned()),
                },
            });
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

    // Online play compares these: a mod that adds maps, overrides stock
    // files or changes gameplay through a replacement must match on every
    // machine. A purely cosmetic replacement (textures, models, class
    // presentation) does not take part, so players may differ in it.
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
        .filter(|((m, _), _)| {
            !m.declared_overrides.is_empty() || !m.maps.is_empty() || m.replaces_gameplay
        })
        .map(|((_, (id, _)), fp)| (id.as_str(), fp.as_str()))
        .collect();
    let digest = content_mod_multiplayer_digest(&override_pairs, !report.active.is_empty());
    report.override_digest = if digest == 0 {
        String::new()
    } else {
        format!("{digest:016x}")
    };

    set_overlays(overlays);
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
        MOD_CACHE_SCHEMA_VERSION, MOD_CONVERTER_VERSION,
    };
    use extensions::profile::ExtensionProfile;
    use std::sync::Mutex;

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
            converter_version: MOD_CONVERTER_VERSION,
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
                modes_cw: Vec::new(),
                modes_gcw: vec!["con".into()],
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
            catalog_digest: String::new(),
            target_choices: BTreeMap::new(),
        };
        meta.save_atomic(&cache).unwrap();
    }

    #[test]
    #[ignore = "requires the prepared KTH cache in OPENBF_KTH_TEST_DATA"]
    fn kothlis_catalog_uses_one_row_and_selects_authored_weather() {
        let data = PathBuf::from(std::env::var_os("OPENBF_KTH_TEST_DATA").unwrap());
        let cache = data.join("mod_cache/mod_kth");
        let meta = ModCacheMeta::load(&cache).unwrap();
        let rows = catalog_entries(&meta.maps, "KTH");
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
            converter_version: MOD_CONVERTER_VERSION,
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
            catalog_digest: String::new(),
            target_choices: BTreeMap::new(),
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
