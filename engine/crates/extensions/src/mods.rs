//! Shared filesystem-based content mod metadata, identity, cache validation,
//! load-order resolution, and safe source fingerprinting.
//!
//! Used identically by `openbf-launcher`, `bake`, and `game` without requiring
//! `game` to link LVL/Lua parsers (`mungefs`).

pub mod fingerprint;
pub mod resolve;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::profile::write_ron_atomic;

pub use fingerprint::compute_mod_fingerprint;
pub use resolve::{
    content_mod_multiplayer_digest, resolve_content_mods, ContentResolutionError,
    ContentResolutionPlan, ResourceOverrideConflict,
};

/// Bump whenever cached asset/mission schemas change so stale caches auto-rebuild.
pub const MOD_CONVERTER_VERSION: u32 = 2;
pub const MOD_CACHE_SCHEMA_VERSION: u32 = 1;
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModMissionSummary {
    pub script_name: String,
    pub mode: String,
    pub era: String,
    pub mission_ron: String,
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
    #[serde(default)]
    pub modes_cw: Vec<String>,
    #[serde(default)]
    pub modes_gcw: Vec<String>,
    #[serde(default)]
    pub missions: Vec<ModMissionSummary>,
    #[serde(default)]
    pub sound_planets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl ModCacheMeta {
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
    pub fn validate(&self, cache_dir: &Path, expected_fingerprint: &str) -> ModCacheStatus {
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
