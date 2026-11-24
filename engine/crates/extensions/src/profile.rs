//! Persistent extension profile (`openbf_data/extensions.ron`), CLI overrides,
//! startup error reporting (`openbf_data/last_startup_error.ron`), and shared
//! data-directory discovery.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// File name of the persistent extension activation profile inside `openbf_data/`.
pub const PROFILE_FILE: &str = "extensions.ron";
/// File name of the last extension startup failure report inside `openbf_data/`.
pub const STARTUP_ERROR_FILE: &str = "last_startup_error.ron";
/// Subdirectory under `openbf_data/` containing user-copied legacy BF2 mod folders.
pub const MODS_DIR: &str = "mods";
/// Subdirectory under `openbf_data/` holding generated versioned native mod caches.
pub const MOD_CACHE_DIR: &str = "mod_cache";
/// Environment variable overriding `openbf_data/` location for both launcher and game.
pub const DATA_DIR_ENV: &str = "OPENBF_DATA_DIR";
/// Environment variable set by the launcher when spawning `openbf`.
pub const LAUNCHED_BY_LAUNCHER_ENV: &str = "OPENBF_LAUNCHED_BY_LAUNCHER";

const CURRENT_SCHEMA_VERSION: u32 = 2;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// Persistent user configuration storing enabled extensions and optional order.
///
/// Missing entries default to disabled (`false`) so bundled extensions like
/// `openbf.vr` start disabled for new installations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionProfile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub enabled: BTreeMap<String, bool>,
    /// Reserved load-order list for future order-sensitive mods/extensions.
    #[serde(default)]
    pub order: Vec<String>,
}

impl Default for ExtensionProfile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            enabled: BTreeMap::new(),
            order: Vec::new(),
        }
    }
}

impl ExtensionProfile {
    /// Load profile from `path`, returning defaults when missing or unreadable.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        ron::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default()
    }

    /// Save profile atomically to `path`.
    pub fn save(&self, path: &Path) -> Result<()> {
        write_ron_atomic(path, self, "extension profile")
    }

    /// Whether `id` is enabled in this profile (disabled by default when absent).
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }

    /// Update the enabled flag for `id`.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        self.enabled.insert(id.to_string(), enabled);
    }

    /// Reconcile persistent `order` with the currently discovered content mod IDs.
    ///
    /// Preserves the relative order of all previously recorded IDs — including
    /// temporarily disabled or removed folders so re-adding restores priority —
    /// deduplicates entries, and appends newly discovered IDs in alphabetical
    /// order at the end.
    pub fn reconcile_order(&mut self, discovered_ids: &[String]) {
        self.schema_version = CURRENT_SCHEMA_VERSION;
        let mut seen = std::collections::BTreeSet::new();
        self.order.retain(|id| seen.insert(id.clone()));

        let mut new_ids: Vec<String> = discovered_ids
            .iter()
            .filter(|id| seen.insert((*id).clone()))
            .cloned()
            .collect();
        new_ids.sort();
        self.order.extend(new_ids);
    }

    /// Return currently discovered IDs ordered by `self.order` (index 0 =
    /// highest override priority), placing any unrecorded IDs alphabetically at
    /// the end.
    pub fn ordered_visible<'a>(&self, discovered_ids: &'a [String]) -> Vec<&'a str> {
        let mut rank: BTreeMap<&str, usize> = BTreeMap::new();
        for (idx, id) in self.order.iter().enumerate() {
            rank.entry(id.as_str()).or_insert(idx);
        }
        let mut out: Vec<&'a str> = discovered_ids.iter().map(String::as_str).collect();
        out.sort_by(|a, b| match (rank.get(a), rank.get(b)) {
            (Some(ra), Some(rb)) => ra.cmp(rb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.cmp(b),
        });
        out
    }

    /// Move `id` up one position among currently visible mods (giving it higher
    /// override priority), persisting across hidden/removed IDs without dead clicks.
    pub fn move_up(&mut self, id: &str, visible_ids: &[String]) -> bool {
        self.reconcile_order(visible_ids);
        let ordered: Vec<String> = self
            .ordered_visible(visible_ids)
            .into_iter()
            .map(str::to_owned)
            .collect();
        let Some(pos) = ordered.iter().position(|x| x == id) else {
            return false;
        };
        if pos == 0 {
            return false;
        }
        let neighbor = &ordered[pos - 1];
        let Some(i_cur) = self.order.iter().position(|x| x == id) else {
            return false;
        };
        let Some(i_prev) = self.order.iter().position(|x| x == neighbor) else {
            return false;
        };
        self.order.swap(i_cur, i_prev);
        true
    }

    /// Move `id` down one position among currently visible mods.
    pub fn move_down(&mut self, id: &str, visible_ids: &[String]) -> bool {
        self.reconcile_order(visible_ids);
        let ordered: Vec<String> = self
            .ordered_visible(visible_ids)
            .into_iter()
            .map(str::to_owned)
            .collect();
        let Some(pos) = ordered.iter().position(|x| x == id) else {
            return false;
        };
        if pos + 1 >= ordered.len() {
            return false;
        }
        let neighbor = &ordered[pos + 1];
        let Some(i_cur) = self.order.iter().position(|x| x == id) else {
            return false;
        };
        let Some(i_next) = self.order.iter().position(|x| x == neighbor) else {
            return false;
        };
        self.order.swap(i_cur, i_next);
        true
    }

    /// 1-based visual priority rank (`#1` = highest override priority).
    pub fn priority_rank(&self, id: &str, visible_ids: &[String]) -> Option<usize> {
        self.ordered_visible(visible_ids)
            .into_iter()
            .position(|x| x == id)
            .map(|i| i + 1)
    }
}

