//! One mod folder, inspected the same way by `bake` and the launcher: the
//! addon layout ([`crate::addon`]), the source fingerprint, and every LVL that
//! is not an addon's own, scanned ([`crate::inventory`]) and placed against
//! the stock catalog ([`extensions::mods::replace`]).

use extensions::mods::catalog::{LvlFacts, StockCatalogs};
use extensions::mods::fingerprint::{mtime_ns, FileHashCache};
use extensions::mods::replace::{
    mod_capabilities, replacement_key, resolve_replacements, Capability, DeclaredReplacement,
    OpStatus, ResourceOp, SourceLvl, TargetSelections,
};
use extensions::mods::{
    compute_mod_fingerprint_cached, merge_optional_manifest, read_explicit_manifest,
    DiscoveredMod, ModCompatibility, ModContentType, ModMapSummary, ModMissionSummary,
};
use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::addon::{
    inspect_addon_dir, AddonCompatibility, AddonInspection, AddonKind, OverrideLvl,
};

pub const LVL_FACTS_CACHE: &str = "cache/lvl_facts.ron";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FactsEntry {
    len: u64,
    mtime_ns: u64,
    facts: LvlFacts,
}

/// LVL scans by absolute path, valid while length and modification time
/// stay the same.
#[derive(Debug, Default)]
pub struct FactsCache {
    path: Option<PathBuf>,
    entries: Mutex<BTreeMap<String, FactsEntry>>,
    dirty: Mutex<bool>,
}

impl FactsCache {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(LVL_FACTS_CACHE);
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries: Mutex::new(entries),
            dirty: Mutex::new(false),
        }
    }

    pub fn facts(&self, path: &Path, hashes: &FileHashCache) -> std::io::Result<LvlFacts> {
        let meta = std::fs::metadata(path)?;
        let key = path.to_string_lossy().into_owned();
        let mt = mtime_ns(&meta);
        if let Some(e) = self.entries.lock().unwrap().get(&key) {
            if e.len == meta.len() && e.mtime_ns == mt && mt != 0 {
                return Ok(e.facts.clone());
            }
        }
        let facts = crate::inventory::scan_file(path)?;
        hashes.remember(path, &meta, &facts.sha256);
        self.entries.lock().unwrap().insert(
            key,
            FactsEntry {
                len: meta.len(),
                mtime_ns: mt,
                facts: facts.clone(),
            },
        );
        *self.dirty.lock().unwrap() = true;
        Ok(facts)
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        if !std::mem::take(&mut *self.dirty.lock().unwrap()) {
            return;
        }
        let entries = self.entries.lock().unwrap();
        if let Err(err) = extensions::profile::write_ron_atomic(path, &*entries, "LVL facts cache")
        {
            eprintln!("modscan: failed to save {}: {err:#}", path.display());
        }
    }
}

/// What an inspection reads besides the mod folder itself.
pub struct ScanContext<'a> {
    pub catalogs: &'a StockCatalogs,
    pub selections: &'a TargetSelections,
    pub hashes: &'a FileHashCache,
    pub facts: &'a FactsCache,
}


fn content_type(kind: AddonKind) -> ModContentType {
    match kind {
        AddonKind::LegacyMapAddon => ModContentType::LegacyMapAddon,
        AddonKind::LegacySideOverhaul => ModContentType::LegacySideOverhaul,
        AddonKind::LegacyAssetPack => ModContentType::LegacyAssetPack,
        AddonKind::LegacyHybridAddon => ModContentType::LegacyHybridAddon,
        AddonKind::Unsupported => ModContentType::Unsupported,
        AddonKind::Invalid => ModContentType::Invalid,
    }
}

fn compatibility(c: &AddonCompatibility) -> ModCompatibility {
    match c {
        AddonCompatibility::Supported => ModCompatibility::Supported,
        AddonCompatibility::PartialWithWarnings => ModCompatibility::PartialWithWarnings,
        AddonCompatibility::Unsupported(s) => ModCompatibility::Unsupported(s.clone()),
        AddonCompatibility::Invalid(s) => ModCompatibility::Invalid(s.clone()),
    }
}

fn map_summaries(insp: &AddonInspection, namespace: &str) -> Vec<ModMapSummary> {
    insp.map_groups(namespace)
        .iter()
        .map(|g| ModMapSummary {
            map_key: g.map_key.clone(),
            legacy_code: g.legacy_code.clone(),
            display_name: g.display_name.clone(),
            description: g.description.clone(),
            source_lvl: g.source_lvl_rel.clone(),
            variant_title: g.variant_title.clone(),
            modes_cw: g.modes_cw.clone(),
            modes_gcw: g.modes_gcw.clone(),
            missions: g
                .missions
                .iter()
                .map(|pm| ModMissionSummary {
                    script_name: pm.mission.script_name.clone(),
                    mode: pm.mission.mode.clone(),
                    era: pm.mission.era.clone(),
                    mission_ron: pm.mission_ron.clone(),
                    single_player_weight: pm.mission.single_player_weight,
                    multiplayer_variant: pm.mission.multiplayer_variant,
                    supported: pm.mission.supported,
                    unsupported_reason: pm.mission.unsupported_reason.clone(),
                })
                .collect(),
            sound_planets: g.sound_planets.clone(),
        })
        .collect()
}

