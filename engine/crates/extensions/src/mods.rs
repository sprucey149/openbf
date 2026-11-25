//! Shared filesystem-based content mod metadata, identity, cache validation,
//! load-order resolution, and safe source fingerprinting.
//!
//! Used identically by `openbf-launcher`, `bake`, and `game` without requiring
//! `game` to link LVL/Lua parsers (`mungefs`).

pub mod catalog;
pub mod fingerprint;
pub mod replace;
pub mod resolve;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::profile::write_ron_atomic;

pub use fingerprint::{compute_mod_fingerprint, compute_mod_fingerprint_cached, FileHashCache};
pub use replace::{Capability, OpStatus, ResourceOp, TargetChoice, TargetSelections};
pub use resolve::{
    content_mod_multiplayer_digest, resolve_content_mods, resolve_content_mods_isolated,
    ContentResolutionError, ContentResolutionPlan, ResourceOverrideConflict,
};

/// Bump whenever cached asset/mission schemas change so stale caches auto-rebuild.
/// 8: legacy resource replacements (stock catalog, class patches, re-baked
/// side models and textures).
/// 9: PC terrain & foliage directional lighting from `normal_terrain_shader.xml`.
/// 10: soldier skeletons re-parented onto the pose rig's hierarchy (helper
/// `root_*` bones between rig bones no longer double-apply their bind).
/// 11: a modded soldier whose `_low1` shares no texture sheet with its high
/// model gets no far LOD (the stock low body was drawn far off).
pub const MOD_CONVERTER_VERSION: u32 = 11;
pub const MOD_CACHE_SCHEMA_VERSION: u32 = 1;

/// The `bake` subcommand that prints the [`MOD_CONVERTER_VERSION`] it writes.
pub const BAKE_CONVERTER_VERSION_ARG: &str = "mod-converter-version";