/// Process-local CLI overrides applied on top of [`ExtensionProfile`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartupOverrides {
    /// Force desktop backend (`--desktop` / `--no-vr`), disabling any
    /// `ExclusiveCapability::DisplayBackend` provider for this launch.
    pub force_desktop: bool,
    /// Extension IDs explicitly requested on the command line (e.g. `--vr` -> `openbf.vr`).
    pub enable: Vec<String>,
    /// Extension IDs explicitly suppressed on the command line.
    pub disable: Vec<String>,
}

/// Persistent record of an extension failure before or during display initialization.
///
/// Written by `game` before exiting on startup failure so `openbf-launcher` can
/// display a diagnostic banner and offer one-click desktop recovery or disabling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupErrorReport {
    pub extension_id: String,
    pub extension_name: String,
    pub summary: String,
    pub details: String,
    #[serde(default)]
    pub timestamp_secs: u64,
    #[serde(default = "default_true")]
    pub can_retry_desktop: bool,
}

fn default_true() -> bool {
    true
}

impl StartupErrorReport {
    pub fn new(
        extension_id: impl Into<String>,
        extension_name: impl Into<String>,
        summary: impl Into<String>,
        details: impl Into<String>,
        can_retry_desktop: bool,
    ) -> Self {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self {
            extension_id: extension_id.into(),
            extension_name: extension_name.into(),
            summary: summary.into(),
            details: details.into(),
            timestamp_secs,
            can_retry_desktop,
        }
    }

    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        ron::from_str(&text).ok()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_ron_atomic(path, self, "startup error report")
    }

    pub fn clear(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

/// Resolve the portable `openbf_data/` directory consistently across `openbf` and `openbf-launcher`.
///
/// Priority:
/// 1. Explicit `env_override` (`OPENBF_DATA_DIR`).
/// 2. When running out of a Cargo `target/` directory inside a repository checkout
///    (containing `scripts/pack.py`), `<checkout_root>/dist/openbf_data`.
/// 3. `<exe_dir>/openbf_data`.
pub fn resolve_data_dir(
    exe_path: &Path,
    cwd: Option<&Path>,
    env_override: Option<&OsStr>,
) -> PathBuf {
    if let Some(override_dir) = env_override.filter(|s| !s.is_empty()) {
        return PathBuf::from(override_dir);
    }
    let exe_dir = exe_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let checkout_root = find_checkout_root(&exe_dir).or_else(|| cwd.and_then(find_checkout_root));
    if let Some(root) = checkout_root.filter(|_| in_cargo_target(&exe_dir)) {
        return root.join("dist").join("openbf_data");
    }
    exe_dir.join("openbf_data")
}

/// Discover `openbf_data/` for the running process using `current_exe`, `current_dir`, and `OPENBF_DATA_DIR`.
pub fn discover_data_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("openbf"));
    let cwd = std::env::current_dir().ok();
    let env = std::env::var_os(DATA_DIR_ENV);
    resolve_data_dir(&exe, cwd.as_deref(), env.as_deref())
}

pub fn profile_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PROFILE_FILE)
}

pub fn startup_error_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STARTUP_ERROR_FILE)
}

pub fn mods_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(MODS_DIR)
}

pub fn mod_cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(MOD_CACHE_DIR)
}

fn in_cargo_target(dir: &Path) -> bool {
    dir.ancestors()
        .any(|a| a.file_name().is_some_and(|n| n == "target"))
}

fn find_checkout_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join("scripts").join("pack.py").is_file())
        .map(Path::to_path_buf)
}