/// The fields both inspections fill the same way; the caller sets the
/// type, compatibility, warnings, ops, capabilities and keys it computes.
fn base_discovered(
    mod_root: &Path,
    mod_id: &str,
    namespace: &str,
    insp: &AddonInspection,
) -> DiscoveredMod {
    let folder_name = mod_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mod")
        .to_owned();
    let custom_sides: BTreeSet<String> = insp
        .missions
        .iter()
        .flat_map(|m| m.custom_side_lvls.iter().map(|(side, _)| side.clone()))
        .collect();
    let maps = map_summaries(insp, namespace);
    let capabilities = if maps.is_empty() {
        Vec::new()
    } else {
        vec![Capability::AddonMissions]
    };
    DiscoveredMod {
        id: mod_id.to_owned(),
        namespace: namespace.to_owned(),
        folder_name,
        root_dir: mod_root.to_path_buf(),
        content_root: insp.content_root.clone(),
        legacy_addon_id: insp.legacy_addon_id.clone(),
        display_name: insp.display_name.clone(),
        version: None,
        author: insp.author.clone(),
        description: insp.description.clone(),
        mod_type: content_type(insp.kind),
        compatibility: compatibility(&insp.compatibility),
        source_fingerprint: String::new(),
        maps,
        dependencies: Vec::new(),
        conflicts: Vec::new(),
        declared_overrides: Vec::new(),
        custom_sides: custom_sides.into_iter().collect(),
        warnings: insp.warnings.clone(),
        errors: insp.errors.clone(),
        unused_lvls: insp.unused_lvls.clone(),
        resource_ops: Vec::new(),
        capabilities,
        replacement_key: String::new(),
    }
}

/// Lightweight, metadata-only discovery for fast UI listing: inspects the
/// directory layout, `openbf_mod.ron`, `Readme.txt`, and `addme.script` without
/// reading or hashing large replacement `.lvl` files. Returns the provisional
/// [`DiscoveredMod`] alongside the total size in bytes of candidate replacement
/// LVLs (`0` when no replacement LVLs exist).
pub fn quick_inspect_mod_folder(
    mod_root: &Path,
    mod_id: &str,
    namespace: &str,
) -> (DiscoveredMod, u64) {
    let insp = inspect_addon_dir(mod_root);
    let override_bytes: u64 = insp
        .stock_override_lvls
        .iter()
        .filter_map(|o| std::fs::metadata(&o.abs_path).ok().map(|m| m.len()))
        .sum();

    let mut discovered = base_discovered(mod_root, mod_id, namespace, &insp);
    if discovered.maps.is_empty() && !insp.stock_override_lvls.is_empty() {
        let sides_only = insp.stock_override_lvls.iter().all(|o| {
            let rel = o.rel_path.to_ascii_lowercase();
            rel.contains("side/") || rel.contains("fpm/")
        });
        discovered.mod_type = if sides_only {
            ModContentType::LegacySideOverhaul
        } else {
            ModContentType::LegacyAssetPack
        };
        if insp.errors.is_empty() {
            discovered.compatibility = if insp.warnings.is_empty() {
                ModCompatibility::Supported
            } else {
                ModCompatibility::PartialWithWarnings
            };
        }
    }
    if let Some(manifest) = read_explicit_manifest(mod_root) {
        merge_optional_manifest(&mut discovered, manifest);
    }
    (discovered, override_bytes)
}

fn scan_source_lvl(o: &OverrideLvl, ctx: &ScanContext<'_>) -> Result<SourceLvl, String> {
    match ctx.facts.facts(&o.abs_path, ctx.hashes) {
        Ok(facts) => Ok(SourceLvl {
            rel: o.rel_path.clone(),
            facts,
        }),
        Err(e) => Err(format!("Could not read `{}`: {e}", o.rel_path)),
    }
}