/// The [`MOD_CONVERTER_VERSION`] a `bake` binary writes, or `None` when it
/// cannot run or predates [`BAKE_CONVERTER_VERSION_ARG`].
pub fn bake_converter_version(bin: &Path) -> Option<u32> {
    let mut cmd = std::process::Command::new(bin);
    cmd.arg(BAKE_CONVERTER_VERSION_ARG)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// The first existing `bake` among `candidates` that writes this build's
/// [`MOD_CONVERTER_VERSION`], else the first existing one. A stale copy left
/// in an earlier search location can no longer shadow the matching `bake`
/// shipped beside the game and launcher.
pub fn pick_bake_bin(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for path in candidates {
        if path.is_file() && !seen.contains(&path) {
            seen.push(path);
        }
    }
    seen.iter()
        .find(|p| bake_converter_version(p) == Some(MOD_CONVERTER_VERSION))
        .or_else(|| seen.first())
        .cloned()
}
pub const CACHE_META_FILE: &str = "cache_meta.ron";
pub const MOD_MANIFEST_FILE: &str = "openbf_mod.ron";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModContentType {
    LegacyMapAddon,
    LegacySideOverhaul,
    LegacyAssetPack,
    LegacyHybridAddon,
    Unsupported,
    Invalid,
}

impl ModContentType {
    pub fn label(self) -> &'static str {
        match self {
            Self::LegacyMapAddon => "Map Addon",
            Self::LegacySideOverhaul => "Side Overhaul",
            Self::LegacyAssetPack => "Asset Pack",
            Self::LegacyHybridAddon => "Hybrid Addon",
            Self::Unsupported => "Unsupported",
            Self::Invalid => "Invalid",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModCompatibility {
    Supported,
    PartialWithWarnings,
    Unsupported(String),
    Invalid(String),
}

impl ModCompatibility {
    pub fn is_playable(&self) -> bool {
        matches!(self, Self::Supported | Self::PartialWithWarnings)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Supported => "Supported",
            Self::PartialWithWarnings => "Supported (warnings)",
            Self::Unsupported(_) => "Unsupported",
            Self::Invalid(_) => "Invalid",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unsupported(r) | Self::Invalid(r) => Some(r.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModCacheStatus {
    Ready,
    NeedsConversion,
    Stale { reason: String },
    Converting,
    Failed(String),
    NotApplicable,
}

impl ModCacheStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Ready => "Ready",
            Self::NeedsConversion => "Pending",
            Self::Stale { .. } => "Needs Rebuild",
            Self::Converting => "Converting...",
            Self::Failed(_) => "Failed",
            Self::NotApplicable => "N/A",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenBfModManifest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub overrides: Vec<String>,
    /// Stock LVL targets for this mod's LVLs, where the evidence alone does
    /// not settle them (`replace::DeclaredReplacement`).
    #[serde(default)]
    pub replacements: Vec<replace::DeclaredReplacement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModMissionSummary {
    pub script_name: String,
    pub mode: String,
    pub era: String,
    pub mission_ron: String,
    /// Number of equivalent single-player world selections in the bounded trace.
    #[serde(default)]
    pub single_player_weight: u8,
    #[serde(default)]
    pub multiplayer_variant: bool,
    pub supported: bool,
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModMapSummary {
    pub map_key: String,
    pub legacy_code: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub source_lvl: String,
    /// The shared display title when one mission selects among world bakes.
    #[serde(default)]
    pub variant_title: Option<String>,
    #[serde(default)]
    pub modes_cw: Vec<String>,
    #[serde(default)]
    pub modes_gcw: Vec<String>,
    #[serde(default)]
    pub missions: Vec<ModMissionSummary>,
    #[serde(default)]
    pub sound_planets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredMod {
    pub id: String,
    pub namespace: String,
    pub folder_name: String,
    pub root_dir: PathBuf,
    #[serde(default)]
    pub content_root: Option<PathBuf>,
    #[serde(default)]
    pub legacy_addon_id: Option<String>,
    pub display_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: String,
    pub mod_type: ModContentType,
    pub compatibility: ModCompatibility,
    pub source_fingerprint: String,
    #[serde(default)]
    pub maps: Vec<ModMapSummary>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub declared_overrides: Vec<String>,
    #[serde(default)]
    pub custom_sides: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub unused_lvls: Vec<String>,
    /// Every LVL of the mod placed against the stock catalog
    /// ([`replace::resolve_replacements`]).
    #[serde(default)]
    pub resource_ops: Vec<ResourceOp>,
    /// What the mod's content does, from its addon missions and its active
    /// replacements.
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// [`replace::replacement_key`] of `resource_ops`: part of the cache key.
    #[serde(default)]
    pub replacement_key: String,
}

impl DiscoveredMod {
    pub fn active_ops(&self) -> impl Iterator<Item = &ResourceOp> {
        self.resource_ops.iter().filter(|o| o.is_active())
    }

    pub fn ambiguous_ops(&self) -> impl Iterator<Item = &ResourceOp> {
        self.resource_ops.iter().filter(|o| o.status.needs_user())
    }

    pub fn changes_gameplay(&self) -> bool {
        self.active_ops().any(ResourceOp::changes_gameplay)
    }
}

/// One file a replacement wrote below the cache's `overrides/assets/`,
/// served over the stock file of the same path while a battle of `game` runs
/// ([`pak::set_overlay_scope`](../../pak/fn.set_overlay_scope.html)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementOutput {
    /// Relative to the baked asset root, `/`-separated.
    pub path: String,
    pub game: catalog::SourceGame,
    /// The stock file is hidden rather than replaced: the mod's version of
    /// the resource has no such file (no cloth, no low-detail model).
    #[serde(default)]
    pub removed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModCacheMeta {
    pub schema_version: u32,
    pub converter_version: u32,
    pub mod_id: String,
    pub namespace: String,
    pub source_fingerprint: String,
    #[serde(default)]
    pub legacy_addon_id: Option<String>,
    pub display_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: String,
    pub mod_type: ModContentType,
    #[serde(default)]
    pub maps: Vec<ModMapSummary>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub declared_overrides: Vec<String>,
    #[serde(default)]
    pub has_locl: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The replacements this cache was converted for.
    #[serde(default)]
    pub resource_ops: Vec<ResourceOp>,
    #[serde(default)]
    pub replacement_key: String,
    /// Files below `overrides/assets/` the replacements wrote.
    #[serde(default)]
    pub replacement_outputs: Vec<ReplacementOutput>,
    /// Per active replacement: what could not be converted, honestly.
    #[serde(default)]
    pub unsupported: Vec<String>,
    /// [`catalog::StockCatalogs::digest`] of the stock catalogs this cache
    /// was resolved against.
    #[serde(default)]
    pub catalog_digest: String,
    /// The player's target choices for this mod when it was converted
    /// ([`replace::TargetSelections::for_mod`]).
    #[serde(default)]
    pub target_choices: BTreeMap<String, TargetChoice>,
}

impl ModCacheMeta {
    /// The replacement key this cache still answers to, for a reader that
    /// cannot resolve replacements itself (the game): the cache's own key
    /// while the stock catalogs and the player's target choices match what it
    /// was converted with. A pure addon map (with maps, no replacement ops,
    /// and no target choices) stays valid across stock catalog updates; any
    /// other mod (including one previously unresolved when catalogs were
    /// absent) invalidates when the stock catalog or target choices change.
    pub fn expected_replacement_key(
        &self,
        catalog_digest: &str,
        choices: &BTreeMap<String, TargetChoice>,
    ) -> String {
        if self.catalog_digest == catalog_digest && &self.target_choices == choices {
            return self.replacement_key.clone();
        }
        if self.resource_ops.is_empty() && !self.maps.is_empty() && choices.is_empty() {
            return self.replacement_key.clone();
        }
        "inputs-changed".into()
    }

    pub fn path_in(cache_dir: &Path) -> PathBuf {
        cache_dir.join(CACHE_META_FILE)
    }

    pub fn load(cache_dir: &Path) -> Option<Self> {
        let text = fs::read_to_string(Self::path_in(cache_dir)).ok()?;
        ron::from_str(&text).ok()
    }

    pub fn save_atomic(&self, cache_dir: &Path) -> Result<()> {
        write_ron_atomic(&Self::path_in(cache_dir), self, "mod cache metadata")
    }

    /// Validate cached outputs against converter version, source fingerprint,
    /// and expected generated files on disk without allowing path traversal.
    pub fn validate(
        &self,
        cache_dir: &Path,
        expected_fingerprint: &str,
        expected_replacement_key: &str,
    ) -> ModCacheStatus {
        if self.schema_version != MOD_CACHE_SCHEMA_VERSION {
            return ModCacheStatus::Stale {
                reason: format!(
                    "cache schema {} != {MOD_CACHE_SCHEMA_VERSION}",
                    self.schema_version
                ),
            };
        }
        if self.converter_version != MOD_CONVERTER_VERSION {
            return ModCacheStatus::Stale {
                reason: format!(
                    "converter updated (v{} -> v{MOD_CONVERTER_VERSION})",
                    self.converter_version
                ),
            };
        }
        if self.source_fingerprint != expected_fingerprint {
            return ModCacheStatus::Stale {
                reason: "mod source files changed".into(),
            };
        }
        if self.replacement_key != expected_replacement_key {
            return ModCacheStatus::Stale {
                reason: "replacement targets or the stock catalog changed".into(),
            };
        }
        for out in self.replacement_outputs.iter().filter(|o| !o.removed) {
            let rel = &out.path;
            if !is_safe_rel_path(rel)
                || !cache_dir.join("overrides").join("assets").join(rel).is_file()
            {
                return ModCacheStatus::Stale {
                    reason: format!("missing converted replacement `{rel}`"),
                };
            }
        }
        for map in &self.maps {
            if !is_safe_rel_token(&map.map_key) {
                return ModCacheStatus::Stale {
                    reason: format!("invalid map key `{}` in cache metadata", map.map_key),
                };
            }
            let manifest_path = cache_dir
                .join("assets")
                .join(&map.map_key)
                .join("manifest.ron");
            if !manifest_path.is_file() {
                return ModCacheStatus::Stale {
                    reason: format!("missing cached manifest for `{}`", map.map_key),
                };
            }
            for m in &map.missions {
                if m.supported {
                    if !is_safe_rel_path(&m.mission_ron) {
                        return ModCacheStatus::Stale {
                            reason: format!("invalid mission path `{}`", m.mission_ron),
                        };
                    }
                    if !cache_dir.join("missions").join(&m.mission_ron).is_file() {
                        return ModCacheStatus::Stale {
                            reason: format!("missing cached mission `{}`", m.mission_ron),
                        };
                    }
                }
            }
        }
        if self.has_locl && !cache_dir.join("locl").join("english.ron").is_file() {
            return ModCacheStatus::Stale {
                reason: "missing cached localization table".into(),
            };
        }
        for rel in &self.declared_overrides {
            if !is_safe_rel_path(rel) {
                return ModCacheStatus::Stale {
                    reason: format!("invalid override path `{rel}`"),
                };
            }
            let candidates = [
                cache_dir.join("overrides").join("assets").join(rel),
                cache_dir.join("overrides").join("missions").join(rel),
                cache_dir.join("overrides").join(rel),
            ];
            if !candidates.iter().any(|p| p.is_file()) {
                return ModCacheStatus::Stale {
                    reason: format!("missing cached override `{rel}`"),
                };
            }
        }
        ModCacheStatus::Ready
    }
}

pub fn sanitize_slug(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in raw.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_sep = false;
        } else if (lower == '-' || lower == '_' || lower == ' ') && !out.is_empty() && !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "mod".to_owned()
    } else {
        trimmed
    }
}

fn short_hash_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut hex = String::with_capacity(8);
    for b in &digest[..4] {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Derive a deterministic, collision-resistant mod ID independent of the
/// legacy 3-character map ID (`ABR` vs two folders both shipping `ABR`).
pub fn stable_mod_id(
    folder_name: &str,
    all_folder_names: &[String],
    explicit_id: Option<&str>,
) -> String {
    if let Some(explicit) = explicit_id.map(str::trim).filter(|s| !s.is_empty()) {
        let slug = sanitize_slug(explicit);
        if slug.starts_with("content.") {
            return slug;
        }
        return format!("content.{slug}");
    }
    let base_slug = sanitize_slug(folder_name);
    let slug_collisions = all_folder_names
        .iter()
        .filter(|other| sanitize_slug(other) == base_slug)
        .count();
    if slug_collisions > 1 {
        format!("content.{base_slug}_{}", short_hash_hex(folder_name))
    } else {
        format!("content.{base_slug}")
    }
}

/// Safe filesystem/mount namespace prefix (`mod_<slug>`) from a mod ID.
pub fn namespace_prefix(mod_id: &str) -> String {
    let raw = mod_id.strip_prefix("content.").unwrap_or(mod_id);
    format!("mod_{}", sanitize_slug(raw))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModFolderEntry {
    pub folder_name: String,
    pub path: PathBuf,
    pub mod_id: String,
    pub namespace: String,
    pub symlink_error: Option<String>,
}

/// Enumerate candidate mod folders inside `openbf_data/mods/`, assigning
/// deterministic stable mod IDs and namespaces without touching LVL parsers.
pub fn list_mod_folders(mods_dir: &Path) -> std::io::Result<Vec<ModFolderEntry>> {
    if !mods_dir.is_dir() {
        return Ok(Vec::new());
    }
    let root_canon = fs::canonicalize(mods_dir)?;
    let mut dirs: Vec<(String, PathBuf, Option<String>)> = Vec::new();
    for entry in fs::read_dir(mods_dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let fname = entry.file_name().to_string_lossy().into_owned();
        if fname.starts_with('.') {
            continue;
        }
        let mut symlink_error = None;
        if meta.file_type().is_symlink() {
            match fs::canonicalize(&path) {
                Ok(target) if target.starts_with(&root_canon) => {}
                _ => {
                    symlink_error = Some(format!(
                        "Refused top-level symlink `{fname}` pointing outside mods directory."
                    ));
                }
            }
        }
        if meta.is_dir() || symlink_error.is_some() {
            dirs.push((fname, path, symlink_error));
        }
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let folder_names: Vec<String> = dirs.iter().map(|(n, _, _)| n.clone()).collect();
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::with_capacity(dirs.len());

    for (folder_name, path, symlink_error) in dirs {
        let explicit_id = if symlink_error.is_none() {
            read_explicit_manifest(&path).and_then(|m| m.id)
        } else {
            None
        };
        let mut mod_id = stable_mod_id(&folder_name, &folder_names, explicit_id.as_deref());
        if !used_ids.insert(mod_id.clone()) {
            mod_id = format!("{mod_id}_{}", short_hash_hex(&folder_name));
            used_ids.insert(mod_id.clone());
        }
        let namespace = namespace_prefix(&mod_id);
        out.push(ModFolderEntry {
            folder_name,
            path,
            mod_id,
            namespace,
            symlink_error,
        });
    }
    Ok(out)
}

pub fn read_explicit_manifest(mod_root: &Path) -> Option<OpenBfModManifest> {
    let manifest_path = mod_root.join(MOD_MANIFEST_FILE);
    let meta = fs::symlink_metadata(&manifest_path).ok()?;
    if meta.file_type().is_symlink() {
        let root_c = fs::canonicalize(mod_root).ok()?;
        let target_c = fs::canonicalize(&manifest_path).ok()?;
        if !target_c.starts_with(&root_c) {
            return None;
        }
    }
    if meta.len() > 64 * 1024 {
        return None;
    }
    let text = fs::read_to_string(&manifest_path).ok()?;
    ron::from_str(&text).ok()
}

pub fn is_safe_rel_token(s: &str) -> bool {
    !s.is_empty() && !s.contains('/') && !s.contains('\\') && !s.contains("..") && !s.contains(':')
}

pub fn is_safe_rel_path(s: &str) -> bool {
    if s.is_empty() || s.starts_with('/') || s.starts_with('\\') || s.contains(':') {
        return false;
    }
    let p = Path::new(s);
    !p.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

/// Apply optional `openbf_mod.ron` overrides cleanly onto discovered fields.
pub fn merge_optional_manifest(mod_info: &mut DiscoveredMod, manifest: OpenBfModManifest) {
    if let Some(name) = manifest.name.filter(|s| !s.trim().is_empty()) {
        mod_info.display_name = name;
    }
    if manifest.version.is_some() {
        mod_info.version = manifest.version;
    }
    if let Some(author) = manifest.author.filter(|s| !s.trim().is_empty()) {
        mod_info.author = Some(author);
    }
    if let Some(desc) = manifest.description.filter(|s| !s.trim().is_empty()) {
        mod_info.description = desc;
    }
    for dep in manifest.dependencies {
        if !dep.trim().is_empty() && !mod_info.dependencies.contains(&dep) {
            mod_info.dependencies.push(dep);
        }
    }
    for conf in manifest.conflicts {
        if !conf.trim().is_empty() && !mod_info.conflicts.contains(&conf) {
            mod_info.conflicts.push(conf);
        }
    }
    for ov in manifest.overrides {
        let norm = ov.replace('\\', "/");
        if is_safe_rel_path(&norm) && !mod_info.declared_overrides.contains(&norm) {
            mod_info.declared_overrides.push(norm);
        }
    }
}

/// Deterministic helper mapping `cache_dir` path for a namespace under `mod_cache/`.
pub fn mod_cache_path(data_dir: &Path, namespace: &str) -> PathBuf {
    crate::profile::mod_cache_dir(data_dir).join(namespace)
}

pub fn dedup_sorted(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let set: BTreeSet<String> = items.into_iter().collect();
    set.into_iter().collect()
}

pub fn normalize_override_map_key(rel_path: &str) -> String {
    rel_path
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

pub fn collect_map_modes_summary(map: &ModMapSummary) -> BTreeMap<&'static str, Vec<String>> {
    let mut out = BTreeMap::new();
    out.insert("gcw", map.modes_gcw.clone());
    out.insert("cw", map.modes_cw.clone());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_bake_bin_skips_missing_and_falls_back_to_the_first_existing() {
        let dir = std::env::temp_dir().join(format!("openbf_pick_bake_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a"), dir.join("b"));
        fs::write(&a, b"not a bake").unwrap();
        fs::write(&b, b"not a bake").unwrap();
        assert_eq!(bake_converter_version(&a), None);
        assert_eq!(
            pick_bake_bin([dir.join("missing"), a.clone(), b.clone()]),
            Some(a.clone())
        );
        assert_eq!(pick_bake_bin([dir.join("missing")]), None);
        let _ = fs::remove_dir_all(&dir);
    }

    fn empty_meta(maps: Vec<ModMapSummary>, catalog_digest: &str) -> ModCacheMeta {
        ModCacheMeta {
            schema_version: MOD_CACHE_SCHEMA_VERSION,
            converter_version: MOD_CONVERTER_VERSION,
            mod_id: "content.test".into(),
            namespace: "mod_test".into(),
            source_fingerprint: "fp".into(),
            legacy_addon_id: None,
            display_name: "Test".into(),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::LegacyAssetPack,
            maps,
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            has_locl: false,
            warnings: Vec::new(),
            resource_ops: Vec::new(),
            replacement_key: "empty".into(),
            replacement_outputs: Vec::new(),
            unsupported: Vec::new(),
            catalog_digest: catalog_digest.into(),
            target_choices: BTreeMap::new(),
        }
    }

    #[test]
    fn unresolved_replacement_mod_invalidates_when_catalog_appears() {
        let meta = empty_meta(Vec::new(), "");
        let choices = BTreeMap::new();
        assert_eq!(meta.expected_replacement_key("", &choices), "empty");
        assert_eq!(
            meta.expected_replacement_key("new-catalog-sha256", &choices),
            "inputs-changed"
        );
    }

    #[test]
    fn pure_addon_map_stays_valid_across_catalog_updates() {
        let map = ModMapSummary {
            map_key: "mod_test__m1".into(),
            legacy_code: "M1".into(),
            display_name: "Map 1".into(),
            description: String::new(),
            source_lvl: "m1.lvl".into(),
            variant_title: None,
            modes_cw: vec!["con".into()],
            modes_gcw: Vec::new(),
            missions: Vec::new(),
            sound_planets: Vec::new(),
        };
        let meta = empty_meta(vec![map], "");
        let choices = BTreeMap::new();
        assert_eq!(
            meta.expected_replacement_key("new-catalog-sha256", &choices),
            "empty"
        );
    }
}