pub(crate) fn write_ron_atomic<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let pretty = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .new_line("\n".to_string());
    let text = ron::ser::to_string_pretty(value, pretty)
        .with_context(|| format!("serializing {label}"))?;
    let tmp = path.with_extension(format!("ron.tmp.{}", std::process::id()));
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    replace_file(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    if dst.exists() {
        let backup = dst.with_extension(format!("ron.bak.{}", std::process::id()));
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(dst, &backup)?;
        if let Err(err) = std::fs::rename(src, dst) {
            let _ = std::fs::rename(&backup, dst);
            return Err(err.into());
        }
        let _ = std::fs::remove_file(&backup);
        return Ok(());
    }
    std::fs::rename(src, dst)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openbf_ext_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_profile_disables_all_extensions_including_vr() {
        let profile = ExtensionProfile::default();
        assert!(!profile.is_enabled(crate::OPENBF_VR_ID));
        assert!(!profile.is_enabled("some.other.ext"));
    }

    #[test]
    fn profile_round_trips_atomically_and_recovers_from_corrupt_files() {
        let dir = scratch("profile_rt");
        let path = profile_path(&dir);

        assert_eq!(ExtensionProfile::load(&path), ExtensionProfile::default());

        let mut profile = ExtensionProfile::default();
        profile.set_enabled(crate::OPENBF_VR_ID, true);
        profile.save(&path).expect("save profile");
        let loaded = ExtensionProfile::load(&path);
        assert_eq!(loaded, profile);
        assert!(loaded.is_enabled(crate::OPENBF_VR_ID));

        profile.set_enabled(crate::OPENBF_VR_ID, false);
        profile.save(&path).expect("overwrite profile");
        assert!(!ExtensionProfile::load(&path).is_enabled(crate::OPENBF_VR_ID));

        std::fs::write(&path, "not valid ron {{{").unwrap();
        assert_eq!(ExtensionProfile::load(&path), ExtensionProfile::default());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_error_report_round_trips_and_clears() {
        let dir = scratch("error_rt");
        let path = startup_error_path(&dir);
        assert_eq!(StartupErrorReport::load(&path), None);

        let report = StartupErrorReport::new(
            crate::OPENBF_VR_ID,
            "OpenBF VR",
            "No active OpenXR runtime found",
            "Install or start SteamVR, Oculus, or Windows Mixed Reality.",
            true,
        );
        report.save(&path).expect("save report");
        assert_eq!(StartupErrorReport::load(&path), Some(report));

        StartupErrorReport::clear(&path);
        assert_eq!(StartupErrorReport::load(&path), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn data_dir_resolution_matches_release_checkout_and_env_override() {
        let dir = scratch("data_dir");
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts").join("pack.py"), "# stub\n").unwrap();

        let target_exe = dir
            .join("engine")
            .join("target")
            .join("debug")
            .join("game.exe");
        assert_eq!(
            resolve_data_dir(&target_exe, Some(&dir), None),
            dir.join("dist").join("openbf_data")
        );

        let release_exe = dir.join("bundle").join("openbf.exe");
        assert_eq!(
            resolve_data_dir(&release_exe, None, None),
            dir.join("bundle").join("openbf_data")
        );

        let custom = dir.join("custom_data");
        assert_eq!(
            resolve_data_dir(&release_exe, None, Some(custom.as_os_str())),
            custom
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_v1_profile_and_preserves_order_across_disable_remove_and_new_mods() {
        let dir = scratch("order_reconcile");
        let path = profile_path(&dir);
        let v1 = "(schema_version: 1, enabled: {\"openbf.vr\": true, \"content.a\": true}, order: [\"content.b\", \"content.a\"])";
        std::fs::write(&path, v1).unwrap();

        let mut profile = ExtensionProfile::load(&path);
        assert!(profile.is_enabled(crate::OPENBF_VR_ID));
        assert!(profile.is_enabled("content.a"));

        // Removing content.b temporarily while keeping visible [content.a, content.c]
        let visible_without_b = vec!["content.a".to_string(), "content.c".to_string()];
        profile.reconcile_order(&visible_without_b);
        assert_eq!(
            profile.ordered_visible(&visible_without_b),
            ["content.a", "content.c"]
        );
        assert_eq!(profile.order, ["content.b", "content.a", "content.c"]);

        // Re-adding content.b restores its saved higher priority above content.a!
        let visible_restored = vec![
            "content.c".to_string(),
            "content.b".to_string(),
            "content.a".to_string(),
        ];
        assert_eq!(
            profile.ordered_visible(&visible_restored),
            ["content.b", "content.a", "content.c"]
        );

        // Removing content.a temporarily while moving content.c above content.b still works without dead clicks.
        let visible_bc = vec!["content.b".to_string(), "content.c".to_string()];
        assert!(profile.move_up("content.c", &visible_bc));
        assert_eq!(
            profile.ordered_visible(&visible_bc),
            ["content.c", "content.b"]
        );
        assert_eq!(profile.priority_rank("content.c", &visible_bc), Some(1));
        assert_eq!(profile.priority_rank("content.b", &visible_bc), Some(2));
        assert!(!profile.move_up("content.c", &visible_bc));
        assert!(!profile.move_down("content.b", &visible_bc));

        // Disabling a mod keeps its exact order slot.
        profile.set_enabled("content.c", false);
        assert_eq!(profile.priority_rank("content.c", &visible_bc), Some(1));

        profile.save(&path).unwrap();
        let reloaded = ExtensionProfile::load(&path);
        assert_eq!(reloaded.schema_version, 2);
        assert_eq!(
            reloaded.ordered_visible(&visible_bc),
            ["content.c", "content.b"]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