/// Inspect one folder of `openbf_data/mods/`.
pub fn inspect_mod_folder(
    mod_root: &Path,
    mod_id: &str,
    namespace: &str,
    ctx: &ScanContext<'_>,
) -> (DiscoveredMod, AddonInspection) {
    let mut insp = inspect_addon_dir(mod_root);

    // Scan replacement LVLs FIRST (in parallel across CPU cores) so each LVL
    // is read and SHA-256 hashed only once, populating `ctx.hashes` before
    // `compute_mod_fingerprint_cached` runs.
    let scanned: Vec<Result<SourceLvl, String>> = if insp.stock_override_lvls.len() > 1 {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(1, 8)
            .min(insp.stock_override_lvls.len());
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results: Vec<Mutex<Option<Result<SourceLvl, String>>>> = (0..insp
            .stock_override_lvls
            .len())
            .map(|_| Mutex::new(None))
            .collect();
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(o) = insp.stock_override_lvls.get(i) else {
                        break;
                    };
                    *results[i].lock().unwrap() = Some(scan_source_lvl(o, ctx));
                });
            }
        });
        results
            .into_iter()
            .filter_map(|slot| slot.into_inner().unwrap())
            .collect()
    } else {
        insp.stock_override_lvls
            .iter()
            .map(|o| scan_source_lvl(o, ctx))
            .collect()
    };

    let mut sources = Vec::new();
    for item in scanned {
        match item {
            Ok(s) => sources.push(s),
            Err(w) => insp.warnings.push(w),
        }
    }

    let fp = compute_mod_fingerprint_cached(mod_root, ctx.hashes).unwrap_or_else(|e| {
        insp.errors.push(format!("Failed to fingerprint mod: {e}"));
        extensions::mods::fingerprint::FingerprintResult {
            digest: String::new(),
            warnings: Vec::new(),
        }
    });
    for w in fp.warnings {
        if !insp.warnings.contains(&w) {
            insp.warnings.push(w);
        }
    }
    // `openbf_mod.ron` is read once: its replacements feed the resolver here
    // and the rest is merged onto the result at the end.
    let mut manifest = read_explicit_manifest(mod_root);
    let declared: Vec<DeclaredReplacement> = manifest
        .as_mut()
        .map(|m| std::mem::take(&mut m.replacements))
        .unwrap_or_default();
    let mut ops: Vec<ResourceOp> = resolve_replacements(
        &sources,
        ctx.catalogs,
        &ctx.selections.for_mod(mod_id),
        &declared,
    );
    if ctx.catalogs.is_empty() {
        for op in &mut ops {
            if !matches!(op.status, OpStatus::Inactive(_)) {
                op.status = OpStatus::Unresolved(
                    "the stock resource catalog is missing; run setup in the launcher".into(),
                );
            }
        }
    }

    let mut discovered = base_discovered(mod_root, mod_id, namespace, &insp);
    let mut capabilities: BTreeSet<Capability> = mod_capabilities(&ops).into_iter().collect();
    capabilities.extend(discovered.capabilities.drain(..));
    let active = ops.iter().filter(|o| o.is_active()).count();
    let ambiguous = ops.iter().filter(|o| o.status.needs_user()).count();

    let warnings = &mut discovered.warnings;
    if ambiguous > 0 {
        warnings.push(format!(
            "{ambiguous} LVL file(s) could match more than one stock file; choose their targets in the Mods tab."
        ));
    }
    let gameplay: Vec<&ResourceOp> = ops
        .iter()
        .filter(|o| o.is_active() && o.changes_gameplay())
        .collect();
    if !gameplay.is_empty() {
        warnings.push(format!(
            "Changes gameplay in {} LVL file(s): {}. Online play requires every player to run the same mods.",
            gameplay.len(),
            gameplay
                .iter()
                .filter_map(|o| o.target.as_ref().map(|t| t.path.clone()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if discovered.maps.is_empty() && !insp.stock_override_lvls.is_empty() {
        let sides_only = ops.iter().filter(|o| o.is_active()).all(|o| {
            o.target
                .as_ref()
                .is_some_and(|t| t.path.starts_with("side/") || t.path.starts_with("fpm/"))
        });
        discovered.mod_type = if active > 0 && sides_only {
            ModContentType::LegacySideOverhaul
        } else {
            ModContentType::LegacyAssetPack
        };
        if active == 0 && ambiguous == 0 && insp.errors.is_empty() {
            let identical = ops
                .iter()
                .filter(|o| o.status == OpStatus::Identical)
                .count();
            let reason = if identical == ops.len() && identical > 0 {
                "Every LVL in this folder is identical to the stock file it would replace.".to_owned()
            } else {
                ops.iter()
                    .find_map(|o| o.status.reason().map(str::to_owned))
                    .unwrap_or_else(|| "No LVL in this folder replaces a stock file.".into())
            };
            discovered.compatibility = ModCompatibility::Unsupported(reason);
        } else if insp.errors.is_empty() {
            discovered.compatibility = if discovered.warnings.is_empty() {
                ModCompatibility::Supported
            } else {
                ModCompatibility::PartialWithWarnings
            };
        }
    }

    discovered.replacement_key = replacement_key(&ops, ctx.catalogs);
    discovered.source_fingerprint = fp.digest;
    discovered.resource_ops = ops;
    discovered.capabilities = capabilities.into_iter().collect();
    if let Some(manifest) = manifest {
        merge_optional_manifest(&mut discovered, manifest);
    }
    (discovered, insp)
}
